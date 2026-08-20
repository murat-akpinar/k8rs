# Screen — `k8rs --once` (the output that ships first)

Not a TUI screen: `--once` reads the cluster, prints what is broken, and exits.
It is also **the first thing anyone ever sees from k8rs** — it ships as v0.0.1
at [milestone M1.5](../todo.md#phase-5--live-reads--milestone-m15),
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

▲ node-3 · 2 hours ago
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done

1 critical, 2 warnings
```

It is the Alerts view with the frame taken off: **same findings, same plain
language, same grouping by owner, same order** (severity, then recency). One
`rules.rs`, one set of strings, two renderers — if `--once` and the Alerts
screen could ever disagree, one of them is lying
([alerts.md](alerts.md)).

That includes the ages, which is where the two nearly did disagree. `· 4 min
ago` is a suffix on the title line here rather than a right-aligned column, but
it is **the same string, from the same ladder**
([widgets.md § 1b](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen))
and it obeys the same rule: **it is present only when a field says when the
event happened.** The cordon finding usually has one now — the node lifecycle
controller stamps the taint it mirrors from `spec.unschedulable`, so a plain
`kubectl cordon` leaves a time behind — but not always: a taint applied by hand
(`kubectl taint`) stamps nothing, and on that node `node-3` gets a bare title
line in both renderers — drawn, both ways, in
[alerts.md § the cordon card](alerts.md#the-cordon-card-with-and-without-its-clock).
Without the frame there is no column for a blank to sit in, which makes the
absence invisible here — one more reason the pod count stays in the card body
where both renderers show it, whether or not the age is present beside it.

**There is no age column here, so none of the console's width budget applies —
and none of it needs to.** The age is a suffix on a line that is as long as it
needs to be; the console's 14-column maximum is a fact about a right-aligned
field competing with a name for one line, and it stops at the frame
([alerts.md](alerts.md#how-wide-a-card-is-and-how-tall)). What crosses over is
the ladder itself, and one consequence of it worth naming because a reader
comparing two reports will notice: **an event between 24 and 48 hours old
prints `30 hours ago`, not `1 day ago`.** The hours rung runs to 48, which is
where `kubectl` puts it too, so a report and the `kubectl` output beside it do
not describe the same moment two different ways.

## How wide the report is, and why nothing in it is cut

**The report wraps at a fixed 72 columns** — 70 of text after the two-column
indent every body line carries. It is fixed rather than read from the terminal
because stdout is as often a file, a pipe or a CI log as it is a terminal, and
a report whose line breaks depend on the window it was produced in is one that
looks different every time it is pasted into a ticket. 72 survives an email
quote, a GitHub comment and an 80-column terminal with room to spare.

**Nothing here is truncated, and that is the one place the two renderers
deliberately differ.** The console caps a card's evidence at three lines with
`…`, because a controller's verbatim message can run to 350 characters and the
Alerts pane has 16 rows to hold a *list*
([alerts.md § the height](alerts.md#the-height)). It can afford the cut because
`⏎` reaches the full text one keypress away. **`--once` has no `⏎`**: this
output is the whole of what the user gets, so the message prints in full and
the report is as long as it is.

**That is not the two renderers disagreeing, and the difference between the two
is worth being precise about.** They produce the same findings, in the same
order, with the same strings and the same ages — the rule this whole file is
built on. What differs is how much of one string is *on screen at once*: the
console shows the first three lines of it and marks the rest with `…`, because
it has somewhere to send the reader for the remainder. A renderer that cut text
it could not restore would be the lie; a renderer that cuts what one keypress
brings back is a pane doing its job.

## When nothing is broken

```
$ k8rs --once
prod-eu · 84 pods · 3 nodes

○ nothing is broken
```

Three lines, and it has to be true — the same claim the empty Alerts screen
makes ([states.md](states.md)). A tool that prints "0 issues" while holding a
lint list would not survive the first person who checked.

## When a check could not run

```
$ k8rs --once --namespace payments
prod-eu · ns: payments · 12 pods · 3 nodes

○ nothing is broken

One node check is off: spotting a node someone started emptying and
did not finish needs every pod in the cluster.
```

The console says this in a banner above the list
([states.md](states.md#you-can-only-see-some-namespaces)); here there is no
banner, so it goes **last** — after `1 critical, 2 warnings` when there are
findings, after `○ nothing is broken` when there are none. That is the place a
reader is already looking to decide whether the report is complete, and the
last thing left on screen when the output is longer than the terminal. It
prints in both cases: a report with findings is no more complete than an empty
one when the same check was switched off.

- **On stdout, with the findings.** Every other line k8rs writes about itself
  goes to stderr, but this one is part of the answer: `k8rs --once >
  findings.txt` that drops it produces a file claiming a clean cluster with no
  note that a check was switched off, which is the failure the line exists to
  prevent.
- **It is the same sentence the console draws**, re-wrapped. One string, two
  renderers — the rule this whole file is built on.
- **`--namespace` and a 403 fallback print it identically**, because the scope
  is identical; the header line names which namespace, and that is the only
  place the cause shows.
- The header line gains `ns: payments` for the same reason the TUI header does:
  a report that does not say what it covered cannot be trusted after it is
  pasted into a ticket.

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
ever enters raw mode — one text, both paths
([states.md](states.md#before-the-tui-ever-starts)).

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
