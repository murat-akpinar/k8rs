# todo — work plan

> The project plan. Items get checked off here when done (CLAUDE.md workflow
> step 7). Every code step runs the full workflow: code → review → build →
> test → security check → loop until green → check off here → git-cliff →
> commit & push on its own branch.
>
> **Forward-only rule (pyramid) and layer order: see CLAUDE.md.** In short:
> earlier steps' files are frozen; if a step would need to change one, the
> plan is wrong — fix the plan, not the code.
>
> **Replanned 2026-08-11** after the scope reversal (read-only → managed
> writes, resource browser, analysis reports, "lazygit for Kubernetes"
> positioning). The old plan's phases 1–3 survived unchanged; everything from
> the UI up was rewritten. Reasoning:
> [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11).

## Milestones — each one is usable on its own

| | Phase | What you can do with it |
|---|---|---|
| **M1** | 3 | The diagnosis engine is correct against real fixtures — it prints everything wrong, in plain language |
| **M1.5** | 5 | **Shipped: v0.0.1 on crates.io.** `k8rs --once` points at a live cluster, prints what is broken, exits ([D10](NOTES.md#d10--m1-ships-publicly-as-v001)) |
| **M2** | 7 | Every operation works headlessly, proven against kind, with dry-run and audit |
| **M3** | 12 | The console runs: three views, navigation, detail tabs, command log — the actual product |
| **M4** | 13 | v0.1 released — binaries, README, crates.io |

No phase is allowed to be "useless until the end". If one becomes that, the
plan is wrong.

**Branches (2026-08-12):** everything from Phase 3 on is developed on the one
long-lived `development` branch, merged into `main` at each phase close —
which is why those headings no longer name a branch
([NOTES § D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).
Phases 1 and 2 still name theirs because that is where they actually ran.

## Phase 0 — Design (current phase)

- [x] Decision record → `NOTES.md`
- [x] Role-based requirements (developer/devops/devsecops) → `REQUIREMENTS.md`
- [x] Name decision → **k8rs** = `k8s` + `rs`, "Kubernetes, in Rust"
      (crates.io · npm · PyPI · GitHub org · Docker Hub all checked free,
      2026-08-12; [Project name](NOTES.md#project-name-k8rs--decided-2026-08-12-replaces-r7s))
- [x] CLAUDE.md (English; workflow, honest-test and security rules)
- [x] All project files in English
- [x] `docs/`: README.md (index) · architecture.md · security.md · tech-stack.md
- [x] `scripts/check-docs.py` — every relative link and anchor must resolve
- [x] Scope reversal recorded and propagated through NOTES · CLAUDE · docs ·
      REQUIREMENTS · this plan (2026-08-11)
- [x] Pull kube-rs / ratatui / k8s docs into `tmp/` and review the
      architecture against current upstream (2026-08-11). Findings and the
      four corrections they forced:
      [NOTES § Upstream review](NOTES.md#upstream-architecture-review-2026-08-11-docs-in-tmp)
- [x] Capability probe decided — features light up if the cluster has them and
      say so plainly if not; this closes the Prometheus/Istio question without
      needing an answer
      ([NOTES § Capability probe](NOTES.md#capability-probe--if-it-is-there-it-works-if-not-it-says-so))
- [x] YAML crate decided by spike → **`serde_yaml_ng`**; neither candidate
      preserves comments, so `e` keeps the user's text buffer as the source of
      truth and parses only to validate
      ([NOTES § YAML crate](NOTES.md#yaml-crate-decided-by-spike-2026-08-11--and-the-edit-model-it-forces))
- [x] Second-pass design review (2026-08-11) — fourteen decisions on audience,
      the Alerts/Analysis dividing line, owner grouping, namespace scoping,
      operation order, the honesty of the audit trail, the key map and the
      licence; propagated through NOTES · CLAUDE · REQUIREMENTS · docs · this
      plan ([NOTES § Design review](NOTES.md#design-review--second-pass-2026-08-11))
- [x] Audit of this plan against those decisions — four real errors found and
      fixed: the licence that would have blocked step one, rule 12 with no
      fixture to test it, the ops subcommand contradicting the no-clap rule,
      and a justfile frozen one phase too early
      ([NOTES § D14](NOTES.md#d14--three-plan-corrections))
- [x] `screens/` — one mockup per screen at 80×24 (alerts, resources,
      analysis, detail tabs, dialogs, help, states). The code has to match
      these, so Phase 11 has nothing left to invent
- [x] `screens/widgets.md` — element → ratatui widget → who owns the state, so
      Phase 11 transcribes instead of designing. Three decisions came out of
      it: no mouse, no animation, and one sentence instead of a layout below
      80×24 ([NOTES § D15](NOTES.md#d15--the-widget-layer-and-what-it-rules-out))
- [x] Two open ends closed (2026-08-11): the **cluster switcher** `X`
      ([screens/context.md](screens/context.md) ·
      [D16](NOTES.md#d16--the-context-switcher)) — "switchable" was claimed and
      never designed — and the **`--once` output**
      ([screens/once.md](screens/once.md) ·
      [D17](NOTES.md#d17--the---once-output)), which ships before the TUI and
      had no design at all. D16 forced a plan correction in Phase 5
- [x] **Third-pass design review** (2026-08-11) — the runtime, not the product:
      the clock (which contradicted [invariant 5](CLAUDE.md)), 401 and
      credential plugins, calls that take minutes, an audit log that cannot be
      written, a dialog outliving its object, permissions discovered by
      failing, and Ctrl-Z. Eight items, seven missing outright
      ([NOTES § Third pass](NOTES.md#design-review--third-pass-2026-08-11));
      propagated through CLAUDE · REQUIREMENTS · docs/security · screens · this
      plan
- [x] `k8s-openapi` pin **confirmed against the crate, not from memory**
      (2026-08-11): `0.28.0` offers exactly `v1_32`…`v1_36`, so pinning the
      oldest — `v1_32` — is available and keeps the ±2 minor window.
      **Reversed 2026-08-15: the pin is the *newest* offered
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15))** —
      the ±2 window this box relied on had been false since the first capture.
      The same
      check answered the time question: `meta::v1::Time` is
      `jiff::Timestamp` and the crate re-exports `jiff`, so `Snapshot::now`
      costs no dependency ([NOTES § D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)).
      The **kind node image** version is pinned in Phase 2, where the cluster
      is actually created — it was never a Phase 0 fact

**Phase 0 is closed (2026-08-11).** Every item above is checked. The design is
done: eight files decided, twenty-five decisions recorded, nine screens drawn,
and the runtime reviewed
([NOTES § Third pass](NOTES.md#design-review--third-pass-2026-08-11)).

**Brought forward from Phase 2 (2026-08-11):** a real cluster was stood up
during design, because several assumptions could only be checked against a
live API server — and three of them were wrong. Findings, and the two
documents they corrected:
[NOTES § Verified against a real cluster](NOTES.md#verified-against-a-real-cluster-2026-08-11).
This is a plan change, recorded rather than applied silently.

**Phase 1 does not begin until the user says "start the code phase"** —
[CLAUDE.md](CLAUDE.md) makes that a rule rather than a formality, and closing
the design phase is not the same as opening the next one.

---

## Phase 1 — Claim & scaffold · branch `feat/scaffold`

Goal: the name is safe and the repo builds, lints and tests empty.

- [x] `LICENSE` (GPL-3.0) kept, and `license = "GPL-3.0-or-later"` in
      `Cargo.toml`. **First, not last:** `cargo publish` refuses a crate
      without the field, so the placeholder cannot be claimed before this
      exists ([NOTES § D13](NOTES.md#d13--licence-gpl-30-or-later-reversed-2026-08-12))
- [x] `cargo init` — edition 2024, `rust-version = "1.85"` (**1.88 since
      2026-08-14** — C1's certificate parser forced it,
      [NOTES § D86](NOTES.md#d86--c1s-parser-costs-three-minor-rust-versions-and-the-alternative-was-an-accepted-vulnerability-2026-08-14)),
      release profile
      `lto`/`strip`/`codegen-units = 1`. **No `panic = "abort"`**: the terminal
      is restored by a `Drop` guard as well as a panic hook, and `Drop` does
      not run when a panic aborts instead of unwinding (invariant 8)
- [x] Publish `k8rs` placeholder to crates.io — **`0.0.0`, live 2026-08-12**.
      The name is claimed; the binary prints "not built yet" and points at the
      repository. Not a release: no tag, no GitHub release, no binaries. M1
      ships `0.0.1`, M4 ships `0.1.0`
- [x] `clippy.toml` — present with an empty `disallowed-methods` list and the
      reason written in it: entries arrive with kube in Phase 5, and clippy is
      the *fast local signal*, never the containment guarantee. Nothing calls
      the API at this point
- [x] `justfile`: `check` · `run` · `cluster-up` · `cluster-down` ·
      `fixtures` · **`e2e`** · **`mutants`**. Every target is *declared* here,
      `e2e` and `mutants` included, even though Phase 7 and Phase 3 are what
      use them — a target invented later would be a forward-only violation.
      The body of `fixtures` (the jq sanitizer) is written in Phase 2 where the
      fixtures are, so the justfile freezes there and not here
      ([NOTES § D14](NOTES.md#d14--three-plan-corrections) ·
      [§ D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12))
- [x] `.gitignore` (`/target`, `/tmp`, `/.agent`, `/.vscode`)
- [x] CI: fmt → clippy `-D warnings` → test → **the guards, as one list neither
      this box nor CI enumerates** · rust-cache · cargo-deny · `cargo check
      --target` matrix (musl x86_64/aarch64, darwin). Top-level
      `permissions: contents: read`, every third-party action pinned to a commit
      SHA, no `pull_request_target`. **Naming the guards here was a third copy of
      a list that had already drifted once** — `todo-guard.py` reached `just
      check` and never ran on a push — so the list lives in one place and this
      box points at it
      ([D111](NOTES.md#d111--the-guard-list-exists-once-and-ci-gets-no-new-action-for-it-2026-08-16))
- [x] CI: **the honest-test guards** — [`scripts/test-guard.py`](scripts/test-guard.py).
      It compares tests *declared in the source* against tests *listed by the
      runner* (zero-versus-zero passes honestly on an empty crate; a test
      hidden behind a `cfg` gate does not) and rejects any `#[ignore]` without
      a written reason
      ([NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12))
- [x] **Prove every guard red before trusting it.** Ledger, 2026-08-12 — each
      guard was fed a deliberate violation and watched refuse it:

      | Guard | Violation fed to it | Result |
      |---|---|---|
      | `cargo fmt --check` | mangled spacing in `src/main.rs` | **red** |
      | `cargo clippy -D warnings` | `for i in 0..v.len()` needless range loop | **red** |
      | `scripts/test-guard.py` | a `#[test]` behind `#[cfg(feature = "never-enabled")]` — declared 1, listed 0 | **red** |
      | `scripts/check-docs.py` | a link to `NOTES.md#no-such-anchor` | **red** |
      | `scripts/write-guard.py` | `api.delete()` in `k8s.rs`, the same call in `ops.rs` | **red** on the first, silent on the second (`--self-test`) |

      One correction the exercise produced: the first `fmt` attempt was fed a
      badly formatted file that no `mod` declaration reached, and it passed —
      `cargo fmt` never visits a file that is not part of the crate. The proof
      was invalid, not the guard; re-run against `main.rs` it went red.
- [x] CI: the write-containment check, written as an **allowlist** —
      [`scripts/write-guard.py`](scripts/write-guard.py). The ban list is not
      typed by hand: it is *derived* from every `&self` method of `Api<K>` in
      the kube version in `Cargo.lock`, minus the allowlist (`get*` / `list*` /
      `watch*` / `logs` / `log_stream` / `apiserver_version`), so `cordon`,
      `evict`, `entry`, `patch_scale` and whatever kube-rs adds next are
      covered without anyone remembering to add them. kube is not a dependency
      yet, so the check reports that instead of passing silently; its logic is
      proven today by `--self-test`
- [x] `deny.toml` (advisories, permissive-only licence allowlist,
      crates.io-only sources, `wildcards = deny`). k8rs is GPL; the *dependency*
      policy stays permissive-only, which is a different question
- [x] `cliff.toml` — `filter_unconventional = true`

**🔒 Security gate:** `cargo deny` green · workflows default to
`permissions: contents: read` · third-party actions pinned to commit SHAs ·
no `pull_request_target` · **the allowlist check is proven to fail** by
committing a throwaway `Api::delete` call on a branch and watching CI go red.
A guard nobody has seen fail is not a guard.

**Done when:** every guard has been **seen red** on a deliberate violation and
green after it is removed; `just check` = CI byte-for-byte. Green on an empty
crate is not the bar — it only proves the YAML parses
([NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)).
**Frozen after:** clippy.toml, deny.toml, cliff.toml, CI yaml, the licence
files. The justfile freezes one phase later — see the item above.

## Phase 2 — Test data · branch `feat/fixtures`

Goal: real cluster JSON, safe to commit, reproducible with one command.
Wider than the old plan — the rule set now covers nodes and certificates, and
`analysis.rs` needs a whole-cluster snapshot, not a handful of pods.

> **This phase is CLOSED (2026-08-14), and it ran open underneath Phase 3 for
> two days on purpose.** Deferring the kind cluster trip was the user's call on
> 2026-08-12 and the reasoning is
> [NOTES § D47](NOTES.md#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12);
> its own boxes closed on 2026-08-13 with the first trip.
> **What kept it quotable after that was the debt it left in Phase 3**: twelve
> tests stood on hand-set fields waiting for objects no capture had. The second
> trip brought them back on 2026-08-14 and all twelve were retired onto real
> JSON, so the sentence *"Phase 3 cannot close before Phase 2 does"* has been
> satisfied rather than deleted. **What the deferral actually cost is now
> measured and is worth reading before deferring anything again**: the trip
> found three defects no green build could see — a rule that contradicts itself
> on a clean exit ([D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)),
> a capture host that silently rewrites one field
> ([D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)),
> and a pinned-clock guard walking a third of what it named. Every one of those
> had been shipping for two days.

- [x] Fixture sanitization — [`scripts/sanitize.jq`](scripts/sanitize.jq),
      **landed before any fixture file** (REQUIREMENTS G-5; a leak never leaves
      git history). Payloads destroyed, references kept, and an object carrying
      node identifiers that are not the kind cluster's is *refused* rather than
      rewritten — mangled node names would break the pod↔node joins the
      N-series rules need. Tested by
      [`scripts/sanitize-test.sh`](scripts/sanitize-test.sh) in `just check`
      and in CI.
      **Corrected 2026-08-12, before the capture ever ran:** the first version
      was written against a single object and was a near no-op on the `List`
      that `kubectl get -A` returns — eight of the fixtures below, `nodes.json`
      among them, and the foreign-cluster refusal did not fire either. The
      filter is now path-free and the test feeds both shapes
      ([NOTES § D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12))
- [x] [`scripts/broken.yaml`](scripts/broken.yaml) (extracted from NOTES,
      which now points at it — two copies of a manifest drift). Planned for
      `tests/manifests/`; it lives beside the script that applies it instead,
      because no Rust test reads it
- [x] [`scripts/healthy.yaml`](scripts/healthy.yaml) — the negative side: a
      genuinely healthy pod (ready, limits set, an init container that
      *succeeds*) and a Deployment that rolled out cleanly, the negative side
      of W1/W2. Every rule needs a healthy counterpart or its false-positive
      test is fiction. Applied by `cluster.sh break` alongside the broken ones,
      so both sides are captured from the same cluster at the same moment.
      Planned for `tests/manifests/`; it lives beside the script that applies
      it, for the same reason `broken.yaml` does
- [x] **Pin the kind node image** — `kindest/node:v1.36.1`, pinned in
      [`scripts/cluster.sh`](scripts/cluster.sh). Write the version down — the fixtures are
      captured from it, so an unpinned image means fixtures that change when
      someone re-runs `just fixtures` on a different machine. The pinned
      `v1_32` types talk to anything in the window and above it (forward
      compatibility), so this is a reproducibility choice, not a compatibility
      one — *and the second half of that sentence was wrong: they talk to it and
      silently drop what it added
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)),
      which is why the image and the pin are now compared by
      `scripts/fixture-audit.sh`*
- [x] kind cluster up, states settled (CrashLoop in backoff, OOM kills seen).
      `cluster.sh verify` asserts each one reached the state its rule is
      about — **23/23 pass** against the real cluster on the second trip
      (2026-08-13), plus the three node predicates `break-nodes` adds after the
      pod capture: 26 in all *(the count moves whenever a fixture does — it was
      13 on the first trip)*. A fixture that
      never reaches its state is a test that cannot fail, and that has to be
      caught
      before anything is captured. The predicates that decide it are
      themselves proven offline by
      [`scripts/verify-test.sh`](scripts/verify-test.sh), which found one that
      matched crashlooping pods as readiness failures.
- [x] `broken-init` added to [`scripts/broken.yaml`](scripts/broken.yaml) and
      to `cluster.sh verify`
      ([NOTES § D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12))
- [x] `just fixtures` written: `cluster.sh verify` and the sanitizer test run
      *first*, then the 10 broken pods + the healthy pair + the quota workload's
      Deployment and ReplicaSets + **nodes, deployments, statefulsets,
      daemonsets, services, PVCs, PDBs** + the `K8S_VERSION` stamp. Every
      object goes through `sanitize.jq` on the way out — never afterwards.
      *(The list grows with the boxes below: `owned-pods` and
      `owned-replicasets` were added with the owned-pod fixture. This item
      describes the recipe existing, not a frozen inventory — the recipe
      itself is the inventory.)*
- [x] **Run it**: `just cluster-up && just cluster-down` wherever docker is, then
      `just fixtures`, then eyeball the output. Nothing above is proven until
      the capture has actually run against a cluster.
      **Half done, and the box stays open for the other half (2026-08-12):**
      `cluster-up`, `break`, `verify` (13/13) and `just fixtures` (23 fixtures
      from v1.36.1) have all run for real. **`just cluster-down` has not** —
      the cluster is deliberately left standing so the repo's own
      reproduce-it-yourself instructions can be checked against it. The
      teardown is the one step that strips `broken-stuck`'s finalizer, so it
      is the one that can still fail; it gets run once no further capture
      needs the cluster — and one still does, so **Phase 3 opens with this box
      open**: Phase 4's Drain-safety and Waste reports need four positive
      fixtures that cluster could not produce yet, and both visits happen in
      one trip. The host it stood on is gone, so that trip is a `cluster-up`
      rebuild from the pinned image, run at the Phase 3 close
      ([NOTES § D33](NOTES.md#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)).
      Closed — [D33](NOTES.md#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12) · [D91](NOTES.md#d91--the-tests-split-and-the-product-file-does-not-2026-08-15) · [D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12) · [D64](NOTES.md#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
- [x] **A broken pod that has an owner** — added to
      [`scripts/broken.yaml`](scripts/broken.yaml) and captured on the same
      trip. Every pod fixture in the repo has `ownerReferences: null`, so the
      grouping key's four workload branches would ship tested only in their
      no-owner case, and mutation testing cannot object to a branch nothing
      exercises. A Deployment with a crashlooping pod covers
      Deployment/ReplicaSet in one object
      ([NOTES § D36](NOTES.md#d36--the-finding-shape-the-review-sent-back-2026-08-12)).
      Closed — [D36](NOTES.md#d36--the-finding-shape-the-review-sent-back-2026-08-12)
- [x] **A mirror pod**, captured on the same trip: `kubectl get pods -n
      kube-system -o json` from the kind cluster. kubelet writes an
      `ownerReference` of kind `Node` onto every static pod, which is the one
      shape that makes a Node an owner — and it is the claim behind the ruling
      that a Node in the owner role is the no-owner case. Right now that
      behaviour is documented upstream and asserted by nobody here; a capture
      turns it into a fixture the rule can be tested against
      ([NOTES § D39](NOTES.md#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)).
      Closed — [D39](NOTES.md#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12) · [D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) · [D62](NOTES.md#d62--the-fifth-place-a-node-name-lives-and-a-guard-that-asked-less-than-its-consumer-2026-08-12)
- [x] **The shapes the first capture could not produce**, all on the same trip.
      Field-level mutation of the Phase 3 decode found 32 fields that could be
      corrupted with the whole suite staying green, and the cause was the
      cluster, not the tests
      ([NOTES § D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).
      Each is decoded from a one-field synthesis until the real object lands
      here, and each synthesized test names the object it is waiting for.
      What the trip owes, by the file that has to change:
      - [`scripts/broken.yaml`](scripts/broken.yaml): a **StatefulSet** at all
        (`statefulsets.json` is an empty list — the one hole synthesis cannot
        fill) · a **Deployment whose second revision has a bad image**,
        captured mid-rollout, which gives a partially-ready workload *and* a
        ReplicaSet in one object · a **DaemonSet with a broken image** for the
        third workload kind · `broken-pending` respun with a **`nodeSelector`
        + toleration** instead of an unschedulable cpu request, so N6's pod
        side has a real object · a **`subPath` on the hostPath mount**, which
        is the shape that walks past rule 8's docker.sock escalator, plus a
        **second container mounting the same volume** so the per-container
        attribution has two entries to tell apart
        ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12))
      Closed — [D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12) · [D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) · [D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12) · [D63](NOTES.md#d63--the-field-kubectl-never-writes-and-a-substitution-test-that-could-not-see-a-clause-2026-08-12) · [D53](NOTES.md#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12) · [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D64](NOTES.md#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
- [x] A multi-node kind config — N-series rules (cordon, skew, pressure) and
      drain safety cannot be captured on a single-node cluster. Three nodes
      (1 control-plane + 2 workers); `K8RS_WORKERS` changes the count.
      **Four since the `break-nodes` box below (2026-08-12):** the default is
      three workers, one per node state — cordoned, tainted, kubelet stopped —
      because a node carrying two of them is not the object either rule is
      about. `K8RS_WORKERS=2` still runs everything except `break-nodes`, which
      refuses rather than doubling up
- [x] **A workload with zero pods** — `broken-quota`, in its **own
      namespace** `k8rs-quota`: a `pods: "0"` quota applies namespace-wide, so
      leaving it beside the others would have blocked every pod above from ever
      being created again on a re-apply. `cluster.sh verify` asserts the
      ReplicaSet's `ReplicaFailure` instead of a pod state, because the whole
      point is that no pod object exists
      ([NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12))
- [x] A cluster-wide snapshot fixture (everything at one instant) for
      `analysis.rs` reports — `nodes`, `deployments`, `statefulsets`,
      `daemonsets`, `services`, `persistentvolumeclaims`,
      `poddisruptionbudgets`, all captured `-A` in one run.
      **Four of them are empty lists**, because nothing in `broken.yaml`
      produces a StatefulSet, a PVC, a PDB or a dead-selector Service: the
      Drain-safety and Waste reports therefore have a negative fixture and no
      positive one. That is a Phase 4 gap, recorded here rather than hidden by
      the tick — the snapshot itself is captured and reproducible
- [x] Certificate fixtures — [`scripts/make-certs.sh`](scripts/make-certs.sh)
      writes three client certificates to `tests/fixtures/certs/`, generated
      locally, never a real one, and the private keys are deleted as soon as
      openssl is done with them. **Dates are pinned, not relative**: a cert
      generated with `-days 20` is a test that passes today and fails in three
      weeks, and the usual repair for that is to weaken the test. Against a
      reference `now` of 2026-08-12: expiring (24 days → C1 warns), healthy
      (365 days → silence), and already-expired (C1 must say "expired", and the
      renderer must not produce a negative duration). Not wired into
      `just fixtures`: each run would rewrite the bytes for no reason —
      re-run it only if the files are lost
- [x] A **pending CSR** fixture for C3 — kind produces only
      `Approved,Issued` ones, so it has to be created deliberately on the
      cluster ([NOTES § Verified](NOTES.md#verified-against-a-real-cluster-2026-08-11)).
      [`scripts/make-csr.sh`](scripts/make-csr.sh) creates one signed by
      `kubernetes.io/kube-apiserver-client`, which the built-in approver
      deliberately ignores — watched sitting Pending for two minutes on a
      cluster whose own kubelet CSRs are approved within seconds. Not wired
      into `just fixtures`, for the same reason `make-certs.sh` is not: every
      run mints a new key and a new `creationTimestamp`
- [x] Eyeball every fixture once: no env values, no annotations, no node IPs,
      no private keys. **All four are enforced now** rather than by the
      eyeball — by `sanitize-test.sh` on the filter and by
      [`scripts/fixture-audit.sh`](scripts/fixture-audit.sh) on the committed
      bytes, because a fixture can reach `tests/fixtures/` without ever
      meeting the filter. The pass happened too, and it is what found the two
      framing gaps the guards had: an address inside a message and a
      base64-wrapped key
      ([NOTES § D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
      Closed — [D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12) · [D52](NOTES.md#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)
- [x] **`sanitize.jq` must refuse a CSR's `.spec.username` and `.spec.groups`
      before the next capture runs.** `csr-pending.json` carries
      `kubernetes-admin` and `kubeadm:cluster-admins` — kind defaults, so
      nothing leaked, and that is luck rather than the guard. A CSR captured
      from a real cluster carries an OIDC email or
      `system:serviceaccount:prod/deployer` there. It takes the **node-name
      treatment — refused, not rewritten**: a requester identity is a
      reference, not a payload, and an allowlist of acceptable usernames would
      be a judgement call re-made on every capture. This is Phase 2's because
      Phase 2 owns capture, and it lands *before* the trip, not after
      ([NOTES § D52](NOTES.md#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)).
      Closed — [D52](NOTES.md#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12) · [D58](NOTES.md#d58--a-phase-2-box-was-passed-over-and-the-order-it-comes-back-in-2026-08-12) · [D59](NOTES.md#d59--the-sanitizer-refuses-a-requester-and-an-exit-status-guard-cannot-see-a-deletion-2026-08-12) · [D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12) · [D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12) · [D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)

**Done when:** `just fixtures` regenerates the captured fixtures from scratch
and they are committed. **It does not regenerate the certificates or the CSR**,
and that is deliberate rather than an omission: their dates are pinned, so there
is nothing for a re-capture to refresh, and re-running the generator writes
private key material into the repo for no gain. `just fixtures` runs
[`scripts/certs-test.sh`](scripts/certs-test.sh) over the committed ones
instead, which is the assertion that matters.
**Frozen after:** the data layer (fixtures change only via re-capture, never by
hand) **and the justfile — with one declared exception**: the `e2e` recipe
carries a placeholder body and the file says so at its declaration, because the
write path it drives does not exist until Phase 7. Phase 7 writes that body and
nothing else in the file. Reading the freeze as absolute would leave Phase 7
unable to do what the justfile itself instructs it to.

## Phase 3 — The product: rules · **milestone M1**

*Also read: [PRIOR-ART § F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended) (never divide by an incomplete denominator) and [§ F3](PRIOR-ART.md#f3--container-semantics-moved-underneath-them) (container semantics move under a rule that pairs by position, or assumes a status has a declaration).*

Goal: k8rs diagnoses correctly, headless. Still the core — everything else in
this plan is delivery mechanism for what this phase produces.

**The open boxes below are two families, and each is briefed and reviewed as one
turn** ([D106](NOTES.md#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16)).
They are not reordered — the brief names them, the file keeps them where the text
that cites them expects to find them.

- **Family A — what the object supports, against what the card claims.** The
  lost init-container status · `the last thing it logged was:` · `exit 255` after
  a node reboot · what `lastState` means on a container that terminated before ·
  a status with no declaration. Shared helpers: `ending`, `exit_meaning`,
  `exit_fact`, `container_snapshots`.
- **Family B — what the card tells the reader to do, on the shape it is drawn
  about.** Rule 6's undatable `ContainerStatusUnknown` card · event expiry taught
  on one role · rule 1's `Failed | None` arm · the dead first instruction ·
  `screens/alerts.md`'s action budget.

- [x] `Finding` struct (severity · title · evidence · action · kubectl_cmd ·
      **owner** — the grouping key: Deployment/StatefulSet/DaemonSet/Job, or
      the bare pod when it has no owner. Grouping itself happens in `views.rs`;
      the *identity* it groups by is decided here, in the bottom layer).
      **Wider than the box asked, after two operator reviews sent it back:**
      `Finding` also carries `object` — what the finding is *about* — because
      one crashlooping pod fires four rules and a card counting findings says
      "4 of 5 pods" about one pod. `kubectl_cmd` and `uid` are `Option`,
      `ObjectId` drops `Hash` so the uid cannot leak into the grouping key,
      and `ObjectKind` gained `CronJob` `ReplicaSet` `Node` `Other(String)`
      ([NOTES § D36](NOTES.md#d36--the-finding-shape-the-review-sent-back-2026-08-12)
      · [§ D38](NOTES.md#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12)
      · [§ D39](NOTES.md#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12))
- [x] The snapshot types live here, in the bottom layer: `PodSnapshot`,
      `NodeSnapshot`, `WorkloadSnapshot`, `ClusterSnapshot`. `k8s.rs` will fill
      them later; rules define the contract.
      **Nine fields wider than the first draft, after the operator review** —
      each one a field the API sends that the decode dropped, which a green
      suite cannot see and a frozen file cannot recover
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)):
      `Running` carries `started_at`, containers carry `started` and `image`,
      the pod carries `conditions[Ready]` whole, `finalizers` and `mirror`,
      hostPath mounts carry `subPath` and the mounting container, `init: bool`
      becomes a three-way role so a native sidecar is neither an init container
      nor a regular one, and `ClusterSnapshot` carries `namespace_scope` —
      without which a rule cannot tell a small cluster from a partial view of a
      large one, and D43's own ruling is unimplementable.
      Closed — [D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) · [D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)
- [x] **`Snapshot` carries `now`**, and every fixture pins it. Rule 12 and the
      certificate rules need the time; calling a clock inside a rule would
      break [invariant 5](CLAUDE.md) and would make fixtures expire — a test
      that rots is a test that gets weakened
      ([NOTES § D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)).
      **The field is `Time`, not a bare `jiff::Timestamp`** — the same newtype
      every decoded API timestamp already wears, so the comparison every rule
      makes is two values of one type
      ([NOTES § D54](NOTES.md#d54--now-is-metav1time-not-a-bare-jifftimestamp-2026-08-12)).
      Closed — [D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact) · [D54](NOTES.md#d54--now-is-metav1time-not-a-bare-jifftimestamp-2026-08-12) · [D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12) · [D55](NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) · [D56](NOTES.md#d56--c1-cannot-represent-never-expires-and-a-rule-may-not-return-a-result-2026-08-12)
- [x] `Finding` carries **timestamps, not phrases**. "4 min ago" is formatted
      by the renderer, so `ui.rs` and the `--once` printer share one source and
      a test asserts a duration instead of parsing English. A non-positive age
      renders "just now" — the API server's clock and the laptop's disagree.
      **The timestamp is an `Option`**: N2 has no moment to point at, and a
      zero there draws as 1970
      ([NOTES § D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)).
      **The box's own premise moved under it**: D64/D65 falsified D43, so the
      `Option` survives for the *hand-applied* taint and not for the cordon —
      the fixture test now asserts both halves off one capture, `2 hours ago`
      for the taint the controller stamped and nothing at all for the one
      `kubectl taint` wrote. **The one shared source is `rules::age` in
      `rules.rs`**, not in a renderer: `ui.rs` is Phase 11 and the `--once`
      printer is this phase, and the ladder's rungs are the strings `screens/`
      already prints rather than the formatter's choice. Sub-second ages join
      the negative ones in "just now" — `0s ago` reads as a stopped clock
      ([NOTES § D68](NOTES.md#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13)).
      Closed — [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D68](NOTES.md#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13) · [D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)
- [x] **Close the one row where `just check` is not CI** — `tester`'s, not
      `dev-core`'s, and it touches `justfile` / `.github/workflows/` only, so it
      runs alongside the rules work rather than ahead of it (disjoint trees).
      **It is deliberately not first**: it gates nothing, and the rules work is
      the phase (2026-08-13, the user's call). Dispatch it in parallel with
      whichever rules box is running — the trees are disjoint — rather than
      waiting for a slot.
      CI cross-compiles the release targets with
      `cargo check --locked --target <t> --all-targets`; `just check` runs
      nothing equivalent, so a cross-compile break is discoverable only after a
      push — the exact failure CLAUDE.md's "`just check` is the whole of CI, or
      it is a lie" exists to prevent, and the workflow's own comment says why
      (it breaks at link time, and late). **The decision is which cost to pay**,
      and it is `tester`'s to make: requiring the targets makes the gate red on
      any machine that has not run `rustup target add`, and a gate red by
      default is one everyone learns to wave through, while a skip when the
      target is missing is only acceptable if the skip is loud enough to survive
      a green run. Found by Phase 2's closing second pass in a Phase 1 artifact,
      which is why it is a box here and not a reopening there
      ([NOTES § D66](NOTES.md#d66--just-check-is-not-quite-the-whole-of-ci-and-the-gap-is-the-one-ci-was-built-to-watch-2026-08-13)).
      Closed — [D66](NOTES.md#d66--just-check-is-not-quite-the-whole-of-ci-and-the-gap-is-the-one-ci-was-built-to-watch-2026-08-13) · [D67](NOTES.md#d67--the-cross-compile-row-closed-with-a-skip-and-what-the-skip-costs-2026-08-13)
- [x] Pod rules 1–8 and 12 (stuck Terminating). Rule 9 (no limits) is not an
      Alerts rule — it belongs to the Capacity report in Phase 4; rule 8 fires
      only on the escalated hostPath case. Events-based rule 11 stays deferred.
      **Rule 7 needs a "since when" or it fires on every deploy** — `Running`
      + `ready: false` is also every container waiting on its first readiness
      probe, so a Deployment with `initialDelaySeconds`, a node reboot or a
      scale-up would each paint the screen. **Rule 8's negative side is
      untestable until the `-n kube-system` capture lands** (the Phase 2 box
      below): writable hostPath is the *normal* state of every CNI/CSI/node
      agent, so as specified rule 8 fires CRITICAL on a fresh kind cluster, and
      the discrimination it needs — DaemonSet-owned, in `kube-system` — has no
      fixture. **Both halves of that sentence moved on 2026-08-13**: the
      capture landed, and it showed the discrimination is incomplete —
      `etcd`, `kube-apiserver` and `kube-controller-manager` are `Node`-owned
      **mirror** pods with writable hostPaths, so DaemonSet alone leaves three
      CRITICALs on a healthy cluster and `PodSnapshot.mirror` is in the
      contract for exactly this. The narrowing is namespace-bound and known
      wrong outside `kube-system` — see *Later*
      ([NOTES § D70](NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)). **Rule 7's "since when" is `ready.last_transition`, never
      `ContainerStatus.started`** — that field is *always* true once a container
      runs when no `startupProbe` is declared, which is every container in every
      committed fixture, so a rule leaning on it rebuilds the false positive it
      was meant to prevent
      ([NOTES § D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).
      **Rule 2 prints the *enacted* limit** (`status.resources`), not the one
      `spec` asked for. **Rule 12's threshold is `deletionTimestamp` in the
      past, not past-plus-grace** — the apiserver already added the grace when
      it wrote the field
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12))
      ([NOTES § D2](NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later))
- [x] **Rule 10 — Pending, and why**, from `conditions[PodScheduled]`: reason
      `Unschedulable` plus that condition's own message, which is the
      scheduler's sentence. No Events watch, no new stream — the fixture is
      already captured
      ([NOTES § D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)).
      **Three things the box did not say and the operator review did.** It is
      silent when `status.nominatedNodeName` is set — preemption has already
      chosen a machine and the card's sentence is simply false — which cost
      `PodSnapshot` one field. Severity **ladders on the condition's age**
      against the ten-minute grace instead of being flat CRITICAL: an
      autoscaler scale-up, `Immediate`-mode volume provisioning and a
      node-group rollover all carry `Unschedulable` on the healthy path, so
      red is reserved for the pod that has not resolved itself. And it stands
      down entirely on a pod with a `deletionTimestamp` — true is not the same
      as actionable, and rule 12 owns that pod and names the finalizer
      ([NOTES § D73](NOTES.md#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13))
      Closed — [D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12) · [D73](NOTES.md#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13)
- [x] **Rules 1–6 read `initContainerStatuses` too.** A pod at
      `Init:CrashLoopBackOff` produces no finding otherwise, and the finding
      has to name the init container — "the app container is fine, the init one
      is not" is the diagnosis.
      **Three roles, not two** — a native sidecar is an init container with
      `restartPolicy: Always` and lives in the same array, and a crashlooping
      one was producing nothing at all. The role is the evidence's first fact
      rather than six extra titles, and each framing is a property of that kind
      of container, never a claim about the pod. `doing_its_job` is role-aware
      because "serving" is meaningless for an init container: terminated with
      exit 0. **Rule 2's permanent CRITICAL was fixed here too** — one OOM
      never reached rule 5's threshold, so nothing carried it and nothing
      cleared it; it is silent only when the container is doing its job *and*
      the kill is older than the grace
      ([NOTES § D75](NOTES.md#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13))
      Closed — [D75](NOTES.md#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13)
- [x] **Rule 13 — placed on a node, but the containers never started.** The
      twelfth Alerts rule, added on 2026-08-13 by an explicit reversal of
      [invariant 13](CLAUDE.md)'s scope guard: the `ContainerCreating` wedge is
      a weekly failure that no v1 rule sees, and **rule 10 does not see it
      either**, because such a pod *is* scheduled. It fires on the **residual**
      — `conditions[PodScheduled] == True`, no container started, and nothing
      else already explains it (not rule 3's pull reasons, not rule 4's config
      error, not rule 1's loop) — after **10 minutes** measured from
      `scheduled.last_transition`, the same borrow from
      `progressDeadlineSeconds` rule 7 makes, because a large image can
      legitimately take minutes to pull and firing under that alerts on every
      cold start. **WARN, not CRITICAL:** the one healthy thing that still
      looks like this is a slow pull.
      Closed — [D72](NOTES.md#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13) · [D76](NOTES.md#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13)
- [x] **Rule 14 — nothing has even looked at this pod.** `phase == Pending`
      with **no `PodScheduled` condition at all**, older than **2 minutes**
      from `metadata.creationTimestamp` — a field `PodSnapshot` must gain, and
      its window closes at Phase 4 close
      ([NOTES § D42](NOTES.md#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).
      **The two minutes are anchored, not picked:** kube-scheduler's leader
      election defaults to a 15s lease with a 10s renew deadline, so leadership
      moves inside ~15 seconds and two minutes is eight times that — past every
      ordinary restart and failover, short enough to be useful at 3am.
      CRITICAL. **Why it earns its place when the rule set is closed:** without
      it, a wedged kube-scheduler — or a `schedulerName` naming one that is not
      installed or lacks RBAC — leaves every pod Pending while `k8rs --once`
      prints *nothing is broken*, which is the one claim
      [`screens/once.md`](screens/once.md) says has to be true. Rare on a
      managed control plane; not rare on kind, k3s or single-control-plane
      on-prem, which is what this tool's audience runs. **The card names both
      causes and claims neither** — `schedulerName` is not in the snapshot and
      is not being added. **Known and deliberately unsolved:** a cluster-wide
      scheduler outage fires this for every owner and buries the screen;
      telling that apart from one bad `schedulerName` needs cross-pod
      reasoning, and that waits for a real cluster to show the wall is real
      ([NOTES § D74](NOTES.md#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13))
      Closed — [D42](NOTES.md#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12) · [D74](NOTES.md#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13)
- [x] Node rules N1–N6 (NotReady · cordoned · pressure · kubelet skew ·
      overcommit · what blocks a Pending pod). **N1's card has to reach the
      pods, not only the node** — every pod rule reads pod *status*, and the
      status of a pod whose kubelet stopped posting is a fossil that never
      expires, so on a NotReady node the workload that is actually down
      produces no card at all. `healthy.json` is exactly that pod (it runs on
      `k8rs-worker3`, which `break-nodes` made `Ready: Unknown`), which is how
      the gap was found. Without this, Alerts says "node NotReady" in one place
      and nothing about the thing the user cares about
      ([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
      Closed — [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13) · [D65](NOTES.md#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13) · [D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) · [D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12) · [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13) · [D81](NOTES.md#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)
- [x] **Workload rules W1–W2** — W1: the pods were never created
      (`ReplicaSet.status.conditions[ReplicaFailure]`, quota/webhook/PVC
      message shown verbatim); W2: the rollout gave up
      (`Progressing.reason == ProgressDeadlineExceeded`). **W2 fires only when
      no pod-level finding already explains the shortfall** — two findings for
      one problem is how the list stops being believable
      ([NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)).
      **Three passes of the cycle, and the box is the reason the gate order
      exists** ([NOTES § D82](NOTES.md#d82--the-w-series-and-the-card-that-would-have-taught-people-to-mute-the-tool-2026-08-14)):
      the author's own pass was green, the first operator review found **three
      blockers** — W1 paging CRITICAL for a service that was 100% up, W2 silent
      on every rollout at one, two or three replicas because `maxUnavailable`
      rounds down to zero, and `ReplicaFailure` read as creation-only when
      upstream also writes `FailedDelete` — and the **second** review found a
      defect the *first fix* had created, an owner lookup falling back to the
      refused object, which can only ever produce a wrong red card. Six things
      the box did not decide are settled in D82: the shortfall's three arms and
      why each is the only one that sees a shape; the suppression keying on the
      pod's own `Ready` condition rather than `doing_its_job`, which is
      vacuously true for a pod with no containers; the three lookups that fail
      towards *unknown* rather than towards *down*; `FailedDelete` ruled out of
      v1; a counter that may not contradict the severity beside it; and an
      action that may not name a command the object in front of it cannot run
- [x] Certificate rule C1 — kubeconfig client certificate expiry, warn at 30
      days. Pure — and **its input arrives on `ClusterSnapshot` like every
      other rule's**, not through a second entry point: the context name and
      the client certificate, never the private key and never the token.
      "PEM bytes in, finding out" would have been a second signature, which
      [invariant 5](CLAUDE.md) does not describe, and amending a hard invariant
      is a stop rather than a convenience
      ([NOTES § D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)). **It is the one finding with no
      API object behind it**: its `ObjectId` is `kind: Other("kubeconfig")`,
      `namespace: None`, `name` = the kubeconfig **context name** — the
      identifier the user recognises — and `uid: None`, which is the only
      `None` uid in the product. Its `ObjectId` takes its
      namespace from an object's own `metadata.namespace` like every other, or
      from nothing at all. Do not give it the *effective scope*: `--namespace`
      is parsed from `args` with no validation and the kubeconfig context's
      namespace is representable as `""`, so a scope-derived identity makes
      `Some("")` reachable, and `group_key` treats it as a namespace named
      empty rather than as cluster-scoped. If this box does it anyway, it owes
      the test for that shape
      ([NOTES § D38](NOTES.md#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12)).
      Closed — [D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12) · [D38](NOTES.md#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12) · [D56](NOTES.md#d56--c1-cannot-represent-never-expires-and-a-rule-may-not-return-a-result-2026-08-12) · [D87](NOTES.md#d87--c1-has-two-bands-and-they-belong-on-two-screens-d2-only-ever-ruled-on-one-of-them-2026-08-14) · [D2](NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)
- [x] **Rule 5 has rule 1's defect, one rule over** — its card says *"it is
      serving now, but something keeps killing it"*, which is false in exactly
      the way rule 1's *"keeps crashing"* was: over a container whose restarts
      are **clean exits**, nothing is killing it. Reachable — a container that
      exits 0 a few times and then blocks is serving, with a restart count — and
      **no committed capture reaches it**, because every captured restart
      history exits non-zero, so the test is a synthesis on a decoded copy that
      names what a future trip owes ([D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)).
      Cheap now and not before: `fn ending` exists since the rule 1 box, so this
      is that enum applied one rule over rather than a second place naming
      `exit 0`. Found by `dev-core` while fixing rule 1 and **reported rather
      than folded in**, which is the right call — an untested branch invented
      inside someone else's box is the scope creep CLAUDE.md names
      ([NOTES § D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)).
      Closed — [D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12) · [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14) · [D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)
- [x] **Rule 1's clean-exit action offers two readings and the true one is
      missing** — found by the operator review of the rule 5 box, and left to
      its own box by the boundary that box established: it crosses into another
      rule. The action closes *"If it is not meant to finish, it is quitting
      early and that is the bug"* — an exhaustive pair (meant to finish /
      quitting early) with no third door. The third is the commonest loop in a
      real cluster: liveness probe fails → kubelet sends SIGTERM → the app traps
      it and shuts down tidily → `exit 0`, repeat, `CrashLoopBackOff` with
      `lastState.exitCode: 0`. Rule 5 already says the true sentence one state
      over — *"a clean exit says the program stopped without an error, not who
      stopped it"* — so this is that reading applied one rule back, not a new
      idea. **It reopens the command question with it:** rule 1's arm names
      `restartPolicy` and therefore carries `kubectl get pod -o yaml`, which
      prints no events at all, so the `Unhealthy` / `Killing` lines that would
      correct the card are exactly what its command cannot show. One clause and
      one command, and the two cannot be decided apart
      ([NOTES § D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)).
      Closed — [D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14) · [D90](NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)
- [x] **The `137` story is role-blind in both places it is told, and they print
      on one screen** — decided once here, or the two are decided differently.
      **(i) `exit_meaning`'s `137` line names a probe the container may not be
      allowed to have** — *"killed because it did not stop when it was asked to
      — a failing liveness probe, or a shutdown that hangs"*, printed for `137`
      without `OOMKilled`. On a plain init container the first half is
      impossible: `validateInitContainers` rejects all three probes on one that
      is not restartable, which is the fact rules 1 and 5 both now state in their
      own actions. So the evidence line and the action one row apart can
      disagree on the same card, which is
      [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)'s
      shape with the translator on the wrong side of it. **It crosses the rule 5
      box** and gets its own for that reason: `exit_meaning` takes a code and a
      reason and **no role**, and it is printed by rules 1, 5 and 6, so a role
      argument changes three rules' cards and
      [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)'s
      table with them. Decide it once: either the translation becomes
      role-aware, or the `137` line drops the probe and names only what is true
      of every role. **And the same line is wrong a second way, which is why it
      is one decision and not two:** *"did not stop when it was asked to"* is
      asserted for every `137` that lacks the word, but
      [D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
      Closed — [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14) · [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13) · [D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14) · [D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14) · [D90](NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15) · [D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15) · [D94](NOTES.md#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)
- [x] **Rules 1 and 5 print the two reasons `exit_meaning` learned and were
      never taught what either one means** — the half the `137` box above left
      out, on purpose and recorded
      ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
      `exit_meaning` now says *"Kubernetes lost track of the container and wrote
      this code in its place"* on `137` + `reason: ContainerStatusUnknown`, and
      all three rules print it, but only rule 6's action branches on it. So on
      one object: **rule 5** offers *"check the memory limit against what it
      really needs — the kernel takes a container that goes over"* under an
      evidence line saying nothing measured anything; **rule 5's serving title**
      Closed — [D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15) · [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)
- [x] **A container that is terminated *right now* with a bad exit is read by no
      rule as an ending** — rule 1 needs a `CrashLoopBackOff` waiting reason,
      rule 7 needs `Running`, `stuck_at_the_starting_line` returns early on any
      `last_terminated`, and rule 6 reads `lastState` and not `state`. One reader
      does exist and it asks something else: `doing_its_job` looks at the current
      terminated state only to decide whether an *init* container finished. So a
      container sitting in `state.terminated` with `exitCode: 3` between restarts
      draws nothing about the exit; only rule 5's count appears, if the count is
      past the band. **Measured, on the shape that exposed it**: a
      `RestartingAllContainers` pod puts the synthesized `137` into every
      container's `lastState` — the trigger's included — while the trigger's own
      failure sits in `state.terminated`, so with rule 6 exempt from that reason
      nothing reads the `exit 3` that started the whole thing. **The exemption did
      not cause this and removing it would not fix it**: the card it removes is
      about the `137`, and no card was ever about the `3`. Found by the second
      operator review of the `137` box, which also corrected the sentence that
      claimed the sibling covered it
      ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
      Closed — [D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15) · [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15) · [D96](NOTES.md#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)
- [x] **A container that has stopped for good inside a pod that is still
      `Running` is on no k8rs screen, and `kubectl get pods` prints `Error` for
      it** — the shape
      [D96](NOTES.md#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)
      ruled out of the transient class because it is **permanent**, and the one
      the operator review says not to leave at the bottom of a phase. Measured on
      kind v1.36.1: two pods sat at `1/2 Error`, `Ready: False`, `phase: Running`
      for fourteen minutes with `restartCount: 0` and an empty `lastState`, every
      rule in this file silent, while the tool the reader already has open printed
      `Error` in its STATUS column — [D2](NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)'s
      *do not teach them to trust the other tool*, pointing at us. The victims are
      **Job pods and bare pods**: a Deployment-owned pod surfaces as a W-series
      shortfall on its owner, and a single-container pod goes terminal and leaves.
      **The condition is decidable and the obvious version of it is a false
      positive.** `restartPolicyRules` can only *add* restarts — the API rejects
      `DoNotRestart` outright — so *will this container come back* is
      `container.restartPolicy ?? pod.restartPolicy`, with any matching rule
      overriding it upward:
      `Always` yes · `OnFailure` only on a non-zero exit · `Never` no · any
      `restartPolicyRules` entry matching the exit code, yes. **A reader that
      takes the policy and not the rules beside it ships the KEP's headline use
      case as a false positive** — measured, pod `Never` / container `Never` with
      one retry rule on exit `3` was in `CrashLoopBackOff` at five restarts, which
      a policy-only reader calls *stopped for good*. Note also that a **regular**
      container may override the pod at this version (`ContainerRestartRules`,
      beta on by default), so *`Always` restarts everything* is a 1.28 sentence.
      **Two shapes are not in that table at all**: a native sidecar (an init
      container with `restartPolicy: Always`) is restarted until the regular
      containers terminate and is then shut down and *not* restarted, so its
      answer changes with the pod's phase of life rather than with its own
      fields; and a plain init container failing under pod `Never` takes the whole
      pod to `Failed`, which is leg 1's door and not this box's subject.
      **What it needs**: `spec.restartPolicy` on `PodSnapshot` and the rules list
      on `ContainerSnapshot` — the container's own `restartPolicy` is already read
      for `ContainerRole::Sidecar`, so this is two fields and no new join — plus a
      **new capture**, because no committed fixture holds the shape (a pod with
      `restartPolicy: Never`, one container exiting non-zero, one sleeping). The
      capture is the PM's, under `K8RS_CLUSTER=k8rs` and the sanitization gate.
      Decide too what the clean-exit half says: `OnFailure` with `exit 0` beside a
      running sibling is the *Job never completes because the helper is still
      running* shape, which is the same silence with a different sentence.
      **Shipped 2026-08-15 as rule 15, `stopped_for_good`
      ([NOTES § D97](NOTES.md#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)).**
      Four conditions and no branches, because on a bad exit the truth table
      collapses to one arm: `Always` restarts it and `OnFailure` restarts it, so
      only `Never` reaches the rule. One new snapshot field — the **effective**
      policy, the container's own then the pod's — measured into existence: a
      regular container declaring `Never` inside an `Always` pod sits at
      `1/2 Error` while its sandbox lives, and a rule keyed on the pod's policy
      misses it entirely. The card carries the file's **first `kubectl logs`**.
      **The capture is real and it is one file**: the pod went into
      `broken.yaml`, the predicate into `cluster.sh verify`, the capture and its
      guard into the recipe, and the single object was taken with `sanitize.jq`
      off the cluster `verify` had just passed — 51 fixtures, byte-identical
      under a second pass. **It cost a repin**: the new capture is a day newer
      than the corpus, so the pinned `now()` moved to `2026-08-16` and eight age
      assertions and three certificate day-counts moved with it, each witnessed
      red at the old number and green at the new one.
      **The operator review found one blocker, and it was mine rather than the
      rule's.** The action said *read its log — it is still there*, generalised
      from one happy-path measurement. `kubectl logs` is the only command any
      card offers that goes to the **kubelet on the node**, and every one of this
      rule's conditions is read from a pod status that **freezes when that
      kubelet dies** — measured, eight minutes of the card drawing unchanged
      while the command answered `connection refused`, with rule 12's honest card
      beside it. **The rule is most likely to fire exactly when its command is
      least likely to work.** The promise came out, the command stayed, and the
      container's own last words went on the card ahead of the duration, because
      the message is in the API server while the log is on the node.
      **The title kept the promise the action had given up** — *nothing **will**
      start it again*, a prediction measurably false in the container-level
      shape, where a node reboot brings it back because the kubelet reads the
      *pod's* policy when it recreates a sandbox. Both are present tense now, and
      the refusal list that guards them lost its leading `Nothing` so it catches
      the rewording rather than the capitalisation, and runs over title, evidence
      and action alike.
      **What stayed out**: the clean-exit half is silent — a container that exits
      `0` under `Never` is doing what `Never` means, and calling it a fault needs
      the Job above the pod, which is not watched — and the `restartPolicyRules`
      window stays open, one measured second between the exit and the retry,
      because the field is unreadable at the pinned `k8s-openapi` feature
      — *the pin moved the next box down
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)),
      and the window stays open for a different reason: the types carry the field
      now, and nothing prunes it into the snapshot*
- [x] **`k8s-openapi` is pinned at feature `v1_32` while every fixture is
      captured from kind v1.36.1, so four versions of fields decode to nothing
      and nothing says so** — found while designing rule 15, which needs
      `spec.containers[].restartPolicyRules` and cannot read it: that field
      arrives in the generated types at **`v1_34`**, and the crate ships
      `v1_32` … `v1_36` **in the version already in `Cargo.lock`**. So this is a
      feature flag and not a new dependency (invariant 10 is not the gate here),
      but the blast radius is every type in the API surface, which is why it is a
      box and not a line in someone else's. **The drift is silent by
      construction**: serde drops unknown fields, so a 1.36 object decodes
      cleanly into 1.32 types and the missing field reads exactly like a field
      the cluster did not set — the same *found none / there were none* confusion
      the fixture guards exist for, one layer down. `tests/fixtures/K8S_VERSION`
      records the cluster's version and the justfile's comment says a feature
      bump means a re-capture; **nothing compares the two**, which is why this
      has been true for the whole phase without a build going red. What the box
      owes: bump to `v1_36`, then find what else the snapshot has been silently
      dropping — every field the rules read that arrived after 1.32 — and decide
      whether the pin follows the kind image from now on or is chosen
      independently and asserted against it.
      Closed — [D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)
- [x] **A pod that used its own restart rule three times and has served ever
      since carries two permanent WARN cards, and the object says it is over** —
      measured on kind v1.36.1: `gang-restart`, `2/2 Ready`, `phase: Running`,
      three declared gang restarts and then quiet, draws *"Container has been
      restarted 3 times — it is serving now"* **once per container, for the life
      of the pod**, because `lastState` never expires and `restartCount` never
      falls. That is [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)'s
      false-positive class and it is the same argument rule 6 uses to stay silent
      on the same reason. **The ruling that let rule 5 keep drawing was made
      against the other object and it stands** — a pod thrashing at 88 restarts
      draws three cards and would otherwise draw none
      ([D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15))
      — **but both objects exist and only one was on the table.**
      Closed — [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13) · [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15) · [D100](NOTES.md#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15)
- [x] **Nothing reports a container that is fine right now and keeps dying on a
      long cycle, and it is one question for four rules rather than a fifth
      threshold on one** — surfaced by
      [D100](NOTES.md#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15)'s
      cost paragraph, whose first draft claimed rule 6 covers the gap and was
      **measured false by the dev who was told to implement it**: rule 6 returns
      `None` on `doing_its_job(c)` with **no clock at all**, rule 2 carries the
      same clause with the same threshold, and rule 1 needs a backoff state the
      container is not in. So a container that OOMs every thirty minutes, or a JVM
      that dies on the nightly batch, draws **nothing from any rule** between its
      restarts — proved by the two whole-capture `nothing(...)` assertions D100's
      box added. The gap is older than that box and rule 5's permanence was
      covering it by accident, at the price of a card on every pod that has ever
      hiccuped; removing the accident is right and leaves the hole visible.
      Closed — [D100](NOTES.md#d100--the-field-that-separates-a-settled-restart-from-a-live-one-was-already-in-the-snapshot-and-rule-5-never-read-it-2026-08-15) · [D101](NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)
- [x] **Rules 5 and 6 print the identical four-line action on two adjacent cards,
      and no `CrashLoopBackOff` is needed to see it** — on a container past the
      restart band whose last recorded run is a lost status, rule 5 draws the
      count and rule 6 draws *No record of how the container's last run ended*,
      **both carrying `unwatched_action` verbatim**: 26 lines about one container
      in a 16-row pane, rule 7 underneath. The suite prints it in its own
      `--nocapture` output and nobody read it, which is the second time that has
      been the finding in this area. [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)
      recorded the pair as rules **1** and 6 and that is the rarer of the two —
      the backoff instance needs a shape the same review could not produce in ~20
      attempts, while this one needs only a sandbox loss on a container that comes
      back unready, which `lost-notready` already is. **Silence is not the fix and
      D93 already refused it**: rule 6's card is what keeps a reader off
      `logs --previous`, which the API will not serve for this record. What is
      cheap is that the *second* copy of a shared sentence says nothing new — so
      the decision is whether a rule may know that a neighbour already said it,
      which is an `analyze` decision and not a rule's, beside
      `explains_a_shortfall`. Whatever it decides applies to every shared action,
      not to this one pair.
      Closed — [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15) · [D102](NOTES.md#d102--the-second-copy-of-a-shared-sentence-is-dropped-by-analyze-and-not-by-a-rule-2026-08-15)
- [x] **A lost init-container status reads as *finished successfully*, and then
      two rules stand down** — `kubelet_pods.go:2714-2718` synthesizes
      `Terminated { reason: "Completed", exitCode: 0 }` for an init container
      whose status the runtime lost, which is the **third** bare literal beside
      the two [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)
      turned into endings. `ending` reads the `0` and answers `Finished`,
      `doing_its_job` then answers true for an init container, and rules 5 and 6
      both return `None`: **k8rs says nothing at all about a run Kubernetes lost**,
      which is the class the D95 box exists to remove, one literal over. It is
      why that box's `ending` premise had to be narrowed to *the two reasons this
      file has evidence for* rather than *a real exit code means the run was
      watched*. **`0` cannot be keyed on alone** — every finished init container
      writes it — so this needs the `reason` beside it, and the object to prove it
      on is one no committed capture holds. Found by the operator review of the
      D95 box, from D93's own source citation.
      Closed, and **the premise above was the defect** — the source writes
      `reason: "Completed"`, which no genuine finish can be told from, and the
      kubelet is deducing rather than guessing, so silence is the true reading:
      [D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)
- [x] **Rule 6's `ContainerStatusUnknown` card can never carry a date, and
      nothing else can age it out** — `Finding::timestamp` comes from
      `finished_at`, the kubelet's literal sets no stamps, and `lastState` never
      expires while this rule's only suppressor is `doing_its_job`. So a container
      that lost its status once in June and is not ready today **for an unrelated
      reason** carries this WARN for the life of the pod, undated, beside the card
      that names the real problem. Measured on `lost-notready` — a failing
      readiness probe beside a loss long over. **The permanence class for the
      fifth time in this file** (D71 on rule 6, D75 on rule 2, D85, rule 5's
      serving title, here) and the first instance no clock can rescue, because
      there is no clock to read. **One fix was proposed and rejected**: putting
      `restartCount` in the evidence reads as *this happened N times* on a card
      whose subject is one lost status, and the count is every restart from every
      cause — `PRIOR-ART.md`'s incomplete-denominator class, which is not traded
      for a fuller evidence line
      ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
      The reader genuinely cannot tell *once* from *ongoing* and the object does
      not answer it, so the question is whether the card should exist on a
      container whose current trouble is something else — which is a suppressor
      question, not a wording one.
      Closed as a suppressor, in `analyze` and not in a rule (D102's own ruling):
      an undatable card about the past yields to a dated card about the present —
      [D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
- [x] **`the last thing it logged was:` prints text the container never wrote** —
      rule 6 reads `lastState.terminated.message` and presents it as the
      application's own last words, but that field is whatever the kubelet put
      there, and on at least one reason the kubelet writes its own sentence into
      it: *"The container could not be located when the pod was terminated"*.
      The `137` box scoped exactly that one reason out of the log arm by
      answering ahead of it
      ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)),
      which fixes the instance and not the class. **The second kubelet-authored
      message is not hypothetical — it exists today, on the pinned version**:
      `reason: RestartingAllContainers` carries *"The container is removed
      because RestartAllContainers in place"*. Measured on kind v1.36.1 by the
      operator review of the `137` box, with the feature gate at its default.
      **Both are unreachable today and by two different accidents, which is the
      state this box has to survive**: `ContainerStatusUnknown` because an arm
      answers ahead of the log arm, `RestartingAllContainers` because rule 6 is
      exempt from it altogether for an unrelated reason (the pod asked for the
      removal). Neither is the class fix, and a third kubelet-authored message
      would print the lie with nothing in its way. Do not read the green as
      coverage. `terminationMessagePolicy:
      FallbackToLogsOnError` is what makes the field the container's words only
      *sometimes*. Reported by `dev-core` rather than folded in, which is the
      right call ([NOTES § D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)).
      Decide whether the card can tell the two apart from the object at all; if
      it cannot, the sentence stops claiming authorship rather than guessing it.
      Closed on the second half — it cannot, because a **third** author writes
      that field and its records carry stamps: the frame now says who *recorded*
      the line, never who wrote it
      ([D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16))
- [x] **A node reboot writes `exit 255` and rule 6 blames the application for
      it** — measured on kind v1.36.1, not reasoned: `docker restart` of the node
      leaves `exitCode: 255, reason: "Unknown"` with a real containerID and real
      timestamps, because containerd's state survives the reboot and the
      containers are *found*, dead. `exit_meaning` has no row for `255`, so the
      card reads *"The container's previous run failed — exit 255"* over the
      general arm's *"read the logs of that run to find the application's own
      error"* — sending someone hunting an error the application never made,
      after a machine restart. **This is the commonest abnormal `lastState` any
      cluster produces** and it is D85's defect on a path nothing in this area
      has touched. Found by the operator review of the `137` box, which also
      established that the `137` card's own sentence about reboots was wrong for
      the same reason ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
      **`255` is not `137`'s question**, which is why it is here and was not
      folded in: it needs no role split and no `reason`, only a row and an arm.
      Closed — and **the `reason` was needed after all** (a program may exit 255),
      while CRI-O turned out to write `-1` / `"Error"` for the same event and was
      still being blamed for it:
      [D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)
- [x] **Only a container's *first* lost status is ever recorded, so a card
      reasoning from `lastState` is wrong on any container that has terminated
      before** — `convertToAPIContainerStatuses`' second site is gated on
      `LastTerminationState.Terminated == nil`, verified in the kubelet source
      and reproduced: a container that already carried a `255` record and was
      then removed from the runtime came back with `restartCount + 1` and the old
      `255` untouched. So `lastState` is not *"the run before this one"* on such
      a container — it is *"the last run Kubernetes managed to write down"*, and
      every rule that says *previous* means the second thing. Turned up by the
      operator review of the `137` box while verifying the kubelet source. Decide
      what the cards may claim about `lastState` at all; it reaches rules 1, 5
      and 6 and `exit_fact` under all three.
      **Measured since, on a live cluster**: the record stood still at one
      `finishedAt` while `restartCount` went 7 → 16, nine restarts under one
      unchanged `lastState`. **And the surviving instances are now named**, because
      [D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)
      dodged the class rather than fixing it: rule 5's two new clauses were worded
      about **the record** — *the record names no ending*, *the pod's rule is on
      record* — precisely so they stay true after the freeze, while **rule 1's
      titles and rule 6's still say *the container's last run***, which is the
      claim this box says the object does not support. One of them is D93-blessed
      and shipped, so moving it is this box's call and not a wording tidy-up.
      Closed — both moved to rule 5's *on record* frame, and one draft of the new
      wording failed invariant 13 on a bare noun the card never introduces:
      [D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)
- [x] **`screens/alerts.md`'s action budget is false of four shipped cards, and
      the file says that is a `rules.rs` finding** — it caps a card's action at
      two to five lines, never cut ([alerts.md](screens/alerts.md)), and states
      that an action wrapping past five is a rule defect rather than a layout
      problem. **Measured, not estimated:** every distinct action the rules print
      was wrapped at that file's own 49-column width — **9 of 52 exceed the
      budget**, at six to nine lines, and they are spread across rules 1, 5 and
      6 rather than clustered in the newest one. The worst is nine lines, which
      with its title and evidence fills 15 of the 16 body rows, breaking the
      ten-line card cap `alerts.md` sets so the pane can always show a second
      finding — *"a screen that can show only one finding is not a list"*. The
      budget has been wrong since before either rule-5 box; `k8s-admin` found it
      while reviewing this one, so nothing here is new breakage. Re-measure
      before deciding: the command is one `textwrap.wrap(s, 49)` over the `→`
      lines of `cargo test -- --nocapture`. **The evidence cap is in the same
      state**: three wrapped lines, and rule 5's init cards measure four on the
      committed captures — a digest-pinned image spends a whole line on its own.
      Rule 5 answered that by putting the load-bearing fact ahead of the image so
      the *image* is what gets cut, which is the right order and not a fix for
      the cap. **Measured again on a card D95 shipped**, where that order paid
      off and left one thing behind: the serving `RestartingAllContainers` card's
      evidence wraps to four lines at 51 columns, the cut takes the image as
      designed, and what the reader is left holding ends `…what this pod asked
      for) ·` — **a trailing separator with nothing after it**. Allowed by this
      file's own cut rule, so it is the renderer's to trim when `ui.rs` lands, not
      a `rules.rs` defect; it belongs here because this is the box that decides
      whether the cap is the right number. Either the budget is wrong or four
      actions are, and that is a `tui-designer` call on `screens/`, not a
      `rules.rs` one; whichever way it goes, it decides what the plain-language
      pass below has to shorten. **Same file, same box:** `alerts.md` quotes
      rule 5's *"it is serving now, but something keeps killing it"* as the
      example of a serving card that does not silence W2. Still true — that is
      the `Failed` arm — but it is one arm of **six** since the two `137` reasons
      got their own
      ([NOTES § D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)),
      and the passage reads as if it were the whole rule. **And the title is the
      part this file caps by implication only** — the ten-line card is 1 identity
      + 1 title + 3 evidence + 5 action, so a title's second and third lines spend
      a budget nothing reserved. D95's first draft proved it costs something real:
      at a **three-digit** restart count its serving clause made a **3-line title
      and an 11-line card**, over the measured maximum, and it was caught by the
      operator review rather than by anything in the repo. It shipped reworded,
      with `the_cards_this_box_ships_fit_the_height_they_are_drawn_at` measuring
      **those four cards and no others** at exactly 10 lines
      ([NOTES § D95](NOTES.md#d95--the-two-137-reasons-become-endings-and-rule-5-draws-where-rule-6-goes-silent-2026-08-15)).
      Two things that box leaves here: **every other title in the file is
      unmeasured**, and a restart count is the one field that grows with the
      cluster's uptime rather than with a rule's wording.
      **One requirement to carry into whatever rewrite this box produces**,
      deferred here rather than lengthening a sentence already over the cap:
      `stopped_action` names two producers of a repeated polite stop — a health
      check, and a node memory killer — and at this repo's target version
      (`tests/fixtures/K8S_VERSION`) there is a third the kubelet performs
      itself, **in-place pod resize with `resizePolicy: RestartContainer`**,
      which VPA drives on a loop. A reader whose pod is being resized is sent
      past the answer to two places that hold nothing. `describe` prints the
      resize conditions, so it costs nothing in invariant-4 terms — only in the
      line budget this box exists to settle. The string is shared by rules 1 and
      5, so one edit fixes both cards.
      **Re-measured after the rule-1 clean-exit box, which took its own three
      arms from 9 / 8 / 9 wrapped lines to 5 / 5 / 5 and put a `rules.rs` test
      under them** — `the_clean_exit_actions_fit_the_card_they_are_drawn_on`,
      which measures those three strings and no others
      ([NOTES § D90](NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
      **Exactly five actions in the file still exceed the cap**, with the same
      wrap at 49 columns: `stopped_action`'s two arms at 6 lines each,
      `failed_action(Init)` at **8** — a 14-row card, which is the one that
      breaks the ten-line cap outright — rule 5's `None` arm at 6, and rule 6's
      action at 6. That is the whole list; the count fell from nine because two
      rule 1 strings were folded away, not because anything was shortened.
      **The doors are not what costs the space**: three readings fit in five
      lines once the preamble and the restatements come out, which is what the
      rewrite above proved on the hardest of them. **One correction to carry into
      the same rewrite:** `stopped_action` names `systemd-oomd` beside `earlyoom`
      as a producer of a *polite* stop, and `systemd-oomd` kills with
      `cgroup.kill` / SIGKILL and offers no graceful signal — it can only ever
      produce `137`, never `143`, so on that card it sends the reader after a
      tool they will never find in their logs. Only `earlyoom` belongs in that
      sentence, and the doc comment repeats the error. **And the enforcement is
      transcribed, not derived:** the 49 columns and the five-line cap live as
      constants in the test, so this file moving does not turn a build red —
      parsing `alerts.md` is the only stronger option and is bigger than either
      box.
      Closed — **it was neither the budget nor the actions**: the parts table
      permitted 11 while the summary said 10. The cap is 12, derived; the title
      gets one for the first time; and parsing `alerts.md` was measured at ~22
      lines of Python, which retires *bigger than either box* — the guard itself
      is `tester`'s, in a later phase — [D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
- [x] **The clean-exit arms teach that events expire on one role and not on the
      other two, and rule 5 is where that costs something** — the `Init` arm
      says *"the events … last about an hour"* and then names the node; the
      `Regular` and `Sidecar` arms send the reader to the same events with no
      window at all. On **rule 1** that is harmless: `CrashLoopBackOff` caps
      backoff at five minutes, so a `Killing` line is always minutes old. On
      **rule 5** the container is *serving* with ten restarts and the last run
      may have ended hours ago, so `Events: <none>` reads to a beginner as
      *nothing stopped it* — which walks them into door 3, *it belongs in a Job
      or a CronJob*, for the healthy Deployment two operator reviews already
      blocked to protect. Inherited rather than introduced: the pre-rewrite
      string had the same exposure. **It is not free**: those two arms have four
      to six characters of slack at the five-line cap, so closing it costs a door
      unless the budget box above moves the cap first — the two are decided in
      that order ([NOTES § D90](NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
      Closed — the budget box moved first, as this box required, and the window
      is on all three arms — [D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
- [x] **Rule 1's `Failed | None` arm sends the reader to a log no command can
      reach** — *"read the previous run's logs — that is where it says why it
      exits"*, with `kubectl describe pod` on the card, which prints no logs at
      all. That is invariant 4 in the small. The `None` half is worse and is
      rule 5's own fixed defect left standing one rule over: with no `lastState`
      there is no `lastState.terminated.containerID`, and the kubelet gates
      `kubectl logs --previous` on exactly that field — so the card is in that
      arm *because* the flag it implies cannot work. Rule 5's `None` arm was
      rewritten for this in its own box; rule 1's, ten lines below the code the
      clean-exit box touched, was not, and the boundary that left it there is
      [D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)'s.
      Found by `k8s-admin` while checking invariant 4 across the whole match.
      **A measured instance joined it on 2026-08-16**: a container whose start
      failed (`command` mistyped) lands on exactly this arm — *keeps crashing …
      read the previous run's logs* about a container that never ran and has no
      log — while rule 6's card beside it carries the runtime's own diagnosis
      ([D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16)).
      Closed — the arm split, `previous_logs` was written, and the fork turned
      out to be *did the run start* rather than the exit code — [D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
- [x] **The first instruction on a clean-exit card can be dead on the shape the
      card is most often drawn about** — the action opens *"check the pod's
      events for a `Killing` line"*, and with stock probe settings
      (`initialDelaySeconds: 0`, `periodSeconds: 10`, `failureThreshold: 3`) the
      earliest a liveness or startup probe can kill a container is about twenty
      seconds after it starts. `exit0.json`, the fixture this whole line of boxes
      descends from, has *"the last run lasted 2s"* on the evidence line one row
      above. Nothing on the card is false — it says *check* — but the reader
      spends their first move proving the first door shut, and `lasted` is
      already in the snapshot and already on the screen. Whether a rule may
      **order** its doors by a fact it holds is the decision here, and it is a
      new one: no rule reorders its own action today.
      Closed — a rule may, under three constraints: it reorders and never
      deletes, the fact must already be on the card, and the threshold is derived —
      [D113](NOTES.md#d113--a-cards-parts-were-budgeted-separately-and-never-added-up-and-everything-else-this-family-found-was-reached-by-fixing-that-2026-08-16)
- [ ] **The clean-exit boxes left three objects no cluster has produced for us**,
      and two of them are one command each. **(a)** A probe kill that reports
      `exit 0` — the premise of the first clause on two arms of two rules, and
      nothing in the repo holds one. `scripts/broken.yaml`'s `broken-sigterm` is
      one word away: `trap 'exit 0' TERM` instead of `exit 143`. **(b)**
      `Init:CrashLoopBackOff` with a clean run behind it — `crictl rmp` on the
      kind worker while a retrying init container's backoff window is still live;
      it is what makes both `Init` arms reachable and it currently ships on a
      plant plus a source read. **(c)** `restartCount` across a node reboot
      (`docker restart kind-worker`), which is the half of the rule-1-versus-rule-5
      producer asymmetry that is argued rather than seen. All three are `k8s-admin`
      or a capture trip, not a dev
      ([NOTES § D90](NOTES.md#d90--the-third-door-and-the-command-trade-d88-made-a-day-earlier-2026-08-15)).
      **(d) joined on 2026-08-15**: a pod under `restartPolicy: Never` whose
      container declares a `restartPolicyRules` entry matching its own exit code,
      which the kubelet then restarts — rule 15's named false positive, and the
      one object the `restartPolicyRules` box below cannot be written without.
      Four objects, one trip
- [ ] **`crash_looping`'s `if c.restarts > 0` is the one surviving mutant in
      `rules.rs`** — `cargo mutants` reports it MISSED at HEAD and in every round
      of the clean-exit box, in three different line positions, so it is neither
      new nor drifting: flipping it to `>= 0` ships `0 restarts` on a real card
      and nothing goes red, because `CrashLoopBackOff` *before* the first restart
      is a state no committed fixture reaches. It is ~15 lines against
      `healthy-retry` with the count zeroed. **Two smaller test-side residues go
      with it**, both declared rather than found later: the budget guard iterates
      a literal three-element array of `ContainerRole` instead of a `match`, so a
      fourth variant would be silently unmeasured while `finished_action`'s own
      `match` would refuse to compile; and every clause pin is a positive
      substring, which catches a door that is **deleted** and not one that is
      **negated** in place. **A third, and it is the one that will be
      "fixed" wrongly:** both ordering helpers use `str::find`, so they read the
      **first** occurrence — a future arm that names the node both before and
      after the hour, or the verdict on both sides of the conditional, goes red
      although the requirement is met. The bias is toward a false red and never a
      false green, which is the safe direction; whoever meets that failure fixes
      the helper, not the assertion.
      **A fourth, the first residue's class on a second axis, declared
      2026-08-16**: the shared-sentence sweep and the card-height guard both walk
      a hand-written `(code, reason)` list, so a seventh `Ending` would escape
      both silently while `rules.rs` itself refuses to compile without it
      ([D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16))
- [x] Exit-code translation table (137/143/1/126/127) — **137 has four readings, the object names three of them, and where it names none the table refuses to guess**: `reason: OOMKilled` is memory, `reason: RestartingAllContainers` is the pod's own restart rules removing the container and is not a failure at all, `reason: ContainerStatusUnknown` is not a kill at all but the number the kubelet writes where it could not read a status, and with none of those the row names the signal and stops. The old "almost always OOM" row was written before the rule had `reason` beside the code ([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)); **what replaced it named the opposite cause just as flatly and was corrected on 2026-08-15** — *did not stop when it was asked to* is false of an init container that may hold no probe, of a cgroup kill whose word was lost on a starved host, and of a rebuilt sandbox ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15))
- [x] hostPath: `rules.rs` fires **only** on `/`, a container-runtime socket
      **or any directory one sits under**, or a writable host mount. There is no
      lower severity to escalate from any more — the ordinary read-only mount is
      a Phase 4 posture row, computed there.
      **The socket list carries one canonical `/run` spelling per socket and the
      compare strips a leading `/var`**, because `/var/run` is a symlink to
      `/run` and a manifest may write either. It used to hold
      `/var/run/crio/crio.sock` alone, so a **read-only** `/run/crio/crio.sock`
      on a `kube-system` DaemonSet fell through the socket branch *and* through
      D70's writable-branch exemption and drew **no card at all** — the exact
      shape the escalator tests the path rather than the mode for
      ([NOTES § D78](NOTES.md#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13)).
      Closed — [D78](NOTES.md#d78--the-socket-the-escalator-could-not-see-and-the-three-mutations-that-survived-the-fix-2026-08-13) · [D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)
- [x] Rule 5 thresholds (≥3 WARN, ≥10 CRITICAL) — **and CRITICAL only when the container is not serving**, because a red card whose own title says it is serving is what teaches people to ignore red. **Rule 12 does not add a
      second grace period** — the apiserver already wrote *request time +
      grace* into `deletionTimestamp`, and the grace beside it is read for the
      age (`asked_at = deletionTimestamp − grace`, with `checked_sub`), never
      to push the deadline out again
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
      **It does get a skew margin**, which is a different thing and was missing:
      at `> 0` a laptop ten minutes fast files a finding for every pod a
      correctly-progressing rollout has just asked to terminate
      ([NOTES § D55](NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12)).
      Closed — [D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) · [D55](NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) · [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
- [x] **A capture trip for the branches no committed fixture can reach — you
      run this one, not an agent** (`just cluster-up` · edit
      `scripts/broken.yaml` · `just fixtures`). **Seven branches shipped with
      no test that can fail** — the mutation sweep leaves them green because no
      captured object reaches them, and each is named in `rules.rs` where it
      sits. Manifests corrected by the operator review, because the obvious
      version of the first four does not produce the shape on kind:
      - **rule 6's `exit 0` exemption** — `restartPolicy: Always` with
        `command: ["sh","-c","sleep 20; exit 0"]`. The sleep matters: an
        instant-exit container spends its life in `Waiting` and the capture is
        timing-flaky. The kubelet applies `CrashLoopBackOff` to repeated
        restarts *regardless of exit code*, so rule 1 co-fires and the test is
        "only rule 1", never "nothing".
      - **rule 6's `exit 143` exemption** — `command: ["sleep","3600"]` in
        **exec form** so `sleep` is PID 1 and dies on SIGTERM's default
        disposition; `sh -c` risks the shell's own handling and can land 137,
        proving the opposite. Plus `livenessProbe: {exec: {command:
        ["false"]}, periodSeconds: 5, failureThreshold: 1}` and
        `terminationGracePeriodSeconds: 30`.
      - **rule 8's runtime-socket escalator**, two manifests: `hostPath:
        /var/run/docker.sock` with `type: FileOrCreate` mounted `readOnly:
        true` (the `type` is what lets the container start on a kind node,
        which has no such file, and read-only is the case worth proving —
        the escalator fires on the path, not the mode); and `hostPath: /run`
        with `subPath: containerd/containerd.sock`, which puts a **real**
        containerd socket behind the escalator instead of a planted one
        ([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
        **That second manifest no longer proves the join**, which is what it was
        written for: since ancestors match, `/run` alone escalates and the
        `subPath` changes nothing. The join is already captured — `hostpath.json`'s
        `nosy` mounts raw `/` and only reaches `/run/containerd` through it
        ([NOTES § D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)).
        **The originally proposed `/var/run` + `subPath: docker.sock` does not
        work** — the kubelet's subPath preparation fails and the pod lands in a
        permanent error state that pollutes the whole-capture test.
      - **`analyze`'s `Succeeded` skip** — a `restartPolicy: OnFailure` Job
        whose counter lives in an `emptyDir` (a container restart gives a fresh
        writable layer, so a naive counter resets): ends `Succeeded` with
        `restartCount: 2` and `lastState.terminated.exitCode: 1`, which is
        rules 5 and 6 both firing without the skip. Leave
        `ttlSecondsAfterFinished` unset so the pod survives to be captured.
        **Capture the `Failed` sibling on the same trip** — same Job with
        `backoffLimit: 0` and a command that always exits 1, or a pod evicted
        by an ephemeral-storage limit.
      - **rule 5's CRITICAL band, and `&& !serving` with it** — no capture
        reaches `RESTARTS_CRITICAL`, so only the constants are asserted and
        both halves of the severity branch are unproven. Needs a container past
        ten restarts that is **serving** (WARN despite the count) and one past
        ten that is not; `broken-restarts` sits at 3 on purpose and must stay
        there, so this is a second pod, not a change to that one.
      - **rule 7's `!c.started` suppressor** — no fixture declares a
        `startupProbe`, so every container reports `started: true` and the
        state gate covers the same ground; deleting either leaves the other
        passing. One pod with `startupProbe: {exec: {command: ["false"]},
        failureThreshold: 60, periodSeconds: 5}` separates all three readings
        and is the only thing that proves the suppressor does what
        [D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
        says it does.
      - **rule 6's two fallback actions** — with a serving container now
        silent, only the log-line arm has a fixture. `(None, 126|127)` needs a
        container whose `command` names something not in the image and whose
        termination message is empty; `(None, _)` needs a non-zero exit with no
        message at all.
      - **rule 13's positive side** ([D72](NOTES.md#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13))
        — a `configMap` **volume** naming an object that does not exist wedges
        a scheduled pod in `ContainerCreating` with
        `PodReadyToStartContainers: True`, which is the residual branch. The
        **network** branch (`False`) needs the sandbox itself to fail and may
        not be reachable on kind without breaking the CNI cluster-wide; if it
        is not, say so in the box rather than leaving it looking untried.
      - **the two init/sidecar branches with no capture** — an init container
        in `scripts/healthy.yaml` that fails twice and then **succeeds** (the
        wait-for-dependency loop, which is what rules 5 and 6 must stay silent
        on once it has finished), and a **sidecar that is running but not
        ready**. Both are proven today on decoded copies rather than on
        committed JSON
      - **a serving container carrying an OOM kill in `lastState`** — no
        committed capture has one, because `oom.json`'s container is
        crashlooping, so both directions of rule 2's recency clause are proven
        only on a decoded copy. One pod that gets OOMKilled once and then stays
        up closes it on real JSON.
      - **rule 14's positive side, and it is the cheapest one here** — a pod
        with `schedulerName: does-not-exist`. Nothing picks it up, so no
        `PodScheduled` condition is ever written; no control-plane surgery, and
        `unbreak` only has to delete it
        ([NOTES § D74](NOTES.md#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13))
- [x] **`tui-designer`: the cordon-card round, and it has to close before this
      phase does.** `screens/alerts.md` § *the cordon card* and
      `screens/once.md` still argue from
      [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)'s
      falsified premise — quoting `Taint.timeAdded` as *"only written for
      NoExecute taints"* — while the code already draws `2 hours ago` off the
      capture. Three things to settle, and **the deadline is structural, not
      tidiness**: `rules::age`'s rungs were derived from these files and freeze
      with `rules.rs` at phase close, while its second caller `ui.rs` is Phase
      11, so a rung this round changes afterwards is a forward-only violation
      on the phase that can least afford one. (1) What the cordon card says now
      that it has a number, without turning into the accusation
      `alerts.md` deleted once — the field dates the *taint*, and anything
      rewriting `node.spec.taints` re-stamps it. (2) **The day rung**: `1 day
      ago` covers 24h01m through 47h59m, and `kubectl` deliberately does not
      truncate there (`HumanDuration` prints `30h`, `47h`, then `2d3h`), so
      k8rs is coarser than the command it teaches, in the band where "before or
      after yesterday's change window" is the question. (3) The age column's
      **width budget** — `alerts.md` right-aligns it with no stated maximum,
      and the widest string is 14 characters
      ([NOTES § D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
      Closed — [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13) · [D37](NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12) · [D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13) · [D83](NOTES.md#d83--the-hours-rung-runs-to-48-and-the-age-ladder-gets-one-home-2026-08-14) · [D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14) · [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
- [x] **Rule 1 must read how the previous run ended** — it draws *"Container
      keeps crashing"* over `exit 0` on a batch job that finished, and a
      **CRITICAL** *"keeps crashing"* whose own evidence line reads *"an
      ordinary shutdown and not an error"* over `exit 143`. Rule 6 has exempted
      both codes since it was written; rule 1 never looks, and `exit_meaning`
      has no row for `0`. Owed: the title true on a clean exit, the `0` row, an
      action that stops pointing at logs holding no answer, and both captured
      objects asserted — `exit0.json` and `sigterm.json` exist now, so the test
      can fail. **A box rather than a footnote, and a plan change recorded
      rather than applied silently**
      ([NOTES § D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)):
      it is rule *logic*, so the plain-language pass below is the wrong home for
      it, and the capture trip above is not unfinished for having found it.
      Closed — [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14) · [D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
- [ ] Plain-language pass over every string a user will read — the jargon test
      is "would someone in their first month understand this sentence?"
      **Three sentences are already known wrong and are owed to this box** — all
      found by an operator review that read the cards rather than the code, and
      all ruled out of scope where they were found so a fix would not widen
      someone else's box:
      **(a) `1 restarts`.** The card prints it, and `healthy-sidecar.json`'s
      `restartCount: 1` makes it real. The phrase is spelled twice — rule 1 and
      rule 5 — so the fix belongs in one shared place or the two rules start
      disagreeing.
      **(b) ~~rule 6's 137 title asserts one cause while its own action now
      lists three~~ — closed 2026-08-15 by the `137` box, not owed here.**
      `exit_meaning` said *"killed because it did not stop when it was asked
      to — a failing liveness probe, or a shutdown that hangs"* while the action
      under it named memory too, because
      [D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)
      showed a real OOM can arrive without the word — visibly out of step on one
      card. The translation now names the signal and refuses the cause, so the
      title claims nothing the action has to walk back
      ([NOTES § D93](NOTES.md#d93--an-exit-code-is-translated-once-for-every-role-and-137-is-read-from-the-object-rather-than-from-the-number-2026-08-15)).
      Left here struck through rather than deleted: it is the worked example of
      what this box is hunting, and the pass below should read it before
      starting.
      **(c) *"read the previous run's logs"* over a `$ kubectl describe pod`
      line.** The command a first-month reader needs is
      `kubectl logs <pod> -c <container> --previous`, and `--previous` is
      exactly the flag they will not know. One `kubectl_cmd` per finding is the
      constraint, so this is a wording problem or a shape problem, not a bug.
      **This sentence claimed `previous_logs` was already built, and it was not
      until Family B wrote it on 2026-08-16** — `logs()` emitted no `--previous`,
      and `tester` built a recommendation on the false half before anyone
      re-read the code. It exists now, beside [`logs`], and rule 6's general
      `Failed` arm carries it, so what is left here is the **call sites this box
      still owns** rather than the function
      ([D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14)).
      A `kubectl_cmd` is a change to what a card *teaches* (invariant 4), so each
      one is an operator-review question and not a wording tidy-up. **The rule
      this leaves behind is the one that cost the time: a box may say what the
      code should do and may not assert what it currently does** — that second
      sentence goes stale with no build to catch it.
      **One warning for whoever runs this box, so a red is not misread:** the
      rule tests pin prose, and one pin — `"finishing at all is the bug"` on rule
      5's sidecar arm — sits in exactly the register this pass exists to rewrite.
      `tester` expects it to break here, and that break is **cosmetic**: the
      assertion messages are written as requirements, so read the message, not
      the string. **Two more on that list**, found by `k8s-admin` mutating a
      faithful rewrite: `"without saying so"` and the `"exit code"` token in rule
      5's init-arm loop both red on a pure rephrasing. The other prose pins are
      short load-bearing clauses (`"of its own accord"`, `"does not allow health
      checks"`) where a red means the card's *claim* changed and is a real
      finding. **The distinction is worth keeping as you go:** a pin that names
      the thing making the claim true survives a rewrite; a pin on a token from
      the sentence does not, and every one of the latter in this file is listed
      above
- [x] **A container can have a status and no declaration, and the decode's own
      comment says it cannot** — `container_snapshots` explains its missing test
      with *"the API cannot produce the object: both container lists are
      immutable after create"*. Immutability is not the thing that breaks it: a
      node implementation that is not a kubelet is. k9s carries the field report
      ([#4145](https://github.com/derailed/k9s/issues/4145), open) — on Tencent
      TKE **virtual nodes** the provider injects a managed logging container into
      `status.containerStatuses` with no entry in `spec.containers`: two declared
      containers, three ready statuses, pod `Ready: True`. Virtual-kubelet,
      serverless nodes and sandboxed runtimes all sit in that gap. **What this
      code does with one today, asserted nowhere:** `declared` is `None`, so the
      container decodes with no requests and no limits, `Regular` from the main
      list and `Init` from the init list — the role whose cards tell the reader
      Kubernetes allows it no health checks, about a container whose manifest we
      never saw. Rule 9's "no limits" column and any count of a pod's containers
      inherit it too. **Done when** a fixture carries a status with no
      declaration, every rule's behaviour on it is asserted rather than
      inherited, and the comment states what is actually true. **The pairing
      itself is not in question** — it is by name, and that is exactly why the
      index-out-of-range panic k9s shipped in `initContainerStats` has no shape
      here ([PRIOR-ART § F3](PRIOR-ART.md#f3--container-semantics-moved-underneath-them)).
      Closed — **on a plant and not a fixture**, which is the honest reading of a
      shape no cluster this repository can build produces, and the requirement is
      asserted as *a card may not name what the spec never said* rather than as
      today's field values
      ([D112](NOTES.md#d112--laststateterminated-has-three-authors-and-the-file-was-reading-it-as-if-it-had-one-2026-08-16))
- [ ] Per rule: positive fixture test **and** negative (healthy) fixture test
- [ ] `cargo mutants --timeout 90` clean over `rules.rs` — a MISSED mutant is a
      rule change no test objected to, i.e. a hole in the diagnosis; it gets a
      test, not an excuse. **It proves the rules' logic and not the decode
      beneath them**: it mutates return values and match guards, never a struct
      literal's field assignment, and on the snapshot decode it found 1 of the
      32 holes a hand-written field-level sweep found. That sweep is the gate
      for the decode.
      **Run 2026-08-16: 519 mutants in 19 min — 271 caught, 229 unviable,
      **19 MISSED**, so the gate is red and this box stays open.** Where they
      are: `lasted` **10** (the humanised duration — `<` vs `<=`, `/` vs `%`, no
      unit boundary is asserted), `out_of_memory` **4** and
      `running_but_not_ready` **2** (both on the `NOT_READY_GRACE` comparison —
      the threshold is read either side but never *at* it), and one each in
      `short_of_pods`, `escalated_host_path` and `crash_looping`. Nineteen
      places where a test cannot fail; each gets a test
      ([NOTES § D41](NOTES.md#d41--cargo-mutants-cannot-see-the-defect-it-was-put-there-to-catch-2026-08-12))
- [ ] Temporary `main.rs` shell (~10 lines): load a fixture path from args,
      print findings. It cannot reach a cluster yet — `k8s.rs` is Phase 5, and
      that is where the v0.0.1 release therefore sits. **It strips control
      characters before printing**: the guard that makes this unnecessary is
      Phase 5's ingest strip, and this is the first code that shows a `Finding`
      — two phases earlier. A printer that displays API text with no guard is
      invariant 9 broken for the length of two phases, and "the fixtures are
      ours" is an argument about today's inputs, not about the code

- [x] **Split `rules_tests.rs` into one file per rule family — at phase close,
      after the last rule box and not before it.** 13 105 lines against the
      product file's 4 339, of which only 2 097 are code: **the test file is the
      one that actually grew**, and it is what every agent turn pages through.
      `src/rules_tests.rs` keeps `rules.rs`'s single
      `#[cfg(test)] #[path = …] mod tests;` declaration and becomes a few lines
      of `#[path = "rules_tests/<family>.rs"] mod <family>;`, one per marked
      region of `rules.rs` — snapshot · pod · node · workload · certificate.
      **Product code does not move**: invariant 11's eight flat files stand, and
      the seams are `rules.rs`'s own `// --- … START ---` markers, so the two
      trees keep the same shape. **The guards survive it as written** —
      `test-guard.py` and `write-guard.py` both `rglob` over `src`, so a
      subdirectory is walked without an edit; the count must still read
      177 declared / 177 listed, and that is the box's own proof. **Why not the
      product file too:** every defect this phase has cost days for was two
      rules reading one container and disagreeing, and the fix each time was one
      shared helper in one file — a module boundary makes a second copy easier
      to grow, which is the thing being defended against. **Why at close:** eight
      boxes are open against `rules.rs` and its tests; moving the tests under
      them lands every open box in a file that just moved. The user ruled on
      2026-08-15 ([NOTES § D91](NOTES.md#d91--the-tests-split-and-the-product-file-does-not-2026-08-15)).
      Closed — the *at phase close* timing was reversed by
      [D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
      and `4d73366` landed it: `src/rules_tests/` holds `snapshot.rs` `pod.rs`
      `node.rs` `workload.rs` `certificate.rs`, `rules_tests.rs` is 499 lines,
      and `test-guard.py` walks the subdirectory unedited — `207 declared, 207
      listed, 0 ignored — OK` (the box's 177 predates the tests added since). It
      stayed unchecked because the ruling that reversed it was written in a
      different file from the box it reversed
      ([D106](NOTES.md#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16))

**🔒 Security gate:** no finding text may quote an env value or a Secret —
findings name *fields*, not payloads. The certificate parser is fed malformed
and truncated PEM in a test and must return "no finding", never panic:
`rules.rs` returns no `Result`, so a panic there is a crash of the whole tool.
The snapshot decode copies API text through unchanged — control characters not
stripped, lengths not bounded — and that is deliberate: both belong to Phase
5's ingest gate, on the way *into* the decode. What this phase owes is the one
consumer that arrives before Phase 5 does: the temporary `main.rs` printer,
above.

**Done when:** all rule tests green against real fixtures; running the binary
on a fixture prints correct findings. *The product works here.*
**Frozen after:** `rules.rs` — **except the snapshot types and their decode,
which freeze at Phase 4 close.** Phase 4's reports are the contract's second
consumer and need fields no Phase 3 rule reads; they may add fields to those
types and nothing else in the file — not a rule, not `Finding`, not `ObjectId`,
not `analyze`
([NOTES § D42](NOTES.md#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).

## Phase 4 — Analysis reports

*Also read: [PRIOR-ART § F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended) — a report is where a number with no complete denominator gets printed.*

Goal: the cluster-wide answers no per-object rule can give. Pure functions
over a `ClusterSnapshot`, so this phase is as testable as Phase 3 and needs no
cluster either.

- [ ] **`scripts/check-docs.py` fails on a `### D##` heading with no line in
      NOTES § Decision index** — `tester`'s box, and it closes the one hole
      [D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
      left open. The index makes the file navigable without reading it, and
      today a *renamed* heading is caught by the anchor check while a heading
      **added** with no line at all is caught by nobody — which is the failure
      that degrades in silence, and the reason CLAUDE.md's rule needs a guard
      and not a promise. Landed in a later phase than the one that found it,
      per CLAUDE.md § What to do next. Done-when: a self-test plants a decision
      with no index line and the guard goes red on it
- [ ] **The fixture sanitization gate does not run over `reports/`, and that is
      where raw cluster output now lands** — `reports/` takes an agent's
      measurements into a *committed* file
      ([D108](NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)),
      which is the path `scripts/sanitize.jq` exists to guard for fixtures. Today
      the rule is a paragraph in [`reports/README.md`](reports/README.md) enforced
      by the PM reading the diff — a promise, and
      [D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12) is what
      promises are worth here. The guard is not the fixture sanitizer reused:
      that one rewrites JSON, this one reads prose and must refuse a token, a
      PEM block, a kubeconfig, an env value, an annotation payload, a node IP or
      a hostname wherever it appears in a markdown file. **Feed it every framing**
      (D31): the value whole, as a substring of a longer line, inside a fenced
      block, and base64-re-encoded. `tester`'s files, and it runs from
      `just check` with a `--self-test` like its neighbours. Done-when: each
      refused class is planted once, seen red, and the clean tree green before
      and after — and one **canary** proves the guard is reading the directory at
      all, so *found nothing* cannot print the same line as *nothing to find*
- [ ] **`reports/` has no retention rule and this repo's disease is append** —
      the directory grows one file per measurement forever, in the tree
      [D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)
      ruled must get smaller. Decide the bound before there is anything to bound:
      whether a report whose decision landed is deleted, kept, or reduced to the
      `D##` that cites it. PM's, at a phase close, not mid-phase

*Moved out of Phase 3: `tester`'s files, no rule touched ([D106](NOTES.md#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16))*

- [ ] **`just check` cannot see a comment's width, so the 100-column rule is a
      convention and not a gate** — `cargo fmt` reflows code and leaves comments
      alone, and two lines over 100 columns shipped into `rules.rs` on
      2026-08-15 and were caught by a reviewer counting characters rather than by
      the build. **Config is not the fix**: `rustfmt`'s `wrap_comments` and
      `error_on_line_overflow` are both nightly-only options, so a `rustfmt.toml`
      would be silently ignored on the pinned stable toolchain — which is worse
      than no gate, because it looks like one. A `scripts/` guard is, and it is
      `tester`'s: fail on any line in `src/*.rs` past 100 columns, run from
      `just check` with a `--self-test` like its neighbours. **It needs a decision
      it cannot make for itself**: the file already carries a deliberate markdown
      table whose rows are long on purpose, so either the table is rewrapped or
      the guard learns one narrow, named exemption — an unnamed allowance is how a
      guard becomes decoration
- [ ] **`certs-test.sh` says `(C1 warns)` and C1 no longer does** — display text
      in the green line, not an assertion, so nothing fails and that is exactly
      why it will survive. One word, `tester`'s file, and it goes with whichever
      of that agent's boxes runs next
- [ ] **`cluster.sh` accepts the one cluster name that defeats the fixture
      guard** — `scripts/sanitize.jq` refuses node names that do not
      `startswith("k8rs-")`, and
      [D92](NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)
      leans on that to make *a review cluster cannot produce a committed fixture*
      mechanical rather than promised. **`k8rs-review` sails straight through
      it**, and that is exactly what the first agent to raise a review cluster
      typed, against three committed files that said `review`
      ([NOTES § D94](NOTES.md#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)).
      **The second reviewer reached for the same wrong name and reported it as a
      repo defect** (2026-08-15) — every committed file still said `review`, and
      the string had reached it through **D94's own title**, which contains
      `k8rs-review` because the entry is about that mistake. The brief that told
      it to read D94 before naming the cluster is what handed it the name. Nothing
      to fix in the record; it is the sharpest possible argument that the refusal
      has to be mechanical, since the file warning about the name is itself a way
      of spreading it. **Third instance, and this one is the sharpest**: the
      reviewer of the D97 box quoted its own agent definition back as saying
      **`K8RS_CLUSTER=k8rs-review`, always** — `.claude/agents/k8s-admin.md:54`
      says `review`, in those words, and every other committed file agrees. It
      used `review` and filed the difference as a defect in the instructions.
      **Three agents, three misreadings, zero wrong strings in the repo**: the
      only place `k8rs-review` is written is D94's own title, and it keeps
      arriving in working memory from there. No wording change can fix a string
      nobody wrote; the anchor is the fix **Third instance the same day, and this one actually ran**:
      the review of the D96 ruling brought its cluster up as `k8rs-review`,
      against a brief that said `review`, so `k8rs-review-control-plane` existed
      on a machine and `sanitize.jq` would have waved its node name through.
      Nothing leaked — a review takes no captures — but the refusal has now been
      defeated in practice by the second agent to reach for it, which is the
      argument for anchoring rather than repeating. **One wording fix belongs with
      the anchor**: `CLAUDE.md` says the sanitizer refuses *a name*, and it
      refuses a **node-name prefix** — `review` is refused only because kind names
      the node `review-control-plane`.
      **The primary fix is in `sanitize.jq`, not in `cluster.sh`** — the reviewer
      never ran `cluster.sh`, it ran `kind create cluster` directly, because a
      review is one measurement and not a fixture trip, and the next one will skip
      that script for the same reason. So: **anchor `refuse_foreign_nodes` to the
      four node names the fixture cluster actually produces** — control plane and
      numbered workers, keeping the `.lan` suffix the identity rule already
      allows — instead of accepting the whole `k8rs-*` family. That refuses
      `k8rs-review-control-plane` on the only path that matters, whoever made the
      cluster and however. **Then** give `cluster.sh` its refusal too, as the
      early and loud one; it must not be the only one. Done-when: both refusals
      proven by running them, both directions, and the proof is a `just check`
      step rather than a paragraph. Watch the blast radius — the anchor must still
      accept every node name in the 50 committed fixtures, so run
      `fixture-audit.sh` before and after and show it unchanged. `tester`'s files

- [ ] `Report` shape: title · rows · the finding each row can jump to
- [ ] **Capacity** — per node: requests vs allocatable vs actual usage, plus
      **the workloads with no limits defined** (the old rule 9, which lives
      here now — it is a risk, not an outage). Two snapshot fields are added
      **here**, not in Phase 3, which is what D42's one-phase window is for:
      `status.allocatedResources` — what the kubelet actually reserved, which
      diverges from `spec` during an in-place pod resize on exactly the 1.33+
      clusters this project targets — and `spec.overhead`, the RuntimeClass
      charge the scheduler counts and a `spec`-only sum does not
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12))
*Moved out of Phase 3: each is a snapshot field before it is a rule, which is the one-phase window [D42](NOTES.md#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12) opened ([D106](NOTES.md#d106--phase-3s-twenty-three-open-boxes-are-two-families-six-foreign-boxes-and-one-already-done-2026-08-16))*

- [ ] **`spec.containers[].restartPolicyRules` is decodable now and still
      reaches no rule, so rule 15's stand-in is a proxy where the real signal is
      available** — the field arrives at `v1_34` and the pin is `v1_36`
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)),
      but **decodable is not present**: invariant 6 prunes the watch to the fields
      the snapshot types *name*, and none names this one. So this is a snapshot
      field, a prune line, a fixture that carries it, and only then the rule
      change — not *read a field*. What it buys is D97's own named residual gap:
      rule 15 fires on `restarts == 0`, and a container declaring a retry rule on
      its own exit code comes back under `Never`, which is KEP-5307's headline use
      case and this rule's headline false positive — measured on kind v1.36.1, a
      pod `Never` with one retry rule on `exit 3` sat in `CrashLoopBackOff` at
      five restarts. **The count cannot be dropped for the field**: a rule matched
      on exit code against the declared rules answers *will it retry this exit*,
      which is what the card claims, but the window is one exit wide and the
      object to prove it on has to be captured — `scripts/broken.yaml` keeps
      `restartPolicyRules` **off** the rule-15 fixture on purpose, so this is a
      new object, not a variant of one already there. **It rides the capture trip
      three boxes up** rather than opening its own: that box already owes a
      cluster for three objects, and one trip is the whole argument for grouping
      them.
      **Placed here rather than at the front of the phase, and that is a PM call
      made in writing.** These three D99 follow-ups were written directly under
      the box that spawned them, which put an *enhancement* ahead of every
      operator-review finding above — a permanent WARN card on a healthy pod, a
      run Kubernetes lost that k8rs says nothing at all about. Weighed against
      those, the defect this box closes is **one backoff window wide**, on a beta
      feature, and costs a snapshot field, a prune line, a capture and a rule
      change. The severe ones go first; the ordering, not the value, is what
      changed
- [ ] **`terminatingReplicas` is decodable now, and a pod on its way out is
      currently counted as a pod that is missing** — added to both
      `DeploymentStatus` and `ReplicaSetStatus` after 1.32
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)).
      The workload rules read ready-versus-desired and cannot today tell a
      rollout draining old pods from a Deployment that genuinely cannot fill its
      replicas — the shape every operator sees during a normal deploy, which is
      the false-positive class that makes a tool get muted. Same shape as the box
      above: snapshot field, prune line, fixture, then the rule. Check first
      whether `explains_a_shortfall` is where it belongs rather than a rule of its
      own
- [ ] **In-place resize makes *what a container asks for* and *what it has* two
      different numbers, and every resource rule reads only the first** —
      `podStatus.resources` and `podStatus.allocatedResources` arrive after 1.32
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)),
      beside the `.status.resize` string this file already records as an
      unreachable branch at the old pin. Rules 8/9 and the Capacity report compare
      `spec.containers[].resources` against the node, which is the *request*, not
      the allocation, and after a resize those disagree. **Decide the scope before
      writing anything**: this may be one snapshot field and a fallback, or it may
      be out of scope entirely under the invariant-13 guard — *would someone who
      runs clusters meet this in a normal week* is a genuine question for a
      feature that went beta in 1.33, and the honest answer may be no until it is
      on by default. Answer it in NOTES either way; do not leave it as a silent
      omission, which is exactly what the pin was

- [ ] **Drain safety** — for each node, what a drain would do and what would
      block it. A PDB whose `minAvailable` equals the replica count means the
      drain never finishes; say so before, not 40 minutes in
- [ ] **Waste** — **Services whose selector matches no pod first** (the 503
      nobody can explain; it stays here rather than in Alerts because
      promoting it would cost a permanent Services + EndpointSlices watch, and
      the watch budget is why k8rs is lighter than k9s), then unbound/unused
      PVCs, Evicted and Completed pod pileups, ReplicaSets parked at 0
- [ ] **`tui-designer` answers where the restart row lives, before it is
      written** —
      [D101](NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)
      hands the screen two constraints and settles neither. Waste is the only pane
      carrying rows of this kind today, and both its headings deny what the row
      says: *Things that cost you something for nothing*, with *Worth knowing (not
      broken)* under it, over a container that keeps dying (invariant 14). And
      Waste is the one pane the sidebar never badges, so the compensation for the
      Alerts silence lands where nothing sends the reader. Re-title, re-home or
      badge it — in `screens/analysis.md`, with the wording of the two columns,
      which says **since this pod started** because a rollout resets the count
- [ ] **Restarts, as a counted row and never an alert card** — the hole D101
      left visible: a container that is fine right now and keeps dying on a long
      cycle draws nothing from rules 1, 2, 5 or 6 between its restarts. **One row
      per container, both numbers that container's own**: `restarts`, and how long
      the run it is in has lasted — from `state.running.started_at` and **not**
      from `last_terminated.finished_at`, which the two synthesized `137`s leave
      `null` on the gang-restart shape (D100's measurement). Sorted worst first,
      **never divided**
      ([PRIOR-ART § F2](PRIOR-ART.md#f2--a-number-that-cannot-be-defended)); no
      sum across a workload's pods, whose count and age would live in two
      different domains, and no grouping on `pod.owner`, which is the ReplicaSet
      until Phase 5 and loses the count on every deploy. **The row jumps to the
      container's pod and not to a finding** — there is no finding, which is the
      whole reason the row exists, and the `Report` shape box above has to allow
      that (the unbound PVC and the parked ReplicaSet are the same case).
      **It may not name how the last run ended** — `ending` and `exit_meaning` are private to `rules.rs`,
      frozen at Phase 3 close, and re-spelling the translation here is the defect
      [D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)
      exists to prevent. That one is a convention with no gate behind it —
      `Terminated`'s `reason` and `exit_code` are `pub`, so a raw `exit 137` in a
      row is reachable and is wrong
- [ ] **Posture** rows: the plain read-only hostPath mounts that no longer
      appear in Alerts — CNI/CSI/node agents are supposed to have them, so
      they are a list to review, not an alarm to answer. Computed **here**,
      not in `rules.rs`: they read pod fields but produce a whole-cluster list,
      and `rules.rs` is frozen by now
      ([NOTES § D14](NOTES.md#d14--three-plan-corrections))
- [ ] **Versions** — control plane vs kubelet vs client skew (this is where N4
      is shown), and which nodes fall outside the supported window
- [ ] **Certificates** — the C-series as a dated table, soonest first. C1
      (kubeconfig client cert) is shown here, and the sidebar badge — `30d` in
      the sketch — is its alerting mechanism
- [ ] Positive and negative fixture tests per report, same discipline as rules
- [ ] `cargo mutants --timeout 90` clean over `analysis.rs` — same gate
      `rules.rs` gets in Phase 3. A report that quietly stops flagging looks
      identical to a report with nothing to flag
      ([NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12))

**Done when:** every report is correct against the cluster-wide fixture, and
the temporary main can print any of them.
**Frozen after:** `analysis.rs`.

## Phase 5 — Live reads · **milestone M1.5**

*Also read, before the first box: [PRIOR-ART § A](PRIOR-ART.md#a-scale--the-largest-single-complaint-class) (scale, and the initial list), [§ B](PRIOR-ART.md#b-connecting--kubeconfig-auth-and-the-network) (kubeconfig, expiry, reconnect, RBAC) and [§ C](PRIOR-ART.md#c-errors-that-lie) (errors that lie). Seven of the twelve gaps that review opened land in this phase, and [§ L2](PRIOR-ART.md#l-two-observations-about-the-tracker-itself) says why: the loudest threads in k9s's tracker are all regressions in the startup path.*

Goal: the same findings and reports, from a living cluster — and the first
public release.

- [ ] `k8s.rs`: kube-rs `watcher` over Pods, Nodes and
      Deployments/StatefulSets/DaemonSets + prune (drop `managedFields`) →
      snapshot store. **The prune line is "the fields the snapshot types in
      `rules.rs` name, across metadata, spec *and* status" — "metadata + status
      only" was never true of this design** and this box said it until
      2026-08-13
      ([NOTES § D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
      **And no snapshot is published until every initial LIST has landed.** A
      rule cannot tell a partial list from a small cluster — invariant 5 leaves
      it no way to ask — so a snapshot emitted mid-bootstrap makes rule 10 say
      "none of the 3 nodes have that label" on a 200-node cluster, and makes
      N2's count and N5's sum confidently wrong. `namespace_scope` covers the
      *deliberately* partial pod list and nothing covers a *transient* one;
      this box is where that hole closes, because nowhere above it can
      ([NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)).
      **One field the prune line will drop if it is read literally, and two
      shipped cards depend on it:** `spec.initContainers[].restartPolicy`.
      `ContainerSnapshot` **named** no `restart_policy` until 2026-08-15 — the
      field was read during the decode, to tell a native sidecar from a plain
      init container — so "the fields the snapshot **types** name" did not cover
      it. **That half is closed and the other half opened in the same change**
      ([D97](NOTES.md#d97--a-container-that-cannot-come-back-gets-rule-15-and-a-restart-count-stands-in-for-a-field-the-pinned-types-cannot-see-2026-08-15)):
      the container's own policy is now a snapshot field, and the field it falls
      back to — **`spec.restartPolicy`, pod level** — is consumed at decode and
      named by **no snapshot type at all**, because `ContainerSnapshot` carries
      the *effective* value rather than the two it was computed from. A prune
      written from the structs keeps the container field and drops the pod one,
      and rule 15 then goes silent on every pod that does not override per
      container — which is nearly all of them, the committed fixture included.
      The doc comment on the field says this at length; a prune is not written
      from doc comments. Drop it and
      every Istio/Linkerd sidecar decodes as `Init`, where rules 1 and 5 both
      tell its owner that *"Kubernetes does not allow health checks on this kind
      of container"* — about a container whose manifest has a liveness probe in
      it. Found by the operator review of the rule 5 box, which is what made the
      claim load-bearing; this is D69's shape a second time, caught before the
      code instead of after
      ([NOTES § D88](NOTES.md#d88--an-exit-code-names-an-ending-never-an-agent-and-the-boundary-for-folding-a-found-defect-in-2026-08-14))
- [ ] **How the initial LIST arrives is a decision, not a default** — the box
      above forbids publishing a snapshot until every initial LIST has landed,
      which makes the shape of that LIST load-bearing. An unpaginated
      `LIST pods -A` on a 10 000-pod cluster is one response the API server has
      to build whole, and it is the single call most likely to time out, while
      `REQUIREMENTS.md` budgets first paint at under a second. Decide
      `limit`/`continue` and the page size, read what kube-rs's `watcher` does by
      default rather than assuming, and write the number down. k9s took six years
      to paginate one call and carried "slow on a large cluster" the whole time
      ([#663](https://github.com/derailed/k9s/issues/663) 2020 →
      [#3987](https://github.com/derailed/k9s/pull/3987) 2026 ·
      [PRIOR-ART § A2](PRIOR-ART.md#a2--the-initial-list-must-be-paginated-and-the-page-size-is-a-decision))
- [ ] **Find out whether kube-rs rate-limits us, and if it does, put it on
      screen** — client-go ships a client-side QPS limiter, so for years a k9s
      user reporting "slow" was partly reporting a queue inside their own binary
      with nothing to see it by; the repair was a default raised from 5 to 50
      ([#3988](https://github.com/derailed/k9s/pull/3988)). Read kube-rs's own
      docs in `tmp/` for this — "it probably does nothing" is the assumption that
      makes it invisible. Whatever it turns out to be: a documented number, and,
      if requests are ever queued, a state the header can show rather than a
      silent default
      ([PRIOR-ART § A3](PRIOR-ART.md#a3--client-side-throttling-is-invisible-and-that-is-the-bug))
- [ ] **Name the oldest API server k8rs supports, enforce it at connect, and put
      a deadline on the first watch sync** — `Cargo.toml` pins `k8s-openapi` to
      `v1_32` and calls it *"the oldest supported version"*, which is a statement
      about the types we compile against and is enforced nowhere at runtime.
      *(The pin is the **newest** offered since 2026-08-15,
      [D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15),
      which makes this box more pressing rather than less: no line anywhere now
      even claims a floor.)*
      **And it now owes the other end too, which is the half D99 did not name.**
      The reversal moved the silent-drop failure out of this repo — where the new
      `fixture-audit.sh` guard catches it — and onto **the user's machine**: a
      cluster *newer* than our pinned types drops its added fields exactly as the
      old pin dropped 1.36's, and that guard structurally cannot see it, because
      it only ever compares the pin against a fixture stamp taken from the pinned
      image. Every k8rs user on 1.37 the day 1.37 ships is in that state. So the
      connect-time check has two sentences, not one: *this cluster is older than
      anything we support* and *this cluster is newer than the types this build
      was compiled against, so some of what it reports may not reach the
      screen* — the second being the one D99 calls the unaffordable failure.
      Found by the operator review of the D99 box
      Nothing stops a stranger pointing v0.0.1 at a v1.24 cluster, and nothing
      would tell them that is what went wrong. The shape to design against is
      k9s's [#4044](https://github.com/derailed/k9s/issues/4044): client-go's
      WatchList default asks for `sendInitialEvents`, an API server older than
      v1.27 **ignores the parameter instead of rejecting it**, the promised
      `BOOKMARK` carrying `initial-events-end` never arrives, and the informer
      waits for it forever — no error, no log line, a spinner that never stops
      while `kubectl get` on the same context returns instantly. Because the
      server returned no error, the documented fallback to LIST+WATCH never
      fires. **This is the way a watch-first design hangs that a polling one
      cannot**, and it is worth knowing before switching kube-rs's own streaming
      list strategy on for speed. **Done when** the version floor is written
      down, the connect path says so in plain language for a cluster below it,
      and a first sync that does not complete becomes a state on screen instead
      of a wait
      ([PRIOR-ART § A7](PRIOR-ART.md#a7--the-watchs-own-initial-list-strategy-can-hang-forever))
- [ ] **Owner name resolution**: a pod's `ownerReferences` names its
      *ReplicaSet*, and the group heading has to read `web`, not
      `web-7d4f5c6b8`. Fetch the ReplicaSet on demand, cache by UID, never
      watch it — and never strip the hash with a string heuristic, which is
      the kind of guess that lies. The same cached object supplies W1's
      `ReplicaFailure` message
- [ ] `kube::discovery`: enumerate every kind the cluster serves, CRDs
      included. This is what the sidebar is built from — never a hard-coded list
- [ ] Server-side `Table` fetch for browser kinds — the columns come from the
      API server, not from us. Hand-built through `Client::request` (kube-rs
      has no `Table` type), Accept header
      `application/json;as=Table;g=meta.k8s.io;v=v1,application/json`, and the
      `406`-from-an-aggregated-API case handled by falling back to the plain
      object list
- [ ] Watch lifecycle: browser views watch `watch_metadata` (tiny) to learn
      *that* something changed and re-fetch the Table, debounced — Table
      cannot be watched. Only the Pod and Node watches stay permanent; a
      closed view drops its stream
- [ ] Capability probe from the same discovery call: `metrics.k8s.io`,
      `policy`, `cert-manager.io`, `monitoring.coreos.com`, Istio/Linkerd/
      Cilium. Absent capability = the feature says why it is off, never hides
- [ ] Reconnect/backoff surfaced as a state the UI can show — **and the tool
      never exits because the cluster went away.** A connectivity failure is a
      banner, retried for as long as the user leaves it open; there is no retry
      budget after which k8rs quits. k9s had the opposite of each half
      ([#3922](https://github.com/derailed/k9s/issues/3922)): its updater ran the
      first refresh *outside* the retry loop, so the first failure killed the
      reconnector permanently · it called `BailOut` after five retries, so a VPN
      blip over lunch meant the tool was gone on return · each failed check held
      the 120-second call timeout, making recovery slower than the outage. The
      lesson under all three: k9s survived disconnects during *active* use, where
      navigation restarts the watches by accident, and only the **idle** path was
      broken — the path nobody tests. So this box is proven idle: leave it
      running against kind, `docker stop` the node, wait past any timeout, start
      it again, and the findings must come back on their own
      ([PRIOR-ART § B3](PRIOR-ART.md#b3--reconnect-logic-dies-quietly))
- [ ] **Connecting is a function, not a step in `main`** — `connect(context)`
      builds the client, runs discovery and the capability probe and starts the
      watches, and can be called again after everything from the previous
      context has been dropped. The `X` switcher in Phase 11 is that call;
      writing it as one-shot startup code here would mean reaching back into a
      frozen `k8s.rs` later ([NOTES § D16](NOTES.md#d16--the-context-switcher))
- [ ] 403 vs 401 vs no-connection distinguished (**three** variants, not two).
      `401` is a credential-plugin token that expired mid-session — the normal
      case on EKS/GKE/AKS — and it names the renewal command from the user's
      own kubeconfig `exec` block rather than guessing a cloud
      ([NOTES § D19](NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program))
- [ ] **A generic message may never stand in for an error we were handed** — the
      three variants above are worth little without it. Whatever failed, the
      screen names *what* failed and *why*; a fallback string is printed only for
      the case it actually describes. k9s tells these errors apart internally and
      still shows `Ruroh? 'v1/pods' command not found` when a credential expires,
      because a generic handler between the call and the screen swallowed the
      typed error — its own log had the truth three lines earlier
      ([#3730](https://github.com/derailed/k9s/issues/3730),
      [#3132](https://github.com/derailed/k9s/issues/3132)). Invariant 14 governs
      the *wording* of what a user reads; this governs where it is allowed to
      come from. **It updates `docs/architecture.md § Error handling`**, which
      today covers the three startup errors and not the general rule
      ([PRIOR-ART § C1](PRIOR-ART.md#c1--the-generic-handler-ate-the-real-error))
- [ ] **Measure resident memory against 10 000 pods** (kind + a generator)
      **plus the three workload watches**, and write the number down. Pruning `managedFields` is agreed; whether the
      pruned store actually fits is unmeasured, and an unmeasured number is not
      a design ([NOTES § D25](NOTES.md#d25--what-this-review-did-not-decide))
- [ ] Startup errors (no kubeconfig / bad context) → stderr + non-zero exit
- [ ] **The six kubeconfig shapes, each with a fixture** — the largest class in
      k9s's tracker is not the cluster, it is the file that describes it, and
      every shape below is a separate closed issue there. **No file** · **a file
      with no current context** — a *panic* in
      [#2465](https://github.com/derailed/k9s/issues/2465) (33 comments) and the
      same cause wearing a different symptom four years later in
      [#2651](https://github.com/derailed/k9s/issues/2651) · **`KUBECONFIG`
      holding several paths** ([#829](https://github.com/derailed/k9s/issues/829),
      and token refresh across them,
      [#620](https://github.com/derailed/k9s/issues/620)) · **a context whose
      name contains a space** ([#3815](https://github.com/derailed/k9s/issues/3815),
      still open) · **a context that names its own namespace**, which is the
      namespace k8rs must then start in — a regression k9s shipped twice
      ([#1397](https://github.com/derailed/k9s/issues/1397),
      [#1444](https://github.com/derailed/k9s/issues/1444)) · **a context whose
      `exec` credential plugin is missing or fails**, which is
      [D19](NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)
      already. They sit beside the box above because they *are* startup errors —
      the ones a stranger meets before they ever see a finding
      ([PRIOR-ART § B1](PRIOR-ART.md#b1--kubeconfig-is-harder-than-it-looks))
- [ ] **The clock-skew line in the header, which D55 declared binding on later
      boxes and nobody owned.** *"Your computer's clock is 11 minutes behind
      the cluster — the times on this screen are wrong"*, in plain language,
      from the API server's own `Date` response header — the only honest source
      for the half no object timestamp can reveal, and a `k8s.rs` question,
      which is why the box is here. It is the other half of the bound
      `rules::age` now carries: past five minutes of skew `age` produces no
      number at all rather than a plausible one, and a screen that goes blank
      without saying why is a worse bug than the one it replaced. **It spans
      two ownership rows, so it is two turns, not one:** `tui-designer` first —
      the box needs a state in `screens/states.md` **and** in `screens/once.md`,
      which has no header to put one in — then `dev-core` wires it. That is
      just step 2 of the cycle, named here because a box whose files have two
      owners is where "someone will do it" means nobody does
      ([NOTES § D55](NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12) ·
      [§ D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13))
- [ ] Certificate rules that need the wire: C2 (API server serving cert) and
      C3 (pending CSRs)
- [ ] **The typed lists `analysis.rs` needs**, fetched on demand when a report
      is opened: Deployments, ReplicaSets, Services, EndpointSlices, PVCs,
      PDBs. These are *not* the browser's `Table` path — a report needs
      `minAvailable` and `.spec.selector` as fields, and Table gives strings
      for display. Phase 3 defined `ClusterSnapshot` and Phase 4 extended it
      ([NOTES § D42](NOTES.md#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12));
      this is the step that fills it, and it has to happen before `k8s.rs`
      freezes
- [ ] **metrics-server polling**, the one thing that cannot be watched: 30s+,
      only for what is on screen, and only when the capability probe found
      `metrics.k8s.io`. Without it the Capacity report's usage column has no
      source — and it says so rather than showing a blank
- [ ] Namespace scoping: `--namespace/-n`, and a 403 on the cluster-wide LIST
      falls back to the context's namespace (then `default`), with the header
      stating which scope is in effect and why. A namespace-scoped user must
      get a working tool, not an empty one
      ([NOTES § D5](NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter))
- [ ] Wire into the same print loop; verify against kind while breaking pods
- [ ] The **read-only `ClusterRole`** written out in `docs/security.md`, and
      verified by running v0.0.1 against kind under exactly that role and
      nothing more. It ships with the first release because it is what a
      stranger needs in order to run the thing at all; the admin role follows
      in Phase 7 with the writes it exists for
- [ ] **Say in the docs where `--once` output ends up.** Findings carry
      controller messages verbatim, and a validating webhook can echo the
      object it rejected — env values included — into one. On the terminal
      that is no worse than `kubectl describe`; redirected into a CI log or
      pasted into a ticket it reaches a wider audience. One documented line,
      not a blanked field
      ([NOTES § D37](NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12))
- [ ] **Release v0.0.1 to crates.io** — `k8rs --once`, exactly as
      [screens/once.md](screens/once.md) draws it: findings on stdout, the
      commands and errors on stderr, `● ▲ ○` carrying severity without colour,
      colour only on a tty with `NO_COLOR` unset, exit `0` when it ran and `2`
      when it could not. No binary matrix and no screenshot; `cargo install` is the whole
      distribution at this stage. Ships the one thing nothing else does, months
      before the TUI, while the rules are still cheap to change
      ([NOTES § D10](NOTES.md#d10--m1-ships-publicly-as-v001))

**🔒 Security gate:** TLS verification is never disabled by us; if the
kubeconfig sets `insecure-skip-tls-verify` it is honoured *and surfaced*, not
swallowed. The token never leaves the kube client — `Debug` is wrapped on
anything that could hold it. Control characters are stripped at ingest, so no
downstream code has to remember — **and the field list is not "names and
messages"**: `metadata.finalizers` reaches `evidence` verbatim through rule 12
and is settable by anyone with `patch` on pods, which is the shape a generic
sentence lets an implementer miss. Field sizes are bounded: a 50MB annotation
must not be stored whole, **and neither must a container's waiting message** —
rules 3 and 4 put the kubelet's whole `state.waiting.message` on the card, and
nothing below this phase bounds it
([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).

**Done when:** watching kind live shows findings appear/disappear as pods
break and heal; discovery lists the CRDs you installed; unplugging the network
shows the reconnect state; and `cargo install k8rs` on a clean machine gives a
stranger a working `k8rs --once`.
**Frozen after:** nothing yet — `k8s.rs` stays the top layer through Phase 6,
which adds the remaining read paths to the same file. It freezes there.

## Phase 6 — Logs and read-only detail

*Also read: [PRIOR-ART § E](PRIOR-ART.md#e-logs) (a stream ends for many reasons; the log view is where CPU dies), [§ D1](PRIOR-ART.md#d1--cluster-data-as-markup) (sanitising for the screen is not emitting for a consumer) and [§ H](PRIOR-ART.md#h-secrets--the-pressure-is-constant-and-it-is-all-in-one-direction) (seven years of pressure to decode secrets by default).*

Goal: the whole beginner debugging loop, still headless, still read-only.

- [ ] `l` logs: fetch and follow, container picker, `--previous` for a
      crashed container — the single most-typed kubectl command there is
- [ ] **Per-object events fetch** (`involvedObject` field selector, this object
      only — never the global Events watch). It feeds two consumers: `describe`
      and the events *tab* of Phase 11. Listing it once, here, is the point:
      `k8s.rs` freezes at the end of this phase, and the tab would otherwise
      have to reach back into it
- [ ] `d` describe: object plus those events, assembled from what we now have
- [ ] `y` YAML view, with Secret values hidden behind an explicit reveal
- [ ] Control-character stripping on every free-text field from the API
- [ ] **The log buffer's bound is a number, and a dropped line is counted out
      loud** — this phase's gate below says "bounded buffer, no unbounded growth"
      and names no figure, which is how a bound stays unbuilt. Pick the line
      count, drop the oldest past it, and when lines are dropped because the
      stream outruns the reader, **say how many on screen**. k9s widened its log
      channel and added a drop counter in the same change
      ([#3978](https://github.com/derailed/k9s/pull/3978)): silently losing log
      lines in a debugging tool is worse than showing fewer of them. What an
      unmeasured process costs is on record without a named cause — a k9s left
      running grew to 21.5 GB resident over eight days and invoked the node's OOM
      killer, which then killed the pods it was there to watch
      ([#871](https://github.com/derailed/k9s/issues/871) ·
      [PRIOR-ART § A6](PRIOR-ART.md#a6--unbounded-memory-in-the-field-for-8-days))
- [ ] **Sanitising for the screen and emitting for a consumer are two different
      functions** — the box above strips control characters on the way in, which
      is half of it. Whatever the *display* does to a string has to be undone
      before that text leaves k8rs through `y`, a copy, a saved file or `--once`.
      k9s learned this across four follow-up PRs to one bug: a secret containing
      `match[]`, copied out, produced different bytes than the secret
      ([#3051](https://github.com/derailed/k9s/issues/3051)); the fix left escape
      residue in input fields ([#3885](https://github.com/derailed/k9s/issues/3885));
      the regex handled one occurrence per line instead of all of them
      ([#4043](https://github.com/derailed/k9s/pull/4043)); and the save/copy path
      needed a de-escape of its own
      ([#3945](https://github.com/derailed/k9s/pull/3945)). ratatui has no markup
      language, so that *mechanism* cannot repeat here — the shape can, the
      moment a renderer clips, wraps or marks a string and something downstream
      emits the marked copy, which is the open half of
      [#4123](https://github.com/derailed/k9s/issues/4123) too (wrapped log lines
      that no longer parse as JSON when copied). **Done when** a fixture line
      carrying control characters, brackets and a wrap-width boundary comes out
      of every emit path with exactly one transformation on it — the documented
      ingest strip — and nothing the renderer added
      ([PRIOR-ART § D1](PRIOR-ART.md#d1--cluster-data-as-markup))

**🔒 Security gate:** log streams are attacker-controlled text — bounded
buffer, control characters stripped, no unbounded growth. Secret values are
hidden in the YAML view by default and the reveal is a separate action.
`serde_json`'s `preserve_order` is on, or the YAML we teach with is
alphabetised and wrong.

**Done when:** the temporary main can print logs, describe output, events and
YAML for any object it can see.
**Frozen after:** `k8s.rs` — every read path in the product now exists. Check
it against all four consumers before closing the phase: the Alerts rules, the
Analysis reports, the browser, and the detail tabs. A read path missed here is
a frozen-file problem in Phase 11, not a small addition.

## Phase 7 — Operations · **milestone M2**

*Also read: [PRIOR-ART § G](PRIOR-ART.md#g-destructive-actions) — three classes k8rs is immune to by design, and each is the ending of a thread that took k9s years. They are the invariants this phase must not negotiate down.*

Goal: every write works and is safe, **before a single key is bound to one**.
This is the phase where the reversal actually happens, and it is deliberately
placed low in the pyramid so the dangerous code is proven headlessly.

- [ ] `ops.rs` with the single `#![allow(clippy::disallowed_methods)]`; CI's
      containment check now expects exactly this file
- [ ] The mutation contract, one shared function so no operation can skip a
      step: *consequence text → dry-run → confirm callback → call → audit*
- [ ] Server-side `dryRun=All` wherever supported; a rejected dry-run aborts
      and surfaces the API server's own message
- [ ] **A dry-run does not reject an unknown field, so the mutation contract
      needs `fieldValidation=Strict` and a place to put the warning** — measured
      2026-08-15 on kind v1.36.1
      ([D99](NOTES.md#d99--the-pin-follows-the-newest-types-and-the-old-rule-was-self-violating-from-the-first-capture-2026-08-15)):
      a merge patch carrying a field the cluster does not have answers
      **`200 OK`** under `dryRun=All`, with the objection only in a `Warning: 299`
      header that kube's `Api` methods do not surface. Same for the `scale`
      subresource and the eviction POST. So the box above it is not the guard it
      reads as: the dry-run passes, the real call passes, and the **audit log
      records a successful mutation that changed nothing** — invariant 4's
      *neither record may lie*, broken by the server rather than by us, and
      exactly the shape a user on a cluster older than our pinned types would
      meet. `PatchParams::default()` sends no `fieldValidation`; `Strict` turns
      the 200 into a `422` (a `400` for eviction). **Two things to decide, not
      one**: whether `Strict` goes on every write or only where a rejection is
      recoverable — it is a behaviour change on a cluster that today accepts the
      call — and where the warning goes when it is *not* strict, since a header
      nobody renders is the same silence one layer up. **Sanitize before
      rendering either**: the `422` body echoes the whole object, every label,
      annotation and `managedFields` entry, and an apiserver error written
      verbatim into the audit log puts there what `scripts/sanitize.jq` exists to
      strip out of fixtures
- [ ] The headless driver: the temporary main takes a subcommand
      (`k8rs ops scale deploy/web 3 -n payments`) so every operation is
      runnable — and scriptable in `just e2e` — before any key exists. This is
      what makes "prove it before binding it to a key" an actual step rather
      than an intention. **Scaffolding, not surface:** it lives in the
      temporary main and disappears when the console lands, so it does not trip
      the "a subcommand means it is time for clap" threshold
      ([NOTES § D14](NOTES.md#d14--three-plan-corrections))
- [ ] `scale` — via the **scale subresource** (`get_scale` / `patch_scale`),
      not a full-object patch
- [ ] `restart` — `Api::restart(name)`, which kube-rs already implements for
      workloads. For a bare pod there is no restart: it is a *delete*, and the
      consequence text must say so in plain words
- [ ] `delete` — requires the typed object name
- [ ] Every call sends the resourceVersion that was read; a `409` offers a
      re-read, never a blind overwrite (the case `edit` will lean on in v0.4 —
      the mechanism is built and tested now, while it is cheap)
- [ ] Audit log: `~/.local/state/k8rs/audit.log`, mode 0600, append-only,
      recording refusals and failures as well as successes, and recording
      **both** the equivalent kubectl line and the real API call (verb, path,
      resourceVersion, dry-run verdict) — the kubectl line is a teaching aid,
      not what ran ([NOTES § D8](NOTES.md#d8--invariant-4-was-not-literally-true))
- [ ] **`may_i(...)`** — `SelfSubjectRulesReview` per namespace, plus a
      `SelfSubjectAccessReview` for the two cluster-scoped operations. It lives
      in `ops.rs` although it mutates nothing, because it is performed with
      `create` and widening the allowlist would turn a mechanical guard into a
      judgement call
      ([NOTES § D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards))
- [ ] **The audit line is written and flushed before the call**, the result
      appended after. If the log cannot be written, the mutation is refused —
      [invariant 4](CLAUDE.md) leaves no other answer. If it cannot be opened
      at startup, k8rs runs read-only and says so
      ([NOTES § D21](NOTES.md#d21--if-the-write-cannot-be-audited-the-write-does-not-happen))
- [ ] **In-flight is part of the contract**, not a UI detail: an operation
      reports started → result, so exactly one mutation can be outstanding and
      `q`/`X` can be refused while one is
      ([NOTES § D20](NOTES.md#d20--a-call-that-takes-time-is-a-state-and-there-was-none))
- [ ] The command log feed — every command as the user would have typed it
      (the UI panel comes later; the data starts here)
- [ ] `--read-only`: `ops.rs` unreachable, not merely unbound
- [ ] Verified against kind: scale it, watch the replicas change through the
      watch stream, read the audit line back. Then the same for each operation
- [ ] e2e job under `--read-only` that fails if any mutating request reaches
      the API server

**🔒 Security gate — the heaviest one in the plan:** object names are
sanitised before they touch a path or a URL segment — `../` must not escape,
and a pod named `; rm -rf ~` must be boring everywhere it appears. Audit log
mode 0600, append-only, recording refusals and failures too, and recording the
real API call alongside the kubectl line. The command log is display text —
k8rs never executes it. `--read-only` verified by the e2e job, which fails if
any mutating request reaches the API server. *(The `$EDITOR` and temp-file
items of this gate move to v0.4 with `edit`; they are written out in the
Later section so they cannot be forgotten.)*

**Done when:** every operation has been run against kind from the temporary
main, including a rejected dry-run and a 409 conflict, and the audit log
matches what happened.
**Frozen after:** `ops.rs`.

## Phase 8 — TUI spike (throwaway)

Goal: learn the ratatui event loop without touching product files.

- [ ] Start from the ratatui async template in `examples/`
- [ ] Dumb list: live updates from the watch, `q` quits, resize works,
      terminal restored on panic
- [ ] Spike the hard interaction: a modal dialog over a list, keyboard focus
      and all. Terminal handover (suspend, give the terminal away, take it
      back) is *not* spiked here — nothing in v0.1 needs it now that `edit`
      moved to v0.4 and `exec` to v0.3. It gets its own spike then

**Done when:** the spike runs and the loop is understood. Code stays in
`examples/`, never merged into `src/`.

## Phase 9 — Theme

*Also read: [PRIOR-ART § D2](PRIOR-ART.md#d2--do-not-fight-the-users-terminal) (the user's own 16 colours win; bold is a per-emulator setting) and [§ K](PRIOR-ART.md#k-accessibility) (colour is never the only carrier of meaning — every state, not just severity).*

- [ ] `theme.rs`: 10 Catppuccin Mocha constants + `COLORTERM` check with a
      16-color fallback
- [ ] Severity symbols `● ▲ ○` — never colour alone, **and the same rule for
      every other meaning on the screen.** Selection, focus, the `changing…`
      state, the disconnected banner and the `--read-only` marker each carry a
      symbol, reverse video or a word beside their colour. It belongs here rather
      than in each screen because `theme.rs` is the single point of change, and
      [screens/alerts.md](screens/alerts.md) is today the only file that promises
      it — about findings only. k9s marks its selected column with a foreground
      colour and nothing else, invisible to a reader with a colour vision
      deficiency or on a skin where that colour does not contrast
      ([#3955](https://github.com/derailed/k9s/issues/3955), open); and its
      highlight leans on `SGR 1`, which Windows Terminal renders as a *bright
      colour* by default, turning the selected row into grey on grey
      ([#3598](https://github.com/derailed/k9s/issues/3598), open — it took a
      thread in microsoft/terminal to find out why). Bold is a per-emulator
      setting, so bold is not a signal either
      ([PRIOR-ART § K](PRIOR-ART.md#k-accessibility))

**Done when:** both palettes render; `COLORTERM` unset degrades instead of
looking broken.
**Frozen after:** `theme.rs`.

## Phase 10 — View state

*Also read: [PRIOR-ART § F1](PRIOR-ART.md#f1--sorting) (sorting the rendered string instead of the value — a defect class k9s has never closed) and [§ F4](PRIOR-ART.md#f4--the-api-surface-is-not-a-constant) (a resource is group + version + resource, always all three).*

Goal: `ui.rs` can be a pure function of state, which is the only thing that
keeps TUI code from rotting.

- [ ] `views.rs`: which view, which item selected, filters, scroll, detail tab
- [ ] Sidebar model built from discovery — groups (workloads / network /
      storage / config / cluster), not a hard-coded list
- [ ] **Grouping by owner** — findings collapse to one card per owner with a
      count ("3 of 40 pods"); the detail view lists which pods. This is the
      single thing standing between Alerts and a 400-line lint report
      ([NOTES § D3](NOTES.md#d3--findings-group-by-owner-not-by-pod))
- [ ] Sorting: severity desc, then recency. Filters: `/` text within the
      current list, `n` namespace substring. **No severity filter** — grouping
      by owner made the list short, and severity is already the sort order
      ([NOTES § D12](NOTES.md#d12--the-key-map-and-two-keys-deleted))
- [ ] **The browser's columns are strings, so decide now whether they sort at
      all** — the sort above is typed and ours. The Resources view is not:
      invariant 12 builds it from the API server's own `Table` output, which is
      **display text**, so a column sort there compares `1Gi` against `999Mi`,
      `2d` against `10h`, `<none>` and `""`. Sorting the rendered string instead
      of the value is the single cause under a defect class k9s has never closed:
      CPU and memory sort silently doing nothing
      ([#3793](https://github.com/derailed/k9s/issues/3793), 29 comments, open
      since January), age sort broken for CRDs, empty capacities, a **panic**
      when a row has fewer fields than the sort index
      ([#3926](https://github.com/derailed/k9s/pull/3926)), and a comparator that
      was not a strict weak ordering
      ([#4070](https://github.com/derailed/k9s/pull/4070)). Two honest answers:
      the browser offers no column sort in v1, or each column type gets a named
      parse with the unparseable pinned last and a test of its own. Choosing
      neither is how #3793 stays open for a year
      ([PRIOR-ART § F1](PRIOR-ART.md#f1--sorting))
- [ ] Modal state: confirm dialog, typed-name confirmation, help overlay
- [ ] Unit tests — selection and filtering are logic, and logic gets tests
      even when it is "just UI"

**Done when:** every navigation and filter case is exercised by tests with no
terminal involved.
**Frozen after:** `views.rs`.

## Phase 11 — The console

*Also read: [PRIOR-ART § C2](PRIOR-ART.md#c2--empty-and-not-loaded-yet-are-different-screens) (loading, empty and denied are three screens) and [§ D3](PRIOR-ART.md#d3--wrapping-and-resizing-must-be-pure-functions) (a wrap that leaks into the data).*

Goal: the screens in [`screens/`](screens/README.md) — the lazygit-shaped
product. Nothing on this list is a design decision any more; every layout,
string and key was settled in the design phase, so this phase is drawing.

- [ ] **First, `tui-designer` settles the ragged right edge on the Alerts
      cards** — `4 min ago` stops two columns short of the border and
      `6 days ago` sits flush against it, so the mockup does not say whether
      the timestamp is right-aligned or trailing the title. Pre-existing, and
      an ambiguous mockup transcribes into an arbitrary renderer
- [ ] Layout: sidebar · content pane · command log strip · key footer
- [ ] **Alerts view** (the default on startup): findings list, severity symbol,
      title bright / evidence dim, blank line between findings
- [ ] **Resources view**: generic table driven by server-side columns; works
      for a CRD without a line of code written for it
- [ ] **Analysis view**: the Phase 4 reports, one pane each
- [ ] Detail tabs per object: logs · describe · yaml · events, `[` / `]`
- [ ] Command log panel — always visible, showing what k8rs ran
- [ ] Context-sensitive key footer + `?` full key map, keys exactly as
      [NOTES § D12](NOTES.md#d12--the-key-map-and-two-keys-deleted) assigns them
- [ ] Confirmation dialogs: consequence in plain language above the kubectl
      line, and the typed-name variant for delete
- [ ] **A dialog tracks its object while open** — holds `uid` +
      `resourceVersion`, and the watch behind it turns the dialog into "already
      gone" or "it changed, re-read" instead of confirming a name that now
      belongs to something else
      ([NOTES § D22](NOTES.md#d22--a-confirmation-can-outlive-the-thing-it-confirms))
- [ ] **Keys the user is not allowed to use are dim from the start**, from the
      `may_i` result, with the reason in the footer. The typed-name delete
      exists to prevent an accident, not to waste the time of someone who was
      never permitted ([NOTES § D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards))
- [ ] **The in-flight screen**: `changing…` in the header, `…` on the command
      line, `q`/`X`/a second mutation refused in the footer
      ([dialogs.md](screens/dialogs.md))
- [ ] States, all eight of [screens/states.md](screens/states.md): loading N
      pods · nothing is broken · disconnected · **login expired** ·
      namespace-scoped fallback banner · and the three startup errors that
      print before the TUI exists
- [ ] **Cluster switcher** (`X`), [screens/context.md](screens/context.md):
      picker over `Kubeconfig::contexts`, then the Phase 5 `connect()` call
      again with everything from the old context dropped. Refused while a
      write is in flight; a failed switch stays on the chosen context and says
      why, it does not fall back
- [ ] `--read-only` visibly marked in the header

**🔒 Security gate:** render a fixture containing ANSI escapes, a right-to-left
override and a 10k-character single-line name — the screen must survive
unchanged. Confirmation dialogs show the *object identity* the action will hit,
so a stale selection cannot be confirmed blindly. Nothing revealed from a
Secret is redrawn after the reveal is dismissed.

**Done when:** the running screen matches [`screens/`](screens/README.md) at
80×24; every key in the footer works.
**Frozen after:** `ui.rs`.

## Phase 12 — Final wiring · **milestone M3**

*Also read: [PRIOR-ART § A5](PRIOR-ART.md#a5--the-perf-fix-that-got-reverted) — k9s's own "skip the cycle when nothing changed" was merged and reverted a month later, and invariant 7 is the same manoeuvre. Also [§ D4](PRIOR-ART.md#d4--the-terminal-after-a-subprocess): leaving raw mode and re-entering it is one function, not one per path.*

Goal: one binary, live and safe.

- [ ] `main.rs`: single `tokio::select!` (watch streams · crossterm events ·
      Ctrl-C), draw-on-change with ~100ms coalescing, block when idle
- [ ] **A coalescing test that ends quiet and asserts the final state** — the
      loop above draws on change with ~100 ms coalescing, which is invariant 7
      and also the exact manoeuvre k9s merged and reverted a month later
      ([#3989](https://github.com/derailed/k9s/pull/3989) →
      [#4033](https://github.com/derailed/k9s/pull/4033), *"skip reconcile cycle
      when informer data is unchanged"*). A coalescer that drops the **last**
      event of a burst shows stale data forever, and it passes every test that
      only checks that events arrived. Feed a storm, stop feeding it, and assert
      the screen equals the last event once the debounce window has expired — the
      defect is a redraw that never comes, so the assertion has to be made after
      the quiet, not during the noise
      ([PRIOR-ART § A5](PRIOR-ART.md#a5--the-perf-fix-that-got-reverted))
- [ ] **Ctrl-Z (SIGTSTP) hands the terminal back properly** — leave raw mode
      and the alternate screen on suspend, re-enter on resume. Without it the
      shell gets a raw-mode terminal and `fg` returns to a dead screen. It is
      the same handover `e` needs in v0.4, so it is written once here and
      reused ([NOTES § D24](NOTES.md#d24--ctrl-z))
- [ ] Panic-safe terminal teardown (`Drop` guard + panic hook, no token in
      backtraces). There is no temp file to clean up in v0.1 — `edit` is the
      only thing that makes one, and it lands in v0.4 with that clause added
      back to this gate
- [ ] Flags from `std::env::args`: `--read-only`, `--context`, `--namespace`,
      `--once`. Four booleans-and-strings is still not a reason for clap; the
      threshold is a flag needing validation, or a subcommand
- [ ] Manual pass of the REQUIREMENTS error-state list (no kubeconfig, 403 on
      read, 403 on write, API down mid-run, watch drop, rejected admission,
      409 conflict)
- [ ] Idle CPU measured at 0%; memory measured at ~1000 pods

**🔒 Security gate:** force a panic on purpose and check two things at once —
the terminal is restored and the backtrace on stderr contains no credential.
This is the one path that is never exercised by accident, so it gets exercised
on purpose.

**Done when:** k8rs runs against kind end-to-end and every error state
behaves as specified.

## Phase 13 — Ship v0.1 · **milestone M4**

*Also read: [PRIOR-ART § J](PRIOR-ART.md#j-distribution) (every packaging channel is a support queue) and [§ L1](PRIOR-ART.md#l-two-observations-about-the-tracker-itself) (most reports are about the environment — the README answers kubeconfig and RBAC plainly, or the tracker becomes a support desk).*

- [ ] `README.md` (EN): what/why, screenshot or asciinema, install, **both**
      RBAC examples, the `--read-only` flag, "no telemetry" statement, and an
      honest paragraph on what k8rs can change in your cluster ·
      `README_TR.md` translation
- [ ] `docs/` refreshed against the code as built
- [ ] Release workflow: tag `v0.1.0` → git-cliff CHANGELOG → musl/darwin
      binaries + `SHA256SUMS` → GitHub release; crates.io publish over v0.0.1
      (the placeholder was replaced back in Phase 5)

**🔒 Security gate:** `strings` the release binary — no path from the build
machine that leaks a username, no embedded credential. `SHA256SUMS` published.
The README states plainly what k8rs can change in a cluster, what
`--read-only` does, and that nothing is sent anywhere.

**Done when:** a stranger in their first month on the job can download a
binary, run it against their cluster, understand a finding, and fix it —
without asking us anything.

---

## Later (recorded, not planned)

- **Rule 8 outside `kube-system`** — the narrowing that keeps it quiet for
  node infrastructure is namespace-bound, and most storage operators live
  somewhere else: Rook in `rook-ceph`, Longhorn in `longhorn-system`, every
  CSI node plugin on `/var/lib/kubelet/plugins` writable. **The namespace is
  not the whole of it, and a live k3s cluster proved so**: the local-path
  provisioner's `helper-pod-create-pvc-…` runs *inside* `kube-system` with **no
  owner at all**, and `node_agent` asks for `mirror || DaemonSet`, so an
  ordinary 64Mi PVC on a healthy k3s cluster draws a CRITICAL. Widening the
  exemption to owner-less pods is not obviously right either — a bare pod with a
  writable host mount is also exactly what an attacker leaves behind. On those
  clusters
  rule 8 prints a wall of CRITICALs, which fails
  [invariant 13](CLAUDE.md)'s first half. **Not fixed on purpose:** widening
  to any DaemonSet anywhere deletes the rule's reason, and an allowlist is
  configuration this project does not have. It needs evidence from a cluster
  that has one of these installed — and the answer may be *severity* rather
  than silence, since the plain hostPath case already belongs to the Analysis
  posture rows
  ([NOTES § D70](NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13))
- **The runtime sockets deliberately left out of the list**, each with the
  reason, so nobody re-opens them from scratch: `/run/nri/nri.sock` grants the
  same node-root but kindnet mounts `/var/run/nri` writable on every kind
  cluster, so with ancestor matching it would light a CRITICAL on a healthy
  screen — a decision, not an add. microk8s's
  `/var/snap/microk8s/common/run/containerd.sock` is **not under `/run`** and
  cannot join without breaking the fold invariant the whole compare rests on.
  Podman's `/run/podman/podman.sock` is a build-farm socket rather than a node
  one; `containerd.sock.ttrpc` and `/var/lib/kubelet/device-plugins/kubelet.sock`
  are real but different escalation classes. `/var/run/dockershim.sock` (k8s
  ≤1.23, still in older crictl probe lists) is redundant rather than missing —
  any node that has it runs dockerd, so `/run/docker.sock` is on the same node
  and already covers it. And no list closes the case at all: a kubelet
  `--container-runtime-endpoint` can put the socket anywhere
  ([NOTES § D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13))
- **The two node takeovers rule 8 is structurally blind to**, found by the second
  operator review and recorded here because walking past them twice is how a
  third reviewer finds them again. Neither is a defect in rule 8; both are
  outside what it looks at.
  1. **No hostPath needed.** `hostPID: true` + `privileged: true` +
     `nsenter --target 1` is a complete node takeover with zero volumes, and
     `privileged`, `hostPID`, `hostNetwork` and `capabilities` appear nowhere in
     `rules.rs` — run live on a k3s node, the pod printed the node's hostname
     and rule 8 emitted nothing, while a **read-only** `/var/run` mount gets the
     loudest card on the screen.
  2. **A hostPath that is not a `hostPath`.** A `PersistentVolume` with
     `hostPath: /run/k3s/containerd`, bound to a PVC and mounted through
     `spec.volumes[].persistentVolumeClaim`, hands over the socket while
     `host_path_mounts` sees an empty list — verified live, the container really
     had the socket. This is the documented Pod Security Admission gap
     (Baseline/Restricted block `hostPath` volumes and do not block PVCs), which
     makes it the shape someone who has read the docs uses.

  **Not v1, and the reason is the same for both:** a securityContext rule is a
  posture rule, not a triage rule — nothing is *broken*, and Alerts is for what
  is broken (the Phase 4 posture rows are where it belongs, and there is no
  posture report yet to receive it). The PVC path additionally needs a PV lookup,
  which the permanent-watch budget has no room for
  ([NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)).
  Until one of those exists, k8rs must not be read as an admission controller
- **v0.2** — cordon / uncordon / drain, wired to the N-series rules and the
  drain-safety report that already exist by then. Cheaper than it looks:
  kube-rs provides `Api<Node>::cordon` / `uncordon` and `Api::evict`, so the
  work is the confirmation UX and the blocker report, not the API calls.
  Plus **`rollout undo`**, which is *not* cheap — it is not an API verb;
  kubectl reads the previous ReplicaSet's template and patches it back
  client-side, and k8rs has to do the same
  ([NOTES § D7](NOTES.md#d7--rollout-undo-joins-the-operation-set)).
  **The drain command line in [dialogs.md](screens/dialogs.md) has to be
  settled when this lands:** it reads `kubectl drain node-3
  --ignore-daemonsets`, and on any node holding a pod with an `emptyDir` —
  which is most real nodes — that command *stops* with "cannot delete Pods
  with local storage". k8rs drains through the Eviction API, which has no such
  client-side guard, so as written the tool would proceed where its own
  printed command refuses, and [invariant 4](CLAUDE.md) says the shown command
  is the one the user would have typed
- **v0.2 rule set** — J1/J2 (failed Job, suspended or overdue CronJob),
  H1 (HPA pinned at max or unable to compute metrics), Q1 (ResourceQuota
  exhausted). Each needs a watch the two-permanent-watch budget has no room
  for in v1; they arrive together, once
  ([NOTES § D9](NOTES.md#d9--one-rule-added-to-v1-the-rest-recorded-not-built))
- **v0.3** — `exec` and `port-forward`: the two operations that need real PTY
  and socket work, and that widen the trust boundary (see
  [docs/security](docs/security.md#data-displayed-and-stored)). The terminal
  handover spike that was cut from Phase 8 happens here
- **v0.4** — `edit` + apply: fetch → YAML to a temp file (mode 0600) →
  `$EDITOR` → diff (`similar` joins the dependency set here) → dry-run →
  apply → remove the file. Unchanged file = no-op; unset `$EDITOR` is an error
  naming the variable, never a guess at `vi`.
  **🔒 The gate that came with it:** `$EDITOR` is spawned with an *argument
  vector, never a shell string* (a pod named `; rm -rf ~` must be boring —
  test it); temp file mode 0600 in the user's own temp dir, removed on exit
  *and* on panic
- **v0.5** — Events watch + rule 11 (probe failures) and the noisy-stream
  handling it requires. Rule 10 shipped in M1
- **Traffic adapter** — Prometheus / Istio / Hubble, endpoint from user config
  only, never auto-discovered
- **Connectivity mesh** — goldpinger-style, a separate binary and repository,
  because it needs a DaemonSet and that is the one guarantee k8rs keeps
