# The cluster picker, third operator read — measurements (2026-09-13)

`k8s-admin`, step 6 round three on the Phase 11 picker boxes, over the uncommitted
tree on `bafcd2c` after NOTES § D264 rulings 22–31. No cluster was used. Everything
ran in private copies: the working tree at `$HOME/k8s-admin-picker-r3`, `HEAD` via
`git archive` at `$HOME/k8s-admin-picker-r3-head`, one
`CARGO_TARGET_DIR=$HOME/k8s-admin-picker-r2-target`. `sha256sum -c` over the shared
tree's `src/*.rs` afterwards: every file OK. Scratch kubeconfigs and stand-in login
programs lived in the scratchpad (`<scratch>` below); their contents are not pasted.
Findings are in the report to the PM; this file holds what was run and what it printed.

## 1. `--once` against HEAD, every fault the pod watch can carry

One scratch test, byte-identical in both copies, calling `pods_unread` over
10 failures (`400 401 403 404 409 422 500`, a transport timeout, `AuthError` inside
`Service`, `Error::Auth`) × `unfinished` false/true × 4 coverages
(`Cluster`, `Asked`, `Refused`, `Blind("default")`) × renewal none/`aws`, plus the
four `ended` cases, each output written to a file; the two files compared by label.

```
$ cargo test --bin k8rs zz_admin_r3_once -- --nocapture      # in each copy
ZZ wrote 164 cases
164 164 True
differ: 48
   404 unfinished=false
   404 unfinished=true
   auth unfinished=false
   auth unfinished=true
   auth-service unfinished=false
   auth-service unfinished=true
every differing case is HEAD's bytes plus an appended tail: True
  tail: \n\n  Check the server address this kubeconfig names — what answered there is not a Kubernetes API server")
  tail: \n\n  Run that program yourself in a terminal to see what it says")
```

The renewal-less `NoCredential` case as `--once` now prints it:

```
=== auth unfinished=false Cluster renewal=None
...What happened: the program this kubeconfig logs in with gave k8rs nothing to sign in with\n\n  Run that program yourself in a terminal to see what it says")
```

## 2. A `server:` that is an HTTP server, with and without a path prefix

`python3 -m http.server --bind 127.0.0.1 18777`, static fake token, `server:`
`http://127.0.0.1:18777` and then `http://127.0.0.1:18777/k8s/clusters/c-m-abc12`.
Both runs printed the same tail (exit 2):

```
k8rs: this cluster did not show k8rs its pods, and every finding starts there, so there is nothing to report

  What k8rs asked for: pods across the whole cluster
  What happened: this server says there is no such thing when k8rs tries to `list` and `watch` pods

  Check the server address this kubeconfig names — what answered there is not a Kubernetes API server
```

What the stub was asked — the pod and version lines of the combined log of both runs, `sort | uniq -c`; every one of the prefixed run's nine requests carried the prefix:

```
      1 "GET /api/v1/pods?&limit=1 HTTP/1.1"
      1 "GET /k8s/clusters/c-m-abc12/api/v1/pods?&limit=1 HTTP/1.1"
      1 "GET /k8s/clusters/c-m-abc12/api/v1/pods?&limit=500 HTTP/1.1"
      2 "GET /k8s/clusters/c-m-abc12/version HTTP/1.1"
```

A namespace that does not exist is `200` with an empty `items` on a real API server:
`reports/2026-08-29-namespace-scope-under-a-real-role.md`, lines 312–315. A namespace
with a `/` in it never reaches a URL: `k8s::namespace_name` (`src/k8s.rs:4769`) gates
both `--namespace` and the context's own `namespace:` (`src/k8s.rs:6864-6910`).

## 3. `NoCredential` at connect: three kubeconfig shapes, and re-running the program

Server `http://127.0.0.1:9` (nothing listening). User entries: an `exec` naming a
stand-in `aws` with args `eks get-token --cluster-name payments-prod --output json`
and `env` `AWS_PROFILE`; a `tokenFile` naming a path that does not exist; an
`auth-provider` named `azure`. The stand-in prints `aws` CLI v2's usage without
`eks get-token`, *Unable to locate credentials* without `AWS_PROFILE`, and the SSO
expiry line otherwise.

```
--- exec: k8rs --once ---
exit=2
Error when retrieving token from sso: Token has expired and refresh failed
k8rs: no cluster to watch — the program this kubeconfig logs in with (`<scratch>/aws`) gave k8rs nothing to sign in with
--- tokenfile: k8rs --once ---
exit=2
k8rs: no cluster to watch — the program this kubeconfig logs in with gave k8rs nothing to sign in with
--- azure: k8rs --once ---
exit=2
k8rs: no cluster to watch — the program this kubeconfig logs in with gave k8rs nothing to sign in with
--- what "run that program yourself" runs, typed as the screen names it ---
usage: aws [options] <command> <subcommand> [<subcommand> ...] [parameters]
aws: error: the following arguments are required: command
exit=252
--- the same program with the args the kubeconfig names, without its env ---
Unable to locate credentials. You can configure credentials by running "aws configure".
exit=253
```

`kubectl get pods` over the same `exec` kubeconfig against an `http://` stub never ran
the stand-in (no SSO line on stderr; the stub saw 10 requests and kubectl printed
`Error from server (NotFound)`), so what kubectl shows for this shape was **not**
measured.

## 4. A login program re-run mid-session

A stand-in `exec` program that emits an `ExecCredential` expiring 30 s out, logs each
run, and from its second run on reads one line of stdin and writes it to stderr.
`--live` against `http.server` on 127.0.0.1:18779, five lines piped to k8rs's stdin,
`timeout 12`:

```
k8rs exit=124
plugin runs: 27   requests the stub saw: 24
--- k8rs stderr, plugin lines only ---
PLUGIN run 2 wrote to stderr; read from stdin: [keystroke-1]
PLUGIN run 3 wrote to stderr; read from stdin: [keystroke-2]
PLUGIN run 4 wrote to stderr; read from stdin: [keystroke-3]
PLUGIN run 5 wrote to stderr; read from stdin: [keystroke-4]
PLUGIN run 6 wrote to stderr; read from stdin: [keystroke-5]
PLUGIN run 7 wrote to stderr; read from stdin: [nothing]
(PLUGIN lines on k8rs stderr: 26)
```

Source: `kube-client-4.2.0/src/client/auth/mod.rs:208-222` refreshes inside the
request's `AsyncPredicate` when `now + 60s >= expiry`, via
`spawn_blocking(Auth::try_from)`, which reaches `auth_exec` (`:568`) and its
`stdin`/`stderr` inherit at `:587-593`. `k8s::connect_with` takes the `Kubeconfig`
by value (`src/k8s.rs:7273-7277`).

## 5. The failure box, drawn (80×24)

Scratch test `zz_admin_r3` in the copy's `src/ui_tests.rs`, through the file's own
`failure_box`, `picking`, `moved` and `picked`.

`NoCredential`, startup, nothing sent, no renewal (the `tokenFile` and `azure` shapes):

```
┌ staging could not be opened ─────────────────────────┐
│                                                      │
│  The program this kubeconfig logs in with gave k8rs  │
│  nothing to sign in with. Run that program yourself  │
│  in a terminal to see what it says                   │
│                                                      │
│  Nothing has connected yet — esc takes you back to   │
│  the list to try a different cluster.                │
│                                                      │
│               [ esc back to the list ]               │
│                                                      │
└──────────────────────────────────────────────────────┘
footer: │ esc back to the list                                                         │
```

`Refused`, `Blind("default")`, over a live switch:

```
┌ staging said no ─────────────────────────────────────┐
│                                                      │
│  The role this kubeconfig uses needs to `list` and   │
│  `watch` pods in the namespace default. This         │
│  kubeconfig names no namespace, so k8rs had to guess │
│  default and was refused there too. Quit and start   │
│  k8rs again in the namespace you work in: --namespace│
│  <name>                                              │
│                                                      │
│  Nothing is wrong with prod-eu — X takes you back.   │
│                                                      │
│                    [ esc dismiss ]                   │
│                                                      │
└──────────────────────────────────────────────────────┘
```

Every paragraph the box builds, checked whole (11 faults × `sent` × 7 coverages ×
4 way-outs × renewal none/named):

```
=== GRID3: 1232 boxes, 20 lose words ===
  Refused sent=true Asked("payments-settlement-eu") X arn …            (2)
  Refused sent=true Asked("payments-settlement-eu") startup …          (2)
  Refused sent=true Asked("payments-settlement-eu") startup arn …      (2)
  Refused sent=true Asked(<62-character namespace>) X / X arn / startup / startup arn …   (8)
  Refused sent=true Blind("payments-settlement-eu") X arn / startup / startup arn …       (6)
```

`Coverage::Blind` is only ever built with `FALLBACK_NAMESPACE` (`src/k8s.rs:6894`).

## 6. A typed filter

```
=== filter: startup '/stag' matching ===
│                 [ ⏎ connect ]    [ esc clear filter ]                │
footer: │ ↑↓ move  / filter  ⏎ connect  esc clear filter                               │
    esc -> quit = false, picker open with filter cleared = true
    after esc footer: │ ↑↓ move  / filter  ⏎ connect  esc quit                                       │
=== filter: X '/stag' matching ===
│                 [ ⏎ switch ]    [ esc clear filter ]                 │
footer: │ ↑↓ move  / filter  ⏎ switch  esc clear filter                                │
    after esc footer: │ ↑↓ move  / filter  ⏎ switch  esc cancel                                      │
=== filter: startup '/zzz' hides all ===
│  No context matches "zzz".                                           │
│                 [ ⏎ connect ]    [ esc clear filter ]                │
footer: │ / filter  esc clear filter                                                   │
=== filter: X after a failed switch, Dropped, '/stag' ===
footer: │ ↑↓ move  / filter  ⏎ switch  esc clear filter                                │
```

## 7. Two GKE default names, by terminal width

`gke_acme-payments-prod_europe-west1_autopilot-cluster-1` and
`gke_acme-payments-staging_europe-west1_autopilot-cluster-1`, servers on two
addresses from the IPv4 documentation range. X picker, widths 80 to 200.

```
=== GKE at 80 ===
│  ▸ …ope-west1_autopilot-cluster-1  ~gcp          (current)           │
│    …ope-west1_autopilot-cluster-1  ~gcp                              │
│                                                                      │
│  …autopilot-cluster-1  →  https://<documentation-range IPv4>         │
first width at which the rows differ at all: 85
│  ▸ …d_europe-west1_autopilot-cluster-1  ~gcp          (current)           │
│    …g_europe-west1_autopilot-cluster-1  ~gcp                              │
first width at which both project names are readable: 100
│  ▸ …me-payments-prod_europe-west1_autopilot-cluster-1  ~gcp          (current)           │
│    …payments-staging_europe-west1_autopilot-cluster-1  ~gcp                              │
```

## 8. Smaller checks

```
$ grep -ohE '^(pub )?const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs | wc -l
15
$ grep -n 'fn runtime_failure\|fn stdout_failure' src/main.rs
261:fn stdout_failure(error: &std::io::Error) -> Option<String> {
285:fn runtime_failure(error: &std::io::Error) -> String {
$ grep -n -- '--watch' screens/context.md | head -3
445:$ kubectl --context staging get pods -A --watch
527:│ $ kubectl --context staging get pods -A --watch   → not allowed    │
709:│ $ kubectl --context staging get pods -A --watch   → not allowed    │
```
