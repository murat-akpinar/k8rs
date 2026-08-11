# Screen — Switching cluster (`X`)

[NOTES § Out of scope](../NOTES.md#out-of-scope-the-most-important-section)
says *"one context at a time, **switchable**"* — and nothing defined the
switch. `--context` was a startup flag only, which means the answer to "let me
check staging" was *quit and start again*, several times a day. This screen is
that gap closed ([NOTES § D16](../NOTES.md#d16--the-context-switcher)).

## The picker

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ Switch cluster ──────────────────────────────────────┐        │
│    │                                                      │        │
│    │  ▸ prod-eu          (current)                        │        │
│    │    staging                                           │        │
│    │    kind-k8rs                                         │        │
│    │    dev-cluster      ⚠ TLS not verified               │        │
│    │                                                      │        │
│    │  staging  →  https://staging.internal:6443           │        │
│    │                                                      │        │
│    │  k8rs does not change your kubeconfig — it just      │        │
│    │  talks to the cluster you pick here.                 │        │
│    │                                                      │        │
│    │         [ ⏎ switch ]     [ esc cancel ]              │        │
│    │                                                      │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl config get-contexts                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move   / filter   ⏎ switch   esc cancel                         │
└────────────────────────────────────────────────────────────────────┘
```

| Part | Rule |
|---|---|
| The list | every context in the kubeconfig, in file order. `kube::config::Kubeconfig` already parses it — no new dependency, no second parser, and no file of our own. |
| `(current)` | the context in use, and the row selected when the picker opens. `⏎` on it closes without doing anything. |
| The server line | the API server address of the **selected** row, updated as you move. This is the "am I about to touch production" line, and it is why the address is not hidden behind a detail view. |
| `⚠ TLS not verified` | the context sets `insecure-skip-tls-verify`. Shown *before* the switch, not after — a beginner cannot be expected to infer it from a header they have not read yet. |
| The sentence | k8rs never writes to `~/.kube/config`. Said on the screen because every user of `kubectl config use-context` will assume the opposite. |
| `/` | filters the list, exactly as `/` does in every other pane ([NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted)). |

**One context in the kubeconfig:** the picker still opens and says *"prod-eu
is the only cluster in ~/.kube/config"*. A key that appears to do nothing is
worse than a screen that explains why.

## Why there is no confirmation dialog

Switching is not a mutation — nothing is written, to the cluster or to disk —
so [invariant 2](../CLAUDE.md) does not apply and a second dialog would be
ceremony. **The picker is the confirmation:** an explicit key, an explicit
selection, the target's address on screen, `⏎`.

What *is* required is that the switch be impossible to make by accident while
something else is in flight:

- `X` is unbound while any modal is open — the `Modal` enum in
  [widgets.md § 5](widgets.md#5-the-modal-layer) makes a picker over a
  confirmation unrepresentable.
- If a write has been confirmed and its call has not returned, `X` refuses in
  the footer: *"finishing the change to payments/web first"*. Swapping the
  client out from under an in-flight mutation is how an operation gets
  attributed to the wrong cluster in the audit log.

## What the command log shows — and what it must not

```
$ kubectl --context staging get pods -A --watch
```

**Not** `kubectl config use-context staging`. That command edits the user's
kubeconfig and k8rs does not; printing it would teach a command with a
side effect k8rs never performs, which is exactly the dishonesty
[invariant 4](../CLAUDE.md) exists to prevent
([NOTES § D8](../NOTES.md#d8--invariant-4-was-not-literally-true)). Every
command line after a switch carries `--context <name>` — honest, and it teaches
the flag that makes `kubectl` safe to use across clusters.

The **audit log** gets its own line (`context switched: prod-eu → staging`),
because which credentials were in use is the first question any trail has to
answer.

## What happens on `⏎`

The switch is the startup path, run again. It is not a special case, and
`k8s.rs` therefore exposes connecting as a function that can be called more
than once rather than as something `main` does at the top
([todo Phase 5](../todo.md#phase-5--live-reads--branch-featwatch--milestone-m15)).

1. Every watch stops; the snapshot store, findings, analysis results, table
   caches and open log streams are dropped. **Nothing from the old cluster
   survives the switch** — prod findings under a staging header is precisely
   the stale-drawn-as-live failure [states.md](states.md) forbids.
2. The command log is cleared and reopened with the new context's first line.
3. The header reads `ctx: staging · connecting…` and the body shows the
   loading screen that already exists in [states.md](states.md). No new state,
   no spinner ([widgets.md § 6](widgets.md#6-when-a-frame-is-drawn)).
4. Discovery, the capability probe and the namespace-scope fallback
   ([NOTES § D5](../NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter))
   all re-run. The new cluster's CRDs, its metrics-server, its permissions —
   none of it is inherited.
5. The view returns to **Alerts**, whichever view was open before. The
   sidebar of the old cluster may not exist in the new one.

`--read-only` is a property of the process, not of the context: it stays on
across a switch and the header keeps saying so.

## When the new cluster does not work

Startup failures print to stderr and exit ([states.md](states.md)) — but a
switch happens *inside* a running TUI, where that path does not exist. The
failure is a modal instead, and k8rs stays on the context the user chose:

```
                                 ctx: staging · ⚠ not allowed · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ staging said no ─────────────────────────────────────┐        │
│    │                                                      │        │
│    │  You switched to staging, but your user is not       │        │
│    │  allowed to list pods there.                         │        │
│    │                                                      │        │
│    │    Missing permission: list pods (cluster-wide)      │        │
│    │    User: dev@example.com                             │        │
│    │                                                      │        │
│    │  Nothing is wrong with prod-eu — X takes you back,   │        │
│    │  or ask for read access to staging.                  │        │
│    │                                                      │        │
│    │                  [ esc dismiss ]                     │        │
│    │                                                      │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context staging get pods -A   → not allowed            │
├────────────────────────────────────────────────────────────────────┤
│ X switch cluster   esc dismiss                                     │
└────────────────────────────────────────────────────────────────────┘
```

- **We do not silently fall back to the old context.** A header that says
  `staging` while the data is from `prod-eu` is the one thing this whole
  screen exists to prevent. The user asked for staging; they get staging, or
  they get told why not.
- Unreachable, 403 and "the context names a cluster the file does not define"
  are three different sentences, not one "connection failed".
- Never a dead end: the way out is on the screen, and it is the same key.

## Rules this screen adds

1. **Credentials come from the kubeconfig, still.** A switch selects a
   different context from the same file; it never accepts a server address,
   a token or a certificate typed by the user. The trust model does not move
   ([invariant 3](../CLAUDE.md)).
2. Context names, cluster names and server addresses come from a file on
   disk and are still untrusted text: they go through the same `sanitize()`
   as anything from the API ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
3. The old context's token is dropped with its client. Nothing is kept "in
   case they switch back" — reconnecting is cheap, a cached credential is not.
