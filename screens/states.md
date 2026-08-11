# Screens — First launch, empty, and everything going wrong

The states that decide whether a newcomer keeps the tool. All of them were
undefined until they were written down, and most of them happen on **first
launch**.

## Nothing is broken

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
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
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │        reading the cluster… 2 140 pods        │
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
│   storage          │    the token your kubeconfig mints has timed  │
│   config           │    out.                                       │
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
                              ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  You can't list pods across the whole         │
│   workloads        │  cluster, so k8rs is showing the namespace     │
│   network          │  your kubeconfig points at: payments.         │
│   storage          │                                               │
│   config           │  Use  --namespace <name>  for a different     │
│   cluster          │  one, or ask for cluster-wide read access.    │
│ ANALYSIS           │                                               │
│   capacity         │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  ? all keys  q quit                                │
└────────────────────────────────────────────────────────────────────┘
```

## Before the TUI ever starts

These never enter the terminal's raw mode — they print on stderr and exit
non-zero. Panicking inside a TUI corrupts the user's terminal, so startup
failures are handled before there is one.

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
