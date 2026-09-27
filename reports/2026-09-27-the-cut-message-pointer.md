# 2026-09-27 — the cut message pointer: row arithmetic, and what the audit log holds

Operator review of Phase 13 box 2 (`ui.rs` § `refused`'s quote block, the
`The full message is saved in the audit log.` row). **No cluster and no test host
were used** — `tester` held the host mirror, and every number below comes from
the working tree at the commit the review was dispatched over plus one local
re-implementation of two `ui.rs` helpers.

Tree read: `development`, working tree modified (`NOTES.md REQUIREMENTS.md
backlog.md screens/dialogs.md src/ui.rs src/ui_tests.rs`), on top of `55e8f83`.

## 1. The wrap and the row budget, reproduced locally

`src/ui.rs`'s `wrapped` (:6315) and `marked` (:6405) re-implemented in Python
over ASCII input, so display width equals character count and the reproduction is
faithful for these strings. Constants read off the tree:
`DISMISS_BOX = 54` (:218), `MODAL_MARGIN = "  "` (:233),
`room(54) = 52` (:2100), the quote's own width `52 - 2 = 50` (:2573),
`MODAL_ROWS = 13` (:294).

```
$ python3 scratchpad/wrap.py
FREEZE chars: 212 | POINTER cols: 43 of 52
quote width: 50 | wrapped rows: 5

--- quote at 5 rows ---
    deployments.apps "broken-owned" is forbidden:
    ValidatingAdmissionPolicy 'k8rs-no-restarts' with
    binding 'k8rs-no-restarts' denied request: this
    cluster does not allow restarting deployments
    during a change freeze

--- quote at 4 rows ---
    deployments.apps "broken-owned" is forbidden:
    ValidatingAdmissionPolicy 'k8rs-no-restarts' with
    binding 'k8rs-no-restarts' denied request: this
    cluster does not allow restarting deployments…

--- quote at 3 rows ---
    deployments.apps "broken-owned" is forbidden:
    ValidatingAdmissionPolicy 'k8rs-no-restarts' with
    binding 'k8rs-no-restarts' denied request: this…

--- row budget per state (MODAL_ROWS = 13) ---
1c  check refused: outcome 1 row(s), because 2 row(s) -> left = 4
2   real refused: outcome 1 row(s), because 1 row(s) -> left = 5
3   never answered: outcome 1 row(s), because 1 row(s) -> left = 5
```

Field values the findings turn on:

- the 212-character message wraps to **5** rows at 50 columns; 4 rows carry 189
  characters (45+49+47+45 plus the three spaces the wrap dropped), which is the
  count `reports/2026-09-26-the-error-state-pass.md` § 8 measured.
- `left` is **4** in state 1c, **5** in states 2 and 3 — so `left` never reaches
  the values where `marked`'s one-row floor (:6413) plus the pointer row would
  overrun `MODAL_ROWS`.
- at 3 rows the last clause kept is `denied request: this…`; at 4 rows it is
  `cluster does not allow restarting deployments…`.

## 2. Candidate row texts against the 52 columns the box has

```
$ python3 - <<'EOF'   # len() == columns for these ASCII/em-dash strings
 27 cols  (box has 52)   What the cluster sent back:
 51 cols  (box has 52)   What the cluster sent back — more in the audit log:
 58 cols  (box has 52)   What the cluster sent back (the rest is in the audit log):
 43 cols  (box has 52)   The full message is saved in the audit log.
 45 cols  (box has 52)   The rest of this message is in the audit log.
 44 cols  (box has 52)   The rest is in ~/.local/state/k8rs/audit.log
 51 cols  (box has 52)   The whole message is in the audit log, cut at 4 KB.
EOF
```

State 1c's explanation, the string the two rows are spent on
(`src/ui.rs:2515`), is **73 columns**: `This is the check that runs before the
real change — it stopped this one.`

## 3. What the write path holds for a `said` longer than 4096 bytes

Read, not run:

- `src/k8s.rs:213` — `pub(crate) const FREE_TEXT: usize = 4096;`
- `src/k8s.rs:220` — `const SHORTENED: &str = "… (shortened by k8rs)";`
- `src/k8s.rs:284-308` — `text(value, cap)`: over `cap`, `value.truncate(cut)`
  then `value.push_str(SHORTENED)`.
- `src/k8s.rs:1176-1179` — `message(status)` runs `text(&mut said, FREE_TEXT)`,
  so the cut happens **once, in `k8s.rs`**, before the value leaves it.
- `src/ui.rs:670-672` — the file's own doc: `views::Modal::Refused::said` is
  `k8s::said`, "at `crate::k8s::FREE_TEXT` either way".
- `src/ops.rs:572-577` — `Outcome::said` returns that same string and "is not
  cleaned again here".
- `src/ops.rs:1268-1276` — `result_line` ends `format!("{}\n", and_said(line,
  outcome.said()))`; `and_said` is `src/ops.rs:1359-1364`.

So the string the dialog quotes and the string the audit line records are the
same value. For a `said` over 4096 bytes both end in `… (shortened by k8rs)`;
on screen that tail is past the quote's cut and is not drawn.

Sizes already measured elsewhere in the tree for this shape:
`NOTES.md:18740` — a `fieldValidation=Strict` 422 on a Deployment, **4859
bytes**; `src/views_tests.rs:2430` feeds `"x".repeat(4859)`.
`src/ui_tests.rs`'s own 4 kB fixture is `"denied: " + "replicas may not exceed
five ".repeat(140)` = **4068 bytes**, under the 4096 cap.

`src/k8s.rs:1162-1168` records a second route to a large `message`: when a
response body is not JSON, kube puts the whole body in that field
(`Status::failure(&text, "Failed to parse error data")`).

## 4. Where `recorded` stops

```
$ grep -n "recorded" src/main.rs
218:  ... NOTES § D21's *nothing was sent because nothing could be recorded*. `1` stays
6910: ... That sentence is the **only** place `recorded: false` is
7379: ... and `recorded` deliberately does not move it: a `Done` k8rs could not write down
10591:        None => "not recorded",
10662:        // **Nothing was sent because nothing could be recorded** (NOTES § D21)
```

- `src/ops.rs:1131` — `let recorded = write_line(audit,
  &record.result_line(attempt, clock(), &outcome)).is_ok();`
- `src/ops.rs:629-648` — `Performed::plainly` appends "— but k8rs could not
  write that to the audit log, so the trail of it is short a line" when
  `recorded` is false.
- `src/main.rs:10616-10686` — `settled` reads `performed.outcome` and
  `outcome_word(...)`; it does not read `performed.recorded`, and builds
  `views::Modal::Refused { sent, fault, said }` (:10675, :10680).
- `src/main.rs:10577-10578` — the doc on `outcome_word` states "The sentence
  still reaches the reader: it is what the refusal box draws."
- `src/ops.rs:1645-1650` — `tester`'s measured failure mode for this flag: a
  tmpfs filled mid-record returned `StorageFull` part-way and the file ended
  with half a line.

`Modal::Refused` is constructed in those two places only
(`grep -n "Modal::Refused" src/views.rs src/main.rs src/ui.rs`: `views.rs:3032`
footer arm, `ui.rs:2015` draw arm, `main.rs:10073/10080` key arm,
`main.rs:10675/10680` construction).

## 5. What the box's second key does

- `src/views.rs:3032` — footer for a refusal: `esc dismiss  ⏎ open`.
- `src/main.rs:10080-10087` — `Some(views::Modal::Refused { .. }) if key.code ==
  KeyCode::Enter` sets `console.app.modal = None` and calls `entered(console,
  cards, open)`: the box closes and the **object** opens.
- `src/ui.rs:6372-6374` — `cut`'s doc still ends "The full text is one `⏎`
  away, which is what makes cutting it legitimate at all
  (`screens/widgets.md` § 7)".
- `screens/widgets.md:1263-1316` — the eleven deliberate cuts, declared closed
  by D266; the five back-cuts are the card's evidence line, the browser's
  summary line, the command-log strip, a `Confirm`'s `$` line and an Analysis
  row. The refusal quote's cut is not among them.

## 6. What names the audit log's path on screen

```
$ grep -rn "state/k8rs/audit.log" screens/ docs/
screens/states.md:1325:│▸ ALERTS     3 ● 7 ▲│  k8rs could not open its audit log at         │
screens/states.md:1459:│   cluster          │  k8rs could not open its audit log at         │
docs/security.md:529:`~/.local/state/k8rs/audit.log`, **created** mode 0600 in a directory created
```

`grep -rn "audit" screens/help.md` returns ten lines, all about the banner and
the withheld keys when the log could **not** be opened (`:252` —
`k8rs could not open its audit log — fix that, then start k8rs again`); none
names the path. `src/ops.rs:3380-3395` builds it from `$XDG_STATE_HOME` or
`$HOME`, and the opened path is not carried into `views`.

## 7. Not measured here, and the commands that would settle it

- **The drawn boxes.** The three new/changed tests were not run — no `cargo` on
  this machine and the host mirror was held by `tester`:
  `ssh ubuntu 'cd ~/k8rs-src && export PATH=$HOME/.cargo/bin:$PATH && cargo test
  --locked --all-targets -- --nocapture ui::tests::the_pointer_to_the_audit_log_draws_where_the_quote_lost_rows_and_nowhere_else
  ui::tests::a_four_kilobyte_refusal_is_cut_to_the_box_and_says_so
  ui::tests::every_dialog_says_the_words_the_screen_file_says'`.
  § 1's arithmetic is a re-implementation of the helpers, not the renderer.
- **The over-`FREE_TEXT` shape through the real dialog.** Nothing in
  `ui_tests.rs` feeds `refused` a `said` that `k8s::text` already shortened. The
  shape to feed is `"x".repeat(4859)` through `k8s::said`'s own path (as
  `views_tests.rs:2430` does), and the value to read is what the audit line ends
  with.
- **A live over-4096 rejection.** A `ValidatingAdmissionPolicy` whose
  `messageExpression` returns more than 4096 characters, or a Strict 422 on a
  Deployment, both on the review cluster:
  `K8RS_CLUSTER=review ./scripts/cluster.sh up` then the patch, reading
  `Status.message`'s byte count and the tail of the audit line.

---

# Re-read, same day — after the two blockers were fixed

Same conditions: no cluster, nothing on the test host (`tester` held the mirror).
Tree now also modified in `screens/widgets.md src/main.rs src/main_tests.rs
src/views.rs src/views_tests.rs`.

## 8. The row budget with the shortened explanation

```
$ python3 - (wrapped() re-implemented as in § 1, COLUMNS = 52, quote width 50)
 50 cols, 1 row(s)  new 1c: The check before the real change stopped this one.
 73 cols, 2 row(s)  old 1c: This is the check that runs before the real change — it stopped this one.
 41 cols, 1 row(s)  pointer: More of this message is in the audit log.
 50 cols, 1 row(s)  no-record: k8rs could not write this to the audit log either.

state 1c: left = 5; FREEZE wraps to 5 rows -> whole, no row
state 2: left = 5; FREEZE wraps to 5 rows -> whole, no row
state 3: left = 5; FREEZE wraps to 5 rows -> whole, no row
```

Both sentences are one row at every width this box has (52 columns, fixed —
`DISMISS_BOX` does not narrow), and the 1c explanation has two columns of slack.

## 9. The cut mockup's four quoted rows, against the helpers

`screens/dialogs.md` § *The quote's last row says where the rest is*, second
mockup, versus `marked(wrapped(oversized(), 50), 50, 4)`:

```
$ python3 -  (said = "denied: " + "replicas may not exceed five " * 140)
bytes: 4068 | wrapped rows at 50: 85
    denied: replicas may not exceed five replicas may|  (49 cols)
    not exceed five replicas may not exceed five|  (44 cols)
    replicas may not exceed five replicas may not|  (45 cols)
    exceed five replicas may not exceed five replicas…|  (50 cols)
```

Character-identical to the page. Row counts between the nested borders: 13 for
the whole-message box (blank, outcome, blank, heading, 5 quoted, blank,
explanation, blank, button) and 13 for the cut box (4 quoted + the pointer row
in place of the fifth).

## 10. The gate, read against the write path

- `src/ops.rs:1225-1239` — `attempt_line` carries object, context, server,
  namespace, uid, kubectl, verb, path, resourceVersion. **No `said`.** So "the
  result line is the only line that quotes the cluster" is true of the tree.
- `src/ops.rs:1131` — `recorded` is that line's `write_line(...).is_ok()`;
  `write_line` is `write_all` then `flush` (`:1652-1655`).
- `src/main.rs:10661` — `let recorded = performed.recorded;`, read before
  `performed.outcome` is moved out; passed at `:10681` and `:10687`.
- `src/views.rs:995-1003` — the field, documented.
- `src/ui.rs:2468`, `:2632-2646` — the parameter and the two sentences under
  `if over`.
- All **23** `Modal::Refused { … }` construction sites in `ui_tests.rs`,
  `views_tests.rs` and `main_tests.rs` set `recorded`
  (`for f in …; grep -n "Modal::Refused {" | … MISSING` printed nothing).
- `src/ui_tests.rs` — `beyond_free_text()` builds a 4859-byte message and runs
  the real `crate::k8s::text` over it, so the shape § 3 named is now fed to the
  box; `oversized()` (4068 bytes) stays the under-cap case.

Two residual error windows, **reasoned from the code rather than run**:
`write_all` can fail on the last byte of a line whose `said` already landed, and
`flush` on a `File` is a no-op (`ops.rs:1049-1055`), so `recorded == true` means
*handed to the OS*, not *on disk*. Both point the same way — the box under-claims
rather than over-claims — and the second is already outside invariant 3.

## 11. Counts in `screens/widgets.md` § 7 after the twelfth entry

```
$ grep -n "three of the five\|All five are marked\|1, 2 and 5 walk\|Six cut from" screens/widgets.md
1266:  Six cut from the **back** — one of them numbered 12, after the front-cuts
1273:  **Back-cuts.** The full text is one `⏎` away in three of the five, which is
1314:  All five are marked with a character the reader can see and step by whole
1315:  characters, never a byte. 1, 2 and 5 walk back to a whole word before they
```

```
$ grep -n "not reachable inside \`refused\` yet" screens/dialogs.md
1722:`recorded` is not reachable inside `refused` yet. That wiring is `dev-core`'s
```

`src/ui_tests.rs:8909-8911` — `FREEZE`'s own doc still reads "one more than
state 1c's two-line explanation leaves it and exactly what state 2's one-line
explanation does", against the test below it, which now asserts the message
draws whole in all three states.

## 12. Ownership wording measured, for § Q3

```
 49 cols  k8rs's check before the real change stopped this.
 53 cols  k8rs's check before the real change stopped this one.
 55 cols  The check k8rs ran before the real change stopped this.
```
