# reports/ — measurements, kept

An agent measures something, pastes the output into its report to the PM, and
that report lives in a conversation that ends. What survived did so only because
someone retyped it into a box body — *`docker restart` leaves `exitCode: 255,
reason: "Unknown"`* is in `todo.md` for that reason and nowhere else. This
directory is where the measurement itself goes
([D108](../NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)).

## What goes here

Raw output and the command that produced it: a `k8s-admin` measurement run, a
benchmark, a capture log, a version probe, the before/after of a guard. One file
per measurement, named `YYYY-MM-DD-<subject>.md`, so two agents writing on the
same day never collide.

## What does not

**A conclusion.** If the measurement settled something, the settlement is a
decision and it goes in `NOTES.md` with a `D##`; the report is the evidence that
decision cites, never the place the reasoning lives. A report that argues is a
second copy of a decision, and the second copy is the one that goes stale
([CLAUDE.md § Every file here also has to get smaller](../CLAUDE.md#every-file-here-also-has-to-get-smaller)).

Nothing here is a plan, a box, or work. `todo.md` holds boxes,
[`backlog.md`](../backlog.md) holds what has no phase, this holds what was seen.

## The sanitization rule — read it before pasting cluster output

A report carries real cluster output into a **committed** file, which is exactly
the path `scripts/sanitize.jq` exists to guard for fixtures. Since 2026-08-20
`scripts/reports-guard.py` runs over this directory on every `just check`, reading
prose rather than JSON ([D126](../NOTES.md#d126--the-guards-family-a-added-and-the-five-judgement-calls-they-could-not-avoid-making-2026-08-20)).
It refuses what is listed below, and it refuses any non-`.md` file here unread —
but it reads for known shapes, so the rule is still yours to follow and narrow:

- **No object dumps.** Not `kubectl get -o yaml`, not `describe`, not a captured
  JSON body. Those are fixtures, they go through `just fixtures` and the
  sanitizer, and they live in `tests/`.
- **What may be pasted:** the command, its exit status, and the *specific field
  values the finding turns on* — `exitCode: 255`, `reason: "Unknown"`, a version
  string, a count, a timing.
- **Never:** tokens, certificates, keys, kubeconfig contents, environment
  variable values, annotation payloads, node IPs or hostnames, Secret data — the
  same list as
  [REQUIREMENTS § DevSecOps](../REQUIREMENTS.md#devsecops-requirements), because
  it is the same risk on a path a reader trusts.

A leak never leaves git history. When in doubt the value does not go in; name the
field instead and say what it held.

**What the guard cannot see is still yours.** It matches known shapes — a token, a
PEM block, a kubeconfig, an env value, an annotation payload, a node IP, a
hostname. A secret in a shape nobody planted walks past it, so keep reports thin
and name the field rather than pasting the value.

## Retention — nothing here is deleted for having landed

Measured at Phase 4 close, `reports/` was 157K against `NOTES.md`'s 778K, and it
is not one of the two files every agent *must* read, which is what D103 was
about. So
there is **no count, no age and no size bound** on this directory, and a report
is **never** reduced to the `D##` that cites it — 13 of the 37 citations from
outside point at a *section* of a report, and a decision is not where a
measurement table lives
([D138](../NOTES.md#d138--reports-keeps-everything-and-the-retention-rule-is-a-re-measure-trigger-2026-08-22)).

Deleting a report is allowed and is guarded: `scripts/check-docs.py` fails on a
link whose file is gone, so you may delete one that nothing cites and not one
that something does. The ruling is re-taken — not re-argued — when `du -sb
reports/ NOTES.md` stops putting this directory well below that file.
