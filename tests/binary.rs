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

/// **`--live` with nothing to connect to: exit 2, stderr, and an empty stdout.**
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
fn live_with_no_kubeconfig_is_exit_2_on_stderr_and_leaves_stdout_empty() {
    let out = k8rs_with_no_kubeconfig(&["--live"]);

    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(out.stdout.is_empty(), "{:?}", text(out.stdout.clone()));
    let stderr = text(out.stderr);
    assert!(stderr.starts_with("k8rs: "), "{stderr:?}");
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
    for args in [
        vec!["--live", "--namespace"],
        vec!["--live", "-n"],
        vec!["--live", "--namespace="],
        vec!["--live", "-n="],
        vec!["--live", "--namespace", "../secrets"],
        vec!["--live", "-n", "../secrets"],
        vec!["--live", "--namespace=a/b"],
        vec!["--live", "-n=.."],
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
            !stderr.contains("no cluster to watch"),
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
