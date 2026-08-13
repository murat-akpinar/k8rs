# Screen — Alerts

The default view. k8rs never opens on a pod list; it opens on what is broken.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   workloads        │    Containers exceeded their memory limit and │
│   network          │    were killed by the kernel (OOMKilled)      │
│   storage          │    limit 256Mi · exit 137 · 47 restarts       │
│   config           │    → raise limits.memory, or find the leak    │
│   cluster          │                                               │
│ ANALYSIS           │  ▲ shop/api  ·  2 of 6 pods       12 min ago  │
│   capacity      1 ▲│    Running, but not receiving traffic — the   │
│   certificates  30d│    readiness check is failing                 │
│   drain safety     │    → check the app's /healthz endpoint        │
│   waste            │                                               │
│   versions         │  ▲ node-3                        2 hours ago  │
│                    │    This node refuses new pods (cordoned)      │
│                    │    2 pods here would still have to move       │
│                    │    → allow new pods once the work is done     │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
│ $ kubectl get nodes --watch                                        │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

## What each part is

| Part | Rule |
|---|---|
| Sidebar | `ALERTS` is selected on startup. Counts are **owners**, not pods. |
| Header row | Three zones on one line: cluster vitals left, the name centred, context · connection state · `admin` or `read-only` right. The right zone also carries the namespace scope when it is not cluster-wide, and a warning if the kubeconfig disables TLS verification. It is never truncated ([widgets.md § The header row](widgets.md#1a-the-header-row)). |
| Finding card | three to five lines ([widgets.md § 2](widgets.md#2-element--widget)): what happened · the evidence · what to do. Title bright, evidence dim. Blank line between cards — half the design. |
| The card's right edge | **when it happened, or nothing.** It is the only right-aligned field on the title line, and it holds one fact: the time of the event this card is about. A card whose finding has no such time leaves it empty rather than borrowing a nearby timestamp that answers a different question (see *No number we cannot produce*). |
| That field is **right-aligned, not trailing** | It ends two columns before the pane's right edge — `4 min ago`, `12 min ago` and `2 hours ago` all stop at the same column, and a longer or shorter age moves its left end, never its right. The mockup used to disagree with itself here, because an age once ran flush to the border with no source behind it (`6 days ago` on the cordon card) — that string is gone; the column itself was never the problem, and the cordon card uses it again below, honestly this time ([the cordon card](#the-cordon-card-with-and-without-its-clock)). |
| Command log | every command k8rs ran, as the user would have typed it. |
| Footer | the keys valid **right now**; `?` opens the full map. |

## The rules this screen obeys

- **One card per owner, never per pod.** `payments/web · 3 of 5 pods`, not
  three cards. A DaemonSet on forty nodes is still one card
  ([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)).
- **`of 5` is a second permission, and the card survives losing it.** The
  denominator comes from the workload watch, not from the pods
  ([NOTES § D28](../NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)),
  so a user allowed `pods` but not `deployments` has the numerator and not the
  total. The card then reads `payments/web  ·  3 pods` — the same rule as the
  age and the cordon count, applied to the one number that looks least like it
  could go missing. The grouping falls back to the pod's own name rather than
  to the ReplicaSet's hashed one; `web-7d4f5c6b8` as a card title is a card
  nobody can read, and D3's fallback for *no owner* is the right one for *owner
  unreadable* too. W1 and W2 need that watch outright, so they are off, and the
  banner says so in the same slot namespace scope uses.
- **That rule is about workloads; a card whose subject is a node is shaped
  differently.** A Node has no owner to group by and no namespace to print, so
  the card is `node-3` — one card, one node, and **never** `infra/node-3`
  ([README § the five rules](README.md#the-five-rules-every-screen-obeys)).
  There is no `n of m` either: that count counts pods, and this card is about
  one machine. The word *node* moves into the sentence underneath, which is
  where the card says what kind of thing it is.
- **"Node rule" is not the same as "node card".** Of the six N-series rules
  ([NOTES § Node rules](../NOTES.md#node-rules-n-series)) only N1, N2 and N3
  put a node on this screen. N4 and N5 are reports, not outages, and are drawn
  in Versions and Capacity ([analysis.md](analysis.md)). **N6 is the one to
  watch**: its subject is a Pending pod that cannot be placed, not the node
  doing the blocking — so it is an ordinary workload card, `payments/web`, and
  the node's name belongs in the evidence line. Choosing the identity by which
  rule fired rather than by what the finding is *about* is how a pod finding
  ends up losing its namespace.
- **Only what is broken right now.** No "this pod has no limits", no read-only
  hostPath list — those are Analysis rows
  ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).
- **No number we cannot produce.** Evidence is `limit 256Mi · exit 137 · 47
  restarts` — the memory a container was using at the moment the kernel killed
  it is not retrievable, so it is not shown. The same rule governs the age at
  the right edge, and it is the harder half: a timestamp is always *available*
  somewhere near the object, so the temptation is to reach for the nearest one
  and let the column stay full. **The age is the time of the event the card
  describes, or it is blank.** The pod's creation time is not when its mount
  became dangerous; the node's creation time is not when someone cordoned it.
  A column that is always populated and sometimes means something else is
  worse than one that is sometimes empty, because nothing on screen marks the
  rows where it lied.
- **The cordon card's age is an `Option`, and both values are real cases, not
  a happy path and an edge case.** N2 fires on `spec.unschedulable` — a bare
  boolean — but the node lifecycle controller stamps `timeAdded` on the
  `NoSchedule` taint it mirrors from that boolean, for every write that flips
  it through the ordinary API path, `kubectl cordon` included. **The one path
  left with nothing is a taint written by hand** — `kubectl taint`, client-side,
  stamps no time — and the same emptiness follows anything that rewrites
  `node.spec.taints` wholesale (`kubectl edit`, a GitOps controller
  reconciling Node objects, a manifest re-apply), which drops a timestamp the
  controller had written and never re-stamps a taint that pre-existed the
  cordon ([NOTES § D65](../NOTES.md#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13) ·
  [§ D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
  No node condition transitions on a cordon either way, so `Ready` keeps the
  `lastTransitionTime` it had before and is never a fallback source.
  `metadata.managedFields` is not a fallback either — it is pruned at ingest
  ([invariant 6](../CLAUDE.md)), the fixture sanitizer deletes it, and a field
  manager's name is not a contract. When the taint carries no stamp, the right
  edge is empty on this card, same as any other card with nothing to point at.
- **The scale-down taints are a separate question from the cordon clock above,
  and neither answer changes N2.** cluster-autoscaler's own client call puts
  the scale-down's unix second in the **value** of its
  `ToBeDeletedByClusterAutoscaler` taint — a different mechanism from the
  controller mirror this card now reads, so it has a timestamp regardless.
  Karpenter's `karpenter.sh/disrupted` is **not** its equivalent: it is
  declared with a key and an effect and no `value` field at all, so it carries
  no timestamp either. Neither fact reaches this card, because neither node
  reaches this screen — a node carrying either taint is cordoned with pods on
  it for the whole eviction window by design, so N2 stays silent on it no
  matter what any clock says
  ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
- Ordering: severity, then recency. `●` critical, `▲` warning — symbol *and*
  colour, never colour alone. **Cards with no age sort last inside their
  severity band**, so the empty right edges collect into a block at the bottom
  of the band instead of leaving a hole in the middle of a column of times. An
  unknown time cannot claim to be more recent than a known one, and a reader
  who sees the blanks grouped reads them as a kind of card rather than as
  missing data.
- Every string here passes the glossary test: a newcomer reads it without
  looking anything up.

## The cordon card, with and without its clock

Losing the age used to cost this card its whole argument, not just a field —
*"someone's maintenance window never closed"* was an accusation a fabricated
duration was paying for, and there was no source for that number at all. There
is one now for the common case: the node lifecycle controller stamps
`timeAdded` on the `NoSchedule` taint it mirrors from `spec.unschedulable`,
so a plain `kubectl cordon` does leave a time behind
([NOTES § D65](../NOTES.md#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)).
`Finding.timestamp` stays an `Option`, because a taint written by hand
(`kubectl taint`, client-side) still stamps nothing, and both are real cases a
reader will hit — not a happy path and a corner case:

```
▲ node-3                                          2 hours ago
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done
```

```
▲ node-3
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done
```

Same card either way — the age is the only line that changes, exactly the rule
every other card already obeys (*No number we cannot produce*, above), not a
special case invented for this one.

**The nearly-right answer was a jsonpath line, and it is worth keeping visible
because the next person will reach for it too.** `kubectl describe node` never
prints `timeAdded` — read out of the source, not inferred
([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13))
— which makes `kubectl get node node-3 -o jsonpath='{.spec.taints}'` look like
the honest choice: it is the one command that can show the exact field the age
came from. But a card's `kubectl_cmd` has to back everything the card claims,
not only the part `describe` happens to skip, and `describe` is the one that
does that: run against a live cluster, it prints `Unschedulable:      true`
(the title's claim) and the `Non-terminated Pods` table (the pod count — the
evidence, and the number
[NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
made this rule's actual trigger, present on every card whether or not the age
is). The jsonpath line prints exactly one thing —
`[{"effect":"NoSchedule","key":"...","timeAdded":"..."}]` — and backs only the
age, the optional half; on the card with `added_at: None` it backs nothing the
card claims at all. Handing a beginner raw JSON to read a date out of is also
the wrong side of [invariant 14](../CLAUDE.md).

So N2's `kubectl_cmd` is

```
kubectl describe node node-3
```

— the same command every other node-scoped card on this screen already uses —
and **the age is the one claim on this card that no kubectl command behind it
can show.** That is not a gap this file hides: it is *No number we cannot
produce* (above) applied to a command instead of a value. k8rs reads the
taint directly off the permanent Node watch; it does not pretend a terminal
command reproduces the number it just showed.

- **The count is what makes this a card at all, with or without a clock beside
  it.** **N2 fires only on a cordoned node that a drain would still have to
  move something off**, so the count is never zero here
  ([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
  · [§ D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  A cordoned node with nothing movable left is *parked*: deliberate, correct,
  and a Capacity concern rather than an outage. A node the cluster autoscaler
  is retiring draws no card either — it is cordoned with pods on it for the
  whole eviction window on purpose.
- **It costs nothing to produce.** Pods are watched permanently
  ([invariant 6](../CLAUDE.md)); the count is `spec.nodeName` matching this
  node, minus the pods a drain would skip — `phase`, the owner's kind and the
  mirror bit, all three already carried on `PodSnapshot`. No extra call, no
  extra permission, no metrics-server.
- **And it survives the trap that killed the alternative.** The reason
  `managedFields` could not supply an age is that it is pruned at ingest and
  the fixture sanitizer deletes it, so no fixture could ever test it.
  `spec.nodeName` is the opposite case: the sanitizer *refuses* a capture
  carrying foreign node names rather than rewriting them
  ([NOTES § D29](../NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)),
  so a kind capture keeps both halves of the join and the count can be asserted
  against a fixture. A number on this screen has to be *checkable*, not merely
  derivable.
- **It goes in the evidence line, not the right edge.** Right-aligned under
  `4 min ago` and `12 min ago`, a bare `2 pods` inherits their question and
  answers a different one — the reader's eye reads a column, and three rows of
  it would be two units. The evidence line is where the OOM card already keeps
  `limit 256Mi · exit 137 · 47 restarts` — an existing slot with an existing
  meaning, and no new field on the card.
- **It is not the `n of m` field either** — that field means *how many of this
  workload's pods are affected*, and none of these two is broken. `node-3`
  keeps a bare title line, or a title line with only the age added
  ([the rules above](#the-rules-this-screen-obeys)).
- **The arrow does not accuse, with or without an age beside it.** *"Allow new
  pods once the work is done"* is true whether the cordon was five minutes ago
  or five months ago: the admin standing in front of the node reads it and
  agrees, and the one who cannot remember any work reads it and has found the
  problem themselves. The tool states the lifecycle; the reader supplies the
  fact only they have. That held when the card had no number to lean on and it
  still holds now that some cards do — the sentence was never a stand-in for
  the clock, so getting the clock back is not a reason to make it one.

### Every count this card can have

Two strings, and they are the whole set:

```
2 pods here would still have to move
1 pod here would still have to move
```

**The count is what a drain would move, not what runs here** — and the wording
had to move with it. `kubectl drain` never evicts a DaemonSet pod or a static
pod, whatever flags it is given, so a node an operator drained *perfectly*
still runs kindnet and kube-proxy on kind, `aws-node` and `ebs-csi-node` on
EKS, and four static pods if it is a control-plane node. `9 pods still run
here` was a true sentence answering a question nobody asked, and it put this
card on every correctly drained node in the cluster
([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
Not counted, therefore: `Succeeded` and `Failed` pods, DaemonSet-owned pods,
and mirror (static) pods.

**The number is smaller than the pod list, so the sentence has to say why it is
smaller.** *"Still run here"* described an inventory the reader can check with
one `kubectl get pods -o wide` — and would find four where the card said two.
*"Would still have to move"* describes a computation, and it is the same
computation the next command the reader types performs: `kubectl drain` evicts
exactly these pods, so the card and the tool it is teaching agree on the
number. The conditional also keeps the card honest about intent — it does not
claim anybody is draining this node, only what draining it would still cost.

**There is still no zero, and the narrowing makes that more true rather than
less.** A cordoned node with nothing movable left does not reach this screen:
it is a finished drain nobody turned back on — parked, not broken — and Alerts
holds only what is broken right now
([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).
What is left on this card is genuinely a half-finished operation, which is
actionable at any age and is why the severity stays `▲` whether or not the
card names one. The parked node is a Capacity concern and its row there is **not yet
designed** — the Capacity report's contents are a Phase 4 decision
([analysis.md](analysis.md)); nothing here should be read as promising one.

### Under namespace scope there is no card, and the screen says so

A user who can see one namespace ([states.md](states.md)) sees a fraction of
the pods on any node — whether they passed `--namespace` or whether the
cluster-wide pod list returned 403 and k8rs fell back
([NOTES § D5](../NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter)).

**An earlier draft of this file said the evidence line is dropped and the card
falls back to three lines. That is superseded and must not be built.** It was
written while the count was decoration; the count is now the trigger. A card
with the number removed asserts *this node is half-drained* on evidence k8rs
does not have, and the far likelier outcome is quieter: `node-3` cordoned with
forty pods on it, none of them in `payments`, the count comes out zero and
**N2 never fires at all** — a missing finding on a screen that still looks
complete. Both are worse than the wrong number this file spent an age field
avoiding ([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)).

**So the rule does not run, and the screen says which check is missing.** That
is not a new mechanism: it is the degradation
[docs/architecture § Error handling](../docs/architecture.md#error-handling)
already specifies for a 403 on a secondary stream — the feature switches off
and names what it needed. The line lives in the **banner above the list** — the
same slot, and the same widget, the disconnected state already uses
([widgets.md § 2](widgets.md#2-element--widget)) — and it is drawn in
[states.md § You can only see some namespaces](states.md#you-can-only-see-some-namespaces).

Where it deliberately does **not** live:

| Not here | Because |
|---|---|
| The header row | It already carries the *cause* — `ns: payments` — and the header is one row of three zones that may never be truncated ([widgets.md § 1a](widgets.md#1a-the-header-row)). A list of switched-off checks is a sentence, and the header has no room for a sentence. |
| The help screen | `?` lists the keys this screen has, exactly and only ([help.md](help.md)). A check is not a key. |
| A greyed-out card | There is no card. Drawing a placeholder for a finding that did not fire invents a fourth severity and puts a node on the screen that may be perfectly healthy. |
| One global notice | A switched-off check is named on **the screen that would have shown its findings**. N2 is an Alerts finding, so Alerts says it; N5 is a Capacity row, so the Capacity report says it ([analysis.md](analysis.md)). This is the rule the next disabled check inherits — it keeps the Alerts banner from growing a list every time one is added. |

The sentence itself is **one string**, wrapped by whoever is drawing it — the
banner at 43 columns, the empty screen's centred block at 34, `--once` at the
same fixed wrap its findings already use ([once.md](once.md#when-a-check-could-not-run)).
If the console and `--once` ever word this differently, one of them is lying
about what was checked.

### What the width bought, and only for the card that has no age

**The ageless card still gets the gain the earlier draft of this file claimed
for every cordon card.** With no age on the line, the title has the whole 43
columns for the name, which is not cosmetic: `ip-10-0-134-201.eu-west-1.compute.internal`
is 42 of them and an ordinary EKS node name. It fits, on the card that has
nothing else to put on that line. Anything longer clips at the pane edge like
every other string — k8rs never truncates one itself
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).

**The card with an age does not get this gain** — it shares the line with
`2 hours ago` the same way `payments/web` shares its line with `4 min ago`,
and that EKS name does not fit beside any age. Exactly how much room is left,
and what happens when a name this long meets an age this card can now also
carry, is not decided here: the age column's width budget is its own
`tui-designer` box, open in `todo.md` and due before this phase closes. This
file draws the age at all; it does not draw the column's limit.

## Node down, node under pressure, and the pod a node is blocking

N2 is not the only node rule, and it was the only one drawn until now. N1 and
N3 are node cards too; N6 has no card of its own — it merges into rule 10's
workload card, whose evidence names a node
([the rules this screen obeys](#the-rules-this-screen-obeys)). All three ages
below are ordinary — not an `Option` the way N2's is — because each reads a
condition or transition that exists the moment the finding does: `Ready`'s
`lastTransitionTime` for N1, the pressure condition's own for N3,
`PodScheduled`'s for N6
([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).

### N1 — a node that stopped answering

A node's own status is current the moment it stops updating; the pods on it
are not. Every pod rule reads pod *status*, and the status of a pod whose
kubelet stopped posting is a fossil that never expires — a crashlooping pod on
a node that has been `NotReady` for ten minutes still reads `Running`, so no
pod rule ever fires for it. Without this card, Alerts says "a node is down" in
one place and nothing about the workload that is actually offline
([NOTES § D71](../NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
So the card names what was running there, not only the node:

```
● node-1                                          6 min ago
  This node has stopped responding — nothing on it can be
  trusted until it does
  payments/web and shop/api were running here (5 pods)
  → check the node itself: is it powered on and reachable?
```

A node with no pods on it when it went down is still an outage, and the
evidence line is simply absent — the same way a card drops a line it has
nothing to put on it elsewhere on this screen:

```
● node-1                                          6 min ago
  This node has stopped responding — nothing on it can be
  trusted until it does
  → check the node itself: is it powered on and reachable?
```

- **The evidence line names owners, not a bare count.** N2's count answers
  "how much would a drain move" and a number is enough; N1's job is to hand the
  reader a workload to go check, because no other card will. Up to two owners
  by name, alphabetically; past that, `payments/web, shop/api and 2 more were
  running here (9 pods)` — the count still carries the total the way N2's does.
- The action line does not promise the node is actually down — a severed
  network link reads identically to a dead machine from the API's side, and
  the card says only what is knowable from here.

### N3 — a node running low on something

```
▲ node-2                                          18 min ago
  This node is running low on disk space — Kubernetes may start
  evicting pods to free it up
  → free up disk space on this node, or move some pods elsewhere
```

Three conditions share this shape; only the resource named in the sentence and
the action change:

| Condition | "This node is running low on…" | "→ …" |
|---|---|---|
| `DiskPressure` | disk space | free up disk space on this node, or move some pods elsewhere |
| `MemoryPressure` | memory | free up memory on this node, or move some pods elsewhere |
| `PIDPressure` | process IDs | find what is creating so many processes, or move some pods elsewhere |

If more than one condition is `True` at once, name all of them, joined with
"and" — `running low on disk space and memory` — rather than picking one and
hiding the other.

### N6 — folded into rule 10's card, not a card of its own

**Corrected after this file first shipped: N6 does not produce a second
Finding.** Rule 10 (`no_node_accepted_it`) already fires on exactly N6's
population — `PodScheduled=False`, `reason=Unschedulable` — so a Pending pod
that can never be placed was already a card before N6 existed. Two Findings
for one pod is the flood [NOTES § D28](../NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
exists to prevent, so N6 is not a rule that draws its own card: it is the
node-side half of rule 10's — which taint or `nodeSelector` is doing the
blocking, folded into the one Finding rule 10 already produces. **The title,
the age and the explanation line are rule 10's, unchanged; N6 supplies the
first half of the evidence line and the action.** The severity is rule 10's
ladder too — Critical unless the pod is still inside its scheduling grace
window — not a fixed `▲`.

This is the shipped card, real strings, from a committed capture (a bare pod,
so there is no owner and no `n of m`):

```
● default/broken-pending                          3 hours ago
  No machine in the cluster will take this
  pod, so it has never started (it shows as
  Pending)
  it asks for a node labelled disktype=ssd,
  and none of the 4 nodes have that label ·
  the scheduler's own words (a node is one
  machine): 0/4 nodes are available: 1
  node(s) had untolerated taint(s), 3 node(s)
  didn't match Pod's node affinity/selector. …
  → change the nodeSelector, or label a node
  disktype=ssd
```

And the taint-blocked cause, same merge, an owned workload this time:

```
● data/migrate-job  ·  1 of 1 pods         2 hours ago
  No machine in the cluster will take this
  pod, so it has never started (it shows as
  Pending)
  node-2 and node-3 are tainted gpu=true, and
  this pod does not tolerate that taint ·
  the scheduler's own words (a node is one
  machine): 0/3 nodes are available: 2
  node(s) had untolerated taint {gpu: true},
  1 node(s) didn't match Pod's node
  affinity/selector.
  → add a toleration for gpu=true, or remove
  the taint
```

- **The evidence is `·`-joined, N6's sentence first, the scheduler's quote
  after it — never one or the other.** The quote is the only place every
  *other* reason the scheduler rejected a node shows up, and
  [NOTES § D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
  requires it kept verbatim; N6's sentence is the one reason plain language
  can commit to. Dropping either loses something a reader needs — the
  paraphrase without the quote hides that a fourth reason might exist too, and
  the quote without the paraphrase hands a beginner `0/4 nodes are available`
  with no translation.
- **This card runs past the three-to-five lines every other card holds to.**
  It is not a special exemption invented for N6 — it is the same collision
  rule 3's verbatim runtime message already has with that budget, still open
  in `todo.md`'s cordon-card-round box (wrap, truncate-with-full-text-on-`⏎`,
  or a card allowed to be tall). This section draws the wrap so the collision
  is visible; it does not resolve it, and neither card above should be read as
  the answer.
- **Ordering, when more than one cause is present: an unsatisfiable
  `nodeSelector` is named first, then a taint every candidate node shares.** A
  taint on only some of the nodes that rejected the pod is not the reason it
  is stuck — the other candidates failed for some other reason — and a card
  that named it anyway would send the reader to fix half a problem.
  `PreferNoSchedule` is never named: the scheduler can overrule it, so a node
  carrying only that taint is not what is blocking the pod.
- **`kubectl_cmd` stays the pod's**, `kubectl get pod broken-pending -n
  default -o yaml` — the subject is the pod and the fix edits the pod's spec
  (`nodeSelector`, `tolerations`), never `kubectl get nodes --show-labels`.
  Rule 10 already reads the pod for the scheduler's message; N6 adds no
  second read and no second command.
- The age is the pod's own `PodScheduled` condition going `False`, never the
  blocking node's taint `added_at` — the card is about the pod's wait, not the
  node's history, and the two clocks answer different questions even when a
  reader could reach for either.

## Empty state

See [states.md](states.md) — an empty Alerts screen says *"nothing is broken
right now"*, and it has to be true, or the whole product is noise. When a check
could not run, that claim is qualified on the same screen rather than made
smaller —
[states.md § nothing broken, and something not checked](states.md#nothing-broken-and-something-not-checked).
An empty list with a switched-off rule behind it is the one screen this tool
cannot afford to draw silently.
