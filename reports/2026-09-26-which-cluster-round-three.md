# 2026-09-26 — the *which cluster* family, third operator read

`k8s-admin`, round three over the same working tree after
[D281](../NOTES.md#d281--round-two-the-fix-that-broke-the-quoting-rule-a-probe-that-was-not-one-and-a-frame-that-is-honest-as-built-and-misreading-as-drawn-2026-09-26)'s
fixes. **Nothing was run on the test host.** Source reads on the dev machine
only. Rounds one and two are `2026-09-25-the-which-cluster-family.md` and
`2026-09-26-which-cluster-round-two.md`; this file records only what is new.

## T1 — the quoting fix, and the order it runs in

```
$ sed -n '/pub fn run_the_login/,/^}/p' src/views.rs
pub fn run_the_login(context: Option<&str>) -> Option<String> {
    let name = context.map(sanitize).filter(|name| !name.is_empty())?;
    Some(format!(
        "Run it yourself first: `kubectl {CONTEXT} {} version`. Then try again.",
        crate::ops::pasteable(&name)
    ))
}
```

Same order as the other taught surface — strip, test the gap, then quote:

```
$ sed -n '3154,3159p' src/main.rs
    match context.map(sanitize).filter(|name| !name.is_empty()) {
        Some(name) => format!("$ kubectl {CONTEXT} {}", ops::pasteable(&name)),
        None => "$ kubectl".to_string(),
    }
```

Value table, the two inputs round two turned on:

| context name in the kubeconfig | `run_the_login` |
|---|---|
| `prod eu; echo pwned` | ``Run it yourself first: `kubectl --context 'prod eu; echo pwned' version`. Then try again.`` |
| all-strippable, or `name: ""` | `None` — no next step |

## T2 — what the decline actually draws

`to` is `k8s::Choice::name`, `None` when `drawable` stripped it to nothing.
The box is still built; only the next step is absent:

```
$ sed -n '3152,3153p' src/ui.rs
    let tail = format!(" {outcome}");
    let to = to.unwrap_or(views::UNNAMED);
```

Field values on that frame:

| slot | value |
|---|---|
| title | `(unnamed) could not be opened` |
| paragraph | `` The program this kubeconfig logs in with (`aws`) gave k8rs nothing to sign in with. `` |
| way out | `Nothing is wrong with prod-eu — X takes you back.` |
| button | `[ esc dismiss ]` |

The row is reachable: `views::landable` excludes only `Address::Undefined`,
and `screens/context.md` § A context whose name strips to nothing states
*"`⏎` goes somewhere fine"*.

```
$ sed -n '/pub fn landable/,/^}/p' src/views.rs
pub fn landable(row: &Choice) -> bool {
    row.shadowed || row.server != Address::Undefined
}
```

```
$ grep -c 'strips to nothing' screens/context.md
2
```

Both hits are § Unhappy states' index line and the picker section; neither is
in § When the new cluster does not work.

## T3 — `parked`, and what else the loop reads after it moves

```
$ sed -n '/^fn parked/,/^}/p' src/main.rs
fn parked(carried: &mut Option<Halt>, mutating: bool, halt: Halt) -> Halt {
    if mutating {
        *carried = Some(halt);
        Halt::Carried
    } else {
        halt
    }
}
```

```
$ sed -n '8829,8835p' src/main.rs
    if let Some(halt) = console.carried.take() {
        return halt;
    }
    ...
    let mutating = running.is_some();
```

The entry read is load-bearing, confirmed against the settle arm:

```
$ sed -n '8923,8926p' src/main.rs
            }, if running.is_some() => {
                settled(console, performed);
                running = None;
                owing.now();
            }
```

`running` is the local `Option<Running>`; after that arm `running.is_some()`
is `false`, so a `parked(..., running.is_some(), ...)` written at the key
site would park nothing.

Every other read of `running` inside the loop, after the settle:

| read | guarded by | after settle |
|---|---|---|
| `running.as_mut()` in the `performed` arm | `if running.is_some()` | not polled |
| `running.as_ref()` in `Did::Answered` | `if let Some` | skipped — and unreachable, `settled` leaves no `Modal::Confirm` |
| `running.as_ref()` for the `shown` drain, loop tail | `if let Some` | **skipped** — anything published in the future's final poll is never drained |

The drain's own comment states the opposite:

```
$ sed -n '8938,8940p' src/main.rs
        // **Read after the `select!` and not inside one arm**: `show` and `ask` are called from
        // inside the mutation's own future, so whatever they published is here once that future has
        // been polled — and a dialog that opened or armed is a frame owed.
```

What keeps parking safe while the call is still on the wire is a guard in
another file:

```
$ sed -n '/pub fn may_switch_cluster/,+2p' src/views.rs
    pub fn may_switch_cluster(&self) -> bool {
        self.modal.is_none() && self.changing.is_none() && self.typing.is_none()
```

`App::changing` is `Some` from the confirm `⏎` until `settled` takes it, so
`Did::Mutate` and `Did::Switch` cannot be produced in that window. The same
field is what `may_quit` already relies on for the same reason (*"Quitting
mid-`PATCH` would leave the audit log holding an attempt with no result"*).
`parked`'s doc names neither.

Coverage, read rather than run:

```
$ grep -n 'fn a_halt_a_mutation_frame_cannot_act_on_is_parked\|fn pump_answers_with_what_was_parked\|fn a_key_pressed_the_moment_a_mutation_settles_leaves_the_frame_parked' src/main_tests.rs
18447, 18499, 18552
```

The third drives the real `pump` with a real `Running`, waits `COALESCE * 20`
so the key lands after the settle, and asserts both `Halt::Carried` and the
slot's contents — so reading `running.is_some()` at the key site is red.

## T4 — the overlong-token case in the new sentence

An EKS context name is one `pasteable` token — `arn:aws:eks:…:cluster/prod`
is 55 characters of `[A-Za-z0-9-_.:/@+=]`, so it is not quoted and does not
break on whitespace. `wrapped` hard-breaks a word wider than the column:

```
$ sed -n '/while width(rest) > columns {/,+8p' src/ui.rs
        while width(rest) > columns {
            let head = fits(rest, columns);
            if head.is_empty() {
                break;
            }
            lines.push(head.to_owned());
            rest = &rest[head.len()..];
        }
```

and `cut` bounds the result at `MODAL_ROWS - (5 + way_out.len())`
(`src/ui.rs:3142`), so the box cannot burst its ceiling. Row estimate for the
ARN case, **arithmetic rather than a render**: reason ~82 chars + next ~128
chars at ~50 columns ≈ 5 rows against a budget of 7.

**Not run:** the render — `just picker` against a kubeconfig whose context is
an EKS ARN with a failing `exec` block, with the box's row count read off the
pty transcript.

## T5 — checked and unchanged

| checked | result |
|---|---|
| `Console::carried` cannot outlive one loop turn | `parked` returns `Halt::Carried`; both `match halt` arms answer `continue`; the next `pump` drains at entry before anything else |
| a parked halt costs no extra frame | the parking `pump` returns without drawing and the draining one returns without drawing, so key → dialog is one frame, as on the unparked path |
| `carried` under `--read-only` | `may_mutate` is false, so `Did::Mutate` is not produced; the `audit.as_mut()` `continue` is a second, redundant floor |
| `views.rs` → `ops.rs` is a downward call | pyramid order `ops.rs → … → views.rs`; `views.rs` already calls `ops::scalable`, `restartable`, `Verdict` |
| the box and the strip quote one way | both `sanitize` then `pasteable`, same order, one helper |
| `views::sanitize` cannot widen what `k8s::text` left | `text` runs at ingest and removes unprintables; `sanitize` can only remove more, and `pasteable` quotes whatever survives |
| the page's mockup line fits | `  first: \`kubectl --context aws-staging version\`. Then` is 54 columns against a 54-column interior |
