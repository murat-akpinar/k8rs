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

---

# Round three — the trailer line (`unverified`, D314's reader)

Same run, appended. Nothing was run on the test host and no cluster was created; these are
reads of the local tree at `M src/main.rs` / `M src/main_tests.rs` over `467e683`.

## M18 — the trailer stack, code order against the page

`render()` (`main.rs:855-931`), in call order:

| Slot | Call | Gate |
|---|---|---|
| 1 | `clock(input.skew)` | `Some` skew past the threshold |
| 2 | `serving_certificate(input.serving_expiry, now)` | `Some` expiry within `CERT_EXPIRY_WARN`; **two arms**, `left < 0` and `left >= 0` |
| 3 | `login_certificate(...)` | `!drawn_as_a_row(..)` and the band |
| 4 | `unverified(input.insecure)` | `input.insecure` |
| 5 | `check_switched_off(namespace_scope)` | a namespace scope |

`screens/once.md` § Stacked with the other trailer lines: *"clock, then C2, then C1, then the
connection-unverified line, then the check-that-could-not-run line."* Same order.
`spaced`'s doc reads `the five trailer lines`; the comment at `:860-864` names all five in
order. Both match.

## M19 — can C2's **expired** arm and `unverified` share a run?

Three code facts, each read at its own site:

1. `serving_certificate`'s expired arm is a **report** line, not a wall
   (`main.rs`, `if left < SignedDuration::ZERO { return Some(format!("… expired {} ago …")) }`),
   printed from slot 2 of the stack above.
2. `k8s::Serving`'s own doc: *"**The date is no longer what separates this from `Until` — a
   completed handshake is.** `Until` is proof some replica behind this address serves a
   certificate a verifying client accepts"*, and `Expired` is *"The `notAfter` rustls **refused**
   a sample over … a verifying kubeconfig never receives the bytes"*
   (`k8s.rs:8907-8924`). So a handshake that **completes** over an already-expired certificate
   yields `Serving::Until(<past timestamp>)`, not `Serving::Expired`.
3. `certificate_is_why` — the wall — has **two** gates:

```rust
let k8s::Serving::Expired(at) = session.serving_expiry else { return None; };
let unanswered = |fault: Option<k8s::Fault>| fault == Some(k8s::Fault::Unanswered);
(unanswered(session.version.as_ref().err().map(k8s::fault))
    && unanswered(session.served.as_ref().err().map(k8s::fault)))
.then(|| …)
```

and `trust_only` copies the knob into the probe:

```rust
probe.accept_invalid_certs = config.accept_invalid_certs;
```

The matrix that follows, per `--once` run:

| Knob | Server certificate | `serving_expiry` | `certificate_is_why` | What prints |
|---|---|---|---|---|
| on | already expired | `Until(past)` | `None` — not `Expired` | **report: C2 expired arm *and* `unverified`** |
| off | already expired | `Expired(at)` | `Some` — version and served both `Unanswered` | wall, exit 2, no report |
| off | expired on one replica behind a LB, session reads fine | `Expired(at)` | `None` — version and served succeeded | report: C2 expired arm, no `unverified` |
| on | expiring, not expired | `Until(future)` | `None` | report: C2 expiring arm *and* `unverified` |

`screens/once.md` § When the connection was never verified: *"The two sentences can never share
a run for exactly that reason: this one only ever prints when the setting is true, and that wall
is only ever reached when it is false."* And § Stacked: *"the expired-and-typed reading for C2 …
do not join it, because when either fires there is no report for it to join."*

The two sentences that co-occur in row 1, adjacent in the stack:

```
A certificate the API server presented — not your kubeconfig's — expired 12 days ago (was valid
until T). When that happens, kubectl and everything else stop being able to reach a cluster until
someone on the control plane renews its certificate — not something k8rs can do.

k8rs never checked that the server on the other end of this connection is really this cluster —
your kubeconfig sets `insecure-skip-tls-verify: true`, and `kubectl` would skip the same check
with it. Everything above came back over that unchecked connection. …
```

Not measured against a live server: no cluster here serves an expired certificate. What would
settle it is a kubeconfig with the knob on pointing at an apiserver past its `notAfter` — on kind,
by rewinding the control-plane container's clock past the cert's expiry, or by re-signing the
apiserver cert with a past `notAfter`. Both are cluster writes and are the PM's.

## M20 — an RBAC refusal on pods is a wall, not a report

```rust
if let Some(why) = pods_unread(&store.troubles(), &coverage, at.renewal) {
    ending = Some(why);
    done = true;
    stop.abort();
    return;
}
if let Some(report) = live_report(store, now, &mut last, analysis, &at) {
```

`main.rs:3953-3958`, ahead of `live_report`, so `render()` and slot 4 are never reached.
`pods_unread` fires on `trouble.kind == ObjectKind::Pod && !trouble.listed`, which is what a
`Fault::Refused` watch leaves (`Watch::settled`). Its text asserts a fact about a named cluster
and ends in an errand to a human:

```
  What k8rs asked for: pods <scope>
  What happened: <because(Refused, "`list` and `watch` pods", …)>

  Ask whoever runs this cluster for a role that may read pods in every namespace …
```

## M21 — is there any surface that claims a connection **was** verified?

```
$ grep -rn '"[^"]*verified' src/main.rs src/views.rs src/ui.rs
src/ui.rs:1317:    format!("{} TLS not verified", mark(theme::ALARM))
```

One string, printed only when the flag is true. `unverified(false)` returns `None`. No surface
prints a positive verification claim, so `insecure: false` produces silence.

---

# Round four — the clause, and the page's corrected disambiguation

Same run, appended. Reads of the local tree only; no cluster, no host. Nothing here serves an
expired certificate, so the pair itself is still unmeasured against a binary (see M25).

## M22 — the clause, and where its two copies live

```
$ grep -rn "THE_WALL_CLAUSE\|connects to it the normal way" src/
src/main.rs:1200:             connects to it the normal way stop being able to reach a cluster until someone on \
src/main_tests.rs:5542:                       kubectl and everything else that connects to it the normal way stop being \
src/main_tests.rs:5550:const THE_WALL_CLAUSE: &str = "connects to it the normal way";
src/main_tests.rs:6603:        EXPIRED.contains(THE_WALL_CLAUSE),
src/main_tests.rs:6617:        wall.contains(THE_WALL_CLAUSE),
src/main_tests.rs:6634:        !EXPIRING.contains(THE_WALL_CLAUSE),
```

The const is in the **test** tree. The product holds two literal copies of the clause — one in
`serving_certificate` (`main.rs:1200`) and one in `certificate_is_why`'s wall, pre-existing. The
three assertions are what couples them; the wall comparison flattens whitespace first, because the
wall is wrapped for a terminal block and the trailer is not.

## M23 — the corrected disambiguation, clause by clause against the code

| The page now says | Code | |
|---|---|---|
| with the setting **off**, an expired certificate refuses every sample's handshake, **and a session whose own two calls fail the same way** lands in the wall | `certificate_is_why` needs `Serving::Expired` **and** `version`/`served` both `Fault::Unanswered` | ✓ — this is the second gate the old passage omitted |
| this line can never join the wall — a run with no report has nothing to print into | the wall `return`s from `live()` before `render()` | ✓ |
| with the setting **on** the same certificate refuses nothing, so C2's expired **trailer** prints in the report this line prints in | `Serving::Until` is *"the soonest `notAfter` any sample read, over a handshake that **completed**"*; `Expired` is *"the `notAfter` rustls **refused** a sample over … a verifying kubeconfig never receives the bytes"* (`k8s.rs:8907-8924`). Knob on ⇒ completed ⇒ `Until(past)` ⇒ `left < 0` ⇒ expired arm | ✓ |
| this line and C2's expired **wall** can never share a run | the wall needs `Serving::Expired`, which needs a rustls refusal, which needs verification on, i.e. the knob off | ✓ |
| this line and C2's expired **trailer**, whenever the certificate warrants it, always do | scoped by the preceding *"With the setting on"*; the converse is not claimed, and § *A clean tally* now documents the knob-off HA path | ✓ |
| § *Stacked*: C2's expired **trailer** is not the wall and takes a slot in the order | slot 2 of `render()` | ✓ — the old *"expired-and-typed reading"* ambiguity is gone |

## M24 — the four cells of § *A clean tally*'s discriminator

The page's closing sentence there: *"§ When the connection was never verified is the trailer line
that tells this reader which one they are looking at."* Enumerated against `Serving::Until`'s
own *soonest sample* rule:

| Knob | Topology | `serving_expiry` | `unverified` line | Report |
|---|---|---|---|---|
| on | single server, expired | `Until(past)` | present | expired arm + unverified |
| off | HA, one replica expired | `Expired(at)`, session calls succeed | absent | expired arm only |
| **on** | **HA, one replica expired** | `Until(past)` — soonest sample | **present** | expired arm + unverified |
| off | single server, expired | `Expired(at)`, both calls fail | — | wall, exit 2 |

Row 3 has the line present and the HA cause.

## M25 — the trailer/caveat join on the live `--analysis` path

```rust
let mut block = render(&findings, &input);
if analysis {
    block.push('\n');
```

`main.rs:2754-2756`. `render()` returns `lines.join("\n")` with no trailing newline and `spaced`
puts a blank line *above* each block, so one `'\n'` here puts the `lists_were_read` caveat on the
line immediately below the last trailer line, where every other block boundary on the page is
`\n\n`. The line is not in this turn's diff.

## M26 — the mockup's line count, before and after the clause

`screens/once.md`'s two expired-certificate blocks, wrapped:

```
before: happens, kubectl and everything else stop being able to reach a
        cluster until someone on the control plane renews its certificate —
        not something k8rs can do.

after:  happens, kubectl and everything else that connects to it the normal
        way stop being able to reach a cluster until someone on the control
        plane renews its certificate — not something k8rs can do.
```

Three lines both times, so the 80×24 budget every mockup on that page is held to is unmoved.
