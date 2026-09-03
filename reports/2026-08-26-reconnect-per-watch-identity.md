# Per-watch identity and the driver — operator review measurements (2026-08-26)

Step 6 review of the Phase 5 reconnect box (`Watch::failure` / `Watch::ended`,
`Trouble`, `Store::troubles`, `updates`, `drive`). Every claim below is read off
the vendored crate sources on this machine or off the repo tree at the reviewed
working-tree state. No conclusions here — see the findings this file is evidence
for.

## The cluster slot was occupied, so nothing was measured against a cluster

```
$ kind get clusters
k8rs
exit=0
```

The PM's fixture cluster is up. `CLAUDE.md` § The one hard rule of concurrency:
one cluster at a time, and a capture and a review measurement never run at once.
No `K8RS_CLUSTER=review` cluster was created. Everything below is source, not
cluster.

## Vendored crate paths read

```
$ ls -d ~/.cargo/registry/src/*/kube-runtime-4.2.0
/home/shyuuhei/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/kube-runtime-4.2.0
```

## The state-machine citations in D162, re-read line by line

Command shape used throughout:

```
$ sed -n '<range>p' .../kube-runtime-4.2.0/src/watcher.rs
```

| Cited | What is actually on the line | Verdict |
|---|---|---|
| `watcher.rs:201-202` | `#[default]` / `ListWatch,` in `enum InitialListStrategy` | correct |
| `watcher.rs:523` | `InitialListStrategy::ListWatch => (Some(Ok(Event::Init)), State::InitPage {` | correct |
| `watcher.rs:548` | `return (Some(Ok(Event::InitApply(next))), State::InitPage {` | correct |
| `watcher.rs:555-559` | `if continue_token.is_none() … return (Some(Ok(Event::InitDone)), State::InitListed { … })` | correct |
| `watcher.rs:568` | `return (Some(Err(Error::NoResourceVersion)), State::Empty);` | correct |
| `watcher.rs:584` | `(Some(Err(Error::InitialListFailed(err))), State::Empty)` | correct |
| `watcher.rs:624-630` | `Some(Err(err)) => … (Some(Err(Error::WatchFailed(err))), State::InitialWatch { stream })` | correct |
| `watcher.rs:650-652` | `(Some(Err(Error::WatchStartFailed(err))), State::InitListed { resource_version, })` | correct |
| `watcher.rs:709` | `(Some(Err(Error::WatchFailed(err))), State::Watching { resource_version, stream, })` | correct |
| `kube-core-4.2.0/src/params.rs:378` | `qp.append_pair("watch", "true");` | correct |
| `kube-core-4.2.0/src/params.rs:381` | `qp.append_pair("timeoutSeconds", &self.timeout.unwrap_or(290).to_string());` | correct |
| `NOTES.md` D162 → `k8s_tests.rs:1146` as "pins ListWatch" | line 1146 is `Ok(Event::Init),` inside `a_relist_in_flight_does_not_withdraw_the_failure_it_is_answering`; the `ListWatch` pin is `kube_still_pages_the_initial_list_at_the_number_this_repo_chose`, `k8s_tests.rs:1350-1353` | wrong line |

## The watch phase has a deadline, and it is 295 s

```
$ sed -n '483,505p' .../kube-runtime-4.2.0/src/watcher.rs
483:const WATCH_IDLE_TIMEOUT_MARGIN: Duration = Duration::from_secs(5);
490:async fn next_with_idle_timeout<S, T>(stream: &mut S, timeout: Option<u32>) -> Option<T>
494:    let idle_timeout = Duration::from_secs(u64::from(timeout.unwrap_or(290))) + WATCH_IDLE_TIMEOUT_MARGIN;
495:    match tokio::time::timeout(idle_timeout, stream.next()).await {
496:        Ok(item) => item,
497:        Err(_elapsed) => { … "watch stream idle timeout, reconnecting" … None }
```

Call sites: `watcher.rs:589` (`State::InitialWatch`) and `watcher.rs:659`
(`State::Watching`). `watcher::Config::default().timeout` is `None`
(pinned by `k8s_tests.rs:1356-1358`), so the effective deadline is 290 + 5 s.

On elapse the arm is `None => (None, State::InitListed { resource_version })`
(`watcher.rs:714`) — no `Err` is emitted, so nothing reaches `Watch::failure`.

## The initial LIST has no deadline at all — `ListParams.timeout` is never sent

```
$ sed -n '93,122p' .../kube-core-4.2.0/src/params.rs      # ListParams::populate_qp
```

Appends, in order: `fieldSelector`, `labelSelector`, `limit`, then `continue`
**or** `resourceVersion` + `resourceVersionMatch`. `timeoutSeconds` appears
nowhere in the list builder, while `WatchParams::populate_qp` (`:381`) does
append it.

`ListParams.timeout`'s own doc comment (`params.rs:135-139`) says
"Configure the timeout for list/watch calls … Defaults to 290s" — the field is
copied by `watcher::Config::to_list_params` (`watcher.rs:398`) and then dropped
by the query builder.

## `watcher()` cannot end; `StreamBackoff` can end it

```
$ sed -n '787,798p' .../kube-runtime-4.2.0/src/watcher.rs
pub fn watcher<K: …>(api: Api<K>, watcher_config: Config) -> impl Stream<Item = Result<Event<K>>> + Send {
    futures::stream::unfold((api, watcher_config, State::default()), |(api, watcher_config, state)| async {
        let (event, state) = step(&FullObject { api: &api }, &watcher_config, state).await;
        Some((event, (api, watcher_config, state)))
    })
}
```

The closure returns `Some(..)` unconditionally.

```
$ sed -n '9,15p' .../kube-runtime-4.2.0/src/utils/stream_backoff.rs
/// Applies a [`Backoff`] policy to a [`Stream`]
/// After any [`Err`] is emitted, the stream is paused for [`Backoff::next_backoff`]. The
/// [`Backoff`] is [`reset`](`Backoff::reset`) on any [`Ok`] value.
/// If [`Backoff::next_backoff`] returns [`None`] then the backing stream is given up on, and closed.
```

`DefaultBackoff` (`watcher.rs:981-988`) is
`ResetTimerBackoff::new(ExponentialBackoff::new(800 ms, 30 s, 2.0, jitter), 120 s)`,
and `ExponentialBackoff::new` calls `.without_max_times()` (`watcher.rs:930`),
so `DefaultBackoff::next()` never yields `None`. A caller-supplied
`backon::ExponentialBuilder` with `max_times` set does (`From` impl at
`watcher.rs:959`).

## What kube says about looping, and what this tree applies

```
$ sed -n '20,27p' .../kube-runtime-4.2.0/src/watcher.rs
/// Errors that a watcher can emit
/// These are all considered retryable from a watcher's point of view,
/// even though they may require patching of rbac/netpols in the background to fix.
/// To avoid constantly looping errors, make sure backoff is applied.

$ sed -n '776,779p' .../kube-runtime-4.2.0/src/watcher.rs
/// The stream will attempt to be recovered on the next poll after an [`Err`] is returned.
/// This will normally happen immediately, but you can use [`StreamBackoff`] …
```

```
$ grep -rn "default_backoff\|StreamBackoff\|\.backoff(" src/
(no matches)
```

`src/k8s.rs:1477-1483` records that kube's *client-side* `default_retry` is left
on; that layer covers 429/503/504 only (`kube-client-4.2.0/src/client/retry.rs:114-119`)
and is a different mechanism from the watcher-stream backoff above.

## What `watcher::Error` can transitively hold

```
$ sed -n '28,45p' .../kube-client-4.2.0/src/error.rs
    #[error("ApiError: {0} ({0:?})")]        Api(#[source] Box<Status>),
    #[error("auth error: {0}")]              Auth(#[source] crate::client::AuthError),   # :101-105

$ sed -n '54,63p' .../kube-client-4.2.0/src/client/auth/mod.rs
    /// Failed to run auth exec command
    #[error("auth exec command '{cmd}' failed with status {status}: {out:?}")]
    AuthExecRun {
        /// The failed command
        cmd: String,
        /// The exit status or exit code of the failed command
        status: std::process::ExitStatus,
        /// Stdout/Stderr of the failed command
        out: std::process::Output,
    },

$ sed -n '634,642p' .../kube-client-4.2.0/src/client/auth/mod.rs
    let out = cmd.output().map_err(Error::AuthExecStart)?;
    if !out.status.success() {
        return Err(Error::AuthExecRun { cmd: format!("{cmd:?}"), status: out.status, out });
    }
    parse_exec_credentials(&out.stdout)
```

`cmd` is the `Debug` of the whole `std::process::Command`, i.e. program plus
every argument. `out` is `std::process::Output`, whose std `Debug` renders
`stdout` and `stderr` as strings when they are valid UTF-8. The exec-credential
plugin writes an `ExecCredential` JSON containing `status.token` to stdout.

The layer is per-request: `kube-client-4.2.0/src/client/builder.rs:208`
(`let auth_layer = config.auth_layer()?;`) and `:252` (`.option_layer(auth_layer)`).

`oauth` / `oidc` are not enabled — `Cargo.toml:43-47` names `client`, `runtime`,
`rustls-tls` with `default-features = false` — so `AuthError::OAuth` and
`AuthError::Oidc` are not compiled. `AuthExecRun`, `AuthExecParse`, `AuthExec`
and `ReadTokenFile` are, under `client`.

## What the automated guard covers here

```
$ sed -n '28,36p' scripts/security-guard.py
4. **Token hygiene** — a type that can hold a kube `Config`/`Client` may not
   *derive* `Debug` … The taint follows a *field type spelled `Client`* into the
   struct that owns it, to a fixpoint …
```

`src/k8s.rs:837` `#[derive(Debug)] pub struct Trouble<'a>` holds
`Option<&'a watcher::Error>`; no field is spelled `Client`, so the taint does not
reach it and `just check` is green on this rule.

## What should be measured on a cluster, and by whom

None of the below was run. They are the halves source-reading cannot settle.

1. **A 403 on one of the five watched kinds.** A `Role`-bound (namespace-scoped)
   kubeconfig cannot be granted `nodes`, which is cluster-scoped. Wanted: the
   `Status` body of `GET /api/v1/nodes` under that identity — specifically
   whether `details.group` / `details.kind` are populated (D160 measured them
   *empty* for a `nonResourceURL` refusal), and whether the verb appears anywhere
   but in `message`. This decides whether a caller can build
   "this kubeconfig may not watch nodes" from a `Trouble` without parsing prose.
2. **The request rate of a permanently-refused watch.** With no `StreamBackoff`,
   count `LIST` requests in the apiserver audit log for 60 s against one watch
   whose list is 403'd. The predicted shape from source is a hot loop bounded
   only by round-trip time.
3. **A 410 desync on a real busy cluster**, to confirm it arrives as
   `WatchEvent::Error{code:410}` → `watcher::Error::WatchError(Box<Status>)`
   from `State::Watching` rather than as `InitialListFailed`, which is the
   variant the synthetic test in `k8s_tests.rs:1136-1147` feeds.
4. **A rolling apiserver restart**, to confirm the 295 s idle deadline is what
   actually recovers a severed watch, and that no `Err` is observed in that
   window.

All four need a cluster and produce no committed artifact; they are
`k8s-admin`'s under `K8RS_CLUSTER=review`, once the fixture cluster's slot is
free — except that 1 and 2 also need a `Client`, which no line in this build
constructs, so they belong to the `connect()` box's done-when (NOTES § D161).
