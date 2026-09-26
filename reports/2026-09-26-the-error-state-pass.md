# The REQUIREMENTS error-state list, driven through the wired console

**2026-09-26 · `k8s-admin` · ephemeral measurement ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15))**

Subject: `todo.md` § Phase 12's *manual pass of the REQUIREMENTS error-state
list*, read against
[REQUIREMENTS § Error states](../REQUIREMENTS.md#error-states-all-were-undefined-all-happen-on-first-launch).
Every earlier report on these states measured them **headlessly**
(`reports/2026-09-05-every-operation-against-a-real-cluster.md`,
`reports/2026-09-05-resourceversion-and-409-on-the-wire.md`,
`reports/2026-09-12-the-rbac-pairs-the-refused-keys-name.md`,
`reports/2026-09-19-the-strip-and-the-connection-word.md`) or against
`TestBackend`. What is new here is that every line below came off a **pty**, from
`console()`, against a **live API server**.

## The bench

| | |
|---|---|
| host | the test host ([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)) |
| build | `cargo build --release` at `d83159f`, `Finished release profile in 5m 19s` |
| cluster | `K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 bash scripts/cluster.sh up` then `… break`, torn down with `… down` before this report was written (`kind get clusters` → `No kind clusters found.`) |
| server | `v1.36.1`, 1 control plane + 1 worker, `kubectl` client `v1.36.3` |
| terminal | 24 × 100 pty |

**How it was driven.** A scratch driver in `/tmp` on the host, deleted with the
cluster, importing `scripts/picker-test.py` by path and reusing its
`run_binary` / `piped` / `written` / `dead_port` / `repainted` / `escaped`
helpers rather than growing a second harness. Two things it added, both scratch:
a replay of the pty byte stream onto a 24 × 100 grid so a frame can be quoted by
row, and — for the watch items — a plain-HTTP relay in front of `kubectl proxy`
that can cut established connections while still accepting new ones, or rewrite
a watch request's `resourceVersion` to `1` so the **API server itself** answers
`410 Gone`. Nothing was added to `scripts/` or `justfile`.

Everything below is the screen as it came off the pty, with the renderer's
cursor-positioning escapes replayed and nothing else changed.

---

## 1. No kubeconfig, an unparseable one, and a `--context` the file does not hold

```
KUBECONFIG=<a path that does not exist> k8rs        # on a pty
KUBECONFIG=<a file holding invalid YAML> k8rs
KUBECONFIG=<the kind kubeconfig> k8rs --context not-in-the-file
```

```
exit code: 2   exited: True
alternate-screen-on=False alternate-screen-off=False escape-byte-in-text=False
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML

exit code: 2   exited: True
alternate-screen-on=False
k8rs: no cluster to watch — this kubeconfig has no such context — check the `current-context` line in the file, and any `--context` on the command line

exit code: 2   exited: True
alternate-screen-on=False
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
```

**Verdict: as required.** One sentence on stderr before the TUI, exit 2, and
`\x1b[?1049h` never emitted at all in any of the three.

## 2. Nothing listening on the API port at startup

One context, `server: https://127.0.0.1:<a port bound and released>`.

```
CPU over a quiet 2 s while retrying: 0 tick(s) (a spin is ~200)
the run ended only when q was pressed: exited=True code=0
anything written outside the alternate screen on the way out: ''
```

The first frame, and the frame 28 s later, are identical:

```
 nodes …                                            ctx: only-one · ⚠ disconnected, retrying · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS            │                                                                             │
│  RESOURCES         │                                                                             │
│   workloads        │                                                                             │
│   network          │                                                                             │
│   storage          │                                                                             │
│   config           │                                                                             │
│   cluster          │                   reading the cluster… 0 pods                               │
│  ANALYSIS          │                                                                             │
│   capacity         │                   Large clusters take a moment. Findings                    │
│   certificates     │                   appear as they are found — this list                      │
│   drain safety     │                   fills up, it does not wait.                               │
│   posture          │                                                                             │
...
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context only-one get statefulsets -A --watch                                         │
│ $ kubectl --context only-one get daemonsets -A --watch                                           │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ? all keys  q quit                                                                               │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Verdict: not a startup error, retried, never an exit, 0% CPU idle — as
[D167](../NOTES.md#d167--eight-faults-not-two-and-the-two-the-review-had-to-produce-2026-08-27)
corrected the requirement.** The body paragraph is finding **F4**.

## 3. The API server goes away under a running console, and comes back

`docker stop review-control-plane` 10 s into the run, `docker start` at 45 s.

Connected:

```
 nodes 2/2                                      k8rs                 ctx: kind-review · live · admin
│▸ ALERTS    21 ● 4 ▲│▸ ● default/broken-sigterm                                        1 min ago  │
```

While the API server is down:

```
 nodes 2/2                                       ctx: kind-review · ⚠ disconnected, retrying · admin
│▸ ALERTS    21 ● 4 ▲│  ▲ k8rs is not getting pods from this cluster: nothing usable came back     │
│  RESOURCES         │  when k8rs tried to `list` and `watch` pods. It keeps asking, and until     │
│   workloads        │  that works nothing here about them can be trusted                          │
│   storage          │▸ ● default/broken-sigterm                                        2 min ago  │
```

After it came back:

```
 nodes 2/2                                      k8rs                 ctx: kind-review · live · admin
│▸ ALERTS    22 ● 5 ▲│▸ ● default/broken-probe0                                         1 min ago  │
```

**Verdict: both directions hold.** The header word changes, the stale card stays
visible under a banner that says it cannot be trusted, the run never ends, and
the link returns to `live` with the list re-populated. Two absences, both
already in [`backlog.md`](../backlog.md) § *A stale vital keeps its count and
never says how old it is*: the vital reads `nodes 2/2` with no `(40s ago)`, and
`screens/states.md` § The connection dropped's *What you see below is from 40
seconds ago* is never drawn. This is the first time that entry has been
reproduced through the console rather than through `--once`/`--live`.

## 4a. The watch streams are cut while the API server stays healthy

The relay cut all six established connections 20 s in and kept listening. Every
request that crossed it, timestamped:

```
+   1.1s GET /api/v1/pods?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=5088
+   1.2s GET /api/v1/nodes?&watch=true&…&resourceVersion=5088
+   1.2s GET /apis/apps/v1/{deployments,statefulsets,daemonsets}?&watch=true&…&resourceVersion=5088
relay: cutting 6 established connection(s)
relay: still listening — the API server itself is fine
+  21.0s GET /api/v1/pods?&watch=true&…&resourceVersion=5088
+  21.0s GET /apis/apps/v1/deployments?&watch=true&…&resourceVersion=5088
+  21.0s GET /api/v1/nodes?&watch=true&…&resourceVersion=5088
+  21.5s GET /apis/apps/v1/daemonsets?&watch=true&…&resourceVersion=5088
+  21.6s GET /apis/apps/v1/statefulsets?&watch=true&…&resourceVersion=5088
+ 311.0s GET /api/v1/pods?&watch=true&…&resourceVersion=5636
+ 311.0s GET /apis/apps/v1/deployments?&watch=true&…&resourceVersion=5627
+ 311.0s GET /api/v1/nodes?&watch=true&…&resourceVersion=5636
+ 311.5s GET /apis/apps/v1/daemonsets?&watch=true&…&resourceVersion=5634
+ 311.6s GET /apis/apps/v1/statefulsets?&watch=true&…&resourceVersion=5636
```

All five watches were re-established **1.0 to 1.6 seconds** after the cut, from
the same `resourceVersion` — a resumed watch, no re-LIST (no `?&limit=500` at
`+21`). The header, read at four points afterwards:

```
[seconds after the cut]   ctx: plain · ⚠ disconnected, retrying · admin   ▲ … not getting pods …
[about 150 s after]       ctx: plain · ⚠ disconnected, retrying · admin   ▲ … not getting nodes …
[about 280 s after]       ctx: plain · ⚠ disconnected, retrying · admin   ▲ … not getting StatefulSets …
[about 340 s after]       ctx: plain · ⚠ disconnected, retrying · admin   ▲ … not getting StatefulSets …
```

The ALERTS badge moved (`22 ● 3 ▲` → `21 ● 4 ▲`) across the same window and the
card ages advanced, so pod events were arriving throughout. Finding **F1**.

## 4b. A real `410 Gone` on every watch

The relay rewrote each watch request's `resourceVersion` to `1`, so the API
server answered `410 Expired` itself.

```
+   1.1s GET /api/v1/pods?&watch=true&…&resourceVersion=5802   (rewritten to 1)
+   2.2s GET /api/v1/pods?&limit=500
+   2.3s GET /api/v1/pods?&watch=true&…&resourceVersion=5804   (rewritten to 1)
+   4.1s GET /api/v1/pods?&limit=500
+   8.3s GET /api/v1/pods?&limit=500
+  17.0s GET /api/v1/pods?&limit=500
+  39.6s GET /api/v1/pods?&limit=500
```

```
 nodes 2/2                                      k8rs   ctx: plain · ⚠ disconnected, retrying · admin
│▸ ALERTS    21 ● 4 ▲│  ▲ k8rs is not getting pods from this cluster: nothing usable came back     │
│  RESOURCES         │  when k8rs tried to `list` and `watch` pods. It keeps asking, and until     │
│   workloads        │  that works nothing here about them can be trusted                          │
```

**Verdict: the reconnect is visible and no data is silently stale.** The relist
gaps per kind grow `1.9 → 3.9 → 8.3 → 16.4 → ~22 s` — the standing backoff, not
a loop. Two notes: the sentence for a `410` is the network one (*nothing usable
came back*), which is `backlog.md`'s already-recorded consequence of `410`
falling to `Fault::Unanswered`; and under a *standing* 410 the LIST succeeds
every time, so the rows on screen are in fact fresh while the banner says they
cannot be trusted.

## 5. 403 on read

A `ServiceAccount` with one namespaced `Role` — `pods`, `deployments`,
`replicasets`: `get`/`list`/`watch` in `default`, nothing else — and a
kubeconfig holding its `TokenRequest` credential.

```
                                                k8rs       ctx: limited · ns: default · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS    20 ● 4 ▲│  ▲ k8rs is not getting nodes from this cluster: the role this kubeconfig    │
│  RESOURCES         │  uses needs to `list` and `watch` nodes. It keeps asking, and until that    │
│   workloads        │  works nothing here about them can be trusted                               │
│   storage          │▸ ● default/broken-probe0                                           19s ago  │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context limited get statefulsets -n default --watch                                  │
│ $ kubectl --context limited get daemonsets -n default --watch                                    │
```

```
CPU over a quiet 2 s with three watches refused: 0 tick(s)
```

**Verdict: as required.** The verb pair and the resource are named, the link
stays `live` (a refusal is not a connection state), the left vital is blank
rather than `0/0`, the app keeps running, and nothing retries in a loop. The
kubeconfig context's own `namespace:` is honoured — `ns: default` in the header
and `-n default` on every taught line — with no `--namespace` on the command
line.

**The Events-watch carve-out is not reachable today.** The relay's request log
shows the five watches k8rs opens and no `events` watch at all; rule 11 waits on
that watch in v0.5 (`CLAUDE.md` § Where to look). Nothing to disable, so nothing
to measure.

### The `nonResourceURL` refusal

`system:discovery`'s ClusterRoleBinding was pointed at a group nobody is in, and
restored afterwards.

```
kubectl get --raw /apis --kubeconfig <the limited one>
Error from server (Forbidden): forbidden: User "system:serviceaccount:default:limited" cannot get path "/apis"
```

The console runs on: Alerts is complete and correct, CPU 1 tick over 2 s. The
sidebar still draws `RESOURCES / workloads / network / storage / config /
cluster`, and no frame anywhere says the discovery call was refused. Finding
**F5**.

## 6. A credential the server stops accepting — a real `401`

The `ServiceAccount` behind the bound token was deleted 14 s into the run.

```
                                                ctx: limited · ns: default · ⚠ login expired · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS    20 ● 3 ▲│  ▲ k8rs is not getting nodes from this cluster: this cluster no longer      │
│  RESOURCES         │  accepts this login — this kubeconfig needs a new one. It keeps asking,     │
│   workloads        │  and until that works nothing here about them can be trusted                │
│   storage          │▸ ● default/broken-sigterm                                        1 min ago  │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  X switch cluster  / filter  ? all keys  q quit                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

**Verdict: a third state, as
[D19](../NOTES.md#d19--401-is-a-third-case-and-the-kubeconfig-can-run-a-program)
requires.** Distinct from both the 403 sentence above and the *disconnected,
retrying* one; `X switch cluster` is promoted onto the footer; the run
continues. No renewal command is named, which is right for a static token — this
kubeconfig runs no login program.

## 7. 403 on a write, through the wired dialog

`r` on a card whose owner is a Deployment, under the same limited credential.
The footer offered the key:

```
│ ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit                                         │
```

`r`:

```
│  RESOURCES         │┌ The cluster refused this ────────────────────────────┐g, and until that    │
│   network          ││  Nothing was changed.                                │                     │
│   config           ││  What the cluster sent back:                         │          4 min ago  │
│   cluster          ││    deployments.apps "broken-owned" is forbidden: User│ger                  │
│  ANALYSIS          ││    "system:serviceaccount:default:limited" cannot    │                     │
│   capacity         ││    patch resource "deployments" in API group "apps"  │1 (the               │
│   certificates     ││    in the namespace "default"                        │                     │
│   posture          ││  This is the check that runs before the real change —│ started it. The     │
│   restarts         ││  it stopped this one.                                │                     │
│                    ││                    [ esc dismiss ]                   │                     │
```

The audit log, `mode=0o600`:

```
… attempt · deployment/broken-owned · context limited · server https://127.0.0.1:6444 · namespace default · no uid was read · kubectl: kubectl --context limited rollout restart deployment/broken-owned -n default · call: PATCH /apis/apps/v1/namespaces/default/deployments/broken-owned · resourceVersion not sent
… result · attempt … · deployment/broken-owned · dry-run: the check was sent and did not pass · the change was never sent — the cluster would not allow it: deployments.apps "broken-owned" is forbidden: User "system:serviceaccount:default:limited" cannot patch resource "deployments" in API group "apps" in the namespace "default"
```

**Verdict: as required.** The dry-run caught it, the confirmation was never
armed, nothing was sent, the server's sentence is on screen and stays until
`esc`, and both the attempt and the refusal are in the audit log.

### The control: the same key where nothing is wrong

```
│   workloads      ┌ Restart default/broken-owned ───────────────────────────────┐                 │
│   storage        │  This asks Kubernetes to replace every copy of your app with│he               │
│   config         │  a new one. How many stop at the same time is a setting on  │                 │
│   cluster        │  this deployment — it can be a few, or all of them at once. │ten before that  │
│  ANALYSIS        │  A paused deployment will not start until you resume it.    │rted it. The     │
│   capacity       │  The cluster checked it first and accepted it.              │                 │
│   drain safety   │  $ kubectl --context… rollout restart deployment/…-owned -n…│                 │
│   restarts       │                [ ⏎ do it ]    [ esc cancel ]                │                 │
```

While the call was on the wire the header carried `· changing…`, the strip line
ended in the running mark, and the footer read `↑↓ move ⏎ open ? keys ·
changing`. Afterwards:

```
│ $ kubectl --context kind-review rollout restart deployment/broken-owned -n default   → done      │
```

```
… attempt · … · call: PATCH /apis/apps/v1/namespaces/default/deployments/broken-owned · resourceVersion not sent
… result · … · dry-run: the cluster checked it first and accepted it · the change was made
```

The elided command inside the dialog is what `screens/dialogs.md` draws (§ *the
protected head*, lines 308–309, 737), not a divergence.

## 8. Rejected admission — a real `ValidatingAdmissionPolicy`

```
kubectl apply -f <a ValidatingAdmissionPolicy + Binding: UPDATE on apps/deployments, expression false>
kubectl patch deployment broken-owned -n default --dry-run=server -p '{"spec":{"template":{"metadata":{"annotations":{"k":"v"}}}}}'
The deployments "broken-owned" is invalid: : ValidatingAdmissionPolicy 'k8rs-no-restarts' with binding 'k8rs-no-restarts' denied request: this cluster does not allow restarting deployments during a change freeze
```

`r` on the same card, admin credential:

```
│  RESOURCES         │┌ The cluster refused this ────────────────────────────┐            57s ago  │
│   network          ││  Nothing was changed.                                │ng panic: dial tcp   │
│   config           ││  What the cluster sent back:                         │written before that  │
│   cluster          ││    deployments.apps "broken-owned" is forbidden:     │ started it. The     │
│  ANALYSIS          ││    ValidatingAdmissionPolicy 'k8rs-no-restarts' with │                     │
│   capacity         ││    binding 'k8rs-no-restarts' denied request: this   │                     │
│   certificates     ││    cluster does not allow restarting deployments…    │                     │
│   posture          ││  This is the check that runs before the real change —│                     │
│   restarts         ││  it stopped this one.                                │                     │
│                    ││                    [ esc dismiss ]                   │                     │
```

```
… result · … · dry-run: the check was sent and did not pass · the change was never sent — the cluster would not accept the request k8rs made: deployments.apps "broken-owned" is forbidden: ValidatingAdmissionPolicy 'k8rs-no-restarts' with binding 'k8rs-no-restarts' denied request: this cluster does not allow restarting deployments during a change freeze
```

**Verdict: the dry-run caught a real admission denial and nothing was sent.**
The audit line separates *would not allow it* (403) from *would not accept the
request k8rs made* (422). The policy's message is **cut** on screen at
`restarting deployments…` — the message is 212 characters, 189 of them reached
four quoted rows, and `during a change freeze` did not — and is whole in the
audit log. Finding **F3**.

## 9. `409 Conflict` — which console operation can produce one, and it did

`scale` sends no precondition on purpose
([D228](../NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05)),
`edit` is not built, and `restart`'s audit line above reads `no uid was read`.
The one console operation that carries a precondition today is **`delete`**,
which sends `preconditions.uid`
([D235](../NOTES.md#d235--the-delete-that-removed-a-pod-nobody-had-seen-and-why-the-fix-costs-no-read-2026-09-05)).
So: `ctrl-d` on a Deployment card, and while the dialog was open,
`kubectl delete deployment broken-owned && kubectl create deployment
broken-owned --image=busybox:latest` out of band.

The dialog, with the name typed in:

```
│  RESOURCES       ┌ Delete default/broken-owned ────────────────────────────────┐                 │
│   network        │  This asks the cluster to remove the deployment and every   │                 │
│   storage        │  copy of the app it runs. k8rs has not read what may be     │                 │
│   config         │  attached to it, and something there may delay this or act  │                 │
│   cluster        │  first — left alone, nothing is left running.               │                 │
│  ANALYSIS        │  k8rs did not check this one with the cluster first.        │                 │
│   capacity       │  $ kubectl --context kin… delete deployment/broken-owned -n…│                 │
│   certificates   │  Type the deployment's name to confirm:                     │                 │
│   posture        │  │ broken-owned_                                         │  │                 │
│   versions       │                [ delete ]    [ esc cancel ]                 │                 │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ⏎ delete  esc cancel                                                                             │
```

Before the name was typed the footer read `type the name to enable  esc cancel`.
After `⏎`:

```
│   network          │┌ The object changed first ────────────────────────────┐                     │
│   config           ││  Nothing was changed.                                │                     │
│  ANALYSIS          ││  Something else changed this object while k8rs was   │                     │
│   capacity         ││  working on it — reading it again shows what it looks│                     │
│   certificates     ││  like now.                                           │                     │
│   restarts         ││                    [ esc dismiss ]                   │                     │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context kind-review delete deployment/broken-owned -n default   → rejected           │
```

```
… attempt · deployment/broken-owned · … · uid <the uid the card was read with> (a condition on the change — the cluster does not make it unless the object is this one) · kubectl: kubectl --context kind-review delete deployment/broken-owned -n default · call: DELETE /apis/apps/v1/namespaces/default/deployments/broken-owned · resourceVersion not sent
… result · … · dry-run: k8rs did not check this one with the cluster first · nothing was changed — the object had already been changed by something else, so look at it again before deciding whether you still want this change: Operation cannot be fulfilled on Deployment.apps "broken-owned": the UID in the precondition (<the uid the card was read with>) does not match the UID in record (<the uid the new object has>). The object might have been deleted and then recreated
```

**Verdict: nothing was overwritten and the next step is a re-read.** The
deployment that now holds the name was not touched. The strip's word is finding
**F2**.

## 10. Permissions checked before they are needed

`ctrl-d` under the limited credential, which may not delete. The footer offered
`r restart`; the dialog opened; the reader typed `broken-owned` in full; `⏎`:

```
│  RESOURCES         │┌ The cluster refused this ────────────────────────────┐g, and until that    │
│   config           ││  What the cluster sent back:                         │          1 min ago  │
│   cluster          ││    deployments.apps "broken-owned" is forbidden: User│tes is restarting    │
│  ANALYSIS          ││    "system:serviceaccount:default:limited" cannot    │                     │
│   capacity         ││    delete resource "deployments" in API group "apps" │ond · exit 0 (the    │
│   certificates     ││    in the namespace "default"                        │                     │
│   posture          ││  This was the real change, not a check.              │ CronJob work; if    │
│                    ││                    [ esc dismiss ]                   │                     │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context limited delete deployment/broken-owned -n default   → rejected               │
```

The `?` screen under that credential promised all three keys with no mark:

```
│  Changing things (each one asks first, and shows the command)                                     │
│    s       not built yet — there is no way yet to type a copy count                              │
│    r       restart, at its own pace       (rollout restart)                                      │
│    ctrl-d  delete — you type the name to confirm                                                 │
```

**Verdict: the requirement is not met, and the console region says why** — the
`may_i_in` probe is listed in `main.rs` § THE CONSOLE's *what is not wired yet*
as a box of its own, and `Refused::default` is what fills the slot meanwhile.
The footer literals that would carry it (`s no scale`, `r no restart`) exist and
are unreachable. Finding **F6**.

### What a one-second watch drop costs the two write keys

Same relay cut, on a run that had a Deployment card selected. With the link
`Live` the footer carries `r restart` (§ 7 above). After the cut:

```
 nodes 2/2                                      k8rs   ctx: plain · ⚠ disconnected, retrying · admin
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                                    │
```

```
│  Changing things (paused while disconnected, retrying)                                           │
│    r       restart, at its own pace       (rollout restart)                                      │
```

---

## Findings, ranked

### F1 — blocker. One transient watch drop leaves the header saying `⚠ disconnected, retrying` for the rest of the run, and takes both write keys with it

`main.rs:9213` `linked()` answers `Link::Lost` if **any** watch carries
`Fault::Unanswered`, and `k8s.rs:1560-1567` clears a watch's `failure` only on
`InitDone` for a LIST that watch started, or on an `Apply`/`Delete`. § 4a
measured that a cut connection is **resumed**, not re-listed — the same
`resourceVersion` at `+21.0s`, and again at `+311.0s` when the 290-second server
timeout expired, with no `?&limit=500` at either point. So for a kind whose
objects do not change, no clear point ever fires: the StatefulSets watch was
still flagged **340 s after** a cut it recovered from in 1.6 s, while pod events
flowed and the badge changed.

The false positive is the measurement itself: a healthy cluster, answering every
request, reported as disconnected. The cost is not only the word. `ui.rs:1713` makes `Link::Lost`
withhold every mutating key, and § 10's last block is that measured: with a
Deployment card selected, the footer carries `r restart` while the link is
`Live` and loses it after the cut, and `?` heads *Changing things* with
**(paused while disconnected, retrying)** — one answer for `s`, `r` and
`ctrl-d`, since `withheld` is what both the footer and Help read. So after one
wifi hiccup, VPN re-key or load-balancer reset an operator cannot restart
anything for the life of the process. A stable production cluster is exactly where StatefulSets and DaemonSets
do not change, so the quieter the cluster the longer the lie.
The store-wide field
[D145](../NOTES.md#d145--a-failure-that-clears-itself-is-a-failure-nobody-sees-and-the-drivers-six-choices-2026-08-22)
left behind had to be monotone because it could not say whose failure it held,
and per-watch identity in
[D162](../NOTES.md#d162--per-watch-identity-and-the-six-choices-the-reconnect-box-had-to-make-2026-08-26)
is what bought the clearing back — *"one blip would stand for the session, D145's
named cost"*, in `k8s.rs:1535`'s own words. `linked()` ORs the five faults again
one layer up, and the header spends exactly that.

### F2 — should-fix. A `409` and a `403` on a real call both read `→ rejected` on the strip, and the dialog beside them says something else

Measured in § 9: the dialog drew *The object changed first*, the audit line drew
*the object had already been changed by something else*, and the command-log
strip drew `→ rejected` — for one event.
[D213](../NOTES.md#d213--the-write-path-is-the-fifth-consumer-of-fault-and-it-cannot-see-the-two-answers-it-meets-most-2026-09-04)
made `Fault::Conflict` its own variant precisely because *a `409` is not a bad
request*. `main.rs:10059` maps every `ops::Outcome::Failed` to `"rejected"`
whatever the fault, and `views::Log::outcome`'s own doc names `refused` and
`login expired` as short forms — neither of which any console path can now
produce. § 10's 403 on a real delete printed `→ rejected` too. `Outcome::Changed`
/ `"changed first"` exists and is a different event (the watch saw the object
change while the dialog was open), so it is not the arm to reuse.

### F3 — later phase. *Verbatim* is not what the refusal box can do, and REQUIREMENTS says verbatim

§ 8: of a 212-character admission message, 189 characters reached the box and
`during a change freeze` did not. The bound is deliberate — `ui.rs:299`
`MODAL_ROWS = 13`, *"it is what bounds what the cluster sent back in a `Refused`
box"*, which is
[D217](../NOTES.md#d217--strict-on-every-write-that-can-carry-it-and-the-422-that-hands-back-the-object-you-sent-2026-09-04)'s
whole-object case — and, measured on a 100-column terminal, it does not widen
with the terminal either: the box keeps the width `screens/dialogs.md` draws. The
line that disagrees is `REQUIREMENTS.md:157`, *"a rejected write shows the API
server's message **verbatim**"*. The whole message is in the audit log, and
nothing on the box says where to find it. How often a real admission message
exceeds 189 characters is not something this run measured.

### F4 — later phase. At startup against an unreachable API, the body says the cluster is being read

§ 2: the header says `⚠ disconnected, retrying` and the body, two feet below it,
says *reading the cluster… 0 pods* and *Large clusters take a moment. Findings
appear as they are found — this list fills up, it does not wait.* A beginner
reads the sentence that promises a result. The code matches
`screens/states.md` § *Over a pane with nothing to show yet* — *"the pane draws
exactly what it would have anyway … the token's death reaches the screen through
the header"* — which was written about a **pane that lost its link mid-read**;
`REQUIREMENTS.md:165` asks for *a banner that says so* for a startup that never
connected, and `notes()` (`main.rs:9257`) keys on `unconnected`, which a dead
port does not set. The screen and the requirement, not the code, are what
disagree.

### F5 — later phase. A discovery refusal is invisible on the screen the reader is on

§ 5: `get /apis` refused, and the sidebar still draws all five RESOURCES groups
with nothing saying why they will be empty. The security gate's own sentence —
*"this kubeconfig may not `get /apis`"*
([D160](../NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26))
— reaches the headless drivers and no console frame. The pane that would carry it
is the browser's `Table` fetch, which `main.rs` § THE CONSOLE lists as not wired.

### F6 — later phase (a box exists). The typed-name delete still asks for the name from a reader who may not delete

§ 10 is [D23](../NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)'s
own scenario, measured: name typed in full, `⏎`, *then* told. Named here because
the box being closed is *the manual pass of the error-state list*, and this is
the one item of that list the console cannot currently satisfy.

### F7 — nit. The command-log strip never marks a watch that is failing

`screens/states.md` § The connection dropped draws
`$ kubectl get pods -A --watch   (reconnecting)` and § Your login expired draws
`→ login expired` on the manifest line. Through the console those lines are
appended with `views::Log::ran` at connect and never gain an outcome: § 3, § 4
and § 6 all show the strip unchanged while the header and the banner carry the
state. `views::Log::sent`/`outcome` support it and no caller uses it for a watch.

## What could not be proven, and why

- **The Events-watch 403 carve-out** (rule 11) — there is no Events watch to
  refuse; the relay's request log shows the five that exist. It becomes
  measurable when v0.5 adds it.
- **A `401` from an expiring credential plugin.** The 401 in § 6 is real but
  arrives by deleting the `ServiceAccount` behind a bound token. The
  `exec`-plugin shape — a token minted with an `expirationTimestamp`, kube
  re-running the plugin inside its 60 s window — needs a plugin script and a
  short-lived credential; `picker-test.py` already has the plugin machinery
  (`logging_in`) and this would extend it rather than start from nothing.
- **How long F1 lasts on a cluster that does churn every kind.** Measured to 340
  s on an idle one; the upper bound is *until an object of that kind changes*,
  which is unbounded, and I did not construct the case where a DaemonSet is
  edited to prove the clear.
- **F5 past the sidebar.** Opening a RESOURCES row would say whether the browser
  pane names the refusal; the `Table` fetch behind it is not wired, so the pane
  cannot answer either way yet.
- **Nothing was measured about the panic path or terminal restore** — that is
  the phase close's gate, not this box's.
