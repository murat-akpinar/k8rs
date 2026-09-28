# k8rs — documentation

> Status: **v0.1.0 is released — milestone M4.** Every phase through 12 is
> closed; Phase 13 has one box still open. `cargo install k8rs` takes it from
> crates.io, and the GitHub release carries static musl and macOS binaries with
> a `SHA256SUMS` beside them.
> The rules and the seven analysis reports are tested against real
> captures from a kind cluster you can stand up yourself
> ([tech-stack § The test cluster](tech-stack.md#the-test-cluster--reproducing-it-yourself)).
> **It runs**: `k8rs` opens the console against your current context — the Alerts
> view, the seven analysis panes, the confirmation dialogs, the write path, the
> cluster picker and the permission probe that marks a key this login may not use
> are wired into one binary, while the reads behind the resource browser and the
> detail tabs are not fetched at all: both panes draw *reading the cluster…* and
> never fill, and `esc` does not leave an opened browser kind
> ([backlog](../backlog.md)) — and `k8rs --once` prints
> the findings and exits
> ([architecture § The command line](architecture.md#the-command-line)).
> Phase 13 landed the permission probe, the driver-flag removal,
> [`README.md`](../README.md) with its Turkish translation, the release
> workflow — and the release. **`v0.1.0` is on crates.io and on the GitHub
> release page**, the tag pushed under the maintainer's account and
> `cargo publish` run from the one machine holding the credential. What keeps
> the box open is that the published tarball carries the README as it read
> *before* the install line moved, which is the page crates.io shows. v0.0.1 is
> skipped
> ([NOTES § D306](../NOTES.md#d306--v001-is-skipped-because-both-halves-of-the-reason-for-it-are-spent-2026-09-28)
> · [§ D307](../NOTES.md#d307--the-registry-publish-ran-in-session-because-the-credential-is-on-the-pms-own-machine-2026-09-28))
> · Last updated: 2026-09-28

This directory is the **built** state: what is true of the shipped tool, written
for humans outside this repo. The reasoning behind any of it lives one level up,
in [NOTES.md](../NOTES.md).

## Map

| Document | Answers |
|---|---|
| [architecture.md](architecture.md) | The three views, data flow, the eight components, the write path, async model, error handling, what is out of scope |
| [security.md](security.md) | Trust model, the write safety model, RBAC for both modes, the audit log, token hygiene, supply chain |
| [tech-stack.md](tech-stack.md) | Crates and versions, toolchain, build targets, visual identity, what is deliberately absent |
| [maps.md](maps.md) | **Every path in the repository** — what it answers, who may write it, and which file to touch for a given change |

## Where things live

| File | Role |
|---|---|
| `docs/` | **The built state.** Never contains anything not yet true of the code |
| [`../NOTES.md`](../NOTES.md) | **Decisions.** Why every choice was made, what was rejected, open questions |
| [`../REQUIREMENTS.md`](../REQUIREMENTS.md) | **What is required**, per role — developer / devops / devsecops |
| [`../todo.md`](../todo.md) | **The plan.** Phases in build order; the only place steps are checked off |
| [`../CLAUDE.md`](../CLAUDE.md) | Working rules for AI agents on this repo |
| [`../screens/`](../screens/README.md) | **The mockups.** What each screen looks like at 80×24, key by key — design-phase, the code has to match them |

## Reading order

New to the project? [NOTES § In one sentence](../NOTES.md#in-one-sentence) →
[architecture.md](architecture.md) → [security.md](security.md). Those three
carry the whole idea; everything else hangs off them.

## The one-line rule

> k8rs explains a cluster to someone who is still learning it, and lets them
> fix what is broken — **showing them the command every time**.

Every decision here is derived from that sentence, and the test for a new idea
has two halves, both required: *would someone who **runs clusters** use it in a
normal week — and can a newcomer read the screen it produces without a
glossary?* The first half keeps expert toys out; the second keeps k8rs from
turning into another cockpit for pilots.

*(Until 2026-08-11 this rule read "It **reads** a cluster and explains it. It
never changes one." Writes were added deliberately; what replaced the read-only
guarantee is in [security.md](security.md#write-safety-model), and why is in
[NOTES § Reversal](../NOTES.md#reversal--read-only--managed-writes-2026-08-11).)*
