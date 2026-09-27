# 2026-09-27 — a refused `get /apis`: which faults reach the sentence, and what it then claims

Operator review of Phase 13's fourth error-state box (`main.rs`'s `unread` and
`discovered`, `ui::discovery`, `views::sidebar`'s `NavItem::Unread`,
`screens/states.md` § *k8rs could not read what this cluster serves*, NOTES § D296).
**No cluster and nothing on the test host** — `tester` held the mirror. Everything
below is read off the working tree, off the vendored dependency source, or is
arithmetic over the shipped format string.

Tree: `development`, working tree modified (`NOTES.md screens/resources.md
screens/states.md screens/widgets.md src/main.rs src/main_tests.rs src/ui.rs
src/ui_tests.rs src/views.rs src/views_tests.rs`) on top of `4106b83`.

## 1. The sentence, as `unread` assembles it

`src/main.rs:9797-9804`, with `views::because`'s `Fault::Refused` arm
(`src/views.rs:4057`) in the middle clause:

```
RESOURCES has nothing under it: the role this kubeconfig uses needs to `get /apis`.
The findings and the reports are read separately and are unaffected. k8rs asks for
this list once, at startup, so start k8rs again once that is fixed.
```

## 2. Which requests `Session::served` actually makes

```
$ grep -n "async fn served" -A 4 src/k8s.rs
8328:async fn served(client: &Client) -> Result<Vec<(ApiResource, ApiCapabilities)>, kube::Error> {
8329:    let aggregated = Discovery::new(client.clone()).run_aggregated().await?;
```

`run_aggregated` is two requests, each behind its own `?`
(`~/.cargo/registry/.../kube-client-4.2.0/src/discovery/mod.rs:171-199`):

```
$ sed -n '171,199p' .../kube-client-4.2.0/src/discovery/mod.rs
        // Query /apis for all non-core groups (single request)
        let apis_discovery = self.client.list_api_groups_aggregated().await?;
        ...
        // Query /api for core group (single request)
            let core_discovery = self.client.list_core_api_versions_aggregated().await?;
```

```
$ grep -n 'Request::get("/api")\|Request::get("/apis")' .../kube-client-4.2.0/src/client/mod.rs
494:            Request::get("/apis")
528:            Request::get("/api")
```

So the error in `Session::served` can be an answer about **`/api`**, and the
sentence names `/apis` at both call sites — `src/main.rs:2942` (`greeting`) and
`src/main.rs:9802` (`unread`). The fallback path (`pairs.is_empty()`) has only
`/apis` behind a `?`; the per-group errors under it are swallowed by
`if let Ok(group)` (`src/k8s.rs:8340-8344`).

`/api` and `/apis` are two separate `nonResourceURLs` entries — `docs/security.md:166`
lists both, plus `/api/*` and `/apis/*` — so a Role granting only what this sentence
names refuses `/api` still. (Whether a trailing `*` also covers the bare path was
**not** verified this turn; the finding does not turn on it.)

## 3. `served` is read once, and where

```
$ grep -n "\.served\b" src/main.rs src/k8s.rs | grep -v _tests
src/main.rs:2923:    match &session.served {          # greeting, stderr
src/main.rs:3255:        && unanswered(session.served.as_ref().err().map(k8s::fault)))
src/main.rs:5475:    let served = match &session.served {
src/main.rs:8334:        .served                          # connected -> console.kinds
src/main.rs:8343:    console.discovery = session.served.as_ref().err().map(|error| {
src/k8s.rs:8118:    let served = served(&client).await.map(|pairs| Served {
```

One producer (`k8s.rs:8118`, inside `connect`); nothing re-runs it on a timer or a
keypress. `console.discovery` and `console.kinds` are written only in `connected`
(`main.rs:8333-8346`) and cleared together in `switched` (`main.rs:8453-8454`).
`screens/context.md:53` — `⏎` on a live `(current)` picker row "closes without
doing anything" — so within a session the only re-read is a switch to a *different*
context and back.

## 4. Which faults can reach the sentence

`Fault::Kubeconfig`, `NoContext` and `BadEntry` cannot (the client is already
built); `Conflict` and `Unfinished` have no producer on this path
(`src/k8s.rs:838-1000`, `watch_fault` has no `Unfinished` arm). Reachable:
`Refused`, `Expired`, `NoCredential`, `Gone`, `Rejected`, `Unanswered`.
`Fault::standing` (`src/k8s.rs:1023-1050`) is `true` for all of them except
`Unanswered`.

## 5. The link gate, and the frame where two banners disagree

`ui::discovery` (`src/ui.rs:3943-3949`) draws under `Live | Connecting` only.
`linked` (`src/main.rs:9626-9678`) computes the link from the **watches'** faults
and not from `served`:

- `Expired` / `NoCredential` on a watch → `Link::Expired` → sentence withheld.
- `Unanswered`/`Unfinished` on a watch **and** no other watch delivering →
  `Link::Lost` → withheld.
- A **refused** watch is on neither side of `dropped`/`answering`; with a snapshot
  present the function returns `Link::Live`.

`Fault::standing` is `true` for `Refused`, so `Watch::settled` is satisfied and
`Store::snapshot` answers `Some` with an empty snapshot; `drawn`
(`src/main.rs:9194-9207`) then builds `Pane::Denied(said, [])` from `unreadable`'s
first line, and `content`'s `Pane::Denied` arm (`src/ui.rs:3603-3606`) stacks
`caveats`, whose fourth rank is this sentence (`src/ui.rs:3658`). A kubeconfig
refused on both `/apis` and pods therefore draws, two rows apart, under a header
reading `live`:

```
▲ k8rs is not getting pods from this cluster: the role this kubeconfig uses needs
  to `list` and `watch` pods. It keeps asking, and until that works nothing here
  about them can be trusted

RESOURCES has nothing under it: … The findings and the reports are read separately
and are unaffected. …
```

(`unreadable`'s line: `src/main.rs:2858-2863`.)

The same two sentences can also coexist for one frame with `Link::Connecting`:
`served` carries `Expired` or `NoCredential`, the watches have not yet reported
theirs, and `Store::snapshot` is `None`.

## 6. What `capabilities` gates today

```
$ grep -rn "capabilities\|Capability::" src/analysis.rs src/rules.rs src/views.rs | grep -v _tests
(no output)
$ grep -rn "Capability" src/main.rs | grep -v '///'
src/main.rs:2926:            said.push(match &served.capabilities {
```

`Served::capabilities` is read by `greeting`'s stderr clause and by nothing else;
`rules::Metrics` is decided by the metrics poll's own fault
(`src/k8s.rs:3124`, `Fault::Gone => Metrics::NotInstalled`). `docs/security.md:161-162`
says the discovery grant buys "the capability probe that decides which analysis
rows can answer at all".

## 7. Row arithmetic, measured against the shipped string

`ui::SIDEBAR` is `20` (`src/ui.rs:86`), `MARKER` is `"▸ "` (`src/ui.rs:152`,
2 columns), indent 1 for a group row (`src/ui.rs:3448`) — 17 columns, matching
`screens/states.md:2101`. `could not read` is 14.

Wrap of the assembled sentence per reachable fault clause (`textwrap`, not
`ui::paragraphed`, but it reproduces both `states.md` mockups line for line
including the `get` / `/apis` break at 39):

```
$ python3 - <<'EOF'   # head = unread()'s format string, clause = because()'s arm
Refused            chars= 234  lines@39= 7  lines@45=6
Gone               chars= 254  lines@39= 7  lines@45=6
Unanswered         chars= 239  lines@39= 7  lines@45=6
Expired(named)     chars= 276  lines@39= 8  lines@45=7
Expired(none)      chars= 259  lines@39= 7  lines@45=7
NoCredential       chars= 266  lines@39= 8  lines@45=7
Rejected(no msg)   chars= 334  lines@39= 9  lines@45=8
EOF
```

`screens/states.md:2252-2255` prices the still-loading arm at 13 of 13 for the
7-line clause. Three reachable clauses are 8 or 9 lines at 39.

Three candidate rewordings of the second clause, measured against the same budget:

```
shipped   chars= 234 lines@39=7 lines@45=6
opt A     "… are read separately, so this does not change them."        245  7  6
opt B     "… are read separately and this changes nothing about them."  251  7  6
opt C     "Nothing else is affected: … are read separately."            241  7  6
```

## 8. Bounds on the one arm that renders server text

`k8s::said` (`src/k8s.rs:1186-1191`) selects `Status.message`, bounded to
`FREE_TEXT` = 4096 (`src/k8s.rs:213`). Only `because`'s `Fault::Rejected` arm reads
it (`src/views.rs:4103`). `banner` (`src/ui.rs:4094-4134`) cuts to the row budget
behind a visible `CUT`, and `caveats` breaks below two rows — so the pane is
bounded by rows, not by the sentence's length.

## 9. Read and found nothing to report

- `views::sidebar` (`src/views.rs:895-921`): `None` pushes exactly one
  `NavItem::Unread`, `Some` keeps the five groups. `NavItem::selectable`
  (`src/views.rs:868-870`) refuses it, `views::selectable` is what the cursor walks
  (`src/ui.rs:3424-3433`), and `NavItem::Unread` has no `Action` arm
  (`src/views.rs:3449`) — no landable row, no `⏎`.
- `discovered` (`src/main.rs:9815-9817`) and `Screen::served`
  (`src/ui.rs:1250-1252`) are the same join spelled twice over two different types,
  so the frame and the key handlers cannot disagree.
- The browser cannot draw this sentence: `caveats` is stacked there
  (`src/ui.rs:4659`) but `View::Resources(nth)` is reachable only through a
  `NavItem::Kind` row, and `App::switched` resets the view.
- `analysis` (`src/ui.rs:5056`) stacks no caveats, and `content`'s Analysis arm
  passes `None` for this sentence (`src/ui.rs:3575`) — matching
  `screens/states.md`'s "the sidebar's row alone".
- The failed-switch frame (`Link::Unconnected`) draws five empty group rows, which
  is `screens/context.md:895-905`'s own ruling with `↑↓`/`⏎` withdrawn from the
  footer — not the shape D296's row-gate paragraph argues against, which is
  `Link::Lost` keeping both keys.
- Load: no new request, no watch, no poll. One `format!` at connect; one
  `Stripped::of` per frame at `src/main.rs:9294`.
- `PRIOR-ART § B4` (a denied permission degrades one feature) — this box adds no
  permission and names the path; `§ C1` (a generic message eating a typed error) —
  the middle clause is `because`'s per-fault sentence, with no fallback string.

## 10. What the console's `Pane::Denied` reason actually is on a scoped run

The ordinary namespaced kubeconfig is the trigger, not a contrived one. A
cluster-scoped node watch cannot be granted by a namespaced `Role`, so
`Store::troubles` (`src/k8s.rs:2151-2213`, no suppression for a scoped run) always
reports the node watch, `unreadable`'s first line becomes `Pane::Denied`'s `said`
(`src/main.rs:9203-9207`), and the console frame carries:

```
▲ k8rs is not getting nodes from this cluster: the role this kubeconfig uses needs
  to `list` and `watch` nodes. It keeps asking, and until that works nothing here
  about them can be trusted

RESOURCES has nothing under it: … The findings and the reports are read separately
and are unaffected. …
```

The namespace-scope paragraphs the new § *Where it sits when something else is
already queued* mockup uses as the co-occupant are **stderr-only** today:

```
$ grep -n "check_switched_off\|scoped_because" src/main.rs | grep -v '///'
903:    if let Some(off) = check_switched_off(input.snapshot.namespace_scope.as_deref()) {
1030:fn check_switched_off(namespace_scope: Option<&str>) -> Option<String> {
3314:fn scoped_because(session: &k8s::Session, stopping: bool) -> Option<String> {
3675:    if let Some(narrowed) = scoped_because(&session, stopping) {
```

Line 903 builds the `--once` report body; line 3675 writes to `err`. `ui::caveats`'
four ranks are `clock`, the pane's own `said`, `writes.said()` and `discovery`
(`src/ui.rs:3658`) — there is no scope slot among them, and `views::scope` /
`views::next_step` reach the console only through the picker's failure box
(`src/ui.rs:3278-3287`).
