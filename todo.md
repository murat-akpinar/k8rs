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

- [ ] `LICENSE-MIT` + `LICENSE-APACHE`, and `license = "MIT OR Apache-2.0"` in
      `Cargo.toml`. **First, not last:** `cargo publish` refuses a crate
      without the field, so the placeholder cannot be claimed before this
      exists ([NOTES § D13](NOTES.md#d13--licence-mit-or-apache-20))
- [ ] `cargo init` (edition pinned, release profile: lto/strip/codegen-units)
- [ ] Publish `k8rs` placeholder to crates.io (needs `cargo login` — user)
- [ ] `clippy.toml` — `disallowed-methods` ban on the K8s write calls,
      crate-wide. The `ops.rs` exemption is not written yet; nothing may call
      them at this point
- [ ] `justfile`: `check` · `run` · `cluster-up` · `cluster-down` ·
      `fixtures` · **`e2e`** · **`mutants`**. Every target is *declared* here,
      `e2e` and `mutants` included, even though Phase 7 and Phase 3 are what
      use them — a target invented later would be a forward-only violation.
      The body of `fixtures` (the jq sanitizer) is written in Phase 2 where the
      fixtures are, so the justfile freezes there and not here
      ([NOTES § D14](NOTES.md#d14--three-plan-corrections) ·
      [§ D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12))
- [x] `.gitignore` (`/target`, `/tmp`, `/.agent`, `/.vscode`)
- [ ] CI: fmt → clippy `-D warnings` → test → `scripts/check-docs.py` ·
      rust-cache · cargo-deny · `cargo check --target` matrix
      (musl x86_64/aarch64, darwin)
- [ ] CI: **the honest-test guards** — fail the build if `cargo test` ran zero
      tests, and on any `#[ignore]` without a written reason beside it. Both
      are ways a suite reports success without having run
      ([NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12))
- [ ] **Prove every guard red before trusting it** — fmt, clippy, check-docs,
      cargo-deny and the allowlist each get a throwaway commit that violates
      them, on a branch, and CI is watched going red before the commit is
      dropped. Record the five red runs here as they happen; an unproven guard
      counts as absent
- [ ] CI: the write-containment check, written as an **allowlist** — outside
      `src/ops.rs` only `get*` / `list*` / `watch*` / `logs` / `log_stream` /
      `apiserver_version` may appear. A denylist would miss `cordon`,
      `restart`, `evict`, `exec`, `portforward`, `entry`, `patch_scale` and
      whatever kube-rs adds next
- [ ] `deny.toml` (advisories, license allowlist, crates.io-only sources)
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

- [ ] Fixture sanitization in the capture script — **lands before any
      fixture file** (REQUIREMENTS G-5; a leak never leaves git history)
- [x] [`scripts/broken.yaml`](scripts/broken.yaml) (extracted from NOTES,
      which now points at it — two copies of a manifest drift). Planned for
      `tests/manifests/`; it lives beside the script that applies it instead,
      because no Rust test reads it
- [ ] `tests/manifests/healthy.yaml` — the negative side. Every rule needs a
      healthy counterpart or its false-positive test is fiction
- [x] **Pin the kind node image** — `kindest/node:v1.36.1`, pinned in
      [`scripts/cluster.sh`](scripts/cluster.sh). Write the version down — the fixtures are
      captured from it, so an unpinned image means fixtures that change when
      someone re-runs `just fixtures` on a different machine. The pinned
      `v1_32` types talk to anything in the window and above it (forward
      compatibility), so this is a reproducibility choice, not a compatibility
      one
- [x] kind cluster up, states settled (CrashLoop in backoff, OOM kills seen).
      `cluster.sh verify` asserts all nine reached the state their rule is
      about — 9/9 pass. A fixture that never reaches its state is a test that
      cannot fail, and that has to be caught before anything is captured.
      Original note follows:
      Rule 12 needs one extra move: `kubectl delete pod broken-stuck
      --wait=false` leaves a pod Terminating forever behind its finalizer,
      which is the state to capture. `cluster-down` must strip that finalizer
      or the kind cluster will not tear down
- [ ] Add `broken-init` to [`scripts/broken.yaml`](scripts/broken.yaml) and to
      `cluster.sh verify` — an init container that exits non-zero, so the pod
      sits at `Init:CrashLoopBackOff`. Phase 2 is not frozen yet, which is the
      only reason this is cheap
      ([NOTES § D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12))
- [ ] `just fixtures`: capture the 10 broken pods + healthy pods + **nodes,
      deployments, services, PVCs, PDBs** + events + `K8S_VERSION` stamp
- [x] A multi-node kind config — N-series rules (cordon, skew, pressure) and
      drain safety cannot be captured on a single-node cluster. Three nodes
      (1 control-plane + 2 workers); `K8RS_WORKERS` changes the count
- [ ] A cluster-wide snapshot fixture (everything at one instant) for
      `analysis.rs` reports
- [ ] Certificate fixtures: an expiring client certificate PEM (generated
      locally, never a real one) and a pending CSR
- [ ] Eyeball every fixture once: no env values, no annotations, no node IPs,
      no private keys

**🔒 Security gate:** the sanitizer lands before the first fixture and is
itself tested — feed it a *poisoned* object (fake token in an annotation, env
value, node IP, private key) and assert the output is clean. A sanitizer with
no test is a hope. Certificates in fixtures are generated locally and expire
quickly; no real cluster material, ever.

**Done when:** `just fixtures` regenerates everything from scratch;
fixtures committed.
**Frozen after:** the data layer (fixtures change only via re-capture, never by
hand) **and the justfile**, whose last unwritten recipe body lands here.

## Phase 3 — The product: rules · branch `feat/rules` · **milestone M1**

Goal: k8rs diagnoses correctly, headless. Still the core — everything else in
this plan is delivery mechanism for what this phase produces.

- [ ] `Finding` struct (severity · title · evidence · action · kubectl_cmd ·
      **owner** — the grouping key: Deployment/StatefulSet/DaemonSet/Job, or
      the bare pod when it has no owner. Grouping itself happens in `views.rs`;
      the *identity* it groups by is decided here, in the bottom layer)
- [ ] The snapshot types live here, in the bottom layer: `PodSnapshot`,
      `NodeSnapshot`, `ClusterSnapshot`. `k8s.rs` will fill them later; rules
      define the contract
- [ ] **`Snapshot` carries `now`**, and every fixture pins it. Rule 12 and the
      certificate rules need the time; calling a clock inside a rule would
      break [invariant 5](CLAUDE.md) and would make fixtures expire — a test
      that rots is a test that gets weakened
      ([NOTES § D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact))
- [ ] `Finding` carries **timestamps, not phrases**. "4 min ago" is formatted
      by the renderer, so `ui.rs` and the `--once` printer share one source and
      a test asserts a duration instead of parsing English. A non-positive age
      renders "just now" — the API server's clock and the laptop's disagree
- [ ] Pod rules 1–8 and 12 (stuck Terminating). Rule 9 (no limits) is not an
      Alerts rule — it belongs to the Capacity report in Phase 4; rule 8 fires
      only on the escalated hostPath case. Events-based rule 11 stays deferred
      ([NOTES § D2](NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later))
- [ ] **Rule 10 — Pending, and why**, from `conditions[PodScheduled]`: reason
      `Unschedulable` plus that condition's own message, which is the
      scheduler's sentence. No Events watch, no new stream — the fixture is
      already captured
      ([NOTES § D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12))
- [ ] **Rules 1–6 read `initContainerStatuses` too.** A pod at
      `Init:CrashLoopBackOff` produces no finding otherwise, and the finding
      has to name the init container — "the app container is fine, the init one
      is not" is the diagnosis
- [ ] Node rules N1–N6 (NotReady · cordoned-and-forgotten · pressure ·
      kubelet skew · overcommit · what blocks a Pending pod)
- [ ] Certificate rule C1 — kubeconfig client certificate expiry, warn at 30
      days. Pure: PEM bytes in, finding out
- [ ] Exit-code translation table (137/143/1/126/127)
- [ ] hostPath: `rules.rs` fires **only** on `/`, docker.sock or a writable
      host mount. There is no lower severity to escalate from any more — the
      ordinary read-only mount is a Phase 4 posture row, computed there
- [ ] Rule 5 thresholds (≥3 WARN, ≥10 CRITICAL); rule 12's threshold is the
      pod's own `terminationGracePeriodSeconds`, not a constant
- [ ] Plain-language pass over every string a user will read — the jargon test
      is "would someone in their first month understand this sentence?"
- [ ] Per rule: positive fixture test **and** negative (healthy) fixture test
- [ ] `cargo mutants --timeout 90` clean over `rules.rs` — a MISSED mutant is a
      rule change no test objected to, i.e. a hole in the diagnosis; it gets a
      test, not an excuse
- [ ] Temporary `main.rs` shell (~10 lines): load a fixture path from args,
      print findings. It cannot reach a cluster yet — `k8s.rs` is Phase 5, and
      that is where the v0.0.1 release therefore sits

**🔒 Security gate:** no finding text may quote an env value or a Secret —
findings name *fields*, not payloads. The certificate parser is fed malformed
and truncated PEM in a test and must return "no finding", never panic:
`rules.rs` returns no `Result`, so a panic there is a crash of the whole tool.

**Done when:** all rule tests green against real fixtures; running the binary
on a fixture prints correct findings. *The product works here.*
**Frozen after:** `rules.rs`.

## Phase 4 — Analysis reports · branch `feat/analysis`

Goal: the cluster-wide answers no per-object rule can give. Pure functions
over a `ClusterSnapshot`, so this phase is as testable as Phase 3 and needs no
cluster either.

- [ ] `Report` shape: title · rows · the finding each row can jump to
- [ ] **Capacity** — per node: requests vs allocatable vs actual usage, plus
      **the workloads with no limits defined** (the old rule 9, which lives
      here now — it is a risk, not an outage)
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

## Phase 5 — Live reads · branch `feat/watch` · **milestone M1.5**

Goal: the same findings and reports, from a living cluster — and the first
public release.

- [ ] `k8s.rs`: kube-rs `watcher` over Pods and Nodes + prune (drop
      `managedFields`) → snapshot store
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
- [ ] **Measure resident memory against 10 000 pods** (kind + a generator) and
      write the number down. Pruning `managedFields` is agreed; whether the
      pruned store actually fits is unmeasured, and an unmeasured number is not
      a design ([NOTES § D25](NOTES.md#d25--what-this-review-did-not-decide))
- [ ] Startup errors (no kubeconfig / bad context) → stderr + non-zero exit
- [ ] Certificate rules that need the wire: C2 (API server serving cert) and
      C3 (pending CSRs)
- [ ] **The typed lists `analysis.rs` needs**, fetched on demand when a report
      is opened: Deployments, ReplicaSets, Services, EndpointSlices, PVCs,
      PDBs. These are *not* the browser's `Table` path — a report needs
      `minAvailable` and `.spec.selector` as fields, and Table gives strings
      for display. Phase 4 defined `ClusterSnapshot`; this is the step that
      fills it, and it has to happen before `k8s.rs` freezes
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
downstream code has to remember. Field sizes are bounded: a 50MB annotation
must not be stored whole.

**Done when:** watching kind live shows findings appear/disappear as pods
break and heal; discovery lists the CRDs you installed; unplugging the network
shows the reconnect state; and `cargo install k8rs` on a clean machine gives a
stranger a working `k8rs --once`.
**Frozen after:** nothing yet — `k8s.rs` stays the top layer through Phase 6,
which adds the remaining read paths to the same file. It freezes there.

## Phase 6 — Logs and read-only detail · branch `feat/detail`

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

## Phase 7 — Operations · branch `feat/ops` · **milestone M2**

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

## Phase 8 — TUI spike · branch `feat/tui-spike` (throwaway)

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

## Phase 9 — Theme · branch `feat/theme`

- [ ] `theme.rs`: 10 Catppuccin Mocha constants + `COLORTERM` check with a
      16-color fallback
- [ ] Severity symbols `● ▲ ○` — never colour alone

**Done when:** both palettes render; `COLORTERM` unset degrades instead of
looking broken.
**Frozen after:** `theme.rs`.

## Phase 10 — View state · branch `feat/views`

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

## Phase 11 — The console · branch `feat/ui`

Goal: the screens in [`screens/`](screens/README.md) — the lazygit-shaped
product. Nothing on this list is a design decision any more; every layout,
string and key was settled in the design phase, so this phase is drawing.

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

## Phase 12 — Final wiring · branch `feat/wire` · **milestone M3**

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

## Phase 13 — Ship v0.1 · branch `feat/release` · **milestone M4**

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

- **v0.2** — cordon / uncordon / drain, wired to the N-series rules and the
  drain-safety report that already exist by then. Cheaper than it looks:
  kube-rs provides `Api<Node>::cordon` / `uncordon` and `Api::evict`, so the
  work is the confirmation UX and the blocker report, not the API calls.
  Plus **`rollout undo`**, which is *not* cheap — it is not an API verb;
  kubectl reads the previous ReplicaSet's template and patches it back
  client-side, and k8rs has to do the same
  ([NOTES § D7](NOTES.md#d7--rollout-undo-joins-the-operation-set))
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
