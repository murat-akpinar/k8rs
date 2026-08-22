# Rule 13's family read together — rules 10, 13, 14 and N1 over the pod nothing reported on

Operator review of the uncommitted tree on 2026-08-22, for todo.md line 611 and
[NOTES § D156](../NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22).
No cluster: everything here is the working tree, the committed fixtures, and the
binary built from them. The cluster measurements this review was built on are in
[2026-08-22-rule-13-the-pod-with-no-container-status.md](2026-08-22-rule-13-the-pod-with-no-container-status.md).

**Every run quoted in this file is the tree as it stood at review time**, which
is *before* findings 1, 2, 4 and 6 were fixed. What Evidence § 1 prints is the
defect, not the current behaviour — that is the point of keeping it. The disposition line on each finding says what became of it, and four
of them had already been acted on when this file was written.

## The gate, re-run rather than taken on report

```
$ just check
advisories ok, bans ok, licenses ok, sources ok
#  GREEN WITHOUT THE CROSS-COMPILE MATRIX — these targets were NOT checked:
#      x86_64-unknown-linux-musl … aarch64-apple-darwin
```

Green, with the documented local skip.

## Findings

Ranked by what they cost to leave. **Verdict** is this review's; **disposition**
is what the PM did with it, recorded here because four were acted on before this
file was written and a report that reads as if nothing happened misleads as badly
as a dangling pointer. A disposition is reported to me, not re-measured by me —
the code moved twice after this review and `tester` has been over it since.

### 1 — blocking · FIXED · N1's card said the pod *was running*

`src/rules.rs:6600`, the `answered` ternary inside `node_stopped_being_ready`.

[D156](../NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22)
ruling 2 makes rule 13's silence conditional on N1 drawing the card instead. Run
it, and N1 said this about the pod no kubelet ever picked up:

```
default/broken-unstarted was running here (1 pod)
```

It never ran. That is the one thing this fixture exists to prove is not true of
it. The verb came from a ternary keyed on whether the node *answered*
(`Ready: False` → `is`/`are`) or went quiet (`Ready: Unknown` → `was`/`were`) —
a distinction about the **node**, used to make a claim about the **pod**. Neither
tense is true of a pod with no container status.

This is the class a family review exists for: rule 13 reads `containers.is_empty()`
and concludes *nothing ever started this*, while N1 reads the same pod through
`pods_on` and prints *was running here*. Before ruling 2 the verb was cosmetic;
ruling 2 made it load-bearing.

The test did not catch it because it asserted the pod was *named*, not what was
said — `src/rules_tests/pod.rs`, in
`the_pod_nothing_reported_on_is_the_nodes_card_when_the_node_went_quiet`:

```rust
down.evidence.contains(&qualified(&p.owner)),
```

`contains(name)` passes on `"X was running here"` and `"X is running here"`
alike. The test printed the false line four times in its own captured output and
was green.

**Disposition:** fixed. The ternary is deleted, both branches read `was placed
here`, `screens/alerts.md` § N1 moved with it, and the `contains` became a full
`assert_eq!`. Written up as D156 ruling 7.

### 2 — should-fix · FIXED · under `--namespace`, nothing on the screen mentions this pod

`src/rules.rs:6588` against `src/rules.rs:6005-6013`.

N1 drops its whole workload fact under a namespace scope:

```rust
if snapshot.namespace_scope.is_none() {
    let pods = pods_on(snapshot, node);
```

For `Ready: Unknown` the kubelet-message fact is also skipped (`if answered`), so
under `--namespace` N1's `facts` is **empty** — a card naming the node with no
evidence at all. Rule 13's stand-down did not consult `namespace_scope`, so it
was silent regardless. An operator running `k8rs --namespace my-app` saw *"This
node has stopped responding"* with a blank evidence line, and the pod that will
never start was on no card anywhere.

Not a regression — rule 13 was silent here before too — but ruling 2 justifies
the silence with *"N1 draws exactly that card and names the node and its
owners"*, and in this scope N1 names neither. The ruling's premise was false
under a documented flag.

**Disposition:** fixed. `placed_but_never_started` now takes `&ClusterSnapshot`
and stands down only when `namespace_scope.is_none()` *and* the node explains it.
It has a test. Still not reachable through the binary — see Evidence § 6.

### 3 — should-fix · DOC FIXED, BEHAVIOUR BACKLOGGED · the five-minute gap is unbounded on a flapping node

`src/rules.rs`, the doc block above `placed_but_never_started`, which claimed:

> a pod already past its ten minutes draws **no card at all for up to five
> minutes** while the node it is on is going down

"Up to five minutes" is the single-transition case. N1's grace runs from
`ready.last_transition` (`src/rules.rs:6580`), and a condition's
`lastTransitionTime` is rewritten on **every** flip. Measured on the review
cluster: the node went `Unknown` at `14:24:24Z`, was patched `True` at
`14:26:37Z`, and returned `Unknown` with a fresh stamp of `14:27:29Z`. So a node
whose `Ready` flaps faster than `NODE_DOWN_GRACE` never fires N1 at all, while
rule 13 stands down on every `Unknown` phase and fires on every `True` phase —
a card blinking on and off while nothing else on the screen explains the pod. A
kubelet missing heartbeats under memory pressure is the ordinary producer.

The claim was not wrong so much as reasoned from one transition, which is the
class CLAUDE.md flags: a formula read correctly and concluded from wrongly.

**Disposition:** the paragraph is rewritten to the three measured stamps. The
blink itself is **not** closed and is in [`backlog.md`](../backlog.md), because
suppressing it means one rule reading another's clock. `tester` then measured the
sharper form: during that window the whole screen prints `nothing is broken`,
which is the claim `screens/once.md` says must be true. That is in the backlog
entry too.

### 4 — should-fix · FIXED · `containers.is_empty()` had no phase guard

`src/rules.rs:6003`.

D156 ruling 1 establishes that the API server refuses `spec.containers: []`. The
trigger reads `status.containerStatuses`. The chain holds for a conforming
kubelet, but the card's sentence — *"nothing there has picked it up"* — is a
claim about the **status writer**, and the rule never looked at `status.phase`,
which is decoded and sitting on the snapshot.

The false positive: a pod with `phase: Running` and no `containerStatuses`, on a
node reporting `Ready: True`. Rule 13 fires and tells the operator nothing has
picked the pod up while it is serving traffic. Two producers, both named by the
code itself — a non-conforming virtual-node provider (`container_snapshots`' own
comment cites Tencent TKE doing exactly this to that array, k9s #4145), and "a
pruned or partial object", which the new test's doc comment says "reaches the
rule the same way".

**Disposition:** fixed with a `phase` must be `Pending` gate — **not** a check on
`spec.containers`. Ruling 1's refusal argument is about the spec and stays where
it is; the guard that landed is about the phase. Noted because the two are easy
to conflate when reading this finding later.

### 5 — nit · OPEN · `break_nodes`' friendliest error message is unreachable

`scripts/cluster.sh`, the binding block in `break_nodes`:

```bash
if [ -z "$("${kc[@]}" get pod broken-unstarted -o jsonpath='{.spec.nodeName}' 2>/dev/null)" ]; then
  "${kc[@]}" create -f - --raw "/api/v1/namespaces/default/pods/broken-unstarted/binding" …
```

If `broken-unstarted` does not exist — `break` was not run against this cluster —
the jsonpath returns empty, the `if` fires, the binding POST 404s, and `set -e`
ends the run there. The assertion block below never executes, so its fallback
text *"no such pod — did 'break' run against this cluster?"* cannot print in the
one case it was written for.

Everything else about that block is sound: the 409 re-run guard is right, the
four assertion clauses each catch a distinct silent failure, and
`has("containerStatuses") | not` is the correct test rather than a length check.

**Disposition:** still open. `tester`'s row, not yet routed.

### 6 — nit · FIXED · D156 said "Five rulings" and listed six

`NOTES.md`, D156's opening paragraph.

**Disposition:** fixed.

### 7 — nit · BACKLOGGED · pre-existing, and not this diff's doing

`src/analysis_tests/restarts.rs:459`. The comment claims *"The two runs began
three seconds apart … each row measures its own"*, and the assertion is a loop
requiring both rows to produce the **same** string. Three seconds apart renders
identically at every granularity the renderer has, so the assertion cannot
distinguish *each row measures its own* from *both read the same one*. True
before the repin at `1 hour ago` and true after at `2 days ago` — the repin
neither caused nor worsened it.

**Disposition:** in [`backlog.md`](../backlog.md).

## Answers to the six questions asked

1. **Is the stand-down correct at 3am?** The design ruling is defensible — one
   node card beats forty pod cards. What is lost is that N1 names *owners*, not
   pods, so on a real node the never-started pod is indistinguishable from the
   forty that were healthy until the kubelet died. Acceptable. What was not
   acceptable is finding 1. The clock gap is finding 3.
2. **The card's action.** Reachable and correct. `Events: <none>` is measured on
   this shape, so *read the Events* would have been a dead end, and `get_yaml`
   over `describe` is right for that reason. The action names the node and points
   at the kubelet, which is where the fault is in every branch that reaches it.
   The no-node fallback is honest.
3. **The absent arm's wording.** *"the machine has not said how far it got — this
   pod's status is missing the line (PodReadyToStartContainers) that would say
   whether it ever got as far as creating the container"* states the absence,
   names the field, glosses what it would have meant, and claims nothing. Not
   vague. One quibble left for the PM: the code comment names "a server old
   enough to predate the condition" as a producer, and ruling 4 pins the
   supported floor at 1.29, where every fixture checked here carries the
   condition.
4. **Is `containers.is_empty()` the right trigger?** Right for the init case —
   `container_snapshots` chains `init_container_statuses` first, so a pod in an
   init step never reaches the branch. Wrong without a phase guard: finding 4.
5. **The machinery.** It would survive an unsupervised trip. The 409 guard, the
   four assertion clauses, the forced `unbreak` and both `justfile` guards are
   correct, and both guards were run here against the committed bytes — green
   (Evidence § 3). One reachable rough edge: finding 5.
6. **The repin.** Clean. All four certificate counts checked against the real PEM
   `notAfter`s with `openssl` and `date`, and the three unit-changing age
   assertions against the fixtures' `startedAt`. Every one is exactly the +2-day
   shift. Nothing was updated to match output (Evidence § 4).

## Evidence

### 1 — What the card actually prints, at review time (this is the defect)

```
$ cargo build && ./target/debug/k8rs tests/fixtures/nodes.json tests/fixtures/unstarted.json
1 pod · 4 nodes

● k8rs-worker3 · 41 hours ago
  This node has stopped responding — nothing on it can be trusted until it does
  default/broken-unstarted was running here (1 pod)
  → check the node itself: is it powered on and reachable?

1 critical
```

```
$ ./target/debug/k8rs tests/fixtures/unstarted.json
1 pod · 0 nodes

▲ default/broken-unstarted · 31 min ago
  This pod was given a machine to run on, but it has not been able to start
  on node k8rs-worker3 · the machine has written nothing at all about this pod: not one of its
  containers has a status, not even a failed attempt, so nothing there has picked it up
  → check the machine itself: nothing on k8rs-worker3 has picked this pod up — look at whether
    the part of Kubernetes that starts containers there (the kubelet) is working, and whether
    that machine is still in the cluster

1 warning
```

The composed positive, from the tests:

```
$ cargo test --quiet the_pod_nothing_reported_on -- --nocapture
▲ default/broken-unstarted · 45 min ago
  This pod was given a machine to run on, but it has not been able to start
  on node k8rs-worker3 · the machine has written nothing at all about this pod: …
  → check the machine itself: nothing on k8rs-worker3 has picked this pod up — …
  $ kubectl get pod broken-unstarted -n default -o yaml

● k8rs-worker3 · 41 hours ago
  This node has stopped responding — nothing on it can be trusted until it does
  default/broken-unstarted was running here (1 pod)
  → check the node itself: is it powered on and reachable?
  $ kubectl describe node k8rs-worker3

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 506 filtered out
```

### 2 — The committed fixture, field by field

```
$ jq -c '{phase:.status.phase,conds:[.status.conditions[]?|{type,status,lastTransitionTime}],
          cs:(.status.containerStatuses//"ABSENT"),node:.spec.nodeName,
          owner:(.metadata.ownerReferences//"NONE"),
          tol:[.spec.tolerations[]?|{key,effect,tolerationSeconds}]}' tests/fixtures/unstarted.json
```
```
{"phase":"Pending",
 "conds":[{"type":"PodScheduled","status":"True","lastTransitionTime":"2026-08-22T16:20:18Z"}],
 "cs":"ABSENT","node":"k8rs-worker3","owner":"NONE",
 "tol":[{"key":"node.kubernetes.io/not-ready","effect":"NoExecute","tolerationSeconds":null},
        {"key":"node.kubernetes.io/unreachable","effect":"NoExecute","tolerationSeconds":null}]}
```

```
$ jq -c '.items[]|select(.metadata.name=="k8rs-worker3")
         |{ready:([.status.conditions[]|select(.type=="Ready")|{status,reason,lastTransitionTime}]|first)}' \
    tests/fixtures/nodes.json
{"ready":{"status":"Unknown","reason":"NodeStatusUnknown","lastTransitionTime":"2026-08-20T23:12:59Z"}}
```

Identical to the shape measured on the review cluster: one condition, absent
`containerStatuses` key, infinite tolerations, no owner. The capture carries what
the tests read off it.

### 3 — The two new guards, run here against the committed bytes

```
$ jq -e '([.status.conditions[]? | select(.type == "PodScheduled" and .status == "True"
          and .lastTransitionTime != null)] | length) == 1
         and (.status // {} | has("containerStatuses") | not)
         and .metadata.deletionTimestamp == null' tests/fixtures/unstarted.json
true

$ jq -e -n --slurpfile nodes tests/fixtures/nodes.json --slurpfile pod tests/fixtures/unstarted.json \
    '[$nodes[0].items[] | select([.status.conditions[]? | select(.type == "Ready" and .status != "True")]
      | length > 0) | .metadata.name] as $unready
     | (($pod[0].spec.nodeName // "") | IN($unready[]))'
true
```

Both pass.

### 4 — The repin, checked against the bytes rather than against the diff

Pin moved `2026-08-21T00:00:00Z` → `2026-08-23T00:00:00Z`, exactly +2 days.

```
$ for c in expiring-client healthy-client expired-client; do
    openssl x509 -in tests/fixtures/certs/$c.crt.pem -noout -enddate; done
notAfter=Sep  5 00:00:00 2026 GMT
notAfter=Aug 12 00:00:00 2027 GMT
notAfter=Aug  9 00:00:00 2026 GMT

$ for d in 2026-09-05 2027-08-12 2026-08-09 2026-09-01; do
    echo "$d: $(( ($(date -u -d $d +%s) - $(date -u -d 2026-08-23 +%s))/86400 )) at new pin,
              $(( ($(date -u -d $d +%s) - $(date -u -d 2026-08-21 +%s))/86400 )) at old"; done
2026-09-05: 13 at new pin, 15 at old
2027-08-12: 354 at new pin, 356 at old
2026-08-09: -14 at new pin, -12 at old
2026-09-01:   9 at new pin, 11 at old
```

Against the diff: `15 → 13`, `356 → 354`, `12 → 14` past, `11 days → 9 days`. Every
one is the +2 shift and nothing else.

The three that changed unit rather than number:

```
$ jq -c '[.status.containerStatuses[]?|{n:.name,started:.state.running.startedAt}]' \
    tests/fixtures/restarts10.json tests/fixtures/startup.json tests/fixtures/gang.json
restarts10: [{"n":"flaky","started":"2026-08-20T23:10:04Z"}]
startup:    [{"n":"slowboot","started":"2026-08-20T22:43:03Z"}]
gang:       [{"n":"bystander","started":"2026-08-20T22:43:27Z"},
             {"n":"trigger","started":"2026-08-20T22:43:24Z"}]
```

| assertion | old | new | against the bytes |
|---|---|---|---|
| `restarts10` run age | `50 min ago` | `2 days ago` | 23:10:04 → +49 m 56 s at old pin; +2 d 0 h 50 m at new |
| `gang` run age | `1 hour ago` | `2 days ago` | 22:43:2x → +1 h 16 m at old pin; +2 d 1 h 17 m at new |
| `card.age()` | `47 min ago` | `2 days ago` | same shift, coarsened by the renderer |

No number moved by an amount the two-day shift does not explain.

### 5 — Family cross-reads

```
$ grep -n "node_condition(" src/rules.rs
6010:            .and_then(|n| node_condition(n, "Ready"))     # rule 13, new
6576:    let ready = node_condition(node, "Ready")?;           # N1
6730:            let c = node_condition(node, type_)…          # node_running_low, pressure types
```

Rule 13 and N1 read the same condition by the same name and both treat
`!= "True"` as not-ready. No disagreement.

`pods_on` (`src/rules.rs:6519`) filters `!finished(p)`, and `finished` is
`phase in {Succeeded, Failed}` (`src/rules.rs:2880`). The fixture's phase is
`Pending`, so N1's list does reach this pod — the hand-off is not lost in the
filter.

`container_snapshots` (`src/rules.rs:~2500`) chains `init_container_statuses`
before `container_statuses`, so a pod sitting in an init step decodes with a
non-empty `containers` and cannot reach the new branch. The init-container false
positive does not exist.

Rules 10, 13 and 14 are disjoint on `scheduled`: 14 needs `is_none()`, 13 needs
`status == "True"`, 10 needs it present and not `True`.

### 6 — What could not be measured here

`src/main.rs:231` pins `namespace_scope: None`, so the `--namespace` behaviour in
finding 2 above is read off the two functions rather than run through the binary. Stated rather
than implied. The fix carries a test; it is still not reachable from the driver,
for the same reason.

## Sanitization re-read

Re-read for the [reports/ rule](README.md#the-sanitization-rule--read-it-before-pasting-cluster-output)
after the file was complete. **Nothing in it came from the review cluster's
objects**: every command output quoted is either a committed fixture, the binary
over committed fixtures, the test suite, `openssl` over the committed test PEMs,
or `date` arithmetic. The only review-cluster material is three condition
`lastTransitionTime` stamps and the name `phantom-node` in finding 3 — a `Node`
object created by hand on a cluster that no longer exists, which names no machine.
No IP, no hostname, no token, no certificate body, no kubeconfig, no environment
value, no annotation payload, no Secret. The node names that do appear
(`k8rs-worker3`) are the fixture cluster's and are already committed throughout
`tests/fixtures/` and `NOTES.md`. `scripts/reports-guard.py` runs clean over the
directory.
