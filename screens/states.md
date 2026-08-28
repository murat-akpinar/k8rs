# Screens — First launch, empty, and everything going wrong

The states that decide whether a newcomer keeps the tool. All of them were
undefined until they were written down, and most of them happen on **first
launch**.

## Nothing is broken

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │                                               │
│   workloads        │               ○  nothing is broken            │
│   network          │                                               │
│   storage          │        84 pods and 3 nodes checked, none of   │
│   config           │        them is in trouble right now.          │
│   cluster          │                                               │
│ ANALYSIS           │        Worth a look anyway:                   │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

An empty list is a failure state in most tools; here it is the goal, so it is
drawn as an answer and points at the report that still has something to say.
This screen is only honest because Alerts holds nothing but *broken right
now* — a lint report would never be empty.

## Still loading

```
 nodes …                        k8rs      ctx: prod-eu · connecting…
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │        reading the cluster… 2,140 pods        │
│   workloads        │                                               │
│   network          │        Large clusters take a moment. Findings │
│   storage          │        appear as they are found — this list   │
│   config           │        fills up, it does not wait.            │
│   cluster          │                                               │
│ ANALYSIS           │                                               │
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
│ q quit                                                             │
└────────────────────────────────────────────────────────────────────┘
```

## The connection dropped

The header is the honest one. Stale data drawn as if it were live is
forbidden.

```
 nodes 3/3 (40s ago)          ctx: prod-eu · ⚠ disconnected, retrying
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ⚠ Not connected to the cluster right now.    │
│   workloads        │    What you see below is from 40 seconds ago. │
│   network          │    Retrying…                                  │
│   storage          │                                               │
│   config           │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   cluster          │    Containers exceeded their memory limit     │
│ ANALYSIS           │                                               │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

## Your login expired

Not a 403, and not a dropped connection — the third case. On EKS, GKE and AKS
the kubeconfig mints a short-lived token from a credential plugin, and it runs
out mid-session ([NOTES § D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)).

```
 nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ⚠ Your login expired.                        │
│   workloads        │                                               │
│   network          │    The cluster still knows who you are, but   │
│   storage          │    the login token your kubeconfig creates    │
│   config           │    has timed out.                             │
│   cluster          │                                               │
│ ANALYSIS           │    Renew it, then press X and pick this       │
│   capacity      1 ▲│    cluster again:                             │
│   certificates  30d│                                               │
│   drain safety     │      aws sso login                            │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │    What you see below is from 2 min ago.      │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   → login expired                    │
├────────────────────────────────────────────────────────────────────┤
│ X switch cluster   ? all keys   q quit                             │
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
- Stale data stays visible and stays labelled, exactly as on the disconnected
  screen. k8rs does not clear the screen because it lost its token.

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
strings](once.md#when-your-clock-and-the-clusters-disagree), re-wrapped, the
same rule every shared string in this product already follows.

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
│ RESOURCES          │    about the time by 11 minutes (this one is  │
│   workloads        │    behind), so recent times are missing and   │
│   network          │    older ones can read smaller than they      │
│   storage          │    really are.                                │
│   config           │                                               │
│   cluster          │  ● payments/web  ·  3 of 5 pods               │
│ ANALYSIS           │    Containers exceeded their memory limit     │
│   capacity      1 ▲│                                               │
│   certificates  30d│  ▲ shop/api  ·  2 of 6 pods  ·  1 min ago     │
│   drain safety     │    Running, but not receiving traffic — the   │
│   posture          │    readiness check is failing                 │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

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
│ RESOURCES          │    about the time by 9 minutes (this one is   │
│   workloads        │    ahead), so times can read larger than they │
│   network          │    really are.                                │
│   storage          │                                               │
│   config           │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   cluster          │    Containers exceeded their memory limit     │
│ ANALYSIS           │    and were killed by the kernel (OOMKilled)  │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
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
│ RESOURCES          │               ○  nothing is broken            │
│   workloads        │                                               │
│   network          │        84 pods and 3 nodes checked, none of   │
│   storage          │        them is in trouble right now.          │
│   config           │                                               │
│   cluster          │        This computer and the cluster disagree │
│ ANALYSIS           │        about the time by 11 minutes (this one │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
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
│ RESOURCES          │    by 11 minutes (this one is behind), so recent times  │
│   workloads        │    are missing and older ones can read smaller than they│
│   network          │    really are.                                          │
│   storage          │                                                         │
│   config           │  You can't list pods across the whole cluster, so k8rs  │
│   cluster          │  is showing the namespace your kubeconfig points at:    │
│ ANALYSIS           │  payments. Use --namespace <name> for a different one,  │
│   capacity         │  or ask for cluster-wide read access.                   │
│   certificates  30d│                                                         │
│   drain safety     │  One node check is off: spotting a node someone started │
│   posture          │  emptying and did not finish needs every pod in the     │
│   restarts         │  cluster.                                               │
│   waste            │                                                         │
│   versions         │  ● payments/web  ·  3 of 5 pods                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                                       │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

This is the one mockup on this page drawn at the real 80-column floor rather
than the file's usual 70-column page width — `ns: payments · read-only ·
⚠ your clock is behind` alone is 44 characters, and at 70 columns total the
header has nowhere to put it beside `nodes 3/3` and `ctx: prod-eu`. At 80 it
fits with 6 columns of gap to spare; a context name one character longer
than `prod-eu`, or a fourth badge (a TLS warning), is what the sacrifice
order in the section above exists for — the clock pointer is the first of
the three to go, and the banner underneath still carries the full sentence
on its own. The longer, two-effect sentence costs this mockup the card's evidence
line: it is trimmed to its title, same rule as every other truncated card
here — the full text is one `⏎` away.

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
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  You can't list pods across the whole         │
│ RESOURCES          │  cluster, so k8rs is showing the namespace    │
│   workloads        │  your kubeconfig points at: payments.         │
│   network          │  Use  --namespace <name>  for a different     │
│   storage          │  one, or ask for cluster-wide read access.    │
│   config           │                                               │
│   cluster          │  One node check is off: spotting a node       │
│ ANALYSIS           │  someone started emptying and did not finish  │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

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
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │               ○  nothing is broken            │
│   workloads        │                                               │
│   network          │        12 pods in payments and 3 nodes        │
│   storage          │        checked, none of them is in trouble    │
│   config           │        right now.                             │
│   cluster          │                                               │
│ ANALYSIS           │        One node check is off: spotting a      │
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
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
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
  re-wrapped for a narrower block. One string, three renderers — the third is
  `--once` ([once.md](once.md#when-a-check-could-not-run)).

## Before the TUI ever starts

**No kubeconfig at all** is always this — stderr, exit non-zero, no raw mode —
there is nothing yet to list a context from. The other three below hold only
when the picker never opened: one context in the file, `--context` given,
`--once`, or a non-tty. Once two-or-more contexts put the picker on screen,
raw mode is already on, and *cannot reach the cluster* / *the certificate
has expired* / *not allowed* become the modal in
[context.md § When the new cluster does not work](context.md#when-the-new-cluster-does-not-work)
instead of a stderr message. Panicking inside a TUI corrupts the user's
terminal, so whichever form applies, the failure is handled before it can do
that.

**The certificate-has-expired message is a more specific *cannot reach the
cluster*, not a fourth kind of failure.** Both mean the connection did not
complete; the generic wording above prints only when k8rs genuinely does not
know why, and steps aside for this one the moment rustls names the reason —
a fallback string is never printed over a typed error it could have used
instead ([docs/architecture.md § Error handling](../docs/architecture.md#error-handling)).
The reasoning that earns it its own sentence, rather than folding into the
generic one, is [once.md § When the certificate is why nothing came
back](once.md#when-the-certificate-is-why-nothing-came-back).

```
$ k8rs
k8rs: no kubeconfig found.

  Looked in: $KUBECONFIG (unset), ~/.kube/config

  k8rs uses the same file kubectl does. If kubectl works on this
  machine, k8rs will too.
```

```
$ k8rs
k8rs: cannot reach the cluster at https://10.0.0.1:6443

  The address is in your kubeconfig, but nothing answered.
  Is the cluster running? Are you on the right VPN?
```

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

```
$ k8rs
k8rs: your user is not allowed to list pods in this cluster.

  Missing permission: list pods (cluster-wide)
  Your kubeconfig context: prod-eu, user: dev@example.com

  Ask for one of the two roles in the README, or run k8rs
  against a single namespace:  k8rs --namespace <name>
```

## Rules that hold across every state on this page

- The header always tells the truth about **context · scope · connection ·
  read-only**, and says so when the kubeconfig disables TLS verification.
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
