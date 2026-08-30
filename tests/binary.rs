//! The three things `main` does that no unit test in `src/main_tests.rs` can reach: argv, the
//! choice of stream, and the exit code.
//!
//! It exists because `cargo mutants` replaced `main`'s whole body with `()` and every unit test
//! stayed green — and because the one defect that shipped in this box lived in exactly that gap:
//! the usage text printed as one run-on line, on a stream nothing asserted about
//! (CLAUDE.md § Step 4 is the anti-leak mechanism).
//!
//! **This is not the lib target [NOTES § D50](../NOTES.md) refused**, and does not open the door
//! to one: it links no product type, reaches nothing private and calls no function in `src/`. It
//! runs the built binary and reads its two streams, which is the use D50 reserves `tests/` for.
//! Phase 7's end-to-end box is a different thing and is untouched.

use std::process::{Command, Output, Stdio};

/// Run the built binary with these arguments. `CARGO_BIN_EXE_k8rs` is set by cargo only for a
/// target under `tests/`, and is the whole reason this file is not a unit test.
///
/// **An argument vector, never a command string.** A path is untrusted text and a pod name will
/// be, so nothing here is allowed to become shell syntax (CLAUDE.md § Untrusted input;
/// `scripts/security-guard.py` § no shell is spawned, whose own self-test draws the line here).
fn k8rs(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .args(args)
        .output()
        .expect("the built binary runs")
}

/// The same, with `KUBECONFIG` pointed at a path that cannot be a kubeconfig.
///
/// **The override is load-bearing, not tidiness.** Inherited, `--live` connects to whatever
/// cluster the developer's `KUBECONFIG` names and watches it until the harness gives up — the
/// watch never ends by design (`src/k8s.rs` § THE DRIVER), so the test would not fail, it would
/// hang. Nothing here reaches a network: the path does not exist, so no client is ever built.
fn k8rs_with_no_kubeconfig(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .args(args)
        .env(
            "KUBECONFIG",
            "/nonexistent/k8rs-tests/there-is-no-kubeconfig-here",
        )
        .output()
        .expect("the built binary runs")
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn text(stream: Vec<u8>) -> String {
    String::from_utf8(stream).expect("k8rs writes UTF-8")
}

/// **No arguments: exit 2, the usage on stderr, and still three lines when it gets there.**
///
/// `'\n'.is_control()` is `true`, so a strip run over the assembled message instead of over the
/// values that entered it eats k8rs's own line breaks — the three sentences print as one run-on
/// line, and every unit test over `run`'s `Err` string stayed green while it did. Nothing goes to
/// stdout, so `k8rs > findings.txt` on a mistyped command leaves an empty file rather than a
/// usage text pretending to be a report (`screens/once.md` § stdout and stderr are split).
#[test]
fn no_arguments_is_the_usage_on_stderr_in_three_lines_and_exit_2() {
    let out = k8rs(&[]);

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{:?}", text(out.stdout.clone()));
    let stderr = text(out.stderr);
    assert_eq!(stderr.lines().count(), 3, "{stderr:?}");
    assert!(stderr.starts_with("usage: k8rs "), "{stderr:?}");
    assert!(stderr.contains("cannot reach a cluster"), "{stderr:?}");
    // **Counting the lines does not read them.** The synopsis is the only place a reader
    // learns which modes this build has, and the whole `--live` form was removable from it
    // with all seven of these green until this loop existed. It is asserted against the
    // first line and not the whole text because the prose below still says `--live` while
    // the synopsis offers no way to reach it.
    let synopsis = stderr.lines().next().expect("the usage has a first line");
    // `--namespace` joined the list when the scoping box landed. It is here for the reason the
    // three beside it are: the synopsis is the only place a reader learns the flag exists, and
    // a flag that scopes what the tool reads is the one a reader most needs offered.
    for named in ["--analysis", "--live", "--context", "--namespace"] {
        assert!(
            synopsis.contains(named),
            "the synopsis does not offer {named}, so nothing tells a reader how to reach \
             it: {synopsis:?}"
        );
    }
}

/// **The sentence a run that got as far as connecting prints**, and the canary every
/// *refused before anything was dialled* assertion in this file rests on
/// ([`a_namespace_that_names_nothing_usable_is_refused_before_anything_connects`]).
///
/// **It is a `const` so the two halves cannot drift apart.** The negative assertions are only
/// worth anything while this text is what a connect failure actually says; reworded in
/// `src/main.rs` and spelled out by hand here, every one of them passes over a binary that
/// dialled the cluster first. Named once, and proved producible in the same test that relies on
/// its absence.
const CONNECT_CANARY: &str = "no cluster to watch";

/// **Neither cluster mode can start without a kubeconfig: exit 2, stderr, and an empty stdout.**
///
/// The unit test over `live` can assert only the sentence it returns — "stdout belongs to the
/// process and a test cannot read it back" (`src/main_tests.rs` § WATCHING A CLUSTER). This is
/// that half, and it is the half `screens/once.md` § stdout and stderr are split is about:
/// `k8rs --live > findings.txt` against a kubeconfig that is not there leaves an **empty** file,
/// not a diagnostic sitting where a report should be.
///
/// **The wording is deliberately not pinned.** Telling `403` from `401` from *nothing answered*
/// is the next box of Phase 5 and it will rewrite this sentence; what may not change is the
/// stream, the exit code and the empty stdout.
#[test]
fn a_cluster_mode_with_no_kubeconfig_is_exit_2_on_stderr_and_leaves_stdout_empty() {
    // **`--once` is asserted beside `--live` because only one of the two has an exit code to
    // get wrong.** `--live` returns a sentence and `main` exits 2 whatever the sentence says;
    // `--once` returns `Option`, and the mode that can answer *it reported* is the mode that can
    // answer it about a cluster it never reached (`screens/once.md` § Exit codes).
    for mode in ["--live", "--once"] {
        let out = k8rs_with_no_kubeconfig(&[mode]);

        assert_eq!(out.status.code(), Some(2), "{mode}: {out:?}");
        assert!(
            out.stdout.is_empty(),
            "{mode} wrote a diagnostic where a report goes: {:?}",
            text(out.stdout.clone())
        );
        let stderr = text(out.stderr);
        assert!(stderr.starts_with("k8rs: "), "{mode}: {stderr:?}");
    }
}

/// **A committed capture: exit 0, the report on stdout, and stderr empty.**
///
/// `healthy.json` is the one fixture whose report does not move with the clock — every other one
/// carries an age and the binary reads the real one. Findings do not change the exit code
/// (NOTES § D17), and this is the arm that proves `0` is reachable at all.
///
/// **The whole report is one literal, header included**, so a count that goes *wrong* reddens
/// this as loudly as one that goes away — which is how the workload count leaving the header
/// was caught (NOTES § D151). The header is `screens/once.md` § When nothing is broken's own
/// line — `prod-eu · 84 pods · 3 nodes` — minus the cluster name a driver that reads files
/// cannot know, and with **no third noun**: `workload` is said once in this product, and it is
/// said on Capacity's row.
#[test]
fn a_healthy_capture_is_the_report_on_stdout_and_exit_0() {
    let out = k8rs(&[&fixture("healthy.json")]);

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert!(out.stderr.is_empty(), "{:?}", text(out.stderr.clone()));
    assert_eq!(text(out.stdout), "1 pod · 0 nodes\n\n○ nothing is broken\n");
}

/// A path that is not there is exit 2 and a sentence naming the file, on stderr and with k8rs's
/// own name on it — never a panic, and never a report (NOTES § D17).
#[test]
fn a_path_that_does_not_exist_is_exit_2_and_names_the_file_on_stderr() {
    let missing = fixture("no-such-fixture.json");

    let out = k8rs(&[&missing]);

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{:?}", text(out.stdout.clone()));
    let stderr = text(out.stderr);
    assert!(
        stderr.starts_with("k8rs: ") && stderr.contains(&missing),
        "{stderr:?}"
    );
}

/// **Invariant 9 at the process boundary, which is the only boundary that reaches a terminal.**
///
/// The strip is proven inside `render` and `load`; nothing until now watched what actually left
/// the process. argv is as untrusted as the API — a shell glob expands whatever the directory is
/// named — and a file does not have to exist for its name to be printed.
///
/// Both halves: nothing controlling survives except the newline `eprintln!` itself adds, **and**
/// the readable part of the name still does. A `sanitize` that returned nothing would pass the
/// first assertion and leave the user an error naming no file at all (CLAUDE.md § A derived list
/// asserts it found something).
#[test]
fn a_crafted_path_leaves_the_process_with_no_control_character_on_stderr() {
    // `ESC`, `CR` and a C1 control — three framings of the same class, in the middle of a value
    // rather than as the whole of one (NOTES § D31).
    let crafted = fixture("no-such\x1b[2J\r\u{9b}fixture.json");

    let out = k8rs(&[&crafted]);

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = text(out.stderr);
    let line = stderr
        .strip_suffix('\n')
        .expect("eprintln! ends what it writes with a newline");
    let survivors: Vec<char> = line.chars().filter(|c| c.is_control()).collect();
    assert!(
        survivors.is_empty(),
        "control characters left the process: {survivors:?}\n{stderr:?}"
    );
    assert!(
        line.contains("no-such[2Jfixture.json"),
        "the path was stripped away along with the escape: {stderr:?}"
    );
}

/// **A `--namespace` that names nothing usable is refused before anything is connected to.**
///
/// **The order is the whole assertion, and only a process can show it.** `mistyped` runs before
/// the mode is chosen, so a bad namespace has to cost a sentence and not a round trip — and the
/// unit tests in `src/main_tests.rs` call `mistyped` directly, which cannot tell *refused first*
/// from *refused after a connection was attempted*. `KUBECONFIG` here points at a path that
/// cannot be one, so a run that reached the connect would say *no cluster to watch*: that
/// sentence is the canary, and its absence is what proves nothing was dialled.
///
/// **All three shapes the flag can be given nothing usable in**, and both spellings of each,
/// because the flag has two and a refusal that only covers the long one lets the short one
/// through into a URL (`k8s::path_safe`, the security gate's *names build paths* row):
/// the word alone at the end of the line, `=` with nothing after it, and a value that is not a
/// namespace name.
///
/// **A missing value is refused rather than ignored, and that is the half worth a process
/// test.** `k8rs --live -n "$NS"` with `NS` unset is the commonest way here; swallowing it would
/// watch **every** namespace, which is the opposite of what the reader asked for and has no line
/// on screen to notice it by.
#[test]
fn a_namespace_that_names_nothing_usable_is_refused_before_anything_connects() {
    // **The canary is proved producible first, or the negative below is vacuous.** Every
    // assertion in the loop rests on *this exact sentence* being what a run that reached the
    // connect says; reword it and the `!contains` goes silently true for every case at once,
    // which is the shape `write-guard.py`'s `CANARIES` exists to refuse (CLAUDE.md § A derived
    // list asserts it found something). `--once` is asserted beside `--live` because the two
    // share one `live()` and one sentence, and a mode that grew a second one would show here.
    for mode in ["--live", "--once"] {
        let reached = text(k8rs_with_no_kubeconfig(&[mode]).stderr);
        assert!(
            reached.contains(CONNECT_CANARY),
            "{mode} that reached the connect no longer says {CONNECT_CANARY:?}, so every \
             `!contains` below proves nothing: {reached:?}"
        );
    }

    for args in [
        vec!["--live", "--namespace"],
        vec!["--live", "-n"],
        vec!["--live", "--namespace="],
        vec!["--live", "-n="],
        vec!["--live", "--namespace", "../secrets"],
        vec!["--live", "-n", "../secrets"],
        vec!["--live", "--namespace=a/b"],
        vec!["--live", "-n=.."],
        // **`--once` goes through the same gate, and only a process can show it.** The unit
        // tests call `mistyped` directly, which cannot tell *refused first* from *refused after
        // a connection was attempted* — and `--once` is the mode whose whole promise is that it
        // ends, so a run that dialled first would still exit 2 and look identical here.
        vec!["--once", "--namespace"],
        vec!["--once", "-n", "../secrets"],
    ] {
        let out = k8rs_with_no_kubeconfig(&args);

        assert_eq!(out.status.code(), Some(2), "{args:?} {out:?}");
        assert!(
            out.stdout.is_empty(),
            "{args:?} wrote a report: {:?}",
            text(out.stdout.clone())
        );
        let stderr = text(out.stderr);
        assert!(
            stderr.starts_with("k8rs: --namespace needs the name of a namespace"),
            "{args:?} was not refused for the namespace: {stderr:?}"
        );
        assert!(
            stderr.contains("usage: k8rs "),
            "{args:?} was refused with no way to see the right spelling: {stderr:?}"
        );
        // The canary. `KUBECONFIG` cannot be read, so any run that got as far as connecting
        // says so — and this refusal has to happen before that.
        assert!(
            !stderr.contains(CONNECT_CANARY),
            "{args:?} reached the connect before the namespace was checked: {stderr:?}"
        );
    }
}

/// **Invariant 9 at the process boundary for the newest thing argv can put on screen.**
///
/// A namespace is echoed back when it is not a name — and *not a name* is exactly the class a
/// control character lands in, so this sentence is a terminal sink for a value nobody has
/// stripped before. `sanitize` runs at the interpolation; nothing until now watched what left
/// the process through it.
///
/// **Both halves, for the reason the path test beside it gives**: nothing controlling survives,
/// **and** the readable part of the value still does. A `sanitize` that returned nothing would
/// pass the first assertion and leave the reader a sentence naming no namespace at all
/// (CLAUDE.md § A derived list asserts it found something).
#[test]
fn a_crafted_namespace_leaves_the_process_with_no_control_character_on_stderr() {
    // `ESC`, `CR` and a C1 control — three framings of the class, inside the value rather than
    // as the whole of one (NOTES § D31). `/` keeps it out of `path_safe` whatever the strip
    // does, so the arm under test is reached for the same reason on every platform.
    let crafted = "pay\u{1b}[2J\rments\u{9b}/x";

    let out = k8rs_with_no_kubeconfig(&["--live", "--namespace", crafted]);

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = text(out.stderr);
    let first = stderr.lines().next().expect("the refusal has a first line");
    let survivors: Vec<char> = first.chars().filter(|c| c.is_control()).collect();
    assert!(
        survivors.is_empty(),
        "control characters left the process: {survivors:?}\n{stderr:?}"
    );
    assert!(
        first.contains("pay[2Jments/x"),
        "the namespace was stripped away along with the escape: {stderr:?}"
    );
}

// --- THE EXIT CODE OF A FAILED WRITE START ---

/// The largest pipe a target in the release matrix can hand us: Linux sizes a pipe at 16
/// buffers of one page, and the biggest page x86_64 or aarch64 ships is 16 KiB — 64 KiB on the
/// 4 KiB kernels CI and this repo's hosts run. **It is a ceiling, not a measurement**, because
/// `F_GETPIPE_SZ` needs `libc` and that is not one of the ten (invariant 10). Reading the real
/// number is the upgrade if a kernel with 64 KiB pages ever matters.
const PIPE_CAPACITY: usize = 16 * 16 * 1024;

/// How many times the capture set is handed to one invocation. Sized off [`PIPE_CAPACITY`] and
/// asserted at the call site, never assumed.
const PASSES: usize = 30;

/// Every committed capture, [`PASSES`] times over — one argv whose report cannot fit in a pipe.
///
/// Read off the directory rather than transcribed: a capture added or dropped by
/// `just fixtures` moves the size with it, and the caller asserts what the size has to be.
fn every_capture_over_and_over() -> Vec<String> {
    let dir = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut paths: Vec<String> = std::fs::read_dir(&dir)
        .expect("the fixture directory is there")
        .map(|entry| entry.expect("a fixture directory entry reads").path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| path.to_str().expect("a fixture path is UTF-8").to_string())
        .collect();
    assert!(!paths.is_empty(), "{dir} holds no .json capture");
    // Sorted, so the report is the same bytes twice for the same fixture set.
    paths.sort();
    // The report is per object read, so N passes over the same argv is N times the report.
    let mut many = paths.clone();
    for _ in 1..PASSES {
        many.extend_from_slice(&paths);
    }
    many
}

/// **A reader that closed the pipe costs nothing: exit 0, silent, no panic.**
///
/// `head -1`, or `less` quit on the first page — which `screens/once.md` § Colour and symbols
/// sells as the way to read this report. Rust masks `SIGPIPE`, so `println!` *panicked* here:
/// exit 101 and a backtrace, a code NOTES § D17's table does not have. [`stdout_failure`]'s own
/// arms are a unit test in `src/main_tests.rs`; **the exit code as an exit code is only
/// observable from here**, and it was the half that was wrong.
///
/// **The size premise is asserted, not assumed.** The read end is dropped straight after
/// `spawn`, but a report that fits in the pipe's buffer is a report the child finishes writing
/// before anyone can close anything — no write ever fails, and this test then goes green over a
/// binary that still panics. That is the whole reason [`PIPE_CAPACITY`] is the matrix's ceiling
/// rather than the 64 KiB this machine has: the hole it closes is one that opens on somebody
/// else's kernel and stays silent. A fixture set that shrinks below it reddens the assertion
/// instead (CLAUDE.md § A derived list asserts it found something).
#[test]
fn a_reader_that_closed_the_pipe_costs_nothing() {
    let paths = every_capture_over_and_over();
    let args: Vec<&str> = paths.iter().map(String::as_str).collect();

    let whole = k8rs(&args);
    assert_eq!(whole.status.code(), Some(0), "{:?}", text(whole.stderr));
    assert!(
        whole.stdout.len() > PIPE_CAPACITY,
        "the report is {} bytes and a pipe holds {PIPE_CAPACITY}: the write would finish before \
         the reader could go away, and this test would prove nothing",
        whole.stdout.len()
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the built binary runs");
    // The read end, closed while the child still has more to write than the buffer can hold.
    drop(child.stdout.take().expect("stdout was piped"));
    let out = child.wait_with_output().expect("the child ends");

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    // Empty rather than merely free of the words: a panic's backtrace lands here too, and so
    // would a well-meant "broken pipe" line the user did not ask for.
    assert!(out.stderr.is_empty(), "{:?}", text(out.stderr));
}

/// **A write that failed for any other reason is exit 2 and a sentence — never silence.**
///
/// `k8rs > findings.txt` onto a full disk. `/dev/full` is the kernel returning a real `ENOSPC`,
/// so no plumbing invents the error shape (NOTES § D29). Silence here would leave a report cut
/// in half looking like a whole one, which is worse than the panic this replaced.
///
/// The reason half of the sentence is the standard library's own string for errno 28, derived
/// here rather than transcribed; the half k8rs owns is transcribed, because that half *is* the
/// requirement. `StorageFull` is asserted so a run that somehow took [`stdout_failure`]'s
/// `BrokenPipe` arm cannot pass as this one.
#[test]
fn a_write_that_fails_any_other_way_is_exit_2_and_says_why() {
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full is not on this machine, and nothing else is a real ENOSPC");
    let enospc = std::io::Error::from_raw_os_error(28);
    assert_eq!(
        enospc.kind(),
        std::io::ErrorKind::StorageFull,
        "ENOSPC is 28 here, or this test is describing the other arm"
    );

    let out = Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .arg(fixture("healthy.json"))
        .stdout(full)
        .stderr(Stdio::piped())
        .output()
        .expect("the built binary runs");

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(
        text(out.stderr),
        format!("k8rs: the report could not be written — {enospc}\n")
    );
}

// --- THE EXIT CODE OF A FAILED WRITE END ---

/// **Invariant 9 on stdout — the one strip path that reaches a real terminal.**
///
/// The stderr half is [`a_crafted_path_leaves_the_process_with_no_control_character_on_stderr`];
/// this is the report itself, where a pod name off the API is drawn by [`card`]. The crafted
/// object is **derived at run time** from a committed capture into the temp directory, so
/// nothing crafted is ever committed and both [NOTES § D53](../NOTES.md) and
/// `scripts/fixture-audit.sh` keep meaning what they say.
///
/// The name is *split*, not replaced, so a re-capture that renames the pod moves the expected
/// text with it — and the capture losing the field it edits is an `expect` here, not a silent
/// skip. Both halves are asserted: nothing controlling survives, **and** the readable part of
/// the name still does, which a `sanitize` that returned nothing would fail.
#[test]
fn a_crafted_name_leaves_the_process_with_no_control_character_on_stdout() {
    let capture = std::fs::read_to_string(fixture("crashloop.json")).expect("the capture reads");
    let mut doc: serde_json::Value = serde_json::from_str(&capture).expect("the capture is JSON");
    let name = doc["metadata"]["name"]
        .as_str()
        .expect("crashloop.json no longer has the metadata.name this test crafts")
        .to_string();
    assert!(
        name.is_ascii() && name.len() > 1,
        "a name this test can split in the middle: {name:?}"
    );
    let (head, tail) = name.split_at(1);
    // `ESC`, `CR` and a C1 — three framings of the class, inside a value rather than as the
    // whole of one (NOTES § D31). The first two go into the file as JSON escapes; `\u{9b}` is
    // above the range JSON escapes and travels raw.
    doc["metadata"]["name"] = format!("{head}\u{1b}[2J\r\u{9b}{tail}").into();

    let path = std::env::temp_dir().join(format!("k8rs-crafted-{}.json", std::process::id()));
    std::fs::write(&path, doc.to_string()).expect("the crafted capture writes");
    let out = k8rs(&[path.to_str().expect("the temp path is UTF-8")]);
    // Removed before the assertions, so a red run leaves nothing behind either.
    std::fs::remove_file(&path).expect("the crafted capture is removed");

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = text(out.stdout);
    // Per line: k8rs's own `\n` is a control character too, and it is the one thing here that
    // is not from outside.
    let survivors: Vec<char> = stdout
        .lines()
        .flat_map(str::chars)
        .filter(|c| c.is_control())
        .collect();
    assert!(
        survivors.is_empty(),
        "control characters reached stdout: {survivors:?}\n{stdout:?}"
    );
    assert!(
        stdout.contains(&format!("{head}[2J{tail}")),
        "the name was stripped away along with the escape: {stdout:?}"
    );
}

// --- ONE REPORT AND OUT START ---
//
// **The half `src/main_tests.rs` § ONE REPORT AND OUT says is not its own.** That module can
// read what `live` *returned*; it cannot read the process's own stdout, so *which stream the
// report lands on*, *what the process exits with* and — the one this box's whole title is about
// — *how many reports come out* are only observable from here.
//
// **A listener rather than a cluster, so `just check` stays the whole of CI.** CI has no
// kubernetes, and a test that needs one is a step that silently does not run. What `--once`
// needs to reach the bootstrap gate is five LISTs that answer, and what those need is
// `std::net` and no dependency at all.

/// A listener that answers every LIST with an empty one — every request except the watch, which
/// is [`Watches`]'s subject — and the kubeconfig that points at it. **The `resourceVersion` is
/// load-bearing**: an answer with none is `k8s::Fault::Unanswered`
/// (`src/main_tests.rs`'s own stub says the same about itself), which keeps the bootstrap gate
/// shut and would test the deadline instead of the report.
///
/// **`items: []` for every kind, because an `ObjectList` with no items is shape-compatible with
/// all five** — the same reasoning `src/main_tests.rs`'s own stub is written under, and it is
/// written twice because a helper cannot cross from `tests/` into a private `mod tests`.
///
/// **[`Watches`] is what the caller picks and it is the only thing that varies**, because the
/// watch kube opens after each initial LIST is where every difference between these tests lives.
fn a_cluster_that_answers_with_nothing_in_it(watches: Watches) -> std::path::PathBuf {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    // **The address the kernel handed back, never a literal one.** `scripts/security-guard.py`
    // § no second outbound path refuses a hardcoded host in any URL under `tests/`, and it is
    // right to: a loopback URL spelled out in a source file is indistinguishable from a dev
    // leftover. `src/main_tests.rs`'s own stub is written the same way.
    let address = listener.local_addr().expect("the port it picked");
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { return };
            std::thread::spawn(move || answer_empty_lists(socket, watches));
        }
    });
    let path = std::env::temp_dir().join(format!(
        "k8rs-stub-{}-{}.kubeconfig.yaml",
        std::process::id(),
        address.port()
    ));
    std::fs::write(
        &path,
        format!(
            "apiVersion: v1\nkind: Config\ncurrent-context: stub\n\
             clusters: [{{name: stub, cluster: {{server: 'http://{address}'}}}}]\n\
             contexts: [{{name: stub, context: {{cluster: stub, user: stub}}}}]\n\
             users: [{{name: stub, user: {{}}}}]\n"
        ),
    )
    .expect("the stub kubeconfig writes");
    path
}

/// What the stub does with the watch kube opens after each initial LIST.
///
/// **It is a parameter and not a constant because the two tests over this listener want opposite
/// clusters**, and one stub answering both was how a flake and a vacuous test shipped together
/// (`tester`, 2026-08-30 — the numbers are on each variant). Both are things a real API server
/// does; neither is a model of one.
///
/// **What is gone from here is the third behaviour, which was not a cluster at all**: answering
/// `watch=true` with the `List` body above. That is not a watch stream, so kube classifies it as
/// a watch failure — and the doc that shipped it said the failure landed too late to matter,
/// *"by then all five have listed"*. **Both halves of that sentence were measured false**, one
/// variable changed between two runs of a request-logging listener with this one's wire
/// behaviour (`tester`'s scratchpad, not a committed file): in three runs of six the pods
/// watch was answered *before* the fifth LIST was, and in the other three all five had listed
/// first and the trouble line came out anyway — because a socket answered is not a store updated.
/// Answering: 6 runs, 6 reports carrying *k8rs is not getting pods from this cluster*. Held open:
/// 3 runs, 3 clean reports.
#[derive(Clone, Copy)]
enum Watches {
    /// **Accepted and never answered, which is what a real watch over a cluster where nothing is
    /// happening does.** The request is read, dropped, and the socket left blocked on its next
    /// `read` until the process exits — not refused, because a refusal is a `k8s::Fault` too and
    /// would print its own trouble line.
    ///
    /// **This is the flake fix.** [`a_once_run_over_a_cluster_that_answers_is_the_report_on_stdout_and_exit_0`]
    /// went 5 red in 20 runs against a listener that answered the watch, and 0 red in 30 after —
    /// which is what unblocked `just mutants-diff`, whose unmutated baseline has to be green.
    HeldOpen,
    /// **Accepted and then cut, which is what a real one does when the connection goes** — a
    /// restarted API server, a dropped VPN, an idle NAT entry. kube records the failure, backs
    /// off and re-lists (`src/k8s.rs` § THE DRIVER), so the store keeps changing after the
    /// bootstrap gate has opened.
    ///
    /// **That is the only thing [`a_once_run_prints_one_report_and_not_a_second_one`] can count,
    /// and [`HeldOpen`](Watches::HeldOpen) leaves it nothing to count.** Over a cluster where
    /// nothing happens, the update that opens the gate is the last update there is, so a binary
    /// with `--once`'s latch *deleted* still prints exactly one report — measured, 20 runs of
    /// that test over an unlatched binary, 20 green, 100 invocations, no red. A test that cannot
    /// fail is the thing this file exists to refuse (CLAUDE.md § Tests must not lie), so the
    /// cluster it runs against is one where something is still arriving.
    Cut,
}

fn answer_empty_lists(mut socket: std::net::TcpStream, watches: Watches) {
    use std::io::{Read, Write};
    let body = r#"{"apiVersion":"v1","kind":"List","metadata":{"resourceVersion":"1"},"items":[]}"#;
    let answer = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    );
    let mut pending = String::new();
    loop {
        let mut chunk = [0_u8; 2048];
        match socket.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => pending.push_str(&String::from_utf8_lossy(&chunk[..read])),
        }
        // A LIST is a GET with no body, so a request ends at the blank line.
        while let Some(end) = pending.find("\r\n\r\n") {
            let head = pending[..end].to_string();
            pending.replace_range(..end + 4, "");
            // The watch, and what happens to it is [`Watches`]'s whole subject. Nothing is
            // written back either way, so hyper never puts a second request on this connection
            // and the loop below stays a queue.
            if head.contains("watch=true") {
                match watches {
                    Watches::HeldOpen => continue,
                    // Dropping the `TcpStream` closes it, which is the cut.
                    Watches::Cut => return,
                }
            }
            if socket.write_all(answer.as_bytes()).is_err() {
                return;
            }
        }
    }
}

/// Run the built binary against [`a_cluster_that_answers_with_nothing_in_it`].
fn k8rs_over_a_stub(kubeconfig: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .args(args)
        .env("KUBECONFIG", kubeconfig)
        .output()
        .expect("the built binary runs")
}

/// The header every report opens with over a cluster with nothing in it. **Counting it is how
/// *one report* is asserted at all** — a second report is a second header, and nothing else in
/// the output is unique per report.
const EMPTY_CLUSTER_HEADER: &str = "0 pods · 0 nodes";

/// **`--once` over a cluster that answers: exit `0`, the report on stdout, the connection's own
/// story on stderr** (NOTES § D17, `screens/once.md` § stdout and stderr are split on purpose).
///
/// **It also proves the run ends without anything killing it.** The watches under it never stop
/// — kube's `watcher()` cannot finish and `k8s::StandingBackoff` never gives up — so a `--live`
/// in its place hangs until the harness gives up, and the only thing that can return this
/// process is the stopping point `--once` added.
///
/// **The stream split is asserted in both directions**, which is what `k8rs --once >
/// findings.txt` rests on: the report may not leak onto stderr, and the greeting may not leak
/// into the file.
#[test]
fn a_once_run_over_a_cluster_that_answers_is_the_report_on_stdout_and_exit_0() {
    let kubeconfig = a_cluster_that_answers_with_nothing_in_it(Watches::HeldOpen);

    let out = k8rs_over_a_stub(&kubeconfig, &["--once"]);

    std::fs::remove_file(&kubeconfig).expect("the stub kubeconfig is removed");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = text(out.stdout);
    let stderr = text(out.stderr);
    assert!(
        stdout.contains(EMPTY_CLUSTER_HEADER),
        "the report is not on stdout, so `k8rs --once > findings.txt` writes an empty \
         file: {stdout:?} / {stderr:?}"
    );
    assert!(
        stdout.contains("nothing is broken"),
        "a cluster with nothing in it got no health claim, which is the one thing an empty \
         report may not do: {stdout:?}"
    );
    // **One trailing newline, not two.** The blank line between reports belongs to `--live`,
    // which has a successor to separate; `--once` does not (`screens/once.md` § What it prints
    // ends at the tally), and a redirected report that ends on two leaves a blank line at the
    // foot of `findings.txt`. The file-driven half of the same claim is pinned by the
    // whole-report literal in [`a_healthy_capture_is_the_report_on_stdout_and_exit_0`]; this
    // half was proved once with `od -c` on a real run and asserted nowhere.
    assert!(
        stdout.ends_with('\n') && !stdout.ends_with("\n\n"),
        "the report does not end at exactly one newline: {stdout:?}"
    );
    assert!(
        stderr.starts_with("k8rs: watching — "),
        "the connection's own story is not on stderr: {stderr:?}"
    );
    assert!(
        !stderr.contains(EMPTY_CLUSTER_HEADER),
        "the report reached stderr as well, so a reader redirecting one stream gets it \
         twice: {stderr:?}"
    );
}

/// **`--once` prints exactly one report.** That is the whole of the flag: *connect, print one
/// report, exit* (NOTES § D17, `screens/once.md`) — and `src/main.rs`'s own closure says it in
/// those words, *"so it is skipped whole, and `--once` prints exactly one thing"*.
///
/// **No unit test can see this and that is why it is here.** `live` returns `Option<String>` and
/// answers `None` for *it reported*, which is the same `None` for one report as for four; the
/// count only exists on the process's stdout.
///
/// **Counted by the header line, because a report cannot be told from a report any other way.**
/// The tally is absent on an empty cluster and the trouble lines differ per pass; the header is
/// on every report and on nothing else.
///
/// **[`Watches::Cut`] and not [`Watches::HeldOpen`], because over a cluster where nothing is
/// arriving this test cannot fail at all** — 20 runs of it against a binary with the latch
/// deleted came back 20 green (`tester`, 2026-08-30). The update that opens the gate is the last
/// one there is, so there is no second report to suppress and nothing to count. `Cut` keeps
/// failures and re-lists arriving behind the gate, which is the traffic the latch exists to
/// swallow.
///
/// **[`ATTEMPTS`] runs and not one, because the failure this catches is a race** — whether the
/// updates that produce a second report land in the same poll as the one that opened the gate is
/// the machine's timing, not the program's. Measured over `Cut` against an unlatched binary, one
/// invocation each: see [`ATTEMPTS`] for the rate the count is set from. A test whose red is a
/// coin flip is the shape CLAUDE.md § Tests must not lie refuses, so the coin is flipped until
/// the odds are not a question — and a run that is correct is correct every time, so this costs
/// a fixed binary nothing but the spawns.
#[test]
fn a_once_run_prints_one_report_and_not_a_second_one() {
    let kubeconfig = a_cluster_that_answers_with_nothing_in_it(Watches::Cut);

    let reports: Vec<usize> = (0..ATTEMPTS)
        .map(|_| {
            text(k8rs_over_a_stub(&kubeconfig, &["--once"]).stdout)
                .matches(EMPTY_CLUSTER_HEADER)
                .count()
        })
        .collect();

    std::fs::remove_file(&kubeconfig).expect("the stub kubeconfig is removed");
    // Not `iter().all(..)`: the counts are the evidence, and a bare `false` would say only that
    // one of five runs was wrong without saying which or by how much.
    assert_eq!(
        reports,
        vec![1; ATTEMPTS],
        "a --once run printed a number of reports that is not one, so a reader piping it to a \
         file or to `jq` gets several answers to a question they asked once — reports per run"
    );
}

/// How many times [`a_once_run_prints_one_report_and_not_a_second_one`] runs the binary.
///
/// **Set from a measured miss rate and not a guessed one** (`tester`, 2026-08-30). This constant
/// was temporarily `1` against a binary with `--once`'s latch deleted, over
/// [`Watches::Cut`]: 40 runs, 29 red, **11 missed — 27.5%**. Eight independent invocations miss
/// together about one time in thirty thousand; at the five this was set to before the stub was
/// fixed it was one in six hundred. The extra three cost three process spawns, and a binary that
/// is correct prints one report every time — 200 invocations of the shipped binary over this
/// listener, 25 runs of this file's whole suite at eight attempts each, came back `1` every
/// time — so nothing here is paid twice.
const ATTEMPTS: usize = 8;

/// **A `--once` report that could not be written is exit `2` and a sentence — never a truncated
/// report claiming success** (NOTES § D17; the file-driven half is
/// [`a_write_that_fails_any_other_way_is_exit_2_and_says_why`]).
///
/// **The wiring is the thing under test, and it is different from the file path's.** There the
/// failed write is `main`'s own `match`; here it happens inside a watch observer that returns
/// `()`, so the sentence has to travel back out of an aborted future before `main` can exit on
/// it. A unit test cannot reach it: the write it would have to fail is the process's own stdout.
///
/// `/dev/full` is the kernel returning a real `ENOSPC`, so nothing here invents the error shape
/// (NOTES § D29).
#[test]
fn a_once_report_that_could_not_be_written_is_exit_2_and_says_why() {
    let full = std::fs::File::options()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full is not on this machine, and nothing else is a real ENOSPC");
    let enospc = std::io::Error::from_raw_os_error(28);
    assert_eq!(
        enospc.kind(),
        std::io::ErrorKind::StorageFull,
        "ENOSPC is 28 here, or this test is describing the other arm"
    );
    let kubeconfig = a_cluster_that_answers_with_nothing_in_it(Watches::HeldOpen);

    let out = Command::new(env!("CARGO_BIN_EXE_k8rs"))
        .arg("--once")
        .env("KUBECONFIG", &kubeconfig)
        .stdout(full)
        .stderr(Stdio::piped())
        .output()
        .expect("the built binary runs");

    std::fs::remove_file(&kubeconfig).expect("the stub kubeconfig is removed");
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = text(out.stderr);
    assert!(
        stderr.contains(&format!("k8rs: the report could not be written — {enospc}")),
        "a report that arrived cut in half exited without saying so: {stderr:?}"
    );
}

/// **`--once --analysis` puts the seven panes on stdout, and `--once` alone puts none there**
/// (NOTES § D188).
///
/// **The unit test beside this one asserts the panes over a store, through `live_report`.** That
/// is the arrangement of the seven; what it cannot show is that the flag survives the trip
/// through `live` and lands on the same stream as the cards — and on the one store that unit
/// test uses, a real `--once` run never reaches `live_report` at all, because every watch is
/// refused and the run ends at *this cluster did not show k8rs its pods*.
///
/// **The headings and not the contents.** A cluster with nothing in it fills no pane with
/// anything; what the flag decides is whether they are drawn.
#[test]
fn analysis_under_once_reaches_stdout_and_plain_once_draws_no_panes() {
    let kubeconfig = a_cluster_that_answers_with_nothing_in_it(Watches::HeldOpen);

    let with = k8rs_over_a_stub(&kubeconfig, &["--once", "--analysis"]);
    let without = k8rs_over_a_stub(&kubeconfig, &["--once"]);

    std::fs::remove_file(&kubeconfig).expect("the stub kubeconfig is removed");
    assert_eq!(with.status.code(), Some(0), "{with:?}");
    let panes = text(with.stdout);
    let plain = text(without.stdout);
    // Three of the seven, one per reason they exist: the pane N4 answers in, the one C1's
    // expiring band answers in, and one that is neither.
    for heading in ["[versions]", "[certificates]", "[capacity]"] {
        assert!(
            panes.contains(heading),
            "{heading} did not reach stdout under --once --analysis, so the three rules whose \
             only reader these panes are print nowhere: {panes:?}"
        );
        assert!(
            !plain.contains(heading),
            "the {heading} pane was drawn without --analysis, which buries the cards the run \
             exists to show: {plain:?}"
        );
    }
}

// --- ONE REPORT AND OUT END ---

// --- ONE OBJECT'S LOG START ---
//
// **The log path's public surface, and the byte shapes no unit test above it can spell.**
// `src/k8s_tests.rs` proves `read_lines` over a `Feed`, `src/main_tests.rs` proves `logs_run`
// over a `kube::Client` — both read a function's answer. What neither can see is which of the
// two streams a line lands on, what the process exits with, and what a *byte sequence* does on
// the way through: `src/main_tests.rs`'s own log stub takes its body as a `&'static str`, so a
// line cut in the middle of a multi-byte character, a `\r\n` ending and a body larger than the
// retained ceiling are shapes only a stub that writes `[u8]` can feed
// (CLAUDE.md § A check is proven only for the input shapes it was fed, NOTES § D29).
//
// **The listener is [`a_cluster_that_answers_with_nothing_in_it`]'s, extended by two paths and
// not rewritten beside it** — one pod GET and one log GET. Everything else still answers the
// empty list `k8s::connect` needs, because `--logs` goes through the same connect as `--once`.

/// Every path the log listener was asked for, in the order it was asked.
///
/// **The query string is the half that matters.** `--previous` and `--follow` are two switches
/// whose whole observable effect is a query parameter, and a `kubectl` line that prints one the
/// request did not carry is invariant 4's record lying — which is only checkable from the side
/// that receives the request.
type Asked = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// **A listener that answers one pod and one log, and the kubeconfig that points at it.**
///
/// **`pieces` are HTTP chunks and that is the point** — one `Transfer-Encoding: chunked` chunk
/// per piece, so the caller decides where a body boundary falls, the way `src/k8s_tests.rs`'s
/// `Feed::of` decides where a read boundary falls. No piece at all is a body of zero bytes,
/// which is a container that has written nothing.
///
/// **`pod` names a committed capture** (`tests/fixtures/`), never a hand-written object
/// (CLAUDE.md § Fixtures come from real cluster captures).
fn a_cluster_that_answers_one_pod_and_one_log(
    pod: &str,
    pieces: Vec<Vec<u8>>,
) -> (std::path::PathBuf, Asked) {
    let body = std::fs::read(fixture(&format!("{pod}.json"))).expect("the capture reads");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    // The address the kernel handed back, never a literal one —
    // `scripts/security-guard.py` § no second outbound path refuses a hardcoded host under
    // `tests/`, and the stub above is written the same way.
    let address = listener.local_addr().expect("the port it picked");
    let asked: Asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = std::sync::Arc::clone(&asked);
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(socket) = socket else { return };
            let body = body.clone();
            let pieces = pieces.clone();
            let seen = std::sync::Arc::clone(&seen);
            std::thread::spawn(move || answer_one_log(socket, &body, &pieces, &seen));
        }
    });
    let path = std::env::temp_dir().join(format!(
        "k8rs-logstub-{}-{}.kubeconfig.yaml",
        std::process::id(),
        address.port()
    ));
    std::fs::write(
        &path,
        format!(
            "apiVersion: v1\nkind: Config\ncurrent-context: stub\n\
             clusters: [{{name: stub, cluster: {{server: 'http://{address}'}}}}]\n\
             contexts: [{{name: stub, context: {{cluster: stub, user: stub}}}}]\n\
             users: [{{name: stub, user: {{}}}}]\n"
        ),
    )
    .expect("the stub kubeconfig writes");
    (path, asked)
}

/// The three answers [`a_cluster_that_answers_one_pod_and_one_log`] gives, by path.
fn answer_one_log(mut socket: std::net::TcpStream, pod: &[u8], pieces: &[Vec<u8>], seen: &Asked) {
    use std::io::{Read, Write};
    let empty =
        br#"{"apiVersion":"v1","kind":"List","metadata":{"resourceVersion":"1"},"items":[]}"#;
    let mut pending = String::new();
    loop {
        let mut chunk = [0_u8; 2048];
        match socket.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => pending.push_str(&String::from_utf8_lossy(&chunk[..read])),
        }
        // Every request here is a GET with no body, so one ends at the blank line.
        while let Some(end) = pending.find("\r\n\r\n") {
            let request: String = pending.drain(..end + 4).collect();
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            seen.lock()
                .expect("the record is never poisoned")
                .push(path.clone());
            // **`/log?` or a path ending in it, never the substring** — a pod named
            // `catalog` has `/log` inside its own path, and a stub that answered a log for a
            // pod GET would leave every test below reading a body nothing asked for.
            let sent = if path.contains("/log?") || path.ends_with("/log") {
                // **Chunked, so `pieces` survives as body frames** rather than being flattened
                // into one `content-length` write the client reads back in its own 8 KiB steps.
                let mut answer = b"HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\n\
                                   transfer-encoding: chunked\r\n\r\n"
                    .to_vec();
                // A zero-length chunk *is* the terminator, so an empty piece would end the
                // body early and every piece after it would vanish without a word. No body at
                // all is spelled by an empty `pieces`, which is the loop running zero times.
                for piece in pieces.iter().filter(|piece| !piece.is_empty()) {
                    answer.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
                    answer.extend_from_slice(piece);
                    answer.extend_from_slice(b"\r\n");
                }
                answer.extend_from_slice(b"0\r\n\r\n");
                answer
            } else {
                let body: &[u8] = match path.contains("/pods/") {
                    true => pod,
                    false => empty,
                };
                let mut answer = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                answer.extend_from_slice(body);
                answer
            };
            if socket.write_all(&sent).is_err() {
                return;
            }
        }
    }
}

/// The capture every test below streams a log for: `default/healthy`, whose snapshot carries an
/// init container and a regular one — two, so the container block has something to list.
const POD: &str = "default/healthy";

/// One fetched log, as the two streams and the exit code — the whole of what this file can see.
fn one_log(pod: &str, pieces: Vec<Vec<u8>>, args: &[&str]) -> (Output, Vec<String>) {
    let (kubeconfig, asked) = a_cluster_that_answers_one_pod_and_one_log(pod, pieces);
    let out = k8rs_over_a_stub(&kubeconfig, args);
    // Removed before the assertions, so a red run leaves nothing behind either.
    std::fs::remove_file(&kubeconfig).expect("the stub kubeconfig is removed");
    let seen = asked.lock().expect("the record is never poisoned").clone();
    (out, seen)
}

/// **`\r\n` is a line ending too, and a log that stops mid-line still hands that line over.**
///
/// **Neither shape had been fed through the binary.** `src/k8s_tests.rs` feeds a last line with
/// no newline to `read_lines` directly; nothing anywhere feeds a `\r`, which reaches
/// `k8s::text` as a character that is both unprintable *and* whitespace — the one class that
/// becomes a space rather than being deleted, and would end every line of a Windows-written
/// container's log with a trailing one.
#[test]
fn a_crlf_log_that_ends_mid_line_is_lines_on_stdout_with_no_carriage_return_left() {
    let (out, _) = one_log(
        "healthy",
        vec![b"connected to postgres\r\nwriting checkpoint\r\npanic: killed here".to_vec()],
        &["--logs", "--object", POD],
    );

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        text(out.stdout),
        "connected to postgres\nwriting checkpoint\npanic: killed here\n",
        "a `\\r\\n` log did not come out as three clean lines — either the carriage return \
         survived to the screen, or the line a crash is explained by was dropped for having no \
         newline after it"
    );
}

/// **A container that has written nothing is a state and not a hang** (`screens/detail.md` §
/// No logs yet, `PRIOR-ART § E1`): exit `0`, an empty stdout, and the sentence on stderr.
///
/// **Empty stdout is half the assertion.** `k8rs --logs … | wc -l` has to answer `0`, which is
/// what puts the sentence on the other stream.
#[test]
fn a_log_that_delivers_no_bytes_at_all_is_a_state_and_not_a_failure() {
    let (out, _) = one_log("healthy", Vec::new(), &["--logs", "--object", POD]);

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        text(out.stdout),
        "",
        "a log with nothing in it put something on stdout, so `k8rs --logs | wc -l` no longer \
         answers 0 for a container that has written nothing"
    );
    let stderr = text(out.stderr);
    assert!(
        stderr.contains("k8rs: nothing has been written to this container's log yet"),
        "a container that has written nothing said nothing about it, which reads as a hang: \
         {stderr:?}"
    );
}

/// The per-line cap `screens/detail.md` § The buffer states, in bytes. **Written here rather
/// than read off `k8s::FREE_TEXT`**, which this target cannot see and which would in any case
/// be the implementation asserting itself.
const CAP: usize = 4096;

/// What a cut says, `screens/detail.md`'s own words.
const MARKER: &str = "… (shortened by k8rs)";

/// **One byte under the cap, exactly on it, and one byte over** — the boundary in all three
/// directions.
///
/// **4 095 was the unfed one.** `src/k8s_tests.rs` feeds exactly `FREE_TEXT` and `FREE_TEXT + 1`;
/// a cut written `>=` rather than `>` is caught by the first of those, but a cap read as
/// `4096 - 1` anywhere on the path is not, and the line under it is the only place that shows.
#[test]
fn a_line_at_the_cap_and_one_either_side_is_cut_only_when_it_is_over() {
    let mut body = Vec::new();
    for length in [CAP - 1, CAP, CAP + 1] {
        body.extend_from_slice(&b"a".repeat(length));
        body.push(b'\n');
    }
    let (out, _) = one_log("healthy", vec![body], &["--logs", "--object", POD]);

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let lines: Vec<String> = text(out.stdout).lines().map(str::to_string).collect();
    assert_eq!(
        lines.len(),
        3,
        "three lines went in and {} came out",
        lines.len()
    );
    for (line, length) in lines.iter().zip([CAP - 1, CAP]) {
        assert_eq!(
            line.len(),
            length,
            "a line of {length} bytes came back {} — the cap is off by one and a line that fitted \
             was cut",
            line.len()
        );
        assert!(
            !line.ends_with(MARKER),
            "a line of {length} bytes was marked as shortened when nothing was lost"
        );
    }
    assert!(
        lines[2].ends_with(MARKER),
        "the first byte over the cap was cut without saying so — a debugging tool that quietly \
         shortens the evidence is lying about what it saw"
    );
    assert_eq!(
        lines[2].len(),
        CAP + MARKER.len(),
        "the line over the cap came back {} bytes, so the cut did not land on the cap",
        lines[2].len()
    );
}

/// **k8rs never prints a character the container did not write** — the property `k8s::LINE_READ`'s
/// four spare bytes exist for, asserted rather than argued.
///
/// **A four-byte character starting at byte 4 093 is the one case that reaches it.** The reader
/// stops holding an unterminated line at `FREE_TEXT + 4`; cut at `FREE_TEXT` instead, exactly
/// three of that character's four bytes are held, `from_utf8_lossy` turns them into one
/// replacement character of exactly three bytes, the line therefore does not exceed the cap, the
/// cut that would have removed it never runs — and `U+FFFD` reaches the screen with k8rs's own
/// *(shortened by k8rs)* after it. Every other offset either keeps the character whole or has it
/// truncated away, which is why the author's `FREE_TEXT + 1` line does not show this.
#[test]
fn a_character_straddling_the_cut_is_never_replaced_by_one_k8rs_invented() {
    let mut body = b"a".repeat(CAP - 3);
    body.extend_from_slice("\u{1f600}".as_bytes());
    // Past `FREE_TEXT + 4`, so the reader stops holding and the line is marked.
    body.extend_from_slice(&b"a".repeat(64));
    body.push(b'\n');
    let (out, _) = one_log("healthy", vec![body], &["--logs", "--object", POD]);

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = text(out.stdout);
    assert!(
        !stdout.contains('\u{fffd}'),
        "k8rs printed a replacement character the container never wrote, so a reader debugging \
         from this line is reading a byte k8rs invented: {stdout:?}"
    );
    assert!(
        stdout.trim_end().ends_with(MARKER),
        "the line was cut and did not say so: {stdout:?}"
    );
}

/// **A line the strip empties is still a line the container wrote.**
///
/// **`screens/detail.md` puts the strip before the bounds** — *control characters are stripped
/// before any of the three bounds is applied* — so what is bounded is the stripped text and what
/// is counted is the line. A container that wrote a newline wrote a line, and swallowing it
/// would silently renumber everything a reader counts against.
#[test]
fn a_line_of_nothing_but_characters_that_cannot_print_is_still_a_line() {
    let (out, _) = one_log(
        "healthy",
        // Three framings of the class (NOTES § D31): a line that is *entirely* strippable, a
        // line whose escape sequence leaves its printable tail behind, and an empty line the
        // container itself wrote.
        vec![
            "before\n\u{1b}\u{7}\u{200b}\u{feff}\n\u{1b}[2J\n\nafter\n"
                .as_bytes()
                .to_vec(),
        ],
        &["--logs", "--object", POD],
    );

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        text(out.stdout),
        "before\n\n[2J\n\nafter\n",
        "a line the strip emptied was swallowed, an escape sequence left something on the screen \
         that has no printed form, or the container's own empty line was renumbered away"
    );
}

/// **A body past the retained ceiling is bounded, and the count above it is exact**
/// (`screens/detail.md` § When the buffer fills).
///
/// **Every number here is derived from the screen and not from a run**: the ceiling is 2 MB, the
/// lines are 3 000 bytes because that is over the per-line cap's half and well under it, and
/// `2 097 152 / 3 000` is 699 lines kept — so 800 in is 101 dropped. A ceiling moved in either
/// direction changes the sentence this asserts.
///
/// **800 lines is deliberately far under the 5 000-line bound**, so it is the byte ceiling that
/// evicts and not the line count — the case `screens/detail.md` says takes over when lines run
/// long, and the one no test above this file feeds through the binary.
#[test]
fn a_body_past_the_retained_ceiling_prints_exactly_what_it_dropped() {
    const CEILING: usize = 2 * 1024 * 1024;
    const LONG: usize = 3_000;
    const SENT: usize = 800;
    let kept = CEILING / LONG;
    let dropped = SENT - kept;
    let mut body = Vec::new();
    for line in 0..SENT {
        // Every line the same length, so `kept` is arithmetic and not an estimate.
        let head = format!("{line:06} ");
        body.extend_from_slice(head.as_bytes());
        body.extend_from_slice(&b"x".repeat(LONG - head.len()));
        body.push(b'\n');
    }
    let (out, _) = one_log("healthy", vec![body], &["--logs", "--object", POD]);

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = text(out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(
            format!("{dropped} lines were dropped from the top to keep this pane bounded.")
                .as_str()
        ),
        "the dropped-lines sentence is not the screen's own, or the count is not exact — the \
         screen says it is never rounded or bucketed"
    );
    assert_eq!(
        lines.len(),
        kept + 1,
        "{SENT} lines of {LONG} bytes left {} lines and one sentence, where {CEILING} bytes buys \
         {kept}",
        lines.len() - 1
    );
    assert!(
        lines[1].starts_with(&format!("{:06} ", SENT - kept)),
        "the oldest surviving line is {:?} — a buffer that dropped from the bottom throws away \
         the newest lines, which are the ones a reader is watching for",
        &lines[1][..8.min(lines[1].len())]
    );
    assert!(
        lines
            .last()
            .is_some_and(|last| last.starts_with(&format!("{:06} ", SENT - 1))),
        "the newest line never arrived"
    );
    assert!(
        lines[1..].iter().map(|line| line.len()).sum::<usize>() <= CEILING,
        "the pane came out over the {CEILING} bytes the screen promises is true in the worst \
         case as well as the common one"
    );
}

/// **A container the pod does not have is refused with the names it does have**, because the
/// reader's next action is to retype it and the only thing they need is the spelling.
#[test]
fn a_container_the_pod_does_not_have_is_refused_with_the_names_it_has() {
    let (out, asked) = one_log(
        "healthy",
        vec![b"never read\n".to_vec()],
        &["--logs", "--object", POD, "--container", "sidecar-envoy"],
    );

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(text(out.stdout), "", "a refused run put a log on stdout");
    let stderr = text(out.stderr);
    assert!(
        stderr
            .contains("k8rs: this pod has no container named sidecar-envoy — it has migrate, app"),
        "the refusal did not name what the pod actually has: {stderr:?}"
    );
    assert!(
        !asked.iter().any(|path| path.contains("/log")),
        "a container that does not exist still reached the cluster as a log request: {asked:?}"
    );
}

/// **`--previous` and `--follow` together reach the request *and* the line that teaches it**
/// (invariant 4: the command log and the real call may not disagree).
///
/// **The pair had never been fed through the binary.** `src/k8s_tests.rs` builds a `LogRequest`
/// with both switches on directly; what that cannot show is that both survive argv, the
/// no-previous-run fallback and the request builder on one run.
///
/// **`crashloop` and not `healthy`**, because its container has restarted: `--previous` on one
/// that has not is turned off on purpose, and asserting the pair over such a pod would assert
/// the fallback instead.
#[test]
fn previous_and_follow_together_reach_the_request_and_the_kubectl_line() {
    let (out, asked) = one_log(
        "crashloop",
        vec![b"boom\n".to_vec()],
        &[
            "--logs",
            "--object",
            "default/broken-crashloop",
            "--previous",
            "--follow",
        ],
    );

    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stderr = text(out.stderr);
    assert!(
        stderr.contains("$ kubectl logs broken-crashloop -n default -c quitter --previous -f"),
        "the teaching line is not the command a reader could have typed: {stderr:?}"
    );
    let log = asked
        .iter()
        .find(|path| path.contains("/log"))
        .unwrap_or_else(|| panic!("no log was ever asked for: {asked:?}"));
    assert!(
        log.contains("previous=true") && log.contains("follow=true"),
        "the request carried neither switch the `kubectl` line above printed, which is the \
         command log describing a call that was never made: {log:?}"
    );
    assert!(
        !stderr.contains("hasn't restarted"),
        "a container with restarts was told it had none: {stderr:?}"
    );
}

/// **Half an instruction, a name that would leave its path segment, and a name with something in
/// it that cannot print — all refused before anything is connected to** (the security gate's
/// *object names are sanitised before they build a filesystem path* row, invariant 9).
///
/// **Through the process and not through `mistyped`.** `src/main_tests.rs` asserts the refusals
/// over the function; what it cannot see is that the sentence lands on stderr with nothing on
/// stdout, that the exit code is `2`, and — the invariant 9 half — that no control character
/// survives the trip out. `KUBECONFIG` points at nothing, so a shape that was *not* refused
/// would fail on the connection instead, with a different sentence.
#[test]
fn a_log_run_that_named_something_unusable_is_refused_with_nothing_on_stdout() {
    for line in [
        vec!["--logs"],
        vec!["--object", "default/web"],
        vec!["--logs", "--object", "../secrets"],
        vec!["--logs", "--object", "default/../secrets"],
        vec!["--logs", "--object", "default/web?watch=true"],
        vec!["--logs", "--object", "a/b/c"],
        vec!["--logs", "--object", "web/"],
        vec!["--logs", "--object", "/web"],
        vec!["--logs", "--object", "de\u{1b}[2Jfault/web"],
        vec!["--logs", "--object", "default/we\u{202e}b"],
        vec!["--logs", "--object", "default/web\u{7}"],
    ] {
        let out = k8rs_with_no_kubeconfig(&line);

        assert_eq!(
            out.status.code(),
            Some(2),
            "{line:?} was not refused: {out:?}"
        );
        assert_eq!(
            text(out.stdout),
            "",
            "{line:?} put something on stdout, so a mistyped log run writes a usage text into \
             `k8rs --logs > out.txt` and calls it a log"
        );
        let stderr = text(out.stderr);
        assert!(
            stderr.starts_with("k8rs: ") && stderr.contains("usage: k8rs "),
            "{line:?} was refused without the usage under it: {stderr:?}"
        );
        assert!(
            !stderr.contains(CONNECT_CANARY),
            "{line:?} reached a connection before it was refused: {stderr:?}"
        );
        let survivors: Vec<char> = stderr
            .lines()
            .flat_map(str::chars)
            .filter(|c| c.is_control())
            .collect();
        assert!(
            survivors.is_empty(),
            "{line:?} put control characters on stderr: {survivors:?}"
        );
    }
}

// --- ONE OBJECT'S LOG END ---
