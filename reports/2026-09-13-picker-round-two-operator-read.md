# The cluster picker, second operator read — measurements (2026-09-13)

`k8s-admin`, step 6 round two on the Phase 11 picker boxes, over the uncommitted
tree on `bafcd2c`. No cluster was used. Everything ran in a private copy of the
working tree (`$HOME/k8s-admin-picker-r2`, `CARGO_TARGET_DIR=$HOME/k8s-admin-picker-r2-target`);
the shared tree was compared file by file against that copy afterwards and had not
moved. Findings and severities are in the report to the PM; this file holds what
was run and what it printed.

## 1. A login program runs with k8rs's own stdin and stderr

Kubeconfig (scratch, not pasted): one context, `server:` on 127.0.0.1 port 9 (nothing
listening), and a user whose `exec` block has `command: sh`, no `interactiveMode`,
and args that read one line from stdin, echo it to stderr, and exit 255.

```
$ printf 'bytes-meant-for-the-tui\n' | KUBECONFIG=<scratch kubeconfig> k8rs --once > exec.stdout 2> exec.stderr
exit=2
--- stderr ---
PLUGIN-STDERR: Token has expired and refresh failed (read from stdin: bytes-meant-for-the-tui)
k8rs: no cluster to watch — the program this kubeconfig logs in with (`sh`) gave k8rs nothing to sign in with
--- stdout ---
```

The source line that decides it, `kube-client-4.2.0/src/client/auth/mod.rs:587-593`:

```
let interactive = auth.interactive_mode != Some(ExecInteractiveMode::Never);
if interactive {
    cmd.stdin(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());
```

and `:634`, `let out = cmd.output()` — synchronous, reached from `Client::try_from`
inside `k8s::connect_with` (`src/k8s.rs` `match Client::try_from(config)`).

## 2. A `server:` that is an HTTP server but not an API server

`python3 -m http.server --bind 127.0.0.1 18777` under `timeout 45`, and a scratch
kubeconfig whose `server:` is `http://127.0.0.1:18777` with a static token (not pasted).

```
$ KUBECONFIG=<scratch kubeconfig> k8rs --once < /dev/null
exit=2
k8rs: watching — could not read the server version (this server says there is no such thing when k8rs tries to `get /version`) · could not list what this cluster serves, so k8rs cannot show you what is in it or tell which add-ons it has (this server says there is no such thing when k8rs tries to `get /apis`)
k8rs: this cluster did not show k8rs its pods, and every finding starts there, so there is nothing to report

  What k8rs asked for: pods across the whole cluster
  What happened: this server says there is no such thing when k8rs tries to `list` and `watch` pods
```

What the server logged (first two requests):

```
"GET /api/v1/pods?&limit=1 HTTP/1.1" 404 -
"GET /version HTTP/1.1" 404 -
```

`kube-client-4.2.0/src/client/mod.rs:551-558`: a body that does not parse as a
`Status` is rebuilt as `Status::failure(&text, …).with_code(status.as_u16())`, so
the HTTP code survives into `k8s::answer` (`404 => Fault::Gone`).

## 3. Context names at the 80-column floor, as the picker draws them

Scratch test `zz_admin_r2` / `zz_admin_r2b` appended to the copy's `src/ui_tests.rs`,
`cargo test --bin k8rs zz_admin_r2 -- --nocapture`, exit 0. List rows only; the
server lines are left out.

Three `aws eks update-kubeconfig` names (`…:cluster/payments-dev|prod|staging`):

```
│    …22223333:cluster/payments-dev  ~aws                              │
│  ▸ …2223333:cluster/payments-prod  ~aws          (current)           │
│    …3333:cluster/payments-staging  ~aws                              │
```

Two GKE names, `gke_acme-payments-prod_europe-west1_autopilot-cluster-1` and
`gke_acme-payments-staging_europe-west1_autopilot-cluster-1`:

```
│  ▸ …ope-west1_autopilot-cluster-1  ~gcp          (current)           │
│    …ope-west1_autopilot-cluster-1  ~gcp                              │
```

Two `oc login` names, `payments/api-ocp-prod-acme-corp-com:6443/kube:admin` and
`payments/api-ocp-stg-acme-corp-com:6443/kube:admin`:

```
│  ▸ …acme-corp-com:6443/kube:admin                (current)           │
│    …acme-corp-com:6443/kube:admin                                    │
```

The same two `oc login` names at 100 columns:

```
│  ▸ …yments/api-ocp-prod-acme-corp-com:6443/kube:admin                (current)           │
│    payments/api-ocp-stg-acme-corp-com:6443/kube:admin                                    │
```

The failure box's way out over the GKE pair:

```
│  Nothing is wrong with …ope-west1_autopilot-cluster-1│
│  — X takes you back.                                 │
```

## 4. The failure box over every fault, `sent`, coverage and way out

Same scratch test. 11 faults × `sent` true/false × 6 coverages × 4 way-outs
(short name, EKS name, startup, startup with EKS name) × renewal none/named, and
for each, whether the box holds the whole paragraph `failed` builds:

```
=== GRID2: 1056 boxes, 14 lose words ===
  Refused sent=true Asked("payments-settlement-eu") X arn …
  Refused sent=true Asked("payments-settlement-eu") startup …
  Refused sent=true Asked("payments-settlement-eu") startup arn …
  Refused sent=true Asked(<62-character namespace>) X / X arn / startup / startup arn …
```

The 22-character namespace at startup:

```
│  The role this kubeconfig uses needs to `list` and   │
│  `watch` pods in the namespace                       │
│  payments-settlement-eu. Ask whoever runs this       │
│  cluster for a role that may read pods in            │
│  payments-settlement-eu — the same rules as          │
│  `k8rs-readonly` in the k8rs docs, granted in one…   │
```

## 5. A typed filter that still matches, at startup

```
│  ▸ staging                                                           │
footer: │ ↑↓ move  / filter  ⏎ connect  esc quit                                       │
    esc -> quit = false, picker still open with filter cleared = true
```

## 6. The move out of `main.rs`

Code of `because` (comments dropped, `k8s::Fault::` normalised) at `HEAD:src/main.rs`
against `src/views.rs`, and the string literals of `pods_unread`'s scope and next
step against `views::scope` + `views::next_step`:

```
because body identical (code, comments dropped): True
HEAD literals: True
6 6
```

Test files that pin a whole or partial sentence, at HEAD:

```
granted in one namespace                 HEAD:
is that role                             HEAD:
gave k8rs nothing to sign in with        HEAD:
renew it there                           HEAD:
something else changed this object       HEAD:
could not be read — it is missing        HEAD:
had to guess                             HEAD: src/main_tests.rs=1
```

## 7. The flag count

```
$ git show HEAD:src/main.rs | grep -oE '^const [A-Z_]+: &str = "--[a-z-]+"' | wc -l
15
$ grep -oE '^const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs | wc -l
14
$ grep -ohE '^const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs | wc -l
14
$ grep -ohE '^(pub )?const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs | wc -l
15
```
