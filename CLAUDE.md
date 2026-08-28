# k8rs — Claude Working Instructions

**lazygit for Kubernetes** — a single-binary Rust + ratatui TUI that tells you
what is broken in a cluster and why in language a beginner understands, and
lets them fix it without memorising long `kubectl` commands, showing the
command it ran every time.

> Scope was widened on 2026-08-11 (read-only → managed writes, browser,
> analysis reports). Read
> [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11)
> before acting on anything that predates it.

This file is the working rules — binding, always in effect. It holds no plans,
no decisions and no requirements; those have their own files, below.

## What to do next

The next task is the **first unchecked box in the lowest open phase** of
[`todo.md`](todo.md), and nowhere else. "Lowest open" can be an earlier phase a
decision deliberately left open underneath a later one, so read the file rather
than assuming the highest-numbered phase is the only one running
([D33](NOTES.md#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12) ·
[D47](NOTES.md#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12)).

All work is on `development`. Product code is bottom-up and forward-only, and
a file finished in an earlier step is frozen.

**A box is never added to an open phase.** Work found mid-phase — a review
finding, a prior-art gap, an idea — is recorded where it belongs and boxed in a
*later* phase, so the phase that is running can converge. Twelve boxes were
injected into a running Phase 3 on 2026-08-14 and the phase stopped closing
([D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)).
The one exception is a defect in the box currently being landed: that is the
same box, not a new one.

## Every file here also has to get smaller

Every other rule in this file ends in *record it*. None of them made anything
shorter, and the two files every agent must read — `NOTES.md` and `todo.md` —
are the two that every box grows. That is why a box came to cost two hours when
the gate that proves it costs forty seconds
([D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)).
Three rules. The first and third are the PM's, at step 7; the second binds
whoever is writing — a dev at step 3 as much as the PM at step 7:

- **`todo.md` holds boxes, not history.** A box says what to do and how it is
  known to be done. When it closes it keeps its title and its `NOTES.md`
  links — the round-by-round story of how it got there does not go back onto
  it, because every claim in that story cites a decision that already holds it.
- **A decision is written once, in `NOTES.md`, and cited everywhere else.** A
  doc comment, a box, a report: they link `D##`, they do not restate it. The
  second copy is the one that goes stale, and it is never the one that gets
  fixed.
- **`NOTES.md`'s index is part of the entry.** A new `### D##` heading lands
  with its line in [§ Decision index](NOTES.md#decision-index) in the same edit.
  `scripts/check-docs.py` fails on a line whose anchor stopped resolving, so a
  renamed heading is caught; a heading added with no line at all is not, and
  that one is on the PM.

## Where to look

| Looking for | Go to |
|---|---|
| Which file holds X, who may write it, what to touch for a change | [docs/maps.md](docs/maps.md) |
| **Why** a choice was made — every decision, numbered `D1…` | [NOTES § Decision index](NOTES.md#decision-index) first — one line each; open the one section you need, never the whole file |
| **What** is required, per role (developer / devops / devsecops) | [REQUIREMENTS.md](REQUIREMENTS.md) |
| **When** — phases, order, done-when. The only place steps are checked off | [todo.md](todo.md) |
| Work that belongs to **no phase yet** — a finding nobody has ruled on, an idea with no home | [backlog.md](backlog.md) — nothing in it is work; read at phase close, never mid-phase ([D108](NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)) |
| A **measurement** somebody already made — commands and their real output | [reports/](reports/README.md) — evidence a decision cites, never the decision |
| The **built** state, for humans outside this repo | [docs/](docs/README.md) — never contains anything not yet true of the code |
| What a screen actually looks like, key by key | [screens/](screens/README.md) — one file per screen |
| The v1 rule set (rules 1–15 — **9 became a Capacity row and 11 waits on the Events watch in v0.5, so neither is a v1 rule** — severities, thresholds) | [NOTES § v1 rule set](NOTES.md#v1-rule-set) |
| Node rules · certificate rules · analysis reports | [§ N-series](NOTES.md#node-rules-n-series) · [§ C-series](NOTES.md#certificate-rules-c-series--and-what-is-not-reachable) · [§ Analysis reports](NOTES.md#analysis-reports) |
| What is deliberately **not** built | [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section) · [docs/architecture § Out of scope](docs/architecture.md#out-of-scope) |
| Why this build order | [NOTES § Build order](NOTES.md#build-order--why-it-is-what-it-is) |
| The three views, keys, operations | [NOTES § The three views](NOTES.md#the-three-views) · [§ Operations](NOTES.md#operations--the-full-admin-surface) |
| Why writes exist and what guards them | [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11) · [docs/security § Write safety](docs/security.md#write-safety-model) |
| Error states on first launch (no kubeconfig, 403, API down) | [REQUIREMENTS § Error states](REQUIREMENTS.md#error-states-all-were-undefined-all-happen-on-first-launch) · [docs/architecture § Error handling](docs/architecture.md#error-handling) |
| Write safety, RBAC, token hygiene, fixture sanitization | [docs/security.md](docs/security.md) · [REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements) |
| Crates, versions, toolchain, build targets, colours | [docs/tech-stack.md](docs/tech-stack.md) · [NOTES § Dependencies](NOTES.md#dependencies) |
| Data flow (watch → prune → store → rules → UI) | [docs/architecture § Data flow](docs/architecture.md#data-flow) |
| The broken-pod test manifest for kind | [NOTES § kind test manifest](NOTES.md#kind-test-manifest) |
| How a tool of this shape breaks — k9s's tracker read as a defect catalogue, and the gaps it opens here | [PRIOR-ART.md](PRIOR-ART.md) — evidence, never a plan; a gap becomes a box only by a ruling ([D89](NOTES.md#d89--k9ss-tracker-is-read-as-prior-art-and-twelve-of-its-classes-become-boxes-2026-08-14) is the first) |

`tmp/` holds downloaded upstream docs (kube-rs, ratatui, k8s) and is never
committed.

## Language and documentation rules

- **Every file in this repo is written in English — never write Turkish into a
  file.** Turkish is used only when talking with the user in conversation. Sole
  exception: `README_TR.md`, the Turkish translation of `README.md`.
- **The application language is English:** finding texts, error messages, key
  hints, help. Jargon is simplified (`OOMKilled` → "container exceeded its
  memory limit"). No i18n (NOTES, YAGNI).
- Code comments and commit messages are in English.
- **A structural change is not done until the docs match it.** Anything that
  changes file layout, architecture, dependencies, the rule set, the key map or
  the CLI surface updates `docs/` (the affected file) and — once Phase 13 writes
  them — `README.md` and `README_TR.md`, in the *same* change. Stale docs are a
  failed step; do not commit them.

## Architecture workflow

- **Read first, draw second, write third:** pull the relevant upstream docs
  (kubernetes / kube-rs / ratatui) into `tmp/` and read them before touching
  the architecture.
- **Pyramid phases, bottom-up, forward-only.** Layer order, bottom → top:
  `rules.rs` → `analysis.rs` → `k8s.rs` → `ops.rs` → `theme.rs` → `views.rs` →
  `ui.rs` → `main.rs`. A step may create new files or shape the current top
  layer; **files finished in earlier steps are frozen.** If a later step needs a
  frozen file changed, the plan is wrong: stop, fix the order, record it in
  `NOTES.md`, continue. The two pure files come first because they need neither
  a cluster nor a terminal to be proven. Learning spikes go into `examples/` as
  throwaway code and never touch product files.
- **Dangerous code is proven before it is bound to a key.** `ops.rs` sits low in
  the pyramid so every write is verified headlessly against kind first.
- **Single point of change:** one change propagates from one place (theme = one
  file, `theme.rs`).

## Hard invariants — never break one without an explicit decision

Breaking one of these is a stop, not a bug to fix later. If a task seems to
require it, fix the plan, record the reversal in [NOTES.md](NOTES.md), continue.

1. **Writes live in `ops.rs` and nowhere else.** An **allowlist**, never a
   denylist — outside `ops.rs` only `get*` / `list*` / `watch*` / `logs` /
   `log_stream` / `apiserver_version` may appear. `clippy.toml` carries the ban
   crate-wide with `-D warnings`; `ops.rs` carries the single visible
   `#![allow(clippy::disallowed_methods)]`. The allowlist stays **mechanical**:
   `may_i(...)` lives in `ops.rs` despite mutating nothing, because it is
   performed with `create`
   ([D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)).
2. **No write is implicit.** Every mutation requires: an explicitly selected
   object → a keypress → a confirmation dialog stating the consequence in plain
   language → a server-side `dryRun=All` where the API supports it → an audit
   line. Deletes and drains additionally require typing the object name.
   `--read-only` makes the whole path unreachable, not merely unbound. Bulk
   mutation does not exist.
3. **Nothing is deployed into the cluster.** k8rs runs on the user's machine
   against their kubeconfig, and that is the entire trust model.
4. **Every mutation is visible twice, and neither record may lie.** The
   **command log** shows the *equivalent* kubectl command as the user would have
   typed it — the teaching device. The **audit log** records that line *and* the
   real call (verb, path, resourceVersion sent, dry-run verdict, result).
   Separate, because k8rs runs `Api::patch_scale`, not `kubectl scale`. Missing
   from either is a bug ([D8](NOTES.md#d8--invariant-4-was-not-literally-true)).
5. **Rules are pure functions:** `analyze(&Snapshot) -> Vec<Finding>` — no
   network, no terminal, no globals, no `Result` (a missing field means no
   finding), and **no clock call**: `Snapshot` carries `now`, captured once by
   the caller
   ([D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)). Findings
   carry timestamps; the *renderer* turns one into "4 min ago". Same for
   `analysis.rs`.
6. **Watch, never poll-list** — a periodic `LIST pods -A` is what makes k9s
   heavy. LIST once then stream changes, pruned with `managedFields` dropped.
   The Alerts view's inputs are watched permanently — Pods, Nodes, and
   Deployments/StatefulSets/DaemonSets — **pruned to the fields the snapshot
   types in `rules.rs` name** and no others, across metadata, spec *and* status;
   "metadata + status only" was never true of this design
   ([D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) ·
   [D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
   ReplicaSets are fetched on demand, never watched; browser kinds are watched
   only while their view is open.
7. **No fixed FPS.** Draw on events, coalesce ~100ms during storms, block when
   idle → 0% CPU idle.
8. **The panic path restores the terminal** *and* leaks no credential
   ([docs/security § Token hygiene](docs/security.md#token-hygiene)). Same for
   the temp file behind `e` edit: mode 0600, removed after use.
9. **Free text from the API is untrusted.** Strip control characters before it
   reaches the screen, or a crafted pod name rewrites the user's terminal.
10. **No new dependencies without asking.** The **twelve** allowed crates:
    `kube`, `k8s-openapi`, `ratatui`, `crossterm`, `tokio`, `anyhow`,
    `serde_json`, `serde_yaml_ng`, `x509-parser`, `similar`, `futures-util`,
    `tokio-rustls`.
    `similar` arrives only in v0.4 with `edit` — approved is not the same as
    present. No `clap` while the flags are `--read-only` / `--context` /
    `--namespace` / `--once`; no `tracing` until debugging demands it
    ([NOTES § Dependencies](NOTES.md#dependencies)).
    **The eleventh was a reversal and is the shape to argue from**
    ([D143](NOTES.md#d143--the-eleventh-crate-and-why-the-list-of-ten-was-wrong-rather-than-the-task-2026-08-22)):
    every `kube-runtime` entry point returns `impl Stream`, `Stream` is not in
    `std`, so the ten approved a client and nothing that could consume it. It
    added **no compiled code** — 213 crates in `Cargo.lock` before and after —
    because `futures-util` was already linked under `kube-client`. A crate that
    is already in the build and only needs *naming* is the narrow case; `clap`
    and `tracing` are not, which is why they are still refused.
    **The twelfth is the same shape, measured the same way**
    ([D178](NOTES.md#d178--c3-lands-whole-c2s-row-cannot-be-drawn-in-a-frozen-pane-and-the-twelfth-crate-was-already-compiled-2026-08-28)):
    C2 is only readable off the peer certificate of a handshake we drive, driving
    one needs a connector, and `tokio-rustls` was already linked under
    `hyper-rustls` — **213 packages in `Cargo.lock` before and after**. It
    re-exports `rustls`, so one crate is named and not two, and the `ClientConfig`
    it is handed is `kube::client::ConfigExt`'s, built from the same kubeconfig CA
    the real client uses — so nothing here calls `dangerous()` and
    the TLS line of the security gate holds structurally.
11. **Eight product files, flat.** `main.rs / k8s.rs / ops.rs / rules.rs /
    analysis.rs / views.rs / ui.rs / theme.rs` — no `mod.rs` pyramid, no trait
    layer, no plugin system. Exactly one ninth is pre-approved: `dialog.rs`, if
    `ui.rs` passes ~800 lines ([D11](NOTES.md#d11--the-ninth-file-pre-approved)).
    **Tests sit beside the file they test, never inside it.** A product file
    with tests carries that one declaration —
    `#[cfg(test)] #[path = "<name>_tests.rs"] mod tests;` — and no test code of
    its own; the tests live in `src/<name>_tests.rs`. It is still a
    child module — it sees the private items, and **no `lib.rs` is added**,
    which is the thing [D50](NOTES.md#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12)
    refused and still refuses. This is a convention, not a per-file judgement
    call: every product file that has tests splits the same way.
    **The test file — and only the test file — may split again**, into
    `src/<name>_tests/` with one `#[path]` module per `// --- … START ---`
    region of the product file, when it has grown past what one turn can read.
    `rules_tests.rs` did on 2026-08-15 and now holds the imports, the helpers
    more than one region reads, and five declarations. Still no `mod.rs`, still
    no `lib.rs`, and **the product file never splits with it**: the defects this
    repo has paid most for were two rules reading one container and disagreeing,
    and a module boundary is exactly where the second copy of a shared helper
    grows back ([D91](NOTES.md#d91--the-tests-split-and-the-product-file-does-not-2026-08-15) ·
    [D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)).
12. **No per-kind code in the browser.** Resource views come from API discovery
    and server-side `Table` printing; typed structs exist only where the rule
    engine needs them. A hand-written column list for a kind is a design failure.
13. **Before adding a feature, read
    [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section).**
    The guard, both halves required: *would someone who **runs clusters** use
    this in a normal week — and can a newcomer read the resulting screen without
    a glossary?* If either fails the answer is no, unless the user reverses it
    explicitly — and a reversal is written into NOTES, not applied silently.
14. **Plain language is a hard rule, not a style preference.** Every string a
    user can see — column header, dialog, error, finding — is written for
    someone who does not yet know the jargon. `CrashLoopBackOff` gets explained,
    not printed and left.

## Security gate — run this list on every change, no exceptions

Every diff, including "just a UI tweak". Anything unchecked here is a red build,
not a follow-up ticket. Reasoning: [docs/security.md](docs/security.md),
[REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements).

**Six of these are `scripts/security-guard.py` and `just check` runs it**
([D105](NOTES.md#d105--the-security-gate-splits-into-what-a-script-can-decide-today-and-what-is-waiting-for-code-2026-08-16)):
workflow hygiene, no shell spawned from `src/`, no dependency or hostname outside
the approved list, no `Debug` over a type that can hold a token, no in-cluster
ServiceAccount door, no TLS knob turned off by us. They are marked **`[auto]`**
below and are not re-read by hand. **Everything unmarked is yours**, and a script
that goes green says nothing about those.

**Identity and transport**

- [auto] Credentials come from the kubeconfig current context and nowhere else.
      No in-cluster ServiceAccount path exists — do not open one.
- [auto] TLS verification is never disabled **by us**; the guard reads the code.
      That a kubeconfig setting `insecure-skip-tls-verify` is honoured *and shown
      in the header* is **yours** — no script can see the header.
- [auto] No `Debug` is derived over a type that can hold config. That the token
      is never logged, rendered or put in an error message is **yours**.

**Authorization**

- [ ] Least privilege holds: the documented read-only role runs everything
      except the operations — **including the `nonResourceURLs` grant discovery
      needs**, which the role lacked until 2026-08-26 and which only a cluster
      without the default `system:discovery` binding reveals
      ([D160](NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26)).
      A 403 degrades that one feature and names the missing verb + resource; it
      never crashes and never retries in a loop. **A `nonResourceURL` refusal
      has neither** — the measured `Status` for `/apis` carries an empty
      `details`, so a formatter reading `details.group`/`details.kind` prints an
      empty sentence and the only true one names the path: *"this kubeconfig may
      not `get /apis`"*.
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
- [auto] **No API string is ever interpolated into a shell** — the guard reads
      `src/`, `tests/`, `examples/` and `benches/`, and for every file that spawns
      at all it refuses a shell program, a `-c` flag and a command string. **Two
      files spawn today**, each through an argument vector: `tests/binary.rs` runs
      the built binary, and `src/k8s_tests.rs` runs `openssl` on literals and temp
      paths to build the CA and leaf its TLS server needs
      ([D179](NOTES.md#d179--the-refusal-that-kept-a-mutant-alive-rested-on-a-dependency-just-check-already-had-2026-08-28)).
      When `$EDITOR` lands it is an argument vector too, never a command string;
      a pod named `; rm -rf ~` is boring.
- [ ] The command log is display text. k8rs does not execute it, and nothing in
      it is fed back into a process.
- [ ] Object names are sanitised before they build a filesystem path — `../` in
      a name must not escape the temp directory.
- [ ] Sizes are bounded: a 50MB annotation or an endless log line must not be
      held whole in memory or blow up the renderer.

**Secrets and local files**

- [ ] Environment variable values are never displayed. Secret values require an
      explicit reveal and never enter the command log, the audit log, or the
      YAML shown by `y`.
- [ ] *(from v0.4, when `edit` lands)* The edit temp file is mode 0600, in the
      user's own temp dir, and removed on exit *and* on panic.
- [ ] The audit log is mode 0600 and append-only.
- [ ] The panic path leaks nothing: no credential in a stderr backtrace, and the
      terminal is restored.
- [auto] No telemetry — the guard counts the outbound paths and the dependency
      list. The only connection is the API server in the user's kubeconfig (plus,
      later, an endpoint the user typed themselves — never one discovered from
      cluster annotations, which is **yours** to check).

**Supply chain and release**

- [auto] `cargo deny check` passes (advisories, licenses, sources); `Cargo.lock`
      committed; no non-crates.io source.
- [ ] A new dependency is a recorded decision (invariant 10), not a reflex.
- [auto] Workflows default to `permissions: contents: read`; third-party actions
      pinned to commit SHAs; no `pull_request_target` with secrets.
- [ ] Releases ship `SHA256SUMS`.

**Test data**

- [ ] Fixture sanitization ran *before* the fixture was committed — no
      annotations, no env values, no node identifiers, no real certificates,
      keys or tokens. A leak never leaves git history.

## Code phase rules

- **Write function-based** — never write the same code twice; extract shared
  functions.
- **Comments are sparse, and the "why" belongs in `NOTES.md`.** A doc comment
  states what the item is and cites the decision that shaped it
  (`NOTES § D27`) — it does not re-argue it. Where a rationale genuinely lives
  nowhere else, keep it short and *write the NOTES entry in the same change*, so
  the next comment can cite it instead of repeating it. Block markers when a
  section needs one:

  ```
  // --- AREA START ---
  code
  // --- AREA END ---
  ```

- **Tests must not lie.** A test that cannot fail is not a test:
  - **Positive and negative, both.** Every rule gets a fixture that triggers the
    finding *and* a healthy one that must not.
  - **Fixtures come from real cluster captures**, never hand-written JSON, and a
    committed capture is never hand-edited to make a test pass
    ([D53](NOTES.md#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)).
  - Never weaken, skip or delete a failing test to reach green. Never assert
    what the implementation happens to return; assert what the requirement says
    it must.
  - **Seen red before trusted.** Every new test or guard is run against the code
    *before* the fix and watched fail, then watched pass after
    ([D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)).
  - **A check is proven only for the input shapes it was fed** — list every
    shape the real pipeline hands it and feed it each
    ([D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12))
    — **and only for the framing it was written for**: not just *which objects*
    reach the check but *where inside a value* the secret sits. Plant one case
    per framing: whole value, substring, re-encoded
    ([D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
  - **A derived list asserts it found something.** "Extracted nothing" and
    "nothing to extract" print the same line — assert a known entry is present
    (`write-guard.py`'s `CANARIES`) or the guard degrades in silence.

## Running it — and `just check`

**`just check` is the whole of CI, or it is a lie.** A step whose tool is not
installed locally is added to `just check` anyway: a missing binary is a loud
error, a missing step is an invisible gap.

**Green tests are not the same as working software.** Something is *run* every
box, and its output goes in the report — never report "done" for something that
was not run. Until Phase 3's last box wires the temporary `main.rs`, the real run
is the test binary over a captured fixture, printed and read
(`cargo test -- --nocapture`, with the finding text quoted); after it, the actual
binary, against a fixture or kind.

## Second pass — nothing is delivered on its first draft

**Everything produced here is reviewed a second time before it is handed
over** — code, a doc section, a plan, a rule, a commit message, an answer.
Always, by default, without being asked. Not a re-read from memory: **open the
artifact as written** and read it as its first reader would — someone who was
not in the room and will follow it literally. What it hunts, in order:

1. **Does it contradict itself?** Two sentences that cannot both be obeyed.
2. **Can every rule in it be complied with?** A gate nobody can pass, a step
   that needs a file that does not exist yet.
3. **Does everything in it have an owner?** "Someone will do it" means two
   people do it, or nobody does.
4. **The unhappy path.** Empty, missing, denied, too large, already exists, the
   human is not available.
5. **What does it silently assume?** That is where the bug lives.

**A second pass produces findings or it names what it checked.** "Looks good" is
the first pass claiming to be the second. Findings are fixed **before**
delivery, in the same turn, never filed as follow-ups — then say what changed.

## Agent workflow — who runs each step, and who is accountable

Five subagents live in `.claude/agents/`, committed. **The main session is the
project manager.** Agents do not talk to each other, do not commit, do not push,
and do not check a box in `todo.md`. Every handoff goes through the PM, so there
is exactly one place a lie can be caught. **The PM is not a sixth agent for the
same reason** — a subagent starts cold on every dispatch and would carry neither
this file, nor `todo.md`, nor the box it just landed; `/basla`
(`.claude/commands/`) is the trigger that puts the main session into this
procedure, not a delegation of it.

### Ownership — and the file each one may write

Every path in the repo appears in exactly one **Writes** cell.

| Agent | Writes | Never writes |
|---|---|---|
| `dev-core` | `rules.rs` `analysis.rs` `k8s.rs` `ops.rs`, **and `main.rs` while it is the temporary driver — until `dev-ui` wires it at Phase 12** | anything that draws |
| `dev-ui` | `theme.rs` `views.rs` `ui.rs`, `main.rs` **from Phase 12**, `examples/` (the Phase 8 spike) | the four lower files |
| `tui-designer` | `screens/` | any `.rs` |
| `tester` | `tests/` `scripts/` `justfile` `clippy.toml` `deny.toml` `.github/workflows/` | product code in `src/` — **including the rule tests**, see below |
| `k8s-admin` | `reports/` — its own measurements, one file per run | every other file: no code, no screen, no plan, no box |
| **PM** (main session) | `todo.md` `NOTES.md` `backlog.md` `REQUIREMENTS.md` `docs/` `README.md` `README_TR.md` `CHANGELOG.md` `Cargo.toml` `Cargo.lock` `cliff.toml` `CLAUDE.md` `.gitignore` `LICENSE` `.claude/agents/` `.claude/commands/`, branches, commits, PRs | `src/` (delegate it) |

**A `<name>_tests.rs`, and every module under `<name>_tests/`, has the same
writer as `<name>.rs`** (invariant 11, which says where they live): the tests
move out of the file, never out of the author's hands. **`tester` does not write
them and does not re-run their red; it attacks them** (step 5). `tests/` is
fixtures and, from Phase 7, end-to-end tests — never the rule tests
([D50](NOTES.md#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12)).

Phase map, from [`todo.md`](todo.md): **2** → `tester` · **3–7** → `dev-core` ·
**8–12** → `dev-ui` · **13** → PM. `tui-designer` and `k8s-admin` have no phases
of their own; they are gates on other people's. `main.rs` is the one file whose
owner changes
([D34](NOTES.md#d34--the-temporary-mainrs-belongs-to-dev-core-until-phase-12-2026-08-12)).

### The PM does not wait for approval — the boxes run back to back

**Standing authorisation
([D98](NOTES.md#d98--the-user-leaves-the-room-and-the-pm-stops-asking-2026-08-15)):
the user is not in the room.** The PM picks the next family, briefs it, runs the
cycle, lands it, and starts the next — no "shall I continue", no question
[`todo.md`](todo.md) already answers. **Nothing else loosens**, and the person who
could have caught a skipped gate has left, so a gate skipped now is never found.
The only stops: the section below, a red build, and any reversal of a design
decision — written into [NOTES.md](NOTES.md) before it is acted on.

### The boxes no agent can run — say so, do not fake them

Some boxes need a machine, a credential or an account the agents do not have:
the crates.io publish, GitHub repo settings, anything behind a login. The PM
prints the exact command for the user and waits for the real output. A box whose
evidence is "this would work" is an unchecked box.

**The cluster is split by what the run produces, not by who runs it**
([D92](NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)).
Anything that **produces an artifact** — `just fixtures`, any write into
`tests/`, and `just e2e` whose green *is* a box's done-when — is the PM's, because
committed fixtures carry [D53](NOTES.md#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)
and the sanitization gate. An **ephemeral measurement** — bring a cluster up,
check one claim, tear it down — is `k8s-admin`'s, and nobody else's: a dev with a
cluster tunes the code until the cluster agrees. It runs under
**`K8RS_CLUSTER=review`** — the default name is the PM's fixture cluster and
teardown would delete it. **The sanitizer never sees a cluster name; it reads
node names**, and since 2026-08-20 `scripts/sanitize.jq` accepts only the four
the fixture cluster actually produces — `k8rs-control-plane` and
`k8rs-worker[N]`, `.lan` suffix allowed — instead of the whole `k8rs-*` family
that `k8rs-review-control-plane` walked straight through three times
([D94](NOTES.md#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)).
So **no cluster but `k8rs` can produce a committed fixture**, which is stronger
than the old claim about one name, and it holds however the cluster was made —
`cluster.sh` refuses the family name too, but it is the loud guard and not the
load-bearing one, because a reviewer runs `kind create cluster` directly. **The cluster is ephemeral; the write-up is not** — the measurement
lands in [`reports/`](reports/README.md) under that file's sanitization rule,
which is what keeps *an object from the cluster* on the PM's side of this split
while *what was observed about it* stays on `k8s-admin`'s
([D108](NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)).
One cluster at a time, and its output is evidence for a *finding* — a
box that needs a cluster to close is still a PM box.

### The one hard rule of concurrency

**One writer per file tree at a time**, and **the scratchpad is a file tree
too** — each agent works in its own subdirectory of it, named after itself, and
re-verifies anything it saved earlier before relying on it
([D60](NOTES.md#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)).
**So is the cluster** ([D92](NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)):
a capture and a review measurement never run at once, whatever the file trees
say. What may genuinely run at the same time — at most one writer per row:

| Safe together | Because |
|---|---|
| one dev writing `src/` · `tester` writing `tests/`, `scripts/` | disjoint trees |
| one dev writing · `tui-designer` on a **later** phase's screen | `screens/` is not code |
| two reviewers (`k8s-admin` + `tui-designer`) on the same diff | disjoint trees — `k8s-admin` writes only `reports/`, `tui-designer` only `screens/`; but if `k8s-admin` is measuring, no capture runs beside it |
| one dev writing · `k8s-admin` auditing an **already merged** phase | the audit lands as findings and a `reports/` file, never as an edit to the thing audited |

Anything else runs one at a time; worktree isolation (`isolation: "worktree"`)
exists if two writers are ever unavoidable, but reach for the plan fix first.
**Review is not one of these slots** — nothing is built on top of a box until
`k8s-admin` reports, and the dev idles meanwhile.

**A re-dispatch to fix a finding is a write, not a review** — `screens/` went to
its owner for a rewrite in the slot the table below reserves for two *reviewers*,
and the reviewer's first read carried a section that no longer existed
([D136](NOTES.md#d136--three-claims-that-were-reasoned-instead-of-measured-and-the-one-sentence-that-catches-all-three-2026-08-21)).

**Sending a finished agent another message is a new dispatch**, and a resumed
agent owns its files again the moment it wakes — so a follow-up while someone
else holds that tree puts two writers on it, and the second one's restore from
backup silently reverts the first
([D96](NOTES.md#d96--the-run-a-container-is-sitting-in-is-no-rules-subject-and-the-one-reader-may-only-suppress-2026-08-15)).
Check who holds the files before resuming; if someone does, wait or tell them
what landed underneath.

**The gate is not split by tree, so the PM is a writer too.** `just check` reads
`docs/`, `todo.md`, `NOTES.md` and `screens/` as well as `src/`. Two rules, both
cheap: **every PM edit is self-consistent on its own** — never a link to an
anchor a later edit will add — and while an agent is running the gate, the PM
either waits or expects to explain the red in somebody else's report.

### The cycle — one **family** of `todo.md` boxes is one turn of it

**The family is the unit of work — briefed, written, reviewed and committed as
one turn** ([D109](NOTES.md#d109--the-family-is-the-unit-of-work-and-the-commit-stays-per-turn-2026-08-16)).
A family is the boxes that touch the same code and answer the same question; the
PM names it at the phase's head in [`todo.md`](todo.md). Boxes with no family
stay one at a time, and **`ops.rs` is never batched** — see step 6.

**The commit does not batch with it.** Every turn commits, never a phase: a
commit costs nothing and is the recovery point, and a phase that commits once
gives sixteen boxes of work one changelog line and no way back to box eleven.

| # | Step | Who | Gate to pass |
|---|---|---|---|
| 1 | Read the box, decide the owner, write the brief | PM | the box is the *first unchecked one in the lowest open phase* — no cherry-picking |
| 2 | Screen spec, **only if a screen changes** | `tui-designer` | the mockup covers every state, not just the happy one |
| 3 | Write the code **and its tests together** | `dev-core` / `dev-ui` | invariants; forward-only; no new dependency |
| 4 | Prove the tests can fail | the author, before reporting | `just mutants-diff` over the box's **own diff**, not the file — a surviving mutant is a test that cannot fail; the author's red/green is pasted in step 3 — see below |
| 5 | Attack it, then the full run | `tester` | the assertions attacked and the unfed shapes fed · `just check` green **and** the code exercised for real |
| 6 | Operator review | `k8s-admin` | blocking for `rules.rs` `analysis.rs` `ops.rs` `k8s.rs`, any dialog, any kubectl line; skippable only for formatting. **Batched by rule family, not by rule** — see below |
| 7 | Land it | PM | see below |

**Step 6 reads the family together, with the shared helpers they all call**
([D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)):
a reviewer shown one rule finds the defect in that one rule, and every expensive
defect this repo has had was two rules reading one container and disagreeing,
which is invisible from inside either. **Two things stay per-box, because their
blast radius is not a family: anything in `ops.rs`, and any change to a shared
helper** — a helper is the thing every rule in the family already agreed on.

Step 7 in order, one push at the end and not two: [second
pass](#second-pass--nothing-is-delivered-on-its-first-draft) over the **landed
tree**, which named what it checked · every box of the [Security
gate](#security-gate--run-this-list-on-every-change-no-exceptions) · docs sync if
the change was structural · check the box in `todo.md`, same commit as the work ·
CHANGELOG with git-cliff, committed separately as `chore(changelog): update` ·
commit and push.

**That pass reads the result, not the diffs — and it is never skipped because
everyone upstream already passed** (2026-08-13, the user's standing
instruction). Every pass before it saw one slice; a review can even create the
defect it could not have seen
([D69](NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
Open the changed files whole, and check the PM's own edits with the rest — they
got no review at all. Findings are fixed **before** the push, in the same turn.

Steps 4–6 loop back to 3 on any failure, and nothing is negotiated down to get
past a gate. **When the reviewer and the author disagree, the PM decides, in
writing:** a finding is closed by being fixed, or by being rejected with the
reason recorded in `NOTES.md`.

**Branches: there is one, `development`.** Every box commits onto it; the PR to
`main` opens **and is merged** at phase close, not per box
([D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).
Agents never create, switch, merge or delete a branch. Work that is not a
phase — a fix, a docs change, this file — goes on `development` too.

### Step 4 is the anti-leak mechanism, so a machine runs it

"I saw it fail" is a claim
([D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)). **The author
still proves its own change red then green and pastes both** — that is step 3's,
not a separate turn. What checks the *claim* is a mutation run, because a
surviving mutant is a test that cannot fail, stated by a tool with no incentive
([D104](NOTES.md#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)).
**Per turn it is scoped to the diff** — **`just mutants-diff`**, never a raw
`cargo mutants` line — because the whole file was 519 mutants at ~2s each the last
time it was run whole (2026-08-16; the file has grown since). `just mutants` whole
is the *phase-close* gate, and `--iterate` skips what an earlier run already
caught. **Both go through `scripts/mutants.sh`, and that is not a convenience.**
cargo-mutants files *any* build failure as `unviable`, so a mutant that never got
built because the scratch volume was full reads exactly like one that cannot
compile — and this box's `/tmp` is a 12 GiB tmpfs that has been at 94% while
`$HOME` had 916 GB free
([D133](NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21)).
The script names its own scratch volume, refuses to start without headroom, and
**reads the run's logs afterwards** — an honest `unviable` names a type, a
dishonest one names a filesystem, and the *count* cannot tell them apart (the last
phase close had 55 legitimate unviables). A shard that dies for space prints no
`MISSED` line, which is exactly what a passing shard prints.

**So `tester` no longer re-runs the author's mutations by hand** — measured, it
found zero defects for fourteen minutes and 120k tokens
([D104](NOTES.md#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)).
Its work is the part that did find things: attack the assertions (is this
expected number derived from the requirement, or updated to match the output?),
feed the shapes the author did not, read what the screen actually prints, and
`just check`. Guards in `scripts/` keep the hand `--self-test`, their equivalent.

### The brief the PM hands out, and the report it gets back

The brief, six lines, no more: the box verbatim · the files you may write ·
what "done" means for this turn · which `NOTES.md` section decides the
behaviour · what is explicitly out of scope · **and what to read, by region**.

**A box written a phase ago may describe a defect the code has already closed**,
so the brief carries its premise re-checked at HEAD, or says plainly that it was
not. Two Phase 4 boxes were stale; the one checked at brief time cost nothing and
the one that was not cost a review round
([D136](NOTES.md#d136--three-claims-that-were-reasoned-instead-of-measured-and-the-one-sentence-that-catches-all-three-2026-08-21)).

**That last line is the one that costs hours when it is missing**
([D110](NOTES.md#d110--the-brief-names-the-regions-because-a-cold-dispatch-reads-fifteen-thousand-lines-2026-08-16)).
An agent starts cold and, told only *fix rule 5*, pages the whole file. Name the
`// --- … START ---` regions and the test modules the turn actually touches, plus
the shared helpers it must not disagree with — the pod rules are **one**
`// --- THE POD RULES START ---` region, not the whole of `rules.rs`, and the
region markers exist for exactly this.

The report, or the work is not received: what changed and where · the exact
commands run and their real output · the red run and the green run · what could
not be proven and why · anything the agent wanted to touch outside its
ownership · **every choice it had to make that the brief did not decide** · what
its own second pass found and changed. **No output pasted, no completion.**

That last item is the one that goes missing: an agent that picked a threshold,
named a field or settled a behaviour the docs did not settle has made a
decision, and the PM writes it into `NOTES.md` before committing.

### Where a leak would actually happen — the PM checks these by hand

- A box checked for work that was written but never *run*.
- A test that has only ever been green — step 4's mutation run skipped because
  the diff looked small. It is `--in-diff` and it costs a minute.
- **A claim reasoned from a definition instead of measured against the object.**
  Numbers are the loud half — two drafts of the mutation gate put a wrong figure
  in this file, each reasoned about the tool rather than read off it
  ([D104](NOTES.md#d104--the-second-agent-was-re-running-the-first-agents-commands-and-a-tool-does-it-better-2026-08-15)).
  The quiet half is prose: a formula read correctly and concluded from wrongly, a
  feature gate's name, a field's type, a column budget estimated in a review and
  repeated as measured — four of them in one turn, each written by someone being
  careful with the object one command away
  ([D136](NOTES.md#d136--three-claims-that-were-reasoned-instead-of-measured-and-the-one-sentence-that-catches-all-three-2026-08-21)).
  **The definition says what it is; only the object says what it does** — and
  somebody else's finding stays an estimate until *you* have run it.
- The security gate skipped because "this diff is only UI".
- An agent editing outside its ownership row, quietly. PM reads the diff before
  committing, every time.
- Docs left stale after a structural change.
- The second pass skipped because the change was small.
- **Step 7's pass skipped because every agent already passed one.** They read
  slices; nobody read the result.
- **The PM's own edits reviewed by nobody.** `NOTES.md`, `todo.md`, `docs/` and
  this file go through step 7's pass with the agents' work, not around it.

## Phase close — the ritual at the end of every phase

A phase is not "mostly done". It closes, or it is still open. When the last box
of a phase in [`todo.md`](todo.md) is about to be checked, run this whole list,
in order, no skipping:

1. **`just check` green**, and the code actually exercised.
2. **Build it and run it on the test host, and paste what it printed.** A phase
   whose only evidence is `cargo test` has never left the fixture. `ssh ubuntu`
   (`murat@192.168.1.130`): build the real binary there and run it — against a
   committed fixture while the temporary `main.rs` is the driver, against kind
   from Phase 5 on, breaking pods by hand and watching the screen answer. **Run
   there, never capture there**: that host has 3.8 GiB and silently reports a
   memory-limit kill as `Error` instead of `OOMKilled`, which is how a capture
   taken on it destroys rule 2's fixture
   ([D84](NOTES.md#d84--a-memory-starved-capture-host-silently-turns-oomkilled-into-error-2026-08-14)).
3. **Every box of the phase is checked, and every check is true.** If something
   could not be proven, leave the box open and say why in the item — an honest
   open box beats a false tick.
4. **The family code review** — `k8s-admin` reads the phase's rules together,
   with the helpers they share, not one box at a time
   ([D103](NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)).
   It runs *after* step 2, because reviewing code nobody has run spends the
   review on questions the run answers for free.
5. **The phase's own security gate** in todo.md, item by item, plus the
   [Security gate](#security-gate--run-this-list-on-every-change-no-exceptions)
   above read against the phase as a whole — `cargo deny` is already inside
   `just check` and is not this step.
6. **Triage, and this is what stops the loop.** A **blocker** — wrong output, a
   crash, anything the security pass rules exploitable — is fixed and steps 1–5
   run again over what the fix could have broken: always the run and the
   security pass, and the family review too if the fix touched a shared helper.
   **Everything else is boxed in a later phase and the phase still closes.** An
   unbounded *go back to the start* means the last nit found is the one that
   decides whether the phase ever ends, and there is always one more nit.
7. **[Second pass](#second-pass--nothing-is-delivered-on-its-first-draft) over
   the whole phase, not box by box** — the only place cross-box defects live:
   two boxes that solved the same problem differently, a decision made in box 3
   that box 9 quietly violated, a gate that stopped being passable halfway
   through.
8. **Docs sync:** `docs/`, `README.md`, `README_TR.md` for anything structural.
9. **CHANGELOG** with git-cliff, committed separately.
10. **Commit, push, PR `development` → `main`, and merge it — the PM does this.**
   Standing authorisation: nobody is asked before a green PR closing the current
   phase is merged. In order: push `development` · open the PR · wait until
   **every** check has *reported* (a pending check is not a green one) · merge
   with a merge commit · **stay on `development`**, it is not deleted. Never on
   red, never mid-run, never force past a conflict — a conflict is a question to
   answer, not an obstacle to push through. If the tooling refuses a step, print
   the exact command for the user rather than leaving the phase half-merged.
11. **Then say, in the reply, that the phase is closed and the context should be
    cleared** — name the phase, name what the next one starts with. Clearing is
    the user's command (`/clear`); the agent cannot issue it.

## Git rules

- Commits and pushes use **the user's signature and identity — only**. **Never
  sign as Claude.** No `Co-Authored-By: Claude` trailer, no "Generated with
  Claude Code" footer, no Claude/AI mention in commit messages, PR titles or PR
  bodies. This overrides any default tooling behavior that adds one.
- **Conventional commits are mandatory** — git-cliff generates the changelog
  from these, and `filter_unconventional` makes a bad message vanish silently
  rather than warn.

  ```
  <type>(<scope>): <subject>

  feat(rules): detect containers killed for exceeding their memory limit
  fix(k8s): distinguish 403 from a dead API server on startup
  ```

  - **types:** `feat` `fix` `docs` `perf` `refactor` `style` `test` `chore` `ci` `revert`
  - **scopes:** `rules` `k8s` `ui` `theme` `main` `fixtures` `ci` `docs`
  - **subject:** English, imperative, lowercase, no trailing period
  - breaking: `feat(rules)!: ...` plus a `BREAKING CHANGE:` footer
- **All work happens on `development`** — one long-lived branch, never deleted.
  `main` only ever advances by merging `development` into it, at phase close.
  Never commit directly to `main`.
- **Before pushing**, update the CHANGELOG with
  [git-cliff](https://github.com/orhun/git-cliff).

Reference workflows: [titus-ai](https://github.com/ChrisTitusTech/titus-ai),
[christitus.com/my-ai-workflow](https://christitus.com/my-ai-workflow/)
