# The command log panel — measurements taken during the operator review

`k8s-admin`, 2026-09-07. Subject: the uncommitted Phase 11 box
*Command log panel — always visible, showing what k8rs ran* — the new
`// --- THE COMMAND LOG ---` region in `src/views.rs`, read together with
`ui.rs`'s `strip()`, `main.rs`'s `command_log`/`kubectl_get`, `k8s.rs`'s
`LogRequest::kubectl`/`events`/`document`, and the mockups in `screens/`.

No cluster was brought up. Every number below is off this repository at the
working tree state `f02fa59` + the uncommitted diff (482 insertions, 0
deletions).

## How these were run

The tree was copied out of the repository (`src/`, `Cargo.toml`, `Cargo.lock`,
`clippy.toml`, `tests/`) into `$HOME/k8rs-review`, scratch `#[test]`s were
appended to the **copy's** `src/ui_tests.rs` and `src/main_tests.rs`, and the
copy was built with its own `CARGO_TARGET_DIR`. Both scratch trees were deleted
afterwards; the repository working tree was not touched.

```
cd $HOME/k8rs-review
CARGO_TARGET_DIR=$HOME/k8rs-review-target cargo test --quiet -- --nocapture --test-threads=1 review_
```

## 1. The manifest is 15 lines in the ordinary case and 16 at most

The box's `KEPT` doc claims **sixteen**. Confirmed as the maximum, not the
usual value.

```
Cluster, --analysis: 15 lines
   $ kubectl get --raw '/api/v1/pods?limit=1'
   $ kubectl get --raw /version
   $ kubectl api-resources --verbs=list
   $ kubectl get certificatesigningrequests
   $ kubectl get replicasets -A
   $ kubectl get services -A
   $ kubectl get endpointslices -A
   $ kubectl get persistentvolumeclaims -A
   $ kubectl get poddisruptionbudgets -A
   $ kubectl top nodes
   $ kubectl get pods -A --watch
   $ kubectl get nodes --watch
   $ kubectl get deployments -A --watch
   $ kubectl get statefulsets -A --watch
   $ kubectl get daemonsets -A --watch

Cluster, no analysis: 8 lines
```

Sixteen is reached only by `Coverage::Refused` / `Coverage::Blind` with a
context that names no namespace, which is what adds the second scope probe:

```
Refused, no context ns, --analysis: 16 lines
   $ kubectl get --raw '/api/v1/pods?limit=1'
   $ kubectl get --raw '/api/v1/namespaces/<namespace>/pods?limit=1'
   ... 14 more, as above
```

(The namespace in that second probe is `k8s::FALLBACK_NAMESPACE` on the real
path; the scratch run passed an empty string, which is not a state
`k8s::coverage` can produce — `k8s.rs:6891-6897` always carries a non-empty
one.)

## 2. What the strip actually draws at startup

`ui.rs:576` takes the **last** `LOG_LINES` of the feed. Fed the real manifest,
rendered at 80x24:

```
--- TUI with reports (--analysis equivalent): 15 manifest lines ---
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl get daemonsets -A --watch                                          │
├──────────────────────────────────────────────────────────────────────────────┤

--- TUI without reports: 8 manifest lines ---
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl get daemonsets -A --watch                                          │
```

`screens/alerts.md:25-26` and `:405-406` draw, in the same two rows:

```
│ $ kubectl get pods -A --watch                                      │
│ $ kubectl get nodes --watch                                        │
```

Those are manifest lines 11 and 12 of 15.

## 3. The strip is 76 columns at 80x24, and it clips at the tail with no marker

`draw` (ui.rs:410-425) spends 2 columns on the outer `Block::bordered`, and
`strip` (ui.rs:575-582) calls `indented`, which is `Margin::new(1, 0)` — 1
column each side. 80 − 2 − 2 = **76**.

`Paragraph` with no `Wrap` truncates. Rendered at 80x24, one line per case;
`intact` is whether the drawn row still contains the input string:

```
input  ( 77): $ kubectl get secret db-credentials -n payments -o yaml --show-managed-fields
drawn        : │ $ kubectl get secret db-credentials -n payments -o yaml --show-managed-field │
intact       : false

input  ( 69): $ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields
drawn        : │ $ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields        │
intact       : true

input  ( 77): $ kubectl get secret pending-secret -n payments -o yaml --show-managed-fields
drawn        : │ $ kubectl get secret pending-secret -n payments -o yaml --show-managed-field │
intact       : false

input  (107): $ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9p -n payments-production -o yaml --show-managed-fields
drawn        : │ $ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9p -n payments-productio │
intact       : false

input  ( 86): $ kubectl events --for pod/checkout-api-canary-7d9f4bc86d-x2k9p -n payments-production
drawn        : │ $ kubectl events --for pod/checkout-api-canary-7d9f4bc86d-x2k9p -n payments- │
intact       : false

input  ( 82): $ kubectl describe pod checkout-api-canary-7d9f4bc86d-x2k9p -n payments-production
drawn        : │ $ kubectl describe pod checkout-api-canary-7d9f4bc86d-x2k9p -n payments-prod │
intact       : false

input  ( 80): $ kubectl scale deployment/checkout-api --replicas=12 -n payments-production   …
drawn        : │ $ kubectl scale deployment/checkout-api --replicas=12 -n payments-production │
intact       : false

input  ( 92): $ kubectl logs checkout-api-canary-7d9f4bc86d-x2k9p -n payments-production -c app --previous
drawn        : │ $ kubectl logs checkout-api-canary-7d9f4bc86d-x2k9p -n payments-production - │
intact       : false
```

The first two are the lines `screens/detail.md:1378` and `:1505` draw
themselves. The seventh is a mutation on the wire whose `…` running mark is
gone.

### The truncation that leaves a *valid* command behind

Prefix `$ kubectl get pod ` is 18 columns and ` -n ` is 4, so a 40-character
object name and a 14-character namespace land the cut exactly on a token
boundary:

```
k8rs ran      (106): $ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0 -o yaml --show-managed-fields
operator reads      : │ $ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0 │
```

The drawn line runs, exits 0, and prints a table row instead of the object.

### The frame in the mockups is not the frame in the code

Measured off `screens/`, character counts of the log row and its enclosing
border:

| file:line | frame width | content columns | log text columns |
|---|---|---|---|
| `detail.md:1287` | 80 | 78 | 69 |
| `detail.md:1378` | 80 | 78 | **77** |
| `detail.md:1505` | 80 | 78 | **77** |
| `detail.md:1141` | 70 | 68 | 51 |
| `dialogs.md:743` | 70 | 68 | 66 |

The two 77-column rows are drawn with a 1-column left margin and **no** right
margin. `indented()` reserves one on both sides.

## 4. The gap before an outcome is not three columns in every mockup

Every mockup in `screens/` that draws an outcome on a command-log line, with
the space count between the command and `→`:

```
context.md:360:│ $ kubectl --context staging get pods -A   → not allowed            │      3
context.md:420:│ $ kubectl --context staging get pods -A   → not allowed            │      3
states.md:218: │ $ kubectl get pods -A --watch   → login expired                    │      3
dialogs.md:743:│ $ kubectl scale deployment/web --replicas=9 -n payments  → rejected│      2
dialogs.md:775:│ $ kubectl delete pod/web-7d9f4 -n payments   → not sent            │      3
detail.md:1141:│ $ kubectl events --for pod/web-7d9f4 -n payments  → refused        │      2
```

Four of six use three; two use two. Both two-space rows sit in 70-column
mockups. `views.rs`'s `OUTCOME_GAP` doc states three columns *"as every mockup
that draws one uses"*.

## 5. Which mockups put an outcome on a line that is not a mutation

- `states.md:218` — `$ kubectl get pods -A --watch   → login expired`. A
  **manifest** line.
- `detail.md:1141` — `$ kubectl events --for pod/web-7d9f4 -n payments  →
  refused`. A **user-initiated read** line.
- `context.md:360`, `:420` — `$ kubectl --context staging get pods -A   → not
  allowed`. A line that is neither in `command_log` nor produced by any builder
  in this box.

`views::Log` sets `waiting` only in `started`, and `outcome` returns having done
nothing when `waiting` is `None` (`views.rs`, and the box's own test
`an_outcome_with_nothing_running_writes_nothing`).

## 6. What the strip in each screen file contains

Every command-log row drawn in a mockup, by file:

```
alerts.md:25,405   $ kubectl get pods -A --watch
alerts.md:26,406   $ kubectl get nodes --watch
resources.md:24    $ kubectl get deployments -n payments
analysis.md:102    $ kubectl get nodes -o json
analysis.md:103    $ kubectl top nodes
analysis.md:280    $ kubectl get pods -n payments --watch
analysis.md:433    $ kubectl get pdb -A
analysis.md:1124   $ kubectl get svc,endpointslices,pvc,replicasets -A
analysis.md:2015   $ kubectl get csr
analysis.md:2016   $ kubectl version
context.md:44,231  $ kubectl config get-contexts
detail.md:25,…     $ kubectl logs web-7d9f4 -n payments -c app --previous
detail.md:435,…    $ kubectl describe pod web-7d9f4 -n payments
detail.md:874,…    $ kubectl events --for pod/web-7d9f4 -n payments
detail.md:1287,…   $ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields
```

The four `analysis.md` spellings are not the ones `main.rs`'s `command_log`
produces for the same reads: it writes `$ kubectl get
certificatesigningrequests` where the mockup writes `$ kubectl get csr`, `$
kubectl get --raw /version` where the mockup writes `$ kubectl version`,
`poddisruptionbudgets` where the mockup writes `pdb`, and five separate `get`
lines where `analysis.md:1124` writes one comma-joined line.
`analysis.md:102-103` names `$ kubectl get nodes -o json`, which appears in no
manifest at all. `analysis.md` also indents its `$` by two columns where every
other file uses one.

## 7. Two spellings of one line

`main.rs:5390-5400`:

```rust
format!(
    "$ kubectl get {} {}{} -o yaml --show-managed-fields",
    sanitize(qualified), sanitize(name),
    namespace.map_or(String::new(), |namespace| format!(" -n {namespace}", …)))
```

`views.rs` `yaml_line`:

```rust
format!(
    "$ kubectl get {resource} {}{} -o yaml --show-managed-fields",
    id.name, in_namespace(id))
```

`main.rs:5521-5523` builds its first argument as `singular` for a core kind and
`format!("{singular}.{group}")` for a grouped one. `views.rs` takes the word
from its caller and its doc states no such requirement.

`main.rs:5528` writes a second line under that command for a Secret
(`caveats`, `main.rs:5431-5439`): *"k8rs: a Secret's values are hidden here and
shown as their sizes — the command above prints them in full"*. There is no
equivalent on the drawn strip.

## 8. Build state

`cargo build` on the copy: **no warnings**. `views::Log`, `describe_line`,
`events_line` and `yaml_line` have no caller in `src/` outside the test
modules:

```
$ grep -rn 'describe_line|events_line|yaml_line|views::Log|log\.ran|log\.started|\.outcome\(' src/ | grep -v _tests
src/views.rs:819,820,946,951,975,997,999,1012   (doc comments and the definitions)
```

No product file constructs a `ui::Screen`, so `ui::draw` is reached only from
`ui_tests.rs`.

## 9. Field values the findings turn on, from files that were read rather than run

- `k8s.rs:205` — `IDENTIFIER = 512`, the bound every name on a read line
  carries.
- `k8s.rs:6069` — `EVENTS_KEPT = 500`, the events fetch's server-side `limit`.
- `k8s.rs:6226-6230` — the events selector is
  `involvedObject.kind`, `involvedObject.name`, and `involvedObject.uid` where
  the caller has one; the namespace is the request's, not a selector term.
- `k8s.rs:6219-6225` — `events(client, namespace, kind, name, uid)`; the
  `namespace` doc names `default` as where a Node's events live.
- `ui.rs:66` — `LOG_LINES = 2`; `ui.rs:213-215` — `Screen::log` is
  `&'a [String]`.
- `ops.rs:510-554` — `Outcome` has six arms; `ops.rs:625-644` —
  `Performed::plainly` interpolates `outcome.said()`, the server's own words.
- `CLAUDE.md` § Secrets and local files, citing NOTES § D217 — a
  `fieldValidation=Strict` rejection returns the whole object in
  `Status.message`, **4859 bytes** on a trivial Deployment.
- `ui.rs:553-571` — `shortened()`, the existing tail-cut-with-a-visible-marker
  helper, used by the header row and by nothing else.
