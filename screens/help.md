# Screen — Help (`?`)

The footer always shows the keys valid *right now*; `?` shows all of them. A
tool for beginners may not hide its verbs behind memory.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌ Keys ──────────────────────────────────────────────────────────────┐
│  Moving around                                                     │
│    ↑ ↓ / j k    move            ⏎     open the selected thing      │
│    tab          next panel      esc   back / close                 │
│    X            switch cluster                                     │
│    [ ]          detail tabs     / n   filter · namespace           │
│                                                                    │
│  Looking at things (always available)                              │
│    l  logs, with the log from before a crash                       │
│       in the log tab:  f follow · c container · ⇧p previous        │
│    d  describe — the object and what happened to it                │
│    y  view as YAML                                                 │
│                                                                    │
│  Changing things (each one asks first, and shows the command)      │
│    s       run more or fewer copies       (scale)                  │
│    r       restart, at its own pace       (rollout restart)        │
│    ctrl-d  delete — you type the name to confirm                   │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                              │
│ $ kubectl get daemonsets -A --watch                                │
├────────────────────────────────────────────────────────────────────┤
│ ? or esc to close                                          q quit  │
└────────────────────────────────────────────────────────────────────┘
```

**The command log strip is not covered, and it is not cleared.** Opening
`?` runs no command of its own, so the two-line block keeps showing whatever
was already there — here, the two permanent watches Alerts starts on top of
the primary pod watch, because this mockup opens help from the default Alerts
screen. The body above (16 lines, exactly [§1's own
budget](widgets.md#1-the-frame)), the log strip (4 rows — border, two lines,
border, [`LOG_LINES`](widgets.md#2-element--widget) never a mockup's
choice) and the footer (1 row) are the same three regions every other screen
draws in the same order; only the footer's own content and the body's own
frame — one bordered block titled `Keys`, the full body width, no sidebar —
differ from an ordinary screen ([widgets.md § 5](widgets.md#5-the-modal-layer)).
Header (1) + top border (1) + body (16) + log block (4) + footer (1) + bottom
border (1) is 24, the floor, not 23 — a fact about this screen's own fixed
content, since nothing here depends on cluster state the way a card list or a
sidebar count does.

Rules:

- **One key, one meaning, everywhere.** `/` always filters or searches the
  pane you are in, `n` is always namespace, `r` is always restart and never
  "retry". The two keys that collided — a severity filter and a manual
  reconnect — were deleted rather than rebound; the reasoning is in
  [NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted). This screen
  is the full map — `q` sits in the footer with the other keys that are valid
  right now, the same place every other screen puts it.
- Grouped by **what you are doing**, not by keycode order, and the jargon is
  in brackets — a newcomer reads the sentence, and learns the term for free.
- Only keys that exist in this build appear. **Once writes are dead for this
  run — `--read-only`, or an audit log that would not open, the same one
  signal either way — the *Changing things* heading is rewritten to say so,
  its `s` row carries the cause's own sentence, and `r`/`ctrl-d` go blank**
  ([D259 ruling 5](../NOTES.md#d259--the-footer-is-a-curated-subset-with-one-pair-that-never-gives-way-the-help-screen-is-the-frame-wearing-a-title-rather-than-a-box-drawn-inside-it-and-a-gate-verified-against-a-substituted-tree-is-not-verified-2026-09-10),
  [D263 ruling 2](../NOTES.md#d263--the-nine-states-a-refusal-that-was-also-a-scope-a-stack-that-cut-the-one-banner-with-nothing-else-to-say-and-a-test-named-for-a-body-it-never-compared-2026-09-12),
  [D265 ruling 5](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).
  Drawn whole at [§ Under a dead-writes
  run](#under-a-dead-writes-run).
- v0.2+ operations join this screen as they land (cordon, drain, rollout undo,
  then exec and port-forward, then edit) — see
  [NOTES § Operations](../NOTES.md#operations--the-full-admin-surface).

## Under a dead-writes run

Two different causes land here — `--read-only`, or `ops::audit_log` failing
to open — and the header reads `read-only` for both
([D265 ruling 3](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).
**The *Changing things* heading stays, and says the state; the row under it
says why and what brings it back** — one fixed sentence per cause, neither
interpolating the audit banner's own sentence
([D265 ruling 5](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).
Nothing else about the screen changes — the same sixteen-row body, the same
command log strip, the same `?`/`q` footer this file draws everywhere else.

```
 nodes 3/3                            k8rs       ctx: prod-eu · live · read-only
┌ Keys ────────────────────────────────────────────────────────────────────────┐
│  Moving around                                                               │
│    ↑ ↓ / j k    move            ⏎     open the selected thing                │
│    tab          next panel      esc   back / close                           │
│    X            switch cluster                                               │
│    [ ]          detail tabs     / n   filter · namespace                     │
│                                                                              │
│  Looking at things (always available)                                        │
│    l  logs, with the log from before a crash                                 │
│       in the log tab:  f follow · c container · ⇧p previous                  │
│    d  describe — the object and what happened to it                          │
│    y  view as YAML                                                           │
│                                                                              │
│  Changing things (off for this whole run)                                    │
│    k8rs was started with --read-only — quit and start it again without it    │
│                                                                              │
│                                                                              │
├──────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl get daemonsets -A --watch                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│ ? or esc to close                                                     q quit │
└──────────────────────────────────────────────────────────────────────────────┘
```

`Writes::Unaudited` draws the same frame, over the same two rows:

```
  Changing things (off for this whole run)
    k8rs could not open its audit log — fix that, then start k8rs again
```

- **Drawn at the real 80-column floor, not this file's usual 70-column
  page** — the same move [§ When a key is refused](#when-a-key-is-refused)
  already makes. `read-only` is four columns longer than `admin`, and
  `ui::header`'s own centring — not this page's own hand-drawn one — shows
  what that costs: at 70 columns `k8rs` centres at column 33 regardless of
  the right zone, so `admin`'s six columns of gap before it narrows to two
  for `read-only`. Two blank columns is not a collision, but it is not
  room either, so this mockup draws the floor instead of dropping the
  centred name.
- **The heading is rewritten, not removed — anchored on `  Changing
  things`, the same leading text [`key_map`] already finds it by.** Its
  `s` row carries the cause's own sentence, at the same four-column indent
  every mutating row uses; `r` and `ctrl-d` go blank. No row is added or
  removed: the block keeps the four rows it always had.
- **Neither row is marked refused.** Invariant 2's *unreachable, not
  merely unbound* is about **mutation**, not `ops.rs` as a whole — `may_i`
  writes nothing and touches no audit log, so neither `--read-only`
  ([D230 ruling 3](../NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05))
  nor `Unaudited` refuses it — the absence here is structural, not a
  verdict a probe gave.
- **The row names the cause and the way back; the path and the error are
  the banner's, when it has room to draw them** — the audit sentence is
  the first to give way under
  [D263 ruling 5](../NOTES.md#d263--the-nine-states-a-refusal-that-was-also-a-scope-a-stack-that-cut-the-one-banner-with-nothing-else-to-say-and-a-test-named-for-a-body-it-never-compared-2026-09-12)'s
  rank, so a queued clock or namespace banner can leave it undrawn even
  once Help closes, and this row must hold true either way.
- **`s`, `r` and `ctrl-d` appear nowhere as mutating keys under either
  cause** — the row that was `s` is prose, `r` and `ctrl-d` are blank, and
  none of the three gains a refused clause: see the last bullet of
  [§ When a key is refused](#when-a-key-is-refused).

## While the call is running

Help is drawn over an ordinary screen everywhere else in this file, but
[a mutation in flight](dialogs.md#while-the-call-is-running) is not a
`Modal` — the screen behind it keeps working, so `?` still opens Help on top
of it. There, unlike anywhere else Help opens, something is still pending: a
second mutation, a cluster switch and `q` are all refused until the call
returns — and the body used to say nothing of that. It promised `X switch
cluster`, `s run more or fewer copies`, `r restart, at its own pace` and
`ctrl-d delete` exactly as if all four worked, on the one screen a reader
opens to find out what they may press; pressing any of them did nothing, with
no line anywhere saying why.

**The body still has sixteen rows — nothing is added or removed — but two of
them are rewritten while a call is on the wire, anchored on their own leading
text and never by counting, the same mechanism [§ When a key is
refused](#when-a-key-is-refused) already uses for a permission this login
lacks:**

In **Moving around**, the `X` row:

```
    X            switch cluster (paused while a change is running)
```

In **Changing things**, the heading and its three rows, unchanged beneath it:

```
  Changing things (paused while a change is running)
    s       run more or fewer copies       (scale)
    r       restart, at its own pace       (rollout restart)
    ctrl-d  delete — you type the name to confirm
```

- **The `X` row keeps its own label and gains a clause** — `switch cluster`
  is unchanged, only what follows it is new — the same append the
  permission-refused rows below already make to their own jargon
  parenthesis. It is anchored on `    X `, its own unique leading text, the
  same way `    s `, `    r ` and `    ctrl-d ` already are.
- **The *Changing things* heading is rewritten instead of its three rows,
  because the reason is one fact for all three, not three separate ones.**
  A call in flight refuses `s`, `r` and `ctrl-d` uniformly — a missing
  permission never does; one key can be refused while the other two are not.
  Rewriting all three rows to say the same six words three times over would
  be the second copy of a fact this codebase already has one home for; the
  heading governs the group and says it once, anchored on `  Changing
  things`, its own unique leading text.
- **This state and a permission refusal are never reconciled on the same
  row.** While a call is in flight, `s`, `r` and `ctrl-d` are inactionable
  for the wait's reason alone, whatever a permission probe would otherwise
  say about any one of them — the rewritten heading is what draws, and the
  ordinary key map or [§ When a key is refused](#when-a-key-is-refused)'s own
  per-key clauses take over again the moment the call returns.
- **Neither clause names the object**, and neither needs to: "a change is
  running" is true regardless of which one, and the object it names is one
  `?` away — dismiss Help and the ordinary screen underneath, including the
  in-flight footer, is exactly where it was.
- **This is not the `no` this screen's own *When a key is refused* section
  reserves for a missing permission** — a call finishing is a wait, not a
  permission this login lacks, and `paused` is the word for a wait
  everywhere else this product uses it (`screens/dialogs.md`'s own paused
  Deployment). The moment the call returns, both rows read exactly as they
  did before it started, or as [§ When a key is
  refused](#when-a-key-is-refused) draws them if a permission is what is
  actually missing.
- **What this costs, once, rather than left for a reader to notice on their
  own:** while a call is in flight, this section's rewritten rows draw for
  every login the same way, whether or not a permission probe would also
  refuse `s`, `r` or `ctrl-d` on its own account. A login that in fact may
  never scale reads `paused`, the same as one that may scale but is waiting
  out someone else's restart — not the harder truth, *"and you may never do
  this either way."* It self-corrects the moment the call returns: the
  ordinary key map comes back, or [§ When a key is
  refused](#when-a-key-is-refused)'s own `s no scale` does, whichever this
  login was always going to see. One state suppresses a fact the other
  already shows correctly, on purpose, rather than the two states agreeing
  to show two different guesses at it.

The footer's right zone still empties the same way this state already did:

```
? or esc to close
```

`q quit` is gone, not marked `q no quit`, for the same reason as before.
**The footer does not also spell out why** — the two rewritten rows above
already do, so a reader who presses `?` to find out why `s` went quiet now
reads the answer in the one place every other key's reason already lives on
this screen, rather than a second sentence squeezed into a footer that has
never carried one.

## While the link is down, the login has expired, or the clock is off

Three more run-level reasons `offered` withholds `s` and `r` for
(`views::Offer::Move`) — `Screen::link` off `Live`, either way, and a clock
this page cannot trust — and Help drew none of them: the ordinary *Changing
things* block, live keys and all, over a run where none of the three could
actually be pressed
([D265 ruling 4](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).

**Same mechanism as [§ While the call is
running](#while-the-call-is-running)'s own — the heading rewritten, its
three rows unchanged beneath it, one reason drawn — not restated here**
([D262](../NOTES.md#d262--the-in-flight-screen-the-state-that-had-to-name-its-object-the-cut-that-gave-way-at-the-wrong-end-and-the-screen-that-answers-what-may-i-press-promising-four-keys-it-refuses-2026-09-12)).
**The `X` row is not rewritten for any of the three** — nothing about a
lost link, an expired login or the clocks disagreeing stops a cluster
switch, so only *Changing things* changes:

```
  Changing things (paused while disconnected, retrying)
```

```
  Changing things (paused — renew your login, then press X)
```

```
  Changing things (paused — the clocks disagree; quit and start k8rs again)
```

- **Order, when more than one applies: dead writes first, then a call in
  flight, then the link, then the clock** ([D265 ruling
  4](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).
  Dead writes never coincide with a call in flight — no call can start to
  be *in flight* once writes are dead — but they can coincide with the
  link or the clock, and outrank both there:
  [states.md § And when the login has also
  expired](states.md#and-when-the-login-has-also-expired) draws exactly
  this, a dead audit log under an expired login, and Help's heading still
  reads the dead-writes state. A call in flight *can* coincide with a lost
  link or an expired login — the watch can drop, or a token can expire,
  while a `PATCH` is still on the wire — and there its clause wins,
  because it is the one of the four that also pauses `X` ([§ While the
  call is running](#while-the-call-is-running)). `Link::Lost` and
  `Link::Expired` are two values of the one field `Screen::link`, never
  both true at once, so there is nothing to rank between them. The clock
  ranks last because `ui::clock` reads `None` whenever the link is not
  `Live`, so a stale reading never gets to compete with a link reason at
  all.
- **The words are the header's and the banners' own, not reinvented
  here.** `disconnected, retrying` is [states.md § The connection
  dropped](states.md#the-connection-dropped)'s own header pointer;
  "renew your login" and "press X" are [states.md § Your login
  expired](states.md#your-login-expired)'s own words — *"Renew it, then
  press X and pick this cluster again"*; "the clocks disagree" matches
  [states.md § Your computer's clock is
  off](states.md#your-computers-clock-is-off)'s own refusal to name which
  clock is wrong. **The fix named is not `X`.** The skew is read once at
  connect, so nothing on screen re-reads it on its own — but `X` does not
  reconnect either: measured, `X` then `⏎` on the picker's own live
  current row only closes the picker
  (`views::Picker::chosen` → `Chosen::Close`), so a reader who pressed it
  hoping to re-check the clock would see nothing happen
  ([D265 ruling
  4](../NOTES.md#d265--the-read-only-mark-the-header-joins-the-permission-word-itself-and-help-swaps-for-either-cause-2026-09-13)).
  `quit and start k8rs again` is [D264 ruling
  23](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)'s
  own phrasing for a reader already inside the TUI, and today it is the
  only thing that actually re-reads the clock.
- **A permission-refused clause never lands on top of one of these three**,
  the same reconciliation [§ While the call is
  running](#while-the-call-is-running) already states for itself: whichever
  reason's heading is drawn is what a reader presses against, and
  [§ When a key is refused](#when-a-key-is-refused)'s own per-key clauses
  return the moment none of the four reasons above it holds.

## When a key is refused

The mockup above is the case this login can use every key it lists, or
nothing is selected yet — the ordinary case, and unchanged. When an object
*is* selected and `may_i_in` comes back `Verdict::No` for one of the three
mutating keys, that key's own row in *Changing things* gains one clause; no
other row and no other block on this screen changes
([D23](../NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards),
[D229](../NOTES.md#d229--the-four-rulings-mayi-could-not-be-briefed-without-and-the-boxs-arithmetic-that-went-stale-under-it-2026-09-05)).
The three keys are independent — one refused, two, or all three, in any
combination — and each row only ever answers for itself; the worst case,
drawn below, is all three at once, with `payments/web` selected (the running
example everywhere else in this product) and this login able to `list` and
`watch` it but nothing more:

**Drawn at the real 80-column floor, not this file's usual 70-column page —
each row is one line in the real UI and is shown as one line here, the same
move [states.md](states.md#your-clock-and-a-scoped-namespace-together) makes
for its own over-70 row:**

```
  Changing things (each one asks first, and shows the command)
    s       run more or fewer copies   (scale — get+patch deployments/scale)
    r       restart, at its own pace   (rollout restart — patch deployments)
    ctrl-d  delete — you type the name to confirm (delete deployments)
```

Counted, not estimated:

| Row | Columns (`deployments`) | Columns (`statefulsets`) |
|---|---|---|
| `s`, refused | 76 | 77 |
| `r`, refused | 76 | 77 |
| `ctrl-d`, refused | 70 | 71 |

`statefulsets` is the longest plural either operation names — `scale` also
reaches a bare `replicasets` (11, same as `deployments`), `restart` also
reaches `daemonsets` (10, shorter still) — so it is the case that decides the
ceiling, not the one this file's running example happens to draw. All six
counts fit inside the 78-column ceiling a body row has at the floor
(`src/ui_tests.rs::mockup`'s own `MIN_WIDTH - 2` assertion, measured at HEAD).
**No row is added and none is removed**: the sixteen-row body this screen is
tested against is unchanged in count, only in the text of up to three of its
lines.

- **The clause extends the existing jargon parenthesis for `s` and `r`, and
  opens a new one for `ctrl-d`.** `(scale)` and `(rollout restart)` already
  taught the kubectl term this key stands for; adding *why not* inside the
  same parenthesis keeps one bracket meaning *the technical detail*, rather
  than a second bracket beside the first that a reader has to learn means
  something else. `ctrl-d`'s row has no such parenthesis to extend — its own
  em dash already separates the key from *"you type the name to confirm"* —
  so its reason opens a fresh one instead of reusing that dash for a second
  job.
- **The opening `(` moves from the row's 44th character to its 40th on every
  refused row, `s`'s included now that its own clause names two verbs.**
  Positions here are 1-based, the way an editor's own column indicator counts
  — everywhere else in this section "columns" measures a length, not a
  position, and the two are not interchangeable. The baseline has both `s`
  and `r` opening `(` at the 44th character (`(scale)`, `(rollout restart)`).
  Holding either refused row's `(` there pushes it past the 78-column
  ceiling, so both give up the same four characters of alignment with the
  baseline and open at the 40th instead — the counts are in the table above.
  The two refused rows now line up with *each other*, not with the baseline
  above them — the same trade every column budget on this page already makes
  when a longer string has nowhere else to give.
- **The resource named is the selected object's own kind, not a fixed
  string, and this whole section presumes the kind supports the operation at
  all — a key the kind does not support is out of its scope.**
  `deployments/scale` and `deployments` are what `payments/web` reads as a
  Deployment; a StatefulSet reads `statefulsets/scale` and `statefulsets`.
  Measured at HEAD (`src/ops.rs`), the two operations do not cover the same
  kinds in either direction: `scale` reaches a Deployment, a StatefulSet or a
  bare ReplicaSet, never a DaemonSet; `restart` reaches a Deployment, a
  StatefulSet or a DaemonSet, never a bare ReplicaSet — and neither reaches a
  Pod, a ConfigMap or a Node. Where the selected kind does not support the
  key at all, `may_i_in` is never asked, `Verdict::No` never arrives, and the
  key does not read as *refused* by this section — it is *withheld*, the same
  fact and the same word [states.md](states.md) already uses for a key with
  nothing to act on, and drawing it is that file's own later box, not this
  one's. A screen that asked the permission question anyway and drew `s no
  scale` on a DaemonSet would tell the reader they lack a permission that
  does not exist to hold.
- **The verb and resource are named because [states.md](states.md) already
  set the pattern for a missing permission on this product** — *"Missing
  permission: list nodes"*
  ([states.md § You can only see some namespaces](states.md#you-can-only-see-some-namespaces))
  — and a second, softer phrasing for the same fact on writes would be a
  second convention for one idea. `get`, `patch` and `delete` are not
  translated: invariant 14 asks that jargon be explained, not left
  unexplained, not that it never appear — the sentence a beginner reads first
  is already the plain one (*"run more or fewer copies"*), and the bracket
  beside it is where the exact term has always lived, on every row, refused
  or not.
- **The slash reads two different ways depending which tool sees it, and this
  section keeps the Role's own spelling rather than a `can-i` query's**
  ([D230 ruling 1](../NOTES.md#d230--the-mayi-review-round-a-spelling-that-answers-the-opposite-of-kubectl-and-the-read-only-user-who-could-not-ask-what-they-may-do-2026-09-05)).
  `deployments/scale` is exactly what a `Role.rules[].resources` entry looks
  like, and that is the string an operator hands to whoever owns the cluster —
  the same next action
  [states.md](states.md#you-can-only-see-some-namespaces)'s "Missing
  permission: list nodes" already sends a reader to. It is *not* what either
  `kubectl auth can-i` or k8rs's own `ops may-i` takes after that resource: in
  both, a `/` there names the *object*, and the subresource is a separate
  `--subresource=` flag — so pasting this clause straight into either command
  answers a different question than the one this screen is asking, and
  answers it wrong the loud way: `kubectl auth can-i patch
  deployments/scale -n default` reads `scale` as an object name and says
  *yes*, and so does k8rs's own tool — measured, `k8rs ops may-i patch
  deployments.apps/scale -n default` says *yes* for the very login this
  screen draws `s no scale` for. Either tool's real question is
  `--subresource=scale`, never the slash. The clause keeps the Role's
  spelling because that is what the reader's next action needs, not because
  it is safe to run as typed.
- **Dead writes win outright, so this section's clauses are never drawn
  under them.** Once writes are unreachable for the run — `--read-only`, or
  an audit log that would not open — the *Changing things* heading is
  rewritten to name it and the `s` row becomes the cause's own sentence;
  neither starts with the anchor text a refused clause looks for, and
  `r`/`ctrl-d` are blank, so there is no row left for one to land on,
  whatever a permission probe would have answered
  ([D259 ruling 5](../NOTES.md#d259--the-footer-is-a-curated-subset-with-one-pair-that-never-gives-way-the-help-screen-is-the-frame-wearing-a-title-rather-than-a-box-drawn-inside-it-and-a-gate-verified-against-a-substituted-tree-is-not-verified-2026-09-10),
  [D263 ruling 2](../NOTES.md#d263--the-nine-states-a-refusal-that-was-also-a-scope-a-stack-that-cut-the-one-banner-with-nothing-else-to-say-and-a-test-named-for-a-body-it-never-compared-2026-09-12)).
  [§ While the call is running](#while-the-call-is-running)'s own rewritten
  heading has the same nothing to rewrite, under the same run — one gap,
  not two, and neither section says it twice. Drawn whole at [§ Under a
  dead-writes run](#under-a-dead-writes-run).
