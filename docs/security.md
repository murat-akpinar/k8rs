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
| **Preflight** — server-side `dryRun=All` where the API offers it, abort on rejection | Discovering an admission-webhook rejection halfway through a change |
| **Typed confirmation** for delete and drain | The keyboard-slip class of accident |
| **Audit** — every attempt, including refusals and failures | Not being able to answer "what happened to this cluster" |

**Opening a confirmation dialog sends one request to the API server, and that
is by design rather than by accident.** `screens/dialogs.md` requires the
dry-run's verdict to be shown *before* the confirm button is live — the dialog's
own line is *"The cluster checked it first and accepted it."* — so for any
operation that has a preflight, the `dryRun=All` goes out while the dialog is on
screen and before anybody has agreed to anything. Nothing is mutated, which is
what `dryRun=All` means, and **authorization is identical to the real call**
(`tmp/k8s/api-concepts.txt:759`) — so the preflight adds no permission to the
documented read-only role and widens no grant. What it does do is leave a trace,
and a reviewer should meet the shape of that trace here rather than find it:

- **The marker rides in a different place per verb, and a rule keyed on the URI
  is blind to half of them.** The `scale` PATCH and the eviction POST carry
  `dryRun=All` in the **query string**; a `DELETE` carries it in the **request
  body**, because `DeleteOptions` is the delete verb's body parameter. So a
  cancelled k8rs *delete* dialog and a delete that actually happened produce the
  **same `requestURI`** in the apiserver's own audit log. The field that tells
  them apart is `requestObject.dryRun`, which exists at `Request` audit level and
  above and not at `Metadata`.
- **A SIEM rule must therefore not be written against the URI alone.** A rule
  counting `patch deployments/scale` counts scale dialogs, including ones nobody
  confirmed, and says nothing about the operation that most needs watching.
- **One spelling detail, because a reviewer will grep for it:** kube emits
  `?&dryRun=All`, with an empty leading pair, so a pattern anchored on
  `?dryRun=All` matches nothing k8rs sends.
- **Every patch also carries `fieldValidation=Strict`, on both passes**, so the
  server rejects a field it does not know instead of accepting the write and
  changing nothing. A delete carries none — `DeleteOptions` is not an object and
  has nothing to validate. Worth knowing when comparing against kubectl: **no
  `kubectl patch` of any kind sends `fieldValidation`**, so k8rs is deliberately
  stricter than the command its own command log teaches
  ([NOTES § D217](../NOTES.md#d217--strict-on-every-write-that-can-carry-it-and-the-422-that-hands-back-the-object-you-sent-2026-09-04)).
- Every matching admission webhook is invoked with `dryRun: true`.

k8rs's own audit log records the outcome of a cancelled dialog as *nothing was
changed*, which is true — but a request was sent, and that is the fact this
section exists to state
([NOTES § D214](../NOTES.md#d214--the-mutation-contract-four-lies-a-record-could-tell-and-the-three-operations-that-have-no-dry-run-2026-09-04) ·
[§ D215](../NOTES.md#d215--the-api-dry-runs-all-three-it-was-kubes-convenience-helper-that-did-not-and-the-annotation-it-writes-is-not-kubectls-2026-09-04)).

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
`kubectl.kubernetes.io/restartedAt` annotation — **the key `kubectl rollout
restart` itself writes, which is not the key kube-rs's `Api::restart` helper
writes.** That helper spells it `kube.kubernetes.io/restartedAt`, and a
different key means the pod template differs from the one kubectl would have
produced: the operator who runs the command the command log taught them gets a
**second** rollout, and finds an annotation in their Deployment that nothing in
their cluster wrote. So k8rs builds this patch itself
([NOTES § D215](../NOTES.md#d215--the-api-dry-runs-all-three-it-was-kubes-convenience-helper-that-did-not-and-the-annotation-it-writes-is-not-kubectls-2026-09-04)). For a bare pod it is a *delete* — and the
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
  # API discovery — the sidebar, and the capability probe that decides which
  # analysis rows can answer at all. `system:discovery` is bound to
  # `system:authenticated` by default, so this rule looks redundant until a
  # cluster removes that binding as ordinary hardening; then every resource
  # grant below still works and `/apis` alone answers 403 (NOTES § D160)
  - nonResourceURLs: ["/api", "/apis", "/api/*", "/apis/*", "/version"]
    verbs: ["get"]
  # `pods/log` is read as of 2026-08-30 — `--logs` fetches and follows one
  # container's log. This grant was measured sufficient for it against a real
  # cluster: extracted verbatim, bound to a ServiceAccount, `--logs` exits 0
  # (`k8s-admin`, reports/2026-08-30-the-log-stream-against-a-cluster.md).
  # `events` was granted ahead of the code (NOTES § D187) and **the code caught
  # up on 2026-08-31**: `--describe` reads this object's events through an
  # `involvedObject` field selector, and `k8s-admin` measured the refusal under a
  # role without it — stdout byte-identical to a pod with no events, the
  # difference carried by exit `2` and one sentence naming the missing verb and
  # resource (NOTES § D198)
  - apiGroups: [""]
    resources: ["pods", "pods/log", "events", "services", "nodes",
                "persistentvolumeclaims"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets", "replicasets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["policy"]
    resources: ["poddisruptionbudgets"]
    verbs: ["get", "list", "watch"]
  # rule C3 — the pending certificate signing requests nobody approved
  - apiGroups: ["certificates.k8s.io"]
    resources: ["certificatesigningrequests"]
    verbs: ["get", "list", "watch"]
  # the waste report — a Service whose selector matches nothing
  - apiGroups: ["discovery.k8s.io"]
    resources: ["endpointslices"]
    verbs: ["get", "list", "watch"]
  # only needed for the capacity report's `using …` lines. `nodes` and not
  # `pods`: k8rs reads one item per node, never one per pod — the pod half was
  # granted before anything read either, and a grant nothing uses is not least
  # privilege (`k8s-admin`, 2026-08-29). It is also the expensive half.
  - apiGroups: ["metrics.k8s.io"]
    resources: ["nodes"]
    verbs: ["get", "list"]
  # only needed for rule C4, and only where cert-manager is installed —
  # omitted deliberately, add it if you want the certificate rows it feeds:
  #   - apiGroups: ["cert-manager.io"]
  #     resources: ["certificates"]
  #     verbs: ["get", "list", "watch"]
```

**Every rule above is reachable by code that exists, and that is checked rather
than assumed.** The role was run against kind under itself on 2026-08-30 — including under an
identity outside `system:authenticated`, so the default `system:discovery`
binding could not apply and the `nonResourceURLs` rule above is what answered
discovery — and it drew every finding and all seven analysis panes with zero
refusals ([NOTES § D187](../NOTES.md#d187--the-read-only-role-under-itself-two-grants-nothing-reads-a-decision-that-described-code-that-was-never-written-and-the-one-sentence-that-sends-an-operator-to-the-wrong-resource-2026-08-30)).
Two grants came out in that audit: `configmaps`, which nothing has ever read —
rule 4 reads the kubelet's *message*, which names the missing object without
needing access to it — and `batch: ["jobs"]`, granted in 2026-08-12 for a
CronJob grouping that was described in NOTES and never written. `pods/log` and
`events` are the two that stayed ahead of their code, and the comment above
them says so.

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
- **The list covers two types, and the door it leaves open is guarded at the
  other end.** The allowlist is derived over `kube::Api` *and* `kube_core::Request`,
  because a write does not have to go through `Api<K>`: a `Request` built by hand
  and posted through the client was a complete DELETE that raised nothing
  ([NOTES § D142](../NOTES.md#d142--a-write-does-not-have-to-go-through-apik-and-the-allowlist-already-fits-the-surface-that-was-missed-2026-08-22)).
  `kube::Client` is deliberately **not** on the list — its `request` and `send`
  are verb-agnostic, and a read outside `ops.rs` needs one of them. So what makes
  such a call read-only is not the method that sends it but **the builder that
  shaped it**: the only request k8rs sends this way is built by
  `Request::get`, which is on the allowlist, and a `Request::delete` piped through
  the same sender is caught at the builder. Reading the sender and not the builder
  is how the DELETE above got through the first time.
- `clippy.toml` still carries a `disallowed-methods` list crate-wide as the
  fast feedback loop (CI runs clippy with `-D warnings`), and `ops.rs` carries
  the single `#![allow(clippy::disallowed_methods)]` in the project — the
  exception announces itself at the top of the file that owns it. **That it is
  still the only one is checked, because clippy structurally cannot check it**:
  an allowed lint never fires, so nothing in the build reports the file that
  turned it off. `scripts/write-guard.py` pins the exception to `ops.rs` — in
  every cargo root, and in `Cargo.toml`, `.cargo/config.toml` and the committed
  rustc command lines that can silence it with no `.rs` file changed
  ([NOTES § D212](../NOTES.md#d212--an-allowed-lint-never-fires-so-clippy-cannot-report-the-file-that-turns-it-off-and-the-switch-was-in-the-justfile-2026-09-03)).
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
  `insecure-skip-tls-verify` is still **honoured**. **It is not yet shown
  anywhere, and this line claimed it was until 2026-08-31.** Measured: one
  kubeconfig with the CA dropped and the flag set produces output byte-identical
  to the verified run, and `grep -rn "\.insecure\b" src/*.rs` outside the tests
  returns nothing — the flag is carried on a context-picker row
  ([D174](../NOTES.md#d174--the-operator-review-of-the-kubeconfig-family-ten-fixed-one-refused-and-the-two-reversals-it-forced-2026-08-28) ·
  [D175](../NOTES.md#d175--the-ruling-in-d174-was-wrong-about-rfc-3986-and-the-parse-that-is-safe-in-both-directions-2026-08-28))
  that Phase 11 has not drawn yet, and the headless surfaces have no equivalent
  at all. The gate item stays *honoured **and** surfaced*; what changed is that
  this file now says which half is built. Boxed in
  [backlog.md](../backlog.md), because the answer is a screen decision before it
  is a Rust one.
  What it cannot decide yet is decided by a human, and
  [NOTES § D105](../NOTES.md#d105--the-security-gate-splits-into-what-a-script-can-decide-today-and-what-is-waiting-for-code-2026-08-16)
  lists every one of those with the phase that makes it mechanical.
- Rules and analysis reports are pure functions with no I/O — the only code
  touching the network is `k8s.rs` (reads) and `ops.rs` (writes).

## Token hygiene

- The kubeconfig token is never logged, never rendered on screen, never
  embedded in an error message. **No type of ours that can reach a token
  derives `Debug`** — the rule is the derive, not a wrapper, because a
  hand-written `Debug` still lets `{:?}` compile and has to be kept correct by
  whoever adds the next field. `Trouble` lost its derive on 2026-08-27 for
  exactly that reason
  ([NOTES § D164](../NOTES.md#d164--the-token-hygiene-guard-learns-three-shapes-it-could-not-see-and-says-out-loud-what-it-still-cannot-2026-08-27)).
  **`Session` is the first type in `src/` that holds one** — through
  `kube::Client`, whose `Config` keeps the oidc and gcp providers' tokens in a
  plain `HashMap<String, String>` with a derived `Debug`. It carries no `Debug`
  of its own, so a stray `{session:?}` is a compile error rather than a leak,
  and `scripts/security-guard.py` refuses the derive if anyone adds it back
  ([NOTES § D166](../NOTES.md#d166--connect-its-shape-its-fourteen-choices-and-the-backoff-kubes-own-default-did-not-earn-2026-08-27)).
- **A credential can arrive in the `server:` line, and it is stripped before
  that address is drawn.** `clusters[].server` may carry URL userinfo
  (`https://admin:hunter2@host`) — basic auth at a proxy in front of an API
  server — and it is all printable, so no control-character strip removes it.
  The context picker draws that address on its most prominent row, so a
  kubeconfig password would reach the first screen a stranger sees and every
  screenshot of it. `k8s::address` removes the userinfo, and **where the two
  readings of an ambiguous `@` disagree it draws nothing at all rather than
  guessing** — guessing the other way invents a hostname, which is a different
  lie on the same line
  ([NOTES § D175](../NOTES.md#d175--the-ruling-in-d174-was-wrong-about-rfc-3986-and-the-parse-that-is-safe-in-both-directions-2026-08-28)).
  No script sees this class: it is printable text in a field nothing else
  treats as secret.
- This includes the panic path: a backtrace dumped to stderr must not
  contain credentials.
- **A `kube` error is never formatted whole — not with `{}`, not with `{:?}`.**
  A renderer selects fields off the typed error: the variant, and the `Status`
  where there is one. **Read
  [`k8s.rs` § WHAT A THROTTLE LOOKS LIKE](../src/k8s.rs) before writing that
  renderer** — *four* `watcher::Error` variants carry a `Status` and only
  *three* wrap it in `Error::Api`. `WatchError(Box<Status>)` holds it directly
  and is the one a busy cluster produces most (the 410 desync, an in-band 403),
  so a formatter written from the three-variant list unwraps, finds nothing,
  and prints a generic message for the commonest watch failure there is —
  `PRIOR-ART § C1` exactly. Key on `Status.code`: it survives both parse
  branches and `reason` survives only one. Measured against the
  crates on 2026-08-26, `Display` interpolates the source at every hop —
  `watcher::Error::InitialListFailed` is `"…: {0}"` (`watcher.rs:30`),
  `kube_client::Error::Auth` is `"auth error: {0}"` (`error.rs:104`), and
  `AuthError::AuthExecRun` is
  `"auth exec command '{cmd}' failed with status {status}: {out:?}"`
  (`client/auth/mod.rs:55`) over a `std::process::Output`, whose `Debug` prints
  stdout as a string when it is valid UTF-8. An `exec` credential plugin writes
  `{"kind":"ExecCredential","status":{"token":"…"}}` to **stdout**, so one
  `format!("{}", err)` on an expired EKS/GKE/AKS session prints a bearer token.
  Turning `oauth` and `oidc` off removes two variants and not this one.
  **This bullet is half mechanical and half yours, and the split is the thing
  to remember.** `scripts/security-guard.py` refuses a *derived* `Debug` on any
  declaration it parses that can reach a `Config`, a `Client` or a qualified
  `kube` error type — that half is enforced, and it is what took the derive off
  `Trouble`. It sees **no format call at all**: a `{}`, a `{:?}` or a
  `.to_string()` on a kube error, a hand-written `Debug` that formats one
  whole, an `anyhow` chain printed after a `?`. The guard prints that list in
  its own summary on every run rather than leaving the gap to be inferred, and
  those are checked by hand against this section
  ([NOTES § D162](../NOTES.md#d162--per-watch-identity-and-the-six-choices-the-reconnect-box-had-to-make-2026-08-26),
  [§ D164](../NOTES.md#d164--the-token-hygiene-guard-learns-three-shapes-it-could-not-see-and-says-out-loud-what-it-still-cannot-2026-08-27)).
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

- Environment variable **values are never displayed** *by a finding, a card or
  any surface k8rs composes*. **The `y` YAML pane is the stated exception**, and
  it is one because of what that pane is: the object as the API server sent it,
  which is the only claim it makes and the only one that makes it useful. A pane
  that quietly dropped an ordinary field would be lying about being a copy —
  `kubectl get -o yaml`, which the reader can already run, shows the same line.
  The rule bites where k8rs goes and *fetches* a value on its own initiative and
  puts it somewhere the reader did not ask for
  ([NOTES § D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12) ·
  [§ D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)),
  and `managedFields` is on the pane for the same reason.
- **Secret contents are hidden by default.** Viewing a Secret shows its keys
  and their sizes; revealing a value requires an explicit second action, and a
  revealed value never enters the command log, the audit log, or the YAML
  shown by `y`. **The command log still shows the command k8rs ran, and on a
  Secret that command prints what this pane hid** — there is no `kubectl` line
  that reproduces a masked view, so rather than print a line that does not
  produce what was printed, k8rs names the difference out loud: *a Secret's
  values are hidden here and shown as their sizes — the command above prints
  them in full*. Found and fixed at Phase 6's close, verified against a real
  Secret
  ([NOTES § D208](../NOTES.md#d208--the-cross-family-review-the-picker-that-called-a-failed-container-done-and-the-owner-fetch-that-was-never-written-2026-09-03)). **A Secret keeps more than one copy of itself, so hiding by
  position is not enough**: `kubectl apply` writes the whole applied body —
  `data` map included, and *plaintext* when it was applied through `stringData`
  — into `metadata.annotations`, so on a Secret every annotation value is hidden
  behind its size too, and the keys stay drawn
  ([NOTES § D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
  Labels stay visible: 63 characters, and nothing writes a Secret's body into
  one. **The headless `--yaml` has no reveal at all** — a reveal is a keypress on
  a drawn pane — so on that surface a Secret's values are unreachable, not merely
  hidden.
- **A report is a document, and its reader chooses where it goes.** A finding
  carries the controller's message **verbatim**
  ([NOTES § D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)),
  and a validating webhook that echoes back the object it rejected — several in
  the wild do — can put an env value inside one. On a terminal that is no worse
  than `kubectl describe`, which the same reader can already run; redirected into
  a CI log with `k8rs --once > findings.txt`, or pasted into a ticket, it reaches
  everyone who can read that log. k8rs does not blank the field — refusing to show
  what `kubectl` shows is a tool lying by omission, not a security control — so
  **a `--once` report carries whatever this cluster's controllers wrote into a
  status, and redirecting it is a decision about who sees that**
  ([NOTES § D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)).
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
