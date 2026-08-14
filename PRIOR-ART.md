# Prior art — what k9s's issue tracker teaches k8rs

[k9s](https://github.com/derailed/k9s) is the tool k8rs is measured against. It
has been in the field since 2019 and has **2324 closed and 48 open issues**. That
tracker is seven years of other people's users telling a Kubernetes TUI exactly
where it hurts, for free. This file reads it as a defect catalogue and turns each
recurring class into something k8rs either already refuses by design, or must
handle on purpose.

This is **not** a feature list. Nothing here is a request to build what k9s
built — [NOTES § Out of scope](NOTES.md#out-of-scope-the-most-important-section)
and invariant 13 still decide that. It is a list of the ways a tool of this shape
breaks.

**How it was gathered** (2026-08-14, reproducible):

```
gh search issues --repo derailed/k9s --sort comments --order desc --limit 60
gh issue list -R derailed/k9s --state open --limit 60
gh api "search/issues?q=repo:derailed/k9s+label:<performance|vulnerability|needs-tlc>"
gh api "search/issues?q=repo:derailed/k9s+<keyword>+in:title&sort=comments"
gh pr list -R derailed/k9s --state merged --limit 60
```

★ marks an entry worth opening yourself: its thread carries the root cause, not
just the symptom. Unmarked entries are cited for their titles, labels and comment
counts — the shape of the class, not the detail.

**Status tags used below**

| Tag | Meaning |
|---|---|
| **immune** | a k8rs decision already rules this failure out — the entry records *why*, so nobody reverses the decision without knowing what it was buying |
| **covered** | k8rs plans for it; the pointer says where |
| **gap** | k8rs has no answer yet. Collected at the end, in [§ Open gaps](#open-gaps-this-review-found) |

**k9s is Go + tview; k8rs is Rust + ratatui.** Some defects are tview's markup
language and do not transfer. Where that is true it is said, because the *shape*
of the bug usually transfers even when the mechanism does not.

---

## A. Scale — the largest single complaint class

Seven years, same report: it is slow on a real cluster.
[#468](https://github.com/derailed/k9s/issues/468) (2020, 39 comments, "extremely
slow since 0.9.3"), [#663](https://github.com/derailed/k9s/issues/663) (2020,
"slow in large clusters", 4k pods),
[#176](https://github.com/derailed/k9s/issues/176) (2019),
[#280](https://github.com/derailed/k9s/issues/280) (2019, secrets),
[#2574](https://github.com/derailed/k9s/issues/2574) (2024),
[#3245](https://github.com/derailed/k9s/issues/3245) (2025),
[#4006](https://github.com/derailed/k9s/issues/4006) ★ (2026, open — a large EKS
namespace shows `No resources found`, then rows appear minutes later, or never).

### A1 — watch-not-poll is necessary and not sufficient

**covered / gap.** Invariant 6 removes the periodic `LIST -A` that made k9s heavy.
It does nothing about the *first* list, which is where
[#4006](https://github.com/derailed/k9s/issues/4006) lives: `kubectl get pods -n
big` takes 10s, so the watch's initial list takes 10s, and for those 10s the
screen is a lie.

**For k8rs:** the initial list is a state with a duration, not an instant. This is
[D20](NOTES.md#d20--a-call-that-takes-time-is-a-state-and-there-was-none) applied
to startup, and `REQUIREMENTS.md` already asks for it — *"large clusters (5k+
pods): the initial LIST is slow → show `loading N pods`"*. What is *not* decided
is pagination — see A2.

### A2 — the initial list must be paginated, and the page size is a decision

k9s merged [#3987](https://github.com/derailed/k9s/pull/3987) "paginate metrics
API calls to prevent timeout on large clusters" in 2026 — six years after
[#663](https://github.com/derailed/k9s/issues/663). An unpaginated `LIST pods -A`
on a 10k-pod cluster is one response the apiserver has to build whole, and it is
the single call most likely to time out.

**gap.** k8rs has no decision on `limit`/`continue` for the initial list.
Phase 5's box measures resident memory against 10 000 pods; it does not say how
those pods arrive. Whatever kube-rs's watcher defaults to, the number is ours to
choose and to record.

### A3 — client-side throttling is invisible, and that is the bug

[#3988](https://github.com/derailed/k9s/pull/3988) — "raise default client QPS
from 5 to 50". client-go rate-limits by default; every user who reported "slow"
before that PR was in part reporting a queue inside their own binary, with no way
to see it.

**For k8rs:** if a client-side limiter is ever added, it is a documented number
and a visible state, never a silent default. Confirm what kube-rs does here when
Phase 5 pulls its docs into `tmp/` — do not assume it does nothing.

### A4 — per-object overhead is paid once per object, per refresh

[#3986](https://github.com/derailed/k9s/pull/3986) "batch Hydrate workers to
eliminate per-item goroutine overhead". The Go shape (a goroutine per row) does
not transfer; the Rust shape does: an allocation, a clone or a format per row per
draw, times 10 000, times every event.

**covered.** Invariant 7 (coalesce ~100 ms, block when idle) is the structural
half. The measurement half is Phase 5's 10 000-pod box.

### A5 — the perf fix that got reverted ★

[#3989](https://github.com/derailed/k9s/pull/3989) "skip reconcile cycle when
informer data is unchanged" was merged in 2026-05 and **reverted the following
month** by [#4033](https://github.com/derailed/k9s/pull/4033).

**This is the closest thing in the tracker to a warning aimed at k8rs.** Our
invariant 7 is the same manoeuvre — do less work when nothing changed — and the
same class of defect is available to us: an event coalescer that drops the last
event of a burst shows stale data forever, and it will look correct in every test
that does not assert the *final* state after a storm.

**gap.** Phase 10/11 needs a coalescing test that ends with a quiet period and
asserts the screen equals the last event, not merely that some events arrived.

### A6 — unbounded memory, in the field, for 8 days ★

[#871](https://github.com/derailed/k9s/issues/871) — k9s outlived the shell that
launched it, grew to **21.5 GB resident**, and invoked the kernel OOM killer,
which then killed the node's real pods. A monitoring tool took down what it was
monitoring.

**covered / gap.** The thread never identifies what grew, and that is the point:
nobody was measuring anything over time. The security gate says "sizes are
bounded" and Phase 5 measures resident memory once, against 10 000 pods — a
single reading of a fresh process. The buffer that runs for hours is the log
stream, and Phase 6's gate bounds it in words with no number attached (see E1).

### A7 — the watch's own initial-list strategy can hang forever ★

[#4044](https://github.com/derailed/k9s/issues/4044) ★ (open) is the most
technically precise report in the tracker, and it lands on invariant 6.

Recent client-go enables the **WatchList** feature gate by default, so informers
open the watch with `sendInitialEvents=true&resourceVersionMatch=NotOlderThan`
instead of doing LIST-then-WATCH. An apiserver older than v1.27 does not *reject*
those parameters — it ignores them, streams the initial `ADDED` events, and never
sends the `BOOKMARK` carrying `k8s.io/initial-events-end`. The reflector waits for
that bookmark forever. No error is logged; the cache never syncs; the screen spins
until the user gives up. Because the server returned no error, the documented
fallback to LIST+WATCH (KEP-3157) never fires. Plain `kubectl get` on the same
context is instant.

**Two things this says to k8rs:**

- **The failure mode is protocol-level and silent, and the tool's own default
  causes it.** A watch-first design has a way to hang that a polling one does not:
  waiting for something the server will never send. Any watch we open needs a
  deadline on *first sync*, after which the UI says what it is waiting for — see
  A1's loading state.
- **The floor we declare is not a floor anything enforces.** `Cargo.toml` pins
  `k8s-openapi` to `v1_32` and calls it "the *oldest* supported version" — that is
  a statement about which version of the API types we compile against, made on the
  client side. Nothing stops a user pointing k8rs at a v1.24 cluster, and nothing
  today would tell them that is what went wrong. kube-rs also offers a
  streaming-list strategy; if it is ever switched on for speed, #4044 is the bill,
  and the oldest apiserver we actually mean to support is what decides.

**gap** — the minimum supported apiserver version is declared in a dependency
feature and enforced nowhere, and the initial-list strategy that goes with it is
undecided.

---

## B. Connecting — kubeconfig, auth, and the network

This is the second-largest class, and unlike A it is almost all *startup*.

### B1 — kubeconfig is harder than it looks

[#829](https://github.com/derailed/k9s/issues/829) (multi-file `KUBECONFIG`, 23
comments) · [#1723](https://github.com/derailed/k9s/issues/1723) (does not honour
`KUBECONFIG` like kubectl) · [#2488](https://github.com/derailed/k9s/issues/2488)
(`--kubeconfig` broken in a release) ·
[#620](https://github.com/derailed/k9s/issues/620) (token refresh fails with
multiple files) · [#1002](https://github.com/derailed/k9s/issues/1002),
[#1397](https://github.com/derailed/k9s/issues/1397),
[#1444](https://github.com/derailed/k9s/issues/1444) (namespace from the context
is ignored) · [#3815](https://github.com/derailed/k9s/issues/3815) (open — a
context name containing a space cannot be selected) ·
[#1324](https://github.com/derailed/k9s/issues/1324) (31 comments, config
directory on macOS) · [#2458](https://github.com/derailed/k9s/issues/2458),
[#2465](https://github.com/derailed/k9s/issues/2465) ★ (33 comments — *panic* when
there is no current context) ·
[#2651](https://github.com/derailed/k9s/issues/2651) (30 comments — same cause,
different symptom, four years later).

**For k8rs:** kubeconfig handling is a feature with its own fixtures, not a line
in `connect()`. The shapes that must each have a test: no file · file with no
current context · `KUBECONFIG` with several paths · a context whose name has a
space · a context that names a namespace · a context whose `exec` credential
plugin is missing.

[D19](NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)
already saw the exec-plugin case, and Phase 5's `connect(context)` box is the
right home. **gap:** the list above is not in todo.md.

### B2 — credentials expire *while the tool is running*

[#2048](https://github.com/derailed/k9s/issues/2048) (hangs when an MFA
credential times out) · [#1345](https://github.com/derailed/k9s/issues/1345)
(access denied after a context switch) ·
[#3730](https://github.com/derailed/k9s/issues/3730) ★ (open — an expired
credential surfaces as `Ruroh? 'v1/pods' command not found`; the log shows the
real error three lines earlier).

**For k8rs:** "login expired" is already one of the header states in Phase 11's
box. The lesson from #3730 is C1's, and it is the more important half.

### B3 — reconnect logic dies quietly ★

[#3922](https://github.com/derailed/k9s/issues/3922) is a root-caused report
worth reading whole. Three defects in one updater:

1. the reconnect goroutine calls the refresh **before** entering its retry loop,
   so the first failure kills the reconnector — permanently;
2. it bails out of the process after 5 retries (~75–125 s), so a VPN blip during
   lunch means the tool is gone when you come back;
3. each failed check uses the 120 s call timeout, so recovery is slower than the
   outage.

The reporter's own note: k9s survives disconnects during *active* use, because
navigating restarts the watches by accident. Only the idle path was broken —
which is exactly the path nobody tests.

**covered / gap.** [`NOTES.md`:455](NOTES.md) deletes the manual reconnect key because
kube-rs backs off and reconnects; `REQUIREMENTS.md:142` and
`docs/architecture.md:273` require the retry to be *visible*. What none of them
say is that the tool never exits because of it. **The rule to write down: a
connectivity failure is a banner, never a shutdown, and it is retried forever.**

### B4 — a denied permission must degrade one feature, not the tool

[#4160](https://github.com/derailed/k9s/pull/4160) (2026-08 — *hangs* on sync when
listing namespaces is RBAC-denied) · [#201](https://github.com/derailed/k9s/issues/201)
· [#242](https://github.com/derailed/k9s/issues/242) (no `-n` does not work under
RBAC) · [#1935](https://github.com/derailed/k9s/issues/1935) (the active namespace
is *lost* after touching a forbidden resource) ·
[#4144](https://github.com/derailed/k9s/issues/4144) (open — the port-forward
pre-flight check demands a verb the operation does not need).

And the one to read: [#3583](https://github.com/derailed/k9s/issues/3583) ★ —
k9s 0.50.12 started looking up the *node's* OS before opening a pod shell. In a
hardened cluster where users can exec into pods but cannot read nodes, `s` began
failing with `no os information available`. A convenience feature silently added a
permission requirement, and a whole class of users lost a working key.

**covered.** This is
[D23](NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)
and the security gate's "a 403 degrades that one feature and names the missing
verb + resource". #3583 sharpens it into a review question for every new call:
**which permission does this add, and what breaks for someone who lacks it?**

---

## C. Errors that lie

### C1 — the generic handler ate the real error

[#3730](https://github.com/derailed/k9s/issues/3730) ★ and
[#3132](https://github.com/derailed/k9s/issues/3132) (13 comments) are the same
failure: an authentication error travels up, some layer turns it into "not a
known command", and the user is told their keyboard is wrong when their token is
expired. `k9s.log` had the truth the whole time.

**For k8rs:** invariant 14 (plain language) is about wording. This is stronger and
belongs beside it: **a fallback message may never replace a typed error.** If a
call failed, the screen names *what* failed and *why*; "unknown command" is only
ever printed for input that was genuinely not a command.

**gap** — worth stating in `docs/architecture.md § Error handling`, which today
covers startup errors and not this.

### C2 — "empty" and "not loaded yet" are different screens

[#4121](https://github.com/derailed/k9s/issues/4121) (open — "make it clear if
there are no resources, or if they just haven't been loaded") ·
[#3993](https://github.com/derailed/k9s/pull/3993) ("show syncing status instead
of spurious no-resources warning") · [#4006](https://github.com/derailed/k9s/issues/4006).

**covered.** [D20](NOTES.md#d20--a-call-that-takes-time-is-a-state-and-there-was-none)
plus the loading requirement. Three states, not two: *loading* · *empty* ·
*denied*. The third is B4's and is the one most often collapsed into the second.

### C3 — the apiserver already tells you things nobody shows

[#4106](https://github.com/derailed/k9s/issues/4106) (open) — API-server
`Warning:` headers (deprecated API versions, admission notices) are dropped on the
floor. They are free, authoritative, and exactly k8rs's subject matter.

Noted, not adopted: it fails no invariant, but it is not in v1's rule set either.
If it is ever built it is a rule, not a UI feature.

---

## D. The terminal is not a canvas

### D1 — cluster data as markup ★

[#3051](https://github.com/derailed/k9s/issues/3051) ★ (28 comments) — a log line
or a secret containing `match[]` loses the `[`, because tview reads `[...]` as a
colour tag. Copying a secret out of k9s produced *different bytes* than the
secret. The trail of follow-ups is the interesting part:
[#3885](https://github.com/derailed/k9s/issues/3885) (escape residue still shows
in input fields after the fix) · [#3921](https://github.com/derailed/k9s/pull/3921)
· [#4043](https://github.com/derailed/k9s/pull/4043) ("sanitizeEsc greedy regex
misses all but the last escape on a line") ·
[#3945](https://github.com/derailed/k9s/pull/3945) ("de-escape tview bracket
sequences in log save/copy output").

**Two lessons, and the second is the one that costs.**

1. **immune, structurally** — ratatui has no markup language. There is no `[` to
   escape, so k8rs cannot have #3051. Do not reintroduce the problem by inventing
   a markup pass over API strings.
2. **gap, and it applies to us** — invariant 9 strips control characters on the
   way *in*. k9s's four follow-up PRs are all about the way *out*: whatever
   transformation the screen needs must be **undone** before text leaves the tool
   through copy, `y` YAML, `--once` output or a saved file. Sanitising for display
   and emitting for consumption are two different functions, and #4043 shows the
   regex that handled one occurrence per line instead of all of them.

### D2 — do not fight the user's terminal

[#294](https://github.com/derailed/k9s/issues/294) (22 comments — 0.8.0 stopped
respecting the emulator's 16 system colours) ·
[#3598](https://github.com/derailed/k9s/issues/3598) ★ (16 comments, open — k9s
emits `SGR 1` for highlights; Windows Terminal renders "intense" as *bright*
colours by default, so the selected row becomes unreadable grey-on-grey. It took a
thread in microsoft/terminal to work out why) ·
[#1149](https://github.com/derailed/k9s/issues/1149) (dialog theme).

**For `theme.rs` (Phase 9), three rules:** use the terminal's own 16 colours and
let the user's scheme win · never encode meaning in bold/intense alone, because
its rendering is a per-emulator setting · never assume the background is dark.

### D3 — wrapping and resizing must be pure functions

[#4123](https://github.com/derailed/k9s/issues/4123) ★ (open — wrapped log lines
gain characters that were never in the log; copy them out and the JSON no longer
parses) · [#4107](https://github.com/derailed/k9s/issues/4107) (flicker on
resize).

Same root as D1.2: the wrap is a *view* transformation that leaked into the data.

### D4 — the terminal after a subprocess

[#1690](https://github.com/derailed/k9s/issues/1690) (11 comments — after exiting
a container shell the panels redraw but the arrow keys are dead) ·
[#1787](https://github.com/derailed/k9s/issues/1787) (26 comments) ·
[#1061](https://github.com/derailed/k9s/issues/1061) (19 comments) ·
[#2538](https://github.com/derailed/k9s/issues/2538),
[#1774](https://github.com/derailed/k9s/issues/1774),
[#2820](https://github.com/derailed/k9s/issues/2820) (editor launches).

**covered.** Invariant 8 (panic path restores the terminal) and Phase 12's Ctrl-Z
box ([D24](NOTES.md#d24--ctrl-z)). The addition worth writing into that box:
**leaving raw mode and re-entering it is one function used by every path** —
panic, Ctrl-Z, `e` edit, and any future exec — not three copies. k9s has a
different bug for each of its paths.

---

## E. Logs

### E1 — a stream ends for many reasons and the viewer says one thing

[#1399](https://github.com/derailed/k9s/issues/1399) is **the most-commented open
issue in the repo** (38 comments, open since 2021): `Stream closed EOF`, printed
whether the container restarted, the connection dropped, the log rotated or the
apiserver timed the watch out. Related:
[#790](https://github.com/derailed/k9s/issues/790) (multi-pod tail) ·
[#1228](https://github.com/derailed/k9s/issues/1228) (incomplete logs) ·
[#256](https://github.com/derailed/k9s/issues/256) (hangs when a pod has no logs)
· [#3503](https://github.com/derailed/k9s/pull/3503) (retry, 2025) ·
[#3978](https://github.com/derailed/k9s/pull/3978) ("increase log channel buffer
**and add a drop counter**").

**For Phase 6, three requirements:** name the reason the stream ended and offer
resume · a pod with no logs yet is a state, not a hang · when lines are dropped
for backpressure, **say how many** — #3978 is k9s learning that silently losing
log lines in a debugging tool is worse than a slow one.

### E2 — the log view is where CPU goes to die

[#1342](https://github.com/derailed/k9s/issues/1342) (the log screen redraws every
second regardless) · [#883](https://github.com/derailed/k9s/issues/883) (18
comments — the initial load scrolls the entire history past the user) ·
[#2183](https://github.com/derailed/k9s/issues/2183) (toggling wrap crashed it).

**covered** by invariant 7, provided the log view obeys it too — a "tail" that
redraws on a timer is the fixed-FPS bug wearing a different hat.

---

## F. Quiet wrongness — the class users do not file, they just distrust

### F1 — sorting

Sorting bugs never stop:
[#3793](https://github.com/derailed/k9s/issues/3793) (29 comments, **open**, "sort
by CPU or memory doesn't work") · [#1693](https://github.com/derailed/k9s/issues/1693)
(age sort broken for CRDs) · [#3869](https://github.com/derailed/k9s/pull/3869) /
[#3865](https://github.com/derailed/k9s/pull/3865) (empty capacity strings) ·
[#3926](https://github.com/derailed/k9s/pull/3926) (**panic** when a row has fewer
fields than the sort index) · [#4070](https://github.com/derailed/k9s/pull/4070)
("satisfy strict-weak-ordering for duration/capacity columns" — an inconsistent
comparator is undefined behaviour in most sort implementations) ·
[#4136](https://github.com/derailed/k9s/pull/4136) (job duration sorted as text) ·
[#4166](https://github.com/derailed/k9s/issues/4166),
[#4168](https://github.com/derailed/k9s/pull/4168) (open).

**The single cause: sorting the rendered string instead of the value.** `1Gi` vs
`999Mi`, `2d` vs `10h`, `<none>`, `""`.

**For k8rs:** the Alerts view sorts by severity then recency —
both typed, both ours ([todo.md:1791](todo.md)). Keep it that way. The Resources
view is where the trap lives: it renders server-side `Table` output, which is
**strings** (invariant 12), so any column sort there is a string sort over
formatted quantities. **gap:** Phase 11 must either not offer column sorting in
the browser, or decide the parse. Not deciding is how you get #3793.

### F2 — a number that cannot be defended

[#3953](https://github.com/derailed/k9s/issues/3953) ★ (open, re-opened from
#3764) — `%MEM/L` sums usage across containers and divides by the sum of limits.
If one container of two has no limit, the pod shows 150% of a limit that does not
exist, and a limit is supposed to be the thing you cannot exceed. The reporter's
conclusion: the metric has no intuitive definition at pod level, because limits
are per container.

**covered, and this is k8rs's founding argument.**
[D4](NOTES.md#d4--the-flagship-example-promised-a-number-that-cannot-exist)
already deleted a number that could not exist. Restated as a rule for `rules.rs`
and `analysis.rs`: **never divide by a denominator that is not guaranteed
complete.** Missing limits make the ratio undefined, and undefined prints as
`n/a`, not as a large number. k9s's own fix in flight
([#4155](https://github.com/derailed/k9s/pull/4155)) says exactly this: "render
pod limit ratios as n/a unless all containers set limits".

### F3 — container semantics moved underneath them

[#3101](https://github.com/derailed/k9s/issues/3101) (23 comments — pods using
Kubernetes 1.29 native sidecars, i.e. init containers with
`restartPolicy: Always`, were displayed as `Init:Error`) ·
[#4005](https://github.com/derailed/k9s/pull/4005) (pod status for sidecar init
containers) · [#3866](https://github.com/derailed/k9s/issues/3866) /
[#3870](https://github.com/derailed/k9s/pull/3870) (**panic** in
`initContainerStats`) · [#4145](https://github.com/derailed/k9s/issues/4145)
(open — a pod is marked unhealthy when `containerStatuses` has more entries than
the spec has containers).

**Directly aimed at `rules.rs`** — and it lands half covered, half not.

**Covered, and worth knowing it was not luck.** `container_snapshots` pairs a
status to its declaration **by name**, never by position, and says why in a
comment; the native sidecar already has its own `ContainerRole::Sidecar`, decided
from `spec.initContainers[].restartPolicy`. The panic k9s shipped in
`initContainerStats` has no shape to take here.

**Not covered: the assumption underneath that pairing.** The same comment states
that a status with no declaration *cannot exist* — *"both container lists are
immutable after create"* — and uses it to explain why the miss has no test.
[#4145](https://github.com/derailed/k9s/issues/4145) ★ is a field report of
exactly that object: on Tencent TKE **virtual nodes**, the provider injects a
managed logging container into `status.containerStatuses` with no entry in
`spec.containers`. Two declared containers, three ready statuses, pod `Ready:
True`. The reporter's ask is the interesting half — *keep* showing `3/2`, because
the extra container is real, but judge health only from statuses that match the
spec.

Immutability is not what breaks the assumption; a node implementation that is not
a kubelet is. Virtual-kubelet, serverless nodes and sandboxed runtimes all sit in
that gap.

**gap** — what a status-only container decodes to, and what every rule then says
about it, is undecided and untested.

### F4 — the API surface is not a constant

[#144](https://github.com/derailed/k9s/issues/144) (HPA version) ·
[#3116](https://github.com/derailed/k9s/issues/3116) (16 comments, CRDs stopped
listing after 0.40.1) · [#2842](https://github.com/derailed/k9s/issues/2842)
(CRDs with multiple versions) ·
[#1486](https://github.com/derailed/k9s/issues/1486) /
[#1637](https://github.com/derailed/k9s/issues/1637) (two CRDs with the same
plural in different groups are confused for one another) ·
[#4143](https://github.com/derailed/k9s/issues/4143) (open, `unsupported GVK`) ·
[#3334](https://github.com/derailed/k9s/issues/3334) ("expecting a meta table but
got `*unstructured.Unstructured`").

**covered by invariant 12**, with one sharpening: a resource is identified by
**group + version + resource**, always all three. `pods` is not a key; `apps/v1
deployments` is. #1486 is what a short key costs.

---

## G. Destructive actions

### G1 — k9s arrived where invariant 2 starts

[#319](https://github.com/derailed/k9s/issues/319) (2019, 17 comments — "add an
option to disable Ctrl-K", i.e. *delete without confirmation*) →
[#1016](https://github.com/derailed/k9s/issues/1016) (2021, "make it hard to
delete a namespace") → [#2248](https://github.com/derailed/k9s/pull/2248) (2023,
"challenge deletion by text phrase").

Four years from "please stop deleting things when I mis-key" to typed-name
confirmation. **immune** — invariant 2 has it on day one, including the typed name
for deletes and drains. Recorded here so it is never argued down as ceremony: it
is the ending of a four-year thread in the tool we are compared to.

### G2 — read-only enforced per view is a hole per view

[#3858](https://github.com/derailed/k9s/pull/3858) — the XRay view still allowed
edit and delete while read-only mode was on. Also
[#2434](https://github.com/derailed/k9s/issues/2434) (a cluster-level
`readOnly: false` failed to override the global `true` — the precedence went the
unsafe way) · [#2613](https://github.com/derailed/k9s/issues/2613),
[#3961](https://github.com/derailed/k9s/pull/3961) (runtime toggles).

**immune.** Invariant 2's `--read-only` makes the write path *unreachable*, not
merely unbound, so a new view cannot forget to check a flag — there is nothing to
call. #3858 is what the other design costs, once per view, forever. It is also the
argument to weigh if a runtime read-only toggle is ever requested: a mode you can
turn off at runtime is a mode every code path must re-check, which is the design
#3858 came from.

### G3 — building a command out of strings

[#4047](https://github.com/derailed/k9s/pull/4047) — "use shlex for shell-aware
command parsing", i.e. k9s was splitting command strings by hand.

**immune / covered** — the security gate already requires an argument vector and
never a command string, and the command log is display text that is never
executed. This is the gate's line arriving in someone else's repo as a merged PR.

---

## H. Secrets — the pressure is constant and it is all in one direction

[#123](https://github.com/derailed/k9s/issues/123) (2019, "display secret") ·
[#373](https://github.com/derailed/k9s/issues/373) (2019, 13 comments, **still
open**) · [#1017](https://github.com/derailed/k9s/issues/1017) (2021, 23 comments,
**still open**) · [#3835](https://github.com/derailed/k9s/pull/3835) (2026, decode
during edit — merged) · [#3982](https://github.com/derailed/k9s/pull/3982) (2026,
open — native decoded secret *editing*) ·
[#4080](https://github.com/derailed/k9s/issues/4080) (2026, open — decode in the
YAML view).

And the other direction, exactly once:
[#2461](https://github.com/derailed/k9s/issues/2461) — "secrets are decoded upon
describe", filed as a **bug**.

**covered** by the security gate ("secret values require an explicit reveal and
never enter the command log, the audit log, or the YAML shown by `y`"). The value
of this entry is knowing the shape of the pressure in advance: seven years of
users asking for convenience, and one issue's worth of the person who found their
secrets on screen without asking. #2461 is the one that decides the default.

---

## I. Files on disk

[#2421](https://github.com/derailed/k9s/issues/2421) ★ (27 comments — k9s
**overwrites the user's `config.yaml` on launch**, silently resetting their
customisations) · [#1324](https://github.com/derailed/k9s/issues/1324) (31
comments, config directory on macOS) ·
[#1642](https://github.com/derailed/k9s/issues/1642) (cannot run on a read-only
filesystem) · [#4128](https://github.com/derailed/k9s/pull/4128) (file descriptor
leak in `SaveYAML`).

**immune** — zero configuration on first run is a product requirement
([`NOTES.md`:98](NOTES.md), `docs/tech-stack.md:50`). k8rs has no config file to
overwrite, no config directory to locate and nothing to write at startup, so this
entire cluster of issues cannot happen.

**What it costs, stated honestly:** no persisted preferences, which is why
[#4019](https://github.com/derailed/k9s/issues/4019) (favourite namespaces) has no
k8rs equivalent. That trade was made deliberately; this file is where the bill is
visible. The two files k8rs *does* write — the audit log and the `e` edit temp
file — carry mode 0600 and their own gate items, and #1642 is the reminder that
both must fail gracefully on a read-only home directory.

---

## J. Distribution

[#1697](https://github.com/derailed/k9s/issues/1697) (winget) ·
[#1873](https://github.com/derailed/k9s/issues/1873) (the snap is out of date) ·
[#2128](https://github.com/derailed/k9s/issues/2128) (23 comments — `k9s: command
not found` after a snap install) · [#166](https://github.com/derailed/k9s/issues/166)
· [#4120](https://github.com/derailed/k9s/issues/4120) (open — the latest release
is not tagged on the official Docker image) ·
[#4045](https://github.com/derailed/k9s/pull/4045) (arm64 image built wrong).
k9s carries a `snap` and a `distribution` label because the volume justified them.

**For Phase 13:** every packaging channel is a support queue. Ship crates.io and
GitHub releases with `SHA256SUMS`, own those two, and let third parties package
downstream without implying they are current.

---

## K. Accessibility

[#3955](https://github.com/derailed/k9s/issues/3955) ★ (open — the selected column
is marked by foreground colour alone, invisible to users with colour vision
deficiency or on a skin where it does not contrast; the proposed fix is reverse
video, "a terminal-native, colourblind-safe signal") · plus D2's
[#3598](https://github.com/derailed/k9s/issues/3598).

**gap, and a narrow one.** [`screens/alerts.md`:120](screens/alerts.md) already
says it for findings — *"`●` critical, `▲` warning — symbol **and** colour"*. It
is said once, about one screen, by the screen's author. Selection, focus, the
`changing…` state and the disconnected banner get no such promise. The rule
belongs in `theme.rs` (Phase 9), where it binds every screen: **colour is never
the only carrier of meaning.**

---

## L. Two observations about the tracker itself

**L1 — most reports are about the environment, not the tool.** kubeconfig,
context, install channel, terminal emulator. k9s carries `pilot-error` and
`as-designed` labels and leans on a stale bot. The consequence for k8rs: the
README either answers "what does it do with my kubeconfig, and what permissions
does it need" plainly, or the tracker becomes a support desk. k9s needed
[#113](https://github.com/derailed/k9s/issues/113) ("document the RBAC permissions
needed to run k9s") filed as an issue to get there.

**L2 — the loudest threads are regressions in the startup path.**
"slow since 0.9.3" ([#468](https://github.com/derailed/k9s/issues/468)) · "no
longer starts in the current context namespace since 0.25.12"
([#1397](https://github.com/derailed/k9s/issues/1397)) · "cannot switch context
after 0.50.10" ([#3566](https://github.com/derailed/k9s/issues/3566)) · "custom
views broken since 0.50.10" ([#3576](https://github.com/derailed/k9s/issues/3576))
· "shell broken since 0.24.3" ([#1061](https://github.com/derailed/k9s/issues/1061))
· "cannot shell without node access as of 0.50.12"
([#3583](https://github.com/derailed/k9s/issues/3583)).

Nobody is upset about a rule being slightly wrong. They are upset when yesterday's
working startup stops working. The connect path deserves the strictest tests in
the repo, and it is the one path our rule fixtures do not touch at all.

---

## Open gaps this review found

Twelve items the review found with no home in `todo.md`. **All twelve are boxes
now** — the user ruled on 2026-08-14, and where each one landed is
[D89](NOTES.md#d89--k9ss-tracker-is-read-as-prior-art-and-twelve-of-its-classes-become-boxes-2026-08-14).
Ten are new boxes; two were folded into boxes that already covered the ground,
because a second box beside an existing one is how the same problem gets solved
twice, differently. The table is the trace from a k9s thread to the box that
answers it.

| # | Gap | From | Box |
|---|---|---|---|
| 1 | A container can carry a status with no declaration — what it decodes to, and what every rule then says about it | F3 | Phase 3, new |
| 2 | Decide `limit`/`continue` paging for the initial LIST, and record the page size | A2 | Phase 5, new |
| 3 | Find out whether kube-rs rate-limits us; if it does, the number is documented and the queue is visible | A3 | Phase 5, new |
| 4 | Name the oldest API server k8rs supports, enforce it at connect, deadline the first watch sync | A7 | Phase 5, new |
| 5 | The six kubeconfig shapes, each with a fixture | B1 | Phase 5, new |
| 6 | A connectivity failure is a banner and never an exit, retried forever — and proven on the *idle* path | B3 | Phase 5, folded into the reconnect box |
| 7 | A generic message may never stand in for a typed error | C1 | Phase 5, new — and it edits `docs/architecture.md § Error handling` |
| 8 | Log buffer: a stated number, a drop policy, and a visible dropped-line count | A6 · E1 | Phase 6, new |
| 9 | Sanitise-for-display and emit-for-consumption are two functions; `y` / copy / save / `--once` emit what came in | D1.2 | Phase 6, new |
| 10 | Colour is never the only carrier of meaning — every state, not just severity | K | Phase 9, folded into the severity-symbols box |
| 11 | Coalescing test that ends quiet and asserts the *final* state | A5 | Phase 12, new — the coalescer lives in `main.rs`, not the view layer |
| 12 | Browser column sorting: refuse it, or decide how a `Table` string becomes a sort key | F1 | Phase 10, new |

## What k8rs already refuses, and what that is worth

The entries tagged **immune** are the ones to defend, because each is a decision
that looks like ceremony until you read the thread it prevents:

| Decision | The thread it prevents |
|---|---|
| Zero config file | [#2421](https://github.com/derailed/k9s/issues/2421) — the tool resetting the user's settings on launch |
| `--read-only` makes the path unreachable | [#3858](https://github.com/derailed/k9s/pull/3858) — one forgotten check per view |
| Typed name on destructive actions, from day one | [#319](https://github.com/derailed/k9s/issues/319) → [#2248](https://github.com/derailed/k9s/pull/2248), four years apart |
| Argument vector, never a command string | [#4047](https://github.com/derailed/k9s/pull/4047) |
| Secrets need an explicit reveal | [#2461](https://github.com/derailed/k9s/issues/2461), against seven years of pressure the other way |
| No markup language over API text | [#3051](https://github.com/derailed/k9s/issues/3051) and its four follow-up PRs |
| Watch, never poll-list | [#468](https://github.com/derailed/k9s/issues/468), [#663](https://github.com/derailed/k9s/issues/663), [#2574](https://github.com/derailed/k9s/issues/2574), [#3245](https://github.com/derailed/k9s/issues/3245) |
| No number without a complete denominator ([D4](NOTES.md#d4--the-flagship-example-promised-a-number-that-cannot-exist)) | [#3953](https://github.com/derailed/k9s/issues/3953) |
