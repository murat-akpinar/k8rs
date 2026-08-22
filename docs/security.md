# k8rs Security Model

> k8rs changes your cluster. This document is what keeps that from being
> frightening: what it can touch, what stops it, and what is written down
> afterwards.
>
> Until 2026-08-11 the tool was read-only and this document said so. The
> guarantee is now *guarded* rather than *structural* — the reasoning is in
> [NOTES § Reversal](../NOTES.md#reversal--read-only--managed-writes-2026-08-11).

## Trust model

- **Nothing is deployed.** No DaemonSet, no CRD, no webhook, no in-cluster
  component. k8rs runs on your machine and talks to the API server the same
  way `kubectl` does. This is the one structural guarantee left, and it is not
  traded away for anything.
- **You keep your own permissions.** k8rs can do exactly what your kubeconfig
  user can do — never more. It does not hold credentials of its own, and there
  is no in-cluster ServiceAccount mode; that code path deliberately does not
  exist.
- **`--read-only` restores the original guarantee** in one flag: the write
  code is unreachable, the keys are unbound, and the header says so.
- **No telemetry.** Nothing leaves the machine; no network connection is made
  except to the API server in your kubeconfig.
- **Nothing happens silently.** Every command k8rs runs appears in the
  command log as you would have typed it, and every mutation is appended to a
  local audit file.

### The one code-execution path, and why it exists

Your kubeconfig may not contain a credential at all. On EKS, GKE and AKS it
names a **program** to run for one — `aws eks get-token`,
`gke-gcloud-auth-plugin` — which is executed to mint a short-lived token. This
is how `kubectl` has always worked, k8rs inherits it through kube-rs, and
removing it would mean k8rs does not run on any managed cluster.

It is worth naming plainly because it is the only path by which k8rs runs
anything but itself:

- k8rs **never installs a credential plugin**, never offers to, and never
  suggests one by name in an error.
- It runs only what the kubeconfig it was pointed at already specifies. A
  kubeconfig is as trusted as an executable, and that has always been true of
  `kubectl` too — k8rs does not widen it.
- The plugin's output is a credential and is treated as one: it is never
  logged, never rendered, never put in an error message
  ([token hygiene](#token-hygiene)).
- These tokens **expire during a session**. That is a `401`, which is neither
  "you are not allowed" nor "nothing answered", and it gets its own
  plain-language state saying the login expired
  ([NOTES § D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)).

## Write safety model

Five mechanisms, each a requirement rather than a nicety:

| Mechanism | What it prevents |
|---|---|
| **Containment** — writes exist only in `ops.rs` | An accidental mutation anywhere else in the codebase |
| **Consent** — selected object + keypress + confirmation stating the consequence | Acting on the wrong object, or without understanding what happens |
| **Preflight** — server-side `dryRun=All`, abort on rejection | Discovering an admission-webhook rejection halfway through a change |
| **Typed confirmation** for delete and drain | The keyboard-slip class of accident |
| **Audit** — every attempt, including refusals and failures | Not being able to answer "what happened to this cluster" |

Plus two absences that matter: **no bulk mutation** (single object, single
confirmation — the multi-select delete is how outages happen) and **no
optimistic local state** (the result comes back through the same watch stream
as any other change, so the screen never shows a change that did not land).

The operations, in the order they ship: scale · rollout restart · delete
(v0.1), then cordon / uncordon / drain / rollout undo with the node rules
(v0.2), then exec and port-forward (v0.3), and edit + apply last (v0.4). The
order is the operator's day, not build convenience — and it puts the two that
widen the trust boundary, and the one that needs a temp file full of object
YAML, at the end where they get their own scrutiny.

**Restart is not an API verb.** For workloads it patches the
`kubectl.kubernetes.io/restartedAt` annotation, exactly as
`kubectl rollout restart` does. For a bare pod it is a *delete* — and the
confirmation says so, because a beginner must not learn "restart" as a
synonym for "delete" by accident.

## RBAC

Two roles, so the mode you run in is enforced by the cluster and not only by
the tool.

Read-only — everything the Alerts and Analysis views need:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-readonly
rules:
  - apiGroups: [""]
    resources: ["pods", "pods/log", "events", "services", "nodes",
                "persistentvolumeclaims", "configmaps"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets", "replicasets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["policy"]
    resources: ["poddisruptionbudgets"]
    verbs: ["get", "list", "watch"]
  # a CronJob's pods are owned by a Job, and only the Job names the CronJob.
  # `cronjobs` itself is deliberately absent: the Job's ownerReference already
  # carries the CronJob's kind, name and uid, so nothing reads the object
  - apiGroups: ["batch"]
    resources: ["jobs"]
    verbs: ["get", "list", "watch"]
  # rule C3 — the pending certificate signing requests nobody approved
  - apiGroups: ["certificates.k8s.io"]
    resources: ["certificatesigningrequests"]
    verbs: ["get", "list", "watch"]
  # the waste report — a Service whose selector matches nothing
  - apiGroups: ["discovery.k8s.io"]
    resources: ["endpointslices"]
    verbs: ["get", "list", "watch"]
  # only needed for the capacity report
  - apiGroups: ["metrics.k8s.io"]
    resources: ["pods", "nodes"]
    verbs: ["get", "list"]
  # only needed for rule C4, and only where cert-manager is installed —
  # omitted deliberately, add it if you want the certificate rows it feeds:
  #   - apiGroups: ["cert-manager.io"]
  #     resources: ["certificates"]
  #     verbs: ["get", "list", "watch"]
```

`batch` is here because of what a pod carries and what it does not. A CronJob's
pod names its Job in `ownerReferences` and says nothing about the CronJob above
it, so grouping the pods of a five-minute schedule onto one card requires a GET
on the Job. Without the verb that GET is a 403, every tick files under its own
Job name, and the card churn lands on the user running the least-privileged
role — the one least equipped to explain it. The degradation is named, not
silent: the finding files under the Job and says the CronJob could not be read.

Admin — the above plus the operations:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-admin
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["delete"]
  - apiGroups: [""]
    resources: ["pods/eviction"]     # drain
    verbs: ["create"]
  - apiGroups: [""]
    resources: ["nodes"]
    verbs: ["patch"]                 # cordon / uncordon
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets"]
    verbs: ["patch", "update"]       # scale, rollout restart, edit
```

`pods/exec` and `pods/portforward` are intentionally absent — grant them only
when those features ship and only to users who need them.

If your user has less than any of this, k8rs still runs: a 403 on a secondary
stream disables only the findings that need it, and a 403 on a write is
reported as "you do not have permission to do this", naming the verb and the
resource.

## Enforcement (not just policy)

- **The check is an allowlist, not a denylist.** Outside `ops.rs`, only read
  methods may appear: `get*`, `list*`, `watch*`, `logs`, `log_stream`,
  `apiserver_version` and the `get_` subresource readers. Anything else fails
  CI.

  This inversion is deliberate. `kube::Api` exposes far more mutating methods
  than the obvious four — `cordon`, `uncordon`, `restart`, `evict`, `attach`,
  `exec`, `portforward`, `entry`, `create_subresource`, `create_token_request`,
  `patch_scale`, `patch_status`, `patch_metadata`, `patch_approval` and their
  `replace_*` siblings. A ban list over that surface is wrong the first time
  kube-rs adds a method; an allowlist asks "is this read-only", which is the
  question that actually matters.
- `clippy.toml` still carries a `disallowed-methods` list crate-wide as the
  fast feedback loop (CI runs clippy with `-D warnings`), and `ops.rs` carries
  the single `#![allow(clippy::disallowed_methods)]` in the project — the
  exception announces itself at the top of the file that owns it.
- The e2e job runs under `--read-only` against kind and fails if any mutating
  request reaches the API server.
- **The mechanizable half of the review checklist is a script**, not a list
  somebody re-reads:
  [`scripts/security-guard.py`](../scripts/security-guard.py) fails the build on
  a workflow that grants write or names an action by tag, a shell spawned from
  `src/`, a dependency or a hostname outside the approved list, a `Debug` derived over a
  type that can hold a token, a call into the in-cluster ServiceAccount
  environment, or a TLS verification knob turned off by us. It bans the **call**
  and never the word, so a kubeconfig that itself sets
  `insecure-skip-tls-verify` is still honoured and still shown in the header.
  What it cannot decide yet is decided by a human, and
  [NOTES § D105](../NOTES.md#d105--the-security-gate-splits-into-what-a-script-can-decide-today-and-what-is-waiting-for-code-2026-08-16)
  lists every one of those with the phase that makes it mechanical.
- Rules and analysis reports are pure functions with no I/O — the only code
  touching the network is `k8s.rs` (reads) and `ops.rs` (writes).

## Token hygiene

- The kubeconfig token is never logged, never rendered on screen, never
  embedded in an error message. The config type's `Debug` output is wrapped.
- This includes the panic path: a backtrace dumped to stderr must not
  contain credentials.
- **One thing off the kubeconfig does enter our own structs, and it is the
  public half only.** Certificate rule C1 warns when the client certificate is
  about to expire, so `ClusterSnapshot` carries the **certificate** bytes and
  the context name — never the private key, never the token, never anything
  else. This is deliberate and narrow: rules are pure functions over the
  snapshot ([invariant 5](../CLAUDE.md)), so C1's input has to arrive the same
  way every other rule's does, and the alternative — a second entry point
  taking PEM bytes directly — would have meant amending a hard invariant
  ([NOTES § D51](../NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).
  A certificate is not a secret; the key beside it on disk is, and the field's
  own doc says so because a reader will reasonably ask why a kubeconfig is
  anywhere near that struct. A test fails if an **armoured** private key appears
  in the fixture that field is tested with — which is a check on the fixture,
  not a constraint on `k8s.rs`: the test builds the value itself, so nothing
  yet stops Phase 5 putting something else there. Closing that is Phase 5's
  ingest gate, and a base64-wrapped key — the framing a kubeconfig actually
  uses for `client-key-data` — walks past the current check
  ([NOTES § D31](../NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).

## The audit log

`~/.local/state/k8rs/audit.log`, mode 0600, append-only, plain text. One line
per attempted mutation:

```
2026-08-11T14:22:07Z  ctx=prod-eu  ns=payments  deployment/web
  shown: kubectl scale deployment/web --replicas=3 -n payments
  call:  PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale
         rv=88213  dry-run=ok                                    → ok
2026-08-11T14:23:15Z  ctx=prod-eu  ns=payments  pod/web-7d9f4
  shown: kubectl delete pod web-7d9f4 -n payments
  call:  (none)                                      → refused by user
```

Two lines, not one, and the difference matters: k8rs calls
`Api::patch_scale`, not `kubectl scale`. The `shown:` line is what the user
saw and learned from; the `call:` line is what actually reached the API
server, with the resourceVersion sent and the dry-run verdict. An audit trail
that records only the teaching aid is fiction.

It records refusals and failures as well as successes — a trail that only
records what worked cannot answer "what did they try". Nothing about it
involves the cluster; it is a local file.

**Order matters:** the attempt line is written and flushed *before* the API
call and the result is appended when it returns, so a crash mid-call leaves an
attempt with no result — the honest record of exactly what is known.

**If it cannot be written, the mutation does not happen.** A full disk or a
read-only home does not get to silently turn the audit trail off; a write that
cannot be recorded is refused, with the reason on screen. If the log cannot be
opened at startup, k8rs says so and runs read-only rather than exiting —
someone should still be able to look at their cluster
([NOTES § D21](../NOTES.md#d21--if-the-write-cannot-be-audited-the-write-does-not-happen)).

**It is not rotated.** A few hundred bytes per mutation reaches a megabyte in
about a decade of daily use. A rotator would be more code than the log.

## Data displayed and stored

- Environment variable **values are never displayed**.
- **Secret contents are hidden by default.** Viewing a Secret shows its keys
  and their sizes; revealing a value requires an explicit second action, and a
  revealed value never enters the command log, the audit log, or the YAML
  shown by `y`.
- **The edit temp file is treated as a leak surface.** A full object YAML can
  carry Secret data, environment values and tokens. It is written to the
  user's own temp directory with mode 0600 and removed on exit *and* on panic.
- **`exec` and `port-forward` change the boundary** and are therefore the last
  features to land: exec hands the terminal to a process inside a container
  (control-character stripping cannot apply to an interactive PTY), and
  port-forward opens a local listening socket. Both bind to loopback only,
  both are shown in the header while active, and both are disabled under
  `--read-only`.
- Free-text API fields (event messages, container status messages) are
  rendered through ratatui's cell-based drawing and additionally stripped of
  control characters — an ANSI escape sequence inside an event message must
  not be able to corrupt or spoof the terminal.
- Test fixtures are sanitized before commit: `managedFields`, all
  annotations (including `last-applied-configuration`, which contains a full
  spec copy with env values), env values, `selfLink` and image pull secret
  names are stripped by the capture script. Raw `kubectl get -o json` output is
  never committed.
- **Node identifiers are refused, not stripped.** Mangled node names would
  break the pod↔node joins the node rules are built on, so a capture carrying
  an identifier from anywhere other than the kind test cluster fails the
  capture instead of producing something that only looks safe.
- The filter walks the whole document rather than named paths, because half the
  capture is the `List` that `kubectl get <kind> -A -o json` returns and the
  objects inside it sit under `.items[]`. Its test feeds it **both** shapes —
  a single object and a `List` — since a filter proven on one of the two reads
  as proven and is not
  ([NOTES § D29](../NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
- **A secret is removed in every framing it can arrive in**, not only when it
  is the whole value. Addresses are replaced inside strings, because a podCIDR
  wears a `/24` suffix and kubelet quotes the address it could not reach inside
  an English sentence. Key material is also matched base64-encoded, which is
  how every Secret value arrives and where `-----BEGIN` never appears;
  certificates are left alone, since a certificate is the public half by
  definition. Both framings shipped past an anchored filter once
  ([NOTES § D31](../NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
- The filter is not the last line. `scripts/fixture-audit.sh` re-checks the
  **committed bytes** of every file under `tests/fixtures/` — not only the
  JSON, since a key is still a key when it is called `admin.key.pem` — because
  a fixture can reach the directory hand-edited, copied from a bug report, or
  captured with an older sanitizer, having never met the filter at all.

## Supply chain

- `Cargo.lock` is committed.
- CI runs `cargo deny check` (advisories, licenses, sources); non-crates.io
  sources are forbidden.
- Dependabot watches cargo + GitHub Actions weekly; kube-rs and k8s-openapi
  are grouped and upgraded together.
- GitHub Actions run with `permissions: contents: read` by default;
  third-party actions are pinned to commit SHAs; `pull_request_target` with
  secrets is forbidden.
- Releases ship with a `SHA256SUMS` file. Binary signing is deferred until
  there is an audience to verify it.

## Future trust-boundary changes (recorded now, on purpose)

- **v3 traffic adapter** (Prometheus / Istio / Hubble): the first connection
  outside the Kubernetes API. The endpoint address comes only from explicit
  user configuration — never auto-discovered from cluster annotations
  (SSRF / unintended-target risk). Token hygiene rules apply unchanged.
- **v4 connectivity mesh** (goldpinger-style): requires a DaemonSet, which
  changes the trust model entirely. It will therefore live in a **separate
  binary and repository**, strictly opt-in; deployment code never enters the
  k8rs binary. "Nothing is deployed into your cluster" is the last structural
  guarantee k8rs has, and it must survive a `--help` inspection.
