# Screens — Confirmations and failures

No mutation happens without one of these. The consequence is stated in plain
language **above** the command, never instead of it.

## Scale — confirm with dry-run

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Scale payments/web ──────────────────────────────────────┐    │
│    │                                                          │    │
│    │  This starts 1 more copy of your app.                    │    │
│    │  Right now: 2 copies.  After: 3 copies.                  │    │
│    │                                                          │    │
│    │  The cluster checked it first and accepted it.           │    │
│    │                                                          │    │
│    │  $ kubectl scale deployment/web --replicas=3 -n payments │    │
│    │                                                          │    │
│    │              [ ⏎ do it ]    [ esc cancel ]               │    │
│    └──────────────────────────────────────────────────────────┘    │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl scale deployment/web --replicas=3 -n payments            │
├────────────────────────────────────────────────────────────────────┤
│ ⏎ do it   esc cancel                                               │
└────────────────────────────────────────────────────────────────────┘
```

The two consequence lines are the only part of the box that changes with the
relation between what is running now and what was asked for — title,
dry-run line and `$` line keep the same shape every time; only the numbers
and the verb move. "Copies", never "replicas" (rule 2), and the count is
always both sides of it so nothing depends on the reader remembering the old
number. The box draws these as two lines; underneath they are one
`consequence` string, not two fields — see § *Printed instead of drawn*
below for why that distinction matters:

- **Up, by one** (2 → 3, the box above) — "This starts 1 more copy of your
  app." / "Right now: 2 copies. After: 3 copies."
- **Up, by more than one** (2 → 5) — "This starts 3 more copies of your
  app." / "Right now: 2 copies. After: 5 copies."
- **Down** (3 → 2) — "This stops 1 copy of your app." / "Right now: 3
  copies. After: 2 copies."
- **Down, to zero** — the one that stops the app, not just some copies of
  it (3 → 0) — "This stops all 3 copies of your app — nothing will be left
  running." / "Right now: 3 copies. After: 0 copies."
- **Down, to zero, from one** — "all 1 copy" is not a sentence anybody says,
  so the only-copy case gets its own wording rather than the rule above (1
  → 0) — "This stops the only copy of your app — nothing will be left
  running." / "Right now: 1 copy. After: 0 copies."
- **Unchanged** — the asked-for count is already what's running (3 → 3) —
  "This asks for the count web is already running." / "Right now: 3 copies.
  After: 3 copies."

Down to zero gets no stricter a guard than any other scale — dry-run, then
`⏎ do it`, same as the box above. Invariant 2 only raises the bar to typing
the name for delete and drain; a scale that empties a Deployment is still a
scale, and the plain-language line above carries the warning instead of a
new confirmation kind. The unchanged case is not a special state either: the
dry-run still runs and still succeeds — a `PATCH` asking for the count
already running is a legal no-op, not a rejected write — so the button goes
live the same way it does for every other relation (rule 3).

**The `$` line reads `deployment/web`, not `deploy/web`.** There is exactly
one `kubectl` string behind both this line and the command-log bar under it
(`ops::Mutation::kubectl`, read again by `ops::Shown` — `src/ops.rs` § THE
MUTATION CONTRACT), so the two can never spell the object two different
ways — whichever ships here is what ships on the log bar too. Full word wins:
every other `$ kubectl …` line on this page and every other screen already
spells the kind out — `kubectl delete pod`, `kubectl drain node-3`, `kubectl
describe pod`, `kubectl get deployments` — and
[NOTES § The three views](../NOTES.md#the-three-views)'s own worked example
of this exact panel already reads
`kubectl scale deployment/web --replicas=3`, not the short form. `deploy` is
real kubectl shorthand, but this line's whole job is teaching a newcomer real
kubectl by letting them read it scroll by (invariant 14) — an abbreviation is
one more piece of jargon they have not met yet, and it buys nothing
invariant 4 needs: `deployment/web` is exactly as real and executable a
command as `deploy/web`. The same choice carries to every kind scale reaches
([NOTES § Operations](../NOTES.md#operations--the-full-admin-surface), the
`s` row) — `statefulset/web`, `replicaset/web`, never `sts/` or `rs/`. This
does not touch the title bar:
`payments/web` there is namespace/name (rule 1), a different sentence
answering a different question, and it was never the one that disagreed.

### Printed instead of drawn — scale on the headless surface

The box above draws the consequence as two lines, but there is exactly one
`consequence: String` on `ops::Mutation` — no second field for the count
sentence to live in. Every `Mutation` reaches `ops::Record` through
`k8s::text` (`src/k8s.rs:284`), which replaces a line break with a single
space rather than keeping it, so a `\n` inside `consequence` cannot survive
into the record or onto this headless surface. The box's two lines are that
one string *wrapped to the box width* — a rendering choice, not two fields —
so the string itself is always the plain-language sentence and the count
sentence joined by a space. That join is the rule for every relation in the
list above, not a special case of the zero one; an implementer who reads the
box as two strings will reach for a `\n` that the ingest guard removes.

The headless `ops scale` prints the same three lines with the box removed —
object and namespace, the consequence, the `$` line, in that order — nothing
here is reworded for the terminal (`src/main.rs`'s `show`). Flag syntax for
the scale verb is this box's dev-core half to name, not this file's, but
whatever the invocation looks like, a scale to zero prints exactly this on
**stderr**, not stdout, before the confirmation prompt — the same wording the
dialog would have shown, taken straight from the relation list above, joined.
A reader who knows `--once` will expect the opposite, because there stdout
*is* the piped answer — but
[once.md § stdout and stderr are split on purpose](once.md#stdout-and-stderr-are-split-on-purpose)'s
split is *stdout is the findings, stderr is everything else*, and a scale
produces no finding, only a change. So `k8rs ops scale … > out` writes an
*empty* `out`; the three lines below still reach the terminal, on stderr.

The middle line is that one string from the paragraph above, no `\n` in it,
and it is 105 columns — wider than the 80-column terminal k8rs supports. A
real terminal does not reflow at word boundaries; it wraps wherever the
column count runs out, so this is what actually reaches the screen:

```
deployment/web in payments
This stops all 3 copies of your app — nothing will be left running. Right now: 3
 copies. After: 0 copies.
$ kubectl scale deployment/web --replicas=0 -n payments
```

That break is the terminal's doing, not k8rs's — a wider terminal draws it
somewhere else, or not at all, and nothing about what was sent changes either
way.

## Restart — confirm with dry-run

`r` on a deployment, a statefulset or a daemonset opens this. Unlike scale,
what happens next is not a number the reader picks — it is a sentence about
how the object's own settings pace the replacement, because that pacing is a
setting on the object and k8rs deliberately reads none of it
([NOTES § D223](../NOTES.md#d223--the-four-rulings-restart-could-not-be-briefed-without-and-the-pod-arm-that-is-deletes-2026-09-04)
ruling 3,
[D224](../NOTES.md#d224--the-restart-review-round-two-blockers-a-stand-in-apiserver-could-not-produce-and-the-sentence-that-promised-a-clusters-settings-2026-09-04)):
a DaemonSet with `maxUnavailable: 3` took every node down at once on a real
cluster, a `partition`ed StatefulSet left copies on the old template
indefinitely, and `OnDelete` moved nothing on either kind. The three sentences
below are worded around that — *asks* is the load-bearing word in every one of
them — and they are fixed text, not this file's to reword (D224).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│  ┌ Restart payments/web ───────────────────────────────────────┐   │
│  │                                                             │   │
│  │  This asks Kubernetes to replace every copy of your app with│   │
│  │  a new one. How many stop at the same time is a setting on  │   │
│  │  this deployment — it can be a few, or all of them at once. │   │
│  │  A paused deployment will not start until you resume it.    │   │
│  │  The cluster checked it first and accepted it.              │   │
│  │                                                             │   │
│  │  $ kubectl rollout restart deployment/web -n payments       │   │
│  │                                                             │   │
│  │                [ ⏎ do it ]    [ esc cancel ]                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl rollout restart deployment/web -n payments               │
├────────────────────────────────────────────────────────────────────┤
│ ⏎ do it   esc cancel                                               │
└────────────────────────────────────────────────────────────────────┘
```

**This box is wider than scale's, and shorter on blank lines, and both are
the same decision.** Scale's nested box is 58 columns of interior with a
4-column margin on each side; none of the three consequence sentences here
fit that box without either dropping a clause D224 put there on purpose or
running past 24 rows once the paused warning is added below. So the margin
narrows to 2/3 and the interior widens to 61, which is as far as it can go
without the nested box touching the outer frame — and the blank line that
would ordinarily sit between the consequence and the dry-run verdict (the
one scale keeps) is gone, because the paused variant below needs that row
and the two states of one dialog should not be shaped differently. The
consequence is still one string, wrapped to the box width exactly as
scale's is (§ *Printed instead of drawn* below) — the wrap points shown are
this box's choice of where to break for readability, not a second field.

The statefulset and daemonset consequences are the same shape, wrapped the
same way, in a box that is otherwise identical but for the title and the `$`
line:

- **statefulset** — "This asks Kubernetes to replace every copy of your app
  with a new one, working down from the highest-numbered copy. How many stop
  at the same time, how far down it goes, and whether it waits for you to
  delete a copy yourself are all settings on this statefulset."
  `$ kubectl rollout restart statefulset/web -n payments`
- **daemonset** — "This asks Kubernetes to replace the copy of your app on
  each node it runs on. How many nodes it takes at a time, and whether it
  waits for you to delete a copy yourself, are settings on this daemonset."
  `$ kubectl rollout restart daemonset/web -n payments`

**The dry-run verdict is real even though the taught command has no
`--dry-run` flag.** `kubectl rollout restart` cannot check itself; the
`PATCH` k8rs sends under it can, on an ordinary path, so `The cluster
checked it first and accepted it.` is true of k8rs's own call and never
claimed of the command on the `$` line
([NOTES § D223](../NOTES.md#d223--the-four-rulings-restart-could-not-be-briefed-without-and-the-pod-arm-that-is-deletes-2026-09-04)
ruling 4).

### The paused Deployment

A Deployment can be paused, and on a real cluster the patch above still
succeeds on one — nothing about the request is invalid. `kubectl rollout
restart` on the same object exits `1` and refuses outright. Without a line
here, the dialog would say *the change was made* on a Deployment that had
not moved, over a command that would have told the operator so itself
(D224, measured against a real apiserver).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│  ┌ Restart payments/web ───────────────────────────────────────┐   │
│  │                                                             │   │
│  │  This asks Kubernetes to replace every copy of your app with│   │
│  │  a new one. How many stop at the same time is a setting on  │   │
│  │  this deployment — it can be a few, or all of them at once. │   │
│  │  A paused deployment will not start until you resume it.    │   │
│  │  This deployment is paused, so nothing will be replaced     │   │
│  │  until somebody resumes it with kubectl rollout resume — and│   │
│  │  the command above will refuse to run until then.           │   │
│  │  The cluster checked it first and accepted it.              │   │
│  │                                                             │   │
│  │  $ kubectl rollout restart deployment/web -n payments       │   │
│  │                                                             │   │
│  │                [ ⏎ do it ]    [ esc cancel ]                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl rollout restart deployment/web -n payments               │
├────────────────────────────────────────────────────────────────────┤
│ ⏎ do it   esc cancel                                               │
└────────────────────────────────────────────────────────────────────┘
```

**This is a warning, not a refusal — the button stays live and `⏎` still
does it.** Writing the annotation on a paused Deployment is not destructive;
it takes effect the moment somebody runs `kubectl rollout resume`. What was
wrong, before this line existed, was not the write — it was the dialog
claiming the copies had already been replaced when they had not. Only a
Deployment carries this line: a StatefulSet and a DaemonSet have no
`spec.paused`, so their dialogs never grow it (D224).

### Refused, not opened — pod and replicaset

`r` on a pod or a replicaset does not open a dialog at all — k8rs says why in
words and stops there, nothing sent, nothing to confirm. This is rule 4
below, ahead of the dialog it describes: restarting a pod really is deleting
it, and that delete does not exist yet as a reachable operation
([NOTES § D223](../NOTES.md#d223--the-four-rulings-restart-could-not-be-briefed-without-and-the-pod-arm-that-is-deletes-2026-09-04)
ruling 1) — once it does, `r` on a pod opens *that* dialog with this same
sentence as its consequence, and this refusal retires. Today, on the
headless surface, both refusals read like this:

```
$ k8rs ops restart pod/web-7d9f4-x8k2p -n payments
k8rs: k8rs will not restart a pod: restarting a pod means deleting it and lettin
g the thing that created it start a replacement. k8rs restarts a deployment, a s
tatefulset and a daemonset — if this pod belongs to one, restart that instead
```

```
$ k8rs ops restart replicaset/web-7d9f4 -n payments
k8rs: k8rs cannot restart a replicaset: a replicaset is normally made by a deplo
yment, and restarting that deployment is what replaces its copies. k8rs restarts
 a deployment, a statefulset and a daemonset
```

Every other kind — a Service, a ConfigMap, anything restart was never meant
for — gets the same shape of answer, naming the kind that was actually
asked for: `k8rs: k8rs cannot restart a service — restarting replaces the
copies an object is running, and k8rs does that for a deployment, a
statefulset and a daemonset`. The doubled `k8rs: k8rs …` opening is not a
typo; every headless refusal reads that way on this build (compare
`screens/context.md`'s own refusals).

### The unhappy paths this page already draws

Restart shares every one of them with scale, unchanged except for the `$`
line: **§ The cluster said no** for a rejected dry-run, **§ The object went
away while the dialog was open** for a Deployment deleted out from under the
open dialog, **§ While the call is running** for the three lines that change
while the `PATCH` is in flight. None of those sections are redrawn here —
swap `kubectl scale deployment/web --replicas=3` for `kubectl rollout
restart deployment/web -n payments` and they read exactly the same.

**What restart does not have is a typed name.** Invariant 2 only raises the
bar to typing the name for delete and drain — the fact § Scale's own
down-to-zero paragraph already states — and a restart is neither, so its
box ends in the same two buttons as scale's — `[ ⏎ do it ]  [ esc cancel ]`
— never a name field. Cancelling is `esc`, the same as everywhere else on
this page.

### Printed instead of drawn — restart on the headless surface

`ops restart` is headless today; it is drawn at Phase 11. Real output,
`echo yes | k8rs ops restart deploy/web -n payments`, against a paused
Deployment:

```
$ echo yes | k8rs ops restart deploy/web -n payments
deployment/web in payments
This asks Kubernetes to replace every copy of your app with a new one. How many 
stop at the same time is a setting on this deployment — it can be a few, or all 
of them at once. A paused deployment will not start until you resume it.
$ kubectl rollout restart deployment/web -n payments
This deployment is paused, so nothing will be replaced until somebody resumes it
 with kubectl rollout resume — and the command above will refuse to run until th
en.
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
```

The consequence and the paused warning are each one unwrapped line at the
source — 232 and 163 columns — and each is split above wherever the 80th,
160th and so on column runs out, not at a word boundary, the same raw wrap
§ Scale's own long line gets. **The order here is not the drawn box's
order.** `show` prints the object, the consequence and the `$` line before
the check is ever sent; the paused warning and the dry-run verdict both come
from what the check answered, so they print after the `$` line, not before
it as the box draws them. Nothing is reworded for either surface — the
box narrates the same three facts as the terminal, in the order that reads
best for each.

## Delete — the name has to be typed

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Delete payments/web-7d9f4 ───────────────────────────┐        │
│    │                                                      │        │
│    │  This removes the pod. Its Deployment will start a   │        │
│    │  replacement immediately — the app keeps running.    │        │
│    │                                                      │        │
│    │  Type the pod's name to confirm:                     │        │
│    │  ┌────────────────────────────────────────────────┐  │        │
│    │  │ web-7d9f_                                      │  │        │
│    │  └────────────────────────────────────────────────┘  │        │
│    │                                                      │        │
│    │            [ delete ]     [ esc cancel ]             │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ type the name to enable   esc cancel                               │
└────────────────────────────────────────────────────────────────────┘
```

The button stays disabled until the typed name matches. This is the
ctrl-key-slip guard, and it is why deletes and drains are the two operations
that require typing.

## The cluster said no

A rejected write is a first-class state, not a toast that vanishes.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ The cluster refused this ────────────────────────────┐        │
│    │                                                      │        │
│    │  Nothing was changed.                                │        │
│    │                                                      │        │
│    │  The cluster's own words:                            │        │
│    │    admission webhook 'limits.example.com' denied     │        │
│    │    the request: replicas may not exceed 5 in this    │        │
│    │    namespace                                         │        │
│    │                                                      │        │
│    │  This is the check that runs before the real change  │        │
│    │  — it stopped this one.                              │        │
│    │                                                      │        │
│    │                     [ esc dismiss ]                  │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl scale deployment/web --replicas=9 -n payments  → rejected│
├────────────────────────────────────────────────────────────────────┤
│ esc dismiss   ⏎ open                                               │
└────────────────────────────────────────────────────────────────────┘
```

## The object went away while the dialog was open

The watch never stopped running behind the modal, so a dialog knows when the
thing it is about stopped existing. This is the pod you selected being replaced
by its ReplicaSet while you were typing its name
([NOTES § D22](../NOTES.md#d22--a-confirmation-can-outlive-the-thing-it-confirms)).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Already gone ────────────────────────────────────────┐        │
│    │                                                      │        │
│    │  This pod is already gone — something else removed   │        │
│    │  it while this was open.                             │        │
│    │                                                      │        │
│    │    payments/web-7d9f4                                │        │
│    │    replaced by web-2c81a 3 seconds ago               │        │
│    │                                                      │        │
│    │  Nothing was changed.                                │        │
│    │                                                      │        │
│    │                  [ esc dismiss ]                     │        │
│    │                                                      │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl delete pod web-7d9f4 -n payments   → not sent            │
├────────────────────────────────────────────────────────────────────┤
│ esc dismiss                                                        │
└────────────────────────────────────────────────────────────────────┘
```

The dialog holds the object's `uid` and the `resourceVersion` it opened with,
which gives two outcomes and not one:

- **Gone** — the screen above. The confirm button dies. Sending a delete by
  name at this point would hit whatever now holds that name, which is how the
  wrong pod gets deleted.
- **Changed** — the dialog says the object changed underneath and offers a
  re-read. The same mechanic as a 409, moved to where it costs the user
  nothing.

## While the call is running

The modal closes when you confirm, not when the cluster finishes. Three lines
change and the rest of the screen keeps working
([NOTES § D20](../NOTES.md#d20--a-call-that-takes-time-is-a-state-and-there-was-none)):

```
header   ctx: prod-eu · live · admin · changing…
log      $ kubectl scale deployment/web --replicas=3 -n payments   …
footer   ↑↓ move  ⏎ open  ?  ·  finishing the change to payments/web first
```

Navigation stays free. A **second mutation**, a **cluster switch** (`X`) and
**`q`** are refused until the call returns — quitting mid-`PATCH` would leave
the audit log holding an attempt with no result. The `…` on the command line is
replaced by the outcome, never removed.

## Drain, which takes minutes

The one operation long enough to need a screen of its own rather than an
indicator. Counts, not a spinner — a changing number is information
([v0.2](../NOTES.md#operations--the-full-admin-surface)).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Draining node-3 ─────────────────────────────────────┐        │
│    │                                                      │        │
│    │  Moving pods off node-3 so it can be                 │        │
│    │  worked on. The node stops accepting new ones.       │        │
│    │                                                      │        │
│    │    moved      4 of 11                                │        │
│    │    waiting    5                                      │        │
│    │    blocked    2  — PodDisruptionBudget won't allow   │        │
│    │                    fewer than 3 copies of shop/api   │        │
│    │                                                      │        │
│    │  Blocked pods stay put. Nothing is forced.           │        │
│    │                                                      │        │
│    │            [ esc stop — keeps what moved ]           │        │
│    │                                                      │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl drain node-3 --ignore-daemonsets   …                     │
├────────────────────────────────────────────────────────────────────┤
│ esc stop draining                                                  │
└────────────────────────────────────────────────────────────────────┘
```

- **`4 of 11` counts the pods this drain will move, and nothing else.** Not
  every pod on the node: `kubectl drain` never evicts a DaemonSet pod or a
  static pod whatever flags it is given, so a total that counted them would
  stall at `9 of 11` forever on a drain that had in fact finished. It is the
  same count, computed the same way, as the one on N2's cordon card
  ([alerts.md § every count](alerts.md#every-count-this-card-can-have) ·
  [NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  Two screens showing the same node must not disagree about how many pods are
  on it.
- **Blockers are named while they block**, not reported after a hang. This is
  the whole reason drain is in the tool
  ([REQUIREMENTS](../REQUIREMENTS.md#write-operations-new--the-reversal)).
- `esc` stops draining and keeps what already moved. It does not undo — nothing
  in k8rs pretends an eviction can be taken back.
- `--force` does not exist here. Evicting a pod nothing will recreate is a
  decision a beginner cannot evaluate from this screen.

## Rules for every dialog on this page

1. The **object identity** is in the title bar — `payments/web` for something
   in a namespace, the bare `node-3` for something that belongs to the whole
   cluster ([README § the five rules](README.md#the-five-rules-every-screen-obeys)).
   A stale selection can never be confirmed blindly.
2. The consequence is plain language and counts things the user can picture
   ("1 more copy"), not API vocabulary.
3. The dry-run verdict is shown *before* the button is live, wherever the API
   supports dry-run. A rejected dry-run never proceeds.
4. Restarting a bare pod is a **delete** and the dialog says so — nobody
   learns "restart" as a synonym for "delete" by accident here.
5. Success, failure and cancellation all reach the audit log. A trail that
   records only what worked cannot answer "what did they try".
6. Under `--read-only` none of this is reachable: the keys are unbound and the
   code path does not exist.
