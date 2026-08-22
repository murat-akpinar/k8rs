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
│   posture          │                                               │
│   restarts         │  ▲ node-3                        2 hours ago  │
│   waste            │    This node refuses new pods (cordoned)      │
│   versions         │    2 pods here would still have to move       │
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
| Finding card | four parts, in this order and only this order: **who** (the identity line, with the age) · **what happened** · **the evidence** · **what to do**. Three to twelve lines, wrapped and capped by [How wide a card is, and how tall](#how-wide-a-card-is-and-how-tall). Title bright, evidence dim. Blank line between cards — half the design. |
| The card's right edge | **when it happened, or nothing.** It is the only right-aligned field on the title line, and it holds one fact: the time of the event this card is about. A card whose finding has no such time leaves it empty rather than borrowing a nearby timestamp that answers a different question (see *No number we cannot produce*). |
| That field is **right-aligned, not trailing** | It ends two columns before the pane's right edge — `4 min ago`, `12 min ago` and `2 hours ago` all stop at the same column, and a longer or shorter age moves its left end, never its right. It is laid out **first**, at its measured width, and the name takes what is left; **at most 14 columns**, which is the whole budget ([How wide a card is, and how tall](#how-wide-a-card-is-and-how-tall)). The mockup used to disagree with itself here, because an age once ran flush to the border with no source behind it (`6 days ago` on the cordon card) — that string is gone; the column itself was never the problem, and the cordon card uses it again below, honestly this time ([the cordon card](#the-cordon-card-with-and-without-its-clock)). |
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

## How wide a card is, and how tall

**The frame mockups on this page are drawn 70 columns wide, the page
convention ([README](README.md#how-to-read-them)). Every number in this section
is the real one, at the 80×24 floor, and the loose cards below are drawn to
it.** The two differ by ten columns, and a card wrapped at the page width is one or
two lines taller than the same card on a real terminal — which is why the
budget is stated here in terminal columns and nowhere in a drawing.

### The columns

| At 80×24 | Columns | From |
|---|---|---|
| Terminal | 80 | the minimum k8rs supports ([widgets.md § 8](widgets.md#8-smaller-than-8024)) |
| Content pane, inside its borders | 57 | 80 − 2 borders − 20 sidebar − 1 divider |
| **Card region** | **53** | the pane less a two-column pad each side |
| Identity line, after `● ` | 51 | the card region less the severity marker |
| Body text | 51 | body lines indent two columns, under the text after `● ` |
| Action continuation text | 49 | continuations indent two more, under the text after `→ ` |

### The age, and what it costs the name

The age is laid out first, at the width the ladder gives it
([widgets.md § 1b](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)),
right-aligned so its last column is the card region's. The name gets the rest,
minus a two-column gap, and clips there.

| The card's age | Age columns | Columns left for the name |
|---|---|---|
| none | 0 | **51** |
| `4 min ago` | 9 | 40 |
| `2 hours ago` | 11 | 38 |
| the widest the ladder can draw | **14** | **35** |

**35 columns of name is the floor, and 51 is what an ageless card gets.**
That is the whole of the age column's budget: it is never reserved when it is
not used, never truncated when it is, and the name is what gives way. Nothing
clamps the age at 14 — the days rung has no upper bound and a wider string
simply takes one more column from the name — but 14 is the widest one a cluster
can reach, so 35 is the number to design the identity line against.
`ip-10-0-134-201.eu-west-1.compute.internal` is 42 columns and an ordinary EKS
node name — it fits on a card with no age and on no card with one, which is
stated again where the cordon card needs it
([What the width bought](#what-the-width-bought-and-only-for-the-card-that-has-no-age)).

**`· n of m pods` gives way before the name does.** The identity line drops it
first, then clips the name. A name half-read still tells you which workload;
a fraction with no workload attached tells you nothing.

### The height

The body pane is **16 rows** at 80×24, and there is only one way to spend the
24: 1 header row + 1 top border + **16 body** + 1 divider + 2 command log +
1 divider + 1 footer + 1 bottom border. Count them in the frame below; nothing
in that list is optional.

**80×24 is the worst case in both directions, and every cap below is stated
against it.** A taller terminal adds rows to the body and nothing else; a wider
one widens the card region, so the same strings wrap to *fewer* lines and every
card gets shorter. Below the minimum there is no layout at all — one sentence
saying so ([widgets.md § 8](widgets.md#8-smaller-than-8024)) — so there is no
size at which a card is worse than what is drawn here.

A card is filled in one order, and only the evidence is ever cut:

| # | Part | Lines | May it be cut? |
|---|---|---|---|
| 1 | identity — `● name` (`· n of m pods`) age | exactly 1, never wraps | the name clips, the age never |
| 2 | what happened (`title`) | wrapped, drawn whole — **capped at 3 lines** | no |
| 3 | the evidence | wrapped, **capped at 3 lines** | **yes — this one only** |
| 4 | `→ ` what to do (`action`) | wrapped, drawn whole — **capped at 5 lines** | no |

**Those four caps add to twelve, and twelve is the card cap.** Every number in
the table is a real one a rule reaches today; 1 + 3 + 3 + 5 is what a card
measures when all four do at once. The caps and the total are one statement and
are read off each other, which is the thing this section did not do until
2026-08-16 — it wrote the parts and the total independently, and they disagreed
by two rows.

- **The action is never cut.** A fix the reader cannot finish reading is not a
  fix, and the long ones are long because each answers a question the reader
  actually has — rule 8's grew in review for exactly that reason
  ([NOTES § D79](../NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)).
  Cutting one to fit was considered and rejected: that trades a real answer for
  a layout. **Shortening one is not the same thing and is not refused** — the
  three clean-exit arms came down from 9 / 8 / 9 wrapped lines to 5 / 5 / 5 with
  all three of their doors still open, and what came out was the preamble and
  the restatements
  ([NOTES § D90](../NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
  *Never cut* is a rule about how a card is built — no rule's fix arrives
  half-written. Five lines is a budget the author writes to. They are not the
  same instrument and they do not contradict each other.
- **The action is k8rs's own words, always — a string the cluster wrote is
  never one.** A controller's or a runtime's message is evidence, and it goes on
  the evidence line with every other quote, behind the three-line cut. This is
  the rule that makes the five-line budget enforceable at all: an author can
  measure what they wrote, and nobody can measure what a runtime will write.
  **Measured, and reachable by a typo:** a container whose `command` names a
  path that is not in the image carries containerd's whole `runc` error, which
  is 7 wrapped lines — *"Kubernetes recorded this: failed to create containerd
  task: failed to create shim task: OCI runtime create failed…"* standing where
  the instruction belongs, telling a beginner nothing to do
  ([invariant 14](../CLAUDE.md)). Two consequences, both deliberate: the card
  keeps the runtime's exact words, one line lower and cut like any other quote;
  and **a rule with nothing useful to say must still say something in its own
  voice** — the action slot is never empty, and it is never filled by borrowing
  from the API. That keeps the card's four parts four different jobs, which is
  the whole reason it has four.
- **The evidence is cut because it is the only unbounded thing on the card**,
  and the bullet above is what makes that true rather than nearly true.
  Everything else was written by a rule author and is as long as they made it;
  the evidence carries a controller's sentence quoted verbatim, which
  [NOTES § D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
  requires be kept word for word and which no author bounds. Measured on the
  run: rule 3's evidence is **422 characters** — it was 347 when this line was
  first written, and it grew because the registry did, not because a rule
  author typed anything — and rule 10's after N6's merge is **358**. Three lines
  is 150-odd of them.
- **Three lines, not two, and the number was measured rather than chosen.** At
  two, rule 10's card cuts at *"· the scheduler's…"* and the reader never sees
  one word of the message the rule went and fetched. At three the quote's
  opening is on screen, which is the point of quoting it.
- **The cut walks back to a whole word, then adds `…`.** A single token longer
  than the line is broken by character instead, because there is no word
  boundary to find — and that is not hypothetical: rule 3's evidence on the
  committed capture contains
  `"https://registry.invalid/v2/does-not-exist/manifests/v9":`, **58 columns**,
  which is wider than the 51 the card has at the floor. Wrapping alone cannot
  make that line fit; only a character break can. The
  `…` is the only thing on any screen k8rs truncates on purpose
  ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
- **The full text is one `⏎` away**, on the object's detail screen, where the
  finding that brought you there is pinned ([detail.md](detail.md)). That is
  what makes the cut honest: nothing is lost, it is one keypress deeper.
- **The title is capped at three lines, and that cap is new.** It was capped by
  implication only until 2026-08-16 — the table said *1 or 2*, so a title's
  third line spent a row nothing had reserved, and that is not hypothetical:
  a serving clause at a three-digit restart count once made a 3-line title and
  an 11-line card, caught by an operator review and by nothing in the repo
  ([NOTES § D95](../NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)).
  **Three, not two, and the number is a measurement:** of the 69 distinct titles
  the rule set prints on the run, **three wordings** wrap to three lines, and
  every one is a fixed frame plus a translation — rule 6 spends its title on
  *what `exit 137` means*, which is invariant 14 doing its job. Cutting those to
  two lines would cut the translation, which is the trade this section refuses
  everywhere else.
- **The title is the one part of a card that two authors write and nobody may
  cut, and neither author can see the other's length.** A rule picks the
  frame — *"The last run on record has no
  exit code of its own — "* — and `exit_meaning` writes the translation that
  finishes it, for a code, not for a frame. Rule 6 has three frames — **32, 53
  and 58 columns** — and any of them can pick up any translation, so the same
  sentence costs a different number of lines depending on which one does. **That is
  where a third line comes from, and it is why this cap is worth writing down
  rather than observing.** Nothing here asks a rule to shorten a translation:
  explaining `exit 255` to someone who has never seen it is the card's job
  ([invariant 14](../CLAUDE.md)).
- **No title the rules can draw measures four today, and the margin is four
  columns.** Measured against the landed code: the tightest one a cluster can
  produce is the `-1` a runtime writes when it could not read an exit code,
  which finishes its third line with **4 free columns**; the `255` beside it has
  6, and `137`'s frames leave 8 to 12. **One word is the whole of that margin** —
  the `255` translation carried *already* until 2026-08-16 and measured four
  lines; the word came out and the title came back inside the cap. The word was
  never the defect, the missing margin was.
- **So a translation is measured against every frame that can reach it**, not
  against the one it was written for — and the sweep is the argument. Pair rule
  6's three frames with the twelve endings `exit_fact` can print and **5 of the
  36 combinations measure four lines**, with a sixth landing on its third line
  with zero columns to spare. Not one of those six is on a screen, and what
  keeps them off is not a budget: it is `ending`, pairing `137` with one frame
  and `255` with another for reasons that have nothing to do with layout. Move a
  pairing there — the kind of change this phase has already made more than
  once — and a card goes over with nobody having touched a word of its wording.
- **A card is three to twelve lines, and 12 is the parts added up — 10 was one
  drawing measured and then written down as if it bound every card.** Three is
  the floor a card with no evidence and one line of each other part reaches;
  four is the shortest the rule set actually produces today (N2's, drawn below).
  The old number came off the unschedulable pod's card, which was 1 identity +
  **1** title + 3 evidence + 5 action when it was written down — and every card
  with an ordinary two-line title has measured 11 ever since, inside every part
  budget this file states and over the total it states. **That is the whole
  finding, and it is arithmetic:** 1 + 2 + 3 + 5 is 11, and 1 + 3 + 3 + 5 is 12.
  (That card was also miscredited to rule 11 here for as long as the number
  stood; it is rule 10's, and rule 11 is the probe-failure card.)
- **Twelve is also exactly what the pane can afford, so the promise gets sharper
  rather than weaker.** The body is 16 rows: 12 for the tallest card, 1 for the
  blank separator, and **3 left — the next finding's identity line and two lines
  of its title**. Not *a second card is somewhere below*: its name and what is
  wrong with it are both on screen, at the worst card the rule set can draw,
  with no rows to spare. That is the property the cap exists to hold, it is
  checkable by looking at the screen, and the frame below is drawn at it. A
  screen that can show only one finding is not a list, and Alerts is a triage
  list before it is anything else.
- **The cap is the card's, and the pane obeys nothing.** The *pane* cuts
  whatever does not fit, as it always has. **The state where that bites is a
  banner**, which shares the body pane with the list: the widest one drawn takes
  **ten of the sixteen rows** — eight lines, the blank between its two
  paragraphs, and the blank above the list
  ([widgets.md § 2](widgets.md#2-element--widget),
  [states.md](states.md#you-can-only-see-some-namespaces)). Six rows left is
  identity + 3 title + 2 evidence, so **a twelve-line card's action is entirely
  off screen** — and no cap fixes that, because the banner is telling the reader
  something they need more than the first card's fix. What the screen owes them
  instead is that the list still moves: `ListState` keeps the selected card in
  view, so `↓` reaches the action rather than scrolling past it, and `⏎` has the
  whole finding pinned ([detail.md](detail.md)). **This is not a 403's problem
  alone** — the namespace-scope banner is the ordinary state for anyone who is
  not a cluster admin.
- **What keeps this bounded is not the cap, it is D3.** Findings group by owner
  ([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)), so a
  DaemonSet broken on forty nodes is one twelve-line card and not four hundred
  lines.
- **An action that wraps past five lines is a `rules.rs` finding, not a layout
  problem** — and that sentence has been in this file for weeks while four
  actions broke it, so here is what a rule author measures against. **Five lines
  is 232 to 245 characters** at the 49 columns a continuation has; the spread is
  where the words happen to break. **Measured on the run, not estimated**
  (`cargo test -- --nocapture`, every distinct `→ ` line wrapped at 49): of 44
  distinct author-written actions, **4 are over** — `stopped_action`'s two arms
  at 6 lines, rule 5's no-record arm at 6, and `failed_action(Init)` at **8**,
  which draws a 14-line card. The fifth string that measured over is the runtime
  quote two bullets up, and it is not an action at all. **What that costs is
  40 · 36 · 18 · 105 characters**, in that order. Nothing here says which words
  go: that is `rules.rs`'s, and D90 is the proof it can be paid without closing
  a door.
- **Ten more actions sit *at* five lines, and the cap does not move for them
  either.** The two clean-exit arms D90 rewrote have **3 and 0 free columns** on
  their last line — so a later box that wants to add four or six characters to
  one of them is asking for a row that does not exist, and the words come out of
  the sentence rather than out of the number. That is the whole point of writing
  a budget down instead of reading it off whatever shipped last.

The tallest card the budget allows, in the frame, at the floor it is budgeted
for — 12 rows, the separator, and the three the next finding gets. **Three card
shapes are over it today**, all of them the same eight-line action, so this is
the shape the rule set is coming back to rather than one it is safely inside:

```
 nodes 3/3                          k8rs         ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ▲ default/healthy-sidecar                   5 min ago  │
│ RESOURCES          │    Container has been restarted 3 times — it is         │
│   workloads        │    serving now, and the last run on record finished     │
│   network          │    cleanly                                              │
│   storage          │    sidecar container proxy (it runs beside the app the  │
│   config           │    whole time) · exit 0 (the run ended without an       │
│   cluster          │    error) · docker.io/library/busybox:latest            │
│ ANALYSIS           │    → exit 0 says the run ended, not who stopped it —    │
│   capacity      1 ▲│      check the pod's events for a Killing line and the  │
│   certificates  30d│      node for a memory killer. If nothing stopped it    │
│   drain safety     │      the program ends itself, and this one must run as  │
│   posture          │      long as the app does: finishing at all is the bug  │
│   restarts         │                                                         │
│   waste            │  ▲ shop/api  ·  2 of 6 pods                 12 min ago  │
│   versions         │    Running, but not receiving traffic — the readiness   │
│                    │    check is failing                                     │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                                │
│ $ kubectl get nodes --watch                                                  │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit              │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Every string on that card is the run's own**, `default/healthy-sidecar` off
the committed captures: 1 identity + 3 title + 3 evidence + 5 action, the four
caps all met at once, and the 16 body rows spent exactly. It is a `▲` and the
card below it is too, which is not a coincidence to build on — the cap is
severity-blind, and the tallest shape is whichever rule reaches all four caps.
**Its evidence fills the three lines without needing the cut**, so no `…`
appears here; that string simply ends where the cap is. The `→ ` costs the
action two columns on its first line, and continuations indent under the text
rather than under the arrow.

`shop/api` gets three rows and that is the floor, not a comfortable margin: its
name, its count, its age, and the whole of *what is wrong with it*. **Its title
fits in two lines. A three-line title in that slot loses its last line to the
pane** — which is the pane cutting, not the card, and is the one place a title
is ever short of what its rule wrote. Nothing below is lost either way, because
the pane scrolls ([widgets.md § 4](widgets.md#4-scrolling)); what the three rows
buy is a reader who has not yet touched a key seeing two findings instead of
one.

### The cards the budget was measured against

Real strings, off the run, at the card region's own width rather than the
frame's. The tallest card is the one in the frame above; these three measure
**10, 9 and 9**, and each gets there a different way. The first is where the
quote gets cut — 422 characters of it, the longest string any rule fetches. The
second is the pod nothing will schedule, where the quote is the scheduler's own
verdict. The third has **no age at all**, because
rule 8 describes a standing property rather than an event
([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)),
so its name has the whole 51 columns and the right edge stays blank. It is a
bare pod, so no owner and no `n of m`, the same as N6's first card below. The
fourth is not a measurement — it is the shape all of this exists to leave alone.

**The first card is the arithmetic in one drawing.** It has the ordinary
two-line title almost every card in the rule set has, it reaches ten lines, and
its action is **four** lines — not five. That is what a ten-line cap actually
bought at a two-line title, while this file went on promising five. The number
was read off the tallest drawing on the page rather than off the parts, and then
the drawing moved: the pod that will not schedule was 1 identity + 1 title + 3
evidence + 5 action when the ten was written down, and its rule has since
reworded the title to two lines and the action to three.

```
● payments/web  ·  2 of 5 pods              4 min ago
  Container image is not usable, so the container
  never started (ErrImagePull)
  container nope · image
  registry.invalid/does-not-exist:v9 · failed to pull
  and unpack image…
  → check the image name and tag, whether this
    namespace has a pull secret for that registry,
    and whether the pull policy lets the node fetch
    it at all
```

```
● default/broken-pending                    9 min ago
  No machine in the cluster will take this pod, so it
  has never started (it shows as Pending)
  the scheduler's own words (a node is one machine):
  0/4 nodes are available: 1 node(s) had untolerated
  taint(s), 3 node(s) didn't match Pod's node…
  → check what this pod asks for: the node labels it
    selects, which machines it says it can run on,
    and how much cpu and memory it requests
```

```
● default/broken-hostpath
  A container can drive the container runtime, which
  is full control of that machine
  container nosy · /run/containerd on the node ·
  writable
  → remove the mount, unless this pod's job is to
    manage or watch the containers on the node — if
    it is, it already has full control of every node
    it runs on
```

And the shape the whole budget exists to protect — most cards are still four
lines, and nothing above makes them taller:

```
▲ node-3                                  2 hours ago
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done
```

### The card the runtime writes, and what moving its quote costs

The one card whose *action* was a string from the cluster. It is
`default/broken-restarts10` on the committed capture — a container whose
`command` names a path the image does not have, so containerd fails to start it
and writes its own error onto the record. **The first drawing is what ships
today** — 10 rows, and the reader is told nothing to do:

```
▲ default/broken-restarts10                2 days ago
  The last run on record failed — exit 128
  container flaky
  → Kubernetes recorded this: failed to create
    containerd task: failed to create shim task: OCI
    runtime create failed: runc create failed: unable
    to start container process: error during
    container init: exec: "/definitely-not-here":
    stat /definitely-not-here: no such file or
    directory
```

And the second is the same object under the rule two sections up — the quote on
the evidence line, cut at three like every other quote, and the action in k8rs's
own voice, 9 rows:

```
▲ default/broken-restarts10                2 days ago
  The last run on record failed — exit 128
  container flaky · Kubernetes recorded this: failed
  to create containerd task: failed to create shim
  task: OCI runtime create failed: runc create…
  → the container never got as far as running, so it
    has no log of its own — check the command and
    arguments in the pod's spec against what is
    actually in the image
```

- **That action is the geometry's sentence, not the rule's** — the same footing
  W1's and W2's are drawn on below. `rules.rs` owns the wording and may differ;
  what is settled here is the slot, and that an action of two to five lines
  keeps this card between 7 and 10.
- **The cut costs the reader the useful half, and that is stated rather than
  hidden.** containerd puts its diagnosis last — `exec:
  "/definitely-not-here": stat …: no such file or directory` — so three lines
  leave them holding the frame and not the fact. It is not reorderable:
  [NOTES § D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
  keeps a controller's sentence word for word, and *load-bearing fact first*
  can only order the facts k8rs joined, never the inside of a quote. **What
  answers it is the action** — a sentence that names the container's `command`
  as the thing to look at gets the reader there without the quote — and the full
  text one `⏎` away ([detail.md](detail.md)), which is the standing answer for
  every quote this screen cuts.
- **This is the trade, said plainly:** the old card handed over a complete
  runtime error and no instruction; the new one hands over a complete
  instruction and a cut runtime error. Only one of those two is a thing the
  reader cannot get anywhere else on this screen, and it is not the quote.

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
▲ node-3                                  2 hours ago
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

### What the number may say, and what it may not

**The right edge carries the ladder's ordinary string — `2 hours ago`, spelled
exactly as it is on every other card.** No `about`, no `~`, no asterisk and no
footnote marker. The column is shared by every rule that has an age and it
means one thing; a rule that spelled its own differently would be inventing a
second column in the same place, and the reader has no legend for it.

**Everything else on the card is unchanged, and that is the answer, not an
omission.** The title, the count and the arrow are the strings that shipped
before the clock came back, and the clock is not a reason to rewrite them:

| The card | Before the clock | Now |
|---|---|---|
| `This node refuses new pods (cordoned)` | the title | unchanged |
| `2 pods here would still have to move` | the trigger, and the evidence | unchanged |
| `→ allow new pods once the work is done` | the action | unchanged |
| right edge | blank | `2 hours ago`, when the taint carries a stamp |

**Nothing on this card reasons from the age.** It is not repeated in the body,
the action does not mention it, and the severity does not depend on it. That
rule is what makes the number safe to print, because of what the number
actually is: `timeAdded` dates **the taint, not the cordon**. Anything that
rewrites `node.spec.taints` wholesale — `kubectl edit`, a GitOps controller
reconciling Node objects, a manifest re-apply — makes the controller stamp a
fresh time on a cordon that is days old
([NOTES § D65](../NOTES.md#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13) ·
[§ D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).

**So the number can only be too small, never too large — it is a floor on how
long this has been cordoned.** That is the safe direction for the only thing a
reader does with it (*"has this been sitting since yesterday's change
window?"*): a card reading `2 hours ago` on a three-day-old cordon makes the
reader look, and a card that over-reported would make them accuse. And it is
precisely why *"somebody's maintenance window never closed"* does not come
back. That sentence was deleted for having no number behind it
([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12));
it stays deleted for having the wrong kind of number behind it. A card that
argues from a figure which can be short by three days is the same accusation
with a citation stapled to it.

**The one thing the clock does change is the sort.** A cordon card with a stamp
sorts by recency inside the `▲` band like any other card; one without still
falls into the ageless block at the bottom of the band
([the rules above](#the-rules-this-screen-obeys)). Both cases are ordinary, and
neither is the exception.

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
banner at the content pane's width less its two-column pads (**53 at 80×24**,
and 43 in the 70-column drawings on these pages, the same two numbers the card
region has: [How wide a card is, and how tall](#how-wide-a-card-is-and-how-tall)),
the empty screen's centred block at 34, `--once` at the
same fixed wrap its findings already use ([once.md](once.md#how-wide-the-report-is-and-why-nothing-in-it-is-cut)).
If the console and `--once` ever word this differently, one of them is lying
about what was checked.

### What the width bought, and only for the card that has no age

**The ageless card still gets the gain the earlier draft of this file claimed
for every cordon card.** With no age on the line, the name has the whole **51
columns**, which is not cosmetic:
`ip-10-0-134-201.eu-west-1.compute.internal` is 42 of them and an ordinary EKS
node name. It fits, on the card that has nothing else to put on that line.

**That number used to read 43, and 43 was wrong twice over.** It was the card
region rather than the name — the severity marker takes two of it — and it was
measured off the 70-column page drawing instead of the 80-column terminal this
screen is budgeted for. The real pair is 53 and 51, at the floor
([How wide a card is, and how tall](#how-wide-a-card-is-and-how-tall)). The
conclusion the old number was reaching for survives intact, which is the only
reason it went unnoticed: the EKS name fits, and it fits by nine columns rather
than by one.

**The card with an age does not get this gain**, and now there is a number for
how much it loses: the name gets 51 less the age and a two-column gap — 38
beside `2 hours ago`, and **35 in the worst case the ladder can produce**. That
EKS name fits beside no age at all. Anything that does not fit clips at the
pane edge like every other string — k8rs never truncates one itself, and the
card's one deliberate cut is on the evidence line rather than here
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).

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
So the card names what was placed there, not only the node:

```
● node-1                                    6 min ago
  This node has stopped responding — nothing on it
  can be trusted until it does
  payments/web and shop/api were placed here (5 pods)
  → check the node itself: is it powered on and
    reachable?
```

A node with no pods on it when it went down is still an outage, and the
evidence line is simply absent — the same way a card drops a line it has
nothing to put on it elsewhere on this screen:

```
● node-1                                    6 min ago
  This node has stopped responding — nothing on it
  can be trusted until it does
  → check the node itself: is it powered on and
    reachable?
```

- **The evidence line names owners, not a bare count.** N2's count answers
  "how much would a drain move" and a number is enough; N1's job is to hand the
  reader a workload to go check, because no other card will. Up to two owners
  by name, alphabetically; past that, `payments/web, shop/api and 2 more were
  placed here (9 pods)` — the count still carries the total the way N2's does.
- The action line does not promise the node is actually down — a severed
  network link reads identically to a dead machine from the API's side, and
  the card says only what is knowable from here.

### N3 — a node running low on something

```
▲ node-2                                   18 min ago
  This node is running low on disk space — Kubernetes
  may start evicting pods to free it up
  → free up disk space on this node, or move some
    pods elsewhere
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
● default/broken-pending                  3 hours ago
  No machine in the cluster will take this pod, so it
  has never started (it shows as Pending)
  it asks for a node labelled disktype=ssd, and none
  of the 4 nodes have that label · the scheduler's
  own words (a node is one machine): 0/4 nodes are…
  → change the nodeSelector, or label a node
    disktype=ssd
```

And the taint-blocked cause, same merge, an owned workload this time:

```
● data/migrate-job  ·  1 of 1 pods        2 hours ago
  No machine in the cluster will take this pod, so it
  has never started (it shows as Pending)
  node-2 and node-3 are tainted gpu=true, and this
  pod does not tolerate that taint · the scheduler's
  own words (a node is one machine): 0/3 nodes are…
  → add a toleration for gpu=true, or remove the
    taint
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
- **This card used to run to twelve lines, and it is eight now — the quote is
  cut, not the sentence.** Both halves of the evidence are still there:
  N6's translation in full, then as much of the scheduler's own words as three
  lines hold, then `…`. That is the general answer
  ([How wide a card is, and how tall](#how-wide-a-card-is-and-how-tall)), not an
  exemption written for N6 — rule 3's verbatim runtime message collides with
  the budget in exactly the same place and is cut by exactly the same rule.
  What the reader loses on this screen they get by pressing `⏎`, where the
  whole message is pinned above the object ([detail.md](detail.md)).
- **The cut point is worth reading, because it is the argument for three lines
  rather than two.** At three, `0/4 nodes are…` is on the card: the reader can
  see the scheduler counted four machines and refused all four. At two the card
  stops at *"the scheduler's…"* and the quote D37 insisted on is represented by
  its introduction alone.
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
- **A one-node cluster gets its own grammar:** `it asks for a node labelled
  disktype=ssd, and the cluster's one node does not have that label` —
  `none of the 1 node have that label` is the same defect plain language
  already fixed for restart counts, and kind, minikube, k3s and Docker
  Desktop all run exactly one node.

## W1 and W2 — the two cards whose subject is not a pod

Every card above is about a pod or a machine. These two are about a
**workload object**: W1 reads the ReplicaSet's `ReplicaFailure` condition —
Kubernetes was asked to make the pods and something refused — and W2 reads a
Deployment whose rollout ran out of patience. Both matter because **the pods
they are about do not exist**, so no pod rule can fire and the screen would
otherwise be empty while nothing works
([NOTES § D28](../NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)).
That is also the whole of W2's warrant, and it is why W2 gives the screen back
the moment some other card has already explained the silence — see
[W1 suppresses W2](#w1-suppresses-w2-and-the-screen-shows-one-card) at the end
of this section.

### Two numbers, one forbidden and one expected

**Neither card carries an `n of m` — and both carry a counter**, in three forms,
with one case that carries none. Those are two different numbers off two
different sources, and reading *"no `n of m`"* as a ban on counting anything
anywhere on the card is how the second one gets left off.

- **Forbidden: `· n of m pods` on the identity line.** That field counts
  distinct pods in the group, and these two findings' objects are a ReplicaSet
  and a Deployment
  ([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
  A card reading `0 of 3 pods` up there would be arithmetic dressed as a fact;
  the identity line simply ends after the name, exactly as the node cards do,
  and the sentence underneath says what kind of thing this is.
- **Expected: a counter at the head of the evidence line.** It is
  read off the **workload object the header names** — the Deployment — not
  counted out of a pod group, and it goes in the slot the cordon card's count
  already occupies, `·`-joined in front of the quote
  ([the cordon card's count, above](#every-count-this-card-can-have)). Without
  it W1 is a severity dot and a quota message naming a single pod, and the
  reader cannot tell *all three refused* from *one of three refused*. A dot is
  a worse instrument than a number for the thing you page on — which is exactly
  what the unresolvable case below costs, and why it is a case and not a
  choice.

| Card | Which counter | From | On the capture |
|---|---|---|---|
| W1, the workload above it resolved | the readiness pair | `status.readyReplicas` of `spec.replicas` | `0 of 1 pod ready` |
| W1, that workload not resolvable | **none** — the quote stands alone | there is nothing to read one off | does not arise; a `Rollout` CR owning the ReplicaSet is the shape that reaches it |
| W2, `readyReplicas` short of `spec.replicas` | the readiness pair | as above | `0 of 1 pod ready` |
| W2, ready whole, `updatedReplicas` short | the rollout's own progress | `status.updatedReplicas` of `spec.replicas` | `1 of 2 pods on the new version` |
| W2, both whole, something unavailable | the pods that exist and are not answering | `status.unavailableReplicas`, and no denominator | does not arise; a one-replica rollout that gave up is the shape that reaches it |

- **W2 picks its form in the order the rule tested the shortfall, so the counter
  can never contradict the severity dot beside it**
  ([NOTES § D82](../NOTES.md#d82--the-w-series-and-the-card-that-would-have-taught-people-to-mute-the-tool-2026-08-14)).
  Readiness is tested first and takes every `●` with it — nothing serving is
  nothing ready, so `0 of 1 pod ready` under a red dot agrees with it. The other
  two are reached only after `readyReplicas` has been found whole, so on any
  workload that wants a pod at all they sit beside a `▲`: `1 of 2 pods on the
  new version` says the old version is still up — the state `kubectl rollout
  status` leads with, and the one a reader needs before deciding whether to roll
  back — and `1 pod not answering` says the replacement pod exists and is not
  taking requests.
- **The third form is a bare count, and the missing denominator is the point.**
  `status.unavailableReplicas` is not counted out of `spec.replicas`; it counts
  pods that exist over and above the ones that work, and a surge can put it
  *above* the number wanted — so a denominator here would eventually print
  `2 of 1 pod not answering`, which is the same shape as the `2 of 1 pod ready`
  D82 records as a number on a screen whose whole promise is that its numbers
  can be believed. It can never read `0 pods` either, because a workload with
  nothing unavailable never got as far as a card. **This is the form that
  catches the one-replica rollout, which is the commonest Deployment size there
  is**: `maxUnavailable` rounds down to 0, so the old pod is never removed and
  the object reads `spec.replicas` 1, `readyReplicas` 1, `updatedReplicas` 1 —
  both counters above whole, and `status.unavailableReplicas` at 1 the only one
  that can see the new pod never came up.
- **Where there is a denominator, the pair and not the numerator alone**, and
  `pod` takes its singular at one in all three forms —
  the same spelling rule the age ladder uses for `hour` and `day`
  ([widgets.md § 1b](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)).
- **W1's counter comes from the Deployment in the header, not from the refused
  ReplicaSet the finding is about.** Same object as the title line, so the
  number and the name cannot disagree — and the `of m` cannot go missing on its
  own the way it can on a pod card, because a user without the workload watch
  gets no W1 or W2 at all
  ([the rules this screen obeys](#the-rules-this-screen-obeys)).
- **When that Deployment cannot be found, W1 prints the quote alone and the dot
  is amber.** An owner an `ownerReference` names and the snapshot does not hold
  is *unknown*, not *down*: an Argo Rollouts `Rollout` CR owns ReplicaSets
  directly with no Deployment between, and k8rs will not decode a CR
  ([invariant 12](../CLAUDE.md)); a 403 on `deployments` with `replicasets`
  still readable ends the same way. Falling back to the refused ReplicaSet's own
  `readyReplicas` would print `0 of 1 pod ready` under a red dot about a canary
  while the stable version serves every request — a number that is real and a
  card that is wrong. So the count goes rather than being borrowed. That is
  *"no number we cannot produce"*
  ([the rules this screen obeys](#the-rules-this-screen-obeys)) deciding a count
  instead of an age, and it is the third thing on this screen that rule has
  settled
  ([NOTES § D82](../NOTES.md#d82--the-w-series-and-the-card-that-would-have-taught-people-to-mute-the-tool-2026-08-14)).
- **That case gets a table row and no drawing of its own, and its geometry was
  measured with W1's below.** Dropping the counter changes the dot and the first
  half of one evidence line; the second and third evidence lines are
  byte-identical with the counter and without it, so a second drawing would
  repeat six lines unchanged to show one absent phrase. Same reason W2's second
  form is a row here rather than a card.
- **An absent replica count is a zero, not a missing number**, so — unlike the
  absent workload two bullets up, which is a missing *object* — it does not
  trip *"no number we cannot produce"*
  ([the rules this screen obeys](#the-rules-this-screen-obeys)): the API omits
  `readyReplicas` and `updatedReplicas` when they are zero — `broken-quota`'s
  status has neither — and `spec.replicas` absent is one replica, which is the
  API's own default and not a guess of ours.
- The second W2 form's arithmetic is `default/broken-rollout`'s from the same
  capture — `spec.replicas` 2, `updatedReplicas` 1, `readyReplicas` 2. That
  Deployment is `Progressing=True` there and files nothing, so the numbers are
  real and the finding is not; no card below is drawn from them.

**W1 is a third verbatim quote, so it is drawn on the geometry above and not
beside it.** Both cards below use the real message from the committed capture
(`k8rs-quota/broken-quota`, a Deployment held by a `pods=0` ResourceQuota):

```
● k8rs-quota/broken-quota                 3 hours ago
  Kubernetes refused to create the pods for this
  workload, so they were never made
  0 of 1 pod ready · the controller's own words: pods
  "broken-quota-59654c756-2fvgv" is forbidden:
  exceeded quota: deny-all-pods, requested: pods=1,…
  → the namespace's quota is what refused it — raise
    the quota, or ask for less
```

```
● k8rs-quota/broken-quota                 3 hours ago
  This rollout gave up — Kubernetes stopped waiting
  for the new pods to come up
  0 of 1 pod ready · the controller's own words:
  ReplicaSet "broken-quota-59654c756" has timed out
  progressing.
  → find out why the new pods will not start, then
    fix it and redeploy, or roll back
```

**The second card is not on the capture's screen** —
[W1 suppresses it](#w1-suppresses-w2-and-the-screen-shows-one-card). Its
strings are still the capture's bytes; what is drawn is the card W2 renders
when nothing else has already explained the same silence.

- **The sentences in these two drawings are the geometry's, not the rules'.**
  W1 and W2 were being written while this section was drawn, so `rules.rs` owns
  the final wording — the counter's too — and it may differ. What is settled
  here is the shape and the source: no `n of m` on the identity line, the
  readiness counter at the head of the evidence line and read off the header's
  Deployment, the quote after it under the three-line cap, the action drawn
  whole. The **quoted** strings are not placeholders — they are the capture's
  bytes, and W1's is 129 characters, which is short enough that the cap costs
  it only its closing clause.
- **The counter costs W1 nothing, and that was measured rather than assumed.**
  `0 of 1 pod ready · ` is 19 columns, `the controller's own words: pods` is 32,
  and 19 + 32 is exactly the 51 the body line has — so the first evidence line
  fills to its last column and lines two and three are byte-identical to the
  drawing without the counter. The cut lands on the same character either way:
  `exceeded quota: deny-all-pods, requested: pods=1,…`. It is not a coincidence
  that survives only at one width — `"broken-quota-59654c756-2fvgv"` is a
  30-column token that can share a line with almost nothing, so it absorbs the
  shift, and a wider counter (`0 of 12 pods ready`) reflows the first two lines
  and still cuts at the same word. Had it cost a line, the counter would still
  go first and the quote would lose the clause: the counter is the part of the
  evidence a reader cannot get anywhere else on this screen, and the quote is
  the part that is one `⏎` away in full ([detail.md](detail.md)).
- **W2 grows from two evidence lines to three and is not cut** — 109 characters
  wrap to exactly three lines, the cap, with `progressing.` alone on the third.
  There is no headroom left on it, so a longer controller message on this rule
  is the first place the `…` will appear.
- **The identity is the Deployment, on both cards.** W1's object is the
  ReplicaSet `broken-quota-59654c756`, and a card titled with a hashed
  ReplicaSet name is a card nobody can read — the same reason D3 gives for
  grouping by owner in the first place
  ([the rules this screen obeys](#the-rules-this-screen-obeys)).
- **The age is the condition's own `lastTransitionTime`** — `ReplicaFailure`'s
  for W1, `Progressing`'s for W2 — never the object's creation time, and never
  the other condition's off the same flat list. That is the same wrong-field
  trap N3 has, one list away
  ([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).

### Pods shutting down: a comma inside W2's readiness fact, never on W1

`status.terminatingReplicas`, decoded as
[`WorkloadSnapshot::terminating`](../src/rules.rs), is read by
[`shutting_down`](../src/rules.rs) alone, and `shutting_down` is called from
[`rollout_gave_up`](../src/rules.rs) — W2 — and nowhere else
([NOTES § D135](../NOTES.md#d135--family-b-the-trip-that-already-ran-the-resize-boxs-stale-premise-and-the-shape-a-capture-cannot-catch-2026-08-21),
[§ D136](../NOTES.md#d136--three-claims-that-were-reasoned-instead-of-measured-and-the-one-sentence-that-catches-all-three-2026-08-21)).
**W1 — [`pods_were_never_created`](../src/rules.rs) — never carries it, in any
shape**, and that is proven rather than assumed: the test plants drainers on
*both* the Deployment W1's count is read from and the ReplicaSet the card is
filed on, and asserts an **equality** against the same card unplanted — not
merely an absence on the corpus, where `terminatingReplicas` happens to be
`0` everywhere. Both pinned cards above are unaffected on the committed
capture either way, W1 for a stronger reason than W2: W2's clause is absent
because nothing is draining there; W1's is absent because W1 never draws it.

- **What it says, and where it sits.** `shutting_down` returns a bare phrase —
  `N pod(s) shutting down`, through `counted()`, so the unit word is present
  (`1 pod shutting down`, `3 pods shutting down`) — and returns nothing where
  `terminating` is `0` or absent. The caller writes the comma: W2's evidence
  reads `0 of 1 pod ready, 1 pod shutting down · the reason Kubernetes gave:
  …`. It is a **suffix inside the readiness fact**, not a fourth `·`-joined
  fact — the distinction this section drew twice before today and drew
  wrong both times.
- **Only W2's first arm.** `rollout_gave_up` has three arms — `ready < desired`,
  then `updated < desired`, then `unavailable > 0` — and the clause rides the
  first one only. Arms two and three (`K of M pods on the new version`,
  `N not answering`) carry no clause; see the two gaps below.
- **Why W1 cannot have it, in any wording — this is the 176-character
  measurement kept from round one, now load-bearing twice over.** Run
  through this repo's own `wrapped_at` at 51 columns against
  `screens/alerts.md`'s three-line evidence cut: W1's evidence with no clause
  at all is **already** four lines and already cut, losing only the tail
  (`used: pods=0, limited: pods=0`) while `exceeded quota: deny-all-pods` —
  the sentence [`pods_were_never_created`](../src/rules.rs)'s own doc calls the whole
  diagnosis — sits on line 3 and survives. Adding any wording moves that
  sentence off the card instead, and the flip is a cliff, not a slope: at
  `+11` characters `exceeded quota: deny-all-pods` is still kept; at `+12` it
  is not, because the 30-column quoted token
  `"broken-quota-59654c756-wzr9s"` re-lands one line later and strands line 2
  at 21 of 51 columns. The shortest wording anyone has proposed on this
  section is `+17` (bare `, 1 shutting down`); the shortest that keeps the
  unit word invariant 14 asks for is `+21` (`, 1 pod shutting down`). **There
  is no wording that fits under `+12`, so the answer this section pins is
  which card, not which words.**
- **The budget agrees with the merits, and both are pinned, not just one.**
  On W1 the quote already explains why `ready` is low — the API server
  refused to create the pods — so a drainer is not the missing explanation
  there. On W2 the quote (`ProgressDeadlineExceeded`) explains nothing about
  the count, so the drainer is exactly the explanation the reader is
  missing. The budget and the merits point at the same card.
- **Two gaps this section pins rather than papers over, one kept from
  before and one corrected today:**
  - **Arm 2 (kept):** `updatedReplicas` is a non-terminating counter and
    carries no clause, so `K of M pods on the new version` understates while
    a pod on the **new** template is draining.
  - **Arm 3 (corrected — the round-two sentence here was wrong twice over).**
    `unavailableReplicas` is `Σ(replicaset.spec.replicas) − availableReplicas`,
    and the minuend is **not a pod count**: it is a *desired* number the
    Deployment controller writes into each ReplicaSet's spec, and it knows
    nothing about a pod terminating. Only the subtrahend is non-terminating.
    The two move together only during a rollout, where the controller has
    already decremented the old ReplicaSet's spec to match — on every other
    drain (`kubectl delete pod`, an eviction, a preemption, a **node drain**)
    the difference **rises one per leaving pod**. So arm 3 can print
    `1 pod not answering` about a pod that *is* answering, mid-drain on an
    old ReplicaSet while the new one reads fully ready. That is a real gap
    and it predates this box — it is [backlog.md](../backlog.md)'s, and the
    clause is deliberately not appended there: the same pod would then be
    counted under two labels on one line. This screen pins what the reader
    sees; it does not propose the fix.
- **The shape, off the synthetic test that plants the field** — nothing on
  the committed capture reaches either value, so both are printed flat, as
  the binary prints them:

  ```
  0 of 1 pod ready, 1 pod shutting down · the reason Kubernetes gave:
  ReplicaSet "broken-quota-59654c756" has timed out progressing.
  ```

  ```
  0 of 1 pod ready, 3 pods shutting down · the reason Kubernetes gave:
  ReplicaSet "broken-quota-59654c756" has timed out progressing.
  ```

  W1's card, planted the same way, is not drawn a third time here: it is
  the byte-identical card already pinned above, proven so by the equality
  test above rather than assumed.
- **The tone finding from round one stands, and the wording it forced
  stands with it.** *"On the way out"* was the only idiom this screen had
  anywhere on a card, and *"already leaving"* is not needed once the clause
  sits inside the fact it explains — position says *this is why the number
  is low* without the reader placing a separate fact in context. What moved
  since round two is narrower: the unit word is back (`1 pod shutting down`,
  not the bare `1 shutting down`), because a bare number beside `of 1 pod`
  reads as an arithmetic error before it reads as two populations, and W2 —
  the only card that carries this clause — has the four spare characters
  that fix it.

### W1 suppresses W2, and the screen shows one card

**On the committed capture these two fire on the same owner sixty seconds
apart, and the collision is decided: W1 wins and W2 stands down**
([NOTES § D82](../NOTES.md#d82--the-w-series-and-the-card-that-would-have-taught-people-to-mute-the-tool-2026-08-14)).
`broken-quota` carries `ReplicaFailure=True` at 20:45:53 and
`Progressing=False` at 20:46:54 — the quota refused the pods, and a minute
later the rollout gave up *because* the quota refused the pods. Two Findings
for one cause is what
[NOTES § D28](../NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
calls the thing that stops the list being believable, so **this is the same
fold N6 got into rule 10**
([N6, above](#n6--folded-into-rule-10s-card-not-a-card-of-its-own)):
the second finding is not a second card, it is a consequence of the first.

- **What the reader sees.** `k8rs-quota/broken-quota` draws **one** card, W1's
  — the quota refusal, the counter, the controller's words, the fix. The
  timeout it caused draws nothing at all: no card, no second line, no marker on
  the first card. The rollout gave up for the reason the one card already
  states, and a reader who fixes the quota has fixed both.
- **The suppression is wider than W1.** `analyze` builds the set of workloads
  that already have a finding filed against them, and W2 stands down for any
  workload in it — **any finding that explains why the pods are not ready**,
  not only W1's. That includes a pod-level card two steps down the chain
  (pod → ReplicaSet → Deployment): a rollout that timed out because its new
  pod cannot pull its image is rule 3's card, and W2 adds nothing a reader
  wants to that.
- **A pod that is *serving* does not silence it, and the test is the pod's own
  `Ready` condition.** Not which rule fired, not which sentence it carries, and
  not whether the owner has a finding at all: a finding filed on a pod that
  reads `Ready=True` explains nothing about a rollout that will not finish, so
  W2 still files beside it. Rule 5's serving card is the shape that reaches this
  most often — *"Container has been restarted 10 times — it is serving now…"*,
  a workload that is up — and **it carries one of six clauses, not one**
  ([`restarting_repeatedly`](../src/rules.rs) reads the ending and appends the
  clause that matches: the run finished cleanly · was stopped · the record names
  no ending · the record names the pod's rule · the exit code is not its own ·
  something keeps killing it, plus a seventh shape with no clause where there is
  no record to read). *"but something keeps killing it"* was quoted here as
  though it were the whole rule; it is the last of the six
  ([NOTES § D95](../NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)).
  **The clause is not what decides anything here** — the same six ride rule 5's
  *down* card, where the pod is not `Ready` and W2 does stand down.
- **This is a `rules.rs` behaviour, and the screen is the place it is
  visible.** The suppression is why the drawn W2 card above is a shape rather
  than a screenshot: nothing on the committed capture renders it.

## Empty state

See [states.md](states.md) — an empty Alerts screen says *"nothing is broken
right now"*, and it has to be true, or the whole product is noise. When a check
could not run, that claim is qualified on the same screen rather than made
smaller —
[states.md § nothing broken, and something not checked](states.md#nothing-broken-and-something-not-checked).
An empty list with a switched-off rule behind it is the one screen this tool
cannot afford to draw silently.
