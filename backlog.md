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

- **Free text from the API is unbounded all the way to the screen, and the
  temporary driver's header is the instance with a measurement.** Handed one
  object whose `kind` is 10 MB of `K`, `k8rs` reads it, holds it and prints it:
  a first line of 10 000 061 bytes, 51 MiB peak RSS, exit 0 — `sanitize` strips
  and deliberately never truncates
  ([D122](NOTES.md#d122--the-strip-goes-on-the-value-entering-the-sentence-not-on-the-finished-sentence-2026-08-20),
  `screens/widgets.md` § 7). **The box to write is not *bound the header***, or
  Phase 5 closes the header and leaves the 50 MB annotation and the endless log
  line beside it — it is *bound every free-text field at ingest*, which
  [CLAUDE.md § Security gate](CLAUDE.md#security-gate--run-this-list-on-every-change-no-exceptions)
  already states and nothing below Phase 5 implements. Not a blocker today: the
  input is argv, so only the operator can reach it. Measured by `tester`,
  2026-08-20.
- **Rule 6's action line says "the `--previous` flag below" and there is nothing
  below it in any renderer that exists.** In the console *below* would be the
  command-log strip, which carries what k8rs ran, not the finding's own
  `kubectl_cmd`; the Alerts card is four parts and has no command line
  ([screens/alerts.md](screens/alerts.md)), and the temporary driver draws the
  same four. So the sentence points a beginner at something no screen shows —
  [invariant 14](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision).
  The fix is renderer-independent wording in `rules.rs`, not a line added to a
  card. Found while reading the driver's real output over the captures, where it
  printed 8 times. 2026-08-20.

### From the Phase 3 close cross-family review (2026-08-20)

*Seven findings, none ruled a blocker, so the phase is not held on them
(CLAUDE.md § Phase close step 6). Evidence and the exact output for all of them:
[reports/2026-08-20-phase-3-close-cross-family-review.md](reports/2026-08-20-phase-3-close-cross-family-review.md).*

- **A pod with a `deletionTimestamp` keeps drawing present-tense container
  cards.** Rules 10, 13 and 14 carry the guard and rule 12 triggers on it; rules
  1–8 and 15 do not. Measured on a lost node: `● …crashloop` (a container is
  backing off) and `▲ …crashloop` (this pod was asked to shut down) on one pod,
  while `kubectl describe` — the command those rules teach — prints
  `Status: Terminating`. Reachable on a healthy cluster too: any rolling update
  whose old pod was crash-looping at the moment it was replaced. Rule 10's own
  doc already holds the reasoning verbatim, so this needs no new ruling — the
  cheapest of the seven ([D73](NOTES.md#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13)).
- **N1 says nothing on the node can be trusted, and nine cards on the same
  screen trust it.** A stopped kubelet's pod status is a fossil that never
  expires — `phase`, `restartCount` and `lastState.terminated.finishedAt` all
  measured frozen over six minutes — so a card dated off it ages forever while
  its subject is over. 9 of the 29 cards on the committed captures are pods on
  `worker3`, which is `Ready: Unknown`. N1's doc names the mechanism and stops
  at N1. **Needs a PM ruling before code**: suppress, re-word, or add a fact —
  all three reverse something.
- **`failed_run_action` promises a log it cannot promise.** `previous_logs`'
  own doc says the action "names the log as the place the answer was written
  **rather than promising to hand it over**"; the string says "is what fetches
  it", and on a lost node `kubectl logs --previous` returns
  `dial tcp …:10250: connect: connection refused`. **This is the other half of
  the `--previous flag below` entry above** — that one is about the word
  *below*, this one about the promise, and the string belongs to
  `failed_run_action`, which rules 1, 5 and 6 all print.
- **Rule 8 hand-spells `container {name}` instead of calling `container_fact`.**
  `host_path_mounts` walks init containers and regular ones and throws the role
  away, so one init container gets two names on one screen: `container prep`
  from rule 8, `init container prep (the app starts only after this one
  finishes)` from rule 6. A reader taking the first to `kubectl describe` looks
  under `Containers:` and it is not there. The second copy of a shared helper,
  which is the class [invariant 11](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
  keeps the product file undivided to prevent.
- **`out_of_memory` is the only rule that prints an exit code without
  `exit_fact`.** Rules 2 and 5 co-fire by design and land adjacent, so one fact
  gets two spellings two lines apart — `exit 137` beside `exit 137 (killed by
  the kernel …)`, and `10 restarts` beside `restarted 10 times`. `FACTS`' own
  doc says a screen may not do this.
- **`explains_a_shortfall`'s doc contradicts itself about rule 14** — one
  paragraph says any of rules 8, 12 and 14 silencing a dead rollout hides the
  outage, another twelve lines down says rule 14's shape is the most common true
  explanation. The code follows the second and is believed right; the first is
  the stale copy, and it is the paragraph the next editor would fix the code
  against. This is the one helper where the whole W-series' silence is decided.
- **The driver heads every card with `object`, and `screens/once.md` heads it
  with the owner** — a fourth divergence where
  [D121](NOTES.md#d121--the-temporary-driver-and-the-three-places-it-does-not-draw-what-the-console-will-2026-08-20)
  enumerates exactly three. Visible on W1, whose doc says the card files under
  the ReplicaSet's owner "so the reader sees the name they deployed and not a
  hashed one": the driver prints `broken-quota-59654c756`. A bug in the
  decision, not in the code.
- **Rule 7 not firing on a lost node is reached by accident, not by a check.**
  `markPodsNotReady` flips the pod-level `Ready` condition and leaves
  `containerStatuses[].ready` alone, and rule 7 requires `!c.ready` — measured.
  The right answer, resting on an upstream behaviour nothing in this repo
  records. Worth a NOTES line whichever way the second entry above is ruled.
- **The driver hands the rules `namespace_scope: None`, which is the value that
  means *I can see every pod in this cluster*, and `load()` cannot know that.**
  Its input is whatever files were named on argv. Reproduced on committed
  fixtures, no cluster: `k8rs tests/fixtures/nodes.json tests/fixtures/healthy.json`
  draws N1 on `k8rs-worker3` with **no pod line at all**; add
  `tests/fixtures/kube-system-pods.json` and the same card reads
  `kube-system/kindnet and kube-system/kube-proxy were running here (2 pods)`.
  N1's own doc comment refuses exactly this — *"one pod was running here" about a
  node carrying forty reads as complete* — and N2's count and N5's sum are gated
  on the same field. Visible in this phase's own close review, where the operator
  fed it `kubectl get pods -n default` and the card counted 2 on a kind worker
  that also carries kindnet and kube-proxy
  ([reports/2026-08-20](reports/2026-08-20-phase-3-close-cross-family-review.md) § 2).
  A **fifth** divergence where [D121](NOTES.md#d121--the-temporary-driver-and-the-three-places-it-does-not-draw-what-the-console-will-2026-08-20)
  enumerates three, and the `object`-versus-owner entry above is a fourth. Ruled
  not a blocker at the Phase 3 close — the header names what was read, and Phase
  5's *Namespace scoping: `--namespace/-n`* box is where the field first gets a
  value a caller can support — but the fix
  is not *set it to `Some`*, which would trade a soft wrong number for hard
  silence in the only harness that exercises the node rules.
- **`just mutants` names a file that does not exist and does not name `main.rs`,
  and `cargo mutants` is silent about both.** The recipe is
  `cargo mutants --timeout 90 --file src/rules.rs --file src/analysis.rs`;
  `analysis.rs` arrives in Phase 4, and measured at HEAD
  `cargo mutants --list --file src/analysis.rs` prints **nothing and exits 0** —
  so a path that drifts or is mistyped makes the phase-close gate go green having
  mutated nothing, which is [CLAUDE.md § a derived list asserts it found
  something](CLAUDE.md#code-phase-rules) one layer out. `main.rs` is product code
  with branches and is in no `--file`; its coverage came from the per-turn
  `--in-diff` run, which for a new file happened to be the whole of it —
  re-measured at HEAD as `cargo mutants --timeout 90 --file src/main.rs`,
  **49 mutants, 46 caught, 3 unviable, 0 missed**, matching `--list`'s 49 exactly.
  Both are `tester`'s, in a later phase; found by the Phase 3 close second pass.

- **`settled` reads a policy and not the restart rules beside it, and a
  *sibling's* rule un-settles the whole pod.** Measured on kind v1.36.1
  (`reports/2026-08-20-settled-and-the-last-run-on-record.md` § 5): rule 15 drew
  *this container has stopped and nothing is starting it again* about
  `bystander`, whose sibling declared `RestartAllContainers`, and the kubelet
  restarted it **48 s later**. A container's own `Restart` rule leaves no trace
  in the status at all, so the whole backoff window is settled-by-mistake. Rule
  15's pre-existing D97 gap widened by measurement, not by this box — but it adds
  a requirement to the Phase 4 `restartPolicyRules` box that was not in it: the
  field must be read **across siblings**, and `RestartAllContainers` anywhere in
  a pod un-settles every container in it
  ([D125](NOTES.md#d125--the-last-run-on-record-is-a-question-about-the-container-not-a-field-and-stateterminated-may-name-a-card-only-where-the-run-is-settled-2026-08-20),
  PRIOR-ART § F3).
- **A container the pod's own rule killed for good draws nothing.** `gangwait`
  after a gang restart — `bystander` at `restarts=1`, `state.terminated exit 1`,
  `lastState 137/RestartingAllContainers` — is `1/2 Error` to `kubectl get pods`
  and `○ nothing is broken` to k8rs. Not a regression (identical before and
  after), but D96 leg 4's accepted cost is one container larger than that entry
  counts, and the `settled` clause makes the silence *decided* rather than
  accidental (`reports/2026-08-20-settled-and-the-last-run-on-record.md` § 5).
- **`--once` is the surface where *the command below* costs most, and it ships
  first.** `screens/once.md` puts findings on stdout and the commands k8rs ran on
  stderr, so `k8rs --once > findings.txt` leaves every action sentence pointing
  at something that is not in the file. Sharpens the two `below` entries already
  in this list rather than replacing them; the flag itself is no longer lost —
  D125's caller appends `, using --previous` — but the deixis is unresolved and
  is a `tui-designer` question before Phase 12.

- **Rule 2 and rule 15 draw two CRITICAL cards about one container and the pair
  overflows the pane.** A never-restarted settled `OOMKilled` container under
  `Never`: rule 2 at 6 lines, rule 15 at 10, 17 with the blank between them,
  against `screens/alerts.md` § The height's **16-row** body pane at 80×24.
  Both actions are genuinely needed — you cannot raise a limit on a pod that has
  to be replaced, and rule 2's card cannot tell you it has to be — so
  `one_card_per_action` is working rather than failing; what the operator review
  wanted at 3am is **one** card carrying both clauses
  (`reports/2026-08-20-the-settled-record-across-four-rules.md` § 7). A rule-set
  change, not a fold change, and it needs `tui-designer` before it needs code.

- **`just check` runs no rustdoc step, so every intra-doc link in the codebase is
  ungated.** Found while landing the `Report` shape, which is almost entirely doc
  comments: `analysis.rs` gained links to `Finding::title`, `Row::Answer::jump`,
  `Jump` and `Row::NotComputed` and nothing in the gate reads them. Proven live
  rather than asserted — `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo
  doc --no-deps --document-private-items` exits 0 on the tree, and exits 101 with
  a planted `[`Row::NoSuchVariant`]`. It is one line in `justfile` and `tester`'s
  tree, not `dev-core`'s. The cost of the gap grows with every frozen file, since
  a doc link that stops resolving in a frozen file cannot be fixed there
  ([D127](NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).

## Ruled out

*Entries that were considered and deliberately not built keep one line here with
the decision that refused them, so the same idea does not arrive twice.
[NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section) is the
long-form version and stays the authority.*

- **mem0 as a persistent-memory service** (2026-08-16) — the job is already done
  twice, by `NOTES.md` and by the session memory directory, and a hosted instance
  would put project data on an outbound connection that
  [docs/security.md](docs/security.md) says does not exist.
