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
│   │  prod-eu  →  https://prod-eu.internal:6443                 │   │
│   │                                                            │   │
│   │  k8rs does not change your kubeconfig — it just            │   │
│   │  talks to the cluster you pick here.                       │   │
│   │                                                            │   │
│   │               [ ⏎ switch ]    [ esc cancel ]               │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl config get-contexts                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  / filter  ⏎ switch  esc cancel                            │
└────────────────────────────────────────────────────────────────────┘
```

| Part | Rule |
|---|---|
| The list | every context in the kubeconfig, in file order. `kube::config::Kubeconfig` already parses it — no new dependency, no second parser, and no file of our own. More rows than the box shows scroll under `↑`/`↓` — see [§ More contexts than fit](#more-contexts-than-fit). |
| `(current)` | the context named `current-context`, and the row selected when the picker opens. `⏎` on it closes without doing anything, **but only while that context is actually connected**; after a failed switch the header still names it even though nothing is live, so `⏎` there connects instead of closing — the same retry a fresh `X` would start ([NOTES § D264 ruling 4](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)). |
| The tag column | one context's own label, told apart from a guess. Fixed width, always in the same place regardless of how long the name to its left is — see [§ The tag column](#the-tag-column) for the full rule. |
| The server line | the API server address of the **selected** row, updated as you move. This is the "am I about to touch production" line, and it is why the address is not hidden behind a detail view. |
| `⚠ TLS not verified` | the context sets `insecure-skip-tls-verify`. Shown *before* the switch, not after — a beginner cannot be expected to infer it from a header they have not read yet. Shares its column with `(current)` — see the tie-break in [§ The tag column](#the-tag-column). |
| The sentence | k8rs never writes to `~/.kube/config`. Said on the screen because every user of `kubectl config use-context` will assume the opposite. |
| `/` | filters the list, exactly as `/` does in every other pane ([NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted)) — matching what the row itself draws, `(unnamed)` and `~` included, and never the server address or a badge. Hiding every row this way is its own state — [§ The filter hides every row](#the-filter-hides-every-row). **While `/` holds text, `esc` clears it first rather than doing its ordinary thing, and both the footer and the box's own `[ esc … ]` button say so** — `esc clear filter` in place of `esc cancel` (mid-session) or `esc quit` (startup), on the button as well as the footer, so the key is never spelled two ways in one frame ([NOTES § D264 rulings 27 and 31](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)). |
| `⏎` | connects, or closes on a live `(current)` row above. **It is not always live** — a shadowed row, no row selected at all, and a filter that hides every row all leave nothing for it to do, and each of those draws the button dim, drops `⏎` from the footer, and says why in the server-line slot rather than leaving a key that looks bound and does nothing ([NOTES § D264 ruling 2](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)). |

**One context in the kubeconfig, and `X` is pressed:** the picker still opens
and says *"prod-eu is the only cluster in your kubeconfig."* A key that
appears to do nothing is worse than a screen that explains why. **The name
gives way from its front only when the name alone is wider than the
line** ([NOTES § D264 ruling 21](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13))
— otherwise the sentence wraps, the same as any other sentence on this
screen, and the name is drawn whole. The sentence sits in the same slot the
server line and the kubeconfig sentence do — the nested box's own interior
at the real 80×24 floor, at the same 2-column indent. An ARN-named context
for the same cluster — `arn:aws:eks:eu-west-1:111122223333:cluster/payments-prod`,
56 characters — is not wider than the line by itself, so it wraps rather
than cuts:

```
│  arn:aws:eks:eu-west-1:111122223333:cluster/payments-prod is the only│
│  cluster in your kubeconfig.                                         │
```

Only a name wider than the line on its own still gives way from its front,
behind a visible `…` — the same rule [§ The tag column](#the-tag-column)
uses for the row it sits above.

**One context in the kubeconfig, at startup:** the picker does not open at
all — there is no choice to present, so k8rs connects to it exactly as it did
before this screen existed
([§ Opening at startup](#opening-at-startup)). `X` still works once k8rs is
running, and still shows the one-cluster sentence above; the two situations
read the same only because they share a sentence, not because the picker
opened twice.

### More contexts than fit

The box shows as many rows as its height allows and the rest scroll under
`↑`/`↓`, the cursor always inside the window. **The rows are drawn as
`Paragraph` lines, not a `List`** — the picker's list, slot and buttons all
share the one box — **and the scroll offset is derived fresh from the
selection every frame rather than kept anywhere between them**, the same
value a fresh `ListState` of that height would give, measured at 1 through
300 rows
([widgets.md § 4](widgets.md#4-scrolling),
[NOTES § D264 ruling 26](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
**The box's height is not
capped at a dialog's usual ceiling: it grows with the terminal, showing more
contexts on a taller one rather than stopping at the confirm-dialog size**
([NOTES § D264 ruling 9](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) —
an operator with fifteen contexts wants to see as many of them at once as the
window allows.

**Whatever is still left over is not silently missing.** Six contexts drawn
at the 80×24 floor once left three rows on screen with no mark of any kind —
a reader at startup had no way to know the other three existed at all. The
fix reuses this product's one convention for a pane taller than its own
viewport rather than inventing a wording of its own: a scrollbar, drawn only
once the list does not fit, the same rule and the same widget
[analysis.md](analysis.md#capacity-when-you-can-only-see-one-namespace)
already applies to a long node list — *"the pane scrolls, and the scrollbar
appears only once the content is taller than the viewport."* **Not `N more`
or `and N more`**: both of this product's other two conventions name a list
that is capped and read by pressing a key, never one a reader scrolls through
themselves, and a static count beside a list that is itself moving would go
stale the moment `↓` is pressed. A scrollbar's thumb answers the one
question a hidden row raises — *is there more below* — without a number this
file would then have to keep in sync with the cursor.

## The tag column

The row is three slots, left to right, and only the first one is flexible:

| Slot | Width | Holds |
|---|---|---|
| name | flexible — takes whatever the terminal has left over the two fixed slots below, and **gives way from its front behind a visible `…` when it does not fit** — the header's own rule ([widgets.md § 1a](widgets.md#1a-the-header-row), [NOTES § D264 ruling 5](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) | the context name, however long |
| tag | **12 columns, fixed** | the tag — see below |
| badge | **20 columns, fixed** | `(current)` or `⚠ TLS not verified` — never both, see the tie-break below |

Fixed-and-flexible is the sidebar's own rule, reused: "the sidebar is a fixed
20 columns … the extra columns go to the content pane"
([widgets.md § 1](widgets.md#1-the-frame)). The tag and the badge stay put;
the name absorbs whatever is left. This is why a 90-character EKS ARN context
name is not a layout problem — it gives way inside its own slot, and the tag
two slots later never moves. At this file's 70-column page width the name
slot renders 20 columns wide; a wider terminal only widens that one slot.

**Which end gives way is not a free choice, and clipping at the tail with no
mark was measured wrong rather than merely plain.** `aws eks
update-kubeconfig` names a context after the *cluster*, not after the
environment, so a fleet of them shares one long prefix and differs only near
the end —
`arn:aws:eks:eu-west-1:111122223333:cluster/payments-prod` and
`arn:aws:eks:eu-west-1:111122223333:cluster/payments-staging` at 80×24 drew
as the identical `arn:aws:eks:eu-west-1:111122223333:cluster/paymen` with the
tail cut and no mark to say so — three such contexts were not merely hard to
tell apart, they were indistinguishable. The name now gives way from its
**front**, behind a visible `…`, the same rule the header already uses for
the same reason ([widgets.md § 1a](widgets.md#1a-the-header-row),
[NOTES § D249](../NOTES.md#d249--the-layout-box-lands-from-a-second-session-the-header-gives-way-from-its-front-and-a-refusal-keeps-the-list-it-is-about-2026-09-06)):
what a name shares with its neighbours sits at the front, so that is the part
it can afford to lose. The tag slot keeps its own tail clip and no mark
([§ A 60-character tag](#a-60-character-tag)) — a written tag is read the
ordinary way, from its front, so cutting its *end* loses nothing the reader
was scanning for first.

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
before the tag ever becomes a `Span`, by the same predicate every other
untrusted string on this screen goes through: `k8s::unprintable`, Unicode's
own control category (`Cc`) plus the zero-width and bidi-formatting ranges —
the embedding/override block (U+202A–U+202E, which is where U+202E sits),
the zero-width block (U+200B–U+200F, which is where the zero-width
joiner/non-joiner and the left/right-to-left marks sit), U+00AD SOFT HYPHEN,
U+FEFF ZERO WIDTH NO-BREAK SPACE, and the invisible-operator block that
carries the bidi isolates (U+2066–U+2069). **U+2028, U+2029 and U+00A0 are
deliberately kept** — a terminal draws something for each of them — and why
the zero-width joiner is removed anyway despite being load-bearing elsewhere
is
[NOTES § D154](../NOTES.md#d154--the-browsers-rows-a-37-that-was-one-event-a-floor-measured-from-the-answer-and-a-guard-that-stopped-at-cc-2026-08-22)'s
to say, not restated here. A bidi-formatting character is exactly the class
[invariant 9](../CLAUDE.md) exists for, not a special case for tags:

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

### Three contexts told apart only by their tail

Drawn at the real 80×24 floor rather than this file's usual 70-column page,
because the point being made — three names that are identical at 70 columns
of page width are not identical at the terminal's own width, and the fix has
to be shown at that width to be believed. **Same box, same rules as
[§ The picker](#the-picker), not a second geometry for a wider terminal**
([NOTES § D264 ruling 19](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):
the server line and the "k8rs does not change your kubeconfig" sentence sit
at the same 2-column indent as every row in § The picker's own box, and the
whole frame is still 24 rows tall, the same floor § The picker's own
four-context box is drawn at. The box itself is not: it sizes its list to
the rows it shows, so three contexts draw a 14-row box, one row shorter than
a four-context box would need. **The frame's own height never depends on
how many contexts happen to be in the file** — the body always reserves one
blank row above the box and one below it, whatever the box's own height is,
and the command log always draws two rows, the second one blank until a
second command has run
([NOTES § D264 ruling 21](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):

```
 nodes 3/3                            k8rs         ctx: prod-eu · live · admin
┌──────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│   ┌ Switch cluster ──────────────────────────────────────────────────────┐   │
│   │                                                                      │   │
│   │  ▸ …2223333:cluster/payments-prod  ~aws                              │   │
│   │    …3333:cluster/payments-staging  ~aws                              │   │
│   │    …22223333:cluster/payments-dev  ~aws                              │   │
│   │                                                                      │   │
│   │  …uster/payments-prod  →  https://B4E2.eu-west-1.eks.amazonaws.com   │   │
│   │                                                                      │   │
│   │  k8rs does not change your kubeconfig — it just                      │   │
│   │  talks to the cluster you pick here.                                 │   │
│   │                                                                      │   │
│   │                    [ ⏎ switch ]    [ esc cancel ]                    │   │
│   │                                                                      │   │
│   └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl config get-contexts                                                │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  / filter  ⏎ switch  esc cancel                                      │
└──────────────────────────────────────────────────────────────────────────────┘
```

Three real names — `arn:aws:eks:eu-west-1:111122223333:cluster/payments-prod`,
`…-staging`, `…-dev` — share their first 52 characters and every one of them
is now legible on its own row, because those 52 shared characters are the
part each row gives up. Read against the header's own reasoning: the front is
what repeats across a fleet, and repeats are what a reader has already seen
by the second row.

**The front cut is not a general answer, and it does not try to be.** It
separates the EKS shape above because EKS names a context after the cluster
last. A GKE fleet's `gke_<project>_<location>_<cluster>` can share its own
cluster name across two different projects and look identical after the same
cut, and OpenShift's `<namespace>/<server>/<user>` differs at either end, not
only the front — no single cut tells every naming scheme apart at 80×24, and
two GKE autopilot defaults first differ at 85 columns, past what a terminal
this size can show at all. The remedy for either shape is a written tag or a
wider terminal — not the server line under the selected row: that line is
unique per cluster, but it does not tell an operator *which* cluster, only
that the two differ — a bare IP on GKE, a hash hostname on EKS. A written
tag is the fix this page already teaches
([NOTES § D264 rulings 24 and 32](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).

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
│   │  prod-eu  →  https://prod-eu.internal:6443                 │   │
│   │                                                            │   │
│   │  k8rs does not change your kubeconfig — it just            │   │
│   │  talks to the cluster you pick here.                       │   │
│   │                                                            │   │
│   │               [ ⏎ connect ]    [ esc quit ]                │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl config get-contexts                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  / filter  ⏎ connect  esc quit                             │
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
| Behind the modal | **cleared**, the same as at startup — `Clear` paints over the running app before the box is drawn, so nothing of it can show through the box's own margins ([NOTES § D264 ruling 9](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) | genuinely nothing — the picker is the first thing drawn, not an overlay on a frame that already exists; the sidebar's labels are static, but they belong to the app frame, and that frame is not built until a cluster has been picked |

**Which variant is drawn is decided by whether any context has connected
*this run*, never by who opened the picker or how many times it has already
been shown** ([NOTES § D264 ruling 4](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
A failed startup connection reopens this same startup picker, `esc` still
quits, and a second and third attempt read exactly like the first — never
sliding into the mid-session chrome just because the picker has now been
drawn more than once. The reverse holds too: after a **mid-session** switch
fails, nothing is live, and the header keeps naming the context `X` just
tried to reach — the same header the failure box itself draws below — never
the one that was live before `X` was pressed
([D16](../NOTES.md#d16--the-context-switcher) ruling 3: a failed switch does
not fall back; [NOTES § D264 ruling 4](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
A fresh `X` still opens the mid-session picker — the variant is still
ruling 4's, because a context did connect earlier this run — and `(current)`
there marks that same attempted context, not a live one, so `⏎` on it
connects again rather than merely closing, exactly as the `(current)` row's
own rule in [§ The picker](#the-picker) already says. `esc` on that reopened
picker cancels back to the same nothing-is-live state the failed switch left
behind, not to a cluster that is actually live.

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
  the footer: *"changing payments/web first"*
  ([dialogs.md § While the call is running](dialogs.md#while-the-call-is-running)).
  Swapping the client out from under an in-flight mutation is how an
  operation gets attributed to the wrong cluster in the audit log.

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

**A switch writes no line of its own to the audit log**
([NOTES § D264 ruling 10](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13),
[D16](../NOTES.md#d16--the-context-switcher) ruling 1) — every attempt line
already carries `context … · server …`, which is the fact a switch's own line
would have existed only to repeat.

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
stderr message. Mid-session, k8rs stays on the context the user chose.
**Drawn over the app, like every dialog** — the mockup below does not draw
the running body behind the box, the same convention every other dialog on
this product follows; [§ Opening at startup](#opening-at-startup)'s
*"behind the modal: cleared"* is the picker's own rule and does not carry
over to this failure box, which sits on top of a frame that already exists
rather than replacing one that does not
([NOTES § D264 ruling 19](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):

```
                                 ctx: staging · ⚠ not allowed · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│      ┌ staging said no ─────────────────────────────────────┐      │
│      │                                                      │      │
│      │  The role this kubeconfig uses needs to `list` and   │      │
│      │  `watch` pods across the whole cluster. Ask whoever  │      │
│      │  runs this cluster for a role that may read pods in  │      │
│      │  every namespace — `k8rs-readonly` in the k8rs docs  │      │
│      │  is that role — or quit and start k8rs again in one  │      │
│      │  namespace you can read: --namespace <name>          │      │
│      │                                                      │      │
│      │  Nothing is wrong with prod-eu — X takes you back.   │      │
│      │                                                      │      │
│      │                    [ esc dismiss ]                   │      │
│      │                                                      │      │
│      └──────────────────────────────────────────────────────┘      │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context staging get pods -A --watch   → not allowed    │
├────────────────────────────────────────────────────────────────────┤
│ esc dismiss                                                        │
└────────────────────────────────────────────────────────────────────┘
```

**Every word inside the box is `because`'s and `pods_unread`'s, not a second
vocabulary invented for this screen** ([NOTES § D264 ruling 1](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
Read as two sentences run together, not two separate fields with their own
labels: *"the role this kubeconfig uses needs to `list` and `watch` pods
across the whole cluster"* is `because(Fault::Refused, …)`'s own sentence
with `pods_unread`'s scope clause folded onto its end — the same words
[states.md § Before the TUI ever starts](states.md#before-the-tui-ever-starts)
draws as two labelled lines (*"What k8rs asked for" / "What happened"*) for a
wider pane;
here they are one flowing paragraph because a 54-column dialog has no room
to spend on labels the sentence does not need to repeat itself to keep. The
second sentence is `pods_unread`'s own per-`Coverage` next step, verbatim —
the drawing above is `Coverage::Cluster`'s, the one this cluster's
capability probe actually produces when nothing has been asked for by name.
**The next step's `--namespace` arm is worded for a k8rs that is already
running** — *quit and start k8rs again in one namespace you can read*, never
merely *run k8rs in one* — because this box is only ever reached from a
running one; `k8rs --once` reads the same next step differently, and that
copy lives on `once.md`, not here
([NOTES § D264 ruling 23](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).

- **The footer reads `esc dismiss` alone — `X` is not there to press.**
  [D16](../NOTES.md#d16--the-context-switcher) ruling 1 is explicit: `X`
  cannot fire while a modal is open, and this refusal is a modal. A footer
  that showed `X switch cluster` here would promise a key that does nothing
  until `esc` has already closed this box — exactly the bug
  [widgets.md § The footer](widgets.md#2a-the-footer) forbids of every modal.
  The body's own *"X takes you back"* still says what to do next; it is
  read after dismissing, not instead of it.
- **`[ esc dismiss ]` puts its spare column on the left, not the right** — 54
  columns of interior width less 15 for the button leaves 39, an odd number
  that cannot split evenly, and the extra column goes to the same side
  [dialogs.md](dialogs.md)'s two 54-wide dismiss boxes already put it on
  ([§ The cluster said no](dialogs.md#the-cluster-said-no),
  [§ The object went away](dialogs.md#the-object-went-away-while-the-dialog-was-open),
  [NOTES § D264 ruling 19](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) —
  one box, one rule, not a coin flip decided fresh per screen.
- **We do not silently fall back to the old context.** A header that says
  `staging` while the data is from `prod-eu` is the one thing this whole
  screen exists to prevent. The user asked for staging; they get staging, or
  they get told why not.
- Never a dead end: the way out is on the screen, and it is the same key.
- **No `User:` line.** k8rs never reads a display name for the identity a
  kubeconfig authenticates as — there is no field here to print, honest or
  otherwise, the same reasoning that dropped the equivalent line from the
  `--once` report
  ([NOTES § D190](../NOTES.md#d190--the-screen-that-ships-first-promises-four-things-the-binary-does-not-do-and-nobody-had-read-them-against-each-other-2026-08-30)).
  A line the binary cannot produce is a promise this screen cannot keep.

### The scope changes the next step, not just a number

`Coverage::namespace()` — the accessor every other screen reads — answers
the same `Some(payments)` whether the reader typed `--namespace payments`
and was refused, or the kubeconfig named no namespace at all and k8rs had to
guess one and was refused there too. Those are not the same next step, and a
box built only from that collapsed answer sends the second reader to ask for
access to a namespace they never chose
([NOTES § D264 ruling 1](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13),
`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1). The box
reads `k8s::Coverage` itself, not its collapsed `namespace()`, so the two
read differently — each quoted exactly as `next_step` returns it, with no
full stop added that the function did not write:

```
Refused in a namespace the reader named:

  Ask whoever runs this cluster for a role that may
  read pods in payments — the same rules as
  `k8rs-readonly` in the k8rs docs, granted in one
  namespace instead of all of them

Refused after k8rs had to guess the namespace:

  This kubeconfig names no namespace, so k8rs had to
  guess default and was refused there too. Quit and
  start k8rs again in the namespace you work in:
  --namespace <name>
```

Both replace the same paragraph in the box above, in place of
`Coverage::Cluster`'s sentence — the opening reason
(*"…needs to `list` and `watch` pods in the namespace {ns}"*) is identical
either way, because the role a login is missing does not depend on how k8rs
arrived at the namespace it tried.

### Every other fault has its own sentence, and this page does not retype it

Eleven `k8s::Fault` variants can reach this box — twelve rows below, because
`Unanswered` draws two different sentences depending on whether a request
went out — and `because` already answers every one of them without a second
wording living here
([NOTES § D264 ruling 1](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
`403` is the only one this page draws in full; for the rest, the title names
the class the fault falls into and the sentence is `because`'s own — quoted
on screen exactly as that function returns it, never paraphrased here where
a second copy would go stale first:

| Fault | A request went out? | Title | The sentence on screen |
|---|---|---|---|
| `Refused` (`403`) | yes | `<name> said no` | drawn in full above — the only one this page draws in full, quoted from `because` and `next_step` |
| `Expired` (`401`) | yes | `<name> said no` | `because`'s `Expired` sentence — names the login program when the kubeconfig has one |
| `Rejected` (`400`) | yes | `<name> said no` | `because`'s `Rejected` sentence — quotes the cluster if it explained itself, else says the fault is k8rs's own |
| `Gone` (`404`) | yes | `<name> said no` | `because`'s `Gone` sentence, plus its own next step — *Check the server address this kubeconfig names — as written, it does not lead to a Kubernetes API server* ([ruling 22](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13), [ruling 32](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) |
| `Conflict` (`409`) | yes | `<name> said no` | `because`'s `Conflict` sentence |
| `Unanswered` | yes | `<name> did not answer` | `because`'s `Unanswered` sentence, plus its own next step — check the server address this kubeconfig names |
| `Unfinished` | yes | `<name> did not answer` | `because`'s `Unfinished` sentence |
| `Kubeconfig` | **no** | `<name> could not be opened` | `because`'s `Kubeconfig` sentence, asked as *reach this cluster* — nothing was ever sent |
| `NoContext` | **no** | `<name> could not be opened` | `because`'s `NoContext` sentence, asked as *reach this cluster* |
| `BadEntry` | **no** | `<name> could not be opened` | `because`'s `BadEntry` sentence, asked as *reach this cluster* |
| `NoCredential` | **no** | `<name> could not be opened` | `because`'s `NoCredential` sentence, asked as *reach this cluster* — names the login program when there is one; no next step of its own ([ruling 32](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) |
| `Unanswered`, its client never built | **no** | `<name> could not be opened` | `because`'s `Unanswered` sentence, asked as *reach this cluster* — no next step; the same sentence `--once` already prints for this case, a known limit rather than a fifth wording ([ruling 28](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13), `backlog.md`) |

**`Gone`'s next step answers a question whose outcome was actually
measured, not invented to fill the row** ([NOTES § D264 ruling 22](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):
a stand-in server answering `/api/v1/pods` with a plain `404` is what a
wrong address looks like. **`NoCredential` has no next step of its own** —
an `exec` plugin's own diagnosis goes only to the terminal's stderr, never
to k8rs, so the diagnosis is the login program's own output, not a sentence
this screen can write in its place; getting that output back to the reader
is Phase 12's terminal handover to do, not this box
([NOTES § D264 ruling 32](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
`Gone`'s next step is drawn the same way whether the box is reached from a
fresh connection or from the `## When the new cluster does not work` boxes
on a watch that fails mid-session.

**"A request went out?" is what decides the title, not the HTTP class.** The
four faults marked **no** above never reached the network — the client
itself could not be built — so a title claiming the cluster *said* anything
would be false about a call that was never made; the box is asked as *reach
this cluster* for those four, and for one more below, `live`'s own framing
for a `NotConnected` that sent nothing. **`Unanswered` is the one fault that
appears both ways**: most of the time a request went out and nothing usable
came back before the fault fired, drawn `did not answer`; the rarer case
where its own client was never built either draws `could not be opened`,
the same sentence with no request to blame it on. Every other fault sent
the same request
this page draws in full — `` `list` and `watch` pods `` — because the
capability probe that opens the permanent pods watch is the one request a
fresh connection makes. **This holds whatever path the fault arrives by, not
only a switch from this picker** ([NOTES § D264 ruling 17](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):
a `NoCredential` that shows up mid-session, when the permanent pods watch's
own reconnect cannot rebuild a client, is titled the same `<name> could not
be opened`, never `did not answer` — that attempt never reached the network
any more than one started from this picker does.
**The way-out sentence is unchanged across every row of this table, and
which of the two it is turns on whether anything has connected this run, not
on which of the eleven faults this was or which picker the reader opened**
([NOTES § D264 ruling 15](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):
*"Nothing is wrong with {the context that was last live} — X takes you
back"* once a context has connected, even when the switch that just failed
is the second or third in a row, *"Nothing has connected yet — esc takes you
back to the list"* only for a run that has never connected at all.

### The same failure, from the startup picker

Only the header, the recovery sentence and the footer change — `X` is not
bound yet, so it cannot be the way back:

```
                                ctx: staging · ⚠ not allowed · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│      ┌ staging said no ─────────────────────────────────────┐      │
│      │                                                      │      │
│      │  The role this kubeconfig uses needs to `list` and   │      │
│      │  `watch` pods across the whole cluster. Ask whoever  │      │
│      │  runs this cluster for a role that may read pods in  │      │
│      │  every namespace — `k8rs-readonly` in the k8rs docs  │      │
│      │  is that role — or quit and start k8rs again in one  │      │
│      │  namespace you can read: --namespace <name>          │      │
│      │                                                      │      │
│      │  Nothing has connected yet — esc takes you back to   │      │
│      │  the list to try a different cluster.                │      │
│      │                                                      │      │
│      │               [ esc back to the list ]               │      │
│      │                                                      │      │
│      └──────────────────────────────────────────────────────┘      │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context staging get pods -A --watch   → not allowed    │
├────────────────────────────────────────────────────────────────────┤
│ esc back to the list                                               │
└────────────────────────────────────────────────────────────────────┘
```

This is the tallest either box draws — thirteen rows between the nested
borders, the ceiling [widgets.md § 5](widgets.md#5-the-modal-layer) sets for
any dialog, with none left to spend
([NOTES § D264 ruling 1](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).

- **`esc` reopens the picker**, not a running app — there is not one yet to
  return to. Pressing `esc` again, now on the picker itself, quits — the same
  rule [§ Opening at startup](#opening-at-startup) already states. Two
  presses, never a dead end, never a third key nobody was told about.
- The header still names the attempted context (`ctx: staging · ⚠ not
  allowed`) even though nothing ever connected — the picker committed to
  trying it, and hiding that after the fact would be the exact
  stale-header failure this whole screen exists to prevent.
- **The picker `esc` reopens is the same `startup: true` picker that opened
  this attempt, not a fresh one** — a second and third failed connection in
  a row still read as the startup variant throughout, never sliding into the
  mid-session chrome partway through
  ([§ Opening at startup, what differs](#opening-at-startup)).

## Unhappy states

The eleven this screen has to answer for, and where each is decided:

1. **Exactly one context.** Startup: does not open, connects straight
   through, no message ([§ Opening at startup](#opening-at-startup)). `X`
   mid-session: still opens, still says so ([§ The picker](#the-picker)).
2. **A tag 60 characters long.** Clips at column 12, no `…`
   ([§ The tag column](#the-tag-column)).
3. **A tag holding a control character or a right-to-left override.**
   Stripped before it is drawn, same as any other untrusted disk-file text
   ([§ The tag column](#the-tag-column)).
4. **A context whose cluster the file does not define.** Below.
5. **A context whose name strips to nothing.** Below.
6. **A context defined twice.** Below.
7. **A server address k8rs will not guess at.** Below.
8. **No row is both current and landable.** Below.
9. **The filter hides every row.** Below.
10. **The kubeconfig has no contexts at all when `X` is pressed.** Below.
11. **`--once`, or stdin that is not a terminal.** Never opens,
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
- **If this is true of every context in the file, or of `current-context`
  itself with only that one context present, the picker still opens with no
  row landable at all** — the drawing
  [§ No row is both current and landable](#no-row-is-both-current-and-landable)
  has for that case, not the one where some other row still is
  ([NOTES § D264 ruling 16](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
  This is no longer indistinguishable from a kubeconfig that cannot build a
  client at all, because a client is not what is missing here — the file
  parsed and every entry is real, only none of them names a cluster the file
  defines. That other failure is still a before-the-TUI stderr exit
  ([states.md § Before the TUI ever starts](states.md#before-the-tui-ever-starts))
  and stays one; this one is a picker with nothing anywhere to land on, not a
  picker that cannot open at all.

### A context whose name strips to nothing

`name: ""`, or a name made only of characters invariant 9 strips: a context
that is really in the file but has nothing left to put in the name slot once
the same clean-up every other name goes through has run
([NOTES § D173](../NOTES.md#d173--the-tags-matching-rules-tightened-against-the-object-rather-than-the-wording-and-the-credential-the-server-line-was-drawing-2026-08-28)).
Nothing here opens the wrong cluster — `Choice::key` keeps the file's own
spelling, so `⏎` still lands on the right entry — but a screen that prints
nothing where a name belongs reads as *no context*, and if this is the row
`current-context` points at, the header would say that about the very
cluster the run is already on.

```
┌────────────────────────────────────────────────────────────┐
│    (unnamed)                           (current)           │
└────────────────────────────────────────────────────────────┘
```

- **`(unnamed)` can collide with a real context name, and that is
  accepted, not solved.** `(current)` is safe because it lives in a badge
  column no disk text ever reaches; `(unnamed)` sits in the name slot,
  where disk text does reach, and `name: "(unnamed)"` is a legal context
  name that would render identically. No literal word is collision-proof,
  so this is not a search for a safer one. What survives the collision is
  correctness, not distinguishability: `Choice::key` keeps the file's own
  spelling for both rows, so each still opens its own entry — a reader who
  has one real context named `(unnamed)` and one whose name stripped to
  nothing cannot tell the two rows apart by name alone, but pressing `⏎`
  on either one still opens the cluster that row actually is.
- **The row is drawn like any other, not dimmed.** This is not the *cluster
  undefined* treatment above — that row is hidden from the cursor because
  there is nowhere for `⏎` to go. Here `⏎` goes somewhere fine; the row
  simply has no name to print. Its tag, server line and badges all behave
  exactly as they would for a named row.
- **The header carries the same word.** `ctx: (unnamed) · live · admin` —
  never the *no context chosen yet* wording reserved for
  [§ Opening at startup](#opening-at-startup). That sentence means no
  cluster has been picked; this context has been picked, k8rs just cannot
  put its name on screen. One placeholder, drawn in both places, so the
  header and the picker cannot disagree about whether a context is in use.

### A context defined twice

Two entries in the kubeconfig share a name — `Choice::shadowed`. Every
lookup that opens a context by name — kube's own loader, `--context`, this
picker's `⏎` — finds the *first* match, so the second entry can never be
the one that opens
([NOTES § D174](../NOTES.md#d174--the-operator-review-of-the-kubeconfig-family-ten-fixed-one-refused-and-the-two-reversals-it-forced-2026-08-28)).
`kubectl` refuses a kubeconfig shaped like this outright; k8rs is the only
tool left in the reader's terminal willing to open the file, which is why it
owes them the sentence kubectl never gets the chance to say:

```
┌────────────────────────────────────────────────────────────┐
│    prod-eu                             ⚠ duplicate name    │
└────────────────────────────────────────────────────────────┘
```

- **The cursor can land on it.** It is drawn dim — the same signal
  *cluster undefined* uses for "not a normal choice" — but `↑` / `↓` do not
  skip it. Skipping it would hide the one thing this row is for: telling the
  reader their file has a duplicate they cannot see anywhere else. Its own
  address and tag are real and are never blanked — they describe the entry
  actually written at this position, even though `⏎` cannot reach it as
  itself. (Its namespace is real too, kept for the same reason, even though
  this screen has nowhere to print one yet.)
- **`⏎` on it changes nothing and the picker stays open** — not the
  `(current)` row's behaviour, even though both are places `⏎` is
  deliberately inert. `⏎` on `(current)` *closes* the picker, because
  landing there and confirming means "yes, stay on this cluster" — the
  picker's job is done. Closing here would do the opposite: it would eject
  the reader from the one screen explaining a kubeconfig problem they still
  need to act on, and either read as a connection that just happened or as
  the picker being broken. So the picker stays open, no popup fires — the
  sentence explaining why is already on screen the moment the row is
  selected — and the reader is left free to move to the real entry above or
  press `esc` when they are done reading.
- **The `[ ⏎ … ]` button draws dim while this row is selected, and the
  footer drops `⏎` too** — a key that would do nothing is not offered as
  though it would
  ([NOTES § D264 ruling 2](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
  The sentence in place of the address line already says why, so nothing new
  is added there for this case — the missing button and the shortened
  footer are the only marks.
- **Selecting it replaces the address line, not the badge.** The badge says
  *what* — `⚠ duplicate name` — and the line below the list says *why*, in
  place of the usual `name → address` — naming what a lookup by this name
  finds, never claiming that entry itself is reachable, since the earlier
  one can be broken too:

  ```
  prod-eu  —  another context earlier in this file is also
              named prod-eu. Every lookup by that name, ⏎
              here included, finds it first, never this row.
  ```

- **Never `⚠ cluster undefined`.** That badge means there is nothing here to
  connect to; this entry defines a cluster perfectly well — it is simply not
  the one a lookup by this name will ever reach. Telling those two facts
  apart is the whole reason this row keeps its data instead of going blank.
- **The badge slot is never shared here.** A shadowed row's own `current`
  and `insecure` are always false, whatever the entry itself would otherwise
  earn ([NOTES § D175](../NOTES.md#d175--the-ruling-in-d174-was-wrong-about-rfc-3986-and-the-parse-that-is-safe-in-both-directions-2026-08-28)) —
  `⚠ duplicate name` never has to compete with `(current)` or
  `⚠ TLS not verified` for the space.

### A server address k8rs will not guess at

The context names a cluster, the cluster has a `server:` line, and k8rs
still will not put an address on screen — `Address::Unreadable`: the
authority does not parse as a plausible `host[:port]`, or nothing printable
survives invariant 9's strip
([NOTES § D175](../NOTES.md#d175--the-ruling-in-d174-was-wrong-about-rfc-3986-and-the-parse-that-is-safe-in-both-directions-2026-08-28)).
`kube` still connects with the raw string, so this row is not broken and not
`⚠ cluster undefined` — that badge means there is no connection to make, and
this row makes one perfectly well. What is missing is a line the reader can
trust: guessing between two readings of an ambiguous address is worse than
showing nothing, because the whole job of the server line is answering *am I
about to touch production*.

```
┌────────────────────────────────────────────────────────────┐
│    weird-proxy                         ⚠ TLS not verified  │
└────────────────────────────────────────────────────────────┘
```

- **The row looks ordinary until it is selected.** No tag is derived — that
  needs a host to match against, and there is none here — but a written tag
  still shows if the context has one, exactly as it would for any other row.
- **The badge slot is unaffected, which is the detail easiest to get
  wrong.** `⚠ TLS not verified` still appears when the cluster sets
  `insecure-skip-tls-verify`, because kube connects with the raw `server:`
  string whatever this screen can draw, and that is exactly the connection
  the warning is about. A row whose address is not shown can still be the
  one that most needs it.
- **Selecting it replaces the address in the server line** — the same slot
  the row above reuses — with a sentence instead of a guess:

  ```
  weird-proxy  —  k8rs found a server address here it cannot read
                  safely, so nothing is shown instead of a guess.
  ```

- `⏎` opens it normally. Nothing about connecting to this entry is
  degraded — the only thing missing is a line to read before doing it.

### No row is both current and landable

`kubectl config delete-context` can leave `current-context` naming an entry
that is no longer in the file; an unset `current-context` reaches the same
place. Measured against both, the picker opened on row 0 regardless, and
`⏎` with no other keypress connected to whatever context happened to be
first — a keystroke choosing a cluster nobody named
([NOTES § D264 ruling 3](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
The picker now opens with **no row marked at all** when this happens. What
it draws next depends on whether a landable row is still there to find. All
four contexts from the kubeconfig are shown, in file order — the point being
made is which rows are landable, not which fit on screen, so this excerpt is
drawn on a terminal taller than the 80×24 floor; at the floor itself these
same four contexts would leave `dev-cluster` under a scrollbar, the same as
[§ More contexts than fit](#more-contexts-than-fit):

```
┌────────────────────────────────────────────────────────────┐
│    prod-eu               aws · prod                        │
│    staging                                                 │
│    kind-k8rs             ~local                            │
│    dev-cluster           ~aws          ⚠ TLS not verified  │
│                                                            │
│  k8rs cannot start you on a row here —                     │
│  pick one with ↑ or ↓.                                     │
└────────────────────────────────────────────────────────────┘
```

- **No `▸` anywhere**, because there is nothing here for it to mean *this is
  where you already are*. This is not the same drawing as a shadowed row or
  an undefined cluster — those rows sit beside an ordinary selection
  elsewhere in the list; here nothing in the list is selected yet.
- **The `[ ⏎ … ]` button draws dim and the footer drops `⏎`** — the same
  rule as a shadowed row
  ([§ A context defined twice](#a-context-defined-twice),
  [NOTES § D264 ruling 2](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) —
  there is no row for it to act on until one is chosen.
- **`↑` and `↓` select the first or last landable row**, never the row the
  cursor happens to sit over first. The moment either is pressed, this
  screen is over: a normal row is selected, the server line replaces the
  message above, and `⏎` connects like any other row.
- **A shadowed row can be exactly the row `↑`/`↓` land on.** `landable()`
  excludes only the row whose cluster is undefined; a shadowed row stays
  reachable so its own explanation can still be read, so `↑`/`↓` looking for
  *the first landable row* can find it before an ordinary one further down,
  exactly as it would with a normal `current-context` — landing there does
  not connect anything, because `⏎` still stays inert on a shadowed row
  ([§ A context defined twice](#a-context-defined-twice)).

**When no row is landable at all** — every context in the file names a
cluster it does not define, or the filter has narrowed the list down to only
rows like that — `↑` and `↓` have nowhere to go either, so the message does
not send the reader to a key that would do nothing
([NOTES § D264 ruling 16](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):

```
┌────────────────────────────────────────────────────────────┐
│    dev-cluster                         ⚠ cluster undefined │
│    old-cluster                         ⚠ cluster undefined │
│                                                            │
│  None of these can be reached — every one names a          │
│  cluster this kubeconfig does not define.                  │
└────────────────────────────────────────────────────────────┘
```

- **The footer reads `/ filter  esc cancel` alone** — neither `↑↓ move` nor
  `⏎` is offered, because neither key has anywhere to go
  ([NOTES § D264 ruling 16](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
- **`⌫` still works.** If a filter narrowed the list down to only rows like
  this, clearing it can bring a landable one back into view; if every
  context in the file really is like this, there is nothing a key on this
  screen can fix, and the reader's next step is their kubeconfig, not this
  screen.

### The filter hides every row

Typing into `/` can narrow the list to nothing — a search for a context that
does not exist, or a typo. The list area goes empty and the slot below it,
which ordinarily carries the selected row's address, says why instead of
being left blank:

```
┌────────────────────────────────────────────────────────────┐
│                                                            │
│                                                            │
│                                                            │
│  No context matches "prod-uk".                             │
└────────────────────────────────────────────────────────────┘
```

- **The typed filter is named**, not just *nothing matches* — the reader
  just typed it and may have mistyped it, and the fix is to see it spelled
  out rather than remember their own keystrokes: nothing on this screen
  echoes the typed filter back as it is typed.
- **The `[ ⏎ … ]` button draws dim, and the footer drops `↑↓ move` as well
  as `⏎`** — a list with no row showing counts as having no landable row,
  the same reading [§ No row is both current and landable](#no-row-is-both-current-and-landable)
  already gives an every-row-undefined list, so this state's footer is the
  same shortened `/ filter  esc clear filter` ruling 16 already draws there
  for a no-landable-row list, not `⏎`'s drop alone — and `esc` reads `clear
  filter` rather than `cancel`, on the box's own button as well as the
  footer, because `/` holds the very text that emptied the list and a key
  is never spelled two ways in one frame
  ([NOTES § D264 rulings 2, 16, 18, 27 and 31](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
- **`⌫` still works** — clearing or narrowing the filter can bring rows
  back, and this state is never a dead end any more than an empty browser
  view is.
- `/` matches only what the row itself draws — the name as drawn,
  `(unnamed)` included, and the tag as drawn, `~` included — never the
  server address or a badge, so a filter that matches an address never
  narrows anything here
  ([NOTES § D264 ruling 7](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).

### The kubeconfig has no contexts at all

Rarer than an empty filter, and reached a different way: the kubeconfig this
run started with had contexts in it, something emptied the file **while k8rs
was already running** — a rewrite, a `> ~/.kube/config`, a config-management
tool resetting it — and then `X` is pressed. There is no row to dim, no row
to skip, nothing to filter: the list is genuinely empty before a single
keystroke.

```
┌────────────────────────────────────────────────────────────┐
│                                                            │
│                                                            │
│                                                            │
│  This kubeconfig has no contexts in it —                   │
│  there is nothing here to connect to.                      │
└────────────────────────────────────────────────────────────┘
```

- **The sentence never says *pick one with ↑ or ↓***
  ([NOTES § D264 ruling 18](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)) —
  that sentence, [§ No row is both current and landable](#no-row-is-both-current-and-landable)'s
  own, promises a key that has somewhere to go; here `↑` and `↓` have
  nothing under them at all, not even a dimmed row, so telling the reader to
  press one would be the exact bug ruling 2 exists to prevent, aimed at a
  key instead of a button.
- **The footer drops `↑↓ move` as well as `⏎`, leaving `/ filter
  esc cancel`** — a list with no row showing counts as having no landable
  row whichever way it got empty, the same shortening ruling 18 gives every
  no-landable-row state
  ([NOTES § D264 ruling 18](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
  `esc` reads plain `cancel` here, drawn this way because no filter has
  been typed yet — `/` still works over an empty list, and the moment
  anything is typed into it `esc` reads `clear filter` instead, button and
  footer both, the same rule [§ The filter hides every
  row](#the-filter-hides-every-row) gives any other typed filter on this
  picker ([NOTES § D264 rulings 27 and 31](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)).
  The list stays just as empty either way — there is nothing here for a
  filter to narrow — so typing is never a way out of this screen, only a
  word change on a key that still does nothing.
  `/` stays bound on the chance a later read of the file brings rows back,
  even though there is nothing to filter yet — the same reasoning that keeps
  it in every other row-less state on this screen.
- **`esc` still cancels back to the running app.** Nothing about the
  connected cluster changed — only the file `X` was about to read from did —
  so this is the mid-session picker's ordinary `esc`, not the startup one.
- **This is not [§ A context whose cluster the file does not
  define](#a-context-whose-cluster-the-file-does-not-define) with every row
  undefined, nor is it the before-the-TUI stderr exit
  [states.md § Before the TUI ever starts](states.md#before-the-tui-ever-starts)
  covers** — that stderr exit is a kubeconfig that never parsed a first
  context in this run at all. Here one did: this run connected somewhere,
  the header keeps naming that context, and the file simply has nothing left
  in it for `X` to offer instead of it.

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
