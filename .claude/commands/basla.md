---
description: Run k8rs boxes back to back as the PM — no approval between boxes
---

You are the PM. `CLAUDE.md` is binding; § *Agent workflow* is the procedure.

Before picking a box: if the working tree is dirty, that is a box left in
flight (a crash, a power cut). Read the diff, finish or land it, and do not
start a new box on top of it.

Then, until told to stop:

1. First unchecked box in the **lowest open** phase of `todo.md` — read the
   file, do not assume the highest-numbered phase is the only one running. If
   the boxes under it are the same **family** touching the same code, brief them
   as one turn (D104); unrelated boxes stay one at a time.
2. Run the cycle: brief → (screen spec) → dev writes code + tests and proves its
   own red/green → `just mutants` → `tester` attacks the assertions and runs
   `just check` → `k8s-admin` reviews the family → land it.
3. Land = second pass over the landed tree · security gate · check the box in
   the work commit · CHANGELOG separately · push.
4. Next box. No "shall I continue" (D98).

This command also runs unattended as a single-turn process (`claude -p`):
when the turn ends the process exits and every background job dies with it.
So never end a turn waiting on background work. Anything you must wait for —
a mutation sweep, a build — runs in the foreground, blocking. If a sweep from
an earlier run is already going, wait for it in the foreground or restart it;
do not park on it and hand back a status report.

A foreground call is capped at ten minutes and the whole mutation sweep is
longer than one, so the phase-close sweep runs in shards — `--shard k/4`, one
call each, all four green or the gate is not passed
([D118](../../NOTES.md#d118--a-foreground-call-is-capped-at-ten-minutes-and-the-phase-close-sweep-is-longer-than-one-2026-08-20)).

Stop only for: a red build, a box no agent can run (credential, login,
account), or a reversal of a design decision — which is written into
`NOTES.md` before it is acted on.

At phase close, run § *Phase close* in full, including the PR to `main`.

Report at each push, in Turkish: which box closed, what was run, what it
printed.
