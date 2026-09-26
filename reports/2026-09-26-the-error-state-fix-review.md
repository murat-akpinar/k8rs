# The D285 fix, driven through the console it was written for

**2026-09-26 · `k8s-admin` · ephemeral measurement ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15))**

Subject: the fix for the two blockers in
[reports/2026-09-26-the-error-state-pass.md](2026-09-26-the-error-state-pass.md),
ruled in `NOTES § D285` rulings 1 and 2 — `linked()`'s `Lost` predicate and
`outcome_word`'s `Failed` arm. What the fix's own evidence did not have is a run
of **the shape it exists for**: pods flowing while the quiet watches hold a stale
error. That is § 1 below. Everything here came off a pty against a live API
server.

## The bench

| | |
|---|---|
| tree | mirrored from the dev machine with the fix and the PM's uncommitted `NOTES.md`/`todo.md`/`backlog.md` edits; `sha256sum src/main.rs src/main_tests.rs` identical on both sides |
| build | `touch src/*.rs && cargo build --release` → `Finished release profile in 5m 16s`; `src/main.rs` 11:28:14, `target/release/k8rs` 11:33:30, so the measured binary is built **from** the fix |
| cluster | `K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6444 … up` + `… break`, v1.36.1, 1 control plane + 1 worker; torn down before this report (`kind get clusters` → `No kind clusters found.`) |
| terminal | 24 × 100 pty |

Driven by a scratch driver in `/tmp` on the host — `scripts/picker-test.py`'s
harness imported by path, plus the same plain-HTTP relay in front of
`kubectl proxy` as the first report, extended with one mode: it can **blackhole**
a connection (leave the socket open and drop every byte) instead of closing it.
Nothing was added to `scripts/` or the `justfile`.

---

## 1. Journey 4a against the fixed binary — the shape the fix exists for

The relay cut all six established connections at `+35.0 s` and kept listening.
Every watch request it carried:

```
relay: accepted #1..#6 at +1.0–1.3s
+   1.4s #3 GET /apis/apps/v1/statefulsets?&watch=true&…&resourceVersion=1649
+   1.4s #6 GET /apis/apps/v1/daemonsets?&watch=true&…&resourceVersion=1649
+   1.4s #2 GET /api/v1/nodes?&watch=true&…&resourceVersion=1649
+   1.4s #4 GET /apis/apps/v1/deployments?&watch=true&…&resourceVersion=1649
+   1.5s #5 GET /api/v1/pods?&watch=true&…&resourceVersion=1649
relay: cut 6 connection(s) at +35.0s, still listening
+  35.8s #7  GET /api/v1/nodes?&watch=true&…&resourceVersion=1649
+  35.9s #8  GET /apis/apps/v1/daemonsets?&watch=true&…&resourceVersion=1649
+  36.2s #9  GET /apis/apps/v1/statefulsets?&watch=true&…&resourceVersion=1649
+  36.2s #10 GET /api/v1/pods?&watch=true&…&resourceVersion=1938
+  36.4s #11 GET /apis/apps/v1/deployments?&watch=true&…&resourceVersion=1922
+ 325.8s #7  GET /api/v1/nodes?&watch=true&…&resourceVersion=2881
+ 326.3s #10 GET /api/v1/pods?&watch=true&…&resourceVersion=2894
```

Three watches resumed from the same `resourceVersion`; pods and deployments came
back at a **higher** one (`1938`, `1922`), which is a re-LIST and therefore an
`InitDone` clear. The header, footer and `?` afterwards:

```
[armed]     header: … ctx: plain · live · admin
            footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[+10 s]     header: … ctx: plain · live · admin
            banner: ▲ k8rs is not getting nodes from this cluster: nothing usable came back
            footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[+60 s]     header: … ctx: plain · live · admin      ALERTS 14 ● 11 ▲
            banner: ▲ k8rs is not getting nodes from this cluster: nothing usable came back
            footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[+160 s]    header: … ctx: plain · live · admin      ALERTS 15 ● 10 ▲
            banner: ▲ k8rs is not getting nodes from this cluster: nothing usable came back
            footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[+350 s]    header: … ctx: plain · live · admin
            banner: ▲ k8rs is not getting StatefulSets from this cluster: nothing usable came
            footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[?]         help:   Changing things (each one asks first, and shows the command)
```

**Verdict: the fix does what ruling 1 says.** Against the same cut that held
`⚠ disconnected, retrying` at `+150 s`, `+280 s` and `+340 s` before it, the
header reads `live` at `+10`, `+60`, `+160` and `+350 s`; the ALERTS badge moves
across the window, so events are arriving; `r restart` is never withheld; and `?`
no longer heads *Changing things* with **(paused while disconnected, retrying)**.

**The compensating record the ruling leans on is intact.** The pane keeps naming
the kind — `▲ k8rs is not getting nodes from this cluster: nothing usable came
back when k8rs tried to list and watch nodes. It keeps asking, and until that
works nothing here about them can be trusted` — and walks to `StatefulSets` as
the busier watches clear, while the header says `live`.

### The transition window was never painted, and that is worth saying

I looked for `⚠ disconnected, retrying` in the bytes that arrived across the cut
and **found none**: the header went from `live` to `live`. The five errors and
the five recoveries interleave inside ~800 ms, so *every* watch having a row at
one instant — which `dropped && !answering` requires — either never held or held
for less than the 100 ms coalescing window. So on a cut that recovers this fast
the reader sees no state change at all, rather than *Lost then live*. Nothing was
stale for a measurable time, so this is not a defect; it is not what the
expectation said either, and § 2 is the case that shows the word still appears
when it should.

## 2. A real outage, post-fix — the regression this fix could have caused

`docker stop review-control-plane` at `+12 s`, `docker start` at `+50 s`.

```
[armed]      header: … ctx: kind-review · live · admin
             footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[down]       header: … ctx: kind-review · ⚠ disconnected, retrying · admin
             banner: ▲ k8rs is not getting pods from this cluster: nothing usable came back
             footer: ↑↓ move  ⏎ open  / filter  ? all keys  q quit
[still down] header: … ctx: kind-review · ⚠ disconnected, retrying · admin
             banner: ▲ k8rs is not getting pods from this cluster: the role this kubeconfig
                       uses needs to `list` and `watch` pods
             footer: ↑↓ move  ⏎ open  / filter  ? all keys  q quit
[back]       header: … ctx: kind-review · live · admin      (no banner at all)
             footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[?]          help:   Changing things (each one asks first, and shows the command)
```

**Verdict: no regression.** A genuine outage still reads `⚠ disconnected,
retrying` and still withholds `r`, and the state clears completely — banner gone,
key back, no pause in Help — when the server returns.

**And one thing nobody has seen before**: while the control plane was coming back
up, the banner said *the role this kubeconfig uses needs to `list` and `watch`
pods*. Finding **G2**.

## 3. The two words F2 could not produce

**`→ refused`** — `ctrl-d` on a Deployment under a credential with no `delete`,
name typed in full, confirmed; a real `403` on the real call:

```
│ $ kubectl --context limited delete deployment/broken-owned -n default   → refused                │
```

with the box above it reading `The cluster refused this` / `What the cluster sent
back:` / `deployments.apps "broken-owned" is forbidden: User
"system:serviceaccount:default:limited" cannot delete resource "deployments" in
API group "apps" in the namespace "default"` / `This was the real change, not a
check.`

**`→ login expired`** — a credential that *may* patch, whose `ServiceAccount` was
deleted **after** the dry-run passed and **before** the confirmation, so the real
`PATCH` met a real `401`:

```
                                                 ctx: writer · ns: default · ⚠ login expired · admin
│▸ ALERTS    19 ● 5 ▲│  ▲ k8rs is not getting nodes from this cluster: this cluster no longer      │
│  RESOURCES         │  accepts this login — this kubeconfig needs a new one. It keeps asking,     │
│   workloads        │  and until that works nothing here about them can be trusted                │
│   network          │┌ The cluster refused this ────────────────────────────┐                     │
│   config           ││  Nothing was changed.                                │                     │
│  ANALYSIS          ││  What the cluster sent back:                         │                     │
│   capacity         ││    Unauthorized                                      │                     │
│   drain safety     ││  This was the real change, not a check.              │                     │
│   restarts         ││                    [ esc dismiss ]                   │                     │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context writer rollout restart deployment/broken-owned -n default   → login expired  │
```

```
… attempt · deployment/broken-owned · context writer · … · call: PATCH /apis/apps/v1/namespaces/default/deployments/broken-owned · resourceVersion not sent
… result · … · dry-run: the cluster checked it first and accepted it · nothing was changed — the login k8rs was using had run out: Unauthorized
```

**Verdict: both words reach the strip off a real server answer.** The dialog
title for the 401 is still `The cluster refused this`, which is what
`screens/dialogs.md` state 2 draws for any refusal of a real call — observation
**G3**, not a divergence.

## 4. Reading the change

`WATCHED` (`main.rs:9207`), `linked()` (`:9237`), `outcome_word()` (`:10059`) and
`the_connection_word_says_disconnected_only_when_no_watch_answers`
(`main_tests.rs:15437`) read against the five shapes an operator meets.

| shape | rows in `Store::troubles` | `dropped` | `answering` | word |
|---|---|---|---|---|
| healthy | none | false | true | `live` |
| scoped run, healthy (§ 5 of the first report) | `nodes`, `statefulsets`, `daemonsets` = `Refused` | false | true (pods, deployments absent) | `live` |
| scoped run, really disconnected | all five | true | false | `⚠ disconnected, retrying` |
| the cut, pods flowing again (§ 1) | the quiet kinds only | true | true | `live` |
| all five down (dead port) | all five | true | false | `⚠ disconnected, retrying` |

**The universe is right.** `Store::troubles` (`k8s.rs:2160-2194`) is built from
exactly the five kinds `WATCHED` names, in that order, and filters on
`failure.is_some() || *ended || *unfinished` (`:2197`), so a healthy watch has no
row — which is what makes *absence* the only available evidence of delivery.

**`WATCHED` is the fourth place these five kinds are spelled, and the pin covers
the one join that matters.** The others are `Store`'s five typed fields, the
array inside `troubles()` (`:2160`) and the one inside `still_listing()`
(`:2306`). The pin compares `WATCHED` against what `troubles()` actually reports
on a real `Store` put wholly into trouble by `stop_waiting()`, so drift between
those two — a kind dropped, added or reordered — fails. What no test covers is a
**sixth watch added to `Store` and to the merge but not to `troubles()`**: it
would be invisible to `WATCHED` and to every other reader of `troubles()` alike,
so that residual is upstream of this change and older than it.

**The pin can fail.** Proven on a copy of the tree under `$HOME` with its own
`CARGO_TARGET_DIR`, one element of `WATCHED` replaced by a kind `troubles()`
never reports:

```
cd ~/k8rs-pin && sed 's/ObjectKind::DaemonSet,$/ObjectKind::ReplicaSet,/' over WATCHED's last row
CARGO_TARGET_DIR=$HOME/.cache/k8rs-pin-target cargo test --locked \
    the_connection_word_says_disconnected_only_when_no_watch_answers

test tests::the_connection_word_says_disconnected_only_when_no_watch_answers ... FAILED
panicked at src/main_tests.rs:15472:5:
assertion `left == right` failed: `k8s::Store::troubles` no longer reports the watches `WATCHED` names
  left: [Pod, Node, Deployment, StatefulSet, DaemonSet]
 right: [Pod, Node, Deployment, StatefulSet, ReplicaSet]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1571 filtered out
EXIT=101
```

## 5. A watch that stops delivering without erroring — measured both ways

Neither predicate can see one, and this is where a `live` header sits over stale
cards. Same relay, blackholing instead of closing.

**One watch blackholed, the other four cut.** `#1` carried the pods watch; from
`+35 s` its socket stayed open and every byte was dropped. The relay log shows
statefulsets, nodes, daemonsets and deployments reconnecting at `+36 s` and **no
further pods request at any point** — no error, no retry, nothing.

```
relay: #1 carries the pods watch — blackholed, socket left open
relay: cut 5 connection(s) at +35.0s, still listening
+  36.1s #7  GET /apis/apps/v1/statefulsets?&watch=true&…
+  36.2s #8  GET /api/v1/nodes?&watch=true&…
+  36.2s #9  GET /apis/apps/v1/daemonsets?&watch=true&…
+  36.4s #10 GET /apis/apps/v1/deployments?&watch=true&…
```

```
[+12 s]  header: … ctx: plain · live · admin     ALERTS 22 ● 3 ▲
         banner: ▲ k8rs is not getting nodes from this cluster: nothing usable came back
[+90 s]  header: … ctx: plain · live · admin     ALERTS 22 ● 3 ▲
         banner: ▲ k8rs is not getting Deployments from this cluster: nothing usable came
[+200 s] header: … ctx: plain · live · admin     ALERTS 22 ● 3 ▲
         banner: ▲ k8rs is not getting StatefulSets from this cluster: nothing usable came
         footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[?]      help:   Changing things (each one asks first, and shows the command)
```

The badge is frozen at `22 ● 3 ▲` across all three readings; § 1's run over the
same window read `11 ● 13 ▲`, `14 ● 11 ▲`, `15 ● 10 ▲`. The card in the `+200 s`
frame still reads `7 restarts`. So the pods pane is genuinely stale, the header
says `live`, and the only marker on screen names **StatefulSets**, a kind the
reader is not looking at.

**Every watch blackholed at once**, no socket closed:

```
relay: blackholed all 6 connection(s) — every socket left open
```

```
[+12 s]  header: … ctx: plain · live · admin     ALERTS 22 ● 3 ▲      (no banner at all)
[+120 s] header: … ctx: plain · live · admin     ALERTS 22 ● 3 ▲      (no banner at all)
         footer: ↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit
[?]      help:   Changing things (each one asks first, and shows the command)
```

Nothing has arrived for 120 s, every watch is silent, and there is no marker
anywhere on the screen. Finding **G1**.

---

## Findings, ranked

### G1 — should-fix, and **not** caused by this fix: absence of a row is read as delivery, and a wedged watch has no row

`answering` is *some `WATCHED` kind has no row in `Store::troubles`*
(`main.rs:9279`). A watch whose connection stops delivering **without erroring**
produces no row at all — measured in § 5, twice — so it counts as answering.
`k8s.rs` § WHAT A THROTTLE LOOKS LIKE already writes down why that state exists —
its own citations, not a reading I repeated this turn: `read_timeout` unset,
`set_tcp_keepalive` never called, so *"a connection that dies without a FIN or an
RST"* is never reported. That is the commonest watch failure on a laptop — a
suspend/resume, a VPN re-key, a NAT table flush, an idle-timeout on a load
balancer that drops rather than resets.

**The split matters for whose defect it is.** With *every* watch wedged there are
no rows, so the old `any(fault == Unanswered)` answered `Live` too: unchanged by
this fix, and unfixable by any predicate over `troubles()`. With *one* wedged and
the others erroring, the old code said `⚠ disconnected, retrying` — wrong about
the cluster, which was reachable, but it happened to mark the stale pane; the new
predicate says `live`. So the fix removed an accidental marker from a case it did
not create.

**This is the dangerous direction and it is not the residual D285 records.** That
one is a silent cluster reading `disconnected` — stale header, safe side. This is
a live-looking header over data that stopped arriving, which is
`REQUIREMENTS.md:167-168`'s *never silently freeze on stale state*. The cheapest door
is a client `read_timeout` above the 290 s watch timeout, which turns the wedge
into an ordinary error and needs no new field on `Watch` and no clock — but it is
a `k8s.rs` change and a PM ruling, not something to fix inside `linked()`, and it
is outside what the ruling I was reviewing decided.

### G2 — later phase: while the API server is restarting, the banner tells the operator their Role is wrong

§ 2, in one frame: the header says `⚠ disconnected, retrying` and the banner says
*the role this kubeconfig uses needs to `list` and `watch` pods*. A kube-apiserver
that is coming back up really does answer `403` before its authorizers are ready,
so the classification is faithful to the wire and the *sentence* is still an
errand to the wrong place — the reader's `Role` is fine. The two sentences in that
frame contradict each other, which is also what makes it detectable: a `Refused`
row while the link is `Lost` is not an RBAC problem. Every rolling control plane —
`kubeadm upgrade`, cert rotation, a managed cluster's maintenance window — draws
it.

### G3 — nit, and it matches the written screen: a 401 on the real call is titled *The cluster refused this* and quotes `Unauthorized`

§ 3. `screens/dialogs.md` state 2 rules the title for any refusal of a real call,
and the header and banner in the same frame carry the login sentence, so nothing
here is wrong. Named because D285 ruling 2 moved the strip's word and the box's
title stayed — if the vocabulary is ever widened, this is the other half.

## D285's two corrected sentences, read against the code

Both are now true, checked line by line:

- *"`Store::troubles` **filters** (`k8s.rs:2197`: `failure.is_some() || *ended ||
  *unfinished`) and its own doc line says empty when all five are healthy"* —
  `:2197` is exactly that filter, and `:2113` reads *"Every watch that is not
  delivering, and why — empty when all five are healthy"*.
- *"`Fault::Unfinished` is itself `standing` (`k8s.rs:1050`, on the `true` side
  beside `Refused` and `Conflict`)"* — `:1050` is `| Fault::Unfinished => true,`,
  with `Refused` at `:1044` and `Conflict` at `:1049`.

`Fault::standing` is absent from `linked()`'s predicate, as the corrected ruling
says it should be.

## What I could not prove

- **That `r` still *acts* 350 s after the cut**, as opposed to being drawn. The
  footer offered it and `?` did not pause it, and `wanting()` gates on the stored
  `Console::offer` — the value that was drawn — so a drawn key is a live key by
  construction; and the key was pressed successfully on this binary twice in § 3.
  It was not pressed in the `+350 s` frame itself.
- **The `ended`-with-no-`failure` shape.** `Trouble::fault()` answers `None`
  there, so five finished streams give five rows, `dropped` false, and the word
  is `live` — the same answer the old code gave. It is a second door into G1's
  state and I did not construct it; it needs a server that closes every watch
  body cleanly and forever.
- **How long G1's mixed shape lasts in the field.** Measured to 200 s and still
  silent; the bound is whenever something makes the wedged socket fail, which
  with keepalive off and no read timeout is not bounded by anything k8rs sets.
