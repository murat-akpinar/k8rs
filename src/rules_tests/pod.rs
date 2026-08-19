//! `rules.rs` § THE POD RULES — its tests (NOTES § D91).

use super::*;

// --- THE POD RULES, AGAINST THE COMMITTED CAPTURES ---
//
// Positive *and* negative for every rule, and the negatives are the half that matters:
// a rule with only a positive is a rule nobody has proved quiet. The healthy captures
// are asserted **empty**, not "not this one finding" — a false positive from any other
// rule reaches the same screen and is the same defect.
//
// **The clock is the second input, and two tests vary it rather than the capture.**
// Rules 7 and 12 both have a threshold, and a threshold nobody crosses is a threshold
// nobody has tested. `now` is a field of the snapshot precisely so a rule can be read
// at a chosen moment (invariant 5, NOTES § D18), so the same committed pod is analysed
// just inside and just outside its window. That is not the "one field changed on a real
// capture" technique above — nothing about the capture moves.

fn findings_at(names: &[&str], now: Time) -> Vec<Finding> {
    analyze(&pods_at(names.iter().map(|n| pod(n)).collect(), now))
}

fn findings(names: &[&str]) -> Vec<Finding> {
    findings_at(names, now())
}

/// **When the run a container is sitting in began** — `state.running.startedAt`, which is rule
/// 5's stamp on a serving card and the field its suppression is measured from (NOTES § D100).
/// Panics on a container that is not running, because every caller below is reading a serving
/// card and a `None` there would silently move the moment to the pin.
fn began_running(p: &PodSnapshot, name: &str) -> Time {
    match &container(p, name).state {
        ContainerState::Running { started_at } => started_at
            .clone()
            .expect("a running container the API stamped"),
        other => panic!("{name} is not running, so it has no current run: {other:?}"),
    }
}

/// **A moment `mins` minutes into that run.** Every serving-card assertion in this file is read
/// at one of these rather than at [`now`], because the restart captures were taken 49 hours
/// before the pin and rule 5 stands its serving card down after [`NOT_READY_GRACE`]
/// (NOTES § D100) — the same bytes at two clocks, which is what
/// `an_old_kill_on_a_container_that_has_been_fine_since_…` does for rule 2.
fn into_the_run(p: &PodSnapshot, name: &str, mins: i64) -> Time {
    Time(
        began_running(p, name)
            .0
            .checked_add(SignedDuration::from_mins(mins))
            .expect("a moment inside a captured run"),
    )
}

/// The pod, and the moment five minutes into its container's current run — the pair every
/// serving-card test hands to [`analyze`].
fn serving_at(p: PodSnapshot, name: &str) -> ClusterSnapshot {
    let moment = into_the_run(&p, name, 5);
    pods_at(vec![p], moment)
}

/// [`analyze`] over that pair, with the cards printed at *that* moment rather than at the pin —
/// so `--nocapture` shows the age the reader of a serving card actually sees.
fn serving_findings(p: PodSnapshot, name: &str) -> Vec<Finding> {
    let snapshot = serving_at(p, name);
    let all = analyze(&snapshot);
    show_at(&all, &snapshot.now);
    all
}

/// **The numbers and the words that came out of a document, asserted against the
/// document.** Everything else below is proved by a capture; these cannot be, because a capture
/// can show a threshold being *applied* and never show where it was set. A constant transcribed
/// from REQUIREMENTS is still a requirement, and without this test lowering rule 5's warn band to
/// a single restart stays green — every card in the corpus would simply move with it.
///
/// **The corpus does not pin them, which is not the same as the corpus being empty**
/// (NOTES § D114). Regular containers now sit at `0, 1, 3, 4, 9, 10, 13` restarts and carry
/// `0, 1, 3, 127, 137, 143, 255` between them, so the bands and most of the translation table do
/// have objects either side. What no capture can say is that the boundary belongs at **3** and
/// **10** rather than wherever the code currently puts it, and that is this test's whole subject.
#[test]
fn the_thresholds_and_the_exit_table_are_the_ones_the_documents_write_down() {
    assert_eq!(
        (RESTARTS_WARN, RESTARTS_CRITICAL),
        (3, 10),
        "REQUIREMENTS: rule 5 warns at three restarts and turns critical at ten"
    );

    // NOTES § v1 rule set's translation table. Every row has to be a *sentence*: the
    // reader who has just met `137` for the first time is exactly who rule 6 is
    // written for (invariant 14).
    for (code, reason, must_say) in [
        (137, Some("OOMKilled"), "more memory than it was allowed"),
        // **The row NOTES got wrong, and the reason it is asserted twice.** A
        // liveness-probe kill that outlives the grace period lands as exit 137 with
        // reason `Error`; the memory sentence there sends someone to raise a limit on
        // a container whose health endpoint is timing out.
        //
        // **It names the signal and stops, since 2026-08-15.** *Did not stop when it was
        // asked to* was asserted for every 137 the word `OOMKilled` was missing from, and
        // three shapes reach that exact object without it: a container that ignores
        // SIGTERM, a genuine cgroup kill on a host too short of memory to attribute it
        // (NOTES § D84), and a rebuilt sandbox killing a container nothing asked to stop
        // (NOTES § D90). The negatives below are the half this substring cannot hold.
        (137, Some("Error"), "killed with SIGKILL"),
        (137, None, "killed with SIGKILL"),
        // **The third meaning, and it is not a kill at all.** `convertToAPIContainerStatuses`
        // writes this pair where the kubelet could not read a status, not where it watched
        // one end — so the row says the number was written in and claims nothing about the
        // run (NOTES § D90, [`STATUS_LOST`]).
        (137, Some(STATUS_LOST), "lost track of the container"),
        // **The fourth meaning, and it is beta-on-by-default at the version the fixtures pin.**
        // `RestartAllContainersOnContainerExits` is `{1.36, Default: true, Beta}` in
        // `kube_features.go`, and when a container that declares `restartPolicyRules` exits into
        // a matching rule the kubelet removes the other containers to restart them together —
        // the field is a container's and the effect is the pod's
        // ([`RESTART_ALL`], NOTES § D96) — with
        // `exitCode: 137`, this reason, and the
        // message *the container is removed because RestartAllContainers in place*. Nothing
        // failed and nothing was killed: the pod asked for it (NOTES § D93, [`RESTART_ALL`]).
        (137, Some(RESTART_ALL), "restart every container in the pod"),
        (143, None, "ordinary shutdown"),
        // **The row the table did not have**, which is why `exit 0` reached the screen as a
        // bare number under a card about crashing (NOTES § D85). The kubelet's own `reason`
        // beside it is `Completed`, and it changes nothing: 0 is 0.
        //
        // **And it names an ending rather than an agent.** *The program finished successfully*
        // is a claim about who ended the run, printed one line above an action whose whole
        // subject is that the code cannot make that claim — a program stopped by a probe or by
        // a memory killer on the node reports `0` too (NOTES § D85, § D88).
        (0, Some("Completed"), "the run ended without an error"),
        (0, None, "the run ended without an error"),
        (1, None, "the application's own error"),
        (2, None, "the application's own error"),
        (126, None, "could not be run"),
        (127, None, "was not found"),
        // **The row the table did not have on 2026-08-16**, which is why the commonest broken-pod
        // state there is — a mistyped `command` — printed a bare `exit 128` over an action about
        // the container's command, with nothing joining the two (invariant 14, NOTES § D113).
        (128, None, "could not start"),
    ] {
        let said = exit_meaning(code, reason)
            .unwrap_or_else(|| panic!("NOTES § v1 rule set translates exit {code}"));
        assert!(
            said.contains(must_say),
            "exit {code} {reason:?} reads {said:?}"
        );
    }
    assert_eq!(
        exit_meaning(42, None),
        None,
        "and a code the table does not cover is not given an invented meaning"
    );
    // **`128` is hedged, and the substring above cannot hold that.** Two authors reach this code —
    // the runtime that could not start the container, and a program that called `exit(128)` — and
    // the record does not say which, so a row asserting the first outright would be the `0` row's
    // defect at a different number (NOTES § D113).
    let start_failure = exit_meaning(128, None).expect("128 is translated");
    assert!(
        start_failure.starts_with("usually"),
        "the one code in this table whose cause the object does not settle says so: \
         {start_failure:?}"
    );
    // **And the actor it names is the one the rest of the table names** (invariant 14). The two
    // rows keyed on [`CODE_UNKNOWN`] already tell this reader *the node found the container dead*
    // and *the node could not tell what code the container ended with*, about the same layer; a
    // third row introducing `runtime` teaches a word nothing on the card explains, and no card
    // this translation reaches explains one — the hostpath rules do, two rules away and on a
    // screen of their own.
    for code in [-1, 128] {
        let said =
            exit_meaning(code, None).unwrap_or_else(|| panic!("the table translates exit {code}"));
        assert!(
            said.contains("the node") && !said.contains("runtime"),
            "exit {code} names the same actor as the rows beside it, in the word they use: \
             {said:?}"
        );
    }
    // **The `0` row may not name an agent, and that is the half the substring above cannot
    // hold**: *the program finished successfully* passes any token about a clean ending while
    // claiming the one thing the code cannot say. It printed one line above an action spending
    // five lines on the opposite, which is NOTES § D85's own shape rebuilt by the fix for it
    // (NOTES § D88).
    for reason in [Some("Completed"), None] {
        let said = exit_meaning(0, reason).expect("the table translates 0");
        assert!(
            !said.contains("program"),
            "a run a liveness probe stopped, and one a memory killer on the node stopped, both \
             report 0 — so the translation says how the run ended and leaves who ended it to the \
             action: {said:?}"
        );
    }
    // **And `137` may not name a cause either, which is the same requirement one row down.** The
    // translation is printed by rules 1, 5 and 6 and takes **no role** — so a cause named here
    // is named on an init container's card too, where `validateInitContainers` has already
    // refused the probe half of it and the action one line below says so. Naming who sent the
    // signal is the action's job, and the action knows the role ([`killed_action`], NOTES § D85,
    // § D88, § D90).
    // **And the two reasons the kubelet writes itself may not be read as kills**, which the
    // substrings above cannot hold: *killed* would pass a table lookup on either while telling
    // the reader something took a container that nothing took (NOTES § D93).
    for (reason, why) in [
        (STATUS_LOST, "the kubelet never watched this run end"),
        (
            RESTART_ALL,
            "the pod's own restart rule removed it on purpose",
        ),
    ] {
        let said = exit_meaning(137, Some(reason)).expect("the table translates 137");
        assert!(
            !said.to_lowercase().contains("killed"),
            "{reason}: {why}, so a row calling it a kill is the number read as a signal it \
             never was: {said:?}"
        );
    }
    // **No `137` row may name a probe, whatever its reason** — the translation takes no role, and
    // an init container is allowed none.
    for reason in [
        Some("Error"),
        None,
        Some("OOMKilled"),
        Some(STATUS_LOST),
        Some(RESTART_ALL),
    ] {
        // Lowercased like every other [`PROBE_WORDS`] site — the two comparisons in this test
        // were the exceptions that made that constant's own doc comment untrue.
        let said = exit_meaning(137, reason)
            .expect("the table translates 137")
            .to_lowercase();
        for probe in PROBE_WORDS {
            assert!(
                !said.contains(probe),
                "137 {reason:?} may not name {probe:?}: this sentence prints on an init \
                 container's card too, one line above an action that has to pick doors for the \
                 role — {said:?}"
            );
        }
    }
    // **And the rows whose agent the object does not name may not claim one.** *Did not stop when
    // it was asked to* was asserted of every unlabelled `137`, and three shapes reach that object
    // with nobody having asked anything (NOTES § D84, § D90, § D93). **[`RESTART_ALL`] is
    // deliberately not in this list**: there the object *does* name the agent — the pod's own
    // restart rule — and a row that refused to say so would be hiding what D71's mechanism exists
    // to surface.
    for reason in [Some("Error"), None, Some(STATUS_LOST)] {
        let said = exit_meaning(137, reason)
            .expect("the table translates 137")
            .to_lowercase();
        assert!(
            !said.contains("asked"),
            "137 {reason:?} may not say anything was asked of the container: the object \
             separates none of the endings that reach it — {said:?}"
        );
    }

    // The formatter, over a real captured termination with one field moved — the same
    // technique the decode tests use, and for the same reason: no capture carries an
    // exit code outside the table, and this is a string function rather than a rule.
    let mut run = container(&pod("crashloop"), "quitter")
        .last_terminated
        .clone()
        .expect("the captured crash loop records how its last run ended");
    assert!(
        exit_fact(&run).starts_with("exit 1 ("),
        "the number the reader searched for comes first: {}",
        exit_fact(&run)
    );
    run.exit_code = 42;
    assert_eq!(
        exit_fact(&run),
        "exit 42",
        "and where the number alone is the honest answer, the number alone is what shows"
    );
}

/// The action's own width on the card it is drawn on — `screens/alerts.md` § How wide a card is:
/// the body indents two columns and `→ ` takes two more, so an action line is 49.
const ACTION_COLUMNS: usize = 49;

/// **A greedy wrap that breaks on spaces, and on characters where a token leaves it no space to
/// break at**, which is the conservative reading of the budget: `textwrap` also splits at a
/// hyphen, so `re-runs` measures one line shorter under it than under a renderer that does not —
/// and a cap measured the generous way passes a string that does not fit (NOTES § D88).
///
/// **The character break is the renderer's own behaviour**, not this measure being strict:
/// `screens/alerts.md` § The height gives a token wider than the line a break by character
/// "because there is no word boundary to find", and ratatui does the same. Handing such a token
/// one line whatever its width is the measure under-reporting a card — a 400-column URL would
/// pass a five-line cap while drawing nine rows, which is latent only for as long as no action
/// carries an image reference or a link.
fn wrapped_at(text: &str, columns: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= columns => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.extend(
                word.chars()
                    .collect::<Vec<char>>()
                    .chunks(columns)
                    .map(|chunk| chunk.iter().collect::<String>()),
            ),
        }
    }
    lines
}

/// **`screens/alerts.md`'s action budget, held on the helper this box rewrote** — that file caps
/// an action at five wrapped lines and states that an action over the cap is a `rules.rs`
/// finding and not a layout problem. [`finished_action`]'s arms measured nine, eight and nine
/// lines; the init one put 15 of the body pane's 16 rows on one card, which is exactly the
/// property the cap exists to hold — the pane always shows a second finding (NOTES § D88).
///
/// **It guards this helper's arms directly, and
/// [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`] guards every other
/// action in the file** — which is where the four that were over the cap were finally caught
/// (NOTES § D113). What this test refuses is a rewrite of *this* helper that quietly grows back,
/// on the shapes a card cannot reach at all: **five arms now**, because the duration decides the
/// order on two of the three roles and only [`crash_looping`] passes one.
#[test]
fn the_clean_exit_actions_fit_the_card_they_are_drawn_on() {
    // **The measure first, or the guard passes because nothing was measured.** Three shapes: text
    // that fits several words to a line, a word wider than the line, and one wider than the line
    // the arms are actually measured at (NOTES § D29).
    assert_eq!(
        wrapped_at("aa bb cc dd ee", 5),
        ["aa bb", "cc dd", "ee"],
        "the wrap fills a line to the column and breaks on the space before the word that would \
         pass it"
    );
    assert_eq!(
        wrapped_at("supercalifragilistic x", 5),
        ["super", "calif", "ragil", "istic", "x"],
        "and a word wider than the line costs every line it fills: there is no space to break it \
         at, so the renderer breaks it by character (`screens/alerts.md` § The height) and the \
         word after it starts a line of its own"
    );
    // **And the same shape at the width the cap below is enforced at**, which is what makes the
    // assertion above a defect fixed rather than a nicety: giving an over-long token one line
    // whatever its width under-reports the card by however wide the token is — a
    // 400-column one measured a single line and passed the cap while filling nine rows of the
    // sixteen the pane has. The longest token in the three arms below is `Kubernetes`, 10 against
    // 49, so this is latent — and it stops being latent on the first action carrying a link or an
    // image reference.
    let link = "https://registry.invalid/v2/does-not-exist/manifests/v9";
    let broken = wrapped_at(link, ACTION_COLUMNS);
    assert_eq!(
        (broken.len(), broken.concat().as_str()),
        (link.chars().count().div_ceil(ACTION_COLUMNS), link),
        "a token with no space in it fills as many lines as it has columns for, and every \
         character of it survives the measure — the old one gave a link a single line whatever \
         its width, so a card was measured shorter than the one drawn: {broken:?}"
    );

    // **Every arm the helper has, and the `None` beside the two durations is a card that shows
    // none** (NOTES § D113). One second under [`PROBE_FLOOR`] and one second over are the two
    // sides of the only comparison it makes.
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for role in [
        ContainerRole::Regular,
        ContainerRole::Sidecar,
        ContainerRole::Init,
    ] {
        for ran_for in [
            None,
            Some(PROBE_FLOOR - SignedDuration::from_secs(1)),
            Some(PROBE_FLOOR),
            Some(PROBE_FLOOR + SignedDuration::from_secs(1)),
        ] {
            let action = finished_action(role, ran_for);
            seen.insert(action);
            let lines = wrapped_at(action, ACTION_COLUMNS);
            println!(
                "{role:?} {ran_for:?}: {} chars, {} lines at {ACTION_COLUMNS} columns\n  {}",
                action.chars().count(),
                lines.len(),
                lines.join("\n  ")
            );
            assert!(
                lines.len() <= 5,
                "{role:?} {ran_for:?}: an action that wraps past five lines is a `rules.rs` \
                 finding (`screens/alerts.md` § The height) — {} lines: {action:?}",
                lines.len()
            );
        }
    }
    // Or the loop measured one arm four times and called it coverage (NOTES § D26).
    assert_eq!(
        seen.len(),
        5,
        "five distinct arms: two roles that reorder, times two sides of the threshold, plus the \
         init one that does not"
    );

    // **Which side of [`PROBE_FLOOR`] the boundary itself falls on, asserted rather than left to
    // the widths above** (NOTES § D113). The number is derived: `initialDelaySeconds: 0`,
    // `periodSeconds: 10`, `failureThreshold: 3` puts the probe at 0s, 10s and 20s and the third
    // consecutive failure kills — so a run of **exactly** 20 seconds is one a probe *could* have
    // ended, and only a shorter one gets the reordered sentence. `<=` here would tell the reader
    // a health check rarely kills at the one length where it demonstrably can.
    for role in [ContainerRole::Regular, ContainerRole::Sidecar] {
        assert_ne!(
            finished_action(role, Some(PROBE_FLOOR - SignedDuration::from_secs(1))),
            finished_action(role, Some(PROBE_FLOOR)),
            "{role:?}: one second under the floor is the short arm"
        );
        assert_eq!(
            finished_action(role, Some(PROBE_FLOOR)),
            finished_action(role, None),
            "{role:?}: and the floor itself is not short — the third probe lands exactly there, \
             so the door it would demote is still open"
        );
    }
}

/// [`mounted_path`] on the shapes the API can produce and the fixtures do not contain.
/// A pure string function, so it is asserted as one — the escalators above it are three
/// equality tests and they only mean what they read as if this normalises.
#[test]
fn what_the_container_actually_gets_is_normalised_before_it_is_compared() {
    let mount = |path: &str, sub: Option<&str>, expr: Option<&str>| HostPathMount {
        path: path.to_string(),
        sub_path: sub.map(str::to_string),
        sub_path_expr: expr.map(str::to_string),
        read_only: false,
        container: "c".to_string(),
    };

    // `//` and `/.` both pass upstream validation — absolute, no backsteps — and both
    // resolve to the node's root. Unnormalised they are not `"/"`, so they fall into
    // the writable branch: silenced in `kube-system`, and elsewhere advised with
    // "mount it read-only", about the whole machine.
    for spelling in ["/", "//", "/.", "/./", "///."] {
        assert_eq!(
            mounted_path(&mount(spelling, None, None)),
            "/",
            "{spelling} is the node's root"
        );
    }
    // NOTES § D46's own example: the socket is only visible once the subPath is joined.
    assert_eq!(
        mounted_path(&mount("/var/run", Some("docker.sock"), None)),
        "/var/run/docker.sock"
    );
    // And the join narrows as well as widens — this is `hostpath.json`'s own shape.
    assert_eq!(
        mounted_path(&mount("/", Some("run/containerd"), None)),
        "/run/containerd"
    );
    assert_eq!(mounted_path(&mount("/var/log/", None, None)), "/var/log");

    // A `subPathExpr` narrows the mount by something k8rs cannot read, so the path
    // stops being the root — the safe direction, since the alternative is the loudest
    // possible false CRITICAL.
    let expr = mounted_path(&mount("/", None, Some("$(POD_NAME)")));
    assert_eq!(expr, "/$(POD_NAME)");
    assert_ne!(
        expr, "/",
        "a container given one directory does not have the machine"
    );

    // A constant nobody can match is a rule that never fires, and every entry here is
    // compared with `==` against this function's output.
    for socket in RUNTIME_SOCKETS {
        assert_eq!(
            mounted_path(&mount(socket, None, None)),
            socket,
            "{socket} is not in the form this function produces, so rule 8 could never \
             match it"
        );
        // **The invariant [`is_runtime_socket`]'s fold rests on.** It strips a leading
        // `/var` and compares what is left against this list, which only finds the
        // second spelling of anything if every entry is written under `/run`. An entry
        // elsewhere would be matched under one name and silently missed under the
        // other — the defect this list has already had once (NOTES § D77) — so it is
        // the constant that is asserted and not the stripping.
        assert!(
            socket.starts_with("/run/"),
            "{socket} is not under /run: stripping a leading `/var` can never produce \
             it, so it would be matched under the one spelling written here and missed \
             under whatever its second name is"
        );
    }
    // **Every member, by name, and the naming kept in step with the list.** The sweeps
    // over rule 8 *iterate* this constant, so they structurally cannot notice a member
    // gone — "matched nothing" and "had nothing to match" are the same green line
    // (CLAUDE.md § Code phase rules). Docker was the one entry with no canary and
    // deleting it survived every mutation `tester` ran (NOTES § D78).
    let canaries = [
        (
            "/run/docker.sock",
            "NOTES § v1 rule set names Docker's socket as the escalator's own example, \
             and it is still what a pre-2022 cluster and every Docker-in-Docker build \
             agent mounts",
        ),
        (
            "/run/containerd/containerd.sock",
            "kind — the cluster every fixture here came off — runs containerd, so a \
             list that stops at Docker's socket is a rule that cannot fire on its own \
             test bed",
        ),
        (
            "/run/crio/crio.sock",
            "CRI-O under the spelling a manifest may actually write is the miss NOTES \
             § D78 exists for",
        ),
        (
            "/run/k3s/containerd/containerd.sock",
            "k3s puts containerd's socket here and nowhere else, and RKE2 embeds k3s's \
             containerd — the distro half of the audience that meets this in a normal \
             week",
        ),
        (
            "/run/cri-dockerd.sock",
            "cri-dockerd is in crictl's own default endpoint probe list, and it is \
             what every node that kept Docker past 1.24 runs, minikube included",
        ),
    ];
    for (socket, why) in canaries {
        assert!(RUNTIME_SOCKETS.contains(&socket), "{socket}: {why}");
    }
    for socket in RUNTIME_SOCKETS {
        assert!(
            canaries.iter().any(|(named, _)| *named == socket),
            "{socket} was added to RUNTIME_SOCKETS without a canary here, so deleting \
             it again would be green: every sweep below iterates the list"
        );
    }
}

/// Rules 1, 5 and 6 on the one pod that earns all three, which is also where every
/// piece of invariant 14 is visible at once: the loop is explained, the exit code is
/// translated, and the container's own last line replaces "go and read the logs".
#[test]
fn the_crash_looping_pod_gets_the_loop_the_count_and_the_exit() {
    let raw = fixture("crashloop");
    let all = findings(&["crashloop"]);
    show(&all);

    assert_eq!(
        all.len(),
        2,
        "rules 1 and 6, and nothing else — rule 5 stays quiet on a container rule 1 is \
         already describing, one incident being one card: {:?}",
        titles(&all)
    );
    assert_eq!(
        all.iter()
            .filter(|f| f.title.contains("has been restarted"))
            .count(),
        0,
        "and the count is already on rule 1's own evidence line: {:?}",
        titles(&all)
    );

    let looping = only(&all, "broken-crashloop", "CrashLoopBackOff");
    assert_eq!(looping.severity, Severity::Critical);
    assert!(
        looping.evidence.contains("container quitter"),
        "the finding names which container: {}",
        looping.evidence
    );
    // The count comes off the capture: how many times a crash loop has gone round by the
    // time the trip photographs it belongs to the cluster, and a literal here reddens on
    // every `just fixtures` for a requirement that never moved.
    let quitter = captured_status(&raw, "containerStatuses", "quitter");
    assert!(
        looping.evidence.contains(&format!(
            "{} restarts",
            captured_i32(quitter, &["restartCount"])
        )),
        "{}",
        looping.evidence
    );
    assert!(
        looping.evidence.contains("ran for 2s"),
        "D51's first fork of a crashloop triage — how long each run survives, which \
         `describe` makes a human subtract at 3am. **One spelling, from [`ran_for`]**: rules 1, 6 \
         and 15 print this off one [`lasted`] call and wrote it three ways until 2026-08-16, \
         which is NOTES § D85's class inside the mechanism written to collapse the duplicate: {}",
        looping.evidence
    );
    assert!(
        looping
            .evidence
            .contains("exit 1 (the application's own error)"),
        "invariant 14: the code is translated, never printed and left: {}",
        looping.evidence
    );
    // **The command is the one this arm's *action* names, and since 2026-08-16 that is the log**
    // (invariant 4, NOTES § D113). Rules 1 and 6 answer [`Ending::Failed`] with one sentence, and
    // that sentence sends the reader to the run's own log — so the card owes `logs --previous`
    // rather than `describe`, which prints no logs at all. The evidence line's facts come from
    // the snapshot and not from the command, the same footing [`stopped_for_good`] has stood on
    // since it took [`logs`].
    assert_eq!(
        looping.kubectl_cmd.as_deref(),
        Some("kubectl logs broken-crashloop -c quitter -n default --previous"),
        "the action names that run's log, so the command has to serve it"
    );
    assert_eq!(
        looping.owner, looping.object,
        "nothing controls this pod, so it files under itself (D3's fallback)"
    );

    // **The moment the run ended, never the moment it began** — both are in the same
    // struct one line apart, and this capture keeps them two seconds apart, which is
    // what makes the second assertion mean anything at all (`Finding::timestamp`).
    let died = at(
        captured_status(&raw, "containerStatuses", "quitter"),
        &["lastState", "terminated"],
    );
    assert_eq!(
        looping.timestamp,
        Some(captured_time(died, &["finishedAt"]))
    );
    assert_ne!(
        captured_time(died, &["finishedAt"]),
        captured_time(died, &["startedAt"]),
        "a capture whose run started and ended in the same second cannot tell the right \
         field from the wrong one, and is not the fixture for this assertion"
    );
    assert_eq!(
        looping.age(&now()).as_deref(),
        Some("13 hours ago"),
        "a duration, not English parsed back into a number"
    );

    let failed = only(&all, "broken-crashloop", "on record failed");
    assert_eq!(failed.severity, Severity::Warn);
    // **The quote is evidence and no longer the action** (NOTES § D113). The kubelet kept the tail
    // of the log, so the card still shows it instead of sending the reader to fetch what k8rs is
    // already holding — and it is the *last* line, not the `starting` this capture opens with. But
    // it sits on the evidence line, behind the three-line cut, because **the action is k8rs's own
    // words and a string the cluster wrote is never one**: the same field on a mistyped `command`
    // carries containerd's whole `runc` error, and that stood where the *what to do* belongs.
    // **The frame still says who recorded the line and not who wrote it** ([`last_words`]) — the
    // runtime reaches this field too.
    assert!(
        failed.evidence.contains(
            "Kubernetes recorded this: panic: dial tcp db.payments.svc:5432: connect: \
             connection refused"
        ),
        "the container's own last line still reaches the card, whole and framed: {}",
        failed.evidence
    );
    assert_eq!(
        failed.action,
        failed_run_action(&exited_run(1), ContainerRole::Regular).0,
        "and the *what to do* is the rule's own sentence, decided by whether the run ever ran — \
         the same sentence rule 1 gives this ending on this container (NOTES § D113)"
    );
    assert!(
        failed.evidence.contains("ran for 2s"),
        "and how long the run survived, which is the fork between bad configuration and \
         a leak: {}",
        failed.evidence
    );
    assert!(
        failed.evidence.find("Kubernetes recorded this") < failed.evidence.find("ran for 2s"),
        "the quote goes ahead of the duration, the order [`stopped_for_good`] already prints them \
         in — one fact, one place (NOTES § D97): {}",
        failed.evidence
    );
    // **The one card in the file whose command is `kubectl logs --previous`** (invariant 4,
    // NOTES § D113). *Read the logs of that run* is the right sentence here — the subject is a run
    // that is over, and this ending's record carries the `containerID` the kubelet gates the flag
    // on — and it stood under [`describe`], which prints no logs at all. The sentence was never
    // the defect; the command under it was.
    assert!(
        failed.action.contains("read the last run's log"),
        "this is the arm that names a log — the other two say nothing about one and keep \
         `describe`: {}",
        failed.action
    );
    // **And it names *which* run, because two of its three cards give the phrase no antecedent.**
    // Rule 1's title is *keeps crashing* and rule 5's is a restart count; *that run* had nothing
    // on either card to attach to, and the run this sentence means is the one `lastState` holds
    // (invariant 14).
    assert!(
        failed.action.contains("--previous"),
        "and the flag its own command carries is explained on the card, because it is the one \
         word here a reader in their first month cannot guess: {}",
        failed.action
    );
    assert_eq!(
        failed.kubectl_cmd.as_deref(),
        Some("kubectl logs broken-crashloop -c quitter -n default --previous"),
        "and an action that names a log owes the one command that serves it"
    );
}

/// **Rule 2's permanence, and the two directions that separate it from a suppressor that
/// would be wrong.**
///
/// `lastState.terminated` never expires, so a container the kernel killed once and that
/// has served ever since would draw a CRITICAL for the life of the pod — and a single kill
/// never reaches [`restarting_repeatedly`]'s `>= 3`, so nothing else carries that pod and
/// nothing ever clears it. But *serving* is not what makes it wrong: a container killed
/// five minutes ago and running now is exactly what belongs on this screen, because the
/// next spike will do it again. Only the two together stand the rule down.
///
/// Both directions are asserted, or the clause is half-proven — one of them alone passes
/// against `if doing_its_job(c)` on its own, and the other against a rule that has stopped
/// firing at all.
///
/// **Captured, and nothing is edited at all.** This was `oom.json` with the kill moved into
/// its past and the container written back up for as long as no committed object held both
/// halves; `broken-oomserving` is that object — one OOM kill in `lastState`, one restart, and a
/// container that is running and ready again — so the two directions are the *same bytes* read
/// at two moments, which is what [`ClusterSnapshot::now`] being an input is for (D18).
#[test]
fn an_old_kill_on_a_container_that_has_been_fine_since_is_not_on_the_broken_now_screen() {
    let recovered = pod("oomserving");
    let app = container(&recovered, "app");
    let killed_at = app
        .last_terminated
        .as_ref()
        .and_then(|run| run.finished_at.clone())
        .expect("the capture records when the kernel took it");
    assert!(
        doing_its_job(app)
            && app.restarts < RESTARTS_WARN
            && matches!(&app.last_terminated, Some(run)
                if run.reason.as_deref() == Some("OOMKilled")),
        "the capture has to be a serving container that still carries the kill, with a \
         count below rule 5's band so nothing else answers for this pod: {app:?}"
    );
    let after = |mins: i64| {
        Time(
            killed_at
                .0
                .checked_add(SignedDuration::from_mins(mins))
                .expect("a moment after the captured kill"),
        )
    };

    nothing(
        &findings_at(&["oomserving"], after(60 * 24 * 30)),
        "the kernel killed this container a month ago and it has been serving ever \
         since. Nothing is broken *now*, and the card could never be cleared — whether \
         its limit is right is a memory-limit question for the Capacity report (D2)",
    );

    // The other direction, and the reason `doing_its_job` alone is the wrong suppressor:
    // the kill is inside the grace, so it is news.
    let all = findings_at(&["oomserving"], after(5));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "a container the kernel killed five minutes ago is running now on borrowed time, \
         and it will happen again on the next spike: {:?}",
        titles(&all)
    );
    assert_eq!(
        only(&all, "broken-oomserving", "OOMKilled").severity,
        Severity::Critical
    );

    // And the pinned `now` is already past the window, so the committed object as it stands is
    // the silent half — the capture itself, with no moment chosen for it.
    nothing(
        &findings(&["oomserving"]),
        "an hour after the kill, read at the pin, the same bytes draw nothing",
    );
}

/// Rule 2, and the one place two rules would otherwise describe a single death.
#[test]
fn the_out_of_memory_card_names_the_limit_and_rule_6_stays_out_of_its_way() {
    let raw = fixture("oom");
    let all = findings(&["oom"]);
    show(&all);

    let killed = only(&all, "broken-oom", "OOMKilled");
    assert_eq!(killed.severity, Severity::Critical);
    assert!(
        killed
            .title
            .contains("used more memory than it was allowed")
            && killed.title.contains("kernel killed it"),
        "invariant 14: OOMKilled is explained and then named, never printed alone: {}",
        killed.title
    );
    // The limit the kubelet enacted, read back off `status.resources` — the field D51
    // sent this rule to, so that a pending resize cannot make the card name a figure
    // the container was never given.
    let enacted = captured_str(
        captured_status(&raw, "containerStatuses", "hog"),
        &["resources", "limits", "memory"],
    );
    assert!(
        killed.evidence.contains(&format!("limit {enacted}")),
        "the evidence line carries the enacted limit ({enacted}): {}",
        killed.evidence
    );
    assert!(killed.evidence.contains("exit 137"), "{}", killed.evidence);
    assert_eq!(
        killed.timestamp,
        Some(captured_time(
            at(
                captured_status(&raw, "containerStatuses", "hog"),
                &["lastState", "terminated"]
            ),
            &["finishedAt"]
        ))
    );

    assert_eq!(
        all.iter()
            .filter(|f| f.title.contains("on record failed"))
            .count(),
        0,
        "rule 6 owns the exit-code table and rule 2 owns this death; both firing puts \
         two cards on one event, the weaker of which says 'exit 137, almost always \
         memory' beside one that already names the limit: {:?}",
        titles(&all)
    );
    assert_eq!(
        all.len(),
        2,
        "rule 2 and the restart card — this container is dying over and over *and* was \
         OOM-killed, and that is one incident with two causes to name, not three cards: {:?}",
        titles(&all)
    );

    // **Which rule draws the second card depends on where in the backoff loop the capture
    // landed, and the fixture is certified for both** — `scripts/cluster.sh` § `[oom]` reads
    // the kill out of `lastState.terminated // state.terminated`, because a crash-looping
    // container is in `state.terminated` more often than in `waiting: CrashLoopBackOff` (39
    // samples of 70, measured there). In backoff it is rule 1's card; caught between runs it is
    // rule 5's. Naming either one here is asserting the coin-flip, which is what this assertion
    // did until 2026-08-16 — the same shape as rule 3's `ErrImagePull` / `ImagePullBackOff` pair
    // a few tests below.
    //
    // **What the requirement actually says is face-independent**: both rules call [`exit_fact`],
    // so whichever of the two speaks, the memory sentence survives where the reason earns it.
    let sibling: Vec<&Finding> = all
        .iter()
        .filter(|f| !f.title.contains("OOMKilled"))
        .collect();
    assert_eq!(
        sibling.len(),
        1,
        "one card beside rule 2's, whichever half of the loop the capture caught: {:?}",
        titles(&all)
    );
    assert!(
        sibling[0].title.contains("CrashLoopBackOff") || sibling[0].title.contains("restarted"),
        "and it is a card about the repetition — rule 1 in backoff, rule 5 between runs — not \
         some third rule that started answering here: {}",
        sibling[0].title
    );
    assert!(
        sibling[0]
            .evidence
            .contains("more memory than it was allowed"),
        "exit 137 *with* `OOMKilled` beside it is the memory kill, and the rule that draws \
         the second card calls the same translator rule 2 does: {}",
        sibling[0].evidence
    );
}

/// Rules 3 and 4. Both are a waiting reason plus the runtime's own sentence, and the
/// sentence is the entire diagnosis in each case (NOTES § D37).
#[test]
fn an_unpullable_image_and_a_missing_configmap_each_name_what_to_go_and_fix() {
    let all = findings(&["image", "config"]);
    show(&all);
    assert_eq!(all.len(), 2, "one card each: {:?}", titles(&all));

    let image = only(&all, "broken-image", "image is not usable");
    assert_eq!(image.severity, Severity::Critical);
    assert!(
        image.title.contains("ErrImagePull") || image.title.contains("ImagePullBackOff"),
        "the kubelet alternates between the two as it backs off, and whichever this \
         capture caught is the word the reader sees in `kubectl get pods`: {}",
        image.title
    );
    assert!(
        image
            .evidence
            .contains("image registry.invalid/does-not-exist:v9"),
        "the resolved name is printed beside the runtime's sentence, because rule 3's \
         action is 'check the image name': {}",
        image.evidence
    );
    assert!(
        image.evidence.contains("no such host"),
        "and the runtime's own sentence is what says the pull actually failed: {}",
        image.evidence
    );
    assert_eq!(
        image.timestamp, None,
        "nothing in a container status records when the first pull was attempted, and \
         `screens/alerts.md` would rather leave the right edge blank than borrow a \
         nearby moment"
    );

    // **`describe` never prints `state.waiting.message`.** kubectl's `describeStatus`
    // renders a waiting container's `Reason` and stops, and that message — the sentence
    // naming the registry that refused, or the ConfigMap that is absent — *is* the whole
    // evidence line of both these cards. It reaches `describe` only through an Event,
    // reworded and gone at `--event-ttl`. A teaching command that does not show what the
    // card says is worse than none (invariant 4), which is the same argument rule 12 is
    // already built on.
    assert_eq!(
        image.kubectl_cmd.as_deref(),
        Some("kubectl get pod broken-image -n default -o yaml")
    );

    let config = only(&all, "broken-config", "ConfigMap or Secret");
    assert_eq!(config.severity, Severity::Critical);
    assert!(
        config
            .evidence
            .contains("configmap \"this-configmap-does-not-exist\" not found"),
        "rule 4's whole value is the name of the object that is missing: {}",
        config.evidence
    );
    assert_eq!(
        config.kubectl_cmd.as_deref(),
        Some("kubectl get pod broken-config -n default -o yaml"),
        "for the same reason as rule 3's above"
    );
}

/// **`subPathExpr` reaches no capture**, because nothing in `scripts/broken.yaml` uses
/// one — and the field it guards against produces the loudest wrong card in the box, a
/// CRITICAL claiming a container has the whole machine when it was given one directory.
/// So the decode is asserted with the technique the rest of this section uses: one
/// field, on a real object, set to a value the API demonstrably produces.
///
/// **Capture trip:** a pod in `scripts/broken.yaml` mounting `hostPath: /` with
/// `subPathExpr: $(POD_NAME)` and the `fieldRef` env var that resolves it, which is the
/// ordinary way this is written.
#[test]
fn a_mount_narrowed_by_an_environment_variable_is_carried_unresolved() {
    let mut object: Pod =
        serde_json::from_value(fixture("hostpath")).expect("hostpath.json is a Pod");
    let spec = object.spec.as_mut().expect("the captured pod has a spec");
    let mount = spec
        .containers
        .iter_mut()
        .find(|c| c.name == "nosy")
        .and_then(|c| c.volume_mounts.as_mut())
        .into_iter()
        .flatten()
        .find(|m| m.name == "root")
        .expect("the capture mounts the host volume in nosy");
    // Upstream forbids both on one mount, so the capture's `subPath` goes as the
    // expression arrives — one edit, not two.
    mount.sub_path = None;
    mount.sub_path_expr = Some("$(POD_NAME)".to_string());

    let p = PodSnapshot::from(object);
    let narrowed = p
        .host_path_mounts
        .iter()
        .find(|m| m.container == "nosy")
        .expect("the mount survives the decode");
    println!("{narrowed:?} -> {}", mounted_path(narrowed));

    assert_eq!(
        narrowed.sub_path_expr.as_deref(),
        Some("$(POD_NAME)"),
        "carried verbatim: the values are in env and in the Secrets behind it, and k8rs \
         reads neither"
    );
    assert_eq!(
        mounted_path(narrowed),
        "/$(POD_NAME)",
        "and it joins like a subPath, so the path stops being the node's root"
    );
    assert_ne!(
        mounted_path(narrowed),
        "/",
        "which is the whole point — a container handed one directory does not have the \
         machine, and rule 8 saying so would be its loudest false CRITICAL"
    );
}

/// Rule 7, both sides of its clock. **Without the window this rule fires on every
/// rolling update**, so the window is the rule (NOTES § D46, § D51).
#[test]
fn a_pod_out_of_the_service_is_only_a_finding_once_it_has_been_that_way_a_while() {
    let raw = fixture("readiness");
    let all = findings(&["readiness"]);
    show(&all);
    assert_eq!(all.len(), 1, "rule 7 alone: {:?}", titles(&all));

    let unready = only(&all, "broken-readiness", "not receiving traffic");
    assert_eq!(unready.severity, Severity::Warn);
    assert!(
        unready.evidence.contains("container app"),
        "{}",
        unready.evidence
    );

    // **The since-when is floored at the container's own run start.** `Ready` is the
    // *pod's* condition and does not move until every container is ready, so a
    // container younger than that condition would be dated to a moment it did not
    // exist for. This capture separates the two by five seconds, so the wrong field is
    // visible here rather than hidden behind two equal timestamps.
    let condition = captured_time(captured_condition(&raw, "Ready"), &["lastTransitionTime"]);
    let began = captured_time(
        captured_status(&raw, "containerStatuses", "app"),
        &["state", "running", "startedAt"],
    );
    assert!(
        began.0 > condition.0,
        "a capture whose container started before the pod went unready cannot tell the \
         floor from the condition, and is not the fixture for this assertion"
    );
    assert_eq!(
        unready.timestamp,
        Some(began.clone()),
        "the later of the two, because a container cannot have been out of the Service \
         for longer than its current run has existed"
    );
    assert_ne!(
        unready.timestamp,
        Some(condition),
        "the pod's own condition dates this container to before it was running"
    );

    // Just inside the window: the same captured pod, read at exactly the grace. This is
    // every pod of every rolling update, and it draws nothing.
    //
    // **The ten minutes are written out here and not read from [`NOT_READY_GRACE`]** — a
    // boundary computed from the constant under test agrees with every value that
    // constant could hold, and no other test pins it. The *moment* is the capture's, so
    // the pair survives the next `just fixtures` without being repinned.
    let after = |mins: i64, secs: i64| {
        Time(
            began
                .0
                .checked_add(SignedDuration::from_mins(mins) + SignedDuration::from_secs(secs))
                .expect("ten minutes after a captured container started is a moment"),
        )
    };
    nothing(
        &findings_at(&["readiness"], after(10, 0)),
        "ten minutes unready is a readiness probe with an `initialDelaySeconds`, not an \
         outage",
    );
    // And one second past it.
    assert_eq!(
        findings_at(&["readiness"], after(10, 1)).len(),
        1,
        "past `progressDeadlineSeconds`' own default is where Kubernetes itself stops \
         calling a rollout healthy"
    );

    // **`started` is read here as a suppressor, which is not the trigger D51 rejected.**
    // This capture declares no `startupProbe`, so it reports `true`; one that declares a
    // slow startup probe reports `false`, and until it passes the kubelet does not run the
    // readiness probe at all — `ready: false` there means *not asked yet*. **That object is
    // captured**: `startup.json`, read by
    // `a_container_still_inside_its_startup_probe_is_not_one_failing_its_readiness_check`.
    assert!(
        container(&pod("readiness"), "app").started,
        "the positive fixture has to be past its startup for this rule to reach it"
    );
}

/// Rule 5's warn band and the sentence that only holds for a container that is
/// actually serving — and rule 6's silence beside it, which is the same fixture's
/// second job.
///
/// **One capture, two clocks** (NOTES § D100). Five minutes into the run this container is
/// sitting in, three restarts is news and the card draws. Forty-nine hours later — the pinned
/// [`now`], which is where the committed bytes sit — the same container has been serving all
/// that time and the card is gone, because `restartCount` never comes down and nothing else
/// would ever clear it. The second direction is the one the box is about: without it this card
/// is on the screen for the life of the pod.
#[test]
fn a_container_that_looks_fine_still_gets_a_card_for_how_often_it_has_died() {
    let snapshot = serving_at(pod("restarts"), "flaky");
    let news = snapshot.now.clone();
    let all = analyze(&snapshot);
    show_at(&all, &news);
    assert_eq!(
        all.len(),
        1,
        "rule 5 alone. **Rule 6 is deliberately silent here**: `lastState.terminated` \
         never expires, so a container that failed once and has served ever since would \
         carry that card for the life of the pod — the largest false-positive volume in \
         the box, and one that needs nothing unusual but uptime: {:?}",
        titles(&all)
    );
    assert!(
        container(&pod("restarts"), "flaky")
            .last_terminated
            .is_some(),
        "the capture does carry a failed previous run, so the silence above is the rule \
         deciding and not the field being absent"
    );

    let counted = only(&all, "broken-restarts", "restarted 3 times");
    assert_eq!(
        counted.severity,
        Severity::Warn,
        "3 is rule 5's warn band and 10 is where it becomes critical (REQUIREMENTS)"
    );
    assert!(
        counted.title.contains("it is serving now"),
        "this container *is* passing its probes, which is the whole of why NOTES words \
         rule 5 'looks healthy now, but something is wrong': {}",
        counted.title
    );

    assert_eq!(
        counted.severity,
        Severity::Warn,
        "and it stays WARN whatever the count while the container is serving: a red card \
         whose own title says it is serving is what teaches a reader to stop believing \
         red (NOTES § D2)"
    );

    // **And the card is dated, which it was not until D100.** The stamp is the start of the run
    // the container is in — the moment the counter last went up — and not the `lastState` this
    // capture happens to carry one on: a gang restart leaves that record with no stamp at all,
    // and the card would render with no age on a screen with an age column.
    assert_eq!(
        counted.timestamp.as_ref(),
        Some(&began_running(&pod("restarts"), "flaky")),
        "rule 5 dates a serving card by `state.running.startedAt`"
    );
    assert_eq!(
        counted.age(&news).as_deref(),
        Some("5 min ago"),
        "and the reader is told when, in the words `screens/widgets.md` spells"
    );

    // **The other direction, and the whole of the box.** Same bytes, 49 hours later.
    nothing(
        &findings(&["restarts"]),
        "this container used its three restarts two days ago and has served ever since. \
         Nothing is broken *now*, the count can never come down, and no other rule carries \
         this pod — so the card would be permanent (NOTES § D100)",
    );
}

/// **The suppression's edges, and the two shapes that must survive it** (NOTES § D100).
///
/// The clause is [`out_of_memory`]'s, with rule 5's field: `is_some_and`, never
/// `unwrap_or(huge)`. So the exemption has to be **proved by the object** — a container the API
/// gave no start time, and one whose start time is in the future, both keep the card, because
/// neither says *this container has been fine for ten minutes*. Written the other way round they
/// would both silence it, and the second one silences it for a whole hour on a node whose clock
/// is ahead.
///
/// **The boundary is asserted on both sides of [`NOT_READY_GRACE`]**, since a threshold nobody
/// crosses is a threshold nobody has tested — the technique rule 7's test uses, and `>` rather
/// than `>=` is what puts the moment itself on the drawing side.
///
/// **Neither of the two shapes is in the corpus and neither can be** — D100 measured 14,672
/// running-container samples with a `startedAt` and none without, and a future stamp needs a
/// skewed clock rather than a broken workload. They are planted on a decoded copy, which is what
/// a plant is for: the requirement is about what the *rule* may assume, not about what this
/// cluster happened to write (NOTES § D40).
#[test]
fn the_serving_card_ages_out_only_where_the_object_proves_it_has_been_fine() {
    let base = pod("restarts10serving");
    let began = began_running(&base, "flaky");
    assert!(
        doing_its_job(container(&base, "flaky")),
        "every direction below is about the serving branch, which is the only one that ages out"
    );

    // The threshold itself is still news, and one second past it is not.
    let edge = into_the_run(&base, "flaky", 10);
    let all = findings_at(&["restarts10serving"], edge.clone());
    show_at(&all, &edge);
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    assert_eq!(
        card.age(&edge).as_deref(),
        Some("10 min ago"),
        "ten minutes in, the container has not yet outlasted the grace Kubernetes' own \
         `progressDeadlineSeconds` gives it"
    );
    nothing(
        &findings_at(
            &["restarts10serving"],
            Time(
                began
                    .0
                    .checked_add(SignedDuration::from_mins(10) + SignedDuration::from_secs(1))
                    .expect("a second past the grace is a moment"),
            ),
        ),
        "and one second past it the container has been serving longer than Kubernetes waits \
         for a rollout, which is where this stops being news",
    );

    // **No start time: the card stays.** Read at the pin, 49 hours after the capture, where a
    // clause written `unwrap_or(huge)` or `map_or(true, …)` would silence it.
    let unstamped = capture_but("restarts10serving", |p| {
        container_status(p, "flaky").state = Some(ApiContainerState {
            running: Some(ContainerStateRunning { started_at: None }),
            ..ApiContainerState::default()
        });
    });
    assert!(
        doing_its_job(container(&unstamped, "flaky")),
        "the plant leaves the container serving — it is the stamp that goes, and nothing else"
    );
    let all = analyze(&pods_at(vec![unstamped], now()));
    show(&all);
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    assert_eq!(
        card.timestamp, None,
        "there is no field to date it from, so the card draws no age — and `Finding::age` \
         answering nothing is what the renderer already handles"
    );
    assert_eq!(
        card.severity,
        Severity::Warn,
        "and it is the serving card, unchanged: an object that cannot prove the container has \
         been fine keeps it (NOTES § D100)"
    );

    // **A start time in the future: the card stays too.** A node whose clock is an hour ahead
    // says nothing about how long this container has been up, and `duration_since` there is
    // negative — which is smaller than the grace under any reading, so only the direction of the
    // comparison keeps this card.
    let ahead = Time(
        now()
            .0
            .checked_add(SignedDuration::from_mins(60))
            .expect("an hour past the pin is a moment"),
    );
    let skewed = capture_but("restarts10serving", |p| {
        container_status(p, "flaky").state = Some(ApiContainerState {
            running: Some(ContainerStateRunning {
                started_at: Some(ahead.clone()),
            }),
            ..ApiContainerState::default()
        });
    });
    assert!(
        doing_its_job(container(&skewed, "flaky")),
        "and this plant leaves the container serving too — a card kept because the container \
         stopped serving would prove nothing about the clause under test"
    );
    let all = analyze(&pods_at(vec![skewed], now()));
    show(&all);
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    assert_eq!(
        (card.severity, card.title.contains("it is serving now")),
        (Severity::Warn, true),
        "it is the serving card that survived, and not the down one: {}",
        card.title
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&ahead),
        "the card carries the field it was given, wrong clock and all"
    );
    assert_eq!(
        card.age(&now()),
        None,
        "and `age` refuses to draw a moment that far ahead, so the reader sees a card with no \
         age rather than one dated in the future (NOTES § D69)"
    );
}

/// **Rule 5 is not sampled while a gang restart has the container parked** (NOTES § D100).
///
/// `RestartAllContainers` puts every container in the pod into
/// `waiting: RestartingAllContainers` for about two seconds of every cycle, and this rule's
/// severity is keyed on whether the container is serving — so the *same* card was measured
/// flipping WARN ↔ CRITICAL on every restart, 1104 samples against 354 of one container. The
/// exemption is one more string beside `CrashLoopBackOff` on the line that already refuses to
/// read a container mid-restart; the alternative — deleting `&& !serving` and keying severity on
/// the count alone — is refused in writing, because `restarts10serving.json` exists to prove a
/// serving container at ten restarts is WARN.
///
/// **The control is what makes it an exemption rather than a rule that stopped firing**: the same
/// plant with the waiting reason a container gets between ordinary restarts still draws, so the
/// silence is keyed on *this* reason and not on waiting at all.
///
/// **No committed capture holds this state, and the capture trip that was owed it has been and
/// gone** (NOTES § D114). `gang.json` landed on 2026-08-16 — a genuine `RestartAllContainers`
/// rule firing on kind v1.36.1 — and it is **not** this shape: it caught the pod *settled*, both
/// containers back `Running` and `ready` with the `137` / [`RESTART_ALL`] record in `lastState`.
/// That is the object
/// [`a_container_the_pods_own_restart_rule_removed_is_not_a_run_that_failed`] reads one rule
/// over, and it now reads it off bytes.
///
/// **What is still uncaptured is the parked window itself** — `waiting:
/// RestartingAllContainers`, which lasts about two seconds per cycle and is exactly what this
/// exemption is keyed on. Catching it needs a capture taken inside that window rather than
/// another trip, so the plant stays and this is a gap named rather than an errand outstanding
/// (NOTES § D40).
#[test]
fn a_container_a_gang_restart_has_parked_is_not_sampled_for_the_restart_card() {
    let parked = |reason: &str| {
        capture_but("restarts10serving", |p| {
            let c = container_status(p, "flaky");
            c.state = waiting_at(
                reason,
                Some("The container is removed because RestartAllContainers in place"),
            );
            c.ready = false;
            c.started = Some(false);
        })
    };

    let gang = parked(RESTART_ALL);
    let c = container(&gang, "flaky");
    println!("{c:?}");
    assert!(
        c.restarts >= RESTARTS_CRITICAL && !doing_its_job(c),
        "the plant keeps the count past the red band and takes the container out of service — \
         which is exactly the pair that used to flip the severity: {c:?}"
    );
    assert!(
        restarting_repeatedly(&now(), &gang, c).is_none(),
        "the rule is asked directly, so the silence is this clause and not a card that lost its \
         way to the screen"
    );
    // And through `analyze`, where the screen is: no restart card at all on this container.
    // **Rule 6's card is still beside it and is left alone** — this capture's previous run is a
    // genuine `exit 1`, not the synthesized record a firing writes, so *the previous run failed*
    // is true of it and its own exemption is a different question (NOTES § D93).
    let all = analyze(&pods_at(vec![gang], now()));
    show(&all);
    assert!(
        all.iter().all(|f| !f.title.contains("restarted")),
        "a container parked two seconds into a restart the pod itself asked for is not this \
         rule's subject — and least of all a CRITICAL that was a WARN a second ago \
         (NOTES § D100): {:?}",
        titles(&all)
    );

    // **The control on the reason.** `ContainerCreating` is what the kubelet writes for the same
    // container between ordinary restarts, and that shape is still one the count speaks for.
    let creating = parked("ContainerCreating");
    let all = analyze(&pods_at(vec![creating], now()));
    show(&all);
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    assert_eq!(
        card.severity,
        Severity::Critical,
        "ten restarts on a container that is not serving is the red band, and the exemption \
         above may not reach it — otherwise the silence is `waiting` and not the reason"
    );
    assert!(
        !card.title.contains("it is serving now"),
        "and the card is the down one: {}",
        card.title
    );

    // **And the flip itself, which is what the exemption is for**: the same object, seconds
    // earlier, is a WARN that says the container is serving. Without the clause the screen shows
    // these two alternately, about one container that never changed.
    let running = only(
        &analyze(&serving_at(pod("restarts10serving"), "flaky")),
        "broken-restarts10serving",
        "restarted 10 times",
    )
    .clone();
    assert_eq!(
        (
            running.severity,
            running.title.contains("it is serving now")
        ),
        (Severity::Warn, true),
        "the same container between two parkings: {}",
        running.title
    );
}

/// **Rule 6's two exemptions, on the two captures the 2026-08-13 trip was sent for.**
///
/// `exit_code == 0 || exit_code == 143` is the clause, and until those objects existed neither
/// half had a fixture: every committed previous run ended `1` or `137`, so deleting either
/// term left the suite green. `broken-exit0`'s batch job finishes cleanly and is restarted
/// forever by a `restartPolicy: Always` it should never have had — every CronJob written as a
/// Deployment — and `broken-sigterm` catches its SIGTERM and exits `143`, which is what a
/// rolling update and every scale-down look like.
///
/// **The container is not serving in either capture**, which is the half that matters: with a
/// running, ready container [`doing_its_job`] would be the clause doing the silencing and
/// these two would prove nothing about the exit codes (NOTES § D71).
#[test]
fn an_ordinary_exit_is_not_a_previous_run_that_failed() {
    for (name, pod_name, code) in [
        ("exit0", "broken-exit0", 0),
        ("sigterm", "broken-sigterm", 143),
    ] {
        let capture = pod(name);
        let c = capture
            .containers
            .first()
            .expect("the capture reports on its container");
        println!("{name}: {c:?}");
        assert_eq!(
            c.last_terminated.as_ref().map(|run| run.exit_code),
            Some(code),
            "{name}.json is the capture whose previous run ended {code}, or this test is \
             about a different exemption"
        );
        assert!(
            !doing_its_job(c),
            "{name}: and the container is not serving, so the silence below is the exit \
             code and not the suppressor beside it: {c:?}"
        );

        let all = findings(&[name]);
        show(&all);
        assert_eq!(
            all.iter()
                .filter(|f| f.title.contains("on record failed"))
                .count(),
            0,
            "{name}: exit {code} is what an ordinary shutdown looks like — a card here \
             fires on every rolling update in the cluster: {:?}",
            titles(&all)
        );
        // And the pod is not invisible meanwhile: the loop it is in is rule 1's card, so
        // the silence above is one rule standing down rather than a pod nothing looks at.
        only(&all, pod_name, "CrashLoopBackOff");
    }
}

/// **The first door of a clean exit, checked wherever one is drawn** — one function so that
/// four cards cannot drift, called once per caller because a shared sentence owes each caller a
/// pin of its own (NOTES § D88).
///
/// **What it requires is the killer and not the probe.** *Check the events for a probe kill* is
/// a door only a probe fits through, and the readings underneath it hang off *if nothing stopped
/// it* — so a reader whose container was stopped by `earlyoom` on a pressured node, or by the
/// kubelet performing an in-place resize with `resizePolicy: RestartContainer`, finds no probe,
/// closes the door and is told their program is quitting early. **`Killing` is the line that
/// proves a kill**, recorded by `killContainer` whatever asked for the stop; **`Unhealthy` is
/// not**, because a failing *readiness* probe writes it with nothing killed behind it, and a
/// reader who greps the word the card gave them would close the door on the wrong evidence.
/// **The node is the second place named**, for the killer that records nothing at all.
fn names_the_killer_and_not_only_the_probe(action: &str) {
    for door in ["Killing", "node"] {
        assert!(
            action.contains(door),
            "the reader is sent to *{door}*, or every killer that is not a probe has no door on \
             this card and the one conditional it offers is closed on the wrong evidence: \
             {action}"
        );
    }
    assert!(
        !action.contains("Unhealthy"),
        "and not to the event a failing readiness probe writes with nothing killed behind it — \
         a reader who finds `Unhealthy: Readiness probe failed` and reads it as the kill has been \
         handed a false positive by their own tool: {action}"
    );
}

/// **The order the doors are offered in, checked wherever a clean exit is drawn** — one function
/// for [`names_the_killer_and_not_only_the_probe`]'s reason, called once per caller because a
/// shared sentence owes each caller a pin (NOTES § D88).
///
/// `verdict` is what the arm settles on once nothing outside the container is left to blame: the
/// workload for a plain container meant to finish, *quitting early* for one that is not,
/// *finishing at all is the bug* for a sidecar. Each is a reading of one `exit 0`, which names an
/// ending and never an agent — so each hangs off a conditional, and an arm stating one ahead of
/// that conditional has settled from a single exit code what it has just said a single exit code
/// cannot settle.
///
/// **`conditional` is the caller's since 2026-08-16, because there are two of them now**
/// (NOTES § D113). The arms drawn about a run longer than [`PROBE_FLOOR`] hang their verdicts off
/// *if nothing did*; the short-run arms lead with the program and hang them off *if it ends
/// itself*, which is the ordering box's whole content. Matching *any* `if` would have made this
/// guard pass on an arm that asserted the verdict outright and mentioned a condition somewhere
/// else, so the anchor is named at the call site and read here.
fn the_verdict_hangs_off_the_conditional(action: &str, conditional: &str, verdict: &str) {
    let opened = action.find(conditional).unwrap_or_else(|| {
        panic!("the readings below it hang off {conditional:?}, which is not here: {action}")
    });
    let settled = action
        .find(verdict)
        .unwrap_or_else(|| panic!("this arm settles on {verdict:?}, and does not: {action}"));
    assert!(
        opened < settled,
        "*{verdict}* hangs off *{conditional}*, because the snapshot cannot say which reading \
         happened — one offered before the condition is stated is the verdict this round removed, \
         rebuilt one clause along: {action}"
    );
}

/// **The three probes `validateInitContainers` rejects, and the word itself** — matched against a
/// **lowercased** haystack everywhere they are used, because a sentence-initial *Probes are worth
/// checking* is the same forbidden advice and walked past the first draft of these guards
/// (NOTES § D31).
///
/// **"Everywhere" is the whole claim, and it was false for two days.** The two call sites over
/// [`exit_meaning`]'s raw return compared unlowered, and were only not a hole because the card
/// guard catches a capitalised `Probes` first — shadowed by a neighbour is not a state to keep,
/// so the sites were fixed rather than the sentence (NOTES § D93).
const PROBE_WORDS: [&str; 4] = ["liveness", "readiness", "startup", "probe"];

/// **The literal pointers at a log** the rules can put in front of a reader. A card whose own
/// evidence says nothing failed must not carry one, and the test that says so has to name them
/// rather than search for the word "log" — the new cards mention logs precisely to stop somebody
/// going there (NOTES § D85).
///
/// **One entry since 2026-08-16, and the other is [`THE_LOG_NO_COMMAND_REACHED`]** — a phrase no
/// card may say again rather than one no *clean-exit* card may say, so it moved out of this list
/// and into a negative over the whole corpus (NOTES § D113). The canary under this array is what
/// caught the change: it asserts every entry is still produced by some card, and this one stopped
/// being.
const SENT_TO_THE_LOGS: [&str; 1] = ["read the last run's log"];

/// **The sentence that used to be the first entry above, kept as a negative after nothing produced
/// it any more** (NOTES § D113). [`crash_looping`]'s [`Ending::Failed`](Ending::Failed) arm was its
/// only caller, and it said it under a card whose command is [`describe`], which prints no logs at
/// all — invariant 4 in the small, and worse on the `None` half beside it, where the record the
/// kubelet gates `logs --previous` on is the very record that is missing.
///
/// **It is not in [`SENT_TO_THE_LOGS`] because that list has a canary under it**: every entry there
/// has to still be produced by some card, or the negatives asserted against it are guarding
/// nothing. This one is the opposite — a phrase no card may say again — so it is asserted absent
/// over the whole corpus instead, in
/// [`no_card_sends_a_reader_to_a_log_the_command_beside_it_cannot_reach`].
const THE_LOG_NO_COMMAND_REACHED: &str = "read the previous run's logs";

/// **Three ways into one state, and rule 1 called all three a crash** (NOTES § D85).
///
/// `CrashLoopBackOff` is what the kubelet says about *any* container it is backing off from
/// restarting, and how the previous run ended is the only thing that says which loop it is:
/// `broken-crashloop` panicked (`exit 1`), `broken-exit0`'s batch program finished (`exit 0`)
/// and `broken-sigterm` was stopped by something outside it (`exit 143`). Until the trip of
/// 2026-08-13 only the first of the three existed in this repository, so the one title fitted
/// every capture there was.
///
/// **The control is `broken-crashloop`, asserted word for word**: this box changes what k8rs
/// says about the two clean exits and nothing about the crash it was already right about.
#[test]
fn the_three_ways_into_a_restart_loop_do_not_get_the_same_card() {
    let loops = [
        ("crashloop", "broken-crashloop"),
        ("exit0", "broken-exit0"),
        ("sigterm", "broken-sigterm"),
    ];
    let sets: Vec<Vec<Finding>> = loops.iter().map(|(name, _)| findings(&[name])).collect();
    let cards: Vec<(&str, &Finding)> = sets
        .iter()
        .zip(loops)
        .map(|(all, (name, pod_name))| {
            show(all);
            let capture = pod(name);
            let c = capture
                .containers
                .first()
                .expect("the capture reports on its container");
            assert!(
                matches!(waiting(c), Some(("CrashLoopBackOff", _))),
                "{name}.json has to be in the state rule 1 fires on, or this is a comparison \
                 of three cards from three different rules: {c:?}"
            );
            (name, only(all, pod_name, "CrashLoopBackOff"))
        })
        .collect();

    for (i, (name, card)) in cards.iter().enumerate() {
        for (other, other_card) in cards.iter().skip(i + 1) {
            assert_ne!(
                card.title, other_card.title,
                "{name} and {other} ended their last run differently, and one sentence \
                 covering both is the sentence that is false about one of them"
            );
            assert_ne!(
                card.action, other_card.action,
                "{name} and {other}: the next step differs with the ending, which is the \
                 whole of why the title does"
            );
        }
        assert_eq!(
            card.severity,
            Severity::Critical,
            "{name}: the colour answers *is this container serving*, and a container the \
             kubelet is backing off from is not — an amber card beside a red \
             CrashLoopBackOff in `kubectl get pods` teaches the reader to believe the other \
             tool (NOTES § D2)"
        );
    }

    // By name, not by position: the control is `broken-crashloop` whatever order the list
    // above is written in.
    let crash = cards
        .iter()
        .find(|(name, _)| *name == "crashloop")
        .expect("the crash itself is one of the three")
        .1;
    assert_eq!(
        crash.title, "Container keeps crashing, and each restart waits longer (CrashLoopBackOff)",
        "the control: a container that really is crashing keeps the card it always had"
    );
    // **The one card of the three whose action is no longer its own** (NOTES § D113). It said
    // *read the previous run's logs* under a command that prints none, and rule 5 answered the
    // same ending on the same container by sending the reader to the limit and the probe — one
    // ending, two rules, two answers, which is NOTES § D85's class. The sentence is
    // [`failed_run_action`]'s now, shared with rules 5 and 6 rather than written a second way.
    assert_eq!(
        crash.action,
        failed_run_action(&exited_run(1), ContainerRole::Regular).0,
        "and the crash arm answers with the sentence rule 6 gives the same ending on the same \
         container — one ending, one answer (NOTES § D85, § D113)"
    );
    assert!(
        !crash.action.contains(THE_LOG_NO_COMMAND_REACHED),
        "the log it used to name is not in any output this card's command produces: {}",
        crash.action
    );
}

/// **Rule 1's two shapes with no ending to read, and the log neither of them may name**
/// (NOTES § D113).
///
/// **The arm was `Failed | None` and the two halves are not the same question.** With a failed run
/// on the record the log exists and the card's own command does not show it — invariant 4 in the
/// small. With **no** record there is no `lastState.terminated.containerID` either, and the kubelet
/// gates `kubectl logs --previous` on exactly that field: the card was in that arm *because* the
/// flag its advice implied could not work, so the API answers `previous terminated container …
/// not found`.
///
/// **The `None` half takes rule 5's own sentence rather than a third framing** ([`no_record_action`]).
/// That rule was rewritten for this shape in its own box while rule 1's fall-through, ten lines
/// away, was not.
///
/// **And the title goes with it.** *Container keeps crashing* is a claim about runs the pod no
/// longer holds, on a card whose count can be `0` — `CrashLoopBackOff` is the wait *before* the
/// next start.
///
/// **Asserted absent over the whole corpus and not only here**, because the phrase is exactly the
/// kind that grows back one arm over: [`SENT_TO_THE_LOGS`]' canary proves its two surviving
/// entries are still produced, and this proves the third is produced by nothing.
#[test]
fn no_card_sends_a_reader_to_a_log_the_command_beside_it_cannot_reach() {
    // The shape the arm exists for: `CrashLoopBackOff` with the run dropped off the status.
    // Container GC below the per-container keep, a runtime that lost its store, a hand-run
    // `crictl rm` — the producers rule 5's own `None` arm carries.
    let forgotten = capture_but("crashloop", |p| {
        container_status(p, "quitter").last_state = None;
    });
    let c = container(&forgotten, "quitter");
    assert!(
        c.last_terminated.is_none() && matches!(waiting(c), Some(("CrashLoopBackOff", _))),
        "the plant has to remove the run and keep the wait, or this is not the arm: {c:?}"
    );
    let all = analyze(&pods_at(vec![forgotten], now()));
    show(&all);
    let card = only(&all, "broken-crashloop", "CrashLoopBackOff");
    assert_eq!(
        card.action,
        no_record_action(),
        "the same sentence rule 5 gives this shape, because two rules reading one container may \
         not answer it two ways (NOTES § D85)"
    );
    assert!(
        !card.title.to_lowercase().contains("crashing"),
        "the pod no longer holds a run that crashed, and `CrashLoopBackOff` can sit on a count of \
         0: {}",
        card.title
    );
    assert!(
        card.title.contains("CrashLoopBackOff"),
        "and the word the reader saw in `kubectl get pods` is still on the card: {}",
        card.title
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-crashloop -n default"),
        "the events are what may still hold the answer, and `describe` is what prints them"
    );

    // **The negative, over every shape the rule set reaches** — the captures, every planted
    // ending on every role, and the two no-record plants. A phrase removed from one arm is a
    // phrase that comes back in another, and only a sweep says so (NOTES § D29).
    let mut swept = analyze(&fixture_snapshot());
    swept.extend(analyze(&pods_at(every_shape_a_container_reaches(), now())));
    swept.extend(analyze(&pods_at(
        vec![
            capture_but("crashloop", |p| {
                container_status(p, "quitter").last_state = None;
            }),
            capture_but("restarts10", |p| {
                container_status(p, "flaky").last_state = None;
            }),
        ],
        now(),
    )));
    println!("{} cards swept", swept.len());
    assert!(
        swept.len() > 50,
        "{} cards is a sweep that stopped reaching the rule set",
        swept.len()
    );
    for f in &swept {
        assert!(
            !f.action.contains(THE_LOG_NO_COMMAND_REACHED),
            "{THE_LOG_NO_COMMAND_REACHED:?} is on a card again, and `--previous` is gated on a \
             field the arm that says it cannot have: {f:#?}"
        );
    }
    // **The flag and its explanation are one thing, checked in both directions over the same
    // sweep** (invariant 4, invariant 14). `--previous` is the one word on these cards a reader in
    // their first month cannot guess — it is the difference between the log of the run that failed
    // and the log of the one running now — so the card that hands it over says what it does; and
    // an action that talks about the flag without the command under it is the defect this test is
    // named for, one word smaller.
    let mut handed_over = 0;
    for f in &swept {
        let commanded = f
            .kubectl_cmd
            .as_deref()
            .is_some_and(|c| c.contains("--previous"));
        assert_eq!(
            commanded,
            f.action.contains("--previous"),
            "a command carrying --previous owes the reader what the flag does, and an action \
             naming it owes the command that runs it: {f:#?}"
        );
        handed_over += usize::from(commanded);
    }
    assert!(
        handed_over > 0,
        "no card in the sweep hands over --previous, so the pairing above is guarding nothing \
         (CLAUDE.md § A derived list asserts it found something)"
    );

    // And the positive beside it, or the sweep above passes on a rule set that stopped drawing
    // (CLAUDE.md § A derived list asserts it found something).
    assert!(
        swept.iter().any(|f| f.action == no_record_action()),
        "the sentence that replaced it is drawn by something, or this test guards a phrase \
         nothing was going to say"
    );
}

/// **A batch program that finished, restarted forever — the commonest way a Job is mis-written,
/// and the card that told its author their container was crashing** (NOTES § D85).
///
/// The kubelet *does* apply `CrashLoopBackOff` to a container that exits `0` under
/// `restartPolicy: Always`, so the state on the card is real and rule 1 is right to have
/// noticed. Everything the old card said about it was not: the title claimed a crash over an
/// evidence line reading `exit 0`, and the action sent the reader to a log that says the
/// program finished.
///
/// **Its second round is the action's own two readings** (NOTES § D88): the shipped card closed
/// on *meant to finish* or *quitting early*, which is a pair, and a probe kill on a program that
/// shuts down tidily is the door it did not have. **The round after that put the pair's second
/// half back**, because the first fix permuted the three rather than adding to them and left an
/// `nginx` with no `daemon off;` — `exit 0` in under a second, same object as this capture —
/// being told to move a web server into a CronJob. So the assertions below are about all three
/// doors being open at once, about none of them being asserted shut, and about the command that
/// lets the reader close the first one themselves.
#[test]
fn a_program_that_finished_is_not_a_container_that_crashed() {
    let raw = fixture("exit0");
    let capture = pod("exit0");
    let c = capture
        .containers
        .first()
        .expect("the capture reports on its container");
    println!("{c:?}");
    assert_eq!(
        c.last_terminated.as_ref().map(|run| run.exit_code),
        Some(captured_i32(
            captured_status(&raw, "containerStatuses", &c.name),
            &["lastState", "terminated", "exitCode"]
        )),
        "the exit code comes off the capture, and it is what makes this card's subject a \
         program that finished rather than one that failed"
    );

    // **The capture's own run is 2s, which is the short arm** — and this test therefore has to
    // read both orders or it stops covering the one it used to (NOTES § D113). The long shape is
    // the same capture with `startedAt` moved back past [`PROBE_FLOOR`]: one field, and the field
    // the order is read from.
    let lengthened = capture_but("exit0", |p| {
        let run = container_status(p, "batch")
            .last_state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the capture records the run that ended");
        run.started_at = run
            .finished_at
            .clone()
            .map(|t| Time(t.0 - SignedDuration::from_mins(5)));
    });
    assert!(
        run_length(
            container(&lengthened, "batch")
                .last_terminated
                .as_ref()
                .expect("the plant kept the run")
        ) > Some(PROBE_FLOOR)
            && run_length(
                c.last_terminated
                    .as_ref()
                    .expect("the capture records the run that ended")
            ) < Some(PROBE_FLOOR),
        "the two shapes have to sit either side of the threshold, or this test measures one arm \
         twice"
    );

    let short = findings(&["exit0"]);
    let long = analyze(&pods_at(vec![lengthened], now()));
    for all in [&short, &long] {
        show(all);
        assert_eq!(
            all.len(),
            1,
            "rule 1 alone — nothing failed here, so rule 6 has nothing to add: {:?}",
            titles(all)
        );
    }
    let short_card = only(&short, "broken-exit0", "CrashLoopBackOff").clone();
    let card = only(&long, "broken-exit0", "CrashLoopBackOff");

    assert!(
        !card.title.to_lowercase().contains("crashing"),
        "nothing crashed: this container ran to the end of its program and exited 0, and a \
         title that says otherwise is contradicted by the evidence line under it: {}",
        card.title
    );
    assert!(
        card.title.contains("CrashLoopBackOff"),
        "and the word the reader saw in `kubectl get pods` is still on the card, or the \
         card is about a pod they cannot find: {}",
        card.title
    );
    // **The absolute the snapshot cannot support.** `CrashLoopBackOff` is entered on
    // *accumulated* backoff and one clean run does not reset the key, so exit 1 four times
    // and then a fifth run that exits 0 leaves exactly this shape — and *nothing has crashed*
    // beside `16 restarts` tells the reader the fifteen restarts before this one were clean
    // too. One `lastState` is one run.
    assert!(
        !card.title.contains("nothing has crashed"),
        "the snapshot holds one run and the title claimed the whole loop: a container that \
         failed four times and then exited 0 is in this state with this `lastState`: {}",
        card.title
    );
    assert!(
        card.title.contains("last run on record"),
        "so the title says which run it is talking about — the last one Kubernetes wrote down, \
         which is not the same as the container's last run: {}",
        card.title
    );
    assert!(
        card.evidence
            .contains("exit 0 (the run ended without an error)"),
        "invariant 14: `0` is translated like every other code, and printing it bare under a \
         card about crashing is how the contradiction reached the screen unremarked — and the \
         translation names the ending only, because the action one line under it is five lines \
         about the code naming no agent (NOTES § D88): {}",
        card.evidence
    );
    for pointer in SENT_TO_THE_LOGS {
        assert!(
            !card.action.contains(pointer),
            "the previous run's log says the program finished — twenty minutes of somebody \
             proving their own tool wrong: {}",
            card.action
        );
    }
    // **The reading the shipped card was missing** (NOTES § D88): a liveness probe fails, the
    // kubelet sends SIGTERM, the program traps it and shuts down tidily, and the kubelet writes
    // `exit 0` for a run something else ended. The clause below is one this rule's clean-exit
    // sentence and rule 5's share, and each caller owes it a pin of its own: until this line,
    // deleting it from [`finished_action`] took rule 5's tests red alone, while both rules' cards
    // lost the reading.
    assert!(
        card.action.contains("does not say who ended the run"),
        "an exit code is the status a process ended with and never a statement about who ended \
         it — a program stopped by a failing probe reports 0 like one that chose to stop, and a \
         card that does not say so has picked one of two readings it cannot tell apart: {}",
        card.action
    );
    // **And the door it opens is the killer, not the probe** — this rule's own pin on the
    // shared clause, and the reasoning is at the function. **On both orders**: the short arm
    // demotes these two doors and does not close them (NOTES § D113).
    names_the_killer_and_not_only_the_probe(&card.action);
    names_the_killer_and_not_only_the_probe(&short_card.action);
    assert!(
        card.action.contains("Job") && card.action.contains("CronJob"),
        "and the fix for a program that is meant to finish is the workload that lets it — the \
         one thing only this role's arm says, where the two above are shared with the sidecar's \
         (NOTES § D88): {}",
        card.action
    );
    // **The third door, and the proof that it is a door.** *Meant to finish* and *quitting early*
    // are a real pair once something else has been ruled out, and the first fix for the missing
    // probe reading deleted the second half of it rather than adding to it — leaving an `nginx`
    // with no `daemon off;`, which is this capture's shape exactly, sent after events that hold
    // nothing and then told to become a CronJob. So the words are not the defect: their position
    // is. Both readings have to sit *after* the conditional, or the card has settled from one
    // exit code what one exit code cannot say (NOTES § D88).
    assert!(
        card.action.contains("quitting early"),
        "a program that is not meant to finish and stops anyway is the commonest of the three in \
         a real cluster, and the card that drops it can only offer a batch workload to the author \
         of a web server: {}",
        card.action
    );
    the_verdict_hangs_off_the_conditional(&card.action, "If nothing did", "Job");
    the_verdict_hangs_off_the_conditional(&card.action, "If nothing did", "quitting early");
    // **The command has to show what the action names** (invariant 4), and this arm's action now
    // names the pod's events: a probe kill is written into `Unhealthy` / `Killing` and nowhere
    // else this card can reach. `get -o yaml` prints no events at all, so the command moved with
    // the sentence — and the `restartPolicy` the old action named went with it, being a field
    // the state already implies (a container backing off from a clean exit is under a policy
    // that restarts one) and the one thing `describe` does not print.
    assert!(
        !card.action.contains("restartPolicy"),
        "an action may not name a field its own command hides — that is the failure the trade \
         between these two commands exists to avoid, not a corner of it: {}",
        card.action
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-exit0 -n default"),
        "an action naming the events owes the one command that prints them"
    );

    // --- THE ORDER, ON THE CAPTURE THIS WHOLE LINE OF BOXES DESCENDS FROM ---
    //
    // **What the short arm is for** (NOTES § D113): with stock probe settings the earliest a
    // health check can kill is [`PROBE_FLOOR`], and this capture's evidence line says `ran for 2s`
    // one row above. The reader's first move on the long arm is to go and prove the first door
    // shut. Three things the arm may not do, all asserted:
    println!("short: {}\nlong:  {}", short_card.action, card.action);
    assert_ne!(
        short_card.action, card.action,
        "the two sides of the threshold get different sentences, or nothing is ordered at all"
    );
    // **1. It reorders and never deletes.** The object makes a probe kill unlikely; it does not
    // prove one did not happen, so every door open on the long arm is open here.
    for door in [
        "Killing",
        "node",
        "memory killer",
        "Job",
        "CronJob",
        "quits early",
    ] {
        assert!(
            short_card.action.contains(door),
            "*{door}* is a door the long arm opens, and a short run is not a reason to close it: \
             {}",
            short_card.action
        );
    }
    // **2. The verdict is still conditional.** What moved is which condition it hangs off, not
    // whether it hangs off one: an arm that asserted *it quits early* would have settled from one
    // exit code what one exit code cannot say (NOTES § D88).
    the_verdict_hangs_off_the_conditional(&short_card.action, "if it ends itself", "Job");
    the_verdict_hangs_off_the_conditional(&short_card.action, "if it ends itself", "quits early");
    // **3. The reason for the order is on the card**, and so is the fact it is read from — the
    // evidence line one row above carries the duration this rule ordered by, so a reader can see
    // why the doors arrive in the order they do. A hidden reason for a visible order is worse than
    // no order at all.
    assert!(
        short_card.action.contains("run this short"),
        "the card says why the program comes first, or the order is an unexplained rearrangement: \
         {}",
        short_card.action
    );
    assert!(
        short_card.evidence.contains("ran for"),
        "and the fact it ordered by is on the card the reader is looking at: {}",
        short_card.evidence
    );
    assert!(
        short_card.action.find("start with the program") < short_card.action.find("A Killing line"),
        "the program comes first and the kill doors after it, which is the whole of the change: {}",
        short_card.action
    );
    assert!(
        card.action.find("Killing line") < card.action.find("it ends itself"),
        "and the long arm keeps the order it had, or the threshold decides nothing: {}",
        card.action
    );

    // **A caller with no duration to show gets the unordered sentence**, which is the `None`
    // side of the helper's own contract. Both rules that draw this ending print one now
    // ([`restarting_repeatedly`] since 2026-08-16, NOTES § D113), so the `None` arm is asserted
    // on the helper rather than through a card — and it stays, because it is what the parameter
    // being an `Option` *means*: a card that shows no duration may not order by one.
    assert_eq!(
        finished_action(ContainerRole::Regular, None),
        card.action,
        "a caller whose card shows no duration gets the unordered sentence"
    );
}

/// **A native sidecar that exits cleanly, and the card that told a Job to become a Job**
/// (KEP-753, NOTES § D85).
///
/// Pod `restartPolicy: Never`, `initContainers[].restartPolicy: Always`: the sidecar exits `0`,
/// the kubelet restarts it — upstream's `SyncPod` runs init containers through the same
/// `doBackOff` closure as regular ones — and `Init:CrashLoopBackOff` with
/// `lastState.terminated.exitCode: 0` is what `kubectl get pods` then shows. The fix for
/// [`crash_looping`]'s `exit 0` branch was written for the pod-level `Always` disjunct alone,
/// so on this object it said *it belongs in a Job or a CronJob* one line under an evidence
/// line reading *it runs beside the app the whole time*, about a pod that already **is** a Job.
///
/// **`healthy-sidecar.json` carries the role and the plant supplies the run** — `proxy` is an
/// init container with `restartPolicy: Always`, currently `Running`, and the role is the field
/// this card turns on. The plant moves `state`, the pod's restart policy and the previous run,
/// and touches nothing else (NOTES § D53).
///
/// **The clean previous run was the capture's own until 2026-08-16** (NOTES § D114). `proxy` is
/// `sleep 3600`, so the `restartCount: 1` and `lastState.terminated.exitCode: 0` the old fixture
/// carried were the first hour of the capture session elapsing — and
/// `scripts/cluster.sh` § `[healthy_sidecar]` asks only for a `Running` pod, all containers
/// ready, and a `restartPolicy: Always` init container. A trip that captures in half an hour
/// brings a sidecar that has never finished, which is what the 2026-08-16 one did. So the run is
/// planted rather than read, and the role — the part that actually decides this card — is still
/// the capture's.
#[test]
fn a_sidecar_that_exits_cleanly_is_not_told_to_move_to_the_workload_it_is_already_in() {
    let job = capture_but("healthy-sidecar", |pod| {
        pod.spec
            .as_mut()
            .expect("the capture has a spec")
            .restart_policy = Some("Never".to_string());
        // Before `backing_off`, which rewrites `state`: this writes the run the kubelet would
        // have left behind and the restart it counts for it.
        ended_as(pod, "proxy", 0, None, None);
        backing_off(pod, "proxy");
    });
    let proxy = container(&job, "proxy");
    println!("{proxy:?}");
    assert_eq!(
        proxy.role,
        ContainerRole::Sidecar,
        "the whole card turns on the role, and a capture whose `restartPolicy: Always` had \
         been dropped would decode as a plain init container and prove nothing"
    );
    assert_eq!(
        proxy.last_terminated.as_ref().map(|run| run.exit_code),
        Some(0),
        "the ending under test is a *clean* one — that is the whole subject, and a plant that \
         built anything else would take a different arm of the rule"
    );
    assert_eq!(
        proxy
            .last_terminated
            .as_ref()
            .and_then(|run| run.reason.as_deref()),
        Some("Completed"),
        "and the reason the API writes beside a zero, or the plant is an object no kubelet \
         produces (NOTES § D40)"
    );
    assert!(
        proxy.restarts > 0,
        "a container with a previous run and no restart to go with it is a pair no kubelet \
         writes either: {}",
        proxy.restarts
    );

    let all = analyze(&pods_at(vec![job], now()));
    show(&all);
    let card = only(&all, "healthy-sidecar", "CrashLoopBackOff");
    assert!(
        card.evidence
            .contains("it runs beside the app the whole time"),
        "the evidence line the action has to agree with: {}",
        card.evidence
    );
    assert!(
        !card.action.contains("Job") && !card.action.contains("CronJob"),
        "this pod is a Job, its restartPolicy is Never, and the container above runs beside \
         the app for the pod's whole life — telling its author to move it to a Job or a \
         CronJob is the contradiction this box exists to remove, rebuilt inside the fix \
         for it: {}",
        card.action
    );
    assert!(
        card.action.contains("events"),
        "and the reader is still left with somewhere to look — an action that only says \
         where *not* to look is no action at all: {}",
        card.action
    );
    // **And what to look for once they are open.** *Check the events* with nothing named is the
    // instruction that leaves the reader where they started, and the line that names one is
    // this arm's first door — pinned here for this role as well as on the plain container above,
    // because arm-level coverage is per-caller (NOTES § D88).
    names_the_killer_and_not_only_the_probe(&card.action);
    // The shared clause this role's card depends on, pinned here as well as in rule 5's tests:
    // arm-level coverage is per-caller, clause-level was not, and deleting this one upstream
    // took only the other rule red (NOTES § D88).
    assert!(
        card.action.contains("does not say who ended the run"),
        "a sidecar that shuts down tidily on SIGTERM reports 0 like one that chose to stop, so \
         the card may not read this 0 as a decision the container made: {}",
        card.action
    );
    // **The positive half of the split, which none of its negatives can stand in for**
    // (NOTES § D88). Every assertion above is satisfied word for word by the *init* arm's
    // sentence — it names no Job and it does name the events — so all three survive the split
    // collapsing in the one direction the test above cannot see. This is the sentence only this
    // arm carries: a container that runs beside the app for the pod's whole life is not
    // finishing early, it is finishing at all.
    assert!(
        card.action.contains("finishing at all is the bug"),
        "a sidecar is the one role where ending cleanly is itself the fault, and a card that \
         stops short of saying so has told the reader only what the problem is not: {}",
        card.action
    );
    // **And that verdict hangs off the reading the events settle, exactly as the plain
    // container's two do** (NOTES § D88). The card opens by saying this `0` cannot name who ended
    // the run and closes by calling finishing the fault. With no conditional between the two it
    // names a probe kill and then rules it out one sentence later, off the same single exit
    // code — the defect this box removed from the plain-container arm, rebuilt one arm over.
    the_verdict_hangs_off_the_conditional(
        &card.action,
        "If nothing did",
        "finishing at all is the bug",
    );
    // The same string rule 5 hands this role, and its own pin on it: a shared sentence owes
    // each caller a pin, or splitting the helper again strips one rule's only coverage with
    // nothing going red (NOTES § D88).
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod healthy-sidecar -n default"),
        "an action naming the events owes the one command that prints them (invariant 4)"
    );
}

/// **A plain init container that finished its work and is being backed off — the arm the last
/// box reported unreachable** (NOTES § D88), and rule 1's third role on the clean ending.
///
/// **It is not reachable in the published pod status, and that is settled rather than pending**
/// (NOTES § D114). D88 and D90 argued it *was*, from the source: `doBackOff` keys on the
/// container's name and never reads the exit code, and `SyncPod` runs init containers through the
/// same closure as regular ones — so the backoff entry `wait-for-db` earned by failing three
/// times is still live when the run after it succeeds, and rebuilding the sandbox inside that
/// window should re-run it straight into a backoff nothing cleared.
///
/// **Measured on a live cluster, the first half happens and the second half never reaches the
/// API.** `doBackOff` does fire — the `BackOff` events prove it — but the kubelet publishes
/// `waiting: CrashLoopBackOff` only for an init container it is *waiting to retry*, and one that
/// exited `0` is finished, so the pair `Init:CrashLoopBackOff` + `lastState.terminated.exitCode:
/// 0` is never written. **~120 samples, zero hits**, with the control on the same cluster in the
/// same minute: `broken-init`, which exits `1`, shows the waiting state exactly as expected.
///
/// **So this arm stays planted permanently**, on the same footing as the CRI-O `-1` spelling
/// ([`CODE_UNKNOWN`], NOTES § D40, § D53) — a shape the code must answer correctly and no capture
/// trip can ever bring. It is not an errand outstanding, and a future trip owes nothing here.
///
/// The rebuild reasoning below is kept because it is what the plant *builds*, and it was right
/// about the kubelet's behaviour even though the status never shows it: `kl.backOff` is built
/// in-process by `NewMainKubelet`, so a node reboot is not one of these — a kubelet that has just
/// started has an empty map. The window belongs to a sandbox that dies while the kubelet does
/// not: a CNI or sandbox flap recorded as `SandboxChanged`, a `crictl rmp`, a container-runtime
/// restart (NOTES § D88).
///
/// **`healthy-retry.json` is one rebuild away, and the plant is that rebuild**: the capture's own
/// successful run moves from `state` to `lastState`, which is exactly where the kubelet puts it
/// when it runs the container again, and `state` becomes the wait. So the clean exit under test
/// is the capture's and not the plant's (NOTES § D53), and the assertion below reads its end
/// stamp back off the JSON as well as its code — the code alone is a `0` a hand-written plant
/// would satisfy, which proves the two values equal and nothing about where either came from.
///
/// **The rest of the pod goes back with it** ([`sandbox_rebuilt`], [`never_ran`]): `phase:
/// Pending`, `Initialized: False`, and the app waiting on `PodInitializing`, unready and
/// unstarted. An init container waiting to be restarted beside a **ready** app in a `Running`
/// pod is a shape no kubelet writes, and a plant is only worth the shape it builds
/// (NOTES § D40).
///
/// **Given the sidecar's sentence this card is wrong twice**, both NOTES § D85's own shape: it
/// calls finishing the bug one line under an evidence line reading *the app starts only after
/// this one finishes*, and it sends the reader after a probe [`stopped_action`] refuses to name
/// on this very container, one ending over in the same rule.
#[test]
fn a_plain_init_container_backing_off_after_a_clean_run_is_not_told_finishing_is_its_bug() {
    let rebuilt = capture_but("healthy-retry", |p| {
        let succeeded = container_status(p, "wait-for-db")
            .state
            .clone()
            .and_then(|s| s.terminated)
            .expect(
                "the capture's init container ran to the end, and that run is the one a \
                     rebuilt sandbox pushes down into `lastState`",
            );
        container_status(p, "wait-for-db").last_state = Some(ApiContainerState {
            terminated: Some(succeeded),
            ..ApiContainerState::default()
        });
        backing_off(p, "wait-for-db");
        never_ran(p, "app", "PodInitializing", None);
        sandbox_rebuilt(p);
    });
    let waiter = container(&rebuilt, "wait-for-db");
    println!("{waiter:?}");
    let raw = fixture("healthy-retry");
    let captured = captured_status(&raw, "initContainerStatuses", "wait-for-db");
    assert!(
        waiter.role == ContainerRole::Init
            && matches!(waiting(waiter), Some(("CrashLoopBackOff", _))),
        "the two facts of the current state that put this container in the arm under test: \
         {waiter:?}"
    );
    // **And the run behind it is the capture's, which the exit code alone cannot show**: `0` is
    // the value a hand-written plant would have chosen too, so both sides of that comparison
    // would be equal with nothing moved off the JSON. The stamp is what the plant did not pick
    // (NOTES § D53).
    assert_eq!(
        waiter
            .last_terminated
            .as_ref()
            .map(|r| (r.exit_code, r.finished_at.clone())),
        Some((
            captured_i32(captured, &["state", "terminated", "exitCode"]),
            Some(captured_time(
                captured,
                &["state", "terminated", "finishedAt"]
            )),
        )),
        "the run under test is the one the cluster wrote, moved down a field: {waiter:?}"
    );

    let all = analyze(&pods_at(vec![rebuilt], now()));
    show(&all);
    // **One card, and the plant's own shape is what could have added a second.** The rebuilt pod
    // is `Pending`, which rules 10 and 14 gate on, and its app is waiting on `PodInitializing`,
    // which is rule 13's subject: rule 10 leaves on a `PodScheduled` that is `True`, rule 14 on
    // its being there at all, and rule 13 because the init container beside it carries a reason
    // of its own to point at (NOTES § D29 — a shape is proven only once it has been fed).
    assert_eq!(
        all.len(),
        1,
        "rule 1 alone on a pod put back the way a rebuilt sandbox puts it: {:?}",
        titles(&all)
    );
    let card = only(&all, "healthy-retry", "CrashLoopBackOff");
    assert_eq!(
        card.severity,
        Severity::Critical,
        "the band answers *is this container serving*, and the app behind this one cannot even \
         start until it succeeds"
    );
    assert!(
        card.evidence
            .contains("the app starts only after this one finishes"),
        "the evidence line the action has to agree with — this container finishing is the \
         contract, not the fault: {}",
        card.evidence
    );
    assert!(
        !card.action.contains("bug"),
        "a plain init container is *meant* to finish, and a card calling that the bug argues \
         with its own evidence line: {}",
        card.action
    );
    for probe in ["liveness", "readiness", "startup"] {
        assert!(
            !card.action.contains(probe),
            "and Kubernetes rejects a {probe} probe on this kind of container, so naming one is \
             advice the reader cannot follow — and this rule's own `143` arm refuses to name it \
             on the same container, which is one rule contradicting itself: {}",
            card.action
        );
    }
    // **The two things only this arm says.** The negatives above do catch both siblings — the
    // sidecar's sentence dies on *the bug* and the plain container's on *probe*, each checked by
    // handing this card the other arm's string — but a negative only ever says what the branch is
    // not, and a reworded sentence that lost the sandbox would pass every one of them
    // (NOTES § D88).
    assert!(
        card.action.contains("sandbox"),
        "what ran a container that had already finished is the question, and the answer is not \
         inside it: Kubernetes re-runs every init container when it rebuilds the pod's sandbox. \
         An action that only says where *not* to look is no action at all: {}",
        card.action
    );
    // **And the record it points at expires** — `--event-ttl` defaults to an hour, and sending a
    // reader to an empty list without warning them is how a tool teaches them to stop believing
    // it. Pinned here as well as in rule 5's test: it is one shared sentence, and clause-level
    // coverage in one caller is coverage the other silently does without (NOTES § D88).
    assert!(
        card.action.contains("about an hour"),
        "the card says how long the record it points at lasts — and it dates it by the cluster's \
         own clock rather than by this card's restart count, which is not always on it: rule 1 \
         drops the count from the evidence line when it is `0`, and a sentence dated against it \
         would then be dated against nothing (NOTES § D88): {}",
        card.action
    );
    // **And after the hour is up the card still names a place.** The events are the record that
    // expires, so the clause that follows them is this arm's whole answer to a reader who arrives
    // late — what rebuilt the sandbox happened on the node and is recorded there, by the kubelet
    // and the container runtime, outside the hour the pod's events last. Ending on *the reason is
    // no longer recorded* closes the question the action opened with (NOTES § D88).
    assert!(
        card.action.contains("node"),
        "the action opens by asking what ran the container again and has to answer it for the \
         reader who reads this card an hour later too — an arm whose only pointer expires is a \
         dead end dressed as an instruction: {}",
        card.action
    );
    // **And the place comes after the expiry, or the two words are only both present.** The three
    // assertions above are satisfied by *"The node kept about an hour of events; after that
    // nothing is left"* — a card naming the sandbox, the hour and the node while telling the
    // reader that nothing survives them. The requirement is that what outlasts the record is
    // named *as* what outlasts it, and the order is what says so (NOTES § D88).
    the_place_outlasts_the_record(&card.action);
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod healthy-retry -n default"),
        "and the events that record a rebuilt sandbox, and the node the pod sits on, are both \
         in that one output (invariant 4)"
    );
}

/// **The init arm's durability requirement, checked wherever that arm is drawn** — one function
/// for the same reason [`names_the_killer_and_not_only_the_probe`] is one, and called once per
/// caller because a shared sentence owes each caller a pin (NOTES § D88).
///
/// **What it requires is a place that outlives the record.** The events expire — `--event-ttl`
/// defaults to an hour — and they routinely never carried the reason in the first place, so the
/// card names the node *after* the hour it gives the events: the node is what is still there for
/// the reader who arrives late, and for the one whose events never said why.
fn the_place_outlasts_the_record(action: &str) {
    let hour = action.find("about an hour").unwrap_or_else(|| {
        panic!("the record the card points at expires, and it says so: {action}")
    });
    let place = action
        .find("node")
        .unwrap_or_else(|| panic!("and something outlasts it: {action}"));
    assert!(
        hour < place,
        "the place is named as what is left once the hour is up, not as the thing that keeps the \
         hour — *the node kept about an hour of events; after that nothing is left* carries every \
         word this arm owes and answers nobody: {action}"
    );
}

/// **Rule 1 in the window before the first restart, where the count is `0` and the card must not
/// print it.** `if c.restarts > 0` in [`crash_looping`] survived every mutation run up to this
/// test, in three different line positions: flipped to `>= 0` the rule ships **`0 restarts`** on a
/// real card and the other 223 tests stayed green, because no committed capture holds a
/// `CrashLoopBackOff` container whose count is still `0`. **This is the test that goes red.**
///
/// **`restartCount` is the number of runs that have been *started again*, not the number that have
/// ended.** The kubelet stamps it on the instance when it creates it, so the first instance carries
/// `0`, and `convertToAPIContainerStatuses` moves that instance's terminated status down into
/// `lastState` and writes the backoff into `state.waiting` without touching the count. Between the
/// first crash and the restart it is waiting to make, the API therefore publishes
/// `CrashLoopBackOff` beside `restartCount: 0` and a real failed run. The window is one backoff
/// wide and `just fixtures` photographs a pod that has been looping for minutes, which is why the
/// corpus does not hold one — the same standing reason [`CODE_UNKNOWN`]'s arm is planted
/// (NOTES § D40, § D53), and the rule's own doc says the count "can still be `0`" here.
///
/// **What the mutant ships is a fact line that argues with its title**: `0 restarts` under
/// *CrashLoopBackOff* reads as *nothing has crashed*, and a fact is printed only when it is a fact
/// (NOTES § v1 rule set, rule 1). The count is omitted, not printed as zero.
///
/// **The plant is the capture's own failed run, one restart earlier.** `healthy-retry`'s init
/// container already carries a real `exit 1 / Error` in `lastState` from the trip that captured
/// it; [`backing_off`] puts `state` back to the wait that record was written under, the count goes
/// to `0`, and the rest of the pod goes with it ([`sandbox_rebuilt`], [`never_ran`]) because an
/// init container backing off beside a **ready** app in a `Running` pod is a shape no kubelet
/// writes — a plant is only worth the shape it builds (NOTES § D40).
#[test]
fn a_container_backing_off_before_its_first_restart_is_not_told_it_has_zero_restarts() {
    let first_backoff = capture_but("healthy-retry", |p| {
        backing_off(p, "wait-for-db");
        container_status(p, "wait-for-db").restart_count = 0;
        never_ran(p, "app", "PodInitializing", None);
        sandbox_rebuilt(p);
    });
    let waiter = container(&first_backoff, "wait-for-db");
    println!("{waiter:?}");
    let raw = fixture("healthy-retry");
    let captured = captured_status(&raw, "initContainerStatuses", "wait-for-db");
    assert!(
        matches!(waiting(waiter), Some(("CrashLoopBackOff", _))) && waiter.restarts == 0,
        "the two facts that put this container in the window under test — the kubelet is waiting \
         between restarts, and it has not made the first one yet: {waiter:?}"
    );
    // **And the run it is backing off from is the cluster's, not the plant's** (NOTES § D53). The
    // exit code alone is a `1` a hand-written record would have chosen too, so the stamp beside it
    // is what says the run came off the capture.
    assert_eq!(
        waiter
            .last_terminated
            .as_ref()
            .map(|r| (r.exit_code, r.finished_at.clone())),
        Some((
            captured_i32(captured, &["lastState", "terminated", "exitCode"]),
            Some(captured_time(
                captured,
                &["lastState", "terminated", "finishedAt"]
            )),
        )),
        "the failed run under test is the one the cluster wrote, with only the wait around it \
         moved back: {waiter:?}"
    );
    assert_ne!(
        captured_i32(captured, &["restartCount"]),
        0,
        "and the count is the one field this plant moves — a capture that already read `0` would \
         make the line below true for free"
    );

    let all = analyze(&pods_at(vec![first_backoff], now()));
    show(&all);
    let card = only(&all, "healthy-retry", "CrashLoopBackOff");
    let facts: Vec<&str> = card.evidence.split(FACTS).collect();
    // **The requirement, both ways round.** A count of `0` is not a fact about restarts, so no
    // such fact is drawn; and the facts the object *does* support are all still there, or a rule
    // that had stopped drawing an evidence line at all would pass the negative on its own.
    assert!(
        !card.evidence.contains("restarts"),
        "`0 restarts` under a title saying this container is crash-looping reads as *nothing has \
         crashed* — the count is left out until there is one (NOTES § v1 rule set, rule 1): {}",
        card.evidence
    );
    // **And counted as well as searched**, because a zero fact worded any other way is the same
    // card. The duration is not spelled out here for the reason the counts are not: how long a
    // captured run lasted belongs to the cluster, and a literal would redden on a trip that
    // changed nothing this test is about.
    assert!(
        facts.len() == 3
            && facts[0].starts_with("init container wait-for-db")
            && facts.iter().any(|f| f.starts_with(&format!(
                "exit {}",
                captured_i32(captured, &["lastState", "terminated", "exitCode"])
            ))),
        "the three facts this object supports and no fourth — the container it is about, how long \
         the run lasted, and the code the cluster recorded for it: {facts:?}"
    );
}

/// **A count of one is not spelled with a plural noun** (invariant 14). `1 restarts` reached a
/// shipped card, and it reached it twice: rules 1 and 2 each hand-rolled `format!("{} restarts")`
/// where [`counted`] is the one place this file spells a counted noun. **Both are driven here,
/// because a fix to one leaves the other saying it** — one fact spelled twice is what made this a
/// defect rather than a typo (NOTES § D85).
///
/// **Rule 2's card is a committed capture read at a moment of its own.** `oomserving`'s container
/// carries `restartCount: 1` beside the kill it survived, and the rule draws inside its recency
/// grace — the same bytes `an_old_kill_on_a_container_that_has_been_fine_since_…` reads from the
/// other side.
///
/// **Rule 1's is planted, one field**, because the corpus holds no crash loop at one restart and
/// cannot: `just fixtures` photographs a pod that has been looping for minutes (NOTES § D40,
/// § D53). `restartCount: 1` is `crashloop.json` eight restarts earlier, and the wait and the
/// failed run around it are the cluster's own.
///
/// **The plural is asserted beside each**, or a fix that reads right at one and wrong at nine
/// passes on the half it was written for.
#[test]
fn a_container_that_has_restarted_once_is_not_told_it_has_one_restarts() {
    let killed = pod("oomserving");
    let app = container(&killed, "app");
    assert_eq!(
        app.restarts, 1,
        "the capture has to be the one-restart object, or this proves nothing about one: {app:?}"
    );
    let news = Time(
        app.last_terminated
            .as_ref()
            .and_then(|run| run.finished_at.as_ref())
            .expect("the capture records when the kernel took it")
            .0
            .checked_add(SignedDuration::from_mins(5))
            .expect("a moment after the captured kill"),
    );
    let all = findings_at(&["oomserving"], news.clone());
    show_at(&all, &news);
    let oom = only(&all, "broken-oomserving", "OOMKilled");
    assert!(
        oom.evidence.contains("1 restart") && !oom.evidence.contains("1 restarts"),
        "one restart is `1 restart` — a number glued to a plural noun is a format string \
         showing through, not a sentence anyone wrote (invariant 14): {}",
        oom.evidence
    );

    // Rule 1, at the same count, one restart before the capture was taken.
    let second_backoff = capture_but("crashloop", |p| {
        container_status(p, "quitter").restart_count = 1;
    });
    let looping = container(&second_backoff, "quitter");
    assert!(
        matches!(waiting(looping), Some(("CrashLoopBackOff", _))) && looping.restarts == 1,
        "the plant moves the count and leaves the wait it was written under: {looping:?}"
    );
    let captured = captured_i32(
        captured_status(&fixture("crashloop"), "containerStatuses", "quitter"),
        &["restartCount"],
    );
    assert_ne!(
        captured, 1,
        "and the count is the one field this plant moves — a capture already reading 1 would \
         make the assertion below true for free"
    );
    let all = analyze(&pods_at(vec![second_backoff], now()));
    show(&all);
    let looped = only(&all, "broken-crashloop", "CrashLoopBackOff");
    assert!(
        looped.evidence.contains("1 restart") && !looped.evidence.contains("1 restarts"),
        "rule 1 spells the same fact, so it spells it the same way: {}",
        looped.evidence
    );

    // **And the plural still reads as one**, off the untouched capture at nine. A helper that
    // pluralised nothing would pass both assertions above on its own.
    let untouched = findings(&["crashloop"]);
    let nine = only(&untouched, "broken-crashloop", "CrashLoopBackOff");
    assert!(
        nine.evidence.contains(&format!("{captured} restarts")),
        "the capture's own count keeps its plural: {}",
        nine.evidence
    );
}

/// **Rule 2's `if c.restarts > 0`, which is rule 1's guard one rule over and was never fed.**
/// `cargo mutants` reported `3240:19 -> >=` MISSED the first time the line beside it moved: with
/// the operator flipped, `0 restarts` ships on a real card and nothing goes red — the same
/// survivor `crash_looping`'s box closed on 2026-08-19, in the rule that copied the `format!`
/// (NOTES § D85's class: two rules spelling one fact, and only one of them proved).
///
/// **A fact is printed only when it is a fact** (NOTES § v1 rule set, rule 1): `0 restarts` under
/// a title saying the kernel killed this container reads as *and it has not happened again*, which
/// is the opposite of the window this shape is.
///
/// **The shape is the one before the first restart**, and it is the window
/// `a_container_backing_off_before_its_first_restart_…` documents: the kubelet moves the
/// terminated status down into `lastState` and writes the backoff into `state.waiting` without
/// touching `restartCount`, so the API publishes `CrashLoopBackOff` beside `restartCount: 0` and a
/// real kill. `just fixtures` photographs a pod that has been looping for minutes, which is why
/// the corpus holds no such object (NOTES § D40, § D53), and `oom.json`'s own kill is what the
/// plant is built around — the `OOMKilled` record it fires on is the cluster's.
#[test]
fn a_container_the_kernel_killed_before_its_first_restart_is_not_told_it_has_zero_restarts() {
    let first_kill = capture_but("oom", |p| {
        backing_off(p, "hog");
        container_status(p, "hog").restart_count = 0;
    });
    let hog = container(&first_kill, "hog");
    assert!(
        hog.restarts == 0
            && matches!(&hog.last_terminated, Some(run) if run.reason.as_deref() == Some("OOMKilled")),
        "the two facts that put this container in the window under test — the kernel has taken \
         it once and it has not been restarted yet: {hog:?}"
    );
    assert_ne!(
        captured_i32(
            captured_status(&fixture("oom"), "containerStatuses", "hog"),
            &["restartCount"]
        ),
        0,
        "and the count is the one field this plant moves, or the assertion below is true for free"
    );
    let all = analyze(&pods_at(vec![first_kill], now()));
    show(&all);
    let killed = only(&all, "broken-oom", "OOMKilled");
    assert!(
        !killed.evidence.contains("restart"),
        "`0 restarts` under a card about a kill reads as *and it has not happened again* — the \
         count is left out until there is one (NOTES § v1 rule set): {}",
        killed.evidence
    );
    // **And the facts the object does support are all still there**, or a rule that had stopped
    // drawing an evidence line at all would pass the negative on its own.
    let facts: Vec<&str> = killed.evidence.split(FACTS).collect();
    assert!(
        facts.len() == 3
            && facts[0] == "container hog"
            && facts[1].starts_with("limit ")
            && facts[2] == "exit 137",
        "the three facts this object supports and no fourth: {facts:?}"
    );
}

/// **A container something outside it keeps stopping, politely** (NOTES § D85) — the same defect
/// as the capture above with the contradiction fully on the page: a **CRITICAL** headed *"keeps
/// crashing"* whose own evidence line read *"an ordinary shutdown and not an error"*, one line
/// apart, both halves written by k8rs.
///
/// `broken-sigterm` catches SIGTERM and exits `143`, which is what the kubelet killing a
/// container looks like when the application handles the signal — a failing liveness probe,
/// which this capture has. The application that *ignores* it is `broken-startup`'s `exit 137`,
/// and the two cards send the reader to the same place.
#[test]
fn a_container_that_is_stopped_politely_is_not_one_that_keeps_crashing() {
    let raw = fixture("sigterm");
    let capture = pod("sigterm");
    let c = capture
        .containers
        .first()
        .expect("the capture reports on its container");
    println!("{c:?}");
    assert_eq!(
        c.last_terminated.as_ref().map(|run| run.exit_code),
        Some(captured_i32(
            captured_status(&raw, "containerStatuses", &c.name),
            &["lastState", "terminated", "exitCode"]
        )),
        "the exit code comes off the capture: 143 is 128 + SIGTERM, and the card below is \
         about a container that was asked to stop and did"
    );

    let all = findings(&["sigterm"]);
    show(&all);
    let card = only(&all, "broken-sigterm", "CrashLoopBackOff");
    assert!(
        !card.title.to_lowercase().contains("crashing"),
        "the container did not crash — it was stopped, and it stopped: {}",
        card.title
    );
    assert!(
        card.evidence.contains("ordinary shutdown and not an error"),
        "the evidence line that made the old title impossible to believe is still there — the \
         card was fixed by making the title true, not by deleting the sentence that exposed \
         it: {}",
        card.evidence
    );
    assert!(
        card.title.contains("stopped"),
        "and the title now says what the evidence says: this container's last run was \
         stopped rather than failing: {}",
        card.title
    );
    // **One `lastState` is one run, here too.** *Something keeps stopping this container*
    // read the whole loop off a single sample, and the same accumulated-backoff shape that
    // breaks the `exit 0` title breaks this one.
    assert!(
        card.title.contains("last run on record"),
        "and it says which run it read that off, because the snapshot holds exactly one — and \
         says it about the record, which does not move when the container runs again: {}",
        card.title
    );
    for pointer in SENT_TO_THE_LOGS {
        assert!(
            !card.action.contains(pointer),
            "an application that shut down when it was asked to logs a shutdown, and that is \
             not why it is being restarted: {}",
            card.action
        );
    }
    assert!(
        card.action.contains("liveness"),
        "a health check that keeps failing is what stops a container that has not crashed, \
         and this capture's liveness probe is `exec: false`: {}",
        card.action
    );
    // **The producer that loops and is not Kubernetes.** A userspace memory killer sends
    // SIGTERM, not SIGKILL, so it lands on this card and not on rule 2's — and it comes back
    // every time the container grows to the same size. `terminationGracePeriodSeconds`
    // expiry and preemption/eviction/drain do not reach here and are deliberately absent.
    assert!(
        card.action.contains("systemd-oomd") || card.action.contains("earlyoom"),
        "a userspace out-of-memory killer sends SIGTERM and keeps sending it, and an action \
         that names only the probes leaves that reader restarting a pod forever: {}",
        card.action
    );
}

/// **An init container that is stopped, and the probe the API server would have refused**
/// (NOTES § D85).
///
/// `validateInitContainers` forbids `livenessProbe`, `readinessProbe` and `startupProbe` on an
/// init container that is not `restartPolicy: Always`, so *check the liveness and startup
/// probes first* sends this reader to a field `kubectl apply` would have rejected — and the
/// rest of that action only says where **not** to look, which leaves no next step at all.
///
/// No capture holds this shape: `broken-init`'s `migrate` loops on `exit 1`. The plant moves
/// the exit code and nothing else (NOTES § D53), which is the one field that decides the
/// branch under test.
#[test]
fn an_init_container_that_was_stopped_is_not_sent_to_a_probe_it_may_not_have() {
    let stopped = capture_but("init", |pod| exited(pod, "migrate", 143));
    let migrate = container(&stopped, "migrate");
    println!("{migrate:?}");
    assert_eq!(
        migrate.role,
        ContainerRole::Init,
        "a restartable init container may have probes, so the arm under test would not be \
         the one that fires"
    );

    let all = analyze(&pods_at(vec![stopped], now()));
    show(&all);
    // Selected by the ending, not by the rule: `stopped_action`'s init arm is shared between
    // rules 1 and 5, and which of the two draws depends on which half of the backoff loop the
    // capture caught — a face `scripts/cluster.sh` § `[init]` accepts either of (NOTES § D114).
    let card = only(&all, "broken-init", "was stopped");
    assert!(
        card.title.contains("stopped"),
        "143 on an init container is the same ending as on any other: {}",
        card.title
    );
    for probe in ["liveness", "readiness", "startup"] {
        assert!(
            !card.action.contains(probe),
            "Kubernetes rejects a {probe} probe on this kind of container, so naming one is \
             advice the reader cannot follow: {}",
            card.action
        );
    }
    assert!(
        card.action.contains("systemd-oomd") || card.action.contains("earlyoom"),
        "and the reader is left with somewhere real to look — a userspace memory killer \
         sends SIGTERM and reaches an init container like any other process: {}",
        card.action
    );
    // **This rule's own pin on [`stopped_action`]'s init arm, and it is not a duplicate of rule
    // 5's.** The sentence is shared, so a card here is one merge away from being covered by
    // nothing but another rule's test — and the day somebody splits the helper again, that
    // coverage leaves with it and nothing goes red. Everything above this line survives wording
    // that keeps the killer's name and loses the rest; the *reason* no probe is named is what
    // only this arm says.
    assert!(
        card.action.contains("does not allow health checks"),
        "the reader is told why no probe is named, or this branch is indistinguishable from one \
         that merely forgot to mention them: {}",
        card.action
    );
}

/// **Rule 6's `137` arm, on the object whose kill came from outside the application**
/// (NOTES § D85, § D71). `broken-startup` declares a `startupProbe` that never passes, so the
/// kubelet kills a container that was running perfectly well — and the general arm's *find the
/// application's own error* is a hunt through a log that does not hold one.
///
/// **`OOMKilled` is rule 2's**, so a `137` that reaches this rule is always the other kind.
///
/// **The previous run is a plant now, and the corpus is why** (NOTES § D114). It was
/// `startup.json`'s own bytes until the 2026-08-16 trip: that probe is
/// `failureThreshold: 720` × `periodSeconds: 5`, so the kubelet's first kill lands **an hour**
/// after the pod starts, and the capture that held it was taken 60m 30s in. The 2026-08-16 trip
/// captured at ~30 minutes and the container had never been killed — within
/// `scripts/cluster.sh` § `[startup]`, which asks for `started == false` and a declared probe and
/// never for a restart. So the shape is not one a capture run can be relied on to produce, and
/// **no committed capture holds a `137` that is not the kernel's**: the only others in the corpus
/// are `oom`/`oomserving` (`OOMKilled`, rule 2's) and `gang` (`RestartingAllContainers`, the
/// restart-rule arm). The plant writes the run the old capture carried, field for field.
#[test]
fn a_kill_from_outside_the_application_does_not_send_the_reader_to_its_logs() {
    let capture = capture_but("startup", |p| ended_as(p, "slowboot", 137, None, None));
    let c = container(&capture, "slowboot");
    let run = c
        .last_terminated
        .as_ref()
        .expect("the plant writes the run the kubelet's kill would have left");
    println!("{run:?}");
    assert_eq!(run.exit_code, 137);
    assert_ne!(
        run.reason.as_deref(),
        Some("OOMKilled"),
        "a kill the kernel took for memory is rule 2's card, and this arm would be \
         unreachable on an object carrying that word"
    );
    assert_eq!(
        run.message, None,
        "the log-line arm answers first whenever a message exists, so this run has to \
         carry none or the arm under test is unreachable"
    );
    assert!(
        container(&pod("startup"), "slowboot")
            .last_terminated
            .is_none(),
        "and the plant is doing real work: the day this capture carries its own previous run \
         again, read it instead of building one"
    );

    let all = analyze(&pods_at(vec![capture], now()));
    show(&all);
    let card = only(&all, "broken-startup", "on record failed");
    for pointer in SENT_TO_THE_LOGS {
        assert!(
            !card.action.contains(pointer),
            "the application did not fail — something outside it killed the container, and \
             its own log holds no error to find: {}",
            card.action
        );
    }
    assert!(
        card.action.contains("liveness") && card.action.contains("startup"),
        "the two health checks that kill a container that is otherwise running, and this \
         capture is killed by the second of them: {}",
        card.action
    );
    // **The cause this repository has written evidence for** (NOTES § D84): on a host without
    // memory headroom a genuine cgroup OOM arrives here as `exitCode: 137, reason: "Error"`,
    // the word `OOMKilled` simply lost — so the correlation runs the wrong way. `oom.json`
    // says `OOMKilled` and reaches rule 2; the same manifest on the capture host did not.
    assert!(
        card.action.contains("memory"),
        "137 without the word is not proof the kill was not for memory — D84 reproduced \
         exactly that five times running, and this action told that reader to go and look \
         at probes: {}",
        card.action
    );
    // **And the caveat itself, on this arm as well as on the `Init` one.** `contains("memory")`
    // alone passes a sentence that names the limit and says nothing about the word being
    // missing — the half that stops a reader reading *no `OOMKilled`* as *not memory*. It is one
    // requirement over both arms of [`killed_action`], so it is pinned on both: pinned on one, a
    // rewrite that drops it here ships green (NOTES § D84).
    assert!(
        card.action.contains("not always labelled"),
        "the kernel's word may simply be absent, and the arm that does not say so rules memory \
         out exactly where a starved node makes it likeliest: {}",
        card.action
    );

    // **The canary under [`SENT_TO_THE_LOGS`].** A rule reworded out from under that list
    // leaves every "must not contain" assertion above passing over a phrase nothing produces
    // any more — "found nothing" and "there was nothing to find" print the same green line
    // (CLAUDE.md § Code phase rules). Both sentences are still the right advice on the cards
    // that keep them: `broken-crashloop` really did crash, and `broken-init` really did fail.
    let everything = findings(&CAPTURED_PODS);
    for pointer in SENT_TO_THE_LOGS {
        assert!(
            everything.iter().any(|f| f.action.contains(pointer)),
            "no card in the whole capture says {pointer:?} any more, so the assertions that \
             forbid it are guarding nothing: {:?}",
            everything
                .iter()
                .map(|f| f.action.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// **The same arm on the role Kubernetes allows no probe on** — rule 6 was the one of the three
/// rules whose `137` action never got a role split, and it prints **beside** rule 5's card about
/// the same container: one screen, one object, one card sending the reader to a liveness probe
/// and the other saying `validateInitContainers` forbids all three (NOTES § D85, § D90).
///
/// **The split is asserted as a split**, not as two independent sentences: the `Regular` arm must
/// keep the probes, or a rewrite that dropped them everywhere would pass the negatives below
/// while making the card useless on the role that *does* have a health check.
///
/// **No committed capture holds an init container killed from outside**, so the previous run is
/// a plant on a decoded copy of `healthy-retry.json` and the current state is
/// [`init_previous_run`]'s (NOTES § D40). **Since 2026-08-16 the `Regular` half below is a plant
/// too**, and for a reason that is about the capture window rather than about the shape — see
/// [`a_kill_from_outside_the_application_does_not_send_the_reader_to_its_logs`] (NOTES § D114).
#[test]
fn an_init_container_killed_from_outside_is_not_sent_to_a_probe_it_may_not_have() {
    let killed = init_previous_run(137, None, None, false);
    let waiter = container(&killed, "wait-for-db");
    println!("{waiter:?}");
    let run = waiter
        .last_terminated
        .as_ref()
        .expect("the plant rewrote the run before this one");
    assert!(
        waiter.role == ContainerRole::Init
            && !doing_its_job(waiter)
            && (run.exit_code, run.reason.as_deref(), run.message.as_deref())
                == (137, Some("Error"), None),
        "the arm under test is rule 6's `137`, and it is reached only past the log-line arm, \
         past rule 2's `OOMKilled` and past the reason the kubelet writes for a status it never \
         read: {waiter:?}"
    );

    let all = analyze(&pods_at(vec![killed], now()));
    show(&all);
    // **Rule 6's card folds into rule 5's here since 2026-08-16** (NOTES § D113): both take
    // [`killed_action`] on this ending and rule 5 now carries the duration, so rule 6 adds
    // nothing. The sentence under test is the same one either way — what changed is which card
    // the reader reads it on, and that is the severe one.
    let card = only(&all, "healthy-retry", "restarted");
    assert_eq!(
        card.action,
        killed_action(ContainerRole::Init),
        "the arm under test, on the card that survived the fold"
    );
    // **Lowercased, or the guard only holds for the capitalisation it happened to be written
    // against.** A sentence-initial *Probes are worth checking* is the same forbidden advice on
    // the same forbidden role, and it walked past this loop while it compared the raw string
    // (NOTES § D31).
    let said = card.action.to_lowercase();
    for probe in PROBE_WORDS {
        assert!(
            !said.contains(probe),
            "`validateInitContainers` rejects a {probe} on this kind of container, and rule 5's \
             card on this very container says so one row down the same screen: {}",
            card.action
        );
    }
    // **What survives the split, both halves.** The memory limit is true of every role — an init
    // container carries limits and the kernel takes it the same way, and on a host without
    // headroom a real cgroup kill arrives here as `137`/`Error` with the word lost, which is the
    // one shape where nothing else on the screen says *memory* (NOTES § D84).
    assert!(
        card.action.contains("memory limit") && card.action.contains("not always labelled"),
        "the reader is sent to the limit and told the kernel's word may be missing — an action \
         that reads anything into its absence rules memory out exactly where memory is likeliest \
         and rule 2 is silent: {}",
        card.action
    );
    // **And the sentence this arm exists for** (NOTES § D85): the kill came from outside, so the
    // general *read the logs* arm is a hunt through a log that holds no error. Without this pin
    // the split could be satisfied by falling through to the shared log sentence, which is one
    // thing the arm may not do.
    assert!(
        card.action.contains("its own logs will not say why"),
        "the reason this arm is not the general one: {}",
        card.action
    );
    for pointer in SENT_TO_THE_LOGS {
        assert!(
            !card.action.contains(pointer),
            "the application did not fail — something outside it killed the container, and its \
             own log holds no error to find: {}",
            card.action
        );
    }

    // **The other side of the split, or the negatives above pass on a sentence that helps
    // nobody.** `broken-startup` is a regular container the kubelet killed for a `startupProbe`
    // that never passed, and it is the one role the probes are real advice for.
    //
    // The kill is planted for the same reason it is in
    // [`a_kill_from_outside_the_application_does_not_send_the_reader_to_its_logs`] — that probe's
    // first kill lands an hour in, and the 2026-08-16 capture was taken before it (NOTES § D114).
    let killed_regular = capture_but("startup", |p| ended_as(p, "slowboot", 137, None, None));
    let captured = analyze(&pods_at(vec![killed_regular], now()));
    show(&captured);
    let regular = only(&captured, "broken-startup", "on record failed");
    assert!(
        regular.action.contains("liveness") && regular.action.contains("startup"),
        "the two health checks that kill a container that is otherwise running stay on the role \
         allowed to have them: {}",
        regular.action
    );
    assert_ne!(
        regular.action, card.action,
        "and the arms are a split, not one sentence reached twice"
    );

    // **The budget both arms are drawn inside** (`screens/alerts.md` § The height). The sentence
    // they replaced measured six wrapped lines; neither replacement may be longer than the cap
    // that file makes a `rules.rs` finding rather than a layout problem.
    for role in [
        ContainerRole::Regular,
        ContainerRole::Sidecar,
        ContainerRole::Init,
    ] {
        let lines = wrapped_at(killed_action(role), ACTION_COLUMNS);
        println!(
            "{role:?}: {} lines at {ACTION_COLUMNS} columns",
            lines.len()
        );
        assert!(
            lines.len() <= 5,
            "{role:?}: an action that wraps past five lines is a `rules.rs` finding — {} lines: \
             {:?}",
            lines.len(),
            killed_action(role)
        );
    }
}

/// **The third meaning of `137`, and the only one that is not a kill** (NOTES § D90).
/// `convertToAPIContainerStatuses` writes `137` beside [`STATUS_LOST`] in two places, both of
/// them a status the kubelet could not read — the runtime reporting the container `Unknown` while
/// the last status said `Running`, and the container gone from the runtime's list altogether. The
/// number is a placeholder (`// this code indicates an error` is the comment beside it), so
/// every door [`killed_action`] opens is about a signal no record holds.
///
/// **It is asserted ahead of the log-line arm on purpose.** The kubelet writes its own sentence
/// into `message` beside that reason, so without this ordering the card prints *the last thing it
/// logged was: The container could not be located when the pod was terminated* about a container
/// that logged nothing.
///
/// **No capture holds this shape** — it was measured on a kind v1.36.1 cluster and never
/// captured, so both the reason and the kubelet's message are planted on a decoded copy
/// (NOTES § D40).
///
/// **Every role is fed it, and the one the cluster produced is the `Regular`.** The arm is
/// role-blind, so the path is the same three times over — and a check is proven only for the
/// shapes it was fed, never for the shapes it would obviously handle (NOTES § D29). What D90
/// measured was a sandbox rebuild under a **healthy regular container**; driving only the init
/// container would have left that one untested and the sidecar reaching [`killed_action`] through
/// a line-width measurement and no card at all.
#[test]
fn a_run_kubernetes_lost_track_of_is_not_read_as_a_kill() {
    // The kubelet's own two sentences, verbatim from `kubelet_pods.go`, and the shape with no
    // message at all (NOTES § D29).
    let messages = [
        Some("The container could not be located when the pod was terminated"),
        Some(
            "The container could not be located when the pod was deleted.  The container used to be Running",
        ),
        None,
    ];
    // One capture per role. `broken-startup` is the shape D90 measured — a regular container the
    // rebuild took; `healthy-unreadysidecar` is a native sidecar that is up and not ready, so
    // [`ended_as`] writes it a previous run and the restart that goes with one; the init
    // container needs its *current* state moved as well, which is [`init_previous_run`]'s job.
    let roles = [
        ("startup", "slowboot", ContainerRole::Regular),
        ("healthy-unreadysidecar", "proxy", ContainerRole::Sidecar),
        ("healthy-retry", "wait-for-db", ContainerRole::Init),
    ];
    for (capture, name, role) in roles {
        for message in messages {
            let lost = match role {
                // **Under [`RESTARTS_WARN`], which is what leaves rule 6 the card that speaks**
                // (NOTES § D102). The other two rows are already there — `startup.json` is
                // captured at one restart and `healthy-unreadysidecar.json` at none, which
                // [`ended_as`] takes to one — while `healthy-retry.json` is a retry loop captured
                // at three, so this role alone has to say the number. Past the band rule 5 draws
                // the same sentence and rule 6's card collapses into it, which is
                // [`one_card_per_action_leaves_the_more_severe_card_standing`]'s subject and not
                // this test's: here the requirement is rule 6's own title, on every role.
                ContainerRole::Init => init_previous_run_counting(
                    137,
                    Some(STATUS_LOST),
                    message,
                    false,
                    Some(RESTARTS_WARN - 1),
                ),
                _ => capture_but(capture, |p| {
                    ended_as(p, name, 137, Some(STATUS_LOST), message)
                }),
            };
            // **Both halves, per role**: the role the card is about, and the gate rule 6 sits
            // behind — a base whose container is serving draws nothing at all, and every negative
            // below would then be asserted about a card that was never made (NOTES § D26).
            let subject = container(&lost, name);
            assert!(
                subject.role == role && !doing_its_job(subject),
                "the role under test and the gate rule 6 sits behind: {subject:?}"
            );
            // **The object the cluster writes, in the two fields two shipped behaviours read.**
            // The kubelet is describing a run it never watched, so it writes the reason, the
            // message and the code and nothing else — no `startedAt`, no `finishedAt`, no
            // `containerID`. A plant that left the capture's stamps behind would prove [`lasted`]
            // and [`Finding::timestamp`] against an object no cluster produces (NOTES § D29).
            assert_eq!(
                subject
                    .last_terminated
                    .as_ref()
                    .map(|r| (r.started_at.is_none(), r.finished_at.is_none())),
                Some((true, true)),
                "{role:?}: measured on kind v1.36.1 — both stamps are null on this shape: \
                 {subject:?}"
            );
            let object = lost.id.name.clone();
            let all = analyze(&pods_at(vec![lost], now()));
            show(&all);
            // **Looked up by the code and not by the words**, so the two assertions below are
            // what fails when the title is wrong. Keyed on the shipped title, `only` goes red
            // first and the requirement never runs — a lookup that doubles as the assertion
            // passes for the wrong reason the day someone rewrites it. Rule 6 is the only rule
            // that puts the exit code in its *title*.
            let card = only(&all, &object, "exit 137");
            assert!(
                card.title.contains("lost track of the container"),
                "{role:?}: the code is translated as what it is — a number written in where a \
                 status went missing, not a kill anything is recorded as having sent: {}",
                card.title
            );
            // **The rule's own subject may not be asserted about this shape.** *The container's
            // previous run failed* stood one line above a translation calling the number a
            // placeholder and an action saying nothing here says what ended the run — three
            // sentences on one card, the first contradicted by the other two and false of the
            // object: the container measured healthy either side of the rebuild on kind v1.36.1
            // (NOTES § D85, § D93).
            assert!(
                !card.title.to_lowercase().contains("failed"),
                "{role:?}: nothing is known to have failed — a title that says so is this box's \
                 own defect rebuilt in the rule it was opened to fix: {}",
                card.title
            );
            // **And the fact it carries instead is the one the reader needs**: there is no
            // previous-run log to go and read, because the kubelet gates `logs --previous` on
            // the `containerID` this shape does not have. Silence was the other door and was
            // refused for that reason (NOTES § D93).
            assert!(
                card.title.to_lowercase().contains("did not record how"),
                "{role:?}: the card says what happened rather than nothing at all — and names \
                 who did not record it, because *the record* is a noun nothing on this card, in \
                 `screens/alerts.md` or in kubectl's own output introduces (invariant 14): {}",
                card.title
            );
            assert!(
                card.action.contains("no signal was recorded") && card.action.contains("node"),
                "{role:?}: and the action answers the title rather than the number: nothing sent \
                 this, so the doors are on the machine that lost it: {}",
                card.action
            );
            // **The producer, and the two it may not name.** Measured on kind v1.36.1, one pod
            // each: `crictl rmp -f` on the sandbox writes this object; a node reboot writes
            // `exit 255` / `Unknown` instead, because containerd's state survives it and the
            // containers are found dead; and restarting containerd changes nothing at all, the
            // shims outlive it. The card sent a 3am reader to `uptime` and
            // `systemctl status containerd` on a machine that had been up for weeks.
            assert!(
                card.action.contains("sandbox"),
                "{role:?}: the measured producer is a rebuilt pod sandbox, and this file already \
                 has the words for it one helper over (`finished_action`): {}",
                card.action
            );
            for wrong in ["reboot", "containerd", "runtime restart", "uptime"] {
                assert!(
                    !card.action.to_lowercase().contains(wrong),
                    "{role:?}: {wrong:?} does not produce this object — a door that cannot be \
                     the cause costs the reader their first move: {}",
                    card.action
                );
            }
            // **Lowercased, because the forbidden word is forbidden however it is capitalised.**
            // A sentence-initial *Probes are worth checking* passed this loop while it compared
            // the raw string, on exactly the role that may not have one (NOTES § D31).
            let said = card.action.to_lowercase();
            for door in ["liveness", "startup", "probe", "memory limit"] {
                assert!(
                    !said.contains(door),
                    "{role:?}: {door:?} is a door onto a kill, and this card is not about one — \
                     an action that lists them under a title saying the status was lost is the \
                     disagreement this box was opened for: {}",
                    card.action
                );
            }
            // **The message never reaches this card, and the assertion for that is the
            // positive one above** — `no signal was recorded` is what the action has to say, and
            // a card printing the kubelet's sentence instead fails it first. A negative naming
            // the frame stood here until 2026-08-16 and could not fail: on this shape the record
            // is stamp-less, so [`last_log_line`] refuses the field before the arm order gets a
            // say, and removing *both* guards reddens the line above rather than this one. The
            // two guards are asserted where they live —
            // [`a_message_on_a_record_nobody_stamped_is_never_read`] and
            // [`the_quote_frame_says_who_recorded_the_line_and_never_who_wrote_it`].
            // **And what the missing fields do to the card, pinned as observed.** Both follow
            // from the object rather than from a choice this rule made: with no `finishedAt`
            // there is no age, which is a state D88 made deliberate for rule 5's arm with no
            // previous run; with no stamps there is no *ran for*, so the evidence is the
            // container and nothing else. Neither is asserted as desirable — they are asserted so
            // that a rule which starts inventing one is caught saying so (NOTES § D93).
            assert_eq!(
                card.timestamp, None,
                "{role:?}: the run carries no `finishedAt`, so the card carries no age and may \
                 not invent one: {card:?}"
            );
            assert!(
                !card.evidence.contains("ran for"),
                "{role:?}: and no duration either — the run has no stamps to measure: {}",
                card.evidence
            );
        }
    }

    // **The canary under the ordering.** With the arm removed the first two shapes above fall to
    // the [`Failed`](Ending::Failed) arm, which is a *different* wrong answer from the one the
    // negatives hunt — so this line proves the ending really is read: on any other reason the same
    // record carries the message onto the card, and here it does not.
    //
    // **On the evidence line since 2026-08-16, not in the action** (NOTES § D113): the *what to
    // do* is k8rs's own words on every ending, so the quote is what moved and not whether it is
    // printed.
    let logged = init_previous_run(137, None, Some("panic: cannot reach db"), false);
    let all = analyze(&pods_at(vec![logged], now()));
    let card = only(&all, "healthy-retry", "on record failed");
    println!("{} | {}", card.evidence, card.action);
    assert!(
        card.evidence
            .contains("Kubernetes recorded this: panic: cannot reach db"),
        "the message still reaches the card on every reason but the one the kubelet writes \
         itself: {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        killed_action(ContainerRole::Init),
        "and the advice is the one the exit code decides, quote or no quote"
    );
}

/// **Rule 6's third exemption: a container the pod's own restart rule removed did not fail**
/// (NOTES § D93). `RestartAllContainersOnContainerExits` is beta-on-by-default at the version
/// `tests/fixtures/K8S_VERSION` pins, so under `restartPolicyRules` the kubelet takes the *other*
/// containers down to restart them together — `exitCode: 137`, [`RESTART_ALL`], and the waiting
/// reason and message it writes for them, all verified in `kubelet_pods.go` at v1.36.1. A WARN
/// card for a declared policy working correctly is the false-positive class this rule was
/// designed around, one per firing, on a field that never expires (NOTES § D71).
///
/// **The silence is proved beside a card, not alone.** A rule that had simply stopped firing
/// would pass any assertion that only looks for nothing, so the sibling is planted in the same
/// pod with the exit that actually ended it — `3`, its own error — and has to draw.
///
/// **Which phase this is, exactly, because the other one is not covered and must not look it.**
/// `RestartAllContainers` removes the old container rather than leaving it for the kubelet to
/// query, so the trigger's own record is propagated into `lastState` when its containerID changes
/// (`kubelet_pods.go:2299-2302`) — that is the object below, and it is producible. **Before that
/// propagation the trigger's `exit 3` is in `state.terminated`, which no rule reads as an
/// ending** — [`doing_its_job`] is the only reader of the current terminated state and asks only
/// whether an init container finished — and the synthesized `137` is in every container's
/// `lastState` including its own: rule
/// 6 is then quiet on the whole pod. So this test proves the exemption keeps a card that exists,
/// not that a card always exists. That gap is a hole and not a hand-off, and it is boxed
/// (NOTES § D93).
///
/// **`broken-hostpath` is the base because it is the one committed capture with two regular
/// containers**; both are `Running` and ready there, so both current states are planted — the
/// removed one into the `RestartingAllContainers` wait the kubelet writes, the sibling into a
/// restart it has not finished (NOTES § D40).
///
/// **The phase this test cannot reach now has a capture of its own** (NOTES § D114).
/// `gang.json` is a real firing, caught *settled*: both containers back `Running` with the
/// synthesized record in `lastState` and no sibling showing an ending of its own — which is
/// exactly the "rule 6 is quiet on the whole pod" phase the paragraph above boxes.
/// [`the_captured_gang_restart_draws_no_card_about_either_container`] reads it, so the hole is
/// measured rather than only reasoned about; this test keeps the plant because it is the only
/// way to prove the exemption *beside a card that still draws*.
#[test]
fn a_container_the_pods_own_restart_rule_removed_is_not_a_run_that_failed() {
    let restarted = capture_but("hostpath", |p| {
        // What the kubelet writes for a container it removed to restart the pod together: the
        // wait, its own sentence, and the previous run carrying the same reason.
        ended_as(
            p,
            "nosy",
            137,
            Some(RESTART_ALL),
            Some("The container is removed because RestartAllContainers in place"),
        );
        let removed = container_status(p, "nosy");
        removed.state = waiting_at(
            RESTART_ALL,
            Some("The container is removed because RestartAllContainers in place"),
        );
        removed.ready = false;
        // And the sibling that actually ended, which is why the rule fired at all.
        ended_as(p, "shipper", 3, None, None);
        let failed = container_status(p, "shipper");
        failed.state = waiting_at("ContainerCreating", None);
        failed.ready = false;
    });
    // **The two shapes, and [`ended_as`]'s strip asserted in both directions.** The kubelet writes
    // its own terminations as three fields — `Reason`, `Message`, `ExitCode` — and nothing else:
    // `kubelet_pods.go:2581-2585` at v1.36.1 is the struct literal, and a zero `metav1.Time`
    // marshals to `null` (`apimachinery/.../v1/time.go:162`). So `nosy` has no stamps and
    // `shipper`, whose reason is the ordinary `Error`, keeps the capture's. **Both directions
    // matter and neither was asserted**: stripping every reason costs rule 1 its *"the last run
    // lasted …"* line, and stripping neither invents one (NOTES § D29, § D93).
    for (name, code, reason, stamped) in [
        ("nosy", 137, Some(RESTART_ALL), false),
        ("shipper", 3, Some("Error"), true),
    ] {
        let c = container(&restarted, name);
        println!("{c:?}");
        assert!(
            !doing_its_job(c)
                && c.last_terminated
                    .as_ref()
                    .map(|r| (r.exit_code, r.reason.as_deref()))
                    == Some((code, reason)),
            "{name}: the gate rule 6 sits behind, and the run it would read — without both, the \
             silence below is the rule not reaching the container rather than exempting it: {c:?}"
        );
        assert_eq!(
            c.last_terminated
                .as_ref()
                .map(|r| (r.started_at.is_some(), r.finished_at.is_some())),
            Some((stamped, stamped)),
            "{name}: {reason:?} is {}a reason the kubelet writes itself, so the stamps are \
             {}there — a plant that got this backwards would prove `lasted` and \
             `Finding::timestamp` against an object no cluster produces: {c:?}",
            if stamped { "not " } else { "" },
            if stamped { "" } else { "not " }
        );
    }

    // **Rule 6 is asked directly, because its title is no longer one string.** The silence was
    // asserted as *no card whose title says "on record failed" names nosy* — and rule 6 has
    // two titles since the `STATUS_LOST` branch, so a rule that drew the other one about this
    // container passed. Calling the rule states the requirement instead of searching for the
    // words it happens to use (NOTES § D26, § D93).
    let removed = previous_run_failed(&restarted, container(&restarted, "nosy"));
    assert!(
        removed.is_none(),
        "rule 6 draws nothing at all about a container the pod's own restart rule removed, \
         whatever it would have titled the card: {removed:?}"
    );
    assert!(
        previous_run_failed(&restarted, container(&restarted, "shipper")).is_some(),
        "and in this phase it still draws about the sibling that actually ended, or the exemption \
         above is a rule that stopped firing rather than one that exempts — the claim is about \
         the object below, whose trigger has had its own record propagated into `lastState`, and \
         not about every phase of a restart-rule firing (NOTES § D93)"
    );

    let all = analyze(&pods_at(vec![restarted], now()));
    show(&all);
    // **The sibling draws, so the silence is an exemption and not a dead rule** (NOTES § D26).
    //
    // **Keyed on `exit 3` and not on the rule's own words**, for the reason the `STATUS_LOST`
    // lookup was re-keyed one test up: with the exemption gone there are *two* cards saying
    // *the last run on record failed*, so `only` panics on the count and the assertion that names
    // the
    // defect never runs. A lookup that doubles as the assertion reports the wrong failure.
    let failed = only(&all, "broken-hostpath", "exit 3");
    assert!(
        failed.evidence.contains("container shipper"),
        "the container that actually ended is the one with the card, and it is named on it: {}",
        failed.evidence
    );
    assert!(
        failed.title.contains("exit 3"),
        "with its own exit code, which is the one thing on this pod that says why: {}",
        failed.title
    );
    // **And the removed one draws no rule 6 card at all.** Checked by name rather than by
    // counting: other rules draw about `nosy` on this capture — it is the host-mount fixture —
    // so "no card mentions nosy" would be false for a reason that has nothing to do with this.
    assert!(
        !all.iter()
            .any(|f| f.title.contains("on record failed") && f.evidence.contains("container nosy")),
        "a container the pod asked Kubernetes to remove did not fail, and a WARN card per \
         restart-rule firing is the false-positive class this rule is designed around: {:?}",
        titles(&all)
    );
    // **The kubelet's own sentence goes with it, and that is a side effect rather than the fix.**
    // It would have become this card's whole *what to do* — a placeholder where the advice
    // belongs, the same shape `STATUS_LOST` is scoped out of by arm order. **The class under both
    // is closed**: [`last_words`] stopped claiming an author and [`last_log_line`] drops a
    // kubelet placeholder outright, so a third reason would meet a structural answer rather than
    // two accidents (NOTES § D88, § D93).
    assert!(
        !all.iter()
            .any(|f| f.action.contains("RestartAllContainers in place")),
        "the kubelet wrote that sentence, not the container: {:?}",
        all.iter().map(|f| f.action.as_str()).collect::<Vec<_>>()
    );
    // **The translation is not silenced with the card.** Rules 1 and 5 print `exit_fact`
    // whatever rule 6 does, so the reading still reaches a screen — this is the assertion that
    // says which rule now carries `RESTART_ALL`'s row to a reader, since rule 6 no longer does.
    let cycling = init_previous_run(137, Some(RESTART_ALL), None, false);
    let all = analyze(&pods_at(vec![cycling], now()));
    show(&all);
    let card = only(&all, "healthy-retry", "restarted 4 times");
    assert!(
        card.evidence.contains("restart every container in the pod"),
        "rule 5 draws the count and carries the translation with it, so the row is still read by \
         a card and not only by the table test: {}",
        card.evidence
    );
}

/// **A gang restart that settled, on the bytes a cluster wrote** (NOTES § D114, § D93).
/// `gang.json` was captured on 2026-08-16 from kind v1.36.1: `trigger` declares
/// `restartPolicyRules: [{action: RestartAllContainers, exitCodes: {In, [3]}}]`, exited `3`, and
/// the kubelet took `bystander` down with it and restarted the pair.
///
/// **It is the phase [`a_container_the_pods_own_restart_rule_removed_is_not_a_run_that_failed`]
/// cannot build**, and the one that file's doc comment boxes as a hole: the trigger's own
/// `exit 3` has been propagated away, so the synthesized `137` / [`RESTART_ALL`] record is in
/// **every** container's `lastState` including the one whose exit caused it, and rule 6 has
/// nothing left to draw about anybody. The plant over there proves the exemption beside a card
/// that still draws; this proves the exemption is what a real firing actually leaves behind.
///
/// **What the capture confirms about the object itself**, all measured rather than reasoned:
/// the record carries `null` stamps and no `containerID` — the three-field struct literal — and
/// it lands on the trigger too, which is the correction the second operator review made.
#[test]
fn the_captured_gang_restart_draws_no_card_about_either_container() {
    let raw = fixture("gang");
    let p = pod("gang");

    // The declaration that makes this a gang restart rather than a plain one — the *action*,
    // which the published schema does not admit exists (NOTES § D97).
    let actions: Vec<&str> = raw["spec"]["containers"]
        .as_array()
        .expect("the capture declares its containers")
        .iter()
        .flat_map(|c| c["restartPolicyRules"].as_array().into_iter().flatten())
        .filter_map(|r| r["action"].as_str())
        .collect();
    // **`RestartAllContainers` is the *action*; [`RESTART_ALL`] is the *reason* the kubelet
    // writes when it acts on it — `RestartingAllContainers`.** Two different strings for the two
    // halves of one event, and the spec side is spelled out here rather than borrowed from the
    // constant, which names the status side.
    assert_eq!(
        actions,
        ["RestartAllContainers"],
        "one container declares one rule and its action is the pod-wide one — the validator takes
         two actions and only this one restarts the siblings, so with `Restart` here the capture \
         is an ordinary restart and proves nothing about this exemption (NOTES § D97)"
    );
    assert_ne!(
        actions[0], RESTART_ALL,
        "and the action is not the reason — a test that conflated them would pass on a capture \
         whose spec said neither"
    );

    // The record, on **both** containers including the trigger, with the kubelet's three fields
    // and no more.
    for c in &p.containers {
        let run = c
            .last_terminated
            .as_ref()
            .unwrap_or_else(|| panic!("{}: the firing leaves a record on every container", c.name));
        assert_eq!(
            (run.exit_code, run.reason.as_deref()),
            (137, Some(RESTART_ALL)),
            "{}: the pair the kubelet writes for a container it removed to restart the pod",
            c.name
        );
        assert_eq!(
            (run.started_at.as_ref(), run.finished_at.as_ref()),
            (None, None),
            "{}: it is describing a run it did not watch, so there are no stamps — and this is \
             the half that was settled from a struct literal until this capture (NOTES § D93)",
            c.name
        );
        assert_eq!(
            ending(run),
            Ending::RestartRule,
            "{}: which is the ending rule 6 exempts",
            c.name
        );
        assert!(
            previous_run_failed(&p, c).is_none(),
            "{}: nothing failed — the pod asked Kubernetes to restart these containers and it \
             did, and a WARN card per firing on a field that never expires is the false-positive \
             class this rule is designed around",
            c.name
        );
    }

    // And on the screen: the whole pod is quiet at the pin. Both containers are serving again,
    // so rule 5's card has aged out too (NOTES § D100) — asserted here because *this* is what a
    // settled gang restart looks like to a reader, and it is the claim
    // `the_whole_capture_through_the_rules_at_once` lists `gang` as silent for.
    let all = findings(&["gang"]);
    show(&all);
    nothing(
        &all,
        "a pod whose own restart rule fired and finished is not broken now (D2)",
    );
}

/// **One ending on each of the three roles, in the state rule 1 reads and the state rule 5
/// reads** — the driver behind the two whole-card-set tests below (NOTES § D29, § D40).
///
/// **`looping: true` is rule 1's state and `false` is rule 5's**, and the base capture moves with
/// it rather than the plant: `crashloop.json` is a captured `CrashLoopBackOff` and
/// `restarts10.json` a captured restart history that is out of it, so on the `Regular` role only
/// the previous run's ending is planted at all.
///
/// **`reason: None` is the control and keeps [`exited`]'s pairing** — `Error` beside a non-zero
/// code, which is what the runtime writes and what every committed capture holds. Fed `1` it is
/// the ordinary bad exit each new arm is measured against, and it is what keeps the forbidden
/// sentences reachable: a negative asserted over a rule set that stopped saying the words guards
/// nothing (CLAUDE.md § Code phase rules).
///
/// **The two `137` reasons are plants on every role, and neither is captured** — both were
/// measured on kind v1.36.1 and never captured (NOTES § D40, § D93). What is captured underneath
/// each is the state: the backoff wait, the restart count, the sidecar that is up and not ready.
///
/// **The sidecar's count is the one field moved beyond the ending**, because
/// `healthy-unreadysidecar.json` was captured at `0` restarts and rule 5 does not look below
/// [`RESTARTS_WARN`] — without it that role reaches rule 1 and never rule 5.
fn every_role_with(
    code: i32,
    reason: Option<&str>,
    looping: bool,
) -> Vec<(ContainerRole, &'static str, PodSnapshot)> {
    let (capture, plain) = if looping {
        ("crashloop", "quitter")
    } else {
        ("restarts10", "flaky")
    };
    // **Built by `match`ing every role rather than by listing three** — a fourth
    // [`ContainerRole`] then fails to compile here, the way it already does in `rules.rs`, instead
    // of going unswept while every guard below stays green (NOTES § D113).
    EVERY_ROLE
        .into_iter()
        .map(|role| match role {
            ContainerRole::Regular => (
                role,
                plain,
                capture_but(capture, |p| ended_as(p, plain, code, reason, None)),
            ),
            ContainerRole::Sidecar => (
                role,
                "proxy",
                capture_but("healthy-unreadysidecar", |p| {
                    ended_as(p, "proxy", code, reason, None);
                    if looping {
                        backing_off(p, "proxy");
                    }
                    container_status(p, "proxy").restart_count = RESTARTS_WARN + 1;
                }),
            ),
            ContainerRole::Init => (
                role,
                "wait-for-db",
                init_previous_run(code, reason, None, looping),
            ),
        })
        .collect()
}

/// **Every card k8rs draws about a container whose last run Kubernetes never watched end** — the
/// assertion the `137` box owed and did not make (NOTES § D93). That box's own test looks one card
/// up by its title, so rule 5's *"something keeps killing it"* and rule 1's *"read the previous
/// run's logs"* stood beside rule 6's correct card and were never read.
///
/// **What no card about this container may say**, whichever rule drew it: that something crashed,
/// failed or was killed — the kubelet wrote `137` where a status went missing and nothing is
/// recorded as having ended the run — and, worse than wrong, that the previous run's log holds
/// the answer: `--previous` is gated on `lastState.terminated.containerID`, which this record does
/// not carry, so the API answers `previous terminated container … not found` (measured).
///
/// **Both states and all three roles**, because rules 1 and 5 are mutually exclusive — rule 5
/// stands down inside `CrashLoopBackOff` — so one state proves only half the card set
/// ([`every_role_with`], NOTES § D29).
///
/// **The control is the same plant with the ordinary `Error` beside the code**, asserted to keep
/// every sentence this test forbids: the wording is not being deleted, it is being made
/// conditional on the reason the object carries.
#[test]
fn no_card_about_a_run_kubernetes_never_watched_end_claims_it_was_killed() {
    for looping in [false, true] {
        for (role, name, planted) in every_role_with(137, Some(STATUS_LOST), looping) {
            let object = planted.id.name.clone();
            let subject = container(&planted, name);
            println!("=== {object} {role:?} looping={looping}\n{subject:?}");
            assert_eq!(
                subject.role, role,
                "the role under test, or the shapes below are three copies of one card"
            );
            let snapshot = pods_at(vec![planted], now());
            let all = analyze(&snapshot);
            show(&all);
            let about = cards_about(&all, name);
            // **The whole set is counted, not searched.** The wrong cards this box removes were
            // standing *beside* a right one, so a test that looks one up by title cannot see
            // them — and a rule that goes silent takes its own negatives with it
            // (NOTES § D26, § D93).
            let expected = match (role, looping) {
                // rule 5, plus rule 7 on the one base that is running and unready, which is a
                // card about the readiness check now and not about the ending. **This is the
                // shape the box was opened on**: three cards, 26 lines about one container in a
                // 16-row pane, two of them carrying the same four-line action.
                (ContainerRole::Regular, false) => 2,
                // rule 1 on the wait and rule 5 on the re-run, each alone: rule 6 answers this
                // ending with the same sentence they do, so its card is folded into whichever of
                // the two drew (NOTES § D102). Both numbers were one higher until 2026-08-15.
                _ => 1,
            };
            assert_eq!(
                about.len(),
                expected,
                "{object} {role:?} looping={looping} draws {} cards about {name} and not \
                 {expected}: {:?}",
                about.len(),
                titles(&all)
            );
            no_card_reads_this_run_as_a_kill(&about, &format!("{object} {role:?}"));
            // **The card rule 1 or rule 5 drew is the one this box is about**, and it says what
            // the object supports: on the wait, that there is no record of the ending; on the
            // re-run, the count, with the translation beside it.
            // **Selected by a field rather than by the words under test.** Rules 1 and 5 both
            // put [`exit_fact`] in the *evidence*; rule 6 puts it in the title and rule 7 does
            // not draw it at all — so this picks the card this box is about without keying on
            // the sentence the assertions below are checking (NOTES § D26).
            let counted = about
                .iter()
                .find(|f| f.evidence.contains("exit 137"))
                .expect("rule 1 or rule 5 speaks for this container");
            if looping {
                assert!(
                    counted
                        .title
                        .starts_with("Kubernetes did not record how the run it last saw ended")
                        && counted.title.contains("CrashLoopBackOff"),
                    "the loop is real and the crash is not: {}",
                    counted.title
                );
            } else {
                assert!(
                    counted.title.contains("restarted") && counted.evidence.contains("exit 137"),
                    "the count is real and the translation goes with it: {} / {}",
                    counted.title,
                    counted.evidence
                );
            }
            // **And it draws the shared sentence, which the negatives above cannot say.** Every
            // one of them is satisfied by *ask a friend*: absent a positive, both call sites can
            // be replaced with any harmless string and the suite stays green — `tester` shipped
            // exactly that mutation past 184 tests. Asserted as **equality with the function**
            // rather than as a substring, because the requirement is that these two rules say
            // what rule 6 says and do not spell a fifth copy of it (NOTES § D93). The words
            // themselves are pinned one test up, on rule 6's own card.
            assert_eq!(
                counted.action,
                unwatched_action(),
                "{object} {role:?}: rules 1 and 5 answer this ending with the sentence rule 6 \
                 answers it with — that sharing is the fix, not a tidy-up of it"
            );
            // **And the command that sentence needs.** It sends the reader to the pod's events
            // — *the events rarely say so* is its own hedge about them — and `get -o yaml`
            // prints no events at all, so these two arms may not drift to it the way the
            // `RestartRule` arms deliberately did (invariant 4, NOTES § D95).
            assert_eq!(
                counted.kubectl_cmd.as_deref(),
                describe(&counted.object).as_deref(),
                "{object} {role:?}: the card offers the output its own action names"
            );
            // **Rule 6 still draws, and [`analyze`] is what takes the card off the screen**
            // (NOTES § D102). Until 2026-08-15 its card stood here unchanged — the one card this
            // shape always had right, which is why the neighbours went unread — and once the
            // neighbours were fixed the two carried one sentence twice.
            //
            // **Asked of the rule directly, because searching the list cannot tell the two
            // failures apart**: a rule that stopped firing and a card that was folded into its
            // neighbour print the same missing title (NOTES § D26, and the same reason the
            // `RestartAllContainers` exemption is asserted this way one test down).
            let pod = &snapshot.pods[0];
            let folded = previous_run_failed(pod, container(pod, name))
                .expect("rule 6 draws this card; the fold is analyze's and not the rule's");
            assert!(
                folded
                    .title
                    .starts_with("Kubernetes did not record how the run it last saw ended"),
                "rule 6 ships what it shipped: {}",
                folded.title
            );
            assert_eq!(
                folded.action, counted.action,
                "{object} {role:?}: the two cards carry one sentence, which is what makes the \
                 second copy of it worth nothing to a reader"
            );
            assert!(
                !about.iter().any(|f| f.title == folded.title),
                "{object} {role:?}: and the second copy is the one that goes — the survivor's \
                 own evidence still carries the exit code and its translation: {:?}",
                titles(&all)
            );
        }
    }

    // **The serving container, which is where rule 5's claim is printed at all.** The other
    // shapes above are past the band and *down*, and that title carries no clause — so without
    // this the sentence the box replaced would be untested in both directions.
    let serving = capture_but("restarts10serving", |p| {
        ended_as(p, "flaky", 137, Some(STATUS_LOST), None)
    });
    assert!(
        doing_its_job(container(&serving, "flaky")),
        "the plant has to stay a serving container, or the clause below is not the one drawn"
    );
    // Inside the run, since a serving card ages out at the pin (NOTES § D100).
    let began = began_running(&serving, "flaky");
    let all = serving_findings(serving, "flaky");
    let about = cards_about(&all, "flaky");
    assert_eq!(
        about.len(),
        1,
        "rule 5 alone — rule 6 stands down on a container that is serving: {:?}",
        titles(&all)
    );
    no_card_reads_this_run_as_a_kill(&about, "broken-restarts10serving serving");
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    // **And this is the shape D100 was measured on**: the record carries neither stamp, so the
    // card would have had no age at all on a screen with an age column. It is dated by the run
    // the container is *in* instead — the exception the helper above leaves to its callers.
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&began),
        "the serving card's age is `state.running.startedAt` and nothing off the record"
    );
    assert!(
        card.title
            .contains("it is serving now, and the record names no ending"),
        "the clause parses inside the serving sentence, claims only what the record holds, and \
         says it about the *record* — *its last run* is false the moment anything has run since, \
         because `lastState` freezes, and *but something keeps killing it* is a positive claim of \
         repeated killing on a run nothing is recorded as having killed (NOTES § D93, § D95): {}",
        card.title
    );
    assert_eq!(
        card.action,
        unwatched_action(),
        "and the serving card carries the same sentence — the clause above is the only thing \
         this shape changes, and a card is asserted whole or the half nobody looked at is where \
         the next wrong sentence sits"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        describe(&card.object).as_deref(),
        "and the command with it: this sentence's own hedge is about the pod's events, which \
         `get -o yaml` does not print"
    );

    // **The control, or every negative above is guarding a sentence the rule set no longer
    // says.** The same plants with the ordinary `Error` the runtime writes keep all three
    // (NOTES § D26): the wording is not being deleted, it is being made conditional on the
    // reason beside the code.
    // **Two codes, because *memory limit* moved arm on 2026-08-16** (NOTES § D113). It was on
    // rule 5's own `Failed` sentence, which is gone — all three rules share one now — and it
    // lives in [`killed_action`], which is reached by `137` and not by an ordinary `exit 1`.
    // A canary that swept only `1` would have said the phrase is produced by nothing.
    // **A termination message on half the plants, because rule 6's card folds without one**
    // (NOTES § D113). Rules 5 and 6 answer a failed ending with one sentence and rule 5 carries
    // the duration, so what keeps rule 6's card — and its *on record failed* title, which this
    // canary is about — is the quote it adds. A sweep with no message would have declared that
    // phrase unproduced.
    let mut kept: HashSet<&str> = HashSet::new();
    for looping in [false, true] {
        for (_, name, planted) in every_role_with(1, None, looping)
            .into_iter()
            .chain(every_role_with(137, None, looping))
            .chain([(
                ContainerRole::Init,
                "wait-for-db",
                init_previous_run(1, None, Some("panic: cannot reach db"), looping),
            )])
        {
            let all = analyze(&pods_at(vec![planted], now()));
            for f in cards_about(&all, name) {
                let said = format!("{} {} {}", f.title, f.evidence, f.action).to_lowercase();
                for phrase in KILLED_IT {
                    if said.contains(phrase) {
                        kept.insert(phrase);
                    }
                }
                for pointer in SENT_TO_THE_LOGS {
                    if f.action.contains(pointer) {
                        kept.insert(pointer);
                    }
                }
                if said.contains("memory limit") {
                    kept.insert("memory limit");
                }
            }
        }
    }
    // The control is a serving card, so it is read where a serving card draws (NOTES § D100) —
    // at the pin this container has been up for 49 hours and the canary would collect nothing.
    let serving = restarts10_ending("restarts10serving", 1);
    for f in cards_about(&serving_findings(serving, "flaky"), "flaky") {
        let said = f.title.to_lowercase();
        for phrase in KILLED_IT {
            if said.contains(phrase) {
                kept.insert(phrase);
            }
        }
    }
    println!("still said on an ordinary Error: {kept:?}");
    for phrase in KILLED_IT
        .iter()
        .chain(SENT_TO_THE_LOGS.iter())
        .chain(["memory limit"].iter())
    {
        assert!(
            kept.contains(phrase),
            "{phrase:?} is said by no card in this rule set any more, so the negatives above \
             guard nothing (CLAUDE.md § Code phase rules): {kept:?}"
        );
    }
}

/// **Which of two cards carrying one sentence is the one left standing** — [`one_card_per_action`]
/// keeps the **more severe** of them, and a tie goes to the rule [`analyze`] ran first
/// (NOTES § D102).
///
/// **Both orders are fed, and that is the whole of the first half.** Rule 5 runs before rule 6 and
/// rule 1 before both, so on every shape a cluster can produce the severe card is *also* the first
/// one: over this rule set *keep the more severe* and *keep the first* cannot be told apart by
/// running [`analyze`], and a test that only ran it would prove neither. The fold is called
/// directly with the pair the other way round rather than by inventing a pod that cannot exist
/// (NOTES § D29, § D40).
///
/// **Both cards are real ones** — rule 5's and rule 6's, drawn by the rules themselves off a
/// planted capture. Nothing here asserts against a `Finding` this file typed out.
#[test]
fn one_card_per_action_leaves_the_more_severe_card_standing() {
    // **The band that differs.** `restarts10.json` is a container past ten restarts and down, so
    // rule 5 is CRITICAL while rule 6 is WARN whatever it draws.
    let down = capture_but("restarts10", |p| {
        ended_as(p, "flaky", 137, Some(STATUS_LOST), None)
    });
    let c = container(&down, "flaky");
    let counted = restarting_repeatedly(&now(), &down, c).expect("rule 5 draws on the count");
    let lost = previous_run_failed(&down, c).expect("rule 6 draws on the ending");
    println!(
        "rule 5: {:?} | {}\nrule 6: {:?} | {}\nboth: {}",
        counted.severity, counted.title, lost.severity, lost.title, counted.action
    );
    assert_eq!(
        (counted.severity, lost.severity),
        (Severity::Critical, Severity::Warn),
        "the two bands this half is about — equal severities and the fold below has nothing to \
         choose between"
    );
    assert_eq!(
        counted.action, lost.action,
        "and the one sentence that makes them one story told twice"
    );
    for (order, pair) in [
        ("as analyze runs them", vec![counted.clone(), lost.clone()]),
        (
            "with the milder card first",
            vec![lost.clone(), counted.clone()],
        ),
    ] {
        let kept = one_card_per_action(pair);
        assert_eq!(
            titles(&kept),
            vec![counted.title.as_str()],
            "{order}: the severe card is the survivor. `Severity`'s derived `Ord` puts `Critical` \
             first, so the fold keeps the *smaller* of the two — a comparison written the natural \
             way round drops exactly the card that matters and passes every other assertion here"
        );
        assert_eq!(kept[0].severity, Severity::Critical, "{order}");
    }

    // **The tie, and it goes to the rule that ran first.** A sidecar four restarts in sits in
    // rule 5's WARN band, so both cards are WARN and the order [`analyze`] calls them in is all
    // that is left to separate them.
    let tied = capture_but("healthy-unreadysidecar", |p| {
        ended_as(p, "proxy", 137, Some(STATUS_LOST), None);
        container_status(p, "proxy").restart_count = RESTARTS_WARN + 1;
    });
    let c = container(&tied, "proxy");
    let first = restarting_repeatedly(&now(), &tied, c).expect("rule 5 draws past the band");
    let second = previous_run_failed(&tied, c).expect("rule 6 draws on the ending");
    println!("tied: {:?} / {:?}", first.severity, second.severity);
    assert_eq!(
        (first.severity, second.severity),
        (Severity::Warn, Severity::Warn),
        "the tie this half is about"
    );
    assert_eq!(
        titles(&one_card_per_action(vec![first.clone(), second])),
        vec![first.title.as_str()],
        "an equal pair keeps the one that came first, which is the order the rules already run in \
         — and rule 5's card carries the count, which rule 6's does not"
    );
}

/// **The eight rules [`analyze`] runs per container, each with its number** — the inventory below
/// is about *which* rules may share a sentence, so the list has to say which one drew what. It is
/// a second copy of the caller's list on purpose: that is what this test polices, and a rule added
/// there and not here shows up as a rule no sweep reads (NOTES § D102).
fn every_container_rule(
    now: &Time,
    pod: &PodSnapshot,
    c: &ContainerSnapshot,
) -> Vec<(&'static str, Finding)> {
    [
        ("rule 1", crash_looping(pod, c)),
        ("rule 2", out_of_memory(now, pod, c)),
        ("rule 3", image_not_pulled(pod, c)),
        ("rule 4", container_config_missing(pod, c)),
        ("rule 5", restarting_repeatedly(now, pod, c)),
        ("rule 6", previous_run_failed(pod, c)),
        ("rule 7", running_but_not_ready(now, pod, c)),
        ("rule 15", stopped_for_good(pod, c)),
    ]
    .into_iter()
    .filter_map(|(rule, f)| f.map(|f| (rule, f)))
    .collect()
}

/// **Every container in the corpus, planted shapes included** — the committed captures, plus the
/// whole `(exit code, reason)` table on all three roles in both states rules 1 and 5 reach.
/// The endings are where a shared sentence can come from at all, so a sweep that read only the
/// captures would read a corpus with no [`Ending::Unwatched`] in it.
fn every_shape_a_container_reaches() -> Vec<PodSnapshot> {
    let mut all = fixture_snapshot().pods;
    for looping in [false, true] {
        for (code, reason) in ENDING_PLANTS {
            all.extend(
                every_role_with(code, reason, looping)
                    .into_iter()
                    .map(|t| t.2),
            );
        }
    }
    all
}

/// **Every `(exit code, reason)` pair the sweeps plant, written once** — read by
/// [`every_shape_a_container_reaches`] and by
/// [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`], which plants them at a
/// count below [`RESTARTS_WARN`] to reach rule 6 with no neighbour. A second copy of this list is
/// a sweep that stops covering an ending the day one is added (NOTES § D85).
///
/// **`128` beside `StartError` is containerd's start failure** — a mistyped `command`, measured on
/// kind v1.36.1 — and it is the row that keeps a `StartError` in the sweep. **Its stamps are
/// real, because [`ended_as`] writes them**, so it takes [`failed_run_action`]'s *ran* arm here;
/// the epoch shape that takes the other one is asserted on the helper (NOTES § D40, § D113).
///
/// **`255` appears twice on purpose**: keying the pair is what makes the [`CODE_UNKNOWN`] row
/// narrow, and without the bare number the sweep never sees `255` fall through (NOTES § D29).
const ENDING_PLANTS: [(i32, Option<&str>); 14] = [
    // --- ONE ROW PER ENDING, FROM THE MATCH BELOW ---
    reaches(Ending::Finished),
    reaches(Ending::Stopped),
    reaches(Ending::Failed),
    reaches(Ending::Unwatched),
    reaches(Ending::RestartRule),
    reaches(Ending::CodeUnknown),
    // --- AND THE CODES THAT SHARE AN ENDING BUT NOT AN ANSWER ---
    (126, None),
    (127, None),
    (128, Some("StartError")),
    (128, None),
    (42, None),
    (137, None),
    (137, Some("OOMKilled")),
    (255, None),
];

/// **One `(exit code, reason)` pair per [`Ending`], through a `match` the compiler checks** —
/// which is the whole point of the function existing rather than the six rows being typed into
/// the array above (NOTES § D113).
///
/// **A seventh variant stops the tests compiling.** `rules.rs` already gets that for free: every
/// rule `match`es on [`Ending`] exhaustively, so a new one is a build error there. The guards had
/// no such thing — [`ENDING_PLANTS`] was a literal list and [`every_role_with`] a literal `vec!`,
/// so a variant nobody planted was silently unswept while the product file refused to build. This
/// is D95's own mechanism applied to the tests that check D95.
///
/// **The pairs are the reachable ones and the array carries the rest.** `137` alone is
/// [`Ending::Failed`] too, and so is `126`; what the array adds past this function is every code
/// that shares an ending with another and not its *answer*, which is what
/// [`failed_run_action`] answers differently.
const fn reaches(ending: Ending) -> (i32, Option<&'static str>) {
    match ending {
        Ending::Finished => (0, None),
        Ending::Stopped => (143, None),
        Ending::Failed => (1, None),
        Ending::Unwatched => (137, Some(STATUS_LOST)),
        Ending::RestartRule => (137, Some(RESTART_ALL)),
        Ending::CodeUnknown => (255, Some(CODE_UNKNOWN)),
    }
}

/// **Every [`ContainerRole`], and the array's length is the compiler's business** — a fourth role
/// makes [`every_role_with`]'s `match` fail to build rather than quietly going unmeasured
/// (NOTES § D113).
const EVERY_ROLE: [ContainerRole; 3] = [
    ContainerRole::Regular,
    ContainerRole::Sidecar,
    ContainerRole::Init,
];

/// **[`reaches`] is onto as well as into** — six distinct endings out of six rows in, checked by
/// running each pair back through [`ending`]. A `match` catches a variant nobody wrote a row for;
/// this catches a row written for the wrong variant, which the compiler cannot see.
#[test]
fn every_ending_has_a_pair_the_sweeps_plant_and_it_is_the_right_one() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for e in [
        Ending::Finished,
        Ending::Stopped,
        Ending::Failed,
        Ending::Unwatched,
        Ending::RestartRule,
        Ending::CodeUnknown,
    ] {
        let (code, reason) = reaches(e);
        let run = Terminated {
            reason: reason.map(str::to_string),
            exit_code: code,
            started_at: None,
            finished_at: None,
            message: None,
        };
        println!("{e:?} ← exit {code} {reason:?} → {:?}", ending(&run));
        assert_eq!(
            ending(&run),
            e,
            "the pair [`reaches`] hands out for {e:?} has to be one [`ending`] reads back as {e:?} \
             — a row on the wrong variant compiles and sweeps the wrong shape"
        );
        assert!(
            seen.insert(format!("{e:?}").leak()),
            "and no two variants share a pair, or one of them is unswept: {e:?}"
        );
        assert!(
            ENDING_PLANTS.contains(&(code, reason)),
            "and the array the sweeps read actually carries it: {code} {reason:?}"
        );
    }
    assert_eq!(seen.len(), 6, "every variant, or the list above went stale");
}

/// **Which pairs the fold is allowed to collapse, counted over the corpus rather than claimed in a
/// comment** (NOTES § D102). [`one_card_per_action`] deletes a card whenever two rules about one
/// container word their advice the same way, and nothing else in this file says which rules those
/// may be — so the next rule that happens to reach for a neighbour's sentence would start deleting
/// cards with the suite green.
///
/// **The inventory is asserted as a set and the count is printed**, because the two failures look
/// alike from here: a pair that appears is a rule that started sharing, and *no* pairs at all is a
/// sweep that stopped reaching the shapes (CLAUDE.md § A derived list asserts it found something).
///
/// **And the key that is no longer ours to worry about, which is a strengthening rather than a
/// move** (NOTES § D113). Rule 6's `Failed` arm used to make its whole *action* out of
/// [`last_words`] over a `Terminated::message` the workload wrote, so a crafted message was
/// compared against every other rule's action and the frame was the only thing between it and a
/// deleted card. The action is k8rs's own words on every arm now, so **no** action anywhere may
/// open with the frame — asserted here over the corpus as an absolute, with the quote counted on
/// the evidence line instead so the sweep is still proved to reach the shape.
/// [`a_crafted_termination_message_cannot_delete_another_rules_card`] drives it end to end
/// (invariant 9).
#[test]
fn only_rule_6_shares_a_sentence_with_a_neighbour_and_only_where_nothing_read_the_ending() {
    let mut pairs: BTreeSet<(&str, &str, String)> = BTreeSet::new();
    let mut shared = 0usize;
    let mut framed = 0usize;
    for pod in every_shape_a_container_reaches() {
        for c in &pod.containers {
            let drawn = every_container_rule(&now(), &pod, c);
            for (i, (rule, f)) in drawn.iter().enumerate() {
                assert!(
                    !f.action.contains(QUOTE_FRAME),
                    "{rule}: no action is a string the cluster wrote — the *what to do* is k8rs's \
                     own words on every card, which is what makes the five-line budget \
                     enforceable and what keeps a crafted message out of the value the fold \
                     matches on: {}",
                    f.action
                );
                if f.evidence.contains(QUOTE_FRAME) {
                    assert_eq!(
                        *rule, "rule 6",
                        "the quote is on one rule's evidence line and no other's — two rules \
                         wording one fact is where NOTES § D85 starts, and rule 15 reaches this \
                         through a different container state: {}",
                        f.evidence
                    );
                    framed += 1;
                }
                for (other, g) in drawn.iter().skip(i + 1) {
                    if g.action == f.action {
                        pairs.insert((rule, other, f.action.clone()));
                        shared += 1;
                    }
                }
            }
        }
    }
    for p in &pairs {
        println!("{} + {}\n  → {}", p.0, p.1, p.2);
    }
    println!("{shared} co-firing pairs with one sentence, {framed} quoted evidence lines");
    assert_eq!(
        pairs,
        [
            ("rule 1", "rule 6", unwatched_action().to_string()),
            ("rule 5", "rule 6", unwatched_action().to_string()),
            ("rule 1", "rule 6", no_exit_code_action().to_string()),
            ("rule 5", "rule 6", no_exit_code_action().to_string()),
            (
                "rule 1",
                "rule 6",
                failed_run_action(&exited_run(1), ContainerRole::Regular)
                    .0
                    .to_string(),
            ),
            (
                "rule 5",
                "rule 6",
                failed_run_action(&exited_run(1), ContainerRole::Regular)
                    .0
                    .to_string(),
            ),
            (
                "rule 5",
                "rule 6",
                killed_action(ContainerRole::Regular).to_string(),
            ),
            (
                "rule 5",
                "rule 6",
                killed_action(ContainerRole::Init).to_string(),
            ),
            // The never-ran arm has no shape in this corpus — [`ended_as`] writes real stamps,
            // which is right, and the epoch `startedAt` containerd writes has no committed
            // capture. It is asserted on the helper in
            // [`what_a_failed_run_needs_is_decided_by_whether_it_ran`] instead (NOTES § D40).
            (
                "rule 1",
                "rule 6",
                killed_action(ContainerRole::Regular).to_string(),
            ),
            (
                "rule 1",
                "rule 6",
                killed_action(ContainerRole::Init).to_string(),
            ),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "the whole inventory of what the fold may collapse — rule 6 against whichever of rules 1 \
         and 5 is speaking. **Two endings and one code** (NOTES § D113). The endings are the ones \
         all three rules answer with one sentence: a run nothing watched end, and a run nobody \
         read the code of. The code is `126`–`128`, where the cause is on the record rather than \
         in the role, so both rules say the one thing ([`failed_run_action`]) — and rule 5 \
         is absent from that row because it stands down inside `CrashLoopBackOff`, the only state \
         rule 1 draws in. Anything else here is a card being deleted that nobody decided to \
         delete"
    );
    assert!(
        shared > 0 && framed > 0,
        "the sweep has to reach both — {shared} pairs and {framed} quoted evidence lines means it \
         read a corpus without the shapes it exists for, and every assertion above is decoration"
    );
}

/// **A card about a run nobody watched end, beside a card about what the container is doing now**
/// (NOTES § D113, [`lost_run_yields_to_the_present`]).
///
/// **The card can never carry a date, so no clock can retire it.** The kubelet synthesizes a
/// [`Ending::Unwatched`] record with three fields and no `finishedAt`, so
/// [`Finding::timestamp`] is `None` and the card sits in the ageless block at the bottom of its
/// band for the life of the pod — beside the card naming the trouble the reader actually has. It
/// is the fifth instance of the permanence class in this file and the first with no stamp to read.
///
/// **[`one_card_per_action`] has no answer for it**, which is why this is a second mechanism and
/// not a widening of that one: the fold collapses rule 6's card into rules 1 and 5 because all
/// three say [`unwatched_action`], and those are exactly the neighbours that were never the
/// problem. What is left standing is rule 6's card beside rules 3, 4, 7 and 15 — different
/// sentences, one container, and only one of them about the present.
///
/// **Both directions on one capture, separated by the clock and nothing else** (invariant 5,
/// NOTES § D18). `readiness.json` is `Running`, not ready, `restartCount: 1` — below rule 5's band
/// and out of rule 1's state, so the lost status is the whole card set at a moment inside rule 7's
/// grace period. Past [`NOT_READY_GRACE`] rule 7 draws, and the undated card goes. Nothing about
/// the pod moves between the two halves, which is what makes this a test of the suppressor rather
/// than of two plants.
///
/// **No committed capture holds the pair** — the shape was measured on a review cluster and never
/// captured — so the ending is planted on a capture that already carries a previous run
/// (NOTES § D40, § D53).
#[test]
fn an_undated_lost_run_yields_to_a_card_about_what_the_container_is_doing_now() {
    let lost = capture_but("readiness", |p| {
        ended_as(p, "app", 137, Some(STATUS_LOST), None);
    });
    let c = container(&lost, "app");
    assert!(
        c.restarts < RESTARTS_WARN && waiting(c).is_none(),
        "below rule 5's band and out of rule 1's state, or the neighbour under test is not rule \
         7: {c:?}"
    );
    let unready_since = lost
        .ready
        .as_ref()
        .and_then(|r| r.last_transition.clone())
        .expect("the capture records when the pod stopped being ready");

    // --- INSIDE RULE 7'S GRACE: THE LOST STATUS IS THE ONLY TROUBLE, AND IT DRAWS ---
    let quiet = Time(unready_since.0 + SignedDuration::from_mins(1));
    let alone = analyze(&pods_at(vec![lost.clone()], quiet.clone()));
    show_at(&alone, &quiet);
    let card = only(&alone, "broken-readiness", "did not record how the run");
    assert_eq!(
        card.action,
        unwatched_action(),
        "the card this suppressor is about"
    );
    assert_eq!(
        card.timestamp, None,
        "and it is undated, which is the whole premise — a synthesized record carries no \
         `finishedAt` for the age column to read: {card:?}"
    );
    assert_eq!(
        alone.len(),
        1,
        "nothing else is drawing, so the lost status *is* the answer and the card stays exactly \
         where it is the reader's best line: {:?}",
        titles(&alone)
    );

    // --- PAST IT: A CARD ABOUT THE PRESENT ARRIVES, AND THE UNDATED ONE GOES ---
    let later = analyze(&pods_at(vec![lost], now()));
    show(&later);
    assert!(
        later
            .iter()
            .any(|f| f.title.contains("not receiving traffic")),
        "rule 7 has to be the thing that changed, or the half below proves nothing: {:?}",
        titles(&later)
    );
    assert!(
        !later.iter().any(|f| f.action == unwatched_action()),
        "an undatable card about a run that is over may not stand beside a dated one about what \
         the container is doing now — it cannot age off the screen and the reader cannot tell \
         *once* from *ongoing* either: {:?}",
        titles(&later)
    );
    // **And nothing else went with it**, which is the failure mode a suppressor keyed too widely
    // has: the card about the present is the one the reader came for.
    assert_eq!(
        titles(&later),
        vec!["Running, but not receiving traffic — the readiness check is failing"],
        "exactly one card goes and it is the undated one"
    );
}

/// **Every [`Reads`] label in [`analyze`], and what actually holds each one** (NOTES § D113).
///
/// **Four of the six `Now` labels were untested and one was reachable.** Flipped to
/// `Reads::Record` one at a time, `restarting_repeatedly` and `running_but_not_ready` went red and
/// `crash_looping`, `stopped_for_good`, `image_not_pulled` and `container_config_missing` stayed
/// green — so on a container waiting for an image *and* carrying a lost status, an undatable card
/// would have shipped beside the ImagePullBackOff card for the life of the pod, which is the
/// defect the suppressor exists to prevent, with a hand-typed enum value the only thing in its
/// way.
///
/// **Two mechanisms, and the doc used to credit the wrong one for half of them.** A rule's card
/// survives the suppressor either because its **label** says it reads the present, or because the
/// **container's state** makes it impossible for it to stand beside rule 6's lost-run card at all.
/// The table below names which per rule, and the `Impossible` rows are asserted as impossible
/// rather than assumed: a rule that starts co-firing is a rule whose label suddenly matters.
///
/// **`Reads::Record` is asserted by the two tests either side of this one** — rule 6's card is the
/// only candidate, and both directions of it are driven on a fixture in
/// [`an_undated_lost_run_yields_to_a_card_about_what_the_container_is_doing_now`].
#[test]
fn every_rule_that_reads_the_present_is_proved_to_be_one() {
    // The lost status every row below is measured against: undated, and rule 6's card about it is
    // what a mislabelled neighbour would leave standing.
    let lost = |p: &mut Pod, name: &str| ended_as(p, name, 137, Some(STATUS_LOST), None);

    // **(i) The rules that can stand beside it — the label is the whole guard.**
    for (rule, subject, plant) in [
        (
            "rule 3 image_not_pulled",
            "ImagePullBackOff",
            capture_but("readiness", |p| {
                lost(p, "app");
                container_status(p, "app").state = waiting_at("ImagePullBackOff", None);
            }),
        ),
        (
            "rule 4 container_config_missing",
            "CreateContainerConfigError",
            capture_but("readiness", |p| {
                lost(p, "app");
                container_status(p, "app").state = waiting_at(
                    "CreateContainerConfigError",
                    Some("secret \"db\" not found"),
                );
            }),
        ),
        (
            "rule 5 restarting_repeatedly",
            "restarted",
            capture_but("readiness", |p| {
                lost(p, "app");
                container_status(p, "app").restart_count = RESTARTS_WARN + 1;
            }),
        ),
        (
            "rule 7 running_but_not_ready",
            "not receiving traffic",
            capture_but("readiness", |p| lost(p, "app")),
        ),
    ] {
        let all = analyze(&pods_at(vec![plant], now()));
        show(&all);
        assert!(
            titles(&all).iter().any(|t| t.contains(subject)),
            "{rule}: the plant has to put this rule's card on the screen, or the row proves \
             nothing about its label: {:?}",
            titles(&all)
        );
        // **Rule 6's card by its own title, not by the action.** Rule 5 answers this ending with
        // the same sentence, and *its* card is a `Reads::Now` one that must survive — asserting
        // the sentence is absent would fail on the row that proves rule 5's label.
        assert!(
            !titles(&all)
                .iter()
                .any(|t| t.starts_with("Kubernetes did not record how the run")),
            "{rule}: it reads the container's present, so the undatable lost-run card yields to \
             it — a `Reads::Record` label here ships that card for the life of the pod: {:?}",
            titles(&all)
        );
    }

    // **(ii) The two whose label is *not* what holds them, and saying so is the point.** No rule
    // labelled `Reads::Now` can draw about a container in `CrashLoopBackOff` or sitting in
    // `state.terminated` beside rule 6 — the states exclude each other — so their labels are
    // never consulted and a flip of either stays green. That is a property of the rule set and
    // not of the suppressor, so it is asserted rather than credited to the label.
    for (rule, only_one, plant, name) in [
        (
            "rule 1 crash_looping",
            "rule 1",
            capture_but("crashloop", |p| lost(p, "quitter")),
            "quitter",
        ),
        // Rule 15's own shape, from the helper its tests already use: one container stopped for
        // good under `restartPolicy: Never` inside a pod that is still running.
        (
            "rule 15 stopped_for_good",
            "rule 15",
            stopped_under(Some("Never"), 1, None),
            "shipper",
        ),
    ] {
        let c = container(&plant, name);
        let drawn: Vec<&str> = every_container_rule(&now(), &plant, c)
            .iter()
            .map(|(r, _)| *r)
            .collect();
        println!("{rule}: {drawn:?}");
        assert!(
            drawn.contains(&only_one),
            "{rule}: the plant has to reach the rule this row is about: {drawn:?}"
        );
        // Rule 6 is the candidate, so it is expected here and is not a neighbour; what this row
        // claims is that no **other** `Reads::Now` rule can draw, which is what makes this rule's
        // own label unconsulted.
        assert_eq!(
            drawn
                .iter()
                .filter(|r| **r != only_one && **r != "rule 6")
                .copied()
                .collect::<Vec<&str>>(),
            Vec::<&str>::new(),
            "{rule}: this row's claim is that the container's *state* leaves no other rule able to \
             draw, which is why a flipped label here changes nothing. Something else drew, so the \
             label is load-bearing after all and this rule needs a row in (i) above: {drawn:?}"
        );
    }
}

/// **A termination message is free text from the API, and it is one of the fold's keys**
/// (invariant 9, NOTES § D102). Rule 6's `Failed` arm put `lastState.terminated.message` on the
/// card, so a workload that writes another rule's advice into it is writing into a value
/// [`one_card_per_action`] matches on — and a match deletes a card.
///
/// **Which value moved on 2026-08-16, and the guard got stronger for it** (NOTES § D113). The
/// quote was the whole *action*, which is the fold's primary key; it is a fact on the evidence
/// line now, which the subset clause reads. So the crafted string can no longer equal another
/// rule's action however exactly it is copied — that is asserted here as an absolute — and what
/// is left to guard is the fact list, where the frame does the same job.
///
/// **[`last_words`]' frame is the guard, and it was an accident until it was written down.** The
/// quote is wrapped in a constant prefix no static action in the file opens with (asserted over
/// the corpus in [`only_rule_6_shares_a_sentence_with_a_neighbour_and_only_where_nothing_read_the_ending`]), so
/// a crafted message cannot equal one however exactly it is copied.
///
/// **`restarts10.json` at `exit 1`**: rule 5 and rule 6 share the sentence and rule 6 adds the
/// quote, on one
/// container, which is the pair a crafted message would be trying to collapse.
///
/// **Three stamp states, and the third is the only one where the frame is the whole guard.**
/// [`last_log_line`] refuses the field on a record with no `finishedAt`, because that is the shape
/// every kubelet-authored message rides ([`a_message_on_a_record_nobody_stamped_is_never_read`]);
/// [`ran_for`] needs **both** stamps. So the two conditions come apart on
/// `startedAt: null, finishedAt: set`: the message *is* read, and neither card carries a duration
/// — [`ran_for`] refuses that record for both rules — so rule 6's facts are a subset of its
/// neighbour's and nothing but the frame stands between a crafted message and a deleted card. **It is producible** — `kuberuntime_container.go:760-763` fills
/// `Message` and `FinishedAt` in one block while `StartedAt` is set on a different branch, and a
/// zero `metav1.Time` marshals to `null`. Adding the guard on 2026-08-16 closed the *both-null*
/// state, which had been carrying this half; without this row invariant 9's guard would be proven
/// only where the subset clause blocked the fold anyway.
#[test]
fn a_crafted_termination_message_cannot_delete_another_rules_card() {
    // **Three runs of the same plant, and the third is where the impersonation would actually
    // delete something.** With both of the capture's stamps rule 6's evidence carries
    // [`ran_for`], which rule 5's card has not got, so the subset clause blocks the fold whatever
    // the sentences say. With neither, [`last_log_line`] refuses the field and the crafted value
    // never reaches a card at all. With `finishedAt` alone the two guards come apart: the message
    // is read and the duration is not, so the frame is the only thing left — which is the state
    // invariant 9's guard has to be proven in (NOTES § D29).
    let plant = |target: Option<&str>, started: bool, finished: bool| {
        capture_but("restarts10", |p| {
            ended_as(p, "flaky", 1, None, target);
            let run = container_status(p, "flaky")
                .last_state
                .as_mut()
                .and_then(|s| s.terminated.as_mut())
                .expect("ended_as wrote the run this plant is stripping");
            if !started {
                run.started_at = None;
            }
            if !finished {
                run.finished_at = None;
            }
        })
    };
    for (stamps, started, finished) in [
        ("both", true, true),
        ("neither", false, false),
        ("finishedAt only", false, true),
    ] {
        // The one the quote turns on, and the one the duration turns on — asserted rather than
        // assumed, or the third row is the second row under a different name.
        let read = analyze(&pods_at(vec![plant(None, started, finished)], now()));
        assert_eq!(
            cards_about(&read, "flaky")
                .iter()
                .any(|f| f.evidence.contains("ran for")),
            started && finished,
            "{stamps}: [`ran_for`] needs both stamps and [`last_log_line`] needs one, which is \
             what makes these three states and not two"
        );
        // The card set with nothing written into the message — what every crafted run below is
        // compared against, so what is asserted is *no card went* rather than a count typed out
        // here (NOTES § D26).
        let plain = read;
        let unaimed: Vec<&str> = cards_about(&plain, "flaky")
            .into_iter()
            .map(|f| f.title.as_str())
            .collect();
        assert!(
            unaimed.len() > 1,
            "{stamps}: more than one card about this container, or there is nothing for a \
             crafted message to delete and every run below passes for free: {unaimed:?}"
        );
        for target in [
            failed_run_action(&exited_run(1), ContainerRole::Regular).0,
            failed_run_action(&never_started_run(), ContainerRole::Regular).0,
            killed_action(ContainerRole::Regular),
            unwatched_action(),
            restart_rule_action(),
            "read the previous run's logs — that is where it says why it exits",
            "check the readiness probe: the path, the port, and whether the application answers \
             it yet",
        ] {
            let all = analyze(&pods_at(
                vec![plant(Some(target), started, finished)],
                now(),
            ));
            let about = cards_about(&all, "flaky");
            for f in &about {
                println!("{stamps} | {} | {}", f.title, f.action);
            }
            // **On the evidence line since 2026-08-16, and never on an action** (NOTES § D113).
            // The action is k8rs's own words on every arm, which takes the crafted value out of
            // the fold's *action* key altogether; what it reaches now is the fact list, which the
            // subset clause reads — and there the frame is what keeps it from equalling a
            // neighbour's fact.
            // The frame and not the payload: `target` is a real action, so a rule that legitimately
            // says it is not the failure — what is refused is a *quoted* string standing where the
            // instruction belongs.
            for f in &about {
                assert!(
                    !f.action.contains(QUOTE_FRAME),
                    "{stamps}: a string the workload wrote has reached a card's *what to do*: {}",
                    f.action
                );
            }
            let quoted = about
                .iter()
                .find(|f| f.evidence.contains(QUOTE_FRAME))
                .map(|f| {
                    f.evidence
                        .split(FACTS)
                        .find(|fact| fact.starts_with(QUOTE_FRAME))
                        .expect("the fact the line was found by")
                });
            assert_eq!(
                quoted,
                finished.then(|| last_words(target)).as_deref(),
                "{stamps}: where the record carries a `finishedAt` the container's words reach \
                 the card **framed**, and the frame is what keeps the crafted copy from equalling \
                 a neighbour's fact; where it does not, the record is the shape the kubelet \
                 synthesizes and the field is not read at all — either way nothing on the screen \
                 carries this string unattributed: {about:?}"
            );
            // **And on that half the message reaches nothing at all**, which is a stronger claim
            // than *it is not quoted*: every field of every card is the one the same pod draws
            // with no message written into it. A rule that dropped the frame and printed the
            // value bare would satisfy the assertion above and be the exact defect it guards
            // (NOTES § D26). Compared as whole findings, because *which line it landed on* is
            // precisely what is not being trusted here.
            if !finished {
                assert_eq!(
                    about,
                    cards_about(&plain, "flaky"),
                    "{stamps}: the crafted message may not change any part of any card — the \
                     record has no `finishedAt`, so the field is never read"
                );
            }
            // **The card set, against the same pod with no message at all.** What a crafted
            // message may change is rule 6's own action; what it may not change is how many cards
            // the container has. Comparing titles rather than counting says *which* card went.
            // **A superset and not an equality, since all three rules share the `Failed`
            // sentence** (NOTES § D113). With no message rule 6 adds no fact and its card folds
            // into rule 5's; the crafted message is a fact, so the card comes back. That is the
            // fold working — what invariant 9 forbids is a message *deleting* a card, and adding
            // one is the opposite direction.
            for title in &unaimed {
                assert!(
                    about.iter().any(|f| f.title == *title),
                    "{stamps}: a message the workload wrote has deleted {title:?} — the value it \
                     reached is the fold's key, and that is the whole of why the frame has to be \
                     a guard and not a wording choice (invariant 9): {:?}",
                    about.iter().map(|f| f.title.as_str()).collect::<Vec<_>>()
                );
            }
        }
    }
}

/// The frame [`last_words`] wraps [`Terminated::message`] in — spelled here so the test that pins
/// it does not re-type the thing it is checking (NOTES § D102). **Not "the container's words"**:
/// three authors reach that field and the frame stopped claiming any of them
/// ([`the_quote_frame_says_who_recorded_the_line_and_never_who_wrote_it`]).
const QUOTE_FRAME: &str = "Kubernetes recorded this: ";

/// **containerd's own error for a container whose `command` names a path the image has not got**,
/// measured on kind v1.36.1 with `command: ["/definitely-not-here"]`
/// (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1). It rides a record with a
/// real `finishedAt`, so [`last_log_line`] reads it.
///
/// **Spelled once because two tests need the same bytes** (NOTES § D85, § D113):
/// [`the_quote_frame_says_who_recorded_the_line_and_never_who_wrote_it`] drives the card it draws,
/// and [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`] measures it — it is
/// **7 wrapped lines at 49 columns**, which is what a `what to do` built out of it costs and why
/// the action is k8rs's own words instead.
const RUNTIME_START_FAILURE: &str = "failed to create containerd task: failed to create shim \
                                     task: OCI runtime create failed: runc create failed: unable \
                                     to start container process: error during container init: \
                                     exec: \"/definitely-not-here\": stat /definitely-not-here: no \
                                     such file or directory";

/// **A message on a record the kubelet synthesized is not read at all, and the missing stamp is
/// what says so** (NOTES § D88, § D93). Every synthesized record in `kubelet_pods.go` at v1.36.1
/// is the same three fields — `Reason`, `Message`, `ExitCode` — with no `startedAt`, no
/// `finishedAt` and no `containerID`, because the kubelet is describing a run it did not watch.
/// There are **four** such literals, not the two this file had scoped out: `:2385` and `:2624`
/// ([`STATUS_LOST`]), `:2584` ([`RESTART_ALL`]) and `:2717`, the init container whose status the
/// runtime lost. Anything arriving through a CRI status carries `FinishedAt` beside its
/// `Message`, off the same `CONTAINER_EXITED` branch
/// (`kuberuntime_container.go:760-763`), so the guard reads the presence of a field rather than
/// guessing at a reason.
///
/// **What it does *not* settle is who wrote the line, and that was claimed here for one turn.**
/// A CRI status is not a container: the runtime writes its own errors into `Message` beside a
/// real `FinishedAt`, so this guard lets those through by design and the *frame* is what stopped
/// claiming an author ([`the_quote_frame_says_who_recorded_the_line_and_never_who_wrote_it`]).
/// This test is about one thing only: a kubelet placeholder never displaces a card's advice.
///
/// **Which direction it fails in is the requirement.** A record with no `finishedAt` and a real
/// message would lose a line the reader could have had — a miss. A kubelet placeholder printed
/// where a rule's own advice belongs is a card that says nothing. The guard takes the first, and
/// the two are not symmetric.
///
/// **Both known instances were unreachable by accident and neither accident is the fix**
/// ([`a_run_kubernetes_lost_track_of_is_not_read_as_a_kill`] holds one by arm order,
/// [`a_container_the_pods_own_restart_rule_removed_is_not_a_run_that_failed`] the other by an
/// exemption granted for something else). This is asserted on [`last_log_line`] itself, which is
/// the one place both rules that print the frame read the field from — so a fifth literal, or a
/// rule that stops scoping one out, cannot reach it however it is routed.
#[test]
fn a_message_on_a_record_nobody_stamped_is_never_read() {
    // **The captured half, and it is the strongest evidence in this test.** `failed.json` carries
    // the kubelet's own sentence on `broken-failed` / `app`, with both stamps `null` — a real
    // cluster's bytes, not a plant (NOTES § D53). No rule reads that field as an ending today,
    // which is exactly why the guard belongs on the helper and not in a rule.
    let captured = pod("failed");
    let ContainerState::Terminated(lost) = &container(&captured, "app").state else {
        panic!("failed.json's container is terminated where the kubelet lost its status")
    };
    println!("failed.json: {lost:?}");
    assert!(
        lost.message.as_deref().is_some_and(|m| !m.is_empty()) && lost.finished_at.is_none(),
        "the capture has to carry a message on a stamp-less record, or the assertion below \
         passes because there was nothing to quote: {lost:?}"
    );
    assert_eq!(
        last_log_line(lost),
        None,
        "the kubelet wrote that sentence about a run it never watched — quoting it as the \
         container's own last words is a record that lies (invariant 4)"
    );

    // **All four synthesized literals, written as the kubelet writes them** — the three fields
    // and nothing else, so the shape under test is the source's and not this file's. Read from
    // `kubelet_pods.go` at v1.36.1; the two `ContainerStatusUnknown` sites differ in their
    // sentence and both are fed, because a guard is proven only for the shapes it was fed
    // (NOTES § D29).
    for (reason, message, code) in [
        (
            STATUS_LOST,
            "The container could not be located when the pod was terminated",
            137,
        ),
        (
            STATUS_LOST,
            "The container could not be located when the pod was deleted.  The container used to \
             be Running",
            137,
        ),
        (
            RESTART_ALL,
            "The container is removed because RestartAllContainers in place",
            137,
        ),
        // The fourth, and the one no arm in this file scopes out: an init container whose status
        // the runtime lost reads `Completed` / `0`, which is what a genuine finish reads too.
        (
            "Completed",
            "Unable to get init container status from container runtime and pod has been \
             initialized, treat it as exited normally",
            0,
        ),
    ] {
        let synthesized = Terminated::from(ContainerStateTerminated {
            reason: Some(reason.to_string()),
            message: Some(message.to_string()),
            exit_code: code,
            ..ContainerStateTerminated::default()
        });
        println!("{reason} exit {code}: {:?}", last_log_line(&synthesized));
        assert_eq!(
            last_log_line(&synthesized),
            None,
            "{reason}: the kubelet wrote this sentence, and the record it rides carries no \
             `finishedAt` because the kubelet never watched the run end — so the frame may not \
             claim the container said it"
        );
    }

    // **The control, or the guard is a delete rather than a discrimination** (NOTES § D26).
    // `crashloop.json` carries the container's own log tail under `FallbackToLogsOnError`, on a
    // record with both stamps, and that is the shape the frame exists for. The same run with
    // `finishedAt` removed is the only difference, and it is enough.
    let mut wrote = container(&pod("crashloop"), "quitter")
        .last_terminated
        .clone()
        .expect("the captured crash loop records how its last run ended");
    assert_eq!(
        last_log_line(&wrote),
        Some("panic: dial tcp db.payments.svc:5432: connect: connection refused"),
        "a container that wrote its own last line still gets it quoted: {wrote:?}"
    );
    wrote.finished_at = None;
    assert_eq!(
        last_log_line(&wrote),
        None,
        "and the one field is what decides it — nothing else about the record moved"
    );
}

/// **The three shapes the fold may not touch** (NOTES § D102). Each of them draws two cards that
/// stay two cards, and each is a different way the drop could be scoped too widely.
///
/// **(i) is the one that costs a reader a container**, and it is the half that was decoration
/// until 2026-08-16. Two containers each losing a status is *not* a pair a pod-wide fold can eat:
/// every card the container rules draw leads with [`container_fact`], and no two containers share
/// one, so the subset clause refuses the pair on the first fact — `k8s-admin` moved the fold out
/// of the loop and the suite stayed green. **What makes the scope load-bearing is a fact that did
/// not come from the container**: [`restarting_repeatedly`] puts `status.containerStatuses[].image`
/// on its evidence line verbatim, so a pod whose image string reads back as its *neighbour's*
/// [`container_fact`] builds the cross-container subset the clause cannot see — and a pod-wide fold
/// deletes the neighbour's card. The loop is what makes that unreachable, and this is the shape
/// that proves it (NOTES § D102, invariant 9).
///
/// **(iii) is the sentence half**: two cards about one container that answer different questions
/// are two questions, and the fold is keyed on the action for exactly that reason.
#[test]
fn one_card_per_action_is_scoped_to_one_container_and_to_one_sentence() {
    // --- (i) TWO CONTAINERS IN ONE POD, AND ONE OF THEM QUOTES THE OTHER ---
    // `hostpath.json` is the one committed capture with two regular containers. Both lose a
    // status; `shipper` is additionally pushed past the red band so rule 5 draws the CRITICAL
    // card that would do the beating, and its image is the string the API would have to carry for
    // the pair to be foldable at all. Both are captured at one restart with a previous run
    // already on them, so [`ended_as`] rewrites that run rather than counting a new one.
    // **`started: false` keeps rule 7 out of the shape, and the shape is what it is measuring**
    // (NOTES § D113). `ready: false` is what puts rule 6 on both containers, and it also puts rule
    // 7 on them once the grace period is past — which is a card about the *present*, so
    // [`lost_run_yields_to_the_present`] would take `nosy`'s lost-run card away and this half
    // would be measuring that suppressor instead of the fold's scope. A container whose
    // `startupProbe` has not passed yet is `ready: false` with the readiness probe not yet run at
    // all, which is exactly the state rule 7 stands down on and rule 6 does not.
    let both = capture_but("hostpath", |p| {
        for name in ["nosy", "shipper"] {
            ended_as(p, name, 137, Some(STATUS_LOST), None);
            container_status(p, name).ready = false;
            container_status(p, name).started = Some(false);
        }
        let beater = container_status(p, "shipper");
        beater.restart_count = RESTARTS_CRITICAL;
        // Free text, and the rule prints it unchanged. A registry would refuse this reference;
        // the API stores whatever the object says, and `rules.rs` never validates it.
        beater.image = "container nosy".to_string();
    });
    let all = analyze(&pods_at(vec![both], now()));
    show(&all);
    // The construction is asserted before what it proves: without the quote on the beating card
    // the subset clause blocks the pair anyway and the scope below is guarded by nothing.
    let beater = only(&all, "broken-hostpath", "restarted 10 times");
    let quoted = container_fact(container(&pod("hostpath"), "nosy"));
    assert!(
        beater.severity == Severity::Critical
            && beater.action == unwatched_action()
            && beater.evidence.split(FACTS).any(|fact| fact == quoted),
        "the beating card has to carry the *other* container's fact, or a pod-wide fold has \
         nothing to eat and this half is decoration again: {}",
        beater.evidence
    );
    for name in ["nosy", "shipper"] {
        let said = cards_about(&all, name)
            .into_iter()
            .filter(|f| f.action == unwatched_action())
            .count();
        assert_eq!(
            said,
            1,
            "{name} lost a status of its own and gets its own card — a pod-wide fold would leave \
             the second container's run unreported out of the first one's: {:?}",
            titles(&all)
        );
    }

    // --- (ii) TWO PODS, ONE SENTENCE EACH ---
    let one = capture_but("startup", |p| {
        ended_as(p, "slowboot", 137, Some(STATUS_LOST), None)
    });
    let other = capture_but("healthy-unreadysidecar", |p| {
        ended_as(p, "proxy", 137, Some(STATUS_LOST), None)
    });
    let all = analyze(&pods_at(vec![one, other], now()));
    show(&all);
    let lost: Vec<&Finding> = all
        .iter()
        .filter(|f| f.action == unwatched_action())
        .collect();
    assert_eq!(
        lost.len(),
        2,
        "one sandbox rebuild per pod is two cards — the fold never reaches across pods: {:?}",
        titles(&all)
    );
    assert_ne!(
        lost[0].object.name, lost[1].object.name,
        "and they are about different pods, or the shape above is one pod twice"
    );

    // --- (iii) ONE CONTAINER, TWO CARDS, TWO SENTENCES ---
    // Rule 2 beside rule 5: a labelled memory kill on a container past ten restarts. Rule 6 is
    // silent here (`OOMKilled` is rule 2's card), and rule 7 draws about the readiness check.
    let starved = capture_but("restarts10", |p| {
        ended_as(p, "flaky", 137, Some("OOMKilled"), None)
    });
    let c = container(&starved, "flaky");
    let killed = out_of_memory(&now(), &starved, c).expect("rule 2 draws on the labelled kill");
    let counted = restarting_repeatedly(&now(), &starved, c).expect("rule 5 draws on the count");
    assert_ne!(
        killed.action, counted.action,
        "two questions, two sentences — with one sentence this shape is the positive case wearing \
         a negative's name"
    );
    let all = analyze(&pods_at(vec![starved], now()));
    show(&all);
    let about = cards_about(&all, "flaky");
    // Written down rather than derived: a rule that goes quiet here would otherwise shrink the
    // set and pass the distinctness claim below by having less to be distinct about (NOTES § D26).
    assert_eq!(
        about.iter().map(|f| f.title.as_str()).collect::<Vec<_>>(),
        vec![
            killed.title.as_str(),
            counted.title.as_str(),
            "Running, but not receiving traffic — the readiness check is failing",
        ],
        "rules 2, 5 and 7 about one container, in the order analyze runs them"
    );
    let sentences: HashSet<&str> = about.iter().map(|f| f.action.as_str()).collect();
    assert_eq!(
        sentences.len(),
        about.len(),
        "three cards, three sentences, nothing folded — the fold is keyed on what a card tells \
         the reader to do and not on how many cards a container has: {sentences:?}"
    );

    // **And the pair where every *other* clause of the condition is satisfied** — same container,
    // rule 5 the more severe, rule 6's single fact already the survivor's first, both cards
    // undated. Only the sentences differ, so only the action check keeps them apart: with it
    // deleted this pair collapses and a reader loses *read the logs of that run* under a card
    // that says to check the memory limit (NOTES § D102).
    //
    // **Stripped rather than captured**: the kubelet stamps a run it watched, so an ordinary
    // `exit 1` with no times is a plant, and [`lasted`] going quiet is what makes rule 6's
    // evidence a subset at all (NOTES § D40).
    let undated = capture_but("restarts10", |p| {
        ended_as(p, "flaky", 1, None, None);
        let run = container_status(p, "flaky")
            .last_state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("ended_as wrote the run this plant is stripping");
        run.started_at = None;
        run.finished_at = None;
    });
    let c = container(&undated, "flaky");
    let severe = restarting_repeatedly(&now(), &undated, c).expect("rule 5 draws on the count");
    let mild = previous_run_failed(&undated, c).expect("rule 6 draws on the failed run");
    println!(
        "rule 5: {:?} | {} | {}\nrule 6: {:?} | {} | {}",
        severe.severity, severe.evidence, severe.action, mild.severity, mild.evidence, mild.action
    );

    // **Rules 5 and 6 can no longer be the pair that *differs*, and that is the point of the
    // family** (NOTES § D113). All three rules take [`failed_run_action`] whole, so wherever both
    // draw they say the one sentence — and on this plant rule 6 adds no fact, so the fold takes
    // it. Asserted here first, because the half below needs a pair that really does differ and
    // this one no longer can.
    assert_eq!(
        severe.action, mild.action,
        "one ending, one answer — a difference here is two rules disagreeing about one container"
    );
    assert_eq!(
        titles(&one_card_per_action(vec![severe.clone(), mild])),
        vec![severe.title.as_str()],
        "and the card that adds nothing goes, leaving the severe one"
    );

    // **The sentence half needs two rules answering different questions**, which is rule 7 beside
    // rule 5: one is about the count and the run behind it, the other about readiness now. Same
    // container, same pod, every other clause of the fold satisfied — and two cards, because the
    // fold folds a repeated sentence and nothing else.
    let asked = capture_but("restarts10", |p| ended_as(p, "flaky", 1, None, None));
    let c = container(&asked, "flaky");
    let counted = restarting_repeatedly(&now(), &asked, c).expect("rule 5 draws on the count");
    let unready = running_but_not_ready(&now(), &asked, c).expect("rule 7 draws on the readiness");
    println!("rule 5: {}\nrule 7: {}", counted.action, unready.action);
    assert!(
        counted.action != unready.action
            && unready
                .evidence
                .split(FACTS)
                .all(|fact| counted.evidence.split(FACTS).any(|kept| kept == fact)),
        "every clause but the action satisfied, or the assertion below is not about the action: \
         {} / {}",
        counted.evidence,
        unready.evidence
    );
    assert_eq!(
        one_card_per_action(vec![counted, unready]).len(),
        2,
        "two sentences are two cards however alike the rest of them is — the fold folds a repeated \
         sentence and nothing else"
    );
}

/// **A shared sentence is not enough on its own — the card that goes has to add nothing**
/// (NOTES § D102). Same action, every fact already on the survivor, and no timestamp the survivor
/// lacks; anything else stays its own card.
///
/// **Why it is a condition and not a comment.** The pair the fold was written for is lossless
/// because [`lasted`] answers `None` on a record with no stamps — three inferences away from the
/// fold, asserted two tests over, and nothing held the two together. Put the stamps back and rule
/// 6's card carries `ran for 30s` and an age that rule 5's does not, so dropping it deletes a fact
/// off the screen to save a repeated sentence. A duplicated sentence is a cheap failure; a
/// silently deleted fact is not.
///
/// **Both stamps are planted, and the shape is deliberately not one the kubelet writes** —
/// `kubelet_pods.go` fills in three fields for this reason and no times ([`ended_as`] strips them
/// for that reason). The subject here is the *fold's* condition, not a cluster's object: what is
/// being proved is that the drop is decided by what the cards carry rather than by a property of
/// one ending that happens to hold today (NOTES § D29, § D40).
///
/// **And the second half is the over-fire.** With only `finishedAt` on the record [`lasted`] is
/// `None` again, both cards date from the same field, and the pair collapses exactly as before —
/// so the condition is not a way of quietly switching the fold off.
#[test]
fn a_card_is_dropped_only_when_it_adds_nothing_to_the_one_that_beats_it() {
    // `restarts10.json` is rule 5's CRITICAL band, so rule 5 is the card that beats rule 6's WARN
    // either way and only the evidence and the stamps move between the two halves below.
    // **The fact rule 6 carries and rule 5 cannot is the container's last words** (NOTES § D113).
    // It was the *duration* until 2026-08-16 — and rule 5 carries one now, added so that the pair
    // this fold was written for actually collapses, which took the differentiator with it. The
    // quote is the right one to be left holding: it is the most useful string on the screen and
    // the reason `k8s-admin` ruled the surviving card worth keeping.
    fn lost_run_stamped(said: Option<&str>) -> PodSnapshot {
        capture_but("restarts10", |p| {
            ended_as(p, "flaky", 1, None, said);
            let run = container_status(p, "flaky")
                .last_state
                .as_mut()
                .and_then(|s| s.terminated.as_mut())
                .expect("ended_as wrote the run this plant is stamping");
            run.started_at = Some(time("2026-08-13T22:32:30Z"));
            run.finished_at = Some(time("2026-08-13T22:33:00Z"));
        })
    }

    // --- BOTH STAMPS: RULE 6 CARRIES A FACT AND AN AGE RULE 5 DOES NOT ---
    let whole = lost_run_stamped(Some("panic: dial tcp db:5432: connect: connection refused"));
    let c = container(&whole, "flaky");
    let counted = restarting_repeatedly(&now(), &whole, c).expect("rule 5 draws on the count");
    let lost = previous_run_failed(&whole, c).expect("rule 6 draws on the ending");
    println!(
        "rule 5: {} | {:?}\nrule 6: {} | {:?}",
        counted.evidence, counted.timestamp, lost.evidence, lost.timestamp
    );
    assert_eq!(
        counted.action, lost.action,
        "the shared sentence, or this shape is not the one the fold would look at"
    );
    assert!(
        lost.evidence.contains(QUOTE_FRAME)
            && !counted.evidence.contains(QUOTE_FRAME)
            && lost.timestamp.is_some(),
        "the plant has to give rule 6 something rule 5 has not, or the assertion below passes \
         for the wrong reason: {} / {}",
        lost.evidence,
        counted.evidence
    );
    let all = analyze(&pods_at(vec![whole], now()));
    show(&all);
    assert_eq!(
        cards_about(&all, "flaky")
            .into_iter()
            .filter(|f| f.action == counted.action)
            .count(),
        2,
        "both cards stand: one of them is carrying a fact the other does not, and a repeated \
         sentence is the cheaper of the two failures: {:?}",
        titles(&all)
    );

    // --- NO MESSAGE: NOTHING EXTRA, AND THE PAIR COLLAPSES ---
    // Rule 6's facts are [`container_fact`] and [`ran_for`], both of which the survivor carries
    // since 2026-08-16, and both cards date from the same `finishedAt`. That is the
    // `timestamp == timestamp` branch of the condition, which every other collapse in this file
    // reaches through `None` (NOTES § D113).
    let half = lost_run_stamped(None);
    let c = container(&half, "flaky");
    let counted = restarting_repeatedly(&now(), &half, c).expect("rule 5 draws on the count");
    let lost = previous_run_failed(&half, c).expect("rule 6 draws on the ending");
    println!(
        "rule 5: {} | {:?}\nrule 6: {} | {:?}",
        counted.evidence, counted.timestamp, lost.evidence, lost.timestamp
    );
    assert!(
        !lost.evidence.contains(QUOTE_FRAME)
            && lost.timestamp.is_some()
            && lost.timestamp == counted.timestamp,
        "nothing extra, and an age both cards read off the same field: {lost:?} / {counted:?}"
    );
    let all = analyze(&pods_at(vec![half], now()));
    show(&all);
    assert_eq!(
        cards_about(&all, "flaky")
            .into_iter()
            .filter(|f| f.action == counted.action)
            .count(),
        1,
        "and here the second copy still goes — a condition that kept every card would be the \
         fold switched off in the shape of a guard: {:?}",
        titles(&all)
    );

    // --- THE STAMP HALF, WHICH NO OBJECT SEPARATES FROM THE FACT HALF ---
    // **The pair is assembled and this is not a fixture.** Every rule that draws this ending dates
    // its card from the same `lastState.terminated.finishedAt`, so a survivor whose stamp is
    // missing while the card it beats has one is a pair the API cannot hand [`analyze`] today —
    // and the clause exists for the *next* shared action, not for this one (NOTES § D29, § D40).
    // What is fed to the fold is still two real cards with one field cleared, which is the same
    // move [`one_card_per_action_leaves_the_more_severe_card_standing`] makes to prove a direction
    // that running `analyze` cannot show.
    let mut undated = counted.clone();
    undated.timestamp = None;
    assert!(
        lost.timestamp.is_some() && undated.action == lost.action,
        "the shared sentence, and an age on the card that would be dropped"
    );
    assert_eq!(
        titles(&one_card_per_action(vec![undated.clone(), lost.clone()])).len(),
        2,
        "a card carrying an age the survivor has not got is not a second copy of it — dropping it \
         takes the *when* off the screen to save a repeated sentence"
    );
}

/// **Every card k8rs draws about a container the pod's own restart rule removed** — rule 6's half
/// was answered when the reason was added; rules 1 and 5 were left claiming it crashed and that
/// something keeps killing it, over an evidence line reading *which is what this pod asked for*
/// (NOTES § D93).
///
/// **Rule 5 gets an arm and not an exemption, and that is the ruling worth reading here.** One
/// restart-rule firing writes the same synthesized record into *every* container's `lastState`,
/// so an exemption on top of rule 6's would leave a pod thrashing 31 times in six minutes with no
/// card on the screen at all. The count is real; the claim about it was not.
///
/// **Rule 1's arm is written and is barely reachable, which is said rather than hidden.** The
/// restart-all path purges every container from the runtime, so `doBackOff` finds no exited
/// record, no backoff entry is made and `CrashLoopBackOff` does not appear — measured at about
/// one restart every 11s behind an 8s sleep, which is no backoff at all. **The `looping: true`
/// shapes below are planted, and a planted shape is not a reachable one**: what they prove is
/// that the arm the enum forced is truthful, not that a cluster draws it (NOTES § D40, § D93).
///
/// **What must not be on any of these cards** is a door onto a kill — the reader is being sent
/// after this container's memory limit and health checks when the container that exited may be
/// its sibling — and, on rule 6, a card at all.
#[test]
fn no_card_about_a_container_the_pods_own_restart_rule_removed_says_it_crashed() {
    // **The detector before the negatives, or the negatives pass because nothing was detected**
    // (NOTES § D26, § D29). Every phrase, and the exact eight words `tester` appended to walk
    // past the first version of this guard.
    for phrase in EXONERATES {
        let planted = format!("and that may be this container, {phrase} — check the spec");
        assert_eq!(
            exonerating(&planted),
            Some(phrase),
            "{phrase:?} is in the list and the detector does not see it"
        );
    }
    assert_eq!(
        exonerating("…can set it off, and that may be this container, but rarely"),
        Some("but rarely"),
        "the mutation that shipped green past three `contains` fragments"
    );
    assert_eq!(
        exonerating("Probes Are Worth Checking"),
        None,
        "and it does not fire on an ordinary sentence, or every card below passes for nothing"
    );

    // **The sentence itself, in one place** — every card below is asserted equal to it, so this
    // is where its content is guarded. **Two guards, because a clause can be taken back two
    // ways**: appending to it, which the `ends_with` catches, and denying it earlier in the
    // sentence, which the detector catches.
    println!("{}", restart_rule_action());
    assert_eq!(
        exonerating(restart_rule_action()),
        None,
        "the trigger carries this very record, so the card may not hand the reader an excuse to \
         stop looking at the container they are looking at (NOTES § D95)"
    );
    assert!(
        restart_rule_action().ends_with("and that may be this container"),
        "and the last thing the reader reads is that clause, unqualified — *but rarely* after it \
         is a card that exonerates in eight words: {}",
        restart_rule_action()
    );

    for looping in [false, true] {
        for (role, name, planted) in every_role_with(137, Some(RESTART_ALL), looping) {
            let object = planted.id.name.clone();
            println!("=== {object} {role:?} looping={looping}");
            // **Rule 6 is asked directly, per role and per state.** Its silence was proved on one
            // regular container in one phase; the arm that now carries it is read by three rules,
            // so the exemption is asserted everywhere it has to hold (NOTES § D29).
            assert!(
                previous_run_failed(&planted, container(&planted, name)).is_none(),
                "{object} {role:?}: the pod declared the rule and the kubelet obeyed it — \
                 nothing failed, whatever the card would have been titled"
            );
            let all = analyze(&pods_at(vec![planted], now()));
            show(&all);
            let about = cards_about(&all, name);
            let expected = match (role, looping) {
                // rule 5 and rule 7, which is about the readiness check and not the ending.
                (ContainerRole::Regular, false) => 2,
                // rule 1 on the wait, rule 5 on the re-run. Rule 6 draws nothing on this reason,
                // and that number is the assertion (NOTES § D26).
                _ => 1,
            };
            assert_eq!(
                about.len(),
                expected,
                "{object} {role:?} looping={looping} draws {} cards about {name} and not \
                 {expected}: {:?}",
                about.len(),
                titles(&all)
            );
            no_card_reads_this_run_as_a_kill(&about, &format!("{object} {role:?}"));
            no_card_lets_this_container_off(&about, &format!("{object} {role:?}"));
            let counted = about
                .iter()
                .find(|f| f.evidence.contains("exit 137"))
                .expect("rule 1 or rule 5 speaks for this container");
            if looping {
                assert!(
                    counted
                        .title
                        .starts_with("The pod's own restart rule removed the container"),
                    "the loop is real and the crash is not — this container was taken away, not \
                     broken: {}",
                    counted.title
                );
            }
            // **The action sends the reader to the container that actually exited, and does not
            // say this one is fine.** The trigger's own record is overwritten by the same
            // synthesized `137`, so *look elsewhere* would be wrong on exactly the container that
            // failed; what the record supports is that it does not say which one went first
            // (NOTES § D93).
            // **Equality, not fragments.** Three `contains` checks all held while eight words
            // were appended that took the third one back; tying the card to the function puts
            // every card behind the two guards at the top of this test (NOTES § D95).
            assert_eq!(
                counted.action,
                restart_rule_action(),
                "{object} {role:?}: the card carries the shared sentence, whose content is \
                 guarded where it is written"
            );
            // **And the command changes with the sentence.** `restartPolicyRules` is in the
            // spec and in no part of `describe`'s output, so an action naming it under
            // `kubectl describe pod` is a card that cannot show what it says (invariant 4,
            // NOTES § D95).
            assert_eq!(
                counted.kubectl_cmd.as_deref(),
                Some(
                    format!(
                        "kubectl get pod {object} -n {} -o yaml",
                        counted.object.namespace.as_deref().unwrap_or_default()
                    )
                    .as_str()
                ),
                "{object} {role:?}: the card offers the output its own action names"
            );
        }
    }

    // **The serving container, where rule 5's clause is printed at all** — see the test above.
    let serving = capture_but("restarts10serving", |p| {
        ended_as(p, "flaky", 137, Some(RESTART_ALL), None)
    });
    // Inside the run, since a serving card ages out at the pin (NOTES § D100).
    let began = began_running(&serving, "flaky");
    let all = serving_findings(serving, "flaky");
    let about = cards_about(&all, "flaky");
    assert_eq!(about.len(), 1, "rule 5 alone: {:?}", titles(&all));
    no_card_reads_this_run_as_a_kill(&about, "broken-restarts10serving serving");
    no_card_lets_this_container_off(&about, "broken-restarts10serving serving");
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    // **This is the shape D100 measured the undated card on** — a restart rule's synthesized
    // record carries neither stamp, in 100% of samples — so the age comes off the run the
    // container is in instead of off the record.
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&began),
        "the serving card's age is `state.running.startedAt` and nothing off the record"
    );
    assert!(
        card.title
            .contains("it is serving now, and the record names the pod's rule"),
        "the clause parses inside the serving sentence and says whose doing this was, about the \
         *record* rather than about the last restart — the record freezes while the count keeps \
         rising, and *but something keeps killing it* is the opposite claim about a pod getting \
         what it asked for (NOTES § D93, § D95): {}",
        card.title
    );
    // **The translation still reaches the screen from this card**, which is the only place it
    // does now that rule 6 is silent on the reason.
    assert!(
        card.evidence.contains("restart every container in the pod"),
        "{}",
        card.evidence
    );
    assert_eq!(
        card.action,
        restart_rule_action(),
        "and the serving card is behind the same guards as the rest — a card asserted on its \
         title alone is a card whose action nobody read"
    );

    // **The shape a cluster actually writes, which every plant above gets wrong in the same
    // way** (NOTES § D95). One firing puts the synthesized record into **every** container's
    // `lastState` — measured three of three and two of two, the trigger included — so the
    // fan-out this rule's own doc describes is drawn by two cards on a two-container pod, and
    // until now no test in this file had ever handed the rules more than one container carrying
    // it. `broken-hostpath` is the committed capture with two regular containers, and both are
    // moved together (NOTES § D40).
    let gang = capture_but("hostpath", |p| {
        for name in ["nosy", "shipper"] {
            ended_as(p, name, 137, Some(RESTART_ALL), None);
            container_status(p, name).restart_count = RESTARTS_WARN + 1;
        }
    });
    // Both containers are serving, and `nosy` started nine seconds before `shipper`, so one
    // moment inside the first run is inside the second as well (NOTES § D100).
    let stamps = ["nosy", "shipper"].map(|name| (name, began_running(&gang, name)));
    let all = serving_findings(gang, "nosy");
    // **Both containers draw, and each card is its own container's.** A rule that fired once for
    // the pod, or twice with one container's name on both, passes any assertion that only counts
    // (NOTES § D26) — which is also why `cards_about` matches the whole name: `nosy` and
    // `shipper` share a pod here, and the `contains` this helper used until now would have
    // merged them.
    //
    // **Counted over the cards drawn from the ending**, because this capture is the host-mount
    // fixture and rules 8 and 9 speak for both containers too — true cards about another
    // question, and not the fan-out under test.
    // **The lookup itself, on the one pod in this file with two containers to confuse.** `nosy`
    // and `shipper` do not nest, so the fan-out above passes under a substring match too — and a
    // helper that merges two containers' cards is a silent over-count in exactly the assertions
    // next door. A strict prefix of a real name is what separates the two implementations, and
    // `app` / `app-proxy` is the pair a real cluster brings (NOTES § D95).
    assert!(
        cards_about(&all, "ship").is_empty(),
        "a card is about a container or it is not: {:?}",
        titles(&all)
    );
    assert!(
        !cards_about(&all, "shipper").is_empty(),
        "and the whole name still finds it, or the line above passes because nothing matches at \
         all (NOTES § D26)"
    );

    for (name, began) in &stamps {
        let about = cards_about(&all, name);
        no_card_reads_this_run_as_a_kill(&about, &format!("broken-hostpath {name}"));
        no_card_lets_this_container_off(&about, &format!("broken-hostpath {name}"));
        let ending: Vec<&&Finding> = about
            .iter()
            .filter(|f| f.evidence.contains("exit 137"))
            .collect();
        assert_eq!(
            ending.len(),
            1,
            "{name}: one card from the ending — the record is on this container as much as on \
             its sibling, and rule 6 is exempt on both: {:?}",
            titles(&all)
        );
        let card = ending[0];
        assert!(
            card.title.contains(
                "restarted 4 times — it is serving now, and the record names the pod's rule"
            ),
            "{name}: the card carries its own count and the clause: {}",
            card.title
        );
        assert_eq!(
            card.action,
            restart_rule_action(),
            "{name}: and both get the sentence that exonerates neither of them — on this object \
             the trigger is one of these two and the record does not say which"
        );
        // **And each card is dated by its own container's run**, nine seconds apart on this
        // capture — the record they share carries no stamp at all, so a rule reading the wrong
        // one of the two would put one container's age on the other's card (NOTES § D100).
        assert_eq!(
            card.timestamp.as_ref(),
            Some(began),
            "{name}: the age is when *this* container's current run began"
        );
    }
}

/// **The cards this box ships, measured at the width they are drawn at** (`screens/alerts.md`
/// § How wide a card is, and how tall; NOTES § D95). A card is the identity line, the title, the
/// evidence capped at three lines, and the action — and **these cards measure ten**, which is
/// tighter than the pane's own cap of 12 and is asserted as their budget rather than as its limit
/// (NOTES § D113). The action is never cut, so a title that grows by one wrapped line spends a
/// line these cards have not got.
///
/// **The count is three digits because that is the realistic worst case, and not because the
/// count is what overflows the card.** The review's own kind cluster hit 132 restarts in ten
/// minutes on a pod whose restart rule was firing, so this is the number a real reader sees.
/// **The height does not move with it**: fed 7, 10, 132, 1320 and 999,999,999 every card here
/// measures the same ten lines, because the count sits on the title's first line, which has
/// slack, and the wrap redistributes around it. What decides the height is the *clause* — the
/// 42-character wording these replaced wrapped the title to three lines at every one of those
/// counts (NOTES § D95). Said plainly because the first draft of this comment claimed the count
/// was the cause, and a false rationale in a test is what the next reader inherits as fact.
///
/// **Measured off the cards [`analyze`] actually draws, not off copies of the strings.** A test
/// that re-typed the title would measure itself, and the wording it is guarding lives in a match
/// arm that no function exposes.
///
/// **It measures the cards these boxes ship and no others** — rule 1's, and rule 5's in both of
/// its branches, the second of which arrived with the clause on 2026-08-16 (NOTES § D102). Five
/// actions elsewhere in the file were over the cap while this stood, which is precisely what a
/// per-box guard cannot see: [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`]
/// is the sweep that does, and it is what caught them (NOTES § D113). What is left here is the
/// tighter per-card number these four shapes hold.
#[test]
fn the_cards_this_box_ships_fit_the_height_they_are_drawn_at() {
    // `screens/alerts.md` § The columns: body text 51, action continuations 49 — and § The
    // height: 1 identity + title + evidence (cut at three) + action.
    const BODY_COLUMNS: usize = 51;
    const EVIDENCE_CAP: usize = 3;
    // **Ten and not the pane's twelve**: these four shapes measure ten today, and holding them
    // there is a tighter claim than the cap — the cap is
    // [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`]'s (NOTES § D113).
    const CARD_LINES: usize = 10;
    // The realistic worst case a cluster reaches, and the absurd bound either side of it — fed
    // as a range rather than as one number, because the claim in this test's own doc is that the
    // height does *not* move with the count and a claim nothing measures is the lore this comment
    // was rewritten to remove (NOTES § D40, § D95).
    const COUNTS: [i32; 4] = [7, 132, 1320, 999_999_999];

    // **The code travels with the reason**, because the third ending is not another `137`:
    // `CODE_UNKNOWN` is `255`, and a pair no kubelet writes would be measuring a card no cluster
    // draws (NOTES § D29, § D95).
    // CRI-O writes the third ending with a different pair and its own translation, so its cards
    // are measured rather than assumed to match containerd's.
    const ENDINGS: [(&str, i32); 4] = [
        (STATUS_LOST, 137),
        (RESTART_ALL, 137),
        (CODE_UNKNOWN, 255),
        ("Error", -1),
    ];
    let mut measured = 0usize;
    for ((reason, code), count) in ENDINGS.into_iter().flat_map(|r| COUNTS.map(|n| (r, n))) {
        // Rule 1's card, on the wait; rule 5's serving card, where the clause prints at all.
        // Both carry `count`, so the `n=` in the output below is true of the card beside it —
        // `crashloop.json` is captured in `CrashLoopBackOff`, so only the ending and the count
        // are planted (NOTES § D40).
        let looping = capture_but("crashloop", |p| {
            ended_as(p, "quitter", code, Some(reason), None);
            container_status(p, "quitter").restart_count = count;
        });
        let serving = capture_but("restarts10serving", |p| {
            ended_as(p, "flaky", code, Some(reason), None);
            container_status(p, "flaky").restart_count = count;
        });
        // **Rule 5's *down* card, which is the one every fold leaves standing** and the one whose
        // title took the clause on 2026-08-16 (NOTES § D102). It is measured here because that
        // change is a title-line change and this test is where a title-line change is priced —
        // the argument for making it was the height, so the height is checked and not asserted.
        let down = capture_but("restarts10", |p| {
            ended_as(p, "flaky", code, Some(reason), None);
            container_status(p, "flaky").restart_count = count;
        });
        // The serving card is measured inside the run it is drawn on, because it ages out at
        // `NOT_READY_GRACE` (NOTES § D100); the looping container is waiting, and rule 1's card
        // never ages out. The height is a property of the wording either way.
        let serving_moment = into_the_run(&serving, "flaky", 5);
        for (pod, moment) in [(looping, now()), (serving, serving_moment), (down, now())] {
            let object = pod.id.name.clone();
            for card in analyze(&pods_at(vec![pod], moment)) {
                // Rules 1 and 5 put the ending in the evidence; this box's other card is rule
                // 6's, which is measured by the box that wrote it.
                if !card.evidence.contains(&format!("exit {code}")) {
                    continue;
                }
                let title = wrapped_at(&card.title, BODY_COLUMNS).len();
                let evidence = wrapped_at(&card.evidence, BODY_COLUMNS)
                    .len()
                    .min(EVIDENCE_CAP);
                let action = wrapped_at(&card.action, ACTION_COLUMNS).len();
                let height = 1 + title + evidence + action;
                println!(
                    "{object} {reason} exit {code} n={count}: {height} lines — 1 + {title} title + \
                     {evidence} evidence + {action} action\n  {}\n  {}",
                    card.title, card.action
                );
                assert!(
                    height <= CARD_LINES,
                    "{object} {reason} exit {code} n={count}: a {height}-line card, and these \
                     shapes are budgeted at \
                     {CARD_LINES} — the title wraps to {title} lines at {BODY_COLUMNS} columns. \
                     The pane's own cap is 12; this is the tighter claim these four cards hold \
                     (`screens/alerts.md` § The height): {}",
                    card.title
                );
                measured += 1;
            }
        }
    }
    // Written down rather than summed: a card that stops being drawn takes its own measurement
    // with it and subtracts from a total nobody reads (NOTES § D26).
    assert_eq!(
        measured,
        ENDINGS.len() * COUNTS.len() * 3,
        "the three cards these boxes ship — rule 1's, and rule 5's in both of its branches — on \
         each ending at each count"
    );
}

/// **Pairs of claims that cannot both be true of one container** — the guard this family owed and
/// did not have (NOTES § D113).
///
/// **Every existing guard hunts two rules saying the *same* sentence; none hunts two rules saying
/// sentences that deny each other.** [`one_card_per_action`]'s inventory is keyed on equality, and
/// the fold's whole purpose is to collapse a repetition — so a repetition is loud and a
/// **contradiction is silent**. This family shipped one twice with 219 tests green and `cargo
/// mutants` clean both times, which is what a hole in the guard set looks like from inside it.
///
/// **Each row is a defect that reached a card, not a hypothetical.** The left phrase is a claim
/// some card makes; the right is a phrase that denies it. Both are matched over the *whole* card
/// set of one container — title, evidence and action together — because the two halves have
/// landed on one card as often as on two.
///
/// - `OOMKilled` beside *not always labelled as one*: [`crash_looping`] took [`killed_action`] on
///   the code alone, and `oom.json` drew a CRITICAL card hedging about a label its own evidence
///   line printed, above [`out_of_memory`] asserting it in a title.
/// - *the command was not found* beside *check the memory limit*: [`crash_looping`]'s first
///   shared-answer draft on `notfound.json`.
/// - *could not be run* beside *not in the image*: the same fix keyed on the exit code, which put
///   both halves on **one** card.
const CANNOT_BOTH_HOLD: [(&str, &str); 3] = [
    ("(OOMKilled)", "not always labelled"),
    ("the command was not found", "check the memory limit"),
    ("could not be run", "not in the image"),
];

/// **No two cards about one container may deny each other** ([`CANNOT_BOTH_HOLD`], NOTES § D113).
///
/// **Swept per container over the whole corpus**, the same shapes
/// [`every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for`] measures, because a
/// contradiction is a property of a card *set* and every other guard in this file reads one card
/// or one rule.
///
/// **The rows are asserted reachable in both halves**, or the guard degrades into a green line
/// over phrases nothing produces: each left phrase has to appear somewhere in the corpus, and each
/// right phrase too. A row whose halves have both stopped being said is a row to delete
/// deliberately, not one to leave passing (CLAUDE.md § A derived list asserts it found something).
#[test]
fn no_two_cards_about_one_container_deny_each_other() {
    let forgotten = capture_but("restarts10serving", |p| {
        container_status(p, "flaky").last_state = None;
    });
    let mut pods = every_shape_a_container_reaches();
    pods.extend(fixture_snapshot().pods);
    pods.push(forgotten);
    pods.extend(ENDING_PLANTS.iter().map(|&(code, reason)| {
        capture_but("restarts10", |p| {
            ended_as(p, "flaky", code, reason, None);
            container_status(p, "flaky").restart_count = 1;
        })
    }));
    // **The container the runtime never started**, which no other corpus shape reaches:
    // [`ended_as`] writes real stamps, correctly, and the epoch `startedAt` containerd leaves has
    // no committed capture (NOTES § D40, § D112). It is the only producer of *not in the image*,
    // and without it the canary below says so — which is the canary doing its job on its first
    // run.
    pods.push(capture_but("crashloop", |p| {
        ended_as(p, "quitter", 128, Some("StartError"), None);
        container_status(p, "quitter")
            .last_state
            .as_mut()
            .and_then(|t| t.terminated.as_mut())
            .expect("ended_as wrote the run this plant is stamping")
            .started_at = Some(time("1970-01-01T00:00:00Z"));
    }));

    let mut said: BTreeSet<&str> = BTreeSet::new();
    let mut denied: Vec<String> = Vec::new();
    for pod in pods {
        // **Per container, because that is the scope a reader takes in**: two cards about two
        // containers saying opposite things are two containers, and only the pod links them.
        for c in &pod.containers {
            let about: Vec<String> = analyze(&pods_at(vec![pod.clone()], now()))
                .into_iter()
                .filter(|f| f.evidence.contains(&container_fact(c)))
                .map(|f| format!("{} · {} · {}", f.title, f.evidence, f.action))
                .collect();
            for (claim, denial) in CANNOT_BOTH_HOLD {
                let claimed = about.iter().any(|card| card.contains(claim));
                let refused = about.iter().any(|card| card.contains(denial));
                if claimed {
                    said.insert(claim);
                }
                if refused {
                    said.insert(denial);
                }
                if claimed && refused {
                    denied.push(format!(
                        "{} / {}: {claim:?} and {denial:?} are both on this container's cards\n  {}",
                        pod.id.name,
                        c.name,
                        about.join("\n  ")
                    ));
                }
            }
        }
    }
    println!("{} phrases reached: {said:?}", said.len());
    assert!(
        denied.is_empty(),
        "two cards about one container deny each other, and no other guard in this file can see \
         it — the fold is keyed on equality, so a repetition is loud and a contradiction is \
         silent (NOTES § D113):\n{}",
        denied.join("\n")
    );
    for (claim, denial) in CANNOT_BOTH_HOLD {
        for half in [claim, denial] {
            assert!(
                said.contains(half),
                "{half:?} is on no card in the corpus, so the row it is in guards nothing — \
                 delete the row deliberately or reach the shape it is about"
            );
        }
    }
}

/// **Every card the whole rule set draws, against all four of `screens/alerts.md`'s caps** — the
/// guard the two tests either side of it are not: each of those measures the cards *one box*
/// ships, which is why four actions and one title went over the budget while both stayed green
/// (NOTES § D113).
///
/// **The caps are the file's, transcribed rather than parsed**: identity 1 line, title ≤ 3,
/// evidence ≤ 3 (the one part the pane cuts), action ≤ 5, and **12** for the card — which is the
/// four added up, and is 16 body rows less the separator and the three the next finding's identity
/// and title get. `alerts.md` moving does not turn this red, which is the trade the two boxes
/// before it already made.
///
/// **It lives in this module because the measure does** — [`wrapped_at`] and [`ACTION_COLUMNS`]
/// are here — but it reads the *whole* snapshot, nodes, workloads and the kubeconfig certificate
/// included, so a node card that grew a sixth action line is caught here and nowhere else.
///
/// **The corpus is the committed captures plus every planted ending**, and three shapes none of
/// them reaches. A container with a restart count and **no `lastState` at all**, in both of the
/// states rules 1 and 5 read — that arm's action was one of the four over the cap. And **rule 6
/// drawing alone**, which needs a count *below* [`RESTARTS_WARN`] on a container that is not
/// waiting: every other shape in the corpus gives it a neighbour, and on the two endings all three
/// rules answer with one sentence [`one_card_per_action`] then folds rule 6's card away — which is
/// how the one title over the cap stayed invisible to a sweep that read only [`analyze`]'s output.
///
/// **Distinct cards are counted and printed.** Two rules drawing one wording measure once, and the
/// count is asserted to be non-trivial: a sweep that read an empty corpus prints the same green
/// line as one that read every card (CLAUDE.md § A derived list asserts it found something).
#[test]
fn every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for() {
    const BODY_COLUMNS: usize = 51;
    const TITLE_CAP: usize = 3;
    const EVIDENCE_CAP: usize = 3;
    const ACTION_CAP: usize = 5;
    const CARD_LINES: usize = 12;

    // The count with no run behind it, in rule 5's state and in rule 1's — the only arm of the
    // whole file that neither the captures nor [`every_shape_a_container_reaches`] produces.
    let forgotten = |capture: &str, name: &'static str| {
        capture_but(capture, |p| {
            container_status(p, name).last_state = None;
            container_status(p, name).restart_count = RESTARTS_WARN + 7;
        })
    };
    let serving = capture_but("restarts10serving", |p| {
        container_status(p, "flaky").last_state = None;
    });
    let moment = into_the_run(&serving, "flaky", 5);
    // **Rule 6 with nobody beside it**: one restart is below the band rule 5 reads, and
    // `restarts10.json`'s container is not waiting, so rules 1 and 5 are both silent and the fold
    // has nothing to collapse this card into.
    let mut alone: Vec<PodSnapshot> = ENDING_PLANTS
        .iter()
        .map(|&(code, reason)| {
            capture_but("restarts10", |p| {
                ended_as(p, "flaky", code, reason, None);
                container_status(p, "flaky").restart_count = 1;
            })
        })
        .collect();
    // **And the record the cluster wrote that no rule author can bound** — containerd's own error
    // for a mistyped `command`, measured on kind v1.36.1 (NOTES § D113,
    // `reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1). It is 7 wrapped lines at
    // 49 columns, so while it *was* an action this sweep is what a card built from it fails on:
    // an author can measure what they wrote and nobody can measure what a runtime will write.
    alone.push(capture_but("restarts10", |p| {
        ended_as(
            p,
            "flaky",
            128,
            Some("StartError"),
            Some(RUNTIME_START_FAILURE),
        );
        container_status(p, "flaky").restart_count = 1;
    }));

    let mut cards = analyze(&fixture_snapshot());
    cards.extend(analyze(&pods_at(every_shape_a_container_reaches(), now())));
    cards.extend(analyze(&pods_at(
        vec![
            forgotten("restarts10", "flaky"),
            forgotten("crashloop", "quitter"),
        ],
        now(),
    )));
    cards.extend(analyze(&pods_at(vec![serving], moment)));
    cards.extend(analyze(&pods_at(alone, now())));

    let mut distinct: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut over: BTreeSet<String> = BTreeSet::new();
    for card in &cards {
        if !distinct.insert((
            card.title.clone(),
            card.evidence.clone(),
            card.action.clone(),
        )) {
            continue;
        }
        let title = wrapped_at(&card.title, BODY_COLUMNS).len();
        let evidence = wrapped_at(&card.evidence, BODY_COLUMNS)
            .len()
            .min(EVIDENCE_CAP);
        let action = wrapped_at(&card.action, ACTION_COLUMNS).len();
        let height = 1 + title + evidence + action;
        println!("{height} lines = 1 + {title} title + {evidence} evidence + {action} action");
        println!("  {}\n  → {}", card.title, card.action);
        if title > TITLE_CAP {
            over.insert(format!("title {title} lines: {}", card.title));
        }
        if action > ACTION_CAP {
            over.insert(format!(
                "action {action} lines ({} chars): {}",
                card.action.chars().count(),
                card.action
            ));
        }
        if height > CARD_LINES {
            over.insert(format!("card {height} lines: {}", card.title));
        }
    }
    println!("{} distinct cards measured", distinct.len());
    // **Named entries, not a floor** (CLAUDE.md § A derived list asserts it found something,
    // NOTES § D113). `len() > 40` against a real 143 passed with any single corpus contribution
    // deleted — the rule-6-alone shapes, the no-record plants, the serving moment and the whole
    // looping half — while this test's own doc argues each one is what makes a card reachable. A
    // floor with a hundred cards of slack measures nothing; one line per claim the doc makes does.
    let says = |needle: &str| {
        distinct
            .iter()
            .any(|(t, e, a)| t.contains(needle) || e.contains(needle) || a.contains(needle))
    };
    for (contribution, entry) in [
        // The `alone` plants: rule 6 with no neighbour is the only way its title reaches a sweep
        // reading [`analyze`]'s output, and that title is the one that was over the cap.
        ("rule 6 drawing alone", "has no exit code of its own"),
        // …and the runtime message on one of them, which is the string a bounded budget cannot be
        // written against and the reason the action is never a quote.
        (
            "the runtime's own error",
            "failed to create containerd task",
        ),
        // The `forgotten` plants: the arm with a count and no record at all. **Rule 1's title and
        // not the shared sentence** — rule 5 answers this shape with the same words off the
        // `serving` plant, so keying on the action left this contribution deletable with the sweep
        // still green.
        (
            "a count with no record",
            "the run that ended is not on the pod (CrashLoopBackOff)",
        ),
        // The `serving` moment: rule 5's card ages out at `NOT_READY_GRACE`, so at the pin it is
        // not drawn and the sweep never sees the serving title.
        ("the serving card", "it is serving now"),
        // The `looping = true` half of [`every_shape_a_container_reaches`].
        ("the backoff half", "CrashLoopBackOff"),
        // And the node and workload cards, which come from the whole snapshot rather than the pods.
        ("the node rules", "This node refuses new pods"),
    ] {
        assert!(
            says(entry),
            "{contribution} is gone from the corpus — {entry:?} is on no card, so the part of \
             this sweep that covers it is measuring nothing"
        );
    }
    assert!(
        over.is_empty(),
        "a part over its cap is a `rules.rs` finding and not a layout problem \
         (`screens/alerts.md` § The height — title {TITLE_CAP}, action {ACTION_CAP}, card \
         {CARD_LINES}):\n{}",
        over.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// **The two readings [`ending`] gained on 2026-08-15, asserted on the function itself** — the
/// root the three rules above all read, and the reason the fix is one change rather than three
/// (NOTES § D95).
///
/// **What this proves is that the number alone does not decide**: every row below holds the code
/// at `137` or gives the reason as `None`, so what separates them is the reason. `0` and `143`
/// are the same ending whatever is written beside them, and `OOMKilled` stays
/// [`Failed`](Ending::Failed) on purpose — rule 2 owns the labelled kill and *something keeps
/// killing it* is true of it, so rules 1 and 5 need no arm for it.
///
/// **The other direction is deliberately unguarded, and the name used to claim otherwise.** This
/// was `ending_reads_the_reason_beside_the_code_and_not_the_number_alone` until `tester` widened
/// both arms from `(137, Some(reason))` to `(_, Some(reason))` and the suite stayed green: no row
/// feeds `(1, Some(STATUS_LOST))`, because **the kubelet never writes that pair** and asserting
/// behaviour on an object the API cannot produce is what NOTES § D29 and § D95 refuse.
/// [`ending`]'s own doc says as much where it records the three unreachable pairs that moved. The
/// gap stays; the **name** had to go, because a name is evidence to the next reader
/// (NOTES § D26).
///
/// **Read off a planted object rather than a literal**, for the reason the decode tests give: the
/// pairing of a code with a reason is the API's, not this file's ([`ended_as`], NOTES § D40).
///
/// **And the two spellings are pinned before anything is planted from them**, because every shape
/// in this file writes the reason *out of the same constant it is then matched against*: a typo in
/// either one ships a rule that never fires against a real cluster and a suite that stays green
/// about it. `tester` proved it — `STATUS_LOST` misspelled as `"containerstatusunknown"` and
/// `RESTART_ALL` as `"RESTARTINGALLCONTAINERS"` both passed 184 tests. It is the
/// `scripts/write-guard.py` `CANARIES` class one level down (CLAUDE.md § Code phase rules).
///
/// **The two are pinned by different evidence, and the difference is stated rather than blurred**
/// (NOTES § D40):
///
/// - **[`STATUS_LOST`] is captured.** `failed.json` carries the kubelet's own object on
///   `broken-failed` / `app` — `exitCode: 137`, the reason spelled out, both stamps `null` and the
///   sentence the kubelet writes beside it — so the spelling *and* the `137` it pairs with are read
///   off a real cluster's bytes rather than off this file. **It sits in `state.terminated`, which
///   no rule reads yet**: that is the next box, and it is why this capture proves the string and
///   not the rule. The bytes are read, never edited (NOTES § D53).
/// - **[`RESTART_ALL`] has no capture at all.** It was measured on kind and never captured, so the
///   pin below is the literal spelling against `kubelet_pods.go` at v1.36.1 — **a source-derived
///   pin, not a captured one.** No fixture is invented for it (NOTES § D29, § D93).
#[test]
fn the_reason_and_not_the_number_alone_decides_which_ending_it_is() {
    // The captured half. `broken-failed`'s container is *currently* terminated, so the object
    // arrives as [`ContainerState::Terminated`] rather than in `last_terminated`.
    let capture = pod("failed");
    let app = container(&capture, "app");
    println!("failed.json, state.terminated: {:?}", app.state);
    let ContainerState::Terminated(lost) = &app.state else {
        panic!("failed.json's container is terminated where the kubelet lost its status: {app:?}")
    };
    assert_eq!(
        (lost.reason.as_deref(), lost.exit_code),
        (Some(STATUS_LOST), 137),
        "the constant is the string a cluster writes, and the code it is written beside — every \
         other shape in this file plants the reason from this same constant, so nothing else in \
         the suite would notice it being wrong: {lost:?}"
    );
    // The source-derived half: `kubelet_pods.go` at v1.36.1 writes this word, and no committed
    // capture holds it. Spelled out here so a typo in the constant has one place that disagrees.
    assert_eq!(
        RESTART_ALL, "RestartingAllContainers",
        "the reason the kubelet writes when a container's own `restartPolicyRules` remove the \
         containers of its pod — the field is a container's and the effect is the pod's \
         (NOTES § D96)"
    );
    // **And the third source-derived pin, which is not the kubelet's at all**: containerd's
    // `unknownContainerStatus()` is `{ExitCode: 255, Reason: "Unknown"}` and
    // `kuberuntime_container.go:760-763` copies it through, so another runtime spelling it
    // differently falls through to the bare number — a miss and never a lie.
    assert_eq!(
        CODE_UNKNOWN, "Unknown",
        "the reason a runtime writes for a container it found dead without an exit status, which \
         is what a node restart leaves behind"
    );

    for (code, reason, expected) in [
        (137, Some(STATUS_LOST), Ending::Unwatched),
        (137, Some(RESTART_ALL), Ending::RestartRule),
        (255, Some(CODE_UNKNOWN), Ending::CodeUnknown),
        // **The number alone still decides nothing here either**: a program that runs `exit -1`
        // in a shell reports 255, and that program really did fail.
        (255, None, Ending::Failed),
        // The three that were already here, and the one deliberately left among them.
        (137, Some("OOMKilled"), Ending::Failed),
        (137, None, Ending::Failed),
        (1, None, Ending::Failed),
        (0, None, Ending::Finished),
        (143, None, Ending::Stopped),
    ] {
        let planted = init_previous_run(code, reason, None, false);
        let run = container(&planted, "wait-for-db")
            .last_terminated
            .as_ref()
            .expect("the plant writes the previous run this reads");
        println!("exit {code} {reason:?} -> {:?}", ending(run));
        assert_eq!(
            ending(run),
            expected,
            "exit {code} beside {reason:?} is {expected:?} — a reading of `Failed` for either of \
             the kubelet's own two reasons is what put *keeps crashing* and *something keeps \
             killing it* on a run nothing killed, in three rules at once: {run:?}"
        );
    }
}

/// **The runtime is a third author, its messages carry `finishedAt`, and the frame stopped
/// claiming the container wrote the line** — box 966's own done-when, reached the way it said it
/// would be: *if it cannot tell the two apart, the sentence stops claiming authorship rather than
/// guessing it.*
///
/// **The stamp told the kubelet's four literals from a container's message and cannot tell the
/// runtime's from either.** containerd writes its start-failure error into `Message` beside a real
/// `FinishedAt` (`internal/cri/server/container_start.go:67-73`, measured — the card printed *the
/// last thing it logged was: failed to create containerd task: …* about a container that logged
/// nothing); CRI-O does the same for a stopped container whose exit code it could not read
/// (`server/container_status.go:107-130`, `Reason: "Error"`, `Message: cState.Error`). **No reason
/// separates them either**: `"Error"` is what an ordinary application failure carries.
///
/// **So the frame names who *recorded* the line and never who wrote it.** That is true of all
/// three authors, it is the one thing the object supports, and it names an owner rather than
/// leaving a bare noun on the card (invariant 14).
///
/// **The shape is a runtime-authored message on a *stamped* record**, which is the author
/// dimension no test in this family had ever been fed (NOTES § D29).
#[test]
fn the_quote_frame_says_who_recorded_the_line_and_never_who_wrote_it() {
    // The frame itself, before any card is read — every assertion below is about this string, and
    // a test that only looked at cards would pass on a frame that had quietly gone empty.
    let framed = last_words("panic: cannot reach db");
    println!("{framed}");
    assert!(
        framed.ends_with(": panic: cannot reach db"),
        "the line still reaches the reader, and still after a colon: {framed}"
    );
    for claimed in ["logged", "it said", "its own words", "wrote"] {
        assert!(
            !framed.to_lowercase().contains(claimed),
            "{claimed:?} claims an author the object does not name — the kubelet, the container \
             and the runtime all reach this field and nothing on the record tells them apart: \
             {framed}"
        );
    }
    assert!(
        framed.starts_with("Kubernetes recorded"),
        "and the positive, or *ask a friend* passes every line above: the card names who put the \
         line there, which is the one thing that is true whoever wrote it: {framed}"
    );

    // **The shape the old frame lied about**, measured on kind v1.36.1 with
    // `command: ["/definitely-not-here"]`: containerd's own error, on a record with a real
    // `finishedAt` that the stamp guard therefore lets through
    // (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1).
    let runtime_said = RUNTIME_START_FAILURE;
    let start_failed = capture_but("restarts10", |p| {
        let run = container_status(p, "flaky")
            .last_state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the capture records a previous run");
        run.exit_code = 128;
        run.reason = Some("StartError".to_string());
        run.started_at = Some(time("1970-01-01T00:00:00Z"));
        run.message = Some(runtime_said.to_string());
    });
    let run = container(&start_failed, "flaky")
        .last_terminated
        .clone()
        .expect("the plant writes the run this reads");
    assert!(
        run.finished_at.is_some() && last_log_line(&run) == Some(runtime_said),
        "the stamp guard lets this one through — which is the point: it was written to tell the \
         kubelet's placeholders from a container's words, and the runtime is neither: {run:?}"
    );
    let all = analyze(&pods_at(vec![start_failed], now()));
    show(&all);
    let quoted = cards_about(&all, "flaky")
        .into_iter()
        .find(|f| f.evidence.contains(runtime_said))
        .expect("rule 6 puts the message on the card, framed");
    assert!(
        quoted
            .evidence
            .split(FACTS)
            .any(|fact| fact == last_words(runtime_said)),
        "one frame, and rule 6 does not spell a second copy of it: {}",
        quoted.evidence
    );
    // **And this is the object that took the quote off the action line** (NOTES § D113). A
    // mistyped `command` is one of the commonest broken-pod states there is, and its whole *what
    // to do* was seven wrapped lines of containerd's `runc` error — telling a beginner nothing to
    // do (invariant 14), and unbounded, so the five-line budget could not be enforced while it
    // stood there. `128` is what the kubelet records for it, and the advice is the one `126` and
    // `127` already got.
    assert_eq!(
        quoted.action,
        "check the container's command and arguments — what they name is not in the image",
        "the card names the thing to look at in k8rs's own words, with the runtime's on the line \
         above it"
    );

    // **The control: a message the container really did write is still printed** (NOTES § D26).
    // `crashloop.json` carries the log tail under `terminationMessagePolicy:
    // FallbackToLogsOnError` — the shape the frame exists for — and what changed is which line it
    // lands on, not whether it reaches the reader.
    let told = analyze(&pods_at(vec![pod("crashloop")], now()));
    let card = only(&told, "broken-crashloop", "on record failed");
    println!("{} | {}", card.evidence, card.action);
    assert!(
        card.evidence.split(FACTS).any(|fact| fact
            == last_words("panic: dial tcp db.payments.svc:5432: connect: connection refused")),
        "the container's own last line still reaches the card whole, under the same frame: {}",
        card.evidence
    );
}

/// **A container whose *start* failed carries the epoch as its `startedAt`, and the card said it
/// had run for 20 681 days** — measured on kind v1.36.1 with `command: ["/definitely-not-here"]`
/// (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1), and still the epoch seven
/// restarts later.
///
/// **The zero survives the whole path because nothing on it is an `Option`.** containerd
/// (`internal/cri/server/container_start.go:67-73`) sets `FinishedAt`, `ExitCode`, `Reason` and
/// `Message` on a start failure and leaves `StartedAt` at `0`; the kubelet writes
/// `metav1.NewTime(cs.StartedAt)` unconditionally, and `time.Unix(0, 0)` is **not** Go's zero
/// time, so it marshals as a real RFC3339 stamp rather than as `null`. [`Finding::timestamp`]'s
/// own note said this could not happen — *an `Option` and not a zero* — which is the sentence
/// that made the shape unthinkable.
///
/// **A mistyped command is one of the commonest broken-pod states there is**, and it reaches
/// rules 1, 6 and 15 alike, so the guard is in [`lasted`] where all three route through.
///
/// **What it is not is *ran for 0s***: nothing ran. The card says nothing about a duration, and
/// the exit code and the message still carry the diagnosis.
#[test]
fn a_container_that_never_started_is_not_one_that_ran_since_the_epoch() {
    // The object as the cluster wrote it, field for field. `restarts10.json` is the base because
    // its container already has a previous run for the plant to rewrite (NOTES § D40, § D53).
    let failed_to_start = capture_but("restarts10", |p| {
        let run = container_status(p, "flaky")
            .last_state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the capture records a previous run");
        run.exit_code = 128;
        run.reason = Some("StartError".to_string());
        run.started_at = Some(time("1970-01-01T00:00:00Z"));
        run.finished_at = Some(time("2026-08-16T01:19:26Z"));
        run.message = Some(
            "failed to create containerd task: failed to create shim task: OCI runtime create \
             failed: runc create failed: unable to start container process: error during \
             container init: exec: \"/definitely-not-here\": stat /definitely-not-here: no such \
             file or directory"
                .to_string(),
        );
    });
    let run = container(&failed_to_start, "flaky")
        .last_terminated
        .clone()
        .expect("the plant writes the run every assertion below reads");
    println!("{run:?}");
    assert_eq!(
        run.started_at.as_ref().map(|t| t.0),
        Some(Timestamp::UNIX_EPOCH),
        "the shape under test is the epoch arriving as a *value*, not as a missing field — a \
         plant that left it `None` would prove the wrong guard: {run:?}"
    );
    assert!(
        run.finished_at.is_some(),
        "and the other stamp is real, which is why the subtraction happens at all: {run:?}"
    );
    // **The arithmetic that was shipping**, so the assertion below is not guarding a number
    // nobody would have printed.
    println!("what the old reading would have said: {:?}", {
        let d = run
            .finished_at
            .as_ref()
            .unwrap()
            .0
            .duration_since(Timestamp::UNIX_EPOCH);
        format!("{} days", d.as_hours() / 24)
    });
    assert_eq!(
        lasted(&run),
        None,
        "a run that never started has no duration — *ran for 20681 days* is what the epoch \
         subtracts to, and *ran for 0s* would be the other wrong answer"
    );
    assert_eq!(ran_for(&run), None, "so no card carries the clause at all");

    // **Every card the pod draws, because the guard is in the shared helper and all three rules
    // that print a duration route through it** (NOTES § D29).
    let all = analyze(&pods_at(vec![failed_to_start], now()));
    show(&all);
    let about = cards_about(&all, "flaky");
    assert!(
        !about.is_empty(),
        "the rules speak for this container, or the negatives below are about an empty screen: \
         {:?}",
        titles(&all)
    );
    for f in &about {
        assert!(
            !f.evidence.contains("ran for") && !f.evidence.contains("day"),
            "no card may say how long a run that never started lasted: {} / {}",
            f.title,
            f.evidence
        );
    }
    // **The control**: the same record with a real `startedAt` still measures, or the guard is a
    // delete rather than a discrimination (NOTES § D26).
    let mut ran = run.clone();
    ran.started_at = Some(time("2026-08-16T01:19:00Z"));
    assert_eq!(
        ran_for(&ran).as_deref(),
        Some("ran for 26s"),
        "and a run that did start is still measured — the epoch is the whole of what changed"
    );
}

/// **The commonest abnormal `lastState` any cluster produces, and it was reading as the
/// application's own failure** — `exit 255` beside [`CODE_UNKNOWN`], measured on kind v1.36.1 by
/// restarting the node's container.
///
/// **It is containerd's pair and not the kubelet's**, which is the one thing that keeps this
/// narrower than *255 means a reboot*: `unknownContainerStatus()` in containerd's CRI plugin is
/// `{ExitCode: 255, Reason: "Unknown"}`, and `kuberuntime_container.go:760-761` copies both
/// straight through. Another runtime spelling it differently falls through to the bare number —
/// a miss, never a lie — and that is why the code is read **beside its reason**, exactly as `137`
/// is. The box asked for a row on the number alone; a program that runs `exit -1` in a shell
/// reports 255 too, and a code-alone row would tell that reader their program did not fail.
///
/// **What the object supports here is more than [`Unwatched`](Ending::Unwatched) supports**: the
/// containers are *found*, dead, so the record carries real stamps and a real `containerID` and
/// `logs --previous` works. What it does not carry is a code anybody read, which is the one thing
/// every card about it turned into the application's error.
///
/// **Every role and both states**, because the arm is role-blind and a check is proven only for
/// the shapes it was fed (NOTES § D29) — and role-blindness is what keeps it clear of
/// `validateInitContainers`: a sentence naming no probe cannot name one an init container may not
/// have.
#[test]
fn a_node_reboot_does_not_read_as_the_application_failing() {
    // **The number alone still means nothing** (the spelling of [`CODE_UNKNOWN`] is pinned
    // beside the other two in
    // [`the_reason_and_not_the_number_alone_decides_which_ending_it_is`], which is where every
    // constant this file plants from is checked against something that is not itself), which is the half the box would have got wrong:
    // `exit -1` in a shell reports 255 and that program really did fail.
    assert_eq!(
        exit_meaning(255, None),
        None,
        "255 with no reason beside it is a code with no accepted meaning, and the number alone is \
         the honest answer — a row keyed on the number would tell a reader whose program exited \
         -1 that it did not fail"
    );
    // **One ending, two translations, because the runtimes tell different stories.** containerd
    // found the container dead; CRI-O could not work out what it ended with. Both rows say the
    // number is a stand-in, and neither may say the application failed.
    for (code, reason, story) in [
        // **The pin is one word, and the assertion beside it is what states the requirement**
        // (NOTES § D113). *already* came out on 2026-08-16 — the one word between
        // [`previous_run_failed`]'s title and `screens/alerts.md`'s three-line cap — and the first
        // rewrite replaced a two-word pin with a four-word one, which is a test demanding a
        // phrasing where its own message asks for a fact. What the row needs is that the node
        // found the container *dead*; *stands in* and the two negatives are asserted for every
        // row below.
        (255, Some(CODE_UNKNOWN), "dead"),
        (-1, Some("Error"), "could not tell"),
    ] {
        let stood_in = exit_meaning(code, reason)
            .expect("the ending is translated for the reader")
            .to_lowercase();
        println!("exit {code} {reason:?}: {stood_in}");
        assert!(
            stood_in.contains("stands in")
                && stood_in.contains(story)
                && !stood_in.contains("crash")
                && !stood_in.contains("fail"),
            "exit {code}: the row says what the number is — a stand-in the node wrote — and what \
             the node knew, and never that the application failed: {stood_in}"
        );
    }

    // **Both runtimes, because the ending is one and the pair is not** — containerd's
    // `(255, "Unknown")` and CRI-O's `(-1, "Error")`, whose reason is what an ordinary
    // application failure carries and whose *code* is therefore the key (NOTES § D29).
    for (code, reason) in [(255, CODE_UNKNOWN), (-1, "Error")] {
        for looping in [false, true] {
            for (role, name, planted) in every_role_with(code, Some(reason), looping) {
                let object = planted.id.name.clone();
                let subject = container(&planted, name);
                println!("=== {object} {role:?} looping={looping}\n{subject:?}");
                let run = subject
                    .last_terminated
                    .as_ref()
                    .expect("the plant writes the previous run every card below reads");
                // **The record is a real one, and that is the difference from [`STATUS_LOST`]**:
                // containerd found the containers rather than losing them, so the stamps survive the
                // reboot. A plant that stripped them would be proving these cards against the *other*
                // shape (NOTES § D29, § D40).
                assert!(
                    subject.role == role
                        && run.started_at.is_some()
                        && run.finished_at.is_some()
                        && ending(run) == Ending::CodeUnknown,
                    "{role:?}: the role under test, the stamps the reboot leaves behind, and the \
                 ending they add up to: {subject:?}"
                );
                let all = analyze(&pods_at(vec![planted], now()));
                show(&all);
                let about = cards_about(&all, name);
                assert!(
                    !about.is_empty(),
                    "{object} {role:?} exit {code}: some card speaks for this container, or every negative below \
                 is asserted about a screen with nothing on it (NOTES § D26): {:?}",
                    titles(&all)
                );
                for f in &about {
                    let said = format!("{} {} {}", f.title, f.evidence, f.action).to_lowercase();
                    // **The two sentences the box named, and the class they belong to.** *The
                    // last run on record failed* over *read the logs of that run to find the
                    // application's own error* sends a reader hunting an error the application never
                    // made, after a machine restart; *keeps crashing* is the same claim in rule 1's
                    // words. Nothing on this pod's screen may say the application failed.
                    for blamed in [
                        "keeps crashing",
                        "the application's own error",
                        "something keeps killing it",
                        "run failed",
                    ] {
                        assert!(
                            !said.contains(blamed),
                            "{object} {role:?} exit {code}: {blamed:?} — the number is a stand-in the node wrote \
                         because it could not read the real one, and blaming the application for \
                         it is this box's whole subject: {} / {} / {}",
                            f.title,
                            f.evidence,
                            f.action
                        );
                    }
                    // **Role-blind, so the init container is covered by construction** — and the
                    // negative is still fed, because *would obviously handle it* is not a shape it
                    // was fed (NOTES § D29, § D85). **Asked of the cards drawn from the ending only**,
                    // which is a scope and not a hole: on `restarts10.json` rule 7 also fires, and
                    // its card names the readiness probe about the container's readiness *now* — a
                    // true card about a different question ([`no_card_reads_this_run_as_a_kill`]).
                    if !said.contains(&format!("exit {code}")) {
                        continue;
                    }
                    for door in PROBE_WORDS {
                        assert!(
                            !said.contains(door),
                            "{object} {role:?} exit {code}: {door:?} — Kubernetes allows a plain \
                             init container no health check, and this card prints on one: {}",
                            f.action
                        );
                    }
                    // **A card may not deny on one line what it prints on the next.** Every card
                    // reaching here carries the exit code, so a title claiming it is *absent*
                    // contradicts its own evidence. The qualifier is what makes the claim true,
                    // and rule 5 shipped one turn without it while rules 1 and 6 had it — the
                    // three words were doing all the work (NOTES § D85).
                    if f.title.to_lowercase().contains("exit code") {
                        assert!(
                            f.title.contains("of its own") || f.title.contains("not its own"),
                            "{object} {role:?} exit {code}: the card prints the code one line \
                             down, so the honest claim is that it is not the container's — never \
                             that there is none: {} / {}",
                            f.title,
                            f.evidence
                        );
                    }
                }
                // **And the card that reads the ending says the positive.** Every assertion above is
                // a negative, and a set of negatives is satisfied by a card that says nothing at all —
                // the hole `tester` shipped past 184 tests once already (NOTES § D95).
                // **Picked by *where* the code sits and not by the sentence under test**: rules 1
                // and 5 put [`exit_fact`] on the evidence line, rule 6 on the title.
                let counted = about
                    .iter()
                    .find(|f| f.evidence.contains(&format!("exit {code}")))
                    .expect("rule 1 or rule 5 speaks for this container");
                assert_eq!(
                    counted.action,
                    no_exit_code_action(),
                    "{object} {role:?} exit {code}: the three rules that draw off `lastState` answer this ending \
                 with one sentence, which is what stops them contradicting each other about one \
                 container (NOTES § D85). **A fourth rule reads the ending and is not in this \
                 corpus**: rule 15 reads `state.terminated` under `restartPolicy: Never`, which \
                 nothing [`every_role_with`] builds, and it draws a sentence of its own — pinned \
                 in [`a_container_stopped_for_good_inside_a_running_pod_draws_a_card_that_names_its_log`], \
                 where its shapes are"
                );
                // **The action may name only what its own command shows** (invariant 4, NOTES
                // § D93). The command is [`describe`], which prints the pod's events and no logs at
                // all — so *did the node restart* is a door and any log is not, however servable that
                // log happens to be on this record. **And *the previous run* is the phrase A4 took
                // off five titles**: `lastState` freezes, so on a container at fifteen restarts
                // `logs --previous` serves run 15 while this card describes run 7. The first draft
                // of this sentence broke both rules at once.
                assert_eq!(
                    counted.kubectl_cmd.as_deref(),
                    describe(&counted.object).as_deref(),
                    "{object} {role:?} exit {code}: the card offers the output its own action names"
                );
                let said = counted.action.to_lowercase();
                assert!(
                    said.contains("node") && said.contains("events"),
                    "{object} {role:?} exit {code}: the machine that is the real subject, and the output that \
                 can answer for it: {}",
                    counted.action
                );
                for promised in ["log", "previous run", "--previous"] {
                    assert!(
                        !said.contains(promised),
                        "{object} {role:?} exit {code}: {promised:?} — `describe` prints no logs, and `lastState` \
                     is not the run before this one: {}",
                        counted.action
                    );
                }
                // **One card, on every shape** ([`one_card_per_action`], NOTES § D102). Rule 6's
                // only fact on this ending is [`container_fact`], which is its neighbour's first, so
                // the repeated sentence collapses whichever of rules 1 and 5 is speaking.
                //
                // **It took two fixes to get here and both were defects, not tuning.** Rules 1 and 6
                // spelled one duration two ways, so the subset clause kept two byte-identical
                // ten-line cards in a sixteen-row pane; and the duration itself was
                // [`ran_for`]'s to refuse on this ending, because containerd stamps `finishedAt` when
                // it recovers rather than when the run ended.
                assert!(
                    !about
                        .iter()
                        .any(|f| f.title.contains(&format!("exit {code}"))),
                    "{object} {role:?} exit {code}: rule 6 says what its neighbour already said and adds no fact \
                 to it, so its card goes: {:?}",
                    about
                        .iter()
                        .map(|f| (&f.title, &f.evidence))
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    about
                        .iter()
                        .filter(|f| f.action == no_exit_code_action())
                        .count(),
                    1,
                    "{object} {role:?} exit {code}: and the sentence is on the screen once: {:?}",
                    titles(&all)
                );
                // **And the duration is gone from the card that is left**, which is what makes the
                // fold possible and is a requirement of its own: `finishedAt` is written at recovery
                // on this ending, so a node down overnight would have printed *ran for 8 hours* about
                // a run of 50 seconds — one line under a title saying nobody read how it ended.
                assert!(
                    !counted.evidence.contains("ran for"),
                    "{object} {role:?} exit {code}: `finishedAt` measures the node's outage here and not the \
                 run: {}",
                    counted.evidence
                );
            }
        }
    }
    // **The base capture carries the container id that claim rests on**, read out of the JSON
    // rather than trusted: `Terminated` does not carry the field, so nothing else in this file
    // would notice it being absent (NOTES § D40).
    let raw = fixture("restarts10");
    assert!(
        captured_str(
            captured_status(&raw, "containerStatuses", "flaky"),
            &["lastState", "terminated", "containerID"]
        )
        .starts_with("containerd://"),
        "the plant is built on a record with a real container id, which is what makes \
         `logs --previous` an answerable pointer here and not one on a lost status"
    );

    // **The budget both the action and the card are drawn inside** (`screens/alerts.md` § The
    // height): five wrapped lines for an action, and the card's own ten are
    // [`the_cards_this_box_ships_fit_the_height_they_are_drawn_at`]'s.
    let lines = wrapped_at(no_exit_code_action(), ACTION_COLUMNS);
    println!("{} lines at {ACTION_COLUMNS} columns", lines.len());
    assert!(
        lines.len() <= 5,
        "an action that wraps past five lines is a `rules.rs` finding — {} lines: {:?}",
        lines.len(),
        no_exit_code_action()
    );
}

/// **The third bare literal, and k8rs is silent about it on purpose** — `kubelet_pods.go:2705-2723`
/// at v1.36.1, the init container whose status the runtime lost.
///
/// **The premise the box was opened on does not survive the source, and this test is what replaces
/// it.** Three things had to be read rather than assumed:
///
/// - **It lands in `state.terminated`, not in `lastState`.** The loop assigns
///   `statuses[container.Name].State`, so no rule that reads an ending off `lastState` sees it at
///   all — rules 1, 5 and 6 are not standing down on this object, they never reach it. The one
///   reader of the current terminated state is [`doing_its_job`].
/// - **The `reason` discriminates nothing.** The literal is `Completed` / `0`, which is byte for
///   byte what a genuine finish writes. Keying an ending on the reason — the shape the box
///   proposed — separates no object the API can produce. **The only field that differs is the
///   stamp**, which is [`last_log_line`]'s key and is why that guard is where the class fix went.
/// - **The kubelet is deducing, not guessing.** The write is gated on
///   `HasAnyRegularContainerCreated`, and the regular containers are started only after every
///   non-restartable init container has succeeded — the source says so in its own comment. So
///   `Finished` is the **true** reading, [`doing_its_job`] answering *yes* is correct, and the
///   silence is the right output.
///
/// **What a card would cost, since silence is never free.** The comment beside that literal names
/// static pods first, so the class is every control-plane pod after a kubelet restart: a card here
/// is a permanent WARN on kube-apiserver, etcd and the two controllers on every cluster k8rs is
/// ever pointed at, about an init container that did its job. That is D71's false-positive class
/// at its widest.
///
/// **The claim is pinned rather than argued, on the object and not on a paraphrase of it**: the
/// three fields the kubelet writes, planted on a decoded copy of a capture that already has the
/// rest of the shape (NOTES § D40, § D53). **The stamps are left off because the missing stamps
/// *are* the shape** — a plant that kept the capture's would be proving the silence against an
/// object no kubelet produces.
#[test]
fn a_lost_init_container_status_reads_as_finished_and_that_is_the_true_reading() {
    // `healthy-retry.json` is the shape one field away: a plain init container that failed once
    // and then finished, with the app running behind it. What the plant moves is the *current*
    // terminated state, from the kubelet's watched `Completed` to its synthesized one.
    let lost = capture_but("healthy-retry", |p| {
        container_status(p, "wait-for-db").state = Some(ApiContainerState {
            terminated: Some(ContainerStateTerminated {
                reason: Some("Completed".to_string()),
                message: Some(
                    "Unable to get init container status from container runtime and pod has been \
                     initialized, treat it as exited normally"
                        .to_string(),
                ),
                exit_code: 0,
                ..ContainerStateTerminated::default()
            }),
            ..ApiContainerState::default()
        });
    });
    let init = container(&lost, "wait-for-db");
    println!("{init:?}");
    let ContainerState::Terminated(run) = &init.state else {
        panic!("the plant writes the current terminated state this test is about: {init:?}")
    };
    // **The object first, in the fields that decide the reading.** The reason is the one a real
    // finish carries and the stamps are the ones it does not — assert both, or the shape under
    // test is not the kubelet's (NOTES § D29).
    assert_eq!(
        (
            run.reason.as_deref(),
            run.exit_code,
            run.started_at.is_none(),
            run.finished_at.is_none()
        ),
        (Some("Completed"), 0, true, true),
        "the literal is `Completed` / `0` with no stamps at all — the reason is exactly what a \
         watched finish writes, so nothing but the missing stamp tells the two apart: {run:?}"
    );
    assert!(
        init.role == ContainerRole::Init
            && init
                .last_terminated
                .as_ref()
                .is_some_and(|r| r.exit_code == 1),
        "an init container with a real failure still on its record beside the synthesized \
         finish — the neighbour whose suppression is half of what this test asserts: {init:?}"
    );
    assert!(
        ending(run) == Ending::Finished && doing_its_job(init),
        "the kubelet only starts the regular containers once every non-restartable init container \
         has succeeded, so this is a deduction and `Finished` is the true reading of it: {init:?}"
    );
    let all = analyze(&pods_at(vec![lost], now()));
    show(&all);
    assert!(
        cards_about(&all, "wait-for-db").is_empty(),
        "nothing is wrong here: a card would be a permanent WARN on every static pod in every \
         cluster after a kubelet restart, which is the class the source's own comment names \
         first: {:?}",
        titles(&all)
    );
    // **And the kubelet's sentence does not reach the screen by the other door either.** It rides
    // the record as a `message`, which is exactly the shape [`last_log_line`] refuses.
    assert!(
        all.iter().all(|f| !f.action.starts_with(QUOTE_FRAME)),
        "the kubelet wrote that sentence, not the container: {:?}",
        all.iter().map(|f| &f.action).collect::<Vec<_>>()
    );

    // **The canary, or the silence above is the rules never reaching this container** (NOTES § D26).
    // The same plant with a failing current run draws rule 6's card off the record beside it.
    let failing = capture_but("healthy-retry", |p| {
        let run = container_status(p, "wait-for-db")
            .state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the capture's init container is currently terminated");
        run.exit_code = 1;
        run.reason = Some("Error".to_string());
    });
    let reached = analyze(&pods_at(vec![failing], now()));
    show(&reached);
    assert!(
        !cards_about(&reached, "wait-for-db").is_empty(),
        "an init container that has *not* finished draws on this very pod, so the silence above \
         is [`doing_its_job`] answering the question rather than the rules failing to look: {:?}",
        titles(&reached)
    );
}

/// **What no card may claim about a run nothing is recorded as having ended** — the negatives the
/// two whole-card-set tests above share, applied to *every* card the pod draws about the
/// container and not to the one the test went looking for (NOTES § D93).
///
/// **The doors onto a kill are asked of the cards drawn from the ending only**, and that is a
/// scope rather than a hole: on `restarts10.json` rule 7 also fires, and its card names the
/// readiness probe about the container's readiness *now* — a true card about a different
/// question. The three cards that read the ending all carry [`exit_fact`] on the title or the
/// evidence, which is what selects them.
fn no_card_reads_this_run_as_a_kill(about: &[&Finding], shape: &str) {
    for f in about {
        let said = format!("{} {} {}", f.title, f.evidence, f.action).to_lowercase();
        for phrase in KILLED_IT {
            assert!(
                !said.contains(phrase),
                "{shape}: {phrase:?} — the kubelet wrote 137 where a status went missing, and \
                 nothing is recorded as having ended this run: {} / {} / {}",
                f.title,
                f.evidence,
                f.action
            );
        }
        // **The pointer that is not merely unhelpful but impossible.** `logs --previous` is gated
        // on `lastState.terminated.containerID`, which this record does not carry, so the API
        // answers `previous terminated container "app" in pod "lost-notready" not found`
        // (measured — NOTES § D93).
        for pointer in SENT_TO_THE_LOGS {
            assert!(
                !f.action.contains(pointer),
                "{shape}: the API will not serve that log — there is no containerID on this \
                 record to serve it from: {}",
                f.action
            );
        }
        assert!(
            !said.contains("--previous"),
            "{shape}: nor the flag itself: {}",
            f.action
        );
        if !said.contains("exit 137") {
            continue;
        }
        // **Neither record carries a stamp, so no card drawn off one may date itself or say how
        // long the run lasted.** Both are asserted on rule 6's card already; rules 1 and 5 build
        // their own age out of the same two fields, and rule 1 draws the same [`ran_for`] clause
        // rule 6 does, so a rule that starts inventing one is caught here saying so
        // (NOTES § D93).
        //
        // **Rule 5's *serving* card is the one exception, and it is dated off a different field**
        // (NOTES § D100): `state.running.startedAt`, the start of the run the container is in —
        // live on the same object, moving on every restart, and nothing to do with the record
        // this function is about. The callers that draw one pin the field it came from, which is
        // where that claim can be checked against the object.
        if !f.title.contains("it is serving now") {
            assert_eq!(
                f.timestamp, None,
                "{shape}: the run carries no `finishedAt`, so the card carries no age: {f:?}"
            );
        }
        assert!(
            !said.contains("lasted") && !said.contains("ran for"),
            "{shape}: and no duration either — the run has no stamps to measure, whichever of \
             the two old spellings a card might reach for: {}",
            f.evidence
        );
        for door in PROBE_WORDS.iter().chain(["memory limit"].iter()) {
            assert!(
                !said.contains(door),
                "{shape}: {door:?} is a door onto a kill, under an evidence line saying no kill \
                 was seen: {} / {} / {}",
                f.title,
                f.evidence,
                f.action
            );
        }
    }
}

/// Every card the pod drew **about one container**, which is what the two tests above assert
/// over — a pod's other containers have cards of their own and are not this box's subject.
///
/// **Matched on the whole name and not on a substring of it.** Every rule puts [`container_fact`]
/// first in its evidence, and that fact ends the name either at the end of the line or at the
/// space before its role gloss — so `app` and `app-proxy` are told apart. A `contains` walked
/// straight past that, and the two-container plant next door is the first shape in this file that
/// would have merged two containers' cards into one silent over-count (NOTES § D95).
fn cards_about<'a>(all: &'a [Finding], name: &str) -> Vec<&'a Finding> {
    let ends_there = format!("container {name}");
    let gloss = format!("container {name} (");
    all.iter()
        .filter(|f| {
            let fact = f.evidence.split(FACTS).next().unwrap_or_default();
            fact.ends_with(&ends_there) || fact.contains(&gloss)
        })
        .collect()
}

/// **The ways a card could tell the reader that this container is the innocent one** — the guard
/// for the blocker the operator review filed, in [`KILLED_IT`]'s shape (NOTES § D95).
///
/// **The claim is false of the object, not merely unhelpful.** One gang restart writes the same
/// synthesized record into *every* container's `lastState`, the container whose own exit
/// triggered it included — its `exit 3` is in `state.terminated`, which no rule reads — so a card
/// that exonerates the container it is drawn about is wrong on precisely the container that
/// failed, and points the one reader who was looking at the right thing somewhere else.
///
/// **Hedges count, and they are what got past the first guard.** `tester` appended eight words —
/// *…and that may be this container, but rarely* — and the suite stayed green: the assertions
/// were three `contains` fragments, all of which that sentence keeps. A clause can be taken back
/// by what follows it.
///
/// **Its control is synthetic, and that is the honest form here.** [`KILLED_IT`] is asserted
/// present on a real card because an ordinary bad exit still says those words; these words the
/// rule set may **never** say, so a control drawn from the product would mean the defect had
/// shipped. What is proved instead is that the detector fires — every phrase, and the exact
/// mutation — which is the same anti-rot requirement one level down (CLAUDE.md § Code phase
/// rules, *a derived list asserts it found something*).
const EXONERATES: [&str; 10] = [
    "but rarely",
    "though rarely",
    "unlikely",
    "not this container",
    "not this one",
    "another container",
    "a different container",
    "its sibling",
    "look elsewhere",
    "is fine",
];

/// The first [`EXONERATES`] phrase in `text`, lowercased for [`PROBE_WORDS`]' reason
/// (NOTES § D31) — `None` when the sentence keeps the reader in the frame.
fn exonerating(text: &str) -> Option<&'static str> {
    let said = text.to_lowercase();
    EXONERATES.into_iter().find(|p| said.contains(p))
}

/// [`EXONERATES`] over **the whole card** and not the action alone — the requirement is that
/// nothing on a gang-restart card tells the reader to stop looking at this container, and a title
/// can say that as easily as an action can (NOTES § D95).
fn no_card_lets_this_container_off(cards: &[&Finding], shape: &str) {
    for f in cards {
        let whole = format!("{} {} {}", f.title, f.evidence, f.action);
        assert_eq!(
            exonerating(&whole),
            None,
            "{shape}: the trigger carries this same record, so the container the reader is \
             looking at may be the one that failed: {} / {} / {}",
            f.title,
            f.evidence,
            f.action
        );
    }
}

/// **The three claims of a kill the rule set can put on a card**, matched against a lowercased
/// haystack for [`PROBE_WORDS`]' reason (NOTES § D31). Each is true of an ordinary bad exit and
/// false of the two reasons the kubelet writes itself, so each is asserted absent on those and
/// **present on the control** — a phrase nothing says any more is a negative guarding nothing.
const KILLED_IT: [&str; 3] = [
    "keeps crashing",
    "something keeps killing",
    "on record failed",
];

/// **The card set this box owes: no card k8rs draws about an `Init` container names a probe,
/// anywhere on it.** `validateInitContainers` rejects `livenessProbe`, `readinessProbe` and
/// `startupProbe` on an init container that is not restartable, so a title, an evidence line or
/// an action naming one is advice the reader cannot follow — and rules 1, 5 and 6 all draw about
/// the same container, onto the same screen, so the fact has to hold across the three of them at
/// once (NOTES § D85, § D90).
///
/// **Driven over every `(exit code, reason)` shape those rules can reach, in both states they
/// reach it in** (NOTES § D29): the wait rule 1 fires on and the re-run rules 5 and 6 fire on,
/// each over the whole translation table plus a code the table does not cover and the reason the
/// kubelet writes for a status it never read.
///
/// **It asserts the cards exist before it asserts what they do not say.** A filter that matched
/// nothing would print the same green line as a rule set that had been fixed
/// (NOTES § D26), and the words themselves are proved reachable off the committed captures at
/// the end — a rule set that stopped saying *liveness* at all would leave every negative here
/// guarding nothing.
#[test]
fn no_card_about_an_init_container_ever_names_a_probe() {
    // `0` and `143` are the two rule 6 exempts and rules 1 and 5 read as clean endings; `1`,
    // `126`, `127` and `137` are the failures; `42` is a code the table does not translate; and
    // `137` is fed all three of its reasons, which is the whole of this box.
    //
    // **The fourth column is how many cards that shape must draw about this container**, and it
    // is the difference between a guard and a counter: with the count only summed and printed,
    // deleting both [`STATUS_LOST`] rows — part (iii) of this box, its whole subject — left the
    // loop green over eight shapes instead of ten (CLAUDE.md § Code phase rules, *a derived list
    // asserts it found something*). One is a clean ending, which only rules 1 and 5 draw on;
    // two is a failure, where rule 6 or rule 2 joins them.
    let runs: [(i32, Option<&str>, Option<&str>, usize); 12] = [
        (0, None, None, 1),
        (143, None, None, 1),
        (1, None, None, 2),
        (126, None, None, 2),
        (127, None, None, 2),
        (42, None, None, 2),
        (137, None, None, 2),
        (137, Some("OOMKilled"), None, 2),
        // **One card, and it was two until 2026-08-15.** Rule 6 draws on this reason and answers
        // it with the sentence rules 1 and 5 answer it with, so the second copy is folded away —
        // by `analyze` and not by any exemption of rule 6's, which is what separates this row
        // from the `RestartAllContainers` pair below (NOTES § D102).
        (137, Some(STATUS_LOST), None, 1),
        // The same reason with the kubelet's own sentence beside it, which is the pair a cluster
        // actually writes and the one that decides whether rule 6 reads the reason or the
        // message (NOTES § D90).
        (
            137,
            Some(STATUS_LOST),
            Some("The container could not be located when the pod was terminated"),
            1,
        ),
        // **The fourth `137` reason, both ways round.** It is beta-on-by-default at the pinned
        // version, so it is a shape the real pipeline hands these rules today — and the kubelet
        // writes a message beside it, which sends rule 6 down a different arm from the bare one
        // (NOTES § D29, § D93).
        // **One card, not two: rule 6 exempts this reason.** The pod's own restart rule removed
        // the container, so nothing failed — the exemption sits beside `OOMKilled`'s and the
        // count here is what says it holds on this role too (NOTES § D93).
        (137, Some(RESTART_ALL), None, 1),
        (
            137,
            Some(RESTART_ALL),
            Some("The container is removed because RestartAllContainers in place"),
            1,
        ),
    ];
    // Written down rather than summed, so a row that is deleted along with the array's length
    // takes this line red with it.
    const INIT_CARDS: usize = 26;
    let mut cards = 0usize;
    for looping in [false, true] {
        for (code, reason, message, expected) in runs {
            // **The two the fold takes, derived rather than typed into a fifth column**
            // (NOTES § D113). On a looping container rule 1 speaks, and since 2026-08-16 both it
            // and rule 6 ask [`failed_run_action`] — so on any `Failed` ending they say the one
            // sentence, rule 6 adds no fact (these rows carry no message), and
            // [`one_card_per_action`] collapses the pair. Off the loop it is rule 5 speaking,
            // whose `Failed` arm is role-keyed and different, so both cards stand. Reading the
            // exception off the product function is what keeps this line honest if that function
            // stops answering for a code.
            // **The pair the fold takes, derived off the product functions rather than typed
            // into a fifth column** (NOTES § D113). Rules 1 and 6 answer [`Ending::Failed`] with
            // [`failed_run_action`]'s one sentence, so their two cards become one — but only where
            // rule 1 speaks (`looping`), only where rule 6 is not exempt (`OOMKilled` is rule 2's,
            // and the clean endings are nobody's), and only where rule 6 adds no fact of its own
            // (a termination message is a fact rule 1 has not got).
            // **No longer gated on `looping`** (NOTES § D113): rule 5 shares the ending too, and
            // it now carries [`ran_for`], so rule 6's facts are a subset of *either* neighbour's.
            let both_say_one_thing = message.is_none()
                && reason != Some("OOMKilled")
                && matches!(
                    ending(&Terminated {
                        reason: reason.map(str::to_string),
                        exit_code: code,
                        ..exited_run(code)
                    }),
                    Ending::Failed
                );
            let expected = if both_say_one_thing {
                expected - 1
            } else {
                expected
            };
            let pod = init_previous_run(code, reason, message, looping);
            let all = analyze(&pods_at(vec![pod], now()));
            let about_init: Vec<&Finding> = all
                .iter()
                .filter(|f| f.evidence.contains("init container wait-for-db"))
                .collect();
            println!(
                "exit {code} {reason:?} message={} looping={looping}: {} findings",
                message.is_some(),
                all.len()
            );
            for f in &about_init {
                println!("  {} | {} | {}", f.title, f.evidence, f.action);
            }
            assert_eq!(
                about_init.len(),
                expected,
                "exit {code} {reason:?} message={} looping={looping} draws {} cards about the \
                 init container and not {expected} — a shape that goes silent takes its own \
                 assertions with it and subtracts from a total nobody reads (NOTES § D26): {:?}",
                message.is_some(),
                about_init.len(),
                titles(&all)
            );
            cards += about_init.len();
            for f in about_init {
                for (part, text) in [
                    ("title", &f.title),
                    ("evidence", &f.evidence),
                    ("action", &f.action),
                ] {
                    // **Lowercased, and that is not tidiness.** *Probes are worth checking* —
                    // capitalised because it opens a sentence — is the forbidden advice on the
                    // forbidden role, and it passed this loop while it compared the raw string.
                    // A guard is proven only for the framing it was written for (NOTES § D31).
                    let said = text.to_lowercase();
                    for probe in PROBE_WORDS {
                        assert!(
                            !said.contains(probe),
                            "exit {code} {reason:?} looping={looping}: the {part} names \
                             {probe:?} about a container Kubernetes allows none — {text}"
                        );
                    }
                }
            }
        }
    }
    println!("{cards} cards about one init container, none of them naming a probe");
    assert_eq!(
        cards, INIT_CARDS,
        "the shapes above are the whole of what rules 1, 5 and 6 can draw about an init \
         container, and a row deleted out of the table is a meaning that stopped being guarded"
    );

    // **The canary: every word above is one this rule set still says.** "Found nothing" and
    // "there was nothing to find" print the same green line, so each is asserted present on a
    // card drawn off the committed captures — where the container *is* allowed a health check
    // (CLAUDE.md § Code phase rules).
    let everything = findings(&CAPTURED_PODS);
    for probe in PROBE_WORDS {
        assert!(
            everything.iter().any(|f| f.action.contains(probe)),
            "no card in the whole capture says {probe:?} any more, so the loop above is \
             guarding a word nothing produces: {:?}",
            everything
                .iter()
                .map(|f| f.action.as_str())
                .collect::<Vec<_>>()
        );
    }
}

/// **A `Terminated` that ran, and one the runtime never started** — the two sides of
/// [`failed_run_action`]'s fork, spelled once because three tests read them (NOTES § D113).
///
/// The epoch `startedAt` is what containerd writes when it never got the process going, and it is
/// a real value the API sends rather than a `None` any `Option` on the path could see.
fn exited_run(code: i32) -> Terminated {
    Terminated {
        reason: Some("Error".to_string()),
        exit_code: code,
        started_at: Some(time("2026-08-13T22:32:30Z")),
        finished_at: Some(time("2026-08-13T22:33:00Z")),
        message: None,
    }
}

fn never_started_run() -> Terminated {
    Terminated {
        started_at: Some(time("1970-01-01T00:00:00Z")),
        exit_code: 128,
        reason: Some("StartError".to_string()),
        ..exited_run(128)
    }
}

/// **[`failed_run_action`]'s fork, on the object rather than on the code** (NOTES § D113).
///
/// **The first version keyed on `126..=128` and the key was wrong.** Measured on kind v1.36.1
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 2), `126` and `127` are
/// what a **shell inside a container that ran** reports — real stamps, real `containerID`, the
/// whole diagnosis on one log line — while `128`/`StartError` is the epoch `startedAt` of a
/// container the runtime never started, whose log is empty. `tests/fixtures/notfound.json` is the
/// `127` row and was the counter-example all along.
///
/// **So the discriminator is [`run_length`]**, and it is asserted here on a run built each way
/// rather than through a card: what the cards do with it is the two tests below, and this is the
/// fork itself.
///
/// **`cargo mutants` could not have caught the range** it replaced — that tool mutates function
/// bodies and binary operators, not range patterns, so `127..=128`, `125..=128` and `126..=129`
/// all built and passed the whole suite. A `match` on an `Option` is mutable and is one more
/// reason the shape is better than the one it replaced.
#[test]
fn what_a_failed_run_needs_is_decided_by_whether_it_ran() {
    let run = |code: i32, started: Option<&str>| Terminated {
        started_at: started.map(time),
        ..exited_run(code)
    };
    // **The epoch is a value the API really sends** — containerd sets the other four fields and
    // leaves `StartedAt` at `0` when it never got the process started, and `time.Unix(0, 0)`
    // marshals as a real RFC3339 stamp (NOTES § D112).
    let never_ran = never_started_run();
    assert_eq!(
        run_length(&never_ran),
        None,
        "the epoch is the shape this fork is keyed on"
    );
    for role in EVERY_ROLE {
        let (action, log) = failed_run_action(&never_ran, role);
        println!("{role:?} never ran: log={log} | {action}");
        assert!(
            action.contains("command and arguments") && !action.contains("log"),
            "{role:?}: a container the runtime never started has no log to read, and what the \
             reader is sent to is the command that named a path the image has not got: {action}"
        );
        assert!(
            !log,
            "{role:?}: and the card may not carry `logs --previous`, which serves nothing here"
        );
    }

    // **The three codes the old key claimed for *never ran*, on runs that did run.** Each is a
    // shell reporting for a container with a real `startedAt`; each has a log holding the answer.
    for code in [126, 127, 128, 1, 42] {
        let ran = run(code, Some("2026-08-13T22:32:30Z"));
        assert!(run_length(&ran).is_some(), "the plant has to have run");
        let (action, log) = failed_run_action(&ran, ContainerRole::Regular);
        println!("exit {code} ran: log={log} | {action}");
        assert!(
            log && action.contains("log"),
            "exit {code} ran, so the answer is on that run's log and the card owes the command \
             that serves it: {action}"
        );
        assert!(
            !action.contains("not in the image"),
            "exit {code}: *what they name is not in the image* is true only of a container that \
             never started — on `exit 126` it stands over an evidence line reading *the command \
             was found but could not be run*, which is one card contradicting itself: {action}"
        );
    }

    // **A backwards clock step is not a container that never ran** (NOTES § D113). `startedAt`
    // and `finishedAt` are two wall-clock stamps written at two moments, and `chrony`'s
    // `makestep` after a bad RTC — or a VM resumed from a snapshot — puts the second before the
    // first on a container that ran normally, has a `containerID`, and whose log holds the panic.
    // [`run_length`] refuses that record, correctly, because no duration can be computed from it;
    // keying the never-ran arm on it sent that reader to *what they name is not in the image*
    // under `describe`, **with the duration missing from the same card** because [`ran_for`]
    // shares the predicate — so nothing on the screen let them see the inconsistency. The arm
    // keys on the start alone ([`ever_started`]) and this row is what says so.
    let stepped = Terminated {
        finished_at: Some(time("2026-08-13T22:32:00Z")),
        ..exited_run(1)
    };
    assert!(
        run_length(&stepped).is_none() && ever_started(&stepped).is_some(),
        "the plant has to be the shape the two predicates disagree about: {stepped:?}"
    );
    let (action, log) = failed_run_action(&stepped, ContainerRole::Regular);
    println!("clock step: log={log} | {action}");
    assert!(
        log && !action.contains("not in the image"),
        "a clock that went backwards between two stamps is not a container that never started, \
         and the log it wrote is still there: {action}"
    );

    // **`137` answers ahead of the fork**, whichever way the stamps read: a kill from outside is
    // neither question, and its own log holds no error to find.
    for started in [
        Some("2026-08-13T22:32:30Z"),
        Some("1970-01-01T00:00:00Z"),
        None,
    ] {
        for role in EVERY_ROLE {
            let (action, log) = failed_run_action(&run(137, started), role);
            assert_eq!(
                (action, log),
                (killed_action(role), false),
                "{role:?} {started:?}: 137 is [`killed_action`]'s whatever the stamps say"
            );
        }
    }

    // **The log arm is shared by every code the ending covers, so it may not claim any of them
    // spoke** (NOTES § D113). A kernel kill funnels here — [`out_of_memory`] draws beside it with
    // the fix and this card carries the question rule 2 does not answer — and a container the
    // kernel SIGKILLed **said nothing**: it was cut off mid-sentence, which is the premise
    // [`killed_action`] is built on one arm up. *That is where the program said what went wrong*
    // was true of `exit 1` and of a shell's `127` and false of `oom.json`, a committed capture,
    // with the falsehood one line under an evidence line reading *killed by the kernel*.
    //
    // **Asserted end to end and not on the helper**, because the pairing of the sentence with
    // that evidence line is the defect; the string on its own reads fine.
    // Picked by the evidence line this block is about rather than by the rule that drew it: on
    // `oom.json` the log arm is reached by rule 1 in backoff and by rule 5 between runs, and the
    // capture is certified for both faces (NOTES § D114, `scripts/cluster.sh` § `[oom]`).
    let all_oom = findings(&["oom"]);
    let kernel = all_oom
        .iter()
        .find(|f| {
            f.evidence
                .contains("killed by the kernel for using more memory")
        })
        .unwrap_or_else(|| {
            panic!(
                "the shape under test is a labelled kill on the card that reaches the log arm, \
                 or this block is about something else: {:?}",
                titles(&all_oom)
            )
        })
        .clone();
    println!("{} | {}", kernel.evidence, kernel.action);
    assert_eq!(
        kernel.action,
        failed_run_action(&exited_run(1), ContainerRole::Regular).0,
        "and it reaches the shared log arm, which is what makes the sentence's truth its problem"
    );
    for spoke in ["said", "says", "told", "reported"] {
        assert!(
            !kernel.action.contains(spoke),
            "the action may not put words in a container the kernel killed mid-sentence — what \
             is true of every code this arm covers is what the log *holds*, never who spoke: {}",
            kernel.action
        );
    }

    // And the two arms really are different advice, or the fork above is one sentence with extra
    // steps (NOTES § D26).
    assert_ne!(
        failed_run_action(&never_ran, ContainerRole::Regular).0,
        failed_run_action(
            &run(127, Some("2026-08-13T22:32:30Z")),
            ContainerRole::Regular
        )
        .0,
        "the fork has to reach different sentences"
    );
}

/// **The capture that proves the fork, and it proved the first version wrong** (NOTES § D113).
///
/// `broken-notfound` is `sh -c 'exec /usr/local/bin/server --serve'` — `exit 127`, `reason:
/// Error`, a **real** `startedAt` and a `containerID`. That is a shell inside a container that
/// **ran**, and its log holds the whole diagnosis on one line. The first version of
/// [`failed_run_action`] keyed on `126..=128` and called this *the container never started*, which
/// put *what they name is not in the image* on the card and sent the reader to `describe` while
/// the log sat one uncalled function away
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 2).
///
/// **And the repetition card draws beside it** — rule 1's while the container is in backoff,
/// rule 5's when the capture lands between two runs. Before the shared answer its CRITICAL card
/// said *check the memory limit and the liveness probe* over an evidence line reading *the
/// command was not found*; with the code-keyed version it said *not in the image* over *the
/// command was found but could not be run*. Both are NOTES § D85's class, the second one inside
/// a single card.
///
/// **Which of the two draws is not asserted, because the capture is certified for both faces**
/// (`scripts/cluster.sh` § `[notfound]`, which asks only for `exit 127` and an unready
/// container). This test selects the survivor by *the container it is about* rather than by
/// title; keying on `CrashLoopBackOff` asserted which half of the backoff loop `just fixtures`
/// happened to catch, and the 2026-08-16 trip caught the other one (NOTES § D114).
///
/// **The never-ran side has no committed capture** and is asserted on the helper in
/// [`what_a_failed_run_needs_is_decided_by_whether_it_ran`], off the epoch `startedAt` containerd
/// writes (NOTES § D40).
#[test]
fn a_command_that_is_not_in_the_image_says_so_instead_of_sending_the_reader_to_the_logs() {
    let capture = pod("notfound");
    let c = capture
        .containers
        .first()
        .expect("the capture reports on its container");
    let run = c
        .last_terminated
        .as_ref()
        .expect("the capture records how the run before this one ended");
    println!("{c:?}");
    assert_eq!(run.exit_code, 127);
    assert!(
        run_length(run).is_some(),
        "the capture has to be a container that *ran* — that is the whole of what this test \
         proves, and a plant with no stamps would take the other arm and prove the opposite: \
         {run:?}"
    );

    let all = findings(&["notfound"]);
    show(&all);
    // **One card** (NOTES § D113). The repetition rule and rule 6 answer this ending with one
    // sentence, rule 6 adds no fact on this capture (no termination message), so the fold
    // collapses it and the survivor is the severe one. Selected by the container it names, not by
    // its title: the title is rule 1's or rule 5's depending on which half of the loop the
    // capture caught, and both are this fixture (NOTES § D114).
    let about = cards_about(&all, &c.name);
    assert_eq!(
        about.len(),
        1,
        "two cards saying one sentence about one container is what the fold exists to stop: {:?}",
        titles(&all)
    );
    let card = about[0];
    assert!(
        card.title.contains("CrashLoopBackOff") || card.title.contains("restarted"),
        "the survivor is the card about the repetition, whichever rule drew it: {}",
        card.title
    );
    assert_eq!(
        card.action,
        failed_run_action(run, c.role).0,
        "the shared answer, and on a container that ran it is the run's own log"
    );
    assert!(
        card.action.contains("log") && !card.action.contains("not in the image"),
        "a shell that could not exec what it was told to run wrote one line saying so — and \
         *not in the image* is false of `/etc/hostname`, which is in the image and not \
         executable: {}",
        card.action
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl logs broken-notfound -c app -n default --previous"),
        "and the command serves the log the action names (invariant 4)"
    );
    assert!(
        card.evidence
            .contains("exit 127 (the command was not found)"),
        "invariant 14: 127 is translated, never printed and left — on the surviving card, since \
         the card whose *title* carried the translation is the one that folded: {}",
        card.evidence
    );

    // **The control, one plant away**: with a termination message on the record rule 6 carries a
    // fact rule 1 has not, the subset clause refuses the fold, and both cards stand. That is what
    // keeps the fold a property of the *facts* rather than of the sentence.
    let spoke = capture_but("notfound", |p| {
        let run = container_status(p, &c.name)
            .last_state
            .as_mut()
            .and_then(|t| t.terminated.as_mut())
            .expect("the capture records the run this plant is writing on");
        run.message = Some("sh: exec: line 0: /usr/local/bin/server: not found".to_string());
    });
    let both = analyze(&pods_at(vec![spoke], now()));
    show(&both);
    let quoted = only(&both, "broken-notfound", "on record failed");
    assert!(
        quoted.evidence.contains(QUOTE_FRAME) && quoted.action == card.action,
        "the quote is what rule 6 adds, and it adds it on the evidence line while saying the same \
         thing the severe card says: {} / {}",
        quoted.evidence,
        quoted.action
    );
    assert_eq!(
        cards_about(&both, &c.name).len(),
        2,
        "so both cards stand, and the fold is keyed on what a card carries rather than on how it \
         is worded: {:?}",
        titles(&both)
    );
}

/// **Rule 7's `started` suppressor, on the capture that separates it from the state gate.**
///
/// `Running && !started` is reachable only where a `startupProbe` is declared and has not
/// passed yet, and until it does the kubelet does not run the readiness probe at all — so
/// `ready: false` there means *not asked yet*, not *failing*. Every other capture reports
/// `started: true`, which left the suppressor and the `ContainerState::Running` gate
/// indistinguishable (NOTES § D71): deleting either one kept the suite green.
///
/// `broken-startup` declares a `startupProbe` that never passes. **The control is the same
/// capture with the flag turned on**, which is the pod once its startup probe finally passes
/// and the readiness probe starts being asked — and that pod *is* rule 7's.
#[test]
fn a_container_still_inside_its_startup_probe_is_not_one_failing_its_readiness_check() {
    let slow = pod("startup");
    let c = container(&slow, "slowboot");
    println!("{c:?}");
    assert!(
        matches!(c.state, ContainerState::Running { .. }) && !c.ready && !c.started,
        "the capture has to be running, unready and not yet started, or the suppressor is \
         not what the silence below is about: {c:?}"
    );
    assert!(
        !findings(&["startup"])
            .iter()
            .any(|f| f.title.contains("not receiving traffic")),
        "the kubelet has not asked this container whether it is ready yet, so *the \
         readiness check is failing* is a sentence about a probe that has not run: {:?}",
        titles(&findings(&["startup"]))
    );

    let past_startup = capture_but("startup", |p| {
        container_status(p, "slowboot").started = Some(true);
    });
    let all = analyze(&pods_at(vec![past_startup], now()));
    show(&all);
    only(&all, "broken-startup", "not receiving traffic");
}

/// **A runtime socket, mounted by an ordinary pod, under the spelling the fold exists for.**
///
/// `hostpath.json` reaches neither half: it mounts the node's root and a subdirectory, so the
/// exact match against [`RUNTIME_SOCKETS`] and the `/var/run` → `/run` fold were only ever
/// exercised on planted mounts (NOTES § D78). `broken-socket` mounts `/var/run/docker.sock`
/// **read-only**, which is the posture a reader is most likely to believe is safe.
#[test]
fn the_captured_runtime_socket_is_a_card_even_though_it_is_read_only() {
    let mounted = pod("socket");
    println!("{:?}", mounted.host_path_mounts);
    assert!(
        mounted
            .host_path_mounts
            .iter()
            .all(|m| m.read_only && m.path.starts_with("/var/run/")),
        "the capture has to be a read-only mount written the `/var/run` way, or neither the \
         fold nor the mode is being tested: {:?}",
        mounted.host_path_mounts
    );
    assert!(
        !mounted.mirror && mounted.owner.kind != ObjectKind::DaemonSet,
        "and it is an ordinary pod, so D70's narrowing is not what would have to be \
         overridden for this card to appear"
    );

    let all = findings(&["socket"]);
    show(&all);
    let card = only(&all, "broken-socket", "drive the container runtime");
    assert_eq!(card.severity, Severity::Critical);
    assert!(
        card.evidence.contains("/var/run/docker.sock on the node")
            && card.evidence.contains("read-only"),
        "the card names the path the manifest wrote — the fold belongs to the compare, not \
         to the reader who has to find this mount in their own YAML — and says that \
         read-only did not save it: {}",
        card.evidence
    );
}

/// Rule 8's positives on one captured pod — the node's root and the node's runtime
/// socket — and the read-only mount of a path that is neither, which is the Analysis
/// posture row and not a card.
#[test]
fn the_two_escalated_host_mounts_both_fire_and_the_ordinary_one_does_not() {
    let all = findings(&["hostpath"]);
    show(&all);
    assert_eq!(all.len(), 2, "one per escalated mount: {:?}", titles(&all));

    // `shipper` mounts `/` and mounts it **read-only**, and it fires anyway: the path
    // alone is the escalator, because read-only access to the node's whole filesystem
    // is still every secret on the machine.
    let root = only(&all, "broken-hostpath", "whole filesystem of the machine");
    assert_eq!(root.severity, Severity::Critical);
    assert!(
        root.evidence.contains("container shipper") && root.evidence.contains("read-only"),
        "{}",
        root.evidence
    );

    // `nosy` mounts the same volume with `subPath: run/containerd`, so what it actually
    // gets is `/run/containerd` — not the node's root, but the directory the node's
    // runtime socket sits in, which is the socket (NOTES § D78). **This is the one
    // captured shape the socket escalator fires on**, and the path it names is the one
    // the container has.
    let socket = only(&all, "broken-hostpath", "drive the container runtime");
    assert_eq!(socket.severity, Severity::Critical);
    assert!(
        socket.evidence.contains("/run/containerd on the node"),
        "the subPath narrows what is mounted and the card has to say what the container \
         really got (D46): {}",
        socket.evidence
    );
    assert!(
        !socket.evidence.contains("/ on the node"),
        "a rule reading `path` alone would call this a mount of the node's root: {}",
        socket.evidence
    );
    assert!(socket.evidence.contains("writable"), "{}", socket.evidence);
    assert!(
        !socket.action.contains("mount it read-only"),
        "and the reader is not told read-only fixes it — that is the writable branch's \
         advice, and on a runtime socket it gives away the node: {}",
        socket.action
    );

    for f in &all {
        assert_eq!(
            f.timestamp, None,
            "a hostPath mount is a standing property, not an event, and a date beside it \
             sends the reader looking for a change that never happened"
        );
    }

    nothing(
        &findings(&["healthy-hostpath"]),
        "a read-only mount of /var/log is how a log shipper is supposed to work, and \
         D2 sends it to the Analysis posture rows",
    );
}

/// **Rule 8's real negative, and the reason the box could not close without this
/// capture.** Writable host mounts are the normal state of every CNI agent, kube-proxy
/// and control-plane component, so the rule as specified fires CRITICAL on a healthy
/// kind cluster.
#[test]
fn kube_systems_node_agents_and_static_pods_are_not_host_mount_findings() {
    let pods: Vec<PodSnapshot> = items::<Pod>("kube-system-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();

    // **The exemption is asserted to be exercised, not assumed.** "Nothing fired"
    // and "nothing could have fired" print the same green line, and this capture is
    // the only place either shape exists — so both are counted before the emptiness
    // below means anything.
    let writable = |p: &PodSnapshot| {
        p.host_path_mounts
            .iter()
            .any(|m| !m.read_only && mounted_path(m) != "/")
    };
    let daemonset_pods = pods
        .iter()
        .filter(|p| p.owner.kind == ObjectKind::DaemonSet && writable(p))
        .count();
    let mirror_pods = pods.iter().filter(|p| p.mirror && writable(p)).count();
    println!(
        "{} pods: {daemonset_pods} DaemonSet-owned and {mirror_pods} mirror pods write \
         to their node",
        pods.len()
    );
    assert!(
        daemonset_pods > 0,
        "kindnet and kube-proxy write to `/etc/cni/net.d`, `/run/xtables.lock` and \
         `/var/run/nri`; a capture without one is not this rule's negative"
    );
    assert!(
        mirror_pods > 0,
        "`etcd` writes to `/var/lib/etcd` and is owned by a **Node**, not a DaemonSet — \
         narrowing rule 8 to DaemonSets alone would still fire on every control plane, \
         which is why the exemption reads `mirror || DaemonSet`"
    );

    nothing(
        &analyze(&pods_at(pods, now())),
        "a fresh kind cluster's own kube-system is healthy, and every rule in this box \
         has to be silent on it",
    );
}

// --- RULE 8'S SOCKET ESCALATOR, ON PLANTED MOUNTS ---
//
// No committed capture mounts a runtime socket — kind's own components have no reason
// to — and none is edited to grow one (NOTES § D53). So every shape below is planted
// into a decoded copy of a real captured pod, one coherent group of fields at a time,
// and each is a shape `kubectl apply` would produce (NOTES § D40).

/// The captured hostPath pod with its one host volume repointed at `path` and both
/// mounts of it narrowed by `sub` — `shipper` read-only, `nosy` writable, so what the
/// two containers get is the same node path under two modes. That is the whole reason
/// this capture is the one to plant on: it is where the question *does the mode matter*
/// can be asked at all.
///
/// **`sub` is the mount's `subPath`, and `None` is not the same test as `Some`** — rule 8
/// reads the join and never `path` alone (NOTES § D46), so a socket assembled out of a
/// directory and a file has to be swept beside the ones written whole.
fn host_volume(path: &str, sub: Option<&str>) -> Vec<Finding> {
    let pod = capture_but("hostpath", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        let volume = spec
            .volumes
            .iter_mut()
            .flatten()
            .find(|v| v.host_path.is_some())
            .expect("hostpath.json is the capture that carries a host volume");
        volume
            .host_path
            .as_mut()
            .expect("the volume just found is the host one")
            .path = path.to_string();
        let name = volume.name.clone();
        for mount in spec
            .containers
            .iter_mut()
            .flat_map(|c| c.volume_mounts.iter_mut().flatten())
            .filter(|m| m.name == name)
        {
            mount.sub_path = sub.map(str::to_string);
        }
    });
    analyze(&pods_at(vec![pod], now()))
}

/// **Every socket in the list, under both of its names and both of its writings.**
/// `/var/run` is a symlink to `/run` on every systemd distribution, so the two spellings
/// are one file on the node and the card may not depend on which one an author typed
/// ([`is_runtime_socket`]); and a manifest may mount the directory and name the socket
/// in the `subPath`, which is a socket only once rule 8 joins them (NOTES § D46).
#[test]
fn a_runtime_socket_is_the_same_socket_under_either_of_its_two_spellings() {
    for socket in RUNTIME_SOCKETS {
        let var = format!("/var{socket}");
        let (dir, file) = socket
            .rsplit_once('/')
            .expect("every socket in the list is an absolute path to a file");

        for (path, sub, spelling) in [
            (socket.to_string(), None, socket.to_string()),
            (var.clone(), None, var.clone()),
            (dir.to_string(), Some(file), socket.to_string()),
            (format!("/var{dir}"), Some(file), var.clone()),
        ] {
            let all = host_volume(&path, sub);
            show(&all);

            assert_eq!(
                all.len(),
                2,
                "both containers mount {spelling} — written {path:?} + subPath \
                 {sub:?} — and nothing else is wrong with this pod: {:?}",
                titles(&all)
            );
            for f in &all {
                assert!(
                    f.title.contains("drive the container runtime"),
                    "and each of them has the machine, not a directory on it: {}",
                    f.title
                );
                assert_eq!(f.severity, Severity::Critical);
                assert!(
                    f.evidence.contains(&format!("{spelling} on the node")),
                    "the card names the path the manifest wrote — the folding of \
                     `/var/run` onto `/run` belongs to the compare, not to the reader \
                     who has to find this mount in their own YAML: {}",
                    f.evidence
                );
            }
            assert!(
                all.iter().any(|f| f.evidence.contains("container shipper")
                    && f.evidence.contains("read-only")),
                "and `shipper`'s bind is read-only, which is no defence: anything that \
                 can talk to this socket can start a privileged container on the node"
            );
        }
    }
}

/// **The shape that drew nothing at all**, and the reason rule 8 escalates on the path
/// rather than on the mode: a *read-only* bind of the runtime socket on a `kube-system`
/// DaemonSet, where the writable escalator is deliberately silent (NOTES § D70). With
/// CRI-O carried under one spelling this pod produced no card — a container able to
/// drive the runtime, invisible (NOTES § D77).
#[test]
fn a_read_only_runtime_socket_on_a_node_agent_is_still_the_whole_machine() {
    let mut pods = items::<Pod>("kube-system-pods");
    // Found by its controller, not by its name: a DaemonSet's pod carries a generated
    // suffix that is minted fresh on every `just fixtures`, and the property this test
    // needs is "owned by a DaemonSet", which is exactly what the reference says.
    let agent = pods
        .iter_mut()
        .find(|p| {
            p.metadata.owner_references.iter().flatten().any(|o| {
                o.controller == Some(true) && o.kind == "DaemonSet" && o.name == "kube-proxy"
            })
        })
        .expect("the capture carries kube-proxy, a DaemonSet D70's narrowing silences");
    let name = agent
        .metadata
        .name
        .clone()
        .expect("a captured pod has a name");
    let spec = agent.spec.as_mut().expect("a captured pod has a spec");
    spec.volumes.get_or_insert_with(Vec::new).push(Volume {
        name: "runtime".to_string(),
        host_path: Some(HostPathVolumeSource {
            path: "/run/crio/crio.sock".to_string(),
            type_: Some("Socket".to_string()),
        }),
        ..Volume::default()
    });
    spec.containers[0]
        .volume_mounts
        .get_or_insert_with(Vec::new)
        .push(VolumeMount {
            name: "runtime".to_string(),
            mount_path: "/run/crio/crio.sock".to_string(),
            read_only: Some(true),
            ..VolumeMount::default()
        });

    let all = analyze(&pods_at(
        pods.into_iter().map(PodSnapshot::from).collect(),
        now(),
    ));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "the rest of this capture is the healthy kube-system the test above proves \
         silent, so one card is the planted mount and nothing else: {:?}",
        titles(&all)
    );

    let card = only(&all, &name, "drive the container runtime");
    assert_eq!(card.severity, Severity::Critical);
    assert_eq!(card.owner.kind, ObjectKind::DaemonSet);
    assert!(
        card.evidence.contains("/run/crio/crio.sock on the node")
            && card.evidence.contains("read-only"),
        "it says which socket and that read-only did not save it: {}",
        card.evidence
    );
    // **And the action does not order this pod's mount deleted.** kube-proxy is
    // `kube-system`'s own DaemonSet, which is the shape a legitimate holder has too —
    // an nvidia container-toolkit installer, a Falco or Datadog node agent. The most
    // severe card on the screen must not talk a newcomer at 3am into breaking GPU
    // scheduling or their own security agent, so it carries both halves: remove it if
    // this is not a pod that manages or watches containers, and if it is, know what it
    // holds.
    assert!(
        card.action.contains("unless"),
        "the socket card is drawn on legitimate holders by design (NOTES § D70, § D78) \
         and an unconditional 'remove the mount' is wrong for every one of them: {}",
        card.action
    );
    assert!(
        card.action.contains("manage or watch"),
        "and the exception has to name what those holders actually do: Falco and Datadog \
         *watch* the containers on the node, they do not manage them, and Google's own \
         cAdvisor DaemonSet draws this card off a read-only `/var/run`. A reader who \
         cannot find their agent in the verb removes the mount, which is the failure the \
         half exists to prevent (NOTES § D79): {}",
        card.action
    );
    assert!(
        card.action.contains("every node"),
        "and the half that is true when the mount stays has to be said out loud — this \
         pod has root on every machine it runs on, which is the finding for a reader \
         who keeps the mount: {}",
        card.action
    );
}

/// The neighbours of a socket that are not one — names that only *look* like a prefix of
/// one, a file beside it, a `/var/run` path that is no socket at all, and `/var` itself.
/// Those last two are what the folding could break: `/var/run/netns` has to reach the
/// ordinary writable branch **under the name it was written with**, and `/var` folds to
/// the empty string, which every ancestor test written the obvious way calls a prefix of
/// everything (NOTES § D78).
///
/// **The last row is the join running the other way.** Everywhere else a `subPath`
/// widens what the volume said — `/` narrowed to `/run/containerd`. Here it narrows out
/// of trouble: the volume is `/run`, which is every socket on the node, and the container
/// is handed `/run/netns`, which is none of them. A check that asked the volume's own
/// path *as well as* the join would call this the machine (NOTES § D46).
#[test]
fn a_path_beside_a_runtime_socket_is_not_a_runtime_socket() {
    for (path, sub, gets) in [
        ("/run/crio.sock.bak", None, "/run/crio.sock.bak"),
        ("/run/crio/crio.sock.bak", None, "/run/crio/crio.sock.bak"),
        ("/run/criox", None, "/run/criox"),
        ("/var/run/netns", None, "/var/run/netns"),
        ("/var", None, "/var"),
        ("/run", Some("netns"), "/run/netns"),
    ] {
        let all = host_volume(path, sub);
        show(&all);
        assert!(
            !all.iter()
                .any(|f| f.title.contains("drive the container runtime")),
            "{gets} is not a control socket — written {path:?} + subPath {sub:?}: {:?}",
            titles(&all)
        );
        let writable = only(&all, "broken-hostpath", "change files on the machine");
        assert!(
            writable.evidence.contains(&format!("{gets} on the node")),
            "and the ordinary branch names what the container was handed: {}",
            writable.evidence
        );
    }
}

/// **A directory above the socket is the socket.** A container handed `/run/containerd`
/// opens `containerd.sock` inside it, and a container handed `/run` opens all five — the
/// same capability as mounting the socket file, and until D78 was widened it drew the
/// writable card, whose advice is *mount it read-only*: giving the node away in the
/// sentence meant to save it.
///
/// Both spellings again, because the fold and the ancestor match have to compose.
#[test]
fn a_directory_above_a_runtime_socket_hands_over_the_same_socket() {
    for dir in [
        "/run",
        "/var/run",
        "/run/containerd",
        "/var/run/containerd",
        "/run/crio",
        "/run/k3s",
        "/run/k3s/containerd",
    ] {
        let all = host_volume(dir, None);
        show(&all);

        assert_eq!(
            all.len(),
            2,
            "both containers are handed {dir}, which is a directory the node's runtime \
             socket lives under: {:?}",
            titles(&all)
        );
        for f in &all {
            assert!(
                f.title.contains("drive the container runtime"),
                "what is inside a mounted directory is mounted too: {}",
                f.title
            );
            assert_eq!(f.severity, Severity::Critical);
            assert!(
                f.evidence.contains(&format!("{dir} on the node")),
                "and the card names the directory the manifest wrote: {}",
                f.evidence
            );
        }
    }
}

/// [`is_runtime_socket`] alone, on the inputs the pipeline cannot hand it and the ones
/// where a prefix test degenerates.
///
/// **The obvious form of the ancestor match is wrong here and this is what catches it**:
/// strip a leading `/var`, then ask whether a socket starts with `format!("{path}/")`.
/// For `/var` the strip yields `""`, the prefix becomes `"/"`, and every socket in the
/// list matches — a CRITICAL *"can drive the container runtime"* card on a pod that
/// mounts `/var` and nothing else (NOTES § D78).
#[test]
fn only_a_real_ancestor_of_a_runtime_socket_counts_as_one() {
    for quiet in [
        "",
        "/",
        "/var",
        "/varlib/run/docker.sock",
        "/runx",
        "/run/cri",
        "/run/crio.sock.bak",
        "/run/criox",
        "/var/lib",
        "/etc/kubernetes",
    ] {
        assert!(
            !is_runtime_socket(quiet),
            "{quiet:?} is neither a runtime socket nor a directory one lives under, and \
             a container given it cannot reach the runtime"
        );
    }
    for loud in [
        "/run",
        "/var/run",
        "/run/containerd",
        "/var/run/containerd",
        "/run/crio",
        "/run/k3s",
        "/run/k3s/containerd",
    ] {
        assert!(
            is_runtime_socket(loud),
            "{loud:?} contains a runtime socket, so a container given it has the machine"
        );
    }
    for socket in RUNTIME_SOCKETS {
        assert!(is_runtime_socket(socket), "{socket} is the socket itself");
        assert!(
            is_runtime_socket(&format!("/var{socket}")),
            "/var{socket} is the same file under the name every systemd distribution \
             also has for it"
        );
    }
}

/// Rule 12, both sides of its margin.
#[test]
fn the_pod_that_will_not_shut_down_says_when_it_was_asked_and_who_is_holding_it() {
    let raw = fixture("stuck");
    let all = findings(&["stuck"]);
    show(&all);
    assert_eq!(all.len(), 1, "rule 12 alone: {:?}", titles(&all));

    let stuck = only(&all, "broken-stuck", "asked to shut down");
    assert_eq!(stuck.severity, Severity::Warn);
    assert!(
        stuck.evidence.contains("k8rs.test/never-removed"),
        "'a finalizer is holding it' and 'the kubelet has not confirmed it' are two \
         causes with unrelated fixes, and the list is the only thing that tells them \
         apart — `kubectl describe pod` does not print it at all: {}",
        stuck.evidence
    );
    // Which machine is the scheduler's business and moves on every capture, so the name
    // is read out of the pod the card is about rather than transcribed beside it.
    assert!(
        stuck.evidence.contains(&format!(
            "on node {}",
            captured_str(&raw, &["spec", "nodeName"])
        )),
        "{}",
        stuck.evidence
    );
    assert_eq!(
        stuck.kubectl_cmd.as_deref(),
        Some("kubectl get pod broken-stuck -n default -o yaml"),
        "and the command is the one that shows a finalizer, which `describe` does not"
    );

    // **The age is the moment the user asked, not the deadline.** The API server wrote
    // `deletionTimestamp` as request time *plus* the grace period, so the deadline is
    // one grace period late, forever (D46).
    let deadline = captured_time(&raw, &["metadata", "deletionTimestamp"]);
    let grace = at(&raw, &["metadata", "deletionGracePeriodSeconds"])
        .as_i64()
        .expect("the capture carries the grace this delete was granted");
    assert_eq!(
        stuck.timestamp,
        Some(Time(
            deadline
                .0
                .checked_sub(SignedDuration::from_secs(grace))
                .expect("five seconds off a captured moment is representable")
        ))
    );
    assert_ne!(
        stuck.timestamp,
        Some(deadline.clone()),
        "the deadline itself is the field the rule may not report, and this capture's \
         {grace}-second grace is what makes the two different values"
    );

    // **Just inside the margin, and the margin is flat.** `deletionTimestamp` already
    // is request + grace, so the kubelet's SIGKILL lands *at* it; a margin that added
    // the grace a second time would leave a StatefulSet pod with a one-hour
    // `terminationGracePeriodSeconds` invisible a full hour past its kill deadline —
    // and those are exactly the workloads whose stuck termination blocks the rollout
    // this rule exists for. Sixty seconds covers kubelet observation, watch latency and
    // ordinary skew, and is not proportional to a number that was already spent.
    // **The sixty seconds are written out and not read from [`OVERDUE_MARGIN`]** — a
    // boundary computed from the constant under test agrees with every value it could
    // hold — while the moment they are added to comes off the capture.
    let past_deadline = |secs: i64| {
        Time(
            deadline
                .0
                .checked_add(SignedDuration::from_secs(secs))
                .expect("a minute past a captured deadline is a moment"),
        )
    };
    nothing(
        &findings_at(&["stuck"], past_deadline(60)),
        "a minute past the deadline is not yet stuck",
    );
    assert_eq!(
        findings_at(&["stuck"], past_deadline(61)).len(),
        1,
        "one second past the margin it is"
    );
    assert!(
        grace < 60,
        "this capture's grace is smaller than the flat margin, so the two cannot be \
         told apart by the boundary above — what the margin may not do is *scale* with \
         it. **Capture trip:** a stuck pod with `terminationGracePeriodSeconds: 3600`, \
         where the old formula stayed silent for an hour"
    );
    nothing(
        &findings_at(&["stuck"], past_deadline(-1)),
        "and before the deadline the pod is shutting down normally, which is the case a \
         rule reading `deletionTimestamp` as the request time would flag",
    );
}

/// **The negatives, as a set.** Nine captured pods that are working, including the six shapes
/// this contract was extended for — a native sidecar, a sidecar that is up and not ready, an
/// init container that retried before it succeeded, pod-level requests, a pending in-place
/// resize, a limit declared on the pod and not the container — and one with no limits at all,
/// which is rule 9's case and belongs to the Capacity report.
#[test]
fn every_healthy_capture_produces_no_finding_at_all() {
    let healthy = [
        "healthy",
        "healthy-sidecar",
        "healthy-podlevel",
        "healthy-hostpath",
        "healthy-retry",
        "healthy-unreadysidecar",
        "resize",
        "podlimit",
        "nolimits",
    ];
    // **Which of them the scheduler put on the broken node is not fixed**, so the claim
    // below is read off the capture instead of naming a file: at least one of these runs
    // on the node caught `Ready: Unknown` under the node controller's `unreachable`
    // taint, and its status is a fossil the kubelet stopped updating. That is N1's
    // finding about the *node*, and no pod rule in this box may invent one from a status
    // that stopped moving.
    let quiet = the_quiet_node(&fixture("nodes")).to_string();
    let fossils: Vec<&str> = healthy
        .iter()
        .copied()
        .filter(|n| pod(n).node.as_deref() == Some(quiet.as_str()))
        .collect();
    println!("on {quiet}, the node that stopped answering: {fossils:?}");
    assert!(
        !fossils.is_empty(),
        "no healthy capture landed on {quiet}, so the silence below says nothing about a \
         status that stopped moving — which is half of what this test is for (D71)"
    );
    for name in healthy {
        nothing(
            &findings(&[name]),
            &format!(
                "nothing in {name}.json is broken *now*, which is the only thing Alerts \
                 holds. Not the same claim as 'this pod is fine': {fossils:?} run on \
                 {quiet}, whose kubelet stopped posting"
            ),
        );
    }
    nothing(
        &findings(&healthy),
        "and they are silent together as well as apart",
    );
}

/// **The pod the rule set could not see** (NOTES § D27), and the card that now names it.
///
/// This test is the previous box's guard, turned over rather than deleted: it asserted
/// that `broken-init` produced *nothing*, which was true and was the blind spot. What
/// makes it worth keeping is its shape — it asserts the capture's preconditions before it
/// asserts the outcome, so a capture whose init container had quietly healed cannot pass
/// a widened rule set by producing nothing and calling that agreement.
///
/// **The diagnosis is which container, not that a container is broken.** `migrate` is in
/// `Init:CrashLoopBackOff` with a restart count in the double figures while `app` sits at `PodInitializing`
/// waiting for it, and a card that named `migrate` without saying what an init container
/// *is* reads as an application that will not start — sending the reader to the app's
/// logs, which are empty, because the app has not run (invariant 14).
#[test]
fn the_crash_looping_init_container_is_found_and_the_card_says_what_kind_it_is() {
    let init = pod("init");
    let migrate = container(&init, "migrate");
    assert_eq!(migrate.role, ContainerRole::Init);
    // **The precondition is the loop, not the state the loop was caught in** (NOTES § D114). A
    // crash-looping container alternates between `waiting: CrashLoopBackOff` and the `terminated`
    // run it just left, and `scripts/cluster.sh` § `[init]` certifies both — so demanding the
    // waiting reason here asserted which half `just fixtures` caught rather than that the fixture
    // is still the blind spot. What has to hold either way: it has restarted into rule 5's band,
    // and the run behind it failed.
    assert!(
        migrate.restarts >= RESTARTS_WARN,
        "a capture whose init container stopped restarting proves nothing about the gap: {}",
        migrate.restarts
    );
    assert!(
        migrate
            .last_terminated
            .as_ref()
            .is_some_and(|run| run.exit_code != 0),
        "and the run behind it failed — an init container looping over a *clean* exit is \
         `healthy-retry`, which is this fixture's negative: {:?}",
        migrate.last_terminated
    );
    assert!(
        !matches!(migrate.state, ContainerState::Running { .. }),
        "a capture caught in the ~2s the container is actually up is a Running pod, and \
         certifying that as crash-looping is the lie `cluster.sh`'s predicate excludes: {:?}",
        migrate.state
    );
    assert!(
        matches!(&container(&init, "app").state, ContainerState::Waiting { reason, .. }
            if reason.as_deref() == Some("PodInitializing")),
        "and the app container has to be the *healthy* half of the diagnosis — a pod \
         whose app container was broken too would let a card about `app` pass for a card \
         about `migrate`: {:?}",
        container(&init, "app").state
    );

    let all = findings(&["init"]);
    show(&all);
    // **One card, and it was two until 2026-08-16** (NOTES § D113). Rules 1 and 6 answer this
    // ending with one sentence now — `init.json` carries no termination message, so rule 6 adds
    // no fact and [`one_card_per_action`] collapses it into the CRITICAL card. What the reader
    // loses is a second copy of one instruction; what they keep is the count, the translation and
    // the log command.
    assert_eq!(
        all.len(),
        1,
        "rule 1 on `migrate`, and nothing on `app`: a container that is waiting \
         for the init sequence is not itself broken, and a card about it would send the \
         reader to a log that is empty because the process never ran: {:?}",
        titles(&all)
    );

    for f in &all {
        assert!(
            f.evidence.contains("init container migrate"),
            "the finding has to name the init container — 'the app container is fine, \
             the one before it is not' is the whole diagnosis (D27): {}",
            f.evidence
        );
        assert!(
            f.evidence
                .contains("the app starts only after this one finishes"),
            "and it has to say what an init container is, in words that need no \
             glossary. `init container migrate` alone reads as an application that \
             will not start (invariant 14): {}",
            f.evidence
        );
    }

    // The one card the assertion above counted — taken by position rather than by title,
    // because the title is rule 1's or rule 5's depending on which half of the backoff loop the
    // capture caught and both are this fixture (NOTES § D114).
    let looping = &all[0];
    assert!(
        looping.title.contains("CrashLoopBackOff") || looping.title.contains("restarted"),
        "and it is a card about the loop rather than some other rule answering here: {}",
        looping.title
    );
    assert_eq!(
        looping.severity,
        Severity::Critical,
        "the pod cannot start at all, which is as broken as a pod gets"
    );
    // Read off `initContainerStatuses` — which array the number came from is half of
    // this assertion, and the app container's own count is zero one array away.
    let migrate_count = captured_i32(
        captured_status(&fixture("init"), "initContainerStatuses", "migrate"),
        &["restartCount"],
    );
    // On the card, not in a named field: rule 1 writes the count into its evidence line
    // (`9 restarts`) and rule 5 into its title (`restarted 10 times`), and which of the two is
    // drawing depends on the face the capture was caught in (NOTES § D114). What the requirement
    // says is that the number the reader sees is the **init** container's — the app container's
    // own count is zero, one array away.
    assert!(
        migrate_count > 0,
        "an init container that has never restarted cannot tell the two arrays apart: \
         {migrate_count}"
    );
    assert!(
        looping.title.contains(&migrate_count.to_string())
            || looping.evidence.contains(&migrate_count.to_string()),
        "the init container's own count, not the app container's zero: {} / {}",
        looping.title,
        looping.evidence
    );

    // **Rule 6's card folded into rule 1's on 2026-08-16** (NOTES § D113). Both rules answer this
    // ending with [`failed_run_action`]'s one sentence now, and `init.json` carries no termination
    // message, so rule 6 adds no fact and [`one_card_per_action`] collapses it. The severity that
    // used to be asserted here belongs to the survivor, which is the CRITICAL card — the more
    // severe one wins, which is the fold's own rule.
    assert_eq!(
        looping.severity,
        Severity::Critical,
        "the survivor is the severe card, and the reader keeps the count, the translation and the \
         instruction on it"
    );
    assert_eq!(
        looping.action,
        failed_run_action(
            container(&init, "migrate")
                .last_terminated
                .as_ref()
                .expect("the capture records how the run ended"),
            ContainerRole::Init
        )
        .0,
        "and the instruction is the shared one, so the card that went said nothing this one does \
         not"
    );
}

/// **The sidecar's negative, and the precondition without which it proves nothing.**
///
/// `healthy-sidecar.json` is in the healthy set above, and a widened rule set being
/// silent on it would be a green line whatever its `proxy` container decoded as — a
/// capture whose sidecar came out `Regular` would assert nothing about the role this box
/// added. So the role is asserted here, on the object, before the silence is claimed.
///
/// A native sidecar *is* reached by rules 1–6 ([`analyze`]) — a crashlooping mesh proxy
/// is exactly as broken as a crashlooping app container — so the silence here is the
/// rules agreeing that a running, ready proxy whose restarts stay under the band and
/// whose last run ended cleanly is fine, not the rules failing to look.
#[test]
fn a_healthy_native_sidecar_is_looked_at_by_every_rule_and_still_says_nothing() {
    let p = pod("healthy-sidecar");
    let proxy = container(&p, "proxy");
    assert_eq!(
        proxy.role,
        ContainerRole::Sidecar,
        "`restartPolicy: Always` on an init container is what makes it a sidecar \
         (D51) — without this the test below is about a regular container"
    );
    // A sidecar that has been up for hours has restarted its `sleep 3600` a few times,
    // the same way `healthy.json`'s app container has — what makes it *working* is that
    // it is serving, that the count is under rule 5's band, and that the run it records
    // ended with exit 0, not that nothing has ever happened to it.
    assert!(
        proxy.ready
            && matches!(proxy.state, ContainerState::Running { .. })
            && proxy.restarts < RESTARTS_WARN
            && proxy
                .last_terminated
                .as_ref()
                .is_none_or(|run| run.exit_code == 0),
        "and it has to be a *working* sidecar for its silence to mean anything: {proxy:?}"
    );
    nothing(
        &findings(&["healthy-sidecar"]),
        "nothing about this proxy is broken, and the rules that now read its array have \
         to say so as plainly as they do for a regular container",
    );
}

/// **Rule 7 did not widen with rules 1–6, and this is the only thing that says so.**
///
/// The narrowing is a deliberate silence, and a silence leaves no card to assert — delete
/// the role guard in [`running_but_not_ready`] and the whole suite still passes unless
/// something holds a sidecar that is running and not ready. **`healthy-unreadysidecar.json`
/// is that object**, captured on 2026-08-13: a `proxy` with `restartPolicy: Always` whose
/// readiness probe never passes, beside an `app` container that is serving — the third
/// container role in the one state no capture held before (NOTES § D75). It was a decoded
/// copy of `healthy-sidecar.json` until then.
///
/// **The control is the same capture with the regular container beside it made unready**,
/// which *must* draw the card. Without it this test would pass against a rule 7 that had
/// stopped working altogether, and it would be asserting that a broken rule is quiet rather
/// than that a working one is narrow.
///
/// Why the narrowing, rather than a card each: rule 7's sentence sends the reader to *the
/// readiness probe*, and on a meshed pod the proxy is not the container answering the
/// traffic. What a not-ready sidecar does to its pod's own readiness is a rule of its own
/// (invariant 13), and it is not this one wearing a wider filter.
#[test]
fn rule_seven_stays_on_the_container_that_answers_the_traffic() {
    let sidecar = pod("healthy-unreadysidecar");
    let proxy = container(&sidecar, "proxy");
    let app = container(&sidecar, "app");
    println!("proxy={proxy:?}\n  app={app:?}");
    assert!(
        proxy.role == ContainerRole::Sidecar
            && matches!(proxy.state, ContainerState::Running { .. })
            && !proxy.ready,
        "the capture has to carry a *running* sidecar that is not ready — every other \
         condition of rule 7 is met by it: {proxy:?}"
    );
    assert!(
        app.ready && app.role == ContainerRole::Regular,
        "and the container beside it has to be serving, or the pod is broken for some \
         other reason and the silence below is not the narrowing's: {app:?}"
    );
    assert!(
        sidecar.ready.as_ref().is_some_and(|c| c.status == "False"
            && c.last_transition
                .as_ref()
                .is_some_and(|t| now().0.duration_since(t.0) > NOT_READY_GRACE)),
        "and the pod has been unready for longer than the grace, or the clock is what is \
         silencing the rule rather than the role: {:?}",
        sidecar.ready
    );
    nothing(
        &findings(&["healthy-unreadysidecar"]),
        "a mesh proxy failing its readiness check is not 'the readiness probe of this \
         application is failing', and the card would send the reader to the wrong probe",
    );

    // The control: the identical state on the regular container of the same capture. Its
    // `Ready` condition is already `False` and already old enough, so the one field that
    // moves is the container's own flag (NOTES § D53 — the committed JSON is not touched).
    let broken_app = capture_but("healthy-unreadysidecar", |p| {
        container_status(p, "app").ready = false;
    });
    let all = analyze(&pods_at(vec![broken_app], now()));
    show(&all);
    only(&all, "healthy-unreadysidecar", "not receiving traffic");
    assert_eq!(
        all.len(),
        1,
        "and the same state on the regular container beside it does draw the card — \
         without this the test above would pass against a rule 7 that had stopped firing \
         at all: {:?}",
        titles(&all)
    );
}

/// **The init container that failed twice and then worked** — the commonest init
/// container there is, and the one shape that would have turned this box into two
/// permanent cards on a healthy pod.
///
/// **Captured.** `healthy-retry.json` is that pod: a `wait-for-db` init container that failed
/// three times before it exited `0`, beside an `app` that has been serving since. Until the
/// 2026-08-13 trip this was a retry history written onto `healthy.json`'s `migrate`, which
/// succeeded first time and so had neither a restart count nor a `lastState` — the two fields
/// rules 5 and 6 read, and the reason [`doing_its_job`]'s init branch had nothing to suppress
/// on any committed object.
///
/// The count is what makes the silence mean something: it is at [`RESTARTS_WARN`], so without
/// the suppressor rule 5 draws a card on a pod that is serving, and the failed previous run
/// puts rule 6's permanent WARN beside it. The **red** band is proved on its own captures,
/// `restarts10.json` and `restarts10serving.json`.
///
/// **The control is the same capture with the last attempt still failing**, and it is what
/// makes the silence mean something: the suppressor is about the container having
/// *succeeded*, not about it being an init container, so an init container that gave up owes
/// both cards. Without this half the test would pass just as well against a rule set that had
/// stopped reading init containers altogether.
///
/// **It is the container's *current* state that decides, not `lastState`** — the first draft
/// of this control varied the previous run's exit code and produced nothing at all, because
/// the container was still sitting on the capture's own `exit 0` and was correctly
/// suppressed. A control that cannot fail for the right reason is the defect it was written
/// to catch, one level up.
#[test]
fn an_init_container_that_retried_and_then_succeeded_draws_no_card() {
    let retried = pod("healthy-retry");
    let waiter = container(&retried, "wait-for-db");
    println!("{waiter:?}");
    assert_eq!(waiter.role, ContainerRole::Init);
    assert!(
        waiter.restarts >= RESTARTS_WARN
            && matches!(&waiter.state, ContainerState::Terminated(run) if run.exit_code == 0)
            && matches!(&waiter.last_terminated, Some(run) if run.exit_code != 0),
        "the capture has to be a *finished* init container carrying enough restarts to reach \
         rule 5's band and a previous run rule 6 would fire on, or the silence below is \
         unearned: {waiter:?}"
    );

    nothing(
        &findings(&["healthy-retry"]),
        "this init container did what it was asked to do — it finished, and the pod has \
         been serving ever since. Its restart count is frozen and its failed previous run \
         is kept for the life of the pod, so a card here is permanent and there is \
         nothing behind it to act on (D2)",
    );

    // The other side of the same capture: the suppressor is about *success*, not about the
    // role. An init container that stopped on a non-zero code is why a pod is not starting,
    // and both rules owe it a card. One field moves, on a decoded copy (NOTES § D53).
    let gave_up = capture_but("healthy-retry", |p| {
        container_status(p, "wait-for-db")
            .state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the captured init container ended in a terminated state")
            .exit_code = 1;
    });
    let all = analyze(&pods_at(vec![gave_up], now()));
    show(&all);
    // **One card since 2026-08-16** (NOTES § D113): rules 5 and 6 answer this ending with one
    // sentence and rule 5 carries the duration, so rule 6 adds nothing here and its card folds.
    assert_eq!(
        all.len(),
        1,
        "rule 5 on an init container that gave up, with rule 6's card folded into it: {:?}",
        titles(&all)
    );
    assert_eq!(
        only(
            &all,
            "healthy-retry",
            &format!("restarted {} times", waiter.restarts)
        )
        .severity,
        Severity::Warn,
        "and the band is the one the count puts it in — the card the successful run above \
         must not draw at all"
    );
}

/// **Rule 5's red band, and the half of it that stays amber** — on the two captures the
/// 2026-08-13 trip was sent for. The whole expression is
/// `restarts >= RESTARTS_CRITICAL && !serving`, and until those objects existed the
/// `&& !serving` half had never been read off a real one: no committed capture reached ten
/// restarts at all, so both directions were decided-copy-only.
///
/// `broken-restarts10` and `broken-restarts10serving` are the same manifest but for the
/// readiness probe, so what separates the two cards below is the one field the clause names.
#[test]
fn ten_restarts_is_red_unless_the_container_is_still_answering() {
    let down = container(&pod("restarts10"), "flaky").clone();
    let serving = container(&pod("restarts10serving"), "flaky").clone();
    println!("not serving: {down:?}\nserving:     {serving:?}");
    assert!(
        down.restarts >= RESTARTS_CRITICAL && serving.restarts >= RESTARTS_CRITICAL,
        "both captures have to be past the red band, or neither card below is about it: \
         {} and {}",
        down.restarts,
        serving.restarts
    );
    assert!(
        !doing_its_job(&down) && doing_its_job(&serving),
        "and the pair has to differ in exactly the field the clause reads, or the two \
         severities below are not being told apart by it"
    );

    let red = only(
        &findings(&["restarts10"]),
        "broken-restarts10",
        &format!("restarted {} times", down.restarts),
    )
    .clone();
    show(std::slice::from_ref(&red));
    assert_eq!(
        red.severity,
        Severity::Critical,
        "ten restarts on a container that is not answering is broken now (REQUIREMENTS)"
    );
    assert!(
        !red.title.contains("it is serving now"),
        "and the sentence that is only true of a working container is not on it: {}",
        red.title
    );
    // **The half of D100 that did not move.** A container that is down is dated by the run that
    // ended, `lastState.terminated.finishedAt`, as it always was — only the serving branch reads
    // the run the container is *in*, because only a serving container has one.
    assert_eq!(
        red.timestamp,
        down.last_terminated
            .as_ref()
            .and_then(|run| run.finished_at.clone()),
        "the down card's age is when the last run ended"
    );

    // Read inside the run, because the serving half is the half that ages out: at the pin this
    // capture has been serving 49 hours and rule 5 has stood it down (NOTES § D100). The red
    // half above needs no such moment — a container that is *not* serving never ages out.
    let snapshot = serving_at(pod("restarts10serving"), "flaky");
    let news = snapshot.now.clone();
    let amber = only(
        &analyze(&snapshot),
        "broken-restarts10serving",
        &format!("restarted {} times", serving.restarts),
    )
    .clone();
    show_at(std::slice::from_ref(&amber), &news);
    assert_eq!(
        amber.severity,
        Severity::Warn,
        "the same count on a container that is passing its probes stays amber: a red card \
         whose own title says it is serving is what teaches a reader to stop believing red \
         (NOTES § D2)"
    );
    assert!(
        amber.title.contains("it is serving now"),
        "and the title says so: {}",
        amber.title
    );
    // And the serving card is dated by the run it is in, which is the other half.
    assert_eq!(
        amber.timestamp.as_ref(),
        Some(&began_running(&pod("restarts10serving"), "flaky")),
        "the serving card's age is `state.running.startedAt` (NOTES § D100)"
    );
    assert_ne!(
        amber.timestamp, red.timestamp,
        "the two branches read different fields, or one of these assertions is passing on the \
         other's field"
    );

    // **And the two halves age differently, which is the other thing the clause says.** Ten
    // restarts on a container that came back and has served for two days is not on a screen
    // about what is broken now; ten restarts on one that is still down is, forever, because
    // nothing about it has got better.
    nothing(
        &findings(&["restarts10serving"]),
        "the serving card ages out at the same threshold rule 2's kill does (NOTES § D100)",
    );
    only(
        &findings(&["restarts10"]),
        "broken-restarts10",
        &format!("restarted {} times", down.restarts),
    );
}

/// `restarts10.json` / `restarts10serving.json` with the previous run's **ending** replaced —
/// the shapes no capture in this repository holds (NOTES § D40). [`exited`] moves the code and
/// the reason together, because the kubelet writes the pair.
///
/// **Half of what a capture trip owed here arrived on 2026-08-16** (NOTES § D114). The `exit 0`
/// shape — a container that reaches [`RESTARTS_WARN`] by *finishing*, then **running** and out of
/// `CrashLoopBackOff` — is `probe0.json`: 13 restarts, `lastState.terminated` `0` / `Completed`,
/// `state.running`, `ready: false`. It is read directly by
/// [`each_ending_sends_the_reader_somewhere_the_answer_can_be`], which stopped stretching a plant
/// to get it.
///
/// **What is still owed is the `exit 143` half** — the same shape reached by a `SIGTERM` the
/// program handles — and the manifest for it is one character away in the capture's own `spec`:
/// `restarts10.json`'s container counts its attempts through a `/state` volume and runs
/// `[ "$n" -le 10 ] && exit 1` before `sleep 86400`, so `kill -TERM` in place of that `exit 1`
/// produces it on a real cluster. Nothing committed reaches **that** one today, so this helper
/// still builds it.
fn restarts10_ending(name: &str, exit_code: i32) -> PodSnapshot {
    capture_but(name, |p| exited(p, "flaky", exit_code))
}

/// **Rule 1's defect, one rule over** (NOTES § D85). *"It is serving now, but something keeps
/// killing it"* is not what a clean ending says, and *"check the memory limit and the liveness
/// probe"* is the same sentence one line down: a container cannot breach its memory limit and
/// come back as `143`, because a cgroup breach is a `SIGKILL`.
///
/// Reachable without an unusual manifest: a program that exits `0` a few times and then blocks
/// is serving, with a restart count, and out of `CrashLoopBackOff` — so rule 5 is the only rule
/// left holding it. Rule 6 exempts `0` and `143` outright, and rule 1 needs the backoff state.
///
/// The control is the capture as it stands, `exit 1`, which keeps the sentence: the wording is
/// not being removed, it is being made conditional on the thing it claims.
#[test]
fn a_serving_container_that_finished_cleanly_is_not_one_something_keeps_killing() {
    // Every card in this test is a *serving* one, so every one of them is read inside the run
    // the capture is sitting in: at the pin the container has been up for 49 hours and rule 5
    // has stood the card down (NOTES § D100), which is its own test one function up.
    let snapshot = serving_at(pod("restarts10serving"), "flaky");
    let news = snapshot.now.clone();
    let killed = only(
        &analyze(&snapshot),
        "broken-restarts10serving",
        "restarted 10 times",
    )
    .clone();
    show_at(std::slice::from_ref(&killed), &news);
    assert!(
        killed.title.contains("something keeps killing it")
            && killed.action == failed_run_action(&exited_run(1), ContainerRole::Regular).0,
        "the control is the committed capture — a serving container whose last run exited 1, \
         where the title is this rule's and the instruction is the one all three rules give this \
         ending (NOTES § D113): {} / {}",
        killed.title,
        killed.action
    );

    for (exit_code, said, printed) in [
        (
            0,
            ", and the last run on record finished cleanly",
            "exit 0 (the run ended without an error)",
        ),
        (
            143,
            ", and the last run on record was stopped",
            "exit 143 (stopped with SIGTERM, which is an ordinary shutdown and not an error)",
        ),
    ] {
        let plant = restarts10_ending("restarts10serving", exit_code);
        let c = container(&plant, "flaky");
        println!("{c:?}");
        assert!(
            c.restarts >= RESTARTS_WARN
                && doing_its_job(c)
                && c.last_terminated.as_ref().map(|r| r.exit_code) == Some(exit_code),
            "the plant has to stay a *serving* container past the band, or the title below is \
             not the one being tested: {c:?}"
        );
        let image = c.image.clone();

        let snapshot = serving_at(plant, "flaky");
        let moment = snapshot.now.clone();
        let all = analyze(&snapshot);
        show_at(&all, &moment);
        assert_eq!(
            all.len(),
            1,
            "rule 5 alone — rule 6 exempts both of these codes, so nothing else speaks for \
             this container: {:?}",
            titles(&all)
        );
        let card = only(&all, "broken-restarts10serving", "restarted 10 times");

        assert!(
            !card.title.contains("killing"),
            "exit {exit_code}: one exit code names an ending, never an agent, and the card \
             that names one is contradicted by the object it was drawn from: {}",
            card.title
        );
        assert!(
            card.title.contains("it is serving now") && card.title.contains(said),
            "the container is still up and the reader is still told which run this is about — \
             one `lastState` is one run, and a claim over all ten restarts is the absolute \
             NOTES § D85 removed from rule 1: {}",
            card.title
        );
        assert!(
            !card.action.contains("memory limit"),
            "and the action goes with it: a memory-limit breach is a SIGKILL and arrives as \
             137, never as {exit_code}: {}",
            card.action
        );
        // **The title now claims how the run ended, so the code it claims it from is on the
        // card** — and rule 6 is silent on exactly these two endings, so without this line the
        // number appears nowhere on the screen (invariant 4, NOTES § D85).
        assert!(
            card.evidence.contains(printed),
            "the reader is shown what the title was read off, in the words rule 6 would have \
             used: {}",
            card.evidence
        );
        // **And it is shown *before* the image, because evidence is the one card line the screen
        // is allowed to cut** (`screens/alerts.md` § The height: three lines, wrapped at 51
        // columns). Behind a digest-pinned image the exit code is what falls off the bottom —
        // leaving a title that says how the run ended above evidence that no longer says it.
        let (at_code, at_image) = (
            card.evidence
                .find(printed)
                .expect("the exit code is on the card"),
            card.evidence
                .find(&image)
                .expect("and so is the image it ran"),
        );
        assert!(
            at_code < at_image,
            "the fact the title rests on goes ahead of the one it does not: {}",
            card.evidence
        );
        assert_eq!(
            card.severity,
            Severity::Warn,
            "the band still answers *is this container serving*, and this one is"
        );
    }
}

/// **The arms' actions, and the command each owes** — the half of the box that is not the title
/// (NOTES § D85, invariant 4).
///
/// **`kubectl describe pod` and `kubectl get pod -o yaml` do not overlap where this rule needs
/// them to:** describe prints the probes and the `Unhealthy` / `Killing` events and no
/// `restartPolicy`; `get -o yaml` prints `restartPolicy` and no events at all. A clean ending is
/// told from a probe kill by the events, so these arms name the events and no card names
/// `restartPolicy` at all — rule 1's clean-exit branch did, under the one command that cannot
/// show it, until NOTES § D88's second round moved it here. **An action naming a field its own
/// card cannot display is the defect this box is fixing, not a smaller version of it.**
#[test]
fn each_ending_sends_the_reader_somewhere_the_answer_can_be() {
    // **The long arm, on a capture instead of a plant since 2026-08-16** (NOTES § D113, § D114).
    // This assertion used to stretch `restarts10serving`'s two-second run to five minutes by
    // hand, because nothing committed carried a clean ending past [`PROBE_FLOOR`]. `probe0.json`
    // does, and it is the real thing rather than the shape: a `livenessProbe` of `false` with
    // `initialDelaySeconds: 30` kills a container that traps `SIGTERM` and exits `0`, so the
    // kubelet records a **32-second** run ending cleanly — a genuine probe kill behind the exact
    // ending this arm is drawn for, which is what the hand-moved `startedAt` was standing in for.
    //
    // The short arm is asserted on the helper in
    // [`the_clean_exit_actions_fit_the_card_they_are_drawn_on`] and end to end on `exit0.json`
    // (2s) in [`a_program_that_finished_is_not_a_container_that_crashed`], so the pair is two
    // captures now rather than one capture and one edit.
    let capture = pod("probe0");
    let run = container(&capture, "app")
        .last_terminated
        .clone()
        .expect("the capture records the run the probe ended");
    println!("{run:?}");
    assert_eq!(
        (run.exit_code, run.reason.as_deref()),
        (0, Some("Completed")),
        "the ending this arm is drawn for is a clean one, read off the capture"
    );
    assert!(
        run_length(&run) > Some(PROBE_FLOOR),
        "and it has to sit on the long side of the threshold, or this test measures the other \
         arm: {run:?}"
    );
    assert_eq!(
        container(&capture, "app").role,
        ContainerRole::Regular,
        "this is the *plain container* half of the role split, and the assertion below is only \
         about it — the sidecar half is the test after this one"
    );
    let all = findings(&["probe0"]);
    show(&all);
    let card = only(&all, "broken-probe0", "restarted 13 times");
    // **Both readings stay open.** An application that traps SIGTERM and shuts down tidily
    // reports `0`, and the kubelet writes `0` / `Completed` whichever of the two happened — so
    // an action that says nothing killed it is wrong about every gracefully-stopping program in
    // the cluster.
    assert!(
        !card.action.contains("nothing killed"),
        "an application that traps SIGTERM and shuts down tidily reports 0, and the kubelet \
         writes 0 / Completed whichever of the two happened — so *nothing killed that run* is \
         wrong about every gracefully-stopping program in the cluster: {}",
        card.action
    );
    assert!(
        card.action.contains("does not say who ended the run")
            && card.action.contains("If nothing did"),
        "both readings have to stay live — the program finished, or something asked it to stop \
         and was obeyed — and the sentence that picks one is the blocker this round is fixing: {}",
        card.action
    );
    // **This rule orders by the duration too, since its evidence line started carrying one**
    // (NOTES § D113). It passed `None` while the card showed no duration — a visible order with
    // a hidden reason is worse than no order — and [`ran_for`] joined its facts for the fold, so
    // the constraint that was blocking it is met and rules 1 and 5 answer this ending alike.
    assert_eq!(
        card.action,
        finished_action(ContainerRole::Regular, Some(PROBE_FLOOR)),
        "the arm the duration on its own card selects: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("ran for"),
        "and the premise of that, read off the card rather than assumed — the reader can see the \
         fact the order turns on: {}",
        card.evidence
    );
    assert!(
        card.action.contains("events"),
        "and it has to send the reader where the two are told apart — a probe kill is written \
         into the pod's events and nowhere else this card can reach: {}",
        card.action
    );
    // **How long the events keep it, on this rule above all** (NOTES § D113). Rule 1's container
    // is inside a backoff that caps at five minutes, so a `Killing` line is always minutes old;
    // this rule's is not bounded that way and the run may have ended hours ago, where
    // `Events: <none>` reads to a beginner as *nothing stopped it* — which is the reading that
    // walks them into the Job door for a Deployment that is fine. **This capture is the case
    // exactly**: `broken-probe0`'s clean run ended 13 hours before the pinned `now`, so the
    // `Killing` line the action sends the reader after has already expired (NOTES § D114).
    assert!(
        card.action.contains("kept an hour"),
        "the reader is told the events expire, or an empty Events list reads as a verdict: {}",
        card.action
    );
    // **And name the line to look for**, or *check the events* is a pointer at a page — and name
    // it for every killer rather than for the probe alone. This rule's own pin on the clause;
    // the reasoning is at the function (NOTES § D88).
    names_the_killer_and_not_only_the_probe(&card.action);
    // **The positive half of the split, which its negative cannot stand in for.** The events
    // above are in *both* arms' sentences, so they hold just as well over a rule that lost the
    // split and gave every role the sidecar wording. The workload named here is the one thing
    // only the plain-container arm says — and the one thing the sidecar test after this asserts
    // is absent (rule 1 carries the same pair).
    assert!(
        card.action.contains("Job") && card.action.contains("CronJob"),
        "a plain container whose program is meant to finish belongs in the workload built for \
         that, and naming it is the whole of this arm — an action that only says where to look \
         leaves the reader where they started: {}",
        card.action
    );
    // **And the reading that is neither of the other two**, pinned here as well as in rule 1's
    // test: a program that is not meant to finish and stops anyway is quitting early, which is
    // the one of the three the card cannot send anybody anywhere for. It went missing once, out
    // of the arm both rules share, and only one rule's tests would have noticed (NOTES § D88).
    assert!(
        card.action.contains("quitting early"),
        "a web server that exits 0 in under a second is not a batch job, and a card offering it \
         only a CronJob has read the same 0 the other two branches did and picked one: {}",
        card.action
    );
    // **And both of them hang off the conditional, which is the pin this rule was doing without**
    // (NOTES § D88). Rule 1's plain-container test and both sidecar tests carry it; this arm did
    // not, so moving the two readings ahead of *if nothing stopped it* — a card that says one
    // exit code cannot name who stopped the container and then names it — took rule 1 red alone
    // while this rule drew the same sentence. Clause-level coverage is per caller, exactly as
    // arm-level coverage is.
    the_verdict_hangs_off_the_conditional(&card.action, "If nothing did", "Job");
    the_verdict_hangs_off_the_conditional(&card.action, "If nothing did", "quitting early");
    assert!(
        !card.action.contains("restartPolicy"),
        "and it may not name the field rule 1 used to name here: the command below prints the \
         events and not that, and a card that sends the reader to a field its own command hides \
         is invariant 4 broken in the direction this box is closing: {}",
        card.action
    );
    // **The whole string, because the command is display text the reader retypes**: a wrong verb
    // and a wrong object are the same failure, and `kubectl get deployment <pod-name>` compiles
    // just as well while returning `NotFound`.
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-probe0 -n default"),
        "an action naming the events owes the one command that prints them"
    );

    let stopped = restarts10_ending("restarts10serving", 143);
    let all = serving_findings(stopped, "flaky");
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    // **Three places now, and the third arrived with the shortening rather than despite it**
    // (NOTES § D113). A polite stop has three producers at the pinned version: a health check, an
    // in-place resize with `resizePolicy: RestartContainer` — which VPA drives on a loop, and
    // which a reader being resized was sent straight past — and a userspace memory killer on the
    // node. **`earlyoom` and not `systemd-oomd`**: the latter kills a cgroup with SIGKILL, so it
    // can only ever produce `137` and never the `143` this arm is about.
    for door in ["probes", "resize", "earlyoom"] {
        assert!(
            card.action.contains(door),
            "143 is a container that was asked to stop and stopped, and *{door}* is one of the \
             three things that ask: {}",
            card.action
        );
    }
    // **The resize door has to send the reader to the *events*** (invariant 4, NOTES § D113).
    // `kubectl describe pod | grep -ic resizePolicy` is 0 — the field itself is only in
    // `get -o yaml`, which this card does not carry — while `ResizeStarted`, `Killing … resize
    // requires restart` and `ResizeCompleted` are durably under `Events:`, measured. The
    // `PodResizeInProgress` condition the first draft leaned on is gone by the time the card is
    // drawn.
    assert!(
        card.action.contains("events"),
        "the resize is findable in the pod's events and nowhere else `describe` prints: {}",
        card.action
    );
    for absent in ["resizePolicy", "can be set to"] {
        assert!(
            !card.action.contains(absent),
            "*{absent}* names a spec field this card's command does not show: {}",
            card.action
        );
    }
    // **And the log clause names its subject.** *Its own log* sat after *the node, where a memory
    // killer…*, whose nearest antecedent is the node — a door pointing at the wrong room.
    assert!(
        !card.action.contains("Its own log") && card.action.contains("container's own log"),
        "the log clause says whose log it is: {}",
        card.action
    );
    // **And the place *not* to look, which the first draft of the shortening cut and called a
    // restatement** (NOTES § D113). It is not one: the clause before it names a cause, this one
    // names somewhere the answer is not, and a beginner's first move on a dead container is
    // `kubectl logs`. Pinned as a requirement — the card says the log holds no crash — rather
    // than as a phrasing.
    assert!(
        card.action.contains("log") && card.action.contains("not a crash"),
        "a container that was asked to stop left no error in its own log, and the card that does \
         not say so sends the reader there first: {}",
        card.action
    );
    assert!(
        !card.action.contains("systemd-oomd"),
        "and not the killer that cannot produce this code at all: {}",
        card.action
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-restarts10serving -n default"),
        "and `describe` prints all three — the probes, the resize conditions and the events"
    );

    // **The arm with no previous run at all.** `restartCount` survives a `lastState` the status
    // no longer carries, and a count on its own supports the count. **Rule 1 answers this shape
    // with the same sentence since 2026-08-16** — its fall-through said *keeps crashing* on the
    // strength of the waiting reason beside it, and neither the crash nor the log it pointed at is
    // on the pod any more ([`no_record_action`], NOTES § D113).
    //
    // **A kubelet restart is not one of the producers**, though it reads like one: the kubelet
    // re-derives status from the runtime, the dead container is still there, and `lastState`
    // comes back. The real ones are container GC — the node-wide dead-container cap pushing a
    // container below the per-container keep of 1 — a runtime that lost its container store
    // while `/var/log/pods` survived to feed `calcRestartCountByLogDir`, and a hand-run
    // `crictl rm`.
    let forgotten = capture_but("restarts10serving", |p| {
        container_status(p, "flaky").last_state = None;
    });
    assert!(
        container(&forgotten, "flaky").last_terminated.is_none(),
        "the plant has to actually remove the previous run"
    );
    let all = serving_findings(forgotten, "flaky");
    let card = only(&all, "broken-restarts10serving", "restarted 10 times");
    assert_eq!(
        card.title, "Container has been restarted 10 times — it is serving now",
        "a count with no run to read it against supports the count and not a word more — \
         *something keeps killing it* here rests on a number that says only how often the \
         container started"
    );
    // **What this arm may say, pinned as the requirement and not as a word.** `contains("log")`
    // was a hole: it stayed green over *"look in the node's system log for a memory killer"*,
    // an action pointing somewhere its own command does not print. These are the claims a bare
    // count cannot carry.
    for absent in ["killing", "memory limit", "probe", "node"] {
        assert!(
            !card.action.contains(absent),
            "with no previous run to read, *{absent}* is either a claim the count cannot carry \
             or a place this card's command does not print: {}",
            card.action
        );
    }
    // **And the thing it must still give the reader: a move.** Claiming nothing is only half an
    // arm — a card that says the pod forgot and stops there is a shrug. The next restart writes
    // the run back, which is a real instruction and one nothing else on this rule says.
    assert!(
        card.action.contains("the next restart will"),
        "an arm with nothing to claim still owes the reader something to do: {}",
        card.action
    );
    // **`kubectl logs --previous` may never be offered here, and the reason is the branch
    // condition itself.** The kubelet gates that flag on `lastState.terminated.containerID`
    // (`validateContainerLogStatus`) — the field whose absence is what put this card in this
    // arm — so the command it used to carry returns `BadRequest` on every object that can reach
    // it, with no exception to test for. A green test asserting a command that always fails is
    // worse than no test, and this file shipped one (invariant 4).
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-restarts10serving -n default"),
        "the events are the only record that may survive the one the pod dropped, and they are \
         in `describe` — which is also the one command this arm can offer that runs at all"
    );
}

/// **The sidecar that keeps finishing, one rule over** — the blocker NOTES § D85 found inside
/// the fix for rule 1, and the one this rule's first draft rebuilt: *a program that is meant to
/// finish belongs in a Job or a CronJob*, printed one line under an evidence line reading *it
/// runs beside the app the whole time*, about a container whose pod may already **be** a Job.
///
/// **`healthy-sidecar.json` carries the role and the readiness** — `proxy` is an init container
/// with `restartPolicy: Always`, running and ready — and the plant supplies the history: a clean
/// previous run and a count at [`RESTARTS_WARN`]. A count is the one field every cluster produces
/// at every value (NOTES § D40, § D53).
///
/// **The clean run was the capture's own until 2026-08-16** (NOTES § D114) — see
/// [`a_sidecar_that_exits_cleanly_is_not_told_to_move_to_the_workload_it_is_already_in`] for why
/// a `sleep 3600` sidecar carries one after a long capture session and not after a short one.
///
/// **What a future trip owes here:** the same pod left alone for three hours, so its `sleep 3600`
/// finishes three times and both the run and the count are read rather than built. Nothing has
/// been captured at that count because nothing has waited.
#[test]
fn a_sidecar_that_keeps_finishing_is_not_told_to_move_to_a_job() {
    let restarted = capture_but("healthy-sidecar", |p| {
        // The run first — it counts the restart that goes with it — then the band, so the number
        // the card is drawn from is the one this test names.
        ended_as(p, "proxy", 0, None, None);
        container_status(p, "proxy").restart_count = RESTARTS_WARN;
    });
    let proxy = container(&restarted, "proxy");
    println!("{proxy:?}");
    assert!(
        proxy.role == ContainerRole::Sidecar
            && doing_its_job(proxy)
            && proxy.last_terminated.as_ref().map(|r| r.exit_code) == Some(0),
        "the role and the readiness are the capture's own and the clean previous run is the \
         plant's — without all three this is not the card: {proxy:?}"
    );

    // A serving card, so read inside the run this sidecar is sitting in (NOTES § D100).
    let all = serving_findings(restarted, "proxy");
    let card = only(&all, "healthy-sidecar", "restarted 3 times");
    assert!(
        card.evidence
            .contains("it runs beside the app the whole time"),
        "the evidence line the action has to agree with: {}",
        card.evidence
    );
    assert!(
        !card.action.contains("Job") && !card.action.contains("CronJob"),
        "this container runs beside the app for the pod's whole life and its pod may already \
         be a Job — telling its author to move it to one is the contradiction D85 removed from \
         rule 1, rebuilt here: {}",
        card.action
    );
    assert!(
        card.action.contains("events"),
        "and the reader is still left somewhere to look — an action that only says where *not* \
         to look is no action at all: {}",
        card.action
    );
    // **And what to look for once they are open.** *Check the events* with nothing named is the
    // instruction that leaves the reader where they started, and this role's card owes the same
    // door the plain container's does — pinned per caller (NOTES § D88).
    names_the_killer_and_not_only_the_probe(&card.action);
    // The clause this arm opens with, pinned here as well as in rule 1's sidecar test: a shared
    // sentence owes each caller a pin, and deleting this one upstream took rule 1's tests red
    // alone while both rules' cards lost the reading (NOTES § D88).
    assert!(
        card.action.contains("does not say who ended the run"),
        "a sidecar that shuts down tidily on SIGTERM reports 0 like one that chose to stop, so \
         the card may not read this 0 as a decision the container made: {}",
        card.action
    );
    // **The positive half of the split, which none of its negatives can stand in for.** Every
    // assertion above is satisfied word for word by the *init* arm's sentence — it names no Job,
    // and it does name the events — so all three survive the split collapsing in the one
    // direction the F1 test cannot see. This is the sentence only this arm carries, and it is
    // the arm's whole point: a container that runs beside the app for the pod's whole life is
    // not finishing early, it is finishing at all.
    assert!(
        card.action.contains("finishing at all is the bug"),
        "a sidecar is the one role where ending cleanly is itself the fault, and a card that \
         stops short of saying so has told the reader only what the problem is not: {}",
        card.action
    );
    // **And it is conditional, exactly as the plain container's two readings are** (NOTES § D88).
    // The card opens by saying this `0` cannot name who ended the run and closes by calling
    // finishing the fault. With no conditional between the two it names a probe kill and then
    // rules it out one sentence later, off the same single exit code — the defect removed from
    // the plain-container arm, one arm over.
    the_verdict_hangs_off_the_conditional(
        &card.action,
        "If nothing did",
        "finishing at all is the bug",
    );
}

/// **`healthy-retry.json`'s init container run again from the start**, which is what pod sandbox
/// recreation does — a node reboot, a containerd restart, a killed sandbox. Kubernetes re-runs
/// every init container while `restartCount` and `lastState` persist on the same pod object, so
/// the container is `Running` and `ready: false` with its previous generation behind it, and the
/// app is back on `PodInitializing`.
///
/// **What this plant moves**: the previous run's exit code and reason ([`exited`]), the init
/// container to `Running` — `ready: false`, `started: true`, which is what the kubelet writes for
/// an init container that is up and has not finished — the app to waiting on `PodInitializing`
/// ([`never_ran`]), and the pod itself back to where a rebuild leaves it ([`sandbox_rebuilt`]).
/// `restartCount` is the capture's own **plus the one restart this plant stages**: `startContainer`
/// reads the previous instance's count and writes `RestartCount + 1`, so a container the kubelet
/// has just run again is one higher than the run now sitting in `lastState`. It was the
/// capture's `3` beside a fresh `Running` state until 2026-08-14 — a number no kubelet writes for
/// that pair, and it sat exactly on [`RESTARTS_WARN`], where the rule's own boundary is
/// (NOTES § D40, § D53). The ending is the parameter: it is the only thing separating this rule's
/// two init arms.
///
/// **It was left inconsistent in three places until 2026-08-14**: `phase` stayed `Running`,
/// `Initialized` stayed `True`, and the init container kept the capture's `started: false` beside
/// a running state. Two of those were argued inert and are — nothing in `rules.rs` reads
/// `Initialized`, and [`running_but_not_ready`] leaves on `role != Regular` before it reaches
/// `started`. **The third argument was wrong**: `phase: Pending` was said to pull
/// [`no_node_accepted_it`] and [`nothing_has_looked_at_it`] into a test about one card, and both
/// leave on this pod before they read the phase — the first on a `PodScheduled` that is `True`,
/// the second on its being there at all (NOTES § D88). A plant is worth the shape it builds, so
/// all three are now written, inert or not.
fn init_run_again(exit_code: i32) -> PodSnapshot {
    init_previous_run(exit_code, None, None, false)
}

/// **[`init_run_again`] over any previous run, and in either state an init container reaches
/// rules 1, 5 and 6 in** — the driver behind
/// [`no_card_about_an_init_container_ever_names_a_probe`], which owes every
/// `(exit code, reason)` shape those three rules can draw an `Init` card from (NOTES § D29).
///
/// `reason: None` keeps the pairing [`exited`] writes — `Completed` beside `0`, `Error` beside
/// everything else, which is what the API emits. It is overridden for the one shape where the
/// kubelet does not read the reason off the code at all: it writes [`STATUS_LOST`] and a sentence
/// of its own into `message`, where it could not read a status rather than watching a run end
/// (NOTES § D90).
///
/// **`looping: true` is the wait, `false` is the re-run.** The first is rule 1's state, taken
/// from [`backing_off`]; the second is rule 5's and rule 6's, and adds the restart the kubelet
/// counts when it starts the container again. Both leave the app on `PodInitializing` behind a
/// pod put back where a rebuilt sandbox leaves it — an init container in either state beside a
/// *ready* app in a `Running` pod is a shape no kubelet writes.
fn init_previous_run(
    exit_code: i32,
    reason: Option<&str>,
    message: Option<&str>,
    looping: bool,
) -> PodSnapshot {
    init_previous_run_counting(exit_code, reason, message, looping, None)
}

/// **The same, with the restart count named rather than inherited** — `None` keeps the count
/// [`init_previous_run`] builds, which is `healthy-retry.json`'s own three plus the one
/// `startContainer` adds.
///
/// **The count is the field that decides which of two cards is left on the screen** (NOTES § D102):
/// past [`RESTARTS_WARN`] rule 5 draws beside rule 6, both answer this ending with
/// [`unwatched_action`], and the pair collapses onto rule 5's card. Under the band rule 5 stands
/// down and rule 6's own card is the one drawn — which is the shape a sandbox rebuild under a
/// *young* init container writes, and the only one that can prove rule 6's title on this role.
fn init_previous_run_counting(
    exit_code: i32,
    reason: Option<&str>,
    message: Option<&str>,
    looping: bool,
    restarts: Option<i32>,
) -> PodSnapshot {
    capture_but("healthy-retry", |p| {
        ended_as(p, "wait-for-db", exit_code, reason, message);
        if looping {
            backing_off(p, "wait-for-db");
        } else {
            let init = container_status(p, "wait-for-db");
            init.state = Some(ApiContainerState {
                running: Some(ContainerStateRunning {
                    started_at: Some(time("2026-08-13T23:34:00Z")),
                }),
                ..ApiContainerState::default()
            });
            init.ready = false;
            init.started = Some(true);
            init.restart_count += 1;
        }
        // Last, so the number named is the one the card is drawn from — the branch above adds the
        // restart `startContainer` counts, and an override placed before it would land one out.
        if let Some(n) = restarts {
            container_status(p, "wait-for-db").restart_count = n;
        }
        never_ran(p, "app", "PodInitializing", None);
        sandbox_rebuilt(p);
    })
}

/// **A captured container's previous run rewritten whole** — the code [`exited`] writes, plus the
/// `reason` and the `message` beside it.
///
/// `reason: None` keeps [`exited`]'s pairing — `Completed` beside `0`, `Error` beside everything
/// else, which is what the API emits. It is overridden for the one pair the kubelet does not read
/// off the code at all: [`STATUS_LOST`] and a sentence of its own, written where it could not read
/// a status rather than where it watched a run end (NOTES § D90).
///
/// **Where the capture holds no previous run the plant creates one, and two things go with it.**
/// `startContainer` writes `RestartCount + 1` when it runs a container again, so a `lastState`
/// beside a count of `0` is a pair no kubelet produces; and the run is stamped **before** the
/// state the capture is in — every capture in this repository was taken with its containers
/// running from `22:33:10Z`, and a previous run that ended after the current one started is a
/// timeline no cluster writes. A plant is only worth the shape it builds (NOTES § D40, § D53).
fn ended_as(pod: &mut Pod, name: &str, code: i32, reason: Option<&str>, message: Option<&str>) {
    let status = container_status(pod, name);
    if status
        .last_state
        .as_ref()
        .and_then(|s| s.terminated.as_ref())
        .is_none()
    {
        status.last_state = Some(ApiContainerState {
            terminated: Some(ContainerStateTerminated {
                started_at: Some(time("2026-08-13T22:32:30Z")),
                finished_at: Some(time("2026-08-13T22:33:00Z")),
                ..ContainerStateTerminated::default()
            }),
            ..ApiContainerState::default()
        });
        status.restart_count += 1;
    }
    exited(pod, name, code);
    let run = container_status(pod, name)
        .last_state
        .as_mut()
        .and_then(|s| s.terminated.as_mut())
        .expect("the run above is either the capture's or this plant's");
    if let Some(reason) = reason {
        run.reason = Some(reason.to_string());
    }
    run.message = message.map(str::to_string);
    // **The two reasons the kubelet writes itself carry three fields and no more**, so the plant
    // strips the rest rather than leaving the capture's behind: it is describing a run it did not
    // watch, so there are no stamps and no `containerID` to describe it with.
    //
    // **Read out of the source rather than inferred**, because the choice moves what rule 1
    // draws. At v1.36.1 both struct literals set `Reason`, `Message` and `ExitCode` and stop —
    // `kubelet_pods.go:2621-2625` for [`STATUS_LOST`], `:2581-2585` for [`RESTART_ALL`] — and an
    // unset `metav1.Time` marshals to `null`
    // (`apimachinery/pkg/apis/meta/v1/time.go:162-166`, *"Encode unset/nil objects as JSON's
    // null"*). `k8s-admin` measured the same nulls for [`STATUS_LOST`] on kind; [`RESTART_ALL`]'s
    // were not measured and are settled here from the identical literal.
    //
    // A plant that kept the capture's stamps would prove the rules against an object no cluster
    // produces (NOTES § D29, § D40), and two shipped behaviours read exactly these fields:
    // [`lasted`] for the evidence line and [`Finding::timestamp`] for the age.
    if matches!(run.reason.as_deref(), Some(STATUS_LOST | RESTART_ALL)) {
        run.started_at = None;
        run.finished_at = None;
        run.container_id = None;
    }
}

/// **A plain init container that finished its work and was run again** — the third role on the
/// clean ending. Rule 1 was reported not to need it, on the reading that `CrashLoopBackOff` gates
/// its own arm; it does not, and the test that shows why is
/// [`a_plain_init_container_backing_off_after_a_clean_run_is_not_told_finishing_is_its_bug`]
/// (NOTES § D88). The two rules reach this role by one producer and now answer it with one
/// sentence ([`finished_action`]); this test is that sentence's pin on **this** rule.
///
/// **The producer is pod sandbox recreation** — a node reboot, a containerd restart, a killed
/// sandbox. Kubernetes re-runs every init container from the start while `restartCount` and
/// `lastState` persist on the same pod object, so three generations reach [`RESTARTS_WARN`] with
/// a clean `exit 0` behind them and the container `Running`, `ready: false`, with the app back
/// behind it on `PodInitializing`.
///
/// **Given the sidecar's sentence this card is wrong twice**, and both are NOTES § D85's own
/// shape: it sends the reader after a probe that [`stopped_action`] refuses to name on this very
/// container — one rule contradicting itself between two of its own arms — and it calls
/// finishing a bug one line under an evidence line reading *the app starts only after this one
/// finishes*. A plain init container **is** meant to finish.
#[test]
fn a_plain_init_container_that_was_re_run_is_not_told_that_finishing_is_its_bug() {
    let rerun = init_run_again(0);
    let waiter = container(&rerun, "wait-for-db");
    println!("{waiter:?}");
    assert!(
        waiter.role == ContainerRole::Init
            && !doing_its_job(waiter)
            && waiter.restarts >= RESTARTS_WARN
            && waiter.last_terminated.as_ref().map(|r| r.exit_code) == Some(0),
        "the exemption at the top of this rule reads the *current* state, so a plain init \
         container that is running again is not exempt whatever its previous run says — that is \
         what makes this arm reachable at all: {waiter:?}"
    );

    let all = analyze(&pods_at(vec![rerun], now()));
    show(&all);
    // Four, not three: the plant is a restart, and the kubelet writes the previous instance's
    // count plus one when it runs the container again ([`init_run_again`]).
    let card = only(&all, "healthy-retry", "restarted 4 times");
    assert!(
        card.evidence
            .contains("the app starts only after this one finishes"),
        "the evidence line the action has to agree with — this container finishing is the \
         contract, not the fault: {}",
        card.evidence
    );
    assert!(
        !card.action.contains("bug"),
        "a plain init container is *meant* to finish, and a card calling that the bug argues \
         with its own evidence line: {}",
        card.action
    );
    for probe in ["liveness", "readiness", "startup"] {
        assert!(
            !card.action.contains(probe),
            "and this rule's own {probe}-refusing arms are right beside it — one rule \
             contradicting itself about one container is the disagreement NOTES § D85 opens \
             with, not a smaller version of it: {}",
            card.action
        );
    }
    assert!(
        card.action.contains("sandbox"),
        "what re-ran a container that had already finished is the question, and the answer is \
         at the pod and the node, not inside it — an action that only says where *not* to look \
         is no action at all: {}",
        card.action
    );
    // **The one arm whose evidence expires on a clock, and it has to say so.** `--event-ttl`
    // defaults to an hour while `restartCount` never decreases, so a card drawn from three
    // rebuilds during a 22:00 node reboot still prints at 09:00 — and `describe` then shows
    // `Events: <none>`. Sending a reader to an empty list without warning them is how a tool
    // teaches them to stop believing it.
    assert!(
        card.action.contains("about an hour"),
        "the events this card points at are routinely gone by the time it is read, and the card \
         that does not say so has promised a record that is not there — named as a fact about \
         the cluster, because the count that used to carry the sentence is rule 5's and rule 1 \
         shares the string with `0` restarts on the card (NOTES § D88): {}",
        card.action
    );
    // **And what is left to read once they have gone.** Both rules take this sentence, so both
    // owe its closing clause a pin: it is the arm's only answer after the hour is up, and what
    // rebuilt the sandbox happened on the node and is recorded there, by the kubelet and the
    // container runtime, outside the hour the pod's events last. An ending that says the reason
    // is gone shuts the question the action opened with (NOTES § D88).
    assert!(
        card.action.contains("node"),
        "the action opens by asking what re-ran the container and has to answer it for the \
         reader who reads this card an hour later too — an arm whose only pointer expires is a \
         dead end dressed as an instruction: {}",
        card.action
    );
    // **And it is named as what outlasts the record, which is the half three `contains` calls
    // cannot see**: *"The node kept about an hour of events; after that nothing is left"* holds
    // the sandbox, the hour and the node and leaves the reader nowhere. Pinned here as well as in
    // rule 1's init test — one shared sentence, one pin per caller (NOTES § D88).
    the_place_outlasts_the_record(&card.action);
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod healthy-retry -n default"),
        "and the events that record a rebuilt sandbox, and the node the pod sits on, are both \
         in that one output (invariant 4)"
    );
}

/// **The same init container on the ending it reaches most often** — and the arm that was
/// role-blind until this box: *check the memory limit and the liveness probe* was handed to every
/// role, one arm above [`stopped_action`] refusing to name a probe on this very container.
/// One card contradicting itself is what NOTES § D85 opens with, so the claim survives the split
/// and the probe does not.
///
/// **The failure is the capture's own** — `wait-for-db` failed three times before it succeeded,
/// and that `exit 1` is asserted against the committed JSON below rather than planted, so only
/// the *current* state is synthesized (NOTES § D53).
#[test]
fn a_failing_init_container_is_not_sent_to_a_probe_it_may_not_have() {
    let captured_code = captured_i32(
        captured_status(
            &fixture("healthy-retry"),
            "initContainerStatuses",
            "wait-for-db",
        ),
        &["lastState", "terminated", "exitCode"],
    );
    let failing = init_run_again(captured_code);
    let waiter = container(&failing, "wait-for-db");
    println!("{waiter:?}");
    assert!(
        waiter.role == ContainerRole::Init
            && !doing_its_job(waiter)
            && waiter.restarts >= RESTARTS_WARN
            && waiter.last_terminated.as_ref().map(|r| r.exit_code) == Some(captured_code),
        "the ending has to be a *failure* and the container an init one, or the arm under test \
         is not the one that fires: {waiter:?}"
    );
    assert!(
        ![0, 143].contains(&captured_code),
        "an ending this rule reads as clean would take one of the other two arms, and the \
         capture's own {captured_code} is what puts this card on the failure arm"
    );

    let all = analyze(&pods_at(vec![failing], now()));
    show(&all);
    // Four, not three, and for [`init_run_again`]'s reason: the plant runs the container again,
    // which is a restart the kubelet counts.
    let card = only(&all, "healthy-retry", "restarted 4 times");
    // **The claim is untouched by the split** — a non-zero exit beside a count carries it
    // (NOTES § D85's asymmetry), and only the advice underneath was ever role-blind. The title
    // says the same thing to every role, which is what *the split is under the title* means; the
    // clause on the end of it is the ending this rule read and is role-blind too (NOTES § D102).
    assert_eq!(
        card.title, "Container has been restarted 4 times, but something keeps killing it",
        "the split is under the title, not in it"
    );
    assert_eq!(
        card.severity,
        Severity::Warn,
        "and the band does not move with the role either — three restarts is amber whoever is \
         counting them"
    );
    // **The positive half is the shared sentence, since 2026-08-16** (NOTES § D113). It was
    // *check the memory limit and the liveness probe* — the half that survived the role split —
    // and rule 6's card beside it on the same `exit 1` said *read that run's log*, which is one
    // ending answered two ways. All three rules take [`failed_run_action`] whole now, so the
    // role split lives where the role actually decides something (`137`, [`killed_action`]) and
    // this arm has nothing left to split.
    assert_eq!(
        card.action,
        failed_run_action(&exited_run(1), ContainerRole::Init).0,
        "the arm answers with the sentence every rule gives this ending: {}",
        card.action
    );
    for probe in ["liveness", "readiness", "startup"] {
        assert!(
            !card.action.contains(probe),
            "and the half that is not goes — `validateInitContainers` rejects a {probe} probe \
             on this kind of container, so this rule would be sending the reader after a thing \
             its own next arm says cannot exist: {}",
            card.action
        );
    }
    // **What replaced the probe, asserted to exist.** `memory limit` is in both arms' sentences,
    // so with only the negatives above, deleting the second half of this one is invisible — and
    // an init container whose limit is fine would be left with a card that names nothing else to
    // check.
    //
    // **This pin and the loop below replaced two that pinned tokens of a recital** — *"with a
    // memory reason beside it"* and *"would not stop when it was asked to"* — which is D88's own
    // lesson arriving late on the pins that shipped with it. Both are gone with the sentence
    // they quoted.
    //
    // **The requirement they now hold, in two halves:** the reader is sent to the memory limit,
    // and the absence of the kernel's word is never allowed to mean anything. Neither direction
    // of `137` is decidable from the object — it is memory only with `reason: OOMKilled` beside
    // it (NOTES § D71), *and* a real cgroup kill arrives as plain `Error` on a host that is
    // itself short of memory (NOTES § D84). A card that branches picks the wrong branch exactly
    // when the node is the cause and rule 2 is silent, which is when the reader needs it most.
    //
    // **The two pins are not equally strong, and saying otherwise is how the earlier holes in
    // this box survived their reviews.** The negative loop underneath is requirement-shaped: it
    // rejects a recital however it is worded, so a rewrite passes it on merit. **This positive
    // one is still a token pin** — *"without recording why"* would satisfy the requirement word
    // for word and turn it red. It fails *closed*, which is the safe direction and why it stays,
    // but a faithful rewrite of this sentence must expect to edit this line, and editing it is a
    // decision to re-read the requirement above rather than a formality.
    // **The clause moved to the arm it is about, and this row moved with it** (NOTES § D113).
    // The kernel's word being possibly missing is a claim about `137`; on the capture's own code
    // it was standing on an arm with no memory question in it at all. All three rules answer the
    // ordinary failure with one sentence now, and the `137` reading stayed where it always was —
    // in [`killed_action`], which is the arm this container reaches when the code is one.
    let killed = init_run_again(137);
    let killed_card = only(
        &analyze(&pods_at(vec![killed], now())),
        "healthy-retry",
        "restarted",
    )
    .clone();
    println!("{}", killed_card.action);
    assert!(
        killed_card.action.contains("without saying so")
            || killed_card.action.contains("not always labelled"),
        "the card has to say the kernel's word may be missing — an action that reads anything \
         into its absence rules memory out on the one shape where memory is likeliest, and \
         `OOMKilled` is what rule 2 keys on, so nothing else on the screen says it either: {}",
        killed_card.action
    );
    for probe in PROBE_WORDS {
        assert!(
            !killed_card.action.to_lowercase().contains(probe),
            "and it still names no {probe} on a container `validateInitContainers` allows none: \
             {}",
            killed_card.action
        );
    }
    for decided in ["137", "exit code", "any other code"] {
        assert!(
            !card.action.contains(decided),
            "and it may not recite what *{decided}* might have meant: the reader's own code is \
             translated one line above by `exit_meaning`, so a branch here is four wrapped lines \
             about numbers they cannot see, ending in a restatement of the one they can: {}",
            card.action
        );
    }
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl logs healthy-retry -c wait-for-db -n default --previous"),
        "the command follows the action, and this arm's action names that run's log — `describe` \
         prints no logs at all (invariant 4, NOTES § D113)"
    );
}

/// **A container something keeps stopping, on the two roles that read the stop differently** —
/// rule 1 splits `Init` out here and tells it the opposite of what it tells the other two, and
/// two rules reading one container and disagreeing is where NOTES § D85 starts. Both rules now
/// read the same [`stopped_action`], so there is one reading of `143` in this file and not two.
///
/// **`healthy-retry.json` is the init half with two fields moved and both real**:
/// `wait-for-db` is a plain init container that failed three times before it succeeded, so the
/// count is the capture's. A container the kubelet keeps stopping sits `Terminated` with the
/// same code its `lastState` holds, between restarts under the pod's `restartPolicy: Always` —
/// and its current state has to move too, or the successful run exempts it from this rule
/// altogether ([`doing_its_job`], NOTES § D75).
#[test]
fn a_stopped_container_reads_the_same_on_this_rule_as_it_does_on_rule_one() {
    let sidecar = capture_but("healthy-sidecar", |p| {
        // [`ended_as`] rather than [`exited`], because this capture no longer carries a previous
        // run of its own to rewrite — `proxy` is a `sleep 3600` and the 2026-08-16 trip captured
        // inside the first hour (NOTES § D114). The run first, then the band, so the count the
        // card is drawn from is the one this test names rather than the one the plant added.
        ended_as(p, "proxy", 143, None, None);
        container_status(p, "proxy").restart_count = RESTARTS_WARN;
    });
    // Still serving, so still read inside its run (NOTES § D100) — unlike the init container
    // below, which is stopped and whose card therefore never ages out.
    let all = serving_findings(sidecar, "proxy");
    let card = only(&all, "healthy-sidecar", "restarted 3 times");
    assert!(
        card.action.contains("probes"),
        "a native sidecar may carry all three probes, so it is on the same side of the split \
         as a plain container — rule 1 divides them the same way: {}",
        card.action
    );

    let stopped = capture_but("healthy-retry", |p| {
        exited(p, "wait-for-db", 143);
        let status = container_status(p, "wait-for-db");
        // `ready` moves with the state, because the kubelet writes them together: an init
        // container's `ready` is true only where it completed successfully, so leaving the
        // capture's `true` beside a `143` builds an object no API server emits — the objection
        // [`exited`]'s own comment makes about `reason`, one field over (NOTES § D40, § D53).
        status.ready = false;
        let run = status
            .state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("the capture's init container has finished");
        run.exit_code = 143;
        run.reason = Some("Error".to_string());
    });
    let migrate = container(&stopped, "wait-for-db");
    println!("{migrate:?}");
    assert!(
        migrate.role == ContainerRole::Init
            && !doing_its_job(migrate)
            && migrate.restarts >= RESTARTS_WARN,
        "a restartable init container may have probes, so the arm under test would not be the \
         one that fires — and a successful one is not this rule's subject at all: {migrate:?}"
    );

    let all = analyze(&pods_at(vec![stopped], now()));
    show(&all);
    let card = only(&all, "healthy-retry", "restarted 3 times");
    for probe in ["liveness", "readiness", "startup"] {
        assert!(
            !card.action.contains(probe),
            "Kubernetes rejects a {probe} probe on this kind of container, so naming one is \
             advice the reader cannot follow — and rule 1 tells the same container the \
             opposite, which is the disagreement this box exists to stop: {}",
            card.action
        );
    }
    // **`earlyoom` alone since 2026-08-16.** `systemd-oomd` was named beside it and cannot reach
    // this card at all: it kills a whole cgroup with `cgroup.kill`, which is SIGKILL and arrives
    // as `137`, so on a card about `143` it sent the reader grepping for a tool that could never
    // be there (NOTES § D113).
    assert!(
        card.action.contains("earlyoom") && !card.action.contains("systemd-oomd"),
        "and the reader is left with somewhere real to look — a userspace memory killer that \
         sends SIGTERM, and only one of the two does: {}",
        card.action
    );
    // **The two things only this arm says.** The memory killer above is in the sibling arm's
    // sentence too, so it and the probe negatives together are satisfied by wording that has
    // lost everything this branch exists for. These two are not: the reason no probe is named,
    // and the reading a signal number cannot rule out — a program may raise SIGTERM on itself,
    // which is what the first round of this box was blocked for asserting away.
    assert!(
        card.action.contains("does not allow health checks"),
        "the reader is told *why* no probe is named here, or the branch is indistinguishable \
         from one that simply forgot to mention them: {}",
        card.action
    );
    // **The door survived the 2026-08-16 shortening and its wording did not** (NOTES § D113):
    // *check whether the program exits 143 of its own accord* was one of the 36 characters that
    // arm had to lose, and what replaced it says the same thing in fewer — the requirement is that
    // the program stays on the list, not that one phrasing does.
    assert!(
        card.action.contains("from the program itself") && card.action.contains("exits 143 itself"),
        "and 143 leaves the program itself on the list — an action that names only outside \
         causes asserts an agent the exit code cannot carry: {}",
        card.action
    );
}

/// **An ending on a container that is *not* serving** — the half of the split the tests above
/// cannot reach, and the band is the one thing that must not move (NOTES § D71): a container that
/// keeps finishing early and is not ready now is as down as one that keeps crashing.
///
/// **This title carries the ending too, since 2026-08-16** (NOTES § D102) — it read the count and
/// nothing else until then, which is the branch every [`one_card_per_action`] fold leaves standing.
#[test]
fn a_container_that_is_down_keeps_its_band_whatever_the_last_run_did() {
    // **`earlyoom` and not `systemd-oomd`**, which came out of both `stopped_action` arms on
    // 2026-08-16: it kills a cgroup with SIGKILL, so it can only ever produce `137` and never the
    // `143` this row is about (NOTES § D113).
    for (exit_code, said, said_ending) in [
        (0, "Job", "finished cleanly"),
        (143, "earlyoom", "was stopped"),
    ] {
        let plant = restarts10_ending("restarts10", exit_code);
        let c = container(&plant, "flaky");
        println!("{c:?}");
        assert!(
            !doing_its_job(c) && c.restarts >= RESTARTS_CRITICAL,
            "the plant has to stay past the red band and not serving: {c:?}"
        );

        let all = analyze(&pods_at(vec![plant], now()));
        show(&all);
        assert!(
            !titles(&all).iter().any(|t| t.contains("on record failed")),
            "rule 6 exempts both of these codes, so it drops out where the committed capture \
             below draws it — rule 7 stays either way, this container is up and not ready: {:?}",
            titles(&all)
        );
        let card = only(&all, "broken-restarts10", "restarted 10 times");
        assert_eq!(
            card.severity,
            Severity::Critical,
            "a container that keeps finishing early and is not ready now is as down as one \
             that keeps crashing — the band reads whether it is serving, and how the run ended \
             does not move it"
        );
        // **The whole title, not a phrase out of it**, so a clause that grew or went is read
        // here rather than inferred from a `contains` that survives both.
        //
        // **The clause is on this title too, and it was the serving branch's alone until
        // 2026-08-16** (NOTES § D102). It is the *non*-serving card that [`one_card_per_action`]
        // leaves standing — rule 6 leaves on [`doing_its_job`], so the pair only ever collapses
        // here — and without the clause every fold took the diagnosis off the title line, which
        // `screens/alerts.md` § The height never cuts, and left it to the evidence line, which is
        // the one line it does. The height that argument turns on is measured in
        // [`the_cards_this_box_ships_fit_the_height_they_are_drawn_at`] and not asserted here.
        assert_eq!(
            card.title,
            format!(
                "Container has been restarted 10 times, and the last run on record {said_ending}"
            ),
            "the count and what the record says ended the run — the two facts this rule read, \
             both on the line the pane may not cut"
        );
        assert!(
            !card.action.contains("memory limit") && card.action.contains(said),
            "the action is the one the ending decides, not the one the state does: {}",
            card.action
        );
    }

    // The control: the committed capture, unmoved, where both the count and the failure are
    // real and the sentence is still the right one.
    //
    // **Rule 6's card folds into this one since 2026-08-16** (NOTES § D113) — same sentence, same
    // command, and rule 5 now carries the duration that was the last fact keeping them apart. So
    // what the control asserts is that the *ending* still reaches the card, which is the exit
    // code on the evidence line, rather than that a second card carries it.
    let both = findings(&["restarts10"]);
    show(&both);
    assert!(
        only(&both, "broken-restarts10", "restarted 10 times")
            .evidence
            .contains("exit 1 (the application's own error)"),
        "the ending reaches the reader on the surviving card — an `exit 1` this rule may still \
         call a kill, on the card the plants above changed: {:?}",
        titles(&both)
    );
    assert_eq!(
        only(&both, "broken-restarts10", "restarted 10 times").action,
        failed_run_action(&exited_run(1), ContainerRole::Regular).0,
        "and the arm answers with the sentence rule 6's card beside it gives the same run — it \
         said *check the memory limit and the liveness probe* over an `exit 1` its own evidence \
         line calls *the application's own error* (NOTES § D113)"
    );
}

/// The captured Pending pod, edited — rule 10's shapes all start here.
fn pending_but(edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    capture_but("pending", edit)
}

/// One entry of a captured pod's condition array, by type, to be written through.
fn pod_condition<'a>(pod: &'a mut Pod, type_: &str) -> &'a mut PodCondition {
    pod.status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition to edit"))
}

/// The `PodScheduled` entry, which is the one every rule-10 shape moves.
fn scheduled_condition(pod: &mut Pod) -> &mut PodCondition {
    pod_condition(pod, "PodScheduled")
}

/// One entry of a captured pod's status arrays, by name — init containers and regular
/// ones searched together, the way [`container_snapshots`] reads them.
fn container_status<'a>(pod: &'a mut Pod, name: &str) -> &'a mut ContainerStatus {
    let status = pod.status.as_mut().expect("the capture has a status");
    status
        .init_container_statuses
        .iter_mut()
        .chain(status.container_statuses.iter_mut())
        .flatten()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("the capture reports on no container {name}"))
}

/// A container status rewritten to *waiting*, with the kubelet's reason and sentence —
/// the shape rule 13's positives are built out of.
fn waiting_at(reason: &str, message: Option<&str>) -> Option<ApiContainerState> {
    Some(ApiContainerState {
        waiting: Some(ContainerStateWaiting {
            reason: Some(reason.to_string()),
            message: message.map(str::to_string),
        }),
        ..ApiContainerState::default()
    })
}

/// **One named container rewritten to a container that has never run** — waiting on the
/// kubelet's reason, with the previous run, the restart count, and the two flags that say it is
/// up cleared beside it.
///
/// The five fields are one coherent group and are moved together (NOTES § D40):
/// `lastState` is precisely what tells [`stuck_at_the_starting_line`] that a container has
/// run before, so a plant that moved `state` alone would be a container that has both never
/// started and a previous run — a shape no kubelet writes, and one the rule under test is
/// right to be silent on. The 2026-08-13 capture is what made that visible: the trip ran long
/// enough for `hostpath.json`'s and `healthy-sidecar.json`'s containers to finish their
/// `sleep 3600` and be restarted, so the bases these plants sit on grew a `lastState` the
/// first capture's did not have.
///
/// **`ready` and `started` joined the group on 2026-08-14** (NOTES § D88): the kubelet writes
/// both `false` for a container it has not started, and every base these plants sit on is a
/// capture of a container that was up. No rule reads either on a waiting container —
/// [`running_but_not_ready`] leaves on the state before it reaches them — so nothing moves
/// today; the group is what the next reader will believe.
///
/// **The count of `0` beside no `lastState` is right for a container that *was* running**, which
/// is the reading it was queried on: `convertToAPIContainerStatuses` reaches this waiting shape
/// only when the runtime no longer holds the container — a `crictl rmp` or a runtime restart
/// takes the sandbox and its containers together — and the branch it takes then copies
/// `RestartCount` and `LastTerminationState` off the *previous API status*, not off its `State`.
/// A container that was `Running` carried an empty `LastTerminationState` and a count of `0`, so
/// both survive as they are. Had the kubelet published a terminated status first, the whole old
/// status is kept and the container would not be waiting at all — so this shape is the only one
/// consistent with `PodInitializing` on a container that had been up.
fn never_ran(pod: &mut Pod, name: &str, reason: &str, message: Option<&str>) {
    let status = container_status(pod, name);
    status.state = waiting_at(reason, message);
    status.last_state = None;
    status.restart_count = 0;
    status.ready = false;
    status.started = Some(false);
}

/// **A captured pod put back to where a rebuilt sandbox leaves the whole object** — `phase:
/// Pending` with `Initialized`, `Ready` and `ContainersReady` all `False`, which is what the
/// kubelet writes while it runs the init containers again.
///
/// **`PodReadyToStartContainers` stays `True` on purpose**: by the time an init container is
/// running or backing off, the sandbox it needed has been rebuilt and is there.
///
/// The two plants that stage that rebuild share it rather than each choosing how far to go
/// (NOTES § D40, § D88). Nothing in `rules.rs` reads `Initialized`; `phase` and the pod's `Ready`
/// condition are read by rules that are silent here either way — [`no_node_accepted_it`] and
/// [`nothing_has_looked_at_it`] both leave on a pod whose `PodScheduled` is `True`, and
/// [`running_but_not_ready`] on a container that is not `Running`. A plant is worth the shape it
/// builds, and "no assertion moves" is not the same claim as "a cluster writes this".
fn sandbox_rebuilt(pod: &mut Pod) {
    pod.status.as_mut().expect("the capture has a status").phase = Some("Pending".to_string());
    for type_ in ["Initialized", "Ready", "ContainersReady"] {
        pod_condition(pod, type_).status = "False".to_string();
    }
}

/// A captured container moved into the state the kubelet writes while it is waiting to restart
/// it — **`lastState` kept**, because that is the field rule 1 reads to tell the three loops
/// apart (NOTES § D85), and a plant that dropped it would take the *no record* branch and prove
/// nothing about any of the endings.
fn backing_off(pod: &mut Pod, name: &str) {
    let status = container_status(pod, name);
    status.state = waiting_at("CrashLoopBackOff", None);
    status.ready = false;
    status.started = Some(false);
}

/// The exit code of a captured container's previous run, rewritten — the one field
/// [`ending`] branches on, for the endings no capture holds on that role.
///
/// **The reason moves with the code, because the API emits them as a pair**: the kubelet writes
/// `Completed` beside `0` and `Error` beside everything else, which is what `exit0.json` and
/// `sigterm.json` show. A plant that moved the code alone would build
/// `exitCode: 0, reason: Error` — an object no API server produces, and a plant is only worth
/// what its shape is (NOTES § D40, § D53).
fn exited(pod: &mut Pod, name: &str, code: i32) {
    let run = container_status(pod, name)
        .last_state
        .as_mut()
        .expect("the capture's container has run before")
        .terminated
        .as_mut()
        .expect("and it exited rather than being killed while waiting");
    run.exit_code = code;
    run.reason = Some(if code == 0 { "Completed" } else { "Error" }.to_string());
}

/// **Rule 10, and the fixture that would break a rule shaped like its neighbours.**
/// `broken-pending` has no `containerStatuses` at all — the kubelet never saw it — so
/// every rule in this file that loops over containers is structurally silent on it, and
/// the one rule that is *about* it has to read the pod.
///
/// The scheduler's own sentence is the card, verbatim (NOTES § D27, § D37): it is the
/// answer to the question a beginner asks most often, and no paraphrase of it can name
/// which nodes refused and for what.
#[test]
fn the_pending_pod_carries_the_schedulers_verdict_and_the_sentence_behind_it() {
    let raw = fixture("pending");
    let all = findings(&["pending"]);
    show(&all);
    assert_eq!(all.len(), 1, "rule 10 alone: {:?}", titles(&all));

    assert!(
        pod("pending").containers.is_empty(),
        "a capture whose kubelet had reported on a container would let a \
         container-shaped rule pass this test by accident, and rule 10's whole subject \
         is the pod no kubelet has seen"
    );

    let unplaced = only(&all, "broken-pending", "will take this pod");
    assert_eq!(
        unplaced.severity,
        Severity::Critical,
        "this capture is three hours past its refusal, which is well outside the window \
         below — nothing is going to place it until a human acts"
    );
    assert!(
        unplaced.title.contains("No machine in the cluster") && unplaced.title.contains("Pending"),
        "invariant 14: the sentence explains what happened, and then names the word the \
         reader is staring at in `kubectl get pods`: {}",
        unplaced.title
    );

    // **Equality against the capture's own bytes.** D37 is the whole rule here: the
    // scheduler counts the nodes and says what each one refused the pod for, and a
    // finding that summarised, truncated or re-punctuated that has thrown away the only
    // thing it had to offer. `contains` would pass on a card that appended to it.
    let sentence = captured_str(captured_condition(&raw, "PodScheduled"), &["message"]);
    assert_eq!(
        unplaced.evidence,
        format!("the scheduler's own words (a node is one machine): {sentence}"),
        "quoted whole, and framed so a newcomer reads it as a quote rather than as \
         k8rs's own prose — and the four-word gloss is the only thing on this card \
         joining the title's *machine* to the quote's four *node*s (invariant 14)"
    );
    assert!(
        unplaced.evidence.contains("nodes are available"),
        "and the sentence still counts the machines that refused it — a capture whose \
         message no longer does is not this rule's fixture: {}",
        unplaced.evidence
    );

    // **The condition's own transition, which is the *first* refusal.**
    // `UpdatePodCondition` carries the old stamp forward while the status has not
    // changed, and the scheduler rewrites this condition on every retry with the same
    // `False` — so this dates the moment the pod became unplaceable, not the last
    // attempt at it.
    assert_eq!(
        unplaced.timestamp,
        Some(captured_time(
            captured_condition(&raw, "PodScheduled"),
            &["lastTransitionTime"]
        ))
    );
    assert_eq!(
        unplaced.age(&now()).as_deref(),
        Some("14 hours ago"),
        "a duration, not English parsed back into a number"
    );

    // `describe` prints conditions as a Type/Status table with no reason and no
    // message. It does print Events, and the scheduler re-emits `FailedScheduling` on
    // every retry, so the sentence usually *is* reachable there — but an Event expires
    // at `--event-ttl` and a field does not, which is the narrower form of rules 3 and
    // 4's argument (invariant 4). `-o yaml` also shows `spec.affinity`, which this
    // capture's own message blames and `describe` never prints.
    assert_eq!(
        unplaced.kubectl_cmd.as_deref(),
        Some("kubectl get pod broken-pending -n default -o yaml")
    );
    assert!(
        !unplaced.action.contains("the machines have"),
        "the action may only ask for what the command beside it can answer: the node \
         side of that comparison is `kubectl get nodes --show-labels`, and it is N6's \
         to make: {}",
        unplaced.action
    );
}

/// **Rule 10's severity ladder, both sides of it.** A flat CRITICAL rested on *"a pod
/// that places normally never carries this"*, and three routine paths falsify it — an
/// autoscaler scale-up (where this condition is the *trigger*), `Immediate`-mode volume
/// provisioning on a fresh StatefulSet replica, and node-group rollover. None needs a
/// human, and CRITICAL in this file means *this will not run until someone acts*.
///
/// The card is immediate either way — the scheduler's sentence is the good half and it
/// does not wait. Only the colour does.
#[test]
fn a_refusal_the_cluster_may_still_fix_itself_is_amber_until_it_has_had_ten_minutes() {
    // **The two moments are the capture's own refusal plus ten minutes, and plus ten
    // minutes and a second.** The offset is written out rather than read from
    // [`NOT_READY_GRACE`] — a boundary computed from the constant under test agrees with
    // every value it could hold — while the moment it is added to comes off the capture,
    // so a recapture moves them together.
    let refused = pod("pending")
        .scheduled
        .as_ref()
        .and_then(|c| c.last_transition.clone())
        .expect("the captured refusal says when the scheduler first gave up");
    let after = |secs: i64| {
        Time(
            refused
                .0
                .checked_add(SignedDuration::from_mins(10) + SignedDuration::from_secs(secs))
                .expect("ten minutes after a captured refusal is a moment"),
        )
    };
    // Exactly ten minutes on, which is an autoscaler that has not finished bringing a
    // node up.
    let early = findings_at(&["pending"], after(0));
    show(&early);
    assert_eq!(
        early.len(),
        1,
        "the card is immediate — a beginner gets the scheduler's sentence at once, and \
         only the band waits: {:?}",
        titles(&early)
    );
    assert_eq!(
        early[0].severity,
        Severity::Warn,
        "ten minutes unplaced is a scale-up in progress, not an outage — and rule 13 in \
         this same phase takes WARN plus this same window for one healthy look-alike, \
         where this rule has three"
    );

    // One second past it.
    let late = findings_at(&["pending"], after(1));
    assert_eq!(
        late[0].severity,
        Severity::Critical,
        "past `progressDeadlineSeconds`' own default is where Kubernetes itself stops \
         calling a rollout healthy, and it is the window rules 7 and 13 borrow — not a \
         number picked for this rule"
    );

    // **No stamp is not read as recent.** A pod that cannot be shown to have just
    // become unplaceable is read as one that has been that way, which is the safe
    // direction — and it is the shape a Kueue-gated pod arrives in from the other
    // side, carrying a *gating* stamp older than its own unschedulability.
    let stampless = pending_but(|p| {
        scheduled_condition(p).last_transition_time = None;
    });
    let one_second_on = Time(
        refused
            .0
            .checked_add(SignedDuration::from_secs(1))
            .expect("a second after a captured refusal is a moment"),
    );
    let all = analyze(&pods_at(vec![stampless], one_second_on));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "a missing stamp costs the age, never the card — rule 7 is the rule that has no \
         finding without a since-when, and this one stands on the verdict alone: {:?}",
        titles(&all)
    );
    assert_eq!(all[0].timestamp, None, "and the right edge is blank");
    assert_eq!(
        all[0].severity,
        Severity::Critical,
        "one second after the capture's own refusal, which would be WARN with a stamp — \
         so this is the absence deciding, not the clock"
    );
}

/// **Rule 10's negatives, and the two that matter are Pending for a different reason.**
/// `Pending` is the phase of a pod waiting on an image pull and of one waiting on a
/// ConfigMap, and rules 3 and 4 already explain both — a rule 10 that read the phase,
/// or that read the condition's presence rather than its value, would put a second and
/// wrong card on each of them.
#[test]
fn a_pod_pending_for_a_reason_that_is_not_the_scheduler_gets_no_rule_ten_card() {
    // The negatives are asserted to be in the shape that could trip the rule, before
    // their silence is worth anything: both really are `Pending`, and both really do
    // carry the condition rule 10 reads.
    for name in ["image", "config"] {
        let p = pod(name);
        let scheduled = p
            .scheduled
            .as_ref()
            .expect("the condition does not go away once a node accepts the pod");
        println!(
            "{}: phase={:?} PodScheduled={} reason={:?}",
            p.id.name, p.phase, scheduled.status, scheduled.reason
        );
        assert_eq!(
            p.phase.as_deref(),
            Some("Pending"),
            "a pod whose image will not pull is Pending, and this is the capture that \
             makes 'Pending' the wrong thing for rule 10 to read"
        );
        assert_eq!(
            scheduled.status, "True",
            "a node did accept it — what is stuck is what happened afterwards"
        );
    }

    let all = findings(&["image", "config"]);
    show(&all);
    assert_eq!(
        all.iter()
            .filter(|f| f.title.contains("will take this pod"))
            .count(),
        0,
        "rules 3 and 4 own these two pods, and a second card saying no machine would \
         have them is both wrong and the loudest thing on the screen: {:?}",
        titles(&all)
    );
    assert_eq!(
        all.len(),
        2,
        "one card each, exactly as before rule 10 existed: {:?}",
        titles(&all)
    );

    // The healthy pod is the other half: `PodScheduled` is `True` there with no reason
    // at all, so a rule testing `scheduled.is_some()` would fire on every working pod
    // in the cluster.
    nothing(
        &findings(&["healthy"]),
        "a scheduled pod keeps the condition rather than dropping it, so presence is \
         not what this rule may test",
    );
}

/// **The half of rule 10's gate no capture can reach.** Every fixture carrying
/// `reason: Unschedulable` also carries `status: "False"`, and every fixture at
/// `status: "True"` carries no reason at all — so a rule that dropped the status check
/// and read the reason alone leaves this suite green, and only this test says
/// otherwise. Measured, not assumed: the gate was mutated to the reason alone and all
/// 66 tests passed.
///
/// The two are separate strings on `status.conditions`, which is a subresource anyone
/// with `patch pods/status` may write — a stale or planted `reason` beside a `True`
/// status is not something the scheduler produces, and it is exactly what invariant 9's
/// "free text from the API is untrusted" means one level up from a string: a *field
/// combination* the object model permits and the controller never emits. The card it
/// would draw is the worst one available here, `No machine in the cluster will take
/// this pod` over a pod that is running and serving.
///
/// One field, on a real captured object, exactly as the `subPathExpr` and DaemonSet
/// tests do it — the capture is not edited (NOTES § D53).
#[test]
fn a_scheduled_pod_carrying_the_unschedulable_reason_anyway_is_not_a_finding() {
    let mut object: Pod =
        serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
    let condition = object
        .status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == "PodScheduled")
        .expect("the captured healthy pod keeps its PodScheduled condition");
    condition.reason = Some("Unschedulable".to_string());
    assert_eq!(
        condition.status, "True",
        "and the status is left as the cluster wrote it — the whole point is the pair"
    );

    let p = PodSnapshot::from(object);
    println!("{:?}", p.scheduled);
    nothing(
        &analyze(&pods_at(vec![p], now())),
        "a pod a node accepted is running, whatever reason string is sitting beside \
         that condition — **and neither half of this gate is redundant**: the reason \
         half is what excludes a gated pod, asserted in \
         `a_pod_the_scheduler_never_judged_is_not_a_pod_it_refused`",
    );
}

/// **The two `PodScheduled: False` reasons that are not a refusal**, and the test that
/// holds the reason half of rule 10's gate in place. Cutting `reason` out of the gate
/// leaves every other test in this file green.
///
/// `SchedulingGated` is a pod its author asked to be held back — `spec.schedulingGates`,
/// which is how Kueue, Volcano and Yunikorn queue work — so a CRITICAL on it is k8rs
/// contradicting a decision the user made, once per queued pod, on a cluster whose whole
/// point is that the queue is long. `SchedulerError` is an internal failure the
/// scheduler retries by itself.
///
/// Both are synthesized from the real refusal rather than captured, because three lines
/// on a committed object is not a capture trip — the shape is one field of one string.
#[test]
fn a_pod_the_scheduler_never_judged_is_not_a_pod_it_refused() {
    for reason in ["SchedulingGated", "SchedulerError"] {
        let p = pending_but(|pod| {
            scheduled_condition(pod).reason = Some(reason.to_string());
        });
        let scheduled = p.scheduled.as_ref().expect("the condition is still there");
        println!(
            "{}: PodScheduled={} reason={:?}",
            p.id.name, scheduled.status, scheduled.reason
        );
        assert_eq!(
            scheduled.status, "False",
            "the status is left exactly as the scheduler wrote it — if these were \
             `True` the status half of the gate would be excluding them and this test \
             would prove nothing about the reason half"
        );
        nothing(
            &analyze(&pods_at(vec![p], now())),
            &format!(
                "`{reason}` is not `Unschedulable`: nothing has refused this pod, so \
                 there is no verdict to report and no scheduler sentence to quote"
            ),
        );
    }
}

/// **The unscheduled pod somebody deleted — rule 10 hands it to rule 12 and says
/// nothing.** Both cards would be *true* on it: it is unplaceable, and it is not going
/// away. Rule 10's action is what disqualifies it — *check what this pod asks for* sends
/// the reader to audit `nodeSelector`, affinity and requests when the only move left is
/// finding what is holding the delete, which is rule 12's card and rule 12 names the
/// finalizer. Alerts is D2's queue of what is broken now **and actionable**, and where
/// a pod could have run stops being actionable once someone has asked for it to go.
///
/// **It also removes the two-word problem rather than managing it.** `printPod`
/// overrides the STATUS column to `Terminating` whenever `deletionTimestamp` is set and
/// the phase is not terminal, while `phase` itself stays `Pending` — which is why
/// `stuck.json` is `phase: Running` and still shows as Terminating. So this pod would
/// have drawn rule 10 saying *"it shows as Pending"* beside rule 12 saying
/// *"it shows as Terminating"*, about one pod, on one screen. The card that had the
/// wrong word is the card that had no business being there.
///
/// This test asserted that pair agreeing until 2026-08-13; it now asserts rule 10 is
/// absent, and it is the red run for the `deletion_timestamp` guard.
#[test]
fn the_deleted_pod_is_rule_twelves_alone_and_rule_ten_stands_down() {
    let deleted = pending_but(|pod| {
        pod.metadata.deletion_timestamp = Some(time("2026-08-12T20:46:23Z"));
        pod.metadata.deletion_grace_period_seconds = Some(30);
        pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
    });
    assert_eq!(
        deleted.scheduled.as_ref().and_then(|c| c.reason.as_deref()),
        Some("Unschedulable"),
        "the trigger is untouched — this pod still satisfies rule 10's gate, which is \
         what makes the silence below the deletion's doing"
    );
    assert_eq!(
        deleted.phase.as_deref(),
        Some("Pending"),
        "and the phase does not move when a pod is deleted, which is why the \
         parenthetical's `phase` check could never have closed this on its own"
    );

    let all = analyze(&pods_at(vec![deleted], now()));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "rule 12 alone: a pod on its way out is rule 12's, and rule 10's action points \
         the reader at the wrong half of the object: {:?}",
        titles(&all)
    );
    let terminating = only(&all, "broken-pending", "asked to shut down");
    assert!(
        terminating.title.contains("Terminating"),
        "and the one card left names the word `kubectl get pods` actually prints: {}",
        terminating.title
    );
    assert!(
        terminating.evidence.contains("k8rs.test/never-removed"),
        "with the finalizer, which is the only thing anyone can act on here: {}",
        terminating.evidence
    );

    // **The minute before rule 12's margin opens draws nothing at all**, and that is
    // correct rather than a hole: for that minute the pod is deleting normally, and
    // neither rule has anything to say about a delete that was accepted seconds ago.
    nothing(
        &analyze(&pods_at(
            vec![pending_but(|pod| {
                pod.metadata.deletion_timestamp = Some(time("2026-08-12T20:46:23Z"));
                pod.metadata.deletion_grace_period_seconds = Some(30);
            })],
            time("2026-08-12T20:47:00Z"),
        )),
        "inside rule 12's margin the delete is still in progress, and rule 10 has \
         already stood down — a deliberate gap, not an unhandled one",
    );
}

/// **The pod preemption has already found a machine for** — where rule 10's trigger is
/// true and its sentence is false, which is the one shape those two come apart in.
///
/// kube-scheduler writes `status.nominatedNodeName` in the *same* status patch that
/// sets `PodScheduled: False / Unschedulable`, and the pair stands for the whole
/// graceful termination of the victims — 30s by default, minutes with a real grace or a
/// `preStop` hook, unbounded when a victim will not go. Through all of it the card
/// would read *"no machine in the cluster will take this pod"* while the API says which
/// machine is being cleared for it.
#[test]
fn a_pod_with_a_machine_already_being_cleared_for_it_is_not_a_pod_nothing_will_take() {
    let nominated = pending_but(|pod| {
        pod.status
            .as_mut()
            .expect("the captured pod has a status")
            .nominated_node_name = Some("k8rs-worker2".to_string());
    });
    println!(
        "nominated={:?} scheduled={:?}",
        nominated.nominated_node_name, nominated.scheduled
    );
    assert_eq!(
        nominated
            .scheduled
            .as_ref()
            .and_then(|c| c.reason.as_deref()),
        Some("Unschedulable"),
        "the trigger is untouched — this pod satisfies every other condition of rule \
         10, which is what makes the silence below the nomination's doing"
    );

    nothing(
        &analyze(&pods_at(vec![nominated], now())),
        "a machine has been chosen and is being cleared, so 'no machine will take this \
         pod' is false — and *'a machine has been chosen, it is waiting for other pods \
         there to shut down'* is a new rule, not a branch of this one (invariant 13). \
         Rule 12 already covers the half that goes wrong, on the victim",
    );
}

// --- RULE 13, THE RESIDUAL ---
//
// The rule with no positive capture (NOTES § D72), so the order below is the reverse of
// every other rule's: the negatives are committed captures and the positives are
// decoded copies. That is not a weaker proof of the negatives — `image.json` and
// `config.json` are the two pods in the repository that match rule 13's gate in every
// respect *except* the residual clause, and they are real.

/// **The two captures rule 13 would fire on if it were not a residual**, and they are
/// the hardest negatives in the file because nothing about them is synthetic.
///
/// `image.json` and `config.json` are both `phase: Pending`, both `PodScheduled: True`,
/// both have a container that has never run, and both are **three hours** older than the
/// pinned `now` — so they clear the ten-minute grace with room to spare and satisfy
/// every clause of [`placed_but_never_started`] except the one that matters. Dropping
/// either exclusion — [`EXPLAINED_ELSEWHERE`] or [`UNUSABLE_IMAGE`] — puts a second card
/// on each: *"it has not been able to start"* beside *"the image is not usable"*, which
/// is the same incident said twice and is exactly the failure a residual rule risks.
#[test]
fn the_two_pods_that_look_like_a_wedge_are_already_explained_by_rules_three_and_four() {
    for (name, phrase) in [
        ("image", "image is not usable"),
        ("config", "ConfigMap or Secret that does not exist"),
    ] {
        let p = pod(name);
        let scheduled = p
            .scheduled
            .as_ref()
            .expect("the capture carries PodScheduled");
        let since = scheduled
            .last_transition
            .as_ref()
            .expect("and the moment it was placed");
        println!(
            "{name}: scheduled={} at {since:?}, {:?} before the pin; containers {:?}",
            scheduled.status,
            now().0.duration_since(since.0),
            p.containers
                .iter()
                .map(|c| (&c.name, &c.state, c.last_terminated.is_some()))
                .collect::<Vec<_>>(),
        );

        // The preconditions are asserted before the outcome, so a capture that had
        // quietly stopped matching rule 13's gate cannot pass this by producing one
        // finding for the wrong reason.
        assert_eq!(
            scheduled.status, "True",
            "{name} is on a machine — this is not rule 10's pod"
        );
        assert!(
            now().0.duration_since(since.0) > NOT_READY_GRACE,
            "{name} was placed {:?} before the pin, and a capture inside the grace \
             would make the silence below the *clock's* doing rather than the \
             residual's",
            now().0.duration_since(since.0)
        );
        assert!(
            p.containers.iter().all(|c| c.last_terminated.is_none()
                && matches!(c.state, ContainerState::Waiting { .. })),
            "and not one of its containers has ever run: {:?}",
            p.containers.iter().map(|c| &c.state).collect::<Vec<_>>()
        );

        let all = findings(&[name]);
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "one incident, one card: {name}.json is explained by the rule that owns its \
             waiting reason, and rule 13 is what is left *after* those rules — not a \
             twelfth opinion on the same pod: {:?}",
            titles(&all)
        );
        only(&all, &p.id.name, phrase);
    }
}

/// **A migration that is simply taking a long time**, which is the false-positive class
/// this whole rule is trying not to become.
///
/// Rules 1–6 read init containers now, so a *broken* one gets its own card. A long one
/// is a different thing entirely: a database migration or a large restore leaves every
/// regular container at `PodInitializing` for as long as it runs, and nothing is wrong.
/// Ten minutes is nothing for that work.
///
/// **Both halves are silent for the same reason, and it is not the waiting reason
/// itself.** [`WAITING_ON_A_SIBLING`] is uninformative on its own — the kubelet writes it
/// on every container of a pod that declares an init container, wedged or not, which is
/// what [`a_pod_that_only_ever_says_podinitializing_is_the_wedge_the_rule_was_added_for`]
/// is about. What silences these two is that there **is** something to point at: here a
/// running init container, and in the committed `init.json` an init container carrying
/// `CrashLoopBackOff`, which is rule 1's card. Two cards there, both about `migrate`, and
/// none about `app`.
#[test]
fn an_init_container_still_doing_its_work_is_not_a_pod_that_never_started() {
    // The capture, unedited: `migrate` is looping and `app` is behind it.
    let captured = findings(&["init"]);
    show(&captured);
    assert!(
        captured.iter().all(|f| f.evidence.contains("migrate")),
        "every card on this pod is about the init container that is failing — a card \
         naming `app` sends the reader to logs that are empty, because the app has not \
         run (D27): {:?}",
        titles(&captured)
    );

    // The same pod with the migration *running* instead of looping — twenty minutes
    // into work that legitimately takes an hour.
    let running = capture_but("init", |pod| {
        let migrate = container_status(pod, "migrate");
        migrate.state = Some(ApiContainerState {
            running: Some(ContainerStateRunning {
                started_at: Some(time("2026-08-12T23:40:00Z")),
            }),
            ..ApiContainerState::default()
        });
        // First attempt, and still on it: the crash loop's history goes with the loop,
        // or rules 5 and 6 answer this test instead of rule 13.
        migrate.restart_count = 0;
        migrate.last_state = None;
    });
    let migrate = container(&running, "migrate");
    let app = container(&running, "app");
    println!("migrate={:?}\napp={:?}", migrate.state, app.state);
    assert!(
        matches!(migrate.state, ContainerState::Running { .. }),
        "the edit has to land on a running init container, or the silence below is the \
         crash loop's doing and not this rule's: {migrate:?}"
    );
    assert!(
        matches!(&app.state, ContainerState::Waiting { reason, .. }
            if reason.as_deref() == Some(WAITING_ON_A_SIBLING)),
        "and the app container has to still be the one waiting its turn — that reason \
         is what this test is about: {app:?}"
    );

    nothing(
        &analyze(&pods_at(vec![running], now())),
        "the pod was placed three hours ago and its app container has never started, so \
         every other clause of rule 13 holds — and it is a migration doing its job. The \
         running init container is both what `PodInitializing` is pointing at and what \
         makes *it has not been able to start* false about this pod; firing here would \
         put a card on every slow migration in the cluster (D2)",
    );
}

/// **The wedge itself, in the two shapes a real kubelet produces** — the rule's only
/// positives. **The first is captured**: `broken-wedged` asks for a `configMap` volume that
/// does not exist, so the kubelet never gets as far as the sandbox and the pod sits at
/// `ContainerCreating` with `PodReadyToStartContainers: False` — D72's own proposed shape,
/// which the 2026-08-13 trip brought back and which was a decoded copy until then. The
/// second shape has no capture behind it: on this cluster nothing reaches the sandbox and
/// then stops, so `True` and the absent condition stay decoded copies.
///
/// **What the card has to say is decided by the order the kubelet does its work, not by
/// what this rule happens to return.** `kubelet.SyncPod` waits for volumes to attach and
/// mount *before* the runtime creates the sandbox, so:
///
/// - **storage or network missing → the condition is `False`.** A `configMap` volume
///   naming an object that does not exist — D72's own proposed capture shape — never
///   reaches the sandbox at all.
/// - **anything after that → the condition is `True`.** The sandbox exists, which is
///   itself proof the mounts succeeded, so a card blaming a disk here is contradicted by
///   the very field it is reading.
///
/// The first draft of this test asserted the opposite of both, because it asserted what
/// the implementation returned instead of what the requirement says — which is how the
/// inversion shipped past a green suite. Each half below therefore also asserts the
/// sentence it must **not** carry: a swap of the two branches has to fail here, and
/// "contains the right words" alone would survive it.
///
/// `config.json` is the base for the copies rather than a hand-written pod: it is already a
/// scheduled pod whose single container has never run, which is every clause of the gate
/// (D40, D53 — the committed JSON is untouched).
#[test]
fn the_wedged_pod_names_the_side_of_the_sandbox_the_kubelet_actually_stopped_on() {
    let wedged = |reason: &'static str, condition: Option<&'static str>| {
        capture_but("config", move |pod| {
            never_ran(pod, "app", reason, None);
            match condition {
                Some(status) => {
                    pod_condition(pod, "PodReadyToStartContainers").status = status.to_string();
                }
                None => pod
                    .status
                    .as_mut()
                    .and_then(|s| s.conditions.as_mut())
                    .expect("the capture has conditions")
                    .retain(|c| c.type_ != "PodReadyToStartContainers"),
            }
        })
    };
    let one_card = |p: PodSnapshot| {
        let name = p.id.name.clone();
        let all = analyze(&pods_at(vec![p], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "rule 13 alone: nothing else in the file reads a container waiting for a \
             reason no rule owns: {:?}",
            titles(&all)
        );
        only(&all, &name, "not been able to start").clone()
    };

    // --- BEFORE THE SANDBOX: the volume wedge, which is what `False` means ---
    // The captured one. Its preconditions are asserted off the JSON first: a capture whose
    // kubelet had got past the sandbox would satisfy the *other* branch and pass this half
    // by drawing the sentence it is supposed to prove absent.
    let raw = fixture("wedged");
    assert_eq!(
        captured_condition(&raw, "PodReadyToStartContainers")["status"],
        "False",
        "this capture is the before-the-sandbox side, and without that it is the other test"
    );
    assert_eq!(
        captured_str(
            captured_status(&raw, "containerStatuses", "app"),
            &["state", "waiting", "reason"]
        ),
        "ContainerCreating",
        "and the container is still waiting on the kubelet, not on a registry"
    );
    let card = one_card(pod("wedged"));
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the one healthy thing that still looks exactly like this is a slow pull, and a \
         red card that is sometimes a slow pull is how red stops meaning broken (D2)"
    );
    assert!(
        card.evidence.contains("container app") && card.evidence.contains("ContainerCreating"),
        "the card names the container and quotes the machine's own word for where it \
         stopped — the reasons a kubelet can be stuck on are an open set, so the word is \
         passed through rather than translated: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("storage"),
        "`False` is written before the sandbox exists, and volumes are attached before \
         the sandbox too — so this is the missing-ConfigMap-volume pod, and a card that \
         names only the network sends its reader to the CNI over a storage fault: {}",
        card.evidence
    );
    assert!(
        !card.evidence.contains("the block is later"),
        "and it must not claim the pod already has what it is waiting for — this is the \
         half of the inversion that told a reader their disks were fine: {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_condition(&raw, "PodScheduled"),
            &["lastTransitionTime"]
        )),
        "the since-when is the moment the scheduler placed it, which is when the machine \
         became responsible for starting it"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-wedged -n default"),
        "and the command is `describe` and not `-o yaml`, unlike every other card whose \
         evidence is a field: what finishes this diagnosis is a `FailedMount` Event, \
         which only `describe` prints"
    );

    // --- AFTER THE SANDBOX: `True` is proof the mounts already succeeded ---
    for (label, p) in [
        ("still pulling", wedged("ContainerCreating", Some("True"))),
        (
            "the container could not be created",
            wedged("CreateContainerError", Some("True")),
        ),
        // Absent is not a third case: an old server or a kubelet that has said nothing
        // is read as "not False", the only claim that survives both.
        ("no condition at all", wedged("ContainerCreating", None)),
    ] {
        let card = one_card(p);
        assert!(
            card.evidence.contains("storage and its network"),
            "{label}: the sandbox exists, so the mounts succeeded and the network is up \
             — the card says so and points past them: {}",
            card.evidence
        );
        assert!(
            !card.evidence.contains("has not been able to give"),
            "{label}: and it must not blame storage the pod demonstrably has. This is \
             the half of the inversion that sent someone hunting a disk while an image \
             downloaded: {}",
            card.evidence
        );
    }
}

/// **The pod that reports `PodInitializing` and nothing else**, which is the shape rule
/// 13 was silent on when it first shipped — and it is most production pods.
///
/// The kubelet's `defaultWaitingState` is `PodInitializing` for **both** status arrays
/// whenever a pod declares an init container, so an Istio- or Linkerd-injected pod, a
/// `vault-agent-init` pod or most Helm charts report exactly this while wedged on a
/// missing volume: every container says the same uninformative word and the real reason
/// appears nowhere in the status at all. Reading that word as a pointer — *another
/// container goes first* — silenced the rule on the whole class it was added for.
///
/// **The preconditions are asserted first**, because a copy that had quietly kept a real
/// reason on one container would fire for the ordinary residual reason and pass this
/// test without ever exercising the branch it is about.
#[test]
fn a_pod_that_only_ever_says_podinitializing_is_the_wedge_the_rule_was_added_for() {
    let injected = capture_but("init", |pod| {
        let migrate = container_status(pod, "migrate");
        migrate.state = waiting_at(WAITING_ON_A_SIBLING, None);
        migrate.restart_count = 0;
        migrate.last_state = None;
    });
    println!(
        "{:?}",
        injected
            .containers
            .iter()
            .map(|c| (&c.name, &c.state))
            .collect::<Vec<_>>()
    );
    assert!(
        injected.containers.len() == 2
            && injected.containers.iter().all(|c| matches!(
                &c.state,
                ContainerState::Waiting { reason, .. }
                    if reason.as_deref() == Some(WAITING_ON_A_SIBLING)
            )),
        "every container has to carry the default waiting state and nothing else, or \
         this fires for an ordinary residual reason and proves nothing: {:?}",
        injected.containers
    );
    assert!(
        nothing_else_to_point_at(&injected),
        "and there has to be nothing for that word to point at — no container running, \
         none carrying a reason of its own"
    );

    let all = analyze(&pods_at(vec![injected], now()));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one card: this pod has been on a machine for three hours, nothing in it has \
         started, and `PodInitializing` is the only thing it has said — which is exactly \
         as wedged as one saying `ContainerCreating`, and was silence before: {:?}",
        titles(&all)
    );
    let card = only(&all, "broken-init", "not been able to start");
    assert!(
        card.evidence.contains("has not said which step it is on"),
        "and the card says the machine named no step, rather than quoting \
         `PodInitializing` as if it were one — it is the kubelet's default waiting \
         state, and dressing the least informative string in the status up as a \
         diagnosis is invariant 14 backwards: {}",
        card.evidence
    );
}

/// **An init container that *succeeded* is not something to point the reader at**, and reading it
/// as one silenced rule 13 on the same wedge the test above is about (NOTES § D114).
///
/// The shape is the one that made this rule exist, one step further along: an Istio- or
/// `vault-agent-init`-injected pod, or a migration pod, whose init containers **finished** and
/// whose main container then cannot be created — a hung image pull, an unresponsive containerd, a
/// runtime that never returns a status. The kubelet keeps writing [`WAITING_ON_A_SIBLING`] onto
/// the regular containers for as long as the pod *declares* an init container: `hasInitContainers`
/// is `len(pod.Spec.InitContainers) > 0` and nothing about whether they ran
/// (`kubelet_pods.go:2119-2125`, `:2499-2501`, v1.36).
///
/// **Nothing else in [`analyze`] reaches this pod**, which is why the silence was total rather
/// than a card of the wrong shape: [`no_node_accepted_it`] and [`nothing_has_looked_at_it`] leave
/// on `PodScheduled: True`, [`crash_looping`] needs a backoff reason, [`restarting_repeatedly`] a
/// restart count, [`previous_run_failed`] a `lastState`, [`stopped_for_good`] an ending that is
/// not [`Ending::Finished`], and [`running_but_not_ready`] a container that is `Running` — a pod
/// whose init step succeeded and whose main container then never started has not one of them.
/// Fourteen hours in, it drew **zero** findings. The assertion below is `all.len() == 1` for that
/// reason and not for tidiness.
///
/// **The shape is built and no capture holds it** (NOTES § D40): every pod this repository has
/// captured whose init container finished went on to run its main container, which is what
/// `healthy.json` is. The plant puts `app` back to the kubelet's default waiting state and the pod
/// back to `Pending` — `Initialized` stays `True`, because the init container did finish, which is
/// the one thing that separates this object from [`sandbox_rebuilt`]'s.
#[test]
fn a_finished_init_container_is_not_something_the_card_can_point_the_reader_at() {
    let wedged = capture_but("healthy", |pod| {
        never_ran(pod, "app", WAITING_ON_A_SIBLING, None);
        pod.status.as_mut().expect("the capture has a status").phase = Some("Pending".to_string());
        for type_ in ["Ready", "ContainersReady"] {
            pod_condition(pod, type_).status = "False".to_string();
        }
    });
    let migrate = container(&wedged, "migrate");
    let app = container(&wedged, "app");
    println!("migrate={:?}\n  app={:?}", migrate.state, app.state);

    let ContainerState::Terminated(run) = &migrate.state else {
        panic!("the base capture's init container has to be stopped in a run: {migrate:?}");
    };
    assert_eq!(
        ending(run),
        Ending::Finished,
        "and that run has to have *succeeded* — any other ending is something to point at, \
         and this test would then be about the clause it is not about: {run:?}"
    );
    assert_eq!(
        migrate.role,
        ContainerRole::Init,
        "on the container whose success is what makes the kubelet keep writing \
         `PodInitializing` — a regular container that finished is a different pod"
    );
    assert!(
        matches!(
            &app.state,
            ContainerState::Waiting { reason, .. } if reason.as_deref() == Some(WAITING_ON_A_SIBLING)
        ) && app.last_terminated.is_none(),
        "and the main container has to be sitting in the kubelet's default waiting state \
         having never run, which is the word this pod has nothing else to say than: {app:?}"
    );
    assert!(
        nothing_else_to_point_at(&wedged),
        "a container that finished is not a container to wait for and not a reason to \
         quote — reading it as either is what left this pod with no card at all"
    );

    let all = analyze(&pods_at(vec![wedged], now()));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one card: this pod got a machine fourteen hours ago, its init step succeeded, and \
         nothing has started since — and no other rule in this file can see it: {:?}",
        titles(&all)
    );
    let card = only(&all, "healthy", "not been able to start");
    assert!(
        card.evidence.contains("container app")
            && card.evidence.contains("has not said which step it is on"),
        "and the card is about the container that never started, quoting no diagnosis \
         because the kubelet gave none: {}",
        card.evidence
    );
}

/// **A pod with something already serving is not a pod that has not been able to
/// start**, and the title is the whole reason for the skip.
///
/// One typo in a sidecar's image leaves a pod `kubectl get pods` shows as `1/2`. A card
/// saying the pod has not started sends the reader to debug the container that has been
/// answering traffic for three minutes — and nothing else in [`analyze`] filters that
/// pod out, because it stays `phase: Pending`.
///
/// **What this costs is named rather than hidden:** the wedged container here draws no
/// card from any rule in the file. That is the trade — a true silence over a confident
/// false sentence ([`placed_but_never_started`]).
#[test]
fn a_pod_with_something_already_serving_gets_no_card_saying_it_never_started() {
    let half_up = capture_but("healthy-sidecar", |pod| {
        never_ran(
            pod,
            "app",
            "CreateContainerError",
            Some("failed to create containerd task"),
        );
    });
    let proxy = container(&half_up, "proxy");
    let app = container(&half_up, "app");
    println!("proxy={:?}\n  app={:?}", proxy.state, app.state);
    assert!(
        is_running(proxy) && proxy.role == ContainerRole::Sidecar,
        "the sidecar has to still be up — it is the container the card would send the \
         reader away from: {proxy:?}"
    );
    assert!(
        stuck_at_the_starting_line(app, nothing_else_to_point_at(&half_up)).is_some(),
        "and the app container has to satisfy every other clause of the rule, or the \
         silence below is not the skip's doing: {app:?}"
    );

    nothing(
        &analyze(&pods_at(vec![half_up], now())),
        "half of this pod is serving, so *it has not been able to start* is false about \
         it — and a confident plain-language sentence that is false about the pod in \
         front of the reader is the 3am failure this file exists to avoid",
    );
}

/// **Two containers, two different failures, and the card may not call them the same
/// thing.** `InvalidImageName` and `ErrImageNeverPull` need two different fixes; folding
/// the second into *"1 other container in the same state"* is the card inventing an
/// agreement the kubelet never reported.
#[test]
fn two_containers_stuck_for_different_reasons_are_both_named() {
    let mixed = capture_but("hostpath", |pod| {
        never_ran(pod, "nosy", "CreateContainerError", None);
        never_ran(pod, "shipper", "RunContainerError", None);
    });
    let all = analyze(&pods_at(vec![mixed], now()));
    show(&all);
    let card = only(&all, "broken-hostpath", "not been able to start");
    assert!(
        card.evidence.contains("shipper (RunContainerError)"),
        "the second container is named with its own reason, because it is a different \
         failure with a different fix: {}",
        card.evidence
    );
    assert!(
        !card.evidence.contains("in the same state"),
        "and it is not counted as agreeing with the first — the count is for containers \
         the kubelet actually reported the same way: {}",
        card.evidence
    );
}

/// **Every way the kubelet says the image is not coming, answered by rule 3 and not by a
/// ten-minute wait.** `nginx:doesnotexist` drew rule 3's CRITICAL immediately with the
/// registry's sentence; `NGINX:::latest` drew nothing for ten minutes and then a WARN
/// about starting that blamed a disk. Two typos, two unrecognisably different answers.
///
/// The moment below is **seven seconds** after the pod was placed — well inside rule
/// 13's grace — so this asserts the answer arrives *now*, which is half the point.
///
/// **One list, so this is one test for two rules.** [`UNUSABLE_IMAGE`] is rule 3's
/// trigger and rule 13's exclusion at the same time, so a reason added to rule 3 that
/// somebody forgot to exclude from the residual is not a shape that exists.
#[test]
fn every_unusable_image_reason_is_rule_threes_card_and_arrives_at_once() {
    let just_placed = time("2026-08-12T20:46:00Z");
    for reason in UNUSABLE_IMAGE {
        let broken = capture_but("config", |pod| {
            never_ran(pod, "app", reason, Some("the runtime's own sentence"));
        });
        let all = analyze(&pods_at(vec![broken.clone()], just_placed.clone()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "{reason}: rule 3 alone, and immediately — the reader does not wait ten \
             minutes to be told a typo is a typo: {:?}",
            titles(&all)
        );
        let card = only(&all, "broken-config", "image is not usable");
        assert_eq!(
            card.severity,
            Severity::Critical,
            "{reason}: this image is never becoming available on its own"
        );
        assert!(
            card.title.contains(reason) && card.evidence.contains("the runtime's own sentence"),
            "{reason}: the reason names which of the seven it is and the kubelet's \
             sentence carries the diagnosis: {} / {}",
            card.title,
            card.evidence
        );

        // ...and three hours later it is still rule 3's card, never the residual's.
        let later = analyze(&pods_at(vec![broken], now()));
        assert_eq!(
            titles(&later),
            titles(&all),
            "{reason}: past rule 13's grace the answer must not change into a WARN about \
             starting — one incident, one card, and the right one"
        );
    }
}

/// **Somebody has already given up on the wedged pod and deleted it**, and rule 13
/// stands down for the reason rule 10 does — the mutation sweep found this clause
/// holding nothing, the same way [D73](NOTES.md) found rule 10's.
///
/// Both cards are *true* about this pod: it never started, and it is not going away.
/// Only one is actionable. Rule 13's action sends the reader to the machine's Events to
/// find out what it is still waiting for, and the answer has stopped mattering the
/// moment the pod is on its way out; what is left to do is find what is holding the
/// delete, which is rule 12's card and names the finalizer. Alerts is D2's queue of what
/// is broken now **and** actionable.
#[test]
fn a_wedged_pod_someone_has_already_deleted_is_rule_twelves_alone() {
    let abandoned = capture_but("config", |pod| {
        never_ran(pod, "app", "ContainerCreating", None);
        pod.metadata.deletion_timestamp = Some(time("2026-08-12T21:00:00Z"));
        pod.metadata.deletion_grace_period_seconds = Some(30);
        pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
    });
    assert!(
        abandoned
            .containers
            .iter()
            .any(|c| stuck_at_the_starting_line(c, nothing_else_to_point_at(&abandoned)).is_some()),
        "the pod still satisfies every other clause of rule 13, which is what makes the \
         silence below the deletion's doing: {:?}",
        abandoned
            .containers
            .iter()
            .map(|c| &c.state)
            .collect::<Vec<_>>()
    );

    let all = analyze(&pods_at(vec![abandoned], now()));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "rule 12 alone: *what is the machine still waiting for* has stopped being a \
         question anyone can act on once the pod has been asked to go: {:?}",
        titles(&all)
    );
    only(&all, "broken-config", "asked to shut down");
}

/// **Two containers of one pod wedged on the same node.** A missing volume blocks every
/// container of the pod at once — one fault, so one card with a count rather than the
/// same sentence per container.
///
/// `hostpath.json` is the base because it is the repository's only multi-container pod
/// whose containers are peers. The planted shape is kept coherent with the cause it
/// names: `ContainerCreating` on both **and** the condition at `False`, which is what a
/// volume that will not mount actually produces, since the mount is attempted before the
/// sandbox exists.
#[test]
fn two_containers_stuck_on_the_same_node_are_one_card_with_a_count() {
    let wedged = capture_but("hostpath", |pod| {
        for name in ["nosy", "shipper"] {
            never_ran(pod, name, "ContainerCreating", None);
        }
        pod_condition(pod, "PodReadyToStartContainers").status = "False".to_string();
    });
    assert_eq!(
        wedged
            .containers
            .iter()
            .filter(|c| stuck_at_the_starting_line(c, nothing_else_to_point_at(&wedged)).is_some())
            .count(),
        2,
        "both containers have to reach the rule, or the count below is untested: {:?}",
        wedged
            .containers
            .iter()
            .map(|c| &c.state)
            .collect::<Vec<_>>()
    );

    let all = analyze(&pods_at(vec![wedged], now()));
    show(&all);
    let card = only(&all, "broken-hostpath", "not been able to start");
    assert!(
        card.evidence
            .contains("1 other container in the same state"),
        "one card for the pod, and the second container is a count rather than a second \
         copy of the same sentence — the node is what is wrong, not either container: {}",
        card.evidence
    );
    assert_eq!(
        all.iter()
            .filter(|f| f.title.contains("not been able to start"))
            .count(),
        1,
        "and it is one card and not two, which is the whole reason this rule takes the \
         pod rather than being called per container: {:?}",
        titles(&all)
    );
}

/// **The ten minutes, from both sides of the line.** A threshold nobody crosses is a
/// threshold nobody has tested, and this one is the whole difference between rule 13 and
/// a card on every cold start of a large image.
///
/// **On the captured wedge and not a planted one** — `broken-wedged` mounts a `configMap`
/// that does not exist, so the kubelet leaves it at `ContainerCreating` with the sandbox
/// never created. The two readings below are that pod's own `PodScheduled` transition at
/// `+10:00` and at `+10:01`, so the pair survives a recapture without being repinned; the
/// **ten minutes are written out rather than read from [`NOT_READY_GRACE`]**, since a
/// boundary computed from the constant under test agrees with every value it could hold.
#[test]
fn a_pod_only_just_placed_is_a_slow_pull_and_not_a_wedge() {
    let wedged = pod("wedged");
    let placed = wedged
        .scheduled
        .as_ref()
        .and_then(|c| c.last_transition.clone())
        .expect("the capture says when it was placed");
    println!("placed at {placed:?}");
    let after = |secs: i64| {
        Time(
            placed
                .0
                .checked_add(SignedDuration::from_mins(10) + SignedDuration::from_secs(secs))
                .expect("ten minutes after a captured placement is a moment"),
        )
    };

    nothing(
        &analyze(&pods_at(vec![wedged.clone()], after(0))),
        "ten minutes to the second is inside the window, not past it: pulling a large \
         image onto a cold node legitimately takes minutes, and a rule firing under \
         `progressDeadlineSeconds`' own default alerts on every cold start",
    );

    let all = analyze(&pods_at(vec![wedged], after(1)));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one second later the same pod is a finding — and the pair is what keeps the \
         constant from being deleted with the suite still green: {:?}",
        titles(&all)
    );
}

/// **The clause the rule is named after, and it was held in place by nothing** — the
/// defect [D73](NOTES.md) recorded on rule 10, one box later and caught by looking for
/// it: deleting `if scheduled.status != "True"` leaves the whole suite green.
///
/// The reason is structural rather than an oversight in the captures. The only pod in
/// the repository with `PodScheduled: False` is `pending.json`, and no kubelet has ever
/// seen it, so it has **no container statuses at all** — the walk finds nothing and the
/// rule is silent for a reason that has nothing to do with the gate. Every other
/// capture is scheduled. So the shape that tells the two apart has to be planted, and
/// it is one the API server does not produce: container statuses appear only once a pod
/// is assigned to a node, and `PodScheduled` never goes back to `False` after that.
///
/// **A shape the API cannot produce is still worth a test when it is the only thing
/// standing between a card and a lie.** *"This pod was given a machine to run on"* is
/// false about an unschedulable pod, its `lastTransitionTime` dates the *refusal* rather
/// than a placement, and rule 10 already owns the pod and quotes the scheduler. The
/// planted status is what makes the clause fail out loud instead of silently.
#[test]
fn a_pod_no_machine_took_was_never_given_one_to_run_on() {
    let refused = pending_but(|pod| {
        pod.status
            .as_mut()
            .expect("the captured pod has a status")
            .container_statuses = Some(vec![ContainerStatus {
            name: "app".to_string(),
            image: "docker.io/library/busybox:latest".to_string(),
            state: waiting_at("ContainerCreating", None),
            ..ContainerStatus::default()
        }]);
    });
    println!(
        "scheduled={:?}\n  containers={:?}",
        refused.scheduled, refused.containers
    );
    assert_eq!(
        refused.scheduled.as_ref().map(|c| c.status.as_str()),
        Some("False"),
        "the pod is still the one no machine would take — only the kubelet's report is \
         planted, and it is planted precisely because the API server never writes one \
         for this pod"
    );
    assert!(
        stuck_at_the_starting_line(&refused.containers[0], nothing_else_to_point_at(&refused))
            .is_some(),
        "and the planted container satisfies every *other* clause of rule 13, or the \
         silence below is the walk finding nothing rather than the gate holding"
    );

    let all = analyze(&pods_at(vec![refused], now()));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "rule 10 alone. *Given a machine to run on* is false about a pod nothing would \
         take, and the moment beside it would date the refusal rather than a placement: \
         {:?}",
        titles(&all)
    );
    only(&all, "broken-pending", "will take this pod");
}

/// **A container that has run before is not a container that never started**, which is
/// what the title claims and therefore what the rule has to mean.
///
/// The shape is real: a container that ran, died, and now cannot be recreated because
/// the node lost the disk under it — `CreateContainerError`, a reason no rule owns, so
/// the exclusion list does not reach it. What keeps rule 13 off it is
/// [`ContainerSnapshot::last_terminated`], and the pod is not invisible meanwhile: the
/// restarts that got it there are rule 5's card.
#[test]
fn a_container_that_ran_and_died_is_not_one_that_never_started() {
    let recreating = capture_but("crashloop", |pod| {
        container_status(pod, "quitter").state = waiting_at(
            "CreateContainerError",
            Some("failed to create containerd task"),
        );
    });
    let quitter = container(&recreating, "quitter");
    println!("{:?}\n  restarts {}", quitter.state, quitter.restarts);
    assert!(
        quitter.last_terminated.is_some() && !EXPLAINED_ELSEWHERE.contains(&"CreateContainerError"),
        "the edit has to leave a previous run on the container and pick a reason no \
         other rule owns, or this passes for the wrong reason: {quitter:?}"
    );

    let restarts = quitter.restarts;
    let all = analyze(&pods_at(vec![recreating], now()));
    show(&all);
    assert!(
        !all.iter()
            .any(|f| f.title.contains("not been able to start")),
        "this container started {restarts} times — *never started* would be a plain lie \
         about it, and the card that is true here is the restart count: {:?}",
        titles(&all)
    );
    only(
        &all,
        "broken-crashloop",
        &format!("restarted {restarts} times"),
    );
}

/// **The captured pod nothing has judged** — rule 14's shape, and it is a capture now rather
/// than a removal. `broken-unjudged` names `schedulerName: does-not-exist`, so nothing in the
/// cluster ever looked at it and the API server wrote no `conditions` key at all: `phase:
/// Pending`, no container statuses, and the `creationTimestamp` the grace is measured from.
///
/// Until the 2026-08-13 trip this was `pending.json` with its verdict deleted, because every
/// committed pod carried a `PodScheduled` condition — including the static pods in
/// `kube-system-pods.json` that no scheduler ever saw, since the kubelet writes the condition
/// itself for a pod handed straight to it.
fn never_judged(edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    capture_but("unjudged", edit)
}

/// **Rule 14, and the pod it must not be confused with** — the same capture with and
/// without the scheduler's line on it, which is the whole distinction the rule is.
///
/// The unedited capture is `Pending` *with* a verdict: something looked at it and refused
/// it, which is rule 10's card. Take the verdict away and nothing has looked at it at all,
/// which no other rule in this file can see — it has no container statuses for rules 1–7,
/// no condition for rules 10 and 13, and no `deletionTimestamp` for rule 12. Without this
/// rule that pod produces the empty screen `screens/once.md` promises means *nothing is
/// broken* (NOTES § D74).
///
/// **Both framings of the absence are fed**, because two different producers reach it: the
/// API server writes no `conditions` key at all for a pod nothing has judged, and a client
/// or a prune can leave an empty array. `From<Pod>` collapses them and no rule may depend
/// on which arrived (CLAUDE.md — a check is proven only for the shapes it was fed).
#[test]
fn the_pod_nothing_has_judged_is_not_the_pod_something_refused() {
    let judged = pod("pending");
    assert!(
        judged.scheduled.is_some() && judged.phase.as_deref() == Some("Pending"),
        "the committed capture is Pending *and* carries a PodScheduled line — which is \
         this rule's negative, and the reason its positive has to be made by removal"
    );
    let refused = analyze(&pods_at(vec![judged], now()));
    show(&refused);
    assert_eq!(
        refused.len(),
        1,
        "rule 10 alone on the unedited capture: a pod something refused has been looked \
         at, and two cards about who looked at one pod is the screen contradicting \
         itself: {:?}",
        titles(&refused)
    );
    only(&refused, "broken-pending", "will take this pod");

    // **Both framings of the absence, and only one of them is a cluster's.** The API server
    // writes no `conditions` key at all for a pod nothing has judged — which is `unjudged.json`
    // exactly as captured — while a client or a prune can leave an empty array behind. `From<Pod>`
    // collapses them and no rule may depend on which arrived, so the second is written onto a
    // decoded copy of the first (NOTES § D53).
    let created = captured_time(&fixture("unjudged"), &["metadata", "creationTimestamp"]);
    for shape in [None, Some(Vec::new())] {
        let unjudged = capture_but("unjudged", |pod| {
            if shape.is_some() {
                pod.status
                    .as_mut()
                    .expect("the captured pod has a status")
                    .conditions = shape;
            }
        });
        assert_eq!(
            (unjudged.scheduled.as_ref(), unjudged.phase.as_deref()),
            (None, Some("Pending")),
            "both framings decode to the same absence, and the phase is the capture's own"
        );
        assert_eq!(
            unjudged.creation_timestamp.as_ref(),
            Some(&created),
            "and the moment the waiting started is the one the API server stamped, read \
             back out of the capture it came from"
        );

        let all = analyze(&pods_at(vec![unjudged], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "one card, and it is this rule's. Nothing else in the file has anything to \
             read on this pod: no container statuses for rules 1–7, no condition for \
             rules 10 and 13, no hostPath for rule 8 and no deletion stamp for rule 12: \
             {:?}",
            titles(&all)
        );
        let unlooked = only(
            &all,
            "broken-unjudged",
            "Nothing has even looked at this pod",
        );
        assert!(
            unlooked.title.contains("(it shows as Pending)"),
            "the card names the word `kubectl get pods` prints for this pod. The \
             parenthetical and the deletion guard are one decision: a deleted pod keeps \
             `phase: Pending` while the column reads Terminating, so this assertion and \
             `the_unjudged_pod_someone_deleted_is_rule_twelves_alone` hold one half each: {}",
            unlooked.title
        );
        assert_eq!(
            unlooked.severity,
            Severity::Critical,
            "CRITICAL — nothing healthy looks like this, and the pod will not start on \
             its own (NOTES § D74)"
        );
        assert!(
            unlooked.evidence.contains("PodScheduled"),
            "the word is named so the reader can find it, and explained by the two states \
             that both write it rather than left bare (invariant 14): {}",
            unlooked.evidence
        );
        assert!(
            unlooked.action.contains("kube-scheduler")
                && unlooked.action.contains("spec.schedulerName"),
            "both causes, neither claimed — a scheduler that is not running and a \
             scheduler named on the pod that nobody runs: {}",
            unlooked.action
        );
        assert_eq!(
            unlooked.kubectl_cmd.as_deref(),
            Some("kubectl get pod broken-unjudged -n default -o yaml"),
            "`get -o yaml` and not `describe`: an absent condition is visible in the yaml, \
             and `spec.schedulerName` — the field that separates the two causes — is \
             printed by neither `describe` nor any Event"
        );
        assert_eq!(
            unlooked.timestamp.as_ref(),
            Some(&created),
            "the age is how long the pod has been waiting for anything to look at it. \
             There is no event of its own to date it by — that absence is the finding"
        );
    }
}

/// **The two minutes, from both sides of the line.** A threshold nobody crosses is a
/// threshold nobody has tested, and this one is the difference between a card and a red
/// screen every time the control plane hands over.
///
/// kube-scheduler's leader election defaults to a 15s lease with a 10s renew deadline, so
/// a handover completes in seconds; two minutes is eight times that (NOTES § D74). **The
/// seconds below are that requirement's own number and not [`NEVER_JUDGED_GRACE`]'s** —
/// computing them from the constant would move with any edit to it and prove nothing.
#[test]
fn a_pod_created_a_moment_ago_is_a_handover_and_not_a_missing_scheduler() {
    let fresh = never_judged(|_| {});
    let created = fresh
        .creation_timestamp
        .clone()
        .expect("the capture says when the pod arrived");
    let at = |secs: i64| {
        Time(
            created
                .0
                .checked_add(SignedDuration::from_secs(secs))
                .expect("the capture's creation time plus two minutes is representable"),
        )
    };
    println!(
        "created at {created:?}, read at {:?} and {:?}",
        at(120),
        at(121)
    );

    nothing(
        &analyze(&pods_at(vec![fresh.clone()], at(120))),
        "two minutes to the second is inside the window, not past it: leadership moves \
         between schedulers in about fifteen seconds and a pod created during one is not \
         a pod nothing will ever look at",
    );

    let all = analyze(&pods_at(vec![fresh], at(121)));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one second later the same pod is a finding — and the pair is what keeps the \
         constant from being deleted with the suite still green: {:?}",
        titles(&all)
    );
}

/// **A pod with no arrival time cannot be shown to have waited**, so it draws nothing —
/// the same direction as rule 13's unstamped condition and the opposite of rule 10's,
/// because here the grace *is* the gate rather than a severity band.
///
/// The API server stamps every accepted create, so the shape's real producer is a prune
/// that drops the field on the way in — which is why the field is one `k8s.rs` must keep
/// (invariant 6). The pod is not invisible in that case; it is invisible in *this file*,
/// and that is the honest failure for a rule whose whole content is a duration.
#[test]
fn a_pod_with_no_arrival_time_cannot_be_shown_to_have_waited() {
    let undated = never_judged(|pod| pod.metadata.creation_timestamp = None);
    println!(
        "phase={:?} scheduled={:?} created={:?}",
        undated.phase, undated.scheduled, undated.creation_timestamp
    );
    assert!(
        undated.phase.as_deref() == Some("Pending") && undated.scheduled.is_none(),
        "every other clause of the rule still holds, so the silence below is the missing \
         stamp and nothing else"
    );
    nothing(
        &analyze(&pods_at(vec![undated], now())),
        "no moment to measure from is no finding: a missing field means no finding \
         (invariant 5), never a default that fires",
    );
    assert_eq!(
        analyze(&pods_at(vec![never_judged(|_| {})], now())).len(),
        1,
        "and the same pod *with* its stamp is a card — without this line the assertion \
         above would pass just as well against a rule that never fires at all"
    );
}

/// **A pod that is not `Pending` is a pod something has plainly looked at**, whatever its
/// conditions array says — it is running, so it was placed and started. The gate is the
/// phase and not the absence alone.
///
/// The shape is planted because the API server does not produce it: a Running pod always
/// carries the condition. That is the point — the clause has to fail out loud rather than
/// be held up by a capture that happens never to test it (NOTES § D73, the clause rule 13
/// found held in place by nothing).
#[test]
fn a_running_pod_missing_its_conditions_is_not_one_nothing_has_looked_at() {
    let strip = |phase: &str| {
        let phase = phase.to_string();
        capture_but("healthy", move |pod| {
            let status = pod.status.as_mut().expect("the captured pod has a status");
            status.conditions = None;
            status.phase = Some(phase);
        })
    };
    let running = strip("Running");
    println!(
        "phase={:?} scheduled={:?} created={:?}",
        running.phase, running.scheduled, running.creation_timestamp
    );
    assert!(
        running.scheduled.is_none() && running.creation_timestamp.is_some(),
        "the absence and the arrival time are both there, so only the phase stands \
         between this pod and the card"
    );
    nothing(
        &analyze(&pods_at(vec![running], now())),
        "*nothing has even looked at this pod* about a pod that is running would be the \
         card contradicting the phase beside it on the same screen",
    );

    // The control: the same pod with only the phase moved. Without it the silence above
    // would also be satisfied by a stamp too young to have cleared NEVER_JUDGED_GRACE.
    assert_eq!(
        analyze(&pods_at(vec![strip("Pending")], now())).len(),
        1,
        "the same pod, Pending, is a card — so the phase is what silenced it and not an \
         arrival time still inside the two minutes"
    );
}

/// **The unjudged pod somebody deleted is rule 12's alone.** Both cards are true of it —
/// nothing looked at it, and it is not going away — and only rule 12's is actionable:
/// *check whether the scheduler is running* is advice about a pod nobody wants scheduled
/// any more, while rule 12 names the finalizer holding it (NOTES § D73).
///
/// **It also keeps two words off one pod.** `printPod` prints **Terminating** for any
/// non-terminal phase carrying a `deletionTimestamp` while `phase` stays `Pending`
/// underneath, so without the guard this card's *(it shows as Pending)* would sit beside
/// rule 12's *(it shows as Terminating)* about one pod. The guard and the parenthetical
/// are one decision in two places, and this is the test that fails if either goes.
#[test]
fn the_unjudged_pod_someone_deleted_is_rule_twelves_alone() {
    // The delete lands a minute after the capture's own creation, so the moment moves with the
    // fixture rather than being transcribed beside it.
    let asked_at = Time(
        captured_time(&fixture("unjudged"), &["metadata", "creationTimestamp"])
            .0
            .checked_add(SignedDuration::from_mins(1))
            .expect("a minute after a captured creation is a moment"),
    );
    let deleted = never_judged(|pod| {
        pod.metadata.deletion_timestamp = Some(asked_at);
        pod.metadata.deletion_grace_period_seconds = Some(30);
        pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
    });
    assert_eq!(
        deleted.phase.as_deref(),
        Some("Pending"),
        "the phase does not move when a pod is deleted, which is exactly why the \
         parenthetical cannot be trusted to the phase alone"
    );

    let all = analyze(&pods_at(vec![deleted], now()));
    show(&all);
    assert_eq!(all.len(), 1, "rule 12 alone: {:?}", titles(&all));
    let terminating = only(&all, "broken-unjudged", "asked to shut down");
    assert!(
        terminating.title.contains("Terminating"),
        "and the one card left names the word `kubectl get pods` actually prints — the \
         word this rule's card would have contradicted: {}",
        terminating.title
    );
}

/// The whole committed capture through [`analyze`] at once — every card printed, so
/// that `cargo test -- --nocapture` shows what a user would actually read, and the
/// properties every finding owes regardless of which rule made it.
#[test]
fn the_whole_capture_through_the_rules_at_once() {
    let all = findings(&CAPTURED_PODS);
    show(&all);
    println!(
        "{} critical, {} warnings",
        all.iter()
            .filter(|f| f.severity == Severity::Critical)
            .count(),
        all.iter().filter(|f| f.severity == Severity::Warn).count()
    );

    // **24 since the 2026-08-16 capture trip** (NOTES § D114), from 22, and the two are both new
    // captures rather than a rule that started firing twice: `probe0.json` draws rule 5's
    // clean-ending card and a readiness card, `neverrules.json` draws rule 6's `exit 3`. The
    // other two captures of that trip draw **nothing**, which is the half worth naming — `gang`
    // is two containers a restart rule parked and put back, and `reboot` is a container serving
    // again long enough that rule 5's card has aged out at the pin.
    //
    // The number also absorbed a card that *went*: rule 13 drew on `broken-init` whenever the
    // capture caught its init container between runs rather than in backoff, naming the **app**
    // container — [`nothing_else_to_point_at`] is where that was fixed in the same change.
    assert_eq!(
        all.len(),
        24,
        "one card per thing that is broken across every pod the repository has captured, \
         counted rather than described: the list is long enough now that a sentence naming \
         each one would be a second copy of the tests above, and a number that moves when a \
         rule starts firing twice is what this assertion is for: {:?}",
        titles(&all)
    );
    // **The twenty-seventh, named rather than absorbed into the number** (NOTES § D96, § D97).
    // `neverback.json` was in the silent list below until rule 15, and it moved because the
    // capture landed with it: a container stopped for good inside a pod that is still `Running`
    // used to draw nothing anywhere in k8rs. **One card and not two** — the same pod's `done`
    // exited `0`, which under `Never` is the policy working, and that silence is leg 7's.
    assert_eq!(
        all.iter()
            .filter(|f| f.object.name == "broken-neverback")
            .map(|f| (f.title.as_str(), f.evidence.as_str()))
            .collect::<Vec<(&str, &str)>>(),
        vec![(
            "This container has stopped and nothing is starting it again",
            "container broke · exit 1 (the application's own error) · ran for under a second"
        )],
        "the one card the new capture draws, and the container it is about"
    );

    // **Which captures are allowed to say nothing, named — and everything else has to speak.**
    // A count alone passes just as well if one rule went silent and another started firing
    // twice. The silent set is the healthy fixtures, the three that are only an Analysis
    // posture row, the two pods that are *over* — a finished pod's restart counts and last exits
    // are not what is broken now, which is all this screen holds (D2), and they reach no other
    // screen either (D96) — and the three whose fault is real but old.
    let silent = [
        // The kill in this one is an hour old and its container has been serving since, which
        // is rule 2's recency clause deciding — read at a `now` five minutes after the kill it
        // is a CRITICAL, and `an_old_kill_on_a_container_that_has_been_fine_since_…` reads it
        // both ways off these same bytes.
        "oomserving",
        // **The same shape, one rule over, since D100.** Both of these are containers that used
        // their restarts and have been serving for the 49 hours between the capture and the pin,
        // and `restartCount` never comes down — so at *this* moment they are old news, and at a
        // moment inside their run they are two of the cards this screen exists for.
        // `a_container_that_looks_fine_…` and `ten_restarts_is_red_…` read both directions off
        // these same bytes.
        "restarts",
        "restarts10serving",
        // **The 2026-08-16 trip's two settled restarts, and they are the same class** (NOTES
        // § D114): three restarts each, `ready`, running again since before the pin. Both were
        // checked to speak rather than assumed to — read at a moment inside their run, `gang`
        // draws *"restarted 3 times — it is serving now, and the record names the pod's rule"*
        // on **both** its containers and `reboot` draws *"…and the exit code is not its own"*.
        // So each is a capture some rule reads, silent here because the fault is old and not
        // because nothing looked at it.
        "gang",
        "reboot",
        // **And one that became silent on the same trip** (NOTES § D114). `broken-startup` is
        // rule 7's `started` suppressor — up, not ready, and still inside a `startupProbe` — so
        // Alerts is right to say nothing about it, and
        // `a_container_still_inside_its_startup_probe_…` is the test that reads it. It used to
        // speak here as well, through a `137` in `lastState` that rule 6 drew on; that run was an
        // artifact of a capture session long enough to outlast `failureThreshold: 720` ×
        // `periodSeconds: 5` (an hour), and the 2026-08-16 trip captured at ~30 minutes. Nothing
        // in `scripts/cluster.sh` § `[startup]` ever asked for it, so it is not a shape a trip
        // can be relied on to bring — the two tests that need it plant it now.
        "startup",
        "healthy",
        "healthy-hostpath",
        "healthy-podlevel",
        "healthy-retry",
        "healthy-sidecar",
        "healthy-unreadysidecar",
        "nolimits",
        "podlimit",
        "resize",
        "succeeded",
        "failed",
    ];
    for name in CAPTURED_PODS {
        let object = pod(name);
        let mine: Vec<&str> = all
            .iter()
            .filter(|f| f.object.name == object.id.name)
            .map(|f| f.title.as_str())
            .collect();
        if silent.contains(&name) {
            assert!(
                mine.is_empty(),
                "{name}.json is one of the captures Alerts must not draw at all: {mine:?}"
            );
        } else {
            assert!(
                !mine.is_empty(),
                "{name}.json was captured because something is wrong with it, and this \
                 screen says nothing about it"
            );
        }
    }

    for f in &all {
        assert_ne!(
            f.severity,
            Severity::Info,
            "no rule reaching the Alerts list produces an Info — D2 sends those to a \
             report: {}",
            f.title
        );
        assert!(
            !f.title.is_empty() && !f.action.is_empty(),
            "a card is what happened · what it means · what to do, and the third is \
             what makes it a work queue rather than a lint report: {f:?}"
        );
        let cmd = f
            .kubectl_cmd
            .as_deref()
            .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
        assert!(
            cmd.contains(&f.object.name) && cmd.contains("-n default"),
            "invariant 4's teaching device points at the object the card is about, in \
             its own namespace: {cmd}"
        );
        assert_eq!(
            f.owner, f.object,
            "`scripts/broken.yaml` creates bare pods, so every one of these files under \
             itself — the owned case is asserted below"
        );
    }
}

/// **The grouping key on a pod that has a controller**, which is D3's whole premise and
/// is not visible in any of the twelve bare captures above. This one is a real capture
/// of a Deployment's pod, so nothing is synthesized.
#[test]
fn a_finding_on_an_owned_pod_files_under_the_controller_and_not_the_pod() {
    let pods: Vec<PodSnapshot> = items::<Pod>("owned-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let all = analyze(&pods_at(pods, now()));
    show(&all);

    let name = owned_pod_name();
    // **Every card on the pod, not one picked by title** (NOTES § D114). Which rule speaks here
    // depends on which half of the backoff loop `just fixtures` caught — `scripts/cluster.sh`
    // § `[owned]` certifies both, having measured `state.terminated` in 39 samples of 70 — and
    // keying on `CrashLoopBackOff` asserted the coin-flip. The grouping under test is a property
    // of *every* finding this pod draws, so asking all of them is both face-independent and a
    // stronger question than the one it replaces.
    let on_pod: Vec<&Finding> = all.iter().filter(|f| f.object.name == name).collect();
    assert!(
        !on_pod.is_empty(),
        "the capture is a crash-looping pod under a ReplicaSet and something has to say so, or \
         every assertion below sweeps an empty list: {:?}",
        titles(&all)
    );
    for looping in &on_pod {
        assert_eq!(
            looping.object.kind,
            ObjectKind::Pod,
            "what the rule looked at is the pod"
        );
        assert_eq!(
            looping.owner.kind,
            ObjectKind::ReplicaSet,
            "and what it files under is the controller — `k8s.rs` resolves this up to the \
             Deployment in Phase 5 (D28), and this layer records what the object said"
        );
        assert_eq!(looping.owner.name, "broken-owned-7bdb7645c8");
        assert_ne!(
            looping.owner, looping.object,
            "D3 groups by owner, and a card per pod is the failure mode it exists to stop"
        );
        assert_eq!(
            looping.kubectl_cmd.as_deref(),
            Some(format!("kubectl logs {name} -c quitter -n default --previous").as_str()),
            "the command still points at the object, never at the card's title — a \
             `logs broken-owned-7bdb7645c8` is a command that does not work. **The verb moved on \
             2026-08-16 and the requirement did not**: this arm's action names that run's log, so \
             the command serves one (NOTES § D113)"
        );
    }
}

// --- THE RUN A CONTAINER IS SITTING IN RIGHT NOW START ---
//
// **`state.terminated` is no rule's subject, and this section is the ruling and not a
// description of what the code happens to do today** (NOTES § D96). One function reads the field —
// [`doing_its_job`] — and it reads it as a *suppressor*: it asks [`ending`] whether an **init**
// container finished, and answering *yes* takes rules 2, 5 and 6 away. **No card is ever drawn
// from it**, which is the thing this section pins: the run a container is sitting in cannot put
// a sentence on the screen, cannot change one, and cannot date one. Four things decide it, and
// none of them is re-argued here:
//
// 1. **A pod that is over is already out.** [`analyze`] skips **every pod rule except rule 12**
//    when [`finished`] — `Succeeded` or `Failed`. Rule 12 sits outside the gate on purpose: a
//    pod that will not go away is still stuck. The skip is proved on `failed.json`, this
//    repository's one capture with a bad exit in this field, by
//    [`a_pod_that_finished_is_charged_to_nobody_and_alarms_about_nothing`]. It covers the
//    *stable* majority of the containers that sit here with a bad exit: a single-container pod
//    whose container dies under `restartPolicy: Never` or `OnFailure` goes terminal and leaves
//    the Alerts screen.
//
//    **And that is where it stops — it does not arrive anywhere else.** *Belongs to the Waste
//    report* is D2's rule for keeping Alerts to what is broken *now*; it is not a destination
//    that exists. `analysis.rs` is unwritten, the Waste report's charter is Evicted/Completed
//    **pileups** rather than a diagnosis of a Job pod that died a minute ago, and Jobs are not
//    watched at all. The honest sentence is that such a pod leaves this screen by D2 and **has
//    no other screen yet**. Still the ruling — but a smaller claim than *something else covers
//    it*.
// 2. **A finished init container is this field's normal state.** Every container any
//    committed capture holds in `state.terminated` inside a pod that is *not* over is an
//    init container at `exit 0` — asserted below, over the whole corpus, rather than
//    asserted about two files. Any reader of this field starts from a haystack of healthy
//    objects, which is why the one reader there is asks only the init question.
// 3. **What is left inside a non-terminal pod is redundant, and a card off it could never be
//    debounced.** *A transient a watch sees and `--once` may not* is the weaker argument and it
//    is measurably wrong: on a backing-off container `state.terminated {exit 3}` was the
//    **visible state across tens of seconds**, while kubectl's own STATUS column read `Error`.
//    **What survives is redundancy**, from the same sample: `state.terminated {exit 3}` and
//    `lastState {exit 3, Error}` were present *simultaneously* — rule 6 fires off the `lastState`
//    copy from restart 1, and rule 1's card follows from the backoff. So refusing the current
//    terminated state loses **nothing** about a container that comes back; not earliness, not a
//    corner, nothing.
//    **And the obvious reply — then debounce it — is closed by invariant 5.** A pure
//    `analyze(&Snapshot)` has nowhere to hold *I saw an exit 3 four seconds ago*: no globals, no
//    clock call, one snapshot in. A card drawn off this field is therefore a function of when
//    the sampler happened to look, permanently and by construction, and no care inside the rule
//    can make it otherwise.
// 4. **The cost is real and is not hidden.** The part that is a property of the feature, and is
//    now confirmed on two clusters: the trigger's own exit **never reaches `lastState`** — 0 of
//    80 samples — while the synthesized `137` was in every one of them. How often the trigger is
//    *visible* in `state.terminated` is a property of the **manifest** and not of the feature —
//    12 of 40 one-second samples on one cluster, 4 of 40 on another whose container lived
//    longer, so **10–30% depending on how long the container runs between exits**. Either way,
//    refusing to read the current terminated state means that container is never nameable by any
//    rule, and rule 5's card keeps its denial that the record says which container went first.
//    That silence is pinned below by name, on the shape that opened this box.
//
// **What the ruling does not cover is boxed, not forgotten**: a container that *cannot* come
// back inside a pod that is *not* over — `restartPolicy: Never`, or `OnFailure` with a clean
// exit, beside a sibling still running — is permanent rather than transient, and leg 3's
// redundancy does not hold for it, because there is no next restart to write a `lastState`.
//
// **Reading it takes more than one field, which is part of why it is a box and not a line.**
// `spec.restartPolicy` is the pod's, and at the pinned version it is no longer the whole answer:
// `ContainerRestartRules` is beta-on-by-default, so a **regular** container may override the pod
// with `restartPolicy: Never` and stay dead inside an `Always` pod — measured, `restartCount: 0`
// after fourteen minutes. NOTES § D96 carries the corrected table whole.
//
// **What is actually missing is narrower than *no plumbing*, and saying otherwise sends the next
// reader to write code that is already there.** [`container_snapshots`] reads
// `spec.containers[].restartPolicy` today — it is what tells a native sidecar from an init
// container — so the decode already reaches the field. What it does **not** do is *carry* it: the
// value is collapsed into [`ContainerRole`], and the regular list is deliberately not asked at
// all, because a regular container answers `Regular` either way. So the three genuinely unreached
// things are the pod's own `spec.restartPolicy`, the `restartPolicyRules` list, and a **regular**
// container's own override — the last of which the decode currently throws away rather than never
// having seen. **Scoping that box is the PM's and none of it is done here.**

/// **Every ending the current terminated state can hold** (NOTES § D29). `3` is the gang
/// trigger's own code — measured in `state.terminated` and never in `lastState` — and the two
/// `137` reasons are here because they are the pair that *does* move a card when it sits in
/// `lastState` ([`ending`], NOTES § D95): if anything read this field, those two would move one
/// from here too.
///
/// **The two `137` rows are load-bearing for a second reason, and trimming them costs it.** They
/// are the only endings [`terminated_now`] leaves **stamp-less** — the kubelet writes those two
/// with three fields and no more — so they are the only rows on which a reader that dated a card
/// from `state.terminated.finishedAt` produces a *different* age from one that did not. Delete
/// them and that reader survives all three tests: `tester` measured it. The age dimension is in
/// [`whole_card`]; these rows are what give it something to see.
///
/// **`0` is the row the reader that exists answers differently**, and it is kept in the same
/// table rather than tested apart, because the claim is about the *whole* set of endings and
/// not about the bad ones.
const CURRENT_RUNS: [(i32, Option<&str>); 6] = [
    (0, None),
    (1, None),
    (3, None),
    (143, None),
    (137, Some(STATUS_LOST)),
    (137, Some(RESTART_ALL)),
];

/// One shape the real pipeline hands the rules with a container sitting in `state.terminated`:
/// a label, the role under test, the container's name, the plant, and **how many cards the pod
/// draws about that container on a clean ending and on a bad one**.
///
/// **The two counts are written down rather than derived.** An equality between two empty sets
/// is exactly the silence this section could otherwise pass by producing — a plant that stopped
/// building the state, a helper that stopped matching the name (NOTES § D26, CLAUDE.md § Code
/// phase rules).
type Sitting = (
    &'static str,
    ContainerRole,
    &'static str,
    fn(i32, Option<&str>) -> PodSnapshot,
    usize,
    usize,
);

/// **All three roles, both states of the field's neighbour, a pod with one container and a pod
/// with two, and one pod that has a controller** (NOTES § D29). Every shape leaves the pod *not*
/// over, because a pod that is over never reaches a pod rule at all.
///
/// **Two of the seven draw nothing about the subject at any ending, and that zero is the
/// ruling's cost written down** rather than a shape that failed to be interesting: a container
/// on its **first** run has no count and no `lastState`, so this field is the only record of it
/// there is — and nothing reads it. It is the same silence the gang trigger gets, reached from
/// the other direction.
const SITTING: [Sitting; 8] = [
    (
        "owned regular",
        ContainerRole::Regular,
        "quitter",
        owned_regular,
        2,
        2,
    ),
    (
        "gang-restart trigger",
        ContainerRole::Regular,
        "nosy",
        gang_restart_trigger,
        2,
        2,
    ),
    // **One card, not two, since 2026-08-16** (NOTES § D113): rules 5 and 6 answer this ending
    // with one sentence and rule 5 carries the duration, so rule 6 adds nothing and folds. The
    // base carries no termination message, which is the fact that would have kept it standing.
    (
        "crashing regular",
        ContainerRole::Regular,
        "flaky",
        crashing_regular,
        1,
        1,
    ),
    (
        "regular, first run",
        ContainerRole::Regular,
        "nosy",
        regular_first_run,
        1,
        1,
    ),
    // Folded for the row above's reason (NOTES § D113).
    (
        "sidecar between restarts",
        ContainerRole::Sidecar,
        "proxy",
        sidecar_down,
        1,
        1,
    ),
    (
        "sidecar, first run",
        ContainerRole::Sidecar,
        "proxy",
        sidecar_first_run,
        0,
        0,
    ),
    // Folded for the two rows above's reason, on the bad half only — the clean half was already
    // silent (NOTES § D113).
    (
        "init that finished",
        ContainerRole::Init,
        "wait-for-db",
        init_finished,
        0,
        1,
    ),
    (
        "init, first run",
        ContainerRole::Init,
        "migrate",
        init_first_run,
        0,
        0,
    ),
];

/// **The run a container is in *right now*, rewritten** — `state.terminated`, where
/// [`ended_as`] writes `lastState.terminated`.
///
/// **Where the capture is already sitting in one the plant only moves the ending**, which is
/// the init shapes: `healthy.json` and `healthy-retry.json` were captured with a finished init
/// container, `ready: true` and `started: false`, and that is the kubelet's own shape for a
/// container that is *done* rather than *down*. Where the container was up, the run now ending
/// **is** the one that was running, so it keeps that run's `startedAt` and gains a `finishedAt`
/// after it — a record whose run began after the one it replaced is a timeline no cluster
/// writes — and both flags go to `false`, which is `failed.json`'s captured shape for a
/// container that is down (NOTES § D40).
///
/// **The reason moves with the code**, [`exited`]'s pairing: `Completed` beside `0`, `Error`
/// beside everything else, unless the row names one of the two the kubelet writes itself — and
/// those two carry three fields and no more, so the stamps go with them
/// ([`ended_as`], NOTES § D93).
fn terminated_now(pod: &mut Pod, name: &str, code: i32, reason: Option<&str>) {
    let status = container_status(pod, name);
    if status
        .state
        .as_ref()
        .and_then(|s| s.terminated.as_ref())
        .is_none()
    {
        let began = status
            .state
            .as_ref()
            .and_then(|s| s.running.as_ref())
            .and_then(|r| r.started_at.clone());
        assert!(
            began.is_some(),
            "{name} was neither terminated nor running in the capture, so this plant cannot say \
             when the run it is ending began"
        );
        // **The end is derived from the beginning, not written down** (NOTES § D114). A literal
        // here is a moment from whichever trip captured the base, and the 2026-08-13 one
        // (`23:40:00Z`) sat *before* the `startedAt` of every container re-captured on
        // 2026-08-16 — building exactly the timeline the paragraph above says no cluster
        // writes, and taking `ran_for` silently to `None` so the cards lost their duration.
        // An hour after the run began: long enough to be a real duration on any rung, and it
        // moves with the capture instead of against it.
        let ended = began
            .as_ref()
            .map(|t| Time(t.0 + SignedDuration::from_hours(1)));
        status.state = Some(ApiContainerState {
            terminated: Some(ContainerStateTerminated {
                started_at: began,
                finished_at: ended,
                ..ContainerStateTerminated::default()
            }),
            ..ApiContainerState::default()
        });
        status.ready = false;
        status.started = Some(false);
    }
    let run = container_status(pod, name)
        .state
        .as_mut()
        .and_then(|s| s.terminated.as_mut())
        .expect("the run above is either the capture's or this plant's");
    run.exit_code = code;
    run.reason = Some(
        reason
            .unwrap_or(if code == 0 { "Completed" } else { "Error" })
            .to_string(),
    );
    if matches!(run.reason.as_deref(), Some(STATUS_LOST | RESTART_ALL)) {
        run.started_at = None;
        run.finished_at = None;
        run.container_id = None;
    }
}

/// **The container beside the subject, made loud** — a previous run that failed and a count
/// inside rule 5's band, so a working rule set always has something to say about this pod.
/// Without one, a shape whose subject is silent proves nothing: *the rules read this field and
/// said nothing* and *the rules said nothing* print the same green line (NOTES § D26).
fn noisy_neighbour(pod: &mut Pod, name: &str) {
    ended_as(pod, name, 1, None, None);
    container_status(pod, name).restart_count = RESTARTS_WARN + 1;
}

/// **The shape that opened this box** (NOTES § D93). One gang restart writes the synthesized
/// `137` into *every* container's `lastState`, the trigger's included, while the trigger's own
/// exit sits in `state.terminated` and never reaches `lastState` at all. `broken-hostpath` is
/// the committed capture with two regular containers; `shipper` stays up and carries the record,
/// `nosy` is the one that went first.
fn gang_restart_trigger(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("hostpath", |p| {
        for name in ["nosy", "shipper"] {
            ended_as(p, name, 137, Some(RESTART_ALL), None);
            container_status(p, name).restart_count = RESTARTS_WARN + 1;
        }
        terminated_now(p, "nosy", code, reason);
    })
}

/// **A single-container pod, down between restarts** — the shape the operator review flagged,
/// because there is no sibling for a card to send the reader to and none of the comparisons the
/// two-container pod above allows. Its history is an ordinary crash history and not a gang
/// record: `restarts10` is captured `Running` and not ready at ten restarts, and here it has
/// just exited again.
fn crashing_regular(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("restarts10", |p| terminated_now(p, "flaky", code, reason))
}

/// **The same container on its first run**, so there is no `lastState` beside the field and no
/// count — the pair the kubelet writes together, moved together ([`never_ran`], NOTES § D40).
///
/// **Not the gang shape**: one restart-all firing writes the record into every container, so a
/// container with none of it did not live through one. Its sibling here has an ordinary crash
/// history, and that is what draws.
fn regular_first_run(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("hostpath", |p| {
        noisy_neighbour(p, "shipper");
        terminated_now(p, "nosy", code, reason);
        let subject = container_status(p, "nosy");
        subject.last_state = None;
        subject.restart_count = 0;
    })
}

/// **A native sidecar down between restarts.** `restartPolicy: Always` on an init container is
/// what makes it one, and it is what puts it back here every time it exits.
fn sidecar_down(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("healthy-unreadysidecar", |p| {
        noisy_neighbour(p, "proxy");
        terminated_now(p, "proxy", code, reason);
    })
}

/// The same sidecar on its **first** run, with nothing in `lastState` — so nothing but this
/// field says anything happened. The regular container beside it is what draws.
fn sidecar_first_run(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("healthy-unreadysidecar", |p| {
        noisy_neighbour(p, "app");
        terminated_now(p, "proxy", code, reason);
    })
}

/// **The field's normal state, over every ending it could hold instead.** `healthy-retry.json`
/// was captured here — a `wait-for-db` that failed three times and then exited `0` — so the
/// clean row is the committed object and the rest are the endings it did not have.
fn init_finished(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("healthy-retry", |p| {
        noisy_neighbour(p, "app");
        terminated_now(p, "wait-for-db", code, reason);
    })
}

/// The same, **first time**: `healthy.json`'s `migrate` succeeded on its first run, so it has
/// no count and no previous run — this field is the only record of it there is.
fn init_first_run(code: i32, reason: Option<&str>) -> PodSnapshot {
    capture_but("healthy", |p| {
        noisy_neighbour(p, "app");
        terminated_now(p, "migrate", code, reason);
    })
}

/// **The one shape whose [`Finding::owner`] is not its [`Finding::object`]** — a real capture of
/// a Deployment's pod, so the heading [`whole_card`] now compares has something to separate it
/// from the object underneath (NOTES § D29, § D96). Every other base here is a bare pod, where
/// the two are equal and a reader that rewrote either would be invisible.
///
/// `owned-pods.json` is a `List` rather than a single object, so this decodes it the way
/// [`a_finding_on_an_owned_pod_files_under_the_controller_and_not_the_pod`] does instead of
/// going through [`capture_but`].
///
/// **The capture is mid-backoff, so the run this shape is about is the *next* one.** The kubelet
/// starts `quitter` again — `startContainer` writes `RestartCount + 1`, and the backoff's own
/// record is already sitting in `lastState` as the run before it — and then it exits. The two
/// fields move together, the way every other plant in this file moves them (NOTES § D40);
/// [`terminated_now`] refuses a container that is neither running nor terminated rather than
/// inventing a moment for it, which is what caught the first draft of this shape.
fn owned_regular(code: i32, reason: Option<&str>) -> PodSnapshot {
    let mut object = items::<Pod>("owned-pods")
        .pop()
        .expect("the capture holds the Deployment's pod");
    let status = container_status(&mut object, "quitter");
    status.state = Some(ApiContainerState {
        running: Some(ContainerStateRunning {
            // After the `lastState` run's `finishedAt`, which is what makes this the run after it.
            started_at: Some(time("2026-08-13T23:31:10Z")),
        }),
        ..ApiContainerState::default()
    });
    status.restart_count += 1;
    terminated_now(&mut object, "quitter", code, reason);
    PodSnapshot::from(object)
}

/// One card, **whole — all eight fields of [`Finding`], counted against the struct and not
/// against the ones a card's body happens to print.** The equalities below are between cards and
/// not between titles: two cards that differ only in an age are two different cards, and an age
/// is precisely what a reader of `finishedAt` on this field would move
/// ([`Finding::timestamp`]).
///
/// **[`Finding::owner`] and [`Finding::object`] are in it because the heading is a card too.**
/// They were left out of the first draft, on the reading that the six below are *what a reader
/// sees* — and `card()` renders `owner` as the name at the top, so a reader of this field that
/// rewrote the owner would have moved a line on the screen with every equality here still green.
/// `tester` shipped exactly that mutation past 188 tests (NOTES § D26, § D96).
fn whole_card(f: &Finding) -> String {
    // Destructured rather than read field by field, so a **ninth** field on `Finding` stops this
    // file compiling instead of quietly falling outside every comparison in this section — which
    // is how `owner` and `object` came to be outside them.
    let Finding {
        severity,
        title,
        evidence,
        action,
        kubectl_cmd,
        owner,
        object,
        timestamp,
    } = f;
    format!(
        "{severity:?} | {title} | {evidence} | {action} | {kubectl_cmd:?} | {owner:?} | \
         {object:?} | {timestamp:?}"
    )
}

fn whole_cards(cards: &[&Finding]) -> Vec<String> {
    let mut out: Vec<String> = cards.iter().map(|f| whole_card(f)).collect();
    out.sort();
    out
}

/// **The ruling, as the suite can hold it: the cards a pod draws do not move when the run its
/// container is sitting in changes.**
///
/// Four properties, over every shape in [`SITTING`] and every ending in [`CURRENT_RUNS`]:
///
/// - **Every bad ending draws the identical set of cards** — **all eight fields** of each,
///   [`whole_card`], over **the whole pod** and not over the cards this test went looking for.
///   Two of the shapes below have a second container, so a reader that moved a card onto a
///   *sibling* is caught by the same equality; one has a **controller**, so a reader that moved
///   the heading is caught too (NOTES § D93, § D95, § D96).
/// - **The clean ending's set is a subset of it.** The one reader can *silence* a card and may
///   never write one, which is what makes it a suppressor rather than a subject.
/// - **A card does draw, at every row**, and where the container has a previous run one of them
///   is about *that* — so the silence is about the current run specifically and not about the
///   container (NOTES § D26).
/// - **The counts are written down** ([`Sitting`]), so a helper that stopped matching the
///   container's name cannot turn every equality above into `[] == []`.
///
/// And the cost this ruling accepts is pinned by name: on the gang shape the trigger's own
/// `exit 3` is nameable by nothing.
///
/// **The name of this test carried an absolute until rule 15, and it is now the conditional it
/// always was** (NOTES § D96, *what the ruling does not cover*). The ruling is about a container
/// that **can come back**, and every shape below is one: each is built on a capture whose pod
/// restarts its containers, so [`stopped_for_good`] cannot reach any of them. That exemption is
/// asserted inside the loop rather than left to the bases — a capture recaptured under a
/// different `restartPolicy` would otherwise turn this test's own subject into rule 15's and
/// nothing would say so.
#[test]
fn the_run_a_container_is_sitting_in_draws_no_card_while_something_will_restart_it() {
    for (label, role, name, build, clean_cards, bad_cards) in SITTING {
        let mut clean: Option<Vec<String>> = None;
        let mut bad: Option<(String, Vec<String>)> = None;
        for (code, reason) in CURRENT_RUNS {
            let planted = build(code, reason);
            let object = planted.id.name.clone();
            let subject = container(&planted, name).clone();
            assert_eq!(
                subject.role, role,
                "{label}: the role under test, or these seven shapes are fewer than seven"
            );
            let ContainerState::Terminated(run) = &subject.state else {
                panic!("{label}: the plant leaves {name} sitting in a terminated run: {subject:?}")
            };
            assert_eq!(
                (run.exit_code, run.reason.as_deref()),
                (
                    code,
                    Some(reason.unwrap_or(if code == 0 { "Completed" } else { "Error" }))
                ),
                "{label}: and it is sitting in *this* row's run — a plant that stopped writing \
                 the ending would make every equality below an equality between two copies of \
                 one object"
            );
            assert!(
                !finished(&planted),
                "{label}: the pod is not over, or no pod rule looks at it at all and the \
                 silence below is `analyze`'s skip rather than this ruling (NOTES § D2)"
            );
            // **Rule 15 is out of every shape here, and out of it by the field it gates on.** A
            // container something will restart is what this ruling is about; one that will not is
            // the exception D96 boxed and [`stopped_for_good`] now draws. Asserted over the whole
            // pod, because a card on a *sibling* would join the equalities below just as quietly.
            for other in &planted.containers {
                assert_ne!(
                    other.restart_policy.as_deref(),
                    Some("Never"),
                    "{label}: {} carries the effective `Never` that is rule 15's fourth \
                     condition, so it is that rule's subject and not this ruling's — the shapes \
                     here are containers something restarts (NOTES § D96)",
                    other.name
                );
            }

            // Read where this pod's cards draw: several of these shapes carry a *serving*
            // neighbour in rule 5's band, and at the pin that card has aged out of half the sets
            // being compared (NOTES § D100). The subject of this test is the terminated run, and
            // nothing about it moves with the clock.
            let moment = while_its_cards_draw(&planted);
            let all = analyze(&pods_at(vec![planted], moment));
            let about = cards_about(&all, name);
            println!(
                "=== {label} ({role:?} {name} on {object}) — exit {code} {reason:?}: {} cards, \
                 {} about {name}\n    {:?}",
                all.len(),
                about.len(),
                titles(&all)
            );
            // **The rules are alive on this object.** [`exit_fact`] reaches the screen from
            // rules 1, 5 and 6 only, so a card carrying it is a card some rule drew by reading
            // an ending — the very family this section says is silent about the current one.
            assert!(
                all.iter()
                    .any(|f| f.title.contains("exit ") || f.evidence.contains("exit ")),
                "{label} exit {code}: no rule that reads an ending drew anything on this pod, so \
                 the silence about {name}'s current run is a silence about everything: {:?}",
                titles(&all)
            );

            // **The whole pod's card set, not the subject's.** A reader of this field could
            // file its card under a sibling — the gang shape has one, and it is the container
            // the fan-out already draws about (NOTES § D95). The written-down count below is
            // what keeps [`cards_about`] honest at the same time.
            let whole = whole_cards(&all.iter().collect::<Vec<&Finding>>());
            let expected_about = if code == 0 { clean_cards } else { bad_cards };
            assert_eq!(
                about.len(),
                expected_about,
                "{label} exit {code}: {expected_about} cards about {name}: {:#?}",
                whole_cards(&about)
            );
            if code == 0 {
                clean = Some(whole);
            } else {
                match &bad {
                    None => bad = Some((format!("exit {code} {reason:?}"), whole)),
                    Some((first, expected)) => assert_eq!(
                        &whole, expected,
                        "{label}: exit {code} {reason:?} draws a different screen from {first} \
                         — the run a container is sitting in is no rule's subject, so nothing \
                         about how it ended may reach a card"
                    ),
                }
            }

            // **The silence is about the current run and not about the container.** Where there
            // is a previous run, some card names *it* — so a rule set that had gone quiet about
            // this container altogether could not pass the equalities above by drawing nothing
            // (NOTES § D26).
            //
            // **Not on the clean row, and that exception is the suppressor doing its job**: an
            // init container that finished takes rules 5 and 6 with it, which is the one thing
            // this field is allowed to decide ([`doing_its_job`]). The subset check below is
            // what holds that direction.
            match (code, &subject.last_terminated) {
                (0, _) | (_, None) => {}
                (_, Some(previous)) => {
                    let previous = exit_fact(previous);
                    assert!(
                        about
                            .iter()
                            .any(|f| f.title.contains(&previous) || f.evidence.contains(&previous)),
                        "{label} exit {code}: no card about {name} names its previous run \
                         ({previous}), so the rules have nothing to say about this container at \
                         all and the silence above is not the current run's: {:#?}",
                        whole_cards(&about)
                    );
                }
            }

            // **The cost, named on the shape that produced it** (NOTES § D93). The trigger's
            // own code is in this field and in no other, so on this row no card anywhere on
            // the pod can carry it — and the reader who wants to know which container went
            // first is not told.
            if code == 3 {
                show(&all);
                for f in &all {
                    let said = format!("{} {} {}", f.title, f.evidence, f.action);
                    assert!(
                        !said.contains("exit 3"),
                        "{label}: {name} exited 3 and this rule set does not read that field — \
                         a card naming it is a decision, and it is one this section rules \
                         against: {said}"
                    );
                }
            }
        }

        // Both halves of [`CURRENT_RUNS`] were reached — a table that lost its clean row, or its
        // bad ones, would make the subset below a comparison with nothing.
        let clean = clean.expect("the endings under test include a clean one");
        let (row, bad) = bad.expect("and at least one bad one");
        for card in &clean {
            assert!(
                bad.contains(card),
                "{label}: a clean ending draws a card that {row} does not. The one reader of \
                 this field is a *suppressor* — it may take rules 5 and 6 away from an init \
                 container that finished, and it may never put a card on the screen that a bad \
                 ending would not also have drawn: {card}"
            );
        }
        println!(
            "--- {label}: the screen is {} cards on a clean ending and {} on a bad one — \
             {:#?}",
            clean.len(),
            bad.len(),
            bad
        );
    }
}

/// **The one reader, asserted to be the only question it asks** — [`doing_its_job`] reads the
/// current terminated state to decide whether an *init* container finished, and nothing else
/// about the run reaches its answer (NOTES § D75, § D95).
///
/// The claim is an equality against the question rather than a table of expected booleans: the
/// answer is `role == Init && ending == Finished`, so `143` and `3` and the two synthesized
/// `137`s are all the same to it, and a `Regular` or `Sidecar` container sitting in this state
/// is *not doing its job* whatever its exit code says.
///
/// **What this test cannot say is that nobody else reads the field** — that is the test above,
/// which watches the cards rather than the function.
#[test]
fn the_one_reader_of_the_current_terminated_state_asks_only_whether_an_init_container_finished() {
    let mut answered = (0usize, 0usize);
    for (label, role, name, build, _, _) in SITTING {
        for (code, reason) in CURRENT_RUNS {
            let planted = build(code, reason);
            let subject = container(&planted, name);
            let ContainerState::Terminated(run) = &subject.state else {
                panic!("{label}: the plant leaves {name} sitting in a terminated run")
            };
            let expected = role == ContainerRole::Init && ending(run) == Ending::Finished;
            println!(
                "{label} {role:?} exit {code} {reason:?}: ending {:?}, doing its job {}",
                ending(run),
                doing_its_job(subject)
            );
            assert_eq!(
                doing_its_job(subject),
                expected,
                "{label}: a container sitting in a terminated run is doing the job it was given \
                 only when it is an init container that finished — {:?} beside {role:?}",
                ending(run)
            );
            if expected {
                answered.0 += 1;
            } else {
                answered.1 += 1;
            }
        }
    }
    // Both answers are reached, or the equality above is satisfied by one constant
    // (CLAUDE.md § Code phase rules, *a derived list asserts it found something*).
    assert_eq!(
        answered,
        (2, 46),
        "the two init shapes on the clean row say yes and the other forty-six rows say no"
    );
}

/// **What the field actually holds on a real cluster, over the whole committed corpus** — the
/// haystack any reader of it would start from, and reasoning (1) and (2) of this section
/// checked rather than asserted about two files.
///
/// Three claims now, and the split between them is the ruling's first half plus the exception it
/// boxed (NOTES § D96, § D97, § D114):
///
/// - Inside a pod that is **not** over, every captured container in `state.terminated` either
///   **finished without an error**, or **has been restarted before** — **or it is the one object
///   captured to be a finding**. That last arm is `neverback.json`, committed 2026-08-15 to give
///   rule 15 a real shape, and it is named here rather than let through.
///
///   **The role dropped out of that claim and the ending carried it, and the difference is the
///   capture's.** Leg 2 read *an init container that finished* off two files; `neverback/done` is
///   a **Regular** container at `exit 0` on a pod that is still going — under `Never` that is the
///   policy doing exactly what it says (NOTES § D96, leg 7) — so the role half was a property of
///   the two captures that happened to exist and not of the field.
///
///   **Then the restart arm had to be added, and the same way: because a capture arrived**
///   (NOTES § D114). The claim was *nothing in this state is a fault unless somebody built one*,
///   and it was true of a corpus in which every crash loop happened to have been photographed in
///   `waiting: CrashLoopBackOff`. That is a coin-flip — `scripts/cluster.sh` § `[owned]` measured
///   `state.terminated` in 39 samples of 70 — and the 2026-08-16 trip photographed four of them
///   on the other face. `init/migrate`, `notfound/app` and `oom/hog` are all sitting in a failed
///   run right now, and all three are perfectly ordinary crash loops.
///
///   **What survives is sharper than what it replaces, and it is rule 15's own condition.** The
///   thing that separates *a container something is restarting* from *a container nothing will
///   start again* is `restarts != 0`, which is exactly the guard [`stopped_for_good`] uses — so
///   the field still puts no card on the screen that another rule was not already drawing, and
///   the one object that is neither is the one rule 15 exists for. The ruling did not move; the
///   corpus finally contains the case it was reasoning about.
/// - The captures that hold a **bad** exit in this field hold it on a pod that is **over**, and
///   [`analyze`] never looks at those (NOTES § D2) — proved, on these same two objects, by
///   [`a_pod_that_finished_is_charged_to_nobody_and_alarms_about_nothing`], which is why the
///   skip is cited here and not re-asserted.
/// - **And the exception has to be exactly one object.** `neverback.json` is named as a whole
///   `pod/container` string, not waved through as *any Regular container that draws a card*, so a
///   second capture landing in this state reddens this test the way the first one did — which is
///   the only reason the narrowing is not a weakening (CLAUDE.md § Code phase rules).
///
/// **A derived list, so it asserts what it found** — the sweep names every container rather
/// than counting them, or a decode that stopped producing [`ContainerState::Terminated`] would
/// print the same green line as a corpus with nothing in it.
#[test]
fn every_captured_container_sitting_in_a_terminated_run_is_healthy_or_is_the_captured_finding() {
    let mut serving: Vec<String> = Vec::new();
    let mut over: Vec<String> = Vec::new();
    let (mut mid_loop, mut clean_endings) = (0usize, 0usize);
    for name in CAPTURED_PODS {
        let p = pod(name);
        for c in &p.containers {
            let ContainerState::Terminated(run) = &c.state else {
                continue;
            };
            let seen = format!(
                "{name}/{} ({:?}, {}, {} restarts) {}",
                c.name,
                c.role,
                p.phase.as_deref().unwrap_or("no phase"),
                c.restarts,
                exit_fact(run)
            );
            if finished(&p) {
                over.push(seen);
            } else {
                // The one capture taken *because* this state is a finding, named as one exact
                // object. Anything else in this state on a pod that is still going either
                // finished cleanly or is mid-loop — and mid-loop is `restarts != 0`, which is
                // rule 15's own guard and the reason no card comes off this field that another
                // rule was not already drawing.
                if format!("{name}/{}", c.name) != "neverback/broke" {
                    assert!(
                        ending(run) == Ending::Finished || c.restarts != 0,
                        "{seen}: a container sitting in a terminated run inside a live pod has \
                         either finished without an error or been restarted before — the second \
                         is an ordinary crash loop caught between runs, and the rules that own it \
                         read `lastState` and the count. A container that failed its *first* run \
                         and is still there is rule 15's subject, and there is exactly one of \
                         those in the corpus (NOTES § D96, § D97, § D114)"
                    );
                }
                if ending(run) == Ending::Finished {
                    clean_endings += 1;
                } else if c.restarts != 0 {
                    mid_loop += 1;
                }
                serving.push(seen);
            }
        }
    }
    println!(
        "inside a pod that is still going:\n  {}",
        serving.join("\n  ")
    );
    println!("inside a pod that is over:\n  {}", over.join("\n  "));
    assert_eq!(
        serving,
        [
            "healthy-retry/wait-for-db (Init, Running, 3 restarts) exit 0 (the run ended without \
             an error)",
            "healthy/migrate (Init, Running, 0 restarts) exit 0 (the run ended without an error)",
            "init/migrate (Init, Pending, 10 restarts) exit 1 (the application's own error)",
            "neverback/broke (Regular, Running, 0 restarts) exit 1 (the application's own error)",
            "neverback/done (Regular, Running, 0 restarts) exit 0 (the run ended without an \
             error)",
            "neverrules/retry (Regular, Running, 1 restarts) exit 1 (the application's own error)",
            "notfound/app (Regular, Running, 10 restarts) exit 127 (the command was not found)",
            "oom/hog (Regular, Running, 10 restarts) exit 137 (killed by the kernel for using \
             more memory than it was allowed)",
        ],
        "every capture that carries the field inside a pod that is still going, named — a sweep \
         that found nothing prints the same line as one that found nothing wrong. **`done` is on \
         this list and is not the exception**: it exited `0`, which under `Never` is the policy \
         doing what it says, so it is the healthy shape reached by a third road (NOTES § D96, \
         leg 7). **The four with a bad ending and a count are the 2026-08-16 trip's**, and they \
         are the case the ruling was always reasoning about (NOTES § D114)"
    );
    // **Both arms of the claim are reached**, or the `||` above is satisfied by one constant and
    // the restart arm is a branch nothing takes (CLAUDE.md § Code phase rules). Counted off the
    // objects rather than off the strings above — a first draft matched `"0 restarts"` as a
    // substring and scored `10 restarts` as a first run.
    println!(
        "{} mid-loop with a bad ending, {} clean",
        mid_loop, clean_endings
    );
    assert!(
        mid_loop > 0 && clean_endings > 0,
        "the corpus has to hold both a container something is restarting after a bad run and one \
         that simply finished, or the two arms of this claim are not both being asked: {serving:?}"
    );
    assert_eq!(
        over,
        [
            "failed/app (Regular, Failed, 4 restarts) exit 137 (Kubernetes lost track of the \
          container and wrote this code in its place)",
            "succeeded/migrate (Regular, Succeeded, 3 restarts) exit 0 (the run ended without an \
             error)"
        ],
        "and the only bad exit this field holds anywhere in the corpus is on a pod that is over"
    );
    for name in ["healthy", "healthy-retry"] {
        nothing(
            &findings(&[name]),
            "a finished init container is what this field looks like on a pod that is working, \
             and nothing about it is a finding",
        );
    }
    // **And the exception is a card, or the arm above exempts a shape nothing draws on.** One
    // card off a pod carrying two containers in this state: `broke` is the finding and `done`'s
    // clean exit is the silence beside it (NOTES § D96, leg 7).
    let drawn = findings(&["neverback"]);
    show(&drawn);
    assert_eq!(
        drawn
            .iter()
            .map(|f| f.title.as_str())
            .collect::<Vec<&str>>(),
        [STOPPED_FOR_GOOD],
        "the one capture exempted above is exempt because a rule reads it, not because a claim \
         was widened to let it past"
    );
}
// --- THE RUN A CONTAINER IS SITTING IN RIGHT NOW END ---

// --- RULE 15: THE CONTAINER THAT HAS STOPPED FOR GOOD START ---
//
// **The exception the section above boxed** (NOTES § D96): a container stopped in the run it is
// sitting in **now**, inside a pod that is *not* over, which nothing will start again.
//
// **One committed capture draws this card and the rest of the section is plants** — `neverback`,
// taken for exactly this, and the claim is asserted rather than assumed by
// [`one_committed_capture_holds_containers_nothing_will_restart_and_it_is_the_one_taken_for_this`],
// which sweeps the whole corpus for the title. Two more pods arrived under `Never` on 2026-08-16
// and neither draws: `gang` is running and `neverrules/retry` has been restarted once, which is
// the guard [`the_restarted_container_is_not_one_nothing_will_restart`] reads (NOTES § D114).
//
// **Four conditions, and each is moved on its own against one base**, so a silence is always
// attributable to the condition the row names and never to the object drifting underneath
// ([`first_run_under`], NOTES § D29).

/// **Rule 15's title, spelled once** — every assertion below asks for this string or for its
/// absence, and a rule whose wording moved would otherwise turn the negatives green by making
/// them match nothing (NOTES § D26).
const STOPPED_FOR_GOOD: &str = "This container has stopped and nothing is starting it again";

/// **Which captures this rule can reach, checked instead of stated** — the sentence that
/// justifies every plant below, and it has been wrong three times now.
///
/// It said every capture was taken under `restartPolicy: Always`; two are not (`failed.json` and
/// `succeeded.json` are `OnFailure`). Then `neverback.json` landed and it became *exactly one
/// pod is under `Never`*. Then the 2026-08-16 capture trip landed `neverrules.json` and
/// `gang.json` and that was false too. **The claim that survives is about the cards, not about
/// the policy**: three committed pods sit under a policy that will not restart their containers,
/// and **exactly one container in the whole corpus draws rule 15's card** — `neverback/broke`,
/// the capture taken to be its positive.
///
/// **The other six are the rule's guards doing work on real bytes**, which is what makes them
/// worth naming here rather than counting:
///
/// - `gang/trigger`, `gang/bystander`, `neverrules/keeper` are **`Running`** — the first
///   condition, and no card.
/// - `neverback/done` **exited 0** — the [`Ending::Finished`] arm, deliberately silent.
/// - `neverback/keeper` is running.
/// - **`neverrules/retry` is the one this trip was for.** It is `Terminated` at `exit 1` under a
///   container-level `Never`, which is every condition of this rule but one — and it has been
///   restarted once, so `restarts != 0` refuses it. Until this capture that guard had no
///   committed object at all and only a plant said it worked
///   ([`the_restarted_container_is_not_one_nothing_will_restart`] asserts the *why*).
///
/// Everything else this section proves — `OnFailure`, the pruned field, a sidecar, an init
/// container — still has no capture and is built (NOTES § D40, § D97, § D114).
///
/// **It asserts what it found**, both ways: the whole per-container map is compared, so a decode
/// that started answering `None` everywhere fails here rather than passing with an empty sweep
/// (CLAUDE.md § Code phase rules).
#[test]
fn one_committed_capture_holds_containers_nothing_will_restart_and_it_is_the_one_taken_for_this() {
    let mut never: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut counted = 0usize;
    for name in CAPTURED_PODS {
        let p = pod(name);
        for c in &p.containers {
            if c.restart_policy.as_deref() == Some("Never") {
                never.push(format!("{name}/{}", c.name));
            }
            let policy = format!("{:?}", c.restart_policy);
            if !seen.contains(&policy) {
                seen.push(policy);
            }
            counted += 1;
        }
    }
    seen.sort();
    never.sort();
    println!(
        "{counted} containers over {} captures: {seen:?}\n  under Never: {never:?}",
        CAPTURED_PODS.len()
    );
    assert_eq!(
        seen,
        ["Some(\"Always\")", "Some(\"Never\")", "Some(\"OnFailure\")"],
        "the policies the corpus actually holds — a sweep that read nothing, or a decode that had \
         stopped filling the field, prints the same green line as one with nothing wrong"
    );
    assert_eq!(
        never,
        [
            "gang/bystander",
            "gang/trigger",
            "neverback/broke",
            "neverback/done",
            "neverback/keeper",
            "neverrules/keeper",
            "neverrules/retry",
        ],
        "three captures and every container of each, named. A fourth pod arriving under `Never` \
         is a shape the plants below stopped being the only proof of, and it has to redden this \
         line rather than slip in behind a count"
    );

    // **And the claim that actually justifies the plants**: of those seven, exactly one draws the
    // card. Counting policies alone stopped being the interesting number the moment a second and
    // third `Never` pod landed — what the section below needs is that no committed object reaches
    // rule 15 except the one taken for it, and that is asserted over the whole corpus rather than
    // inferred from the list above.
    let drew: Vec<String> = analyze(&fixture_snapshot())
        .iter()
        .filter(|f| f.title == STOPPED_FOR_GOOD)
        .map(|f| format!("{}/{}", f.object.name, f.evidence))
        .collect();
    assert_eq!(
        drew.len(),
        1,
        "one committed container draws rule 15's card and the other six are refused by a \
         condition each — a second card here is a capture that quietly became this rule's \
         positive without anybody choosing it: {drew:?}"
    );
    assert!(
        drew[0].starts_with("broken-neverback/container broke"),
        "and it is the capture taken for this rule: {drew:?}"
    );
}

/// **Rule 15's `restarts != 0` guard, on the first committed object that reaches it**
/// (NOTES § D114). `neverrules/retry` is `Terminated` at `exit 1` under a container-level
/// `Never` inside a `Never` pod — every condition of rule 15 but the restart count, which is
/// `1` because the container's own `restartPolicyRules` restarted it once on `exit 3`.
///
/// **Why the guard is not redundant with the policy**, which is the thing this object proves and
/// a plant only asserted: the API let a container be `Never` *and* carry a rule that restarts it,
/// so *nothing is starting it again* is false about an object whose policy says `Never`. The
/// count is the only field that knows.
///
/// The silence is asserted **and attributed**: the same capture with the count moved to `0`
/// draws the card, so a green here is the guard working rather than some other condition
/// refusing first (NOTES § D29).
#[test]
fn the_restarted_container_is_not_one_nothing_will_restart() {
    let raw = fixture("neverrules");
    let captured = captured_status(&raw, "containerStatuses", "retry");
    assert_eq!(
        captured_str(captured, &["state", "terminated", "reason"]),
        "Error",
        "the container is stopped in the run it is sitting in, which is rule 15's first \
         condition — a capture that moved on is not the fixture for this"
    );
    assert_eq!(
        captured_i32(captured, &["restartCount"]),
        1,
        "and it has been restarted exactly once, which is the only condition of rule 15 this \
         object fails — read off the capture, because the count belongs to the cluster"
    );
    let declared = &raw["spec"]["containers"]
        .as_array()
        .expect("the capture declares its containers")
        .iter()
        .find(|c| c["name"] == "retry")
        .expect("the capture declares retry")["restartPolicy"];
    assert_eq!(
        declared, "Never",
        "and the policy is the container's own, not the pod's fallback — which is what makes \
         *nothing is starting it again* a claim the policy alone would have got wrong: {declared}"
    );

    let all = findings(&["neverrules"]);
    show(&all);
    assert!(
        !titles(&all).contains(&STOPPED_FOR_GOOD),
        "a container something has already restarted once is not one nothing will restart, and \
         the card would tell the reader to replace a pod that is retrying by design: {:?}",
        titles(&all)
    );

    // The attribution: one field moved, and the card the capture is refused appears.
    let never_restarted = capture_but("neverrules", |p| {
        container_status(p, "retry").restart_count = 0;
    });
    let then = analyze(&pods_at(vec![never_restarted], now()));
    show(&then);
    assert!(
        titles(&then).contains(&STOPPED_FOR_GOOD),
        "with the count at zero the same bytes draw the card — so the silence above is the \
         restart guard and not another condition refusing first: {:?}",
        titles(&then)
    );
}

/// **Rule 15 against the object a cluster actually wrote** — `neverback.json`, captured
/// 2026-08-15 on kind v1.36.1 for this rule and verified by `cluster.sh verify` before it was
/// sanitized (NOTES § D97). Three regular containers under pod `restartPolicy: Never`, in a pod
/// still `Running`: `broke` stopped at `exit 1`, `done` stopped at `exit 0`, `keeper` up.
///
/// **What this capture proves and what it does not, said rather than blurred** (NOTES § D40):
/// every container's own `spec.restartPolicy` is `null`, so the bytes prove the **pod-level
/// fallback** of [`ContainerSnapshot::restart_policy`] and nothing else. The **container-level
/// override** — a `Never` container inside an `Always` pod, measured sitting at `1/2 Error` with
/// `restartCount: 0` indefinitely — has no capture and is the plant in
/// [`the_effective_restart_policy_is_the_containers_own_and_then_the_pods`]. That override is the
/// leg that makes this field the *container's* rather than the pod's, so which of the two is
/// captured and which is built matters and is written down here.
///
/// **`done` is the clean-exit negative on the same object**, which is what makes the silence
/// leg 7 rules a property of a real pod and not of a plant: one card off a pod carrying two
/// containers stopped in the run they are sitting in.
#[test]
fn the_captured_pod_that_stopped_for_good_draws_one_card_and_the_clean_exit_beside_it_draws_none() {
    let raw = fixture("neverback");
    let p = pod("neverback");
    assert_eq!(
        raw["spec"]["restartPolicy"], "Never",
        "read off the capture's own bytes, because the value belongs to the cluster and not to \
         this file"
    );
    let declared: Vec<&serde_json::Value> = raw["spec"]["containers"]
        .as_array()
        .expect("the capture declares its containers")
        .iter()
        .map(|c| &c["restartPolicy"])
        .collect();
    assert!(
        declared.len() == 3 && declared.iter().all(|v| v.is_null()),
        "all three containers, and not one of them declares its own policy — which is what makes \
         this capture proof of the pod-level fallback and of nothing else: {declared:?}"
    );
    for (name, policy, restarts, state) in [
        ("broke", "Never", 0, "terminated exit 1"),
        ("done", "Never", 0, "terminated exit 0"),
        ("keeper", "Never", 0, "running"),
    ] {
        let c = container(&p, name);
        println!("{state}: {c:?}");
        assert_eq!(
            (c.role, c.restart_policy.as_deref(), c.restarts),
            (ContainerRole::Regular, Some(policy), restarts),
            "{name}: the effective policy this pod's containers inherit, and a count the policy \
             makes permanent"
        );
    }
    assert!(
        !finished(&p) && p.phase.as_deref() == Some("Running"),
        "`keeper` is what keeps this pod out of the terminal phase, and that is the whole reason \
         the shape exists at all: a single-container pod of it goes `Failed` and leaves this \
         screen (NOTES § D96)"
    );

    let all = findings(&["neverback"]);
    show(&all);
    let card = only(&all, "broken-neverback", STOPPED_FOR_GOOD);
    // **One card per *failed* stopped container, which is not the same claim as one per pod.**
    // Two failing helpers under `Never` beside a runner is two CRITICALs, correctly — the rule is
    // per container like rules 1–7. What this object proves is narrower and is the whole of what
    // it was captured for: it holds **two** containers stopped in the run they are sitting in, and
    // only the one that failed draws (NOTES § D96 leg 7, § D97).
    assert_eq!(
        all.len(),
        1,
        "two containers of this pod are stopped for good and one card is drawn — `done` exited 0, \
         which under `Never` is the policy working. Not a claim that this rule draws once per pod"
    );
    assert_eq!(card.severity, Severity::Critical);
    assert_eq!(
        card.evidence,
        "container broke · exit 1 (the application's own error) · ran for under a second",
        "the container that failed, and not the one beside it that finished"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl logs broken-neverback -c broke -n default"),
        "the command measured on the cluster this came off: with the pod still there, the \
         container's log is served with no --previous and no error"
    );
    // **And why it is served, read off the bytes rather than remembered from a cluster.** The
    // kubelet gates a container's log on its `containerID`, which is the same gate rule 1's
    // `Unwatched` arm records as *missing* — a synthesized status carries none, so `logs
    // --previous` is refused there and that arm may name no log at all. This capture carries one,
    // in `state.terminated`, which is exactly the run the card sends the reader to.
    assert!(
        at(&raw, &["status", "containerStatuses"])[0]["state"]["terminated"]["containerID"]
            .as_str()
            .is_some_and(|id| id.starts_with("containerd://")),
        "no containerID on the run this card names a log for, and the API would refuse to serve \
         it: {}",
        at(&raw, &["status", "containerStatuses"])[0]["state"]["terminated"]
    );
    // **The moment, not the rendered age** (NOTES § D18, § D69). The phrase is a function of the
    // pin as well as of the rule — this card drew *no age at all* until `fn now()` was repinned in
    // the same change that added the capture, and it reads `12 hours ago` now — so a test written
    // against the phrase would move every time the corpus is recaptured, while asserting nothing
    // this rule decides. The field is the claim: when *this* run finished, off `state.terminated`
    // and not off a `lastState` this container has never had.
    // Read out of the capture rather than transcribed from it: the literal that used to sit here
    // was this cluster's `finishedAt` and reddened on the next trip, which is the same defect the
    // paragraph above describes one level down (NOTES § D114).
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            at(
                captured_status(&raw, "containerStatuses", "broke"),
                &["state", "terminated"]
            ),
            &["finishedAt"]
        )),
        "the run the card is about ended here, and the bytes say so — `state.terminated`, and \
         not a `lastState` this container has never had"
    );
    // The clean-exit silence, on the object rather than on a plant (NOTES § D96, leg 7).
    let done = container(&p, "done");
    assert!(
        matches!(&done.state, ContainerState::Terminated(run) if ending(run) == Ending::Finished),
        "`done` is stopped in the run it is sitting in too, so the silence about it is this \
         rule's exemption and not a container the rule never reached: {done:?}"
    );
    assert!(
        stopped_for_good(&p, done).is_none(),
        "a container that exits 0 under `Never` is the policy doing exactly what it says; the \
         fault a reader would want named there is the Job above the pod, and Jobs are not \
         watched (invariant 6)"
    );
}

/// **The two promises this card made until the operator review, and may never make again** —
/// each measured false on a review cluster rather than argued down (NOTES § D97).
///
/// - *its log is still there.* [`logs`] is the only command in this file that goes to the
///   **kubelet on the node**; every condition rule 15 fires on is read from a pod status that
///   **freezes when that kubelet dies**. Measured: kubelet stopped, and eight minutes later the
///   card was unchanged while its own command answered `connection refused`. The corridor D88
///   names, built by a card that promised a door.
/// - *nothing will run it again inside this pod.* False in the one shape the effective-policy
///   key exists for: pod `Always` with a container's own `Never`, node rebooted, `restartCount`
///   0 → 1 → 4, because the kubelet reads the **pod's** policy when it rebuilds a sandbox.
///
/// **The phrases are fragments rather than whole sentences**, so a rewording that keeps the
/// promise still trips them — the hedge-shaped escape NOTES § D95 recorded, where eight appended
/// words walked past three `contains` checks.
///
/// **This list had the framing hole it exists to catch, and shipped with it** (NOTES § D31,
/// § D97). Its first draft was `Nothing will run it again` / `nothing will run it again` — the
/// sentence the *action* used to carry, in both capitalisations — while the **title** said
/// *nothing **will start** it again* and went straight through, because the list was fed the words
/// that had been written rather than the words that were there. Two corrections, and they fix
/// different halves:
///
/// - **The verb, not one verb.** `run`, `start` and `restart` all say it, so a fragment covers
///   each. Dropping the leading `Nothing`/`nothing` makes them case-independent *and* catches the
///   rewordings a capitalised fragment cannot — *something will start it again*, *nothing else
///   will restart it*.
/// - **The field, not one field.** The sweep read the action alone; a prediction in the title or
///   the evidence is the same false claim on the same card, and the title is the line the reader
///   reads first. It walks all three now — D31's *where inside the value* half, where the list
///   above is the *which words* half.
///
/// **Its control is synthetic and that is the honest form here** (CLAUDE.md § Code phase rules):
/// these are words the card may **never** say, so a control drawn from the product would mean the
/// defect had shipped. [`the_detector_for_the_two_promises_fires_on_each_of_them`] proves the
/// check instead.
const PROMISES_THIS_CARD_MAY_NOT_MAKE: [&str; 5] = [
    "still there",
    "will run it again",
    "will start it again",
    "will restart",
    "only a new pod",
];

/// **The detector above, proved to fire** — every phrase planted into every line of a real card.
/// Without this, a list of `contains` checks against strings that never contained them is a row of
/// green lines proving nothing (NOTES § D26).
///
/// **Both axes, because the hole was in the second one.** Each phrase is planted, and it is
/// planted into the title, the evidence and the action in turn: a list that has the right words
/// and is only ever pointed at one line of the card is the defect this whole guard shipped with
/// (NOTES § D31, § D97). The card is the one carrying the container's last words, so all three
/// lines are non-empty and every plant lands on real text.
#[test]
fn the_detector_for_the_two_promises_fires_on_each_of_them() {
    let all = analyze(&pods_at(vec![stopped_saying(1)], now()));
    let card = only(&all, "broken-hostpath", STOPPED_FOR_GOOD);
    for (line, real) in [
        ("title", &card.title),
        ("evidence", &card.evidence),
        ("action", &card.action),
    ] {
        println!("the {line} as shipped: {real}");
        assert!(
            !real.is_empty(),
            "the {line} is empty, so planting into it proves nothing"
        );
        for promise in PROMISES_THIS_CARD_MAY_NOT_MAKE {
            assert!(
                !real.contains(promise),
                "{promise:?} is in the shipped {line}, which is the defect this list exists to \
                 stop"
            );
            let regressed = format!("{real} — {promise}");
            assert!(
                regressed.contains(promise),
                "{promise:?} planted into the {line} and the check did not see it, so the sweep \
                 next door is a comparison against nothing"
            );
        }
    }
}

/// **A captured pod whose containers have never been restarted, under the policy the row is
/// about** — `broken-hostpath` is the only committed capture with two regular containers, which
/// is what lets one stop while the other keeps the pod at `phase: Running`.
///
/// **The counts and the previous runs move with the policy rather than beside it.** Nothing under
/// `Never` has ever been restarted, so the capture's `restartCount: 1` and its `lastState` are
/// records that pod cannot hold — and the count is precisely what rule 15's guard reads, so a
/// plant that left them would prove a silence for the wrong reason (NOTES § D40). They are
/// cleared on **every** row, `Always` included, so the policy is the only thing that differs
/// between a positive and its negative.
///
/// `None` is the pruned field, and it is a row rather than an oversight: the API server defaults
/// this on every accepted create, so an absent one is a promise nobody made.
fn first_run_under(policy: Option<&str>, edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    capture_but("hostpath", |p| {
        p.spec
            .as_mut()
            .expect("the capture has a spec")
            .restart_policy = policy.map(str::to_string);
        let status = p.status.as_mut().expect("the capture has a status");
        for c in status
            .init_container_statuses
            .iter_mut()
            .chain(status.container_statuses.iter_mut())
            .flatten()
        {
            c.restart_count = 0;
            c.last_state = None;
        }
        edit(p);
    })
}

/// The shape itself: `shipper` stopped in the run it is sitting in, `nosy` still up — which is
/// what keeps the pod `Running` and out of [`finished`]'s door.
fn stopped_under(policy: Option<&str>, code: i32, reason: Option<&str>) -> PodSnapshot {
    first_run_under(policy, |p| terminated_now(p, "shipper", code, reason))
}

/// **The same shape with the kubelet's termination message beside it** — what
/// `terminationMessagePolicy: FallbackToLogsOnError` leaves on a container that exits for good,
/// and the one fact on this card that lives in the **API server** rather than on the node
/// (NOTES § D97).
///
/// **The message is a real one, off `crashloop.json`**, rather than a length this file chose: what
/// the clause costs in card height is decided by what a program actually prints when it dies, and
/// a short invented string would measure a card nobody gets (NOTES § D40).
fn stopped_saying(code: i32) -> PodSnapshot {
    let real = fixture("crashloop")["status"]["containerStatuses"][0]["lastState"]["terminated"]
        ["message"]
        .as_str()
        .expect("the captured crash loop kept its container's last words")
        .to_string();
    first_run_under(Some("Never"), |p| {
        terminated_now(p, "shipper", code, None);
        container_status(p, "shipper")
            .state
            .as_mut()
            .and_then(|s| s.terminated.as_mut())
            .expect("terminated_now just wrote this run")
            .message = Some(real);
    })
}

/// **Every card rule 15 drew anywhere on the pod**, which is what the silences below are about
/// — `broken-hostpath` mounts `/` in both containers, so rule 8 is on this screen at every row
/// and is not this rule's business.
///
/// **Not narrowed to the container under test, deliberately.** A rule that fired about the
/// *sibling* is the same defect on the same screen, and [`cards_about`] would have walked past
/// it; the title is this rule's alone, so the whole list is the honest filter.
fn stopped_cards(all: &[Finding]) -> Vec<&Finding> {
    all.iter().filter(|f| f.title == STOPPED_FOR_GOOD).collect()
}

/// **The card, on both framings the pipeline hands this rule.** An ordinary bad exit, and the
/// memory kill — which reaches rule 15 rather than rule 2, because [`out_of_memory`] reads
/// `lastState` and a container that was never restarted has none (NOTES § D96).
///
/// **The command is the first `kubectl logs` in `rules.rs` and the assertion is an equality**,
/// not a `contains`: the container is named with `-c`, the namespace with `-n`, and there is no
/// `--previous` — a flag that would send this reader to a run that never happened, since the run
/// the card is about is the one the container is still sitting in ([`logs`], invariant 4).
#[test]
fn a_container_stopped_for_good_inside_a_running_pod_draws_a_card_that_names_its_log() {
    for (framing, code, reason, meaning, ended, promises) in [
        (
            "an ordinary bad exit",
            1,
            None,
            "the application's own error",
            Ending::Failed,
            // The run failed and the application is what failed, so its own log is where the
            // answer is — the sentence this rule has always drawn.
            true,
        ),
        (
            "a memory kill nothing else can see",
            137,
            Some("OOMKilled"),
            "killed by the kernel for using more memory than it was allowed",
            Ending::Failed,
            true,
        ),
        // **The third framing, and the one whose ending is not `Failed`.** A node restart leaves
        // the container found dead with no code anybody read — it *has* stopped, and under
        // `Never` at zero restarts nothing is starting it again, so this rule's title holds
        // where [`Unwatched`](Ending::Unwatched)'s and [`RestartRule`](Ending::RestartRule)'s do
        // not. What the reader gets that they did not before is the evidence line: the number is
        // named as a stand-in instead of being printed bare under *it stopped*.
        // **Both runtimes' spellings of the same ending**, because the translation differs per
        // code even where the ending does not, and this rule prints it (NOTES § D29).
        (
            "CRI-O, where the code could not be read at all",
            -1,
            Some("Error"),
            "the node could not tell what code the container ended with, so this number stands in",
            Ending::CodeUnknown,
            false,
        ),
        (
            "a node restart, where the code is a stand-in",
            255,
            Some(CODE_UNKNOWN),
            "the node found the container dead, so this number stands in for a code nobody read",
            Ending::CodeUnknown,
            // **And here the same sentence would be a false promise.** Nobody read how that run
            // ended, so the log holds no *why* to send anyone after — the evidence line one row
            // up says as much, and a card may not contradict itself between two of its lines
            // (NOTES § D85). The clause is derived from what the record carries, not copied off
            // the rows above: it shipped copied for one turn, because this ending was folded into
            // the `Failed` arm and inherited its wording.
            false,
        ),
    ] {
        let p = stopped_under(Some("Never"), code, reason);
        let c = container(&p, "shipper");
        println!("=== {framing}: {c:?}");
        // The four conditions, read off the object before any card is. A plant that stopped
        // building one of them would make the card below the rule firing on something else.
        assert!(
            matches!(&c.state, ContainerState::Terminated(run) if ending(run) == ended),
            "{framing}: the run it is sitting in, and the ending rule 15 gates on: {c:?}"
        );
        assert_eq!(
            (c.restarts, c.restart_policy.as_deref(), c.role),
            (0, Some("Never"), ContainerRole::Regular),
            "{framing}: never restarted, under the one policy that reaches this rule, on the one \
             role that can be under it: {c:?}"
        );
        assert!(
            !finished(&p) && p.phase.as_deref() == Some("Running"),
            "{framing}: the pod is still going — `analyze` skips one that is over, and that skip \
             is what makes a single-container pod of this shape unreachable (NOTES § D96)"
        );

        let all = analyze(&pods_at(vec![p.clone()], now()));
        show(&all);
        let card = only(&all, "broken-hostpath", STOPPED_FOR_GOOD);
        assert_eq!(
            card.severity,
            Severity::Critical,
            "{framing}: D2 asks whether this container is serving now, and the answer is no and \
             will stay no"
        );
        assert!(
            card.evidence.contains("container shipper")
                && card.evidence.contains(&format!("exit {code} ({meaning})")),
            "{framing}: which container and what the code means: {}",
            card.evidence
        );
        // **How long it lasted, on the endings whose `finishedAt` measures the run.** On
        // [`CodeUnknown`](Ending::CodeUnknown) containerd stamps that field when it recovers, so
        // the duration is the node's outage and [`ran_for`] refuses it — the same `promises`
        // split, because both halves come from the same fact about the record.
        assert_eq!(
            card.evidence.contains("ran for"),
            promises,
            "{framing}: a duration is claimed exactly where the record measures one: {}",
            card.evidence
        );
        assert_eq!(
            card.kubectl_cmd.as_deref(),
            Some("kubectl logs broken-hostpath -c shipper -n default"),
            "{framing}: the container is named, the namespace is named, and there is no \
             --previous — the log this card sends the reader to is the current run's"
        );
        // **The two clauses that are true on every ending this rule draws.** A finding whose
        // last line only says what will not happen has two parts of the three (NOTES § D97).
        assert!(
            card.action.contains("has to be replaced") && card.action.contains("still without it"),
            "{framing}: what the reader has to do, and what is broken until they do it: {}",
            card.action
        );
        // **And the clause that is the ending's.** The log is named either way — it is what this
        // rule's own command serves — but *that is where it says why it stopped* is a promise
        // only a run somebody watched end can keep.
        assert_eq!(
            card.action.contains("that is where it says why it stopped"),
            promises,
            "{framing}: whether this card may promise the log answers *why*, read off the record \
             rather than off the row above it: {}",
            card.action
        );
        assert!(
            card.action.contains("read its log"),
            "{framing}: the log is still where the container's own side of it is, and this \
             rule's command is the one `kubectl logs` in the file: {}",
            card.action
        );
        if !promises {
            assert!(
                card.action.contains("the node wrote that number")
                    && card.action.contains("not the app"),
                "{framing}: what the object does support in its place — where the number came \
                 from, which is the same thing the evidence line says: {}",
                card.action
            );
        }
        // **And the card it is drawn on has room for it** (`screens/alerts.md` § How tall).
        // The title is two lines at 51 columns and the evidence is cut at three, so the action
        // has four — an ending-aware clause that says the same thing at twice the length draws a
        // twelve-line card in a sixteen-row pane, which is a `rules.rs` finding and not a layout
        // problem. Measured on both endings, because the arm this rule gained is the one that
        // grew.
        let lines = wrapped_at(&card.action, ACTION_COLUMNS);
        println!(
            "{framing}: {} action lines at {ACTION_COLUMNS} columns",
            lines.len()
        );
        assert!(
            lines.len() <= 4,
            "{framing}: {} lines, and this card has room for four once its title and its cut \
             evidence are on it: {:?}",
            lines.len(),
            card.action
        );
        // **The card, whole.** A prediction in the title is the same false claim as one in the
        // action, and the title is the line the reader reads first (NOTES § D31, § D97).
        for (line, text) in [
            ("title", &card.title),
            ("evidence", &card.evidence),
            ("action", &card.action),
        ] {
            for promise in PROMISES_THIS_CARD_MAY_NOT_MAKE {
                assert!(
                    !text.contains(promise),
                    "{framing}: the {line} says {promise:?} — measured false on a review cluster \
                     (NOTES § D97): {text}"
                );
            }
        }
        // Read off the object the rule was handed rather than transcribed: the moment is
        // [`terminated_now`]'s, which derives it from the capture's own `startedAt`, so a
        // literal here is a date from whichever trip captured the base (NOTES § D114). The
        // *"not `lastState`"* half is structural — [`first_run_under`] clears that field, so
        // the assertion below is against the only run this container has.
        let ContainerState::Terminated(sitting_in) = &c.state else {
            panic!("{framing}: the plant builds a terminated run, asserted above");
        };
        assert_eq!(
            c.last_terminated, None,
            "{framing}: this container has never been restarted, so there is no `lastState` for \
             the age to come from by accident"
        );
        assert_eq!(
            card.timestamp, sitting_in.finished_at,
            "{framing}: the age is when *this* run finished, off the state the rule read — not \
             `lastState`, a run this container has never had (`Finding::timestamp`)"
        );
        // No policy word anywhere on the card: `kubectl logs` prints no part of the spec, and an
        // action may name only what its own command shows (invariant 4, NOTES § D88).
        let said = format!("{} {} {}", card.title, card.evidence, card.action);
        assert!(
            !said.contains("restartPolicy") && !said.contains("Never"),
            "{framing}: this card's command shows no spec, so it may not name a spec field: \
             {said}"
        );
    }
}

/// **The memory kill's other half: rule 2 is structurally silent on it, so rule 15's card is the
/// only one.** [`out_of_memory`] keys on `lastState.terminated.reason`, and a container that has
/// never been restarted has no `lastState` at all — so the kill reaches the reader through
/// [`exit_fact`] on this card or it reaches them nowhere (NOTES § D96).
#[test]
fn the_memory_kill_a_never_restarted_container_takes_is_on_this_card_or_on_none() {
    let p = stopped_under(Some("Never"), 137, Some("OOMKilled"));
    let c = container(&p, "shipper");
    assert!(
        c.last_terminated.is_none(),
        "the field rule 2 reads is empty, which is what makes it silent here rather than \
         exempt: {c:?}"
    );
    assert!(
        out_of_memory(&now(), &p, c).is_none(),
        "rule 2 cannot see this kill"
    );
    let all = analyze(&pods_at(vec![p.clone()], now()));
    show(&all);
    let card = only(&all, "broken-hostpath", STOPPED_FOR_GOOD);
    assert!(
        card.evidence.contains("more memory than it was allowed"),
        "so the kernel's own reading of 137 has to be on this card: {}",
        card.evidence
    );
}

/// **The container's own last words, on the card and ahead of the duration** (NOTES § D97).
///
/// `state.terminated.message` is populated for exactly the containers people set
/// `terminationMessagePolicy: FallbackToLogsOnError` on — the ones that exit for good — and this
/// rule read it nowhere until the operator review. **It is the one fact on the card that survives
/// a dead node**: the kubelet writes it into the API server, while the log the action points at
/// lives on the machine, and every condition this rule fires on is read from a status that
/// freezes when that machine goes.
///
/// **So the order is asserted and not incidental.** The evidence is cut at three lines
/// (`screens/alerts.md` § The height) and a real message costs a whole line, so on the shape this
/// test builds the *duration* is what the cut takes. That is the trade this rule wants: a clause
/// answering *why did it stop* over a clause answering *for how long did it run*. A plant that put
/// them the other way round would ship the reverse and measure the same height.
///
/// **The frame is [`last_words`], which rule 6 also prints**, so one fact is not worded two ways
/// on one screen (NOTES § D85).
#[test]
fn the_containers_last_words_are_on_the_card_ahead_of_how_long_it_ran() {
    let quiet = stopped_under(Some("Never"), 1, None);
    assert!(
        !only(
            &analyze(&pods_at(vec![quiet], now())),
            "broken-hostpath",
            STOPPED_FOR_GOOD
        )
        .evidence
        .contains("logged"),
        "a container that left no termination message gets no clause about one — the usual case, \
         and the control for the assertions below"
    );

    let speaking = stopped_saying(1);
    let run = match &container(&speaking, "shipper").state {
        ContainerState::Terminated(run) => run.clone(),
        other => panic!("the plant leaves shipper stopped: {other:?}"),
    };
    let said = last_log_line(&run).expect("and the plant gave it something to have said");
    println!("the kubelet kept: {said:?}");
    assert_eq!(
        said, "panic: dial tcp db.payments.svc:5432: connect: connection refused",
        "the **last** non-empty line of the captured message and not its first, which is \
         `starting` — a card printing the first line would say the container's last words were \
         that it had begun ([`last_log_line`])"
    );

    let all = analyze(&pods_at(vec![speaking], now()));
    let card = only(&all, "broken-hostpath", STOPPED_FOR_GOOD);
    println!("{}", card.evidence);
    assert!(
        card.evidence.contains(&last_words(said)),
        "the words are on the card, framed the way rule 6 frames the same field: {}",
        card.evidence
    );
    let words = card
        .evidence
        .find(QUOTE_FRAME)
        .expect("asserted present one line above");
    let ran = card
        .evidence
        .find("ran for")
        .expect("the plant's run has both stamps, so the duration is there to be ordered against");
    assert!(
        words < ran,
        "the message comes before the duration, because the evidence is cut at three lines and \
         the last clause is the one the reader loses: {}",
        card.evidence
    );
}

/// **Each of the four conditions, moved on its own** (NOTES § D29). Every row is the positive
/// with exactly one thing changed, and every row has to be silent — a rule that dropped a
/// condition would keep drawing its card here and nowhere else would notice.
#[test]
fn every_one_of_rule_fifteens_four_conditions_is_load_bearing() {
    for (why, planted) in [
        (
            "the container has not stopped at all — it is still running",
            first_run_under(Some("Never"), |_| {}),
        ),
        (
            "it exited 0, which under Never is the policy doing exactly what it says",
            stopped_under(Some("Never"), 0, None),
        ),
        (
            "it was asked to stop and did — exit 143 is a shutdown, not a fault",
            stopped_under(Some("Never"), 143, None),
        ),
        (
            "nothing watched the run end, so `it has stopped` is a claim the record cannot make",
            stopped_under(Some("Never"), 137, Some(STATUS_LOST)),
        ),
        (
            "the pod's own restart rule removed it, which is a restart already under way",
            stopped_under(Some("Never"), 137, Some(RESTART_ALL)),
        ),
        (
            // **The KEP false positive, and the reason the count is the guard.** A container may
            // declare `restartPolicyRules`, which can only *add* restarts to `Never` — measured
            // on kind v1.36.1, a retry rule on exit 3 had one in `CrashLoopBackOff` at five
            // restarts. The generated types carry that field at the `v1_36` feature `Cargo.toml`
            // pins — it arrives at `v1_34` — but **no cluster below 1.34 can carry it at all**,
            // and the pin sits above the cluster on purpose (NOTES § D99), so k8rs meets clusters
            // where reading the field answers nothing. `restarts == 0` is the guard that holds
            // across the whole range (NOTES § D97): something that has already been restarted has
            // something restarting it (NOTES § D96). **The open box that teaches rule 15 to read
            // the field keeps this case** — the count is a permanent companion to the field, not
            // a placeholder that box deletes.
            "it has been restarted once already, so something is restarting it",
            first_run_under(Some("Never"), |p| {
                // `ended_as` writes the previous run *and* bumps the count, which is the pair the
                // kubelet writes together — a count with no `lastState` beside it is a shape it
                // never produces (NOTES § D40).
                ended_as(p, "shipper", 3, None, None);
                terminated_now(p, "shipper", 1, None);
            }),
        ),
        (
            "the pod restarts everything, so this container is coming back",
            stopped_under(Some("Always"), 1, None),
        ),
        (
            "the pod restarts a bad exit, and this was one",
            stopped_under(Some("OnFailure"), 1, None),
        ),
        (
            "no policy was readable at all, and a missing field is not a licence to guess",
            stopped_under(None, 1, None),
        ),
    ] {
        let c = container(&planted, "shipper");
        let all = analyze(&pods_at(vec![planted.clone()], now()));
        println!("=== {why}\n    {c:?}\n    {:?}", titles(&all));
        assert!(
            stopped_cards(&all).is_empty(),
            "{why} — and rule 15 drew its card anyway: {:?}",
            titles(&all)
        );
    }
    // **The control, or every row above passes because the phrase matches nothing.** The same
    // base with none of the rows' changes does draw, so the silences are the conditions and not
    // a title that has moved out from under this test (NOTES § D26).
    let all = analyze(&pods_at(vec![stopped_under(Some("Never"), 1, None)], now()));
    assert_eq!(
        stopped_cards(&all).len(),
        1,
        "the base every row above is one edit away from has to draw the card: {:?}",
        titles(&all)
    );
}

/// **The effective policy is the container's own and then the pod's, and the two are separated on
/// one object** — a plant where the pod says `Never` and the sidecar says `Always`, so a decode
/// that read only the pod and one that read only the container both fail, in opposite directions
/// ([`ContainerSnapshot::restart_policy`]).
///
/// **The role is read off the same field and is deliberately not the same reading**: `proxy` is a
/// [`Sidecar`](ContainerRole::Sidecar) because of its *own* `Always`, and stays one under a pod
/// that restarts nothing — a derivation that had started using the effective value would turn
/// every init container in an `Always` pod into a sidecar, silently.
#[test]
fn the_effective_restart_policy_is_the_containers_own_and_then_the_pods() {
    // As captured: the pod says `Always` and no container overrides it.
    let plain = pod("hostpath");
    for c in &plain.containers {
        assert_eq!(
            c.restart_policy.as_deref(),
            Some("Always"),
            "{}: no container here declares one, so every one falls back to the pod's: {c:?}",
            c.name
        );
    }

    // The separating object: `healthy-unreadysidecar` declares `restartPolicy: Always` on
    // `proxy`, and the pod is put under `Never` beneath it.
    let split = capture_but("healthy-unreadysidecar", |p| {
        p.spec
            .as_mut()
            .expect("the capture has a spec")
            .restart_policy = Some("Never".to_string());
    });
    for (name, expected, role) in [
        ("proxy", Some("Always"), ContainerRole::Sidecar),
        ("app", Some("Never"), ContainerRole::Regular),
    ] {
        let c = container(&split, name);
        println!("{c:?}");
        assert_eq!(
            (c.restart_policy.as_deref(), c.role),
            (expected, role),
            "{name}: the container's own answer wins where it has one, and the pod's is the \
             fallback where it does not — and the role still reads the container's field alone"
        );
    }

    // And a spec with no policy on it anywhere, which is the pruned field rather than a value.
    let bare = first_run_under(None, |_| {});
    for c in &bare.containers {
        assert_eq!(
            c.restart_policy, None,
            "{}: nothing said what happens to this container, and `None` fires nothing: {c:?}",
            c.name
        );
    }
}

/// **Only a regular container can reach this rule, and both halves are by construction rather
/// than by a role check** (NOTES § D96, measured on kind v1.36.1).
///
/// - **A native sidecar's effective policy is its own `Always`**, so the fourth condition refuses
///   it out of the same field its role came from. Swept over the whole committed corpus, and then
///   built: a sidecar stopped in exactly rule 15's shape under a pod that says `Never` still
///   draws nothing.
/// - **A plain init container failing under pod `Never` takes the whole pod to `phase: Failed`**,
///   which leaves through [`finished`] before [`analyze`] reaches any container rule.
///
/// **The rule is asked directly for the sidecar half.** Going through [`analyze`] would let the
/// silence come from anywhere on the pod; `stopped_for_good` returning `None` on the container
/// itself is the claim.
#[test]
fn no_sidecar_and_no_init_container_can_reach_the_rule_about_a_container_nothing_is_starting() {
    // **The corpus half, and it asserts what it found**: a sweep over a decode that had stopped
    // producing `Sidecar` would print the same green line as one with nothing wrong.
    let mut swept = Vec::new();
    for name in CAPTURED_PODS {
        let p = pod(name);
        for c in &p.containers {
            if c.role != ContainerRole::Sidecar {
                continue;
            }
            assert_eq!(
                c.restart_policy.as_deref(),
                Some("Always"),
                "{name}/{}: a sidecar *is* an init container declaring `Always`, so its own \
                 answer is the effective one and it can never be `Never`: {c:?}",
                c.name
            );
            swept.push(format!("{name}/{}", c.name));
        }
    }
    swept.sort();
    println!("sidecars in the corpus: {swept:?}");
    assert_eq!(
        swept,
        ["healthy-sidecar/proxy", "healthy-unreadysidecar/proxy"],
        "both captured sidecars, named — a sweep that found none proves nothing"
    );

    // The built half: the sidecar put in rule 15's exact shape, under a pod that restarts
    // nothing. Its own `Always` is what keeps it out.
    let down = capture_but("healthy-unreadysidecar", |p| {
        p.spec
            .as_mut()
            .expect("the capture has a spec")
            .restart_policy = Some("Never".to_string());
        let proxy = container_status(p, "proxy");
        proxy.restart_count = 0;
        proxy.last_state = None;
        terminated_now(p, "proxy", 1, None);
    });
    let sidecar = container(&down, "proxy");
    println!("{sidecar:?}");
    assert!(
        matches!(&sidecar.state, ContainerState::Terminated(run) if ending(run) == Ending::Failed)
            && sidecar.restarts == 0,
        "three of the four conditions hold on this container, so the fourth is what refuses it — \
         without them the silence below is the plant and not the rule: {sidecar:?}"
    );
    assert!(
        stopped_for_good(&down, sidecar).is_none(),
        "a sidecar is restarted until the regular containers are done, whatever the pod says"
    );

    // **The init half, both readings of the fourth condition** (NOTES § D97). The first draft
    // built only the pod-level one; the review built the other — pod `Always` with the init
    // container declaring its own `Never`, which the API accepts — and measured the pod going
    // `Failed` there too. Both have to leave by the same door, or the door is an argument about
    // one of the two ways this rule can see `Never`.
    for (reading, pod_policy, own_policy) in [
        ("the pod says Never", "Never", None),
        (
            "the container says Never under an Always pod",
            "Always",
            Some("Never"),
        ),
    ] {
        let over = init_that_failed_for_good(pod_policy, own_policy);
        let init = container(&over, "migrate");
        println!("{reading}: {init:?}");
        assert_eq!(
            (init.role, init.restart_policy.as_deref(), init.restarts),
            (ContainerRole::Init, Some("Never"), 0),
            "{reading}: every condition rule 15 reads holds on this container — the pod's phase \
             is the only thing standing between it and a card: {init:?}"
        );
        assert!(
            finished(&over),
            "{reading}: and that phase is what `analyze` skips on (NOTES § D2)"
        );
        nothing(
            &analyze(&pods_at(vec![over], now())),
            "an init container that failed and will not be retried takes its pod out of this \
             screen, so rule 15 never sees one",
        );
    }
}

/// **A captured pod whose init container failed for good, under either reading of `Never`** —
/// the pod's own policy, or the container declaring one against an `Always` pod, which the API
/// accepts and the review measured going `phase: Failed` all the same (NOTES § D97).
fn init_that_failed_for_good(pod_policy: &str, own: Option<&str>) -> PodSnapshot {
    capture_but("healthy", |p| {
        let spec = p.spec.as_mut().expect("the capture has a spec");
        spec.restart_policy = Some(pod_policy.to_string());
        if let Some(own) = own {
            spec.init_containers
                .as_mut()
                .and_then(|list| list.iter_mut().find(|c| c.name == "migrate"))
                .expect("the capture declares the init container")
                .restart_policy = Some(own.to_string());
        }
        terminated_now(p, "migrate", 1, None);
        // **The whole shape, not just the phase.** An init container that fails under `Never`
        // stops the pod there: the kubelet never starts the regular containers, so `app` is
        // waiting on a sibling rather than running, and the pod goes `Failed` — which is the door
        // this half leaves by. A plant that moved the phase alone would be a `Failed` pod with a
        // container still up, a shape no kubelet writes (NOTES § D40).
        never_ran(p, "app", WAITING_ON_A_SIBLING, None);
        p.status.as_mut().expect("the capture has a status").phase = Some("Failed".to_string());
    })
}

/// **Nothing else in this file has anything to say about this container** — asked rule by rule
/// rather than by counting cards, because `broken-hostpath` mounts `/` and rule 8 is on the
/// screen at every row of this section (NOTES § D96).
///
/// Rule 13 is asked through its per-container half at **both** answers of `bare`, which is
/// stronger than the pod-level reading: it says this container can never be rule 13's subject
/// whatever its siblings are doing, rather than that it is not one on this pod.
///
/// **And rule 15 is asked last, or the eleven silences above are a container no rule can reach.**
#[test]
fn no_other_rule_draws_on_the_container_that_has_stopped_for_good() {
    let p = stopped_under(Some("Never"), 1, None);
    let c = container(&p, "shipper");
    println!("{c:?}");
    for (rule, drew) in [
        ("rule 1, the restart loop", crash_looping(&p, c)),
        ("rule 2, the memory kill", out_of_memory(&now(), &p, c)),
        ("rule 3, the image", image_not_pulled(&p, c)),
        (
            "rule 4, the missing configuration",
            container_config_missing(&p, c),
        ),
        (
            "rule 5, the restart count",
            restarting_repeatedly(&now(), &p, c),
        ),
        ("rule 6, the previous run", previous_run_failed(&p, c)),
        (
            "rule 7, the readiness probe",
            running_but_not_ready(&now(), &p, c),
        ),
    ] {
        assert!(
            drew.is_none(),
            "{rule} drew on a container that has stopped for good, so this shape is two cards \
             about one incident: {drew:#?}"
        );
    }
    for bare in [true, false] {
        assert!(
            stuck_at_the_starting_line(c, bare).is_none(),
            "rule 13's per-container half claims a container that has run — this one is stopped \
             in the run it ran, not waiting to start (bare = {bare})"
        );
    }
    assert!(
        stopped_for_good(&p, c).is_some(),
        "and rule 15 does draw, or every silence above is a container no rule in this file can \
         reach (NOTES § D26)"
    );
}

/// **Rule 15's card, measured at the width it is drawn at** (`screens/alerts.md` § How wide a
/// card is, and how tall) — the same measure the box before this one made of its own cards, on
/// the one card this box ships.
///
/// **Ten lines is this card's own budget and the action is never cut**, so a title or an evidence
/// line that grows by one wrapped line spends a line it has not got. The *pane's* cap is 12
/// (NOTES § D113); ten is what this card measures, and the equality at the end of this test is why
/// the tighter number is the one asserted. Measured off the card
/// [`analyze`] actually draws, not off a copy of the strings: a test that re-typed the wording
/// would measure itself.
///
/// **This card's height has a maximum rather than a worst case that has to be guessed at**, and
/// that is worth stating because it is not true of its neighbours. The title and the action are
/// constants — no role split, no per-ending arm — and the evidence is cut at three lines whatever
/// it holds, so the tallest this card can ever be is `1 + 2 + 3 + 4 = 10`, two rows inside the
/// pane's twelve. A word added to the title or the action is a card
/// that overflows, not a card that gets tighter.
///
/// **Fed every reading of the exit code this rule can reach**, longest first: the bare `137`,
/// whose sentence is the longest in [`exit_meaning`]'s table that gets here, then the labelled
/// kill, then the two short ones.
#[test]
fn the_card_this_box_ships_fits_the_height_it_is_drawn_at() {
    const BODY_COLUMNS: usize = 51;
    const EVIDENCE_CAP: usize = 3;
    // **Ten, and it is this card's own budget rather than the pane's twelve** — the title and the
    // action are constants here and the evidence is cut, so the tallest this card can ever be is
    // exactly ten and the equality below says so (NOTES § D113).
    const CARD_LINES: usize = 10;

    let mut measured = 0usize;
    for (code, reason, speaks) in [
        (137, None, false),
        (137, Some("OOMKilled"), false),
        (126, None, false),
        (1, None, false),
        // **Re-measured with the container's last words on the card** (NOTES § D97). The clause
        // is a fourth fact, so these are the rows that decide whether the evidence cut is doing
        // work — and the cut is why the height does not move with it.
        (1, None, true),
        (137, None, true),
    ] {
        let p = if speaks {
            stopped_saying(code)
        } else {
            stopped_under(Some("Never"), code, reason)
        };
        let all = analyze(&pods_at(vec![p], now()));
        for card in all.iter().filter(|f| f.title == STOPPED_FOR_GOOD) {
            let title = wrapped_at(&card.title, BODY_COLUMNS).len();
            let evidence = wrapped_at(&card.evidence, BODY_COLUMNS)
                .len()
                .min(EVIDENCE_CAP);
            let action = wrapped_at(&card.action, ACTION_COLUMNS).len();
            let height = 1 + title + evidence + action;
            println!(
                "exit {code} {reason:?}{}: {height} lines — 1 + {title} title + {evidence} \
                 evidence + {action} action\n  {}\n  {}\n  {}",
                if speaks { " +last words" } else { "" },
                card.title,
                card.evidence,
                card.action
            );
            assert!(
                height <= CARD_LINES,
                "exit {code}: a {height}-line card, and this card's budget is {CARD_LINES} — \
                 tighter than the pane's twelve, because nothing on it can grow \
                 (`screens/alerts.md` § The height): {} / {}",
                card.title,
                card.action
            );
            measured += 1;
        }
    }
    assert_eq!(
        measured, 6,
        "one card per row fed, or the loop measured a screen this rule was not on"
    );
    // **The bound, not just the samples.** A name is the one thing in the evidence a cluster
    // chooses, and a 63-character one — the longest a container name may be — still measures the
    // same card, because the cut at three lines is what holds the height and not the wording.
    // Measured on the widest shape there is: the longest `137` reading **and** the last words.
    let long = stopped_saying(137);
    let card = only(
        &analyze(&pods_at(vec![long], now())),
        "broken-hostpath",
        STOPPED_FOR_GOOD,
    )
    .clone();
    let padded = Finding {
        evidence: format!("{} · {}", card.evidence, "x".repeat(63)),
        ..card.clone()
    };
    for f in [&card, &padded] {
        let height = 1
            + wrapped_at(&f.title, BODY_COLUMNS).len()
            + wrapped_at(&f.evidence, BODY_COLUMNS)
                .len()
                .min(EVIDENCE_CAP)
            + wrapped_at(&f.action, ACTION_COLUMNS).len();
        assert_eq!(
            height, CARD_LINES,
            "this card is exactly {CARD_LINES} lines in its widest reachable shape and cannot be \
             taller, because the title and the action are constants and the evidence is cut — \
             which is its own budget and two rows inside the pane's twelve \
             (`screens/alerts.md` § The height, NOTES § D113): {f:?}"
        );
    }
}
// --- RULE 15: THE CONTAINER THAT HAS STOPPED FOR GOOD END ---
