# k8rs — Kubernetes Triage TUI — Decision Record

> Last updated: 2026-08-11 · Status: design phase, no code

## In one sentence

**lazygit for Kubernetes**: a single-binary Rust + ratatui TUI that tells you
**what is broken right now and why** in language a beginner understands, and
lets you fix it without memorising a single long `kubectl` command — showing
you the command it ran, every time.

*(Revised 2026-08-11 — see [Reversal](#reversal--read-only--managed-writes-2026-08-11).
The original sentence ended "…without installing anything into the cluster",
and the tool was read-only. Nothing is still installed into the cluster; the
read-only part is gone on purpose.)*

## Decision index

Every numbered decision, one line each — **read this, then open the one section
you need.** The file is ~150k tokens and no agent turn should page through it to
find an entry (NOTES § D103). Each line links its own anchor, so
`scripts/check-docs.py` fails on a heading that was renamed or removed without
its line moving with it.

- [D1](#d1--the-audience-contradiction-resolved) — The audience contradiction, resolved
- [D2](#d2--the-dividing-line-broken-now-vs-risky-later) — The dividing line: broken now vs. risky later
- [D3](#d3--findings-group-by-owner-not-by-pod) — Findings group by owner, not by pod
- [D4](#d4--the-flagship-example-promised-a-number-that-cannot-exist) — The flagship example promised a number that cannot exist
- [D5](#d5--namespace-scoping-is-a-v1-requirement-not-a-filter) — Namespace scoping is a v1 requirement, not a filter
- [D6](#d6--operation-order-was-inverted-for-the-audience) — Operation order was inverted for the audience
- [D7](#d7--rollout-undo-joins-the-operation-set) — `rollout undo` joins the operation set
- [D8](#d8--invariant-4-was-not-literally-true) — Invariant 4 was not literally true
- [D9](#d9--one-rule-added-to-v1-the-rest-recorded-not-built) — One rule added to v1; the rest recorded, not built
- [D10](#d10--m1-ships-publicly-as-v001) — M1 ships publicly as v0.0.1
- [D11](#d11--the-ninth-file-pre-approved) — The ninth file, pre-approved
- [D12](#d12--the-key-map-and-two-keys-deleted) — the key map, and two keys deleted
- [D13](#d13--licence-gpl-30-or-later-reversed-2026-08-12) — licence: `GPL-3.0-or-later` (reversed 2026-08-12)
- [D14](#d14--three-plan-corrections) — three plan corrections
- [D15](#d15--the-widget-layer-and-what-it-rules-out) — the widget layer, and what it rules out
- [D16](#d16--the-context-switcher) — the context switcher
- [D17](#d17--the---once-output) — the `--once` output
- [D18](#d18--the-clock-is-an-input-not-an-ambient-fact) — the clock is an input, not an ambient fact
- [D19](#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program) — 401 is a third case, and the kubeconfig can run a program
- [D20](#d20--a-call-that-takes-time-is-a-state-and-there-was-none) — a call that takes time is a state, and there was none
- [D21](#d21--if-the-write-cannot-be-audited-the-write-does-not-happen) — if the write cannot be audited, the write does not happen
- [D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms) — a confirmation can outlive the thing it confirms
- [D23](#d23--permissions-are-discovered-by-failing-and-that-is-backwards) — permissions are discovered by failing, and that is backwards
- [D24](#d24--ctrl-z) — Ctrl-Z
- [D25](#d25--what-this-review-did-not-decide) — what this review did not decide
- [D26](#d26--a-green-build-that-proves-nothing-2026-08-12) — a green build that proves nothing
- [D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12) — two findings the open watch already paid for
- [D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) — the workload watch, and the blind spot it closes
- [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12) — a guard is proven only for the shapes it was fed
- [D30](#d30--the-guards-phase-2-added-and-the-freeze-they-collided-with-2026-08-12) — the guards Phase 2 added, and the freeze they collided with
- [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12) — the sanitizer matched the whole string, and secrets are rarely the whole string
- [D32](#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12) — one long-lived `development` branch, not one per phase
- [D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12) — Phase 3 opens with one Phase 2 box still open, on purpose
- [D34](#d34--the-temporary-mainrs-belongs-to-dev-core-until-phase-12-2026-08-12) — the temporary `main.rs` belongs to `dev-core` until Phase 12
- [D35](#d35--just-mutants-is-a-check-that-cannot-fail-and-the-justfile-unfreezes-for-one-line-2026-08-12) — `just mutants` is a check that cannot fail, and the justfile unfreezes for one line
- [D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) — the `Finding` shape the review sent back
- [D37](#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12) — a controller's message is a status field, not a payload
- [D38](#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12) — the grouping key was a derive, and a derive cannot be told what to ignore
- [D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12) — a Node owns pods, and three more things the shape could not say
- [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12) — the capture could not produce the shape, so the test sets one field
- [D41](#d41--cargo-mutants-cannot-see-the-defect-it-was-put-there-to-catch-2026-08-12) — `cargo mutants` cannot see the defect it was put there to catch
- [D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12) — the snapshot types freeze one phase after the file they live in
- [D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) — N2 has no clock, and that makes a finding's age optional
- [D44](#d44--five-more-mockups-promising-numbers-nothing-produces-2026-08-12) — five more mockups promising numbers nothing produces
- [D45](#d45--the-decode-invented-a-container-state-the-api-says-is-waiting-2026-08-12) — the decode invented a container state the API says is `Waiting`
- [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) — nine fields the contract dropped, and the drain that does not drain
- [D47](#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12) — Phase 3 is running ahead of an open Phase 2, and what that buys and owes
- [D48](#d48--a-check-that-is-switched-off-is-named-on-the-screen-that-would-have-shown-it-2026-08-12) — a check that is switched off is named on the screen that would have shown it
- [D49](#d49--the-link-checker-skipped-every-link-that-wrapped-and-had-been-green-all-along-2026-08-12) — the link checker skipped every link that wrapped, and had been green all along
- [D50](#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12) — the rule tests live in `rules.rs`, and no lib target is added to change that
- [D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12) — the third review of the same contract, and the sentence that would have rebuilt the bug it closed
- [D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12) — the guards were fed the shapes their authors wrote, not the shapes the repo produces
- [D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12) — a committed capture is never edited to make a test pass
- [D54](#d54--now-is-metav1time-not-a-bare-jifftimestamp-2026-08-12) — `now` is `meta::v1::Time`, not a bare `jiff::Timestamp`
- [D55](#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) — the clock was written backwards, and the clamp protects the harmless half
- [D56](#d56--c1-cannot-represent-never-expires-and-a-rule-may-not-return-a-result-2026-08-12) — C1 cannot represent "never expires", and a rule may not return a `Result`
- [D57](#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12) — the pinned `now` is part of the fixture contract, and it makes "recent" unrepresentable
- [D58](#d58--a-phase-2-box-was-passed-over-and-the-order-it-comes-back-in-2026-08-12) — a Phase 2 box was passed over, and the order it comes back in
- [D59](#d59--the-sanitizer-refuses-a-requester-and-an-exit-status-guard-cannot-see-a-deletion-2026-08-12) — the sanitizer refuses a requester, and an exit-status guard cannot see a deletion
- [D60](#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12) — CLAUDE.md was compressed, and four stories moved here
- [D61](#d61--a-verify-predicate-must-hold-across-the-whole-window-not-at-one-instant-2026-08-12) — a verify predicate must hold across the whole window, not at one instant
- [D62](#d62--the-fifth-place-a-node-name-lives-and-a-guard-that-asked-less-than-its-consumer-2026-08-12) — the fifth place a node name lives, and a guard that asked less than its consumer
- [D63](#d63--the-field-kubectl-never-writes-and-a-substitution-test-that-could-not-see-a-clause-2026-08-12) — the field kubectl never writes, and a substitution test that could not see a clause
- [D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13) — the capture trip: what the cluster settled, and the approval it reversed
- [D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13) — the repin: N2 gains a clock, and what two agents decided that no brief did
- [D66](#d66--just-check-is-not-quite-the-whole-of-ci-and-the-gap-is-the-one-ci-was-built-to-watch-2026-08-13) — `just check` is not quite the whole of CI, and the gap is the one CI was built to watch
- [D67](#d67--the-cross-compile-row-closed-with-a-skip-and-what-the-skip-costs-2026-08-13) — the cross-compile row closed with a skip, and what the skip costs
- [D68](#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13) — the age ladder is not the formatter's choice, and what the brief still left open
- [D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13) — the operator review that reopened the box, and the prune line that was never true
- [D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13) — rule 8 is narrowed to `kube-system`, and every storage operator lives outside it
- [D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13) — nine rules, three blockers, and the two that were decisions, not code
- [D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13) — rule 13 is added to v1, and the field it was proposed on is narrower than the case
- [D73](#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13) — rule 10, and the test that argued for its own deletion
- [D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13) — two candidate rules, one refused and one taken, decided on who actually runs this
- [D75](#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13) — the third role nobody asked about, and the card that never cleared
- [D76](#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13) — the review that built a cluster, and the premise it measured away
- [D77](#d77--the-comment-cut-and-the-rationale-that-stays-in-the-code-2026-08-13) — the comment cut, and the rationale that stays in the code
- [D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13) — the socket the escalator could not see, and the three mutations that survived the fix
- [D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13) — the review that found the door beside the one D78 closed
- [D80](#d80--the-tests-moved-out-of-rulesrs-and-d50s-ruling-did-not-move-with-them-2026-08-13) — the tests moved out of `rules.rs`, and D50's ruling did not move with them
- [D81](#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13) — the node rules, and the four things a real cluster said about them
- [D82](#d82--the-w-series-and-the-card-that-would-have-taught-people-to-mute-the-tool-2026-08-14) — the W-series, and the card that would have taught people to mute the tool
- [D83](#d83--the-hours-rung-runs-to-48-and-the-age-ladder-gets-one-home-2026-08-14) — the hours rung runs to 48, and the age ladder gets one home
- [D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14) — a memory-starved capture host silently turns `OOMKilled` into `Error`
- [D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14) — rule 1 contradicts itself on a clean exit, and it gets its own box
- [D86](#d86--c1s-parser-costs-three-minor-rust-versions-and-the-alternative-was-an-accepted-vulnerability-2026-08-14) — C1's parser costs three minor Rust versions, and the alternative was an accepted vulnerability
- [D87](#d87--c1-has-two-bands-and-they-belong-on-two-screens-d2-only-ever-ruled-on-one-of-them-2026-08-14) — C1 has two bands and they belong on two screens; D2 only ever ruled on one of them
- [D88](#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14) — an exit code names an ending, never an agent, and the boundary for folding a found defect in
- [D89](#d89--k9ss-tracker-is-read-as-prior-art-and-twelve-of-its-classes-become-boxes-2026-08-14) — k9s's tracker is read as prior art, and twelve of its classes become boxes
- [D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15) — the third door, and the command trade D88 made a day earlier
- [D91](#d91--the-tests-split-and-the-product-file-does-not-2026-08-15) — the tests split, and the product file does not
- [D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15) — who may touch a cluster, split by the artifact and not by the agent
- [D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15) — an exit code is translated once for every role, and `137` is read from the object rather than from the number
- [D94](#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15) — the first review cluster was named `k8rs-review`, and a guard the obvious wrong name walks straight past is not a guard
- [D95](#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15) — the two `137` reasons become endings, and rule 5 draws where rule 6 goes silent
- [D96](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15) — the run a container is sitting in is no rule's subject, and the one reader may only suppress
- [D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15) — a container that cannot come back gets rule 15, and a restart count stands in for a field the pinned types cannot see
- [D98](#d98--the-user-leaves-the-room-and-the-pm-stops-asking-2026-08-15) — the user leaves the room, and the PM stops asking
- [D99](#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15) — the pin follows the newest types, and the old rule was self-violating from the first capture
- [D100](#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15) — the field that separates a settled restart from a live one was already in the snapshot, and rule 5 never read it
- [D101](#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15) — a point sample cannot separate a settled container from one on a long cycle, so the count becomes a report row
- [D102](#d102--the-second-copy-of-a-shared-sentence-is-dropped-by-analyze-and-not-by-a-rule-2026-08-15) — the second copy of a shared sentence is dropped, by `analyze` and not by a rule
- [D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15) — the process was measured, and what it lacked was a rule that makes something smaller
- [D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15) — the second agent was re-running the first agent's commands, and a tool does it better
- [D105](#d105--the-security-gate-splits-into-what-a-script-can-decide-today-and-what-is-waiting-for-code-2026-08-16) — the security gate splits into what a script can decide today, and what is waiting for code
- [D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16) — Phase 3's twenty-three open boxes are two families, six foreign boxes and one already done
- [D107](#d107--claudemd-grew-back-past-the-size-that-made-d60-compress-it-2026-08-16) — CLAUDE.md grew back past the size that made D60 compress it
- [D108](#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16) — work with no phase gets a file, and measurements get a directory
- [D109](#d109--the-family-is-the-unit-of-work-and-the-commit-stays-per-turn-2026-08-16) — the family is the unit of work, and the commit stays per turn
- [D110](#d110--the-brief-names-the-regions-because-a-cold-dispatch-reads-fifteen-thousand-lines-2026-08-16) — the brief names the regions, because a cold dispatch reads fifteen thousand lines
- [D111](#d111--the-guard-list-exists-once-and-ci-gets-no-new-action-for-it-2026-08-16) — the guard list exists once, and CI gets no new action for it
- [D112](#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16) — `lastState.terminated` has three authors, and the file was reading it as if it had one
- [D113](#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16) — a card's parts were budgeted separately and never added up, and everything else this family found was reached by fixing that
- [D114](#d114--the-capture-trip-that-put-four-objects-on-disk-and-the-init-arm-that-is-not-reachable-at-all-2026-08-16) — the capture trip that put four objects on disk, and the init arm that is not reachable at all
- [D115](#d115--the-prune-line-bounds-memory-and-was-read-as-if-it-bounded-time-and-the-paint-budget-is-stated-at-a-cluster-size-the-risk-is-not-2026-08-18) — the prune line bounds memory and was read as if it bounded time, and the paint budget is stated at a cluster size the risk is not
- [D116](#d116--the-environment-picker-moves-to-startup-and-the-tag-comes-out-of-the-kubeconfig-itself-2026-08-19) — the environment picker moves to startup, and the tag comes out of the kubeconfig itself
- [D117](#d117--the-plain-language-pass-and-the-two-things-it-found-that-were-not-sentences-2026-08-19) — the plain-language pass, and the two things it found that were not sentences
- [D118](#d118--a-foreground-call-is-capped-at-ten-minutes-and-the-phase-close-sweep-is-longer-than-one-2026-08-20) — a foreground call is capped at ten minutes and the phase-close sweep is longer than one
- [D119](#d119--the-last-surviving-mutant-was-equivalent-and-the-fix-is-to-stop-spelling-the-tie-by-hand-2026-08-20) — the last surviving mutant was equivalent, and the fix is to stop spelling the tie by hand
- [D120](#d120--the-two-things-the-operator-review-measured-that-the-tests-cannot-see-2026-08-20) — the two things the operator review measured that the tests cannot see
- [D121](#d121--the-temporary-driver-and-the-three-places-it-does-not-draw-what-the-console-will-2026-08-20) — the temporary driver, and the three places it does not draw what the console will
- [D122](#d122--the-strip-goes-on-the-value-entering-the-sentence-not-on-the-finished-sentence-2026-08-20) — the strip goes on the value entering the sentence, not on the finished sentence
- [D123](#d123--the-mutation-gate-has-nothing-to-say-about-mains-body-so-a-test-drives-the-binary-2026-08-20) — the mutation gate has nothing to say about `main`'s body, so a test drives the binary
- [D124](#d124--the-freeze-forbids-reaching-back-into-finished-logic-and-a-card-the-capture-proves-wrong-is-not-that-2026-08-20) — the freeze forbids reaching back into finished logic, and a card the capture proves wrong is not that

## Why it exists — where the gap is

Looking at the existing tools:

| Tool | What it does | What it lacks |
|---|---|---|
| k9s / Rancher | **Explorer.** Draws `kubectl get pods -A` nicely | *Tells* you nothing. You still need to know what to look at — and k9s assumes you already speak Kubernetes |
| popeye | Scans the cluster and reports problems | One-shot CLI report, not live. Weak TUI |
| k8sgpt | Interprets via LLM | LLM-dependent, doesn't work offline. Weak TUI |
| lazygit / lazydocker | The approachability model we are copying | Not Kubernetes |

The gap: nothing is **live + interpreting + approachable** at the same time.
k9s is a cockpit for pilots; the newcomer in their first month needs the thing
that says *this is broken, here is why, press `s` to fix it — and by the way,
the command was `kubectl scale …`*.

The critical distinction — not an explorer, an **interpreter**:

```
k9s       : web-7d9f  0/1  CrashLoopBackOff  47  3d
our tool  : web-7d9f restarted 47 times — last exit OOMKilled (137),
            limit 256Mi. The limit is too small.
```

The first is data, the second is an answer. And the second needs **no extra
data source whatsoever**: `lastState.terminated.reason` and `resources.limits`
already sit side by side in the same API response. Nobody joins them — that's
the whole trick.

## Reversal — read-only → managed writes (2026-08-11)

The strongest original claim was *"this tool cannot break anything."* The user
reversed it deliberately, with the alternatives (stay read-only / defer writes
to v3) on the table. Recorded here rather than applied silently, per
CLAUDE.md invariant 10.

**What changed:** k8rs becomes a **full admin console that diagnoses** —
"everything an admin needs, in one TUI" (user, 2026-08-11). Three views
instead of one screen: **Alerts** (the findings engine, still the default
view), **Resources** (browser over every kind in the cluster, with writes),
**Analysis** (cluster-wide reports that join across kinds). It is no longer a
read-only diagnostician.

**Honest sizing, stated once:** this is roughly 5–10x the original v1 and a
multi-month project rather than a multi-week one. It is planned in `todo.md`
so that a useful binary ships at the end of every milestone (M1 rules → M2
writes → M3 console), not only at the end.

**What this costs, accepted knowingly:**

| Lost | Consequence |
|---|---|
| "Cannot break anything" | The sales pitch changes: safety is now *guarded*, not *structural* |
| Trivial RBAC | Two ClusterRoles instead of one; the admin role holds `patch`/`delete` |
| Single-screen simplicity | Sidebar + resource views + modal layer — the thing NOTES called the k9s trap |
| Five files, flat | Eight files, still flat (below). No mod pyramid regardless |
| Small scope | The named number-one risk (scope creep) is now realized on purpose |

**What is kept, because it is the only differentiator left:** the rule engine.
Every competitor with a browser (k9s, Rancher) shows state; none of them
*interprets* it. **k8rs opens on Alerts, never on a pod list** — that single
default is what separates it from a k9s clone with fewer features. If that
ever changes, the project has no reason to exist.

### Positioning — "lazygit for Kubernetes" (user, 2026-08-11)

The reference point is **lazygit / lazydocker**, not k9s. The user is someone
in their first weeks on the job who should not have to memorise
`kubectl scale deployment web --replicas=3 -n payments` to do a normal day's
work. This is a stronger constraint than "admin console", and it decides
several things that were otherwise open:

1. **The command log is a first-class panel, not a hidden toggle.** lazygit's
   best idea: it shows the exact command it just ran. For k8rs the same panel
   is simultaneously the teaching device *and* the audit trail required by the
   safety model — one feature, two requirements. A beginner watches
   `kubectl scale deployment/web --replicas=3` scroll by and learns it without
   being taught.
2. **Keys are always on screen, context-sensitive.** The footer shows the keys
   valid for the panel you are in right now; `?` opens the full map. A tool for
   beginners may not hide its verbs behind memory.
3. **Zero configuration on first run.** No flags, no config file, no setup
   step. It reads your kubeconfig's current context and works.
4. **Plain language over jargon, everywhere** — not only in findings. Column
   headers, confirmation dialogs, error messages. `Evicted` is "removed by the
   node because it ran out of room".
5. **Confirmations explain the consequence, not the API call.** "This starts 2
   more copies of your app" above the `kubectl` line, not instead of it.
6. **Per-object detail tabs**, the lazydocker pattern: logs · describe · yaml ·
   events, switched with `[` / `]`. A newcomer's whole debugging loop without a
   single typed command.

This positioning also resolves the tension in the reversal: lazygit is not
"read-only git", it is git made approachable — and it is trusted precisely
because it is explicit about what it runs.

### The three views

```
┌ k8rs ───────────────────────────────┬ ctx: prod-eu · ⟳ live · ADMIN ─┐
│ ALERTS      ▸ 3 critical  7 warn   │                                │
│ RESOURCES   ▸ workloads            │   (the selected view fills      │
│               network              │    this pane — one at a time,   │
│               storage              │    no split panes)              │
│               config               │                                 │
│               cluster              │                                 │
│ ANALYSIS    ▸ capacity             │                                 │
│               certificates         │                                 │
│               drain safety         │                                 │
│               waste                │                                 │
│               versions             │                                 │
└────────────────────────────────────┴─────────────────────────────────┘
```

| View | What it is | Cadence |
|---|---|---|
| **Alerts** | `rules.rs` output: per-object findings, severity-sorted, live. The original triage screen, unchanged. Default view on startup. | streamed, live |
| **Resources** | Browser over every kind, grouped (workloads / network / storage / config / cluster). Where the write operations live. | list + watch |
| **Analysis** | `analysis.rs`: cluster-wide reports that **join across kinds** and cannot be expressed as a per-object rule — capacity/overcommit, certificate expiry, drain safety, waste, version skew. | on demand |

Alerts vs Analysis is not cosmetic: a rule looks at *one object* and fires
continuously; an analysis looks at *the whole cluster* and is computed when
opened. Different shape, different cadence, therefore different file.

### How "every kind" is supported without writing code per kind

Hand-writing a view for 40+ built-in kinds plus arbitrary CRDs is the trap
that makes browsers enormous. Two mechanisms avoid it:

1. **API discovery** (`kube::discovery`) enumerates every kind the cluster
   actually serves, including CRDs, at startup. The sidebar is generated from
   the cluster, not from a hard-coded list.
2. **Server-side printing** — asking the API server for
   `Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json`
   returns *the exact columns `kubectl get` prints*, computed by the API
   server, for any kind including CRDs. No column definitions in our code, no
   per-kind formatting, and CRDs display correctly for free. (The trailing
   `,application/json` is not optional — see the upstream review below.)

So: `DynamicObject` + Table for browsing (generic, zero per-kind code), and
**typed** `k8s-openapi` structs only on the paths the rule engine needs
(Pod, Node, Deployment, PVC, Service). Rules stay strongly typed and testable;
the browser stays generic. This split is the single decision that keeps
"everything" from meaning "everything hand-written".

### Operations — the full admin surface

| Key | Operation | Applies to | Guard | Ships |
|---|---|---|---|---|
| `l` | logs (follow, previous, container picker) | pods | read | v0.1 |
| `d` | describe — object + its events | any | read | v0.1 |
| `y` | view YAML | any | read | v0.1 |
| `s` | scale replicas | deploy/sts/rs | confirm + dry-run | v0.1 |
| `r` | rollout restart | deploy/sts/ds | confirm | v0.1 |
| `ctrl-d` | delete | any | **type the name** | v0.1 |
| `c` | cordon / uncordon | nodes | confirm | v0.2 |
| `ctrl-r` | drain (respects PDBs, reports blockers) | nodes | **type the name** | v0.2 |
| `u` | rollout undo — roll back to the previous revision | deploy/sts/ds | confirm + dry-run | v0.2 |
| `x` | exec into container | pods | confirm (writes into a shell) | v0.3 |
| `p` | port-forward | pods/services | confirm; forwards listed in the header | v0.3 |
| `e` | edit in `$EDITOR` → diff → apply | any | confirm + dry-run + diff | v0.4 |

The ladder is ordered by *what an operator does in a normal day*, not by what
is easiest to build — see [D6](#d6--operation-order-was-inverted-for-the-audience).
`exec` and `port-forward` need real terminal work (PTY handover, local socket
lifetime). `edit` lands last: it is the most dangerous operation, the least
provable without a terminal, and the one every admin already has a habit for.
`rollout undo` is not an API verb — kubectl computes the previous ReplicaSet's
template client-side and patches it back, and k8rs has to do the same
([D7](#d7--rollout-undo-joins-the-operation-set)).

**Not offered, still:** anything that deploys k8rs itself into the cluster
(invariant 2 survives the reversal — the trust model is still "runs on your
machine with your kubeconfig"), and bulk mutation of a selection.

### The safety model that replaces "read-only"

Writes exist, so the guarantee is no longer structural. It is replaced by five
mechanisms, each of which is a requirement, not a nicety:

1. **No implicit write, ever.** Every mutation is a keypress on a selected
   object followed by a confirmation. No bulk operations, no "apply to all",
   no write triggered by navigation or refresh.
2. **Server-side dry-run first.** Scale/edit/restart run with `dryRun=All`
   and show the API server's verdict before the real call. A rejected dry-run
   never proceeds. Free validation, and it catches the admission-webhook class
   of surprise.
3. **Destructive actions are typed, not tapped.** `delete` requires typing the
   resource name — the ctrl-key-slip class of accident is the one that ends
   careers.
4. **Every write is logged locally** to `~/.local/state/k8rs/audit.log`:
   timestamp, context, namespace, object, the equivalent kubectl command,
   and the API result. Append-only, plain text, no cluster involvement.
5. **`--read-only` flag restores the original guarantee** in one flag: the
   write code path is not reachable, the keys are not bound, and the header
   says so. This is the mode for teaching, demos, and production contexts —
   and it is the mode the CI e2e job runs in.

**Structural consequence — writes live in exactly one file.** `ops.rs` is the
only file permitted to call `create`/`patch`/`replace`/`delete`; the
`clippy.toml` ban stays crate-wide and `ops.rs` carries a single visible
`#![allow(clippy::disallowed_methods)]` at its top. One file to audit, one
line that announces the exception. `rules.rs` stays pure, `ui.rs` never
touches the API.

## Design review — second pass (2026-08-11)

A second review of the reversed design, from the point of view of the person
who would actually run this daily. Fourteen decisions — eleven from the review
itself, three more from the audit of the plan that followed it. Each one closes
a contradiction the reversal left open, and several of them **delete** planned
work rather than add it.

### D1 — The audience contradiction, resolved

"lazygit for Kubernetes" (month-one newcomer) and "full admin console" are
different products, and the scope guard written for the first one would have
deleted half of the second: drain safety, version skew, overcommit and
certificate expiry are not things anyone uses in their first month.

**Decision — one audience, two axes:**

> **Written for someone in their first month. Useful to the person on call.**

The *language* target is the newcomer: every string, every column header,
every dialog. The *feature* target is the working operator. The two never
conflict, because explaining a thing plainly does not make it less useful to
someone who already knew it — it only costs words.

**The scope guard is replaced.** Old: *"would someone in their first month use
this in a normal week?"* New, both halves required:

> Would someone who runs clusters use it in a normal week — **and** can a
> newcomer read the screen it produces without a glossary?

The first half rejects expert toys; the second rejects k9s.

### D2 — The dividing line: broken now vs. risky later

The noise problem and the audience problem have one shared answer. Every rule
is assigned by a single question:

| Question | Goes to |
|---|---|
| Is something **broken right now**? | **Alerts** |
| Is something **risky, wasteful or expiring**? | **Analysis** |

Alerts is a work queue: everything in it should be actionable today, and an
empty Alerts screen must be believable. Analysis is a posture review: you open
it when you want it.

Applied to the existing set, this **moves four rules out of Alerts**:

- **Rule 9 (no limits defined)** → the Capacity report. A pod without limits is
  not broken; it is a risk. Almost every cluster has hundreds of them, and left
  in Alerts it would bury the three findings that matter on day one.
- **Rule 8 (hostPath)** → only the escalated case stays in Alerts (`/`, a
  container-runtime socket or a directory one sits under, or a writable host
  mount — [D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)). The plain read-only
  hostPath is how CNI, CSI and every node agent are *supposed* to work; it goes
  to the posture rows of Analysis.
- **N4 (kubelet version skew)** → the Versions report, where it already had a
  home.
- **C1 (kubeconfig certificate expiry)** → the Certificates report. The sidebar
  badge (`certificates 30d` in the sketch) is the alerting mechanism, and it
  needs no new machinery.

Nothing is lost — those findings still exist, still get tested, and are one
keypress away. What changes is that Alerts stops being a lint report.

### D3 — Findings group by owner, not by pod

**Requirement, not a nicety.** One DaemonSet on a 40-node cluster is one
finding with a count, not forty findings. Findings are grouped by owner
(Deployment / StatefulSet / DaemonSet / Job, falling back to the bare pod when
there is no owner), and the card reads *"3 of 40 pods"* with the offenders
listed on the detail view.

Without this the reversal's own principle — one accent colour, red means
broken — dies in the first real cluster, and k8rs becomes the thing NOTES
criticised popeye for. This was implied by one line in the screen sketch
(*"same pattern in 4 more pods"*) and was never a decision. Now it is.

### D4 — The flagship example promised a number that cannot exist

The sketch showed `limit 256Mi, used 251Mi`. metrics-server reports *current*
usage of a *live* pod; the usage of a container at the moment the kernel killed
it is not retrievable from any API k8rs talks to. The tool's headline example
was a number it could not produce.

**Decision:** OOM evidence is `limit 256Mi · exit 137 (SIGKILL) · 47 restarts ·
last 3 min ago`. Live usage appears only when metrics-server exists *and* the
pod is currently running, and is never labelled as the usage at kill time.
Fixed in the sketch below.

### D5 — Namespace scoping is a v1 requirement, not a filter

`Api::all()` fails with a 403 for the very common user who has access to some
namespaces and not the cluster. For that user the Pod watch dies, and with it
the Alerts view — i.e. the whole product, on first launch, for a large slice of
the audience. "A 403 degrades one feature" does not cover the case where the
feature is everything.

**Decision:** `--namespace/-n` exists in v1, and a 403 on the cluster-wide LIST
falls back to the kubeconfig context's namespace (then `default`), with the
header stating which scope is in effect and why. Four flags now
(`--read-only`, `--context`, `--namespace`, `--once`); still no `clap` — the
threshold for revisiting that is a flag needing validation or a subcommand,
not a fourth boolean.

### D6 — Operation order was inverted for the audience

An operator's day is *logs · describe · exec*. The plan shipped `edit+apply` in
the first release — the most dangerous operation, the least provable
headlessly (it needs `$EDITOR`), the one every admin already has a habit for
(`kubectl edit`) — while `exec` waited until v0.3.

**Decision — the write ladder is reordered:**

| Release | Operations |
|---|---|
| **v0.1** | scale · restart · delete *(plus all reads, including logs)* |
| **v0.2** | cordon / uncordon · drain · **rollout undo** |
| **v0.3** | exec · port-forward |
| **v0.4** | edit + apply |

Consequences: `similar` leaves the v1 dependency set with `edit` (see
[Dependencies](#dependencies)); `serde_yaml_ng` stays, because `y` still shows
YAML. The YAML spike result and the "user's buffer is the source of truth" edit
model stay recorded as decided — they are simply applied in v0.4.

### D7 — `rollout undo` joins the operation set

The single most-reached-for command after a bad deploy, and it was missing
entirely while `edit` was in. Added at v0.2, with the honest cost noted: it is
not an API verb. `kubectl rollout undo` reads the previous ReplicaSet's pod
template and patches it back into the Deployment, client-side, and k8rs has to
do the same. That is real work, not a one-line call — which is exactly why it
is recorded now rather than discovered later.

### D8 — Invariant 4 was not literally true

"Every command k8rs runs is shown to the user" — but k8rs runs
`Api::patch_scale`, `Api::restart`, `Api::evict`. Those are not kubectl
invocations; the panel shows the *equivalent* command. Fine as a teaching
device, dishonest as an audit trail: a log recording a command that never ran
cannot answer "what did they actually do".

**Decision:** the split is made explicit. The **command log** shows the
equivalent kubectl command (teaching). The **audit log** records both that line
*and* the real API call — verb, path, resourceVersion sent, dry-run verdict,
result. Invariant 4 in CLAUDE.md is reworded accordingly.

### D9 — One rule added to v1; the rest recorded, not built

The review turned up five genuinely common failures the rule set missed. Only
one is free with the watches already running:

- **Rule 12 — pod stuck Terminating.** A `deletionTimestamp` already in the
  past means a finalizer or a wedged kubelet is holding it. Costs
  nothing (the Pod watch is already open), happens constantly, and `kubectl
  get` never explains it. **In v1.**

The other four need a stream or list that the two-permanent-watch budget
([invariant 6](#architecture--where-lightweight-comes-from)) has no room for in
v1. Recorded as the **v0.2 rule set**, in the order they earn their watch:

| # | Finding | Needs |
|---|---|---|
| J1 | Job failed permanently (backoffLimit exceeded) | `batch/v1` Jobs |
| J2 | CronJob suspended, or no successful run in N schedules | `batch/v1` CronJobs |
| H1 | HPA pinned at maxReplicas, or unable to compute metrics | `autoscaling/v2` |
| Q1 | ResourceQuota exhausted — nothing can be created in the namespace | quota list |
| — | Deprecated / removed API versions in use | discovery + a removal table that goes stale; Analysis report, later |

**Related, and decided the same way:** *"Service whose selector matches no
pod"* — the best single finding in the whole design, the 503 nobody can
explain — **stays in the Waste report** and does not get promoted to Alerts.
Promoting it would cost a permanent Services + EndpointSlices watch, and the
watch budget is the reason k8rs is lighter than k9s. It becomes the first row
of the report instead.

### D10 — M1 ships publicly as v0.0.1

The plan's real failure mode is dying during the console phase with nothing
released. Phase 3 already builds a temporary main that prints findings; the
decision is to **release it** as `k8rs --once` — reads the cluster, prints
what is broken, exits.

It costs one flag and a README. In exchange, the diagnosis engine — the only
part of this project nothing else does — is in people's hands months before the
TUI, and the feedback arrives while the rules can still change cheaply.

### D11 — The ninth file, pre-approved

`ui.rs` has to draw three views, a sidebar, detail tabs, a command log, a help
overlay and a modal layer. It will not stay small, and
[invariant 11](#file-layout) demands a boundary argument before a ninth file
exists. The argument is accepted **in advance** so that Phase 11 does not stall
on a rule debate: if `ui.rs` passes ~800 lines, the modal layer splits into
`dialog.rs` — confirmations, typed-name deletes and the help overlay, the one
part of the UI that is separately testable and that renders the safety
contract.

That is the only pre-approved split. Everything else still needs an argument.

### D12 — the key map, and two keys deleted

Auditing the plan against the screen mockups turned up four keys with two
meanings each — not a design, an accident. Resolved by giving every key **one
meaning everywhere**, and deleting the two features that caused the collisions:

| Key | Meaning | Where |
|---|---|---|
| `/` | filter this list / search this pane | everywhere, including the log tab |
| `n` | filter by namespace | every list view |
| `f` | follow | the log tab only; unbound elsewhere |
| `c` | pick a container (log tab) · cordon/uncordon (a node in Resources) | two panes that cannot both be open, so this one is legal by context and is recorded rather than resolved |
| `r` | rollout restart | never "retry" |
| `X` | switch cluster | everywhere; unbound while a modal is open ([D16](#d16--the-context-switcher)) |
| `⇧p` | the previous container's log (`--previous`) | the log tab only; unbound elsewhere |
| `q` | quit | everywhere; refused while a write is in flight ([dialogs](screens/dialogs.md)) |

**`⇧p` was bound before it was recorded.** It sat in the `detail.md` footer
with no row here, so the one screen that promises to show every key —
`help.md` — did not show it, and neither did `f` or `c`, which *were* recorded
here. Fixed on both sides: the three log-tab keys are now on the help screen,
and `⇧p` has a row. A key that exists in a mockup and nowhere else is a key
nobody agreed to.

This table is the collisions and the exceptions, not the full map — the full
map is `help.md`, which is the screen the user actually reads.

**Deleted: the severity filter.** It existed because the Alerts list was going
to be long; owner grouping ([D3](#d3--findings-group-by-owner-not-by-pod))
made it short, and severity is already the sort order. One key and one feature
fewer.

**Deleted: the manual reconnect key.** kube-rs backs off and reconnects on its
own; the requirement was always that the state is *visible*, never that the
user drives it. The disconnected banner reports, it does not ask.

### D13 — licence: `GPL-3.0-or-later` (reversed 2026-08-12)

Decided as `MIT OR Apache-2.0` on 2026-08-11 for one reason — it is the Rust
ecosystem default — and reversed on 2026-08-12 by the author's actual
requirement: *nobody repackages this as their own closed product.*

**What was not the deciding factor:** attribution. MIT, Apache-2.0 and GPL all
require the copyright notice to travel with the code, so "my name stays on it"
was never at risk under any of them.

**What decided it:** under MIT/Apache anyone may take k8rs, modify it, and ship
a **closed-source** product; the notice survives in a licence file nobody
reads, the product does not. GPL-3.0 permits selling — it has never forbidden
that — but a distributor must hand over the source under the same licence, so
the author's name and the freedom travel with every copy. That is the
requirement, stated in licence terms.

**What it costs, honestly:** k8rs cannot be embedded as a library inside a
closed-source tool. For a standalone binary that talks to a kubeconfig, that
is close to no cost — running it, internally or commercially, is unrestricted
for everyone. Some organisations do keep GPL policies aimed at *linking*;
none of them prevent running a CLI.

**Unchanged:** `cargo publish` needs the `license` field either way
(`GPL-3.0-or-later`, SPDX). `deny.toml`'s copyleft rejection for
*dependencies* stays exactly as it was — that policy is about what we pull in,
not about what we publish, and every approved crate is MIT/Apache, which is
GPL-compatible in this direction.

**Not chosen: AGPL-3.0.** Its extra clause covers use *over a network*, and
k8rs runs on the user's machine against their own cluster — there is no
service to close. Revisit only if a hosted version ever exists.

**The name is separate from the licence.** GPL covers the code, not the
trademark: "k8rs" as a name is not licensed with it, so a fork must rename
itself rather than ship "k8rs" with someone else's product behind it. That
sentence belongs in the README when it is written.

### D14 — three plan corrections

Not design decisions, but the plan was wrong in three places and the rule is
that the plan gets fixed in writing, not patched at build time:

1. **The justfile does not freeze at the end of Phase 1.** Its targets are all
   declared there, but the body of `fixtures` — the jq sanitizer — is written
   in Phase 2, where the fixtures are. Freezing it a phase early made a
   forward-only violation inevitable. It freezes after Phase 2.
2. **The Phase 7 `k8rs ops …` subcommand is scaffolding, not surface.** It
   lives in the temporary main so that every write can be proven headlessly,
   and it disappears when the console lands. It therefore does not trip the
   "a subcommand means it is time for clap" threshold — nothing shipped has a
   subcommand.
3. **The read-only hostPath posture rows belong to `analysis.rs`.** They read
   pod fields, so they *could* live in `rules.rs`, but they are a whole-cluster
   list with no per-object alert — putting them in `rules.rs` would mean
   reaching back into a frozen file to make them a report. `rules.rs` emits the
   escalated case only.

### D15 — the widget layer, and what it rules out

The mockups said what every screen looks like and nothing said what draws it,
so Phase 11 would have designed the UI while implementing it. The map lives in
[screens/widgets.md](screens/widgets.md) — one file, because a widget choice
scattered across seven mockups is seven places to keep in sync. Three things
it settles, all of which are decisions rather than transcription:

1. **No mouse support.** crossterm's mouse capture takes the terminal's own
   text selection away from the user, and the command log exists to be copied.
   A tool whose teaching device cannot be copy-pasted has traded its point for
   a click target.
2. **No animation, no spinner.** Every one of them needs a timer tick, and a
   timer tick is a frame rate — [invariant 7](CLAUDE.md) says draws happen
   on events and the loop blocks when idle. "Still loading" stays a static
   line of text.
3. **Below 80×24 the layout is not attempted.** One centered sentence naming
   the required size and the current one, and the normal frame returns when
   the terminal grows. The alternative is responsive breakpoints for a fixed
   20-column sidebar next to a server-printed table, which is a second layout
   engine in exchange for a screen nobody can read anyway.

A fourth thing it confirms rather than decides: every ratatui state object
(`ListState`, `TableState`, `ScrollbarState`, tab index, scroll offset) lives
in `views.rs`, leaving `ui.rs` a pure function of state. That was already the
file layout's intent; it is now written where someone building the UI will
read it.

### D16 — the context switcher

The out-of-scope list said "one context at a time, **switchable**" and no
screen, key or step ever defined the switch: `--context` was a startup flag,
so checking staging meant quitting and relaunching. Closed by `X` and a modal
picker ([screens/context.md](screens/context.md)). What the design turned on:

1. **No confirmation dialog.** Nothing is written — not to the cluster, not to
   the kubeconfig — so [invariant 2](CLAUDE.md) does not apply and a second
   dialog would be ritual. The picker *is* the confirmation: explicit key,
   explicit selection, the target's API server address on screen. What is
   required instead is that `X` cannot fire while a modal is open or a
   confirmed write is still in flight.
2. **The command log must not print `kubectl config use-context`.** That
   command edits the user's kubeconfig; k8rs never does. Showing it would
   teach a side effect the tool does not have — the same dishonesty
   [D8](#d8--invariant-4-was-not-literally-true) was written to remove. Every
   line after a switch carries `--context <name>` instead, which is both true
   and the better thing to learn.
3. **A failed switch does not fall back.** Unreachable or 403 on the new
   context shows a modal and *stays there*. Silently returning to the old
   cluster produces a header naming one cluster over another's data, which is
   the exact failure [screens/states.md](screens/states.md) forbids.
4. **The switch is the startup path run again**, which forced a plan
   correction: `k8s.rs` (Phase 5) has to expose connecting as a re-callable
   function. Discovering that in Phase 11 would have meant reaching back into
   a frozen file — the forward-only rule catching a real error rather than a
   hypothetical one.

The kubeconfig stays the only source of credentials: a switch picks a
different context out of the same file and never accepts a typed server,
token or certificate. [Invariant 3](CLAUDE.md) does not move.

### D17 — the `--once` output

Seven TUI screens were drawn and the one thing that ships **first** had none:
`k8rs --once` is v0.0.1, months ahead of the console, and it is the first
output any stranger sees ([D10](#d10--m1-ships-publicly-as-v001)). Designed in
[screens/once.md](screens/once.md); three decisions in it:

1. **Findings do not change the exit code.** `0` when k8rs ran, `2` when it
   could not (no kubeconfig, unreachable, not allowed). k8rs is a report, not
   a linter — a beginner who sees three warnings and then `$?` = 1 concludes
   the tool broke. `1` stays unused so a `--exit-code` flag has somewhere to go
   if anyone ever asks, without redefining `0`.
2. **stdout is the findings, stderr is the commands and the errors.** The
   command log is the teaching device outside the TUI too
   ([invariant 4](CLAUDE.md)), but a piped report should arrive without it.
   Splitting the streams gives both with no flag.
3. **No analysis reports in `--once`.** Selecting a report needs an argument
   that takes a value, and that is the threshold that pulls `clap` in
   ([invariant 10](CLAUDE.md)). `--once` answers one question; the reports are
   a console feature.

It is the Alerts view with the frame removed — same rules, same strings, same
owner grouping, same order. Two renderers over one `rules.rs`; if they can
disagree, one of them is lying.

## Design review — third pass (2026-08-11)

The second pass reviewed the *product*. This one reviewed the **runtime**: what
this kind of application actually does between the screens — time, credentials
that expire, calls that take minutes, records that cannot be written, objects
that vanish mid-dialog, a terminal that gets suspended. Seven of these were
missing outright and one of them contradicts a hard invariant.

None of it is new scope. Every item below is something the already-agreed
design must do and did not say how.

### D18 — the clock is an input, not an ambient fact

[Invariant 5](CLAUDE.md) says rules are pure: "no clock, no network, no
terminal, no globals". But rule 12 fires on a pod whose `deletionTimestamp` is
*older than the grace period*, C1/C2/C4 report a certificate that *expires in
N days*, and every finding on the Alerts screen ends in *"4 min ago"*. All
three need to know what time it is. The invariant and the rule set have been
contradicting each other since both were written.

**Resolution: `now` is a field on `Snapshot`.** The caller captures it once per
analysis pass and puts it in the input. `analyze(&Snapshot) -> Vec<Finding>`
stays exactly as pure as the invariant demands — it reads a field, it does not
call a clock — and the invariant's wording gains one clause rather than an
exception.

This is also what keeps the tests honest. A fixture pins `now` alongside the
objects, so a test written today still passes in 2029; a rule that called
`SystemTime::now()` would need its fixtures re-captured every time a certificate
in them expired, and the fix for that rotting test would be to weaken it.

Two consequences that the implementation cannot skip:

- **Findings carry timestamps, not phrases.** "4 min ago" is formatted by the
  renderer — `ui.rs` and the `--once` printer — from a timestamp the rule
  emitted. Two renderers, one source, and the same rule is readable in a test
  without parsing English.
- **Clock skew is real and shows.** The timestamps come from the API server;
  `now` comes from the user's laptop. A machine a few minutes *behind* the
  cluster produces a negative age, and *"in -3 minutes"* on the first screen a
  beginner sees is worse than useless. A non-positive age renders as
  **"just now"**. *(This sentence said "fast" until 2026-08-12 and had the
  direction backwards — a fast laptop inflates ages, it does not negate them,
  and it is the half that manufactures findings. What the clamp actually
  protects, and what the fast half needs instead, is
  [D55](#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12).)*

**The type is `jiff::Timestamp`, and it is already in the tree** — verified
against `k8s-openapi 0.28.0` rather than assumed, because the assumption was
wrong the first time. `meta::v1::Time` is `pub struct Time(pub jiff::Timestamp)`
and the crate re-exports the library (`pub use jiff;` in `lib.rs`), so `now` is
the same type the API's own timestamps already are: no conversion layer, no
`Cargo.toml` line, no [invariant 10](CLAUDE.md) decision to make. *(It was
`chrono` until k8s-openapi moved; writing "chrono" here from memory would have
put a dependency in the plan that the crate graph does not contain.)*

What it does cost, and what is therefore recorded: k8rs's own time type is now
tied to **k8s-openapi's** jiff major version. That is the same coupling the
API types already have, it moves when k8s-openapi moves, and k8s-openapi is
upgraded together with kube-rs and never separately
([docs/tech-stack](docs/tech-stack.md)).

### D19 — 401 is a third case, and the kubeconfig can run a program

The error taxonomy is a "2-variant enum: 403 vs no-connection"
([REQUIREMENTS § Error states](REQUIREMENTS.md#error-states-all-were-undefined-all-happen-on-first-launch)).
On every managed Kubernetes service that is wrong, in two ways.

**A third arm: credentials that expired.** EKS, GKE and AKS kubeconfigs hold no
token at all. They name an external binary — `aws eks get-token`,
`gke-gcloud-auth-plugin` — that is executed to mint a short-lived credential.
That credential expires *during a session*, and the API server answers **401**,
which is not 403 ("you are known and not allowed") and not a dead connection
("nothing answered"). It needs its own plain-language state: the login expired,
here is the command that renews it, k8rs is still running. Telling a beginner
"you are not allowed to list pods" when the truth is "your login timed out"
sends them to their platform team for nothing.

**And the security fact nobody had written down:** that exec plugin means **a
kubeconfig can make k8rs spawn an arbitrary process**. It is the only
code-execution path in the entire trust model. k8rs inherits it from kube-rs,
which inherits it from the way `kubectl` has always worked, and it is not
something to remove — without it the tool does not run on any managed cluster.
What gets written down is that k8rs never *extends* it: it never installs a
credential plugin, never offers to, never runs one from a file the user did not
point it at, and treats the plugin's stdout as a credential like any other —
[token hygiene](docs/security.md#token-hygiene) applies to it unchanged.

### D20 — a call that takes time is a state, and there was none

Every screen is drawn as though the API answers instantly. `scale` against a
busy API server takes seconds; `drain` takes **minutes**. Nothing defined what
the UI is during that window, and "it will be quick" is not a design.

- **The modal closes on confirmation, not on completion.** The command log line
  is written immediately and ends in `…` until the result replaces it. A dialog
  that sits there frozen teaches the user that the tool hangs.
- **Navigation stays free. Three things do not:** a second mutation, `X`
  ([D16](#d16--the-context-switcher)), and `q`. Quitting mid-`PATCH` leaves the
  audit log with an attempt and no result, which is exactly the trail that
  cannot answer what happened; `q` refuses in the footer and quits when the
  call returns.
- **`drain` is different in kind, not in degree.** Minutes, pod by pod, with
  evictions that can be refused by a PDB one at a time. It gets a progress pane
  — *"evicting 4 of 11 · 2 blocked by PodDisruptionBudget"* — not an indicator.
  It arrives with drain in v0.2, and it is the only long-running operation in
  the plan until `port-forward` in v0.3.

No spinner anywhere; a changing *count* is information, a rotating character is
a frame rate ([D15](#d15--the-widget-layer-and-what-it-rules-out)).

### D21 — if the write cannot be audited, the write does not happen

`~/.local/state/k8rs/audit.log` is specified down to its file mode, and
nothing says what happens when it cannot be written — full disk, read-only
home, a `$XDG_STATE_HOME` pointing somewhere that does not exist.
[Invariant 4](CLAUDE.md) makes the answer forced rather than a preference: a
mutation missing from either record *is a bug*, so a mutation that cannot be
recorded must not occur.

- The **attempt** line is written and flushed *before* the API call; the result
  is appended when it returns. A crash mid-call therefore leaves an attempt
  with no result — which is the honest record of what happened, and the reason
  the two are separate writes.
- If the log cannot be opened **at startup**, k8rs says so and continues in
  read-only mode. It does not exit — a broken state directory should not stop
  someone from looking at their cluster — and it does not quietly drop the
  trail, which is the only unacceptable option.
- **No rotation.** A few hundred bytes per mutation, at fifty mutations a day,
  is a megabyte in something like a decade. A rotator would be more code than
  the log itself and is the sort of thing that gets written because it feels
  responsible.

### D22 — a confirmation can outlive the thing it confirms

Open the delete dialog for `web-7d9f4`, start typing its name, and its
ReplicaSet replaces it while you type. The rule that the object's identity sits
in the title bar ([dialogs.md](screens/dialogs.md)) stops a *stale selection*
from being confirmed; it does nothing about an object that is already gone.

The watch is still running behind the modal, so the dialog does not need to
poll anything:

- The dialog holds the object's `uid` **and** the `resourceVersion` it was
  opened with.
- **Gone:** the dialog becomes *"this pod is already gone — something else
  removed it"* and the confirm button dies. Deleting by name what the user
  believes is the object they selected is how the wrong pod gets deleted.
- **Changed:** it says so and offers a re-read. This is the 409 mechanic from
  [REQUIREMENTS § Conflict handling](REQUIREMENTS.md#write-operations-new--the-reversal), moved to
  where the user can still do something about it cheaply.

### D23 — permissions are discovered by failing, and that is backwards

The current rule is "a 403 degrades that one feature and names the missing verb
and resource" — *after* the attempt. For reads that is fine. For the typed-name
delete it is not: the user types a pod name in full, presses `⏎`, and only then
learns they were never allowed. The typing exists to prevent an accident; asking
it of someone who cannot perform the action is not safety, it is a chore.

**One `SelfSubjectRulesReview` per namespace answers it in a single call** —
the API returns everything the user may do there, so keys the user cannot use
are dim from the start and the footer says why. Cluster-scoped operations
(cordon, drain) need a `SelfSubjectAccessReview` each; there are two of them.

**This trips [invariant 1](CLAUDE.md), and the trip is the interesting part.**
Both reviews are performed with **`create`**, so the allowlist outside `ops.rs`
(`get*` / `list*` / `watch*` / `logs` / `log_stream` / `apiserver_version`)
forbids them — even though they change nothing in the cluster. The decision is
to **put them in `ops.rs`** rather than to widen the allowlist: the allowlist is
a mechanical guard and the moment it grows a "but this create is harmless"
clause, it stops being mechanical. `ops.rs` gains one clearly-named read-only
function, `may_i(...)`, with a comment saying why a file called "every write"
contains something that writes nothing. That is a smaller cost than a rule that
requires judgement to apply.

### D24 — Ctrl-Z

`$EDITOR` needs terminal handover in v0.4 and that was planned for. **Ctrl-Z
needs the same handover in v0.1 and was not.** SIGTSTP on a raw-mode TUI hands
the terminal back to the shell still in raw mode, with the alternate screen
still active; the user's prompt is unusable and `fg` returns to an application
that never re-initialised. It is the most-reported bug class in TUI issue
trackers and it costs one function.

That function — leave raw mode, leave the alternate screen, and the reverse on
resume — is the same one `e` needs later, so it is written once in v0.1 and
reused rather than discovered twice. `ratatui::init()` installs a panic hook
that restores the terminal, which is a different path and does not cover this
([widgets.md § 1](screens/widgets.md#1-the-frame)).

### D25 — what this review did not decide

Memory at scale is **unmeasured**, and an unmeasured number is not a decision.
Pruning `managedFields` is agreed; whether a 10 000-pod cluster fits in a
reasonable resident set once pruned is a question with a real answer that
nobody has run. It becomes a Phase 5 measurement with a number attached, not a
design paragraph written from a guess.

### D26 — a green build that proves nothing (2026-08-12)

CLAUDE.md says tests must not lie, and the plan enforced that for the *rules*
and for one guard. Four holes were left, all of the same shape: a check that
reports success without having checked anything.

1. **"CI is green on an empty crate" was the Phase 1 done-when.** A green run
   over zero tests and no product code is the false positive we said we would
   not accept — it proves the YAML parses, nothing else. Every guard added in
   Phase 1 is now proven the way the write-allowlist already was: break it on
   purpose, watch CI go red, then fix it. The red run is recorded per guard
   (fmt · clippy · check-docs · cargo-deny · the allowlist). A guard nobody
   has seen fail is not a guard, and that applies to all five, not to the
   scary one alone.
2. **`cargo test` exits 0 when it runs no tests**, and `#[ignore]` deletes a
   test while leaving its name in the file — the two cheapest ways for this
   project to fool itself. CI asserts a non-zero test count and fails on any
   `#[ignore]` that does not carry a written reason.
3. **Mutation testing covered `rules.rs` only.** `analysis.rs` is the same
   kind of file — pure, fixture-tested, the second diagnosis engine — and a
   report that silently stops flagging anything looks exactly like a report
   with nothing to flag. It gets the same `cargo mutants` gate in Phase 4.
   The two pure files are precisely where a passing test can be worthless,
   which is why they are also the two that come first
   ([build order](#build-order--why-it-is-what-it-is)).
4. **The justfile freezes at the end of Phase 2, but `cargo mutants` is first
   needed in Phase 3.** Same forward-only violation `e2e` had, same fix: the
   `mutants` target is declared in Phase 1 with the others, body and all
   ([D14](#d14--three-plan-corrections)).

What this does not add: no coverage percentage. A coverage number is satisfied
by tests that execute code without asserting anything about it, which is the
failure mode being closed here, not a measurement of it. Mutants answers the
question coverage only gestures at — *would anyone notice if this line were
wrong?*

### D27 — two findings the open watch already paid for (2026-08-12)

A pre-code read of the rule set against what an operator hits in a normal
week. Both items below are computed from the **Pod object that is already
being watched** — no new stream, no new dependency, no new screen. They were
missing because of where they were filed, not because they cost anything.

**1. Init containers were invisible.** Every pod rule reads
`status.containerStatuses`. `status.initContainerStatuses` is a *separate
array*, and a pod stuck at `Init:CrashLoopBackOff` or `Init:Error` therefore
produced no finding at all — while `kubectl get pods` shows it plainly. Init
containers are where migrations, config fetches and wait-for-dependency loops
live, so this is not an edge case: it is the first thing that breaks in a
freshly deployed app, and the tool was silent on it. Rules 1–6 read both
arrays; the finding names which init container, because "the app container is
fine and the init one is not" is the whole diagnosis.

**2. "Pending, and why" does not need the Events watch.** Rule 10 was deferred
to v0.5 bundled with rule 11 (probe failures), on the reading that both need
Events. Rule 11 does. Rule 10 does not — the scheduler writes both the verdict
*and* the human sentence onto the pod itself:

```
status.conditions[type=PodScheduled]:
  status:  "False"
  reason:  Unschedulable
  message: "0/3 nodes are available: 3 Insufficient cpu."
```

That message is the answer to the single most common beginner question, and it
is sitting in a field the store already holds. The bundle is split: **rule 10
ships in v1**, rule 11 stays in v0.5 with the Events watch it genuinely needs.
Confirming the case: `broken-pending` was already one of the nine fixtures
Phase 2 captures — the data was being collected for a rule that was not going
to ship. N6 (which taint or nodeSelector is blocking it) already computes the
node side of the same question, so the two now arrive together instead of four
releases apart.

Neither item widens scope: no new watch ([invariant 6](CLAUDE.md)), no new
key, no new view. They are the same Alerts list, with two blind spots removed.

### D28 — the workload watch, and the blind spot it closes (2026-08-12)

**Decided: Deployments, StatefulSets and DaemonSets join Pods and Nodes in the
permanent watch set.** This changes [invariant 6](CLAUDE.md), which is why it
is a decision and not a task.

**The blind spot.** Every v1 rule reads a Pod. When the pods were never
created — a ResourceQuota denial, an admission webhook rejection, a missing
PVC, a bad pull secret at ReplicaSet level — there is nothing to iterate over:
`kubectl get pods` is empty, the Deployment sits at 0/3, and k8rs reports a
healthy cluster. It is the most beginner-hostile failure class in Kubernetes
and it was the only one the tool could not see at all. Saying *nothing is
wrong* when something is, is worse than missing a finding: it is the one
behaviour that would make the Alerts screen not believable
([REQUIREMENTS G-6](REQUIREMENTS.md#flagged-by-more-than-one-role-at-once-highest-priority)).

**The second hole, closed by the same objects.**
[D3](#d3--findings-group-by-owner-not-by-pod) groups findings by
Deployment/StatefulSet/DaemonSet/Job, but a pod's `ownerReferences` points at
its *ReplicaSet*. Nothing said where `web-7d4f5c6b8` becomes `web`, so as
planned, M1 would have grouped under a hashed string — in the product whose
rule is that every visible string reads without a glossary.

**What is watched, and what is not.** The three workload kinds are watched for
`metadata` + `status` only (desired vs ready, the `Progressing` condition).
ReplicaSets are **not** watched: they are fetched on demand, cached by UID,
when a finding needs the `ReplicaFailure` message or a group heading needs the
Deployment behind a ReplicaSet. Jobs and CronJobs stay in the v0.2 J-series as
planned.

**Why this does not contradict what invariant 6 protects.** The invariant
exists to keep k8rs off the path that makes k9s heavy: a repeated
`LIST pods -A` ([§ Architecture](#architecture--where-lightweight-comes-from)).
Workload objects are two orders of magnitude fewer than pods and barely churn
— three low-traffic streams, not forty. The line the invariant now draws is
the honest one: *the Alerts view's own inputs are watched; everything the
browser shows is watched only while it is open.*

**New rules, W-series** (in `rules.rs`, pure, same as every other rule):

| # | Finding | Source |
|---|---|---|
| W1 | **The pods were never created** — quota exceeded, admission webhook denied, PVC missing | `ReplicaSet.status.conditions[ReplicaFailure]`, message shown verbatim |
| W2 | **Rollout gave up** — stuck part-way with no failing pod to explain it | `Deployment.status.conditions[Progressing].reason == ProgressDeadlineExceeded` |

W2 fires **only when no pod-level finding already explains the shortfall** —
otherwise a crashlooping Deployment produces two findings for one problem, and
the list stops being believable for the opposite reason.

**Costs, stated:** invariant 6 is rewritten; the 10 000-pod memory measurement
([D25](#d25--what-this-review-did-not-decide)) now includes the three kinds;
the read-only `ClusterRole` in [docs/security.md](docs/security.md) needs no
change — it already grants `get`/`list`/`watch` on all four `apps` resources,
so the least-privilege claim holds unchanged (checked 2026-08-12).

### D29 — a guard is proven only for the shapes it was fed (2026-08-12)

Found by auditing Phase 2 before running it, not by a failing build — which is
the point: nothing was failing.

**What was wrong.** `scripts/sanitize.jq` was written against a single Kubernetes
object: `del(.metadata.annotations)`, `.spec.containers[]?`, `.spec.nodeName`.
Half of `just fixtures` is `kubectl get <kind> -A -o json`, which returns a
**`List`** — the real objects sit under `.items[]`, a workload's containers two
levels below that under `.spec.template.spec`, and the top-level `.kind` is
`"List"`, not `"Node"`. Every path clause therefore addressed the wrapper and
missed the contents. Fed a poisoned List, the filter exited **0** and left
behind: the `last-applied-configuration` annotation, every env value, the
imagePullSecret, the selfLink, the node IPs — and it did **not** refuse a
capture whose node names came from a production cluster, because the refusal
tested `.kind == "Node" or .spec.nodeName != ""` at the top level only.

Eight of the fixtures `just fixtures` writes are that shape, `nodes.json` —
the most identity-carrying file in the set — among them.

**Why no test caught it.** `sanitize-test.sh` fed the filter one Pod. The Pod
path worked, so the test was green, and it would have stayed green through the
capture. This is the failure mode CLAUDE.md's *tests must not lie* already
names, arriving through a door it had not: not a test that cannot fail, but a
test that covers one of the two shapes the pipeline produces.

**The fix, and the shape of it.** The filter no longer contains a path-based
clause. `del(.. | objects | .annotations?, .managedFields?, …)` and a `walk`
over anything carrying `.env` reach every depth — List, pod template, bare
object — in one expression, which is also shorter than what it replaced. Node
identity is collected with `..` as well, and a Node is recognised by
`.status.nodeInfo` rather than by `.kind`, because the items of a List do not
reliably carry one. The refusal is now **any** foreign identifier, not *all*:
one real node name inside an otherwise-kind capture is the leak.

`sanitize-test.sh` feeds both shapes, plus a mixed List that must not be
laundered by the kind-shaped name sitting next to the foreign one. Run against
the old filter it reports eight failures; against the new one, none.

**Two rules came out of this**, both now in [CLAUDE.md](CLAUDE.md) rather than
left as a lesson learned:

1. **A check is proven only for the input shapes it was fed.** Enumerate what
   the real pipeline hands it, feed it each one.
2. **A derived list asserts it found something.** `write-guard.py` builds its
   ban list by parsing kube's `Api<K>` signatures; a `&self` wrapped onto its
   own line is not matched, so the method silently never gets banned — and
   "extracted nothing" prints the same as "nothing to extract". It now asserts
   `delete`/`patch`/`replace`/`create` are present before trusting the list,
   and its `--self-test` proves the wrapped-signature hole exists rather than
   assuming it does not.

**A third finding, same audit:** `just check` was documented as byte-for-byte
CI and was not. It omitted `test-guard.py --self-test` and the whole
`cargo deny` job — the reason cargo-deny's licence failure in Phase 1 could
only surface on a push. Both are in `just check` now, cargo-deny last so a
machine without it still gets the other nine results.

**And a fourth:** `just fixtures` deleted `broken-stuck` with `--wait=false`
and captured it immediately, never asserting the `deletionTimestamp` that rule
12 reads. `cluster.sh verify` checks the finalizer, but it runs *before* the
delete, so nothing checked the state actually being captured. The capture now
asserts it.

### D30 — the guards Phase 2 added, and the freeze they collided with (2026-08-12)

Four findings from finishing the Phase 2 audit, one of which is a plan change
rather than a fix.

**1. The CI yaml is frozen, and Phase 2 had to touch it anyway.** Phase 1 lists
the CI workflow among the files that freeze after it. Phase 2 then produced two
new security tests — `scripts/certs-test.sh` here, `scripts/verify-test.sh`
alongside it — and a security test that CI does not run is not a security test.
[CLAUDE.md](CLAUDE.md) now also requires `just check` to be the whole of CI, so
leaving CI behind would break a rule in the same breath. The freeze is
therefore broken deliberately, minimally (one `run:` step per test, no
restructuring) and in writing, which is what the forward-only rule asks for.

**The structural fix was not taken, and is the owner's call.** CI enumerates
its steps in a second list that has already drifted from `just check` once
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)). A
CI job that installed `just` and ran `just check` would make the drift
impossible instead of merely fixed, and would end the freeze collision for
good — every future check would land in one file. It costs one pinned
third-party action. Recorded, not applied.

**2. Addresses are stripped now, not eyeballed.** Phase 2's checklist asks a
human to confirm "no node IPs" in the committed fixtures, and a real capture
showed the sanitizer kept every one of them: `status.addresses[].address` on
nodes, `podIP` / `podIPs` / `hostIP` on pods. An eyeball step is not a guard —
it passes whenever the person running it is tired, which at that point in a
capture is everyone. `sanitize.jq` now redacts any value that is an IP address
as a whole string, which leaves the `Hostname` entry sitting in the same
`addresses` array untouched, because that one is the node name the N-series
joins on. The eyeball item stays; it just no longer carries the weight alone.

**3. `managedFields` can never reach a fixture, so it can never be tested
there.** `kubectl get -o json` omits them unless asked
(`--show-managed-fields=true` returns them, the default returns none), and the
sanitizer deletes them regardless. Both are correct. The consequence is not:
[docs/architecture.md](docs/architecture.md) claimed *"fixtures deserialize
through `k8s_openapi::Pod`, so the prune path is covered by the same tests"* —
it is not, and a test asserting "managedFields were pruned" would pass against
a fixture that never had any. The prune path is a `k8s.rs` concern and needs a
Phase 5 test against live watch data, where the field actually arrives. The
29% measurement in
[§ Verified against a real cluster](#verified-against-a-real-cluster-2026-08-11)
still stands — it was taken over the wire, which is where pruning pays.

**4. openssl writes the private key before it validates the dates.** Found
while verifying that the certificate fixtures carry the pinned dates
`todo.md` claims — they do, all three, exactly. But a malformed `-not_after`
makes openssl emit `expiring-client.key.pem` and *then* fail, and `set -e`
skips the `rm` that was supposed to remove it, leaving key material in the
fixture directory. A `trap … EXIT` covers the unhappy path now. The same edit
stopped discarding openssl's stderr: on an openssl older than 3.5 the run fails
with `Extra (unknown) options: "not_before"`, and hiding that message is what
would send the next reader reaching for a relative `-days`.

**One thing this produced for Phase 3:** the pinned `notBefore` of the expiring
and healthy fixtures is *exactly* the reference `now` of 2026-08-12, so C1's
validity comparison has to be inclusive (`notBefore <= now`). A strict `<`
classifies both fixtures as not-yet-valid, which is a third C1 case and not the
one either fixture is for.

### D31 — the sanitizer matched the whole string, and secrets are rarely the whole string (2026-08-12)

D29 fixed *where* the filter looked — every depth instead of a fixed path. This
is the other half: *how much* of a string it looked at. Both gaps were found by
scanning for the pattern rather than by a failing build, and both were shipped
with a green `sanitize-test.sh`.

**1. An address is not always the entire value.** The IPv4 rule was anchored
(`^…$`), so it caught `"10.244.2.2"` and missed both shapes a real capture
actually contains:

- `"10.244.0.0/24"` — a `podCIDR`, an address wearing a suffix. Six of them sat
  in the committed `nodes.json`.
- `"…dial tcp 172.18.0.1:53: no such host"` — an address quoted inside an
  English message, which is where kubelet puts the one it could not reach. One
  sat in the committed `image.json`.

`sanitize.jq`'s own comment argued the gap away: *"An address quoted inside an
English message is not caught; the foreign-capture refusal above is what covers
a capture from a cluster that is not kind."* That reasoning does not hold. The
refusal keys on **node names**, and this cluster's nodes are called `k8rs-*` no
matter what address the apiserver was given. `K8RS_APISERVER_ADDRESS` exists
precisely so the cluster can live on a real LAN — so a capture from a perfectly
legitimate kind cluster passes the refusal and carries that LAN address out
inside a message. The addresses actually found were kind and Docker defaults and
gave nothing away, which is luck, not a control.

IPv4 is now replaced *inside* strings. IPv6 stays anchored on purpose:
unanchored, `::` matches a Rust path, a C++ scope operator and every
`key::value` in a log line.

**2. Key material is never text when it arrives.** Every Secret value is
base64 in JSON, and base64 contains no `-----BEGIN`, so the PEM rule read a
`.data["tls.key"]` as ordinary prose and handed it straight back. Demonstrated
with one object carrying the same private key twice: the plain copy came back
`REDACTED-PEM`, the encoded copy came back untouched. The same blindness covers
`certificate-authority-data` in a captured kubeconfig and ServiceAccount token
Secrets.

Only the **key** half is redacted. A certificate is the public half by
definition, and `csr-pending.json`'s `.spec.request` is typed as a ByteString —
redacting it would leave C3's own fixture unparseable. Decoding is guarded
rather than attempted: jq's `@base64d` is a hard error on input it cannot
decode, which would abort the whole capture, so a string reaches it only after
matching the encoded PEM header, the base64 alphabet, and a whole-group length.

The replacement is deliberately not valid base64. Nothing here has a legitimate
private key in a fixture, so the only object this can fire on is one that should
never have been captured, and it should fail loudly at the next parse rather
than deserialize into a tidy placeholder nobody looks at.

**`fixture-audit.sh` had both holes too**, being written from the same two
regexes — so the guard that exists to catch what the filter misses was blind in
exactly the same places. It now checks the committed bytes for embedded
addresses and for base64-wrapped keys, and both checks were watched failing on
planted input before being trusted.

**The rule this adds to CLAUDE.md's *tests must not lie*:** *a redaction proves
only the framing it was written for.* D29 asked which shapes the pipeline
produces. This asks where inside a value the secret can sit — whole string,
substring, or another encoding of it — and requires one planted case per answer.

**3. The same question, asked of the refusal (2026-08-12, second pass).** The
foreign-cluster refusal read exactly two fields: `.nodeName`, and a Node's own
`.metadata.name`. A node name identifies infrastructure in four more places,
all of them ordinary:

- `.status.nominatedNodeName` — where the scheduler parks a name before it
  commits to it
- `.spec.nodeSelector["kubernetes.io/hostname"]`
- `.metadata.labels["kubernetes.io/hostname"]` — which the committed
  `nodes.json` carries for all three nodes, so this is not a hypothetical field
- `matchExpressions[] | select(.key == "kubernetes.io/hostname") | .values[]`

A production capture reaching the sanitizer through any of them was sanitized
and written instead of refused. All four are now collected, each with its own
assertion so a partial fix cannot pass. What was **not** done is Agent-suggested
and deliberately rejected: making the refusal positive — "refuse when no node
identifier is found but a hostname-shaped string exists" — would refuse
`deployments.json` and `services.json`, which legitimately contain no node
field at all.

**4. IPv6 written out in full** (`2001:0db8:0000:…:8329`) carries no `::`, so
it matched neither branch. Added as a second anchored alternative; unanchored
IPv6 is still refused as a matter of policy, because `::` alone matches a Rust
path and every `key::value` in a log line.

**5. `check-docs.py` fenced on ``` only.** A heading inside a `~~~` block became
an anchor here and does not on GitHub — the one way that script could call a
genuinely broken link green. It now remembers which marker opened the fence
rather than toggling a boolean, so a ``` inside a `~~~` block does not close it,
and it gained the `--self-test` the other three guards already had.

### D32 — one long-lived `development` branch, not one per phase (2026-08-12)

The branch-per-phase model (`feat/rules`, `feat/analysis`, … — named in
todo.md's phase headings since the design phase) is replaced by a single
long-lived `development` branch. Every box commits onto it; at phase close it
is merged into `main` by [CLAUDE.md § phase close](CLAUDE.md), item 7;
`development` is never deleted and work continues on it immediately.

**Why.** The per-phase branch was invented before anything had been built on
it, and by the time two phases had shipped it had cost more than it returned:
it added a create step, a delete step and a "which branch am I on" question to
every phase, and the delete step failed twice on tooling that refuses a remote
branch deletion. It also duplicated a boundary that already exists and is
already enforced — the phase itself, in todo.md.

**What it buys with the agents.** A rule already in force is that agents never
create, switch or delete a branch; they write files on whatever branch they are
handed. With one permanent branch that rule stops being a restriction anyone
can trip over, because the branch is simply always there.

**What it costs, and where that cost is now carried.** A branch per phase made
the freeze mechanically visible: `git diff main...feat/rules` was exactly one
phase, and a frozen file reappearing in it was impossible to miss. One shared
branch does not give that for free. The freeze therefore rests entirely on two
things that were already required — the PM reading every diff before it is
committed, and the phase-close second pass reading the phase end to end. If a
frozen file ever does slip through, this is the decision that made it possible,
and the answer is a guard, not a branch.

**Direction, and why no back-merge.** `main` only ever advances by merging
`development` into it. That keeps `development` an ancestor of `main` at all
times, so nothing has to be merged back; the moment someone commits directly to
`main`, that stops being true and this note stops being accurate.

Phases 1 and 2 ran on `feat/scaffold` and `feat/fixtures` and their headings
still say so — that is what happened, and rewriting it would make the plan
file lie about its own history.

### D33 — Phase 3 opens with one Phase 2 box still open, on purpose (2026-08-12)

Phase 2's last box — `just cluster-down` on the LAN host — is unchecked and
stays that way while Phase 3 runs. Phase 3 starts anyway. That is a plan
change, so it is written here rather than applied by quietly ticking the box.

**Why the box cannot be closed yet.** The box's own condition is "run it once
no further capture needs the cluster", and a capture still does: Phase 4's
Drain-safety and Waste reports have a negative fixture and no positive one,
because nothing in `broken.yaml` produces a StatefulSet, a PVC, a PDB or a
dead-selector Service. Tearing the cluster down now buys nothing and costs a
rebuild.

**Why Phase 3 does not have to wait for it.** Phase 3 is pure functions over
fixtures that are already captured, sanitized, audited and committed — 23 of
them, from `v1.36.1`. It needs no cluster, no network and no terminal
([invariant 5](CLAUDE.md)). The open box blocks nothing in it.

**Amended the same day: the host went down in a power cut and is unreachable,
so whether that cluster is still standing is now unknown.** It changes nothing
here, and that is the point of the pinned node image: `just cluster-up`
rebuilds the same `v1.36.1` cluster from `scripts/cluster.sh`, so the visit
that closes this box is a re-create if it has to be. Nothing was lost either
way — the fixtures live in git, not on the host. What the outage does prove is
that leaving a box open against a machine you do not control is the correct
record: had it been ticked on "the teardown is basically done", the plan would
now be claiming a step nobody can show ran.

**Where the trip now happens, and how it leaves nothing behind (2026-08-12).**
With the host gone, the rebuild runs on the developer's own machine at the
Phase 3 close — one visit that captures Phase 4's four missing fixtures and
then tears the cluster down, closing this box. It is set up to evaporate: the
docker socket is granted for the session with
`sudo setfacl -m u:$USER:rw /var/run/docker.sock`, which a docker restart
undoes, **not** by adding the user to the `docker` group — that group is
root-equivalent and would be a permanent privilege grant bought for a
half-hour capture. `KUBECONFIG` points at a scratch path so kind never writes
`~/.kube/config`, and `just cluster-down` removes the containers. Nothing
about the capture depends on this: `scripts/cluster.sh` pins the node image,
so the cluster is the same `v1.36.1` wherever it is built.

**What this is not.** It is not permission to start a phase on top of unfinished
work in general. The exception is narrow and holds only because the open box
and everything in Phase 3 are disjoint: no Phase 3 box reads anything the
teardown would change. Phase 2 remains formally open until the teardown runs
and Phase 4's missing fixtures are captured in the same visit to the cluster —
an honest open box, which [CLAUDE.md § phase close](CLAUDE.md) item 2 prefers
to a false tick.

### D34 — the temporary `main.rs` belongs to `dev-core` until Phase 12 (2026-08-12)

[CLAUDE.md § Ownership](CLAUDE.md#ownership--and-the-file-each-one-may-write) gave
`main.rs` to `dev-ui`, and the phase map in the same section gave Phases 3–7 to
`dev-core` — while Phase 3, Phase 5 and Phase 7 each write a box into `main.rs`
(the fixture-printing shell, the `--once` output, the headless ops driver). Two
sentences that cannot both be obeyed, found the first time a box needed the
file. The table is corrected rather than worked around.

**`main.rs` is `dev-core`'s while it is the temporary driver, and passes to
`dev-ui` at Phase 12, where it is wired for real.** The driver's boxes are in
Phases 3, 5 and 7, but naming that range as the ownership window leaves 8–11
with no owner at all — the same hole one phase further on. No box in those four
phases touches the file, and the handover stays where it was decided. Nothing
about it draws before then: it parses `args`, loads a fixture or a client, and
prints.
The file is the one place the ownership rule has a handover, and the handover is
a phase boundary, which is exactly where the plan already stops.

**The mechanical reason it cannot simply be skipped.** A file that no `mod`
declaration reaches is not part of the crate, and `cargo fmt` never visits it —
the correction Phase 1's guard ledger already paid for once. So `src/rules.rs`
does not exist as far as fmt, clippy or the test runner are concerned until
`mod rules;` is in `main.rs`. Writing the module and the line that reaches it are
one change, not two, and they cannot belong to two different agents.

### D35 — `just mutants` is a check that cannot fail, and the justfile unfreezes for one line (2026-08-12)

Three findings came back from the test gate on Phase 3's first box. They are
closed here, because a finding closed in conversation is a finding nobody can
find again.

**Accepted, applied one box later.** `cargo mutants --file src/rules.rs --file
src/analysis.rs` prints `Found 0 mutants to test` and **exits 0** — and it does
exactly the same for `--file src/does-not-exist.rs`, which was tried. That is
the derived-list failure [CLAUDE.md § Code phase rules](CLAUDE.md) names: a
path typo and a clean run print the same line. It needs a floor assertion on
the mutant count. It is **not** applied in this box, because today the zero is
honest — `rules.rs` holds types and no functions — so a floor now would be a
red gate nobody could pass, and an impassable gate is the thing that teaches
everyone gates are decorative. It lands in the box that first puts a function
in `rules.rs`, written by `tester`. **The `justfile` froze at the Phase 2
close; it unfreezes for that one line and nothing else** — recorded here rather
than done quietly, which is what the forward-only rule asks for.

**Fixed now.** `.gitignore` had no `mutants.out` entry, so every `just mutants`
run left `mutants.out/` and `mutants.out.old/` untracked in the repo root.

**Rejected, with the reason.** A variant inserted *above* `Critical` leaves the
severity-order test green. That is correct behaviour, not a hole: the test's
claim is "declaration order is severity order", and inserting a variant keeps
it true. A test pinned to the variant set instead would go red on a legitimate
addition — which is a different lie, and a more expensive one, because the
person who adds a severity would learn to edit the test to get past it.

### D36 — the `Finding` shape the review sent back (2026-08-12)

Phase 3's first box was written exactly to the shape
[docs/architecture.md § The shared contract](docs/architecture.md#the-shared-contract)
specified, and the operator review blocked it on three counts. The code was
faithful; **the documented contract was the thing that was wrong**, which is
why this is a decision and not a bug fix. The box exists to decide the identity
every later rule files under, and `rules.rs` freezes at the end of this phase —
so the shape is settled here, before there is anything to migrate.

**What was missing, and the concrete case that proves each.**

- **The pod the finding is about.** One crashlooping pod fires rules 1, 5, 6 and
  7 — checked against this repo's own `tests/fixtures/crashloop.json`, which is
  ready=false, 5 restarts, `CrashLoopBackOff`, exit 1. A `views.rs` holding only
  `Vec<Finding>` can count findings, not pods, so the card renders **"4 of 5
  pods" for a single broken pod** — the blast radius, wrong by 4× on the most
  common failure in the rule set. `Finding` now carries `object` beside `owner`:
  the numerator is the count of distinct `object`s. It is also the only possible
  data source for [detail.md](screens/detail.md)'s "⏎ lists which pods of the
  group are affected", which had none.
- **A Node.** N1–N6 are boxes in this same phase and a Node is cluster-scoped.
  The old shape could only say `{Pod, "", "k8rs-worker2"}`, which renders
  `/k8rs-worker2`, offers `logs` and a container picker for a machine, and
  builds `kubectl describe pod k8rs-worker2 -n ""` — invariant 4 lying in the
  one record that may not. `OwnerKind::Node` exists and `namespace` is
  `Option<String>`, `None` meaning cluster-scoped.
- **Owners that are not one of five kinds.** A CronJob's pods are owned by a
  Job whose name changes every run, so filing under `Job` grows a fresh card
  every schedule tick — the flood [D3](#d3--findings-group-by-owner-not-by-pod)
  exists to prevent. `--cascade=orphan` leaves pods whose chain stops at a
  ReplicaSet, and under Argo Rollouts or any database operator the
  ownerReference kind is a CRD. `CronJob`, `ReplicaSet` and `Other(String)`
  cover all three; `Other` prints the kind it actually got instead of asserting
  a false one. Resolving ReplicaSet → Deployment stays `k8s.rs`'s job
  ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)) —
  this type only has to hold either answer.

**Two `String`s became `Option`s for the same reason.** `kubectl_cmd` was
empty-as-absent, so "no such command exists" (rule C1 reads a local file) and
"the rule author forgot" were the same value, indistinguishable by any test.
`uid` was absent entirely, and [D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms)
requires the confirm dialog to hold one: in the Alerts view the selected object
*is* a `Finding`, so without it a card drawn before an Argo re-sync can scale
the object that replaced the one the operator inspected. The struct already
made this argument in its own doc comment — `Option<Owner>` would be "the arm
every call site forgets" — and then took the opposite side one field down.

**Rejected: `resourceVersion` does not go on `Finding`.** D22 names it beside
the uid, but it belongs to the moment the dialog opens, not the moment the rule
ran; a rule-time value would be stale by construction and would invite a
comparison that means nothing. The uid answers "is this still the same object",
which is the question a stale card actually poses. Recorded because the field's
absence otherwise reads as an oversight.

**The gap this opened, which no code change closes.** Every pod fixture in the
repo has `ownerReferences: null` — all twelve, checked one by one; only the two
ReplicaSet captures carry an owner at all. `scripts/broken.yaml` creates bare
pods, so **every positive test in Phase 3 files under `OwnerKind::Pod` and the
four workload branches ship with no positive fixture** — and `cargo mutants`
would not object, because nothing exercises them. A grouping key tested only in
its no-owner case is [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
again, one layer up. `broken.yaml` needs a Deployment-owned broken pod, which
needs the cluster — so it is a new open box in Phase 2, captured on the same
trip as [D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)'s
teardown, not a note in a review nobody reads again.

### D37 — a controller's message is a status field, not a payload (2026-08-12)

Two recorded rules collided the moment `Finding.evidence` was written down.
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) says W1
shows the controller's message **verbatim** — "exceeded quota: pods, used 0,
limited 0" is the whole diagnosis, and paraphrasing it is how a tool becomes
useless. The `evidence` field's own rule says a finding names **fields, never
payloads**. A validating webhook that echoes the submitted object back into its
denial message — several in the wild do — puts an env value inside a controller
message, and now both rules apply to one string.

**Verbatim wins, and the payload rule is narrowed to say what it always meant.**
The rule governs what *k8rs goes and fetches*: it never reads Secret data, never
renders an env value, never puts either in evidence — that is absolute and it is
the [security gate](CLAUDE.md#security-gate--run-this-list-on-every-change-no-exceptions)'s
"environment variable values are never displayed". A message a controller wrote
into a status the user can already `kubectl describe` is a different thing: k8rs
did not go looking for it, it is not privileged relative to the reader, and
hiding it would leave the user staring at "the pods were never created" with the
reason blanked out. Refusing to show what `kubectl` shows is not a security
control, it is a tool that lies by omission.

**What that costs, and what pays for it.** The message is untrusted free text,
so it takes the same treatment every other API string takes: control characters
stripped at the render boundary ([invariant 9](CLAUDE.md)) and length bounded at
ingest, which Phase 5 already owes for the 50MB-annotation case. The one thing
that changes is a warning where it belongs: `--once` output is what gets
redirected into CI logs and pasted into tickets, so it is the place a webhook's
echo reaches an audience wider than the person at the terminal. That is a
documentation line for the `--once` box, not a reason to blank the field.

**Addendum to [D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) —
how a cluster-scoped finding renders.** Fixing the type left the mockups
asserting the opposite, so `screens/` moved with it: the identity line is
`namespace/name` for a namespaced object and a bare `name` for a cluster-scoped
one, stated once in [screens/README.md](screens/README.md) rather than fixed in
the one place it was caught. Three consequences were settled there, all worth
keeping: dropping `infra/` also drops the only clue that `node-3` is a machine,
so **the kind moved into the sentence under the name** (N2 now reads "This node
refuses new pods (cordoned)"); it is `node-3`, **not** `node/node-3`, because a
`kind/name` prefix puts a slash back on the very line whose slash the reader was
just taught means a namespace; and **N6's card is a workload card, not a node
card** — a Pending pod that cannot be placed is about the pod, with the node
named in the evidence, so only N1–N3 produce a node card at all. The drain
dialog was the proof that this was never cosmetic: its title said
`Draining infra/node-3` while the command log under it said
`kubectl drain node-3`, and the fake namespace was what hid the disagreement.

### D38 — the grouping key was a derive, and a derive cannot be told what to ignore (2026-08-12)

The shape [D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) settled
carried one defect no compiler could catch. `ObjectId` derived `Hash` and `Eq`
over all four fields, `uid` among them, and `views.rs` groups by `owner` — so
these are two different grouping keys:

```
ObjectId { kind: Deployment, namespace: Some("payments"), name: "web", uid: Some("9f2c-aaaa") }
ObjectId { kind: Deployment, namespace: Some("payments"), name: "web", uid: Some("9f2c-bbbb") }
```

Two cards for one Deployment: the flood [D3](#d3--findings-group-by-owner-not-by-pod)
exists to prevent, arriving through the field added to prevent a *different*
bug. It compiles, the suite passes, and it would surface in Phase 9, months
after `rules.rs` froze.

**The mechanism above is the second one written here; the first was wrong and
the correction is the useful part.** This entry originally said the two uids
were `Some` and `None` — a rule that resolved ReplicaSet → Deployment without
the on-demand fetch having the name but no uid. The operator review killed it:
`uid` is a *required* field of `metav1.OwnerReference`, non-`Option` in
`k8s-openapi` and present in this repo's own `quota-replicasets.json`, and
without the fetch there is no Deployment **name** either, because `web` is not
derivable from `web-7d4f5c6b8`. The pair was hand-built for a test and then
asserted as though it came off a cluster. The reachable case is a **Deployment
deleted and recreated under the same name**: old-generation pods still
terminating under uid-A while new pods run under uid-B, which is what any Argo
prune-and-recreate produces. Same fix, honest reason — and the difference is
not academic, because the old wording told the `k8s.rs` author that a workload
owner with `uid: None` was normal, and `uid: None` is the one value that
silently disables [D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms).
**A workload owner always carries a uid; C1's kubeconfig certificate is the
only `None` in the product.**

**`Hash` is dropped from `ObjectId` and the identity becomes a method** —
`group_key() -> (&ObjectKind, Option<&str>, &str)`. Not a doc comment asking
callers to be careful: the wrong key stops compiling, so it is unrepresentable
rather than discouraged. Two details, both found by probing rather than by
reading: the wall is not the declaration — `HashMap<ObjectId, _>` *declares*
fine, because the `Hash` bound sits on `insert`/`get`/`entry`, not on
`HashMap::new` — and when it does fire, rustc says
`help: consider annotating ObjectId with #[derive(Hash)]`, offering the
two-cards bug as the fix in the text a future developer reads before any doc
comment. The comment in `rules.rs` pre-empts that advice by name. It also makes the
box's own sentence mechanical — the identity is decided in the bottom layer
instead of re-derived in `views.rs`, where a second definition would drift.

**`Eq` stays derived over all four fields, on purpose.** It answers
[D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms)'s question — is
the object about to be mutated still the one the operator inspected — and an
`Eq` that quietly ignored the uid would answer *that* wrong, which is worse than
the bug being fixed. Two questions, two mechanisms: the derive for identity of
the object, the method for identity of the card.

**The gate that found it was itself broken, which is the more expensive half.**
Hunting this, `tester` deleted every test from `rules.rs` and ran the guard:
`test-guard: 0 declared, 0 listed, 0 ignored — OK`, `cargo test` green,
`just check` green. **The whole suite could be deleted and CI would applaud.**
Every rule in the guard was a comparison between two counts, and comparisons all
hold at zero — [D26](#d26--a-green-build-that-proves-nothing-2026-08-12) item 2
said "CI asserts a non-zero test count" and nothing ever did, in `just check` or
in the workflow. The floor is in, with a positive and a negative self-test, and
it is passable today, which is what separates it from the `just mutants` floor
[D35](#d35--just-mutants-is-a-check-that-cannot-fail-and-the-justfile-unfreezes-for-one-line-2026-08-12)
deferred. A guard whose rules are all relative is the derived-list failure in a
new costume: with nothing to compare, everything agrees.

**One assertion in the new tests has never been red, and that is recorded
rather than hidden.** The negative test's fourth case — cluster-scoped versus
namespaced — cannot fail in any way its "different namespace" case does not
fail first, and it survived the one mutation aimed at it (`group_key`
flattening `None` to `Some("")`). It stays, because it states intent for a
shape the rest of the suite does not name, but by this project's own rule it
proves nothing yet. What would make it real is written into the C1 box in
[todo.md](todo.md): `Some("")` is unreachable only while an `ObjectId`'s
namespace comes from an object's own `metadata.namespace` — measured, not
assumed, against all 23 captures, `sanitize.jq`, and the DNS-1123 label rule
that forbids an empty namespace name. A C1 that builds its identity from the
*effective scope* instead would make it reachable, because `--namespace` is
parsed from `args` with no validation.

**Accepted as a known ceiling, not fixed:** `#![expect(dead_code)]` at the top of
`rules.rs` is module-wide, so an item written *after* it can be dead and
invisible — a never-called function was appended and clippy stayed green.
Narrowing it to per-item would cost a line of noise on every type, so it stays
module-wide.

**Its exit was also mis-stated, and the correction is a pre-authorisation.**
This entry first called the attribute self-limiting, "because the file freezes
with everything constructed". Nothing guarantees that: `Severity::Info` has no
producer in `rules.rs` today, `analysis.rs` gets one in Phase 4, and whether
N4's version skew adds one here is N4's own box. While *any* item stays
unconstructed the expectation is still fulfilled and the line cannot be
removed — removing it makes `dead_code` fire and the build go red. It becomes
**unfulfilled** only when the last item is constructed, which may be phases
later, and `-D warnings` then turns `just check` red pointing at
`src/rules.rs:11` — possibly a file frozen a phase earlier. `clippy
--all-targets` evaluates it per target, and the two targets already disagree:
with the attribute removed, the bin reports five dead items and the test target
two, so they can flip at different boxes. **Whichever box constructs the last
item in this file deletes that one line. The deletion is authorised here, in
advance: it is not a freeze violation and it does not need a new decision** —
which is the point of writing it down while the red build is still hypothetical
and nobody is under pressure to explain it.

### D39 — a Node owns pods, and three more things the shape could not say (2026-08-12)

The second operator review of the same box. The shape held; what did not hold
were the sentences written around it, each falsified by a cluster this repo
builds or a fixture it has already committed.

**A mirror pod is owned by a Node, so "a Node is nobody's owner" was false on
`just cluster-up`'s own cluster.** kubelet writes an `ownerReference` of kind
`Node` onto every static pod, which on kind and on any kubeadm cluster means
`kube-apiserver-*`, `etcd-*`, `kube-scheduler-*` and `kube-controller-manager-*`
all have one. Left alone, an `etcd` that restarts four times after a laptop
suspend files under `{ kind: Node, namespace: None, name: "k8rs-control-plane" }`
— `kube-system` gone from the card a beginner needs in order to find the pod
again, four control-plane pods collapsed onto one card, and, since `views.rs`
picks the card shape from `owner.kind`, a machine-shaped card drawn for a
crashing pod. **An `ownerReference` of kind `Node` is discarded: the pod files
under itself, so `owner.kind` is `Pod` and `owner == object`.** A mirror pod
has no controller that can be scaled, restarted or rolled, and the card stays
`kube-system/etcd-k8rs-control-plane`. `ObjectKind::Node` appears in `owner`
only when the finding is *about* the node — N1–N3. Stated that mechanically on
purpose: the first wording said "a `Node` in the owner role is the no-owner
case", which reads as an instruction to *keep* the Node, and an implementer
obeying it arrives at the exact card this ruling forbids. The upstream behaviour is documented but asserted by nobody here, so
the same trip that captures the owned pod captures `-n kube-system` too.

**The numerator rule printed "1 of 0 pods".** `object`'s doc defined the
numerator as distinct `object.name`s in the group, which holds only while every
object in it is a pod. W1's object is a **ReplicaSet** — `quota-replicasets.json`
is `broken-quota-59654c756`, `ReplicaFailure`, `status.replicas: 0` — so the
group renders 1 of 0, on the failure class
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) added
the workload watch to stop the tool lying about; W2 breaks it the other way and
counts a Deployment as one of its own pods. **The numerator counts distinct
objects whose kind is `Pod`, and a group with none of them has no `n of m` at
all** — the shape a node card already uses. **Distinct by the whole `object`,
not by `group_key()`**, which is the half that had to be said out loud:
`ObjectId` has no `Hash`, the type's own doc answers that compile error with
"use `group_key()`", and a reader following that advice while *counting* writes
the wrong thing. Not a bug being prevented — for objects of kind `Pod` the two
are provably identical on any snapshot, since `group_key` is
`(kind, namespace, name)` and two pods differ in it exactly when their names
differ. An intent being stated: `group_key()` answers *which card*, the whole
identity answers *what is counted on it*, and code that says the first while
meaning the second stops being right the moment something other than a pod is
counted.

**The first draft of that paragraph justified it with a scenario that cannot
happen** — a terminating `web-0` counted beside the `web-0` that replaced it.
Two pods cannot share a name in one namespace: the name is the etcd key, the
second create is rejected `AlreadyExists`, and a StatefulSet will not recreate
an ordinal until the terminating pod is gone from the API. It is the defect
[D38](#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12)
recorded one round earlier — a pair built by hand for an argument and then told
as though it came off a cluster — repeated one field down by the same author,
in the same week, having just written the correction. Worth keeping visible for
that reason: the owner-uid divergence one field up *is* real, and the two look
identical on the page. What separates them is that the two Deployment
generations have different uids and their pods have different **names**, which
is exactly why those pods coexist and two pods called `web-0` never do.
[D4](#d4--the-flagship-example-promised-a-number-that-cannot-exist) is the
precedent: a number that cannot exist is not a rendering detail.

**`CronJob` was unreachable under this project's own least-privilege role.** A
pod names its Job and says nothing about the CronJob above it, so the grouping
needs a GET on the Job — and `k8rs-readonly` granted `""`, `apps`, `policy` and
`metrics.k8s.io`, with no `batch`. Under the role the docs tell people to use,
every tick would file under its own Job name: the churn the variant was added
to prevent, delivered to the user least equipped to explain it.
[docs/security.md](docs/security.md#rbac) now grants `batch: jobs` read verbs,
and the 403 degrades by name — the finding files under the Job and says the
CronJob could not be read. `cronjobs` is deliberately not granted: the Job's
ownerReference already carries the CronJob's kind, name and uid, so nothing
reads the object, and a role whose entire argument is least privilege does not
get to carry a resource it never GETs. Auditing that block found two more of
the same class, both fixed in the same edit — `certificates.k8s.io` for rule C3
and `discovery.k8s.io/endpointslices` for the waste report, each a Phase 5 box
with its fixture already committed, each a 403 waiting for the user who
followed the documentation. `cert-manager.io` stays out and is written into the
block as a commented-out opt-in, because C4 only exists where cert-manager is.
The lesson is the same one
[D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) learned about
docs: this rework synced `docs/architecture.md` and `REQUIREMENTS.md` for the
shape and forgot the file that says what permissions the shape needs.

**Two smaller rulings, both written into `rules.rs` because it freezes first.**
The members of a group must agree on the uid; where they disagree — the
recreate case again — the confirm dialog refuses and offers a re-read rather
than picking one, because the natural implementation is `.first()` and
`.first()` is whichever finding happened to sort first, which turns D22 into a
coin flip that reports "already gone" for a Deployment that is running. And
C1's identity is settled: `kind: Other("kubeconfig")`, `namespace: None`,
`name` = the kubeconfig **context name**, `uid: None` — otherwise its author
must either invent a kind the API never reported or put a sentence in a field
that kubectl lines are built from.

**The file was also two-thirds prose, restating these entries in slightly
different words.** CLAUDE.md keeps the *why* in NOTES and asks for comments
sparingly; a second copy in another wording is a drift generator, and this file
freezes with both copies in it. The comments that stay are the ones the
compiler cannot say — chiefly that rustc answers the missing `Hash` with
`help: consider annotating ObjectId with #[derive(Hash)]`, offering the
two-cards bug as the fix — plus the rulings a later box must obey, each
pointing here instead of re-arguing.

### D40 — the capture could not produce the shape, so the test sets one field (2026-08-12)

Phase 3's second box decoded the twelve committed pod captures into
`PodSnapshot` and every test passed. `tester` then corrupted the decode one
field at a time — 94 mutations, each applied to a pristine copy, compiled, run
and reverted against a hash — and **32 fields could be broken with the whole
suite staying green**. `PodSnapshot.owner` replaced by `id.clone()`:
`19 passed; 0 failed`. [D3](#d3--findings-group-by-owner-not-by-pod)'s one card per owner, the
premise the entire Alerts view is built on, was proven on a ReplicaSet and
never once on a pod.

**The cause is not the tests, it is the cluster.** Not one captured pod carries
an `ownerReferences` at all; no node in `nodes.json` is NotReady, cordoned or
under pressure; every workload fixture is either fully ready or all-null, so
`desired`/`ready` could be read from five different wrong status fields
undetected; the single hostPath mount carries no `readOnly` key, and rule 8
fires *on* writable, so a decode hardwiring "writable" is a false-positive
generator that passes; `allocatable` equalled `capacity` on all three kind
nodes, and the gap between them is the whole of N5. A branch whose input the
capture cannot contain is a branch no test can reach — and the obvious fix is
the one CLAUDE.md forbids, because hand-written JSON resembles reality and is
not it.

**Ruling: a decode test may start from a committed capture and set one field.**
The precedent was already in the box's own diff —
`a_forced_delete_beats_the_grace_period_the_spec_asked_for` takes the real
`stuck.json` and sets the grace period a `--grace-period=0` would have set. Made
general, with the bounds that are the difference between this and the thing it
resembles:

- it starts from a **committed capture**, never a literal object;
- **one field, or one coherent group of fields, per test**, and the comment says
  what value, why the API produces it, and why today's capture does not;
- **a third case the first draft of this ruling did not name**: a test proving
  *which* field is read must set the neighbours too, and the count is then not a
  measure of anything. `desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour`
  sets sixteen fields across three objects, because setting only
  `readyReplicas` cannot show that `ready` is not read from `availableReplicas`
  — five sibling counters, five **distinct** values, and every one of the four
  wrong-field decodes dies on it. That is not a coherence group, it is the
  discrimination mechanism, and it is the shape the rule wants most;
- the whole synthesized object must still be one **the API could emit**. The
  NotReady node synthesis set `DiskPressure` to `True` with the reason
  `KubeletHasDiskPressure` and left the captured message reading *"kubelet has
  no disk pressure"* — nothing asserted it, so nothing was red, but the licence
  for this entire technique is "a value the API demonstrably produces" and half
  an object is not one;
- it is a **decode** test. A *rule's* positive fixture stays a real capture —
  this never becomes the way a rule gets proven, which is the whole reason the
  Phase 2 capture trip exists;
- each such test **names the object the capture trip should bring back** to
  replace it, so the list of what the cluster still owes is in the tests rather
  than in someone's memory.

`From<StatefulSet>` is the one hole this cannot close — `statefulsets.json` is
an empty list, and synthesising a whole StatefulSet is hand-written JSON with
extra steps. It stays untested, says so in the code, and goes on the trip.

**Two syntheses are permanent, and saying so is part of the ruling** — a
"replace me later" that can never be replaced is the kind of note that decays
into furniture. A node whose `allocatable` differs from its `capacity` requires
`--kube-reserved` on the kubelet: kind reserves nothing, so that is a cluster
configuration change rather than a workload the capture trip can apply, and N5's
whole subject is the gap between the two numbers. A non-controlling
`ownerReference` is the other: producing one means contorting `broken.yaml` into
a shape no real workload has, which buys a fixture at the cost of the manifest
being an honest description of a broken cluster.

One coupling accepted knowingly: rule 3's image-pull message is pinned by
equality, and that message contains the sanitizer's `REDACTED-IP` placeholder,
so the decode test is coupled to the sanitizer's output. It stays — but **not
for the reason it was first written down here.** Restoring the real addresses
in `image.json` was tried, and `scripts/fixture-audit.sh` fails first and far
more legibly (*"[image.json] carries 5 IP addresses — this file never met
scripts/sanitize.jq"*) while the equality assert produces a 400-character
string diff inside a *decode* test. The audit is the first line of that
defence; the assert is redundancy, and describing it as the thing that prevents
silent acceptance would have been an overclaim that outlived the person who
made it.

### D41 — `cargo mutants` cannot see the defect it was put there to catch (2026-08-12)

Phase 3 ends on a box reading "`cargo mutants --timeout 90` clean over
`rules.rs` — a MISSED mutant is a rule change no test objected to". Run against
the decode above, it reports **1 missed of 34**; `tester`'s hand-written
mutations found **32**. The tool mutates return values and match guards, and 30
of the 32 holes live in **struct-literal field assignments** — `restarts:
c.restart_count` becoming `restarts: 0` is not a mutation it generates.

The one it did find is worth naming, because it is the same one a human found
independently: `replace match guard o.kind != "Node" with true in owner_of` —
[D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)'s
mirror-pod discard, unreachable because no fixture carries a Node
`ownerReference`.

**So the box stays, and stops being read as a proof of the decode.** A green
`cargo mutants` over `rules.rs` means the rules' *logic* survived mutation; it
says nothing about whether the fields those rules read were decoded from the
right place. That second question is answered by field-level mutation done by
hand, and the box now says so. This is
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) one level up: there,
a guard that had only ever been green; here, a gate that is green because it
never looked.

**A `scripts/` harness to make the hand sweep permanent was offered and
declined.** The process already carries it: step 4 of the cycle is `tester`'s on
every box, and this entry is what tells that step to go field by field on a
decode. A second mechanism enforcing what a named person is already accountable
for is machinery that has to be maintained and can itself rot green. If a later
box ever gets its decode changed without a sweep, the process failed and the
harness becomes worth building — that is the trigger, recorded so the decision
is revisited on evidence rather than on taste.

### D42 — the snapshot types freeze one phase after the file they live in (2026-08-12)

todo.md says **Frozen after: `rules.rs`** at the end of Phase 3, and its own
Phase 5 box says "*Phase 4* defined `ClusterSnapshot`". Both cannot be true,
and the second is the honest one about what Phase 4 needs: the Capacity report
wants `cpu_limit`, Waste wants `status.reason` for Evicted pileups, and
Drain-safety wants Services, PVCs, PDBs and EndpointSlices on the snapshot —
none of which any Phase 3 rule reads, so Phase 3 correctly refuses to invent
them (a field with no rule behind it is a guess, and `cargo mutants` cannot
object to a branch nothing exercises).

Moving `ClusterSnapshot` up to `analysis.rs` would invert the pyramid —
`rules.rs` is below it and takes the snapshot as its input. So the freeze is
made per-concern instead of per-file: **`rules.rs` freezes at Phase 3 close
except for the snapshot types and their decode, which freeze at Phase 4
close.** Phase 4 may add fields to them and nothing else in the file: not a
rule, not `Finding`, not `ObjectId`, not `analyze`. The forward-only rule is
intact — nobody reaches back into finished *logic*; a shared contract simply
has two consumers and gains its second one a phase later.

### D43 — N2 has no clock, and that makes a finding's age optional (2026-08-12)

> **Superseded in its premise on 2026-08-13 — read
> [D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
> first.** A cordon *does* leave a timestamp: the node lifecycle controller
> stamps `timeAdded` on the taint it mirrors from `spec.unschedulable`,
> regardless of effect, so **N2 may say "cordoned 2 hours ago" after all** —
> [D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)
> records that as a capability the rule may use, not one it must. The rest of
> this entry — the autoscaler cases, and N2 staying quiet during a scale-down —
> still holds, and so does the conclusion the title draws: an age stays
> **optional**, because the hand-applied taint carries no stamp.

N2 was written as **"unschedulable for 6 days"** and `screens/alerts.md` drew
the card with `6 days ago` in the age column. There is no source for that
number. `spec.unschedulable` is a bare boolean; no node condition transitions
on a cordon, `Ready` stays `True`; and the taint `kubectl cordon` adds is
`NoSchedule`, while `k8s-openapi 0.28.0`'s own `Taint::time_added` says
verbatim *"It is only written for NoExecute taints"* — read out of the crate
source, not inferred. (That sentence is **gone from the v1_34+ generated docs**
in the same crate, which say only "the time at which the taint was added". The
behaviour is unchanged; the citation stops being reproducible if the feature
pin moves up, so it is quoted here with its version.)

**That is a statement about `kubectl cordon`, and not about every cordon.**
The two controllers that cordon most nodes in the wild do stamp the time:
cluster-autoscaler's `ToBeDeletedByClusterAutoscaler` taint carries *the unix
second the scale-down began* as its **value** (`utils/taints/taints.go`:
`Value: fmt.Sprint(time.Now().Unix())`, added by the same call that cordons).
**Karpenter is *not* its equivalent, and
an earlier version of this entry said it was** — `karpenter.sh/disrupted` is
declared with a key and an effect and no `Value` field at all
(`kubernetes-sigs/karpenter/pkg/apis/v1/taints.go`), and it is `NoSchedule`, so
`Taint::added_at` is empty too. On a Karpenter cluster there is no clock
anywhere in the node object. The claim was corrected on the same day it was
written, by an operator review that went to the source instead of accepting the
symmetry.

Neither case needs a contract change — `NodeSnapshot.taints` already carries
key, value and effect — but both change N2: during a scale-down the node is
cordoned *with pods on it* for the whole eviction window, so N2 would fire,
repeatedly, on a cluster doing exactly what it was configured to do. **N2 stays
quiet when a scale-down taint is present.** A node an autoscaler is
deliberately removing is not a half-finished operation; it is an operation in
progress.

**But silence forever is a different claim from silence during the window, and
only the second one is defensible.** A scale-down that a PodDisruptionBudget
blocks indefinitely — node cordoned, tainted, workload still on it, the
autoscaler retrying — is a real loss of capacity, it is the most common way a
scale-down goes wrong, and under the rule above nothing on any screen shows it.
That case belongs to the **Drain safety report** (Phase 4: *for every node,
what a drain would do and what would block it*), which is where "a disruption
that is not finishing" is exactly the question being asked, and it keeps Alerts
free of a card about an operation that is merely slow. Recording it matters
more than which screen wins: before this, a stuck scale-down was silent and
nobody had written down that it was.

`metadata.managedFields` does carry the timestamp of whoever set the field, and
it is rejected: it is pruned at ingest by design, the sanitizer deletes it so
no fixture could ever test the path
([D30](#d30--the-guards-phase-2-added-and-the-freeze-they-collided-with-2026-08-12)),
and a field manager's name is not an API contract.

**So N2 reports no duration**, and the action line loses the inference the
duration was carrying — "someone's maintenance window never closed" is a claim
about elapsed time the tool cannot make. The consequence reaches two boxes
ahead: **a `Finding`'s timestamp is an `Option`**, because some findings have
no moment to point at, and the renderer needs an answer for that column rather
than a zero that draws as 1970.
[D4](#d4--the-flagship-example-promised-a-number-that-cannot-exist) is the
precedent and this is its third occurrence — a number that cannot exist is not
a rendering detail, and a mockup is where it hides longest.

**Losing the clock cost N2 the thing that made it a finding, so the rule is
narrowed rather than left noisy.** Without a duration, a node cordoned forty
seconds ago for routine maintenance draws exactly the card a node cordoned six
months ago draws — an alert raised on every routine maintenance window, and a false
positive is a bug here, not a tuning question. The discrimination the duration
was carrying is available from a field that exists: **N2 fires only on a
cordoned node that still has pods on it** — a drain someone started and did not
finish, which is both actionable and true at any age. **Corrected the same day
by [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12):
"still has pods on it" as written is true of every correctly drained node,
because a drain never evicts DaemonSet or static pods. The count is only of
pods a drain would actually move.** A cordoned node with
nothing on it is a *parked* node: correct, deliberate, and money burning
quietly, which is a **Capacity report row**, not an alert. That is
[D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s own line applied
honestly — broken now stays in Alerts, risky later goes to a report — and it is
what the card shows: `9 pods still run here` is the evidence, and the zero case
does not reach this screen at all.

The count survives the trap that killed `managedFields` as a source: the
sanitizer *refuses* a capture carrying foreign node identifiers rather than
rewriting them ([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)),
so both halves of the `spec.nodeName` join stay intact in a fixture and the
number is testable.

**Narrowing the rule moved the blindness from the evidence line to the trigger,
and that is a different bug.** The first version of this ruling said the count
is suppressed under namespace scope — fine when the count was decoration. Now
it decides whether the card exists at all: a user running `--namespace
payments`, or one whose cluster-wide pod LIST 403'd into the fallback, sees
zero pods on a node that has forty, and N2 silently does not fire. A wrong
number was replaced by a missing finding, which is worse because nothing on the
screen shows it happened.

**So a rule that needs every pod on a node is disabled under namespace scope
and says so, rather than computing a partial answer.** That is not a new
mechanism — it is the degradation
[docs/architecture § Error handling](docs/architecture.md#error-handling)
already specifies for a 403 on a secondary stream: the feature switches off and
names what is missing. **N2 and N5 both take it** — overcommit is the same
cluster-wide join, and a partial sum of requests understates every node it is
asked about. N6 is unaffected: it reads node taints and the Pending pod's own
spec, both of which are in scope by definition.

### D44 — five more mockups promising numbers nothing produces (2026-08-12)

Fixing N2's card came with a sweep of the other 30 mockups, because a defect
found by an implementer three phases early means nothing had been checking. The
sweep is the entry; the fixes belong to the boxes that build these screens, and
each is named here so it is inherited rather than rediscovered.

- **`screens/resources.md` — `jobs 7` in the sidebar.** Jobs are not in the
  permanent watch set ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
  puts the J-series in v0.2) and browser kinds are watched only while their view
  is open, so that count either buys a stream
  [invariant 6](CLAUDE.md) forbids or materialises out of nowhere after the
  first visit. The pod and workload counts above it are free; this one is not.
- **`screens/states.md` — `reading the cluster… 2,140 pods`.** Mid-LIST there is
  no total. Paginated, the API offers `remainingItemCount`, which its own
  documentation calls a hint.
- **`screens/analysis.md` — `API server certificate 210 days`.** C2, drawn as a
  row while kube-rs does not expose the peer certificate and the second
  connection it needs is undecided. A row with no data path is a promise made in
  a picture.
- **`screens/dialogs.md` — `replaced by web-2c81a 3 seconds ago`.** The
  timestamp is real (`creationTimestamp`); *"replaced by"* is an inference from
  a shared ownerReference and names the wrong pod whenever the ReplicaSet scaled
  for any other reason.
- **Rules 3, 4 and 8 have no event time.** `state.waiting` carries no timestamp
  and rule 8 is a fact about a spec, not an event. The general rule, binding on
  the box that puts timestamps on `Finding`: **a card's age is the time of the
  event it describes, or it is blank** — creation time is not a substitute,
  because a pod created nine days ago that started failing to pull four minutes
  ago is nine days old and four minutes wrong.

### D45 — the decode invented a container state the API says is `Waiting` (2026-08-12)

`ContainerState` was given a fourth variant, `Unknown`, for "the kubelet
reported a state with nothing set in it". k8s-openapi 0.28.0's generated doc,
straight from the OpenAPI definition, says otherwise: *"Only one of its members
may be specified. **If none of them is specified, the default one is
ContainerStateWaiting.**"* No fixture reaches the branch, so nothing was red —
the file's contract and the API's contract simply disagreed, quietly, in the
one file that freezes first.

**The API wins: the empty case decodes as `Waiting { reason: None, message:
None }` and `Unknown` is deleted.** Behaviour is unchanged — rules 1, 3 and 4
match on a *named* reason, so a `Waiting` with none fires nothing, exactly as
`Unknown` fired nothing — and one variant that nothing constructs goes away
with it. The alternative was keeping a state whose only justification was a
guess about the kubelet, in a type Phase 5 hands live watch data to.

Found by mutation testing, not by a rule: the survivor list was empty, so the
sweep went to the *source* to check the one claim it could not falsify — that
the three-way precedence is unobservable because upstream sets exactly one
member. The first sentence confirmed it and the second one broke something
else. A verification that only looks where it was pointed finds only what it
was pointed at.

### D46 — nine fields the contract dropped, and the drain that does not drain (2026-08-12)

The operator review of the snapshot types found eleven things. Three of them
were blockers, and the middle one killed a ruling written the same day. All of
them are the same defect class: **a field the API sends and the contract drops
at ingest** — invisible to a green suite, because a test can only assert on
what the struct kept, and unrecoverable afterwards, because these types freeze
one phase later ([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).

**Rule 7 would have fired on every deploy.** The rule is "Running and
`ready: false`", and so is every container between start and its first
successful readiness probe. Three fields answer *since when* and all three were
discarded: `ContainerStateRunning.started_at` (the `Running` variant was a unit
variant), `ContainerStatus.started` — **corrected below; it is far weaker than
this entry first claimed** — and
`conditions[Ready].lastTransitionTime`, the only place in the whole object that
records when the pod left the Service endpoints; a container status has no such
field anywhere. A Deployment with `initialDelaySeconds: 30`, a node reboot, a
scale-up: each would paint the screen whose entire promise is *only what is
broken*. The comment claiming the other four pod conditions "repeat what the
containers already say" was the assumption underneath it, and it was wrong.

**A pure function cannot know its own view is partial.**
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
disables N2 and N5 under namespace scope, and `todo.md` and
[docs/architecture § Error handling](docs/architecture.md#error-handling) both
say so — but `ClusterSnapshot` had no field distinguishing *"this cluster has
three pods"* from *"I can see three of this cluster's four thousand"*, and
[invariant 5](CLAUDE.md) forbids a rule asking anything outside the snapshot. A
small cluster and a namespace-scoped view of a large one decoded identically,
so the ruling was unimplementable by construction: `node-3` cordoned with forty
pods on it, none in `payments`, and N2 files nothing. `namespace_scope:
Option<String>` on `ClusterSnapshot` closes it — one field, both causes
(`--namespace` and the 403 fallback), and it gives the "not checked" message
its noun.

**`kubectl drain` never drains a node empty, so "still has pods on it" is true
of every node an operator drained correctly.** `kubectl/pkg/drain/filters.go`
returns skip for DaemonSet pods and for mirror pods *regardless of flags* — so
a properly drained kind worker still runs kindnet and kube-proxy, a drained EKS
node still runs `aws-node`, `kube-proxy` and `ebs-csi-node`, and a
control-plane node cordoned for an upgrade still runs four static pods no drain
can move. D43 narrowed N2 to kill exactly this false positive and the narrowing
did not narrow. **N2 counts only pods a drain would actually move**: not
`Succeeded`/`Failed` (`phase`, carried), not DaemonSet-owned (`owner.kind`,
carried), not mirror — and the third was the one the contract could not
express, because [D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)
discards the Node `ownerReference` that identifies a static pod. It is kept as
one bit, `PodSnapshot.mirror`, and the identity stays discarded: D39 is about
which card a finding files under, and that answer does not change. The bit
comes from the ownerReference and **not** from the `kubernetes.io/config.mirror`
annotation kubectl itself keys on — the sanitizer destroys annotations, so an
annotation-sourced bit would decode `false` in every fixture and could never be
tested ([D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
is the same lesson from the other side).

**Six more fields, each one a rule that would otherwise say less than
`kubectl describe`:**

- **A native sidecar is not an init container, in the arithmetic or in
  English.** `restartPolicy: Always` on an init container has been GA since
  1.29 and is how Istio, Linkerd and Vault agent run. The scheduler's effective
  request is `max( max over init prefix , sum(regular) + sum(restartable-init) )`
  — a sidecar is *additive*. One `init: bool` forces N5 to either overstate a
  2Gi migration container or drop 100m per pod on every meshed node, which on
  sixty pods is six CPUs invisible to the rule whose whole job is "the cluster
  is lying to itself about capacity". And "the init container `istio-proxy` is
  crashlooping" is not plain language, it is false — so rules 1–6 need the
  three-way distinction too, not just N5. `init: bool` becomes a three-way role
  with the invalid state unrepresentable.
- **`finalizers`.** Rule 12 promises "a finalizer *or* the kubelet is holding
  it" — a coin flip between two causes whose fixes are unrelated. The list is
  the answer, `describe` does not print it at all, and the positive fixture has
  carried `k8rs.test/never-removed` since Phase 2.
- **`subPath` on a hostPath mount, and the container that mounts it.**
  `hostPath: /var/run` + `subPath: docker.sock` recorded `/var/run`, and rule
  8's docker.sock escalator never saw it — a read-only bind-mounted socket is
  still full root on the node. Without the container name the finding cannot
  say *which* container mounts the node root, and two containers mounting one
  volume produced two entries the rule could not tell apart.
- **`ContainerStatus.image`.** Rule 3's action is "check the image name/tag or
  the pull secret", and the name reached the user only inside the runtime's own
  sentence — `image.json`'s message is containerd-worded and CRI-O phrases it
  differently.

**And `deletionTimestamp` is a deadline, not a moment.** The apiserver sets it
to *request time + grace period* (`registry/rest/delete.go`:
`metav1Now().Add(GracePeriodSeconds)`), which `stuck.json` has been proving all
along: `23:16:54` with a 5-second grace, so the delete landed at `23:16:49`. A
rule written to the comment that said otherwise would double its own threshold
— flagging a default pod at 60 seconds, not 30 — and report an age short by
exactly one grace period, forever. Both fields were already carried, so
`asked_at = deletionTimestamp − grace` is computable; what was wrong was
written down, in the file and in the rule-12 row here.

Two more went to Phase 4 rather than here, which D42 permits: `spec.overhead`
(RuntimeClass) and `status.allocatedResources` — the latter is what the node
actually reserved during an in-place pod resize, and it diverges from `spec` on
exactly the 1.33+ clusters this project targets. Both belong to the Capacity
report, and they are recorded on its box so they are inherited rather than
rediscovered.

**Smaller rulings the fix itself forced**, recorded because a decision nobody
wrote down is one that gets rediscovered with no memory of why:

- **`owner_of` returns the identity and the mirror bit together**, not a second
  `is_mirror(meta)` beside it. One traversal, so the bit and the discarded
  reference cannot come to disagree — the failure mode of every "derive it
  again over there".
- **Only a *controlling* Node reference makes a pod a mirror pod.** A
  non-controlling one is a garbage-collection link and means nothing about who
  writes the pod. It is one `controller == Some(true)` and it is load-bearing:
  a Node reference that does not control does not exempt the pod from N2's
  count.
- **A container the status names but the spec does not declare decodes as
  `Init`, never `Sidecar`.** The kubelet only reports declared containers, so
  the case does not arise; the point of choosing is that the safe answer keeps
  the finding's English right if it ever does.
- **The sidecar role is read off the init list only.** The tempting
  justification — "upstream allows `restartPolicy` nowhere else" — is already
  expiring: 1.34 began relaxing the field on regular containers. The real
  reason is that a regular container answers `Regular` either way, which is a
  statement about our own behaviour rather than a bet on upstream's. Same
  reason no test synthesizes that object: under [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
  a synthesis is licensed only while every field of it is a value the *pinned*
  API emits, and `v1_32` does not emit this one.
- **`PodSnapshot.ready` and `ContainerSnapshot.ready` keep the same name with
  different types**, which was raised as a collision worth renaming before the
  freeze. Kept: `PodSnapshot` already carries `scheduled: Option<Condition>`,
  so `ready` beside it is the consistent name and renaming one of a matched
  pair is the worse trade. The two never appear without their receiver.
- **`subPath` is stored raw and the join is the rule's.** `HostPathMount` holds
  the fact; rule 8 decides whether `/` joined with `var/run/docker.sock` is the
  escalated case. Storing the joined path would be storing a verdict, which is
  the same line the type already draws by holding `read_only` instead of
  "dangerous".
- **The order of `PodSnapshot.containers` is explicitly *not* a contract.** It
  is init-first today and nothing may rely on that: containers are read by
  name, and a screen that wants them grouped sorts by `role`. The tempting
  alternative — declaring init-first *as* the contract — would have been the
  same defect this box was sent back to fix, a stated requirement with no
  assertion behind it. `ContainerRole` deliberately derives no `Ord`, because
  which role sorts first is a display decision and this file does not make
  those. The claim was verified rather than asserted: reversing the chain
  breaks no assertion in the file, which is exactly why the rule has to be
  written down instead of discovered later by `views.rs`.
- **A container the status names but the spec does not declare stays untested,
  by ruling and now in writing.** Both container lists are immutable after
  create, so the kubelet cannot report on an undeclared container and
  [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
  does not license synthesizing one. The reason now lives in the code, so the
  absence of a test reads as a decision rather than an oversight — which is the
  whole difference between the two.
- **`restartPolicy: Always` on a *regular* container is KEP-5307 (alpha in
  1.34), and the ruling deliberately does not depend on that.** Whether a given
  1.36 server admits the field turns on a feature gate nobody here could
  verify, so the requirement is written to be independent of the answer:
  `Regular` is the role whatever the restart policy says and whoever admitted
  it. A synthesis that leaned on "a v1.36.1 server produces this" would have
  been an unverifiable version claim dressed as a fact — the failure mode
  [D45](#d45--the-decode-invented-a-container-state-the-api-says-is-waiting-2026-08-12)
  is about, arriving from the opposite direction.

The review that found all of this ran against a file whose 32 tests were green
and whose every mutation had been killed. That is the argument for the gate
being held by someone who runs clusters rather than by the suite: a test
asserts what the struct kept, and none of these was in the struct.

### D47 — Phase 3 is running ahead of an open Phase 2, and what that buys and owes (2026-08-12)

Phase 2 has four boxes left and they are all the same box: **the kind cluster
trip.** Stand the cluster up, apply `broken.yaml` and `healthy.yaml`, wait for
the states to settle, capture, tear it down. Phase 3 has been built past it on
an explicit instruction — *"continue Phase 3, we do the trip at close"* — and
that is allowed. It was also **already recorded** before this phase began —
[D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)
is that ruling, and the box in `todo.md` states in its own text which half has
run and which has not.

**What has changed since D33 is the size of what is open.** D33 deferred *one*
box, `just cluster-down`, waiting on Phase 4's four missing report fixtures.
Phase 3 has since added three more, and every one was discovered by building
the layer above: a broken pod that has an owner
([D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12)), a mirror pod
([D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)),
and the list of shapes the first capture could not produce
([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)),
which itself grew three items during this phase alone
([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
That growth is not drift, it is the pyramid working — a capture list written
before the layer above it exists is a guess, and each of these came from a rule
that could not honestly be tested without it. But four open boxes is a
different fact from one, so the accounting goes in the record rather than in
someone's head.

**Why deferring is the right call anyway.** The trip is one physical
operation — a cluster stood up, broken on purpose, waited on, captured, torn
down — and running it four times to close four boxes buys nothing over running
it once. It also needs a machine: docker on this host, and a login session that
postdates the `docker` group grant, which is a condition the agents cannot
create for themselves ([the boxes no agent can run](CLAUDE.md)). One trip, at
the point where the list of what it owes has stopped growing.

**What it costs, measured rather than asserted.** `rules.rs` carries 16
synthesis sites. Four are permanent — an API state no cluster can be asked to
emit ([D45](#d45--the-decode-invented-a-container-state-the-api-says-is-waiting-2026-08-12)),
a node whose `allocatable` differs from its `capacity` (a kubelet flag, not a
workload), a non-controlling `ownerReference`, a second Node reference. **The
other twelve are a loan**: a test standing on a hand-set field instead of a
capture, each naming the object that retires it.
[D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
licenses that, and it licenses it *as a loan* — the synthesis is bounded by
"every field of it is a value the API actually produces", which is a rule that
holds only as long as someone keeps checking. It has already failed once in
each direction: a synthesized node condition that contradicted its own message,
and a decode that invented a state the API defines as something else. Both were
caught by going to the source, not by the suite, and neither would have existed
if the object had come off a cluster.

**So the boundary is hard: Phase 3 does not close until Phase 2 does.** Not
"the trip happens around then" — twelve of Phase 3's tests are unfinished until
it runs, and [phase close, item 2](CLAUDE.md#phase-close--the-ritual-at-the-end-of-every-phase)
says a box checked for work that was written but never run is a lie in the one
file the plan is read from. The trip's checklist is not a memory: every
synthesis names its object, and Phase 2's own boxes list them by the file that
has to change.

### D48 — a check that is switched off is named on the screen that would have shown it (2026-08-12)

[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
turned N2 and N5 off under namespace scope and said they should "say so". It
did not say **where**, and `screens/alerts.md` had kept the superseded half of
the ruling — a card with its evidence line dropped, which is a card asserting a
node is broken on evidence it does not have. The screen and the decision record
disagreed, and `screens/` is what the code has to match, so Phase 9 would have
built the wrong one.

**The line goes in the banner above the list, on the screen that would have
shown the findings** — Alerts names N2's absence, the Capacity report names
N5's. Rejected: the header (it carries the cause, `ns: payments`, and has no
room for a sentence), the help screen (keys only), a greyed-out card (there is
no card), and one global notice (which grows into a list as more checks switch
off, on every screen, including the ones unaffected). The banner is the widget
the disconnected state already uses, so this is placement, not a new mechanism.

Three rulings follow from drawing it, and the second is the one that was
actually wrong before:

- **Node listability is a separate permission from namespace scope.**
  `widgets.md` justified a blank header vital with "a namespace-scoped user
  cannot list nodes", which is false — and D43's entire premise is a user who
  *can* list nodes but cannot list every pod. Decoupled: it now drives the
  header vitals, the `capacity` badge and which node rules run, independently.
- **The `of 5` denominator is a second permission too.** It comes from the
  workload watch, not the pod watch, so a user allowed `pods` but not
  `deployments` has the numerator alone. The card reads `3 pods`, grouping
  falls back to the pod's own name rather than the hashed ReplicaSet, and
  W1/W2 are named as off — [D3](#d3--findings-group-by-owner-not-by-pod)'s *no
  owner* fallback extended to *owner unreadable*.
- **Three more checks join N2 and N5**: drain safety, the pending-CSR row and
  Versions. D43 named two because two were in front of it. The line that
  generalises them: **a sum breaks silently under a partial view, a list just
  gets shorter** — so Waste, whose rows are per-object facts, runs unchanged,
  and greying out the whole Analysis screen would have hidden three true
  answers. Drain safety is the sharpest of them: *"node-1 ok, 18 pods move"*
  computed from a partial view is a green light for an operation that then
  hangs.

**N2's evidence line changed with its trigger**, which is the part that would
have rotted quietly. `9 pods still run here` described an inventory — the
reader checks it with `kubectl get pods -o wide` and finds a different number,
because [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)
stopped counting DaemonSet, static and finished pods. **`2 pods here would
still have to move`** describes the computation `kubectl drain` itself performs,
so the card and the command it teaches agree. A two-number form (`2 to move · 7
stay`) was rejected: the second number needs its own fixture and invites exactly
the arithmetic-checking the first wording lost to.

**One thing found in passing and ruled here rather than deferred:**
`screens/dialogs.md`'s drain dialog prints *"PodDisruptionBudget won't allow
fewer than 3 copies"* — raw jargon, while `analysis.md` explains the identical
fact in plain English. It was raised as "a v0.2 screen, your call".
[Invariant 14](CLAUDE.md) has no version exemption: a string a user can read is
written for someone who does not know the term, whichever release it ships in.
The rewrite belongs to the box that builds that screen, and this is the record
that it is owed rather than optional.

### D49 — the link checker skipped every link that wrapped, and had been green all along (2026-08-12)

`scripts/check-docs.py` scans **line by line**, so a Markdown link whose
`[label]` wraps across a newline matches no regex and is skipped **entirely** —
not just its anchor, its target file too. Three such links were in the repo.
Reproduced here before believing it: a deliberately broken wrapped link
(`NOTES.md#no-such-anchor-at-all`, label split over two lines) appended to
`docs/tech-stack.md` produced *"checked 24 markdown files / OK — all relative
links resolve"*; the identical link on one line produced *"FAIL … 1 broken
link(s)"*. The hole is the wrap, exactly.

This is [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
and [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
a third time, and the recurrence is the point. Both of those were about the
*sanitizer* — which objects reach the check, and where inside a value a secret
can sit. This is the same defect in a different guard: **the shapes a guard was
fed were all the shapes its author happened to write**, and this project wraps
its prose at 79 columns, so the untested shape is the one the house style
produces most. Every guard in `scripts/` is now suspect in the same way, not
because any specific hole is known, but because none of them was fed input in
the shape the *repo* generates rather than the shape the test wrote.

The fix is one line — read the file whole with `re.S` instead of iterating
lines — and it is `tester`'s, along with the wrapped-link case as a permanent
`--self-test`. Found by `tui-designer` while checking its own work, which is
the only reason it was found at all: nothing in `just check` could report it,
because the guard reporting green *was* the bug.

### D50 — the rule tests live in `rules.rs`, and no lib target is added to change that (2026-08-12)

> **Half of this title is out of date, the half that matters is not.** The tests
> moved to `src/rules_tests.rs` on 2026-08-13
> ([D80](#d80--the-tests-moved-out-of-rulesrs-and-d50s-ruling-did-not-move-with-them-2026-08-13))
> — as a `#[path]` child module, still inside the bin crate. **No lib target was
> added**, which is the thing this entry refused, and it is still refused.

`cargo metadata` reports exactly one target — `k8rs`, kind `bin`, root
`main.rs` — so a file under `tests/` cannot `use k8rs::rules::PodSnapshot`.
Every rule test therefore lives inside `src/rules.rs`, which is `dev-core`'s
file, and `tests/` holds only `fixtures/`. Raised because it makes `tester`'s
ownership row unusable for rule coverage across the whole of Phase 3, and the
remaining boxes would each hit the same wall.

**No lib target is added.** The fix would be a `src/lib.rs` carrying the `mod`
lines with `main.rs` consuming it — a ninth file, and
[invariant 11](CLAUDE.md) wants the same kind of boundary argument the eighth
had. "So that integration tests can reach the code" is not that argument when
unit tests already reach it: `#[cfg(test)] mod tests` in the same file is
Rust's normal idiom, 38 of them are already there, and they run under
`cargo test` exactly like anything in `tests/` would. Adding a file of pure
plumbing to relocate tests that already work is the trade this project keeps
refusing.

**What actually needed saying is who does what, and the model already had it
right.** [The cycle](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it)
step 3 is "write the code **and its tests together**" and names the dev; step 4
is `tester` witnessing red then green. So on rules, `tester`'s job was never to
author the tests — it is to *attack* them, and on this box it reproduced the
author's fourteen mutations with its own driver and then found two the author
had missed: a `started: None` branch no capture exercises, and a regular
container with `restartPolicy: Always` decoding as `Sidecar` against the file's
own stated requirement. Both went back to the author. That is the gate working
as designed, and it is worth more than a directory boundary would have been.

`tester`'s `tests/` row keeps its meaning for what will actually live there:
the fixtures, and the Phase 7 end-to-end tests that drive the binary rather
than call into it.

### D51 — the third review of the same contract, and the sentence that would have rebuilt the bug it closed (2026-08-12)

The snapshot types went through three operator reviews. The first found nine
missing fields ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)),
`tester` then found two holes the author's own mutation sweep had missed, and
the third — reading the file *after* all of it was fixed and independently
attacked — found a blocker and five more. That progression is the argument for
the gate, so it is worth stating what the blocker actually was: **not a field,
a sentence.**

**`ContainerStatus.started` does not mean what D46 said it meant.** This record
called it "the startup-probe bit, which is precisely 'still booting' versus
'was serving and stopped'". The crate's own generated doc finishes the
thought — *"**Is always true when no startupProbe is defined** and container is
running and has passed the postStart lifecycle hook"* — and **no container in
any committed fixture declares a `startupProbe`**. For the overwhelming
majority of real workloads the field flips true the instant the container runs
and discriminates nothing at all. A rule 7 written from that prose —
`Running && !ready && started` — fires CRITICAL on every pod of every rolling
update, every node reboot and every scale-up: precisely the blocker this
contract was sent back to fix, rebuilt out of the sentence written to close it.
`healthy.json` would not have caught it either, because there is no mid-rollout
pod in the capture.

The field stays; it does discriminate where a `startupProbe` *is* declared. But
the "since when" rule 7 rests on is `ready.last_transition` and nothing else,
and that is now said where a rule author will read it. The lesson is narrower
and worse than "check upstream": the *first half* of that doc sentence was
quoted correctly, and the half that mattered was two clauses later. A quotation
that stops where the point was already proven is not a verification.

**Four more fields, each a rule that would otherwise be confidently wrong:**

- **Pod-level requests** (`PodSpec.resources`, KEP-2837). *Two version facts
  that look like a contradiction and are not:* the pinned `v1_32` generated doc
  says *"This is an alpha field and requires enabling the PodLevelResources
  feature gate"*, while the field went beta and default-on in **1.34** and the
  fixtures are **v1.36.1**. Both are true at their own version, and the field
  is present in the pinned crate either way, which is all a decode needs — it
  is written here so the next reader who opens `v1_32/api/core/v1/pod_spec.rs`
  does not conclude the field was carried on a guess. A pod declaring
  `spec.resources.requests: {cpu: "4"}` with no
  per-container requests decodes as all-`None`, so N5 sums **zero** and calls
  the node healthy while four committed CPUs sit invisible. This is
  `namespace_scope`'s shape a second time — a pure function looking at zeros
  with no way to tell "requests nothing" from "requests something I did not
  look at". When the pod-level value is set it *replaces* the container sum.
- **The enacted memory limit, not the requested one.** `ContainerStatus.resources`
  is *"the limits that have been successfully enacted on the running
  container"*, and it is already populated in every committed capture. Rule 2
  reads `spec`. Patch a crashing pod 128Mi → 512Mi, have the in-place resize
  sit `Deferred` because the node cannot fit it, and the evidence line reads
  *"exceeded its 512Mi limit · exit 137"* about a container never given 512Mi —
  an operator sent hunting a leak in an application that never had the memory.
  Status first, spec as fallback.
- **`ObjectKind` must read the API group, not the kind string alone.**
  OpenKruise is deliberately drop-in: its Advanced StatefulSet is
  `apps.kruise.io/v1beta1, Kind: StatefulSet`. `owner_of` holds the whole
  `OwnerReference` and was passing only `kind`, so those pods decoded as the
  core variant. The card lying is the small half; the large half is Phase 7
  aiming `scale` at `apps/v1 statefulsets/<name>` — a 404, or a *different*
  object that happens to share the name. A write path pointed at the wrong
  object is not a display bug. Argo Rollouts was safe only by the accident of a
  unique kind string.
- **`Terminated` carries `started_at` and `message`.** "Restarted 5 times" is
  the same sentence for two unrelated incidents. *"Runs for about 2 seconds,
  then exits 1"* versus *"ran for 40 minutes, then exited 1"* is the first fork
  of every crashloop triage — bad config on one side, a leak or a downstream
  timeout on the other — and `describe` makes you subtract the timestamps
  yourself at 3am. Every capture has both. `message` is the same argument: with
  `terminationMessagePolicy: FallbackToLogsOnError` the kubelet writes the log
  tail there, which turns rule 6's action from "check the logs" into the log
  line. `Waiting` already carried `message`; `Terminated` not carrying it was
  an asymmetry with no reason behind it.

**And C1's input goes into `ClusterSnapshot`, settled now rather than at its
own box.** `todo.md` had described C1 as "PEM bytes in, finding out" — a second
entry point that [invariant 5](CLAUDE.md)'s stated signature does not describe,
which makes it an invariant amendment, which is a stop rather than a
convenience. So the snapshot carries the kubeconfig **context name** and the
client **certificate**; it is already documented as assembled by `k8s.rs` and
never decoded from one API object, so this is what it is for. The certificate
only — never the private key, never the token, never anything else off the
kubeconfig, and the field says so, because the next reader will rightly wonder
why a kubeconfig is anywhere near this struct. Deciding it here costs two
fields; deciding it at the C1 box would have meant deciding it with the file
one box from frozen.

**Rulings the fix forced, and one it uncovered.** The author's own second pass
found a **fourth** site of the blocker that this review had not named:
`ContainerState::Running`'s doc claimed `started_at` was *"the only 'since
when' a running container has"* and that rule 7 needed it — the same wrong
claim one field over, and it flatly contradicted `PodSnapshot::ready`'s doc on
the same page. A rule 7 written from *that* sentence rebuilds the false
positive by a second route. `started_at` is the current run's uptime, which is
rules 1/5/6 evidence — *"it came back up forty seconds later"* — and nothing
else. Three sentences were fixed by instruction and the fourth by someone
reading the file as a stranger would; that ratio is the argument for the second
pass being mandatory rather than conditional.

- **The group identifies a type; the version is serialisation.** `apps/v1beta1
  StatefulSet` still decodes `StatefulSet`, and `Other`'s format is kubectl's
  own notation, `Kind.group` — a core-group unknown kind stays bare
  (`Other("ReplicationController")`, no trailing dot), and `Other("kubeconfig")`
  for C1 is untouched.
- **The mirror-pod discard moved onto the *resolved* kind**, not the raw
  string, so a CRD named `Node` in someone else's group is an ordinary owner
  rather than a static pod. That is the same bug as the OpenKruise one
  approached from the other side, and it was not in the brief — it has a test.
- **`effective()` falls back per key, not per side.** A server too old to
  populate `status.resources` is inside the support window, so a key missing
  there falls through to `spec` rather than the whole side reading as "nothing
  enacted". The cost is stated where the helper lives: a resize that *adds* a
  limit names the not-yet-enacted value until it lands, which degrades to
  exactly today's behaviour — the safe direction.
- **`status.allocatedResources` is deliberately not read.** It repeats
  `status.resources.requests` and would add a third precedence step for no new
  fact. This narrows what D46 sent to Phase 4: the *requests* half is covered
  by `effective()` already, and what Capacity may still want is `spec.overhead`.
- **`context` is `Option<String>`** — a kubeconfig can name no current context,
  and `None` there means C1 has no name to file under, which is a real state
  rather than a defensive one.

### D52 — the guards were fed the shapes their authors wrote, not the shapes the repo produces (2026-08-12)

Fourth occurrence of one defect. [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
was the sanitizer seeing a single Pod and not the `List` half the captures
arrive in. [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
was the same guard matching whole values while secrets sit inside sentences and
inside base64. [D49](#d49--the-link-checker-skipped-every-link-that-wrapped-and-had-been-green-all-along-2026-08-12)
was the link checker scanning line by line in a repo that wraps at 79 columns.
And now `fixture-audit.sh`, which owns the correct base64 predicate and applies
it to `*.json` only, skipping `certs/*.crt.pem` outright — so a real
certificate with a private key base64-wrapped and appended printed **"no key
material"** and exited 0, in both that guard and `certs-test.sh`. The green log
was byte-identical to a clean run.

**The directory it exempted is the only one where key material is actually
generated**, and it is about to matter more: `ClusterSnapshot` now carries a
client certificate for rule C1 ([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)),
a kubeconfig fixture is on the roadmap for that rule, and a kubeconfig carries
`client-key-data` **base64-encoded** — the exact framing the check skipped.

So the rule stops being "test the guard" and becomes specific enough to act on:
**feed every guard the shapes the repo produces, not the shapes its test
writes.** The two are systematically different, and the difference is invisible
because a guard that misses a case reports success in the same words as a guard
that found nothing wrong. Every guard in `scripts/` is being swept on that
basis — not because a further hole is known, but because none of them was ever
fed repo-shaped input, and four for four is no longer a coincidence.

Each fix carries the shape into its own `--self-test`, or it regresses the
first time someone refactors it. That is what makes this different from
patching four bugs.

**The sweep found holes in five guards, not the one it was sent for.** Two were
known going in; three were not, and one of them is a hard invariant:

- **`write-guard.py` exempted any file *named* `ops.rs`, not `src/ops.rs`.**
  Fed six files each containing `api.delete("web")`, it reported two and
  silently exempted `tests/ops.rs`, `examples/ops.rs`, `benches/ops.rs` — and
  `build.rs`, which is **compiled and executed at build time** and sat under
  none of the four scanned roots at all. `tests/ops.rs` is the obvious name for
  Phase 7's write-path integration test: the one file that must not be able to
  opt out of the invariant it exists to test. [Invariant 1](CLAUDE.md) is
  enforced "mechanically and stays that way", and for the whole of Phases 1–3
  the mechanism had a name-shaped hole in it. Now exempt by resolved path, with
  `build.rs` scanned.
- **`test-guard.py` would have turned `just check` red the first time anyone
  ran `just mutants`.** `cargo mutants` writes a full copy of the source tree
  to `mutants.out/`, which the blocklist did not cover, so the guard counted
  every test twice: *"90 declared, 45 listed — 45 never run"*. A fabricated red
  build, and precisely the kind that gets repaired by weakening the guard
  rather than by fixing it — in the phase where `just mutants` starts being
  used. The blocklist became the same roots allowlist `write-guard.py` uses,
  because a blocklist needs a new entry per tool and an allowlist excludes
  every stray copy by construction.
- **`screens-check.py` printed green over a 201-column frame** in four shapes,
  including a mockup inside a `~~~` fence — which `check-docs.py` had already
  decided *is* a fence, so two guards in the same repo disagreed about what a
  code block is. A fence nobody closed silently dropped the block; a mockup one
  directory down was never looked at (`glob`, not `rglob`); and an empty
  `screens/` reported *"0 mockups fit 80x24 — OK"*, which is D26's own lesson
  about a count of zero reading as success.

**One gap is left open on purpose, because closing it is a policy choice rather
than a bug fix.** `sanitize.jq` does not cover a CertificateSigningRequest's
`.spec.username` and `.spec.groups`. The committed `csr-pending.json` carries
`kubernetes-admin` and `kubeadm:cluster-admins`, which are kind defaults — so
nothing has leaked, and that is luck rather than the guard. A CSR captured from
a real cluster carries an OIDC email or `system:serviceaccount:prod/deployer`
there.

**It takes the node-name treatment: refused, not rewritten.** A requester
identity is a *reference*, not a payload — the same category as a node name,
and the sanitizer already refuses a capture whose node names did not come from
the kind cluster rather than quietly mangling them
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
Extending that one refusal to one more field needs no new mechanism and no new
policy shape, which is exactly why it is the right answer: an allowlist of
acceptable usernames would be a judgement call re-made on every capture. **This
lands before the next capture trip, not after** — it is a Phase 2 box, and
Phase 2 owns fixture capture.

Decisions the sweep forced, worth keeping: `openssl` is the key oracle rather
than a regex (already required by two scripts, so not a new dependency), and it
is called with `-passin pass:` — without it an encrypted key prompts on the
terminal and **hangs `just check`**, which is a guard that fails by hanging
rather than by failing. Thresholds are 100 base64 characters and 64 PEM-body
characters: an EC P-256 key is ~184 base64 characters, so the *smallest* key
anything real produces still clears both, and the self-test generates exactly
that key rather than a comfortably large RSA one.

### D53 — a committed capture is never edited to make a test pass (2026-08-12)

`effective()`'s per-key fallback needed a shape no capture has: a `requests`
map the kubelet enacted *without* `cpu` in it, so that `spec` is the only
source for that one key. The proposal was to add `cpu: 250m` to `oom.json`'s
spec requests.

**Right shape, wrong method, and the distinction is the whole of why fixtures
are trusted here.** `tests/fixtures/*.json` are real captures — that is the
property [the honest-test rules](CLAUDE.md#code-phase-rules)
buy by refusing hand-written JSON, and it is a property of the *file*, not of
any one test. Editing a committed capture so a test goes green spends that
property for every other fixture in the directory at once, and it spends it
invisibly: nothing in the file says which bytes came off a cluster and which
were typed. [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
already licenses the honest version — deserialize the capture, set the one
field in memory, assert, and name the object the capture trip owes. The synthesis
is visible in the test that does it; an edited fixture is visible nowhere.

**A surviving mutant that was written up honestly rather than closed.**
`effective()` reading `status.resources` *only* for keys `spec` also declares is
indistinguishable from the real code unless the kubelet enacts a key the spec
never asked for. All 23 fixtures were scanned — zero such shape — and no way to
make a cluster emit one was found, since an enacted value is enacted *from* a
declared one. The first draft of that comment stated it as fact ("both lists
are defaulted at admission, so an enacted key has a declared key behind it");
the author's own second pass caught that as [D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)'s
exact failure — a sentence claimed harder than its evidence — reappearing
inside the comment written to close D51. It now reads **"a shape nobody could
produce, not one ruled out"**, and names the surviving mutant out loud. That is
the right register: the same standard already applied to the unmatched container
status and to `From<StatefulSet>`, and it costs nothing to say which of the two
things you know.

**And the caution paid off within the hour — the shape is reachable.** The
operator review found it in upstream: with pod-level resources, a container that
declares *some* limit but no memory limit gets the **pod's** memory limit
written into its own status (`kuberuntime_container_linux.go`'s `getMemoryLimit`:
*"When container-level memory limit is not set, the pod-level limit is used"*),
and `convertContainerStatusResources`'s memory branch has no key-existence
guard, unlike the CPU branches beside it. So `status.resources.limits` carries
`memory` while `spec.containers[].resources.limits` does not — exactly the key
the mutant would drop, and the value the container is actually killed at. One
manifest produces it, and it is now on the capture trip.

Had the first draft's *"both lists are defaulted at admission"* been left in,
this record would today be asserting a falsehood with upstream code contradicting
it. The honest framing was not merely more modest — it was the one that stayed
true.

**`effective()` keeps its per-key fallback, and the argument against it lost on
the source rather than on preference.** The review's case for per-object rested
on a container's status resources being a copy of its *spec*; upstream copies
the **allocated** map (`convertContainerStatusResources` takes
`allocatedContainer.Resources.DeepCopy()`), and allocated and spec diverge in
key set exactly while a key-adding resize is pending, which validation permits.
Upstream's own arithmetic is a per-key union — `maxResourceList` takes a key
present on *either* side — and the scheduler reads it with
`UseStatusResources: true`, so per-object would charge N5 nothing for a request
the scheduler is actively counting. `PodRequests` even carries the comment
*"the computation is part of the API and must be reviewed as an API change"*.

The real split is not requests-versus-limits, it is normal-versus-**Infeasible**:
when a resize is rejected as infeasible, upstream drops the spec entirely. That
one case k8rs reads wrongly, **knowingly** — reading it right needs
`PodResizePending` on `PodSnapshot`, no v1 rule reads that condition, and the
file freezes before Phase 4, so adding it would be a plan change rather than a
code one. It is written down here instead of being discovered as a bug.

**One more thing the fix turned up, and it is the kind that only reading the Go
types finds: `desired` and `ready` are both `Option<i32>` and their `None`s mean
opposite things.** `ReadyReplicas` is a plain `int32` with `omitempty`, so it is
absent **exactly when it is zero** — `ready.unwrap_or(0)` is correct, and a W2
written as `if let (Some(d), Some(r))` would go silent on precisely the total
outage it exists to report. `Replicas` is a `*int32`, so `replicas: 0`
serialises fine and `None` means *absent*, which the API then defaults to
**one** — so `desired.unwrap_or(0)` would say "wants nothing" where the API says
"wants one". Two identical-looking `Option`s, two opposite readings, and the
field docs had been transposed on top of that. DaemonSet's counters carry no
`omitempty` at all and are always `Some`.

Two smaller things settled in the same pass, both about what the contract can
still express:

- **N2 does not need to see `emptyDir`.** `kubectl drain` also refuses on
  local-storage pods without `--delete-emptydir-data`, and the snapshot carries
  only `hostPath` volumes — so N2 cannot tell that a drain would *block* on
  such a pod. It does not need to: N2's sentence is *"N pods here would still
  have to move"*, and a pod with an `emptyDir` does still have to move. What
  would be wrong is the sentence *"N pods a drain would move"*, which promises
  the drain succeeds. Blockers are the **Drain safety** report's question, and
  it is the second place today that a half-finished disruption has been sent
  ([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).
- **`ClusterSnapshot` still has no `now`**, correctly — it is the next box. But
  `ObjectKind` and `ContainerRole` freeze at Phase 3 close while the snapshot
  types get until Phase 4 close ([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)),
  and `now` is the one field [invariant 5](CLAUDE.md) names by itself. It does
  not get to drift into that extra phase's slack.

### D54 — `now` is `meta::v1::Time`, not a bare `jiff::Timestamp` (2026-08-12)

[D18](#d18--the-clock-is-an-input-not-an-ambient-fact) settled which crate the
time type comes from and wrote the answer as `jiff::Timestamp`. The field that
landed is `meta::v1::Time`, which *is* that type — `pub struct Time(pub
jiff::Timestamp)` — wearing the same newtype every decoded API timestamp in
`rules.rs` already wears.

The reason is the comparison, which is the only thing this field is for: rule 12
compares `now` against a `deletion_timestamp: Option<Time>`, C1 against a
certificate date, the renderer against a finding's timestamp. One type on both
sides is `<=`; two types is `.0` at every site, and `.0` at every site is one
forgotten `.0` away from comparing a laptop instant against something that only
looks like one. The crate coupling D18 recorded is unchanged — `Time` is
k8s-openapi's, and k8s-openapi's jiff is what moves.

It buys nothing for the *arithmetic*, and that half is written down where the
next box will read it rather than here: `.0` is needed on both sides of every
subtraction, `a - b` yields a seconds-only `Span` whose `.get_minutes()` is `0`
for a 43-minute gap, and the call that behaves is `Timestamp::duration_since`.

**Not an `Option`.** A snapshot always has a moment. An `Option` would push a
"what if there is no time" branch into every rule that reads one, and the only
answer available in that branch is the value the caller already had.

### D55 — the clock was written backwards, and the clamp protects the harmless half (2026-08-12)

D18 said a laptop running **fast** produces a negative age. It does not, and the
error had been copied into `rules.rs` before the operator review caught it.

Age is `now − event`:

| laptop | pod deleted 11:59:50 | what the screen does |
|---|---|---|
| 10 min **fast** | age **+10m05s** | rule 12 **fires**: "asked to shut down 10 minutes ago and still hasn't" |
| 10 min **slow** | age **−9m55s** | rule 12 silent; the renderer says "just now" |

So the "just now" clamp guards the *slow* case — the one that under-reports and
harms nobody — and does nothing at all about the *fast* case, which is the one
that manufactures findings on a healthy cluster. Three consequences, all
binding on later boxes:

- **Rule 12's trigger gets a margin.** The recorded threshold was
  `deletionTimestamp` in the past, full stop. With a laptop ten minutes fast —
  an NTP-less machine, a VM resumed from suspend, a WSL2 host after sleep —
  every pod asked to terminate in the last ten minutes is "overdue", and a
  correctly-progressing 50-replica rollout fills the one screen whose promise
  is *only what is broken*. Even on a perfect clock a pod is briefly overdue
  between its deadline and the kubelet's SIGKILL landing. Rule 12 fires on
  `now − deletionTimestamp > max(30s, grace)`. Nothing is lost: a pod held by a
  finalizer is stuck for minutes or forever.

  > **The formula was wrong and is `> 60s`, flat, since 2026-08-13.**
  > `deletionTimestamp` **already is** request + grace, so `max(30s, grace)`
  > charges the grace twice: a pod with `terminationGracePeriodSeconds: 3600`
  > stayed invisible a full hour past its kill deadline — two hours after the
  > operator typed the delete — and a Kafka or Vault pod stuck exactly that way
  > is what blocks the rollout rule 12 exists to surface. The sentence above it
  > mis-sized the margin too: the SIGTERM→SIGKILL window is *before* the
  > deadline, not after, and what actually needs covering after it is kubelet
  > observation plus watch latency plus ordinary skew — a constant, not
  > something proportional to a number the deadline already spent. Reading
  > `grace` for the *age* (`deadline − grace`, `checked_sub`) was always right
  > and is untouched
  > ([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
- **The slow half is detectable, so it is said out loud.** Any snapshot
  timestamp more than a few seconds after `now` means the laptop is behind the
  cluster. That is data k8rs already holds, and it belongs in the header in
  plain language — *"your computer's clock is 11 minutes behind the cluster —
  the times on this screen are wrong"* — not only in a test assertion.
- **The fast half is not detectable from object timestamps**, and pretending
  "just now" covers it is how the wrong belief survived this long. The honest
  source is the API server's `Date` response header, which is a Phase 5
  `k8s.rs` question and is recorded as one.

D18's sentence is corrected in place rather than left standing with a
correction pinned to it, because the next box to read that paragraph is the
renderer that implements the clamp.

### D56 — C1 cannot represent "never expires", and a rule may not return a `Result` (2026-08-12)

RFC 5280 §4.1.2.5 gives certificates with no well-defined expiry the literal
`99991231235959Z`. jiff's `Timestamp` stops before it — the range ends at Unix
second `253402207200` (`9999-12-30T22:00:00Z`) — so `Timestamp::from_second`
returns `Err` for exactly that value, and [invariant 5](CLAUDE.md) forbids C1
from propagating it.

The mapping that is both correct and permitted is **no finding**: a certificate
that does not expire has no expiry to warn about. Recorded before C1 is written
because the shape that gets typed by reflex is `.unwrap()`, and the input is a
kubeconfig — a corporate PKI is precisely where a non-expiring CA turns up, and
a panic at startup is what the user would get.

The same range is why the grace-period subtraction is `checked_sub`: v1.36.1
accepted `terminationGracePeriodSeconds: 9223372036854775807` in a server-side
dry-run against the live kind cluster, and `deletionTimestamp − that` overflows.
Anyone with `create` and `delete` on pods could otherwise kill the TUI through
a pure function that cannot fail.

### D57 — the pinned `now` is part of the fixture contract, and it makes "recent" unrepresentable (2026-08-12)

> **The value below is superseded: the pin is `2026-08-13T00:00:00Z` since
> 2026-08-13.** The mechanism this entry describes is unchanged — read it, then
> the update at the end for what moved and what deliberately did not.

The tests pin `now` at `2026-08-12T00:00:00Z`. The value was not chosen freely:
`scripts/certs-test.sh` already asserted the committed certificates against that
instant (24 days / 365 days / −3 days), so any other literal would compute C1's
arithmetic from two moments only one of which the build checks. `certs-test.sh`
now extracts the Rust pin and refuses to disagree with it — the one edge of that
coupling nothing had been guarding.

**It moves with the capture, in four places.** A re-run of `just fixtures`
stamps every object after the pin and the guard goes red — correctly, but for a
reason that is not a wrong clock. The instant lives in `src/rules.rs`'s
`fn now()`, `scripts/certs-test.sh`'s `now` and its `pinned[]` rows, and
`scripts/make-certs.sh`'s dates; they move together or the fixtures and the
certificates stop describing the same afternoon. That obligation belongs to the
capture box in Phase 2, which is where the trip is, and it survives
`rules.rs` freezing at Phase 3 close: a re-capture may touch `fn now()` and
nothing else in that file — the pin is fixture data that happens to be spelled
in Rust, not code.

> **The pin moved on 2026-08-13 to `2026-08-13T00:00:00Z`**, with the second
> capture trip, and this entry's `2026-08-12` is the superseded value — the
> mechanism above is unchanged, only the instant. All four places moved
> together. The certificates were **not** regenerated: their `notBefore` and
> `notAfter` bytes are untouched, so the day counts shifted instead — 23 / 364
> / −4 where this entry says 24 / 365 / −3, **and 22 / 363 / −5 since the
> 2026-08-14 capture moved the pin a second time, to `2026-08-14T00:00:00Z`.**
> That second move is the mechanism working rather than a surprise, and it cost
> five places rather than four: this file, `certs-test.sh`, `make-certs.sh`'s
> header, `src/rules_tests.rs` and **`docs/maps.md`**, which had gone stale
> unnoticed because nothing compares its three numbers to the script's — and
> **that stayed unguarded, deliberately, until it went stale a second time.**
> The 2026-08-15 capture moved the pin again, to `2026-08-16T00:00:00Z`
> ([D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)),
> and that row was wrong again — this time carrying **two** wrong facts rather
> than one, since it also said `now` sat *two days* into each certificate where
> it now sits four. Twice on consecutive moves is a pattern, and the fix is not
> a guard: **the numbers came out of the doc row and out of this paragraph.** A
> guard would mean `check-docs.py` parsing a bash array to compare integers in
> prose; deleting the copy costs nothing, because `certs-test.sh`'s `pinned[]`
> holds them and asserts that `src/rules_tests.rs` pins the same instant. What
> is left in prose is where the numbers live, which does not rot when they move.
> **The general rule this is the second instance of**: a fact that moves on a
> schedule should be *pointed at*, not restated — a copy nothing compares is a
> lie with a delay on it. Each
> fixture still exercises the
> case it exists for (inside C1's 30-day window, far outside it, already
> expired), and regenerating would have written fresh key material into the
> repo to buy nothing. One relationship changed quietly with it and is worth
> knowing: `now` used to equal the certificates' `notBefore` exactly, and now
> sits one day after it. Nothing asserts that equality in either direction, so
> no guard would have caught it had it mattered
> ([D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)).

**What the pin costs, which is the part nobody had written down.** It sits 43
minutes after the newest captured timestamp, so **nothing in the fixture set can
be "recent"**. Every below-threshold case — rule 7's rolling-update pod, N1's
node that went NotReady thirty seconds ago, rule 12's pod inside its grace
period — is unreachable by capture and has to be synthesised in memory under
[D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12).
That is a real cost, accepted for the certificates' sake, and naming it is what
stops the next author from reading the fixture set as "no such case exists".

### D58 — a Phase 2 box was passed over, and the order it comes back in (2026-08-12)

Phase 2 has five open boxes. Four are the kind cluster trip and their deferral
is recorded ([D47](#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12),
[D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)).
The fifth — `sanitize.jq` refusing a CSR's `.spec.username` and `.spec.groups`
([D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12))
— needs no cluster, no hardware and nobody's permission, and was passed over
anyway. Its own text says it lands *before* the trip; an audit of both phases on
2026-08-12 found it still open with `csr-pending.json` still carrying
`kubernetes-admin` and `kubeadm:cluster-admins` through the sanitizer unchanged.

Nothing excuses it: the cycle's rule is the first unchecked box in the lowest
open phase, and the four trip boxes are the *only* ones carrying an exemption.
It is recorded because the same audit found no other silent skip — every other
open box in Phases 2 and 3 is genuinely unstarted, and a phase that has been
audited once should say so, or the next audit starts from zero.

**The order it returns in**, decided when the kind cluster came back up on the
development machine: the sanitizer refusal lands **first**, then the manifests
for the shapes the first capture could not produce, then the capture, then the
teardown that closes the trip. A sanitizer fix landing after a capture is a
sanitizer that never ran on the bytes it was written for.

### D59 — the sanitizer refuses a requester, and an exit-status guard cannot see a deletion (2026-08-12)

[D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)
left one gap open on purpose: a CertificateSigningRequest's `.spec.username` and
`.spec.groups`. Closing it produced four rulings and one lesson that is bigger
than the box.

**The allowed set is derived, not curated.** "Refused, not rewritten" was
already settled; what counted as *foreign* was not. The answer is the node-name
shape — refuse anything the pinned kind cluster does not itself issue — and the
set was **read off the live cluster** rather than recalled: the two kubelet
identities with their groups, and kubeadm's admin from the kubeconfig client
certificate's subject. It is a fact about kind, not a policy about which real
identities are tolerable, which is the allowlist D52 rejected. The regex is
anchored at both ends, unlike the `k8rs-` node prefix: a node name is a family
kind generates, an identity is not, and unanchored,
`kubernetes-admin@corp.example.com` launders itself into kind's own admin.

**`.spec.extra` and `.spec.uid` are payload, deleted rather than refused, and
scoped to the object that carries the marker.** `extra` is where a real cluster
puts its OIDC claims and it cannot take the refusal — the `credential-id` the
apiserver stamps is a fresh hash every time, so there is nothing to allowlist.
The scoping is not fussiness: a bare `del(.. | objects | .uid?)` empties
`metadata.uid` on all 23 fixtures — the identity the rule engine is built on —
while still passing every test written about a CSR.

**The marker is `signerName` *or* `issuerRef`, and the first ruling was
reversed.** `signerName` alone was accepted as a deliberate narrowing with
cert-manager's `CertificateRequest` recorded as a known miss; the operator
review then put that kind through the filter and it came out **unmodified**
carrying `alice@corp.example.com` and an OIDC claim. A known miss with a live
proof is not a narrowing, it is the leak. Keyed on the marker rather than on
`has("request") and has("username")`, which reads better and fails open exactly
when it matters: an object carrying `groups` and `extra` without a `username`
is then never examined, and those are two of the four fields being protected.

**An accepted limit, recorded rather than engineered around.** A requester
identity sits in three places on a CSR — `.spec.username`, the DN inside the
base64 `.spec.request`, and the DN inside `.status.certificate` — and jq cannot
decode a DER subject, so the guard reads one of three. A production kubeadm CSR
carrying `CN=alice@corp.example.com` in its request passes, because
`CN=kubernetes-admin, O=kubeadm:cluster-admins` is what **every** kubeadm
cluster calls its admin — that half of the allowed set is not kind-specific at
all, and the comment claiming it was has been corrected. Closing it needs
openssl in `fixture-audit.sh`, which is where it would live if it is ever
worth it.

**The lesson, which outlived the box: a guard that asks for an exit status
cannot see a deletion.** `fixture-audit.sh` gained a backstop asking
`sanitize.jq` whether it would *refuse* each committed fixture — and
`csr-pending.json`, captured before the `del(.extra, .uid)` clause existed,
still carried the credential hash. The filter accepted it, so the backstop
passed, and the audit's green line was byte-identical to a clean run. The
question worth asking is not "would the filter refuse this" but **"would the
filter change this"** — idempotence. Twenty-two of the twenty-three JSON
fixtures were already byte-identical under the filter; the twenty-third was the
one that had never met it. Nothing leaked — the value was a thumbprint of a
throwaway kind admin certificate — but the guarantee that
[G-5](REQUIREMENTS.md#devsecops-requirements) rests on had quietly stopped
holding, which is the same failure as
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) wearing different
clothes.

**And the guard's own failure mode was worse than the leak it caught.**
`make-csr.sh` sanitized with `jq -f sanitize.jq … > "$out"` where `$out` *is*
the committed fixture: the shell truncates before jq runs, so a refusal — the
one thing the filter exists to do — destroyed the file the script exists to
produce, and `set -euo pipefail` then skipped the cleanup and left a CSR on the
cluster. It writes to a working copy and moves on success, and the delete moved
into the `EXIT` trap. *"The cluster is left as it was found"* was true of the
happy path and nothing else.

### D60 — CLAUDE.md was compressed, and four stories moved here (2026-08-12)

`CLAUDE.md` is loaded in full at the start of every session and again after
every compaction, before a single line of work has been read. Measured, it was
682 lines and 6,548 words — roughly 9,500 tokens of standing cost, of which
§ Agent workflow alone was 30%. It is now ~3,000.

**The rule applied: every rule stays in `CLAUDE.md`, the story behind it moves
here.** That is not a new split, it is the one
[docs/maps.md](docs/maps.md) already states — *why* is this file, *how must I
work* is `CLAUDE.md` — and `CLAUDE.md` was the file breaking it, carrying
paragraphs of justification for rules that already link to a `D`-number. No
invariant, no security-gate box, no ownership row and no cycle step was
dropped; what went is prose that argued for them a second time.

**Two defects the pass found, both invisible while writing.** The header read
*"Current phase: CODE — Phase 1, `feat/scaffold`"* while the same file's git
rules said all work is on one long-lived `development` branch
([D32](#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)) —
a hand-maintained status line that rots the moment a phase closes. It is gone
rather than corrected: the *rule* for finding the next box is stable, the phase
number is not, and `todo.md` already answers it. And § Workflow (per feature)
described the same seven steps as § The cycle, having drifted apart in the
middle — one ordered docs-sync after the `todo.md` tick, the other before.
There is one description now, in the cycle, and its step 7 spells the order.

**Four things existed only in `CLAUDE.md` and are recorded here so the rules
they justify do not become arbitrary:**

1. **The scratchpad is a shared tree.** `tester`'s mutation driver was
   overwritten between rounds by a file it never wrote; running it would have
   re-run **the author's own sweep** and reported it as independent
   verification — a plausible, well-formatted, entirely worthless result, and
   the exact closed loop step 4 exists to break. Hence one subdirectory per
   agent, and a hash or line count before anything saved earlier is reused.
2. **Review is not a parallel slot.** Work stacked on an unreviewed box turns a
   rejection into a rebase, and a rebase under time pressure is how a finding
   gets quietly dropped. The dev idles during review; that idle is the price of
   the gate meaning something.
3. **`just check` is the whole of CI, or it is a lie.** `cargo deny` first went
   red *after* a push, because CI ran a step `just check` did not. A step whose
   tool is not installed locally is added anyway — a missing binary is a loud
   error, a missing step is an invisible gap.
4. **"Run it" means something different before `main.rs` exists.** Demanding a
   binary run in Phase 3 would set a gate nobody can pass, and an impassable
   gate teaches everyone to wave gates through — which is
   [the second pass's own item 2](CLAUDE.md#second-pass--nothing-is-delivered-on-its-first-draft)
   turned on the file that states it.

**The limit going forward.** A new rule costs its own line in `CLAUDE.md`; its
justification costs a `D`-number here. If that file passes ~4,000 words again,
what grew is prose, not rules — check before adding.

### D61 — a verify predicate must hold across the whole window, not at one instant (2026-08-12)

`cluster.sh verify` certifies that a fixture reached the state its rule is
about, and `just fixtures` captures that object seconds later. A predicate that
names **one instant of a cycle** therefore certifies a moment that may already
be over by the time the bytes are written.

`[crashloop]` demanded `state.waiting.reason=="CrashLoopBackOff"`. Sampled on
the live cluster, the container was in `state.terminated` 39 times out of 70,
in `waiting: CrashLoopBackOff` 29 and `running` twice — the predicate named the
**minority** half of its own loop. It failed open twice over: `verify` retries
in a wait loop until the flap happens to land right, and its report pass
re-fetches, so a green line proved the state existed at *some* instant, not
that a capture would find it. What actually holds across the loop is *it has
already restarted after an exit 1, and it is not up right now*, and that is
what it now asserts. The ~2s window where it **is** up stays excluded on
purpose: certifying a Running pod as crashlooping is the lie the function
exists to prevent.

**The sweep found one, not two.** The first report named `[init]` as well; that
was asserted from family resemblance rather than read, and `[init]` was already
an `or` over both halves. Recorded because a review that names a defect it did
not read is the same failure as a test that has only ever been green — this one
happened to be wrong in the harmless direction.

**The report/wait asymmetry stays, deliberately.** `verify` waits on one read
and reports on a re-fetch, and that asymmetry is the only reason this class was
visible at all: had it reported the verdict it waited on, a flapping predicate
would have printed PASS every time and stayed invisible until a fixture landed
in a state no rule could fire on. It is also the last read before the capture's
own `kubectl get`, so it is the closest available proxy for the bytes that will
land.

**Two limits accepted rather than engineered away:**

- Every `sleep 3600` fixture exits and restarts an hour after `break`, and
  `[restarts]` asserts `ready==true`, which is false for a moment then. The fix
  is a constraint on the trip, not on the code — `break → verify → fixtures`
  runs well inside the hour — because `ready==true` is the whole point of
  rule 5's fixture.
- `[init]` is loose in the *other* direction: it matches an init container that
  crashed and then succeeded, since it never asks whether the container is down
  now. No fixture in the set can produce that shape, so tightening it would
  change a passing predicate with nothing to prove the difference.

**`broken-owned` is a third workload shape, not a second W2.** Its Deployment
reports `Progressing=True / NewReplicaSetAvailable` with `Available=False /
MinimumReplicasUnavailable` — the replica set rolled out fine and the pods
inside it are dying, which is not
[W2](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)'s
`ProgressDeadlineExceeded`. Both will be in `deployments.json`; they are
different findings.

**D39's claim is now asserted somewhere.** That kubelet writes an
`ownerReference` of kind `Node` onto every static pod was documented upstream
and checked by nobody here; `verify-test.sh`'s corpus carries a real mirror pod
as the negative `[owned]` must refuse. This does **not** close the mirror-pod
box — that one wants the fixture, for N2's `mirror: true` count and rule 8's
only negative.

**Deferred, with the reason.** `verify` certifies the live object; nothing
asserts that the **written** fixture still satisfies the predicate that
certified it — the one check that would catch "verify passed, the capture
landed two seconds later in another state". It belongs in `fixture-audit.sh`,
it is ~12 lines, and it waits until after the capture: writing it now means
proving it against 23 files that the capture is about to replace, two of which
do not exist yet.

### D62 — the fifth place a node name lives, and a guard that asked less than its consumer (2026-08-12)

`kube-system` is the one namespace `broken.yaml` cannot imitate, and preparing
to capture it opened two holes in a sanitizer that had only ever been fed pods
built from that file.

**A node name lives in five places, not four.**
[D31 § 3](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
enumerated four fields beyond `.spec.nodeName`; the fifth is the
`ownerReference` of kind `Node` that kubelet writes onto every static pod —
`{kind: Node, name: prod-master-01}` names a machine exactly as well as
`.nodeName` does. Before the fix, a pod named `etcd-prod-master-01` owned by a
foreign `prod-master-01` and carrying no `.nodeName` was **sanitized and
written** rather than refused. The enumeration was not careless: it was
complete for the shapes it had been fed, which is
[D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12) again
on the field axis instead of the object axis. Note the same clause must return
nothing for a *full* Node object, which carries `.kind == "Node"` and its name
under `.metadata.name` — hence the `// empty`.

**IPv6 gets one exemption from its anchor: the bracketed URL form.** D31
anchored IPv6 to the whole string on purpose, because unanchored `::` matches a
Rust path, a C++ scope operator and every `key::value` in a log line. `[fd00::1]`
is exempt because the brackets are what remove that ambiguity — nothing in
prose is bracketed like an address. It is the same two forms as the anchored
rule, in the framing `-n kube-system` introduced: etcd and the apiserver carry
their addresses inside `--listen-client-urls=` and friends. Proven against
planted input only — `cluster.sh` writes no `ipFamily`, so the test cluster is
IPv4 and the real capture contains no IPv6 at all.

**A guard may not ask a weaker question than the thing it guards.** Both new
owner predicates first tested `ownerReferences[].kind == "Node"` with no
`.controller == true`, while D46's ruling above and `rules.rs` both resolve the
*controlling* reference — and the `owned` guard written one box earlier, twelve
lines up in the same recipe, does check it. Three poisoned captures that would
produce **zero** mirror pods for N2 passed green. Not a live leak, since kubelet
always writes `controller: true`; the defect is that a guard certifying a
fixture for a consumer must ask the consumer's question, or it certifies
something the consumer cannot use. Two questions about one field in one file,
two boxes apart, is the cross-box drift the phase-close pass exists to catch.

**`kubernetes.io/config.mirror` is deliberately not among the guards.** The
filter destroys every annotation, which is exactly why D46 takes the mirror bit
off the ownerReference instead. Recorded so its absence does not later read as
an oversight.

**Accepted rather than fixed, each with its reason:**

- **No `kube-system-pods.json` entry in `fixture-audit.sh`'s must-still-be-there
  list.** It would be a third copy of a predicate that already has two homes,
  and [D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)
  is explicit that the audit and the sanitizer went blind in the same places
  *because* each kept its own copy. The audit's copy-free backstop — re-run the
  filter, demand byte-identity — covers the sanitizer-drift case and now
  reports a foreign Node ownerReference as a refusal.
- **A node name inside a command-line flag is not refused.**
  `--name=k8rs-control-plane` and `--initial-cluster=<node>=https://…` carry the
  identity as a substring, and `node_names` reads fields, not prose. `.spec.nodeName`
  sits on every one of those pods, so the refusal fires on the neighbouring
  field; a substring refusal over arbitrary hostnames cannot be written without
  false-positiving on `--cluster-name=k8rs` and `registry.k8s.io/…`.
- **The capture is taken whole** (~115 KB, the largest fixture in the set).
  Trimming it is hand-editing, which
  [D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)
  forbids. The name is `kube-system-pods.json` and not `kube-system.json`
  because it is pods only.

### D63 — the field kubectl never writes, and a substitution test that could not see a clause (2026-08-12)

The last Phase 2 manifest box added thirteen shapes the first capture could not
produce. Two operator reviews, run independently on the same diff, converged on
one blocker — and it was not in a manifest, it was in the predicate meant to
certify one.

**`kubectl taint` writes no `timeAdded`, for any effect.** The predicate for the
`dedicated=gpu:NoExecute` fixture demanded one, on the strength of the API
type's own sentence — *"it is only written for NoExecute taints"* — which reads
as a promise and is not one. The only writer in the tree is the node
lifecycle controller's `SwapNodeControllerTaint`, for the taints it adds
itself; k/k [#113044](https://github.com/kubernetes/kubernetes/issues/113044)
asks for the flag that would change that and has not got it, and
[#131644](https://github.com/kubernetes/kubernetes/pull/131644) deleted the
sentence from the type as inaccurate in the other direction too — the
controller stamps `NoSchedule` taints as well.

What that would have cost is the part worth recording. `break-nodes` applies
the taint *first* and asserts *after*, so the eviction lands, the poll loop
then spends its whole 420s on a taint that was applied perfectly, `set -euo
pipefail` ends `just fixtures` at its last line, and `nodes.json` — the one
capture the whole subcommand exists for — is never written. Every other
fixture is already on disk by then, and the bare pods the `NoExecute` taint
evicted are gone for good, so the retry is not "run it again": it is `unbreak`,
`break`, and a fresh ten-minute settle. **A predicate that cannot pass is more
expensive than a rule that is wrong**, because it fails at the end of the most
expensive step, and it fails looking exactly like a cluster that did not
cooperate.

**The timestamp is recovered from the taint nobody types.** Stopping a kubelet
makes the controller add `node.kubernetes.io/unreachable:NoExecute` through the
one function that stamps a time, so `[notready]` asserts the timestamp and
`[tainted]` asserts only what `kubectl` actually writes — key, value, effect.
The cordon's own `node.kubernetes.io/unschedulable:NoSchedule` taint is
asserted to *exist* and not to carry a time: whether a `NoSchedule` taint keeps
its stamp is the half nobody could settle from the source, and asserting it
would have rebuilt the trap one clause over.

**Two files outside this box now rest on the deleted sentence**, and neither is
touched here — recorded so the trip's own `nodes.json` is read as the answer to
both:

- `rules.rs`'s taint test names `kubectl taint … dedicated=gpu:NoExecute` as
  the object that will retire its synthesis. It will not; the controller's
  unreachable taint will. `dev-core` re-points it when the capture lands.
- [`screens/alerts.md`](screens/alerts.md)'s cordon card argues that a cordon
  has no age *because* `timeAdded` is NoExecute-only — the sentence that was
  deleted. If the unschedulable taint carries a stamp, a cordon time is
  readable and [D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)'s
  premise moves. `[cordoned]` now requires that taint to be in the capture, so
  the trip settles it either way. Nothing is built on it before then.

**A substitution test proves a predicate has two halves; it cannot prove a
clause does any work.** Replacing each predicate with `true` and with `false`
and demanding both go red — the discipline
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) installed — was run
over all twenty-six and passed, while `[pending]`'s new toleration clause was
dead: every corpus object that had to refuse the predicate already failed on
the *nodeSelector* clause first, so the toleration could be deleted with the
file staying green. The fix is to delete each clause on its own and demand a
red, which needs an object that differs in exactly that clause — and building
those is what turned up the second false premise in the same box: the manifest
justified its respin by saying no captured pod carried `tolerations`, when the
`DefaultTolerationSeconds` admission plugin puts two on every pod in every
capture. What the respin genuinely adds is a toleration with a *value*.

**The reviews' other seven findings, and what each one was.** A second `break`
could not run at all, because a resized pod's `spec.resources` may not be
changed by an `apply` and nothing put it back (fixed by resetting through the
`resize` subresource first — and a cluster holding an *earlier generation* of
these manifests still cannot be applied over, which is now written where it
happens instead of claimed away). `100Gi` was infeasible only on small
machines, since a kind node reports the host's memory — `1Pi` is infeasible
everywhere, **and the cluster then falsified the conclusion drawn from that**:
a request above the node's allocatable is refused at *admission*, so it never
reaches the kubelet and parks nothing. Being infeasible everywhere is exactly
what stops `1Pi` from producing this fixture. Nothing asserted that the cordoned worker still carried a pod a
drain would move, i.e. that N2's positive fixture was not N2's negative wearing
its name; the node is now *chosen* by what is on it and the choice is asserted
against the committed bytes. The `why` line claimed an N3 positive that a
stopped kubelet does not produce — pressure conditions need an eviction
threshold crossed, which is a cluster change, so **N3 joins the permanently
synthesized list**. The one step that can fail on permissions ran third, after
two destructive ones, on a machine where docker access is per-login; it is
preflighted now, and `unbreak` reports what it undid instead of swallowing a
denial that looks identical to nothing-to-undo. A guard asserted a termination
message was non-null when `REDACTED-IP` is also non-null. And a comment claimed
`verify-test.sh` guarded the usage text's line range, which it never did —
**an invented guard is worse than none, because the next editor trusts it**; the
range is gone rather than the claim.

**Decided in passing, none of it forced by the box:** three workers by default
(one per node state, so no node fixture has two causes; `break-nodes` refuses
on fewer rather than doubling up) · `break-nodes` is its own subcommand called
last by `just fixtures`, never part of `break`, because every one of its three
actions changes a pod state that is still settling · the resize is a capture
step like rule 12's delete, since a pod that is not running has no enacted
resources to disagree with · `broken-sts` carries two replicas so it can be
*partially* ready, which is the only shape that separates `ready` from
`desired` · `broken-rollout` gets an hour's progress deadline so it cannot
become a second W2 fixture mid-trip · the three healthy shapes are separate
pods, because the `healthy` pod is every rule's negative at once and a host
mount on it would leave rule 8 with two positives and no negative · the
toleration moved to `NoExecute` to match the taint rather than the other way
round, since with `timeAdded` gone the effect is the only thing separating
`[tainted]` from its negative.

### D64 — the capture trip: what the cluster settled, and the approval it reversed (2026-08-13)

The capture Phase 2 was built for was finally taken, against a four-node kind
cluster on `v1.36.1`: **23 of 23 pod predicates and 3 of 3 node predicates
passed, and 34 fixtures landed.** Everything below is something no amount of
reading upstream could have produced, which is the argument for the trip.

**The resize fixture cannot use a constant, and the reason inverts the one this
repo had written down.** `100Gi` was rejected as machine-dependent and replaced
with `1Pi` on the grounds that it is infeasible everywhere — which is true, and
is exactly why it produces nothing. A request above the node's allocatable is
refused at *admission*:

```
Error from server (Forbidden): pods "broken-resize" is forbidden: node didn't
have enough allocatable resources: memory, requested: 1125899906842624,
allocatable: 24860065792
```

It never reaches the kubelet, so there is no parked resize and no divergence to
capture. The window that is admitted-and-unenactable is `(available,
allocatable]`, and its top edge is the only point in it that does not depend on
what the other pods on that node happen to be holding — so `break` now reads the
allocatable of whichever node the pod landed on and asks for exactly that. The
number is written down nowhere.

**And the reason is `Deferred`, not `Infeasible` — an approval I gave was
falsified by the cluster.** The review asked for the predicate to demand
`Infeasible`, on the argument that `Deferred` is "not right now" and resolves
itself; I approved it. Anything that would be `Infeasible` dies at admission, so
that tightening made the predicate unsatisfiable by the only path `break` has.
The lesson is not about resize: **a reviewer's argument and my agreement with it
are still both claims**, and the two of them agreeing is not evidence. The
predicate now accepts both spellings and holds the line with a reason *enum*
rather than a single value, so an in-flight `PodResizeInProgress` still fails it.

**`break` was half-idempotent, which is worse than not being idempotent at
all.** The second run hung: `rollout status statefulset/broken-sts` never
returns once an earlier `break` has left a pod on the bad revision, because
under `OrderedReady` the StatefulSet controller will not touch an unready pod,
so `broken-sts-1` stays on a revision the object no longer has — permanently,
not slowly. The guard meant to prevent exactly this had been dead since it was
written: it read the workload's *template* to ask "is this already broken", and
it ran after the `apply` that puts the good template back, so it could only ever
answer no. **A comment described the design; the code did not implement it, and
nothing failed until a second run happened.** The fix reads the **pods'** images
instead — the one fact the apply cannot rewrite — with each workload's selector
taken off the workload itself rather than spelled as a convention shared with
`broken.yaml`. The Deployment survives the same starting state by structure and
not by luck: its replicas have no identity, and with `maxUnavailable: 0` the
unready pod is a surge pod above the desired count, so deleting it costs no
availability.

**The premise under
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
is false: a plain `kubectl cordon` does leave a timestamp.** The capture shows
`node.kubernetes.io/unschedulable:NoSchedule` carrying
`timeAdded: 2026-08-12T21:43:02Z`, while the hand-applied
`dedicated=gpu:NoExecute` — the *more* privileged effect — carries none. The
dividing line is not the effect at all, it is **who wrote the taint**. `kubectl`
is a client: `cordon` sets the bare `spec.unschedulable` boolean and stamps
nothing, and `kubectl taint` writes the taint the user typed and nothing more.
The node lifecycle controller then mirrors that boolean into a taint, and every
taint it adds goes through `SwapNodeControllerTaint`, which does
`taintToAdd.TimeAdded = &now` for each one **before looking at its effect** —
the NoSchedule pass that adds the unschedulable taint calls the identical
function as the NoExecute pass. Read in upstream source and not only in the
capture, because a fixture only ever proves what one version did.

D43 was careful about the citation and wrong about the inference. It quoted
`k8s-openapi 0.28.0`'s `Taint::time_added` — *"It is only written for NoExecute
taints"* — noted that the sentence is **gone from the v1_34+ generated docs**,
and concluded "the behaviour is unchanged". The behaviour was never what that
sentence said: upstream's own comment today is *"TimeAdded represents the time
at which the taint was added"*, `+optional`, with no mention of effects. Reading
the *doc* of a field rather than the *writer* of it is what cost the entry, and
the writer was two function calls away.

What survives D43 untouched is everything downstream of that premise — the
autoscaler and Karpenter split, and N2 staying quiet while a scale-down taint is
present. What falls is only "there is no source for that number". What follows
for `screens/alerts.md`, which drew the card without an age on D43's authority,
is `tui-designer`'s and is not settled here.

**What the trip proved that could not be proven offline**, all of it previously
carried as an assumption: the kubelet does copy a pod-level memory limit into a
container status whose own spec declares only cpu ([D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12));
a Deployment mid-rollout with `maxSurge: 1 / maxUnavailable: 0` reports exactly
the five counter values the fixture was designed around; a StatefulSet's
`updatedReplicas` counts a created-but-not-ready pod; and the kubelet keeps a
log tail in the termination message under `FallbackToLogsOnError`.

**Decided in passing, none of it forced by the box:** the resize predicate keeps
its unreachable `.status.resize` string branch — the pin is `v1_32` and the
support window is pinned ±2, so a server that spells it that way is inside the
window even though the pinned node image is not, and a predicate naming only the
reachable half is one nobody can read a year from now · `scan_second_revisions`
and the break loop are fed by one array, because a workload in the second list
and not the first walks back into the wait it cannot survive · an unreadable
selector falls back to *waiting*, never to skipping, since `-l ""` matches every
pod in the namespace and `broken-image` carries the same bad image · the trip's
teardown reads the capture **before** `unbreak`, so a missing shape is caught
while the cluster that could still produce it exists.

**One hazard is known and deliberately left open:** `break` waits on
`pod/broken-resize` becoming Ready, and on a cluster where `break-nodes` has
already run that pod can sit on the kubelet-less node or be evicted by the
`NoExecute` taint — the wait then burns its 300s and ends the run at the same
place the StatefulSet used to. That is a `break-nodes` leftover rather than a
`break` one, and what `break` should do about a cluster `break-nodes` damaged is
a plan decision; `unbreak` is the documented answer until it is made.

### D65 — the repin: N2 gains a clock, and what two agents decided that no brief did (2026-08-13)

The capture stamps every object after the old pin, so the pin moves with it:
**`2026-08-13T00:00:00Z`**, midnight after the capture day, 2h16m after the
newest moment in the new fixtures. The value was decided by the PM and taken
away from both agents on purpose. D57 puts the same instant in four places
across two ownership rows, and `certs-test.sh` reads the Rust pin and refuses to
disagree with it — so two agents each choosing a defensible value produces a red
build whose cause reads like a clock bug. A fact written in four places is not
four decisions.

**N2 gains a clock, and that is a rule capability change, not a test detail.**
`Taint::added_at`'s doc said upstream writes `timeAdded` for `NoExecute` taints
only, therefore N2 could never say how long a node had been cordoned. The
capture shows the split is *who wrote the taint*, not which effect: the node
lifecycle controller stamps `timeAdded` on every taint it adds — including the
`NoSchedule` one it mirrors from `spec.unschedulable` — and `kubectl taint`,
being client-side, stamps none. So the cordon **does** carry a time and N2 can
say "cordoned 2 hours ago"; the `Option` survives, for the hand-applied taint
rather than for the cordon. This is the last standing consequence of the premise
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
was built on, and it lands as a capability the rule may now use — not one it
must. `screens/alerts.md`'s cordon card is `tui-designer`'s round.

**The certificates were not regenerated.** Their `notBefore`/`notAfter` bytes
are committed evidence; moving `now` past them changes only what the arithmetic
says — 23 / 364 / −4 where it used to say 24 / 365 / −3, and again at every
capture since; **the current values are `certs-test.sh`'s `pinned[]` and are
deliberately not repeated in prose any more**
([D57](#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)). Each fixture still
exercises the case it exists for, and regeneration would have written fresh key
material into the repo to buy nothing. One relationship ended quietly with it:
`now` used to equal the certificates' `notBefore` exactly and now sits a day
after. Nothing asserted that equality in either direction, so it was free to
break — which is the point worth keeping, not the day.

**What re-deriving the seventeen tests actually bought.** They were fitted to
literals — a uid, `restarts == 5`, a scheduler sentence — so a bigger cluster
reddened them and the cheap repair is to paste in the new literal, which is
fitting the test to the answer. They now read the capture's own JSON *at the
path the field must have come from*, which is not a tautology: the decode is not
what it is read from, so a field dropped, filled from its neighbour or rewritten
still fails. Each pairs the derivation with the property the fixture must keep —
`restartCount` **and** `≥3`, because CrashLoopBackOff implies several deaths —
so a fixture that goes soft is still caught. Two things the old shape hid
surfaced: one hostPath synthesis had gone **degenerate**, cloning a container
into a collision with the real one, and `nodes.json`'s "nothing is wrong
anywhere" loop was **false by construction** against a deliberately broken
cluster.

**The choices the briefs did not make, recorded because nobody could
reconstruct them later.** From `dev-core`: `image.json` decodes `ErrImagePull`
and not `ImagePullBackOff`, because the kubelet alternates between the two for
one broken image and nothing in `just fixtures` waits for either — the test
derives the reason from the capture and asserts it is one of the two, and this
was the single failure where "the world changed" and "the requirement changed"
were genuinely hard to separate; the three hostPath syntheses are **retired**
rather than kept beside the capture, which is what their own notes said landing
meant; `restarts.json` asserts the band `(3..10)` as a deliberate tripwire, so a
pod that drifts past ten reddens the build for having stopped being rule 5's
*WARN* fixture; kindnet's `desired` is **not** coupled to the node count, which
is honest today and a false red the day a node stops tolerating it; and N4 gains
a cross-check that the control plane's kubelet version equals
`tests/fixtures/K8S_VERSION`, so a fixture that acquires a skew is announced
rather than discovered. From `tester`: the repin was **staged in two steps** to
force a real red on the day-count assertions, because the free red only
exercised the cross-file check and those three would have gone green to green
unseen; and two comments outside the lines the brief named were corrected, since
a file whose header narrates a count its own guard no longer produces
contradicts itself.

**A concurrency hazard worth knowing before it costs an hour.** `certs-test.sh`
`sed`-reads the pin out of `src/rules.rs`. Run while a dev holds that pen, it
read a half-written file and reported a **false red** — five consecutive runs
passed seconds later. Disjoint file trees make two agents safe to run in
parallel; a guard that reads across the boundary is the exception, and a lone
cross-file red is suspect until the writer puts the pen down.

### D66 — `just check` is not quite the whole of CI, and the gap is the one CI was built to watch (2026-08-13)

> **Closed the same day it was opened —
> [D67](#d67--the-cross-compile-row-closed-with-a-skip-and-what-the-skip-costs-2026-08-13)
> records which horn was taken.** `just check` now ends with `just cross`, so
> the row below is history: the eleven-out-of-twelve caveat is spent, and the
> deferral in the middle paragraph was a deferral of about six hours.

Found by Phase 2's closing second pass, against
[CLAUDE.md § Running it](CLAUDE.md): *"`just check` is the whole of CI, or it is
a lie."* Comparing the two step lists, they agree everywhere except one row.
`cargo deny` is not the exception it looks like — `just check` runs the binary
and CI runs the pinned `cargo-deny-action`, which is the same check by a
different vehicle. The real one is the **cross-compilation matrix**: CI runs
`cargo check --locked --target <t> --all-targets` across the release targets
and `just check` runs nothing equivalent, so a cross-compile break can only be
discovered after a push. The workflow's own comment says why that matters —
"cross-compilation breaks at link time and it breaks late" — which makes this
the precise failure the rule exists to prevent.

**It is recorded rather than fixed, and that is a judgement, not an oversight.**
The obvious repair — add the targets to `just check` — makes the gate red on
every machine that has not run `rustup target add`, including the one closing
this phase, and a gate that is red by default is one everybody learns to wave
through. The alternative, a skip when the target is missing, is the "loud
error" the rule prefers only if the loudness survives; a green run with a
skipped step in it is what the rule calls an invisible gap. Choosing between
them is a `tester` decision with a real cost either way, and it belongs to
whoever owns the CI box, not to a phase close that found it in passing.

**What is not deferred:** the claim. Until it is closed, "`just check` is the
whole of CI" is true of eleven steps out of twelve, and anybody relying on it
for a cross-target change should run the matrix by hand.

### D67 — the cross-compile row closed with a skip, and what the skip costs (2026-08-13)

[D66](#d66--just-check-is-not-quite-the-whole-of-ci-and-the-gap-is-the-one-ci-was-built-to-watch-2026-08-13)
named two horns and left the choice to `tester`. **The skip won, and on this
machine it was not really a choice:** there is no `rustup` here at all —
`/usr/bin/cargo` is the distribution's rust — so "require the four targets"
does not mean "inconvenient until someone runs `rustup target add`", it means
`just check` is red forever on the machine that closes phases. That is D66's
own *"a gate red by default is one everybody learns to wave through"* in its
worst available form.

**The skip is a narrow, deliberate exception to
[CLAUDE.md § Running it](CLAUDE.md)'s *"a missing binary is a loud error, a
missing step is an invisible gap"*, and the exception is smaller than it
looks.** What that sentence is protecting is the *step*, and the step is now in
`just check` where it was not before. What is missing here is not a binary but
a target's standard library, and the rule offers no verdict on that case. The
loudness is paid for three ways, all of which had to survive a **green** run:
`cross` is the last thing `check` runs, so the banner is the last thing on
screen; the banner names every target that did not run rather than counting
them; and it goes to stderr, so redirecting stdout to a log does not eat it.

**Two failures are deliberately *not* skips**, because either would shrink the
gate in silence — the same invisible gap wearing a different coat. A triple
`rustc` has never heard of is a typo in CI's matrix and fails hard. A matrix
the recipe cannot read out of `ci.yml` fails hard too, rather than cheerfully
checking an empty list.

**The coupling nobody asked for, and why it is the right one.** `just cross`
reads the `- target:` lines straight out of `.github/workflows/ci.yml` instead
of holding its own copy. A second copy of that list is *precisely* the material
this row is made of — a list in two files that agree until they do not — so
closing the drift with a fresh instance of it would have been a repair that
reopens itself. It is guarded by a canary
([CLAUDE.md § a derived list asserts it found something](CLAUDE.md)):
`x86_64-unknown-linux-musl` must appear in the extraction or the recipe stops,
since "extracted nothing" and "nothing to extract" print the same line.

**The direction of that dependency is worth noticing**, because it is the
opposite of the structural fix still recorded as open above — *a CI job that
installed `just` and ran `just check`*. That imagined the workflow depending on
the justfile; what landed is the justfile depending on the workflow. Both leave
exactly one list, so neither blocks the other, and if the CI-runs-`just check`
fix is ever taken the two compose rather than collide. It is a partial step in
that direction and not a substitute for it.

**What could not be proven, stated rather than hidden.** No cross std exists on
this host, so the *non*-skip branch was proven against the host triple
(`x86_64-unknown-linux-gnu` temporarily added to the matrix): same code path,
same command string, only `$t` differs — a real type error in a temporary test
file took `just check` to exit 101, and the healthy run before it genuinely
compiled the crate. A musl- or darwin-*specific* break remains unprovable here,
which is the gap the banner exists to announce. And "loud enough" is a human
judgement, not a testable claim; what is testable and true is *last on screen,
on stderr, every skipped target named*.

**Found in passing and fixed:** `cluster-up`'s `just --list` description was a
dangling fragment (`# worker per node state break-nodes produces…`), because a
two-line comment above a recipe loses its first line to the listing.

### D68 — the age ladder is not the formatter's choice, and what the brief still left open (2026-08-13)

`Finding` now carries `timestamp: Option<Time>` and `rules::age(now, event)`
turns it into words. Two things about that were **taken away from the agent on
purpose**, for the same reason D65 took the pin away from two of them:

**Where the formatter lives.** [Invariant 5](CLAUDE.md) says the *renderer*
turns a timestamp into "4 min ago", and the renderer does — but the function it
calls sits in `rules.rs`, the only file the pyramid lets both callers reach:
`ui.rs` is Phase 11 and the `--once` printer is this phase's temporary
`main.rs`. `screens/once.md` states the consequence outright — *if the two
renderers could disagree about the same finding, one of them is lying* — and
two copies of a ladder is how they would.

**The rungs themselves**, which are not a formatting preference: every string
is one a `screens/` file already prints. `40s ago` from `states.md`'s stale
vitals, `4 min ago` from `alerts.md` and `once.md`, `6 days ago` from the
cordon card `alerts.md` describes as the age it lost. `min` stays abbreviated
and unpluralised because that is how both screens spell it; hours and days are
words and get their singular. The hours rung is the one nothing draws yet — it
follows from the days rung above it rather than from a screen.

**What `dev-core` decided that no brief did, recorded so nobody has to
reconstruct it.** The function is `age` and the private pluraliser is
`counted`. "No age at all" stays `None` all the way to the renderer instead of
arriving as an empty `String` that neither renderer could tell from a
formatting bug — the operator review below then moved the whole render decision
behind `Finding::age`, so no caller writes the flattening at all. Days come from
`as_hours() / 24` rather than
a second division of seconds, so no bare `86400` is in the file. `timestamp` is
the last field of `Finding`. The fixture test asserts the **band before the
string** — the cordon must land on the hours rung, *then* read `2 hours ago` —
so a recapture that moves the pin fails with a repin hint rather than on the
phrase. And `Finding` is not a snapshot type, so the pinned-`now` sweep gains
no walk: the moment a rule puts there comes from a field that sweep already
covers.

**The one rung the brief did not reach, found by the agent's own second pass:**
an event 400ms old. The ladder was keyed on whole seconds and said nothing
about the sub-second window, where the truncation produces `0s ago` — a string
no screen draws and one that reads as a stopped clock. It says `just now`,
which puts a *positive* age in the branch [D18](#d18--the-clock-is-an-input-not-an-ambient-fact)
built for negative ones; that branch is now "too new to count" as well as "the
laptop is behind", and only the second half is about a wrong clock.

**Still open, and it is `tui-designer`'s:** `screens/alerts.md` § *the cordon
card* and `screens/once.md` both still argue from
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)'s
falsified premise — quoting `Taint.timeAdded` as *"only written for NoExecute
taints"* — and the test landing here asserts the opposite for the card N2 will
file. Nothing ships a contradiction yet, because N2 itself is a later box, but
the two have to be reconciled before it does. [D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)
handed that round to `tui-designer` and this is the second entry pointing at
it.

### D69 — the operator review that reopened the box, and the prune line that was never true (2026-08-13)

The `Finding.timestamp` box passed its own second pass, `just check` and a real
`--nocapture` run, and the **operator review sent it back anyway**. Every
finding below is closed here — fixed, or deferred to a box with a named owner.
None was rejected, which is worth saying plainly: the gate earned its place for
the fourth time on the same contract
([D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) ·
[D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) ·
[D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).

**The blocker: `just now` had no bound on the future side.** `elapsed <= 0`
rendered *any* moment after `now` as "just now" — an event 25 hours ahead
included, and the ladder test pinned that as the requirement. The `Option`
catches *"there is no field to point at"*; this branch **hid** *"the rule
filled the wrong field"*, and the wrong fields here are future-dated by nature:
C1's `notAfter`, C2/C4's the same, rule 12's raw `deletionTimestamp` inside its
grace window — which the snapshot sweep already documents as legitimately
future. A confident wrong string, on the screen whose whole promise is that a
number on it can be believed.

`age` now answers `Option<String>` and produces **no number** past the bound,
which is `screens/alerts.md` § *No number we cannot produce* applied to the one
case the code exempted from it — the blank right edge already exists and needs
no new screen state. **The bound is five minutes and it was the PM's to set,
not the formatter's:** five minutes is the conventional clock-skew tolerance
(Kerberos, JWT `nbf`/`exp` leeway, most TLS handshake allowances), it covers an
unsynced laptop without argument, and past it the honest answer is not a
smaller number but the header line D55 asked for. Which is the second half of
this decision, because a screen that goes quietly blank is a worse bug than the
one it replaced — see the deferrals below.

**The doc sentence that would have frozen an age two screens require to
advance.** *"`now` is `ClusterSnapshot::now`, handed in"* reads as binding on
every caller; combined with `now` captured once per pass and invariant 7's
block-when-idle, the header's stale-vitals age (`screens/states.md`
`nodes 3/3 (40s ago)` → `(2 min ago)`, both **disconnected** states) could never
move off its first value — and that string is the provenance the ladder cites
for its own seconds rung. `now` is the *caller's* moment: the snapshot's for a
finding drawn in that pass, a fresh read for the staleness age.

**Three more fixed in the same turn.** `age(now, event)` is two arguments of
one type, so a swapped call compiles, cannot panic, and paints *every* card
`just now` — which reads as the cluster falling over; `Finding::age(&self, now)`
removes the swap on the path that matters and collapses the render expression
D68 had recorded as something two agents write twice. The cordon test's band
accepted `[1h, 24h)` and then asserted `"2 hours ago"`, so the recapture it was
built to catch would still have failed on the phrase. And `Option`'s derived
`Ord` puts `None` **first** while `screens/alerts.md` requires ageless cards to
sort **last** in their band — nothing derives `Ord` today, which is exactly why
it needed writing down before Phase 9 reaches for the reflex.

**The wrong-field class had a name and not a single pair.** The review supplied
them, and three are reachable today from fields the snapshot already carries:
**rule 7** must read `ready.last_transition`, never `scheduled.last_transition`
three lines away — a pod up six days that went unready four minutes ago would
have read `6 days ago`, and that number is the one correlated with the deploy;
**N3** must read *that condition's* `last_transition`, not `Ready`'s off the
same flat `Vec`, or a DiskPressure card carries the node's boot time; **N6**'s
subject is the pod, so the blocking node's taint `added_at` is the wrong clock
on the right card — newly tempting precisely because D65 just certified taint
stamps. The list now sits on the field. It also corrected the weakest sentence
in that doc: rule 8 stays `None` **not** because creation time is not when the
mount became dangerous — `spec.volumes` is immutable, so it is exactly that —
but because the card describes a standing property rather than an event, and a
date beside it reads as *"something happened"*.

**`timeAdded` is the age of the taint, not of the cordon**, and N2 will print
it. It survives a kubelet restart (the kubelet writes `spec.taints` only at
registration) and a controller-manager restart (`MatchTaint` compares key and
effect, so an existing taint is never re-added), but anything that rewrites
`node.spec.taints` wholesale — `kubectl edit`, a GitOps controller reconciling
Node objects, a manifest re-apply — drops it and the controller re-stamps it,
and a taint that pre-existed the cordon is never stamped at all. So the card may
say *"cordoned about 2 hours ago"* and never build an argument on it. **The
durability claims are upstream-derived, not measured** — there is no cluster up
([§ the boxes no agent can run](CLAUDE.md)) — and the sequence that would close
it is `cordon` → read `.spec.taints` → delete the controller-manager pod → read
again → `docker restart` the node → read again → `uncordon && cordon` → read
again, the last one being the only reading that may change.

**And the line that was never true, found by the same review.** Four files said
the Alerts watches keep *"metadata + status only"* — CLAUDE.md's invariant 6
among them. Five fields the rule set reads live in `spec`: `spec.volumes`
(rule 8), `spec.terminationGracePeriodSeconds` (rule 12), `spec.unschedulable`
and its taints (N2), `spec.containers[].resources` (rule 2, N5) and
`spec.replicas` (the workload `desired` — for Deployments, StatefulSets and
ReplicaSets; a DaemonSet is the one kind that answers from
`status.desiredNumberScheduled`, and it is the only watched kind the old phrase
would have survived). A Phase 5 prune written literally from that phrase
deletes the field this box exists to fill, along with four rules. The wording is
now **"pruned to the fields the `rules.rs` snapshot types name"**, which is
single-sourced and checkable where the old one was a guess that read like a
budget. The occurrence inside D28's resolved open question is left alone: it is
a record of what was argued then, not an instruction.

**Deferred, each to a box with an owner** — the review's remaining findings are
real and none of them belongs to this file. The **day rung** hides 24h01m
through 47h59m behind `1 day ago` while `kubectl`'s own `HumanDuration` prints
`30h` and `47h` before `2d3h`, so k8rs is coarser than the command it teaches
in the band where "before or after yesterday's window" is the question; that,
the cordon card's wording, and the age column's **width budget** (widest string
14 characters, no stated maximum) are one `tui-designer` box, and it is now
Phase 3's, **before** close — `rules::age` freezes with `rules.rs` while `ui.rs`
is Phase 11, so a rung changed after that is a forward-only violation. The
**skew header line** is a Phase 5 box. And the cordon card's `kubectl describe
node` is the one command that does **not** print `timeAdded`, so N2's box owes
either a line that shows it (`-o jsonpath='{.spec.taints}'`) or a written
admission that the age is the claim `describe` cannot back — invariant 4's
teaching device pointing at the one thing it fails to teach.

### D70 — rule 8 is narrowed to `kube-system`, and every storage operator lives outside it (2026-08-13)

Rule 8 fires only on the escalated hostPath case, and even that needed a
narrowing or a fresh kind cluster paints the screen: **it stays quiet for a pod
in `kube-system` that is DaemonSet-owned or a mirror pod.** Writable hostPath is
the *normal* state of node infrastructure, and `tests/fixtures/kube-system-pods.json`
is the proof — kindnet and kube-proxy mount `cni-cfg`, `xtables-lock` and
`nri-plugin` writable across eight pods, and `etcd`, `kube-apiserver` and
`kube-controller-manager` do the same as **`Node`-owned mirror pods**, which is
why the narrowing covers `mirror` and not only DaemonSets. The `/` and
`docker.sock` escalators fire through the silence, because those are not normal
for a CNI agent either.

**The namespace is the part that does not survive contact with a real cluster.**
Every storage and networking operator worth naming installs outside
`kube-system` and mounts writable hostPaths just as legitimately: Rook/Ceph in
`rook-ceph` (`/var/lib/rook`, and the OSD pods mount raw devices), Longhorn in
`longhorn-system` (`/var/lib/longhorn`, plus the manager's `/dev` and
`/proc`), OpenEBS, Cilium wherever it was installed, and essentially every CSI
driver's node plugin, whose whole job is `/var/lib/kubelet/plugins` writable
with `Bidirectional` mount propagation. On any of those clusters k8rs as
specified prints a wall of CRITICALs on the first screen a beginner sees —
which is [invariant 13](CLAUDE.md)'s first half failing outright: someone who
runs clusters would not use this in a normal week.

**It is recorded and not fixed, because both obvious repairs are worse than the
bug.** Widening to *any* DaemonSet-owned pod in any namespace makes a careless
or hostile DaemonSet invisible, and a DaemonSet mounting the node's filesystem
is precisely the case rule 8 exists for — the rule would keep its name and lose
its reason. A **namespace allowlist** is configuration, and this project has no
config file and does not want one
([§ Out of scope](#out-of-scope-the-most-important-section)); an allowlist that
ships as a constant is the same thing with worse ergonomics, and one the user
cannot correct when their operator is in `storage`.

**What would settle it is evidence, and there is none yet.** Every fixture here
comes off a kind cluster that has no storage operator on it, so the
false-positive rate outside `kube-system` is unmeasured, and an unmeasured
number is not a design ([D25](#d25--what-this-review-did-not-decide)). Two
shapes are worth checking when a real cluster is available, because either would
decide this without configuration: whether the *owning workload* is itself
cluster-infrastructure by some readable signal, and whether the honest answer is
**severity rather than silence** — the plain read-only hostPath already belongs
to the Analysis posture rows (Phase 4), and a routine operator mount may belong
beside it rather than on Alerts. Until then rule 8 is correct on the cluster it
was tested against and known to be wrong beyond it, which is a better thing to
write down than to guess at.

### D71 — nine rules, three blockers, and the two that were decisions, not code (2026-08-13)

The first nine rules landed with 14 tests, 31 of 33 mutations red, and every
one of the fourteen printed cards re-derived from the fixture JSON by the
operator review: container names, images, the enacted limit, restart counts,
exit codes, durations, the `path`+`subPath` join, the finalizer, and each
timestamp against its own row in the `Finding::timestamp` table. **The card
arithmetic was sound. Every finding below is about a card these rules produce
on an object the fixtures do not contain** — which is the whole argument for
reviewing against a cluster operator's memory rather than against the suite.

**The escalator list was a decade out of date, and that was the worst of it.**
`RUNTIME_SOCKETS` named Docker's two spellings only. kind runs **containerd**,
as does essentially every cluster built after 2022, so
`/run/containerd/containerd.sock` fell past the escalators into the writable
branch — which is exempted for node agents — and a `kube-system` DaemonSet
mounting the container-runtime socket, the single most common cluster-takeover
shape and the exact object rule 8 exists to catch, produced **nothing**. The
rule's own doc claimed *"nothing in `kube-system` needs the runtime socket"*,
which is true; the code could not see the socket. The exemption's *shape* —
[D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)'s
`mirror || DaemonSet`, escalators asked first — was verified correct, including
that a `kube-system` DaemonSet mounting `/` writable does still fire. Its input
list was the defect. Two smaller evasions of the same compare: `hostPath: "//"`
passes upstream validation and is `/` to the kernel, and `/.` likewise, so
paths are normalised before they are matched.

**Rule 7 dated a container by another container's event.** `pod.ready` is
pod-scoped and does not move until *every* container is ready, while the rule
fires per container — so a container thirty seconds old in a pod unready for an
hour drew `1 hour ago`, which is the `Finding::timestamp` table's own rule-7
row happening one level down. It also bypassed the ten-minute grace outright:
a crash-looping container caught *between* restarts is `Running && !ready`
inside a pod whose `Ready` went False hours ago. The since-when is now floored
at the container's own run start — a container cannot have been out of the
Service longer than its current run.

**And `started` came back, as a suppressor, without reopening
[D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12).**
D51 rejected it as a **trigger** and that ruling stands: it is always true once
a container runs with no `startupProbe`, so it discriminates nothing. The
inverse is a different field. `Running && !started` is reachable *only* when a
`startupProbe` is declared and has not passed — and while it has not passed the
kubelet does not run the readiness probe at all. Without the suppressor rule 7
fires at ten minutes on every Cassandra, Elasticsearch or Vault pod whose
startup probe is `failureThreshold: 60, periodSeconds: 30`. The trigger /
suppressor distinction is written on the rule, because the next reader will
otherwise read this as D51 reversed.

**Two findings were bugs in a decision, not in the code**, which is the pair
worth remembering — the implementation matched what was written down, and what
was written down was wrong.

*Rule 12's margin double-counted the grace.* Corrected in place at
[D55](#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12):
`max(30s, grace)` on top of a `deletionTimestamp` that **already is** request +
grace hid a `terminationGracePeriodSeconds: 3600` pod for an hour past its kill
deadline. It is `> 60s`, flat.

*The exit-code table said 137 is "almost always OOM".* Written when the rule
had no `reason` beside the code; it has one now, and a liveness-probe kill that
outlives the grace lands as `137 / Error`. Rule 2 correctly stayed quiet and
rule 6 printed the memory sentence anyway, holding the field that disproves it.
Corrected in the table above — and corrected a second time on 2026-08-15, when
the row that replaced it turned out to name the *other* cause with the same
false confidence: 137 has three readings, the object names two, and where it
names neither the table says the signal and stops
([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).

**Three false-positive classes that needed no unusual manifest, only uptime.**
Rule 6 had no bound at all — `lastState.terminated` never expires, so one
transient restart six months ago is a permanent WARN, the largest volume in the
box and the thing that makes an empty Alerts screen unbelievable
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)); it is silent while
the container is serving, where its history belongs to rule 5. The `Succeeded`
skip missed `Failed`, so Evicted pods — GC'd only above 12 500 by default —
carried permanent cards for pods that will never run again, which NOTES already
routes to the **Waste** report. And rule 5 fired *beside* rule 1 on the same
container: `broken-oom` drew three CRITICALs for one incident, the third
carrying nothing the first two did not. Rule 6 already implemented that
principle for rule 2 and named it — *one event, one card*; rule 5 is the same
case one step over, and is silent under `CrashLoopBackOff`.

**Rule 5's severity now depends on whether the container is serving.** A
container up six weeks with a nightly leak-restart reaches forty and sat
permanently in the same band as `CrashLoopBackOff` while passing every probe. A
red card whose own title says it is serving is what teaches people to ignore
red. `REQUIREMENTS.md` marks the ≥3 / ≥10 numbers *(suggestion)* and they are
kept — but a raw lifetime counter carries no rate, 10 restarts in an hour and
10 in a year are one number, and the snapshot cannot supply the rate. So the
band stays and red does not reach a working container.

**Rules 3 and 4 pointed at a command that cannot show what they claim.**
kubectl's `describeStatus` prints `Reason` for a Waiting container and stops —
`state.waiting.message` is never rendered, and it is those two cards' entire
evidence. This project had already accepted `-o yaml` for rule 12 on exactly
that argument, so they take the same line. `describe` does print Limits,
Restart Count, State and Last State with its message and exit code, so rules 1,
2, 5, 6 and 8 keep it; that was checked per card, not assumed.

**`subPathExpr` was dropped, and it is the one field this box was allowed to
add** (the snapshot types do not freeze until Phase 4 close). It is the
env-expanded twin of `subPath`: a mount of `/` with `subPathExpr: $(POD_NAME)`
gets one directory, and k8rs claimed the whole node filesystem — CRITICAL,
false, the loudest possible wrong card. It cannot be resolved without reading
env values, so a non-empty `subPathExpr` means *narrowed by something we cannot
read* and drops the `/` escalator rather than asserting the root.

**The one the fixtures prove backwards, and it belongs to N1.** `healthy.json`
runs on `k8rs-worker3`, which `scripts/cluster.sh break-nodes` deliberately made
`Ready: Unknown` with the node controller's `unreachable:NoExecute` taint. The
status of a pod whose kubelet stopped posting is a fossil that never expires,
and every rule in this box reads pod status — so the fixture the negative test
leans on hardest is a pod on a node the control plane has given up on, and k8rs
calls the cluster fine. The assertion is still correct and the test message
claiming *"a working pod"* was not. **What this owes N1:** its card has to reach
the pods, not only the node, or Alerts will say "node NotReady" in one place and
nothing at all about the workload that is down.

**Recorded and not built: the pod class nothing here can see.** A pod wedged in
`ContainerCreating` — `FailedAttachVolume`, a PVC that will not bind, a volume
still attached to a dead node, a CNI that will not allocate an IP — is invisible
to all nine rules, and **also to rule 10**, because such a pod *is* scheduled.
It is a weekly failure for anyone who runs clusters. The signal is already in
every committed capture and dropped at decode:
`conditions[PodReadyToStartContainers]` (KEP-3085, GA in 1.32) is False for
exactly this state while `PodScheduled` is True. It costs one field and no new
watch, and it passes both halves of
[invariant 13](CLAUDE.md) — but it is a **new rule**, not one of the eleven, and
this project's named number-one risk is scope creep. So it is written down here
and goes to the user, not into the code. If it is taken, it is a box, and the
field has to land before the snapshot types freeze at Phase 4 close.

**Four boxes, one unit of work.** `todo.md`'s exit-code table, hostPath, and
rule-5-thresholds boxes are *specifications* of the pod-rules box, not separate
work: they cannot be built before it and are necessarily built with it. All four
are checked together, and the plan defect is recorded rather than papered over —
a box that cannot be the unit of work is a box that should have been a sentence
inside another one.

### D72 — rule 13 is added to v1, and the field it was proposed on is narrower than the case (2026-08-13)

**The user reversed the scope guard explicitly on 2026-08-13**, which is the
only way a twelfth Alerts rule gets in
([invariant 13](CLAUDE.md)). The case
[D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
recorded is real and weekly: a pod that was placed on a node and whose
containers never started — the `ContainerCreating` wedge. Nothing in the v1 set
sees it, and **rule 10 does not either**, because such a pod *is* scheduled.

**But the field the review proposed it on does not cover the failures the
review listed, and building on that sentence would have shipped a rule that is
quiet for most of its own class.** `conditions[PodReadyToStartContainers]` is
KEP-3085's renamed `PodHasNetwork`, it is written only once a pod is assigned to
a node, and it is not the trigger — it distinguishes *why*.

> **The mechanism in the paragraph that used to stand here was wrong, and it was
> mine.** It said the sandbox is built first and volume work happens after, so a
> volume wedge would read `True`. The opposite is true: `kubelet.SyncPod` calls
> `volumeManager.WaitForAttachAndMount` **before** `containerRuntime.SyncPod`
> creates the sandbox, so **every volume failure leaves the condition `False`**,
> and `True` means the mounts already succeeded. Measured on a real kind cluster
> at the fixtures' own node image, not reasoned: a pod with a missing `configMap`
> volume — this entry's own proposed capture shape — reads `False`; a pod
> mid-pull on a large image reads `True`. The code implemented the sentence
> faithfully and told a beginner whose ConfigMap did not exist to go and look at
> the CNI. Corrected below and in `rules.rs`
> ([D76](#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13)).

So the condition reads **`False` for both pre-sandbox causes — storage and
network** — and `True` when the pod has its storage and its network and the
block is later: the image still downloading, or a container that could not be
created. The name is the trap: `PodHasNetwork` is what it used to be called, and
reading the *name* rather than the *writer* is what cost this entry its
mechanism — the same mistake, in the same file, that
[D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
recorded about `Taint::time_added` a day earlier.

Two facts from the committed captures, checked rather than assumed: **every**
captured pod carries the condition and every one is `True`, so there is no
positive fixture; and `pending.json` — the unschedulable pod — carries **no such
condition at all** and no `containerStatuses`, which confirms the condition is
written only once a pod is assigned to a node.

**Rule 13, as decided.** It fires on the **residual**: the pod is assigned
(`PodScheduled: True`), no container has started, and nothing else already
explains it — not `ErrImagePull`/`ImagePullBackOff` (rule 3), not
`CreateContainerConfigError` (rule 4), not `CrashLoopBackOff` (rule 1). Its
since-when is `scheduled.last_transition` — how long since the scheduler placed
it — and it waits the same **10 minutes** rule 7 waits, borrowed from
`progressDeadlineSeconds`' default for the same reason: a container image can
legitimately take minutes to pull, and firing under that would make every
cold start of a large image an alert. **`PodReadyToStartContainers` is the
evidence line, not the gate**: `False` says the machine has not given the pod a
network yet, `True` says the sandbox is up and the block is after it — which is
almost always a volume. **WARN, not CRITICAL**, because the one thing that
still looks like this on a healthy cluster is a slow pull, and a red card that
is sometimes a slow pull is how red stops meaning broken
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)).

**Its positive side does not exist yet and the capture is not obviously
producible.** Both branches are, and cheaply — which is the *other* thing the
corrected mechanism changes. A `configMap` volume naming an object that does not
exist wedges a scheduled pod in `ContainerCreating` with the condition
**`False`**, and any image failure gives **`True`**. Neither needs CNI surgery;
the paragraph that used to stand here sent the capture trip after a cluster-wide
break to reach a branch a bad image name produces on its own.

### D73 — rule 10, and the test that argued for its own deletion (2026-08-13)

Rule 10 reads one condition and quotes the scheduler's sentence, which is the
whole value of it. The operator review found the gate correct for every shape it
could construct but one, and the two blockers are worth keeping for what they
are rather than what they cost.

**The card was false for a preempting pod, and the field that disproves it sits
in the same status object.** When preemption picks a node, kube-scheduler writes
`status.nominatedNodeName` in the *same status patch* as
`PodScheduled: False / Unschedulable`, and the pod stays there for the whole
graceful termination of its victims — 30s by default, minutes with a real grace
or a `preStop` hook, unbounded when a victim is stuck, which is rule 12's reason
to exist. So k8rs said *"no machine in the cluster will take this pod"* and sent
the reader to audit requests, labels and taints, while the API said a machine had
been chosen and was clearing space. `PodSnapshot` gains `nominated_node_name`
and rule 10 is silent when it is set.

**The second card was refused, and recorded rather than dropped.** *"A machine
has been chosen for this pod, but it is waiting for other pods there to shut
down first"* is true, useful, and something no other tool explains to a
beginner — and it is a **new rule**, so it goes to the user like
[rule 13](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)
did rather than arriving as a side effect of a bug fix. Silence is defensible
in the meantime: preemption in progress is the system working, and when it
stops working the victim carries rule 12.

**The second blocker is the one to remember: the box's most consequential
decision was held in place by nothing.** Deleting the reason half of the gate —
`if scheduled.status != "False"` — left the **entire suite green**. So the
`SchedulingGated` exclusion, which the rule's doc argues at length, was
untested; and on a Kueue, Volcano or Yunikorn cluster that one-line
simplification puts a CRITICAL *"no machine will take this pod"* on every pod
in the queue, all of them parked exactly as their author intended.

What makes it worth an entry is *why* it was missed. The dev tested the pair the
API server **cannot** produce — `status: True` with `reason: Unschedulable`,
reachable only by `patch pods/status` — and measured that hole honestly, closed
it with a planted field, and proved it red. Both shapes the API server produces
**daily** went untested, and the test's own closing message —
*"the verdict is `status`, and `reason` only says which verdict it was"* — reads
as permission to delete the untested half. **A test can argue for its own
deletion.** The rule that catches this is already in CLAUDE.md and is about
inputs, not intentions: a check is proven only for the shapes it was fed
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)),
and "shapes" means the ones the real pipeline hands it, not the ones the
argument is about.

**Severity: the flat CRITICAL was reversed into a ladder.** The dev's argument
was that `Unschedulable` is a verdict rather than a phase every healthy pod
passes through, so the window rules 7 and 13 need does not apply. The premise is
false on three routine paths: an autoscaler scale-up, where this condition **is**
the trigger Cluster Autoscaler and Karpenter watch for; `Immediate`-mode volume
provisioning, where every fresh StatefulSet replica carries *"pod has unbound
immediate PersistentVolumeClaims"* for the seconds the CSI driver takes; and
node-group rollover or spot reclaim. None needs a human, and CRITICAL in this
file means *this will not run until someone acts*. The card stays immediate —
the scheduler's sentence is the value and a beginner should not wait ten minutes
for it — and the **severity** ladders on the condition's age against
`NOT_READY_GRACE`: WARN below, CRITICAL above, CRITICAL when the stamp is
missing.

**The age is honest about less than it looks.** `LastTransitionTime` moves only
when `Status` changes, and `SchedulingGated` is also `False` — so a pod held by
Kueue for two days and released at 03:00 into a full cluster keeps the *gating*
stamp and reads "2 days ago" one second after becoming unschedulable. It is
still the best field available and there is no code change; what changed is the
wording, and the interaction with the ladder above is stated rather than left to
be discovered: such a pod reaches CRITICAL immediately.

**Recorded for the user, not built: the Pending pod nobody explains.** A pod
with **no `PodScheduled` condition at all** — kube-scheduler down or
crashlooping, or a `schedulerName` naming a scheduler that is not installed,
crashlooping, or missing RBAC — is silent in every rule in the file: rules 1–7
iterate containers and there are none, rule 8 needs a hostPath, rule 12 a
`deletionTimestamp`, rule 13 gates on `PodScheduled == True`. `kubectl get pods`
shows a wall of Pending and `k8rs --once` prints *nothing is broken*, the one
claim `screens/once.md` says has to be true. It is week one of adopting Volcano
or Kueue, which is exactly when someone reaches for a tool like this — but it is
also a **new rule**, and the residual it needs (`Pending`, no condition, older
than a grace) requires `metadata.creationTimestamp`, which `PodSnapshot` does
not carry and whose window closes at Phase 4
([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).
It is taken as **rule 14** in
[D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13).

**A finding whose fix the PM specified wrongly, and the dev proved it.** The
review found the card's `(it shows as Pending)` asserted without reading a
field; the instruction back was *"gate the parenthetical on the phase actually
being `Pending`"*, and that does not close it. `kubectl` prints **Terminating**
off `deletionTimestamp`, not off `phase` — `printPod` overrides the column
whenever a non-terminal phase carries a deletion stamp, which is why
`stuck.json` is `phase: Running` and shows as Terminating. A deleted unscheduled
pod keeps `phase: Pending`, so the literal instruction leaves the parenthetical
saying "Pending" at a reader looking at "Terminating" — the same two-words-one-pod
defect it was meant to fix. The dev implemented the correct gate, then **mutated
the code down to the brief's literal wording and watched the test fail**, which
is what a brief that under-specifies is owed.

**And rule 10 now goes silent on a deleting pod rather than merely quieter.**
Both cards were true of that pod, which is why the dev left the choice open. The
tiebreak is not truth but direction: rule 10's action sends the reader to audit
`nodeSelector`, affinity and requests, while the only thing anyone can do for a
pod on its way out is find what is holding the delete — rule 12's card, which
names the finalizer. Alerts is D2's queue of what is broken now **and
actionable**, and "unschedulable" stops being actionable the moment someone has
asked for the pod to go away. For the 60 seconds before rule 12's margin opens
such a pod draws nothing, which is correct: it is deleting normally.

### D74 — two candidate rules, one refused and one taken, decided on who actually runs this (2026-08-13)

[D73](#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13)
left two new-rule candidates for the user, who handed the ruling back on
2026-08-13: *decide it yourself, for this project.* Both are run through
[invariant 13](CLAUDE.md)'s two halves, and they come out on opposite sides —
which is the useful part, because it shows the guard discriminating rather than
waving things through.

**Refused: "a machine has been chosen, it is waiting for other pods to shut
down" (the preemption card).** The sentence is true, readable, and unexplained
by any other tool — it fails the *first* half anyway. Preemption of user
workloads needs PriorityClasses configured for it, which is a deliberate and
relatively advanced setup: common on batch and ML clusters, absent from most
others, so it is not something someone meets in a normal week. And the deciding
argument is not frequency but **what the reader does with it: nothing.** The
card is informational, the state resolves itself, and Alerts is
[D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s
work queue of things that are broken *now*. When preemption genuinely stops —
a victim held by a finalizer — rule 12 already fires, **on the victim**, which
is both the actionable object and the one the user can do something about.
Suppressing rule 10 on `nominatedNodeName` is the whole fix; silence there is
not a gap.

**Taken as rule 14: the Pending pod with no `PodScheduled` condition at all.**
It passes the first half in a way the raw frequency hides. A wedged
kube-scheduler is rare on a managed control plane and *not* rare on the clusters
this tool's audience actually runs — kind, minikube, k3s, single-control-plane
on-prem — and the other producer, a `schedulerName` naming a scheduler that is
not installed or lacks RBAC, is week one of adopting Volcano or Kueue, which is
exactly when someone reaches for a tool like this. It passes the second half
easily: *"nothing has even looked at this pod yet"* needs no glossary, and it is
precisely the diagnosis a beginner cannot reach alone — they see `Pending`, run
`kubectl describe`, find **no events at all**, and have nowhere to go.

**What decides it is the failure mode, not the frequency.** Without this rule,
every pod in the cluster is Pending and `k8rs --once` prints *nothing is
broken* — the one claim [`screens/once.md`](screens/once.md) says has to be
true. A tool whose empty screen is a lie is worse than no tool, and this is the
only known input that produces one.

**Its shape, decided here so the box does not have to invent it.** Residual, like
rule 13: `phase == Pending`, **no `PodScheduled` condition at all**, and older
than **2 minutes** measured from `metadata.creationTimestamp` — which
`PodSnapshot` must gain, and whose window closes at Phase 4 close
([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).
The two minutes are anchored rather than picked: kube-scheduler's leader
election defaults to a 15s lease with a 10s renew deadline, so leadership moves
within about fifteen seconds, and two minutes is eight times that — long enough
that no ordinary restart or failover reaches it, short enough to be useful at
3am. CRITICAL: a pod nothing has looked at is not running and will not start on
its own. The card names both causes without claiming which, because the rule
cannot tell them apart — `schedulerName` is not in the snapshot and is not being
added for this.

**One consequence is known and deliberately not solved now.** If the scheduler
really is down cluster-wide, this fires for every owner in the cluster and
buries everything else on the screen. Distinguishing "one pod's `schedulerName`
is wrong" from "the scheduler is gone" needs cross-pod reasoning that `analyze`
could do — it holds the whole snapshot — and that is a second mechanism for a
case nobody has met yet on a real cluster. Grouping by owner already collapses a
Deployment's fifty pods into one card. If the wall turns out to be real, it is a
finding from a real cluster and a later box, not a guess encoded today.

### D75 — the third role nobody asked about, and the card that never cleared (2026-08-13)

The box said *"rules 1–6 read `initContainerStatuses` too"*. `ContainerRole` is
a **three-way** — a native sidecar is an init container with
`restartPolicy: Always` and is neither of the other two
([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12))
— and both non-Regular roles live in that array. So the widening covers all
three: **a crashlooping native sidecar was producing nothing at all**, which is
the same silence the box exists to end, and nobody had noticed because the box
named the array rather than the roles inside it. Rule 7 stays `Regular`-only;
what a not-ready sidecar does to the pod's readiness is a different question.

**The role goes in the evidence's first fact, not in six titles.** One
`container_fact` means a role added later reaches every rule at once, where a
per-role title would have multiplied six strings into eighteen. And each
framing is a **property of that kind of container**, never a claim about this
pod — *"init container migrate (the app starts only after this one finishes)"* —
because rules 5 and 6 also reach an init container that finished long ago in a
pod that is now serving fine, where *"the app has not started"* would be false.

**`doing_its_job` is the shared suppressor, and it is role-aware because
"serving" is meaningless for an init container.** Running-and-ready for Regular
and Sidecar; **terminated with exit 0** for Init. Leaving the old expression
would have shipped the exact false positive rule 6's suppressor exists to
prevent, arriving through the other array: a wait-for-dependency loop that
crashes until the database answers and then exits 0 keeps its restart count and
its failed `lastState` for the pod's whole life, and drew a permanent rule 5
**CRITICAL** plus a rule 6 WARN on a pod that is working. **Rule 5 is silent
rather than downgraded** on a finished init container: its count is frozen and
can never rise again, and *"looks healthy now, but something is wrong"* is a
sentence about a container that is still running.

**And rule 2 never cleared, which the widening turned up by giving it an init
door.** A container OOMKilled once and serving ever since drew a permanent
CRITICAL — a single OOM never reaches rule 5's `>= 3`, so nothing else carried
it and nothing ever dismissed it. Same class as rule 6's unbounded WARN in
[D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13),
one band louder. **`doing_its_job` alone would have been the wrong fix:** a
container killed five minutes ago and running now is exactly what belongs on
the screen — the kernel just killed it and the next spike will do it again.
What was wrong is *permanence*, so rule 2 is silent only when the container is
doing its job **and** the kill is older than `NOT_READY_GRACE`. The card lost is
a month-old kill on a container that has been fine since, which is a
memory-limit question for Phase 4's Capacity report rather than a queue of what
is broken now.

Two things the dev decided inside that clause, both worth keeping: **an undated
kill is never suppressed** — `finished_at` is an `Option` and the exemption must
be *proved*, so `is_some_and` rather than `map_or(true, …)`, which also drops a
future-dated kill (what clock skew produces) back into the firing branch. And
the **asymmetry with rule 6 is documented at the rule**, or the next reader
finds two suppressors that disagree and "fixes" one: a non-zero exit is an
application error the restart already spent, while a kill by the kernel is a
resource fact about a container still under the same limit — it predicts the
next spike, which is what earns the higher band and why it may be dismissed for
being *old* and never for being *over*.

**A green test that was measuring the wrong thing**, found by the dev's own
second pass: its control varied the *previous* run's exit code and produced
nothing, because the suppressor keys on the **current** state — so the test
passed for a reason unrelated to what it claimed. It is the failure
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) describes arriving
inside a test that had a red run: witnessing red proves the assertion is
connected to *something*, not that it is connected to the thing named in it.

### D76 — the review that built a cluster, and the premise it measured away (2026-08-13)

Rule 13's operator review did what nothing else in this project has done yet: it
**stood up a real kind cluster** on the fixtures' own node image, planted nine
pods covering the shapes the brief named, put each through `sanitize.jq`, and
read the cards out of the unmodified `analyze()`. Every finding below is a
measurement. Three of them are blockers, and the first one was **mine**.

**The condition's mechanism was backwards, and the card was confidently wrong
in plain language.**
[D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)
said the kubelet builds the sandbox first and mounts volumes after. It does the
reverse — `WaitForAttachAndMount` runs before `containerRuntime.SyncPod` — so
every volume failure leaves `PodReadyToStartContainers: False`, and `True` means
the mounts already succeeded. The code implemented the sentence faithfully and
therefore told a beginner whose ConfigMap did not exist to go and look at the
CNI, and told a pod whose disks are provably fine that a disk was probably
missing. Corrected in D72 and in the two evidence sentences.

**What produced the error is worth more than the error.** I read the field's
*name* — `PodHasNetwork`, what KEP-3085 called it before the rename — and
inferred the mechanism from it. That is the identical mistake
[D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
recorded one day earlier about `Taint::time_added`, where the field's *doc* was
trusted over the field's *writer*. Twice now the fix has come from reading the
code that writes the field, and twice the cost was a rule that would have shipped
a confident wrong sentence. **A field is defined by what writes it.**

**The rule was silent on most production pods, and the doc argued it could not
be.** `PodInitializing` was excluded as a pointer that always has something to
point at. The kubelet's `defaultWaitingState` is `PodInitializing` for **both**
status arrays whenever a pod declares an init container — so on anything with
Istio or Linkerd injection, a migration, or a vault-agent-init, every container
reads `PodInitializing` and rule 13 could not fire on a volume wedge, a sandbox
wedge or a stuck pull. It is a pointer only when something is there to point at:
treated as stuck when no container is `Running` and none carries a reason of its
own.

**A gate that needed one container, and a title that spoke for all of them.**
One typo in a sidecar's image gave a pod `kubectl get pods` reports as `1/2`, and
a card claiming its containers had never started — so the reader debugged the
container that had been serving for three minutes. **Both halves were wrong and
both were fixed**, which the brief only asked for one of: the rule skips when any
container is running, *and* the title became NOTES' own row-13 sentence, *"given
a machine to run on, but it has not been able to start"*. The dev found the
second half — the old title is false of a pod whose **init** container completed,
and no skip covers that, because nothing there is running.

**Two containers, two failures, one card, and the count said they matched.**
`stuck.first()` is not spec order: the kubelet **sorts regular container
statuses by name**, so the named container is alphabetical. The count fact said
*"1 other container in the same state"* about a container failing for an
entirely different reason.

**And the residual was swallowing a family that has a rule already.**
`InvalidImageName`, `ErrImageNeverPull`, `ImageInspectError`,
`RegistryUnavailable` and `SignatureValidationFailed` all mean *this image will
never become available*, and the kubelet's message carries the whole diagnosis.
So `nginx:doesnotexist` drew rule 3's CRITICAL immediately with the registry's
own sentence, while `NGINX:::latest` drew **nothing for ten minutes** and then a
WARN about starting that blamed a disk. Two typos, two unrecognisably different
answers. They move to rule 3 — whose box is closed and whose phase is not, the
same in-phase correction rule 2 took in
[D75](#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13)
— and into `EXPLAINED_ELSEWHERE` in the same change, because that const's own
doc requires the pair to move together. `CreateContainerError` and
`RunContainerError` stay in the residual: open-ended causes, and the message
carries the diagnosis.

**The test locked the inversion in, and CLAUDE.md already names why.** The
positive planted `ContainerCreating` on a capture while keeping its `True` and
asserted the evidence contained *"disk"* — but that pair is a real shape, a pod
downloading an image, and *"disk"* is the wrong answer for it. Both halves
asserted what the implementation returned rather than what the requirement says
it must, which is the one thing
[§ Tests must not lie](CLAUDE.md) forbids by name. **A test written from the
code cannot falsify the code**, and a red run does not save it: witnessing red
proves the assertion is wired to something, never that the something is right.
Each half now also asserts the sentence it must **not** carry, so a future swap
has to fail.

**Three things the dev settled that the findings did not.** The image family is
**seven** reasons, not the six the send-back counted — the enumeration was taken
over the arithmetic. `UNUSABLE_IMAGE` is a single const read by rule 3 as its
trigger and by rule 13 as its exclusion, rather than five strings copied into
`EXPLAINED_ELSEWHERE`, which meets that const's *move together* requirement
structurally instead of by promise. And the bare card had been printing *"the
machine's own word for where it is stuck: PodInitializing"* — dressing the least
informative string in the status as a diagnosis; it now says the machine has not
said which step it is on.

### D77 — the comment cut, and the rationale that stays in the code (2026-08-13)

`rules.rs`' product half carried **1557 comment lines against 1180 of code**,
and the largest blocks were second copies of entries in this file — rule 10's
74 lines against [D73](#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13),
rule 13's 63 against [D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13),
rule 14's 53 against [D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13).
Two copies of one argument have no owner keeping them in step. The half is now
**1019 comment lines (46%)**, every block *what the item is plus the decision it
cites*, and the guard is mechanical: the non-comment lines are byte-identical to
the previous commit, checked by diffing both files with comments stripped.

**The ruling: ten rationales stay in `rules.rs` rather than becoming entries
here.** [CLAUDE.md § Code phase rules](CLAUDE.md) says a rationale that lives
nowhere else is kept short *and* written into NOTES in the same change. That is
read as being about **arguments**, not **constraints**. A sentence that tells the
next reader what they may not break — `mounted_path` normalising before it
compares, `last_log_line` taking the *last* non-empty line because the first is
the boot banner, `RUNTIME_SOCKETS` carrying both spellings of each socket, an
`Other(_)` needing lowercasing before Phase 7 prints it — belongs against the
line it constrains, where deleting the code deletes it too. Moved here, ten
constraints would sit one file away from the only place anyone meets them. **What
belongs here is why a choice was made; what stays there is what the code may not
stop doing.**

**The pass found a defect a comment cut created — the previous one.**
`RUNTIME_SOCKETS` says *"each socket appears under both spellings, so the rule
may not depend on which one an author typed"*, and the clause that made that
true — CRI-O's default is the `/var/run` form — arrived with the rule-8 box
(`a3bd1fc`) and left with the **first** cut (`cdc2a89`), whose own second pass
did not notice it had orphaned the sentence above the five entries that
contradict it. **A cut is an edit, and an edit can break a claim by taking away
its support** — which is why step 7 reads the result rather than the diff, and
why reading it again a commit later is not wasted. The sentence now names the
miss instead of merely being true about it.

**Recorded, not built: `/run/crio/crio.sock` matches nothing.** The list carries
one spelling of the CRI-O socket, so a mount written the other way falls through
to the writable branch — and a *read-only* one draws no card at all, which is
exactly the shape [D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
added containerd for. It is one string plus a positive and a negative test and an
operator review, and it is not smuggled into a comment-only commit. **Taken and
closed the same day in [D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13)**,
which is also where the three mutations that survived the first fix are recorded.

**Four corrections to CLAUDE.md, from reading it as its first reader.** The
ownership table gave `main.rs` to `dev-core` for *"Phases 3–7"* while the phase
map gave 8–12 to `dev-ui`, so **Phases 8–11 owned the file by nobody** —
[D34](#d34--the-temporary-mainrs-belongs-to-dev-core-until-phase-12-2026-08-12)'s
own title already said *until Phase 12* and its body said otherwise;
[docs/maps.md](docs/maps.md) had it right all along, which is what a third copy
is for. `LICENSE` appeared in no **Writes** cell under a table claiming every
path appears in exactly one. The evidence rule pointed at *"before Phase 5 wires
the binary"* when Phase 3's last box wires it. And the docs-sync rule ordered
`README.md` and `README_TR.md` updated in the same change — two files Phase 13
has not written yet.

### D78 — the socket the escalator could not see, and the three mutations that survived the fix (2026-08-13)

`RUNTIME_SOCKETS` carried `/var/run/crio/crio.sock` and not `/run/crio/crio.sock`,
though `/var/run` is a symlink to `/run` on every systemd distribution and a
manifest may write either. **The miss is not the interesting part; where it
landed is.** Rule 8 escalates on the *path* and not on the mode precisely because
a read-only bind of a runtime socket is still full root — and
[D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)
silences the writable branch for `kube-system` node agents. So a **read-only**
`/run/crio/crio.sock` on a `kube-system` DaemonSet fell through both and drew
**no card at all**. Reproduced against the pre-fix code before anything was
written: zero findings for a container holding the machine.

**The mechanism, and the one that was refused.** The constant now carries three
canonical `/run` spellings and `is_runtime_socket` folds `/var/run/…` → `/run/…`
**at the compare only**; `mounted_path` is untouched, so the card still prints
what the manifest wrote — the string the reader greps their own YAML for.
(Approximately: `mounted_path` normalises first, so Longhorn's real
`hostPath: /var/lib/longhorn/` prints without its trailing separator. A grep
still lands on the line, which is what the claim is for.) The
alternative, carrying both spellings in the list, was refused: it is one string
smaller and it leaves the next socket's correctness resting on an author
remembering to add a duplicate line, which is the defect this entry exists for.

**Settled by the dev, not by the brief:** the stored form is `/run`, so the array
no longer literally contains the `/var/run/docker.sock` string [§ v1 rule
set](#v1-rule-set) names; the fold is one-directional and `/var/run/`-only, not a
symlink resolver; `is_runtime_socket` is a named function so the constraint has
somewhere to live; the planted node agent is `kube-proxy-88dlk` because it is
DaemonSet-owned in `kube-system`, the exact shape D70 silences; and the negatives
are planted in `default` rather than `kube-system`, because there *"no socket
card"* and *"no card at all"* print the same green line.

**What the test gate found after the fix is why this entry is long.** The box
arrived with three new tests, 95 green, both reds witnessed. `tester` then ran 21
hand mutations and **three survived**:

- **`is_runtime_socket(&m.path)` in place of the joined path.**
  [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)'s
  original bug walking back in — `hostPath: /run/crio` + `subPath: crio.sock` is
  a socket only after the join. Under the mutation the read-only container's card
  disappears: the same symptom this box exists to fix, through a different door.
  No test reached the socket branch through a join, because the sweep's own
  helper strips the `subPath`. **Closed since, in
  [D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13),
  and the join has two directions** — which is worth reading before the next
  such claim. *Narrow → wide*, the raw path missing a socket the join creates,
  fell out as a side effect: `nosy`'s raw path is `/` and its joined path is
  `/run/containerd`, so the capture's own verdict test kills it. *Wide → narrow*,
  the raw path being a socket ancestor the join then leaves — `hostPath: /run`
  with `subPath: netns` — had no test at all, survived D79's own gate, and needed
  one written for it. The first was mistaken for the whole thing until `tester`
  asked which other direction existed.
- **Deleting `/run/docker.sock` from the list.** Green. The sweep *iterates* the
  constant, so it structurally cannot notice a member gone; containerd had a
  canary, crio was hard-coded in another test, Docker had nobody. CLAUDE.md's *a
  derived list asserts it found something* — found by mutation, not by reading.
- **The `.filter(|rest| rest.starts_with("/run/"))` could not be killed, and no
  test could ever kill it:** the filtered and unfiltered versions differ only for
  an entry whose `/var`-stripped remainder is in the list without starting with
  `/run/`, and no such entry exists. **Ruled: delete it and assert the invariant
  instead** — every entry starts with `/run/`. That is what the fold's
  correctness actually rests on, and what the sweep's `format!("/var{socket}")`
  already assumed in silence; it goes red the moment someone adds a non-`/run`
  entry, which is exactly when a human should be made to think. Dead code that
  cannot be proven wrong is worse than no code.

**Still unproven, and not by an oversight this time.** Nothing here has a capture
behind it, and unlike the rest of `rules.rs`'s planted shapes this one does not
wait for the capture trip: the fixtures' own cluster runs **containerd**
(`containerd://2.3.1` on all three nodes, checked live), so a CRI-O socket mount
cannot be produced there at all. The planted shape is permanent here, not
provisional.

This closes [D77](#d77--the-comment-cut-and-the-rationale-that-stays-in-the-code-2026-08-13)'s
*"Recorded, not built"* paragraph and the `todo.md` § Later bullet it created.
It did **not** close the class it belongs to — see
[D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13),
which the operator review opened before any of this was committed.

### D79 — the review that found the door beside the one D78 closed (2026-08-13)

[D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13)
taught the escalator a second **spelling** of a socket. The blocking operator
review asked the next question and it had a worse answer: `is_runtime_socket`
matched the socket **file** and nothing above it, so a container handed the
*directory* — `/run/containerd`, `/var/run`, `/run` itself — got exactly the same
node-root and fell through to the writable branch.

**What made this a blocker rather than a later box: the wrong card was already
being printed by our own committed capture.** `hostpath.json`'s `nosy` mounts
`hostPath: /` with `subPath: run/containerd`, and rule 8 answered

> `container nosy · /run/containerd on the node · writable`
> → *mount it read-only if the container only needs to read it*

while forty lines up the same rule says a read-only bind of that socket is full
control. Two cards, opposite advice, one capability — and the beginner at 3am
gets the one that changes nothing. Reproduced, printed, and read before the fix.

**The escalator now matches the socket or any directory above it.** The obvious
implementation is a trap and was rejected in the brief rather than in review:
`"/var".strip_prefix("/var")` is `Some("")`, so a `format!("{p}/")` prefix test
becomes `"/"` and *every* socket matches — a pod mounting `/var` would draw a
CRITICAL socket card. The shipped form strips in reverse (`socket.strip_prefix(path)`,
remainder empty or starting with `/`) behind a non-empty guard, and allocates
nothing.

**The guard's justification was wrong on the first telling, which is worth
keeping visible.** This entry claimed `path: ""` reaches the function off the
wire because `host_path_mounts` copies `hostPath.path` verbatim. It does not —
the operator review put it to a live apiserver and `validateHostPathVolumeSource`
answers `spec.volumes[0].hostPath.path: Required value`. The guard is still
necessary and unchanged, for the reason beside it: **`/var` folds to `""`**, and
`/var` does arrive off the wire. A load-bearing reason sitting next to a false
one is how the false one survives a review, so the false one is cut rather than
demoted.

**Two tests were reversed, deliberately, and that is why they are written here.**
`a_path_beside_a_runtime_socket_is_not_a_runtime_socket` asserted `/run/crio` is
*not* a socket — it is the socket's directory, so it now is one, and the
beside-but-not-under case survives as `/run/crio.sock.bak` and `/run/criox`.
`the_two_escalated_host_mounts_both_fire_and_the_ordinary_one_does_not` expected
the writable card for `nosy` and now expects the socket card. The capture itself
was not touched ([D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12));
only the expectation moved, and it moved because it was asserting the defect.

**The list was right for 2022 and shipped blind to the distro the audience
runs.** `/run/k3s/containerd/containerd.sock` — k3s *and* RKE2, which embeds
k3s's containerd — is what a homelab, an edge fleet and half of all first real
clusters run. `/run/cri-dockerd.sock` is in `crictl`'s own default probe list and
is every node that kept Docker past 1.24, minikube included. Both added; both
under `/run`, so the fold invariant holds. **The list is not complete and the
comment now says so**: a kubelet `--container-runtime-endpoint` can put
containerd anywhere, and no path list closes that.

**Refused, with the reason, so the next reader does not re-open them:**
`/run/nri/nri.sock` grants the same power but kindnet mounts `/var/run/nri`
writable on every kind cluster — with an ancestor match, adding it lights a
CRITICAL on a healthy cluster, and the whole-capture test is the live proof.
microk8s's `/var/snap/microk8s/common/run/containerd.sock` is not under `/run`
and cannot be added without breaking the fold invariant D78 rests on. Podman,
`.ttrpc` and `device-plugins/kubelet.sock` are different escalation classes and
are `todo.md` § Later.

**The card's advice was true and still harmful.** *"remove the mount — a
read-only bind of this socket is still full control"* is unconditional, and a
real cluster has legitimate holders in `kube-system`: an nvidia container-toolkit
installer, a Falco or Datadog node agent. A newcomer who obeys it breaks GPU
scheduling or their own security agent, at 3am, on the most severe card on the
screen. Title and severity are unchanged — it *is* root on that machine — and
the action now carries both halves: remove it unless **managing or watching**
the node's containers is this pod's job, and if it is, it already has full
control of every node it runs on. This is [D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)'s
open *"severity rather than silence"* question surfacing on the escalator side.

**"Or watch" is there because the first rewrite said only "manage", and missed
two of the three holders it was written for.** The installer manages containers;
Falco and Datadog *watch* them. The second operator review pulled Google's own
cAdvisor DaemonSet, which mounts `hostPath: /var/run` with `readOnly: true` — so
it draws this card, since the socket branch runs before the mode check — and
pointed out that a newcomer looking at `falco-8x2qk` reads "unless this pod's job
is to manage the containers", decides monitoring is not managing, and removes the
mount. One verb short of the exact failure the sentence exists to prevent. **The
lesson is not about the word.** An exception written for a list of examples has to
be checked back against that list, and this one was not until a reviewer did it.

**The false-positive question the widening actually raises was answered on a
cluster, not in argument.** Adding sockets is cheap; making `/run` and `/var/run`
escalate is not, and a wall of CRITICALs on k3s would have been worse than the
miss it closes. So a stock `rancher/k3s:v1.31.5-k3s1` was stood up: coredns,
both helm-install jobs, local-path-provisioner, metrics-server, svclb, traefik —
**zero hostPath volumes cluster-wide**, nothing above `/run/k3s/containerd`. RKE2
was read at the source instead (`pkg/rke2/rke2.go` hard-codes the same socket
path; its static pods mount only ssl dirs and `$DataDir`), and every hostPath in
canal, Calico 3.29.1, Cilium 1.16.5, Longhorn 1.7.2, Rook 1.15.6 and KubeVirt
1.4.0 was run through a verbatim copy of the shipped predicate: no socket verdict
anywhere. **The new entries add almost no surface** — `/run/cri-dockerd.sock`
sits under `/run`, which `/run/docker.sock` already made an ancestor, and nothing
on any distro mounts `/run/k3s`. The whole of the risk in this change is `/run`
and `/var/run` as ancestors, which is where cAdvisor lands.

**Settled by the dev, not by the brief:** the guard is `!path.is_empty()` rather
than a `starts_with("/run")` test, because the latter restates the constant's
invariant in a second place — the thing D78 deleted; the canary table is checked
**both** directions, so a sixth entry added without a canary goes red instead of
relying on the next author's memory; and the action got its own assertions after
the first draft's `!contains("mount it read-only")` stayed green under a revert
to the old string — the whole of this fix would have been undoable in silence.

**Four comments in this one rule stated something false, and that is the finding
rather than any one of them.** The `/var/run`-default clause a cut removed and
left its conclusion behind
([D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13));
`path: ""` above; *"a relative path cannot come off the API — `hostPath.path`
must be absolute"*, which `--dry-run=server` accepts (upstream rejects `..` and
nothing else, and `run/crio` resolves against the pod's bundle directory on the
node, so matching no escalator stays the safe direction); and a claim that the
emptiness guard is what excludes `/`, when the below-check does it. **Every one
of them was a sentence about what the API can hand us, and every one was true of
the author's mental model rather than of an apiserver.** They survive review
because they read as background rather than as claims. The habit that catches
them is the one that caught all four here: put the sentence to a live
`--dry-run=server` instead of to a reviewer.

**One caveat the review proved live and no code closes.** `kubectl describe pod`
prints the host path and the mode, so the card is backed by the command it hands
you — but when a `subPath` is involved the evidence string is a **join** the
reader performs across the `Mounts:` and `Volumes:` sections; it never appears as
one string. And `subPathExpr` is not printed by describe at all, so for that
shape the path is unfindable in the command offered. Pre-existing, same class as
the N2 `timeAdded` admission, recorded rather than fixed.

### D80 — the tests moved out of `rules.rs`, and D50's ruling did not move with them (2026-08-13)

`src/rules.rs` was 8913 lines, of which 6584 were `#[cfg(test)] mod tests` — the
product file was a quarter of its own file. The user asked for the split and then
asked for it as a **convention**, so it is written as one:
[invariant 11](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
now says every product file with tests carries the single declaration
`#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;` and no test code of its own.
Not a per-file judgement call — `analysis.rs` will split the same way in Phase 4
without anyone deciding again.

**`#[path]`, not a module directory, and above all not a lib target.**
[D50](#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12)'s
*title* stopped being true the moment the file moved; its *ruling* is untouched
and is the reason this shape was chosen. `#[path]` keeps `tests` a child module
of the bin crate: it still sees the private items, `use super::*;` still resolves,
the test paths are still `rules::tests::*` — which is why the 97-name test list
is byte-identical before and after. The thing D50 refused was `src/lib.rs`, a
ninth file of pure plumbing added so that tests could live in a directory. That
is still refused. Nothing under `tests/` can reach a product type, and nothing
here changed that; the tests did not leave the crate, they left the *file*.

**The brief contradicted itself and `dev-core` said so instead of picking
quietly.** It demanded a pure relocation — "not one test reformatted" — and
`cargo fmt --check` clean. Both cannot hold: dedenting by four gives thirteen
sites four more columns and rustfmt wants them unwrapped. It resolved toward fmt,
proved the pure move *first* against the pre-fmt file with an exact-four-space
diff — empty — and reported the reflow separately: seven sites of words re-joined
across a line break, two of them dropping a comma the join made redundant, no
identifier, literal or assertion text moved. That ordering is the point. Run the
same dedent diff against the *landed* file and it is not empty; every hunk in it
is a rustfmt line-join, which is readable only because the move was already
proven clean underneath. Had fmt run first, no later diff could have told a moved
test from an edited one.

**The guard that broke is the finding.** `scripts/certs-test.sh` reads the pinned
instant out of the Rust side with a `sed` range — and `fn now()` is a *test
helper*, so the file it was reaching into was the one that moved. It needed two
edits, not one: the path, and the range terminator `/^    }/` → `/^}/`, because a
dedented `fn now()` closes at column 0. The second is the one that would have been
missed: with only the path fixed the guard still yields the right instant today,
by accident, because no other `time("…")` happens to sit before the next
four-space `}`. **A guard that reads another file's interior by line shape is
coupled to that file's indentation, and the coupling is invisible until the
layout moves.** It failed loudly rather than silently only because of the
existing `-z` "found nothing" branch — the same
[derived-list rule](CLAUDE.md#code-phase-rules) that put it there.
`scripts/test-guard.py` needed nothing: it `rglob`s `src/`, so it picked the new
file up and still reported 97 declared, 97 listed.

**A second drift fell out of the same look, older and unrelated to the move.**
`docs/maps.md` said `certs-test.sh` asserts "24 / 365 / −3 days at the pinned
`now`" — those are the certificates' *lengths*, and `now` sat one day into each,
so the guard asserted 23 / 364 / −4 days **left** — and it moved again with
every capture after, the row going stale a *second* time before the numbers
were taken out of it altogether
([D57](#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)).
Wrong since the row was
written, and it survived because both numbers are plausible and neither is
compared to anything. Fixed to say both, since the pair is what makes it
checkable.

**The operator review was skipped, and this is the PM saying so in writing.**
[The cycle](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it) step 6 is
blocking for `rules.rs`, but a relocation has no operator surface: no rule
changed, no card text changed, no kubectl line changed, and the mechanical
proof — an empty diff on the 97 test *names*, and the dedent diff above — is
stronger evidence than a read of 6584 lines would have been. What did get
attacked is the thing a review would not have caught anyway: **a relocated test
that can no longer fail.** `tester` reverted two implementations and watched the
tests go red from their new home — `is_runtime_socket` → `false` reddens five,
`out_of_memory` → `None` reddens three including the whole-capture test — with
the panic locations naming `src/rules_tests.rs`, and `src/rules.rs` restored to
the same sha256 afterwards. The failure mode of a move is not a wrong rule.

### D81 — the node rules, and the four things a real cluster said about them (2026-08-13)

N1–N6 landed together. Four decisions in this entry reverse something this repo
already said in writing, and three of the four were caught by an operator review
that went to a live cluster and to upstream source instead of reasoning from the
documents.

**The gate order is the entry's actual subject, because each gate caught what the
one before it could not.** The author's own pass produced six rules and 21 tests
that were green. `k8s-admin` — reading as an operator, against a live cluster —
found eight, one of them a blocker. The author fixed all eight, witnessed red and
green on each, and was green again. `tester` then reproduced those eight
independently and found **six more**, including that the blocker was still live on
the other branch of the same `if`. Nobody was careless at any step; every finding
was invisible from where the previous reader was standing. A green build after a
fix is not evidence the fix was complete
([D26](#d26--a-green-build-that-proves-nothing-2026-08-12)), and this box is the
sharpest example the project has produced.

**N6 is not a card. It is rule 10's second half.** `no_node_accepted_it` already
fires on N6's exact population — `PodScheduled=False`, `reason=Unschedulable` —
and its own comment, written phases earlier, says the node half "is N6's".
Shipping both would have put two cards on one screen for one pod, which
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) calls the
thing that stops the list being believable. So N6 supplies the first half of rule
10's evidence and its action when the node join can name a cause, and rule 10
keeps the strings it had when it cannot.
[D37](#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
survives intact: the scheduler's sentence stays on the card, `·`-joined after
N6's, because it is the only place the *other* refusals appear. The cost is
geometry — that card is now twelve lines against `widgets.md`'s three-to-five —
and it is recorded on the open card-geometry box rather than paid for by cutting
a sentence the reader needs.

**N6 was telling people to defeat Kubernetes' own safety mechanisms, and the
policy that closes it is a general one.** The first implementation named any
`NoSchedule`/`NoExecute` taint every candidate node carried. On a single-node
cluster — kind, minikube, k3s, Docker Desktop, which is most of this tool's
audience — `kubectl cordon` followed by a deploy produced *"add a toleration for
`node.kubernetes.io/unschedulable`, or remove the taint"*. The answer is
`kubectl uncordon`. With `node.kubernetes.io/unreachable` it was worse: the card
told the reader to schedule onto a dead machine, "remove the taint" is impossible
because the node controller re-adds it within seconds, and **N1 was drawing "This
node has stopped responding" three cards up the same screen**. The sharpest
instance is `ToBeDeletedByClusterAutoscaler`, which N2 is deliberately silent on
as an operation in progress
([D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12))
while rule 10 offered to tolerate it — two rules in one file disagreeing about
one taint. **The ruling: never tell the reader to tolerate a taint the node
controller manages.** Those families are translated, not printed raw, which is
[invariant 14](CLAUDE.md) as well — `node.kubernetes.io/unschedulable` bare is
the `CrashLoopBackOff`-printed-and-left case. `node-role.kubernetes.io/control-plane`
is the one where "add a toleration" is genuinely right, so this is a translation
table and not a suppression list.

**The kubelet skew threshold was ours, was wrong, and the test locked it in.**
This file said two minors. Upstream's version-skew policy says a kubelet may be
**three** minors older than `kube-apiserver`, and has since 1.28 — the two-minor
limit applies only to kubelet < 1.25. The card's title claims a node is *"too far
behind the control plane to be supported"*, which is a claim about upstream's
policy rather than about this project's taste, and it was false across a whole
minor version: every cluster mid-upgrade would have had supported nodes listed as
unsupported by the Versions report, whose entire product is that list. The test
planted `v1.33` under `v1.36`, asserted it was unsupported, and pinned the
constant against this document — so the suite was holding the wrong number in
place. `SUPPORTED_SKEW` is 3, and the N-series table above now carries upstream's
number with upstream named.

**N2's kubectl line reversed twice, and the second reversal overturned the PM's
own ruling.**
[D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)
offered two ways to handle `describe node` not printing `timeAdded`: print a
command that *can* show it, or record that the age is the one claim `describe`
cannot back. The PM picked the first and put `-o jsonpath='{.spec.taints}'` in
the brief. Run against a live node, that prints the taint array and nothing else:
it backs only the **age**, the optional half of the card; it backs *nothing the
card claims* when `added_at` is `None`; and it hands a beginner raw JSON.
`describe node` prints `Unschedulable: true` and the `Non-terminated Pods`
table — the title and the count, and the count *is* the trigger D43 narrowed this
rule to. So the command is `describe node`, and the screen states that the age is
the single claim no command behind this card can show. `screens/` keeps the wrong
turn visible rather than replacing it with the answer, because the next person
will reach for jsonpath for the same reason.

**The blocker looked like arithmetic. It was a missing test, and believing the
first diagnosis is what let it survive being fixed.** N5 compared CPU as `f64`:
`quantity_value` turns `"100m"` into `100.0 * 1e-3`, millicores are not
representable in binary, and the comparison carried no tolerance. An exactly-full
node — legal and ordinary, since `noderesources.Fit` admits while
`request <= allocatable - requested` — summed to `0.30000000000000004 > 0.3` and
drew a card whose two numbers printed *identically*. Across realistic
allocatables packed exactly full, 19% fired, and the result was sum-order
dependent, so a node at the line would flap as watch events reordered the pod
list. The fix moved the whole rule to integer millicores.

**And the bug survived it.** The boundary test written alongside the fix planted
`allocatable.cpu` only, so mutating the *memory* comparison to `>=` left all 121
tests green — the same defect, the same card printing two identical numbers, on
the branch nobody had stood a test on. Integers did not save the memory branch,
because integers were never what was wrong: **nothing in the suite was standing
on the line.** Every committed fixture was comfortably over or comfortably under,
which is how this survived a review that reproduced it, a fix, and a passing
build. The lesson `f64` was hiding: **a boundary is a place, not a data type**,
and a fix aimed at the mechanism rather than at the untested place leaves the
other half of the same `if` exactly as it was. The rule that follows from it —
assert the boundary from both sides, on every branch that has one — is worth more
than the arithmetic story it replaced.

Two more came out of the same pass, both of them things a reader would have
called impossible. **A pure rule panicked**, on `(numerator + denominator - 1)`
in the new integer parse: every multiplication around it was `checked_`, that
addition was not, and `170141183460469231731687303715884105m` reached it through
an ordinarily decoded pod. Not hypothetical — put to a live apiserver with
`--dry-run=server`, the string is **accepted and stored verbatim**. In release it
does not even panic; it wraps to a negative millicore count and feeds the
comparison. That is [invariant 5](CLAUDE.md) broken by the fix for the blocker.
And `quantity_milli` refused the exponent form on a doc comment claiming the API
server canonicalises `1e3` to `1k` before it can arrive — true unquoted, false
quoted, because `Quantity` caches its original string and Helm charts quote their
quantities routinely. The measured cost on the project's own capture was one
whole node silently absent from the Capacity report. `[eE][+-]?[0-9]+` is in
upstream's Quantity grammar and `ParseQuantity` accepts it, so this was another
sentence about what the API can hand us that was true of its author's model
rather than of an apiserver — the [D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)
pattern, third occurrence, and the reason the habit that catches it is putting
the sentence to a server rather than to a reviewer.

**Four ceilings on the taint table, each one a thing the next reader would
otherwise re-derive wrongly.** They are in the code's doc comment too, but they
belong here or the comment is arguing with nobody.

- **No row may promise another card.** The first draft's `not-ready` and
  `unreachable` actions said *"there is a card for it on this screen"*. N1 waits
  `NODE_DOWN_GRACE` — five minutes — before it draws anything, and the taints do
  not wait at all: `nodelifecycle`'s `doNoScheduleTaintingPass` runs off the node
  informer, so the taint lands a fraction of a second after `Ready` flips. The
  300 seconds everyone reaches for belongs to the **NoExecute** taint — eviction,
  not scheduling. A runtime dying at 03:02 and a deploy at 03:03 sent the reader
  hunting a card that arrives at 03:07; a node with no `Ready` condition at all
  never gets one. **Aligning N1's grace to the taint was the wrong repair and was
  refused**: that number is borrowed from
  `--default-unreachable-toleration-seconds` and is not to be tuned to make a
  sentence true. The rows point at the machine and stop.
- **`node-role.kubernetes.io/control-plane` can never join the table, for a
  structural reason rather than a judgement about that taint.** Every row here is
  one whose removal is impossible (the controller re-adds it) or pointless (it
  clears itself). The control-plane taint is neither — nothing changes on its
  own — so *"wait"* or *"check the machine"* would strand the reader, while both
  halves of the untranslated wording are the real answers: the documented
  single-node kubeadm fix is literally
  `kubectl taint nodes --all node-role.kubernetes.io/control-plane-`.
- **`network-unavailable` names the network plugin and that is deliberately the
  only answer it names.** The other producer of `NodeNetworkUnavailable=True` is
  the cloud **route controller**, waiting on routes to the node's pod CIDR — a
  control-plane problem, not something on that machine. Route-based networking is
  legacy now, and cloud jargon does not belong on a card a kind user can see, so
  the common producer wins the sentence.
- **`memory-pressure` survives a trap that would have inverted its advice.** The
  `PodTolerationRestriction` admission plugin adds an `Exists` toleration for it
  to every non-BestEffort pod, which would have made *"Kubernetes stops placing
  new pods"* true only of BestEffort ones and *"free up memory"* the wrong advice
  against *"give this pod a memory request"*. The plugin is **not** default-enabled
  in 1.36, so the sentence holds on a default cluster; where it is enabled,
  `tolerated()` matches the auto-toleration and the branch is never reached. Safe
  in both directions — which is only knowable by checking, and was checked.

**One ceiling on the raw-key guard, recorded so nobody strips the wrong thing.**
The tests assert no managed taint key reaches the screen. A key can still arrive
inside the scheduler's own message, which
[D37](#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
requires carried verbatim. The pinned v1.36.1 scheduler summarises — `1 node(s)
had untolerated taint(s)`, no key — so the assertion is safe today. If a future
scheduler names keys again the test reddens, and the fix is to narrow the
assertion to k8rs's own half of the evidence, **never** to edit the quote.

**What the review confirmed is worth as much as what it caught.** N5's sum was
computed against `kubectl describe node`'s *Allocated resources* on three live
nodes and matched on all six numbers — the check that matters, because an
operator will run exactly that command and a disagreement costs the card its
credibility. `tolerated()` is upstream's `Toleration.ToleratesTaint` field for
field. All three of D69's timestamp traps are avoided. And the cordon taint
carries `timeAdded` in the committed capture's real API bytes, so
[D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)'s
claim is evidence rather than inference.

**Three rules ship with no captured positive**, planted on decoded copies
([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)):
N1's `Ready: False` branch (`break-nodes` stops a kubelet, which yields
`Unknown`), N3 entirely, N4 entirely. Each carries a capture-trip note. The one
that matters is N3 — its mutation does **not** redden the whole-capture test,
because no captured node is under pressure, so the planted test is its only
proof.

### D82 — the W-series, and the card that would have taught people to mute the tool (2026-08-14)

W1 and W2 landed together. **The entry's real subject is the gate order**, because
this box went through the cycle three times and each pass found something the
one before it could not — including a defect that existed *only because* of the
previous pass's fix.

| pass | outcome |
|---|---|
| author's own, green | 7 tests, `just check` green, both rules firing on the committed capture |
| `k8s-admin`, first | **3 blockers**, 3 should-fixes, 6 nits — most reproduced against a live cluster |
| rework, green again | all 12 taken, red-and-green witnessed per fix |
| `k8s-admin`, second | **0 blockers**, 6 should-fixes — one of them created by the rework itself |
| rework, green again | all 8 taken, 7 of 8 witnessed red |

Nothing was negotiated down to get past a gate, and the box was not checked off
until the third pass. What follows is what those passes settled.

#### The three blockers, because each is a rule about rules

**1. W1 paged CRITICAL for a service that was 100% up.** The severity band read
`readyReplicas` off the **ReplicaSet** the finding is about. A refused rollout's
*new* ReplicaSet always reads `0 of N` while the old one carries every request —
so every quota-refused rollout drew a red card for a service that never went
down, with no pods under that ReplicaSet for anything on the screen to
contradict. Reproduced live: a `pods: 2` quota against a 2-replica Deployment,
both pods `Running 1/1`, `Available: True`, and k8rs saying CRITICAL. **That is
the card that teaches a user to mute the tool in week one**, and after that none
of the other findings matter. The band now reads the **owning Deployment**.

**2. W2 was silent on the most common failed rollout there is.** The shortfall
gate was `ready < desired`. RollingUpdate defaults `maxUnavailable` to 25%
*rounded down* — **0** at one, two and three replicas — so the old pods are never
removed, `readyReplicas` stays equal to `spec.replicas`, and the gate is false on
a Deployment whose own condition says it gave up. The repo's own
`tests/fixtures/deployments.json` has that shape and only fails to draw because
`broken.yaml` set `progressDeadlineSeconds: 3600`.

**3. `ReplicaFailure` is not only about creating pods.** Upstream writes two
reasons — `FailedCreate` and **`FailedDelete`**, a scale-*down* the API refused.
Reproduced live with a webhook denying `DELETE pods`: W1 drew *"Kubernetes
refused to create the pods…"* over the evidence line **"2 of 1 pod ready"**, a
number on a screen whose whole promise is that its numbers can be believed. W1
now filters `reason == FailedCreate`. **`FailedDelete` gets no card in v1** — a
PM ruling, recorded here so the next reader knows it is a decision and not an
oversight: the service is up, a third card is a new rule, and
[invariant 13](CLAUDE.md)'s guard applies. Gatekeeper and Kyverno delete
constraints make it reachable, so it is a known gap and a v0.2 candidate.

#### The shortfall has three arms, and each is the only one that sees a shape

`short_of_pods` is `ready < desired || updated < desired || unavailable > 0`.

- `unavailable > 0` is the only arm that sees the **n=1** rollout above.
- `updated < desired` is what a rollout is actually short of, and it doubles as
  the evidence line `kubectl rollout status` leads with.
- `ready < desired` is **redundant on a Deployment** — `unavailable == 0` implies
  `ready >= desired` there — and is *not* redundant on a ReplicaSet or
  StatefulSet, which carry no unavailable counter and whose `updated` counts pods
  that exist rather than pods that work. `broken-owned-7bdb7645c8` is the proof:
  one crash-looping pod, `updated == desired`, short only through readiness. The
  author derived this in their own second pass, after first justifying the arm
  with a Recreate transient that does not need it.

**A scaled-to-zero Deployment is silent — but only because the arms are gated on
`desired > 0`, and this entry said otherwise for an hour.** The first version of
this paragraph argued the arms take care of it: `spec.replicas` is a pointer, so
`omitempty` does not hide its zero
([D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)),
and nothing can be below zero. That is true of arms 1 and 2 and **false of arm
3**, which `tui-designer` caught while drawing the counter table off this very
paragraph. `unavailableReplicas` is `sum(replicaset.spec.replicas) −
availableReplicas`, floored at zero — it is **never** read off the Deployment's
own `spec.replicas`, so the two have different authors and no shared instant. In
the window where the pod is no longer available and not yet gone,
`spec.replicas: 0` and `unavailableReplicas: 1` coexist. Add the sticky
`ProgressDeadlineExceeded` — and `kubectl scale --replicas=0` is *how a person
stops a broken rollout*, so it is reliably there — and W2 drew **CRITICAL
`1 pod not answering` about a Deployment the user had just deliberately turned
off**. Seconds long, and red. The gate is written in front of all three arms
rather than inside the one that needs it, because that is where the rule is
stated once: a workload that wants no pods is not short of them.

The same false guarantee was written in two more places — `WorkloadSnapshot::unavailable`'s
own field doc, and this paragraph. **A sentence that is wrong is worth finding
twice**: the fix that only closes the site named in the report leaves the
written promise standing two doc comments away, which is where the next reader
looks.

#### Three lookups that must fail towards *unknown*, not towards *down*

The rework closed blocker 2 with a lookup that fell back to the refused object
when the owner was absent — and **that fallback can only ever produce a wrong
CRITICAL**, because a refused ReplicaSet's `readyReplicas` is 0 *by definition of
having been refused*. It is blocker 1 arriving through a different door, and the
second review caught it. Argo Rollouts is the shape that reaches it: a `Rollout`
CR owns ReplicaSets directly with no Deployment between, and
[invariant 12](CLAUDE.md) forbids decoding a CR. A 403 on `deployments` with
`replicasets` readable is the second path.

So all three lookups now answer the same way:

- **`the_workload_that_serves` returns `Option`.** No owner resolved → no severity
  band and **no counter** — the card prints the controller's quote alone. That is
  *"no number we cannot produce"* applied to a count instead of an age.
- **`workload_owner` hops only off a ReplicaSet.** The chain is exactly
  Pod → ReplicaSet → Deployment; an unconditional hop walked one step too far
  from W1's own finding, whose owner is already the Deployment, and landed on the
  operator CR above it — two CRITICALs for one refusal, the second headed by the
  CR while its `$ kubectl` line named the Deployment.
- **W2 fails open.** An owner it cannot resolve is unknown, so the card draws.
  Reading unknown as *related* would let one crash loop anywhere in the snapshot
  silence every W2 in it, and silence is the failure the W-series exists to end.

#### The suppression is per-shortfall, not per-owner

[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) says W2
stands down when a pod-level finding **explains** the shortfall. The first
implementation read "when any finding exists under this owner", which let rule
5's *"it is serving now, but something keeps killing it"* — a card whose own
sentence says the pod is fine — silence a CRITICAL saying the rollout is dead.

**The discriminator is the pod's own `Ready` condition, not `doing_its_job`.**
That was the author's call and it is right: `doing_its_job` is per-container and
`.all()` over an empty container list is vacuously **true**, so an unscheduled
pod would read as *serving* — which is rules 10 and 14's exact shape and the most
common true explanation of a stalled rollout. `Ready == True` is the same
arithmetic `readyReplicas` is counted in, so a pod that fails it is by definition
part of the shortfall. A pod with **no** `Ready` condition at all counts as
explaining, and that third framing is fed a real object rather than argued from
the source ([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).

W1 suppresses W2 too. On the committed capture the quota refusal and the timeout
it caused are sixty seconds apart on one owner, and the screen shows **one card**
— the same fold N6 got into rule 10
([D81](#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)).

#### Two things the cards may not do

**A counter may not contradict the dot beside it.** W2 first chose its counter by
the *update* arithmetic, so a CRITICAL could print `0 of 1 pod on the new version`
on a first-revision Deployment — red saying *down*, the words saying *an old
version is still up*, which are the two opposite triage decisions at 3am on one
line. The counter is chosen by the readiness arithmetic first, so *"on the new
version"* is only ever printed when an old version demonstrably is serving.

**An action may not name a command the object in front of it cannot run.**
W2 offered `kubectl rollout undo`, which errors on a single-revision Deployment —
which is the shipped fixture — and on a paused one. [Invariant 4](CLAUDE.md)
governs `kubectl_cmd`; this entry extends the same honesty to the action line.
Both W cards use `kubectl get … -o yaml` rather than `describe`, because
`describeReplicaSet` and `describeDeployment` reduce conditions to a
Type/Status/Reason table and drop the message the whole card is made of.

### D83 — the hours rung runs to 48, and the age ladder gets one home (2026-08-14)

[D68](#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13)
settled that the age ladder's rungs are not the formatter's choice but the
strings the screens already print. It left one rung wrong, and the cordon-card
round measured it.

**`1 day ago` covered 24h01m through 47h59m** — a whole day of resolution thrown
away in the one band where the reader's question is *"was this before or after
yesterday's change window?"*. `kubectl`'s own `HumanDuration` prints `30h`, then
`47h`, then `2d3h`, so k8rs was **coarser than the command it exists to teach**,
in the band that matters most. The hours rung now runs to 48. Past it the
question stops being *which* window and one unit is enough, so the days rung
stays coarse on purpose — nothing here invites `2 days 3 hours ago` later.

**`1 day ago` is therefore not a reachable string**, and neither is `0s ago`.
Both absences are deliberate; a screen drawing either is drawing something the
ladder cannot produce.

**The ladder now has exactly one home** —
[`screens/widgets.md` § 1b](screens/widgets.md) — because it was derived from
three screens and lived in none of them. `rules::age`'s doc table cites that
section rather than re-arguing the rungs, which is the shape CLAUDE.md asks a
doc comment to have.

**`lasted`'s identical `as_hours() < 24` was deliberately left alone**, and this
sentence exists so nobody closes the gap with a find-and-replace. It formats a
*span*, not a moment: `1 day` reads naturally there, kubectl renders no
equivalent, and the two functions only ever shared `counted`.

**The round also fixed the age column's budget, which had never been stated**:
14 columns, from `20678 days ago` — the epoch string, i.e. the case
`Option<Time>` exists to keep off the screen. Nothing is clamped at 14; a wider
string takes one more column from the name beside it.

### D84 — a memory-starved capture host silently turns `OOMKilled` into `Error` (2026-08-14)

Found while pre-warming the capture trip's cluster. Nothing was failing;
`just check` was green and had been for days. **This entry was written once with
the wrong cause and is recorded here with both** — the mistake is the more
useful half.

**What happened.** The 4-node cluster was stood up on the LAN host and
`cluster.sh verify` came back **22 of 23** — `oom` failed, printing
`exitCode: 137, reason: "Error"`. The committed `tests/fixtures/oom.json` says
`reason: "OOMKilled"`, and rule 2's entire discrimination rests on that word
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13):
*137 with `OOMKilled` is memory, 137 without it is a container that did not stop
when asked*). A re-capture there would have replaced rule 2's positive fixture
with an object proving the opposite rule.

**Four things it is not.** Not a per-restart race — five consecutive restarts,
all `Error`. Not the manifest — four further shapes, all `Error`: `stress` with
a five-second sleep before allocating so the shim's watcher is armed, a gentle
70 M overshoot instead of 250 M, and `dd bs=100M` as PID 1. Not the kernel
failing to notice — the pod cgroup's own `memory.events` read
`oom 8 · oom_kill 24 · oom_group_kill 8`, and `dmesg` named the right
constraint, `CONSTRAINT_MEMCG`. Not `memory.oom.group`, which was 0 on the
container scopes, was set to **1** by hand during a deliberate 40-second sleep,
and changed nothing.

**The wrong conclusion, and how it was caught.** One command run on both
machines — docker 29.7.2, cgroup v2, `kindest/node:v1.36.1` identical on both —
gave `OOMKilled=true` on the dev machine (kernel 7.1.6-cachyos) and
`OOMKilled=false` on the LAN host (kernel 6.8.0-137-generic), and this entry
first said **the host kernel decides**. It does not. The two hosts differed in
*two* ways at once — kernel version **and** free memory — and the conclusion
changed one while observing the other, which is the plainest confounded
variable there is. Repeating it against the **idle** LAN host, after its cluster
was torn down, gives the opposite answer:

| host | kernel | free | `tail /dev/zero` | `dd … bs=100M` |
|---|---|---|---|---|
| dev machine, cluster running | 7.1.6 | 15 GiB of 23 | `true` ×5 | `true` ×5 |
| LAN host, **cluster running** | 6.8.0 | ~0.2 GiB of 3.8 | — | **`false`**, and `Error` on every in-cluster shape |
| LAN host, **idle** | 6.8.0 | 3 GiB of 3.8 | `true` ×5 | `true` ×5 |

**What actually decides it is how much memory the host has left.** Under the
4-node cluster on a 3.8 GiB box the attribution was lost *systematically*; with
the same kernel and the same docker on the same box, idle, it is reliable. The
dev machine attributes correctly **while running the identical cluster**,
because 23 GiB leaves it 15 GiB of headroom. One `false` was also seen on the
idle LAN host in an earlier single run, so even idle it is not perfect there —
the honest reading is *headroom*, not a threshold anyone should write down.

**Three things follow.**

1. **The capture trip runs on a host with real memory headroom** — the dev
   machine. That the committed fixtures carry `OOMKilled` says only that they
   were captured somewhere with enough room, and
   [D57](#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)'s
   rule that the set describes *one afternoon* means the rest of it cannot be
   re-captured somewhere the word does not survive.
2. **The pinned node image is not the whole reproducibility contract.**
   `scripts/cluster.sh` pins `kindest/node:v1.36.1` precisely so fixtures do not
   change when `just fixtures` is re-run on a different machine — and the host's
   spare memory walks straight through that pin, silently, changing one word in
   one field. `tests/fixtures/K8S_VERSION` records the server version; **nothing
   records the machine**, and this entry is the only place that gap is written
   down.
3. **The guard held.** `just fixtures` runs `cluster.sh verify` *first*, so on a
   host that cannot produce the state the trip aborts before writing a byte and
   the good fixture is never overwritten. That ordering was written for a
   different reason — a fixture that never reached its state is a test that
   cannot fail — and it paid for itself here.

**Two traps for anyone reproducing this.** `--memory-swap` must equal `-m`, or
docker grants an equal amount of swap, nothing OOMs at all, and the container
runs forever; that cost two runs before it was noticed. And **`count=1` is the
difference between a container that OOMs and one that exits 0** — a `read()` of
100 MB from `/dev/zero` may return short, busybox `dd` counts a short read as
its one block, and it exits cleanly having never held 100 MB. `broken-oomserving`
shipped with `count=1` while the command validated here had no `count`: the two
differed by exactly the token that mattered, and `cluster.sh verify` was what
caught it. The manifest now uses `exec tail /dev/zero`, which has no newline to
stop at and therefore no short read to end on.

### D85 — rule 1 contradicts itself on a clean exit, and it gets its own box (2026-08-14)

The capture trip's third finding, and the one that justifies the trip on its
own: **two of the twelve new fixtures make rule 1 print a card that argues with
itself.** Neither object could exist in this repository before the trip, so no
test could fail and no review could see it — the rule shipped, was reviewed
twice, and was wrong the whole time.

```
● default/broken-exit0 · 29 min ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  container batch · 16 restarts · the last run lasted 2s · exit 0
  → read the previous run's logs — that is where it says why it exits

● default/broken-sigterm · 28 min ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  container app · 27 restarts · the last run lasted 4s · exit 143 (stopped with
  SIGTERM, which is an ordinary shutdown and not an error)
```

The first is a batch job that finishes cleanly under `restartPolicy: Always` —
the commonest way a beginner mis-writes a Job, and the kubelet does apply
`CrashLoopBackOff` to it, so the *state* is real. What is false is the sentence:
nothing crashed. The action is worse than the title, because it sends the reader
to logs that will say nothing is wrong, which is how somebody spends twenty
minutes proving their own tool wrong.

The second is the same defect with the contradiction fully on the page: a
**CRITICAL** card headed *"keeps crashing"* whose own evidence line says the exit
was *"an ordinary shutdown and not an error"*. Both halves are generated by
k8rs, one line apart.

**The knowledge already exists in the file and is not shared.** `previous_run_failed`
(rule 6) has exempted `exit 0` and `143` since it was written — that is the
clause the trip captured `exit0.json` and `sigterm.json` to prove. `crash_looping`
(rule 1) reads the waiting reason and never looks at how the last run ended, and
`exit_meaning` has no row for `0` at all, so it prints the code bare. Two rules
looking at one container disagree about whether anything is wrong.

**This is not the plain-language pass, and filing it there would mis-place it.**
The wording is a symptom; the fix is that rule 1 must consult how the previous
run ended, which is rule *logic* and needs its own tests, its own red-and-green
and an operator review. Nor is it the capture trip's box: that box's contract is
to bring the objects back, and it did — finding this is the trip paying off, not
the trip being unfinished.

**So the plan gets a box rather than a footnote**, inserted in Phase 3 ahead of
the plain-language pass ([CLAUDE.md](CLAUDE.md): if a step turns out to need
something the order does not give it, fix the order and record it). What it owes:
rule 1 silent or re-worded on a clean exit, `exit_meaning` given its `0` row, the
action stopped from pointing at logs that hold no answer, and both captured
objects asserted — they exist now, so the test can fail.

**Shipped in two rounds, and the operator review's blocker was inside the fix.**
The first version's exit-0 action told the reader the container *"belongs in a
Job or a CronJob rather than a workload that restarts it forever
(`restartPolicy: Always`)"* — and the reachable set of that branch is *pod-level
`Always`* **or a restartable init container**, so on a **native sidecar in a
Job** — KEP-753's headline case — both halves are false: the object already is a
Job and its `restartPolicy` is `Never`. `tests/fixtures/healthy-sidecar.json` is
one short run away from producing it. **That is this entry's own defect rebuilt
inside the fix for it**, which is why the actions are now role-aware.

**Three more things the review found, all one shape.** The title *"nothing has
crashed"* was an absolute built from one sample: `CrashLoopBackOff` is entered on
*accumulated* backoff and a clean run does not reset it, so four crashes then one
clean exit prints "nothing has crashed" beside `4 restarts`. The exit-0 card told
the reader to check `restartPolicy` while offering `kubectl describe pod`, which
prints no such field — `describePod` never touches it. And rule 6's new 137 arm
named probes without naming memory, when [D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
had just recorded that a genuine cgroup OOM arrives as plain `Error` on a starved
host — the correlation runs the wrong way, so 137-without-the-word is not proof
the kill was not for memory.

**The reviewer's own summary is the durable lesson: the rule reasons past its
evidence.** It makes a three-way claim about *why* a loop exists from a single
observation of one run, and the snapshot cannot hold a second. Every branch's
wording now stays inside what one `lastState` can support. **`"Container keeps
crashing"` is deliberately left as it was** — a *positive* claim of repeated
failure is carried by the state name plus one failed run, while *nothing has
crashed* needed a history the snapshot does not have. The asymmetry is the point,
not an oversight.

**One permanence to watch, recorded rather than fixed:** the "poor man's cron" —
a program run under `restartPolicy: Always` so the five-minute backoff acts as a
scheduler — now sits at CRITICAL forever, and nobody will clear it. It is not a
false positive, the pod genuinely is in backoff; but permanence is the failure
mode this project has now rediscovered three times
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
rule 6, [D75](#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13)
rule 2, and here).

**Two smaller relatives found in the same read**, for whoever takes the box:
`broken-startup`'s rule 6 card offers *"read the logs … to find the
application's own error"* over an exit 137 SIGKILL, where the kill came from
outside the application; and rule 1's own `restarts` count and `lasted` line
remain correct throughout, so the card is not wholly wrong — only its first
sentence and its last are, which is the shape that makes a reader distrust the
parts that were right.

### D86 — C1's parser costs three minor Rust versions, and the alternative was an accepted vulnerability (2026-08-14)

`x509-parser` was pre-approved by [invariant 10](CLAUDE.md) and arrives with C1.
Adding it turned `cargo deny` red on the first run:

```
error[vulnerability]: Denial of Service via Stack Exhaustion
   ├ ID: RUSTSEC-2026-0009
   ├ time v0.3.45
     ├── asn1-rs v0.7.2
     │   ├── der-parser v10.0.0
     │   │   └── x509-parser v0.18.1
```

**The fix and the cost are the same release.** The advisory is fixed in `time`
**0.3.47**, and 0.3.47 is the first release that requires **Rust 1.88** — so
there is no version that closes it at the 1.85 this repo had declared since
Phase 1. Nor is the dependency droppable: `asn1-rs` needs `time` to decode a
certificate's `UTCTime`/`GeneralizedTime`, which is the one thing C1 reads.

**`rust-version` moves to 1.88.** The alternative was a scoped `deny.toml`
exception, and it was rejected: an exception for a *licence* states a policy,
while an exception for a *vulnerability* states that somebody analysed
reachability and accepted the residual risk — a claim that has to be re-made
every time `x509-parser` moves, by whoever moves it, and which decays silently
when nobody does. The gate exists to be passable, and here it was.

**What the analysis would have said, recorded because it is the reason this was
a decision and not a reflex:** the vulnerable path is `time`'s **RFC 2822**
parser, and a certificate carries ASN.1 `UTCTime`, not RFC 2822 — so k8rs almost
certainly cannot reach it. *Almost certainly* is exactly the kind of sentence
that should not be load-bearing in a security gate, and it is written here
rather than in `deny.toml` so that it informs the next reader without excusing
anything.

**The cost, stated plainly:** anyone building k8rs from source needs Rust 1.88
rather than 1.85. It costs the release binaries nothing — those are built by CI
on current toolchains — and `rust-version` was a claim about who can compile
this, which is now three minor versions narrower and **true**, where leaving it
at 1.85 would have been one minor version wider and a lie the moment the lock
file moved.

### D87 — C1 has two bands and they belong on two screens; D2 only ever ruled on one of them (2026-08-14)

`dev-core` wrote C1 with `Warn` inside the window and `Critical` past it, then
raised the contradiction rather than burying it: my brief said *"called from
`analyze`"*, and
[D2](#d2--the-dividing-line-broken-now-vs-risky-later) says C1 goes to **the
Certificates report**, with the sidebar badge as its alerting mechanism.
`screens/alerts.md` draws no C1 card; `screens/analysis.md` draws it exactly.
Both cannot be obeyed.

**The ruling: `Info` while it is expiring, `Critical` once it has expired.**

**D2 is honoured for the band it ruled on, and no machinery is added to do it.**
`Severity::Info` already means *this finding lives in a report, not in Alerts* —
the enum says so in its own doc, and N4 and N5 already use it exactly this way.
An expiring certificate is D2's "risky later" and takes the same route the
kubelet-skew rule takes. The `▲` `screens/analysis.md` draws beside the 30-day
row is **the report's own marker, keyed to the date** — the same sketch draws
`○` at 210 days and `●` for a pending CSR — so it is not a claim about
`Finding::severity` and nothing there has to move.

**D2's letter is reversed for the expired band, because D2 never considered it.**
Read what it argued: it moved C1 out of Alerts as a thing that is *risky later*.
A certificate that expired five days ago is not risky later — it is **the most
broken-now object k8rs can see**, and it is the reason every other card on the
screen is absent. Without it the user gets a 401 and an empty list; with it they
get *"your kubeconfig certificate expired 5 days ago"*. That is the whole
dividing line D2 exists to draw, applied to the case it did not have in front of
it.

**Why this was worth a ruling rather than a default.** The cost of leaving it
implicit is precise and was named by the author: Phase 9 renders whatever
`analyze` returns, so an unruled `Warn` would have drawn a C1 card in Alerts
that `tui-designer` never specified and D2 forbids — a screen nobody designed,
arriving two phases from now, traceable to nothing. **When the reviewer and the
brief disagree, the PM decides in writing** ([CLAUDE.md](CLAUDE.md)); this is
that, and the brief was the thing that was wrong.

**Two sentences the ruling falsified, found by the author's own second pass and
worth naming as a class.** `Severity::Info`'s enum doc said *"No rule reaching
the Alerts list produces one"* — after this ruling C1 reaches that list through
`Critical` and produces an `Info` through the other band, so the sentence was
literally wrong; it now states the contract Phase 9 must implement instead
("nothing drawn in the Alerts list is an `Info`"). And `analyze`'s N4/N5 clause
justified their exclusion with *"no `Info` finding reaches the Alerts list, so
`analysis.rs` calls them and this does not"* — a causal claim C1 disproves. **A
decision that changes behaviour also changes what is true elsewhere**, and the
sentences it falsifies are never in the diff.

**`CERT_EXPIRY_WARN` keeps its name deliberately.** It shares a word with a
`Severity` variant the rule no longer uses, but it names the *window*, not the
band, and renaming it would touch the tests and `certs-test.sh` for no
behaviour.

**One consequence to carry forward:** C1 is now the only rule in `rules.rs` whose
severity decides which *screen* it appears on rather than only how loud it is.
Phase 4 owns the Certificates report and will read the `Info` band out of
`analyze`'s output; it does not need C1 reimplemented in `analysis.rs`, and
splitting one rule across two files to satisfy a routing question would have
been the expensive way to obey D2's letter while losing its point.

### D88 — an exit code names an ending, never an agent, and the boundary for folding a found defect in (2026-08-14)

The box [D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
opened for rule 5, and it looked like the cheapest box in the phase: `fn ending`
already existed, so this was one enum applied one rule over. It took **nine
rounds and three operator reviews, every one of which blocked**, and the defects
that mattered were found by mutation and by reading the shipped strings — not by
reading the diff.

What shipped is four arms, each owning its claim, its action **and its command**:

```
▲ default/broken-restarts10serving · 1 hour ago
  Container has been restarted 10 times — it is serving now, and its last run finished cleanly
  container flaky · exit 0 (the program finished successfully) · docker.io/library/busybox:latest
  → a clean exit says the program stopped without an error, not who stopped it — check the pod's
    events for a liveness or startup probe kill. If nothing stopped it the program is finishing on
    its own, and a program that is meant to finish belongs in a Job or a CronJob rather than a
    workload that restarts it forever
  $ kubectl describe pod broken-restarts10serving -n default

▲ default/broken-restarts10serving
  Container has been restarted 10 times — it is serving now
  container flaky · docker.io/library/busybox:latest
  → the pod has kept the count but not the run that ended, so nothing here says why. Check the
    pod's events, which may still name what stopped it — and if they have expired too, the next
    restart will write the run back into the pod, so watch it rather than guess
  $ kubectl describe pod broken-restarts10serving -n default
```

> **The first card above is a transcript of what shipped on 2026-08-14 and is no
> longer what the code draws** — its evidence line and its action were both
> replaced by [D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15).
> The second is unchanged. Kept as written because this entry is a record of a
> decision, not a description of the current code.

**The blocker: `exit 0` is the exit status of a process, not a statement about
who ended it.** The first draft's action read *"nothing killed that run — the
program ended on its own"*. A container that traps `SIGTERM` and shuts down
gracefully reports `0`, and the kubelet writes `0`/`Completed` regardless of who
ended it — so a failing liveness or startup probe on any application with a
graceful shutdown lands in that arm, and the card told its owner that nothing
killed a container something is killing every few minutes, then recommended
restructuring a healthy Deployment into a CronJob. **That is this entry's parent
charge — *the rule reasons past its evidence* — rebuilt inside the fix for it**,
and the second time a review has caught a repair reconstructing the defect it was
sent to remove
([D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
was the first; [D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)
is its neighbour — the door *beside* the one just closed, not the same door
rebuilt). The tell was available and missed: **rule 1 does not open with that
claim.** Its title states the exit and its action *asks* — *"check whether this
program is meant to finish"*. The fix went past the ruling it cited while citing
it. Rule 1 is not clean, and this entry should not be read as saying so: it asks
first and then **closes** with an assertion — *"it is quitting early and that is
the bug"* — which offers two readings where the true one is a third, and which
has its own box for that reason.

**Rule 1 and rule 5 now answer the same `exit 0` with different kubectl
commands, deliberately.** *(Reversed a day later by
[D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15):
both rules now take `describe`, because `CrashLoopBackOff` behind a clean exit
already fixes `restartPolicy` to `Always` while the events remain the only
discriminator. The paragraph stands as the reasoning that was made at the time.)*
Rule 1 names `restartPolicy` and takes
`kubectl get pod -o yaml`; rule 5 names the pod's events and takes
`kubectl describe pod`. Invariant 4 allows one command per card, so each card
gets **one family of facts**, and which family follows from the question its own
state leaves open: rule 1's container is in `CrashLoopBackOff` and the open
question is *why it is being started again*, which `restartPolicy` answers; rule
5's is **serving**, and the open question is *what ended the last run*, which
only the `Unhealthy` / `Killing` events can separate from *it finished on its
own*. `restartPolicy` lost the seat, and rule 5's sidecar arm had to make the
same point without naming the field. A card whose command cannot show what its
action names is the failure this trade exists to avoid, not a corner of it.

**`Init` + `Finished` was reported unreachable and is not.** The exemption is
`role == Init && doing_its_job(c)`, and `doing_its_job` reads the **current**
state: a plain init container that is running is `ready: false`, so the guard
never fires whatever `lastState` holds. The producer is **pod sandbox
recreation** — Kubernetes re-runs every init container when it rebuilds the
sandbox (a node reboot, a container-runtime restart), while `restartCount` and
`lastState` persist on the same pod object, so three generations reach the band
with a clean exit behind them. The card that came out told an init container to
check a probe `validateInitContainers` forbids it having — the sentence rule 5's
*own* `Stopped` arm exists to refuse — and called finishing its bug, one line
under an evidence line reading *"the app starts only after this one finishes"*.
**The author's own lesson, reported rather than buried: it reasoned about the arm
it had written instead of the guard that gates it.** A guard is what a reviewer
reads last and what decides whether any of the wording is ever seen.

**When a defect found mid-box folds into it, and when it gets its own box.** The
`Failed` arm turned out to hand every role the probe advice, `Init` included —
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)'s
defect one arm over, on a shape a cluster produces daily and no committed capture
holds, so the test synthesizes it. It was escalated, not
fixed, which was right; the ruling was to **fold it in**, and the boundary is
this: D85 crossed **into another rule**, where an untested branch invented inside
someone else's box is the scope creep CLAUDE.md names, so it earned a box of its
own. This crossed nothing — same rule, same function, same class of defect, and
this box is what turned it from a cross-rule disagreement into a **self**-
contradiction, with `stopped_action` refusing the probe sentence two arms below
the one still handing it out. The test is not *how big is the fix* but *does
closing it require inventing behaviour in somebody else's subject*. A second
thing was true and worth separating: the PM had cleared that arm twice, but only
its **claim** — role-blindness was a question nobody had asked, so answering it
reversed no ruling.

**The sharper form of that boundary, which the box then applied three times:**
escalate when closing the finding would **invent behaviour** in another rule's
subject; fold it in when it only **records a fact already true** of a card that
has shipped. The `Failed` arm's role split records a fact (init containers may
not have probes) — folded in. Rule 1's own test lacking a pin records a fact
about a string this box merged — folded in. Rule 1's clean-exit action offering
two readings where the true one is a third *invents* the missing reading and
reopens which command that card carries — its own box. Size is not the test, and
"it is only one line" is the argument that gets a rule rewritten inside somebody
else's box.

**`stopped_action` and `failed_action` exist because copying rule 1's strings
verbatim is exactly how two rules drift apart.** The second round reused rule 1's
wording by pasting it, leaving two byte-identical four-line strings in one file —
against CLAUDE.md's *never write the same code twice*, and against this entry's
whole subject. They are shared now and deliberately **not** merged with each
other. The evidence that the sharing is real rather than a function wrapped round
one copy: a single mutation of `stopped_action` kills a test in **both** rules.
**And sharing a sentence hid something the first time: coverage can be shared by
accident.** Once the string was merged, rule 1's init branch was pinned only
*through* the helper and rule 5's test — a mutation that gutted that sentence
left rule 1's own test green. That is the failure this box removed, rebuilt one
level up: merge the sentence, leave the coverage as a single copy somewhere else,
and splitting the helper again silently strips a rule's only pin with nothing
going red. It was closed with one assertion in rule 1's own test, which now dies
beside rule 5's to the same mutation. The rule: **a shared string owes each
caller its own pin**, or the sharing is load-bearing in a way nobody wrote down.

**The arm that claims nothing.** With no `lastState` at all — container GC
dropping the dead container, a runtime that lost its container store while
`/var/log/pods` survived to feed `calcRestartCountByLogDir`, a manual
`crictl rm`; **not** a kubelet restart, which re-derives the status from a
runtime that still holds the container — `restartCount` survives the run that
produced it. A count says
restarts happened, not that anything killed them, so that arm's title stops after
*"it is serving now"*.

**Its first command could never have worked, and the operator review is what
caught it.** The arm offered `kubectl logs … --previous` — and the kubelet gates
`--previous` on `lastState.terminated.containerID`, *the same field whose absence
puts a card in this arm*. Not "usually fails": there is no state in which the
branch fires and the command succeeds, by construction, off the bytes the branch
itself read. The hedge written to cover it — *the log is the only record left,
for as long as it is kept* — had the dependency backwards: the pod's record is
what makes the log reachable through the API at all, and kubelet's GC deletes
both in one call. The arm now carries `describe` like its siblings and tells the
reader something to **do** — the next restart writes the run back into the pod,
so watch rather than guess. **The general form, worth more than the fix:** a
branch that reads a field to decide it exists must not then offer a command the
API gates on that same field. Nothing in a pure rules file can discover this;
it took someone who knew where the kubelet checks. **One consequence recorded for
whoever writes the Alerts renderer:** this card carries
**no age**, because `Finding::timestamp` reads `last_terminated.finished_at`.
That was already true of any container past the band with no previous run; this
box makes it a named, worded state rather than an accident.

**The test hole that only a mutation could find.** The same arm was pinned with
`action.contains("log")` — loose on prose, tight on the command, and defended as
deliberate. `tester` mutated the action to send the reader to the node's system
log while leaving `kubectl logs --previous` as the command: **every test stayed
green**, shipping invariant 4 broken in the exact direction this box was closing.
The replacement pin then failed the author's own shipped caveat, which named the
node in passing, and the caveat was reworded. **A pin that needs an exception for
its own author's sentence is not a pin** — and reading the test would never have
shown either, which is what [D26](#d26--a-green-build-that-proves-nothing-2026-08-12)
buys.

**Then the same hole turned up three more times, and its shape is now nameable:
an arm pinned only by what it must *not* say, where the sibling arm's shipped
sentence satisfies every one of those negatives.** A sidecar card could print the
init arm's real sentence — about Kubernetes rebuilding the pod's sandbox — and
stay green, because its test asked only that the words *Job* and *CronJob* were
absent. Two more arms were pinned by a phrase both siblings share. Each is one
positive assertion away from closed, and each proves the split it belongs to in
one direction only: regrouping `Init` with `Sidecar` died, the reverse did not.
The rule that comes out of it, beyond this file: **a negative assertion cannot
pin a branch whose sibling would also pass it** — name the one thing only this
arm says. The comment reasoning this out for the `Regular` arm was already in the
test file when the other three arms shipped without it, which is the ordinary way
a lesson fails to travel the eighty lines to its neighbour.

**Then a third coat, and with it the shape underneath all of them.** The fix that
qualified `137` — memory *only* with `OOMKilled` beside it — was pinned by one of
its two conjuncts, so deleting the qualifier and tidying the leftover prose left
the card asserting flatly that `137` is memory, with the suite green. Three
instances, one failure: **a pin that proves a sentence contains the right words,
against a mutation that keeps the words and drops the logic.** `contains("log")`
under a command that could never run; three negatives a sibling arm also
satisfied; a conjunction held up by one conjunct. The rule that closes all three
is the same one each time, and it was re-learned one arm over each time: **assert
the thing that makes the claim true, never a token from the sentence carrying
it.** The author wrote that line themselves after the third, which is the only
reason it is here rather than waiting for a fourth.

**And then the lesson ate its own tail, which is the durable half of it.** That
same line went into a comment claiming both new pins were requirement-shaped —
while one of them was still a token pin, as the reviewer proved by rewriting the
sentence faithfully and watching it red. Every hole in this box survived its
review the same way: a comment or a report said the coverage was stronger than it
was, and everybody downstream believed it. **The claim about a test is part of
the test, and it needs the same standard of proof as the assertion under it.**

**The last defect in the box was the same mistake twice, in opposite directions,
and the fix was to stop choosing.** The init arm's `137` sentence first said the
kernel took it for memory — true only with `reason: OOMKilled` beside it
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
Corrected, it said a `137` without the word was a program that would not stop
when asked — which
[D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
refutes, because a starved host delivers real cgroup kills as plain `Error`. Each
version was a **branch on evidence the object does not carry**, and the second
was worse than the first: it ruled memory out exactly when a memory-starved node
made memory likeliest, with rule 2 silent because it keys on the word. The
sentence became correct only when it stopped branching and named the limit
without making the word's absence mean anything. **The general rule, which
reaches past this card: where two readings are indistinguishable from the
snapshot, a card that picks one is wrong half the time and the reader cannot tell
which half.** That is invariant 5's *a missing field means no finding*, carried
into the wording — and the proof it is a requirement rather than a preference is
the mutation that put the recital back **correctly qualified in both directions**
and still went red.

**One word kept as jargon, against the usual reflex.** The init arm says
Kubernetes *"rebuilds the pod's sandbox"*. Invariant 14 would translate it away;
it stays because the reader is being sent to the pod's events, where the word
they must match is `SandboxChanged`. Explaining a term in place beats hiding a
term they have to recognise.

**What a capture trip still owes here**, written into the test doc comments
rather than left in a head: no committed capture reaches any ending but
`Failed` on this rule. Wanted — a container that reaches the band by *finishing*
(`exit 0`, and a second on `kill -TERM`) while running and out of
`CrashLoopBackOff`, and the `Init` + `Finished` shape a rebuilt sandbox produces.
`restarts10.json`'s own `spec` is one character from the first
([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12),
[D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)).

**And one finding this box deliberately did not absorb:** `screens/alerts.md`
caps a card's action at five lines and says an action past that is a `rules.rs`
finding — but wrapped at that file's own 49 columns, **9 of the 52 distinct
actions the rules print exceed it**, six to nine lines, across rules 1, 5 and 6.
The budget has been false since before this box, so it is a
`screens/` question for `tui-designer` and has its own box in `todo.md`, which is
the same boundary the `Failed` arm was tested against and lands on the other side
of it.

### D89 — k9s's tracker is read as prior art, and twelve of its classes become boxes (2026-08-14)

**What happened.** The user asked for the k9s issue tracker to be read and
compiled — *"dikkat edeceğimiz yerler"*, the places to be careful. It is the one
body of evidence this project has that was not produced by this project: 2324
closed and 48 open issues, seven years of other people's users telling a
Kubernetes TUI where it hurts. The result is [PRIOR-ART.md](PRIOR-ART.md), and
the user then ruled that its twelve open gaps become boxes.

**PRIOR-ART.md is evidence, not a plan, and the distinction is load-bearing.**
Nothing in it is a request to build what k9s built —
[§ Out of scope](#out-of-scope-the-most-important-section) and invariant 13 still
decide that, and the file says so in its second paragraph. Three k9s features
were looked at and *not* adopted, the closest call being API-server `Warning:`
headers (k9s [#4106](https://github.com/derailed/k9s/issues/4106)): free,
authoritative, exactly our subject matter — and if it is ever built it is a rule,
not a UI feature, so it waits for the rule set rather than jumping the queue.

**What the ruling did not decide, and this entry does.**

- **Two gaps were folded into boxes that already existed, not added beside
  them.** The reconnect gap joined *"Reconnect/backoff surfaced as a state"*, and
  the colour-is-not-a-signal gap joined *"Severity symbols `● ▲ ○` — never colour
  alone"*. A new box next to an existing one covering the same ground is exactly
  the cross-box defect the phase-close pass exists to catch: two boxes, solved
  differently, both true. Ten new boxes, two amendments, twelve gaps.
- **The coalescing test moved from Phase 10/11 to Phase 12.** The file guessed at
  the view layer; the coalescer is in `main.rs`'s `tokio::select!`, so the test
  belongs beside it or it tests something else.
- **The typed-error rule lands as a Phase 5 box that also edits
  `docs/architecture.md`**, rather than as a docs-only change. A rule about where
  a message may come from has to be built, not only written down.
- **The emit-path gap lands once, in Phase 6**, not once there and once in Phase
  11. It defines two functions; the later phase uses them.

**The finding that came out of reading the code rather than the tracker.** The
first draft of § F3 said `containerStatuses` and `spec.containers` are not
guaranteed to match in length or order, so *"index-by-position is a panic"* — the
defect k9s shipped in `initContainerStats`. `container_snapshots` already pairs by
**name**, and says why in a comment, so that half was never ours. What the
comment also says is that a status with no declaration **cannot exist**, because
*"both container lists are immutable after create"* — and it uses that to explain
why the miss has no test. k9s
[#4145](https://github.com/derailed/k9s/issues/4145) is a field report of the
object: on Tencent TKE **virtual nodes** the provider injects a managed logging
container into `status.containerStatuses` with no entry in `spec.containers` —
two declared containers, three ready statuses, pod `Ready: True`. Immutability is
not what breaks the assumption; a node implementation that is not a kubelet is,
and virtual-kubelet, serverless nodes and sandboxed runtimes all sit there. The
gap shrank from a panic we do not have to an assumption we do have, which is the
smaller and truer box — and it is in **Phase 3**, the open phase, not a later one.

**Cost, stated.** A thirteenth file at the repo root, listed in
[CLAUDE.md](CLAUDE.md)'s map. It is the only file here whose subject is another
project, and it dates: every issue number is a claim about a tracker that moves.
It is a snapshot taken on 2026-08-14 with the queries printed at the top of it, so
it can be re-run rather than trusted.

**Two claims in the file were wrong on its first draft and are named here so the
pattern is visible**: the F3 one above, and § A6, which said the 21.5 GB k9s
process ([#871](https://github.com/derailed/k9s/issues/871)) grew because of a log
stream — the thread never identifies what grew. Both were assertions about
something the reviewer had not opened: one our code, one their thread. That is the
same defect twice, and it is the one this file's own second pass exists to catch.

**Added 2026-08-15 — how the file is consulted, after the user asked whether
`todo.md` should be reworked against it a second time.** It should not, and the
check came before the answer: all twelve gaps were traced line by line to the box
that answers them, each in the phase this entry's table claims. A second planning
pass over the same file produces a second box beside an existing one, which is
this entry's own warning. What was missing is not boxes but **timing** — the file
is a review checklist, and its classes bite when the code they describe is being
written, phases from now. Two pointers were added instead, both zero-maintenance:
each affected phase in `todo.md` opens with the sections to read first, and
`k8s-admin`'s brief gains a seventh check — *has this class already broken k9s* —
because sorting, an incomplete denominator, a wrap that leaks into the data and a
generic message eating a typed error are all found in review rather than while the
code is written. **An entry tagged `immune` is an argument to defend, not a box to
tick**: a change that would reverse the decision earning the tag is a finding.

### D90 — the third door, and the command trade D88 made a day earlier (2026-08-15)

[D88](#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)
closed rule 5 and named its own leftover: rule 1's clean-exit action *asks* first
and then **closes** with an assertion — *"if it is not meant to finish, it is
quitting early and that is the bug"* — an exhaustive pair whose true third
reading is missing. This is that box. It took five rounds, four `tester` passes
and three operator reviews; the review that settled it brought up a kind cluster
and reproduced the object no fixture in this repo holds.

What ships, all three arms of one shared `finished_action`, drawn by rules 1
and 5 alike:

```
● default/broken-exit0 · 29 min ago
  The container's last run finished cleanly, and Kubernetes is restarting it (CrashLoopBackOff)
  container batch · 16 restarts · the last run lasted 2s · exit 0 (the run ended without an error)
  → exit 0 says the run ended, not who stopped it — check the pod's events for a Killing
    line and the node for a memory killer. If nothing stopped it, it ends itself: if that
    is meant, it belongs in a Job or a CronJob; if not, it is quitting early
  $ kubectl describe pod broken-exit0 -n default
```

**D88's command trade is reversed, and the reversal is the box's other half.**
Rule 1's `Finished` arm named `restartPolicy` and therefore took
`kubectl get pod -o yaml`, which prints no events at all — so the `Killing` line
that would correct the card was exactly what its own command could not show. The
field turned out to be **implied by the state**: `ShouldContainerBeRestarted`
returns `false` under `restartPolicy: Never`, and under `OnFailure` when
`exitCode == 0`, so a plain container sitting in `CrashLoopBackOff` behind a
clean exit can only be under `Always`. D88 bought a command to print a field the
state already fixes and paid for it with the only discriminator the object has.
For a `Sidecar` it is the container's **own**
`initContainers[].restartPolicy: Always` — the field that made it a sidecar in
the first place — which is why the shipped sentence names a policy and not the
pod's. **No card in `rules.rs` names `restartPolicy` any more**; `get_yaml`
stays for rules 3, 4, 12 and the W-series.

**One helper, two rules.** `finished_action(role)` joins `stopped_action` and
`failed_action`: two byte-identical copies of a sentence is
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)'s
defect with a delay on it.

**`Init` + `CrashLoopBackOff` + `exit 0` is reachable, and that is the second
"unreachable" in this area that was not.** `doBackOff` selects by container name
and `ContainerStateExited` and **never reads the exit code**; `SyncPod` runs init
containers through the same `start` closure; a sandbox rebuild re-runs every init
container while `restartCount` and `lastState` persist on the same pod object.
**A node reboot is not a producer for rule 1** — `kl.backOff` is built in-process
by `NewMainKubelet`, so a restarted kubelet has an empty map and nothing to make
the re-run wait — while it **is** one for rule 5, which gates on the count alone.
That asymmetry is why the two rules' doc comments differ, and it is written down
so the next reader does not tidy it away.

**The blocker: the first fix permuted the doors instead of completing them.** It
dropped *quitting early* while adding *something stopped it*, so an `nginx` with
the stock entrypoint and no `daemon off;`, a `sh -c './server &'`, a Java `main`
that returns while its daemon threads die — all `exit 0` in under a second on the
shape `exit0.json` holds — were sent to events that hold nothing and then offered
a CronJob. **D88's own blocker sentence, rebuilt inside the fix for the box D88
opened for it**, and the third time a repair in this area reconstructed the
defect it was sent to remove. What ships is three branches, none of them a
verdict: something stopped it · it stopped itself and is meant to · it stopped
itself and is not.

**Door 1 is the killer, not the probe** — measured on kind v1.36.1, not argued.
Every kubelet-initiated stop goes through `killContainer`, which records a
`Killing` event whatever asked for it, and its message names the probe itself:
`Container app failed liveness probe, will be restarted`. So the card names
`Killing` and drops the words *liveness* and *startup* — `describe` hands them
back on the same row. **`Unhealthy` is deliberately not named**: a failing
*readiness* probe writes it with nothing killed behind it, and a reader who greps
the word the card gave them would close the door on the wrong evidence. The node
is named beside it for the killer that writes no event at all, and whether an
outside killer arrives as `143` or as `0` is decided by the application's own
SIGTERM handler, not by who sent the signal.

**The `Init` arm's events clause is hedged, because the commonest rebuild writes
no event.** `SandboxChanged` is emitted only where the kubelet finds a sandbox
that *changed* (`podContainerChanges.SandboxID` non-empty); where the sandbox is
simply **gone** — `crictl rmp`, a runtime restart, a node reboot — it re-runs
every init container while logging at V(4) and emitting nothing. Both paths were
run on kind: the removed-sandbox pod came back with `Restart Count: 1`, `exit 0`
and `Pulled` / `Created` / `Started` `(x2)` and no line saying why. So the card
says the events *often do not say why* and points at the node without an *after
that* in front of it — the answer for the reader who is late **and** for the one
whose events never carried the reason.

**`exit_meaning`'s `0` row named an agent one line above an action whose whole
subject is that the code names none.** *"The program finished successfully"* is
false of every graceful shutdown a probe asked for, and it printed directly under
this card. It is now *"the run ended without an error"*. The row reaches the
screen from rules 1 and 5 only — rule 6 exempts `0` and `143` — which is why it
was settled here rather than in the open `137` box; that box keeps `137` and the
role question, and gains what the same cluster trip measured: a sandbox rebuild
gives a healthy container `137` with `reason: ContainerStatusUnknown` or `Error`,
which the current `137` sentence is wrong about for every role.

**`screens/alerts.md`'s five-line action cap became a `rules.rs` test.** That
file already said an over-budget action is a rule defect and not a layout
problem; nothing enforced it. The three arms went **9 / 8 / 9 wrapped lines to
5 / 5 / 5** at 49 columns, and
`the_clean_exit_actions_fit_the_card_they_are_drawn_on` holds them there —
including a self-test of its own wrapper, which under-measured a token wider than
the line until it was made to break one by character the way the renderer does.
**The doors were never what cost the space**: three readings fit in five lines,
the preamble and the restatements did not. Four sibling actions still break the
cap and have their own box.

**Two compressions and one divergence, recorded rather than discovered later.**
*The program ends itself* → *it ends itself* on the `Regular` arm bought the
subject back for *if that is meant* (invariant 14: an elided subject heading a
clause is a telegram); the `Sidecar` arm keeps the longer wording, so the two
word one shared clause differently, which is accepted — neither is defective and
the budget is per arm. On `Init`, the second *again* paid for the hedge, and the
question one clause earlier keeps *runs them all* a re-run.

**D88's per-caller pin rule was kept at arm granularity and rebuilt one level
down.** Deleting a *clause* from a shared arm took only rule 5's tests red twice
over, and a card could lose the conditional framing on the `Sidecar` arm with all
177 tests green. Four rounds of mutation found what four rounds of reading did
not; the pins are now two shared assertion helpers called once per caller.
Accepted costs, so they are not rediscovered as bugs: the literal
*If nothing stopped* is pinned at several sites, so a faithful rewording is a red
build; positive substring pins cannot catch a door that is *negated* rather than
deleted; and the 49 columns and the five-line cap live in the test as transcribed
constants, because the only stronger option is parsing `screens/alerts.md`.

**What could not be proven, and what the next capture trip owes.** No committed
capture reaches any ending but `Failed` on these arms, so every other arm is
tested on a decoded copy ([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).
The object that would retire most of that is one word away in
`scripts/broken.yaml` — `trap 'exit 0' TERM` instead of `exit 143` on
`broken-sigterm` gives a real probe kill that reports `0`. `Init:CrashLoopBackOff`
with a clean run behind it needs `crictl rmp` inside a live backoff window, and
the `restartCount`-across-a-reboot half of the asymmetry above needs a restarted
node.

**The boundary, since D88 set it and this box tested it:** three findings were
folded in — the missing third door, `Unhealthy` vs `Killing`, and the `0` row —
because each sat inside the string this box was already rewriting and each fix
made the card *shorter*. Four were filed as boxes, because each crosses into
another rule's action or another agent's file. A found defect is folded in when
fixing it is smaller than describing it; it gets a box when closing it would need
a decision the current box has not made.

### D91 — the tests split, and the product file does not (2026-08-15)

The user asked whether the `.rs` files can be made modular. Measured before
answering, because the file that felt biggest is not the one that is:

| file | lines | of which code | doc/comment |
|---|---|---|---|
| `src/rules.rs` | 4 339 | **2 097** | 2 100 (48%) |
| `src/rules_tests.rs` | 13 105 | **9 663** | 2 863 |

**Half of `rules.rs` is the documentation this repo requires** — every rule
citing the decision that shaped it — so the product file is 2 000 lines of code
across 79 functions, which is an ordinary Rust module. The test file is three
times its size and is what every agent turn pages through.

**Ruling: split `src/rules_tests.rs` by rule family, leave the product file
whole.** `rules.rs` keeps its single `#[cfg(test)] #[path = …] mod tests;`
declaration; `rules_tests.rs` becomes a few `#[path]` lines, one module per
`// --- … START ---` region of `rules.rs` — snapshot, pod, node, workload,
certificate — so the two trees keep the same shape and a reader who knows where
a rule lives knows where its tests live. **Invariant 11's eight flat product
files stand**; what deviates is its test clause, which named exactly one
`<name>_tests.rs` file, and that deviation is this entry.

**Why the product file stays whole**, and it is not conservatism: every defect
this phase lost days to was two rules reading one container and disagreeing —
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14),
[D88](#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14),
[D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15) —
and the fix each time was **one shared helper in one file**. `exit_meaning`,
`ending`, `describe`, `finished_action` are single readings that two rules must
not diverge on; a module boundary is where the second copy grows back.

**Why at Phase 3 close and not now:** eight boxes are open against `rules.rs`
and its tests. Moving the tests under them lands every open box in a file that
has just moved, which is the forward-only rule read backwards.

**What it does not cost:** `scripts/test-guard.py` and `scripts/write-guard.py`
both `rglob("*.rs")` over `src` / `tests` / `examples` / `benches`, so a
subdirectory is walked with no edit to either — checked, not assumed. The
177 declared / 177 listed count is the box's own proof that nothing was hidden
by the move.

### D92 — who may touch a cluster, split by the artifact and not by the agent (2026-08-15)

`k8s-admin` brought up a kind cluster during the [D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)
review and measured what no fixture holds. That was logged as an *exception*.
It was not one — it was **two sentences that cannot both be obeyed**, which is
the second pass's first question and it had never been asked of these two files:

- `.claude/agents/k8s-admin.md` — *"You may run `kubectl` and `just` against the
  kind test cluster to check a claim rather than assume it. Prefer checking."*
- `CLAUDE.md` § The boxes no agent can run — *"the kind cluster … the agents do
  not have"*.

The reviewer followed its own brief. Ruling by what the rule was protecting:
**fabrication, not access.** "A box whose evidence is *this would work* is an
unchecked box" guards against an agent claiming a run it could not perform —
and an agent that pastes real `kubectl` output is not doing that. So the line
moves off *who* and onto *what is produced*:

| act | who | why |
|---|---|---|
| **ephemeral measurement** — bring a cluster up, check a claim, tear it down | `k8s-admin` | the reviewer is the one who knows what to measure; routing it through the PM means the PM guessing the right call from a finding |
| **artifact production** — `just fixtures`, anything writing into `tests/`, anything whose green *is* a box's done-when (`just e2e`) | **PM only** | a fixture is committed data. [D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12) and the fixture-sanitization gate live on that path; it gets no unreviewed link |
| **any cluster at all** | `dev-core` / `dev-ui` — **no** | a writer with a cluster tunes the code until the cluster agrees, which is CLAUDE.md's own *"never assert what the implementation happens to return"* failure. The hand that measures is not the hand that writes |

**The hard condition, and it is not a courtesy.** `scripts/cluster.sh` reads
`CLUSTER="${K8RS_CLUSTER:-k8rs}"`, so a reviewer that takes the default collides
with the PM's fixture cluster and deletes it on teardown. Worse, a second
4-node cluster on the 3.8 GiB LAN host is exactly the state
[D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
documents:
every OOM capture comes back `reason: "Error"` instead of `"OOMKilled"` —
**silent, and semantically inverted**. A concurrent reviewer can therefore
poison a capture nobody re-reads. Hence **`K8RS_CLUSTER=review`**, one cluster at
a time, torn down in the same report.

**The name is chosen so the rule is mechanical, not promised.**
`scripts/sanitize.jq` aborts on any capture whose node names do not
`startswith("k8rs-")` — so a cluster called `review` produces nodes named
`review-control-plane` and **physically cannot yield a committed fixture**,
while `k8rs-review` would have sailed straight through the guard. That was the
first name written here and it was wrong for exactly the reason invariant 1
gives for the allowlist: a boundary that depends on everyone remembering it is
not a boundary. **The cluster is a file tree** in the
sense [D60](#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)
gave the scratchpad — one writer, named per owner.

**What a measurement is worth.** Evidence for a *finding*, never a box's
done-when. D90 is the shape: the measurement settled the design question, and
the box still closed on `just check` run by `tester`. A box that needs a cluster
to *close* stays a PM box, unchanged.

**Why this entry exists at all:** the question was put to the user, and the
answer was that this class of call is the PM's to make and record. It is —
CLAUDE.md already says the PM decides in writing when two parties disagree, and
a contradiction between two committed files is that, with nobody in the room to
argue it.

### D93 — an exit code is translated once for every role, and `137` is read from the object rather than from the number (2026-08-15)

`todo.md`'s three-part `137` box asked one question in three places, and it had
to be answered once or the three answers drift — which is the whole of
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14).
The question: **what does `137` mean for a container that may not be allowed a
probe?**

**The door taken: `exit_meaning` stays role-blind, and its bare-`137` row stops
naming a cause.** The box offered the other door — make the translation
role-aware — and it was refused for three reasons:

- **A translation names how a run ended; naming *what* ended it is diagnosis**,
  and diagnosis is the action's job. The action is already split by role in
  four places (`finished_action`, `stopped_action`, `failed_action`, and now
  `killed_action`), so a role fork in the translation restates one line down
  what the card already says one line up.
- **`exit_meaning` is printed by rules 1, 5 and 6.** A role argument there
  changes three rules' cards at once and puts the same branch in three places
  that must not disagree — the defect the box exists to remove, rebuilt as its
  fix.
- **Three causes reach the same `137`-without-`OOMKilled` shape and the object
  separates none of them**: a SIGKILL after an unanswered SIGTERM; a genuine
  cgroup kill whose word was lost on a host short of memory
  ([D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14));
  and a sandbox rebuild on a container nothing asked to stop
  ([D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
  **The third reaches the bare row even after the reasons below were added**, and
  that is worth stating because it looks contradicted by them: D90 measured the
  rebuild twice, and only the *removed* sandbox writes
  `ContainerStatusUnknown` — a sandbox merely **stopped** arrives as plain
  `137`/`Error`, indistinguishable from the other two. A one-line evidence field
  naming one of three is wrong two-thirds of the time, and it was naming the one
  an init container cannot have.

**`137` with `reason: ContainerStatusUnknown` gets its own row**, by the same
mechanism D71 established for `OOMKilled`: *the object says which*. It is not a
kill at all — `convertToAPIContainerStatuses` writes `exitCode: 137` where the
kubelet could not read a status, with `// this code indicates an error` beside
the number in its own source. **Verified from the kubelet source before the row
shipped, not from D90's measurement**, which observed the pair from outside and
could not tell a synthesized number from a watched one.

**A fourth reason turned up in the operator review and it is not a finding at
all.** `RestartAllContainersOnContainerExits` is `{1.36, Default: true, Beta}` at
the version `tests/fixtures/K8S_VERSION` pins, and the kubelet writes
`exitCode: 137, reason: RestartingAllContainers` when a pod's own
`restartPolicyRules` remove a container. Measured on kind with no gates touched.
Two halves, ruled apart:

- **The translation row ships** — the `reason` beside the code says exactly what
  removed it, and *"the code does not say what sent it"* is false whenever it
  does. Same function, same question, verified in the kubelet source before the
  row was written.
- **Rule 6 goes silent on it**, beside `exit 0`, `exit 143` and `OOMKilled`. The
  pod declared the rule, the kubelet obeyed it, and nothing failed — a WARN card
  for a declared policy working correctly is D71's false-positive class on a
  field that never expires.

**"Nothing is lost, the sibling draws its card" was written into three files and
is true of one phase of two.** The second operator review measured the other: the
kubelet writes the synthesized `137` record into **every** container's
`lastState`, the trigger included, while the trigger's own bad exit sits in
`state.terminated`. In that phase rule 6 is exempt on both containers and **no
rule reads the `exit 3` that started it as an ending** — `doing_its_job` reads
that field, but only to ask whether an init container finished — so rule 5's
count is the only card.
The claim is corrected wherever it was made, to *a container that failed draws its
card in the phase where its own record has landed in `lastState`*, and **the other
phase is a hole and not a hand-off** — the register `stuck_at_the_starting_line`
already uses for the same shape. The gap is boxed; the exemption stands, because
the card it removes was never the one that diagnosed the failure.

**Adding the row created the contradiction it then had to fix**, which is
[D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)'s
shape — the title read *the previous run failed* one line above *which is what
this pod asked for*. The exemption removes the card and the contradiction
together. **It also removes a kubelet-authored log line as a side effect, and
that is not the fix**: `ContainerStatusUnknown` is scoped out of the log arm by
arm order and `RestartingAllContainers` by a rule exemption granted for an
unrelated reason, so two ad-hoc mechanisms now hide one class defect. That is
written into the box that owes the class fix, because two accidental greens are
how a third instance ships.

**Rule 6's `137` action splits by role through `killed_action`**, shared rather
than spelled a fourth time. It sent every role after a liveness probe while rule
5's card on the same container said Kubernetes allows an init container none —
two k8rs cards, one object, one screen.

**What was left out, and why the boundary sits there.** Rules 1 and 5 print the
new translation and their actions were **not** taught about
`ContainerStatusUnknown`: rule 5 offers *check the memory limit* under an
evidence line saying Kubernetes lost track of the container. Found twice, from
both sides — by `dev-core` while writing the fix and by `tester` while attacking
it — and boxed rather than folded in, because it asks a **different** question
(*what should a rule do about a run Kubernetes never watched end*) and because
answering it decides rule 1's `Failed` arm, which is an open box below this one.
That is [D88](#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)'s
boundary applied, and what it leaves is generic advice under specific evidence,
not the denial printed beside the thing it denies that this box removed. **The
operator review agreed the boxing was right and corrected the box twice**: rule
1's half is not merely generic, it names a log the API cannot serve, because
`--previous` is gated on the `containerID` the synthesized status does not carry;
and rule 5's card is *permanent*, since `lastState` never expires and
`restartCount` never falls.

**The guard the box owed was written, and it did not hold on the first draft.**
*No card k8rs draws about an init container names a probe anywhere* — title,
evidence or action, across rules 1, 5 and 6. `tester` broke it twice: the word
list was matched **case-sensitively**, so `"Probes are worth checking"` planted
into the one arm the guard exists to protect passed green — a framing hole of
exactly [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)'s
kind, in the check written to close a framing hole; and the card total was
printed and never asserted, so deleting the two shapes part (iii) is about left
the guard green on a smaller set
([D26](#d26--a-green-build-that-proves-nothing-2026-08-12)). **A guard is only
proven for the framings it was fed, including the framings of its own haystack**,
and that is the general lesson: this one had been fed the shapes and not the
letter-cases.

**A gap accepted, in writing, because closing it would need an object no cluster
writes.** Rule 6's new title branches on `reason == ContainerStatusUnknown`, and
`tester` proved no test can tell that implementation apart from one keyed on the
null `finishedAt` beside it: a stamp-keyed branch passes all 181.

**The first wording of this paragraph said no producible object separates the two
keys, and that is false** — the second operator review produced one. A
`RestartingAllContainers` record carries `finishedAt: null` **and** a reason that
is not `ContainerStatusUnknown`, so a stamp-keyed branch would put *No record of
how the container's last run ended* over a run whose record is complete and whose
message says exactly what happened. It never reaches the branch because the
exemption three lines above returns first. **So the indistinguishability is a
property of the exemption list, not of the object space**, and the day that
exemption is narrowed — which the box below has to consider, since it must teach
rules 1 and 5 about the same reason — the two keys diverge on the first object
the cluster writes. The comment beside the branch now cites the exemption
assertion as the thing that holds the ruling up, so the reason is visible where
it would stop holding. (A third stamp-less literal exists for completeness:
`kubelet_pods.go:2714-2718` writes `Terminated{Reason: "Completed", ExitCode: 0}`
for an init container whose status the runtime lost, so *stamp-less ⟺
`ContainerStatusUnknown`* was never true of the kubelet, only of what rule 6
currently reaches.)

The ruling itself is unchanged. The alternative — plant a reason with the stamps
left on — asserts behaviour against
an object the API cannot produce, which is what
[D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12) exists
to refuse in the other direction. **Recorded so nobody later "fixes" the test by
inventing the impossible shape**, and noted as self-closing: if the two shapes
ever diverge, the divergent one separates the keys and the tests start telling
them apart on their own.

**One finding rejected, in writing.** The operator review held that
`killed_action`'s *"whether it stops when asked to"* names something
`kubectl describe pod` does not print — true, there is no such field, and
`Termination Grace Period` renders only while a pod is deleting. **Rejected as a
change:** that clause is a hypothesis for the reader to test, not a field for
them to look up, and the `Killing` event plus the exit code in the card's own
title is the trail it sends them down. The distinction is worth stating because
this area has been asked the question three times: an action may name a *thing to
find out*, and may not name a *thing to find* that the command does not show.
Recorded in `killed_action`'s doc comment so the fourth asking has an answer.

**Two nuances about the kubelet's own source that the review turned up and that
outlive this box.** `convertToAPIContainerStatuses`' first site writes the
**current** `state.terminated`, reaching `lastState` only indirectly; and its
second is gated on `LastTerminationState.Terminated == nil`, so **only a
container's first lost status is ever recorded**. A container that already
carried a `255` and was then removed from the runtime came back with
`restartCount + 1` and the old `255` untouched. `lastState` is therefore not
*"the run before this one"* — it is *"the last run Kubernetes managed to write
down"*, which is a different sentence, and every card in this file says the
first. Boxed, because it reaches rules 1, 5, 6 and `exit_fact` alike.

### D94 — the first review cluster was named `k8rs-review`, and a guard the obvious wrong name walks straight past is not a guard (2026-08-15)

[D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)
was written a day earlier and its central claim was mechanical: the review
cluster runs as **`K8RS_CLUSTER=review`**, "additionally a name
`scripts/sanitize.jq` **refuses**, so a review cluster cannot produce a committed
fixture even by mistake". D92 chose `review` over `k8rs-review` *specifically*
because `refuse_foreign_nodes` keys on the prefix `k8rs-` and would have let the
second one through.

**The first agent to use it typed `k8rs-review`.** Three committed sources said
`review` — CLAUDE.md § The boxes no agent can run, `.claude/agents/k8s-admin.md`,
and the brief it was handed — and the run went up under the one name D92 had
examined and rejected. It measured the consequence itself, in both directions:

```
$ jq -f scripts/sanitize.jq nodes.json          # k8rs-review-control-plane
jq exit=0 — sanitizer ACCEPTED the capture
$ jq -f scripts/sanitize.jq nodes-review.json   # review-control-plane
jq: error: sanitize: node identifiers are not from the kind test cluster … exit 5
```

No artifact was produced and the cluster was torn down, so nothing reached the
repository. **The boundary held because no capture was attempted, not because the
mechanism stopped one.**

**The ruling, and it is against the design and not the agent.** `k8rs-review` is
what anyone types by analogy with `k8rs`, and it is the one wrong name the guard
is blind to. The guard is not useless — it still refuses a capture taken from
somebody's production cluster, which is the threat it was written for — but it
does **none** of the work D92 assigned it, because the single name it lets
through is the single name a reviewer would pick. D92's claim is amended: the
*name* is still `review`, and it is still
refused by the sanitizer, but **choosing a refused name is not the same as
refusing every unrefused one**.

**The first mechanism this entry named was aimed at the wrong door, and the agent
that tripped it said so.** It owed a refusal in `scripts/cluster.sh` — but the
command actually run was `kind create cluster` directly, because a review is one
measurement and not a fixture trip, so the next reviewer will skip that script for
the same reason. **The guard that runs on the only path that matters is
`sanitize.jq` itself**, on every capture, and its blind spot is one loose
predicate: `startswith("k8rs-")` accepts the whole `k8rs-*` family where the
fixture cluster produces exactly four node names. Anchoring it to those names —
keeping the `.lan` suffix the identity rule already allows — refuses
`k8rs-review-control-plane` where the refusal is written down, whoever created the
cluster and however they created it. `cluster.sh` still gets its refusal as the
early, loud one; it must not be the only one. Both are `tester`'s and both are in
one box.

**Reported by the agent that did it**, as a contradiction between committed
files rather than as its own slip — which is how it was caught, and is worth more
than a correct run that reported nothing. The misattribution is recorded because
the sequence matters to the lesson and not because it needs answering.

**The general shape, and it is the third instance in one turn** — three
different agents, three different guards, and no two of them found by the same
pair of eyes. `tester` found the `137` box's own probe guard passing a
capitalised `"Probes"`; `dev-core`, proving that fix, found a line-width cap
going red *first* on a long plant, so a layout constraint briefly looked like a
content guard; and `k8s-admin` found this one, in the mechanism written the day
before to make a promise mechanical
([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
Each was proven for the case its author had in mind and blind to the neighbouring
one. **A guard is tested by the mistake someone would actually make**, which is
rarely the mistake it was written against — and the pattern here is that nobody
finds that mistake in their own guard.

### D95 — the two `137` reasons become endings, and rule 5 draws where rule 6 goes silent (2026-08-15)

[D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)
taught `exit_meaning` two readings of `137` the kubelet writes itself and
deliberately stopped there: rules 1 and 5 printed both translations and their
actions knew neither. So one card said *Kubernetes lost track of the container*
and the card under it said *check the memory limit and the liveness probe*, and
rule 1 sent the reader to `logs --previous` for a run whose `containerID` the
API does not have — a command that cannot run, not merely advice that does not
help.

**The root is `ending()`, and this box's whole design is that the fix is one
function and not three cards.** It read the number alone; it now reads the
reason beside it, and gains `Ending::Unwatched` (`137` + `ContainerStatusUnknown`)
and `Ending::RestartRule` (`137` + `RestartingAllContainers`). **The mechanism is
the compiler**: every `match` on that enum stopped building until rules 1, 5 and
6 each said what the two mean. A `reason` check inside one rule — the obvious
smaller diff — would have left the other two silently wrong, which is exactly how
this pair of defects shipped in the first place.

**The reason is read *beside the code*, never alone, and that is a real
behaviour change on unreachable objects.** Rule 6 used to exempt
`RestartingAllContainers` and re-title `ContainerStatusUnknown` on the reason
whatever the exit code was; now both keys are the pair. Three shapes moved —
`1 + ContainerStatusUnknown` from the *No record* title to the ordinary one, and
`1` or `5` + `RestartingAllContainers` from silent to drawing a card. **Nothing
the kubelet writes is affected**, and this is the one place in the file where
that claim rests on a committed capture rather than on the kubelet's source:
`failed.json` carries the pair on `broken-failed` / `app`. The ruling: a real
exit code means the run *was* watched, so an ordinary reading of it is the
honest one, and no test is invented for a pair the API cannot produce
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).

**`OOMKilled` is deliberately not a variant.** Rule 2 owns the labelled kill,
*something keeps killing it* is true of it, and rules 1 and 5 need nothing new to
say — so it stays `Failed` and rule 6 keeps exempting it by reason. The enum
holds the endings the *other* rules have to tell apart, not every reason the
kubelet writes.

**Rule 5 gets arms and not an exemption. That is the ruling of this box.** Rule 6
goes silent on `RestartingAllContainers` because *the previous run failed* is its
whole subject and the ending refuses it. Rule 5's subject is the **count**, which
is real under both reasons — and silence would blank the pod: one restart-rule
firing writes the same synthesized record into *every* container's `lastState`,
the trigger's included, so a rule 5 exemption on top of rule 6's would leave a
pod thrashing 31 times in six minutes with nothing on the screen at all. What
goes is the claim and the action, not the card: *"something keeps killing it"* is
a positive claim of repeated killing on a run nothing is recorded as having
killed, and `failed_action`'s memory limit and liveness probe are doors onto a
kill under an evidence line saying no kill was seen.

**Two shared sentences, not five copies** — `unwatched_action` and
`restart_rule_action`, beside the four `finished_action` / `stopped_action` /
`failed_action` / `killed_action` already there. Both are **role-blind**, which is
also what keeps them clear of `validateInitContainers`: a sentence that names no
probe cannot name one an init container may not have, so the split the other four
make has nothing to split on. `unwatched_action` is rule 6's shipped text
unchanged, now called by three rules instead of one.

**`restart_rule_action` may not say *this container is fine*, and that is the
whole of its wording.** The record lands on every container including the one
whose own exit triggered the rule — its own bad exit is in `state.terminated`,
which no rule reads — so *look at the other containers* would be wrong on exactly
the container that failed. What the object supports is that it does not say which
one went first; **where the card sends the reader instead is the operator
review's finding below**, and the first draft got it wrong by naming a thing
`describe` does not show. The sentence ends on *and that may be this container*
by design, and a guard now holds it there — a hedge appended after that clause
takes it back, which is how the second attack broke it.

**Rule 1's `RestartRule` arm is written and is barely reachable, and it says so.**
The restart-all path purges every container from the runtime, so `doBackOff`
finds no exited record, no backoff entry is made and `CrashLoopBackOff` does not
appear — measured at about one restart every 11s behind an 8s sleep, which is no
backoff at all. The arm exists because the enum forces an answer; the shape it is
tested on is planted, and a planted shape is not a reachable one
([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).

**D93's unobservable branch is observable now, by the route D93 predicted and not
the one it named.** That entry accepted that a title keyed on the null `finishedAt`
could not be told from one keyed on the reason, and recorded that the day rule 6's
exemption narrowed, the keys would diverge. The exemption did not move — rule
**5** stopped exempting instead, and because it draws a different clause for each
variant, a stamp-keyed `ending()` now prints *no ending on record* over a run
whose record is complete. The tests tell the two apart on the first object.

**The attack found two holes the author's own green run could not**, and both are
the same class one level apart:

- **A negative-only assertion proves the card does not lie, not that it says
  anything.** Replacing `unwatched_action()` at rules 1's and 5's call sites with
  the literal `"ask a friend"` passed 184 tests: the test forbade every kill word,
  every log pointer and every probe, and `"ask a friend"` says none of them. Its
  `RestartRule` sibling caught the same mutation immediately, because it asserts
  what the sentence *contains*. **Every negative test in this file owes one
  positive line beside it**, and the asymmetry between two tests written in one
  sitting is how far that goes without being noticed.
- **A constant every test plants from is pinned by nothing.** `STATUS_LOST`
  misspelled as `"containerstatusunknown"` and `RESTART_ALL` as
  `"RESTARTINGALLCONTAINERS"` both shipped 184 green, because every shape in the
  suite writes the reason *out of the same constant it is then matched against* —
  a rule that never fires against a real cluster, with a suite that is green about
  it. `write-guard.py`'s `CANARIES` class one level down. The two are pinned by
  different evidence and the difference is stated rather than blurred:
  `STATUS_LOST` is read off `failed.json`'s captured bytes, spelling and `137`
  together; `RESTART_ALL` has no capture and is pinned to the literal against
  `kubelet_pods.go` at v1.36.1, **a source-derived pin and not a captured one**.

**The operator review took a cluster to it and the first draft did not survive
contact.** Everything below was measured on kind v1.36.1, and it is why this entry
has a second half.

**The blocker: the new action named a thing to find that the pod does not
contain.** *Check the containers in the pod and look for the one with an exit code
of its own* fails on three measured shapes — on a settled pod every container
prints `Exit Code: 137` and the trigger's own `exit 3` is gone; on a thrashing pod
it was visible in **12 of 40** one-second samples; and a **single-container** pod
declaring `action: RestartAllContainers` gets the identical record and was being
told to compare it with siblings it does not have. That is `killed_action`'s own
rule broken one function over: an action may name a *thing to find out*, never a
*thing to find* that its command does not show.

**The fix trades the command, which is the second time this area has paid that
price** ([D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
`describe` cannot name the trigger by any route that survives measurement, so the
`RestartRule` arm of rules 1 and 5 carries `get_yaml` instead and names the one
field that can: `restartPolicyRules` is declared on the container(s) that can set
a gang restart off, it is in `get -o yaml` and in no part of `describe`, and on a
one-container pod it resolves to that container. The card keeps the denial — *this
record does not say which container exited* — and its closing clause keeps **this**
container in the frame, because the trigger carries the same record as everyone
else and a card that exonerated it would be wrong on exactly the container that
failed.

**What was given up, both measured rather than assumed.** `describe`'s events do
name the killed containers, and the kubelet's spam filter eats them: `x2` and `x3`
recorded against **130** real restarts, expiring in an hour. And the pod's
`AllContainersRestarting` condition — the field that looked like the answer —
**is a transient, not a state**: `True` in 7 of 40 one-second samples on the
thrashing two-container pod, one per kill-and-recreate window, and on the
single-container pod `False` in 71 of 71 samples at 5 Hz while it restarted six
times. Only the *presence* of the row is stable, and presence says no more than
the card's evidence line already says.

**The general lesson, and it is the sharpest one in this entry: two point samples
of a transient are an inference wearing a measurement's clothes.** The reviewer
filed *the condition is `True` on the thrashing pod and `False` on the settled
one* off two samples, recommended a card be pointed at it, and then — asked to
confirm one unrelated detail about the same field — sampled it properly and killed
its own recommendation. **That is the same defect it had just filed as finding
1**, one field over, by the same author, inside one review. A field read once is
not a field measured; it is a field caught at a moment. The PM had already written
the wrong claim into two `todo.md` boxes on the strength of it.

**Three more findings the review measured, all now closed in the code:** rule 5's
`Unwatched` serving title made an **11-line card** at a three-digit restart count
against `screens/alerts.md`'s measured maximum of ten — both claims were reworded
to the `Failed` arm's 32-character budget and the four cards this box ships are
measured at exactly 10 by a new test, off the cards `analyze` really draws rather
than off copies of the strings. Rule 1's `Unwatched` arm turned out to be **as
unproven as its `RestartRule` neighbour** — never produced in ~20 attempts, since
the kubelet's synthesized write is gated on `LastTerminationState.Terminated ==
nil` and a container that earned a backoff necessarily has one — and it now
carries the same caveat instead of being presented as a field fix. And
`ending()`'s premise, *a real exit code means the run was watched*, was **false**:
`kubelet_pods.go:2714-2718` synthesizes `Completed` / `0` for an init container
whose status the runtime lost, which `ending` reads as `Finished`, `doing_its_job`
then reads as *this container is fine*, and rules 5 and 6 both stand down — k8rs
silent on a lost run, which is this box's own defect one literal over. The premise
is narrowed to the two reasons this file has evidence for; the third literal is
boxed.

**Two mechanism findings worth more than the wording ones.** `doing_its_job` read
`ending(run) == Ending::Finished`, so **the compiler did not force it** to answer
for the new variants — it classified both silently, and this entry's central claim
was true of three call sites out of four. It is a `match` now, and adding a
throwaway variant stops the build at four places. And the tests never fed the shape
the cluster actually writes: every plant put the record on **one** container where
the kubelet puts it on **all** of them, so the fan-out described in prose was
asserted nowhere. The two-container plant is in.

**The guard for that blocker had to be written twice, and the second break was in
the assertion's *shape* rather than in what it asserted.** The first version
pinned the new sentence with three `contains` fragments; appending eight words —
*…and that may be this container, **but rarely*** — keeps all three and reverses
the sentence, and the suite stayed green. A longer exonerating variant did go red,
but **only in the height test**, for being five action lines rather than for
lying: inside an action's remaining slack the wording guard was blind, and that
action had 32 characters of it. **A `contains` fragment cannot see what comes
after it** — the sentence it pins can be taken back by the next clause, which is
the same framing hole [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
named for substrings and [D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)
named for letter-case, in the guard written to close the finding that came from
both. What holds it now: every card's action compared to the function by value,
the sentence required to *end* on the clause that keeps this container in frame,
and an `EXONERATES` list run over title, evidence and action alike — because a
title exonerates as easily as an action, which is the path the author's own second
pass added rather than the reviewer's. **Its control is synthetic and says so**:
these are words the rule set may never say, so a control drawn from the product
would mean the defect had already shipped; what is proved is that the detector
fires, on every phrase and on the exact mutation.

**Three smaller things the same pass found, worth recording because each is a way
a green suite lies.** Two commands were unpinned — moving rule 1's and rule 5's
`Unwatched` arms from `describe` to `get_yaml` stayed green, on the two arms whose
sentence hedges *the events rarely say so* about a command that prints no events;
they are pinned by value now. A comment justifying a constant was **false and
would have become lore**: the height test's three-digit restart count was
explained as what makes the card overflow, and the card measures ten lines at 7,
132, 1320 and 999,999,999 alike — the count sits on a line with slack and the
*clause* is what decides, so the constant is now a list the test loops over and
the comment says what is true. And a test's **name** claimed what it did not feed:
widening `ending()`'s two arms from `137` to any code stays green, because the
unreachable pairs are deliberately unasserted — the gap stays, by the same refusal
as above, and the name went instead. A name is evidence to the next reader.

**What this box did not close, recorded so the next reader does not read silence
as completeness.** The **fan-out** is now six *truthful* cards instead of six wrong
ones; which container exited first needs `state.terminated`, and *whether the cards
should be drawn at all* has **no measured field behind it** — the condition above
was that field until it was sampled. A pod that gang-restarted three times and has
served ever since carries a permanent WARN card per container, which is D71's class
on a record that carries no stamps either, so no clock can age it out. Rules **5**
and 6 print the identical four-line action on adjacent cards — 26 lines about one
container in a 16-row pane — and the reachable pair is that one, not the rules 1
and 6 pair this entry named in its first draft, which needs a shape the review
could not produce. Silence is not the fix (D93 refused it); knowing that a
neighbour already said it is an `analyze` decision, beside `explains_a_shortfall`.
All three are boxed, with their measurements.

### D96 — the run a container is sitting in is no rule's subject, and the one reader may only suppress (2026-08-15)

`state.terminated` — the run a container is stopped in **right now**, as opposed
to `lastState`, the run before this one — is read by exactly one function in
`rules.rs` and draws no card at all. The box asked whether that is a decision or
an omission. **It is a decision**, and the reader that exists is a *suppressor*:
`doing_its_job` asks `ending()` whether an **init** container finished, and
answering yes takes rules 2, 5 and 6 away. Nothing may put a sentence on the
screen from this field, change one, or date one.

**Leg 1 — a pod that is over is already gone from this screen, and it lands
nowhere else yet.** `analyze` skips every pod rule except rule 12 when the phase
is `Succeeded` or `Failed`: `stuck_terminating` runs *before* the `finished(pod)`
gate, and that ordering is load-bearing rather than incidental — a pod held by a
finalizer after it completed is squarely rule 12's subject and would be invisible
from inside the gate. **`analyze`'s own doc said so all along** — *"Rule 12 is
deliberately outside the skip: a `Succeeded` pod that will not go away is still
stuck"* — and the leg was still written as *every pod rule*, by a PM reading the
code, in a brief a dev built prose from, and it took an operator review with a
cluster to catch it. A doc comment three lines above the loop is not a place
anybody looked. Rule 12's own doc now carries it too, where a reader of the rule
will meet it. That covers the *stable* majority: a
single-container pod whose container dies under `Never` or `OnFailure` goes
terminal and leaves. **The first draft of this leg said those pods "belong to the
Waste report", and that is a promise rather than a destination**: `analysis.rs`
does not exist, the Waste report's charter is Evicted/Completed *pileups* rather
than a diagnosis of a Job pod that died a minute ago, and Jobs are not watched at
all (invariant 6). So the honest sentence is that such a pod leaves the Alerts
screen by [D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s
rule and is visible on no k8rs screen today. The ruling holds either way; the
claim is smaller than it was written.

**Leg 2 — this field's normal state is a healthy object.** Every container any
committed capture holds in `state.terminated` inside a pod that is *not* over
**ended without an error, whatever its role** — swept over the whole corpus
rather than asserted about the files someone remembered. A reader keyed on this
field starts from a haystack of healthy objects, which is why the one reader
there is asks only the init question.

**That leg was written as *a finished init container* and the next box falsified
it the same day** — deliberately, by capturing the object it describes. `neverback`
holds two **Regular** containers in this state, one at `exit 1` and one at `exit 0`,
so *init* was a property of the two captures that happened to exist rather than of
the field; what the ruling actually rests on is that **nothing in this state is a
fault unless somebody built one**. The guard now exempts `neverback/broke` by
**exact `pod/container` name** rather than by a class like *any Regular container
that draws a card* — a second capture landing in this state reddens it exactly as
the first one did, which is the only reason narrowing it is not weakening it. The
general lesson is worth more than the correction: **a claim about the corpus has a
shelf life tied to the corpus**, and the thing that tells you it expired is the
test carrying it, not the entry
([D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)).

**Leg 3 — the reason is redundancy, not transience, and the first draft had the
weaker half.** *A transient a watch will see and `--once` may not* is measurably
false of a backing-off container: `state.terminated exit 3` was the visible state
across tens of seconds while kubectl's own STATUS column read `Error`. What
survives measurement is that **the same record is in `lastState` in the same
snapshot** — `state.terminated {exit 3}` and `lastState {exit 3, Error}` observed
together, with rule 6 already firing off the second copy and rule 1's card
following from the backoff. So refusing the current terminated state **loses
nothing about any container that comes back**, not merely earliness. And the
obvious reply — *then debounce it* — is answered by
[invariant 5](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision): a pure `analyze(&Snapshot)` has nowhere to hold *I
saw an exit 3 four seconds ago*, so a card drawn from this field would be a
function of when the sampler happened to look, permanently, by construction.

**Leg 4 — the one exception, and it is the cost this ruling accepts.** On the
gang-restart loop the trigger's own `exit 3` never reaches `lastState`: **0 of 80
samples across two clusters and two manifests**, while the synthesized `137` was
in `lastState` in every one. It is visible in `state.terminated` in 10–30% of
samples — the ratio of restart latency to container lifetime, 4 of 40 on a
20-second container and 12 of 40 on a shorter one, which is why the figure is a
range and not a property of the feature. So that container is **never nameable by
any rule**, and rule 5's card keeps its denial that the record says which one went
first. Three other candidates for *the current state is the only record there will
ever be* were hunted and none of them is one: a pod being deleted publishes no
per-container `terminated` at all during the grace window; a container removed
from the runtime under `Never` is **restarted anyway** and its synthesized record
lands in `lastState`; eviction and graceful node shutdown set the phase to
`Failed` and leave through leg 1's door.

**Would the flicker be worth it at 3am?** No — and the reviewer who watched the
pod thrash is the one who answered. Rule 5 is already drawing on that pod and its
action already says the record does not name the container that exited and where
the rule that did it is declared; what a `state.terminated` card would add is one
container name that is right 10% of the time and absent 90%, on a screen where
absence means *not broken*. **Two things would reopen it**: lowering rule 5's
restart band, since this argument depends on rule 5 covering the loud case; and
the kubelet ever keeping the trigger's own record somewhere durable, which it does
not today.

**What the ruling does not cover, and the correction that scoped it.** A
container that **cannot come back** inside a pod that is **not** over is permanent
rather than transient, and no rule reads it: measured, two such pods sat at
`1/2 Error`, `Ready: False`, `phase: Running` for fourteen minutes with
`restartCount: 0` and an empty `lastState`, with every k8rs rule silent — while
`kubectl get pods`, the tool the reader already has open, printed `Error` in its
STATUS column. That is [D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s
own argument about teaching the reader to trust the other tool, pointing the other
way. It is boxed rather than built, and **the truth table it was first scoped on
does not survive v1.36.1**:

- `ContainerRestartRules` is **beta and on by default**, so a **regular**
  container may override the pod — a container declaring `restartPolicy: Never`
  under an `Always` pod stayed dead at `restartCount: 0` after fourteen minutes.
  *Always restarts everything* is a 1.28 sentence.
- **The rules can only add restarts.** The API rejects a `DoNotRestart` action
  outright — `supported values: "Restart", "RestartAllContainers"` — which is what
  makes *will this container come back* decidable at all.
- There is **no `pod.spec.restartPolicyRules`**; the field is
  `spec.containers[].restartPolicyRules`, per container, with a pod-wide effect
  only when the action is `RestartAllContainers`. Doc comments in `rules.rs` said
  otherwise and were corrected; no user-visible string was wrong, because the card
  names the field it means.
- So the condition is `container.restartPolicy ?? pod.restartPolicy`, with any
  matching rule overriding it upward — and a rule that reads the policy **and not
  the rules beside it** ships the KEP's headline use case as a false positive:
  measured, a pod `Never` / container `Never` with one retry rule on exit `3` was
  in `CrashLoopBackOff` at five restarts, which a policy-only reader would have
  called *stopped for good*.

**The Waste-report promise had four readers and only two of them were found by
looking.** Retiring one sentence meant retiring it in `analyze`'s doc,
`finished`'s doc, a comment in the whole-capture test — and, the one that
matters, **the assert message of the test that proves the skip**. That last one
is the sentence a *failing* run prints, so the stale promise would have been the
last thing a reader saw before going to look for a report that does not exist. A
claim removed from a doc comment is not removed from the build until the
assertions that repeat it are checked too; the sweep that found the other two was
`grep` across `.rs` **and** `.md`, and it is the step that turns "I fixed the
doc" into "the claim is gone".

**A process note, second instance.** The reviewer again ran its cluster as
`k8rs-review` where the brief said `review` — and this time the cluster really
came up under that name, whose nodes
[D94](#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)
proved `sanitize.jq` waves through. Nothing leaked, because a review takes no
captures — but the refusal was defeated in practice for the second time in two
days, by the second agent to touch it, which is the whole argument for anchoring
the guard rather than repeating the name. Also worth stating precisely, since
`CLAUDE.md` blurs it: the sanitizer refuses a **node-name prefix**, not a cluster
name; `review` is refused because kind names its node `review-control-plane`.

**And one the PM broke itself, one box after writing the rule.** The concurrency
paragraph added in the previous box says the gate is not split by tree; what it
did not say is that **resuming a finished agent is a new dispatch**. `dev-core`
had reported, `tester` was dispatched to attack the same two files, and then
`dev-core` was resumed for a six-line citation fix — two writers on one tree, and
`tester`'s method is *copy, mutate, restore from copy*, so a restore taken before
those edits would have reverted them with both agents reporting green. It did not
happen: the citations were still in the tree when checked, and the holder was told
what had landed under it. The general shape is the one this file keeps finding —
**a rule is broken first by the person who just wrote it**, because they are the
one acting on the assumption it was written to correct.

### D97 — a container that cannot come back gets rule 15, and a restart count stands in for a field the pinned types cannot see (2026-08-15)

[D96](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)
refused the run a container is sitting in **as a transient** and named one
exception on its way out: a container that has stopped and **cannot come back**,
inside a pod that is not over, is permanent, and no rule read it. This is that
rule.

**The shape, measured on the fixture cluster before a line was written.** A pod at
`phase: Running` with `restartPolicy: Never`, one container terminated `exit 1` at
`restartCount: 0`, a second terminated `exit 0`, a third still running.
`kubectl get pods` prints `1/3 Error`. Every rule in `rules.rs` was silent on it —
rule 5 needs a restart count that `Never` can never move, rule 6 needs a
`lastState` a container that never restarted has never had, rule 7 needs
`Running`, rule 13 stands down while anything in the pod runs.

**The truth table collapses to one arm, which is why the rule is four conditions
and no branches.** On a *bad* exit, `Always` restarts it and `OnFailure` restarts
it; only `Never` does not. So rule 15 fires on `ContainerState::Terminated` +
`Ending::Failed` + `restarts == 0` + effective policy `Never`, and CRITICAL,
because [D2](#d2--the-dividing-line-broken-now-vs-risky-later) asks whether this
container is serving and the answer will not change.

**The policy that decides is the container's, not the pod's** — measured, and it
is the leg that would have been silently wrong. Under `ContainerRestartRules`
(beta, on by default at v1.36.1) a **regular** container may declare
`restartPolicy: Never` inside an `Always` pod: the one I ran sat at
`restartCount: 0`, `phase: Running`, `1/2 Error` for as long as its sandbox
lived. A rule keyed on the pod's policy misses it entirely.

**"For as long as its sandbox lived" is the correction the review made to this
paragraph, and it is not a hedge.** The first draft said *indefinitely*. Measured:
reboot the node and that container **comes back**, `restartCount` 0 → 1 → 4,
because the kubelet reads the **pod's** policy when it recreates a sandbox and
`Always` there restarts everything, override or not. The pod-level `Never` shape
behaves the other way and honestly — the pod goes `phase: Failed` and leaves
through `finished(pod)`, card and all. So the two shapes this rule folds together
have different futures, and the card may not make an absolute claim about either:
what is true of both is that nothing is *waiting* to run the container again, and
that replacing the pod is what starts it over. So the snapshot carries the **effective**
policy — the container's own where it has one, the pod's otherwise — derived at
decode beside `ContainerRole`, which is derived from the same field.

**Two shapes cannot reach this rule, and that is by construction rather than by a
role check** — both measured: an init container failing under pod `Never` takes
the pod to `phase: Failed`, so it leaves through `finished(pod)`; and a native
sidecar is `restartPolicy: Always` by definition, so its effective policy is never
`Never` — the one I ran restarted twice and the pod stayed `Running`. Only
`Regular` containers arrive, so the rule needs no role split and names no probe.

**The count guard is a stand-in for a field the build cannot read, and that is
worth stating plainly.** `restartPolicyRules` can only *add* restarts — the API
rejects a `DoNotRestart` action outright — so a container whose rules match its
exit code comes back even under `Never`: one measured pod sat in
`CrashLoopBackOff` at five restarts. **`Cargo.toml` pins `k8s-openapi` at feature
`v1_32` and that field arrives in the generated types at `v1_34`**, so it decodes
to nothing today. `restarts == 0` is the proxy: something that has restarted is
not something that will not restart. **The residual gap is one window** — the
first exit, before the first retry — and it is in the rule's doc rather than in a
footnote. The pin itself is a box of its own: the crate already ships `v1_32`
through `v1_36` in the version `Cargo.lock` holds, so it is a feature flag rather
than a dependency, and every fixture in the repo is captured from **v1.36.1**
while being decoded by 1.32 types. Serde drops what it does not know, so four
versions of fields read exactly like fields the cluster did not set.

**The card's command is the first `kubectl logs` in the file, and it was measured
before it was written.** `kubectl logs <pod> -c <container>` on the terminated
container returned its own last line, with no `--previous` and no error, because
the pod still exists and the log is still there — still true 46 minutes later,
across a kubelet restart, and across a full node reboot, since the kubelet's
container GC keeps the one dead instance.

**And the card said *it is still there*, which was one measurement generalised to
every state — the exact move
[D88](#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)
exists to refuse.** `kubectl logs` is the **only** command any card offers that
goes to the kubelet on the node rather than to the API server, and this rule's
four conditions are read from a pod status that **freezes when that kubelet
dies**. Measured on a review cluster: kubelet stopped, and eight minutes later the
card still drew — `Terminated`, `Failed`, `restarts == 0`, effective `Never`,
`phase: Running`, every field unchanged — while the command beside it answered
`dial tcp …:10250: connect: connection refused`; at five minutes the pod picked up
a `deletionTimestamp` it could not complete, so rule 12 drew an honest card beside
rule 15's dead corridor. **The rule is most likely to be firing exactly when its
command is least likely to work.** The clause came out; the command stayed. What
replaced the promise is the container's own last words from
`state.terminated.message`, which is the one fact that survives that state,
because it is in the API server and the log is not.

**What the card says instead, and what it cost.** The action ends on the reader's
next move rather than on a prediction — *nothing is waiting to start it again, so
the pod has to be replaced; until it is, whatever needed this container is still
without it* — which is the *what to do* the first draft never had: its third part
described what would **not** happen. The container's own last words go on the card
ahead of the duration, because the evidence is cut at three lines and something
had to give: the message is in the API server and the log is on the node, so on
the very shape that made this a blocker it is the only clause that still answers.
**The card is at ten lines with no slack in any shape that carries a message** —
the title and the action are constants and the evidence is cut, so it is bounded
rather than merely measured, but a word added anywhere overflows it.

**The clean-exit half is silent, and the fixture carries it.** A container that
exits `0` and is not restarted is doing exactly what `Never` means. Calling that a
fault needs the Job above the pod — *the Job never completes because the helper is
still running* — and Jobs are not watched (invariant 6). The captured pod holds
such a container on purpose, so the silence is asserted against a real object
rather than assumed.

**The capture is one fixture, not a re-run of all fifty, and that is a decision.**
`just fixtures` rebuilds the whole corpus from a fresh cluster; running it to add
one pod would rewrite fifty committed files with new UIDs and timestamps and put
every test that reads a captured value at risk, for one new object. So the pod
went into `scripts/broken.yaml`, `cluster.sh verify` learned its predicate and the
recipe learned its capture and its guard — **the recipe reproduces it on the next
full run** — and the single file was taken with the recipe's own filter,
`sanitize.jq`, on the cluster `cluster.sh verify` had passed. A single-fixture
capture is a real capture; what would make it a hand-written one is skipping any
of those three.

**What it cost, which the first draft of this paragraph left out.** The new
fixture is dated 2026-08-15 where the corpus is 2026-08-13, so the pinned test
clock — `fn now()`, which sat between them — was **before** a capture it is read
against, and the card this rule was written for drew with no age at all. Editing
the capture's timestamp is what [D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)
forbids, so **the clock follows the corpus**: `now()` moved to
`2026-08-16T00:00:00Z`, and with it eight age assertions (`28 min ago` and
`1 hour ago` alike became `2 days ago`, the cordon's guard 24 → 2904 minutes) and
the three certificates' day counts, in `certs-test.sh` and `make-certs.sh`. No
assertion was weakened — only expected values moved, each witnessed red at the old
number and green at the new one. **A single capture taken on a new day carries a
repin with it**, and the next person taking one should expect that rather than
discover it; the cordon fixture now sits 24 minutes past `age()`'s 48-hour rung,
so a trip finishing later in the day flips its phrase, which the test says out
loud with the arithmetic in its failure message.

**A boundary in [D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)
the PM crossed deliberately, recorded rather than glossed.** That entry gives
ephemeral measurement to `k8s-admin` and nobody else; the three shapes above were
measured by the **PM**, on the fixture cluster, in a scratch namespace deleted
afterwards. The reason is the one D93's first draft failed on: the person writing
the ruling has to be able to defend it, and a design key defended by hearsay is
how *the pod's own restart rule* got written about a field that lives on a
container. The amendment is narrow and it is the whole licence: **the PM may
measure what a decision it is about to write turns on**, in its own namespace, on
no `demo=broken` object, deleted after, and reported like any other measurement.
Anything that tunes code until a cluster agrees is still `k8s-admin`'s.

### D98 — the user leaves the room, and the PM stops asking (2026-08-15)

The standing instruction, given after the rule-15 box closed: *manage the
project — brief the agents, run the checks, make the final calls; develop it end
to end, and do not wait for my "continue" or my approval.* Recorded rather than
remembered, because a standing authorisation that lives only in a chat log is one
the next context window cannot honour.

**What changes is one thing: the pause between boxes.** Until now a box ended at
the push and the next one waited to be picked. It no longer does — the PM reads
the *first unchecked box in the lowest open phase*, briefs it, runs the cycle,
lands it, and starts the next one in the same breath. The choice of box was never
the user's anyway; [`todo.md`](todo.md) has decided it since Phase 0, and asking
was a courtesy, not a gate.

**What does not change is everything else, and that is the point.** Every step of
the cycle, the security gate, the second pass over the landed tree, the phase
close ritual, forward-only, one writer per file tree. A gate exists to stop *the
PM* as much as an agent, and the party it was protecting the work from has just
left. So none of them is negotiable now, and the reflex to check "is this worth
bothering the user with" is replaced by "is this written down where the next
reader will find it".

**Three things still stop the run, and none of them is approval.** A box needing
a credential or an account nobody here holds — the crates.io publish, GitHub
repo settings, anything behind a login — still stops: the PM prints the exact
command and the box stays **open**, because [§ The boxes no agent can
run](CLAUDE.md) is about fabrication and fabrication got easier today, not
harder. A red build stops, and is never negotiated down to get past a gate. And a
reversal of something the user decided is still a *decision*, written here before
it is acted on — autonomy is the licence to choose the next box, not to rewrite
the design.

**The cost, stated plainly.** The user was the last reader outside the loop, and
the leak list in CLAUDE.md § *Where a leak would actually happen* was written for
a PM who could be caught. Nobody catches it now. The compensation is that reports
are written for a reader who was not there and will check: commands and their
real output, the red run and the green one, every choice the brief did not
decide. A summary that cannot be audited from the repo alone is the failure mode
this entry creates, and naming it is the only guard available.

### D99 — the pin follows the newest types, and the old rule was self-violating from the first capture (2026-08-15)

`Cargo.toml` pinned `k8s-openapi` at feature **`v1_32`**, the *oldest* the crate
offers, on a rule written in Phase 0 and repeated in **four** files:
[`docs/tech-stack.md`](docs/tech-stack.md) — *"pinned to the oldest feature still
offered … the API is forward compatible, so an old pin talks to newer clusters.
Support window = pinned ±2 minor"* — [`REQUIREMENTS.md`](REQUIREMENTS.md),
[`PRIOR-ART.md`](PRIOR-ART.md), which records k9s doing the same, and
[`docs/architecture.md`](docs/architecture.md) § Version compatibility. **The
sentence is true of the wire and false of the diagnosis**, and this entry
reverses it.

*Four, because the first draft of this paragraph said three and the operator
review found the fourth.* It is worth a sentence rather than a silent correction:
`docs/architecture.md` is the file CLAUDE.md describes as *never containing
anything not yet true of the code*, and leaving it would have handed the next
contributor the ±2 window as a live instruction — under which the honest repair
is to re-capture the fixtures against a kind **1.34** image, which the new guard
cheerfully passes. A doc that is wrong in the same direction as a guard's blind
spot is worse than either alone.

**The two failure modes are not symmetric, and only one of them is visible.**

| | what happens | what the tool shows |
|---|---|---|
| pin **older** than the cluster | serde drops the unknown field at decode | nothing — the field reads exactly like one the cluster did not set. *Found none* and *there were none* are the same silence, one layer under the fixture guards that exist for it |
| pin **newer** than the cluster | the field is absent from the object | `None` → invariant 5's defined behaviour: *a missing field means no finding*. Every rule already handles it, because every rule already handles a field the cluster did not set |

Forward compatibility was never at risk in either direction — reads work both
ways. What the old pin bought was a guarantee nobody needed; what it cost was
the tool's own subject matter.

**`None` is the right half of that table for an added *optional* field, and the
operator review found the two places it is not** — neither reachable from k8rs's
surface today, both recorded because this entry is what the next box adding a
typed kind will read:

- **A required field does not become `None`, it becomes `Default`.** k8s-openapi
  decodes non-`Option` fields with `unwrap_or_default()`, not a missing-field
  error, so an absent required field reads `""` / `0` / `false` — **a value, not
  an absence** — and invariant 5's *a missing field means no finding* does not
  cover it: the rule sees a confident wrong answer. `ContainerStatus::image`,
  `ready` and `restart_count` are all this shape. Eleven fields flipped optional
  → required crate-wide inside this window, so the mechanism is live, and the
  struct this entry's own follow-up box will read has one
  (`ContainerRestartRule.action` is a bare `String`). **The lesson generalises past the pin**: a snapshot field
  read off a required field can never distinguish *false* from *not there*, and
  the check belongs where the field is pruned in.
- **Twelve resource types changed group/version in the window** — `SelfSubjectReview`
  `v1beta1`→`v1`, `ValidatingAdmissionPolicy` `v1beta1`→`v1`, the `resource.k8s.io`
  claim types `v1beta1`→`v1beta2`, and so on. Against an older apiserver that is a
  **404, not a `None`**; confirmed live (`/apis/authentication.k8s.io/v1beta1` →
  `HTTP 404` on a cluster whose only version is `v1`). None of the twelve is in
  k8rs's surface: `may_i` uses `authorization.k8s.io/v1`, the C-series uses
  `certificates.k8s.io/v1`, and the browser derives its group/version from
  discovery rather than a constant (invariant 12) — which is one more thing that
  invariant buys.

**The rule was self-violating before its first capture.** `v1_32 ± 2 minor` is
`1.30 … 1.34`. Every fixture in `tests/fixtures` was captured from kind
**v1.36.1**, two minors above the stated ceiling, from the first capture trip
onward. And `scripts/verify-test.sh` records its field names as *"cross-checked
against … the k8s-openapi **v1_36** generated types"* — a check performed against
types the build did not have. The drift had already produced a false document; it
took rule 15 needing a field to make anyone look.

**What was actually being dropped, measured rather than assumed.** Field-name
sets diffed between `v1_32` and `v1_36` of the generated types, over exactly the
structs [`rules.rs`](src/rules.rs) decodes (`PodSpec`, `PodStatus`,
`PodCondition`, `Container`, `ContainerStatus`, `ContainerState*`, `Node*`,
`ResourceRequirements`, `ObjectMeta`, the four workload `*Status` and
`*Condition` types) — every file asserted present in both trees first, because a
sweep that silently skips a path prints the same empty list as a clean one:

- **12 fields added, 0 removed** — *at that scope*, which is the named struct
  list and not the transitive closure. The operator review walked everything
  reachable from the six watched kinds instead and found **20**, the extra seven
  being `NodeSystemInfo.swap`, `EnvVarSource.fileKeyRef`,
  `VolumeMountStatus.volumeStatus`, `ResourceHealth.message`,
  `VolumeProjection.podCertificate`, `EphemeralContainer.restartPolicyRules` and
  `Lifecycle.stopSignal`. All optional, none read anywhere in `src/`; the only one
  sitting in a struct the rules genuinely decode is `NodeSystemInfo.swap`, beside
  the `kubeletVersion` N4 already reads. **0 removed is confirmed twice**, at both
  scopes — which is why the bump compiles untouched and all 200 tests pass on it.
  The two numbers are kept rather than reconciled: the difference between them
  *is* the answer to "what counts as a field k8rs decodes", and it is the
  transitive one.
- Three of them can change a rule's answer, and each becomes its own box:
  `container.restartPolicyRules` (v1_34 — the field rule 15 works around),
  `deploymentStatus.terminatingReplicas` and `replicaSetStatus.terminatingReplicas`
  (a pod on its way out is not a pod that is missing), and
  `podStatus.resources` / `allocatedResources` (in-place resize — what a
  container *has* stops being what its spec *asks for*).
- The rest are inert here: `hostnameOverride`, `schedulingGroup`,
  `observedGeneration` on pod and condition, `stopSignal`, `declaredFeatures`,
  and the extended-resource-claim statuses.

**The new rule: pin the newest feature the crate offers, and assert it against
the cluster the fixtures came from.** The guard compares the feature in
`Cargo.toml` with `tests/fixtures/K8S_VERSION` and is an **inequality** — pin ≥
cluster — not an equality, so the crate may move ahead of the kind image without
turning `just check` red for nothing. The justfile has promised this since Phase
2 (*"when the k8s-openapi feature is bumped, re-capture against a matching kind
version — the K8S_VERSION stamp is what makes that drift visible"*) and **nothing
compared the two**, which is the whole reason the pin sat wrong for a phase.

**What the inequality costs, named rather than left to be discovered.** A pin
above the cluster puts fields in the types that no fixture can contain, so a rule
could read one and be untestable — the class NOTES already carries as the
unreachable `.status.resize` branch. No new gate is added for it: *positive and
negative fixture, both* already refuses such a rule, because a positive fixture
for a field the capture cluster cannot produce cannot be written. The existing
gate is the guard.

**And the failure this entry calls unaffordable is not eliminated — it is
relocated, onto the user's machine.** Before: pin 1.32, fixtures 1.36, so the
drift lived in this repo, where the new guard now catches it. After: pin = newest
offered = the kind image, so the guard is satisfied by construction and can only
fire in the window between kind shipping a minor and k8s-openapi shipping it. The
instance that remains is **the user's cluster newer than our pin** — everyone on
1.37 the day 1.37 ships — dropping its added fields exactly as the old pin
dropped 1.36's, and `fixture-audit.sh` structurally cannot see it: it compares the
pin against a stamp taken from the pinned image, never against anybody's cluster.
So the connect-time check `todo.md` already carries for the *floor* owes a second
sentence for the *ceiling*, and that box now says so. Naming this is the price of
the reversal being honest rather than merely correct: the argument that justifies
it is the same argument that indicts what is left.

**Writes are not a new risk, and the first draft of this paragraph gave the
wrong reason for it** (Phase 7, unbuilt). The conclusion stands: `ops.rs` sets
replicas, `unschedulable`, annotations and eviction, all far older than this
window in every direction. The reason it gave — *"every mutation is preceded by a
server-side `dryRun=All`, which is where an unknown field would be rejected
loudly by an older apiserver"* — **is measurably false**, and it is the sentence
Phase 7 would have cited as its licence not to check. Measured by the operator
review on kind v1.36.1 through `kubectl proxy`, sending what kube-rs sends (an
apiserver cannot tell a field that is unknown because it is *new* from one that
is unknown because it is *invented*, so this is the same code path a v1.30 server
takes):

| call, with `dryRun=All` | answer |
|---|---|
| merge patch on a Deployment with an unknown field | **`200 OK`**, `Warning: 299 - "unknown field …"`, field silently absent from the result |
| merge patch on the `scale` subresource — what `Api::patch_scale` uses | **`200 OK`**, same warning |
| `POST …/eviction` with an unknown field | **`201 Created`**, same warning |
| any of the above **plus `fieldValidation=Strict`** | `422` / `400` — rejected |
| server-side apply (`application/apply-patch+yaml`) | `500`, *field not declared in schema* |

`kube::api::PatchParams::default()` is `field_validation: None` and kube's own
doc says that parameter applies only to `Patch::Apply`, which this design does
not use. So **dry-run is not the guard against sending a field the cluster does
not have** — the dry-run returns 200, the real call returns 200, and the audit
log records a successful mutation that changed nothing. That is invariant 4's
*neither record may lie* broken by the server rather than by us, and the only
signal is a `Warning` header kube's `Api` methods do not surface. Phase 7 owns
the fix; the box is open, and it carries the second half of the measurement:
the `422` body echoes **the whole object**, every label, annotation and
`managedFields` entry, so an apiserver error rendered verbatim into the audit log
puts there exactly what `scripts/sanitize.jq` strips out of fixtures.

**The requirement that predicted this is the one directly under the rule that
caused it.** [`REQUIREMENTS.md`](REQUIREMENTS.md) § the version triangle ends
*"a stale pin silently produces wrong diagnoses — the most likely breaking point
of the project"*, two lines below *"pin the oldest supported version feature"*.
Both were written on 2026-08-11 and they are the same sentence read twice: the
prediction was filed as a risk of falling behind, and the pin rule was the thing
that guaranteed it from day one.

**Readable in the types is not present in the snapshot, and the difference is a
whole box each.** Raised by `dev-core` while rewriting the comments, and it
narrows this entry: invariant 6 prunes the watch to *the fields the snapshot
types in `rules.rs` name*, so a field the generated types now carry is still
absent from every object a rule sees until a snapshot field names it and the
pruner keeps it. So the three boxes below are not *read a field* — each is a
snapshot field, a prune line, a fixture that contains it, and only then a rule.
The bump removes the floor under that work; it does none of it.

**The guard's first draft failed the way the box describes, one layer down**, and
is recorded because it is the third instance in this file. Under `set -euo
pipefail` a `grep` that matches nothing kills the script before it can say
anything, so an unparseable `Cargo.toml` produced **exit 1 with no message** — and
a self-test asserting only the exit code cannot tell that from a diagnosis. Found
by `tester`'s own second pass; the fix is that the self-test asserts the failure
*text*, not the status. A guard against silent nothing that fails silently on
nothing is the box's own subject, re-enacted by the fix for it.

**What this box does not do: change a rule.** The bump makes three fields
decodable; using them is three boxes, opened in `todo.md` and not started here.
Rule 15 keeps `restarts == 0` as the stand-in
[D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)
gave it; what changes today is that the comments explaining it may no longer say
the field cannot be read.

### D100 — the field that separates a settled restart from a live one was already in the snapshot, and rule 5 never read it (2026-08-15)

The box asked to be closed one of two ways: *rule 5 lives with the permanence as
it does for every other ending, or something not yet found does it.* It is the
second, and the thing was not merely findable — it was **already decoded**, and
its own doc comment names rule 5 as one of the rules it is evidence for.

**The premise that had to go.** The box ended *"no measured field separates them,
and these records carry no stamps either — so `Finding::timestamp` is `None` and
no clock can age the card out."* Half is right: the synthesized `lastState` a
gang restart writes carries **neither** stamp, measured `null` in 100% of samples
on both pods, so rule 5's `timestamp` really is `None` for this shape. The other
half is false. `state.running.startedAt` — *when the current run began* — sits on
the same container in the same object, is written by a different path, and does
not freeze.

**Measured on kind v1.36.1, `K8RS_CLUSTER=review`, 20,458 container-samples at
5 Hz across two runs**, restricted to the samples where rule 5 actually draws:

| object | serving samples | age min → max | distinct `startedAt` |
|---|---|---|---|
| settled gang pod (3 restarts, then quiet) | 1629 per container | 5.4 s → **923.6 s** | **1** |
| thrashing gang pod (never stops) | 853–1225 | 0 → **7.7 s** | 74 |
| slow crasher (60 s cycle) | 424 | 0 → 60.4 s | 6 |
| native sidecar (6 s cycle) | 49 | 0 → 7.0 s | 6 |

`lastState` was byte-identical across all three of the settled pod's restarts
while `startedAt` moved on every one. **It is not the transient
`AllContainersRestarting` turned out to be**, and the difference is structural
rather than lucky: a condition is a boolean somebody has to keep current, while a
timestamp is a monotone function of `now` that changes only when the thing it
dates happens. The two objects are indistinguishable for the first ~8 seconds
after a restart, which is correct — a pod that restarted five seconds ago *is* a
thrashing pod as far as any object can tell.

**The field cannot be missing in the branch that needs it**, which is what makes
this a suppression that can be proved rather than assumed: `doing_its_job`
returns `c.ready` only under `ContainerState::Running`, and rule 5 returns `None`
on its one other true arm (a finished init container) before reaching the card.
So *serving* implies *Running* implies the variant that carries `started_at`.
14,672 running-container samples, `kube-system` included: zero without it.

**The ruling, and it is [`out_of_memory`](src/rules.rs)'s clause with rule 5's
field.**

1. `Finding::timestamp` comes from `state.running.started_at` on the serving
   branch. This stands on its own merits: rule 5's own doc says *the age is when
   the counter last went up*, and for a serving container that moment is exactly
   this field. Today the `RestartRule` card renders with no age at all, on a
   screen with an age column, and sorts **first** in its band rather than last
   because `Option`'s derived `Ord` puts `None` first.
2. The serving card is suppressed once that age passes **`NOT_READY_GRACE`** —
   the constant `out_of_memory` already uses for the identical question, *is this
   old news on a container that has been fine since?* Its doc claims *one
   threshold for one question, so changing it moves all four*; making it five
   keeps that claim true and inventing a sixth number makes it false.
3. **Absence keeps the card, a future stamp keeps the card, and `!serving` never
   ages out** — `is_some_and`, copied from `out_of_memory`, never
   `unwrap_or(huge)`. The exemption has to be proved.

**The cost, named here rather than discovered later.** A container on a long
cycle — an OOM every thirty minutes, a JVM that dies on the nightly batch — has
an age that exceeds ten minutes for most of its cycle, so its rule 5 card is on
the screen for ten minutes in every thirty instead of always.

**The first draft of this paragraph softened that with a sentence that is
false**, and `dev-core` refused to copy it into the code: *"rule 6 still draws on
the same container throughout, so the screen is not blank about it."* It does
not. `previous_run_failed` returns `None` on `doing_its_job(c)` **with no clock
at all** — the instant the container is serving, not ten minutes later — and
rule 2 carries the same clause with the same threshold while rule 1 needs a
backoff state the container is not in. So between restarts that container draws
**nothing from any rule**, and the two green `nothing(...)` assertions this box
adds at the pinned clock are whole captures drawing zero cards.

Accepted anyway, and the correction changes which argument carries it. Not *the
screen still says something* — it does not. What holds is that **the gap is
older than this change and is shared by four rules**: rule 5's permanence was
covering it by accident, at the price of a permanent card on every pod that has
ever hiccuped, which is the defect this box exists to remove. Rule 5 was in fact
the *least* consistent of the four and is now the most persistent — quiet after
ten minutes where rule 6 is quiet immediately. The alternative discriminator, a
restart *rate*, is either history (invariant 5 forbids it) or `restarts ÷ pod
age`, which is a second number deciding what the clock means and two rules
wearing one card. **What is left over is a real hole and it gets its own box**:
nothing in the rule set reports a container that is fine right now and keeps
dying on a long cycle. That is one question for the whole set, not a fifth
threshold bolted onto rule 5.

**What was *not* done, and why the tempting version is worse.** The same
measurement found rule 5's severity flipping WARN ↔ CRITICAL every restart cycle
on the thrashing pod — 1104 WARN against 354 CRITICAL samples of one container —
because a gang restart parks it in `waiting: RestartingAllContainers` for ~2 s of
every ~9 s and `severity` is keyed on `!serving`. The tidy-looking fix is to
delete `&& !serving` and key severity on the count alone. **It is refused**:
`tests/fixtures/restarts10serving.json` is a captured object whose entire purpose
is that a container serving at ten restarts is WARN and not CRITICAL, so deleting
the clause deletes the fixture's reason to exist. What lands instead is one more
string in the exemption already on that line — `RestartingAllContainers` beside
`CrashLoopBackOff` — so `serving` is never sampled *during* a restart. Same
class as the box's own rejection of `AllContainersRestarting`: a point sample of
a transient must not decide what the user sees.

**The PM closed this box wrongly on the first attempt, and the arithmetic is the
lesson.** `restarts10serving.json` reads `finishedAt 22:54:14` → `startedAt
22:59:26`, and that 5m12s was offered as *how long it has been serving*. It is
not: it is the gap between the runs, which is the CrashLoopBackOff backoff cap.
The age is `now − startedAt`, and at the pinned `now` **both** restart fixtures
have been serving ~49 hours, so both are suppressed and their serving-card
assertions have to move. The pattern that proves it is already in the file, on
the rule this borrows from: `an_old_kill_on_a_container_that_has_been_fine_since_is_not_on_the_broken_now_screen`
reads one capture at two moments through `findings_at`. Same bytes, two clocks —
no new fixture for the ordinary ending.

**One object is still owed and it is on the capture trip**
([D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)
puts it with the PM): a settled gang-restart pod, two containers,
`restartPolicyRules {In,[3]} → RestartAllContainers`, three restarts then quiet,
captured while `2/2 Ready`, so the committed bytes carry
`lastState {137, RestartingAllContainers, startedAt: null, finishedAt: null}`
beside a live `state.running.startedAt`. Nothing in the corpus holds
`RestartingAllContainers` at all today. Note for the manifest, measured: **the
API rejects `restartPolicyRules` unless the container also sets its own
`restartPolicy`** — `must specify restartPolicy when restart rules are used` —
which `broken.yaml`'s comment block does not mention.

**This is not the debounce [D96](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)
refused.** That one needed `analyze` to remember *I saw an exit 3 four seconds
ago*. This is a pure function of `(now, object)` with `now` carried on the
snapshot — invariant 5's own mechanism, which rules 2, 7, 10 and 13 already use.

### D101 — a point sample cannot separate a settled container from one on a long cycle, so the count becomes a report row (2026-08-15)

[D100](#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15)
left a hole and boxed it: **nothing in the rule set reports a container that is
fine right now and keeps dying on a long cycle.** The box demanded the answer be
written here first, because it is one question for four rules and not a fifth
threshold on one.

**What is true today, read off the file rather than remembered.**
[`crash_looping`](src/rules.rs) needs `waiting == CrashLoopBackOff`, and a
container in the middle of a thirty-minute cycle is `Running`, not waiting.
[`out_of_memory`](src/rules.rs) stands down on `doing_its_job` **and** a kill
older than `NOT_READY_GRACE`. [`restarting_repeatedly`](src/rules.rs) stands down
on serving **and** a current run older than the same ten minutes.
[`previous_run_failed`](src/rules.rs) stands down on `doing_its_job` with **no
clock at all**. So the JVM that dies on the nightly batch draws a card for ten
minutes at 03:00 and nothing at all by morning.

**The ruling: the rule set does not answer it, all four suppressors are
unchanged, and the question becomes a counted row in the Phase 4 reports.** It is
one decision for the four — the next rule that wants a fifth constant has to
reverse *this* entry, not add one.

**Why no clause over one Pod sample can carry it.** Three shapes were tried, and
each is refuted by a container this project has already had to reason about:

1. **A count with no clock** — never suppress at or above some restart count.
   Every container in question is above any such line by definition, so this is a
   permanent card on every pod that has ever reached it, whether the line is
   `RESTARTS_WARN` or ten: the false-positive class that needs no unusual
   manifest, only uptime, which
   [D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
   removed from rule 6 and D100 finished removing from rule 5. Moving the line
   changes *which* pods carry the permanent card, not whether one exists.
2. **A longer shared clock** — keep every suppressor and raise
   `NOT_READY_GRACE`. This is the first thing the next reader proposes and it
   fails on its own arithmetic: a threshold catches only cycles shorter than
   itself, so covering the nightly batch needs one above 24 hours, which draws a
   card on every pod on a node that rebooted last night and on every liveness
   blip of the past day. It also needs a sixth number, against D100's own claim
   that there is one threshold for one question.
3. **The current run against the previous one** — *has it outlived the run before
   it?* Inverted where it matters: a container that ran three weeks, hiccupped
   once and has been up twenty-five minutes fails that test for three weeks. It
   survives the count gate too — three restarts in a bad hour last month, a clean
   three-week run, then a node reboot twenty-five minutes ago is four restarts
   and a current run younger than its predecessor — and the arithmetic is one
   nobody reconstructs at 03:00.

**What separates the two containers is how often the numbers moved, and the Pod
does not record that.** Every field the snapshot carries — `restarts`, the
current run's `started_at`, the previous run's stamps, `creation_timestamp` —
differs between a
settled container and a thirty-minute cycler only in how *recent* it is, so every
separator built out of one collapses into candidate 2. History is invariant 5's
forbidden, and the only point-in-time stand-in is a rate, which the box refused
before this entry was written: `restarts ÷ pod age` is a second number deciding
what the clock means, and two rules wearing one card.

**The sequence does exist in the cluster, in another object, and it is refused on
cost and not on physics.** Saying *impossible* here would be false, and a false
impossibility is what stops a question being re-asked when the cost changes. A
core/v1 `Event` carries `count`, `firstTimestamp` and `lastTimestamp` — a counted
sequence, held in an object, readable at one moment, with no history we keep and
no rate we compute; it is what `kubectl describe pod` prints as `(x5 over 2h)`.
It would separate the thirty-minute case and **not** the nightly one, because the
API server's default `--event-ttl` is one hour. It is not read for the reason
this repo has refused it three times already — a global Events watch is the
noisiest stream in the cluster, and the watch budget is why k8rs is lighter than
k9s — and rule 11 waits on that same watch in v0.5
([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12), § Rules
that need Events). **When it opens, this question is asked again** rather than
treated as closed by this entry.

**What a card may not assert, a sorted table may print.** A card claims *this is
broken now* and has to age out or it is noise; a row says *here is the count,
here is how long this run has lasted* and asserts nothing — so permanence, fatal
on the broken-now screen, is correct in a table. The two numbers are printed side
by side and **never divided**, which is [PRIOR-ART §
F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended)'s actual rule: never
divide by a denominator that is not guaranteed complete, and restarts are per
container. That such a number belongs in a report rather than nowhere is Phase
4's own header in [`todo.md`](todo.md), not something F2 says — F2 would have it
print as `n/a`, and a row of two defensible numbers is what that looks like when
the ratio is the undefined thing.

**The row is per container, and both its numbers belong to that container** —
`restarts`, and the age of the run it is in from `state.running.started_at`, the
field D100 established. **Not `last_terminated.finished_at`**: two of `ending`'s
five variants are the `137`s the kubelet synthesizes, and its literal sets
`Reason`, `Message` and `ExitCode` and stops — D100 measured the gang-restart one
`null` in 100% of samples — so a row dated off that field is blank on the very
shape this box is named after, and sorts *first*, because `Option`'s derived
`Ord` puts `None` there. **And not a sum across a workload's pods**: the count
would be per container while the age was per pod, which is F2's own defect with
the division outsourced to the reader — six pods at two restarts each from a node
reboot forty days ago would print the same first two columns as one pod that has
OOMed twelve times since lunch. So the report emits one row per container and
leaves any collapsing to the view; `pod.owner` is the ReplicaSet until Phase 5
resolves it, and a report grouping on it today would key on a hashed revision
name and lose the count on every deploy.

**What the row may not do.** It may not name how the last run ended: `ending` and
`exit_meaning` are private to `rules.rs`, which freezes at Phase 3 close, and
re-spelling the translation in a second file is the two-readers-one-container
defect [D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
exists to prevent. **That boundary is a convention, not a gate**: `Terminated`'s
`reason` and `exit_code` are `pub`, so nothing mechanical stops a raw `exit 137`
reaching a report row, and nothing should print one. The *field list* is a choice
rather than a wall in the other direction —
[D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)
keeps the snapshot types open until Phase 4 close, so a report that genuinely
needs a new field may add one; what it may not do is widen a private item's
visibility quietly.

**Where it goes on screen is `tui-designer`'s, and this entry hands it two
constraints.** The Waste pane is the only report carrying rows of this kind
today, and its headings say the opposite of what this row means — *Things that
cost you something for nothing*, with *Worth knowing (not broken)* under it — of
a container that keeps dying (invariant 14). And Waste is the one pane the
sidebar never badges, so the compensation for the Alerts silence would sit where
nothing sends the reader. Neither is settled here; both go into a Phase 4 box of
their own that sits **ahead** of the row's, because a screen spec that arrives
after the code is a spec of what was built.

**The costs, named here rather than discovered later.**

- **Between restarts the Alerts screen is silent about it**, and that is the
  whole of what this ruling accepts. For a thirty-minute cycle the card is up for
  about ten minutes in every thirty; for a nightly batch it is ten minutes a day,
  at an hour nobody is looking. That reader — the one who checks in the morning —
  is the report's reader, and had nothing before this entry either.
- **On a short cycle the card also disappears while it is being read.** Ten
  minutes of every thirty means a reader who opens k8rs nine minutes into the
  window watches the card go with no action of their own, on a screen that draws
  on events (invariant 7) — which teaches them they misread it. That is D100's
  consequence rather than this entry's, and this is the entry that accepts it.
- **`--once` carries findings and not reports**
  ([`screens/once.md`](screens/once.md)), so a cron report at 09:00 still says
  nothing about the 03:00 crash. Not fixed here: choosing a report needs an
  argument that takes a value, which is what pulls `clap` in
  ([docs/tech-stack.md](docs/tech-stack.md)). It is a real gap and it belongs to
  whoever reopens `--once`, not to this box.
- **The count resets when the pod is replaced** — a rollout, an eviction, a
  drain — so the row understates a chronic restarter right after a deploy. The
  wording has to say *since this pod started* or the row lies, which makes it
  `tui-designer`'s line before it is anyone's code.
- **The report is a point sample too.** It prints how many and how long; it never
  prints how often, because that is the division nobody can defend. Two numbers
  side by side is not a rate — no threshold is derived from them and nothing is
  asserted — and the reader's own arithmetic is not a claim k8rs has to justify at
  03:00.

### D102 — the second copy of a shared sentence is dropped, by `analyze` and not by a rule (2026-08-15)

Rules 5 and 6 draw the same four-line `unwatched_action` on one container: 26
lines about one thing in a 16-row pane. Silencing rule 6 is not the fix:
[D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)
already drew the line at removing a card only where it was never the one that
diagnosed the failure, and here it is — its card is what keeps a reader off
`logs --previous`, which the API will not serve for that record. So the cheap
half is that the *second* copy of a sentence says nothing new.

**The fold lives in `analyze`, beside `explains_a_shortfall`, and not in a
rule.** No rule may be made to know what its neighbour drew, or the two grow a
dependency `rules.rs`'s purity does not survive. A rule owes a true card;
`analyze` owes that two true cards about one container do not tell one story
twice.

**The scope is the container, and the first reason written here for it was
measurably false.** `Finding::object` is the *pod*, and this entry said a
pod-wide fold would therefore drop the card about a second container that lost
its status too. The operator review moved the fold out of the loop and ran the
suite: **205 passed.** It cannot eat the neighbour's card *today*, because every
card the eight container rules draw leads with `container_fact`, so the
subset clause below fails on the first fact of any cross-container pair. The
scope is kept for the converse, which is the reason that will still be true
later: **the day a rule draws a card whose evidence does not lead with
`container_fact`, a pod-wide fold starts deleting the neighbour's cards**, and
`for c in &pod.containers` insures against that for nothing. A reason that only
holds while a coincidence holds is not a reason, and the test named after this
paragraph passed with the fold pod-wide until it was rewritten to fail.

**The more severe survives** — `Severity`'s derived `Ord` declares `Critical`
first, so the fold keeps the *smaller* value, and a comparison written the
natural way round drops exactly the card that matters while passing every other
assertion. A tie goes to the rule that ran first, which is the order `analyze`
already calls them in; survivors keep their emission order, because a `sort`
would reorder every card in the file to settle two.

**A shared sentence is not enough on its own — the card that goes has to add
nothing**, and this is checked rather than assumed: every fact on it, its
evidence split on `FACTS`, must already be on the card that beats it, and it may
carry no `timestamp` the survivor lacks. A duplicated sentence is a cheap
failure; a fact deleted off the screen because a neighbour worded its advice the
same way is not.

**The key is API free text in one arm, and the guard that makes that safe was
accidental.** `previous_run_failed`'s `Failed` branch sets its action from
`last_words(Terminated::message)` — a string the object carries, which anything
that can write `/dev/termination-log` can choose. So a crafted message that
equalled another rule's action verbatim would delete that rule's card, and the
only thing preventing it is that `last_words` frames the quote with a constant
prefix no static action shares. Nobody wrote that down and nothing asserted it;
it is the security gate's *free text from the API is untrusted* reaching a place
the gate's own list does not name — not the screen, but a comparison the screen
is derived from. **The ruling: the frame is pinned by a test**, and the fold's
key stays a `String` rather than growing a type, because the frame is what makes
the strings disjoint.

**Today it fires on exactly two pairs**, both against `previous_run_failed` on
`Ending::Unwatched`: `crash_looping` beside it, and `restarting_repeatedly`
beside it. Every other action in the file is distinct per container, and the
inventory is **asserted over the corpus** rather than left here as a claim — a
rule that starts wording its advice like a neighbour would otherwise begin
deleting cards with nothing going red.

**The fold moved a sentence off the card, so rule 5 had to say it itself.**
`restarting_repeatedly` appended its `claim` clause — *, and the record names no
ending* — only in the *serving* branch, and rule 6 leaves on `doing_its_job`: so
the pair the fold collapses is always the **non**-serving one, and every fold
left the bare count behind. What went with rule 6's card was a title-line
sentence, and the line left holding it is the evidence, the one line
`screens/alerts.md` § The height may cut — measured, two of three roles overflow
at 51 columns. The clause is on both titles now. That is not a rule knowing what
its neighbour drew: `ending` had already told rule 5 how the run ended, and the
title is where a rule says what it read.

**Cost:** O(n²) over at most eight cards, which is why the drops are picked
rather than sorted for. It applies to every shared action, not to this one pair
— which is what the box asked for.

### D103 — the process was measured, and what it lacked was a rule that makes something smaller (2026-08-15)

The user asked whether the rules themselves had made the work slow: one box was
taking about two hours. Measured before answering.

| | |
|---|---|
| age | 2026-08-12 00:25 → 2026-08-15 21:00, ~3.8 days |
| boxes | 76 closed, 121 open |
| last three days | ~6 boxes/day ≈ **2 h/box** |
| `just check`, full, warm | **40 s** |

**The gate is 40 seconds of a two-hour box, so the build was never the
bottleneck.** What costs is the reading. The set an agent must orient in before
it may write a line:

| file | ≈tokens |
|---|---|
| `src/rules_tests.rs` | 231k |
| `NOTES.md` | 149k |
| `src/rules.rs` | 83k |
| `todo.md` | 70k |
| `CLAUDE.md` + `PRIOR-ART.md` + `REQUIREMENTS.md` | 27k |

**≈560k tokens, and a box spends four to five cold dispatches in it** —
`dev-core`, `tester` twice, `k8s-admin`, then the PM's own pass. None of them can
hold it, so each greps, reads a slice, misses what was not in the slice, and the
review sends the box back. **The cost per box is a function of the boxes already
closed**, which is the deceleration the user felt: 39 decisions were recorded on
day one and 103 exist now, and every one of them is read by every box after it.

**The evidence that this is what it is, and not ordinary review:** rule 1 got
*reads how the previous run ended* on 08-14 08:36, rule 5 got the same fix on
08-14 21:08, `137` was made to be read from the object on 08-15 03:37, and *every
rule reads them once* landed 08-15 11:46. One class defect, four boxes, 27 hours,
one rule at a time — because no reviewer could hold every rule at once. Over the
same window 41 commits carried **12 `feat`** and 17 `docs`.

**The defect is structural and it is one sentence: this repo has append and no
compaction.** Every rule here ends in *record it* — `NOTES.md`, `todo.md`, a doc
comment citing both. Not one rule anywhere makes something smaller, and the two
files every agent must read are the two that every box grows. `todo.md`'s worked
Phase 3 boxes average **35 lines** against **10.6** in an untouched Phase 5,
because the outcome of each review round is appended back onto the box that
prompted it. That history is real, but it is a summary of `NOTES.md` entries the
box already links to, sitting in the file whose job is to say *what is next*.

**Four rulings, and the first reverses this file:**

1. **`src/rules_tests.rs` splits now, not at Phase 3 close**, reversing
   [D91](#d91--the-tests-split-and-the-product-file-does-not-2026-08-15)'s timing
   and nothing else of it — the split is still by rule family, and `rules.rs`
   still stays whole for D91's own reason. D91 deferred it because eight boxes
   were open against the file; twenty-four are open now, and the file went from
   13 105 lines to 17 460 in the one day since, +33%. The premise inverted: the
   deferral was priced at eight boxes of churn and is now being paid at
   twenty-four, on the largest single read in the repo.
2. **`todo.md` holds boxes, not history.** A closed box keeps its title and its
   `NOTES.md` links; the round-by-round narrative under it goes, because every
   claim in it cites a decision that holds the same thing.
3. **`NOTES.md` opens with a one-line index of every decision**, so an agent
   reads ~4k tokens to find the entry and opens one section instead of grepping
   149k. It costs no new machinery: each line is a link to its own anchor, and
   `scripts/check-docs.py` already fails on a broken one, so a renamed or deleted
   heading is caught by the guard that exists.
4. **`k8s-admin` reviews a rule *family*, not a rule** — see
   [CLAUDE.md § The cycle](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it).
   The 137 chain above is what per-rule review costs: the reviewer that could
   have seen all four at once never saw two.

**And the standing rule the four are instances of: a box may not be added to an
open phase.** On 08-14 the k9s prior-art review turned twelve gaps into twelve
Phase 3 boxes while Phase 3 was running, which is how a phase stops converging.
Found work is recorded and lands in a later phase.

**What this does not touch.** No gate is loosened, no test is deleted, no
invariant moves. Everything cut here is a second copy of something that exists
elsewhere — which is [D102](#d102--the-second-copy-of-a-shared-sentence-is-dropped-by-analyze-and-not-by-a-rule-2026-08-15)'s
finding, one layer up.

### D104 — the second agent was re-running the first agent's commands, and a tool does it better (2026-08-15)

[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
measured what a box costs to *read*. This is what a box costs to *run*, measured
on the same day over one box — the `analyze` fold,
[D102](#d102--the-second-copy-of-a-shared-sentence-is-dropped-by-analyze-and-not-by-a-rule-2026-08-15):

| dispatch | wall | tokens | defects it found |
|---|---|---|---|
| `dev-core` writes it, proves it red then green | 19 min | 207k | — (the work) |
| `tester`, step 4 | 14 min | 120k | **0** |
| `k8s-admin`, family review | 33 min | 212k | **2**, both load-bearing |
| `dev-core` fixes | 8 min | 230k | — |

**Step 4 re-ran four mutations that were already red and found nothing.** Its
fifth, its own, went green once the predicate was tightened — an artifact of the
looser code, not a defect. What that turn *did* find came from somewhere else
entirely: a probe that fed the fold a stamped record, and an audit of whether the
expected numbers in the edited tests were derived from the requirement or updated
to match the output. Meanwhile the two real defects — the survivor's title losing
the diagnosis, and this file recording a reason for the container scope that a
five-minute experiment showed was false — came from the review that reads a
family at once.

**So the reproduction moves to `just mutants` and stays mechanical.** D26's rule
was never *a second agent repeats the first one*; it was *a test that cannot fail
is not a test*, and a surviving mutant states exactly that, from a tool with no
incentive to say the build is green. The author still pastes its own red and
green — that is evidence for the reader, not the gate. `tester` keeps the work
that pays: attack the assertions, feed the shapes nobody fed, read what the
screen prints, run `just check`.

**And the dev writes a family in one turn, because that is what the reviewer
already reads.** The `137` chain cost four boxes over 27 hours one rule at a
time, and the fourth found what the first three could not see from inside a
single rule. A family briefed as one box is one dev turn and one review; boxes
that share no code stay one at a time, because batching unrelated work is how a
review stops fitting in a reviewer's head.

**And the first draft of this entry wrote the gate at the wrong scope, twice —
in both directions.** It said *`just mutants` over the changed file*. Run once:
**519 mutants** on `rules.rs`, 58-second baseline build, **~2s per mutant**,
twenty-odd minutes whole. The first correction called that *hours, unrunnable in
a box*, which was an estimate written before the run finished and was wrong by an
order of magnitude — the same defect as the sentence it was fixing, one turn
later. What is true is smaller and still decides it: **a box that changed one
function has no business re-proving the other five hundred mutants**, so the
per-box gate is `cargo mutants --in-diff` over the box's own diff, and the
whole-file run stays what `todo.md` always had it as, a phase-close gate, with
`--iterate` skipping what an earlier run caught. Both wrong versions came from
reasoning about a number instead of reading it.

**What is deliberately not cut**: the operator review, the second pass over the
landed tree, the security gate. In this measurement they are the only things that
found anything, and speed taken out of them is not speed.

**What is left, measured, for whoever picks this up next.** `rules.rs` is 5 186
lines and **2 400 of them are doc comments** — 51 blocks of fifteen lines or more
account for 1 644. Every dispatch reads that file. This repo's own rule says a
doc comment *states what the item is and cites the decision that shaped it — it
does not re-argue it*, and at 46% the file is re-arguing at scale: the same
second-copy defect [D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
found in `todo.md`, one layer down. `src/rules_tests/pod.rs` is the same shape at
9 809 lines, and it already carries the section markers a further split would cut
along.

### D105 — the security gate splits into what a script can decide today, and what is waiting for code (2026-08-16)

`CLAUDE.md` § *Security gate* is run by a human on every diff, and a checklist
item is the thing that gets skipped — every other list in this repo that mattered
became a script. `scripts/security-guard.py` is that script for the half a
script can answer **on this tree**, and the split is the decision, not the code.

**Mechanical now, six checks, 17 planted violations in its `--self-test`.**
Workflow hygiene — a top-level `permissions:` that defaults to read, every
third-party `uses:` pinned to a 40-hex SHA, no `pull_request_target` beside
secrets — which nothing checked before and which the CI file now checks about
itself. No shell spawned from `src/`. No dependency or hostname outside the ten
(invariant 10). No `Debug` derived over a type that can hold a token. No door
into the in-cluster ServiceAccount environment. No TLS verification knob turned
off by us.

**Two of those guard a class that is empty today, and that is the point.**
`src/` spawns no process and holds no client, so checks 2, 5 and 6 pass over
nothing — they exist so the class *stays* empty when `$EDITOR` lands in v0.4 and
`kube` in Phase 5. That is the honest version of an early guard, and it is the
opposite of the vacuous one: **a guard over an empty class is a guard; a guard
over an unwritten feature is a pass that means nothing.** None was written for
the second kind.

**They ban the call and never the word.** `Config::infer` and
`Client::try_default` are in the ban list beside `Config::incluster*` because
they are the *convenient* doors — both try the ServiceAccount environment
first — while `Config::from_kubeconfig` stays legal. The TLS ban needs a literal
`true`, so `accept_invalid_certs = user_said_insecure` is still writable: Phase 5
has to **honour** a kubeconfig that sets `insecure-skip-tls-verify` and Phase 11
has to show it in the header, and a guard that made either unimplementable would
have been enforcing the opposite of the rule. Both negatives are asserted in the
self-test, in those words.

**Still human-checked, with what each one needs and the phase that brings it.**
Deferred because the code to check does not exist — not because it is hard:
dry-run before the real call · the resourceVersion carried and the 409 re-read ·
the typed object name on destructive actions · no bulk mutation · every attempt
reaching the audit log · that log's `0600` and append-only-ness · `--read-only`
being structurally unreachable (Phase 7, some of it e2e) · control-character
stripping on every free-text field, which is the most valuable of them and needs
a renderer to feed `\x1b[2J` through · env values and Secret reveal · bounded
sizes (Phases 5–6, 9–11) · the edit temp file at `0600` and `../` in a name
(v0.4) · `SHA256SUMS` at release, which extends check 1 the day the release
workflow exists (Phase 13).

**Two rows split rather than deferred whole**, and that shape is worth copying:
the panic path's static half — no derived `Debug` over a config holder — is check
4 today while its pty half waits for Phase 11; TLS splits the same way, *us*
disabling verification is mechanical now, *honouring and displaying* the user's
flag is not.

**What it cost to get honest output.** The guard's own second pass found two
silent-green defects in itself, both of the class this repo keeps paying for: a
`#[derive(...)]` wrapped across lines matched nothing and read as *derives no
Debug*, and a field typed `HashMap<String, Client>` was cut at the comma, so the
half carrying the token was the half the pattern never saw. Both were found by
running it against `src/rules.rs` rather than against its own fixtures — the same
rule as [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12),
applied to the guard.

### D106 — Phase 3's twenty-three open boxes are two families, six foreign boxes and one already done (2026-08-16)

[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
wrote the rule that stops a running phase being fed new boxes, and it has held:
nothing has been injected into Phase 3 since. **The pile that was already there
was never triaged under it**, and Phase 3 has not closed in the two days since.
All twenty-three were read whole on 2026-08-16 — not their titles, their bodies —
and the first finding is that **almost none of them is deferrable on merit**. They
describe cards that print wrong text today, and Phase 3's own done-when is
*running the binary on a fixture prints correct findings*. The defect is not that
the boxes exist; it is that they are twenty-three units of work for what is
**two classes, and a class is what [D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
ruled is reviewed as one turn.**

**Family A — what the object actually supports, against what the card claims it
does.** The lost init-container status read as a clean finish; `the last thing it
logged was:` presenting a kubelet-authored sentence as the application's own
words; `exit 255` after a node reboot blamed on the application; `lastState`
meaning *the last run Kubernetes managed to write down* rather than *the previous
run*; a container status arriving with no matching declaration. Five boxes, one
question, and they touch the same four helpers — `ending`, `exit_meaning`,
`exit_fact`, `container_snapshots`. This is the `137` chain
([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15))
arriving a third time: one class, split across boxes, each review seeing one
slice.

**Family B — what the card tells the reader to do, on the shape it is actually
drawn about.** Rule 6's `ContainerStatusUnknown` card that no clock can age out;
event expiry taught on one role and not the other two; rule 1's `Failed | None`
arm pointing at a log no command can reach; the first instruction being dead on
the commonest shape. Plus **`screens/alerts.md`'s action budget**, which belongs
here and not in a later phase: it rules that an action wrapping past five lines is
a `rules.rs` defect, 9 of 52 shipped actions exceed it, and `rules.rs` freezes at
Phase 3 close — a box that may require a frozen file to change cannot be deferred
past the freeze. Five boxes, one question.

**Six boxes are not Phase 3's, by owner or by rule, and move to Phase 4.** Three
are `tester`'s and touch no rule: the 100-column guard, `certs-test.sh`'s stale
`(C1 warns)`, and anchoring `sanitize.jq`'s node-name refusal
([D94](#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)).
Three arrived from the pin bump rather than from Phase 3's contract —
`restartPolicyRules`, `terminatingReplicas`, and the in-place-resize split between
what a container asks for and what it has
([D99](#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)) —
and each is a snapshot field before it is a rule, which is precisely the
one-phase window
[D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)
opened. Moving them is that rule being used, not an exception to it.

**One box is already done.** *Split `rules_tests.rs` into one file per rule
family — at phase close, after the last rule box and not before it.*
[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)'s
first ruling reversed that timing and `4d73366` landed it: `src/rules_tests/`
holds `pod.rs`, `node.rs`, `workload.rs`, `certificate.rs`, `snapshot.rs`, and
`rules_tests.rs` is 499 lines against the 13 105 the box describes. It stayed
unchecked because the ruling that reversed it was written in a different file from
the box it reversed.

**The boxes are not reordered, and that is deliberate.** Physically regrouping
twenty-three bodies is a large diff over text every one of them cites, for no gain
the brief cannot give for free: the families are named at Phase 3's head and the
PM briefs them as one turn. Six turns and one tick replace twenty-three.

**What this does not do is loosen a gate.** Every box in both families is still
fixed before Phase 3 closes, with its own red and green; what changes is that
`k8s-admin` reads five at once with the helpers they share, which is the only
position from which two rules disagreeing about one container is visible.

### D107 — CLAUDE.md grew back past the size that made D60 compress it (2026-08-16)

[D60](#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)
measured `CLAUDE.md` at 682 lines and 6 548 words, called that a standing cost
paid at every session start and every compaction, and cut it to roughly 3 000
words. Measured again on 2026-08-16: **689 lines, 6 604 words** — larger than the
state that was ruled a problem, four days later. § *Agent workflow* is 244 of
those lines, 35% of the file; D60's own figure for the same section was 30%.

**The mechanism is the one D60 named and the one
[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
generalised: every rule here ends in *record it*, and the file where a rule is
recorded is this one.** Each addition was correct on its own — a real leak, a real
concurrency defect, a real measurement — and each arrived with the story that
justified it, which is the half D60 ruled belongs in `NOTES.md`.

**The rule is unchanged and is applied again, not rewritten: the rule stays in
`CLAUDE.md`, the story moves here and is cited by number.** What goes is
narrative that a `D##` already holds. The `137` chain's *four boxes, 27 hours* is
told three times in the file — in § The cycle, in step 6, and again in § Phase
close — and [D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
holds it once. The fold box's *fourteen minutes and 120k tokens* is
[D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)'s.
The scratchpad collision, the review that had to prove a red in two files it did
not own, the twelve boxes injected into a running Phase 3 — all held elsewhere,
all retold here.

**No invariant, no security-gate line, no ownership row and no cycle step is
dropped**, which is the same guarantee D60 gave and the thing to check this
against: the count of each is the test, and it is stated in the box that lands
this.

**What the compression actually returned, measured after the fact: 702 → 680
lines, 3%.** The target written into this entry before it ran was ~450, and it
was wrong — an estimate about a file rather than a reading of it, which is the
error this file already has a rule against. § *Agent workflow* went 244 → 224,
§ *Security gate* went **up** 76 → 85 because six of its lines now carry the
`[auto]` marker and the sentence saying what the script does and does not cover.
The count was held: 14 invariants, 7 cycle steps, 6 ownership rows, every
security-gate item present.

**The reason it resisted is the finding, and it reverses the premise of this
entry.** [D60](#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)
already took the stories out. What grew back was not narrative fat but *rules* —
D92's cluster split, D96's resume-is-a-dispatch, D98's standing authorisation,
D103's family review, D104's mutation gate, D105's `[auto]` split, D108's two new
trees. Every one is load-bearing and none can be cited away, because a rule
cannot live in `NOTES.md` and be obeyed from `CLAUDE.md`. **`CLAUDE.md` grows
because the process grows**, and the only honest lever on it is having fewer
rules, not shorter ones.

**So the file was not the cause of the slow box, and saying otherwise would have
been the third estimate in a row.** The measured cause is
[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)'s
— cold dispatches re-reading everything — and the lever that actually moved it
was batching:
[D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)
took Phase 3 from twenty-three units to six turns, roughly 4×, on the same day.

**One real duplicate was found and removed, and it was not in `CLAUDE.md`.**
Measured across the six agent-facing files: **invariants were restated in five of
them** — `dev-ui` five, `dev-core` three, `tui-designer` two, `k8s-admin` two —
each file opening by declaring that `CLAUDE.md` is binding and not repeated
there, and then repeating it. `540b87e` is what that costs: the agent files were
carrying a copy of the rule that had changed. They now cite invariant *numbers*.
The line saving is six; the point is that there is one copy to edit.

### D108 — work with no phase gets a file, and measurements get a directory (2026-08-16)

Two gaps, both visible in the same hour, both created by rules that were right.

**`todo.md` had no home for work that belongs to no phase.**
[CLAUDE.md](CLAUDE.md#what-to-do-next) says a box is never added to an open phase
and what is found mid-flight is *boxed in a later phase* — which is correct and
has one failure mode: *later phase* becomes the answer to everything, and the
next phase's scope is decided by what the previous phase happened to trip over.
Measured the same day:
[D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)
moved six boxes out of Phase 3 and took Phase 4 from twelve open boxes to
eighteen — a 50% scope increase on a phase nobody had planned that way.
[`backlog.md`](backlog.md) is where an unphased finding goes instead.

**The rule that keeps it from becoming a second `todo.md` is that nothing in it
is work.** No agent reads it, `/basla` reads `todo.md` and only `todo.md`, and
the triage that turns entries into boxes runs at phase close with the rest of the
ritual. It also makes the file every turn pages through smaller, which is
[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)'s
standing demand: an entry that is never picked is pure weight in `todo.md` and
costs nothing in a file nobody opens mid-phase.

**`k8s-admin` measures and writes nothing down.** Its definition tells it to
paste commands and real output, and that output lands in a report to the PM that
lives in a conversation and ends with it. What survived did so by being retyped
into a box body — *`docker restart` leaves `exitCode: 255, reason: "Unknown"`* is
in `todo.md` for that reason and nowhere else, and it is the measurement that
turned a shipped rule into a defect. [`reports/`](reports/README.md) keeps the
measurement itself, one file per run, named by date and subject.

**So `k8s-admin`'s ownership row changes from *nothing* to `reports/`**, and it is
the only agent whose row changes. It is the agent whose output was evaporating;
giving the tree to anyone else would mean transcription, which is the step that
was already losing the data. Everything else about it holds — it still never
edits product code, still never commits, and a measurement is still evidence for
a finding and never a box's done-when
([D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)).

**A report may not conclude.** If a measurement settled something, the settlement
is a decision with a `D##` here, and the report is what that decision cites. A
report that argues is the second copy of a decision, and this file has already
ruled which copy goes stale.

**The security finding this opens, stated rather than deferred quietly.** A report
carries real cluster output into a *committed* file, which is the exact path
`scripts/sanitize.jq` exists to guard for fixtures — and it does not run over
`reports/`. Until a guard does, the rule is manual and narrow: no object dumps,
only the specific field values a finding turns on, never a token, key,
kubeconfig, env value, annotation payload or node identifier
([REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements)). Extending
the gate is `tester`'s and is boxed in Phase 4. A manual rule enforced by the PM
reading the diff is the weaker thing this repo has already been burned by
([D26](#d26--a-green-build-that-proves-nothing-2026-08-12)), which is why the box
exists rather than a promise.

**mem0 was proposed in the same breath and is not built.** Persistent memory is
already done twice — this file, versioned and guarded by `scripts/check-docs.py`,
and the session memory directory outside the repo — and a hosted instance would
put project data on an outbound connection
[docs/security.md](docs/security.md) says does not exist. Recorded in
[`backlog.md`](backlog.md) § Ruled out so the idea does not arrive a second time
without the reason attached.

### D109 — the family is the unit of work, and the commit stays per turn (2026-08-16)

The user asked for a phase-based cycle on 2026-08-16, with one reason: **a box
takes hours.** The reason is real and
[D103](#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
measured it — cost per box is a function of the boxes already closed, because
every agent re-reads this file, `todo.md` and `CLAUDE.md` on every cold dispatch.
The proposed fix does not address that cause: batching the *review* to a whole
phase makes nothing faster, it makes a defect in box 3 surface after box 16 was
built on it.

**What was granted, because it is the same instinct at a granularity that
works:** the **family** becomes the unit of the cycle rather than an exception
inside it, and the same change already paid on the day it was made —
[D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)
turned Phase 3's twenty-three open boxes into six turns. That is the reduction
being asked for, without the *find out at the end* failure. `ops.rs` and shared
helpers stay per-box, unchanged from D103: their blast radius is not a family.

**What was refused, and the user's own reason is what refuses it:** the commit
does not batch. A commit costs no time at all, so moving it to phase close buys
nothing against *hours per box* and pays for it with the recovery point — sixteen
boxes of work, one changelog line, and no way back to box eleven. Conventional
commits are also what git-cliff reads
([CLAUDE.md § Git rules](CLAUDE.md#git-rules)), so a phase-sized commit collapses
a phase of history into a single entry.

**The two levers that do address hours per box are elsewhere and both are
running:** [D107](#d107--claudemd-grew-back-past-the-size-that-made-d60-compress-it-2026-08-16)
cuts what every agent reads before it starts, and
[D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)
already removed a whole dispatch's worth of re-run from every box.

### D110 — the brief names the regions, because a cold dispatch reads fifteen thousand lines (2026-08-16)

The user reported a box costing two hours and named the markdown files as the
cause. Measured instead of accepted, and the measurement refuses the premise —
this is what one cold `dev-core` dispatch has in front of it:

| File | Lines |
|---|---|
| `src/rules_tests/pod.rs` | 9 809 |
| `NOTES.md` | 8 658 |
| `src/rules.rs` | 5 186 |
| `todo.md` | 2 449 |
| **`CLAUDE.md`** | **680** |

**`CLAUDE.md` is 2% of it.** [D107](#d107--claudemd-grew-back-past-the-size-that-made-d60-compress-it-2026-08-16)
cut it by 3%, which is three parts in ten thousand of what a dispatch reads, and
an hour was spent there before anything was measured — the same error D107 itself
records, made twice in one day. `pod.rs` alone is fourteen times the file that
was being compressed.

**The second half of the cost is the dispatch count, and it is now measured too.**
The `todo-guard` turn — one agent, one small Python file, no rule logic — took
**10.2 minutes and 94 632 tokens**. A box runs four to five of those in sequence:
dev, the mutation run, `tester`, `k8s-admin`, then the PM's own pass. Two hours is
that arithmetic, not a bloated instruction file.

**The ruling: the brief names what to read, by region.** `rules.rs` already
carries `// --- … START ---` markers and they map one-to-one onto the test
modules — pod 2 454 lines, snapshot 994, node 948, workload 326, certificate 153.
An agent told *fix rule 5* and nothing else pages all 5 186; an agent told *the
pod region, `rules_tests/pod.rs`, and the helpers `ending` / `exit_meaning`*
reads a third of it. The brief grows from five lines to six, and it is the
cheapest of the three levers because it moves no file and changes no rule.

**The other two levers are already banked and are the larger ones.**
[D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)
and [D109](#d109--the-family-is-the-unit-of-work-and-the-commit-stays-per-turn-2026-08-16)
made the family the unit, taking Phase 3 from twenty-three turns to six — a ~4×
cut in dispatch count, which is the thing the clock is actually spent on.
[D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)
removed one dispatch's worth of re-run from every box before that.

**What is deliberately not done: `pod.rs` is not split.** It is the largest file
in the repo and the obvious next cut, and it is exactly where
[D91](#d91--the-tests-split-and-the-product-file-does-not-2026-08-15)'s warning
applies — a module boundary is where the second copy of a shared helper grows
back, and the defects this repo has paid most for were two rules reading one
container and disagreeing. A split there is a decision with evidence behind it,
not a reflex against a line count. Recorded in [`backlog.md`](backlog.md).

### D111 — the guard list exists once, and CI gets no new action for it (2026-08-16)

`scripts/todo-guard.py` landed in `just check` on 2026-08-16 and **did not run on
a push**, because `.github/workflows/ci.yml` kept its own hand-written list of
guard steps. The justfile's header has said *`just check` is the whole of CI, or
it is a lie* since Phase 1; the lie was seventeen lines long and nothing could
have reported it. Found by `tester` in its own report on the box that created it,
which is why it is that box's defect and not a new one
([CLAUDE.md § What to do next](CLAUDE.md#what-to-do-next)).

**The fix is the class, not the instance:** the guards live in one list, CI
invokes it by name, and adding a guard is one edit. A third copy existed in
`todo.md`'s Phase 1 CI box, enumerating the same steps in prose — removed in the
same change, because it is the next thing that drifts.

**What is asserted rather than trusted.** *CI runs every guard* is true by
construction once CI stops enumerating them. *No guard is added to the wrong
place* is not, so the list asserts that `just check` calls it and names no
`scripts/` guard directly — a guard reachable only from `check` would never run
on a push, which is the original bug wearing a new coat. Proven red three ways
against the final bytes: a guard line added to `check`, `check` no longer calling
the list, and `check` renamed away. The middle one doubles as the extraction's
canary — an empty read fails it rather than vetting nothing, which is the
degrade-in-silence shape `write-guard.py`'s `CANARIES` exists for.

**The single CI step is safe to leave unnamed because `set -x` names the failing
guard.** The last traced `+ python3 scripts/…` line is the culprit, proven by
running in a tree with no `scripts/`. Nine named steps bought a label that the
trace already gives at 3am.

**And the reversal, which is the part worth recording.** The first version
reached for `taiki-e/install-action` so CI could call `just`, pinned to a 40-hex
SHA and correctly flagged by `tester` as a supply-chain decision the PM owns.
**It was refused.** `just` had no other use in CI — `run: just guards` was its
only appearance — so a fourth third-party action would have been bought purely to
reach an entry point, against a rule that says a dependency is asked for and not
reflexed (invariant 10, and the security gate's line on third-party actions).
The list moved into `scripts/guards.sh` instead: `just guards` calls it, CI calls
it, one list, no new trust. **The canary moved into the script rather than staying
in the justfile recipe** — CI would not have run it there, and a local-only
assertion about CI coverage is the same hole again.

**The rule this leaves behind:** when a fix needs a new dependency to reach an
entry point, the entry point is the thing to change.

### D112 — `lastState.terminated` has three authors, and the file was reading it as if it had one (2026-08-16)

Phase 3's **Family A** — five boxes, one question: *what does the object support,
against what does the card claim*
([D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)).
The answer that came back is one sentence and every finding in the family is an
instance of it: **`lastState.terminated` is written by three different authors**,
and `rules.rs` knew about one and a half of them.

1. the **container** — its own last words, through `terminationMessagePath` or the
   log tail under `terminationMessagePolicy: FallbackToLogsOnError`;
2. the **kubelet** — four synthesized literals, every one of them stamp-less;
3. the **container runtime** — `StartError`, containerd's `255`/`Unknown`, CRI-O's
   `-1`. The one nothing in this file knew about, and the only one that writes
   real stamps beside its own sentence.

Evidence: [`reports/2026-08-16-terminated-record-stamps-and-authors.md`](reports/2026-08-16-terminated-record-stamps-and-authors.md),
an ephemeral `K8RS_CLUSTER=review` run at `kindest/node:v1.36.1`
([D92](#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)).
**Three of the defects below had shipped past 211 green tests**, and a cluster is
what produced all three: the epoch duration, the runtime's sentence printed as the
container's, and the duration that counts a node's downtime. None of them is a
shape a fixture in this repository holds.

#### Two boxes were reversed, and both reversals were written down after they were acted on

[CLAUDE.md § The PM does not wait for approval](CLAUDE.md#the-pm-does-not-wait-for-approval--the-boxes-run-back-to-back)
requires a reversal in this file **before** it is acted on. Both of these were
decided in the brief and recorded here afterwards. Naming the slip is the point of
recording it.

**931 asked for a third `Ending` keyed on the `reason`, and it was refused.**
`kubelet_pods.go:2705-2723` at `release-1.36` writes `Terminated { reason:
"Completed", exitCode: 0 }` for an init container whose status the runtime lost —
but `reason: "Completed"` is byte for byte what a watched finish writes, so it
separates no object the API can produce. And the kubelet is **deducing rather than
guessing**: the write is gated on `HasAnyRegularContainerCreated`, and regular
containers start only once every non-restartable init container has succeeded.
`Ending::Finished` is therefore the true reading, `doing_its_job` answering *yes*
is correct, and a card would be a permanent WARN on **every static pod in every
cluster** after a kubelet restart — the class the source's own comment names
first. The box's premise was the defect, not the code; the outcome is a corrected
doc and a test that pins the silence. `k8s-admin` verified the deduction against
v1.36.1 and ruled the box closed rather than reopened.

Two corrections underneath that ruling, both found after it was written:

- **The literal can reach `lastState`.** The first reason recorded was *it lands
  in `state.terminated`, so rules 1, 5 and 6 never reach it* — false at v1.36.1.
  `convertContainerStatus` (`kubelet_pods.go:2294-2306`) copies
  `oldStatus.State.Terminated` into `lastState` when the containerID changes, and
  the literal carries an empty one. The verdict does not move; what moves is that
  the silence rests on the *reading* and not on the field being out of reach.
- **The gate is weaker than the deduction it licenses.**
  `HasAnyRegularContainerCreated` counts a regular container in `Exited`, while
  `computeInitContainerActions` computes the same thing from the same list and
  deliberately excludes it (*"If the node is rebooted, all containers will be in
  the exited state…"*). So the status path can write the literal during a sandbox
  rebuild about an init container that has not run *yet* in the new sandbox. Still
  true of the past, silence still right — recorded rather than boxed, because the
  doc may not read tighter than the gate is.

**991 said "it needs no `reason`, only a row and an arm", and was overruled.** A
program that runs `exit -1` in a shell reports `255`, so a code-alone row would
tell that reader their program did not fail. Keyed on the pair, as `137` is. Two
things then arrived that the box did not have. The pair is **containerd's and not
the kubelet's** — `cStatus.Reason = status.Reason` copies straight out of the CRI
status — and **CRI-O writes `-1` / `"Error"` for the same event**, which fell
through to `Ending::Failed` and shipped the exact lie `991` exists to remove, on
every OpenShift cluster. `-1` joined the ending keyed on the **code alone**, and it
is the *cleaner* key of the two: POSIX statuses are `0..=255`, so no process can
report it and the objection that made `255` a pair does not apply. A source-derived
pin, on the footing
[`RESTART_ALL`](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)
already stands on — no CRI-O node exists on any host this repository builds on, and
no fixture is invented for one
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12),
[D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).

#### The stamp premise was half right, and the half that was wrong decided box 966

Guarding on a missing `finishedAt` is sound about the **kubelet**: of the five
places `kubelet_pods.go` builds a `ContainerStateTerminated`, one sets `StartedAt`,
`FinishedAt` and `ContainerID`, and the other four set `Reason`, `Message` and
`ExitCode` and stop. It is not sound about the **runtime**: a `StartError` record
carries a real `finishedAt` beside a message runc wrote, and CRI-O does the same
with `Reason: "Error"` — which is also what an ordinary application failure
carries, so no reason tells them apart either. Measured, the old frame printed
*the last thing it logged was: failed to create containerd task: …* about a
container that logged nothing.

Box 966's own done-when then decided it: **if the card cannot tell the two apart
from the object, the sentence stops claiming authorship rather than guessing it.**
It cannot. `last_words` now says who *recorded* the line and never who wrote it.
The stamp guard stayed, on the narrower claim it can actually support — a kubelet
placeholder never displaces a card's advice.

#### Two numbers were printed with more confidence than their inputs carry

Both in `lasted`, both
[PRIOR-ART § F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended)'s class, both
measured rather than reasoned.

**`startedAt` arrives as the epoch, not as null.** containerd leaves it at `0` on a
start failure; the kubelet writes `metav1.NewTime(cs.StartedAt)` unconditionally;
`time.Unix(0, 0)` is **not** Go's zero time, so it marshals as a real RFC3339 stamp
and no `Option` on the path can see it. A mistyped `command` — one of the
commonest broken-pod states there is — drew **`ran for 20681 days`**, and was still
drawing it seven restarts later. **`Finding::timestamp`'s own note asserted the
opposite in writing** (*"An `Option` and not a zero"*), which is what made the shape
unthinkable one function away: the note that guarantees a thing is the note to
check.

**`finishedAt` on a node outage is written at recovery, not at death.** containerd
stamps it when it comes back and finds the task gone, so a container that ran for
50 seconds behind a node away for three minutes printed **`ran for 3 min`**, and a
node down overnight would print *ran for 8 hours* — a duration nobody measured, one
line under a translation saying nobody read how the run ended. Dropped on
`Ending::CodeUnknown`.

**And dropping it bought the card back.** Rule 1 spelled the duration *the last run
lasted …* while rules 6 and 15 spelled it *ran for …*, off one `lasted` call —
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)'s
own class, one fact in two wordings — and because
[`one_card_per_action`](#d102--the-second-copy-of-a-shared-sentence-is-dropped-by-analyze-and-not-by-a-rule-2026-08-15)
folds only where the beaten card's facts are a **subset** of the survivor's, the
two spellings left **two byte-identical ten-line cards on one container in a
sixteen-row pane**, inside the mechanism written to prevent exactly that. One
spelling in one helper fixed half of it; the duration not belonging on that ending
fixed the rest, and rule 6's facts now reduce to the survivor's first fact on both
shapes. Two defects, one visible only through that clause.

#### What the cards say now, and one string that failed the plain-language rule

The `lastState` claims were reworded because the record is *the last run Kubernetes
managed to write down* and not *the run before this one* —
`kubelet_pods.go:2616` gates the second write site on
`LastTerminationState.Terminated == nil`, so only a container's **first** lost
status is ever recorded, measured at nine restarts under one unchanged record. The
frame is [D95](#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)'s
existing *on record* wording rather than a third framing, and height forced the
titles shorter rather than longer: every card measures ten lines of ten.

**One first draft failed invariant 13's second half and was replaced.** *"The
record names no ending for this container"* is jargon-free and **referent-free** —
nothing on the card, in `screens/alerts.md`, or in `kubectl` output introduces *the
record* as a thing, so a newcomer has no object to attach the noun to. The owner is
nameable, so it names one. A bare noun the card never introduces is the failure
mode; *on record* as a **modifier** on a subject the reader already has is not, and
that distinction is what the surviving titles rest on.

#### Not proven, and it is written into the code that way

No CRI-O node and no virtual node exist on any host here, so the `-1` pair and the
undeclared-container shape are source-derived pins and plants on decoded captures
([D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)),
not captures. `cargo mutants` does not mutate struct-literal fields, so the
decode's assertions rest on a hand mutation. Box 1380's done-when says *a fixture*
and it is a plant; that is the honest reading of a shape no cluster this repository
can build will produce, and it is named here rather than smoothed over.

#### The process finding, and it is the same one as the code finding

The family took four rounds. Rounds 1 and 2 were the author and `tester` working
from source and fixtures; **rounds 3 and 4 came from the two passes that read
something nobody else had read** — a real cluster, and the assembled files. Neither
found a wording nit. The cluster found two defects that ship, and the landed-tree
pass found a headline stating an absolute the code two lines below broke, a
sentence left in twice by the edit that superseded it, and a comment that went
stale in the same turn that made it stale. **A gate that is skipped because
everyone upstream passed is the gate that had the finding**
([CLAUDE.md § The cycle](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it)).

### D113 — a card's parts were budgeted separately and never added up, and everything else this family found was reached by fixing that (2026-08-16)

Phase 3's **Family B**, five boxes, one question: *what does the card tell the
reader to do, on the shape it is actually drawn about*
([D106](#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)).
Evidence: [`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md`](reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md).

`screens/alerts.md` said an action wrapping past five lines was a `rules.rs`
defect, and four actions had been over it for weeks. The box framed it as *either
the budget is wrong or the actions are*. **It was neither: the file's parts table
permitted `1 + 2 + 3 + 5 = 11` while its summary capped a card at 10, and the two
numbers had never been added up.** Measured over 93 shapes, 15 exceed 10 and four
of those are over with **every part inside its own stated budget**. The 10 came
off one drawing whose rule has since been reworded; the number stayed.

The cap is **12**, derived — 16 body rows − 1 separator − 3 for the next finding's
identity and two title lines — and the promise it protects gets sharper: *the pane
always shows a second finding* becomes *the next finding's name and what is wrong
with it are both on screen*. Everything below was reached by fixing that.

#### The title gets a cap for the first time, because two authors write it

A rule picks the frame; `exit_meaning` writes the translation that finishes it,
**for a code, not for a frame**. Rule 6 has three frames — 32, 53 and 58 columns —
and any can pick up any translation. Pair them with the twelve endings `exit_fact`
prints and **5 of the 36 combinations measure four lines**, a sixth landing with
zero columns to spare. **None is on a screen, and what keeps them off is not a
budget — it is `ending`'s pairings.** Move one, which this phase has done more than
once, and a card goes over with nobody having touched a word. The tightest
reachable title has **four free columns**; the `255` translation carried the word
*already* until this box and measured four lines. The word was never the defect,
the missing margin was.

#### The action is k8rs's own words, always

The one input no rule author can bound is a quoted API value, and rule 6 was the
only rule putting one in the **action** — rules 3, 10, W1 and W2 all put a quoted
controller message on the evidence line, behind the three-line cut. So a mistyped
`command` produced the *what to do* `Kubernetes recorded this: failed to create
containerd task: … OCI runtime create failed …`, which tells a beginner nothing to
do. Moving it to evidence closes the budget's only unbounded input and makes
**invariant 9 stronger**: a crafted message moves from
`one_card_per_action`'s primary key to its subset clause, where it can only
*add* a fact — the fold gets harder to satisfy, not easier.

#### The fork is the object, not the number — and the PM's own ruling was the defect

`126` and `127` were routed to `describe` on the premise that the container never
ran. **Measured, that premise is false and the counter-example was a committed
capture.** They are what a *shell inside a container that ran* reports: real
`startedAt`, real `containerID`, and the log holds the diagnosis on one line —
`tests/fixtures/notfound.json` is exactly that. Only `128`/`StartError` is the
runtime never starting a process, and that shape's `startedAt` is the **epoch**,
the field [D112](#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)
found.

**The PM ruled that `126..=128` was one code answered once and shared.** That
sentence put *the command was found but could not be run* on a card's evidence line
and *what they name is not in the image* on its action line — D85's class moved
*inside a single card*, and shared onto the CRITICAL card as well as the WARN one.
Corrected to key on whether the run **started**, which is also **positive evidence
the log is servable**: `Terminated` carries no `containerID` at all (invariant 6
pruned it), so the duration is the only proxy there is and it is strictly better
than the code. `128` was never a safe key either — a program can choose
`os.Exit(128)`. And the never-ran arm keys on the **start alone**, because a
backwards clock step gives `finishedAt < startedAt` on a container that ran
perfectly well; before that split it cost a duration line, and for one round it
would have cost a false diagnosis.

#### One ending, one answer — and sharing went wrong twice before it went right

Rules 1, 5 and 6 now answer `Ending::Failed` with one sentence and one command, and
`failed_action` is deleted. Each intermediate state shipped a defect on a committed
capture, which is the argument for the end state rather than against it:

- rule 1 keeping `failed_action` said *check the memory limit and the liveness
  probe* over its own evidence line reading *exit 127 (the command was not found)*;
- sharing `137` by the code alone handed an `OOMKilled` container `killed_action`,
  whose sentence *a kill for using too much memory is not always labelled as one*
  stood beside rule 2's card asserting the label — **three contradictions on one
  screen about one container**, always together, since rule 2's suppressor cannot
  fire inside `CrashLoopBackOff`. Fixed at the **helper**, not the caller:
  `killed_action`'s doc has always stated its precondition, and enforcing it once
  covers rule 5's identical hole and every future caller;
- rule 5 keeping `failed_action` left the CRITICAL card carrying the weaker answer
  beside rule 6's WARN card on `restarts10.json`. The stated reason — *this rule
  reads a count and has no run to ask with* — was false; it calls `ending` on the
  line above.

**And the shared sentence itself had to stop naming a speaker.** *That is where the
program said what went wrong* is false of a container the kernel SIGKILLed
mid-sentence. It promises a **place** now and never a diagnosis, which is true of an
application's error, a shell's `not found`, and an interrupted run alike.

#### What a cluster settled, and is now a citation rather than a hedge

A **sidecar is resizable** and the kubelet restarts it; a plain init container is
not — the API refuses `RestartContainer` on a non-sidecar init container outright,
so `stopped_action(Init)`'s exhaustive claim stays exhaustive. **`systemd-oomd` can
only produce `137`**, never the `143` its card was about; only `earlyoom` belonged
there. **`describe` prints no `resizePolicy`**, so the resize door names the durable
**events** rather than the transient condition. **`--event-ttl` is 1h** on kind, GKE
and AKS, and aggregation refreshes the lease, so an ongoing `Killing x5` outlives it
— in the reader's favour. `Events: <none>` on a serving container had been reading
to a beginner as *nothing stopped it*.

#### A rule may order its own doors by a fact on its card

New, and the only case. Three constraints, all load-bearing: it **reorders and
never deletes**, because the object makes a probe kill unlikely and never
impossible; the fact must be **already on the card**, so the reader can see why the
order is what it is; and the threshold is **named and derived** — `PROBE_FLOOR` is
stock probe settings putting the earliest kill at 20s, measured conservative
(third failure at ~23s, kill at 29s). The shape sent the long way round is
`failureThreshold: 1`, and the door is demoted, never shut.

#### D112 is right and narrow, and four doc comments generalised it

`lastState.terminated` **advances at every ordinary restart** — measured, the record
and its `containerID` moving with `restartCount` 2 → 3. Only the **synthesized**
write is frozen, which is `Ending::Unwatched`, which carries no `containerID` and
keeps `describe` anyway. `--previous` cannot diverge from the card even in
principle: the kubelet resolves it *through* `lastState.terminated.containerID`, the
exact record rule 6 read. **The residual is availability, not identity** — the
kubelet keeps one dead container per name, so the log is gone for the window in
which the container is `terminated` rather than running, and `doing_its_job` is
false in exactly that window.

#### The guards could not fail, and one class no guard here could see

- The whole-rule-set sweep's canary was `len() > 40` against a real 143, and every
  corpus contribution could be deleted with the sweep still green.
- Four of six `Reads` labels were untested and one was reachable: `image_not_pulled`
  mislabelled ships the undatable card for the life of the pod with **218 of 218
  tests passing**. Two more are held not by the label but by **state exclusion**,
  and the doc credited the label.
- The corpus was literal arrays with **no `match` over `Ending` anywhere in the test
  tree** — the third declaration of the residue the open box at `todo.md` ~1183
  already carries twice. Fixed rather than declared again, so a new variant stops
  the **tests** compiling: D95's mechanism applied to the guards that check D95.
- **`cargo mutants` does not mutate range patterns.** Three of the four mutations of
  `126..=128` were green. The gate was run correctly and could not have caught it.

**The general lesson is a guard, and it is this family's real output.** Every guard
in this file hunts two rules saying the **identical** sentence, because the fold's
whole purpose is to collapse repetition — so a repetition is loud and a
**contradiction is silent**. `CANNOT_BOTH_HOLD` sweeps `(claim, denial)` pairs per
container across title, evidence and action; each row is a defect this family
actually shipped, and each half is asserted reachable. Its canary paid for itself on
its first run, finding a phrase nothing produced. A card contradicting **its own
evidence line** needs its own end-to-end assertion — the pair guard is quiet there,
correctly, because the two cards agree.

#### One thing that is not a defect, ruled and recorded

Two cards about `broken-crashloop` carry a byte-identical action and command, and
they stay. The pair is the fold's **output**, not a failure: the second card appears
if and only if it carries a fact the first lacks — here the panic line, the most
useful string on the screen — and where there is nothing extra the pair already
collapses. **The caveat that makes it true**: the fold survives only because rule
6's facts are a subset of its neighbour's, so a later box adding any fact to rule 6,
**or removing one from a survivor**, stops the fold on every container rather than
only the ones with a message. That is written at `one_card_per_action`, from both
ends.

#### Two panics, in a layer that had none

`grep -c '\.expect(\|\.unwrap()' src/rules.rs` was **0** before this family. Two
arms re-unwrapped an `Option` their own `match` had already destructured — unable to
fire, and that is the point: this layer's guarantee that a missing field means no
finding is **mechanical** (`From` cannot fail), and an `.expect()` is the other kind
of promise. Bound in the match instead, and the count is back to 0 — and the
rebind deleted a redundant third lookup of the same `Option` with it.

**The part worth recording is not the two calls: it is that nothing could have
caught them.** Each was the honest local shape for *the arm already proved this*,
added while making the code more correct. `clippy` does not flag `.expect()` by
default, the mutation run cannot see a call that never fires, and `just check`
counts no panics at all — so a green build, 0 MISSED and two reviews were all
silent. The number came out of the PM's security-gate pass by hand, which is where
invariant 8's half of this lives and is exactly the leak
[CLAUDE.md § Where a leak would actually happen](CLAUDE.md#where-a-leak-would-actually-happen--the-pm-checks-these-by-hand)
says is the PM's. **A guard is owed**, and it is `tester`'s in a later phase.

### D114 — the capture trip that put four objects on disk, and the init arm that is not reachable at all (2026-08-16)

**What the box asked for.** Four objects no cluster had produced for this
repository, named by
[D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)
as what its own arms were shipping without: a probe kill reporting `exit 0`, an
`Init:CrashLoopBackOff` with a clean run behind it, a `restartCount` raised by a
node reboot, and a container the kubelet restarts under `restartPolicy: Never`.
Four landed. The one that did not is the most useful thing the trip found.

**A fifth object was folded in, and that is a scope ruling rather than a
discovery.**
[D100](#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15)
had already parked one more object on *the* capture trip and put it with the PM:
a settled gang restart. It is the same manifest field as the box's own (d) —
`spec.containers[].restartPolicyRules` — and no cluster trip is cheap, so it went
into this manifest rather than earning a second one. **The counter-ruling, made
in the same breath and in the other direction:** the debt `restarts10_ending`
names — rule 5 reaching its band through an ending that *finished* — is two more
pods on the 26-minute backoff climb and a different rule's subject, so it went to
[`backlog.md`](backlog.md). The line between them is not size. It is whether the
object was already assigned to this trip by a decision.

**`Init` + `CrashLoopBackOff` + `exit 0` is not reachable, and D90 said it was.**
That entry reasoned from the kubelet source: `doBackOff` selects by container name
and `ContainerStateExited` and never reads the exit code, so a sandbox rebuild
should put a cleanly exiting init container into the same backoff as a failing
one. **The source read is correct and the conclusion is wrong**, because the two
are about different objects. Measured on kind v1.36.1:

- `doBackOff` does fire — `describe` carries
  `Warning BackOff (x9 over 12m) spec.initContainers{warmup}: Back-off restarting failed container warmup`.
- The **published pod status never rests there**. `state.waiting` was sampled about
  120 times — across 51 sandbox rebuilds, then 25 spaced rebuilds, then 40
  consecutive polls immediately after a `crictl stopp`. Zero hits. The init
  container reads `state.terminated / Completed`, `Ready: true`, every time.
- The control, on the same cluster in the same minute: `broken-init`, whose init
  container exits **1**, reads
  `state.waiting {reason: CrashLoopBackOff, message: "back-off 5m0s restarting failed container=migrate…"}`.

The kubelet publishes that waiting state for an init container it is **waiting to
retry**. One that exited 0 is *finished*; nothing is waiting to retry it, so the
event fires at rebuild time and the status has already moved past it.
[`rules.rs`](src/rules.rs) reads the published status and nothing else. **So the
`Init` arm of `finished_action` stays planted on a decoded copy permanently**,
on the same footing as the CRI-O runtime-socket mount — not "until the capture
trip" ([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12),
[D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)).
The manifest, the predicate and the guard were **removed** rather than left
failing: a predicate that can never pass blocks every future capture at `verify`,
which is worse than having no fixture at all.

**A node reboot does not write `ContainerStatusUnknown` either, and the brief that
said so was quoting D90.** Superseded by the measurement one day later
([`reports/2026-08-16-terminated-record-stamps-and-authors.md`](reports/2026-08-16-terminated-record-stamps-and-authors.md)
§ 2, which the `STATUS_LOST` doc comment already carried): containerd's state
survives the reboot, the containers are *found* dead, and
`internal/cri/server/restart.go:353-357` writes `(255, "Unknown")` — which is
`Ending::CodeUnknown`, not `Ending::Unwatched`. `crictl rmp -f` on the sandbox
is `ContainerStatusUnknown`'s producer and a reboot is not. Caught by `tester`
against the PM's brief, *before* the predicate was written; one aimed at the
unmeasured shape would have burned the timeout and then failed a fixture that was
correct all along. **So `broken-reboot` retires the `CodeUnknown` plant, not the
`Unwatched` one** — the object the box asked for, answering a different rule than
the box thought it would.

**What landed, and what each bought — which is less than "four plants retired",
and the difference is worth writing down.** A capture retires a plant only when it
carries the *whole* shape the plant was standing in for. Three of these four do
not, and the plants stay beside a new test that reads the real bytes:

| pod | the bytes | what it bought |
|---|---|---|
| `broken-probe0` | a liveness kill the program reported as `exit 0`, on a run of **32s** | **a plant retired** — `each_ending_sends_the_reader_somewhere_the_answer_can_be`'s long arm was `exit0.json` with `startedAt` moved by hand. The *other* `PROBE_FLOOR` plant stays: it needs rule 1's card, and `probe0` draws rule 5's |
| `broken-reboot` | three restarts a node caused, on a container serving now: `(255, "Unknown")` | `Ending::CodeUnknown` is captured, and `CODE_UNKNOWN`'s doc claim that no committed capture reaches it was false and is corrected. The CRI-O `-1` half stays source-derived |
| `broken-neverrules` | restarted under `Never` by a rule on its own exit code | `stopped_for_good`'s `restarts != 0` guard gets its first object, and a test that asserts the silence *and* attributes it — the count moved to 0 on a decoded copy makes the card appear |
| `broken-gang` | `{137, RestartingAllContainers}`, no stamps, on **both** containers, beside a live `startedAt` | **both `RESTART_ALL` plants stay**, and that is the honest outcome: this is a *settled* gang restart, and the parked shape one of them is about lives in `state.waiting` for about two seconds a cycle. A captured test sits beside them |

**The gang object confirmed D100's own reading on first contact**: the synthesized
record lands in *every* container's `lastState`, the trigger's included, so no card
drawn off that reason may call this container the innocent one. Both containers
settled at `restartCount: 3`, `2/2 Ready`.

**A crash loop has two faces, the capture predicates always knew it, and the tests
did not.** `scripts/cluster.sh`'s `[crashloop]`, `[init]` and `[owned]` each read
`waiting: CrashLoopBackOff` **or** `state.terminated`, with the measurement beside
them — sampled 70 times, the container was terminated 39 and waiting 29, and
demanding the waiting reason is called *the too-tight half* there. This trip's
captures landed on the other face from the last one's, and 56 tests went red: they
had been pinned to whichever face the previous capture happened to hold. **The
bytes were right and the assertions were over-fitted**, so what changed is the
tests — where a test's subject is an action or a grouping it now selects by
ending, container and position rather than by the face, which is the shape rule
3's `ErrImagePull || ImagePullBackOff` already had.

**And the second face was hiding a live defect in rule 13.**
`nothing_else_to_point_at` counted a container as something to point at only when
it was `Running`, or `Waiting` with a reason — so a container in
`state.terminated` counted as neither. On `broken-init` *between* runs, rule 13
therefore drew *"This pod was given a machine to run on, but it has not been able
to start · container `app`"* while rules 1, 5 and 6 drew the true card about
`migrate` one row down: D27's blind spot inverted, live for the ~56% of a crash
loop that sits in `terminated`, and invisible for as long as the corpus held only
the other face. The title is the argument for the fix — *has not been able to
start* is false of a pod holding a container that ran and stopped.

**And the first fix for it was too broad, which the operator review caught by
running the shape the rule exists for.** Counting *any* `Terminated` container as
something to point at silences rule 13 on a pod whose init containers **finished**
and whose main container then cannot be created — a hung image pull, an
unresponsive containerd, an Istio or `vault-agent-init` sidecar pod. The kubelet
keeps writing `PodInitializing` onto the regular containers for as long as the pod
declares init containers at all (`hasInitContainers` is
`len(pod.Spec.InitContainers) > 0`, nothing about whether they finished —
`kubelet_pods.go:2119-2125`, `:2499-2501`), and every other rule leaves: 10 and 14
on `PodScheduled: True`, 1/5/6 on the `lastState` a finished init container does
not have, 7 on `Running`. Fourteen hours in, `analyze` returned **nothing**. What
ships is the narrow clause — `Terminated(run) if ending(run) != Ending::Finished`
— which keeps the original fix (`broken-init` between runs still draws no rule-13
card, the whole-capture count stays 24) *and* keeps this card. **The clause that
matters is the clean exit**: a pod whose init step succeeded did get that far, so
its silence is earned; one whose container ran and *failed* has something better
to point at. Three states were each measured red or green rather than two.

**Re-pinning `now` is an edit in two ownership rows, and the second one is a
guard.** The corpus moved to `2026-08-17T00:00:00Z` (midnight after the newest
capture, the pin's own rule). `src/rules_tests/snapshot.rs` is `dev-core`'s;
`scripts/certs-test.sh` extracts that same pin and refuses to disagree with it, and
it is `tester`'s. A repin that stops at the first file is a red build in somebody
else's tree — which is the guard working, and it is written into both files so the
next person meets the note before the failure.

**Two findings the run produced that the trip did not fix**, both in
[`backlog.md`](backlog.md) rather than boxed into a running phase: `assert_states`'
report pass re-fetches and can therefore contradict the wait loop it just ran — a
false red on any state that is transient by design, which cost one whole slow pass
— and rule 5's finishing endings, above. Neither is folded in, because
`assert_states` is the shared helper every state runs through and CLAUDE.md keeps
a shared-helper change per box.

### D115 — the prune line bounds memory and was read as if it bounded time, and the paint budget is stated at a cluster size the risk is not (2026-08-18)

Invariant 6's prune has been corrected twice, and both corrections were about
*which* fields survive it:
[D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)
replaced "metadata + status only" because five rules read `spec`, and
[D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)
found `spec.restartPolicy` named by no snapshot type at all. Both are right.
Neither noticed the thing underneath them: **the prune cannot run on the wire,
so the field list decides memory and nothing else.**

There is no mechanism to ask the API server for a subset of `status`.
`PartialObjectMetadata` returns metadata alone, and the pod rules read
`status.containerStatuses` and `status.conditions` between them — rule 10 and
the never-judged-pod rule live entirely in the second, which is why "read the
container statuses only" is not a way out either. So the full object is
serialized by the API server, sent, and deserialized by us, and *then* fields
are dropped. **Pruning removes no byte from the network and no
microsecond from the decode.** It bounds the resident set, which is exactly what
`REQUIREMENTS.md`'s `< 50MB RSS at ~1000 pods` asks for, and it is the right
mechanism for that number.

**What made the misreading easy** is that invariant 6 states the prune in the
same breath as a cost claim — *"a periodic `LIST pods -A` is what makes k9s
heavy"*. That is true, and watch-not-poll genuinely fixes it: the repetition is
the cost, and the repetition is gone. But the sentence reads as though pruning
is also what makes the **first** LIST affordable, and it is not. Nothing has yet
been *decided* that makes the first LIST affordable — the box that will decide it
is the pagination box directly below the prune box, and it is unbuilt, not
missing.

**Which leaves a range nobody has defined.** The budget is written at ~1000 pods
(`first paint < 1s, findings < 3s`). The pagination box argues from a 10 000-pod
cluster ([PRIOR-ART § A2](PRIOR-ART.md#a2--the-initial-list-must-be-paginated-and-the-page-size-is-a-decision)).
No document says what happens between them. And the usual escape — paint the
first page, let the rest stream in — is closed by the correctness rule sitting
in the same Phase 5 box: no snapshot is published until every initial LIST has
landed, because a rule cannot tell a partial list from a small cluster and
invariant 5 leaves it no way to ask
([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)).

Three claims, each correct on its own, and they cannot all hold at every cluster
size:

1. the prune bounds memory (invariant 6 — and it does not bound latency);
2. nothing is published until every initial LIST has landed (correctness, D28);
3. first paint under a second (`REQUIREMENTS.md`).

**That they collide is structural and needs no measurement.** The initial LIST
grows with the cluster and has no size-independent bound, so above *some* size it
exceeds any fixed budget, and 2 forbids painting anything before it lands.
**At what size is unmeasured, and this entry invents no figure for it**: D25's
rule holds, and the two numbers standing near this box — ~1000 in the budget,
10 000 in the pagination box — are the sizes those documents were *written* at,
not a measured crossing point. Finding the crossing point is the work; knowing
that one exists is not.

**The third claim is the one that gives, and it gives by naming a size.** Not
because latency matters least — because 1 and 2 are correctness and 3 is a
target, and a target that silently stops being true above some size nobody has
measured does not report as a missed target. It reports as *"slow on a large
cluster"*, which is the sentence k9s carried for six years while the cause sat
in one unpaginated call. So the budget becomes a budget **at a stated cluster size**,
with a stated behaviour above it: a first paint that says what it is still
waiting for and how far along it is, rather than a number that quietly expires.

**No mechanism is decided here** — page size, whether the count is known early
enough to show progress at all, and what the decode actually costs are Phase 5
measurements, and
[D25](#d25--what-this-review-did-not-decide) already refuses to write that from
a guess. What is decided is which of the three gives, so that the measurement
has something to report against.

**Invariant 6's wording is left alone.** It says watch-not-poll and
pruned-to-named-fields, and both are true; what was wrong was only the reading.
The correction belongs where the reading happens — one sentence in the Phase 5
box saying what the prune does *not* buy, beside D69's and D97's two sentences
about which fields it keeps.
### D116 — the environment picker moves to startup, and the tag comes out of the kubeconfig itself (2026-08-19)

[D16](#d16--the-context-switcher) closed *"switchable"* with `X` and a modal
picker, and left the launch path exactly where it found it: `k8rs` reads
`current-context` and connects, silently. That is the wrong default for the
person this tool is for — someone with `aws-prod`, `aws-staging`, `kind-k8rs`
and two more in one file, whose `current-context` is whatever the last
`kubectl` command left behind. The screen then says `ctx: aws-prod` in eleven
dim characters, which is a header nobody reads until after they have acted on
it (user, 2026-08-19).

**The startup picker is the `X` picker, called earlier.** No second screen, no
second key map, no second code path: [screens/context.md](screens/context.md)
grows a startup section and `connect()` — already re-callable by
[D16](#d16--the-context-switcher)'s fourth ruling — is what runs on `⏎`. The
only genuinely new behaviour is *when the modal is on screen before there is
anything behind it*, and that is one rule: at startup `esc` quits, because
there is no cluster to go back to. Mid-session `esc` still cancels onto the
cluster you are already on.

**At startup it opens only when there is a real choice: two or more contexts
and no `--context`.** One context in the file, or a context named on the command
line, connects straight through as it does today. *Zero configuration on first
run* ([§ Positioning](#positioning--lazygit-for-kubernetes-user-2026-08-11),
item 3) stays literally true — the picker asks nothing that is not already in
the user's file, the current context is preselected, and `⏎` lands where the
old default landed. Precedence, once and in one place: `--context` beats the
picker, the picker beats `current-context`. `--once` and any non-tty stdin
never open it — a picker in a pipeline is a script that hangs forever, and
`--once`'s contract is stdout and an exit code.

**The tag is a field of the kubeconfig, not a file of ours.**
[§ Out of scope](#out-of-scope-the-most-important-section) refuses a config
file, and a tool that mixes up clusters is not fixed by adding a second file
that can disagree with the first. The user's own label lives where the context
lives:

```yaml
contexts:
- name: aws-prod
  context:
    cluster: prod-eu
    user: admin
    extensions:
    - name: k8rs
      extension: { tag: "aws · prod" }
```

`kube::config::Context` already parses that field — `extensions:
Option<Vec<NamedExtension>>`, verified on docs.rs 2026-08-19 — so this is a
lookup, not a parser, and no new dependency (invariant 10) and no second reader
of the file. A tool that rewrites the context entry (`aws eks
update-kubeconfig` and friends) can drop the tag with it; the column then falls
back to the derived one, which is the safe direction and the reason the
fallback is not optional. `X` mid-session is unchanged and still opens on a
single-context file, saying so — that one is an explicit keypress, and a key
that appears to do nothing is worse than a screen that explains why.

**k8rs does not write the tag, for the same reason it does not write the
switch.** Nothing in this repo edits `~/.kube/config`
([D16](#d16--the-context-switcher)). The picker shows the YAML block above in
`?` help for a context that has no tag, and shows **YAML, not an invented
`kubectl config set` line** — `set` does not address a list element under
`extensions`, and a command log that teaches a command which does not work is
the dishonesty [D8](#d8--invariant-4-was-not-literally-true) exists to prevent.

**An untagged context still gets a tag, and it names the provider — never the
environment.** Derived from the API server host and the context name:
`amazonaws.com` → `aws`, `gke`/`googleapis` → `gcp`, `azmk8s.io` → `azure`,
loopback or a `kind-`/`minikube`/`docker-desktop` name → `local`, anything
else → blank. That is the whole heuristic, and the boundary is the point: a
guessed `prod` on a cluster that is not prod is worse than an empty column,
because it is the exact mistake the feature was added to prevent. **Only the
user's own tag may make an environment claim**, and the screen has to show
which of the two it is looking at — a derived tag is a guess about a hostname,
a written one is a statement by the person who owns the cluster.

**Four things the screen settled that this decision did not** (drawn in
[screens/context.md](screens/context.md), which is where the columns and the
wording live — they are not repeated here):

- **A derived tag is marked on the row, not only coloured** — dim *and*
  `~`-prefixed (`~aws`, `~local`). Colour alone cannot carry "this is a guess"
  through a monochrome terminal or a pasted screenshot, which is the rule the
  severity symbols already follow.
- **A context whose cluster the file does not define is drawn but unreachable**
  — dimmed, and the cursor skips it. The kubeconfig parse knows this before the
  first keypress, so spending a connection attempt to discover it, and a
  failure modal to report it, would be a round trip for a fact already in hand.
- **A connect failure after the picker has drawn is a modal, not stderr.** The
  picker puts the terminal in raw mode *before* the first connection, so two of
  the three startup errors in [screens/states.md](screens/states.md) now have
  two forms: stderr and exit when no picker opened, a modal that returns to the
  list when one did. The stderr path is not replaced, it is narrowed.
- **`⚠ TLS not verified` beats `(current)`** where a row is both and one badge
  column has to hold it: the safety warning is the one the user has not already
  been told by the cursor's own position.

Two things this does not move. The list is still every context in the
kubeconfig and nothing else: no typed server, no token, no cluster the file
does not define ([invariant 3](CLAUDE.md)). And context names, tags and server
addresses are text off a disk file, which is untrusted in exactly the way an
API string is — same `sanitize()`, same bound on length, and a tag is one
short column, not a place to render forty characters somebody pasted.

### D117 — the plain-language pass, and the two things it found that were not sentences (2026-08-19)

The box named three defects and struck one out. What the pass actually settled,
recorded here because a wording chosen by whoever happened to be writing is the
kind of decision that gets re-argued every time somebody reads the card:

- **The count is spelled by `counted` everywhere, and nowhere by hand.** Rules 1
  and 2 carried the same `format!("{} restarts", …)` — a copy, not a
  coincidence — and rule 5 carried a third wording of the same defect in its
  title. `counted` has existed since the age ladder was written; the sites that
  did not call it simply predate the habit.
- **`read the last run's log`, not `read that run's log`.** *That run* is exact
  under rule 6's title, which names a run, and dangles under rule 1's *keeps
  crashing* and rule 5's restart count, which name none. One sentence serves
  three cards, so it has to be true on the card that names the least.
- **The action glosses `--previous`, at the end.** It is the one word on these
  cards a first-month reader cannot guess, and it is the difference between the
  log of the run that failed and the log of the one running now. A flag printed
  and left is [invariant 14](CLAUDE.md) in the small; the command log is where
  this tool teaches ([invariant 4](CLAUDE.md)). It goes last because the
  instruction is the sentence and the gloss is the gloss.
- **Exit 128 says *the node*, not *the runtime*.** The two `CODE_UNKNOWN` rows
  beside it already name the node for the same layer
  ([D112](#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)),
  and a third row introducing a second word for one actor teaches a term
  nothing on the card explains.
- **N6 says *the cluster's one node does not have*, on a one-node cluster.** Not
  in the box, found by the sweep: `none of the 1 node have that label` is
  `1 restarts` one rule over, and it lands on kind, minikube, k3s and Docker
  Desktop — the clusters this tool is pointed at most. The multi-node string is
  left byte-identical, which is what keeps
  [screens/alerts.md](screens/alerts.md) § N6 true rather than merely unchanged.

**Rule 5's singular cannot be drawn, and the change was kept anyway.**
`restarting_repeatedly` returns before its title on any count below
`RESTARTS_WARN` (3), so `restarted 1 times` reaches no reachable card and **no
test can fail on it** — the pass proved that by reverting the line and watching
the suite stay green, which is the honest form of *this has no red*. It is kept
because the sentence is otherwise one threshold edit away from being wrong, the
edit is one line through a helper that already exists, and the output is
byte-identical at every count the rule can reach. What is not claimed is
coverage: this one line is asserted by nothing, and that is written here rather
than papered over with a test that cannot fail
([D26](#d26--a-green-build-that-proves-nothing-2026-08-12)).

**Two things the pass deliberately did not touch**, both because they are claims
rather than phrasings — the same reason the box's own (b) was moved out of it
before it started. Rule 2's evidence keeps its bare `exit 137`: routing it
through `exit_fact` would print the kernel sentence one line under a title that
already says it, which is
[D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)'s
two-spellings-on-one-screen. And rule 5's *", but something keeps killing it"*
asserts an external killer over evidence that, on the committed corpus, reads
*the application's own error* — that is a rule's claim about who acted, it is
pinned in two other files, and it goes to [backlog.md](backlog.md) for a ruling
rather than into a wording box.

**The box's own text was wrong in both halves and was corrected from a count
rather than from memory** — it said the phrase was spelled twice, in rules 1 and
5; it is spelled three times, and the identical pair is rules 1 and 2. That is
the same defect class this file keeps recording
([D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)):
a number written from memory instead of from a run.

### D118 — a foreground call is capped at ten minutes and the phase-close sweep is longer than one (2026-08-20)

`/basla` also runs unattended, as a single-turn `claude -p` process: when the
turn ends the process exits and every background job dies with it, so a sweep
started in the background is a sweep nobody ever reads. The rule that follows is
*anything you must wait for runs in the foreground* — and that rule, written on
its own, cannot be obeyed for the one gate it was written about. **A foreground
call is capped at ten minutes; the whole mutation sweep is longer than one.**

The numbers are read off runs, not reasoned about
([D104](#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)):
the sweep is **558** mutants across `rules.rs` and `analysis.rs` today, against
the **519** it carried on 2026-08-16 ([todo.md](todo.md) Phase 3, the sweep
box), and the four shards took 2m41s, 2m53s, 2m44s and 2m52s — **11m10s of wall
clock**, past the cap with the file still growing.

**So the phase-close sweep runs in shards** — `--shard k/4`, one foreground call
each, **all four green or the gate is not passed**.

**`k` is zero-based**, so the four are `0/4 1/4 2/4 3/4` and `4/4` is not the
fourth shard but an error — *shard k must be less than n*. That is worth a
sentence rather than a footnote, because the run that ends in that error prints
**no `MISSED` line at all**, and a gate read by grepping for `MISSED` therefore
calls a quarter of the mutants green without having run one of them. This entry
had it wrong in its first draft and the run is what corrected it: shard `0/4`
holds ten of the fifteen misses on the file today. Two things this does *not*
change. The per-turn gate is
`--in-diff` and already fits in one call, so it is untouched. And `--iterate`
narrows what a *later* run does, never what one call costs, so it is not a
substitute for sharding.

**The `justfile` keeps its recipe.** `just mutants` is `tester`'s file and a flag
typed at a gate is not a recipe change; if the sharding should become one, that
is a box, not an edit the PM makes in passing.

### D119 — the last surviving mutant was equivalent, and the fix is to stop spelling the tie by hand (2026-08-20)

The sweep box closed at **one** survivor, not zero:
`src/rules.rs:3982` in [`running_but_not_ready`] —

```rust
let since = match started_at {
    Some(began) if began.0 > unready_since.0 => began,
    _ => unready_since,
};
```

`>` and `>=` differ on exactly one input, `began.0 == unready_since.0`, and there
the two arms return the same value: `Time` is
`pub struct Time(pub jiff::Timestamp)` — a **single-field newtype**, so equality
of `.0` is equality of the whole, and `since` is only ever read through
`duration_since(since.0)` and `since.clone()`. Both spellings therefore produce a
byte-identical card. **It is an equivalent mutant and no test can kill it**;
writing one would be a test that cannot fail, which
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) forbids more strongly
than the sweep asks for a kill. Verified rather than argued, and **twice**: with
`>=` applied the suite was green at 231 passed mid-turn, and `tester` re-ran the
same two spellings against the delivered tree — `>` and `>=` — and got
**232 passed, 0 failed** from both. The first number is quoted because it is the
one this ruling was made on; the second because it is the one a reader can
reproduce, and a claim whose whole point is *verified* has to be re-runnable at
the count the tree actually has.

**So the ruling is not a test and not an exemption — the tie stops being spelled
by hand.**

```rust
let since = started_at.as_ref().map_or(unready_since, |b| b.max(unready_since));
```

`Time` derives `Ord`, and `Ord::max` **returns its second argument when the two
compare equal**, so the tie resolves to `unready_since` exactly as the guard did.
The behaviour is unchanged, the line now says *the later of the two moments*
instead of making the reader derive it, and there is no binary comparison left
for the tool to mutate.

**The ruling has a price and it is stated here rather than in the report only:
the sweep loses five mutable sites and the file goes 558 → 553.** `map_or` and
`Ord::max` are a method call and not a binary operator, so the tool generates
nothing there — the floor's correctness is now held by three tests and not by the
sweep. That is the trade, taken knowingly: the three cells (`began` after,
before, and equal to the condition) assert the behaviour, where the sweep only
ever asserted that *some* test noticed a flip. What was **not** given up is the
rest of the function — the `NOT_READY_GRACE` comparison one line below still
generates its mutant and is still caught, which is exactly what
`#[mutants::skip]` would have thrown away.

**That sentence said *six* in its first draft, and 558 − 6 is not 553.** Both
endpoints had been read off `cargo mutants --list`; the difference between them
was done in my head, which is the one step of the three nobody ran. It is the
leak [CLAUDE.md](CLAUDE.md#where-a-leak-would-actually-happen--the-pm-checks-these-by-hand)
names in as many words — *a number written from an estimate instead of a run* —
landing in the entry that opens by saying the numbers are read off runs. Caught
by the operator review re-measuring both ends.

**The two rejected options, and why.** `#[mutants::skip]` on the function would
have blinded the sweep to everything else in it — the `!c.started` gate and the
`NOT_READY_GRACE` comparison are killable and are killed — to hide one line.
Recording the survivor and leaving the gate at *1 missed* is worse: a gate whose
green is a non-zero number is a gate nobody reads, which is the degradation
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) and the *derived list
asserts it found something* rule both exist to prevent. **`rules.rs` is Phase 3's
own layer**, so this is a change inside the open phase, not a frozen file
reopened.

**What the turn settled that the brief did not**, recorded because a choice made
by whoever was writing is the kind that gets re-argued:

- **The duration ladder is asserted at eleven spans, through both `lasted` and
  `ran_for`.** The spellings come from `lasted`'s own doc, from `counted`
  ([D68](#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13))
  and from [D83](#d83--the-hours-rung-runs-to-48-and-the-age-ladder-gets-one-home-2026-08-14)
  for the 24-hour boundary — not from what the code returned. D83 protected that
  boundary **in prose**: *`lasted`'s identical `as_hours() < 24` was deliberately
  left alone … so nobody closes the gap with a find-and-replace*, `age` having
  moved to 48. `86399s → 23 hours` and `86400s → 1 day` are now asserted, so the
  find-and-replace D83 asked nobody to do is caught by a test rather than by a
  sentence.
- **`short_of_pods` is asserted on a *paused* Deployment whose template
  changed** — a state where upstream creates no new ReplicaSet, so
  `updatedReplicas` is absent while nothing is unavailable. The test also asserts
  `analyze` stays silent there, so it is not arguing that a paused release should
  page anyone.
  **The reach of that arm is wider than the first draft of this entry said**, and
  the correction is `tester`'s: the draft called the paused state *the one stable
  real state the arm is load-bearing in*, on the reasoning that
  `unavailableReplicas = sum(rs.spec.replicas) − available` swallows every
  quota-refused **or surging** rollout. The surging half does not hold at two or
  more replicas — at `replicas: 2` the defaults resolve to `maxSurge: 1,
  maxUnavailable: 0`, and in the sync where the surge pod becomes available
  upstream scales the old ReplicaSet down in the same pass and persists
  `readyReplicas: 3, updatedReplicas: 1` with `unavailableReplicas` absent, which
  is this arm, unpaused. **Read out of
  `pkg/controller/deployment/{sync,rolling}.go`, not measured** — so it is
  recorded as reasoning and the measurement is boxed in
  [backlog.md](backlog.md). It does not weaken the test: the arm is load-bearing
  in *more* states than claimed and the plant is still a real shape. The same
  over-narrow sentence is in `short_of_pods`' own doc and is corrected with it.
  **The operator review then sharpened it once more**: the paused state is one
  W2 can never reach *on a card at all*, because its `Progressing` reason is
  `DeploymentPaused` and `rollout_gave_up` returns before `short_of_pods` is
  called. So the arm is load-bearing **on a card** in a *timed-out surging*
  rollout — `desired 3, maxSurge 1, maxUnavailable 0` giving `ready 3 ≥ 3`,
  `updated 1 < 3`, `unavailable 1 > 0`, where arm 2 decides the evidence line.
  The test is a sound helper test and stays; the sentence around it was a degree
  too strong twice over.
- **Rule 8's exemption is asserted as a 2×2 truth table**, and the
  DaemonSet-outside-`kube-system` cell is pinned as **an open ruling**, not as a
  requirement — this entry's first draft said *requirement* and the operator
  review sent it back.
  [D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)
  does not record a requirement: it records a narrowing *"correct on the cluster
  it was tested against and known to be wrong beyond it"* and asks for evidence.
  The test's job is therefore to stop the exemption being widened **silently**,
  not to say the card is wanted — and the evidence D70 asked for arrived in the
  same review that caught the wording
  ([D120](#d120--the-two-things-the-operator-review-measured-that-the-tests-cannot-see-2026-08-20)).
- **The OOM boundary extended an existing test rather than adding one** — same
  capture, same subject, one more rung.

**A pre-existing defect found and left alone**, because it is outside this box
and outside the writer's files: `src/rules_tests.rs:475` and `:487` cite a script
subcommand `break-nodes`, and `scripts/cluster.sh` has no such subcommand — it is
`break` / `unbreak`. Boxed in [backlog.md](backlog.md) rather than fixed in
passing.

### D120 — the two things the operator review measured that the tests cannot see (2026-08-20)

The family review for the mutation boxes stood a cluster up
(`K8RS_CLUSTER=review`, one worker, torn down) and came back with two findings
that no test in this repository could have produced, because both are the spec
being wrong rather than the code disagreeing with it. Measurements:
[reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md](reports/2026-08-20-pod-rule-family-clocks-and-host-mounts.md).
**Neither is fixed here.** Both are rule behaviour, both reverse something this
file already decided, and a reversal is a ruling with the user in the room — so
what this entry does is record them, name what was measured, and put the fix in
[backlog.md](backlog.md) rather than into a phase that is trying to close.

**1. Rule 8's `/` escalator is the evidence
[D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)
asked for, and it points the other way.** D70 narrowed the exemption to
`kube-system`, weighed the *writable* escalator and the namespace, and closed
with *"what would settle it is evidence, and there is none yet."* The evidence:
`prometheus-node-exporter` — the DaemonSet `kube-prometheus-stack` installs by
default, with `hostRootFsMount.enabled` defaulting to `true` — declares a
`hostPath: /`. The `/` arm fires **on the path alone**, before the exemption:
read-only or not, `kube-system` or not, DaemonSet or not. Measured, two pods,
both `mountsReadOnly=/host/root:true`, both `Severity::Critical`, both
`timestamp: None` — so **one permanent CRITICAL per node-agent DaemonSet, with
no clearing condition**, on any cluster running the commonest monitoring stack
there is. Longhorn, the case D70 argued about, is the *smaller* one: three or
four cards, on clusters whose operator recognises the names. The action makes it
worse than noise — *"mount only the directory the container actually needs, not
the root"* — because the directory node-exporter needs **is** the root
(`--path.rootfs=/host/root` is how it enumerates mountpoints). A reader who
follows the loudest card on the screen breaks their monitoring, which is
[invariant 13](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)'s
first half failing outright. Note the asymmetry inside one function: the socket
arm names its legitimate holder
([D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13)),
and the `/` arm — the one with the commonest legitimate holder in the
ecosystem — names none.

**2. Rule 7's floor is one variable doing two jobs, and only one of them is
right.** `since` both dates the card and is the moment `NOT_READY_GRACE` is
counted from. `Ready` is **pod**-scoped and its `lastTransitionTime` moves only
when the status changes, so a container replaced under an already-unready pod
does not move it — measured across seven restarts, `startedAt` walking
`23:29:37 → 23:34:35 → 23:40:47` while `Ready=False` stayed pinned at
`23:29:33`. Two consequences, one visible and one silent:

- **The card understates the outage.** A pod out of the Service for three hours
  whose container restarted fifteen minutes ago renders *"15 min ago"*. The title
  names the **Service**, and the Service has been short this pod for three hours
  however many times the container was replaced.
  [D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)'s
  argument — *a container cannot have been out of the Service for longer than its
  current run has existed* — is true of a container and false of the thing the
  sentence names.
- **The rule goes silent exactly where it is most needed.** Where the container
  keeps being replaced, `now − since` never exceeds the restart interval —
  measured at 60s early and 3m47s at restart 7, and the kubelet's back-off caps
  at 5 min, all under the 10-minute grace. Rule 5 covers it from `restarts ≥ 3`
  and rule 6 whenever the ending is `Failed`; a well-behaved app exiting `0` or
  `143` on SIGTERM is exempt from rule 6, so with a lenient liveness probe there
  is a window with **no card at all**.

The shape the floor genuinely protects is `c.restarts == 0` — the pod that spent
twenty minutes pulling an image before any container existed. Past the first
restart, the restart is itself the proof the pod was already unready before this
run began.

**What is done in this phase instead of the fix:** the gap is named in
`running_but_not_ready`'s own doc, citing this entry, so the next reader of that
function is not left to rediscover it — and the pre-existing test that pins the
coupling keeps pinning it, because until the ruling lands, *unpinned* is worse
than *pinned and documented as wrong*.

### D121 — the temporary driver and the three places it does not draw what the console will (2026-08-20)

Phase 3's last box is a driver that reads Kubernetes objects out of JSON files
named on argv, runs [`analyze`](#d17--the---once-output) over them and prints
the result. It is `--once`'s ancestor and is not `--once`: `k8s.rs` is Phase 5,
so nothing here reaches a cluster. **It may not invent a third format** — two
renderers over one `rules.rs`, and a third would be the lie
[screens/once.md](screens/once.md) is built on — so it draws that file's card,
`● ▲ ○` and all, and diverges in exactly three places, each of which is written
into the file's own module doc:

1. **No owner grouping and therefore no `3 of 5 pods`.** One card per finding.
   Grouping is `views.rs`, Phase 10, and doing it twice is how the two copies
   come to disagree.
2. **Severity is the whole sort key.** `sort_by_key` is stable, so `analyze`'s
   own order survives inside a band; recency is Phase 10's second key.
   [D35](#d35--just-mutants-is-a-check-that-cannot-fail-and-the-justfile-unfreezes-for-one-line-2026-08-12)
   is where *declaration order is severity order* is ruled, in its own third item.
3. **It draws the `Info` band, and `--once` will not.** `Severity::Info` says
   nothing on the Alerts list ([D2](#d2--the-dividing-line-broken-now-vs-risky-later),
   [D87](#d87--c1-has-two-bands-and-they-belong-on-two-screens-d2-only-ever-ruled-on-one-of-them-2026-08-14)),
   and `screens/once.md` is the Alerts view with the frame off — so the released
   renderer takes the band off, and Phase 5's `--once` box owns that. This one
   keeps it, because a driver whose whole job for two phases is *show me what
   `analyze` returned* may not silently drop one of them. It is unreachable
   today anyway: C1 is the only `Info` producer `analyze` calls, C1 needs
   `client_certificate`, and a file path is not a kubeconfig — which is why the
   whole capture set prints `20 critical, 9 warnings` and no notes.

**Two additions to the header are this driver's and not the console's**, for one
reason: *read nothing* and *found nothing* must not print the same line. The
counts carry `workloads` beside pods and nodes, and a kind no rule reads is
counted and named — `4 objects no rule reads (CertificateSigningRequest,
Service)` over the committed captures. `screens/once.md`'s header has neither,
because a cluster connection cannot be handed the wrong file.

**The label for a document that names no kind is `(no kind)`, one label for four
shapes** — no `kind` field, `{"kind":42}`, a top-level array, a bare `null`.
*No kind field* over a document that has one is the same lie, one size down, as
a report misnaming what it read.

### D122 — the strip goes on the value entering the sentence, not on the finished sentence (2026-08-20)

`sanitize` is `char::is_control` — Unicode's `Cc` category, which is the C0
range, `DEL` and the C1 range a UTF-8 stream carries as `\u{80}`–`\u{9f}`. That
is [invariant 9](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)'s
class exactly, and the strip removes rather than substitutes, because *stripped*
is the word in both the invariant and
[screens/widgets.md § 7](screens/widgets.md#7-text-that-came-from-the-api) and a
substituted space is a character the API did not send.

**Where it is applied is the decision, and the first draft got it wrong in the
way that only shows up on the unhappy path.** The driver ran the strip over the
*assembled* stderr message. `'\n'.is_control()` is `true`, so it ate the line
breaks out of k8rs's own three-line usage text and printed three sentences as
one run-on line — the first thing a new user sees. Every one of the 255 unit
tests stayed green through it, because they all asserted on `run`'s `Err`
*value*, which was correct; what was wrong was what `main` did to it afterwards.

So: **every fragment that came from outside is stripped at the `format!` that
interpolates it** — a `Finding`'s fields, a path off argv, an error string from
`serde_json`, the standard library or `jiff` — and every literal in the file is
ours and stays whole. A `\n` *from the cluster* still dies, and must: it would
forge a second card. Phase 5's ingest strip supersedes this by applying the same
rule one layer earlier, on the way in, which is why no renderer after it has to
remember.

**The `Finding` surface is treated as outside wholesale**, its title and action
included: those are k8rs's own sentences but they interpolate API text, and a
per-field judgement call is how one field gets forgotten.

### D123 — the mutation gate has nothing to say about main's body, so a test drives the binary (2026-08-20)

`cargo mutants --in-diff` is [step 4](CLAUDE.md#step-4-is-the-anti-leak-mechanism-so-a-machine-runs-it)'s
whole gate, and on this box it reported `MISSED 0` **across the entire life of
the usage-text defect above**. It is not a flaw in the tool: mutants replaces a
function's body, and the only mutation it has for `fn main()` is *replace with
`()`* — so a defect in how `main` acts on a correct value from a tested helper
is not expressible as a mutant of either. The gate is a floor, not a ceiling,
and `main`'s body is the one place it is silent.

The one mutant it did have — `replace main with ()` — could not be killed from
`src/`: `CARGO_BIN_EXE_k8rs` is set only for a target under `tests/`. So
`tests/binary.rs` lands now, two phases before Phase 7's end-to-end box, and it
asserts the three things `main` does that nothing else can watch: argv, the
choice of stream, and the exit code.

**This is not the lib target [D50](#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12)
refused, and does not open a door to one.** It links no product type, reaches
nothing private and calls no function in `src/` — it runs the built binary and
reads its two streams, which is the use D50 explicitly reserves `tests/` for
("the Phase 7 end-to-end tests that drive the binary rather than call into it").
What moved is the timing, not the ruling. Phase 7's box is a different thing —
a `--read-only` job that fails if a mutating request reaches the API — and is
untouched.

**It earned itself immediately, on a defect the gate could not see.** Rust masks
`SIGPIPE`, so `println!` *panics* when the write fails: `k8rs | head` printed a
backtrace and exited **101**, a code [D17](#d17--the---once-output)'s table does
not have, on the very pipeline `screens/once.md` sells the `● ▲ ○` symbols for.
The fix is three cases that are deliberately not collapsed — `BrokenPipe` is the
reader doing its job and costs exit `0` in silence; any other failed write is
`k8rs > findings.txt` onto a full disk and costs a sentence and exit `2`,
because a report truncated in silence is worse than the panic it replaced; a
failed write to stderr is dropped, there being no third stream to complain on.

**One thing that test has to keep asserting is its own premise.** A report that
fits in the pipe buffer is one the child finishes writing before any reader can
go away, so no write ever fails and the test goes green over a binary that still
panics. It therefore asserts the report exceeds the largest pipe a release
target can have — Linux sizes a pipe at 16 buffers of one page, and the biggest
page in the matrix is 16 KiB, so 256 KiB rather than the 64 KiB this machine
has. A hole that opens only on somebody else's kernel is the kind that stays
silent.

### D124 — the freeze forbids reaching back into finished logic, and a card the capture proves wrong is not that (2026-08-20)

Phase 3 closed with **`rules.rs` frozen** — except the snapshot types and their
decode, which [D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)
holds open until Phase 4 close. Phase 4's **first** box needs rule 6 changed. Both
cannot stand, and the next session would have met the collision cold on box one.

**The defect is real and it is on a committed capture**, reproduced at HEAD
rather than argued:

```
$ ./target/release/k8rs tests/fixtures/neverrules.json
1 pod · 0 nodes · 0 workloads

▲ default/broken-neverrules · 3 days ago
  The last run on record failed — exit 3
  container retry · ran for under a second
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The --previous flag below is what fetches it

1 warning

$ jq -r '.status.containerStatuses[] | "\(.name): state=\(.state|keys[0]) exit=\(.state.terminated.exitCode // "-") lastState=\(.lastState|keys[0]? // "none") lastExit=\(.lastState.terminated.exitCode // "-")"' tests/fixtures/neverrules.json
keeper: state=running    exit=- lastState=none       lastExit=-
retry:  state=terminated exit=1 lastState=terminated lastExit=3
```

`retry` is sitting in `state.terminated` at **exit 1**; the card names **exit 3**,
which is the run before that one. And `--previous` resolves through
`lastState.terminated.containerID`, so the command the card hands the reader
fetches the log of the run the card should not have been about. That is
[invariant 4](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)
in the small — a record that lies — on the teaching device the whole tool is built
around.

**What the freeze is for.** Forward-only exists so that nothing built on a layer
finds the layer moved: `analysis.rs`, `views.rs` and `ui.rs` all consume
`Finding`, `ObjectId` and `analyze`'s signature, and reshaping those under them is
the failure the rule names. It was never a rule that a wrong card becomes
permanent because the phase that drew it has ended. Freezing this one would ship
v1 with a card that names the wrong run — the freeze defeating its own purpose.

**The precedent is this file's own, twice.**
[D30](#d30--the-guards-phase-2-added-and-the-freeze-they-collided-with-2026-08-12)
broke the CI freeze "deliberately, minimally and in writing, which is what the
forward-only rule asks for"; D42 made the `rules.rs` freeze per-concern rather
than per-file. This is the third of the same shape and the narrowest.

**The ruling, and the bounds are the entry.** A frozen `rules.rs` may be changed
by a box in a later phase when **all five** hold:

1. The defect is **proven on a committed capture**, output pasted. An argument
   from consistency is not enough — that is
   [the class two PM rulings already shipped a defect through](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13).
2. The box was **written into the later phase before that phase opened**. Nothing
   found mid-phase gets in; CLAUDE.md's *a box is never added to an open phase*
   already says so and this does not widen it.
3. It may change what a rule **reads, says or stands down on**. It may **not**
   add a rule, or touch `Finding`, `ObjectId`, `analyze`'s signature, or a shared
   helper's — those are the surface everything above is built on, and they are
   what the freeze actually protects. `ClusterSnapshot` keeps D42's window and no
   more.
4. The **whole-file mutation gate for `rules.rs` re-runs at the close of the
   phase that made the change**. Phase 3's 553 / 0 was taken before it; a gate
   whose subject moved underneath it is not a gate.
5. The box names this decision, so the unfreeze is never silent.

**What this does not license.** Phase 4's other twenty boxes are `analysis.rs`,
`scripts/` and snapshot fields, and none of them touches a rule. If a second
`rules.rs` box appears in Phase 4 it arrives under condition 2 or not at all.

## Decisions made

### Product

- **Target audience: Kubernetes beginners.** This forces jargon translation —
  `OOMKilled` means nothing to a newcomer.
- **Every finding has three parts:** what happened · what it means · what to do.
- **Show the kubectl command being used** (`dim`, toggled with a key).
  Sounds backwards, but: hide the command and the user stays dependent; show
  it and they learn. Serves both "I don't want to memorize commands" and the
  teaching goal at once. With writes present this doubles as the audit
  trail's human-readable form — the command shown is the command logged.
- **Writes: scale · restart · delete** in v0.1 (reversal above), each behind
  confirm + dry-run, all disableable with `--read-only`. The rest of the
  ladder — cordon/drain/rollout undo, then exec/port-forward, then edit —
  arrives in that order and for that reason
  ([D6](#d6--operation-order-was-inverted-for-the-audience)).
- **The findings screen is the default view.** The browser is reached by
  choosing it in the sidebar; k8rs never opens on a pod list.

### Design

**Palette: Catppuccin Mocha, accent = teal.** Single file `theme.rs`,
10 constants — no theme *loader* (TOML, hot reload), YAGNI.

| Role | Catppuccin Mocha | Hex |
|---|---|---|
| background | `base` | `#1e1e2e` |
| selected row / panel | `surface0` | `#313244` |
| border | `overlay0` | `#6c7086` |
| main text | `text` | `#cdd6f4` |
| evidence line (dim) | `subtext0` | `#a6adc8` |
| accent / highlight | `teal` | `#94e2d5` |
| CRITICAL | `red` | `#f38ba8` |
| WARN | `peach` | `#fab387` |
| OK / healthy | `green` | `#a6e3a1` |
| info / kubectl command | `blue` | `#89b4fa` |

⚠️ Catppuccin is 24-bit RGB. `Color::Rgb` needs a truecolor terminal —
Windows Terminal supports it, legacy conhost does not. Since the target
audience is beginners this is a real risk. Ship a `COLORTERM` check with a
16-color fallback in v1 (~5 lines; originally deferred, pulled into v1 by
the requirements review — first-impression risk, cheap fix).

**Reference note — the rusternetes trap:** the `calfonso/rusternetes`
screenshots were liked as inspiration, but it is *not* a TUI; it is the
**TypeScript web console** of a Rust implementation of Kubernetes. Designed
in terminal aesthetics, but it runs in a browser. Its ring/donut charts,
rounded filled cards, icons and pixel alignment **cannot be done** in a
terminal. The aesthetic direction (mono, warm dark background, low contrast,
dim section headers, stat tiles, sparkline) carries over; fidelity does not.
Also its sidebar is a full resource browser — copy it and you're back to k9s.

- One accent color (mint) + gray tones. **If everything is colored, nothing
  is highlighted** — that's popeye's mistake.
- Red only for "broken right now."
- Two-level hierarchy: title bright, evidence line `dim`. No third level.
- Generous whitespace. The blank line between findings is half the design.
- **Two panes, never more:** navigation on the left, one content pane on the
  right, plus the command log strip at the bottom. What makes k9s feel complex
  is its modal/panel layering — lazygit's discipline is the target instead.
- `● ▲ ○` symbols — never rely on color alone (color blindness + copyability).
- **No nerd-font dependency**, ASCII fallback is mandatory. Otherwise half
  your users see boxes.

Screen sketch — Alerts, the default view on startup:

```
┌ k8rs ───────────────┬ ctx: prod-eu · live · admin ─────────────────┐
│ ALERTS      3 ● 7 ▲│                                               │
│ RESOURCES          │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   workloads        │    Containers exceeded their memory limit and │
│   network          │    were killed by the kernel (OOMKilled)      │
│   storage          │    limit 256Mi · exit 137 · 47 restarts       │
│   config           │    → raise limits.memory, or find the leak    │
│   cluster          │                                               │
│ ANALYSIS           │  ▲ shop/api  ·  2 of 6 pods       12 min ago  │
│   capacity         │    Running, but not receiving traffic — the   │
│   certificates  30d│    readiness check is failing                 │
│   drain safety     │    → check the app's /healthz endpoint        │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
│ $ kubectl scale deployment/web --replicas=3 -n payments            │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

The bottom strip is the command log — every command k8rs runs, as you would
have typed it. It is the teaching device and the audit trail at once, and it
is why a beginner graduates from k8rs instead of depending on it.

### Architecture — where "lightweight" comes from

What determines the load is **not the language, it's how you pull the data**.
k9s is heavy because it fires `LIST pods -A` on an interval; every call makes
the API server read all pods from etcd and serialize them. It degrades
linearly as the cluster grows.

The right way is **watch**: LIST once, then only changes arrive on the
stream. `kube-rs` provides this out of the box with `watcher()` /
`reflector()`.

```
watcher(Api<Pod>::all())
  → prune: ~10 fields, NO managedFields
  → reflector store (memory)
  → rules: pure function, Pod -> Vec<Finding>
  → ratatui: draw only when a change arrives
```

Concrete load reducers:

- **Drop `metadata.managedFields`** — often a third of the object, completely
  useless.
- **Don't store the full `Pod` object**, reduce it to your own small struct.
  Memory drops ~10x.
- **Don't watch Events globally** — the noisiest stream in the cluster.
  Filter `Warning` or fetch on demand.
- **Poll metrics-server slowly** (30s+), only for pods currently on screen.
  It can't be watched, polling is unavoidable.
- **No fixed-FPS drawing.** Draw on events, block when idle → 0% CPU at idle.

Added by the browser (2026-08-11), widened by
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
(2026-08-12): **the Alerts view's own inputs are watched permanently** — Pods,
Nodes, and Deployments/StatefulSets/DaemonSets, five low-traffic streams,
pruned to the fields the `rules.rs` snapshot types name — metadata, spec **and**
status, since the rule set reads `spec` on all three kinds
([D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)). Every other kind in the Resources view is listed when you
open it and watched only while it is on screen; closing the view drops the
watch. Otherwise "browse every kind" would mean forty permanent streams, which
is a worse version of the polling problem this architecture exists to avoid.
ReplicaSets are deliberately *not* in the set — they are read on demand, for
the one finding and the one group heading that need them.

### v1 rule set

This is where the product's real value lives. Every rule is a pure function;
all of them testable.

**From the Pod object alone** (one watch, no extra requests):

| # | Finding | Source field | What we tell the user |
|---|---|---|---|
| 1 | CrashLoopBackOff | `state.waiting.reason` **and how the previous run ended** | **Three sentences, not one** — the loop is the same, what put the container in it is not ([D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)): its last run *crashed* (a failing exit), *finished cleanly* (exit 0 — **three doors, because the code says how the run ended and never who ended it**: the events and the node for a killer, a Job or a CronJob for a program that is meant to finish, and a bug in the program for one that is not ([D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15))), or was *stopped* (exit 143 — a health check or the node's memory killer, not a crash). **Each sentence is scoped to the one run the snapshot holds**, because `CrashLoopBackOff` is entered on accumulated backoff and a clean run does not reset it: four crashes then one clean exit is a real state, and a title claiming the whole loop would have called it *"nothing has crashed"*. The `Finished` and `Stopped` actions are **role-aware** — a native sidecar in a Job is already a Job, and Kubernetes forbids probes on a plain init container. Plus the exit code, translated below |
| 2 | OOMKilled | `lastState.terminated.reason` (exit 137) + `resources.limits.memory` | "Exceeded its memory limit and was killed by the kernel" |
| 3 | **The image is not usable** — the whole family, not just the two: `ErrImagePull`, `ImagePullBackOff`, `InvalidImageName`, `ErrImageNeverPull`, `ImageInspectError`, `RegistryUnavailable`, `SignatureValidationFailed`. All of them mean *this image will never become available* and all carry the kubelet's diagnosis; splitting them sent `nginx:doesnotexist` to rule 3 immediately and `NGINX:::latest` to rule 13 ten minutes later with a card about a disk ([D76](#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13)) | `state.waiting.reason` + `.message` | "Container image is not usable, so the container never started" — wrong name or tag, no pull secret for that registry, or a pull policy that forbids fetching it |
| 4 | CreateContainerConfigError | `state.waiting.reason` + `.message` | "Referenced a ConfigMap/Secret that doesn't exist" |
| 5 | High restart count (even if Running) | `restartCount` | "Restarted N times — looks healthy now, but something is wrong" |
| 6 | Non-zero exit | `lastState.terminated.exitCode` | Translate the exit code (see below) |
| 7 | **Pod Running but container not ready** | `containerStatuses[].ready == false`, **plus how long** — `conditions[Ready].lastTransitionTime` and `containerStatuses[].started`, or the rule fires on every rolling update ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | "Running but not receiving traffic — readiness probe is failing, so it was removed from the Service" |
| 8 | hostPath mount — **only the escalated case** (`/`, a runtime socket **or a directory one sits under**, writable) | `spec.volumes[].hostPath` | "Mounts the node's own filesystem, writable" |
| 10 | **Pending — and why** | `conditions[PodScheduled].reason == Unschedulable` + that condition's own `message` | "No node can accept it" + the scheduler's own sentence (insufficient cpu / nodeSelector / taint) |
| 12 | **Pod stuck Terminating** | `deletionTimestamp` already in the past — the apiserver sets it to *request time + grace*, so it is the deadline, not the moment ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | "Asked to shut down N minutes ago and still hasn't — held by *(the finalizer, named)* / the kubelet" |
| 13 | **Placed on a node, but the containers never started** — the `ContainerCreating` wedge, the *residual* after rules 1/3/4 explain themselves. Gate: `conditions[PodScheduled] == True`, no container started, > 10 min since that transition. `conditions[PodReadyToStartContainers]` is the **evidence**, not the gate — `False` = no sandbox/network yet, `True` = the block is after it, almost always a volume ([D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)) | `conditions[PodScheduled]` + `containerStatuses[].state` + `conditions[PodReadyToStartContainers]` | "It was given a machine to run on, but it has not been able to start — the node cannot give it *(a network / its storage)*" |
| 14 | **Nothing has even looked at this pod** — `phase == Pending` with **no `PodScheduled` condition at all**, older than 2 minutes from `metadata.creationTimestamp`. kube-scheduler is down or crashlooping, or `schedulerName` names one that is not installed or lacks RBAC. Without it every pod is Pending and `--once` prints *nothing is broken* ([D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13)) | absence of `conditions[PodScheduled]` + `metadata.creationTimestamp` | "Nothing has even looked at this pod yet — the scheduler that should give it a machine may not be running" |
| 15 | **A container has stopped and nothing is starting it again** — `state.terminated` with a bad ending, `restartCount == 0`, and an effective `restartPolicy` of `Never`, inside a pod that is **not** over. The one exception [D96](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15) carves out of *the run a container is sitting in is no rule's subject*: that state is a transient everywhere else, and here it is permanent — `kubectl get pods` prints `Error` for such a pod while every other rule in the file is silent. CRITICAL, and the card is the file's first `kubectl logs` ([D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)) | `containerStatuses[].state.terminated` + `restartCount` + `spec.containers[].restartPolicy` falling back to `spec.restartPolicy` | "This container has stopped and nothing is starting it again — read its log; nothing is waiting to start it again, so the pod has to be replaced" |

**Rules 1–6 read `initContainerStatuses` as well as `containerStatuses`** — a
pod stuck at `Init:CrashLoopBackOff` is invisible otherwise, and init
containers are where migrations and wait-for-dependency loops live. The
finding names the init container ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)).

Two rules left this table in the [second-pass review](#design-review--second-pass-2026-08-11):
**rule 9 (no limits defined)** moved to the Capacity report and the plain
read-only **hostPath** case to the Analysis posture rows — neither is *broken
right now*, and both are numerous enough to bury the ones that are. Rule 12
took their place because it costs nothing: the Pod watch is already open.

**Rules that need Events** (second watch, `Warning`-filtered — not in v1,
see the requirements review). Rule 10 used to be listed here and was moved
into the v1 table above: the scheduler writes its reason onto the pod, so it
never needed this watch ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)):

| # | Finding | Source | What we tell the user |
|---|---|---|---|
| 11 | Probe failure | Event `reason == Unhealthy` | "Liveness/readiness probe failing" + how many times |

**Exit code translation** (for rules 6 and 2 — where beginners stumble most):

| Code | Meaning |
|---|---|
| **0** | **The run ended without an error.** It says *how* the run ended and never *who* ended it — a program that traps SIGTERM and shuts down tidily reports `0` whether it chose to stop or a liveness probe asked it to, so the first wording, *"the program finished successfully"*, named an agent one line above an action whose whole subject is that the code names none ([D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)). Added 2026-08-14, when the capture trip produced the first object that reaches it: a container exiting 0 under `restartPolicy: Always` is in `CrashLoopBackOff` like any other, and with no row here the code printed bare under a title claiming a crash ([D85](#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)) |
| 137 **with** `reason: OOMKilled` | SIGKILL after running out of memory |
| 137 **with** `reason: RestartingAllContainers` | **Not a failure at all** — a sibling exited into a `restartPolicyRules` entry **whose action is `RestartAllContainers`**, and the kubelet removed this one to do it. The rules are declared **per container** (`spec.containers[].restartPolicyRules`, no pod-level field exists at v1.36.1), and the action matters: measured, `action: Restart` restarted the failing container five times without touching its sibling, while `kubectl explain` and the OpenAPI document both claim `Restart` is the only value there is ([D96](#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15), [D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)). `RestartAllContainersOnContainerExits` is `{1.36, Default: true, Beta}` at the pinned version, so this needs no unusual cluster, only a pod that declares the rules. **Rule 6 is exempt from it**, beside `exit 0`, `exit 143` and `OOMKilled`: the container that actually failed is the sibling ([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)) |
| 137 **with** `reason: ContainerStatusUnknown` | **Not a kill at all** — the number the kubelet writes where it could not read a status. `convertToAPIContainerStatuses` fills in `exitCode: 137` for a container the runtime reports `Unknown` or has dropped from its list, with `// this code indicates an error` beside the number in its own source ([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)) |
| 137 **with none of those** | SIGKILL — a stop the program cannot refuse, **and the code does not say what sent it.** It named a cause until 2026-08-15 — *did not stop when it was asked to, a failing liveness probe or a shutdown timeout* — which is three claims the object cannot support: an init container may hold no probe at all, a genuine cgroup kill arrives without the word on a starved host ([D84](#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)), and a rebuilt sandbox kills a container nothing asked to stop ([D90](#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)). Who sent it is the **action's** question, because only the action knows the role ([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)) |
| 143 | SIGTERM — graceful shutdown, not an error |
| 1 / 2 | The application's own error, check the logs |
| 126 / 127 | Command not executable / not found — `command` is wrong |
| **255 with `reason: Unknown`** | **Not the application's code at all** — the node found the container already dead and the *runtime* wrote this in place of a code nobody read. **containerd's pair, not the kubelet's**, and the commonest abnormal `lastState` a cluster produces: a node restart leaves it, with real stamps and a real `containerID`, so `logs --previous` works here ([D112](#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)). **Bare `255` stays untranslated** — `exit -1` in a shell reports it, and a code-alone row would tell that reader their program did not fail |
| **-1** | The same event as the row above, spelled by **CRI-O**: the exit code could not be determined. Keyed on the **code alone**, because CRI-O's reason is `Error`, which an ordinary failure also carries — and because `-1` is outside the POSIX range `0..=255`, so no process can report it. A source-derived pin, no CRI-O node exists here ([D112](#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)) |

**137 has four meanings, the object names three of them, and where it names none
the table refuses to guess** — this row read "almost always OOM" until
2026-08-13, and it was written for a rule that had no `reason` field beside the
code. It has one now: a liveness-probe kill that outlives the grace period lands
as `exitCode: 137, reason: "Error"`, rule 2 correctly stays quiet on it, and rule
6 printing the memory sentence sends someone to raise a limit on a container
whose probe endpoint is timing out — the most expensive kind of wrong, because
the fix appears to work for a while
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
**The replacement then named the opposite cause just as flatly**, and was
corrected on 2026-08-15 for the same reason the first row was wrong: it answered
from the code what only the object can answer, and the object is silent
([D93](#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).

**Severity escalators** (for rule 8): if the path is `/`, a **container-runtime
socket or any directory one sits under**, or the mount is not `readOnly` →
CRITICAL instead. **The socket is not Docker's alone** — the list read
`/var/run/docker.sock` until 2026-08-13, and kind, and essentially every cluster
built after 2022, runs containerd: `/run/containerd/containerd.sock`, CRI-O's
`/run/crio/crio.sock`, k3s and RKE2's `/run/k3s/containerd/containerd.sock` and
`/run/cri-dockerd.sock` grant the same node-root, so the escalator that could not
see them silenced the exact object rule 8 exists for. **Each is stored once, in
its `/run` form**, and the compare folds `/var/run/…` onto it
([D78](#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13))
and matches ancestors
([D79](#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13));
the list is deliberately **not** complete, since `--container-runtime-endpoint`
can move the socket anywhere. Paths are compared after normalisation, because `//`
and `/.` are `/` to the kernel and were not to a string compare
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13))
of WARN.

### Node rules (N-series)

Nodes are few (tens, not thousands), so a Node watch is cheap. These are what
an admin actually reacts to:

| # | Finding | Source field |
|---|---|---|
| N1 | Node NotReady for more than 5 min — the pods on it are dead weight | `status.conditions[Ready]` + `lastTransitionTime` |
| N2 | **Cordoned with pods a drain would still move** — a drain someone started and did not finish. **The age is optional, not absent**: the node lifecycle controller stamps `timeAdded` on the `NoSchedule` taint it mirrors from `spec.unschedulable`, so a `kubectl cordon` *does* carry a time and a hand-applied `kubectl taint` does not ([D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)) — and `kubectl describe node` cannot print it, so the card's command is `-o jsonpath='{.spec.taints}'`. **No finding when nothing movable is left** — that is a parked node and a Capacity row. Not counted: `Succeeded`/`Failed` pods, DaemonSet pods and static pods, none of which a drain ever evicts; and the rule is silent on a node carrying an autoscaler scale-down taint ([D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | `spec.unschedulable` + the pod↔node join |
| N3 | DiskPressure / MemoryPressure / PIDPressure — evictions are coming | `status.conditions` |
| N4 | kubelet version skew **> 3 minor** from the control plane = unsupported. **The number is upstream's, not ours** — the version-skew policy allows a kubelet three minors older than `kube-apiserver` (the two-minor limit applies only to kubelet < 1.25), and this table said 2 until 2026-08-13, which told every cluster mid-upgrade that a supported node was unsupported ([D81](#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)) | `status.nodeInfo.kubeletVersion` |
| N5 | Overcommitted: sum of pod requests exceeds allocatable. A native sidecar (`restartPolicy: Always` on an init container) is **added**, not maxed — the scheduler charges `max( max over init prefix , sum(regular) + sum(restartable-init) )` ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | node + pod join |
| N6 | Which taint / nodeSelector is blocking a Pending pod | node taints + pod spec |

N4 is computed here but **shown in the Versions report, not in Alerts** — a
skewed kubelet is a risk, not an outage
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)). The same rule sends
C1 (kubeconfig certificate expiry) to the Certificates report, where the
sidebar badge does the alerting.

### Certificate rules (C-series) — and what is *not* reachable

Correcting a common expectation: `kubeadm certs check-expiration` reads files
on the control-plane node's **disk**. From a laptop, over the API, those files
do not exist. What genuinely is reachable:

| # | Finding | How | Cost |
|---|---|---|---|
| C1 | **Your kubeconfig client certificate expires in N days** (warn at 30) | `client-certificate-data` is base64 PEM in a file we already read — zero cluster traffic | needs an X.509 date parser |
| C2 | API server serving certificate expiry | the peer certificate from the TLS handshake | kube-rs does not expose it; a separate connection is needed |
| C3 | Pending / unapproved CSRs — a kubelet that cannot join | `certificates.k8s.io` list/watch | cheap |
| C4 | cert-manager `Certificate` expiry / renewal failure | CRD `status.notAfter` (no Secret access needed) | cheap, cert-manager only |
| — | Ingress TLS Secrets | would require `get secrets` | **rejected** — reading Secrets to check a date is a bad trade against token hygiene |

### Analysis reports

Cluster-wide, computed on demand, each one a join no per-object rule can do:

| Report | Answers |
|---|---|
| **Capacity** | Per node: requests vs allocatable vs actual usage. Where is the cluster lying to itself |
| **Certificates** | The C-series above, as a dated table sorted by soonest expiry |
| **Drain safety** | For every node: what a drain would do, and what would block it. A PDB with `minAvailable` equal to the replica count means the drain **never finishes** — admins normally discover this 40 minutes in |
| **Waste** | PVCs bound to nothing, Evicted/Completed pod pileups, Services whose selector matches no pod (the 503 nobody can explain), replica sets kept at 0 forever |
| **Versions** | Control plane vs kubelet vs client skew; which nodes are outside the supported window |

Waste and capacity are where krr/popeye play; the difference is that these are
live and one keystroke from the object they blame.

### Upstream architecture review (2026-08-11, docs in `tmp/`)

The plan leaned on three unverified assumptions. Docs pulled from docs.rs and
kubernetes.io; here is what survived and what did not.

**Confirmed:**

- `kube::discovery` exists with `Discovery` / `ApiGroup` / `ApiResource` and a
  `oneshot` mode — the sidebar can be generated from the cluster as designed.
- `PatchParams::dry_run` exists → the preflight step is one field, not a
  hand-built request.
- `dryRun=All` is confirmed in the Kubernetes API docs for all mutating verbs.

**Better than assumed — kube-rs already implements four of our operations:**

| We planned to write | kube-rs provides |
|---|---|
| a `restartedAt` annotation patch | `Api::restart(name)` |
| a `spec.unschedulable` patch | `Api<Node>::cordon(name)` / `uncordon(name)` |
| a hand-built eviction request | `Api::evict(name, &EvictParams)` |
| a full-object patch to change replicas | `Api::get_scale` / `patch_scale` — the scale subresource, which is the correct way |

`ops.rs` shrinks accordingly, and cordon/uncordon — the operation that started
this whole scope discussion — is a one-line call.

**Broken assumption — kube-rs has no `Table` type.** Server-side printing is
not exposed by the typed API. It is still reachable: `Client` has `request`,
`request_text` and `send`, so we build the GET ourselves with the Accept
header and deserialize `meta.k8s.io/v1 Table` with `serde_json`. Roughly forty
lines, once, still zero per-kind code — the design holds, the mechanism was
wrong on paper.

Two details the Kubernetes docs settle, both of which would have been bugs:

1. The header is `application/json;as=Table;g=meta.k8s.io;v=v1` — and
   **aggregated/extension API servers may not serve Table at all, returning
   406**. The Accept header must therefore be
   `application/json;as=Table;g=meta.k8s.io;v=v1,application/json` and the
   client must handle either shape. A browser that 406s on someone's CRD is
   worse than one with hand-written columns.
2. Table is a **GET/list** representation; watching in Table form is not part
   of the contract. So browser views do not watch the Table. They watch
   `Api::watch_metadata` (PartialObjectMetadata — tiny) to learn *that*
   something changed, then re-fetch the Table, debounced. Live enough for a
   human, and still not polling blindly.

**Correction to the version pin:** `k8s-openapi` currently offers `v1_32` …
`v1_36`; the `v1_30` named in REQUIREMENTS no longer exists. Pin the oldest
still offered — **`v1_32`** — and keep the ±2 minor support window.

**Correction to the write ban.** A `disallowed-methods` list of
`create/patch/replace/delete/delete_collection` is not enough: `Api` also
exposes `cordon`, `uncordon`, `restart`, `evict`, `attach`, `exec`,
`portforward`, `entry`, `create_subresource`, `create_token_request`,
`patch_scale`, `patch_status`, `patch_metadata`, `patch_ephemeral_containers`,
`patch_approval` and their `replace_*` siblings. A denylist over that surface
rots the first time kube-rs adds a method.

So the enforcement inverts: **outside `ops.rs`, only an allowlist of read
methods may appear** — `get*`, `list*`, `watch*`, `logs`, `log_stream`,
`apiserver_version`, and the `get_` subresource readers. CI greps for anything
else and fails. A denylist asks "did we remember to ban it"; an allowlist asks
"is this read-only", which is the question that actually matters.

### Dependencies

```toml
kube          # client + runtime (watcher/reflector) + discovery features
k8s-openapi   # API types — one k8s version feature must be pinned
ratatui
crossterm
tokio
anyhow
serde_json    # fixtures, DynamicObject, Table decoding
```

Seven crates carried over from the read-only design. The scope reversal adds
exactly three more, each with a reason — nothing else gets in without the same
justification:

| Crate | Needed for | Why not hand-rolled |
|---|---|---|
| `serde_yaml_ng` (or `serde_norway`) | `e` edit and `y` view — admins edit YAML, not JSON | Writing a YAML emitter is a project of its own; `serde_yaml` itself is unmaintained, these are the maintained forks |
| `x509-parser` | C1 kubeconfig certificate expiry | Parsing ASN.1 dates by hand in a security-adjacent path is exactly the wrong place to be clever |
| `similar` *(v0.4, with `e`)* | the diff shown before `e` applies | A correct diff (Myers) is not a few lines, and the diff is what makes edit safe |

`similar` is approved but **not in the v0.1 dependency set** — `edit` moved to
v0.4 ([D6](#d6--operation-order-was-inverted-for-the-audience)), and the crate
arrives with it. `serde_yaml_ng` still ships in v0.1, because `y` shows YAML.

Still deliberately absent: `clap` — the four flags (`--read-only`,
`--context`, `--namespace`, `--once`) are parsed from `std::env::args` and
none of them needs validation. Also absent: `tracing`, and any TUI widget
framework beyond ratatui.

### File layout

```
src/
  main.rs      event loop, terminal setup/teardown, view routing
  k8s.rs       discovery, watches, prune -> snapshot store (read paths only)
  ops.rs       every write. The ONLY file allowed to mutate the cluster
  rules.rs     analyze(&Snapshot) -> Vec<Finding>      ← per-object, live
  analysis.rs  cluster-wide reports (capacity, certs, drain safety, waste)
  views.rs     per-view state: selection, filters, tabs, scroll
  ui.rs        ratatui drawing
  theme.rs     Catppuccin constants (10 of them)
tests/
  fixtures/    JSON captured from a real cluster
```

Eight files, still flat. **No `mod.rs` pyramid, no trait layer, no plugin
system** — that invariant survives the reversal intact. Each new file exists
because of a boundary that has to be auditable or testable, not because of
tidiness:

- `ops.rs` — the write surface must be one file, or the safety model is not
  reviewable.
- `analysis.rs` — different cadence and shape from `rules.rs` (whole cluster,
  on demand vs one object, streamed).
- `views.rs` — keeps `ui.rs` a pure function of state; the alternative is UI
  state smeared through `main.rs`, which is how TUI code rots.

**Build order is forward-only** (see CLAUDE.md): layers freeze bottom-up —
`rules.rs` → `analysis.rs` → `k8s.rs` → `ops.rs` → `theme.rs` → `views.rs` →
`ui.rs` → `main.rs`. A later step never reaches back into a frozen file; if it
would have to, the plan is wrong and the plan gets fixed first.

The placement of `ops.rs` low in the pyramid is deliberate and is the reason
M2 comes before any UI: **every write operation is verifiable headlessly**
against kind (scale a deployment, watch the replica count change, read the
audit line back). Doing it in this order means the dangerous code is proven
before it is ever attached to a keypress, and the UI phase becomes pure
wiring — `s` calls a function that already works.

### Out of scope (the most important section)

What will kill this project is not technical difficulty, it's **scope
creep**. Three items were removed from this list by the 2026-08-11 reversal
(resource browser, kubectl frontend, write operations) — that is the whole
reason the reversal is written down instead of assumed. What remains absent:

- ❌ Anything deployed into the cluster — no DaemonSet, no CRD, no webhook,
  no in-cluster agent. **The trust model still is "your machine, your
  kubeconfig"** and this survived the reversal untouched.
- ❌ LLM / AI interpretation (k8sgpt exists, and we want to stay offline)
- ❌ Free-form topology graph (nodes+edges) — doable with Canvas but
  unreadable past ~20 nodes
- ❌ Bulk mutation — no "select 12 pods and delete them". Single object,
  single confirmation. The multi-select delete is how outages happen
- ❌ Cluster lifecycle — creating clusters, node pools, cloud provider APIs.
  k8rs talks to one API server and nothing else
- ❌ Editing anything k8rs cannot dry-run first
- ❌ Config file / theming / plugin system / scripting hooks (lazygit has
  these; they came years after it was good, and each is a project)
- ❌ Multi-cluster panes side by side — one context at a time, switchable with
  `X` ([D16](#d16--the-context-switcher) · [screens/context.md](screens/context.md))

**The replacement guard, now that "read-only" is gone** — revised by
[D1](#d1--the-audience-contradiction-resolved), because the first version of
this guard would have deleted the drain-safety report and half of Analysis.
Both halves must hold:

> Would someone who **runs clusters** use it in a normal week — **and** can a
> newcomer read the screen it produces **without a glossary**?

The first half rejects expert toys and one-off curiosities. The second rejects
becoming k9s. "An expert would occasionally want it" still fails the first
half; "an operator needs it but it can only be explained in jargon" fails the
second and gets rewritten, not dropped.

## Traffic / APM side — the fact to know

**There is no traffic data in the Kubernetes API.** RPS, request counts,
service-to-service flow — the API server doesn't know any of it. An external
source is mandatory:

- **Prometheus HTTP API** (`/api/v1/query_range`) — easiest path if
  kube-prometheus-stack is installed, plain HTTP + JSON
- **Istio / Linkerd** — service→service RPS and error rate via
  `istio_requests_total`. Exactly what we want, but a mesh must be installed
- **Cilium Hubble** — gRPC observability API, L3/L4/L7 flows
- Your own eBPF agent — privileged DaemonSet, a separate product

So the TUI does not generate traffic data, it reads what exists. This is an
**adapter** concern and it is not in v1.

### Goldpinger and the contradiction ⚠️

Goldpinger is a node-to-node ping mesh, i.e. *connectivity health* — not
traffic volume. A self-made implementation was requested (roadmap steps 3-4).

**But:** it needs something running on every node, which means deploying a
DaemonSet. At that moment the tool stops being a "CLI that runs on your
machine" — different RBAC, image distribution, different trust model. **We
would have to walk back the sentence "I install nothing into your cluster".**

Note after the 2026-08-11 reversal: the read-only claim is gone, but *this*
claim is not — it is the last structural guarantee k8rs has, and it is the
one that keeps the security model reviewable. Giving up two guarantees instead
of one is how a tool ends up needing an audit before anyone will run it.

Solution: position it as an opt-in plugin, never the default. Go in with
eyes open.

## Roadmap

Revised by the reversal. Each milestone must produce something a person would
actually use — the plan is not allowed to have a "useless until the end" phase.

1. **M1 — the Alerts engine.** The pod rules, the node rules N1–N6 and the
   kubeconfig certificate rule ([§ v1 rule set](#v1-rule-set) is the list, and
   it has grown since this line was written), headless, tested against real
   fixtures. Already a useful tool: point it at a fixture and it prints what is
   broken. **It is released one milestone later**, as `k8rs --once`, v0.0.1 —
   [D10](#d10--m1-ships-publicly-as-v001) asks it to read a *cluster*, which is
   M1.5 and Phase 5 ([todo § Milestones](todo.md#milestones--each-one-is-usable-on-its-own)).
2. **M2 — operations, headless.** `ops.rs`: scale · restart · delete, each with
   dry-run, the confirmation contract and the audit log. Proven against kind
   before a single pixel is drawn.
3. **M3 — the console.** The three views, sidebar navigation, detail tabs,
   logs, command log panel, `?` help. This is the lazygit-shaped product.
4. **M4 — v0.1 release.** musl/darwin binaries, README with a screenshot,
   crates.io.
5. **v0.2 — cordon / uncordon / drain and rollout undo**, wired to the node
   rules and the drain-safety report that already exist by then; plus the
   J/H/Q rule set (Jobs, CronJobs, HPAs, ResourceQuotas) that earns its watch
   here ([D9](#d9--one-rule-added-to-v1-the-rest-recorded-not-built)).
6. **v0.3 — exec and port-forward.** The two operations that need real
   terminal and socket work, and that widen the trust boundary.
7. **v0.4 — edit + apply** (`$EDITOR`, diff, 409 handling) — last on the write
   ladder on purpose.
8. **v0.5 — Events-based rule 11** (probe failures) and the noisy-stream
   handling it requires. Rule 10 shipped in M1 — it reads the pod, not events
   ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)).
9. **Later — traffic adapter** (Prometheus / Istio / Hubble, endpoint from
   user config only) and, separately, the goldpinger-style connectivity mesh
   as its own binary and repository (see the trust-model note below).

The step list for all of this lives in [`todo.md`](todo.md) and only there.

## Settled

- ✅ **Test environment: kind.** Local. Deliberately producing
  OOMKilled/Pending in a real cluster is both hard and risky; in kind we
  break whatever we want.
- ✅ **Interface language: English.** Error messages, K8s terms and search
  results are already English; if open-sourced the audience is too. Jargon
  still gets simplified — `OOMKilled` → *"container exceeded its memory
  limit"*. No i18n (YAGNI); splitting later is cheaper.
- ✅ **All project files in English** (2026-08-11) — notes, docs, README,
  code comments, commit messages. Turkish only in conversation.
- ✅ **Scope: full admin console, not read-only** (2026-08-11) — writes,
  resource browser, analysis reports. See
  [Reversal](#reversal--read-only--managed-writes-2026-08-11); the read-only
  guarantee is replaced by the five-mechanism safety model and `--read-only`.
- ✅ **Positioning: lazygit for Kubernetes** (2026-08-11) — the audience is
  someone in their first month on the job, and that is now the scope guard.
- ✅ **The browser writes no per-kind code** (2026-08-11) — API discovery plus
  server-side `Table` printing. This is what makes "every kind, CRDs included"
  affordable at all.
- ✅ **Licence: `GPL-3.0-or-later`** (reversed 2026-08-12,
  [D13](#d13--licence-gpl-30-or-later-reversed-2026-08-12)) — a fork may be
  sold but not closed. `cargo publish` requires the field either way, so this
  blocked the very first step of the code phase.
- ✅ **Second-pass review closed fourteen open contradictions** (2026-08-11) —
  audience, the Alerts/Analysis dividing line, owner grouping, namespace
  scoping, the operation order, and the honesty of invariant 4. Nothing about
  the architecture changed; what changed is which findings appear where, which
  operations ship when, and that M1 gets released on its own
  ([Design review § second pass](#design-review--second-pass-2026-08-11)).

### YAML crate decided by spike (2026-08-11) — and the edit model it forces

Ran both candidates against a realistic pod manifest (spike outside the repo;
design phase stays doc-only). Results:

| | `serde_yaml_ng` | `serde_norway` |
|---|---|---|
| comments survive a parse→emit round trip | **no** | **no** |
| key order survives | yes | yes |
| block scalars survive | yes | yes |
| flow style (`["sh","-c"]`) survives | no — normalised to block lists | no |
| parse result | identical to the other | identical |

**Decision: `serde_yaml_ng`.** The two are equivalent on every axis that was
tested, so the tiebreaker is lineage clarity, not capability.

The more important result is that the question was framed wrong. *No*
serde-based YAML crate can keep comments — the serde data model has nowhere to
put them — and formatting is normalised too. So the edit model is forced, and
it is the safer one anyway:

> **The user's text buffer is the source of truth during an edit.** k8rs
> parses their YAML only to *validate* it and to build the request. It never
> re-emits their file back to them. Comments and formatting therefore survive
> because they are never round-tripped — including when an apply is rejected
> and the editor is reopened with their work intact.

**A bug the spike caught before it was written.** On the display path
(API JSON → YAML shown by `y`), `serde_json::Value` uses a `BTreeMap`, so keys
come out **alphabetised**: `metadata: {labels, name, namespace}` instead of
kubectl's `{name, namespace, labels}`. For a tool whose entire point is
teaching beginners what kubectl shows, that is not cosmetic. Fix: enable
serde_json's `preserve_order` feature so API key order is kept.

### Capability probe — "if it is there it works, if not it says so"

Decided 2026-08-11, and it closes the Prometheus/Istio open question by making
it not need an answer: **k8rs detects what the cluster has and adapts.**

The discovery call from Phase 5 already lists every API group the cluster
serves, so the probe is free — no extra request, no configuration:

| Present? | Turns on |
|---|---|
| `metrics.k8s.io` | actual usage numbers in the capacity report |
| `policy/PodDisruptionBudget` | drain safety |
| `cert-manager.io` | C4 certificate findings |
| `monitoring.coreos.com` (kube-prometheus-stack) | the traffic view, later |
| `networking.istio.io` / Linkerd / Cilium | service-to-service traffic, later |

Two rules make this honest rather than magic:

1. **A missing capability is stated, never hidden.** The row stays in the
   Analysis list and reads "needs metrics-server — not installed in this
   cluster". Silently disappearing features teach a beginner that the tool is
   unreliable; a sentence teaches them what their cluster is missing.
2. **Detection is not configuration.** Presence comes from API discovery;
   the *address* of anything outside the Kubernetes API still comes only from
   explicit user configuration — never from pod annotations
   ([REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements),
   SSRF). "Prometheus is installed, tell me where to reach it" is the whole
   interaction.

The same mechanism covers RBAC: a 403 on a stream disables exactly the
findings that need it and says which permission is missing.

## Open questions
- [x] Project name? → **k8rs** (2026-08-10, see naming section)
- [x] **A workload whose pods were never created produces no finding — is that
      worth a watch?** → **yes**, resolved 2026-08-12 as
      [D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12):
      Deployments, StatefulSets and DaemonSets are watched; ReplicaSets are
      fetched on demand. The reasoning that produced it: Every v1 rule reads a Pod. If the pods do not exist — a
      ResourceQuota denial, an admission webhook rejection, a missing PVC, a
      bad pull secret at ReplicaSet level — there is nothing to iterate:
      `kubectl get pods` is empty, the Deployment sits at 0/3, and k8rs says
      *cluster healthy*. It is the most beginner-hostile failure class there
      is, and the only one the tool currently cannot see. The signal lives on
      the workload, not the pod: `ReplicaSet.status.conditions[ReplicaFailure]`
      carries the quota/webhook message verbatim, and
      `Deployment.status.conditions[Progressing].reason ==
      ProgressDeadlineExceeded` marks a rollout that gave up.

      **A second hole closes with the same call.** [D3](#d3--findings-group-by-owner-not-by-pod)
      groups findings by Deployment/StatefulSet/DaemonSet/Job, but a pod's
      `ownerReferences` points at its **ReplicaSet**. Nothing in the plan says
      where `web-7d4f5c6b8` becomes `web`, so as written, M1 groups under the
      hashed ReplicaSet name — a random string, in the product whose rule is
      that every visible string is readable by a newcomer.

      **Recommendation:** watch Deployments, StatefulSets and DaemonSets
      (metadata + status only — desired vs ready, plus the Progressing
      condition), and fetch a ReplicaSet **on demand**, only when a finding or
      a group heading needs it, cached by UID. Workload objects are two orders
      of magnitude fewer than pods and barely churn, so this is not the thing
      that makes k9s heavy — a repeated `LIST pods -A` is
      ([§ Architecture](#architecture--where-lightweight-comes-from)). Jobs and
      CronJobs stay in the v0.2 J-series as already planned.

      **Cost, stated honestly:** it changes [invariant 6](CLAUDE.md) — "only
      Pods and Nodes are watched permanently" — so it is a decision, not a
      task, and it must be taken before `k8s.rs` freezes in Phase 5. It also
      adds the three workload kinds to the 10 000-pod memory measurement
      ([D25](#d25--what-this-review-did-not-decide)).

## Build order — why it is what it is

*(The steps themselves live in `todo.md` and only there. This section records
the reasoning behind their order; it is not a plan and is not checked off.)*

**Context: this is a first TUI project.** The architecture is built
accordingly. The critical property: the project splits into two halves with
uneven difficulty:

| | Hard / new | Easy / familiar |
|---|---|---|
| What | ratatui event loop + kube-rs watch | Rules: `fn analyze(&Pod) -> Vec<Finding>` |
| Why | Terminal goes into raw mode, events awaited via select, drawing synchronized | Pure function. Struct in, list out |
| Testing | Hard, by eye | **Easy — plain unit tests against captured JSON fixtures, no terminal needed** |

So **the product's real value (the rules) can be written and tested fully
independent of the TUI.** For a TUI newcomer this erases most of the risk.

Hence the order in `todo.md`: fixtures first, then rules (Phase 3, milestone
M1), and only then the TUI. It is deliberate — when the rules phase finishes
you already have a working product (ugly, but working), so if the TUI phase
stalls the project does not die.

> **A cluster is not needed to start the rules** — all they need is pod JSON.
> But hand-written JSON must never survive into the committed fixtures:
> it resembles reality, it is not reality; rules can only be trusted against
> real captures.

## Dev environment setup checklist

Surveyed on the dev machine 2026-08-11. **Most of the plan needs no cluster at
all**, which is the whole reason the build order puts the pure layers first:

| Phase | Needs |
|---|---|
| 3 rules · 4 analysis | cargo + fixtures. **No cluster, no container runtime** — and this is where the product's value lives |
| 8 spike · 9 theme · 10 views · 11 ui | a terminal |
| 2 fixtures · 5–6 reads · 7 ops · 12 wiring | kind, therefore a container runtime |

- [x] **Rust** — 1.97.1 present (distro package). `rustup` is *not* installed;
      it is in the repos if a pinned toolchain is ever wanted. CI pins its own,
      so this is not blocking.
- [ ] **Container runtime** — neither docker nor podman is installed *on this
      laptop*. Both are packaged (`podman 6.0.2`, `docker 29.7.2`). It never
      needs to be: the cluster lives on the LAN host below, and k8rs only
      needs a kubeconfig.
- [ ] **kind** — packaged (`kind 0.32.0`). With podman it needs
      `KIND_EXPERIMENTAL_PROVIDER=podman`; with docker it just runs.
- [ ] **kubectl** — packaged (`1.36.3`). Note the fit: `k8s-openapi` offers
      `v1_32`…`v1_36`, so this client sits at the top of our window and the
      pinned `v1_32` talks to it fine (forward compatibility).
- [x] **The test cluster runs on a LAN host, not here** (2026-08-11) — Docker
      29.7.2 + kind 0.32.0 + kubectl 1.36.3, three nodes, brought up by
      [`scripts/cluster.sh`](scripts/cluster.sh) with
      `K8RS_APISERVER_ADDRESS` set to that host. Not an accident of what was
      lying around: k8rs runs on the user's machine against a kubeconfig
      ([invariant 3](CLAUDE.md)), so a cluster reached over the network is the
      real trust model, and it is the only way a dropped connection can be
      tested by actually dropping one. Findings:
      [§ Verified against a real cluster](#verified-against-a-real-cluster-2026-08-11).
- [x] **just** — packaged (`1.58.0`), install with the rest.
- [x] **git** (2026-08-12) — repository initialized on branch `main`, remote
      `git@github.com:murat-akpinar/k8rs.git`. History starts at `chore: init`:
      the pre-rename repository was deleted with the `r7s` name, so nothing
      before the design phase survives.
- [ ] A truecolor-capable terminal (for the Catppuccin palette)

> **Rules may be started before a cluster exists** — they only need pod JSON.
> But hand-written JSON is a bootstrap, never a committed fixture: it
> resembles reality, it is not reality. Phase 2 exists to replace it.

## Verified against a real cluster (2026-08-11)

Test host: a LAN machine, not this laptop — Ubuntu 24.04.4, 4 vCPU / 3915 MiB,
Docker 29.7.2, kind 0.32.0, `kindest/node:v1.36.1`, three nodes (1 control-plane
+ 2 workers), API served on port 6443.

**Deliberately not localhost.** k8rs runs on the user's machine against a
kubeconfig ([invariant 3](CLAUDE.md)), so the test path has real TLS, real
latency, and a connection that can actually be cut — which is the only way the
disconnected and login-expired screens in [states.md](screens/states.md) get
tested honestly. kind writes `127.0.0.1:<random>` into the kubeconfig by
default; `networking.apiServerAddress` + a fixed `apiServerPort` are what make
it reachable from another machine.

### Confirmed

| Assumption | Evidence |
|---|---|
| Server-side `Table` works for namespaced and cluster-scoped kinds ([invariant 12](CLAUDE.md)) | Pods and Nodes both return `kind: Table` with the columns `kubectl get` prints |
| `may_i` is performed with `create` — D23's reason for keeping it in `ops.rs` | `kubectl auth can-i -v=7` shows `POST .../selfsubjectaccessreviews` |
| The capability probe has a real "not installed" branch to render | `metrics.k8s.io` absent from `api-versions`; `kubectl top nodes` → *"Metrics API not available"* |
| `managedFields` pruning is worth doing | **29%** of the pod list on a *fresh* cluster. That is the floor, not the typical case — the field grows with every controller that touches an object |

### Contradicts a document

1. **"Table is a list representation and cannot be watched" is false.**
   `?watch=true` with the Table `Accept` header returns 200 and streams
   `{"type":"ADDED","object":{"kind":"Table",…}}`. The mechanism
   [resources.md](screens/resources.md) chose — watch metadata, re-fetch the
   Table debounced — is still right, but for a different reason: **every event
   re-sends the entire column schema, 3086 bytes of `columnDefinitions` to
   deliver an 82-byte row.** A 37× overhead is the argument; impossibility was
   never true.
2. **The `,application/json` fallback is unproven, not wrong.** The claim is
   that aggregated API servers may answer `406` to a Table-only `Accept`. This
   cluster has **zero** aggregated APIServices, so nothing exercised it. Keep
   the fallback — it costs one header — but it stays untested until
   metrics-server or another aggregated API is installed.
3. **"The exact columns `kubectl get` would show" needs a filter.** The server
   returns *both* sets in one response: `priority: 0` is plain `kubectl get`,
   `priority: 1` is `-o wide` only. Pods come back with nine columns, five of
   them priority 1. Without filtering on `priority == 0` the browser shows the
   wide view on every screen.

### Not written down anywhere yet

4. **Table rows carry `PartialObjectMetadata`, not the object.**
   `.rows[].object` has `metadata` and nothing else — no `spec`, no `status`.
   That is enough for display *and* enough for alerts to bleed through (name,
   namespace and uid are all present to match a row against a finding), but a
   report can never be built from a Table row. This is why `analysis.rs`
   fetches typed lists separately, already a step in
   [todo Phase 5](todo.md).
5. **`Quantity` is a string, and nothing in the ten approved crates does
   arithmetic on it. — OPEN, needs a decision.** The cluster hands back
   `cpu: "4"` *and* `cpu: "100m"`, `memory: "4009164Ki"` *and* `"70Mi"`.
   `k8s-openapi` models this as `Quantity(pub String)`: no parsing, no
   comparison, no addition. The Capacity report, drain safety, waste **and the
   `capacity  N ▲` sidebar badge** all need `sum(requests) vs allocatable`.
   This is an unrecorded gap against [invariant 10](CLAUDE.md). Two ways out: a
   ~40-line suffix parser of our own (`m`, `k/Ki`, `M/Mi`, `G/Gi`, `T/Ti`,
   `P/Pi`, `E/Ei`, plus decimal exponents) living with the rules, or an
   eleventh crate.
6. **C2 does not need the API at all.** The API server's serving certificate
   comes off the TLS handshake — `CN=kube-apiserver`, issuer `CN=kubernetes`,
   one year, SANs including the address the user typed. `/version` also answers
   anonymously. So the certificate-expiry warning still works for a user whose
   RBAC allows nothing else.
7. **C3 needs a manufactured fixture.** kind does produce CSRs — two kubelet
   bootstrap requests — but they arrive `Approved,Issued`. A *pending* CSR, the
   thing rule C3 reports, has to be created deliberately.
8. **kind's kubeconfig has no `exec` block and no `insecure-skip-tls-verify`.**
   Both the D19 "your login expired" path and the TLS-not-verified header
   warning need a hand-written kubeconfig; kind will never produce either.
9. **kind node allocatable is not physically true.** Each of the three nodes
   reports `cpu: "4"` and `memory: 4009164Ki` — the whole host, three times
   over, on a 4-core / 3915 MiB box. Fixtures captured here are internally
   consistent, so rules compute correctly against them, but they do not
   describe a real cluster's per-node isolation. Any capacity fixture needs a
   line saying so, or someone reads 12 cpu as real.
10. **The browser's grouping problem is 67 api-resources across 22 API groups**
    — 33 namespaced, 34 cluster-scoped — on a *bare* cluster, before a single
    CRD exists.
11. `/version` on 1.36 also returns `emulationMajor/Minor` and
    `minCompatibilityMajor/Minor`. Version skew reads `gitVersion` for the
    control plane and `Node.status.nodeInfo.kubeletVersion` per node; the
    emulation fields are a newer concept and are **not** what skew means.

### Measured cost

Three nodes plus one probe pod: **1568 MiB used of 3915, 2347 MiB available.**
Comfortable for rules, fixtures and the write path. But
[todo.md](todo.md)'s *"measure resident memory against 10 000 pods"* does not
fit here — etcd plus the apiserver watch cache would exceed what is left. That
step needs a bigger host or a recorded snapshot, and knowing it now is cheaper
than discovering it in Phase 7.

## kind test manifest

The deliberately broken pods needed for writing rules. Numbers match the
rules table.

[`scripts/broken.yaml`](scripts/broken.yaml) — **the runnable copy, and the
only copy**. Applied with `scripts/cluster.sh break`. It used to be inlined
here; two copies of a manifest drift, and the one that drifts is always the one
in the prose.

| Rule | Pod | State it produces |
|---|---|---|
| 2 | `broken-oom` | OOMKilled, exit 137 — memory limit 64Mi against a 250M allocation |
| 1 + 6 | `broken-crashloop` | CrashLoopBackOff, exit 1. Needs minutes to enter backoff |
| 3 | `broken-image` | ImagePullBackOff — an image on a registry that does not resolve |
| 4 | `broken-config` | CreateContainerConfigError — `envFrom` a ConfigMap that does not exist |
| 10 | `broken-pending` | Pending, unschedulable — requests 500 cpu. The scheduler's own explanation lands in `conditions[PodScheduled].message`, which is what rule 10 reads |
| 8 | `broken-hostpath` | hostPath mount of `/`, writable → must come out CRITICAL |
| 7 + 11 | `broken-readiness` | Running but never Ready — the readiness probe always fails |
| 9 | `broken-nolimits` | No limits set. **Not an alert** — this fixture exists to prove the *Capacity report* row |
| 12 | `broken-stuck` | Stuck Terminating: a finalizer nothing removes. Applied by the script, put into Terminating by the capture step |
| 1–6 (init) | `broken-init` | `Init:CrashLoopBackOff` — an init container that exits non-zero while the app container never starts. The pod the old rule set could not see ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)) |
| 14 | **none yet, and this one is easy** | `schedulerName: does-not-exist` on an otherwise ordinary pod. Nothing picks it up, so no `PodScheduled` condition is ever written — the exact shape, with no control-plane surgery and nothing to clean up ([D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13)) |
| 15 | `broken-neverback` | `restartPolicy: Never`, three containers: one exits 1 (the finding), one exits 0 (the clean-exit negative on the same object), one sleeps so the pod never reaches `Failed` and leaves through `finished(pod)`. Captured single-file rather than by re-running the whole corpus, which moved the pinned `now` with it ([D97](#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)) |
| 13 | **none yet** | The `ContainerCreating` wedge. Every captured pod has `PodReadyToStartContainers: True`, so rule 13 ships with a negative side only until the next trip. The residual branch is reachable — a `configMap` **volume** naming an object that does not exist — and the network branch may not be, since it needs the sandbox itself to fail ([D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)) |
| W1 | `broken-quota` (namespace `k8rs-quota`) | A Deployment whose ReplicaSet cannot create a single pod — the quota allows zero. `kubectl get pods` is empty and the truth lives only on the ReplicaSet's `ReplicaFailure`. It sits in its own namespace because a `pods: "0"` quota applies namespace-wide and would block every pod above ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)) |

`broken-stuck` is why `cluster.sh unbreak` patches the finalizer away before
deleting — a plain `kubectl delete` on it never returns.

Fixture capture is [`just fixtures`](justfile) — and only there. It runs
`cluster.sh verify` and the sanitizer test before capturing anything, pipes
every object through [`scripts/sanitize.jq`](scripts/sanitize.jq) on the way
out, and stamps `tests/fixtures/K8S_VERSION` with the server version the
fixtures came from. The command used to be written out here as well; two
copies of a procedure drift, and the one that drifts is always the one in the
prose.

The negative side lives in [`scripts/healthy.yaml`](scripts/healthy.yaml) and
goes up with the broken pods, so both sides are captured from the same cluster
at the same moment.

# Project name: k8rs ✅ (decided 2026-08-12, replaces r7s)

**Expansion: `k8s` + `rs` — "Kubernetes, in Rust."** Not one coined word but
two marks the reader already knows, joined: `k8s`, the numeronym everyone in
this ecosystem expands without being taught, and `rs`, the extension a Rust
file carries and the suffix the ecosystem puts on *this thing, in Rust* —
[`kube-rs`](https://github.com/kube-rs/kube), `serde-rs`, `tokio-rs`. Said out
loud: "kay-eight-arr-ess", or simply "kubernetes-rs".

**Why the numeronym rule was dropped.** `r7s` obeyed the `k8s` construction
exactly — `r|ustnete|s`, seven letters elided — and that was the flaw: the
reader has to be handed the expansion (`rustnetes`) before the name means
anything, and `rustnetes` then collides with
[rusternetes](https://github.com/calfonso/rusternetes) (716★, "kubernetes,
reimplemented in Rust") inside the very sentence that would explain it. `k8rs`
needs no expansion: both halves arrive already expanded, so the name is
legible on first sight to exactly the audience the tool is built for. It does
not claim to be a numeronym — it is a compound, and reads as one.

## What the name does not say

`k8rs` carries Kubernetes and Rust; it does not carry **TUI**. That is
deliberate — a third element makes it unsayable — so it is the tagline's job,
permanently: *the Kubernetes TUI, in Rust*. A description that omits
"terminal" leaves the name reading like a library.

**Accepted risk:** [`kube-rs`](https://github.com/kube-rs/kube) — the client
crate this project is built on — is the *other* thing called "Kubernetes in
Rust". `k8rs` will sometimes be taken for a library. Not solved, mitigated:
every first line (repo description, crates.io summary, README) leads with
what it *is* — a terminal UI — before what it is made of.

## Availability (verified 2026-08-12)

- crates.io, npm, PyPI: `k8rs` free.
- GitHub user/org `k8rs`: free. Docker Hub namespace `k8rs`: free.
- GitHub repos named `k8rs`: two abandoned hobby projects — a 2★ "Kubernetes
  operator written in Rust" untouched since the day it was created
  (2024-12-01), and a 1★ "for learning purposes only" repo with no commit
  after creation (2025-01-24). No incumbent to displace.
- Rejected on the same check: `kr8` / `kr8s` (`apptio/kr8` is a real k8s
  config tool, `kr8s` a known Python k8s client), `r8s` (a phylogenetics
  program, a Samsung device codename, and an `r8s-rs` org), `krust`
  (krustlet, 3604★), `kargo` (3556★), `krab` (532★), `lazykube` (582★),
  `rubernetes` (existing org — and it is **Ruby**ernetes), `rustik`,
  `rustui`, `cure`, `mend`, `kraft`, `patina`, `vigil`, `krate` — all taken
  on crates.io or with a real incumbent.
- Runner-up kept on the shelf: **`kr8t`** — all four elements at once
  (**K**ubernetes, **R**ust, the **8** of k8s, **T**UI), free on every
  registry and on GitHub, and it reads as "crate". Lost because its digit
  sits mid-word: harder to say and to dictate than `k8rs`, where the digit is
  the one this ecosystem already says out loud every day.

## Lineage — three names, one filter

`rat8s` (2026-08-10) → `r7s` (2026-08-11) → `k8rs` (2026-08-12). Each name
died on a criterion the next had to satisfy, and the three together are the
test any future name must pass:

1. **It expands without a lesson.** `rat8s` expanded to nothing — `rat` + 8
   letters + `s` is not a word, and in this ecosystem every reader tries.
   `r7s` expanded only once taught. `k8rs` arrives expanded.
2. **It has one obvious pronunciation.** "rat-eights"? "rats"? "rat-kates"?
   That question ended `rat8s`. A tool recommended out loud to a colleague
   gets one reading, not three.
3. **It points at the user, not at the implementation.** `rat` was ratatui —
   a fact about our source tree, meaningless to someone learning Kubernetes.

## Logo

Still open, deliberately. The rat silhouette died with `rat8s`; `k8rs` is a
compound of two ecosystem marks and implies no picture. No mascot is better
than a mascot the name did not earn.

**First task (start of code phase):** publish a `k8rs` placeholder to
crates.io before someone else takes the name — and claim the GitHub org
`k8rs` in the same sitting.
Fallback if the name is ever lost: **k8ray** ("the X-ray of k8s").

## Tagline (name-independent)

> Kubernetes TUI in Rust — fast, safe, zero-runtime.

## Inspiration / reference tools

- [k9s](https://github.com/derailed/k9s) — competitor/neighbor; explorer, we are the interpreter
- [krr](https://github.com/robusta-dev/krr) — resource recommendations
- [KubeUI](https://github.com/IvanJosipovic/KubeUI)
- [khi](https://github.com/GoogleCloudPlatform/khi) — log visualization
- [snorlax](https://github.com/moonbeam-nyc/snorlax)
- [rusternetes](https://github.com/calfonso/rusternetes) — ⚠️ not a TUI, a web console (see the design section)
- [goldpinger](https://github.com/bloomberg/goldpinger) — source of the v4 mesh idea
- [Keda HTTP scaling](https://github.com/SevginGalibov/Keda-HTTP-Add-On-Scaling)
