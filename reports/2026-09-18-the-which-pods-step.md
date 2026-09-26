# The which-pods step and the pinned blocks — measurements behind the operator read (2026-09-18)

Everything below was measured against the **uncommitted working tree**, on the test host, in a
copy of the tree under `$HOME` with its own `CARGO_TARGET_DIR`. No cluster was created, nothing
was written to one, and no committed file was produced by any of it. The copy and its build
scratch were removed when the run ended.

```
$ rsync -a --delete --exclude=/target --exclude='/mutants.out*' ~/GIT/k8rs/ ubuntu:k8rs-review/
$ ssh ubuntu 'cd ~/k8rs-review && CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --bin k8rs'
test result: ok. 1457 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 73.78s
$ ssh ubuntu 'du -sh ~/k8rs-review ~/.cache/k8rs-review-target; rm -rf ~/k8rs-review ~/.cache/k8rs-review-target'
89M     /home/murat/k8rs-review
3.2G    /home/murat/.cache/k8rs-review-target
```

The measurements are eight `#[test]`s appended to `src/ui_tests.rs` **in that copy only**; they
render frames through `TestBackend` at the 80×24 floor with the tree's own helpers
(`stepping`, `detailed`, `pinned_on`, `cluster`, `rows`, `celled`) and print them. They were
never written into `~/GIT/k8rs`.

## 0. The host was not quiet, and one number here is affected

At the start of the timing run the host read `load average: 78.88` — a kind cluster up for ~20 h
and another session's `cargo test` in `~/k8rs-src`, plus a `--release` build this review had
started. **The release build was stopped** (only its own two pids, by name, leaving the other
session's alone) and the timing in § 6 was re-taken in the debug profile at
`load average: 14.68`. Every other measurement in this file is a rendered `Buffer` compared
against itself and carries no wall clock, so load does not reach it (D84, D185).

## 1. Every `Finding::title` the rule set actually produces, against the column a pod row has

`rules::analyze` over the committed captures (`ui_tests::cluster()`), then each title through
`ui::cut(title, room, 1)` at the two widths a pod row can offer: `room = 30` (the slot a
`web-7d9f4bc86d-x2k9p`-length name leaves at `region = 55`) and `room = 20` (`FACT_FLOOR`).

```
$ cargo test --bin k8rs m1_the_trailing -- --nocapture
== every distinct Finding::title analyze() produced over tests/fixtures ==
 82 cols | room=30 -> "A container can drive the…"    | room=20 -> "A container can…"   | full: "A container can drive the container runtime, which is full control of that machine"
 80 cols | room=30 -> "A container has the whole…"    | room=20 -> "A container has the…" | full: "A container has the whole filesystem of the machine it runs on mounted inside it"
 74 cols | room=30 -> "Container keeps crashing, and…"| room=20 -> "Container keeps…"    | full: "Container keeps crashing, and each restart waits longer (CrashLoopBackOff)"
 68 cols | room=30 -> "The last run on record failed…"| room=20 -> "The last run on…"    | full: "The last run on record failed — exit 1 (the application's own error)"
 77 cols | room=30 -> "This node has stopped…"        | room=20 -> "This node has…"      | full: "This node has stopped responding — nothing on it can be trusted until it does"
 37 cols | room=30 -> "This node refuses new pods…"   | room=20 -> "This node refuses…"  | full: "This node refuses new pods (cordoned)"
titles: 6
```

Field values the finding turns on: **shortest title 37 columns, longest 82, none at or under 20.**
The two strings `FACT_FLOOR`'s doc comment measures against — `ran out of memory` and
`image pull failed`, 17 columns each — appear nowhere in `src/rules.rs`; they are
`screens/detail.md`'s mockup words. Titles not in this fixture set, read off `src/rules.rs`:
`src/rules.rs:4415` is 87 columns, `:4507` 85, `:6258` 90, `:6330` 75, `:5221` 67, `:5544` 59.

## 2. The cards the committed fixture set produces

```
$ cargo test --bin k8rs m2_the_real_cards -- --nocapture
== cards from analyze(cluster()) ==
owner "-"/k8rs-worker3        kind=Node affected=0 count=None findings=1
owner "default"/broken-crashloop kind=Pod  affected=1 count=None findings=2
owner "default"/broken-hostpath  kind=Pod  affected=1 count=None findings=2
owner "default"/broken-socket    kind=Pod  affected=1 count=None findings=1
owner "-"/k8rs-worker         kind=Node affected=0 count=None findings=1
cards with affected >= 2 (the only ones the step opens on): 0
```

## 3. The same pinned block, drawn into four different panes

One finding, one pod, `Detail::card` set. The rows printed are the three the block occupies, with
their drawn column count.

```
$ cargo test --bin k8rs m8_the_same_block -- --nocapture
== yaml, Pane::Loading ==
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ago|
 54 |    Container used more memory than it was allowed and|
 50 |    limit 256Mi · exit 137 (SIGKILL) · 47 restarts|
== logs, Pane::Ready with nothing arrived ==
 53 |    ● payments/web  ·  1 of 5 pods          1014 days|
 52 |      Container used more memory than it was allowed|
 52 |      limit 256Mi · exit 137 (SIGKILL) · 47 restarts|
== logs, Pane::Ready with one line ==
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ago|
 54 |    Container used more memory than it was allowed and|
 50 |    limit 256Mi · exit 137 (SIGKILL) · 47 restarts|
== logs, Pane::Loading ==
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ago|
 54 |    Container used more memory than it was allowed and|
 50 |    limit 256Mi · exit 137 (SIGKILL) · 47 restarts|
```

The second block differs from the other three by four columns: it starts two further right and
its lines end four earlier. The identity line's age reads `1014 days` where the other three read
`1014 days ago`; the title line ends at `allowed` where the other three end at `allowed and`.
No `…` is drawn on either. `src/ui.rs:4879` (`stream`) applies `padded` to its area on entry and calls `leads` with the
result at `src/ui.rs:4937`; `leads` (`src/ui.rs:3736`) applies `padded` again, at `:3747`.
`leads`' five other call sites — `:4854` (`logs`, `Pane::Loading`), `:4968` (`describe`),
`:5087` and `:5102` (`events`), `:5238` (`yaml`) — pass an unpadded pane, so only `stream`'s
double-pads. The block was laid out by `pinned(open, screen, usize::from(padded(body).width))`
at `src/ui.rs:4517`, which is 53.

## 4. A stack taller than the body, against the three panes with nothing under it

Two findings on one pod: the first with the OOM evidence, the second quoting the `runc` error
`screens/alerts.md` § The height already measures at 7 wrapped lines. **Two findings about one
pod is a shape the rule set already produces on the committed captures** — § 2 above shows
`default/broken-crashloop` and `default/broken-hostpath` at `findings=2` with `affected=1`. Identity 1 + title 2 +
evidence 1 + action 1, blank 1, identity 1 + title 2 + evidence 4 + action 1 = **14 rows against
the 13-row body** `screens/detail.md` § The arithmetic counts.

```
$ cargo test --bin k8rs m3_a_tall_stack -- --nocapture
== (b) events tab, none right now, two findings pinned ==
│  ANALYSIS          │    → raise limits.memory, or find the leak              │
│                    │                                                         │
│                    │  ● payments/web  ·  1 of 5 pods          1014 days ago  │
│                    │    Container image is not usable, so the container      │
│                    │    never started (CreateContainerError)                 │
│                    │    failed to create containerd task: failed to create   │
│                    │    shim task: OCI runtime create failed: runc create    │
│                    │    failed: unable to start container process: error     │
│                    │    during container init: exec:                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
holds `none right now`: false
holds `Kubernetes only keeps events`: false

== (c) yaml tab still loading, two findings pinned ==
(identical rows to (b) but for the tab marker row)
holds `reading the cluster…`: false

== (a) logs tab, nothing arrived, two findings pinned ==
holds `no logs yet`: false
holds the last word of the runc quote (`unknown`): false
holds a cut mark: false
```

Counted off frame (b): identity 1 + title 2 + evidence 1 + action 1 = 5, blank 1, identity 1 +
title 2 + evidence 4 (of the quote's full length) = 13 rows drawn against a 13-row body.
The last body row ends at `during container init: exec:` in every case. The quoted message
continues for a further ~90 characters and is drawn nowhere; nothing on the frame marks that it
was cut, and none of these three panes has a scroll offset. Frames (b) and (c) differ only in
which tab is marked open.

## 5. The step's own geometry

```
$ cargo test --bin k8rs m4_the_head_row -- --nocapture
 43 |  payments/web  ·  2 of 5 pods — pick a pod|
 52 |  Nothing has even looked at this pod yet, so it has|
 55 |  never started and the controller has had a great deal|
 44 |  to say about the matter over several lines|
  0 ||
 56 |▸ ● web-a   Nothing has even looked at this pod yet, so…|
 13 |  ▲ web-b   y|
```

Head row: `padded(head)`, `src/ui.rs:4544` — 53 columns starting at column 2, right edge at 55.
Block and pod rows: `region = body.width - width(MARKER)`, `src/ui.rs:4567` — 55 columns starting
at column 2, right edge at 57, which is the pane's own border.

Same card handed to the step with no pods on it (`affected == 0`, a node card):

```
$ cargo test --bin k8rs m7_the_step_on_a_card -- --nocapture
count() -> None, affected 0
│▸ ALERTS         1 ▲│  node-3 — pick a pod                                    │
│  RESOURCES         │This node refuses new pods (cordoned)                    │
│   workloads        │2 pods here would still have to move                     │
│   network          │→ allow new pods once the work is done                   │
```

With no selectable row, `ListState` has no selection, `List` does not reserve
`highlight_symbol`'s columns, and every row shifts two columns left onto the divider. With a
selection present the same rows sit at column 2 (§ 5 above, § 7 below).

Two pod objects sharing a name and differing only in uid:

```
$ cargo test --bin k8rs m6_two_rows -- --nocapture
Card::pods() -> 2 entries, affected reads 2
count() -> Some("2 pods")
│▸ ALERTS         1 ●│  payments/web  ·  2 pods — pick a pod                   │
│   config           │▸ ● web-0   ran out of memory                            │
│   cluster          │  ▲ web-0   image pull failed                            │
```

Both rows carry the anchor `"web-0"` (`src/ui.rs:4693`), so `Cursor::follow` resolves both to the
first. The shape is not reachable through the real pipeline: `k8s.rs:1380` keys the watch store
`type Key = (Option<String>, String)` — namespace and name — so two live pod objects of one name
cannot coexist in it.

## 6. What a frame of the step costs, by group size

Debug profile (`cargo test --bin k8rs`, no `--release`), host at `load average: 14.68`, 20 frames
averaged per size. Absolute values are an upper bound on release; the growth is the measurement.

```
$ cargo test --bin k8rs m5_the_frame_cost -- --nocapture
    3 pods: 3.347062ms per frame
   38 pods: 6.860963ms per frame
  200 pods: 25.238513ms per frame
 1000 pods: 158.346969ms per frame
 5000 pods: 2.030822056s per frame
```

200 → 1000 is 5× the pods for 6.3× the time; 1000 → 5000 is 5× for 12.8×. `Card::pods`
(`src/views.rs:346`) is a linear `contains` per finding, so it is itself O(F²) in `ObjectId`
comparisons; `worst_first` (`src/ui.rs:4616`) calls it again and then filters all findings once
per pod, so the per-frame count is about 2×F² and not the F² a single scan would be.

## 7. What the step draws with a group of three (unchanged, for comparison)

```
$ cargo test --bin k8rs the_which_pods_step_and_a_pinned_tab -- --nocapture
│▸ ALERTS         1 ●│  payments/web  ·  3 of 5 pods — pick a pod              │
│  RESOURCES         │  ran out of memory                                      │
│   workloads        │  limit 256Mi · exit 137                                 │
│   network          │  → raise limits.memory, or find the leak                │
│   storage          │                                                         │
│   config           │▸ ● web-7d9f4bc86d-x2k9p   ran out of memory             │
│   cluster          │  ● web-7d9f4bc86d-m3p1q   ran out of memory             │
│  ANALYSIS          │  ▲ web-7d9f4bc86d-t8g2r   image pull failed             │
```

The three facts here are the test fixture's own invented titles, not any string § 1 measured.

---

# Round two — measurements over the fixes (same day, same box)

Same method: the tree re-mirrored to a copy under `$HOME` on the test host with its own
`CARGO_TARGET_DIR`, six measurements appended to `src/ui_tests.rs` **in that copy only**, the copy
and its 2.6 GB scratch removed at the end. The host was quiet throughout — `load average: 0.49`
at the start, `3.88` at the end, no other build and no capture beside it.

```
$ ssh ubuntu 'cd ~/k8rs-review && CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --bin k8rs'
test result: ok. 1461 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.67s
```

## 8. The block's own right edge, with and without the scrollbar `leads` now draws

One finding (block fits, no bar) and two findings (14 rows against `floor(13) = 10`, bar drawn),
same card, describe tab.

```
$ cargo test --bin k8rs m9_the_identity -- --nocapture
== short (no bar) ==
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ago|
 21 |    ran out of memory|
bar drawn: false
holds `ago`: true
== tall (bar) ==
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ag█|
 55 |    ran out of memory                                 █|
 55 |  ● payments/web  ·  1 of 5 pods          1014 days ag█|
bar drawn: true
holds `ago`: false
```

`src/ui.rs:3792` takes the bar's column off the block's own width:

```rust
let [block, bar] =
    Layout::horizontal([Constraint::Min(0), Constraint::Length(u16::from(over))]).areas(held);
let at = scrolled(frame, block, app.scroll, false, above.to_vec());
```

`above` was wrapped at 53 by `pinned(open, screen, usize::from(padded(body).width))`
(`src/ui.rs:4585`); `block` is 52 when `over`. `Paragraph` with no `Wrap` truncates at the pane
width, and `identity` (`src/ui.rs:3882`) lays its line out to **exactly `region`** whenever the
finding has a drawable age — `body = region - GUTTER`, then `glyph` + `left` +
`" ".repeat(body - width(left) - measured)` + `age`, which sums to `region` by construction
(`:3894`, `:3920`). So every identity line on an overflowing block loses its last character.

The same frame under a refusal banner, to show it is not an artefact of one pane:

```
$ cargo test --bin k8rs m10_three_states -- --nocapture   (Logs / denied)
  you are not allowed to read this. Ask for `get` on
  pods in payments.

  container: app                      previous log: off

  ● payments/web  ·  1 of 5 pods          1014 days ag█
    ran out of memory                                 █
    exit 137                                          ║
    → raise limits.memory, or find the leak           ║
                                                      ║
  ○  no logs yet

  Nothing has been written to this container's log yet.
```

`pod_pick` solved the identical problem the other way (`src/ui.rs:4643`): the bar draws in a
`PAD`-wide margin the rows already reserve, so `region` does not move. § 11 below shows that
working.

**Which tests see which half.** `a_block_over_a_log_that_never_started_is_drawn_at_the_width_it_was_wrapped_for`
(`src/ui_tests.rs:13897`) asserts `identity.ends_with("ago")` on a **one-finding** card — block
fits, no bar, `over` is false.
`a_block_taller_than_the_pane_keeps_the_tabs_own_sentence_and_scrolls_for_the_rest`
(`src/ui_tests.rs:13961`) uses a **two-finding** card — bar drawn — and asserts the sentence, the
bar's existence and the tail's reachability, but nothing about the block's right edge. The two
tests partition the space so that neither stands on the intersection.

## 9. Loading, empty and denied — twelve frames, four tabs, all compared

Each of the four tabs in each of the three states, under the same 14-row block, compared
byte-for-byte against every other frame.

```
$ cargo test --bin k8rs m10_three_states -- --nocapture
---- Logs / loading ----     loading=true  nologs=false noevents=false denied=false bar=true
---- Logs / empty ----       loading=false nologs=true  noevents=false denied=false bar=true
---- Logs / denied ----      loading=false nologs=true  noevents=false denied=true  bar=true
---- Describe / loading ---- loading=true  nologs=false noevents=false denied=false bar=true
---- Describe / empty ----   loading=false nologs=false noevents=false denied=false bar=false
---- Describe / denied ----  loading=false nologs=false noevents=false denied=true  bar=false
---- Yaml / loading ----     loading=true  nologs=false noevents=false denied=false bar=true
---- Yaml / empty ----       loading=false nologs=false noevents=false denied=false bar=false
---- Yaml / denied ----      loading=false nologs=false noevents=false denied=true  bar=false
---- Events / loading ----   loading=true  nologs=false noevents=false denied=false bar=true
---- Events / empty ----     loading=false nologs=false noevents=true  denied=false bar=true
---- Events / denied ----    loading=false nologs=false noevents=false denied=true  bar=false
== frames that are byte-identical to another ==
collisions: 0
```

## 10. The sentence `leads` reserves `FLOOR` rows for

A block far past the cap, so `leads` spends its whole `floor(area)` and `plainly` is handed
exactly `FLOOR = 3` rows.

```
$ cargo test --bin k8rs m11_the_sentence -- --nocapture
│                    │  ○  none right now                                      │
│                    │                                                         │
│                    │  Kubernetes only keeps events for a while, and this     │
├────────────────────┴─────────────────────────────────────────────────────────┤
holds `none right now`: true
holds `Kubernetes only keeps events`: true
holds the sentence's own last words: false
```

`views::NO_EVENTS` (`src/views.rs:3237`) is *"Kubernetes only keeps events for a while, and this
pod has run long enough that none are left."* — 95 characters, two lines at 53 columns. `plainly`
(`src/ui.rs:3810`) spends one row on the headline and one on the blank, leaving one, and renders a
`Paragraph` with no offset and no `CUT`. The surviving text ends mid-clause.

The logs sentence does not reach this: *"Nothing has been written to this container's log yet."*
is 53 characters, exactly one line at 53 columns. `WAITING` is 20. So `FLOOR = 3` is one row short
for exactly one of the four sentences.

## 11. The step's geometry after the fix

```
$ cargo test --bin k8rs m12_the_step_geometry -- --nocapture
== step ==
 43 |  payments/web  ·  2 of 5 pods — pick a pod|
 55 |  never started and the controller has had a great deal|
 53 |▸ ● web-a   Nothing has even looked at this pod yet,…|
== 38 pods, scrolling ==
 45 |  payments/web  ·  38 of 40 pods — pick a pod|
 57 |  cannot be scheduled                                   █|
 57 |▸ ● log-shipper-000   cannot be scheduled               █|
 57 |  ● log-shipper-001   cannot be scheduled               ║|
```

Longest block row 55 and longest pod row 53, against 56 and 57 before the change (§ 5 above). The
rows now stop two columns short of the frame, which is the margin the head row keeps. With 38
pods the bar sits in that reserved margin and no row gives up a column for it.

The same finding pinned on a tab, for comparison:

```
== the same finding on a tab ==
 54 |    Nothing has even looked at this pod yet, so it has|
 52 |    never started and the controller has had a great|
 51 |    deal to say about the matter over several lines|
```

Both panes now hand `said` a `region` of 53. The remaining two columns of difference are
`said`'s own `under` argument — `"  "` on a tab, which has an identity line to hang under, and
`""` on the step, which does not (`src/ui.rs:3733`) — so the wrap widths are 51 and 53 by design
and not by geometry.

## 12. `leads` handed a pane with nothing left to give

A refusal long enough that `banner` spends its whole `floor(area)` budget: `rest` is 3 rows,
`stream`'s own header takes 2, `floor(1) == 0`, `leads` returns `None`.

```
$ cargo test --bin k8rs m14_leads_handed -- --nocapture
│                    │  container: app                      previous log: off  │
│                    │                                                         │
│                    │                     ○  no logs yet                      │
holds any block text: false
holds the block's identity: false
holds `no logs yet`: true
holds a cut mark on the banner: false
```

The banner is drawn whole and first; the block is absent, with nothing on the frame recording that
the object has a finding. `rest.height >= FLOOR` is structural whenever `leads` answers `Some`
(`tall <= floor(area) = height - 3`), so this is the only shape in which the block is dropped
rather than deferred.
