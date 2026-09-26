# Phase 11 close — the family read of `ui.rs` and `views.rs` (2026-09-13)

`k8s-admin`, CLAUDE.md § Phase close step 4, HEAD `03ec00c`, clean tree. Both files freeze at this
close (todo.md Phase 11 *Frozen after*). **No cargo was run** — a phase-close mutation sweep held
the machine — so nothing below is a render of the real binary. What was measured is (a) the code
and the screen files read by line, (b) `ratatui-core-0.1.2` / `ratatui-widgets-0.3.2` read off the
registry, and (c) an ASCII transcription of `ui.rs` § MEASURING AND CUTTING run in Python over
real-shaped names. Every line of (c) is a claim `tester` should re-take through `ui::draw` before
anything is ruled on it. Severities are the reviewer's; the PM rules.

## 1. The cut transcription, run

Command (script in the agent's scratchpad, a line-by-line transcription of `fits`, `clipped`,
`command_cut`, `shortened`, `name_cut`, `wrapped` for ASCII input, where width equals length):

```
$ python3 cuts.py ; echo exit=$?
exit=0
== A. Alerts card identity line, 80x24: region 53, body 51, room = 51 - (age + 2)
  age='4 min ago'    room=40  drawn='openshift-cluster-node-tuning-operator/t'
  age='2 hours ago'  room=38  drawn='openshift-cluster-node-tuning-operator'
  age='4 min ago'    room=40  drawn='team-alpha-payments-platform/checkout-wo'
  age='2 hours ago'  room=38  drawn='team-alpha-payments-platform/checkout-'
  age='4 min ago'    room=40  drawn='team-alpha-payments-platform/checkout-wo'
  age='2 hours ago'  room=38  drawn='team-alpha-payments-platform/checkout-'
== B. Confirm box, restart (consequence lines at 56: 5 -> CROWDED_BOX 61)
  title : 'Restart team-alpha-payments-platform/checkout-worker-servi…'
  $ line: '$ kubectl rollout restart…'
  title : 'Restart team-alpha-payments-platform/checkout-worker-servi…'
  $ line: '$ kubectl rollout restart…'
== C. Confirm box, scale ("This starts 1 more copy of your app." fits -> CONFIRM_BOX 58)
  title : 'Scale team-alpha-payments-platform/checkout-worker-serv…'
  $ line: '$ kubectl scale…'
  title : 'Scale team-alpha-payments-platform/checkout-worker-serv…'
  $ line: '$ kubectl scale…'
== D. the in-flight footer's cut of the same identity (room 76 - fixed parts)
  footer: 'team-alpha-payments-platform/che…'
  footer: 'team-alpha-payments-platform/che…'
== E. Gone box body: clipped(name, room(54) - 2 = 50)
  body  : 'team-alpha-payments-platform/checkout-worker-serv…'
  body  : 'team-alpha-payments-platform/checkout-worker-serv…'
== F. sidebar kind row: List reserves the 2-col symbol, indent 3 -> 15 columns, ratatui clips silently
  persistentvolumes                    -> 'persistentvolum'   (front cut would be '…sistentvolumes')
  persistentvolumeclaims               -> 'persistentvolum'   (front cut would be '…ntvolumeclaims')
  validatingadmissionpolicies          -> 'validatingadmis'   (front cut would be '…issionpolicies')
  validatingadmissionpolicybindings    -> 'validatingadmis'   (front cut would be '…policybindings')
  horizontalpodautoscalers             -> 'horizontalpodau'   (front cut would be '…podautoscalers')
  certificatesigningrequests           -> 'certificatesign'   (front cut would be '…igningrequests')
  customresourcedefinitions            -> 'customresourced'   (front cut would be '…rcedefinitions')
  volumeattachments                    -> 'volumeattachmen'   (front cut would be '…umeattachments')
== G. the command log strip, 76 columns: Log::sent appends '   …' (RUNNING), Log::outcome swaps it for '→ word'
  command width 75
    while running : '$ kubectl rollout restart deployment/payments-api -n…'
    after outcome : '$ kubectl rollout restart deployment/payments-api -n…'
  command width 55
    while running : '$ kubectl scale deployment/web --replicas=3 -n payments   …'
    after outcome : '$ kubectl scale deployment/web --replicas=3 -n payments   → rejected'
== H. a --context appended at the tail of a read line, 76 columns
    '$ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields…'
```

The widths the transcription uses were read off the code and cross-checked against the screen
files: the card's 51/40/38 are `screens/alerts.md` § The age, and what it costs the name's own table;
the in-flight room of 33 is `ui::name_cut`'s own doc; 76 is `ui::strip`'s doc.

**ratatui, read off the registry and not assumed.** `ratatui-widgets-0.3.2/src/list/rendering.rs`
lines 66–104: with a selection present (`HighlightSpacing::WhenSelected`), every item's area starts
`highlight_symbol_width` in, so the sidebar's 20 columns leave 18, and a kind row's indent of 3
leaves 15 — and the item is rendered into that area with no mark. Lines 146–200: an item taller
than the viewport is never drawn, selected or not (traced with one 20-row item in 14 rows:
`first == last == 0`). `ratatui-core-0.1.2/src/text/span.rs:314` and `buffer/buffer.rs:351` drop
any grapheme containing a `Cc` character, so an unstripped `ESC` cannot reach the terminal through a
widget; `U+202E` is `Cf` and passes.

## 2. Phase 12's boxes against the frozen public surface

| Phase 12 box | A caller alone? | What it needs that is not there |
|---|---|---|
| `tokio::select!` loop, audit `File` lifetime | yes | `ui::draw`, every `Screen`/`App` field and every `Modal` variant is `pub` |
| coalescing test | yes | — |
| Ctrl-Z · panic teardown | yes | — |
| flags | yes | `Writes::ReadOnly` is `pub` |
| one place decides the context | yes | `Picker::new`, `Connection` are `pub` |
| the cluster picker is wired | yes, with a placement ruling | `--context` on every later line: `views::describe_line`/`events_line`/`yaml_line` and `ops::Shown::kubectl` carry none, so the caller splices; *where* decides what `ui::command_cut` keeps (§ 1 H) |
| every dialog string proven stripped | yes | — |
| **the two sentences `ops.rs` keeps to itself stop being copied** | **no** | the box names a second copy in `src/ui.rs`: `ui::gone`'s hedge, ui.rs:1826, is `ops.rs:2397/2403` word for word; removing it edits `ui.rs` (frozen now) and reaching the original edits `ops.rs` (frozen after Phase 7). NOTES § D260 item 6 boxed *a visibility change* in a frozen file |
| manual error-state pass · idle CPU | yes | — |

And three obligations Phase 11's own done-when (*every key in the footer works*, todo.md:4547–4548)
hands to Phase 12's loop, none of which a caller can meet:

- **`c container`** (views.rs:2204, drawn as `container: app ▾` at ui.rs:3656–3663): no `Modal`
  variant, no `App::footer` arm, no drawing for `screens/detail.md` § Choosing a container
  (lines 195–235, and `screens/widgets.md` § 2a's own mode list row). No box in todo.md or entry in
  backlog.md names it.
- **`/ filter` and `n`** on Alerts and Resources: `App` has no *typing a filter* state
  (views.rs:1727–1770), `ui.rs` never reads `app.filters` (grep: no hit), `App::open` keeps the
  filter across views (views.rs:2323–2337), and an Alerts list a filter emptied can only be handed
  over as `Pane::Ready(vec![])`, which draws `○ nothing is broken` (ui.rs:2454–2456). backlog.md:2536
  records *drawn in no pane*; it does not record the collapse, and its fix site freezes today.
- **`⏎ open` on a card.** `ui::Detail` (ui.rs:731–761) carries no finding, and `Described`/`Logs`
  hold a `PodSnapshot` only (`views::describe_line` spells `pod`, views.rs:1561). What the card's
  evidence `…`, `N more problems — ⏎ to see` and the browser's `● web has 3 pods with problems — ⏎
  to see` promise is `screens/detail.md` lines 377–402 (every finding pinned; a grouped card first
  lists which pods) and `screens/alerts.md` 285–287, 709–716. Phase 10's grouping box
  (todo.md:4212–4215, checked) says *the detail view lists which pods*; nothing in either file does.

`s`/`r` on a kind the operation does not support (Pod, Node, a DaemonSet's `s`, a bare ReplicaSet's
`r`): `screens/help.md` lines 383–400 call it *withheld* and hand the drawing to states.md's *later
box*; `views::Offer` (views.rs:1941–1972) has no per-key shape, and the only non-`Act` answer a
caller can pass also refuses `ctrl-d` through `App::may_mutate` (views.rs:2031–2033), which `delete`
supports on all six kinds.

## 3. Cross-box observations, by line

- **One object identity, cut six ways.** Card: `fits` with no mark (ui.rs:2851–2855). Confirm title:
  `clipped` from the tail (ui.rs:1448, 1713–1718), and its `$` line walked back past the name
  (ui.rs:1692–1698). Gone body: `clipped` (ui.rs:1843–1848). Detail heading and sidebar plural:
  handed to a `Paragraph`/`List` and clipped by ratatui with no mark (ui.rs:3465–3471,
  2318–2326). In-flight footer: `name_cut` (ui.rs:1036) — which keeps the `/`, not the name (§ 1 D).
  `screens/widgets.md` § 7 (lines 685–734) lists eight deliberate cuts; the card, the dialog title,
  the dialog `$` line, the Gone body, the header zone and the picker's tag/badge are not among them.
  `screens/widgets.md:37–39` rests the fixed sidebar on *the labels are fixed-length strings*;
  discovery plurals are not.
- **`…` means two things on one strip.** `views::RUNNING` (views.rs:1328) and `ui::CUT` (ui.rs:137)
  are the same character, and `command_cut` removes the tail first, where the running mark and the
  outcome both live (§ 1 G). The first case also shows the walk-back dropping `payments-production-eu`
  although the head ended exactly on it.
- **`○ nothing is broken` ignores the link.** `ui::clock` hides structurally under a non-`Live`
  link (ui.rs:2524–2526); the calm headline does not (ui.rs:2454–2456).
  `an_expired_login_keeps_the_key_that_renews_it_on_every_frame` (ui_tests.rs:2308) builds exactly
  `Pane::Ready(Vec::new())` with `Link::Expired` and asserts only the footer.
- **The header's connection word is still the caller's string** beside `Screen::link`
  (ui.rs:457–470), which is NOTES § D265 ruling 1's *a caller-joined word can say `admin` over dead
  keys*, one word along.
- **The Analysis `List` never got the card fix.** `ui::alerts` truncates every card to the pane
  (ui.rs:2717–2733, citing the same ratatui lines as § 1); `ui::analysis` does not
  (ui.rs:3295–3316), and `ui::drawn` says a row *wraps and never clips*. A drain-safety row folds
  NODE_SILENT, a budget's explanation, *N block the drain too* and up to three problem paragraphs
  (analysis.rs:945–983); at 49 columns that estimates past the 14-row body. Estimated, not rendered.
- **The refusal box versus the audit line for one outcome.** For `Outcome::NotSent`, `ops::check`
  (ops.rs:1185–1200) writes *the check never left this machine* or *k8rs does not know whether the
  check reached the cluster*; `ui::refused` (ui.rs:1750–1755) draws *The cluster refused this* and
  *This is the check that runs before the real change — it stopped this one* for every `sent:
  false`. `screens/dialogs.md:800–805` rules a dead socket during the check into that state. The box
  holds `fault` and never reads `views::because`, so a `409` (reachable today through `delete`'s
  `preconditions.uid`) draws *refused* and not the re-read `because(Conflict)` already words.
- **Follow, then `k`.** `App::scroll_by` (views.rs:2342–2345) turns `following` off and adds to a
  `scroll` that following never advanced; the bottom is `ui::scrolled`'s private, width-dependent
  `last` (ui.rs:3543–3554). Overscroll past `last` is equally unrecoverable from the caller.
- **`expect(dead_code)`** stays on ui.rs:37–43 and views.rs:42–48 with *deleted by hand* promised to
  turns that can no longer edit either file; theme.rs:24–34 records that a `dead_code` expectation
  is not reported unfulfilled in this crate, so an unwired `may_mutate`, `may_quit` or `offered`
  after Phase 12 raises nothing.
- **A backlog premise that is false.** backlog.md:2943–2951 says a mutation `…` deadline is *not
  fixable after `views.rs` freezes without a reversal*; `Log::outcome("k8rs stopped waiting")` is
  20 bytes under `SAID`'s 32 (views.rs:1352, 1481) and callable from `main.rs`.

## 4. Phase 11's own security gate (todo.md:4541–4545)

- **Hostile input, by surface.** Fed in `ui_tests.rs`: a 10k name to the Confirm title (8417); a
  10k context name and a 600-byte address to the picker (10204); `U+202E` to the picker tag (9411).
  No `ESC` fixture is fed to any surface in `ui_tests.rs` (grep for `u{1b}` / `x1b`: no hit); ratatui
  would drop it (§ 1). Not fed any of the three: the card owner name, evidence, browser cells, the
  detail heading, log and event lines, yaml, the sidebar plural, the header zone, a `Pane::Denied`
  sentence, the in-flight footer, the Gone body, the `Unconnected` title. `Screen::namespace`
  (ui.rs:494) is drawn raw by `heading()` and `empty()` (ui.rs:2950, 2987) and, when it came from
  `--namespace`, never met `k8s::text` — NOTES § D264 ruling 1 says so for the same value inside a
  `Coverage`.
- **Object identity in the confirmation.** § 1 B and C: at 80×24 two Deployments differing only in
  their tail draw byte-identical scale and restart dialogs.
- **A revealed Secret redrawn after dismissal.** There is no reveal: no `Modal` variant, no key, no
  field; backlog.md:2997 and NOTES § D256 record it as waiting on a frozen `k8s.rs`. The row holds
  vacuously.

## 5. Read and found holding

`ui::withheld` is the one reader for the footer's `Offer` and Help's heading (ui.rs:1071–1140,
344–402), in NOTES § D265 ruling 4's order; Help's `X` row is rewritten only for a call in flight,
matching `App::may_switch_cluster`. The header's front cut keeps `read-only`, the TLS warning and
`changing…` at the tail (ui.rs:1257–1339). `ui::failed` draws only `views::because`/`scope`/
`next_step` (ui.rs:2137–2172) — no second vocabulary. `ui::caveats` ranks clock, pane, audit and
`ui::note` puts the audit sentence first (ui.rs:2481–2583), NOTES § D263. A `Refused` mark reaches
only `Offer::Act`, and only `Verdict::No` marks (views.rs:1854–1903). `Object::new` refuses an
empty uid; `Dialog::verdict` and `Refused::resource` are `&'static str`. `Log::outcome` strips and
bounds through `k8s::text`. Every `Confirm` draws its `$` line in its frame. The picker's footer
and buttons read one `Picker::verbs`/`inert`/`nowhere`.
