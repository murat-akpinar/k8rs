# Screen — Choosing and switching cluster (startup · `X`)

[NOTES § Out of scope](../NOTES.md#out-of-scope-the-most-important-section)
says *"one context at a time, **switchable**"* — and nothing defined the
switch. `--context` was a startup flag only, which means the answer to "let me
check staging" was *quit and start again*, several times a day. This screen is
that gap closed ([NOTES § D16](../NOTES.md#d16--the-context-switcher)).

[NOTES § D116](../NOTES.md#d116--the-environment-picker-moves-to-startup-and-the-tag-comes-out-of-the-kubeconfig-itself-2026-08-19)
moves the same picker earlier: it now also opens **before** the first
connection, whenever the kubeconfig holds a real choice, so the cluster a
newcomer lands on is picked on purpose instead of inherited from whatever
`kubectl` command ran last. One modal, one list, one key map — everything
below is shared unless [§ Opening at startup](#opening-at-startup) says
otherwise.

## The picker

The same list, drawn once and used both ways: `X` opens it mid-session, or it
opens itself at startup when there is a real choice
([§ Opening at startup](#opening-at-startup)).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ Switch cluster ────────────────────────────────────────────┐   │
│   │                                                            │   │
│   │  ▸ prod-eu               aws · prod    (current)           │   │
│   │    staging                                                 │   │
│   │    kind-k8rs             ~local                            │   │
│   │    dev-cluster           ~aws          ⚠ TLS not verified  │   │
│   │                                                            │   │
│   │  staging  →  https://staging.internal:6443                 │   │
│   │                                                            │   │
│   │  k8rs does not change your kubeconfig — it just            │   │
│   │  talks to the cluster you pick here.                       │   │
│   │                                                            │   │
│   │         [ ⏎ switch ]     [ esc cancel ]                    │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
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
| The tag column | one context's own label, told apart from a guess. Fixed width, always in the same place regardless of how long the name to its left is — see [§ The tag column](#the-tag-column) for the full rule. |
| The server line | the API server address of the **selected** row, updated as you move. This is the "am I about to touch production" line, and it is why the address is not hidden behind a detail view. |
| `⚠ TLS not verified` | the context sets `insecure-skip-tls-verify`. Shown *before* the switch, not after — a beginner cannot be expected to infer it from a header they have not read yet. Shares its column with `(current)` — see the tie-break in [§ The tag column](#the-tag-column). |
| The sentence | k8rs never writes to `~/.kube/config`. Said on the screen because every user of `kubectl config use-context` will assume the opposite. |
| `/` | filters the list, exactly as `/` does in every other pane ([NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted)). |

**One context in the kubeconfig, and `X` is pressed:** the picker still opens
and says *"prod-eu is the only cluster in ~/.kube/config"*. A key that
appears to do nothing is worse than a screen that explains why.

**One context in the kubeconfig, at startup:** the picker does not open at
all — there is no choice to present, so k8rs connects to it exactly as it did
before this screen existed
([§ Opening at startup](#opening-at-startup)). `X` still works once k8rs is
running, and still shows the one-cluster sentence above; the two situations
read the same only because they share a sentence, not because the picker
opened twice.

## The tag column

The row is three slots, left to right, and only the first one is flexible:

| Slot | Width | Holds |
|---|---|---|
| name | flexible — takes whatever the terminal has left over the two fixed slots below, and clips at the cell boundary like any other over-long string ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)); no `…`, no manual truncation | the context name, however long |
| tag | **12 columns, fixed** | the tag — see below |
| badge | **20 columns, fixed** | `(current)` or `⚠ TLS not verified` — never both, see the tie-break below |

Fixed-and-flexible is the sidebar's own rule, reused: "the sidebar is a fixed
20 columns … the extra columns go to the content pane"
([widgets.md § 1](widgets.md#1-the-frame)). The tag and the badge stay put;
the name absorbs whatever is left. This is why a 90-character EKS ARN context
name is not a layout problem — it clips inside its own slot, and the tag two
slots later never moves. At this file's 70-column page width the name slot
renders 20 columns wide; a wider terminal only widens that one slot.

### Two kinds of tag

- **Written** — the person who owns the cluster put it in their kubeconfig:

  ```yaml
  contexts:
  - name: aws-prod
    context:
      cluster: prod-eu
      user: admin
      extensions:
      - name: k8rs
        extension: { tag: "aws · prod" }
  ```

  Shown **bright, no marker**. It is a statement by whoever wrote it — k8rs
  shows it exactly as given, sanitised the same as any other disk-file text
  ([rule 2](#rules-this-screen-adds)), and never presents it as a guess.

- **Derived** — k8rs guessed it from the API server host or the context
  name, because most contexts have no `extensions` block on day one:
  `amazonaws.com` → `aws`, `gke`/`googleapis` → `gcp`, `azmk8s.io` → `azure`,
  loopback or a `kind-`/`minikube`/`docker-desktop` name → `local`, anything
  else → blank. Shown **dim, and prefixed `~`** — `~aws`, `~local` — the same
  symbol-carries-the-fact-on-its-own rule the severity icons already use
  ([README § the five rules, item 4](README.md#the-five-rules-every-screen-obeys)):
  the tilde survives a monochrome terminal and a copy-paste into a chat
  message the same way `●` does. **A written tag always wins** — the
  heuristic runs only when `extensions` has none, never as a second opinion
  on one that exists.
- **Blank** — no `extensions` entry and no heuristic match. The normal case
  on day one, not an error state; see
  [§ Where the tag hint lives](#where-the-tag-hint-lives).

### A 60-character tag

```
┌────────────────────────────────────────────────────────────┐
│    aws-prod              production-u                      │
└────────────────────────────────────────────────────────────┘
```

Clipped at column 12 — same rule as the name column, same reason. There is no
`…` and nowhere to escape into and read the rest: the list *is* the whole
screen, not a row with a detail view behind it.

### A tag holding a control character or a right-to-left override

The tag is text off a disk file, exactly as untrusted as anything the API
sends ([rule 2](#rules-this-screen-adds),
[widgets.md § 7](widgets.md#7-text-that-came-from-the-api)). A kubeconfig
holding a tag value of `prod` + U+202E (RIGHT-TO-LEFT OVERRIDE) + `reversed`
— written here as the codepoint, never pasted as the character itself, for
the same reason it must never reach the screen — has that codepoint stripped
before the tag ever becomes a `Span`. A bidi-formatting character is exactly
the class [invariant 9](../CLAUDE.md) exists for, not a special case for tags:

```
┌────────────────────────────────────────────────────────────┐
│    aws-staging           prodreversed                      │
└────────────────────────────────────────────────────────────┘
```

The column never reverses, hides, or draws over what sits next to it.

### The badge tie-break

A row that is both the current context and has TLS verification off has one
20-column slot for two facts. `⚠ TLS not verified` wins: it is a safety
warning, and `(current)` is redundant with where the cursor already sat when
the picker opened.

## Where the tag hint lives

Most contexts are untagged on day one, and a blank column is not a problem to
nag about on every launch —
[NOTES § D116](../NOTES.md#d116--the-environment-picker-moves-to-startup-and-the-tag-comes-out-of-the-kubeconfig-itself-2026-08-19)
is explicit that the hint is shown once, not repeated on the picker every
time it opens. The YAML block that adds a tag belongs in `?` help:

```
  Cluster tags

    k8rs shows a short tag next to each context — aws, gcp, azure or
    a label of your own choosing. Add one to any context in your
    kubeconfig:

      contexts:
      - name: aws-prod
        context:
          cluster: prod-eu
          user: admin
          extensions:
          - name: k8rs
            extension: { tag: "aws · prod" }

    k8rs never writes this for you — there is no `kubectl config`
    command that can add one line to a list like this, so the hint
    is the YAML itself, never a command to run.
```

This block is content **for** [help.md](help.md), not an edit to it — wiring
it into the `?` screen is a separate box, outside what this one may write.
What is decided here is that it exists exactly once, lives in help and never
on the picker, and says plainly that k8rs will not write it for you.

## Opening at startup

```
                                k8rs        choose a cluster · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ Choose a cluster ──────────────────────────────────────────┐   │
│   │                                                            │   │
│   │  ▸ prod-eu               aws · prod    (current)           │   │
│   │    staging                                                 │   │
│   │    kind-k8rs             ~local                            │   │
│   │    dev-cluster           ~aws          ⚠ TLS not verified  │   │
│   │                                                            │   │
│   │  staging  →  https://staging.internal:6443                 │   │
│   │                                                            │   │
│   │  k8rs does not change your kubeconfig — it just            │   │
│   │  talks to the cluster you pick here.                       │   │
│   │                                                            │   │
│   │         [ ⏎ connect ]     [ esc quit ]                     │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl config get-contexts                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move   / filter   ⏎ connect   esc quit                          │
└────────────────────────────────────────────────────────────────────┘
```

| When | What happens |
|---|---|
| Two or more contexts, no `--context` | the picker above, unprompted, before anything else draws. |
| One context, or `--context` given | connects straight through, silently — exactly what happened before this screen existed. No picker, not even the one-cluster sentence from [§ The picker](#the-picker): there is nothing to ask. |
| `--once`, or stdin is not a terminal | never opens, regardless of how many contexts exist. `k8rs --once` answers one question on stdout and exits ([NOTES § D17](../NOTES.md#d17--the---once-output)); a picker in that path is a script that hangs forever. Ambiguity resolves the way it always did: `current-context`, silently. |

**Precedence, once:** `--context` beats the picker, the picker beats
`current-context`. *Zero configuration on first run*
([NOTES § Positioning, item 3](../NOTES.md#positioning--lazygit-for-kubernetes-user-2026-08-11))
stays true — the picker asks nothing that is not already in the file, and the
current context is preselected exactly as `(current)` marks it in
[§ The picker](#the-picker), so `⏎` with no other keypress lands on the same
cluster today's silent default would have picked.

What differs from the mid-session picker, and only this:

| | Mid-session (`X`) | Startup |
|---|---|---|
| Header | `ctx: prod-eu · live · admin` | no context chosen yet — `choose a cluster · admin` (or `· read-only`, the process flag is known before any connection); the centred name is dropped the same way the disconnected header already drops it first ([widgets.md § 1a](widgets.md#1a-the-header-row)) |
| `⏎` button | `[ ⏎ switch ]` | `[ ⏎ connect ]` — nothing is being switched *from* |
| `esc` | cancels — returns to the cluster already connected | **quits**, cleanly, same as `q` elsewhere — there is no cluster behind the modal yet |
| Behind the modal | the running app (not drawn in either mockup — the modal covers it either way) | genuinely nothing — the picker is the first thing drawn, not an overlay on a frame that already exists; the sidebar's labels are static, but they belong to the app frame, and that frame is not built until a cluster has been picked |

Identical, and not drawn twice above: the list, the tag column, `/` filter,
`↑↓`, the server line, `⚠ TLS not verified`, the "k8rs does not change your
kubeconfig" sentence, and the command log line — `$ kubectl config
get-contexts` reads the same local file either way, so it is the same line
whether the picker opened by itself or by `X`.

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
([todo Phase 5](../todo.md#phase-5--live-reads--milestone-m15)). Read
backwards: the startup picker's first `⏎` is not a special case of *this*
either — it is this function's first call, not a call that needed a
different one written for it
([§ Opening at startup](#opening-at-startup)).

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

Failures that happen **before the picker ever draws** — no kubeconfig, a
`--context` that names nothing in the file — still print to stderr and exit
([states.md](states.md)); nothing here changes that. But once the picker is
on screen, raw mode is already active, at startup exactly as much as
mid-session, and a connect failure gets the same modal either way, never a
stderr message. Mid-session, k8rs stays on the context the user chose:

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

### The same failure, from the startup picker

Only the header, the recovery sentence and the footer change — `X` is not
bound yet, so it cannot be the way back:

```
                                ctx: staging · ⚠ not allowed · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│    ┌ staging said no ─────────────────────────────────────┐        │
│    │                                                      │        │
│    │  You picked staging, but your user is not            │        │
│    │  allowed to list pods there.                         │        │
│    │                                                      │        │
│    │    Missing permission: list pods (cluster-wide)      │        │
│    │    User: dev@example.com                             │        │
│    │                                                      │        │
│    │  Nothing has connected yet — esc takes you back      │        │
│    │  to the list to try a different cluster.             │        │
│    │                                                      │        │
│    │              [ esc back to the list ]                │        │
│    │                                                      │        │
│    └──────────────────────────────────────────────────────┘        │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context staging get pods -A   → not allowed            │
├────────────────────────────────────────────────────────────────────┤
│ esc back to the list                                               │
└────────────────────────────────────────────────────────────────────┘
```

- **`esc` reopens the picker**, not a running app — there is not one yet to
  return to. Pressing `esc` again, now on the picker itself, quits — the same
  rule [§ Opening at startup](#opening-at-startup) already states. Two
  presses, never a dead end, never a third key nobody was told about.
- The header still names the attempted context (`ctx: staging · ⚠ not
  allowed`) even though nothing ever connected — the picker committed to
  trying it, and hiding that after the fact would be the exact
  stale-header failure this whole screen exists to prevent.

## Unhappy states

The five this screen has to answer for, and where each is decided:

1. **Exactly one context.** Startup: does not open, connects straight
   through, no message ([§ Opening at startup](#opening-at-startup)). `X`
   mid-session: still opens, still says so ([§ The picker](#the-picker)).
2. **A tag 60 characters long.** Clips at column 12, no `…`
   ([§ The tag column](#the-tag-column)).
3. **A tag holding a control character or a right-to-left override.**
   Stripped before it is drawn, same as any other untrusted disk-file text
   ([§ The tag column](#the-tag-column)).
4. **A context whose cluster the file does not define.** Below.
5. **`--once`, or stdin that is not a terminal.** Never opens,
   `current-context` connects silently
   ([§ Opening at startup](#opening-at-startup)).

### A context whose cluster the file does not define

`kube::config::Kubeconfig` parses the whole file in one read, so this is
known before the first keypress — no wasted connection attempt is needed to
find out. The row still appears, in file order like every other row, but
dimmed and **unreachable by the cursor** — `↑` / `↓` skip it exactly as if it
were not in the list:

```
┌────────────────────────────────────────────────────────────┐
│    old-cluster                         ⚠ cluster undefined │
└────────────────────────────────────────────────────────────┘
```

- No tag is derived for it — there is no server host to derive one from.
- The server line never shows for it, because the cursor can never land on
  it.
- **Left open:** if this is true of *every* context in the file, or of
  `current-context` itself with only that one context present, nothing above
  resolves what happens next. That is indistinguishable from a kubeconfig
  that cannot build a client at all, which is already a
  before-the-TUI stderr exit
  ([states.md § Before the TUI ever starts](states.md#before-the-tui-ever-starts));
  whether it should stay that way or fall into this screen's dimmed-row
  treatment is a call for whoever wires discovery, not a mockup.

## Rules this screen adds

1. **Credentials come from the kubeconfig, still.** A switch selects a
   different context from the same file; it never accepts a server address,
   a token or a certificate typed by the user. The trust model does not move
   ([invariant 3](../CLAUDE.md)).
2. Context names, cluster names, tags and server addresses come from a file
   on disk and are still untrusted text: they go through the same
   `sanitize()` as anything from the API
   ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
3. The old context's token is dropped with its client. Nothing is kept "in
   case they switch back" — reconnecting is cheap, a cached credential is not.
4. **k8rs never writes a tag**, the same way it never runs
   `kubectl config use-context` — nothing in this repo edits
   `~/.kube/config`. A derived tag is computed fresh every time the list is
   drawn, never cached into the file, and the `?` help hint is YAML to paste
   by hand, never a command k8rs offers to run for you
   ([§ Where the tag hint lives](#where-the-tag-hint-lives)).
5. **The picker is structurally absent, not merely unbound, under `--once`
   and when stdin is not a terminal** — the same distinction
   [invariant 2](../CLAUDE.md) draws for `--read-only`.
