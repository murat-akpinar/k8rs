# Phase 8's TUI spike — what its watch, its loop and its sanitiser actually do

`k8s-admin`, 2026-09-05, operator review of `examples/spike_tui.rs` (276 lines,
untracked at measure time) against
[NOTES § D238](../NOTES.md#d238--the-spike-cannot-import-the-product-and-the-tui-crate-does-not-go-in-the-shipped-artifact-to-learn-a-loop-2026-09-05).
Every number below was produced on **this** machine. Nothing was written to any
cluster, no fixture was produced, and nothing under `tests/` was touched.

## The rig

**No `kind` cluster of my own was created.** Almost every claim here is about how
the client behaves when the *server* does something, so the server is a stub: a
`python3 http.server` on `http://127.0.0.1:18443` and a four-line kubeconfig in
the scratchpad naming it, handed to the process as `KUBECONFIG`. That gives byte
control over the LIST body, the watch stream and the failure, which no real
cluster does, and it means the PM's `k8rs` cluster was never a variable.

The spike was built from a **copy** of the tree (`tar`-excluded `target/` and
`.git/`) with its own `CARGO_TARGET_DIR`, per CLAUDE.md's rule for a reviewer who
needs to build. Debug where noted, `--release` for the load table. It is driven
under `tmux` — a detached session, `send-keys` for input, `capture-pane -p` for
output — which is the method D238 ruling 3 names.

One run touched the live `kind-k8rs` cluster, read-only: a watch, no writes. It
is reported at the end.

**Environment note.** `/tmp` was at **100% of 12 GiB** when this review started
and a `cargo build` there died with `No space left on device (os error 28)` — the
condition [D133](../NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21)
was written about. The target directory was moved to the home partition (894 GB
free). Recorded because a mutation run started in that state reports `unviable`.

## 1. What the spike puts on the wire

Stub replies to the LIST with an empty `PodList`, then holds the watch open.

```
$ MODE=ok python3 stub.py 18443
stub MODE=ok on ('127.0.0.1', 18443)
[  1.032s] #1 GET /api/v1/pods?&limit=500
[  1.033s] #2 GET /api/v1/pods?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=12345
```

Two requests, then silence. One paginated LIST at the client-go page size, one
long-poll watch with bookmarks. **This is invariant 6's shape and it matches the
product's own** — `src/k8s.rs` builds `watcher::Config::default().page_size(500)`
and the query string above is the one its § THE WATCH REQUEST comment describes
line by line. No `resourceVersion=0` on the LIST, because `ListSemantic` defaults
to `MostRecent`; that is a quorum read and `src/k8s.rs:3571` says it is chosen on
purpose.

## 2. A permanent refusal — the retry rate, measured

Stub replies **403** to everything, with a real `Status` body. Twenty seconds,
100x12 pane, debug build.

```
--- as-written : header after 20s of permanent 403 ---
 1 pods · 438 watch events · cursor 0
--- as-written : cpu/etime of pid 3709540 ---
11.3      20        2
--- as-written : requests the stub saw ---
[  1.039s] #1 GET /api/v1/pods?&limit=500
[  1.041s] #2 GET /api/v1/pods?&limit=500
[  1.087s] #3 GET /api/v1/pods?&limit=500
[  1.134s] #4 GET /api/v1/pods?&limit=500
[  1.182s] #5 GET /api/v1/pods?&limit=500
[  1.227s] #6 GET /api/v1/pods?&limit=500
```

438 events in ~19.5 s of running. A refused cycle emits `Ok(Init)` then
`Err(InitialListFailed)`, so that is **219 LIST attempts, about 11 per second
averaged, indefinitely**, and 219 full terminal redraws with them.

**The two sides disagree and the disagreement is worth stating.** Stub-side the
first gaps are 46, 47, 48, 45 ms — 21/s — while the client-side count averages
11/s over the whole run, so the stub slowed down under its own thread-per-request
load. Both numbers are bounded by the Python stub and neither is a ceiling for a
real apiserver. **What does not depend on the rate is that the gaps are flat
rather than growing**, and that is the no-backoff proof.

The same binary with `.default_backoff()` added on one line, same stub, same 20 s:

```
--- with-default-backoff : header after 20s of permanent 403 ---
 1 pods · 16 watch events · cursor 0
--- with-default-backoff : cpu/etime ---
 0.6      20        0
[  1.056s] #1 GET /api/v1/pods?&limit=500
[  2.472s] #2 GET /api/v1/pods?&limit=500
[  4.062s] #3 GET /api/v1/pods?&limit=500
[  5.154s] #4 GET /api/v1/pods?&limit=500
[  6.087s] #5 GET /api/v1/pods?&limit=500
[  7.170s] #6 GET /api/v1/pods?&limit=500
```

8 cycles instead of 219, and 0.6% CPU instead of 11.3%. **The gaps plateau around
1 s and do not climb** — 1.42, 1.59, 1.09, 0.93, 1.08 s — which independently
reproduces the reason `src/k8s.rs:6662-6674` gives for refusing `.default_backoff()`
and writing `StandingBackoff`: `StreamBackoff` resets on every non-error item and
a refused `watcher()` emits `Ok(Init)` before every `Err`, so the exponential
never leaves its first step.

kube's own source, 4.2.0: `watcher()` at `watcher.rs:791` is a bare
`futures::stream::unfold` with no backoff in it, and `watcher.rs:26` reads *"To
avoid constantly looping errors, make sure backoff is applied."*

## 3. A `410 Gone` re-list — the row that never leaves

Stub: LIST returns `alpha` and `beta`; two seconds later the watch emits a
`410 Expired`; the re-LIST returns **only `alpha`**, i.e. `beta` was deleted while
the watch was down and no `Delete` event exists for it.

```
[  1.06s] LIST  #1  /api/v1/pods?&limit=500  -> ['alpha', 'beta']
[  1.06s] WATCH #1  ...&resourceVersion=100
[  3.06s]   -> sending 410 Gone on watch #1
[  3.07s] LIST  #2  /api/v1/pods?&limit=500  -> ['alpha']
[  3.07s] WATCH #2  ...&resourceVersion=300
```

Screen at T+1.5 s:

```
 2 pods · 4 watch events · cursor 0
┌ pods ───────────────────────────────
│> default/alpha  Running restarts=0
│  default/beta  Running restarts=0
```

Screen at T+7.5 s, four seconds after the successful re-LIST and the healthy
second watch:

```
 3 pods · 7 watch events · cursor 0
┌ pods ───────────────────────────────
│> ! watch  error returned by apiserver during watch: too old resource version: 100 (300): Expired
│  default/alpha  Running restarts=0
│  default/beta  Running restarts=0
```

`default/beta` does not exist in the cluster and is on the screen. It stays for
the life of the process. The `! watch` row is also still there four seconds after
the watch recovered, and it sorts to position 0 — `!` is 0x21 — so it holds the
cursor. The header counts it: `3 pods` where the cluster has one.

`src/k8s.rs` does not do this. `Event::Init` there sets `filling = Some(...)`,
`InitApply` fills it and `InitDone` swaps it into `live` whole (`:1569-1588`).

## 4. The open dialog's target moves on a watch event

Stub: LIST returns `alpha` and `beta`; eight seconds later the watch delivers an
`ADDED` for `default/aaa-brand-new`, a pod that sorts before `alpha`. The operator
presses `d` on the top row at T+3 s and then **touches nothing**.

```
=== operator selects the top row and presses d ===
 2 pods · 4 watch events · cursor 0
                  ┌ Confirm ───────────────────────────
                  │Pretend this deletes:
                  │  default/alpha
                  │
                  │> Yes, do it
                  │  No, leave it alone

=== 8 seconds later, no key pressed, one pod created elsewhere in the cluster ===
 3 pods · 5 watch events · cursor 0
                  ┌ Confirm ───────────────────────────
                  │Pretend this deletes:
                  │  default/aaa-brand-new
                  │
                  │> Yes, do it
                  │  No, leave it alone
```

Stub log:

```
[  1.0s] LIST -> ['alpha', 'beta']
[  1.0s] WATCH opened
[  9.0s] sending ADDED default/aaa-brand-new (sorts before default/alpha)
```

The confirmation names a different object than the one the operator selected, and
the "Yes" cursor did not move. The cause is `examples/spike_tui.rs:193` —
`app.rows.keys().nth(app.cursor)` is evaluated inside `draw`, so the target is a
live index rather than a value captured when the dialog opened.

## 5. Invariant 9 — what `clean()` lets through that `text()` does not

Stub sends a watch `ERROR` whose `Status.message` is 254 characters and contains a
newline, U+202E (right-to-left override), U+202C, and U+200B inside a name.
`examples/spike_tui.rs:236` routes it through `clean()`. Pane row, `cat -A`:

```
> ! watch  error returned by apiserver during watch: pods is forbidden:namespace M-bM-^@M-.gnp-doprdM-bM-^@M-, may not list pod M-bM-^@M-^Kkube-system/coredns
```

Three measured differences from `src/k8s.rs`'s `text()` / `unprintable()`:

- `M-bM-^@M-.` is `e2 80 ae` = **U+202E**, `M-bM-^@M-,` is `e2 80 ac` = U+202C, and
  `M-bM-^@M-^K` is `e2 80 8b` = U+200B. All three are inside `unprintable()`'s
  ranges (`'\u{202a}'..='\u{202e}'`, `'\u{200b}'..='\u{200f}'`) and all three
  reached the terminal. The rendered text shows `gnp-doprd`, which is `prod-png`
  drawn backwards — Trojan Source, live, from a server-controlled string.
- `forbidden:namespace` — the newline was **deleted**, gluing two words. `text()`
  turns an unprintable that is `char::is_whitespace` into one space for exactly
  this reason (`src/k8s.rs:261-265`).
- The 254-char message was cut at 120 with **no marker**. `text()` appends
  `SHORTENED`, so a reader can tell.

`take(120)` also counts `char`s where `text()`'s cap counts bytes.

## 6. Load — the initial paint, release build

Stub returns N pods in one page, then a quiet watch. Time from process start to
the header reading `N pods`, 100x24 pane, `--release`.

| pods | time to full list | frames | %CPU | RSS |
|---|---|---|---|---|
| 500 | **0.108 s** | 502 | 61.5 | 9.2 MB |
| 2000 | **0.836 s** | 2002 | 84.8 | 9.8 MB |
| 5000 | **4.15 s** | 5002 | 97.6 | 12.1 MB |

Ten times the pods, thirty-eight times the time. Debug build for comparison:
0.77 s at 500 and **19.8 s** at 5000.

Two independent multiplicands, and either one alone removes the curve. One draw
per `InitApply` gives N frames — the file's own header declares this and defers
coalescing to Phase 12. `draw` at `examples/spike_tui.rs:171-175` then builds a
`Vec<String>` over **every** row of the map on every frame, 24 of which are
visible, so each frame is O(total) and not O(viewport). N x N is 25 000 000
`format!`s at N=5000. Only the first is written down anywhere.

Memory is fine: two short `String`s per pod, 12 MB RSS at 5000.

## 7. The mechanical checks

```
$ cargo clippy --offline --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.82s
=== exit 0 ===
$ python3 scripts/write-guard.py
write-guard: 72 methods known across 2 types (kube::Api 48, kube::core::Request 24),
45 banned outside src/ops.rs, clippy.toml names exactly those, and src/ops.rs is the
only file in 5 cargo roots that silences any of the 4 lints whose `allow` would turn
the ban off — with no `-A` naming one of those on any committed rustc command line
either — OK
```

`--all-targets` compiles `examples/`, so `clippy.toml`'s ban was applied to the
spike and found nothing.

Terminal restore on the deliberate panic, `stty` before and after in the same pane:

```
icanon echo  <- BEFORE
thread 'main' (3720918) panicked at examples/spike_tui.rs:107:17:
spike: deliberate panic, to prove the terminal comes back
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
EXIT=101
icanon echo  <- AFTER PANIC
```

`q` exits with `EXIT=0` inside the 3-second window measured.

## 8. The two knowledge claims

**`crossterm::event::EventStream` is unreachable with a `ratatui`-only manifest.**

```
$ cargo tree -e features -i crossterm
crossterm v0.29.0
├── crossterm feature "bracketed-paste"
├── crossterm feature "default" (*)
├── crossterm feature "derive-more"
├── crossterm feature "events"
└── crossterm feature "windows"
$ cargo tree -e features | grep -c "event-stream"
0
```

The author's list is exact. Structurally as well as by count: `ratatui` 0.30.2's
`[features]` has no entry mentioning `event-stream`, `ratatui-crossterm` 0.1.2's
only passthroughs to the crossterm it wraps are `serde`, `scrolling-regions` and
`unstable-backend-writer`, and `EventStream` is `#[cfg(feature = "event-stream")]`
at `crossterm-0.29.0/src/event.rs:124`.

**`spawn_blocking` would hang the process; `std::thread` does not.** Two probe
binaries, identical except for the spawn, each parking a 10-second sleep and then
returning from `main` at 200 ms.

```
=== spawn_blocking ===
main future done at 201.270986ms -- returning now
blocking task finished at 10.000226496s
process exited after 10.004000311 s

=== std::thread ===
main future done at 201.408964ms -- returning now
process exited after .205872434 s
```

The runtime's drop waited the full ten seconds for the parked blocking task.
`event::read()` does not return after ten seconds, it returns when a key arrives.

## 9. The one live-cluster run

The spike, built from the copy, against `kind-k8rs` — a watch, read-only, nothing
written. Its header settled at **41 pods, 44 watch events**, consistent with
D238's *43 frames for 41 pods* (`Init` + 41 x `InitApply` + `InitDone`) plus real
changes arriving during the run. Pod names are not reproduced here.

## What was checked and did not break

- `Delete` for a key never applied: `BTreeMap::remove` is a no-op, cursor clamp holds.
- No mutating call, direct or indirect: `Config::from_kubeconfig`, `Client::try_from`,
  `Api::<Pod>::all`, `watcher`, `.boxed()`, `.next()`. `ResourceExt` is avoided, so
  no `Api::namespace`. The `exec` credential-plugin path inside
  `Config::from_kubeconfig` can spawn a process, but that is kube honouring a
  kubeconfig and is identical in the shipped binary.
- `Client::try_default` is genuinely absent; the in-cluster ServiceAccount door is
  not opened.
- Terminal restored on panic and on `q`; no credential in the panic output.
- Wire shape and page size match the product's watch exactly.
