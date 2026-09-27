# A wedged watch on a live cluster: the healthy negative, the rows, the header, and `--follow`

**2026-09-27 · `k8s-admin` · ephemeral measurement ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15))**

Subject: the six things
[D297](../NOTES.md#d297--the-frozen-file-opens-for-a-wedged-watch-and-the-timeout-has-five-seconds-of-room-to-land-in-2026-09-27)
and its *review rounds* section assert about the shipped `read_timeout` of 292 s
and could only be checked by running the binary — the negative first, since a
spurious row on a healthy cluster is the one result that would move the number.
Companion to [reports/2026-09-27-watch-close-timing.md](2026-09-27-watch-close-timing.md),
which measured the server's close with a scratch probe and no k8rs code; this one
measures the real client, the real screen and the real exit codes.

## The bench

| | |
|---|---|
| host | test host (`ssh ubuntu`, [D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)); Ubuntu 24.04.4, 4 cores, 3915 MiB, `load average: 0.18` at start; nothing else of this repo running, one unrelated container of the user's that predates the run |
| binary | **release**, built on the host from the mirrored working tree: commit `89077fe` **plus the uncommitted box** (`src/k8s.rs`, `src/k8s_tests.rs`, `src/ops.rs`, `scripts/read-deadline-guard.py`). `sha256sum target/release/k8rs` = `611b3567df3a23b0d049ca4ed1cdf3c0d1efb6f0d3124f3c6e7c24916d6a8428`; `sha256sum src/k8s.rs` = `90eddff1a866b0ed6cc9b4dcde30c1fc8d348519395a7bc020b2ea24bd7845fd` |
| cluster | `K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 ./scripts/cluster.sh up`, `kindest/node:v1.36.1`, 1 control plane + 1 worker, **idle** (no `break`), torn down before this report |
| screen | the TUI with no flags, on a 200×50 pty |
| drivers | three scratch files in `/tmp` on the host, nothing added to `scripts/` or the `justfile` |

```
$ rsync -a --delete --exclude=/target --exclude='/mutants.out*' ~/GIT/k8rs/ ubuntu:k8rs-src/
$ ssh ubuntu 'cd ~/k8rs-src && touch src/*.rs && export PATH=$HOME/.cargo/bin:$PATH && cargo build --release --locked'
    Finished `release` profile [optimized] target(s) in 5m 33s
EXIT=0
$ ssh ubuntu 'cd ~/k8rs-src && K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 ./scripts/cluster.sh up'
API: https://127.0.0.1:6444   context: kind-review
EXIT=0
```

## The harness

**`/tmp/ptyrun.py`** — runs the binary on a pty of a fixed size and timestamps
every output chunk into a log, then sends `q`. The screen draws on events
(invariant 7), so a chunk boundary *is* an event: 33 chunks in 700 s on a healthy
cluster.

**`/tmp/screen.py`** — reports the first and last chunk containing a string, and
dumps a frame. A cursor-position escape becomes a newline, so one frame
reconstructs roughly row by row; **the word breaks in the dumps below are the
extractor's, not the screen's.**

**One detection lesson, stated because it invalidated a first pass**: `ui::banner`
wraps, so `"not getting"` can straddle a line break and a grep for it misses a
banner that is on the screen. Every negative below is therefore taken on `▲`,
which the banner always emits as its own cell run (proved by items 2 and 3, where
`▲`, `getting`, `asking` and `trusted` all land in the same chunk).

**`/tmp/relay.py`** — a TCP relay that can wedge. `pass` forwards both ways;
`hole` forwards nothing and **closes nothing**, which is the state the box is
about. It logs each connection's accept, its request lines, its downstream gaps
over 30 s, and — the number this round turns on — the moment the client closes a
connection together with the timestamp of the last byte that connection received:

```python
            if wedged:
                r, _, _ = select.select([c], [], [], 0.2)
                if c in r:
                    d = c.recv(65536)
                    if not d:
                        say("conn%-3d client closed while wedged (down=%d last_down=%s)"
                            % (cid, down, "-" if last_down is None else "T+%.3f" % last_down))
                        return
                    held.append(d)
                continue
```

For items 1, 2 and 5 the relay is a plain TCP hop in front of the API server and
TLS runs through it end to end (a kubeconfig copy with the port changed). For
item 3 it sits in front of `kubectl proxy` over plain HTTP, **because a TCP relay
cannot read a request path through TLS** and that item needs one kind wedged and
four healthy. Named as a difference: the deadline under test is on the TCP stream
below TLS in both shapes.

---

## Item 1 — a healthy cluster, 700 s, two close cycles per watch

D297's margin is 1.98 s over the worst measured close. Expected: no fault row.

```
$ ssh ubuntu 'echo pass > /tmp/relay.mode; nohup python3 /tmp/relay.py 7443 6444 /tmp/relay.mode /tmp/relay_item1.log &
              KUBECONFIG=/tmp/kubeconfig-relay.yaml python3 /tmp/ptyrun.py 700 /tmp/item1.log ./target/release/k8rs'
EXIT=0 log=/tmp/item1.log

$ ssh ubuntu 'python3 /tmp/screen.py /tmp/item1.log "not getting" "disconnected, retrying" \
              "has stopped receiving" "never finished reading" "live" "▲" "●"'
chunks=33 span=T+0.050..T+690.201  tail====END T+701.711 ended=None wait_status=0
ABSENT 'not getting'                      never, in 33 chunks
ABSENT 'disconnected, retrying'           never, in 33 chunks
ABSENT 'has stopped receiving'            never, in 33 chunks
ABSENT 'never finished reading'           never, in 33 chunks
FIRST  'live'                             T+0.202      (in 1 chunks, last T+0.202)
ABSENT '▲'                                never, in 33 chunks
ABSENT '●'                                never, in 33 chunks
```

`live` is drawn once at T+0.202 and never redrawn, i.e. the header row never
changed for 700 s. No alarm mark of either shape appeared.

The relay's view of the same run — six connections held open for the whole 700 s,
receiving bytes to the end:

```
T+   1.051 [17:44:11] conn7   accepted (mode=pass)
T+   1.053 [17:44:11] conn8   accepted (mode=pass)
...
T+ 701.213 [17:55:51] conn7   client closed (down=25266 last_down=T+691.099)
T+ 701.214 [17:55:51] conn8   client closed (down=16533 last_down=T+641.394)
T+ 701.215 [17:55:51] conn11  client closed (down=33466 last_down=T+700.918)
T+ 701.215 [17:55:51] conn9   client closed (down=109238 last_down=T+701.110)
T+ 701.215 [17:55:51] conn1   client closed (down=85551 last_down=T+701.122)
T+ 701.216 [17:55:51] conn10  client closed (down=7986 last_down=T+700.644)
```

**No connection was replaced during the run**, which is a fact about the client
rather than about the cluster: the server keeps the TCP connection open after the
watch's last chunk (measured in the companion report), so hyper returns it to the
pool and the next watch generation is written down the same socket. The close
cycles are therefore invisible as reconnects and visible only as bytes; item 2's
healthy phase below shows the same five connections spanning a close at T+291.

Corroboration from the API server's own counters over item 2's healthy phase
(`apiserver_request_total{verb="WATCH"}`, 310 s apart, cluster-wide so other
clients are in it too): `daemonsets 6→8`, `deployments 7→8`, `statefulsets 9→12`,
`nodes 30→36`, `pods 21→29`.

## Item 2 — every watch wedged

Wedge on at wall `18:09:16` = relay T+332. The five watch connections and when
the **client** closed them, against the last byte each had received:

```
T+   1.013 [18:03:45] conn1   accepted (mode=pass)
T+   1.054 [18:03:45] conn8   accepted (mode=pass)
T+   1.055 [18:03:45] conn9   accepted (mode=pass)
T+   1.058 [18:03:45] conn10  accepted (mode=pass)
T+   1.060 [18:03:45] conn11  accepted (mode=pass)
T+ 583.113 [18:13:27] conn8   client closed while wedged (down=6696  last_down=T+291.112)
T+ 583.113 [18:13:27] conn1   client closed while wedged (down=68992 last_down=T+291.111)
T+ 583.115 [18:13:27] conn10  client closed while wedged (down=17422 last_down=T+291.113)
T+ 583.118 [18:13:27] conn11  client closed while wedged (down=97139 last_down=T+291.117)
T+ 590.938 [18:13:35] conn9   client closed while wedged (down=35416 last_down=T+298.936)
```

| connection | last byte | client closed | interval |
|---|---|---|---|
| conn1 | T+291.111 | T+583.113 | **292.002 s** |
| conn8 | T+291.112 | T+583.113 | **292.001 s** |
| conn10 | T+291.113 | T+583.115 | **292.002 s** |
| conn11 | T+291.117 | T+583.118 | **292.001 s** |
| conn9 | T+298.936 | T+590.938 | **292.002 s** |

The anchor is the connection's own last byte and not the wedge: four of the five
last read at T+291.11, **41 s before the wedge went on**, and ended 251 s after
it.

The screen, same run (pty T0 = relay T+1.0):

```
$ ssh ubuntu 'python3 /tmp/screen.py /tmp/item2.log "getting" "▲" "asking" "trusted" "discon" "live"'
chunks=78 span=T+0.052..T+1230.203  tail====END T+1251.725 ended=None wait_status=0
FIRST  'getting'                          T+582.214    (in 1 chunks, last T+582.214)
FIRST  '▲'                                T+582.214    (in 1 chunks, last T+582.214)
FIRST  'asking'                           T+582.214    (in 1 chunks, last T+582.214)
FIRST  'trusted'                          T+582.214    (in 1 chunks, last T+582.214)
FIRST  'discon'                           T+590.039    (in 1 chunks, last T+590.039)
FIRST  'live'                             T+0.206      (in 1 chunks, last T+0.206)
```

| | pty | relay |
|---|---|---|
| first row on the Alerts pane | **T+582.214** | conn1/8/10/11 timeout T+583.11 |
| header leaves `live` | **T+590.039** | conn9 timeout T+590.94 |
| first row → header flip | **7.825 s** | last-byte spread 291.11 → 298.94 = **7.83 s** |

The header frame at the flip (fragmented by the extractor, not by the screen):

```
----- first frame with 'discon', T+590.039 -----
  | ctx: kind-review · ⚠ discon
  | ect
  | d, retry
  | ng
```

D297's *review rounds* item 1 puts that lag at "292 s plus up to about a bookmark
interval… the measured per-kind bookmark offsets run 59.6–60.9 s apart". The
59.6–60.9 s in the companion report is the interval **between consecutive
bookmarks on one watch**, not an offset between kinds. Measured here through the
product's own client, the per-kind gaps are 59.4–60.3 s on every watch and the
kinds are nearly in step:

```
T+  60.510 [18:04:45] conn8   downstream quiet 59.4s (T+1.109 -> T+60.510)
T+  60.691 [18:04:45] conn1   downstream quiet 59.6s (T+1.109 -> T+60.691)
T+  60.896 [18:04:45] conn11  downstream quiet 59.8s (T+1.115 -> T+60.896)
T+  61.096 [18:04:45] conn9   downstream quiet 60.0s (T+1.110 -> T+61.096)
T+  61.117 [18:04:45] conn10  downstream quiet 60.0s (T+1.111 -> T+61.117)
```

Four of the five anchors landed within **6 ms** of each other; the 7.83 s
straggler is conn9, whose timer had been reset by a real event
(`downstream quiet 46.8s (T+74.565 -> T+121.348)`) rather than by a bookmark.

**One `▲` only, with all five kinds wedged.** The pane draws `said.first()`, so
the reader is shown one line naming pods and no count of the other four.

**What refreshes the row while a wedge lasts is not 292 s.** In this wedge the
relay starves the TLS handshake too, so each retry dies in kube's own
`connect_timeout`, and the metrics poll dies in its own 10 s — exactly 30.0 s and
10.0 s, repeatedly:

```
T+ 689.327 [18:15:14] conn38  accepted (mode=hole)
T+ 719.328 [18:15:44] conn38  client closed while wedged (down=0 last_down=-)
T+ 751.101 [18:16:15] conn49  accepted (mode=hole)
T+ 761.102 [18:16:25] conn49  client closed while wedged (down=0 last_down=-)
```

## Item 3 — one watch wedged, four healthy

Plain HTTP behind `kubectl proxy`, the relay wedging only the request that is
both `/api/v1/pods` and `watch=true`:

```
T+   1.065 [18:38:54] conn2   request  GET /apis/apps/v1/daemonsets?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5679 HTTP/1.1
T+   1.066 [18:38:54] conn3   request  GET /apis/apps/v1/statefulsets?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5679 HTTP/1.1
T+   1.068 [18:38:54] conn6   request  GET /apis/apps/v1/deployments?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5679 HTTP/1.1
T+   1.068 [18:38:54] conn5   request  GET /api/v1/nodes?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5679 HTTP/1.1
T+   1.069 [18:38:54] conn4   request  GET /api/v1/pods?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5679 HTTP/1.1
T+   1.069 [18:38:54] conn4   ^^ PATH-HOLED: this stream is wedged from here
```

```
$ ssh ubuntu 'python3 /tmp/screen.py /tmp/item3.log "getting" "▲" "discon" "live" "asking"'
chunks=32 span=T+0.028..T+600.146  tail====END T+621.670 ended=None wait_status=0
FIRST  'getting'                          T+292.154    (in 1 chunks, last T+292.154)
FIRST  '▲'                                T+292.154    (in 1 chunks, last T+292.154)
ABSENT 'discon'                           never, in 32 chunks
FIRST  'live'                             T+0.151      (in 1 chunks, last T+0.151)
FIRST  'asking'                           T+292.154    (in 1 chunks, last T+292.154)
```

Row at **T+292.154**, against the wedged watch's request at relay T+1.069 (pty
T+0.07). `discon` absent over the whole 620 s: the header stayed `live` for 308 s
after the row, across a further close cycle of the four healthy watches. Every
frame after T+292.154 is a 25-byte no-op, so the row neither moved nor gained a
neighbour.

## Item 4 — the sentence, off the pty bytes

The same text in items 2 and 3 (extractor line breaks, screen words):

```
----- first frame with '▲', T+292.154 -----
  | ▲ | k8rs | is | not | getting | pods | from | this | cluster: | nothing | usable | came
  | back | when | k8rs | tried | to | `list` | and | `watch` | pods. | It | keeps | asking,
  | and | until | that | works | nothing | here | about | them | can | be | trusted
```

`Check the server address this kubeconfig names` (`views::next_step`) appears in
neither run: greps for `check the server address` and `Check the server address`
are ABSENT in all 78 and all 32 chunks.

## Item 5 — recovery on an idle cluster

Wedge off at wall `18:17:06` ≈ pty T+801. Every frame of the rest of the run,
with its byte count:

```
T+790.203      25B
T+810.247      25B
T+818.472     109B  | ctx: ki | d-r | view · l | ve
T+840.245      25B
T+870.204      25B
   … 14 more frames, all 25B …
T+1217.689     25B
T+1230.203     69B  ===END T+1251.725 ended=None wait_status=0
```

| | |
|---|---|
| header back to `live` | T+818.472, **17.5 s** after the wedge was lifted |
| banner drawn | T+582.214 |
| banner removed | **never**; the run ended at T+1230.203 with it on screen |
| banner standing after the header said `live` | **412 s**, and still standing at the end |
| `nothing is broken` redrawn | never (`in 1 chunks, last T+0.206`) |

Every frame between T+818.472 and the end is 25 bytes, i.e. no cell changed: the
screen carried `ctx: kind-review · live` and
`▲ k8rs is not getting pods from this cluster … It keeps asking` at the same time
for the remaining 412 s. The header's return needs one kind with no row
(`main.rs`'s `linked`), so at least one kind cleared within 17.5 s while pods did
not; which kinds cleared is not visible on a pane that draws one line.

## Item 6 — `--follow` on a container that writes nothing

```
$ ssh ubuntu 'kubectl run quiet --image=busybox:1.36 --restart=Never -- sh -c "sleep 3600"'
pod/quiet created
pod/quiet condition met
$ ssh ubuntu 'kubectl logs quiet'          # no bytes, exit 0
$ ssh ubuntu './target/release/k8rs --logs --object default/quiet --follow'
```

```
18:50:25 starting: k8rs --logs --object default/quiet --follow
18:55:17 k8rs exited code=2 after 292s
=== stdout (bytes: 0) ===
=== stderr ===
$ kubectl logs quiet -n default -c quiet -f
k8rs: the log stopped arriving before it ended, so what is above is not all of it — ServiceError: error reading a body from connection
```

Exit **2** after **292 s**, stdout empty, and the clause after the em dash is
`ServiceError: error reading a body from connection` — kube's and hyper's own
Display text. The words *timed out* do not appear, and
`k8rs: nothing has been written to this container's log yet`
(`main.rs`'s `nothing_written`) does not print, the error path returning first.

## Incidental, and settled by these runs rather than reasoned

- **`timeoutSeconds=290` and `allowWatchBookmarks=true` are on k8rs's own watch
  requests**, seen on the wire in item 3, though `watcher::Config::timeout` is
  left unset — `kube-core`'s `unwrap_or(290)` puts it there. Same for
  `limit=500` on the initial LISTs (`GET /api/v1/nodes?&limit=500`).
- **One connection serves several kinds over its life.** In item 3 the connection
  whose first request was `/apis/policy/v1/poddisruptionbudgets?` later carried a
  watch, and another carried `/api/v1/pods?&limit=1`, `/version` twice, `/apis`,
  `/api`, `/api/v1/services?` and then the daemonsets watch.
- **The TUI polls node usage every 30.0 s with no `--analysis`** — one connection
  in every run showed `downstream quiet 30.0s` on that period, and under the
  wedge that poll's connection died at exactly 10.0 s.
- **A healthy watch's longest quiet stretch through the real client is 60.3 s**,
  so the 1.98 s margin over the server's close is the margin only on a server
  that sends no bookmarks.

## What this run did not measure

- **A remote API server.** Everything is loopback plus one local relay hop, so
  the round trip added to the server's close is still ~0. The WAN case D297 names
  as unmeasured stays unmeasured.
- **A large cluster.** One control plane, one worker, 11 pods, idle.
- **Which kinds cleared their row after the un-wedge**, and by which of the two
  clearing events — the pane draws one line and the store is not visible from a
  pty.
- **A wedge that lets new connections handshake.** This relay starves TLS too, so
  the retries died in `connect_timeout`; a NAT-drop that only kills established
  flows would refresh the row on a different period.
- **The first attempt at item 3 was invalid and was re-run**: the relay parsed
  only each connection's first request line, and since hyper reuses connections
  the pods watch was never matched (`PATH-HOLED` absent, no row, `getting`
  absent). Fixed to scan every request line, then re-run; the invalid run's
  620 s produced no row and is not evidence of anything.

## Teardown

```
$ ssh ubuntu 'cd ~/k8rs-src && K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 ./scripts/cluster.sh down'
Deleting cluster "review" ...
Deleted nodes: [the two review nodes]
$ ssh ubuntu 'kind get clusters; pgrep -a -x python3 || echo "no python3"; pgrep -a -x kubectl || echo "no kubectl"'
No kind clusters found.
no python3
no kubectl
```

The one container left running on the host is the user's own, unrelated, and was
up before this measurement started. Node names are left out of this file by
[reports/README.md](README.md) § The sanitization rule; the deleted-nodes line
above is the only place they occurred and is described rather than quoted.
