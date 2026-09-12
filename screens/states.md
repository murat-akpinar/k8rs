# Screens — First launch, empty, and everything going wrong

The states that decide whether a newcomer keeps the tool. All of them were
undefined until they were written down, and most of them happen on **first
launch**.

## Nothing is broken

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│  RESOURCES         │                                               │
│   workloads        │               ○  nothing is broken            │
│   network          │                                               │
│   storage          │        84 pods and 3 nodes checked, none of   │
│   config           │        them is in trouble right now.          │
│   cluster          │                                               │
│  ANALYSIS          │        Worth a look anyway:                   │
│   capacity      1 ▲│          ANALYSIS → capacity   (1 node is     │
│   certificates  30d│          promising more than it has)          │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

An empty list is a failure state in most tools; here it is the goal, so it is
drawn as an answer and points at the report that still has something to say.
This screen is only honest because Alerts holds nothing but *broken right
now* — a lint report would never be empty.

- **The footer withholds `s scale` and `r restart`, and nothing else.** Zero
  findings means nothing is selected, so the two keys that act on a selected
  object are not drawn — the same rule the empty kind in the browser already
  follows, below. `↑↓ move` and `⏎ open` stay: the sidebar's own rows
  (`RESOURCES`, `ANALYSIS`, `capacity`, …) are still there to move across and
  open, and — unlike [Still loading](#still-loading), just below — every
  one of their own badges has already settled, `capacity  1 ▲` included,
  because this screen is drawn only once that read has completed. `/ filter`
  stays too, for the reason the empty
  kind gives it: a pane-level control, not an operation on an object, so it is
  honest to offer even over a list with nothing in it right now
  ([widgets.md § 2a](widgets.md#2a-the-footer)'s own bucket for this state,
  grouped with disconnected, login-expired, clock-skew, namespace-scoped and
  [the audit log failing to open](#the-audit-log-could-not-be-opened) — one
  shape, "ordinary, mutations withheld," for all six).

## An empty kind in the browser

Not the same claim as the one above. *Nothing is broken* is Alerts' verdict
on the whole cluster, computed once and stated in one voice. `jobs` coming
back with zero rows is not a verdict on anything — the cluster can have an
OOMKilled pod three panes over while `jobs` is legitimately empty, and
pairing the two would make the browser disagree with the screen it is never
allowed to disagree with
([resources.md § Rules](resources.md#rules), *alerts bleed through*). So the
empty pane borrows the shape — centred, dim — and none of the four reserved
symbols: `●` `▲` `○` are severities and this carries none, `⚠` is a
connection or trust problem and this is neither
([README § the five rules, item 4](README.md#the-five-rules-every-screen-obeys)).
No glyph, one line of dim text.

There are exactly three sentences, one per reason the pane can be empty:

| When | The pane says |
|---|---|
| A namespaced kind, scoped to one namespace (title already reads `ns: payments`) | `no jobs in payments` |
| A namespaced kind, no scope in effect — [browsing every namespace](resources.md#browsing-every-namespace), the ordinary case today | `no jobs in this cluster` |
| The kind that was selected is no longer in the sidebar's own list (discovery changed mid-frame) | `no longer in the list — pick another kind` |

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│  ALERTS     3 ● 7 ▲│  jobs                                         │
│  RESOURCES         │                                               │
│▸  workloads        │                                               │
│     deployments  12│                                               │
│     statefulsets  3│                                               │
│     daemonsets    5│            no jobs in this cluster            │
│     pods         84│                                               │
│     jobs          0│                                               │
│   network          │                                               │
│   storage          │                                               │
│   config           │                                               │
│   cluster          │                                               │
│  ANALYSIS          │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get jobs -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ / filter  ? all keys  q quit                                       │
└────────────────────────────────────────────────────────────────────┘
```

- **The footer loses `⏎ open`, `s scale` and `r restart`.** All three act on a
  selected object (invariant 2), and zero rows leaves nothing to select —
  showing them would be exactly the "promised key that does nothing" this
  file's own key rule already forbids
  ([README § the five rules, item 2](README.md#the-five-rules-every-screen-obeys)).
  `↑↓ move` goes with them; there is nothing to move a cursor across.
  `/ filter` stays: it opens a pane-level control, not an operation on an
  object, so it is still honest to offer even though this particular list has
  nothing to narrow. `? all keys` and `q quit` stay too — they hold on this
  pane and every other one on this page: their one named exception is a
  mutation call in flight, and a pane with nothing in it has none
  ([widgets.md § The footer](widgets.md#2a-the-footer)).
- **`no jobs in this cluster` is the ordinary state, not a fallback** — see
  [resources.md § Browsing every namespace](resources.md#browsing-every-namespace):
  without a namespace scope, `-A` is what k8rs is always doing today.

Scoped to one namespace, only the sentence changes — the title still carries
`ns: payments`, so the sentence does not repeat it:

```
┌───────────────────────────────────────────────┐
│  jobs          ns: payments                   │
│                                               │
│              no jobs in payments              │
│                                               │
└───────────────────────────────────────────────┘
```

And when the kind itself is the thing missing — the sidebar's own list moved
out from under the pane's remembered position, so there is no kind name left
to put in a sentence:

```
┌───────────────────────────────────────────────┐
│                                               │
│   no longer in the list — pick another kind   │
│                                               │
└───────────────────────────────────────────────┘
```

- **The title bar is blank, not guessed.** Same rule the header's own vitals
  already follow — a fact k8rs cannot read is blank, never invented
  ([widgets.md § 1a](widgets.md#1a-the-header-row)) — applied here to a kind
  name instead of a node count.
- **The sentence still names a next step**, unlike a bare "nothing here":
  every other state on this page ends by saying what to try next, and a rare
  state is not the one to make an exception of. `⏎`, `s`, `r` and `ctrl-d` are
  gone here too, for the same reason as the ordinary case above — there is
  not even a kind to say nothing was selected *of*.
- This is a resync glitch, not a failure — nothing to retry, nothing to
  report. It clears itself the moment the reader picks any kind from the
  sidebar, which is why the sentence sends them there instead of explaining
  the mechanism.

## Still loading

```
 nodes …                        k8rs      ctx: prod-eu · connecting…
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│  RESOURCES         │        reading the cluster… 2,140 pods        │
│   workloads        │                                               │
│   network          │        Large clusters take a moment. Findings │
│   storage          │        appear as they are found — this list   │
│   config           │        fills up, it does not wait.            │
│   cluster          │                                               │
│  ANALYSIS          │                                               │
│   capacity         │                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ ? all keys  q quit                                                 │
└────────────────────────────────────────────────────────────────────┘
```

`↑↓ move` and `⏎ open` are gone, and the honest reason is not that the
sidebar is off screen — it is drawn, cursor and all, the same as on every
other page in this file. It is that the sidebar is itself still mid-read:
`capacity` carries no badge yet, while `certificates  30d` already does,
because that number is C1's — read straight out of the kubeconfig's own
client certificate, zero cluster traffic
([NOTES § v1 rule set](../NOTES.md#v1-rule-set)) — while `capacity` needs the
node and pod data this screen is still waiting on. The mockup above draws
both states in the one sidebar to make the point checkable rather than
asserted. Moving into a row and opening it is exactly what would
let someone act on a count that has not finished arriving, which the "vital
that cannot be read is blank, never guessed" rule already refuses for the
header vitals ([widgets.md § 1a](widgets.md#1a-the-header-row)) — the same
protection extended to the sidebar's own rows while they are still filling
in. By [Nothing is broken](#nothing-is-broken), that same read has
completed, every row's own badge is final, and the two keys come back. `?`
is not withheld either way: help does not need a finding to explain, and the
anchor pair holds here — its one exception is a mutation call in flight, and
the read that is still filling this sidebar is not that
([widgets.md § The footer](widgets.md#2a-the-footer)).

## The connection dropped

The header is the honest one. Stale data drawn as if it were live is
forbidden.

```
 nodes 3/3 (40s ago)          ctx: prod-eu · ⚠ disconnected, retrying
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│  RESOURCES         │  ⚠ Not connected to the cluster right now.    │
│   workloads        │    What you see below is from 40 seconds ago. │
│   network          │    Retrying…                                  │
│   storage          │                                               │
│   config           │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   cluster          │    Containers exceeded their memory limit     │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   (reconnecting)                     │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **`s scale` and `r restart` are withheld, not marked `no`.** A stale card is
  still a selected object in principle, so this is not the empty-pane reason
  the states above use — it is the other reason a key can be missing, and the
  two must not be confused: `no` is `may_i_in`'s answer about *this login's
  permission*, and a dropped connection is not a permission question at all.
  k8rs does not know whether a write would be allowed right now because it
  cannot ask, and drawing `s no scale` would claim a verdict nobody gave. So
  the key is withheld outright, the same way it is under `--read-only` —
  structurally unreachable reads the same as never drawn.

## Your login expired

Not a 403, and not a dropped connection — the third case. On EKS, GKE and AKS
the kubeconfig mints a short-lived token from a credential plugin, and it runs
out mid-session ([NOTES § D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)).

```
 nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│  RESOURCES         │  ⚠ Your login expired.                        │
│   workloads        │                                               │
│   network          │    The cluster still knows who you are, but   │
│   storage          │    the login token your kubeconfig creates    │
│   config           │    has timed out.                             │
│   cluster          │                                               │
│  ANALYSIS          │    Renew it, then press X and pick this       │
│   capacity      1 ▲│    cluster again:                             │
│   certificates  30d│                                               │
│   drain safety     │    aws sso login                              │
│   posture          │                                               │
│   restarts         │    What you see below is from 2 min ago.      │
│   waste            │                                               │
│   versions         │                                               │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   → login expired                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  X switch cluster  / filter  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

- **"Your login expired" and "you are not allowed" are different sentences**
  because they send the user to different places: one is a command they run
  themselves, the other is a conversation with whoever owns the cluster.
  Printing the 403 text for a 401 sends a beginner to their platform team over
  a timeout.
- The renewal command comes from the `exec` block of their own kubeconfig —
  it is the binary k8rs was already told to run, not a guess about which
  cloud they are on. If the kubeconfig does not use a credential plugin, the
  line is omitted rather than invented.
- **`aws sso login` earns no indent of its own — it sits at column 4, the
  same hanging indent every other line in this banner already keeps.** A
  literal command was drawn two columns deeper here once, as if a thing to
  type deserved to be set apart from a thing to read; it does not need the
  extra column to be found; it is already alone on its own line with a blank
  above and below it, which is what actually sets it apart, and a second,
  narrower indent would be a rule this page owed the code and the code does
  not have. One blank line separates it from the sentence before and the one
  after, not several — the same spacing every other paragraph break in this
  banner keeps.
- Stale data stays visible and stays labelled, exactly as on the disconnected
  screen. k8rs does not clear the screen because it lost its token.
- **The footer keeps `↑↓ move` and `⏎ open`, for the reason the line above
  gives them a job to do.** The explanation this state needs is long enough
  to spend the whole content pane on it, so no card is drawn under it here —
  but the sidebar's own rows are exactly as navigable as on every other
  degraded screen on this page, `nothing is broken` included, and the stale
  list is one `X` and a fresh connection away rather than gone. Withholding
  the two keys because this one mockup had no room to also draw a card would
  make the footer a fact about the page's layout, not about the screen.
  `s scale` and `r restart` are withheld for the same reason as the
  disconnected screen just above — a write nobody can currently be asked
  about is not the same thing as a write refused, and looks the same on
  screen either way: absent, never `no`.
- **`X switch cluster` is the one key on this page drawn where it would
  otherwise stay behind `?`.** Every other screen leaves it bound and unnamed
  — [help.md](help.md) lists it under *Moving around*, always available, never
  on an ordinary footer — because on every other screen it is one option
  among several. Here it is *the* next step, named in the body two lines
  above the footer that repeats it, so a reader does not have to hold
  `aws sso login` in their head while hunting the key map for how to act on
  it.

### Over a pane with nothing to show yet

Reachable, and drawn rather than left for a caller to guess at: the token can
run out while a kind's own first `LIST` is still on the wire, or has already
come back with zero rows — a browser open on `jobs` for the first time, or
any pane still on [Still loading](#still-loading), the moment `X` and the
header's own `⚠ login expired` become true out from under it. **The pane
draws exactly what it would have anyway.** There is nothing yet to relabel
as stale — no card carries an age, no row exists to say *"from N ago"* about
— so the content pane is not this section's to rewrite a second time; the
token's death reaches the screen through the header, already true on every
degraded page, and the footer, which is:

```
 nodes …                        k8rs      ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│  RESOURCES         │        reading the cluster… 2,140 pods        │
│   workloads        │                                               │
│   network          │        Large clusters take a moment. Findings │
│   storage          │        appear as they are found — this list   │
│   config           │        fills up, it does not wait.            │
│   cluster          │                                               │
│  ANALYSIS          │                                               │
│   capacity         │                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ X switch cluster  ? all keys  q quit                               │
└────────────────────────────────────────────────────────────────────┘
```

```
 nodes 3/3                      k8rs     ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│  ALERTS     3 ● 7 ▲│  jobs                                         │
│  RESOURCES         │                                               │
│▸  workloads        │                                               │
│     deployments  12│                                               │
│     statefulsets  3│                                               │
│     daemonsets    5│            no jobs in this cluster            │
│     pods         84│                                               │
│     jobs          0│                                               │
│   network          │                                               │
│   storage          │                                               │
│   config           │                                               │
│   cluster          │                                               │
│  ANALYSIS          │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get jobs -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ X switch cluster  / filter  ? all keys  q quit                     │
└────────────────────────────────────────────────────────────────────┘
```

**`X` is promoted onto both, and the rule is one sentence, not two: it goes
wherever the login has expired, on Alerts and on the browser, whatever the
pane under it is showing.** Detail tabs keep their own footer regardless —
[widgets.md § 2a](widgets.md#2a-the-footer)'s closed mode list already draws
the anchor there and nothing else, `X` never among it, and an expired login
is not the thing that opens a fourth door into a footer that names none of
the other mutating keys either. The reason it was promoted at all — *"so a
reader does not have to
hold `aws sso login` in their head while hunting the key map"* — argues
hardest exactly here: these are the two frames with the least else on
screen, and a reader watching a spinner that will never resolve is the one
most likely to go looking for `?` next. Refusing the promotion because there
is no list to act on would be answering a question about **mutation
keys** (`s`, `r`, which withhold themselves for exactly that reason,
[§ The connection dropped](#the-connection-dropped)) as if it applied to a
**navigation** key it does not: `X` never acted on a selected row to begin
with, so *"nothing is selected"* was never its condition for appearing.
**Both keep their own pane's own shape otherwise** — Still loading still
drops `↑↓ move` and `⏎ open` for its own reason, the empty kind still keeps
`/ filter` for its own — an expired login changes one thing, the same one
thing, everywhere it is true.

## Your computer's clock is off

D55 found the direction the header owed an explanation was backwards, and D69
drew the boundary this box inherits: past five minutes of skew, `rules::age`
produces no number at all, not a wrong one
([NOTES § D55](../NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) ·
[§ D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
This is the state that says why, on the two screens whose whole promise is
that a number on them can be believed.

### It does not fit in the header, so it does not go there whole

*"This computer and the cluster disagree about the time by 11 minutes (this
one is behind), so recent times are missing and older ones can read smaller
than they really are."* is 171 characters. The header row is one line
([widgets.md § 1a](widgets.md#1a-the-header-row)), already carrying `nodes
3/3` on the left and up to four `·`-separated facts on the right, and the
context is never truncated. There is no wrapping, no abbreviation and no
reflow of that row that fits the sentence in whole.

So the header carries a **pointer**, sized like the two pointers already
living there — `⚠ disconnected, retrying`, `⚠ login expired` — and the
sentence itself goes where those two put their own explanation: the content
pane, above whatever else is there, the same slot [the namespace-scoping
banner](#you-can-only-see-some-namespaces) uses for the same reason (more to
say than one line holds).

```
ctx: prod-eu · live · admin · ⚠ your clock is behind
```

25 characters (` · ⚠ your clock is behind`) added to the busiest right zone
this file draws — `ctx: prod-eu · ns: payments · read-only`, 39 — lands at 64:
comfortably inside the 70-column page this file is drawn at and the 80-column
floor both ([the combined case, below](#your-clock-and-a-scoped-namespace-together)).
Neither word is jargon: [invariant 14](../CLAUDE.md) rules out "clock skew"
and "NTP", and "clock" plus "behind"/"ahead" is the whole vocabulary the
pointer needs.

**The pointer is the newest, lowest-priority segment in that zone**, so it is
the first of the *added* facts to drop if a longer context name or a TLS
warning ever left no room — after it, the existing sacrifice order
([widgets.md § 1a](widgets.md#1a-the-header-row): name, then vitals, never
context) still applies unchanged. In every case measured for this file, it
never has to: the worst-case right zone above still fits with 6 columns to
spare at 80.

### Two directions, two sentences, because they break differently

Behind the cluster, `age` does not fail one clean way: an event young enough
still returns `None` and blanks, but everything older prints a number that is
the whole gap too small — a crash long past can read as a minute old
([D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28)).
Ahead of the cluster only does the second thing: every age inflates by the
gap and nothing blanks. One sentence covering both directions would have to
hedge ("may be blank or wrong"), and a beginner reading a hedge does not know
which card in front of them to distrust — so each sentence names every effect
its own direction actually has, and neither assigns fault: k8rs measures a
*gap* between two clocks, not which one is wrong, and a middlebox or a
control-plane VM with a stopped clock can produce the identical reading an
unsynced laptop does (D177's second finding). So there are two:

| Direction | What actually happens | The sentence |
|---|---|---|
| **behind** the cluster | recent events blank; older ones print a number too small by the size of the gap ([D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28)) | *"This computer and the cluster disagree about the time by 11 minutes (this one is behind), so recent times are missing and older ones can read smaller than they really are."* |
| **ahead** of the cluster | every age inflates by the size of the gap; nothing blanks | *"This computer and the cluster disagree about the time by 9 minutes (this one is ahead), so times can read larger than they really are."* |

Both sentences are written for **any renderer that can hold one line**,
deliberately not tied to the word "screen" — [`--once` carries the identical
strings](once.md#when-your-clock-and-the-clusters-disagree), printed
unfolded rather than fit to a pane, the same rule every shared string in this
product already follows.

These replace the pair this file shipped hours earlier the same day, which
under-drew *behind* to blanking alone and named a culprit neither direction
had actually measured — both mistakes, and the evidence that caught them, are
[NOTES § D176](../NOTES.md#d176--the-clock-skew-line-does-not-fit-in-the-header-and-the-two-halves-do-not-share-a-sentence-2026-08-28)
and [§ D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28),
not re-argued here.

### The threshold: five minutes, the same five, both directions

`rules::age`'s `SKEW_ALLOWANCE` is already five minutes — the conventional
clock-skew tolerance D69 borrowed rather than tuned. This box reuses that
same number for the header pointer and banner, in both directions, rather
than inventing a second constant:

- **Below it, nothing on screen is different.** No age blanks; no finding is
  inflated by more than rule 12's own 60-second margin already absorbs
  ([D55](../NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12)).
  A sentence appearing before anything is visibly different would be a
  warning with nothing on screen to point at.
- **One number is one fewer thing to explain**, and D69's own argument for
  five minutes — the tolerance Kerberos, JWT `nbf`/`exp` and most TLS
  handshakes already settled on — does not argue for a slow laptop
  differently than it argues for a fast one.
- This is a **tui-designer** call, not a restatement of D69: D69 bound only
  the *blanking* threshold on the *behind* side. The *ahead* side's threshold,
  and the header pointer's own trigger point on both sides, were open, and
  are decided here, together, at the same number.

### Behind the cluster

```
 nodes 3/3       ctx: prod-eu · live · admin · ⚠ your clock is behind
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ⚠ This computer and the cluster disagree     │
│  RESOURCES         │    about the time by 11 minutes (this one is  │
│   workloads        │    behind), so recent times are missing and   │
│   network          │    older ones can read smaller than they      │
│   storage          │    really are.                                │
│   config           │                                               │
│   cluster          │  ● payments/web  ·  3 of 5 pods               │
│  ANALYSIS          │    Containers exceeded their memory limit     │
│   capacity      1 ▲│                                               │
│   certificates  30d│  ▲ shop/api  ·  2 of 6 pods        1 min ago  │
│   drain safety     │    Running, but not receiving traffic — the   │
│   posture          │    readiness check is failing                 │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

**The footer is the ordinary one minus `s scale` and `r restart`, on every
mockup in this section.** Clock skew is not a connectivity or permission
problem — the header still reads `live · admin` — so this is not the
disconnected screen's reason, and neither key is marked `no`: nothing asked
`may_i_in` and nothing it would say is in question here. It is the reason two
paragraphs below: an age that reads fresher or staler than it really is is
exactly what decides which card someone reacts to first, and reacting here
means pressing `s` or `r` on it. Withholding both for as long as the skew
holds means the reader opens the card and reads its own text — which carries
no clock — before acting on one, rather than trusting a number this very
banner has just said cannot be trusted. `/ filter` stays: narrowing the list
on screen asks nothing of the cluster and trusts no age.

Two cards, two effects, and neither hides which one it is showing. On
`payments/web` the right edge — normally `4 min ago` on this exact finding
([the connection dropped](#the-connection-dropped)) — is simply absent: *No
number we cannot produce*
([alerts.md](alerts.md#the-rules-this-screen-obeys)), the same mechanism the
cordon card already uses for a taint stamped by hand
([alerts.md § the cordon card](alerts.md#the-cordon-card-with-and-without-its-clock)),
now firing on every recent card at once instead of one rare one — the scale
that turns a self-explaining absence into one that needs the banner above it.
On `shop/api`, `1 min ago` is not absent and is not flagged: it is the same
finding [§ What it prints](once.md#what-it-prints) shows at its true age,
`12 min ago` — computed against the wrong `now`, it reads as fresher than it
is, and a stale problem that reads as fresh is exactly what someone acts on
first — the failure
[D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28)
found this box's first draft missed entirely.

### Ahead of the cluster

```
 nodes 3/3        ctx: prod-eu · live · admin · ⚠ your clock is ahead
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ⚠ This computer and the cluster disagree     │
│  RESOURCES         │    about the time by 9 minutes (this one is   │
│   workloads        │    ahead), so times can read larger than they │
│   network          │    really are.                                │
│   storage          │                                               │
│   config           │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   cluster          │    Containers exceeded their memory limit     │
│  ANALYSIS          │    and were killed by the kernel (OOMKilled)  │
│   capacity      1 ▲│    limit 256Mi · exit 137 · 47 restarts       │
│   certificates  30d│    → raise limits.memory, or find the leak    │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

This card is **byte-for-byte identical** to one on a correctly-clocked
cluster. `4 min ago` is not flagged, not asterisked, not dimmed differently —
there is nothing on the card itself that can carry the doubt. Unlike the
behind direction, that is *always* true here — every card on an ahead-skewed
screen looks like this one, never blank — so the banner above is the *only*
signal on the whole screen, which is why it gets a whole sentence rather than
a symbol.

### Nothing is broken, and the clock is still off

The pointer and the banner do not wait for a finding to exist — they are a
statement about the data on screen, not about the cluster, and a laptop that
has simply never synced deserves to be told before anything else goes wrong:

```
 nodes 3/3       ctx: prod-eu · live · admin · ⚠ your clock is behind
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│  RESOURCES         │               ○  nothing is broken            │
│   workloads        │                                               │
│   network          │        84 pods and 3 nodes checked, none of   │
│   storage          │        them is in trouble right now.          │
│   config           │                                               │
│   cluster          │        This computer and the cluster disagree │
│  ANALYSIS          │        about the time by 11 minutes (this one │
│   capacity      1 ▲│        is behind), so recent times are missing│
│   certificates  30d│        and older ones can read smaller than   │
│   drain safety     │        they really are.                       │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

The `⚠` is gone here on purpose, not by omission. It belongs to the alarmed,
left-flush family this section's first two mockups borrow from
(`disconnected`, `login expired`); the calm, centred nothing-is-broken family
never uses it, exactly as [the namespace-scoped variant of this same
page](#nothing-broken-and-something-not-checked) already drops it for its own
caveat. Clock skew is drawn in whichever family the rest of the screen is
already in — it does not bring its own.

The `Worth a look anyway → capacity` line from the [plain nothing-is-broken
state](#nothing-is-broken) is left off this mockup for room, not for a rule:
unlike the namespace-scoped variant, clock skew does not switch the Capacity
check off, so that line is free to stay in the real screen. It is only absent
here because this file has fifteen rows to draw in and the longer, two-effect
sentence now uses five of them.

### Your clock and a scoped namespace together

Both banners can be true at once — being unable to list pods cluster-wide
says nothing about the machine's own clock. They stack, most-fundamental
first: the clock line before the namespace line, because a reader who cannot
trust *any* time on the page should be told that before being told which
*part* of the page they can see.

```
 nodes 3/3     ctx: prod-eu · ns: payments · read-only · ⚠ your clock is behind
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ⚠ This computer and the cluster disagree about the time│
│  RESOURCES         │    by 11 minutes (this one is behind), so recent times  │
│   workloads        │    are missing and older ones can read smaller than they│
│   network          │    really are.                                          │
│   storage          │                                                         │
│   config           │  You can't list pods across the whole cluster, so k8rs  │
│   cluster          │  is showing the namespace your kubeconfig points at:    │
│  ANALYSIS          │  payments. Use --namespace <name> for a different one,  │
│   capacity         │  or ask for cluster-wide read access.                   │
│   certificates  30d│                                                         │
│   drain safety     │  One node check is off: spotting a node someone         │
│   posture          │  started emptying and did not finish needs every pod…   │
│   restarts         │                                                         │
│   waste            │  ● payments/web  ·  3 of 5 pods                         │
│   versions         │    Containers exceeded their memory limit               │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                                       │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                │
└──────────────────────────────────────────────────────────────────────────────┘
```

This is the one mockup on this page drawn at the real 80-column floor rather
than the file's usual 70-column page width — `ns: payments · read-only ·
⚠ your clock is behind` alone is 44 characters, and at 70 columns total the
header has nowhere to put it beside `nodes 3/3` and `ctx: prod-eu`. At 80 it
fits with 6 columns of gap to spare; a context name one character longer
than `prod-eu`, or a fourth badge (a TLS warning), is what the sacrifice
order in the section above exists for — the clock pointer is the first of
the three to go.

**What gives way when two of these banners queue up, stated once, because a
third — the audit log's own — joins them below.** The body is **16 rows**,
measured off `ui.rs`'s own layout comment (`1 header + 1 top border + 16
body + …`, `src/ui.rs::draw`), not the 15 this page's mockups drew before
this round. This mockup and the audit log's own, further down, now draw the
16th — a real row where one is needed, the blank sidebar cell
[alerts.md](alerts.md)'s own longest card already uses for its overflow line
where one is not. **The rest of this page's mockups still draw 15**, and
that is a cosmetic debt this box did not chase down everywhere at once —
every one of them draws the 16th row blank on both sides today, which looks
identical to not drawing it at all, so nothing on screen is wrong; a later
pass should still true them up rather than leave two conventions on one
page. **The list beneath the stack keeps a
floor of 3 rows, never fewer** — [alerts.md § How wide a card is, and how
tall](alerts.md#how-wide-a-card-is-and-how-tall)'s own number, *"`shop/api`
gets three rows and that is the floor"* — so whatever queues above it, from
one banner to three, shares the other **13**. Each banner already reserves
its own trailing blank as the row that separates it from whatever is next
(`ui::banner`'s own `text.len() + 1`), so the 13 is spent on banners and
their separators together, not 13 of text alone.

**When what queues fits in 13, nothing is cut — this page's every
single-banner mockup, including the audit log's own further down, is this
case.** When it does not, one of them has to give, and *which* one is a
**rank, not a draw order**: **the audit sentence is always the first to give
way among whatever is queued, because it is the one fact on this page with a
second carrier.** Once writes are dead, the footer already withholds `s`
and `r`, and the header carries `read-only` (once the mark it needs is
wired — [§ The header reads `read-only`](#the-audit-log-could-not-be-opened)) —
two other places already say *this login may not change anything*, so the
audit banner's own
text can afford to be the one that shrinks first: what it alone still owns
is *why*, not *whether*. The clock pointer and the pane's own reason (which
namespace, which check is off, what command fixes a dead login) have no such
second carrier anywhere on the screen, so between the two of them the
existing order stands — clock first, because a reader who cannot trust *any*
time on the page should be told that before being told which part of it they
can see — but **neither one may give way to feed the audit sentence more
room; only the audit sentence gives way to feed them.** The draw order and
the give-way order are therefore the *same* order only when audit is the
thing actually queued last, which this page's mockups arrange for on
purpose: clock, then the pane's own reason, then the audit sentence, always
in that sequence, so *last drawn* and *first to give way* never come apart.

**Whichever banner is last in that order is wrapped as far as its own
remaining budget allows and marked with a visible `…` at a word boundary**,
the same rule [widgets.md § 7](widgets.md#7-text-that-came-from-the-api)
already uses for the card evidence line and the command log — never a
banner ranked above it, and never the list's own 3-row floor. Here, with no
audit sentence in the picture, the namespace denial is what's last and what
gives: the clock line is unchanged at 4 lines (5 rows with its own trailing
blank), and the namespace denial had 8 lines, of which only **7** fit in the
8 rows left (13 − 5) — so it loses exactly one word, `cluster.`, the
sentence's own last one. **The card gained a row it did not have before**,
not lost one: correcting the 15-row undercount handed the list back the one
row this mockup had been drawing as if it did not exist, so the identity
line now keeps its title too, where the earlier draft showed neither the
16th row nor the title.

**A banner whose own share of the 13 comes to fewer than two rows draws
nothing at all — not a one-line fragment, not a dangling mark, the whole
banner is absent — and that is a ruled outcome, not a silent one.** Two rows
is one line of text plus the row that separates it from whatever is next;
under that, there is no honest way to show a mark without a sentence for it
to sit at the end of. This is why the rank matters and not only the order:
it is always the *audit* sentence whose share can fall that low, because it
is last, and it is the one whose absence still leaves two other tellers —
the footer, the header — saying the same fact in fewer words. [The audit log
could not be opened](#the-audit-log-could-not-be-opened) draws every point
on this range: alone with room to spare, cut to a handful of words beside a
protected pane's-own reason, and cut to nothing at all beside two.

### While disconnected, or while the login has expired

Neither pointer nor banner survives a state where k8rs is not currently
completing requests, and this is not a special case written for clock skew —
[the connection-dropped header](#the-connection-dropped) and [the
login-expired header](#your-login-expired) are exactly as drawn on their own
pages, with no `⚠ your clock is behind` appended, even if one was showing the
moment before the connection or the token died. The reading needs a live
response's `Date` header to stay honest
([D55](../NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12)),
and a value computed from the *last* successful request is exactly the kind
of guess [the header's own rule](widgets.md#1a-the-header-row) forbids — "a
vital that cannot be read is blank, never guessed" applies here as much as it
does to `nodes …` while connecting. The clock line returns the moment a
request succeeds again, same as every other vital on the page.

### When there is nothing to say

Two more cases, and both are silence, not a new mockup:

- **The `Date` header is missing or does not parse.** Some proxies strip it;
  some do not send one at all. k8rs cannot measure a skew it cannot read, so
  neither the pointer nor the banner appears — the same "blank rather than
  guessed" rule as above, applied to the input instead of the output. This is
  indistinguishable on screen from a clock that is fine, which is the honest
  answer: k8rs has no evidence either way.
- **The skew is real but under five minutes.** Nothing on screen is
  different (the threshold section above), so nothing is drawn. This is the
  overwhelmingly common case — most laptops drift by seconds, not minutes —
  and it is why the pointer and banner are rare enough, when they do appear,
  to be worth reading in full.

## You can only see some namespaces

Not an error — the common case for anyone who is not a cluster admin. A 403
on the cluster-wide list falls back instead of failing
([NOTES § D5](../NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter)).

```
 nodes 3/3                    ctx: prod-eu · ns: payments · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  You can't list pods across the whole         │
│  RESOURCES         │  cluster, so k8rs is showing the namespace    │
│   workloads        │  your kubeconfig points at: payments.         │
│   network          │  Use  --namespace <name>  for a different     │
│   storage          │  one, or ask for cluster-wide read access.    │
│   config           │                                               │
│   cluster          │  One node check is off: spotting a node       │
│  ANALYSIS          │  someone started emptying and did not finish  │
│   capacity         │  needs every pod in the cluster.              │
│   certificates  30d│                                               │
│   drain safety     │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   posture          │    Containers exceeded their memory limit and │
│   restarts         │    were killed by the kernel (OOMKilled)      │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit  │
└────────────────────────────────────────────────────────────────────┘
```

- **`s scale` and `r restart` stay, and the header reads `admin`, not
  `read-only` — a namespace scope is not a permission and this file no
  longer says it is.** Whether a mutating key is reachable rides on the
  connection and the audit log, never on which namespace a session happens
  to be scoped to: a developer with a `RoleBinding` in `payments` — the most
  common non-admin RBAC shape there is — may scale and restart there, and so
  may an admin who simply typed `--namespace payments`. This mockup's own
  login is an ordinary admin session that happens to be namespace-scoped.
  **`read-only` is a separate fact from the scope, and the two must not
  share a wire**: a login that is *actually* read-only, or whose audit log
  [could not open](#the-audit-log-could-not-be-opened), withholds both keys
  for that reason wherever it is true, namespace-scoped or not — the header
  word belongs to the cause, never to the scope, so a later box wiring
  *namespace fallback → read-only* would be reopening the same confusion
  this bullet exists to close.

### The second paragraph is the point of this screen

A check that is switched off and says nothing looks exactly like a check that
passed. Two of them are switched off here, because both add up **every** pod on
a node and this view has a fraction of them
([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)):

| Off | Would have appeared | Said where |
|---|---|---|
| The half-finished drain — a node taken out of service with pods still to move | as a `node-3` card in **Alerts** ([alerts.md](alerts.md#under-namespace-scope-there-is-no-card-and-the-screen-says-so)) | the banner above, in the words drawn there |
| Overcommitted nodes — promised more than they have | as a row in the **Capacity report**, and as the `capacity  1 ▲` badge beside it in the sidebar | on that report when it is opened ([analysis.md](analysis.md#capacity-when-you-can-only-see-one-namespace)) |

- **Each screen names the check it would have run.** Alerts says the Alerts one;
  Capacity says the Capacity one. Nothing collects them into a single global
  notice, so adding a third disabled check later grows one screen by a sentence
  instead of growing this banner by a list.
- **The sidebar badge stays blank, and that is why the report has to speak.**
  `capacity  1 ▲` has room for a number, not for a sentence, and a fourth symbol
  meaning *not checked* would need a legend nobody has read yet — so the badge
  obeys the existing rule (a vital that cannot be read is blank, never guessed,
  [widgets.md § 1a](widgets.md#1a-the-header-row)) and the screen behind it
  carries the explanation.
- **This is a degradation, not a new mechanism.** It is what
  [docs/architecture § Error handling](../docs/architecture.md#error-handling)
  already specifies for a 403 on a secondary stream: the feature switches off
  and names what it needed.
- **The banner is above the list, not below it.** A reader who scrolls to the
  bottom of the findings to learn the list was incomplete has already believed
  it.

### The same screen, three ways it can differ

- **Two causes, one scope.** `--namespace payments` and a 403 on the
  cluster-wide pod list produce the identical state — `ClusterSnapshot` carries
  one `namespace_scope` for both ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  Only the first paragraph differs: the flag case reads *"Showing only the
  payments namespace, because `--namespace` asked for it"* and keeps the second
  paragraph unchanged, because the checks are off for the same reason either way.
- **Nodes may still be listable, and here they are.** Being scoped to a
  namespace for *pods* says nothing about *nodes* — the header keeps
  `nodes 3/3`, and N1 (a node that is not ready) and N3 (a node running out of
  disk or memory) still fire, because they read the node's own conditions and
  join nothing.
- **If nodes are not listable either**, the header's left zone is blank
  ([widgets.md § 1a](widgets.md#1a-the-header-row)) and the second paragraph
  says that instead, in the same slot: *"Nodes are not checked at all — your
  user can't list them. Missing permission: list nodes."* Same banner, same
  rule, different cause. It names the verb and the resource because that is the
  string the reader has to hand to whoever owns the cluster, which is the rule
  every other 403 on this page already follows.

### Nothing broken, and something not checked

The dangerous combination, and the reason the banner exists: *"nothing is
broken"* is the strongest claim k8rs makes, and under a partial view it is
making it while one check is switched off.

```
 nodes 3/3                    ctx: prod-eu · ns: payments · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│  RESOURCES         │               ○  nothing is broken            │
│   workloads        │                                               │
│   network          │        12 pods in payments and 3 nodes        │
│   storage          │        checked, none of them is in trouble    │
│   config           │        right now.                             │
│   cluster          │                                               │
│  ANALYSIS          │        One node check is off: spotting a      │
│   capacity         │        node someone started emptying and      │
│   certificates  30d│        did not finish needs every pod in      │
│   drain safety     │        the cluster.                           │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The claim is scoped to what was actually read.** `84 pods and 3 nodes` on
  the cluster-wide screen becomes `12 pods in payments and 3 nodes` — the
  sentence counts what k8rs looked at, never what the cluster has.
- **"3 nodes checked" and "one node check is off" are both true**, and reading
  them together is the whole point: the nodes were checked, but not for
  everything. Either sentence alone would mislead.
- **`Worth a look anyway → ANALYSIS → capacity` is gone from this variant**, and
  its absence is not a layout decision. Under this scope the Capacity report has
  nothing to say either; sending the reader there would be a tour of a second
  switched-off check.
- The wording of the missing check is **the same sentence** as the banner above,
  wrapped for this narrower block. One string, three renderers — the third is
  `--once`, which prints it unfolded rather than wrapped
  ([once.md](once.md#when-a-check-could-not-run)).

## The audit log could not be opened

The ninth state on this page, and the only one not driven by the cluster at
all: a full disk, a home directory k8rs cannot write into, or an
`$XDG_STATE_HOME` that points nowhere. [NOTES § D21](../NOTES.md#d21--if-the-write-cannot-be-audited-the-write-does-not-happen)
rules it — *k8rs says so and continues in read-only mode. It does not
exit* — and [§ D231](../NOTES.md#d231--the-audit-box-was-built-under-three-other-boxes-and-d21s-startup-clause-belongs-to-a-screen-that-does-not-exist-2026-09-05)
names this exact screen as the sentence's first true reader: a headless run
has no *continue* to continue into, so it refuses the line outright, but a
TUI can start, draw, and leave the write keys dead — which is what this page
draws for the first time.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  k8rs could not open its audit log at         │
│  RESOURCES         │  /home/you/.local/state/k8rs/audit.log (under │
│   workloads        │  your home directory): Permission denied (os  │
│   network          │  error 13) — every change k8rs makes is       │
│   storage          │  written to that log before it is sent, so    │
│   config           │  k8rs will not change anything until that is  │
│   cluster          │  fixed, and reading your cluster still works  │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   certificates  30d│    Containers exceeded their memory limit     │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The banner is one wrapped paragraph, byte for byte
  [`ops::audit_log`](../src/ops.rs)'s own returned string — never split back
  into re-punctuated sentences.** `audit_log` returns one `String`; `ui.rs`
  wraps it at the pane's width the same way it wraps a card's evidence line,
  it does not re-author it into paragraphs with blank lines between them,
  because a `Paragraph` given one string draws one wrapped block and nothing
  here builds a second string to hand it instead. The mockup used to draw
  three hand-broken paragraphs in reworded prose (*"Every change is
  written…"* for *"every change k8rs makes is written…"*) — that was this
  file inventing a sentence next to the one the function actually returns,
  which is exactly the mistake of drawing a screen that disagrees with the
  code it specifies. **The path is `path.to_string_lossy()`'s own —
  absolute — never the `~` this file's first draft shortened it to**: a
  reader's home directory is not `k8rs`'s to abbreviate, and the extra length
  is part of the row budget below, not a rounding error. `/home/you/` stands
  in for it the way `prod-eu` stands in for a real context name — a real run
  prints the real path.
  **It is drawn here off `open_log`'s *could not open* refusal, with an
  unwritable state directory as the cause, because that is the common way to
  actually reach this screen, not merely a relatable one.** A first-run full
  disk was drawn here once, and it was wrong twice over: `open(O_CREAT |
  O_APPEND)` only returns *No space left on device* the first time the log
  is created — every run after that opens the existing file fine and fails
  later, at a `write(2)` this file does not draw — so a full disk is the
  *rare* way in, and `Permission denied (os error 13)` on a state directory
  the operator cannot write into is the one measured off a real `0500`
  directory, 291 characters. Naming the wrong likelihood twice is worse than
  naming no likelihood at all, so this is stated as measured rather than
  guessed a second time: the four refusals stay otherwise interchangeable —
  this file only claims to know which one a reader meets *first*, not which
  is worse. The `({from})` clause — `open_log`'s own
  `Source::clause` — is drawn where the function puts it, between the path
  and the rest of the sentence, not dropped. `audit_log` fails one of four
  ways — nowhere to keep the log at all, a state directory k8rs could not
  create, something at the path that is not an ordinary file, or (drawn
  here) the `open` itself failing — and every one of the four ends with the
  same tail, `without()`'s own words, unpunctuated at the join the way the
  function actually writes it. Only the first clause changes between the
  four; the tail is one function and one string for all of them, the same
  "one string, several renderers" rule the clock-skew and namespace-scoped
  banners above already follow — the other three are read off the function,
  not reproduced live, the same caveat [the pre-TUI section
  below](#before-the-tui-ever-starts) already states for the failures it
  cites rather than measures.
- **This sentence obeys the same 13-of-16 cap as every other caveat, and it
  is always the first of them to give way**
  ([§ Your clock and a scoped namespace
  together](#your-clock-and-a-scoped-namespace-together) states the rank and
  the row arithmetic once, for every combination). Alone, as drawn above, it
  never comes close: 7 lines plus its own trailing blank is 8 rows, well
  inside the 13 the region allows, so nothing here is cut and the list keeps
  all 8 rows it would otherwise have anyway. What the rank costs it is drawn
  in the sections below, from a one-line cut beside clock skew, to a
  two-line fragment beside a protected namespace banner, to nothing at all
  beside two protected banners at once.
- **Reading still works, and the mockup says so by doing it.** The sidebar
  badges are real (`3 ● 7 ▲`, `capacity 1 ▲`), the watch is live (the header
  reads `live`, not a stale-data warning), and a real card sits below the
  banner — because D21's whole point is that a broken state directory must
  not stop somebody looking at a cluster that is on fire. This is not the
  "banner above a list" mechanism the disconnected and namespace-scoped
  screens use for a *live, ongoing* condition that can end on its own; it is
  closer to `--read-only` in shape: fixed for the life of this run, because
  fixing it means fixing the state directory and starting k8rs again, not
  waiting or reconnecting.
- **The header reads `read-only`, the same word a deliberate `--read-only`
  run already shows, and that reuse is deliberate, not a placeholder.** Both
  causes put k8rs in the identical place — no mutation is reachable — and
  the header's job is to say what is true right now, not why. What *is* this
  box's finding: the mechanism that turns "writes are dead" into that one
  word in the header is not built yet for either cause
  ([D259 ruling 5](../NOTES.md#d259--the-footer-is-a-curated-subset-with-one-pair-that-never-gives-way-the-help-screen-is-the-frame-wearing-a-title-rather-than-a-box-drawn-inside-it-and-a-gate-verified-against-a-substituted-tree-is-not-verified-2026-09-10)) —
  and whichever box builds it should read one signal true for both causes,
  not two — `--read-only`'s own header mark is the very next box, and this
  is its first reader. Nothing stops that box from adding a *second* word
  once the header mark exists, if an operator review later finds one
  genuinely wanted (a full disk is worth fixing; `--read-only` was asked
  for) — but that is a reason to widen a mechanism that does not exist yet,
  not a reason to invent a second header segment here, ahead of the box that
  owns § 1a's own zone table.
- **`s scale` and `r restart` are withheld, never marked `no`, for the same
  reason `--read-only` withholds them and not the reason a `may_i_in` refusal
  does.** `Verdict::No` is an answer about *this login's grant*; nothing
  asked that question here, and nothing about this login's RBAC changed. The
  cause is structural — invariant 2's *unreachable, not merely unbound* — so
  the two keys are simply not on the line, the same way they are not
  constructed anywhere under `--read-only`. `/ filter` and the anchor pair
  stay: neither writes anything.
- **No retry, no reconnect key, nothing to press.** Disconnected offers
  nothing to press either, but it is *trying* on its own; login-expired names
  `X switch cluster` because that is a real next step reachable from inside
  k8rs. Nothing inside k8rs fixes a full disk or a home directory it cannot
  write into — the fix is outside this program, and the footer does not
  invent a key for an action it cannot perform.

### And when the clock is off at the same time

The case the 13-of-16 cap exists for: two banners that both have something to
say, on a machine whose clock has drifted and whose state directory is full
in the same run.

```
 nodes 3/3       ctx: prod-eu · live · read-only · ⚠ your clock is behind
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ⚠ This computer and the cluster disagree     │
│  RESOURCES         │    about the time by 11 minutes (this one is  │
│   workloads        │    behind), so recent times are missing and   │
│   network          │    older ones can read smaller than they      │
│   storage          │    really are.                                │
│   config           │                                               │
│   cluster          │  k8rs could not open its audit log at         │
│  ANALYSIS          │  /home/you/.local/state/k8rs/audit.log (under │
│   capacity      1 ▲│  your home directory): Permission denied (os  │
│   certificates  30d│  error 13) — every change k8rs makes is       │
│   drain safety     │  written to that log before it is sent, so    │
│   posture          │  k8rs will not change anything until that is… │
│   restarts         │                                               │
│   waste            │  ● payments/web  ·  3 of 5 pods               │
│   versions         │    Containers exceeded their memory limit     │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

Drawn at this page's usual **70-column** width, where clock is 5 lines (6
rows with its own trailing blank) and the audit sentence is 7. That leaves 7
of the 13-row region for audit — one row short of the 8 its own 7 lines and
their blank would need, so it is capped to 6 lines — and **only one line
gives way**, the sentence's own last one, *"…still works"*. The reader loses
the reassurance and keeps everything that told them what happened and where;
the header already says `read-only`, which is the fact that sentence would
have repeated. The list keeps its full 3-row floor, and the card shows its
title, same as it would alone.

**At the real 80-column floor, neither is cut: both fit whole.** Wider lines
hold more per line, so clock wraps to 4 (5 rows) and the audit sentence to 6
(7 rows) — 12 of the 13-row region, one row to spare, nothing marked with
`…`. A reader checking this page against a live 80-column terminal sees
every word of both sentences; the cut above is what the same pair looks like
on a narrower one, drawn here because this page's mockups are.

### This sentence does not hide with the clock's

[§ While disconnected, or while the login has
expired](#while-disconnected-or-while-the-login-has-expired) withdraws the
clock pointer and its banner in both of those states, because a clock
reading is only honest off a live response's `Date` header — kept past the
last successful request, it would be exactly the guess that rule refuses. **A
ruling, stated here because the caller that has to obey it is Phase 12's and
will meet this section long after that one:** the audit sentence carries no
such staleness. `Writes::Unaudited` is a fact about this machine's state
directory, fixed for the run the moment `audit_log` returns it — it does not
go stale while the connection is down, and it does not need a live round trip
to stay true. So it is **not** withdrawn while disconnected or while the
login has expired: if the state directory could not be opened, the operator
still cannot write once the connection comes back, and a sentence that had
hidden itself in the meantime would have to reappear from nowhere with no
event to explain why. It stacks with whichever of those two banners is
showing, under the same 13-of-16 cap and the same rank as clock and audit
already demonstrate above — the namespace denial and the login-expired
explanation are both *the pane's own banner* in that rank, so both outrank
audit the same way clock does, and audit is what gives way beside either of
them, drawn next.

### And when a namespace is all you can see, too

No clock this time — just a scoped session whose audit log also could not
open, the other measured combination.

```
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  You can't list pods across the whole cluster,│
│  RESOURCES         │  so k8rs is showing the namespace your        │
│   workloads        │  kubeconfig points at: payments. Use          │
│   network          │  --namespace <name> for a different one, or   │
│   storage          │  ask for cluster-wide read access.            │
│   config           │                                               │
│   cluster          │  One node check is off: spotting a node       │
│  ANALYSIS          │  someone started emptying and did not finish  │
│   capacity         │  needs every pod in the cluster.              │
│   certificates  30d│                                               │
│   drain safety     │  k8rs could not open its audit log at         │
│   posture          │  /home/you/.local/state/k8rs/audit.log (under…│
│   restarts         │                                               │
│   waste            │  ● payments/web  ·  3 of 5 pods               │
│   versions         │    Containers exceeded their memory limit     │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

Drawn at this page's usual **70-column** width. The namespace denial is now
the pane's own banner and outranks audit, so it draws in full — all 9 lines
of it, 10 rows with its trailing blank, both paragraphs whole, *"One node
check is off"* included. That leaves only 3 of the 13-row region for audit:
2 lines fit, and the sentence is cut down to *"k8rs could not open its audit
log at / /home/you/.local/state/k8rs/audit.log (under…"* — the path itself
cut off before it finishes. This is the smallest a drawn (not omitted)
banner gets on this page at this width, and it is still the right two lines
to keep: they say a write failed and where to start looking, which is
exactly the amount the header and the footer do not already say. Read-only
is true here for the audit reason, not the namespace one — [§ You can only
see some namespaces](#you-can-only-see-some-namespaces) rules that namespace
scope alone never implies it.

**At the real 80-column floor the namespace denial needs no more of the
region than it does here — 9 lines, 10 rows, still under the cap — but each
line holds more, so audit's own share is unchanged at 4 rows and reaches one
line further into the sentence: 3 lines, cut to *"…Permission denied (os
error 13) —…"* instead of 2 cut to *"…(under…"*.** The width does not change
which sentence gives way, only how much of the losing one survives.

### And when the login has also expired

```
 nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│  RESOURCES         │  ⚠ Your login expired.                        │
│   workloads        │                                               │
│   network          │    The cluster still knows who you are, but   │
│   storage          │    the login token your kubeconfig creates    │
│   config           │    has timed out.                             │
│   cluster          │                                               │
│  ANALYSIS          │    Renew it, then press X and pick this       │
│   capacity      1 ▲│    cluster again:                             │
│   certificates  30d│                                               │
│   drain safety     │    aws sso login                              │
│   posture          │                                               │
│   restarts         │    What you see below is from 2 min ago.      │
│   waste            │                                               │
│   versions         │                                               │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   → login expired                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  X switch cluster  / filter  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

Drawn at this page's usual **70-column** width, byte for byte [§ Your login
expired](#your-login-expired)'s own mockup: at that width the login banner
is already 12 lines, 13 rows with its trailing blank — the whole region on
its own — so audit's own share is zero and it draws nothing, not a fragment,
the outcome [§ Your clock and a scoped namespace
together](#your-clock-and-a-scoped-namespace-together) rules for a share
under two rows.

**At the real 80-column floor this is not what draws.** The same banner's
sentences wrap to fewer lines on a wider terminal — 10, 11 rows with its
trailing blank — which leaves the audit sentence exactly 2 rows: one line,
cut, *"k8rs could not open its audit log at…"*, under the ANALYSIS row where
`certificates` sits above it. A reader at 80 columns sees that one line; the
absence above is what the identical screen looks like narrower, and this
page draws the narrower one. Either way, the header's `read-only` (once
wired) and the footer's withheld `s`/`r` are still there — an operator who
cannot renew their login this second is not owed the audit log's specific
complaint in the same breath as the one they have to act on right now, and
at 80 they get one line of it rather than none.

### All three at once

```
 nodes 3/3     ctx: prod-eu · ns: payments · read-only · ⚠ your clock is behind
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  ⚠ This computer and the cluster disagree about the time│
│  RESOURCES         │    by 11 minutes (this one is behind), so recent times  │
│   workloads        │    are missing and older ones can read smaller than they│
│   network          │    really are.                                          │
│   storage          │                                                         │
│   config           │  You can't list pods across the whole cluster, so k8rs  │
│   cluster          │  is showing the namespace your kubeconfig points at:    │
│  ANALYSIS          │  payments. Use --namespace <name> for a different one,  │
│   capacity         │  or ask for cluster-wide read access.                   │
│   certificates  30d│                                                         │
│   drain safety     │  One node check is off: spotting a node someone         │
│   posture          │  started emptying and did not finish needs every pod…   │
│   restarts         │                                                         │
│   waste            │  ● payments/web  ·  3 of 5 pods                         │
│   versions         │    Containers exceeded their memory limit               │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                                       │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                │
└──────────────────────────────────────────────────────────────────────────────┘
```

Byte for byte [§ Your clock and a scoped namespace
together](#your-clock-and-a-scoped-namespace-together)'s own two-banner
mockup — adding the audit sentence to that exact run changes nothing a
reader can see. Clock (4 lines here, at this wider page) and the namespace
denial (cut to 7 of 8, the same *cluster.* it already loses alone) already
spend the whole 13-row region between them, so audit's own share is zero
before it is even considered: omitted for the same reason as the
login-expired case above, drawn here to show that a third fact with no
share left costs the screen nothing further once the first two have already
exhausted it — the cap does not grow a fourth time for a third banner, and
it does not need to.

### On a healthy or a still-loading Alerts screen

**Silence is the one thing a degraded state may not do**
([§ Rules that hold across every state on this
page](#rules-that-hold-across-every-state-on-this-page)), and the two panes
that draw no list at all — Loading, and Ready with zero findings — are
exactly where this sentence had nowhere to go: `caveats` only runs ahead of a
list, and neither of those two panes has one. There the sentence joins
[`Screen::note`](../src/ui.rs)'s own paragraphs, the same seat clock skew
already has there (*"clock skew is drawn in whichever family the rest of the
screen is already in — it does not bring its own"*, [§ Nothing is broken, and
the clock is still off](#nothing-is-broken-and-the-clock-is-still-off)) —
because on a healthy cluster Alerts is empty for the whole session and the
first frame of every run is Loading, so this is not a rare corner of the
screen, it is where most runs would otherwise never hear about a dead write
path at all.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │               ○  nothing is broken            │
│  RESOURCES         │                                               │
│   workloads        │        k8rs could not open its audit log at   │
│   network          │        /home/you/.local/state/k8rs/audit.log  │
│   storage          │        (under your home directory): Permission│
│   config           │        denied (os error 13) — every change    │
│   cluster          │        k8rs makes is written to that log      │
│  ANALYSIS          │        before it is sent, so k8rs will not    │
│   capacity      1 ▲│        change anything until that is fixed,   │
│   certificates  30d│        and reading your cluster still works   │
│   drain safety     │                                               │
│   posture          │        84 pods and 3 nodes checked, none of   │
│   restarts         │        them is in trouble right now.          │
│   waste            │                                               │
│   versions         │                                               │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

**Restated positionally, because `ui.rs` cannot rank paragraphs by reading
their words: the audit sentence sits directly under the calm headline, and
every paragraph the caller supplied comes after it.** `Screen::note` is a
slice of strings a caller assembles — Phase 12's `main.rs`, per its own
doc — and the one thing `note` can do without opening one is compare it
against another string it also did not write; recognising *"Worth a look
anyway"* to drop it first would be the second copy of that sentence this
repo already refuses everywhere else. A fixed **position** costs nothing to
implement and nothing to keep in step: audit is always the paragraph right
after the headline (or, on Loading, right at the top — there is no headline
to sit under), and whatever the caller supplied is drawn in the caller's own
order after it, so **whichever of the caller's own paragraphs is last is
what gives way.**

**This is a second rule, not the banner stack's rank read onto a new pane.**
On a banner stack, audit gives way *first* — it is ranked below clock and
the pane's own reason, because it alone has a second carrier to fall back
on. Here it gives way *last*: it sits ahead of every paragraph the caller
supplied, and what shrinks is whichever of those the caller put at the end.
The two rulings do not contradict each other because they answer different
problems — the banner stack has three things with unequal claims to defend
and a real rank to put them in; the calm block has one thing `ui.rs` knows
by name (audit) and an opaque list of strings it cannot read, so *position*
is the only rule available, and position happens to protect audit here
because there is nothing else in that slot for it to lose room to. **This
does cost the count its place directly under the verdict**, and that is the
right trade, not a side effect accepted for convenience: a reader is told
*whether* they can act on what they are looking at before they are told
*how much* of the cluster was checked, the same ordering `alerts.md` already
puts consequence ahead of evidence on every card. `Worth a look anyway →
capacity` is still the paragraph dropped for room here — it is the caller's
own last paragraph, exactly where the rank now looks for one to give.

**Capped the same 13-of-16 way a banner is**, because `note` centres
whatever `Screen::note` hands it with no cut of its own today — an unbounded
value reaching this pane is the same size hazard as one reaching a banner,
and a screen that caps one path and not the other has not really capped the
value at all. Word-boundary `…`, same mark. Here it is not needed: headline
(1) + blank (1) + audit (8) + blank (1) + the count (2) is 13 of the 13 the
region allows — the whole cap, nothing to spare, but still nothing cut,
because it fits exactly rather than needing the mark to say it does not.

**Every paragraph `note` wraps on this page — this one included — is wrapped
at 39 columns, and that number is measured off this page's own drawings, not
asserted.** `84 pods and 3 nodes checked, none of` ([Nothing is
broken](#nothing-is-broken)) is 36; `Large clusters take a moment. Findings`
([Still loading](#still-loading)) is 38; this paragraph's own widest line,
`(under your home directory): Permission`, is 39 — the longest already drawn
anywhere `note` speaks. `ui::BLOCK`'s own doc names this page as its source
and gives no number of its own to check that claim against; **39 is the
number**, and a build wrapping at anything narrower would cut a line this
page already draws whole.

**Still loading draws the identical sentence first, ahead of everything the
store has read so far, for the same positional reason — there is no
headline here for it to sit under, so it sits at the top instead:**

```
 nodes …                        k8rs      ctx: prod-eu · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │        k8rs could not open its audit log at   │
│  RESOURCES         │        /home/you/.local/state/k8rs/audit.log  │
│   workloads        │        (under your home directory): Permission│
│   network          │        denied (os error 13) — every change    │
│   storage          │        k8rs makes is written to that log      │
│   config           │        before it is sent, so k8rs will not    │
│   cluster          │        change anything until that is fixed,   │
│  ANALYSIS          │        and reading your cluster still works   │
│   capacity         │                                               │
│   certificates  30d│        reading the cluster… 2,140 pods        │
│   drain safety     │                                               │
│   posture          │        Large clusters take a moment. Findings │
│   restarts         │        appear as they are found — this list…  │
│   waste            │                                               │
│   versions         │                                               │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ ? all keys  q quit                                                 │
└────────────────────────────────────────────────────────────────────┘
```

Measured false in the round before this one: *"already has room to spare"*
was not checked against this page's own numbers. Audit (8) + blank (1) +
`reading the cluster… 2,140 pods` (1) + blank (1) + the caller's own last
paragraph is 11 before that last paragraph draws a single line, leaving only
**2** of its 3 — cut to *"Large clusters take a moment. Findings / appear as
they are found — this list…"*, losing `fills up, it does not wait.` The
count line survives whole because it is one line and comes before the
paragraph that is last; the rank looks for the *last* paragraph to shrink,
never an earlier one, on this pane exactly as above it.

## Before the TUI ever starts

**No kubeconfig at all** is always this — stderr, exit non-zero, no raw mode —
there is nothing yet to list a context from. The other two below hold only
when the picker never opened: one context in the file, `--context` given,
`--once`, or a non-tty. Once two-or-more contexts put the picker on screen,
raw mode is already on, and *the certificate has expired* / *not allowed*
become the modal in
[context.md § When the new cluster does not work](context.md#when-the-new-cluster-does-not-work)
instead of a stderr message. Panicking inside a TUI corrupts the user's
terminal, so whichever form applies, the failure is handled before it can do
that.

**The certificate-has-expired message is a more specific version of the wall
it replaces, not a new kind of failure beside it.** Once a session opens, a
call that gets nothing usable back ordinarily prints [`k8s::Fault::Unanswered`](../src/k8s.rs)'s
own plain wording, once per call — measured live off a dead address: *"could
not read the server version (nothing usable came back when k8rs tried to
`get /version`) · could not list what this cluster serves, so k8rs cannot
show you what is in it or tell which add-ons it has (nothing usable came back
when k8rs tried to `get /apis`)"* — and steps aside for the certificate
sentence the moment rustls can name the reason, because a fallback string is
never printed over a typed error it could have used instead
([docs/architecture.md § Error handling](../docs/architecture.md#error-handling)).
The reasoning that earns it its own sentence, rather than folding into that
wall, is [once.md § When the certificate is why nothing came
back](once.md#when-the-certificate-is-why-nothing-came-back).

**No kubeconfig at all, one that cannot be read, and one that is not valid
YAML all reach the same sentence — and that is `Fault`'s own rule doing the
collapsing, not a lack of anything to say.** kube's own `ReadConfig` carries
the exact path it tried and the `io::Error` that stopped it
([`kubeconfig_fault`](../src/k8s.rs)); `k8s::Fault::Kubeconfig` throws both
away, the same "a `Fault` is a fact and never a sentence… it carries no
string whatever" rule every other error in this file already follows
(`k8s.rs`). Measured against `$KUBECONFIG` unset and against a file that
exists but will not parse as YAML alike, one unwrapped line, 114 columns —
past this page's frame, so quoted rather than fenced, the same treatment the
`pods_unread` header gets a few paragraphs below for the same reason:
*"k8rs: no cluster to watch — the kubeconfig itself could not be read — it
is missing, unreadable, or not valid YAML."*

**"Cannot reach the cluster" prints no message of its own on this path,
because this section is the way into the TUI and that failure cannot end
it.** Building the client from a kubeconfig never makes a network call, so a
dead address never fails at connect — the session opens, and on this path
[`pods_unread`](../src/main.rs) never runs at all: it fires only under
`--once` (`stopping`, `main.rs:2939`), which has no TUI to start in the
first place, and its own doc names it *"the one watch whose failure ends a
`--once` run"* (`:3038`). What actually shows, once the session is open, is
the same repeating `▲ k8rs is not getting <kind> from this cluster: …`
banner a session that loses its connection mid-run shows — measured live
against a dead address: five such lines, one per watch, settling there, and
nothing else. No `What k8rs asked for:`, no next step — that gap is a
product question, already in `backlog.md`, not this screen's to fix.

```
$ k8rs
k8rs: the certificate the API server presented expired 3 days ago

  Not your kubeconfig's — the API server's own, and it ran out on
  2026-08-25T00:00:00Z. That is why nothing about this cluster
  could be read this run: kubectl and anything else that connects
  to it the normal way is refused too, until someone on the
  control plane renews it — not something k8rs can do.

  If this cluster runs more than one API server behind a load
  balancer, trying again may reach one that still works.
```

**A permission refusal and a cluster that never answers share one shape**,
because both leave k8rs with no pods to build a report from — and every
finding starts there — and the reader needing the same three answers: what
k8rs asked for, what happened, and what to do next
([`pods_unread`](../src/main.rs)). The function was `pods_refused` until this
box; it is `pods_unread` now because a refusal is only one of the two faults
that reach it
([NOTES § D191](../NOTES.md#d191--the---once-review-round-three-blockers-and-the-one-pm-ruling-a-measurement-refused-2026-08-30)).
**Two blocks, not one with two fillings** — a denied role and a cluster that
never answers are different problems with different next actions, and this
page already draws one block per distinct failure rather than folding
lookalikes together (the four in this section, and the "you can only see
some namespaces" banner below it).

**Every block below leads with the same header before anything else, and it
is not redrawn at full width here** — `pods_unread` builds it with one
`format!` call and nothing in `main.rs` wraps it: *"k8rs: this cluster did
not show k8rs its pods, and every finding starts there, so there is nothing
to report."* Measured, that is 108 columns — past this page's frame and the
80-column terminal both draw — so it is named once, here, instead of
redrawn overwide in every block that follows.

```
$ k8rs
  What k8rs asked for: pods in the namespace default
  What happened: the role this kubeconfig uses needs to `list` and `watch` pods
```

Measured against a real cluster and a kubeconfig with no RBAC grants at all,
no `--namespace` given, and a context that names no namespace either — the
plain `k8rs` a beginner types first, against a cluster where nobody has
granted them anything yet. k8rs tries the whole cluster first, is refused,
then guesses the namespace `default` and tries that too — refused there as
well — which is why the scope line already reads `in the namespace default`
rather than `across the whole cluster`.

**Only the closing sentence changes with how the refusal was scoped** — the
first two lines and the shape are fixed, and the scope is always named
because that is the one fact that decides whether the reader goes and asks
for a `Role` or a `ClusterRole`:

- **No `--namespace`, and the guessed one above was refused too** (drawn
  above) — the next step is to say which namespace: *"This kubeconfig names
  no namespace, so k8rs had to guess default and was refused there too. Say
  which namespace you work in: `--namespace <name>`."*
- **`--namespace kube-system`, or a context that already names one, refused
  in that one namespace** — measured the same way, against the same
  identity, header and "What happened" unchanged from above:
  ```
  $ k8rs --namespace kube-system
    What k8rs asked for: pods in the namespace kube-system
  ```
  and the closing sentence, also one unwrapped line, this one 178 columns:
  *"Ask whoever runs this cluster for a role that may read pods in
  kube-system — the same rules as `k8rs-readonly` in the k8rs docs, granted
  in one namespace instead of all of them."* `--namespace <name>` is not
  offered here — the reader already used that door, so the next step is the
  role to ask for instead.
- **Refused cluster-wide with nothing to fall back to guessing** — `list`
  succeeds and the persistent watch's `watch` verb specifically does not, an
  easy RBAC gap to leave open by hand: *"Ask whoever runs this cluster for a
  role that may read pods in every namespace — `k8rs-readonly` in the k8rs
  docs is that role — or run k8rs in one namespace you can read:
  `--namespace <name>`."* This is the one case where offering `--namespace`
  is still useful, because the reader has not typed it yet. Read off
  [`pods_unread`](../src/main.rs) rather than measured live — the cluster
  used for this box has no `Role` shaped that way and building one is a
  write this review does not make.

A cluster that never answers reads through the same shape. Not reproduced
live in this review — that needs a listener which completes the handshake
and then blocks, and building one was outside a read-only pass over a shared
cluster — but read off [`pods_unread`](../src/main.rs) and
[`because`](../src/main.rs)'s `Unanswered` arm byte for byte, and it is the
shape [NOTES § D191](../NOTES.md#d191--the---once-review-round-three-blockers-and-the-one-pm-ruling-a-measurement-refused-2026-08-30)
names as blocker 2, an endpoint that accepts the connection and then says
nothing — the same header as above, then:

```
$ k8rs
  What k8rs asked for: pods across the whole cluster
```

"What happened" differs from the two refusals above and, like them, prints
as one unwrapped line, 84 columns: *"What happened: nothing usable came
back when k8rs tried to `list` and `watch` pods."* So does the closing
sentence, also 84: *"Check the server address this kubeconfig names, and
that this machine can reach it."*

Three things about the block this replaced were wrong, and the PM ruled each
([NOTES § D191](../NOTES.md#d191--the---once-review-round-three-blockers-and-the-one-pm-ruling-a-measurement-refused-2026-08-30)):
**there is no README** until Phase 13, so the role is named where it
actually lives — [docs/security.md § RBAC](../docs/security.md#rbac)'s
`k8rs-readonly`. **`--namespace <name>` is a spent door** for a reader who
already typed it, so the next step differs by scope instead of repeating one
line for everyone. **The scope is always in the sentence** now, whole
cluster or the exact namespace, rather than missing entirely under
`--namespace kube-system` the way it used to. **The old block's `Your
kubeconfig context: prod-eu, user: dev@example.com` line is gone and not
replaced**: `pods_unread` never had that string to print — this stage runs
before the header exists to read it from — and a line the binary cannot
produce is a promise the screen cannot keep
([NOTES § D190](../NOTES.md#d190--the-screen-that-ships-first-promises-four-things-the-binary-does-not-do-and-nobody-had-read-them-against-each-other-2026-08-30)).

## Rules that hold across every state on this page

- The header always tells the truth about **context · scope · connection ·
  read-only**, and says so when the kubeconfig disables TLS verification.
- **`read-only` and a namespace scope are two different facts, and the header
  must not let one stand for the other.** A session scoped to one namespace —
  by `--namespace` or by a 403 on the cluster-wide list — may still scale and
  restart inside it; `read-only` is earned separately, by the connection, the
  audit log, or the flag, never by which namespace a session happens to see
  ([§ You can only see some namespaces](#you-can-only-see-some-namespaces)).
- **A vital in the header is blank rather than guessed, and stale rather than
  hidden.** `nodes 3/3` becomes `nodes …` while connecting, `nodes 3/3
  (40s ago)` when the stream is gone, and nothing at all for a user who cannot
  list nodes. The `capacity` badge follows the same rule
  ([widgets.md § The header row](widgets.md#1a-the-header-row)).
- No state is a dead end: each one names the next thing to try.
- No jargon without its explanation, including in the stderr messages — the
  first thing a newcomer ever sees from k8rs is one of these.
- A 403 degrades exactly the feature that needed the permission and names the
  missing verb and resource. It never crashes and never retries in a loop.
- **A laptop clock more than five minutes off the cluster's says so, in both
  directions, and stays silent below that line and while disconnected.**
  [§ Your computer's clock is off](#your-computers-clock-is-off).
- **A check that could not run says so, on the screen where its findings would
  have appeared.** Silence is the one thing it may not do: an alert list with a
  disabled rule behind it looks identical to an alert list that found nothing,
  and the second is the claim the whole product rests on.
- **A write that cannot be audited is a write that cannot happen, and k8rs
  says so and reads on rather than exiting.**
  [§ The audit log could not be opened](#the-audit-log-could-not-be-opened),
  [NOTES § D21](../NOTES.md#d21--if-the-write-cannot-be-audited-the-write-does-not-happen).
  The two keys this costs are withheld the way `--read-only` withholds them —
  never marked `no`, which is reserved for a permission this login lacks. It
  is not withdrawn while disconnected or while the login has expired, unlike
  the clock pointer — it is a fact about this machine, not a reading that can
  go stale ([§ This sentence does not hide with the
  clock's](#this-sentence-does-not-hide-with-the-clocks)).
- **Whatever queues above the pane's own content — the clock pointer, the
  pane's own reason, the audit sentence — shares one 16-row body and never
  takes more than 13 of it.** The list or the calm block beneath keeps the
  rest. Which one gives way is a **rank**, not simply whichever is drawn
  last: the audit sentence is always first to give, because it is the one
  fact with a second carrier (the footer, and — once wired — the header);
  clock and the pane's own reason (which namespace, what fixes a dead
  login) have none, so between those two the draw order and the rank agree.
  Whatever is last in the rank is wrapped as far as its own remaining budget
  allows and marked with a visible `…` at a word boundary; a share under two
  rows draws nothing at all rather than an unmarked fragment
  ([§ Your clock and a scoped namespace
  together](#your-clock-and-a-scoped-namespace-together)).
- **A left-flush banner that opens `⚠ ` hangs its wrap under the text, not
  under the mark, and this is the banner's rule, not an accident of how this
  page happens to be typeset.** The mark and the space after it are spent
  once, on the first line; every line after it — whether it continues the
  same sentence or starts the next one in the same paragraph — begins at the
  column the text itself started on, four in from the pane's own left edge,
  the same as the first line's own indent (`  ⚠ ` and `    ` are both four
  characters). [§ The connection dropped](#the-connection-dropped) and
  [§ Your login expired](#your-login-expired) both draw it this way for
  three sentences at once, not one wrapped across lines, which is why it is
  a rule about the *paragraph* and not merely about word-wrap: a renderer
  that left-aligned every line under the mark instead would still wrap
  correctly and would still be wrong, because the ⚠ would then read as
  repeating once per sentence rather than opening the paragraph once. The
  centred calm-block family carries no mark at all and this rule has nothing
  to align there — [§ Nothing is broken, and the clock is still
  off](#nothing-is-broken-and-the-clock-is-still-off) already says why.
- **`ui::note`'s block wraps at 39 columns** — measured off this page's own
  widest already-drawn line in it, [§ The audit log could not be
  opened](#the-audit-log-could-not-be-opened)'s own `note`-path mockup; see
  that section for the three lines the number is read off rather than
  guessed from.
- **`X switch cluster` is promoted onto the footer wherever the login has
  expired, on Alerts and on the browser, whatever the pane beneath it is
  drawing** — [§ Over a pane with nothing to show
  yet](#over-a-pane-with-nothing-to-show-yet) draws the two cases that are
  not already shown with a card selected: a still-loading pane, and an
  empty kind. Neither pane's own content is rewritten for it; only the
  footer gains the key, the same one line every time. A detail tab keeps its
  own closed footer regardless — [widgets.md § 2a](widgets.md#2a-the-footer)
  already draws it with no `X`.
