# k8rs Technology Stack

> The stack decisions and their reasons, as settled in the design phase.
> Rationale details live in `../NOTES.md`; this is the authoritative list.

## Core choices

| Decision | Choice | Why |
|---|---|---|
| Language | **Rust** | Single static binary with no runtime, low idle footprint, and the ecosystem below. Fits the promise "one binary, nothing installed". |
| Licence | **`GPL-3.0-or-later`** | A fork may be sold, but it cannot be closed: the source, the licence and the author's name travel with every copy. Permissive licences protect attribution too, but they allow a closed-source repackaging — which is the thing being prevented ([NOTES § D13](../NOTES.md#d13--licence-gpl-30-or-later-reversed-2026-08-12)). `cargo publish` requires the field, so it exists from the first commit. Dependency policy is unaffected: `deny.toml` still rejects copyleft *dependencies*. |
| UI | **ratatui** (+ **crossterm** backend) | The de-facto Rust TUI library; immediate-mode drawing fits "redraw only on change". crossterm gives cross-platform terminal control (raw mode, events, colors). |
| Kubernetes client | **kube-rs** | Provides `watcher()` / `reflector()` out of the box — the watch-based architecture is the whole performance story. |
| API types | **k8s-openapi** | Typed Pod/Event structs. Pinned to the **newest** feature offered — **`v1_36`** (the window is `v1_32`…`v1_36`). Reversed from *oldest* on 2026-08-15: an old pin drops every field added since, at decode, without a word, while a new pin against an older cluster simply reads `None` — which every rule already treats as no finding. `scripts/fixture-audit.sh` fails if the pin falls below the cluster the fixtures came from. Upgraded together with kube-rs, never separately. |
| Async runtime | **tokio** | Required by kube-rs; also drives the single `select!` event loop (watch stream + keyboard + Ctrl-C). |
| Errors | **anyhow** | Startup errors only. Rules never return `Result` (missing field = no finding). One tiny enum distinguishes "403", "401 — your login expired" and "no connection", because the user message differs in all three. |
| Time | **`k8s_openapi::jiff`** — not a dependency of ours | `meta::v1::Time` *is* `jiff::Timestamp`, and k8s-openapi re-exports the library, so `Snapshot::now` uses the same type the API's own timestamps already have: no conversion layer and no eleventh crate. Verified against 0.28.0, not assumed — it was `chrono` before k8s-openapi moved ([NOTES § D18](../NOTES.md#d18--the-clock-is-an-input-not-an-ambient-fact)). |
| Fixtures / JSON | **serde_json**, feature `preserve_order` | Test fixtures, `DynamicObject`, decoding server-side `Table` responses. `preserve_order` is not optional: without it `Value` is a `BTreeMap` and every YAML we display comes out alphabetised instead of in kubectl's order. |
| YAML | **serde_yaml_ng** | `y` view in v0.1, `e` edit in v0.4 — admins read and write YAML, not JSON. Chosen over `serde_norway` by spike: the two are equivalent, and neither preserves comments, which is why edits keep the user's text buffer rather than round-tripping it. |
| X.509 | **x509-parser** | Certificate expiry warnings. Hand-parsing ASN.1 dates in a security-adjacent path is the wrong place to be clever. |
| Diff | **similar** *(v0.4)* | The diff shown before an edit is applied — the thing that makes `e` safe to press. Approved, but it enters the build with `edit`, not before. |

Full dependency list (ten crates approved; nine ship in v0.1, `similar`
arrives with `edit` in v0.4):

```toml
kube            # client + runtime (watcher/reflector) + discovery features
k8s-openapi     # API types — one k8s version feature pinned
ratatui
crossterm
tokio
anyhow
serde_json      # fixtures, dynamic objects, Table responses
serde_yaml_ng   # edit / view YAML
x509-parser     # certificate expiry
similar         # edit diff
```

The last three were added by the 2026-08-11 scope reversal. Everything else
predates it.

## Deliberately absent

| Not used | Until |
|---|---|
| `clap` | a flag needs validation, or a shipped subcommand appears. The four flags — `--read-only`, `--context`, `--namespace`, `--once` — are parsed from `std::env::args`. (The `k8rs ops …` driver used to prove the writes headlessly is scaffolding in the temporary main and never ships.) |
| `tracing` | debugging genuinely demands it |
| `tempfile` | `std::env::temp_dir()` plus an explicit 0600 create covers the edit buffer |
| theme loader (TOML, hot-reload) | never, most likely — `theme.rs` is 10 constants (YAGNI) |
| config file of any kind | never for v1 — zero configuration on first run is a product requirement |
| i18n framework | never for v1 — UI is English only, splitting later is cheaper |
| plugin system / trait layers | never — eight plain files |

Adding a new dependency is a decision, not a reflex: it gets a line in
NOTES.md with the reason.

## Visual identity

- **Catppuccin Mocha** palette, accent = teal, defined as constants in
  `theme.rs`.
- Truecolor (24-bit RGB) with a `COLORTERM` check and 16-color fallback in v1.
- Common Unicode symbols only (`● ▲ ○`), no nerd-font dependency.

## Toolchain

| Tool | Role |
|---|---|
| **cargo** | build; release profile: `lto = true`, `strip = true`, `codegen-units = 1` |
| **just** | task runner (`check`, `run`, `cluster-up/down`, `fixtures`, `e2e`, `mutants`) — Rust-ecosystem norm, works on Windows |
| **cargo-mutants** | run over the two pure files, `rules.rs` and `analysis.rs`: a surviving mutant is a diagnosis change no test objected to, i.e. a hole in what the tool claims to detect. It proves the **rules**, not the snapshot decode below them — it mutates return values and match guards, never a struct literal's field assignment, so on the decode it found 1 of the 32 holes a hand-written field-level sweep found ([NOTES § D41](../NOTES.md#d41--cargo-mutants-cannot-see-the-defect-it-was-put-there-to-catch-2026-08-12)) |
| **kind** + **kubectl** | test cluster with deliberately broken pods; fixture capture. Driven by [`scripts/cluster.sh`](../scripts/cluster.sh) — `up` · `down` · `reset` · `break` · `verify` · `break-runtime` · `break-nodes` · `unbreak` · `status`. Four nodes — a control plane and three workers, one per node state `break-nodes` produces — with the node image pinned to **`kindest/node:v1.36.1`** so fixtures stay reproducible. `K8RS_APISERVER_ADDRESS` points it at another machine; kind writes `127.0.0.1` otherwise and no other host can reach it |
| **jq** | fixture sanitization — [`scripts/sanitize.jq`](../scripts/sanitize.jq), applied to every object as it is captured. Payloads are destroyed (managedFields, annotations, env values, pull secrets, anything PEM-shaped); references are kept, because a rule reports *which* Secret a pod reads, never what is in it; and a capture carrying node identifiers that did not come from the kind cluster is refused outright rather than quietly rewritten — read from all five places a node name lives, including the `ownerReference` kubelet writes onto a static pod ([NOTES § D62](../NOTES.md#d62--the-fifth-place-a-node-name-lives-and-a-guard-that-asked-less-than-its-consumer-2026-08-12)). The filter walks the whole document instead of naming paths, because half the capture is the `List` that `kubectl get -A` returns. Tested in CI against a poisoned object **in both shapes** — a single object and a `List` ([NOTES § D29](../NOTES.md#d29--a-guard-is-proven-only-for-the-shapes-it-was-fed-2026-08-12)) — carrying each secret in every framing it can arrive in: whole value, quoted inside a sentence, and base64-encoded ([NOTES § D31](../NOTES.md#d31--the-sanitizer-matched-the-whole-string-and-secrets-are-rarely-the-whole-string-2026-08-12)) |
| **git-cliff** | CHANGELOG from conventional commits (`feat:` / `fix:`) |
| **cargo-deny** | advisories, license policy, source policy (CI) |
| **clippy** | `-D warnings` + `disallowed-methods` ban on K8s write calls |
| **GitHub Actions** | fmt/clippy/test + cross-compile check matrix; release on `v*` tags. Also the honest-test guards: a run with zero tests, or an unexplained `#[ignore]`, fails the build |

## The test cluster — reproducing it yourself

Every fixture in `tests/fixtures/` was captured from a cluster you can stand up
in one command. Nothing here is specific to the machine it was first run on.

```
just cluster-up                # four nodes, kindest/node:v1.36.1
scripts/cluster.sh break       # apply the deliberately broken pods + the healthy pair
scripts/cluster.sh status      # watch the states settle — a few minutes
scripts/cluster.sh verify      # assert each one reached the state its rule is about
just fixtures                  # capture, sanitized on the way out
scripts/cluster.sh unbreak     # remove the demo pods and put every worker back
just cluster-down              # tear it down
```

`just fixtures` damages the **machines** itself, at the end, in two steps: it
captures every pod and workload first, then calls `scripts/cluster.sh
break-runtime` — which reboots the node one pod is on, three times, so a restart
count rises with no crash behind it — captures that one, then calls
`scripts/cluster.sh break-nodes`, which cordons one worker, taints a second and
stops the kubelet on a third, and captures `nodes.json` last. That order is the
design, not a detail: a reboot alone raises `restartCount` on every pod on that
worker, a cordon changes where a pod would go, a `NoExecute` taint evicts what is
already there, and a stopped kubelet turns every pod on that node `Unknown`
within a minute — so any of them landing before the pod captures would write a
state no manifest asked for. It also means the cluster is left broken on purpose
when the capture finishes — `unbreak` is what puts it back, including a node
container left stopped by a reboot that did not finish.

`verify` is the step worth understanding: it is what stands between the project
and a fixture that never reached its state, which is a test that cannot fail. It
waits, then prints one line per fixture naming the rule each one exists for, and
refuses to let the capture run until all of them pass. Each predicate asserts
something true across the *whole* window the capture could land in, not at one
instant of it — a crash loop is several states, and naming one of them
certifies a moment that is over by the time the bytes are written
([NOTES § D61](../NOTES.md#d61--a-verify-predicate-must-hold-across-the-whole-window-not-at-one-instant-2026-08-12)).
Run `break → verify → fixtures` in one sitting: the healthy-side pods sleep for
an hour and then restart, and a capture taken after that catches one of them
briefly not ready.

Knobs, all optional:

| Variable | Default | Why you would set it |
|---|---|---|
| `K8RS_APISERVER_ADDRESS` | `127.0.0.1` | The cluster is on another machine. kind writes `127.0.0.1` into the kubeconfig otherwise, and no other host can reach it |
| `K8RS_APISERVER_PORT` | `6443` | Port already taken |
| `K8RS_WORKERS` | `3` | One worker per node state `break-nodes` produces — cordoned, tainted, kubelet stopped — so no node fixture has two causes at once. Everything except `break-nodes` works on fewer, and `break-nodes` refuses out loud rather than doubling two states onto one node |
| `K8RS_CLUSTER` | `k8rs` | Running more than one. The sanitizer refuses captures whose node names do not start with `k8rs-`, so a renamed cluster cannot produce fixtures — and that is used on purpose: a throwaway cluster raised to check one claim runs as `K8RS_CLUSTER=review`, which the guard rejects, so it cannot yield a committed fixture even by mistake ([D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)) |
| `K8RS_NODE_IMAGE` | `kindest/node:v1.36.1` | Pinned on purpose — fixtures are only comparable against a known version, and the capture stamps it into `tests/fixtures/K8S_VERSION` |
| `K8RS_VERIFY_TIMEOUT` | `420` | How long `verify` waits for states to settle. CrashLoopBackOff has to enter backoff and an OOM kill has to actually happen |
| `K8RS_RUNTIME_TIMEOUT` | `300` | How long `break-runtime` waits for each of its two states — a rebooted node coming back, and the pod on it running again. It is the one step that can hang on a machine rather than on the API server |

**Running it against a cluster that is not kind is refused, not sanitized.**
Node names carry real infrastructure, and rewriting them would break the pod↔node
joins the node rules are built on — so
[`scripts/sanitize.jq`](../scripts/sanitize.jq) errors out instead of producing
something that only looks safe.

## Targets & platforms

- Release binaries: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
  (static — keeps the single-binary promise), `x86_64/aarch64-apple-darwin`.
- Windows: best-effort via `cargo install`, no binary in v1.
- Minimum terminal: 80×24, truecolor preferred (fallback provided).
