# 2026-08-16 — measurements taken for the operator review of the capture trip (D114)

Measured for the Phase 3 capture-trip review. **No cluster was raised** — the PM holds the only
one. Everything here is read off the landed working tree, off the four new captures, and off a
throwaway copy of `src/` in the agent scratchpad, which is where the two rule-13 runs were made.
Tree state: `git status` at 14:12 local — 60 modified files plus the four untracked fixtures;
`src/rules.rs` last written 13:56, `scripts/certs-test.sh` 14:03 and `scripts/make-certs.sh` 14:11
— i.e. two files moved during this review.

## 1 — the gate, on the landed tree

```
$ cargo fmt --all -- --check
FMT_OK
$ cargo clippy --locked --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.11s
$ cargo test --locked --all-targets
test result: ok. 222 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
$ just guards
security-guard: self-test passed — 17 planted violations, each seen red, and the clean tree green before and after
security-guard: token hygiene — 13 structs, 0 can hold a token — OK
verify-test: 45 predicates, each matched in its own state and refused in a neighbouring one
sanitize-test: single object, List and kube-system List — every planted secret removed …
certs-test: dates pinned at 2026-08-17 00:00:00Z (src/rules_tests.rs pins the same instant) — expiring 19 days (C1 warns), healthy 360 days (C1 silent), expired -8 days (C1 says expired); no key material
fixture-audit: 55 committed fixtures (51 parsed as JSON) — no annotations, no env values, no addresses; no key material in any framing …
```

## 2 — the four new captures, the fields their predicates turn on

```
$ jq -c '…{name,ready,started,restartCount,state,lastState}…' tests/fixtures/{probe0,reboot,neverrules,gang}.json
```

| capture | container | `state` | `restartCount` | `ready` | `lastState.terminated` |
|---|---|---|---|---|---|
| `probe0` | `app` | `running` since `10:10:42` | 13 | false | `0` / `Completed`, `10:10:09`→`10:10:41` |
| `reboot` | `app` | `running` since `10:12:25` | 3 | true | `255` / `Unknown`, `10:11:59`→`10:12:15`, `containerID` present |
| `neverrules` | `retry` | `terminated` `1` / `Error` | 1 | false | `3` / `Error` |
| `neverrules` | `keeper` | `running` | 0 | true | `{}` |
| `gang` | `trigger`, `bystander` | `running` since `09:44:35` / `09:44:38` | 3, 3 | true | `137` / `RestartingAllContainers`, **both stamps `null`**, no `containerID`, `message: "The container is removed because RestartAllContainers in place"` |

Each `cluster.sh` predicate run against its own capture:

```
$ for n in probe0 neverrules gang reboot; do jq -r "${P[$n]}" tests/fixtures/$n.json; done
probe0      true
neverrules  true
gang        true
reboot      true
$ jq -r '.status.containerStatuses[0].lastState.terminated | ((.finishedAt|fromdateiso8601)-(.startedAt|fromdateiso8601))' tests/fixtures/probe0.json
32
```

`NOTES § D114`'s table row for `broken-probe0` says **34s**; the committed record is **32s**, and
the card prints `ran for 32s`.

Sanitization of the four new files:

```
$ jq -r '…paths(scalars)… select(k8rs-|worker|BEGIN |eyJ|IPv4)…' tests/fixtures/{probe0,reboot,neverrules,gang}.json
spec.nodeName = k8rs-worker3 / k8rs-worker2 / k8rs-worker / k8rs-worker3
  managedFields:null annotations:null env:[null] …
```

Nothing else matched. `fixture-audit` above passes over all 55.

## 3 — which face each pod capture landed in, old and new

```
$ for f in tests/fixtures/*.json; do jq -r '… "\(.name)=\(.state|keys[0])\(reason)"' $f; done
$ git show HEAD:tests/fixtures/<n>.json | jq -r '…'
```

| capture | HEAD | landed |
|---|---|---|
| `crashloop` | `waiting:CrashLoopBackOff` rc=16 | `waiting:CrashLoopBackOff` rc=9 |
| `exit0` | `waiting:CrashLoopBackOff` rc=16 | `waiting:CrashLoopBackOff` rc=9 |
| `sigterm` | `waiting:CrashLoopBackOff` rc=27 | `waiting:CrashLoopBackOff` rc=13 |
| `oom` | `waiting:CrashLoopBackOff` rc=16 | **`terminated`** `137`/`OOMKilled` rc=10 |
| `notfound` | `waiting:CrashLoopBackOff` rc=16 | **`terminated`** `127`/`Error` rc=10 |
| `init` (`migrate`) | `waiting:CrashLoopBackOff` rc=16 | **`terminated`** `1`/`Error` rc=10 |
| `startup` (`slowboot`) | `running` rc=1, `lastState` `137` | `running` **rc=0, `lastState` absent** |
| `healthy` (`app`) | rc>0 with a clean `lastState` | **rc=0, `lastState` absent** |
| `healthy-sidecar` (`proxy`) | rc=1 with a clean `lastState` | **rc=0, `lastState` absent** |

Running containers that carry a previous run, over the whole corpus:

```
$ for f in tests/fixtures/*.json; do jq -r 'select(.state.running != null and .lastState.terminated != null) …' $f; done
gang/bystander rc=3 lastExit=137/RestartingAllContainers
gang/trigger   rc=3 lastExit=137/RestartingAllContainers
oomserving/app rc=1 lastExit=137/OOMKilled
probe0/app     rc=13 lastExit=0/Completed
reboot/app     rc=3 lastExit=255/Unknown
restarts10/flaky rc=10 lastExit=1/Error
restarts10serving/flaky rc=10 lastExit=1/Error
restarts/flaky rc=3 lastExit=1/Error
```

## 4 — rule 13 against a pod whose init container finished

Scratch copy of `src/` and `tests/fixtures/` in the agent scratchpad; one added test builds
`healthy.json` with `phase: Pending` and the `app` container put back to the kubelet's default
waiting state (`never_ran(pod, "app", "PodInitializing", None)`), leaving `migrate` in
`terminated` / `0` / `Completed`. Read at the pinned `now` (`2026-08-17T00:00:00Z`), so the
`PodScheduled` condition is ~14 h old and `NOT_READY_GRACE` has long passed.

**With the landed `nothing_else_to_point_at`:**

```
containers: [("migrate", Terminated(exit 0, "Completed"), Init), ("app", Waiting("PodInitializing"), Regular)]
nothing_else_to_point_at = false
REVIEW PROBE cards = []
```

**With only the added line `|| matches!(c.state, ContainerState::Terminated(_))` removed:**

```
nothing_else_to_point_at = true
▲ default/healthy · 14 hours ago
  This pod was given a machine to run on, but it has not been able to start
  container app · the machine has not said which step it is on — it still reports every container as starting up (PodInitializing) · this pod has its storage and its network, so the block is later — the image is still downloading, or the container could not be created
  → read the Events at the bottom of the describe output — that is where the machine says what it is still waiting for
  $ kubectl describe pod healthy -n default
```

**With the clause narrowed to a failed ending instead** —
`matches!(&c.state, ContainerState::Terminated(run) if ending(run) != Ending::Finished)`:

```
nothing_else_to_point_at = true
REVIEW PROBE cards = ["This pod was given a machine to run on, but it has not been able to start"]
$ cargo test          # same scratch copy, whole committed suite
test result: ok. 223 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

(223 = the committed 222 plus the probe. `the_whole_capture_through_the_rules_at_once` still
counts 24 cards, i.e. `broken-init` still draws no rule-13 card under the narrow clause.)

Why `PodInitializing` survives a finished init container, read off the kubelet source in
`…/scratchpad/kubelet_pods.go` (v1.36):

```
2499  defaultWaitingState := v1.ContainerState{Waiting: &v1.ContainerStateWaiting{Reason: ContainerCreating}}
2500  if hasInitContainers {
2501     defaultWaitingState = v1.ContainerState{Waiting: &v1.ContainerStateWaiting{Reason: PodInitializing}}
2119  apiPodStatus.ContainerStatuses = kl.convertToAPIContainerStatuses(… pod.Spec.Containers, …,
2125     len(pod.Spec.InitContainers) > 0, false, podRestarting)
```

`hasInitContainers` is `len(pod.Spec.InitContainers) > 0` and nothing about whether they have
finished.

## 5 — `cluster.sh` § `[init]` against a running init container

The predicate as landed, and its two siblings written in the same change:

```
[init]      ([.status.initContainerStatuses[]?|select(.state.waiting.reason=="CrashLoopBackOff" or .lastState.terminated.exitCode==1)]|length)>0
[crashloop] .status.containerStatuses[0] | .lastState.terminated.exitCode==1 and (.state.waiting.reason=="CrashLoopBackOff" or .state.terminated.exitCode==1) and …
[owned]     …select((.status.containerStatuses[0]|.lastState.terminated.exitCode==1 and (.state.waiting.reason=="CrashLoopBackOff" or .state.terminated.exitCode==1)) and …)
```

```
$ jq "$INIT" tests/fixtures/init.json
true
$ jq '.status.initContainerStatuses[0].state = {running:{startedAt:"2026-08-16T10:10:50Z"}}' tests/fixtures/init.json | jq "$INIT"
true
$ jq '.status.containerStatuses[0].state  = {running:{startedAt:"2026-08-16T10:10:50Z"}}' tests/fixtures/crashloop.json | jq "$CRASH"
false
```

`verify-test.sh` feeds `[init]` three objects; none is the running face:

```
$ grep -n '^check init ' scripts/verify-test.sh
3022:check init      match init      "broken-init"
3023:check init      miss  healthy   "the healthy pod, whose init container completed with exit 0"
3024:check init      miss  crashloop "a pod whose *app* container crashloops and which has no init container"
$ grep -n '^check owned ' scripts/verify-test.sh | grep 'up now'
3047:check owned     miss  restarts_owned "an owned pod that crashed three times and is up now — history is not a loop"
```

## 6 — `verify-test.sh`'s `probe0_pod` plant, after the recapture

`obj[probe0_pod]` is `obj[sigterm_pod]` with `lastState.terminated.startedAt` set to the literal
`2026-08-11T22:46:12Z`, and the comment beside it says the run then lasts **31s**.

```
$ jq -r '.status.containerStatuses[0].lastState.terminated | "startedAt=\(.startedAt) finishedAt=\(.finishedAt)"' tests/fixtures/sigterm.json
startedAt=2026-08-16T10:05:57Z finishedAt=2026-08-16T10:06:01Z
$ python3 -c "…2026-08-16T10:06:01Z minus 2026-08-11T22:46:12Z…"
probe0_pod plant duration = 4 days, 11:19:49
```

The `[probe0]` predicate asks `> 25`, so `verify-test.sh` stays green on a 386 389-second run.

## 7 — what the corpus prints, through `analyze`

```
$ cargo test the_whole_capture_through_the_rules_at_once -- --nocapture
17 critical, 7 warnings
```

The two cards this review turned on:

```
● default/broken-init · 13 hours ago
  Container has been restarted 10 times, but something keeps killing it
  init container migrate (the app starts only after this one finishes) · exit 1 (the application's own error) · ran for 2s · docker.io/library/busybox:latest
  → read that run's log — it holds the last thing written before the run ended, from the program or from the shell that started it
  $ kubectl logs broken-init -c migrate -n default --previous

▲ default/broken-neverrules · 14 hours ago
  The last run on record failed — exit 3
  container retry · ran for under a second
  → read that run's log — it holds the last thing written before the run ended, from the program or from the shell that started it
  $ kubectl logs broken-neverrules -c retry -n default --previous
```

`broken-neverrules`' `retry` is in `state.terminated` with `exitCode: 1` and
`lastState.terminated.exitCode: 3`; its `restartPolicy` is `Never` on both the pod and the
container, `restartCount` is `1`, and `stopped_for_good` returns `None` at its `restarts != 0`
guard. `rules.rs:3991` (rule 15's condition table) records `lastState` as "the run before this
one"; `rules.rs:3856` (rule 6's `Failed` arm) titles the same field "The last run on record".

`broken-init` draws one card, not the two the pre-trip capture drew: `crash_looping` returns at
`waiting(c)?` because `migrate` is in `state.terminated`, and rule 6 folds into rule 5 through
`one_card_per_action`.

## 8 — doc claims in `rules.rs` checked against the landed bytes

```
$ jq -r '… ready/started/restartCount/lastState …' tests/fixtures/{startup,restarts10,restarts10serving,exit0,sigterm,init,notfound}.json
startup/slowboot   ready=false started=false rc=0  last=none/-
restarts10/flaky   ready=false started=true  rc=10 last=1/Error
restarts10serving/flaky ready=true started=true rc=10 last=1/Error
exit0/batch        ready=false started=false rc=9  last=0/Completed
sigterm/app        ready=false started=false rc=13 last=143/Error
init/migrate       ready=false started=false rc=10 last=1/Error
notfound/app       ready=false started=false rc=10 last=127/Error
```

- `rules.rs:2823` — "`startup.json` is the captured `Regular` arm" of `killed_action`: the capture
  now carries `restartCount: 0` and no `lastState`.
- `rules.rs:3712` — "`startup.json` reaches the `137` arm" of `previous_run_failed`: same bytes.
- `src/rules_tests/pod.rs` plants that `137` in both tests and says so, citing D114.
- `rules.rs:3464-3465` (`restarts10` not serving / `restarts10serving` serving) still holds.
- `scripts/verify-test.sh:2666` — "`broken-reboot` is a manifest whose capture the trip did not
  reach"; `tests/fixtures/reboot.json` exists, is guarded in the `justfile`, and is in
  `CAPTURED_PODS`. File mtimes: `verify-test.sh` 12:26, `probe0.json` 13:11.
