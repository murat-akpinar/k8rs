# Screen — `k8rs --once` (the output that ships first)

Not a TUI screen: `--once` reads the cluster, prints what is broken, and exits.
It is also **the first thing anyone ever sees from k8rs** — it ships as v0.0.1
at [milestone M1.5](../todo.md#phase-5--live-reads--branch-featwatch--milestone-m15),
months before the console exists ([NOTES § D10](../NOTES.md#d10--m1-ships-publicly-as-v001)).
Seven TUI screens were drawn and this one was not; that is the gap this file
closes ([NOTES § D17](../NOTES.md#d17--the---once-output)).

## What it prints

```
$ k8rs --once
prod-eu · 84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

▲ infra/node-3 · 6 days ago
  Marked unschedulable and left that way
  → someone's maintenance window never closed

1 critical, 2 warnings
```

It is the Alerts view with the frame taken off: **same findings, same plain
language, same grouping by owner, same order** (severity, then recency). One
`rules.rs`, one set of strings, two renderers — if `--once` and the Alerts
screen could ever disagree, one of them is lying
([alerts.md](alerts.md)).

## When nothing is broken

```
$ k8rs --once
prod-eu · 84 pods · 3 nodes

○ nothing is broken
```

Three lines, and it has to be true — the same claim the empty Alerts screen
makes ([states.md](states.md)). A tool that prints "0 issues" while holding a
lint list would not survive the first person who checked.

## stdout and stderr are split on purpose

**stdout is the findings. stderr is everything else** — the commands k8rs
ran, and any error.

```
$ k8rs --once 2>/dev/null        # just the report
$ k8rs --once > findings.txt     # the commands still print to the terminal
```

```
$ kubectl get pods -A
$ kubectl get nodes
```

The command log is the teaching device and it does not stop being one outside
the TUI ([invariant 4](../CLAUDE.md)) — but a report that is piped somewhere
should arrive without it. Splitting the streams gives both for free, with no
flag.

## Colour and symbols

- ANSI colour only when stdout is a terminal **and** `NO_COLOR` is unset. Piped
  or redirected output is plain text.
- `● ▲ ○` carry the severity by themselves, exactly as in the TUI: colour only
  reinforces. This is what makes the output readable after `| less`, in a CI
  log, or by someone who does not distinguish red from green.
- Common Unicode only, no nerd fonts ([docs/tech-stack § Visual identity](../docs/tech-stack.md#visual-identity)).

## Exit codes

| Code | Meaning |
|---|---|
| `0` | k8rs ran and reported — **whether or not anything was broken** |
| `2` | k8rs could not run: no kubeconfig, unreachable cluster, not allowed to list pods |

**Findings do not change the exit code.** k8rs is a report, not a linter: a
beginner who runs it, sees three warnings and then sees `$?` = 1 will conclude
the tool failed. `1` is left unused so that a future `--exit-code` flag, if one
is ever actually asked for, has somewhere to go without moving what `0` means.

Failures print the same plain-language stderr messages the TUI prints before it
ever enters raw mode — one text, both paths ([states.md § Before the TUI ever
starts](states.md#before-the-tui-ever-starts)).

## What `--once` does not do

| Not offered | Why |
|---|---|
| Analysis reports (capacity, drain safety, waste, certificates, versions) | Choosing *which* report needs an argument, and an argument that takes a value is the threshold that would pull `clap` in ([docs/tech-stack](../docs/tech-stack.md)). The reports are a console feature; `--once` answers one question. |
| `-o json` / `-o yaml` | Nobody has asked. It is one function over `Vec<Finding>` when someone does, and inventing an output schema now means maintaining it forever ([NOTES § Out of scope](../NOTES.md#out-of-scope-the-most-important-section)). |
| `--watch` | That is the TUI. |

`--context` and `--namespace` apply unchanged. `--read-only` is accepted and
does nothing in v0.0.1 — there is no write path in the release that ships this,
and a flag that errors because the danger it guards has not been built yet
teaches the wrong lesson.

## The rule that matters most here

Findings contain names, messages and annotations from the cluster, and there is
**no ratatui between them and the terminal**. `sanitize()` runs on the same
strings, at the same boundary, before anything is printed
([invariant 9](../CLAUDE.md) · [widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
A pod named with an escape sequence must be as boring in `--once` as it is in
the console — and `--once` is the path that ships first, so it is the path that
gets the untrusted-input test first.
