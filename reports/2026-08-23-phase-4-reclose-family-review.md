# Phase 4 re-close — family review of the seven reports and the 21 boxes

Operator review for the re-close box at `todo.md:2049`, run against `development`
at `4654425` with a clean tree. Read under `NOTES § D157` (a re-close runs the
whole ritual, and its first job is to hunt *a box checked over work narrower than
its own text*), `§ D155`, `§ D158`, `§ D134`, `§ D137` and `§ D42`.

**No cluster was brought up.** Everything below is local: the committed fixtures,
the built binary, the guards' own self-tests, and one synthetic input built in the
scratchpad and deleted afterwards. Nothing here is an object dump.

## 1 — The tree as it stands

```
$ git log --oneline -1 && git status --short
4654425 chore(changelog): update
(clean)

$ cargo test 2>&1 | grep -E '^test result'
test result: ok. 518 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.17s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.80s

$ just check
... advisories ok, bans ok, licenses ok, sources ok
GREEN WITHOUT THE CROSS-COMPILE MATRIX — musl/darwin std not installed here
```

`just check` is green. The cross-compile matrix was skipped for missing std, which
the recipe prints loudly; CI runs it.

## 2 — Every count a Phase 4 box states, re-taken

`D157` says two numbers are re-taken and never carried. The whole-file mutation
sweep is the PM's (it was running during this review — see § 7). These are the
rest.

| Where | Box says | Measured at `4654425` |
|---|---|---|
| `todo.md:1887` (Drain safety) | `Done: five row kinds` | **seven** — 7 `DrainLine` constructions, bands `4 3 2 1 0 0 0` |
| `todo.md:1765` (`sanitize.jq` anchor) | `over all 55 fixtures` | **63 committed fixtures (59 parsed as JSON)** |
| `todo.md:1720` (retention) | `157K` vs `778K` | `reports/` **277389 B**, `NOTES.md` **914242 B** |
| `todo.md:1720` (retention) | `13 of the 37` by-name citations | **136** by-name occurrences outside `reports/`; the 13/37 split not reproducible with a regex I trust |
| `todo.md:1937` (restart pane) | ANALYSIS block drawn in **six** files, all gained `restarts` + `posture` | six files name the seven-pane list; `screens/resources.md` draws the header collapsed with no children, by design |
| `todo.md:2020` / `:2027` (mutants) | `849 / 756 / 0 / 93` | not re-taken here — the PM's sweep was running |

Commands:

```
$ grep -n "band: [0-9]" src/analysis.rs
965:            band: 4,
1000:            band: 0,
1023:            band: 3,
1059:            band: 2,
1088:            band: 1,
1139:            band: 0,
1155:        band: 0,

$ git show 1ef9e3f:src/analysis.rs | grep -c "band: [0-9]"
7
$ git show 1ef9e3f:src/analysis.rs | grep -n "kinds of row"
702:/// **One row per node, seven kinds of row, drawn in band order**: the node a drain would never
842:/// **One node's verdict.** Seven kinds of row, and the order they are asked in is *not* quite the

$ grep -n "row kinds" screens/analysis.md
450:- **Seven row kinds now, five bands deep, worst first**: `would never finish

$ find tests/fixtures -type f | wc -l
63
$ bash scripts/fixture-audit.sh 2>&1 | tail -1
fixture-audit: 63 committed fixtures (59 parsed as JSON) — no annotations, no env values, no
addresses; no key material in any framing (armoured, base64-wrapped, DER, mislabeled); node names
intact; scripts/sanitize.jq leaves every one of them byte-identical; and the k8s-openapi pin is not
below the cluster they came from

$ du -sb reports/ NOTES.md
277389  reports/
914242  NOTES.md

$ grep -rEno "reports/20[0-9-]+[a-z0-9-]*\.md" --include="*.md" --include="*.rs" . \
    | grep -v "^\./reports/" | wc -l
136
```

The Drain safety number is the one that matters: the code had seven row kinds and
its own doc said `seven kinds of row` **in the commit that closed the box**
(`1ef9e3f`, 2026-08-21 17:16), and `screens/analysis.md:450` says
`Seven row kinds now, five bands deep`. The Done note collapsed *bands* into
*row kinds*.

## 3 — Two doc claims in `analysis.rs` that the phase itself falsified

`analysis.rs` freezes at this close, so this is the last cheap moment for both.

**a. `src/analysis.rs:1938-1941`**, the `finished_pods_left_behind` doc:

> NOTES' own words for the term are *removed by the node because it ran out of
> room* (NOTES § Positioning, invariant 14), so the row's text and the whole of
> its explanation are written from that …

`NOTES § Positioning` item 4 at HEAD (`NOTES.md:267-276`) reads:

> `Evicted` is "its node stopped it and took the room back" — **and the
> translation names no cause** … The first draft of this line said "because it ran
> out of room", and a report row built faithfully on it told operators their node
> was short …

The row's own strings are correct (`Either the node was short, or the pod went
over its own disk limit (Evicted).`). The doc above them is the surviving copy of
the sentence `D158` deleted, and it names that sentence as the row's source.

```
$ grep -rn "ran out of room" src/ NOTES.md
src/analysis.rs:1939:/// own words for the term are *removed by the node because it ran out of room* (NOTES §
NOTES.md:272:   "because it ran out of room", and a report row built faithfully on it told
NOTES.md:13550:thing** — *"`Evicted` is 'removed by the node because it ran out of room'"* — and
```

**b. `src/analysis.rs:86`** — *"and Phase 4's `Posture` report is in no sketch at
all"*. True when written, false 51 minutes later:

```
$ git log --pretty="%h %ci %s" | grep -E "10b8580|bc0970f"
10b8580 2026-08-20 20:17:10 +0300 docs(ui): the six panes, and the one place a missing metrics-server is said
bc0970f 2026-08-20 19:26:41 +0300 feat(rules): the report shape, where the variant says what the cursor may do

$ grep -n "^## Posture" screens/analysis.md
1433:## Posture
```

`analysis.rs` was edited on 08-20 twice more, on 08-21, 08-22 and 08-23 without
the sentence moving.

Same paragraph, `src/analysis.rs:105`: *"Across all five mockups exactly two
entries badge"*. The **substance** re-measures true — no sidebar entry other than
`capacity` and `certificates` carries a value anywhere in `screens/` — but the
count of mockups has moved:

```
$ grep -rhoE "│ +(drain safety|posture|restarts|waste|versions) +[^ │][^│]*│" screens/ | sort -u
(no output)

$ grep -rho "│ ANALYSIS" screens/*.md | wc -l
17
$ grep -rc "ANALYSIS  " screens/*.md
screens/alerts.md:2  screens/analysis.md:7  screens/detail.md:1  screens/resources.md:1  screens/states.md:6
```

## 4 — The phase's security-gate paragraph (`todo.md:2033-2047`), item by item

Every claim in it re-checked against `src/` at `4654425`. All hold.

```
$ grep -n "\.unwrap()\|\.expect(\|panic!\|unreachable!\|todo!\|\[0\]\|\[1\]\|\[i\]" src/analysis.rs
(no output)

$ grep -n " / " src/analysis.rs
213:        /// memory or CPU limit`, Waste's `47 pods` / `12 replicasets`, Certificates' `2
584:/// Parsed with [`quantity_milli`] and printed with [`cpu_text`] / [`bytes`], which is what the row
3043:                deadline.duration_since(snapshot.now.0).as_hours() / 24

$ grep -n " - " src/analysis.rs
1129:            let others = stale.len() - 1;
2209:        let over = self.namespaces.len() - named.len();
2449:                RESTARTS_WARN - 1
2789:    let unmeasured = nodes.len() - measured.len();

$ grep -n "RESTARTS_WARN" src/rules.rs | grep -i const
2348:pub(crate) const RESTARTS_WARN: i32 = 3;
```

- **One division, by the constant `24`** — `analysis.rs:3043` is the only one; the
  other two hits are prose.
- **Three unsigned subtractions, all guarded** — `1129` behind an explicit
  `stale.len() > 1`; `2209` behind `take(NAMESPACES_NAMED)` over the same set;
  `2789` behind a `filter_map` over the same `nodes`. The fourth `-` is
  `RESTARTS_WARN - 1` on an `i32`, so the paragraph's word *unsigned* is exact.
- **No `unwrap`/`expect`/`panic!`/indexing** — the grep is empty. `drain_safety`
  even documents why it uses `unwrap_or_default()` where an `expect` would read
  naturally (`analysis.rs:728`).
- **Every `Row` field stripped as it enters its line, named individually, no
  `..`** — `main.rs:560-588` destructures `Row::Answer { severity, text, detail,
  action, jump: _ }` and calls `sanitize` at each `format!`. The only `..` in
  `analysis.rs` are three `matches!` patterns (`1687`, `2606`) and one prose
  mention (`2258`).
- **`analysis.rs` reads no env value, no Secret, no annotation** — the class is
  empty; the import list at `analysis.rs:67-75` carries no such accessor.
- **Length bounding is Phase 5's ingest gate, amended to name
  `spec.volumes[].hostPath.path`** — confirmed at `k8s.rs:383-390`,
  `text(&mut self.path, FREE_TEXT)`, with `FREE_TEXT = 4096`. `PodSnapshot::reason`
  is bounded beside it, `maybe(&mut self.reason, IDENTIFIER)` at `k8s.rs:415`.
- **No dependency changed** — `Cargo.toml`, `Cargo.lock`, `deny.toml` untouched;
  `cargo deny` inside `just check` reports `advisories ok, bans ok, licenses ok,
  sources ok`.

## 5 — The boxes' other checkable specifics, verified against `src/`

Each of these was read against the code rather than taken from the box.

| Box | Specific | Result |
|---|---|---|
| Rule 6 vs 15 | one shared reader `last_run_on_record`, one shared predicate `settled` | `rules.rs:3070`, `:3123`; callers `4268 4393 4737 5004`, plus `5521` (`stopped_for_good`) calling `settled` directly — five rules, as the box says |
| Rule 6 vs 15 | `neverrules.json` names exit 1 | `retry` decodes `state.terminated.exitCode: 1`, `lastState.terminated.exitCode: 3` — the split the box is about |
| Rule 6 vs 15 | no second reader of a terminated record | `doing_its_job` and `nothing_else_to_point_at` read the *current* state through the shared `ending`; `analysis.rs` reads none at all (grep for `exit_code`/`last_terminated`/`Terminated(` in `analysis.rs` returns doc comments only) |
| `check-docs.py` | both directions, level-3 only | `--self-test` passes and says so |
| `reports-guard.py` | 21 planted values, 7 classes, canary | `--self-test`: *"21 planted values across 7 classes, each refused whole, as a substring, inside a fence and base64-encoded"* |
| `width-guard.py` | `--self-test`, one table-row exemption | passes and says so |
| `certs-test.sh` | now `(C1 reports)` | `certs-test.sh:166` |
| `sanitize.jq` anchor | `k8rs-(control-plane\|worker[N])`, same anchor backs the CSR rule | `sanitize.jq:110` `kind_node_re`, used by `refuse_foreign_nodes` (`:115`) and at `:197` for `system:node:` |
| `Report` shape | `Row::Answer\|Prose\|NotComputed`, `Jump::Finding\|Object`, no `Jump::Set` | `analysis.rs:171`, `:278` — `Jump` has exactly two variants |
| Capacity | `spec.overhead` summed **inside** `charged`, not on top | `node_row` passes `overhead_cpu`/`overhead_memory` into `promised` (`analysis.rs:437-450`); one summation |
| Capacity | five metrics states reach the one slot | `live_usage_row` matches `Read`/`None`/`NotInstalled`/`Silent`/`Denied`; `using()` adds the sixth (node absent from the map) |
| `restartPolicyRules` | `ExitRule` + `ContainerSnapshot::restart_rules`, rule 15 stands down only where a rule is *shown* to cover the exit | `rules.rs:722`, `:804`, `restart_rules_bring_it_back` at `:5327`, gated at `:5521` |
| `terminatingReplicas` | `WorkloadSnapshot::terminating`, W2's readiness fact only, no gate | `rules.rs:1427`; single reader `shutting_down` (`:7474`) called by `rollout_gave_up` only; `explains_a_shortfall` (`:7636`) does not read it |
| in-place resize | `effective` resolves enacted-over-declared for all four resource fields | `rules.rs:2000-2003` — `cpu_request`, `memory_request`, `memory_limit`, `cpu_limit` |
| in-place resize | `allocated_*` decoded, tested, read by nobody | only readers are `rules_tests/snapshot.rs` and `k8s.rs`'s bound |
| Waste | Service first, then claims, then the pileup, then parked ReplicaSets | `waste()` at `analysis.rs:1669-1672` in that order |
| Waste | per-object sections cap at five with a `Row::Prose` overflow; counted rows do not | `at_most` (`:1759`) is called by `services_reaching_nothing` and `disks_nobody_mounts` only |
| Waste (D158) | `PodSnapshot::reason`, one `if`/`else` inside the existing gate, two rows partition | `finished_pods_left_behind` at `:1981` — one loop over `finished(pod)`, `removed`/`completed`, no second filter |
| Posture | one row per host path, `Info`, no badge, opening `Row::Prose`, partition per (pod, path) | `posture()` at `:2109`, `host_paths` dedupes into a per-pod `BTreeMap` before merging (`:2274-2301`) |
| Versions | three minor versions via N4; *could not compare* and *could not read* are two sentences | `behind_row` gates on `kubelet_too_far_behind`; `SOME_UNMEASURED` vs `NOTHING_COMPARABLE` at `:2872`, `:2879` |
| Certificates | C1 by identity, the only `Jump::Finding`; badge `…d`/`out`; C3 one `NotComputed`; C2 not drawn | `c1()` matches `ObjectKind::Other("kubeconfig")`; `expiry_badge` at `:3030`; `kubelets_waiting_to_join` at `:3075` |
| Restarts | three-clause filter, `Info` throughout, `Jump::Object`, two `detail` paragraphs, no cap, tie on the younger run | `serving_and_restarting` at `:2600`; `cycling.sort_by` at `:2481` compares `b.started.cmp(a.started)`, a moment and not `age`'s string |
| Positive **and** negative per producer | seven test modules, each with negatives | `src/analysis_tests/{capacity,drain,waste,posture,versions,certificates,restarts}.rs` — 18 / 7 / 16 / 9 / 10 / 4 / 16 negative-shaped assertions |

## 6 — The binary run, over the fixtures the Waste split is about

```
$ cargo build --release
$ ./target/release/k8rs --analysis tests/fixtures/evicted.json tests/fixtures/succeeded.json \
    tests/fixtures/nodes.json tests/fixtures/services.json tests/fixtures/endpointslices.json \
    tests/fixtures/persistentvolumeclaims.json tests/fixtures/healthy-replicasets.json \
    tests/fixtures/poddisruptionbudgets.json
```

The Waste pane, verbatim:

```
[waste]
  Things that cost you something for nothing
  ● default/broken-noendpoints matches no pod
      This Service points at nothing. Anything calling it gets a 503.
      → fix its selector, or delete it
  ▲ default/broken-unused-disk is 128Mi nobody is using
      A disk was reserved for it and no pod is mounting it. It stays reserved until somebody
      deletes it. A StatefulSet keeps its pods' disks by default, even after it is scaled down,
      so some of this is normal.
  ▲ default/healthy-disk is 64Mi nobody is using
      ...
  ○ 1 pod was removed by a node and remains
      Either the node was short, or the pod went over its own disk limit (Evicted).
      → look at one of the pods — its own message names what ran out
  ○ 1 pod finished and was never removed
      Kubernetes keeps a few finished Jobs by default, so some of this is normal. It uses no CPU
      or memory — it only makes every pod list longer.
```

Two rows, both `Info`, summing to the one count the single row used to draw. The
Drain safety pane in the same run drew `would never finish draining` over the node
N1 calls silent and `is ready to drain — nothing on it would move` over the other
three, which is `D134`'s green-light fix still holding.

## 7 — The scale measurement, and why it is half a measurement

The seven producers are pure functions over the whole snapshot, and both
`capacity` and `drain_safety` call `pods_on` once per node — a full linear scan of
`snapshot.pods` each time — so both are O(nodes × pods), with `node_overcommitted`
inside `node_row` scanning again.

Measured at the shape the review brief names — **5000 pods across 200 nodes**,
synthesised in the scratchpad from the committed `healthy.json` and `nodes.json`
templates and deleted afterwards:

```
$ ./target/release/k8rs                 $S/big-pods.json $S/big-nodes.json   # 3 runs
  0.517 s   0.563 s   0.488 s
$ ./target/release/k8rs --analysis      $S/big-pods.json $S/big-nodes.json   # 3 runs
  0.531 s   0.499 s   0.507 s
```

The seven reports add nothing detectable at that size; both figures are dominated
by JSON parsing (1 pod + 200 nodes runs in 0.012 s).

**The larger node counts could not be measured.** A first pass at 2000 nodes ×
5000 pods showed a delta, then the same inputs varied 1.6 s → 4.0 s run to run,
and:

```
$ uptime && nproc
 13:21:19 up 20 min,  1 user,  load average: 18,80, 10,34, 5,67
12
$ ps -ef | grep mutants
timeout 590 just mutants --shard 0/8 --jobs 4
bash scripts/mutants.sh --timeout 90 --file src/rules.rs --file src/analysis.rs --shard 0/8 --jobs 4
```

The PM's re-close sweep was saturating the machine. Every number past the 200-node
row above is discarded rather than reported.

## 8 — What was checked and found clean

- **Sorting** (`PRIOR-ART § F1`, *sorting the rendered string instead of the
  value*). Every comparator in `analysis.rs` sorts a typed value: `bool`/`&str`
  (Capacity), `u8` band (Drain safety), `u32` gap (Versions), `usize` pod count
  (Posture), `i32` restarts then `&Time` (Restarts), `(namespace, name)` tuples
  (Waste's two per-object sections, and the budget list, whose own doc records the
  earlier defect of sorting the joined `namespace/name` string). Restarts'
  comparator explicitly refuses `age`'s string — the F1 defence, made in the file.
- **`PRIOR-ART § F2`, a number with an incomplete denominator.** `control_plane_line`
  draws `N of M` only when every kubelet was measured and splits the sentence
  otherwise; Capacity's promised sum switches the whole node section off under a
  namespace scope rather than coming out low; Waste's counts are list lengths;
  Restarts never divides; Posture names three namespaces and counts the rest.
- **RBAC degradation.** Each of the seven either cannot fail (Restarts, Posture —
  pod data only) or draws a `Row::NotComputed` naming the missing verb and
  resource: `list services and endpointslices`, `list persistentvolumeclaims`,
  `list replicasets`, `list poddisruptionbudgets`, `list nodes`,
  `list certificatesigningrequests`, `read access to node metrics`. Every one of
  those is in the documented read-only `ClusterRole` (`docs/security.md:96-123`),
  so the documented role runs all seven.
- **Two panes counting one thing two ways.** Capacity's node verdict is
  `node_overcommitted` (N5) itself, not a second comparison; Drain safety's
  not-ready reading is N1's card off the findings slice, not a second read of
  `conditions[Ready]`; Versions' rows are gated by N4; Certificates' row and badge
  both come from one `c1()`; Posture's partition against rule 8 is asserted both
  ways in the tests. No producer re-derives a rule.
- **Invariant 4 and the command log.** No producer builds a kubectl line. Drain
  safety names `--ignore-daemonsets` and `--delete-emptydir-data` inside prose and
  documents at `analysis.rs:754` why neither reaches the command strip: the strip
  shows only a command k8rs actually ran, and this pane never drains anything.
- **Invariant 5.** No clock call in `analysis.rs`; `snapshot.now` is the only time
  source (`expiry_badge`, `restarts`). No `Result`, no network, no globals.
- **Every `screens/analysis.md § …` citation in `analysis.rs` resolves** — 29
  distinct citations checked against the file's headings and text; no dangling
  section.
- **`main.rs`'s `reports()` order matches the sidebar** — capacity, certificates,
  drain safety, posture, restarts, waste, versions, which is the order every
  `ANALYSIS` mockup in `screens/` draws.

## 8a — One narrow inconsistency the family already ruled on once

`analysis.rs:2481-2488`, the Restarts comparator, breaks its third tie on the
**joined** `namespace/name` string:

```rust
    .then_with(|| qualified(&a.pod.id).cmp(&qualified(&b.pod.id)))
```

`drain_row` (`analysis.rs:897-907`) records why the budget list stopped doing
exactly that: `'-'` (0x2D) sorts before `'/'` (0x2F), so `team-a/api` comes out
before `team/web` while `kubectl get -A` prints `team web` first
(`reports/2026-08-21-family-c-analysis-report-family-review.md` § 7). Waste's two
per-object sections and the budget list all key the `(namespace, name)` tuple now;
this one line does not.

It is reachable rather than theoretical: `Time` is second-granular, so two pods in
different namespaces with the same restart count that came back in the same second
— which is what a node reboot across a DaemonSet produces, and `D137` measured a
reboot taking the qualifying set from 6 to 17 — hit the third tie-break. What it
costs is an order that differs from the reader's own `kubectl get pods -A`, on a
pane where nothing else is wrong. `analysis.rs` freezes at this close.

## 9 — What could not be proven here

- The whole-file mutation sweep. It is the PM's and it was running during this
  review; its numbers are not re-taken in this file.
- Any behaviour whose subject is time on a live cluster (`D137`'s own lesson). No
  cluster was brought up for this review.
- Anything past 200 nodes, for the reason in § 7.
- `screens/` fidelity below the level of *the section exists and says what the
  code cites it for* — that is `tui-designer`'s tree, not this one.
