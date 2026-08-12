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
- **All work happens on `development`** — one long-lived branch, never deleted
  ([NOTES § D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).
  `main` only ever advances by merging `development` into it, at phase close,
  by the ritual in
  [phase close, item 7](#phase-close--the-ritual-at-the-end-of-every-phase) —
  described there and nowhere else. Never commit directly to `main`: the
  moment that happens, `development` stops being an ancestor of it and the
  merge stops being clean.
- **Before pushing**, update the CHANGELOG with
  [git-cliff](https://github.com/orhun/git-cliff).

## Second pass — nothing is delivered on its first draft

**Everything produced here is reviewed a second time before it is handed
over** — code, a doc section, a plan, a rule, a commit message, an answer.
Not "if it feels risky": always, and by default, without being asked.

The second pass is not a re-read from memory. **Open the artifact as written**
and go through it as its first reader would — someone who was not in the room
while it was being made, and who will follow it literally.

What the second pass is actually hunting, in this order:

1. **Does it contradict itself?** Two sentences that cannot both be obeyed.
   This is the most common defect in anything longer than a page and the
   hardest to see while writing, because both halves felt right in isolation.
2. **Can every rule in it actually be complied with?** A gate nobody can pass,
   a step that needs a file that does not exist yet, a check that needs a
   machine nobody has. An impassable rule does not get followed carefully —
   it teaches everyone that rules here are decorative.
3. **Does everything in it have an owner?** Every file, every step, every
   decision. "Someone will do it" means two people do it in the same turn, or
   nobody does.
4. **The unhappy path.** Empty, missing, denied, too large, already exists,
   the human is not available. First drafts describe the case that works.
5. **What does it silently assume?** The assumption that was obvious while
   writing is invisible to the reader, and it is where the bug lives.

**A second pass produces findings or it names what it checked.** "Looks good"
is not a second pass — it is the first pass claiming to be the second. If it
genuinely found nothing, say what was examined and what would have failed it;
an empty review that lists nothing it looked at proves nothing, exactly like a
test that has only ever been green.

Findings from the second pass are fixed **before** delivery, in the same turn,
not filed as follow-ups. Then say what the second pass changed — the user sees
the review, not just its output.

## Workflow (per feature)

1. Write code
2. **Review it yourself** — the [second pass](#second-pass--nothing-is-delivered-on-its-first-draft)
   above, applied to the diff: does it break a hard invariant, does it add a
   dependency, is there an `unwrap()` on a path that can fail at runtime
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

## Agent workflow — who runs each step, and who is accountable

Five subagents live in `.claude/agents/`, committed — the sections below
describe people who have to be in the room, and a clone that lost them is a
clone that cannot follow this file. **The main session is the project
manager.** Agents do not
talk to each other, do not commit, do not push, and do not check a box in
`todo.md`. Every handoff goes through the PM, so there is exactly one place a
lie can be caught.

### Ownership — and the file each one may write

Every path in the repo appears in exactly one **Writes** cell. A file with no
owner is written by whoever happens to be holding the pen, which is how two
agents end up editing it in the same turn.

| Agent | Writes | Never writes |
|---|---|---|
| `dev-core` | `rules.rs` `analysis.rs` `k8s.rs` `ops.rs` | anything that draws |
| `dev-ui` | `theme.rs` `views.rs` `ui.rs` `main.rs`, `examples/` (the Phase 8 spike) | the four lower files |
| `tui-designer` | `screens/` | any `.rs` |
| `tester` | `tests/` `scripts/` `justfile` `clippy.toml` `deny.toml` `.github/workflows/` | product code in `src/` |
| `k8s-admin` | nothing — reads everything, reports | every file |
| **PM** (main session) | `todo.md` `NOTES.md` `docs/` `README.md` `README_TR.md` `CHANGELOG.md` `Cargo.toml` `Cargo.lock` `cliff.toml` `CLAUDE.md` `.claude/agents/`, branches, commits, PRs | `src/` (delegate it) |

Phase map, from [`todo.md`](todo.md): **2** → `tester` · **3–7** → `dev-core` ·
**8–12** → `dev-ui` · **13** → PM. `tui-designer` and `k8s-admin` have no
phases of their own; they are gates on other people's.

`Cargo.toml` sits with the PM on purpose: a dependency is a recorded decision
(invariant 10), and the agent that wants the crate is the last one who should
be able to add it.

### The boxes no agent can run — say so, do not fake them

Some boxes need a machine, a credential or an account the agents do not have:
the kind cluster on the LAN host (`just cluster-up`, `just fixtures`, `just
e2e`), the crates.io publish, GitHub repo settings, anything behind a login.
The PM does **not** improvise around these and does not check the box. It
prints the exact command for the user to run and waits for the real output.
A box whose evidence is "this would work" is an unchecked box.

### The one hard rule of concurrency

**One writer per file tree at a time.** Two agents editing the same tree in the
same worktree corrupts both diffs and neither notices.

This costs less than it sounds, because the pyramid already serialises it: the
lower four files are phases 3–7 and the upper four are 8–12, so `dev-core` and
`dev-ui` are *never* both writing product code. If a task appears to need both
at once, that is a plan error (forward-only rule), not a throughput problem —
stop, fix the order, record it in `NOTES.md`.

What may genuinely run at the same time — always at most one writer per row:

| Safe together | Because |
|---|---|
| one dev writing `src/` · `tester` writing `tests/`, `scripts/` | disjoint trees |
| one dev writing · `tui-designer` on a **later** phase's screen | `screens/` is not code |
| two reviewers (`k8s-admin` + `tui-designer`) on the same diff | neither writes |
| one dev writing · `k8s-admin` auditing an **already merged** phase | the audit lands as findings, not as an edit |

Anything not in that table runs one at a time. Worktree isolation
(`isolation: "worktree"`) exists if two writers are ever genuinely unavoidable
— but reach for the plan fix first.

**Review is not one of the parallel slots.** `k8s-admin` reviews the box that
is in front of it and nothing is built on top until it reports: work stacked on
an unreviewed box turns a rejection into a rebase, and a rebase under time
pressure is how a finding gets quietly dropped. The dev idles during review.
That idle is the price of the gate meaning something.

### The cycle — one `todo.md` box is one turn of it

The box is the unit of work, never a phase and never "the next few boxes".

| # | Step | Who | Gate to pass |
|---|---|---|---|
| 1 | Read the box, decide the owner, write the brief | PM | the box is the *first unchecked one in the lowest open phase* — no cherry-picking |
| 2 | Screen spec, **only if a screen changes** | `tui-designer` | the mockup covers every state, not just the happy one |
| 3 | Write the code **and its tests together** | `dev-core` / `dev-ui` | invariants; forward-only; no new dependency |
| 4 | Witness red, then green | `tester` | **reverts the implementation and re-runs** — see below |
| 5 | Full run | `tester` | `just check` green **and** the code exercised for real — see below |
| 6 | Operator review | `k8s-admin` | blocking for `rules.rs` `analysis.rs` `ops.rs` `k8s.rs`, any dialog, any kubectl line; skippable only for formatting |
| 7 | Second pass on the diff, security gate, docs sync, check the box, git-cliff, commit, push | PM | every box of the [Security gate](#security-gate--run-this-list-on-every-change-no-exceptions), and the [second pass](#second-pass--nothing-is-delivered-on-its-first-draft) named what it checked |

Steps 4–6 loop back to 3 on any failure. Nothing is negotiated down to get past
a gate — that is the whole reason the gates are held by someone other than the
author.

**"Run it" means something different before `main.rs` exists.** `main.rs` is
wired last, so demanding a binary run in Phase 3 sets a gate nobody can pass,
and an impassable gate teaches everyone to wave gates through. Until Phase 5
wires the binary, the real run is the test binary over a captured fixture,
printed and read: `cargo test -- --nocapture`, and the finding text quoted in
the report. From Phase 5 on it is the actual binary, against a fixture or kind.
Either way something ran and its output is in the report — that part never
relaxes.

**When the reviewer and the author disagree, the PM decides, in writing.** A
finding is closed one of two ways: fixed, or rejected with the reason recorded
in `NOTES.md`. "The dev said it is fine" is not a resolution — an unrecorded
rejection is a finding that will be rediscovered in six months with no memory
of why it was allowed.

**Branches: there is one, `development`, and it is always there.** Every box
commits onto it, whoever wrote the box; the PR to `main` opens **and is merged**
at phase close, not per box — the ritual is
[phase close, item 7](#phase-close--the-ritual-at-the-end-of-every-phase), and
it lives there only, so there is one description of it to keep true. Agents
never create, switch, merge or delete a branch — they write files on
`development` and that is the whole of their git surface
([NOTES § D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).

Work that is not a phase — a fix, a docs change, this file — goes on
`development` too, and reaches `main` with the next phase close unless the PM
has a reason to merge it sooner.

### Step 4 is the anti-leak mechanism, so it is mechanical

"I saw it fail" is a claim. `tester` does not accept it, it reproduces it:
stash or comment out the implementation body, run the test, watch it fail,
restore, watch it pass — and pastes **both** outputs. A test that stays green
with the implementation removed tests nothing, and that is exactly the failure
[NOTES § D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12) is
about. This applies to guards in `scripts/` the same way.

### The brief the PM hands out, and the report it gets back

The brief, five lines, no more: the box verbatim · the files you may write ·
what "done" means for this box · which `NOTES.md` section decides the
behaviour · what is explicitly out of scope this turn.

The report, or the work is not received: what changed and where · the exact
commands run and their real output · the red run and the green run · what could
not be proven and why · anything the agent wanted to touch outside its
ownership · **every choice it had to make that the brief did not decide** ·
what its own [second pass](#second-pass--nothing-is-delivered-on-its-first-draft)
found and changed. **No output pasted, no completion.** An agent reporting
"done, tests pass" without the terminal text is sent back, not trusted, and so
is one whose second pass found nothing and cannot say what it looked at.

That last item is the one that goes missing. An agent that picked a threshold,
named a field, or settled a behaviour the docs did not settle has made a
decision, and this project records decisions in `NOTES.md` or it does not have
them. The PM writes it there before committing — not the agent, so the wording
stays in one voice, and not "later", because later is a decision nobody can
reconstruct.

### Where a leak would actually happen — the PM checks these by hand

- A box checked for work that was written but never *run* (phase-close item 2).
- A test that has only ever been green — step 4 skipped because the change
  "obviously works".
- The security gate skipped because "this diff is only UI" — which is precisely
  when the untrusted-input items get missed.
- An agent editing outside its ownership row, quietly. The diff is the
  evidence: PM reads it before committing, every time.
- Docs left stale after a structural change — a failed step, not a follow-up.
- The second pass skipped because the change was small — small changes are
  where it is cheapest to run and where nobody is watching.

## Phase close — the ritual at the end of every phase

A phase is not "mostly done". It closes, or it is still open. When the last
box of a phase in [`todo.md`](todo.md) is about to be checked, run this whole
list — in order, no skipping:

1. **`just check` green**, and the code actually exercised — the binary against
   a fixture or kind once Phase 5 has wired one, the test binary read with
   `--nocapture` before that ([the cycle](#the-cycle--one-todomd-box-is-one-turn-of-it),
   step 5). Green tests are not the same as working software, and a gate that
   cannot be passed yet is not one either.
2. **Every box of the phase is checked, and every check is true.** A box
   checked for work that was written but never *run* is a lie in the one file
   the plan is read from. If something could not be proven, leave the box open
   and say why in the item — an honest open box beats a false tick.
3. **The phase's own security gate** in todo.md, item by item.
4. **[Second pass](#second-pass--nothing-is-delivered-on-its-first-draft) over
   the whole phase, not box by box.** The per-box passes each saw one diff;
   this one reads the phase as a stranger would read it end to end, which is
   the only place the cross-box defects live: two boxes that solved the same
   problem differently, a decision made in box 3 that box 9 quietly violated, a
   gate that stopped being passable halfway through, an assumption nobody
   wrote down. Findings are fixed before the phase closes — a phase does not
   close with a known gap in it.
5. **Docs sync:** `docs/`, `README.md`, `README_TR.md` for anything
   structural. Stale docs are a failed step, not a follow-up.
6. **CHANGELOG** with git-cliff, committed separately.
7. **Commit, push, PR `development` → `main`, and merge it — the PM does this,
   it is not handed back.** Standing authorisation: nobody is asked before a
   green PR closing the current phase is merged. In order: push `development` ·
   open the PR · wait until **every** check has *reported* (a pending check is
   not a green one) · merge with a merge commit, so the phase stays one
   readable block in `git log` · **stay on `development`** — it is not deleted
   and the next phase continues on it
   ([NOTES § D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).
   Never on red, never mid-run, never force past a conflict — a clean merge is
   the proof that nothing was committed to `main` behind the branch's back, so
   a conflict is a question to answer, not an obstacle to push through. If the
   tooling refuses a step, print the exact command for the user
   ([§ the boxes no agent can run](#the-boxes-no-agent-can-run--say-so-do-not-fake-them))
   rather than leaving the phase half-merged. Frozen files stay frozen from
   here — and with one shared branch nothing but the diff you read enforces
   that, so read it.
8. **Then say, in the reply, that the phase is closed and the context should be
   cleared** — name the phase, name what the next one starts with. Clearing is
   the user's command (`/clear`); the agent cannot issue it and must not
   pretend a fresh context happened on its own. The next phase starts by
   reading `todo.md`'s first unchecked box, which is exactly why the previous
   phase's chatter is not needed to continue.

Reference workflows: [titus-ai](https://github.com/ChrisTitusTech/titus-ai),
[christitus.com/my-ai-workflow](https://christitus.com/my-ai-workflow/)
