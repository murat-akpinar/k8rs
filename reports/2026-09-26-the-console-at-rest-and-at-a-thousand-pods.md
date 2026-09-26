# What the wired console costs at rest, and its resident set at 1 011 pods (2026-09-26)

**`k8s-admin` · ephemeral measurement ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15))**

Subject: `todo.md` § Phase 12's last box — *Idle CPU measured at 0%; memory
measured at ~1000 pods*. The subject under test is the **wired console on a
pty**, never `--live`: the two existing figures are the temporary driver's
([D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)
measured 58 752 KiB at 1 011 pods,
[D204](../NOTES.md#d204--the-resident-set-named-by-an-instrument-the-store-is-cheaper-than-the-wire-and-the-memory-is-in-a-page-of-500-whole-pods-2026-09-03)
profiled it), and
[`reports/2026-09-26-the-error-state-pass.md`](2026-09-26-the-error-state-pass.md) § 2's
`0 tick(s)` is 2 s against a **disconnected** console with no watch traffic at
all.

Two readings are kept separate throughout, per the brief's ruling 1:
**(a)** what the process spends over windows that contain no wake, and
**(b)** the cadence and the cost of every wake that does happen.

## The bench

| | |
|---|---|
| host | the test host ([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)) — 4 cores, `MemTotal` 4 009 144 kB, `SwapTotal` 2 097 148 kB, kernel 6.8.0, `CLK_TCK=100` |
| build | `cargo build --release --locked` at `ea88356` after `touch src/*.rs`, `Finished release profile in 5m 14s`, `EXIT=0` |
| cluster | `K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 bash scripts/cluster.sh up`, server `v1.36.1`, 1 control plane + 1 worker, API on `https://127.0.0.1:6444`; torn down before this file was written |
| terminal | 24 × 100 pty |
| also on the host | one unrelated long-running container that is not ours (up 21 h), left alone; `kind get clusters` → `No kind clusters found.` before `up` and after `down` |
| not on the host | nothing else of ours — no fixture cluster, no build, no sweep |

`/usr/bin/time` is still not the instrument here; everything below comes out of
`/proc`, the same place [D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)
read `VmRSS` and `VmHWM` from.

## How each number was taken

One throwaway harness, `console-probe.py` (verbatim at the end of this file), run
from `/tmp` on the host. It forks a pty the way `scripts/suspend-test.py` and
`scripts/picker-test.py` do, `exec`s the release binary with `KUBECONFIG` and
`--context kind-review`, and every 0.5 s writes down:

| instrument | read from | what it answers |
|---|---|---|
| `ticks` | `utime + stime` in `/proc/<pid>/stat` | CPU at the resolution `top` has — 10 ms |
| `ns` | sum of `/proc/<pid>/task/*/schedstat` field 1 | CPU below one tick, across every thread |
| `vol` / `inv` | `voluntary` / `nonvoluntary_ctxt_switches`, summed over the same tasks | how many times a thread blocked, vs was preempted |
| `bytes` | what arrived on the pty in that interval | **a frame was painted** — nothing else writes there in a console (under `--live` it is the printed report instead) |
| socket set | `/proc/<pid>/fd` symlinks, peer port from `/proc/net/tcp*` | when a connection is opened |
| per-socket bytes | `ss -tnieH state established '( dport = :6444 )'`, matched to the child's fds by `ino:` | **which** connection carried a wake, and in which direction |
| `VmRSS` / `VmHWM` | `/proc/<pid>/status` | resident set, and its high-water mark |
| `MemAvailable`, swap used | `/proc/meminfo` at the same instant | the host's own state at every read time |

Two properties of the method that the numbers depend on:

- **Every counter is cumulative**, so a wake shorter than the sampling interval
  is never missed — only its *timestamp* is quantised to 0.5 s. That is where the
  29.87 s / 30.40 s jitter in the gap column below comes from.
- **The pty is drained continuously.** A full pty buffer blocks the writer, and a
  console blocked in `write()` would read as a console that is idle.

A *wake* below is a run of consecutive intervals with `ns > 0`; the cost quoted
for it is that run's total.

## 1. The console was connected, and this is the screen the readings were taken against

Replayed out of the saved pty stream of the 1 011-pod run — every chunk applied
in order onto a 24 × 100 grid, since ratatui writes only the cells that changed.
Row 1 is a replay artefact: the header was repainted several times and my naive
replay overlays those writes, so it reads as overlapping words. Everything below
row 1 is one frame.

```
knodes 2/2hing — server v1.36.1 · 60 kinds · {Dik8rstionBudget       ctx: kind-review · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS            │                                                                             │
│  RESOURCES         │                                                                             │
│   workloads        │                                                                             │
│   network          │                                                                             │
│   storage          │                                                                             │
│   config           │                                                                             │
│   cluster          │                            ○  nothing is broken                             │
│  ANALYSIS          │                                                                             │
│   capacity         │                   1011 pods and 2 nodes checked, none of                    │
│   certificates     │                   them is in trouble right now.                             │
│   drain safety     │                                                                             │
│   posture          │                                                                             │
│   restarts         │                                                                             │
│   waste            │                                                                             │
│   versions         │                                                                             │
│                    │                                                                             │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context kind-review get statefulsets -A --watch                                      │
│ $ kubectl --context kind-review get daemonsets -A --watch                                        │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                                    │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

`ctx: kind-review · live · admin` — the header word is `live`, not
`⚠ disconnected, retrying`. **The replay accumulates, so it says the *last* state
of each cell; what says the word never changed in between is the byte log** — after
1.04 s the only thing ever written to this pty was the 25-byte escape-only epilogue
of § 4, and a header word changing would have written text.

Free from the same samples, and not this box's subject: at 1 011 pods the pty
received 2 892 bytes by the 0.51 s sample and 470 more by 1.04 s, and then no
content at all until the first periodic frame at 30.37 s. So the loading frame and
the settled frame above were both on screen **within 1.04 s of `exec`** — the
resolution is the 0.5 s sampling interval, which is too coarse to put a figure on
*first* paint.

## 2. Reading (a) and (b) on the bare cluster — 11 pods, 600 s

```
$ for k in pods nodes deployments statefulsets daemonsets; do printf "%s=%s\n" "$k" \
    "$(kubectl --context kind-review get $k -A --no-headers 2>/dev/null | wc -l)"; done
pods=11
nodes=2
deployments=2
statefulsets=0
daemonsets=2
$ python3 /tmp/console-probe.py .../target/release/k8rs ~/.kube/config kind-review 600 0.5 idle11
label=idle11 peak_VmHWM_kB=13388 last_VmRSS_kB=13388
samples=1125 interval=0.5s window=600.2s
total ticks=7 total ns=80425218 total pty bytes=3887
longest quiet run: 54 intervals = 27.0s, ending at 569.2s, 0 ticks and 0 ns in every one
wakes=53
```

**(a) The windows with no wake.** 1 051 of the 1 125 half-second windows
recorded **0 ticks and 0 ns** — no CPU at all, at the resolution below one tick.
The longest unbroken run of those is **27.0 s**, and no quiet window in this run
reaches 30 s — the poll in (b) is what ends every one of them.

**(b) Every wake, and what it was.** The full table as printed, then the
attribution:

```
  at_s    ns      ticks  ptybytes vol  ivals  gap   sockets that moved bytes (sent/recv)
     0.51  34910249     3     3312   405     1          7091087:2386/53138 7091791:1609/5158 7091792:1726/3865 7091793:1832/106737 7091794:1706/26141 7091795:1736/21719
    28.78     23146     0        0     1     1   28.27  
    30.37   1309118     0       25    16     1    1.59  7091793:96/423
    57.57     34671     0        0     1     1   27.20  
    60.24   2097953     0       25    32     2    2.67  7091087:0/398 7091791:0/258 7091792:0/317 7091793:96/423 7091794:0/167 7091795:0/406
    86.37     27855     0        0     1     1   26.13  
    90.11   1392603     0       25    17     2    3.74  7091793:96/423
   105.04   1210199     1       25    11     1   14.93  7091087:0/5410
   117.84   1195461     0       25     9     1   12.80  7091087:0/5901
   118.91   2353188     0       25    37     4    1.07  7091087:0/398 7091791:0/258 7091792:0/317 7091793:96/423 7091794:0/167 7091795:0/406
   147.71     23562     0        0     1     1   28.80  
   150.38   1330900     0       25    14     1    2.67  7091793:96/423
   176.52     22913     0        0     1     1   26.14  
   180.25   2244070     0       25    37     1    3.73  7091087:0/398 7091791:0/258 7091792:0/317 7091793:96/423 7091794:0/167 7091795:0/406
   209.07     50512     0        0     1     1   28.82  
   210.13   1380764     0       25    15     2    1.06  7091793:96/423
   237.89     59646     0        0     1     1   27.76  
   240.03   1998460     0       25    31     2    2.14  7091087:0/398 7091791:0/258 7091792:0/317 7091793:96/423 7091795:0/406
   241.63    172050     0        0     4     1    1.60  7091794:0/167
   262.44     34417     0        0     1     1   20.81  
   266.71     34123     0        0     1     1    4.27  
   270.44   1328480     0       25    14     1    3.73  7091793:96/423
   288.06   1614120     1        0    32     3   17.62  7091087:0/398 7091791:0/516 7091792:0/634 7091794:0/167 7091795:0/812
   290.19   1551804     0        0    41     1    2.13  7091087:162/390 7091791:150/390 7091792:164/390 7091794:163/390 7091795:151/390
   299.26     55030     0        0     1     1    9.07  
   300.33   1308181     0       25    16     1    1.07  7091793:96/423
   328.08     21542     0        0     1     1   27.75  
   330.21   1331465     0       25    16     1    2.13  7091793:96/423
   349.43   1016104     0        0    20     3   19.22  7091087:0/406 7091791:0/167 7091792:0/317 7091794:0/258 7091795:0/398
   356.89     33361     0        0     1     1    7.46  
   360.10   1382708     0       25    16     2    3.21  7091793:96/423
   389.44     33542     0        0     1     1   29.34  
   390.50   1353452     0       25    15     1    1.06  7091793:96/423
   410.24   2077375     0       25    31     1   19.74  7091087:0/406 7091791:0/167 7091792:0/317 7091794:0/258 7091795:0/5808
   418.24     32205     0        0     1     1    8.00  
   420.37   1258862     0       25    16     1    2.13  7091793:96/423
   425.18   1182146     1       25     9     1    4.81  7091795:0/5901
   446.52     28977     0        0     1     1   21.34  
   450.26   1440293     0       25    16     1    3.74  7091793:96/423
   469.46    195110     0        0     4     1   19.20  7091791:0/167
   470.52    814913     1        0    16     2    1.06  7091087:0/406 7091792:0/317 7091794:0/258 7091795:0/398
   479.59   1344149     0       25    17     3    9.07  7091793:96/423
   508.42     43477     0        0     1     1   28.83  
   510.55   1283178     0       25    16     1    2.13  7091793:96/423
   524.43     31285     0        0     1     1   13.88  
   529.77   1065913     0        0    20     4    5.34  7091087:0/406 7091791:0/167 7091792:0/317 7091794:0/258 7091795:0/398
   536.70     37787     0        0     1     1    6.93  
   540.44   1297173     0       25    16     1    3.74  7091793:96/423
   569.78   1459945     0       25    18     2   29.34  7091793:96/423
   577.78   1547103     0        0    30     3    8.00  7091087:0/812 7091791:0/334 7091792:0/317 7091794:0/258 7091795:0/796
   580.45   1663741     0        0    53     1    2.67  7091087:162/390 7091791:150/390 7091792:164/390 7091794:163/390 7091795:151/390
   598.06     20992     0        0     1     1   17.61  
   600.19    664945     0        0    16     1    2.13  7091793:96/423
socket opens=1
     0.51 inode 7091087 -> peer port 6444 inode 7091791 -> peer port 6444 inode 7091792 -> peer port 6444 inode 7091793 -> peer port 6444 inode 7091794 -> peer port 6444 inode 7091795 -> peer port 6444
```

Six connections to `127.0.0.1:6444`, all opened in the first interval and **none
opened again in 600 s** — the re-established watches ride the connections they
already had.

### The five wake families

| family | how it is recognised | how often | cost each | frame? |
|---|---|---|---|---|
| **1 · connect + first LIST** | the 0.51 s sample | once | **34.9 ms**, 3 312 pty bytes | yes, the loading frame then the first real one |
| **2 · the metrics poll** | `96` bytes **sent** / `423` received on one connection — the only outbound request an idle console makes | **20 times in 600.2 s, mean gap 29.991 s** | **1.26–1.46 ms** (mean of the 15 unmerged ones ≈ **1.35 ms**) | yes — 25 bytes, see § 4 |
| **3 · the watch re-establishment** | five connections carry ~160 sent / 390 received at once, 2.1 s after five receive-only closes | twice: `288.06`+`290.19` and `577.78`+`580.45`; **290.26 s apart** | 1.6 ms + 1.55 ms | **no** — 0 pty bytes |
| **4 · the server's own watch traffic** | receive-only, same byte sizes recurring (`398/258/317/167/406`) on five or six connections | ~every 60 s | **0.81–1.07 ms** where it lands alone (`349.43`, `470.52`, `529.77`); 2.0–2.4 ms where it shares an interval with a poll | **no** — 0 pty bytes in all three |
| **5 · a micro-wake with no wire bytes and no frame** | 1 voluntary switch, no bytes in either direction | 19 times, on a ~30 s chain, always 1.6–3.7 s *before* a poll | **21–60 µs** | no |

**Four intervals are none of the five families:** `105.04` and `117.84` (5 410
and 5 901 bytes received on the pod-watch connection, 25 pty bytes, 1.2 ms each),
`425.18` (5 901 bytes, the same shape) and the tail of `410.24` (5 808 bytes on one
connection in the same interval as a family-4 round). Those are the **cluster's own
churn** — real watch events on `kube-system` objects on a cluster minutes old, not
a cost of being idle.

**Duty cycle at 11 pods.** 80.43 ms of CPU over 600.2 s, of which 34.91 ms is
connect. **45.51 ms over the remaining 599.7 s = 0.0076%.** By the 10 ms tick
counter the whole run is **7 ticks**.

## 3. What attributes family 2 to the metrics poll — the control

`k8s::METRICS_POLL` is 30 s (`src/k8s.rs:2913`) and the console pushes
`node_usage_poll` into its merge unconditionally (`src/main.rs:8270`), where
`--live` gates it on `--analysis` (`polls_node_usage`, `src/main.rs:3402`). So the
same binary run as `--live` **without** `--analysis` is the same process with that
one stream removed:

```
$ python3 /tmp/console-probe.py .../k8rs ~/.kube/config kind-review 95 0.5 control_live --live
label=control_live peak_VmHWM_kB=12484 last_VmRSS_kB=12484
samples=179 interval=0.5s window=95.4s
total ticks=2 total ns=29687904 total pty bytes=579
longest quiet run: 110 intervals = 55.0s, ending at 59.1s, 0 ticks and 0 ns in every one
wakes=3
  at_s    ns      ticks  ptybytes vol  ivals  gap   sockets that moved bytes (sent/recv)
     0.51  28432568     2      579   337     1          7117547:2423/143897 7119336:1484/13210 7119337:1631/15742 7119338:1634/3405 7119339:1631/12476 7119342:1398/1550
    59.69   1085545     0        0    20     3   59.18  7119336:0/398 7119337:0/406 7119338:0/317 7119339:0/258 7119342:0/167
    90.08    169791     0        0     1     1   30.39  
socket opens=1
     0.51 inode 7117547 -> peer port 6444 ...
```

- The `96/423` fingerprint appears **0 times in 95 s** here, against 20 times in
  600 s in the console — and the console's chain has a mean gap of 29.991 s, a
  period anchored to the client's own connect and not to any server event.
- The 60 s receive-only family (4) is there without the poll, with the same byte
  sizes — it is the server's, not ours.
- The micro-wake family (5) is **also there without the poll** (`90.08`, 170 µs),
  so it is not the poll's timer tick. It is the one thing in this report I could
  not name; see *What was not measured*.
- Sideways, on the driver and not the console: with no poll merged, the longest
  run with no CPU at all is **55.0 s**, ending only when the server's own 60 s
  traffic arrives.

## 4. What the 30 s frame writes to the terminal

Every store update calls `owing.changed()` (`src/main.rs:8955`), so a poll that
answers *the same thing as last time* still owes a frame, and `drawn` rebuilds
the snapshot, runs `analyze` and the seven reports. The saved pty stream says what
reaches the terminal:

```
$ python3 - <<EOF   # 12 lines: split /tmp/pods1k.pty on the `--- <t>s <n>B ---`
                    # markers the harness writes, print every 25-byte payload
                    # and count the distinct ones
chunks: 14
30.37 25 b'\x1b[39m\x1b[49m\x1b[59m\x1b[0m\x1b[?25l'
distinct 25B payloads: 1
12 b'\x1b[39m\x1b[49m\x1b[59m\x1b[0m\x1b[?25l'
```

All 12 of the periodic frames in that run wrote the **same 25 bytes**: reset
foreground, reset background, reset underline, SGR reset, hide cursor. **No cell
changed** — ratatui's diff was empty and only the frame's own epilogue went out.

## 5. Reading (a) and (b) at 1 011 pods — 310 s

Generated with the pod generator from
[`reports/2026-08-28-ten-thousand-pod-resident-set.md`](2026-08-28-ten-thousand-pod-resident-set.md)
§ *The generator, verbatim*, reused and not rewritten, with the two substitutions
named in § *Choices this measurement had to make*:

```
$ kubectl --context kind-review create namespace gen
namespace/gen created
$ kubectl --context kind-review proxy --port 8011 &   # then, in /tmp
$ time python3 /tmp/gen.py pods 0 1000
errors=0
real	0m5.510s
$ time python3 /tmp/gen.py workloads 100 20 10
errors=0
real	0m0.530s
$ for k in pods nodes deployments statefulsets daemonsets; do ... done   # the loop above
pods=1011
nodes=2
deployments=102
statefulsets=20
daemonsets=12
$ kubectl --context kind-review -n gen get pods --no-headers | awk '{print $2" "$3}' | sort | uniq -c
   1000 1/1 Running
```

The same object counts as [D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)'s
reading 4, which is what makes the two comparable at all. The `kubectl proxy` was
killed before any measurement started.

```
$ python3 /tmp/console-probe.py .../k8rs ~/.kube/config kind-review 310 0.5 pods1k
label=pods1k peak_VmHWM_kB=60720 last_VmRSS_kB=60720
samples=582 interval=0.5s window=310.4s
total ticks=61 total ns=624614379 total pty bytes=3662
longest quiet run: 52 intervals = 26.0s, ending at 118.4s, 0 ticks and 0 ns in every one
wakes=26
  at_s    ns      ticks  ptybytes vol  ivals  gap   sockets that moved bytes (sent/recv)
     0.51 455389375    44     3362 12270     2          7125932:2744/8126879 7125946:1739/19507 7125947:1710/17774 7125948:1724/564175 7125949:1732/102611 7125950:1708/62695
    28.77     53698     0        0     1     1   28.26  
    30.37  13527600     2       25    15     1    1.60  7125946:96/423
    57.58     29625     0        0     1     1   27.21  
    59.71  15734949     1       25    36     3    2.13  7125932:0/167 7125946:96/423 7125947:0/398 7125948:0/258 7125949:0/317 7125950:0/406
    86.38     33881     0        0     1     1   26.67  
    90.11  12772804     1       25    18     2    3.73  7125946:96/423
   118.91     65842     0        0     1     1   28.80  
   120.51  13465207     2       25    36     2    1.60  7125932:0/167 7125946:96/423 7125947:0/398 7125948:0/258 7125949:0/317 7125950:0/406
   144.51  14679904     1       25    11     1   24.00  7125947:0/5410
   147.72     33245     0        0     1     1    3.21  
   150.38  14391706     2       25    14     1    2.66  7125946:96/423
   159.99  13150268     1       25    10     1    9.61  7125947:0/5901
   176.52     34346     0        0     1     1   16.53  
   180.26  14225701     1       25    37     3    3.74  7125932:0/167 7125946:96/423 7125947:0/398 7125948:0/258 7125949:0/317 7125950:0/406
   209.07     44322     0        0     1     1   28.81  
   210.14  12908153     2       25    16     2    1.07  7125946:96/423
   237.88     34036     0        0     1     1   27.74  
   240.01  14341113     1       25    35     4    2.13  7125932:0/167 7125946:96/423 7125947:0/398 7125948:0/258 7125949:0/317 7125950:0/406
   262.43     52189     0        0     1     1   22.42  
   266.70     29079     0        0     1     1    4.27  
   270.43  12575321     1       25    15     1    3.73  7125946:96/423
   288.03   1387469     1        0    12     1   17.60  7125947:0/398 7125949:0/317 7125950:0/406
   289.09   3510763     0        0    79     4    1.06  7125932:150/557 7125947:151/788 7125948:163/390 7125949:164/707 7125950:162/796
   299.22     62585     0        0     1     1   10.13  
   300.29  12081198     1       25    16     1    1.07  7125946:96/423
socket opens=1
     0.51 inode 7125932 -> peer port 6444 inode 7125946 -> peer port 6444 ...
```

**(a)** 542 of 582 windows at **0 ticks and 0 ns**; longest unbroken run
**26.0 s**.

**(b)** The same five families, the same cadences — the poll fingerprint 10 times
at a mean gap of **29.991 s**, the watch re-establishment at `288.03`+`289.09`,
the 60 s server traffic, the micro-wakes at 29–66 µs. **What changed is the price
of a wake that owes a frame:**

| wake | at 11 pods | at 1 011 pods |
|---|---|---|
| a metrics poll (unmerged) | 1.26–1.46 ms | **12.08–14.39 ms**, mean ≈ 13.0 ms |
| one real pod watch event | 1.20–1.21 ms | 13.15–14.68 ms |
| watch re-establishment (no frame) | 1.55–1.61 ms | 1.39 + 3.51 ms |
| a micro-wake (no frame) | 21–60 µs | 29–66 µs |
| connect + first LIST | 34.9 ms | **455.4 ms** |

The two costs that grew are the two that draw a frame, and they grew ~10× with a
~92× the pods (1 011 against 11); the two that draw nothing did not grow. **Duty cycle at
1 011 pods: 624.61 ms over 310.4 s, of which 455.39 ms is connect — 169.23 ms
over the remaining 309.9 s = 0.055%.** The metrics poll's own share is
13.0 ms / 30 s = **0.043%**.

`inv` (nonvoluntary context switches) after the first two samples: **30** in the
600 s run, **25** in the 310 s run, **0** in both `--live` controls. Nothing here
is being preempted off a CPU it is spinning on.

## 6. The resident set at 1 011 pods

```
elapsed,ticks,ns,vol,inv,bytes,rss,hwm,socks,memavail,swapused
0.51,27,284915604,7196,327,2892,52836,56080,10,2475964,275780
1.04,17,170473771,5074,639,470,60712,60712,10,2468524,275780
1.57,0,0,0,0,0,60712,60712,10,2467288,275780
2.11,0,0,0,0,0,60712,60712,10,2467288,275780
2.64,0,0,0,0,0,60712,60712,10,2467036,275780
```

```
$ python3 - <<EOF   # over every sample of each run
idle11: samples=1125 MemAvailable min=2694628 max=2794516 kB | swapused min=275524 max=275780 kB | VmRSS min=13356 max=13388 last=13388 | VmHWM max=13388
   VmRSS first reached 13388 at 580.45s; values after that: [13388]
pods1k: samples=582 MemAvailable min=2419752 max=2485996 kB | swapused min=275780 max=275780 kB | VmRSS min=52836 max=60720 last=60720 | VmHWM max=60720
   VmRSS first reached 60720 at 290.16s; values after that: [60720]
control_live: samples=179 MemAvailable min=2713952 max=2779160 kB | swapused min=275524 max=275524 kB | VmRSS min=12484 max=12484 last=12484 | VmHWM max=12484
live1k: samples=122 MemAvailable min=2450192 max=2469996 kB | swapused min=275780 max=275780 kB | VmRSS min=60956 max=60956 last=60956 | VmHWM max=60956
```

| what | pods | `VmRSS` | `VmHWM` | in MB (decimal) |
|---|---|---|---|---|
| **the console** | **1 011** | **60 712 KiB** settled at 1.04 s, **60 720 KiB** from 290.16 s | **60 720 KiB** | **62.2 MB** |
| the console | 11 | 13 356 → 13 388 KiB | 13 388 KiB | 13.7 MB |
| `--live`, same host, same cluster, same binary | 1 011 | 60 956 KiB | 60 956 KiB | 62.4 MB |
| `--live`, same host | 11 | 12 484 KiB | 12 484 KiB | 12.8 MB |

`REQUIREMENTS.md:214` states **`Memory: < 50MB RSS at ~1000 pods`** and already
carries, in the same item, *"Measured 2026-08-28 and not met — 58 752 KiB (57.4
MiB) at 1 011 pods"*. The measured **console** value at 1 011 pods is 60 720 KiB =
59.3 MiB = **62.2 MB**, so it does not meet that figure either, by 12.2 MB. That
is the same miss
[D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)
recorded for the driver at the same object counts (58 752 KiB = 60.2 MB), and
D171's ruling on the budget line is not this box's to reopen — the number is
reported and nothing follows it here.

**The 8 KiB step at 290.16 s** is the first watch re-establishment (§ 2 family 3);
the 11-pod run shows the same shape, 13 356 → 13 388 KiB at 580.45 s, its own
first re-establishment. Nothing else moved `VmRSS` in either run.

**The console is not measurably larger than the driver.** On this host,
`--live` at the same 1 011 pods reads **60 956 KiB**, i.e. **236 KiB above** the
console. Phase 12's `App`/`Screen` state, the command log and ratatui's two
24 × 100 buffers do not show up against run-to-run variation at this resolution.
The `+1 968 KiB` a reader would get by subtracting D171's 58 752 KiB is a
**different machine and a different binary**, not the TUI.

### Host condition at every read time

| when | `MemAvailable` | swap used of 2 097 148 kB | load | note |
|---|---|---|---|---|
| before `up` | 3 427 828 kB | 277 316 kB | 0.00 | no kind clusters |
| after `up`, before any run | 2 838 372 kB | 275 524 kB | 1.28 | 11 pods |
| across the 600 s 11-pod run | 2 694 628 – 2 794 516 kB | 275 524 – 275 780 kB | — | sampled every 0.5 s |
| after generating 1 011 pods | 2 483 088 kB | 275 780 kB | 1.27 | — |
| across the 310 s 1 011-pod run | 2 419 752 – 2 485 996 kB | **275 780 kB, unchanged in all 582 samples** | — | sampled every 0.5 s |
| after `down` | 3 450 248 kB | 274 500 kB | 0.05 | `No kind clusters found.` |

**Swap used did not move during either measured run** — 275 780 kB before, during
and after the 1 011-pod run — and `MemAvailable` never fell below 2.3 GiB. So this
is not a reading taken while the host was pushing pages out, and `VmRSS` is not
under-reporting the working set here (the brief's ruling 5).

## Choices this measurement had to make

1. **The window lengths**: 600 s at 11 pods (20 poll periods and two watch
   re-establishments, so a 290 s cadence can be seen twice) and 310 s at 1 011
   pods (10 poll periods and one re-establishment). The brief set neither.
2. **Where the ~1 000-pod idle reading is taken.** Both readings (a) and (b) are
   taken twice, at 11 and at 1 011 pods, because the per-wake cost turned out to
   be the number that moves with size and the box asks for both halves.
3. **The `--live` runs.** Two of them, and they are in here for two separate
   reasons that are not the box's subject: the 95 s one is the *control* that
   attributes family 2, and the 65 s one at 1 011 pods exists so the console's
   resident set is compared against the driver **on this host with this binary**
   rather than against D171's figure from another machine.
4. **The generator is cited, not re-pasted.** It is committed verbatim in the
   2026-08-28 report; the two substitutions the brief named are `FIX` →
   `/home/murat/k8rs-src/tests/fixtures/healthy.json` on the host and `HOST_IP` →
   a syntactically valid IPv4 address in kind's node network, redacted here as
   that report redacts it. Nothing else in it changed.
5. **`ss -tnie` as the attribution instrument.** The API server's own
   `apiserver_request_total` cannot see the metrics poll at all — three calls to
   `/apis/metrics.k8s.io/v1beta1/nodes` moved no counter:

   ```
   $ kubectl --context kind-review get --raw /apis/metrics.k8s.io/v1beta1/nodes
   Error from server (NotFound): the server could not find the requested resource
   $ # three such calls, /metrics diffed before and after:
   == counters that moved ==
   3 apiserver_request_total{code="200",component="apiserver",dry_run="",group="coordination.k8s.io",resource="leases",scope="resource",subresource="",verb
   ```

   — a request to an unserved API group is answered before the instrumented
   handler, so per-socket byte counters are what is left.
6. **Wake cost is reported as a run of intervals, not as a single sample**, since
   a wake can straddle a 0.5 s boundary (the `ivals` column).

## What was not measured

- **The micro-wake family is unattributed.** 19 of them in 600 s, 21–60 µs each,
  **0.65 ms** of the 45.5 ms total, no wire bytes, no frame. Ruled out: it is not the
  metrics poll's timer (it is present in the `--live` control that has no poll),
  not a request (nothing is sent), not a draw (nothing reaches the pty). What it
  is — the HTTP connection pool's idle reaper, a TLS timer, tokio's own timer
  bookkeeping — cannot be told from outside the process. **The experiment that
  would settle it is a symbol-level profile of the live process** (`perf record`
  on the host, which needs `perf_event_paranoid` lowered), and it was not run.
- **A disconnected console over a long window.** The error-state pass has 2 s;
  this report has nothing between 2 s and forever for the case where the merge is
  empty, which is the one `burnt()`'s own docstring in `scripts/picker-test.py`
  worries about.
- **Anything under churn.** Both readings are a *resting* cluster: 1 000 of the
  1 011 pods are inert by construction (a `schedulerName` nothing answers to, so
  no scheduler and no kubelet touches them). A cluster with 1 000 pods actually
  starting, dying and restarting sends events these runs did not, and the
  coalescing window is what bounds that — untested here.
- **Any size but these two.** No 100-pod point, no 10 000-pod point (out of
  scope); D171 has the four-point curve for the driver.
- **Where the resident set goes.** No allocator instrumentation and no heap
  profile — `VmRSS` and `VmHWM` are the whole instrument. D204 profiled the
  driver.
- **Keys, mutations, the browser, the reports.** Every reading here is a console
  sitting on the Alerts screen with nobody at the keyboard. The cost of a
  keypress, of opening a browser view (which starts a watch) and of a report pane
  is not in this file.
- **The frame budget.** `COALESCE` bounds a storm to ~10 frames a second; nothing
  here produced a storm, so that bound is unexercised.
- **`q` and the exit path.** The harness kills the child after sending `q`; the
  exit code is `reports/2026-09-26-the-error-state-pass.md`'s, not this one's.
- **Whether the sampler perturbed the subject.** The `ss` call spawns a process
  every 0.5 s on a 4-core host. That load is the *host's*, not the child's — the
  child's counters are its own — but the host's idle margin during these runs
  was not measured without the sampler running.
- **The `k8rs` fixture cluster.** Not up, not touched, not measured.

## The harness, verbatim

Throwaway, in `/tmp` on the host, not committed anywhere but here. (`summary`'s
`total ticks=` line carries a dead `if False else` left by an in-place patch during
the trip; it is what ran, so it is what is pasted.)

```python
#!/usr/bin/env python3
"""What a wired k8rs console costs while nothing is happening, and what wakes it.

Throwaway measurement harness (k8s-admin, 2026-09-26). The pty fork, the winsize and the
`drain` loop are `scripts/suspend-test.py` / `scripts/picker-test.py`'s shape; what is new is
sampling, per interval:

  ticks   utime+stime from /proc/<pid>/stat          (CLK_TCK=100, so 10 ms resolution)
  ns      sum of /proc/<pid>/task/*/schedstat[0]     (sub-tick, all threads)
  vol/inv voluntary + nonvoluntary ctxt switches, summed over the same tasks
  bytes   what the console wrote to the pty in this interval == a frame was painted
  socks   the child's socket fds, and the peer port of any that appeared

  console-probe.py <binary> <kubeconfig> <context> <seconds> <interval> <label> [extra args...]
"""
import contextlib
import os
import pty
import select
import signal
import struct
import sys
import re
import subprocess
import termios
import time
import fcntl
from pathlib import Path

ROWS, COLS = 24, 100


def drain(fd, seconds, keep=None):
    """Read the pty for this long and return how many bytes arrived.

    Draining is not optional: a full pty buffer blocks the writer, and a console blocked in
    write() would read as a console that is idle."""
    end = time.monotonic() + seconds
    got = 0
    while True:
        left = end - time.monotonic()
        if left <= 0:
            return got
        ready, _, _ = select.select([fd], [], [], left)
        if not ready:
            continue
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            return got
        if not chunk:
            return got
        got += len(chunk)
        if keep is not None:
            keep.append(chunk)


def ticks(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
    return int(fields[11]) + int(fields[12])


def runtime_ns(pid):
    total = 0
    for task in Path(f"/proc/{pid}/task").iterdir():
        total += int((task / "schedstat").read_text().split()[0])
    return total


def switches(pid):
    vol = inv = 0
    for task in Path(f"/proc/{pid}/task").iterdir():
        for line in (task / "status").read_text().splitlines():
            if line.startswith("voluntary_ctxt_switches"):
                vol += int(line.split()[1])
            elif line.startswith("nonvoluntary_ctxt_switches"):
                inv += int(line.split()[1])
    return vol, inv


def mem(pid):
    out = {}
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        head = line.split(":")[0]
        if head in ("VmRSS", "VmHWM"):
            out[head] = int(line.split()[1])
    return out.get("VmRSS", -1), out.get("VmHWM", -1)


def host_mem():
    want = ("MemAvailable", "SwapTotal", "SwapFree")
    found = {}
    for line in Path("/proc/meminfo").read_text().splitlines():
        head = line.split(":")[0]
        if head in want:
            found[head] = int(line.split()[1])
    return found


def sockets(pid):
    inodes = set()
    where = f"/proc/{pid}/fd"
    try:
        entries = os.listdir(where)
    except OSError:
        return inodes
    for fd in entries:
        try:
            link = os.readlink(f"{where}/{fd}")
        except OSError:
            continue
        if link.startswith("socket:["):
            inodes.add(link[8:-1])
    return inodes


def peers(inodes):
    """Peer port per socket inode, out of the netns tcp tables."""
    found = {}
    for name in ("tcp", "tcp6"):
        try:
            lines = Path(f"/proc/net/{name}").read_text().splitlines()[1:]
        except OSError:
            continue
        for line in lines:
            fields = line.split()
            if len(fields) > 9 and fields[9] in inodes:
                found[fields[9]] = int(fields[2].split(":")[1], 16)
    return found


def wire(port):
    """Cumulative bytes per socket inode, for every established connection to the API port.

    `ss -tnie` is the only place a *per socket* byte counter is readable without root, and the
    inode is what ties it to the child's own fd. This is what attributes a wake to the metrics
    poll rather than to a watch: the poll has a connection of its own."""
    try:
        blob = subprocess.run(["ss", "-tnieH", "state", "established", f"( dport = :{port} )"],
                              capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return {}
    out = {}
    for chunk in blob.split("ino:")[1:]:
        found = re.match(r"(\d+)", chunk)
        if not found:
            continue
        sent = re.search(r"bytes_sent:(\d+)", chunk)
        got = re.search(r"bytes_received:(\d+)", chunk)
        out[found.group(1)] = (int(sent.group(1)) if sent else 0,
                               int(got.group(1)) if got else 0)
    return out


def probe(binary, config, context, seconds, interval, label, extra):
    pid, fd = pty.fork()
    if pid == 0:
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        os.environ["TERM"] = "xterm-256color"
        os.environ["KUBECONFIG"] = config
        args = [binary, "--context", context, *extra]
        os.execv(binary, args)
        os._exit(127)

    rows = []
    started = time.monotonic()
    log = open(f"/tmp/{label}.csv", "w")
    tape = open(f"/tmp/{label}.pty", "wb")
    log.write("elapsed,ticks,ns,vol,inv,bytes,rss,hwm,socks,memavail,swapused,wire\n")
    before = (ticks(pid), runtime_ns(pid), *switches(pid), sockets(pid))
    on_wire = wire(6444)
    peak = 0
    try:
        while time.monotonic() - started < seconds:
            chunks = []
            got = drain(fd, interval, chunks)
            if chunks:
                tape.write(f"\n--- {time.monotonic() - started:.2f}s {got}B ---\n".encode())
                tape.write(b"".join(chunks))
            now = time.monotonic() - started
            try:
                tick, ns, (vol, inv) = ticks(pid), runtime_ns(pid), switches(pid)
                rss, hwm = mem(pid)
                socks = sockets(pid)
            except (OSError, IndexError):
                print(f"child gone at {now:.1f}s", file=sys.stderr)
                break
            hm = host_mem()
            now_wire = wire(6444)
            moved = {}
            for inode, (sent, rcvd) in now_wire.items():
                was = on_wire.get(inode, (0, 0))
                if inode in socks and (sent - was[0] or rcvd - was[1]):
                    moved[inode] = (sent - was[0], rcvd - was[1])
            on_wire = now_wire
            appeared = peers(socks - before[4])
            row = (round(now, 2), tick - before[0], ns - before[1], vol - before[2],
                   inv - before[3], got, rss, hwm, len(socks), hm["MemAvailable"],
                   hm["SwapTotal"] - hm["SwapFree"])
            log.write(",".join(str(v) for v in row) + ',"' +
                      " ".join(f"{i}+{s}/{g}" for i, (s, g) in sorted(moved.items())) + '"' +
                      ("," + ";".join(f"{i}:{p}" for i, p in sorted(appeared.items())) if appeared else "") + "\n")
            rows.append((row, appeared, moved))
            peak = max(peak, hwm)
            before = (tick, ns, vol, inv, socks)
        os.write(fd, b"q")
        time.sleep(0.5)
        drain(fd, 1.0)
    finally:
        log.close()
        tape.close()
        for sig in (signal.SIGCONT, signal.SIGKILL):
            with contextlib.suppress(ProcessLookupError):
                os.kill(pid, sig)
        with contextlib.suppress(ChildProcessError):
            os.waitpid(pid, 0)
        os.close(fd)
    return rows, peak


def summary(rows, interval):
    """Quiet runs and wakes. A wake is a run of consecutive intervals with ns > 0."""
    live = [row[0][2] > 0 for row in rows]
    quiet, longest, at = 0, 0, 0
    for i, busy in enumerate(live):
        if busy:
            quiet = 0
        else:
            quiet += 1
            if quiet > longest:
                longest, at = quiet, i
    wakes, i = [], 0
    while i < len(rows):
        if live[i]:
            j = i
            ns = tick = byte = vol = 0
            carried = {}
            while j < len(rows) and live[j]:
                ns += rows[j][0][2]
                tick += rows[j][0][1]
                byte += rows[j][0][5]
                vol += rows[j][0][3]
                for inode, (s, g) in rows[j][2].items():
                    was = carried.get(inode, (0, 0))
                    carried[inode] = (was[0] + s, was[1] + g)
                j += 1
            wakes.append((rows[i][0][0], ns, tick, byte, vol, j - i, carried))
            i = j
        else:
            i += 1
    print(f"samples={len(rows)} interval={interval}s window={rows[-1][0][0]:.1f}s")
    print(f"total ticks={rows[-1][0][1] if False else sum(r[0][1] for r in rows)} "
          f"total ns={sum(r[0][2] for r in rows)} "
          f"total pty bytes={sum(r[0][5] for r in rows)}")
    print(f"longest quiet run: {longest} intervals = {longest*interval:.1f}s, "
          f"ending at {rows[at][0][0]:.1f}s, 0 ticks and 0 ns in every one")
    print(f"wakes={len(wakes)}")
    print("  at_s    ns      ticks  ptybytes vol  ivals  gap   sockets that moved bytes (sent/recv)")
    last = None
    for at_s, ns, tick, byte, vol, span, carried in wakes:
        gap = "" if last is None else f"{at_s - last:.2f}"
        moved = " ".join(f"{i}:{s}/{g}" for i, (s, g) in sorted(carried.items()))
        print(f"  {at_s:7.2f} {ns:9d} {tick:5d} {byte:8d} {vol:5d} {span:5d} {gap:>7}  {moved}")
        last = at_s
    opened = [(row[0][0], row[1]) for row in rows if row[1]]
    print(f"socket opens={len(opened)}")
    for at_s, appeared in opened:
        print(f"  {at_s:7.2f} {' '.join(f'inode {i} -> peer port {p}' for i, p in sorted(appeared.items()))}")


if __name__ == "__main__":
    binary, config, context, seconds, interval, label, *extra = sys.argv[1:]
    rows, peak = probe(binary, config, context, float(seconds), float(interval), label, extra)
    print(f"label={label} peak_VmHWM_kB={peak} last_VmRSS_kB={rows[-1][0][6]}")
    summary(rows, float(interval))
```

## Teardown

```
$ K8RS_CLUSTER=review bash scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: [<the two node names, redacted>]
$ kind get clusters
No kind clusters found.
$ docker ps --format '{{.Names}} {{.Status}}'
<the one unrelated container that was already there> Up 21 hours
```

No artifact of this cluster was committed: the generated objects are described
above and are not a fixture, and the harness lives in this file.

**The host rebooted at 14:50:53, thirteen minutes after the teardown and fourteen
after the last measurement** — `uptime -s` → `2026-09-26 14:50:53`, and swap used
went from 275 780 kB to 0. Nothing here ran during or after it: every reading above
was taken between 14:06 and 14:36 and the cluster was deleted at 14:37. What the
reboot took with it is `/tmp` on the host, so the per-sample CSVs and the pty tapes
the tables above were derived from **no longer exist** — this file is the only copy,
which is what `reports/` is for
([D108](../NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)).
The reboot was not asked for by anything in this trip and its cause was not
investigated.
