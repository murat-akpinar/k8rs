# k8rs Architecture

> Status: design phase — this document describes the agreed architecture
> before any code exists. Decisions and their rationale live in `../NOTES.md`;
> the technology choices (language, crates, toolchain) in `tech-stack.md`;
> this is the buildable summary.

## Overview

k8rs is a single-binary Rust + ratatui TUI — *lazygit for Kubernetes*. It
watches a cluster, turns raw state into diagnoses (*what happened · what it
means · what to do*), and lets the user act on them without typing long
`kubectl` commands. It installs nothing into the cluster, and it shows every
command it runs.

Three views, one content pane at a time:

| View | Content | Cadence |
|---|---|---|
| **Alerts** *(default)* | per-object findings from `rules.rs`, severity-sorted | live |
| **Resources** | browser over every kind the cluster serves; the operations live here | list + watch while open |
| **Analysis** | cluster-wide reports from `analysis.rs` — capacity, certificates, drain safety, posture, restarts, waste, versions | on demand |

One question decides which view a finding belongs to: **is it broken right
now, or is it risky, wasteful or expiring?** The first goes to Alerts, the
second to Analysis. Alerts is a work queue and an empty one has to be
believable — so "this pod has no memory limit" is a Capacity row, not an
alarm. Alerts findings are also **grouped by owner**: one Deployment with
three sick pods is one entry carrying a count, never three entries, and a
DaemonSet on forty nodes is still one.

## Data flow

```
Kubernetes API server
        │  (one LIST, then a watch stream — no polling loops)
        ├──────────────────────────────┬────────────────────────────┐
        ▼                              ▼                            ▼
kube-rs watcher × 5            discovery + Table lists      ops.rs (writes)
        │  Pods, Nodes, Deployments,   │  server-side columns,      │  dry-run
        │  StatefulSets, DaemonSets    │  no per-kind code          │  → confirm
        │  prune: the fields the       │                            │  → apply
        │  snapshot types name;        │                            │  → audit
        │  drop managedFields          │                            │
        ▼                              │                            │
Snapshot store (small structs)         │                            │
        │                              │                            │
        ├──► rules::analyze()   -> Vec<Finding>    ← pure, per object, live
        └──► analysis::report() -> Report          ← pure, whole cluster
                                       │                            │
                                       ▼                            ▼
                    views.rs (selection, filters, tabs) ◄───────────┘
                                       ▼
                    ratatui UI — redraws only on change; blocks when idle
```

Writes are a one-way street from `ops.rs` to the API; nothing else in the
binary can mutate anything, and the result comes back through the same watch
stream as any other change — there is no optimistic local mutation of the
store.

**Two things in the store did not come from a watch**, and the diagram would
otherwise imply they did: the API server's version, read once at startup for
N4's skew comparison, and — for certificate rule C1 — the kubeconfig context
name and the client **certificate**, which never came from the cluster at all.
Rules are pure functions over the snapshot, so an input that is not an API
object still has to arrive on it. The private key is not carried; see
[Token hygiene](security.md#token-hygiene).

Why watch instead of polling: every `LIST pods -A` forces the API server to
read and serialize every pod from etcd, degrading linearly with cluster
size (this is what makes interval-polling tools feel heavy). A watch pays
that cost once, then receives only deltas.

## Components

```
src/
  main.rs      event loop, terminal setup/teardown, view routing
  k8s.rs       connect(context), discovery, watches, prune -> store (reads only)
  ops.rs       every write. The ONLY file that may mutate the cluster
  rules.rs     analyze(&Snapshot) -> Vec<Finding>     ← the product lives here
  analysis.rs  cluster-wide reports
  views.rs     per-view state: selection, filters, tabs, scroll
  ui.rs        ratatui drawing
  theme.rs     Catppuccin constants (10 of them)
tests/
  fixtures/    sanitized JSON captured from a real cluster
```

Eight files. No `mod.rs` pyramid, no trait layer, no plugin system.

`ops.rs` is one file on purpose: the write surface has to be reviewable in a
single sitting. `clippy.toml` bans the mutating API methods crate-wide and
`ops.rs` carries the only `#![allow(clippy::disallowed_methods)]` in the
project, so the exception announces itself at the top of the file that owns
it.

### Resource views without per-kind code

The browser never hard-codes a kind. `kube::discovery` enumerates what the
cluster actually serves — built-ins and CRDs alike — and each list is fetched
with `Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json`,
which makes the API server return exactly the columns `kubectl get` would
print. Custom resources therefore display correctly without a line written for
them, and column formatting can never drift from kubectl.

Two details this depends on:

- The `,application/json` fallback is mandatory. Aggregated and extension API
  servers may not serve Table at all and answer `406` to a Table-only Accept
  header; the client must handle either shape.
- kube-rs does not expose a `Table` type, so this one request is built through
  `Client::request` and decoded with `serde_json`. It is the only hand-built
  HTTP request in the binary.
- Table is a list representation, not a watch one. Browser views therefore
  watch `watch_metadata` (PartialObjectMetadata — tiny) to learn *that*
  something changed and re-fetch the Table, debounced. No blind polling.

Typed `k8s-openapi` structs are used only where the rule engine needs field
access (Pod, Node, Deployment, Service, PVC).

### The shared contract

The first thing written in the code phase, because three files meet on it:

```rust
struct Finding {
    severity:    Severity,       // Critical | Warn | Info
    title:       String,         // what happened (plain language)
    evidence:    String,         // the numbers/fields that prove it
    action:      String,         // what to do about it
    kubectl_cmd: Option<String>, // the command that shows the same thing;
                                 // None when no such command exists
    owner:       ObjectId,       // the grouping key: Deployment/DaemonSet/…,
                                 // or the pod itself when it has no owner
    object:      ObjectId,       // what the finding is about — the pod, the node
    timestamp:   Option<Time>,   // when the event happened — the moment, never
                                 // the phrase; None when no field records it
}

struct ObjectId {
    kind:      ObjectKind,      // …/CronJob/ReplicaSet/Node/Pod/Other(String)
    namespace: Option<String>,  // None = cluster-scoped, e.g. a Node
    name:      String,
    uid:       Option<String>,  // None only when it is not an API object
}
```

`rules.rs` decides the identity; `views.rs` does the grouping. The bottom
layer stays pure and the presentation layer stays replaceable.

`ObjectId::group_key()` — kind, namespace, name, without the uid — is what
`views.rs` groups by; `ObjectId` itself derives no `Hash`, so grouping by the
whole thing stops compiling at the first map insert. That grouping would split
one Deployment into two cards when it is deleted and recreated under the same
name — old-generation pods still terminating under one uid while the new ones
run under another, which is what any Argo prune-and-recreate produces
([NOTES § D38](../NOTES.md#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12)).

`owner` and `object` are both here because one broken pod produces several
findings — a crashlooping pod fires four of the v1 rules at once — so a card
counting *findings* would say "4 of 5 pods" about a single pod. The numerator
is the count of distinct `object`s; the denominator comes from the snapshot.
The reasons behind the rest of the shape are
[NOTES § D36](../NOTES.md#d36--the-finding-shape-the-review-sent-back-2026-08-12).

`timestamp` is a moment and never a phrase: `Finding::age(now)` turns it into
`Some("4 min ago")` at draw time, and it is the one call both the Alerts view
and `--once` make — two renderers spelling the same finding differently is one
of them lying. It answers `None` in two cases that draw the same blank: no
field records when the event happened, so the right edge stays empty rather
than borrowing a nearby timestamp that answers a different question; or the
moment is *ahead* of `now` by more than five minutes, which is either a rule
that filled the wrong field or a machine whose clock is behind the cluster's,
and a plausible phrase would hide both. Inside that five-minute window a future
moment draws `just now`
([NOTES § D18](../NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact) ·
[§ D68](../NOTES.md#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13)).

**That blank is only half of what a wrong clock does, and `k8s.rs` says the
other half out loud.** With this machine behind the cluster by `S`, an event of
age `A` is blanked only while `A < S − 5min`; everything older prints a number
short by the whole `S`, so the same screen both hides recent times and
under-reports old ones. Ahead of the cluster nothing blanks and every age
inflates. Neither is visible from inside `rules.rs`, whose only input is the
snapshot, so the measurement is `k8s.rs`'s: it reads the API server's own `Date`
response header — refusing a non-2xx, because a refusal's clock may be a
middlebox's — and `Session::skew` carries the signed gap past the same five
minutes. The renderer states the gap and the direction and never whose clock is
wrong, which is not something the header can tell
([NOTES § D55](../NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) ·
[§ D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28)).

### Rules are pure functions

`analyze()` takes a snapshot, returns findings. No I/O, no `Result` —
a missing field simply produces no finding. This makes every rule testable
with plain unit tests against JSON fixtures; no cluster and no terminal
needed. The same holds for `analysis.rs`, which takes a whole-cluster snapshot
and returns a report. The rule tables live in `../NOTES.md`.

### The write path

Every mutation follows the same five steps, in `ops.rs`:

```
select object → keypress → confirm dialog (consequence in plain language,
                                           plus the kubectl equivalent)
             → server-side dryRun=All  (abort and show the API message on reject)
             → the real call
             → audit line + command log entry, success or failure
```

Deletes and drains insert one more step: the user types the object name.
`--read-only` makes the whole path unreachable — the keys are not bound and
`ops.rs` is never called. There is no bulk mutation and no operation that
runs without a selected object.

### Async model

One `tokio::select!` loop in `main.rs` over three sources:

1. the watcher stream (cluster changes)
2. crossterm `EventStream` (keyboard)
3. Ctrl-C

Drawing happens only when one of these fires. No separate UI thread, no
channel layer, no actors. Terminal restore is guaranteed via a `Drop` guard
plus a panic hook — a TUI must never leave the terminal in raw mode.

## Build order — forward-only (pyramid)

Layers freeze bottom-up; a later step never modifies a file frozen by an
earlier step. If it would have to, the plan is wrong and gets fixed first
(rule details in `../CLAUDE.md`, step sequence in `../todo.md`):

```
rules.rs     → frozen after the rules + tests step   (the product core)
analysis.rs  → frozen after the reports step        (pure, like rules)
k8s.rs       → frozen after the read paths are complete (watch, discovery, logs)
ops.rs       → frozen after the operations step, proven headlessly against kind
theme.rs     → frozen after the theme step (incl. COLORTERM fallback)
views.rs     → frozen after the view-state step
ui.rs        → the screens
main.rs      → top of the pyramid; the only file still being wired at the end
```

`ops.rs` sits low deliberately: every write is verified without a terminal
(scale a deployment against kind, watch the replicas change, read the audit
line back). The dangerous code is proven before it is ever bound to a key,
which turns the UI phase into wiring.

Learning spikes (e.g. ratatui experiments) go into `examples/` as throwaway
code and never touch product files.

## Performance behaviors

- The Alerts view's own inputs are watched permanently: Pods, Nodes, and
  Deployments/StatefulSets/DaemonSets — five low-traffic streams; workload
  objects are far fewer than pods and barely churn. **The prune list is the
  snapshot types in `rules.rs`**, and it spans metadata, spec and status on all
  three kinds: `spec.volumes` (rule 8), `spec.terminationGracePeriodSeconds`
  (rule 12), `spec.unschedulable` and the taints under it (N2),
  `spec.containers[].resources` (rule 2, N5), `spec.replicas` (the workload
  `desired`) and `spec.containers[].restartPolicyRules` — on **every** container
  of the pod and on the init list, since a rule one container over can restart
  this one (rule 15,
  [NOTES § D135](../NOTES.md#d135--family-b-the-trip-that-already-ran-the-resize-boxs-stale-premise-and-the-shape-a-capture-cannot-catch-2026-08-21)) —
  all sit in the half an earlier "metadata + status only" would have
  dropped
  ([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
  ReplicaSets are fetched on demand and cached, never watched. Every
  other kind is listed when its view opens and watched only while it is on
  screen — "browse everything" must not mean forty permanent streams.
- Drop `metadata.managedFields` at ingest — often a third of the object.
- Store reduced snapshots, not full `Pod` objects (~10x memory).
- No global Events watch in v1 — noisiest stream in the cluster; the
  event-based rules ship in v2.
- metrics-server (if ever used) is polled slowly (30s+) and only for
  visible pods; it cannot be watched.
- No fixed-FPS rendering: draw on change, block when idle → 0% CPU at idle.
- Redraws are coalesced (~100ms debounce) so rollouts don't spike CPU.

Targets: < 50MB RSS at ~1000 pods · first paint < 1s · findings < 3s ·
minimum terminal 80×24. The paint figures are quoted at that cluster size on
purpose — the initial LIST grows with the cluster and nothing is drawn until it
lands, so the size they hold up to is measured in Phase 5 and stated, and above
it the first paint reports what it is waiting for
([NOTES § D115](../NOTES.md#d115--the-prune-line-bounds-memory-and-was-read-as-if-it-bounded-time-and-the-paint-budget-is-stated-at-a-cluster-size-the-risk-is-not-2026-08-18)).

## Error handling

- **Startup errors are the ones with nothing to connect *with*** — no
  kubeconfig, a file that will not load, a context the file does not name, a
  login program that produced no credential. Those reach stderr **before** the
  TUI starts, with a non-zero exit.
- **An unreachable API server is not one of them.** A cluster that is down at
  connect is a screen full of *this is failing*, retried forever, never a tool
  that would not start — `PRIOR-ART § B3`'s rule is that a connectivity failure
  is a banner, not a shutdown, and `k8s.rs` § CONNECTING implements it that way.
  This bullet said the opposite until 2026-08-27, when a run against a dead port
  was measured and did not exit
  ([NOTES § D167](../NOTES.md#d167--eight-faults-not-two-and-the-two-the-review-had-to-produce-2026-08-27)).
- **Eight distinctions matter to the user, not two, and they get one enum:
  `k8s::Fault`.** *Permission denied vs no connection* was the original pair
  and it was never enough — `Kubeconfig` (the file itself: missing, unreadable,
  not valid YAML) · `NoContext` (the file loaded and names no such context) ·
  `BadEntry` (the file loaded and something it points at did not — a certificate
  it names, a `server:` line, a cluster a context refers to) · `NoCredential`
  (the kubeconfig names a login program and that program produced nothing —
  still nothing sent, and the fix is on the reader's own machine; reachable at
  connect **and** mid-session) · `Expired` (`401`, the ordinary managed-cluster case, and it names
  the login program from the user's own `exec` block rather than guessing a
  cloud, [NOTES § D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program))
  · `Refused` (`403` — the sentence says the **role needs** the verb, never that
  the kubeconfig *is not allowed*: k8rs needs both `list` and `watch` to watch a
  kind and cannot tell from a refusal which one was missing) · `Gone` (`404`) ·
  `Unanswered` (everything that did not come back usably — one variant on
  purpose, because from the reader's side they are one fact).
- **A fallback string is printed only for the case it actually describes**, and
  every site holding a typed error routes through the one classifier. This is
  stronger than invariant 14 and sits beside it: invariant 14 governs the
  *wording* a user reads, this governs where it is allowed to come from. k9s
  tells these errors apart internally and still prints
  `Ruroh? 'v1/pods' command not found` when a credential expires, because a
  generic handler between the call and the screen swallowed the typed error
  ([PRIOR-ART § C1](../PRIOR-ART.md#c1--the-generic-handler-ate-the-real-error)).
  One fallback remains legitimate and is named rather than implied: a watch
  that ended with no error attached says *nothing was ever said about why*.
- **The words are the caller's, and so is what was asked.** `Fault` carries no
  string at all; the sentence is written where the call site is, because that
  is what knows the verb and the resource the security gate requires a `403`
  to name — and a `nonResourceURL` refusal has neither, so the only true
  sentence names the path
  ([NOTES § D160](../NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26)).
- **One refusal the classifier cannot see, stated because the code claimed the
  opposite until it was measured**: every field of `Status` is
  `#[serde(default)]`, so a proxy answering `403` with a JSON body that is not
  a `Status` deserializes *successfully* into an all-default one — `code: 0`,
  no reason — and kube's own `with_code` fallback never runs. The HTTP status
  is then unrecoverable from `kube::Error::Api`, and such a refusal reads as
  *nothing usable came back*
  ([NOTES § D167](../NOTES.md#d167--eight-faults-not-two-and-the-two-the-review-had-to-produce-2026-08-27)).
- Watch drops / `410 Gone`: kube-rs watcher reconnects with backoff; the UI
  must show "disconnected, retrying" — silently stale data is forbidden.
- Partial RBAC: a 403 on a secondary stream disables only the rules that
  need it; the app keeps running and says which permission is missing. A 403
  on a *write* is reported as "you do not have permission to do this", naming
  the verb and resource — the most common thing a newcomer will hit.
- A 403 on the **cluster-wide** pod LIST is not a degraded feature, it is the
  whole Alerts view — so it falls back to the kubeconfig context's namespace
  (then `default`) instead of failing, and the header states the scope in
  effect. `--namespace` sets it explicitly. Access to two namespaces must
  produce a working tool. **The rules that join *every* pod on a node switch
  off under that scope and say so** — N2 (cordoned with a drain left
  unfinished) and N5 (overcommit). A partial view turns the first into a
  finding that silently never fires and the second into an understated sum, and
  a wrong number that looks confident is worse than a feature that names what
  it is missing
  ([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)).
  The scope is a **field on the snapshot**, not something a rule can ask about:
  rules are pure functions with no globals, so without it a small cluster and a
  partial view of a large one look identical from inside a rule
  ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
- A rejected write (admission webhook, validation, conflict) shows the API
  server's own message verbatim and stays until dismissed. A `409 Conflict` on
  apply means the object changed underneath the edit; the user is offered a
  re-read, never a blind overwrite.
- Every failed or refused mutation is written to the audit log too — a trail
  that records only successes cannot answer "what did they try".

## Testing

- Every rule has a **positive** fixture (triggers the finding) and a
  **negative** fixture (healthy pod → nothing) — false positives are bugs.
- Fixtures are real captures from a kind cluster running deliberately broken
  pods ([`scripts/broken.yaml`](../scripts/broken.yaml), with its healthy
  counterpart [`scripts/healthy.yaml`](../scripts/healthy.yaml)), sanitized by script, committed, and
  recorded with the k8s version they came from. Hand-written JSON is only a
  bootstrap and must be replaced.
- Fixtures deserialize through `k8s_openapi::Pod`, so the *decode* path is
  covered by the same tests. **The prune path is not**, and cannot be: `kubectl
  get -o json` omits `managedFields` unless explicitly asked, and the sanitizer
  deletes them regardless — so a fixture never carries the field pruning is
  about, and a test asserting it was pruned would pass over an object that
  never had it. Pruning is to be verified against live watch data in the
  client layer, where the field actually arrives — Phase 5 is where that
  becomes true; no code in this repo has met an API server yet
  ([NOTES § D30](../NOTES.md#d30--the-guards-phase-2-added-and-the-freeze-they-collided-with-2026-08-12)).
- **A decode test may set one field on a real capture** — a branch whose input
  the capture cannot contain is a branch no test can reach, and the corpus has
  always lacked *something*. The three shapes that licence was written for are
  no longer among them: a cordoned node, a partially-ready workload and a pod
  with an owner have each since been captured (`nodes.json`'s `k8rs-worker`,
  `statefulsets.json`'s `broken-sts` at 1 ready of 2, `owned-pods.json`), and
  each plant was retired by the trip that brought its object back. That is the
  licence working, not an exception to it. It starts from a committed capture, changes one
  field to a value the API demonstrably produces, says why the capture lacks
  it, and names the object the next capture trip should bring back to replace
  it. A **rule's** positive fixture is still a real capture — this never
  becomes the way a rule gets proven
  ([NOTES § D40](../NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).
- The decode itself is proven by **field-level mutation done by hand**, not by
  `cargo mutants`, which does not mutate struct-literal field assignments
  ([NOTES § D41](../NOTES.md#d41--cargo-mutants-cannot-see-the-defect-it-was-put-there-to-catch-2026-08-12)).

## Version compatibility

`k8s-openapi` is pinned to the **newest** version feature the crate offers —
`v1_36` today. The pin decides which fields exist in the generated types, and the
two ways of getting it wrong are not symmetric: pinned **below** the cluster,
every field added since is dropped at decode without a word, and a dropped field
is indistinguishable from one the cluster never set; pinned **above** it, the
field is simply absent and reads as no finding, which every rule already handles.
A diagnosis tool cannot afford the first, so the pin leads.

The rule was the opposite until 2026-08-15 — *oldest feature, window ±2 minor* —
and it is reversed in
[NOTES § D99](../NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15),
which also measures what the old pin had been dropping.

`scripts/fixture-audit.sh` fails when the pin's minor falls below the version in
`tests/fixtures/K8S_VERSION` — an inequality, so the crate may run ahead of the
kind image. kube-rs and k8s-openapi are upgraded together, never separately.

### Which clusters k8rs supports

**The oldest API server k8rs is supported against is Kubernetes 1.29, and the
newest it fully understands is the one its `k8s-openapi` pin was built from — 1.36
today.** Outside that window k8rs still runs: it says one line at connect and
carries on. Refusing to start would tell somebody with a broken old cluster
nothing at all about their broken old cluster, and nothing k8rs does on one is
unsafe.

**Nothing k8rs sends is refused by an older server.** The initial LIST asks for
`limit` and follows `continue` (chunking, on by default since 1.9); the watch asks
for `allowWatchBookmarks` (stable at 1.17, and a server that does not implement it
ignores the parameter). k8rs deliberately does **not** use streaming lists —
`sendInitialEvents` is ignored by servers older than 1.27, which leaves the client
waiting forever for a bookmark that never comes
([k9s #4044](https://github.com/derailed/k9s/issues/4044)), and is rejected with a
403 by a server that knows it with the `WatchList` gate off (which 1.33 shipped as
the default).

**Below 1.29, some findings go quiet — and one used to say more than the cluster
told it, which is where the floor came from.** A field the cluster does not have
reads as absent, and every rule treats
absent as *no finding*, so a cluster too old for container restart rules, in-place
resize, pod-level resources, native sidecars or `status.terminatingReplicas` simply
reports less — which is right, because it also *has* less.

The floor was set by the one place that did worse than report less. The
`PodReadyToStartContainers` condition enters the Kubernetes API at 1.29, and the
card for a pod that was scheduled and never started used to read its absence as
*storage and network are fine* — a sentence no older cluster ever said
([NOTES § D149](../NOTES.md#d149--the-floor-is-129-because-one-rules-else-turns-a-missing-field-into-a-claim-2026-08-22)).
**That branch was fixed on 2026-08-22 and the floor did not move with it**
([NOTES § D156](../NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22)):
the card now has a third arm and claims nothing when the condition is absent. The
floor stays 1.29 for a weaker reason — **nobody has read every other `else` over
an optional API field against the same question**, and until someone has, 1.29 is
the oldest cluster this tool is willing to claim it is honest on. One rule was
fixed; the class was not.

**Above the pin, some findings never arrive.** A cluster newer than the types this
binary was compiled against still answers every request, but fields Kubernetes
added after that version are dropped when the response is decoded — silently,
exactly like a field the cluster never set. Everybody running 1.37 the day 1.37
ships is in that state, and no test in this repo can see it happen on somebody
else's machine, which is why k8rs says it out loud at connect instead. Upgrading
k8rs is the fix.

## Out of scope

Nothing is ever deployed into the cluster — no DaemonSet, no CRD, no webhook.
No LLM. No bulk mutation. No cluster lifecycle or cloud-provider APIs. No
free-form topology graphs. No config file, theming or plugin system. No
side-by-side multi-cluster panes.

The browser and the write operations *were* on this list until 2026-08-11;
they are in scope now by an explicit, recorded decision. The full list and the
reasoning: `../NOTES.md`.
