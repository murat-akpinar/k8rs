# Screens — First launch, empty, and everything going wrong

The states that decide whether a newcomer keeps the tool. All of them were
undefined until they were written down, and most of them happen on **first
launch**.

## Nothing is broken

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │                                               │
│   workloads        │               ○  nothing is broken            │
│   network          │                                               │
│   storage          │        84 pods and 3 nodes checked, none of   │
│   config           │        them is in trouble right now.          │
│   cluster          │                                               │
│ ANALYSIS           │        Worth a look anyway:                   │
│   capacity      1 ▲│          ANALYSIS → capacity   (1 node is     │
│   certificates  30d│          promising more than it has)          │
│   drain safety     │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

An empty list is a failure state in most tools; here it is the goal, so it is
drawn as an answer and points at the report that still has something to say.
This screen is only honest because Alerts holds nothing but *broken right
now* — a lint report would never be empty.

## Still loading

```
 nodes …                        k8rs      ctx: prod-eu · connecting…
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │        reading the cluster… 2,140 pods        │
│   workloads        │                                               │
│   network          │        Large clusters take a moment. Findings │
│   storage          │        appear as they are found — this list   │
│   config           │        fills up, it does not wait.            │
│   cluster          │                                               │
│ ANALYSIS           │                                               │
│   capacity         │                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A                                              │
├────────────────────────────────────────────────────────────────────┤
│ q quit                                                             │
└────────────────────────────────────────────────────────────────────┘
```

## The connection dropped

The header is the honest one. Stale data drawn as if it were live is
forbidden.

```
 nodes 3/3 (40s ago)          ctx: prod-eu · ⚠ disconnected, retrying
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ⚠ Not connected to the cluster right now.    │
│   workloads        │    What you see below is from 40 seconds ago. │
│   network          │    Retrying…                                  │
│   storage          │                                               │
│   config           │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   cluster          │    Containers exceeded their memory limit     │
│ ANALYSIS           │                                               │
│   capacity      1 ▲│                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   (reconnecting)                     │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

## Your login expired

Not a 403, and not a dropped connection — the third case. On EKS, GKE and AKS
the kubeconfig mints a short-lived token from a credential plugin, and it runs
out mid-session ([NOTES § D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)).

```
 nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ⚠ Your login expired.                        │
│   workloads        │                                               │
│   network          │    The cluster still knows who you are, but   │
│   storage          │    the login token your kubeconfig creates    │
│   config           │    has timed out.                             │
│   cluster          │                                               │
│ ANALYSIS           │    Renew it, then press X and pick this       │
│   capacity      1 ▲│    cluster again:                             │
│   certificates  30d│                                               │
│   drain safety     │      aws sso login                            │
│   waste            │                                               │
│   versions         │    What you see below is from 2 min ago.      │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch   → login expired                    │
├────────────────────────────────────────────────────────────────────┤
│ X switch cluster   ? all keys   q quit                             │
└────────────────────────────────────────────────────────────────────┘
```

- **"Your login expired" and "you are not allowed" are different sentences**
  because they send the user to different places: one is a command they run
  themselves, the other is a conversation with whoever owns the cluster.
  Printing the 403 text for a 401 sends a beginner to their platform team over
  a timeout.
- The renewal command comes from the `exec` block of their own kubeconfig —
  it is the binary k8rs was already told to run, not a guess about which
  cloud they are on. If the kubeconfig does not use a credential plugin, the
  line is omitted rather than invented.
- Stale data stays visible and stays labelled, exactly as on the disconnected
  screen. k8rs does not clear the screen because it lost its token.

## You can only see some namespaces

Not an error — the common case for anyone who is not a cluster admin. A 403
on the cluster-wide list falls back instead of failing
([NOTES § D5](../NOTES.md#d5--namespace-scoping-is-a-v1-requirement-not-a-filter)).

```
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  You can't list pods across the whole         │
│ RESOURCES          │  cluster, so k8rs is showing the namespace    │
│   workloads        │  your kubeconfig points at: payments.         │
│   network          │  Use  --namespace <name>  for a different     │
│   storage          │  one, or ask for cluster-wide read access.    │
│   config           │                                               │
│   cluster          │  One node check is off: spotting a node       │
│ ANALYSIS           │  someone started emptying and did not finish  │
│   capacity         │  needs every pod in the cluster.              │
│   certificates  30d│                                               │
│   drain safety     │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   waste            │    Containers exceeded their memory limit and │
│   versions         │    were killed by the kernel (OOMKilled)      │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

### The second paragraph is the point of this screen

A check that is switched off and says nothing looks exactly like a check that
passed. Two of them are switched off here, because both add up **every** pod on
a node and this view has a fraction of them
([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)):

| Off | Would have appeared | Said where |
|---|---|---|
| The half-finished drain — a node taken out of service with pods still to move | as a `node-3` card in **Alerts** ([alerts.md](alerts.md#under-namespace-scope-there-is-no-card-and-the-screen-says-so)) | the banner above, in the words drawn there |
| Overcommitted nodes — promised more than they have | as a row in the **Capacity report**, and as the `capacity  1 ▲` badge beside it in the sidebar | on that report when it is opened ([analysis.md](analysis.md#capacity-when-you-can-only-see-one-namespace)) |

- **Each screen names the check it would have run.** Alerts says the Alerts one;
  Capacity says the Capacity one. Nothing collects them into a single global
  notice, so adding a third disabled check later grows one screen by a sentence
  instead of growing this banner by a list.
- **The sidebar badge stays blank, and that is why the report has to speak.**
  `capacity  1 ▲` has room for a number, not for a sentence, and a fourth symbol
  meaning *not checked* would need a legend nobody has read yet — so the badge
  obeys the existing rule (a vital that cannot be read is blank, never guessed,
  [widgets.md § 1a](widgets.md#1a-the-header-row)) and the screen behind it
  carries the explanation.
- **This is a degradation, not a new mechanism.** It is what
  [docs/architecture § Error handling](../docs/architecture.md#error-handling)
  already specifies for a 403 on a secondary stream: the feature switches off
  and names what it needed.
- **The banner is above the list, not below it.** A reader who scrolls to the
  bottom of the findings to learn the list was incomplete has already believed
  it.

### The same screen, three ways it can differ

- **Two causes, one scope.** `--namespace payments` and a 403 on the
  cluster-wide pod list produce the identical state — `ClusterSnapshot` carries
  one `namespace_scope` for both ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  Only the first paragraph differs: the flag case reads *"Showing only the
  payments namespace, because `--namespace` asked for it"* and keeps the second
  paragraph unchanged, because the checks are off for the same reason either way.
- **Nodes may still be listable, and here they are.** Being scoped to a
  namespace for *pods* says nothing about *nodes* — the header keeps
  `nodes 3/3`, and N1 (a node that is not ready) and N3 (a node running out of
  disk or memory) still fire, because they read the node's own conditions and
  join nothing.
- **If nodes are not listable either**, the header's left zone is blank
  ([widgets.md § 1a](widgets.md#1a-the-header-row)) and the second paragraph
  says that instead, in the same slot: *"Nodes are not checked at all — your
  user can't list them. Missing permission: list nodes."* Same banner, same
  rule, different cause. It names the verb and the resource because that is the
  string the reader has to hand to whoever owns the cluster, which is the rule
  every other 403 on this page already follows.

### Nothing broken, and something not checked

The dangerous combination, and the reason the banner exists: *"nothing is
broken"* is the strongest claim k8rs makes, and under a partial view it is
making it while one check is switched off.

```
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS            │                                               │
│ RESOURCES          │               ○  nothing is broken            │
│   workloads        │                                               │
│   network          │        12 pods in payments and 3 nodes        │
│   storage          │        checked, none of them is in trouble    │
│   config           │        right now.                             │
│   cluster          │                                               │
│ ANALYSIS           │        One node check is off: spotting a      │
│   capacity         │        node someone started emptying and      │
│   certificates  30d│        did not finish needs every pod in      │
│   drain safety     │        the cluster.                           │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

- **The claim is scoped to what was actually read.** `84 pods and 3 nodes` on
  the cluster-wide screen becomes `12 pods in payments and 3 nodes` — the
  sentence counts what k8rs looked at, never what the cluster has.
- **"3 nodes checked" and "one node check is off" are both true**, and reading
  them together is the whole point: the nodes were checked, but not for
  everything. Either sentence alone would mislead.
- **`Worth a look anyway → ANALYSIS → capacity` is gone from this variant**, and
  its absence is not a layout decision. Under this scope the Capacity report has
  nothing to say either; sending the reader there would be a tour of a second
  switched-off check.
- The wording of the missing check is **the same sentence** as the banner above,
  re-wrapped for a narrower block. One string, three renderers — the third is
  `--once` ([once.md](once.md#when-a-check-could-not-run)).

## Before the TUI ever starts

**No kubeconfig at all** is always this — stderr, exit non-zero, no raw mode —
there is nothing yet to list a context from. The other two below hold only
when the picker never opened: one context in the file, `--context` given,
`--once`, or a non-tty. Once two-or-more contexts put the picker on screen,
raw mode is already on, and *cannot reach the cluster* / *not allowed* become
the modal in
[context.md § When the new cluster does not work](context.md#when-the-new-cluster-does-not-work)
instead of a stderr message. Panicking inside a TUI corrupts the user's
terminal, so whichever form applies, the failure is handled before it can do
that.

```
$ k8rs
k8rs: no kubeconfig found.

  Looked in: $KUBECONFIG (unset), ~/.kube/config

  k8rs uses the same file kubectl does. If kubectl works on this
  machine, k8rs will too.

$ k8rs
k8rs: cannot reach the cluster at https://10.0.0.1:6443

  The address is in your kubeconfig, but nothing answered.
  Is the cluster running? Are you on the right VPN?

$ k8rs
k8rs: your user is not allowed to list pods in this cluster.

  Missing permission: list pods (cluster-wide)
  Your kubeconfig context: prod-eu, user: dev@example.com

  Ask for one of the two roles in the README, or run k8rs
  against a single namespace:  k8rs --namespace <name>
```

## Rules that hold across every state on this page

- The header always tells the truth about **context · scope · connection ·
  read-only**, and says so when the kubeconfig disables TLS verification.
- **A vital in the header is blank rather than guessed, and stale rather than
  hidden.** `nodes 3/3` becomes `nodes …` while connecting, `nodes 3/3
  (40s ago)` when the stream is gone, and nothing at all for a user who cannot
  list nodes. The `capacity` badge follows the same rule
  ([widgets.md § The header row](widgets.md#1a-the-header-row)).
- No state is a dead end: each one names the next thing to try.
- No jargon without its explanation, including in the stderr messages — the
  first thing a newcomer ever sees from k8rs is one of these.
- A 403 degrades exactly the feature that needed the permission and names the
  missing verb and resource. It never crashes and never retries in a loop.
- **A check that could not run says so, on the screen where its findings would
  have appeared.** Silence is the one thing it may not do: an alert list with a
  disabled rule behind it looks identical to an alert list that found nothing,
  and the second is the claim the whole product rests on.
