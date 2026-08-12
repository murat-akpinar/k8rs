# Screens — Confirmations and failures

No mutation happens without one of these. The consequence is stated in plain
language **above** the command, never instead of it.

## Scale — confirm with dry-run

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Scale payments/web ──────────────────────────────────┐        │
│    │                                                      │        │
│    │  This starts 1 more copy of your app.                │        │
│    │  Right now: 2 copies.  After: 3 copies.              │        │
│    │                                                      │        │
│    │  The cluster checked it first and accepted it.       │        │
│    │                                                      │        │
│    │  $ kubectl scale deploy/web --replicas=3 -n payments │        │
│    │                                                      │        │
│    │            [ ⏎ do it ]    [ esc cancel ]             │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl scale deployment/web --replicas=3 -n payments            │
├────────────────────────────────────────────────────────────────────┤
│ ⏎ do it   esc cancel                                               │
└────────────────────────────────────────────────────────────────────┘
```

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
│    │  This was caught by the check that runs before the   │        │
│    │  real change, so nothing reached your cluster.       │        │
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
│    │  Nothing was sent to the cluster.                    │        │
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
