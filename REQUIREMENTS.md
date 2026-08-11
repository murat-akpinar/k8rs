# Role-Based Requirements — Developer / DevOps / DevSecOps

> Date: 2026-08-10 · Does not repeat decisions from NOTES.md; records only
> **missing** requirements, contradictions and risks. Produced from three
> role perspectives.

## Flagged by more than one role at once (highest priority)

1. **Write safety must be verifiable** *(DevOps + DevSecOps)*
   *Rewritten 2026-08-11: this requirement used to be "the read-only promise
   must be verifiable". Writes now exist
   ([NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11)),
   so the verifiable claim changes shape — but it must stay verifiable, or the
   tool is just a sharp object.*
   - **Containment:** mutations appear in `ops.rs` and nowhere else, enforced
     by an **allowlist** — outside `ops.rs`, only `get*` / `list*` / `watch*` /
     `logs` / `log_stream` / `apiserver_version` may appear. A denylist is not
     acceptable here: `Api` also exposes `cordon`, `uncordon`, `restart`,
     `evict`, `attach`, `exec`, `portforward`, `entry`, `patch_scale` and
     more, and that surface grows with every kube-rs release (verified
     2026-08-11, [NOTES § Upstream review](NOTES.md#upstream-architecture-review-2026-08-11-docs-in-tmp)).
     clippy `disallowed-methods` stays as the fast local signal.
   - **Consent:** no mutation without an explicit selection, a keypress, and a
     confirmation stating the consequence in plain language. Deletes and drains
     require typing the object name. No bulk mutation exists.
   - **Preflight:** server-side `dryRun=All` wherever the API supports it; a
     failed dry-run aborts and shows the API server's own message.
   - **Evidence:** every attempted mutation — including refused and failed ones
     — is appended to `~/.local/state/k8rs/audit.log` and shown in the command
     log panel as the equivalent kubectl command.
   - **Escape hatch:** `--read-only` restores the original guarantee; the write
     path is unreachable, keys unbound, header marked. CI e2e runs in this mode.
   - **Docs:** two example `ClusterRole` YAMLs (read-only and admin) — the
     proof of what each mode can touch.

2. **Fixture sanitization BEFORE the first fixture commit** *(DevOps + DevSecOps)*
   `kubectl get -o json` puts raw data into the repo; a leaked annotation
   never leaves git history. The capture script must strip (jq):
   `managedFields`, all `annotations` (especially
   `last-applied-configuration` — a full copy of the spec, env values
   included), `env[].value`, `imagePullSecrets[].name`, node names/IPs.
   Fixtures get a note recording the k8s version they were captured from
   (drift tracking).

3. **The k8s-openapi / kube-rs version triangle** *(all three roles)*
   - Pin the **oldest** supported version feature — verified 2026-08-11 as
     **`v1_32`**; `v1_30` is no longer offered by k8s-openapi — the k8s
     API is forward compatible; a client built against an old feature talks
     to newer clusters.
   - Document the supported window in the README: pinned ±2 minor
     (kubectl skew policy).
   - kube-rs + k8s-openapi are coupled: **group them** in Dependabot,
     upgrade together.
   - A stale pin silently produces wrong diagnoses — the most likely
     breaking point of the project.

4. **The truecolor decision contradicts the target audience** *(Developer)*
   NOTES said "fall back when someone complains", but the target audience is
   beginners + the Windows Terminal risk was acknowledged in the same
   document. The ~5-line `COLORTERM` check **belongs in v1**, not deferred.
   First-impression risk, cheap fix. *(Resolved: pulled into v1.)*

5. **Approachability is a requirement, not a finish** *(added 2026-08-11 with
   the lazygit positioning)*
   The user is in their first month on the job. Therefore:
   - Context-sensitive keys are visible on screen at all times; `?` opens the
     full map. Nothing important is reachable only by memory.
   - Zero configuration on first run — no flag, no config file, no setup.
   - The command log panel is always present, showing what k8rs ran as the
     user would have typed it.
   - Confirmations state the consequence in plain language above the command,
     never instead of it.
   - Every visible string avoids untranslated jargon; K8s reasons are
     explained inline the first time they appear.
   - Detail tabs per object (logs · describe · yaml · events) so the whole
     debugging loop needs no typed command.

6. **The Alerts list must stay believable** *(DevOps + Developer, added
   2026-08-11 by the second-pass review)*
   - **Grouping by owner is a requirement, not a presentation detail.** One
     finding per owner (Deployment / StatefulSet / DaemonSet / Job, falling
     back to the bare pod), carrying a count — *"3 of 40 pods"* — with the
     offenders on the detail view. Forty identical rows for one DaemonSet is a
     failed feature.
   - **Alerts holds only what is broken right now.** Risk, waste and expiry
     belong to Analysis. This is what moved rule 9, the plain hostPath case,
     N4 and C1 out of the Alerts list
     ([NOTES § D2](NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).
   - An empty Alerts screen must be *true*, not merely reachable — that is the
     property the whole product rests on.
   - **Namespace scoping is a v1 requirement.** `--namespace/-n`, and a 403 on
     the cluster-wide LIST falls back to the kubeconfig context's namespace
     (then `default`) with the header stating the scope in effect. A user with
     access to two namespaces must not be met with an empty, broken tool
     ([NOTES § D5](NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter)).

## Developer requirements

### Write operations (new — the reversal)

- **Three operations in the first release:** scale · rollout restart · delete.
  Then cordon/uncordon · drain · rollout undo (v0.2), exec · port-forward
  (v0.3), and edit+apply last (v0.4). The ladder follows what an operator does
  in a normal day, not what is easiest to build; `edit` is last because it is
  the most dangerous, the least provable headlessly, and the one thing every
  admin already has a habit for
  ([NOTES § D6](NOTES.md#d6--operation-order-was-inverted-for-the-audience)).
- **Restart is not an API verb.** For workloads it is a patch of
  `spec.template.metadata.annotations.kubectl.kubernetes.io/restartedAt`
  (what `kubectl rollout restart` does); for a bare pod it is a delete, and
  the confirmation must say so — a beginner must not learn "restart" as a
  synonym for "delete" by accident.
- **Edit flow:** fetch → YAML to a temp file (mode 0600) → `$EDITOR` with the
  TUI suspended → on return, diff against the original → dry-run → confirm →
  apply → delete the temp file. An unchanged file is a no-op, not an apply.
  If `$EDITOR` is unset, say so and name the variable; do not guess `vi`.
- **Conflict handling:** apply uses the resourceVersion that was read. On a
  409 the user is told the object changed underneath them and is offered a
  re-read — never a blind overwrite.
- **A confirmation dialog tracks its object while it is open.** The watch is
  still running behind the modal: if the object is deleted while the user is
  typing its name, the dialog says it is already gone and the button dies; if
  it merely changed, the dialog offers the re-read before the call rather than
  after the 409
  ([NOTES § D22](NOTES.md#d22--a-confirmation-can-outlive-the-thing-it-confirms)).
- **A call in flight is a visible state.** The modal closes on confirmation,
  not on completion; the command log line ends in `…` until the result arrives;
  a second mutation, `X` and `q` are refused until it does. `drain` gets a
  progress pane with counts, because it takes minutes
  ([NOTES § D20](NOTES.md#d20--a-call-that-takes-time-is-a-state-and-there-was-none)).
- **Drain must report what blocks it** (PDBs, unmanaged pods) *before*
  starting, not after hanging. This is the whole reason drain is in the tool.
- **Failure is a first-class state:** a rejected write shows the API server's
  message verbatim, in the same panel, and stays visible until dismissed.

### Error states (all were undefined; all happen on first launch)

- No kubeconfig / invalid context → clear message on stderr **before**
  entering the TUI + non-zero exit. Panicking inside the TUI is forbidden
  (raw mode corrupts the terminal).
- API unreachable → clear error at startup; if the connection drops while
  running, show "disconnected, retrying" in the header — never silently
  freeze on stale state.
- Insufficient RBAC (403) → say which permission is missing; if the Events
  watch gets 403, only the affected rule (11) is disabled, the app
  keeps running.
- **Expired credentials (401) → a third state, not a 403.** Managed clusters
  mint short-lived tokens from a credential plugin and those expire mid-session.
  "Your login expired" and "you are not allowed" send the user to two entirely
  different places; conflating them sends a beginner to their platform team
  over a timeout
  ([NOTES § D19](NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)).
- **Permissions are checked before they are needed, not after.** One
  `SelfSubjectRulesReview` per namespace dims the keys the user cannot use and
  says why. Asking someone to type a pod's name in full and *then* telling them
  they were never allowed to delete it is not a safety measure
  ([NOTES § D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)).
- Watch dropped / 410 Gone → kube-rs watcher handles backoff; requirement:
  **the reconnect must be visible in the UI**, silent stale data is
  forbidden.
- Terminal cleanup guaranteed even on panic: `Drop` guard + panic hook.
  (The most common TUI mistake.)

### Behavior gaps

- No findings → a "cluster healthy" screen (not an empty list).
- When a pod recovers, its finding leaves the list (handle the reflector
  delete event).
- Ordering: severity desc, then last-event time desc. Findings are grouped by
  owner (item 6 above); several findings on one owner make one card with
  several lines, not several cards.
- `/` filters the current list by text; `n` by namespace substring. There is
  **no severity filter** — owner grouping made the list short and severity is
  the sort order already. Fuzzy search is YAGNI. Every key has one meaning
  everywhere ([NOTES § D12](NOTES.md#d12--the-key-map-and-two-keys-deleted)).
- `⏎` details content in v1: full finding text + raw fields + the kubectl
  command + **which pods of the group are affected**.
- Rule 5 thresholds: restarts ≥3 WARN, ≥10 CRITICAL (suggestion).
- Rule 9 (no limits) is no longer an Alerts row at all — it is a column of the
  Capacity report. Grouping alone would not have saved it: "no limits" is not
  broken, and a cluster has hundreds of them.
- Large clusters (5k+ pods): the initial LIST is slow → show "loading N
  pods"; the findings list must scroll (missing from the sketch).

### Non-functional targets

- Memory: < 50MB RSS at ~1000 pods. First paint < 1s, findings < 3s.
- Coalesce redraws during event storms (min ~100ms debounce) — otherwise
  CPU spikes during rollouts. 0% CPU at idle is already decided.
- Minimum terminal 80×24; redraw on resize.
- Symbols: common Unicode only (`● ▲ ○`), no nerd fonts; a separate ASCII
  mode is YAGNI.
- Platforms: Linux + macOS; Windows best-effort.

### Architecture

- The flat layout is right, don't touch it — eight files after the reversal,
  still no `mod.rs` pyramid. First thing to write:
  `Finding { severity, title, evidence, action, kubectl_cmd, owner }` — the
  shared contract where three files meet. `owner` is the grouping key: the
  Deployment/StatefulSet/DaemonSet/Job behind the object, or the object itself
  when there is none.
- **Browser views carry no per-kind code:** `kube::discovery` enumerates what
  the cluster serves, server-side `Table` printing supplies the columns. CRDs
  must work without a line of code written for them.
- Rules do not return `Result`: missing field = no finding. anyhow only for
  main/k8s.rs startup errors. One exception: "403 vs no connection" changes
  the user message → a simple 2-variant enum suffices.
- Async: a single `tokio::select!` loop — (a) watcher stream, (b) crossterm
  `EventStream`, (c) Ctrl-C. NO separate UI thread / channel layer / actors.
- Tests: every rule gets a positive **and** a negative fixture (a healthy
  pod must trigger nothing — false-positive test). Fixtures are deserialized
  as `k8s_openapi::Pod` so the prune code is covered too.
- **Do not open the Events watch in v1** — rule 11 is v0.5 anyway (rule 10
  reads the pod's own `PodScheduled` condition and ships in v1,
  [NOTES § D27](NOTES.md#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)); the
  "second watch" section in NOTES blurred this. Noisiest stream in the
  cluster + the involvedObject join would be v1's most complex code.
- Don't write step 4 (learning ratatui) from scratch; start from the ratatui
  async template.
- Context selection: kubeconfig current-context; if `--context` is ever
  needed, one flag via `std::env::args` — still no clap.

## DevOps requirements

### CI (GitHub Actions)

- One workflow (PR + main): `fmt --check` → `clippy -D warnings` →
  `cargo test`.
- Tests must pass with no `KUBECONFIG` — clusterless CI.
- kind e2e job: **not in v1** (fixtures already come from kind; it would
  test the same thing 10 minutes more expensively). If the watch code ever
  regresses, add an optional `workflow_dispatch` job.
- `Swatinem/rust-cache` — one line, halves build time.
- Don't leave cross-compilation to the last minute: a `cargo check --target`
  matrix runs from day one (check only, not test — cheap).

### Build & release

- Targets: `x86_64-unknown-linux-musl` (static — actually keeps the
  "single binary" promise), `aarch64-unknown-linux-musl`,
  `x86_64/aarch64-apple-darwin`. No Windows binary in v1; `cargo install`
  covers it.
- Cargo.toml release profile: `lto = true`, `strip = true`,
  `codegen-units = 1` (~40% smaller). Try `opt-level = "z"`.
- Release flow: tag `v*` → git-cliff CHANGELOG → cross-compile matrix →
  `gh release create` + binaries + `SHA256SUMS`. Alternative: `cargo-dist`
  does all of it with one tool — weigh against 4 hand-written YAML jobs.
- **Publish the `k8rs` placeholder to crates.io** (name decided
  2026-08-11) — claiming a name is cheap, losing it is expensive.

### Versioning

- 0.x semver: rule-set changes are minor, finding-text fixes are patch.
- **The branch model in CLAUDE.md didn't specify commit message format** —
  git-cliff needs conventional-commit `feat:`/`fix:` prefixes, otherwise the
  changelog comes out empty. No commit-lint in CI needed — PR title
  discipline suffices.

### Dev environment

- `justfile` (not Makefile — Rust ecosystem norm, works on Windows too),
  ~6 targets: `cluster-up` (kind + broken.yaml), `cluster-down`, `fixtures`
  (capture + wait/retry for CrashLoop backoff + sanitization), `check`
  (fmt+clippy+test, byte-identical to CI), `run`.
- `broken.yaml` must be extracted from NOTES into
  `tests/manifests/broken.yaml` — YAML in a document drifts, a file gets
  applied.
- Fixtures are committed (test determinism); `just fixtures` regenerates,
  the diff shows up in PRs. When the k8s-openapi feature is bumped, fixtures
  must be re-captured against a matching kind version — write this as a
  comment in the justfile or it will be forgotten.

## DevSecOps requirements

- **Token hygiene:** kubeconfig/token is never logged, never rendered,
  never embedded in error messages (wrap the config type's `Debug`). Also
  applies to the panic hook — a backtrace dumped to stderr must not contain
  tokens.
- **No in-cluster ServiceAccount mode in v1** — nothing is deployed anyway;
  don't even open that code path.
- **The audit log is a security control, not a convenience.** Append-only,
  `~/.local/state/k8rs/audit.log`, mode 0600, one line per attempted
  mutation: timestamp · context · namespace · kind/name · equivalent kubectl
  command · **the real API call (verb, path, resourceVersion sent, dry-run
  verdict)** · result. The kubectl line is a teaching aid and is not what ran —
  recording only it would make the trail fiction
  ([NOTES § D8](NOTES.md#d8--invariant-4-was-not-literally-true)). It records
  refusals and failures too: an audit trail that only records successes cannot
  answer "what did they try".
- **The edit temp file is a leak surface.** A full object YAML can contain
  Secret data, env values and tokens. Mode 0600, in the user's own temp
  directory, removed on exit *and* on panic. Never written to a shared path.
- **Secret contents stay hidden by default.** Viewing a Secret shows keys and
  sizes; revealing a value takes an explicit second action, and revealed
  values never enter the command log, the audit log or the YAML shown by `y`.
- **`exec` and `port-forward` change the trust boundary** and are therefore
  last in the plan: exec hands the user's terminal to a container's process
  (control-character stripping does not apply to an interactive PTY), and
  port-forward opens a local listening socket. Both bind to loopback only,
  both are listed in the header while active, and both are disabled under
  `--read-only`.
- **`--read-only` must be structurally true**, not a UI condition: with the
  flag set, `ops.rs` is never called and the admin keys are not bound.
- **Never display env values** — event `.message` and `state.waiting.message`
  fields are free text; acceptable risk for v1 (the user already has RBAC to
  see their own data). A masking engine is YAGNI.
- **Terminal injection:** free text from the API may contain ANSI escapes
  (title spoofing / terminal corruption). ratatui's cell-based drawing
  protects mostly; requirement: strip control characters (a one-line filter).
- **No telemetry, no data leaves the machine** — state it explicitly in the
  README (part of the sales pitch).
- **Supply chain:** commit `Cargo.lock`; `cargo deny check` in CI
  (advisories + licenses + sources — covers `cargo audit`); deny.toml:
  reject copyleft + forbid non-crates.io sources; Dependabot
  (cargo + github-actions), weekly.
- **CI security:** top-level `permissions: contents: read` in workflows;
  `contents: write` only in the release job's own scope; third-party
  actions pinned to commit SHAs; `pull_request_target` + secrets is
  forbidden — the standard fork-PR trap.
- **Release signing (cosign/GPG) is YAGNI in v1** — `SHA256SUMS` suffices;
  add GitHub artifact attestation if it's one line, otherwise v2.
- **v4 DaemonSet trust model:** the plugin must be a **separate
  binary/image/repo**; deploy code never enters the main binary — otherwise
  the "read-only single binary" claim dies with one `--help` output.
- **Traffic adapter (v3):** the TUI's first connection outside the K8s API
  (Prometheus / Istio / Hubble). The address comes only from user config;
  **no** auto-discovery via pod annotations (SSRF / unintended targets).
  If a token is involved, token hygiene rules apply unchanged.
- Out of scope (YAGNI): SBOM, container image/image scanning (there is no
  image), SLSA provenance, seccomp/sandbox — security theater for a
  read-only CLI.

## Where these requirements get built

This file states *what* is required, never *when*. The ordered plan that
satisfies it lives in `todo.md` and only there.
