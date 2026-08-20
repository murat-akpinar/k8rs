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
the path `scripts/sanitize.jq` exists to guard for fixtures — and that guard does
not run here. Until it does, the rule is manual and narrow:

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
  it is the same risk on a path with no guard yet.

A leak never leaves git history. When in doubt the value does not go in; name the
field instead and say what it held.

**The guard is owed.** Extending the fixture sanitization gate over `reports/` is
`tester`'s, boxed in Phase 4. Until it lands, the paragraph above is enforced by
the PM reading the diff, which is the weaker thing this repo has already been
burned by — so keep reports thin.
