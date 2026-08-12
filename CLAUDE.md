# k8rs — Claude Working Instructions

**lazygit for Kubernetes** — a single-binary Rust + ratatui TUI that tells you
what is broken in a cluster and why in language a beginner understands, and
lets them fix it without memorising long `kubectl` commands, showing the
command it ran every time.

> Scope was deliberately widened on 2026-08-11 (read-only → managed writes,
> browser, analysis reports). Read
> [NOTES § Reversal](NOTES.md#reversal--read-only--managed-writes-2026-08-11)
> before acting on anything that predates it.

This file is the working rules — binding, always in effect. It holds no plans,
no decisions and no requirements; those have their own files, below.

## What to do next

The next task is the **first unchecked box in the lowest open phase** of
[`todo.md`](todo.md), and nowhere else — and "lowest open" can be an earlier
phase that a decision deliberately left open underneath a later one, so read
the file rather than assuming the highest-numbered phase is the only one
running
([D33](NOTES.md#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12) ·
[D47](NOTES.md#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12)).

All work is on `development`. Product code is bottom-up and forward-only, and
a file finished in an earlier step is frozen.

## Where to look

| Looking for | Go to |
|---|---|
| Which file holds X, who may write it, what to touch for a change | [docs/maps.md](docs/maps.md) |
| **Why** a choice was made — every decision, numbered `D1…` | [NOTES.md](NOTES.md) |
| **What** is required, per role (developer / devops / devsecops) | [REQUIREMENTS.md](REQUIREMENTS.md) |
| **When** — phases, order, done-when. The only place steps are checked off | [todo.md](todo.md) |
| The **built** state, for humans outside this repo | [docs/](docs/README.md) — never contains anything not yet true of the code |
| What a screen actually looks like, key by key | [screens/](screens/README.md) — one file per screen |
| The v1 rule set (rules 1–11, severities, thresholds) | [NOTES § v1 rule set](NOTES.md#v1-rule-set) |
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

`tmp/` holds downloaded upstream docs (kube-rs, ratatui, k8s) and is never
committed.

## Language and documentation rules

- **Every file in this repo is written in English — never write Turkish into a
  file.** Turkish is used only when talking with the user in conversation. Sole
  exception: `README_TR.md`, the Turkish translation of `README.md`.
- **The application language is English:** finding texts, error messages, key
  hints, the help screen. Jargon is simplified (`OOMKilled` → "container
  exceeded its memory limit"). No i18n (NOTES, YAGNI).
- Code comments and commit messages are in English.
- **A structural change is not done until the docs match it.** Anything that
  changes file layout, architecture, dependencies, the rule set, the key map or
  the CLI surface updates `docs/` (the affected file), `README.md`, and
  `README_TR.md` if the README changed — in the *same* change. Stale docs count
  as a failed step; do not commit them.

## Architecture workflow

- **Read first, draw second, write third:** pull the relevant upstream docs
  (kubernetes / kube-rs / ratatui) into `tmp/` and read them before touching
  the architecture.
- **Pyramid phases, bottom-up, forward-only.** Layer order, bottom → top:
  `rules.rs` → `analysis.rs` → `k8s.rs` → `ops.rs` → `theme.rs` → `views.rs` →
  `ui.rs` → `main.rs`. A step may create new files or shape the current top
  layer; **files finished in earlier steps are frozen** — step 3 never reaches
  back into a file built in step 1. If a later step needs a frozen file
  changed, the code is not the problem, the plan is: stop, fix the order,
  record it in `NOTES.md`, continue. The two pure files come first because they
  need neither a cluster nor a terminal to be proven. Learning spikes go into
  `examples/` as throwaway code and never touch product files.
- **Dangerous code is proven before it is bound to a key.** `ops.rs` sits low
  in the pyramid so every write is verified headlessly against kind first.
- **Single point of change:** one change should propagate through the project
  from one place (theme = one file, `theme.rs`).

## Hard invariants — never break one without an explicit decision

Breaking one of these is not a bug to fix later, it is a stop. If a task seems
to require it, the plan is wrong: fix the plan, record the reversal in
[NOTES.md](NOTES.md), then continue.

1. **Writes live in `ops.rs` and nowhere else.** An **allowlist**, never a
   denylist — `Api`'s mutating surface grows upstream: outside `ops.rs` only
   `get*` / `list*` / `watch*` / `logs` / `log_stream` / `apiserver_version`
   may appear. `clippy.toml` carries the ban crate-wide with `-D warnings`;
   `ops.rs` carries the single visible
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
   against their kubeconfig, and that is the entire trust model — now the only
   structural guarantee left. It does not get traded away.
4. **Every mutation is visible twice, and neither record may lie.** The
   **command log** shows the *equivalent* kubectl command as the user would
   have typed it — the teaching device. The **audit log** records that line
   *and* the real call (verb, path, resourceVersion sent, dry-run verdict,
   result) — the trail. Separate, because k8rs runs `Api::patch_scale`, not
   `kubectl scale`. Missing from either is a bug
   ([D8](NOTES.md#d8--invariant-4-was-not-literally-true)).
5. **Rules are pure functions:** `analyze(&Snapshot) -> Vec<Finding>` — no
   network, no terminal, no globals, no `Result` (a missing field means no
   finding), and **no clock call**: `Snapshot` carries `now`, captured once by
   the caller, which is what keeps a fixture from expiring
   ([D18](NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)). Findings
   carry timestamps; the *renderer* turns one into "4 min ago". Same for
   `analysis.rs`.
6. **Watch, never poll-list** — a periodic `LIST pods -A` is the exact thing
   that makes k9s heavy
   ([NOTES § Architecture](NOTES.md#architecture--where-lightweight-comes-from)).
   LIST once then stream changes, pruned with `managedFields` dropped. The
   Alerts view's own inputs are watched permanently — Pods, Nodes, and
   Deployments/StatefulSets/DaemonSets, metadata + status only
   ([D28](NOTES.md#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12));
   ReplicaSets are fetched on demand, never watched; browser kinds are watched
   only while their view is open.
7. **No fixed FPS.** Draw on events, coalesce ~100ms during storms, block when
   idle → 0% CPU idle.
8. **The panic path restores the terminal** *and* leaks no credential: a
   backtrace on stderr must never contain a token
   ([docs/security § Token hygiene](docs/security.md#token-hygiene)). Same for
   the temp file behind `e` edit: mode 0600, removed after use.
9. **Free text from the API is untrusted.** Strip control characters before it
   reaches the screen, or a crafted pod name rewrites the user's terminal.
10. **No new dependencies without asking.** The ten allowed crates: `kube`,
    `k8s-openapi`, `ratatui`, `crossterm`, `tokio`, `anyhow`, `serde_json`,
    `serde_yaml_ng`, `x509-parser`, `similar`. `similar` arrives only in v0.4
    with `edit` — approved is not the same as present. No `clap` while the
    flags are `--read-only` / `--context` / `--namespace` / `--once`; no
    `tracing` until debugging demands it
    ([NOTES § Dependencies](NOTES.md#dependencies)).
11. **Eight files, flat.** `main.rs / k8s.rs / ops.rs / rules.rs / analysis.rs
    / views.rs / ui.rs / theme.rs` — no `mod.rs` pyramid, no trait layer, no
    plugin system. Exactly one ninth is pre-approved: `dialog.rs`, if `ui.rs`
    passes ~800 lines ([D11](NOTES.md#d11--the-ninth-file-pre-approved)).
    Anything beyond needs the boundary argument made first.
12. **No per-kind code in the browser.** Resource views come from API discovery
    and server-side `Table` printing; typed structs exist only where the rule
    engine needs them. A hand-written column list for a kind is a design
    failure, not a feature.
13. **Before adding a feature, read
    [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section)
    first.** Scope creep is the named number-one risk. The guard, both halves
    required: *would someone who **runs clusters** use this in a normal week —
    and can a newcomer read the resulting screen without a glossary?* If either
    fails the answer is no, unless the user reverses it explicitly — and a
    reversal is written into NOTES, not applied silently.
14. **Plain language is a hard rule, not a style preference.** Every string a
    user can see — column header, dialog, error, finding — is written for
    someone who does not yet know the jargon. `CrashLoopBackOff` gets
    explained, not printed and left.

## Security gate — run this list on every change, no exceptions

Not a phase, not a review milestone: this runs on every diff, including "just a
UI tweak". Anything unchecked here is a red build, not a follow-up ticket. The
reasoning lives in [docs/security.md](docs/security.md) and
[REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements).

**Identity and transport**

- [ ] Credentials come from the kubeconfig current context and nowhere else.
      No in-cluster ServiceAccount path exists — do not open one.
- [ ] TLS verification is never disabled by us. A kubeconfig that sets
      `insecure-skip-tls-verify` is honoured *and shown in the header* —
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
      with an argument vector, never a command string — a pod named
      `; rm -rf ~` must be boring.
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
- [ ] *(from v0.4, when `edit` lands — nothing before it writes a temp file)*
      The edit temp file is mode 0600, in the user's own temp dir, and removed
      on exit *and* on panic.
- [ ] The audit log is mode 0600 and append-only.
- [ ] The panic path leaks nothing: no credential in a stderr backtrace, and
      the terminal is restored.
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
- **Use comments sparingly**, with block markers when needed:

  ```
  // --- AREA START ---
  code
  // --- AREA END ---
  ```

- **Tests must not lie.** A test that cannot fail is not a test:
  - **Positive and negative, both.** Every rule gets a fixture that triggers
    the finding *and* a healthy one that must not — missing negatives are how
    false positives ship.
  - **Fixtures come from real cluster captures**, never hand-written JSON
    (hand-written JSON resembles reality; it is not reality), and a committed
    capture is never hand-edited to make a test pass
    ([D53](NOTES.md#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)).
  - Never weaken, skip or delete a failing test to reach green — a failing test
    means the code or the plan is wrong. Never assert what the implementation
    happens to return; assert what the requirement says it must.
  - **Seen red before trusted.** Every new test or guard is run against the
    code *before* the fix and watched fail, then watched pass after. One that
    has only ever been green proves as much as an empty file does — and this
    applies to every test added from here on, not only Phase 1's
    ([D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)).
  - **A check is proven only for the input shapes it was fed.** List every
    shape the real pipeline hands it and feed it each — the sanitizer was
    proven on a single Pod and was a near no-op on every `kubectl get -A` List
    ([D29](NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
  - **…and only for the framing it was written for.** Not just *which objects*
    reach the check but *where inside a value* the secret sits — a `-----BEGIN`
    rule misses the same key base64-wrapped, which is how every Secret value
    arrives. Plant one case per framing: whole value, substring, re-encoded
    ([D31](NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)).
  - **A derived list asserts it found something.** When a check builds its own
    rules from another source, "extracted nothing" and "nothing to extract"
    print the same line. Assert a known entry is present (`write-guard.py`'s
    `CANARIES`) or the guard degrades in silence.

## Running it — and `just check`

**`just check` is the whole of CI, or it is a lie.** Anything CI runs that
`just check` skips can only ever fail *after* a push. A step whose tool is not
installed locally is added to `just check` anyway: a missing binary is a loud
error, a missing step is an invisible gap.

**Green tests are not the same as working software.** Something is *run* every
box, and its output goes in the report — never report "done" for something that
was not run. Before Phase 5 wires the binary, the real run is the test binary
over a captured fixture, printed and read (`cargo test -- --nocapture`, with
the finding text quoted); from Phase 5 on it is the actual binary, against a
fixture or kind. Demanding a binary run in Phase 3 would set a gate nobody can
pass, and an impassable gate teaches everyone to wave gates through.

## Second pass — nothing is delivered on its first draft

**Everything produced here is reviewed a second time before it is handed
over** — code, a doc section, a plan, a rule, a commit message, an answer. Not
"if it feels risky": always, by default, without being asked. It is not a
re-read from memory: **open the artifact as written** and go through it as its
first reader would — someone who was not in the room, and who will follow it
literally.

What it hunts, in this order:

1. **Does it contradict itself?** Two sentences that cannot both be obeyed —
   the most common defect in anything longer than a page, and the hardest to
   see while writing, because both halves felt right in isolation.
2. **Can every rule in it be complied with?** A gate nobody can pass, a step
   that needs a file that does not exist yet. An impassable rule teaches
   everyone that rules here are decorative.
3. **Does everything in it have an owner?** "Someone will do it" means two
   people do it in the same turn, or nobody does.
4. **The unhappy path.** Empty, missing, denied, too large, already exists, the
   human is not available. First drafts describe the case that works.
5. **What does it silently assume?** The assumption that was obvious while
   writing is invisible to the reader, and it is where the bug lives.

**A second pass produces findings or it names what it checked.** "Looks good"
is the first pass claiming to be the second; if it genuinely found nothing, say
what was examined and what would have failed it. Findings are fixed **before**
delivery, in the same turn, never filed as follow-ups — then say what the
second pass changed, so the user sees the review and not just its output.

## Agent workflow — who runs each step, and who is accountable

Five subagents live in `.claude/agents/`, committed. **The main session is the
project manager.** Agents do not talk to each other, do not commit, do not
push, and do not check a box in `todo.md`. Every handoff goes through the PM,
so there is exactly one place a lie can be caught.

### Ownership — and the file each one may write

Every path in the repo appears in exactly one **Writes** cell. A file with no
owner is written by whoever happens to be holding the pen.

| Agent | Writes | Never writes |
|---|---|---|
| `dev-core` | `rules.rs` `analysis.rs` `k8s.rs` `ops.rs`, **and `main.rs` while it is the temporary driver (Phases 3–7)** | anything that draws |
| `dev-ui` | `theme.rs` `views.rs` `ui.rs`, `main.rs` **from Phase 12**, `examples/` (the Phase 8 spike) | the four lower files |
| `tui-designer` | `screens/` | any `.rs` |
| `tester` | `tests/` `scripts/` `justfile` `clippy.toml` `deny.toml` `.github/workflows/` | product code in `src/` — **including the rule tests**, see below |
| `k8s-admin` | nothing — reads everything, reports | every file |
| **PM** (main session) | `todo.md` `NOTES.md` `REQUIREMENTS.md` `docs/` `README.md` `README_TR.md` `CHANGELOG.md` `Cargo.toml` `Cargo.lock` `cliff.toml` `CLAUDE.md` `.gitignore` `.claude/agents/`, branches, commits, PRs | `src/` (delegate it) |

Phase map, from [`todo.md`](todo.md): **2** → `tester` · **3–7** → `dev-core` ·
**8–12** → `dev-ui` · **13** → PM. `tui-designer` and `k8s-admin` have no
phases of their own; they are gates on other people's. `main.rs` is the one
file whose owner changes, because the writer of a module and the writer of the
`mod` line that reaches it cannot be two people
([D34](NOTES.md#d34--the-temporary-mainrs-belongs-to-dev-core-until-phase-12-2026-08-12)).
`Cargo.toml` sits with the PM because the agent that wants the crate is the
last one who should be able to add it.

**`tests/` is fixtures and, from Phase 7, end-to-end tests — not where the rule
tests live**
([D50](NOTES.md#d50--the-rule-tests-live-in-rulesrs-and-no-lib-target-is-added-to-change-that-2026-08-12)).
Rule tests live in `#[cfg(test)] mod tests` inside the file they test, written
by the dev who wrote it; `tester`'s job on those is step 4: attack them.

### The boxes no agent can run — say so, do not fake them

Some boxes need a machine, a credential or an account the agents do not have:
the kind cluster (`just cluster-up`, `just fixtures`, `just e2e`), the
crates.io publish, GitHub repo settings, anything behind a login. The PM does
**not** improvise around these and does not check the box: it prints the exact
command for the user and waits for the real output. A box whose evidence is
"this would work" is an unchecked box.

### The one hard rule of concurrency

**One writer per file tree at a time**, and **the scratchpad is a file tree
too** — each agent works in its own subdirectory of it, named after itself, and
re-verifies anything it saved earlier (a hash, a line count) before relying on
it
([D60](NOTES.md#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)).
The pyramid already serialises most of this: `dev-core` and `dev-ui` are never
both writing product code, and a task that seems to need both at once is a plan
error, not a throughput problem.

What may genuinely run at the same time — at most one writer per row:

| Safe together | Because |
|---|---|
| one dev writing `src/` · `tester` writing `tests/`, `scripts/` | disjoint trees |
| one dev writing · `tui-designer` on a **later** phase's screen | `screens/` is not code |
| two reviewers (`k8s-admin` + `tui-designer`) on the same diff | neither writes |
| one dev writing · `k8s-admin` auditing an **already merged** phase | the audit lands as findings, not as an edit |

Anything else runs one at a time; worktree isolation
(`isolation: "worktree"`) exists if two writers are ever unavoidable, but reach
for the plan fix first. **Review is not one of these slots** — nothing is built
on top of a box until `k8s-admin` reports, and the dev idles meanwhile
([D60](NOTES.md#d60--claudemd-was-compressed-and-four-stories-moved-here-2026-08-12)).

### The cycle — one `todo.md` box is one turn of it

The box is the unit of work, never a phase and never "the next few boxes".

| # | Step | Who | Gate to pass |
|---|---|---|---|
| 1 | Read the box, decide the owner, write the brief | PM | the box is the *first unchecked one in the lowest open phase* — no cherry-picking |
| 2 | Screen spec, **only if a screen changes** | `tui-designer` | the mockup covers every state, not just the happy one |
| 3 | Write the code **and its tests together** | `dev-core` / `dev-ui` | invariants; forward-only; no new dependency |
| 4 | Witness red, then green | `tester` | **reverts the implementation and re-runs** — see below |
| 5 | Full run | `tester` | `just check` green **and** the code exercised for real |
| 6 | Operator review | `k8s-admin` | blocking for `rules.rs` `analysis.rs` `ops.rs` `k8s.rs`, any dialog, any kubectl line; skippable only for formatting |
| 7 | Land it | PM | see below |

Step 7 in order, one push at the end and not two: [second
pass](#second-pass--nothing-is-delivered-on-its-first-draft) on the diff, which
named what it checked · every box of the [Security
gate](#security-gate--run-this-list-on-every-change-no-exceptions) · docs sync
if the change was structural · check the box in `todo.md`, same commit as the
work · CHANGELOG with git-cliff, committed separately as `chore(changelog):
update` · commit and push.

Steps 4–6 loop back to 3 on any failure, and nothing is negotiated down to get
past a gate. **When the reviewer and the author disagree, the PM decides, in
writing:** a finding is closed by being fixed, or by being rejected with the
reason recorded in `NOTES.md`. "The dev said it is fine" is not a resolution.

**Branches: there is one, `development`.** Every box commits onto it; the PR to
`main` opens **and is merged** at phase close, not per box, by
[phase close, item 7](#phase-close--the-ritual-at-the-end-of-every-phase) —
described there and nowhere else. Agents never create, switch, merge or delete
a branch
([D32](NOTES.md#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)).
Work that is not a phase — a fix, a docs change, this file — goes on
`development` too.

### Step 4 is the anti-leak mechanism, so it is mechanical

"I saw it fail" is a claim; `tester` reproduces it. Stash or comment out the
implementation body, run the test, watch it fail, restore, watch it pass — and
paste **both** outputs. A test that stays green with the implementation removed
tests nothing
([D26](NOTES.md#d26--a-green-build-that-proves-nothing-2026-08-12)). Same for
guards in `scripts/`.

### The brief the PM hands out, and the report it gets back

The brief, five lines, no more: the box verbatim · the files you may write ·
what "done" means for this box · which `NOTES.md` section decides the
behaviour · what is explicitly out of scope this turn.

The report, or the work is not received: what changed and where · the exact
commands run and their real output · the red run and the green run · what could
not be proven and why · anything the agent wanted to touch outside its
ownership · **every choice it had to make that the brief did not decide** ·
what its own [second pass](#second-pass--nothing-is-delivered-on-its-first-draft)
found and changed. **No output pasted, no completion.**

That last item is the one that goes missing. An agent that picked a threshold,
named a field or settled a behaviour the docs did not settle has made a
decision — the PM writes it into `NOTES.md` before committing, so the wording
stays in one voice and it does not become a decision nobody can reconstruct.

### Where a leak would actually happen — the PM checks these by hand

- A box checked for work that was written but never *run*.
- A test that has only ever been green — step 4 skipped because the change
  "obviously works".
- The security gate skipped because "this diff is only UI" — precisely when the
  untrusted-input items get missed.
- An agent editing outside its ownership row, quietly. The diff is the
  evidence: PM reads it before committing, every time.
- Docs left stale after a structural change — a failed step, not a follow-up.
- The second pass skipped because the change was small — small changes are
  where it is cheapest to run and where nobody is watching.

## Phase close — the ritual at the end of every phase

A phase is not "mostly done". It closes, or it is still open. When the last box
of a phase in [`todo.md`](todo.md) is about to be checked, run this whole list,
in order, no skipping:

1. **`just check` green**, and the code actually exercised — the binary against
   a fixture or kind once Phase 5 has wired one, the test binary read with
   `--nocapture` before that.
2. **Every box of the phase is checked, and every check is true.** A box
   checked for work that was written but never *run* is a lie in the one file
   the plan is read from. If something could not be proven, leave the box open
   and say why in the item — an honest open box beats a false tick.
3. **The phase's own security gate** in todo.md, item by item.
4. **[Second pass](#second-pass--nothing-is-delivered-on-its-first-draft) over
   the whole phase, not box by box.** The per-box passes each saw one diff;
   this one reads the phase end to end, which is the only place the cross-box
   defects live: two boxes that solved the same problem differently, a decision
   made in box 3 that box 9 quietly violated, a gate that stopped being
   passable halfway through. A phase does not close with a known gap in it.
5. **Docs sync:** `docs/`, `README.md`, `README_TR.md` for anything structural.
6. **CHANGELOG** with git-cliff, committed separately.
7. **Commit, push, PR `development` → `main`, and merge it — the PM does this,
   it is not handed back.** Standing authorisation: nobody is asked before a
   green PR closing the current phase is merged. In order: push `development` ·
   open the PR · wait until **every** check has *reported* (a pending check is
   not a green one) · merge with a merge commit, so the phase stays one
   readable block in `git log` · **stay on `development`**, it is not deleted.
   Never on red, never mid-run, never force past a conflict: a clean merge is
   the proof that nothing reached `main` behind the branch's back, so a
   conflict is a question to answer, not an obstacle to push through. If the
   tooling refuses a step, print the exact command for the user
   ([§ the boxes no agent can run](#the-boxes-no-agent-can-run--say-so-do-not-fake-them))
   rather than leaving the phase half-merged. Frozen files stay frozen from
   here, and only the diff you read enforces that — so read it.
8. **Then say, in the reply, that the phase is closed and the context should be
   cleared** — name the phase, name what the next one starts with. Clearing is
   the user's command (`/clear`); the agent cannot issue it and must not
   pretend a fresh context happened on its own.

## Git rules

- Commits and pushes use **the user's signature and identity — only**.
  **Never sign as Claude.** No `Co-Authored-By: Claude` trailer, no "Generated
  with Claude Code" footer, no Claude/AI mention in commit messages, PR titles
  or PR bodies. This overrides any default tooling behavior that adds one.
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
  Never commit directly to `main`: the moment that happens, `development` stops
  being an ancestor of it and the merge stops being clean.
- **Before pushing**, update the CHANGELOG with
  [git-cliff](https://github.com/orhun/git-cliff).

Reference workflows: [titus-ai](https://github.com/ChrisTitusTech/titus-ai),
[christitus.com/my-ai-workflow](https://christitus.com/my-ai-workflow/)
