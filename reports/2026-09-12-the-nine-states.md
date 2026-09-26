# The nine states — operator review measurements (2026-09-12)

Step 6 over the uncommitted `dev-ui` box that draws `screens/states.md`'s nine states and
gives each its own footer (`views::Offer`, `ui::Writes`, `ui::offered`, `ui::caveats`).

**No cluster was brought up** — nothing in this box calls an API, so every frame below is
`ratatui`'s `TestBackend` rendered by probe tests. The repo was copied to
`/home/shyuuhei/k8rs-review` with its own `CARGO_TARGET_DIR` under `$HOME` (CLAUDE.md § the
one hard rule of concurrency); `df -h /home` at the start: `954G 84G 870G 9%`. The probes
were appended **to the copy only** and run with
`CARGO_TARGET_DIR=… cargo test --quiet probe_ -- --nocapture --test-threads=1`.

**This file was measured and written by `k8s-admin` and transcribed by the PM**, because
that agent's definition in `.claude/agents/k8s-admin.md` grants it no `Write` tool while
CLAUDE.md's ownership table says `reports/` is the one tree it writes. The mismatch is the
PM's to close; the content below is the reviewer's.

## The measurements

| § | Command / probe | Result |
|---|---|---|
| 1 | `offered`/`may_mutate` over 4 view shapes × `Writes::{Live,Unaudited}` | `Unaudited ⇒ may_mutate false` in all four |
| 2 | same, `Writes::Live` | `Act`/`true` for Analysis and for an open detail tab, whose footers name no mutating key |
| 3 | `Pane::Denied(--namespace wording, [oom card])`, header `live · admin` | `offer: Move  may_mutate: false` |
| 4 | `clock: Some(behind)` + `writes: Unaudited(real sentence)` at 80×24 | `the card is on screen: false`, sidebar still `1 ●` |
| 5 | Alerts `Ready([])` and `Loading` with `Unaudited` | `audit sentence on screen: false` (both); the browser draws it for the same two |
| 6 | `open_log(dir, Source::Home)` / `Source::StateHome` | `(under your home directory)` / `(from $XDG_STATE_HOME)`, absolute path, **no trailing full stop** |
| 7 | `Ready(Table::default())` vs `Denied(msg, Table::default())` | `Filter` / `Move` — `⏎ open` promised over zero rows |
| 8 | `expired: true` over `Ready(empty table)` / `Loading` | `Filter` / `Nothing` — no `X switch cluster` on either |

### 3 — a namespace-scoped session can never reach `Act`

```
--- ADMIN WHO TYPED --namespace payments ---
offer: Move  may_mutate: false
footer: "↑↓ move  ⏎ open  / filter  ? all keys  q quit"

│▸ ALERTS         1 ●│  Showing only the payments namespace, because           │
│   network          │  ● payments/web  ·  3 of 5 pods              4 min ago  │
```

`Pane::Denied` is the namespace-scoped fallback as well as the refusal
(`views.rs`'s own doc says so), and `offered` reads it as *withhold every mutation for the
life of the run*. A developer with a `RoleBinding` in `payments` — the most common
non-admin RBAC shape there is — has `may_i` answering `Yes` for both keys and no way to
press either.

### 4 — two caveats at the floor delete the list the badge is still counting

```
│▸ ALERTS         1 ●│  ⚠ This computer and the cluster disagree about the     │
│  RESOURCES         │  time by 11 minutes (this one is behind), so recent     │
│   workloads        │  times are missing and older ones can read smaller      │
│   network          │  than they really are.                                  │
│   storage          │                                                         │
│   config           │  there is something at                                  │
│   cluster          │  /home/…/.local/state/k8rs/audit.log (under your        │
│  ANALYSIS          │  home directory) that is not an ordinary file — a       │
│                    │  pipe, a device, a directory or a link — and k8rs will  │
│                    │  not write its audit log into it — every change k8rs    │
│                    │  makes is written to that log before it is sent, so     │
│                    │  k8rs will not change anything until that is fixed,     │
│                    │  and reading your cluster still works                   │
the card is on screen: false
```

With the audit banner alone the card survives (7 body rows left). It takes two. `banner`
is fed the **absolute** path, not the mockup's `~/…`, which costs one extra wrap line.

### 6 — the sentence `ops::audit_log` actually returns

```
HOME >>there is something at /tmp/…/audit.log (under your home directory) that is not an
ordinary file — … — and reading your cluster still works<<
XDG  >>… (from $XDG_STATE_HOME) …<<
```

No tilde (the function writes `path.to_string_lossy()`), no `($HOME)` clause, no trailing
full stop.

## Files read

`src/ui.rs` · `src/views.rs` · `src/ui_tests.rs` · `src/ops.rs` · `screens/states.md` ·
`screens/widgets.md`

## Round two — the fixes, re-measured

The copy at `/home/shyuuhei/k8rs-review` was refreshed with
`cp -a src/. screens/. tests/. Cargo.toml Cargo.lock`; `diff -rq` against the working
tree's `src` and `screens` came back identical before any probe was added. `df -h /home`:
`954G 89G 865G 10%`. Probes appended to the copy only, run with
`CARGO_TARGET_DIR=/home/shyuuhei/k8rs-review/target cargo test --quiet r2_p -- --nocapture --test-threads=1`.
`REAL_OPEN` is `open_log`'s *could not open* shape with the `(under your home directory)`
clause.

| Probe | Screen | Result |
|---|---|---|
| p01 | `Denied(--namespace, [oom])`, Live/Live | `offer Act may_mutate true`; `s scale  r restart` on the footer |
| p02 | clock + `REAL_OPEN`, cursor on card 1 | `card on screen: true  tail on screen: true` |
| p03 | clock + `REAL_OPEN` + namespace `Denied` | namespace sentence `false`, *One node check is off* `false`, no `…`, card `true`; clock + namespace only: check `true` |
| p04 | expired `Denied` + `REAL_OPEN` | alone: renew/command/staleness label/card all `true`; with audit: renew `false` command `false` staleness label `false` card `true` |
| p05 | `Link::Lost` + `REAL_OPEN` + clock set | staleness label `true`, clock `false`, card `true` |
| p06 | clock + `REAL_OPEN`, two cards, cursor on card 2 | first card `false`, second card `false`; sidebar `1 ● 1 ▲` |
| p07 | browser `Loading` + `REAL_OPEN` | rows starting the audit sentence: `2` |
| p08 | logs tab `Loading` + `REAL_OPEN` | audit sentence on the loading tab: `true` |
| p09/p12 | Alerts `Loading` with both paragraphs + audit | tail `false`, `…` `true` |
| p09/p12 | `Ready([])` with the file's three paragraphs + audit | tail `false`, consequence `false`, *Worth a look* kept `true` |
| p10 | detail open / Analysis / refused empty kind / expired loading kind / expired + detail | `Move{false} false` / `Move{false} false` / `Filter{false}` / `Nothing{true}` `X switch cluster  ? all keys  q quit` / detail footer without `X` |
| p11 | `REAL_OPEN` + namespace `Denied`, no clock | first paragraph `true`, *One node check is off* `false`, `…` `true` |
| p13 | expired alone, two cards, cursor on card 2 | card 2 (cordon, 4 lines) drawn — one banner leaves room |
| ops | `open_log` on a 0500 directory, `Source::Home` | `… at <path> (under your home directory): Permission denied (os error 13) — … still works`, 291 characters |

### Three banners, and the one that vanishes

```
AUDIT + NAMESPACE SCOPE
│  or ask for cluster-wide read access.…                  │
namespace first paragraph true  'One node check is off' false  cut mark true

EXPIRED + AUDIT
│  ⚠ Your login expired.                                  │
│    The cluster still knows who you are, but the login   │
│    token your kubeconfig creates has timed out.…        │
EXPIRED + AUDIT: renew false command false stale-label false card true
```

With clock + audit + namespace scope the namespace banner is not cut but gone, with no `…`.

### One keypress blanks the list again

```
--- CLOCK + AUDIT, cursor on the second card ---
│▸ ALERTS     1 ● 1 ▲│  …                                  │
│                    │  until that is fixed, and reading your cluster still │
│                    │  works                                                  │
│                    │                                                         │
first card false  second card false
```

Only the first card is trimmed to the 3-row floor; `List` moves its offset to the selected
card and skips it whole when it does not fit.
