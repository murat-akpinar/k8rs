# The permission probe, read from the operator's chair

**2026-09-26 · `k8s-admin` · operator review, step 6 — blocking**

Subject: Phase 13 box 1, `ops::may_i_in` wired into the console — `src/main.rs`
`refusals` / `permitted` / `remember` / `Console::{client,permits,wondering}` /
the `pump` probe arm, read together with `src/ops.rs` § MAY I, `views::Refused`,
`views::App::footer`, `ui::offered`, `ui::withheld` and `ui::key_map`
([D292](../NOTES.md#d292--wiring-the-permission-probe-the-owner-the-dead-writes-gate-and-the-plural-three-existing-tables-refuse-to-give-2026-09-26),
[D261](../NOTES.md#d261--the-refused-keys-round-a-permission-that-is-two-questions-and-was-counted-as-one-a-reason-that-did-not-fit-the-line-it-was-promised-to-and-a-row-rewritten-by-arithmetic-another-box-would-have-moved-2026-09-12)
rulings 8–11,
[D229](../NOTES.md#d229--the-four-rulings-mayi-could-not-be-briefed-without-and-the-boxs-arithmetic-that-went-stale-under-it-2026-09-05)
ruling 4).

## What this is and is not

**No cluster was started and nothing ran on the test host** — `tester` held the
mirror for `just check` in parallel. This is a read-and-reason review with one
exception: the authorizer questions in § 1 were settled against **upstream
Kubernetes source fetched over HTTP**, which is a measurement of the definition
and not of a cluster. Every claim below is labelled *read*, *fetched* or
*reasoned*, and each unsettled one names the command that would settle it.

What was read, whole: `src/main.rs` 5694–5810 (`Kind`, `KINDS`, `known_kind`),
7415–7461, 7571–7598, 8014–8024, 8320–8332, 8420–8440, 8971–9105, 9236–9300,
9320–9436, 10213–10221, 10319–10356 · `src/ops.rs` 2863–3269 (§ MAY I),
2381–2482 (`restart`), 2725–2840 (`delete`), 1707–1965 (`scale`) ·
`src/views.rs` 2286–2460 (`Refused`, `refuses`), 2560–2705 (`Offer`,
`Offer::act`, `offers`), 3080–3215 (`App::footer`) · `src/ui.rs` 456–560
(`key_map`), 1540–1730 (`offered`, `addressed`, `withheld`) ·
`src/main_tests.rs` and `src/ui_tests.rs` diffs whole · `screens/help.md`
§ *When a key is refused* · `reports/2026-09-26-the-error-state-pass.md` § 5,
§ 10, F6.

---

## 1. The three authorizer questions D261 ruling 11 left open

**Fetched, 2026-09-26.** Three upstream files, at `master`:

```
curl -sL https://raw.githubusercontent.com/kubernetes/kubernetes/master/staging/src/k8s.io/apiserver/plugin/pkg/authorizer/webhook/webhook.go
curl -sL https://raw.githubusercontent.com/kubernetes/kubernetes/master/plugin/pkg/auth/authorizer/node/node_authorizer.go
curl -sL https://raw.githubusercontent.com/kubernetes/kubernetes/master/pkg/kubeapiserver/authorizer/reload.go
curl -sL https://raw.githubusercontent.com/kubernetes/kubernetes/master/staging/src/k8s.io/apiserver/pkg/authorization/union/union.go
```

### 1a. A Webhook authorizer always sets `incomplete`, so ruling 11's feared case is not reachable through it

`WebhookAuthorizer.RulesFor`, verbatim and unconditional:

```go
func (w *WebhookAuthorizer) RulesFor(ctx context.Context, user user.Info, namespace string) ([]authorizer.ResourceRuleInfo, []authorizer.NonResourceRuleInfo, bool, error) {
	var (
		resourceRules    []authorizer.ResourceRuleInfo
		nonResourceRules []authorizer.NonResourceRuleInfo
	)
	incomplete := true
	return resourceRules, nonResourceRules, incomplete, fmt.Errorf("webhook authorizer does not support user rule resolution")
}
```

`reload.go`'s `newForConfig` appends the webhook to **both** lists —
`authorizers = append(...)` at 194 and `ruleResolvers = append(ruleResolvers,
webhookAuthorizer)` at 198 — and `union.NewRuleResolvers(...).RulesFor` ORs the
flag (`if incomplete { incompleteStatus = true }`).

So on any cluster with a webhook authorizer in the chain, **every**
`SelfSubjectRulesReview` comes back `incomplete: true` with that error string.
`ops::may_i_in` (`src/ops.rs` 3103) sets `unsure` from
`status.incomplete || status.evaluation_error.is_some()`, `Permits::may`
(3046–3066) turns every non-matching question into `Verdict::CouldNotTell`, and
`views::refuses` (`src/views.rs` 2455) only matches `Some(Verdict::No)`.
**No key is ever marked.** Ruling 11's under-report cannot happen this way.

### 1b. The cost of 1a: on a webhook cluster the box is inert, and nothing says so

Same chain, other direction. `kubectl auth can-i --list` on such a cluster prints
*"the list may be incomplete: webhook authorizer does not support user rule
resolution"*, and AKS with **Entra ID / Azure RBAC for Kubernetes
Authorization** is documented as installing exactly that webhook
([learn.microsoft.com/en-us/azure/aks/manage-azure-rbac](https://learn.microsoft.com/en-us/azure/aks/manage-azure-rbac),
[learn.microsoft.com/en-us/azure/aks/entra-id-authorization](https://learn.microsoft.com/en-us/azure/aks/entra-id-authorization),
[Azure/AKS#4743](https://github.com/Azure/AKS/issues/4743) is the warning in the
wild). On those clusters this box draws the pre-box screen — every key unmarked —
and the reader cannot tell that from *you may do everything*. F6's journey is
intact there.

Settles with one command per cluster, no k8rs build needed:
`kubectl auth can-i --list -n default 2>&1 | head -3` — a "may be incomplete"
warning means k8rs will mark nothing on that cluster.

### 1c. The reachable ruling-11 shape is a different authorizer, and it is on every cluster

`reload.go` 110–115, before the mode switch and outside it:

```go
	// Add SystemPrivilegedGroup as an authorizing group
	superuserAuthorizer := authorizerfactory.NewPrivilegedGroups(user.SystemPrivilegedGroup)
	authorizers = append(authorizers, union.NamedAuthorizer{
		AuthorizerName: "system-privileged-group.authorizer.kubernetes.io",
		Authorizer:     superuserAuthorizer,
	})
```

Appended to `authorizers` and **not** to `ruleResolvers`. So a `system:masters`
identity is authorized for everything by an authorizer that contributes no rules
and sets no `incomplete` — ruling 11's exact shape, through the superuser path
rather than the webhook. In practice the `cluster-admin` ClusterRoleBinding to
`system:masters` is bootstrap policy and is reconciled on apiserver start, so
RBAC lists `*` on `*` for that group anyway; the gap opens only in the window
where that binding is absent. **Reachable in principle, very unlikely in the
wild** — reasoned from the source above, not measured.

### 1d. The Node authorizer flags `incomplete` only for a node identity, so the box does work on a plain `Node,RBAC` cluster

```go
	if _, isNode := r.identifier.NodeIdentity(user); isNode {
		return nil, nil, true, fmt.Errorf("node authorizer does not support user rule resolution")
	}
	return nil, nil, false, nil
```

D230's measurement (*kind's Node authorizer sets `incomplete: true`*) was taken
**under a node identity**. For a human or a ServiceAccount on the same
`Node,RBAC` cluster the Node authorizer returns `(nil, nil, false, nil)`, so
`incomplete: false` and `Verdict::No` is reachable — **provided nothing else in
that cluster's chain is a webhook**, which is § 1b's question and is unverified
for EKS, GKE and OpenShift. That is what makes the
Phase 12 credential of `reports/2026-09-26-the-error-state-pass.md` § 5 — a
namespaced Role, `get`/`list`/`watch` on pods, deployments, replicasets — able to
produce the marks this box draws.

---

## 2. The verbs, checked against what the operations call

**Read**, `src/ops.rs`:

| Key | Constant (`src/views.rs`) | What `ops` calls | Verdict |
|---|---|---|---|
| `s` | `SCALE_VERBS = ["get","patch"]` (2332) | `api.get_scale` (1872) then `api.patch_scale` (1961), both on `<plural>/scale` | correct |
| `r` | `RESTART_VERBS = ["patch"]` (2336) | `api.patch(name, …)` on the base resource (2475), nothing read first | correct |
| `ctrl-d` | `DELETE_VERBS = ["delete"]` (2340) | `api.delete(name, params)` (2834) | correct |

`restart` reads nothing before it patches — the patch body is built from the
clock (`restart_patch(&clock())`), and the `dryRun=All` preflight is the same
`PATCH` verb. `delete` is `checkable: false`, so it sends nothing first; its
`DeleteParams` preconditions carry the uid and need no extra verb. Neither names
a verb the operation does not need — `PRIOR-ART § B4`'s
[k9s#4144](https://github.com/derailed/k9s/issues/4144) shape, and it is clear.

`refusals` (9383–9385) asks the `/scale` pair with `subresource: Some("scale")`
and the other two with `None`, which is the distinction `ops::addresses`
(3235–3249) implements. That matcher accepts exactly `*`, `<resource>/<sub>` and
`*/<sub>` — **fetched**: RBAC's own `ResourceMatches` accepts the same three and
no more, so the matcher is faithful in both directions.

---

## 3. Asking about the named object

`Asking::name = Some(&card.owner.name)` (9372). Traced against the key router:
`wanting` (10327–10356) builds its `Wanted` from `selected(console, cards)` —
**the same function `refusals` calls** (10213) — and takes
`card.owner.{name,namespace,kind}`. So the object the probe asks about is the
object the key acts on, field for field. There is no case where a
`resourceNames`-limited Role dims a key usable on a *different* object, because
the question is re-asked per frame for whatever is under the cursor.

`ops::about` (3263) is `listed.is_empty() || name.is_none_or(|n| covers(listed, n))`
and `Permits::may` folds rules with `.any()`, so the union of a name-limited rule
and a general one still answers `Yes`. `covers` treats `*` in `resourceNames` as
*all*, which over-reports — the direction D229 ruling 4 requires.

The name never leaves the process: `may_i_in` sends only
`spec.namespace`, and `Permits::may` compares in memory. No API string reaches a
path segment or a shell here (invariant 9, the gate's *no API string is ever
interpolated into a shell*).

---

## 4. Fail open, traced

Every path that could dim a key on something that is not a clean `Verdict::No`:

| Path | Where | Result |
|---|---|---|
| nothing selected | 9354 | `Refused::default()`, nothing asked |
| kind outside `KINDS` (Job, CronJob, `ObjectKind::Other`) | 9358 via `ui::addressed` → `("","")` | default |
| cluster-scoped (Node, `namespace: None`) | 9361 | default, **no review on the wire** |
| namespace not answered yet — including the first frame of every run | 9364 | default + `wondering` |
| review refused (403), half answered, `incomplete`, dead socket | `ops::may_i_in` 3103 → `unsure` | `CouldNotTell`, `refuses` ignores it |
| review never returns | `permitted` 9406 → `None` | nothing remembered, nothing marked |
| answer filed under the wrong namespace | key is the `String` moved into `permitted`; `Permits.namespace` is the same value; `Permits::may` re-checks it | impossible |
| a cache entry from the previous connection | `switched` 8429–8431 clears `client`, `permits`, `wondering`; the in-flight `probing` is a **`pump` local** (8981) and `switched` runs in `console`'s loop *after* `pump` returns, so the future is dropped with the frame | closed |
| dead-writes run, lost link, expired login, skewed clock | `ui::withheld` → `offered` answers `Offer::Move` → the gate at 9286 | default, no review |

`views::Refused`'s fields are private and `Refused::of` is the only constructor,
so no caller can set a mark from anything but a `Verdict::No`. **I found no path
that dims a key on a non-`No`.** What I did find is the opposite failure — cases
where no key can be marked *at all* and the screen cannot say so (F1, F2, and the
Node in § 6) — and one divergence that is latent until the browser box lands (F5).

---

## 5. The 10 s deadline and the 256-entry cache

`PROBE_DEADLINE` (7449) is 10 s, matching `k8s::REPORT_FETCH` (`src/k8s.rs`
2523) and `k8s::SERVING_PROBE` (8602) — **read**, both are
`from_secs(10)`. Sensible: a `SelfSubjectRulesReview` is an in-memory RBAC walk,
so 10 s is two orders of magnitude of headroom and the slot never holds the
screen.

`PERMITS_KEPT` (7461) is 256, evicted by `permits.clear()` before the insert
(9428–9430, read before the insert). Rate, **reasoned** from the loop: one review at a time
(`probing.is_none()`, 8997), the next is fired from the draw arm, and the draw
arm only wants a namespace the cursor is actually on — so the request rate is
bounded by cursor movement, not by frame rate. One `POST
selfsubjectrulesreviews` per namespace visited, no debounce.

Settles with:
`kubectl get --raw /metrics | grep 'apiserver_request_total.*selfsubjectrulesreviews'`
before and after holding `↓` down a full Alerts list on a cluster with many
namespaces.

---

## 6. The two questions that want a plain answer, answered plainly

### The Node's `ctrl-d`, and whether it makes the done-when false

**Read**: a Node card has `owner.namespace == None`
(`src/rules.rs` 86–89 — `None` means cluster-scoped), so `refusals` returns at
9361 and no review goes out. D261 ruling 10 is working as designed. So **the exact
journey this box was opened for — `ctrl-d`, the name typed in full, and only then
refused — is still reachable on a Node**, and F2 says it is reachable on a whole
class of cluster for every kind.

**My answer: acceptable for this box, and the done-when needs one clause rather
than a re-open.** Three reasons, in the order I weigh them:

1. It is a different call. A cluster-scoped question is `ops::may_i`
   (`SelfSubjectAccessReview`), one round trip per question, and D261 ruling 10
   ruled it out of this box before the box was written. Wiring it here would be
   a second probe with a second cache and a second failure mode, in a box that
   already added three fields to `Console`.
2. The journey is already recorded as open by D261 ruling 7, which names the
   cheap place: the delete confirmation dialog, **before** the name field, which
   is `screens/dialogs.md`'s box. That closes it for a Node *and* for a webhook
   cluster *and* for a login with no `create selfsubjectrulesreviews` — one
   guard covering all three of F1, F2 and this, where a per-kind probe covers
   one.
3. A `delete node/<name>` is the rarest of the three keys by a wide margin, and
   the reader reaching for it is not the beginner invariant 14 is written for.

What is **not** acceptable is a done-when that reads *permissions are checked
before they are needed* with no qualifier, because that sentence is false on a
Node, false on a webhook cluster and false for a login that may not ask. The
honest wording is *off a namespaced rules review*, and the three cases above are
what the box does not cover.

### `no restart` beside `r`

**Right, and I would defend it against a change.** Three columns, no glyph to
learn, and it reads as a fact about the key rather than jargon — D261 ruling 1's
arithmetic (footer at 71 of 76 with both marks) is what forces the reason behind
`?`, and that trade is the one the browser row and the evidence line already
make. A reader who has never opened `?` still learns from `r no restart` that the
key is off for them, which is the whole job of the mark.

The clause it points at is where I push back, and that is F3: `(rollout restart —
patch deployments)` teaches *what to ask for* and never says *you do not have
it*. On `r` that is survivable, because the footer already said `no`. On `ctrl-d`
there is no footer and nothing else — and `ctrl-d` is the key the box exists for.

---

## Findings, ranked

**No blockers.** The diagnosis is true, the verbs are the ones the operations
call, and I could construct no false positive — no path dims a key on anything
but a clean `Verdict::No` (§ 2, § 3, § 4). Eleven findings, five should-fix and
six nits. **One of them, F1, is this box's own docs sync and belongs in the same
commit** (CLAUDE.md § *A structural change is not done until the docs match
it*); every other should-fix is a later box, a ruling, or a comment.

### F1 — should-fix. `docs/security.md`'s `k8rs-readonly` gap went live with this box, and the file still says it arrives with a later one

`docs/security.md` 265–277 carries the `create selfsubjectrulesreviews` /
`selfsubjectaccessreviews` rule in the **admin** role, and 280–296 says plainly
that `k8rs-readonly` does not carry it, that this is *a gap rather than a
policy*, and that **"It arrives with the browser row that reads it."** This box
is a second reader and it landed first.

The concrete journey, on
[D160](../NOTES.md#d160--the-capability-probe-the-seven-group-strings-a-cluster-confirmed-and-the-two-prose-claims-it-took-away-2026-08-26)'s
cluster — the one that dropped the default `system:basic-user` binding — for a
login bound only to `k8rs-readonly`, running `k8rs` **without** `--read-only`
(nothing tells k8rs the *role* is read-only; `ui::withheld` reads the flag, not
the cluster): `Offer::Act` is reachable, the probe is refused for want of
`create selfsubjectrulesreviews`, `may_i_in` returns `CouldNotTell`, no key is
marked, `ctrl-d` opens the box, the name is typed in full, and the cluster
refuses. **F6 verbatim, for the role the docs recommend to exactly the reader
this box was built for.** CLAUDE.md § *A structural change is not done until the
docs match it* — this is the box's own docs sync, not a later one's, and it is
the one finding here I would hold the commit for. It is two edits in one file:
the rule onto `k8rs-readonly`, and the sentence that promises it to a box that no
longer owns it.

The fix is the role, not the code: `k8rs-readonly` gains the rule, and the
paragraph stops promising it to the browser box. Whether it is *sufficient* there
is the measurement D229's own *what is owed to the documented roles* paragraph
already asks for and which has not been re-taken since 2026-09-05:

```
kubectl --context <role-only-context> auth can-i create selfsubjectrulesreviews.authorization.k8s.io
K8RS_CLUSTER=review ... k8rs --context <role-only-context>   # and read the ? screen
```

### F2 — should-fix (ruling, not code). The probe is inert on a whole class of real cluster and the screen cannot say so

§ 1a and § 1b. Any cluster with a webhook authorizer — **AKS with Azure RBAC /
Entra ID authorization is one checkbox away from this, and it is on by default on
AKS Automatic** — returns `incomplete: true` on every review, so every question
is `CouldNotTell` and no key is ever marked. That is the correct fail-open
direction and nothing is wrong on screen; what is wrong is that *the box's
done-when is unmet there and nothing distinguishes it from a permitted login*.
There is no surface for it either: `views::Refused` deliberately drops
`CouldNotTell`'s sentence (`src/views.rs` 2305), the footer has no columns,
and `ui::help`'s permission clauses only append.

I am **not** asking for code. `views.rs` and `ui.rs` are frozen, and a fifth
reason on the `?` heading would cost a string and a row. What this needs is a
ruling recorded against this report, and a line in `docs/` so an AKS operator
filing *"k8rs never dims anything"* gets an answer instead of a bug hunt.

Settles per provider with the § 1b command. **GKE, EKS-with-access-entries and
OpenShift are unverified** — I did not have a cluster of any of them, and the
one-line check above is the whole test.

### F3 — should-fix. `ctrl-d`'s refusal is signalled only by an unlabelled parenthesis, on the one key the box exists for

`ui::key_map` 534–541 appends `(delete deployments)` to the `ctrl-d` row, and
`screens/help.md`'s mockup (500–507) is that row — 506 is the line.
`ctrl-d` is on **neither list footer** ([D259](../NOTES.md#d259--the-footer-is-a-curated-subset-with-one-pair-that-never-gives-way-the-help-screen-is-the-frame-wearing-a-title-rather-than-a-box-drawn-inside-it-and-a-gate-verified-against-a-substituted-tree-is-not-verified-2026-09-10),
D261 ruling 7), so that parenthesis is the **only** place a refused delete is
ever drawn. A reader who has never seen the unrefused row cannot tell it from the
jargon parenthesis on the row above it — `r`'s `(rollout restart)` is the same
shape and means *here is the kubectl term*, not *you may not*. `r` at least
carries `no` on the footer; `ctrl-d` carries nothing anywhere.

`screens/help.md` 589–594 cites
`states.md`'s **"Missing permission: list nodes"** as the pattern being followed,
and the row does not follow it: it names the grant and never the fact that it is
missing.

**There is room, and only on this row.** Counted off the two literals, against
`screens/help.md`'s own 78-column body ceiling and its asserted 70/76:

| Row | as landed | with `no ` | with `needs ` |
|---|---|---|---|
| `ctrl-d`, `deployments` | 70 | 73 | 76 |
| `ctrl-d`, `statefulsets` | 71 | 74 | 77 |
| `r`, `deployments` | 76 | 79 — over | 82 — over |
| `r`, `statefulsets` | 77 | 80 — over | 83 — over |

So `ctrl-d` can take a word and `r` cannot, which is an asymmetry somebody has to
rule on rather than a fix I can name. `screens/` is `tui-designer`'s and the
literal is `ui.rs`'s, which is frozen — so this is a later box either way, but it
should be written down now, while the reason is in front of somebody.

### F4 — should-fix. The timeout re-asks forever on a busy cluster, and the comment says it does not

`src/main.rs` 9093–9098, the probe arm's own comment: *"no frame is owed — the
retry is the next frame something else owes, **which is what keeps a cluster that
will not answer from being asked in a loop**"*.

**Read against `Owing` (7641–7669) that is true only on an idle cluster.** A
watch event calls `owing.changed()` → a frame in ≤100 ms, and the cluster you are
looking at in k8rs is by definition the one with things going wrong in it. So:
probe out → 10 s timeout → slot freed → the next frame (≈100 ms later) re-reads
`console.wondering`, which is still set, and fires again. **One `POST
selfsubjectrulesreviews` every 10 s, for the life of the session**, for a
namespace whose review never returns. The security gate's *never retries in a
loop* is paced, not satisfied.

Likelihood is low — a hanging `selfsubjectrulesreviews` specifically, while the
watches stay live, is a narrow failure — so the finding is the **comment**, which
claims a property the code does not have. If the behaviour is to be kept, the
comment should say *re-asked no more often than the deadline*, which is true and
is a different sentence.

### F5 — should-fix. When the browser's `Table` lands, two views will disagree about the same object

`refusals` → `selected` (10214) returns `None` unless
`console.app.view == View::Alerts`. `ui::offered` (1605–1637) *can* answer
`Offer::Act` from `View::Resources`, and today does not only because `drawn`
hands it `browser: &UNOPENED` (9245), which is `Pane::Loading`. The moment the
browser fetch box fills that pane, a Deployment selected in **Alerts** draws `r
no restart` and the **same Deployment** selected in the browser draws `r
restart` — no test fails, nothing panics.

That is `PRIOR-ART § G2`'s *read-only enforced per view is a hole per view* and
[D103](../NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)'s
*two rules reading one container and disagreeing*, one box early. It belongs in
the browser box's premise, not in this diff — but it has to be written down,
because the reviewer of *that* box will be shown the browser and not this
function.

The `refusals` doc (9330) gives the reason as *"a browser row (whose `Table` is
not wired, so `selected` answers `None` there)"*, which conflates two facts:
`selected` answers `None` because of the **view**, and `Offer::Act` is
unreachable because of the **pane**. Only the second goes away with the box.

### F6 — nit. `refusals` asks the permission question for operations the selected kind does not support

9383–9385 ask all three unconditionally. So a **DaemonSet** card yields
`refused.scale() == true` under an ordinary `patch daemonsets` Role — a DaemonSet
has no `/scale` subresource at all — and a **Pod** card yields
`refused.restart() == true`. Both contradict D261 ruling 8 and
`views::Refused`'s own doc (2290–2296): *"A key the kind cannot act on is
**withheld** … and never *refused*."*

Neither is drawn: `App::footer` only reaches a `no` through `Offer::Act {
scalable: true, .. }` / `{ restartable: true }`, and `Offer::act` (2680–2682)
gates both on `ops::scalable` / `ops::restartable`. So the stored bit is wrong and
the screen is right, **by a guard in another file**. The `refusals` doc names the
wrong guard for the future: it says *"even though `s` is never marked
(`views::SCALE_IS_BUILT`)"*, and when that constant flips — which its own doc
promises is one word — what keeps the DaemonSet's wrong bit off the footer is
`ops::scalable`, not the constant.

Laziest correct fix, one predicate each, and it makes the code say what the
ruling says:

```rust
let scalable = ops::scalable(singular).is_ok_and(|served| served.group == group);
let scale = views::Refused::SCALE_VERBS
    .map(|verb| scalable.then(|| permits.may(&asking(verb, Some("scale")))));
// … and `scale.each_ref().map(Option::as_ref)` at the `Refused::of` call, which is
// already the shape that function takes — `[None, None]` is *not asked*, and
// `views::refuses` over an all-`None` array is already `false`.
```

— or leave it and correct the doc to name `Offer::act`'s kind gate as the guard.

### F7 — nit. Above 256 namespaces a mark appears, disappears and comes back

`remember` (9428–9430) empties the whole map, so on a cluster where more than
`PERMITS_KEPT` distinct namespaces are visited in one session — Alerts is
cluster-wide and its cards span namespaces, which is D261 ruling 10's own premise
— the 257th answer drops the other 256. Scrolling back up a long list then
re-draws `r restart` for an object that read `r no restart` a moment earlier,
until the re-probe lands. Fail open, and pressing in that window gets the real
refusal from the dry-run — but *a mark that flickers* is a worse screen than a
mark that never appears, and the constant's own doc calls the cost *"one review
re-asked for the namespace the cursor is on"*, which is true of one namespace and
not of the 256 that were thrown away with it.

256 is otherwise a reasonable number: I have not seen a cluster where an operator
walks more than that many namespaces in one sitting. If the eviction is ever
revisited, keeping the *newest N* rather than clearing is the same few lines.

### F8 — nit. The cache never expires, so a permission granted mid-session is never seen

`permits` is emptied only by `switched` (8430). A reconnect after
`Link::Lost` reuses the same session (`connected` is called at 8070 and 8440
only), so nothing clears it. An admin who grants the reader `delete deployments`
while k8rs is open leaves `ctrl-d` reading refused on the `?` screen for the rest
of the run. Fail open on the press — `may_mutate` does not read `Refused`, so the
key still works — so what is stale is the screen and not the behaviour. Worth a
line somewhere rather than a TTL.

### F9 — nit. `probing`'s lifetime is what makes `switched`'s clear complete, and nothing says so

The cross-cluster hole I went looking for is closed, and it is closed by
something invisible: `probing` is a **local in `pump`** (8981), `switched` runs
in `console`'s loop after `pump` has returned, so an in-flight review for the old
cluster's client is dropped rather than filed into the new cluster's cache under a
namespace name both clusters happen to have (`default`, `kube-system`). If a
later box moves `probing` onto `Console` — which is the obvious tidying — that
hole opens and `switched`'s three-line clear will look complete. One sentence on
`Console::permits` or beside the local would hold it.

### F10 — nit. `changing` is a fourth state where a mark cannot be read and a review still goes out

D292 ruling 2's gate is `Offer::Act`, and `offered` does *not* fold
`App::changing` into it: `App::footer` returns the `changing … first` line early
(3093–3100), and `ui::key_map` heads `RUNNING` (494) and gates every permission
clause behind `else if heading.is_none()` (519). So while a mutation is in flight
on an un-cached namespace, one review goes out whose answer nothing can draw. One
round trip, cached afterwards — genuinely a nit, and I mention it only because
`refusals`' own doc lists the four states that *cannot* reach it and this is a
fifth it does not name.

### F11 — nit. `selected` clones a `Card` on every frame now

`selected` (10220) returns `(*card).clone()`; a `views::Card` owns
`Vec<Finding>` and every `Finding`'s strings. Before this box that happened on a
keypress; now it happens on every frame, to read three fields. `shown_cards`
already hands back `Vec<&Card>`, so a borrowing sibling would do — but `selected`
is shared with `wanting` and `entered`, and CLAUDE.md keeps a shared-helper change
per-box. Not worth a round trip of its own.

---

## What I did not check, and what would

- **Nothing was run.** No `cargo`, no `just`, no cluster, no binary — `tester`
  held the host mirror. The dev's tests were read, not executed.
- **The marks have never been seen on a real screen.** The claim that this box
  closes F6's `?` half is read off `ui::key_map` and the dev's `main_tests.rs`
  assertions, not off a pty. The PM's cluster proof is what settles it, against
  § 5's credential of `reports/2026-09-26-the-error-state-pass.md`, and the one
  thing to look at is the `?` screen's three rows plus the footer, side by side
  with the same run under the admin context.
- **GKE, EKS and OpenShift authorizer chains** (F2) — one command each,
  § 1b.
- **`create selfsubjectrulesreviews` under `k8rs-readonly` on a cluster with no
  default bindings** (F1) — never measured for this reader; D229 asks for it and
  2026-09-05 measured it for the headless `may-i` only.
- **The request rate on a big cluster** (§ 5) — the `apiserver_request_total`
  command above, over a held `↓`.

---

# Round 2 — the fixes read back

**2026-09-27 · `k8s-admin` · operator review, round 2 — blocking**

Same conditions: **nothing was run**, no cluster, no test host. Read-and-reason,
plus two things I counted locally with `python3` (string widths — not a build, not
a guard, not the binary) and one fact I found already **measured** in `NOTES.md`
that settles question 2 better than my reasoning could.

What I read this round: `src/views.rs` 2664–2711 (`Offer::act`, the two new
predicates) · `src/main.rs` 5694–5790, 9288–9299 (the link clear), 9404–9447
(`refusals`), 9449–9460 (`answered`), 9587–9639 (`linked`) · `src/ui.rs` 446–470,
537–552 · `src/ops.rs` 2556–2658 (`DELETABLE`, `removal`) ·
`src/main_tests.rs` 16920–17092 · `src/ui_tests.rs`
`no_refused_row_outgrows_the_body_for_any_kind_it_can_name` ·
`screens/help.md` diff whole · `docs/security.md` diff whole ·
`CLAUDE.md` diff · `NOTES.md` § D285 ruling 4, § D293.

## Q1 — the seam, the group check, and `delete` against `DELETABLE`

**The seam is right.** Two readers now derive from one function and `Offer::act`
is one of them, so the arrangement is *two callers, one predicate* rather than
*one guard plus one caller that has to remember it* — which is the shape my round-1
F6 asked for and did not get. It also moves the guard to where the next
reviewer will be standing: a box that flips `SCALE_IS_BUILT` reads
`views::Offer::scales` and not a `main.rs` comment about a constant.

**The group check belongs inside, and having it inside is the point.**
`ui::addressed` hands back a pair that *cannot come apart*
([D51](../NOTES.md#d51--the-third-review-of-the-same-contract-and-the-sentence-that-would-have-rebuilt-the-bug-it-closed-2026-08-12)), and `k8s::browsable` deliberately keeps the same plural
under two groups as two resources — `apps/v1 StatefulSet` beside OpenKruise's
`apps.kruise.io/v1beta1 StatefulSet`, one sidebar row. A predicate taking only the
kind word would answer `true` for the Kruise one. Inside, no caller can forget it;
outside, every caller has to remember, and that is the defect one file down.

**`SCALE_IS_BUILT` staying out is correct** and the doc says why in the right
terms: the constant suppresses a key that exists, the predicate answers whether
the operation exists for the kind. Folding it in would make *never asked* true for
the wrong reason and wrong the day it flips.

**`delete` ungated, checked against `DELETABLE` as asked — correct.**
`src/ops.rs` 2556 is `"a deployment, a statefulset, a daemonset, a replicaset, a
pod and a node"`: six kinds, the same six as `KINDS`. `refusals` only reaches
`delete` for a kind `known_kind` resolved, which is those six; `ops::removal`
refuses only a word that names no kind; `views::Op::Delete`'s `offers` is `true`
for every `Offer::Act`. So there is no kind reaching `refusals` for which a
`delete` verdict would name a permission that does not exist to hold, and the
asymmetry between the three is the operations' own and not the code's.

`answered` (9455) maps *not asked* to `[None; N]`, and `Refused::of` draws `None`
exactly as a yes — so a withheld operation is `refused == false` and carries no
reason. That is the half D293 was about, and it holds.

## Q2 — clearing on any link transition

**`NOTES.md` § D285 ruling 4 already measured the answer, and it is not the one
the fix's comment assumes.** On a fast cut of all six connections, *"searching the
bytes across the cut for `disconnected, retrying` found none — `live` to `live`.
Five errors and five recoveries interleave inside ~800 ms."* So on the commonest
drop **`screen.link` never leaves `Live`, no transition exists, and the clear never
fires.** That is R1 below.

**What it does mean is that there is no storm to worry about.** Reasoned from
three things read at HEAD: `linked` (9587) only answers `Lost` when
`dropped && !answering` — *every* watch in trouble, not one — so a transition is a
real total outage and not renderer noise; `Live`/`Connecting` turns on
`snapshot.is_some()`, which does not flip back; and while the link is anything but
`Live`, `ui::withheld` answers `Paused`, `offered` answers `Offer::Move`, and
`refusals` is not called at all, so **no probe runs during the down phase**. A
flap therefore costs exactly **one review, for the one namespace under the cursor,
per painted recovery** — and the re-LIST of five watches in that same window costs
far more.

**So: "any transition" is right, and it is the more defensive of the two
readings.** Clearing on the way down is free, because nothing will be probed
until recovery; and it stays correct if a later box ever lets `Lost` reach
`Offer::Act`, where "only back to `Live`" would not. One comparison, no cost. I
would not narrow it.

Settles on a cluster with the relay of
`reports/2026-09-26-the-error-state-pass.md` § 4a: cut all six connections with a
Deployment card selected and an answer cached, and read whether the `?` row keeps
its clause across the cut. `docker stop` on the control plane is the other shape,
the one D285 ruling 4 measured as *doing* paint `Lost`.

## Q3 — 78 columns exactly

**Counted, not taken from the file** (`python3`, char count, and the `—` is one
column and three bytes — which is why the test's `width(line)` and not `len` is
load-bearing):

| row | landed | `(` at |
|---|---|---|
| `r`, `deployments` | 77 | 38 |
| `r`, `statefulsets` | **78** | 38 |
| `r`, `daemonsets` | 76 | 38 |
| `ctrl-d`, `statefulsets` | 74 | 51 |
| `ctrl-d`, `pods` | 66 | 51 |

`screens/help.md`'s table (77/78, 73/74) and its *`(` at the 38th* are both
right, and `r`/`statefulsets` is the ceiling itself.

**Acceptable to ship.** Zero headroom is safe here because both inputs that can
grow the row are closed sets guarded from both sides: the resource comes from
`KINDS`' six literals and `statefulsets` is already the longest, and the verb
comes from `RESTART_VERBS` joined — so `no_refused_row_outgrows_the_body_for_any_kind_it_can_name`
iterates every kind × every combination and *also* asserts the boundary
(`clusterroles` at 12 fits exactly, `deviceclasses` at 13 overflows by one). A
grown row is a red test, not a silent clip.

**What zero headroom actually costs is the next operation's wording, and that
debt is already booked.** D261 ruling 4 says v0.2's `drain` makes the constants
gain *a resource beside each verb*; if `restart` ever gains a second verb the
join grows by at least four columns and this row is over. So the row is now the
constraint that will decide how `cordon` and `drain` word their clauses — worth
knowing before somebody discovers it as a surprise in a red build.

## Q4 — the wording that shipped

**`(no delete deployments)` is right and I would ship it unchanged.** The
parenthesis is fresh, `no` is its first word, and it mirrors the footer's
`r no restart`. A reader who has never met RBAC reads *no delete deployments* as
*I am not allowed to delete deployments* — which is both the true sentence and,
word for word, the grant to hand an admin. It does not read as a command, because
`no` cannot begin one.

**`(rollout restart — no patch deployments)` has a natural wrong reading, and it
is new.** `A — no B`, with A and B both verb phrases, parses in ordinary English
as *does A, not B*: **"this key does a rollout restart, it does not patch the
deployment."** That is a coherent, plausible, entirely wrong sentence, and for a
reader who does not know `patch` is an RBAC verb it is the *more* natural of the
two readings. Before `no` landed, `— patch deployments` was ambiguous but inert;
`no` makes the ambiguity point somewhere specific and false.

`screens/help.md`'s own ruling is the fix's constraint and it is a good one —
`no` must not touch the term, because `no rollout restart` would misname the
command. Two wordings satisfy it, and **both are 78 columns on `statefulsets`,
so neither costs a column**:

```
    r       restart, at its own pace (rollout restart · no patch statefulsets)   78
    r       restart, at its own pace (no patch statefulsets — rollout restart)   78
```

The first is one character and the laziest thing that works: `·` is this
product's established *these are two separate facts* separator — 27 uses across
`views.rs` and `ui.rs`, and the header's own `ctx: limited · ns: default · live ·
admin` — and it cannot be read as *not*. The second makes `r` open with `no` the
way `ctrl-d` does, which is the consistency `screens/help.md` argues for two
bullets earlier, and leaves the term unnegated at the end.

This is `tui-designer`'s ruling and `ui.rs`'s literal, not mine to make. I am
naming the misreading and the fact that fixing it is free.

## Q5 — F1's fix, and whether the grant is sufficient

**The docs fix is correct and lands in the right role.** `docs/security.md` adds
the rule to `k8rs-readonly` (before the C4 comment, inside that role's `rules`),
the admin role keeps its own copy, and the paragraph now says the grant arrived
with a different box than the one it was promised to, with the journey spelled
out. The one thing I would check on a second read is that the sentence *"Two
readers, and neither is a write"* is exact: it is — the console probe is
`may_i_in` and the subcommand is `may_i`, and `selfsubjectaccessreviews` is
needed by the **second** reader only, so granting both is right and the comment
does not overclaim.

**Reasoned, not measured: the grant is sufficient for the console probe, and
nothing else in the role is missing for it.** The probe's whole wire is
`create selfsubjectrulesreviews` in `authorization.k8s.io`; it needs no discovery
(a refused `/apis` empties the sidebar's RESOURCES and leaves Alerts and the link
alone), no object read, and no subresource. Everything else it depends on — a card
on screen — the role already grants. The half I cannot reason is whether a
namespaced review under an identity with **no default bindings** returns
`status.resourceRules` containing that role's own rules rather than an empty list
beside `incomplete: false`; if it returned empty-and-complete, every key would be
marked `no`, which is the one direction D229 ruling 4 forbids.

**The commands, in order** — the identity trick is the repo's own, from D261
ruling 4: bind so the subject carries `system:unauthenticated`, which strips
`system:authenticated`'s default `system:discovery` and `system:basic-user`:

```
# 1. the identity has no default bindings
kubectl --context <role-only> auth can-i --list

# 2. the grant itself
kubectl --context <role-only> auth can-i create selfsubjectrulesreviews.authorization.k8s.io

# 3. what the review actually returns for it — the half that decides the direction
kubectl --context <role-only> create -f - -o yaml <<'EOF'
apiVersion: authorization.k8s.io/v1
kind: SelfSubjectRulesReview
spec: {namespace: default}
EOF
#    read three fields only: status.incomplete, whether status.resourceRules
#    names `deployments`, and whether any rule carries the verb `delete`

# 4. the screen, with a Deployment card selected
K8RS_CLUSTER=review <binary> --context <role-only>   # `?` → expect `(no delete deployments)`
#    negative control: drop the rule from the role, re-run, expect no clause
```

Step 3 is the one that cannot be skipped: steps 2 and 4 both pass for a cluster
that answers `Yes` to everything.

## Q6 — the F2 paragraph, read as an AKS operator

**It stops the bug hunt.** Read cold, it answers the three questions that
operator has, in the order they have them: *is it broken* (no — the cluster cannot
enumerate, upstream's own function returns `incomplete: true` unconditionally),
*am I less safe* (no — **every key stays lit and every operation still works**,
the dry-run and the real call decide), and *how do I confirm it is my cluster*
(the `SelfSubjectRulesReview` call, with `status.incomplete: true` named as the
condition). Naming AKS and Azure RBAC by name is what makes it findable by
someone searching their own symptom, and marking GKE, EKS and OpenShift
**unverified** is the right shape — it does not claim a measurement nobody took.

One thing I would not change but will name: `kubectl auth can-i --list` alone
prints *"the list may be incomplete: webhook authorizer does not support user rule
resolution"*, which is the whole diagnosis in one line, and it is the first of the
two commands. Good ordering.

---

## Round 2 findings, ranked

**No blockers.** The blocker of round 1 (`key_map` drawing a clause for a key the
footer withholds) is closed at the right seam, and I could construct no case where
the two readers disagree: both now call `views::Offer::scales` / `restarts` with
the same `(group, singular)` pair, and `answered` makes *not asked* draw as a yes.

### R1 — should-fix. The link-transition clear does not fire for the drop that was measured, and never fires for the journey its comment names

`src/main.rs` 9288–9299, and the test at `src/main_tests.rs` 17046–17092.

Two gaps, and the code is right in both — it is the comment and the test that
claim more than they deliver.

1. **The measured drop paints no transition.** D285 ruling 4, on a relay cut of
   all six connections: *"searching the bytes across the cut for `disconnected,
   retrying` found none — `live` to `live`."* `linked` needs *every* watch in
   trouble to answer `Lost`, and on a fast cut the five failures and five
   recoveries interleave inside ~800 ms, inside the coalescing window. So
   `console.link != screen.link` is false throughout and the cache is not
   cleared. The clear fires for a total outage — `docker stop` on the control
   plane, D285 ruling 4's other shape — and not for a wifi hiccup.
2. **The motivating journey involves no link change at all.** The comment and the
   test doc both say it: *"the likeliest reason a link drops and returns while a
   reader is staring at a refusal is an operator fixing that very refusal."* An
   admin editing a RoleBinding drops no watch and changes no link. So the fix
   keys on a correlation, and the journey — a permission granted mid-session
   becoming visible — is still only closed by `X` or a restart.

The test drives `console.link = ui::Link::Lost` **by hand** onto a healthy store,
which proves the mechanism and cannot prove the journey; the state it sets is one
D285 ruling 4 measured as not painted.

**I am not asking for a TTL.** Fail-open means the key always worked and only the
screen was stale, and a timer in a loop that has none is a worse trade. What is
owed is the comment saying what it catches — *a total outage, where every watch
stopped answering* — and what it does not: RBAC edited under a live link. My
round-1 F8 is narrowed, not closed, and the backlog entry should say so.

### R2 — should-fix. The state this round's blocker created has no mockup

`(no delete pods)` exists in exactly one place in the tree —
`src/main_tests.rs` 16994 — and `screens/help.md` § *When a key is refused* draws
only the both-refused Deployment, calling it *"the worst case"*. A **Pod** card,
with `ctrl-d` refused and `r` withheld, is the commonest card this product has and
is the exact state D293's blocker produced. The section's prose covers it (the
*withheld, not refused* bullet); no mockup does.

CLAUDE.md step 2's gate is *the mockup covers every state, not just the happy
one*. Had that mockup existed, a clause on a withheld `r` was visible at step 2
rather than at step 5 — which is the whole argument for the gate. `screens/` is
`tui-designer`'s.

### R3 — should-fix (wording, free to fix). `A — no B` reads as "does A, not B"

Q4 above. `src/ui.rs` 540 and `screens/help.md` 504. Two 78-column alternatives
measured, neither costing a column; `·` is the one-character version and is
already this product's separator for *two separate facts*.

### R4 — nit. Two unrelated `restarts` in one file

`views::Offer::restarts` (`src/views.rs` 2708) answers *does `ops::restart` serve
this address*; the free `views::restarts` (3622) formats `, 3 restarts` for a
container. Different paths, so nothing breaks — but one word with two unrelated
meanings in one file is a `grep` that lies, and the associated function is the
newer of the two. `serves_restart` would have cost nothing.

### R5 — nit. "Any transition" is asserted in one direction

`a_link_that_dropped_and_came_back_forgets_what_the_old_one_was_told` drives
`Lost → Live`. The `Live → Lost` direction is the half the comment argues for
(*"any transition and not just back to `Live`"*) and nothing asserts it. One more
`assert!(console.permits.is_empty())` after a frame whose store *is* in trouble
would hold the sentence. Cheap, and the behaviour is right either way.

### R6 — nit. A mechanical rewrap left a two-word line mid-paragraph

`src/ui.rs` 461–463: `` `ctrl-d` `` / `has no baseline` / `position to hold and
moves nothing.` — a 14-column line between two full ones, which is what a width
guard applied after the prose was written produces. Cosmetic, and worth one
sentence only because CLAUDE.md now puts that guard on the dev's own gate list:
it fixes the width and cannot fix the wrap.

---

## What round 2 did not check

- **Nothing was run.** No `cargo`, no `just`, no cluster, no binary.
- **The widths are mine, counted locally with `python3`** over the literals as
  written — not through `Paragraph` at `MIN_WIDTH`. The test does that, and I read
  the test rather than running it.
- **Q2's flap behaviour is reasoned** from `linked`, `withheld` and the
  `Offer::Act` gate, with D285 ruling 4's *measurement* doing the load-bearing
  half. The relay command in Q2 is what would settle the probe's own behaviour
  across a cut.
- **Q5 step 3 is the measurement nobody has taken** — whether a namespaced
  review under an identity with no default bindings returns that role's rules or
  an empty list beside `incomplete: false`. Everything else about F1 follows from
  it.
