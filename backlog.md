# Backlog — work that belongs to no phase yet

`todo.md` holds **phases**: ordered boxes, each with a done-when, picked one at a
time. This file holds what has no phase — a finding nobody has ruled on, a gap
`PRIOR-ART.md` names, an idea that survived
[CLAUDE.md § invariant 13](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
but has no home. It exists so that *later phase* stops being the answer to
everything found mid-flight
([D108](NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)).

**Three rules, and the second is the one that keeps this from becoming a second
`todo.md`:**

1. **Nothing here is work.** No agent picks from this file, ever. `/basla` reads
   `todo.md` and only `todo.md`.
2. **Write freely during a phase, read only at phase close.** Anything found
   mid-phase lands here in one line; the triage that turns entries into boxes
   happens at phase close, with the rest of the ritual.
3. **An entry that becomes a box leaves.** It is deleted here and written there —
   never both, or the copy that goes stale is this one.

An entry is one line: what was found, where the evidence is, and — if there is
one — the `NOTES.md` decision that already touches it. If it needs a paragraph to
state, it needs a decision, and a decision goes in `NOTES.md`.

---

## Open

- **The phase-close run on the test host has no guard.** Every other step of
  [CLAUDE.md § Phase close](CLAUDE.md#phase-close--the-ritual-at-the-end-of-every-phase)
  is proven by something that cannot lie — `just check`, the `scripts/` guards,
  `cargo deny` — except step 2, *build it and run it on the test host*, which
  needs a remote machine and is therefore the one that can be skipped in silence.
  Raised 2026-08-16; the user has not ruled on it.
- **Rule 5's `Ending::Failed` clause asserts an external killer its own evidence
  contradicts.** `", but something keeps killing it"` (`rules.rs:3596`) sits, on
  the committed corpus, over `exit 127 (the command was not found)`, `exit 128`
  and `exit 1 (the application's own error)` — three endings that name the
  program, not a killer. `", and the last run on record failed"` would be true
  on every arm and parallel to the five clauses beside it. Found by the
  plain-language pass and left alone on purpose
  ([D117](NOTES.md#d117--the-plain-language-pass-and-the-two-things-it-found-that-were-not-sentences-2026-08-19)):
  it is a claim about who acted, not a phrasing, and it is pinned in
  `screens/alerts.md` and in `pod.rs`'s `KILLED_IT` guard, so changing it is a
  ruling with two files behind it.

- **`node_running_low`'s multi-pressure action joins two whole sentences** —
  *"free up disk space on this node and free up memory on this node, or move
  some pods elsewhere"*. Clumsy, not misleading, so the pass left it
  ([D117](NOTES.md#d117--the-plain-language-pass-and-the-two-things-it-found-that-were-not-sentences-2026-08-19)).
  The fix is not local: `on this node` would have to come out of the `PRESSURES`
  table, and `screens/alerts.md` states that table row by row.

- **Rule 1's `Ending::Failed` arm titles a card *"keeps crashing"* on a
  container that has crashed exactly once.** Found while closing the
  `restarts > 0` mutant box, by the first test in the repo that draws that arm
  with `restartCount: 0` — the window between the first crash and the first
  restart, which no committed capture holds. The `None` arm already had this
  reasoning applied to it and says so in its own doc; `Ending::Failed` kept the
  old title. Not fixed here: the box was one mutant wide, the title is
  `rules.rs`, and whether *keeps crashing* is wrong or merely early is a
  question for whoever reads rule 1's three sentences together — the same
  family review [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
  came out of.

- **`src/rules_tests/pod.rs` is 12 902 lines (2026-08-19), the largest file in
  the repo**, and
  every dispatch touching a pod rule pages it
  ([D110](NOTES.md#d110--the-brief-names-the-regions-because-a-cold-dispatch-reads-fifteen-thousand-lines-2026-08-16)).
  Splitting it is the obvious next cut and is deliberately not taken on a line
  count alone: it is where
  [D91](NOTES.md#d91--the-tests-split-and-the-product-file-does-not-2026-08-15)'s
  warning lands, since a module boundary is where a second copy of a shared
  helper grows back. Needs evidence and a ruling, at a phase close.
- **An in-place resize restart reaches `137` far more often than `143`, and no
  card on that path names it.** Measured on kind v1.36.1
  ([reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md](reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md)):
  a container whose PID 1 has no `SIGTERM` handler — the stock case — comes back
  from `resizePolicy: RestartContainer` as `137` / `Error`, and only one that
  traps the signal gives `143`. Family B added the resize door to
  `stopped_action`, i.e. to the ending a *well-behaved* container reaches; the
  commoner outcome lands on `killed_action` and `failed_action`, which name no
  resize, and `killed_action` names no events either — so the
  `Killing … resize requires restart` line the answer is sitting on is on no
  card's path. Found by the Family B operator review, 2026-08-16.
- **The lost-run suppressor deletes the only card naming a sandbox rebuild, in
  one shape.** A container with `Ending::Unwatched` in `lastState`, restarted
  **once** (below `RESTARTS_WARN`, so rule 5 is silent), whose readiness probe
  has been failing past `NOT_READY_GRACE`: rule 7 fires, is `Reads::Now`, and
  rule 6's lost-run card goes. Nothing left says the container was taken out from
  under the kubelet. The operator review would still ship the suppressor — the
  deleted card is undated and permanent — and named the shape because it is the
  one where the deleted card was the answer
  ([D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)).
  2026-08-16.
- **Nothing in `just check` counts panics in product code, and two reached
  `rules.rs` before anyone noticed.** `clippy` does not flag `.expect()` by
  default, the mutation run cannot see a call that never fires, and both were
  found by the PM reading the diff by hand
  ([D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)).
  A guard is one `grep` over `src/*.rs` minus the test modules, on the footing
  `security-guard.py`'s other six checks already stand. `tester`'s, and the
  interesting half is what the allowlist is: `main.rs` will need `expect` on the
  terminal restore, so the rule is *not zero*, it is *named and argued*.
  2026-08-16.
- **`assert_states`' report pass re-fetches, so it can contradict the wait loop
  it just ran.** The loop drops a name from `pending_list` the moment its
  predicate holds; the report then samples the object *again* and prints
  PASS/FAIL off the second sample. On a fixture whose state is transient by
  design that is a false red: on 2026-08-16 `[crashloop]` failed the report while
  passing 3/3 live seconds later, because the re-sample landed in the ~2s window
  where the container is up — the window that predicate's own comment says it
  deliberately excludes. `set -e` then ended the run before the slow pass, so one
  race costs the whole 26-minute climb. The bias is toward a false red and never
  a false green, which is the safe direction, and the fix is to report the loop's
  own verdict rather than a fresh sample. Not taken during the capture trip that
  found it: `assert_states` is the shared helper every state runs through, and
  CLAUDE.md keeps a shared-helper change per-box. 2026-08-16.
- **Rule 5 reaches its band only through endings that failed, and the two that
  finish are still plants.** `src/rules_tests/pod.rs`'s `restarts10_ending` names
  it exactly: a container that reaches `RESTARTS_WARN` by *finishing* — `exit 0`,
  and a second on `exit 143` — and is then **running** and out of
  `CrashLoopBackOff`. `restarts10.json`'s own `spec` is one character away
  (`[ "$n" -le 10 ] && exit 1`), so the manifest is known; what it costs is two
  more pods on the 26-minute backoff climb. Ruled out of the 2026-08-16 capture
  trip by the PM as a different rule's subject
  ([D114](NOTES.md#d114--the-capture-trip-that-put-four-objects-on-disk-and-the-init-arm-that-is-not-reachable-at-all-2026-08-16)),
  which is what makes it phaseless rather than owed. 2026-08-16.
- **`verify` proves the live object, and the capture is a second fetch minutes
  later.** `assert_states` polls the running pod; the `fixtures` recipe then
  captures in a separate `kubectl get`, so a capture can still land in the
  ~2s up-window that `verify` just refused. The `justfile`'s `guard` lines are
  the only thing that reads the *committed bytes*, and on 2026-08-16 seven
  captures had none at all — `oom`, `image`, `config`, `readiness`, `nolimits`,
  `stuck`, `init`. The two crash-loop ones (`crashloop`, `init`) got guards in
  the capture trip because their state is transient; the other five hold still,
  and whether they want guards anyway is a ruling nobody has made. Found by
  `tester` while closing the `[init]` predicate hole
  ([D114](NOTES.md#d114--the-capture-trip-that-put-four-objects-on-disk-and-the-init-arm-that-is-not-reachable-at-all-2026-08-16)).
  2026-08-16.
- **Rule 13's terminated clause stops at a clean exit, and the endings past it
  are a smaller instance of the same silence.** The shipped clause counts a
  `Terminated` container as something to point at unless it `Finished`. Measured
  by `dev-core` with a throwaway probe rather than reasoned: a container whose
  ending is `Stopped` (and by the same route `Unwatched`, `RestartRule`,
  `CodeUnknown`) beside nothing but a `PodInitializing` sibling makes
  `nothing_else_to_point_at` false, suppresses the wedge card, and leaves the pod
  with **no card at all** — the same total silence the rule 13 blocker was about,
  one ending over. Deliberately not widened in the box that found it: no capture
  or measurement produces the shape, and widening a silence on reasoning alone is
  exactly how the clause was got wrong the first time
  ([D114](NOTES.md#d114--the-capture-trip-that-put-four-objects-on-disk-and-the-init-arm-that-is-not-reachable-at-all-2026-08-16)).
  2026-08-16.
- **`PRIOR-ART.md`'s gaps that no ruling has boxed.** The file is evidence and
  never a plan, and a gap becomes a box only by a decision
  ([D89](NOTES.md#d89--k9ss-tracker-is-read-as-prior-art-and-twelve-of-its-classes-become-boxes-2026-08-14)
  is the first and so far the only one). The rest sit there unread between
  phases; this line is the reminder to read them at each close, not a licence to
  box them mid-phase.

- **`rules_tests.rs` cites a `cluster.sh` subcommand that does not exist.** Lines
  475 and 487 name `break-nodes`; `scripts/cluster.sh` has `break` and `unbreak`
  and nothing else. Found by `dev-core` while writing the mutation tests and left
  alone, because it is outside that box and `scripts/` is `tester`'s tree — the
  comment is in `dev-core`'s. Two writers, one line each, so it needs a box
  rather than a fix in passing
  ([D119](NOTES.md#d119--the-last-surviving-mutant-was-equivalent-and-the-fix-is-to-stop-spelling-the-tie-by-hand-2026-08-20)).
  2026-08-20.

- **`short_of_pods`' `updated < desired` arm is reachable mid-surge, and nobody
  has watched it happen.** `tester` read
  `pkg/controller/deployment/{sync,rolling}.go` and found that at `replicas: 2`
  (defaults `maxSurge: 1, maxUnavailable: 0`) the sync where the surge pod
  becomes available persists `readyReplicas: 3, updatedReplicas: 1` with
  `unavailableReplicas` absent — this arm, on an ordinary unpaused rollout, which
  both the rule's doc and the first draft of
  [D119](NOTES.md#d119--the-last-surviving-mutant-was-equivalent-and-the-fix-is-to-stop-spelling-the-tie-by-hand-2026-08-20)
  said could not happen. Reasoned from upstream source, **not measured**; the
  wording in both places now says so. What is unknown is how long that window
  lasts and therefore whether the rule flickers a card during every normal
  rollout — a `K8RS_CLUSTER=review` measurement, one two-replica rollout watched
  through. 2026-08-20.

- **Rule 8's `/` escalator draws a permanent CRITICAL on every cluster running
  `prometheus-node-exporter`, and its action names a fix that pod cannot take.**
  Measured, two pods, read-only `/host/root`, `Severity::Critical` with
  `timestamp: None` so the card never clears
  ([reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md](reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md)
  § 1). This is the evidence
  [D70](NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)
  asked for and it points against the current spec; the ruling is the user's
  because it reverses `NOTES.md`'s own escalator list
  ([D120](NOTES.md#d120--the-two-things-the-operator-review-measured-that-the-tests-cannot-see-2026-08-20)).
  The cheapest half is not a rule change at all: the `/` arm names no legitimate
  holder where the socket arm names one. 2026-08-20.
- **Rule 7's floor is the grace clock as well as the timestamp, and past the
  first restart that is wrong in both directions** — the card understates a
  three-hour outage as *15 min ago*, and on a restarting container the grace
  never elapses so the rule is silent altogether. Measured across seven restarts
  ([reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md](reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md)
  § 2). The proposed cut is `c.restarts == 0` — the shape the floor genuinely
  protects — and it reverses
  [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13),
  so it is a ruling and not a fix; the gap is named in the rule's doc meanwhile
  ([D120](NOTES.md#d120--the-two-things-the-operator-review-measured-that-the-tests-cannot-see-2026-08-20)).
  2026-08-20.
- **A `subPathExpr` mount puts a path on the card that `kubectl describe` cannot
  show, and prints `$(POD_NAME)` at a beginner.** `describe` prints `path=` for
  `subPath` and nothing for `subPathExpr`, so rule 8's evidence line names a path
  its own teaching command does not contain — [invariant 4](CLAUDE.md) in the
  small, and a template variable on a card is [invariant 14](CLAUDE.md).
  `-o yaml` shows both fields. Pre-existing and D46-sanctioned; named as a cost
  nowhere until now
  ([reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md](reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md)
  § 4). 2026-08-20.
- **Upstream refuses to overwrite `ProgressDeadlineExceeded` with the paused
  condition, so pausing a rollout that has already timed out keeps W2's card
  standing.** Read as correct — the rollout did give up — but recorded nowhere,
  and the rule's doc is where a reader would look
  ([reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md](reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md)
  § 5). 2026-08-20.

- **`out_of_memory` and `restarting_repeatedly` carry a byte-identical grace
  clause** — `|t| now.0.duration_since(t.0) > NOT_READY_GRACE` — spelled out in
  both rather than reached through a helper. Two rules reading one field through
  a copied expression is the class
  [CLAUDE.md](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it)
  says has cost this repo most. **It is not a hole today**: both copies are
  killed by the sweep, and the 2026-08-20 boundary test pins rule 2's at exactly
  ten minutes. It is a helper waiting to be extracted, and the moment to do it is
  when a third rule wants the same clause. Found by `dev-core` while anchoring a
  red-proof script, 2026-08-20.

## Ruled out

*Entries that were considered and deliberately not built keep one line here with
the decision that refused them, so the same idea does not arrive twice.
[NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section) is the
long-form version and stays the authority.*

- **mem0 as a persistent-memory service** (2026-08-16) — the job is already done
  twice, by `NOTES.md` and by the session memory directory, and a hosted instance
  would put project data on an outbound connection that
  [docs/security.md](docs/security.md) says does not exist.
