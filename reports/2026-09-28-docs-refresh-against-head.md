# docs/ read against the tree at HEAD — 2026-09-28

Operator review of the Phase 13 box *`docs/` refreshed against the code as
built*. Every claim below was checked against source, `Cargo.toml`,
`Cargo.lock`, the justfile, `scripts/` and `.github/` on this machine. **No
cluster was raised and nothing was built** — every claim in this box is
answerable off the tree.

Tree: `development`, working tree carrying the PM's `docs/` edits
(`git status --short` → `M backlog.md docs/README.md docs/architecture.md
docs/maps.md docs/security.md docs/tech-stack.md`).

---

## 1. RBAC — every `apiGroups`/`resources` pair against what the code requests

### What the code actually asks for

```
$ grep -n 'use k8s_openapi::api' src/k8s.rs
132:use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
133:use k8s_openapi::api::certificates::v1::CertificateSigningRequest;
134:use k8s_openapi::api::core::v1::{Node, PersistentVolumeClaim, Pod, Service};
137:use k8s_openapi::api::core::v1::Event as ClusterEvent;
138:use k8s_openapi::api::discovery::v1::EndpointSlice;
139:use k8s_openapi::api::policy::v1::PodDisruptionBudget;
```

```
$ grep -n 'watcher(' src/k8s.rs | sed -n '15,20p'
8541:            watcher(scoped::<Pod>(client.clone(), coverage), config.clone())
8547:            watcher(Api::<Node>::all(client.clone()), config.clone())
8552:            watcher(  ... scoped::<Deployment>
8560:            watcher(  ... scoped::<StatefulSet>
8568:            watcher(scoped::<DaemonSet>(client.clone(), coverage), config)
```

Five permanent watches. On-demand lists (`report_lists` + `certificate_requests`,
`src/k8s.rs` § WHAT A REPORT ASKS FOR): ReplicaSet, Service, EndpointSlice,
PersistentVolumeClaim, PodDisruptionBudget, CertificateSigningRequest.
Metrics (`node_usage`, § WHAT A NODE IS USING): `NodeMetricsList`, LIST only.

Every mutating / `create` call in `ops.rs`:

```
$ grep -n '\.patch\|\.delete\|\.create\|patch_scale\|get_scale' src/ops.rs | grep -v '^\s*//'
1873:    let read = match tokio::time::timeout(READ_DEADLINE, api.get_scale(scaling.name)).await {
1962:            async move { api.patch_scale(scaling.name, &params, patch).await }
2476:                api.patch(restarting.name, &params, patch)
2835:                api.delete(deleting.name, &params)
3098:    let (rules, unsure) = match api.create(&PostParams::default(), &review).await {   # SelfSubjectRulesReview
3149:    match api.create(&PostParams::default(), &review).await {                        # SelfSubjectAccessReview
```

```
$ grep -rn 'eviction\|unschedulable\|\.replace' src/ops.rs | grep -v '^\s*//'
(no match outside comments)
```

Kinds each operation accepts (`scalable` :1760, `rollout` :2231, `removal` :2597):

| operation | kinds | verb on the wire |
|---|---|---|
| scale | deployment, statefulset, replicaset | `get`+`patch` on `<kind>/scale` |
| rollout restart | deployment, statefulset, daemonset | `patch` on `<kind>` |
| delete | deployment, statefulset, daemonset, replicaset, pod, node | `delete` |
| may_i / may_i_in | — | `create` on both `SelfSubject*Review` |

### `k8rs-readonly` (docs/security.md:155–229)

| rule | code that reaches it | verdict |
|---|---|---|
| `nonResourceURLs [/api /apis /api/* /apis/* /version]` get | `served()` → `Discovery::run_aggregated()`; fallback `client.list_api_groups()` + `discovery::group()`; `client.apiserver_version()` + `skew`'s second `/version` | **true** |
| `"" pods, pods/log` g/l/w | 5-watch set, `lists_pods` scope probe, log stream | **true** |
| `"" events` g/l/w | `Api::<ClusterEvent>::namespaced(...).list(...)` at `src/k8s.rs:6307` — core/v1, LIST only | **true for get/list; `watch` unused** |
| `"" services, persistentvolumeclaims` g/l/w | `report_lists` | **true for get/list; `watch` unused** |
| `"" nodes` g/l/w | node watch + `--yaml/--describe` on a node | **true** |
| `apps deployments, statefulsets, daemonsets, replicasets` g/l/w | 3 watches + RS owner cache/`report_lists` | **true** |
| `policy poddisruptionbudgets` g/l/w | `disruption_budgets` | **true for get/list** |
| `certificates.k8s.io certificatesigningrequests` g/l/w | `certificate_requests` | **true for get/list** |
| `discovery.k8s.io endpointslices` g/l/w | `endpoint_slices` | **true for get/list** |
| `metrics.k8s.io nodes` get,list | `node_usage` sends LIST only | **`list` true, `get` unused** |
| `authorization.k8s.io selfsubjectrulesreviews, selfsubjectaccessreviews` create | `may_i_in` :3090, `may_i` :3132 | **true, both** |

Nothing the read path requests is missing from the role. The six kinds
`--yaml`/`--describe` accept are `KINDS: [Kind; 6]` (`src/main.rs:5699`):
deployment, statefulset, daemonset, replicaset, pod, node — all covered.

### `k8rs-admin` (docs/security.md:246–299)

| rule | code that reaches it | verdict |
|---|---|---|
| `"" pods delete` | `removal("pod")` | **true** |
| `"" pods/eviction create` — commented `# drain` | no `evict` call anywhere in `src/` | **false — nothing reads it** |
| `"" nodes patch` — commented `# cordon / uncordon` | no cordon/uncordon operation exists | **false — nothing reads it** |
| `"" nodes delete` | `removal("node")` | **true** |
| `apps deploy/sts/ds get` | documented as the operator's, for the taught `kubectl rollout restart` | **true as documented** |
| `apps deploy/sts/ds patch` | `restart` | **true** |
| `apps deploy/sts/ds update` — commented `# … edit …` | no `replace`/`update` call in `ops.rs` | **false — nothing reads it** |
| `apps deploy/sts/ds delete` | `removal` | **true** |
| `apps replicasets delete` | `removal("replicaset")` | **true** |
| `apps */scale get, patch` | `get_scale` + `patch_scale` | **true, both verbs** |
| `authorization.k8s.io … create` | `may_i` | **true** |

`NOTES.md:481` § Operations places cordon/uncordon and drain at **v0.2** and
edit at **v0.4**.

---

## 2. Every `kubectl` line printed in `docs/`

```
$ grep -c 'kubectl' docs/*.md
docs/architecture.md:12   docs/security.md:20   docs/tech-stack.md:4
```

Runnable lines, checked against the string the binary builds:

| doc | line | what the binary builds | verdict |
|---|---|---|---|
| security.md:546 | `kubectl --context prod-eu scale deployment/web --replicas=5 -n payments` | `src/ops.rs:1903` `format!("kubectl{} scale {object} --replicas={} -n {namespace}", context_segment(...), ...)` | **identical** |
| security.md:546 | `call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale` | `src/ops.rs:1911` `format!("{}/{}/scale", DynamicObject::url_path(&resource, Some(namespace)), name)` | **identical** |
| security.md:547 | `result · attempt … · recorded … · deployment/web · dry-run: … · the change was made` | `src/ops.rs:1271` `result_line` | **identical field order** |
| security.md:338 | `kubectl auth can-i --list` | no binary equivalent; a reader's own probe | runnable, matches the prose |
| security.md:339–343 | `kubectl create -f - -o yaml <<'EOF' … SelfSubjectRulesReview … spec: {namespace: default}` | matches `SelfSubjectRulesReviewSpec { namespace }` in `src/ops.rs:3091` | **field name correct** |
| architecture.md:442 | `kubectl get pods -A --chunk-size=1` — quoted as the **rejected** spelling | binary prints `$ kubectl get --raw '/api/v1/pods?limit=1'` (`src/main_tests.rs:4625`) | **prose is correct**: the rejected line is named as rejected |
| architecture.md:446–447 | the discovery line carries `--verbs=list` | `src/main.rs:3156` `log.push(format!("{kubectl} api-resources --verbs=list"))` | **identical** |
| architecture.md:202 | `Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json` | `src/k8s.rs:4888` `TABLE_ACCEPT` | **byte-identical** |
| security.md:137 | restart patches `kubectl.kubernetes.io/restartedAt`, not kube's `kube.kubernetes.io/…` | `restart_patch` `src/ops.rs:2289` | **true** |
| tech-stack.md:141–147 | `just cluster-up` / `scripts/cluster.sh break|status|verify|unbreak` / `just fixtures` / `just cluster-down` | justfile recipes `cluster-up`, `cluster-down`, `fixtures`; `scripts/cluster.sh:1445-1454` dispatch | **all nine subcommands exist** |

No `--force` and no missing `-n` found in any doc line. Every namespaced
example carries `-n`; `delete node/…` correctly carries none (`src/ops.rs:2785`
branches on `deleting.namespace`).

---

## 3. Counts and lists

| doc claim | command | output | verdict |
|---|---|---|---|
| tech-stack.md:106, architecture.md:184 — `theme.rs` is 10 `Colour` + 8 `Signal` | `grep -oE ': (Colour\|Signal) =' src/theme.rs \| sort \| uniq -c` | `10 : Colour =` / `8 : Signal =` | **true** |
| tech-stack.md:103 — fourteen flags | `grep -oE '^(pub )?const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs \| wc -l` | `14` | **true** |
| tech-stack.md:23 — ten of thirteen crates in `Cargo.toml` | parsed `[dependencies]`=9 + `[dev-dependencies]` `serde_json` | 10 distinct names; absent: `crossterm`, `anyhow`, `similar` | **true** |
| tech-stack.md:77,79 — `Cargo.lock` at 319 | `grep -c '^\[\[package\]\]' Cargo.lock` | `319` | **true** |
| tech-stack.md:133 / maps.md:175 — ci.yml, three jobs | `grep -nE '^  [a-z-]+:' .github/workflows/ci.yml` | `49: check` `89: deny` `98: cross` | **true** |
| maps.md:174 / security.md:216 — `.github/` holds one file | `find .github -type f` | `.github/workflows/ci.yml` | **true** |
| tech-stack.md:196 — four release targets | ci.yml matrix | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin` | **true** |
| maps.md:164 — five agent definitions | `ls .claude/agents/` | `dev-core dev-ui k8s-admin tester tui-designer` | **true** |
| maps.md:44 — eight product files | `ls src/*.rs` minus `*_tests*` | 8 | **true** |
| maps.md:57 — `ui.rs` passed ~800 lines eight times over | `wc -l src/ui.rs` → `6622` | 6622/800 = 8.3 | **true** |
| maps.md:101 — security-guard: six checks, **17** planted violations | `python3 scripts/security-guard.py --self-test` | `self-test passed — 37 planted violations`; run prints six `OK` lines | **six true, 17 false (37)** |
| maps.md:119 — `Cargo.toml`'s `exclude` drops **nine** entries | counted `Cargo.toml:24-33` | 19 entries | **false** |
| maps.md:91 — everything in `scripts/` runs in `just check` | `grep 'scripts/' scripts/guards.sh` | `make-certs.sh`, `make-csr.sh` appear in no caller | **false** |
| maps.md:38 — both RBAC roles copied into the READMEs **by script** | `grep -rn 'k8rs-readonly' scripts/` | no match | **false — no script compares them** |
| maps.md:38 — the two README RBAC blocks match security.md | normalised diff of the two fenced blocks | identical (comments aside) | **true today** |
| architecture.md:64 — seven usage forms, `   \|   ` separated | `src/main.rs:362` `USAGE` | seven forms, that separator, text identical | **true** |
| architecture.md:336–338 — crossterm 0.29.0, no `event-stream` | `grep -A2 '^name = "crossterm"' Cargo.lock` | `version = "0.29.0"` | **true** |
| architecture.md:632 — floor 1.29 | `src/k8s.rs:3874` `const OLDEST_SERVER: u32 = 29;` | | **true** |
| architecture.md:613 — pin `v1_36` | `Cargo.toml:49` `features = ["v1_36"]` | | **true** |
| tech-stack.md:118 — COLORTERM check + 16-colour fallback, wired | `src/theme.rs:102` `depth()`; `src/main.rs:7984` calls it | | **true** |
| tech-stack.md:119 — `● ▲ ○`, no nerd font | signal values in `src/theme.rs` | `● ▲ ○ ▸ ⚠` + `changing…` + `read-only` | **true but incomplete list** |
| maps.md:136 — `examples/spike_tui.rs` | `ls examples/` | present | **true** |
| maps.md:132 — `tests/binary.rs § THE WIRE` | `grep -n 'THE WIRE' tests/binary.rs` | `3113 // --- THE WIRE START ---` | **true** |

---

## 4. Present-tense claims about code that does not exist

| doc:line | claim | tree |
|---|---|---|
| architecture.md:417 | "metrics-server (if ever used) is polled slowly (30s+) and only for **visible pods**" | `node_usage` reads `NodeMetricsList`, cluster-wide, one item per node; no pod metrics path exists. `METRICS_POLL = 30s` is true. security.md:201-204 documents the pod half as *removed* |
| architecture.md:213–214 | the Table request "is the only hand-built HTTP request in the binary" | three more: `src/k8s.rs:3124` `client.request::<NodeMetricsList>`, `:6636` `client.request` for `document`, and `Request::get` for `/version` (`:8286`) |
| architecture.md:215–217 | "Browser views therefore watch `watch_metadata` … and re-fetch the Table, debounced" | § KEEPING A BROWSER VIEW FRESH (`src/k8s.rs:5238`) is policy only — the code block is ```` ```ignore ````, and `:5283` records that `metadata_watcher` is `#[deprecated]` and was rejected. No browser watch is built |
| architecture.md:590–593 | "Phase 5 is where that becomes true; **no code in this repo has met an API server yet**" | Phase 5 closed; `k8s.rs` is 9083 lines of live client |
| architecture.md:569–572 | "A `409 Conflict` on apply … the user is offered a re-read" | `apply`/`edit` is v0.4. `src/ops.rs:1575` ships the *sentence* ("look at it again before deciding"), not an offer. CLAUDE.md § Security gate carries the correction this line lacks |
| security.md:614–616 | "revealing a value requires an explicit second action" | `grep -rn 'reveal' src/ui.rs src/views.rs` → no match. `k8s::mask` (`src/k8s.rs:6458`) is unconditional |
| security.md:646–648 | the edit temp file "is written … mode 0600 and removed on exit *and* on panic" | v0.4; no temp-file code in `src/` |
| security.md:62, architecture.md:314 | typed confirmation "for delete and drain" | drain is v0.2 |
| architecture.md:220 | typed structs "only where the rule engine needs field access (Pod, Node, Deployment, Service, PVC)" | twelve typed kinds imported in `k8s.rs` (list in § 1) |
| tech-stack.md:16 | Core-choices row `Errors \| **anyhow**` | `grep -rn 'anyhow' src/` → no match; not in `Cargo.toml`. tech-stack.md:58 states the absence 42 lines later |

---

## 5. What was not checked

- **Nothing was compiled or run.** No `cargo`, no `just check`, no binary. The
  only executable run was `python3 scripts/security-guard.py` (+ `--self-test`),
  which reads the tree and spawns no build.
- **No cluster.** Every RBAC verdict above is derived from the call sites in
  `src/`, not from applying a role to a live apiserver. The last live proof of
  `k8rs-readonly` is D187's 2026-08-30 run and it predates the
  `authorization.k8s.io` rule that was added on 2026-09-26; **that rule has not
  been exercised under the role itself**.
- `screens/` against the drawn output,
  `REQUIREMENTS.md`, `NOTES.md`, `todo.md`, `README.md`/`README_TR.md` bodies
  beyond their two RBAC blocks.
- Anchor resolution of the `D##` links added by this diff — `check-docs.py`
  covers that and was not run here.
- `docs/security.md` §§ Trust model, Token hygiene, Data displayed and stored
  were read for present-tense claims about unbuilt code (§ 4 above) and not
  re-derived line by line against `k8s.rs`'s error renderers.
