# k8rs — Claude Working Instructions

**lazygit for Kubernetes** — a single-binary Rust + ratatui TUI that tells you
what is broken in a cluster and why in language a beginner understands, and
lets them fix it without memorising long `kubectl` commands, showing the
command it ran every time.

> Scope was deliberately widened on 2026-08-11 (read-only → managed writes,
> browser, analysis reports). Read
> [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11)
> before acting on anything that predates it.

## Current phase: CODE — Phase 1, `feat/scaffold` (opened 2026-08-12)

- The design phase closed on 2026-08-12. Code and config files are now written,
  in the order [`todo.md`](todo.md) sets and nowhere else: the first unchecked
  box in the lowest open phase is the next task.
- Product code is still bottom-up and forward-only: `rules.rs` and
  `analysis.rs` come first, `main.rs` is wired last, and a file finished in an
  earlier step is frozen (CLAUDE.md § Architecture workflow).
- Phase 1 exists to make the guards real *before* there is anything to guard —
  and every guard must be seen red on a deliberate violation before it counts
  ([NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)).

## Source files — what lives where

| File | Owns | Never contains |
|---|---|---|
| [`NOTES.md`](NOTES.md) | Decision record — **why** every choice was made. Every decision gets recorded here. | The step list (that is `todo.md`) |
| [`REQUIREMENTS.md`](REQUIREMENTS.md) | **What** is required, per role (developer / devops / devsecops) | Ordering or scheduling |
| [`todo.md`](todo.md) | **The plan** — phases, order, done-when, what freezes after each. The only place steps are tracked and checked off. | Rules (those are here in CLAUDE.md) |
| [`docs/`](docs/README.md) | The **built** state, for humans outside this repo | Anything not yet true of the code |
| [`screens/`](screens/README.md) | **Screen mockups** — one file per screen: ASCII layout, keys, empty/error states. What the code has to match. | Decisions (NOTES) or steps (todo) |
| `CLAUDE.md` | Working rules for the agent — binding, always in effect | Plans, decisions, requirements |
| [`scripts/check-docs.py`](scripts/check-docs.py) | Verifies every relative Markdown link **and anchor** resolves | — |
| `tmp/` | Downloaded upstream library docs (kube-rs, ratatui, k8s) | — never committed |

Look it up before asking or re-deriving:

| Looking for | Go to |
|---|---|
| The v1 rule set (rules 1–11, severities, thresholds) | [NOTES § v1 rule set](NOTES.md#v1-rule-set) |
| Node rules, certificate rules, analysis reports | [NOTES § Node rules](NOTES.md#node-rules-n-series) · [§ Certificate rules](NOTES.md#certificate-rules-c-series--and-what-is-not-reachable) · [§ Analysis reports](NOTES.md#analysis-reports) |
| Why writes exist and what guards them | [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11) · [docs/security § Write safety](docs/security.md#write-safety-model) |
| The three views, keys, operations | [NOTES § The three views](NOTES.md#the-three-views) · [§ Operations](NOTES.md#operations--the-full-admin-surface) |
| What a screen actually looks like | [screens/](screens/README.md) — one file per screen |
| Audience, Alerts-vs-Analysis split, owner grouping, operation order | [NOTES § Design review, second pass](NOTES.md#design-review--second-pass-2026-08-11) |
| Why it is "lazygit for Kubernetes" | [NOTES § Positioning](NOTES.md#positioning--lazygit-for-kubernetes-user-2026-08-11) |
| File layout & why eight files | [NOTES § File layout](NOTES.md#file-layout) · [docs/architecture § Components](docs/architecture.md#components) |
| What is deliberately **not** built | [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section) · [docs/architecture § Out of scope](docs/architecture.md#out-of-scope) |
| Why this build order | [NOTES § Build order](NOTES.md#build-order--why-it-is-what-it-is) |
| Error states on first launch (no kubeconfig, 403, API down) | [REQUIREMENTS § Error states](REQUIREMENTS.md#error-states-all-were-undefined-all-happen-on-first-launch) · [docs/architecture § Error handling](docs/architecture.md#error-handling) |
| Write safety, RBAC, token hygiene, fixture sanitization | [docs/security.md](docs/security.md) · [REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements) |
| Crates, versions, toolchain, build targets | [docs/tech-stack.md](docs/tech-stack.md) · [NOTES § Dependencies](NOTES.md#dependencies) |
| Data flow (watch → prune → store → rules → UI) | [docs/architecture § Data flow](docs/architecture.md#data-flow) |
| Colors / theme constants | [docs/tech-stack § Visual identity](docs/tech-stack.md#visual-identity) |
| The broken-pod test manifest for kind | [NOTES § kind test manifest](NOTES.md#kind-test-manifest) |
| What to do next | [todo.md](todo.md) — first unchecked box in the lowest open phase |

## Language and documentation rules

- **Everything under `docs/` is written in English.**
- **`README.md` is written in English**; `README_TR.md` is its Turkish translation.
- **The application language is English:** everything in the TUI — finding
  texts, error messages, key hints, help screen — is written in English.
  Jargon is simplified (`OOMKilled` → "container exceeded its memory limit").
  No i18n (decided in NOTES, YAGNI).
- Code comments and commit messages are in English.
- **Every file in this repo is written in English — never write Turkish into
  a file.** Turkish is used only when talking with the user in conversation.
  Sole exception: `README_TR.md`, which is by definition the Turkish
  translation of README.md.
- **A structural change is not done until the docs match it.** Anything that
  changes file layout, architecture, dependencies, the rule set, the key map
  or the CLI surface must update, in the *same* change: `docs/` (the affected
  file), `README.md`, and `README_TR.md` if the README changed. Stale docs
  count as a failed step — do not commit them.

## Architecture workflow

- **Read first, draw second, write third:** before touching the architecture,
  pull the relevant docs (kubernetes / kube-rs / ratatui) into `tmp/` and
  read them.
- **Pyramid phases, bottom-up, forward-only:** each phase is the foundation
  of the next, and todo.md steps only move forward. A step may create new
  files or shape the current top layer; **files finished in earlier steps
  are frozen** — step 3 never reaches back into a file built in step 1.
  If a later step would require changing a frozen file, the code is not the
  problem, the plan is: stop, fix the plan/order (record the change in
  NOTES.md), then continue. This is what keeps the foundations solid.
  Layer order, bottom → top: `rules.rs` → `analysis.rs` → `k8s.rs` →
  `ops.rs` → `theme.rs` → `views.rs` → `ui.rs` → `main.rs` (main is the
  top; it is the only file still being wired at the end). The two pure files
  come first because they need neither a cluster nor a terminal to be proven. Learning spikes
  (e.g. ratatui experiments) go into `examples/` as throwaway code and never
  touch product files.
- **Dangerous code is proven before it is bound to a key.** `ops.rs` sits low
  in the pyramid precisely so every write is verified headlessly against kind
  first; by the time a keypress calls it, it already works.
- **Single point of change:** directory layout and style stay uniform; one
  change should propagate through the whole project from one place
  (e.g. theme = single file `theme.rs`).
- File layout is decided in NOTES: `main.rs / k8s.rs / ops.rs / rules.rs /
  analysis.rs / views.rs / ui.rs / theme.rs` — eight files, no mod pyramid.

## Hard invariants — never break one without an explicit decision

Breaking one of these is not a bug to fix later, it is a stop. If a task seems
to require it, the plan is wrong: fix the plan, record the reversal in
[NOTES.md](NOTES.md), then continue.

1. **Writes live in `ops.rs` and nowhere else.** Enforced as an **allowlist**:
   outside `ops.rs` only `get*` / `list*` / `watch*` / `logs` / `log_stream` /
   `apiserver_version` may appear. Never write a denylist here — `Api` also
   has `cordon`, `uncordon`, `restart`, `evict`, `exec`, `portforward`,
   `entry`, `patch_scale` and more, and that list grows upstream.
   `clippy.toml` carries the ban crate-wide with `-D warnings`; `ops.rs`
   carries the single visible `#![allow(clippy::disallowed_methods)]`.
   The allowlist is **mechanical and stays that way**: `may_i(...)` — the
   permission check — lives in `ops.rs` despite mutating nothing, because it
   is performed with `create` and the alternative was an allowlist clause
   requiring judgement ([NOTES § D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)).
   *(Replaced the original "read-only, always" on 2026-08-11 — see
   [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11).)*
2. **No write is implicit.** Every mutation requires: an explicitly selected
   object → a keypress → a confirmation dialog stating the consequence in
   plain language → a server-side `dryRun=All` where the API supports it →
   an audit line. Deletes and drains additionally require typing the object
   name. `--read-only` must make the whole path unreachable, not merely
   unbound. Bulk mutation does not exist.
3. **Nothing is deployed into the cluster.** k8rs runs on the user's machine
   against their kubeconfig, and that is the entire trust model. This is now
   the *only* structural guarantee left — it does not get traded away.
4. **Every mutation is visible twice, and neither record may lie.** The
   **command log** shows the *equivalent* kubectl command, as the user would
   have typed it — the teaching device. The **audit log** records that line
   *and* the real call (verb, path, resourceVersion sent, dry-run verdict,
   result) — the trail. They are separate because they are not the same fact:
   k8rs runs `Api::patch_scale`, not `kubectl scale`. A mutation missing from
   either is a bug, not an optimisation
   ([NOTES § D8](NOTES.md#d8--invariant-4-was-not-literally-true)).
5. **Rules are pure functions:** `analyze(&Snapshot) -> Vec<Finding>` — no
   network, no terminal, no globals, no `Result` (a missing field means no
   finding), and **no clock call**: `Snapshot` carries `now`, captured once by
   the caller, so "expires in 30 days" reads a field instead of asking the
   system what time it is. Findings carry timestamps; the *renderer* turns one
   into "4 min ago" ([NOTES § D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)).
   Same for the `analysis.rs` reports. This is what makes them testable
   against fixtures — and what keeps a fixture from expiring.
6. **Watch, never poll-list.** LIST once then stream changes, pruned with
   `managedFields` dropped. **The Alerts view's own inputs are watched
   permanently** — Pods, Nodes, and Deployments/StatefulSets/DaemonSets
   (metadata + status only, [NOTES § D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12));
   ReplicaSets are fetched on demand, never watched; browser kinds are watched
   only while their view is open. A periodic
   `LIST pods -A` is the exact thing that makes k9s heavy —
   [NOTES § Architecture](NOTES.md#architecture--where-lightweight-comes-from).
7. **No fixed FPS.** Draw on events, coalesce ~100ms during storms, block when
   idle → 0% CPU idle.
8. **The panic path restores the terminal** *and* leaks no credential: a
   backtrace on stderr must never contain a token —
   [docs/security § Token hygiene](docs/security.md#token-hygiene). Same for
   the temp file behind `e` edit: mode 0600, removed after use.
9. **Free text from the API is untrusted.** Strip control characters before it
   reaches the screen, or a crafted pod name rewrites the user's terminal.
10. **No new dependencies without asking.** The ten allowed crates: `kube`,
    `k8s-openapi`, `ratatui`, `crossterm`, `tokio`, `anyhow`, `serde_json`,
    plus `serde_yaml_ng`, `x509-parser`, `similar` (added by the reversal,
    each with a recorded reason). `similar` arrives only in v0.4 with `edit`
    — approved is not the same as present. No `clap` while the flags are
    `--read-only` / `--context` / `--namespace` / `--once`; revisit only when
    a flag needs validation or a subcommand appears. No `tracing` until
    debugging demands it ([NOTES § Dependencies](NOTES.md#dependencies)).
11. **Eight files, flat.** `main.rs / k8s.rs / ops.rs / rules.rs /
    analysis.rs / views.rs / ui.rs / theme.rs` — no `mod.rs` pyramid, no trait
    layer, no plugin system. A ninth file needs the same kind of boundary
    argument the eighth had. Exactly one is pre-approved: `dialog.rs`, if
    `ui.rs` passes ~800 lines
    ([NOTES § D11](NOTES.md#d11--the-ninth-file-pre-approved)). Everything
    beyond that still needs the argument made first.
12. **No per-kind code in the browser.** Resource views come from API
    discovery and server-side `Table` printing; typed structs exist only where
    the rule engine needs them. A hand-written column list for a kind is a
    design failure, not a feature.
13. **Before adding a feature, read
    [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section)
    first.** Scope creep is the named number-one risk and it has already been
    realized once, deliberately. The guard, both halves required: *would
    someone who **runs clusters** use this in a normal week — and can a
    newcomer read the resulting screen without a glossary?* If either fails,
    the answer is no — unless the user reverses it explicitly, and a reversal
    gets written into NOTES, not applied silently.
14. **Plain language is a hard rule, not a style preference.** Every string a
    user can see — column header, dialog, error, finding — is written for
    someone who does not yet know the jargon. `CrashLoopBackOff` gets
    explained, not printed and left.

## Security gate — run this list on every change, no exceptions

Not a phase, not a review milestone: this runs on every diff, including
"just a UI tweak". Anything unchecked here is a red build, not a follow-up
ticket. The reasoning behind each item lives in
[docs/security.md](docs/security.md) and
[REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements).

**Identity and transport**

- [ ] Credentials come from the kubeconfig current context and nowhere else.
      No in-cluster ServiceAccount path exists — do not open one.
- [ ] TLS verification is never disabled by us. If the user's kubeconfig sets
      `insecure-skip-tls-verify`, it is honoured *and shown in the header* —
      silently trusting a MITM-able connection is not acceptable in a tool for
      beginners.
- [ ] The token is never copied out of the kube client into our own structs,
      never logged, never rendered, never put in an error message. Any type
      that can hold config has a wrapped `Debug`.

**Authorization**

- [ ] Least privilege holds: the documented read-only role runs everything
      except the operations. A 403 degrades that one feature and names the
      missing verb + resource; it never crashes and never retries in a loop.
- [ ] `--read-only` is structurally true — `ops.rs` unreachable, keys unbound.

**The write path**

- [ ] Mutations exist only in `ops.rs` (allowlist check, invariant 1).
- [ ] Dry-run precedes the real call wherever the API supports it.
- [ ] Destructive actions require the typed object name.
- [ ] Applies carry the resourceVersion that was read; a 409 offers a re-read,
      never a blind overwrite.
- [ ] No bulk mutation, no operation without a selected object.
- [ ] Every attempt — success, failure, refusal — reaches the audit log.

**Untrusted input from the API** *(the class that gets forgotten)*

- [ ] Every free-text field (names, messages, annotations, log lines) is
      stripped of control characters before it reaches the screen.
- [ ] **No API string is ever interpolated into a shell.** `$EDITOR` is spawned
      with an argument vector, never a command string. A pod named
      `; rm -rf ~` must be boring.
- [ ] The command log is display text. k8rs does not execute it, and nothing
      in it is fed back into a process.
- [ ] Object names are not used to build filesystem paths without sanitising —
      `../` in a name must not escape the temp directory.
- [ ] Sizes are bounded: a 50MB annotation or an endless log line must not be
      held whole in memory or blow up the renderer.

**Secrets and local files**

- [ ] Environment variable values are never displayed. Secret values require
      an explicit reveal and never enter the command log, the audit log, or
      the YAML shown by `y`.
- [ ] *(from v0.4, when `edit` lands — nothing before it writes a temp file)*
      The edit temp file is mode 0600, in the user's own temp dir, and removed
      on exit *and* on panic.
- [ ] The audit log is mode 0600 and append-only.
- [ ] The panic path leaks nothing: a backtrace on stderr contains no
      credential, and the terminal is restored.
- [ ] No telemetry. The only outbound connection is the API server in the
      user's kubeconfig (plus, later, an endpoint the user typed themselves —
      never one discovered from cluster annotations).

**Supply chain and release**

- [ ] `cargo deny check` passes (advisories, licenses, sources); `Cargo.lock`
      committed; no non-crates.io source.
- [ ] A new dependency is a recorded decision (invariant 10), not a reflex.
- [ ] Workflows default to `permissions: contents: read`; third-party actions
      pinned to commit SHAs; no `pull_request_target` with secrets.
- [ ] Releases ship `SHA256SUMS`.

**Test data**

- [ ] Fixture sanitization ran *before* the fixture was committed — no
      annotations, no env values, no node identifiers, no real certificates,
      keys or tokens. A leak never leaves git history.

## Code phase rules (apply once it starts)

- **Write function-based** — never write the same code twice; extract shared
  functions.
- **Security in every change:** the DevSecOps requirements in REQUIREMENTS
  (read-only guarantee, token hygiene, fixture sanitization) are binding.
- **Tests:** rules are written with fixture-based unit tests; everything
  testable gets a test step.
- **Tests must not lie.** A test that cannot fail is not a test:
  - Every rule gets a positive fixture (triggers the finding) **and** a
    negative fixture (healthy pod → no finding, catches false positives).
  - Fixtures come from real cluster captures, not hand-written JSON
    (hand-written JSON resembles reality; it is not reality).
  - Never weaken, skip, or delete a failing test to make the build green —
    a failing test means the code or the plan is wrong, fix that instead.
  - Never assert what the implementation happens to return; assert what the
    requirement says it must return.
  - **Seen red before trusted.** Every new test or guard is run against the
    code *before* the fix and watched fail, then watched pass after. One that
    has only ever been green proves as much as an empty file does. This is not
    a Phase 1 ritual that ended with Phase 1 — it applies to every test added
    from here on.
  - **A check is proven only for the input shapes it was fed.** Before
    trusting one, list every shape the real pipeline hands it and feed it each.
    The fixture sanitizer was tested on a single Pod and was a near no-op on
    every `kubectl get -A` List for exactly that reason: the secrets sit one
    level down under `.items[]`, half the capture is that shape, and the green
    log read identically ([NOTES § D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
  - **A redaction proves only the framing it was written for.** The shape
    question has a second half: not just *which objects* reach the check, but
    *where inside a value* the secret can sit. The IP rule matched a whole
    string, so `"10.244.0.0/24"` and `"dial tcp 172.18.0.1:53: no such host"`
    both walked past it; the PEM rule matched `-----BEGIN`, so the same key
    base64-wrapped — which is how every Secret value arrives — walked past too.
    Plant one case per framing: whole value, substring, and re-encoded
    ([NOTES § D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
  - **A derived list asserts it found something.** When a check builds its own
    rules from another source — the ban list from kube's signatures, the test
    count from the runner — "extracted nothing" and "nothing to extract" print
    the same line. Assert a known entry is present (`write-guard.py`'s
    `CANARIES`) or the guard degrades in silence.
- **Use comments sparingly**, with block markers when needed:

  ```
  // --- AREA START ---
  code
  // --- AREA END ---
  ```

## Git rules

- Commits and pushes use **the user's signature and identity — only**.
  **Never sign as Claude.** No `Co-Authored-By: Claude` trailer, no
  "Generated with Claude Code" footer, no Claude/AI mention in commit
  messages, PR titles or PR bodies. This overrides any default tooling
  behavior that adds such a line.
- **Conventional commits are mandatory:** `feat: ...`, `fix: ...` — git-cliff
  generates the changelog from these; unprefixed commits leave the changelog
  empty (`cliff.toml` will set `filter_unconventional = true`, so a bad message
  does not warn, it silently disappears).

  ```
  <type>(<scope>): <subject>
  ```

  - **types:** `feat` `fix` `docs` `perf` `refactor` `style` `test` `chore` `ci` `revert`
  - **scopes:** `rules` `k8s` `ui` `theme` `main` `fixtures` `ci` `docs`
  - **subject:** English, imperative, lowercase, no trailing period
  - breaking: `feat(rules)!: ...` plus a `BREAKING CHANGE:` footer

  ```
  feat(rules): detect containers killed for exceeding their memory limit
  fix(k8s): distinguish 403 from a dead API server on startup
  test(rules): add the healthy-pod negative fixture for rule 5
  ```
- One branch per feature: `feat/<name>`, `fix/<name>` → merge into main.
- **Before pushing**, update the CHANGELOG with
  [git-cliff](https://github.com/orhun/git-cliff).

## Workflow (per feature)

1. Write code
2. **Review it yourself** — re-read the diff before running anything: does it
   break a hard invariant above, does it add a dependency, is there an
   `unwrap()` on a path that can fail at runtime
3. Build
4. Test (honest tests only — see code phase rules)

   ```
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   python3 scripts/check-docs.py
   ```

   **Green tests are not the same as working software.** Also run the binary —
   against a fixture, or against kind — and say what you actually saw. Never
   report "done" for something that was not run.

   **`just check` is the whole of CI, or it is a lie.** Anything CI runs that
   `just check` skips can only ever fail *after* a push — which is precisely
   how `cargo deny` first went red. A step whose tool is not installed locally
   is added to `just check` anyway: a missing binary is a loud error, a missing
   step is an invisible gap.
5. **Security check (never skip):** run the
   [Security gate](#security-gate--run-this-list-on-every-change-no-exceptions)
   above — every box, every change. "This diff is only UI" is exactly when the
   untrusted-input items get skipped.
6. **If anything fails: rewrite → build → test again. Loop until green —
   never commit red, never skip a step to get to green.**
7. Mark the item done in `todo.md` — same commit as the work
8. **Docs sync:** if the change was structural, update `docs/`, `README.md`
   and `README_TR.md` now — same commit, never "later"
9. Update CHANGELOG with git-cliff, committed separately as
   `chore(changelog): update`
10. git commit & push (with the rules above) — one push at the end, not two

## Phase close — the ritual at the end of every phase

A phase is not "mostly done". It closes, or it is still open. When the last
box of a phase in [`todo.md`](todo.md) is about to be checked, run this whole
list — in order, no skipping:

1. **`just check` green**, and the binary actually run (a fixture, or kind) —
   green tests are not the same as working software.
2. **Every box of the phase is checked, and every check is true.** A box
   checked for work that was written but never *run* is a lie in the one file
   the plan is read from. If something could not be proven, leave the box open
   and say why in the item — an honest open box beats a false tick.
3. **The phase's own security gate** in todo.md, item by item.
4. **Docs sync:** `docs/`, `README.md`, `README_TR.md` for anything
   structural. Stale docs are a failed step, not a follow-up.
5. **CHANGELOG** with git-cliff, committed separately.
6. **Commit, push, PR, CI green, merge.** Frozen files stay frozen from here.
7. **Then say, in the reply, that the phase is closed and the context should be
   cleared** — name the phase, name what the next one starts with. Clearing is
   the user's command (`/clear`); the agent cannot issue it and must not
   pretend a fresh context happened on its own. The next phase starts by
   reading `todo.md`'s first unchecked box, which is exactly why the previous
   phase's chatter is not needed to continue.

Reference workflows: [titus-ai](https://github.com/ChrisTitusTech/titus-ai),
[christitus.com/my-ai-workflow](https://christitus.com/my-ai-workflow/)
