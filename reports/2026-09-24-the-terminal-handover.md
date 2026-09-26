# The terminal handover — job control, measured on a pty

`k8s-admin`, 2026-09-24, step 6 of Phase 12's **Ctrl-Z handover + panic-safe
terminal teardown** family (`src/main.rs` § THE TERMINAL HANDOVER, uncommitted).

**No cluster and no test host were touched.** Every measurement below is a local
pty on the dev machine driving an interactive shell through job control, plus
reads of the vendored `ratatui-core-0.1.2`, `ratatui-0.30.2` and `crossterm-0.29.0`
sources in `~/.cargo/registry`.

## 1. The harness

Two files, written to the session scratchpad (not committed):

- `rawtui.py` — a stand-in for the console: `tty.setraw(0)` + `\x1b[?1049h` +
  `\x1b[?25l`, then a loop that logs `termios.tcgetattr(0)`'s `lflag` bits,
  `getpgrp()` and `tcgetpgrp(0)` every 250 ms.
  - `selfstop` mode models `suspended()`: restore termios, `\x1b[?1049l\x1b[?25h`,
    `os.kill(os.getpid(), SIGSTOP)`, then re-take.
  - `nostop` mode models **the absence of a SIGTSTP watch**: sit in raw mode and
    let an external `SIGTSTP` land with no handler.
- `drive.py` — `pty.openpty()`, `fork`, `setsid`, `TIOCSCTTY`, exec the shell
  `-i`, then feed it the command, signal the child, and send `jobs`, `stty -a`,
  `fg` (or `bg`).

```
python3 drive.py <bash|zsh|fish> <selfstop|nostop> <case>
```

`ICANON=0 ECHO=0 ISIG=0` is raw. `ICANON=1 ECHO=1 ISIG=1` is the shell's cooked
mode.

## 2. `ctrl-z` as the code does it — `raise(SIGSTOP)` after the handover

`python3 drive.py bash selfstop bash-selfstop`

Shell session (paths elided, `<ESC>` for `\x1b`):

```
PROMPT$ python3 RAWTUI selfstop LOG PID
<ESC>[?1049h<ESC>[?25l<ESC>[?1049l<ESC>[?25h
[1]+  Stopped                    python3 RAWTUI selfstop LOG PID
PROMPT$ jobs
[1]+  Stopped                    python3 RAWTUI selfstop LOG PID
PROMPT$ fg
python3 RAWTUI selfstop LOG PID
<ESC>[?1049h<ESC>[?25l<ESC>[?1049l<ESC>[?25h
```

termios log:

```
before raw                   ICANON=1 ECHO=1 ISIG=1  pgrp=N tcpgrp=N
after take()                 ICANON=0 ECHO=0 ISIG=0  pgrp=N tcpgrp=N
after handed_back()          ICANON=1 ECHO=1 ISIG=1  pgrp=N tcpgrp=N
after SIGSTOP returned       ICANON=1 ECHO=1 ISIG=1  pgrp=N tcpgrp=N
after taken_back()           ICANON=0 ECHO=0 ISIG=0  pgrp=N tcpgrp=N
loop                         ICANON=0 ECHO=0 ISIG=0  pgrp=N tcpgrp=N
exit                         ICANON=1 ECHO=1 ISIG=1  pgrp=N tcpgrp=N
```

bash prints plain `Stopped`, not `Stopped(SIGSTOP)`, for a job stopped by
`SIGSTOP`.

## 3. The same program stopped by an external `SIGTSTP` with no handler

`python3 drive.py bash nostop bash-nostop` — `os.kill(child, SIGTSTP)` from
outside, then `fg`.

Shell session:

```
PROMPT$ python3 RAWTUI nostop LOG PID
<ESC>[?1049h<ESC>[?25l
[1]+  Stopped                    python3 RAWTUI nostop LOG PID
PROMPT$ jobs
[1]+  Stopped                    python3 RAWTUI nostop LOG PID
PROMPT$ stty -a | head -2
speed 38400 baud; rows 24; columns 80; line = 0;
intr = ^C; quit = ^\; erase = ^?; kill = ^U; eof = ^D; eol = <undef>;
PROMPT$ fg
python3 RAWTUI nostop LOG PID
q
<ESC>[?1049l<ESC>[?25h
```

There is no `<ESC>[?1049l` between the `1049h` and the `[1]+ Stopped` line: the
shell's prompt, the `jobs` output and the `stty -a` output were all drawn inside
the alternate screen, with the cursor still hidden.

termios log (repeated `loop` lines collapsed):

```
before raw                   ICANON=1 ECHO=1 ISIG=1
after take()                 ICANON=0 ECHO=0 ISIG=0
loop  x10                    ICANON=0 ECHO=0 ISIG=0     <- stopped here
loop  x16                    ICANON=1 ECHO=1 ISIG=1     <- after `fg`
exit                         ICANON=1 ECHO=1 ISIG=1
```

After `fg` the program is running again and the tty is **cooked**: `ICANON=1
ECHO=1 ISIG=1`. The `q` in the session transcript is the shell echoing the
keystroke — it reached the program only after the driver also sent a newline.

Same case under zsh (`python3 drive.py zsh nostop zsh-nostop`):

```
before raw                   ICANON=1 ECHO=1 ISIG=1
after take()                 ICANON=0 ECHO=0 ISIG=0
loop  (all)                  ICANON=1 ECHO=1 ISIG=1
```

fish 4.9.3 was **not** measured: `fish --no-config -i` on this harness blocks on
its startup terminal-capability probes (`<ESC>[?u`, `<ESC>[>0q`, `<ESC>]11;?`,
`<ESC>P+q696e646e`, DA1) and never reaches a prompt against a pty that answers
none of them.

## 4. `bg` instead of `fg` after the handover

`python3 drive_bg.py bash selfstop bash-selfstop-bg`

```
PROMPT$ bg
[1]+ python3 RAWTUI selfstop LOG PID &
PROMPT$ q
bash: q: command not found
[1]+  Stopped                    python3 RAWTUI selfstop LOG PID
PROMPT$
exit
There are stopped jobs.
```

termios log ends at:

```
after handed_back()          ICANON=1 ECHO=1 ISIG=1  pgrp=3411956 tcpgrp=3411956
after SIGSTOP returned       ICANON=0 ECHO=0 ISIG=1  pgrp=3411956 tcpgrp=3411876
```

No `after taken_back()` line: the `tcsetattr` inside `take()` ran from a
background process group and `SIGTTOU` stopped the process inside it. The
`ICANON=0 ECHO=0 ISIG=1` reading at that point is the *shell's* readline termios,
read by a background process (`tcgetattr` raises no signal). No `<ESC>[?1049h`
reached the terminal, because the `tcsetattr` comes before the escape sequence.

## 5. Library source read, not inferred

`~/.cargo/registry/src/index.crates.io-*/`

- `ratatui-core-0.1.2/src/terminal/buffers.rs:147` — `Terminal::clear` opens with
  `let original_cursor = self.backend.get_cursor_position()?;`.
- same file `:158` — `Terminal::clear_viewport` for `Viewport::Fullscreen` is
  `self.backend.clear_region(ClearType::All)` and reads nothing.
- `ratatui-core-0.1.2/src/terminal/resize.rs:23` — `Terminal::resize` calls
  `set_viewport_area` then `clear_viewport`, never `clear`, and sets
  `last_known_area = area`.
- `ratatui-core-0.1.2/src/terminal.rs:477` — `Terminal::drop` sends `show_cursor`
  only `if self.hidden_cursor`.
- `ratatui-0.30.2/src/init.rs:397` — `try_init` = `set_panic_hook(); enable_raw_mode()?;
  execute!(stdout(), EnterAlternateScreen)?;` — the hook is installed **before**
  raw mode, so it is installed even when `try_init` returns `Err`.
- `ratatui-0.30.2/src/init.rs:554` — `try_restore` = `disable_raw_mode()?;
  execute!(stdout(), LeaveAlternateScreen)?;` — no cursor.
- `ratatui-0.30.2/src/init.rs:566` — `set_panic_hook` = `let hook = take_hook();
  set_hook(|info| { restore(); hook(info); })`.
- `ratatui-0.30.2/src/init.rs:524` — `restore()` on failure prints
  `Failed to restore terminal: {err}` to stderr.
- `crossterm-0.29.0/src/terminal/sys/unix.rs:108` — `enable_raw_mode` opens with
  `let mut original_mode = TERMINAL_MODE_PRIOR_RAW_MODE.lock(); if
  original_mode.is_some() { return Ok(()); }` — it does **not** touch the tty when
  crossterm already believes raw mode is on.
- same file `:148` — `disable_raw_mode` restores the stored termios and sets the
  slot to `None`.
- same file `:14`, `:30` — the slot is a `parking_lot::Mutex`, not reentrant and
  not poisoned.

## 6. Source facts read off this tree

```
$ grep -rn "libc::" src/
src/main.rs:7660:    unsafe { libc::raise(libc::SIGSTOP) };

$ grep -rn "tokio::signal" src/
(no output)

$ grep -rn -i "suspend\|ctrl-z" screens/help.md
(no output)

$ grep -n "^name = \"signal-hook-registry\"\|^name = \"mio\"\|^name = \"signal-hook\"" Cargo.lock
1430:name = "mio"
2262:name = "signal-hook"
2283:name = "signal-hook-registry"

$ grep -n "\.unwrap()\|\.expect(\|panic!\|unreachable!\|todo!" src/main.rs src/k8s.rs src/ops.rs src/views.rs src/ui.rs src/rules.rs src/analysis.rs src/theme.rs
src/views.rs:3372:    /// *ignored* draw the same screen and only one of them needs a `panic!`.
src/main.rs:7584:                    // it is a `continue` and not a `panic!`.
(both are comments; no panicking call in product code)

$ grep -n "CHECK_DEADLINE\s*:" src/ops.rs
898:pub(crate) const CHECK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(35);

$ grep -rn "should_panic\|catch_unwind" src/ | grep -v main.rs:77
src/ops_tests.rs:1860:#[should_panic(expected = "nothing to state on screen")]
src/ops_tests.rs:5505:#[should_panic(expected = "requires the object's name")]
src/ops_tests.rs:5516:#[should_panic(expected = "a press confirms")]
src/main_tests.rs:11354:#[should_panic(expected = "replayed")]
src/rules_tests/node.rs:1363:        match std::panic::catch_unwind(|| quantity_milli(q)) {
src/rules_tests/node.rs:1436:    let got = std::panic::catch_unwind(|| node_overcommitted(&snapshot, &node));
```

## 7. Not measured

- fish, for the reason in § 3.
- A real `k8rs` binary under any of this — nothing was built or run on the test
  host this round.
- The flake rate of `the_panic_hook_chains_the_one_it_replaced` against the six
  deliberate-panic sites in § 6; that needs repeated `cargo test --locked` runs
  on the host.
- Whether enabling `tokio`'s `signal` feature moves `Cargo.lock`'s 319 packages.
  `cargo tree -e features,no-dev -i tokio` on the host answers it.

## 8. Re-derived on this machine rather than read off a comment

```
$ grep -c '^\[\[package\]\]' Cargo.lock
319

$ grep -n -A2 '^name = "libc"' Cargo.lock
1330:name = "libc"
1331-version = "0.2.189"
1332-source = "registry+https://github.com/rust-lang/crates.io-index"
```

`PRIOR-ART.md:364-378`, § D4 — the terminal after a subprocess. First issue
cited: k9s [#1690](https://github.com/derailed/k9s/issues/1690), *"after exiting
a container shell the panels redraw but the arrow keys are dead"*. The entry is
marked **covered** by invariant 8 and D24, i.e. by this family.

`scripts/handover-guard.py` exists in the working tree and is untracked —
`tester`'s step-5 work, landing in parallel. It refuses a stdin-reading call in
the handover region by name and states the same `Terminal::clear` /
`Terminal::resize` fact measured in § 5 above.
