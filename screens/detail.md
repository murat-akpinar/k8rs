# Screen — Object detail (the tabs)

`⏎` on anything opens it. Four tabs, `[` and `]` to move between them — the
whole debugging loop without a typed command.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  ‹ logs ›   describe   yaml   events          │
│   workloads        │  ──────                                       │
│   network          │  container: app ▾          previous log: on   │
│   storage          │                                               │
│   config           │  14:21:58  starting worker pool               │
│   cluster          │  14:22:01  connected to postgres              │
│  ANALYSIS          │  14:22:06  allocating 240MB cache             │
│   capacity      1 ▲│  14:22:07  --- killed here ---                │
│   certificates  30d│                                               │
│   drain safety     │  This is the log from before the last crash,  │
│   posture          │  which is usually the one you want.           │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app --previous             │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  esc back  ? all keys  q quit      │
└────────────────────────────────────────────────────────────────────┘
```

| Tab | Shows | Notes |
|---|---|---|
| **logs** | follow, container picker, `--previous` | The most-typed kubectl command there is. `--previous` is one keypress because that is the log a crash loop needs. |
| **describe** | the object plus its events | Assembled from what we already hold; the event list is fetched for this object only, never a global Events watch. |
| **yaml** | the object as YAML | Key order is the API's, not alphabetised. Secret values are hidden behind an explicit reveal, and a revealed value never enters the command log, the audit log or this pane's copy buffer. |
| **events** | this object's events, newest first | Plain-language reason word, the controller's own message kept beside it: `Unhealthy` reads "the health check failed" next to "Readiness probe failed: …", never instead of it. |

**Every footer on every tab below loses exactly one word while a call
confirmed elsewhere is still on the wire: `q quit`. Nothing else on any of
them changes**
([dialogs.md § Detail tabs and Analysis keep their own footer, not this line](dialogs.md#detail-tabs-and-analysis-keep-their-own-footer-not-this-line)).

**The logs tab's footer carries `f follow` and `c container`, not `⇧p
previous` or `/ search`.** Both keys still work — `⇧p` and `/` are bound
exactly as [NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted)
says — they are just not two of the six things this one line has room to
name before `? all keys  q quit`, the pair that never gives way outside its
own one named exception, above
([widgets.md § The footer](widgets.md#2a-the-footer)). Follow and the
container picker are what a reader reaches for on nearly every open log pane;
`⇧p` only matters once a container has actually crashed, and a text search is
the same `/` every other pane already carries silently.

## The heading, when the name does not fit

The heading is one `Paragraph` holding `namespace/name` in full — and until
this round, nothing cut it: ratatui clipped it at the pane edge with no mark
at all, the one silent truncation [widgets.md § 7](widgets.md#7-text-that-came-from-the-api)
exists to forbid
([NOTES § D266](../NOTES.md#d266--the-phase-11-close-six-screens-that-draw-something-false-and-a-freeze-set-one-phase-before-its-consumer-2026-09-13)).
It now runs through [the identity cut](widgets.md#7-text-that-came-from-the-api)
first, the same rule every other object identity on this product now shares.
Room is the content pane, less its own two-column pad each side — **53**
columns at 80×24, the same figure `alerts.md`'s card region and
`analysis.md`'s report region both measure against.

Two pods from the same Deployment's two rollouts, differing only in their own
generated suffix, at 53 columns of room:

```
…form/checkout-worker-service-canary-7d9f4bc86d-x2k9p
…form/checkout-worker-service-stable-7d9f4bc86d-x2k9p
```

and two Deployments sharing a long OpenShift namespace:

```
…ter-node-tuning-operator/tuned-metrics-reader-canary
…ter-node-tuning-operator/tuned-metrics-reader-stable
```

Before this round both pairs drew the identical heading —
`team-alpha-payments-platform/checkout-worker-service-` and
`openshift-cluster-node-tuning-operator/tuned-metrics-` — naming neither pod
by the one thing that told them apart.

## Picking a pod, before Detail has one

D3 files one card per owner, not one per pod
([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)) — a
Deployment with three sick pods out of five is one card, `payments/web  ·  3
of 5 pods`. `⏎` on that card cannot open Detail the way it does on a bare
pod's card: Detail's four tabs are about one concrete object, and the card
names an owner, not a pod. **On a grouped finding, `⏎` first lists which pods
of the group are affected, then opens the one you pick** — the closing rule
[the logs tab used to carry alone](#every-finding-about-this-object-pinned-at-the-top-of-every-tab)
and this section is where it is actually designed. The same step sits behind
the browser's own `● web has 3 pods with problems — ⏎ to see`
([resources.md § The line under the table](resources.md#the-line-under-the-table)) —
one mechanism, reached from two screens, because both sentences name the same
fact about the same card.

This is not a small floating box like [the container
picker](#choosing-a-container-and-when-there-is-nothing-to-choose). A
container picker chooses among at most a handful of rows with short state
words; this step can face D3's own founding number — a DaemonSet's pods on a
40-node cluster, which is at least 40 findings, one `Finding::object` per
pod, folded into one card by `Finding::owner`
([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)) — and it
has to hold the full pinned block beside the list, which [the next
section](#every-finding-about-this-object-pinned-at-the-top-of-every-tab)
shows can already run past the whole 13-row body of an ordinary tab on its
own, one block alone. A box capped the way `Confirm`, `Refused` and `ContainerPick` are
([widgets.md § 5](widgets.md#5-the-modal-layer)) has no room for either. So
this step is drawn **in the exact slot Detail already owns** — the same
sidebar, the same header, the content pane handed the same way `screen.detail`
already takes it "over the view, not instead of one," so `esc` goes back to
that view exactly as it already does for an open Detail (`fn detail`'s own
doc comment). What differs from an ordinary Detail is only the head row and
the body's content: no tab row, no underline — there is no tab to be on until
an object is chosen — and a `List` where a tab's `Paragraph` would be.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web  ·  3 of 5 pods — pick a pod    │
│  RESOURCES         │  Containers exceeded their memory limit and   │
│   workloads        │  were killed by the kernel (OOMKilled)        │
│   network          │  limit 256Mi · exit 137 · 47 restarts         │
│   storage          │  → raise limits.memory, or find the leak      │
│   config           │                                               │
│  ANALYSIS          │▸ ● web-7d9f4bc86d-m3p1q   Containers…         │
│   capacity      1 ▲│  ● web-7d9f4bc86d-x2k9p   Containers…         │
│   certificates  30d│  ● web-7d9f4bc86d-t8g2r   Container image is… │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

Every row above is one line — the trailing fact **back-cuts** at a word
boundary the same way [the container picker's own state word
already does](#when-a-containers-own-state-is-what-does-not-fit) when it
does not fit; it does not wrap the way the pinned block's own prose does,
because a `List` row is one line and two facts on two lines would read as
two rows. **The two facts drawn here are real titles, cut** —
`"Containers exceeded their memory limit and were killed by the kernel
(OOMKilled)"` and `"Container image is not usable, so the container never
started (ErrImagePull)"`, the same two strings this file already quotes in
full elsewhere, cut at this pane's own room the same way any title this
long already has to be. No fact drawn on this page is ever the whole
sentence; see the bullet below for the range.

- **The head row keeps the owner's identity and adds `Card::count()`'s own
  fragment** — `payments/web` (or the bare `node-3` shape, though a
  cluster-scoped owner never reaches this step, [below](#a-group-of-one-pod-or-none-at-all--there-is-no-step))
  plus `  ·  3 of 5 pods`, exactly the string the Alerts card and the
  browser's own summary line already draw
  ([`Card::count`](../src/views.rs)) — plus a fixed `— pick a pod` tail that
  never gives way, the same protected-tail reasoning
  [resources.md § When it does not fit](resources.md#when-it-does-not-fit-the-name-gives-way--and-now-it-says-so)
  already gives `⏎ to see`: the identity is what fronts a cut, never the
  instruction. Where the denominator is not readable, this reads `payments/web
  · 3 pods — pick a pod`, `Card::count()`'s own other form, unchanged.
- **The pinned block comes first, and there is exactly one of it — the
  card's own deciding finding, not a stack of every finding the card
  holds.** It is chosen by the identical two keys
  [alerts.md § A card with more than one finding](alerts.md#a-card-with-more-than-one-finding)
  already uses to pick which finding a stacked card draws on its face:
  [`Card::severity`](../src/views.rs) (the worst present) and, where more
  than one finding shares it, [`Card::newest`](../src/views.rs)'s own
  tie-break (NOTES §
  [D246 ruling 4](../NOTES.md#d246--the-viewsrs-review-round-a-fraction-whose-halves-count-different-things-a-card-that-draws-a-count-the-screen-ends-without-and-the-freeze-that-was-set-one-phase-too-early-2026-09-06)).
  Showing anything else here would mean this step drew a different finding
  than the `⏎` that opened it just showed on the card face — a second,
  disagreeing answer to "what is wrong here", which is worse than showing a
  stack, not merely smaller than one. **The rest of the card's findings are
  not lost — they are why the rows underneath say what they do**: every pod
  row's own trailing fact is that pod's own worst finding's title, so a
  reader chasing a specific one of a DaemonSet's 38 findings picks the pod
  it is about rather than scrolling a stack to find it. **Title, evidence
  and action draw, identity does not** — the one place this block differs
  from the four-part shape [the next
  section](#every-finding-about-this-object-pinned-at-the-top-of-every-tab)
  otherwise keeps whole, and on purpose: this step's own head row already
  carries the owner and the count a block's identity line would repeat, so
  repeating it a second time two lines down would be the redundant kind of
  pinned text this file elsewhere argues against, not a second fact.
- **Every row is `▸ ` (the cursor, when selected) plus the pod's own
  severity glyph plus its name, flexible, front-cut the way [the container
  picker's own name column already is](widgets.md#7-text-that-came-from-the-api)
  when two pods share a long owner-generated prefix** — the exact collision
  [the heading section above](#the-heading-when-the-name-does-not-fit) already
  draws for two rollouts of one Deployment. **The trailing fact is that pod's
  own worst finding's title**, back-cut at a word boundary the same way [the
  container picker's own state word already gives way](#when-a-containers-own-state-is-what-does-not-fit) —
  not the raw reason (`OOMKilled` stays a card's job to translate, invariant
  14, and this fact is that translation, already done).
  **The floor this cut gives way at is 20 columns, and no real title is
  drawn whole against it — none is.** Every distinct `Finding::title`
  `rules::analyze` produces over the committed captures runs 37 to 82
  columns, most in the high 60s to 90s
  (`reports/2026-09-18-the-which-pods-step.md` § 1) — the two 17-column
  strings this page used to justify the number by (`ran out of memory`,
  `image pull failed`) were never real, and the mockups below are fixed to
  match: a fact is two or three words and a `…` at this pane's own width,
  every time, the same honest floor [the container picker's own state
  word](#when-a-containers-own-state-is-what-does-not-fit) already draws
  cut, not a whole sentence this column was ever wide enough to hold.
- **Rows sort by severity band, then name — never recency.** Recency
  already decided which *card* the reader is looking at before this step
  ever opened; using it again here would re-sort the whole list under a
  reader mid-scan of the 12 rows the pane shows at once, because every
  restart of any one of a DaemonSet's 38 pods moves that pod's own
  timestamp. `Cursor::follow` keeps *which pod* is selected, not *which
  row* a reader's eye was on, so a list that moves under the cursor is a
  list a reader cannot work through. Name is what holds still — the fact
  every row already leads with, cut or not — so a reader who remembers a
  pod by name can predict where it sits. Where the band ties, rows are stably
  ordered by name; **this replaces the earlier promise of nothing**, which
  was more pessimistic than a stable sort already is. Recency still
  decides which finding represents a pod that carries more than one — the
  same tie-break the pinned block above is chosen by, one function
  ([`decides`](../src/ui.rs)) reused for both — this is only the order the
  *rows* fall in, one level up from that.
- **The command log strip carries over unchanged.** Moving the cursor here
  runs nothing — no `kubectl` command exists for "look at a list k8rs already
  holds" — so the strip still shows whatever last ran, blank on a fresh
  session, and gains its next line only once a row is opened or a write is
  confirmed elsewhere. The mockup above draws that blank case.
- **The footer is `↑↓ move  ⏎ open  esc back  ? all keys  q quit`**, four
  words this product already uses — `↑↓ move` and `⏎ open` are the Alerts and
  Resources footers' own words
  ([alerts.md](alerts.md), [resources.md](resources.md)), `esc back` is
  Detail's own ([the logs tab's footer](#the-logs-tab)) — assembled, not
  invented. **No `/` filter is offered.** A picker over a card's own pods is
  already narrower than the list D3 was written to shrink, and adding a key
  nothing here asked for is the invariant this file exists to avoid; a filter
  for a 40-pod group is `backlog.md`'s to raise, not this box's to build.

### A group of one pod, or none at all — there is no step

**`Card::count()` returns `None` for exactly the two shapes this step has
nothing to offer**: `affected == 0` — every node card, and any finding whose
own `object` is not a pod at all (N1–N3, W1, W2) — and the bare-pod card,
where `owner == object` and D246 ruling 2 already rules there is no group to
speak of. Both reach [`Card::count`](../src/views.rs)'s `None`, and both
already draw `⏎` opening the card's own object directly, unaffected by this
box — this section adds a step only where one is real, it does not take one
away.

**`affected == 1` is a third shape, and `Card::count()` does not mark it —
it reads `1 of 5 pods`, a real, non-`None` string.** A card can hold more
than one finding (the Alerts face's own `1 more problem — ⏎ to see`) while
still naming only one pod-kind `object` between them — one finding about the
sick pod, a second about the Deployment's own readiness with no pod named at
all — and `affected` counts distinct pod objects across every finding on the
card, not findings. Whichever way a card arrives at `affected == 1`, there is
still only one candidate, and **the same rule this product already applies
to a single-container pod decides it**: *"a key that does nothing is a bug
already shipped once here"*
([§ Choosing a container](#choosing-a-container-and-when-there-is-nothing-to-choose)).
`⏎` skips this step and opens that one pod's Detail directly, with every
finding the card holds — not only the one naming that pod — pinned there
exactly as [the next
section](#every-finding-about-this-object-pinned-at-the-top-of-every-tab)
describes. **That equivalence is this shape's own, not a wider one.** The
next section pins the findings about the object a tab is open on, plus any
that name no pod at all — never the whole card — and on a card where
`affected ≤ 1` there is no *other* pod for a finding to be about, so "every
finding the card holds" and "every finding about that one pod" name the same
set. They stop being the same set the moment `affected ≥ 2`, which is
exactly the case this step exists for. A group only exists, and this step
only appears, where `affected ≥ 2` — `Card::count()` reading `n of m pods`
or `n pods` with `n ≥ 2`.

### More pods than the pane shows

The list is a `List` like the sidebar's own — the pinned block's own lines
([there is exactly one block](#picking-a-pod-before-detail-has-one), never a
stack) as unselectable rows ahead of the selectable ones (the pods), `↑↓`
skipping the
former exactly as it already skips a sidebar group heading
([widgets.md § 2](widgets.md#2-element--widget)) — so it inherits
`ListState`'s own guarantee for free: the selected row is always kept on
screen, the same escape hatch the Alerts card list already relies on when a
stack of banners eats into its row budget
([alerts.md § The height](alerts.md#the-height)). A `Scrollbar` appears once
the list is taller than the room left for it, the same rule as any other
overflowing pane ([widgets.md § 2](widgets.md#2-element--widget)) — never
before.

There is no fixed number of pods this pane promises to show without
scrolling, for the same reason [the next
section](#every-finding-about-this-object-pinned-at-the-top-of-every-tab)
gives for the ordinary tabs: the pinned block is not capped here either, so
how many rows are left for the list depends on how long the card's own
deciding finding is. What is fixed is the reach: `↓` gets to any pod in
the group, however many there are.

**Counted, not guessed, for one real shape.** The content pane is 16 rows
([the arithmetic below](#the-arithmetic)); this step spends one on the head
row, leaving **15** for the `List`. A log-shipper DaemonSet reading `38 of 40
pods` holds at least 38 findings, one per pod — and pins its **one**
deciding finding, not 38 of them, with an empty evidence line — a title that
wraps to one line
at 53 columns, an action that does too — spends 2 rows on the block and 1 on
the blank separator: **15 − 3 = 12** rows left for pods before the bar
appears. A taller block leaves fewer; the OOM example earlier, at 4 rows
(title 2, evidence 1, action 1) plus its separator, leaves 10 — which is why
its own three pods drew with room to spare and no bar at all.

```
   payments/log-shipper  ·  38 of 40 pods — pick a pod   ║
   Nodes without enough memory refused to run this Pod   ║
   → free up memory on these nodes, or lower the request ║
                                                         ║
 ▸ ● log-shipper-abc12   Nodes without enough memory…    ║
   ● log-shipper-bcd23   Nodes without enough memory…    ║
   ● log-shipper-cde34   Nodes without enough memory…    ║
   ● log-shipper-def45   Nodes without enough memory…    ║
   ● log-shipper-efg56   Nodes without enough memory…    ║
   ● log-shipper-fgh67   Nodes without enough memory…    ║
   ● log-shipper-ghi78   Nodes without enough memory…    ║
   ● log-shipper-hij89   Nodes without enough memory…    ║
   ● log-shipper-ijk90   Nodes without enough memory…    ║
   ● log-shipper-jkl01   Nodes without enough memory…    ║
   ● log-shipper-klm12   Nodes without enough memory…    ║
   ● log-shipper-lmn23   Nodes without enough memory…    ║
```

Every row's fact is the block's own title, cut the same way any title this
long is — a scheduling failure reads identically across every pod it hits,
so twelve real, distinct pods legitimately carry one repeated fact, not a
placeholder standing in for twelve different ones. Twelve pod rows, all
drawn — the bar appears because 38 pods do not fit in 12, not because this
mockup ran out of room to draw them. **The bar runs the
full height of the `List`, block rows included, not only past the
selectable ones** — the block and its blank separator are still rows of the
same `List` [the section above](#more-pods-than-the-pane-shows) already
calls them, and a `Scrollbar` tracks the widget's own content, not a filter
over which of its rows a reader can land on; a bar that started three rows
down would be answering a different, narrower question no scrollbar on this
product has ever been asked. `↓` from the twelfth pod scrolls the thirteenth
into view the same way `ListState` already promises everywhere else on this
product, and the thumb reflects **15 of the list's 41 rows** visible — 3
unselectable (title, action, separator) plus 38 pods — not 12 of 38, because
the fraction is rows of the list on screen over rows of the list, and the
block rows are never left out of either side of it. (Shown here without the
frame around it — the fixed chrome is the same as the ordinary case above,
and it is the list's own row count that changes.)

### A pod that disappears while this list is open

**Not the same fact as [the container picker's own pod-disappearing
case](#the-pod-disappears-while-the-picker-is-open).** There, the *whole*
object the picker was about was gone, so the picker had nothing left to be
about and closed itself. Here the object this step is about is the **card**,
not any one pod in it — the permanent Pod watch behind `rules::PodSnapshot`
is what feeds this list too, and it can drop one row out of many while the
rest of the group is still real. **One pod vanishing removes one row**,
`ListState` moving the selection to a neighbour the same way any live list on
this product already handles a row leaving from under the cursor; the count
in the head row (`Card::count()`, recomputed from the same snapshot) drops
with it, and nothing else about the step changes. **This step does not
auto-close itself down to a single remaining row** — closing it out from
under a reader who has not pressed anything would be a second surprise this
box does not need to invent, and the "nothing to pick" rule above only ever
governs what `⏎` does when the step is *opened*, not what a list already open
does when its count changes under it. Only the group reaching **zero** — the
last pod gone, which the same rule that filed the card in the first place has
by then almost certainly also un-filed it — closes the step and hands back to
the view beneath, the same `esc`-shaped return every other exit from this
step already uses; there is no second `Gone` to draw, for the same reason the
container picker's own case gives: picking a pod is not a pending mutation,
so there is nothing to reassure the reader about.

## Every finding about this object pinned at the top of every tab

**All four tabs draw it, not only logs.** The rule below used to sit at the
end of [§ The logs tab](#the-logs-tab), headed only "Rules for this screen,"
and never said which screen that meant — an omission `ui.rs` could not
guess, and the fix is this section, not a caption. The reason is in the rule
itself: *"you never lose the reason you opened the object"* is a promise
about the **object**, not about whichever tab happens to be open when you
arrive at it. A reader who opens `describe` first, or switches to `yaml`
mid-read, has not stopped needing to know why they are looking at
`payments/web-7d9f4` at all.

**And it is a promise about that one object, not about every finding the
card behind it holds.** A tab pins the findings whose own `Finding::object`
*is* the pod it is open on, plus any finding on the card that names no pod
at all — an owner-level, W1/W2-shaped one, which has no more specific object
to prefer. A DaemonSet card built from a rule that fires once per pod holds
one `Finding::object` per pod and one shared `Finding::owner`
([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)); opening
pod #7's own Detail pins the finding **about pod #7**, never the other 37.
[§ A pod's own findings, not the whole card's](#a-pods-own-findings-not-the-whole-cards),
below, is where this is worked through.

**What "pinned" means here, settled once.** The closing rules this section
replaces said, in the same breath, that a card's finding or findings "stay
visible at the top" and that each one "wraps to the pane and **scrolls with
it** rather than being pinned." Both cannot be literally true of a fixed
chrome row the way `fn detail`'s own name/tab-row/underline trio is — a
truly fixed block big enough to hold a controller's whole message would
shrink the space left for the tab it sits on top of, and [the arithmetic
below](#the-arithmetic) shows one block alone can already exceed the entire
body. **The second reading wins, and it is the only one that can be built:**
the block or blocks are the **first lines of the tab's own scrollable body**,
drawn before the log lines, the `describe` text, the YAML or the events —
visible at the top the instant the tab is opened, at scroll offset zero,
which is what "stays visible" actually means and is all the rule needs — and
they scroll away exactly like everything below them once the reader scrolls
past them, the same single `Paragraph`/offset every tab already draws
([widgets.md § 4](widgets.md#4-scrolling)). Nothing about `fn detail`'s own
three truly-pinned rows — the object's name, the tab row, its underline —
changes; the block or blocks are not a fourth one, they are the top of the
part that already scrolls.

**Each block is the card's own four parts, not a smaller copy of them.**
Identity, title and action wrap at the same 53-column width the card's own
region does — this pane's own heading section already measures that figure
off this exact width — so the same caps an author already writes to there
hold here without being re-derived: title stays inside three lines, action
inside five, because both are k8rs's own words and both were already bounded
at this width before this screen existed. **Only evidence differs, and this
is the one place it is not cut.** [Alerts.md § The
height](alerts.md#the-height) caps it at three wrapped lines with `…`
because a controller's message can run past any card; this is where the rest
of it is — drawn in full, however many lines that takes, which is why there
is no fixed cap on a block's own height to state here that would not be a
guess. **The four parts, not three, is a tab's own rule** — the one place
this page pins a block with the identity line left out is [the which-pods
step](#picking-a-pod-before-detail-has-one), and only because that step's own
head row already says what the identity line would; an ordinary tab's own
heading names one pod, never the owner or the count, so nothing there already
carries what the identity line says and it stays.

### The arithmetic

**16 rows in the content pane** ([alerts.md § The
height](alerts.md#the-height)'s own count for the 80×24 floor: 1 header + 1
top border + 16 body + 1 divider + 2 command log + 1 divider + 1 footer + 1
bottom border), **less 3 for `fn detail`'s own pinned trio** — the object's
name, the tab row, its underline, each a `Constraint::Length(1)` before the
open tab's own `Constraint::Min(0)` — **leaves 13 rows for the open tab's own
body** at 80×24. That 13 is what a block or blocks are drawn against.

A single block's own worst case is unbounded — `1 (identity) + ≤3 (title) +
E (evidence) + ≤5 (action)`, and `E` has no ceiling this file can honestly
name. What is bounded is what a real block costs, measured off text this
file has already drawn or already cited a real measurement for. **A stack
of two or three here is [§ A pod's own findings, not the whole
card's](#a-pods-own-findings-not-the-whole-cards) own case, not this page's
disproven "every finding the card holds" one**: a single pod carrying two
or three findings of its own — the ordinary way a stack happens at all,
since a tab never pins a finding about a different pod.

| Stack | Rows | Against the 13-row body |
|---|---|---|
| 1 block, the OOM finding [the which-pods step](#picking-a-pod-before-detail-has-one) draws — here **with** its identity line, which that step's own mockup leaves out for the one reason given above (identity 1 + title 2 + evidence 1 + action 1) | 5 | 8 rows of real tab content still visible at scroll offset 0 |
| 2 blocks of that size + 1 blank separator | 11 | 2 rows visible |
| 3 blocks of that size + 2 separators | 17 | none — the reader scrolls before seeing one line the tab itself drew |
| 1 block, a title and action at typical length but evidence 9 lines wrapped (`identity 1 + title 2 + evidence 9 + action 1`) | 13 | none — the block alone exactly fills the body |
| 1 block, every part at its own cap (`1 + 3 + 9 + 5`) | 18 | negative — this block alone is taller than the whole body |

Nine wrapped lines of evidence is not invented for this table: it is two
lines past the seven this file already measures for a real `runc` error
([alerts.md § The height](alerts.md#the-height), *"a container whose
`command` names a path that is not in the image carries containerd's whole
`runc` error, which is 7 wrapped lines"*) — a controller's own message
reaching nine is well inside the range this codebase has already put a
number on, not a worst case dreamed up to make a point. **The honest
conclusion is that this page promises no fixed number of rows for a stacked
block against the body's own budget, on purpose** — the same conclusion
[alerts.md](alerts.md#every-count-this-card-can-have) already reaches for a
row it has not designed, stated here instead of guessed at: a reader who
opens the logs tab of a pod with two or three findings of its own may need
to scroll before the first log line, and that is the true cost of never
cutting the evidence this
screen exists to show in full.

### When the stack is taller than a Loading or Empty tab has anything of its own

**Measured against the built tree, not reasoned about**
(`reports/2026-09-18-the-which-pods-step.md` § 4): two findings on one
pod — a real shape, not invented, `default/broken-crashloop` and
`default/broken-hostpath` both produce it on the committed captures — where
the second quotes the `runc` error this file already measures at 7 wrapped
lines reaches **14 rows** against the 13-row body. What was actually drawn
was worse than the arithmetic above admits to: `still loading`, `no logs
yet` and `none right now` were **erased**, not shortened, because they were
never drawn at all — the block's own layout claimed every row the tab had
and left nothing for them. The block was then **itself cut with no mark**,
its last words gone mid-quote, and the pane carried no scroll offset, so
the rest was not merely off-screen, it was unreachable. `still loading` and
`none right now` are then the same frame but for which tab is marked
open — the very failure
[PRIOR-ART § C2](../PRIOR-ART.md#c2--empty-and-not-loaded-yet-are-different-screens)
is tagged **covered** in this repo for having avoided.

**The fix reuses what this product already has, in the shape it already
has it.** [`floor`](../src/ui.rs) is one line — `area.height.saturating_sub(FLOOR)`
— and `banner` already spends it reserving room *below* a fixed message;
the block gets the identical cap spent the other way, reserving room
*below itself*: **at most `body.height − FLOOR` rows for the block**, the
rest given to whatever comes after it. A block that fits does not notice —
the OOM example throughout this section is 5 rows against 13, nowhere near
the cap. A block that does not fit is **not silently clipped**: it carries
the same `Scrollbar` [every other overflowing pane already
draws](widgets.md#2-element--widget), and `app.scroll` — [the same offset
every free-text pane on these four tabs already
has](widgets.md#4-scrolling) — reaches every line the block holds, block
included, exactly as it already reaches the rest of a long log stream.
Nothing is discarded to make the block fit; it is deferred behind a scroll
a reader can see is there.

**The sentence stops being centred the moment a block leads it, and takes
the `FLOOR` rows the cap guarantees it instead.** Centring text inside a
region whose top edge moves every time a block grows or shrinks is not a
position this file can compute honestly; the fixed alternative is what
erased the sentence in the first place. So `no logs yet`, `none right now`
and `reading the cluster…` draw as ordinary left-aligned lines directly
under the block — the same shape a real log line or a `describe` field
already takes there — never further than `FLOOR` rows below whatever of
the block is showing. **This is what keeps loading and empty two frames
and not one**: the cap guarantees the sentence is never more than a
short, bounded scroll away, and it is drawn — in full, its own words, never
paraphrased into the block's — every single time, which is the property
that was missing, not merely a taller pane. (Denied was never the pair
these two collapsed into, and stays a third, for the separate reason two
paragraphs down.) Where there is no block at all
— a healthy pod opened straight from the browser, nothing filed against it
— the sentence is not capped against anything and centres exactly as
[states.md](states.md) already draws it; this section changes nothing
there.

**Denied is not this shape, and this round changes nothing about it.** A
refused or failed pane's own sentence is a fixed banner, drawn *before* any
block or content, off the exact `floor(area)` helper above — `logs`'
`Pane::Denied` arm already reserves the banner's own room first and hands
only what is left to the block and the stream beneath it. A pinned finding
explains why the object was worth opening; it does not know why *this*
read failed, and the read's own reason — the verb, the resource, the next
step — draws whole and first, never behind a scroll a block could push it
past. Nothing here reopens that.

### A pod's own findings, not the whole card's

**A tab pins the findings whose own `object` is the pod it is open on, plus
any card finding that names no pod at all — never the whole card.**
`Finding::object` is what the rule looked at, and most rules name the pod
they found something wrong with; a W1/W2-shaped finding names the owner
itself instead, with no pod in it to prefer, and that kind pins on every
pod's own tab because there is no more specific object for it to be about.
A DaemonSet card built from 38 crashlooping pods holds at least 38 findings
— one `Finding::object` per pod, one shared `Finding::owner`, folded by
`views::cards` into the single card D3 exists to keep short
([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)) — and
opening pod #7's own Detail pins the finding **about pod #7**, not the other
37. Nothing about that tab claims to speak for the group; [the which-pods
step](#picking-a-pod-before-detail-has-one) it was opened from already did,
and its own rows are where the other 37 pods' reasons are — each one's own
trailing fact.

**One finding pins one block — the ordinary case, and every mockup on this
page before this section drew it that way without saying so.** Where a
card's own `N more problems — ⏎ to see`
([alerts.md § A card with more than one finding](alerts.md#a-card-with-more-than-one-finding))
fired because *this same pod* carries a second finding — the same pod, a
different reason — both pin on that pod's own tab, in that same section's
own order: worst severity first, the most recent breaking a tie between
equals, never only the one the card face led with. Where the extra findings
are about *other* pods in the group instead, they do not: a card can read `N
more problems` for any `N` and a reader who opens one pod's Detail still
sees exactly the findings that pod earned, however large `N` is. **This is
the condition [§ A group of one pod, or none at all](#a-group-of-one-pod-or-none-at-all--there-is-no-step)
already relies on, stated in full**: at `affected ≤ 1` there is no *other*
pod for a finding to be about, so every finding the card holds and every
finding about that one pod are the same set — that section's own sentence
stays true word for word. They stop being the same set at `affected ≥ 2`,
and a tab pins the narrower one.

That marker is still what promises this: *"⏎ leads somewhere real"* is
that page's own phrase for it, and a Detail tab that dropped every finding
that pod earned but the one drawn would be pointing the marker at nothing —
the promise was always about the object the marker leads to, not about the
group it was found in.

## The logs tab

### The buffer: 2 MB retained, 5,000 lines, 4,096 bytes per line

Three numbers, not two, and only one of them is load-bearing. A line count
and a per-line byte cap multiply into a worst case nobody budgeted and no
reader of a table could predict — **~19.5 MB** was this section's first
draft, silently a product of the other two rather than a number anyone had
chosen. **The retained-bytes ceiling is the fix**: it is the one figure that
is true in both the common case and the worst case, because whichever of the
three limits below is hit first is the one that evicts.

| Bound | Value | What actually binds it |
|---|---|---|
| **Retained bytes per open pane** | **2 MB** | the load-bearing ceiling — the other two vary, this one does not |
| Lines kept per open pane | **5,000**, oldest dropped first | the common case: short lines fill 5,000 slots at well under 2 MB |
| Length of one line before it is cut | **4,096 bytes** | a single line, on its own, whatever the other two are doing |

**2 MB is the number to defend, and it does not lean on borrowed headroom.**
The security gate's *"a 50MB annotation or an endless log line must not be
held whole in memory"* is a **prohibition** on one unbounded value, not a
budget a feature may spend up to — this file's first draft read it backwards.
The figure that actually is a budget is a different 50 MB entirely:
`REQUIREMENTS.md`'s **whole-process** `< 50MB RSS at ~1000 pods`, and it is
already measured over — **58 752 KiB at 1 011 pods**, peak and steady the
same value, with the ruling that the target stays written as missed rather
than moved to match
([NOTES § D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)).
So 2 MB is not "spare room" in either figure — there is none to spend, one of
them prohibits the framing outright and the other already measures over it —
**60.2 MB against the 50 MB the target names** — before a log pane exists.
It is defended on its own terms instead: a **fixed** addition, small next to
the process's other costs (a single decoded page of objects alone runs
several MB, per D171's own arithmetic), that does not grow with session
length. Eight minutes or eight days, it is the same 2 MB — the property A6
lacked, where nothing was measuring what the log stream held over time and
it reached 21.5 GB resident before the node's own OOM killer acted
([PRIOR-ART § A6](../PRIOR-ART.md#a6--unbounded-memory-in-the-field-for-8-days)).
A byte ceiling is what makes that true regardless of line length; a line
count alone is not, which is exactly the gap the product-of-two left open.

**5,000 lines is what actually evicts in the ordinary case**, and is why the
dropped-lines count in the pane usually tracks something a reader can
picture: at a generous 256 bytes per line — a timestamped, short structured
message, not a stack trace — 5,000 lines is about **1.2 MB**, comfortably
under the 2 MB ceiling and enough to scroll back through a crash's run-up,
which is rarely more than a few hundred lines. **Only when lines run long
does the byte ceiling take over from the line count** — stuff every line to
the 4,096-byte cap and the pane holds roughly 500 of them, not 5,000, because
2 MB runs out first. Either way the pane never exceeds 2 MB; only how many
lines that buys changes.

**4,096 bytes is not a fresh number** — it is the same `FREE_TEXT` figure
`k8s::ingest` already uses to bound a message field on the way into the
snapshot, chosen there from a census of real captures. Reusing it here is a
`tui-designer` call, not a re-derivation: one number is one fewer to explain,
and it is independently generous for a log line (most are well under it; a
line that reaches it is already unusual). Whether the Rust constant is
literally shared or a same-valued sibling is `dev-core`'s call, not this
file's. Whether the **2 MB retained-bytes ceiling** is a third constant or
computed as `5000 × FREE_TEXT` headroom is likewise `dev-core`'s call — this
file specifies the observable behaviour (never more than 2 MB retained,
whichever bound gets there first), not the Rust shape underneath it.

### When the buffer fills: the dropped-lines line

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  ‹ logs ›   describe   yaml   events          │
│   workloads        │  ──────                                       │
│   network          │  container: app ▾          previous log: off  │
│   storage          │                                               │
│   config           │  142 lines were dropped from the top to keep  │
│  ANALYSIS          │  this pane bounded.                           │
│   capacity      1 ▲│                                               │
│   certificates  30d│  14:23:41  connected to postgres              │
│   drain safety     │  14:23:44  allocating 240MB cache             │
│   posture          │  14:23:47  writing checkpoint                 │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app -f                     │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  esc back  ? all keys  q quit      │
└────────────────────────────────────────────────────────────────────┘
```

- **Silent below one drop, exact at and above it** — the same rule as every
  other banner on this product: nothing is shown until it is true
  ([states.md](states.md#the-second-paragraph-is-the-point-of-this-screen)).
  A pane that has dropped nothing shows no line at all; the moment the buffer
  evicts its first line, this replaces the blank row above the content and
  never goes away again for that pane — it only counts up.
- **Position: the top of the visible log content**, dim, because that is
  literally where the gap is — the lines missing are the oldest ones, which
  would have been above what is now the first line on screen
  ([widgets.md § 4](widgets.md#4-scrolling), which already promises this line
  and only this file was missing the number and the words).
- **The count is exact and grows live** — "1 line was dropped" (not "1
  lines"), then "2 lines", climbing for as long as the pane stays open and
  the stream outruns it. It is never rounded or bucketed; a beginner counting
  on this tool to tell the truth about what it lost gets the real number.
- **Follow (`f`) and the drop counter are independent.** Turning follow off
  freezes the *view*, not the stream underneath it while the pane is still
  open — dropping can still happen off-screen and the counter still climbs;
  turning follow back on does not "catch up" the dropped lines, because they
  are gone.

### A line longer than the cap, and a line longer than the pane — not the same thing

[widgets.md § 7](widgets.md#7-text-that-came-from-the-api) already rules that
ratatui wraps free text to the pane rather than k8rs clipping it — nothing
here overrides that. **Wrapping is not cutting.**
A perfectly ordinary long line — a connection string, a stack frame — simply
takes more rows in the pane. Nothing is missing and nothing is marked:

```
14:23:50  connecting to postgres://payments-db.svc.cluster
          .local:5432/payments?sslmode=verify-full&connect_
          timeout=10
```

A line that runs past the 4,096-byte cap is a different event: k8rs cut it,
and says so with the same marker the ingest prune already uses for an
over-long field, so the product has one way of saying "we shortened this,"
not two ([NOTES § D146](../NOTES.md#d146--the-ingest-guard-two-bounds-off-a-census-a-visible-marker-and-the-newline-a-real-kubelet-sent-2026-08-22)):

```
14:23:51  {"level":"error","msg":"panic: runtime err…  (shortened by k8rs)
```

(shown short for the page — the real cut lands at 4,096 bytes, or up to three
earlier, stepped back to a whole character so a multi-byte one is never
split.) **Attributed on purpose**, same reasoning as the ingest prune:
without the name on it, the cut reads as the application's own line trailing
off, and a debugging tool that quietly shortens the evidence is lying about
what it saw.

### Choosing a container, and when there is nothing to choose

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ payments/web-7d9f4 — pick a container ──────────────────────┐  │
│   │                                                             │  │
│   │ ▸ app                                  running              │  │
│   │   sidecar-envoy                        running   3 restarts │  │
│   │   init-migrate                         done                 │  │
│   │                                                             │  │
│   │  sidecar-envoy restarted 3 times. ⇧p shows the log from     │  │
│   │  before that restart, if the kubelet still has it.          │  │
│   │                                                             │  │
│   │               [ ⏎ pick ]       [ esc cancel ]               │  │
│   │                                                             │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app                        │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move   ⏎ pick   esc cancel                                      │
└────────────────────────────────────────────────────────────────────┘
```

**Drawn to the renderer's own layout, not the shape this section used to
draw** — measured off the built box
(`reports/2026-09-18-filter-and-container-picker.md` §§ 1–3): the name is
flexible and takes whatever the row has left; the state word and the
restart count are fixed-width columns, each sized to the *widest* value
either one holds anywhere in the list, pinned toward the right edge. This is
[the cluster picker](context.md#the-picker)'s own tag-column shape, read
literally rather than by analogy — "the row is three slots, left to right,
and only the first one is flexible" is true of this box too, `▸` and the
name where the picker has a bare name, the state word and the restart count
together where it has one fixed-width tag. **Restart count is shown next to
a container that has one**, because that is exactly the signal that makes
`⇧p` worth pressing, and the line under the list spells out which key does
it and on which container.

**A single-container pod has nothing to pick, so the picker is not offered
at all** — invariant: a key that does nothing is a bug already shipped once
here.

| Element | Multi-container pod | Single-container pod |
|---|---|---|
| Header line | `container: app ▾   previous log: off` | `container: app   previous log: off` — no `▾`, nothing opens |
| Footer | `… c container …` | `c container` is gone from the footer |
| `c` | opens the picker above | not bound; there is only ever one answer |

**Restart hints never claim a crash, and never promise the log is still
there.** A restart is not always a crash — `exitCode: 0` under
`restartPolicy: Always` restarts a container that asked to stop on purpose —
and `kubectl logs --previous` can 404 once the kubelet has rotated the old
log out from under it. *"…shows the log from before that restart, if the
kubelet still has it"* is the wording every mockup on this page now uses
for that key, in place of the old *"the log from just before its last
crash"*, which claimed both things this paragraph just ruled out. This is
the one wording change on this page R4.4 asks for; the same false claim
still stands in [help.md](help.md)'s own key map (*"logs, with the log from
before a crash"*) and in this file's own logs-tab mockup, above — both are
the same defect in a different place and belong to a box that can touch
those files, not this one.

**More containers than the box shows scroll under `↑`/`↓`, but this box does
not grow with the terminal the way the cluster picker's does — it is capped
the way `Confirm`, `Refused` and `Gone` already are**
([widgets.md § 5](widgets.md#5-the-modal-layer)), because it is drawn the
same way they are: a small nested box centred over the frame, not
`ContextPick`'s own full-width shape that "grows with the terminal rather
than stopping at a fixed dialog height"
([context.md § More contexts than fit](context.md#more-contexts-than-fit)) —
that sentence describes a different box and does not transfer here just
because both are pickers. **24 is the ceiling either way**
([widgets.md § 5](widgets.md#5-the-modal-layer)), so the number to count is
how many rows this box's own fixed chrome leaves under it, not whether a
taller terminal buys more — nothing is taller than the floor this page
draws at, which is the contradiction a fifth-container "on a narrower
terminal" parenthesis used to hide.

Counted off the mockup above, line by line and never the three that are the
list itself: the header; the outer frame's top and bottom border; the blank
row inside the outer frame before the nested box and the one after it; the
nested box's own top and bottom border; the blank row inside the nested box
before the list; the blank row between the list and the restart hint; the
hint's own two lines; the blank row between the hint and the buttons; the
button row; the blank row after the buttons; the log strip's two separators
and its one command line; and the footer. **Eighteen rows that are never
the list** — leaves **six** for it before the 24-row ceiling is reached.
Three, as the mockup above draws, is comfortable.

**A seventh container is what puts a `Scrollbar` on the list's right
edge — and, measured, that column used to be the widest row's own last
one, not a reserved one of its own**
(`reports/2026-09-18-filter-and-container-picker.md` § 3): `10 restarts`
drew as `10 restart`, its final character silently the scrollbar's own
paint. **The list now reserves that column before the state word and the
restart count are measured, whenever it is going to scroll at all** — one
column narrower to work with, so nothing the row draws can ever sit where
the scrollbar is about to be, the same reasoning
[widgets.md § 2](widgets.md#2-element--widget) already gives for drawing a
`Scrollbar` only once content exceeds the viewport, extended to the column
budget the row math itself uses rather than only to whether the widget
appears at all. Six containers with restarts, one scrolled out of view:

```
   ┌ payments/web-7d9f4 — pick a container ──────────────────────┐
   │                                                            ║│
   │ ▸ app                            not started               ║│
   │   migrate                        failed        10 restarts ║│
   │   sidecar-0                      failed         4 restarts ║│
   │   sidecar-1                      failed         4 restarts ║│
   │   sidecar-2                      failed         4 restarts ║│
   │   sidecar-3                      failed         4 restarts ║│
   │                                                            ║│
   │ migrate restarted 10 times. ⇧p shows the log from before   ║│
   │ that restart, if the kubelet still has it.                 ║│
   │                                                            ║│
   │              [ ⏎ pick ]       [ esc cancel ]               ║│
   │                                                            ║│
   └─────────────────────────────────────────────────────────────┘
```

Six or seven containers on one pod is not the ordinary case — three or four
already is — but it is a real one (a sidecar per concern is how a service
mesh, a log shipper and a metrics exporter add up), and the number above is
what a review checks any width against rather than a drawing.

**A container name too long for its column is a second call site of the
cluster picker's own name-slot cut, not a new one** — front-cut 6, one `…`,
no word to walk back to, because a name is one token
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)):
`istio-proxy` and `istio-proxy-metrics` would otherwise draw identically at a
narrow enough column, the same collision two ARN-named contexts already
motivate that rule for. **This is the name's own column giving way to a
name too long for it; the next section is the opposite direction — a state
word long enough to try to take the name's column instead.**

### When a container's own state is what does not fit

`views::container_state` returns *"needs a ConfigMap or Secret that does
not exist"* for `CreateContainerConfigError` — 47 columns, measured
against a real capture
(`reports/2026-09-18-filter-and-container-picker.md` § 1). Handed to the
row math as written, the state word is what `word` is computed from, so a
single container in this state pushed `slot` — the name's own column —
negative, clamped to **one** column, and drew two containers with no
readable name at all: `▸     running` and `      needs a ConfigMap or
Secret that does not exist`, neither `trigger` nor `bystander` anywhere on
the row.

**The name never gives way — a picker exists to name things — so the state
word does instead, and it is the state that was already going to run long
before this box ever meets a screen this narrow.** The name column is
guaranteed a floor of **20 columns** regardless of how long any state word
in the list is — comfortable for the names already on this page
(`sidecar-envoy` is 13, `istio-proxy-metrics` is 19) and a name that still
does not fit it front-cuts the same as always, above. Once that floor is
spent, whatever room the restart-count column and the two gaps leave is
what the state word gets, and a state word wider than that gives way —
back-cut at a word boundary, one `…`, the same shape
[widgets.md § 7](widgets.md#7-text-that-came-from-the-api) already gives
the Alerts card's own evidence line, because both are a sentence a reader
loses nothing from losing the tail of: the object's own `describe` tab has
the whole reason, one `esc` and one `d` away, the same way a card's full
evidence is one `⏎` away.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ payments/web-7d9f4 — pick a container ──────────────────────┐  │
│   │                                                             │  │
│   │ ▸ trigger                running                 3 restarts │  │
│   │   bystander              needs a ConfigMap or…   3 restarts │  │
│   │                                                             │  │
│   │  trigger restarted 3 times. ⇧p shows the log from before    │  │
│   │  that restart, if the kubelet still has it.                 │  │
│   │                                                             │  │
│   │               [ ⏎ pick ]       [ esc cancel ]               │  │
│   │                                                             │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c trigger                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move   ⏎ pick   esc cancel                                      │
└────────────────────────────────────────────────────────────────────┘
```

Both names read in full. `bystander`'s own reason is cut, not guessed at or
dropped — `needs a ConfigMap or…` is still enough of the sentence that a
reader who has met the plain-language translation once already recognises
it, and the whole of it is one keypress away.

### The pod disappears while the picker is open

The same watch that lets [dialogs.md § The object went away](dialogs.md#the-object-went-away-while-the-dialog-was-open)
catch a deleted pod under a Confirm dialog is already running under this
picker too — it is a `Modal` like any other, and the pod it is about can stop
existing while it is up. This is **not** a second `Gone` variant: picking a
container is not a pending mutation, so the reassurance a Confirm's `Gone`
carries — *"Nothing was changed"* — has nothing to reassure about here, and a
modal that said it anyway would raise a question ("changed what?") nobody
asked. The picker instead closes itself and hands back to exactly the state
[§ No logs yet, no previous run, and the pod disappearing mid-stream](#no-logs-yet-no-previous-run-and-the-pod-disappearing-mid-stream)
below already draws for the same fact reaching the logs tab directly: the
`--- stream ended: pod deleted ---` marker in the pane, the same one-sentence
explanation, and the same pointer to the replacement through `esc` then
`⏎`. One fact, reached two ways, is one screen, not two — a picker-shaped
"already gone" box would be a second sentence for something this page
already says correctly.

### The logs tab, before the container list is known

The Resources browser's `Table` and the permanent Pod watch behind
`rules::PodSnapshot` are two different streams — a Table row names a pod the
moment discovery's own `LIST` returns it; the matching `PodSnapshot`,
containers included, lands whenever the permanent watch's own event for that
pod is processed, which is not the same instant. A row can be on screen,
selectable, and opened before the store holds a snapshot for it.

Detail treats that gap exactly like the single-container row it already
draws: `c container` is not offered and the header carries no `▾`, because
k8rs does not yet know there is more than one container to pick from any
more than it would for a pod that only ever has one. The container name
itself is blank rather than guessed — the same rule the header's own vitals
already follow ([widgets.md § 1a](widgets.md#1a-the-header-row)) — until
`PodSnapshot` answers, at which point the ordinary rules take over: one
container keeps the header exactly as it was, more than one gains `▾` and
`c` and the picker above becomes reachable.

### No logs yet, no previous run, and the pod disappearing mid-stream

**A container that has produced nothing** is a state, not a hang
([PRIOR-ART § E1](../PRIOR-ART.md#e1--a-stream-ends-for-many-reasons-and-the-viewer-says-one-thing)) —
a `Pending` pod, or a container that just started:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/queue-worker-xk2p9                  │
│  RESOURCES         │  ‹ logs ›   describe   yaml   events          │
│   workloads        │  ──────                                       │
│   network          │  container: worker ▾       previous log: off  │
│   storage          │                                               │
│   config           │               ○  no logs yet                  │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│        Nothing has been written to this       │
│   certificates  30d│        container's log yet.                   │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs queue-worker-xk2p9 -n payments -c worker -f         │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  esc back  ? all keys  q quit      │
└────────────────────────────────────────────────────────────────────┘
```

`○` is reused rather than invented — it is already the product's symbol for
*calm, not a problem, just information* ([states.md](states.md#nothing-is-broken)).

**`⇧p` on a container that has never restarted** has no previous run to
show. k8rs does not print the API's refusal and does not leave `previous
log: on` pointed at nothing — it says so in one line and falls back to the
run that does exist:

```
  container: app ▾          previous log: off
  ⇧p — app hasn't restarted, so there's no previous run
       to show. Showing the current run instead.
```

**The stream ends because the pod itself is gone** — deleted while `f` was
following it. This is the one case E1 asks for by name: say why the stream
ended, and do not make the reader wonder whether it was a dropped connection.
It is marked the same way the existing mockup already marks a kill event
in-line (`--- killed here ---`), because that convention already exists on
this exact screen and a second one would be a second thing to learn:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  ‹ logs ›   describe   yaml   events          │
│   workloads        │  ──────                                       │
│   network          │  container: app ▾          previous log: off  │
│   storage          │  14:24:58  writing checkpoint                 │
│   config           │  14:25:02  shutting down                      │
│  ANALYSIS          │  14:25:03  --- stream ended: pod deleted ---  │
│   capacity      1 ▲│                                               │
│   certificates  30d│  Not a dropped connection — the pod itself    │
│   drain safety     │  is gone, so there's nothing left to stream.  │
│   posture          │                                               │
│   restarts         │  payments/web is a Deployment, so a           │
│   waste            │  replacement pod is probably starting. esc,   │
│   versions         │  then ⏎ its row opens that one instead.       │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app -f                     │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  esc back  ? all keys  q quit      │
└────────────────────────────────────────────────────────────────────┘
```

No `⚠` here — that symbol is reserved for a connection or trust problem and
this is neither: the connection is fine, the object it was watching no
longer exists ([README.md § The five rules every screen obeys](README.md#the-five-rules-every-screen-obeys)).
**"Offer resume"** (E1) is not literal for a deleted pod — there is nothing
left to resume — so the offer is the next useful thing instead: open
whatever replaced it, through the same `esc` and `⏎` every other screen
already uses. No new key.

### Printed instead of drawn — logs on the headless surface

Phase 6 has no TUI: the temporary `main.rs` prints this tab's output to
stdout the way [`--once`](once.md) prints findings to it, with the same
[stdout/stderr split](once.md#stdout-and-stderr-are-split-on-purpose) — the
lines are the payload, the kubectl-equivalent line is the teaching device on
stderr. **What flag or argument tells the temporary driver which pod to
stream is not decided by this file** — today's parser only knows the four
flags invariant 10 names, and picking the fifth is a CLI-surface call for
whoever wires this box, not a screen. The bounds above do not carry over
identically regardless of that shape, and the reason is what each one
actually protects:

- **The per-line cut is a property of the line itself**, sanitised and
  capped before either surface sees it — `… (shortened by k8rs)` prints
  byte-for-byte the same whether it lands in a pane or a pipe. Piping this
  stream through `grep` sees exactly what the pane would have shown.
- **The headless path carries no lossy buffer between the stream and
  stdout, and that is a requirement on it, not a description of how it
  happens to be built.** A pane needs the retained-bytes ceiling because it
  has to *redraw* recent lines; a print-as-it-arrives loop does not, provided
  each line is capped, printed and forgotten with nothing buffered in
  between — no line kept past the moment it is written, none ever evicted to
  make room for a newer one. That is what a stdout dump with **no
  dropped-lines counter** is honest about only if nothing upstream of the
  print can silently lose a line instead. A bounded channel between the API
  stream and stdout is an ordinary thing to reach for and nothing here rules
  it out by construction — so if the driver is ever built with one, the same
  dropped-lines wording this file already specifies for the pane goes on
  stdout with it, on its own line, same rule as
  [once.md](once.md#stdout-and-stderr-are-split-on-purpose): it is payload,
  so it belongs on stdout, not stderr. Only a driver with no such buffer gets
  to print nothing here — and, done that way, it holds the same near-zero
  resident memory whether it runs for eight minutes or the eight days that
  grew k9s to 21.5 GB (A6), which is a stronger property than a bounded
  retained buffer, not a weaker one.

Whatever the invocation turns out to be, the printed shape is the teaching
line on stderr followed by the sanitised, capped content on stdout — nothing
else runs between them:

```
$ kubectl logs web-7d9f4 -n payments -c app -f
14:23:41  connected to postgres
14:23:44  allocating 240MB cache
14:23:51  {"level":"error","msg":"panic: runtime err…  (shortened by k8rs)
14:25:03  --- stream ended: pod deleted ---
```

**Log streams are attacker-controlled text: an open pane retains at most
2 MB, oldest lines dropped first — up to 5,000 lines in the common case,
fewer if they run long; a single line is cut at 4,096 bytes and marked.**
Control characters are stripped before any of the three bounds is applied.
[§ The buffer](#the-buffer-2-mb-retained-5000-lines-4096-bytes-per-line) above
has the arithmetic and the wording; this used to promise a bound with no
number, which is how a bound stays unbuilt. The pinned finding block every
tab draws, and the step that picks a pod before any tab opens at all, are
designed in their own sections now —
[§ Picking a pod, before Detail has one](#picking-a-pod-before-detail-has-one)
and [§ Every finding about this object pinned at the top of every tab](#every-finding-about-this-object-pinned-at-the-top-of-every-tab) —
rather than as a coda to this one tab, which is what left the question of
which tabs draw them, and which findings, unanswered in the first place.

## The describe tab

**The object, plus what happened to it — assembled from two reads, never
from the watch store.** `describe` opens with the same fresh, unpruned GET
[the yaml tab](#the-yaml-tab) uses (NOTES §
[D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)),
then adds one more read this family builds for the first time: this object's
own events, fetched by an `involvedObject` field selector that names it and
nothing else. **Never the global Events watch** — that would mean holding
every event in the cluster just to answer one object's question, and
invariant 6 already draws the line at watching Pods, Nodes and the three
workload kinds.

**This is the only place in Phase 6 that reads events, and it is the whole
reason the fetch is being built now rather than when the events *tab* needs
it in Phase 11.** `k8s.rs` freezes at the end of this phase; a fetch built
only for the tab would have to be reopened later for describe, and a second
version of "this object's events" is exactly the two-places-disagreeing
defect this repo pays most for (invariant 11's own reasoning, one layer up).
One function, two callers, one order — newest first — settled once here.

**What describe needs of events, and what it deliberately does not build.**
The events *tab*'s own drawn layout — its own scrolling and its own columns —
is [Phase 11's](#the-events-tab), out of scope for this file today. What
describe needs is smaller: the same list, oldest to newest reversed, each
line short enough to sit under a container block without turning the pane
into the tab it is not trying to be.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 3 days ago           │
│   storage          │  containers                                   │
│   config           │    app             failed                     │
│   cluster          │      container exceeded its memory limit —    │
│  ANALYSIS          │      exit 137, 4 restarts                     │
│   capacity      1 ▲│    sidecar-envoy   keeps crashing and         │
│   certificates  30d│      restarting, 12 restarts                  │
│   drain safety     │    init-migrate    done                       │
│   posture          │                                               │
│   restarts         │  events (newest first)                        │
│   waste            │  3 min ago  the container is being stopped    │
│   versions         │  (Killing) Stopping container app             │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **The containers block reuses the picker's order and its calm words**
  (declared, then init; `running` / `done` / `not started` unchanged) **but
  no longer stops at `waiting` for every container that isn't** — measured
  on a real pod, `done` was printed for a container that exited `0` *and*
  for one that exited `255`, and `waiting` covers `ImagePullBackOff`,
  `CrashLoopBackOff` and `CreateContainerConfigError` alike
  (`k8s-admin`, three containers, 2026-08-31). `done` is still correct for a
  clean exit; everything else now says what actually happened, below.
- **A terminated container that did not exit cleanly says its reason, in
  plain language where one is known, and its exit code always** — `app`
  above is the pod's own OOM story from [the logs tab](#the-logs-tab),
  `container exceeded its memory limit` being invariant 14's own worked
  example (`CLAUDE.md`). Only `OOMKilled` is translated today; every other
  reason — `Error`, `ContainerCannotRun`, or the empty string a real
  container can carry (`k8s-admin` measured `reason=Error, exit=1` and a
  bare `exit=255` with nothing in `reason` on the same pod) — falls through
  to the exit code alone, never a guessed word. **`done` is not renamed to
  `failed` before it earns that word**: a clean `exit 0` stays `done`,
  because that is not a diagnosis, it is the healthy case.
- **A waiting container says its reason in plain language, not the generic
  `waiting`** — `sidecar-envoy` above is `CrashLoopBackOff`, translated;
  `ImagePullBackOff` / `ErrImagePull` and `CreateContainerConfigError` get
  their own short phrases the same way. An ordinary, momentary
  `ContainerCreating` stays the calm `not started` rather than being dressed
  up as a problem, and a reason this table does not recognise falls through
  to the raw word, sanitised — the same safe-fallback rule the events table
  below uses, stated once and reused rather than invented twice.
- **Every event reason is a short plain-language phrase *beside* the
  controller's own message, never instead of it** ([NOTES §
  D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
  `Killing` above reads *"the container is being stopped"* on its own line,
  then `(Killing) Stopping container app` — the raw word and the verbatim
  message together — on the line under it. **This reverses what this file
  said before**: a translated sentence that *replaces* the message can be
  measurably false (`Pulled` translated as "the image finished downloading"
  is false whenever the image was already cached) or can quietly delete the
  one fact the diagnosis turns on (`Unhealthy` covers both a liveness probe,
  which kills the container, and a readiness probe, which only takes it out
  of the Service — the same reason word, two different outcomes, and only
  the message says which). The table below is short **on purpose**: a reason
  not in it prints as its own raw word, the message beside it, and nothing
  invented — the same discipline `BackOff` already had, now applied to every
  reason rather than carved out for one.

  | Raw reason | Phrase |
  |---|---|
  | `Scheduled` | kubernetes placed this pod on a node |
  | `Pulling` | the container started pulling its image |
  | `Pulled` | the image is ready |
  | `Killing` | the container is being stopped |
  | `Unhealthy` | the health check failed |
  | `BackOff`, or anything else | *(no phrase — the raw word and the message, nothing more)* |

- **The age is [the one ladder](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen),
  not a shorthand.** `3 min ago`, `1 hour ago`, `4 days ago` — the same
  strings a card's right edge or `--once`'s title suffix draws, because it is
  one function reached from here too. `1 day ago` never appears, for the same
  reason it never appears anywhere else on this product.
- **No command line of its own for the events fetch.** `kubectl describe`
  is one word the user would have typed for two real reads — the pod and its
  events — and invariant 4 already tells the command log to show the
  *equivalent*, not the two calls underneath it. The audit log, not drawn on
  this page, is where both real reads land. A reveal of any kind costs no
  command line either, the same reasoning [the yaml tab's](#a-secret-values-hidden-behind-an-explicit-reveal)
  does — nothing here is ever sent to the cluster that the object's own read
  did not already send.

### No events at all — a healthy pod is not a broken fetch

A pod up for a week has almost certainly outlived every event it ever had —
Kubernetes keeps them for a while and then drops them, so *nothing left* and
*nothing happened* are different facts wearing the same empty list. Saying
only "nothing happened" would be true the day the pod started and false a
week later, in the one case a reader has no other way to check.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web                                 │
│  RESOURCES         │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 8 days ago           │
│   storage          │                                               │
│   config           │  containers                                   │
│   cluster          │    app             running                    │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│  events                                       │
│   certificates  30d│  ○  none right now                            │
│   drain safety     │                                               │
│   posture          │  Kubernetes only keeps events for a while, and│
│   restarts         │  this pod has run long enough that none are   │
│   waste            │  left.                                        │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web -n payments                             │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

`○` is the product's own symbol for *calm, not a problem*
([states.md](states.md#nothing-is-broken)), reused rather than invented —
same rule the no-logs-yet state already follows
([§ No logs yet](#no-logs-yet-no-previous-run-and-the-pod-disappearing-mid-stream)).
The second line is the point of the state, same as everywhere else on this
product ([states.md § the second paragraph is the point of this screen](states.md#the-second-paragraph-is-the-point-of-this-screen)):
it says *why* the list is empty, not just that it is.

### A repeated event — one line for something that happened 2,383 times

The kubelet does not create a new Event object per occurrence; it bumps
`count` on the one it already has. A card that shows only the last
occurrence and the reason word is silently dropping the fact that actually
matters: *"the health check failed 3 minutes ago"* and *"it has failed 2,383
times since the pod started"* are different diagnoses of the same pod, and
only one of them tells a reader whether to look now or to have looked four
days ago.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 5 days ago           │
│   storage          │  containers                                   │
│   config           │    app             running                    │
│   cluster          │                                               │
│  ANALYSIS          │  events (newest first)                        │
│   capacity      1 ▲│  3 min ago    the health check failed         │
│   certificates  30d│  (Unhealthy) Readiness probe failed:          │
│   drain safety     │  HTTP probe failed with statuscode: 503       │
│   posture          │  happened 2,383 times since 4 days ago        │
│   restarts         │                                               │
│   waste            │  4 hours ago  the image is ready              │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **The fourth line only appears when `count` is more than one.** `4 hours
  ago the image is ready` above carries no such line — a `count` of `1`
  draws exactly what every other event row draws, nothing extra, because a
  thing that happened once needs no sentence saying it happened once.
- **`happened N times since <span> ago`, not `x2383 over 4d8h`.** The number
  is exact — commas at the thousand, never rounded, the same discipline the
  dropped-log-lines counter already keeps
  ([§ When the buffer fills](#when-the-buffer-fills-the-dropped-lines-line)) —
  and the span uses [the one age ladder](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)
  a second time, on `firstTimestamp` rather than `lastTimestamp`. **Both
  numbers are needed and neither replaces the other**: the count without the
  span is "a lot," of unknown recency; the span without the count is "still
  going," of unknown severity. `kubectl`'s own `3m14s (x2383 over 4d8h)` is
  the same two facts, spelled for someone who already knows what `x` and
  `over` mean here.
- **This is measured, not invented** (`k8s-admin`, 2026-08-31): a real
  readiness probe on an 8-day cluster, `count` 2,383, first seen 4 days
  before the last. The translated line above is this file's own words over
  those real numbers.

### More events than the pane — it scrolls, the same as everything else

Describe does not cap the list and does not add its own "N more" line. The
pane is a `Paragraph` with a scroll offset like every other overflowing pane
on this product ([widgets.md § 4](widgets.md#4-scrolling)), and a busy pod's
events scroll exactly the way the Analysis panes that refuse to cap already
do ([analysis.md § Restarts](analysis.md#restarts)) — no new affordance, no
new key.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  4 hours ago  the container started pulling   │
│   storage          │  its image                                    │
│   config           │  (Pulling) Pulling image "payments/web:2.3.1" │
│   cluster          │                                               │
│  ANALYSIS          │  6 hours ago  kubernetes placed this pod on a │
│   capacity      1 ▲│  node                                         │
│   certificates  30d│  (Scheduled) Successfully assigned            │
│   drain safety     │  payments/web-7d9f4 to node-3                 │
│   posture          │                                               │
│   restarts         │  9 hours ago                                  │
│   waste            │  (BackOff) Back-off restarting failed         │
│   versions         │  container app                                │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

This is the same pod as the main mockup, scrolled past the identity line and
the containers block — the tab row and the underline under it stay pinned
(they are the `Tabs` widget, a separate element from the scrolling
`Paragraph` below it), only the body moves. A message too long for one line
wraps to the pane exactly as a log line does
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)) — *"the container
started pulling its image"* above is the ordinary case, not a special one.
`BackOff` is the fall-through case drawn for real: no phrase, so the age sits
alone on its own line, and `(BackOff)` and the message it came with follow
under it, nothing invented.

### The pod's own reason, when it has one

`status.reason` sits beside `status.phase` and was dropped entirely before
this review — a pod carrying `reason: Evicted` printed `Pod · failed ·
created 8 days ago` and never said why, which is a `Failed` that told a
reader nothing a `Failed` from any other cause would not also have told
them. The identity line now carries it, translated where the word is known,
with `status.message` kept beside it for the same reason an event's message
stays beside its reason — [the Waste report](analysis.md#waste) already
promises this exact page: *"look at one of the pods — its own message names
what ran out."* This is that pod.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/worker-4kd2p                        │
│  RESOURCES         │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · failed · created 8 days ago            │
│   storage          │  removed by the node to take back room        │
│   config           │  (Evicted) The node was low on resource:      │
│   cluster          │  ephemeral-storage.                           │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│  containers                                   │
│   certificates  30d│    worker          not started                │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod worker-4kd2p -n payments                    │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **The phrase and the raw word follow the exact same layout as an event**:
  a short line, then `(RawReason) message` under it, wrapped like any other
  pane text. One layout for "a word that explains a state," used for a pod's
  own reason, a container's reason and an event's reason alike, is the point
  — three separate inventions here would be three things to keep agreeing.
  Only `Evicted` is translated today; anything else `status.reason` can hold
  falls through to the raw word beside the message, the same safe fallback
  as everywhere else on this page.
- **`Pod · failed · …` still comes first, unchanged**, because `status.phase`
  and `status.reason` answer different questions — *what state is it in*
  and *why* — and a reader who only wants the first still gets it on one
  short line.

### An object that does not exist

Reachable today only headlessly, because `⏎` in the eventual browser opens a
row that is, by construction, currently in the watched store — a typed
`--object` name has no such guarantee. [§ Printed instead of drawn](#printed-instead-of-drawn--describe-on-the-headless-surface)
below has the exact sentence; it is the same one `logs_run`'s pod-fetch
already prints for a missing pod, reused rather than reworded, because
describe's first read *is* that fetch. **The drawn pane's own version of this
state is Phase 11's** — dialogs.md already has the shape a re-read that finds
the object gone should take
([§ The object went away while the dialog was open](dialogs.md#the-object-went-away-while-the-dialog-was-open)),
and describe/yaml opening on a since-deleted object is the same fact reached
by a different door, not a new one to design here.

### Free text that carried control characters

An event's `message` is exactly the kind of field invariant 9 exists for —
free text a controller wrote, not something k8rs generated. It is stripped
before it is drawn, the same way a log line or a pod name is
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)): silently,
character by character, with nothing left in its place. A message reading
`FailedMount: secret "prod` + U+202E + `terces" not found` draws as
`FailedMount: secret "prodterces" not found` — the reader sees a shorter,
correctly-ordered sentence and nothing marks that anything was removed,
exactly as `sanitize()` already does for every other free-text field on this
build (`src/main.rs`, `fn sanitize`). Reused, not reinvented: this file does
not ask for a second convention where the first one already holds.

**Unchanged on purpose, checked against [the yaml tab's own
reversal](#free-text-that-carried-control-characters--reversed-for-this-one-pane)
rather than assumed exempt from it** (NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)
narrows this by the reason it was written, not by a second reading of it).
D198's own distinction is exactly the one that decides this: a **document**
keeps `\n` because the payload *is* the text and a newline prints as itself;
a **cell** does not, because in a cell a newline breaks the layout instead
of printing as itself. Every row on this tab — the identity line, a
container's row, an event's — is a cell: one line in a list, wrapped by
width like any other pane text but never carrying a hard line break of its
own, the same way a table row never does. A `\n` inside an event's message
would not print as a second line of that event, it would open a second
row that looks like a second event — which is worse than losing the
character, not better. So this section's own example — a control character
disappearing with nothing marking the cut — is the correct behaviour here,
and stays exactly as written above.

### Printed instead of drawn — describe on the headless surface

Same split as [the logs tab](#printed-instead-of-drawn--logs-on-the-headless-surface)
and [`--once`](once.md#stdout-and-stderr-are-split-on-purpose): the teaching
line on stderr, the payload on stdout, `--describe` beside the same `--object`
[D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)
already reserved for this family of verbs. **No new pod-fetch code**: the
namespace check, the 404 sentence, the connect-timeout sentence and the
cluster-unreachable sentence are `logs_run`'s own first steps, read again
rather than rewritten, because describe's first read is the identical
`k8s::pod()` call before either verb touches what makes it different.
`--container`, `--previous` and `--follow` are simply not read by
`--describe` — not specially refused, the same way `--context` without
`--live` is not refused today (`fn mistyped`'s own stated rule).

```
$ kubectl describe pod web-7d9f4 -n payments
Pod · running · created 3 days ago

containers:
  app             failed
    container exceeded its memory limit — exit 137, 4 restarts
  sidecar-envoy   keeps crashing and restarting, 12 restarts
  init-migrate    done

events (newest first):
  3 min ago  the container is being stopped
    (Killing) Stopping container app
```

A pod carrying `status.reason` prints it the same way, right under the
`Pod · … · created …` line — `removed by the node to take back room` then
`(Evicted) The node was low on resource: ephemeral-storage.` — and an event
with `count` above `1` prints one more line under its message: `happened
2,383 times since 4 days ago`. Both are the drawn pane's own wording,
unboxed.

**No events prints no heading, on stdout or stderr** — the same reasoning
`nothing_written` already states for a container with no log yet
(`src/main.rs`): stdout is the payload, and when the payload really is
empty it stays empty rather than dressing itself up, so a reader piping
`k8rs --describe … | wc -l` still gets an honest count and still learns why
from stderr. An empty `events (newest first):` heading over nothing is not
honest payload, it is decoration, so the heading is dropped and the one-line
explanation — *"Kubernetes only keeps events for a while, and this pod has
run long enough that none are left"* — moves to stderr instead, beside the
`$ kubectl describe …` line it belongs with.

**Exit code is the only thing that can carry the difference between *this
object has no events* and *k8rs could not find out*.** Both print the
identical `Pod · … / containers: …` block on stdout with no events
section — the payload looks the same either way, because in both cases
describe has nothing honest to add under the heading it already dropped.
The distinction a reader needs — *calm* versus *broken* — has nowhere left
to live but the exit code: `0` for a read that succeeded and found nothing,
`2` for a read that did not finish. A script branching on `k8rs --describe …
&& echo ok` gets the right answer even though stdout alone could not have
told it.

| Failure | Stream | Exit |
|---|---|---|
| `--object` names no such pod | stderr: `k8rs: there is no pod named ghost in payments — check the name and the namespace` | `2` |
| Cluster unreachable / login expired | stderr: the same `because(...)` sentence `logs_run` prints | `2` |
| The pod fetch does not answer inside the timeout | stderr: `k8rs: this cluster has not answered for the pod … in … after … seconds` | `2` |
| Events fetch fails after the pod read succeeded | stderr: one sentence naming what failed, same `because(...)` shape | `2` |
| Nothing has gone wrong, this object simply has no events | stderr: the one-line explanation above; stdout carries the object and containers with no events section | `0` |
| `--kind` names anything but `pod` | stderr: `k8rs: --describe only knows how to read a pod right now — containers and events don't mean the same thing on a Secret. --kind pod is the only value it accepts` | `2` |
| Success | stdout: the block above | `0` |

**One open question this file does not close**: if a run somehow named more
than one of `--logs` / `--describe` / `--yaml`, which one wins is a tie-break
`dev-core` picks and records — the same way [D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)
left the flag's exact spelling to them. This file specifies what each verb
shows once chosen, not the precedence between three that all narrow to one
object.

## The events tab

**The same fetch describe already reads — reused, not reopened.** `events`
shows exactly what [describe's own events read](#the-describe-tab) already
fetches: the `involvedObject` field-selector GET, newest first, the one
function two callers share so there is never a second version of "this
object's events" to keep in agreement with the first — "One function, two
callers, one order — newest first — settled once here," in describe's own
words. Nothing new goes to the cluster; what is new is the pane. Describe
fits a handful of these rows under a container block. Here the whole content
pane is the list.

**What the tab draws that describe's own block does not: nothing.** Same
rows, more of them, no header. `events (newest first)` — the line describe
prints above its own block, because it needs to say what the block below it
is — is dropped here: the tab label already says that, the same way the
yaml pane carries no `yaml:` heading of its own repeating what the tab
underneath it already says. The reason→phrase table is
[describe's own six rows](#the-describe-tab), unchanged and not grown here —
a reason the table does not recognise still falls through to its raw word
beside the message, nothing invented. The `(RawReason) message` line, the
`happened N times since <span> ago` line (only when `count` is more than
one), and [the one age ladder](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)
are the identical rules, reached from the identical function. A second
grammar for the same fact would be exactly the two-places-disagreeing defect
this repo pays most for — invariant 11's own reasoning, restated once by
describe, cited rather than repeated here.

Several events, newest first, on a pane with nothing above it but the
object's name and the tab row — the ordinary case, and it happens to be the
same pod [describe's own repeated-event example](#a-repeated-event--one-line-for-something-that-happened-2383-times)
already measured, so the count-more-than-one row and a plain, once-only row
sit side by side without inventing a second fixture:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │  3 min ago    the health check failed         │
│   storage          │  (Unhealthy) Readiness probe failed:          │
│   config           │  HTTP probe failed with statuscode: 503       │
│   cluster          │  happened 2,383 times since 4 days ago        │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│  4 hours ago  the image is ready              │
│   certificates  30d│  (Pulled) Successfully pulled image           │
│   drain safety     │  "payments/web:2.3.1"                         │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web-7d9f4 -n payments                   │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **The `(Unhealthy)`/count-2,383 row and the plain `(Pulled)` row are the
  same two events [describe's own repeated-event section](#a-repeated-event--one-line-for-something-that-happened-2383-times)
  measured on a real cluster.** Reused rather than re-fixtured, for the same
  reason the events read itself is reused rather than refetched.
- **The second row's `(Pulled) Successfully pulled image
  "payments/web:2.3.1"` line is drawn here that describe's own mockup did
  not have room for.** Describe's block sits under a container list and
  cropped it for space; the rule above it — every event reason is a phrase
  *beside* the controller's own message, never instead of it — carves out no
  exception for a `count` of one, and this pane has the room to show it
  correctly. This is the one place this section adds a line describe's
  mockup omitted; everything else here is transcribed, not invented.
- **An event can reach this pane with no age at all** — `Happening::at` is
  `Option<Time>`, and a real event can carry none of the four fields that
  fill it, which draws no age rather than one this file invented, [the same
  "no number we cannot produce" rule](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)
  every age on this product already keeps. The age column still pads to the
  widest age actually on the pane, and describe's own rule for the row that
  has neither an age nor a phrase applies unchanged here, one function away:
  the first line is dropped rather than left as a row of blank padding, so
  a phrase-less, age-less `BackOff` reads as its two lines, not three:
  ```
  (BackOff) Back-off restarting failed container app
  ```
  — no leading blank line above it.

### No events at all — the same words, filling the pane instead of a block in it

[Describe's own reasoning](#no-events-at-all--a-healthy-pod-is-not-a-broken-fetch)
is unchanged: *nothing left* and *nothing happened* are different facts, and
only the second paragraph tells them apart. The words do not change when the
pane is the whole screen rather than a block under a container list — reusing
them is the same "written once" discipline as everywhere else on this page.
What changes is only the layout: with nothing else sharing the pane, this is
now a whole-screen calm state like
[the ordinary Nothing is broken screen](states.md#nothing-is-broken), so it is
centred the same way that one is, rather than left-flush under a heading that
no longer exists here.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web                                 │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │                                               │
│   storage          │                                               │
│   config           │               ○  none right now               │
│   cluster          │                                               │
│  ANALYSIS          │    Kubernetes only keeps events for a         │
│   capacity      1 ▲│    while, and this pod has run long enough    │
│   certificates  30d│    that none are left.                        │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web -n payments                         │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

`○` is still the product's own calm symbol
([states.md § Nothing is broken](states.md#nothing-is-broken)), reused rather
than invented a third time.

### More events than the pane — the whole pane scrolls now, not a block in it

[Describe's own rule](#more-events-than-the-pane--it-scrolls-the-same-as-everything-else)
holds unchanged: no cap, no "N more" line, a `Paragraph` with a scroll offset
like every other overflowing pane on this product
([widgets.md § 4](widgets.md#4-scrolling)). The object's name, the tab row
and its underline stay pinned — drawn above the scrolling `Paragraph`, not
inside it — three rows here and, once a read has been cut,
[a fourth](#more-events-than-k8rs-was-given--a-different-claim-from-more-than-the-pane-holds)
alongside them; nothing is reserved for that fourth row until there is
something to put in it. Otherwise exactly as
[describe's own scrolled mockup](#more-events-than-the-pane--it-scrolls-the-same-as-everything-else)
already shows — and only the event list itself moves. This is the identical
three events from that section, scrolled to the same point, because the pane
holding them is the same widget with the same content:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │  4 hours ago  the container started pulling   │
│   storage          │  its image                                    │
│   config           │  (Pulling) Pulling image "payments/web:2.3.1" │
│   cluster          │                                               │
│  ANALYSIS          │  6 hours ago  kubernetes placed this pod on a │
│   capacity      1 ▲│  node                                         │
│   certificates  30d│  (Scheduled) Successfully assigned            │
│   drain safety     │  payments/web-7d9f4 to node-3                 │
│   posture          │                                               │
│   restarts         │  9 hours ago                                  │
│   waste            │  (BackOff) Back-off restarting failed         │
│   versions         │  container app                                │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web-7d9f4 -n payments                   │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

`BackOff` is still the fall-through case drawn for real: no phrase, so the
age sits alone on its own line and `(BackOff)` and the message it came with
follow under it. A message too long for one line still wraps exactly as a
log line does
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).

### More events than k8rs was given — a different claim from more than the pane holds

**Not the same state as the one above.** "More than the pane" is this
product's own display choice — it scrolls, and nothing on screen needs to
say so because scrolling is how every overflowing pane on this product
already answers it. This one is the server's choice, not this product's:
the fetch is capped at `EVENTS_KEPT` — 500 today — and once the cluster has
more than that for one object, the read stops there. There is no scrolling
to what was never fetched. That costs this pane its own opening claim: a
`limit` returns the cluster's own storage order, not the newest, so *newest
first* — the thing every other mockup in this section is quietly true of —
is false the moment the fetch is cut, and the words that promised it are the
words that have to be withdrawn.

**The heading carries the withdrawal, because the heading is the only place
the claim was made.** This pane draws no heading in the ordinary case — [the
opening section above](#the-events-tab) rules that the tab label already
says what the pane is — so a cut list is the one case that heading comes
back, and it comes back saying the opposite of what it would otherwise imply
by its absence: not "these are the newest," but that no such promise can be
made. The words are exactly what describe's own headless print already
says when its own events read is cut — this file had not written them down
before now, but the product already had, and they are reused rather than
reworded a second time here. Only the number is a fact about this build,
not a fact about the object, so it is named plainly rather than rounded or
hidden:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │  events (the first 500 k8rs was given — there │
│   storage          │  are more, and these are not the newest):     │
│   config           │                                               │
│   cluster          │  9 hours ago                                  │
│  ANALYSIS          │  (BackOff) Back-off restarting failed         │
│   capacity      1 ▲│  container app                                │
│   certificates  30d│                                               │
│   drain safety     │  3 min ago    the container is being stopped  │
│   posture          │  (Killing) Stopping container app             │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web-7d9f4 -n payments                   │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **The 3-minute-old `Killing` row sits below the 9-hour-old `BackOff`
  row on purpose.** This is not a scrolled view — it is the top of the
  pane — and the order is the cluster's storage order, not time. Drawing it
  chronologically would quietly rely on the one thing the heading just
  said not to trust.
- **`500` is read off `k8s::EVENTS_KEPT`, not typed twice.** If that
  constant ever changes, this heading's number
  changes with it in the running product; it is written here because a
  mockup shows what actually renders, the same way `2,383` above is a real
  measured count and not a smaller stand-in.
- **The heading pins, and only the event list scrolls under it.** A cut
  list can also be longer than the pane — 500 events is far more than the
  ten rows left once the heading takes its own two — so [the pane-overflow
  rule](#more-events-than-the-pane--the-whole-pane-scrolls-now-not-a-block-in-it)
  still applies, but the heading is not part of what it scrolls: it draws
  pinned above the scrolling `Paragraph`, beside the object's name, the tab
  row and the underline — a fourth pinned row, and one this pane carries
  only because this is the one state that has something to pin there.
  **Nothing is reserved for it otherwise** — an ordinary, uncut list keeps
  the same three pinned rows every other mockup on this page draws, no
  blank line held open for a heading that never withdrew anything. Scrolled
  well past the list's own first two rows, the heading is still the first
  thing on the pane:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │  events (the first 500 k8rs was given — there │
│   storage          │  are more, and these are not the newest):     │
│   config           │  3 min ago    the health check failed         │
│   cluster          │  (Unhealthy) Readiness probe failed:          │
│  ANALYSIS          │  HTTP probe failed with statuscode: 503       │
│   capacity      1 ▲│  happened 2,383 times since 4 days ago        │
│   certificates  30d│                                               │
│   drain safety     │  4 hours ago  the image is ready              │
│   posture          │  (Pulled) Successfully pulled image           │
│   restarts         │  "payments/web:2.3.1"                         │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web-7d9f4 -n payments                   │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

This is the same `Unhealthy`/`Pulled` pair [the ordinary case
above](#the-events-tab) already measured — reused, not re-fixtured — now
standing in for two rows from deeper in the (much longer, capped-at-500)
list. `BackOff` and `Killing`, the two rows the mockup above draws right
under the heading, have scrolled out of view above them; the heading has
not, because it was never part of what scrolled. Nor is the blank row the
mockup above draws between the heading and `BackOff`: that row is the top of
the *scrollable* body, not the pinned area, so scrolling past it removes it
the same as any other row — which is why no gap is left here between the
heading and the event now sitting at the top of the visible list.

### The events fetch could not be completed

Two different facts share this heading, and they are not the same failure
wearing two names.

**A permission gap degrades this one tab and nothing else.** Nothing has
happened to the object — only to what k8rs may read about it — so this stays
a message inside the pane, the same shape
[the namespace-scoping banner](states.md#you-can-only-see-some-namespaces)
already uses for a 403 elsewhere, and it names the missing verb and resource
the same way every other refusal on this product does
([states.md § Rules that hold across every state on this page](states.md#rules-that-hold-across-every-state-on-this-page):
*"A 403 degrades exactly the feature that needed the permission and names the
missing verb and resource. It never crashes and never retries in a loop."*):

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│  RESOURCES         │  logs   describe   yaml   ‹ events ›          │
│   workloads        │                           ────────            │
│   network          │                                               │
│   storage          │  k8rs can't read this pod's events.           │
│   config           │                                               │
│   cluster          │  Missing permission: list events in payments. │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│  The other tabs on this object still work —   │
│   certificates  30d│  only this permission is missing.             │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl events --for pod/web-7d9f4 -n payments   → refused       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

No `⚠` — that glyph is reserved for a connection or trust problem
([states.md § An empty kind in the browser](states.md#an-empty-kind-in-the-browser)),
and a scoped-down role is neither;
[the namespace-scoping banner](states.md#you-can-only-see-some-namespaces) it
is modelled on carries none either.

**The object being gone entirely is a different fact, and takes a different
shape.** This tab cannot be opened on an object that does not exist — `⏎`
only ever opens a row currently in the watched store, the same guard
[describe's own version of this state](#an-object-that-does-not-exist)
already names — so the only way to reach it here is the object being removed
by something else while this view is already open on it. That is exactly
[dialogs.md § The object went away while the dialog was open](dialogs.md#the-object-went-away-while-the-dialog-was-open),
which describe already ruled applies unchanged to "describe/yaml opening on a
since-deleted object," for the same reason it applies here: the read behind
this tab is downstream of the identical `k8s::pod()` call. No new pane, no
new mockup — the existing "Already gone" shape covers it, minus its "Nothing
was changed" line, which is a sentence about a mutation this tab, a read,
never attempted.

### Free text that carried control characters — the same rule as describe's, not a second reading of it

Identical to describe's own rule, not a second reading of it: an event's
`message` is a **cell**, not a document — one line in a list, wrapped by
width, never carrying a hard line break of its own — and
[D198's own distinction](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)
is what decides that, the same way it decides it for
[describe's own version of this section](#free-text-that-carried-control-characters).
`\n` does not survive here any more than it does there — this pane is exactly
the shape D198 carved the exception *away* from, not the one it carved it
into. Every character `unprintable` refuses on describe it refuses here too.

### No headless surface for this tab

Every other tab on this page has a "Printed instead of drawn" section
because `--logs` / `--describe` / `--yaml` already exist in the temporary
driver
([D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)).
There is no `--events`, and this section does not add one: invariant 10
fixes the flag list at fifteen, and a sixteenth is a recorded decision this
box does not make. This tab is drawn only, reachable through `⏎` then
`[`/`]`, never through a flag — and nothing is lost by that:
[describe already prints this object's events headlessly](#printed-instead-of-drawn--describe-on-the-headless-surface),
newest-to-oldest reversed under its own `events (newest first):` heading,
which is the one place a script gets this same fact today.

### The command log, and the footer

**The command log shows `kubectl events --for pod/web-7d9f4 -n payments`,
not `kubectl describe`.** Describe's own pane earns `kubectl describe` as its
equivalent because its pane shows two reads folded into one — the object
*and* its events — and `kubectl describe` is genuinely what a user would type
to get both. This pane shows only the second half, so the line it teaches is
the command that produces only that half: `kubectl events --for TYPE/NAME` is
a real, current subcommand (stable since kubectl 1.28) built for exactly this
question — "what happened to this one object" — and it is a truer equivalent
of what is on screen than `kubectl describe` would be, which shows spec and
status this pane does not. The refused mockup above appends `→ refused`, the
same convention [the login-expired header](states.md#your-login-expired)
already uses for a command that was sent and answered no.

**Typing the line does not reproduce the order this pane promises, and that
is worth saying rather than leaving for a reader to find out at 3am.**
`kubectl events --for` sorts its own output oldest first — read off
kubectl's own `pkg/cmd/events/events.go`, `sort.Sort(SortableEvents(...))`
ascending on `eventTime` — the reverse of the newest-first order this pane
draws and the fetch behind it returns. There is no `--sort-by` on `kubectl
events` to add to the line, so the difference is not one this file can fix
by teaching a longer command; it can only name it: the command log teaches
the *equivalent* a user would type, per invariant 4, and an equivalent that
hands back the reverse of what the pane just showed is exactly the surprise
invariant 4 exists to prevent, not one it excuses.

**It also matches on one field this pane's own fetch does not stop at.**
`kubectl events --for` selects by kind, apiVersion and name; the pane's
`involvedObject` selector adds `uid`, so a replacement object under the same
name is a different match to the pane and the same match to the typed line.
That is why the `uid` term is in the selector at all, not a decoration on
it: without it, a StatefulSet pod deleted and recreated under the same name
inside the event TTL would have the typed command hand back its
predecessor's events where the pane shows none — a consequence read off the
selector, not yet measured against a cluster.

**The footer reads `[ ] tabs  esc back  ? all keys  q quit`, the same as
describe's.** Nothing else applies: there is no follow (this is a fetch, not
a stream — invariant 6 keeps events off the permanent watch, so there is
nothing to tail), no container picker (an event is not scoped to one
container), no `⇧p previous` (an event has no earlier version to ask for),
and no reveal (nothing on this pane is a secret). Offering any of them would
be exactly the promised-key-that-does-nothing bug
[the README's key rules](README.md#the-five-rules-every-screen-obeys)
already forbids.

## The yaml tab

**A fresh, unpruned GET — never the watch store.** The store is pruned to the
fields `rules.rs` names (invariant 6); a yaml pane fed from it would show a
partial object and call it the object. So `y` re-reads, the same way `d`
does, and pays the same one extra round trip for the same reason: this is
the one pane on the product where "the object, exactly as the API returned
it" is the entire point, and a stale, pruned stand-in would be lying about
what it is.

**This mockup and the two after it are drawn 80 columns wide, not this
file's usual 70** — the first width exception anywhere in `screens/`, and a
measured one, not a stylistic one. `kubectl`'s own `--show-managed-fields`
is 22 characters on its own, and once it is added to the teaching line
(below), no object name that still reads as one fits inside a 68-column
strip: the shortfall was checked with the actual flag and the actual
namespace, not estimated. 80 is not a wider terminal than this product
promises — it is [the same 80×24
minimum](README.md#how-to-read-them) this whole directory already targets,
drawn at the width it actually has rather than the narrower 70 chosen
elsewhere for the page's own readability; the sidebar stays the fixed 20 it
always is, and the content pane takes the extra 10 columns, exactly the rule
[widgets.md § 1](widgets.md#1-the-frame) already states for any terminal
wider than the minimum. Nothing here works *only* wider than 80×24 — it is
what 80×24 already draws, shown at its own size instead of a narrower one.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                                     │
│  RESOURCES         │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Pod                                                │
│   config           │metadata:                                                │
│   cluster          │  name: web-7d9f4                                        │
│  ANALYSIS          │  namespace: payments                                    │
│   capacity      1 ▲│  labels:                                                │
│   certificates  30d│    app: web                                             │
│   drain safety     │  managedFields:                                         │
│   posture          │    - manager: kubelet                                   │
│   restarts         │      operation: Update                                  │
│   waste            │      …                                                  │
│   versions         │spec:                                                    │
│                    │  containers:                                            │
│                    │    - name: app                                          │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields        │
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **`--show-managed-fields` is on the teaching line because `kubectl` has
  hidden `managedFields` from `get -o yaml` by default since 1.21, and k8rs
  does not** — measured on a real pod: 95 of 246 lines, 39% of the document,
  are `managedFields` (`k8s-admin`, 2026-08-31). Dropping them to match
  `kubectl`'s default was considered and refused: this pane's one claim is
  that it is the object, and pruning a field the object actually carries to
  make the printed line simpler is the exact failure [the yaml pane's own
  intro above](#the-yaml-tab) already refuses for the watch store. The flag
  says, honestly, what the command underneath was always going to return.
- **`managedFields` is real YAML and gets no special treatment beyond
  wrapping** — shown short for the page above (`…`, the same marker every
  other trimmed-for-the-page block in this file uses), not because k8rs cuts
  it. A reader who wants the whole thing has the whole thing; this is a
  page-width choice, not a product one.
- **No two-space reading margin here**, unlike every other tab on this
  screen. The pane's own left edge *is* the document's, so the YAML's own
  indentation is the only indentation drawn — adding a margin on top of it
  would misrepresent what the object's own structure is, which is the one
  thing this pane exists to get right. `yaml` and `logs` are the two panes
  that do not wrap-trim for exactly this reason
  ([widgets.md § 2](widgets.md#2-element--widget)): leading whitespace is
  meaningful in both.
- **Key order is the API's, never alphabetised** — already promised in the
  tab table at the top of this file, restated here because it is this pane's
  whole contract with a reader who already knows what `kubectl get -o yaml`
  looks
  like and is checking that this is the same thing.
- **Nothing here is masked except on a Secret — and there, `data` is not
  the only field that carries the Secret.** Every value under
  `metadata.annotations` is masked too, the same way and for the same
  reason as `data`, below — an annotation on a Secret is treated as a copy
  of the Secret, not as metadata about it ([NOTES §
  D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
  `metadata.labels` is not masked: it is validated to 63 characters by the
  API server itself, nothing writes a Secret's body into one, and the
  review that found the annotation leak looked and found none — a residual
  named on purpose rather than silently assumed safe.
- **A Pod's literal environment values**, if it has any set directly rather
  than through a `secretKeyRef`, are shown exactly as the API returned them,
  unredacted. That reads as a contradiction of *"environment variable values
  are never displayed"* ([REQUIREMENTS §
  DevSecOps](../REQUIREMENTS.md#devsecops-requirements) ·
  [docs/security.md § Data displayed and stored](../docs/security.md#data-displayed-and-stored))
  until the rule that already settled it is read: that line governs what
  *k8rs goes and fetches and interprets on its own initiative* — the same
  words cover Secret data — and it is explicitly not a mandate to hide what
  `kubectl` already shows verbatim
  ([NOTES §
  D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12) ·
  [D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)).
  A yaml pane that diverged from `kubectl get -o yaml` on an ordinary field
  would be lying by omission about what it is, which is the exact failure
  D37 already ruled against for a controller's own message. Building a
  detector for "this field looks like a secret" is the masking engine
  REQUIREMENTS itself already calls YAGNI — Secret `data` and, now,
  `metadata.annotations` get their own rule because each is a *named,
  structural* field, on a *specific kind*, not a heuristic over free text.

### A Secret, values hidden behind an explicit reveal

`data`'s values are one thing this pane always masks, by key, with the size
the value decodes to rather than the value itself — and, since this review,
so is every value under `metadata.annotations`, by the same rule:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/db-credentials                                │
│  RESOURCES         │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Secret                                             │
│   config           │metadata:                                                │
│   cluster          │  name: db-credentials                                   │
│  ANALYSIS          │  namespace: payments                                    │
│   capacity      1 ▲│  annotations:                                           │
│   certificates  30d│    kubectl.kubernetes.io/last-applied-configuration:    │
│   drain safety     │    <hidden — 612 bytes>                                 │
│   posture          │  managedFields: …                                       │
│   restarts         │type: Opaque                                             │
│   waste            │data:                                                    │
│   versions         │  username: <hidden — 8 bytes>                           │
│                    │  password: <hidden — 16 bytes>                          │
│                    │  tls.crt: <hidden — 1,172 bytes>                        │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get secret db-credentials -n payments -o yaml...                   │
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  v reveal  esc back  ? all keys  q quit                             │
└──────────────────────────────────────────────────────────────────────────────┘
```

**This is the first line on the page that does not fit, and it is cut, not
clipped.** `--show-managed-fields` makes the full teaching command 77
characters; the strip has 76 to give it — pane width minus the outer border
minus the one-column margin `indented()` reserves on each side
([widgets.md § 1](widgets.md#1-the-frame)) — even at the true 80-column floor
this section already draws at. The cut walks back to the last space rather
than stopping mid-flag, the rule [the evidence line already
follows](widgets.md#7-text-that-came-from-the-api): a flag with its last
character sheared off would still look like a flag, and `--show-managed-fiel`
is not one a reader would notice was wrong. Dropping the whole flag instead
leaves `...` right after `yaml` — the strip's own three-period mark, never
the single `…` glyph, because on this one strip `…` already means something
else, the running mark ([widgets.md § 7, back-cut
3](widgets.md#7-text-that-came-from-the-api)) — and if a reader deletes just
the `...`, what is left is a real command: the one `kubectl get -o yaml`
already runs by default, without `managedFields`. A wider terminal never
needs this cut; 80×24 is the floor, and the content pane only grows from
here.

**Why an annotation on a Secret gets treated as a copy of the Secret rather
than as metadata about it.** `kubectl apply -f secret.yaml` — the ordinary
way a Secret is created — writes the whole applied body, `data` included,
into `metadata.annotations["kubectl.kubernetes.io/last-applied-configuration"]`,
base64 inside base64; a Secret written through `stringData` puts
**plaintext** there instead, not even base64. Measured: that annotation's
own bytes decode to the same `username`/`password` values the block above
already shows masked, so masking `data` and leaving `metadata.annotations`
alone would tell a reader this document is safe to paste into a ticket while
the second copy sits four lines away, untouched (`k8s-admin`, 2026-08-31;
NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
**Every key in `metadata.annotations` is masked, not only this one** — a
denylist of `last-applied-configuration` by name is invariant 1's own
allowlist-not-denylist reasoning applied one layer up: every GitOps
controller that reconciles a Secret writes its own reconstruction into its
own annotation key, and a mask that only catches the one `kubectl` happens
to write catches nothing the day a different controller manages the object.
The key is still drawn, so a reader can see *that* something is stored
there and go looking with `kubectl` if they need to; only the value is
replaced by its size.

**`v` reveals `data`, and only `data`** — the modal below lists
`username` / `password` / `tls.crt`, never the annotation. A copy nobody
asked to see is not a value somebody pressed a key to read; revealing it
would be k8rs itself decoding and displaying a controller's own
reconstruction of the Secret, which is one more place the plaintext could
end up on screen for no reader benefit over the three real keys already
there. If a reader genuinely needs what is inside that annotation, `kubectl`
still shows it — k8rs choosing not to make it one keypress easier here is
the whole point of masking it in the first place.

`v` is new — free, not used anywhere else in the key map (`r` already means
*restart*, everywhere, and could not be reused here even though "reveal"
reads just as naturally). It only appears in the footer when the object is a
Secret **and** it has at least one key — a key that does nothing is a bug
this product has already shipped once, and a zero-key Secret has nothing to
reveal (below).

**`v` opens a small modal; it does not rewrite the pane.** This is the
choice the brief leaves to this file, and it is what makes the security
gate's own sentence literally true rather than a matter of trusting the
terminal's mouse selection: *"a revealed value never enters the command log,
the audit log or the YAML shown by `y`"*
([docs/security.md § Data displayed and stored](../docs/security.md#data-displayed-and-stored)).
If reveal rewrote the pane's own text in place, whatever copies that text —
a future export, a future "copy the whole document" key — would carry the
plaintext with it. A modal is a separate, disposable view: what it shows
never becomes part of the document, so the document — the thing `y` is
named for — stays masked no matter how many times a value is looked at.
This reuses the existing modal layer rather than inventing a new mechanism
([widgets.md § 5](widgets.md#5-the-modal-layer)), the same shape the
container picker already is.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ payments/db-credentials — revealed ────────────────────────┐   │
│   │                                                            │   │
│   │  username     admin                                        │   │
│   │  password     hunter22                                     │   │
│   │                                                            │   │
│   │  tls.crt      binary — 1,172 bytes, not shown as           │   │
│   │               text                                         │   │
│   │                                                            │   │
│   │  esc closes this — nothing was sent to the                 │   │
│   │  cluster to show it.                                       │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl get secret db-credentials -n payments -o yaml...         │
├────────────────────────────────────────────────────────────────────┤
│ esc close                                                          │
└────────────────────────────────────────────────────────────────────┘
```

- **No new command-log line.** The value was already in the object k8rs
  already read; revealing it decodes bytes already in memory and draws them.
  Nothing is sent to the cluster, so the command strip stays on whatever it
  already showed, and the modal's own footer says so in the reader's own
  words rather than leaving the absence of a `$` line to be noticed.
- **A value that is not valid UTF-8 once decoded is never printed as text.**
  `tls.crt` above is the ordinary case for that key — a certificate is DER
  bytes, not a string — and printing arbitrary bytes into a terminal is the
  same class of risk invariant 9 exists to close, worse here because a
  Secret is exactly the content most likely to be adversarial-shaped by
  accident. The reveal names the byte count and says plainly that it is not
  shown as text; it does not attempt a lossy decode that would show
  something that was never actually in the Secret.
- **All keys reveal together, not one at a time.** The pane holds a scroll
  offset and nothing else ([widgets.md § 2](widgets.md#2-element--widget)) —
  no per-line selection exists to point `v` at a single key, and adding one
  only for this would be new state carried for one feature. A Secret with
  many keys is the uncommon case; scrolling the modal like any other
  overflowing one is not a new idea this file has to introduce.

### A Secret with no keys

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/pending-secret                                │
│  RESOURCES         │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Secret                                             │
│   config           │metadata:                                                │
│   cluster          │  name: pending-secret                                   │
│  ANALYSIS          │  namespace: payments                                    │
│   capacity      1 ▲│  managedFields: …                                       │
│   certificates  30d│type: Opaque                                             │
│   drain safety     │data: {}                                                 │
│   posture          │                                                         │
│   restarts         │  This Secret holds no keys yet.                         │
│   waste            │                                                         │
│   versions         │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get secret pending-secret -n payments -o yaml...                   │
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back  ? all keys  q quit                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Same cut, same reason.** `pending-secret` is the same length as
`db-credentials`, so with `--show-managed-fields` on it this line is the same
77 characters against the same 76-column floor — cut in the same place, for
the reason spelled out at [the mockup above](#a-secret-values-hidden-behind-an-explicit-reveal).

No `metadata.annotations` here — this Secret has none yet, and an empty
section is not drawn any more than an empty `data` map invents keys that
are not there.

`data: {}` is drawn exactly as the API returns it — there is nothing to mask
because there is nothing there — and the one-line explanation under it says
so in a reader's words rather than leaving an empty map to be interpreted.
`v` is absent from the footer, same rule as the multi-container picker not
being offered to a single-container pod: nothing to act on, so no key.

### An object that does not exist, and a very large object

**Does not exist** is the identical state described's has, reached by the
identical first read: [§ An object that does not exist](#an-object-that-does-not-exist)
and the headless sentence below both apply unchanged, because `yaml`'s first
step is the same `k8s::pod()` call.

**Very large** is deliberately *not* bounded the way the log buffer is
([§ The buffer](#the-buffer-2-mb-retained-5000-lines-4096-bytes-per-line)).
A log is a stream with no natural end; one GET of one object is not — its
size is already bounded by what the Kubernetes API server will accept for a
single object in the first place, a ceiling this product does not own and
does not need to police a second time. The pane simply scrolls, the same way
[the Analysis panes that refuse to cap already do](analysis.md#restarts),
holding the whole document once rather than retaining a stream indefinitely.
**This is a deliberate, narrow exception to
[widgets.md § 7](widgets.md#7-text-that-came-from-the-api)'s closing rule —
*"long values are bounded before they are stored, not at draw time"* — and
that section does not currently say so.** Flagged here rather than edited
there, since only `tui-designer` writes `screens/` and `widgets.md` is a
different file: the PM's call whether it gets its own cross-reference.

### Free text that carried control characters — reversed for this one pane

**This file was wrong, and the reversal is measured, not reasoned** (NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
The section this replaces said every control character is stripped here the
same way an event message strips one, silently, no exception — and `\n` is
a control character. Measured against a real ConfigMap:
`kubectl get cm coredns -n kube-system -o yaml` is 33 lines; the same object
through this pane, as this section was written until now, printed **20** —
its 20-line `Corefile` value collapsed onto one, because every newline
inside it had already become a space. A reader who redirected that into a
file and re-applied it would have shipped a different config than the one
running.

**The ruling: on this pane, `\n` and `\t` survive; everything else
`unprintable` refuses still does not.** `ESC`, `U+202E` and `U+200B` are
still stripped, checked again in the same run that found the Corefile bug —
this is not "the yaml pane trusts the cluster now," it is that a newline
and a tab are not the class of thing invariant 9 exists to catch. The class
is characters that *do something* to a terminal instead of printing as
themselves: an escape sequence, a bidi override, a zero-width joiner. A
newline inside a YAML document does exactly what it says — starts a new
line — which is what the document already looked like before k8rs read it.

```
data:
  Corefile: |
    .:53 {
        errors
        health
        ready
    }
```

drawn as four real lines under `Corefile: |`, matching what `kubectl get cm
coredns -o yaml` already shows, not collapsed onto one the way this section
used to specify.

**Read against [describe's own copy of this section](#free-text-that-carried-control-characters),
this narrows it rather than contradicts it — and does not touch it.** Every
row on the describe tab is a cell in a list: one line for a container, one
line for an event, wrapped by width but never carrying a hard line break
of its own, because a `\n` inside an event message would not print as a
second line of that event, it would open a second row that looks like a
second event. The yaml pane is the other case — the payload *is* the
document, `\n` prints as exactly what it is there, and stripping it is what
was wrong. One predicate, `unprintable`, decides what invariant 9 removes
everywhere; what changed here is which surface a newline is judged
*against* — a cell's layout, or a document's own content — and only the
document path reads `\n` and `\t` as printing correctly. `ESC`, `U+202E`
and `U+200B` still have nothing to do with either judgement: they do not
print as themselves in a cell or in a document, so both paths keep
removing them, silently, with nothing marking the cut — the same as any
other free-text field on this build.

### Printed instead of drawn — yaml on the headless surface

Same split, same reused pod-fetch, same `--object`
([§ Printed instead of drawn — describe](#printed-instead-of-drawn--describe-on-the-headless-surface)) —
`--yaml` is the third verb beside it. The payload is the document itself,
whole, on stdout; the teaching line is on stderr.

**`--yaml` also takes `--kind`, defaulting to `pod`.** Every other verb on
this surface only ever reads a pod, so `--object` alone was enough; `--yaml`
is the one that has to say which kind of object it means, because the
Secret masking below has no other caller in this phase and `k8s.rs` freezes
at the end of it — code that ships with no reachable caller can only ever be
unit-tested, never run for real, and this repo's own rule is that something
is run every box. `--kind` resolves through the discovery machinery
`k8s.rs` already built for the browser (`browsable()` /
`ApiResource::from_gvk_with_plural`), so this is not a second, hand-written
notion of what a kind is — invariant 12 still holds. **`--object`'s own
parse does not change**: it stays `[namespace/]name`, one reader shared by
all three verbs, precisely so the kind travels in its own flag instead of a
second parse of `--object` learning to disagree with the first.

**`--kind` takes a bare word, like `secret`** — or, when one word names two
different things this cluster serves, that word followed by a dot and one
more part naming which. This is `kubectl`'s own spelling, the one a reader
already knows if they have ever had to use it. A bare word works whenever it
names only one thing; the dotted form is for the rare case below where it
does not.

The default case is unchanged from every earlier example in this file, other
than one flag — `--show-managed-fields`, kept on every yaml-tab teaching
line for the same reason it is on the drawn pane's: `kubectl` hides
`managedFields` from `-o yaml` by default and k8rs does not, so the line
k8rs prints has to say the one thing that makes it produce the same
document:

```
$ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields
apiVersion: v1
kind: Pod
metadata:
  name: web-7d9f4
  namespace: payments
  managedFields:
    - manager: kubelet
      operation: Update
      …
…
```

(shown short for the page — nothing here is cut by k8rs; the real object,
`managedFields` included, continues to its real end.) A multi-line value
anywhere in this document — a ConfigMap's `Corefile`, a Secret's PEM block
once revealed — prints as the many real lines it is, not squashed onto one;
[the drawn pane's own reversal above](#free-text-that-carried-control-characters--reversed-for-this-one-pane)
is one predicate reading one surface, so stdout gets the same fix the pane
does, not a second copy of it.

**Naming a kind is what makes the Secret masking above provable rather than
merely written.** `k8rs --yaml --object payments/db-credentials --kind secret`
is a real command against a real cluster now, and this is what it prints —
the identical masked `data:` **and** `metadata.annotations` the drawn
mockup shows, because both surfaces call the one masking function:

```
$ kubectl get secret db-credentials -n payments -o yaml --show-managed-fields
apiVersion: v1
kind: Secret
metadata:
  name: db-credentials
  namespace: payments
  annotations:
    kubectl.kubernetes.io/last-applied-configuration: <hidden — 612 bytes>
  managedFields:
    …
type: Opaque
data:
  username: <hidden — 8 bytes>
  password: <hidden — 16 bytes>
  tls.crt: <hidden — 1,172 bytes>
```

**There is no `--reveal` flag, and there will not be one on this surface**:
a reveal is a keypress on a drawn pane and Phase 6 has no pane, so `--yaml`
on a Secret redacts unconditionally, with no way to ask for the plaintext
headlessly. This is a ruling, not an oversight, and it holds regardless of
`--kind` making the masked path reachable — reachable is not the same as
revealable.

| Failure | Stream | Exit |
|---|---|---|
| `--object` names no such object of the given kind | stderr: `k8rs: there is no pod named ghost in payments — check the name and the namespace` (the kind's own singular in place of *pod*) — a cluster-scoped kind drops both the namespace clause and the *namespace* word: `k8rs: there is no node named ghost — check the name`, the same rule README's [§ The five rules every screen obeys](README.md#the-five-rules-every-screen-obeys) already states for a namespace shown only where there is one | `2` |
| Cluster unreachable / login expired | stderr: the same `because(...)` sentence `logs_run` prints | `2` |
| The object fetch does not answer inside the timeout | stderr: `k8rs: this cluster has not answered for the pod … in … after … seconds` (kind's own singular in place of *pod*, same as the row above) | `2` |
| The document could not be serialised to YAML | stderr: one sentence naming what failed | `2` |
| stdout write fails, not `BrokenPipe` | stderr: `k8rs: the report could not be written — …` | `2` |
| stdout write fails with `BrokenPipe` (the reader closed the pipe, e.g. `\| head`) | nothing | `0` |
| `--kind` with nothing after it | stderr: `k8rs: --kind needs the name of a kind` + usage — the same three-shapes-of-nothing check `--namespace` and `--context` already get (`fn mistyped`) | `2` |
| The cluster does not serve a kind by that name | stderr: `k8rs: this cluster does not serve a kind named widget — check the spelling` | `2` |
| The kind word names more than one thing the cluster serves and neither spelling was qualified — `events` is the real example: `core/v1` and `events.k8s.io/v1` both serve it, and `browsable()` keeps both, adjacent, because they are different resources | stderr: `k8rs: --kind events matches two things this cluster serves — the original one, and the one events.k8s.io adds. Say which: --kind 'events.' for the original one, or --kind 'events.events.k8s.io' for the other` | `2` |
| Success | stdout: the document | `0` |
