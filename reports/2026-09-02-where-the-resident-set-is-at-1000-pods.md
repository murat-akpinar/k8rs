# Where the resident set at 1 000 pods actually is — a heap profile (2026-09-02)

Ephemeral measurement by `k8s-admin` for the Phase 6 box *Where the 58 752 KiB at
1 000 pods actually is*, the follow-up
[NOTES § D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)
opened when `VmRSS` ran out of things it could say.

Cluster: a throwaway `kind` cluster created and deleted inside this run under
`K8RS_CLUSTER=review`, one control plane and one worker,
`kindest/node:v1.36.1`, on API port 6444 so the PM's fixture cluster `k8rs` was
never touched and stayed up beside it throughout — which is required, and is
also a confounder for one term below (§ *What could not be proven*, last entry).
No artifact of it was committed.

Binary under test: `target/release/k8rs` at `b9765b7`, tree clean.

Host: 12 cores, 23 GiB, Linux 7.1.6, the `k8rs` fixture cluster running
throughout. glibc version read rather than assumed:

```
$ ldd --version | head -1
ldd (GNU libc) 2.44
```

Objects: the generator pasted verbatim in
[reports/2026-08-28-ten-thousand-pod-resident-set.md § The generator](2026-08-28-ten-thousand-pod-resident-set.md#the-generator-verbatim),
reused unchanged except for its one address literal. Same inert pods, same
`replicas: 0` workloads, same namespace.

## The instruments, and what each one can see

Four, none of which needs a line of code changed:

| instrument | what it reports | blind to |
|---|---|---|
| `memusage` (glibc 2.44's own `LD_PRELOAD` heap profiler) | `heap peak` = peak of (bytes requested − bytes freed) | chunk overhead, arena slack, anything not through `malloc` |
| `/proc/<pid>/smaps` | resident bytes per mapping — `[heap]`, each anonymous region, each file | which allocation is in which page |
| `GLIBC_TUNABLES` | a counterfactual: the same program under a different allocator policy | anything the tunable does not reach |
| `taskset` | a counterfactual on thread count, since tokio sizes its pool from available parallelism | same |

`memusage` prints from an `atexit` handler, so a `SIGKILL`ed process prints
nothing: every profiled run is `--once`, never `--live`. That `--once` reaches
the same peak as `--live` is measured below and not assumed.

`ru_maxrss` from `wait4()` was tried first and **discarded as contaminated**:
the launcher's own resident set leaks into the child's figure, which is what the
kernel carrying a pre-`exec` `mm`'s high-water mark into the new one would do.
The leak is measured; the mechanism is the explanation and was not:

```
$ python3 peak.py /bin/true                    # fork + execvp, parent is python3
[peak.py] status=0 maxrss_kB=6020 wall_s=0.003
$ python3 -c "...os.posix_spawn('/bin/true',...)"
posix_spawn /bin/true maxrss_kB= 13800
python own rss_kB= 13928
```

`/bin/true` does not have a 13.8 MB resident set. `VmHWM` in
`/proc/<pid>/status` is the current `mm`'s high-water mark and `exec` resets it,
so it cannot be contaminated that way — it is D171's instrument and it stays the
instrument here.

## `--once` reaches the same peak as `--live`

At the 1 011-pod shape, same cluster, minutes apart:

```
$ ./live.sh r4live 20                       # --live, VmRSS/VmHWM at 10 Hz
label=r4live max_VmRSS_kB=59584 peak_VmHWM_kB=59584 samples=200
--- first samples (elapsed VmRSS VmHWM) ---
.002606204 6120 6120
.106826453 43172 43156
.211976858 59584 59584
.321083423 59584 59584
--- last samples ---
21.338624132 59584 59584
21.445208094 59584 59584
21.553366221 59584 59584

$ for i in 1 2 3; do ./once.sh r4-$i ./target/release/k8rs --once --context kind-review; done
label=r4-1 peak_VmHWM_kB=59288 samples=280 rc=0
label=r4-2 peak_VmHWM_kB=60300 samples=352 rc=0
label=r4-3 peak_VmHWM_kB=59532 samples=358 rc=0
```

D171's reading 4 was **58 752 KiB**; `--live` here is 59 584 and `--once` is
59 288–60 300. The shape reproduces to within 1–3 %, and `--once` is inside the
`--live` spread. Peak is reached at ~0.2 s and does not move for the remaining
21 s, as D171 found.

## The shapes measured

Three, all of them D171's, so the deltas are attributable:

| shape | pods | Deploy / STS / DS | D171's figure |
|---|---|---|---|
| 2 | 111 | 2 / 0 / 2 | 19 492 KiB |
| 3 | 111 | 1 002 / 200 / 32 | 51 216 KiB |
| 4 | 1 011 | 102 / 20 / 12 | **58 752 KiB** |

Object counts verified against the API server before each reading, e.g.:

```
$ for k in pods deployments statefulsets daemonsets; do printf "%s=%s\n" "$k" \
    "$(kubectl --context kind-review get $k -A --no-headers | wc -l)"; done
pods=1011
deployments=102
statefulsets=20
daemonsets=12
$ kubectl --context kind-review -n gen get pods --no-headers | awk '{print $2" "$3}' | sort | uniq -c
   1000 1/1 Running
```

Every run at every shape printed `1011 pods · 2 nodes` (resp. `111 pods`) and
`○ nothing is broken`, so no rule fired and no card was built.

## What `memusage` says

`--once`, three runs at shape 4 and one each at shapes 2 and 3. Bytes as
`memusage` prints them.

```
$ memusage ./target/release/k8rs --once --context kind-review     # shape 4
Memory usage summary: heap total: 128026302, heap peak: 37377658, stack peak: 167232
         total calls   total memory   failed calls
 malloc|     529330      120729798              0
realloc|      30386        7235771              0  (nomove:13133, dec:2737, free:0)
 calloc|        231          60733              0
   free|     529525      124973478
Histogram for block sizes:
    0-15         332028  59% ==================================================
```

repeat runs: `heap peak: 37381882` and `heap peak: 37386106` — a 0.02 % spread.

```
$ memusage ./target/release/k8rs --once --context kind-review     # shape 3
Memory usage summary: heap total: 101048152, heap peak: 25504668, stack peak: 167232
         total calls   total memory   failed calls
 malloc|     395676       96299372              0
realloc|      16811        4688047              0  (nomove:7484, dec:318, free:0)
 calloc|        231          60733              0
   free|     395871      100645952

$ memusage ./target/release/k8rs --once --context kind-review     # shape 2
Memory usage summary: heap total: 14611786, heap peak: 7907180, stack peak: 160128
         total calls   total memory   failed calls
 malloc|      61449       13781390              0
realloc|       4039         769695              0  (nomove:1576, dec:245, free:0)
 calloc|        229          60701              0
   free|      61642       14508594
```

**At shape 4 the peak live heap is 37 377 658 bytes against a 59 288–61 812 KiB
peak resident set** — 37.4 MB against 60.7–63.3 MB, or ~60 %.

## What `smaps` says the other ~40 % is

`--live` at shape 4, sampled at rest (5 s in, `VmRSS` flat since 0.2 s):

```
$ grep -E '^Vm(RSS|HWM|Data|Stk|Exe|Lib|Size):' /proc/$PID/status
VmSize:	  801540 kB
VmHWM:	   60844 kB
VmRSS:	   60844 kB
VmData:	   76128 kB
VmStk:	     172 kB
VmExe:	    8436 kB
VmLib:	    2696 kB
$ ls /proc/$PID/task | wc -l
13
$ awk '/^[0-9a-f]+-[0-9a-f]+ /{name=$6; if(name=="")name="<anon>"; cur=name; next}
       /^Rss:/{rss[cur]+=$2; tot+=$2}
       END{for(n in rss) if(rss[n]>0) printf "%8d  %s\n", rss[n], n; printf "%8d  TOTAL\n", tot}' smaps.txt | sort -rn
   60844  TOTAL
   40560  [heap]
    9996  <anon>
    7680  /home/shyuuhei/GIT/k8rs/target/release/k8rs
    1544  /usr/lib/libc.so.6
     336  /usr/lib/libm.so.6
     272  /usr/lib/ld-linux-x86-64.so.2
     172  [stack]
     164  /etc/ld.so.cache
     104  /usr/lib/libgcc_s.so.1
       8  [vvar]
       8  [vdso]
```

So **10 100 KiB — one sixth of the resident set — is not heap at all**: the
binary's own text and data, the four shared libraries and the loader cache. That
term is independent of pod count.

The anonymous 9 996 KiB, broken down by mapping (size KiB, Rss KiB):

```
     4976      4972
     2208      2208
     2180      2180
      224       220
      176       172
     6164        60
     4112        40
      ...
$ awk '... group anon mappings by Size ...'
size=65404 KiB count=6 rss_total=0
size=65360 KiB count=1 rss_total=0
size=65312 KiB count=1 rss_total=0
size=63356 KiB count=1 rss_total=0
size=63328 KiB count=1 rss_total=0
size=60560 KiB count=1 rss_total=0
size=2060  KiB count=6 rss_total=60
```

Eleven arena reservations with **zero** resident pages — glibc's secondary
arenas, `PROT_NONE` and untouched, which is where `VmSize: 801540 kB` comes from
and why virtual size says nothing here. Three of the eleven are the unused *tail*
of an arena that is being written to, and the arithmetic says which:
65 536 − 4 976 = 60 560, 65 536 − 2 208 = 63 328, 65 536 − 2 180 = 63 356. So
three arenas hold 9 360 KiB between them and eight hold nothing. The six 2 060 KiB
regions hold 60 KiB between them — ~10 KiB each, which is stack-shaped — but
`ls /proc/$PID/task` above counts **13** threads, so those six are not all of the
process's stacks and nothing here says which six they are.

## The counterfactuals — which of those bytes glibc would give back

`--live` at shape 4, 12 s, `VmHWM` and the last `VmRSS` sample, `[heap]` resident
read from `smaps` at the end of the window:

| `GLIBC_TUNABLES` | peak `VmHWM` | last `VmRSS` | `[heap]` |
|---|---|---|---|
| *(default)* | 61 812 | 61 812 | 40 552 |
| *(default)*, repeat | 59 316 | 56 764 | 36 908 |
| *(default)*, repeat | 59 940 | 57 300 | 36 900 |
| `arena_max=1` | 51 524 | 51 524 | 40 664 |
| `arena_max=2` | 50 576 | 47 672 | 37 004 |
| `arena_max=4` | 57 348 | 54 712 | 36 924 |
| `arena_max=1` + `trim_threshold=131072` | 50 444 | 45 664 | 34 868 |
| `mmap_threshold=16384` | 55 680 | 47 264 | 34 648 |
| `arena_max=1` + `mmap_threshold=16384` + `trim_threshold=131072` | 50 444 | 45 312 | 34 732 |
| the same, repeat | 50 416 | 45 584 | 34 752 |

Two things move, and each is a separate mechanism:

- **`arena_max` ≤ 2 removes 7 800–10 300 KiB of peak** (the default's three runs
  span 59 316–61 812; `arena_max=1` gives 51 524) **and `[heap]` does not grow to
  absorb it** — 40 552 under the default against 40 664 under `arena_max=1`.
  Bytes that vanish without reappearing in the main arena were slack in the
  secondary arenas, not live data. **Measured under co-tenancy** — see the last
  entry of § *What could not be proven*.
- **Naming `mmap_threshold` or `trim_threshold` at all** makes the run give
  5 100–8 400 KiB back between peak and rest (`arena_max=1` alone: 51 524 peak,
  51 524 rest; the same plus `trim_threshold=131072`: 50 444 peak, 45 664 rest).
  131072 *is* glibc's default trim threshold, so the value changed nothing — what
  changed is that setting either tunable turns off glibc's dynamic raising of
  both thresholds, which is the documented mechanism for a delta of this shape.
  Under the default, that same memory is freed by the program and never returned
  to the kernel.

`taskset`, same shape, to test whether the arena slack is simply one arena per
tokio worker:

```
label=c-all cpus=all threads=13 peak_VmHWM_kB=60260 last_VmRSS_kB=57560 heap_region_kB=36896
label=c-4   cpus=0-3 threads=5 peak_VmHWM_kB=58732 last_VmRSS_kB=55620 heap_region_kB=36888
label=c-2   cpus=0-1 threads=3 peak_VmHWM_kB=58100 last_VmRSS_kB=54776 heap_region_kB=36876
label=c-1   cpus=0   threads=2 peak_VmHWM_kB=58304 last_VmRSS_kB=54740 heap_region_kB=36880
```

Thirteen threads down to two costs **~2 000 KiB**, not the ~10 000 KiB above. The
slack is not one arena per thread, and fewer tokio workers would not recover it.

## Arithmetic on the table, and nothing more than arithmetic

Peak live heap (`memusage`, requested bytes), three shapes, two unknowns:

- shape 3 − shape 2, workloads alone changing: (25 504 668 − 7 907 180) / 1 230 =
  **14 307 B per workload object**
- shape 4 − shape 2 is 37 381 882 − 7 907 180 = 29 474 702, of which 130 workload
  objects at that rate are 1 859 910: (29 474 702 − 1 859 910) / 900 =
  **30 683 B per pod**
- residue with 111 pods and 4 workloads removed: **4.44 MB** that is neither

Steady `VmRSS` under `arena_max=1` + `mmap_threshold=16384` +
`trim_threshold=131072`, where the allocator returns what the program frees —
shape 2 = 19 048, shape 3 = 36 092, shape 4 = 45 312 and 45 584 (mean 45 448):

- (36 092 − 19 048) / 1 230 = **13.86 KiB per workload object**
- (45 448 − 19 048) = 26 400, minus 130 × 13.86 = 1 802:
  24 598 / 900 = **27.33 KiB per pod**

The two arms differ by 1 % on workload objects (13.97 KiB against 13.86 KiB) and
10 % on pods (29.96 KiB against 27.33 KiB) — and **the pod gap is a bias with a
known sign, not agreement**. The peak arm carries kube's page buffer, which is
capped at 500: at 111 pods it holds 111 objects and at 1 011 it holds 500, so it
charges 389 extra decoded objects to the 900 pods between them, a term the
resting arm never sees. The gap it leaves, 30 683 − 27 986 = 2 697 B per pod,
implies a decoded-`Pod` term of ~6.2 KB; measured directly on the captures that
object is 17 956 B (below). So the two arms are **not independent
corroboration**, and the peak arm is the one that over-states.

### What the slope is a slope of — and what it is not

**These are slopes of the whole process's retention against pod count, not the
cost of one stored object.** This file's first draft read them as the store, and
that is further than the evidence goes: nothing measured here opens a
`PodSnapshot`. What the slopes support is that shape 4's 37.4 MB peak live heap
splits **31.0 MB with pod count, 1.9 MB with workload count, 4.4 MB with
neither** — where the bytes *scale*, and nothing about which allocation holds
them. Three candidates fit that shape and this run separates none of them:

- **kube's page buffer** — `INITIAL_LIST_PAGE` is 500 and kube decodes a whole
  page of full `k8s_openapi` `Pod`s before it emits the first event.
- **The store's two copies** — the pruned `PodSnapshot`, plus the deep copy
  `Store::snapshot` publishes at exactly the instant `VmHWM` is read.
- **Allocator retention** over both, which the counterfactuals above bound at
  8–10 MB in total but do not attribute per pod.

It has since been measured directly, with no cluster and no tool, by a
`#[cfg(test)]` counting `#[global_allocator]` over the committed captures —
`src/k8s_tests.rs` § WHAT A POD COSTS IN MEMORY, and the note over
`INITIAL_LIST_PAGE` in `src/k8s.rs`. Against these slopes it says **the store is
the small term**: one stored pod is **2 701 bytes all in at the median** (1 032
of struct, 1 669 of heap), which is *less* than the 3 708-byte capture median it
arrives in and not 6.7× it; the published second copy is **3 028**; the two
together are **5 729 bytes per pod, under 20 % of the 27–31 KB slope**. The
expensive object is the one the snapshot is pruned *out of* — a decoded `Pod` is
**17 956 bytes all in, 6.43×** its pruned form, and a 500-object page of those is
**~9 MB on the captures, 18–24 MB scaled to a live pod**, which is 58–75 % of
what the store does not account for.

**A page buffer capped at 500 also explains the disagreeing slopes D171 left
unmodelled.** Below 500 pods it holds one entry per pod and is charged to the
slope in full; at and above 500 it stops growing and drops out of the marginal
altogether. That is exactly the shape of D171's two per-pod slopes — **82.5 KiB**
between 11 and 111 pods, where the buffer grows with every pod, against
**7.52 KiB** between 111 and 10 011, where it grows by 389 entries over 9 900
pods — with no per-object cost changing anywhere. (D171's third figure, 25.8 KiB,
is per *workload object* and is not a point on that curve.)

### The peak, term by term

`memusage`'s block-size histogram, converted to glibc chunk sizes
(`max(32, align16(request + 8))`) over every bucket:

```
r2.mem: buckets_counted=65711  requested~13.8MB chunked~15.0MB  overhead=9% alloc<=15B=37961  (58%)
r3.mem: buckets_counted=412686 requested~87.2MB chunked~94.4MB  overhead=8% alloc<=15B=219954 (53%)
r4.mem: buckets_counted=559920 requested~121.3MB chunked~131.7MB overhead=9% alloc<=15B=332028 (59%)
```

59 % of allocations request 15 bytes or fewer and each takes a 32-byte chunk, but
over the whole mix that is only 9 % more bytes than were requested:
**per-chunk overhead is not where the resident set went**, on a cumulative count.

Peak resident set at shape 4, with every term above named:

| term | bytes | how it was known |
|---|---|---|
| live heap held by the program | 37.4 MB | measured — `memusage heap peak` |
| chunk overhead on it | ~3.3 MB | **estimated** — see below |
| binary + shared libraries + stacks | 10.6 MB | measured — `smaps` |
| glibc arena slack | 8.0–10.5 MB | measured — `arena_max=1` counterfactual |
| **accounted** | **59.3–61.8 MB** | sum of the four |
| **unaccounted residual** | **~1.4 MB (2.3 %)** | measured peak 60.7–63.3 MB, minus the above |

**The chunk-overhead row is the one extrapolation in that table and is not an
instrument reading.** The 9 % rate is over the *cumulative* allocation stream,
59 % of which is ≤15-byte requests that are overwhelmingly short-lived; nothing
here measured the size distribution of the **live** set, which is the set the
overhead would actually apply to. The residual sits inside that row's
uncertainty, and it is carried as its own line rather than folded into a total.

## What could not be proven, and why

- **The resting live heap, exactly.** `malloc_stats()` and `mallinfo2()` would
  give it; `gdb` is installed but `ptrace_scope=1` and the absence of any symbol
  table for libc on this host defeated three attempts to call either in a
  running process (`No symbol table is loaded`). The resting figure is therefore
  bounded rather than read: 45 312 KiB (46.4 MB) steady under tunables that
  return freed memory, minus the 10.6 MB non-heap floor, is **~36 MB at rest
  against a 37.4 MB peak** — so the initial LIST's transient is ~5 MB and the
  rest of the peak is resident store, but that last step is subtraction and not
  an instrument.
- **Which allocation the per-pod slope is in.** Nothing in this run separates
  the page buffer from the store from allocator retention; that took a counting
  allocator over the captures, and the answer is in § *What the slope is a slope
  of*. Splitting it *inside* a live process would still need an allocation
  profiler with stack traces (`heaptrack`, `valgrind --tool=massif`), and neither
  is installed here.
- **Anything above 1 011 pods.** D171's 10 011-pod reading was not repeated.
- **Whether `arena_max` is reachable without a new dependency.** The tunable is
  an environment variable read before `main`; `mallopt(3)` is the in-process
  equivalent and this repo has no `libc` crate.
- **Any machine that is not this one.** Arena count is a function of core count
  and thread contention, and both differ elsewhere.
- **What the fixture cluster cost these numbers.** Leaving `k8rs` up throughout
  is required — it is the PM's cluster and this run may not disturb it — but a
  four-container cluster on the same host is also a **confounder**, and the term
  it lands on is the one named just above: arena count follows core count *and
  thread contention*, and the machine was contended for every reading. So the
  **8.0–10.5 MB arena slack is measured under co-tenancy** and is the figure a
  backlog entry now proposes acting on. The 37.4 MB live-heap figure is not
  exposed to it — it counts this process's own `malloc` calls and nothing else.
  [D84](../NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
  is this project's precedent for a second cluster silently moving a reading.

## What was captured and what was not

Nothing. No fixture, no object dump, no file under `tests/`. The generated
objects are described above and in D171; the throwaway cluster is gone.

## Teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: [the two nodes of the throwaway cluster]
$ kind get clusters
k8rs
$ kubectl config current-context
kind-k8rs
$ kubectl --context kind-k8rs get nodes --no-headers | wc -l
4
$ pgrep -a -x k8rs; pgrep -af 'kubectl proxy'
(nothing)
```

`kind create cluster` sets the current kubectl context, so it was put back to
`kind-k8rs` after `up` and again after `down`; the fixture cluster answered with
its four nodes both times.
