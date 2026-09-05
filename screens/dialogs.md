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
spells the kind out — `kubectl delete pod/web-7d9f4`, `kubectl drain
node-3`, `kubectl describe pod`, `kubectl get deployments` — and
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
Run `k8rs ops` on its own to see everything it can do.
```

```
$ k8rs ops restart replicaset/web-7d9f4 -n payments
k8rs: k8rs cannot restart a replicaset: a replicaset is normally made by a deplo
yment, and restarting that deployment is what replaces its copies. k8rs restarts
 a deployment, a statefulset and a daemonset
Run `k8rs ops` on its own to see everything it can do.
```

Among the six kinds an operation can be pointed at, `restart` refuses pod,
replicaset and node this same way — the two drawn above, and node, which this
page does not draw. A kind outside those six — a Service, a ConfigMap — never
reaches this arm at all: `known_kind` refuses it before `restartable` is ever
asked, so it gets the unknown-shape refusal described next, full synopsis and
all, not this sentence-plus-pointer shape.

The doubled `k8rs: k8rs …` opening is not a typo; every headless refusal
reads that way on this build (compare `screens/context.md`'s own refusals).

**The pointer line is new, and it is not on every refusal.** A refusal here
is either the reader naming a shape k8rs does not recognise — a bad
operation word, a missing `-n`, missing replica count, an unknown kind — or
naming a real operation and a real object and being told k8rs does not do
that to that kind. The first group does not know the shape yet and keeps
the eight-line `ops_usage()` synopsis; the second already named a real
operation and object correctly, so the synopsis would bury the one-sentence
answer under a menu they do not need — they get the sentence and this
one-line pointer back to it instead
([NOTES § D236](../NOTES.md#d236--the-four-rulings-the-e2e-box-needs-where-a-wire-is-visible-what-just-e2e-is-then-and-the-synopsis-that-buried-a-correct-answer-2026-09-05)
ruling 4). Both refusals on this page — pod and replicaset — are the second
kind: `restart` is a real operation and both are real objects, restart
simply does not apply to either. The split is general over every operation
that exists and reaches more than one kind, not particular to `restart` —
pod and replicaset are this page's two drawn examples, not its only two.
An operation added later lands on this same side only if it refuses a kind
through this same call; nothing here is drawn or promised for a verb that
has not shipped.

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

## Delete — the name has to be typed, and nothing is checked first

`ctrl-d` opens this on any object, and — alone among the operations on this
page that reach more than one kind — it refuses none of them.
`deployment`, `statefulset`, `daemonset`, `replicaset`, `pod` and `node` —
every kind in the driver's `KINDS` — reach it, because unlike `restartable`
(a restart of a replicaset is a word with no meaning) there is no kind a
delete is meaningless on, so there is no `deletable()` to write and no
second matrix to keep straight
([NOTES § D225](../NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04)
ruling 3).

**And it is the first operation that asks the cluster nothing before it asks
the reader.** `scale` and `restart` both send a real `dryRun=All` and put its
answer — *"The cluster checked it first and accepted it."* — into the box
before the button goes live. `delete` never sends one
(D225 ruling 1): a dry-run delete and a real one build the **identical**
request line, with the only marker that tells them apart riding in the body,
so sending one before anybody has typed a name would put a live `DELETE` on
the wire — and in the cluster's own audit record, at the level most clusters
run, a cancelled dialog and a delete that happened would read the same. That
is a lie in a record k8rs does not own and cannot correct, which is worse
than the diagnostic value a preflight buys back. So every box below carries
`UNCHECKABLE`'s own line and never § Scale's or § Restart's: **"k8rs did not
check this one with the cluster first."** Nothing else is read before a
delete either — no replica count, no owner, no `uid`, no `finalizers` — so a
consequence sentence here never claims a fact k8rs has not asked for
(D225 ruling 4).

**Nor has k8rs read what may be attached to the object that answers back
after the delete is sent.** A `DELETE` that returns `200` is not proof the
object is gone: a `finalizer` can hold it — `deletionTimestamp` set, nothing
else changed — for as long as whatever put it there takes to act or step
aside. Measured against a real cluster: a Node carrying one answered `200`
and was still there; a Pod under one came back `status.phase: Running`,
unchanged
([reports/2026-09-04-delete-the-operator-review.md § 5](../reports/2026-09-04-delete-the-operator-review.md#5-what-a-successful-delete-hands-back-and-whether-the-object-is-gone)).
k8rs has read no `finalizers`, the way it has read no `ownerReferences`
(ruling 4) — so a delete here is a request the cluster may take time over,
or act on, before it is done. The four boxes below with no hedge of their
own say *asks* rather than *removes* because of this; pod and replicaset
already hedge, for the unread-ownership reason above, and are not redrawn
for this one.

**The taught line sits through more of that wait than k8rs does.**
`kubectl delete` waits by default (`--wait=true`): it blocks until the
object is verified gone rather than returning the moment the cluster
accepts the request. Measured: `kubectl delete node/…` against a node held
by a finalizer prints its usual `"<name>" deleted` and then does not
return — a five-second `timeout` around it exits `124`, still waiting, long
after k8rs's own call has already returned (same report, § 5). k8rs never
claims the object is gone, only that the cluster accepted the request; the
wait a finalizer adds is the taught line's to sit through, not k8rs's, and a
reader who copies it by hand meets it there.

### The pod, and the claim the old mockup made that k8rs cannot back

The box below fixes a defect in this file. It used to read *"Its Deployment
will start a replacement immediately — the app keeps running"* — a promise
k8rs has no way to keep, since nothing is read before a delete and a pod's
owner is one of the facts that stays unread. What replaces it says only what
k8rs can actually know: that *something* may replace the pod, and that k8rs
has not checked whether anything will.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ Delete payments/web-7d9f4 ──────────────────────────────────┐  │
│   │                                                             │  │
│   │  This removes the pod. Whatever created it will normally    │  │
│   │  replace it — k8rs has not checked whether anything did.    │  │
│   │  k8rs did not check this one with the cluster first.        │  │
│   │                                                             │  │
│   │  Type the pod's name to confirm:                            │  │
│   │  ┌───────────────────────────────────────────────────────┐  │  │
│   │  │ web-7d9f_                                             │  │  │
│   │  └───────────────────────────────────────────────────────┘  │  │
│   │                                                             │  │
│   │                [ delete ]     [ esc cancel ]                │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl delete pod/web-7d9f4 -n payments                         │
├────────────────────────────────────────────────────────────────────┤
│ type the name to enable   esc cancel                               │
└────────────────────────────────────────────────────────────────────┘
```

**The `$` line was missing from this box before, in the nested frame and on
the log strip both** — a gap against rule 3 below ("the command is shown"),
now closed the same way § Scale's and § Restart's already were:
`kubectl delete pod/web-7d9f4 -n payments`, the kind spelled out in full, no
`--dry-run` and no `propagationPolicy` flag. k8rs sends `Background`
explicitly on the real call — `kubectl delete`'s own default when none is
given — so the taught line needs no flag to be equivalent to what k8rs sent
(D225 ruling 5, measured against a real `kubectl` rather than recalled off
its docs; if that measurement ever disagrees, the line follows it and this
sentence is what was wrong).

**This box narrows to 61 columns of interior for the same reason § Restart's
did** — the two sentences and the typed-name field together do not fit
§ Scale's 58, and 61 is as far as the nested box goes before it touches the
outer frame. The blank line that would ordinarily sit between the
consequence and the verdict is gone, for the same reason § Restart's paused
variant dropped it: the row it would occupy is spent on the typed-name field
that § Scale's and § Restart's boxes do not have. This box is 22 rows —
two below the 24-row ceiling only § Restart's paused variant had reached
before it; § Scale's plain box is 20 rows and § Restart's is 21.

The four bullets below are not one shape any more. `replicaset` still shares
the pod's *hedge clause* word for word — not its wording — because neither
object's creator has been read: a replicaset can just as easily be owned by
something that will recreate it, and k8rs has no more checked that than it
has for a pod. What differs is the sentence in front of the hedge, and it
has to: deleting a replicaset also removes every pod it manages, which
deleting one pod does not. `deployment`, `statefulset` and `daemonset` carry
no hedge at all in the old wording, which is exactly backwards given the
paragraphs above — so all three now open with *asks* and close on one
finalizer hedge, worded identically across the three because the fact is
the same one three times:

- **deployment** — "This asks the cluster to remove the deployment and
  every copy of the app it runs. k8rs has not read what may be attached to
  it, and something there may delay this or act first — left alone,
  nothing is left running."
  `$ kubectl delete deployment/web -n payments`
- **statefulset** — "This asks the cluster to remove the statefulset and
  every copy of the app it runs. k8rs has not read what may be attached to
  it, and something there may delay this or act first — left alone,
  nothing is left running."
  `$ kubectl delete statefulset/web -n payments`
- **daemonset** — "This asks the cluster to remove the daemonset and the
  copy of the app it runs on every node. k8rs has not read what may be
  attached to it, and something there may delay this or act first — left
  alone, nothing is left running."
  `$ kubectl delete daemonset/web -n payments`
- **replicaset** — "This removes the replicaset and every pod it manages.
  Whatever created it will normally replace it — k8rs has not checked
  whether anything did."
  `$ kubectl delete replicaset/web-9f3a2 -n payments`

*Left alone* carries the same weight *asks* does above: it names the common
case — nothing attached, nothing to hold the delete up — without claiming
it is the only one, which "nothing is left running" did on its own until
this round.

### Node — the one a beginner reads backwards

Node is the first cluster-scoped object any operation in k8rs mutates —
`Api::all_with` where `scale` and `restart` have only ever built
`Api::namespaced_with` — and its title bar carries no namespace, by rule 1
below: the bare `node-3`, never `infra/node-3`.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ Delete node-3 ──────────────────────────────────────────────┐  │
│   │                                                             │  │
│   │  This asks the cluster to remove its record of node-3, not  │  │
│   │  the machine. Something attached to it, unread by k8rs, may │  │
│   │  delay this or act first. Left alone, its pods are deleted  │  │
│   │  and the machine keeps running until its kubelet restarts.  │  │
│   │  k8rs did not check this one with the cluster first.        │  │
│   │                                                             │  │
│   │  Type the node's name to confirm:                           │  │
│   │  ┌───────────────────────────────────────────────────────┐  │  │
│   │  │ node-_                                                │  │  │
│   │  └───────────────────────────────────────────────────────┘  │  │
│   │                                                             │  │
│   │                [ delete ]     [ esc cancel ]                │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl delete node/node-3                                       │
├────────────────────────────────────────────────────────────────────┤
│ type the name to enable   esc cancel                               │
└────────────────────────────────────────────────────────────────────┘
```

This is the one consequence on this page a beginner is likely to read
exactly backwards, and it has been corrected twice now, each time toward
the more destructive reading rather than the milder one. **The first
correction** was about the kubelet: the original draft said a kubelet
still running re-registers the node within seconds, and that a kubelet
already gone is the case that loses the pods — both backwards, measured on
kubelet v1.36.1
([reports/2026-09-04-delete-on-the-wire.md § 7](../reports/2026-09-04-delete-on-the-wire.md#7-a-node-whose-object-is-deleted-while-its-kubelet-keeps-running),
[NOTES § D225](../NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04)
ruling 3 as corrected). A running kubelet does not re-register on its own
at all — `registerWithAPIServer` runs once per process — so the node
stayed absent for the full 2 minutes 45 seconds watched and came back only
2 seconds after the kubelet process itself was restarted; the pods went
either way. **The second correction is this round's**, and it is the
sentence in front of that fact rather than the fact itself: *removes* and
*keeps running* both claimed more certainty than a `finalizer` k8rs has
not read allows, for the general reason the paragraphs above this box
give. *Asks* and *left alone* say what is true either way — the request
was made, and here is what happens if nothing intervenes — without
claiming nothing will.

**What the delete does not touch is node-3's ability to come back.**
Measured: its kubelet's client certificate is untouched, and no CSR
appeared asking to replace one in the 60 seconds watched. What does go
with the node automatically is its `Lease` in `kube-node-lease`, owned by
it through `ownerReferences` and removed the same way any owned object is;
what does not go is a pod already held on node-3 by a finalizer of its
own — left naming a node that no longer exists, with nothing in the
cluster to ever clear it
([reports/2026-09-04-delete-the-operator-review.md § 8](../reports/2026-09-04-delete-the-operator-review.md#8-node-deletion-second-run--what-goes-with-the-object)).

**Draining — `ctrl-r`, v0.2, § Drain below — is the operation that empties
a node safely: it moves pods off *before* anything is deleted and stops
new ones landing.** This delete does neither; it is not drain, and nothing
here performs one or requires one to have run first. That sentence is the
box's to have and the one still cut from it: four consequence lines
already reach the same 24-row ceiling § Restart's paused variant did, with
a typed-name field on top of them that neither of those boxes carries —
and the pods, the machine, and now the uncertainty over both earned the
room ahead of it.

### The verdict line, and what "not checked" costs and buys

The sentence in the box is not a lesser version of § Scale's and
§ Restart's — it is the true and complete verdict for an operation that
sends no check, said plainly rather than dressed up as a wait that never
happens. Two things follow from that, and both are worth a reader
understanding before they press `⏎`:

- **There is no earlier warning.** An admission rule or an RBAC role that
  would refuse this delete does not surface here — it surfaces only after
  the name is typed and the delete is sent for real, never before (see
  below, "Where delete's unhappy paths differ").
- **The button is never actually waiting on anything.** For `scale` and
  `restart`, `esc` is inert for as long as a real round trip to the cluster
  takes ([NOTES § D214](../NOTES.md#d214--the-mutation-contract-four-lies-a-record-could-tell-and-the-three-operations-that-have-no-dry-run-2026-09-04)'s
  "`esc` is inert until the verdict arrives"). For `delete` that rule still
  holds structurally — the confirm callback still cannot run before a
  `Checked` exists — but nothing was sent to wait on, so there is no
  perceptible delay: the verdict line and a live typed-name field appear
  in the same frame the dialog opens in.

**What is given up is small, and it is D225's to weigh, not this file's to
relitigate**: a preflight would catch a denying admission webhook before a
name is typed, but a `403` or a `404` comes back from the real call
regardless, and *the object went away* is already the watch's job and not a
dry-run's. Declining costs a little early warning; sending one would cost the
cluster's own record of the difference between a delete that was cancelled
and one that happened.

### Where delete's unhappy paths differ from scale's and restart's

Two of the three shared unhappy-path sections apply to `delete` unchanged.
**§ The object went away while the dialog was open** already uses a delete
as its own example — a pod replaced by its ReplicaSet while its name is
being typed — and nothing about it changes here. **§ While the call is
running** also applies as written: the modal closes on confirm, not on
completion, and the `…` on the command log line is replaced by the outcome
once the real `DELETE` returns.

**§ The cluster said no does not apply the same way, and the difference is
not cosmetic.** That section is what a *rejected dry-run* looks like:
`Outcome::NotSent`, the confirm button never having gone live enough to be
pressed, "Nothing was changed" *and* "This is the check that runs before the
real change — it stopped this one." Nothing about that sentence is true of a
refused delete, because there was no check to stop it — `checkable: false`
means the only call `delete` ever makes is the real one. So when a delete is
refused, it is `Outcome::Failed`, reached only *after* the name has been
typed and `⏎` has been pressed, and the sentence the operator is left
reading is built from the fault the real call hit rather than from a dry-run
verdict:

```
nothing was changed — the cluster would not allow it
```

which is `Fault::Refused`'s own wording — the sentence a `403` gets, keyed
off the fault the same way every other outcome on this page is (`fn
verdict`, `src/ops.rs` § THE MUTATION CONTRACT; a `422` from a validating
webhook is `Fault::Rejected` and reads differently — "the cluster would not
accept the request k8rs made" — but an RBAC role missing the `delete` verb
is the one this box below draws). The cluster's own words are joined on
after a colon where it sent any. "Nothing was changed" is still true — the
object is still there — but the clause after it is honest about what
actually happened: the real delete was sent, and the cluster said no to it,
not to a rehearsal.

### Printed instead of drawn — delete on the headless surface

`ops delete` is headless today. Because there is no dry-run, `show` and
`ask` run back to back with no cluster round trip between them — unlike
§ Restart's own printed section, where the paused warning and the verdict
had to wait on what the check answered, here nothing is waited on, and the
headless order matches the drawn box's order line for line. Confirming a
delete on `payments/web-7d9f4`, piped rather than typed at a tty:

```
$ echo web-7d9f4 | k8rs ops delete pod/web-7d9f4 -n payments
pod/web-7d9f4 in payments
This removes the pod. Whatever created it will normally replace it — k8rs has no
t checked whether anything did.
$ kubectl delete pod/web-7d9f4 -n payments
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the change was made
```

And a delete an RBAC role refuses — the same four lines up to the prompt,
then:

```
k8rs: nothing was changed — the cluster would not allow it: pods "web-7d9f4" is 
forbidden: User "jane" cannot delete resource "pods" in API group "" in the name
space "payments"
```

And a delete on an object a finalizer is holding — `node/node-3`'s own four
lines up to the prompt, then:

```
$ echo node-3 | k8rs ops delete node/node-3
node/node-3
This asks the cluster to remove its record of node-3, not the machine. Something
 attached to it, unread by k8rs, may delay this or act first. Left alone, its po
ds are deleted and the machine keeps running until its kubelet restarts.
$ kubectl delete node/node-3
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the cluster accepted this and the object is still there — something is del
aying the removal, and the command above waits for that where k8rs does not
```

`Outcome::Started`, and not `Outcome::Done`: the `200` the DELETE got back
was a `Node`, not a `Status`, which is the shape the answer takes when
something is still holding the object rather than gone
(`k8s-admin`). Exit stays `0` either way — the cluster did change,
`deletionTimestamp` is now set — so this is not a failure reported beside
the two above it; it is the third thing a delete can honestly say, next to
*it happened* and *nobody let it happen*. **What it does not say is
whether anything already acted.** The consequence hedges both — *may delay
this or act first* — because k8rs cannot tell them apart from the request
side; the result line reports only the half a `200` can prove, which is
the delay. That an admission controller or some other watcher may already
have started moving the pods, or the node, before this line was printed is
real and is the consequence's to carry, not this line's to guess at.

As with § Scale's own long line, the breaks above are wherever an 80-column
terminal ran out of room, not a word boundary — k8rs sends each of these as
one unwrapped line, and a wider terminal draws the break somewhere else or
not at all. (The confirmation prompt happens to be exactly 80 columns and so
does not visibly wrap at all here — a coincidence of this one sentence's
length, not a rule.) The consequence and the verdict are unrelated to the
confirmation prompt that follows them in all three examples; what changes
between them is only the very last line, which is `ops::Performed::plainly`'s
(`src/ops.rs` § THE MUTATION CONTRACT) and reads the same whichever operation
produced it.

The button stays disabled until the typed name matches, in the drawn dialog
exactly as it does here. This is the ctrl-key-slip guard, and it is why
deletes and drains are the two operations on this page that require typing
rather than a press.

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
│ $ kubectl delete pod/web-7d9f4 -n payments   → not sent            │
├────────────────────────────────────────────────────────────────────┤
│ esc dismiss                                                        │
└────────────────────────────────────────────────────────────────────┘
```

The dialog holds the object's `uid` — the field that answers *is this still
the same object*, the question this dialog exists to ask. It does not hold
the `resourceVersion` it opened with, which is what this section said until
this round and was wrong: `resourceVersion` answers *has anything at all been
written*, and that field bumps on a status-only write from the object's own
controller as readily as on a real replacement. Measured on a real cluster: a
healthy, idle Deployment writes zero times in three minutes, while a
`CrashLoopBackOff` one — the object the operator most needs this dialog to
still work on — writes every 2.45 seconds, median
([NOTES § D228](../NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05)).
A dialog keyed on that field would flip to "changed" and kill its own confirm
button about that often.

So **Gone** — the screen above — is the only outcome this dialog has. The
confirm button dies, because sending a delete by name at this point would hit
whatever now holds that name, which is how the wrong pod gets deleted.

A write that leaves the object in place — a scale, a status update, a label —
does not reopen this dialog, and none of today's three operations need it to:
`scale` asks for a count, not a change relative to whatever is running, so a
write between open and confirm cannot make the count the operator agreed to
wrong; `restart` and `delete` read no live number this dialog would need to
refresh either. A genuine read-modify-write conflict — the case a
`resourceVersion` precondition actually guards — is `edit`'s, arriving with
v0.4, with its own precondition and its own state to design then, never this
one
([REQUIREMENTS](../REQUIREMENTS.md#write-operations-new--the-reversal)).

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
