# 2026-08-27 — the six-`Fault` family against a live API server

Operator review of the uncommitted Phase 5 turn (`Fault` · `answer` · `fault` ·
`NotConnected::fault`/`renewal` · `Trouble::fault` · `Session::renewal` ·
`because`/`greeting`/`runtime_failure`). Every line below was produced by the
built binary or by `curl`, on this machine, on 2026-08-27.

## What was and was not brought up

**No cluster was created and none was torn down.** The PM's fixture cluster
`k8rs` (kind, `kindest/node:v1.36.1`, 1 control-plane + 3 workers) was already
running when this review started, and
[CLAUDE.md § The one hard rule of concurrency](../CLAUDE.md#the-one-hard-rule-of-concurrency)
allows one cluster at a time, so a second one was not started. Everything here
is an **unauthenticated or unauthorised read** against that running server from
kubeconfigs written in the reviewer's scratchpad: no object was created,
modified or deleted, no RBAC was changed, `scripts/cluster.sh` was never
invoked, and no file under `tests/` was written.

Kubeconfigs used, all built in the scratchpad and none pasted here:

| name | user block | what it produces |
|---|---|---|
| `anon` | empty | `system:anonymous` — `/version` allowed, everything else `403` |
| `badtoken` | a made-up inline bearer string | `401` on every call |
| `selfcert` | a self-signed client certificate the cluster CA never signed | `401` on every call |
| `exec` | an `exec` block naming a script the reviewer wrote | answers once, then exits non-zero |
| `dead-server` | admin, server port rewritten to `:9` | nothing listening |
| `missingcert` | `client-certificate` naming a path that does not exist | fails before any request |
| `nourl` | a cluster entry with no `server` | fails before any request |
| `proxy` | plain HTTP to a local forwarder | `list` forwarded, `?watch=true` refused `403` |
| `evil` | an `exec` `command` carrying ESC, U+202E and 4000 padding characters | fails before any request |

## 1 — a live `403`, including the `nonResourceURL` one

```
$ curl -s --cacert ca.crt -w 'HTTP %{http_code}' https://127.0.0.1:6443/apis
reason: "Forbidden"   code: 403   details: {}          HTTP 403
$ curl -s --cacert ca.crt -w 'HTTP %{http_code}' https://127.0.0.1:6443/api/v1/pods
reason: "Forbidden"   code: 403   details: {"kind":"pods"}   HTTP 403
$ curl -s --cacert ca.crt -w 'HTTP %{http_code}' https://127.0.0.1:6443/version
HTTP 200
```

`details` on the `/apis` refusal is `{}` — NOTES § D160's measurement,
re-measured on a cluster that still has its `system:discovery` binding, because
`system:anonymous` is outside `system:authenticated`.

```
$ KUBECONFIG=anon.kubeconfig timeout 20 target/debug/k8rs --live
stderr: k8rs: watching — server v1.36.1 · could not list what this cluster serves,
        so k8rs cannot show you what is in it or tell which add-ons it has
        (this kubeconfig is not allowed to `get /apis`)
stdout: ▲ k8rs is not getting pods from this cluster: this kubeconfig is not allowed
        to `list` and `watch` pods. It keeps asking, and until that works nothing
        here about them can be trusted
        (…the same line for nodes, Deployments, statefulsets, daemonsets)
exit: 124 (killed by timeout — it never stops on its own)
```

Five trouble lines arrive one at a time, and the whole report is reprinted each
time one lands: 5 blocks in the first ~4 seconds of a 20-second run.

## 2 — a live `401`, from two different causes

```
$ KUBECONFIG=badtoken.kubeconfig timeout 15 target/debug/k8rs --live
k8rs: watching — could not read the server version (this cluster no longer accepts
      this login — this kubeconfig needs a new one) · could not list what this
      cluster serves … (same clause)
```

```
$ curl -s --cacert ca.crt --cert self.crt --key self.key https://127.0.0.1:6443/api/v1/pods
message: "Unauthorized"   reason: "Unauthorized"   code: 401   HTTP 401
$ KUBECONFIG=selfcert.kubeconfig timeout 10 target/debug/k8rs --live
… this cluster no longer accepts this login — this kubeconfig needs a new one …
```

A client certificate the cluster will not accept is rejected by the
**authenticator**, not by the TLS handshake: it arrives as `Error::Api` with
`code: 401`, so it classifies `Fault::Expired` and not `Fault::Unanswered`.

## 3 — an `exec` plugin that dies at connect, and the same one dying mid-session

One kubeconfig, one script, two runs. The script answers a well-formed
`ExecCredential` (a made-up bearer string plus an expiry 65 seconds out) on its
first invocation and exits 1 on every later one.

```
$ rm -f plugin.count; KUBECONFIG=exec.kubeconfig timeout 25 target/debug/k8rs --live
first blocks:  … this cluster no longer accepts this login — it comes from
               `…/acme-cloud-login`, so renew it there …
last blocks:   ▲ k8rs is not getting pods from this cluster: nothing usable came
               back when k8rs tried to `list` and `watch` pods. …
plugin invocations during the run: 13
exit: 124
```

```
$ echo pre-seeded > plugin.count      # the script now fails on its first call
$ KUBECONFIG=exec.kubeconfig timeout 10 target/debug/k8rs --live
k8rs: no cluster to watch — the program this kubeconfig logs in with
      (`…/acme-cloud-login`) gave k8rs nothing to sign in with
exit: 2
```

Same plugin, same failure, same cluster: `Fault::NoCredential` before the
session exists, `Fault::Unanswered` after it exists.

Source read for the mechanism (kube 4.2.0, exactly as vendored in
`~/.cargo/registry`):

- `kube-client/src/client/auth/mod.rs:200-205` — `AsyncPredicate::check` ends
  `refreshable.to_header().await.map_err(Into::into)`, and the target is
  `tower::BoxError`, so what is boxed is `auth::Error`.
- `tower-0.5.3/src/filter/future.rs` — `AsyncResponseFuture::poll` propagates the
  predicate's error with `?` and no wrapping.
- `kube-client/src/client/mod.rs:222-233` — `Client::send` maps the tower error
  with `err.downcast::<Error>()`, where `Error` is `kube::Error`; the boxed value
  is `auth::Error`, so the downcast misses, the `hyper::Error` downcast misses,
  and `unwrap_or_else(Error::Service)` runs.
- `kube-client/src/error.rs:27-28` — `Service(#[source] tower::BoxError)`.
- `kube-client/src/client/auth/mod.rs:177` — "the visibility must be `pub` for
  `impl Layer for AuthLayer`, but this is not exported from the crate. It's not
  accessible from outside".

`auth::Error`'s own `Display` chain is the one `Trouble::failure`'s doc quotes:
`AuthExecRun { cmd, status, out }` renders `out: {out:?}`, which is the plugin's
stdout.

## 4 — `list` allowed, `watch` refused

A local forwarder in the scratchpad passed every request through to the API
server with an administrative identity and answered `?watch=true` with a real
`403` `Status` (`reason: "Forbidden"`, `code: 403`, `details: {"kind":"pods"}`).

```
$ KUBECONFIG=proxy.kubeconfig timeout 20 target/debug/k8rs --live
▲ k8rs is not getting pods from this cluster: this kubeconfig is not allowed to
  `list` and `watch` pods. It keeps asking, and until that works nothing here
  about them can be trusted
… and, in the same output: 40 pods · 4 nodes, with the full card report.
```

Forwarder log over the 20-second run:

```
total requests 76 · non-watch 53 · watch attempts 23
/api/v1/pods 1 · /api/v1/nodes 1 · /version 2 · /api 2 · /apis 22
```

One LIST per kind for the whole run: a refused *watch* does not re-LIST
(`kube-runtime/src/watcher.rs:650-652` returns to `State::InitListed`). The 22
`/apis` are an artefact of the forwarder rewriting `Accept`, which pushes
discovery onto the legacy per-group path; 60 kinds still came back through it.

## 5 — everything that fails before a request is sent

```
$ KUBECONFIG=<file that does not exist>   → k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
$ --context definitely-not-here           → (identical line)
$ kubeconfig with no current-context      → (identical line)
$ --context ""                            → (identical line)
$ client-certificate: /no/such/dir/…      → (identical line)
$ cluster entry with no server url        → (identical line)
```

All six exit 2 and none panics. `kube::config::KubeconfigError`
(`kube-client/src/config/mod.rs:35-109`) has 19 variants, among them
`CurrentContext`, `LoadContext(name)`, `FindPath`, `ReadConfig(io, path)`,
`Parse`, `MissingClusterUrl`, `ParseClusterUrl`, `LoadClientCertificate`,
`LoadClientKey`.

```
$ dead-server.kubeconfig (nothing listening on the port)
k8rs: watching — could not read the server version (nothing usable came back when
      k8rs tried to `get /version`) · could not list what this cluster serves …
      (nothing usable came back when k8rs tried to `get /apis`)
▲ k8rs is not getting pods … nothing usable came back when k8rs tried to `list`
  and `watch` pods …
exit: 124 — the run does not end
```

A context name containing a space works both as `--context "my ctx"` and as the
file's own `current-context`.

`k8rs --live --context` with nothing after it connects to the kubeconfig's
current context and prints no line about the flag; `k8rs --live --context ""`
is refused.

## 6 — the healthy run

```
$ KUBECONFIG=admin.kubeconfig timeout 25 target/debug/k8rs --live
stderr: k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
stdout: 40 pods · 4 nodes  + the card report, printed once
lines matching "k8rs is not getting" or "k8rs has stopped receiving": 0
```

## 7 — invariant 9 on the one new string a screen may print

`Session::renewal` is the only string this turn adds to a sentence. Fed an
`exec` `command` of 4030 characters containing `ESC[31m`, `ESC[0m` and U+202E:

```
$ KUBECONFIG=evil.kubeconfig timeout 8 target/debug/k8rs --live
output 641 characters · ESC present: False · U+202E present: False
prefix: k8rs: no cluster to watch — the program this kubeconfig logs in with (`/no/such/[31mRED[0mknp.txt-AAAA…
tail:   …AAA… (shortened by k8rs)`) gave k8rs nothing to sign in with
```

## 8 — `answer()` against codes it is not fed

`answer` matches `status.code` on 401/403/404 and falls through to
`status.reason` on `UNAUTHORIZED`/`FORBIDDEN`/`NOT_FOUND`.
`kube-runtime/src/watcher.rs:610-622` emits `Err(Error::WatchError(status))`
for a watch-stream error and only then re-lists when `err.code == 410`; a
Kubernetes `410` for a stale `resourceVersion` carries `reason: "Expired"` —
`kube-core/src/response.rs:390` is `pub const EXPIRED: &str = "Expired"`, and
`:288` is a separate `GONE: &str = "Gone"` — so neither the code arm nor the
reason arm of `answer` matches a `410`.

## What could not be measured here

- A watch-verb `403` from **real RBAC** rather than a forwarder: it needs a
  `Role` and a `RoleBinding`, which is a write into the PM's fixture cluster.
- A `410` desync from a real API server: it needs etcd compaction or a watch
  cache eviction, neither of which is producible on demand in a 4-node kind
  cluster inside a review.
- A `403` from a proxy answering a JSON body that is not a `Status`: already
  measured by the author on 2026-08-27 against a local listener; not repeated.
- Request rate under a standing refusal: measured by the author
  (`reports/2026-08-27-connect-and-the-idle-proof.md`); not repeated.
