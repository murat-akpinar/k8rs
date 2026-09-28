# test.md — the A-to-Z pass over the shipped binary

Every feature of k8rs, run by hand against the real binary, and answered *works*
or *does not*. It exists because `just check` green and a published release are
two different claims: the gate proves the code does what its tests say, and
nothing in it has ever opened the console, pressed a key, or watched a pane
answer. This file is the other half.

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

- [ ] `k8rs tests/fixtures/crashloop.json` — a card, in plain language, no jargon
      left unexplained (invariant 14). Read it as someone who does not know
      `CrashLoopBackOff`
- [ ] `k8rs tests/fixtures/healthy.json` — says nothing is broken, and says it in
      a sentence rather than by printing nothing
- [ ] `k8rs tests/fixtures/oom.json` — *exceeded its memory limit*, not `OOMKilled`
- [ ] a fixture per rule family: `pending.json` `init.json` `image.json`
      `probe0.json` `hostpath.json` `nodes.json` `deployments.json`
      `csr-pending.json` — each draws the finding its name promises
- [ ] two files on one line — both are read, and the cards are not double-counted
- [ ] a file holding a **list** of objects, and a file holding **one** — both work
- [ ] `k8rs --analysis tests/fixtures/nodes.json` — the seven panes, and a pane
      with nothing to say says so
- [ ] a file that is not JSON · an empty file · a JSON file holding `{}` — three
      different sentences, none of them a panic
- [ ] a 0-byte file and a directory passed as a path
- [ ] exit codes: `0` for a report drawn, `2` for a refusal. Nothing else
- [ ] **the timestamps read as English** — *4 min ago*, and never a raw RFC3339

## C. Door 3 — `--once` against the cluster

- [ ] `k8rs --once --context kind-k8rs` — the same cards, off a live cluster,
      exit `0`
- [ ] `k8rs --once --analysis --context kind-k8rs` — seven panes, exit `0`
- [ ] `k8rs --once -n kube-system` — scoped, and the scope is *stated on the
      output* rather than silently applied
- [ ] `k8rs --once` against a cluster with nothing broken — the healthy sentence
- [ ] after `scripts/cluster.sh reset`, every broken pod the script plants shows up
- [ ] **the run has a deadline** — unplug the cluster (`--context` at a dead
      endpoint) and it names the failure and exits rather than hanging
- [ ] `--once` with a kubeconfig that cannot `get /apis` — *this kubeconfig may not
      `get /apis`*, naming the path
      ([D160](NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26))

## D. Door 4 — `--logs`

- [ ] `--logs --object <ns>/<pod>` — the log, control characters stripped
      (invariant 9)
- [ ] `--previous` on a pod that has crashed — the log from before the crash
- [ ] `--previous` on a pod that never crashed — says so, not an empty success
- [ ] `--container` on a multi-container pod · a container name that does not exist
- [ ] `--follow` — streams, and `ctrl-c` leaves the terminal usable
- [ ] `--object` with no namespace, against a kubeconfig that names none
- [ ] a pod name with `../` in it — no path escapes anything (security gate)
- [ ] an endless log line — bounded, not held whole in memory (security gate)

## E. Door 5 — `--describe` and `--yaml`

- [ ] `--describe --object <ns>/<pod>` — the object and what happened to it
- [ ] `--yaml --object <ns>/<pod>` — the manifest, `managedFields` gone
- [ ] `--kind deployment --object <ns>/<name>` — a kind other than Pod
- [ ] `--kind` with a kind the cluster does not serve — named, refused
- [ ] **a Secret through `--yaml`** — values do not appear (security gate). If they
      do, that is a blocker and it is fixed in the same turn
- [ ] `--describe` on an object that has just been deleted — *gone*, not a panic

## F. Doors 6 and 7 — `ops` and `ops may-i`

**`ops.rs` is never batched** ([CLAUDE.md § The cycle](CLAUDE.md#the-cycle--one-family-of-todomd-boxes-is-one-turn-of-it)
step 6), so each row here is read on its own, and any finding on this page is a
per-box fix and not a family one.

- [ ] `k8rs ops` with no operation — the usage lists `scale`, `restart`, `delete`
      and what each one asks for
- [ ] `ops restart deployment/<name> -n <ns>` — confirmation first, in plain
      language, naming the consequence; then the command log line and the audit line
- [ ] `ops delete pod/<name> -n <ns>` — **the name must be typed**, and a wrong
      typing refuses
- [ ] `ops scale deployment/<name> 3 -n <ns>` — the value is read, and no
      precondition is put on a field that moves
      ([D228](NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05))
- [ ] `--read-only ops restart …` — **structurally refused**, and the refusal says
      why rather than looking like a failure
- [ ] every operation declines or performs a `dryRun=All` **as its own box ruled**;
      `delete` is the one that declines
      ([D225](NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04))
- [ ] an operation on an object that does not exist · on a kind that cannot take it
      (`restart` a Pod) — both named, neither a panic
- [ ] **the audit log**: mode `0600`, append-only, and it carries the verb, the
      path, the resourceVersion sent, the dry-run verdict and the result
- [ ] the command log line is the kubectl a human would have typed — and k8rs does
      not run it (invariant 4)
- [ ] a `409` offers a re-read and never a blind overwrite
- [ ] `ops may-i get pods` · `may-i create pods.apps/x` · `may-i get
      pods/log --subresource log` — each answers, and a refusal is an answer
- [ ] `may-i` under `--read-only` — still reachable, because it writes nothing
      ([D230](NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05))
- [ ] **no bulk anything** — there is no way to point an operation at two objects

## G. Door 1 — the console, launch and the ways it can go wrong

Every row in tmux, read with `capture-pane`.

- [ ] no kubeconfig at all — a sentence a newcomer can act on, not a stack trace
- [ ] a kubeconfig naming a dead API server
- [ ] a kubeconfig whose token has expired
- [ ] a context the kubeconfig does not have
- [ ] `403` on the pod watch — one feature degrades, names the missing verb and
      resource, does not crash and does not retry in a loop
- [ ] `insecure-skip-tls-verify` in the kubeconfig — honoured **and shown in the
      header** (security gate; no script can see the header)
- [ ] the clock more than five minutes off the cluster's — both directions
- [ ] a kubeconfig that can see one namespace only
- [ ] the audit log cannot be opened — says so, and the run continues
- [ ] a terminal smaller than 80×24
- [ ] `ctrl-c` and `q` both leave the terminal usable — and so does a **panic**:
      no credential in the backtrace, terminal restored (invariant 8)
- [ ] `ctrl-z` then `fg` — suspends and comes back drawing

## H. The console — Alerts

- [ ] the view draws at all, against the broken cluster
- [ ] `↑ ↓ / j k` move · `⏎` opens · `tab` changes panel · `esc` comes back
- [ ] **idle CPU is 0%** — `top` on the host while nothing moves (invariant 7)
- [ ] break a pod by hand and watch the screen answer without a keypress
- [ ] `/` filter — narrows, `esc` clears, and every printable key is text while it
      has focus
- [ ] `n` namespace — scopes, and the header says so
- [ ] the cursor stays on the same object across a watch update
- [ ] a pod name carrying control characters does not rewrite the terminal
      (invariant 9)
- [ ] `X` switch cluster — the picker, a context that fails to connect, and `esc`
      out of it

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

- [ ] `r` restart — selected object, keypress, confirmation naming the consequence,
      dry-run, audit line. All five, in that order (invariant 2)
- [ ] `ctrl-d` delete — the typed name, and a wrong one refuses
- [ ] `s` — withheld, and `?` says not built yet rather than promising `(scale)`
- [ ] `esc` on a confirmation whose check has not answered — the one modal `esc`
      does not close ([D214](NOTES.md#d214--the-mutation-contract-four-lies-a-record-could-tell-and-the-three-operations-that-have-no-dry-run-2026-09-04))
- [ ] the object stopped existing between the keypress and the confirm — *gone*
- [ ] a refused write — the refusal box, `esc` dismisses, `⏎` opens the object
- [ ] the attempt reaches the audit log **whether it succeeded, failed or was
      refused**
- [ ] `--read-only`: no key on this page is bound at all

## N. Security rows that only a running binary can answer

The `[auto]` rows of
[CLAUDE.md § Security gate](CLAUDE.md#security-gate--run-this-list-on-every-change-no-exceptions)
are `scripts/security-guard.py`'s and are not re-read here. These are the ones no
script can see:

- [ ] no environment variable **value** appears anywhere on any surface
- [ ] a Secret needs an explicit reveal, and its value never reaches the command
      log, the audit log, or `y`
- [ ] a `fieldValidation=Strict` rejection hands back the whole object in
      `Status.message`, and that is what the audit line quotes — check what that
      means for a Secret
      ([D217](NOTES.md#d217--strict-on-every-write-that-can-carry-it-and-the-422-that-hands-back-the-object-you-sent-2026-09-04))
- [ ] a 50MB annotation does not blow up the renderer or get held whole
- [ ] `strings` the installed binary — no personal username, no credential shape
- [ ] the only outbound connection is the API server in the kubeconfig — watch it
      with `ss -tnp` while the console runs

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

**Four rows of § A have been run, and nothing else has.** F1 and F2 came out of the
Phase 13 close review; F1 was re-measured here against the released binary, and F3
and F4 came out of this file. Every other row is unrun and says so.
