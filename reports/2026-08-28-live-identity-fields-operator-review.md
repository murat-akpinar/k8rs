# The three live identity fields, measured through the binary (2026-08-28)

Ephemeral measurements taken by `k8s-admin` during the operator review of the
box that wires `server_version`, `context` and `client_certificate` into
`Store::snapshot`. Read-only against the PM's live fixture cluster `k8rs`
(context `kind-k8rs`, 4 nodes, 40 pods, one worker cordoned). **No second
cluster was created** and nothing was written to this one.

Binary under test: `cargo build --release` of the working tree at review time.

## 1 — the live admin certificate, so C1's silence can be judged

```
$ kubectl config view --raw -o jsonpath='{.users[0].user.client-certificate-data}' \
    | base64 -d > admin.crt
$ openssl x509 -in admin.crt -noout -dates -subject -nameopt RFC2253
notBefore=Aug 22 15:45:16 2026 GMT
notAfter=Aug 22 15:50:16 2027 GMT
subject=CN=kubernetes-admin,O=kubeadm:cluster-admins
$ wc -c admin.crt
1155 admin.crt
```

Days left at the time of the run: **359**. `CERT_EXPIRY_WARN` is 30 days.
The PEM is **1155 bytes**, which is the figure `CERTIFICATE_BYTES`' doc cites.

Which auth fields the live kubeconfig carries (names only, no values):

```
$ kubectl config view --raw -o json | python3 -c "
import json,sys
c=json.load(sys.stdin)
for u in c.get('users',[]): print('user:', u['name'], 'fields:', sorted(u['user'].keys()))
for x in c.get('contexts',[]): print('context:', x['name'], 'keys:', sorted(x['context'].keys()))"
user: kind-k8rs fields: ['client-certificate-data', 'client-key-data']
context: kind-k8rs keys: ['cluster', 'user']
```

## 2 — `--live --analysis` against the live cluster, unmodified kubeconfig

```
$ timeout 20 ./target/release/k8rs --live --analysis > live-analysis.out 2>err
$ cat err
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
$ grep -n '^\[' live-analysis.out
84:[capacity]   95:[certificates]   100:[drain safety]   105:[posture]
135:[restarts]  220:[waste]         234:[versions]
```

Versions, verbatim:

```
[versions]
  What version everything here is running
  Versions
  Control plane v1.36.1 · 4 of 4 kubelets match
  Every machine is running the same version as the control plane. Nothing to do.
```

Certificates, verbatim — no badge on the label line, no C1 row:

```
[certificates]
  What expires, soonest first
  Machines waiting to join are not checked. Seeing them takes a cluster-wide list of joining requests, and k8rs does not have one.
  Ask for permission to list certificatesigningrequests across the whole cluster.
```

Node versions the `4 of 4` counts against:

```
$ kubectl get nodes -o custom-columns=NAME:.metadata.name,V:.status.nodeInfo.kubeletVersion --no-headers
k8rs-control-plane   v1.36.1
k8rs-worker          v1.36.1
k8rs-worker2         v1.36.1
k8rs-worker3         v1.36.1
```

## 3 — C1's row and badge drawn against the live cluster, read-only

A kubeconfig was written in the session scratchpad naming the same live server.
Its user block holds:

* a `client-certificate` **path** pointing at a throwaway self-signed certificate
  generated locally with `openssl req -x509 -days 9` (`CN=k8rs-review-decoy`,
  notAfter `Sep 5 23:33:19 2026 GMT`, 1135 bytes) — never committed, deleted
  after the run;
* **no** client key;
* an `exec` block whose command is a local script that prints an `ExecCredential`
  carrying the identity the live kubeconfig already holds.

No CertificateSigningRequest was created, no CA key was read off a node, and
nothing in the cluster was changed.

`kubectl` refuses this kubeconfig outright:

```
$ env KUBECONFIG=$PWD/kc-exec.yaml kubectl get nodes --no-headers
error: client-key-data or client-key must be specified for u to use the clientCert authentication method.
exit=1
```

`k8rs` connects and reports on the file:

```
$ timeout 15 env KUBECONFIG=$PWD/kc-exec.yaml ./target/release/k8rs --live --analysis
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
40 pods · 4 nodes
...
○ prod-eu-west-1
  Your kubeconfig certificate expires in 8 days
  valid until 2026-09-05T23:33:19Z · this is the file on your own machine that proves who you are — nothing in the cluster is broken
  → ask whoever gave you access for a new kubeconfig before that date — k8rs cannot renew it, and after it kubectl stops working for you too

13 critical, 3 warnings, 1 note
...
[certificates] 8d
  What expires, soonest first
  ▲ Your kubeconfig certificate expires in 8 days
      valid until 2026-09-05T23:33:19Z · this is the file on your own machine that proves who you are — nothing in the cluster is broken
      → ask whoever gave you access for a new kubeconfig before that date — k8rs cannot renew it, and after it kubectl stops working for you too
```

The session's actual TLS identity in that run is the one the `exec` block
returned, whose notAfter is the 2027 date in § 1.

Same kubeconfig with a `client-key` added beside the certificate (a well-formed
static pair, still shadowed by the `exec` block):

```
$ timeout 15 env KUBECONFIG=$PWD/kc-pair-exec.yaml ./target/release/k8rs --live --analysis
○ pair-exec-ctx
  Your kubeconfig certificate expires in 8 days
...
  ▲ Your kubeconfig certificate expires in 8 days
```

Same certificate path with **no** `exec` block and no key:

```
$ timeout 15 env KUBECONFIG=$PWD/kc-nokey.yaml ./target/release/k8rs --live --analysis
k8rs: no cluster to watch — this kubeconfig loaded, and something it points at did not — a certificate file it names, a `server:` line, or a cluster one of its contexts refers to
exit=2
```

## 4 — resolution order

`--context` against a file whose current context is a different name:

```
$ timeout 15 env KUBECONFIG=$PWD/kc-exec.yaml ./target/release/k8rs --live --analysis --context staging-eu-west-1
○ staging-eu-west-1
  Your kubeconfig certificate expires in 8 days
```

Two files on `KUBECONFIG`. The first names only a context; the second holds the
cluster, the user, its own current context, and a **relative**
`client-certificate` (`decoy.crt`, sitting beside that second file):

```
$ env KUBECONFIG=$PWD/merge-a.yaml:$PWD/mergedir/b.yaml kubectl config current-context
first-file-ctx
$ timeout 15 env KUBECONFIG=$PWD/merge-a.yaml:$PWD/mergedir/b.yaml ./target/release/k8rs --live --analysis
○ first-file-ctx
  Your kubeconfig certificate expires in 8 days
  valid until 2026-09-05T23:33:19Z · ...
```

kube's own rules, read from
`kube-client-4.2.0/src/config/file_config.rs`:

* `Kubeconfig::read()` → `from_env()` folds every `KUBECONFIG` path with
  `merge`, and `merge` is `self.current_context.or(next.current_context)` — first
  file wins (`:568`).
* `read_from` rewrites `client_certificate`, `client_key`, `token_file` and a
  separator-bearing `exec.command` to absolute paths against **that file's**
  directory (`:441-486`), before any merge.
* `load_from_base64_or_file` is
  `value.map(load_from_base64).or_else(|| file.as_ref().map(load_from_file))`
  (`:739-750`) — embedded data wins over the path.
* `identity_pem` returns `Err(LoadClientKey(NoBase64DataOrFile))` for
  `(Some(cert), None)` (`:651-661`).
* `rustls_client_config` takes `exec_identity_pem()` first and only calls
  `identity_pem()` when that is `None`
  (`kube-client-4.2.0/src/client/config_ext.rs:390-393`).
* `k8s-openapi-0.28.0/src/byte_string.rs:27` decodes with
  `base64::engine::general_purpose::STANDARD`, the same engine
  `load_from_base64` uses.

## 5 — PEM shapes the read is not fed by any test

The decoy PEM was rewritten two ways and pointed at through the same
`exec`-plugin kubeconfig:

| variant | bytes | `[certificates]` label line |
|---|---|---|
| as written by openssl | 1135 | `[certificates] 8d` |
| trailing newline removed | 1134 | `[certificates] 8d` |
| every `\n` → `\r\n` | 1154 | `[certificates] 8d` |

kube appends a trailing newline (`ensure_trailing_newline`,
`file_config.rs:765`) and `kubeconfig_certificate` does not; neither shape
changes what `expires_at` answers.

## 6 — what `--analysis` costs the live driver

Two processes against the same cluster, 90 seconds each, started together:

```
$ timeout 90 ./target/release/k8rs --live          > plain90.out 2>/dev/null &
$ timeout 90 ./target/release/k8rs --live --analysis > panes90.out 2>/dev/null &
$ grep -c 'pods · ' plain90.out ; wc -l < plain90.out
10
840
$ grep -c 'pods · ' panes90.out ; wc -l < panes90.out
10
2400
```

Ten reprints each. No pod was created or deleted during the window; the churn
was container restarts on the already-broken pods.

## 7 — flag surface

```
$ ./target/release/k8rs
usage: k8rs [--analysis] <file.json>...   |   k8rs --live [--analysis] [--context <name>]
Each file holds Kubernetes objects as JSON: one object, or a list of them.
Without --live this build reads files only — it cannot reach a cluster.
exit=2

$ ./target/release/k8rs --live --analysis=true
k8rs: --analysis=true is not a flag k8rs has
usage: k8rs [--analysis] <file.json>...   |   k8rs --live [--analysis] [--context <name>]
...
exit=2

$ timeout 8 ./target/release/k8rs --live --context=
k8rs: no cluster to watch — this kubeconfig has no such context — check the `--context` you gave, or the `current-context` line in the file
exit=2

$ timeout 8 ./target/release/k8rs --analysis --live
[certificates]
```

## 8 — pinned numbers this review checked rather than took on report

```
$ grep -n 'now=' scripts/certs-test.sh
now="2026-08-23 00:00:00Z"
$ grep -n 'expiring-client' scripts/certs-test.sh
"expiring-client|2026-08-12 00:00:00Z|2026-09-05 00:00:00Z|13"
$ sed -n '/fn now() -> Time {/,/^}/p' src/main_tests.rs
fn now() -> Time {
    Time("2026-08-23T00:00:00Z".parse().expect("a fixed timestamp"))
}
```

`EXPIRES_IN_DAYS = 13` in `src/main_tests.rs` is the same figure the guard's
own table carries.

## Cleanup

The scratchpad kubeconfigs, the throwaway keypair and the copies of the live
cluster's credential blobs were removed at the end of the run. Nothing from
this measurement was committed except this file.
