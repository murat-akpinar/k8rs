# k8rs Technology Stack

> The stack decisions and their reasons, as settled in the design phase.
> Rationale details live in `../NOTES.md`; this is the authoritative list.

## Core choices

| Decision | Choice | Why |
|---|---|---|
| Language | **Rust** | Single static binary with no runtime, low idle footprint, and the ecosystem below. Fits the promise "one binary, nothing installed". |
| Licence | **`MIT OR Apache-2.0`** | The Rust ecosystem default, and permissive is the right answer for something people run against production clusters. `cargo publish` requires the field, so it exists from the first commit. |
| UI | **ratatui** (+ **crossterm** backend) | The de-facto Rust TUI library; immediate-mode drawing fits "redraw only on change". crossterm gives cross-platform terminal control (raw mode, events, colors). |
| Kubernetes client | **kube-rs** | Provides `watcher()` / `reflector()` out of the box — the watch-based architecture is the whole performance story. |
| API types | **k8s-openapi** | Typed Pod/Event structs. Pinned to the **oldest** feature still offered — **`v1_32`** (the window is `v1_32`…`v1_36`); the API is forward compatible, so an old pin talks to newer clusters. Support window = pinned ±2 minor. Upgraded together with kube-rs, never separately. |
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
| **cargo-mutants** | run over the two pure files, `rules.rs` and `analysis.rs`: a surviving mutant is a diagnosis change no test objected to, i.e. a hole in what the tool claims to detect |
| **kind** + **kubectl** | test cluster with deliberately broken pods; fixture capture. Driven by [`scripts/cluster.sh`](../scripts/cluster.sh) — `up` · `down` · `reset` · `break` · `verify`. Three nodes, node image pinned to **`kindest/node:v1.36.1`** so fixtures stay reproducible. `K8RS_APISERVER_ADDRESS` points it at another machine; kind writes `127.0.0.1` otherwise and no other host can reach it |
| **jq** | fixture sanitization in the capture script |
| **git-cliff** | CHANGELOG from conventional commits (`feat:` / `fix:`) |
| **cargo-deny** | advisories, license policy, source policy (CI) |
| **clippy** | `-D warnings` + `disallowed-methods` ban on K8s write calls |
| **GitHub Actions** | fmt/clippy/test + cross-compile check matrix; release on `v*` tags. Also the honest-test guards: a run with zero tests, or an unexplained `#[ignore]`, fails the build |

## Targets & platforms

- Release binaries: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`
  (static — keeps the single-binary promise), `x86_64/aarch64-apple-darwin`.
- Windows: best-effort via `cargo install`, no binary in v1.
- Minimum terminal: 80×24, truecolor preferred (fallback provided).
