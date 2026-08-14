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
      oldest — `v1_32` — is available and keeps the ±2 minor window. The same
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
- [x] `cargo init` — edition 2024, `rust-version = "1.85"`, release profile
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
- [x] CI: fmt → clippy `-D warnings` → test → `scripts/check-docs.py` ·
      rust-cache · cargo-deny · `cargo check --target` matrix
      (musl x86_64/aarch64, darwin). Top-level `permissions: contents: read`,
      every third-party action pinned to a commit SHA, no `pull_request_target`
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
      | `cargo deny check licenses` | nothing — it failed on its own, on the *first* CI run | **red** |

      `cargo deny` was written off as unprovable until the first dependency
      existed, and then proved itself: it checks the **root crate** as well as
      the graph, so k8rs's own `GPL-3.0-or-later` failed the permissive-only
      policy meant for dependencies. Fixed with an exception scoped to this one
      crate — the policy still rejects a copyleft *dependency*, which is what
      it is for. What that run proves is that the licence check runs and can
      reject; the advisory and copyleft-dependency paths are still unproven and
      belong to Phase 3, with the first real dependency
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

> **This phase is OPEN and Phase 3 is running ahead of it — deliberately, and
> at a cost that is now measured.** Its four remaining boxes are all one thing:
> the kind cluster trip. Deferring it was the user's call on 2026-08-12 and the
> reasoning is [NOTES § D47](NOTES.md#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12).
> **Phase 3 cannot close before Phase 2 does** — twelve of its tests currently
> stand on hand-set fields waiting for an object this trip brings back, and a
> phase does not close with a known gap in it.

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
      one
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
      Original note follows:
      Rule 12 needs one extra move: `kubectl delete pod broken-stuck
      --wait=false` leaves a pod Terminating forever behind its finalizer,
      which is the state to capture. `cluster-down` must strip that finalizer
      or the kind cluster will not tear down
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
      **The rebuild happened on 2026-08-12, on the development machine** —
      `just cluster-up` from `kindest/node:v1.36.1`, clean, nothing applied to
      it yet. Docker access there is per-login: the user is in the `docker`
      group but a stale session is not, so cluster commands run as
      `echo '<cmd>' | newgrp docker` until the next login. **The re-capture
      moves the pinned `now`** in `src/rules.rs`'s `fn now()`,
      `scripts/certs-test.sh` and `scripts/make-certs.sh` together — they
      describe one afternoon or none
      ([NOTES § D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)).
      **Closed 2026-08-13, the other half run:** `break` → `verify` (23/23) →
      `just fixtures` (34 fixtures from v1.36.1, the sanitizer test green, then
      `break-nodes` and 3/3 node predicates) → the capture read *before*
      teardown, so a missing shape could still have been re-captured against a
      live cluster → `unbreak` (three node changes undone) → `just
      cluster-down`. The teardown stripped `broken-stuck`'s finalizer and the
      cluster is gone, which is what this box was waiting to see
      ([NOTES § D64](NOTES.md#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13))
- [x] **A broken pod that has an owner** — added to
      [`scripts/broken.yaml`](scripts/broken.yaml) and captured on the same
      trip. Every pod fixture in the repo has `ownerReferences: null`, so the
      grouping key's four workload branches would ship tested only in their
      no-owner case, and mutation testing cannot object to a branch nothing
      exercises. A Deployment with a crashlooping pod covers
      Deployment/ReplicaSet in one object
      ([NOTES § D36](NOTES.md#d36--the-finding-shape-the-review-sent-back-2026-08-12)).
      **The manifest and the verification have landed** — `broken-owned` in
      `broken.yaml`, the `[owned]` predicate and its seven negatives in
      `cluster.sh verify`, both halves of the crash loop in `verify-test.sh`'s
      corpus, and the two `owned-*.json` lines in `just fixtures` with a guard
      that refuses a capture carrying no controlling `ownerReference`. The box
      stays open for the capture itself, which three boxes share and which
      happens once, after all three have their manifests.
      **Captured 2026-08-13:** `owned-pods.json` carries
      `broken-owned-7bdb7645c8-vhwcp` under a controlling
      `ReplicaSet/broken-owned-7bdb7645c8`, with `owned-replicasets.json`
      beside it, so the grouping key's Deployment/ReplicaSet branch has a real
      object instead of a `null`
- [x] **A mirror pod**, captured on the same trip: `kubectl get pods -n
      kube-system -o json` from the kind cluster. kubelet writes an
      `ownerReference` of kind `Node` onto every static pod, which is the one
      shape that makes a Node an owner — and it is the claim behind the ruling
      that a Node in the owner role is the no-owner case. Right now that
      behaviour is documented upstream and asserted by nobody here; a capture
      turns it into a fixture the rule can be tested against
      ([NOTES § D39](NOTES.md#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)).
      **This capture now has three consumers, not one** — it is also the only
      source of a `mirror: true` pod for N2's drain-aware count, and the only
      negative fixture rule 8 has: every CNI/CSI/node agent in `kube-system`
      mounts a writable hostPath legitimately, so without it rule 8 ships with
      its false-positive class never run
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
      **The capture line and its guards have landed** — `kube-system-pods.json`
      in `just fixtures`, guarded after the filter for all three consumers (a
      pod owned by a *controlling* Node, a controlling-DaemonSet pod with a
      writable hostPath, and a read-only hostPath mount). Preparing for this
      shape closed two sanitizer holes it exposed — the Node `ownerReference`
      as the fifth place a node name lives, and bracketed IPv6 in a URL
      ([NOTES § D62](NOTES.md#d62--the-fifth-place-a-node-name-lives-and-a-guard-that-asked-less-than-its-consumer-2026-08-12)).
      **Captured 2026-08-13:** `kube-system-pods.json` holds the four static
      pods — etcd, kube-apiserver, kube-controller-manager, kube-scheduler —
      each with the `Node` `ownerReference` the kubelet writes. The upstream
      behaviour behind the no-owner ruling is now a fixture, and N2's
      drain-aware count and rule 8's false-positive class have the negative
      they were shipping without
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
      - [`scripts/healthy.yaml`](scripts/healthy.yaml): `limits.memory` on the
        `migrate` **init container** · a second, **`readOnly: true` hostPath
        mount** — the posture case belongs on the healthy side, and rule 8
        fires on writable, so nothing today catches a decode that always says
        writable · an init container with **`restartPolicy: Always`** — the
        native sidecar, which is how every service mesh runs and the only
        object that separates `Sidecar` from `Init` in a real capture · a pod
        declaring **`spec.resources.requests`** at the pod level, which is the
        only object that proves N5 does not sum zero for it
      - [`scripts/broken.yaml`](scripts/broken.yaml), the second round: a pod
        whose **memory limit was patched onto a node that cannot fit it**,
        captured with the resize still pending, so `status.resources` and
        `spec` genuinely disagree — today's fallback test synthesizes that
        divergence · a container with **`terminationMessagePolicy:
        FallbackToLogsOnError`** that died, for a real `terminated.message`
        instead of a written one · a pod with a **pod-level `limits.memory`
        whose container declares only a cpu limit** — the kubelet copies the
        pod's memory limit into that container's status while its spec has
        none, which is the one shape that proves `effective()` does not drop a
        key the spec never declared
        ([NOTES § D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12))
      - [`scripts/cluster.sh`](scripts/cluster.sh) `break-nodes`: one worker
        **cordoned**, one **tainted**, and one with its **kubelet stopped**
        before the node capture — real N1 and N6 positives. **Not N3:** a
        stopped kubelet makes every condition `Unknown`, which is N1; pressure
        conditions need an eviction threshold crossed, which is a cluster
        change and not a workload
      - **Not on the trip, synthesized permanently**: a node whose
        `allocatable` differs from its `capacity` (needs `--kube-reserved` on
        the kubelet, a cluster change and not a workload), a non-controlling
        `ownerReference` (producing one means contorting `broken.yaml` into a
        shape no real workload has), and **N3's pressure conditions**, for the
        reason above
      - **The manifests, the predicates and the guards have landed
        (2026-08-12); the box stays open for the capture**, which it shares
        with the three boxes above it. What landed: thirteen shapes in the two
        manifests, `break-nodes` as its own subcommand, the predicate table
        grown 14 → 26 with the healthy side and the three node states covered
        for the first time, and twelve `just fixtures` guards that name the
        field each consumer needs. Two operator reviews sent it back once, over
        a predicate that could never pass and six smaller defects
        ([NOTES § D63](NOTES.md#d63--the-field-kubectl-never-writes-and-a-substitution-test-that-could-not-see-a-clause-2026-08-12))
      - **What the trip is told to do when a node predicate fails.** Two of the
        three read a taint the *node controller* writes, not one `kubectl`
        does, and that could only be read from the source. If `[cordoned]` or
        `[notready]` FAILs after its 420s, the cluster is not the problem: drop
        the clause naming that taint, and the decode it was retiring stays a
        synthesis. Do not hand-edit the capture
        ([D53](NOTES.md#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)).
        A failure *after* `break-nodes` has tainted costs `unbreak` + `break` +
        a fresh settle, because the `NoExecute` taint evicts bare pods and
        nothing recreates them
      - **One design question the capture answers, and the PM owns reading
        it.** [`screens/alerts.md`](screens/alerts.md)'s cordon card argues a
        cordon has no age because `timeAdded` is written for NoExecute taints
        only — the sentence upstream deleted as inaccurate. `[cordoned]`
        requires the controller's `node.kubernetes.io/unschedulable:NoSchedule`
        taint to be in `nodes.json`, so the capture shows whether it carries a
        stamp. If it does, a cordon time *is* readable and
        [D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
        gets revisited with `tui-designer`; if it does not, D43 stands on a
        fact instead of a deleted sentence. Either way it is answered, not
        assumed ([NOTES § D63](NOTES.md#d63--the-field-kubectl-never-writes-and-a-substitution-test-that-could-not-see-a-clause-2026-08-12))
      - **`just check` goes red the moment the capture lands, and that is the
        synthesis retiring, not breakage.** `rules.rs`'s two hostPath tests
        assert a one-mount pod; the recaptured `hostpath.json` has two. The
        pinned scheduler message in the Pending test changes with the respin
        and with the fourth node. `dev-core` owns those edits, at capture time,
        not before
      - **The trip ran on 2026-08-13 and the box closes with it.** 34 fixtures
        from `kindest/node:v1.36.1`; `verify` 23/23 and the node predicates
        3/3; every one of the thirteen shapes on disk. Three things the trip
        settled that reading could not: the resize fixture **cannot** be
        written against a constant — a request above the node's allocatable is
        refused at *admission* and parks nothing, so the target is read off the
        node at break time and the reachable parking is `Deferred`, not the
        `Infeasible` a review had approved. `break` was **not idempotent** over
        a cluster it had already broken, and the probe meant to catch that read
        a template the apply had just restored. And the cordon question above
        is answered **yes**: `nodes.json` carries `timeAdded` on the mirrored
        `unschedulable` taint, so D43's premise fell and the card is
        `tui-designer`'s round
        ([NOTES § D64](NOTES.md#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13))
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
      **"All four are enforced" was true of `*.json` and not of `certs/`, and
      that is now fixed rather than reworded** — `fixture-audit.sh` applied its
      base64 predicate to JSON only and skipped `certs/*.crt.pem` outright, so
      a real certificate with a base64-wrapped key appended printed *"no key
      material"* and exited 0. The one directory where key material is actually
      generated was the one the check exempted
      ([NOTES § D52](NOTES.md#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12))
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
      **It was passed over once and is the first box of the returning trip.**
      An audit on 2026-08-12 found it still open — nothing blocked it, it needs
      no cluster — while Phase 3 ran ahead; the four boxes above it are the only
      ones carrying a deferral. Order for the trip: this box, then the manifests,
      then the capture, then the teardown
      ([NOTES § D58](NOTES.md#d58--a-phase-2-box-was-passed-over-and-the-order-it-comes-back-in-2026-08-12)).
      **Wider than the box, and one blocker found on the way
      ([NOTES § D59](NOTES.md#d59--the-sanitizer-refuses-a-requester-and-an-exit-status-guard-cannot-see-a-deletion-2026-08-12)):**
      the allowed identity set is *derived* from the live pinned cluster, not
      curated, and anchored at both ends; `.spec.extra` and `.spec.uid` take
      the payload treatment instead, scoped to the object carrying the marker;
      the marker is `signerName` **or** `issuerRef`, because cert-manager's
      `CertificateRequest` carries the identical fields and went through
      unmodified with an OIDC email in it. **The blocker:**
      `fixture-audit.sh`'s new backstop asked whether the filter would *refuse*
      a committed fixture, and a deletion is invisible to an exit-status check
      — `csr-pending.json` predated the `.spec.extra` clause and both guards
      printed green over it. The question is now whether the filter would
      *change* the file; the fixture was re-captured, not edited. And
      `make-csr.sh` sanitized with `> "$out"` onto the committed fixture, so a
      refusal truncated the file it exists to produce

**🔒 Security gate:** the sanitizer lands before the first fixture and is
itself tested — feed it a *poisoned* object (fake token in an annotation, env
value, node IP, private key) and assert the output is clean, **in every shape
the capture produces**: a single object *and* the `List` from
`kubectl get -A`. A sanitizer with no test is a hope; a sanitizer tested on one
of two shapes is worse, because it reads as proven
([NOTES § D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
The poisoned object must also carry each secret in **every framing it can
arrive in** — whole value, embedded in a sentence, and base64-encoded — because
both of the sanitizer's remaining holes were framings, not shapes
([NOTES § D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
Certificates in fixtures are generated locally; no real cluster material, ever.
**"and expire quickly" was reversed and this line said so for one phase too
long (corrected 2026-08-13):** a certificate with a short relative life is a
test that passes today and fails in three weeks, and the usual repair for that
is to weaken the test. The dates are **pinned** instead, and the safety the
original wording was reaching for is delivered by other means —
[`scripts/make-certs.sh`](scripts/make-certs.sh) generates self-signed
throwaways locally and deletes the private key it was forced to write, and
`fixture-audit` fails the build on key material in any framing
([NOTES § D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)).

**Done when:** `just fixtures` regenerates the captured fixtures from scratch
and they are committed. **It does not regenerate the certificates or the CSR**,
and that is deliberate rather than an omission: their dates are pinned, so
there is nothing for a re-capture to refresh, and re-running the generator
writes private key material into the repo for no gain. `just fixtures` runs
[`scripts/certs-test.sh`](scripts/certs-test.sh) over the committed ones
instead, which is the assertion that matters.
**Frozen after:** the data layer (fixtures change only via re-capture, never by
hand) **and the justfile — with one declared exception**: the `e2e` recipe
carries a placeholder body and the file says so at its declaration, because the
write path it drives does not exist until Phase 7. Phase 7 writes that body and
nothing else in the file. Reading the freeze as absolute would leave Phase 7
unable to do what the justfile itself instructs it to.

## Phase 3 — The product: rules · **milestone M1**

Goal: k8rs diagnoses correctly, headless. Still the core — everything else in
this plan is delivery mechanism for what this phase produces.

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
      **Six more after the third review** — pod-level requests
      (`PodSpec.resources`, or N5 sums zero for a pod that committed four
      CPUs), the *enacted* memory limit from `ContainerStatus.resources` rather
      than the requested one from `spec`, the API **group** in `ObjectKind` (an
      OpenKruise Advanced StatefulSet is `Kind: StatefulSet` and Phase 7 would
      aim `scale` at the wrong object), `Terminated.started_at` and `.message`,
      and C1's input — the kubeconfig context name and client certificate,
      never the key
      ([NOTES § D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12))
- [x] **`Snapshot` carries `now`**, and every fixture pins it. Rule 12 and the
      certificate rules need the time; calling a clock inside a rule would
      break [invariant 5](CLAUDE.md) and would make fixtures expire — a test
      that rots is a test that gets weakened
      ([NOTES § D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)).
      **The field is `Time`, not a bare `jiff::Timestamp`** — the same newtype
      every decoded API timestamp already wears, so the comparison every rule
      makes is two values of one type
      ([NOTES § D54](NOTES.md#d54--now-is-metav1time-not-a-bare-jifftimestamp-2026-08-12)).
      **The pin is `2026-08-13T00:00:00Z` since the second capture trip
      (2026-08-13); it was `2026-08-12T00:00:00Z` when this box was written,
      and it was not chosen freely either time:**
      `scripts/certs-test.sh` already asserted the certificate fixtures against
      that instant, and it now extracts the Rust pin and refuses to disagree
      with it — the one edge of that coupling nothing was guarding. The pin
      moves with the capture, in four places, and its cost is that **nothing in
      the fixture set can be "recent"**
      ([NOTES § D57](NOTES.md#d57--the-pinned-now-is-part-of-the-fixture-contract-and-it-makes-recent-unrepresentable-2026-08-12)).
      **Three corrections the reviews forced, none of them cosmetic:** the
      guard first asserted every snapshot timestamp `<= now`, which is false of
      `deletionTimestamp` — the apiserver writes *request time + grace*, so a
      pod inside its grace period, i.e. rule 12's own negative fixture, was
      rejected and the user's clock blamed; it asserts `deletionTimestamp −
      grace <= now` instead. D18's clock-skew sentence had the direction
      backwards and had been copied into the code
      ([NOTES § D55](NOTES.md#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12)).
      And the arithmetic the next box will write has three traps, now named on
      the field: `.0` on both sides, `a - b` is a seconds-only `Span` whose
      `.get_minutes()` is `0` over 43 minutes, and the grace subtraction is
      `checked_sub` because a real apiserver accepts a grace that overflows it
      ([NOTES § D56](NOTES.md#d56--c1-cannot-represent-never-expires-and-a-rule-may-not-return-a-result-2026-08-12))
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
      **Reopened once by the operator review, and it was right to be**: the
      "just now" branch had no bound on the future side, so a rule filling
      `timestamp` from a certificate's `notAfter` would have printed a
      plausible sentence instead of being visibly wrong. `age` answers
      `Option<String>` and refuses past five minutes of skew; the render
      decision moved behind `Finding::age(now)` so neither renderer retypes
      it; and the field now carries the **right source field per rule**,
      because "the wrong-field class" named no pairs and three of them are one
      line apart from the right answer
      ([NOTES § D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13))
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
      **The skip horn won, and this machine forced it** — there is no `rustup`
      here at all, so "require" means red forever on the machine that closes
      phases. Paid for three ways so a *green* run still shows it: `cross` runs
      last, the banner names every skipped target, and it goes to stderr. An
      unknown triple and an unreadable matrix are **not** skips. `just cross`
      now reads the target list out of `ci.yml` rather than keeping a second
      copy — a list in two files is the drift this row was made of
      ([NOTES § D67](NOTES.md#d67--the-cross-compile-row-closed-with-a-skip-and-what-the-skip-costs-2026-08-13))
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
      **`conditions[PodReadyToStartContainers]` is the evidence line and not
      the gate** — the review proposed it as the trigger and it is narrower
      than the case: KEP-3085 defines it as *sandbox created and network
      configured*, so `FailedAttachVolume`, an unbound PVC and a volume still
      attached to a dead node all read `True` while the pod sits wedged. It
      says *why*: `False` = no network yet, `True`/absent = the block is after
      the sandbox, almost always a volume.
      **It ships with a negative side only** — every captured pod has the
      condition `True`, so the positive fixture is a capture-trip item below
      ([NOTES § D72](NOTES.md#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)).
      **The operator review built a real kind cluster for this one and three
      blockers came back.** The two evidence sentences were **inverted** —
      the kubelet mounts volumes *before* it creates the sandbox, so `False`
      covers storage *and* network and `True` means the mounts already
      succeeded; the card had been sending a beginner whose ConfigMap was
      missing to look at the CNI. `PodInitializing` was silencing the rule on
      every pod that declares an init container — Istio, Linkerd, migrations,
      most Helm charts — which is most of the class it was added for. And the
      title spoke for every container while the gate needed one. The image
      family (`InvalidImageName`, `ErrImageNeverPull`, `ImageInspectError`,
      `RegistryUnavailable`, `SignatureValidationFailed`) **moved to rule 3**,
      which they always belonged to
      ([NOTES § D76](NOTES.md#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13))
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
      **N2's age is optional, and it fires only when the cordoned node still
      has pods a drain would move** — the narrowing is what stops every routine
      maintenance window raising an alert; a cordoned node with nothing movable
      left is parked, not broken, and belongs to the Capacity report.
      **The "no duration" this box used to require was reversed by
      [NOTES § D65](NOTES.md#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)**:
      the node lifecycle controller stamps `timeAdded` on the `NoSchedule`
      taint it mirrors from `spec.unschedulable`, so a `kubectl cordon` carries
      a time and only a hand-applied `kubectl taint` does not. The card is
      drawn both ways, and the *gate* never depended on the clock.
      The finding names the pod count. **"Still has pods" is not the same as
      "a drain left something behind":** a drain never evicts DaemonSet pods
      or static pods, so counting every pod fires N2 on every correctly
      drained node — kindnet + kube-proxy on kind, four static pods on a
      cordoned control-plane node. Not counted: `Succeeded`/`Failed`,
      DaemonSet-owned, `mirror`. **And N2 stays silent on a node carrying an
      autoscaler scale-down taint** (`ToBeDeletedByClusterAutoscaler`,
      `karpenter.sh/disrupted`) — that node is cordoned with pods on it for the
      whole eviction window by design
      ([NOTES § D46](NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
      **N5 adds a native sidecar's requests rather than maxing them** — same
      section — and **a pod-level `spec.resources` request replaces the
      container sum rather than adding to it**; the formula in `rules.rs` is
      the order-free simplification of upstream's `resource.PodRequests` and
      understates the rare pod declaring a plain init container after a
      sidecar. **A scale-down that never finishes is a Drain-safety row, not an
      Alerts card** — N2 stays silent on the taint, so nothing else would ever
      show a PDB-blocked scale-down
      ([NOTES § D51](NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)). **N2 and N5 do not fire at all under namespace scope** and
      say so: both join every pod on a node, and a partial view turns N2 into
      a missing finding and N5 into an understated sum — the degradation
      `docs/architecture.md` § Error handling already specifies for a 403,
      not a new mechanism. N6 is unaffected (node taints + the Pending pod's
      own spec are in scope by definition)
      ([NOTES § D43](NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)).
      **Three timestamp traps, all reachable from fields the snapshot already
      carries:** N3 reads *that condition's* `last_transition`, never `Ready`'s
      off the same flat `Vec`, or a DiskPressure card is dated the node's boot
      time; N6's subject is the **pod**, so `scheduled.last_transition`, never
      the blocking node's taint `added_at`; and N2's age is the age of the
      *taint*, which anything rewriting `node.spec.taints` re-stamps — so the
      card says "cordoned about 2 hours ago" and builds no argument on it.
      **N2 also owes a kubectl line that can show the number it prints**:
      `kubectl describe node` does not print `timeAdded`, so either the card
      offers `-o jsonpath='{.spec.taints}'` or it records that the age is the
      one claim `describe` cannot back
      ([NOTES § D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
      **Shipped, and four things landed that this box did not ask for**
      ([NOTES § D81](NOTES.md#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)):
      D69's choice above resolved to **`describe node`**, because it backs the
      title and the count while `jsonpath` backs only the optional age; **N6 is
      not a card** but the node half of rule 10's, since the two fire on one
      population and D28 forbids two cards for one pod; **`SUPPORTED_SKEW` is
      3**, upstream's number, not the 2 this repo had written down; and a
      **managed-taint translation table** exists because naming those keys raw
      told the reader to tolerate a cordon, an unreachable node and an
      autoscaler's own scale-down
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
- [ ] Certificate rule C1 — kubeconfig client certificate expiry, warn at 30
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
      **A certificate that never expires produces no finding, and that is the
      only shape available:** RFC 5280 spells "no well-defined expiry" as
      `99991231235959Z`, which is past the end of jiff's `Timestamp` range, so
      the conversion returns an `Err` a pure rule may not propagate. The reflex
      shape is `.unwrap()`, the input is a kubeconfig, and a corporate PKI is
      exactly where a non-expiring CA turns up — the panic would land on
      startup
      ([NOTES § D56](NOTES.md#d56--c1-cannot-represent-never-expires-and-a-rule-may-not-return-a-result-2026-08-12))
- [x] Exit-code translation table (137/143/1/126/127) — **137 has two meanings and the object says which**: with `reason: OOMKilled` it is memory, without it the container did not stop when asked, which is a failing liveness probe or a hanging shutdown. The old "almost always OOM" row was written before the rule had `reason` beside the code ([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13))
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
      The operator review then found the same door one level up: matching the
      socket **file** alone left `/run/containerd` — which our own capture
      mounts — drawing the writable card and its *"mount it read-only"* advice,
      which hands over the node. Ancestors match now, k3s/RKE2 and cri-dockerd
      joined the list, and the socket card no longer tells a legitimate node
      agent to remove the mount that is its job
      ([NOTES § D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13))
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
      **The margin is `> 60s`, flat — the `max(30s, grace)` this box used to
      specify was wrong** and charged the grace twice, hiding a pod with
      `terminationGracePeriodSeconds: 3600` for a full hour past its kill
      deadline, which is the Kafka/Vault case rule 12 exists for. The margin
      covers kubelet observation, watch latency and skew; none of those scales
      with a grace the deadline already spent
      ([NOTES § D71](NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13))
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
      **(4) A fourth, and it is two rules that cannot both be obeyed:** rule 3's
      evidence is the runtime's verbatim sentence — ~250 characters on the
      committed capture — because
      [D37](NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12)
      requires the message be quoted and not paraphrased, while
      `screens/widgets.md` § 7 caps a card at 3–5 lines. As built, that card is
      six. `rules.rs` cannot resolve it without breaking D37, so the answer is
      geometry: wrap, truncate with the full text on selection, or let one card
      be tall. Whichever it is, `views.rs` needs it decided before Phase 9, not
      discovered there.
      **It is owed on the `action` line too, not just on evidence** — measured
      off the printed cards, three actions run 200, 149 and 146 characters
      (rule 10's scheduler advice, rule 1's pull advice, rule 8's socket
      advice), and at `alerts.md`'s 45-column card pane the socket action alone
      wraps to four lines by itself. **N6's merge made the worst case worse
      rather than adding a fourth entry**: rule 10's card now carries N6's
      sentence *and* the scheduler's verbatim message on one `·`-joined
      evidence line, which `screens/alerts.md` § N6 draws at **twelve** lines
      and says so rather than pretending it fits. Shortening them is not the fix: each is
      long because it answers a question the reader actually has
      ([NOTES § D79](NOTES.md#d79--the-review-that-found-the-door-beside-the-one-d78-closed-2026-08-13)
      for why rule 8's grew). Mitigating, from D3: findings group by owner, so a
      40-node DaemonSet is one card and not forty.
      **Closed 2026-08-14, and all four answered with drawings rather than
      prose** ([NOTES § D83](NOTES.md#d83--the-hours-rung-runs-to-48-and-the-age-ladder-gets-one-home-2026-08-14)):
      (1) the cordon card prints the ladder's ordinary string and **nothing on
      it reasons from the age** — `timeAdded` dates the *taint*, so a
      `spec.taints` rewrite re-stamps it and the number can only ever be too
      small, which is a safe floor to sort by and a fatal thing to argue from.
      (2) The hours rung runs to **48**, matching `HumanDuration`'s own
      boundary, and `1 day ago` becomes an unreachable string. (3) The age
      column's budget is **14** columns, from the epoch string. (4) The geometry
      is settled at the 80×24 floor — card region 53, body text 51, evidence
      capped at **three** wrapped lines with the full text one `⏎` away, action
      never cut, card 3–10 lines so a second finding is always on screen; and
      `--once` does not cut at all, because it has no keypress to restore with.
      The round also gave the ladder **one home**, `screens/widgets.md` § 1b,
      which is now what `rules::age` cites instead of three screens. Two
      measurements corrected numbers this file had been quoting: rule 3's
      evidence is **347** characters, not ~250, and the longest unbreakable
      token is 58 columns — wide enough that wrapping alone cannot fit it and
      only a character break can
      **The trip ran on 2026-08-14 and this box closes with it: 48 fixtures
      from `kindest/node:v1.36.1`, `verify` 37/37, twelve of the thirteen shapes
      on the first attempt.** Four things it settled that reading could not
      ([NOTES § D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14),
      [§ D85](NOTES.md#d85--rule-1-contradicts-itself-on-a-clean-exit-and-it-gets-its-own-box-2026-08-14)):
      **the capture host must have memory headroom** — a starved one reports
      every memory-limit kill as `reason: "Error"`, which is the word D71 uses
      for the *opposite* rule, and `cluster.sh verify` refusing on the wrong
      host before a byte is written is what saved rule 2's positive fixture.
      **`broken-oomserving` shipped with `count=1`** on a `dd`, and a short read
      from `/dev/zero` satisfies a count without allocating, so the container
      exited 0 and the shape never appeared; `exec tail /dev/zero` has no
      newline to stop at and no short read to end on. **`CAPTURED_PODS` claimed
      "every pod capture in the repository" and held 12 of 31**, so the pin
      guard walked a third of what it named and nineteen captures — every new
      one among them — had their timestamps compared against `now` by nothing.
      And **rule 1 draws a card that argues with itself** on the two objects the
      trip brought back for rule 6, which is D85's own box below.
      **Twelve syntheses retired onto real objects**, three of them branches
      that had no test at all and could be deleted with the suite still green:
      rule 6's `exit 0` and `143` exemptions, and rule 7's `started` suppressor.
      The pin moved to `2026-08-14T00:00:00Z` in five places, one of which
      (`docs/maps.md`) nothing had been guarding
- [ ] **Rule 1 must read how the previous run ended** — it draws *"Container
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
      it, and the capture trip above is not unfinished for having found it
- [ ] Plain-language pass over every string a user will read — the jargon test
      is "would someone in their first month understand this sentence?"
- [ ] Per rule: positive fixture test **and** negative (healthy) fixture test
- [ ] `cargo mutants --timeout 90` clean over `rules.rs` — a MISSED mutant is a
      rule change no test objected to, i.e. a hole in the diagnosis; it gets a
      test, not an excuse. **It proves the rules' logic and not the decode
      beneath them**: it mutates return values and match guards, never a struct
      literal's field assignment, and on the snapshot decode it found 1 of the
      32 holes a hand-written field-level sweep found. That sweep is the gate
      for the decode
      ([NOTES § D41](NOTES.md#d41--cargo-mutants-cannot-see-the-defect-it-was-put-there-to-catch-2026-08-12))
- [ ] Temporary `main.rs` shell (~10 lines): load a fixture path from args,
      print findings. It cannot reach a cluster yet — `k8s.rs` is Phase 5, and
      that is where the v0.0.1 release therefore sits. **It strips control
      characters before printing**: the guard that makes this unnecessary is
      Phase 5's ingest strip, and this is the first code that shows a `Finding`
      — two phases earlier. A printer that displays API text with no guard is
      invariant 9 broken for the length of two phases, and "the fixtures are
      ours" is an argument about today's inputs, not about the code

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

Goal: the cluster-wide answers no per-object rule can give. Pure functions
over a `ClusterSnapshot`, so this phase is as testable as Phase 3 and needs no
cluster either.

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
- [ ] **Drain safety** — for each node, what a drain would do and what would
      block it. A PDB whose `minAvailable` equals the replica count means the
      drain never finishes; say so before, not 40 minutes in
- [ ] **Waste** — **Services whose selector matches no pod first** (the 503
      nobody can explain; it stays here rather than in Alerts because
      promoting it would cost a permanent Services + EndpointSlices watch, and
      the watch budget is why k8rs is lighter than k9s), then unbound/unused
      PVCs, Evicted and Completed pod pileups, ReplicaSets parked at 0
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
      ([NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12))
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
- [ ] Reconnect/backoff surfaced as a state the UI can show
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
- [ ] **Measure resident memory against 10 000 pods** (kind + a generator)
      **plus the three workload watches**, and write the number down. Pruning `managedFields` is agreed; whether the
      pruned store actually fits is unmeasured, and an unmeasured number is not
      a design ([NOTES § D25](NOTES.md#d25--what-this-review-did-not-decide))
- [ ] Startup errors (no kubeconfig / bad context) → stderr + non-zero exit
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

Goal: every write works and is safe, **before a single key is bound to one**.
This is the phase where the reversal actually happens, and it is deliberately
placed low in the pyramid so the dangerous code is proven headlessly.

- [ ] `ops.rs` with the single `#![allow(clippy::disallowed_methods)]`; CI's
      containment check now expects exactly this file
- [ ] The mutation contract, one shared function so no operation can skip a
      step: *consequence text → dry-run → confirm callback → call → audit*
- [ ] Server-side `dryRun=All` wherever supported; a rejected dry-run aborts
      and surfaces the API server's own message
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

- [ ] `theme.rs`: 10 Catppuccin Mocha constants + `COLORTERM` check with a
      16-color fallback
- [ ] Severity symbols `● ▲ ○` — never colour alone

**Done when:** both palettes render; `COLORTERM` unset degrades instead of
looking broken.
**Frozen after:** `theme.rs`.

## Phase 10 — View state

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
- [ ] Modal state: confirm dialog, typed-name confirmation, help overlay
- [ ] Unit tests — selection and filtering are logic, and logic gets tests
      even when it is "just UI"

**Done when:** every navigation and filter case is exercised by tests with no
terminal involved.
**Frozen after:** `views.rs`.

## Phase 11 — The console

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

Goal: one binary, live and safe.

- [ ] `main.rs`: single `tokio::select!` (watch streams · crossterm events ·
      Ctrl-C), draw-on-change with ~100ms coalescing, block when idle
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
