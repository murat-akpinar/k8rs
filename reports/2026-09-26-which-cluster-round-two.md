# 2026-09-26 — the *which cluster* family, second operator read

`k8s-admin`, round two over the same working tree after
[D280](../NOTES.md#d280--the-which-cluster-review-round-a-header-slot-with-no-vocabulary-a-mockup-that-cannot-be-drawn-and-four-tests-weaker-than-they-read-2026-09-26)'s
fixes. **Nothing was run on the test host** — `tester` held it for
`just check` and `just picker`. Source reads on the dev machine only;
where a claim needs a run, the run is named and marked *not run*.

Round one's evidence is `2026-09-25-the-which-cluster-family.md`; this file
records only what is new.

## R1 — the taught command in the failure box is not shell-quoted

`src/views.rs`, § WHY A CALL DID NOT WORK:

```
$ sed -n '/pub fn run_the_login/,/^}/p' src/views.rs
pub fn run_the_login(context: Option<&str>) -> String {
    format!(
        "Run it yourself first: `kubectl {CONTEXT} {} get ns`. Then try again.",
        context.unwrap_or(UNNAMED)
    )
}
```

The other taught surface, for comparison:

```
$ sed -n '3144p' src/main.rs
        Some(name) => format!("$ kubectl {CONTEXT} {}", ops::pasteable(&name)),
```

```
$ grep -n 'pasteable' src/views.rs
(no output)
```

`views.rs` already reaches into `ops`, so nothing is plumbed for the call:

```
$ grep -n 'crate::ops::' src/views.rs | head -3
78:use crate::ops::Verdict;
2666:                && crate::ops::scalable(kind).is_ok_and(|served| served.group == group),
2667:            restartable: crate::ops::restartable(kind).is_ok_and(|served| served.group == group),
```

**What the name can hold when it gets there.** `to` is `k8s::Choice::name`,
which is `drawable` → `text(value, IDENTIFIER)`. That filter removes
unprintable characters and caps length; it is not a charset allowlist:

```
$ sed -n '284,298p' src/k8s.rs
pub(crate) fn text(value: &mut String, cap: usize) {
    let mut kept = String::new();
    let mut break_pending = false;
    for character in value.chars() {
        if !unprintable(character) {
            if break_pending && !kept.is_empty() && !kept.ends_with(' ') {
                kept.push(' ');
            }
            break_pending = false;
            kept.push(character);
        } else if character.is_whitespace() {
            break_pending = true;
        }
    }
```

```
$ sed -n '/pub fn pasteable/,/^}/p' src/ops.rs
pub fn pasteable(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:/@+=".contains(c))
    {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}
```

Field values the finding turns on, for a context the kubeconfig names
`prod eu; echo pwned` (the name
[D278 ruling 5](../NOTES.md#d278--the-flags-box-what-had-to-be-ruled-before-it-could-be-briefed-a-record-that-named-the-wrong-cluster-and-opsrs-reopens-for-a-taught-command-2026-09-24)
measured on the real binary for the sibling surface):

| value | result |
|---|---|
| `Choice::name` after `text` | `prod eu; echo pwned` — unchanged, every byte printable |
| `main::kubectl` → `ops::pasteable` | `$ kubectl --context 'prod eu; echo pwned'` |
| `views::run_the_login` | ``Run it yourself first: `kubectl --context prod eu; echo pwned get ns`. Then try again.`` |

And for a name that strips to nothing (`k8s::drawable` answers `None`):

| surface | result |
|---|---|
| `main::kubectl` | `$ kubectl` — the segment is dropped, its own doc: *"`--context ` with an empty value after it is a line that does not run, so there is nothing to teach"* |
| `views::run_the_login` | ``Run it yourself first: `kubectl --context (unnamed) get ns`. Then try again.`` — `UNNAMED` |

The box's paragraph is bounded by a prose cut, not by a command-aware one:

```
$ sed -n '3142p' src/ui.rs
    let paragraph = cut(why, columns, MODAL_ROWS.saturating_sub(5 + way_out.len()));
```

**Not run:** the sibling of D278's own measurement — the real binary against
a kubeconfig whose context is renamed `prod eu; echo pwned`, switched to with
`X`, with the failure box's paragraph read off the screen.

## R2 — the strip after a failed switch is two lines, and the mockups draw one

```
$ grep -n 'const LOG_LINES' src/ui.rs
88:const LOG_LINES: u16 = 2;
```

```
$ grep -n 'console.log.ran(views::GET_CONTEXTS' src/main.rs
7940:            console.log.ran(views::GET_CONTEXTS.to_owned());
9382:            console.log.ran(views::GET_CONTEXTS.to_owned());
```

`7940` is the `Opens::Asking` arm, `9382` is `pressed`'s `X` arm. Those are
the only two sites that build a `Modal::ContextPick`, and
`views::Chosen::Connect` is reachable from nowhere else — so at the moment a
switch is asked for, the log's last line is always `GET_CONTEXTS`. The clear
moved off `⏎`:

```
$ grep -n 'console.log = views::Log::default' src/main.rs
8177:    console.log = views::Log::default();
```

`8177` is inside `connected`, i.e. on success only. `views::App::switched` no
longer takes the log at all:

```
$ sed -n '/pub fn switched(&mut self)/,/^    }/p' src/views.rs
    pub fn switched(&mut self) {
        *self = App::default();
    }
```

Strip contents, by frame, mid-session path:

| frame | strip row 1 | strip row 2 (the one the eye lands on) |
|---|---|---|
| picker open (`X`) | `$ kubectl --context prod-eu get daemonsets -A --watch` | `$ kubectl config get-contexts` |
| `connecting…` | same | same |
| failure box | same | same |
| after `esc dismiss` | same | same |
| after a *successful* switch | the new context's first line | the new context's second line |

Startup path: the log holds `GET_CONTEXTS` alone, so the strip is one line.

The `ui.rs` test fixture models the two-line shape:

```
$ sed -n '/fn failed_under/,+8p' src/ui_tests.rs | tail -5
    let log = [
        Stripped::of("$ kubectl get daemonsets -A --watch"),
        Stripped::of(views::GET_CONTEXTS),
    ];
```

`screens/context.md`'s two mid-session mockups draw one strip row each:

```
$ grep -n 'kubectl --context prod-eu get pods -A --watch' screens/context.md
(inside § When the new cluster does not work and § After `esc dismiss`)
```

The `main_tests.rs` switch test seeds a one-line log with no `GET_CONTEXTS`,
so the assertion `console.log.lines().len() == 1` is over a shape the running
console does not produce on that path.

**Not run:** `just picker` against a kubeconfig with two contexts, one of
them broken — the two strip rows read off the pty transcript on the frame
after `esc dismiss`.

## R3 — the carried halt

```
$ sed -n '7980,7998p' src/main.rs
    let mut carried: Option<Halt> = None;
    ...
    loop {
        let halt = match carried.take() {
            Some(halt) => halt,
            None => {
                pump(
```

```
$ sed -n '8072p' src/main.rs
                    halt @ (Halt::Mutate(_) | Halt::Switch(_)) => carried = Some(halt),
```

Control flow, verified by read: the inner `pump` still does not return when
`settled` runs, so the key is live inside it; the halt it produces is now
stored and taken by the next turn of the outer loop, where both arms handle
it. `wanting` and the `Chosen::Connect` arm mutate nothing before returning,
so nothing is half-applied either way.

What the new test reaches:

```
$ grep -n 'fn a_key_pressed_the_moment_a_mutation_settles_is_not_refused' src/main_tests.rs
```

It drives `settled` then `keyed`, and asserts `Did::Mutate` and
`Did::Changed`. `keyed` answered those before the fix as well — the drop was
one level up, in `console()`'s `match halt`, which no test reaches (it needs
a terminal and a cluster).

**Not run:** the run that would cover the regression — `just picker`, or a
`suspend-test`-shaped pty script: restart something, wait for the dialog to
close, press `r` once, and assert a confirm box is on the next frame rather
than on the frame after the second press.

## R4 — the header's connection slot, every state it can be in

```
$ sed -n '/^fn not_connected/,/^}/p' src/main.rs
fn not_connected() -> String {
    format!("{} not connected", ui::mark(theme::ALARM))
}
```

```
$ sed -n '8289,8293p' src/main.rs
            console.context = views::Stripped::of(&format!(
                "{} · {}",
                zone(asked.to.as_deref(), namespace),
                not_connected()
            ));
```

The word is joined in `switched`'s `Err` arm only, and `Screen::context` is
rebuilt from `zone` on every connect and every switch, so it cannot
accumulate. Field values per state:

| state | `unconnected` | `Screen::context` | `Link::state()` | header right zone |
|---|---|---|---|---|
| startup picker | `true` | `""` | — (`header` takes the `picking` branch) | `choose a cluster · admin` |
| `connecting…`, any switch | `false` | `ctx: aws-staging` | `Some("connecting…")` | `ctx: aws-staging · connecting… · admin` |
| mid-session failure, box open | `true` | `ctx: aws-staging · ⚠ not connected` | `None` | `ctx: aws-staging · ⚠ not connected · admin` |
| after `esc dismiss` | `true` | unchanged | `None` | unchanged |
| startup failure, box open | `true` | `ctx: aws-staging · ⚠ not connected` | `None` | same |
| startup failure, `esc` → picker | `true` | stale, **not drawn** | — (`picking` branch) | `choose a cluster · admin` |
| mid-session picker reopened by `X` | `true` | `ctx: aws-staging · ⚠ not connected` | `None` | `ctx: aws-staging · ⚠ not connected · admin` |
| second failed switch in a row | `true` | `ctx: still-not-in-the-file · ⚠ not connected` | `None` | rebuilt, no accumulation |

Both `⚠` marks on the frame come from one spelling:

```
$ grep -n 'ui::mark(theme::ALARM)' src/main.rs
8666:        format!("{} not connected", ui::mark(theme::ALARM))
9196:                "{} Not connected to the cluster right now.",
```

## R5 — checked and unchanged

| checked | where | result |
|---|---|---|
| the word is not claimed before it is knowable | `switched` draws at `8272`, joins the word at `8289` after `connect_with` answers | the `connecting…` frame carries none; the test forbids `⚠ not connected` on it |
| `--read-only` still outlives `X` | `switched` never touches `Console::writes`; asserted in `a_switch_that_cannot_connect_…` | `ui::Writes::ReadOnly` |
| audit `server:` and `context` after a switch | unchanged from round one (`connected` at `8146-8148`) | the connected row |
| the header's front-cut order with the new word | joined string is `ctx: X · ns: Y · ⚠ not connected · admin`; `ui::shortened` eats the front | the name erodes first, the word and `admin` survive (D249's order) |
| `Modal::ContextPick` producers all log `GET_CONTEXTS` | `src/main.rs:7940`, `9382`; `App::escape` reopens a stored picker over a log that already holds it | the strip's last line on a not-connected frame is never a cluster line |
| the paragraph cannot burst the modal ceiling | `cut(why, columns, MODAL_ROWS - (5 + way_out.len()))` | bounded with a visible mark |
| invariant 10's flag count survives `CONTEXT` moving to `views.rs` | `grep -oE '^(pub )?const [A-Z_]+: &str = "--[a-z-]+"' src/main.rs src/views.rs` | 15 flags, all found — `views.rs` was already in the counting command (D264 ruling 14) |
| `run_the_login`'s gate | `(fault == Fault::NoCredential && renewal.is_some())` (`src/ui.rs:3066`); `renewal` is the kubeconfig's `exec.command` | shows on exactly the contexts that log in with a program; `BadEntry` and `Unanswered` get none |
| `get ns` as the probe | a cluster-scoped `list namespaces`, not the cluster-wide pod access the real connect asks for | exercises the login without needing the permission the connect is about to test |
