# 2026-09-25 — the *which cluster* family, read at HEAD + working tree

`k8s-admin`, step 6 of Phase 12's *One place decides which context is used* /
*The cluster picker is wired* (NOTES § D279), reviewed uncommitted in the
working tree. **No cluster was brought up and nothing was run on the test
host** — `tester` held the mirror. Everything below is a read of source files
on the dev machine: the repo tree, and `kube-client 4.2.0` as
`Cargo.lock` pins it. Where a claim needs a cluster to settle, the
command that would settle it is written out and marked *not run*.

## M1 — what the header row draws while nothing is connected

Three reads, one frame.

```
$ sed -n '1841,1855p' src/ui.rs
    let state = screen.link.state();
    let segments: Vec<&str> = if picking {
        vec!["choose a cluster", screen.writes.permission()]
    } else {
        [
            Some(screen.context.as_str()),
            state.as_deref(),
            Some(screen.writes.permission()),
            screen.insecure.then_some(tls.as_str()),
            app.changing.is_some().then_some(mark(theme::CHANGING)),
        ]
        .into_iter()
        .flatten()
        .collect()
    };
```

```
$ sed -n '1014,1022p' src/ui.rs
    fn state(self) -> Option<String> {
        match self {
            Link::Connecting => Some("connecting…".to_owned()),
            Link::Live => Some("live".to_owned()),
            Link::Lost => Some(format!("{} disconnected, retrying", mark(theme::ALARM))),
            Link::Expired => Some(format!("{} login expired", mark(theme::ALARM))),
            Link::Unconnected => None,
        }
    }
```

```
$ sed -n '8255,8256p' src/main.rs
    console.unconnected = false;
    console.context = views::Stripped::of(&zone(asked.to.as_deref(), namespace));
```

`zone(Some("staging"), None)` is `"ctx: staging"` (`src/main.rs:8609`). No
call site writes a fault word into `Screen::context`:

```
$ grep -rn 'not allowed' src/*.rs | grep -v _tests | grep -c 'format!\|push_str'
0
```

Field values the finding turns on, for a mid-session switch that failed with
`Fault::Refused`:

| value | what it is |
|---|---|
| `Console::unconnected` | `true` |
| `Screen::link` | `Link::Unconnected` |
| `Link::state()` | `None` |
| `Screen::context` | `ctx: staging` |
| `Screen::writes.permission()` | `admin` |
| joined header right zone | `ctx: staging · admin` |

The binding text, `screens/widgets.md` § 1a:

```
$ sed -n '84,86p' screens/widgets.md
  work](context.md#when-the-new-cluster-does-not-work) writes into the box
  itself (`⚠ not allowed` for a `Refused`) — not a fifth connection word, and
  not a blank segment either, for as long as nothing is connected
```

The assertion in the turn's own test, `src/main_tests.rs` § WHICH CLUSTER,
`nothing_connected_draws_no_context_at_startup_and_no_connection_word_after_a_failure`:

```rust
    assert!(
        after.contains("ctx: staging · admin"),
        "the header lost the context the reader chose:\n{after}"
    );
```

## M2 — the inner `pump` keeps running after a mutation settles

`src/main.rs`, § THE CONSOLE. The mutation arm of `pump` does not return:

```
$ sed -n '8830,8839p' src/main.rs
            performed = async {
                match running.as_mut() {
                    Some(running) => running.performing.as_mut().await,
                    None => std::future::pending().await,
                }
            }, if running.is_some() => {
                settled(console, performed);
                running = None;
                owing.now();
            }
```

`settled` clears the field both liveness gates read:

```
$ sed -n '/^fn settled(/,+6p' src/main.rs | grep 'changing'
    let object = console.app.changing.take();
```

```
$ grep -n 'fn may_switch_cluster' -A 3 src/views.rs
2722:    pub fn may_switch_cluster(&self) -> bool {
2723-        self.modal.is_none() && self.changing.is_none() && self.typing.is_none()
```

The caller's arm for the two halts that arrive after that point:

```
$ sed -n '8064,8070p' src/main.rs
                    // **A second mutation cannot be asked for while one is running**
                    // (`views::App::may_mutate` reads `App::changing`), and neither can a switch
                    // (`views::App::may_switch_cluster` reads the same field), so both are
                    // unreachable rather than ignored — and unreachable and ignored draw the same
                    // screen, so it is a `continue` and not a `panic!`.
                    Halt::Mutate(_) | Halt::Switch(_) => continue,
```

`wanting` (`src/main.rs:9755`) changes no state before returning
`Did::Mutate`; `over_modal`'s `Chosen::Connect` arm (`src/main.rs:9481`)
changes none before returning `Did::Switch`.

**Not run:** the test that would settle it, on the test host —
`cargo test --locked --all-targets` over a `pump` driven with a `TestBackend`:
complete one `Did::Answered(Some(..))` mutation, then feed `r`, and assert the
returned `Halt` reaches `switched`/`mutating` rather than the caller's
`continue`.

## M3 — `interactive_mode: Never`, read off kube-client 4.2.0

```
$ sed -n '586,593p' ~/.cargo/registry/src/*/kube-client-4.2.0/src/client/auth/mod.rs
    let interactive = auth.interactive_mode != Some(ExecInteractiveMode::Never);
    if interactive {
        cmd.stdin(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());
    } else {
        cmd.stdin(std::process::Stdio::piped());
    }
```

The plugin's own diagnosis therefore stops being inherited and starts being
captured. Where it lands:

```
$ sed -n '53,62p' ~/.cargo/registry/src/*/kube-client-4.2.0/src/client/auth/mod.rs
    /// Failed to run auth exec command
    #[error("auth exec command '{cmd}' failed with status {status}: {out:?}")]
    AuthExecRun {
        /// The failed command
        cmd: String,
        /// The exit status or exit code of the failed command
        status: std::process::ExitStatus,
        /// Stdout/Stderr of the failed command
        out: std::process::Output,
```

When the plugin runs, relative to raw mode: `Auth::try_from` calls `auth_exec`
synchronously, and `Auth::try_from` is reached from `Client::try_from(config)`.

```
$ sed -n '345,348p' ~/.cargo/registry/src/*/kube-client-4.2.0/src/client/auth/mod.rs
        if let Some(exec) = &auth_info.exec {
            let creds = auth_exec(exec)?;
```

Call order in `console()` for the `Opens::With` path:

| line | call |
|---|---|
| `src/main.rs:7845` | `login_stays_off_the_terminal(kubeconfig)` |
| `src/main.rs:7885` | `k8s::connect_with(...)` → `Client::try_from` → `auth_exec` |
| `src/main.rs:7945` | `ratatui::try_init()` |

`src/k8s.rs:7341` is the `Client::try_from` line; `src/main.rs:7945` is
`try_init`. The plugin runs 60 source lines before raw mode is entered on
that path. On the `Opens::Asking` path the first `connect_with` is
`src/main.rs:8262`, inside `switched`, which is after `try_init`.

The sentence the reader is handed for the resulting `Fault::NoCredential`:

```
$ sed -n '3889,3891p' src/views.rs
        Fault::NoCredential => format!(
            "the program this kubeconfig logs in with{named} gave k8rs nothing to sign in with"
        ),
```

`screens/context.md`'s eleven-fault table, `NoCredential` row: *"no next step
of its own"*.

**Not run:** a kubeconfig whose `exec` block is a device-code login, connected
with and without the field set, on the dev machine under
`K8RS_CLUSTER=review` — what the plugin writes, its exit status, and what k8rs
then draws.

## M4 — which faults can build `Modal::Unconnected`

`connect_with` has exactly two error arms:

```
$ grep -n 'map_err(NotConnected::Kubeconfig)\|Err(failure) => Err(NotConnected::Client' src/k8s.rs
7294:    .map_err(NotConnected::Kubeconfig)?;
7356:        Err(failure) => Err(NotConnected::Client { failure, renewal }),
```

A 403 on the pod list is not one of them — it is a value on the `Ok` side:

```
$ sed -n '6873,6897p' src/k8s.rs
    if lists_pods(client, None, REPORT_FETCH).await {
        return Coverage::Cluster;
    }
    ...
    let Some(named) = context_scope(context_namespace) else {
        return if lists_pods(client, Some(FALLBACK_NAMESPACE), REPORT_FETCH).await {
            Coverage::Refused(FALLBACK_NAMESPACE.to_string())
        } else {
            Coverage::Blind(FALLBACK_NAMESPACE.to_string())
        };
    };
    Coverage::Refused(named.to_string())
```

`switched` builds the box with `sent: false` and nothing else:

```
$ grep -n 'sent: false,' src/main.rs | sed -n '1,4p'
8281:                sent: false,
```

and `ui::failed` reads no coverage and produces no next step on that side:

```
$ sed -n '/^fn failed(/,+14p' src/ui.rs | sed -n '8,14p'
    let local = matches!(
        fault,
        Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential
    );
    let (outcome, reason, next) = if !sent || local {
        let reason = views::because(fault, views::REACH, renewal, None);
        ("could not be opened", reason, None)
```

**Not run:** on kind under `K8RS_CLUSTER=review`, a context bound to a
`Role`-only login, switched to with `X` — which of *the box* and *the banner*
draws, and what the header's connection segment reads afterwards.

## M5 — the command-log strip across a switch

`App::switched` empties the log (`src/views.rs:2757-2760`), and nothing
appends to it again until `connected` runs:

```
$ grep -n 'console.log.ran\|console.app.switched\|for line in command_log' src/main.rs
7930:            console.log.ran(views::GET_CONTEXTS.to_owned());
8171:    for line in command_log(
8249:    console.app.switched(&mut console.log);
9339:            console.log.ran(views::GET_CONTEXTS.to_owned());
```

`8171` is inside `connected`; `8249` is the first statement of `switched`,
before `drawn` at `8261` and before `connect_with` at `8262`. `views::Log`
has no call to `outcome` on any connect path:

```
$ grep -n 'log.outcome' src/main.rs
(only inside `settled`, src/main.rs — the mutation path)
```

Field values, mid-session switch that failed:

| frame | `Screen::log` length |
|---|---|
| picker open (`X` pressed) | previous cluster's lines + `$ kubectl config get-contexts` |
| `connecting…` (after `⏎`) | `0` |
| failure box | `0` |
| after `esc dismiss` | `0` |

`screens/context.md` draws
`$ kubectl --context staging get pods -A --watch   → not allowed` in the last
three of those.

## M6 — `pasteable`, and which spelling of the name the taught line carries

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

No space in the allowlist, so `prod eu; echo pwned` becomes
`'prod eu; echo pwned'`. `--context` is placed immediately after `kubectl`
and before the verb (`src/main.rs:3158`), which is where `kubectl` resolves a
global flag.

Which name reaches it — the *drawn* one, not the kubeconfig key:

```
$ sed -n '8173p' src/main.rs
        &kubectl(session.context.as_deref()),
```

`Session::context` is `kubeconfig_context` → `drawable` → `text(&mut value,
IDENTIFIER)`, i.e. invariant 9's strip and a 512-byte cap
(`src/k8s.rs:7375-7378`, `src/k8s.rs` § the `IDENTIFIER` constant).
`connect_with` is handed `Switching::key` instead — `Choice::key`, the file's
raw spelling (`src/main.rs:8262`, `src/main.rs:7621`).

**Not run:** a kubeconfig context named with an embedded `U+200B`, connected
to, and the strip's line pasted into a shell beside `kubectl config
get-contexts`.

## M7 — `--namespace` across a switch

```
$ sed -n '8248,8251p' src/main.rs
    *cluster = nothing_connected(namespace);
```

```
$ sed -n '8262p' src/main.rs
    match k8s::connect_with(kubeconfig.clone(), Some(&asked.key), namespace).await {
```

`namespace` is `opening.namespace`, the process flag, unchanged across every
switch. `coverage()` short-circuits on it without asking the new cluster
anything:

```
$ sed -n '6864,6872p' src/k8s.rs
    if let Some(namespace) = asked {
        return Coverage::Asked(if namespace_name(namespace) {
            namespace.to_string()
        } else {
            context_scope(context_namespace)
                .unwrap_or(FALLBACK_NAMESPACE)
                .to_string()
        });
    }
```

`Screen::namespace` has one reader in `ui.rs`, the browser pane's title:

```
$ grep -n 'screen.namespace' src/ui.rs
4427:        .and(screen.namespace.as_ref())
```

`notes()` draws no scope sentence (`src/main.rs:9138-9187`), so the Alerts
paragraph for a healthy scoped run is
`"{n} pods and {m} nodes checked, none of them is in trouble right now."`

**Not run:** `k8rs --namespace payments` against a two-context kind pair under
`K8RS_CLUSTER=review`, switching to the cluster that has no `payments` — the
HTTP status of the pods LIST, and what the Alerts pane then prints.

## M8 — checked and found unchanged

Reads that produced no divergence, recorded so the next pass does not repeat
them:

| checked | where | result |
|---|---|---|
| `--read-only` survives `X` | `switched` never touches `Console::writes`; `ui::withheld` returns `Some(Held::Off)` for `Writes::ReadOnly` before any kind is consulted (`src/ui.rs:1673`) | `Offer::Move`, `may_mutate` false, `wanting` returns `Did::Nothing` |
| audit `server:` after a switch | `Cluster::server` = `current_server(&console.contexts)` where `contexts = k8s::contexts(kubeconfig, Some(&asked.key))` (`src/main.rs:8251`, `8143-8145`) | `current` row is the connected one; D278 ruling 4's defect does not recur |
| audit `context` after a switch | `mutating(..., session.context.as_deref()...)` (`src/main.rs:8039`) | the connected context |
| a switch while a write is on the wire | `may_switch_cluster` reads `App::changing`, and `X` also needs `modal.is_none()` | refused |
| `Coverage::Blind` does not hang the frame on *reading the cluster…* | a standing refusal settles the watch (`Watch::settled`, `src/k8s.rs`), so `still_listing()` empties, `snapshot()` answers `Some`, and `Pane::Denied` carries the banner | banner draws |
| `esc` on the startup picker ends the run at `0` | `App::escape` returns `true` only for a `startup()` `ContextPick` (`src/views.rs:3263`), `Did::Quit` → `Halt::Quit` → `console` answers `None` → `main` returns | exit `0`, nothing on stderr |
| `esc` on the startup failure box reopens the same picker | `src/views.rs:3267-3270` puts `Before::Picking`'s stored `Picker` back | two presses, no dead end |
| the picker's row indices survive a switch | `k8s::contexts` maps one row per kubeconfig entry; only `current` moves | stored `Picker::at` still names the same row |
| no kubeconfig at all is stderr before raw mode | `src/main.rs:7843-7846` precedes `try_init` at `7945` | wall, exit `2` |
| `login_stays_off_the_terminal` covers every entry | `for named in &mut kubeconfig.auth_infos` (`src/main.rs:7760`) | every `exec` block, not only the connected one |
| kube reads the field off the in-memory value at the refresh | the near-expiry refresh path re-runs `Auth::try_from(&auth_info)` over the stored `AuthInfo` clone, in `kube-client-4.2.0/src/client/auth/mod.rs` lines 210-222 | one field set covers connect and refresh |
| the plugin's captured output never reaches a k8rs string | `NotConnected` and `Trouble` both select named fields; neither derives `Debug`; `watch_said` selects `Status::message` only (`src/k8s.rs:1361`) | no formatter over a kube error |
