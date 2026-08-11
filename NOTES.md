# k8rs — Kubernetes Triage TUI — Decision Record

> Last updated: 2026-08-11 · Status: design phase, no code

## In one sentence

**lazygit for Kubernetes**: a single-binary Rust + ratatui TUI that tells you
**what is broken right now and why** in language a beginner understands, and
lets you fix it without memorising a single long `kubectl` command — showing
you the command it ran, every time.

*(Revised 2026-08-11 — see [Reversal](#reversal--read-only--managed-writes-2026-08-11).
The original sentence ended "…without installing anything into the cluster",
and the tool was read-only. Nothing is still installed into the cluster; the
read-only part is gone on purpose.)*

## Why it exists — where the gap is

Looking at the existing tools:

| Tool | What it does | What it lacks |
|---|---|---|
| k9s / Rancher | **Explorer.** Draws `kubectl get pods -A` nicely | *Tells* you nothing. You still need to know what to look at — and k9s assumes you already speak Kubernetes |
| popeye | Scans the cluster and reports problems | One-shot CLI report, not live. Weak TUI |
| k8sgpt | Interprets via LLM | LLM-dependent, doesn't work offline. Weak TUI |
| lazygit / lazydocker | The approachability model we are copying | Not Kubernetes |

The gap: nothing is **live + interpreting + approachable** at the same time.
k9s is a cockpit for pilots; the newcomer in their first month needs the thing
that says *this is broken, here is why, press `s` to fix it — and by the way,
the command was `kubectl scale …`*.

The critical distinction — not an explorer, an **interpreter**:

```
k9s       : web-7d9f  0/1  CrashLoopBackOff  47  3d
our tool  : web-7d9f restarted 47 times — last exit OOMKilled (137),
            limit 256Mi. The limit is too small.
```

The first is data, the second is an answer. And the second needs **no extra
data source whatsoever**: `lastState.terminated.reason` and `resources.limits`
already sit side by side in the same API response. Nobody joins them — that's
the whole trick.

## Reversal — read-only → managed writes (2026-08-11)

The strongest original claim was *"this tool cannot break anything."* The user
reversed it deliberately, with the alternatives (stay read-only / defer writes
to v3) on the table. Recorded here rather than applied silently, per
CLAUDE.md invariant 10.

**What changed:** k8rs becomes a **full admin console that diagnoses** —
"everything an admin needs, in one TUI" (user, 2026-08-11). Three views
instead of one screen: **Alerts** (the findings engine, still the default
view), **Resources** (browser over every kind in the cluster, with writes),
**Analysis** (cluster-wide reports that join across kinds). It is no longer a
read-only diagnostician.

**Honest sizing, stated once:** this is roughly 5–10x the original v1 and a
multi-month project rather than a multi-week one. It is planned in `todo.md`
so that a useful binary ships at the end of every milestone (M1 rules → M2
writes → M3 console), not only at the end.

**What this costs, accepted knowingly:**

| Lost | Consequence |
|---|---|
| "Cannot break anything" | The sales pitch changes: safety is now *guarded*, not *structural* |
| Trivial RBAC | Two ClusterRoles instead of one; the admin role holds `patch`/`delete` |
| Single-screen simplicity | Sidebar + resource views + modal layer — the thing NOTES called the k9s trap |
| Five files, flat | Eight files, still flat (below). No mod pyramid regardless |
| Small scope | The named number-one risk (scope creep) is now realized on purpose |

**What is kept, because it is the only differentiator left:** the rule engine.
Every competitor with a browser (k9s, Rancher) shows state; none of them
*interprets* it. **k8rs opens on Alerts, never on a pod list** — that single
default is what separates it from a k9s clone with fewer features. If that
ever changes, the project has no reason to exist.

### Positioning — "lazygit for Kubernetes" (user, 2026-08-11)

The reference point is **lazygit / lazydocker**, not k9s. The user is someone
in their first weeks on the job who should not have to memorise
`kubectl scale deployment web --replicas=3 -n payments` to do a normal day's
work. This is a stronger constraint than "admin console", and it decides
several things that were otherwise open:

1. **The command log is a first-class panel, not a hidden toggle.** lazygit's
   best idea: it shows the exact command it just ran. For k8rs the same panel
   is simultaneously the teaching device *and* the audit trail required by the
   safety model — one feature, two requirements. A beginner watches
   `kubectl scale deployment/web --replicas=3` scroll by and learns it without
   being taught.
2. **Keys are always on screen, context-sensitive.** The footer shows the keys
   valid for the panel you are in right now; `?` opens the full map. A tool for
   beginners may not hide its verbs behind memory.
3. **Zero configuration on first run.** No flags, no config file, no setup
   step. It reads your kubeconfig's current context and works.
4. **Plain language over jargon, everywhere** — not only in findings. Column
   headers, confirmation dialogs, error messages. `Evicted` is "removed by the
   node because it ran out of room".
5. **Confirmations explain the consequence, not the API call.** "This starts 2
   more copies of your app" above the `kubectl` line, not instead of it.
6. **Per-object detail tabs**, the lazydocker pattern: logs · describe · yaml ·
   events, switched with `[` / `]`. A newcomer's whole debugging loop without a
   single typed command.

This positioning also resolves the tension in the reversal: lazygit is not
"read-only git", it is git made approachable — and it is trusted precisely
because it is explicit about what it runs.

### The three views

```
┌ k8rs ───────────────────────────────┬ ctx: prod-eu · ⟳ live · ADMIN ─┐
│ ALERTS      ▸ 3 critical  7 warn   │                                │
│ RESOURCES   ▸ workloads            │   (the selected view fills      │
│               network              │    this pane — one at a time,   │
│               storage              │    no split panes)              │
│               config               │                                 │
│               cluster              │                                 │
│ ANALYSIS    ▸ capacity             │                                 │
│               certificates         │                                 │
│               drain safety         │                                 │
│               waste                │                                 │
│               versions             │                                 │
└────────────────────────────────────┴─────────────────────────────────┘
```

| View | What it is | Cadence |
|---|---|---|
| **Alerts** | `rules.rs` output: per-object findings, severity-sorted, live. The original triage screen, unchanged. Default view on startup. | streamed, live |
| **Resources** | Browser over every kind, grouped (workloads / network / storage / config / cluster). Where the write operations live. | list + watch |
| **Analysis** | `analysis.rs`: cluster-wide reports that **join across kinds** and cannot be expressed as a per-object rule — capacity/overcommit, certificate expiry, drain safety, waste, version skew. | on demand |

Alerts vs Analysis is not cosmetic: a rule looks at *one object* and fires
continuously; an analysis looks at *the whole cluster* and is computed when
opened. Different shape, different cadence, therefore different file.

### How "every kind" is supported without writing code per kind

Hand-writing a view for 40+ built-in kinds plus arbitrary CRDs is the trap
that makes browsers enormous. Two mechanisms avoid it:

1. **API discovery** (`kube::discovery`) enumerates every kind the cluster
   actually serves, including CRDs, at startup. The sidebar is generated from
   the cluster, not from a hard-coded list.
2. **Server-side printing** — asking the API server for
   `Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json`
   returns *the exact columns `kubectl get` prints*, computed by the API
   server, for any kind including CRDs. No column definitions in our code, no
   per-kind formatting, and CRDs display correctly for free. (The trailing
   `,application/json` is not optional — see the upstream review below.)

So: `DynamicObject` + Table for browsing (generic, zero per-kind code), and
**typed** `k8s-openapi` structs only on the paths the rule engine needs
(Pod, Node, Deployment, PVC, Service). Rules stay strongly typed and testable;
the browser stays generic. This split is the single decision that keeps
"everything" from meaning "everything hand-written".

### Operations — the full admin surface

| Key | Operation | Applies to | Guard | Ships |
|---|---|---|---|---|
| `l` | logs (follow, previous, container picker) | pods | read | v0.1 |
| `d` | describe — object + its events | any | read | v0.1 |
| `y` | view YAML | any | read | v0.1 |
| `s` | scale replicas | deploy/sts/rs | confirm + dry-run | v0.1 |
| `r` | rollout restart | deploy/sts/ds | confirm | v0.1 |
| `ctrl-d` | delete | any | **type the name** | v0.1 |
| `c` | cordon / uncordon | nodes | confirm | v0.2 |
| `ctrl-r` | drain (respects PDBs, reports blockers) | nodes | **type the name** | v0.2 |
| `u` | rollout undo — roll back to the previous revision | deploy/sts/ds | confirm + dry-run | v0.2 |
| `x` | exec into container | pods | confirm (writes into a shell) | v0.3 |
| `p` | port-forward | pods/services | confirm; forwards listed in the header | v0.3 |
| `e` | edit in `$EDITOR` → diff → apply | any | confirm + dry-run + diff | v0.4 |

The ladder is ordered by *what an operator does in a normal day*, not by what
is easiest to build — see [D6](#d6--operation-order-was-inverted-for-the-audience).
`exec` and `port-forward` need real terminal work (PTY handover, local socket
lifetime). `edit` lands last: it is the most dangerous operation, the least
provable without a terminal, and the one every admin already has a habit for.
`rollout undo` is not an API verb — kubectl computes the previous ReplicaSet's
template client-side and patches it back, and k8rs has to do the same
([D7](#d7--rollout-undo-joins-the-operation-set)).

**Not offered, still:** anything that deploys k8rs itself into the cluster
(invariant 2 survives the reversal — the trust model is still "runs on your
machine with your kubeconfig"), and bulk mutation of a selection.

### The safety model that replaces "read-only"

Writes exist, so the guarantee is no longer structural. It is replaced by five
mechanisms, each of which is a requirement, not a nicety:

1. **No implicit write, ever.** Every mutation is a keypress on a selected
   object followed by a confirmation. No bulk operations, no "apply to all",
   no write triggered by navigation or refresh.
2. **Server-side dry-run first.** Scale/edit/restart run with `dryRun=All`
   and show the API server's verdict before the real call. A rejected dry-run
   never proceeds. Free validation, and it catches the admission-webhook class
   of surprise.
3. **Destructive actions are typed, not tapped.** `delete` requires typing the
   resource name — the ctrl-key-slip class of accident is the one that ends
   careers.
4. **Every write is logged locally** to `~/.local/state/k8rs/audit.log`:
   timestamp, context, namespace, object, the equivalent kubectl command,
   and the API result. Append-only, plain text, no cluster involvement.
5. **`--read-only` flag restores the original guarantee** in one flag: the
   write code path is not reachable, the keys are not bound, and the header
   says so. This is the mode for teaching, demos, and production contexts —
   and it is the mode the CI e2e job runs in.

**Structural consequence — writes live in exactly one file.** `ops.rs` is the
only file permitted to call `create`/`patch`/`replace`/`delete`; the
`clippy.toml` ban stays crate-wide and `ops.rs` carries a single visible
`#![allow(clippy::disallowed_methods)]` at its top. One file to audit, one
line that announces the exception. `rules.rs` stays pure, `ui.rs` never
touches the API.

## Design review — second pass (2026-08-11)

A second review of the reversed design, from the point of view of the person
who would actually run this daily. Fourteen decisions — eleven from the review
itself, three more from the audit of the plan that followed it. Each one closes
a contradiction the reversal left open, and several of them **delete** planned
work rather than add it.

### D1 — The audience contradiction, resolved

"lazygit for Kubernetes" (month-one newcomer) and "full admin console" are
different products, and the scope guard written for the first one would have
deleted half of the second: drain safety, version skew, overcommit and
certificate expiry are not things anyone uses in their first month.

**Decision — one audience, two axes:**

> **Written for someone in their first month. Useful to the person on call.**

The *language* target is the newcomer: every string, every column header,
every dialog. The *feature* target is the working operator. The two never
conflict, because explaining a thing plainly does not make it less useful to
someone who already knew it — it only costs words.

**The scope guard is replaced.** Old: *"would someone in their first month use
this in a normal week?"* New, both halves required:

> Would someone who runs clusters use it in a normal week — **and** can a
> newcomer read the screen it produces without a glossary?

The first half rejects expert toys; the second rejects k9s.

### D2 — The dividing line: broken now vs. risky later

The noise problem and the audience problem have one shared answer. Every rule
is assigned by a single question:

| Question | Goes to |
|---|---|
| Is something **broken right now**? | **Alerts** |
| Is something **risky, wasteful or expiring**? | **Analysis** |

Alerts is a work queue: everything in it should be actionable today, and an
empty Alerts screen must be believable. Analysis is a posture review: you open
it when you want it.

Applied to the existing set, this **moves four rules out of Alerts**:

- **Rule 9 (no limits defined)** → the Capacity report. A pod without limits is
  not broken; it is a risk. Almost every cluster has hundreds of them, and left
  in Alerts it would bury the three findings that matter on day one.
- **Rule 8 (hostPath)** → only the escalated case stays in Alerts (`/`,
  `/var/run/docker.sock`, or a writable host mount). The plain read-only
  hostPath is how CNI, CSI and every node agent are *supposed* to work; it goes
  to the posture rows of Analysis.
- **N4 (kubelet version skew)** → the Versions report, where it already had a
  home.
- **C1 (kubeconfig certificate expiry)** → the Certificates report. The sidebar
  badge (`certificates 30d` in the sketch) is the alerting mechanism, and it
  needs no new machinery.

Nothing is lost — those findings still exist, still get tested, and are one
keypress away. What changes is that Alerts stops being a lint report.

### D3 — Findings group by owner, not by pod

**Requirement, not a nicety.** One DaemonSet on a 40-node cluster is one
finding with a count, not forty findings. Findings are grouped by owner
(Deployment / StatefulSet / DaemonSet / Job, falling back to the bare pod when
there is no owner), and the card reads *"3 of 40 pods"* with the offenders
listed on the detail view.

Without this the reversal's own principle — one accent colour, red means
broken — dies in the first real cluster, and k8rs becomes the thing NOTES
criticised popeye for. This was implied by one line in the screen sketch
(*"same pattern in 4 more pods"*) and was never a decision. Now it is.

### D4 — The flagship example promised a number that cannot exist

The sketch showed `limit 256Mi, used 251Mi`. metrics-server reports *current*
usage of a *live* pod; the usage of a container at the moment the kernel killed
it is not retrievable from any API k8rs talks to. The tool's headline example
was a number it could not produce.

**Decision:** OOM evidence is `limit 256Mi · exit 137 (SIGKILL) · 47 restarts ·
last 3 min ago`. Live usage appears only when metrics-server exists *and* the
pod is currently running, and is never labelled as the usage at kill time.
Fixed in the sketch below.

### D5 — Namespace scoping is a v1 requirement, not a filter

`Api::all()` fails with a 403 for the very common user who has access to some
namespaces and not the cluster. For that user the Pod watch dies, and with it
the Alerts view — i.e. the whole product, on first launch, for a large slice of
the audience. "A 403 degrades one feature" does not cover the case where the
feature is everything.

**Decision:** `--namespace/-n` exists in v1, and a 403 on the cluster-wide LIST
falls back to the kubeconfig context's namespace (then `default`), with the
header stating which scope is in effect and why. Four flags now
(`--read-only`, `--context`, `--namespace`, `--once`); still no `clap` — the
threshold for revisiting that is a flag needing validation or a subcommand,
not a fourth boolean.

### D6 — Operation order was inverted for the audience

An operator's day is *logs · describe · exec*. The plan shipped `edit+apply` in
the first release — the most dangerous operation, the least provable
headlessly (it needs `$EDITOR`), the one every admin already has a habit for
(`kubectl edit`) — while `exec` waited until v0.3.

**Decision — the write ladder is reordered:**

| Release | Operations |
|---|---|
| **v0.1** | scale · restart · delete *(plus all reads, including logs)* |
| **v0.2** | cordon / uncordon · drain · **rollout undo** |
| **v0.3** | exec · port-forward |
| **v0.4** | edit + apply |

Consequences: `similar` leaves the v1 dependency set with `edit` (see
[Dependencies](#dependencies)); `serde_yaml_ng` stays, because `y` still shows
YAML. The YAML spike result and the "user's buffer is the source of truth" edit
model stay recorded as decided — they are simply applied in v0.4.

### D7 — `rollout undo` joins the operation set

The single most-reached-for command after a bad deploy, and it was missing
entirely while `edit` was in. Added at v0.2, with the honest cost noted: it is
not an API verb. `kubectl rollout undo` reads the previous ReplicaSet's pod
template and patches it back into the Deployment, client-side, and k8rs has to
do the same. That is real work, not a one-line call — which is exactly why it
is recorded now rather than discovered later.

### D8 — Invariant 4 was not literally true

"Every command k8rs runs is shown to the user" — but k8rs runs
`Api::patch_scale`, `Api::restart`, `Api::evict`. Those are not kubectl
invocations; the panel shows the *equivalent* command. Fine as a teaching
device, dishonest as an audit trail: a log recording a command that never ran
cannot answer "what did they actually do".

**Decision:** the split is made explicit. The **command log** shows the
equivalent kubectl command (teaching). The **audit log** records both that line
*and* the real API call — verb, path, resourceVersion sent, dry-run verdict,
result. Invariant 4 in CLAUDE.md is reworded accordingly.

### D9 — One rule added to v1; the rest recorded, not built

The review turned up five genuinely common failures the rule set missed. Only
one is free with the watches already running:

- **Rule 12 — pod stuck Terminating.** A `deletionTimestamp` older than the
  grace period means a finalizer or a wedged kubelet is holding it. Costs
  nothing (the Pod watch is already open), happens constantly, and `kubectl
  get` never explains it. **In v1.**

The other four need a stream or list that the two-permanent-watch budget
([invariant 6](#architecture--where-lightweight-comes-from)) has no room for in
v1. Recorded as the **v0.2 rule set**, in the order they earn their watch:

| # | Finding | Needs |
|---|---|---|
| J1 | Job failed permanently (backoffLimit exceeded) | `batch/v1` Jobs |
| J2 | CronJob suspended, or no successful run in N schedules | `batch/v1` CronJobs |
| H1 | HPA pinned at maxReplicas, or unable to compute metrics | `autoscaling/v2` |
| Q1 | ResourceQuota exhausted — nothing can be created in the namespace | quota list |
| — | Deprecated / removed API versions in use | discovery + a removal table that goes stale; Analysis report, later |

**Related, and decided the same way:** *"Service whose selector matches no
pod"* — the best single finding in the whole design, the 503 nobody can
explain — **stays in the Waste report** and does not get promoted to Alerts.
Promoting it would cost a permanent Services + EndpointSlices watch, and the
watch budget is the reason k8rs is lighter than k9s. It becomes the first row
of the report instead.

### D10 — M1 ships publicly as v0.0.1

The plan's real failure mode is dying during the console phase with nothing
released. Phase 3 already builds a temporary main that prints findings; the
decision is to **release it** as `k8rs --once` — reads the cluster, prints
what is broken, exits.

It costs one flag and a README. In exchange, the diagnosis engine — the only
part of this project nothing else does — is in people's hands months before the
TUI, and the feedback arrives while the rules can still change cheaply.

### D11 — The ninth file, pre-approved

`ui.rs` has to draw three views, a sidebar, detail tabs, a command log, a help
overlay and a modal layer. It will not stay small, and
[invariant 11](#file-layout) demands a boundary argument before a ninth file
exists. The argument is accepted **in advance** so that Phase 11 does not stall
on a rule debate: if `ui.rs` passes ~800 lines, the modal layer splits into
`dialog.rs` — confirmations, typed-name deletes and the help overlay, the one
part of the UI that is separately testable and that renders the safety
contract.

That is the only pre-approved split. Everything else still needs an argument.

### D12 — the key map, and two keys deleted

Auditing the plan against the screen mockups turned up four keys with two
meanings each — not a design, an accident. Resolved by giving every key **one
meaning everywhere**, and deleting the two features that caused the collisions:

| Key | Meaning | Where |
|---|---|---|
| `/` | filter this list / search this pane | everywhere, including the log tab |
| `n` | filter by namespace | every list view |
| `f` | follow | the log tab only; unbound elsewhere |
| `c` | pick a container (log tab) · cordon/uncordon (a node in Resources) | two panes that cannot both be open, so this one is legal by context and is recorded rather than resolved |
| `r` | rollout restart | never "retry" |
| `X` | switch cluster | everywhere; unbound while a modal is open ([D16](#d16--the-context-switcher)) |

**Deleted: the severity filter.** It existed because the Alerts list was going
to be long; owner grouping ([D3](#d3--findings-group-by-owner-not-by-pod))
made it short, and severity is already the sort order. One key and one feature
fewer.

**Deleted: the manual reconnect key.** kube-rs backs off and reconnects on its
own; the requirement was always that the state is *visible*, never that the
user drives it. The disconnected banner reports, it does not ask.

### D13 — licence: `MIT OR Apache-2.0`

Undecided until the audit, and it blocks step one of the code phase:
`cargo publish` refuses a crate without a `license` field, so the placeholder
could not have been claimed. Dual MIT/Apache-2.0 is the Rust ecosystem default
and the permissive choice a tool people run against production clusters should
make. Both files land with `cargo init`, and `deny.toml`'s copyleft rejection
for dependencies stays as it was — that policy is about what we pull in, not
about what we publish.

### D14 — three plan corrections

Not design decisions, but the plan was wrong in three places and the rule is
that the plan gets fixed in writing, not patched at build time:

1. **The justfile does not freeze at the end of Phase 1.** Its targets are all
   declared there, but the body of `fixtures` — the jq sanitizer — is written
   in Phase 2, where the fixtures are. Freezing it a phase early made a
   forward-only violation inevitable. It freezes after Phase 2.
2. **The Phase 7 `k8rs ops …` subcommand is scaffolding, not surface.** It
   lives in the temporary main so that every write can be proven headlessly,
   and it disappears when the console lands. It therefore does not trip the
   "a subcommand means it is time for clap" threshold — nothing shipped has a
   subcommand.
3. **The read-only hostPath posture rows belong to `analysis.rs`.** They read
   pod fields, so they *could* live in `rules.rs`, but they are a whole-cluster
   list with no per-object alert — putting them in `rules.rs` would mean
   reaching back into a frozen file to make them a report. `rules.rs` emits the
   escalated case only.

### D15 — the widget layer, and what it rules out

The mockups said what every screen looks like and nothing said what draws it,
so Phase 11 would have designed the UI while implementing it. The map lives in
[screens/widgets.md](screens/widgets.md) — one file, because a widget choice
scattered across seven mockups is seven places to keep in sync. Three things
it settles, all of which are decisions rather than transcription:

1. **No mouse support.** crossterm's mouse capture takes the terminal's own
   text selection away from the user, and the command log exists to be copied.
   A tool whose teaching device cannot be copy-pasted has traded its point for
   a click target.
2. **No animation, no spinner.** Every one of them needs a timer tick, and a
   timer tick is a frame rate — [invariant 7](CLAUDE.md) says draws happen
   on events and the loop blocks when idle. "Still loading" stays a static
   line of text.
3. **Below 80×24 the layout is not attempted.** One centered sentence naming
   the required size and the current one, and the normal frame returns when
   the terminal grows. The alternative is responsive breakpoints for a fixed
   20-column sidebar next to a server-printed table, which is a second layout
   engine in exchange for a screen nobody can read anyway.

A fourth thing it confirms rather than decides: every ratatui state object
(`ListState`, `TableState`, `ScrollbarState`, tab index, scroll offset) lives
in `views.rs`, leaving `ui.rs` a pure function of state. That was already the
file layout's intent; it is now written where someone building the UI will
read it.

### D16 — the context switcher

The out-of-scope list said "one context at a time, **switchable**" and no
screen, key or step ever defined the switch: `--context` was a startup flag,
so checking staging meant quitting and relaunching. Closed by `X` and a modal
picker ([screens/context.md](screens/context.md)). What the design turned on:

1. **No confirmation dialog.** Nothing is written — not to the cluster, not to
   the kubeconfig — so [invariant 2](CLAUDE.md) does not apply and a second
   dialog would be ritual. The picker *is* the confirmation: explicit key,
   explicit selection, the target's API server address on screen. What is
   required instead is that `X` cannot fire while a modal is open or a
   confirmed write is still in flight.
2. **The command log must not print `kubectl config use-context`.** That
   command edits the user's kubeconfig; k8rs never does. Showing it would
   teach a side effect the tool does not have — the same dishonesty
   [D8](#d8--invariant-4-was-not-literally-true) was written to remove. Every
   line after a switch carries `--context <name>` instead, which is both true
   and the better thing to learn.
3. **A failed switch does not fall back.** Unreachable or 403 on the new
   context shows a modal and *stays there*. Silently returning to the old
   cluster produces a header naming one cluster over another's data, which is
   the exact failure [screens/states.md](screens/states.md) forbids.
4. **The switch is the startup path run again**, which forced a plan
   correction: `k8s.rs` (Phase 5) has to expose connecting as a re-callable
   function. Discovering that in Phase 11 would have meant reaching back into
   a frozen file — the forward-only rule catching a real error rather than a
   hypothetical one.

The kubeconfig stays the only source of credentials: a switch picks a
different context out of the same file and never accepts a typed server,
token or certificate. [Invariant 3](CLAUDE.md) does not move.

### D17 — the `--once` output

Seven TUI screens were drawn and the one thing that ships **first** had none:
`k8rs --once` is v0.0.1, months ahead of the console, and it is the first
output any stranger sees ([D10](#d10--m1-ships-publicly-as-v001)). Designed in
[screens/once.md](screens/once.md); three decisions in it:

1. **Findings do not change the exit code.** `0` when k8rs ran, `2` when it
   could not (no kubeconfig, unreachable, not allowed). k8rs is a report, not
   a linter — a beginner who sees three warnings and then `$?` = 1 concludes
   the tool broke. `1` stays unused so a `--exit-code` flag has somewhere to go
   if anyone ever asks, without redefining `0`.
2. **stdout is the findings, stderr is the commands and the errors.** The
   command log is the teaching device outside the TUI too
   ([invariant 4](CLAUDE.md)), but a piped report should arrive without it.
   Splitting the streams gives both with no flag.
3. **No analysis reports in `--once`.** Selecting a report needs an argument
   that takes a value, and that is the threshold that pulls `clap` in
   ([invariant 10](CLAUDE.md)). `--once` answers one question; the reports are
   a console feature.

It is the Alerts view with the frame removed — same rules, same strings, same
owner grouping, same order. Two renderers over one `rules.rs`; if they can
disagree, one of them is lying.

## Design review — third pass (2026-08-11)

The second pass reviewed the *product*. This one reviewed the **runtime**: what
this kind of application actually does between the screens — time, credentials
that expire, calls that take minutes, records that cannot be written, objects
that vanish mid-dialog, a terminal that gets suspended. Seven of these were
missing outright and one of them contradicts a hard invariant.

None of it is new scope. Every item below is something the already-agreed
design must do and did not say how.

### D18 — the clock is an input, not an ambient fact

[Invariant 5](CLAUDE.md) says rules are pure: "no clock, no network, no
terminal, no globals". But rule 12 fires on a pod whose `deletionTimestamp` is
*older than the grace period*, C1/C2/C4 report a certificate that *expires in
N days*, and every finding on the Alerts screen ends in *"4 min ago"*. All
three need to know what time it is. The invariant and the rule set have been
contradicting each other since both were written.

**Resolution: `now` is a field on `Snapshot`.** The caller captures it once per
analysis pass and puts it in the input. `analyze(&Snapshot) -> Vec<Finding>`
stays exactly as pure as the invariant demands — it reads a field, it does not
call a clock — and the invariant's wording gains one clause rather than an
exception.

This is also what keeps the tests honest. A fixture pins `now` alongside the
objects, so a test written today still passes in 2029; a rule that called
`SystemTime::now()` would need its fixtures re-captured every time a certificate
in them expired, and the fix for that rotting test would be to weaken it.

Two consequences that the implementation cannot skip:

- **Findings carry timestamps, not phrases.** "4 min ago" is formatted by the
  renderer — `ui.rs` and the `--once` printer — from a timestamp the rule
  emitted. Two renderers, one source, and the same rule is readable in a test
  without parsing English.
- **Clock skew is real and shows.** The timestamps come from the API server;
  `now` comes from the user's laptop. A machine a few minutes fast produces a
  negative age, and *"in -3 minutes"* on the first screen a beginner sees is
  worse than useless. A non-positive age renders as **"just now"**.

**The type is `jiff::Timestamp`, and it is already in the tree** — verified
against `k8s-openapi 0.28.0` rather than assumed, because the assumption was
wrong the first time. `meta::v1::Time` is `pub struct Time(pub jiff::Timestamp)`
and the crate re-exports the library (`pub use jiff;` in `lib.rs`), so `now` is
the same type the API's own timestamps already are: no conversion layer, no
`Cargo.toml` line, no [invariant 10](CLAUDE.md) decision to make. *(It was
`chrono` until k8s-openapi moved; writing "chrono" here from memory would have
put a dependency in the plan that the crate graph does not contain.)*

What it does cost, and what is therefore recorded: k8rs's own time type is now
tied to **k8s-openapi's** jiff major version. That is the same coupling the
API types already have, it moves when k8s-openapi moves, and k8s-openapi is
upgraded together with kube-rs and never separately
([docs/tech-stack](docs/tech-stack.md)).

### D19 — 401 is a third case, and the kubeconfig can run a program

The error taxonomy is a "2-variant enum: 403 vs no-connection"
([REQUIREMENTS § Error states](REQUIREMENTS.md#error-states-all-were-undefined-all-happen-on-first-launch)).
On every managed Kubernetes service that is wrong, in two ways.

**A third arm: credentials that expired.** EKS, GKE and AKS kubeconfigs hold no
token at all. They name an external binary — `aws eks get-token`,
`gke-gcloud-auth-plugin` — that is executed to mint a short-lived credential.
That credential expires *during a session*, and the API server answers **401**,
which is not 403 ("you are known and not allowed") and not a dead connection
("nothing answered"). It needs its own plain-language state: the login expired,
here is the command that renews it, k8rs is still running. Telling a beginner
"you are not allowed to list pods" when the truth is "your login timed out"
sends them to their platform team for nothing.

**And the security fact nobody had written down:** that exec plugin means **a
kubeconfig can make k8rs spawn an arbitrary process**. It is the only
code-execution path in the entire trust model. k8rs inherits it from kube-rs,
which inherits it from the way `kubectl` has always worked, and it is not
something to remove — without it the tool does not run on any managed cluster.
What gets written down is that k8rs never *extends* it: it never installs a
credential plugin, never offers to, never runs one from a file the user did not
point it at, and treats the plugin's stdout as a credential like any other —
[token hygiene](docs/security.md#token-hygiene) applies to it unchanged.

### D20 — a call that takes time is a state, and there was none

Every screen is drawn as though the API answers instantly. `scale` against a
busy API server takes seconds; `drain` takes **minutes**. Nothing defined what
the UI is during that window, and "it will be quick" is not a design.

- **The modal closes on confirmation, not on completion.** The command log line
  is written immediately and ends in `…` until the result replaces it. A dialog
  that sits there frozen teaches the user that the tool hangs.
- **Navigation stays free. Three things do not:** a second mutation, `X`
  ([D16](#d16--the-context-switcher)), and `q`. Quitting mid-`PATCH` leaves the
  audit log with an attempt and no result, which is exactly the trail that
  cannot answer what happened; `q` refuses in the footer and quits when the
  call returns.
- **`drain` is different in kind, not in degree.** Minutes, pod by pod, with
  evictions that can be refused by a PDB one at a time. It gets a progress pane
  — *"evicting 4 of 11 · 2 blocked by PodDisruptionBudget"* — not an indicator.
  It arrives with drain in v0.2, and it is the only long-running operation in
  the plan until `port-forward` in v0.3.

No spinner anywhere; a changing *count* is information, a rotating character is
a frame rate ([D15](#d15--the-widget-layer-and-what-it-rules-out)).

### D21 — if the write cannot be audited, the write does not happen

`~/.local/state/k8rs/audit.log` is specified down to its file mode, and
nothing says what happens when it cannot be written — full disk, read-only
home, a `$XDG_STATE_HOME` pointing somewhere that does not exist.
[Invariant 4](CLAUDE.md) makes the answer forced rather than a preference: a
mutation missing from either record *is a bug*, so a mutation that cannot be
recorded must not occur.

- The **attempt** line is written and flushed *before* the API call; the result
  is appended when it returns. A crash mid-call therefore leaves an attempt
  with no result — which is the honest record of what happened, and the reason
  the two are separate writes.
- If the log cannot be opened **at startup**, k8rs says so and continues in
  read-only mode. It does not exit — a broken state directory should not stop
  someone from looking at their cluster — and it does not quietly drop the
  trail, which is the only unacceptable option.
- **No rotation.** A few hundred bytes per mutation, at fifty mutations a day,
  is a megabyte in something like a decade. A rotator would be more code than
  the log itself and is the sort of thing that gets written because it feels
  responsible.

### D22 — a confirmation can outlive the thing it confirms

Open the delete dialog for `web-7d9f4`, start typing its name, and its
ReplicaSet replaces it while you type. The rule that the object's identity sits
in the title bar ([dialogs.md](screens/dialogs.md)) stops a *stale selection*
from being confirmed; it does nothing about an object that is already gone.

The watch is still running behind the modal, so the dialog does not need to
poll anything:

- The dialog holds the object's `uid` **and** the `resourceVersion` it was
  opened with.
- **Gone:** the dialog becomes *"this pod is already gone — something else
  removed it"* and the confirm button dies. Deleting by name what the user
  believes is the object they selected is how the wrong pod gets deleted.
- **Changed:** it says so and offers a re-read. This is the 409 mechanic from
  [REQUIREMENTS § Conflict handling](REQUIREMENTS.md#write-operations-new--the-reversal), moved to
  where the user can still do something about it cheaply.

### D23 — permissions are discovered by failing, and that is backwards

The current rule is "a 403 degrades that one feature and names the missing verb
and resource" — *after* the attempt. For reads that is fine. For the typed-name
delete it is not: the user types a pod name in full, presses `⏎`, and only then
learns they were never allowed. The typing exists to prevent an accident; asking
it of someone who cannot perform the action is not safety, it is a chore.

**One `SelfSubjectRulesReview` per namespace answers it in a single call** —
the API returns everything the user may do there, so keys the user cannot use
are dim from the start and the footer says why. Cluster-scoped operations
(cordon, drain) need a `SelfSubjectAccessReview` each; there are two of them.

**This trips [invariant 1](CLAUDE.md), and the trip is the interesting part.**
Both reviews are performed with **`create`**, so the allowlist outside `ops.rs`
(`get*` / `list*` / `watch*` / `logs` / `log_stream` / `apiserver_version`)
forbids them — even though they change nothing in the cluster. The decision is
to **put them in `ops.rs`** rather than to widen the allowlist: the allowlist is
a mechanical guard and the moment it grows a "but this create is harmless"
clause, it stops being mechanical. `ops.rs` gains one clearly-named read-only
function, `may_i(...)`, with a comment saying why a file called "every write"
contains something that writes nothing. That is a smaller cost than a rule that
requires judgement to apply.

### D24 — Ctrl-Z

`$EDITOR` needs terminal handover in v0.4 and that was planned for. **Ctrl-Z
needs the same handover in v0.1 and was not.** SIGTSTP on a raw-mode TUI hands
the terminal back to the shell still in raw mode, with the alternate screen
still active; the user's prompt is unusable and `fg` returns to an application
that never re-initialised. It is the most-reported bug class in TUI issue
trackers and it costs one function.

That function — leave raw mode, leave the alternate screen, and the reverse on
resume — is the same one `e` needs later, so it is written once in v0.1 and
reused rather than discovered twice. `ratatui::init()` installs a panic hook
that restores the terminal, which is a different path and does not cover this
([widgets.md § 1](screens/widgets.md#1-the-frame)).

### D25 — what this review did not decide

Memory at scale is **unmeasured**, and an unmeasured number is not a decision.
Pruning `managedFields` is agreed; whether a 10 000-pod cluster fits in a
reasonable resident set once pruned is a question with a real answer that
nobody has run. It becomes a Phase 5 measurement with a number attached, not a
design paragraph written from a guess.

### D26 — a green build that proves nothing (2026-08-12)

CLAUDE.md says tests must not lie, and the plan enforced that for the *rules*
and for one guard. Four holes were left, all of the same shape: a check that
reports success without having checked anything.

1. **"CI is green on an empty crate" was the Phase 1 done-when.** A green run
   over zero tests and no product code is the false positive we said we would
   not accept — it proves the YAML parses, nothing else. Every guard added in
   Phase 1 is now proven the way the write-allowlist already was: break it on
   purpose, watch CI go red, then fix it. The red run is recorded per guard
   (fmt · clippy · check-docs · cargo-deny · the allowlist). A guard nobody
   has seen fail is not a guard, and that applies to all five, not to the
   scary one alone.
2. **`cargo test` exits 0 when it runs no tests**, and `#[ignore]` deletes a
   test while leaving its name in the file — the two cheapest ways for this
   project to fool itself. CI asserts a non-zero test count and fails on any
   `#[ignore]` that does not carry a written reason.
3. **Mutation testing covered `rules.rs` only.** `analysis.rs` is the same
   kind of file — pure, fixture-tested, the second diagnosis engine — and a
   report that silently stops flagging anything looks exactly like a report
   with nothing to flag. It gets the same `cargo mutants` gate in Phase 4.
   The two pure files are precisely where a passing test can be worthless,
   which is why they are also the two that come first
   ([build order](#build-order--why-it-is-what-it-is)).
4. **The justfile freezes at the end of Phase 2, but `cargo mutants` is first
   needed in Phase 3.** Same forward-only violation `e2e` had, same fix: the
   `mutants` target is declared in Phase 1 with the others, body and all
   ([D14](#d14--three-plan-corrections)).

What this does not add: no coverage percentage. A coverage number is satisfied
by tests that execute code without asserting anything about it, which is the
failure mode being closed here, not a measurement of it. Mutants answers the
question coverage only gestures at — *would anyone notice if this line were
wrong?*

## Decisions made

### Product

- **Target audience: Kubernetes beginners.** This forces jargon translation —
  `OOMKilled` means nothing to a newcomer.
- **Every finding has three parts:** what happened · what it means · what to do.
- **Show the kubectl command being used** (`dim`, toggled with a key).
  Sounds backwards, but: hide the command and the user stays dependent; show
  it and they learn. Serves both "I don't want to memorize commands" and the
  teaching goal at once. With writes present this doubles as the audit
  trail's human-readable form — the command shown is the command logged.
- **Writes: scale · restart · delete** in v0.1 (reversal above), each behind
  confirm + dry-run, all disableable with `--read-only`. The rest of the
  ladder — cordon/drain/rollout undo, then exec/port-forward, then edit —
  arrives in that order and for that reason
  ([D6](#d6--operation-order-was-inverted-for-the-audience)).
- **The findings screen is the default view.** The browser is reached by
  choosing it in the sidebar; k8rs never opens on a pod list.

### Design

**Palette: Catppuccin Mocha, accent = teal.** Single file `theme.rs`,
10 constants — no theme *loader* (TOML, hot reload), YAGNI.

| Role | Catppuccin Mocha | Hex |
|---|---|---|
| background | `base` | `#1e1e2e` |
| selected row / panel | `surface0` | `#313244` |
| border | `overlay0` | `#6c7086` |
| main text | `text` | `#cdd6f4` |
| evidence line (dim) | `subtext0` | `#a6adc8` |
| accent / highlight | `teal` | `#94e2d5` |
| CRITICAL | `red` | `#f38ba8` |
| WARN | `peach` | `#fab387` |
| OK / healthy | `green` | `#a6e3a1` |
| info / kubectl command | `blue` | `#89b4fa` |

⚠️ Catppuccin is 24-bit RGB. `Color::Rgb` needs a truecolor terminal —
Windows Terminal supports it, legacy conhost does not. Since the target
audience is beginners this is a real risk. Ship a `COLORTERM` check with a
16-color fallback in v1 (~5 lines; originally deferred, pulled into v1 by
the requirements review — first-impression risk, cheap fix).

**Reference note — the rusternetes trap:** the `calfonso/rusternetes`
screenshots were liked as inspiration, but it is *not* a TUI; it is the
**TypeScript web console** of a Rust implementation of Kubernetes. Designed
in terminal aesthetics, but it runs in a browser. Its ring/donut charts,
rounded filled cards, icons and pixel alignment **cannot be done** in a
terminal. The aesthetic direction (mono, warm dark background, low contrast,
dim section headers, stat tiles, sparkline) carries over; fidelity does not.
Also its sidebar is a full resource browser — copy it and you're back to k9s.

- One accent color (mint) + gray tones. **If everything is colored, nothing
  is highlighted** — that's popeye's mistake.
- Red only for "broken right now."
- Two-level hierarchy: title bright, evidence line `dim`. No third level.
- Generous whitespace. The blank line between findings is half the design.
- **Two panes, never more:** navigation on the left, one content pane on the
  right, plus the command log strip at the bottom. What makes k9s feel complex
  is its modal/panel layering — lazygit's discipline is the target instead.
- `● ▲ ○` symbols — never rely on color alone (color blindness + copyability).
- **No nerd-font dependency**, ASCII fallback is mandatory. Otherwise half
  your users see boxes.

Screen sketch — Alerts, the default view on startup:

```
┌ k8rs ───────────────┬ ctx: prod-eu · live · admin ─────────────────┐
│ ALERTS      3 ● 7 ▲│                                               │
│ RESOURCES          │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   workloads        │    Containers exceeded their memory limit and │
│   network          │    were killed by the kernel (OOMKilled)      │
│   storage          │    limit 256Mi · exit 137 · 47 restarts       │
│   config           │    → raise limits.memory, or find the leak    │
│   cluster          │                                               │
│ ANALYSIS           │  ▲ shop/api  ·  2 of 6 pods       12 min ago  │
│   capacity         │    Running, but not receiving traffic — the   │
│   certificates  30d│    readiness check is failing                 │
│   drain safety     │    → check the app's /healthz endpoint        │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
│ $ kubectl scale deployment/web --replicas=3 -n payments            │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

The bottom strip is the command log — every command k8rs runs, as you would
have typed it. It is the teaching device and the audit trail at once, and it
is why a beginner graduates from k8rs instead of depending on it.

### Architecture — where "lightweight" comes from

What determines the load is **not the language, it's how you pull the data**.
k9s is heavy because it fires `LIST pods -A` on an interval; every call makes
the API server read all pods from etcd and serialize them. It degrades
linearly as the cluster grows.

The right way is **watch**: LIST once, then only changes arrive on the
stream. `kube-rs` provides this out of the box with `watcher()` /
`reflector()`.

```
watcher(Api<Pod>::all())
  → prune: ~10 fields, NO managedFields
  → reflector store (memory)
  → rules: pure function, Pod -> Vec<Finding>
  → ratatui: draw only when a change arrives
```

Concrete load reducers:

- **Drop `metadata.managedFields`** — often a third of the object, completely
  useless.
- **Don't store the full `Pod` object**, reduce it to your own small struct.
  Memory drops ~10x.
- **Don't watch Events globally** — the noisiest stream in the cluster.
  Filter `Warning` or fetch on demand.
- **Poll metrics-server slowly** (30s+), only for pods currently on screen.
  It can't be watched, polling is unavoidable.
- **No fixed-FPS drawing.** Draw on events, block when idle → 0% CPU at idle.

Added by the browser (2026-08-11): **only two watches run permanently** — Pods
and Nodes, because the Alerts view depends on them continuously. Every other
kind in the Resources view is listed when you open it and watched only while
it is on screen; closing the view drops the watch. Otherwise "browse every
kind" would mean forty permanent streams, which is a worse version of the
polling problem this architecture exists to avoid.

### v1 rule set

This is where the product's real value lives. Every rule is a pure function;
all of them testable.

**From the Pod object alone** (one watch, no extra requests):

| # | Finding | Source field | What we tell the user |
|---|---|---|---|
| 1 | CrashLoopBackOff | `state.waiting.reason` | "Container keeps crashing and restarting" + exit code |
| 2 | OOMKilled | `lastState.terminated.reason` (exit 137) + `resources.limits.memory` | "Exceeded its memory limit and was killed by the kernel" |
| 3 | ImagePullBackOff / ErrImagePull | `state.waiting.reason` + `.message` | "Image can't be pulled — wrong name/tag, or registry credentials missing" |
| 4 | CreateContainerConfigError | `state.waiting.reason` + `.message` | "Referenced a ConfigMap/Secret that doesn't exist" |
| 5 | High restart count (even if Running) | `restartCount` | "Restarted N times — looks healthy now, but something is wrong" |
| 6 | Non-zero exit | `lastState.terminated.exitCode` | Translate the exit code (see below) |
| 7 | **Pod Running but container not ready** | `containerStatuses[].ready == false` | "Running but not receiving traffic — readiness probe is failing, so it was removed from the Service" |
| 8 | hostPath mount — **only the escalated case** (`/`, docker.sock, writable) | `spec.volumes[].hostPath` | "Mounts the node's own filesystem, writable" |
| 12 | **Pod stuck Terminating** | `deletionTimestamp` older than the grace period | "Asked to shut down N minutes ago and still hasn't — a finalizer or the kubelet is holding it" |

Two rules left this table in the [second-pass review](#design-review--second-pass-2026-08-11):
**rule 9 (no limits defined)** moved to the Capacity report and the plain
read-only **hostPath** case to the Analysis posture rows — neither is *broken
right now*, and both are numerous enough to bury the ones that are. Rule 12
took their place because it costs nothing: the Pod watch is already open.

**Rules that need Events** (second watch, `Warning`-filtered — not in v1,
see the requirements review):

| # | Finding | Source | What we tell the user |
|---|---|---|---|
| 10 | Pending / unschedulable | `conditions[PodScheduled].reason == Unschedulable` + event message | "No node can accept it" + *why* (insufficient cpu / nodeSelector / taint) |
| 11 | Probe failure | Event `reason == Unhealthy` | "Liveness/readiness probe failing" + how many times |

**Exit code translation** (for rules 6 and 2 — where beginners stumble most):

| Code | Meaning |
|---|---|
| 137 | SIGKILL — almost always OOM |
| 143 | SIGTERM — graceful shutdown, not an error |
| 1 / 2 | The application's own error, check the logs |
| 126 / 127 | Command not executable / not found — `command` is wrong |

**Severity escalators** (for rule 8): if the path is `/`, or
`/var/run/docker.sock`, or the mount is not `readOnly` → CRITICAL instead
of WARN.

### Node rules (N-series)

Nodes are few (tens, not thousands), so a Node watch is cheap. These are what
an admin actually reacts to:

| # | Finding | Source field |
|---|---|---|
| N1 | Node NotReady for more than 5 min — the pods on it are dead weight | `status.conditions[Ready]` + `lastTransitionTime` |
| N2 | **Cordoned and forgotten** — "unschedulable for 6 days" is a maintenance window nobody closed | `spec.unschedulable` |
| N3 | DiskPressure / MemoryPressure / PIDPressure — evictions are coming | `status.conditions` |
| N4 | kubelet version skew > 2 minor from the control plane = unsupported | `status.nodeInfo.kubeletVersion` |
| N5 | Overcommitted: sum of pod requests exceeds allocatable | node + pod join |
| N6 | Which taint / nodeSelector is blocking a Pending pod | node taints + pod spec |

N4 is computed here but **shown in the Versions report, not in Alerts** — a
skewed kubelet is a risk, not an outage
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)). The same rule sends
C1 (kubeconfig certificate expiry) to the Certificates report, where the
sidebar badge does the alerting.

### Certificate rules (C-series) — and what is *not* reachable

Correcting a common expectation: `kubeadm certs check-expiration` reads files
on the control-plane node's **disk**. From a laptop, over the API, those files
do not exist. What genuinely is reachable:

| # | Finding | How | Cost |
|---|---|---|---|
| C1 | **Your kubeconfig client certificate expires in N days** (warn at 30) | `client-certificate-data` is base64 PEM in a file we already read — zero cluster traffic | needs an X.509 date parser |
| C2 | API server serving certificate expiry | the peer certificate from the TLS handshake | kube-rs does not expose it; a separate connection is needed |
| C3 | Pending / unapproved CSRs — a kubelet that cannot join | `certificates.k8s.io` list/watch | cheap |
| C4 | cert-manager `Certificate` expiry / renewal failure | CRD `status.notAfter` (no Secret access needed) | cheap, cert-manager only |
| — | Ingress TLS Secrets | would require `get secrets` | **rejected** — reading Secrets to check a date is a bad trade against token hygiene |

### Analysis reports

Cluster-wide, computed on demand, each one a join no per-object rule can do:

| Report | Answers |
|---|---|
| **Capacity** | Per node: requests vs allocatable vs actual usage. Where is the cluster lying to itself |
| **Certificates** | The C-series above, as a dated table sorted by soonest expiry |
| **Drain safety** | For every node: what a drain would do, and what would block it. A PDB with `minAvailable` equal to the replica count means the drain **never finishes** — admins normally discover this 40 minutes in |
| **Waste** | PVCs bound to nothing, Evicted/Completed pod pileups, Services whose selector matches no pod (the 503 nobody can explain), replica sets kept at 0 forever |
| **Versions** | Control plane vs kubelet vs client skew; which nodes are outside the supported window |

Waste and capacity are where krr/popeye play; the difference is that these are
live and one keystroke from the object they blame.

### Upstream architecture review (2026-08-11, docs in `tmp/`)

The plan leaned on three unverified assumptions. Docs pulled from docs.rs and
kubernetes.io; here is what survived and what did not.

**Confirmed:**

- `kube::discovery` exists with `Discovery` / `ApiGroup` / `ApiResource` and a
  `oneshot` mode — the sidebar can be generated from the cluster as designed.
- `PatchParams::dry_run` exists → the preflight step is one field, not a
  hand-built request.
- `dryRun=All` is confirmed in the Kubernetes API docs for all mutating verbs.

**Better than assumed — kube-rs already implements four of our operations:**

| We planned to write | kube-rs provides |
|---|---|
| a `restartedAt` annotation patch | `Api::restart(name)` |
| a `spec.unschedulable` patch | `Api<Node>::cordon(name)` / `uncordon(name)` |
| a hand-built eviction request | `Api::evict(name, &EvictParams)` |
| a full-object patch to change replicas | `Api::get_scale` / `patch_scale` — the scale subresource, which is the correct way |

`ops.rs` shrinks accordingly, and cordon/uncordon — the operation that started
this whole scope discussion — is a one-line call.

**Broken assumption — kube-rs has no `Table` type.** Server-side printing is
not exposed by the typed API. It is still reachable: `Client` has `request`,
`request_text` and `send`, so we build the GET ourselves with the Accept
header and deserialize `meta.k8s.io/v1 Table` with `serde_json`. Roughly forty
lines, once, still zero per-kind code — the design holds, the mechanism was
wrong on paper.

Two details the Kubernetes docs settle, both of which would have been bugs:

1. The header is `application/json;as=Table;g=meta.k8s.io;v=v1` — and
   **aggregated/extension API servers may not serve Table at all, returning
   406**. The Accept header must therefore be
   `application/json;as=Table;g=meta.k8s.io;v=v1,application/json` and the
   client must handle either shape. A browser that 406s on someone's CRD is
   worse than one with hand-written columns.
2. Table is a **GET/list** representation; watching in Table form is not part
   of the contract. So browser views do not watch the Table. They watch
   `Api::watch_metadata` (PartialObjectMetadata — tiny) to learn *that*
   something changed, then re-fetch the Table, debounced. Live enough for a
   human, and still not polling blindly.

**Correction to the version pin:** `k8s-openapi` currently offers `v1_32` …
`v1_36`; the `v1_30` named in REQUIREMENTS no longer exists. Pin the oldest
still offered — **`v1_32`** — and keep the ±2 minor support window.

**Correction to the write ban.** A `disallowed-methods` list of
`create/patch/replace/delete/delete_collection` is not enough: `Api` also
exposes `cordon`, `uncordon`, `restart`, `evict`, `attach`, `exec`,
`portforward`, `entry`, `create_subresource`, `create_token_request`,
`patch_scale`, `patch_status`, `patch_metadata`, `patch_ephemeral_containers`,
`patch_approval` and their `replace_*` siblings. A denylist over that surface
rots the first time kube-rs adds a method.

So the enforcement inverts: **outside `ops.rs`, only an allowlist of read
methods may appear** — `get*`, `list*`, `watch*`, `logs`, `log_stream`,
`apiserver_version`, and the `get_` subresource readers. CI greps for anything
else and fails. A denylist asks "did we remember to ban it"; an allowlist asks
"is this read-only", which is the question that actually matters.

### Dependencies

```toml
kube          # client + runtime (watcher/reflector) + discovery features
k8s-openapi   # API types — one k8s version feature must be pinned
ratatui
crossterm
tokio
anyhow
serde_json    # fixtures, DynamicObject, Table decoding
```

Seven crates carried over from the read-only design. The scope reversal adds
exactly three more, each with a reason — nothing else gets in without the same
justification:

| Crate | Needed for | Why not hand-rolled |
|---|---|---|
| `serde_yaml_ng` (or `serde_norway`) | `e` edit and `y` view — admins edit YAML, not JSON | Writing a YAML emitter is a project of its own; `serde_yaml` itself is unmaintained, these are the maintained forks |
| `x509-parser` | C1 kubeconfig certificate expiry | Parsing ASN.1 dates by hand in a security-adjacent path is exactly the wrong place to be clever |
| `similar` *(v0.4, with `e`)* | the diff shown before `e` applies | A correct diff (Myers) is not a few lines, and the diff is what makes edit safe |

`similar` is approved but **not in the v0.1 dependency set** — `edit` moved to
v0.4 ([D6](#d6--operation-order-was-inverted-for-the-audience)), and the crate
arrives with it. `serde_yaml_ng` still ships in v0.1, because `y` shows YAML.

Still deliberately absent: `clap` — the four flags (`--read-only`,
`--context`, `--namespace`, `--once`) are parsed from `std::env::args` and
none of them needs validation. Also absent: `tracing`, and any TUI widget
framework beyond ratatui.

### File layout

```
src/
  main.rs      event loop, terminal setup/teardown, view routing
  k8s.rs       discovery, watches, prune -> snapshot store (read paths only)
  ops.rs       every write. The ONLY file allowed to mutate the cluster
  rules.rs     analyze(&Snapshot) -> Vec<Finding>      ← per-object, live
  analysis.rs  cluster-wide reports (capacity, certs, drain safety, waste)
  views.rs     per-view state: selection, filters, tabs, scroll
  ui.rs        ratatui drawing
  theme.rs     Catppuccin constants (10 of them)
tests/
  fixtures/    JSON captured from a real cluster
```

Eight files, still flat. **No `mod.rs` pyramid, no trait layer, no plugin
system** — that invariant survives the reversal intact. Each new file exists
because of a boundary that has to be auditable or testable, not because of
tidiness:

- `ops.rs` — the write surface must be one file, or the safety model is not
  reviewable.
- `analysis.rs` — different cadence and shape from `rules.rs` (whole cluster,
  on demand vs one object, streamed).
- `views.rs` — keeps `ui.rs` a pure function of state; the alternative is UI
  state smeared through `main.rs`, which is how TUI code rots.

**Build order is forward-only** (see CLAUDE.md): layers freeze bottom-up —
`rules.rs` → `analysis.rs` → `k8s.rs` → `ops.rs` → `theme.rs` → `views.rs` →
`ui.rs` → `main.rs`. A later step never reaches back into a frozen file; if it
would have to, the plan is wrong and the plan gets fixed first.

The placement of `ops.rs` low in the pyramid is deliberate and is the reason
M2 comes before any UI: **every write operation is verifiable headlessly**
against kind (scale a deployment, watch the replica count change, read the
audit line back). Doing it in this order means the dangerous code is proven
before it is ever attached to a keypress, and the UI phase becomes pure
wiring — `s` calls a function that already works.

### Out of scope (the most important section)

What will kill this project is not technical difficulty, it's **scope
creep**. Three items were removed from this list by the 2026-08-11 reversal
(resource browser, kubectl frontend, write operations) — that is the whole
reason the reversal is written down instead of assumed. What remains absent:

- ❌ Anything deployed into the cluster — no DaemonSet, no CRD, no webhook,
  no in-cluster agent. **The trust model still is "your machine, your
  kubeconfig"** and this survived the reversal untouched.
- ❌ LLM / AI interpretation (k8sgpt exists, and we want to stay offline)
- ❌ Free-form topology graph (nodes+edges) — doable with Canvas but
  unreadable past ~20 nodes
- ❌ Bulk mutation — no "select 12 pods and delete them". Single object,
  single confirmation. The multi-select delete is how outages happen
- ❌ Cluster lifecycle — creating clusters, node pools, cloud provider APIs.
  k8rs talks to one API server and nothing else
- ❌ Editing anything k8rs cannot dry-run first
- ❌ Config file / theming / plugin system / scripting hooks (lazygit has
  these; they came years after it was good, and each is a project)
- ❌ Multi-cluster panes side by side — one context at a time, switchable with
  `X` ([D16](#d16--the-context-switcher) · [screens/context.md](screens/context.md))

**The replacement guard, now that "read-only" is gone** — revised by
[D1](#d1--the-audience-contradiction-resolved), because the first version of
this guard would have deleted the drain-safety report and half of Analysis.
Both halves must hold:

> Would someone who **runs clusters** use it in a normal week — **and** can a
> newcomer read the screen it produces **without a glossary**?

The first half rejects expert toys and one-off curiosities. The second rejects
becoming k9s. "An expert would occasionally want it" still fails the first
half; "an operator needs it but it can only be explained in jargon" fails the
second and gets rewritten, not dropped.

## Traffic / APM side — the fact to know

**There is no traffic data in the Kubernetes API.** RPS, request counts,
service-to-service flow — the API server doesn't know any of it. An external
source is mandatory:

- **Prometheus HTTP API** (`/api/v1/query_range`) — easiest path if
  kube-prometheus-stack is installed, plain HTTP + JSON
- **Istio / Linkerd** — service→service RPS and error rate via
  `istio_requests_total`. Exactly what we want, but a mesh must be installed
- **Cilium Hubble** — gRPC observability API, L3/L4/L7 flows
- Your own eBPF agent — privileged DaemonSet, a separate product

So the TUI does not generate traffic data, it reads what exists. This is an
**adapter** concern and it is not in v1.

### Goldpinger and the contradiction ⚠️

Goldpinger is a node-to-node ping mesh, i.e. *connectivity health* — not
traffic volume. A self-made implementation was requested (roadmap steps 3-4).

**But:** it needs something running on every node, which means deploying a
DaemonSet. At that moment the tool stops being a "CLI that runs on your
machine" — different RBAC, image distribution, different trust model. **We
would have to walk back the sentence "I install nothing into your cluster".**

Note after the 2026-08-11 reversal: the read-only claim is gone, but *this*
claim is not — it is the last structural guarantee k8rs has, and it is the
one that keeps the security model reviewable. Giving up two guarantees instead
of one is how a tool ends up needing an audit before anyone will run it.

Solution: position it as an opt-in plugin, never the default. Go in with
eyes open.

## Roadmap

Revised by the reversal. Each milestone must produce something a person would
actually use — the plan is not allowed to have a "useless until the end" phase.

1. **M1 — the Alerts engine.** Pod rules 1–8 and 12, node rules N1–N6 and the
   kubeconfig certificate rule, headless, tested against real fixtures.
   Already a useful tool: point it at a fixture or a cluster and it prints
   what is broken — **and it gets released**, as `k8rs --once`, v0.0.1
   ([D10](#d10--m1-ships-publicly-as-v001)).
2. **M2 — operations, headless.** `ops.rs`: scale · restart · delete, each with
   dry-run, the confirmation contract and the audit log. Proven against kind
   before a single pixel is drawn.
3. **M3 — the console.** The three views, sidebar navigation, detail tabs,
   logs, command log panel, `?` help. This is the lazygit-shaped product.
4. **M4 — v0.1 release.** musl/darwin binaries, README with a screenshot,
   crates.io.
5. **v0.2 — cordon / uncordon / drain and rollout undo**, wired to the node
   rules and the drain-safety report that already exist by then; plus the
   J/H/Q rule set (Jobs, CronJobs, HPAs, ResourceQuotas) that earns its watch
   here ([D9](#d9--one-rule-added-to-v1-the-rest-recorded-not-built)).
6. **v0.3 — exec and port-forward.** The two operations that need real
   terminal and socket work, and that widen the trust boundary.
7. **v0.4 — edit + apply** (`$EDITOR`, diff, 409 handling) — last on the write
   ladder on purpose.
8. **v0.5 — Events-based rules** (10–11) and the noisy-stream handling they
   require.
9. **Later — traffic adapter** (Prometheus / Istio / Hubble, endpoint from
   user config only) and, separately, the goldpinger-style connectivity mesh
   as its own binary and repository (see the trust-model note below).

The step list for all of this lives in [`todo.md`](todo.md) and only there.

## Settled

- ✅ **Test environment: kind.** Local. Deliberately producing
  OOMKilled/Pending in a real cluster is both hard and risky; in kind we
  break whatever we want.
- ✅ **Interface language: English.** Error messages, K8s terms and search
  results are already English; if open-sourced the audience is too. Jargon
  still gets simplified — `OOMKilled` → *"container exceeded its memory
  limit"*. No i18n (YAGNI); splitting later is cheaper.
- ✅ **All project files in English** (2026-08-11) — notes, docs, README,
  code comments, commit messages. Turkish only in conversation.
- ✅ **Scope: full admin console, not read-only** (2026-08-11) — writes,
  resource browser, analysis reports. See
  [Reversal](#reversal--read-only--managed-writes-2026-08-11); the read-only
  guarantee is replaced by the five-mechanism safety model and `--read-only`.
- ✅ **Positioning: lazygit for Kubernetes** (2026-08-11) — the audience is
  someone in their first month on the job, and that is now the scope guard.
- ✅ **The browser writes no per-kind code** (2026-08-11) — API discovery plus
  server-side `Table` printing. This is what makes "every kind, CRDs included"
  affordable at all.
- ✅ **Licence: `MIT OR Apache-2.0`** (2026-08-11) — the Rust default, and
  `cargo publish` requires the field, so this blocked the very first step of
  the code phase ([D13](#d13--licence-mit-or-apache-20)).
- ✅ **Second-pass review closed fourteen open contradictions** (2026-08-11) —
  audience, the Alerts/Analysis dividing line, owner grouping, namespace
  scoping, the operation order, and the honesty of invariant 4. Nothing about
  the architecture changed; what changed is which findings appear where, which
  operations ship when, and that M1 gets released on its own
  ([Design review § second pass](#design-review--second-pass-2026-08-11)).

### YAML crate decided by spike (2026-08-11) — and the edit model it forces

Ran both candidates against a realistic pod manifest (spike outside the repo;
design phase stays doc-only). Results:

| | `serde_yaml_ng` | `serde_norway` |
|---|---|---|
| comments survive a parse→emit round trip | **no** | **no** |
| key order survives | yes | yes |
| block scalars survive | yes | yes |
| flow style (`["sh","-c"]`) survives | no — normalised to block lists | no |
| parse result | identical to the other | identical |

**Decision: `serde_yaml_ng`.** The two are equivalent on every axis that was
tested, so the tiebreaker is lineage clarity, not capability.

The more important result is that the question was framed wrong. *No*
serde-based YAML crate can keep comments — the serde data model has nowhere to
put them — and formatting is normalised too. So the edit model is forced, and
it is the safer one anyway:

> **The user's text buffer is the source of truth during an edit.** k8rs
> parses their YAML only to *validate* it and to build the request. It never
> re-emits their file back to them. Comments and formatting therefore survive
> because they are never round-tripped — including when an apply is rejected
> and the editor is reopened with their work intact.

**A bug the spike caught before it was written.** On the display path
(API JSON → YAML shown by `y`), `serde_json::Value` uses a `BTreeMap`, so keys
come out **alphabetised**: `metadata: {labels, name, namespace}` instead of
kubectl's `{name, namespace, labels}`. For a tool whose entire point is
teaching beginners what kubectl shows, that is not cosmetic. Fix: enable
serde_json's `preserve_order` feature so API key order is kept.

### Capability probe — "if it is there it works, if not it says so"

Decided 2026-08-11, and it closes the Prometheus/Istio open question by making
it not need an answer: **k8rs detects what the cluster has and adapts.**

The discovery call from Phase 5 already lists every API group the cluster
serves, so the probe is free — no extra request, no configuration:

| Present? | Turns on |
|---|---|
| `metrics.k8s.io` | actual usage numbers in the capacity report |
| `policy/PodDisruptionBudget` | drain safety |
| `cert-manager.io` | C4 certificate findings |
| `monitoring.coreos.com` (kube-prometheus-stack) | the traffic view, later |
| `networking.istio.io` / Linkerd / Cilium | service-to-service traffic, later |

Two rules make this honest rather than magic:

1. **A missing capability is stated, never hidden.** The row stays in the
   Analysis list and reads "needs metrics-server — not installed in this
   cluster". Silently disappearing features teach a beginner that the tool is
   unreliable; a sentence teaches them what their cluster is missing.
2. **Detection is not configuration.** Presence comes from API discovery;
   the *address* of anything outside the Kubernetes API still comes only from
   explicit user configuration — never from pod annotations
   ([REQUIREMENTS § DevSecOps](REQUIREMENTS.md#devsecops-requirements),
   SSRF). "Prometheus is installed, tell me where to reach it" is the whole
   interaction.

The same mechanism covers RBAC: a 403 on a stream disables exactly the
findings that need it and says which permission is missing.

## Open questions
- [x] Project name? → **k8rs** (2026-08-10, see naming section)

## Build order — why it is what it is

*(The steps themselves live in `todo.md` and only there. This section records
the reasoning behind their order; it is not a plan and is not checked off.)*

**Context: this is a first TUI project.** The architecture is built
accordingly. The critical property: the project splits into two halves with
uneven difficulty:

| | Hard / new | Easy / familiar |
|---|---|---|
| What | ratatui event loop + kube-rs watch | Rules: `fn analyze(&Pod) -> Vec<Finding>` |
| Why | Terminal goes into raw mode, events awaited via select, drawing synchronized | Pure function. Struct in, list out |
| Testing | Hard, by eye | **Easy — plain unit tests against captured JSON fixtures, no terminal needed** |

So **the product's real value (the rules) can be written and tested fully
independent of the TUI.** For a TUI newcomer this erases most of the risk.

Hence the order in `todo.md`: fixtures first, then rules (Phase 3, milestone
M1), and only then the TUI. It is deliberate — when the rules phase finishes
you already have a working product (ugly, but working), so if the TUI phase
stalls the project does not die.

> **A cluster is not needed to start the rules** — all they need is pod JSON.
> But hand-written JSON must never survive into the committed fixtures:
> it resembles reality, it is not reality; rules can only be trusted against
> real captures.

## Dev environment setup checklist

Surveyed on the dev machine 2026-08-11. **Most of the plan needs no cluster at
all**, which is the whole reason the build order puts the pure layers first:

| Phase | Needs |
|---|---|
| 3 rules · 4 analysis | cargo + fixtures. **No cluster, no container runtime** — and this is where the product's value lives |
| 8 spike · 9 theme · 10 views · 11 ui | a terminal |
| 2 fixtures · 5–6 reads · 7 ops · 12 wiring | kind, therefore a container runtime |

- [x] **Rust** — 1.97.1 present (distro package). `rustup` is *not* installed;
      it is in the repos if a pinned toolchain is ever wanted. CI pins its own,
      so this is not blocking.
- [ ] **Container runtime** — neither docker nor podman is installed *on this
      laptop*. Both are packaged (`podman 6.0.2`, `docker 29.7.2`). It never
      needs to be: the cluster lives on the LAN host below, and k8rs only
      needs a kubeconfig.
- [ ] **kind** — packaged (`kind 0.32.0`). With podman it needs
      `KIND_EXPERIMENTAL_PROVIDER=podman`; with docker it just runs.
- [ ] **kubectl** — packaged (`1.36.3`). Note the fit: `k8s-openapi` offers
      `v1_32`…`v1_36`, so this client sits at the top of our window and the
      pinned `v1_32` talks to it fine (forward compatibility).
- [x] **The test cluster runs on a LAN host, not here** (2026-08-11) — Docker
      29.7.2 + kind 0.32.0 + kubectl 1.36.3, three nodes, brought up by
      [`scripts/cluster.sh`](scripts/cluster.sh) with
      `K8RS_APISERVER_ADDRESS` set to that host. Not an accident of what was
      lying around: k8rs runs on the user's machine against a kubeconfig
      ([invariant 3](CLAUDE.md)), so a cluster reached over the network is the
      real trust model, and it is the only way a dropped connection can be
      tested by actually dropping one. Findings:
      [§ Verified against a real cluster](#verified-against-a-real-cluster-2026-08-11).
- [x] **just** — packaged (`1.58.0`), install with the rest.
- [x] **git** (2026-08-12) — repository initialized on branch `main`, remote
      `git@github.com:murat-akpinar/k8rs.git`. History starts at `chore: init`:
      the pre-rename repository was deleted with the `r7s` name, so nothing
      before the design phase survives.
- [ ] A truecolor-capable terminal (for the Catppuccin palette)

> **Rules may be started before a cluster exists** — they only need pod JSON.
> But hand-written JSON is a bootstrap, never a committed fixture: it
> resembles reality, it is not reality. Phase 2 exists to replace it.

## Verified against a real cluster (2026-08-11)

Test host: a LAN machine, not this laptop — Ubuntu 24.04.4, 4 vCPU / 3915 MiB,
Docker 29.7.2, kind 0.32.0, `kindest/node:v1.36.1`, three nodes (1 control-plane
+ 2 workers), API served on port 6443.

**Deliberately not localhost.** k8rs runs on the user's machine against a
kubeconfig ([invariant 3](CLAUDE.md)), so the test path has real TLS, real
latency, and a connection that can actually be cut — which is the only way the
disconnected and login-expired screens in [states.md](screens/states.md) get
tested honestly. kind writes `127.0.0.1:<random>` into the kubeconfig by
default; `networking.apiServerAddress` + a fixed `apiServerPort` are what make
it reachable from another machine.

### Confirmed

| Assumption | Evidence |
|---|---|
| Server-side `Table` works for namespaced and cluster-scoped kinds ([invariant 12](CLAUDE.md)) | Pods and Nodes both return `kind: Table` with the columns `kubectl get` prints |
| `may_i` is performed with `create` — D23's reason for keeping it in `ops.rs` | `kubectl auth can-i -v=7` shows `POST .../selfsubjectaccessreviews` |
| The capability probe has a real "not installed" branch to render | `metrics.k8s.io` absent from `api-versions`; `kubectl top nodes` → *"Metrics API not available"* |
| `managedFields` pruning is worth doing | **29%** of the pod list on a *fresh* cluster. That is the floor, not the typical case — the field grows with every controller that touches an object |

### Contradicts a document

1. **"Table is a list representation and cannot be watched" is false.**
   `?watch=true` with the Table `Accept` header returns 200 and streams
   `{"type":"ADDED","object":{"kind":"Table",…}}`. The mechanism
   [resources.md](screens/resources.md) chose — watch metadata, re-fetch the
   Table debounced — is still right, but for a different reason: **every event
   re-sends the entire column schema, 3086 bytes of `columnDefinitions` to
   deliver an 82-byte row.** A 37× overhead is the argument; impossibility was
   never true.
2. **The `,application/json` fallback is unproven, not wrong.** The claim is
   that aggregated API servers may answer `406` to a Table-only `Accept`. This
   cluster has **zero** aggregated APIServices, so nothing exercised it. Keep
   the fallback — it costs one header — but it stays untested until
   metrics-server or another aggregated API is installed.
3. **"The exact columns `kubectl get` would show" needs a filter.** The server
   returns *both* sets in one response: `priority: 0` is plain `kubectl get`,
   `priority: 1` is `-o wide` only. Pods come back with nine columns, five of
   them priority 1. Without filtering on `priority == 0` the browser shows the
   wide view on every screen.

### Not written down anywhere yet

4. **Table rows carry `PartialObjectMetadata`, not the object.**
   `.rows[].object` has `metadata` and nothing else — no `spec`, no `status`.
   That is enough for display *and* enough for alerts to bleed through (name,
   namespace and uid are all present to match a row against a finding), but a
   report can never be built from a Table row. This is why `analysis.rs`
   fetches typed lists separately, already a step in
   [todo Phase 5](todo.md).
5. **`Quantity` is a string, and nothing in the ten approved crates does
   arithmetic on it. — OPEN, needs a decision.** The cluster hands back
   `cpu: "4"` *and* `cpu: "100m"`, `memory: "4009164Ki"` *and* `"70Mi"`.
   `k8s-openapi` models this as `Quantity(pub String)`: no parsing, no
   comparison, no addition. The Capacity report, drain safety, waste **and the
   `capacity  N ▲` sidebar badge** all need `sum(requests) vs allocatable`.
   This is an unrecorded gap against [invariant 10](CLAUDE.md). Two ways out: a
   ~40-line suffix parser of our own (`m`, `k/Ki`, `M/Mi`, `G/Gi`, `T/Ti`,
   `P/Pi`, `E/Ei`, plus decimal exponents) living with the rules, or an
   eleventh crate.
6. **C2 does not need the API at all.** The API server's serving certificate
   comes off the TLS handshake — `CN=kube-apiserver`, issuer `CN=kubernetes`,
   one year, SANs including the address the user typed. `/version` also answers
   anonymously. So the certificate-expiry warning still works for a user whose
   RBAC allows nothing else.
7. **C3 needs a manufactured fixture.** kind does produce CSRs — two kubelet
   bootstrap requests — but they arrive `Approved,Issued`. A *pending* CSR, the
   thing rule C3 reports, has to be created deliberately.
8. **kind's kubeconfig has no `exec` block and no `insecure-skip-tls-verify`.**
   Both the D19 "your login expired" path and the TLS-not-verified header
   warning need a hand-written kubeconfig; kind will never produce either.
9. **kind node allocatable is not physically true.** Each of the three nodes
   reports `cpu: "4"` and `memory: 4009164Ki` — the whole host, three times
   over, on a 4-core / 3915 MiB box. Fixtures captured here are internally
   consistent, so rules compute correctly against them, but they do not
   describe a real cluster's per-node isolation. Any capacity fixture needs a
   line saying so, or someone reads 12 cpu as real.
10. **The browser's grouping problem is 67 api-resources across 22 API groups**
    — 33 namespaced, 34 cluster-scoped — on a *bare* cluster, before a single
    CRD exists.
11. `/version` on 1.36 also returns `emulationMajor/Minor` and
    `minCompatibilityMajor/Minor`. Version skew reads `gitVersion` for the
    control plane and `Node.status.nodeInfo.kubeletVersion` per node; the
    emulation fields are a newer concept and are **not** what skew means.

### Measured cost

Three nodes plus one probe pod: **1568 MiB used of 3915, 2347 MiB available.**
Comfortable for rules, fixtures and the write path. But
[todo.md](todo.md)'s *"measure resident memory against 10 000 pods"* does not
fit here — etcd plus the apiserver watch cache would exceed what is left. That
step needs a bigger host or a recorded snapshot, and knowing it now is cheaper
than discovering it in Phase 7.

## kind test manifest

The deliberately broken pods needed for writing rules. Numbers match the
rules table.

[`scripts/broken.yaml`](scripts/broken.yaml) — **the runnable copy, and the
only copy**. Applied with `scripts/cluster.sh break`. It used to be inlined
here; two copies of a manifest drift, and the one that drifts is always the one
in the prose.

| Rule | Pod | State it produces |
|---|---|---|
| 2 | `broken-oom` | OOMKilled, exit 137 — memory limit 64Mi against a 250M allocation |
| 1 + 6 | `broken-crashloop` | CrashLoopBackOff, exit 1. Needs minutes to enter backoff |
| 3 | `broken-image` | ImagePullBackOff — an image on a registry that does not resolve |
| 4 | `broken-config` | CreateContainerConfigError — `envFrom` a ConfigMap that does not exist |
| 10 | `broken-pending` | Pending, unschedulable — requests 500 cpu |
| 8 | `broken-hostpath` | hostPath mount of `/`, writable → must come out CRITICAL |
| 7 + 11 | `broken-readiness` | Running but never Ready — the readiness probe always fails |
| 9 | `broken-nolimits` | No limits set. **Not an alert** — this fixture exists to prove the *Capacity report* row |
| 12 | `broken-stuck` | Stuck Terminating: a finalizer nothing removes. Applied by the script, put into Terminating by the capture step |

`broken-stuck` is why `cluster.sh unbreak` patches the finalizer away before
deleting — a plain `kubectl delete` on it never returns.

Fixture capture:

```sh
kubectl apply -f broken.yaml
# wait a few minutes for states to settle (CrashLoop must enter backoff)
# Sanitization (REQUIREMENTS.md G-5): drop managedFields + annotations
# (last-applied-configuration is a full copy of the spec, env values included),
# redact env values. Raw `kubectl get -o json` is never committed.
sanitize='del(.metadata.managedFields, .metadata.annotations)
  | (.spec.containers[]?.env[]? | select(has("value")) | .value) = "REDACTED"'
kubectl delete pod broken-stuck --wait=false   # rule 12: leaves it Terminating
for p in oom crashloop image config pending hostpath readiness nolimits stuck; do
  kubectl get pod broken-$p -o json | jq "$sanitize" > tests/fixtures/$p.json
done
kubectl get events --field-selector type=Warning -o json \
  | jq 'del(.items[].metadata.managedFields, .items[].metadata.annotations)' \
  > tests/fixtures/events.json
# record which k8s version the fixtures were captured from (drift tracking)
kubectl version -o json | jq -r .serverVersion.gitVersion > tests/fixtures/K8S_VERSION
```

# Project name: k8rs ✅ (decided 2026-08-12, replaces r7s)

**Expansion: `k8s` + `rs` — "Kubernetes, in Rust."** Not one coined word but
two marks the reader already knows, joined: `k8s`, the numeronym everyone in
this ecosystem expands without being taught, and `rs`, the extension a Rust
file carries and the suffix the ecosystem puts on *this thing, in Rust* —
[`kube-rs`](https://github.com/kube-rs/kube), `serde-rs`, `tokio-rs`. Said out
loud: "kay-eight-arr-ess", or simply "kubernetes-rs".

**Why the numeronym rule was dropped.** `r7s` obeyed the `k8s` construction
exactly — `r|ustnete|s`, seven letters elided — and that was the flaw: the
reader has to be handed the expansion (`rustnetes`) before the name means
anything, and `rustnetes` then collides with
[rusternetes](https://github.com/calfonso/rusternetes) (716★, "kubernetes,
reimplemented in Rust") inside the very sentence that would explain it. `k8rs`
needs no expansion: both halves arrive already expanded, so the name is
legible on first sight to exactly the audience the tool is built for. It does
not claim to be a numeronym — it is a compound, and reads as one.

## What the name does not say

`k8rs` carries Kubernetes and Rust; it does not carry **TUI**. That is
deliberate — a third element makes it unsayable — so it is the tagline's job,
permanently: *the Kubernetes TUI, in Rust*. A description that omits
"terminal" leaves the name reading like a library.

**Accepted risk:** [`kube-rs`](https://github.com/kube-rs/kube) — the client
crate this project is built on — is the *other* thing called "Kubernetes in
Rust". `k8rs` will sometimes be taken for a library. Not solved, mitigated:
every first line (repo description, crates.io summary, README) leads with
what it *is* — a terminal UI — before what it is made of.

## Availability (verified 2026-08-12)

- crates.io, npm, PyPI: `k8rs` free.
- GitHub user/org `k8rs`: free. Docker Hub namespace `k8rs`: free.
- GitHub repos named `k8rs`: two abandoned hobby projects — a 2★ "Kubernetes
  operator written in Rust" untouched since the day it was created
  (2024-12-01), and a 1★ "for learning purposes only" repo with no commit
  after creation (2025-01-24). No incumbent to displace.
- Rejected on the same check: `kr8` / `kr8s` (`apptio/kr8` is a real k8s
  config tool, `kr8s` a known Python k8s client), `r8s` (a phylogenetics
  program, a Samsung device codename, and an `r8s-rs` org), `krust`
  (krustlet, 3604★), `kargo` (3556★), `krab` (532★), `lazykube` (582★),
  `rubernetes` (existing org — and it is **Ruby**ernetes), `rustik`,
  `rustui`, `cure`, `mend`, `kraft`, `patina`, `vigil`, `krate` — all taken
  on crates.io or with a real incumbent.
- Runner-up kept on the shelf: **`kr8t`** — all four elements at once
  (**K**ubernetes, **R**ust, the **8** of k8s, **T**UI), free on every
  registry and on GitHub, and it reads as "crate". Lost because its digit
  sits mid-word: harder to say and to dictate than `k8rs`, where the digit is
  the one this ecosystem already says out loud every day.

## Lineage — three names, one filter

`rat8s` (2026-08-10) → `r7s` (2026-08-11) → `k8rs` (2026-08-12). Each name
died on a criterion the next had to satisfy, and the three together are the
test any future name must pass:

1. **It expands without a lesson.** `rat8s` expanded to nothing — `rat` + 8
   letters + `s` is not a word, and in this ecosystem every reader tries.
   `r7s` expanded only once taught. `k8rs` arrives expanded.
2. **It has one obvious pronunciation.** "rat-eights"? "rats"? "rat-kates"?
   That question ended `rat8s`. A tool recommended out loud to a colleague
   gets one reading, not three.
3. **It points at the user, not at the implementation.** `rat` was ratatui —
   a fact about our source tree, meaningless to someone learning Kubernetes.

## Logo

Still open, deliberately. The rat silhouette died with `rat8s`; `k8rs` is a
compound of two ecosystem marks and implies no picture. No mascot is better
than a mascot the name did not earn.

**First task (start of code phase):** publish a `k8rs` placeholder to
crates.io before someone else takes the name — and claim the GitHub org
`k8rs` in the same sitting.
Fallback if the name is ever lost: **k8ray** ("the X-ray of k8s").

## Tagline (name-independent)

> Kubernetes TUI in Rust — fast, safe, zero-runtime.

## Inspiration / reference tools

- [k9s](https://github.com/derailed/k9s) — competitor/neighbor; explorer, we are the interpreter
- [krr](https://github.com/robusta-dev/krr) — resource recommendations
- [KubeUI](https://github.com/IvanJosipovic/KubeUI)
- [khi](https://github.com/GoogleCloudPlatform/khi) — log visualization
- [snorlax](https://github.com/moonbeam-nyc/snorlax)
- [rusternetes](https://github.com/calfonso/rusternetes) — ⚠️ not a TUI, a web console (see the design section)
- [goldpinger](https://github.com/bloomberg/goldpinger) — source of the v4 mesh idea
- [Keda HTTP scaling](https://github.com/SevginGalibov/Keda-HTTP-Add-On-Scaling)
