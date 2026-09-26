# The `--read-only` header mark and Help swap, operator read — measurements (2026-09-13)

`k8s-admin`, step 6 on Phase 11's last box (NOTES § D265), over the uncommitted tree on
`87ace3a`. No cluster was used. Everything ran in a private copy of the working tree at
`$HOME/.cache/k8s-admin-review/k8rs` with `CARGO_TARGET_DIR=$HOME/.cache/k8s-admin-review/target`;
the shared tree's `src/ui.rs` and `src/ui_tests.rs` hashed identical before the copy and after the
runs (`87c3c1b8…`, `755a3381…`). Four scratch tests were appended to the copy's `src/ui_tests.rs`
only, built from that file's own helpers (`screen`, `render`, `rows`, `app`, `changing`, `fed`,
`dead_log`). Findings are in the report to the PM; this file holds what was run and what it printed.

## 1. `?` under `Writes::Live` while `offered` answers `Move`

`screen.clock = Some(<skew sentence>)`, then `screen.link = Link::Lost`, then `Link::Expired`, each
with `writes = Writes::Live`; `offered(&app(), &screen)` printed, then `?` rendered at 80×24 and
body rows 13–17 printed.

```
$ cargo test --bin k8rs zz_probe -- --nocapture
PROBE[clock skew] writes=Live offered=Move { switch: false }
PROBE[clock skew] │  Changing things (each one asks first, and shows the command)                │
PROBE[clock skew] │    s       run more or fewer copies       (scale)                            │
PROBE[clock skew] │    r       restart, at its own pace       (rollout restart)                  │
PROBE[clock skew] │    ctrl-d  delete — you type the name to confirm                             │
PROBE[link lost] writes=Live offered=Move { switch: false }
PROBE[link lost] │  Changing things (each one asks first, and shows the command)                │
PROBE[link lost] │    s       run more or fewer copies       (scale)                            │
PROBE[link lost] │    r       restart, at its own pace       (rollout restart)                  │
PROBE[link lost] │    ctrl-d  delete — you type the name to confirm                             │
PROBE[login expired] writes=Live offered=Move { switch: true }
PROBE[login expired] │  Changing things (each one asks first, and shows the command)                │
PROBE[login expired] │    s       run more or fewer copies       (scale)                            │
PROBE[login expired] │    r       restart, at its own pace       (rollout restart)                  │
PROBE[login expired] │    ctrl-d  delete — you type the name to confirm                             │
test result: ok. 2 passed; 0 failed
```

## 2. The header row in the three link states whose mockups draw no permission word

`screen.context` set to the right zone `screens/states.md` draws at l.158, l.209 and l.250/356,
`alerts = Pane::Loading`, both `Writes::Live` and `Writes::ReadOnly`; row 0 printed between bars.

```
PROBE[connecting Live] | nodes 3/3                            k8rs    ctx: prod-eu · connecting… · admin|
PROBE[connecting ReadOnly] | nodes 3/3                                ctx: prod-eu · connecting… · read-only|
PROBE[disconnected Live] | nodes 3/3                       ctx: prod-eu · ⚠ disconnected, retrying · admin|
PROBE[disconnected ReadOnly] | nodes 3/3                   ctx: prod-eu · ⚠ disconnected, retrying · read-only|
PROBE[expired Live] | nodes 3/3                                ctx: prod-eu · ⚠ login expired · admin|
PROBE[expired ReadOnly] | nodes 3/3                            ctx: prod-eu · ⚠ login expired · read-only|
```

The mockups those zones come from, as committed:

```
$ grep -rn 'ctx: \|choose a cluster' screens/*.md | grep -v '^\S*:[0-9]*:\s*[-*|]' | grep -v 'admin\|read-only'
screens/states.md:158: nodes …                        k8rs      ctx: prod-eu · connecting…
screens/states.md:209: nodes 3/3 (40s ago)          ctx: prod-eu · ⚠ disconnected, retrying
screens/states.md:250: nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
screens/states.md:331: nodes …                        k8rs      ctx: prod-eu · ⚠ login expired
screens/states.md:356: nodes 3/3                      k8rs     ctx: prod-eu · ⚠ login expired
screens/states.md:676:header has nowhere to put it beside `nodes 3/3` and `ctx: prod-eu`. At 80 it
screens/states.md:1192: nodes 3/3 (2 min ago)                 ctx: prod-eu · ⚠ login expired
screens/context.md:481:3. The header reads `ctx: staging · connecting…` and the body shows the
$ grep -n 'login expired\|disconnected, retrying\|connecting…' src/ui_tests.rs   # header comparisons: none
```

## 3. § All three at once under `Writes::Unaudited`, Alerts and `?`

The same `all` screen `the_audit_sentence_gives_way_first_and_the_pane_s_own_reason_never_does`
builds (clock + namespace denial + `Unaudited(dead_log())`), context `ctx: prod-eu · ns: payments · live`.

```
$ cargo test --bin k8rs zz_probe_all_three -- --nocapture
PROBE3 alerts header | nodes 3/3                        ctx: prod-eu · ns: payments · live · read-only|
PROBE3 alerts footer |│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                │|
PROBE3 alerts frame mentions 'audit': false
PROBE3 help frame mentions 'audit': false
PROBE3 help line |│  read-only mode — nothing can be changed from here                           │|
test result: ok. 1 passed; 0 failed
```

## 4. The longest reachable tails behind an EKS ARN at 80 columns, `insecure = true`

```
$ cargo test --bin k8rs zz_probe_arn -- --nocapture
PROBE4 ReadOnly running=false |…n-eu · ns: payments · ⚠ disconnected, retrying · read-only · ⚠ TLS not verified|
PROBE4 Live running=true |…er/production-eu · ns: payments · live · admin · ⚠ TLS not verified · changing…|
test result: ok. 1 passed; 0 failed
```

## 5. Who writes `Screen::insecure` and `Screen::writes` in product code

```
$ grep -rn 'Screen {' src/ | grep -v '_tests'          # no output
$ grep -n 'TLS\|insecure' todo.md
2892:      use** — name, API server host, `insecure-skip-tls-verify`, and the tag:
3283:**🔒 Security gate:** TLS verification is never disabled by us; if the
3284:kubeconfig sets `insecure-skip-tls-verify` it is honoured *and surfaced*, not
```

---

# Round two (2026-09-13, after D265 rulings 4–8)

Same method, over the uncommitted tree whose `src/ui.rs` hashes `48cc87bc…` and `src/ui_tests.rs`
`69719535…` (checked identical in the copy at `$HOME/.cache/k8s-admin-review/r2`, own
`CARGO_TARGET_DIR`). Four scratch tests appended to the copy only.

## R2.1 Section 1's frames again (`writes = Live`)

```
$ cargo test --bin k8rs zz_r2_probe -- --nocapture --test-threads=1
R2P1[clock skew] writes=Live offered=Move { switch: false }
R2P1[clock skew] │  Changing things (paused while the clock is off)                             │
R2P1[clock skew] │    s       run more or fewer copies       (scale)                            │
R2P1[link lost] writes=Live offered=Move { switch: false }
R2P1[link lost] │  Changing things (paused while disconnected, retrying)                       │
R2P1[login expired] writes=Live offered=Move { switch: true }
R2P1[login expired] │  Changing things (paused until you renew your login)                         │
```

## R2.2 Section 3's frame again (§ All three at once, `Unaudited`)

```
R2P3 alerts header | nodes 3/3                        ctx: prod-eu · ns: payments · live · read-only|
R2P3 alerts frame mentions 'audit': false
R2P3 help frame mentions 'audit': true
R2P3 help │  Changing things (off for this whole run)                                    │
R2P3 help │    k8rs could not open its audit log — fix that, then start k8rs again       │
```

## R2.3 Help against `offered` + `may_mutate`, every run-level combination

3 writes × 3 links × clock none/some × running false/true × refused none/all, on Alerts with a
card. A case is a mismatch when Help draws the plain heading with an `s` row but `may_mutate`
says no, or the reverse, or any body row passes 78 columns.

```
R2P4 cases=72 mismatches=0
R2P4 heading seen: "  Changing things (each one asks first, and shows the command)"
R2P4 heading seen: "  Changing things (off for this whole run)"
R2P4 heading seen: "  Changing things (paused until you renew your login)"
R2P4 heading seen: "  Changing things (paused while a change is running)"
R2P4 heading seen: "  Changing things (paused while disconnected, retrying)"
R2P4 heading seen: "  Changing things (paused while the clock is off)"
```

## R2.4 Combinations, widths, empty context

```
R2P5 Unaudited(<dead_log>) Expired running=false
R2P5   │    X            switch cluster                                               │
R2P5   │  Changing things (off for this whole run)                                    │
R2P5   │    k8rs could not open its audit log — fix that, then start k8rs again       │
R2P5 ReadOnly Lost running=false
R2P5   │  Changing things (off for this whole run)                                    │
R2P5   │    k8rs was started with --read-only — start it again without it             │
R2P5 Live Lost running=true
R2P5   │    X            switch cluster (paused while a change is running)            │
R2P5   │  Changing things (paused while a change is running)                          │
R2P5 why width 61 (+4 indent = 65): k8rs was started with --read-only — start it again without it
R2P5 why width 67 (+4 indent = 71): k8rs could not open its audit log — fix that, then start k8rs again
R2P5 empty context header | nodes 3/3                            k8rs                                 admin|
R2P5 empty context ro+tls | nodes 3/3                            k8rs        read-only · ⚠ TLS not verified|
test result: ok. 4 passed; 0 failed
```

## R2.5 The link-state headers at 80 columns, against the edited mockups

```
$ cargo test --bin k8rs the_header_of_every_link_state_is_the_pages_own -- --nocapture
## Still loading · Live
 nodes …                              k8rs    ctx: prod-eu · connecting… · admin
## Still loading · ReadOnly
 nodes …                                  ctx: prod-eu · connecting… · read-only
## Your login expired · Live
 nodes …                                  ctx: prod-eu · ⚠ login expired · admin
## Your login expired · Live
 nodes 3/3                                ctx: prod-eu · ⚠ login expired · admin
test ui::tests::the_header_of_every_link_state_is_the_pages_own ... ok
```

Header row width against the frame row under it, for every mockup whose header names a context
(Python `unicodedata` width, `git show HEAD:` for the committed file):

```
screens/states.md NOW  [(158, 76, 70), (209, 77, 70), (250, 77, 70), (331, 80, 70), (356, 79, 70), (795, 72, 70), (893, 72, 70), (945, 72, 70), (1072, 73, 70), (1192, 80, 70), (1295, 72, 70)]
screens/states.md HEAD [(331, 72, 70), (356, 71, 70), (793, 72, 70), (891, 72, 70), (943, 72, 70), (1070, 73, 70), (1293, 72, 70)]
screens/help.md    NOW [] HEAD []
screens/context.md NOW [] HEAD []
```

## R2.6 What Phase 12 can read `insecure-skip-tls-verify` from

```
$ awk 'NR>=6953 && NR<=7175' src/k8s.rs | grep -n 'pub(crate) [a-z_]*:'      # Session's fields
8:    pub(crate) client: Client,
15:    pub(crate) version: Result<String, kube::Error>,
19:    pub(crate) served: Result<Served, kube::Error>,
22:    pub(crate) watches: Vec<BoxStream<'static, Update>>,
33:    pub(crate) renewal: Option<String>,
54:    pub(crate) context: Option<String>,
63:    pub(crate) namespace: Option<String>,
73:    pub(crate) coverage: Coverage,
83:    pub(crate) client_certificate: Option<Vec<u8>>,
116:    pub(crate) skew: Option<SignedDuration>,
150:    pub(crate) serving_expiry: Serving,
161:    pub(crate) kinds: Vec<Browsable>,
165:    pub(crate) capabilities: Option<BTreeSet<Capability>>,
$ grep -n '    pub fn \|    pub async fn ' kube-client-4.2.0/src/client/mod.rs
108:    pub fn supports_stream_close(&self) -> bool {
113:    pub fn into_stream(self) -> WebSocketStream<TokioIo<hyper::upgrade::Upgraded>> {
153:    pub fn new<S, B, T>(service: S, default_namespace: T) -> Self
175:    pub fn with_valid_until(self, valid_until: Option<Timestamp>) -> Self {
180:    pub fn valid_until(&self) -> &Option<Timestamp> {
201:    pub async fn try_default() -> Result<Self> {
210:    pub fn default_namespace(&self) -> &str {
217:    pub async fn send(&self, request: Request<Body>) -> Result<Response<Body>> {
240:    pub async fn connect(&self, request: Request<Vec<u8>>) -> Result<Connection> {
281:    pub async fn request<T>(&self, request: Request<Vec<u8>>) -> Result<T>
295:    pub async fn request_text(&self, request: Request<Vec<u8>>) -> Result<String> {
307:    pub async fn request_stream(&self, request: Request<Vec<u8>>) -> Result<impl AsyncBufRead + use<>> {
318:    pub async fn request_status<T>(&self, request: Request<Vec<u8>>) -> Result<Either<T, Status>>
340:    pub async fn request_events<T>(
414:    pub async fn apiserver_version(&self) -> Result<k8s_openapi::apimachinery::pkg::version::Info> {
420:    pub async fn list_api_groups(&self) -> Result<k8s_meta_v1::APIGroupList> {
443:    pub async fn list_api_group_resources(&self, apiversion: &str) -> Result<k8s_meta_v1::APIResourceList> {
450:    pub async fn list_core_api_versions(&self) -> Result<k8s_meta_v1::APIVersions> {
456:    pub async fn list_core_api_resources(&self, version: &str) -> Result<k8s_meta_v1::APIResourceList> {
493:    pub async fn list_api_groups_aggregated(&self) -> Result<APIGroupDiscoveryList> {
525:    pub async fn list_core_api_versions_aggregated(&self) -> Result<APIGroupDiscoveryList> {
$ grep -n accept_invalid_certs kube-client-4.2.0/src/config/mod.rs
161:    pub accept_invalid_certs: bool,
193:            accept_invalid_certs: false,
275:            accept_invalid_certs: false,
324:        let accept_invalid_certs = loader.cluster.insecure_skip_tls_verify.unwrap_or(false);
341:            accept_invalid_certs,
$ grep -n 'Client::try_from(config)' src/k8s.rs
7341:    match Client::try_from(config) {
$ grep -n 'Frozen after.*k8s.rs. —' todo.md
3624:**Frozen after:** `k8s.rs` — **with two named exceptions, because the check this
$ sed -n 7638,7639p src/k8s.rs
pub(crate) fn contexts(kubeconfig: &Kubeconfig, asked_for: Option<&str>) -> Vec<Choice> {
    let current = wanted(kubeconfig, asked_for).map(|entry| entry.name.as_str());
```

`contexts` resolves a context's cluster with `.iter().find(|entry| entry.name == context.cluster)`,
the same first-wins lookup `ConfigLoader` uses.

## R2.7 The skew is read once

```
$ sed -n 7057,7059p src/k8s.rs
    /// **Read once, at connect, and never refreshed** — the ceiling [`Identity`] states for the
    /// three facts beside it. A machine whose clock is fixed while k8rs is open keeps saying so
    /// until the next [`connect`].
```

---

# Round three (2026-09-13, after D265 rulings 4 and 8 were amended)

Over the uncommitted tree whose `src/ui.rs` hashes `f2635315…` and `src/ui_tests.rs` `e9a46d03…`
(checked identical in the copy at `$HOME/.cache/k8s-admin-review/r3`, own `CARGO_TARGET_DIR`). One
scratch test appended to the copy only.

## R3.1 The five Help headings, rendered at 80×24

```
$ cargo test --bin k8rs zz_r3_probe -- --nocapture
R3P1[clock] offered=Move { switch: false } widest body row=72
R3P1[clock] │  Changing things (paused — the clocks disagree; press X to check again)      │
R3P1[expired] offered=Move { switch: true } widest body row=62
R3P1[expired] │  Changing things (paused — renew your login, then press X)                   │
R3P1[lost] offered=Move { switch: false } widest body row=62
R3P1[lost] │  Changing things (paused while disconnected, retrying)                       │
R3P1[read-only] offered=Move { switch: false } widest body row=74
R3P1[read-only] │  Changing things (off for this whole run)                                    │
R3P1[read-only] │    k8rs was started with --read-only — quit and start it again without it    │
R3P1[unaudited] offered=Move { switch: false } widest body row=71
R3P1[unaudited] │  Changing things (off for this whole run)                                    │
R3P1[unaudited] │    k8rs could not open its audit log — fix that, then start k8rs again       │
```

## R3.2 What `X` then `⏎` does on the connected context

`contexts_of(FOUR)` (current row `prod-eu`), `views::Picker::new(rows, connection)`, then
`picker.chosen(rows)` with the cursor where it opens.

```
R3P2 Live(Some("prod-eu")): cursor opens on row Some(0) (current row Some(0)); ⏎ answers Close
R3P2 Dropped(Some("prod-eu")): cursor opens on row Some(0) (current row Some(0)); ⏎ answers Connect(Some("prod-eu"))
test result: ok. 1 passed; 0 failed
$ sed -n 1013,1015p src/views.rs
        let before = match &self.connection {
            Connection::Live(_) if row.current => return Chosen::Close,
            Connection::Live(name) | Connection::Dropped(name) => Before::Connected(name.clone()),
$ grep -n 'The caller supplies' todo.md
4603:      renewal program. The caller supplies `views::Connection`. The picker
$ sed -n 1137,1138p src/ui.rs
        Link::Live => clock(screen)
            .map(|_| Held::Paused("paused — the clocks disagree; press X to check again")),
```

## R3.3 Mockup widths

```
screens/states.md header rows against the frame row under them (width, frame):
158 70 70 · 209 70 70 · 250 70 70 · 331 70 70 · 356 70 70 · 1192 70 70
795 72 70 · 893 72 70 · 945 72 70 · 1072 73 70 · 1295 72 70      (unchanged from HEAD)
screens/help.md l.95–109 (§ Under a dead-writes run): every row 80
```
