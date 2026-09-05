//! Phase 8's TUI spike — **throwaway code, deleted at Phase 12** (NOTES § D238).
//!
//! It exists to answer one question the product cannot be written without: what does the
//! event loop look like when a `kube` watch and the terminal's keyboard have to be waited on
//! together. Everything else is deliberately dumb — no theme, no severity, no abstraction
//! over a widget, no config. What it demonstrates, and how each claim is driven headlessly
//! with `tmux`, is in the turn's report.
//!
//! Four things it had to settle, kept here because they are what Phase 11 and 12 inherit:
//!
//! 1. **`ratatui::init()` / `restore()`, not `ratatui::run()`.** `run` takes
//!    `FnOnce(&mut DefaultTerminal) -> R` and this loop is `async`, so there is no closure to
//!    hand it. `init`/`restore` is the documented escape hatch for exactly that — "manual
//!    control over terminal lifetime and the event loop structure" — and it installs the same
//!    panic hook, which is the half invariant 8 cares about.
//! 2. **Keys arrive on a plain OS thread, not a `crossterm::event::EventStream`.** The async
//!    stream needs crossterm's `event-stream` feature, `ratatui-crossterm` 0.1.2 has no
//!    passthrough for it, and D238 keeps a second `crossterm` line out of the manifest. The
//!    thread is detached on purpose: a `spawn_blocking` task parked in `event::read()` would
//!    hold runtime shutdown open forever, because a blocking task cannot be cancelled.
//! 3. **One draw per event, nothing on a timer** — invariant 7's shape, and it is not the
//!    whole of it.
//! 4. **Both `select!` arms are cancel-safe, and that is load-bearing rather than lucky.**
//!    `tokio::select!` drops the losing future on every iteration, so an arm that buffers
//!    internally loses whatever it had. `UnboundedReceiver::recv` and `StreamExt::next` over
//!    a `BoxStream` are both documented cancel-safe; a `read_line`-shaped arm would silently
//!    eat a keystroke per frame.
//!
//! ponytail: **no backoff, and that is the ceiling — not the frame count.** A refused API
//! server makes `watcher()` retry at socket speed and this loop redraw once per attempt:
//! measured against a stub answering 403, **407 requests in 20 s and still climbing at the
//! same rate**, indefinitely. A wrong kubeconfig or an expired credential is the first thing
//! a beginner hits, so this is not an exotic path. The upgrade is a `StandingBackoff`-shaped
//! wrapper around the stream in `main`, which `src/k8s.rs` already has and this file cannot
//! reach (NOTES § D238 ruling 1).
//!
//! Second, smaller: no coalescing. Every watch event redraws, so the initial LIST of the
//! 41-pod `kind-k8rs` cluster is **43** frames, measured off the header counter. Invariant 7's
//! "coalesce ~100ms during storms" is Phase 12's, and the shape is a `tokio::time::sleep`
//! deadline that keeps draining `select!` until it fires.

use std::collections::BTreeMap;
use std::error::Error;

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use k8s_openapi::api::core::v1::Pod;
use kube::config::{Config, KubeConfigOptions, KubeconfigError};
use kube::runtime::watcher;
use kube::{Api, Client};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::widgets::{Block, Clear, List, ListState, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc::UnboundedReceiver;

/// **`is_control()` is the first clause of a two-clause predicate, and the second is the one
/// that took a decision to arrive at.** This is `src/k8s.rs`'s `unprintable()`, copied
/// character for character because the spike cannot call it (NOTES § D238 ruling 1).
///
/// The first draft of this file had the control half only, and a review measured Trojan Source
/// live on a path this file actually has: a `Status.message` carrying U+202E rendered
/// `prod-png` as `gnp-doprd` on screen. U+2028/U+2029 and U+00A0 are deliberately *kept* —
/// they print as a glyph or as a visible space, and removing them would change text the
/// cluster meant to send.
fn unprintable(character: char) -> bool {
    character.is_control()
        || matches!(character,
            '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

/// Invariant 9 — every free-text field from the API is untrusted, so it is stripped and bounded
/// *before* it can reach a cell. A pod named with an escape sequence rewrites the user's
/// terminal; a 50MB annotation blows up the renderer.
///
/// **An unprintable character that is whitespace becomes one space; every other one is
/// removed.** Deleting a newline glues two words into one — the same review measured
/// `forbidden:namespace` where the server had sent two lines — so the boundary survives even
/// though the character does not. `char::is_whitespace` decides which, so the split is the
/// standard library's and not a list kept here.
///
/// **Two things it does more simply than `src/k8s.rs`'s `text()`, said so nobody reads this as
/// a copy of it**: the cap counts characters rather than bytes, and the marker is `…` rather
/// than the product's `SHORTENED` count. What it does *not* do differently is truncate in
/// silence — a cut with no marker reads as a value the cluster actually sent.
fn clean(text: &str) -> String {
    let mut kept = String::new();
    let mut break_pending = false;
    for character in text.chars() {
        if unprintable(character) {
            break_pending |= character.is_whitespace();
        } else {
            // Only between two characters that were kept, and only where there is not one
            // already: a run of breaks is one boundary, and a leading or trailing one is gone.
            if break_pending && !kept.is_empty() && !kept.ends_with(' ') {
                kept.push(' ');
            }
            break_pending = false;
            kept.push(character);
        }
    }
    if kept.chars().count() > 120 {
        kept = kept.chars().take(119).collect::<String>() + "…";
    }
    kept
}

/// Why the kubeconfig could not be used, in a sentence **this file** wrote.
///
/// **The return type is `&'static str`, and that is the entire guard.** It is structurally
/// unable to carry a byte of the error, so there is no sanitiser here to review and no way for
/// a later edit to widen it by accident — the same property `src/k8s.rs` gets from mapping
/// this error to a `Fault` that holds no string (NOTES § D239).
///
/// **What made that necessary rather than tidy**: kube 4.2.0 parses the kubeconfig with
/// `serde-saphyr`, and its `Parse` variant carries a `CroppedRegion` — 64 characters of the
/// *input* either side of the syntax error, not a line number. The neighbourhood of a syntax
/// error in a kubeconfig is `token:`, `client-key-data:`, `password:`. `main` used to hand this
/// error to `Box<dyn Error>`, which `Debug`-prints it, and a canary token measured for this
/// turn came out verbatim on stderr.
///
/// **Every arm is a literal and none of them formats the error**, including the catch-all:
/// `KubeconfigError` is not `#[non_exhaustive]`, so an exhaustive match would compile today and
/// break on a kube bump — a `_` that is also a fixed sentence is safe in both directions.
///
/// **Six arms, counted off this function and not recalled** —
/// `awk '/^fn because/,/^}/' examples/spike_tui.rs | grep -c '=>'` prints `6`, and it is
/// written without line numbers so it still answers after this file moves. The first draft of
/// this comment claimed six when
/// there were five, which is a wrong number wearing a measurement's clothes and is the one
/// thing a file whose product is knowledge cannot do. Each was reached and printed its own
/// line; the canary one of them carried was the *context name* rather than a token, so an arm
/// that formatted its payload would have said so.
///
/// **`src/k8s.rs` groups the same fifteen variants into three**, because a `Fault` has three
/// places to send a reader and every extra group needs a fourth. A `&'static str` has as many
/// as are written, so the cluster-entry and certificate arms stay apart here where the product
/// merges both into `Fault::BadEntry`.
fn because(error: &KubeconfigError) -> &'static str {
    match error {
        KubeconfigError::FindPath | KubeconfigError::ReadConfig(..) => {
            "the kubeconfig file is missing, or cannot be read"
        }
        KubeconfigError::Parse(..) => "the kubeconfig is not valid YAML",
        KubeconfigError::CurrentContextNotSet | KubeconfigError::LoadContext(..) => {
            "the kubeconfig is valid YAML but does not name a context that is in it"
        }
        // **These four are not a context problem, and saying so sends the reader to the wrong
        // file** (`src/k8s.rs`'s `kubeconfig_fault`): for `LoadClusterOfContext` the context
        // *was* found and the `clusters:` entry it names is missing. Told to check
        // `current-context:` and `--context`, the reader checks a line that is correct.
        KubeconfigError::LoadClusterOfContext(..)
        | KubeconfigError::MissingClusterUrl
        | KubeconfigError::ParseClusterUrl(..)
        | KubeconfigError::ParseProxyUrl(..) => {
            "this kubeconfig loaded, and a cluster one of its contexts points at did not — \
             a missing `clusters:` entry, or a `server:` line that is not a URL"
        }
        KubeconfigError::LoadCertificateAuthority(..)
        | KubeconfigError::LoadClientCertificate(..)
        | KubeconfigError::LoadClientKey(..)
        | KubeconfigError::ParseCertificates(..) => {
            "the certificate or key the kubeconfig points at could not be loaded"
        }
        _ => "the kubeconfig could not be used",
    }
}

/// Say which of the things went wrong and stop. Nothing is open yet — both callers run before
/// `ratatui::init()` — so skipping destructors costs nothing.
fn refuse(why: &'static str) -> ! {
    eprintln!("spike: no cluster to watch — {why}");
    std::process::exit(2);
}

/// `"ns/name"` and the status text beside it.
///
/// Fields are read straight off `metadata` and `status` because that is the shorter way to
/// write it, and for **no** other reason. The first draft claimed this dodged the write
/// allowlist's `Api::namespace` ban; it does not, because `clippy.toml` bans a def-id on
/// `Api<K>` and `ResourceExt::namespace` is a different one on the object. `ResourceExt` is
/// free to use in Phase 11 — the reasoned claim here was wrong.
///
/// **Restarts count init containers too** (`src/rules.rs` reads both). A pod crashlooping in
/// an init container reported `restarts=0` until this was fixed.
fn row(pod: &Pod) -> (String, String) {
    let namespace = clean(pod.metadata.namespace.as_deref().unwrap_or("-"));
    let name = clean(pod.metadata.name.as_deref().unwrap_or("-"));
    let status = pod.status.as_ref();
    let phase = clean(status.and_then(|s| s.phase.as_deref()).unwrap_or("?"));
    let restarts: i32 = status
        .map(|s| {
            [&s.container_statuses, &s.init_container_statuses]
                .into_iter()
                .flatten()
                .flatten()
                .map(|c| c.restart_count)
                .sum()
        })
        .unwrap_or(0);
    (
        format!("{namespace}/{name}"),
        format!("{phase} restarts={restarts}"),
    )
}

#[derive(Default)]
struct App {
    rows: BTreeMap<String, String>,
    /// What `Init` … `InitDone` collects before it is published, mirroring `src/k8s.rs`'s
    /// `filling`. `None` outside a relist.
    filling: Option<BTreeMap<String, String>>,
    /// **Not a row.** The first draft inserted the watch error into `rows` under the key
    /// `"! watch"`, and `!` is 0x21: it sorted to the top, held the cursor, was counted as a
    /// pod, and stayed on screen after the watch recovered. A failure is a property of the
    /// watch, so it lives beside the list and not in it.
    failure: Option<String>,
    /// **The fifth line of `src/k8s.rs`'s `InitDone`, which the first draft copied four of.**
    /// `false` until a LIST has finished, and it is the whole of `PRIOR-ART § C2` — *"empty"
    /// and "not loaded yet" are different screens*. `rows` stays empty until `InitDone`, which
    /// on `kind-k8rs` is watch event **43 of 43**, so without this flag every frame before it
    /// drew an empty bordered box for a cluster with 41 pods — milliseconds on local kind,
    /// seconds on 5000 pods over a WAN, and indistinguishable from the true empty answer the
    /// whole time. C2 wants three states: this is two, and *denied* is `failure` above.
    ///
    /// **Never reset, exactly as `src/k8s.rs` never resets it.** A relist leaves the previous
    /// answer in `rows` while it runs, so flipping back to *still listing* would call a screen
    /// full of rows unloaded. The one place that reads oddly is a cluster that listed empty and
    /// is now relisting: it says *no pods* rather than *listing*. The product does the same,
    /// and "the last complete answer" is what the word means.
    complete: bool,
    /// Proof the watch is live even in the minute nothing in the cluster happens to change.
    events: u64,
    cursor: usize,
    /// `Some(choice)` while the modal is open — the whole of "keyboard focus".
    dialog: Option<usize>,
}

impl App {
    /// Returns `true` to quit.
    ///
    /// **This is box 3.** The dialog arm returns before the list arm is reached, so while the
    /// modal is open *every* key belongs to it — including `q`, which is why the list cannot
    /// be quit out from under a confirmation, and including `up`/`down`, which move the
    /// dialog's choice and leave the list's cursor exactly where it was.
    ///
    /// **That is true of the keyboard and it is not true of the screen, which is the half
    /// Phase 8 found** (`PRIOR-ART § G1`). No key can retarget the modal; a *watch event* can.
    /// `draw` re-resolves `rows.keys().nth(cursor)` every frame, so a pod arriving ahead of the
    /// cursor slides a different name under an open confirmation — reproduced with an
    /// unrelated `Apply`, `cursor=1` naming `prod/c-app` and then `prod/b-app`. **Left
    /// deliberately**: invariant 2's typed name is what it is compared *against*, and Phase 11
    /// is where the dialog captures its subject at open. The spike demonstrating the defect is
    /// worth more than the spike quietly not having it.
    fn key(&mut self, code: KeyCode) -> bool {
        if let Some(choice) = self.dialog.as_mut() {
            match code {
                KeyCode::Up => *choice = choice.saturating_sub(1),
                KeyCode::Down => *choice = (*choice + 1).min(1),
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Enter => self.dialog = None,
                _ => {}
            }
            return false;
        }
        match code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('p') => {
                panic!("spike: deliberate panic, to prove the terminal comes back")
            }
            KeyCode::Char('d') => self.dialog = Some(0),
            KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Down => self.cursor = (self.cursor + 1).min(self.rows.len().saturating_sub(1)),
            _ => {}
        }
        false
    }

    /// The spike's whole watch→state step. The product boxes this as
    /// `k8s.rs`'s `Update = Box<dyn FnOnce(&mut Store)>` because five streams of three
    /// different types are merged there and the driver must name none of them; one stream of
    /// one type needs no closure, which is the difference worth having seen.
    fn watched(&mut self, event: watcher::Event<Pod>) {
        self.events += 1;
        // **Not on `Init`/`InitApply`**, which is the whole of the guard. kube emits
        // `Ok(Init)` *before* every failed request, so clearing there would wipe the error a
        // few milliseconds after it was shown and make a permanently refused watch look
        // healthy. `InitDone` only counts if a list was actually collected — a stray one on a
        // broken stream answered nothing. This is `src/k8s.rs`'s `answered`, unchanged.
        let answered = match &event {
            watcher::Event::Init | watcher::Event::InitApply(_) => false,
            watcher::Event::InitDone => self.filling.is_some(),
            _ => true,
        };
        if answered {
            self.failure = None;
        }
        match event {
            watcher::Event::Apply(pod) => {
                let (key, status) = row(&pod);
                self.rows.insert(key, status);
            }
            // **`Init`, the `InitApply`s, then `InitDone` is a *relist*, and ignoring that
            // sequence is a correctness bug, not a cosmetic one.** kube's own doc: "Any
            // objects that
            // were previously `Applied` but are not listed in any of the `InitApply` events
            // should be assumed to have been `Deleted`." So after a `410 Expired`, a pod that
            // died while the watch was broken never gets a `Delete` — it simply stops being
            // listed, and a list that only ever inserts keeps showing it forever. Measured:
            // a stub that lists alpha+beta, expires, then re-lists alpha alone left `beta` on
            // screen for the life of the process. Buffer, then swap.
            watcher::Event::Init => self.filling = Some(BTreeMap::new()),
            watcher::Event::InitApply(pod) => {
                let (key, status) = row(&pod);
                self.filling.get_or_insert_default().insert(key, status);
            }
            // A `take()` and not a clone: `filling` being `None` publishes nothing rather than
            // publishing an empty cluster, which is what a broken stream would otherwise say.
            watcher::Event::InitDone => {
                if let Some(listed) = self.filling.take() {
                    self.rows = listed;
                    self.complete = true;
                    self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
                }
            }
            watcher::Event::Delete(pod) => {
                self.rows.remove(&row(&pod).0);
                // The list can shrink under a cursor sitting at the end, and nothing else
                // clamps it: `key`'s `Down` arm does, but `Up` from an out-of-range value
                // stays out of range. Clamping where the invalidation happens is the only
                // place it cannot be forgotten.
                self.cursor = self.cursor.min(self.rows.len().saturating_sub(1));
            }
        }
    }
}

fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let [wide] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [box_] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(wide);
    box_
}

fn draw(frame: &mut Frame, app: &App) {
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    frame.render_widget(
        Paragraph::new(match &app.failure {
            Some(why) => format!(" watch is broken: {why}"),
            None => format!(
                " {} pods · {} watch events · cursor {}",
                app.rows.len(),
                app.events,
                app.cursor
            ),
        }),
        head,
    );

    // **An empty box has to say *which* empty it is** (`PRIOR-ART § C2`, finding 1 above).
    // `still listing` and `no pods` are different answers and a bordered box with nothing in it
    // is the first one wearing the second one's clothes.
    let items: Vec<String> = if app.rows.is_empty() {
        let state = if app.complete {
            "this cluster has no pods"
        } else {
            "still listing the cluster…"
        };
        vec![format!("  {state}")]
    } else {
        app.rows
            .iter()
            .map(|(key, status)| format!("{key}  {status}"))
            .collect()
    };
    // The selection is marked with a symbol and not a colour, because `tmux capture-pane -p`
    // captures text and drops the attributes — the spike has to be legible in its own proof.
    // No selection over the placeholder: a state is not a row and must not look pickable.
    let selected = (!app.rows.is_empty()).then_some(app.cursor);
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::bordered().title(" pods "))
            .highlight_symbol("> "),
        body,
        &mut state,
    );

    frame.render_widget(
        Paragraph::new(" up/down move · d dialog · p panic · q quit"),
        foot,
    );

    if let Some(choice) = app.dialog {
        let selected = app.rows.keys().nth(app.cursor).map_or("-", String::as_str);
        // **Centred on `body` and clamped to it, not on the whole frame.** 9 is two border
        // rows plus seven lines: at 8 the modal silently clipped its own key hint. But a fixed
        // 62x9 over `frame.area()` swallowed the header and the key hints on any terminal that
        // small, so the size it asks for is the size that is there.
        let area = centred(body, 62.min(body.width), 9.min(body.height));
        // `Clear` first, or the list shows through the modal's own cells.
        frame.render_widget(Clear, area);
        let mark = |n: usize| if n == choice { ">" } else { " " };
        frame.render_widget(
            Paragraph::new(format!(
                "Pretend this deletes:\n  {selected}\n\n{} Yes, do it\n{} No, leave it alone\n\nup/down choose · enter picks · esc or n dismisses",
                mark(0),
                mark(1),
            ))
            .block(Block::bordered().title(" Confirm ")),
            area,
        );
    }
}

/// **The only error that may reach this `Box<dyn Error>` is `terminal.draw`'s `io::Error`** —
/// `main` `Debug`-prints whatever comes back, which is the mechanism [`because`] exists to
/// close. Anything config-shaped is refused before this function is called; do not widen it.
async fn run(
    terminal: &mut DefaultTerminal,
    mut pods: BoxStream<'static, Result<watcher::Event<Pod>, watcher::Error>>,
    mut keys: UnboundedReceiver<Event>,
) -> Result<(), Box<dyn Error>> {
    let mut app = App::default();
    loop {
        terminal.draw(|frame| draw(frame, &app))?;
        tokio::select! {
            key = keys.recv() => match key {
                Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    if app.key(k.code) {
                        return Ok(());
                    }
                }
                // Resize needs no arm: the loop redraws on *any* event, and ratatui re-reads
                // the backend's real size inside `draw` rather than trusting the event.
                Some(_) => {}
                None => return Ok(()),
            },
            watched = pods.next() => match watched {
                Some(Ok(event)) => app.watched(event),
                // **A watch failure is a state, not an exit — and this spike retries it in a
                // hot loop, which the product must not.** The first draft said "`watcher()`
                // reconnects underneath" as if that were the end of it. It reconnects
                // *immediately and forever*: `kube-runtime`'s `watcher()` is a bare `unfold`
                // and its own doc says "To avoid constantly looping errors, make sure backoff
                // is applied" (`watcher.rs:26`). Measured here, against a stub that answers
                // 403 to everything and counts what arrives: **407 requests in 20 s — 68 by
                // t=5s, 180 by t=10s, 294 by t=15s — ~20 a second, flat and not growing**,
                // for as long as the process lives. That is the security gate's *never
                // retries in a loop*, broken, and a wrong kubeconfig is how a beginner
                // reaches it.
                //
                // **The caller owes the policy, and `.default_backoff()` is not it.**
                // `StreamBackoff` resets on every non-error item and a refused `watcher()`
                // emits `Ok(Init)` before every `Err`, so the exponential never leaves its
                // first step. `src/k8s.rs`'s `StandingBackoff` is that reset silenced, and it
                // is unreachable from here (NOTES § D238 ruling 1) — so this stays a hot loop
                // and says so, rather than pretending a fix it cannot make.
                Some(Err(err)) => app.failure = Some(clean(&err.to_string())),
                None => return Ok(()),
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Connect *before* the terminal is taken over, so a missing kubeconfig or a dead API
    // server prints on a normal screen instead of behind the alternate one.
    //
    // **`Config::from_kubeconfig` and not `Client::try_default`**, which is the one thing
    // `scripts/security-guard.py` found in this file: `try_default` (and `Config::infer`
    // under it) tries the in-cluster ServiceAccount environment *first*, and the security
    // gate's *credentials come from the kubeconfig current context and nowhere else* means
    // there is no such path to open — not even in throwaway code.
    // **Neither of these may be handed to `?`.** `main` returns `Box<dyn Error>`, which is
    // `Debug`-printed on the way out, and a `KubeconfigError::Parse` quotes the user's own file
    // back at them — see [`because`]. What the reader still learns is which of the things broke;
    // what they never learn is a line of their kubeconfig.
    let config = match Config::from_kubeconfig(&KubeConfigOptions::default()).await {
        Ok(config) => config,
        Err(error) => refuse(because(&error)),
    };
    // **One sentence and no classification, unlike the above.** By here the kubeconfig has
    // parsed, so what is left is TLS material or an auth mode this build does not carry — and
    // the reader's next move is the same for all of them. `kube::Error` is also the type whose
    // `Debug` reaches furthest into the config, so it is the last one to start formatting.
    let client = match Client::try_from(config) {
        Ok(client) => client,
        Err(_) => refuse("the kubeconfig was read, but no client could be built from it"),
    };
    let pods = watcher(Api::<Pod>::all(client), watcher::Config::default()).boxed();

    let mut terminal = ratatui::init();

    // **After `init()`, not before.** `event::read()` starts reading stdin the moment the
    // thread runs, and until `init()` has enabled raw mode that stdin is still line-buffered
    // — a key struck in that window is held by the tty until Enter. The first draft spawned
    // this above `init()` and the window was real, if small.
    let (tx, keys) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if tx.send(event).is_err() {
                break;
            }
        }
    });

    let result = run(&mut terminal, pods, keys).await;
    ratatui::restore();
    result
}
