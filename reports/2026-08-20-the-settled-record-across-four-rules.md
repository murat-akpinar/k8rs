# 2026-08-20 — the settled record across rules 1, 2, 5 and 6

Second operator review of the same box. Measured against the **working tree** ("NEW",
uncommitted round 2) and against **`git show HEAD`** ("HEAD"), plus an ephemeral kind
cluster, `K8RS_CLUSTER=review`, one worker, brought up and torn down inside this run.
No fixture was produced; the two pods below were written for this run into a namespace
`probe` with no `demo=broken` label. Container IDs are redacted to `containerd://…`.
The first round's measurements are
`reports/2026-08-20-settled-and-the-last-run-on-record.md`.

```
$ K8RS_CLUSTER=review K8RS_WORKERS=1 ./scripts/cluster.sh up
node/review-control-plane condition met
node/review-worker condition met
API: https://127.0.0.1:6443   context: kind-review

$ kubectl --context kind-review version -o json | …
client v1.36.3 server v1.36.1
```

Four binaries: NEW is `cargo build --release` on the working tree; HEAD is `git archive
HEAD` unpacked into the scratchpad and built there; the **`-cmd`** pair of each is the
same source with one line added to `main.rs`'s `card()` so `Finding::kubectl_cmd` prints
under the action — the shipped driver does not print it (first round, § 6). No repo file
was edited.

## 1 — the ladder, live: one card where round 1 drew two

`retryladder`, rebuilt from round 1 § 4: pod `restartPolicy: Never`; container `retry`
with its own `Never` plus `{action: Restart, exitCodes: {operator: In, values: [3]}}`,
`sh -c` exiting `3` for four runs and `1` after; sibling `keeper` running `sleep 86400`.

```
$ kubectl --context kind-review -n probe get pod retryladder -o json | …
podPolicy Never phase Running
keeper: restarts=0 state=running     exit=-/-      last=-/-
retry:  restarts=4 state=terminated  exit=1/Error  last=3/Error
```

```
$ k8rs-new-cmd <live retryladder object>
1 pod · 0 nodes · 0 workloads

▲ probe/retryladder · 2 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 1 (the application's own error) · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — … The command below is what fetches it
  $ kubectl logs retryladder -c retry -n probe

1 warning

$ k8rs-head-cmd <same object>
▲ probe/retryladder · 3 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 3 · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — … The --previous flag below is what fetches it
  $ kubectl logs retryladder -c retry -n probe --previous

1 warning
```

Both commands run against that pod at that moment:

```
$ kubectl --context kind-review logs retryladder -c retry -n probe
this is run 5 speaking
(rc=0)

$ kubectl --context kind-review logs retryladder -c retry -n probe --previous
unable to retrieve container logs for containerd://…(rc=0)
```

Run 5 is the run that exited `1`. The count on the title (`4`) was produced by four
`exit 3` runs; the exit code on the evidence line is run 5's.

## 2 — the same two halves holding two different runs, under an `Always` policy

`walker`: pod `restartPolicy: Always`; container `walk` with its own `Always` plus
`{action: Restart, exitCodes: {operator: NotIn, values: [0]}}`, exiting `10+n` on run
`n`; sibling `keeper`. Caught on the first sampling pass:

```
$ kubectl --context kind-review -n probe get pod walker -o json | …   # 13:57:21
walk: restarts=2 state=terminated exit=13/Error last=12/Error
```

```
$ k8rs-new-cmd <that object>          # HEAD prints the same card and the same command
▲ probe/walker · 30s ago
  The last run on record failed — exit 12
  container walk · ran for under a second
  → read the last run's log — … The command below is what fetches it, using --previous
  $ kubectl logs walker -c walk -n probe --previous

1 warning

$ kubectl --context kind-review logs walker -c walk -n probe --previous
unable to retrieve container logs for containerd://…(rc=0)

$ kubectl --context kind-review logs walker -c walk -n probe
this is run 4 speaking
(rc=0)
```

How long the two halves hold different runs, sampled every 3s:

```
13:57:53 restarts=3 state=terminated exit=14 last=13
…  (16 consecutive samples, 13:57:53 → 13:58:40, all exit=14 last=13)
13:58:43 restarts=3 state=waiting     exit=-  last=14
13:58:46 restarts=4 state=terminated exit=15 last=14
…  (8 consecutive samples, 13:58:46 → 13:59:08, all exit=15 last=14)
```

24 of 25 samples over 75s carry a `state.terminated` one run newer than `lastState`.

## 3 — the whole committed corpus, HEAD vs NEW

51 fixtures, 30 drawn cards.

```
$ for f in tests/fixtures/*.json; do k8rs "$f"; done   # HEAD and NEW, diffed
$ diff … | grep -c '^<'   →  10
$ diff … | grep -c '^>'   →  10
```

Ten lines move with the shipped renderer: **9 action lines** and **1 title line**.

```
$ diff -u … | grep '^[-+]  [A-Z]'
-  The last run on record failed — exit 3
+  The last run on record failed — exit 1 (the application's own error)

$ diff -u … | grep -c '^[-+]  → '            18   (= 9 pairs)
$ diff -u … | grep -c '^+.*using --previous'  8
```

The ninth action line is `broken-neverrules`, which loses the flag reference. With the
`-cmd` renderer an eleventh line moves — `broken-neverrules`'s command,
`kubectl logs broken-neverrules -c retry -n default --previous` → the same line without
the flag.

`broken-neverrules` is the only fixture whose card changes substance.

## 4 — the clause against the flag, across the corpus

```
$ every card carrying ", using --previous", with the command printed under it
      2   $ kubectl logs broken-crashloop -c quitter -n default --previous
      1   $ kubectl logs broken-init -c migrate -n default --previous
      1   $ kubectl logs broken-notfound -c app -n default --previous
      1   $ kubectl logs broken-oom -c hog -n default --previous
      2   $ kubectl logs broken-owned-7bdb7645c8-tb9tn -c quitter -n default --previous
      1   $ kubectl logs broken-restarts10 -c flaky -n default --previous

$ cards where the clause and the flag disagree in either direction
mismatches: 0
```

## 5 — the clause's cost in wrapped lines

Greedy wrap at `ACTION_COLUMNS = 49`, the measure `wrapped_at` uses:

```
HEAD (flag in sentence):  177 chars, 4 lines   last line 29 cols
NEW without the clause:   169 chars, 4 lines   last line 24 cols
NEW with the clause:      187 chars, 4 lines   last line 42 cols
57-char alternative:      226 chars, 5 lines   last line 31 cols

len(", using --previous") = 18
len(", and its --previous flag is what makes it that run's log") = 57
```

Card heights over the 30 corpus cards, `1 + title + min(evidence,3) + action`, title
and evidence at 51 columns, action at 49:

```
max card height HEAD: 11   NEW: 11        (cap 12)
action-line histogram HEAD: {4:13, 2:8, 3:5, 5:4}
action-line histogram NEW:  {4:13, 2:8, 3:5, 5:4}
cards whose height changed: 1
  ▲ default/broken-neverrules   7 lines (1+1+1+4)  →  8 lines (1+2+1+4)
```

The one card that grew did so on its **title**, not its action: `exit 3` →
`exit 1 (the application's own error)`.

## 6 — plants off committed captures, HEAD vs NEW

Decoded plants written into the scratchpad, never into `tests/`; each names the fields
changed. `neverback` is pod `Never`, phase `Running`, two containers.

| plant | change | HEAD | NEW |
|---|---|---|---|
| P1 | `broke`: `state.terminated` → `137/OOMKilled`, `restartCount 0`, no `lastState`, `limits.memory 64Mi` | 1 card: rule 15 | **2 cards**: rule 2 (`limit 64Mi · exit 137`, *raise the limit*, `describe`) **and** rule 15 |
| P2 | P1 with `restartCount 5` and `137/OOMKilled` in `lastState` | rule 2 + rule 5, rule 5 `--previous` | rule 2 + rule 5, rule 5 plain `logs`, `ran for` off the settled run |
| P3b | `broke`: `state.terminated` → `137/ContainerStatusUnknown` (no stamps, no containerID), `lastState` `1/Error`, `restartCount 2` | `The last run on record failed — exit 1 (the application's own error)`, `--previous` | `Kubernetes did not record how the run it last saw ended — exit 137 (…)`, `describe`, **no age** |
| P6 | `broke`: `state.terminated` → `255/Unknown` with both stamps, `lastState` `1/Error`, `restartCount 2` | `The last run on record failed — exit 1 …`, `--previous` | `The last run on record has no exit code of its own — exit 255 (…)`, `describe`, age kept |
| P4a | pod → `OnFailure`, no container policy; `state.terminated` → `0/Completed`, `lastState` `1/Error`, `restartCount 1` | `The last run on record failed — exit 1 …` | `○ nothing is broken` |
| P4b | P4a with `restartCount 3` | rule 5 `, but something keeps killing it · exit 1 · --previous` | rule 5 `, and the last run on record finished cleanly · exit 0 · describe` |
| P5 | `gang.json` `trigger`: `state.terminated` → `3/Error` with stamps, `lastState` left at `137/RestartingAllContainers`, `restartCount 3` | rule 5, `, and the record names the pod's rule` | identical |
| P5b | `gang.json` both containers `restartCount 0`, `lastState` removed, `trigger` `state.terminated` → `3/Error` | rule 15, `This container has stopped and nothing is starting it again`, `exit 3` | identical |

`oom.json`'s rule 2 card and `oomserving.json`'s silence are unchanged; the only line
that moves on `oom.json` is rule 5's action gaining the clause.

## 7 — the screen cost of the P1 pair

```
 6 lines = 1 + 2 title + 1 evidence + 2 action   Container used more memory than it was allowed …
10 lines = 1 + 2 title + 3 evidence + 4 action   This container has stopped and nothing is starting …
total including the blank between them: 17
```

`screens/alerts.md` § The height: the body pane is **16 rows** at 80×24.

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]

$ kind get clusters
No kind clusters found.
```
