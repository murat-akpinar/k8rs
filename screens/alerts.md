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
│   versions         │  ▲ node-3                                     │
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
| That field is **right-aligned, not trailing** | It ends two columns before the pane's right edge — `4 min ago` and `12 min ago` both stop at the same column, and a longer or shorter age moves its left end, never its right. The mockup used to disagree with itself here, because the one age that ran flush to the border was `6 days ago` on the cordon card, and that age no longer exists. |
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
- **The cordon card carries no age, and this is the case that proves the rule
  above.** N2 fires on `spec.unschedulable` — a bare boolean. **`kubectl
  cordon` records no time a client can read**, and neither does anything else
  that simply flips that field: the taint it adds is `NoSchedule`, and
  `Taint.timeAdded` is *"only written for NoExecute taints"*; no node condition
  transitions on a cordon either, so `Ready` keeps the `lastTransitionTime` it
  had before. The right edge is therefore empty on this card.
  `metadata.managedFields` is not a way out — it is pruned at ingest
  ([invariant 6](../CLAUDE.md)), the fixture sanitizer deletes it, and a field
  manager's name is not a contract.
- **The claim is about `kubectl cordon`, not about Kubernetes.** One writer
  *does* record the moment: cluster-autoscaler puts the scale-down's unix
  second in the **value** of its `ToBeDeletedByClusterAutoscaler` taint, and
  Karpenter's `karpenter.sh/disrupted` is the equivalent. That is no help here,
  and the reason is the same one that keeps those nodes off this screen
  entirely — a node carrying either taint is cordoned with pods on it for the
  whole eviction window by design, so N2 stays silent on it
  ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  The one path that has a timestamp is the one path with nothing to stamp.
- Ordering: severity, then recency. `●` critical, `▲` warning — symbol *and*
  colour, never colour alone. **Cards with no age sort last inside their
  severity band**, so the empty right edges collect into a block at the bottom
  of the band instead of leaving a hole in the middle of a column of times. An
  unknown time cannot claim to be more recent than a known one, and a reader
  who sees the blanks grouped reads them as a kind of card rather than as
  missing data.
- Every string here passes the glossary test: a newcomer reads it without
  looking anything up.

## The cordon card, and what replaced its clock

Losing the age cost this card its whole argument, not just a field. *"Someone's
maintenance window never closed"* was an accusation the duration was paying
for; with no duration behind it, the tool would be asserting *forgotten* about
a node an admin cordoned forty seconds ago and is standing in front of right
now. A false alarm on the screen whose entire promise is *only what is broken*
is worse than a missing number.

So the card drops the clock and says what is true without one — what the node
is doing, and how much is riding on it:

```
▲ node-3
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done
```

- **The count is the signal the duration was standing in for**, and it is a
  better one. Six days is only alarming if something depends on the node; work
  still sitting on it is alarming on its own. It is also the number that decides
  whether there is a card at all — **N2 fires only on a cordoned node that a
  drain would still have to move something off**, so the count is never zero
  here ([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
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
  keeps a bare title line ([the rules above](#the-rules-this-screen-obeys)).
- **The arrow stopped accusing and started assuming.** *"Allow new pods once
  the work is done"* is true whether or not there is any work: the admin who
  cordoned it five minutes ago reads it and agrees, and the one who cannot
  remember any work reads it and has found the problem themselves. The tool
  states the lifecycle; the reader supplies the fact only they have.

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
actionable at any age and is why the severity stays `▲` without a clock behind
it. The parked node is a Capacity concern and its row there is **not yet
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

### What the width bought

With the age gone the title line has the whole 43 columns for the name, which
is not a cosmetic gain: `ip-10-0-134-201.eu-west-1.compute.internal` is 42 of
them and an ordinary EKS node name. It fits now and did not before. Anything
longer clips at the pane edge like every other string — k8rs never truncates
one itself ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).

## Empty state

See [states.md](states.md) — an empty Alerts screen says *"nothing is broken
right now"*, and it has to be true, or the whole product is noise. When a check
could not run, that claim is qualified on the same screen rather than made
smaller —
[states.md § nothing broken, and something not checked](states.md#nothing-broken-and-something-not-checked).
An empty list with a switched-off rule behind it is the one screen this tool
cannot afford to draw silently.
