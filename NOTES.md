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

- **Rule 12 — pod stuck Terminating.** A `deletionTimestamp` already in the
  past means a finalizer or a wedged kubelet is holding it. Costs
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
| `⇧p` | the previous container's log (`--previous`) | the log tab only; unbound elsewhere |
| `q` | quit | everywhere; refused while a write is in flight ([dialogs](screens/dialogs.md)) |

**`⇧p` was bound before it was recorded.** It sat in the `detail.md` footer
with no row here, so the one screen that promises to show every key —
`help.md` — did not show it, and neither did `f` or `c`, which *were* recorded
here. Fixed on both sides: the three log-tab keys are now on the help screen,
and `⇧p` has a row. A key that exists in a mockup and nowhere else is a key
nobody agreed to.

This table is the collisions and the exceptions, not the full map — the full
map is `help.md`, which is the screen the user actually reads.

**Deleted: the severity filter.** It existed because the Alerts list was going
to be long; owner grouping ([D3](#d3--findings-group-by-owner-not-by-pod))
made it short, and severity is already the sort order. One key and one feature
fewer.

**Deleted: the manual reconnect key.** kube-rs backs off and reconnects on its
own; the requirement was always that the state is *visible*, never that the
user drives it. The disconnected banner reports, it does not ask.

### D13 — licence: `GPL-3.0-or-later` (reversed 2026-08-12)

Decided as `MIT OR Apache-2.0` on 2026-08-11 for one reason — it is the Rust
ecosystem default — and reversed on 2026-08-12 by the author's actual
requirement: *nobody repackages this as their own closed product.*

**What was not the deciding factor:** attribution. MIT, Apache-2.0 and GPL all
require the copyright notice to travel with the code, so "my name stays on it"
was never at risk under any of them.

**What decided it:** under MIT/Apache anyone may take k8rs, modify it, and ship
a **closed-source** product; the notice survives in a licence file nobody
reads, the product does not. GPL-3.0 permits selling — it has never forbidden
that — but a distributor must hand over the source under the same licence, so
the author's name and the freedom travel with every copy. That is the
requirement, stated in licence terms.

**What it costs, honestly:** k8rs cannot be embedded as a library inside a
closed-source tool. For a standalone binary that talks to a kubeconfig, that
is close to no cost — running it, internally or commercially, is unrestricted
for everyone. Some organisations do keep GPL policies aimed at *linking*;
none of them prevent running a CLI.

**Unchanged:** `cargo publish` needs the `license` field either way
(`GPL-3.0-or-later`, SPDX). `deny.toml`'s copyleft rejection for
*dependencies* stays exactly as it was — that policy is about what we pull in,
not about what we publish, and every approved crate is MIT/Apache, which is
GPL-compatible in this direction.

**Not chosen: AGPL-3.0.** Its extra clause covers use *over a network*, and
k8rs runs on the user's machine against their own cluster — there is no
service to close. Revisit only if a hosted version ever exists.

**The name is separate from the licence.** GPL covers the code, not the
trademark: "k8rs" as a name is not licensed with it, so a fork must rename
itself rather than ship "k8rs" with someone else's product behind it. That
sentence belongs in the README when it is written.

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
  `now` comes from the user's laptop. A machine a few minutes *behind* the
  cluster produces a negative age, and *"in -3 minutes"* on the first screen a
  beginner sees is worse than useless. A non-positive age renders as
  **"just now"**. *(This sentence said "fast" until 2026-08-12 and had the
  direction backwards — a fast laptop inflates ages, it does not negate them,
  and it is the half that manufactures findings. What the clamp actually
  protects, and what the fast half needs instead, is
  [D55](#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12).)*

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

### D27 — two findings the open watch already paid for (2026-08-12)

A pre-code read of the rule set against what an operator hits in a normal
week. Both items below are computed from the **Pod object that is already
being watched** — no new stream, no new dependency, no new screen. They were
missing because of where they were filed, not because they cost anything.

**1. Init containers were invisible.** Every pod rule reads
`status.containerStatuses`. `status.initContainerStatuses` is a *separate
array*, and a pod stuck at `Init:CrashLoopBackOff` or `Init:Error` therefore
produced no finding at all — while `kubectl get pods` shows it plainly. Init
containers are where migrations, config fetches and wait-for-dependency loops
live, so this is not an edge case: it is the first thing that breaks in a
freshly deployed app, and the tool was silent on it. Rules 1–6 read both
arrays; the finding names which init container, because "the app container is
fine and the init one is not" is the whole diagnosis.

**2. "Pending, and why" does not need the Events watch.** Rule 10 was deferred
to v0.5 bundled with rule 11 (probe failures), on the reading that both need
Events. Rule 11 does. Rule 10 does not — the scheduler writes both the verdict
*and* the human sentence onto the pod itself:

```
status.conditions[type=PodScheduled]:
  status:  "False"
  reason:  Unschedulable
  message: "0/3 nodes are available: 3 Insufficient cpu."
```

That message is the answer to the single most common beginner question, and it
is sitting in a field the store already holds. The bundle is split: **rule 10
ships in v1**, rule 11 stays in v0.5 with the Events watch it genuinely needs.
Confirming the case: `broken-pending` was already one of the nine fixtures
Phase 2 captures — the data was being collected for a rule that was not going
to ship. N6 (which taint or nodeSelector is blocking it) already computes the
node side of the same question, so the two now arrive together instead of four
releases apart.

Neither item widens scope: no new watch ([invariant 6](CLAUDE.md)), no new
key, no new view. They are the same Alerts list, with two blind spots removed.

### D28 — the workload watch, and the blind spot it closes (2026-08-12)

**Decided: Deployments, StatefulSets and DaemonSets join Pods and Nodes in the
permanent watch set.** This changes [invariant 6](CLAUDE.md), which is why it
is a decision and not a task.

**The blind spot.** Every v1 rule reads a Pod. When the pods were never
created — a ResourceQuota denial, an admission webhook rejection, a missing
PVC, a bad pull secret at ReplicaSet level — there is nothing to iterate over:
`kubectl get pods` is empty, the Deployment sits at 0/3, and k8rs reports a
healthy cluster. It is the most beginner-hostile failure class in Kubernetes
and it was the only one the tool could not see at all. Saying *nothing is
wrong* when something is, is worse than missing a finding: it is the one
behaviour that would make the Alerts screen not believable
([REQUIREMENTS G-6](REQUIREMENTS.md#flagged-by-more-than-one-role-at-once-highest-priority)).

**The second hole, closed by the same objects.**
[D3](#d3--findings-group-by-owner-not-by-pod) groups findings by
Deployment/StatefulSet/DaemonSet/Job, but a pod's `ownerReferences` points at
its *ReplicaSet*. Nothing said where `web-7d4f5c6b8` becomes `web`, so as
planned, M1 would have grouped under a hashed string — in the product whose
rule is that every visible string reads without a glossary.

**What is watched, and what is not.** The three workload kinds are watched for
`metadata` + `status` only (desired vs ready, the `Progressing` condition).
ReplicaSets are **not** watched: they are fetched on demand, cached by UID,
when a finding needs the `ReplicaFailure` message or a group heading needs the
Deployment behind a ReplicaSet. Jobs and CronJobs stay in the v0.2 J-series as
planned.

**Why this does not contradict what invariant 6 protects.** The invariant
exists to keep k8rs off the path that makes k9s heavy: a repeated
`LIST pods -A` ([§ Architecture](#architecture--where-lightweight-comes-from)).
Workload objects are two orders of magnitude fewer than pods and barely churn
— three low-traffic streams, not forty. The line the invariant now draws is
the honest one: *the Alerts view's own inputs are watched; everything the
browser shows is watched only while it is open.*

**New rules, W-series** (in `rules.rs`, pure, same as every other rule):

| # | Finding | Source |
|---|---|---|
| W1 | **The pods were never created** — quota exceeded, admission webhook denied, PVC missing | `ReplicaSet.status.conditions[ReplicaFailure]`, message shown verbatim |
| W2 | **Rollout gave up** — stuck part-way with no failing pod to explain it | `Deployment.status.conditions[Progressing].reason == ProgressDeadlineExceeded` |

W2 fires **only when no pod-level finding already explains the shortfall** —
otherwise a crashlooping Deployment produces two findings for one problem, and
the list stops being believable for the opposite reason.

**Costs, stated:** invariant 6 is rewritten; the 10 000-pod memory measurement
([D25](#d25--what-this-review-did-not-decide)) now includes the three kinds;
the read-only `ClusterRole` in [docs/security.md](docs/security.md) needs no
change — it already grants `get`/`list`/`watch` on all four `apps` resources,
so the least-privilege claim holds unchanged (checked 2026-08-12).

### D29 — a guard is proven only for the shapes it was fed (2026-08-12)

Found by auditing Phase 2 before running it, not by a failing build — which is
the point: nothing was failing.

**What was wrong.** `scripts/sanitize.jq` was written against a single Kubernetes
object: `del(.metadata.annotations)`, `.spec.containers[]?`, `.spec.nodeName`.
Half of `just fixtures` is `kubectl get <kind> -A -o json`, which returns a
**`List`** — the real objects sit under `.items[]`, a workload's containers two
levels below that under `.spec.template.spec`, and the top-level `.kind` is
`"List"`, not `"Node"`. Every path clause therefore addressed the wrapper and
missed the contents. Fed a poisoned List, the filter exited **0** and left
behind: the `last-applied-configuration` annotation, every env value, the
imagePullSecret, the selfLink, the node IPs — and it did **not** refuse a
capture whose node names came from a production cluster, because the refusal
tested `.kind == "Node" or .spec.nodeName != ""` at the top level only.

Eight of the fixtures `just fixtures` writes are that shape, `nodes.json` —
the most identity-carrying file in the set — among them.

**Why no test caught it.** `sanitize-test.sh` fed the filter one Pod. The Pod
path worked, so the test was green, and it would have stayed green through the
capture. This is the failure mode CLAUDE.md's *tests must not lie* already
names, arriving through a door it had not: not a test that cannot fail, but a
test that covers one of the two shapes the pipeline produces.

**The fix, and the shape of it.** The filter no longer contains a path-based
clause. `del(.. | objects | .annotations?, .managedFields?, …)` and a `walk`
over anything carrying `.env` reach every depth — List, pod template, bare
object — in one expression, which is also shorter than what it replaced. Node
identity is collected with `..` as well, and a Node is recognised by
`.status.nodeInfo` rather than by `.kind`, because the items of a List do not
reliably carry one. The refusal is now **any** foreign identifier, not *all*:
one real node name inside an otherwise-kind capture is the leak.

`sanitize-test.sh` feeds both shapes, plus a mixed List that must not be
laundered by the kind-shaped name sitting next to the foreign one. Run against
the old filter it reports eight failures; against the new one, none.

**Two rules came out of this**, both now in [CLAUDE.md](CLAUDE.md) rather than
left as a lesson learned:

1. **A check is proven only for the input shapes it was fed.** Enumerate what
   the real pipeline hands it, feed it each one.
2. **A derived list asserts it found something.** `write-guard.py` builds its
   ban list by parsing kube's `Api<K>` signatures; a `&self` wrapped onto its
   own line is not matched, so the method silently never gets banned — and
   "extracted nothing" prints the same as "nothing to extract". It now asserts
   `delete`/`patch`/`replace`/`create` are present before trusting the list,
   and its `--self-test` proves the wrapped-signature hole exists rather than
   assuming it does not.

**A third finding, same audit:** `just check` was documented as byte-for-byte
CI and was not. It omitted `test-guard.py --self-test` and the whole
`cargo deny` job — the reason cargo-deny's licence failure in Phase 1 could
only surface on a push. Both are in `just check` now, cargo-deny last so a
machine without it still gets the other nine results.

**And a fourth:** `just fixtures` deleted `broken-stuck` with `--wait=false`
and captured it immediately, never asserting the `deletionTimestamp` that rule
12 reads. `cluster.sh verify` checks the finalizer, but it runs *before* the
delete, so nothing checked the state actually being captured. The capture now
asserts it.

### D30 — the guards Phase 2 added, and the freeze they collided with (2026-08-12)

Four findings from finishing the Phase 2 audit, one of which is a plan change
rather than a fix.

**1. The CI yaml is frozen, and Phase 2 had to touch it anyway.** Phase 1 lists
the CI workflow among the files that freeze after it. Phase 2 then produced two
new security tests — `scripts/certs-test.sh` here, `scripts/verify-test.sh`
alongside it — and a security test that CI does not run is not a security test.
[CLAUDE.md](CLAUDE.md) now also requires `just check` to be the whole of CI, so
leaving CI behind would break a rule in the same breath. The freeze is
therefore broken deliberately, minimally (one `run:` step per test, no
restructuring) and in writing, which is what the forward-only rule asks for.

**The structural fix was not taken, and is the owner's call.** CI enumerates
its steps in a second list that has already drifted from `just check` once
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)). A
CI job that installed `just` and ran `just check` would make the drift
impossible instead of merely fixed, and would end the freeze collision for
good — every future check would land in one file. It costs one pinned
third-party action. Recorded, not applied.

**2. Addresses are stripped now, not eyeballed.** Phase 2's checklist asks a
human to confirm "no node IPs" in the committed fixtures, and a real capture
showed the sanitizer kept every one of them: `status.addresses[].address` on
nodes, `podIP` / `podIPs` / `hostIP` on pods. An eyeball step is not a guard —
it passes whenever the person running it is tired, which at that point in a
capture is everyone. `sanitize.jq` now redacts any value that is an IP address
as a whole string, which leaves the `Hostname` entry sitting in the same
`addresses` array untouched, because that one is the node name the N-series
joins on. The eyeball item stays; it just no longer carries the weight alone.

**3. `managedFields` can never reach a fixture, so it can never be tested
there.** `kubectl get -o json` omits them unless asked
(`--show-managed-fields=true` returns them, the default returns none), and the
sanitizer deletes them regardless. Both are correct. The consequence is not:
[docs/architecture.md](docs/architecture.md) claimed *"fixtures deserialize
through `k8s_openapi::Pod`, so the prune path is covered by the same tests"* —
it is not, and a test asserting "managedFields were pruned" would pass against
a fixture that never had any. The prune path is a `k8s.rs` concern and needs a
Phase 5 test against live watch data, where the field actually arrives. The
29% measurement in
[§ Verified against a real cluster](#verified-against-a-real-cluster-2026-08-11)
still stands — it was taken over the wire, which is where pruning pays.

**4. openssl writes the private key before it validates the dates.** Found
while verifying that the certificate fixtures carry the pinned dates
`todo.md` claims — they do, all three, exactly. But a malformed `-not_after`
makes openssl emit `expiring-client.key.pem` and *then* fail, and `set -e`
skips the `rm` that was supposed to remove it, leaving key material in the
fixture directory. A `trap … EXIT` covers the unhappy path now. The same edit
stopped discarding openssl's stderr: on an openssl older than 3.5 the run fails
with `Extra (unknown) options: "not_before"`, and hiding that message is what
would send the next reader reaching for a relative `-days`.

**One thing this produced for Phase 3:** the pinned `notBefore` of the expiring
and healthy fixtures is *exactly* the reference `now` of 2026-08-12, so C1's
validity comparison has to be inclusive (`notBefore <= now`). A strict `<`
classifies both fixtures as not-yet-valid, which is a third C1 case and not the
one either fixture is for.

### D31 — the sanitizer matched the whole string, and secrets are rarely the whole string (2026-08-12)

D29 fixed *where* the filter looked — every depth instead of a fixed path. This
is the other half: *how much* of a string it looked at. Both gaps were found by
scanning for the pattern rather than by a failing build, and both were shipped
with a green `sanitize-test.sh`.

**1. An address is not always the entire value.** The IPv4 rule was anchored
(`^…$`), so it caught `"10.244.2.2"` and missed both shapes a real capture
actually contains:

- `"10.244.0.0/24"` — a `podCIDR`, an address wearing a suffix. Six of them sat
  in the committed `nodes.json`.
- `"…dial tcp 172.18.0.1:53: no such host"` — an address quoted inside an
  English message, which is where kubelet puts the one it could not reach. One
  sat in the committed `image.json`.

`sanitize.jq`'s own comment argued the gap away: *"An address quoted inside an
English message is not caught; the foreign-capture refusal above is what covers
a capture from a cluster that is not kind."* That reasoning does not hold. The
refusal keys on **node names**, and this cluster's nodes are called `k8rs-*` no
matter what address the apiserver was given. `K8RS_APISERVER_ADDRESS` exists
precisely so the cluster can live on a real LAN — so a capture from a perfectly
legitimate kind cluster passes the refusal and carries that LAN address out
inside a message. The addresses actually found were kind and Docker defaults and
gave nothing away, which is luck, not a control.

IPv4 is now replaced *inside* strings. IPv6 stays anchored on purpose:
unanchored, `::` matches a Rust path, a C++ scope operator and every
`key::value` in a log line.

**2. Key material is never text when it arrives.** Every Secret value is
base64 in JSON, and base64 contains no `-----BEGIN`, so the PEM rule read a
`.data["tls.key"]` as ordinary prose and handed it straight back. Demonstrated
with one object carrying the same private key twice: the plain copy came back
`REDACTED-PEM`, the encoded copy came back untouched. The same blindness covers
`certificate-authority-data` in a captured kubeconfig and ServiceAccount token
Secrets.

Only the **key** half is redacted. A certificate is the public half by
definition, and `csr-pending.json`'s `.spec.request` is typed as a ByteString —
redacting it would leave C3's own fixture unparseable. Decoding is guarded
rather than attempted: jq's `@base64d` is a hard error on input it cannot
decode, which would abort the whole capture, so a string reaches it only after
matching the encoded PEM header, the base64 alphabet, and a whole-group length.

The replacement is deliberately not valid base64. Nothing here has a legitimate
private key in a fixture, so the only object this can fire on is one that should
never have been captured, and it should fail loudly at the next parse rather
than deserialize into a tidy placeholder nobody looks at.

**`fixture-audit.sh` had both holes too**, being written from the same two
regexes — so the guard that exists to catch what the filter misses was blind in
exactly the same places. It now checks the committed bytes for embedded
addresses and for base64-wrapped keys, and both checks were watched failing on
planted input before being trusted.

**The rule this adds to CLAUDE.md's *tests must not lie*:** *a redaction proves
only the framing it was written for.* D29 asked which shapes the pipeline
produces. This asks where inside a value the secret can sit — whole string,
substring, or another encoding of it — and requires one planted case per answer.

**3. The same question, asked of the refusal (2026-08-12, second pass).** The
foreign-cluster refusal read exactly two fields: `.nodeName`, and a Node's own
`.metadata.name`. A node name identifies infrastructure in four more places,
all of them ordinary:

- `.status.nominatedNodeName` — where the scheduler parks a name before it
  commits to it
- `.spec.nodeSelector["kubernetes.io/hostname"]`
- `.metadata.labels["kubernetes.io/hostname"]` — which the committed
  `nodes.json` carries for all three nodes, so this is not a hypothetical field
- `matchExpressions[] | select(.key == "kubernetes.io/hostname") | .values[]`

A production capture reaching the sanitizer through any of them was sanitized
and written instead of refused. All four are now collected, each with its own
assertion so a partial fix cannot pass. What was **not** done is Agent-suggested
and deliberately rejected: making the refusal positive — "refuse when no node
identifier is found but a hostname-shaped string exists" — would refuse
`deployments.json` and `services.json`, which legitimately contain no node
field at all.

**4. IPv6 written out in full** (`2001:0db8:0000:…:8329`) carries no `::`, so
it matched neither branch. Added as a second anchored alternative; unanchored
IPv6 is still refused as a matter of policy, because `::` alone matches a Rust
path and every `key::value` in a log line.

**5. `check-docs.py` fenced on ``` only.** A heading inside a `~~~` block became
an anchor here and does not on GitHub — the one way that script could call a
genuinely broken link green. It now remembers which marker opened the fence
rather than toggling a boolean, so a ``` inside a `~~~` block does not close it,
and it gained the `--self-test` the other three guards already had.

### D32 — one long-lived `development` branch, not one per phase (2026-08-12)

The branch-per-phase model (`feat/rules`, `feat/analysis`, … — named in
todo.md's phase headings since the design phase) is replaced by a single
long-lived `development` branch. Every box commits onto it; at phase close it
is merged into `main` by [CLAUDE.md § phase close](CLAUDE.md), item 7;
`development` is never deleted and work continues on it immediately.

**Why.** The per-phase branch was invented before anything had been built on
it, and by the time two phases had shipped it had cost more than it returned:
it added a create step, a delete step and a "which branch am I on" question to
every phase, and the delete step failed twice on tooling that refuses a remote
branch deletion. It also duplicated a boundary that already exists and is
already enforced — the phase itself, in todo.md.

**What it buys with the agents.** A rule already in force is that agents never
create, switch or delete a branch; they write files on whatever branch they are
handed. With one permanent branch that rule stops being a restriction anyone
can trip over, because the branch is simply always there.

**What it costs, and where that cost is now carried.** A branch per phase made
the freeze mechanically visible: `git diff main...feat/rules` was exactly one
phase, and a frozen file reappearing in it was impossible to miss. One shared
branch does not give that for free. The freeze therefore rests entirely on two
things that were already required — the PM reading every diff before it is
committed, and the phase-close second pass reading the phase end to end. If a
frozen file ever does slip through, this is the decision that made it possible,
and the answer is a guard, not a branch.

**Direction, and why no back-merge.** `main` only ever advances by merging
`development` into it. That keeps `development` an ancestor of `main` at all
times, so nothing has to be merged back; the moment someone commits directly to
`main`, that stops being true and this note stops being accurate.

Phases 1 and 2 ran on `feat/scaffold` and `feat/fixtures` and their headings
still say so — that is what happened, and rewriting it would make the plan
file lie about its own history.

### D33 — Phase 3 opens with one Phase 2 box still open, on purpose (2026-08-12)

Phase 2's last box — `just cluster-down` on the LAN host — is unchecked and
stays that way while Phase 3 runs. Phase 3 starts anyway. That is a plan
change, so it is written here rather than applied by quietly ticking the box.

**Why the box cannot be closed yet.** The box's own condition is "run it once
no further capture needs the cluster", and a capture still does: Phase 4's
Drain-safety and Waste reports have a negative fixture and no positive one,
because nothing in `broken.yaml` produces a StatefulSet, a PVC, a PDB or a
dead-selector Service. Tearing the cluster down now buys nothing and costs a
rebuild.

**Why Phase 3 does not have to wait for it.** Phase 3 is pure functions over
fixtures that are already captured, sanitized, audited and committed — 23 of
them, from `v1.36.1`. It needs no cluster, no network and no terminal
([invariant 5](CLAUDE.md)). The open box blocks nothing in it.

**Amended the same day: the host went down in a power cut and is unreachable,
so whether that cluster is still standing is now unknown.** It changes nothing
here, and that is the point of the pinned node image: `just cluster-up`
rebuilds the same `v1.36.1` cluster from `scripts/cluster.sh`, so the visit
that closes this box is a re-create if it has to be. Nothing was lost either
way — the fixtures live in git, not on the host. What the outage does prove is
that leaving a box open against a machine you do not control is the correct
record: had it been ticked on "the teardown is basically done", the plan would
now be claiming a step nobody can show ran.

**Where the trip now happens, and how it leaves nothing behind (2026-08-12).**
With the host gone, the rebuild runs on the developer's own machine at the
Phase 3 close — one visit that captures Phase 4's four missing fixtures and
then tears the cluster down, closing this box. It is set up to evaporate: the
docker socket is granted for the session with
`sudo setfacl -m u:$USER:rw /var/run/docker.sock`, which a docker restart
undoes, **not** by adding the user to the `docker` group — that group is
root-equivalent and would be a permanent privilege grant bought for a
half-hour capture. `KUBECONFIG` points at a scratch path so kind never writes
`~/.kube/config`, and `just cluster-down` removes the containers. Nothing
about the capture depends on this: `scripts/cluster.sh` pins the node image,
so the cluster is the same `v1.36.1` wherever it is built.

**What this is not.** It is not permission to start a phase on top of unfinished
work in general. The exception is narrow and holds only because the open box
and everything in Phase 3 are disjoint: no Phase 3 box reads anything the
teardown would change. Phase 2 remains formally open until the teardown runs
and Phase 4's missing fixtures are captured in the same visit to the cluster —
an honest open box, which [CLAUDE.md § phase close](CLAUDE.md) item 2 prefers
to a false tick.

### D34 — the temporary `main.rs` belongs to `dev-core` until Phase 12 (2026-08-12)

[CLAUDE.md § Ownership](CLAUDE.md#ownership--and-the-file-each-one-may-write) gave
`main.rs` to `dev-ui`, and the phase map in the same section gave Phases 3–7 to
`dev-core` — while Phase 3, Phase 5 and Phase 7 each write a box into `main.rs`
(the fixture-printing shell, the `--once` output, the headless ops driver). Two
sentences that cannot both be obeyed, found the first time a box needed the
file. The table is corrected rather than worked around.

**`main.rs` is `dev-core`'s while it is the temporary driver — Phases 3 to 7 —
and passes to `dev-ui` at Phase 12, where it is wired for real.** Nothing about
it draws before then: it parses `args`, loads a fixture or a client, and prints.
The file is the one place the ownership rule has a handover, and the handover is
a phase boundary, which is exactly where the plan already stops.

**The mechanical reason it cannot simply be skipped.** A file that no `mod`
declaration reaches is not part of the crate, and `cargo fmt` never visits it —
the correction Phase 1's guard ledger already paid for once. So `src/rules.rs`
does not exist as far as fmt, clippy or the test runner are concerned until
`mod rules;` is in `main.rs`. Writing the module and the line that reaches it are
one change, not two, and they cannot belong to two different agents.

### D35 — `just mutants` is a check that cannot fail, and the justfile unfreezes for one line (2026-08-12)

Three findings came back from the test gate on Phase 3's first box. They are
closed here, because a finding closed in conversation is a finding nobody can
find again.

**Accepted, applied one box later.** `cargo mutants --file src/rules.rs --file
src/analysis.rs` prints `Found 0 mutants to test` and **exits 0** — and it does
exactly the same for `--file src/does-not-exist.rs`, which was tried. That is
the derived-list failure [CLAUDE.md § Code phase rules](CLAUDE.md) names: a
path typo and a clean run print the same line. It needs a floor assertion on
the mutant count. It is **not** applied in this box, because today the zero is
honest — `rules.rs` holds types and no functions — so a floor now would be a
red gate nobody could pass, and an impassable gate is the thing that teaches
everyone gates are decorative. It lands in the box that first puts a function
in `rules.rs`, written by `tester`. **The `justfile` froze at the Phase 2
close; it unfreezes for that one line and nothing else** — recorded here rather
than done quietly, which is what the forward-only rule asks for.

**Fixed now.** `.gitignore` had no `mutants.out` entry, so every `just mutants`
run left `mutants.out/` and `mutants.out.old/` untracked in the repo root.

**Rejected, with the reason.** A variant inserted *above* `Critical` leaves the
severity-order test green. That is correct behaviour, not a hole: the test's
claim is "declaration order is severity order", and inserting a variant keeps
it true. A test pinned to the variant set instead would go red on a legitimate
addition — which is a different lie, and a more expensive one, because the
person who adds a severity would learn to edit the test to get past it.

### D36 — the `Finding` shape the review sent back (2026-08-12)

Phase 3's first box was written exactly to the shape
[docs/architecture.md § The shared contract](docs/architecture.md#the-shared-contract)
specified, and the operator review blocked it on three counts. The code was
faithful; **the documented contract was the thing that was wrong**, which is
why this is a decision and not a bug fix. The box exists to decide the identity
every later rule files under, and `rules.rs` freezes at the end of this phase —
so the shape is settled here, before there is anything to migrate.

**What was missing, and the concrete case that proves each.**

- **The pod the finding is about.** One crashlooping pod fires rules 1, 5, 6 and
  7 — checked against this repo's own `tests/fixtures/crashloop.json`, which is
  ready=false, 5 restarts, `CrashLoopBackOff`, exit 1. A `views.rs` holding only
  `Vec<Finding>` can count findings, not pods, so the card renders **"4 of 5
  pods" for a single broken pod** — the blast radius, wrong by 4× on the most
  common failure in the rule set. `Finding` now carries `object` beside `owner`:
  the numerator is the count of distinct `object`s. It is also the only possible
  data source for [detail.md](screens/detail.md)'s "⏎ lists which pods of the
  group are affected", which had none.
- **A Node.** N1–N6 are boxes in this same phase and a Node is cluster-scoped.
  The old shape could only say `{Pod, "", "k8rs-worker2"}`, which renders
  `/k8rs-worker2`, offers `logs` and a container picker for a machine, and
  builds `kubectl describe pod k8rs-worker2 -n ""` — invariant 4 lying in the
  one record that may not. `OwnerKind::Node` exists and `namespace` is
  `Option<String>`, `None` meaning cluster-scoped.
- **Owners that are not one of five kinds.** A CronJob's pods are owned by a
  Job whose name changes every run, so filing under `Job` grows a fresh card
  every schedule tick — the flood [D3](#d3--findings-group-by-owner-not-by-pod)
  exists to prevent. `--cascade=orphan` leaves pods whose chain stops at a
  ReplicaSet, and under Argo Rollouts or any database operator the
  ownerReference kind is a CRD. `CronJob`, `ReplicaSet` and `Other(String)`
  cover all three; `Other` prints the kind it actually got instead of asserting
  a false one. Resolving ReplicaSet → Deployment stays `k8s.rs`'s job
  ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)) —
  this type only has to hold either answer.

**Two `String`s became `Option`s for the same reason.** `kubectl_cmd` was
empty-as-absent, so "no such command exists" (rule C1 reads a local file) and
"the rule author forgot" were the same value, indistinguishable by any test.
`uid` was absent entirely, and [D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms)
requires the confirm dialog to hold one: in the Alerts view the selected object
*is* a `Finding`, so without it a card drawn before an Argo re-sync can scale
the object that replaced the one the operator inspected. The struct already
made this argument in its own doc comment — `Option<Owner>` would be "the arm
every call site forgets" — and then took the opposite side one field down.

**Rejected: `resourceVersion` does not go on `Finding`.** D22 names it beside
the uid, but it belongs to the moment the dialog opens, not the moment the rule
ran; a rule-time value would be stale by construction and would invite a
comparison that means nothing. The uid answers "is this still the same object",
which is the question a stale card actually poses. Recorded because the field's
absence otherwise reads as an oversight.

**The gap this opened, which no code change closes.** Every pod fixture in the
repo has `ownerReferences: null` — all twelve, checked one by one; only the two
ReplicaSet captures carry an owner at all. `scripts/broken.yaml` creates bare
pods, so **every positive test in Phase 3 files under `OwnerKind::Pod` and the
four workload branches ship with no positive fixture** — and `cargo mutants`
would not object, because nothing exercises them. A grouping key tested only in
its no-owner case is [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
again, one layer up. `broken.yaml` needs a Deployment-owned broken pod, which
needs the cluster — so it is a new open box in Phase 2, captured on the same
trip as [D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)'s
teardown, not a note in a review nobody reads again.

### D37 — a controller's message is a status field, not a payload (2026-08-12)

Two recorded rules collided the moment `Finding.evidence` was written down.
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) says W1
shows the controller's message **verbatim** — "exceeded quota: pods, used 0,
limited 0" is the whole diagnosis, and paraphrasing it is how a tool becomes
useless. The `evidence` field's own rule says a finding names **fields, never
payloads**. A validating webhook that echoes the submitted object back into its
denial message — several in the wild do — puts an env value inside a controller
message, and now both rules apply to one string.

**Verbatim wins, and the payload rule is narrowed to say what it always meant.**
The rule governs what *k8rs goes and fetches*: it never reads Secret data, never
renders an env value, never puts either in evidence — that is absolute and it is
the [security gate](CLAUDE.md#security-gate--run-this-list-on-every-change-no-exceptions)'s
"environment variable values are never displayed". A message a controller wrote
into a status the user can already `kubectl describe` is a different thing: k8rs
did not go looking for it, it is not privileged relative to the reader, and
hiding it would leave the user staring at "the pods were never created" with the
reason blanked out. Refusing to show what `kubectl` shows is not a security
control, it is a tool that lies by omission.

**What that costs, and what pays for it.** The message is untrusted free text,
so it takes the same treatment every other API string takes: control characters
stripped at the render boundary ([invariant 9](CLAUDE.md)) and length bounded at
ingest, which Phase 5 already owes for the 50MB-annotation case. The one thing
that changes is a warning where it belongs: `--once` output is what gets
redirected into CI logs and pasted into tickets, so it is the place a webhook's
echo reaches an audience wider than the person at the terminal. That is a
documentation line for the `--once` box, not a reason to blank the field.

**Addendum to [D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) —
how a cluster-scoped finding renders.** Fixing the type left the mockups
asserting the opposite, so `screens/` moved with it: the identity line is
`namespace/name` for a namespaced object and a bare `name` for a cluster-scoped
one, stated once in [screens/README.md](screens/README.md) rather than fixed in
the one place it was caught. Three consequences were settled there, all worth
keeping: dropping `infra/` also drops the only clue that `node-3` is a machine,
so **the kind moved into the sentence under the name** (N2 now reads "This node
refuses new pods (cordoned)"); it is `node-3`, **not** `node/node-3`, because a
`kind/name` prefix puts a slash back on the very line whose slash the reader was
just taught means a namespace; and **N6's card is a workload card, not a node
card** — a Pending pod that cannot be placed is about the pod, with the node
named in the evidence, so only N1–N3 produce a node card at all. The drain
dialog was the proof that this was never cosmetic: its title said
`Draining infra/node-3` while the command log under it said
`kubectl drain node-3`, and the fake namespace was what hid the disagreement.

### D38 — the grouping key was a derive, and a derive cannot be told what to ignore (2026-08-12)

The shape [D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) settled
carried one defect no compiler could catch. `ObjectId` derived `Hash` and `Eq`
over all four fields, `uid` among them, and `views.rs` groups by `owner` — so
these are two different grouping keys:

```
ObjectId { kind: Deployment, namespace: Some("payments"), name: "web", uid: Some("9f2c-aaaa") }
ObjectId { kind: Deployment, namespace: Some("payments"), name: "web", uid: Some("9f2c-bbbb") }
```

Two cards for one Deployment: the flood [D3](#d3--findings-group-by-owner-not-by-pod)
exists to prevent, arriving through the field added to prevent a *different*
bug. It compiles, the suite passes, and it would surface in Phase 9, months
after `rules.rs` froze.

**The mechanism above is the second one written here; the first was wrong and
the correction is the useful part.** This entry originally said the two uids
were `Some` and `None` — a rule that resolved ReplicaSet → Deployment without
the on-demand fetch having the name but no uid. The operator review killed it:
`uid` is a *required* field of `metav1.OwnerReference`, non-`Option` in
`k8s-openapi` and present in this repo's own `quota-replicasets.json`, and
without the fetch there is no Deployment **name** either, because `web` is not
derivable from `web-7d4f5c6b8`. The pair was hand-built for a test and then
asserted as though it came off a cluster. The reachable case is a **Deployment
deleted and recreated under the same name**: old-generation pods still
terminating under uid-A while new pods run under uid-B, which is what any Argo
prune-and-recreate produces. Same fix, honest reason — and the difference is
not academic, because the old wording told the `k8s.rs` author that a workload
owner with `uid: None` was normal, and `uid: None` is the one value that
silently disables [D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms).
**A workload owner always carries a uid; C1's kubeconfig certificate is the
only `None` in the product.**

**`Hash` is dropped from `ObjectId` and the identity becomes a method** —
`group_key() -> (&ObjectKind, Option<&str>, &str)`. Not a doc comment asking
callers to be careful: the wrong key stops compiling, so it is unrepresentable
rather than discouraged. Two details, both found by probing rather than by
reading: the wall is not the declaration — `HashMap<ObjectId, _>` *declares*
fine, because the `Hash` bound sits on `insert`/`get`/`entry`, not on
`HashMap::new` — and when it does fire, rustc says
`help: consider annotating ObjectId with #[derive(Hash)]`, offering the
two-cards bug as the fix in the text a future developer reads before any doc
comment. The comment in `rules.rs` pre-empts that advice by name. It also makes the
box's own sentence mechanical — the identity is decided in the bottom layer
instead of re-derived in `views.rs`, where a second definition would drift.

**`Eq` stays derived over all four fields, on purpose.** It answers
[D22](#d22--a-confirmation-can-outlive-the-thing-it-confirms)'s question — is
the object about to be mutated still the one the operator inspected — and an
`Eq` that quietly ignored the uid would answer *that* wrong, which is worse than
the bug being fixed. Two questions, two mechanisms: the derive for identity of
the object, the method for identity of the card.

**The gate that found it was itself broken, which is the more expensive half.**
Hunting this, `tester` deleted every test from `rules.rs` and ran the guard:
`test-guard: 0 declared, 0 listed, 0 ignored — OK`, `cargo test` green,
`just check` green. **The whole suite could be deleted and CI would applaud.**
Every rule in the guard was a comparison between two counts, and comparisons all
hold at zero — [D26](#d26--a-green-build-that-proves-nothing-2026-08-12) item 2
said "CI asserts a non-zero test count" and nothing ever did, in `just check` or
in the workflow. The floor is in, with a positive and a negative self-test, and
it is passable today, which is what separates it from the `just mutants` floor
[D35](#d35--just-mutants-is-a-check-that-cannot-fail-and-the-justfile-unfreezes-for-one-line-2026-08-12)
deferred. A guard whose rules are all relative is the derived-list failure in a
new costume: with nothing to compare, everything agrees.

**One assertion in the new tests has never been red, and that is recorded
rather than hidden.** The negative test's fourth case — cluster-scoped versus
namespaced — cannot fail in any way its "different namespace" case does not
fail first, and it survived the one mutation aimed at it (`group_key`
flattening `None` to `Some("")`). It stays, because it states intent for a
shape the rest of the suite does not name, but by this project's own rule it
proves nothing yet. What would make it real is written into the C1 box in
[todo.md](todo.md): `Some("")` is unreachable only while an `ObjectId`'s
namespace comes from an object's own `metadata.namespace` — measured, not
assumed, against all 23 captures, `sanitize.jq`, and the DNS-1123 label rule
that forbids an empty namespace name. A C1 that builds its identity from the
*effective scope* instead would make it reachable, because `--namespace` is
parsed from `args` with no validation.

**Accepted as a known ceiling, not fixed:** `#![expect(dead_code)]` at the top of
`rules.rs` is module-wide, so an item written *after* it can be dead and
invisible — a never-called function was appended and clippy stayed green.
Narrowing it to per-item would cost a line of noise on every type, so it stays
module-wide.

**Its exit was also mis-stated, and the correction is a pre-authorisation.**
This entry first called the attribute self-limiting, "because the file freezes
with everything constructed". Nothing guarantees that: `Severity::Info` has no
producer in `rules.rs` today, `analysis.rs` gets one in Phase 4, and whether
N4's version skew adds one here is N4's own box. While *any* item stays
unconstructed the expectation is still fulfilled and the line cannot be
removed — removing it makes `dead_code` fire and the build go red. It becomes
**unfulfilled** only when the last item is constructed, which may be phases
later, and `-D warnings` then turns `just check` red pointing at
`src/rules.rs:11` — possibly a file frozen a phase earlier. `clippy
--all-targets` evaluates it per target, and the two targets already disagree:
with the attribute removed, the bin reports five dead items and the test target
two, so they can flip at different boxes. **Whichever box constructs the last
item in this file deletes that one line. The deletion is authorised here, in
advance: it is not a freeze violation and it does not need a new decision** —
which is the point of writing it down while the red build is still hypothetical
and nobody is under pressure to explain it.

### D39 — a Node owns pods, and three more things the shape could not say (2026-08-12)

The second operator review of the same box. The shape held; what did not hold
were the sentences written around it, each falsified by a cluster this repo
builds or a fixture it has already committed.

**A mirror pod is owned by a Node, so "a Node is nobody's owner" was false on
`just cluster-up`'s own cluster.** kubelet writes an `ownerReference` of kind
`Node` onto every static pod, which on kind and on any kubeadm cluster means
`kube-apiserver-*`, `etcd-*`, `kube-scheduler-*` and `kube-controller-manager-*`
all have one. Left alone, an `etcd` that restarts four times after a laptop
suspend files under `{ kind: Node, namespace: None, name: "k8rs-control-plane" }`
— `kube-system` gone from the card a beginner needs in order to find the pod
again, four control-plane pods collapsed onto one card, and, since `views.rs`
picks the card shape from `owner.kind`, a machine-shaped card drawn for a
crashing pod. **An `ownerReference` of kind `Node` is discarded: the pod files
under itself, so `owner.kind` is `Pod` and `owner == object`.** A mirror pod
has no controller that can be scaled, restarted or rolled, and the card stays
`kube-system/etcd-k8rs-control-plane`. `ObjectKind::Node` appears in `owner`
only when the finding is *about* the node — N1–N3. Stated that mechanically on
purpose: the first wording said "a `Node` in the owner role is the no-owner
case", which reads as an instruction to *keep* the Node, and an implementer
obeying it arrives at the exact card this ruling forbids. The upstream behaviour is documented but asserted by nobody here, so
the same trip that captures the owned pod captures `-n kube-system` too.

**The numerator rule printed "1 of 0 pods".** `object`'s doc defined the
numerator as distinct `object.name`s in the group, which holds only while every
object in it is a pod. W1's object is a **ReplicaSet** — `quota-replicasets.json`
is `broken-quota-59654c756`, `ReplicaFailure`, `status.replicas: 0` — so the
group renders 1 of 0, on the failure class
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12) added
the workload watch to stop the tool lying about; W2 breaks it the other way and
counts a Deployment as one of its own pods. **The numerator counts distinct
objects whose kind is `Pod`, and a group with none of them has no `n of m` at
all** — the shape a node card already uses. **Distinct by the whole `object`,
not by `group_key()`**, which is the half that had to be said out loud:
`ObjectId` has no `Hash`, the type's own doc answers that compile error with
"use `group_key()`", and a reader following that advice while *counting* writes
the wrong thing. Not a bug being prevented — for objects of kind `Pod` the two
are provably identical on any snapshot, since `group_key` is
`(kind, namespace, name)` and two pods differ in it exactly when their names
differ. An intent being stated: `group_key()` answers *which card*, the whole
identity answers *what is counted on it*, and code that says the first while
meaning the second stops being right the moment something other than a pod is
counted.

**The first draft of that paragraph justified it with a scenario that cannot
happen** — a terminating `web-0` counted beside the `web-0` that replaced it.
Two pods cannot share a name in one namespace: the name is the etcd key, the
second create is rejected `AlreadyExists`, and a StatefulSet will not recreate
an ordinal until the terminating pod is gone from the API. It is the defect
[D38](#d38--the-grouping-key-was-a-derive-and-a-derive-cannot-be-told-what-to-ignore-2026-08-12)
recorded one round earlier — a pair built by hand for an argument and then told
as though it came off a cluster — repeated one field down by the same author,
in the same week, having just written the correction. Worth keeping visible for
that reason: the owner-uid divergence one field up *is* real, and the two look
identical on the page. What separates them is that the two Deployment
generations have different uids and their pods have different **names**, which
is exactly why those pods coexist and two pods called `web-0` never do.
[D4](#d4--the-flagship-example-promised-a-number-that-cannot-exist) is the
precedent: a number that cannot exist is not a rendering detail.

**`CronJob` was unreachable under this project's own least-privilege role.** A
pod names its Job and says nothing about the CronJob above it, so the grouping
needs a GET on the Job — and `k8rs-readonly` granted `""`, `apps`, `policy` and
`metrics.k8s.io`, with no `batch`. Under the role the docs tell people to use,
every tick would file under its own Job name: the churn the variant was added
to prevent, delivered to the user least equipped to explain it.
[docs/security.md](docs/security.md#rbac) now grants `batch: jobs` read verbs,
and the 403 degrades by name — the finding files under the Job and says the
CronJob could not be read. `cronjobs` is deliberately not granted: the Job's
ownerReference already carries the CronJob's kind, name and uid, so nothing
reads the object, and a role whose entire argument is least privilege does not
get to carry a resource it never GETs. Auditing that block found two more of
the same class, both fixed in the same edit — `certificates.k8s.io` for rule C3
and `discovery.k8s.io/endpointslices` for the waste report, each a Phase 5 box
with its fixture already committed, each a 403 waiting for the user who
followed the documentation. `cert-manager.io` stays out and is written into the
block as a commented-out opt-in, because C4 only exists where cert-manager is.
The lesson is the same one
[D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) learned about
docs: this rework synced `docs/architecture.md` and `REQUIREMENTS.md` for the
shape and forgot the file that says what permissions the shape needs.

**Two smaller rulings, both written into `rules.rs` because it freezes first.**
The members of a group must agree on the uid; where they disagree — the
recreate case again — the confirm dialog refuses and offers a re-read rather
than picking one, because the natural implementation is `.first()` and
`.first()` is whichever finding happened to sort first, which turns D22 into a
coin flip that reports "already gone" for a Deployment that is running. And
C1's identity is settled: `kind: Other("kubeconfig")`, `namespace: None`,
`name` = the kubeconfig **context name**, `uid: None` — otherwise its author
must either invent a kind the API never reported or put a sentence in a field
that kubectl lines are built from.

**The file was also two-thirds prose, restating these entries in slightly
different words.** CLAUDE.md keeps the *why* in NOTES and asks for comments
sparingly; a second copy in another wording is a drift generator, and this file
freezes with both copies in it. The comments that stay are the ones the
compiler cannot say — chiefly that rustc answers the missing `Hash` with
`help: consider annotating ObjectId with #[derive(Hash)]`, offering the
two-cards bug as the fix — plus the rulings a later box must obey, each
pointing here instead of re-arguing.

### D40 — the capture could not produce the shape, so the test sets one field (2026-08-12)

Phase 3's second box decoded the twelve committed pod captures into
`PodSnapshot` and every test passed. `tester` then corrupted the decode one
field at a time — 94 mutations, each applied to a pristine copy, compiled, run
and reverted against a hash — and **32 fields could be broken with the whole
suite staying green**. `PodSnapshot.owner` replaced by `id.clone()`:
`19 passed; 0 failed`. [D3](#d3--findings-group-by-owner-not-by-pod)'s one card per owner, the
premise the entire Alerts view is built on, was proven on a ReplicaSet and
never once on a pod.

**The cause is not the tests, it is the cluster.** Not one captured pod carries
an `ownerReferences` at all; no node in `nodes.json` is NotReady, cordoned or
under pressure; every workload fixture is either fully ready or all-null, so
`desired`/`ready` could be read from five different wrong status fields
undetected; the single hostPath mount carries no `readOnly` key, and rule 8
fires *on* writable, so a decode hardwiring "writable" is a false-positive
generator that passes; `allocatable` equalled `capacity` on all three kind
nodes, and the gap between them is the whole of N5. A branch whose input the
capture cannot contain is a branch no test can reach — and the obvious fix is
the one CLAUDE.md forbids, because hand-written JSON resembles reality and is
not it.

**Ruling: a decode test may start from a committed capture and set one field.**
The precedent was already in the box's own diff —
`a_forced_delete_beats_the_grace_period_the_spec_asked_for` takes the real
`stuck.json` and sets the grace period a `--grace-period=0` would have set. Made
general, with the bounds that are the difference between this and the thing it
resembles:

- it starts from a **committed capture**, never a literal object;
- **one field, or one coherent group of fields, per test**, and the comment says
  what value, why the API produces it, and why today's capture does not;
- **a third case the first draft of this ruling did not name**: a test proving
  *which* field is read must set the neighbours too, and the count is then not a
  measure of anything. `desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour`
  sets sixteen fields across three objects, because setting only
  `readyReplicas` cannot show that `ready` is not read from `availableReplicas`
  — five sibling counters, five **distinct** values, and every one of the four
  wrong-field decodes dies on it. That is not a coherence group, it is the
  discrimination mechanism, and it is the shape the rule wants most;
- the whole synthesized object must still be one **the API could emit**. The
  NotReady node synthesis set `DiskPressure` to `True` with the reason
  `KubeletHasDiskPressure` and left the captured message reading *"kubelet has
  no disk pressure"* — nothing asserted it, so nothing was red, but the licence
  for this entire technique is "a value the API demonstrably produces" and half
  an object is not one;
- it is a **decode** test. A *rule's* positive fixture stays a real capture —
  this never becomes the way a rule gets proven, which is the whole reason the
  Phase 2 capture trip exists;
- each such test **names the object the capture trip should bring back** to
  replace it, so the list of what the cluster still owes is in the tests rather
  than in someone's memory.

`From<StatefulSet>` is the one hole this cannot close — `statefulsets.json` is
an empty list, and synthesising a whole StatefulSet is hand-written JSON with
extra steps. It stays untested, says so in the code, and goes on the trip.

**Two syntheses are permanent, and saying so is part of the ruling** — a
"replace me later" that can never be replaced is the kind of note that decays
into furniture. A node whose `allocatable` differs from its `capacity` requires
`--kube-reserved` on the kubelet: kind reserves nothing, so that is a cluster
configuration change rather than a workload the capture trip can apply, and N5's
whole subject is the gap between the two numbers. A non-controlling
`ownerReference` is the other: producing one means contorting `broken.yaml` into
a shape no real workload has, which buys a fixture at the cost of the manifest
being an honest description of a broken cluster.

One coupling accepted knowingly: rule 3's image-pull message is pinned by
equality, and that message contains the sanitizer's `REDACTED-IP` placeholder,
so the decode test is coupled to the sanitizer's output. It stays — but **not
for the reason it was first written down here.** Restoring the real addresses
in `image.json` was tried, and `scripts/fixture-audit.sh` fails first and far
more legibly (*"[image.json] carries 5 IP addresses — this file never met
scripts/sanitize.jq"*) while the equality assert produces a 400-character
string diff inside a *decode* test. The audit is the first line of that
defence; the assert is redundancy, and describing it as the thing that prevents
silent acceptance would have been an overclaim that outlived the person who
made it.

### D41 — `cargo mutants` cannot see the defect it was put there to catch (2026-08-12)

Phase 3 ends on a box reading "`cargo mutants --timeout 90` clean over
`rules.rs` — a MISSED mutant is a rule change no test objected to". Run against
the decode above, it reports **1 missed of 34**; `tester`'s hand-written
mutations found **32**. The tool mutates return values and match guards, and 30
of the 32 holes live in **struct-literal field assignments** — `restarts:
c.restart_count` becoming `restarts: 0` is not a mutation it generates.

The one it did find is worth naming, because it is the same one a human found
independently: `replace match guard o.kind != "Node" with true in owner_of` —
[D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)'s
mirror-pod discard, unreachable because no fixture carries a Node
`ownerReference`.

**So the box stays, and stops being read as a proof of the decode.** A green
`cargo mutants` over `rules.rs` means the rules' *logic* survived mutation; it
says nothing about whether the fields those rules read were decoded from the
right place. That second question is answered by field-level mutation done by
hand, and the box now says so. This is
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) one level up: there,
a guard that had only ever been green; here, a gate that is green because it
never looked.

**A `scripts/` harness to make the hand sweep permanent was offered and
declined.** The process already carries it: step 4 of the cycle is `tester`'s on
every box, and this entry is what tells that step to go field by field on a
decode. A second mechanism enforcing what a named person is already accountable
for is machinery that has to be maintained and can itself rot green. If a later
box ever gets its decode changed without a sweep, the process failed and the
harness becomes worth building — that is the trigger, recorded so the decision
is revisited on evidence rather than on taste.

### D42 — the snapshot types freeze one phase after the file they live in (2026-08-12)

todo.md says **Frozen after: `rules.rs`** at the end of Phase 3, and its own
Phase 5 box says "*Phase 4* defined `ClusterSnapshot`". Both cannot be true,
and the second is the honest one about what Phase 4 needs: the Capacity report
wants `cpu_limit`, Waste wants `status.reason` for Evicted pileups, and
Drain-safety wants Services, PVCs, PDBs and EndpointSlices on the snapshot —
none of which any Phase 3 rule reads, so Phase 3 correctly refuses to invent
them (a field with no rule behind it is a guess, and `cargo mutants` cannot
object to a branch nothing exercises).

Moving `ClusterSnapshot` up to `analysis.rs` would invert the pyramid —
`rules.rs` is below it and takes the snapshot as its input. So the freeze is
made per-concern instead of per-file: **`rules.rs` freezes at Phase 3 close
except for the snapshot types and their decode, which freeze at Phase 4
close.** Phase 4 may add fields to them and nothing else in the file: not a
rule, not `Finding`, not `ObjectId`, not `analyze`. The forward-only rule is
intact — nobody reaches back into finished *logic*; a shared contract simply
has two consumers and gains its second one a phase later.

### D43 — N2 has no clock, and that makes a finding's age optional (2026-08-12)

> **Superseded in its premise on 2026-08-13 — read
> [D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
> first.** A cordon *does* leave a timestamp: the node lifecycle controller
> stamps `timeAdded` on the taint it mirrors from `spec.unschedulable`,
> regardless of effect, so **N2 may say "cordoned 2 hours ago" after all** —
> [D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)
> records that as a capability the rule may use, not one it must. The rest of
> this entry — the autoscaler cases, and N2 staying quiet during a scale-down —
> still holds, and so does the conclusion the title draws: an age stays
> **optional**, because the hand-applied taint carries no stamp.

N2 was written as **"unschedulable for 6 days"** and `screens/alerts.md` drew
the card with `6 days ago` in the age column. There is no source for that
number. `spec.unschedulable` is a bare boolean; no node condition transitions
on a cordon, `Ready` stays `True`; and the taint `kubectl cordon` adds is
`NoSchedule`, while `k8s-openapi 0.28.0`'s own `Taint::time_added` says
verbatim *"It is only written for NoExecute taints"* — read out of the crate
source, not inferred. (That sentence is **gone from the v1_34+ generated docs**
in the same crate, which say only "the time at which the taint was added". The
behaviour is unchanged; the citation stops being reproducible if the feature
pin moves up, so it is quoted here with its version.)

**That is a statement about `kubectl cordon`, and not about every cordon.**
The two controllers that cordon most nodes in the wild do stamp the time:
cluster-autoscaler's `ToBeDeletedByClusterAutoscaler` taint carries *the unix
second the scale-down began* as its **value** (`utils/taints/taints.go`:
`Value: fmt.Sprint(time.Now().Unix())`, added by the same call that cordons),
added by the same call that cordons). **Karpenter is *not* its equivalent, and
an earlier version of this entry said it was** — `karpenter.sh/disrupted` is
declared with a key and an effect and no `Value` field at all
(`kubernetes-sigs/karpenter/pkg/apis/v1/taints.go`), and it is `NoSchedule`, so
`Taint::time_added` is empty too. On a Karpenter cluster there is no clock
anywhere in the node object. The claim was corrected on the same day it was
written, by an operator review that went to the source instead of accepting the
symmetry.

Neither case needs a contract change — `NodeSnapshot.taints` already carries
key, value and effect — but both change N2: during a scale-down the node is
cordoned *with pods on it* for the whole eviction window, so N2 would fire,
repeatedly, on a cluster doing exactly what it was configured to do. **N2 stays
quiet when a scale-down taint is present.** A node an autoscaler is
deliberately removing is not a half-finished operation; it is an operation in
progress.

**But silence forever is a different claim from silence during the window, and
only the second one is defensible.** A scale-down that a PodDisruptionBudget
blocks indefinitely — node cordoned, tainted, workload still on it, the
autoscaler retrying — is a real loss of capacity, it is the most common way a
scale-down goes wrong, and under the rule above nothing on any screen shows it.
That case belongs to the **Drain safety report** (Phase 4: *for every node,
what a drain would do and what would block it*), which is where "a disruption
that is not finishing" is exactly the question being asked, and it keeps Alerts
free of a card about an operation that is merely slow. Recording it matters
more than which screen wins: before this, a stuck scale-down was silent and
nobody had written down that it was.

`metadata.managedFields` does carry the timestamp of whoever set the field, and
it is rejected: it is pruned at ingest by design, the sanitizer deletes it so
no fixture could ever test the path
([D30](#d30--the-guards-phase-2-added-and-the-freeze-they-collided-with-2026-08-12)),
and a field manager's name is not an API contract.

**So N2 reports no duration**, and the action line loses the inference the
duration was carrying — "someone's maintenance window never closed" is a claim
about elapsed time the tool cannot make. The consequence reaches two boxes
ahead: **a `Finding`'s timestamp is an `Option`**, because some findings have
no moment to point at, and the renderer needs an answer for that column rather
than a zero that draws as 1970.
[D4](#d4--the-flagship-example-promised-a-number-that-cannot-exist) is the
precedent and this is its third occurrence — a number that cannot exist is not
a rendering detail, and a mockup is where it hides longest.

**Losing the clock cost N2 the thing that made it a finding, so the rule is
narrowed rather than left noisy.** Without a duration, a node cordoned forty
seconds ago for routine maintenance draws exactly the card a node cordoned six
months ago draws — an alert raised on every routine maintenance window, and a false
positive is a bug here, not a tuning question. The discrimination the duration
was carrying is available from a field that exists: **N2 fires only on a
cordoned node that still has pods on it** — a drain someone started and did not
finish, which is both actionable and true at any age. **Corrected the same day
by [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12):
"still has pods on it" as written is true of every correctly drained node,
because a drain never evicts DaemonSet or static pods. The count is only of
pods a drain would actually move.** A cordoned node with
nothing on it is a *parked* node: correct, deliberate, and money burning
quietly, which is a **Capacity report row**, not an alert. That is
[D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s own line applied
honestly — broken now stays in Alerts, risky later goes to a report — and it is
what the card shows: `9 pods still run here` is the evidence, and the zero case
does not reach this screen at all.

The count survives the trap that killed `managedFields` as a source: the
sanitizer *refuses* a capture carrying foreign node identifiers rather than
rewriting them ([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)),
so both halves of the `spec.nodeName` join stay intact in a fixture and the
number is testable.

**Narrowing the rule moved the blindness from the evidence line to the trigger,
and that is a different bug.** The first version of this ruling said the count
is suppressed under namespace scope — fine when the count was decoration. Now
it decides whether the card exists at all: a user running `--namespace
payments`, or one whose cluster-wide pod LIST 403'd into the fallback, sees
zero pods on a node that has forty, and N2 silently does not fire. A wrong
number was replaced by a missing finding, which is worse because nothing on the
screen shows it happened.

**So a rule that needs every pod on a node is disabled under namespace scope
and says so, rather than computing a partial answer.** That is not a new
mechanism — it is the degradation
[docs/architecture § Error handling](docs/architecture.md#error-handling)
already specifies for a 403 on a secondary stream: the feature switches off and
names what is missing. **N2 and N5 both take it** — overcommit is the same
cluster-wide join, and a partial sum of requests understates every node it is
asked about. N6 is unaffected: it reads node taints and the Pending pod's own
spec, both of which are in scope by definition.

### D44 — five more mockups promising numbers nothing produces (2026-08-12)

Fixing N2's card came with a sweep of the other 30 mockups, because a defect
found by an implementer three phases early means nothing had been checking. The
sweep is the entry; the fixes belong to the boxes that build these screens, and
each is named here so it is inherited rather than rediscovered.

- **`screens/resources.md` — `jobs 7` in the sidebar.** Jobs are not in the
  permanent watch set ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
  puts the J-series in v0.2) and browser kinds are watched only while their view
  is open, so that count either buys a stream
  [invariant 6](CLAUDE.md) forbids or materialises out of nowhere after the
  first visit. The pod and workload counts above it are free; this one is not.
- **`screens/states.md` — `reading the cluster… 2,140 pods`.** Mid-LIST there is
  no total. Paginated, the API offers `remainingItemCount`, which its own
  documentation calls a hint.
- **`screens/analysis.md` — `API server certificate 210 days`.** C2, drawn as a
  row while kube-rs does not expose the peer certificate and the second
  connection it needs is undecided. A row with no data path is a promise made in
  a picture.
- **`screens/dialogs.md` — `replaced by web-2c81a 3 seconds ago`.** The
  timestamp is real (`creationTimestamp`); *"replaced by"* is an inference from
  a shared ownerReference and names the wrong pod whenever the ReplicaSet scaled
  for any other reason.
- **Rules 3, 4 and 8 have no event time.** `state.waiting` carries no timestamp
  and rule 8 is a fact about a spec, not an event. The general rule, binding on
  the box that puts timestamps on `Finding`: **a card's age is the time of the
  event it describes, or it is blank** — creation time is not a substitute,
  because a pod created nine days ago that started failing to pull four minutes
  ago is nine days old and four minutes wrong.

### D45 — the decode invented a container state the API says is `Waiting` (2026-08-12)

`ContainerState` was given a fourth variant, `Unknown`, for "the kubelet
reported a state with nothing set in it". k8s-openapi 0.28.0's generated doc,
straight from the OpenAPI definition, says otherwise: *"Only one of its members
may be specified. **If none of them is specified, the default one is
ContainerStateWaiting.**"* No fixture reaches the branch, so nothing was red —
the file's contract and the API's contract simply disagreed, quietly, in the
one file that freezes first.

**The API wins: the empty case decodes as `Waiting { reason: None, message:
None }` and `Unknown` is deleted.** Behaviour is unchanged — rules 1, 3 and 4
match on a *named* reason, so a `Waiting` with none fires nothing, exactly as
`Unknown` fired nothing — and one variant that nothing constructs goes away
with it. The alternative was keeping a state whose only justification was a
guess about the kubelet, in a type Phase 5 hands live watch data to.

Found by mutation testing, not by a rule: the survivor list was empty, so the
sweep went to the *source* to check the one claim it could not falsify — that
the three-way precedence is unobservable because upstream sets exactly one
member. The first sentence confirmed it and the second one broke something
else. A verification that only looks where it was pointed finds only what it
was pointed at.

### D46 — nine fields the contract dropped, and the drain that does not drain (2026-08-12)

The operator review of the snapshot types found eleven things. Three of them
were blockers, and the middle one killed a ruling written the same day. All of
them are the same defect class: **a field the API sends and the contract drops
at ingest** — invisible to a green suite, because a test can only assert on
what the struct kept, and unrecoverable afterwards, because these types freeze
one phase later ([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).

**Rule 7 would have fired on every deploy.** The rule is "Running and
`ready: false`", and so is every container between start and its first
successful readiness probe. Three fields answer *since when* and all three were
discarded: `ContainerStateRunning.started_at` (the `Running` variant was a unit
variant), `ContainerStatus.started` — **corrected below; it is far weaker than
this entry first claimed** — and
`conditions[Ready].lastTransitionTime`, the only place in the whole object that
records when the pod left the Service endpoints; a container status has no such
field anywhere. A Deployment with `initialDelaySeconds: 30`, a node reboot, a
scale-up: each would paint the screen whose entire promise is *only what is
broken*. The comment claiming the other four pod conditions "repeat what the
containers already say" was the assumption underneath it, and it was wrong.

**A pure function cannot know its own view is partial.**
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
disables N2 and N5 under namespace scope, and `todo.md` and
[docs/architecture § Error handling](docs/architecture.md#error-handling) both
say so — but `ClusterSnapshot` had no field distinguishing *"this cluster has
three pods"* from *"I can see three of this cluster's four thousand"*, and
[invariant 5](CLAUDE.md) forbids a rule asking anything outside the snapshot. A
small cluster and a namespace-scoped view of a large one decoded identically,
so the ruling was unimplementable by construction: `node-3` cordoned with forty
pods on it, none in `payments`, and N2 files nothing. `namespace_scope:
Option<String>` on `ClusterSnapshot` closes it — one field, both causes
(`--namespace` and the 403 fallback), and it gives the "not checked" message
its noun.

**`kubectl drain` never drains a node empty, so "still has pods on it" is true
of every node an operator drained correctly.** `kubectl/pkg/drain/filters.go`
returns skip for DaemonSet pods and for mirror pods *regardless of flags* — so
a properly drained kind worker still runs kindnet and kube-proxy, a drained EKS
node still runs `aws-node`, `kube-proxy` and `ebs-csi-node`, and a
control-plane node cordoned for an upgrade still runs four static pods no drain
can move. D43 narrowed N2 to kill exactly this false positive and the narrowing
did not narrow. **N2 counts only pods a drain would actually move**: not
`Succeeded`/`Failed` (`phase`, carried), not DaemonSet-owned (`owner.kind`,
carried), not mirror — and the third was the one the contract could not
express, because [D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)
discards the Node `ownerReference` that identifies a static pod. It is kept as
one bit, `PodSnapshot.mirror`, and the identity stays discarded: D39 is about
which card a finding files under, and that answer does not change. The bit
comes from the ownerReference and **not** from the `kubernetes.io/config.mirror`
annotation kubectl itself keys on — the sanitizer destroys annotations, so an
annotation-sourced bit would decode `false` in every fixture and could never be
tested ([D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
is the same lesson from the other side).

**Six more fields, each one a rule that would otherwise say less than
`kubectl describe`:**

- **A native sidecar is not an init container, in the arithmetic or in
  English.** `restartPolicy: Always` on an init container has been GA since
  1.29 and is how Istio, Linkerd and Vault agent run. The scheduler's effective
  request is `max( max over init prefix , sum(regular) + sum(restartable-init) )`
  — a sidecar is *additive*. One `init: bool` forces N5 to either overstate a
  2Gi migration container or drop 100m per pod on every meshed node, which on
  sixty pods is six CPUs invisible to the rule whose whole job is "the cluster
  is lying to itself about capacity". And "the init container `istio-proxy` is
  crashlooping" is not plain language, it is false — so rules 1–6 need the
  three-way distinction too, not just N5. `init: bool` becomes a three-way role
  with the invalid state unrepresentable.
- **`finalizers`.** Rule 12 promises "a finalizer *or* the kubelet is holding
  it" — a coin flip between two causes whose fixes are unrelated. The list is
  the answer, `describe` does not print it at all, and the positive fixture has
  carried `k8rs.test/never-removed` since Phase 2.
- **`subPath` on a hostPath mount, and the container that mounts it.**
  `hostPath: /var/run` + `subPath: docker.sock` recorded `/var/run`, and rule
  8's docker.sock escalator never saw it — a read-only bind-mounted socket is
  still full root on the node. Without the container name the finding cannot
  say *which* container mounts the node root, and two containers mounting one
  volume produced two entries the rule could not tell apart.
- **`ContainerStatus.image`.** Rule 3's action is "check the image name/tag or
  the pull secret", and the name reached the user only inside the runtime's own
  sentence — `image.json`'s message is containerd-worded and CRI-O phrases it
  differently.

**And `deletionTimestamp` is a deadline, not a moment.** The apiserver sets it
to *request time + grace period* (`registry/rest/delete.go`:
`metav1Now().Add(GracePeriodSeconds)`), which `stuck.json` has been proving all
along: `23:16:54` with a 5-second grace, so the delete landed at `23:16:49`. A
rule written to the comment that said otherwise would double its own threshold
— flagging a default pod at 60 seconds, not 30 — and report an age short by
exactly one grace period, forever. Both fields were already carried, so
`asked_at = deletionTimestamp − grace` is computable; what was wrong was
written down, in the file and in the rule-12 row here.

Two more went to Phase 4 rather than here, which D42 permits: `spec.overhead`
(RuntimeClass) and `status.allocatedResources` — the latter is what the node
actually reserved during an in-place pod resize, and it diverges from `spec` on
exactly the 1.33+ clusters this project targets. Both belong to the Capacity
report, and they are recorded on its box so they are inherited rather than
rediscovered.

**Smaller rulings the fix itself forced**, recorded because a decision nobody
wrote down is one that gets rediscovered with no memory of why:

- **`owner_of` returns the identity and the mirror bit together**, not a second
  `is_mirror(meta)` beside it. One traversal, so the bit and the discarded
  reference cannot come to disagree — the failure mode of every "derive it
  again over there".
- **Only a *controlling* Node reference makes a pod a mirror pod.** A
  non-controlling one is a garbage-collection link and means nothing about who
  writes the pod. It is one `controller == Some(true)` and it is load-bearing:
  a Node reference that does not control does not exempt the pod from N2's
  count.
- **A container the status names but the spec does not declare decodes as
  `Init`, never `Sidecar`.** The kubelet only reports declared containers, so
  the case does not arise; the point of choosing is that the safe answer keeps
  the finding's English right if it ever does.
- **The sidecar role is read off the init list only.** The tempting
  justification — "upstream allows `restartPolicy` nowhere else" — is already
  expiring: 1.34 began relaxing the field on regular containers. The real
  reason is that a regular container answers `Regular` either way, which is a
  statement about our own behaviour rather than a bet on upstream's. Same
  reason no test synthesizes that object: under [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
  a synthesis is licensed only while every field of it is a value the *pinned*
  API emits, and `v1_32` does not emit this one.
- **`PodSnapshot.ready` and `ContainerSnapshot.ready` keep the same name with
  different types**, which was raised as a collision worth renaming before the
  freeze. Kept: `PodSnapshot` already carries `scheduled: Option<Condition>`,
  so `ready` beside it is the consistent name and renaming one of a matched
  pair is the worse trade. The two never appear without their receiver.
- **`subPath` is stored raw and the join is the rule's.** `HostPathMount` holds
  the fact; rule 8 decides whether `/` joined with `var/run/docker.sock` is the
  escalated case. Storing the joined path would be storing a verdict, which is
  the same line the type already draws by holding `read_only` instead of
  "dangerous".
- **The order of `PodSnapshot.containers` is explicitly *not* a contract.** It
  is init-first today and nothing may rely on that: containers are read by
  name, and a screen that wants them grouped sorts by `role`. The tempting
  alternative — declaring init-first *as* the contract — would have been the
  same defect this box was sent back to fix, a stated requirement with no
  assertion behind it. `ContainerRole` deliberately derives no `Ord`, because
  which role sorts first is a display decision and this file does not make
  those. The claim was verified rather than asserted: reversing the chain
  breaks no assertion in the file, which is exactly why the rule has to be
  written down instead of discovered later by `views.rs`.
- **A container the status names but the spec does not declare stays untested,
  by ruling and now in writing.** Both container lists are immutable after
  create, so the kubelet cannot report on an undeclared container and
  [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
  does not license synthesizing one. The reason now lives in the code, so the
  absence of a test reads as a decision rather than an oversight — which is the
  whole difference between the two.
- **`restartPolicy: Always` on a *regular* container is KEP-5307 (alpha in
  1.34), and the ruling deliberately does not depend on that.** Whether a given
  1.36 server admits the field turns on a feature gate nobody here could
  verify, so the requirement is written to be independent of the answer:
  `Regular` is the role whatever the restart policy says and whoever admitted
  it. A synthesis that leaned on "a v1.36.1 server produces this" would have
  been an unverifiable version claim dressed as a fact — the failure mode
  [D45](#d45--the-decode-invented-a-container-state-the-api-says-is-waiting-2026-08-12)
  is about, arriving from the opposite direction.

The review that found all of this ran against a file whose 32 tests were green
and whose every mutation had been killed. That is the argument for the gate
being held by someone who runs clusters rather than by the suite: a test
asserts what the struct kept, and none of these was in the struct.

### D47 — Phase 3 is running ahead of an open Phase 2, and what that buys and owes (2026-08-12)

Phase 2 has four boxes left and they are all the same box: **the kind cluster
trip.** Stand the cluster up, apply `broken.yaml` and `healthy.yaml`, wait for
the states to settle, capture, tear it down. Phase 3 has been built past it on
an explicit instruction — *"continue Phase 3, we do the trip at close"* — and
that is allowed. It was also **already recorded** before this phase began —
[D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)
is that ruling, and the box in `todo.md` states in its own text which half has
run and which has not.

**What has changed since D33 is the size of what is open.** D33 deferred *one*
box, `just cluster-down`, waiting on Phase 4's four missing report fixtures.
Phase 3 has since added three more, and every one was discovered by building
the layer above: a broken pod that has an owner
([D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12)), a mirror pod
([D39](#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)),
and the list of shapes the first capture could not produce
([D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)),
which itself grew three items during this phase alone
([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
That growth is not drift, it is the pyramid working — a capture list written
before the layer above it exists is a guess, and each of these came from a rule
that could not honestly be tested without it. But four open boxes is a
different fact from one, so the accounting goes in the record rather than in
someone's head.

**Why deferring is the right call anyway.** The trip is one physical
operation — a cluster stood up, broken on purpose, waited on, captured, torn
down — and running it four times to close four boxes buys nothing over running
it once. It also needs a machine: docker on this host, and a login session that
postdates the `docker` group grant, which is a condition the agents cannot
create for themselves ([the boxes no agent can run](CLAUDE.md)). One trip, at
the point where the list of what it owes has stopped growing.

**What it costs, measured rather than asserted.** `rules.rs` carries 16
synthesis sites. Four are permanent — an API state no cluster can be asked to
emit ([D45](#d45--the-decode-invented-a-container-state-the-api-says-is-waiting-2026-08-12)),
a node whose `allocatable` differs from its `capacity` (a kubelet flag, not a
workload), a non-controlling `ownerReference`, a second Node reference. **The
other twelve are a loan**: a test standing on a hand-set field instead of a
capture, each naming the object that retires it.
[D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
licenses that, and it licenses it *as a loan* — the synthesis is bounded by
"every field of it is a value the API actually produces", which is a rule that
holds only as long as someone keeps checking. It has already failed once in
each direction: a synthesized node condition that contradicted its own message,
and a decode that invented a state the API defines as something else. Both were
caught by going to the source, not by the suite, and neither would have existed
if the object had come off a cluster.

**So the boundary is hard: Phase 3 does not close until Phase 2 does.** Not
"the trip happens around then" — twelve of Phase 3's tests are unfinished until
it runs, and [phase close, item 2](CLAUDE.md#phase-close--the-ritual-at-the-end-of-every-phase)
says a box checked for work that was written but never run is a lie in the one
file the plan is read from. The trip's checklist is not a memory: every
synthesis names its object, and Phase 2's own boxes list them by the file that
has to change.

### D48 — a check that is switched off is named on the screen that would have shown it (2026-08-12)

[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
turned N2 and N5 off under namespace scope and said they should "say so". It
did not say **where**, and `screens/alerts.md` had kept the superseded half of
the ruling — a card with its evidence line dropped, which is a card asserting a
node is broken on evidence it does not have. The screen and the decision record
disagreed, and `screens/` is what the code has to match, so Phase 9 would have
built the wrong one.

**The line goes in the banner above the list, on the screen that would have
shown the findings** — Alerts names N2's absence, the Capacity report names
N5's. Rejected: the header (it carries the cause, `ns: payments`, and has no
room for a sentence), the help screen (keys only), a greyed-out card (there is
no card), and one global notice (which grows into a list as more checks switch
off, on every screen, including the ones unaffected). The banner is the widget
the disconnected state already uses, so this is placement, not a new mechanism.

Three rulings follow from drawing it, and the second is the one that was
actually wrong before:

- **Node listability is a separate permission from namespace scope.**
  `widgets.md` justified a blank header vital with "a namespace-scoped user
  cannot list nodes", which is false — and D43's entire premise is a user who
  *can* list nodes but cannot list every pod. Decoupled: it now drives the
  header vitals, the `capacity` badge and which node rules run, independently.
- **The `of 5` denominator is a second permission too.** It comes from the
  workload watch, not the pod watch, so a user allowed `pods` but not
  `deployments` has the numerator alone. The card reads `3 pods`, grouping
  falls back to the pod's own name rather than the hashed ReplicaSet, and
  W1/W2 are named as off — [D3](#d3--findings-group-by-owner-not-by-pod)'s *no
  owner* fallback extended to *owner unreadable*.
- **Three more checks join N2 and N5**: drain safety, the pending-CSR row and
  Versions. D43 named two because two were in front of it. The line that
  generalises them: **a sum breaks silently under a partial view, a list just
  gets shorter** — so Waste, whose rows are per-object facts, runs unchanged,
  and greying out the whole Analysis screen would have hidden three true
  answers. Drain safety is the sharpest of them: *"node-1 ok, 18 pods move"*
  computed from a partial view is a green light for an operation that then
  hangs.

**N2's evidence line changed with its trigger**, which is the part that would
have rotted quietly. `9 pods still run here` described an inventory — the
reader checks it with `kubectl get pods -o wide` and finds a different number,
because [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)
stopped counting DaemonSet, static and finished pods. **`2 pods here would
still have to move`** describes the computation `kubectl drain` itself performs,
so the card and the command it teaches agree. A two-number form (`2 to move · 7
stay`) was rejected: the second number needs its own fixture and invites exactly
the arithmetic-checking the first wording lost to.

**One thing found in passing and ruled here rather than deferred:**
`screens/dialogs.md`'s drain dialog prints *"PodDisruptionBudget won't allow
fewer than 3 copies"* — raw jargon, while `analysis.md` explains the identical
fact in plain English. It was raised as "a v0.2 screen, your call".
[Invariant 14](CLAUDE.md) has no version exemption: a string a user can read is
written for someone who does not know the term, whichever release it ships in.
The rewrite belongs to the box that builds that screen, and this is the record
that it is owed rather than optional.

### D49 — the link checker skipped every link that wrapped, and had been green all along (2026-08-12)

`scripts/check-docs.py` scans **line by line**, so a Markdown link whose
`[label]` wraps across a newline matches no regex and is skipped **entirely** —
not just its anchor, its target file too. Three such links were in the repo.
Reproduced here before believing it: a deliberately broken wrapped link
(`NOTES.md#no-such-anchor-at-all`, label split over two lines) appended to
`docs/tech-stack.md` produced *"checked 24 markdown files / OK — all relative
links resolve"*; the identical link on one line produced *"FAIL … 1 broken
link(s)"*. The hole is the wrap, exactly.

This is [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
and [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
a third time, and the recurrence is the point. Both of those were about the
*sanitizer* — which objects reach the check, and where inside a value a secret
can sit. This is the same defect in a different guard: **the shapes a guard was
fed were all the shapes its author happened to write**, and this project wraps
its prose at 79 columns, so the untested shape is the one the house style
produces most. Every guard in `scripts/` is now suspect in the same way, not
because any specific hole is known, but because none of them was fed input in
the shape the *repo* generates rather than the shape the test wrote.

The fix is one line — read the file whole with `re.S` instead of iterating
lines — and it is `tester`'s, along with the wrapped-link case as a permanent
`--self-test`. Found by `tui-designer` while checking its own work, which is
the only reason it was found at all: nothing in `just check` could report it,
because the guard reporting green *was* the bug.

### D50 — the rule tests live in `rules.rs`, and no lib target is added to change that (2026-08-12)

`cargo metadata` reports exactly one target — `k8rs`, kind `bin`, root
`main.rs` — so a file under `tests/` cannot `use k8rs::rules::PodSnapshot`.
Every rule test therefore lives inside `src/rules.rs`, which is `dev-core`'s
file, and `tests/` holds only `fixtures/`. Raised because it makes `tester`'s
ownership row unusable for rule coverage across the whole of Phase 3, and the
remaining boxes would each hit the same wall.

**No lib target is added.** The fix would be a `src/lib.rs` carrying the `mod`
lines with `main.rs` consuming it — a ninth file, and
[invariant 11](CLAUDE.md) wants the same kind of boundary argument the eighth
had. "So that integration tests can reach the code" is not that argument when
unit tests already reach it: `#[cfg(test)] mod tests` in the same file is
Rust's normal idiom, 38 of them are already there, and they run under
`cargo test` exactly like anything in `tests/` would. Adding a file of pure
plumbing to relocate tests that already work is the trade this project keeps
refusing.

**What actually needed saying is who does what, and the model already had it
right.** [The cycle](CLAUDE.md#the-cycle--one-todomd-box-is-one-turn-of-it)
step 3 is "write the code **and its tests together**" and names the dev; step 4
is `tester` witnessing red then green. So on rules, `tester`'s job was never to
author the tests — it is to *attack* them, and on this box it reproduced the
author's fourteen mutations with its own driver and then found two the author
had missed: a `started: None` branch no capture exercises, and a regular
container with `restartPolicy: Always` decoding as `Sidecar` against the file's
own stated requirement. Both went back to the author. That is the gate working
as designed, and it is worth more than a directory boundary would have been.

`tester`'s `tests/` row keeps its meaning for what will actually live there:
the fixtures, and the Phase 7 end-to-end tests that drive the binary rather
than call into it.

### D51 — the third review of the same contract, and the sentence that would have rebuilt the bug it closed (2026-08-12)

The snapshot types went through three operator reviews. The first found nine
missing fields ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)),
`tester` then found two holes the author's own mutation sweep had missed, and
the third — reading the file *after* all of it was fixed and independently
attacked — found a blocker and five more. That progression is the argument for
the gate, so it is worth stating what the blocker actually was: **not a field,
a sentence.**

**`ContainerStatus.started` does not mean what D46 said it meant.** This record
called it "the startup-probe bit, which is precisely 'still booting' versus
'was serving and stopped'". The crate's own generated doc finishes the
thought — *"**Is always true when no startupProbe is defined** and container is
running and has passed the postStart lifecycle hook"* — and **no container in
any committed fixture declares a `startupProbe`**. For the overwhelming
majority of real workloads the field flips true the instant the container runs
and discriminates nothing at all. A rule 7 written from that prose —
`Running && !ready && started` — fires CRITICAL on every pod of every rolling
update, every node reboot and every scale-up: precisely the blocker this
contract was sent back to fix, rebuilt out of the sentence written to close it.
`healthy.json` would not have caught it either, because there is no mid-rollout
pod in the capture.

The field stays; it does discriminate where a `startupProbe` *is* declared. But
the "since when" rule 7 rests on is `ready.last_transition` and nothing else,
and that is now said where a rule author will read it. The lesson is narrower
and worse than "check upstream": the *first half* of that doc sentence was
quoted correctly, and the half that mattered was two clauses later. A quotation
that stops where the point was already proven is not a verification.

**Four more fields, each a rule that would otherwise be confidently wrong:**

- **Pod-level requests** (`PodSpec.resources`, KEP-2837). *Two version facts
  that look like a contradiction and are not:* the pinned `v1_32` generated doc
  says *"This is an alpha field and requires enabling the PodLevelResources
  feature gate"*, while the field went beta and default-on in **1.34** and the
  fixtures are **v1.36.1**. Both are true at their own version, and the field
  is present in the pinned crate either way, which is all a decode needs — it
  is written here so the next reader who opens `v1_32/api/core/v1/pod_spec.rs`
  does not conclude the field was carried on a guess. A pod declaring
  `spec.resources.requests: {cpu: "4"}` with no
  per-container requests decodes as all-`None`, so N5 sums **zero** and calls
  the node healthy while four committed CPUs sit invisible. This is
  `namespace_scope`'s shape a second time — a pure function looking at zeros
  with no way to tell "requests nothing" from "requests something I did not
  look at". When the pod-level value is set it *replaces* the container sum.
- **The enacted memory limit, not the requested one.** `ContainerStatus.resources`
  is *"the limits that have been successfully enacted on the running
  container"*, and it is already populated in every committed capture. Rule 2
  reads `spec`. Patch a crashing pod 128Mi → 512Mi, have the in-place resize
  sit `Deferred` because the node cannot fit it, and the evidence line reads
  *"exceeded its 512Mi limit · exit 137"* about a container never given 512Mi —
  an operator sent hunting a leak in an application that never had the memory.
  Status first, spec as fallback.
- **`ObjectKind` must read the API group, not the kind string alone.**
  OpenKruise is deliberately drop-in: its Advanced StatefulSet is
  `apps.kruise.io/v1beta1, Kind: StatefulSet`. `owner_of` holds the whole
  `OwnerReference` and was passing only `kind`, so those pods decoded as the
  core variant. The card lying is the small half; the large half is Phase 7
  aiming `scale` at `apps/v1 statefulsets/<name>` — a 404, or a *different*
  object that happens to share the name. A write path pointed at the wrong
  object is not a display bug. Argo Rollouts was safe only by the accident of a
  unique kind string.
- **`Terminated` carries `started_at` and `message`.** "Restarted 5 times" is
  the same sentence for two unrelated incidents. *"Runs for about 2 seconds,
  then exits 1"* versus *"ran for 40 minutes, then exited 1"* is the first fork
  of every crashloop triage — bad config on one side, a leak or a downstream
  timeout on the other — and `describe` makes you subtract the timestamps
  yourself at 3am. Every capture has both. `message` is the same argument: with
  `terminationMessagePolicy: FallbackToLogsOnError` the kubelet writes the log
  tail there, which turns rule 6's action from "check the logs" into the log
  line. `Waiting` already carried `message`; `Terminated` not carrying it was
  an asymmetry with no reason behind it.

**And C1's input goes into `ClusterSnapshot`, settled now rather than at its
own box.** `todo.md` had described C1 as "PEM bytes in, finding out" — a second
entry point that [invariant 5](CLAUDE.md)'s stated signature does not describe,
which makes it an invariant amendment, which is a stop rather than a
convenience. So the snapshot carries the kubeconfig **context name** and the
client **certificate**; it is already documented as assembled by `k8s.rs` and
never decoded from one API object, so this is what it is for. The certificate
only — never the private key, never the token, never anything else off the
kubeconfig, and the field says so, because the next reader will rightly wonder
why a kubeconfig is anywhere near this struct. Deciding it here costs two
fields; deciding it at the C1 box would have meant deciding it with the file
one box from frozen.

**Rulings the fix forced, and one it uncovered.** The author's own second pass
found a **fourth** site of the blocker that this review had not named:
`ContainerState::Running`'s doc claimed `started_at` was *"the only 'since
when' a running container has"* and that rule 7 needed it — the same wrong
claim one field over, and it flatly contradicted `PodSnapshot::ready`'s doc on
the same page. A rule 7 written from *that* sentence rebuilds the false
positive by a second route. `started_at` is the current run's uptime, which is
rules 1/5/6 evidence — *"it came back up forty seconds later"* — and nothing
else. Three sentences were fixed by instruction and the fourth by someone
reading the file as a stranger would; that ratio is the argument for the second
pass being mandatory rather than conditional.

- **The group identifies a type; the version is serialisation.** `apps/v1beta1
  StatefulSet` still decodes `StatefulSet`, and `Other`'s format is kubectl's
  own notation, `Kind.group` — a core-group unknown kind stays bare
  (`Other("ReplicationController")`, no trailing dot), and `Other("kubeconfig")`
  for C1 is untouched.
- **The mirror-pod discard moved onto the *resolved* kind**, not the raw
  string, so a CRD named `Node` in someone else's group is an ordinary owner
  rather than a static pod. That is the same bug as the OpenKruise one
  approached from the other side, and it was not in the brief — it has a test.
- **`effective()` falls back per key, not per side.** A server too old to
  populate `status.resources` is inside the support window, so a key missing
  there falls through to `spec` rather than the whole side reading as "nothing
  enacted". The cost is stated where the helper lives: a resize that *adds* a
  limit names the not-yet-enacted value until it lands, which degrades to
  exactly today's behaviour — the safe direction.
- **`status.allocatedResources` is deliberately not read.** It repeats
  `status.resources.requests` and would add a third precedence step for no new
  fact. This narrows what D46 sent to Phase 4: the *requests* half is covered
  by `effective()` already, and what Capacity may still want is `spec.overhead`.
- **`context` is `Option<String>`** — a kubeconfig can name no current context,
  and `None` there means C1 has no name to file under, which is a real state
  rather than a defensive one.

### D52 — the guards were fed the shapes their authors wrote, not the shapes the repo produces (2026-08-12)

Fourth occurrence of one defect. [D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)
was the sanitizer seeing a single Pod and not the `List` half the captures
arrive in. [D31](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
was the same guard matching whole values while secrets sit inside sentences and
inside base64. [D49](#d49--the-link-checker-skipped-every-link-that-wrapped-and-had-been-green-all-along-2026-08-12)
was the link checker scanning line by line in a repo that wraps at 79 columns.
And now `fixture-audit.sh`, which owns the correct base64 predicate and applies
it to `*.json` only, skipping `certs/*.crt.pem` outright — so a real
certificate with a private key base64-wrapped and appended printed **"no key
material"** and exited 0, in both that guard and `certs-test.sh`. The green log
was byte-identical to a clean run.

**The directory it exempted is the only one where key material is actually
generated**, and it is about to matter more: `ClusterSnapshot` now carries a
client certificate for rule C1 ([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)),
a kubeconfig fixture is on the roadmap for that rule, and a kubeconfig carries
`client-key-data` **base64-encoded** — the exact framing the check skipped.

So the rule stops being "test the guard" and becomes specific enough to act on:
**feed every guard the shapes the repo produces, not the shapes its test
writes.** The two are systematically different, and the difference is invisible
because a guard that misses a case reports success in the same words as a guard
that found nothing wrong. Every guard in `scripts/` is being swept on that
basis — not because a further hole is known, but because none of them was ever
fed repo-shaped input, and four for four is no longer a coincidence.

Each fix carries the shape into its own `--self-test`, or it regresses the
first time someone refactors it. That is what makes this different from
patching four bugs.

**The sweep found holes in five guards, not the one it was sent for.** Two were
known going in; three were not, and one of them is a hard invariant:

- **`write-guard.py` exempted any file *named* `ops.rs`, not `src/ops.rs`.**
  Fed six files each containing `api.delete("web")`, it reported two and
  silently exempted `tests/ops.rs`, `examples/ops.rs`, `benches/ops.rs` — and
  `build.rs`, which is **compiled and executed at build time** and sat under
  none of the four scanned roots at all. `tests/ops.rs` is the obvious name for
  Phase 7's write-path integration test: the one file that must not be able to
  opt out of the invariant it exists to test. [Invariant 1](CLAUDE.md) is
  enforced "mechanically and stays that way", and for the whole of Phases 1–3
  the mechanism had a name-shaped hole in it. Now exempt by resolved path, with
  `build.rs` scanned.
- **`test-guard.py` would have turned `just check` red the first time anyone
  ran `just mutants`.** `cargo mutants` writes a full copy of the source tree
  to `mutants.out/`, which the blocklist did not cover, so the guard counted
  every test twice: *"90 declared, 45 listed — 45 never run"*. A fabricated red
  build, and precisely the kind that gets repaired by weakening the guard
  rather than by fixing it — in the phase where `just mutants` starts being
  used. The blocklist became the same roots allowlist `write-guard.py` uses,
  because a blocklist needs a new entry per tool and an allowlist excludes
  every stray copy by construction.
- **`screens-check.py` printed green over a 201-column frame** in four shapes,
  including a mockup inside a `~~~` fence — which `check-docs.py` had already
  decided *is* a fence, so two guards in the same repo disagreed about what a
  code block is. A fence nobody closed silently dropped the block; a mockup one
  directory down was never looked at (`glob`, not `rglob`); and an empty
  `screens/` reported *"0 mockups fit 80x24 — OK"*, which is D26's own lesson
  about a count of zero reading as success.

**One gap is left open on purpose, because closing it is a policy choice rather
than a bug fix.** `sanitize.jq` does not cover a CertificateSigningRequest's
`.spec.username` and `.spec.groups`. The committed `csr-pending.json` carries
`kubernetes-admin` and `kubeadm:cluster-admins`, which are kind defaults — so
nothing has leaked, and that is luck rather than the guard. A CSR captured from
a real cluster carries an OIDC email or `system:serviceaccount:prod/deployer`
there.

**It takes the node-name treatment: refused, not rewritten.** A requester
identity is a *reference*, not a payload — the same category as a node name,
and the sanitizer already refuses a capture whose node names did not come from
the kind cluster rather than quietly mangling them
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)).
Extending that one refusal to one more field needs no new mechanism and no new
policy shape, which is exactly why it is the right answer: an allowlist of
acceptable usernames would be a judgement call re-made on every capture. **This
lands before the next capture trip, not after** — it is a Phase 2 box, and
Phase 2 owns fixture capture.

Decisions the sweep forced, worth keeping: `openssl` is the key oracle rather
than a regex (already required by two scripts, so not a new dependency), and it
is called with `-passin pass:` — without it an encrypted key prompts on the
terminal and **hangs `just check`**, which is a guard that fails by hanging
rather than by failing. Thresholds are 100 base64 characters and 64 PEM-body
characters: an EC P-256 key is ~184 base64 characters, so the *smallest* key
anything real produces still clears both, and the self-test generates exactly
that key rather than a comfortably large RSA one.

### D53 — a committed capture is never edited to make a test pass (2026-08-12)

`effective()`'s per-key fallback needed a shape no capture has: a `requests`
map the kubelet enacted *without* `cpu` in it, so that `spec` is the only
source for that one key. The proposal was to add `cpu: 250m` to `oom.json`'s
spec requests.

**Right shape, wrong method, and the distinction is the whole of why fixtures
are trusted here.** `tests/fixtures/*.json` are real captures — that is the
property [the honest-test rules](CLAUDE.md#code-phase-rules-apply-once-it-starts)
buy by refusing hand-written JSON, and it is a property of the *file*, not of
any one test. Editing a committed capture so a test goes green spends that
property for every other fixture in the directory at once, and it spends it
invisibly: nothing in the file says which bytes came off a cluster and which
were typed. [D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12)
already licenses the honest version — deserialize the capture, set the one
field in memory, assert, and name the object the capture trip owes. The synthesis
is visible in the test that does it; an edited fixture is visible nowhere.

**A surviving mutant that was written up honestly rather than closed.**
`effective()` reading `status.resources` *only* for keys `spec` also declares is
indistinguishable from the real code unless the kubelet enacts a key the spec
never asked for. All 23 fixtures were scanned — zero such shape — and no way to
make a cluster emit one was found, since an enacted value is enacted *from* a
declared one. The first draft of that comment stated it as fact ("both lists
are defaulted at admission, so an enacted key has a declared key behind it");
the author's own second pass caught that as [D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)'s
exact failure — a sentence claimed harder than its evidence — reappearing
inside the comment written to close D51. It now reads **"a shape nobody could
produce, not one ruled out"**, and names the surviving mutant out loud. That is
the right register: the same standard already applied to the unmatched container
status and to `From<StatefulSet>`, and it costs nothing to say which of the two
things you know.

**And the caution paid off within the hour — the shape is reachable.** The
operator review found it in upstream: with pod-level resources, a container that
declares *some* limit but no memory limit gets the **pod's** memory limit
written into its own status (`kuberuntime_container_linux.go`'s `getMemoryLimit`:
*"When container-level memory limit is not set, the pod-level limit is used"*),
and `convertContainerStatusResources`'s memory branch has no key-existence
guard, unlike the CPU branches beside it. So `status.resources.limits` carries
`memory` while `spec.containers[].resources.limits` does not — exactly the key
the mutant would drop, and the value the container is actually killed at. One
manifest produces it, and it is now on the capture trip.

Had the first draft's *"both lists are defaulted at admission"* been left in,
this record would today be asserting a falsehood with upstream code contradicting
it. The honest framing was not merely more modest — it was the one that stayed
true.

**`effective()` keeps its per-key fallback, and the argument against it lost on
the source rather than on preference.** The review's case for per-object rested
on a container's status resources being a copy of its *spec*; upstream copies
the **allocated** map (`convertContainerStatusResources` takes
`allocatedContainer.Resources.DeepCopy()`), and allocated and spec diverge in
key set exactly while a key-adding resize is pending, which validation permits.
Upstream's own arithmetic is a per-key union — `maxResourceList` takes a key
present on *either* side — and the scheduler reads it with
`UseStatusResources: true`, so per-object would charge N5 nothing for a request
the scheduler is actively counting. `PodRequests` even carries the comment
*"the computation is part of the API and must be reviewed as an API change"*.

The real split is not requests-versus-limits, it is normal-versus-**Infeasible**:
when a resize is rejected as infeasible, upstream drops the spec entirely. That
one case k8rs reads wrongly, **knowingly** — reading it right needs
`PodResizePending` on `PodSnapshot`, no v1 rule reads that condition, and the
file freezes before Phase 4, so adding it would be a plan change rather than a
code one. It is written down here instead of being discovered as a bug.

**One more thing the fix turned up, and it is the kind that only reading the Go
types finds: `desired` and `ready` are both `Option<i32>` and their `None`s mean
opposite things.** `ReadyReplicas` is a plain `int32` with `omitempty`, so it is
absent **exactly when it is zero** — `ready.unwrap_or(0)` is correct, and a W2
written as `if let (Some(d), Some(r))` would go silent on precisely the total
outage it exists to report. `Replicas` is a `*int32`, so `replicas: 0`
serialises fine and `None` means *absent*, which the API then defaults to
**one** — so `desired.unwrap_or(0)` would say "wants nothing" where the API says
"wants one". Two identical-looking `Option`s, two opposite readings, and the
field docs had been transposed on top of that. DaemonSet's counters carry no
`omitempty` at all and are always `Some`.

Two smaller things settled in the same pass, both about what the contract can
still express:

- **N2 does not need to see `emptyDir`.** `kubectl drain` also refuses on
  local-storage pods without `--delete-emptydir-data`, and the snapshot carries
  only `hostPath` volumes — so N2 cannot tell that a drain would *block* on
  such a pod. It does not need to: N2's sentence is *"N pods here would still
  have to move"*, and a pod with an `emptyDir` does still have to move. What
  would be wrong is the sentence *"N pods a drain would move"*, which promises
  the drain succeeds. Blockers are the **Drain safety** report's question, and
  it is the second place today that a half-finished disruption has been sent
  ([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).
- **`ClusterSnapshot` still has no `now`**, correctly — it is the next box. But
  `ObjectKind` and `ContainerRole` freeze at Phase 3 close while the snapshot
  types get until Phase 4 close ([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)),
  and `now` is the one field [invariant 5](CLAUDE.md) names by itself. It does
  not get to drift into that extra phase's slack.

### D54 — `now` is `meta::v1::Time`, not a bare `jiff::Timestamp` (2026-08-12)

[D18](#d18--the-clock-is-an-input-not-an-ambient-fact) settled which crate the
time type comes from and wrote the answer as `jiff::Timestamp`. The field that
landed is `meta::v1::Time`, which *is* that type — `pub struct Time(pub
jiff::Timestamp)` — wearing the same newtype every decoded API timestamp in
`rules.rs` already wears.

The reason is the comparison, which is the only thing this field is for: rule 12
compares `now` against a `deletion_timestamp: Option<Time>`, C1 against a
certificate date, the renderer against a finding's timestamp. One type on both
sides is `<=`; two types is `.0` at every site, and `.0` at every site is one
forgotten `.0` away from comparing a laptop instant against something that only
looks like one. The crate coupling D18 recorded is unchanged — `Time` is
k8s-openapi's, and k8s-openapi's jiff is what moves.

It buys nothing for the *arithmetic*, and that half is written down where the
next box will read it rather than here: `.0` is needed on both sides of every
subtraction, `a - b` yields a seconds-only `Span` whose `.get_minutes()` is `0`
for a 43-minute gap, and the call that behaves is `Timestamp::duration_since`.

**Not an `Option`.** A snapshot always has a moment. An `Option` would push a
"what if there is no time" branch into every rule that reads one, and the only
answer available in that branch is the value the caller already had.

### D55 — the clock was written backwards, and the clamp protects the harmless half (2026-08-12)

D18 said a laptop running **fast** produces a negative age. It does not, and the
error had been copied into `rules.rs` before the operator review caught it.

Age is `now − event`:

| laptop | pod deleted 11:59:50 | what the screen does |
|---|---|---|
| 10 min **fast** | age **+10m05s** | rule 12 **fires**: "asked to shut down 10 minutes ago and still hasn't" |
| 10 min **slow** | age **−9m55s** | rule 12 silent; the renderer says "just now" |

So the "just now" clamp guards the *slow* case — the one that under-reports and
harms nobody — and does nothing at all about the *fast* case, which is the one
that manufactures findings on a healthy cluster. Three consequences, all
binding on later boxes:

- **Rule 12's trigger gets a margin.** The recorded threshold was
  `deletionTimestamp` in the past, full stop. With a laptop ten minutes fast —
  an NTP-less machine, a VM resumed from suspend, a WSL2 host after sleep —
  every pod asked to terminate in the last ten minutes is "overdue", and a
  correctly-progressing 50-replica rollout fills the one screen whose promise
  is *only what is broken*. Even on a perfect clock a pod is briefly overdue
  between its deadline and the kubelet's SIGKILL landing. Rule 12 fires on
  `now − deletionTimestamp > max(30s, grace)`. Nothing is lost: a pod held by a
  finalizer is stuck for minutes or forever.

  > **The formula was wrong and is `> 60s`, flat, since 2026-08-13.**
  > `deletionTimestamp` **already is** request + grace, so `max(30s, grace)`
  > charges the grace twice: a pod with `terminationGracePeriodSeconds: 3600`
  > stayed invisible a full hour past its kill deadline — two hours after the
  > operator typed the delete — and a Kafka or Vault pod stuck exactly that way
  > is what blocks the rollout rule 12 exists to surface. The sentence above it
  > mis-sized the margin too: the SIGTERM→SIGKILL window is *before* the
  > deadline, not after, and what actually needs covering after it is kubelet
  > observation plus watch latency plus ordinary skew — a constant, not
  > something proportional to a number the deadline already spent. Reading
  > `grace` for the *age* (`deadline − grace`, `checked_sub`) was always right
  > and is untouched
  > ([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).
- **The slow half is detectable, so it is said out loud.** Any snapshot
  timestamp more than a few seconds after `now` means the laptop is behind the
  cluster. That is data k8rs already holds, and it belongs in the header in
  plain language — *"your computer's clock is 11 minutes behind the cluster —
  the times on this screen are wrong"* — not only in a test assertion.
- **The fast half is not detectable from object timestamps**, and pretending
  "just now" covers it is how the wrong belief survived this long. The honest
  source is the API server's `Date` response header, which is a Phase 5
  `k8s.rs` question and is recorded as one.

D18's sentence is corrected in place rather than left standing with a
correction pinned to it, because the next box to read that paragraph is the
renderer that implements the clamp.

### D56 — C1 cannot represent "never expires", and a rule may not return a `Result` (2026-08-12)

RFC 5280 §4.1.2.5 gives certificates with no well-defined expiry the literal
`99991231235959Z`. jiff's `Timestamp` stops before it — the range ends at Unix
second `253402207200` (`9999-12-30T22:00:00Z`) — so `Timestamp::from_second`
returns `Err` for exactly that value, and [invariant 5](CLAUDE.md) forbids C1
from propagating it.

The mapping that is both correct and permitted is **no finding**: a certificate
that does not expire has no expiry to warn about. Recorded before C1 is written
because the shape that gets typed by reflex is `.unwrap()`, and the input is a
kubeconfig — a corporate PKI is precisely where a non-expiring CA turns up, and
a panic at startup is what the user would get.

The same range is why the grace-period subtraction is `checked_sub`: v1.36.1
accepted `terminationGracePeriodSeconds: 9223372036854775807` in a server-side
dry-run against the live kind cluster, and `deletionTimestamp − that` overflows.
Anyone with `create` and `delete` on pods could otherwise kill the TUI through
a pure function that cannot fail.

### D57 — the pinned `now` is part of the fixture contract, and it makes "recent" unrepresentable (2026-08-12)

> **The value below is superseded: the pin is `2026-08-13T00:00:00Z` since
> 2026-08-13.** The mechanism this entry describes is unchanged — read it, then
> the update at the end for what moved and what deliberately did not.

The tests pin `now` at `2026-08-12T00:00:00Z`. The value was not chosen freely:
`scripts/certs-test.sh` already asserted the committed certificates against that
instant (24 days / 365 days / −3 days), so any other literal would compute C1's
arithmetic from two moments only one of which the build checks. `certs-test.sh`
now extracts the Rust pin and refuses to disagree with it — the one edge of that
coupling nothing had been guarding.

**It moves with the capture, in four places.** A re-run of `just fixtures`
stamps every object after the pin and the guard goes red — correctly, but for a
reason that is not a wrong clock. The instant lives in `src/rules.rs`'s
`fn now()`, `scripts/certs-test.sh`'s `now` and its `pinned[]` rows, and
`scripts/make-certs.sh`'s dates; they move together or the fixtures and the
certificates stop describing the same afternoon. That obligation belongs to the
capture box in Phase 2, which is where the trip is, and it survives
`rules.rs` freezing at Phase 3 close: a re-capture may touch `fn now()` and
nothing else in that file — the pin is fixture data that happens to be spelled
in Rust, not code.

> **The pin moved on 2026-08-13 to `2026-08-13T00:00:00Z`**, with the second
> capture trip, and this entry's `2026-08-12` is the superseded value — the
> mechanism above is unchanged, only the instant. All four places moved
> together. The certificates were **not** regenerated: their `notBefore` and
> `notAfter` bytes are untouched, so the day counts shifted instead — 23 / 364
> / −4 where this entry says 24 / 365 / −3. Each fixture still exercises the
> case it exists for (inside C1's 30-day window, far outside it, already
> expired), and regenerating would have written fresh key material into the
> repo to buy nothing. One relationship changed quietly with it and is worth
> knowing: `now` used to equal the certificates' `notBefore` exactly, and now
> sits one day after it. Nothing asserts that equality in either direction, so
> no guard would have caught it had it mattered
> ([D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)).

**What the pin costs, which is the part nobody had written down.** It sits 43
minutes after the newest captured timestamp, so **nothing in the fixture set can
be "recent"**. Every below-threshold case — rule 7's rolling-update pod, N1's
node that went NotReady thirty seconds ago, rule 12's pod inside its grace
period — is unreachable by capture and has to be synthesised in memory under
[D40](#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12).
That is a real cost, accepted for the certificates' sake, and naming it is what
stops the next author from reading the fixture set as "no such case exists".

### D58 — a Phase 2 box was passed over, and the order it comes back in (2026-08-12)

Phase 2 has five open boxes. Four are the kind cluster trip and their deferral
is recorded ([D47](#d47--phase-3-is-running-ahead-of-an-open-phase-2-and-what-that-buys-and-owes-2026-08-12),
[D33](#d33--phase-3-opens-with-one-phase-2-box-still-open-on-purpose-2026-08-12)).
The fifth — `sanitize.jq` refusing a CSR's `.spec.username` and `.spec.groups`
([D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12))
— needs no cluster, no hardware and nobody's permission, and was passed over
anyway. Its own text says it lands *before* the trip; an audit of both phases on
2026-08-12 found it still open with `csr-pending.json` still carrying
`kubernetes-admin` and `kubeadm:cluster-admins` through the sanitizer unchanged.

Nothing excuses it: the cycle's rule is the first unchecked box in the lowest
open phase, and the four trip boxes are the *only* ones carrying an exemption.
It is recorded because the same audit found no other silent skip — every other
open box in Phases 2 and 3 is genuinely unstarted, and a phase that has been
audited once should say so, or the next audit starts from zero.

**The order it returns in**, decided when the kind cluster came back up on the
development machine: the sanitizer refusal lands **first**, then the manifests
for the shapes the first capture could not produce, then the capture, then the
teardown that closes the trip. A sanitizer fix landing after a capture is a
sanitizer that never ran on the bytes it was written for.

### D59 — the sanitizer refuses a requester, and an exit-status guard cannot see a deletion (2026-08-12)

[D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)
left one gap open on purpose: a CertificateSigningRequest's `.spec.username` and
`.spec.groups`. Closing it produced four rulings and one lesson that is bigger
than the box.

**The allowed set is derived, not curated.** "Refused, not rewritten" was
already settled; what counted as *foreign* was not. The answer is the node-name
shape — refuse anything the pinned kind cluster does not itself issue — and the
set was **read off the live cluster** rather than recalled: the two kubelet
identities with their groups, and kubeadm's admin from the kubeconfig client
certificate's subject. It is a fact about kind, not a policy about which real
identities are tolerable, which is the allowlist D52 rejected. The regex is
anchored at both ends, unlike the `k8rs-` node prefix: a node name is a family
kind generates, an identity is not, and unanchored,
`kubernetes-admin@corp.example.com` launders itself into kind's own admin.

**`.spec.extra` and `.spec.uid` are payload, deleted rather than refused, and
scoped to the object that carries the marker.** `extra` is where a real cluster
puts its OIDC claims and it cannot take the refusal — the `credential-id` the
apiserver stamps is a fresh hash every time, so there is nothing to allowlist.
The scoping is not fussiness: a bare `del(.. | objects | .uid?)` empties
`metadata.uid` on all 23 fixtures — the identity the rule engine is built on —
while still passing every test written about a CSR.

**The marker is `signerName` *or* `issuerRef`, and the first ruling was
reversed.** `signerName` alone was accepted as a deliberate narrowing with
cert-manager's `CertificateRequest` recorded as a known miss; the operator
review then put that kind through the filter and it came out **unmodified**
carrying `alice@corp.example.com` and an OIDC claim. A known miss with a live
proof is not a narrowing, it is the leak. Keyed on the marker rather than on
`has("request") and has("username")`, which reads better and fails open exactly
when it matters: an object carrying `groups` and `extra` without a `username`
is then never examined, and those are two of the four fields being protected.

**An accepted limit, recorded rather than engineered around.** A requester
identity sits in three places on a CSR — `.spec.username`, the DN inside the
base64 `.spec.request`, and the DN inside `.status.certificate` — and jq cannot
decode a DER subject, so the guard reads one of three. A production kubeadm CSR
carrying `CN=alice@corp.example.com` in its request passes, because
`CN=kubernetes-admin, O=kubeadm:cluster-admins` is what **every** kubeadm
cluster calls its admin — that half of the allowed set is not kind-specific at
all, and the comment claiming it was has been corrected. Closing it needs
openssl in `fixture-audit.sh`, which is where it would live if it is ever
worth it.

**The lesson, which outlived the box: a guard that asks for an exit status
cannot see a deletion.** `fixture-audit.sh` gained a backstop asking
`sanitize.jq` whether it would *refuse* each committed fixture — and
`csr-pending.json`, captured before the `del(.extra, .uid)` clause existed,
still carried the credential hash. The filter accepted it, so the backstop
passed, and the audit's green line was byte-identical to a clean run. The
question worth asking is not "would the filter refuse this" but **"would the
filter change this"** — idempotence. Twenty-two of the twenty-three JSON
fixtures were already byte-identical under the filter; the twenty-third was the
one that had never met it. Nothing leaked — the value was a thumbprint of a
throwaway kind admin certificate — but the guarantee that
[G-5](REQUIREMENTS.md#devsecops-requirements) rests on had quietly stopped
holding, which is the same failure as
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) wearing different
clothes.

**And the guard's own failure mode was worse than the leak it caught.**
`make-csr.sh` sanitized with `jq -f sanitize.jq … > "$out"` where `$out` *is*
the committed fixture: the shell truncates before jq runs, so a refusal — the
one thing the filter exists to do — destroyed the file the script exists to
produce, and `set -euo pipefail` then skipped the cleanup and left a CSR on the
cluster. It writes to a working copy and moves on success, and the delete moved
into the `EXIT` trap. *"The cluster is left as it was found"* was true of the
happy path and nothing else.

### D60 — CLAUDE.md was compressed, and four stories moved here (2026-08-12)

`CLAUDE.md` is loaded in full at the start of every session and again after
every compaction, before a single line of work has been read. Measured, it was
682 lines and 6,548 words — roughly 9,500 tokens of standing cost, of which
§ Agent workflow alone was 30%. It is now ~3,000.

**The rule applied: every rule stays in `CLAUDE.md`, the story behind it moves
here.** That is not a new split, it is the one
[docs/maps.md](docs/maps.md) already states — *why* is this file, *how must I
work* is `CLAUDE.md` — and `CLAUDE.md` was the file breaking it, carrying
paragraphs of justification for rules that already link to a `D`-number. No
invariant, no security-gate box, no ownership row and no cycle step was
dropped; what went is prose that argued for them a second time.

**Two defects the pass found, both invisible while writing.** The header read
*"Current phase: CODE — Phase 1, `feat/scaffold`"* while the same file's git
rules said all work is on one long-lived `development` branch
([D32](#d32--one-long-lived-development-branch-not-one-per-phase-2026-08-12)) —
a hand-maintained status line that rots the moment a phase closes. It is gone
rather than corrected: the *rule* for finding the next box is stable, the phase
number is not, and `todo.md` already answers it. And § Workflow (per feature)
described the same seven steps as § The cycle, having drifted apart in the
middle — one ordered docs-sync after the `todo.md` tick, the other before.
There is one description now, in the cycle, and its step 7 spells the order.

**Four things existed only in `CLAUDE.md` and are recorded here so the rules
they justify do not become arbitrary:**

1. **The scratchpad is a shared tree.** `tester`'s mutation driver was
   overwritten between rounds by a file it never wrote; running it would have
   re-run **the author's own sweep** and reported it as independent
   verification — a plausible, well-formatted, entirely worthless result, and
   the exact closed loop step 4 exists to break. Hence one subdirectory per
   agent, and a hash or line count before anything saved earlier is reused.
2. **Review is not a parallel slot.** Work stacked on an unreviewed box turns a
   rejection into a rebase, and a rebase under time pressure is how a finding
   gets quietly dropped. The dev idles during review; that idle is the price of
   the gate meaning something.
3. **`just check` is the whole of CI, or it is a lie.** `cargo deny` first went
   red *after* a push, because CI ran a step `just check` did not. A step whose
   tool is not installed locally is added anyway — a missing binary is a loud
   error, a missing step is an invisible gap.
4. **"Run it" means something different before `main.rs` exists.** Demanding a
   binary run in Phase 3 would set a gate nobody can pass, and an impassable
   gate teaches everyone to wave gates through — which is
   [the second pass's own item 2](CLAUDE.md#second-pass--nothing-is-delivered-on-its-first-draft)
   turned on the file that states it.

**The limit going forward.** A new rule costs its own line in `CLAUDE.md`; its
justification costs a `D`-number here. If that file passes ~4,000 words again,
what grew is prose, not rules — check before adding.

### D61 — a verify predicate must hold across the whole window, not at one instant (2026-08-12)

`cluster.sh verify` certifies that a fixture reached the state its rule is
about, and `just fixtures` captures that object seconds later. A predicate that
names **one instant of a cycle** therefore certifies a moment that may already
be over by the time the bytes are written.

`[crashloop]` demanded `state.waiting.reason=="CrashLoopBackOff"`. Sampled on
the live cluster, the container was in `state.terminated` 39 times out of 70,
in `waiting: CrashLoopBackOff` 29 and `running` twice — the predicate named the
**minority** half of its own loop. It failed open twice over: `verify` retries
in a wait loop until the flap happens to land right, and its report pass
re-fetches, so a green line proved the state existed at *some* instant, not
that a capture would find it. What actually holds across the loop is *it has
already restarted after an exit 1, and it is not up right now*, and that is
what it now asserts. The ~2s window where it **is** up stays excluded on
purpose: certifying a Running pod as crashlooping is the lie the function
exists to prevent.

**The sweep found one, not two.** The first report named `[init]` as well; that
was asserted from family resemblance rather than read, and `[init]` was already
an `or` over both halves. Recorded because a review that names a defect it did
not read is the same failure as a test that has only ever been green — this one
happened to be wrong in the harmless direction.

**The report/wait asymmetry stays, deliberately.** `verify` waits on one read
and reports on a re-fetch, and that asymmetry is the only reason this class was
visible at all: had it reported the verdict it waited on, a flapping predicate
would have printed PASS every time and stayed invisible until a fixture landed
in a state no rule could fire on. It is also the last read before the capture's
own `kubectl get`, so it is the closest available proxy for the bytes that will
land.

**Two limits accepted rather than engineered away:**

- Every `sleep 3600` fixture exits and restarts an hour after `break`, and
  `[restarts]` asserts `ready==true`, which is false for a moment then. The fix
  is a constraint on the trip, not on the code — `break → verify → fixtures`
  runs well inside the hour — because `ready==true` is the whole point of
  rule 5's fixture.
- `[init]` is loose in the *other* direction: it matches an init container that
  crashed and then succeeded, since it never asks whether the container is down
  now. No fixture in the set can produce that shape, so tightening it would
  change a passing predicate with nothing to prove the difference.

**`broken-owned` is a third workload shape, not a second W2.** Its Deployment
reports `Progressing=True / NewReplicaSetAvailable` with `Available=False /
MinimumReplicasUnavailable` — the replica set rolled out fine and the pods
inside it are dying, which is not
[W2](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)'s
`ProgressDeadlineExceeded`. Both will be in `deployments.json`; they are
different findings.

**D39's claim is now asserted somewhere.** That kubelet writes an
`ownerReference` of kind `Node` onto every static pod was documented upstream
and checked by nobody here; `verify-test.sh`'s corpus carries a real mirror pod
as the negative `[owned]` must refuse. This does **not** close the mirror-pod
box — that one wants the fixture, for N2's `mirror: true` count and rule 8's
only negative.

**Deferred, with the reason.** `verify` certifies the live object; nothing
asserts that the **written** fixture still satisfies the predicate that
certified it — the one check that would catch "verify passed, the capture
landed two seconds later in another state". It belongs in `fixture-audit.sh`,
it is ~12 lines, and it waits until after the capture: writing it now means
proving it against 23 files that the capture is about to replace, two of which
do not exist yet.

### D62 — the fifth place a node name lives, and a guard that asked less than its consumer (2026-08-12)

`kube-system` is the one namespace `broken.yaml` cannot imitate, and preparing
to capture it opened two holes in a sanitizer that had only ever been fed pods
built from that file.

**A node name lives in five places, not four.**
[D31 § 3](#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)
enumerated four fields beyond `.spec.nodeName`; the fifth is the
`ownerReference` of kind `Node` that kubelet writes onto every static pod —
`{kind: Node, name: prod-master-01}` names a machine exactly as well as
`.nodeName` does. Before the fix, a pod named `etcd-prod-master-01` owned by a
foreign `prod-master-01` and carrying no `.nodeName` was **sanitized and
written** rather than refused. The enumeration was not careless: it was
complete for the shapes it had been fed, which is
[D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12) again
on the field axis instead of the object axis. Note the same clause must return
nothing for a *full* Node object, which carries `.kind == "Node"` and its name
under `.metadata.name` — hence the `// empty`.

**IPv6 gets one exemption from its anchor: the bracketed URL form.** D31
anchored IPv6 to the whole string on purpose, because unanchored `::` matches a
Rust path, a C++ scope operator and every `key::value` in a log line. `[fd00::1]`
is exempt because the brackets are what remove that ambiguity — nothing in
prose is bracketed like an address. It is the same two forms as the anchored
rule, in the framing `-n kube-system` introduced: etcd and the apiserver carry
their addresses inside `--listen-client-urls=` and friends. Proven against
planted input only — `cluster.sh` writes no `ipFamily`, so the test cluster is
IPv4 and the real capture contains no IPv6 at all.

**A guard may not ask a weaker question than the thing it guards.** Both new
owner predicates first tested `ownerReferences[].kind == "Node"` with no
`.controller == true`, while D46's ruling above and `rules.rs` both resolve the
*controlling* reference — and the `owned` guard written one box earlier, twelve
lines up in the same recipe, does check it. Three poisoned captures that would
produce **zero** mirror pods for N2 passed green. Not a live leak, since kubelet
always writes `controller: true`; the defect is that a guard certifying a
fixture for a consumer must ask the consumer's question, or it certifies
something the consumer cannot use. Two questions about one field in one file,
two boxes apart, is the cross-box drift the phase-close pass exists to catch.

**`kubernetes.io/config.mirror` is deliberately not among the guards.** The
filter destroys every annotation, which is exactly why D46 takes the mirror bit
off the ownerReference instead. Recorded so its absence does not later read as
an oversight.

**Accepted rather than fixed, each with its reason:**

- **No `kube-system-pods.json` entry in `fixture-audit.sh`'s must-still-be-there
  list.** It would be a third copy of a predicate that already has two homes,
  and [D52](#d52--the-guards-were-fed-the-shapes-their-authors-wrote-not-the-shapes-the-repo-produces-2026-08-12)
  is explicit that the audit and the sanitizer went blind in the same places
  *because* each kept its own copy. The audit's copy-free backstop — re-run the
  filter, demand byte-identity — covers the sanitizer-drift case and now
  reports a foreign Node ownerReference as a refusal.
- **A node name inside a command-line flag is not refused.**
  `--name=k8rs-control-plane` and `--initial-cluster=<node>=https://…` carry the
  identity as a substring, and `node_names` reads fields, not prose. `.spec.nodeName`
  sits on every one of those pods, so the refusal fires on the neighbouring
  field; a substring refusal over arbitrary hostnames cannot be written without
  false-positiving on `--cluster-name=k8rs` and `registry.k8s.io/…`.
- **The capture is taken whole** (~115 KB, the largest fixture in the set).
  Trimming it is hand-editing, which
  [D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12)
  forbids. The name is `kube-system-pods.json` and not `kube-system.json`
  because it is pods only.

### D63 — the field kubectl never writes, and a substitution test that could not see a clause (2026-08-12)

The last Phase 2 manifest box added thirteen shapes the first capture could not
produce. Two operator reviews, run independently on the same diff, converged on
one blocker — and it was not in a manifest, it was in the predicate meant to
certify one.

**`kubectl taint` writes no `timeAdded`, for any effect.** The predicate for the
`dedicated=gpu:NoExecute` fixture demanded one, on the strength of the API
type's own sentence — *"it is only written for NoExecute taints"* — which reads
as a promise and is not one. The only writer in the tree is the node
lifecycle controller's `SwapNodeControllerTaint`, for the taints it adds
itself; k/k [#113044](https://github.com/kubernetes/kubernetes/issues/113044)
asks for the flag that would change that and has not got it, and
[#131644](https://github.com/kubernetes/kubernetes/pull/131644) deleted the
sentence from the type as inaccurate in the other direction too — the
controller stamps `NoSchedule` taints as well.

What that would have cost is the part worth recording. `break-nodes` applies
the taint *first* and asserts *after*, so the eviction lands, the poll loop
then spends its whole 420s on a taint that was applied perfectly, `set -euo
pipefail` ends `just fixtures` at its last line, and `nodes.json` — the one
capture the whole subcommand exists for — is never written. Every other
fixture is already on disk by then, and the bare pods the `NoExecute` taint
evicted are gone for good, so the retry is not "run it again": it is `unbreak`,
`break`, and a fresh ten-minute settle. **A predicate that cannot pass is more
expensive than a rule that is wrong**, because it fails at the end of the most
expensive step, and it fails looking exactly like a cluster that did not
cooperate.

**The timestamp is recovered from the taint nobody types.** Stopping a kubelet
makes the controller add `node.kubernetes.io/unreachable:NoExecute` through the
one function that stamps a time, so `[notready]` asserts the timestamp and
`[tainted]` asserts only what `kubectl` actually writes — key, value, effect.
The cordon's own `node.kubernetes.io/unschedulable:NoSchedule` taint is
asserted to *exist* and not to carry a time: whether a `NoSchedule` taint keeps
its stamp is the half nobody could settle from the source, and asserting it
would have rebuilt the trap one clause over.

**Two files outside this box now rest on the deleted sentence**, and neither is
touched here — recorded so the trip's own `nodes.json` is read as the answer to
both:

- `rules.rs`'s taint test names `kubectl taint … dedicated=gpu:NoExecute` as
  the object that will retire its synthesis. It will not; the controller's
  unreachable taint will. `dev-core` re-points it when the capture lands.
- [`screens/alerts.md`](screens/alerts.md)'s cordon card argues that a cordon
  has no age *because* `timeAdded` is NoExecute-only — the sentence that was
  deleted. If the unschedulable taint carries a stamp, a cordon time is
  readable and [D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)'s
  premise moves. `[cordoned]` now requires that taint to be in the capture, so
  the trip settles it either way. Nothing is built on it before then.

**A substitution test proves a predicate has two halves; it cannot prove a
clause does any work.** Replacing each predicate with `true` and with `false`
and demanding both go red — the discipline
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) installed — was run
over all twenty-six and passed, while `[pending]`'s new toleration clause was
dead: every corpus object that had to refuse the predicate already failed on
the *nodeSelector* clause first, so the toleration could be deleted with the
file staying green. The fix is to delete each clause on its own and demand a
red, which needs an object that differs in exactly that clause — and building
those is what turned up the second false premise in the same box: the manifest
justified its respin by saying no captured pod carried `tolerations`, when the
`DefaultTolerationSeconds` admission plugin puts two on every pod in every
capture. What the respin genuinely adds is a toleration with a *value*.

**The reviews' other seven findings, and what each one was.** A second `break`
could not run at all, because a resized pod's `spec.resources` may not be
changed by an `apply` and nothing put it back (fixed by resetting through the
`resize` subresource first — and a cluster holding an *earlier generation* of
these manifests still cannot be applied over, which is now written where it
happens instead of claimed away). `100Gi` was infeasible only on small
machines, since a kind node reports the host's memory — `1Pi` is infeasible
everywhere, **and the cluster then falsified the conclusion drawn from that**:
a request above the node's allocatable is refused at *admission*, so it never
reaches the kubelet and parks nothing. Being infeasible everywhere is exactly
what stops `1Pi` from producing this fixture. Nothing asserted that the cordoned worker still carried a pod a
drain would move, i.e. that N2's positive fixture was not N2's negative wearing
its name; the node is now *chosen* by what is on it and the choice is asserted
against the committed bytes. The `why` line claimed an N3 positive that a
stopped kubelet does not produce — pressure conditions need an eviction
threshold crossed, which is a cluster change, so **N3 joins the permanently
synthesized list**. The one step that can fail on permissions ran third, after
two destructive ones, on a machine where docker access is per-login; it is
preflighted now, and `unbreak` reports what it undid instead of swallowing a
denial that looks identical to nothing-to-undo. A guard asserted a termination
message was non-null when `REDACTED-IP` is also non-null. And a comment claimed
`verify-test.sh` guarded the usage text's line range, which it never did —
**an invented guard is worse than none, because the next editor trusts it**; the
range is gone rather than the claim.

**Decided in passing, none of it forced by the box:** three workers by default
(one per node state, so no node fixture has two causes; `break-nodes` refuses
on fewer rather than doubling up) · `break-nodes` is its own subcommand called
last by `just fixtures`, never part of `break`, because every one of its three
actions changes a pod state that is still settling · the resize is a capture
step like rule 12's delete, since a pod that is not running has no enacted
resources to disagree with · `broken-sts` carries two replicas so it can be
*partially* ready, which is the only shape that separates `ready` from
`desired` · `broken-rollout` gets an hour's progress deadline so it cannot
become a second W2 fixture mid-trip · the three healthy shapes are separate
pods, because the `healthy` pod is every rule's negative at once and a host
mount on it would leave rule 8 with two positives and no negative · the
toleration moved to `NoExecute` to match the taint rather than the other way
round, since with `timeAdded` gone the effect is the only thing separating
`[tainted]` from its negative.

### D64 — the capture trip: what the cluster settled, and the approval it reversed (2026-08-13)

The capture Phase 2 was built for was finally taken, against a four-node kind
cluster on `v1.36.1`: **23 of 23 pod predicates and 3 of 3 node predicates
passed, and 34 fixtures landed.** Everything below is something no amount of
reading upstream could have produced, which is the argument for the trip.

**The resize fixture cannot use a constant, and the reason inverts the one this
repo had written down.** `100Gi` was rejected as machine-dependent and replaced
with `1Pi` on the grounds that it is infeasible everywhere — which is true, and
is exactly why it produces nothing. A request above the node's allocatable is
refused at *admission*:

```
Error from server (Forbidden): pods "broken-resize" is forbidden: node didn't
have enough allocatable resources: memory, requested: 1125899906842624,
allocatable: 24860065792
```

It never reaches the kubelet, so there is no parked resize and no divergence to
capture. The window that is admitted-and-unenactable is `(available,
allocatable]`, and its top edge is the only point in it that does not depend on
what the other pods on that node happen to be holding — so `break` now reads the
allocatable of whichever node the pod landed on and asks for exactly that. The
number is written down nowhere.

**And the reason is `Deferred`, not `Infeasible` — an approval I gave was
falsified by the cluster.** The review asked for the predicate to demand
`Infeasible`, on the argument that `Deferred` is "not right now" and resolves
itself; I approved it. Anything that would be `Infeasible` dies at admission, so
that tightening made the predicate unsatisfiable by the only path `break` has.
The lesson is not about resize: **a reviewer's argument and my agreement with it
are still both claims**, and the two of them agreeing is not evidence. The
predicate now accepts both spellings and holds the line with a reason *enum*
rather than a single value, so an in-flight `PodResizeInProgress` still fails it.

**`break` was half-idempotent, which is worse than not being idempotent at
all.** The second run hung: `rollout status statefulset/broken-sts` never
returns once an earlier `break` has left a pod on the bad revision, because
under `OrderedReady` the StatefulSet controller will not touch an unready pod,
so `broken-sts-1` stays on a revision the object no longer has — permanently,
not slowly. The guard meant to prevent exactly this had been dead since it was
written: it read the workload's *template* to ask "is this already broken", and
it ran after the `apply` that puts the good template back, so it could only ever
answer no. **A comment described the design; the code did not implement it, and
nothing failed until a second run happened.** The fix reads the **pods'** images
instead — the one fact the apply cannot rewrite — with each workload's selector
taken off the workload itself rather than spelled as a convention shared with
`broken.yaml`. The Deployment survives the same starting state by structure and
not by luck: its replicas have no identity, and with `maxUnavailable: 0` the
unready pod is a surge pod above the desired count, so deleting it costs no
availability.

**The premise under
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
is false: a plain `kubectl cordon` does leave a timestamp.** The capture shows
`node.kubernetes.io/unschedulable:NoSchedule` carrying
`timeAdded: 2026-08-12T21:43:02Z`, while the hand-applied
`dedicated=gpu:NoExecute` — the *more* privileged effect — carries none. The
dividing line is not the effect at all, it is **who wrote the taint**. `kubectl`
is a client: `cordon` sets the bare `spec.unschedulable` boolean and stamps
nothing, and `kubectl taint` writes the taint the user typed and nothing more.
The node lifecycle controller then mirrors that boolean into a taint, and every
taint it adds goes through `SwapNodeControllerTaint`, which does
`taintToAdd.TimeAdded = &now` for each one **before looking at its effect** —
the NoSchedule pass that adds the unschedulable taint calls the identical
function as the NoExecute pass. Read in upstream source and not only in the
capture, because a fixture only ever proves what one version did.

D43 was careful about the citation and wrong about the inference. It quoted
`k8s-openapi 0.28.0`'s `Taint::time_added` — *"It is only written for NoExecute
taints"* — noted that the sentence is **gone from the v1_34+ generated docs**,
and concluded "the behaviour is unchanged". The behaviour was never what that
sentence said: upstream's own comment today is *"TimeAdded represents the time
at which the taint was added"*, `+optional`, with no mention of effects. Reading
the *doc* of a field rather than the *writer* of it is what cost the entry, and
the writer was two function calls away.

What survives D43 untouched is everything downstream of that premise — the
autoscaler and Karpenter split, and N2 staying quiet while a scale-down taint is
present. What falls is only "there is no source for that number". What follows
for `screens/alerts.md`, which drew the card without an age on D43's authority,
is `tui-designer`'s and is not settled here.

**What the trip proved that could not be proven offline**, all of it previously
carried as an assumption: the kubelet does copy a pod-level memory limit into a
container status whose own spec declares only cpu ([D53](#d53--a-committed-capture-is-never-edited-to-make-a-test-pass-2026-08-12));
a Deployment mid-rollout with `maxSurge: 1 / maxUnavailable: 0` reports exactly
the five counter values the fixture was designed around; a StatefulSet's
`updatedReplicas` counts a created-but-not-ready pod; and the kubelet keeps a
log tail in the termination message under `FallbackToLogsOnError`.

**Decided in passing, none of it forced by the box:** the resize predicate keeps
its unreachable `.status.resize` string branch — the pin is `v1_32` and the
support window is pinned ±2, so a server that spells it that way is inside the
window even though the pinned node image is not, and a predicate naming only the
reachable half is one nobody can read a year from now · `scan_second_revisions`
and the break loop are fed by one array, because a workload in the second list
and not the first walks back into the wait it cannot survive · an unreadable
selector falls back to *waiting*, never to skipping, since `-l ""` matches every
pod in the namespace and `broken-image` carries the same bad image · the trip's
teardown reads the capture **before** `unbreak`, so a missing shape is caught
while the cluster that could still produce it exists.

**One hazard is known and deliberately left open:** `break` waits on
`pod/broken-resize` becoming Ready, and on a cluster where `break-nodes` has
already run that pod can sit on the kubelet-less node or be evicted by the
`NoExecute` taint — the wait then burns its 300s and ends the run at the same
place the StatefulSet used to. That is a `break-nodes` leftover rather than a
`break` one, and what `break` should do about a cluster `break-nodes` damaged is
a plan decision; `unbreak` is the documented answer until it is made.

### D65 — the repin: N2 gains a clock, and what two agents decided that no brief did (2026-08-13)

The capture stamps every object after the old pin, so the pin moves with it:
**`2026-08-13T00:00:00Z`**, midnight after the capture day, 2h16m after the
newest moment in the new fixtures. The value was decided by the PM and taken
away from both agents on purpose. D57 puts the same instant in four places
across two ownership rows, and `certs-test.sh` reads the Rust pin and refuses to
disagree with it — so two agents each choosing a defensible value produces a red
build whose cause reads like a clock bug. A fact written in four places is not
four decisions.

**N2 gains a clock, and that is a rule capability change, not a test detail.**
`Taint::added_at`'s doc said upstream writes `timeAdded` for `NoExecute` taints
only, therefore N2 could never say how long a node had been cordoned. The
capture shows the split is *who wrote the taint*, not which effect: the node
lifecycle controller stamps `timeAdded` on every taint it adds — including the
`NoSchedule` one it mirrors from `spec.unschedulable` — and `kubectl taint`,
being client-side, stamps none. So the cordon **does** carry a time and N2 can
say "cordoned 2 hours ago"; the `Option` survives, for the hand-applied taint
rather than for the cordon. This is the last standing consequence of the premise
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)
was built on, and it lands as a capability the rule may now use — not one it
must. `screens/alerts.md`'s cordon card is `tui-designer`'s round.

**The certificates were not regenerated.** Their `notBefore`/`notAfter` bytes
are committed evidence; moving `now` past them changes only what the arithmetic
says — 23 / 364 / −4 where it used to say 24 / 365 / −3. Each fixture still
exercises the case it exists for, and regeneration would have written fresh key
material into the repo to buy nothing. One relationship ended quietly with it:
`now` used to equal the certificates' `notBefore` exactly and now sits a day
after. Nothing asserted that equality in either direction, so it was free to
break — which is the point worth keeping, not the day.

**What re-deriving the seventeen tests actually bought.** They were fitted to
literals — a uid, `restarts == 5`, a scheduler sentence — so a bigger cluster
reddened them and the cheap repair is to paste in the new literal, which is
fitting the test to the answer. They now read the capture's own JSON *at the
path the field must have come from*, which is not a tautology: the decode is not
what it is read from, so a field dropped, filled from its neighbour or rewritten
still fails. Each pairs the derivation with the property the fixture must keep —
`restartCount` **and** `≥3`, because CrashLoopBackOff implies several deaths —
so a fixture that goes soft is still caught. Two things the old shape hid
surfaced: one hostPath synthesis had gone **degenerate**, cloning a container
into a collision with the real one, and `nodes.json`'s "nothing is wrong
anywhere" loop was **false by construction** against a deliberately broken
cluster.

**The choices the briefs did not make, recorded because nobody could
reconstruct them later.** From `dev-core`: `image.json` decodes `ErrImagePull`
and not `ImagePullBackOff`, because the kubelet alternates between the two for
one broken image and nothing in `just fixtures` waits for either — the test
derives the reason from the capture and asserts it is one of the two, and this
was the single failure where "the world changed" and "the requirement changed"
were genuinely hard to separate; the three hostPath syntheses are **retired**
rather than kept beside the capture, which is what their own notes said landing
meant; `restarts.json` asserts the band `(3..10)` as a deliberate tripwire, so a
pod that drifts past ten reddens the build for having stopped being rule 5's
*WARN* fixture; kindnet's `desired` is **not** coupled to the node count, which
is honest today and a false red the day a node stops tolerating it; and N4 gains
a cross-check that the control plane's kubelet version equals
`tests/fixtures/K8S_VERSION`, so a fixture that acquires a skew is announced
rather than discovered. From `tester`: the repin was **staged in two steps** to
force a real red on the day-count assertions, because the free red only
exercised the cross-file check and those three would have gone green to green
unseen; and two comments outside the lines the brief named were corrected, since
a file whose header narrates a count its own guard no longer produces
contradicts itself.

**A concurrency hazard worth knowing before it costs an hour.** `certs-test.sh`
`sed`-reads the pin out of `src/rules.rs`. Run while a dev holds that pen, it
read a half-written file and reported a **false red** — five consecutive runs
passed seconds later. Disjoint file trees make two agents safe to run in
parallel; a guard that reads across the boundary is the exception, and a lone
cross-file red is suspect until the writer puts the pen down.

### D66 — `just check` is not quite the whole of CI, and the gap is the one CI was built to watch (2026-08-13)

> **Closed the same day it was opened —
> [D67](#d67--the-cross-compile-row-closed-with-a-skip-and-what-the-skip-costs-2026-08-13)
> records which horn was taken.** `just check` now ends with `just cross`, so
> the row below is history: the eleven-out-of-twelve caveat is spent, and the
> deferral in the middle paragraph was a deferral of about six hours.

Found by Phase 2's closing second pass, against
[CLAUDE.md § Running it](CLAUDE.md): *"`just check` is the whole of CI, or it is
a lie."* Comparing the two step lists, they agree everywhere except one row.
`cargo deny` is not the exception it looks like — `just check` runs the binary
and CI runs the pinned `cargo-deny-action`, which is the same check by a
different vehicle. The real one is the **cross-compilation matrix**: CI runs
`cargo check --locked --target <t> --all-targets` across the release targets
and `just check` runs nothing equivalent, so a cross-compile break can only be
discovered after a push. The workflow's own comment says why that matters —
"cross-compilation breaks at link time and it breaks late" — which makes this
the precise failure the rule exists to prevent.

**It is recorded rather than fixed, and that is a judgement, not an oversight.**
The obvious repair — add the targets to `just check` — makes the gate red on
every machine that has not run `rustup target add`, including the one closing
this phase, and a gate that is red by default is one everybody learns to wave
through. The alternative, a skip when the target is missing, is the "loud
error" the rule prefers only if the loudness survives; a green run with a
skipped step in it is what the rule calls an invisible gap. Choosing between
them is a `tester` decision with a real cost either way, and it belongs to
whoever owns the CI box, not to a phase close that found it in passing.

**What is not deferred:** the claim. Until it is closed, "`just check` is the
whole of CI" is true of eleven steps out of twelve, and anybody relying on it
for a cross-target change should run the matrix by hand.

### D67 — the cross-compile row closed with a skip, and what the skip costs (2026-08-13)

[D66](#d66--just-check-is-not-quite-the-whole-of-ci-and-the-gap-is-the-one-ci-was-built-to-watch-2026-08-13)
named two horns and left the choice to `tester`. **The skip won, and on this
machine it was not really a choice:** there is no `rustup` here at all —
`/usr/bin/cargo` is the distribution's rust — so "require the four targets"
does not mean "inconvenient until someone runs `rustup target add`", it means
`just check` is red forever on the machine that closes phases. That is D66's
own *"a gate red by default is one everybody learns to wave through"* in its
worst available form.

**The skip is a narrow, deliberate exception to
[CLAUDE.md § Running it](CLAUDE.md)'s *"a missing binary is a loud error, a
missing step is an invisible gap"*, and the exception is smaller than it
looks.** What that sentence is protecting is the *step*, and the step is now in
`just check` where it was not before. What is missing here is not a binary but
a target's standard library, and the rule offers no verdict on that case. The
loudness is paid for three ways, all of which had to survive a **green** run:
`cross` is the last thing `check` runs, so the banner is the last thing on
screen; the banner names every target that did not run rather than counting
them; and it goes to stderr, so redirecting stdout to a log does not eat it.

**Two failures are deliberately *not* skips**, because either would shrink the
gate in silence — the same invisible gap wearing a different coat. A triple
`rustc` has never heard of is a typo in CI's matrix and fails hard. A matrix
the recipe cannot read out of `ci.yml` fails hard too, rather than cheerfully
checking an empty list.

**The coupling nobody asked for, and why it is the right one.** `just cross`
reads the `- target:` lines straight out of `.github/workflows/ci.yml` instead
of holding its own copy. A second copy of that list is *precisely* the material
this row is made of — a list in two files that agree until they do not — so
closing the drift with a fresh instance of it would have been a repair that
reopens itself. It is guarded by a canary
([CLAUDE.md § a derived list asserts it found something](CLAUDE.md)):
`x86_64-unknown-linux-musl` must appear in the extraction or the recipe stops,
since "extracted nothing" and "nothing to extract" print the same line.

**The direction of that dependency is worth noticing**, because it is the
opposite of the structural fix still recorded as open above — *a CI job that
installed `just` and ran `just check`*. That imagined the workflow depending on
the justfile; what landed is the justfile depending on the workflow. Both leave
exactly one list, so neither blocks the other, and if the CI-runs-`just check`
fix is ever taken the two compose rather than collide. It is a partial step in
that direction and not a substitute for it.

**What could not be proven, stated rather than hidden.** No cross std exists on
this host, so the *non*-skip branch was proven against the host triple
(`x86_64-unknown-linux-gnu` temporarily added to the matrix): same code path,
same command string, only `$t` differs — a real type error in a temporary test
file took `just check` to exit 101, and the healthy run before it genuinely
compiled the crate. A musl- or darwin-*specific* break remains unprovable here,
which is the gap the banner exists to announce. And "loud enough" is a human
judgement, not a testable claim; what is testable and true is *last on screen,
on stderr, every skipped target named*.

**Found in passing and fixed:** `cluster-up`'s `just --list` description was a
dangling fragment (`# worker per node state break-nodes produces…`), because a
two-line comment above a recipe loses its first line to the listing.

### D68 — the age ladder is not the formatter's choice, and what the brief still left open (2026-08-13)

`Finding` now carries `timestamp: Option<Time>` and `rules::age(now, event)`
turns it into words. Two things about that were **taken away from the agent on
purpose**, for the same reason D65 took the pin away from two of them:

**Where the formatter lives.** [Invariant 5](CLAUDE.md) says the *renderer*
turns a timestamp into "4 min ago", and the renderer does — but the function it
calls sits in `rules.rs`, the only file the pyramid lets both callers reach:
`ui.rs` is Phase 11 and the `--once` printer is this phase's temporary
`main.rs`. `screens/once.md` states the consequence outright — *if the two
renderers could disagree about the same finding, one of them is lying* — and
two copies of a ladder is how they would.

**The rungs themselves**, which are not a formatting preference: every string
is one a `screens/` file already prints. `40s ago` from `states.md`'s stale
vitals, `4 min ago` from `alerts.md` and `once.md`, `6 days ago` from the
cordon card `alerts.md` describes as the age it lost. `min` stays abbreviated
and unpluralised because that is how both screens spell it; hours and days are
words and get their singular. The hours rung is the one nothing draws yet — it
follows from the days rung above it rather than from a screen.

**What `dev-core` decided that no brief did, recorded so nobody has to
reconstruct it.** The function is `age` and the private pluraliser is
`counted`. "No age at all" stays `None` all the way to the renderer instead of
arriving as an empty `String` that neither renderer could tell from a
formatting bug — the operator review below then moved the whole render decision
behind `Finding::age`, so no caller writes the flattening at all. Days come from
`as_hours() / 24` rather than
a second division of seconds, so no bare `86400` is in the file. `timestamp` is
the last field of `Finding`. The fixture test asserts the **band before the
string** — the cordon must land on the hours rung, *then* read `2 hours ago` —
so a recapture that moves the pin fails with a repin hint rather than on the
phrase. And `Finding` is not a snapshot type, so the pinned-`now` sweep gains
no walk: the moment a rule puts there comes from a field that sweep already
covers.

**The one rung the brief did not reach, found by the agent's own second pass:**
an event 400ms old. The ladder was keyed on whole seconds and said nothing
about the sub-second window, where the truncation produces `0s ago` — a string
no screen draws and one that reads as a stopped clock. It says `just now`,
which puts a *positive* age in the branch [D18](#d18--the-clock-is-an-input-not-an-ambient-fact)
built for negative ones; that branch is now "too new to count" as well as "the
laptop is behind", and only the second half is about a wrong clock.

**Still open, and it is `tui-designer`'s:** `screens/alerts.md` § *the cordon
card* and `screens/once.md` both still argue from
[D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)'s
falsified premise — quoting `Taint.timeAdded` as *"only written for NoExecute
taints"* — and the test landing here asserts the opposite for the card N2 will
file. Nothing ships a contradiction yet, because N2 itself is a later box, but
the two have to be reconciled before it does. [D65](#d65--the-repin-n2-gains-a-clock-and-what-two-agents-decided-that-no-brief-did-2026-08-13)
handed that round to `tui-designer` and this is the second entry pointing at
it.

### D69 — the operator review that reopened the box, and the prune line that was never true (2026-08-13)

The `Finding.timestamp` box passed its own second pass, `just check` and a real
`--nocapture` run, and the **operator review sent it back anyway**. Every
finding below is closed here — fixed, or deferred to a box with a named owner.
None was rejected, which is worth saying plainly: the gate earned its place for
the fourth time on the same contract
([D36](#d36--the-finding-shape-the-review-sent-back-2026-08-12) ·
[D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12) ·
[D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)).

**The blocker: `just now` had no bound on the future side.** `elapsed <= 0`
rendered *any* moment after `now` as "just now" — an event 25 hours ahead
included, and the ladder test pinned that as the requirement. The `Option`
catches *"there is no field to point at"*; this branch **hid** *"the rule
filled the wrong field"*, and the wrong fields here are future-dated by nature:
C1's `notAfter`, C2/C4's the same, rule 12's raw `deletionTimestamp` inside its
grace window — which the snapshot sweep already documents as legitimately
future. A confident wrong string, on the screen whose whole promise is that a
number on it can be believed.

`age` now answers `Option<String>` and produces **no number** past the bound,
which is `screens/alerts.md` § *No number we cannot produce* applied to the one
case the code exempted from it — the blank right edge already exists and needs
no new screen state. **The bound is five minutes and it was the PM's to set,
not the formatter's:** five minutes is the conventional clock-skew tolerance
(Kerberos, JWT `nbf`/`exp` leeway, most TLS handshake allowances), it covers an
unsynced laptop without argument, and past it the honest answer is not a
smaller number but the header line D55 asked for. Which is the second half of
this decision, because a screen that goes quietly blank is a worse bug than the
one it replaced — see the deferrals below.

**The doc sentence that would have frozen an age two screens require to
advance.** *"`now` is `ClusterSnapshot::now`, handed in"* reads as binding on
every caller; combined with `now` captured once per pass and invariant 7's
block-when-idle, the header's stale-vitals age (`screens/states.md`
`nodes 3/3 (40s ago)` → `(2 min ago)`, both **disconnected** states) could never
move off its first value — and that string is the provenance the ladder cites
for its own seconds rung. `now` is the *caller's* moment: the snapshot's for a
finding drawn in that pass, a fresh read for the staleness age.

**Three more fixed in the same turn.** `age(now, event)` is two arguments of
one type, so a swapped call compiles, cannot panic, and paints *every* card
`just now` — which reads as the cluster falling over; `Finding::age(&self, now)`
removes the swap on the path that matters and collapses the render expression
D68 had recorded as something two agents write twice. The cordon test's band
accepted `[1h, 24h)` and then asserted `"2 hours ago"`, so the recapture it was
built to catch would still have failed on the phrase. And `Option`'s derived
`Ord` puts `None` **first** while `screens/alerts.md` requires ageless cards to
sort **last** in their band — nothing derives `Ord` today, which is exactly why
it needed writing down before Phase 9 reaches for the reflex.

**The wrong-field class had a name and not a single pair.** The review supplied
them, and three are reachable today from fields the snapshot already carries:
**rule 7** must read `ready.last_transition`, never `scheduled.last_transition`
three lines away — a pod up six days that went unready four minutes ago would
have read `6 days ago`, and that number is the one correlated with the deploy;
**N3** must read *that condition's* `last_transition`, not `Ready`'s off the
same flat `Vec`, or a DiskPressure card carries the node's boot time; **N6**'s
subject is the pod, so the blocking node's taint `added_at` is the wrong clock
on the right card — newly tempting precisely because D65 just certified taint
stamps. The list now sits on the field. It also corrected the weakest sentence
in that doc: rule 8 stays `None` **not** because creation time is not when the
mount became dangerous — `spec.volumes` is immutable, so it is exactly that —
but because the card describes a standing property rather than an event, and a
date beside it reads as *"something happened"*.

**`timeAdded` is the age of the taint, not of the cordon**, and N2 will print
it. It survives a kubelet restart (the kubelet writes `spec.taints` only at
registration) and a controller-manager restart (`MatchTaint` compares key and
effect, so an existing taint is never re-added), but anything that rewrites
`node.spec.taints` wholesale — `kubectl edit`, a GitOps controller reconciling
Node objects, a manifest re-apply — drops it and the controller re-stamps it,
and a taint that pre-existed the cordon is never stamped at all. So the card may
say *"cordoned about 2 hours ago"* and never build an argument on it. **The
durability claims are upstream-derived, not measured** — there is no cluster up
([§ the boxes no agent can run](CLAUDE.md)) — and the sequence that would close
it is `cordon` → read `.spec.taints` → delete the controller-manager pod → read
again → `docker restart` the node → read again → `uncordon && cordon` → read
again, the last one being the only reading that may change.

**And the line that was never true, found by the same review.** Four files said
the Alerts watches keep *"metadata + status only"* — CLAUDE.md's invariant 6
among them. Five fields the rule set reads live in `spec`: `spec.volumes`
(rule 8), `spec.terminationGracePeriodSeconds` (rule 12), `spec.unschedulable`
and its taints (N2), `spec.containers[].resources` (rule 2, N5) and
`spec.replicas` (the workload `desired` — for Deployments, StatefulSets and
ReplicaSets; a DaemonSet is the one kind that answers from
`status.desiredNumberScheduled`, and it is the only watched kind the old phrase
would have survived). A Phase 5 prune written literally from that phrase
deletes the field this box exists to fill, along with four rules. The wording is
now **"pruned to the fields the `rules.rs` snapshot types name"**, which is
single-sourced and checkable where the old one was a guess that read like a
budget. The occurrence inside D28's resolved open question is left alone: it is
a record of what was argued then, not an instruction.

**Deferred, each to a box with an owner** — the review's remaining findings are
real and none of them belongs to this file. The **day rung** hides 24h01m
through 47h59m behind `1 day ago` while `kubectl`'s own `HumanDuration` prints
`30h` and `47h` before `2d3h`, so k8rs is coarser than the command it teaches
in the band where "before or after yesterday's window" is the question; that,
the cordon card's wording, and the age column's **width budget** (widest string
14 characters, no stated maximum) are one `tui-designer` box, and it is now
Phase 3's, **before** close — `rules::age` freezes with `rules.rs` while `ui.rs`
is Phase 11, so a rung changed after that is a forward-only violation. The
**skew header line** is a Phase 5 box. And the cordon card's `kubectl describe
node` is the one command that does **not** print `timeAdded`, so N2's box owes
either a line that shows it (`-o jsonpath='{.spec.taints}'`) or a written
admission that the age is the claim `describe` cannot back — invariant 4's
teaching device pointing at the one thing it fails to teach.

### D70 — rule 8 is narrowed to `kube-system`, and every storage operator lives outside it (2026-08-13)

Rule 8 fires only on the escalated hostPath case, and even that needed a
narrowing or a fresh kind cluster paints the screen: **it stays quiet for a pod
in `kube-system` that is DaemonSet-owned or a mirror pod.** Writable hostPath is
the *normal* state of node infrastructure, and `tests/fixtures/kube-system-pods.json`
is the proof — kindnet and kube-proxy mount `cni-cfg`, `xtables-lock` and
`nri-plugin` writable across eight pods, and `etcd`, `kube-apiserver` and
`kube-controller-manager` do the same as **`Node`-owned mirror pods**, which is
why the narrowing covers `mirror` and not only DaemonSets. The `/` and
`docker.sock` escalators fire through the silence, because those are not normal
for a CNI agent either.

**The namespace is the part that does not survive contact with a real cluster.**
Every storage and networking operator worth naming installs outside
`kube-system` and mounts writable hostPaths just as legitimately: Rook/Ceph in
`rook-ceph` (`/var/lib/rook`, and the OSD pods mount raw devices), Longhorn in
`longhorn-system` (`/var/lib/longhorn`, plus the manager's `/dev` and
`/proc`), OpenEBS, Cilium wherever it was installed, and essentially every CSI
driver's node plugin, whose whole job is `/var/lib/kubelet/plugins` writable
with `Bidirectional` mount propagation. On any of those clusters k8rs as
specified prints a wall of CRITICALs on the first screen a beginner sees —
which is [invariant 13](CLAUDE.md)'s first half failing outright: someone who
runs clusters would not use this in a normal week.

**It is recorded and not fixed, because both obvious repairs are worse than the
bug.** Widening to *any* DaemonSet-owned pod in any namespace makes a careless
or hostile DaemonSet invisible, and a DaemonSet mounting the node's filesystem
is precisely the case rule 8 exists for — the rule would keep its name and lose
its reason. A **namespace allowlist** is configuration, and this project has no
config file and does not want one
([§ Out of scope](#out-of-scope-the-most-important-section)); an allowlist that
ships as a constant is the same thing with worse ergonomics, and one the user
cannot correct when their operator is in `storage`.

**What would settle it is evidence, and there is none yet.** Every fixture here
comes off a kind cluster that has no storage operator on it, so the
false-positive rate outside `kube-system` is unmeasured, and an unmeasured
number is not a design ([D25](#d25--what-this-review-did-not-decide)). Two
shapes are worth checking when a real cluster is available, because either would
decide this without configuration: whether the *owning workload* is itself
cluster-infrastructure by some readable signal, and whether the honest answer is
**severity rather than silence** — the plain read-only hostPath already belongs
to the Analysis posture rows (Phase 4), and a routine operator mount may belong
beside it rather than on Alerts. Until then rule 8 is correct on the cluster it
was tested against and known to be wrong beyond it, which is a better thing to
write down than to guess at.

### D71 — nine rules, three blockers, and the two that were decisions, not code (2026-08-13)

The first nine rules landed with 14 tests, 31 of 33 mutations red, and every
one of the fourteen printed cards re-derived from the fixture JSON by the
operator review: container names, images, the enacted limit, restart counts,
exit codes, durations, the `path`+`subPath` join, the finalizer, and each
timestamp against its own row in the `Finding::timestamp` table. **The card
arithmetic was sound. Every finding below is about a card these rules produce
on an object the fixtures do not contain** — which is the whole argument for
reviewing against a cluster operator's memory rather than against the suite.

**The escalator list was a decade out of date, and that was the worst of it.**
`RUNTIME_SOCKETS` named Docker's two spellings only. kind runs **containerd**,
as does essentially every cluster built after 2022, so
`/run/containerd/containerd.sock` fell past the escalators into the writable
branch — which is exempted for node agents — and a `kube-system` DaemonSet
mounting the container-runtime socket, the single most common cluster-takeover
shape and the exact object rule 8 exists to catch, produced **nothing**. The
rule's own doc claimed *"nothing in `kube-system` needs the runtime socket"*,
which is true; the code could not see the socket. The exemption's *shape* —
[D70](#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)'s
`mirror || DaemonSet`, escalators asked first — was verified correct, including
that a `kube-system` DaemonSet mounting `/` writable does still fire. Its input
list was the defect. Two smaller evasions of the same compare: `hostPath: "//"`
passes upstream validation and is `/` to the kernel, and `/.` likewise, so
paths are normalised before they are matched.

**Rule 7 dated a container by another container's event.** `pod.ready` is
pod-scoped and does not move until *every* container is ready, while the rule
fires per container — so a container thirty seconds old in a pod unready for an
hour drew `1 hour ago`, which is the `Finding::timestamp` table's own rule-7
row happening one level down. It also bypassed the ten-minute grace outright:
a crash-looping container caught *between* restarts is `Running && !ready`
inside a pod whose `Ready` went False hours ago. The since-when is now floored
at the container's own run start — a container cannot have been out of the
Service longer than its current run.

**And `started` came back, as a suppressor, without reopening
[D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12).**
D51 rejected it as a **trigger** and that ruling stands: it is always true once
a container runs with no `startupProbe`, so it discriminates nothing. The
inverse is a different field. `Running && !started` is reachable *only* when a
`startupProbe` is declared and has not passed — and while it has not passed the
kubelet does not run the readiness probe at all. Without the suppressor rule 7
fires at ten minutes on every Cassandra, Elasticsearch or Vault pod whose
startup probe is `failureThreshold: 60, periodSeconds: 30`. The trigger /
suppressor distinction is written on the rule, because the next reader will
otherwise read this as D51 reversed.

**Two findings were bugs in a decision, not in the code**, which is the pair
worth remembering — the implementation matched what was written down, and what
was written down was wrong.

*Rule 12's margin double-counted the grace.* Corrected in place at
[D55](#d55--the-clock-was-written-backwards-and-the-clamp-protects-the-harmless-half-2026-08-12):
`max(30s, grace)` on top of a `deletionTimestamp` that **already is** request +
grace hid a `terminationGracePeriodSeconds: 3600` pod for an hour past its kill
deadline. It is `> 60s`, flat.

*The exit-code table said 137 is "almost always OOM".* Written when the rule
had no `reason` beside the code; it has one now, and a liveness-probe kill that
outlives the grace lands as `137 / Error`. Rule 2 correctly stayed quiet and
rule 6 printed the memory sentence anyway, holding the field that disproves it.
Corrected in the table above: 137 has two meanings and the object says which.

**Three false-positive classes that needed no unusual manifest, only uptime.**
Rule 6 had no bound at all — `lastState.terminated` never expires, so one
transient restart six months ago is a permanent WARN, the largest volume in the
box and the thing that makes an empty Alerts screen unbelievable
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)); it is silent while
the container is serving, where its history belongs to rule 5. The `Succeeded`
skip missed `Failed`, so Evicted pods — GC'd only above 12 500 by default —
carried permanent cards for pods that will never run again, which NOTES already
routes to the **Waste** report. And rule 5 fired *beside* rule 1 on the same
container: `broken-oom` drew three CRITICALs for one incident, the third
carrying nothing the first two did not. Rule 6 already implemented that
principle for rule 2 and named it — *one event, one card*; rule 5 is the same
case one step over, and is silent under `CrashLoopBackOff`.

**Rule 5's severity now depends on whether the container is serving.** A
container up six weeks with a nightly leak-restart reaches forty and sat
permanently in the same band as `CrashLoopBackOff` while passing every probe. A
red card whose own title says it is serving is what teaches people to ignore
red. `REQUIREMENTS.md` marks the ≥3 / ≥10 numbers *(suggestion)* and they are
kept — but a raw lifetime counter carries no rate, 10 restarts in an hour and
10 in a year are one number, and the snapshot cannot supply the rate. So the
band stays and red does not reach a working container.

**Rules 3 and 4 pointed at a command that cannot show what they claim.**
kubectl's `describeStatus` prints `Reason` for a Waiting container and stops —
`state.waiting.message` is never rendered, and it is those two cards' entire
evidence. This project had already accepted `-o yaml` for rule 12 on exactly
that argument, so they take the same line. `describe` does print Limits,
Restart Count, State and Last State with its message and exit code, so rules 1,
2, 5, 6 and 8 keep it; that was checked per card, not assumed.

**`subPathExpr` was dropped, and it is the one field this box was allowed to
add** (the snapshot types do not freeze until Phase 4 close). It is the
env-expanded twin of `subPath`: a mount of `/` with `subPathExpr: $(POD_NAME)`
gets one directory, and k8rs claimed the whole node filesystem — CRITICAL,
false, the loudest possible wrong card. It cannot be resolved without reading
env values, so a non-empty `subPathExpr` means *narrowed by something we cannot
read* and drops the `/` escalator rather than asserting the root.

**The one the fixtures prove backwards, and it belongs to N1.** `healthy.json`
runs on `k8rs-worker3`, which `scripts/cluster.sh break-nodes` deliberately made
`Ready: Unknown` with the node controller's `unreachable:NoExecute` taint. The
status of a pod whose kubelet stopped posting is a fossil that never expires,
and every rule in this box reads pod status — so the fixture the negative test
leans on hardest is a pod on a node the control plane has given up on, and k8rs
calls the cluster fine. The assertion is still correct and the test message
claiming *"a working pod"* was not. **What this owes N1:** its card has to reach
the pods, not only the node, or Alerts will say "node NotReady" in one place and
nothing at all about the workload that is down.

**Recorded and not built: the pod class nothing here can see.** A pod wedged in
`ContainerCreating` — `FailedAttachVolume`, a PVC that will not bind, a volume
still attached to a dead node, a CNI that will not allocate an IP — is invisible
to all nine rules, and **also to rule 10**, because such a pod *is* scheduled.
It is a weekly failure for anyone who runs clusters. The signal is already in
every committed capture and dropped at decode:
`conditions[PodReadyToStartContainers]` (KEP-3085, GA in 1.32) is False for
exactly this state while `PodScheduled` is True. It costs one field and no new
watch, and it passes both halves of
[invariant 13](CLAUDE.md) — but it is a **new rule**, not one of the eleven, and
this project's named number-one risk is scope creep. So it is written down here
and goes to the user, not into the code. If it is taken, it is a box, and the
field has to land before the snapshot types freeze at Phase 4 close.

**Four boxes, one unit of work.** `todo.md`'s exit-code table, hostPath, and
rule-5-thresholds boxes are *specifications* of the pod-rules box, not separate
work: they cannot be built before it and are necessarily built with it. All four
are checked together, and the plan defect is recorded rather than papered over —
a box that cannot be the unit of work is a box that should have been a sentence
inside another one.

### D72 — rule 13 is added to v1, and the field it was proposed on is narrower than the case (2026-08-13)

**The user reversed the scope guard explicitly on 2026-08-13**, which is the
only way a twelfth Alerts rule gets in
([invariant 13](CLAUDE.md)). The case
[D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)
recorded is real and weekly: a pod that was placed on a node and whose
containers never started — the `ContainerCreating` wedge. Nothing in the v1 set
sees it, and **rule 10 does not either**, because such a pod *is* scheduled.

**But the field the review proposed it on does not cover the failures the
review listed, and building on that sentence would have shipped a rule that is
quiet for most of its own class.** `conditions[PodReadyToStartContainers]` is
KEP-3085's renamed `PodHasNetwork`, it is written only once a pod is assigned to
a node, and it is not the trigger — it distinguishes *why*.

> **The mechanism in the paragraph that used to stand here was wrong, and it was
> mine.** It said the sandbox is built first and volume work happens after, so a
> volume wedge would read `True`. The opposite is true: `kubelet.SyncPod` calls
> `volumeManager.WaitForAttachAndMount` **before** `containerRuntime.SyncPod`
> creates the sandbox, so **every volume failure leaves the condition `False`**,
> and `True` means the mounts already succeeded. Measured on a real kind cluster
> at the fixtures' own node image, not reasoned: a pod with a missing `configMap`
> volume — this entry's own proposed capture shape — reads `False`; a pod
> mid-pull on a large image reads `True`. The code implemented the sentence
> faithfully and told a beginner whose ConfigMap did not exist to go and look at
> the CNI. Corrected below and in `rules.rs`
> ([D76](#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13)).

So the condition reads **`False` for both pre-sandbox causes — storage and
network** — and `True` when the pod has its storage and its network and the
block is later: the image still downloading, or a container that could not be
created. The name is the trap: `PodHasNetwork` is what it used to be called, and
reading the *name* rather than the *writer* is what cost this entry its
mechanism — the same mistake, in the same file, that
[D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
recorded about `Taint::time_added` a day earlier.

Two facts from the committed captures, checked rather than assumed: **every**
captured pod carries the condition and every one is `True`, so there is no
positive fixture; and `pending.json` — the unschedulable pod — carries **no such
condition at all** and no `containerStatuses`, which confirms the condition is
written only once a pod is assigned to a node.

**Rule 13, as decided.** It fires on the **residual**: the pod is assigned
(`PodScheduled: True`), no container has started, and nothing else already
explains it — not `ErrImagePull`/`ImagePullBackOff` (rule 3), not
`CreateContainerConfigError` (rule 4), not `CrashLoopBackOff` (rule 1). Its
since-when is `scheduled.last_transition` — how long since the scheduler placed
it — and it waits the same **10 minutes** rule 7 waits, borrowed from
`progressDeadlineSeconds`' default for the same reason: a container image can
legitimately take minutes to pull, and firing under that would make every
cold start of a large image an alert. **`PodReadyToStartContainers` is the
evidence line, not the gate**: `False` says the machine has not given the pod a
network yet, `True` says the sandbox is up and the block is after it — which is
almost always a volume. **WARN, not CRITICAL**, because the one thing that
still looks like this on a healthy cluster is a slow pull, and a red card that
is sometimes a slow pull is how red stops meaning broken
([D2](#d2--the-dividing-line-broken-now-vs-risky-later)).

**Its positive side does not exist yet and the capture is not obviously
producible.** Both branches are, and cheaply — which is the *other* thing the
corrected mechanism changes. A `configMap` volume naming an object that does not
exist wedges a scheduled pod in `ContainerCreating` with the condition
**`False`**, and any image failure gives **`True`**. Neither needs CNI surgery;
the paragraph that used to stand here sent the capture trip after a cluster-wide
break to reach a branch a bad image name produces on its own.

### D73 — rule 10, and the test that argued for its own deletion (2026-08-13)

Rule 10 reads one condition and quotes the scheduler's sentence, which is the
whole value of it. The operator review found the gate correct for every shape it
could construct but one, and the two blockers are worth keeping for what they
are rather than what they cost.

**The card was false for a preempting pod, and the field that disproves it sits
in the same status object.** When preemption picks a node, kube-scheduler writes
`status.nominatedNodeName` in the *same status patch* as
`PodScheduled: False / Unschedulable`, and the pod stays there for the whole
graceful termination of its victims — 30s by default, minutes with a real grace
or a `preStop` hook, unbounded when a victim is stuck, which is rule 12's reason
to exist. So k8rs said *"no machine in the cluster will take this pod"* and sent
the reader to audit requests, labels and taints, while the API said a machine had
been chosen and was clearing space. `PodSnapshot` gains `nominated_node_name`
and rule 10 is silent when it is set.

**The second card was refused, and recorded rather than dropped.** *"A machine
has been chosen for this pod, but it is waiting for other pods there to shut
down first"* is true, useful, and something no other tool explains to a
beginner — and it is a **new rule**, so it goes to the user like
[rule 13](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)
did rather than arriving as a side effect of a bug fix. Silence is defensible
in the meantime: preemption in progress is the system working, and when it
stops working the victim carries rule 12.

**The second blocker is the one to remember: the box's most consequential
decision was held in place by nothing.** Deleting the reason half of the gate —
`if scheduled.status != "False"` — left the **entire suite green**. So the
`SchedulingGated` exclusion, which the rule's doc argues at length, was
untested; and on a Kueue, Volcano or Yunikorn cluster that one-line
simplification puts a CRITICAL *"no machine will take this pod"* on every pod
in the queue, all of them parked exactly as their author intended.

What makes it worth an entry is *why* it was missed. The dev tested the pair the
API server **cannot** produce — `status: True` with `reason: Unschedulable`,
reachable only by `patch pods/status` — and measured that hole honestly, closed
it with a planted field, and proved it red. Both shapes the API server produces
**daily** went untested, and the test's own closing message —
*"the verdict is `status`, and `reason` only says which verdict it was"* — reads
as permission to delete the untested half. **A test can argue for its own
deletion.** The rule that catches this is already in CLAUDE.md and is about
inputs, not intentions: a check is proven only for the shapes it was fed
([D29](#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)),
and "shapes" means the ones the real pipeline hands it, not the ones the
argument is about.

**Severity: the flat CRITICAL was reversed into a ladder.** The dev's argument
was that `Unschedulable` is a verdict rather than a phase every healthy pod
passes through, so the window rules 7 and 13 need does not apply. The premise is
false on three routine paths: an autoscaler scale-up, where this condition **is**
the trigger Cluster Autoscaler and Karpenter watch for; `Immediate`-mode volume
provisioning, where every fresh StatefulSet replica carries *"pod has unbound
immediate PersistentVolumeClaims"* for the seconds the CSI driver takes; and
node-group rollover or spot reclaim. None needs a human, and CRITICAL in this
file means *this will not run until someone acts*. The card stays immediate —
the scheduler's sentence is the value and a beginner should not wait ten minutes
for it — and the **severity** ladders on the condition's age against
`NOT_READY_GRACE`: WARN below, CRITICAL above, CRITICAL when the stamp is
missing.

**The age is honest about less than it looks.** `LastTransitionTime` moves only
when `Status` changes, and `SchedulingGated` is also `False` — so a pod held by
Kueue for two days and released at 03:00 into a full cluster keeps the *gating*
stamp and reads "2 days ago" one second after becoming unschedulable. It is
still the best field available and there is no code change; what changed is the
wording, and the interaction with the ladder above is stated rather than left to
be discovered: such a pod reaches CRITICAL immediately.

**Recorded for the user, not built: the Pending pod nobody explains.** A pod
with **no `PodScheduled` condition at all** — kube-scheduler down or
crashlooping, or a `schedulerName` naming a scheduler that is not installed,
crashlooping, or missing RBAC — is silent in every rule in the file: rules 1–7
iterate containers and there are none, rule 8 needs a hostPath, rule 12 a
`deletionTimestamp`, rule 13 gates on `PodScheduled == True`. `kubectl get pods`
shows a wall of Pending and `k8rs --once` prints *nothing is broken*, the one
claim `screens/once.md` says has to be true. It is week one of adopting Volcano
or Kueue, which is exactly when someone reaches for a tool like this — but it is
also a **new rule**, and the residual it needs (`Pending`, no condition, older
than a grace) requires `metadata.creationTimestamp`, which `PodSnapshot` does
not carry and whose window closes at Phase 4
([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).
It is taken as **rule 14** in
[D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13).

**A finding whose fix the PM specified wrongly, and the dev proved it.** The
review found the card's `(it shows as Pending)` asserted without reading a
field; the instruction back was *"gate the parenthetical on the phase actually
being `Pending`"*, and that does not close it. `kubectl` prints **Terminating**
off `deletionTimestamp`, not off `phase` — `printPod` overrides the column
whenever a non-terminal phase carries a deletion stamp, which is why
`stuck.json` is `phase: Running` and shows as Terminating. A deleted unscheduled
pod keeps `phase: Pending`, so the literal instruction leaves the parenthetical
saying "Pending" at a reader looking at "Terminating" — the same two-words-one-pod
defect it was meant to fix. The dev implemented the correct gate, then **mutated
the code down to the brief's literal wording and watched the test fail**, which
is what a brief that under-specifies is owed.

**And rule 10 now goes silent on a deleting pod rather than merely quieter.**
Both cards were true of that pod, which is why the dev left the choice open. The
tiebreak is not truth but direction: rule 10's action sends the reader to audit
`nodeSelector`, affinity and requests, while the only thing anyone can do for a
pod on its way out is find what is holding the delete — rule 12's card, which
names the finalizer. Alerts is D2's queue of what is broken now **and
actionable**, and "unschedulable" stops being actionable the moment someone has
asked for the pod to go away. For the 60 seconds before rule 12's margin opens
such a pod draws nothing, which is correct: it is deleting normally.

### D74 — two candidate rules, one refused and one taken, decided on who actually runs this (2026-08-13)

[D73](#d73--rule-10-and-the-test-that-argued-for-its-own-deletion-2026-08-13)
left two new-rule candidates for the user, who handed the ruling back on
2026-08-13: *decide it yourself, for this project.* Both are run through
[invariant 13](CLAUDE.md)'s two halves, and they come out on opposite sides —
which is the useful part, because it shows the guard discriminating rather than
waving things through.

**Refused: "a machine has been chosen, it is waiting for other pods to shut
down" (the preemption card).** The sentence is true, readable, and unexplained
by any other tool — it fails the *first* half anyway. Preemption of user
workloads needs PriorityClasses configured for it, which is a deliberate and
relatively advanced setup: common on batch and ML clusters, absent from most
others, so it is not something someone meets in a normal week. And the deciding
argument is not frequency but **what the reader does with it: nothing.** The
card is informational, the state resolves itself, and Alerts is
[D2](#d2--the-dividing-line-broken-now-vs-risky-later)'s
work queue of things that are broken *now*. When preemption genuinely stops —
a victim held by a finalizer — rule 12 already fires, **on the victim**, which
is both the actionable object and the one the user can do something about.
Suppressing rule 10 on `nominatedNodeName` is the whole fix; silence there is
not a gap.

**Taken as rule 14: the Pending pod with no `PodScheduled` condition at all.**
It passes the first half in a way the raw frequency hides. A wedged
kube-scheduler is rare on a managed control plane and *not* rare on the clusters
this tool's audience actually runs — kind, minikube, k3s, single-control-plane
on-prem — and the other producer, a `schedulerName` naming a scheduler that is
not installed or lacks RBAC, is week one of adopting Volcano or Kueue, which is
exactly when someone reaches for a tool like this. It passes the second half
easily: *"nothing has even looked at this pod yet"* needs no glossary, and it is
precisely the diagnosis a beginner cannot reach alone — they see `Pending`, run
`kubectl describe`, find **no events at all**, and have nowhere to go.

**What decides it is the failure mode, not the frequency.** Without this rule,
every pod in the cluster is Pending and `k8rs --once` prints *nothing is
broken* — the one claim [`screens/once.md`](screens/once.md) says has to be
true. A tool whose empty screen is a lie is worse than no tool, and this is the
only known input that produces one.

**Its shape, decided here so the box does not have to invent it.** Residual, like
rule 13: `phase == Pending`, **no `PodScheduled` condition at all**, and older
than **2 minutes** measured from `metadata.creationTimestamp` — which
`PodSnapshot` must gain, and whose window closes at Phase 4 close
([D42](#d42--the-snapshot-types-freeze-one-phase-after-the-file-they-live-in-2026-08-12)).
The two minutes are anchored rather than picked: kube-scheduler's leader
election defaults to a 15s lease with a 10s renew deadline, so leadership moves
within about fifteen seconds, and two minutes is eight times that — long enough
that no ordinary restart or failover reaches it, short enough to be useful at
3am. CRITICAL: a pod nothing has looked at is not running and will not start on
its own. The card names both causes without claiming which, because the rule
cannot tell them apart — `schedulerName` is not in the snapshot and is not being
added for this.

**One consequence is known and deliberately not solved now.** If the scheduler
really is down cluster-wide, this fires for every owner in the cluster and
buries everything else on the screen. Distinguishing "one pod's `schedulerName`
is wrong" from "the scheduler is gone" needs cross-pod reasoning that `analyze`
could do — it holds the whole snapshot — and that is a second mechanism for a
case nobody has met yet on a real cluster. Grouping by owner already collapses a
Deployment's fifty pods into one card. If the wall turns out to be real, it is a
finding from a real cluster and a later box, not a guess encoded today.

### D75 — the third role nobody asked about, and the card that never cleared (2026-08-13)

The box said *"rules 1–6 read `initContainerStatuses` too"*. `ContainerRole` is
a **three-way** — a native sidecar is an init container with
`restartPolicy: Always` and is neither of the other two
([D51](#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12))
— and both non-Regular roles live in that array. So the widening covers all
three: **a crashlooping native sidecar was producing nothing at all**, which is
the same silence the box exists to end, and nobody had noticed because the box
named the array rather than the roles inside it. Rule 7 stays `Regular`-only;
what a not-ready sidecar does to the pod's readiness is a different question.

**The role goes in the evidence's first fact, not in six titles.** One
`container_fact` means a role added later reaches every rule at once, where a
per-role title would have multiplied six strings into eighteen. And each
framing is a **property of that kind of container**, never a claim about this
pod — *"init container migrate (the app starts only after this one finishes)"* —
because rules 5 and 6 also reach an init container that finished long ago in a
pod that is now serving fine, where *"the app has not started"* would be false.

**`doing_its_job` is the shared suppressor, and it is role-aware because
"serving" is meaningless for an init container.** Running-and-ready for Regular
and Sidecar; **terminated with exit 0** for Init. Leaving the old expression
would have shipped the exact false positive rule 6's suppressor exists to
prevent, arriving through the other array: a wait-for-dependency loop that
crashes until the database answers and then exits 0 keeps its restart count and
its failed `lastState` for the pod's whole life, and drew a permanent rule 5
**CRITICAL** plus a rule 6 WARN on a pod that is working. **Rule 5 is silent
rather than downgraded** on a finished init container: its count is frozen and
can never rise again, and *"looks healthy now, but something is wrong"* is a
sentence about a container that is still running.

**And rule 2 never cleared, which the widening turned up by giving it an init
door.** A container OOMKilled once and serving ever since drew a permanent
CRITICAL — a single OOM never reaches rule 5's `>= 3`, so nothing else carried
it and nothing ever dismissed it. Same class as rule 6's unbounded WARN in
[D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13),
one band louder. **`doing_its_job` alone would have been the wrong fix:** a
container killed five minutes ago and running now is exactly what belongs on
the screen — the kernel just killed it and the next spike will do it again.
What was wrong is *permanence*, so rule 2 is silent only when the container is
doing its job **and** the kill is older than `NOT_READY_GRACE`. The card lost is
a month-old kill on a container that has been fine since, which is a
memory-limit question for Phase 4's Capacity report rather than a queue of what
is broken now.

Two things the dev decided inside that clause, both worth keeping: **an undated
kill is never suppressed** — `finished_at` is an `Option` and the exemption must
be *proved*, so `is_some_and` rather than `map_or(true, …)`, which also drops a
future-dated kill (what clock skew produces) back into the firing branch. And
the **asymmetry with rule 6 is documented at the rule**, or the next reader
finds two suppressors that disagree and "fixes" one: a non-zero exit is an
application error the restart already spent, while a kill by the kernel is a
resource fact about a container still under the same limit — it predicts the
next spike, which is what earns the higher band and why it may be dismissed for
being *old* and never for being *over*.

**A green test that was measuring the wrong thing**, found by the dev's own
second pass: its control varied the *previous* run's exit code and produced
nothing, because the suppressor keys on the **current** state — so the test
passed for a reason unrelated to what it claimed. It is the failure
[D26](#d26--a-green-build-that-proves-nothing-2026-08-12) describes arriving
inside a test that had a red run: witnessing red proves the assertion is
connected to *something*, not that it is connected to the thing named in it.

### D76 — the review that built a cluster, and the premise it measured away (2026-08-13)

Rule 13's operator review did what nothing else in this project has done yet: it
**stood up a real kind cluster** on the fixtures' own node image, planted nine
pods covering the shapes the brief named, put each through `sanitize.jq`, and
read the cards out of the unmodified `analyze()`. Every finding below is a
measurement. Three of them are blockers, and the first one was **mine**.

**The condition's mechanism was backwards, and the card was confidently wrong
in plain language.**
[D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)
said the kubelet builds the sandbox first and mounts volumes after. It does the
reverse — `WaitForAttachAndMount` runs before `containerRuntime.SyncPod` — so
every volume failure leaves `PodReadyToStartContainers: False`, and `True` means
the mounts already succeeded. The code implemented the sentence faithfully and
therefore told a beginner whose ConfigMap did not exist to go and look at the
CNI, and told a pod whose disks are provably fine that a disk was probably
missing. Corrected in D72 and in the two evidence sentences.

**What produced the error is worth more than the error.** I read the field's
*name* — `PodHasNetwork`, what KEP-3085 called it before the rename — and
inferred the mechanism from it. That is the identical mistake
[D64](#d64--the-capture-trip-what-the-cluster-settled-and-the-approval-it-reversed-2026-08-13)
recorded one day earlier about `Taint::time_added`, where the field's *doc* was
trusted over the field's *writer*. Twice now the fix has come from reading the
code that writes the field, and twice the cost was a rule that would have shipped
a confident wrong sentence. **A field is defined by what writes it.**

**The rule was silent on most production pods, and the doc argued it could not
be.** `PodInitializing` was excluded as a pointer that always has something to
point at. The kubelet's `defaultWaitingState` is `PodInitializing` for **both**
status arrays whenever a pod declares an init container — so on anything with
Istio or Linkerd injection, a migration, or a vault-agent-init, every container
reads `PodInitializing` and rule 13 could not fire on a volume wedge, a sandbox
wedge or a stuck pull. It is a pointer only when something is there to point at:
treated as stuck when no container is `Running` and none carries a reason of its
own.

**A gate that needed one container, and a title that spoke for all of them.**
One typo in a sidecar's image gave a pod `kubectl get pods` reports as `1/2`, and
a card claiming its containers had never started — so the reader debugged the
container that had been serving for three minutes. **Both halves were wrong and
both were fixed**, which the brief only asked for one of: the rule skips when any
container is running, *and* the title became NOTES' own row-13 sentence, *"given
a machine to run on, but it has not been able to start"*. The dev found the
second half — the old title is false of a pod whose **init** container completed,
and no skip covers that, because nothing there is running.

**Two containers, two failures, one card, and the count said they matched.**
`stuck.first()` is not spec order: the kubelet **sorts regular container
statuses by name**, so the named container is alphabetical. The count fact said
*"1 other container in the same state"* about a container failing for an
entirely different reason.

**And the residual was swallowing a family that has a rule already.**
`InvalidImageName`, `ErrImageNeverPull`, `ImageInspectError`,
`RegistryUnavailable` and `SignatureValidationFailed` all mean *this image will
never become available*, and the kubelet's message carries the whole diagnosis.
So `nginx:doesnotexist` drew rule 3's CRITICAL immediately with the registry's
own sentence, while `NGINX:::latest` drew **nothing for ten minutes** and then a
WARN about starting that blamed a disk. Two typos, two unrecognisably different
answers. They move to rule 3 — whose box is closed and whose phase is not, the
same in-phase correction rule 2 took in
[D75](#d75--the-third-role-nobody-asked-about-and-the-card-that-never-cleared-2026-08-13)
— and into `EXPLAINED_ELSEWHERE` in the same change, because that const's own
doc requires the pair to move together. `CreateContainerError` and
`RunContainerError` stay in the residual: open-ended causes, and the message
carries the diagnosis.

**The test locked the inversion in, and CLAUDE.md already names why.** The
positive planted `ContainerCreating` on a capture while keeping its `True` and
asserted the evidence contained *"disk"* — but that pair is a real shape, a pod
downloading an image, and *"disk"* is the wrong answer for it. Both halves
asserted what the implementation returned rather than what the requirement says
it must, which is the one thing
[§ Tests must not lie](CLAUDE.md) forbids by name. **A test written from the
code cannot falsify the code**, and a red run does not save it: witnessing red
proves the assertion is wired to something, never that the something is right.
Each half now also asserts the sentence it must **not** carry, so a future swap
has to fail.

**Three things the dev settled that the findings did not.** The image family is
**seven** reasons, not the six the send-back counted — the enumeration was taken
over the arithmetic. `UNUSABLE_IMAGE` is a single const read by rule 3 as its
trigger and by rule 13 as its exclusion, rather than five strings copied into
`EXPLAINED_ELSEWHERE`, which meets that const's *move together* requirement
structurally instead of by promise. And the bare card had been printing *"the
machine's own word for where it is stuck: PodInitializing"* — dressing the least
informative string in the status as a diagnosis; it now says the machine has not
said which step it is on.

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

Added by the browser (2026-08-11), widened by
[D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)
(2026-08-12): **the Alerts view's own inputs are watched permanently** — Pods,
Nodes, and Deployments/StatefulSets/DaemonSets, five low-traffic streams,
pruned to the fields the `rules.rs` snapshot types name — metadata, spec **and**
status, since the rule set reads `spec` on all three kinds
([D69](#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)). Every other kind in the Resources view is listed when you
open it and watched only while it is on screen; closing the view drops the
watch. Otherwise "browse every kind" would mean forty permanent streams, which
is a worse version of the polling problem this architecture exists to avoid.
ReplicaSets are deliberately *not* in the set — they are read on demand, for
the one finding and the one group heading that need them.

### v1 rule set

This is where the product's real value lives. Every rule is a pure function;
all of them testable.

**From the Pod object alone** (one watch, no extra requests):

| # | Finding | Source field | What we tell the user |
|---|---|---|---|
| 1 | CrashLoopBackOff | `state.waiting.reason` | "Container keeps crashing and restarting" + exit code |
| 2 | OOMKilled | `lastState.terminated.reason` (exit 137) + `resources.limits.memory` | "Exceeded its memory limit and was killed by the kernel" |
| 3 | **The image is not usable** — the whole family, not just the two: `ErrImagePull`, `ImagePullBackOff`, `InvalidImageName`, `ErrImageNeverPull`, `ImageInspectError`, `RegistryUnavailable`, `SignatureValidationFailed`. All of them mean *this image will never become available* and all carry the kubelet's diagnosis; splitting them sent `nginx:doesnotexist` to rule 3 immediately and `NGINX:::latest` to rule 13 ten minutes later with a card about a disk ([D76](#d76--the-review-that-built-a-cluster-and-the-premise-it-measured-away-2026-08-13)) | `state.waiting.reason` + `.message` | "Container image is not usable, so the container never started" — wrong name or tag, no pull secret for that registry, or a pull policy that forbids fetching it |
| 4 | CreateContainerConfigError | `state.waiting.reason` + `.message` | "Referenced a ConfigMap/Secret that doesn't exist" |
| 5 | High restart count (even if Running) | `restartCount` | "Restarted N times — looks healthy now, but something is wrong" |
| 6 | Non-zero exit | `lastState.terminated.exitCode` | Translate the exit code (see below) |
| 7 | **Pod Running but container not ready** | `containerStatuses[].ready == false`, **plus how long** — `conditions[Ready].lastTransitionTime` and `containerStatuses[].started`, or the rule fires on every rolling update ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | "Running but not receiving traffic — readiness probe is failing, so it was removed from the Service" |
| 8 | hostPath mount — **only the escalated case** (`/`, docker.sock, writable) | `spec.volumes[].hostPath` | "Mounts the node's own filesystem, writable" |
| 10 | **Pending — and why** | `conditions[PodScheduled].reason == Unschedulable` + that condition's own `message` | "No node can accept it" + the scheduler's own sentence (insufficient cpu / nodeSelector / taint) |
| 12 | **Pod stuck Terminating** | `deletionTimestamp` already in the past — the apiserver sets it to *request time + grace*, so it is the deadline, not the moment ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | "Asked to shut down N minutes ago and still hasn't — held by *(the finalizer, named)* / the kubelet" |
| 13 | **Placed on a node, but the containers never started** — the `ContainerCreating` wedge, the *residual* after rules 1/3/4 explain themselves. Gate: `conditions[PodScheduled] == True`, no container started, > 10 min since that transition. `conditions[PodReadyToStartContainers]` is the **evidence**, not the gate — `False` = no sandbox/network yet, `True` = the block is after it, almost always a volume ([D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)) | `conditions[PodScheduled]` + `containerStatuses[].state` + `conditions[PodReadyToStartContainers]` | "It was given a machine to run on, but it has not been able to start — the node cannot give it *(a network / its storage)*" |
| 14 | **Nothing has even looked at this pod** — `phase == Pending` with **no `PodScheduled` condition at all**, older than 2 minutes from `metadata.creationTimestamp`. kube-scheduler is down or crashlooping, or `schedulerName` names one that is not installed or lacks RBAC. Without it every pod is Pending and `--once` prints *nothing is broken* ([D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13)) | absence of `conditions[PodScheduled]` + `metadata.creationTimestamp` | "Nothing has even looked at this pod yet — the scheduler that should give it a machine may not be running" |

**Rules 1–6 read `initContainerStatuses` as well as `containerStatuses`** — a
pod stuck at `Init:CrashLoopBackOff` is invisible otherwise, and init
containers are where migrations and wait-for-dependency loops live. The
finding names the init container ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)).

Two rules left this table in the [second-pass review](#design-review--second-pass-2026-08-11):
**rule 9 (no limits defined)** moved to the Capacity report and the plain
read-only **hostPath** case to the Analysis posture rows — neither is *broken
right now*, and both are numerous enough to bury the ones that are. Rule 12
took their place because it costs nothing: the Pod watch is already open.

**Rules that need Events** (second watch, `Warning`-filtered — not in v1,
see the requirements review). Rule 10 used to be listed here and was moved
into the v1 table above: the scheduler writes its reason onto the pod, so it
never needed this watch ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)):

| # | Finding | Source | What we tell the user |
|---|---|---|---|
| 11 | Probe failure | Event `reason == Unhealthy` | "Liveness/readiness probe failing" + how many times |

**Exit code translation** (for rules 6 and 2 — where beginners stumble most):

| Code | Meaning |
|---|---|
| 137 **with** `reason: OOMKilled` | SIGKILL after running out of memory |
| 137 **without** it | SIGKILL — it did not stop when it was asked to, which is what a failing liveness probe or a shutdown timeout does |
| 143 | SIGTERM — graceful shutdown, not an error |
| 1 / 2 | The application's own error, check the logs |
| 126 / 127 | Command not executable / not found — `command` is wrong |

**137 has two meanings and the object says which** — this table read "almost
always OOM" until 2026-08-13, and it was written for a rule that had no
`reason` field beside the code. It has one now: a liveness-probe kill that
outlives the grace period lands as `exitCode: 137, reason: "Error"`, rule 2
correctly stays quiet on it, and rule 6 printing the memory sentence sends
someone to raise a limit on a container whose probe endpoint is timing out —
the most expensive kind of wrong, because the fix appears to work for a while
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)).

**Severity escalators** (for rule 8): if the path is `/`, a **container-runtime
socket**, or the mount is not `readOnly` → CRITICAL instead. **The socket is not
Docker's alone** — the list read `/var/run/docker.sock` until 2026-08-13, and
kind, and essentially every cluster built after 2022, runs containerd:
`/run/containerd/containerd.sock` (and CRI-O's `/var/run/crio/crio.sock`) grant
the same node-root, so the escalator that could not see them silenced the exact
object rule 8 exists for. Paths are compared after normalisation, because `//`
and `/.` are `/` to the kernel and were not to a string compare
([D71](#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13))
of WARN.

### Node rules (N-series)

Nodes are few (tens, not thousands), so a Node watch is cheap. These are what
an admin actually reacts to:

| # | Finding | Source field |
|---|---|---|
| N1 | Node NotReady for more than 5 min — the pods on it are dead weight | `status.conditions[Ready]` + `lastTransitionTime` |
| N2 | **Cordoned with pods a drain would still move** — a drain someone started and did not finish. **No duration** (`kubectl cordon` records no time) and **no finding when nothing movable is left** — that is a parked node and a Capacity row. Not counted: `Succeeded`/`Failed` pods, DaemonSet pods and static pods, none of which a drain ever evicts; and the rule is silent on a node carrying an autoscaler scale-down taint ([D43](#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12) · [D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | `spec.unschedulable` + the pod↔node join |
| N3 | DiskPressure / MemoryPressure / PIDPressure — evictions are coming | `status.conditions` |
| N4 | kubelet version skew > 2 minor from the control plane = unsupported | `status.nodeInfo.kubeletVersion` |
| N5 | Overcommitted: sum of pod requests exceeds allocatable. A native sidecar (`restartPolicy: Always` on an init container) is **added**, not maxed — the scheduler charges `max( max over init prefix , sum(regular) + sum(restartable-init) )` ([D46](#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)) | node + pod join |
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
8. **v0.5 — Events-based rule 11** (probe failures) and the noisy-stream
   handling it requires. Rule 10 shipped in M1 — it reads the pod, not events
   ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)).
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
- ✅ **Licence: `GPL-3.0-or-later`** (reversed 2026-08-12,
  [D13](#d13--licence-gpl-30-or-later-reversed-2026-08-12)) — a fork may be
  sold but not closed. `cargo publish` requires the field either way, so this
  blocked the very first step of the code phase.
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
- [x] **A workload whose pods were never created produces no finding — is that
      worth a watch?** → **yes**, resolved 2026-08-12 as
      [D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12):
      Deployments, StatefulSets and DaemonSets are watched; ReplicaSets are
      fetched on demand. The reasoning that produced it: Every v1 rule reads a Pod. If the pods do not exist — a
      ResourceQuota denial, an admission webhook rejection, a missing PVC, a
      bad pull secret at ReplicaSet level — there is nothing to iterate:
      `kubectl get pods` is empty, the Deployment sits at 0/3, and k8rs says
      *cluster healthy*. It is the most beginner-hostile failure class there
      is, and the only one the tool currently cannot see. The signal lives on
      the workload, not the pod: `ReplicaSet.status.conditions[ReplicaFailure]`
      carries the quota/webhook message verbatim, and
      `Deployment.status.conditions[Progressing].reason ==
      ProgressDeadlineExceeded` marks a rollout that gave up.

      **A second hole closes with the same call.** [D3](#d3--findings-group-by-owner-not-by-pod)
      groups findings by Deployment/StatefulSet/DaemonSet/Job, but a pod's
      `ownerReferences` points at its **ReplicaSet**. Nothing in the plan says
      where `web-7d4f5c6b8` becomes `web`, so as written, M1 groups under the
      hashed ReplicaSet name — a random string, in the product whose rule is
      that every visible string is readable by a newcomer.

      **Recommendation:** watch Deployments, StatefulSets and DaemonSets
      (metadata + status only — desired vs ready, plus the Progressing
      condition), and fetch a ReplicaSet **on demand**, only when a finding or
      a group heading needs it, cached by UID. Workload objects are two orders
      of magnitude fewer than pods and barely churn, so this is not the thing
      that makes k9s heavy — a repeated `LIST pods -A` is
      ([§ Architecture](#architecture--where-lightweight-comes-from)). Jobs and
      CronJobs stay in the v0.2 J-series as already planned.

      **Cost, stated honestly:** it changes [invariant 6](CLAUDE.md) — "only
      Pods and Nodes are watched permanently" — so it is a decision, not a
      task, and it must be taken before `k8s.rs` freezes in Phase 5. It also
      adds the three workload kinds to the 10 000-pod memory measurement
      ([D25](#d25--what-this-review-did-not-decide)).

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
| 10 | `broken-pending` | Pending, unschedulable — requests 500 cpu. The scheduler's own explanation lands in `conditions[PodScheduled].message`, which is what rule 10 reads |
| 8 | `broken-hostpath` | hostPath mount of `/`, writable → must come out CRITICAL |
| 7 + 11 | `broken-readiness` | Running but never Ready — the readiness probe always fails |
| 9 | `broken-nolimits` | No limits set. **Not an alert** — this fixture exists to prove the *Capacity report* row |
| 12 | `broken-stuck` | Stuck Terminating: a finalizer nothing removes. Applied by the script, put into Terminating by the capture step |
| 1–6 (init) | `broken-init` | `Init:CrashLoopBackOff` — an init container that exits non-zero while the app container never starts. The pod the old rule set could not see ([D27](#d27--two-findings-the-open-watch-already-paid-for-2026-08-12)) |
| 14 | **none yet, and this one is easy** | `schedulerName: does-not-exist` on an otherwise ordinary pod. Nothing picks it up, so no `PodScheduled` condition is ever written — the exact shape, with no control-plane surgery and nothing to clean up ([D74](#d74--two-candidate-rules-one-refused-and-one-taken-decided-on-who-actually-runs-this-2026-08-13)) |
| 13 | **none yet** | The `ContainerCreating` wedge. Every captured pod has `PodReadyToStartContainers: True`, so rule 13 ships with a negative side only until the next trip. The residual branch is reachable — a `configMap` **volume** naming an object that does not exist — and the network branch may not be, since it needs the sandbox itself to fail ([D72](#d72--rule-13-is-added-to-v1-and-the-field-it-was-proposed-on-is-narrower-than-the-case-2026-08-13)) |
| W1 | `broken-quota` (namespace `k8rs-quota`) | A Deployment whose ReplicaSet cannot create a single pod — the quota allows zero. `kubectl get pods` is empty and the truth lives only on the ReplicaSet's `ReplicaFailure`. It sits in its own namespace because a `pods: "0"` quota applies namespace-wide and would block every pod above ([D28](#d28--the-workload-watch-and-the-blind-spot-it-closes-2026-08-12)) |

`broken-stuck` is why `cluster.sh unbreak` patches the finalizer away before
deleting — a plain `kubectl delete` on it never returns.

Fixture capture is [`just fixtures`](justfile) — and only there. It runs
`cluster.sh verify` and the sanitizer test before capturing anything, pipes
every object through [`scripts/sanitize.jq`](scripts/sanitize.jq) on the way
out, and stamps `tests/fixtures/K8S_VERSION` with the server version the
fixtures came from. The command used to be written out here as well; two
copies of a procedure drift, and the one that drifts is always the one in the
prose.

The negative side lives in [`scripts/healthy.yaml`](scripts/healthy.yaml) and
goes up with the broken pods, so both sides are captured from the same cluster
at the same moment.

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
