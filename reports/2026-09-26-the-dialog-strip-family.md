# 2026-09-26 — the dialog-strip family, read as one thing

Operator review of the uncommitted Phase 12 box *"Every string a dialog draws is
proven stripped"* (NOTES § D283), read together with `views.rs`'s modal layer,
`ui.rs`'s dialogs and `main.rs`'s console wiring ([D103](../NOTES.md#d103--the-process-was-measured-and-what-it-lacked-was-a-rule-that-makes-something-smaller-2026-08-15)).

**No cluster was brought up and nothing was built.** `tester` held the test host
for step 5 for the whole of this run, so every number below is read off the source
or re-derived as arithmetic from it, and each is labelled as one or the other. The
one claim that wants a real run is named in § 7 with the command.

## 1. Who writes `Log::waiting`, and therefore whether a refusal can be seen

```
$ grep -rn "\.sent(" src/*.rs | grep -v "_tests"
src/main.rs:9553:                console.log.sent(format!("$ {}", dialog.kubectl.as_str()));
```

```
$ grep -n "log\.sent(\|log\.ran(\|log\.outcome(" src/main.rs
7981:            console.log.ran(views::GET_CONTEXTS.to_owned());
8241:        console.log.ran(line);
9458:            console.log.ran(views::GET_CONTEXTS.to_owned());
9553:                console.log.sent(format!("$ {}", dialog.kubectl.as_str()));
10087:            console.log.outcome("refused");
```

Field values the finding turns on, read off the source:

- `views::Log::waiting` — `Option<usize>`, set by `Log::sent` and by nothing else
  (`src/views.rs:1754-1774`).
- `views::Log::outcome` — `let Some(at) = self.waiting.take() else { return; }`
  (`src/views.rs:1806-1808`).
- `ops::restart`'s `object_name` refusal returns before `Mutation` is built
  (`src/ops.rs:2405`), so `show` never runs, so `over_modal`'s confirm arm
  (`src/main.rs:9553`) never runs.

## 2. Where `ops.rs` checks the name, relative to `show`

```
$ grep -n "object_name" src/ops.rs
55:use crate::k8s::{FREE_TEXT, Fault, IDENTIFIER, fault, namespace_name, object_name, said, text};
402:    /// [`crate::k8s::object_name`] refuses an empty name — but [`Record::of`]'s strip can empty a
1846:    if !object_name(scaling.name) {
1973:/// two rules are `k8s::object_name`'s and `k8s::namespace_name`'s, the driver's own refusals
2405:    if !object_name(restarting.name) {
2567:/// argument as the [`object_name`] and [`namespace_name`] guards two functions down, which the
2762:    if !object_name(deleting.name) {
```

Predicate values, read off `src/k8s.rs:4736-4804`:

- `NAME_MAX` = 253; `object_name(word)` = `word.len() <= NAME_MAX && path_safe(word)`.
- `path_safe` admits ASCII alphanumerics, `-` and `.`, first character alphanumeric.
- `NAMESPACE_MAX` = 63; `namespace_name` is a DNS-1123 label.

## 3. Where the refusal sentence is stripped

- `ops::unaddressable` (`src/ops.rs:1975-1981`) interpolates `cleaned(object, FREE_TEXT)`.
- `ops::cleaned` (`src/ops.rs:1665-1669`) is `k8s::text` over an owned copy.
- `k8s::unprintable` (`src/k8s.rs:251-256`) answers `true` for `char::is_control`,
  so `ESC` is dropped rather than kept (`src/k8s.rs:288-296`: non-whitespace
  unprintables are removed, whitespace ones become one space).

## 4. `views::Object`'s field visibility, and the sites that write past the door

```
$ grep -rn "\.name = \|\.namespace = \|Object {" src/*.rs | grep -v "^src/k8s.rs"
src/views.rs:1331:pub struct Object {
src/views.rs:1390:impl Object {
src/ui_tests.rs:9857:    dialog.object.name = "w".repeat(10_000);
src/ui_tests.rs:9952:    dialog.object.name = "w".repeat(400);
   … (other hits are `rules::ObjectId`, `k8s::Row`, `views::Filters`, `ui::Screen`)
```

Field values, `src/views.rs:1339-1387`:

- `pub kind: &'static str`, `pub namespace: Option<String>`, `pub name: String`,
  and `uid: Option<String>` — private, the only private one.
- A private field blocks the struct literal and functional-update syntax (E0451).
  It does not block `object.name = value`, and the two sites above are that.

NOTES § D283 ruling 3 cites `grep -rn 'Object {' src/*.rs` as its evidence. That
pattern matches struct literals only; neither line above matches it.

## 5. The two caps that decide whether the confirm button can ever light

Read off the source:

| value | where | cap |
|---|---|---|
| `ops::Record::of`'s `confirm` | `src/ops.rs:1183-1186` | `k8s::IDENTIFIER` = 512 bytes |
| `views::Stripped::of` (what `installed` spends on `asks`) | `src/views.rs:192-196` | `k8s::FREE_TEXT` = 4096 bytes |
| `views::Input::push` | `src/views.rs:274-278` | `k8s::IDENTIFIER` = 512 bytes |
| `k8s::SHORTENED`, appended after every cut | `src/k8s.rs:220` | 23 bytes / 21 columns |

`views::Dialog::armed` (`src/views.rs:1567-1572`) requires `typed.text() == asks`.
So a value `text` had to cut is `cap + 23` bytes and can never be typed back at
`cap`.

`src/ui.rs:2171`'s `typed_name` doc states: *"`views::Input` bounds it at
`k8s::IDENTIFIER`, which is exactly the longest name a dialog can ask for."*
The longest `Record::of` can produce is 535 bytes; the longest the *type* now
allows is 4119.

## 6. Arithmetic: what a crafted name becomes, and what the title bar shows

**Not a run of k8rs.** `k8s::text`, `ui::shortened` and `ui::name_cut`
transliterated into Python only to get byte and column counts right; the script is
in the scratchpad and is not committed. Inputs are `main_tests.rs`'s own
`crafted_name()` and the constants above.

```
$ python3 arith.py
crafted: 10010 chars 10014 bytes
Object::new name: 535 bytes, 533 cols, markers: 1
  head 'web[31mzzzzz' tail 'zzzzz… (shortened by k8rs)'
re-strip at IDENTIFIER: 535 bytes, markers: 1
  tail 'zzzzzzzzzzzzzzzzzzzzzzzzz… (shortened by k8rs)'
Stripped::of(name) unchanged? True
title room: 51 -> title 59 cols
title: 'Restart …/…zzzzzzzzzzzzzzzzzzzzzzzzzzz… (shortened by k8rs)'
marker is 21 of the 51 title columns
```

Two values this settles:

- **`k8s::text` is idempotent at a fixed cap.** The cut lands on the marker's own
  boundary and the marker is re-appended, so a second pass at the same cap returns
  the same 535 bytes and one marker — not two. `views.rs:16-17`'s new *"over such
  a value it is a no-op"* holds.
- **The `Confirm` title at `CROWDED_BOX` (61) has 51 columns**, and for a name
  `k8s::text` had to cut, 21 of those 51 are k8rs's own marker; the 27 name
  characters shown are bytes 479–505 and not the name's end, because `k8s::text`
  back-cuts and `ui::shortened` front-cuts.

## 7. The one claim not settled here

Whether the refusal line the author pasted really carries a raw `ESC`. § 3 says it
cannot, from three source lines. The run that would settle it against the compiled
code, on the test host, once `tester` is out of the mirror:

```
cargo test --locked a_crafted_name_is_refused_before_any_box_can_open_on_it \
  -- --nocapture 2>&1 | cat -v | head -5
```

`cat -v` is the load-bearing half: a terminal swallows the byte either way.

## 8. `Dialog::warning`: who populates it

```
$ grep -rn "while_paused\|warning: None," src/main.rs src/views.rs
src/main.rs:7212:fn while_paused(kind: &str, checked: Option<&bool>) -> Option<String> {
src/main.rs:7271:            if let Some(warning) = while_paused(kind, checked.returned()) {
src/main.rs:9947:            warning: None,
src/views.rs:1469:    /// `while_paused` off the `bool` `ops::Checked::returned` carries (NOTES § D224).
```

Field values, read off the source:

- `src/main.rs:7271` is inside `restarted`, the **headless** `ops restart` path; it
  writes the sentence to the same stream `show` printed to.
- `src/main.rs:9947` is inside `mutating`'s `show` closure — the **console** path —
  and is the literal `None`. `main.rs`'s `ask` closure two screens down publishes
  `Published::Checked { verdict, asks }` (`src/main.rs:9954-9958`); that enum
  (`src/main.rs:8818`) carries no warning, so nothing downstream can set the field.
- `ops::Checked::returned()` is `pub` and carries the `bool` either caller needs
  (`src/ops.rs:358-360`).
- `ops::restart`'s success maps `Landing::Finished` → `Outcome::Done`
  (`src/ops.rs:1099`) → `main.rs`'s `outcome_word` → `"done"`
  (`src/main.rs:10047`).
- `screens/dialogs.md:801-843` draws the paused box with both sentences, for the TUI.
- `grep -n "warning" src/main_tests.rs` finds no console test of it; the only
  renders of the paused box are `ui_tests.rs`'s hand-built `paused.warning = Some(…)`.

## 9. What was checked and found unchanged

Read, no finding: `ops.rs` gained no call and no verb; `--read-only` still gates
`wanting` through `views::App::may_mutate` (`src/main.rs:9891`); `may_i` untouched;
no LIST, no watch and no poll added — `Stripped::of` runs twice per mutation (dialog
open, verdict install), not per frame; no new dependency; no new file; the
`dryRun`/`resourceVersion`/typed-name path is byte-identical to HEAD.

---

# Round two — the seven fixes

Same conditions: **no cluster, nothing built, mirror not touched** (`tester` held
it). Everything below is read off the source at round-two HEAD or is labelled
arithmetic.

## R1. The one-word budget fix: do the two expressions ever differ?

`src/ui.rs:2337` is now
`short.saturating_sub(consequence.len().saturating_sub(keep))`, where it was
`short.saturating_sub(consequence.len() - keep)`. Two lines above:
`let keep = consequence.len().saturating_sub(short).max(1);`

**The one-line reason.** `keep` is `max(a, 1)` where `a = c.saturating_sub(s) <= c`.
For `c >= 1` both arguments of the `max` are `<= c`, so `keep <= c`, so `c - keep`
never underflows and is equal to `c.saturating_sub(keep)`. The expressions can
therefore differ only at `c == 0`.

Brute force over every pair a 61-column box can reach (arithmetic only, the two
expressions transliterated; not a run of k8rs):

```
$ python3 budget.py
pairs checked: 3721
keep differs anywhere: no
short differs for a non-underflowing pair: no
underflowing pairs (all have consequence.len() == 0): [0]
what the new expression gives on those: [(0, 0), (1, 1), (2, 2), (3, 3)] ... i.e. short == over
```

Field values the answer turns on:

- `wrapped("")` returns an empty `Vec` — the `while let Some(at) = left.find(…)`
  never enters and `line` is never pushed (`src/ui.rs:6246-6288`). Same for a
  whitespace-only string.
- `marked(vec![], columns, 1)` returns the empty `Vec` — `lines.len() <= most`
  (`src/ui.rs:6333-6336`). No spurious `…`.
- `[profile.release]` in `Cargo.toml:264-267` sets `lto`, `strip` and
  `codegen-units` and **no `overflow-checks`**, so release took the default
  `false`: `0usize - 1` wrapped to `usize::MAX` and
  `short.saturating_sub(usize::MAX)` was `0`.
- So at `c == 0` the old code was: debug → panic, release → `short = 0` (the
  warning never cut, box over `MODAL_ROWS` = 13). New: `short = over`.
- `c == 0` is unreachable from `ops.rs`: every `consequence` is one of `rollout`'s
  or `removal`'s `&'static str` literals, and `Record::of` carries
  `debug_assert!(!consequence.is_empty())` (`src/ops.rs:1161-1164`).

## R2. Ceiling arithmetic for the two re-pointed claims

Read off the source:

| question | value | where |
|---|---|---|
| longest `Object::new` can emit | `IDENTIFIER` 512 + `SHORTENED` 23 = **535** bytes | `src/views.rs:1411-1420`, `src/k8s.rs:220` |
| longest name the **product** can put in a dialog | `NAME_MAX` = **253** bytes | `src/k8s.rs:4785`, gate at `src/ops.rs:2405` and `:2762` |
| longest `asks` `ops::Record::of` can emit | 512 + 23 = **535** bytes | `src/ops.rs:1183-1186` |
| longest `asks` the *type* now allows | `FREE_TEXT` 4096 + 23 = **4119** bytes | `src/views.rs:195-196` |
| longest `views::Input` can ever hold | **512** bytes | `src/views.rs:274-278` |

The gate order that makes 253 the product's ceiling: `ops::restart` runs
`object_name(restarting.name)` at `src/ops.rs:2405`, before `Mutation` is built
and therefore before `perform` calls `show`; `mutating`'s `show` closure passes
`asked.name.clone()` — the same string `object_name` judged — to `Object::new`
(`src/main.rs:9943`). `ops::delete` is the same shape at `src/ops.rs:2762`.

## R3. Every door in the restored leftover list, traced

```
$ grep -n "self.waiting" src/views.rs
1788:        self.waiting = Some(self.lines.len() - 1);
1822:        let Some(at) = self.waiting.take() else {
1874:            self.waiting = self.waiting.and_then(|at| at.checked_sub(over));
```

```
$ grep -rn "Connection::Live(\|Connection::Dropped(" src/*.rs | grep -v _tests
src/main.rs:7842:        (views::Connection::Live(name), ui::Link::Expired) => {
src/main.rs:7843:            views::Connection::Dropped(name.clone())
src/main.rs:8221:    console.connection = views::Connection::Live(session.context.clone());
src/main.rs:8336:                views::Before::Connected(last) => views::Connection::Dropped(last.clone()),
src/views.rs:1260:            Connection::Live(_) if row.current => return Chosen::Close,
src/views.rs:1261:            Connection::Live(name) | Connection::Dropped(name) => Before::Connected(name.clone()),
```

Doors, one row per list entry:

| drawn value | route | cap |
|---|---|---|
| `Modal::Refused::said` | `ops::Outcome::said` → `k8s::said` (`k8s.rs:1186`) → `k8s::message` (`:1173`) | `FREE_TEXT` |
| `Unconnected::said` | `k8s::Trouble::said` (`k8s.rs:1326`) → `watch_said` (`:1361`) → `said` **or**, on the `WatchError` arm, `message` directly | `FREE_TEXT` |
| `Unconnected::to` | `Choice::name` (`main.rs:9602`) → `drawable` (`k8s.rs:7375`) | `IDENTIFIER` |
| `Unconnected::renewal` | `k8s::renewal` (`k8s.rs:8089`) = `drawable(auth.exec.command)` | `IDENTIFIER` |
| `Before`'s name | `Session::context` (`main.rs:8221`, the only origin) → `kubeconfig_context` → `drawable` | `IDENTIFIER` |
| `Screen::contexts`' rows | `Choice::name`/`namespace`/`Address::Server`/`Tag::Written` → `drawable` (`k8s.rs:7375`), `namespace_of` (`:7468`), `text(&mut drawn, IDENTIFIER)` (`:7685`), `written_tag` (`:7741`) | `IDENTIFIER` |
| `Unconnected::coverage` | see below — a **predicate**, twice | 63 |

`Coverage`, read off `src/k8s.rs:6859-6906`:

```rust
if let Some(namespace) = asked {
    return Coverage::Asked(if namespace_name(namespace) {
        namespace.to_string()
    } else {
        context_scope(context_namespace).unwrap_or(FALLBACK_NAMESPACE).to_string()
    });
}
…
fn context_scope(context_namespace: Option<&str>) -> Option<&str> {
    context_namespace.filter(|namespace| namespace_name(namespace))
}
```

So: **every string every `Coverage` arm carries has passed `k8s::namespace_name`** —
the argv value directly, the context's namespace through `context_scope`'s
`filter`, and `FALLBACK_NAMESPACE`, a literal that satisfies it. A word that
*fails* the predicate still produces a `Coverage::Asked`; what it does not do is
get into one. The context's own namespace additionally arrives through a *strip*:
`namespace_of` → `drawable` → `text(_, IDENTIFIER)` (`k8s.rs:7468`).

## R4. Object's field privacy, checked for a way round it

```
$ grep -rn "&mut views::Object\|&mut Object" src/*.rs | grep -v _tests
(no output)
```

```
$ grep -rn "\.object\.name = \|\.object\.namespace = " src/*.rs
(no output)
```

- `impl Object` is `new`, `name`, `namespace`, `uid` — no `&mut self` method
  (`src/views.rs:1401-1450`).
- `#[derive(Clone, Debug, PartialEq, Eq)]` — no `Default`, no `Deserialize`
  (`src/views.rs:1330`).
- The only `Self { … }` for `Object` is inside `new`.
- `views_tests.rs` is a `#[path]` child of `views.rs` and *may* write the private
  fields; it does not.
- `ui_tests.rs` and `main_tests.rs` are children of other files and cannot.

## R5. `asks`, drawn or not

```
$ grep -n "\.asks" src/ui.rs
2098:/// than merely unused: `dialog.asks`, the consequence's wrapped length and the command's width
2146:    let confirm = match dialog.asks {
2281:    let field = usize::from(dialog.asks.is_some()) * FIELD_ROWS;
2369:    if dialog.asks.is_some() {
```

Three call sites, all on the discriminant (`match … { Some(_) => … }`,
`.is_some()`). The value reaches no `Span` anywhere in `ui.rs`.

## R6. What can set `Dialog::warning` in the console

```
$ grep -rn "\.warning = \|warning:" src/main.rs | grep -v "^src/main.rs:72"
src/main.rs:9953:            warning: None,
```

One occurrence. `Published::Checked { verdict, asks }` (`src/main.rs:8818`) carries
no field for `ops::Checked::returned()`, and `installed` (`src/main.rs:10023`) is
the only thing that mutates an open dialog other than `over_modal`'s `typed`.
`ops::Checked::returned()` is `pub` at `src/ops.rs:358`; `while_paused` is read at
`src/main.rs:7271`, inside the headless `restarted` only.
