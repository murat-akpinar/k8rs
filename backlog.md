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

- **`analysis.rs` claims the `hostPath: {path: "."}` shape is "in neither" list,
  and it is in one.** A writable `.` hostPath outside `kube-system` draws a rule 8
  card with the path simply missing — `container app ·  on the node · writable`,
  two spaces, because `rules.rs` formats `{path} on the node` with `path` empty.
  The card is frozen `rules.rs`; the false sentence is `analysis.rs`'s doc
  comment. Measured at Phase 4's close
  ([reports/2026-08-22-phase-4-close-cross-family-review.md](reports/2026-08-22-phase-4-close-cross-family-review.md)
  § 5).
- **`Info` earns two different things across the seven reports.** Posture and
  Restarts document `○` as *the pane makes no judgement*; Drain safety's `Info`
  row (*needs one more flag for N pods*) **is** a judgement — a bare drain
  refuses. Same glyph, one screen. Whether that matters is a question for the
  phase that draws the bands, not a defect in either producer
  ([reports/2026-08-22-phase-4-close-cross-family-review.md](reports/2026-08-22-phase-4-close-cross-family-review.md)
  § 7).
- **The one unglossed sentence on the analysis screen.** Drain safety opens with
  *"A drain below assumes `--ignore-daemonsets`, so DaemonSet pods never count as
  moving"* — a bare flag name on the busiest pane, where every neighbour glosses
  (*"what Kubernetes calls an emptyDir volume"*, *"started by hand, with no
  Deployment behind them"*). The code matches `screens/analysis.md` verbatim, so
  **the decision is what fails [invariant 14](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
  and the wording is `tui-designer`'s to re-take**
  ([reports/2026-08-22-phase-4-close-cross-family-review.md](reports/2026-08-22-phase-4-close-cross-family-review.md)
  § 8).
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
  `kube-system/kindnet and kube-system/kube-proxy were placed here (2 pods)`.
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

- **The `ANALYSIS` sidebar grew a sixth entry and five screen files still draw
  five.** `screens/analysis.md` added `posture`; `alerts.md`, `states.md`,
  `detail.md`, `README.md` and `once.md` all draw the old list. One
  `tui-designer` turn, and it is deliberately **not** taken yet: Family D may add
  `restarts` as a seventh entry, and doing it now edits six files twice. The same
  turn carries the badge glyph rule — *a count draws its band as a glyph, a
  duration does not* — into `screens/widgets.md` § 2, the one place every badge
  on every screen is written down
  ([D128](NOTES.md#d128--the-six-panes-the-one-rendering-of-a-missing-metrics-server-and-the-badge-that-does-not-fit-2026-08-20)).

- **`kind_node_re` is wider than the comment that justifies it.**
  `scripts/sanitize.jq`'s `"k8rs-(control-plane|worker[0-9]*)(\.[a-z0-9-]+)*"`
  allows *any* number of dotted labels, so `k8rs-worker.corp.example.com` is
  accepted while the comment justifies only the single `.lan` the LAN host hands
  out. No leak is constructible from it — the `k8rs-` prefix is this project's own
  cluster name and `k8rs-review-control-plane` is still refused, proven live — and
  `tester` was right to leave it: it is the **shared** regex
  `refuse_foreign_identities` also reads, so it is per-box work
  ([D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)),
  and the one-character tightening `*` → `?` has a capture trip aborting
  mid-redirect as its blast radius
  ([D130](NOTES.md#d130--the-unblock-turn-what-the-export-gap-actually-cost-and-eleven-things-two-agents-settled-that-no-box-had-2026-08-20)).

- **Drain safety's composed sentence has no fixture: the cordoned node carries
  none of the budget's pods.** `broken-pdb-floor`'s two protected pods sit on
  `k8rs-worker2` (NoExecute) and `k8rs-worker3` (kubelet stopped); `k8rs-worker`,
  the node `break-nodes` cordons and N2 is about, carries none — so *cordoned*
  **and** *its drain will never finish because of a budget* is unrepresentable.
  Needs a manifest that lands one replica on the cordoned node, which is a fixture
  design and not a defect. Operator review 2026-08-21,
  [reports/](reports/README.md) ·
  [D132](NOTES.md#d132--the-trip-that-took-four-runs-and-the-sixteen-things-three-agents-settled-under-it-2026-08-21)
- **`kubectl drain` aborts on unreplicated pods before it ever reaches a budget,
  and the pane ranks the two the other way round.** Measured on kind:
  `drain k8rs-worker3 --dry-run=client --ignore-daemonsets` fails on
  `cannot delete Pods that declare no controller` and never touches the eviction
  API. Drain safety ranks *would never finish draining* (PDB) above *pods nothing
  would restart* (unreplicated), which is the inverse of the order kubectl hits
  them. `k8rs-worker3` carries both, so the corpus can prove the ordering matters
  and nothing has decided it. Operator review 2026-08-21
- **The PDB join needs `deletionTimestamp`, because the controller's own counter
  skips terminating pods.** `countHealthyPods` skips a pod carrying a
  `deletionTimestamp` (and the 2-minute `disruptedPods` window); a join on `Ready`
  alone over-counts by one during every rolling update and prints *"has exactly
  5"* where the API server says 4 — on the pane whose credibility is agreeing with
  the eviction API. `PodSnapshot.deletion_timestamp` is already carried; the
  corpus cannot feed the shape (`broken-stuck` is the only terminating pod and
  carries no budget label), so it is a
  [D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
  plant. Operator review 2026-08-21
- **`EndpointSliceSnapshot` is per-slice and has only ever been fed one slice per
  Service.** Past `maxEndpointsPerSlice` (100) a Service gets several and the
  controller can leave one empty, so *is there a slice with 0 endpoints* prints
  *"matches no pod — anything calling it gets a 503"* about a Service with 250
  healthy backends. The right question is the **sum** over every slice carrying
  that `kubernetes.io/service-name` in that namespace; the type supports it and
  [D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
  says it is proven only for the shape it was fed. Operator review 2026-08-21
- **Waste's orphan-claim row is false about a StatefulSet scaled to zero.** Its
  PVCs stay `Bound` and unmounted **on purpose**, so the data survives; nothing on
  disk exercises it (`broken-sts` has no `volumeClaimTemplates`) and
  `ClaimSnapshot` carries neither labels nor `ownerReferences`, so the type could
  not tell them apart. Contained today only because
  [`screens/analysis.md`](screens/analysis.md) gives that row no `→` action.
  Operator review 2026-08-21
- **Nothing detects a mixed-trip corpus.** Today's is provably one trip — every
  pod capture's `creationTimestamp` sits in a 32-second window — but that is an
  observation, not a gate. A `just fixtures` that dies partway leaves some
  fixtures from the new cluster and the rest from HEAD, and `just check` goes
  green if the shapes line up. One assertion over the spread closes it.
  [D114](NOTES.md#d114--the-capture-trip-that-put-four-objects-on-disk-and-the-init-arm-that-is-not-reachable-at-all-2026-08-16)
  states the one-trip property and nothing enforces it. Operator review 2026-08-21
- **A cluster-total allocatable row would print 48 cpu on a 12-cpu machine.** All
  four kind nodes report the host's `allocatable: cpu 12`, so any Capacity row
  that sums allocatable across nodes is counting one machine four times. Per-node
  rows are unaffected, which is why this is a row to refuse rather than a bug to
  fix. Operator review 2026-08-21

- **Only the cordoned worker keeps its pods past `break-nodes`, and one fixture
  out of thirty-eight is on it.** A cordon evicts nothing; `k8rs-worker2`'s
  `dedicated=gpu:NoExecute` and `k8rs-worker3`'s unreachable taint both do — so
  the corpus places 14 pods on worker2 and 12 on worker3 that the cluster deletes
  or un-Readies minutes later, against 10 on `k8rs-worker`. Measured: worker2
  reads **190m over 16 pods** in the corpus and **100m over 3** live. Any fixture
  whose *node* is part of what a report prints has to live on the cordoned worker
  or carry tolerations that survive the sequence; `overhead.json` is the only
  object the rule has been applied to, by `nodeName`. **Drain safety's join is the
  urgent instance**: both `broken-pdb-floor` pods sit in the eviction zone, so
  `currentHealthy` is 2 in the corpus and 0 live, and `snapshot.rs`'s *"draining
  either one is blocked by this budget"* is true of the budget and of neither
  node. A two-replica Deployment cannot use `nodeName`; the one-line answer is
  `dedicated=gpu` plus an unreachable toleration with no `tolerationSeconds` on
  the template. **Ruled a box and not a blocker on 2026-08-21**: the corpus is not
  wrong about what it photographed (both pods Running and Ready at capture,
  `disruptionsAllowed: 0`), no producer reads these fields yet, and pinning one
  more workload leaves the general rule unenforced while buying a fifth trip.
  Operator review 2026-08-21 round 2, [reports/](reports/README.md) ·
  [D132](NOTES.md#d132--the-trip-that-took-four-runs-and-the-sixteen-things-three-agents-settled-under-it-2026-08-21)
- **`spec.unhealthyPodEvictionPolicy` turns Drain safety's `●` into a false alarm
  on the corpus's own budget.** Read off the live v1.36 apiserver: under the
  default `IfHealthyBudget`, a Running-but-not-Ready pod is evictable while
  `currentHealthy >= desiredHealthy` — which `broken-pdb-floor` satisfies at
  `2 >= 2`. So a report that stops at `disruptionsAllowed: 0` prints *"would never
  finish draining"* over a drain that finishes, at CRITICAL, on the pane whose
  promise is *only what is broken*. **No field is owed for the default case** —
  pod `Ready`, `current_healthy` and `desired_healthy` are all carried; only
  `AlwaysAllow` needs the spec field. Report logic with no fixture behind it.
  Operator review 2026-08-21 round 2
- **A pod covered by two PodDisruptionBudgets is refused outright, and nothing
  tests it.** The eviction subresource refuses rather than choosing;
  [`scripts/broken.yaml`](scripts/broken.yaml) already records it and keeps the two
  committed budgets deliberately disjoint, so the corpus cannot show it. Nothing
  is owed at ingest — two selectors, pod labels and namespaces are all carried —
  but it is a refusal path one relabelled workload away, with no object and no
  test. Operator review 2026-08-21 round 2
- **A node at `Ready: Unknown` has every pod deletion-stamped, so Drain safety's
  local-storage and orphan facts vanish on the one node the row exists for.** The
  node controller stamps every pod on an unreachable node once its tolerations
  expire — 16 of 19 on the live cluster — and `a_drain_would_move`'s
  `deletion_timestamp.is_none()` then drops them all, so `local` and `orphans`
  come out `0`. The reader powers the machine back on, drains, and meets
  `cannot delete Pods with local storage`. `a_drain_would_move` is shared with N2,
  so this is a per-box change and not a family one; its doc also cites
  `skipDeletedFilter`, which is inert at kubectl's default
  (`--skip-wait-for-delete-timeout=0`). Operator review 2026-08-21 round 2, § 2
- **Which of several blocking budgets wins a drain row's text is alphabetical.**
  The row's `action` differs materially by branch — *check what X points at*
  against *run one more copy* against *get the pods healthy again first* — and
  position 0 of a `(namespace, name)` sort decides it, so `aaa-pdb-syncfailed`
  wins on an `a`. The sort itself is right for the *list*; using it as a severity
  ranking is [PRIOR-ART § F1](PRIOR-ART.md#f1--sorting)'s class.
  Operator review 2026-08-21 round 2, § 6
- **The pin under *N1 is the only Critical node rule* asserts over one cluster's
  output, not over the rule set.** `not_ready` picks N1 by kind + name +
  `Critical` because a `Finding` carries no rule id
  ([D134](NOTES.md#d134--family-c-the-six-reports-the-frozen-file-they-had-to-move-and-the-two-green-lights-a-review-took-away-2026-08-21)).
  The claim is true today, verified from source. The test builds one cluster, and
  N4 and N5 never fire on it — so a future Critical node rule keyed on a shape
  that cluster lacks lands green and puts its sentence under *would never finish
  draining*. The honest gate is a `scripts/` guard counting `Severity::Critical`
  inside the node-rules region; that is `tester`'s.
  Operator review 2026-08-21 round 2, § 7
- **`N workloads have no memory or CPU limit` counts controllers ∪ bare pods, and
  the pane does not say which.** 42 uncapped pods were 29 group keys and 9
  controllers on the test cluster. Counting a bare pod as its own workload is
  defensible — it is the object you edit — but nothing on the row says so, and it
  carries no detail line to say it in. Second-order: `owner` is the ReplicaSet
  until Phase 5 resolves it up, so this row's answer changes then and that is not
  a regression. Operator review 2026-08-21 round 2, § 8
- **Waste stacks two `NotComputed` rows over an empty pane when exactly two of its
  three lists are unread.** All three unread folds to one row; two do not, because
  the folded sentence names all four lists and would be false about the one that
  *was* read. Closing it needs either a dynamically built ask list — a second copy
  of the three `is_none()` predicates — or a sentence that lies. Tested and
  documented at both ends. Found by the author, 2026-08-21
- **The temporary driver's badge prints `1` without its unit.**
  `screens/widgets.md` § 2's new rule is that a count badge draws its band as a
  glyph because *"a reader who copies `capacity  1` out of the terminal has lost
  what the number was of"* — and `main.rs`'s `pane()` prints exactly that line. It
  is the temporary driver, so it is a nit; the rule is the thing that just got
  written down. Operator review 2026-08-21 round 2, § 10
- **`unhealthyPodEvictionPolicy: AlwaysAllow` can make *would never finish
  draining* wrong.** Read off the live v1.36 API server: `AlwaysAllow` lets every
  running pod be evicted regardless of the budget, so the block itself can be
  false. On no snapshot type and in no prune line. A false red, which is why it
  ranks below the false greens. Operator review 2026-08-21 round 1, § 13
- **`kubectl get pdb -A` cannot show the two fields the new drain branches turn
  on.** The command log's line is the equivalent call and invariant 4 holds, but
  `observedGeneration` and the `DisruptionAllowed` reason are not in that table —
  so the teaching half fails for the stale-counters and `SyncFailed` rows.
  Operator review 2026-08-21 round 1, § 16
- **`drain_row`'s *"One other rule on this node has not caught up either."* is the
  last word-spelled count on the analysis page.** Every other counted paragraph
  now spells the digit; this one was out of scope for the round that fixed the
  blocking budgets' line. Found by the author, 2026-08-21
- **`just mutants-diff` cannot see an untracked product file.** It scopes to
  `git diff HEAD`, which excludes untracked files, so a brand-new `src/*.rs` is
  invisible to the per-turn gate until it is staged. Harmless for a test module;
  a new product file would be a silent gap of exactly the kind
  [D133](NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21)
  and [D134](NOTES.md#d134--family-c-the-six-reports-the-frozen-file-they-had-to-move-and-the-two-green-lights-a-review-took-away-2026-08-21)
  are both about. Found by the author, 2026-08-21
- **`scripts/certs-test.sh` has no `--self-test`.** It grew a second check this
  turn — the two files that pin an instant against the committed certificates —
  and its red was proven on scratch copies rather than by a self-test, which every
  other guard in `scripts/` carries. `tester`'s, raised 2026-08-21

- **`scripts/broken.yaml` cannot produce a non-zero `terminatingReplicas`.** Every
  committed Deployment and ReplicaSet reports `0`, because a draining rollout is a
  window a capture has to land inside. A long `terminationGracePeriodSeconds` plus
  a `preStop` sleep on `broken-rollout`'s old revision would hold the window open
  for minutes and retire the synthesis the rule ships with. `tester`'s manifest,
  and it needs a trip, so it is nobody's until one is owed
  ([D135](NOTES.md#d135--family-b-the-trip-that-already-ran-the-resize-boxs-stale-premise-and-the-shape-a-capture-cannot-catch-2026-08-21)).
  Raised 2026-08-21

- **`settled` reads the policy and not the declared restart rules, so rule 6 and
  rule 15 disagree about one container.** At `restarts != 0`, sitting in
  `state.terminated exit 3` with its own `{Restart, In [3]}` rule: rule 15 stands
  down, rule 6 calls that run the last one on record and hands over a plain
  `kubectl logs` that the next kubelet action falsifies. Reproduced on the built
  binary off `neverrules.json` rewound one restart (operator review,
  `reports/2026-08-21-family-b-restart-rules-and-terminating-replicas.md` § 2).
  The self arm needs no signature change; the sibling arm needs the pod, which is
  [D124](NOTES.md#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20)
  condition 3. A shared helper is a turn of its own — not a regression, and
  [D125](NOTES.md#d125--the-last-run-on-record-is-a-question-about-the-container-not-a-field-and-stateterminated-may-name-a-card-only-where-the-run-is-settled-2026-08-20)
  is where it was promised
  ([D135](NOTES.md#d135--family-b-the-trip-that-already-ran-the-resize-boxs-stale-premise-and-the-shape-a-capture-cannot-catch-2026-08-21)).
  Raised 2026-08-21

- **W2's third arm can say `1 pod not answering` about a pod that is answering.**
  `unavailableReplicas` is `Σ(replicaset.spec.replicas) − availableReplicas`, and
  the minuend is a *desired* number the Deployment controller writes into the
  ReplicaSet spec — it does not know a pod is terminating. So on any drain that is
  not a spec scale-down (`kubectl delete pod`, an eviction, a preemption, a node
  drain) it rises one per leaving pod. Reachable: desired 2, new RS ready 2, an old
  RS whose pod is drained off a node — `ready 2 ≥ 2`, `updated 2 ≥ 2`,
  `Σspec 3 − available 2 = 1` → the card reads `1 pod not answering`. Predates
  Family B, which only added a doc claim that it needs no correction; the claim is
  fixed, the arm is not, and `terminatingReplicas` cannot correct it because it does
  not say which template the pod was on. Operator review 2026-08-21 round 2, § 1
  ([D135](NOTES.md#d135--family-b-the-trip-that-already-ran-the-resize-boxs-stale-premise-and-the-shape-a-capture-cannot-catch-2026-08-21)).
  Raised 2026-08-21

- **`screens/alerts.md`'s two pinned W cards quote a prefix the binary does not
  print.** They show `the controller's own words:`; `controller_said` prints `the
  reason Kubernetes gave:`. Predates Family B — the file disclaims it in place
  (*"rules.rs owns the final wording"*), and `screens-check.py` measures mockup
  widths only, so nothing mechanical sees it. Either the pin or the disclaimer
  should go. Found by `tui-designer` while re-pinning the same file, 2026-08-21

- **A row's own text can now exceed the pane width, and nothing says how it
  wraps.** Every `Row::Answer` before Restarts fitted one line; that pane's rows
  carry `container_fact`'s gloss — `sidecar container proxy (it runs beside the
  app the whole time)` — so a wrap is reachable in normal output for the first
  time. `Row::Answer::text` forbids a `\n` and puts wrapping on `views.rs`, which
  is correct, but no screen states whether a continuation line aligns under the
  glyph or under the text, and `screens/analysis.md`'s own mockup draws it under
  the glyph while every `detail` line indents. Phase 11's question, and the mockup
  is a de-facto answer nobody ruled on. Found by the PM's step-7 pass over Family
  D, 2026-08-22
  ([D137](NOTES.md#d137--family-d-the-restart-row-got-a-pane-of-its-own-and-a-real-cluster-took-four-claims-away-2026-08-22))

### Moved out of a running Phase 5 by the D153 triage (2026-08-22)

*Eight of the ten boxes the PM injected into an open Phase 5. Each is a real
finding and none of them blocks a Phase 5 box, which is the test
[D153](NOTES.md#d153--the-pm-injected-ten-boxes-into-a-running-phase-5-which-is-the-rule-the-pm-was-enforcing-2026-08-22)
set. Read at a phase close like everything else here; the two that stayed say
in their own bodies why.*

- **Nothing observes what k8rs actually puts on the wire — every claim about
  it is read off kube's source.** D147 established the initial LIST pages at
  500 and follows `continue` itself **by reading
  `kube-runtime-4.2.0/src/watcher.rs`**, and the tests synthesise the
  `Init → InitApply* → InitDone` sequence rather than receiving it. A
  **localhost fake API server** answering canned paginated responses would let
  the real `watcher()` run against it and turn four read claims into observed
  ones: that `limit=500` is sent, that the `continue` token comes back, that a
  compacted token restarts the LIST, and that a 403 arrives as
  `InitialListFailed` rather than another variant. It needs `tokio`'s `net`
  feature — **a feature on a crate already present, not a twelfth crate**, so
  invariant 10 is not in question. If boxed: a test drives the real watcher
  against it and each of the four is asserted from what crossed the socket

- **The ingest guard bounds every field and no collection, so the product of
  the two is still unbounded.** `k8s::ingest` caps an identifier at 512 bytes
  and free text at 4096
  ([D146](NOTES.md#d146--the-ingest-guard-two-bounds-off-a-census-a-visible-marker-and-the-newline-a-real-kubelet-sent-2026-08-22)),
  but a pod with 100 000 finalizers costs 100 000 × 512 and every one of them
  is individually legal. Same for `labels`, `tolerations`, `volumes`,
  `containers` and `conditions`. **Deferred deliberately and not overlooked**:
  dropping list entries is a *silent* cut, which is the exact thing
  `… (shortened by k8rs)` exists to prevent, and the box that added the
  per-field bound was asked for a bound per field. So the real
  question is **what a reader is told when a list is cut**, and the field
  answer does not transfer — *"3 of 100 000 finalizers shown"* is a sentence,
  not a marker. Decide that first, then the numbers, and take them off a
  census the way the field bounds were rather than inventing them. Note the
  one place the guard already loses rather than shortens: two labels whose
  keys truncate to the same string collapse into one, first in key order
  winning

- **`no second outbound path` catches only a hostname somebody typed as a
  literal.** Fed the shapes rather than read as a regex: a literal
  `"https://telemetry.k8rs.dev/collect"` is caught, and
  `format!("https://{host}/collect")`, string concatenation, a bare hostname
  with no scheme, and a host handed in at runtime are all **missed** — as is
  an HTTP crate declared under `[target."cfg(unix)".dependencies]`, because
  the dependency walk reads only `dependencies`, `dev-dependencies` and
  `build-dependencies`. So the check **cannot tell the API server from a
  second path**: it passes the kube path not by recognising it as the one
  allowed connection but because that path happens to contain no hostname
  literal. The literal form is the only one refused, and it is the form
  nobody adding telemetry would use. The `[target.*]` walk is cheap and
  should just be done; **an assembled host is not decidable by grep, so the
  honest half is to say what the check covers** rather than let its name
  promise containment it does not have (`tester` wrote the limit into the
  docstring on 2026-08-22 — what is wanted here is the fix, not the disclosure).

- **`tests/binary.rs` is the only test that runs the built binary, and it pins
  two of the nine shapes that binary prints.** It caught the header change —
  whole-stdout literal, so a *wrong* count would have reddened as loudly as a
  removed noun — but only for one input: `healthy.json` is 1 pod, 0 nodes, no
  unread kind, no finding. **Never covered at the process boundary**: the tally
  (`20 critical, 9 warnings`), a card's shape, the unread-kind clause, and
  `--analysis` entirely — `grep -c analysis tests/binary.rs` is **0**, so the
  seven panes have never been printed by a process any test watched. **The
  cheapest fix is one line of machinery that already exists**:
  `a_reader_that_closed_the_pipe_costs_nothing` already runs the whole corpus
  through the built binary and holds the entire report in `whole.stdout`, then
  asserts only exit 0 and a length — pinning that report's first and last lines
  puts the multi-object header and the tally under process cover for nothing.
  Found by `tester`, 2026-08-22

- **Nothing committed exercises the driver's unread-kind branch, and a ruling
  leans on it.** `take()` files Services and CertificateSigningRequests into
  the snapshot, so all 55 fixtures are kinds the driver reads and
  `k8rs tests/fixtures/*.json` prints `55 pods · 4 nodes` with no unread-kind
  clause — the branch is dead over the whole corpus. It is covered whole-line
  by unit tests over `header` with a synthesised pair, so the mechanism works;
  but D121's own example of it went stale unnoticed and
  [D151](NOTES.md#d151--owner-resolution-and-the-noun-collision-that-turned-out-to-be-the-headers-fault-2026-08-22)
  then leaned on it as *the* surviving mechanism. If boxed: a committed fixture of
  a kind no rule reads, so the branch is reachable from the corpus and from the
  binary — or an explicit ruling that a synthesised unit test is enough,
  recorded so the next reader is not the third to trip on it. Also:
  **`width-guard.py` reads `src/` only** (`ROOT / "src"`), so `tests/`,
  `examples/` and `benches/` are outside the 100-column rule — decide whether
  that is intended and say so in the guard

- **W1 draws no card at all on a live cluster for the refusal it exists to
  catch.** Rule W1 reads a ReplicaSet's `ReplicaFailure` condition — *Kubernetes
  refused to create the pods this workload asked for* — and the ReplicaSets it
  is about have `replicas: 0` **because that is what the refusal means**. So no
  pod carries their `ownerReference`, so owner resolution never names them, so
  nothing ever fetches them, and invariant 6 forbids watching ReplicaSets.
  Measured, not reasoned: the only controlling owners any pod in the corpus
  names are `kindnet`, `kube-proxy`, a `Node` and three ReplicaSets — none of
  them the quota-refused one — while `quota-replicasets.json` carries the
  condition with `replicas: 0`
  ([D151](NOTES.md#d151--owner-resolution-and-the-noun-collision-that-turned-out-to-be-the-headers-fault-2026-08-22)).
  The rule passes every test because the file driver hands it a ReplicaSet the
  live path cannot supply. **Two design choices and neither is obviously
  right**: LIST the ReplicaSets of a Deployment that is short of pods — a LIST
  and not a `get`, and arguably the same fetch Waste's `replica_sets` already
  wants — or reconsider W1's kind gate, which exists so that one refusal does
  not draw two cards, the Deployment carrying the same condition. Decide which,
  and say what happens to the *other* card either way

- **`TYPES_BUILT_FOR` is a third copy of the `k8s-openapi` pin and
  `fixture-audit.sh` compares only two of them.** The script already parses
  `features = ["v1_NN"]` out of `Cargo.toml` and compares it with
  `tests/fixtures/K8S_VERSION`; `src/k8s.rs` now carries the same number again,
  as the version k8rs tells users it understands. A test ladder guards two of
  the three ways they can drift apart — pin lowered, constant edited alone —
  and **not the third: the pin raised to `v1_37` on a newer `k8s-openapi`**,
  which would tell every user on 1.37 that their cluster is newer than this
  build, i.e. D99's table stated backwards **in a user-facing string**. The
  author's own argument for the ladder was that raising the pin needs a
  dependency bump a human reads — and the author then withdrew it, because
  `just check` is where drift is supposed to be caught and this is the one
  guard whose green says nothing about the case that matters. If boxed: the script
  greps `const TYPES_BUILT_FOR: u32 = NN;` and asserts equality, its
  `--self-test` gains a row where they disagree **and** one where the file
  fails to parse (which must fail loudly, never pass as agreement — the trap
  D99's own guard fell into), and the ladder test is **deleted in the same
  change**, because two guards for one number is the second copy again.

- **Rule 13 tells a reader their storage and network are fine on any cluster
  that never published the condition — and every `else` over an API `Option`
  is the same shape.** `placed_but_never_started` reads
  `ready_to_start_containers`, and its `else` prints *"this pod has its
  storage and its network, so the block is later — the image is still
  downloading, or the container could not be created"*. An **absent**
  condition takes that branch, so on a cluster below 1.29 — where the
  `PodReadyToStartContainers` condition does not exist in the API types at all
  — the card asserts something the cluster never said, and sends a reader
  whose CNI is broken to look at the image pull. **This is the whole reason
  the supported floor is 1.29**, so fixing it is what would let the floor
  move down ([D149](NOTES.md#d149--the-floor-is-129-because-one-rules-else-turns-a-missing-field-into-a-claim-2026-08-22)).
  The fix is a third arm: say nothing about storage or network when the
  condition is absent. `rules.rs` is frozen, so this is a
  [D124](NOTES.md#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20)
  question — and its first condition is *a defect proven on a committed
  capture*, which this is **not**: every committed capture comes from
  v1.36.1, where the condition is always present. So it needs either a
  capture from an older cluster or an explicit ruling that the API types are
  the object.
  **Half of this closed on 2026-08-22, and it is the half worth *least*.** Rule
  13's own branch is three arms now, and it needed no old cluster after all:
  `unstarted.json` reaches the absent condition on the 1.36 fixture cluster,
  which is [D124](NOTES.md#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20)'s
  first condition met ([D156](NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22)
  ruling 4). **The floor stays 1.29 and this entry stays open for the audit.**

  **And the general form is the part worth more than the one rule.** D99 names
  two ways an old cluster does worse than answer nothing; this is a third —
  **an `else` that treats *absent* as *the negative case is false*, turning a
  missing field into a positive claim.** Invariant 5's *a missing field means
  no finding* does not cover it, because the missing field does not remove the
  finding, it changes the finding's **text**. If boxed — and rule 13's own arm is
  already done, so this is the whole of it: every other `else` over an `Option`
  from the API has been read against the same question, with the ones that are
  safe named so nobody re-audits them

### From the browser's-rows family and its operator review (2026-08-22)

*Everything the family found that is not a defect in the family. Ranked roughly
by what it costs to leave — the first two are reachable from a keypress once
`connect()` lands, and the third has a deadline that is not a phase close.*

- **A kind that can be listed and cannot be watched exists on every cluster, and
  nothing here has a state for it.** Measured: of the 42 resources a bare kind
  cluster advertises `list` on, `componentstatuses` has no `watch`, and the
  server answers `watch is not supported on resources of kind
  "componentstatuses"` — **permanently, not transiently**. `browsable()` filters
  on `list` alone, so the sidebar offers it; a caller then opens a metadata watch
  and gets a 405 forever, and kube's `watcher` carries no backoff, so that is a
  hot loop against the API server reachable by pressing Enter on a sidebar row —
  the security gate's *never retries in a loop* by name. **Not the
  reconnect/backoff box**: that one is about a watch that should work and
  blipped; this one can never work. `Browsable` already carries `verbs`, so the
  data-side check costs nothing; the screen half — a view with no change signal
  fetches once and offers a manual refresh key — is the ruling. Named in
  `k8s.rs`'s doc, not built ([reports/2026-08-22-browser-rows-table-watch-and-refresh.md](reports/2026-08-22-browser-rows-table-watch-and-refresh.md)).

- **`Row::name` is what a kubectl line and every dialog will name, and nothing
  judges it beyond the strip and a 512-byte bound.** `path_safe` guards the URL
  and argues from exactly the right threat — an aggregated API server chooses
  `resources[].resource` — and it chooses `metadata.name` by the same amount. A
  name of `web -n kube-system` renders a command log line reading `kubectl
  delete pod web -n kube-system -n payments`: k8rs does not execute it, but
  [invariant 4](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
  says *neither record may lie*, and that one does. The same string later builds
  the `e` edit temp file path, where `../` is the gate's named case. No command
  log and no `ops.rs` exists yet, so it was named in the doc rather than fixed.

- **The Table is fetched unpaged, and the field that would say *truncated* is
  dropped at the decode — and `k8s.rs` freezes after Phase 6.** `ListParams::
  default().limit` is `None`, so the browser asks for everything in one body: 34 MB
  at 5 000 rows, which is `PRIOR-ART § A2`'s complaint one door along from the
  initial-LIST box that owns it. A Table pages perfectly well — measured,
  `?limit=5` returns 5 rows with `metadata.continue` and `remainingItemCount:
  48` — but `TableResponse` names no `metadata`, so the *showing 500 of 5 048*
  line has nothing to read. **This is the one entry here with a deadline that is
  not a phase close**: after Phase 6 the file is frozen and adding the field is a
  plan correction rather than an edit.

- **Should the browser watch the Table after all?** [D154](NOTES.md#d154--the-browsers-rows-a-37-that-was-one-event-a-floor-measured-from-the-answer-and-a-guard-that-stopped-at-cc-2026-08-22)'s boxed question. The
  measured comparison does not favour what shipped: a Table watch event is ~3 062
  bytes and already carries the row identity, a metadata event is ~2 624 bytes
  **plus** a whole re-fetch at 6 852 bytes per row. What shipped is defended by
  what kube gives the metadata path for free — `resourceVersion` bookkeeping, the
  410 relist, the init event — and a Table watch is a hand-rolled
  `Client::request_stream` owing all three. Real work either way; nobody has
  ruled which is less.

- **`path_safe` sits at the sink and the stronger placement was refused for a
  reason worth revisiting.** Putting it in `browsable()` would keep a hostile row
  out of the sidebar entirely, but it reversed two behaviours the discovery box
  proved — a CRD naming itself with control characters is *offered with its name
  stripped*, and a runaway plural is *offered shortened* — and neither survives
  the predicate afterwards. The PM ruled the sink is enough (the row degrades to
  *cannot fetch* rather than exploiting anything). The question is whether
  *offered but unopenable* is the right screen.

- **A Table with no `priority: 0` column renders nothing.** Fed `columns=[A(1),
  B(1)]`, the drawn output is `"\n"`; same for zero `columnDefinitions` with rows
  present. Built-in printers and the CRD table convertor always emit `Name` at 0,
  but an aggregated API server writes its own Table. The screen needs a rule for
  *the server gave me no narrow columns*.

- **`kubectl get pods` with no `-n` is not what k8rs will have done.**
  `Fetch::table(kind, None)` is `/api/v1/pods`, every namespace; the honest
  command log line is `kubectl get pods -A`, because plain `kubectl get pods`
  uses the kubeconfig's current namespace. A trap for the command-log box, and
  invariant 4 again.

- **The fixture that would turn one test's claim into invariant 12 proper.**
  `assert_ne!` over two captures proves the columns are *per-kind*, which a
  hand-written map would also give; what proves there is no map is a `Table` of a
  kind no built-in printer knows. One namespaced CRD with
  `additionalPrinterColumns`, captured with the same Accept header, on the next
  capture trip — it is also the only shape that exercises a column header written
  by somebody outside the control plane.

- **`unprintable` has two disposals and they differ.** `text` turns a removed
  *whitespace* character into a space and bounds the result; `main.rs`'s
  `sanitize` removes it. So on the fixture path a `\n` inside a cluster-sent
  message glues two words together where the ingest path leaves one space. The
  real fix is that `load` should go through `Store` rather than straight through
  `rules.rs`'s `From` impls — a plan question, named in [D154](NOTES.md#d154--the-browsers-rows-a-37-that-was-one-event-a-floor-measured-from-the-answer-and-a-guard-that-stopped-at-cc-2026-08-22)'s third section.
  **And there is a third spelling, in a test**: `k8s_tests.rs:1547` and `:2438`
  assert `!chars().any(char::is_control)` — true, weaker than the guard beside
  them, and exactly the shape that let the narrow word come back. Not a hole
  (their siblings feed the wider set); a widening nobody would notice.

- **`bounded_impl` reads an impl body as raw text**, so a field named only in a
  *comment* inside the body would satisfy `every_string_the_browsers_rows_keep_is
  _named_by_the_ingest_guard`. `Row`'s and `Column`'s impls carry no comments
  today, so it holds; `ObjectId`'s does. Pre-existing, found while attacking this
  family.


### From the whole-project review (2026-08-22)

*Three reviewers read the repo as one thing — an operator over the product files,
a test engineer over the suite and the guards, an outside reader over the process.
The two blockers they found were false ticks and went back onto their own boxes;
these are everything else
([D155](NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22)).
**The first two are marked `[before Phase 5 close]` because that is when they
stop being cheap.***

- **`[before Phase 5 close]` `just mutants` sweeps two files and Phase 5 writes a
  third.** `justfile`'s recipe is pinned to `--file src/rules.rs --file
  src/analysis.rs`. Measured with `cargo mutants --list`: 604 + 245 swept, **132
  in `k8s.rs` and 61 in `main.rs` never swept**. Phase 5's entire product is
  `k8s.rs`, so its close would run a green whole-file sweep over two files the
  phase did not touch. `mutants-diff` covers each turn's diff, so this is a
  phase-close hole and not a per-turn one. One word in `tester`'s file.

- **`[before Phase 5 close]` `cargo mutants` generates nothing for a `const`, so
  every threshold is outside the gate.** Measured: zero mutants naming
  `RESTARTS_WARN`, `NOT_READY_GRACE`, `PROBE_FLOOR` or `SKEW_ALLOWANCE` in 604.
  There are **19** numeric and duration `const`s across the four product files
  and **6** are pinned by a literal `assert_eq!` naming the constant and its
  number: `RESTARTS_WARN`, `RESTARTS_CRITICAL`, `CERT_EXPIRY_WARN`,
  `INITIAL_LIST_PAGE`, `SUPPORTED_SKEW`, `TYPES_BUILT_FOR`. The other **13** —
  `NOT_READY_GRACE`, `NODE_DOWN_GRACE`, `NEVER_JUDGED_GRACE`, `OVERDUE_MARGIN`,
  `PROBE_FLOOR`, `SKEW_ALLOWANCE`, `FREE_TEXT`, `IDENTIFIER`, `REFRESH_FLOOR`,
  `OLDEST_SERVER`, `UNREADABLE_SECTIONS`, `MOST_ROWS_PER_SECTION`,
  `NAMESPACES_NAMED` — are constrained only by test timings *derived from the
  constant symbol*, which move with it when somebody "fixes" a number. **The
  review's own first count of this was wrong in both figures** (20 and 7): two
  of the claimed pins turned out to be an `assert_eq!` on `seen.len()` and one
  on `scheduled.status` that merely have the constant nearby — which is the
  same shape as the defect, one level up. The gate's own box
  discloses that it cannot mutate a struct literal's field; it does not say *and
  never a `const`*, and that is the larger class.

- **Drain safety says *waits forever* about a rollout that finishes on its own.**
  The *at its floor* arm fires on `current_healthy >= desired_healthy`, which is
  true both of 2 replicas at `minAvailable: 2` (blocked forever) and of 3
  replicas at `minAvailable: 2` with one pod not Ready this second (blocked for
  seconds). `status.expectedPods` separates them, it is **already in the
  committed capture** at both values 2 and 3, and `DisruptionBudgetSnapshot` does
  not decode it. The action bites hardest: *run one more copy* when a third copy
  already exists scales to 4 or lowers a correct budget. `rules.rs` refuses this
  exact shape one file over — a point sample of a transient deciding what the
  user sees — and wins there.

- **An OOM-killed container above three restarts draws two `Critical` cards.**
  Rule 2 says *raise the memory limit*; rule 5, on the same container, says *read
  the last run's log*, which for a `SIGKILL` ends mid-line and holds nothing. The
  suppressors cannot fold them — different actions, and rule 5 is `Reads::Now`.
  The rule 2 + rule 15 pair is already an entry above; this is the commoner pair
  and was not recorded.

- **Drain safety's `not_ready` row keys on severity and kind, not on the rule.**
  It finds any `Critical` finding whose object is this node and then prints *this
  node has stopped responding* as a fact. Correct only while N1 is the only
  `Critical` node rule. The entry above records that the pin asserting that rests
  on one cluster's output; this is the consumer that breaks when it stops holding,
  and it breaks by asserting the wrong sentence rather than by drawing nothing.

- **Waste says a Service with no endpoints gives its callers a `503`.** A
  ClusterIP with no endpoints is an iptables/IPVS `REJECT` — the caller sees
  *connection refused*, or an empty DNS answer for a headless Service. A `503`
  comes from an Ingress controller, a mesh sidecar or a gateway, a layer this row
  knows nothing about, and at 3am the word decides where the reader looks.
  **Reasoned from the mechanism, not measured** — it needs a client pod on a live
  cluster to settle, so it is an entry and not a finding.

- **`check-docs.py` reads markdown only.** A planted `NOTES § D999` inside a
  `.rs` file leaves it green; there are **1,636** `NOTES § D##` citations in
  `src/` and `tests/` (1,867 counting the bare `§ D##` form), and all of them
  resolve today. It matters because the assertion
  *message* is where a reader decides whether an expected number was derived from
  a requirement or transcribed from output, and nothing checks the citation in it.

- **`k8s_tests.rs` writes the five-watch `drive(vec![one_watch(…), …])` list
  inline three times** while `streams()`, `bootstrapped()` and `all_but()` already
  exist. A normalized-block scan over all thirteen test files found it as the
  only ≥8-line block repeated three times or more, which is worth stating in
  both directions: § *Write function-based* looks broken here and nowhere else
  in 39,620 lines. That scan is the reviewer's and was not re-run here.

- **The read-only role's over-grant is the other half of a box that is already
  open.** `docs/security.md` grants cluster-wide `list configmaps` and
  `list events`; no product code reads either, and the test suite uses `ConfigMap`
  as its example of *a kind nothing reads*. `list configmaps` cluster-wide is not
  neutral — they routinely hold what should have been Secrets. Named here only so
  it is not lost: it belongs to **Phase 5's own unchecked `ClusterRole` box**,
  which already carries the under-grant half.

- **The rule 8 `/` entry above is missing its worst half:** the `kube-system` +
  DaemonSet exemption does not save `prometheus-node-exporter` either, so there is
  no namespace an operator can move it into to silence the card — and the card
  prints `read-only` on its own evidence line under a `Critical` title.

- **Nothing shrinks, and the file that says so grew 19%.** Since D103: `NOTES.md`
  +60%, `CLAUDE.md` +19%, the four prose files +61% together. 89% of every
  markdown line ever written is still in the repo, against 37% of Rust deleted.
  Three deletions, ranked by lines removed per unit of risk: **(1)** the doc
  comments in `rules.rs` (62%) and `k8s.rs` (63%) cut to the contract `CLAUDE.md`
  already states — comments compile to nothing, so the suite is a complete
  regression check, and the longest single run is 166 lines re-arguing five
  decisions it also links; **(2)** a 60-line cap on a decision body, with the
  round-by-round narrative going to `reports/` where D108 already put it — 41
  decisions are ≥100 lines and 17 are ≥150, against a median of 67; **(3)**
  `chore(changelog): update` moved to phase close, which is **101 of 253
  commits** regenerating a 168-line file.

- **`NOTES.md`'s heading tree is fiction and only one break is guarded.**
  D1–D133 are nested under a `##` dated to one day of design work in 2026-08-11;
  D134–D155 are nested under `## Inspiration / reference tools`. D154's three
  subsections escaped to `##` in `21f85a9` and were re-nested by hand in the
  change that wrote this entry — `check-docs.py` could not see it, because it
  matches a decision only at `level == 3`. Two things, in order: one real
  `## Decisions` section, then ~10 lines in the guard requiring a `### D##` to be
  its immediate child.

- **Waste counts every `Failed` pod that is not `Evicted` as a finished Job, and
  two of those reasons arrive in hundreds.** `OutOfcpu` / `OutOfmemory` /
  `OutOfpods` / `OutOfephemeral-storage` (`pkg/kubelet/lifecycle/predicate.go`)
  are the kubelet refusing a pod the scheduler already bound — *a node that
  literally ran out of room*, being told *"Kubernetes keeps a few finished Jobs by
  default, so some of this is normal"*. `Terminated`
  (`nodeshutdown_manager.go:88`) is every managed node-pool upgrade and every spot
  reclaim; a 30-node rolling upgrade leaves hundreds. Both read as CronJob
  leftovers today. A third pileup row was refused as *not this box*
  ([D158](NOTES.md#d158--the-waste-boxs-second-half-and-the-jargon-translation-that-was-wrong-in-this-file-first-2026-08-23));
  the alternative is narrowing the completed row's sentence to what it can prove —
  the Job sentence over `Succeeded`, a neutral one over `Failed`
  ([reports/2026-08-23-waste-evicted-row-operator-review.md](reports/2026-08-23-waste-evicted-row-operator-review.md) § 5).

- **Waste's removed-pods row could name the node when there is exactly one.**
  `PodSnapshot::node` is already carried and `listed()` is already `pub(crate)`,
  so `1 pod was removed by k8rs-worker and remains` costs nothing new and answers
  half of what its action reaches for. Refused for the general case, measured
  rather than assumed: real node names are `ip-10-0-1-23.ec2.internal` and two of
  them through `listed()` blow the pane's 53-column content budget (D158,
  same report § 9).

- **Waste's disk row counts a finished pod as a mounter, and one of those pods is
  now visibly dead on the same pane.** Deliberate and documented — a `Succeeded`
  CronJob pod is evidence something mounts the claim every run — but an evicted
  pod under `restartPolicy: Never` with no owner will never mount anything again,
  and since D158 the pane prints `N pods were removed by a node` two rows under
  the claim that their disk is in use. The argument for the row (never push a
  reader at deleting a volume) still holds; what is new is that one screen now
  asserts both (same report § 8).

- **The eviction the corpus does not hold is the node-pressure one.** Every claim
  about that half of `status.reason: Evicted` rests on upstream source and on the
  *absence* of a `DisruptionTarget` condition from `evicted.json`, not on an
  object. The capture that would settle it: a pod evicted while a node's
  `MemoryPressure` is `True`, carrying `The node was low on resource: memory.` and
  a `DisruptionTarget: True / TerminationByKubelet` condition. With it the row
  could say *which* mechanism instead of naming both — which is the only reason
  `status.message` would be worth decoding (D158, same report § 1).


### From the rule 13 family review (2026-08-22)

*Two findings from [`reports/2026-08-22-rule-13-family-review.md`](reports/2026-08-22-rule-13-family-review.md)
that are not defects in the box that produced them. Read at the next phase close.*

- **A node whose `Ready` flaps faster than five minutes gets no N1 card and a
  blinking rule 13 card, and closing that means one rule reading another's
  clock.** Measured on the review cluster: a condition's `lastTransitionTime` is
  rewritten on **every** flip — `Unknown` at `14:24:24Z`, `True` at `14:26:37Z`,
  `Unknown` again with a fresh stamp at `14:27:29Z`. N1's grace runs from that
  stamp, so a node flapping under `NODE_DOWN_GRACE` never reaches it, while rule
  13 stands down on every `Unknown` phase and fires on every `True` phase
  ([D156](NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22)
  ruling 2 is what couples them). The ordinary producer is a kubelet missing
  heartbeats under memory pressure — not exotic. **And the honest form of the cost
  is sharper than the doc comment's**: `tester` ran the binary at a node two
  minutes into `Unknown` with the pod bound 42 minutes earlier, and the screen
  printed **`nothing is broken`** — the one claim
  [`screens/once.md`](screens/once.md) says has to be true, and the exact sentence
  [D155](NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22)
  re-opened this box over. Still strictly better than before, when the shape drew
  nothing ever. **Neither obvious fix is free**:
  suppressing the blink means rule 13 reading N1's clock, and dating the flap
  means the snapshot carrying a condition's *history*, which it deliberately does
  not. There is a third reading — that a flapping node is its own finding and
  neither rule should own it — and that is a new rule, so [invariant
  13](CLAUDE.md) applies. Decide which; the code change is small once it is
  decided

- **`src/analysis_tests/restarts.rs:459`'s comment claims the assertion proves
  something the assertion cannot see.** It says *"The two runs began three seconds
  apart … each row measures its own"*, and the assertion is a loop requiring both
  rows to produce the **same** string. Three seconds render identically at every
  granularity the renderer has, so it cannot tell "each row measures its own" from
  "both read the same one". Pre-existing, untouched by the repin that made it
  visible — true at `1 hour ago` and true at `2 days ago`. If boxed: either the
  fixture gains two runs far enough apart to render differently, or the comment
  stops claiming what the loop does not check

- **`break_nodes`' binding step has no offline test, and the only honest shape for
  one is a new file.** Every other predicate in `cluster.sh` is proved offline by
  `scripts/verify-test.sh`; the binding block is not, because proving it needs a
  stub `kubectl` that answers `get`, `create --raw` and a 409 — a second
  implementation of the API server's semantics. `tester` built one in a scratchpad
  to prove the missing-pod fix and it **lied twice inside twenty minutes** (a 201
  for a binding whose pod does not exist; a `-o jsonpath` match served from its
  `-o json` case), each lie producing a confident wrong red. That is the argument
  for the box and against a quick one: a committed stub drifts the same way with
  nobody watching. If boxed: a `tester`-owned `scripts/break-nodes-test.sh` with
  the stub beside it and a line in `guards.sh`, and the stub's own dishonesty is
  what its `--self-test` has to catch first. What holds today is the capture trip
  itself, which is a real cluster and runs once a phase

### From the Phase 3 re-close review (2026-08-22)

*Two findings from [`reports/2026-08-22-phase-3-reclose-family-review.md`](reports/2026-08-22-phase-3-reclose-family-review.md)
that are not defects and not docs sync. The first has a deadline that is a phase,
not a phase close.*

- **Rule 13's new evidence is four lines against a three-line cap, and the line
  the cut would take is the diagnosis.** Measured at 49, 51 and 53 columns:
  *"on node X · the machine has written nothing at all about this pod: not one of
  its containers has a status, not even a failed attempt, **so nothing there has
  picked it up**"* — and it is that last clause, the one that turns an absence
  into a finding, that falls off a three-line cut. **No cut exists yet**:
  `main.rs` prints evidence whole, so nothing is wrong today and this is a
  **Phase 8 deadline**, when the pane that cuts arrives.
  [`screens/alerts.md`](screens/alerts.md) § the evidence cap justifies cutting
  because an evidence line carries an unbounded API quote — and this one quotes
  nothing, which is the question to answer before the cut is written: either the
  cap is about quoted text and this line is outside it, or the evidence is
  reworded so the diagnosis survives three lines.
  **`tui-designer` has already answered the design half and the answer is
  narrow**: `screens/alerts.md` § the evidence cap argues the cut by *contrast* —
  every author-written part (title, action) is never cut, because an author can
  measure and shorten their own sentence, and the evidence alone is cut because
  it carries a controller's sentence quoted verbatim that no author bounds
  ([D37](NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)). Rule 13's
  new evidence quotes nothing and is bounded, so **a straight `…` cut is the
  wrong tool** — it would reclassify an author-written sentence as unmeasurable.
  What is left to decide is which of the two: author-shorten it to three lines
  like every other bounded field, or give the cap a second, wider budget for the
  author-written case

- **`scripts/mutants.sh`'s three log scans read `mutants.out` unconditionally, so
  a run that never got the lock reports another run's numbers.** Reproduced by
  accident during the re-close review: an invocation that tested **zero** mutants
  printed *"180 log(s) read"* and *"18 unviable"*, all of them `analysis.rs`, from
  a sweep another process was running in the same tree. cargo-mutants only rotates
  its output directory once it holds the lock. **The exit status is unaffected**
  (`exit $rc`), so nothing has ever passed on this — what it corrupts is the
  human-readable line the script exists to make trustworthy, which is the whole
  point of [D133](NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21).
  **Seen a second way an hour later, which is what makes it a box rather than a
  curiosity**: `just mutants-diff` over a diff that contains **only** a
  `#[cfg(test)]` module printed `INFO No mutants to filter` and then thirteen
  `src/rules.rs` unviables from a shard of the whole-file sweep that had ended
  twenty minutes earlier. Exit 0 both times. The recipe already refuses an
  *empty* diff for exactly this reason — nothing to test reads like nothing got
  past — and does not refuse a diff with **no mutants in it**, which is the same
  sentence. If boxed: the scans run only when this invocation owned the lock and
  actually tested something, the stale output directory is cleared or
  timestamped, and the `--self-test` gains both cases — a run that never held the
  lock, and a run that held it and tested zero — each of which must print
  *nothing read*, never a count

### From the Phase 4 re-close review (2026-08-23)

*Two findings from [`reports/2026-08-23-phase-4-reclose-family-review.md`](reports/2026-08-23-phase-4-reclose-family-review.md)
that are not defects in any checked box. Both are in `analysis.rs`, which freezes
at this close, so both need a ruling before they can be written — the same
position the four findings above them are in
([D159](NOTES.md#d159--the-phase-4-re-close-and-the-three-counts-that-only-a-close-re-takes-2026-08-23)).*

- **The Restarts comparator ties on the joined `namespace/name` string, which
  this family already ruled against once.** `analysis.rs`'s Restarts sort ends
  `.then_with(|| qualified(&a.pod.id).cmp(&qualified(&b.pod.id)))`, while
  `drain_row` records why the budget list stopped doing exactly that: `'-'`
  (0x2D) sorts before `'/'` (0x2F), so `team-a/api` comes out ahead of
  `team/web` while `kubectl get -A` prints `team web` first. Waste's two
  per-object sections and the budget list all key the `(namespace, name)` tuple;
  this one comparator does not. **Reachable rather than theoretical** — `Time` is
  second-granular, so two pods in different namespaces with the same restart
  count that came back in the same second land on this tie-break, which is what a
  node reboot across a DaemonSet produces (D137 measured a reboot taking a set
  from 6 to 17). Cost is a pane whose order differs from the reader's own
  `kubectl`, which is the one thing this pane is read beside. If boxed: key the
  tuple, and the assertion is two namespaces whose names straddle `'/'`
- **`capacity` and `drain_safety` are both O(nodes × pods) by construction** —
  `pods_on` scans every pod once per node, and `node_overcommitted` scans them
  again. **This is an algorithmic reading, not a measurement**, and the review
  says so: at 5000 pods across 200 nodes the seven reports added nothing
  detectable (0.531/0.499/0.507 s against 0.517/0.563/0.488 s without), and the
  larger shapes it tried varied 1.6→4.0 s run to run under a load average of
  18.80 from the mutation sweep, so those numbers were discarded rather than
  reported. It is in no checked box. If boxed: measure it on a quiet machine
  first and only then decide whether an index earns its complexity — Phase 5's
  store is where a node→pods map would live, not here

### From the capability-probe operator review (2026-08-26)

*Every entry measured against an ephemeral cluster —
[reports/2026-08-26](reports/2026-08-26-capability-probe-group-strings.md) —
and ruled in [D160](NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26).*

- **C4 has a capability and no permission, so an empty list would read as
  *healthy*.** `docs/security.md` omits the `cert-manager.io` grant from
  `k8rs-readonly` **deliberately**, and the probe answers *present* off the CRDs
  alone — so on a cluster that really runs cert-manager, C4's every list is a
  403 over a row the screen was told to draw. `Capability::CertManager`'s doc
  already says a 403 is the feature's to report; the C4 box has to actually
  report it rather than render zero findings. C4 has no phase yet, which is why
  this is here and not a box.
- **Nothing runs rustdoc, so a doc link rots unseen.** `just check` has no
  `cargo doc` step, and `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo
  doc --no-deps --document-private-items` reports three already broken, all in
  frozen files: `crate::rules::in_days` (`analysis.rs:3028`),
  `crate::analysis::capped` (`rules.rs:973`) and `Row::NotComputed`
  (`rules.rs:1812`). This repo writes more doc comment than code and cites items
  by path throughout, so it is the one gate whose absence is invisible — and
  `check-docs.py` covers markdown anchors only. Found while proving the
  capability probe's own new links resolve, which nothing else would have done.
  `tester`'s, and forward-only says the three existing ones are not this box's.
- **`kind delete cluster` was refused four times by the session's permission
  system**, so the review's teardown went through `docker rm -f
  <name>-control-plane` plus `kubectl config delete-context/delete-cluster`.
  Verified clean, and the fixture cluster was untouched — but a measurement
  brief should say which teardown path is expected rather than leaving the
  agent to find one under time pressure.
- **`▲` is `Severity::Warn`'s glyph and the watch-health lines reuse it for a
  second axis** (2026-08-27). `screens/resources.md:10` draws `ALERTS 3 ● 7 ▲`
  counting *findings*; the temporary driver's health lines print `▲`/`●` for
  *tool* health. When `views.rs` lands, either those lines inflate the finding
  count or one glyph means two things on one screen
  ([screens/README.md](screens/README.md)). `tui-designer`'s to settle before
  Phase 11 draws the Alerts header. The terminal `ended` line already took `●`
  by a PM ruling, which is the severity right but not the axis. Found by the
  operator review of `connect()`, which also noted the case any reordering must
  not break: the health lines print *above* the counts and the verdict, and that
  ordering is what makes `○ nothing is broken` read as qualified rather than
  asserted while five watches are down.
- **`ResetTimerBackoff`'s 120-second recovery is not unit-tested** (2026-08-27).
  `tokio::time::pause`/`advance` need tokio's `test-util` feature, which is not
  enabled, so `StandingBackoff`'s recovery half rests on kube's own tests plus an
  assertion that we still delegate to them
  ([NOTES § D165](NOTES.md#d165--the-two-cargotoml-lines-the-first-client-forced-and-the-one-that-was-a-panic-on-every-machine-2026-08-27)).
  Refused for now because the operator review **observed the timer firing on a
  live cluster** — twelve minutes after the plateau, a fresh outage restarted at
  the bottom of the ladder — so the property is measured even though it is not
  pinned. One word on the existing `[dev-dependencies] tokio` line if it is ever
  worth pinning.
- **`scripts/security-guard.py` has no rule against `src/` binding a socket**
  (2026-08-27). Nothing in k8rs should ever listen; the existing outbound rule
  only matches literal hostnames, so the discovery fallback's stub server —
  `TcpListener::bind("127.0.0.1:0")` with a `format!`-built URL — passes it, and
  so would a real listener. `tester`'s. Found while proving that stub was
  allowed to exist.

- **A `403` from a proxy that answers JSON reads as *nothing usable came back***
  (2026-08-27). Every field of `Status` is `#[serde(default)]`, so any JSON
  object body deserializes successfully into an all-default `Status` and kube's
  `with_code` fallback never runs; the HTTP status is then unrecoverable from
  `kube::Error::Api`, which carries the parsed `Status` and nothing else.
  Measured through the binary — the same `403` with a `text/plain` body
  classifies correctly. oauth2-proxy, an auth-annotated ingress and an API
  gateway all answer JSON. **The only route is a `ClientBuilder::with_layer`
  above the transport** that rewrites such a response before kube parses it,
  which no box has claimed and which is machinery, not a fix. Stated in
  `answer()`'s doc and pinned by a test rather than claimed away
  ([NOTES § D167](NOTES.md#d167--eight-faults-not-two-and-the-two-the-review-had-to-produce-2026-08-27)).
- **`cargo doc` has three broken intra-doc links and nothing runs it**
  (2026-08-27). `cargo doc --document-private-items` reports
  `src/analysis.rs:3028` (`crate::analysis::capped`), `src/rules.rs:973`
  (`crate::rules::in_days`) and `src/rules.rs:1812` (`Row::NotComputed`). All
  three are in **frozen** files and all three are pre-existing; `just check` does
  not run `cargo doc`, which is why they survived. Adding the step is `tester`'s
  and fixing the links is a [D124](NOTES.md#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20)
  question, so the two do not land together. Found by `dev-core` while checking
  its own new doc links.
- **A routine `410` watch desync prints *"nothing usable came back"* for a
  second** (2026-08-27). `watcher.rs:610-622` emits `Err(WatchError(status))` for
  a stale `resourceVersion` and then re-lists; that `Status` carries
  `reason: "Expired"` (`kube-core/src/response.rs:390`), which `answer()` matches
  on neither its code arms nor its reason arms. It clears on the next `InitDone`,
  so it is a wrong sentence and not a wrong state. Read off the two match arms —
  producing it needs etcd compaction or watch-cache eviction on demand, which
  `k8s-admin` could not do.
- **One credential fault is reported seven times** (2026-08-27). Under a
  mid-session 401 an operator gets two greeting clauses plus one line per watch,
  all saying the same thing, and the whole report reprints as each lands — five
  blocks in the first four seconds, measured. Correct per watch, unreadable as a
  screen. This is `views.rs`'s header and belongs to Phase 11; named here so it
  is boxed rather than discovered there.
- **The Waste pane names a working `ExternalName` Service and tells the operator
  to delete it** (2026-08-27). `analysis.rs:1798` selects on
  `!service.selector.is_empty()` and nothing else. A `type: ExternalName` Service
  with a leftover `spec.selector` is accepted by the API server, the
  endpointslice controller creates **no slice at all** for it, and the row fires:
  *"This Service points at nothing. Anything calling it gets a 503. → fix its
  selector, or delete it"*. Measured on a live cluster through the built binary —
  `spec.clusterIP` is **empty**, so there is no address for anything to get a 503
  *from*, and deleting it breaks whatever resolves through the CNAME. This is
  what a `type:` change from `ClusterIP` leaves behind, which is how it reaches a
  real cluster. **Not a one-line fix**: `ServiceSnapshot` (`rules.rs:1583`)
  carries `id` and `selector` only, so it needs `spec.type` on the snapshot plus
  a prune-line change in **frozen** `rules.rs` — a
  [D124](NOTES.md#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20)
  question and a box of its own. Pre-existing: the filter is byte-identical
  before and after the join was rewritten.
- **The Waste join's speedup is not reachable through `--live` yet, so the number
  that decides a budget has not been taken** (2026-08-27). `k8s.rs:1515` sets
  `endpoint_slices: None`, so this row is `NotComputed` on a real cluster and all
  three timing runs went through the file driver. The ordering is right — the
  join is fixed before the fetch makes it hot — but the measurement worth
  budgeting against is the one taken *after* the on-demand fetch lands, with
  watch and prune cost in it
  ([reports/2026-08-27](reports/2026-08-27-endpoints-behind-join-and-growth.md)).
- **`endpoints_behind` counts endpoints, which is `pods × address families`**
  (2026-08-27). Measured on a dual-stack cluster: one Deployment, one replica,
  `ipFamilyPolicy: RequireDualStack` → two slices (IPv4 and IPv6), and the join
  returns **2** for one pod. Correct as written and harmless while the only
  reader is `== 0`, but every slice in `tests/fixtures/endpointslices.json` is
  IPv4, so the corpus cannot hold the shape, and the first consumer that reads
  this `usize` as *pods behind the Service* inherits
  [PRIOR-ART § F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended).
- **`scripts/reports-guard.py` classes cluster DNS as a machine hostname**
  (2026-08-27). It refused a report line containing `*.svc.cluster.local` under
  "a hostname". A cluster-internal DNS name is never a machine identifier, and
  any report describing DNS behaviour hits it. `tester`'s. Found by `k8s-admin`
  writing up the join measurement.

### From the Posture node-infrastructure box and its operator review (2026-08-27)

- **The all-`kube-system` writable sentence still concludes where every other string
  now reports.** *"Kubernetes runs its own node agents this way"* (`analysis.rs`,
  `Mounters::sentence`) asserts what a pod *is* from the same narrow check
  [D168](NOTES.md#d168--posture-sorts-the-row-it-cannot-vouch-for-first-and-says-the-check-instead-of-a-verdict-2026-08-28)
  rewrote every other sentence to stop doing. It is not wrong today — `left_by_rule_8`
  guarantees the writer cleared the check — and it is a reassurance rather than an
  accusation, which is why it was left. Ruled: wrong-and-quiet in
  [D70](NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)'s
  sense, sweep it when D70 itself is widened. Raised by `tui-designer`.
- **The densest Posture detail sentence is now 155 characters where it was 44**, which
  is five lines at `screens/analysis.md` § Posture's 40-column detail budget, and the
  section's mockups only draw the three-line case. Nothing wraps yet because `views.rs`
  does not exist, so nothing is wrong today; it is whoever draws the pane who inherits
  it. Raised by `tester`.
- **Nothing in the sidebar points at a pane that now says something is worth a look.**
  Posture badges nothing, deliberately and for a reason that has not changed
  ([D127](NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)),
  so the box's value only reaches someone already standing on the pane. Raised by
  `k8s-admin` as a tension, not a finding — a badge is the obvious answer and is the
  one D127 refused.
- **D70 fires wrong-and-loud on Alerts too, and this box could not touch it.** With
  kindnet in `calico-system`, its *writable* mounts (`/etc/cni/net.d`, `/var/run/nri`,
  half of `/run/xtables.lock`) leave Posture and become rule 8 CRITICAL cards. That is
  D70's recorded limit on the other screen; it needs `rules.rs`, which is frozen.
  Measured by `dev-core` while proving the Calico render.

### From the live-fields box and its attack (2026-08-28)

- **C1's card says *"this is the file on your own machine that proves who you are"* about a
  file the connection may never have touched.** When a kubeconfig's `exec` plugin returns
  `clientCertificateData`, kube takes the plugin's identity and never calls `identity_pem()`,
  so `client-certificate` is read by k8rs and by nobody else
  (`kube-client-4.2.0/src/client/config_ext.rs:391`, measured on the binary by `tester`).
  Ruled 2026-08-28: **k8rs keeps reading it** — an exec plugin returning a *token* does fall
  through to `identity_pem`, so the certificate genuinely is the TLS identity there, and a
  missed expiry is worse than an over-broad card. What is left is the card's *framing*, which
  is `rules.rs` and frozen. Sweep it with the C-series wording.
- **A context name has no length bound but `IDENTIFIER`'s 512**, and it is the first object
  name on a card that is not a Kubernetes object name — the API caps its own at 253, a
  kubeconfig context is whatever the user's file says. A 304-character name drew a 306-column
  card line. Control characters are stripped and nothing crashes; it is a `views.rs` question
  from Phase 8 on. Found by `tester`.
- **`--context` without `--live` still prints errno jargon about a file nobody named** —
  `k8rs --context prod` → `k8rs: --context: No such file or directory (os error 2)`, because
  `run()` filters only `--analysis` out of `paths`. Invariant 14's exact shape, arriving
  through the door `mistyped`'s own doc says it closed. **Pre-existing** — verified against
  `git show HEAD:src/main.rs` — and not caused by the live-fields box. Found by `tester`.
- **The one MISSED mutant on `connect_with`'s `client_certificate` has a route that was
  refused, not one that does not exist.** An `exec` plugin emitting an ExecCredential with a
  key generated at test time closes it and commits no key material. Refused 2026-08-28 because
  it puts `openssl`-on-PATH into `cargo test`, and *`just check` is the whole of CI or it is a
  lie*. Reopen only if the toolchain requirement stops being one. Found by `tester`.

### From the clock-skew screen spec (2026-08-28)

- **Nothing decides whether a zero severity count is omitted or printed as `0` in the sidebar
  badge.** The clock-skew mockups first drew `│▸ ALERTS     1 ●    │`; every badge drawn
  anywhere else in `screens/` is either blank or `3 ● 7 ▲`, so it was a new shape and was
  re-drawn out of that box on 2026-08-28
  ([D176](NOTES.md#d176--the-clock-skew-line-does-not-fit-in-the-header-and-the-two-halves-do-not-share-a-sentence-2026-08-28)).
  It is a `views.rs` question and that file does not exist yet, so it is Phase 10's.
  **The answer is very likely already fixed by precedent**, and whoever picks this up should
  start there rather than re-deciding it: `--once` settled the identical question in code —
  `tally()` emits *only the bands that have something in them* (`src/main.rs:550-566`) — and
  `screens/once.md`'s own rule is one string, two renderers. A badge that reads `1 ● 0 ▲`
  would put the two renderers in disagreement on the same fact.
  **The all-zero case is not this question**: it is already drawn as a different thing entirely
  (`○ nothing is broken`, the centred pane), which is why the counter is not a fixed-shape
  template. Found by `tui-designer`, which flagged it against itself.

  > **The `--once` half of this entry was wrong and is deleted.** As first written on
  > 2026-08-28 it also called `1 critical` an unbacked shape and had the mockups re-drawn to
  > avoid it. `tally()` had answered that in shipped code for a phase, and the PM ruled on the
  > `screens/` files without opening `main.rs` — the failure `CLAUDE.md` § *Where a leak would
  > actually happen* names as *a claim reasoned from a definition instead of measured against
  > the object*. Both drawn forms are correct; only the badge is open.

## Ruled out

*Entries that were considered and deliberately not built keep one line here with
the decision that refused them, so the same idea does not arrive twice.
[NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section) is the
long-form version and stays the authority.*

- **mem0 as a persistent-memory service** (2026-08-16) — the job is already done
  twice, by `NOTES.md` and by the session memory directory, and a hosted instance
  would put project data on an outbound connection that
  [docs/security.md](docs/security.md) says does not exist.
