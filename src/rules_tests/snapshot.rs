//! `rules.rs` § SNAPSHOT TYPES — its tests (NOTES § D91).

use super::*;

/// The Alerts list is sorted by severity, and that order is nothing but the order
/// the variants are declared in — so it is asserted, not assumed.
#[test]
fn severity_sorts_most_severe_first() {
    let mut got = vec![Severity::Info, Severity::Critical, Severity::Warn];
    got.sort();
    println!("sorted severities: {got:?}");
    assert_eq!(
        got,
        vec![Severity::Critical, Severity::Warn, Severity::Info],
        "sorting by severity must put the most severe first"
    );
    assert!(Severity::Critical < Severity::Warn);
    assert!(Severity::Warn < Severity::Info);
}

/// One Deployment deleted and recreated under the same name — an Argo
/// prune-and-recreate: the old generation's pods still terminate under uid-A while
/// the new ones run under uid-B. D3 says that is **one** card.
#[test]
fn the_uid_is_not_part_of_the_grouping_key() {
    let old = deployment("9f2c-aaaa");
    let new = deployment("9f2c-bbbb");

    println!("old generation: {old:?}\nnew generation: {new:?}");
    println!("group keys: {:?} vs {:?}", old.group_key(), new.group_key());

    assert_eq!(
        old.group_key(),
        new.group_key(),
        "two ObjectIds differing only in uid must group onto one card (D3)"
    );

    // The same fact the way `views.rs` will actually meet it: as map keys.
    let keys: HashSet<_> = [old.group_key(), new.group_key()].into_iter().collect();
    println!("distinct group keys: {}", keys.len());
    assert_eq!(keys.len(), 1, "one Deployment must not produce two cards");
}

/// The negative half: a `group_key` dropping the namespace, or the name, or
/// returning a constant satisfies the test above perfectly, and over-grouping is the
/// worse bug — one card over two Deployments hides an outage. The last case is a
/// Node, the shape a cluster-scoped `ObjectId` actually arrives in.
#[test]
fn objects_that_are_not_the_same_object_do_not_share_a_group_key() {
    let base = deployment("9f2c-aaaa");
    let mut other_namespace = base.clone();
    other_namespace.namespace = Some("staging".to_string());
    let mut other_name = base.clone();
    other_name.name = "api".to_string();
    let mut other_kind = base.clone();
    other_kind.kind = ObjectKind::StatefulSet;
    let node = ObjectId {
        kind: ObjectKind::Node,
        namespace: None,
        name: "k8rs-worker2".to_string(),
        uid: Some("7b1e-cccc".to_string()),
    };

    for (label, other) in [
        ("different namespace", &other_namespace),
        ("different name", &other_name),
        ("different kind", &other_kind),
        ("cluster-scoped node vs namespaced workload", &node),
    ] {
        println!("{label}: {:?} vs {:?}", base.group_key(), other.group_key());
        assert_ne!(
            base.group_key(),
            other.group_key(),
            "{label}: these are different objects and must not share a card"
        );
    }
}

/// Equality still sees the uid, because it answers D22's question — is this still
/// the object the operator inspected. A `group_key` that grouped correctly by
/// weakening `Eq` would trade a drawing bug for a mutation bug.
#[test]
fn equality_still_sees_the_uid() {
    assert_ne!(
        deployment("9f2c-aaaa"),
        deployment("9f2c-bbbb"),
        "equality must distinguish an object from the one that replaced it (D22)"
    );
    assert_eq!(
        deployment("9f2c-aaaa"),
        deployment("9f2c-aaaa"),
        "the same object read twice must still compare equal"
    );
}

// --- THE AGE AT THE RIGHT EDGE ---
//
// Every case here hands a **duration** in and compares the answer against the string a
// screen draws. The ladder goes through [`age`] because the rungs are what it is
// testing; the card goes through [`Finding::age`], because that is the call a renderer
// makes for a finding. Nothing parses English back into a number: a test that read
// "4" out of "4 min ago" would agree with an implementation that printed the minutes
// of the wall clock, which is the class of bug the whole "timestamps, not phrases"
// contract exists to stop.

/// A moment `secs` seconds before the pinned [`now`]. Negative puts the event in the
/// future: D55's *slow* laptop while it is inside [`SKEW_ALLOWANCE`], and past that a
/// rule reading a field that was never an event time.
///
/// `checked_sub`, because every subtraction in this file that can leave the
/// representable range is checked (NOTES § D56); here the failure would be a
/// mistyped case rather than a hostile pod, and it names itself either way.
fn ago(secs: i64) -> Time {
    Time(
        now()
            .0
            .checked_sub(SignedDuration::from_secs(secs))
            .unwrap_or_else(|e| panic!("{secs}s before the pinned now is not a moment: {e}")),
    )
}

/// **The ladder, at both sides of every boundary it has.** The rungs are not a
/// choice: each string below is one a `screens/` file already prints, and the
/// boundaries are where one stops being the truth and the next starts.
///
/// **43 minutes is the case that is here for the arithmetic and not for the
/// wording** — `now.0 - event.0` yields a seconds-only `Span`, so a formatter written
/// with `.get_minutes()` reads "0 min ago" for it, and for every gap under an hour.
/// The value comes from NOTES § D54, which names the trap and the length it hides.
///
/// **The two cases at the top are the [`SKEW_ALLOWANCE`] boundary**, and the far one
/// is not a clock story: 25 hours ahead is what a rule pointed at a certificate's
/// `notAfter` or at a raw `deletionTimestamp` produces, and the requirement is that it
/// draws *nothing* rather than a sentence that reads fine.
///
/// **The hours rung runs to 48**, `screens/widgets.md` § 1b, so the three cases across
/// its far edge are 24 h, 47 h and 48 h — the band `1 day ago` used to flatten. **`1 day
/// ago` is not in the table because the ladder cannot produce it**, and a row asserting
/// it would be asserting a string no screen may draw.
#[test]
fn the_age_ladder_is_the_words_the_screens_print() {
    for (secs, want) in [
        (-90_000, None),
        (-301, None),
        (-300, Some("just now")),
        (-1, Some("just now")),
        (0, Some("just now")),
        (1, Some("1s ago")),
        (40, Some("40s ago")),
        (59, Some("59s ago")),
        (60, Some("1 min ago")),
        (60 * 4, Some("4 min ago")),
        (60 * 43, Some("43 min ago")),
        (60 * 60 - 1, Some("59 min ago")),
        (60 * 60, Some("1 hour ago")),
        (60 * 60 * 2, Some("2 hours ago")),
        (60 * 60 * 24 - 1, Some("23 hours ago")),
        (60 * 60 * 24, Some("24 hours ago")),
        (60 * 60 * 47, Some("47 hours ago")),
        (60 * 60 * 48 - 1, Some("47 hours ago")),
        (60 * 60 * 48, Some("2 days ago")),
        (60 * 60 * 24 * 6, Some("6 days ago")),
    ] {
        let got = age(&now(), &ago(secs));
        println!("{secs:>9}s -> {got:?}");
        assert_eq!(
            got.as_deref(),
            want,
            "an event {secs}s before now has to read {want:?} — the strings are the \
             ones screens/ draws, and the boundaries are where they stop being true"
        );
    }

    // The rung the table cannot reach, because its cases are whole seconds: an event
    // 400ms old is inside the first one. `0s ago` is a string no screen draws and it
    // reads as a stopped clock, so the sub-second gap says "just now" with the
    // negative ages — the one place this branch is not about a wrong laptop.
    // Derived from the pin rather than transcribed beside it: `ago` is whole seconds, and
    // a literal here is a fifth place the pin would have to move (NOTES § D57).
    let sub_second = age(
        &now(),
        &Time(
            now()
                .0
                .checked_sub(SignedDuration::from_millis(400))
                .expect("400ms before the pinned now is a moment"),
        ),
    );
    println!("     0.4s -> {sub_second:?}");
    assert_eq!(
        sub_second.as_deref(),
        Some("just now"),
        "an event 400ms old is \"just now\", never \"0s ago\""
    );
}

/// **One event, four laptops** — the framing NOTES § D55 corrects, and the two things
/// the guard does not do.
///
/// A laptop a little behind the cluster produces a negative age and draws "just now":
/// under-reporting, which harms nobody. Far enough behind and the timestamp stops
/// being distinguishable from a rule reading a field that is future-dated by design,
/// so [`age`] draws **nothing** and leaves the explaining to the header banner, which
/// is its own box. A laptop *ahead* of the cluster inflates the same event into a
/// ten-minute-old one, and **that is left visible on purpose** — clamping it would
/// hide a wrong clock rather than survive one, and it is the half that manufactures
/// findings on a healthy cluster.
///
/// A formatter that took `.abs()` of the difference, or clamped both ends, passes the
/// ladder test above and fails here.
#[test]
fn a_laptop_a_little_behind_says_just_now_far_behind_says_nothing_and_ahead_is_not_hidden() {
    let event = time("2026-08-12T12:00:00Z");
    let behind = |mins: i64| {
        Time(
            event
                .0
                .checked_sub(SignedDuration::from_mins(mins))
                .expect("a moment"),
        )
    };

    for (label, laptop, want) in [
        ("2 min behind the cluster", behind(2), Some("just now")),
        ("agreeing with the cluster", behind(0), Some("just now")),
        ("10 min behind the cluster", behind(10), None),
        (
            "10 min ahead of the cluster",
            behind(-10),
            Some("10 min ago"),
        ),
    ] {
        let got = age(&laptop, &event);
        println!("event {:?}, laptop {label}: {got:?}", event.0);
        assert_eq!(got.as_deref(), want, "a laptop {label} must draw {want:?}");
    }
}

/// **The `Option`, on the two taints the capture actually carries.** N2's card is
/// where the field's two states are one keystroke apart: `break-nodes` cordons
/// `k8rs-worker`, the node lifecycle controller mirrors that boolean into a taint and
/// stamps `timeAdded` on it — so the card can say when — while the operator's own
/// `dedicated=gpu:NoExecute` on `k8rs-worker2` was written by `kubectl taint`, which
/// is client-side and stamps nothing (NOTES § D64, § D65).
///
/// What is asserted is the whole render decision — [`Finding::age`], the one call
/// both renderers make: a phrase for the card that has a moment, **nothing at all**
/// for the one that has not, which is `screens/alerts.md`'s blank right edge and
/// `screens/once.md`'s bare title line.
///
/// **And why the field is not a plain `Time`:** the value a non-optional field would
/// hold is the epoch, which this formatter dates honestly and uselessly. That
/// assertion is deliberately loose about the count — it is 1970 that is being shown,
/// not a number worth pinning.
#[test]
fn the_captured_cordon_dates_itself_and_the_hand_applied_taint_leaves_the_age_blank() {
    let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
    let taint = |node: &str, key: &str| {
        nodes
            .iter()
            .find(|n| n.id.name == node)
            .unwrap_or_else(|| panic!("the capture has no {node}"))
            .taints
            .iter()
            .find(|t| t.key == key)
            .unwrap_or_else(|| panic!("{node} carries no {key} taint"))
            .clone()
    };
    let cordon = taint("k8rs-worker", "node.kubernetes.io/unschedulable");
    let by_hand = taint("k8rs-worker2", "dedicated");

    // The card N2 files, with the moment the capture gives it and without one. Both
    // identities are the node itself — `owner == object` for N1–N3 (D39).
    let node = ObjectId {
        kind: ObjectKind::Node,
        namespace: None,
        name: "k8rs-worker".to_string(),
        uid: None,
    };
    let card = |t: Option<Time>| Finding {
        severity: Severity::Warn,
        title: "This node refuses new pods (cordoned)".to_string(),
        evidence: "2 pods here would still have to move".to_string(),
        action: "allow new pods once the work is done".to_string(),
        kubectl_cmd: Some("kubectl describe node k8rs-worker".to_string()),
        owner: node.clone(),
        object: node.clone(),
        timestamp: t,
    };
    let dated = card(cordon.added_at.clone());
    let undated = card(by_hand.added_at.clone());
    println!(
        "cordon taint {:?}\n  {} · {:?}\nhand-applied taint {:?}\n  {} · {:?}",
        cordon.added_at,
        dated.title,
        dated.age(&now()),
        by_hand.added_at,
        undated.title,
        undated.age(&now()),
    );

    // The property the fixture has to keep, and it is asserted at the precision of the
    // string below rather than looser. A band of `[1m, 24h)` would let a recapture past
    // this line and fail on the phrase instead, with a message about cards saying when —
    // which is the confusion the check exists to prevent, not to cause.
    //
    // **The pin is the midnight after the *newest* capture, and the rung follows the gap
    // between that capture and this one** (NOTES § D57, § D97, § D156). The corpus is one trip
    // plus one targeted capture since 2026-08-22 — `unstarted.json` was taken 40 h after the
    // rest and the pin followed it — so the trip no longer sits inside a single day of the pin,
    // and that distance decides the rung. This cordon has stood on three different rungs —
    // minutes, then days at 48h 24m, then hours, then minutes at 47m 52s, and days again now at
    // 48h 47m 52s — which is why the number above is asserted at all: the rung is a property of
    // how far apart the two captures fell and of nothing this test is about.
    //
    // **The pin lands on a midnight, so the lever is a whole day and never an hour.** Any
    // targeted capture taken on 2026-08-22 pins 2026-08-23 and prints `2 days ago` here; one
    // taken a day earlier would have pinned 2026-08-22 and printed `24 hours ago`, and a day
    // later `3 days ago`. So the near boundary is not the 48 minutes this cordon happens to sit
    // over the days rung — it is one fewer day of gap, and the far one is one more. Crossing
    // either is the same edit as any other repin, and the one that looks like a bug rather than
    // a clock move. The number above is the guard: it fails first, with the arithmetic in the
    // message.
    let stamped = cordon.added_at.clone().expect(
        "the controller stamps timeAdded on the taint it mirrors from spec.unschedulable \
         — a capture without it is D64's premise back again",
    );
    let elapsed = now().0.duration_since(stamped.0);
    assert_eq!(
        elapsed.as_mins(),
        2927,
        "the cordon is {elapsed:?} before the pinned now, and the phrase below says 2 \
         days — if `just fixtures` was re-run, repin `fn now()` (see the note there for \
         what moves with it) and move both together"
    );
    assert_eq!(
        dated.age(&now()).as_deref(),
        Some("2 days ago"),
        "a cordon the controller stamped has a moment, and the card says when"
    );
    assert_eq!(
        undated.age(&now()),
        None,
        "`kubectl taint` stamps no time, so the card has no age to draw and draws \
         none — never a nearby timestamp that answers a different question"
    );

    // **The third state, which no capture can hold.** Every committed timestamp is
    // before the pin by construction — the sweep guarantees it — so the card that was
    // filled from a field which is future-dated *by design* has to be synthesised, the
    // same licence D40 gives the taint that carries a value and a stamp at once. The
    // moment here is C1's shape: `notAfter` on the healthy committed certificate,
    // `2027-08-12` — a date `certs-test.sh` pins and which every pin this file has ever
    // carried is a year behind (`certificate.rs` § C1 on why the date and not a count of
    // days out). `Finding::age` flattens it to the
    // same blank the missing field draws — `.map` in place of `.and_then` would print
    // it, and `Option<Time>` alone cannot tell the two cases apart because the field
    // is present and perfectly valid.
    let wrong_field = card(Some(time("2027-08-12T00:00:00Z")));
    println!(
        "a rule that filled the timestamp from a certificate's notAfter: {:?}",
        wrong_field.age(&now())
    );
    assert_eq!(
        wrong_field.age(&now()),
        None,
        "a moment a year ahead is a rule reading the wrong field, not a wrong clock, \
         and it draws nothing rather than a sentence that reads fine"
    );

    let epoch = age(&now(), &time("1970-01-01T00:00:00Z")).expect("1970 is in the past");
    println!("what a zero would have drawn: {epoch}");
    assert!(
        epoch.ends_with(" days ago") && epoch != "just now",
        "a zero timestamp draws as 1970 and not as silence — which is why the field is \
         an Option, and it read {epoch:?}"
    );
}

/// Rules 1, 5 and 6 all fire on this one pod, so it is the fixture that proves the
/// three fields they read arrive together and separately.
#[test]
fn crashloop_pod_decodes_what_rules_1_5_and_6_read() {
    let raw = fixture("crashloop");
    let p = pod("crashloop");
    println!("{:?}\n  containers: {:?}", p.id, p.containers);

    assert_eq!(p.id.kind, ObjectKind::Pod);
    assert_eq!(p.id.namespace.as_deref(), Some("default"));
    assert_eq!(p.id.name, "broken-crashloop");
    assert_eq!(
        p.id.uid.as_deref(),
        Some(captured_str(&raw, &["metadata", "uid"])),
        "D22 asks whether this is still the object the operator inspected, and this \
         field is the whole of the answer — the apiserver's own uid, minted fresh for \
         every pod of every capture"
    );
    assert_eq!(
        p.owner, p.id,
        "nothing controls this pod, so it files under itself (D3)"
    );
    assert_eq!(
        p.node.as_deref(),
        Some(captured_str(&raw, &["spec", "nodeName"])),
        "the key N5 and N6 join on; which worker the scheduler picked is the \
         cluster's business, and it picked a different one this capture"
    );
    assert_eq!(p.phase.as_deref(), Some("Running"));

    let status = captured_status(&raw, "containerStatuses", "quitter");
    let c = container(&p, "quitter");
    assert_eq!(c.role, ContainerRole::Regular);
    assert!(!c.ready);
    assert!(
        !c.started,
        "it is between crashes, so it is not started either — and rule 7 must not \
         read that as a readiness failure"
    );
    assert_eq!(
        c.restarts,
        captured_i32(status, &["restartCount"]),
        "rule 5 counts restarts, and it counts this container's own"
    );
    assert!(
        c.restarts >= 3,
        "a container the kubelet has put in CrashLoopBackOff has died several times \
         by definition, so a count below rule 5's own WARN threshold means the \
         counter is wrong and not the pod: got {}",
        c.restarts
    );
    match &c.state {
        ContainerState::Waiting { reason, message } => {
            assert_eq!(reason.as_deref(), Some("CrashLoopBackOff"), "rule 1");
            assert!(
                message.as_deref().unwrap_or_default().contains("back-off"),
                "the capture has to carry `state.waiting.message`, which is the field \
                 `justfile`'s `back-off` guard is anchored to — no rule renders it on a \
                 `CrashLoopBackOff` container, so a capture that lost it would be a capture of \
                 the wrong shape and nothing else would say so (NOTES § D131). Got {message:?}"
            );
        }
        other => panic!("a crashlooping container must decode as waiting, got {other:?}"),
    }
    // The two moments come from their own keys, and the gap between them is the whole
    // point of carrying both: "it runs for about two seconds and then exits 1" is a
    // different incident from "it ran for forty minutes and then exited 1", and a
    // decode that filled `started_at` from `finished_at` — or dropped it — tells the
    // operator the same sentence for both. The exit code and the reason stay literal:
    // they are what `scripts/broken.yaml`'s container *does*, not when it was
    // photographed doing it.
    let last = at(status, &["lastState", "terminated"]);
    assert_eq!(
        c.last_terminated,
        Some(Terminated {
            reason: Some("Error".to_string()),
            exit_code: 1,
            started_at: Some(captured_time(last, &["startedAt"])),
            finished_at: Some(captured_time(last, &["finishedAt"])),
            // D51's field, and the capture carries it now: `terminationMessagePolicy:
            // FallbackToLogsOnError` makes the kubelet copy the tail of the
            // container's log in here, which is what turns rule 6's action from
            // "check the logs" into the log line itself.
            message: Some(captured_str(last, &["message"]).to_string()),
        }),
        "rule 6 translates the exit code, and the finding is aged from finished_at"
    );
    let decoded = c.last_terminated.as_ref().expect("asserted just above");
    assert!(
        decoded.started_at < decoded.finished_at,
        "the run has to have started before it ended, or one of the two was filled \
         from the other: {decoded:?}"
    );
    assert!(
        decoded
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("connection refused"),
        "rule 6 shows the application's own last line, and the log tail is what makes \
         it worth showing: {:?}",
        decoded.message
    );
    assert_eq!(
        c.memory_limit, None,
        "this container sets no limit, and rule 2 may not report exceeding one"
    );
}

/// Rule 2 needs two fields from two different places: the kill reason from the
/// status and the limit it broke from the spec.
#[test]
fn oom_pod_decodes_the_exit_code_and_the_limit_rule_2_reads() {
    let p = pod("oom");
    let c = container(&p, "hog");
    println!(
        "{}: {:?} limit={:?}",
        c.name, c.last_terminated, c.memory_limit
    );

    let last = c
        .last_terminated
        .as_ref()
        .expect("the container was killed once");
    assert_eq!(last.reason.as_deref(), Some("OOMKilled"));
    assert_eq!(
        last.exit_code, 137,
        "137 is SIGKILL, almost always the OOM killer"
    );
    assert_eq!(
        c.memory_limit.as_deref(),
        Some("64Mi"),
        "rule 2's evidence is the limit that was exceeded"
    );
    assert_eq!(c.memory_request.as_deref(), Some("64Mi"));
    assert_eq!(c.cpu_request, None, "this pod sets no cpu request");
}

/// Rules 3 and 4 have nothing to say beyond the runtime's own message, so losing it
/// in the decode would leave the finding with no evidence at all.
#[test]
fn image_and_config_failures_keep_the_runtimes_own_message() {
    let raw = fixture("image");
    let image = pod("image");
    let c = container(&image, "nope");
    println!("image: {:?}", c.state);
    let ContainerState::Waiting { reason, message } = &c.state else {
        panic!("an image that cannot be pulled leaves the container waiting")
    };
    // **The reason is one of two and the kubelet alternates between them**, which is
    // why neither is written here as a literal. A failed pull reports `ErrImagePull`,
    // the kubelet then backs off and reports `ImagePullBackOff` until it retries, and
    // a capture lands on whichever half of that cycle was running — this one caught
    // the first, the capture before it caught the second, and nothing about rule 3
    // changed in between. NOTES § v1 rule set row 3 names both for exactly this
    // reason, so both are the requirement and only the pair is asserted.
    let waiting = at(
        captured_status(&raw, "containerStatuses", "nope"),
        &["state", "waiting"],
    );
    assert_eq!(reason.as_deref(), Some(captured_str(waiting, &["reason"])));
    assert!(
        matches!(reason.as_deref(), Some("ImagePullBackOff" | "ErrImagePull")),
        "rule 3 fires on those two and nothing else — a fixture in a third state is \
         not a fixture for it: {reason:?}"
    );
    // Verbatim against the capture's own bytes, for the reason the two condition
    // messages are: rule 3 has no evidence but this sentence, and a decode that
    // appended to it or cut it short passes any looser assertion.
    assert_eq!(
        message.as_deref(),
        Some(captured_str(waiting, &["message"]))
    );
    assert!(
        message
            .as_deref()
            .unwrap_or_default()
            .contains("registry.invalid/does-not-exist:v9"),
        "and only the message names the image, which is the whole of what rule 3 \
         tells the user to go and check: {message:?}"
    );
    assert_eq!(c.restarts, 0, "it never started, so it never restarted");
    assert_eq!(c.last_terminated, None);

    let config = pod("config");
    let c = container(&config, "app");
    println!("config: {:?}", c.state);
    let ContainerState::Waiting { reason, message } = &c.state else {
        panic!("a missing ConfigMap leaves the container waiting")
    };
    assert_eq!(reason.as_deref(), Some("CreateContainerConfigError"));
    assert_eq!(
        message.as_deref(),
        Some("configmap \"this-configmap-does-not-exist\" not found"),
        "rule 4 names the object that is missing, and the message is where it is"
    );
}

/// D27's blind spot, at the decode: this pod's app container is fine and the init one is
/// dead, and a snapshot built from `containerStatuses` alone would hand the rules nothing
/// to fire on. The rules do read both arrays now ([`analyze`]); what this test holds is
/// the list they read it off.
#[test]
fn the_init_container_is_in_the_list_and_marked_as_one() {
    let raw = fixture("init");
    let p = pod("init");
    println!(
        "{:?}",
        p.containers
            .iter()
            .map(|c| (&c.name, c.role))
            .collect::<Vec<_>>()
    );

    assert_eq!(p.containers.len(), 2, "both arrays are read, not just one");

    let migrate = container(&p, "migrate");
    assert_eq!(
        migrate.role,
        ContainerRole::Init,
        "the finding has to be able to say which one is the init one — and this one \
         declares no restartPolicy, so it is an init container and not a sidecar"
    );
    // Out of `initContainerStatuses`, which is the point of the test: the same
    // counter sits in `containerStatuses` for `app`, at 0, so a decode that read the
    // regular list for both containers answers a plausible number here.
    assert_eq!(
        migrate.restarts,
        captured_i32(
            captured_status(&raw, "initContainerStatuses", "migrate"),
            &["restartCount"]
        ),
        "rule 5's counter for an init container comes out of the init array"
    );
    assert!(
        migrate.restarts > 0,
        "an init container that has never restarted is not the D27 blind spot this \
         fixture exists for: got {}",
        migrate.restarts
    );
    // **The state, against the array it came out of — both faces, because the capture reaches
    // both** (NOTES § D114). A crash-looping container alternates between `waiting:
    // CrashLoopBackOff` and the `terminated` run it just left, and `scripts/cluster.sh`
    // § `[init]` accepts either: it measured `state.terminated` in 39 samples of 70 and calls
    // demanding the waiting reason "the too-tight half". Asserting one of them is asserting
    // which half `just fixtures` caught, and the 2026-08-16 trip caught the other one.
    //
    // What is asserted instead is that the decode agrees with **the JSON it was handed**, which
    // is the technique this file uses everywhere else and catches the same three defects — a
    // field dropped, filled from its neighbour, or rewritten.
    let raw_state = &captured_status(&raw, "initContainerStatuses", "migrate")["state"];
    match &migrate.state {
        ContainerState::Waiting { reason, .. } => assert_eq!(
            reason.as_deref(),
            Some(captured_str(raw_state, &["waiting", "reason"])),
            "the reason the capture holds, out of the init array"
        ),
        ContainerState::Terminated(run) => assert_eq!(
            run.exit_code,
            captured_i32(raw_state, &["terminated", "exitCode"]),
            "the code the capture holds, out of the init array"
        ),
        other => panic!(
            "a crash-looping init container is in backoff or in the run it just left, and \
             {other:?} is neither — this fixture stopped being the D27 blind spot"
        ),
    }
    // And the property the fixture has to keep whichever face it was caught in: the run before
    // this one failed, which is what makes it a *loop* rather than a container doing its job.
    assert_eq!(
        migrate.last_terminated.as_ref().map(|t| t.exit_code),
        Some(captured_i32(
            captured_status(&raw, "initContainerStatuses", "migrate"),
            &["lastState", "terminated", "exitCode"]
        ))
    );
    assert_ne!(
        migrate.last_terminated.as_ref().map(|t| t.exit_code),
        Some(0),
        "an init container whose previous run succeeded is `healthy-retry`, not this fixture"
    );

    let app = container(&p, "app");
    assert_eq!(app.role, ContainerRole::Regular);
    assert!(
        matches!(&app.state, ContainerState::Waiting { reason, .. }
            if reason.as_deref() == Some("PodInitializing")),
        "the app container is waiting on the init one, not broken itself: {:?}",
        app.state
    );
    assert_eq!(
        app.restarts, 0,
        "it is still waiting on the init container, so it has never started and \
         never restarted — and the init container's count above is not this one"
    );
    // The two containers of this pod carry *different* images — the app one was never
    // pulled, so the kubelet echoed the spec's `busybox` back instead of the resolved
    // reference. A decode reading the image off the pod, or off the first status, or
    // off the spec, answers the same string twice here.
    assert_eq!(
        migrate.image, "docker.io/library/busybox:latest",
        "rule 3's action is 'check the image name or the pull secret', so the name is \
         per container and comes from the status"
    );
    assert_eq!(app.image, "busybox");
}

/// Rule 10 exists because the scheduler writes both the verdict and the sentence onto
/// the pod, so no Events watch is needed (D27). Losing the message would leave the
/// most common beginner question unanswered.
#[test]
fn pending_pod_carries_the_schedulers_sentence_and_no_containers() {
    let raw = fixture("pending");
    let p = pod("pending");
    println!("{:?}\n  scheduled: {:?}", p.id, p.scheduled);

    let c = p
        .scheduled
        .as_ref()
        .expect("an unschedulable pod has a PodScheduled condition");
    assert_eq!(c.type_, "PodScheduled");
    assert_eq!(c.status, "False");
    assert_eq!(c.reason.as_deref(), Some("Unschedulable"));
    // Equality against the capture's own bytes, not `starts_with`: D37 says a
    // controller's message is shown verbatim, and a decode that appended or truncated
    // one would pass any looser assertion. The sentence itself is the scheduler's and
    // counts the cluster it ran against — "0/3 … 2 Insufficient cpu" when this fixture
    // was three nodes and a cpu request, "0/4 … didn't match Pod's node
    // affinity/selector" now that it is four and a nodeSelector — so what is pinned
    // here is that it arrives whole, and what rule 10 needs of it is asserted below.
    assert_eq!(
        c.message.as_deref(),
        Some(captured_str(
            captured_condition(&raw, "PodScheduled"),
            &["message"]
        )),
        "the scheduler's own sentence is the finding, word for word"
    );
    assert!(
        c.message
            .as_deref()
            .unwrap_or_default()
            .contains("nodes are available"),
        "rule 10's entire evidence is this sentence — a message that no longer counts \
         the nodes that refused the pod is a fixture rule 10 cannot be written \
         against: {:?}",
        c.message
    );

    assert_eq!(
        p.phase.as_deref(),
        Some("Pending"),
        "a pod no node accepted is Pending, and N5 must not charge its requests to one"
    );
    assert_eq!(p.node, None, "it was never scheduled onto anything");
    assert!(
        p.containers.is_empty(),
        "the kubelet never reported on it, so no container rule can fire: {:?}",
        p.containers
    );
}

/// **The pod that was placed and then reported on by nobody** — the decode every rule-side test
/// of rule 13's second shape rests on (NOTES § D156).
///
/// `broken-unstarted` was bound to a node through the `binding` subresource — which is what
/// actually writes `PodScheduled: True`; a create carrying `spec.nodeName` writes no condition at
/// all — and that node's kubelet was stopped before the bind, so nothing ever wrote a status for
/// it. What lands on disk is a `status` with one condition in it and no `containerStatuses` key.
///
/// **The two halves are asserted separately and neither is the other.** The capture's
/// `spec.containers` is a real container the API server accepted; its `status.containerStatuses`
/// is not there. That pairing is the whole of D156's first ruling — `spec.containers: []` and an
/// absent `spec.containers` are both refused by the API server (`spec.containers: Required
/// value`), so an empty [`PodSnapshot::containers`] on a decoded pod means *the kubelet has
/// written no status* and can mean nothing else. A decode that filled `containers` from the spec
/// would satisfy neither assertion below, and rule 13's second shape would have no input.
///
/// **And the property the fixture has to keep:** the node it names reads `Ready: Unknown` in
/// `nodes.json`. That is what makes the committed capture rule 13's *negative* and N1's positive
/// — a recapture that brought this pod back on a healthy node would silently turn
/// [`the_pod_nothing_reported_on_is_the_nodes_card_when_the_node_went_quiet`] into a test of the
/// other branch.
#[test]
fn the_placed_pod_no_kubelet_ever_reported_on_decodes_with_no_containers_at_all() {
    let raw = fixture("unstarted");
    let p = pod("unstarted");
    println!(
        "{:?}\n  scheduled: {:?}\n  node: {:?}\n  containers: {:?}\n  ready_to_start: {:?}",
        p.id, p.scheduled, p.node, p.containers, p.ready_to_start_containers
    );

    assert!(
        !at(&raw, &["spec", "containers"])
            .as_array()
            .expect("a Pod the API server accepted declares spec.containers")
            .is_empty(),
        "the pod declares a container — without that the empty decode below would be a pod \
         with nothing to run rather than a pod nothing has run"
    );
    assert!(
        at(&raw, &["status", "containerStatuses"]).is_null(),
        "and the kubelet wrote no statuses for it — the key is absent, which is the framing a \
         real API server produces: {:?}",
        at(&raw, &["status", "containerStatuses"])
    );
    assert!(
        p.containers.is_empty(),
        "so the snapshot's container list is empty, and that is rule 13's second trigger: {:?}",
        p.containers
    );

    let c = p
        .scheduled
        .as_ref()
        .expect("a bound pod carries a PodScheduled condition");
    assert_eq!(c.type_, "PodScheduled");
    assert_eq!(
        c.status, "True",
        "it was given a machine — this is not rule 10's pod and not rule 14's"
    );
    assert_eq!(
        c.last_transition.as_ref(),
        Some(&captured_time(
            captured_condition(&raw, "PodScheduled"),
            &["lastTransitionTime"]
        )),
        "and the moment it was bound, which is both rule 13's clock and the age on its card — \
         the API server writes it once at bind and never refreshes it"
    );
    assert_eq!(
        at(&raw, &["status", "conditions"])
            .as_array()
            .expect("the capture has a conditions array")
            .len(),
        1,
        "PodScheduled is the only line in this pod's status: no Ready, no Initialized, and no \
         PodReadyToStartContainers"
    );
    assert!(
        p.ready_to_start_containers.is_none(),
        "so the sandbox condition is absent rather than False, which is the arm of rule 13's \
         evidence line that must not claim this pod has its storage: {:?}",
        p.ready_to_start_containers
    );

    assert_eq!(
        p.node.as_deref(),
        Some(captured_str(&raw, &["spec", "nodeName"])),
        "the bind set a node name, and that name is the whole of rule 13's hand-off to N1"
    );
    assert_eq!(p.phase.as_deref(), Some("Pending"));
    assert_eq!(
        p.deletion_timestamp, None,
        "nobody has asked for it to go, so rule 12 is not the card here"
    );

    let node = captured_nodes()
        .into_iter()
        .find(|n| Some(n.id.name.as_str()) == p.node.as_deref())
        .expect("the pod's node is in the committed node capture");
    assert_ne!(
        node.conditions
            .iter()
            .find(|c| c.type_ == "Ready")
            .map(|c| c.status.as_str()),
        Some("True"),
        "and that node is not saying it is ready — this capture is rule 13's negative, and a \
         recapture that landed it on a healthy worker would turn the hand-off test into a test \
         of the branch it exists to contrast with"
    );
}

/// Rule 5's whole point: this pod is Running and Ready and something is still wrong.
#[test]
fn a_pod_that_looks_healthy_still_carries_its_restart_count() {
    let raw = fixture("restarts");
    let status = captured_status(&raw, "containerStatuses", "flaky");
    let p = pod("restarts");
    let c = container(&p, "flaky");
    println!(
        "{}: ready={} restarts={} state={:?}",
        c.name, c.ready, c.restarts, c.state
    );

    assert!(c.ready, "it is passing its probes right now");
    // The restart is what makes this pod interesting, and the run that followed it is
    // what this state carries. Rule 5's "restarted 3 times" is aged from
    // `last_terminated`, but "it came back up and has been up since" is only readable
    // here — so the two are asserted against each other below rather than against a
    // pair of literals that were true of one afternoon's cluster.
    assert_eq!(
        c.state,
        ContainerState::Running {
            started_at: Some(captured_time(status, &["state", "running", "startedAt"])),
        }
    );
    assert_eq!(
        c.restarts,
        captured_i32(status, &["restartCount"]),
        "rule 5's whole evidence on a pod that looks healthy"
    );
    assert!(
        (3..10).contains(&c.restarts),
        "this is rule 5's WARN fixture (REQUIREMENTS: ≥3 warns, ≥10 is critical), and \
         a capture that drifted out of that band is a fixture for the other severity \
         or for neither: got {}",
        c.restarts
    );
    let last = c
        .last_terminated
        .as_ref()
        .expect("a pod that restarted has a previous run");
    assert_eq!(
        last.exit_code, 1,
        "and it died with the application's own error code"
    );
    assert!(
        matches!(&c.state, ContainerState::Running { started_at }
            if started_at.as_ref() >= last.finished_at.as_ref()),
        "the run that is up now began after the one that died, or the two states were \
         filled from one another: {:?} vs {:?}",
        c.state,
        last.finished_at
    );
}

/// Rule 12 compares the deletion timestamp against the pod's own grace period, never
/// a constant.
#[test]
fn the_terminating_pod_carries_its_deletion_timestamp_and_its_own_grace_period() {
    let raw = fixture("stuck");
    let stuck = pod("stuck");
    println!(
        "stuck: deleted at {:?}, grace {:?}",
        stuck.deletion_timestamp, stuck.grace_period_seconds
    );
    assert_eq!(
        stuck.deletion_timestamp,
        Some(captured_time(&raw, &["metadata", "deletionTimestamp"])),
        "the deadline the apiserver wrote when it accepted the delete — a different \
         instant on every capture, and rule 12 subtracts the grace below from it"
    );
    assert_eq!(
        stuck.grace_period_seconds,
        Some(5),
        "`scripts/broken.yaml` asks for five seconds and the delete granted them; \
         this is the manifest's number, not the cluster's"
    );
    // Rule 12 promises "a finalizer, *or* the kubelet is holding it" — two causes
    // whose actions are nothing alike, and this is the field that decides which.
    // `scripts/broken.yaml` puts this one on the pod on purpose and says the fix is
    // patching it out.
    assert_eq!(
        stuck.finalizers,
        vec!["k8rs.test/never-removed".to_string()],
        "the finding names who is holding the object, and `kubectl describe pod` \
         does not print this at all"
    );

    let healthy = pod("healthy");
    assert_eq!(
        healthy.deletion_timestamp, None,
        "a pod nobody deleted must not look like it is shutting down"
    );
    assert!(
        healthy.finalizers.is_empty(),
        "and an ordinary pod has none, so rule 12 blames the kubelet instead: {:?}",
        healthy.finalizers
    );
    assert_eq!(
        healthy.grace_period_seconds,
        Some(30),
        "no delete has happened, so the value falls back to what the spec asked for"
    );
}

/// The two grace fields agree in every capture, so precedence is asserted by taking
/// the real object and giving it the one thing a cluster can produce and this
/// capture did not: a `kubectl delete --grace-period=0`.
#[test]
fn a_forced_delete_beats_the_grace_period_the_spec_asked_for() {
    let mut object: Pod = serde_json::from_value(fixture("stuck")).expect("stuck.json is a Pod");
    object.metadata.deletion_grace_period_seconds = Some(0);
    object
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .termination_grace_period_seconds = Some(30);

    let p = PodSnapshot::from(object);
    println!("forced delete: grace {:?}", p.grace_period_seconds);
    assert_eq!(
        p.grace_period_seconds,
        Some(0),
        "rule 12 must not stay quiet for 30 seconds the pod was never granted"
    );
}

/// Rule 8 fires on `/`, on a runtime socket or a directory one sits under, or on a
/// writable mount (NOTES § D78), and the Phase 4 posture report lists the read-only
/// ones — so the decode carries the fact and not a verdict.
///
/// **One volume, two mounts, and every field of the pair is asserted here**, because
/// this is the object the three discriminations below are read off: one hostPath
/// volume mounted twice, narrowed by a `subPath` in one container and read-only in the
/// other. The values are `scripts/broken.yaml`'s own — a re-capture of the same
/// manifest produces them again — which is why they are literals where the uid and
/// the timestamps beside them are not.
#[test]
fn the_hostpath_mount_keeps_the_path_and_the_writable_flag() {
    let p = pod("hostpath");
    println!("{:?}", p.host_path_mounts);
    assert_eq!(
        p.host_path_mounts,
        vec![
            HostPathMount {
                path: "/".to_string(),
                sub_path: Some("run/containerd".to_string()),
                sub_path_expr: None,
                read_only: false,
                container: "nosy".to_string(),
            },
            HostPathMount {
                path: "/".to_string(),
                sub_path: None,
                sub_path_expr: None,
                read_only: true,
                container: "shipper".to_string(),
            },
        ],
        "the node's whole filesystem, writable — both of rule 8's escalators, and \
         the finding has to name the container that has it"
    );

    let healthy = pod("healthy");
    assert!(
        healthy.host_path_mounts.is_empty(),
        "the projected service-account volume is not a hostPath: {:?}",
        healthy.host_path_mounts
    );
}

/// Rule 7 is "running but not receiving traffic". A crashlooping container is also
/// not ready, and it is rule 1's, not rule 7's — the state is what separates them.
///
/// It is also the pod that carries the one field rule 7 needs to *not* fire on a pod
/// that is merely still booting: the pod's `Ready` condition, and the moment it last
/// changed. `started` is **not** that field, and this pod is the proof — nothing in
/// it declares a `startupProbe`, so `started` is true because the container is
/// running and for no other reason (NOTES § D51).
#[test]
fn running_but_not_ready_is_distinguishable_from_waiting() {
    let raw = fixture("readiness");
    let readiness = pod("readiness");
    let c = container(&readiness, "app");
    println!(
        "readiness: ready={} started={} state={:?}\n  pod Ready: {:?}",
        c.ready, c.started, c.state, readiness.ready
    );
    assert_eq!(
        c.state,
        ContainerState::Running {
            started_at: Some(captured_time(
                captured_status(&raw, "containerStatuses", "app"),
                &["state", "running", "startedAt"]
            )),
        }
    );
    assert!(
        !c.ready,
        "the readiness probe is failing, so the Service dropped it"
    );
    assert!(
        c.started,
        "no `startupProbe` is declared here, so this is upstream's \"always true when \
         no startupProbe is defined and container is running\" case — it says nothing \
         about whether the container ever served, and a rule 7 built on it would fire \
         on every rolling update. The discrimination is the Ready condition below"
    );

    // The only "not ready since" there is: no container status carries one.
    let ready = readiness
        .ready
        .as_ref()
        .expect("a pod the kubelet has reached carries a Ready condition");
    assert_eq!(ready.type_, "Ready");
    assert_eq!(ready.status, "False");
    assert_eq!(ready.reason.as_deref(), Some("ContainersNotReady"));
    assert_eq!(
        ready.message.as_deref(),
        Some("containers with unready status: [app]")
    );
    assert_eq!(
        ready.last_transition,
        Some(captured_time(
            captured_condition(&raw, "Ready"),
            &["lastTransitionTime"]
        )),
        "without this, rule 7 also describes every rolling update inside its \
         initialDelaySeconds"
    );

    let crashloop = pod("crashloop");
    let c = container(&crashloop, "quitter");
    assert!(!c.ready);
    assert!(
        !matches!(c.state, ContainerState::Running { .. }),
        "rule 7 must not also fire on the crashlooping pod rule 1 already explains: \
         {:?}",
        c.state
    );
}

/// The `Ready` condition is picked by name off the same five-entry array `scheduled`
/// comes from, so the two must not be able to collapse onto one another. The healthy
/// pod is the discriminator: both of its conditions are `True`, and only the `type_`
/// and the transition time tell them apart. `broken-pending` is the third shape —
/// the kubelet never saw it, so `Ready` is absent while `PodScheduled` is present.
#[test]
fn ready_and_scheduled_are_two_different_conditions() {
    let raw = fixture("healthy");
    let p = pod("healthy");
    let ready = p.ready.as_ref().expect("a running pod reports Ready");
    let scheduled = p.scheduled.as_ref().expect("and it reports PodScheduled");
    println!("healthy ready={ready:?}\n  scheduled={scheduled:?}");

    assert_eq!(ready.type_, "Ready");
    assert_eq!(scheduled.type_, "PodScheduled");
    assert_eq!(ready.status, "True", "nothing is wrong with this pod");
    assert_ne!(
        ready, scheduled,
        "`Ready` is third in this pod's condition array and `PodScheduled` is fifth; \
         a decode taking the first, or the same one twice, reads identically here — \
         and the first is `PodReadyToStartContainers`, which any substring match on \
         \"Ready\" lands on"
    );
    assert_eq!(
        ready.last_transition,
        Some(captured_time(
            captured_condition(&raw, "Ready"),
            &["lastTransitionTime"]
        )),
        "the moment it started serving — rule 7's clock when it stops"
    );
    assert_eq!(
        scheduled.last_transition,
        Some(captured_time(
            captured_condition(&raw, "PodScheduled"),
            &["lastTransitionTime"]
        )),
        "and the moment a node accepted it, which is a different moment out of a \
         different entry of the same array"
    );

    let pending = pod("pending");
    println!("pending ready={:?}", pending.ready);
    assert_eq!(
        pending.ready, None,
        "no node accepted it, so the kubelet never wrote a Ready condition — and a \
         rule that read a missing condition as `False` would file rule 7 on top of \
         rule 10's answer"
    );
    assert!(
        pending.scheduled.is_some(),
        "while PodScheduled, the one rule 10 reads, is there"
    );
}

/// The negative side, and the one that catches false positives: nothing in here may
/// look like anything a rule fires on.
#[test]
fn the_healthy_pod_offers_no_rule_anything_to_fire_on() {
    let raw = fixture("healthy");
    let p = pod("healthy");
    println!("{:?}", p);

    assert_eq!(
        p.containers.len(),
        2,
        "an init container that succeeded, and the app"
    );
    let migrate = container(&p, "migrate");
    assert_eq!(migrate.role, ContainerRole::Init);
    let migrate_status = captured_status(&raw, "initContainerStatuses", "migrate");
    assert_eq!(
        migrate.state,
        ContainerState::Terminated(Terminated {
            reason: Some("Completed".to_string()),
            exit_code: 0,
            started_at: Some(captured_time(
                migrate_status,
                &["state", "terminated", "startedAt"]
            )),
            finished_at: Some(captured_time(
                migrate_status,
                &["state", "terminated", "finishedAt"]
            )),
            // This container writes nothing and its policy is the default `File`, so
            // there is no termination message to carry — the crashlooping container in
            // `crashloop.json` is where the populated case is asserted.
            message: None,
        }),
        "an init container that finished is terminated with exit 0, not a finding"
    );
    // The pair that kills a hardwired `started`: same pod, same decode, two answers.
    // An init container that ran to completion is not "started" — it is done.
    assert!(!migrate.started);

    let app = container(&p, "app");
    assert!(app.ready);
    assert!(app.started, "and the app container is");
    assert_eq!(
        app.state,
        ContainerState::Running {
            started_at: Some(captured_time(
                captured_status(&raw, "containerStatuses", "app"),
                &["state", "running", "startedAt"]
            )),
        }
    );
    // **The negative fixture is not a pod nothing has ever happened to** — but how much has
    // happened to it is a function of the capture session and not of the fixture.
    // `healthy.yaml` ends on a `sleep 3600`, so a trip that runs more than an hour photographs
    // the shell exiting 0 and the kubelet restarting it, and a shorter one photographs a
    // container that has never finished. Both are `scripts/cluster.sh` § `[healthy_init]`,
    // which asks for a `Running` pod with every container ready and nothing else; the
    // 2026-08-13 trip brought the first and the 2026-08-16 trip the second (NOTES § D114).
    //
    // So what is asserted is the **pair**, which holds at either count and is the decode
    // invariant worth having: the kubelet writes `restartCount` and `lastState` together —
    // `startContainer` increments the count when it leaves the record — so one without the
    // other is a decode that dropped a field or filled it from its neighbour.
    let app_status = captured_status(&raw, "containerStatuses", "app");
    assert_eq!(app.restarts, captured_i32(app_status, &["restartCount"]));
    assert!(
        app.restarts < RESTARTS_WARN,
        "the negative fixture has to stay under rule 5's band, or its silence is the \
         threshold's doing and not the pod's: {} restarts",
        app.restarts
    );
    assert_eq!(
        app.last_terminated.is_some(),
        app.restarts > 0,
        "a restart and the run it ended are one event the kubelet writes twice, and a decode \
         that carries one without the other has dropped a field: {} restarts, {:?}",
        app.restarts,
        app.last_terminated
    );
    if let Some(last) = app.last_terminated.as_ref() {
        assert_eq!(
            last.exit_code, 0,
            "and the run it records ended cleanly, which is rule 6's first exemption — the \
             one `exit0.json` proves fires on a container that is *not* serving"
        );
    }
    // **And the populated case is asserted over the corpus rather than over this one pod**,
    // because this pod is no longer guaranteed to hold it. A `lastState` beside a `Running`
    // state is the shape the branch above goes quiet on, and it has to exist somewhere or the
    // `if` is a test that cannot fail (CLAUDE.md § A derived list asserts it found something).
    let with_history: Vec<String> = CAPTURED_PODS
        .iter()
        .flat_map(|n| {
            pod(n)
                .containers
                .into_iter()
                .filter(|c| {
                    matches!(c.state, ContainerState::Running { .. }) && c.last_terminated.is_some()
                })
                .map(move |c| format!("{n}/{}", c.name))
        })
        .collect();
    println!("running containers carrying a previous run: {with_history:?}");
    assert!(
        !with_history.is_empty(),
        "no committed capture decodes a previous run beside a running container, so the \
         branch above is guarding a shape the corpus cannot produce"
    );
    // The absent case, on the container of this same pod that has never been restarted:
    // `lastState: {}` decodes to `None` and not to a zero-filled `Terminated`.
    assert_eq!(
        migrate.last_terminated, None,
        "an init container that succeeded on its first try has no previous run"
    );
    assert_eq!(app.memory_limit.as_deref(), Some("64Mi"));
    assert_eq!(app.cpu_request.as_deref(), Some("10m"));
    // The only container in any capture whose request and limit differ, so it is the
    // only place a request read out of `limits` can be caught. `broken-oom` sets both
    // to 64Mi, which is what a burstable-looking pod that is really guaranteed does.
    assert_eq!(
        app.memory_request.as_deref(),
        Some("16Mi"),
        "N5 sums what was reserved, not what was capped — they are different numbers"
    );
    // The condition does not go away once the pod is scheduled, it flips to True — so
    // rule 10 has to test `status` and `reason`, never "is it there".
    let scheduled = p
        .scheduled
        .as_ref()
        .expect("a scheduled pod keeps the condition, it does not drop it");
    // **This pod is the one that proves the condition was picked by name.** Its five
    // conditions lead with `PodReadyToStartContainers` and end with `PodScheduled`,
    // so a decode that simply took the first would land on the wrong one here — and
    // on `broken-pending`, which carries only `PodScheduled`, it never could.
    assert_eq!(scheduled.type_, "PodScheduled");
    assert_eq!(scheduled.status, "True");
    assert_eq!(
        scheduled.reason, None,
        "nothing refused it, so there is no reason to give"
    );

    assert!(p.node_selector.is_empty());
    assert!(
        !p.mirror,
        "an ordinary pod is not a static one, and N2 counts it as drainable"
    );
    // Not "no tolerations": the admission controller adds these two to every pod in
    // the cluster, so N6 must not read a pod that tolerates nothing as one that
    // tolerates something. Asserted whole — dropping `operator` or `effect` would
    // leave N6 unable to say whether a taint is tolerated, which is its only job.
    assert_eq!(
        p.tolerations,
        vec![
            Toleration {
                key: Some("node.kubernetes.io/not-ready".to_string()),
                operator: Some("Exists".to_string()),
                value: None,
                effect: Some("NoExecute".to_string()),
            },
            Toleration {
                key: Some("node.kubernetes.io/unreachable".to_string()),
                operator: Some("Exists".to_string()),
                value: None,
                effect: Some("NoExecute".to_string()),
            },
        ]
    );
}

/// N1–N6 all read this object, and N5/N6 join it against the pods.
///
/// **The capture is no longer of a healthy cluster, and that is the point of it.**
/// `scripts/cluster.sh break-nodes` hands each worker exactly one broken state —
/// cordoned (N2), `NoExecute`-tainted (N6), kubelet stopped (N1) — one per node so
/// that no fixture wears two, which is why this reads four nodes where it once read
/// three and why the "nothing is wrong anywhere" loop it used to end with has been
/// replaced by a comparison against what the capture actually says.
#[test]
fn nodes_decode_with_their_conditions_taints_and_versions() {
    let raw = fixture("nodes");
    let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
    println!(
        "{:?}",
        nodes
            .iter()
            .map(|n| (&n.id.name, n.unschedulable, &n.kubelet_version))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        nodes.len(),
        raw["items"].as_array().map_or(0, Vec::len),
        "every node in the capture decodes into one snapshot — N2 and N5 are joins \
         over this list, and a node quietly dropped from it is a join that closes \
         over the wrong denominator"
    );
    assert!(
        nodes.len() >= 4,
        "`break-nodes` gives each worker one of the three broken states and needs \
         three workers to do it, so a control plane and three workers is the floor \
         this fixture is captured at: got {}",
        nodes.len()
    );

    let cp = nodes
        .iter()
        .find(|n| n.id.name == "k8rs-control-plane")
        .expect("the control plane is in the capture");
    assert_eq!(cp.id.kind, ObjectKind::Node);
    assert_eq!(
        cp.id.namespace, None,
        "a node is cluster-scoped, and `\"\"` is a lie"
    );
    let cp_raw = captured_item(&raw, "k8rs-control-plane");
    assert_eq!(
        cp.kubelet_version.as_deref(),
        Some(captured_str(
            cp_raw,
            &["status", "nodeInfo", "kubeletVersion"]
        )),
        "N4 compares this"
    );
    assert_eq!(
        cp.kubelet_version.as_deref(),
        Some(
            std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/K8S_VERSION"
            ))
            .expect("the capture stamps the version it came from")
            .trim()
        ),
        "N4 is 'this kubelet is behind the API server', and on this cluster nothing \
         is — a fixture where they already disagree is N4's positive case, and it \
         would want saying out loud rather than arriving by accident"
    );
    // Allocatable is the machine the capture ran on and moves with it — the first
    // capture came off a 4-CPU box and this one off the dev machine.
    assert_eq!(
        cp.allocatable_cpu.as_deref(),
        Some(captured_str(cp_raw, &["status", "allocatable", "cpu"])),
        "N5 measures against this"
    );
    assert_eq!(
        cp.allocatable_memory.as_deref(),
        Some(captured_str(cp_raw, &["status", "allocatable", "memory"]))
    );
    assert_eq!(
        cp.taints,
        vec![Taint {
            key: "node-role.kubernetes.io/control-plane".to_string(),
            value: None,
            effect: "NoSchedule".to_string(),
            added_at: None,
        }],
        "N6 explains a Pending pod with this"
    );
    assert_eq!(
        cp.labels.get("kubernetes.io/hostname").map(String::as_str),
        Some("k8rs-control-plane"),
        "N6 matches a pod's nodeSelector against the labels"
    );

    let ready = cp
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .expect("N1 reads the Ready condition");
    assert_eq!(ready.status, "True");
    assert_eq!(
        ready.last_transition,
        Some(captured_time(
            captured_condition(cp_raw, "Ready"),
            &["lastTransitionTime"]
        )),
        "N1 needs how long it has been that way, not just that it is"
    );

    // **What the decode says about every node is what the capture says**, condition by
    // condition and cordon by cordon. This replaces an assertion that the whole
    // cluster was healthy: that was true of the first capture and is deliberately
    // false of this one, but the defect it was written against — a decode that
    // invented a pressure, dropped a condition or answered one node's for another's —
    // is caught by the comparison and not by the optimism.
    let decoded: Vec<(&str, &str, &str)> = nodes
        .iter()
        .flat_map(|n| {
            n.conditions
                .iter()
                .map(|c| (n.id.name.as_str(), c.type_.as_str(), c.status.as_str()))
        })
        .collect();
    let captured: Vec<(&str, &str, &str)> = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|n| {
            n["status"]["conditions"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    (
                        captured_str(n, &["metadata", "name"]),
                        captured_str(c, &["type"]),
                        captured_str(c, &["status"]),
                    )
                })
        })
        .collect();
    println!("{decoded:?}");
    assert_eq!(
        decoded, captured,
        "N1 and N3 read these, and the decode may neither invent one nor lose one"
    );

    let cordoned: Vec<&str> = nodes
        .iter()
        .filter(|n| n.unschedulable)
        .map(|n| n.id.name.as_str())
        .collect();
    let captured_cordoned: Vec<&str> = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .collect();
    println!("cordoned: {cordoned:?}");
    assert_eq!(
        cordoned, captured_cordoned,
        "N2 is the whole of 'cordoned and forgotten', and this field is all of it — \
         an absent key is a schedulable node, never a cordoned one"
    );

    // The three states `break-nodes` puts on the cluster, asserted as three because a
    // capture that produced only two of them is a fixture the N-series cannot be
    // written against — and every one of them reads as "healthy" to a decode that
    // dropped the field it lives in.
    assert_eq!(
        cordoned.len(),
        1,
        "one worker is cordoned and still carrying pods (N2's positive): {cordoned:?}"
    );
    let not_ready: Vec<&str> = nodes
        .iter()
        .filter(|n| {
            n.conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status != "True")
        })
        .map(|n| n.id.name.as_str())
        .collect();
    println!("not ready: {not_ready:?}");
    assert_eq!(
        not_ready.len(),
        1,
        "and one worker's kubelet has stopped posting (N1's positive): {not_ready:?}"
    );
    let gone = nodes
        .iter()
        .find(|n| n.id.name == not_ready[0])
        .expect("just found by name");
    assert_eq!(
        gone.conditions
            .iter()
            .find(|c| c.type_ == "Ready")
            .map(|c| c.status.as_str()),
        Some("Unknown"),
        "`Unknown` is a kubelet that went quiet and `False` is one that answered and \
         said no — N1 must be able to tell the operator which of the two happened"
    );
    let tainted: Vec<&Taint> = nodes
        .iter()
        .flat_map(|n| &n.taints)
        .filter(|t| t.effect == "NoExecute" && t.value.is_some())
        .collect();
    println!("valued NoExecute taints: {tainted:?}");
    // Asserted whole, because N6's job is to name the taint that is blocking a pod and
    // `dedicated=gpu:NoExecute` is three fields, not one. This is also the **only
    // captured proof that `value` survives at all**: every other taint in the capture
    // was written by the node controller, which gives its taints no value, so the
    // control plane's `value: None` above is an absence and this is the presence.
    // `cluster.sh break-nodes` applies it, so the string is the script's, not the
    // cluster's.
    let operators = Taint {
        key: "dedicated".to_string(),
        value: Some("gpu".to_string()),
        effect: "NoExecute".to_string(),
        // Absent, and that is the split: a hand-applied taint carries no `timeAdded`
        // (see [`Taint::added_at`]), which is why the pair of fields together can only
        // be asserted on a synthesis.
        added_at: None,
    };
    assert_eq!(
        tainted,
        vec![&operators],
        "and one worker carries an operator's own `dedicated=gpu:NoExecute` (N6's \
         positive, and the only taint in the capture with a value)"
    );

    // N3's positive is `True`, and this capture has none: the unreachable node's
    // pressures are `Unknown`, which is N1's answer and not N3's. A decode that read
    // an absent or unknown condition as a pressure would file evictions-are-coming on
    // a node nobody can reach.
    for n in &nodes {
        for c in &n.conditions {
            let pressured = matches!(
                c.type_.as_str(),
                "MemoryPressure" | "DiskPressure" | "PIDPressure"
            ) && c.status == "True";
            assert!(
                !pressured,
                "{} reported {} = {} (N3)",
                n.id.name, c.type_, c.status
            );
        }
    }
}

/// W1: the pods were never created, so the only object that knows anything is the
/// ReplicaSet. It is also the one capture in the repo with a real `ownerReference`,
/// so it is what proves the owner decode reads one at all.
#[test]
fn the_failed_replicaset_carries_the_quota_message_and_files_under_its_deployment() {
    let rs: Vec<WorkloadSnapshot> = items::<ReplicaSet>("quota-replicasets")
        .into_iter()
        .map(Into::into)
        .collect();
    let rs = rs.first().expect("the quota namespace has one ReplicaSet");
    println!(
        "{:?}\n  owner: {:?}\n  {:?}",
        rs.id, rs.owner, rs.conditions
    );

    assert_eq!(rs.id.kind, ObjectKind::ReplicaSet);
    assert_eq!(rs.id.name, "broken-quota-59654c756");
    assert_eq!(rs.id.namespace.as_deref(), Some("k8rs-quota"));

    assert_eq!(
        rs.owner.kind,
        ObjectKind::Deployment,
        "the chain goes up one step"
    );
    assert_eq!(
        rs.owner.name, "broken-quota",
        "the card reads the name the user deployed, not the hashed one (D28)"
    );
    let raw = fixture("quota-replicasets");
    let controller = raw["items"][0]["metadata"]["ownerReferences"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|o| o["controller"] == true)
        .expect("the capture's ReplicaSet is controlled by its Deployment");
    assert_eq!(
        rs.owner.uid.as_deref(),
        Some(captured_str(controller, &["uid"]))
    );
    assert_ne!(
        rs.owner.uid, rs.id.uid,
        "the owner's own uid and never the object's — the group agrees on the \
         owner's (D39), and the two are one field apart in the JSON"
    );
    assert_eq!(
        rs.owner.namespace, rs.id.namespace,
        "an ownerReference carries no namespace; the owner is in the object's own"
    );

    assert_eq!(rs.desired, Some(1));
    assert_eq!(
        rs.ready, None,
        "not one pod exists — this is the blind spot D28 closed"
    );
    let failure = rs
        .conditions
        .iter()
        .find(|c| c.type_ == "ReplicaFailure")
        .expect("W1 reads ReplicaFailure");
    assert_eq!(failure.status, "True");
    assert_eq!(failure.reason.as_deref(), Some("FailedCreate"));
    // Verbatim means verbatim (D37) — `contains` would pass on a message the decode
    // had appended to or cut short, and this one is the whole of W1's evidence. The
    // sentence names the pod the ReplicaSet tried to create, whose generated suffix is
    // new on every capture, so the comparison is against the capture's own bytes and
    // what W1 needs of them is asserted beside it.
    assert_eq!(
        failure.message.as_deref(),
        Some(captured_str(
            captured_condition(&raw["items"][0], "ReplicaFailure"),
            &["message"]
        )),
        "W1 shows the API server's own refusal, word for word"
    );
    assert!(
        failure
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("exceeded quota: deny-all-pods"),
        "and the refusal has to still name the quota that refused it, or W1's finding \
         sends the operator looking for a broken image: {:?}",
        failure.message
    );

    // The negative side: a ReplicaSet that worked has no such condition at all.
    let healthy: Vec<WorkloadSnapshot> = items::<ReplicaSet>("healthy-replicasets")
        .into_iter()
        .map(Into::into)
        .collect();
    let healthy = healthy
        .first()
        .expect("the healthy Deployment has one ReplicaSet");
    assert_eq!(healthy.owner.name, "healthy-deploy");
    assert_eq!((healthy.desired, healthy.ready), (Some(2), Some(2)));
    assert!(
        healthy.conditions.is_empty(),
        "no failure means no condition, not a condition saying False: {:?}",
        healthy.conditions
    );
}

/// W2 reads a Deployment's `Progressing`; a DaemonSet counts its pods somewhere else
/// entirely, which is the whole reason four kinds get four decodes.
#[test]
fn deployments_and_daemonsets_decode_their_own_desired_and_ready() {
    let deployments: Vec<WorkloadSnapshot> = items::<Deployment>("deployments")
        .into_iter()
        .map(Into::into)
        .collect();
    println!(
        "{:?}",
        deployments
            .iter()
            .map(|w| (&w.id.name, w.desired, w.ready, w.updated))
            .collect::<Vec<_>>()
    );

    let broken = deployments
        .iter()
        .find(|w| w.id.name == "broken-quota")
        .expect("the quota Deployment is in the capture");
    assert_eq!(broken.id.kind, ObjectKind::Deployment);
    assert_eq!(
        broken.owner, broken.id,
        "nothing controls a Deployment, so it is its own card"
    );
    assert_eq!(
        (broken.desired, broken.ready, broken.updated),
        (Some(1), None, None),
        "no pod of this Deployment was ever created, so neither counter is written at all"
    );
    // **That `None` is a zero, and the capture is what says so.** `readyReplicas` is
    // `omitempty` on a plain `int32`, so it is absent exactly when it is 0 — provable
    // here from the counter the API server *did* write: one replica wanted, one
    // unavailable, therefore none ready. Asserted rather than described, because a
    // W2 reading `None` as "no number" would skip precisely this Deployment.
    let raw = fixture("deployments");
    let status = &raw["items"]
        .as_array()
        .expect("deployments.json has an items array")
        .iter()
        .find(|w| w["metadata"]["name"] == "broken-quota")
        .expect("the quota Deployment is in the capture")["status"];
    let (ready_key, unavailable) = (status.get("readyReplicas"), &status["unavailableReplicas"]);
    println!("broken-quota: readyReplicas={ready_key:?} unavailableReplicas={unavailable}");
    assert!(
        ready_key.is_none() && *unavailable == 1,
        "the shape `ready: None` means zero rests on: no readyReplicas key, and one \
         of one replica unavailable — got {ready_key:?} and {unavailable}"
    );
    let progressing = broken
        .conditions
        .iter()
        .find(|c| c.type_ == "Progressing")
        .expect("W2 reads Progressing");
    assert_eq!(progressing.status, "False");
    assert_eq!(
        progressing.reason.as_deref(),
        Some("ProgressDeadlineExceeded"),
        "W2 fires on this reason and no other"
    );

    // The negative side of W2, from the same capture.
    let healthy = deployments
        .iter()
        .find(|w| w.id.name == "healthy-deploy")
        .expect("the healthy Deployment is in the capture");
    assert_eq!(
        (healthy.desired, healthy.ready, healthy.updated),
        (Some(2), Some(2), Some(2)),
        "and a rollout that finished has every pod on the new template — the state \
         `short_of_pods` has to read as *not* short on all three counters"
    );
    assert_eq!(
        healthy
            .conditions
            .iter()
            .find(|c| c.type_ == "Progressing")
            .and_then(|c| c.reason.as_deref()),
        Some("NewReplicaSetAvailable"),
        "a rollout that finished must not look like one that gave up"
    );

    let daemonsets: Vec<WorkloadSnapshot> = items::<DaemonSet>("daemonsets")
        .into_iter()
        .map(Into::into)
        .collect();
    let kindnet = daemonsets
        .iter()
        .find(|w| w.id.name == "kindnet")
        .expect("kind runs kindnet on every node");
    println!(
        "kindnet: desired={:?} ready={:?}",
        kindnet.desired, kindnet.ready
    );
    assert_eq!(kindnet.id.kind, ObjectKind::DaemonSet);
    assert_eq!(kindnet.id.namespace.as_deref(), Some("kube-system"));
    // **Both numbers out of the DaemonSet's own status keys.** A DaemonSet has no
    // `spec.replicas` to read and wants one pod per matching node, so the pair is the
    // size of the cluster and moves with it — it was `(3, 3)` while the fixture
    // cluster had two workers and is `(4, 4)` now that `break-nodes` needs three. What
    // must not move is which field each comes out of, and that is what is asserted;
    // that neither is a *neighbouring* counter is
    // `desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour`.
    let raw = fixture("daemonsets");
    let kindnet_raw = captured_item(&raw, "kindnet");
    assert_eq!(
        (kindnet.desired, kindnet.ready),
        (
            Some(captured_i32(
                kindnet_raw,
                &["status", "desiredNumberScheduled"]
            )),
            Some(captured_i32(kindnet_raw, &["status", "numberReady"])),
        ),
        "a DaemonSet has no spec.replicas: both numbers come from its status"
    );

    // **The third kind, and it had no object at all until the 2026-08-13 trip.**
    // `statefulsets.json` was an empty list, so `From<StatefulSet>` shipped with no test that
    // could fail and synthesizing a whole StatefulSet would have been the hand-written JSON
    // CLAUDE.md forbids (NOTES § D40). `broken-sts` is partially ready, which is the one state
    // that separates `spec.replicas` from `status.readyReplicas` — an all-ready object reads
    // the same whichever field either number came from.
    let statefulsets: Vec<WorkloadSnapshot> = items::<StatefulSet>("statefulsets")
        .into_iter()
        .map(Into::into)
        .collect();
    let sts = statefulsets
        .iter()
        .find(|w| w.id.name == "broken-sts")
        .expect("the partially ready StatefulSet is in the capture");
    println!(
        "broken-sts: desired={:?} ready={:?}",
        sts.desired, sts.ready
    );
    assert_eq!(sts.id.kind, ObjectKind::StatefulSet);
    assert_eq!(
        sts.owner, sts.id,
        "nothing controls a StatefulSet, so it is its own card"
    );
    let sts_json = fixture("statefulsets");
    let sts_raw = captured_item(&sts_json, "broken-sts");
    assert_eq!(
        (sts.desired, sts.ready),
        (
            Some(captured_i32(sts_raw, &["spec", "replicas"])),
            Some(captured_i32(sts_raw, &["status", "readyReplicas"])),
        ),
        "a StatefulSet wants what its spec says and reports what its status says, and the two \
         disagree on this object — which is what makes reading either from the other visible"
    );
    assert_ne!(
        sts.desired, sts.ready,
        "a capture where every replica is ready cannot tell the two fields apart, and is not \
         the fixture for this assertion"
    );
    assert_eq!(
        sts.terminating, None,
        "and a StatefulSet has no `terminatingReplicas` at all — KEP-3973 wrote it onto \
         Deployments and ReplicaSets only, so this kind's `readyReplicas` still counts a pod on \
         its way out and needs no correction ([`WorkloadSnapshot::terminating`])"
    );

    // **`terminatingReplicas` off the two kinds that have it, and it is `0` on every committed
    // object** — the counter is non-zero only while a rollout is draining and no capture landed
    // inside that window (NOTES § D135). The value is read out of the capture rather than
    // written down, so a trip that *does* catch one reddens this line instead of quietly
    // becoming the fixture for a case nobody chose; and `captured_i32` panics on an absent key,
    // which is what makes this an assertion that the beta field was on at all rather than a
    // comparison of `None` against `None`.
    let deployments_raw = fixture("deployments");
    for w in &deployments {
        let captured = captured_i32(
            captured_item(&deployments_raw, &w.id.name),
            &["status", "terminatingReplicas"],
        );
        assert_eq!(
            (w.terminating, captured),
            (Some(0), 0),
            "{}: the field is present in the capture and decodes off its own path — and the \
             corpus has no draining workload, which is what [`ready_count`]'s clause has no \
             committed object for and what its negative in `rules_tests/workload.rs` stands on",
            w.id.name
        );
    }
    let quota_rs: Vec<WorkloadSnapshot> = items::<ReplicaSet>("quota-replicasets")
        .into_iter()
        .map(Into::into)
        .collect();
    let quota_rs = quota_rs
        .first()
        .expect("the quota namespace has one ReplicaSet");
    assert_eq!(
        (
            quota_rs.terminating,
            captured_i32(
                &fixture("quota-replicasets")["items"][0],
                &["status", "terminatingReplicas"]
            )
        ),
        (Some(0), 0),
        "and the ReplicaSet half of KEP-3973 decodes off the same path — a bare ReplicaSet is a \
         workload W1 reads its band off, so the field cannot be a Deployment-only decode"
    );
}

/// **[`CAPTURED_PODS`] against the directory, in both directions** — the coupling the list never
/// had, and without which *"over the whole committed corpus"* means *over the 31 names somebody
/// remembered to type*.
///
/// **It was not a theoretical hole.** `tester` committed a fixture derived from `restarts10.json`
/// — a `Running` pod with a **Regular** container in `state.terminated` at `exit 3`, which
/// directly falsifies what
/// [`every_captured_container_sitting_in_a_terminated_run_is_a_finished_init_container`] claims
/// about the corpus — and the suite stayed green at 188, because nothing read the directory.
/// `scripts/fixture-audit.sh` counted 51 files instead of 50 and passed, correctly: it audits
/// sanitization, not membership.
///
/// **Both directions.** A capture that lands and is not listed is a capture no test reads; a name
/// left behind after its file goes is a `CAPTURED_PODS` entry that panics in [`fixture`] the next
/// time anyone touches it.
///
/// **`kind: Pod` and not "a file with pods in it"**: a `List` cannot decode into that array, so
/// the single-object captures and the `kubectl get pods` captures are two arrays and the sweep
/// checks both. [`CAPTURED_POD_LISTS`] is the second, and it is why this test now walks the
/// directory twice. **There is no exclusion list, because nothing is excluded** — if a Pod
/// capture ever has to stay out, it goes in a named list with its reason beside it, never in the
/// gap between this assertion and an array.
///
/// **The `List` half is here because its absence cost a trip** (NOTES § D131). Until 2026-08-21
/// only `kind: Pod` was swept, so a `List` of pods was coupled to nothing: `owned-pods.json` sat
/// outside [`every_captured_pod`] for weeks and `healthy-deploy-pods.json` arrived outside it,
/// which left [`the_pods_the_blocking_budget_protects_and_the_ones_no_budget_can_be_joined_to`]
/// green on a trip that had just captured the two pods it was written to notice.
///
/// **What it still cannot see is an empty `List`**, which carries no `items[].kind` to be
/// identified by — `kubectl` stamps the kind on the items and on nothing else. A capture of zero
/// pods is a capture with no shape for any test here, and the assertion says so rather than
/// pretending otherwise.
#[test]
fn every_committed_pod_capture_is_named_in_the_list_that_claims_to_hold_them_all() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let stems: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{dir} could not be read: {e}"))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|x| x == "json"))
        .map(|path| {
            path.file_stem()
                .expect("a .json path has a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    // [`fixture`] does the read and the parse, and panics with the path on either.
    let holds_pods = |stem: &String| {
        let capture = fixture(stem);
        capture["kind"] == "List"
            && capture["items"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item["kind"] == "Pod")
    };
    let (mut lists, mut singles): (Vec<String>, Vec<String>) =
        stems.into_iter().partition(holds_pods);
    singles.retain(|stem| fixture(stem)["kind"] == "Pod");
    lists.sort();
    singles.sort();

    let mut named_singles: Vec<String> = CAPTURED_PODS.iter().map(|n| (*n).to_string()).collect();
    let mut named_lists: Vec<String> = CAPTURED_POD_LISTS
        .iter()
        .map(|n| (*n).to_string())
        .collect();
    named_singles.sort();
    named_lists.sort();
    println!(
        "on disk: {} Pod captures, {} pod Lists {lists:?} · named: {} and {}",
        singles.len(),
        lists.len(),
        named_singles.len(),
        named_lists.len()
    );

    // A found-nothing sweep and a nothing-to-find sweep print the same line, and a `read_dir`
    // that returned nothing would satisfy both equalities below against emptied arrays. Canaries
    // rather than a count, so a capture legitimately retired does not redden this line with a
    // message about the wrong thing (CLAUDE.md § Code phase rules). **One per array**, because
    // the two halves are two sweeps and a canary in one says nothing about the other.
    for canary in ["healthy", "healthy-retry", "failed"] {
        assert!(
            singles.iter().any(|n| n == canary),
            "{canary}.json is a Pod capture this file reads by name and the sweep did not find \
             it, so the sweep is not reading {dir}: {singles:?}"
        );
    }
    for canary in ["kube-system-pods", "healthy-deploy-pods"] {
        assert!(
            lists.iter().any(|n| n == canary),
            "{canary}.json is a `kubectl get pods` capture this file joins on and the sweep did \
             not classify it as one, so the `List` half is reading nothing: {lists:?}"
        );
    }
    assert_eq!(
        singles, named_singles,
        "`tests/fixtures` and `CAPTURED_PODS` disagree. A capture on disk and not in the array is \
         one no test reads — and it can falsify a claim this file makes about the corpus without \
         reddening anything (NOTES § D96). A name in the array with no file is an entry that \
         panics the moment it is read."
    );
    assert_eq!(
        lists, named_lists,
        "`tests/fixtures` and `CAPTURED_POD_LISTS` disagree. A `List` of pods on disk and not in \
         the array is a set of pods outside [`every_captured_pod`], which is a set of pods \
         outside every node-rule join — a per-node sum computed without them is wrong and \
         nothing else here would say so (NOTES § D46, § D131)."
    );
}

/// The snapshot is what a rule is handed, and N5 and N6 are joins across it — so the
/// join has to close. `scripts/sanitize.jq` refuses to rewrite node names for exactly
/// this reason, and this is the assertion that would notice if it ever did.
#[test]
fn a_cluster_snapshot_joins_its_pods_to_the_nodes_they_run_on() {
    let snapshot = fixture_snapshot();
    println!(
        "{} pods, {} nodes, {} workloads, server {:?}",
        snapshot.pods.len(),
        snapshot.nodes.len(),
        snapshot.workloads.len(),
        snapshot.server_version
    );

    assert_eq!(snapshot.pods.len(), CAPTURED_PODS.len());
    assert_eq!(
        snapshot.server_version.as_deref(),
        Some("v1.36.1"),
        "N4 compares the kubelets against this"
    );

    let mut unplaced = Vec::new();
    for p in &snapshot.pods {
        let Some(node) = &p.node else {
            unplaced.push(p.id.name.as_str());
            continue;
        };
        assert!(
            snapshot.nodes.iter().any(|n| &n.id.name == node),
            "{} says it runs on {node}, which is in no NodeSnapshot — N5 and N6 cannot join",
            p.id.name
        );
    }
    println!("no machine took: {unplaced:?}");
    assert!(
        !unplaced.is_empty() && unplaced.len() < snapshot.pods.len(),
        "the capture has to hold both kinds — a pod with no `nodeName` is what makes the \
         join's `else` reachable, and a capture of nothing but those would make the join \
         itself untested: {unplaced:?}"
    );
}

/// One swept timestamp: the field it came from, the value, and the grace that has to
/// come back off it before it names a moment.
///
/// **The grace is `Some` for exactly one field, and that is the whole point of the third
/// slot.** Every other label is a moment that has already happened, so the value *is*
/// the moment. [`PodSnapshot::deletion_timestamp`] is a **deadline** — request time
/// *plus* grace — so it legitimately points at the future for a pod inside its grace
/// period, and comparing it against `now` would reject that pod and blame the clock.
type Swept<'a> = (&'static str, &'a Time, Option<i64>);

/// One entry per `Some`, labelled with the field it came from, and no grace — every
/// caller of this is a field whose value is already a moment. A `None` contributes
/// nothing at all, which is exactly why the labels are asserted separately below.
fn labelled<'a>(out: &mut Vec<Swept<'a>>, label: &'static str, t: Option<&'a Time>) {
    out.extend(t.map(|t| (label, t, None)));
}

/// `Terminated` hangs in two places — a container's current state, and the run before
/// this one — and **the label pair comes from the caller so those two walks are named
/// apart**. Sharing one pair made either walk satisfy the set on its own: deleting the
/// `ContainerState::Terminated` arm lost 2 timestamps and deleting the
/// `last_terminated` walk lost 8, both silently green, because the other walk kept
/// filling the same two labels.
fn terminated_times<'a>(
    out: &mut Vec<Swept<'a>>,
    started: &'static str,
    finished: &'static str,
    t: &'a Terminated,
) {
    labelled(out, started, t.started_at.as_ref());
    labelled(out, finished, t.finished_at.as_ref());
}

/// Every `Time` a [`ClusterSnapshot`] exposes, each carrying the name of the field it
/// was read out of.
fn snapshot_times(s: &ClusterSnapshot) -> Vec<Swept<'_>> {
    let mut out = Vec::new();
    for p in &s.pods {
        // Both or neither. The apiserver writes `deletionTimestamp` and the grace it
        // granted in the same accepted delete, so a deadline with no grace beside it
        // is not a shape it produces — and if one ever arrives, the label goes
        // unreached and the coverage assertion names it, which is louder than
        // guessing a grace of zero and asserting the deadline itself.
        if let (Some(dt), Some(grace)) = (&p.deletion_timestamp, p.grace_period_seconds) {
            out.push(("pod.deletion_timestamp", dt, Some(grace)));
        }
        labelled(
            &mut out,
            "pod.creation_timestamp",
            p.creation_timestamp.as_ref(),
        );
        for c in [&p.scheduled, &p.ready, &p.ready_to_start_containers]
            .into_iter()
            .flatten()
        {
            labelled(
                &mut out,
                "pod.condition.last_transition",
                c.last_transition.as_ref(),
            );
        }
        for c in &p.containers {
            match &c.state {
                ContainerState::Running { started_at } => labelled(
                    &mut out,
                    "container.state.running.started_at",
                    started_at.as_ref(),
                ),
                ContainerState::Terminated(t) => terminated_times(
                    &mut out,
                    "container.state.terminated.started_at",
                    "container.state.terminated.finished_at",
                    t,
                ),
                // A waiting container has no time of its own: it is not running, and
                // the run that ended is `last_terminated` below.
                ContainerState::Waiting { .. } => {}
            }
            if let Some(t) = &c.last_terminated {
                terminated_times(
                    &mut out,
                    "container.last_terminated.started_at",
                    "container.last_terminated.finished_at",
                    t,
                );
            }
        }
    }
    for n in &s.nodes {
        for c in &n.conditions {
            labelled(
                &mut out,
                "node.condition.last_transition",
                c.last_transition.as_ref(),
            );
        }
        for t in &n.taints {
            labelled(&mut out, "node.taint.added_at", t.added_at.as_ref());
        }
    }
    for w in &s.workloads {
        for c in &w.conditions {
            labelled(
                &mut out,
                "workload.condition.last_transition",
                c.last_transition.as_ref(),
            );
        }
    }
    out
}

/// **A pin behind the timestamps the snapshot exposes makes every duration in the suite
/// run backwards, and nothing else here would notice.** `now` is the user's laptop and
/// the fixture timestamps are the API server's, so an early pin enters D55's *slow* half
/// permanently and by construction: rule 12 computes "asked to shut down in 43 minutes",
/// and the renderer draws the whole suite as "just now". Every other assertion in this
/// file reads a field rather than subtracting two times, so this is the only place the
/// pin can be wrong out loud.
///
/// **The sweep is labelled, not counted**, because a bare total cannot tell every field
/// walked once from one field walked ninety-six times, and a sweep that reached nothing
/// prints the same green line as one with nothing to reach (CLAUDE.md — a derived list
/// asserts it found something). Each walk is named separately: deleting any one turns
/// this red.
///
/// **What it does *not* buy is a guard against a new field.** A new *variant* is caught
/// by the compiler — the sweep's `match &c.state` is exhaustive — but a new **field** is
/// caught by nothing: adding a `Time` to [`PodSnapshot`] and decoding it leaves this
/// green on the labels it already had. **A box that adds a `Time` to these types adds
/// its walk here in the same change**, and no mechanism will remind it.
///
/// **This is a guard over the contract, not over the captures**, and four fields sit in
/// the gap: `metadata.creationTimestamp` on every object that is not a Pod, a pod's
/// `status.startTime`, and the two [`Condition`] keeps no room for —
/// `NodeCondition.lastHeartbeatTime` and `DeploymentCondition.lastUpdateTime`. All four
/// sit before the pin today and nothing asserts that they do; NOTES § D42 lets Phase 4
/// add any of them.
#[test]
fn the_pinned_now_is_not_before_the_captures_it_is_read_against() {
    // **[`every_captured_pod`] and not [`fixture_snapshot`]'s own list**, which is the narrower
    // one: the three `kubectl get pods` captures hold timestamps too, and until 2026-08-21
    // nothing compared them against the pin — 86 of the 339 moments this walks were never
    // checked against it (NOTES § D131). `owned-pods.json` had been outside this walk for weeks.
    let snapshot = ClusterSnapshot {
        pods: every_captured_pod(),
        ..fixture_snapshot()
    };
    let times = snapshot_times(&snapshot);
    let reached: BTreeSet<&str> = times.iter().map(|&(label, _, _)| label).collect();
    println!(
        "now {:?}\n  {} timestamps, newest {:?}\n  fields reached: {reached:?}",
        snapshot.now,
        times.len(),
        times.iter().map(|&(_, t, _)| t).max(),
    );

    // A superset, not an equality: reaching *more* than this is a new walk over a field
    // the captures started filling, which is right and must not be a red build.
    // Reaching *less* is the defect, and that is all this asks about.
    let expected = BTreeSet::from([
        "container.last_terminated.finished_at",
        "container.last_terminated.started_at",
        "container.state.running.started_at",
        "container.state.terminated.finished_at",
        "container.state.terminated.started_at",
        "node.condition.last_transition",
        "node.taint.added_at",
        "pod.condition.last_transition",
        "pod.creation_timestamp",
        "pod.deletion_timestamp",
        "workload.condition.last_transition",
    ]);
    assert!(
        reached.is_superset(&expected),
        "the sweep no longer reaches {:?} — every Time field the captures fill has to \
         be walked, because a sweep that reached nothing prints the same green line as \
         one with nothing to reach, and the loop below is just as quiet either way",
        expected.difference(&reached).collect::<Vec<_>>()
    );

    for &(label, t, grace) in &times {
        // What has to be in the past is the moment the thing *happened*. For ten of
        // the eleven labels that is the value; for the deadline it is the value minus
        // the grace it was granted — D46's `asked_at`.
        let moment = match grace {
            None => t.0,
            // Checked, and the failure is named rather than skipped: a grace this
            // subtraction cannot represent is reachable from the cluster, not
            // theoretical — v1.36.1 accepted `terminationGracePeriodSeconds:
            // 9223372036854775807` in a server-side dry-run (NOTES § D56).
            Some(g) => {
                t.0.checked_sub(SignedDuration::from_secs(g))
                    .unwrap_or_else(|e| {
                        panic!(
                            "{label} is {t:?} with a grace of {g}s, and taking the grace \
                         back off it cannot be represented: {e}\n  \
                         `grep -rl {g} tests/fixtures` names the capture, if one carries it."
                        )
                    })
            }
        };
        assert!(
            moment <= snapshot.now.0,
            "{label} puts its moment at {moment}, after the pinned now {:?}.\n  \
             If `just fixtures` was just re-run, the pin moved out from under this \
             and the captures are simply newer than it — repin `fn now()` (see the \
             note there for what moves with it).\n  \
             Otherwise it is what it looks like: a clock behind the cluster's, whose \
             negative ages D18 renders as \"just now\".",
            snapshot.now
        );
    }
}

/// **The two snapshots that decode identically and mean opposite things** (D43). Without
/// this field "a small cluster" and "one namespace of a big one" are the same value, and
/// N2 counts zero on a cordoned node whose 40 pods are all outside the scope — a missing
/// finding with nothing on the screen to show it happened. One value, two producers:
/// `--namespace` and the 403 fallback, which are the same fact to a rule.
#[test]
fn the_snapshot_says_whether_its_pod_list_covers_the_whole_cluster() {
    let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
    let one_namespace: Vec<PodSnapshot> = ["crashloop", "healthy"].iter().map(|n| pod(n)).collect();

    // Same pods, same nodes; every other field of the two snapshots is equal.
    let whole_cluster = ClusterSnapshot {
        now: now(),
        pods: one_namespace.clone(),
        nodes: nodes.clone(),
        workloads: Vec::new(),
        server_version: None,
        context: None,
        client_certificate: None,
        namespace_scope: None,
        ..nothing_fetched()
    };
    let scoped = ClusterSnapshot {
        namespace_scope: Some("default".to_string()),
        ..whole_cluster.clone()
    };
    println!(
        "{:?} vs {:?}",
        whole_cluster.namespace_scope, scoped.namespace_scope
    );

    assert_ne!(
        whole_cluster, scoped,
        "a two-pod cluster and a two-pod view of a large one must not be one value"
    );
    assert_eq!(
        scoped.namespace_scope.as_deref(),
        Some("default"),
        "N2 and N5 name the namespace they were limited to when they switch off"
    );
    assert_eq!(
        whole_cluster.namespace_scope, None,
        "`None` is the whole cluster — never an empty string, which is a namespace \
         nothing is in"
    );
}

/// **C1's input arrives on the snapshot like every other rule's.** `analyze(&Snapshot)
/// -> Vec<Finding>` is the whole signature [invariant 5](CLAUDE.md) describes, so
/// "PEM bytes in, finding out" would have been a second entry point — an amendment to
/// a hard invariant, which is a stop rather than a convenience (D51). C1 is the one
/// finding with no API object behind it, so its identity is built here to show the
/// context name is enough to make one.
///
/// The certificate assertion is the security half and it is not decoration: this
/// field carries the certificate and **nothing else off the kubeconfig**, and a
/// private key arriving in it is a credential copied into our own types, one `Debug`
/// away from a backtrace.
#[test]
fn the_snapshot_carries_c1s_certificate_and_the_context_name_it_files_under() {
    let snapshot = ClusterSnapshot {
        now: now(),
        pods: Vec::new(),
        nodes: Vec::new(),
        workloads: Vec::new(),
        server_version: None,
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(certificate("expiring-client")),
        namespace_scope: None,
        ..nothing_fetched()
    };

    let pem = snapshot
        .client_certificate
        .as_deref()
        .expect("the kubeconfig authenticates with a client certificate");
    let text = String::from_utf8(pem.to_vec()).expect("a PEM file is text");
    println!(
        "context {:?}, {} bytes of PEM starting {:?}",
        snapshot.context,
        pem.len(),
        text.lines().next()
    );
    assert!(
        text.starts_with("-----BEGIN CERTIFICATE-----"),
        "C1 parses PEM, so the bytes arrive as they sit on disk"
    );
    assert!(
        !text.contains("PRIVATE KEY"),
        "the certificate and nothing else: a key in this field is a credential in \
         our own struct, and `scripts/make-certs.sh` deletes them for that reason"
    );

    // The identity C1 files under — the one `None` uid in the product, and a kind
    // that names what the thing is because there is no API object to name.
    let id = ObjectId {
        kind: ObjectKind::Other("kubeconfig".to_string()),
        namespace: None,
        name: snapshot
            .context
            .clone()
            .expect("the kubeconfig names a current context"),
        uid: None,
    };
    println!("C1 files under {:?}", id.group_key());
    assert_eq!(
        id.group_key(),
        (
            &ObjectKind::Other("kubeconfig".to_string()),
            None,
            "kind-k8rs"
        ),
        "the card is named after the context the user recognises, not after a file path"
    );

    // The other authentication paths — a token, an exec plugin, OIDC — leave nothing
    // to parse, and C1 says nothing rather than guessing.
    let token_auth = ClusterSnapshot {
        client_certificate: None,
        ..snapshot.clone()
    };
    assert_ne!(
        token_auth, snapshot,
        "a cluster reached with a token is not the same snapshot as one reached with \
         a certificate — C1 fires on exactly one of them"
    );
}

/// A lookup table, so it is tested as one. Eight of the nine branches are unreachable
/// from any committed capture — every fixture object is a Pod, a Node, a Deployment,
/// a ReplicaSet or a DaemonSet — and an unreachable branch is one nobody would notice
/// going wrong.
#[test]
fn every_kind_this_file_has_a_branch_for_maps_to_it() {
    for (api_version, kind, expected) in [
        ("apps/v1", "Deployment", ObjectKind::Deployment),
        ("apps/v1", "StatefulSet", ObjectKind::StatefulSet),
        ("apps/v1", "DaemonSet", ObjectKind::DaemonSet),
        ("apps/v1", "ReplicaSet", ObjectKind::ReplicaSet),
        ("batch/v1", "Job", ObjectKind::Job),
        ("batch/v1", "CronJob", ObjectKind::CronJob),
        ("v1", "Node", ObjectKind::Node),
        ("v1", "Pod", ObjectKind::Pod),
    ] {
        let got = ObjectKind::from_api(api_version, kind);
        println!("{api_version} {kind} -> {got:?}");
        assert_eq!(got, expected);
    }

    // The version is not the identity: a type is named by its group and its kind, and
    // `apps/v1beta1` is the same StatefulSet as `apps/v1`.
    assert_eq!(
        ObjectKind::from_api("apps/v1beta1", "StatefulSet"),
        ObjectKind::StatefulSet,
        "an older serialisation of a built-in kind is still that kind"
    );

    // The owner chain genuinely stops at kinds this project has no branch for, and
    // the kind has to survive as text rather than collapse onto a wrong variant.
    assert_eq!(
        ObjectKind::from_api("argoproj.io/v1alpha1", "Rollout"),
        ObjectKind::Other("Rollout.argoproj.io".to_string()),
        "an Argo Rollout is a real owner, and inventing a variant for it is per-kind code"
    );
    assert_ne!(
        ObjectKind::from_api("apps/v1", "deployment"),
        ObjectKind::Deployment,
        "the API sends TitleCase kinds; matching loosely would map an unknown kind onto a real one"
    );

    // The core group is written `v1` and has no group *name*, so a core kind with no
    // branch here has nothing to qualify it with and must stay bare. A
    // ReplicationController is a real, still-supported `controller: true` pod owner,
    // and `ReplicationController.` on a card is a dangling dot in front of a user
    // (invariant 14). Phase 4's drain report files findings about
    // PersistentVolumeClaims and PodDisruptionBudgets, which take this same arm.
    assert_eq!(
        ObjectKind::from_api("v1", "ReplicationController"),
        ObjectKind::Other("ReplicationController".to_string()),
        "a core-group kind keeps its bare name — there is no group to append"
    );
}

/// **The kind string alone does not name a kind, and the write path is what pays for
/// that.** OpenKruise is deliberately drop-in: its Advanced StatefulSet is
/// `apps.kruise.io/v1beta1, Kind: StatefulSet`, its Advanced DaemonSet
/// `apps.kruise.io/v1alpha1, Kind: DaemonSet`, and Volcano's Job is
/// `batch.volcano.sh/v1alpha1` — each one decodes as the built-in variant if only the
/// kind is read. The card lying is the small half; the large half is Phase 7 aiming
/// `scale` at `apps/v1 statefulsets/<name>`, which is a 404 or, worse, a different
/// object that happens to share the name.
///
/// The last case is the same question from the other side: `Node` in somebody's CRD
/// group is an ordinary owner, not the kubelet's mirror-pod reference, so the group
/// has to gate the *discard* as well as the variant.
/// **Capture trip:** none — installing OpenKruise on the fixture cluster to
/// photograph one owner reference is a cluster change, not a fixture.
#[test]
fn the_api_group_decides_which_kind_an_owner_reference_names() {
    let owned_by = |api_version: &str, kind: &str| {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object.metadata.owner_references = Some(vec![OwnerReference {
            api_version: api_version.to_string(),
            kind: kind.to_string(),
            name: "web".to_string(),
            uid: "5c8a1f37-2b6d-4a90-8e11-77c2d4b0a913".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);
        PodSnapshot::from(object)
    };

    let kruise = owned_by("apps.kruise.io/v1beta1", "StatefulSet");
    println!("kruise owner: {:?} mirror: {}", kruise.owner, kruise.mirror);
    assert_ne!(
        kruise.owner.kind,
        ObjectKind::StatefulSet,
        "an Advanced StatefulSet is not an apps/v1 one — Phase 7 would scale the \
         wrong object, or nothing at all"
    );
    assert_eq!(
        kruise.owner.kind,
        ObjectKind::Other("StatefulSet.apps.kruise.io".to_string()),
        "and the group survives, so a card can say which StatefulSet this is"
    );
    assert_eq!(kruise.owner.name, "web", "it is still a real owner");

    let volcano = owned_by("batch.volcano.sh/v1alpha1", "Job");
    println!("volcano owner: {:?}", volcano.owner);
    assert_eq!(
        volcano.owner.kind,
        ObjectKind::Other("Job.batch.volcano.sh".to_string()),
        "a Volcano Job is not a batch/v1 Job either"
    );

    let crd_node = owned_by("example.com/v1", "Node");
    println!(
        "crd node owner: {:?} mirror: {}",
        crd_node.owner, crd_node.mirror
    );
    assert!(
        !crd_node.mirror,
        "only the core `v1` Node is the kubelet's own reference; a CRD that happens \
         to be called Node does not make this a static pod, and N2 must still count \
         a pod a drain would move"
    );
    assert_eq!(
        crd_node.owner.kind,
        ObjectKind::Other("Node.example.com".to_string()),
        "so it is kept as an ordinary owner instead of being discarded"
    );
}

/// **`spec.containers[].restartPolicyRules`, off the two captures that carry one** — the field
/// [`stopped_for_good`] reads before it claims nothing is starting a container again
/// ([`ContainerSnapshot::restart_rules`], NOTES § D99, § D135).
///
/// **Two captures and not one, because the two actions are different facts.** `neverrules`
/// declares [`RESTART_SELF`], which stops at the container that declared it; `gang` declares
/// [`RESTART_ALL_ACTION`], which is the only spelling that reaches a sibling — and the
/// **validator** accepts both while `kubectl explain` and the published schema name only the
/// first (NOTES § D97). A decode tested against one of them proves nothing about the other.
///
/// **Every value is read back out of the capture**, action, operator and codes alike: the rules
/// are written in `scripts/broken.yaml` and belong to the cluster that answered, not to this
/// file. What is asserted *here* is the shape rule 15's tests need the fixtures to keep — one
/// rule, one operator, one code — so a manifest that grew a second rule reddens this line
/// instead of quietly widening what the rule tests are standing on.
///
/// **The negative is in the same two objects**: `keeper` and `bystander` declare no rules at
/// all, which is the state every other container in the corpus is in and the one that leaves
/// [`ContainerSnapshot::restart_policy`] as the whole answer. Asserted over the **whole**
/// corpus, both ways, so a decode that had stopped filling the field would print the same green
/// line as one with nothing wrong (CLAUDE.md § A derived list asserts it found something).
#[test]
fn the_two_captures_that_declare_restart_rules_decode_them_and_the_rest_of_the_corpus_has_none() {
    let mut carried: Vec<String> = Vec::new();
    for name in CAPTURED_PODS {
        let p = pod(name);
        for c in &p.containers {
            if !c.restart_rules.is_empty() {
                carried.push(format!("{name}/{}", c.name));
            }
        }
    }
    carried.sort();
    println!("containers declaring restartPolicyRules: {carried:?}");
    assert_eq!(
        carried,
        ["gang/trigger", "neverrules/retry"],
        "two captures declare rules and every other container in the corpus declares none — a \
         third arriving is a shape rule 15's tests stopped being the only proof of, and a \
         decode that had gone empty would otherwise pass with an empty sweep"
    );

    for (capture, container_name, action) in [
        ("neverrules", "retry", RESTART_SELF),
        ("gang", "trigger", RESTART_ALL_ACTION),
    ] {
        let raw = fixture(capture);
        let declared = &raw["spec"]["containers"]
            .as_array()
            .expect("the capture declares its containers")
            .iter()
            .find(|c| c["name"] == container_name)
            .unwrap_or_else(|| panic!("{capture} declares {container_name}"))["restartPolicyRules"];
        let rules = declared
            .as_array()
            .unwrap_or_else(|| panic!("{capture}/{container_name} carries restartPolicyRules"));
        assert_eq!(
            rules.len(),
            1,
            "{capture}/{container_name}: one rule is the shape rule 15's tests are written \
             around — {declared}"
        );
        assert_eq!(
            captured_str(&rules[0], &["action"]),
            action,
            "{capture}/{container_name}: and it is this action, which is what makes the two \
             captures two different facts rather than one repeated"
        );

        let decoded = pod(capture);
        let c = container(&decoded, container_name);
        // `values` is an array, and [`at`] walks object keys — so the codes come out of it by
        // hand, with the count asserted rather than assumed: a rule naming two codes and a decode
        // keeping one would otherwise compare equal on the first.
        let codes: Vec<i32> = rules[0]["exitCodes"]["values"]
            .as_array()
            .unwrap_or_else(|| panic!("{capture}/{container_name}: the rule names exit codes"))
            .iter()
            .map(|v| captured_i32(v, &[]))
            .collect();
        assert_eq!(
            codes.len(),
            1,
            "{capture}/{container_name}: one code is the shape rule 15's tests are written \
             around — {codes:?}"
        );
        println!("{capture}/{container_name}: {:?}", c.restart_rules);
        assert_eq!(
            c.restart_rules,
            vec![ExitRule {
                action: captured_str(&rules[0], &["action"]).to_string(),
                operator: Some(captured_str(&rules[0], &["exitCodes", "operator"]).to_string()),
                values: codes,
            }],
            "the whole rule, off its own three paths: the action, the operator inside \
             `exitCodes`, and the codes inside that — three nested keys a decode can drop one \
             of and stay green on the other two"
        );
        assert_eq!(
            c.restart_rules[0].operator.as_deref(),
            Some("In"),
            "{capture}/{container_name}: an `In` rule is what the manifests declare, and the \
             `NotIn` half is built rather than captured (NOTES § D40)"
        );

        // **The sibling in the same object declares nothing**, which is the negative that makes
        // the positive above a discrimination rather than a decode that fills every container.
        let sibling = match capture {
            "neverrules" => "keeper",
            _ => "bystander",
        };
        let other = container(&decoded, sibling);
        assert!(
            other.restart_rules.is_empty(),
            "{capture}/{sibling}: `restartPolicyRules` is null in the capture, and an empty \
             list is what that has to decode to — a rule borrowed from the container next to it \
             would take this one's card away: {:?}",
            other.restart_rules
        );
    }
}

// --- ONE FIELD CHANGED ON A REAL CAPTURE ---
//
// Everything below starts from a committed capture and changes exactly one field, or
// one group of fields that a single `kubectl` action changes together, to a value the
// API demonstrably produces and the kind cluster did not happen to be in when it was
// photographed. Each test says which value, why the API produces it, and — since the
// 2026-08-13 trip — either the object that retired it or the reason no trip can bring one.
//
// These are **decode** tests. A rule's positive fixture stays a real capture — this
// technique never becomes one.

/// D3's whole premise — one card per owner — and not one of the twelve pod captures
/// carries an `ownerReference`, because `scripts/broken.yaml` creates bare pods.
/// A pod created by a Deployment carries exactly this: its ReplicaSet, `controller:
/// true`. **Capture trip:** the owned broken pod already listed in the open Phase 2
/// box retires this.
#[test]
fn a_pod_with_a_controller_files_under_it_and_not_under_itself() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    object.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "web-7d4f5c6b8".to_string(),
        uid: "3f1c9a20-0d2e-4c19-9a71-5a4b1f6e8d10".to_string(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    }]);

    let p = PodSnapshot::from(object);
    println!("owner: {:?}\nobject: {:?}", p.owner, p.id);

    assert_ne!(p.owner, p.id, "a controlled pod does not file under itself");
    assert_eq!(p.owner.kind, ObjectKind::ReplicaSet);
    assert_eq!(p.owner.name, "web-7d4f5c6b8");
    assert_eq!(
        p.owner.uid.as_deref(),
        Some("3f1c9a20-0d2e-4c19-9a71-5a4b1f6e8d10"),
        "the owner's own uid, never the pod's — the group agrees on the owner's (D39)"
    );
    assert_eq!(
        p.owner.namespace.as_deref(),
        Some("default"),
        "an ownerReference carries no namespace; the owner is in the pod's own"
    );
    assert_eq!(p.id.name, "broken-crashloop", "the object is still the pod");
    assert!(
        !p.mirror,
        "a pod with a real controller is not a static pod, and a drain will move it"
    );
}

/// D39: kubelet writes an `ownerReference` of kind `Node` onto every static pod, so
/// on this project's own `just cluster-up` cluster `etcd-*`, `kube-apiserver-*`,
/// `kube-scheduler-*` and `kube-controller-manager-*` all carry one — `controller:
/// true` included. Kept, the card loses `kube-system` and draws as a machine.
///
/// **The identity is discarded and the bit is kept.** `kubectl drain` skips a mirror
/// pod unconditionally, so a control-plane node cordoned for an upgrade still runs
/// its four static pods when the drain has finished perfectly — and N2, which fires
/// on "cordoned and still has pods on it", would file a finding on every one of them.
/// **Capture trip:** the `-n kube-system` capture already listed in the open Phase 2
/// box retires this.
#[test]
fn a_mirror_pods_node_owner_is_discarded_and_it_files_under_itself() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    object.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "v1".to_string(),
        kind: "Node".to_string(),
        name: "k8rs-control-plane".to_string(),
        uid: "dc9525d6-e89b-4409-81fa-f3e4a57409aa".to_string(),
        controller: Some(true),
        block_owner_deletion: None,
    }]);

    let p = PodSnapshot::from(object);
    println!("owner: {:?} mirror: {}", p.owner, p.mirror);

    assert_eq!(
        p.owner, p.id,
        "a mirror pod files under itself — there is no controller to scale or roll"
    );
    assert_eq!(p.owner.kind, ObjectKind::Pod);
    assert_eq!(
        p.owner.namespace.as_deref(),
        Some("default"),
        "keeping the Node would have taken the namespace off the card with it"
    );
    assert_ne!(
        p.owner.name, "k8rs-control-plane",
        "four control-plane pods must not collapse onto one machine-shaped card"
    );
    assert!(
        p.mirror,
        "the Node reference is the only trace a static pod leaves — the annotation \
         that also says so is stripped by the sanitizer — and dropping it with the \
         identity makes N2 fire on every correctly drained node"
    );
}

/// A Node reference that does **not** control the pod is somebody's garbage-collection
/// link, not the kubelet's. The mirror bit rides on the same match arm as the
/// discarded owner, so it inherits that arm's filter; asserted because "kind is Node"
/// and "the *controller* is a Node" are one character apart in the implementation and
/// a whole rule apart in N2.
/// **Capture trip:** none — see D40 on why `broken.yaml` is not contorted to produce
/// a non-controlling reference.
#[test]
fn a_node_reference_that_does_not_control_the_pod_does_not_make_it_a_mirror_pod() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    object.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "v1".to_string(),
        kind: "Node".to_string(),
        name: "k8rs-control-plane".to_string(),
        uid: "dc9525d6-e89b-4409-81fa-f3e4a57409aa".to_string(),
        controller: Some(false),
        block_owner_deletion: None,
    }]);

    let p = PodSnapshot::from(object);
    println!("owner: {:?} mirror: {}", p.owner, p.mirror);

    assert_eq!(
        p.owner, p.id,
        "it still files under itself — nothing controls it"
    );
    assert!(
        !p.mirror,
        "only the kubelet's own controlling reference means a static pod; a drain \
         moves this one, so N2 must count it"
    );
}

/// Both `ownerReferences` committed in this repo are controllers, so the filter that
/// picks one is unproven. An object can carry several references and at most one of
/// them controls it — `kubectl delete --cascade=orphan` and any operator that adds a
/// reference for garbage collection produce the non-controlling kind.
/// **Capture trip:** none needed; a second reference is not something `broken.yaml`
/// should be contorted to produce.
#[test]
fn a_reference_that_does_not_control_the_pod_is_not_its_owner() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    object.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: "not-the-controller".to_string(),
        uid: "8c2d17bb-4e6a-4f0b-9d55-2f1a7c3e9b44".to_string(),
        controller: Some(false),
        block_owner_deletion: Some(true),
    }]);

    let p = PodSnapshot::from(object);
    println!("owner: {:?}", p.owner);

    assert_eq!(
        p.owner, p.id,
        "only the reference marked `controller: true` decides the card"
    );
    assert_ne!(p.owner.name, "not-the-controller");
}

/// The spec lookup reads `initContainers` *and* `containers`, and no captured init
/// container declares resources — so dropping the first list from the chain changes
/// nothing today. A migration container with a memory limit is ordinary.
/// **Capture trip:** put `resources.limits.memory` on the `migrate` init container in
/// `scripts/healthy.yaml`, and this synthesis retires.
#[test]
fn an_init_containers_limits_are_found_in_the_init_list() {
    let mut object: Pod =
        serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
    object
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .init_containers
        .as_mut()
        .expect("this pod declares an init container")[0]
        .resources = Some(ResourceRequirements {
        limits: Some(BTreeMap::from([(
            "memory".to_string(),
            Quantity("128Mi".to_string()),
        )])),
        requests: Some(BTreeMap::from([(
            "cpu".to_string(),
            Quantity("50m".to_string()),
        )])),
        claims: None,
    });

    let p = PodSnapshot::from(object);
    let migrate = container(&p, "migrate");
    println!(
        "migrate: limit={:?} request={:?}",
        migrate.memory_limit, migrate.cpu_request
    );

    assert_eq!(migrate.role, ContainerRole::Init);
    assert_eq!(
        migrate.memory_limit.as_deref(),
        Some("128Mi"),
        "rule 2 must be able to name the limit an init container exceeded"
    );
    assert_eq!(
        migrate.cpu_request.as_deref(),
        Some("50m"),
        "N5 sums these too"
    );
    assert_eq!(
        container(&p, "app").memory_limit.as_deref(),
        Some("64Mi"),
        "and the regular container still finds its own"
    );
}

/// **The limit a finding may name is the one the container was actually given.**
/// `ContainerStatus.resources` is "the compute resource requests and limits that have
/// been successfully enacted on the running container", and in-place resize (beta and
/// default-on since 1.33) is what makes it disagree with the spec: patch a crashing
/// pod's limit 64Mi → 512Mi and the resize can sit `Deferred` because the node cannot
/// fit it. A spec-first read then prints "exceeded its 512Mi limit · exit 137" about a
/// container never given 512Mi, and sends an operator hunting a leak in an application
/// that never had the memory.
///
/// Both directions are pinned on one object, because a decode hardwired to either side
/// passes half of this: the *enacted* value wins when the two differ, and the spec is
/// still the answer when the kubelet has reported no resources at all — which is a
/// shape the captures already contain (`init.json`'s `app` carries `resources: null`).
/// **Capture trip:** patch the memory limit of a pod onto a node that cannot fit the
/// new value, and capture it with the resize still pending.
#[test]
fn the_memory_limit_is_the_enacted_one_and_not_the_one_the_spec_asked_for() {
    let captured = || -> Pod { serde_json::from_value(fixture("oom")).expect("oom.json is a Pod") };

    let mut deferred = captured();
    deferred
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .containers[0]
        .resources
        .as_mut()
        .expect("the captured container declares resources")
        .limits
        .as_mut()
        .expect("including a memory limit")
        .insert("memory".to_string(), Quantity("512Mi".to_string()));

    let p = PodSnapshot::from(deferred);
    let c = container(&p, "hog");
    println!(
        "resize deferred: limit={:?} request={:?}",
        c.memory_limit, c.memory_request
    );
    assert_eq!(
        c.memory_limit.as_deref(),
        Some("64Mi"),
        "the status says 64Mi was enacted, so rule 2 may not name the 512Mi the \
         patch asked for and the node refused"
    );
    assert_eq!(
        c.memory_request.as_deref(),
        Some("64Mi"),
        "and N5 charges what the node actually reserved, for the same reason"
    );

    // The other half: a container the kubelet has reported no resources on falls back
    // to the spec, or the limit disappears from every cluster that does not send the
    // field at all.
    let mut no_status_resources = captured();
    no_status_resources
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .containers[0]
        .resources
        .as_mut()
        .expect("the captured container declares resources")
        .limits
        .as_mut()
        .expect("including a memory limit")
        .insert("memory".to_string(), Quantity("512Mi".to_string()));
    no_status_resources
        .status
        .as_mut()
        .expect("the captured pod has a status")
        .container_statuses
        .as_mut()
        .expect("it has a container status")[0]
        .resources = None;

    let p = PodSnapshot::from(no_status_resources);
    let c = container(&p, "hog");
    println!("no enacted resources: limit={:?}", c.memory_limit);
    assert_eq!(
        c.memory_limit.as_deref(),
        Some("512Mi"),
        "with nothing enacted to read, what the spec asked for is the best answer \
         there is — not no answer"
    );
}

/// **The fallback is per key, not per side.** [`effective`] promises that a `requests`
/// map the kubelet did populate does not suppress the spec for the keys it left out,
/// and until now nothing said so: a per-*object* read fails only because
/// `healthy.json`'s `migrate` happens to carry `"resources": {}` in its status, which
/// is an accident of one capture rather than a property anything asserts.
///
/// The shape that decides it is one key of two. `oom.json`'s `hog` has its memory
/// request enacted and no cpu request at all, so adding one to the spec leaves the
/// status the answer for `memory` and the spec the only source there is for `cpu` —
/// the doc's own named cost case, a resize *adding* a value where none existed, read
/// from whichever side has each key. Read per side instead and N5 charges this pod
/// nothing for a quarter of a CPU it is committed to.
///
/// The mirror shape — a key in `status.resources` the spec never declared — is **not
/// asserted, so a decode reading the status only for keys the spec names is
/// indistinguishable from this one.** It is nonetheless **reachable**: under pod-level
/// resources (KEP-2837) `getMemoryLimit` puts the *pod's* memory limit on a container
/// cgroup whose own is unset, and `convertContainerStatusResources` copies it back
/// without testing whether the spec declared that key.
///
/// What *is* structural is the other direction: the status map begins as
/// `allocatedContainer.Resources.DeepCopy()` and `validateContainerResize` forbids
/// *removing* a key while permitting an addition, so the **spec's** key set is the
/// superset-or-equal. That is what makes the shape this test asserts legitimate rather
/// than invented.
///
/// **Capture trip:** the pod the test above waits for, and — for the mirror shape — a
/// pod declaring `spec.resources.limits.memory` whose container declares a cpu limit
/// and no memory limit.
#[test]
fn a_key_the_kubelet_did_not_enact_still_reads_from_the_spec() {
    let mut resizing: Pod = serde_json::from_value(fixture("oom")).expect("oom.json is a Pod");
    let requested = resizing
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .containers[0]
        .resources
        .as_mut()
        .expect("the captured container declares resources")
        .requests
        .as_mut()
        .expect("including a memory request");
    requested.insert("memory".to_string(), Quantity("512Mi".to_string()));
    requested.insert("cpu".to_string(), Quantity("250m".to_string()));

    // Untouched: the kubelet enacted `memory` alone, exactly as captured. Asserted,
    // because the whole test is about the key that is missing from this map.
    let enacted = resizing
        .status
        .as_ref()
        .expect("the captured pod has a status")
        .container_statuses
        .as_ref()
        .expect("it has a container status")[0]
        .resources
        .as_ref()
        .expect("the kubelet reported enacted resources");
    assert!(
        !enacted
            .requests
            .as_ref()
            .is_some_and(|r| r.contains_key("cpu")),
        "the shape this test rests on: no cpu request was enacted here, got {:?}",
        enacted.requests
    );

    let p = PodSnapshot::from(resizing);
    let c = container(&p, "hog");
    println!(
        "pending resize: cpu={:?} memory={:?}",
        c.cpu_request, c.memory_request
    );
    assert_eq!(
        c.memory_request.as_deref(),
        Some("64Mi"),
        "the key the kubelet did enact is still read from the status, not the spec"
    );
    assert_eq!(
        c.cpu_request.as_deref(),
        Some("250m"),
        "and the key it did not enact falls through to the spec — a populated status \
         is not an answer for every key that is absent from it"
    );
}

/// **KEP-2837: the pod itself can hold the request, and then the containers do not.**
/// `spec.resources` is beta and default-on since 1.34 and the captures are v1.36.1,
/// but nothing in `scripts/*.yaml` declares one, so every fixture decodes `None` here
/// and a decode that dropped the field entirely would read as correct. A pod asking
/// for `cpu: "4"` with no per-container request then decodes as all-`None`, N5 sums
/// zero and calls the node healthy while four committed CPUs sit invisible.
///
/// **When it is set it replaces the container sum for N5 — it does not add to it** —
/// so both numbers have to survive the decode, which is why the container assertion
/// is here too.
/// **Capture trip:** add `spec.resources.requests` to a pod in `scripts/healthy.yaml`
/// — it is a healthy shape, not a broken one.
#[test]
fn a_pod_that_declares_its_own_request_carries_it_beside_the_containers() {
    let captured: Pod = serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");

    let untouched = PodSnapshot::from(captured.clone());
    println!(
        "captured: pod cpu={:?} memory={:?}",
        untouched.cpu_request, untouched.memory_request
    );
    assert_eq!(
        (untouched.cpu_request, untouched.memory_request),
        (None, None),
        "this pod declares no pod-level request, and inventing one from the \
         containers would be a number nobody committed"
    );

    let mut object = captured;
    object
        .spec
        .as_mut()
        .expect("the captured pod has a spec")
        .resources = Some(ResourceRequirements {
        limits: None,
        requests: Some(BTreeMap::from([
            ("cpu".to_string(), Quantity("4".to_string())),
            ("memory".to_string(), Quantity("2Gi".to_string())),
        ])),
        claims: None,
    });

    let p = PodSnapshot::from(object);
    println!(
        "pod-level: cpu={:?} memory={:?}",
        p.cpu_request, p.memory_request
    );
    assert_eq!(
        p.cpu_request.as_deref(),
        Some("4"),
        "four CPUs the scheduler committed, which N5 must not sum to zero"
    );
    assert_eq!(p.memory_request.as_deref(), Some("2Gi"));
    assert_eq!(
        container(&p, "app").cpu_request.as_deref(),
        Some("10m"),
        "and the container keeps its own — N5 replaces the sum with the pod-level \
         value rather than adding to it, and it needs both numbers to do that"
    );
}

/// `terminationMessagePolicy: FallbackToLogsOnError` makes the kubelet copy the tail
/// of the container's log into `terminated.message`, which is what turns rule 6's
/// action from "check the logs" into the log line. No captured container sets the
/// policy, so the field is absent everywhere and a decode that dropped it reads as
/// correct. `Waiting` already carried a message; `Terminated` not carrying one was an
/// asymmetry with nothing behind it.
///
/// The message is copied through **exactly as the API sent it**, newline included —
/// bounding and control-character stripping belong to `k8s.rs` at ingest, as the
/// section header above says, and doing half of it here would leave nobody sure which
/// half.
/// **Capture trip:** set `terminationMessagePolicy: FallbackToLogsOnError` on
/// `scripts/broken.yaml`'s crashlooping container.
#[test]
fn a_terminated_container_keeps_the_message_the_kubelet_left_behind() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    // A hostname, not an address: the captures carry `REDACTED-IP` where the sanitizer
    // took one out, and writing an address-shaped literal back into the same file is
    // confusing at best.
    let tail = "panic: dial tcp db.payments.svc:5432: connect: connection refused\n";
    object
        .status
        .as_mut()
        .expect("the captured pod has a status")
        .container_statuses
        .as_mut()
        .expect("it has a container status")[0]
        .last_state
        .as_mut()
        .expect("the container has died at least once")
        .terminated
        .as_mut()
        .expect("and it died rather than being killed while waiting")
        .message = Some(tail.to_string());

    let p = PodSnapshot::from(object);
    let c = container(&p, "quitter");
    println!("last message: {:?}", c.last_terminated);
    assert_eq!(
        c.last_terminated
            .as_ref()
            .and_then(|t| t.message.as_deref()),
        Some(tail),
        "verbatim (D37): rule 6 shows the application's own last line, and a decode \
         that trimmed or truncated it would pass any looser assertion"
    );
    assert_eq!(
        c.last_terminated.as_ref().map(|t| t.exit_code),
        Some(1),
        "and the rest of the termination is untouched"
    );
}

/// **The sidecar, and it is the case with a specific wrong answer available.** An
/// init container with `restartPolicy: Always` is the native sidecar — GA since 1.29,
/// and how Istio, Linkerd and the Vault agent run. It is charged like a regular
/// container (the scheduler's effective request is
/// `max(max over the init prefix, sum(regular) + sum(restartable init))`) and it is
/// described like one: "the init container `istio-proxy` is crashlooping" is not
/// plain language, it is wrong. Under a boolean it decodes as `Init`, which is the
/// answer that overstates a migration container or drops the proxy's request on
/// every pod of a meshed node.
///
/// The neighbours are set on purpose (D40's third category): the same `migrate`
/// container without the field is asserted `Init` two tests up, and the regular `app`
/// container beside it carries **the same `restartPolicy: Always`** and still decodes
/// `Regular` — so this cannot pass by answering `Sidecar` for everything, for every
/// init container, for the whole pod, or for every container that declares the field.
/// The last of those is the one the list is read for: only the *init* list makes the
/// policy mean sidecar, and a regular container is charged additively and described as
/// itself whatever its restart policy says. 1.34 began relaxing where the field may
/// appear and the fixtures are pinned at v1.36.1 (`tests/fixtures/K8S_VERSION`), so a
/// server of that generation accepts it there; whether a given cluster's feature gates
/// do is not what this asserts, because the requirement does not depend on it — the
/// answer is `Regular` whatever the restart policy says and whoever admitted it.
/// **Capture trip:** add a `restartPolicy: Always` init container to
/// `scripts/healthy.yaml` — it is a healthy shape, not a broken one.
#[test]
fn an_init_container_that_never_finishes_is_a_sidecar_and_not_an_init_container() {
    let mut object: Pod =
        serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
    let spec = object.spec.as_mut().expect("the captured pod has a spec");
    spec.init_containers
        .as_mut()
        .expect("this pod declares an init container")[0]
        .restart_policy = Some("Always".to_string());
    spec.containers[0].restart_policy = Some("Always".to_string());

    let p = PodSnapshot::from(object);
    println!(
        "{:?}",
        p.containers
            .iter()
            .map(|c| (&c.name, c.role))
            .collect::<Vec<_>>()
    );

    assert_eq!(
        container(&p, "migrate").role,
        ContainerRole::Sidecar,
        "it starts in the init sequence and then keeps running beside the workload, \
         so N5 adds its request instead of taking a maximum over it"
    );
    assert_eq!(
        container(&p, "app").role,
        ContainerRole::Regular,
        "and the container it runs beside is still a regular one — it declares the \
         very same `restartPolicy: Always`, and on a regular container that changes \
         neither how it is charged nor what it is called"
    );
}

/// **A container can have a status and no declaration, and this is what the decode does with
/// one** (`container_snapshots`). The pairing is by name and the miss was explained away as
/// impossible — *both container lists are immutable after create* — which is not the thing that
/// would prevent it: **a node implementation that is not a kubelet is.** k9s carries the field
/// report ([#4145](https://github.com/derailed/k9s/issues/4145), open): on Tencent TKE **virtual
/// nodes** the provider injects a managed logging container into `status.containerStatuses` with
/// no entry in `spec.containers` — two declared containers, three ready statuses, pod
/// `Ready: True`. Virtual-kubelet, serverless nodes and sandboxed runtimes all sit in that gap,
/// and nothing in the API server rejects the object: `spec` and `status` are separate
/// subresources and no admission plugin cross-checks the two lists.
///
/// **This asserts what the code does today, and changes nothing.** Every claim below is a
/// consequence of `declared` being `None` rather than a decision anyone made, which is exactly
/// why it is written down: the next reader inherits it as behaviour instead of re-deriving it,
/// and a decode that starts inventing a limit for an undeclared container is caught saying so.
///
/// **The requirement is that no card claims what the spec never said.** A container with no
/// declaration has no requests, no limits and no `restartPolicy` of its own, so nothing k8rs draws
/// about it may name one — that is what the second half of this test drives, on a shape that
/// actually fires rather than on a healthy pod where every rule is silent anyway.
///
/// **The regular list is the shape that matters, and the init one is a companion with no known
/// producer.** k9s #4145 is a provider injecting into `status.containerStatuses`; nothing
/// documented injects into `status.initContainerStatuses`. The init row is here because the role
/// is read off the list a status arrived in and from nothing else (NOTES § D29), so a decode that
/// answered `Regular` for everything would pass the regular row alone — it is not a claim that
/// the case occurs.
///
/// **Planted on a decoded copy of a committed capture, never on the JSON** (NOTES § D53): no
/// cluster this repository can build runs a virtual node.
#[test]
fn a_container_status_with_no_declaration_decodes_with_nothing_the_spec_would_have_given_it() {
    // What a provider injects: a name, an image, and a ready running container. No `resources`,
    // because the pod's manifest never asked for any and the provider does not fill the field —
    // which is the whole of what this container is missing.
    let injected = |name: &str| ContainerStatus {
        name: name.to_string(),
        image: "provider.invalid/logging:v1".to_string(),
        image_id: String::new(),
        ready: true,
        started: Some(true),
        restart_count: 0,
        state: Some(ApiContainerState {
            running: Some(ContainerStateRunning {
                started_at: Some(time("2026-08-13T23:33:17Z")),
            }),
            ..ApiContainerState::default()
        }),
        ..ContainerStatus::default()
    };
    let mut object: Pod =
        serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
    let status = object
        .status
        .as_mut()
        .expect("the captured pod has a status");
    status
        .container_statuses
        .as_mut()
        .expect("the capture reports its regular container")
        .push(injected("provider-logs"));
    status
        .init_container_statuses
        .as_mut()
        .expect("the capture reports its init container")
        .push(injected("provider-setup"));

    let p = PodSnapshot::from(object);
    for c in &p.containers {
        println!("{c:?}");
    }
    assert_eq!(
        p.containers.len(),
        4,
        "the status lists decide how many containers a pod has, and a status with no \
         declaration is still a container: {:?}",
        p.containers.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    // **The declared pair still decodes off its own declaration**, or every assertion below
    // passes because the lookup broke for everybody (NOTES § D26).
    assert_eq!(
        (
            container(&p, "app").memory_limit.as_deref(),
            container(&p, "migrate").role
        ),
        (Some("64Mi"), ContainerRole::Init),
        "the containers that do have a declaration are unaffected by the one that does not"
    );
    for (name, role) in [
        ("provider-logs", ContainerRole::Regular),
        ("provider-setup", ContainerRole::Init),
    ] {
        let c = container(&p, name);
        assert_eq!(
            c.role, role,
            "{name}: with no declaration to read `restartPolicy` off, the list the status arrived \
             in is the only thing left deciding the role — a decode that answered `Regular` for \
             everything, or `Init` for everything, passes one of these two rows and not both"
        );
        assert_eq!(
            (
                c.cpu_request.as_deref(),
                c.memory_request.as_deref(),
                c.memory_limit.as_deref()
            ),
            (None, None, None),
            "{name}: nothing was declared and nothing was enacted, so rule 2 has no limit to \
             name and N5 has nothing to add up — a decode that guessed one would be naming a \
             number that exists nowhere"
        );
        assert_eq!(
            c.restart_policy.as_deref(),
            Some("Always"),
            "{name}: the container's own policy is unreadable, so the effective one is the \
             pod's — the fallback `container_snapshots` already makes for a container that \
             declares none"
        );
    }
    // **And no rule invents a card about it.** `healthy.json` is the capture that draws nothing,
    // so anything here is something an undeclared container made up.
    let all = analyze(&pods_at(vec![p], now()));
    show(&all);
    nothing(
        &all,
        "a healthy pod with two containers nobody declared is still a healthy pod",
    );

    // --- THE SHAPE THAT ACTUALLY DRAWS ---
    //
    // **A pod where nothing fires asserts nothing about the cards**, so the same injection is
    // driven again on a container in trouble: backing off, restarted, with a memory kill on the
    // record. Rules 1 and 2 both reach it, and rule 2 is the one that would name a number the
    // spec never carried.
    //
    // **The declared container beside it takes the identical plant**, and it is what makes this a
    // discrimination rather than a rule that has gone quiet: `status.resources` is stripped from
    // both, so the *only* thing left that could name a limit is the declaration — `app` has one
    // and `provider-logs` has none.
    let kill = |c: &mut ContainerStatus| {
        c.state = Some(ApiContainerState {
            waiting: Some(ContainerStateWaiting {
                reason: Some("CrashLoopBackOff".to_string()),
                message: None,
            }),
            ..ApiContainerState::default()
        });
        c.last_state = Some(ApiContainerState {
            terminated: Some(ContainerStateTerminated {
                reason: Some("OOMKilled".to_string()),
                exit_code: 137,
                started_at: Some(time("2026-08-13T23:30:00Z")),
                finished_at: Some(time("2026-08-13T23:33:00Z")),
                ..ContainerStateTerminated::default()
            }),
            ..ApiContainerState::default()
        });
        c.ready = false;
        c.started = Some(false);
        c.restart_count = 7;
        c.resources = None;
    };
    let mut broken: Pod =
        serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
    let statuses = broken
        .status
        .as_mut()
        .expect("the captured pod has a status")
        .container_statuses
        .as_mut()
        .expect("the capture reports its regular container");
    statuses.push(injected("provider-logs"));
    for c in statuses.iter_mut() {
        kill(c);
    }
    let p = PodSnapshot::from(broken);
    assert_eq!(
        (
            container(&p, "app").memory_limit.as_deref(),
            container(&p, "provider-logs").memory_limit.as_deref()
        ),
        (Some("64Mi"), None),
        "the declaration is the only difference left between these two containers"
    );
    let all = analyze(&pods_at(vec![p], now()));
    show(&all);

    for (name, limit) in [("app", true), ("provider-logs", false)] {
        let about: Vec<&Finding> = all
            .iter()
            .filter(|f| f.evidence.contains(&format!("container {name}")))
            .collect();
        assert!(
            about.len() >= 2,
            "{name}: the rules reach this container — a card about the loop and a card about the \
             kill — or every assertion below is about a screen with nothing on it: {:?}",
            titles(&all)
        );
        // **The requirement: a card may not claim what the spec never said.** Rule 2 names the
        // limit the container exceeded, and there is no limit to name when nobody declared one.
        assert_eq!(
            about.iter().any(|f| f.evidence.contains("limit 64Mi")),
            limit,
            "{name}: the memory limit is on the card exactly where a declaration carried one — \
             a number printed for a container whose manifest k8rs never saw is invented: {:?}",
            about.iter().map(|f| &f.evidence).collect::<Vec<_>>()
        );
        for f in &about {
            assert!(
                !f.evidence.contains("limit") || limit,
                "{name}: nor any other limit: {}",
                f.evidence
            );
            // **And it is described as what the status list said it was.** `provider-logs`
            // arrived in `status.containerStatuses`, so no card may call it an init container or
            // a sidecar and hand the reader the sentences those roles carry.
            assert!(
                !f.evidence.contains("init container") && !f.evidence.contains("sidecar"),
                "{name}: the regular list is where this status arrived: {}",
                f.evidence
            );
        }
    }
}

/// Rule 8's evasion: `subPath` narrows a hostPath mount, so the volume's own path is
/// not what the container gets. `hostPath: /` with `subPath: run/containerd` hands the
/// container the node's container runtime state while the volume still records `/` —
/// the same shape as the `var/run/docker.sock` case rule 8 escalates on, and the one
/// `scripts/broken.yaml` can produce on a kind node.
///
/// **Captured, not synthesized.** This was a one-field synthesis for as long as the
/// only committed mount set no `subPath` at all — where a decode that dropped the
/// field read as correct — and the capture trip it named has landed: the field is now
/// in `hostpath.json`. What makes the assertion mean anything is the pair: one of the
/// two mounts of this volume is narrowed and the other is not, so a decode that
/// dropped `sub_path`, or filled it with the mountPath, or answered one mount's value
/// for both, fails on the object itself.
#[test]
fn a_hostpath_mount_keeps_the_subpath_that_says_what_is_really_mounted() {
    let p = pod("hostpath");
    println!("{:?}", p.host_path_mounts);

    let narrowed = p
        .host_path_mounts
        .iter()
        .find(|m| m.container == "nosy")
        .expect("the captured pod mounts the host in `nosy`");
    assert_eq!(
        narrowed.path, "/",
        "the volume still records the node's root, which is what makes the subPath \
         the only field that says what is really mounted"
    );
    assert_eq!(
        narrowed.sub_path.as_deref(),
        Some("run/containerd"),
        "rule 8 reads the path joined with the subPath; the volume alone says `/` \
         and hides what the container was actually handed"
    );

    let whole = p
        .host_path_mounts
        .iter()
        .find(|m| m.container == "shipper")
        .expect("and the second container mounts the same volume");
    assert_eq!(
        whole.sub_path, None,
        "this mount narrows nothing, so `None` here is a decoded absence and not a \
         default the field was never given: {whole:?}"
    );
}

/// `read_only` and `container` both belong to the **mount**, and one hostPath volume
/// can be mounted twice — a node agent writing to `/var/log` beside a log shipper
/// reading it is the ordinary shape. With one entry per volume the two collapse and
/// rule 8 either misses the writable one or reports the read-only one; without the
/// container name the finding cannot say which of them has the node's root, while
/// `kubectl describe pod` can.
///
/// **Captured, not synthesized:** the second container the capture trip asked for is
/// in `hostpath.json`, so the two mounts are two real mounts of one real volume rather
/// than a clone of the first with its name changed.
#[test]
fn two_containers_mounting_one_hostpath_are_two_mounts_and_are_told_apart() {
    let raw = fixture("hostpath");
    let p = pod("hostpath");
    println!("{:?}", p.host_path_mounts);

    // The premise, out of the capture: one hostPath volume, and every mount below is a
    // mount of it. Two entries from two volumes would satisfy every assertion here
    // while proving nothing about the collapse this test exists over.
    let volumes: Vec<&str> = raw["spec"]["volumes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v.get("hostPath").is_some())
        .map(|v| captured_str(v, &["name"]))
        .collect();
    println!("hostPath volumes: {volumes:?}");
    assert_eq!(
        volumes.len(),
        1,
        "this pod declares one hostPath volume and mounts it twice — that is the \
         shape being asserted: {volumes:?}"
    );

    assert_eq!(
        p.host_path_mounts.len(),
        2,
        "one entry per volume collapses the two and rule 8 sees whichever of them \
         survived: {:?}",
        p.host_path_mounts
    );
    let containers: Vec<&str> = p
        .host_path_mounts
        .iter()
        .map(|m| m.container.as_str())
        .collect();
    assert_eq!(
        containers,
        vec!["nosy", "shipper"],
        "the finding has to name which container has the node's root, and \
         `kubectl describe pod` can"
    );
    let writable: Vec<&str> = p
        .host_path_mounts
        .iter()
        .filter(|m| !m.read_only)
        .map(|m| m.container.as_str())
        .collect();
    assert_eq!(
        writable,
        vec!["nosy"],
        "two mounts of one volume, and rule 8 fires on exactly one of them"
    );
}

/// Rule 8 fires *on* a writable mount, so a decode that hardwired `read_only: false`
/// would pass every assertion a single writable hostPath can make — while turning
/// every CNI and CSI agent in a real cluster into a critical finding. A read-only host
/// mount is what a correctly written node agent uses, and the captured mounts set no
/// `readOnly` key at all where they are writable, so the two are not symmetric in the
/// JSON either: `true` is present and `false` is an absence.
///
/// **Captured, not synthesized, and on both sides.** `hostpath.json` now mounts one
/// volume twice with opposite flags — so neither hardwired answer survives the same
/// pod — and `healthy-hostpath.json` is the posture case the capture trip asked
/// `scripts/healthy.yaml` for: a node agent reading `/var/log`, which the Phase 4
/// posture report lists and rule 8 must leave alone.
#[test]
fn a_read_only_hostpath_mount_is_not_reported_as_writable() {
    let p = pod("hostpath");
    println!("broken: {:?}", p.host_path_mounts);

    let by_container = |name: &str| -> HostPathMount {
        p.host_path_mounts
            .iter()
            .find(|m| m.container == name)
            .unwrap_or_else(|| panic!("the captured pod mounts the host in {name}"))
            .clone()
    };
    // One volume, one path, two containers, opposite answers: whichever way a decode
    // hardwired the flag, one of these two fails.
    assert!(
        !by_container("nosy").read_only,
        "the mount that declares no `readOnly` key is writable — rule 8's escalator"
    );
    assert!(
        by_container("shipper").read_only,
        "and the mount beside it that declares `readOnly: true` is not, on the very \
         same hostPath: {:?}",
        by_container("shipper")
    );

    // The posture case, on the healthy side: a real node agent's read-only mount of a
    // path that is not the node's root.
    let healthy = pod("healthy-hostpath");
    println!("healthy: {:?}", healthy.host_path_mounts);
    assert_eq!(
        healthy.host_path_mounts,
        vec![HostPathMount {
            path: "/var/log".to_string(),
            sub_path: None,
            sub_path_expr: None,
            read_only: true,
            container: "reader".to_string(),
        }],
        "the path is still the node's, but rule 8 has neither of its escalators here \
         — this one belongs in the posture report and nowhere else"
    );
}

/// N6's pod side. Every captured pod was scheduled by the default scheduler with no
/// constraints of its own, so `node_selector` is empty everywhere and a decode that
/// always returned an empty map would look right. Pinning a pod to a node class is
/// the ordinary reason a pod goes Pending forever, which is the finding N6 explains.
/// **Capture trip:** `broken-pending` should be respun with a `nodeSelector` and a
/// matching toleration instead of an unschedulable cpu request.
#[test]
fn a_pod_that_asks_for_a_particular_node_keeps_what_it_asked_for() {
    let mut object: Pod =
        serde_json::from_value(fixture("pending")).expect("pending.json is a Pod");
    let spec = object.spec.as_mut().expect("the captured pod has a spec");
    // One coherent group: `kubectl` users write these two together, because a
    // nodeSelector aimed at a tainted node class is useless without the toleration.
    spec.node_selector = Some(BTreeMap::from([
        ("disktype".to_string(), "ssd".to_string()),
        ("kubernetes.io/os".to_string(), "linux".to_string()),
    ]));
    spec.tolerations = Some(vec![ApiToleration {
        key: Some("dedicated".to_string()),
        operator: Some("Equal".to_string()),
        value: Some("gpu".to_string()),
        effect: Some("NoSchedule".to_string()),
        toleration_seconds: None,
    }]);

    let p = PodSnapshot::from(object);
    println!(
        "selector: {:?}\ntolerations: {:?}",
        p.node_selector, p.tolerations
    );

    assert_eq!(
        p.node_selector,
        BTreeMap::from([
            ("disktype".to_string(), "ssd".to_string()),
            ("kubernetes.io/os".to_string(), "linux".to_string()),
        ]),
        "N6 cannot name the label that is keeping the pod off every node without this"
    );
    assert_eq!(
        p.tolerations,
        vec![Toleration {
            key: Some("dedicated".to_string()),
            operator: Some("Equal".to_string()),
            value: Some("gpu".to_string()),
            effect: Some("NoSchedule".to_string()),
        }],
        "`Equal` against a value is the toleration form the captured pods never carry"
    );
}

/// N2. Every node in the capture was schedulable, so a decode that always answered
/// `false` reads as correct. `kubectl cordon` sets exactly this field.
///
/// **The capture trip this named has landed and this synthesis is now redundant:**
/// `break-nodes` cordons a worker, so the node it picks here is already
/// `unschedulable: true` and the line below sets a field to the value it has.
/// `nodes_decode_with_their_conditions_taints_and_versions` asserts the real one, out
/// of the capture; retiring this belongs with the box that goes through the
/// synthesized tests, not with the one that repaired the assertions the capture
/// reddened.
#[test]
fn a_cordoned_node_decodes_as_unschedulable() {
    let mut object: Node = items::<Node>("nodes")
        .into_iter()
        .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
        .expect("the capture has a worker");
    object
        .spec
        .as_mut()
        .expect("the captured node has a spec")
        .unschedulable = Some(true);

    let n = NodeSnapshot::from(object);
    println!("{}: unschedulable={}", n.id.name, n.unschedulable);
    assert!(
        n.unschedulable,
        "N2 is the whole of 'cordoned and forgotten', and this field is all of it"
    );
}

/// **This one does not depict an object the cluster produces — it proves the two
/// fields are decoded independently of one another**, and the capture structurally
/// cannot.
///
/// [`Taint::added_at`] and [`Taint::value`] come from different writers: the node
/// controller stamps `timeAdded` on the taints it adds and gives them no value, and an
/// operator's `kubectl taint` supplies a value and no `timeAdded`. So no real taint
/// carries both, and each field's *own* proof is on a real object elsewhere — `value`
/// on the captured `dedicated=gpu:NoExecute` in
/// `nodes_decode_with_their_conditions_taints_and_versions`, `added_at` on the
/// controller's taints via the sweep in
/// `the_pinned_now_is_not_before_the_captures_it_is_read_against`. What neither can
/// reach is the pair: a decode that zeroed `value` whenever `added_at` was present, or
/// the reverse, satisfies both of them, because in every committed object one of the
/// two is already `None`.
///
/// **The shape is legal, it is merely undemonstrated** — `validateNodeTaints`
/// (`pkg/apis/core/validation/validation.go`) checks the key, the value, the effect
/// and `<key,effect>` uniqueness and never looks at `TimeAdded`, so any client that
/// sets both produces a valid object. That is the distinction D40 draws: this is not a
/// test against an impossible object, it is a synthesis licensed by a property the
/// captures cannot exhibit. **Capture trip:** none — a cluster is not asked to write
/// an object no controller of it writes.
#[test]
fn a_taint_keeps_its_value_and_the_time_it_was_added() {
    let mut object: Node = items::<Node>("nodes")
        .into_iter()
        .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
        .expect("the capture has a worker");
    object
        .spec
        .as_mut()
        .expect("the captured node has a spec")
        .taints = Some(vec![ApiTaint {
        key: "dedicated".to_string(),
        value: Some("gpu".to_string()),
        effect: "NoExecute".to_string(),
        time_added: Some(time("2026-08-11T22:50:00Z")),
    }]);

    let n = NodeSnapshot::from(object);
    println!("{:?}", n.taints);
    assert_eq!(
        n.taints,
        vec![Taint {
            key: "dedicated".to_string(),
            value: Some("gpu".to_string()),
            effect: "NoExecute".to_string(),
            added_at: Some(time("2026-08-11T22:50:00Z")),
        }],
        "N6 has to name the taint in full — `dedicated=gpu:NoExecute`, not `dedicated`"
    );
}

/// N1 and N3. The capture is of a healthy cluster, so every condition in it is the
/// benign one and a decode that dropped `status`, `reason` or `last_transition` would
/// still satisfy the negative assertions. A kubelet that stops posting is what N1 is,
/// and it is the single most common node failure there is.
/// **Capture trip:** stop the kubelet on one worker before `just fixtures` (`docker
/// exec k8rs-worker systemctl stop kubelet`), wait past the 40s grace, and N1 and N3
/// both get real positive fixtures.
#[test]
fn a_notready_node_keeps_the_status_the_reason_and_when_it_changed() {
    let mut object: Node = items::<Node>("nodes")
        .into_iter()
        .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
        .expect("the capture has a worker");
    let status = object
        .status
        .as_mut()
        .expect("the captured node has a status");
    // One field group: this is what the node controller writes when a kubelet stops
    // reporting and the node then runs out of disk.
    for c in status.conditions.iter_mut().flatten() {
        match c.type_.as_str() {
            "Ready" => {
                c.status = "Unknown".to_string();
                c.reason = Some("NodeStatusUnknown".to_string());
                c.message = Some("Kubelet stopped posting node status.".to_string());
                c.last_transition_time = Some(time("2026-08-11T23:00:00Z"));
            }
            "DiskPressure" => {
                c.status = "True".to_string();
                c.reason = Some("KubeletHasDiskPressure".to_string());
                // The message moves with the other three or the object is one no API
                // server could emit: the capture's "kubelet has no disk pressure"
                // beside `status: True` is a contradiction, and a synthesis is only
                // licensed while every field of it is a value the API produces.
                c.message = Some("kubelet has disk pressure".to_string());
                c.last_transition_time = Some(time("2026-08-11T23:05:00Z"));
            }
            _ => {}
        }
    }

    let n = NodeSnapshot::from(object);
    println!("{:?}", n.conditions);

    let ready = n
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .expect("N1 reads Ready");
    assert_eq!(
        ready.status, "Unknown",
        "a kubelet that went quiet reports Unknown, not False — N1 must see the difference"
    );
    assert_eq!(ready.reason.as_deref(), Some("NodeStatusUnknown"));
    assert_eq!(
        ready.message.as_deref(),
        Some("Kubelet stopped posting node status.")
    );
    assert_eq!(
        ready.last_transition,
        Some(time("2026-08-11T23:00:00Z")),
        "N1 fires only after five minutes, so the timestamp is half the rule"
    );

    let disk = n
        .conditions
        .iter()
        .find(|c| c.type_ == "DiskPressure")
        .expect("N3 reads the pressure conditions");
    assert_eq!(disk.status, "True", "N3 is 'evictions are coming'");
    assert_eq!(disk.reason.as_deref(), Some("KubeletHasDiskPressure"));
    // Asserted so the synthesis above cannot drift back into a contradiction nothing
    // reads: an unasserted synthesized field is how the first one got there.
    assert_eq!(disk.message.as_deref(), Some("kubelet has disk pressure"));
}

/// N5 measures pod requests against **allocatable**, and the gap between capacity and
/// allocatable — what the kubelet and the system daemons reserve — is the entire
/// subject of the rule. kind reports the two as identical because it reserves
/// nothing, so reading the wrong one is invisible in the capture; every managed
/// cluster there is (EKS, GKE, AKS) reserves a substantial slice.
/// **Capture trip:** none — kind cannot produce this. It needs `--kube-reserved` on
/// the kubelet, which is a cluster change, not a fixture.
#[test]
fn allocatable_is_read_and_not_capacity() {
    let mut object: Node = items::<Node>("nodes")
        .into_iter()
        .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
        .expect("the capture has a worker");
    object
        .status
        .as_mut()
        .expect("the captured node has a status")
        .allocatable = Some(BTreeMap::from([
        ("cpu".to_string(), Quantity("3800m".to_string())),
        ("memory".to_string(), Quantity("3484172Ki".to_string())),
    ]));

    let n = NodeSnapshot::from(object);
    println!(
        "allocatable: {:?} / {:?}",
        n.allocatable_cpu, n.allocatable_memory
    );
    assert_eq!(
        n.allocatable_cpu.as_deref(),
        Some("3800m"),
        "the capture reports the same number for capacity and allocatable, so N5 \
         comparing against capacity would miss every overcommit inside the \
         reservation and look right here"
    );
    assert_eq!(n.allocatable_memory.as_deref(), Some("3484172Ki"));
}

/// D45: an empty `state` is not a fourth state, it is a waiting one — upstream's own
/// `ContainerState` doc says "if none of them is specified, the default one is
/// ContainerStateWaiting". The captured statuses all carry a populated state, so the
/// branch has no coverage from any capture; without this, answering `Running` there
/// would turn rule 7 ("running but not ready") into a false CRITICAL on any container
/// whose state the kubelet had not filled in yet.
/// **Capture trip:** none. No cluster can be asked to emit this on purpose; the API
/// definition is the whole of the evidence, which is why the ruling cites it.
#[test]
fn a_container_state_with_nothing_in_it_is_a_waiting_one() {
    let mut object: Pod =
        serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
    object
        .status
        .as_mut()
        .expect("the captured pod has a status")
        .container_statuses
        .as_mut()
        .expect("it has a container status")[0]
        .state = Some(ApiContainerState::default());

    let p = PodSnapshot::from(object);
    let c = container(&p, "quitter");
    println!("empty state decodes as: {:?}", c.state);
    assert_eq!(
        c.state,
        ContainerState::Waiting {
            reason: None,
            message: None
        },
        "the API defines the empty case as waiting, so nothing here may invent another"
    );
}

/// The other field the API defines by its absence. `status.started` is an
/// `Option<bool>` upstream and all twelve captured pods set it, so the null case has
/// never been decoded — and upstream's own `ContainerStatus` doc rules it: "The null
/// value must be treated the same as false." Answering `true` there would claim the
/// kubelet had judged a startup probe it has not yet run — wrong on the one class of
/// workload where this field carries any information at all, the pods that declare a
/// `startupProbe`. It is not what keeps rule 7 off a rolling update: no committed
/// fixture declares a `startupProbe`, so `started` there is true the instant the
/// container runs, and the "since when" is `ready.last_transition` and nothing else
/// (NOTES § D51).
///
/// The same capture asserts the field's `true` in
/// `running_but_not_ready_is_distinguishable_from_waiting`, so the pair pins both
/// answers on one object: a decode that hardwires either one fails one of them.
/// **Capture trip:** none. A kubelet that has not evaluated a startup probe yet omits
/// the field, but that window is not something `just fixtures` can be aimed at; the
/// upstream type and its own doc are the whole of the evidence, as with D45 above.
#[test]
fn a_container_status_that_omits_started_is_not_started() {
    let mut object: Pod =
        serde_json::from_value(fixture("readiness")).expect("readiness.json is a Pod");
    object
        .status
        .as_mut()
        .expect("the captured pod has a status")
        .container_statuses
        .as_mut()
        .expect("it has a container status")[0]
        .started = None;

    let p = PodSnapshot::from(object);
    let c = container(&p, "app");
    println!("started omitted decodes as: {}", c.started);
    assert!(
        !c.started,
        "a missing `started` means false, not true — the kubelet has not said this \
         container passed a startup probe, so nothing here may say that it did"
    );
}

/// Every workload in the capture is either fully ready or entirely absent, so
/// `desired`, `ready`, `updated` and `unavailable` could be read from three other replica
/// counters each and stay green. A rollout in progress separates all of them — which is the
/// only state W2 is ever evaluated in.
///
/// **`updated` is the one whose wrong answer is silence rather than a wrong number**
/// ([`WorkloadSnapshot::updated`], NOTES § D82): read from `replicas` or `availableReplicas`
/// it comes back at or above `desired` on exactly the stuck rollout W2 exists for, and the
/// card never draws. So its distractors are set on both sides of it — 6 above, 3 and 0 below.
///
/// **`unavailable` is the one whose *right* answer is a different field per kind**
/// ([`WorkloadSnapshot::unavailable`]): `unavailableReplicas` on a Deployment,
/// `numberUnavailable` on a DaemonSet, and nothing at all on a ReplicaSet, which has no such
/// field and must not borrow one from the four counters it does carry.
///
/// **`terminating` is the one that is `0` on every committed object** — KEP-3973's counter is
/// non-zero only while a rollout is draining, which is a window no capture landed inside
/// (NOTES § D135) — so without a value set here it could be read off any of the five neighbours
/// and stay green on all six workloads. It is a fifth distinct number below, and on the two
/// kinds that have no such field it has to come back `None`
/// ([`WorkloadSnapshot::terminating`]).
///
/// **Capture trip:** a Deployment whose new revision cannot start (a bad image on the
/// second revision) captured mid-rollout gives the Deployment and its ReplicaSet at
/// once; a DaemonSet with a broken image gives the third.
#[test]
fn desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour() {
    let mut deployment: Deployment = items::<Deployment>("deployments")
        .into_iter()
        .find(|d| d.metadata.name.as_deref() == Some("healthy-deploy"))
        .expect("the capture has a healthy Deployment");
    deployment.spec.as_mut().expect("it has a spec").replicas = Some(5);
    let status = deployment.status.as_mut().expect("it has a status");
    status.replicas = Some(6); // surge: one more pod than wanted exists
    status.ready_replicas = Some(2);
    status.available_replicas = Some(0); // ready, but not past minReadySeconds
    status.updated_replicas = Some(4);
    status.unavailable_replicas = Some(3);
    status.terminating_replicas = Some(7);

    let w = WorkloadSnapshot::from(deployment);
    println!(
        "deployment: desired={:?} ready={:?} updated={:?} unavailable={:?} terminating={:?}",
        w.desired, w.ready, w.updated, w.unavailable, w.terminating
    );
    assert_eq!(
        (w.desired, w.ready, w.updated, w.unavailable, w.terminating),
        (Some(5), Some(2), Some(4), Some(3), Some(7)),
        "desired is what the spec asked for, ready is what is passing probes, updated is \
         how many are on the new template, unavailable is how many are not answering, \
         terminating is how many are on their way out — and no two of the seven counters on \
         this object are equal"
    );

    let mut replicaset: ReplicaSet = items::<ReplicaSet>("healthy-replicasets")
        .into_iter()
        .next()
        .expect("the capture has a healthy ReplicaSet");
    replicaset.spec.as_mut().expect("it has a spec").replicas = Some(5);
    let status = replicaset.status.as_mut().expect("it has a status");
    status.replicas = 6;
    status.ready_replicas = Some(2);
    status.available_replicas = Some(0);
    status.fully_labeled_replicas = Some(4);
    status.terminating_replicas = Some(7);

    let w = WorkloadSnapshot::from(replicaset);
    println!(
        "replicaset: desired={:?} ready={:?} updated={:?} unavailable={:?} terminating={:?}",
        w.desired, w.ready, w.updated, w.unavailable, w.terminating
    );
    assert_eq!(
        (w.desired, w.ready, w.updated, w.unavailable, w.terminating),
        (Some(5), Some(2), Some(6), None, Some(7)),
        "a ReplicaSet's `status.replicas` is not optional and is not the desired count — it \
         is how many pods it has on its one template, which is what `updated` means here \
         (D82). `fullyLabeledReplicas` and `availableReplicas` are neither, and there is no \
         unavailable counter on this kind at all — but there *is* a terminating one, which is \
         the half of KEP-3973 a ReplicaSet does carry"
    );

    let mut daemonset: DaemonSet = items::<DaemonSet>("daemonsets")
        .into_iter()
        .find(|d| d.metadata.name.as_deref() == Some("kindnet"))
        .expect("the capture has kindnet");
    let status = daemonset.status.as_mut().expect("it has a status");
    status.desired_number_scheduled = 4;
    status.number_ready = 2;
    status.current_number_scheduled = 3;
    status.number_available = Some(1);
    status.updated_number_scheduled = Some(0);
    status.number_unavailable = Some(5);

    let w = WorkloadSnapshot::from(daemonset);
    println!(
        "daemonset: desired={:?} ready={:?} updated={:?} unavailable={:?} terminating={:?}",
        w.desired, w.ready, w.updated, w.unavailable, w.terminating
    );
    assert_eq!(
        w.terminating, None,
        "KEP-3973 gave `terminatingReplicas` to Deployments and ReplicaSets and to nothing \
         else, so this kind answers `None` and none of the six counters set above may be \
         borrowed for it — on a cluster with no such field `readyReplicas` still counts a pod \
         on its way out, which is what makes the absence right rather than merely empty"
    );
    assert_eq!(
        (w.desired, w.ready, w.updated, w.unavailable),
        (Some(4), Some(2), Some(0), Some(5)),
        "a DaemonSet wants one pod per matching node, and `currentNumberScheduled` \
         is how many exist — not how many are wanted, and not how many are updated: the \
         one counter that is zero here is the one that has to come back zero, and the \
         unavailable count is spelled `numberUnavailable` on this kind alone"
    );
}

// --- WHAT FAMILY C'S REPORTS READ ---
//
// The fields and types NOTES § D42's window was held open for, added by NOTES § D129's turn and
// read by no rule in `rules.rs` — every consumer is a Phase 4 report. **What is asserted here is
// the decode**: that each one comes off the path it claims and not off the neighbour beside it,
// which for resources is four paths deep and is where the last three defects in this file lived.
//
// **Three of them no committed capture can reach**, and that is stated rather than skipped —
// `the_three_report_inputs_no_capture_can_fill` names them and goes red the moment the trip that
// closes the gap lands, so the hole cannot be filled without the assertions being written.

/// **Four limit numbers off one pod, all different, none of them a request.** `podlimit.json` is
/// the shape the Capacity report's limits row exists for and the one a lazy decode gets wrong in
/// four separate ways: the container declares only a CPU limit, the *pod* declares only a memory
/// one, and both requests sit beside them holding different values again.
#[test]
fn the_limits_row_can_ask_both_levels_and_neither_answers_from_the_request_beside_it() {
    let raw = fixture("podlimit");
    let pod = pod("podlimit");
    let c = container(&pod, "app");

    // The capture has to keep holding the shape, or every assertion below is about nothing.
    assert_eq!(
        captured_str(&raw, &["spec", "resources", "limits", "memory"]),
        "128Mi",
        "podlimit's whole point is a limit declared at pod level and nowhere else"
    );
    assert!(
        at(&raw, &["spec", "resources", "limits"])
            .get("cpu")
            .is_none(),
        "and no CPU limit at that level, which is what makes the pair discriminating"
    );

    // **Pod level.** `cpu_limit` is `None` even though a CPU *request* of `10m` sits in the same
    // object one key away — a decode reading `spec.resources.requests` would answer `Some("10m")`
    // and the limits row would then count this pod as limited when it is not.
    assert_eq!(pod.memory_limit.as_deref(), Some("128Mi"));
    assert_eq!(
        pod.cpu_limit, None,
        "there is no CPU limit at pod level, and the 10m beside it is a request"
    );
    assert_eq!(pod.cpu_request.as_deref(), Some("10m"));

    // **Container level, and it is not a copy of the pod's.** The container declares its own CPU
    // limit; the memory one it reports is the pod's, merged into the status by the kubelet — so
    // `100m`/`128Mi` here is two different objects' text arriving through one field, which is
    // exactly why `effective` reads the status first ([`ContainerSnapshot::cpu_limit`]).
    assert_eq!(c.cpu_limit.as_deref(), Some("100m"));
    assert_eq!(c.memory_limit.as_deref(), Some("128Mi"));
    assert_eq!(
        c.cpu_request.as_deref(),
        Some("10m"),
        "the request is a fifth number and the limit is not read from it"
    );
    println!(
        "podlimit: pod cpu_limit {:?} memory_limit {:?} · container cpu_limit {:?} \
         memory_limit {:?} cpu_request {:?}",
        pod.cpu_limit, pod.memory_limit, c.cpu_limit, c.memory_limit, c.cpu_request
    );
}

/// **What the kubelet reserved is a third number, and `resize.json` is where all three disagree
/// at once** — the in-place-resize shape NOTES § D46 sent to Phase 4 and box 1753 named. The spec
/// asks for `24277416Ki`, the status says `64Mi` was enacted, and `allocatedResources` says
/// `64Mi` is what the node is actually holding. A decode taking `allocated_*` off `spec` or off
/// `status.resources` cannot be told apart on any other capture in the repository.
#[test]
fn what_the_kubelet_reserved_decodes_from_its_own_field_and_not_from_the_two_beside_it() {
    let raw = fixture("resize");
    let asked = raw["spec"]["containers"][0]["resources"]["limits"]["memory"]
        .as_str()
        .expect("resize.json's container declares the memory limit it asked for");
    let status = captured_status(&raw, "containerStatuses", "app");
    assert_ne!(
        asked,
        captured_str(status, &["resources", "limits", "memory"]),
        "resize.json is only evidence while the spec and the status still disagree"
    );

    let resized = pod("resize");
    let c = container(&resized, "app");
    assert_eq!(c.allocated_memory.as_deref(), Some("64Mi"));
    assert_eq!(
        c.allocated_cpu, None,
        "this pod reserves no CPU, and an absent key is not the memory value beside it"
    );
    assert_eq!(
        c.memory_limit.as_deref(),
        Some("64Mi"),
        "the enacted limit, which happens to agree here — the spec's 24277416Ki does not"
    );

    // The one capture where the reservation and the *limit* are different numbers, so those two
    // fields cannot be reading one path: `podlimit`'s container reserves 10m and is limited to
    // 100m.
    let with_limit = pod("podlimit");
    let limited = container(&with_limit, "app");
    assert_eq!(limited.allocated_cpu.as_deref(), Some("10m"));
    assert_eq!(limited.cpu_limit.as_deref(), Some("100m"));

    // **The pair no capture separates, separated with one field set** (NOTES § D40). On every
    // committed capture `allocatedResources` holds exactly what `status.resources.requests`
    // holds — they only diverge inside the in-place-resize window, and none of the captures
    // caught one — so a decode reading the neighbour passes every assertion above. Measured:
    // planting that decode left this test green until this block existed.
    //
    // The shape is the one the field was added for and one the API emits: the kubelet holds the
    // old reservation while a new request is being enacted, so `allocatedResources` lags
    // `resources.requests` by exactly one resize.
    let mid_resize = capture_but("resize", |pod| {
        let status = pod
            .status
            .as_mut()
            .expect("resize.json's kubelet has reported on it");
        let c = &mut status
            .container_statuses
            .as_mut()
            .expect("with a container status")[0];
        c.allocated_resources = Some(
            [(
                "memory".to_string(),
                k8s_openapi::apimachinery::pkg::api::resource::Quantity("32Mi".to_string()),
            )]
            .into_iter()
            .collect(),
        );
    });
    let lagging = container(&mid_resize, "app");
    assert_eq!(
        lagging.allocated_memory.as_deref(),
        Some("32Mi"),
        "the reservation is read from `allocatedResources` and from nowhere else"
    );
    assert_eq!(
        lagging.memory_request.as_deref(),
        Some("64Mi"),
        "while the enacted request one key away still says 64Mi — which is the whole divergence, \
         and asserting only the first would not have seen it"
    );
    println!(
        "resize: allocated {:?}/{:?} vs spec {asked} · podlimit: allocated {:?} vs limit {:?}",
        c.allocated_cpu, c.allocated_memory, limited.allocated_cpu, limited.cpu_limit
    );
}

/// **Every captured pod carries the labels a PodDisruptionBudget would be matched against.**
/// Without them the Drain safety report has no join and answers *node-1 is ready to drain* about
/// a node it could not check (NOTES § D129).
#[test]
fn a_pod_carries_the_labels_a_disruption_budget_is_matched_against() {
    let raw = fixture("healthy");
    let captured = at(&raw, &["metadata", "labels"])
        .as_object()
        .expect("every pod this cluster creates is labelled");
    let pod = pod("healthy");

    assert_eq!(pod.labels.len(), captured.len());
    for (key, value) in captured {
        assert_eq!(
            pod.labels.get(key).map(String::as_str),
            value.as_str(),
            "{key} arrived changed or not at all"
        );
    }
    // A derived list asserts it found something (CLAUDE.md § tests must not lie): an empty map
    // and a map that was never read print the same green line. **The key is the one the capture
    // actually uses** — `scripts/broken.yaml` labels these pods `demo`, not `app`, and asserting
    // the habitual name was this test's own first red.
    assert_eq!(pod.labels.get("demo").map(String::as_str), Some("healthy"));

    // And it is the *whole* map, not a subset — a selector may name any key, so there is no
    // subset to keep and a prune that kept one would silently stop matching.
    let every: Vec<usize> = every_captured_pod()
        .iter()
        .map(|p| p.labels.len())
        .collect();
    assert!(
        every.iter().all(|n| *n > 0),
        "every captured pod is labelled: {every:?}"
    );
}

/// **A `LabelSelector` decodes both halves**, and `matchExpressions` is the half that is
/// load-bearing: a PDB written with expressions alone matches no pod under a `matchLabels`-only
/// reader, so the budget looks like one protecting nothing and the report calls a node safe.
///
/// **No captured object carries one** — all eleven selectors in the repository are `matchLabels`
/// — so the expression half is a committed capture with one field set, under NOTES § D40.
#[test]
fn a_label_selector_decodes_its_expressions_and_not_only_its_labels() {
    let mut deploys = items::<Deployment>("deployments");
    let d = deploys
        .iter_mut()
        .find(|d| d.metadata.name.as_deref() == Some("healthy-deploy"))
        .expect("the capture holds healthy-deploy");
    let captured = d
        .spec
        .as_ref()
        .map(|s| s.selector.clone())
        .expect("a Deployment cannot exist without a selector");

    let plain = Selector::from(captured.clone());
    assert_eq!(
        plain.match_labels.get("app").map(String::as_str),
        Some("healthy-deploy")
    );
    assert!(
        plain.match_expressions.is_empty(),
        "no committed capture carries one, which is the reason for the half below"
    );

    // One field set on the captured selector — the operator and the shape are both what the
    // pinned API emits, and `In` with one value is what `kubectl` writes for `-l app in (x)`.
    let mut with_expression = captured;
    with_expression.match_expressions = Some(vec![
        k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelectorRequirement {
            key: "app".to_string(),
            operator: "In".to_string(),
            values: Some(vec!["healthy-deploy".to_string()]),
        },
    ]);
    let rich = Selector::from(with_expression);
    assert_eq!(rich.match_expressions.len(), 1);
    assert_eq!(rich.match_expressions[0].key, "app");
    assert_eq!(rich.match_expressions[0].operator, "In");
    assert_eq!(rich.match_expressions[0].values, vec!["healthy-deploy"]);
    assert_eq!(
        rich.match_labels, plain.match_labels,
        "the labels half is untouched by the expressions half"
    );

    // `Selector::default` is the value a **present** selector written `{}` decodes to, and
    // upstream reads it as *every object* — `all` over two empty halves. An **absent** selector
    // is not this value and stopped sharing it on 2026-08-21: it is `None` on the field, which
    // `policy/v1` reads as *selects no pods*, the reverse of the `v1beta1` it replaced
    // ([`the_two_ways_a_budget_can_say_nothing_about_which_pods_and_they_are_not_one_value`]).
    let empty = Selector::default();
    assert!(empty.match_labels.is_empty() && empty.match_expressions.is_empty());
}

/// The four captured Services, **including the one with no selector at all**. `kubernetes` in
/// `default` exists on every cluster ever built and its endpoints are written by the apiserver,
/// so *matches no pod* is not a thing the Waste report may say about it.
///
/// **The fourth arrived with the 2026-08-20 trip** — `broken-noendpoints`, the Service Waste's
/// headline row is about, whose own EndpointSlice is
/// [`the_service_that_reaches_nothing_is_a_slice_with_no_endpoints`]'s subject.
#[test]
fn the_captured_services_decode_and_an_absent_selector_is_not_an_empty_one() {
    let raw = fixture("services");
    let services: Vec<ServiceSnapshot> = items::<Service>("services")
        .into_iter()
        .map(Into::into)
        .collect();
    assert_eq!(services.len(), 4, "the capture holds four");

    let by = |name: &str| {
        services
            .iter()
            .find(|s| s.id.name == name)
            .unwrap_or_else(|| panic!("no {name} among {:?}", services))
    };
    let sts = by("broken-sts");
    assert_eq!(
        sts.selector.get("app").map(String::as_str),
        Some("broken-sts"),
        "read off spec.selector, and the capture agrees: {}",
        captured_str(
            captured_item(&raw, "broken-sts"),
            &["spec", "selector", "app"]
        )
    );
    assert_eq!(sts.id.namespace.as_deref(), Some("default"));
    assert_eq!(
        sts.id.kind,
        ObjectKind::Other("Service".to_string()),
        "core group, so the kind is unqualified — `api_kind` reads it off the type"
    );

    assert!(
        by("kubernetes").selector.is_empty(),
        "the apiserver's own Service selects nothing and is not a finding"
    );
    assert!(
        by("kube-dns").selector.contains_key("k8s-app"),
        "and the third one is selected by a key that is not `app`, so nothing here is \
         hardcoded to one label name"
    );

    // **The fourth has a selector and still reaches nothing**, which is the pair Waste needs:
    // the Service is well-formed, so the emptiness is a fact about its EndpointSlice and not
    // about this object. A row that read *no selector* here would file `kubernetes` as the
    // outage and this one as fine — exactly backwards.
    assert_eq!(
        by("broken-noendpoints")
            .selector
            .get("app")
            .map(String::as_str),
        Some("broken-noendpoints"),
        "read off spec.selector, and the capture agrees: {}",
        captured_str(
            captured_item(&raw, "broken-noendpoints"),
            &["spec", "selector", "app"]
        )
    );
    let pods = every_captured_pod();
    assert!(pods.len() > 40, "walked {} pods", pods.len());
    assert!(
        !pods
            .iter()
            .any(|p| p.labels.get("app").map(String::as_str) == Some("broken-noendpoints")),
        "and no captured pod carries that label — the emptiness of its slice is a property \
         of the cluster the capture was taken from, not of the decode"
    );
}

/// **C3's input: a pending CSR is one with no verdict on it**, and the type carries the fact
/// rather than the verdict. `csr-pending.json` is `scripts/make-csr.sh`'s object, guarded at
/// capture time to have reached `status: {}`.
///
/// **The security half is not decoration**: the snapshot type has no field for `spec.request`
/// (the CSR body) or `spec.extra` (the requester's credential id), and this asserts that the
/// decoded value cannot carry either — `scripts/fixture-audit.sh` refuses them in the committed
/// file and the type refuses them one layer earlier.
#[test]
fn a_pending_certificate_request_decodes_as_pending_and_carries_no_credential() {
    let raw = fixture("csr-pending");
    assert!(
        at(&raw, &["status"])
            .as_object()
            .is_some_and(serde_json::Map::is_empty),
        "the fixture is only evidence about C3 while nothing has approved it"
    );
    let csr: CertificateSigningRequest = serde_json::from_value(raw.clone())
        .expect("csr-pending.json is a CertificateSigningRequest");
    let snap = CertificateRequestSnapshot::from(csr);

    assert_eq!(snap.id.name, "k8rs-pending-fixture");
    assert_eq!(
        snap.id.namespace, None,
        "a CSR is cluster-scoped, so `-n \"\"` can never be built from it"
    );
    assert_eq!(snap.signer_name, "kubernetes.io/kube-apiserver-client");
    // **`api_kind`'s other branch.** The Service test proves the core group, where a kind stays
    // unqualified; this is a kind in a real API group, and NOTES § D36 says it is qualified by
    // it. Asserted on a captured object, so the mapping is proven from both sides rather than
    // from the one that happens to be shortest.
    assert_eq!(
        snap.id.kind,
        ObjectKind::Other("CertificateSigningRequest.certificates.k8s.io".to_string())
    );
    assert!(
        snap.conditions.is_empty(),
        "pending is the absence of Approved, Denied and Failed"
    );
    assert!(!snap.issued, "and nothing has been signed");

    // **The other side of the same object, with two fields set** (NOTES § D40). The capture is
    // pending by construction — `scripts/make-csr.sh` refuses to write anything else — so on it
    // alone a decode hardcoding `conditions: vec![]` and `issued: false` is green, which is
    // what planting both proved. An approved-and-issued request is a value the API emits every
    // time a kubelet joins.
    let mut approved: CertificateSigningRequest =
        serde_json::from_value(raw.clone()).expect("the same capture");
    let status = approved.status.get_or_insert_with(Default::default);
    status.conditions = Some(vec![
        k8s_openapi::api::certificates::v1::CertificateSigningRequestCondition {
            type_: "Approved".to_string(),
            status: "True".to_string(),
            reason: Some("AutoApproved".to_string()),
            message: Some("Auto approving kubelet client certificate".to_string()),
            last_transition_time: None,
            last_update_time: None,
        },
    ]);
    status.certificate = Some(k8s_openapi::ByteString(b"-- signed --".to_vec()));
    let signed = CertificateRequestSnapshot::from(approved);
    assert_eq!(signed.conditions.len(), 1);
    assert_eq!(signed.conditions[0].type_, "Approved");
    assert_eq!(signed.conditions[0].status, "True");
    assert_eq!(
        signed.conditions[0].reason.as_deref(),
        Some("AutoApproved"),
        "the CSR condition joins the six the `condition_from!` macro already writes once, so its \
         reason and message arrive like every other controller's"
    );
    assert!(signed.issued, "and `status.certificate` is set");
    assert!(
        !format!("{signed:?}").contains("signed"),
        "the certificate is a bit and never the bytes — nothing prints one"
    );

    // The PEM body is in the capture and may not be in the snapshot.
    let body = captured_str(&raw, &["spec", "request"]);
    let debug = format!("{snap:?}");
    assert!(body.len() > 100, "the capture does carry a request body");
    assert!(
        !debug.contains(body) && !debug.contains("BEGIN CERTIFICATE"),
        "no part of the request reaches this type, Debug included"
    );
    println!("csr: signer {} · issued {}", snap.signer_name, snap.issued);
}

// --- THE FIVE FAMILY C INPUTS THE 2026-08-20 TRIP PUT AN OBJECT BEHIND ---
//
// **These five were one test until 2026-08-20, and it was called
// `what_no_committed_capture_can_prove_about_family_cs_inputs`.** It asserted that a PDB, a PVC,
// an EndpointSlice, a `spec.overhead` and a pod mounting a claim were all absent, so that it
// would **go red the moment the capture trip landed** and the gap could not be filled without
// its assertions being written (NOTES § D129, § D130). The trip landed, it went red, and what
// replaces it is one positive test per item. A test named *what no capture can prove* that
// proves five things is the defect this repository keeps paying for, so the name went with the
// gap it was holding; [`what_family_cs_inputs_still_have_no_object_for`] holds what is left.

/// **The two committed PodDisruptionBudgets — the one that blocks a drain and the one that does
/// not** (NOTES § D129, § D130). Drain safety's whole reason for existing is the first: a node
/// carrying `broken-pdb-floor`'s pods can be cordoned and drained forever and the drain never
/// finishes, because the budget's floor is already the number of healthy pods there are.
///
/// **The row reads `status.desiredHealthy` and never `spec.minAvailable`** (NOTES § D130).
/// `minAvailable` is an `IntOrString` and `minAvailable: "50%"` is legal and common, so a row
/// reading it prints *"wants at least 50% copies"* or nothing at all; the API server resolves it
/// **and** `maxUnavailable` into the status field. The capture fixes both on one object —
/// `minAvailable: 2` beside `desiredHealthy: 2` — so the reading that is only sometimes correct
/// stays visible, and the half below moves the spec field on a decoded copy to prove which one
/// the decode actually read (NOTES § D40, § D29: a fixture whose two fields always agree cannot
/// prove which one was read).
///
/// **Two more facts joined it on 2026-08-21, and neither is a counter: whether the counters are
/// current, and why they are what they are** (NOTES § D46's class — a field the API sends and
/// the contract drops at ingest). Upstream refuses every eviction while `metadata.generation`
/// is ahead of `status.observedGeneration`, and `DisruptionAllowed`'s `reason` is the only
/// thing separating a workload at its floor from a controller that could not compute the number
/// at all. Both interesting shapes are **plants** (NOTES § D40), each carrying what a trip
/// would have to do to replace it; the current-and-`InsufficientPods` pair is the capture.
#[test]
fn the_blocking_disruption_budget_and_the_one_with_room() {
    let raw = fixture("poddisruptionbudgets");
    let budgets = disruption_budgets();
    let by = |name: &str| {
        budgets
            .iter()
            .find(|b| b.id.name == name)
            .unwrap_or_else(|| panic!("no {name} among {budgets:?}"))
    };
    // The one condition the disruption controller writes. **Picking it out of the list is a
    // report's job and is done here, not in the decode** — which is the whole reason the
    // conditions are carried whole (`DisruptionBudgetSnapshot::conditions`); what this layer
    // owes is that it can be found at all.
    fn allowed(b: &DisruptionBudgetSnapshot) -> &Condition {
        b.conditions
            .iter()
            .find(|c| c.type_ == "DisruptionAllowed")
            .unwrap_or_else(|| panic!("no DisruptionAllowed condition on {}", b.id.name))
    }

    let blocking = by("broken-pdb-floor");
    assert_eq!(
        blocking.id.kind,
        ObjectKind::Other("PodDisruptionBudget.policy".to_string()),
        "a kind in a real API group is qualified by it (NOTES § D36) — asserted here off a \
         decoded object rather than off `api_kind` alone, which is what the whole-type absence \
         used to leave it as"
    );
    assert_eq!(blocking.id.namespace.as_deref(), Some("default"));
    assert_eq!(
        blocking.disruptions_allowed,
        Some(captured_i32(
            captured_item(&raw, "broken-pdb-floor"),
            &["status", "disruptionsAllowed"]
        )),
        "the controller's own answer, and it is the one field that says *this drain blocks*"
    );
    assert_eq!(
        blocking.disruptions_allowed,
        Some(0),
        "and the capture is on the blocking side of it, or this object is not this test's fixture"
    );
    assert_eq!(
        (blocking.current_healthy, blocking.desired_healthy),
        (Some(2), Some(2)),
        "the two numbers the row's sentence is built from — *wants at least 2 copies and has \
         exactly 2* — and their being equal is what leaves no room to evict into"
    );
    assert_eq!(
        selector_of(blocking)
            .match_labels
            .get("app")
            .map(String::as_str),
        Some("healthy-deploy"),
        "and the selector says which pods it protects, or the join has a budget and no way to \
         find the pods it is about"
    );
    assert!(
        selector_of(blocking).match_expressions.is_empty(),
        "this one is `matchLabels` alone — the expression half is \
         `a_label_selector_decodes_its_expressions_and_not_only_its_labels`'s subject"
    );

    // --- WHETHER THOSE THREE NUMBERS ARE CURRENT, AND WHY THEY ARE WHAT THEY ARE ---
    //
    // Both facts were in the committed bytes and dropped by this type until 2026-08-21, which
    // is NOTES § D46's class exactly. Upstream's eviction handler compares
    // `status.observedGeneration` against `metadata.generation` and refuses **every** eviction
    // while the spec is ahead — `TooManyRequests`, whatever `disruptionsAllowed` says — and
    // `reason` is the only field separating *the workload is at its floor* from *the controller
    // could not compute this at all*.
    let raw_blocking = captured_item(&raw, "broken-pdb-floor");
    assert_eq!(
        (blocking.generation, blocking.observed_generation),
        (
            Some(captured_i64(raw_blocking, &["metadata", "generation"])),
            Some(captured_i64(
                raw_blocking,
                &["status", "observedGeneration"]
            )),
        ),
        "both sides of the comparison the API server itself makes, each read off the field it \
         lives in — one filled from the other would pass every *are these numbers current* \
         check that could ever be written against it"
    );
    assert_eq!(
        (blocking.generation, blocking.observed_generation),
        (Some(1), Some(1)),
        "and on the capture the controller has caught up, so the three counters above are ones \
         an eviction would actually be judged by — the plants below are the shapes where they \
         are not"
    );

    let raw_condition = &at(raw_blocking, &["status", "conditions"])[0];
    assert_eq!(
        captured_str(raw_condition, &["type"]),
        "DisruptionAllowed",
        "the capture carries the disruption controller's own condition, or everything below is \
         asserted about a condition this object does not have"
    );
    assert_eq!(
        allowed(blocking).reason.as_deref(),
        Some(captured_str(raw_condition, &["reason"])),
        "the controller's own word for *why*, carried verbatim (NOTES § D37)"
    );
    assert_eq!(
        (
            allowed(blocking).reason.as_deref(),
            allowed(blocking).status.as_str()
        ),
        (Some("InsufficientPods"), "False"),
        "*the workload is at its floor* — the reading whose action is **run one more copy**, \
         and the tri-state agrees with the zero above rather than being inferred from it"
    );
    assert_eq!(
        captured_str(raw_condition, &["message"]),
        "",
        "the committed bytes hold an empty message on this shape of condition, or the next \
         assertion is proving nothing"
    );
    assert_eq!(
        allowed(blocking).message,
        None,
        "`metav1::Condition` declares `message` a required string, so `\"\"` is how it spells \
         *absent* — `Some(\"\")` here would draw a blank explanation line under every budget \
         in the corpus, both of which carry exactly that"
    );
    assert_eq!(
        allowed(blocking).last_transition,
        Some(captured_time(raw_condition, &["lastTransitionTime"])),
        "and the moment the controller last changed its mind survives the required-to-optional \
         crossing — off the captured string, so a decode filling it from any other time on the \
         object is not the same as one that carried it"
    );

    // **The negative, and it is what lets the positive fail.** Without a budget that allows an
    // eviction, *disruptions_allowed is 0* is satisfied by a decode that hardcodes zero, or by
    // one reading a field that is absent on every PDB.
    let room = by("healthy-pdb-room");
    assert_eq!(
        room.disruptions_allowed,
        Some(1),
        "one pod may go, so a drain of its node finishes — the shape the blocking card must not \
         be drawn on"
    );
    assert_eq!(
        selector_of(room)
            .match_labels
            .get("app")
            .map(String::as_str),
        Some("broken-rollout"),
        "and it protects a different workload, so the two budgets are told apart by their \
         selectors and not only by their names"
    );
    assert_ne!(
        blocking.selector, room.selector,
        "a decode that dropped the selector would give both budgets the same `None` and every \
         assertion about *which pods* would pass on both"
    );
    assert_eq!(
        (
            allowed(room).reason.as_deref(),
            allowed(room).status.as_str()
        ),
        (Some("SufficientPods"), "True"),
        "and the reason is the other one of the pair, so a decode returning a fixed string — or \
         the first condition of whichever object it was handed — is caught here"
    );
    assert_eq!(
        allowed(room).message,
        None,
        "and this one's message is empty in the committed bytes too, which is what makes \
         `Some(\"\")` a blank line under *every* budget in the corpus rather than one"
    );
    assert_eq!(
        (room.generation, room.observed_generation),
        (Some(1), Some(1)),
        "this budget is current too, which is what makes the plant below a difference of one \
         field rather than a difference between two objects"
    );

    // --- `status.desiredHealthy`, NOT `spec.minAvailable` (NOTES § D130) ---
    //
    // The committed bytes have the two agreeing on both objects, so on the capture alone a
    // decode reading the spec is green. One field moved, in the shape the field exists for: a
    // percentage, which is what `minAvailable` is an `IntOrString` for and what a spec reader
    // would print as *"wants at least 50% copies"*.
    let mut percentage: PodDisruptionBudget =
        serde_json::from_value(captured_item(&raw, "broken-pdb-floor").clone())
            .expect("the same capture");
    percentage
        .spec
        .get_or_insert_with(Default::default)
        .min_available =
        Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String("50%".to_string()));
    let still = DisruptionBudgetSnapshot::from(percentage);
    assert_eq!(
        still.desired_healthy,
        Some(2),
        "the API server resolved the floor into the status and that is the only field read — a \
         decode reaching into `spec.minAvailable` cannot even name this value"
    );
    assert_eq!(
        still.disruptions_allowed, blocking.disruptions_allowed,
        "and nothing else moved with it"
    );

    // --- A STATUS THAT HAS NOT CAUGHT UP WITH ITS SPEC (NOTES § D40) ---
    //
    // **The false green light, and it is the budget with room that produces it.** Upstream's
    // eviction handler refuses every eviction while `metadata.generation` is ahead of
    // `status.observedGeneration`, so this object says *one pod may go* and the API server would
    // say `TooManyRequests` to all of them — a drain the report called ready that then hangs.
    //
    // **The capture does not hold it, and an ordinary trip could not**: the API server bumps the
    // generation on a spec write and the disruption controller answers well inside the time a
    // capture takes, so a photographed PDB has the two equal — `broken-pdb-floor` and
    // `healthy-pdb-room` both do, asserted above. **What a trip would have to do to replace this
    // plant, and it is not an ordinary one**: stop
    // kube-controller-manager on the control-plane node (move its static-pod manifest out of
    // `/etc/kubernetes/manifests`), `kubectl patch` the budget's `minAvailable`, capture, then
    // put the manifest back — a wedged controller is the same shape and the operator case the
    // finding is about. One field is moved here instead, to the value the API server itself
    // writes on any spec edit.
    let mut edited: PodDisruptionBudget =
        serde_json::from_value(captured_item(&raw, "healthy-pdb-room").clone())
            .expect("the same capture");
    edited.metadata.generation = Some(2);
    let stale = DisruptionBudgetSnapshot::from(edited);
    assert_eq!(
        stale.disruptions_allowed,
        Some(1),
        "the counter still says a pod may be evicted — which is exactly why the report may not \
         stop at it"
    );
    assert_eq!(
        (stale.generation, stale.observed_generation),
        (Some(2), Some(1)),
        "and the two numbers disagree, in the direction that means *the controller has not seen \
         this budget yet*, with both values on the screen for the reader to check"
    );
    assert_eq!(
        (
            stale.disruptions_allowed,
            stale.current_healthy,
            stale.desired_healthy,
            stale.observed_generation
        ),
        (
            room.disruptions_allowed,
            room.current_healthy,
            room.desired_healthy,
            room.observed_generation
        ),
        "and nothing else moved with it — the plant differs from the object it was made from in \
         `metadata.generation` alone, which is what makes the disagreement above attributable \
         to that field and not to a second one nobody named"
    );

    // --- `SyncFailed`: THE SAME ZERO, A DIFFERENT SENTENCE (NOTES § D40) ---
    //
    // `disruptionsAllowed: 0` has two causes and one of them is not the workload's fault: under
    // `SyncFailed` the controller could not resolve the workload's `scale` subresource at all —
    // a CRD owner, a missing verb — so the counters beside it are not a measurement of
    // anything, and *"wants at least 2 copies and has exactly 2 → run one more copy"* would be
    // a sentence invented out of numbers nobody computed.
    //
    // **The capture cannot hold this either**: producing it takes a workload whose scale
    // subresource fails, and `scripts/broken.yaml` has none — a trip would have to add a PDB
    // selecting the pods of a CRD the controller has no `scale` access to. The value is the
    // API's own: `PodDisruptionBudgetStatus.conditions` in k8s-openapi v1_36 documents
    // `SyncFailed`, `InsufficientPods` and `SufficientPods` as the three the disruption
    // controller writes, so the plant is a value upstream demonstrably produces.
    let mut unsynced: PodDisruptionBudget =
        serde_json::from_value(captured_item(&raw, "broken-pdb-floor").clone())
            .expect("the same capture");
    unsynced
        .status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == "DisruptionAllowed")
        .expect("the captured budget carries the condition the plant moves")
        .reason = "SyncFailed".to_string();
    let failed = DisruptionBudgetSnapshot::from(unsynced);
    assert_eq!(
        (
            failed.disruptions_allowed,
            failed.current_healthy,
            failed.desired_healthy
        ),
        (
            blocking.disruptions_allowed,
            blocking.current_healthy,
            blocking.desired_healthy
        ),
        "every number is the blocking budget's — the two objects are indistinguishable on the \
         counters, which is what makes the reason the only field that can tell them apart"
    );
    assert_eq!(
        allowed(&failed).reason.as_deref(),
        Some("SyncFailed"),
        "and it says the controller could not compute those numbers, so a row built from them \
         would be inventing its sentence"
    );
    assert_ne!(
        allowed(&failed).reason,
        allowed(blocking).reason,
        "the one field of difference, or the decode is not reading the reason at all"
    );

    // --- `None` IS NOT ZERO (the type's own doc) ---
    //
    // The three counters are absent until the disruption controller has looked at the budget,
    // and reading that as zero calls every freshly created budget blocking. No capture holds
    // that moment — the controller answers in well under the time a capture takes — so the
    // status is removed from a decoded copy, which is the shape the API emits in the seconds
    // after a `kubectl apply`.
    let mut fresh: PodDisruptionBudget =
        serde_json::from_value(captured_item(&raw, "broken-pdb-floor").clone())
            .expect("the same capture");
    fresh.status = None;
    let unanswered = DisruptionBudgetSnapshot::from(fresh);
    assert_eq!(
        (
            unanswered.disruptions_allowed,
            unanswered.current_healthy,
            unanswered.desired_healthy
        ),
        (None, None, None),
        "nobody has looked yet, and that is not the same fact as *nothing may be evicted* — a \
         zero here puts a blocking card on every budget created in the last second"
    );
    assert_eq!(
        unanswered.selector, blocking.selector,
        "and the selector is in the spec, so it survives a status nobody has written"
    );
    assert_eq!(
        (unanswered.observed_generation, unanswered.conditions.len()),
        (None, 0),
        "nobody has looked, so there is no generation the controller has observed and no reason \
         it could have given — *nobody looked* and *nothing to find* stay different facts \
         (NOTES § D129)"
    );
    assert_eq!(
        (unanswered.generation, blocking.generation),
        (Some(1), Some(1)),
        "while the generation is in the metadata and survives — which is what makes a \
         freshly-applied budget read as *not caught up* rather than as *caught up at zero*, the \
         two `None`s a report would otherwise compare"
    );

    println!(
        "budgets: {}",
        budgets
            .iter()
            .map(|b| format!(
                "{} allows {:?} (healthy {:?}/{:?}) for {:?} — gen {:?}/observed {:?}, {:?}",
                b.id.name,
                b.disruptions_allowed,
                b.current_healthy,
                b.desired_healthy,
                selector_of(b).match_labels,
                b.generation,
                b.observed_generation,
                allowed(b).reason
            ))
            .collect::<Vec<String>>()
            .join(" · ")
    );
    println!(
        "planted: {} allows {:?} but gen {:?}/observed {:?} · {} allows {:?} because {:?}",
        stale.id.name,
        stale.disruptions_allowed,
        stale.generation,
        stale.observed_generation,
        failed.id.name,
        failed.disruptions_allowed,
        allowed(&failed).reason
    );
}

/// **The join a budget's selector is, on both sides real: the captured pods' labels against the
/// captured budgets' `matchLabels`** (NOTES § D129, § D131).
///
/// **The matcher itself is Phase 5's and does not exist yet**, so what is proved here is that
/// both halves arrive in a form one can be written against — the labels whole (that is
/// [`a_pod_carries_the_labels_a_disruption_budget_is_matched_against`]) and the selector as a
/// key-value map rather than as an opaque string.
///
/// **This test used to assert that the join was empty**, because the trip photographed
/// `scripts/broken.yaml`'s pods and neither budget selects one of those. It was written to redden
/// the moment a trip captured one — and on 2026-08-20 a trip captured two, and it stayed green,
/// because [`every_captured_pod`] chained `kube-system-pods` by name and the new `List` was
/// invisible to it (NOTES § D131). The tripwire is gone because the gap it held is closed on one
/// side; what replaces it is the join run for real, plus the *other* side still saying it is
/// open.
///
/// **The asymmetry is the point.** `broken-pdb-floor` protects `app=healthy-deploy` and the
/// corpus holds those two pods; `healthy-pdb-room` protects `app=broken-rollout` and the corpus
/// holds none of those, so the *has room* budget has no pods to prove it with. That half is
/// asserted, not described: a trip that captures `broken-rollout`'s pods reddens this test, which
/// is how the remaining half gets written instead of forgotten.
///
/// **The join is namespaced**, like the claim join above it: a PodDisruptionBudget protects pods
/// in its own namespace and nowhere else, and since `kube-system`'s pods entered
/// [`every_captured_pod`] a namespace-blind matcher is one relabelled DaemonSet away from
/// matching across the boundary.
#[test]
fn the_pods_the_blocking_budget_protects_and_the_ones_no_budget_can_be_joined_to() {
    let budgets = disruption_budgets();
    let pods = every_captured_pod();
    assert!(pods.len() > 50, "walked {} pods", pods.len());

    // A `matchLabels`-only selector matches when every pair of it is a pair of the pod's labels.
    // Written here and not in `rules.rs`: the report that owns the real matcher is a later box,
    // and a second implementation of it is what NOTES § D46 forbids — this one is confined to
    // this test and reads `matchLabels` alone, which is all any committed selector carries.
    let selects = |selector: Option<&Selector>, labels: &BTreeMap<String, String>| {
        // **`None` is `policy/v1`'s *selects no pods***, and a present one is read on
        // `matchLabels` alone — all any committed selector carries. The `all` is upstream's,
        // including over an empty map, where it answers *every pod in the namespace*; the
        // guard that used to sit here answering `false` there is the fold NOTES § D46 caught.
        selector.is_some_and(|s| {
            s.match_expressions.is_empty()
                && s.match_labels.iter().all(|(k, v)| labels.get(k) == Some(v))
        })
    };
    let budget = |name: &str| {
        budgets
            .iter()
            .find(|b| b.id.name == name)
            .unwrap_or_else(|| panic!("no {name} among {budgets:?}"))
    };
    let matches = |b: &DisruptionBudgetSnapshot, p: &PodSnapshot| {
        p.id.namespace == b.id.namespace && selects(b.selector.as_ref(), &p.labels)
    };
    let protects = |b: &DisruptionBudgetSnapshot| -> Vec<String> {
        let mut names: Vec<String> = pods
            .iter()
            .filter(|p| matches(b, p))
            .map(|p| p.id.name.clone())
            .collect();
        names.sort();
        names
    };

    let floor = budget("broken-pdb-floor");
    let room = budget("healthy-pdb-room");
    println!(
        "{} pods · {} {:?} -> {:?} · {} {:?} -> {:?}",
        pods.len(),
        floor.id.name,
        selector_of(floor).match_labels,
        protects(floor),
        room.id.name,
        selector_of(room).match_labels,
        protects(room),
    );

    // --- THE COVERED SIDE: THE BLOCKING BUDGET AND THE PODS IT BLOCKS ---
    //
    // **The names come out of the capture, never out of this file.** A ReplicaSet mints a
    // five-character suffix on every trip, so a literal here would assert the trip and not the
    // join — the same reason [`owned_pod_name`] exists.
    let mut expected: Vec<String> = items::<Pod>("healthy-deploy-pods")
        .into_iter()
        .map(|p| PodSnapshot::from(p).id.name)
        .collect();
    expected.sort();
    assert_eq!(
        expected.len(),
        2,
        "`healthy-deploy` runs two replicas, and the budget's floor of 2 is only a floor because \
         there are exactly that many: {expected:?}"
    );
    assert_eq!(
        protects(floor),
        expected,
        "the blocking budget selects the two pods the capture holds for it, and no others — this \
         is the positive half Drain safety had never had (NOTES § D129)"
    );

    // **The controller's own count and this join's count are the same number, and that is the
    // assertion.** `status.currentHealthy` is what the disruption controller found by running
    // this same selector server-side; a matcher that read the wrong key, or the wrong namespace,
    // or matched everything, disagrees with it. It is a comparison against the requirement — the
    // join must find what the API server found — and not against what this code returns.
    let protected: Vec<&PodSnapshot> = pods.iter().filter(|p| matches(floor, p)).collect();
    assert!(
        protected
            .iter()
            .all(|p| p.ready.as_ref().is_some_and(|c| c.status == "True")),
        "every pod the budget selects is Ready in this capture, which is the only reason the \
         equality below holds: the controller counts the *healthy* ones and this join counts the \
         matching ones. A capture with an unready replica makes those two different numbers, and \
         it is the capture that would have to say so first: {:?}",
        protected
            .iter()
            .map(|p| (&p.id.name, &p.ready))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        i32::try_from(protected.len()).ok(),
        floor.current_healthy,
        "the disruption controller ran this selector server-side and counted {:?}; a join that \
         disagrees with it is the join that is wrong, because one of them is the API server",
        floor.current_healthy
    );
    assert_eq!(
        (floor.desired_healthy, floor.disruptions_allowed),
        (Some(2), Some(0)),
        "and the floor is already the count, which is what makes a drain of either of these \
         nodes never finish"
    );

    // **Two pods, two nodes** — the property this fixture has to keep for the report to be about
    // more than one machine. Both replicas on one node still blocks that node's drain, so this is
    // not what makes the budget blocking; it is what makes *two* nodes blocked, and a trip that
    // co-locates them narrows Drain safety's only positive case to a single node without
    // changing a line of code.
    let mut nodes: Vec<&str> = protected
        .iter()
        .map(|p| {
            p.node
                .as_deref()
                .expect("a Running pod has been given a node")
        })
        .collect();
    nodes.sort();
    nodes.dedup();
    assert_eq!(
        nodes.len(),
        2,
        "the two protected pods sit on two different nodes, so draining either one is blocked \
         by this budget: {nodes:?}"
    );

    // **The namespace clause, which the corpus on its own cannot exercise.** No `kube-system`
    // pod carries `app=healthy-deploy`, so deleting the namespace comparison from `matches`
    // leaves every assertion above green — the clause is proved instead against a decoded copy
    // of a `kube-system` pod wearing the label (NOTES § D40). A budget protects its own
    // namespace and nowhere else, and a matcher that forgot it counts a DaemonSet pod towards a
    // `default` budget's floor and reports a drain as safe.
    let mut elsewhere = PodSnapshot::from(items::<Pod>("kube-system-pods").remove(0));
    assert_eq!(
        elsewhere.id.namespace.as_deref(),
        Some("kube-system"),
        "`kube-system-pods.json` is the other namespace the trip photographs, and this plant is \
         the whole reason it is not enough for the pod to carry the label"
    );
    elsewhere
        .labels
        .insert("app".to_string(), "healthy-deploy".to_string());
    assert!(
        selects(floor.selector.as_ref(), &elsewhere.labels),
        "the labels alone do match, which is what leaves the namespace as the only thing \
         standing between this pod and a `default` budget's floor: {:?}",
        elsewhere.labels
    );
    assert!(
        !matches(floor, &elsewhere),
        "and the join still refuses it, because a PodDisruptionBudget in `default` protects \
         nothing in `{}`",
        elsewhere.id.namespace.as_deref().unwrap_or("(none)")
    );

    // --- THE UNCOVERED SIDE: THE BUDGET WITH ROOM, AND NO PODS TO PROVE IT WITH ---
    //
    // This is the half the old tripwire held for both, and it is held the same way: it goes red
    // the moment a trip captures `broken-rollout`'s pods, and the join above is then written for
    // this budget too.
    assert!(
        protects(room).is_empty(),
        "a captured pod now matches `healthy-pdb-room` — write its half of the join here, so the \
         *drain finishes* case has real pods behind it too (NOTES § D129, § D131). Got {:?}",
        protects(room)
    );

    // **And the emptiness is a fact about the corpus, not about the matcher.** An empty result
    // and a matcher that never matches print the same green line, so the one committed pod whose
    // labels are not `healthy-deploy`'s is relabelled on a decoded copy (NOTES § D40) and the
    // same selector finds it.
    let mut captured = items::<Pod>("owned-pods");
    assert_eq!(
        captured.len(),
        1,
        "`broken-owned` runs one replica, and the plant below names *the* pod"
    );
    let mut stand_in = PodSnapshot::from(captured.remove(0));
    assert!(
        !selects(room.selector.as_ref(), &stand_in.labels),
        "the pod starts outside the selector, or the plant below proves nothing: {:?} against \
         {:?}",
        selector_of(room).match_labels,
        stand_in.labels
    );
    let wanted = selector_of(room)
        .match_labels
        .get("app")
        .expect("the budget with room selects on `app`, like every committed selector")
        .clone();
    assert_eq!(
        wanted, "broken-rollout",
        "and it is the workload `scripts/broken.yaml` deploys but the trip does not photograph \
         — that is the whole of the gap above"
    );
    stand_in.labels.insert("app".to_string(), wanted);
    assert!(
        selects(room.selector.as_ref(), &stand_in.labels),
        "`healthy-pdb-room`'s selector matches a pod carrying its label, so the empty join above \
         is a corpus with no such pod and not a selector that arrived broken: {:?} against {:?}",
        selector_of(room).match_labels,
        stand_in.labels
    );
    assert_ne!(
        floor.selector, room.selector,
        "the two budgets are told apart by their selectors — a decode that dropped both would \
         give them `Selector::default`, which matches nothing, and the empty half above would be \
         free"
    );
}

/// **The two committed PersistentVolumeClaims, and both halves of the join Waste's unused-disk
/// row is** (NOTES § D129, § D130). `broken-unused-disk` is `Bound` and mounted by nothing;
/// `healthy-disk` is `Bound` and `healthy-disk.json` is the pod that mounts it.
///
/// **The orphan is a static `hostPath` PV with `storageClassName: ""`**, because kind's default
/// class is `WaitForFirstConsumer`: a claim nothing mounts never gets a consumer, so the
/// provisioner is never asked and it sits `Pending` — the wrong state under the right name. The
/// row is about a **`Bound`** claim, so the phase is what keeps the report from billing the
/// reader for storage that was never provisioned.
///
/// **The capacity is `status.capacity.storage` and never the spec request** (NOTES § D130), and
/// the orphan is the object that proves it: it asked for `64Mi` and the static PV gave it
/// `128Mi`, so the two fields disagree on a committed capture and a decode reading the neighbour
/// is red rather than indistinguishable (NOTES § D29).
#[test]
fn the_bound_claim_nothing_mounts_and_the_pod_that_mounts_the_other_one() {
    let raw = fixture("persistentvolumeclaims");
    let claims = persistent_volume_claims();
    let by = |name: &str| {
        claims
            .iter()
            .find(|c| c.id.name == name)
            .unwrap_or_else(|| panic!("no {name} among {claims:?}"))
    };

    let orphan = by("broken-unused-disk");
    assert_eq!(
        orphan.id.kind,
        ObjectKind::Other("PersistentVolumeClaim".to_string()),
        "core group, so the kind is unqualified — the same branch the captured Services take"
    );
    assert_eq!(orphan.id.namespace.as_deref(), Some("default"));
    assert_eq!(
        orphan.phase.as_deref(),
        Some("Bound"),
        "a `Pending` claim has reserved no disk yet and is somebody else's problem — this one \
         has, which is what makes nobody mounting it a waste rather than a queue"
    );

    let asked_for = captured_str(
        captured_item(&raw, "broken-unused-disk"),
        &["spec", "resources", "requests", "storage"],
    );
    let provisioned = captured_str(
        captured_item(&raw, "broken-unused-disk"),
        &["status", "capacity", "storage"],
    );
    assert_ne!(
        asked_for, provisioned,
        "this capture's two storage numbers disagree, and that is the property the assertion \
         below rests on — a claim whose request and capacity are equal cannot say which field \
         was read (NOTES § D29, § D130)"
    );
    assert_eq!(
        orphan.capacity.as_deref(),
        Some(provisioned),
        "what was actually provisioned, which is the number the reader is billed for"
    );
    assert_eq!(
        orphan.capacity.as_deref(),
        Some("128Mi"),
        "and it is the static PV's size, not the `64Mi` the claim asked for"
    );

    // **The other half: a claim a pod does mount, so *nothing mounts it* is a fact about the
    // first one and not the shape of the join.**
    let used = by("healthy-disk");
    assert_eq!(used.phase.as_deref(), Some("Bound"));
    assert_eq!(used.capacity.as_deref(), Some("64Mi"));

    let mounter = pod("healthy-disk");
    assert_eq!(
        mounter.claims,
        vec!["healthy-disk".to_string()],
        "`spec.volumes[].persistentVolumeClaim.claimName`, and only that volume — the projected \
         service-account token beside it is not a claim and must not be counted as one"
    );

    // The join, both directions, over every pod the repository has captured. A claim is unused
    // when no pod in the same namespace names it.
    let pods = every_captured_pod();
    let mounted_somewhere = |claim: &ClaimSnapshot| {
        pods.iter()
            .any(|p| p.id.namespace == claim.id.namespace && p.claims.contains(&claim.id.name))
    };
    let unused: Vec<&str> = claims
        .iter()
        .filter(|c| !mounted_somewhere(c))
        .map(|c| c.id.name.as_str())
        .collect();
    println!(
        "claims {:?} · mounted by {:?} · unused {unused:?}",
        claims
            .iter()
            .map(|c| format!("{} {:?} {:?}", c.id.name, c.phase, c.capacity))
            .collect::<Vec<String>>(),
        pods.iter()
            .filter(|p| !p.claims.is_empty())
            .map(|p| format!("{}:{:?}", p.id.name, p.claims))
            .collect::<Vec<String>>()
    );
    assert_eq!(
        unused,
        ["broken-unused-disk"],
        "exactly one claim nothing mounts — and the other one being mounted is what makes this \
         a join rather than a list of every claim in the cluster"
    );
    assert!(
        mounted_somewhere(used),
        "the claim `healthy-disk.json` mounts is found from the claim's side too, or the \
         direction above is the only one that works"
    );
}

/// **A generic ephemeral volume is a claim the pod mounts, under the name the API server gives
/// it** — `<pod name>-<volume name>`, off `kubectl explain
/// pod.spec.volumes.ephemeral.volumeClaimTemplate` (NOTES § D131,
/// `reports/2026-08-21-family-c-analysis-report-family-review.md` § 4).
///
/// **The plant is the only way to reach the shape**: no pod on the fixture cluster declares one,
/// so `just fixtures` has never captured it (NOTES § D40). A trip that runs a workload with an
/// `ephemeral:` volume replaces it.
///
/// **Three framings of one field, all fed** (NOTES § D29): the claim a pod names outright, the
/// claim it never names because the API server derives it, and the projected token volume that is
/// neither and must not be counted as one.
#[test]
fn a_generic_ephemeral_volume_is_a_claim_under_the_name_the_api_server_derives() {
    let plain = pod("healthy-disk");
    assert_eq!(
        plain.claims,
        vec!["healthy-disk".to_string()],
        "the captured pod names one claim and mounts a projected token beside it"
    );
    assert!(
        !plain.local_storage_disk && !plain.local_storage_memory,
        "a claim is not local storage — a drain does not throw it away, and the fields must not \
         be reading one volume list the same way"
    );

    // **The pod is renamed on the way in, and that is the whole of this plant** (NOTES § D40).
    // `healthy-disk.json` is a pod named `healthy-disk` mounting a claim named `healthy-disk`, so
    // the pod's name, the claim it already names and the prefix the API server derives are one
    // string — an assertion over that pod cannot tell a derivation off `metadata.name` from one
    // off `claimName`, which is the review's own finding
    // (`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 6).
    let with_ephemeral = capture_but("healthy-disk", |pod| {
        pod.metadata.name = Some("mounts-a-disk".to_string());
        pod.spec
            .as_mut()
            .expect("the capture has a spec")
            .volumes
            .get_or_insert_with(Vec::new)
            .push(Volume {
                name: "scratch".to_string(),
                ephemeral: Some(EphemeralVolumeSource {
                    volume_claim_template: Some(PersistentVolumeClaimTemplate::default()),
                }),
                ..Volume::default()
            });
    });
    println!(
        "{} claims: {:?}",
        with_ephemeral.id.name, with_ephemeral.claims
    );
    assert_eq!(
        with_ephemeral.claims,
        vec![
            "healthy-disk".to_string(),
            "mounts-a-disk-scratch".to_string()
        ],
        "the derived name is the **pod's** own name and the volume's, joined by a hyphen — not \
         the claim the pod already mounts, which on the unrenamed capture is the same string. \
         The claim is `Bound`, mounted by a running pod, and named by no `claimName` anywhere, \
         so a Waste report reading only `claimName` calls a disk in use nobody's"
    );
    assert!(
        !with_ephemeral.local_storage_disk && !with_ephemeral.local_storage_memory,
        "and it is deliberately not local storage: the claim outlives the pod's container \
         filesystem, and `kubectl drain` does not warn about one"
    );
}

/// **`local_storage_disk` is the `emptyDir` a bare `kubectl drain` refuses on**, and the two pods
/// it named are captured: `default/broken-gang` and `default/broken-restarts`
/// (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 1).
///
/// **Every volume shape the field can meet, fed** (NOTES § D29): an `emptyDir`, a
/// `persistentVolumeClaim`, a generic `ephemeral` (above), and a pod with neither.
#[test]
fn local_storage_is_the_empty_dir_a_bare_kubectl_drain_refuses_on() {
    for name in ["gang", "restarts"] {
        let pod = pod(name);
        println!(
            "{}: disk={} memory={}",
            pod.id.name, pod.local_storage_disk, pod.local_storage_memory
        );
        assert!(
            pod.local_storage_disk,
            "`kubectl drain k8rs-worker` refused on {} for local storage, so the field a \
             report asks that question of has to say so",
            pod.id.name
        );
        assert!(
            !pod.local_storage_memory,
            "and the corpus carries no tmpfs — the capture's own `emptyDir`s name no medium \
             (`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 2)"
        );
    }
    assert!(
        !pod("healthy").local_storage_disk && !pod("healthy").local_storage_memory,
        "a pod with no volume at all keeps files on nothing"
    );

    // The plant is the presence of the entry and nothing inside it: an `emptyDir: {}` is what a
    // manifest usually writes, and a field read off `sizeLimit` would miss it.
    let planted = capture_but("healthy", |pod| {
        pod.spec
            .as_mut()
            .expect("the capture has a spec")
            .volumes
            .get_or_insert_with(Vec::new)
            .push(Volume {
                name: "scratch".to_string(),
                empty_dir: Some(EmptyDirVolumeSource::default()),
                ..Volume::default()
            });
    });
    assert!(
        planted.local_storage_disk && !planted.local_storage_memory,
        "`emptyDir: {{}}` is the ordinary spelling and is the whole of the disk trigger"
    );
    assert!(
        planted.claims.is_empty(),
        "and an emptyDir reserves no disk, so it is not a claim either"
    );
}

/// **`medium` splits one volume kind into two facts that do not point the same way** — the drain
/// still refuses, and nothing is lost (NOTES § D134, `screens/analysis.md` § *One volume kind, two
/// mediums*). Istio's injector adds a `Memory` one to every meshed pod, so the undifferentiated
/// field would have put *copy your files off first* on every node of every meshed cluster.
///
/// **The plant is the only way to reach the shape**: no pod on the fixture cluster and nothing in
/// the corpus names a medium at all (NOTES § D40,
/// `reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 2). A trip that runs a
/// pod with `emptyDir: {medium: Memory}` replaces it.
///
/// **All four spellings the field can meet, fed** (NOTES § D29): absent, the explicit empty
/// string the API server writes for the default, `Memory`, and one pod naming both kinds.
#[test]
fn an_empty_dir_backed_by_memory_is_a_second_fact_and_not_the_first_one_again() {
    let with_mediums = |mediums: &[Option<&str>]| {
        let mediums: Vec<Option<String>> = mediums
            .iter()
            .map(|m| m.map(std::string::ToString::to_string))
            .collect();
        capture_but("healthy", |pod| {
            let volumes = pod
                .spec
                .as_mut()
                .expect("the capture has a spec")
                .volumes
                .get_or_insert_with(Vec::new);
            for (n, medium) in mediums.into_iter().enumerate() {
                volumes.push(Volume {
                    name: format!("scratch{n}"),
                    empty_dir: Some(EmptyDirVolumeSource {
                        medium,
                        ..EmptyDirVolumeSource::default()
                    }),
                    ..Volume::default()
                });
            }
        })
    };
    let read = |pod: &PodSnapshot| (pod.local_storage_disk, pod.local_storage_memory);

    let absent = with_mediums(&[None]);
    println!("absent: {:?}", read(&absent));
    assert_eq!(
        read(&absent),
        (true, false),
        "unset is the default medium, which is the node's own disk"
    );

    // **The explicit empty string is the same volume**, and the shape the API server writes back:
    // a check that compared against `None` alone would call this a tmpfs.
    let explicit = with_mediums(&[Some("")]);
    println!("explicit empty: {:?}", read(&explicit));
    assert_eq!(read(&explicit), (true, false));

    let memory = with_mediums(&[Some("Memory")]);
    println!("Memory: {:?}", read(&memory));
    assert_eq!(
        read(&memory),
        (false, true),
        "a tmpfs is not the machine's disk — there is nothing on it to copy off, and a row that \
         says otherwise is wrong about every meshed pod on the cluster"
    );

    // **A pod can name both, and it counts once in each** — the same deliberate
    // non-deduplication the orphan and local-storage counts already practise on each other.
    let both = with_mediums(&[None, Some("Memory")]);
    println!("both: {:?}", read(&both));
    assert_eq!(read(&both), (true, true));

    // **A value neither of the two legal ones is read as neither**, which is the direction that
    // invents nothing: `medium` is `""` or `Memory` and nothing else
    // (`kubectl explain pod.spec.volumes.emptyDir.medium`).
    let unknown = with_mediums(&[Some("HugePages-2Mi")]);
    println!("unknown: {:?}", read(&unknown));
    assert_eq!(read(&unknown), (false, false));
}

/// **The Service that reaches nothing, which is Waste's headline row** — a slice with zero
/// endpoints, and the `kubernetes.io/service-name` label that ties it to the Service it is about
/// (NOTES § D129). `EndpointSliceSnapshot::from` had never run on a real object before the
/// 2026-08-20 trip.
///
/// **`endpoints` counts every endpoint, ready or not** (NOTES § D130), and `broken-sts` is the
/// object that proves it: two endpoints, one of them `ready: false`. A pod failing its readiness
/// probe is Alerts' rule 7, already on the other screen, and counting it as *nothing* here would
/// put one pod on two screens saying two different things.
#[test]
fn the_service_that_reaches_nothing_is_a_slice_with_no_endpoints() {
    let raw = fixture("endpointslices");
    let slices = endpoint_slices();
    let by = |service: &str| {
        slices
            .iter()
            .find(|s| s.service.as_deref() == Some(service))
            .unwrap_or_else(|| panic!("no slice for {service} among {slices:?}"))
    };

    let empty = by("broken-noendpoints");
    assert_eq!(
        empty.id.kind,
        ObjectKind::Other("EndpointSlice.discovery.k8s.io".to_string()),
        "a kind in a real API group is qualified by it (NOTES § D36)"
    );
    assert_eq!(empty.id.namespace.as_deref(), Some("default"));
    assert_eq!(
        empty.endpoints, 0,
        "nothing is behind this Service, which is the 503 nobody can explain"
    );
    assert!(
        empty.id.name.starts_with("broken-noendpoints-"),
        "the controller mints the slice's name with a suffix of its own, so the Service is found \
         through the label and never through the name: {}",
        empty.id.name
    );
    // And the Service it names is a captured object, or the row points at nothing.
    assert!(
        items::<Service>("services")
            .into_iter()
            .map(ServiceSnapshot::from)
            .any(|s| s.id.name == "broken-noendpoints" && s.id.namespace == empty.id.namespace),
        "the label ties the slice to a Service the capture also holds — the pair is the row"
    );

    // **Ready or not** (NOTES § D130). The negative that makes the count mean something, and the
    // one endpoint that is `ready: false` is why it is this slice and not `kube-dns`'s.
    let statefulset = by("broken-sts");
    let conditions: Vec<Option<bool>> =
        at(captured_item(&raw, &statefulset.id.name), &["endpoints"])
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| e["conditions"]["ready"].as_bool())
            .collect();
    let ready = conditions.iter().filter(|r| **r == Some(true)).count();
    let not_ready = conditions.iter().filter(|r| **r == Some(false)).count();
    assert_eq!(
        (ready, not_ready),
        (1, 1),
        "this capture holds one endpoint that is ready and one that is not — a slice whose \
         endpoints all agree cannot tell *every endpoint* from *the ready ones* (NOTES § D29). \
         Counted rather than compared in order: the controller writes the array in whatever \
         order it walked the pods, and asserting that is asserting the trip. Got {conditions:?}"
    );
    assert_eq!(
        statefulset.endpoints, 2,
        "both are counted: a pod failing its readiness probe is Alerts' rule 7 and is not \
         *nothing behind the Service*"
    );

    // The other two, so the sweep found more than the one object it is about.
    assert_eq!(by("kubernetes").endpoints, 1);
    assert_eq!(by("kube-dns").id.namespace.as_deref(), Some("kube-system"));
    assert_eq!(slices.len(), 4, "the capture holds four");

    // **A slice with no `kubernetes.io/service-name` says nothing about any Service.**
    // Hand-managed slices exist and carry no such label; no capture holds one, so the label is
    // removed from a decoded copy (NOTES § D40).
    let mut hand_managed: EndpointSlice =
        serde_json::from_value(captured_item(&raw, &statefulset.id.name).clone())
            .expect("the same capture");
    hand_managed
        .metadata
        .labels
        .as_mut()
        .expect("the controller labels its own slices")
        .remove("kubernetes.io/service-name");
    let orphan = EndpointSliceSnapshot::from(hand_managed);
    assert_eq!(
        orphan.service, None,
        "no Service is named, and inventing one from the object's name would file a row under a \
         Service that may not exist"
    );
    assert_eq!(
        orphan.endpoints, statefulset.endpoints,
        "and the count is unaffected by the label, or one of the two fields is being read from \
         the other"
    );

    println!(
        "slices: {}",
        slices
            .iter()
            .map(|s| format!(
                "{} -> {:?} ({} endpoints)",
                s.id.name, s.service, s.endpoints
            ))
            .collect::<Vec<String>>()
            .join(" · ")
    );
}

/// **The one captured pod the RuntimeClass admission controller charged more for than its
/// container asked for** (NOTES § D124, § D130). `spec.overhead` is what the *scheduler* counts
/// on top of the container requests, and no capture carried one until 2026-08-20 because kind
/// runs runc with no RuntimeClass to charge — which is why D124's first condition, *a defect
/// proven on a committed capture*, could not be met and the Capacity arithmetic stayed frozen.
///
/// **The controller writes it, not the manifest.** A create request carrying `spec.overhead` is
/// rejected outright, so this field can only come from a RuntimeClass with `overhead.podFixed`
/// — which is what makes it a genuine capture rather than a manifest the fixture author typed.
///
/// **Nothing computes with it yet** and this test does not make it: [`charged`] is frozen and
/// reads `cpu_request`, and a report adding overhead on top of its answer is the report and N5
/// disagreeing about one node, which is NOTES § D46's named defect. What is asserted is that the
/// number is on disk and decodes, so the arithmetic box has the evidence it was blocked on.
#[test]
fn the_runtime_class_charged_this_pod_for_more_than_its_container_asked_for() {
    let raw = fixture("overhead");
    let charged = pod("overhead");

    assert_eq!(
        charged.overhead_cpu.as_deref(),
        Some(captured_str(&raw, &["spec", "overhead", "cpu"])),
        "read off `spec.overhead`, and the capture agrees"
    );
    assert_eq!(
        charged.overhead_memory.as_deref(),
        Some(captured_str(&raw, &["spec", "overhead", "memory"])),
    );
    assert_eq!(
        (
            charged.overhead_cpu.as_deref(),
            charged.overhead_memory.as_deref()
        ),
        (Some("250m"), Some("120Mi")),
        "the RuntimeClass's `overhead.podFixed`, which the admission plugin copied onto the pod"
    );
    assert_eq!(
        captured_str(&raw, &["spec", "runtimeClassName"]),
        "broken-overhead",
        "and it came from a RuntimeClass, which is the only writer there is — a manifest \
         carrying `spec.overhead` is rejected at create"
    );

    // **The container's own request, which is the number every frozen sum already counts.** The
    // two differ in both dimensions, so a decode reading `resources.requests` for either
    // overhead key is red rather than indistinguishable (NOTES § D29), and the gap between them
    // is what a Capacity row that reads only the container would leave off the node.
    let app = container(&charged, "app");
    assert_eq!(
        (app.cpu_request.as_deref(), app.memory_request.as_deref()),
        (Some("100m"), Some("64Mi")),
        "what the container asked for, beside a pod-level charge two and a half times larger"
    );
    assert_ne!(app.cpu_request, charged.overhead_cpu);
    assert_ne!(app.memory_request, charged.overhead_memory);

    // **And it is the only one**, so the field is read off the path it names rather than filled
    // in from a neighbour every pod has. A derived list asserts it found something: the sweep is
    // the same one that used to hold this item's tripwire.
    let pods = every_captured_pod();
    let with_overhead: Vec<&str> = pods
        .iter()
        .filter(|p| p.overhead_cpu.is_some() || p.overhead_memory.is_some())
        .map(|p| p.id.name.as_str())
        .collect();
    assert!(pods.len() > 40, "walked {} pods", pods.len());
    assert_eq!(
        with_overhead,
        ["broken-overhead"],
        "one pod in the corpus carries a RuntimeClass overhead and the rest carry none"
    );
    println!(
        "{}: overhead {:?}/{:?} on top of container {:?}/{:?}",
        charged.id.name,
        charged.overhead_cpu,
        charged.overhead_memory,
        app.cpu_request,
        app.memory_request
    );
}

/// **What Family C's inputs still have no object for** — the successor to
/// `what_no_committed_capture_can_prove_about_family_cs_inputs`, which held five items until the
/// 2026-08-20 trip landed all five (NOTES § D129, § D130). One is left, and it is held the same
/// way: this test **goes red the moment the fetch lands**, so the gap cannot close in silence.
///
/// `certificate_requests` is `None` because C3's fetch is a Phase 5 box — the one committed CSR
/// is read by [`a_pending_certificate_request_decodes_as_pending_and_carries_no_credential`]
/// rather than smuggled into the snapshot every rule runs over, so Phase 4 draws one
/// `Row::NotComputed` for it.
///
/// **The other half is the distinction the `Option` exists for**: an empty answer is
/// `Some(vec![])` and *nobody looked* is `None`, and the five lists that are fetched now all
/// answer with objects. A list that quietly emptied would satisfy every *is it `Some`* assertion
/// in this file, so each is asserted non-empty and named.
#[test]
fn what_family_cs_inputs_still_have_no_object_for() {
    let snapshot = fixture_snapshot();

    let filled: Vec<(&str, usize)> = vec![
        (
            "replica_sets",
            snapshot.replica_sets.as_ref().map_or(0, Vec::len),
        ),
        ("services", snapshot.services.as_ref().map_or(0, Vec::len)),
        (
            "endpoint_slices",
            snapshot.endpoint_slices.as_ref().map_or(0, Vec::len),
        ),
        ("claims", snapshot.claims.as_ref().map_or(0, Vec::len)),
        (
            "disruption_budgets",
            snapshot.disruption_budgets.as_ref().map_or(0, Vec::len),
        ),
    ];
    println!("fetched: {filled:?} · certificate_requests: not fetched");
    for (name, held) in &filled {
        assert!(
            *held > 0,
            "{name} is fetched and the capture fills it — a list that decoded to nothing would \
             leave every reader of it green and every report of it blank"
        );
    }
    assert_eq!(
        filled.iter().map(|(_, n)| n).sum::<usize>(),
        1 + 4 + 4 + 2 + 2,
        "and the counts are the capture's: one ReplicaSet, four Services, four EndpointSlices, \
         two claims, two budgets — a number that moves when a trip adds an object, which is the \
         only thing that brings a reader back to this file"
    );

    assert_eq!(
        snapshot.certificate_requests, None,
        "C3's fetch is a Phase 5 box, so Phase 4 draws one `Row::NotComputed` for it — when it \
         lands, replace this with the assertions the CSR list deserves (NOTES § D129)"
    );
}

/// **The two ways a PodDisruptionBudget can say nothing about *which pods*, and they are not one
/// answer** — the decode half of the change Drain safety's report proved through a pane
/// ([`DisruptionBudgetSnapshot::selector`], NOTES § D46).
///
/// Upstream states both on the field itself: *"A null selector will match no pods, while an
/// empty ({}) selector will select all pods within the namespace"* — read off
/// `k8s-openapi`'s generated docs for `policy/v1 PodDisruptionBudgetSpec`, in this tree. Until
/// 2026-08-21 an `unwrap_or_default()` folded the first onto the second, so a budget written
/// `{}` decoded to the value the matcher reads as *nothing*.
///
/// **Both shapes are plants and neither is an edit to a capture** (NOTES § D40, § D53): every
/// PDB a cluster ever writes down carries the selector its author typed, so a capture cannot
/// hold either one.
#[test]
fn the_two_ways_a_budget_can_say_nothing_about_which_pods_and_they_are_not_one_value() {
    let plant = |edit: fn(&mut PodDisruptionBudget)| -> DisruptionBudgetSnapshot {
        let mut object = items::<PodDisruptionBudget>("poddisruptionbudgets")
            .into_iter()
            .find(|b| b.metadata.name.as_deref() == Some("broken-pdb-floor"))
            .expect("poddisruptionbudgets.json has no broken-pdb-floor");
        edit(&mut object);
        DisruptionBudgetSnapshot::from(object)
    };

    assert_eq!(
        plant(|b| b.spec.get_or_insert_with(Default::default).selector = None).selector,
        None,
        "absent is `None` — *selects no pods*, and never a shape a matcher has to read"
    );
    assert_eq!(
        plant(|b| {
            b.spec.get_or_insert_with(Default::default).selector = Some(LabelSelector::default());
        })
        .selector,
        Some(Selector::default()),
        "present and empty is `Some` — upstream's `labels.Everything()`, every pod in the \
         namespace, and a decode folding it onto the line above calls a drain safe that hangs"
    );

    // **And the spec itself missing is the absent one too**, which is the third shape the
    // pipeline can hand this decode (NOTES § D29): a PDB whose whole spec the prune dropped
    // says nothing about which pods, not everything about them.
    assert_eq!(
        plant(|b| b.spec = None).selector,
        None,
        "no spec at all is not a selector that selects the namespace"
    );

    // The negative that keeps all three honest: the committed object writes a real selector, so
    // a decode returning `None` for everything would pass the first assertion and nothing else.
    assert_eq!(
        plant(|_| ()).selector,
        Some(Selector {
            match_labels: BTreeMap::from([("app".to_string(), "healthy-deploy".to_string())]),
            match_expressions: Vec::new(),
        }),
        "and the capture's own selector survives untouched"
    );
}

/// The selector a captured budget carries. **Both committed budgets write one**, so `None` here
/// is a decode that dropped the field rather than a `policy/v1` *selects no pods* — and the two
/// stopped being one value on 2026-08-21
/// ([`DisruptionBudgetSnapshot::selector`]), which is why this unwraps loudly instead of
/// falling back to a default that would make every assertion below pass on nothing.
fn selector_of(budget: &DisruptionBudgetSnapshot) -> &Selector {
    budget
        .selector
        .as_ref()
        .unwrap_or_else(|| panic!("{} was captured with a selector", budget.id.name))
}

/// The three on-demand lists the 2026-08-20 trip filled, decoded once each. Local to this module
/// because nothing outside it reads them yet — the reports that will are a later phase, and a
/// helper hoisted to `rules_tests.rs` before it has a second caller is the divergence the split
/// exists to avoid (NOTES § D91).
fn disruption_budgets() -> Vec<DisruptionBudgetSnapshot> {
    items::<PodDisruptionBudget>("poddisruptionbudgets")
        .into_iter()
        .map(Into::into)
        .collect()
}

fn persistent_volume_claims() -> Vec<ClaimSnapshot> {
    items::<PersistentVolumeClaim>("persistentvolumeclaims")
        .into_iter()
        .map(Into::into)
        .collect()
}

fn endpoint_slices() -> Vec<EndpointSliceSnapshot> {
    items::<EndpointSlice>("endpointslices")
        .into_iter()
        .map(Into::into)
        .collect()
}
