# Where a real API server's watch close lands, and the gaps inside a healthy watch

**2026-09-27 · `k8s-admin` · ephemeral measurement ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15))**

Subject: the number
[D297](../NOTES.md#d297--the-frozen-file-opens-for-a-wedged-watch-and-the-timeout-has-five-seconds-of-room-to-land-in-2026-09-27)
leaves to a measurement — the client `read_timeout` `R` that has to satisfy
`290 < R < 295`. What that requires is where a real API server's close actually
lands relative to its own `timeoutSeconds=290`, and how long a healthy watch is
quiet between two reads, since the field is a deadline on **each read**.

Supersedes the two incidental samples in
[reports/2026-09-26-the-error-state-fix-review.md](2026-09-26-the-error-state-fix-review.md)
§ 1 (290.0 s and 290.1 s, taken through a plain-HTTP relay in front of
`kubectl proxy`, resolution ~0.1 s). Everything below is over TLS straight at the
API server, no proxy and no relay.

## The bench

| | |
|---|---|
| host | test host (`ssh ubuntu`, [D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)); Ubuntu 24.04.4, 4 cores, 3915 MiB, `load average: 0.08` at start; nothing else of this repo's running, one pre-existing unrelated container of the user's |
| cluster | `K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 ./scripts/cluster.sh up`, `kindest/node:v1.36.1`, server `v1.36.1`, 1 control plane + 1 worker; torn down before this report |
| transport | TLS to `https://127.0.0.1:6444`, kubeconfig CA + client certificate, **no ALPN offered**, HTTP/1.1, one watch per TCP connection |
| driver | `/tmp/watchclose.py` on the host, stdlib only (`socket`, `ssl`, `threading`); nothing added to `scripts/` or the `justfile` |
| samples | 30 watch lifetimes over three rounds × 10 concurrent watches (2 per kind × the 5 kinds k8rs watches) |

**Why no ALPN, and why that is the path k8rs uses.** `kube-client-4.2.0` compiles
`hyper-util` with `features = ["client", "client-legacy", "http1", "tokio"]`
(`Cargo.toml:204-211`) — **no `http2`** — and sets `alpn_protocols` nowhere in the
crate (`grep -rn 'alpn_protocols' kube-client-4.2.0/src/` → no match). So the
client offers no ALPN, the server stays on HTTP/1.1, and each of the six watches
is its own connection with its own read deadline. The probe mirrors that by not
calling `set_alpn_protocols`.

**The probe, the parts the numbers turn on.** Per watch: one TLS connection, one
request, `recv()` in a loop with a monotonic timestamp per return.

```
    q = (f"{path}?watch=true&timeoutSeconds={TMO}&allowWatchBookmarks=true"
         f"&resourceVersion={rvs[kind]}")
    ...
    req = (f"GET {q} HTTP/1.1\r\nHost: {HOST}:{PORT}\r\n"
           "Accept: application/json\r\nUser-Agent: watchclose/1\r\n\r\n")
    open_at = time.monotonic()
    t.sendall(req.encode())
```

`elapsed` is measured from `sendall` to the last chunk; `maxgap` is the largest
interval between two successive `recv()` returns and `@` is when it ended;
`reads` counts `recv()` returns. End detection is the HTTP/1.1 last chunk,
followed by a 5 s probe read to see whether a FIN follows it:

```
        if tail.endswith(b"\r\n0\r\n\r\n"):        # HTTP/1.1 last chunk
            body_end = now - open_at
            t.settimeout(5)                          # does a FIN follow it?
            try:
                after = t.recv(4096)
                ended = "last-chunk+FIN" if not after else "last-chunk+more(%db)" % len(after)
            except Exception as e:
                ended = f"last-chunk, socket kept open ({type(e).__name__})"
```

The first attempt had no last-chunk detection and hung on every watch until the
socket timeout — which is itself the answer to question 4 below, found by
accident: the server does not close the connection after the body.
`resourceVersion` per kind comes from `kubectl get --raw '<path>?limit=1'`
immediately before the round.

---

## Round A — idle cluster, 10 concurrent watches

```
$ ssh ubuntu 'timeout 400 python3 /tmp/watchclose.py kind-review "round A - idle cluster, 2 per kind" 2'
 14:01:18 up 23:10,  1 user,  load average: 0.37, 0.41, 0.27
EXIT=0
=== round A - idle cluster, 2 per kind: 10 watches, timeoutSeconds=290, TLS 127.0.0.1:6444 ===
watch               open@  elapsed   ttfb  reads  maxgap      @  end
deployments#2        0.03  290.001  0.002      7   60.59  241.6  last-chunk, socket kept open (TimeoutError)
statefulsets#2       0.03  290.001  0.001      7    60.9   60.9  last-chunk, socket kept open (TimeoutError)
pods#2               0.02  290.002  0.002      7   60.37  180.2  last-chunk, socket kept open (TimeoutError)
deployments#1        0.02  290.002  0.003      7   60.59  241.6  last-chunk, socket kept open (TimeoutError)
daemonsets#1         0.03  290.002  0.004      7   60.69  181.1  last-chunk, socket kept open (TimeoutError)
pods#1               0.03  290.002  0.001      7   60.37  180.2  last-chunk, socket kept open (TimeoutError)
nodes#1              0.03  290.002  0.002     12   60.36  120.3  last-chunk, socket kept open (TimeoutError)
nodes#2              0.02  290.003  0.005     12   60.36  120.4  last-chunk, socket kept open (TimeoutError)
statefulsets#1       0.02  290.003  0.002      7   60.91   60.9  last-chunk, socket kept open (TimeoutError)
daemonsets#2         0.03  290.003  0.002      7    60.7  181.1  last-chunk, socket kept open (TimeoutError)
min=290.001  max=290.003  spread=0.002
status lines: ['HTTP/1.1 200 OK']
tails: ['b\'":""}}}}\\n\\r\\n0\\r\\n\\r\\n\'', 'b\'as":0}}}\\n\\r\\n0\\r\\n\\r\\n\'', 'b\'dy":0}}}\\n\\r\\n0\\r\\n\\r\\n\'', 'b\'us":{}}}\\n\\r\\n0\\r\\n\\r\\n\'']
one response header block: HTTP/1.1 200 OK
Cache-Control: no-cache, private
Content-Type: application/json
Date: Sun, 27 Sep 2026 14:01:19 GMT
Transfer-Encoding: chunked
bookmarks deployments#2: n=5 at [60.28, 120.65, 181.05, 241.64, 288.4]
bookmarks statefulsets#2: n=5 at [60.9, 120.77, 180.92, 240.91, 288.66]
bookmarks pods#2: n=5 at [60.02, 119.86, 180.23, 240.23, 288.45]
bookmarks deployments#1: n=5 at [60.29, 120.65, 181.05, 241.64, 288.41]
bookmarks daemonsets#1: n=5 at [60.56, 120.44, 181.14, 241.15, 289.01]
bookmarks pods#1: n=5 at [60.01, 119.85, 180.22, 240.22, 288.45]
bookmarks nodes#1: n=6 at [59.99, 120.35, 180.24, 240.53, 287.85, 289.05]
bookmarks nodes#2: n=6 at [59.99, 120.35, 180.24, 240.53, 287.85, 289.06]
bookmarks statefulsets#1: n=5 at [60.91, 120.77, 180.92, 240.91, 288.66]
bookmarks daemonsets#2: n=5 at [60.56, 120.44, 181.14, 241.15, 289.01]
other nodes#1: n=2 first=[(158.02, 'MODIFIED'), (200.92, 'MODIFIED')] last=[(158.02, 'MODIFIED'), (200.92, 'MODIFIED')]
other nodes#2: n=2 first=[(158.03, 'MODIFIED'), (200.92, 'MODIFIED')] last=[(158.03, 'MODIFIED'), (200.92, 'MODIFIED')]
```

Three header lines are omitted from the paste: `Audit-Id` and the two
`X-Kubernetes-Pf-*-Uid` values, neither load-bearing.

## Round B — churning cluster, 10 concurrent watches

`./scripts/cluster.sh break` applied first (crashloop and OOM pods restarting),
plus a scratch loop scaling a healthy Deployment 1↔3 every 6 s for the whole
round, so pods, ReplicaSets and Deployment status all produced events.

```
$ ssh ubuntu '... nohup /tmp/churn.sh & ... timeout 400 python3 /tmp/watchclose.py kind-review "round B ..." 2'
 14:07:58 up 23:17,  1 user,  load average: 1.61, 0.78, 0.43
EXIT=0
=== round B - churning cluster (break applied + scale loop), 2 per kind: 10 watches, timeoutSeconds=290, TLS 127.0.0.1:6444 ===
watch               open@  elapsed   ttfb  reads  maxgap      @  end
nodes#1              0.03  290.001  0.001     10   60.18   60.2  last-chunk, socket kept open (TimeoutError)
statefulsets#2       0.03  290.001  0.003      7   60.69   60.7  last-chunk, socket kept open (TimeoutError)
pods#2               0.03  290.002  0.002    521    6.11  270.1  last-chunk, socket kept open (TimeoutError)
daemonsets#2         0.04  290.003  0.002      7   60.69   60.7  last-chunk, socket kept open (TimeoutError)
daemonsets#1         0.04  290.003  0.003      7   60.68   60.7  last-chunk, socket kept open (TimeoutError)
pods#1               0.02  290.004  0.004    521    6.11  270.1  last-chunk, socket kept open (TimeoutError)
deployments#1        0.03  290.004  0.005    400    6.11   44.3  last-chunk, socket kept open (TimeoutError)
statefulsets#1       0.02  290.005  0.004      7   60.71   60.7  last-chunk, socket kept open (TimeoutError)
nodes#2              0.02  290.005  0.004     10   60.19   60.2  last-chunk, socket kept open (TimeoutError)
deployments#2        0.02  290.006  0.007    400    6.11   44.3  last-chunk, socket kept open (TimeoutError)
min=290.001  max=290.006  spread=0.005
status lines: ['HTTP/1.1 200 OK']
bookmarks nodes#1: n=5 at [60.19, 120.19, 180.21, 239.74, 288.28]
bookmarks pods#1: n=6 at [59.65, 120.06, 179.74, 240.42, 287.51, 288.67]
bookmarks deployments#1: n=5 at [60.59, 120.52, 180.53, 240.09, 288.84]
bookmarks statefulsets#1: n=5 at [60.71, 120.0, 180.16, 240.69, 288.55]
bookmarks daemonsets#1: n=5 at [60.69, 119.97, 180.57, 240.5, 288.15]
other pods#1: n=349 first=[(0.0, 'MODIFIED'), (0.74, 'MODIFIED'), (1.03, 'MODIFIED'), (1.12, 'MODIFIED')] last=[(288.42, 'DELETED'), (288.43, 'DELETED')]
other deployments#1: n=199 first=[(1.55, 'MODIFIED'), (1.57, 'MODIFIED'), (1.58, 'MODIFIED'), (1.61, 'MODIFIED')] last=[(288.41, 'MODIFIED'), (288.45, 'MODIFIED')]
other nodes#1: n=1 first=[(107.29, 'MODIFIED')] last=[(107.29, 'MODIFIED')]
```

The `#2` copies' bookmark and event lines are dropped from this paste only — they
match their `#1` twin to within 20 ms on every entry, as the full rounds A and C
tables show.

## Round C — churn plus CPU contention

Same churn, plus 8 busy loops on 4 cores (`for i in $(seq $(( $(nproc) * 2 )));
do yes > /dev/null & done`), to see how late the close can be pushed when the
API server has to compete for the core it writes the last chunk on.

```
$ ssh ubuntu '... nohup /tmp/churn.sh & nohup /tmp/load.sh & sleep 20; timeout 400 python3 /tmp/watchclose.py kind-review "round C ..." 2'
 14:14:30 up 23:23,  1 user,  load average: 6.53, 3.47, 1.79      (before)
 14:19:27 up 23:28,  1 user,  load average: 12.19, 8.61, 4.48     (after)
EXIT=0
=== round C - churn + 8 busy loops on 4 cores: 10 watches, timeoutSeconds=290, TLS 127.0.0.1:6444 ===
watch               open@  elapsed   ttfb  reads  maxgap      @  end
deployments#2        0.07  290.002  0.002    379    6.25   41.2  last-chunk, socket kept open (TimeoutError)
pods#2               0.07  290.002  0.004    267    6.34   41.2  last-chunk, socket kept open (TimeoutError)
statefulsets#2       0.08  290.002  0.004      7   60.67   60.7  last-chunk, socket kept open (TimeoutError)
daemonsets#2         0.07  290.003  0.002      7   60.42  180.0  last-chunk, socket kept open (TimeoutError)
statefulsets#1       0.07  290.004  0.006      7   60.68   60.7  last-chunk, socket kept open (TimeoutError)
daemonsets#1         0.07  290.004  0.006      7   60.42  180.0  last-chunk, socket kept open (TimeoutError)
nodes#2              0.07  290.004  0.003     13   60.34  120.0  last-chunk, socket kept open (TimeoutError)
nodes#1              0.04  290.009  0.008     13   60.34  120.0  last-chunk, socket kept open (TimeoutError)
deployments#1        0.04  290.011  0.018    379    6.25   41.2  last-chunk, socket kept open (TimeoutError)
pods#1               0.03  290.017  0.017    267    6.33   41.3  last-chunk, socket kept open (TimeoutError)
min=290.002  max=290.017  spread=0.015
status lines: ['HTTP/1.1 200 OK']
bookmarks pods#1: n=6 at [60.27, 119.84, 179.7, 239.78, 287.31, 288.38]
bookmarks nodes#1: n=6 at [59.7, 120.04, 180.17, 239.29, 287.55, 288.77]
bookmarks deployments#1: n=6 at [60.41, 120.18, 179.61, 240.4, 287.94, 289.05]
bookmarks statefulsets#1: n=5 at [60.69, 120.84, 180.61, 240.88, 288.75]
bookmarks daemonsets#1: n=5 at [60.16, 119.61, 180.03, 240.44, 288.1]
other pods#1: n=221 first=[(2.95, 'MODIFIED'), (3.66, 'ADDED'), (3.68, 'MODIFIED'), (3.69, 'ADDED')] last=[(286.52, 'DELETED'), (286.52, 'DELETED')]
other deployments#1: n=186 first=[(3.63, 'MODIFIED'), (3.66, 'MODIFIED'), (3.68, 'MODIFIED'), (3.72, 'MODIFIED')] last=[(286.54, 'MODIFIED'), (286.57, 'MODIFIED')]
other nodes#1: n=2 first=[(19.71, 'MODIFIED'), (262.36, 'MODIFIED')] last=[(19.71, 'MODIFIED'), (262.36, 'MODIFIED')]
```

A late reading here is an upper bound on the *server's* lateness, not a
separation of it: under this load the probe's own Python timestamps are subject
to the same contention.

---

## The 30 samples, by round

| round | state | n | min | max | spread | lateness over 290 (max) |
|---|---|---|---|---|---|---|
| A | idle | 10 | 290.001 | 290.003 | 0.002 | **6 ms** |
| B | churn (`break` + scale loop), load ~1.6 | 10 | 290.001 | 290.006 | 0.005 | **6 ms** |
| C | churn + 8 busy loops on 4 cores, load 6.5→12.2 | 10 | 290.002 | 290.017 | 0.015 | **17 ms** |
| all | | 30 | **290.001** | **290.017** | 0.016 | **17 ms** |

No sample fell below 290.000, and none reached 290.1 — the 290.1 s in the
2026-09-26 report is the relay's 0.1 s resolution, not the server's.

## Largest quiet gap inside a healthy watch, and the bookmarks

| | seconds |
|---|---|
| largest gap between two successive reads, all 30 watches | **60.91** (round A, `statefulsets#1`) |
| largest gap on a kind with traffic (pods/deployments under churn) | 6.11–6.34 |
| gap from the last byte before the close to the last chunk | **1.0–2.7** (last bookmark 287.3–289.06, close at 290.00x) |

Bookmarks **do** arrive on all five kinds, on every one of the 30 watches: 5 or 6
per watch, at ~60 s intervals (59.6–60.9 s between consecutive ones), plus one
final bookmark 1.0–2.7 s before the close. Nine watches of the 30 saw two
bookmarks inside the last 2 s. `allowWatchBookmarks=true` was set on every
request, and the server honoured it on every one.

That cadence is **not something a number may lean on**: kube's own params doc
says clients should not assume bookmarks arrive at any specific interval
(`kube-core-4.2.0/src/params.rs:329`, already quoted in `k8s.rs` § WHAT A THROTTLE
LOOKS LIKE). With bookmarks suppressed, the largest gap on a quiet kind would be
the whole window up to the close.

## How the stream ends

All 30: a clean HTTP/1.1 **last chunk** — `tails` shows `…}\n\r\n0\r\n\r\n` on
every watch — on a connection the server then **keeps open**. The 5 s probe read
after the last chunk timed out on all 30 (`socket kept open (TimeoutError)`);
none returned a FIN, and none raised a reset. No `Connection: close` in the
response headers, `Transfer-Encoding: chunked`, `HTTP/1.1 200 OK`.

## What the window has left — arithmetic over the rows above

kube's bound is `timeoutSeconds` + `WATCH_IDLE_TIMEOUT_MARGIN` = 290 + 5 = 295 s
(`kube-runtime-4.2.0/src/watcher.rs:483`, `:494`), quoted from
[D297](../NOTES.md#d297--the-frozen-file-opens-for-a-wedged-watch-and-the-timeout-has-five-seconds-of-room-to-land-in-2026-09-27),
not re-derived here.

| candidate `R` | clearance over the measured max close (290.017) | clearance under kube's 295 |
|---|---|---|
| 290.5 | 0.483 | 4.5 |
| 291 | 0.983 | 4.0 |
| 292 | 1.983 | 3.0 |
| 292.5 | 2.483 | 2.5 |
| 293 | 2.983 | 2.0 |
| 294 | 3.983 | 1.0 |

## What this run did not measure

- **A remote API server.** Every sample is loopback on the same host, so the
  round trip added to the close is ~0. A kubeconfig pointing at a managed control
  plane over a WAN, or through a load balancer with its own buffering, adds that
  RTT to every row above, and nothing here bounds it.
- **A large cluster.** One control plane, one worker, ~30 pods. The events per
  round (up to 521 reads on the pods watch) are churn, not scale.
- **An apiserver flag other than the default.** `--min-request-timeout` was left
  alone; it only applies when the client sends no `timeoutSeconds`, and every
  request here sent 290.
- **Any k8rs code.** No binary was built and nothing in `src/` was run; the probe
  is a scratch driver in `/tmp` on the host. Whether the chosen `R` behaves
  through `hyper-timeout` in the real client, and what `--follow` does on a
  container quiet past `R`, are the box's own runs.
- **The wedge itself.** Out of scope for this dispatch.

## Teardown

```
$ ssh ubuntu 'cd ~/k8rs-src && K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 ./scripts/cluster.sh down'
$ ssh ubuntu 'kind get clusters; docker ps --format "{{.Names}} {{.Status}}"; pgrep -c yes'
No kind clusters found.
openberat-ad-samba-ad-1 Up 23 hours
0
no yes processes
```

The one container left is the user's own, unrelated, and was running before this
measurement started. One bench note: round C's cleanup `pkill -f "/tmp/churn.sh"`
matched the remote shell that was running it — the same self-match
[D275](../NOTES.md#d275--the-wait-loop-watched-for-the-commands-own-name-so-it-matched-itself-and-never-ended-2026-09-24)
is about — so that round's output was read from its log afterwards and the busy
loops were killed by `pkill -x yes` in the next command. The round itself had
already finished (`EXIT=0` in the log).
