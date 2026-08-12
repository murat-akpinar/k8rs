# k8rs — documentation

> Status: **Phase 3 — the rules.** The design is closed; the guards and the
> fixture-capture pipeline exist, and `tests/fixtures/` holds real captures from
> a kind cluster you can stand up yourself
> ([tech-stack § The test cluster](tech-stack.md#the-test-cluster--reproducing-it-yourself)).
> `rules.rs` carries the finding shape, the snapshot types and the clock;
> the diagnoses themselves are being written. Nothing runs yet — `main.rs` is
> wired last · Last updated: 2026-08-12

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
