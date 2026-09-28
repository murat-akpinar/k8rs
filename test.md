# test.md — the A-to-Z pass over the shipped binary

Every feature of k8rs, run by hand against the real binary, and answered *works*
or *does not*. It exists because `just check` green and a published release are
two different claims: the gate proves the code does what its tests say, and
nothing in it has ever opened the console, pressed a key, or watched a pane
answer. This file is the other half.

## Where this stands — read this first

**Say *continue from `test.md`* and this is the file to pick up.** It is the pass
over the shipped binary, and it is **not** the box work: the next *box* is still
[`todo.md`](todo.md)'s first unchecked one, which today routes to
§ *What the v0.1 close still owes*. The two run side by side — a failing row here
becomes a finding, and a finding becomes either a fix or a `backlog.md` line.

| | |
|---|---|
| **Run so far** | § A (part) · § B · § C · § D · § E · § F · § G (part) · § H (part) · § M (part) · § N (part) |
| **Not started** | **§ I** (Analysis, partly seen) · **§ J** (the browser — its fix landed 2026-09-28, so every row is now re-runnable) · **§ K** (the four detail tabs) · **§ L** (keys, footer, `?`) |
| **Findings** | 10, in § Findings at the end. F1/F2/F4 fixed; F3, F5, F6, F8, F9, F10 are `backlog.md` rulings; F7 judged and closed |
| **Binary** | `cargo install k8rs` → `0.1.0` on the test host. The working-tree build is what a re-run after a fix uses |
| **Cluster** | the four-node `k8rs` kind cluster is up. `scripts/cluster.sh reset` to re-break; never stand a second one up |

**The cheapest next step is § J**, because the fix it was blocked on has landed and
nothing there has been run against it: open a kind, press `esc`, read the footer.
Then § L, whose one row — *every key the help screen names either works or is marked
not built yet* — is what F2 existed for and has never been checked as a whole.

**What a run costs and where it goes.** Every row is driven on the test host, the
console rows inside `tmux` (see below), and the result is written back into its own
row — `[x]` with what it printed, or `[ ]` with why it could not run. An unrun row
that says nothing is worse than an open one that says *needs a restricted
kubeconfig*.

**It holds checks, not boxes.** The next task is still the first unchecked box in
the lowest open phase of [`todo.md`](todo.md) *and nowhere else*
([CLAUDE.md § What to do next](CLAUDE.md#what-to-do-next)) — a row here is never
picked up as work, and checking one off is never a phase's progress. A row that
fails produces a **finding**, and § Findings below is the only place it goes.

**What a failing row is worth is decided the same way a phase close decides it**
([CLAUDE.md § Phase close](CLAUDE.md#phase-close--the-ritual-at-the-end-of-every-phase)
step 6): a **blocker** — wrong output, a crash, a trap with no way out, anything
the security gate rules exploitable — is fixed now, in the same turn, and never
boxed. Everything else lands in [`backlog.md`](backlog.md) with its measurement,
and waits for a ruling. Neither a fix nor a backlog line ever adds a box to an
open phase.

## The version never moves

**`0.1.0`, for as long as this file runs.** Not `0.1.1`, not `0.2.0` — the
maintainer ruled it on 2026-09-28 and the reason is on the record
([D309](NOTES.md#d309--the-cratesio-page-keeps-a-readme-that-says-the-crate-is-not-published-and-the-maintainer-will-not-spend-a-version-number-on-it-2026-09-28)).
So a fix out of this file changes `src/`, the docs and this page, and it changes
neither `Cargo.toml`'s `version` nor `Cargo.lock`'s. **A row that can only be
answered by publishing a new version is not run** — it is recorded in § Findings
and left for whatever release eventually happens.

## What is under test, and where it runs

Two binaries, and they are not interchangeable:

| | Built from | Use it for |
|---|---|---|
| **released** | `cargo install k8rs` — the registry's `0.1.0` | every row, unless the row says otherwise. This is what a stranger gets |
| **working tree** | `cargo build --release` on the test host | re-running a row after a fix, before the fix is pushed |

**A row that has been fixed can never pass on the released binary again, and that
is not a failure.** The version never moves (above), so `0.1.0` on crates.io keeps
every defect this file finds. Once a row has a finding with a fix landed, it is
re-run against the **working-tree** binary and the row says so; the released column
stops being the authority for that one row and stays the authority for every other.

**The pass is the PM's**, the same way the two findings below it were: it stands no
cluster up and tears none down, so it is not the ephemeral-measurement errand
[D92](NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)
gives `k8s-admin`. A row that *does* need its own cluster — a second node pool, a
broken control plane — is `k8s-admin`'s under `K8RS_CLUSTER=review`, and there is
none below today.

**Nothing builds or runs on the dev machine**
([D267](NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)).
Everything below runs on the test host:

```
ssh ubuntu                                   # murat@192.168.1.130
rsync -a --delete --exclude=/target --exclude='/mutants.out*' ~/GIT/k8rs/ ubuntu:k8rs-src/
```

**The console needs a terminal, and `ssh host 'k8rs'` does not give it one** — no
PTY, so the console refuses to draw and answers with the usage and exit `2`, with
nothing on screen to explain why. Every console row is run inside `tmux` on the
host and read with `capture-pane`. **The geometry is part of the row** — F3 is a
cut that only happens at some widths, so a row that fails records the `-x`/`-y` it
failed at:

```
tmux new-session -d -x 120 -y 40 -s t 'k8rs --context kind-k8rs'
tmux send-keys -t t Tab ; sleep 1 ; tmux capture-pane -p -t t
tmux kill-session -t t
```

**The cluster** is the four-node `k8rs` kind cluster, one at a time. It is up as
of 2026-09-28; to get broken pods back, `scripts/cluster.sh reset` — never
`unbreak` then `break`, and the trip finishes inside an hour of the break, or the
pods have moved on. `up · down · status · reset · break · break-nodes ·
break-runtime · rollout · quota · sts · ds · owned · verify` are its verbs.

**The 61 committed fixtures** in [`tests/fixtures/`](tests/fixtures) are what
every file-driven row reads; they need no cluster and no network.

## The surface, counted off the code and not recalled

Re-derive these before trusting them — a count is evidence and goes stale:

```
grep -oE '^(pub )?const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs   # 14 flags
grep -n 'const USAGE' -A 15 src/main.rs                                        # 7 doors
grep -n 'OPERATIONS: \[' -A 20 src/main.rs                                     # 3 operations
grep -n 'const PANES' -A 10 src/main.rs                                        # 7 analysis panes
```

**Seven doors**, and every section below is one of them or one surface behind one:

1. the console — `k8rs [--read-only] [--context <name>] [--namespace <name>]`
2. the file-driven report — `k8rs [--analysis] <file.json>...`
3. the headless cluster report — `k8rs --once [--analysis] [--context] [--namespace]`
4. logs — `k8rs --logs --object <[ns/]pod> [--container] [--previous] [--follow]`
5. one object — `k8rs --describe|--yaml --object <[ns/]name> [--kind <kind>]`
6. an operation — `k8rs [--read-only] ops <operation> <kind>/<name> [<value>] -n <ns>`
7. a permission question — `k8rs ops may-i <verb> <resource>.<group>[/<name>] [--subresource]`

**Fourteen flags.** Six released — `--read-only` `--context` `--namespace` (also
`-n`) `--once` `--analysis` `--subresource` — and eight belonging to the
temporary driver: `--logs` `--describe` `--yaml` `--object` `--kind`
`--container` `--previous` `--follow`
([CLAUDE.md invariant 10](CLAUDE.md#hard-invariants--never-break-one-without-an-explicit-decision)).

---

## A. The command line, before any cluster is dialled

- [ ] `k8rs --help` · `k8rs -h` — does either exist, and if not, what does a
      newcomer get? Record what actually happens; do not assume.
- [ ] `k8rs` with no argument at all — opens the console (door 1), not a usage line
- [ ] `k8rs --nonsense` — usage on stderr, exit `2`, and the usage names every door
- [ ] `k8rs --once nonexistent.json` — a path with `--once` is the refusal the
      usage promises, not a silent cluster run
- [ ] `k8rs --logs` with no `--object` — names the missing flag, exit `2`
- [ ] `k8rs --object x` with neither `--logs` nor `--describe`/`--yaml` — refused
- [ ] `k8rs --describe --yaml --object x` — two doors at once is refused, not
      silently one of them
- [ ] `k8rs --context does-not-exist` — names the context and what it looked in
- [ ] `k8rs -n` with no value · `--namespace` with no value — refused, exit `2`
- [ ] every refusal above goes to **stderr** and prints nothing to stdout
- [ ] no refusal takes longer than an eyeblink — none of them dials a cluster first

## B. Door 2 — the file-driven report

**Run 2026-09-28 on the released `0.1.0`, test host, fixtures copied outside the
mirror.** Every row below passed except the one that was wrong, which is marked.

- [x] `crashloop.json` — plain language, and `CrashLoopBackOff` is explained not
      printed: *"Container keeps crashing, and each restart waits longer"*, with
      `exit 1 (the application's own error)` and a next step naming `--previous`.
      Two cards for one container, and the second earns its place — it carries the
      panic line Kubernetes recorded
- [x] `healthy.json` — `○ nothing is broken`, a sentence, not silence
- [x] `oom.json` — *"Container used more memory than it was allowed and the kernel
      killed it (OOMKilled)"*. The jargon is in parentheses behind the explanation,
      which is the right way round
- [x] a fixture per rule family — `pending` *no machine will take this pod* ·
      `init` · `image` *image is not usable* · `probe0` · `hostpath` *a container
      can drive the container runtime* · `nodes` *stopped responding* ·
      `deployments` *this rollout gave up*. All seven draw the finding their name
      promises
- [x] **`csr-pending.json` does not, and the row was wrong, not the code.** The
      fixture is deliberately a *human* asking for a kubeconfig —
      `signerName: kubernetes.io/kube-apiserver-client` — and C3 answers only for
      `…-client-kubelet`, a node trying to join
      ([D40](NOTES.md#d40--the-capture-could-not-produce-the-shape-so-the-test-sets-one-field-2026-08-12), `src/analysis.rs:3285`). The two tests that
      cover C3 take this fixture and override the signer, and
      `src/analysis_tests/certificates.rs:47` says so in as many words. So
      `[certificates]` printing *"no machine is waiting to be let in"* over it is
      correct. **A fixture's name is not a promise about which rule it fires** —
      this row assumed it was
- [x] two files on one line — `2 pods · 0 nodes`, nothing double-counted
- [x] a file holding a **list** and a file holding **one** — both read
- [x] `--analysis` — all seven panes drew. A pane with nothing to say says which
      thing it lacks: `[capacity]` *"Not checked. Reading what a node has needs
      permission to list nodes"*, `[posture]` *"Nothing here mounts a path from the
      node it runs on"*. Run over `csr-pending.json`, not `nodes.json`
- [x] not JSON · empty · `{}` — three answers, no panic. `bad.txt` *not JSON —
      expected ident at line 1 column 2*; a 0-byte file *not JSON — EOF while
      parsing a value*; `{}` exits `0` and says *"1 object no rule reads"*, which is
      the honest answer rather than a refusal
- [x] a 0-byte file and a directory — the directory is *"Is a directory (os error
      21)"*, a missing path *"No such file or directory"*
- [x] exit codes: `0` for every report drawn, `2` for every refusal, nothing else
- [x] the timestamps read as English — *38 days ago* off an old capture, never an
      RFC3339 string

## C. Door 3 — `--once` against the cluster

**Run 2026-09-28 on the released `0.1.0` against the four-node `kind-k8rs`.**

- [x] `--once --context kind-k8rs` — exit `0`, `66 pods · 4 nodes`, 158 lines of
      findings on stdout and 9 lines of connection story on stderr, which is the
      split `screens/once.md` promises. First card: *"Container needs a ConfigMap
      or Secret that does not exist"* naming the missing object
- [x] `--once --analysis` — all seven panes drew, with real numbers rather than
      refusals: `[capacity]` gave `k8rs-control-plane   0.95 of 4 cpu · 290Mi of
      3.8Gi` for each of the four nodes
- [x] `--once -n kube-system` — scoped, and **the scope is on stdout, not only in
      the kubectl line on stderr**: `ns: kube-system · 14 pods · 4 nodes`, then
      *nothing is broken in kube-system*. A reader who pipes stdout to a file still
      sees which namespace it is about
- [x] **a dead endpoint names the failure and exits rather than hanging** — an
      unknown context exits `2` in **0.006 s** with *"no cluster to watch — this
      kubeconfig has no such context — check the `current-context` line in the
      file, and any `--context` on the command line"*, which names both places to
      look
- [x] the broken pods the cluster carries all show up — `broken-config`,
      `broken-crashloop`, `broken-exit0`, `broken-probe0`, `broken-sigterm`
- [ ] `--once` against a cluster with nothing broken — this cluster is
      deliberately broken, so it needs `scripts/cluster.sh unbreak` and a trip of
      its own
- [ ] `scripts/cluster.sh reset` and re-run — not run; the pods above are from the
      standing break
- [ ] a kubeconfig that cannot `get /apis` — needs a restricted kubeconfig built
      for it, which is its own errand
- [x] **F8: `-n <a namespace that does not exist>` answers `○ nothing is broken in
      nope-not-here`, exit `0`.** `0 pods` is on the line above it, and Kubernetes
      returns an empty list rather than a `404`, so *not there* and *cannot see it*
      are indistinguishable without a call RBAC may refuse. Ruled a backlog item,
      not a blocker — see § Findings

## D. Door 4 — `--logs`

**Run 2026-09-28, released `0.1.0` against `kind-k8rs`.** Every message names what
is wrong *and* what to do, and every one prints its kubectl equivalent.

- [x] `--logs --object default/broken-crashloop` — the log, and
      `$ kubectl logs broken-crashloop -n default -c quitter` beside it
- [x] `--previous` on a crashed pod — same line plus `--previous`
- [x] `--previous` on a pod that never restarted — *"app hasn't restarted, so
      there's no previous run to show. Showing the current run instead."* Then the
      current run also failed, for its own reason, and said so: *"this cluster would
      not accept the request k8rs made to get pods/log in default, and said:
      container "app" ... is waiting to start: CreateContainerConfigError"*. Two
      sentences, both true, the server's own words quoted. Exit `2`
- [x] `--container bystander` on a two-container pod — works. A container that does
      not exist: *"this pod has no container named nope — it has trigger,
      bystander"*, which names the choices
- [x] a multi-container pod with no `--container` — refuses and lists them with
      their state: *"trigger (running, 3 restarts), bysta…"*
- [x] a pod that does not exist — *"there is no pod named no-such-pod in default —
      check the name an…"*
- [x] **`--object "default/../../etc/passwd"`** — refused on the shape before any
      path is built: *"--object names one pod, written as `<namespace>/<name>` or
      just `<…`"*. Security gate row
- [ ] `--follow` and `ctrl-c` — needs an interactive terminal, not run
- [ ] an endless log line held whole in memory — needs a pod built to emit one

## E. Door 5 — `--describe` and `--yaml`

- [x] `--describe --object default/broken-crashloop` — works, with
      `$ kubectl describe pod broken-crashloop -n default`
- [x] `--yaml --object …` — works, with
      `$ kubectl get pod … -o yaml --show-managed-fields`
- [x] **`managedFields` is in the output on purpose and that is checked, not
      assumed.** 88 of 217 lines are the `managedFields` block. `main.rs:5342`
      carries the reason — without the flag the *taught line* would produce a
      different, shorter document, which `main_tests.rs:10779` measured — so
      invariant 4's *the command log shows the equivalent command* is literally
      true. Invariant 6's prune is the watch path's, not this read's. **No finding**;
      whether the console's `y` should prune is that box's question
- [x] `--kind` — **it is not pod-only, and the earlier reading of this row was
      wrong.** `--yaml` takes `secret`, `configmap`, `node`, `deployment` (resolved
      to `deployment.apps` in the kubectl line) and `service`. Only `--describe` is
      pod-only, and it says so: *"--describe only knows how to read a pod right
      now"*
- [x] **A Secret through `--yaml` does not leak, measured with a planted canary.**
      A `k8rs-probe` Secret carrying `password=SUPERSECRETCANARY123` was created,
      read back, and **the canary appears 0 times in plaintext and 0 times
      base64-encoded**. What is drawn is `password: <hidden — 20 bytes>` — the size,
      which is the 20 characters the canary is. And the tool says so rather than
      hiding the hiding: *"a Secret's values are hidden here and shown as their
      sizes — the command above prints them in full"*, which is the one honest
      sentence available when the teaching command is less safe than the tool.
      The planted Secret was deleted and its absence confirmed (`NotFound`)
- [ ] `--describe` on an object that has just been deleted — not run

## F. Doors 6 and 7 — `ops` and `ops may-i`

**Run 2026-09-28 against `kind-k8rs`, on a throwaway `k8rs-probe` deployment the PM
created for it — never on the cluster's broken fixtures.** Cleanup verified: zero
`k8rs-probe` objects left, 42 broken fixture pods intact.

- [x] `k8rs ops` with no operation — the usage names all four, what each asks for,
      and closes with *"There is no flag that means yes."*
- [x] **`scale`, answering yes — every row of invariant 2 visible in one screen.**
      The object (`deployment/k8rs-probe in default`), the consequence in plain
      language (*"This starts 1 more copy of your app. Right now: 2 copies. After: 3
      copies."*), the taught line (`$ kubectl --context kind-k8rs scale … -n
      default`), the server-side dry-run (*"the cluster checked it first and
      accepted it"*), the prompt, then *"the change was made"*. Replicas really
      became 3
- [x] **answering `no` changes nothing** — *"nobody confirmed it, so nothing was
      changed"*, replicas still 3
- [x] `restart` — the consequence carries what an operator actually needs: *"How
      many stop at the same time is a setting on this deployment … A paused
      deployment will not start until you resume it."*
- [x] **`delete` requires the typed name.** `not-the-name` → refused, the object
      still there. `k8rs-probe` → deleted. And **the declined dry-run is said out
      loud**: *"k8rs did not check this one with the cluster first"*
      ([D225](NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04) ruling 1)
- [x] `--read-only ops restart` — *"--read-only was asked for, so k8rs will not
      change anything"*. The flag is filtered out before the line is parsed at all
      ([D230](NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05) ruling 3)
- [x] `ops may-i` — two lines, the question in English then the answer: *"may this
      login get pods in default?"* → *"yes — this login is allowed to do that"*.
      `delete pods.`, `create pods.apps` and a `--subresource` form all answer
- [x] **`may-i` still works under `--read-only`**, because it writes nothing
      ([D230](NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05) ruling 3)
- [x] **no bulk mutation, in three shapes.** `deployment/a deployment/b` →
      *"`ops restart` does not know what to do with …"*; a comma list and a `*` glob
      each → *"is not the name of an object — a name is letters …"*
- [x] **the audit log: mode `0600`, directory `0700`, and every attempt in it.**
      `~/.local/state/k8rs/audit.log`. Each attempt pairs with a result keyed on the
      attempt's own timestamp, and carries the context, the server URL, the uid with
      an honest qualifier (*"what k8rs read, not what it changed"*), the taught
      kubectl line **and** the real call — `PATCH
      /apis/apps/v1/namespaces/default/deployments/k8rs-probe/scale`, `DELETE …` —
      plus `resourceVersion not sent` and the dry-run verdict. **Refusals are in
      there too**: *"nobody confirmed it, so nothing was changed"*. Invariant 4's two
      records differ correctly and neither lies
- [x] an operation on an object that does not exist, and `restart` on a Pod — both
      refused before anything is sent
- [x] `resourceVersion` — `not sent` for `scale` and `restart`, which is
      [D228](NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05)'s ruling: they are absolute intent, not a read-modify-write. The
      only read-modify-write is v0.4's `edit` and it does not exist, so the
      409-offers-a-re-read row has nothing to exercise yet
- [x] **F9: `ops` takes no `--context`** — `k8rs --context kind-k8rs ops …` answers
      *"`ops` has to be the first word on the line"*, so an operation can only ever
      reach the kubeconfig's current context, while the console can write to any
      context it switched to with `X`. Recorded, not a bug — see § Findings

## G. Door 1 — the console, launch and the ways it can go wrong

**Run 2026-09-28 in `tmux` on the test host, released `0.1.0`, read with
`capture-pane`.** Crafted kubeconfigs live outside the mirror.

- [x] **no kubeconfig at all** — *"no cluster to watch — the kubeconfig itself could
      not be read — it is missing, unreadable, or not valid YAML"*. Three
      possibilities named, no stack trace
- [x] **a kubeconfig naming a dead API server** — the console **draws** rather than
      dying. Header `ctx: dead · ⚠ disconnected, retrying`; the pane says *"⚠ Nothing
      is coming back from the cluster. It keeps asking, on its own — nothing for you
      to do. Press X for a different cluster."* — the state, that the tool is
      handling it, and the one action there is. The sidebar drops its five groups for
      a single `could not read` row, which is [D296](NOTES.md#d296--a-refused-apis-is-two-surfaces-with-two-gates-and-the-row-outlives-the-sentence-2026-09-27)'s shape, and the footer
      offers `X switch cluster`
- [x] a context the kubeconfig does not have — exit `2`, naming both the
      `current-context` line and `--context` (§ C)
- [x] **`insecure-skip-tls-verify: true` is honoured *and shown in the header*** —
      `ctx: ins · live · admin · ⚠ TLS not verified`, with the console working
      normally behind it (21 findings drawn). This is the security-gate row marked
      *yours* because no script can see a header. **Measured, passes**
- [x] **a terminal smaller than 80×24** — *"k8rs needs a terminal at least 80×24.
      This one is 60×20."* Names the requirement and the actual size; no garbled frame
- [x] **the audit log cannot be opened** — the path, where the path came from
      (*"under your home directory"*), the OS error, **and the policy**: *"every
      change k8rs makes is written to that log before it is sent, so k8rs will not
      change anything until that is fixed, and reading your cluster still works"*.
      Writes stop, reads degrade gracefully. Directory mode restored to `700` after
- [x] `q` leaves the terminal usable — the shell after it ran and printed
- [ ] **`ctrl-z` then `fg`** — not run, and **no finding is filed from it**: the
      harness was wrong, not the product. `tmux new-session "k8rs; …"` runs under
      `sh -c`, which has no job control, so there was no shell to `fg` into. The repo
      has `just suspend` and `scripts/handover-guard.py` for exactly this; it needs
      the mirror, which `tester` was holding
- [x] **a `403` degrades one feature, names the missing verb *and* resource, does not
      crash and does not loop.** Built a `k8rs-narrow` ServiceAccount that may
      `get/list/watch` pods and nothing else (plus the `nonResourceURLs` discovery
      needs), and ran under its token. `--once` exits `0` and leads with four
      warnings, one per blinded input: *"▲ k8rs is not getting nodes from this
      cluster: the role this kubeconfig uses needs to `list` and `watch` nodes.
      Nothing here about them can be trusted"* — then `66 pods` and the pod findings
      it *can* make. The console says the same with the retry named: *"It keeps
      asking, and until that works nothing here about them can be trusted"*. **No
      loop**: 8 kubectl lines and 9 stderr lines for the whole run
- [x] **the permission probe marks the right row, and only the right row.** Under that
      role `?` draws `ctrl-d  delete — you type the name to confirm **(no delete
      pods)**` — the *why not* clause on the key the login may not use
      ([D292](NOTES.md#d292--wiring-the-permission-probe-the-owner-the-dead-writes-gate-and-the-plural-three-existing-tables-refuse-to-give-2026-09-26)). `r` carries no clause and is absent from the footer for a
      **different** reason — the selected object is a Pod and `r` applies to
      deployments, statefulsets and daemonsets — so kind and permission are not
      conflated
- [x] **a kubeconfig with no usable user degrades to anonymous rather than crashing** —
      it draws, says it is not getting pods, and the sidebar's five groups collapse to
      one `could not read` row, which is [D296](NOTES.md#d296--a-refused-apis-is-two-surfaces-with-two-gates-and-the-row-outlives-the-sentence-2026-09-27)'s shape
- [ ] an expired token · a kubeconfig that can see one namespace only — each needs its
      own restricted credential; the `403` row above covers the degradation path
- [ ] the clock more than five minutes off the cluster's — needs the host clock
      moved, too invasive to do beside a running gate
- [ ] a **panic** leaves no credential in the backtrace and restores the terminal —
      needs a way to force a panic; not run

## H. The console — Alerts

**Run 2026-09-28 in `tmux` on the test host against `kind-k8rs`.**

- [x] the view draws against the broken cluster — 21 findings, `21 ● 4 ▲` on the
      sidebar, cards in plain language with a next step under each
- [x] `↑ ↓` move · `⏎` opens · `tab` changes panel
- [x] **invariant 7, measured rather than reasoned: `1` jiffy of `2000` over 20
      seconds untouched** — read off `/proc/<pid>/stat`, so 0.05% of one core. *No
      fixed FPS · block when idle* holds on the real binary
- [x] **the watch drives the frame with no keypress** — `49 restarts` on one card,
      `78 restarts` 35 s later, nothing pressed in between
- [x] **`/` filter** — the footer is *fully replaced*, not curated
      (`screens/widgets.md § 2b`): `filter:   ⏎ done  esc cancel`. Typing `sigterm`
      took the cards from 2 to 1, and **the footer's own label changed with what the
      press would reach** — `esc cancel` while the buffer is empty, `esc clear
      filter` once it has text. `esc` restored both cards
- [x] **`n` namespace, and it is a filter rather than a scope — drawn as one.**
      `namespace like:   ⏎ done  esc cancel`, then `esc clear namespace`. Committed,
      the pane says *"No problems match a namespace like "kube-system"."* — the
      filter and the result in one sentence — and the footer carries `esc clear
      namespace` beside the keys that came back. **The header shows no `ns:`
      segment, and that is right**: `ns:` means a scope the watch narrowed, which is
      `--namespace`'s (measured on stdout in § C). Two different facts, two
      different renderings. No finding
- [ ] the cursor stays on the same object across a watch update — needs a watched
      object to move under a held cursor; not run
- [ ] a pod name carrying control characters — needs a pod named to carry them,
      which is a cluster write of its own
- [ ] `X` switch cluster, a context that fails to connect, and `esc` out of it —
      partially seen (the dead-server run offers `X` on its footer, § G); the picker
      itself not driven

## I. The console — Analysis

- [ ] all seven panes open: capacity · certificates · drain safety · posture ·
      restarts · waste · versions
- [ ] a pane with nothing to say says so, and does not draw an empty frame
- [ ] `[` `]` move between panes, and the footer names the keys that work

## J. The console — the Resources browser

**Known broken as of 2026-09-28 and being fixed inside the Phase 13 close**
([D310](NOTES.md#d310--the-browsers-kind-pane-is-made-honest-rather-than-wired-and-that-reverses-the-freeze-on-three-top-layer-files-2026-09-28)) —
see F1 in § Findings. Re-run every row below after that fix lands.

- [ ] the five groups draw: workloads · network · storage · config · cluster
- [ ] **a kind row's count** — every mockup draws one, no code does (F5). Either
      the ruling built it or the mockups lost it; check which, and check the page
      and the screen now agree
- [ ] every group expands, and a CRD gets a row with no code written for it
      (invariant 12)
- [ ] `⏎` on a kind — the pane says honestly that it cannot be listed yet
- [ ] **`esc` comes back**, and the footer names it
- [ ] `tab` still gets out, and is not the only key that does
- [ ] discovery refused — the groups do not draw and one row says why
      ([D296](NOTES.md#d296--a-refused-apis-is-two-surfaces-with-two-gates-and-the-row-outlives-the-sentence-2026-09-27))

## K. The console — the detail tabs

- [ ] `l` `d` `y` from a selected object — each says plainly it is not built yet
      (F2 in § Findings), and `esc` closes each one
- [ ] `[` `]` move between the four tabs
- [ ] the container picker has nothing to pick, and says so rather than drawing empty

## L. Keys, footer, help

- [ ] `?` opens, `?` and `esc` both close, `q` still quits from under it
- [ ] **every key the help screen names either works or is marked not built yet** —
      this is the row F2 exists for, and it is the whole point of the section
- [ ] every key the footer names works, and no working key is missing from it
- [ ] under `--read-only`, the whole *Changing things* block is gone — not greyed
- [ ] a login that may not `create pods/eviction` gets a *why not* clause on the
      row it is for, and never on a row it is not
- [ ] nothing on screen is jargon a newcomer needs a glossary for (invariant 14)

## M. The write path, from the console

**Run 2026-09-28 in `tmux` against `kind-k8rs`, on throwaway `k8rs-probe2`/`probe3`
deployments the PM created — never on the cluster's broken fixtures.** Both deleted
after, 42 fixture pods intact.

- [x] **no selected object means no mutating key, proved by absence.** A *healthy*
      throwaway draws no card at all — the Alerts view shows findings — and the footer
      was `↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit`, with no
      `r`. Break the same object (a bad image) and `r restart` appears
- [x] the card reads in plain language: `● default/k8rs-probe3  ·  1 of 1 pods`, then
      *"Container image is not usable, so the container never started (ErrImagePull)"*,
      then the server's own error quoted whole
- [x] the committed filter is drawn at rest — `filter: "probe3"   esc clears it`
      (`screens/widgets.md § 2b`'s own section)
- [x] **`r` — every element of invariant 2 on one screen.** `┌ Restart
      default/k8rs-probe3 ─┐` names the object; the consequence in plain language
      carries what an operator needs (*"it can be a few, or all of them at once"*, *"A
      paused deployment will not start until you resume it."*); **`The cluster checked
      it first and accepted it.`** — the dry-run ran *before* the button was offered;
      the kubectl line; `[ ⏎ do it ]    [ esc cancel ]`; and the footer **fully
      replaced** with `⏎ do it  esc cancel`
- [x] **declining with `esc` reaches the audit log.** The dialog went, and **two lines
      were appended** — the attempt and its result. The security gate's *every
      attempt, success failure or refusal* row, measured on a refusal
- [x] **confirming with `⏎`: the command log strip holds the line whole** —
      `$ kubectl --context kind-k8rs rollout restart deployment/k8rs-probe3 -n default
      → done`. The dialog's own cut is cosmetic; the strip is where a reader copies
      from, and it is complete. Invariant 4's teaching device works
- [x] `resourceVersion` is `not sent`, which is [D228](NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05)'s
      ruling for absolute intent, and the audit pair records it
- [ ] `ctrl-d` delete from the console, and the typed name in the dialog — not run
- [ ] `s` withheld on every footer — not driven (`views::SCALE_IS_BUILT` is `false`,
      so there is nothing to press)
- [ ] `esc` on a confirmation whose check has not answered — needs a slow dry-run
- [ ] the object stopped existing between the keypress and the confirm — needs a race
- [ ] the refusal box, `esc` dismisses and `⏎` opens the object — needs a refused write
- [ ] `--read-only`: no key on this page is bound at all — the flag's refusal was
      measured headlessly (§ F); the console's footer under it was not read

## N. Security rows that only a running binary can answer

The `[auto]` rows of
[CLAUDE.md § Security gate](CLAUDE.md#security-gate--run-this-list-on-every-change-no-exceptions)
are `scripts/security-guard.py`'s and are not re-read here. These are the ones no
script can see.

- [x] **a Secret's value never reaches the YAML** — measured with a planted canary in
      § E: `password=SUPERSECRETCANARY123` read back through `--yaml` gives
      `password: <hidden — 20 bytes>`, and the canary appears **0 times** plaintext
      and **0 times** base64
- [x] **and it never reached the audit log either** — 0 plaintext, 0 base64, and the
      whole log holds no `password`/`token`/`secret`/`key`-shaped substring at all
- [x] **the only outbound connection is the API server in the kubeconfig** — `ss -tnp`
      against the running console's pid shows six sockets and every one goes to
      `127.0.0.1:6443`, the kind cluster's own API server. No other host, no telemetry
- [x] **TLS verification off in the kubeconfig is honoured *and shown*** — `⚠ TLS not
      verified` in the header (§ G)
- [x] **the audit log is mode `0600` in a `0700` directory**, and when it cannot be
      opened, writes stop and reads continue, with the policy stated (§ F, § G)
- [x] **an object name cannot escape the temp directory** —
      `--object "default/../../etc/passwd"` is refused on its shape before a path is
      built (§ D)
- [ ] no environment variable **value** appears on any surface — needs a pod carrying
      one and the detail tabs, which are not wired
- [ ] a Secret reveal on a surface that offers one — no surface offers it yet
- [ ] a `fieldValidation=Strict` rejection quoting the whole object in the audit line
      ([D217](NOTES.md#d217--strict-on-every-write-that-can-carry-it-and-the-422-that-hands-back-the-object-you-sent-2026-09-04))
      — only a read-modify-write can reach it, and the only one is v0.4's `edit`
- [ ] a 50MB annotation does not blow up the renderer — needs an object built to carry one
- [ ] `strings` the installed binary — done at the release and recorded in the phase's
      own gate; not re-run here

---

## Findings

One row per finding, newest last. **Where it went** is the column that matters: a
blocker is fixed in the turn that found it, everything else is a
[`backlog.md`](backlog.md) line waiting for a ruling, and neither is ever a box in
an open phase.

| # | What is wrong | Found | Blocker | Where it went |
|---|---|---|---|---|
| F1 | Opening a kind in the Resources browser is a dead end: the pane draws `reading the cluster…` for a fetch nothing issues, and `esc` does not come back | 2026-09-28, close review; **re-measured on the released binary**, 120×40 tmux, `csidrivers` unchanged after 20 s, footer down to `? all keys  q quit` | yes | **fix in flight** in the Phase 13 close, [D310](NOTES.md#d310--the-browsers-kind-pane-is-made-honest-rather-than-wired-and-that-reverses-the-freeze-on-three-top-layer-files-2026-09-28) |
| F2 | `?` lists `l logs`, `d describe` and `y view as YAML` under *always available* while the same body marks `s` *not built yet*; all three land on a fetch nothing issues | 2026-09-28, close review | yes | **fix in flight**, same turn, [D310](NOTES.md#d310--the-browsers-kind-pane-is-made-honest-rather-than-wired-and-that-reverses-the-freeze-on-three-top-layer-files-2026-09-28) |
| F3 | The sidebar cuts a kind's **front**, and five of `storage`'s eight rows are cut in its twenty columns: `…ributesclasses` `…ragecapacities` `…ntvolumeclaims` `…sistentvolumes` `…umeattachments`. `ui::front` keeps the tail deliberately — the front-cut alternative collapses `persistentvolumes` and `persistentvolumeclaims` — so neither cut is right and `screens/widgets.md § 7`'s *still reads as itself* is not true of `…ributesclasses` | 2026-09-28, this file's first pass | no | [`backlog.md`](backlog.md), for a screen ruling ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |
| F4 | `k8rs -h` answers `k8rs: -h: No such file or directory (os error 2)` — a flag read as a path. The same binary answers `-h is not a flag k8rs has` for `k8rs --once -h`; `-x` and `-v` behave like `-h`. The unknown-flag test is `arg.starts_with("--")`, so no single-dash word reaches it and the default door reads it as a file | 2026-09-28, row A of this file | yes | **fix in flight**, folded into D310's turn ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |

| F5 | Every sidebar mockup draws a per-kind count (`deployments  12`, `pods  84`) that no code draws — eight mockups across `resources.md`, `widgets.md`, `states.md`, against `screens/README.md`'s own *the code has to match them*. `ui.rs:3470` hands `NavItem::Kind` an empty badge under a comment asserting the opposite of the spec and citing no decision; no `§ Rules` on any page explains the count | 2026-09-28, reviewing the screen spec | no | [`backlog.md`](backlog.md), one ruling over the whole page ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |

| F6 | `k8rs --help` and `--version` answer *"is not a flag k8rs has"* on stderr with exit `2`. The usage is printed, so nobody is stranded — but the first command typed against a fresh `cargo install` is answered as an error, and `--version` is how a bug report names its version | 2026-09-28, row A of this file | no | [`backlog.md`](backlog.md) — it lands on invariant 10's *generated help* threshold, so it is a ruling ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |

| F7 | `k8rs <file holding `{}`>` prints `1 object no rule reads ((no kind))` — the `(no kind)` label (`main.rs:712`, deliberate) lands inside the `no rule reads (…)` fragment's own parentheses (`main.rs:1318`, also deliberate). Cosmetic, malformed-input path only, unambiguous to a reader | 2026-09-28, § B of this file | no | **judged, no action** — neither half is wrong and nesting them costs a reader nothing; recorded so the next pass does not re-investigate |

| F8 | `--once -n <namespace that does not exist>` answers `○ nothing is broken in <that name>`, exit `0` — an affirmative claim about a scope that is not there. Same silent-wrong-scope class the `--context` arms already refuse. Kubernetes returns an empty list rather than a `404`, so *not there* and *cannot see it* are the identical `0 pods` | 2026-09-28, § C of this file | no | [`backlog.md`](backlog.md), for a ruling — the alternative costs a `get namespaces` RBAC may refuse ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |

| F9 | `ops` takes no `--context` — it must be the first word on the line, so a headless operation can only reach the kubeconfig's **current** context, while the console can write to whichever context `X switch cluster` moved it to. `--read-only` got an explicit carve-out before the parse ([D230](NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05) ruling 3); `--context` did not, and no decision records the choice | 2026-09-28, § F of this file | no | [`backlog.md`](backlog.md), to be decided with the ruling already open there on `ops`'s status as a shipped subcommand ([D311](NOTES.md#d311--the-a-to-z-pass-gets-a-file-and-its-first-four-rows-found-a-binary-that-calls-a-flag-a-missing-file-2026-09-28)) |

| F10 | **k8rs reads a kubeconfig `kubectl` refuses.** An unquoted `n` as a cluster/user/context name is a *boolean* in YAML 1.1, which Go's parser follows — `kubectl` answers *"cannot unmarshal bool into Go struct field NamedCluster.clusters.name"* — while `serde_yaml_ng` follows YAML 1.2 and reads it as the string `n`. So k8rs connects and teaches `$ kubectl --context narrow …` lines **that cannot run for that user**, which is invariant 4's teaching device pointing at a command the reader's own `kubectl` rejects. Narrow (needs an unquoted `n`/`y`/`yes`/`no`/`on`/`off` name) | 2026-09-28, § G of this file | no | [`backlog.md`](backlog.md), for a ruling — matching Go's YAML 1.1 booleans is a parser decision, not a patch |

**§ A–H, M and N have been run in whole or part; § I, J, K and L have not.** F1 and F2 came out of the
Phase 13 close review; F1 was re-measured here against the released binary. F3 and
F4 came out of this file, F6 out of § A and F7 out of § B, and F5 out of reviewing the fix for F1 — which is worth
noting, because it is the only one no row below would have caught: the count is
missing from the screen, and a reader with no mockup beside them sees nothing
wrong. Every other row is unrun and says so.
