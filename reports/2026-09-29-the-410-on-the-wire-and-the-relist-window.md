# The `410` shapes on the wire, and the fields a relist leaves unchanged (2026-09-29)

`k8s-admin`, step 6 over the uncommitted `src/k8s.rs` + `src/k8s_tests.rs`
([D314](../NOTES.md#d314--phase-5s-close-family-review-no-blockers-two-box-bodies-that-do-not-describe-their-own-code-and-a-security-gate-row-the-headless-surface-does-not-meet-2026-09-28),
[D315](../NOTES.md#d315--the-fourth-member-of-a-class-the-code-had-already-named-found-by-the-closes-own-whole-phase-pass-2026-09-28)).

`relisting()` keys on `status.code == 410`. `kube_core::watch::WatchEvent::Error`
carries `Box<Status>` deserialized straight off the wire with
`#[serde(default)] pub code: u16` (`kube-core-4.2.0/src/watch.rs:27`,
`response.rs:33-35`), so whether the predicate fires at all depends on what a real
apiserver writes. Measured against the running fixture cluster, **read-only**
(GETs only; `kind get kubeconfig` to stdout, no `export`). No second cluster was
created: `K8RS_CLUSTER=review ./scripts/cluster.sh up` refused on
`Bind for 127.0.0.1:6443 failed: port is already allocated` and kind deleted its
own partial nodes; `kind get clusters` → `k8rs` before and after.

Server: `v1.36.1`. Pods on the cluster at measurement time: `41`.

## M1 — the watch desync, as the apiserver writes it

```
$ timeout 20 kubectl get --raw '/api/v1/pods?watch=true&resourceVersion=1' | head -1
{"type":"ERROR","object":{"kind":"Status","apiVersion":"v1","metadata":{},"status":"Failure","message":"too old resource version: 1 (3171407)","reason":"Expired","code":410}}
exit=0
```

Fields the predicate reads: `code` = `410`, `reason` = `"Expired"`. Both present.

## M2/M3 — the paginated LIST, with a `continue` token whose `rv` is too old

A real token was taken from `/api/v1/pods?limit=1`, base64-decoded, its `rv`
replaced with `1`, and re-encoded. Token fields: `['rv', 'start', 'v']`, `rv` is an
`int`. The token itself and its `start` key are not pasted — `start` is an object
name.

```
$ timeout 20 kubectl get --raw "/api/v1/pods?limit=1&continue=<rv=1 token>"
Error from server (Expired): The provided continue parameter is too old to display a
consistent list result. You can start a new list without the continue parameter, or use
the continue token in this response to retrieve the remainder of the results. [...]
```

The response body's own fields:

```
$ ... -v=8 2>&1 | grep -oE '"(code|reason|status)":("[A-Za-z]*"|[0-9]+)' | sort -u
"code":410
"reason":"Expired"
"status":"Failure"
```

## M4 — watch-cache metrics this server exposes

```
$ kubectl get --raw /metrics | grep -oE '^apiserver_watch_cache_[a-z_]+' | sort -u
apiserver_watch_cache_consistent_read_total
apiserver_watch_cache_events_dispatched_total
apiserver_watch_cache_events_received_total
apiserver_watch_cache_initializations_total
apiserver_watch_cache_read_wait_seconds_bucket
apiserver_watch_cache_read_wait_seconds_count
apiserver_watch_cache_read_wait_seconds_sum
apiserver_watch_cache_resource_version
```

No `apiserver_watch_cache_capacity` on `v1.36.1`, so the ring size that decides how
often M1 recurs was **not** measured here. `apiserver_watch_cache_initializations_total`
is `1` for `pods`, `nodes` and `deployments`.

## M5 — one paginated LIST round trip

`kubectl get --raw '/api/v1/pods?limit=500'`, 41 pods, loopback, wall clock around
the subprocess (so process spawn is included):

```
  round trip 1: 0.063 s
  round trip 2: 0.069 s
  round trip 3: 0.066 s
  round trip 4: 0.077 s
  round trip 5: 0.068 s
```

## M6 — kube's state machine at the lines the new doc cites

Read off `kube-runtime-4.2.0/src/watcher.rs` in
`~/.cargo/registry/src/index.crates.io-*/`:

| Cited | At that line |
|---|---|
| `InitialListFailed` → `State::Empty` for any list error | `(Some(Err(Error::InitialListFailed(err))), State::Empty)` — the `Err(err)` arm of `api.list(&lp)`, no code test |
| `WatchError` resets only on `410` | `let new_state = if err.code == 410 { State::default() } else { State::Watching { resource_version, stream } }` |
| `WatchStartFailed` keeps the `resourceVersion` | `(Some(Err(Error::WatchStartFailed(err))), State::InitListed { resource_version })` |
| `WatchFailed` keeps the stream | `(Some(Err(Error::WatchFailed(err))), State::Watching { resource_version, stream })` |
| `NoResourceVersion` re-lists | `(Some(Err(Error::NoResourceVersion)), State::Empty)` on the list path; `State::default()` on both `State::Watching` object paths |

## M7 — the backoff band, off the constants

`watcher.rs:981-988`:

```rust
Self(ResetTimerBackoff::new(
    ExponentialBackoff::new(Duration::from_millis(800), Duration::from_secs(30), 2.0, true),
    Duration::from_secs(120),
))
```

`min_delay` 800 ms, `max_delay` 30 s, factor 2.0, jitter on, reset timer 120 s.
`StandingBackoff::reset` is `fn reset(&mut self) {}` (`k8s.rs:8600`).
`StreamBackoff` calls `backoff.reset()` on every non-error item
(`utils/stream_backoff.rs:88-91`).

## M8 — the store fields a post-LIST relist changes

Read off `Watch::take` / `Watch::outstanding` / `Store::troubles` at `HEAD` + this
diff. For a watch that has already completed one LIST and then takes
`Err(WatchError(410))`, `Init`, `InitApply…`:

| Field | Before the `410` | During the relist, with this diff |
|---|---|---|
| `failure` | `None` | `None` — `Watch::failed` drops it |
| `complete` | `true` | `true` — only `InitDone` writes it, never reset |
| `live` | the listed objects | unchanged; `Init` writes `filling`, not `live` |
| `ended` | `false` | `false` |
| `unfinished` | `false` | `false` — `stop_waiting` is `!complete` |
| `outstanding()` | `None` | `None` — gated on `!self.complete` |
| `Store::troubles()` filter `failure.is_some() \|\| ended \|\| unfinished` | no row | no row |
| `last_progress` | last `Init`/`InitApply` | restamped on the relist's `Init` and each `InitApply` |

At the old `HEAD` the same sequence left `failure = Some(WatchError(410))`, so the
`troubles()` filter produced a row. `answer()` (`k8s.rs:1139`) has arms for
`400 401 403 404 409 422` and six `reason` strings; neither `410` nor `"Expired"`
is among them, so that row's `Trouble::fault()` was `Fault::Unanswered`.

## M9 — the three downstream readers of `Fault::Unanswered`

```
$ grep -n "nothing usable came back" src/views.rs
4259:        Fault::Unanswered => format!("nothing usable came back when k8rs tried to {asked}"),
```

`src/views.rs:4344-4347` — `next_step`:

```rust
Fault::Unanswered => Some(
    "Check the server address this kubeconfig names, and that this machine can reach it"
        .to_string(),
),
```

`src/main.rs:2645` then `:987-988`:

```rust
let watch_trouble = !troubles.is_empty();
...
fn health(input: &Input) -> Option<String> {
    if input.watch_trouble {
        return None;
```

(The brief's line numbers `views.rs:4229` / `views.rs:4314` are `4259` / `4344` at
this tree.)

## M10 — readers of `insecure` outside `k8s.rs`

```
$ grep -rn "insecure" src/views.rs src/main.rs src/ui.rs
src/ui.rs:763:    pub insecure: bool,
src/ui.rs:2005:            screen.insecure.then_some(tls.as_str()),
src/ui.rs:3186:    } else if row.insecure {
src/main.rs:7506:    insecure: bool,
src/main.rs:8316:        console.insecure = tls_unverified(&console.contexts);
src/main.rs:8444:        console.insecure = tls_unverified(&console.contexts);
src/main.rs:8794:    contexts.iter().any(|row| row.current && row.insecure)
src/main.rs:9281:        insecure: console.insecure,
```

Every one of those resolves to `k8s::Choice::insecure` via `main::tls_unverified`.
`grep` finds no reader of `k8s::Session::insecure` in `main.rs`, `views.rs` or
`ui.rs`. `contexts(kubeconfig, asked_for)` sets `current` from
`wanted(kubeconfig, asked_for)`, i.e. the `--context` override when one was given.

## M11 — what `security-guard.py` bans

`scripts/security-guard.py:801-812`, the `"TLS verification is never disabled by us"`
pattern:

```
r"(?:\baccept_invalid_(?:certs|hostnames)\s*(?:=\s*true|:\s*true|\(\s*true)"
r"|\bdanger\w*\s*\("
r"|\bSslVerifyMode\s*::\s*NONE)"
```

It matches on `accept_invalid_certs`/`accept_invalid_hostnames`, `danger*(` and
`SslVerifyMode::NONE`. It does not name `Session`, and `insecure: false` literals
exist at `k8s.rs:8308` and `main_tests.rs:4219`.

---

# Round two — the fix (D317's `Watch::desynced`)

Same run, same cluster, appended rather than a second file. Read-only GETs again; the
scratch kubeconfig was re-fetched with `kind get kubeconfig --name k8rs` to stdout and
deleted afterwards. No cluster created — `kind get clusters` reads `k8rs` alone.

## M12 — which `watcher::Error` variant a too-old resourceVersion actually becomes

The distinguishing question is the **HTTP status**, because `WatchError` is dropped by
`relisting` and `WatchStartFailed` is not.

```
$ kubectl get --raw '/api/v1/pods?watch=true&resourceVersion=1' -v=6 2>&1 | grep round_trippers
"Response" verb="GET" url=".../api/v1/pods?resourceVersion=1&watch=true" status="200 OK" milliseconds=7
```

```
$ kubectl get --raw '/api/v1/pods?limit=1&continue=<rv=1 token>' -v=6 2>&1 | grep round_trippers
"Response" verb="GET" url=".../api/v1/pods?continue=<redacted: the token base64-encodes an object name>&limit=1" status="410 Gone" milliseconds=9
```

A watch whose `resourceVersion` is too old is **`200 OK`** with the `410` as an in-band
`ERROR` frame (the M1 body). A paginated LIST whose `continue` token is too old is an
**HTTP `410 Gone`**.

Note the `reason` on the two paths differs from M1/M3's: `kubectl` prints the LIST one as
`410 Gone` on the status line while the body's `reason` field is `Expired` (M3). Neither
`relisting` nor `answer()` reads the status line.

## M13 — `metadata.resourceVersion` across the pages of one paginated LIST

```
$ kubectl get --raw '/api/v1/pods?limit=5'          # then follow metadata.continue twice
  page 1: resourceVersion = 3185615  items = 5
  page 2: resourceVersion = 3185615  items = 5
  page 3: resourceVersion = 3185615  items = 5
  equal across pages: True
  cluster resourceVersion now: 3185615
```

Constant. `watcher.rs` carries the per-page value into `State::InitListed { resource_version }`
and watches from it (`:555-559`), so the `resourceVersion` a watch is established with is the
one page 1 was served at, whatever the walk took.

## M14 — where `desynced` can be cleared, per loop, read off `Watch::take`

`answered` is `false` for `Init` and `InitApply`, `filling.is_some()` for `InitDone`, `true`
for `Apply`/`Delete`. The event sets each loop actually produces:

| Loop | Events it emits | An `answered == true` event in it? |
|---|---|---|
| walk never completes (`InitialListFailed(410)` each pass) | `Init`, `InitApply`×N, `Err(410)` | **no** — neither `Ok` member is `answered` |
| walk completes, watch desyncs at once (`WatchError(410)` each pass) | `Init`, `InitApply`×N, `InitDone`, `Err(410)` | **yes**, `InitDone`, every pass |

`Apply`/`Delete` require `State::Watching`, which is reached only through `State::InitListed`,
which is entered only with the `InitDone` at `:555-559`.

## M15 — `Store` surfaces for the second loop, read off the code at this diff

For a watch whose walk completes and which then takes `WatchError(410)` on every watch
establishment:

| | Value |
|---|---|
| `failed()` branch taken | `relisting(&f) && !self.desynced` → drop, every pass, because `InitDone` cleared `desynced` |
| `failure` | `None` throughout |
| `complete` | `true` |
| `live` | replaced whole at each `InitDone` |
| `troubles()` | empty |
| `still_listing()` | empty (`outstanding()` gated on `!complete`) |
| `main.rs:988` | prints `○ nothing is broken` |
| store refresh interval | one `StandingBackoff` step + one full walk; the step ramps 0.8-1.6 s → 30-60 s (M7) and `reset` is a no-op |

`a_second_desync_with_no_list_between_them_is_reported_and_the_first_is_not`'s fourth step
pins the drop for this case: *"a complete LIST landed and the next ordinary desync was still
reported, so one compaction an hour lights a kind up for the life of the process."*

## M16 — the bootstrap loop's two surfaces after the second drop

`Fault::standing(Fault::Unanswered)` is `false` (`k8s.rs:1039-1041`), so `Watch::settled` is
`false` for a recorded `410`, so `progress()` is `Some` while `complete` is `false`:

| Call | For the kind, after the second dropped desync at bootstrap |
|---|---|
| `troubles()` | a row — `failure.is_some()` |
| `still_listing()` | a row too — `progress()` = `Some((filling.len(), last_progress))` |
| `filling.len()` | resets to `0` on each `Init` |
| `last_progress` | restamped on each `Init` and each `InitApply` |
| `listed()` / `snapshot()` | `false` / `None` |

## M17 — `said` on the path to the screen

```
$ grep -n "\.said()" src/main.rs
2884:                Some(fault) => because(fault, &asked, renewal, trouble.said().as_deref()),
```

`because()`'s arms: `Fault::Rejected` interpolates it (`views.rs:4232-4235`,
`"… and said: {said}"`); `Fault::Unanswered` does not (`views.rs:4259`,
`format!("nothing usable came back when k8rs tried to {asked}")`). The server's own message
for the LIST case is the M3 body — *"The provided continue parameter is too old to display a
consistent list result. You can start a new list without the continue parameter, …"*
