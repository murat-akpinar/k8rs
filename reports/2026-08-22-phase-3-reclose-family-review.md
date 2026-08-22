# Phase 3 re-close — the family read again, over the phase (2026-08-22)

`k8s-admin`, step 4 of CLAUDE.md § Phase close, on the **re**-close that
[D155](../NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22)
forced and [D156](../NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22)
landed. Tree under review: `977fe2b`, clean. **No cluster was used** — every
command below runs against the committed tree, the committed fixtures, or the
test binary built from them.

Not re-found here: everything in
[2026-08-20-phase-3-close-cross-family-review.md](2026-08-20-phase-3-close-cross-family-review.md)
and [2026-08-22-rule-13-family-review.md](2026-08-22-rule-13-family-review.md),
both read first.

## Findings

Ranked by what they cost to leave. Verdicts against CLAUDE.md's triage rule: a
**blocker** is wrong output, a crash, or something the security pass rules
exploitable, and it re-runs the ritual; everything else is a backlog entry and
the phase still closes. **Nothing here is a blocker.**

---

### 1 — should-fix · `docs/architecture.md:389-394` still names a defect the code fixed, as the live reason for the supported floor

The paragraph, present tense, at HEAD:

```
$ sed -n '389,394p' docs/architecture.md
reports less — which is right, because it also *has* less. The exception is the
`PodReadyToStartContainers` condition, which enters the Kubernetes API at 1.29: the
card for a pod that was scheduled and never started reads its absence as *storage
and network are fine*, and on an older cluster nothing said that. That is where the
floor comes from, and it is the only place a supported-window statement was needed
([NOTES § D149](../NOTES.md#d149--the-floor-is-129-because-one-rules-else-turns-a-missing-field-into-a-claim-2026-08-22)).
```

That branch is three arms since `dfe6136`. `src/k8s.rs:1399-1407` was rewritten in
the same commit to say so — *"That branch has been three arms since 2026-08-22,
and the floor did not move with it … The live defect is gone and the unfinished
audit is not"* — and `NOTES.md:12794` and `backlog.md:806` both carry the same
correction. `docs/` did not move with them:

```
$ git show --stat dfe6136 | grep -c '^ docs/'
0                      # exit 1 — grep matched nothing, which is the finding
```

**What breaks:** `docs/` is the built state for readers outside this repo
(CLAUDE.md § Where to look — *never contains anything not yet true of the code*).
A reader deciding whether to run k8rs on 1.28 is told the floor exists because a
card lies on their cluster. It no longer does; the floor now rests on D149's
unfinished audit, which is a different and weaker argument, and the one the
`k8s.rs` comment is careful to state. Two files now give two reasons for one
number, and the stale one is the one a user reads.

**Not a blocker:** no output is wrong, only a document. Docs-sync at step 8 of
the close is where it belongs, in this turn — CLAUDE.md calls stale docs after a
structural change *a failed step*, not a follow-up ticket.

---

### 2 — should-fix · `screens/alerts.md:343-355` and `:363-366` state four `rules.rs` defects that no longer exist

The file says an action wrapping past five lines *is a `rules.rs` finding*, then
records four shipped violations of its own rule and prices them:

```
$ sed -n '348,355p' screens/alerts.md
  (`cargo test -- --nocapture`, every distinct `→ ` line wrapped at 49): of 44
  distinct author-written actions, **4 are over** — `stopped_action`'s two arms
  at 6 lines, rule 5's no-record arm at 6, and `failed_action(Init)` at **8**,
  which draws a 14-line card. The fifth string that measured over is the runtime
  quote two bullets up, and it is not an action at all. **What that costs is
  40 · 36 · 18 · 105 characters**, in that order.
```

and, twelve lines later, that the tallest card in the mockup is not the worst
case:

```
$ sed -n '364,366p' screens/alerts.md
for — 12 rows, the separator, and the three the next finding gets. **Three card
shapes are over it today**, all of them the same eight-line action, so this is
the shape the rule set is coming back to rather than one it is safely inside:
```

Re-measured at HEAD by the file's own method — its own command, its own 49
columns:

```
$ cargo test --quiet -- --nocapture 2>/dev/null | grep -oP '(?<=→ ).*' | sort -u | wc -l
75

$ python3 -c '...textwrap.wrap(s,49) over each distinct action...'
distinct actions: 75
  1 wrapped lines: 12
  2 wrapped lines: 28
  3 wrapped lines: 15
  4 wrapped lines: 10
  5 wrapped lines: 10
```

**Zero over.** The longest action reachable is 238 characters and wraps to
exactly five. The *at-cap* half of the same passage is still exactly right: ten
actions sit at five lines, which is the number the file states.

**And the eight-line one the mockup section is built around cannot be
re-measured, because the function is gone:**

```
$ grep -rn "failed_action" src/
(no match)

$ grep -n "failed_action is deleted" NOTES.md
8126:`failed_action` is deleted. Each intermediate state shipped a defect on a committed
```

`screens/alerts.md:350` and `todo.md:1135` both name `failed_action(Init)`, and
no such function has existed in `rules.rs` since that rewrite. What stands in its
place is `finished_action`'s five arms (`src/rules.rs:3645-3673`), every one of
them 229-234 characters and five wrapped lines, and `stopped_action`'s two
(`:3737`), 229 and 234 characters, five lines each — with the `systemd-oomd`
correction the box also asked for applied (`src/rules.rs:3692`: *"`systemd-oomd`
is gone from both arms"*; only `earlyoom` survives in either sentence).

**Method note, because I got this wrong once:** my first attempt read the arms
out of the source with a regex and reported one at 322 characters and eight
lines. It was concatenating literals across arms. The number above is the
rendered output of the run, which is the object; the regex was a reconstruction
of it, and CLAUDE.md's rule that only the object says what it does applies to a
reviewer's own measurement as much as to anyone else's.

**What breaks:** the file tells the next author that four rule defects are
outstanding in a file that is now frozen, and points a `tui-designer` round at a
trade — *either the budget is wrong or four actions are* — that was settled by
[D113](../NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
and then paid off in `rules.rs` by somebody else's box. Box `todo.md:1069` is the
one that owned the measurement and it closed on *"it was neither the budget nor
the actions"*; the shortening happened anyway, and the file that carries the
number was never re-measured.

**Ceiling on my measurement, stated:** an action that no test ever prints is not
in the 75. The file's own method has the same ceiling, and 75 > the 44 it
measured, so the coverage is not narrower than the claim being checked.

---

### 3 — should-fix · `src/analysis_tests/restarts.rs:303` and `:468` — two age assertions that the repin left provable only by a string five fixtures share

`tester`'s claim, reported to the PM, is that each of the moved age strings is
still pinned by a `timestamp` assertion beside it. Outside the synthetic ladder
table there are **seven** `2 days ago` assertions, not six; all seven were
checked. **Five hold, two do not.**

Verified (the two the brief asked for, plus three more):

| assertion | what pins it, beside it |
|---|---|
| `src/rules_tests/node.rs:179` | `card.timestamp == captured_time(Ready.lastTransitionTime)`, `:169-176` |
| `src/rules_tests/node.rs:436` | `card.timestamp == captured_time(taint.timeAdded)`, `:422-435` |
| `src/rules_tests/pod.rs:648` | `looping.timestamp == captured_time(lastState.terminated.finishedAt)`, `:635-638`, with an `assert_ne!` at `:639-645` proving the capture can tell `finishedAt` from `startedAt` |
| `src/rules_tests/pod.rs:10014` | `unplaced.timestamp == captured_time(PodScheduled.lastTransitionTime)`, `:10005-10010` |
| `src/rules_tests/snapshot.rs:338` | `elapsed.as_mins() == 2927`, `:329-335` — stronger than a timestamp, it is the exact minute count off the fixture's own stamp |

Not pinned:

```
$ grep -n "startedAt\|captured_time\|timestamp\|as_mins\|duration_since" src/analysis_tests/restarts.rs
38:// where `state.running.startedAt` is still `None` (NOTES § D100) together with a start past
93:                started_at: Some(started),
379:            started_at: Some(now()),
470:            "the count first, then the run's own age off `state.running.startedAt`"
515:    // The under-eight-second window after a restart leaves `startedAt` null (NOTES § D100), and a
546:            .started_at = None;
556:            .started_at = Some(Time(now().0 + SignedDuration::from_hours(24)));
```

Lines 93 and 379 are synthetic times for other tests. There is **no** assertion
anywhere in the file tying either `2 days ago` to a captured `startedAt` — and
there is nothing to tie it to: an `analysis::Row::Answer` carries `detail:
Vec<String>` with the age already rendered into prose, so this pane has no
`timestamp` field to assert. The claim cannot be true of these two by
construction.

What the string used to do and no longer does:

```
$ git show dfe6136 -- src/analysis_tests/restarts.rs | grep -E "^[-+].*This run started"
-            "This run started 50 min ago.".to_string(),
+            "This run started 2 days ago.".to_string(),
-                "This run started 1 hour ago.".to_string(),
+                "This run started 2 days ago.".to_string(),
```

```
$ cargo test --quiet the_worst_leads_and_the_row_is_the_container_fact -- --nocapture
○ default/broken-restarts10serving · container flaky
      Restarted 10 times since this pod started.
      This run started 2 days ago.
○ default/broken-reboot · container app
      Restarted 3 times since this pod started.
      This run started 2 days ago.
○ default/broken-restarts · container flaky
      Restarted 3 times since this pod started.
      This run started 2 days ago.
○ default/broken-gang · container bystander
      Restarted 3 times since this pod started.
      This run started 2 days ago.
○ default/broken-gang · container trigger
      Restarted 3 times since this pod started.
      This run started 2 days ago.
```

The four `startedAt` values behind those five rows:

```
$ for f in restarts10 reboot restarts gang; do echo -n "$f: "; \
    jq -c '[.status.containerStatuses[]?|{n:.name,s:.state.running.startedAt}]' tests/fixtures/$f.json; done
restarts10: [{"n":"flaky","s":"2026-08-20T23:10:04Z"}]
reboot:     [{"n":"app","s":"2026-08-20T23:12:04Z"}]
restarts:   [{"n":"flaky","s":"2026-08-20T22:43:53Z"}]
gang:       [{"n":"bystander","s":"2026-08-20T22:43:27Z"},{"n":"trigger","s":"2026-08-20T22:43:24Z"}]
```

**The scenario:** `50 min ago` was unique to `restarts10` in that pane at the old
pin — 23:10:04 is 49 m 56 s before `2026-08-21T00:00:00Z`, while `reboot` was
47 m 56 s and the other three ~1 h 16 m. Swap the row's age source to any other
container's `startedAt` today and the assertion at `:303` still passes: all five
render `2 days ago`. It asserts that a string was produced, not that the row read
its own clock — which is exactly what the comment above `:466` claims it proves
(*"each row measures its own"*), and is the finding already backlogged from the
rule-13 family review for `:468` alone. The repin extended that defect from one
assertion to two and from two rows to five.

**Not this phase's tree** — `analysis_tests/` is Phase 4's — but the pin that
moved it is Phase 3's `fn now()`, and it is the only place I found where the
repin cost assertion strength rather than just changing a number. Backlog, and
the fix is an `elapsed.as_mins()` line beside each, the shape
`rules_tests/snapshot.rs:329` already uses.

---

### 4 — should-fix · the phase-close mutation gate has not been reported over the changed `rules.rs`, and box `todo.md:1517` quotes a count that is no longer of this file

The box's own done-when is a whole-file run:

```
$ sed -n '1517,1525p' todo.md
- [x] `cargo mutants --timeout 90` clean over `rules.rs` — a MISSED mutant is a
      ...
      **Closed 2026-08-20: 553 mutants, 498 caught, 55 unviable, 0 MISSED, 0
      timeouts**, over four shards
```

`rules.rs` has changed since, in the file the box names. What the brief reports as
run is `just mutants-diff` (10 caught / 0 missed) — the **per-turn** gate, scoped
to the diff. CLAUDE.md § *Step 4 is the anti-leak mechanism* is explicit that
`just mutants` whole is the *phase-close* gate and `--in-diff` is not it.

The gap is real and measurable: the diff-scoped run mutates changed lines only.
Over the two functions this box touched, whole:

```
$ cargo mutants --file src/rules.rs -F 'placed_but_never_started|node_stopped_being_ready' --list
… 19 mutants
```

19 against the diff run's 10 — **nine mutants live in unchanged lines of the two
functions the box rewrote**, and no run since `dfe6136` has covered them.

**What bounds the risk, measured rather than assumed.** The class the 2026-08-20
whole-file run actually caught was the duration ladder — D119 records ten of the
fifteen opening misses as that one defect — and a repin onto the days rung is
exactly what would resurrect it. It does not: the ladder is asserted against a
synthetic `ago(secs)` table at both sides of every boundary
(`src/rules_tests/snapshot.rs:140-172`), which is independent of `fn now()`. So
the highest-probability regression is ruled out and what remains is unmeasured
rather than suspected.

**Observed at review time, and reported rather than assumed:** a
`cargo-mutants … --file src/rules.rs --file src/analysis.rs --shard 0/4` process
was already running in this tree while I read it, so somebody is running the
whole-file gate now. Its result was not in the brief and is not in this report.
The box's numbers (`553 / 498 / 55`) need replacing with that run's, or the `[x]`
carries a measurement of a different file — which is the shape D155 re-opened
this phase for.

**Process note, same paragraph:** my own targeted `cargo-mutants` invocation
collided with that run. No damage — cargo-mutants' own `mutants.out/lock.json`
held, mine waited on the lock and tested nothing, and I killed it. Recorded
because two mutation runs in one working tree is the concurrency rule
(CLAUDE.md § *The one hard rule of concurrency*: the scratchpad is a file tree
too, and so is `mutants.out`), and the lock is what saved it rather than the
process.

---

### 5 — nit · rule 13's new evidence is four wrapped lines against a three-line cap, and it is the first evidence line on the screen that has no API quote to lose

`src/rules.rs:6040-6048`. Measured at the three widths `screens/alerts.md` names
(49 continuation, 51 at the floor, 53):

```
$ python3 -c "...textwrap.wrap(evidence, N)..."
evidence at 49: 178 chars -> 4 lines
   1 |on node k8rs-worker3 · the machine has written|
   2 |nothing at all about this pod: not one of its|
   3 |containers has a status, not even a failed|
   4 |attempt, so nothing there has picked it up|
evidence at 51/53: 178 chars -> 4 lines
   ...
   4 |so nothing there has picked it up|
```

`screens/alerts.md:199` caps the evidence at three lines and marks it the one
part that may be cut. So on Alerts this card cuts, and what it cuts is *"so
nothing there has picked it up"* — the clause that turns the absence into a
diagnosis. The reader is left with a stated absence and no conclusion.

**Why it is worth saying rather than shrugging at:** `alerts.md:238-249` argues
the cut is honest *because* the evidence is "the only unbounded thing on the
card" — it carries a controller's verbatim sentence that D37 forbids trimming and
no author bounds. Rule 13's second shape quotes nothing. Its evidence is 158
characters of prose an author chose plus a node name, and the cut therefore lands
on k8rs's own words, which is the trade every other bullet in that section
refuses. `alerts.md:209` and `:222-235` state the general rule the other way round:
*the action is never cut … a rule with nothing useful to say must still say
something in its own voice*.

**Nit, not should-fix:** no cut exists yet. `--once` prints the evidence whole on
one line (`src/main.rs:433-436`, no wrapping), and `screens/once.md` promises no
cut, so today the sentence arrives complete. The renderer that would cut it is
Phase 8. `rules.rs` is frozen, so this is a backlog entry with a deadline, not an
edit — and the fix is free, because the fact order is the lever the same file
already documents (`alerts.md:1086-1088`, rule 5 putting the load-bearing fact
ahead of the image so the image is what gets cut).

**Checked in the same pass and clean:** both new actions are inside the five-line
cap — 4 lines / 168 chars with a node, 2 lines / 95 chars without — and the code
comment's reason for dropping `{node}` from the action is measured-true, not
reasoned: the old wording is 217 characters and exactly 5 lines with the
twelve-character fixture node name, and 246 characters and 6 lines with a
41-character cloud-default node name.

---

### 6 — nit · [D156](../NOTES.md#d156--rule-13s-silence-is-ruled-on-the-node-and-the-three-of-four-routes-to-its-own-shape-that-delete-themselves-2026-08-22) ruling 2 describes a signature the code does not have, and the argument it made no longer holds

`NOTES.md:13372-13373`:

> `placed_but_never_started` takes `&snapshot.nodes` — the shape
> [`no_node_accepted_it`] already has, so it is not a new one

At HEAD it takes the whole snapshot, because the family review's namespace-scope
finding needed `namespace_scope`:

```
$ grep -nE "^fn (stuck_terminating|escalated_host_path|no_node_accepted_it|placed_but_never_started|nothing_has_looked_at_it)" src/rules.rs
5571:fn escalated_host_path(pod: &PodSnapshot) -> Vec<Finding> {
5720:fn no_node_accepted_it(now: &Time, pod: &PodSnapshot, nodes: &[NodeSnapshot]) -> Option<Finding> {
5991:fn placed_but_never_started(snapshot: &ClusterSnapshot, pod: &PodSnapshot) -> Option<Finding> {
6216:fn nothing_has_looked_at_it(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
6279:fn stuck_terminating(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
```

Both halves are now false: the shape is not `&snapshot.nodes`, and it **is** a new
one — `placed_but_never_started` is the only pod rule in the file that takes
`&ClusterSnapshot`, so it is the only one that can see `namespace_scope`, the
other pods, the workloads and the server version. Nothing in `rules.rs` says the
exception is bounded to the two fields it actually reads. The narrow signatures
everywhere else are what makes a pod rule trivially independent of every other
pod, and this is the one place a later change could quietly grow a cross-pod
dependency without a reviewer noticing the input widened.

A decision that disagrees with reality is a bug in the decision: one line in
`NOTES.md`, and — if the PM wants the guard rather than only the correction — one
sentence at `src/rules.rs:5991` saying which two fields of the snapshot this rule
is allowed to read. This is the only place in the phase where the rule 13
signature change left a stale description; `src/rules.rs:1011`, `:5919` and
`:6598` all still read correctly.

---

### 7 — nit · `backlog.md:338-339` quotes N1's card in wording the code stopped printing

```
$ sed -n '337,340p' backlog.md
  `tests/fixtures/kube-system-pods.json` and the same card reads
  `kube-system/kindnet and kube-system/kube-proxy were running here (2 pods)`.
  N1's own doc comment refuses exactly this — *"one pod was running here" about a
  node carrying forty reads as complete* — and N2's count and N5's sum are gated
```

Both quotes moved to `placed here` in `dfe6136` (`src/rules.rs:6643`,
`src/rules.rs:6615`). The entry's *reasoning* is untouched and still correct —
this is a live entry about the driver's `namespace_scope: None`, read at a phase
close — but its reproduction line no longer reproduces, and a reader who runs the
command it gives and does not see that string will not know whether the entry or
the code moved. One-line edit, `backlog.md` is the PM's.

---

### 8 — nit · `scripts/mutants.sh:283-321` scans `mutants.out` on a run where cargo-mutants never wrote it, and reports another process's numbers as this run's

Reproduced, unintentionally, while another mutation run held the lock:

```
$ bash scripts/mutants.sh --timeout 90 --file src/rules.rs -F 'placed_but_never_started|node_stopped_being_ready'
mutants: scratch /home/…/.cache/k8rs-mutants (909 GiB free, 2 required)
 INFO Waiting for lock on /home/…/mutants.out/lock.json ...: Resource temporarily unavailable (os error 11)
ERROR interrupted
Error: interrupted
mutants: no log names the filesystem or a denied lint — 180 log(s) read on /home/…/.cache/k8rs-mutants
mutants: 18 unviable — each of these is a claim that there was nothing to test:
           src/analysis.rs:324:5: replace capacity -> Report with Default::default()
           …
```

This invocation tested **zero** mutants and named `--file src/rules.rs`. Every
line after `Error: interrupted` describes a different process's run: 180 logs
from the shared scratch, and 18 `unviable` entries that are all `analysis.rs`,
which this invocation could not have produced. The three scans at `:283`, `:293`
and `:319` read `$OUT` unconditionally, and cargo-mutants only rotates
`mutants.out` once it has the lock — so when it never starts, `$OUT` is whatever
was there before.

**The verdict is not affected** and I want to be exact about that, because my
first reading of this was wrong: `exit $rc` at `:322` carries cargo-mutants'
status, and the `exit 0` I saw came from my own `| tail` in the outer shell,
which has no `pipefail`. So this produces misleading prose, never a false green.

It is still worth a line, in the one file whose entire subject is
[D133](../NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21)
— *a non-result must not read as a result*. The guard for it is one condition:
scan only if this run wrote `$OUT`, or say which run's logs are being reported.
`tester`'s file, backlog.

---

## The three sections, and what turned up nothing

### Part 1 — is any other Phase 3 box checked over work its own text does not describe?

Read against the code, not against the tests. Box `todo.md:611` (rule 13) now
carries the *Two shapes, not one* paragraph and describes what shipped.

Hunted the conjunction shape specifically — a box naming two things where the
code does one — and **found none in the product rules**. What was checked, by
box:

| box | the specific shape its text names | at HEAD |
|---|---|---|
| `:548` Pod rules 1–8 and 12 | rule 8 escalated-only, rule 12 outside the finished-skip | `analyze` calls `stuck_terminating` before the `finished(pod)` gate, `rules.rs:2422-2425` — the one pod rule deliberately outside it, as written |
| `:595` **Rules 1–6** read `initContainerStatuses` too | all six, not some | the container loop iterates `pod.containers`, which `container_snapshots` builds as `init.chain(main)`; all eight container rules see init containers, so the range is if anything understated |
| `:653` Node rules **N1–N6** | six rules, three in `analyze` | not a narrowing: `rules.rs:2383-2384` states *N4 and N5 are not missing, they are `Info`* and routes them to Versions and Capacity; N6 folds into rule 10 (`screens/alerts.md` § N6). Three cards in the node loop is the documented answer, not a gap |
| `:669` Workload rules W1–W2 | W2 fires only when no pod finding explains the shortfall | `analyze` runs the W-series last, in two passes, collecting before appending (`rules.rs:2461-2474`) |
| `:1264` exit table, **137 has four readings** | `OOMKilled` · `RestartingAllContainers` · `ContainerStatusUnknown` · none | four arms, `rules.rs:3397/3400/3403/3407` — `OOMKilled`, `ContainerStatusUnknown`, `RestartingAllContainers`, then a bare `137` that names the signal and stops. `ending` (`:3225-3235`) gives two of the four their own variants and lets the other two fall to `Failed`, which is the split D95 recorded |
| `:1265` hostPath fires **only** on `/`, a socket **or any directory one sits under**, or a writable mount | the prefix half, and the `/var` fold | `rules.rs:5678-5683`: `/var` stripped, prefix tested at a `/` boundary, and the emptiness guard that stops `""` being a prefix of everything |
| `:1510` per rule, positive **and** negative | 21 of 21 | the newest shape has both: `unstarted.json` on a `Ready: Unknown` node is rule 13's negative and the composed `Ready: True` copy is the positive (D156 ruling 5) |
| `:1069` action budget | *four actions are over* | **the code is right and the file is stale** — finding 2 |
| `:1517` `cargo mutants` clean over `rules.rs` | the whole file | **the measurement no longer describes the file** — finding 4 |

The two that turned up something are the last two in that table, and neither is
the D155 shape in the direction D155 found it: the code is not narrower than its
box. In both cases the code moved *past* the box and the artifact recording the
box's evidence stayed put. That is the same class read backwards, and it is worth
the PM naming as such, because the audit that catches it is the same one.

### Part 2 — what the rule 13 change could have broken across the phase

- **The signature change.** One stale description, in `NOTES.md` rather than in
  `rules.rs` — finding 6. `src/rules.rs:1011`, `:5919`, `:6598` and
  `backlog.md:788-811` all read correctly at HEAD.
- **N1's evidence line, for every pod on a down node.** Consumers checked, all
  consistent. The Drain report reads N1 **by identity and never by text** —
  `not_ready` (`src/analysis.rs:1217-1232`) matches on
  `severity == Critical && kind == Node && name`, and `drain_row` uses
  `n1.action`, never `n1.evidence`, with its own `NODE_SILENT` paragraph for the
  prose. So the wording change reaches exactly one screen. `screens/alerts.md`
  moved with the code in the same commit (`:823`, `:829`, `:850`), and
  `src/rules_tests/node.rs:324` now guards the old wording out by name.
  *Is it true of every pod that can appear?* Yes — `pods_on` filters
  `p.node == node && !finished(p)`, and every pod with a `nodeName` was placed
  there, including the mirror pods the kubelet created for itself and the
  never-started pod the sentence exists for. What the `False` branch gave up is a
  true present-tense claim it was entitled to — on a node whose kubelet answered,
  `containerStatuses[].ready` is not a fossil — but that branch has no mockup in
  `screens/alerts.md` by design (`rules.rs:6603`, *only one of them is
  `screens/alerts.md`'s*), so there is no spec it now disagrees with. Recorded,
  not a finding.
- **`PodReadyToStartContainers` at three arms.** Rule 13 is the only reader, and
  the grep proves it rather than assuming it:

  ```
  $ grep -rn "ready_to_start_containers\|PodReadyToStartContainers" src/ screens/ docs/ | grep -v _tests
  src/k8s.rs:421:        self.ready_to_start_containers.bound();
  src/k8s.rs:1389 / :1411   (the floor comment)
  docs/architecture.md:390  (finding 1)
  src/rules.rs:1007 / :1022 / :2094   (the field, its doc, its decode)
  src/rules.rs:5984 / :6145 / :6160   (rule 13)
  ```

  The snapshot type's own doc at `src/rules.rs:1014-1021` was rewritten in the
  same commit and now says `None` **is** a third case. It is the one doc in the
  phase that had to move and did.
- **The repin.** Five of seven anchored, two not — finding 3. No other age
  assertion in the phase depends on a shared string: the ladder itself is
  asserted against a synthetic table (`rules_tests/snapshot.rs:140-172`) that
  `fn now()` cannot move, and the certificate counts are re-derived by
  `scripts/certs-test.sh` from the extracted literal on every `just check`.

### Part 3 — the phase's own security gate, against the phase as a whole

- **No finding text quotes an env value or a Secret.** Holds, including the two
  new card texts. Rule 13's second shape interpolates exactly one API string,
  `pod.node`, plus the pod's own identity; N1's changed line interpolates owner
  names. No annotation, no env, no Secret, no certificate body reaches a
  `Finding` anywhere in the file — C1, the one rule holding bytes that matter,
  turns them into a date and nothing else (`src/rules.rs:7748-7757`).
- **Malformed and truncated PEM returns no finding and never panics.** Covered at
  the framing level D31 asks for, not just the input level — five shapes, and the
  one that matters most is a *real, parseable* certificate wearing a
  `RSA PRIVATE KEY` label, so the refusal is the label and not luck about what
  the body decodes to (`src/rules_tests/certificate.rs:384-414`). The parser is
  two `.ok()?`s and an explicit label check with no `unwrap`, no indexing and no
  slice arithmetic (`src/rules.rs:7748-7757`). Nothing to report.
- **The deferral, and its one exception.** Control characters and length bounds
  are Phase 5's ingest gate except for the temporary `main.rs` printer, and the
  printer strips everything it draws: `card()` sanitizes title, evidence, action,
  the age and both halves of the object name (`src/main.rs:418-458`), and the
  report printer sanitizes badge, title, every row text, every detail paragraph,
  every action and both `NotComputed` sentences with **no `..` in the match**
  (`src/main.rs:546-589`), so a new string field on a row is a compile error
  rather than a silent unstripped line. Rule 13's `on node {node}` therefore
  reaches the terminal stripped. `kubectl_cmd` is not printed by this driver at
  all, so the new card's `get_yaml` line is not a path either.

## Sanitization re-read

Re-read against the [reports/ rule](README.md#the-sanitization-rule--read-it-before-pasting-cluster-output)
after the file was complete. **No cluster was used for this review and nothing
here came from one.** Every output quoted is a committed file, the test suite over
committed fixtures, `git`, `grep`, or arithmetic over strings in the repository.
The only object field values pasted are four `state.running.startedAt` stamps
from committed fixtures, already in `tests/` and quoted in NOTES. Node names that
appear (`k8rs-worker3`) are the fixture cluster's. No token, certificate, key,
kubeconfig, environment value, annotation payload, IP or hostname — the
41-character cloud node name used in the width measurement of finding 5 is
described by its length and deliberately not written out. Home directory paths in
finding 8's paste are elided to `/home/…`.
