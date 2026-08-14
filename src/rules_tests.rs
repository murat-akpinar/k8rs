use super::*;
// `ContainerStateRunning` is imported here and not beside the decode's own types: no
// product code in this file constructs one, and the top-level list is what `rules.rs`
// reads off the API.
use k8s_openapi::api::core::v1::{
    ContainerStateRunning, ContainerStateWaiting, HostPathVolumeSource, Taint as ApiTaint,
    Toleration as ApiToleration, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use std::collections::{BTreeSet, HashSet};

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

fn deployment(uid: &str) -> ObjectId {
    ObjectId {
        kind: ObjectKind::Deployment,
        namespace: Some("payments".to_string()),
        name: "web".to_string(),
        uid: Some(uid.to_string()),
    }
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
    // string below rather than looser: the pin is the midnight after the capture day
    // (D57), so the cordon is as old as the gap between `break-nodes` and midnight — the
    // 2026-08-13 trip ran at 23:35 UTC, which puts it on the minutes rung. A band of
    // `[1m, 24h)` would let a recapture past this line and fail on the phrase instead,
    // with a message about cards saying when — which is the confusion the check exists to
    // prevent, not to cause.
    let stamped = cordon.added_at.clone().expect(
        "the controller stamps timeAdded on the taint it mirrors from spec.unschedulable \
         — a capture without it is D64's premise back again",
    );
    let elapsed = now().0.duration_since(stamped.0);
    assert_eq!(
        elapsed.as_mins(),
        24,
        "the cordon is {elapsed:?} before the pinned now, and the phrase below says 24 \
         minutes — if `just fixtures` was re-run, repin `fn now()` (see the note there for \
         what moves with it) and move both together"
    );
    assert_eq!(
        dated.age(&now()).as_deref(),
        Some("24 min ago"),
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
    // which `certs-test.sh` reports as 363 days out. `Finding::age` flattens it to the
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

// --- THE DECODE, AGAINST THE COMMITTED CAPTURES ---
//
// Every fixture below came off the kind cluster `scripts/cluster.sh` builds, was
// verified to have reached the state its rule is about, and was sanitized on the way
// out (`just fixtures`). What is asserted is the value a later rule fires on — not
// that the JSON parsed, which is `serde`'s claim and not ours.

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
}

/// C1's input is a PEM file, not JSON — `scripts/make-certs.sh` writes the three with
/// pinned dates, so a certificate fixture cannot expire out from under a test.
fn certificate(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/certs/{name}.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path} could not be read: {e}"))
}

fn pod(name: &str) -> PodSnapshot {
    let pod: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    PodSnapshot::from(pod)
}

/// `kubectl get -A` answers with `kind: List`, which `k8s_openapi::List<T>` refuses
/// — it wants `NodeList` — so the items come out by hand here. JSON plumbing in a
/// test, not a second decode of the snapshot types.
fn items<T: k8s_openapi::serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
    fixture(name)["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}.json has no items array"))
        .iter()
        .map(|v| {
            serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
        })
        .collect()
}

fn time(s: &str) -> Time {
    Time(
        s.parse()
            .unwrap_or_else(|e| panic!("{s} is not a time: {e}")),
    )
}

// --- WHAT THE CAPTURE ITSELF SAYS START ---
//
// **An expectation read out of the capture, for the fields whose value belongs to the
// cluster rather than to the requirement.** A uid, a timestamp, a restart counter, a
// node name and a scheduler's sentence are all new on every `just fixtures`: a literal
// transcribed from one capture asserts the cluster that produced it, and the next trip
// reddens a test whose requirement never moved (which is exactly what happened here —
// the four-node capture of 2026-08-12 turned seventeen of these red at once).
//
// **It is not a tautology, because the decode is not what it is read from.** These are
// decode assertions and what they exist to catch is a field dropped on the way
// through, filled from its neighbour, or rewritten — comparing the snapshot against
// the *JSON the decode was handed* catches all three, and names the path it must have
// come from while doing it (`initContainerStatuses` is not `containerStatuses`, and
// `startedAt` is not `finishedAt`).
//
// **What it cannot check is that the capture still has the shape the test exists
// for** — a fixture whose restart count fell to zero would satisfy every derived
// assertion above and prove nothing about rule 5. So each use is paired with the
// property the fixture has to keep: a counter inside its rule's band, two moments that
// differ, a message that still names the object it is evidence about.
//
// Absence panics rather than comparing `None` against `None`: a path that stops
// matching the capture is the one failure this technique could otherwise hide.

fn at<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
    let mut at = value;
    for key in path {
        at = &at[key];
    }
    at
}

fn captured_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a str {
    at(value, path).as_str().unwrap_or_else(|| {
        panic!("the capture carries no string at {path:?}, so nothing here is compared against it")
    })
}

/// `i32` because that is what the API declares a restart count and every replica
/// counter as, and what the snapshot types carry.
fn captured_i32(value: &serde_json::Value, path: &[&str]) -> i32 {
    let n = at(value, path).as_i64().unwrap_or_else(|| {
        panic!("the capture carries no number at {path:?}, so nothing here is compared against it")
    });
    i32::try_from(n)
        .unwrap_or_else(|_| panic!("{path:?} is {n}, which is not the i32 the API declares"))
}

fn captured_time(value: &serde_json::Value, path: &[&str]) -> Time {
    time(captured_str(value, path))
}

/// One container's captured status, out of the array it has to have come from.
/// **Which array is part of the assertion** — a number read back from
/// `containerStatuses` is not evidence about the init one, and reading both lists is
/// the whole of D27.
fn captured_status<'a>(
    pod: &'a serde_json::Value,
    array: &str,
    name: &str,
) -> &'a serde_json::Value {
    pod["status"][array]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("the capture has no {name} in status.{array}"))
}

/// One condition of a captured object, by type — the array [`PodSnapshot::ready`],
/// [`PodSnapshot::scheduled`] and every `NodeSnapshot` condition are picked out of by
/// name, and picking by index instead is the defect the healthy pod exists to catch.
fn captured_condition<'a>(object: &'a serde_json::Value, type_: &str) -> &'a serde_json::Value {
    object["status"]["conditions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["type"] == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition"))
}

/// One item of a `kubectl get -A` capture, by name — the List counterpart of
/// [`captured_status`].
fn captured_item<'a>(list: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["metadata"]["name"] == name)
        .unwrap_or_else(|| panic!("the capture has no {name} in its items"))
}
// --- WHAT THE CAPTURE ITSELF SAYS END ---

/// **The moment every snapshot in this file is read at.** One helper rather than a
/// literal at each construction site: the pin is a single fact about the committed
/// captures, and a copy of a fact is a copy that drifts.
///
/// **The value is not free.** `scripts/certs-test.sh` extracts this literal out of this
/// function, refuses to disagree with it, and asserts the committed certificates against
/// it on every `just check`. **Moving it moves `scripts/certs-test.sh` and
/// `scripts/make-certs.sh` in the same change** — the pin is one fact spelled in four
/// places across two ownership rows (NOTES § D57).
///
/// It also lands after every `Time` the snapshot types *expose*, which
/// `the_pinned_now_is_not_before_the_captures_it_is_read_against` asserts rather than
/// leaves to trust.
///
/// **The shape of the value is the midnight after the capture day** — near enough that a
/// fixture's age is one an operator would recognise, and round enough to be repeated in
/// three other files without transcription error.
fn now() -> Time {
    time("2026-08-14T00:00:00Z")
}

fn container<'a>(pod: &'a PodSnapshot, name: &str) -> &'a ContainerSnapshot {
    pod.containers
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("{} has no container {name}", pod.id.name))
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
                "rule 1 shows the kubelet's own sentence, got {message:?}"
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
    assert!(
        matches!(&migrate.state, ContainerState::Waiting { reason, .. }
            if reason.as_deref() == Some("CrashLoopBackOff")),
        "Init:CrashLoopBackOff, got {:?}",
        migrate.state
    );
    assert_eq!(
        migrate.last_terminated.as_ref().map(|t| t.exit_code),
        Some(1)
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
    // **The negative fixture is not a pod nothing has ever happened to.** `healthy.yaml`
    // ends on a `sleep 3600`, so a capture taken more than an hour into the cluster's
    // life photographs the shell exiting 0 and the kubelet restarting it — which is the
    // ordinary shape of a long-lived pod, and the one rule 5 and rule 6 have to stay
    // silent about. Read off the capture rather than transcribed: the count is however
    // many hours the trip ran, and the requirement is only that it is under
    // `RESTARTS_WARN` and that the run it records ended cleanly.
    let app_status = captured_status(&raw, "containerStatuses", "app");
    assert_eq!(app.restarts, captured_i32(app_status, &["restartCount"]));
    assert!(
        app.restarts < RESTARTS_WARN,
        "the negative fixture has to stay under rule 5's band, or its silence is the \
         threshold's doing and not the pod's: {} restarts",
        app.restarts
    );
    let last = app
        .last_terminated
        .as_ref()
        .expect("a container the kubelet has restarted records how the run before it ended");
    assert_eq!(
        last.exit_code, 0,
        "and the run it records ended cleanly, which is rule 6's first exemption — the \
         one `exit0.json` proves fires on a container that is *not* serving"
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
}

/// **Every pod capture in the repository**, and the claim is checked rather than assumed:
/// `just fixtures` guards each file, and `the_whole_capture_through_the_rules_at_once` names
/// which of these are allowed to draw nothing, so a fixture added to `tests/fixtures` and not
/// to this list shows up as a capture no test reads.
///
/// Named once because four things read the same set — the join, the pin guard, the whole-capture
/// run and every node-rule join through [`every_captured_pod`] — and a second copy is a second
/// list to keep in step. **The pin guard is why completeness matters**: it walks only what is in
/// this snapshot, so a capture left out of it is a capture whose timestamps were never compared
/// against [`now`].
const CAPTURED_PODS: [&str; 31] = [
    "config",
    "crashloop",
    "exit0",
    "failed",
    "healthy-hostpath",
    "healthy-podlevel",
    "healthy-retry",
    "healthy-sidecar",
    "healthy-unreadysidecar",
    "healthy",
    "hostpath",
    "image",
    "init",
    "nolimits",
    "notfound",
    "oom",
    "oomserving",
    "pending",
    "podlimit",
    "readiness",
    "resize",
    "restarts",
    "restarts10",
    "restarts10serving",
    "sigterm",
    "socket",
    "startup",
    "stuck",
    "succeeded",
    "unjudged",
    "wedged",
];

/// The snapshot a rule would be handed if it ran over the whole committed capture at
/// once: every pod, every node, the Deployments, and the pinned [`now`](now).
fn fixture_snapshot() -> ClusterSnapshot {
    ClusterSnapshot {
        now: now(),
        pods: CAPTURED_PODS.iter().map(|n| pod(n)).collect(),
        nodes: items::<Node>("nodes").into_iter().map(Into::into).collect(),
        workloads: items::<Deployment>("deployments")
            .into_iter()
            .map(Into::into)
            .collect(),
        server_version: Some(
            std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/K8S_VERSION"
            ))
            .expect("the capture stamps the version it came from")
            .trim()
            .to_string(),
        ),
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(certificate("healthy-client")),
        // `just fixtures` captures `-A`, so this snapshot covers the whole cluster
        // and N2 and N5 are allowed to run over it.
        namespace_scope: None,
    }
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
    let snapshot = fixture_snapshot();
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

    let w = WorkloadSnapshot::from(deployment);
    println!(
        "deployment: desired={:?} ready={:?} updated={:?} unavailable={:?}",
        w.desired, w.ready, w.updated, w.unavailable
    );
    assert_eq!(
        (w.desired, w.ready, w.updated, w.unavailable),
        (Some(5), Some(2), Some(4), Some(3)),
        "desired is what the spec asked for, ready is what is passing probes, updated is \
         how many are on the new template, unavailable is how many are not answering — and \
         no two of the six counters on this object are equal"
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

    let w = WorkloadSnapshot::from(replicaset);
    println!(
        "replicaset: desired={:?} ready={:?} updated={:?} unavailable={:?}",
        w.desired, w.ready, w.updated, w.unavailable
    );
    assert_eq!(
        (w.desired, w.ready, w.updated, w.unavailable),
        (Some(5), Some(2), Some(6), None),
        "a ReplicaSet's `status.replicas` is not optional and is not the desired count — it \
         is how many pods it has on its one template, which is what `updated` means here \
         (D82). `fullyLabeledReplicas` and `availableReplicas` are neither, and there is no \
         unavailable counter on this kind at all"
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
        "daemonset: desired={:?} ready={:?} updated={:?} unavailable={:?}",
        w.desired, w.ready, w.updated, w.unavailable
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

/// The snapshot [`analyze`] is handed below: pods and a moment. No nodes and no
/// workloads, because every rule in this box reads a Pod — the joins belong to the
/// N-series and W-series, which are later boxes and would only add rules that cannot
/// fire to every assertion here.
fn pods_at(pods: Vec<PodSnapshot>, now: Time) -> ClusterSnapshot {
    ClusterSnapshot {
        now,
        pods,
        nodes: Vec::new(),
        workloads: Vec::new(),
        server_version: None,
        context: None,
        client_certificate: None,
        namespace_scope: None,
    }
}

fn findings_at(names: &[&str], now: Time) -> Vec<Finding> {
    analyze(&pods_at(names.iter().map(|n| pod(n)).collect(), now))
}

fn findings(names: &[&str]) -> Vec<Finding> {
    findings_at(names, now())
}

/// One finding as `--once` would print it (`screens/once.md`) — so that
/// `cargo test -- --nocapture` shows the sentences a user reads and not a `Debug` dump
/// of the struct they came in. CLAUDE.md's "green tests are not working software" gate
/// is read off this.
fn card(f: &Finding, now: &Time) -> String {
    let mark = match f.severity {
        Severity::Critical => '●',
        Severity::Warn => '▲',
        Severity::Info => '○',
    };
    let name = match &f.owner.namespace {
        Some(ns) => format!("{ns}/{}", f.owner.name),
        None => f.owner.name.clone(),
    };
    let age = f.age(now).map_or(String::new(), |a| format!(" · {a}"));
    format!(
        "{mark} {name}{age}\n  {}\n  {}\n  → {}\n  $ {}\n",
        f.title,
        f.evidence,
        f.action,
        f.kubectl_cmd
            .as_deref()
            .unwrap_or("(no command shows this)")
    )
}

fn titles(all: &[Finding]) -> Vec<&str> {
    all.iter().map(|f| f.title.as_str()).collect()
}

fn show(all: &[Finding]) {
    for f in all {
        println!("{}", card(f, &now()));
    }
}

/// The one finding on `pod` whose title contains `phrase` — and a failure when there is
/// not exactly one. "The rule fired" and "the rule fired twice on one container" print
/// the same green line otherwise.
fn only<'a>(all: &'a [Finding], pod: &str, phrase: &str) -> &'a Finding {
    let mut hits = all
        .iter()
        .filter(|f| f.object.name == pod && f.title.contains(phrase));
    let found = hits
        .next()
        .unwrap_or_else(|| panic!("nothing on {pod} says {phrase:?} — got {:?}", titles(all)));
    assert!(
        hits.next().is_none(),
        "two findings on {pod} say {phrase:?} — got {:?}",
        titles(all)
    );
    found
}

fn nothing(all: &[Finding], why: &str) {
    assert!(all.is_empty(), "{why} — got {:?}", titles(all));
}

/// **The numbers and the words that came out of a document, asserted against the
/// document.** Everything else below is proved by a capture; these cannot be, because
/// no committed capture sits in the bands they draw — no regular container in the
/// repository has one or two restarts, and none exited a code outside `1` and `137`.
/// A constant transcribed from REQUIREMENTS is still a requirement, and without this
/// test lowering rule 5's warn band to a single restart stays green.
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
        (137, Some("Error"), "did not stop when it was asked to"),
        (137, None, "did not stop when it was asked to"),
        (143, None, "ordinary shutdown"),
        (1, None, "the application's own error"),
        (2, None, "the application's own error"),
        (126, None, "could not be run"),
        (127, None, "was not found"),
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
        looping.evidence.contains("the last run lasted 2s"),
        "D51's first fork of a crashloop triage — how long each run survives, which \
         `describe` makes a human subtract at 3am: {}",
        looping.evidence
    );
    assert!(
        looping
            .evidence
            .contains("exit 1 (the application's own error)"),
        "invariant 14: the code is translated, never printed and left: {}",
        looping.evidence
    );
    assert_eq!(
        looping.kubectl_cmd.as_deref(),
        Some("kubectl describe pod broken-crashloop -n default"),
        "and the command shows the state, the last termination and the count the card \
         just claimed"
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
        Some("28 min ago"),
        "a duration, not English parsed back into a number"
    );

    let failed = only(&all, "broken-crashloop", "previous run failed");
    assert_eq!(failed.severity, Severity::Warn);
    assert_eq!(
        failed.action,
        "the last thing it logged was: panic: dial tcp db.payments.svc:5432: connect: \
         connection refused",
        "the kubelet kept the tail of the log, so the card shows it instead of sending \
         the reader to fetch what k8rs is already holding — and it is the *last* line, \
         not the `starting` this capture opens with"
    );
    assert!(
        failed.evidence.contains("ran for 2s"),
        "and how long the run survived, which is the fork between bad configuration and \
         a leak: {}",
        failed.evidence
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
            .filter(|f| f.title.contains("previous run failed"))
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
        "rules 1 and 2 — this container is crash-looping *and* was OOM-killed, and that \
         is one incident with two causes to name, not three cards: {:?}",
        titles(&all)
    );
    // Rule 1 calls the same translator, so the memory sentence survives where the
    // reason earns it — and this is the only card in the box that still says it.
    let looping = only(&all, "broken-oom", "CrashLoopBackOff");
    assert!(
        looping.evidence.contains("more memory than it was allowed"),
        "exit 137 *with* `OOMKilled` beside it is the memory kill: {}",
        looping.evidence
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
#[test]
fn a_container_that_looks_fine_still_gets_a_card_for_how_often_it_has_died() {
    let all = findings(&["restarts"]);
    show(&all);
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
                .filter(|f| f.title.contains("previous run failed"))
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

/// **Rule 6's third action, on the capture that reaches it.** The arms are: the container's own
/// last log line if the kubelet kept one, then `126`/`127`'s *"the command is not in the
/// image"*, then the general *"read the logs"*. The log-line arm answers first whenever a
/// message exists, and every committed termination carried one — so `broken-notfound` was
/// captured with `terminationMessagePolicy` left at its default and a command that is not in
/// the image, which is `127` with nothing beside it.
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
    assert_eq!(
        run.message, None,
        "the log-line arm answers first whenever a message exists, so this capture has to \
         carry none or the arm under test is unreachable"
    );

    let all = findings(&["notfound"]);
    show(&all);
    let failed = only(&all, "broken-notfound", "previous run failed");
    assert!(
        failed.title.contains("was not found"),
        "invariant 14: 127 is translated, never printed and left: {}",
        failed.title
    );
    assert_eq!(
        failed.action,
        "check the container's command and arguments — what they name is not in the image",
        "and the action points at the manifest, not at a log that says the same word the \
         card already said"
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
    assert!(
        migrate.restarts >= RESTARTS_WARN
            && matches!(&migrate.state, ContainerState::Waiting { reason, .. }
                if reason.as_deref() == Some("CrashLoopBackOff")),
        "a capture whose init container is healthy proves nothing about the gap: {:?}",
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
    assert_eq!(
        all.len(),
        2,
        "rules 1 and 6 on `migrate`, and nothing on `app`: a container that is waiting \
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

    let looping = only(&all, "broken-init", "CrashLoopBackOff");
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
    assert!(
        looping
            .evidence
            .contains(&format!("{migrate_count} restarts")),
        "the init container's own count, not the app container's zero: {}",
        looping.evidence
    );

    let previous = only(&all, "broken-init", "previous run failed");
    assert_eq!(
        previous.severity,
        Severity::Warn,
        "rule 6 is the WARN beside rule 1's CRITICAL wherever the container is *also* \
         broken right now, and it is the exit code that says why"
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
    assert_eq!(
        all.len(),
        2,
        "rules 5 and 6 on an init container that gave up: {:?}",
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

    let amber = only(
        &findings(&["restarts10serving"]),
        "broken-restarts10serving",
        &format!("restarted {} times", serving.restarts),
    )
    .clone();
    show(std::slice::from_ref(&amber));
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
}

/// A committed capture with one field moved — the technique the rest of this file uses
/// for a shape no capture holds. The committed JSON is never touched (NOTES § D53); the
/// decoded copy is.
fn capture_but(name: &str, edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    let mut object: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    edit(&mut object);
    PodSnapshot::from(object)
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
/// kubelet's reason, with the previous run and the restart count cleared beside it.
///
/// The three fields are one coherent group and are moved together (NOTES § D40):
/// `lastState` is precisely what tells [`stuck_at_the_starting_line`] that a container has
/// run before, so a plant that moved `state` alone would be a container that has both never
/// started and a previous run — a shape no kubelet writes, and one the rule under test is
/// right to be silent on. The 2026-08-13 capture is what made that visible: the trip ran long
/// enough for `hostpath.json`'s and `healthy-sidecar.json`'s containers to finish their
/// `sleep 3600` and be restarted, so the bases these plants sit on grew a `lastState` the
/// first capture's did not have.
fn never_ran(pod: &mut Pod, name: &str, reason: &str, message: Option<&str>) {
    let status = container_status(pod, name);
    status.state = waiting_at(reason, message);
    status.last_state = None;
    status.restart_count = 0;
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
        Some("1 hour ago"),
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

    assert_eq!(
        all.len(),
        26,
        "one card per thing that is broken across every pod the repository has captured, \
         counted rather than described: the list is long enough now that a sentence naming \
         each one would be a second copy of the tests above, and a number that moves when a \
         rule starts firing twice is what this assertion is for: {:?}",
        titles(&all)
    );

    // **Which captures are allowed to say nothing, named — and everything else has to speak.**
    // A count alone passes just as well if one rule went silent and another started firing
    // twice. The silent set is the healthy fixtures, the three that are only an Analysis
    // posture row, the two pods that are *over* — a finished pod's restart counts and last exits
    // belong to the Waste report and not to the screen that holds what is broken now (D2) — and
    // the one whose fault is real but old.
    let silent = [
        // The kill in this one is an hour old and its container has been serving since, which
        // is rule 2's recency clause deciding — read at a `now` five minutes after the kill it
        // is a CRITICAL, and `an_old_kill_on_a_container_that_has_been_fine_since_…` reads it
        // both ways off these same bytes.
        "oomserving",
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

/// **The one pod of `owned-pods.json`, by the name the capture gives it.** A ReplicaSet's
/// pods carry a generated five-character suffix minted fresh on every `just fixtures`,
/// while the ReplicaSet's own name is a hash of the pod template and does not move — so
/// the suffix is read out of the capture and the hash is written down.
fn owned_pod_name() -> String {
    let pods = items::<Pod>("owned-pods");
    assert_eq!(
        pods.len(),
        1,
        "`broken-owned` runs one replica, and every assertion below names *the* pod"
    );
    pods[0]
        .metadata
        .name
        .clone()
        .expect("a captured pod has a name")
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
    let looping = only(&all, &name, "CrashLoopBackOff");
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
        Some(format!("kubectl describe pod {name} -n default").as_str()),
        "the command still points at the object, never at the card's title — a \
         `describe pod broken-owned-7bdb7645c8` is a command that does not work"
    );
}

// --- THE NODE RULES, AGAINST THE COMMITTED CAPTURES ---
//
// `scripts/cluster.sh break-nodes` gives each worker exactly one broken state, so three of
// the six rules have a real positive: `k8rs-worker3`'s kubelet stopped posting (N1),
// `k8rs-worker` is cordoned with pods on it (N2), and `k8rs-worker2` carries an operator's
// `dedicated=gpu:NoExecute` (N6). The other three cannot be captured off a healthy-enough
// cluster — no node in it is under pressure, no kubelet is behind the control plane, and
// nothing is over-promised — so those are planted into a **decoded copy** the same way rule
// 8's socket escalators are, one coherent group of fields at a time (NOTES § D40, § D53).
//
// **The negatives are the half that matters here**, because five of the six rules join the
// pods to a node and a join is the easiest thing in this file to get quietly wrong: a count
// that includes what a drain would never move fires N2 on every node an operator drained
// correctly, and a sum that maxes a sidecar instead of adding it reports a full node healthy.

/// The captured node list, decoded — the other half of every join below.
fn captured_nodes() -> Vec<NodeSnapshot> {
    items::<Node>("nodes").into_iter().map(Into::into).collect()
}

/// **Every pod the capture holds, in both namespaces it photographed.** The node rules are
/// joins, and joining only the twelve `default` pods hides the two shapes N2 exists to skip:
/// `kube-system` is where the DaemonSet and the static pods are, and on this cluster they are
/// the only ones there are.
fn every_captured_pod() -> Vec<PodSnapshot> {
    CAPTURED_PODS
        .iter()
        .map(|n| pod(n))
        .chain(
            items::<Pod>("kube-system-pods")
                .into_iter()
                .map(PodSnapshot::from),
        )
        .collect()
}

/// [`pods_at`] with the node list filled in — the snapshot a node rule is actually handed.
fn cluster(pods: Vec<PodSnapshot>, nodes: Vec<NodeSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        nodes,
        ..pods_at(pods, now())
    }
}

/// One captured node with one field moved — [`capture_but`]'s counterpart for the object the
/// N-series is about. The committed JSON is never touched (NOTES § D53).
fn node_but(name: &str, edit: impl FnOnce(&mut Node)) -> NodeSnapshot {
    let mut object: Node = serde_json::from_value(captured_item(&fixture("nodes"), name).clone())
        .unwrap_or_else(|e| panic!("{name} is not a Node in nodes.json: {e}"));
    edit(&mut object);
    NodeSnapshot::from(object)
}

/// One condition of a captured node, to be written through — [`condition_of`]'s node twin.
fn node_condition_mut<'a>(node: &'a mut Node, type_: &str) -> &'a mut NodeCondition {
    node.status
        .as_mut()
        .expect("a captured node has a status")
        .conditions
        .iter_mut()
        .flatten()
        .find(|c| c.type_ == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition"))
}

/// **One captured pod on `node` that a drain would still have to move** — the live half of the
/// N2 counts below, read out of the capture rather than named. Which worker the scheduler put a
/// pod on is its business and moves on every `just fixtures`, so a name here asserts the trip
/// that happened rather than the requirement.
fn a_pod_a_drain_would_move_on(node: &str) -> PodSnapshot {
    CAPTURED_PODS
        .iter()
        .map(|n| pod(n))
        .find(|p| p.node.as_deref() == Some(node) && a_drain_would_move(p) && !finished(p))
        .unwrap_or_else(|| {
            panic!(
                "no captured pod on {node} that a drain would move, so the count below is untested"
            )
        })
}

/// The pods of a snapshot that are **running** on one node, by the field the join is made on —
/// [`pods_on`]'s expectation, re-derived. A pod that has finished keeps its `nodeName` until
/// something collects it (`succeeded.json`, `failed.json`), and every N-series count is about
/// work the machine is still doing, so the phase filter belongs on both sides of the comparison.
fn on_node<'a>(pods: &'a [PodSnapshot], node: &str) -> Vec<&'a PodSnapshot> {
    pods.iter()
        .filter(|p| p.node.as_deref() == Some(node) && !finished(p))
        .collect()
}

/// The one node in the capture whose `Ready` is not `True`, read out of the JSON rather than
/// transcribed: `break-nodes` picks which worker it stops, and a literal here would assert the
/// capture that happened to be taken (NOTES § D65).
fn the_quiet_node(raw: &serde_json::Value) -> &str {
    let down: Vec<&str> = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| captured_condition(n, "Ready")["status"] != "True")
        .map(|n| captured_str(n, &["metadata", "name"]))
        .collect();
    assert_eq!(
        down.len(),
        1,
        "`break-nodes` stops exactly one kubelet, and N1's positive is that node: {down:?}"
    );
    down[0]
}

/// **N1, and the gap it was written to close** (NOTES § D71). The capture's own `healthy` pod
/// runs on the node whose kubelet stopped posting, and its status is a fossil: `Running`,
/// `ready: true`, no restarts, forever. Every other rule in this file reads pod status, so
/// without this card the workload that is actually offline produces nothing at all and Alerts
/// says a node is down in one place and nothing about what went down with it.
///
/// **The evidence names owners, not a count** — that is N2's question, and this card's job is to
/// hand the reader a workload to go and check, because no other card will.
#[test]
fn the_node_that_went_quiet_names_the_workloads_that_went_with_it() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), captured_nodes()));
    show(&all);

    // The fossil, first: nothing else on the screen mentions this node's workload.
    let here = on_node(&pods, quiet);
    assert!(
        here.len() >= 4,
        "the node that stopped answering is carrying real work, or this rule is being \
         proved on an empty machine: {}",
        here.len()
    );
    // **Which pod is the fossil belongs to the scheduler**, so it is found by the property
    // rather than by name: still `Running`, every container still `ready`, on a machine the
    // control plane has given up on. A capture where the scheduler happened to place things
    // differently must not redden a requirement that never moved (NOTES § D65).
    let fossil = here
        .iter()
        .find(|p| p.phase.as_deref() == Some("Running") && p.containers.iter().all(|c| c.ready))
        .expect(
            "the node break-nodes stopped is carrying a pod whose status still reads healthy — \
             without one, D71's premise is not in the capture at all",
        );
    println!(
        "{} on {quiet}: phase {:?}, ready {:?}",
        fossil.id.name,
        fossil.phase,
        fossil
            .containers
            .iter()
            .map(|c| c.ready)
            .collect::<Vec<_>>()
    );
    assert!(
        !all.iter()
            .any(|f| f.object.kind == ObjectKind::Pod && f.object.name == fossil.id.name),
        "and no pod rule fires for it, because every one of them reads that fossil: \
         without this node card Alerts is silent about the workload that is actually \
         offline (D71): {:?}",
        titles(&all)
    );

    let card = only(&all, quiet, "stopped responding");
    assert_eq!(
        card.severity,
        Severity::Critical,
        "a machine the control plane cannot reach is broken now, not risky later (D2)"
    );
    assert_eq!(
        card.owner, card.object,
        "a node has no owner to file under (D39)"
    );
    assert_eq!(card.object.kind, ObjectKind::Node);
    assert_eq!(
        card.object.namespace, None,
        "a node is cluster-scoped, and `infra/node-3` is a card nobody can act on"
    );

    // The requirement re-derived, not the implementation re-read: up to two owners
    // alphabetically, then how many were left out, then the total pod count beside it.
    let mut owners: Vec<String> = here
        .iter()
        .map(|p| match &p.owner.namespace {
            Some(ns) => format!("{ns}/{}", p.owner.name),
            None => p.owner.name.clone(),
        })
        .collect();
    owners.sort();
    owners.dedup();
    assert!(
        owners.len() > 2,
        "the capture has to reach past the two-name cap for the `and N more` half to be \
         proved at all: {owners:?}"
    );
    assert_eq!(
        card.evidence,
        format!(
            "{}, {} and {} more were running here ({} pods)",
            owners[0],
            owners[1],
            owners.len() - 2,
            here.len()
        ),
        "`screens/alerts.md` § N1 — two names, then a count, and the total in brackets"
    );

    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_condition(captured_item(&raw, quiet), "Ready"),
            &["lastTransitionTime"]
        )),
        "the `Ready` condition's own transition — the moment the node stopped being one"
    );
    assert_eq!(
        card.age(&now()).as_deref(),
        Some("23 min ago"),
        "a duration off the pinned now, not English parsed back into a number"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some(format!("kubectl describe node {quiet}").as_str()),
        "`describe node` prints the conditions with their reasons and the pods the node is \
         carrying — both halves of this card (invariant 4)"
    );

    // And the three nodes that are answering draw nothing of their own.
    let node_cards: Vec<&str> = all
        .iter()
        .filter(|f| f.object.kind == ObjectKind::Node)
        .map(|f| f.object.name.as_str())
        .collect();
    println!("node cards: {node_cards:?}");
    assert!(
        !node_cards.contains(&"k8rs-worker2"),
        "a node that is Ready, uncordoned and under no pressure has no card: {node_cards:?}"
    );
}

/// **The five minutes are Kubernetes' own, and both sides of them are tested.** A node the
/// control plane has not heard from for four minutes is a kubelet restart, a node upgrade or a
/// network blip, and every one of those resolves without anybody being paged.
#[test]
fn a_node_is_given_the_same_five_minutes_kubernetes_gives_it() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let stopped = captured_time(
        captured_condition(captured_item(&raw, quiet), "Ready"),
        &["lastTransitionTime"],
    );
    let at = |secs: i64| {
        let moment = Time(
            stopped
                .0
                .checked_add(SignedDuration::from_secs(secs))
                .expect("a few minutes after the capture is a moment"),
        );
        analyze(&ClusterSnapshot {
            now: moment,
            ..cluster(every_captured_pod(), captured_nodes())
        })
    };

    let inside = at(300);
    assert!(
        !inside
            .iter()
            .any(|f| f.title.contains("stopped responding")),
        "exactly five minutes in is still the window Kubernetes itself waits before it \
         moves anything: {:?}",
        titles(&inside)
    );
    let outside = at(301);
    assert!(
        outside
            .iter()
            .any(|f| f.title.contains("stopped responding")),
        "one second past it is an outage: {:?}",
        titles(&outside)
    );

    // **The number is borrowed, not picked** — and the capture carries the proof, on every
    // pod: `--default-unreachable-toleration-seconds` is what the admission controller writes,
    // and it is how long Kubernetes waits before it starts evicting from a node it cannot
    // reach. `Toleration` deliberately drops `tolerationSeconds`, so this reads the JSON.
    let tolerations = fixture("crashloop")["spec"]["tolerations"].clone();
    let unreachable = tolerations
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["key"] == "node.kubernetes.io/unreachable")
        .expect("the admission controller writes this onto every pod in the cluster");
    println!("{unreachable}");
    assert_eq!(
        NODE_DOWN_GRACE.as_secs(),
        unreachable["tolerationSeconds"]
            .as_i64()
            .expect("the toleration carries its seconds"),
        "N1's window is the one the cluster itself is running with"
    );
}

/// **A kubelet that answered and said no is not a kubelet that went quiet**, and the card may
/// not say *"has stopped responding"* about a machine that is talking (invariant 14). The
/// kubelet's own sentence is the diagnosis on that branch, so it is carried verbatim
/// (NOTES § D37) — where a silent node has no sentence to carry.
///
/// **Planted:** no captured node is `Ready: False`. `break-nodes` stops a kubelet, which
/// produces `Unknown`; `False` is what a live kubelet writes when its container runtime or its
/// network will not come up, and the message below is `pkg/kubelet/kubelet.go`'s own
/// `runtimeState` sentence. **Capture trip:** a node with a broken CNI retires this.
#[test]
fn a_node_that_answered_and_said_no_is_a_different_card_from_one_that_went_quiet() {
    let refusing = node_but("k8rs-worker2", |n| {
        let ready = node_condition_mut(n, "Ready");
        ready.status = "False".to_string();
        ready.reason = Some("KubeletNotReady".to_string());
        ready.message = Some(
            "container runtime network not ready: NetworkReady=false reason:NetworkPluginNotReady \
             message:Network plugin returns error: cni plugin not initialized"
                .to_string(),
        );
        ready.last_transition_time = Some(time("2026-08-12T21:00:00Z"));
    });
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), vec![refusing]));
    show(&all);

    let card = only(&all, "k8rs-worker2", "cannot run pods");
    assert_eq!(card.severity, Severity::Critical);
    assert!(
        !card.title.contains("stopped responding") && !card.action.contains("powered on"),
        "the machine is answering — asking whether it is powered on wastes the first \
         thing the reader does: {} / {}",
        card.title,
        card.action
    );
    // **Framed the way rule 10 frames the scheduler's sentence** (D81): glued straight on to the
    // owner list with a `·`, a kubelet's `NetworkReady=false reason:NetworkPluginNotReady` reads
    // as k8rs's own prose, and the reader meets four pieces of jargon with nothing saying who
    // wrote them.
    assert!(
        card.evidence
            .contains("the kubelet's own words (the kubelet is the part of Kubernetes that runs on the machine): container runtime network not ready"),
        "the kubelet said what is wrong; the frame says a machine wrote it and glosses the one \
         word the card would otherwise leave bare (D37, invariant 14): {}",
        card.evidence
    );
    let here = on_node(&pods, "k8rs-worker2");
    assert!(
        card.evidence
            .contains(&format!("are running here ({} pods)", here.len())),
        "and the tense follows: these pods are still reporting, because the kubelet that \
         reports them is up: {}",
        card.evidence
    );
}

/// **An undated condition still draws the card**, rule 10's direction and not rule 13's: a node
/// that cannot be *shown* to have just gone quiet is read as one that has been quiet, which is
/// the safe direction — and the right edge is empty rather than borrowed from somewhere else.
///
/// **Planted:** every captured condition carries its stamp; a prune that dropped the field is
/// what this is about, and no capture can hold one (invariant 6).
#[test]
fn a_node_whose_condition_carries_no_stamp_still_draws_the_card_without_an_age() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw).to_string();
    let undated = node_but(&quiet, |n| {
        node_condition_mut(n, "Ready").last_transition_time = None;
    });
    let all = analyze(&cluster(every_captured_pod(), vec![undated]));
    show(&all);

    let card = only(&all, &quiet, "stopped responding");
    assert_eq!(card.timestamp, None);
    assert_eq!(
        card.age(&now()),
        None,
        "no field to point at is the empty right edge, never a zero that draws as 1970"
    );
}

/// **N2, and the count that is its trigger.** The cordoned node in the capture carries a mix of
/// ordinary pods and node agents, and a drain would move only the first kind: `kubectl drain`
/// never evicts a DaemonSet pod or a static pod, whatever flags it is given, so counting what
/// runs there would put this card on every node an operator drained perfectly (NOTES § D46).
/// Both counts are re-derived from the capture below rather than written down, because which
/// pods the scheduler put on the cordoned node is its business and moves on every trip.
#[test]
fn the_cordoned_node_counts_only_the_pods_a_drain_would_actually_move() {
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("`break-nodes` cordons one worker, and N2's positive is that node");
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), captured_nodes()));
    show(&all);

    let here = on_node(&pods, cordoned);
    // Re-derived from the pod's own fields rather than by calling [`a_drain_would_move`], which
    // would agree with any narrowing it happened to have: the three `kubectl drain` skips are a
    // static pod, a DaemonSet's pod, and one the drain has already evicted.
    let skipped: Vec<&str> = here
        .iter()
        .filter(|p| {
            p.mirror || p.owner.kind == ObjectKind::DaemonSet || p.deletion_timestamp.is_some()
        })
        .map(|p| p.id.name.as_str())
        .collect();
    println!("{} pods on {cordoned}, drain skips {skipped:?}", here.len());
    assert!(
        !skipped.is_empty(),
        "kindnet and kube-proxy run on every kind node, and without them in the snapshot \
         this test cannot tell a filtered count from an unfiltered one"
    );

    let card = only(&all, cordoned, "refuses new pods");
    assert_eq!(card.severity, Severity::Warn);
    assert_eq!(card.title, "This node refuses new pods (cordoned)");
    assert_eq!(
        card.evidence,
        format!(
            "{} pods here would still have to move",
            here.len() - skipped.len()
        ),
        "the number a `kubectl drain` would actually move — the same computation the next \
         command the reader types performs (`screens/alerts.md`)"
    );
    assert_eq!(
        card.action, "allow new pods once the work is done",
        "it states the lifecycle and does not accuse: true whether the cordon was five \
         minutes ago or five months ago"
    );

    // **The command has to be able to show the number beside it.** `kubectl describe node`
    // prints `Taints:` and never `timeAdded`, so this one card does not point at it (D69).
    // **`describe node`, not the jsonpath line** (D81 reversing D69's other horn): it prints
    // `Unschedulable: true` and the `Non-terminated Pods` table, which are the title and the
    // count — and the count is the trigger, so it is on every one of these cards.
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some(format!("kubectl describe node {cordoned}").as_str()),
        "the age is the one claim this command cannot back, and it is the optional half"
    );
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_item(&raw, cordoned)["spec"]["taints"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|t| t["key"] == CORDON_TAINT)
                .expect("the node lifecycle controller mirrors the boolean onto a taint"),
            &["timeAdded"]
        )),
        "the age is the taint's, which the controller stamps — never `Ready`'s, which does \
         not move when a node is cordoned (D65)"
    );
    assert_eq!(card.age(&now()).as_deref(), Some("24 min ago"));
}

/// **A node a drain finished with is parked, not broken** — and both of the two shapes a drain
/// refuses to move are in the capture, on two different nodes. Counting either of them is what
/// puts N2 on a correctly drained node, which is the false positive the narrowing exists for
/// (NOTES §  D43, § D46).
///
/// **Half planted:** the control plane is not cordoned in the capture, so the boolean is moved
/// on to a decoded copy of it — one field, and the field is a `kubectl cordon` away.
#[test]
fn a_cordoned_node_with_nothing_a_drain_would_move_draws_no_card() {
    let system: Vec<PodSnapshot> = items::<Pod>("kube-system-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();

    // The DaemonSet half, on the node the capture really did cordon.
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("one worker is cordoned in the capture");
    let agents: Vec<PodSnapshot> = on_node(&system, cordoned).into_iter().cloned().collect();
    println!(
        "{cordoned} keeps {:?}",
        agents.iter().map(|p| &p.id.name).collect::<Vec<_>>()
    );
    assert!(
        !agents.is_empty() && agents.iter().all(|p| p.owner.kind == ObjectKind::DaemonSet),
        "kindnet and kube-proxy are what a drained kind node is left running"
    );
    let drained = analyze(&cluster(agents, captured_nodes()));
    show(&drained);
    assert!(
        !drained.iter().any(|f| f.title.contains("refuses new pods")),
        "a node whose last two pods are DaemonSet pods is parked, and Alerts holds only \
         what is broken: {:?}",
        titles(&drained)
    );

    // The static-pod half: a control-plane node cordoned for an upgrade still runs four pods
    // no drain can move, and its own `coredns` replicas are deliberately left out — those a
    // drain *would* move, and this is the case where nothing is left.
    let statics: Vec<PodSnapshot> = system
        .iter()
        .filter(|p| p.mirror || p.owner.kind == ObjectKind::DaemonSet)
        .filter(|p| p.node.as_deref() == Some("k8rs-control-plane"))
        .cloned()
        .collect();
    assert!(
        statics.iter().filter(|p| p.mirror).count() >= 4,
        "the kubelet mirrors etcd, the apiserver, the scheduler and the controller manager"
    );
    let upgrading = analyze(&cluster(
        statics,
        vec![node_but("k8rs-control-plane", |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .unschedulable = Some(true);
        })],
    ));
    show(&upgrading);
    assert!(
        !upgrading
            .iter()
            .any(|f| f.title.contains("refuses new pods")),
        "four static pods are not a half-finished drain: {:?}",
        titles(&upgrading)
    );
}

/// **N2 is silent while an autoscaler is deliberately emptying the node** — it is cordoned with
/// pods on it for the whole eviction window by design, so a card here fires repeatedly on a
/// cluster doing exactly what it was configured to do. A scale-down that never finishes is the
/// Drain safety report's row (NOTES § D43).
///
/// **Planted:** no cloud autoscaler runs on kind. Both taints are declared upstream — the
/// cluster-autoscaler one carries the unix second of the scale-down in its value, Karpenter's
/// carries no value at all — and both are `NoSchedule`.
#[test]
fn a_node_an_autoscaler_is_taking_away_is_not_a_half_finished_drain() {
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("one worker is cordoned in the capture")
        .to_string();
    let pods = every_captured_pod();

    // Not vacuous: the same node without the taint is N2's positive.
    let plain = analyze(&cluster(pods.clone(), captured_nodes()));
    assert!(
        plain.iter().any(|f| f.title.contains("refuses new pods")),
        "this node is N2's positive, or the silence below proves nothing"
    );

    for (key, value) in [
        ("ToBeDeletedByClusterAutoscaler", Some("1755037382")),
        ("karpenter.sh/disrupted", None),
    ] {
        let retiring = node_but(&cordoned, |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .taints
                .get_or_insert_with(Vec::new)
                .push(ApiTaint {
                    key: key.to_string(),
                    value: value.map(str::to_string),
                    effect: "NoSchedule".to_string(),
                    time_added: None,
                });
        });
        let all = analyze(&cluster(pods.clone(), vec![retiring]));
        show(&all);
        assert!(
            !all.iter().any(|f| f.title.contains("refuses new pods")),
            "{key} means an operation in progress, not one that stopped half way: {:?}",
            titles(&all)
        );
    }
}

/// **N2 and N5 do not run at all when the view is one namespace** (NOTES § D43, § D46). Both
/// join every pod on a node, and a fraction of the pods turns N2's count into a silence and
/// N5's sum into a number that reads as *fine*. The screen says which check is off — Phase 9's
/// banner, and deliberately not a finding from this file.
///
/// **N1 is unaffected as a card and loses its evidence line**: the node's own condition is not
/// namespaced, but *"one pod was running here"* about a node carrying forty is the wrong
/// number this screen exists not to print.
#[test]
fn the_two_rules_that_need_every_pod_do_not_answer_from_one_namespace() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let scoped = ClusterSnapshot {
        namespace_scope: Some("default".to_string()),
        ..cluster(every_captured_pod(), captured_nodes())
    };
    let all = analyze(&scoped);
    show(&all);

    assert!(
        !all.iter().any(|f| f.title.contains("refuses new pods")),
        "N2 counts what a drain would move, and it cannot see the pods to count: {:?}",
        titles(&all)
    );
    assert!(
        node_overcommitted(
            &scoped,
            scoped.nodes.first().expect("the capture has nodes")
        )
        .is_none(),
        "and N5 does not add up a fraction of a node's pods"
    );

    let card = only(&all, quiet, "stopped responding");
    assert_eq!(
        card.evidence, "",
        "the card stays — the node's condition is not namespaced — and the line that would \
         have counted from a partial list is simply not drawn: {}",
        card.evidence
    );
}

/// **N3 names every pressure the node has, and dates each from its own condition.** Reading
/// `Ready`'s `lastTransitionTime` off the same flat list is the trap this rule is one of three
/// warned about: a DiskPressure card would carry the node's boot time (NOTES § D69).
///
/// **Planted:** nothing in the capture is under pressure — the unreachable node's three read
/// `Unknown`, which is N1's answer, not this one. `True` with `KubeletHasDiskPressure` is what
/// the kubelet writes when the image filesystem crosses its eviction threshold.
#[test]
fn the_node_running_low_names_what_it_is_low_on_and_when_that_started() {
    let disk = time("2026-08-12T22:00:00Z");
    let memory = time("2026-08-12T23:00:00Z");
    let pressured = node_but("k8rs-worker2", |n| {
        let c = node_condition_mut(n, "DiskPressure");
        c.status = "True".to_string();
        c.reason = Some("KubeletHasDiskPressure".to_string());
        c.last_transition_time = Some(disk.clone());
    });
    let all = analyze(&cluster(every_captured_pod(), vec![pressured]));
    show(&all);

    let card = only(&all, "k8rs-worker2", "running low");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "evictions are coming is wrong-now-broken-soon, which is what amber means (D2)"
    );
    assert_eq!(
        card.title,
        "This node is running low on disk space — Kubernetes may start evicting pods to free \
         it up",
        "`screens/alerts.md` § N3, word for word — and `DiskPressure` is not a word a \
         beginner has met (invariant 14)"
    );
    assert_eq!(
        card.action,
        "free up disk space on this node, or move some pods elsewhere"
    );
    assert_eq!(
        card.timestamp,
        Some(disk.clone()),
        "*that* condition's transition. `Ready` on this node moved at 20:45:35Z, which is \
         when the machine booted, and a card dated by it is a card dated by nothing (D69)"
    );

    // Two at once: one card, both named, and the earlier of the two stamps it — the question
    // the age answers is how long this has been going on.
    let both = node_but("k8rs-worker2", |n| {
        for (type_, at) in [("DiskPressure", &disk), ("MemoryPressure", &memory)] {
            let c = node_condition_mut(n, type_);
            c.status = "True".to_string();
            c.last_transition_time = Some(at.clone());
        }
    });
    let all = analyze(&cluster(Vec::new(), vec![both]));
    show(&all);
    let card = only(&all, "k8rs-worker2", "running low");
    assert!(
        card.title.contains("running low on disk space and memory"),
        "naming one and hiding the other is what `screens/alerts.md` § N3 forbids: {}",
        card.title
    );
    assert!(
        card.action.contains("free up disk space") && card.action.contains("free up memory"),
        "and the action answers both, or half the node stays broken: {}",
        card.action
    );
    assert_eq!(card.timestamp, Some(disk));

    // The negative, from the capture: `Unknown` is not `True`.
    let healthy = analyze(&cluster(Vec::new(), captured_nodes()));
    show(&healthy);
    assert!(
        !healthy.iter().any(|f| f.title.contains("running low")),
        "the unreachable node's pressures read `Unknown`, and filing *evictions are coming* \
         on a machine nobody can reach is the shape this test exists to catch: {:?}",
        titles(&healthy)
    );
}

/// **N4 — the kubelet the control plane no longer supports.** `Info`, and the whole of its
/// negative side is the capture: `just fixtures` cross-checks the control plane's kubelet
/// against `tests/fixtures/K8S_VERSION`, so a fixture that acquires a skew is announced rather
/// than discovered (NOTES § D65).
///
/// **Planted:** every kubelet in the capture is the version the cluster was built at. A node
/// three minors behind is what an upgrade that stalled on one node group looks like.
#[test]
fn the_kubelet_too_far_behind_the_control_plane_to_be_supported() {
    let server = Some("v1.36.1");
    for node in captured_nodes() {
        assert_eq!(
            kubelet_too_far_behind(server, &node),
            None,
            "{} runs the version the cluster was built at",
            node.id.name
        );
    }

    let behind = |version: &str| {
        node_but("k8rs-worker2", |n| {
            n.status
                .as_mut()
                .expect("a captured node has a status")
                .node_info
                .as_mut()
                .expect("a captured node reports its kubelet version")
                .kubelet_version = version.to_string();
        })
    };
    let found = kubelet_too_far_behind(server, &behind("v1.32.0"))
        .expect("four minors behind is past the window upstream publishes");
    println!("{}", card(&found, &now()));
    assert_eq!(
        found.severity,
        Severity::Info,
        "an unsupported kubelet is a risk to answer this month, not an outage — it is the \
         Versions report's row and never an Alerts card (D2)"
    );
    assert!(
        found.evidence.contains("kubelet v1.32.0")
            && found.evidence.contains("control plane v1.36.1")
            && found.evidence.contains("4 versions behind"),
        "both numbers and the distance between them: {}",
        found.evidence
    );
    assert_eq!(
        found.kubectl_cmd.as_deref(),
        Some("kubectl get nodes -o wide"),
        "the command prints the number this card is about, for every node at once"
    );
    assert_eq!(
        found.timestamp, None,
        "nothing records when a kubelet was installed"
    );

    assert_eq!(
        kubelet_too_far_behind(server, &behind("v1.33.0")),
        None,
        "**exactly three minors behind is supported**, and this is the row the first version of \
         this rule got wrong: upstream says a kubelet may be up to three minor versions older \
         than kube-apiserver, so at two everybody mid-upgrade was told a supported cluster was \
         not (D81)"
    );
    assert_eq!(
        kubelet_too_far_behind(server, &behind("v1.37.0")),
        None,
        "a kubelet *ahead* of the control plane is a different fault and not one of the \
         eleven rules — inventing a card for it here is scope creep (invariant 13)"
    );
    assert_eq!(
        kubelet_too_far_behind(None, &behind("v1.32.0")),
        None,
        "with no control-plane version there is nothing to compare against, and comparing \
         against a guess is the one thing this rule may not do"
    );
    assert_eq!(
        SUPPORTED_SKEW, 3,
        "the number is upstream's own window and the card makes a claim about it: *kubelet may \
         be up to three minor versions older than kube-apiserver* (D81)"
    );
    assert!(
        found.action.contains("at most 3 minor versions older"),
        "and the card cites that window rather than asserting a number of its own: {}",
        found.action
    );
}

/// The version strings a real cluster answers with, none of which is `v1.36.1`.
#[test]
fn a_version_is_read_as_far_as_its_minor_and_no_further() {
    for (version, want) in [
        ("v1.36.1", Some((1, 36))),
        ("1.36.1", Some((1, 36))),
        ("v1.29.7-gke.1104000", Some((1, 29))),
        ("v1.28.15-eks-1234567", Some((1, 28))),
        ("v1.31.4+k3s1", Some((1, 31))),
        ("v1.30.0-rc.2", Some((1, 30))),
        ("", None),
        ("v1", None),
        ("kubelet", None),
    ] {
        println!("{version:>24} -> {:?}", minor_version(version));
        assert_eq!(
            minor_version(version),
            want,
            "{version} is where a distribution's own suffix meets N4's subtraction"
        );
    }
}

/// **N5's arithmetic, on the three captured pods that each break a different naive version of
/// it** (NOTES § D46, § D51). None of the three is planted: `just fixtures` captured them for
/// exactly this rule.
#[test]
fn what_a_node_is_charged_for_a_pod_is_the_number_the_scheduler_uses() {
    // Millicores, so every number below is an exact integer (D81).
    let cpu = |p: &PodSnapshot| {
        charged(
            p,
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref(),
        )
        .expect("every captured request parses")
    };

    // A native sidecar is *added*, never maxed: it runs beside the app for the whole life of
    // the pod. Maxing drops 100m per meshed pod, which is six CPUs invisible on sixty of them.
    let sidecar = pod("healthy-sidecar");
    assert!(
        sidecar
            .containers
            .iter()
            .any(|c| c.role == ContainerRole::Sidecar),
        "the capture declares `restartPolicy: Always` on an init container, or this proves \
         nothing: {:?}",
        sidecar
            .containers
            .iter()
            .map(|c| c.role)
            .collect::<Vec<_>>()
    );
    println!("sidecar pod charged {}m cpu", cpu(&sidecar));
    assert_eq!(
        cpu(&sidecar),
        20,
        "10m for the app and 10m for the sidecar beside it — a maxing sum answers 10m"
    );

    // A pod-level request *replaces* the container sum. The pod below asks for 100m at the pod
    // level and 10m in its one container: adding them answers 0.11, and reading only the
    // containers answers 0.01, on a pod that has committed 100m of the node.
    let pod_level = pod("healthy-podlevel");
    println!("pod-level pod charged {}m cpu", cpu(&pod_level));
    assert_eq!(
        cpu(&pod_level),
        100,
        "KEP-2837: the pod-level number is the one the scheduler charges (D51)"
    );

    // An init container that requests nothing costs nothing, and a pod with no requests at all
    // is not a pod with unknown requests.
    println!("init pod charged {}m cpu", cpu(&pod("init")));
    assert_eq!(cpu(&pod("init")), 0);

    // A quantity that cannot be read stops the node rather than being skipped: an understated
    // sum is a card that says the node is fine, which is the one wrong answer here.
    let broken = capture_but("healthy", |p| {
        p.status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("the kubelet reported on this container")[0]
            .resources = None;
        p.spec
            .as_mut()
            .expect("a captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the capture declares requests")
            .requests
            .as_mut()
            .expect("the capture declares a cpu request")
            .insert("cpu".to_string(), Quantity("not a number".to_string()));
    });
    assert_eq!(
        charged(
            &broken,
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref()
        ),
        None,
        "and it says so rather than guessing low"
    );
}

/// **A node over-promised, out of the capture's own strings.** No node in the capture is:
/// `broken-resize` asks for the whole machine's memory and the kubelet **deferred** the resize, so
/// what it was actually given is 64Mi ([`effective`], NOTES § D51). Landing that resize is one
/// field, and the value planted is the one the same capture already carries in its own `spec`.
///
/// Shared by N5's card test and by the one that proves `analyze` leaves it out, because a rule
/// that cannot be shown to fire proves nothing about being excluded.
fn over_promised() -> ClusterSnapshot {
    let raw = fixture("resize");
    let asked_for = captured_str(
        &raw["spec"]["containers"][0],
        &["resources", "requests", "memory"],
    )
    .to_string();
    let landed = capture_but("resize", |p| {
        let status = p.status.as_mut().expect("a captured pod has a status");
        let enacted = status.container_statuses.as_mut().expect("one container")[0]
            .resources
            .as_mut()
            .expect("the kubelet enacted the original request");
        enacted
            .requests
            .as_mut()
            .expect("with a memory request in it")
            .insert("memory".to_string(), Quantity(asked_for.clone()));
    });
    assert_eq!(
        container(&landed, "app").memory_request.as_deref(),
        Some(asked_for.as_str()),
        "one field moved on a decoded copy, to the value the same capture asks for (D40)"
    );
    let node = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == "k8rs-worker3")
        .expect("the capture has the node broken-resize runs on");
    let pods: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .chain([landed])
        .filter(|p| p.node.as_deref() == Some("k8rs-worker3"))
        .collect();
    cluster(pods, vec![node])
}

/// **N5 — the node has promised more than it has.** `Info`, and the Capacity report's input:
/// nothing is down, which is why it is not on Alerts (NOTES § D2, `screens/analysis.md`).
///
/// **Planted, out of the capture's own strings.** No node in the capture is over-promised —
/// `broken-resize` asks for the whole machine's memory and the kubelet **deferred** the
/// resize, so what it was actually given is 64Mi ([`effective`], NOTES § D51). Landing that
/// resize is one field, and the value planted is the one the same capture already carries in
/// its own `spec`.
#[test]
fn the_node_that_promised_more_than_it_has() {
    let allocatable = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == "k8rs-worker3")
        .and_then(|n| n.allocatable_memory)
        .expect("a node reports what it can give");
    let snapshot = over_promised();
    let found = node_overcommitted(&snapshot, &snapshot.nodes[0])
        .expect("one pod holding the whole machine plus its neighbours is over the line");
    println!("{}", card(&found, &now()));

    assert_eq!(found.severity, Severity::Info);
    assert_eq!(found.object.kind, ObjectKind::Node);
    assert!(
        found.title.contains("promised more memory than it has"),
        "`screens/analysis.md` § Capacity words the row this feeds: {}",
        found.title
    );
    assert!(
        !found.title.contains("nothing new can start"),
        "a pod that requests nothing is placed on a node at 100% of its requests all day, and \
         a beginner who tries it must not be contradicted by their own cluster (D81): {}",
        found.title
    );
    assert!(
        found.evidence.contains(&format!(
            "usable {}",
            bytes(quantity_milli(&allocatable).expect("a captured quantity parses"))
        )),
        "measured against this node's own allocatable, in a unit a manifest is written in: {}",
        found.evidence
    );
    assert_eq!(found.timestamp, None, "an arithmetic is not an event (D69)");

    // The negative is the capture as committed: the resize is still deferred there.
    let real = cluster(every_captured_pod(), captured_nodes());
    for node in &real.nodes {
        assert_eq!(
            node_overcommitted(&real, node),
            None,
            "{} is a twelve-CPU machine running pods that ask for milli-CPUs",
            node.id.name
        );
    }
}

/// The two string functions the Capacity numbers pass through, on values the API writes and
/// this file's own arithmetic produces. Pure functions, so they are asserted as ones — the
/// card above only means what it reads if these do.
#[test]
fn a_quantity_becomes_a_number_and_a_number_becomes_a_size_a_human_reads() {
    // **Every shape the pipeline can hand this**, not the six the committed fixtures happen to
    // carry: each suffix arm was individually deletable with the suite green (D81). `None` is a
    // right answer here; a panic and a wrapped number are not (invariant 5).
    #[rustfmt::skip]
    let table: [(&str, Option<i64>); 46] = [
        // What the API and a manifest actually write.
        ("0",        Some(0)),
        ("100m",     Some(100)),
        ("1",        Some(1_000)),
        ("1.5",      Some(1_500)),
        ("1500m",    Some(1_500)),
        ("1Ki",      Some(1_024_000)),
        ("1Mi",      Some(1_048_576_000)),
        ("1Gi",      Some(1_073_741_824_000)),
        ("1Ti",      Some(1_099_511_627_776_000)),
        ("1Pi",      Some(1_125_899_906_842_624_000)),
        // 1024^6 * 1000 is 1.15e21 — past i64, which is an exabyte node nobody has.
        ("1Ei",      None),
        ("64Ei",     None),
        // Decimal and binary suffixes are different numbers and must not be confused.
        ("100M",     Some(100_000_000_000)),
        ("100Mi",    Some(104_857_600_000)),
        ("1k",       Some(1_000_000)),
        // **Every decimal arm, at a size where it is observable.** `1E` and `1Ei` are past i64,
        // so the whole-unit rows below them answer `None` — and `None` is also what deleting the
        // arm produces, which is how five of these went untested (D81).
        ("1G",       Some(1_000_000_000_000)),
        ("1T",       Some(1_000_000_000_000_000)),
        ("1P",       Some(1_000_000_000_000_000_000)),
        ("0.001E",   Some(1_000_000_000_000_000_000)),
        ("0.001Ei",  Some(1_152_921_504_606_846_976)),
        // Kubernetes has no `K` — only `k`. Answering 1000 here would invent a suffix.
        ("1K",       None),
        // Sub-milli rounds up, `Quantity::MilliValue`'s own direction.
        ("1n",       Some(1)),
        ("1u",       Some(1)),
        ("0n",       Some(0)),
        ("0.5m",     Some(1)),
        // **The exponent form parses** — upstream's grammar has it, `ParseQuantity` accepts it,
        // and a *quoted* `"1e3"` round-trips off a real apiserver verbatim (D81). The doc
        // sentence that used to justify `None` here was a claim about apiserver behaviour that a
        // `--dry-run=server` contradicts, and it cost a whole node its Capacity row.
        ("1e3",      Some(1_000_000)),
        ("1E3",      Some(1_000_000)),
        ("1e-3",     Some(1)),
        // Upstream puts the exponent *in place of* a suffix, so this is not a quantity.
        ("1e3Ki",    None),
        // Not numbers.
        ("1.2.3",    None),
        ("",         None),
        ("NaN",      None),
        ("inf",      None),
        ("m",        None),
        ("100mm",    None),
        ("100Mib",   None),
        // A request cannot be negative, and the sign is not even scanned.
        ("-1",       None),
        ("-100m",    None),
        ("+5",       None),
        // Whitespace is not trimmed anywhere upstream of this, so it is not a number.
        (" 5 ",      None),
        ("5 ",       None),
        // i64::MAX itself: x1000 is past i64, so it is not a number this can carry.
        ("9223372036854775807", None),
        ("9223372036854775", Some(9_223_372_036_854_775_000)),
        ("9223372036854776", None),
        // Past i128, which is where the mantissa parse itself has to give up.
        ("100000000000000000000000000000000000000000", None),
        (".5",       Some(500)),
    ];
    for (q, want) in table {
        let got = quantity_milli(q);
        println!("{q:>44} -> {got:?}");
        assert_eq!(
            got, want,
            "{q:?} is a shape the pipeline hands this function"
        );
    }
    assert_eq!(
        quantity_milli("5."),
        Some(5_000),
        "a trailing point is upstream's grammar too"
    );

    for (milli, want) in [
        (67_108_864_000, "64Mi"),
        (24_860_065_792_000, "23.1Gi"),
        (1_024_000, "1Ki"),
        (1_610_612_736_000, "1.5Gi"),
        // Below a kibibyte Kubernetes writes the bare number, and so does this — a floor the
        // card cannot reach, since no node's allocatable is 512 bytes.
        (512_000, "512"),
    ] {
        println!("{milli:>20} -> {}", bytes(milli));
        assert_eq!(bytes(milli), want);
    }
    assert_eq!(cpu_text(9_100), "9.1");
    assert_eq!(cpu_text(12_000), "12");
    assert_eq!(cpu_text(1), "0.001", "a 1m request is not nothing");
    assert_eq!(cpu_text(0), "0");
}

/// **`100m` × n is where the `f64` fired.** The property that replaced it, asserted as a property
/// and not as one lucky row: exact, and independent of how many.
#[test]
fn millicores_sum_exactly_and_a_float_does_not() {
    let one = quantity_milli("100m").expect("a millicore request parses");
    for n in 1..=100i64 {
        let integer: i64 = (0..n).map(|_| one).sum();
        assert_eq!(
            integer,
            quantity_milli(&format!("{}m", n * 100)).expect("parses"),
            "{n} x 100m must equal {}m exactly",
            n * 100
        );
    }
    // The bug this replaced, reproduced so the test above is known to discriminate.
    let float: f64 = (0..3).map(|_| 100.0 * 1e-3).sum();
    println!("f64: 3 x 100m = {float:.20} vs 0.3 = {:.20}", 0.3_f64);
    assert!(
        float > 0.3,
        "if this ever stops being true the float bug was never reachable and the integer \
         rewrite proved nothing"
    );
}

fn truncate(q: &str) -> String {
    if q.chars().count() > 44 {
        format!("{}…({} chars)", &q[..30], q.len())
    } else {
        q.escape_debug().to_string()
    }
}

/// **Nothing this function is handed may take the process down** — it parses a string that came
/// off the API, and a rule may not panic (invariant 5). The two long ones are not theoretical:
/// `kubectl apply --dry-run=server` against the kind cluster **accepts and stores them verbatim**,
/// so a watch really can hand the decode one (NOTES § D81).
#[test]
fn quantity_milli_never_panics() {
    let mut hostile: Vec<String> = vec![
        String::new(),
        ".".to_string(),
        "..".to_string(),
        "...".to_string(),
        "-".to_string(),
        "0.".to_string(),
        ".0".to_string(),
        "e".to_string(),
        "e-".to_string(),
        "1e".to_string(),
        "1e999999999".to_string(),
        "1e-999999999".to_string(),
        "\u{0}".to_string(),
        "1\u{1b}[2J".to_string(),
        "1".repeat(200),
        format!("{}.{}", "9".repeat(100), "9".repeat(100)),
        format!("0.{}n", "9".repeat(60)),
        format!("{}Ei", "9".repeat(30)),
        // i128::MAX/1000 as the mantissa with the point 20 places in: the numerator lands 727
        // short of i128::MAX, and `numerator + denominator - 1` used to be an unchecked add.
        "1701411834604692.31731687303715884105n".to_string(),
        "170141183460469231731687303715884105n".to_string(),
        "170141183460469231731687303715884105m".to_string(),
        format!("1.{}n", "0".repeat(30)),
    ];
    for suffix in [
        "", "m", "n", "u", "k", "M", "G", "T", "P", "E", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "e9",
    ] {
        hostile.push(format!("{}.{}{suffix}", "9".repeat(40), "9".repeat(30)));
        hostile.push(format!("{}{suffix}", "9".repeat(38)));
    }
    let mut panicked: Vec<String> = Vec::new();
    for q in &hostile {
        match std::panic::catch_unwind(|| quantity_milli(q)) {
            Ok(got) => {
                println!("{:>50} -> {got:?}", truncate(q));
                assert!(
                    got.is_none_or(|m| m >= 0),
                    "{} answered {got:?} — a negative request is a number no quantity can \
                     mean, and in release that is what the unchecked add produced instead of \
                     the panic",
                    truncate(q)
                );
            }
            Err(_) => {
                println!("{:>50} -> PANIC", truncate(q));
                panicked.push(q.clone());
            }
        }
    }
    assert!(
        panicked.is_empty(),
        "a quantity string off the API took a pure rule down (invariant 5): {:?}",
        panicked.iter().map(|q| truncate(q)).collect::<Vec<_>>()
    );
}

/// **The same string, arriving the way it actually would** — through the decode, off a pod spec,
/// into the rule. A panic in a helper is a bug; a panic reachable from a rule is invariant 5. In
/// release, where the add wrapped instead of panicking, the answer was a *negative* sum, which the
/// comparison reads as a node promising less than nothing.
#[test]
fn the_overflow_reaches_the_rule_through_a_real_pod() {
    const HOSTILE: &str = "170141183460469231731687303715884105n";
    let pod = capture_but("healthy", |p| {
        p.spec
            .as_mut()
            .expect("a captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the capture declares requests")
            .requests
            .as_mut()
            .expect("with a cpu request in it")
            .insert("cpu".to_string(), Quantity(HOSTILE.to_string()));
        for status in p
            .status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .iter_mut()
            .flatten()
        {
            status
                .resources
                .as_mut()
                .expect("the kubelet enacted a request")
                .requests
                .as_mut()
                .expect("with a cpu request in it")
                .insert("cpu".to_string(), Quantity(HOSTILE.to_string()));
        }
    });
    println!(
        "decoded cpu_request: {:?}",
        pod.containers
            .iter()
            .map(|c| c.cpu_request.as_deref())
            .collect::<Vec<_>>()
    );
    let node = captured_nodes()
        .into_iter()
        .find(|n| Some(n.id.name.as_str()) == pod.node.as_deref())
        .expect("the pod's node is in the capture");
    let snapshot = cluster(vec![pod], vec![node.clone()]);
    let got = std::panic::catch_unwind(|| node_overcommitted(&snapshot, &node));
    match &got {
        Ok(f) => println!("N5 answered {:?}", f.as_ref().map(|f| &f.evidence)),
        Err(_) => println!("N5 PANICKED"),
    }
    assert!(
        got.is_ok(),
        "one pod requesting a large-but-legal quantity took the rule engine down"
    );
    if let Ok(Some(f)) = got {
        assert!(
            !f.evidence.contains('-'),
            "the sum wrapped and the card printed a negative: {}",
            f.evidence
        );
    }
}

/// **A quoted exponent arrives off a watch, and it used to take the whole node with it.** An
/// unquoted `1e3` is canonicalised to `1k` by the apiserver; a quoted `"1e3"` — how every chart
/// that quotes its quantities writes it — is stored and returned verbatim, because `Quantity`
/// caches the string it was parsed from. Refusing it made `promised` answer `None` for the node,
/// which is one machine silently absent from the Capacity report (NOTES § D81).
#[test]
fn a_quoted_exponent_is_a_number_and_not_a_node_lost_from_the_report() {
    let pod = capture_but("healthy", |p| {
        for status in p
            .status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .iter_mut()
            .flatten()
        {
            status
                .resources
                .as_mut()
                .expect("the kubelet enacted a request")
                .requests
                .as_mut()
                .expect("with a cpu request in it")
                .insert("cpu".to_string(), Quantity("1e3".to_string()));
        }
    });
    assert_eq!(
        container(&pod, "app").cpu_request.as_deref(),
        Some("1e3"),
        "the decode carries the string the apiserver stored, exponent and all"
    );
    let node = captured_nodes()
        .into_iter()
        .find(|n| Some(n.id.name.as_str()) == pod.node.as_deref())
        .expect("the pod's node is in the capture");
    let here: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .filter(|p| p.node.as_deref() == Some(node.id.name.as_str()))
        .chain([pod])
        .collect();
    let snapshot = cluster(here, vec![node.clone()]);
    let borrowed: Vec<&PodSnapshot> = snapshot.pods.iter().collect();
    let sum = promised(
        &borrowed,
        node.allocatable_cpu.as_deref(),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
    );
    println!("cpu sum for {}: {sum:?}", node.id.name);
    let (asked, _) = sum.expect(
        "one quoted exponent among the pods must not delete the whole node from the report",
    );
    assert!(
        asked >= quantity_milli("1e3").expect("the exponent form parses"),
        "and the value counts towards the sum rather than being skipped: {asked}"
    );
}

/// **N4 and N5 are computed in this file and do not reach Alerts** — `Severity::Info` is the
/// line D2 draws, and these two rules are the ones `analyze` does not call at all. **Both rules are
/// shown to fire on the snapshot first**: an exclusion asserted over a rule that answers `None`
/// anyway is an exclusion that stays green the day somebody wires it in (D81).
///
/// **C1 is the one `Info` that does leave `analyze`, and it is not in these snapshots** — neither
/// carries a client certificate, so the loop below is about N4 and N5 and says nothing about the
/// band D87 routes to the Certificates report.
#[test]
fn the_two_info_rules_are_the_reports_input_and_never_an_alerts_card() {
    let skewed = node_but("k8rs-worker2", |n| {
        n.status
            .as_mut()
            .expect("a captured node has a status")
            .node_info
            .as_mut()
            .expect("a captured node reports its kubelet version")
            .kubelet_version = "v1.30.0".to_string();
    });
    let snapshot = ClusterSnapshot {
        server_version: Some("v1.36.1".to_string()),
        ..cluster(every_captured_pod(), vec![skewed.clone()])
    };
    assert!(
        kubelet_too_far_behind(snapshot.server_version.as_deref(), &skewed).is_some(),
        "N4 answers on this snapshot, which is what `analysis.rs` will call it for"
    );

    // N5's own: the capture is not over-promised, so its exclusion has to be asserted over the
    // planted snapshot that is — the half this test used to leave to chance.
    let promised = over_promised();
    let full = promised.nodes.first().expect("the planted node is there");
    assert!(
        node_overcommitted(&promised, full).is_some(),
        "N5 answers on this snapshot too, or the assertion below is about a rule that was \
         never going to say anything"
    );
    let over = analyze(&promised);
    show(&over);
    assert!(
        !over.iter().any(|f| f.title.contains("promised more")),
        "an over-promised node is the Capacity report's row: {:?}",
        titles(&over)
    );

    let all = analyze(&snapshot);
    show(&all);
    for f in all.iter().chain(over.iter()) {
        assert_ne!(
            f.severity,
            Severity::Info,
            "N4 and N5 are not called from here and no certificate is in these snapshots, so \
             nothing this returns may be an Info (D2, § D87): {}",
            f.title
        );
    }
    assert!(
        !all.iter().any(|f| f.title.contains("kubelet")),
        "a skewed kubelet is the Versions report's row: {:?}",
        titles(&all)
    );
}

/// **N6 — the node half of rule 10's card, and not a second card** (NOTES § D28). The captured
/// Pending pod asks for `disktype=ssd` and no node in the cluster is labelled that way, which
/// the scheduler's own sentence agrees with from the other side: *3 node(s) didn't match Pod's
/// node affinity/selector*.
#[test]
fn the_pending_pod_is_told_which_label_nothing_in_the_cluster_has() {
    let nodes = captured_nodes();
    let all = analyze(&cluster(vec![pod("pending")], nodes.clone()));
    show(&all);
    assert_eq!(
        all.iter()
            .filter(|f| f.object.kind == ObjectKind::Pod)
            .count(),
        1,
        "one card about the pod, never a second one about the node that refused it — two \
         findings for one pod is what stops the list being believable (D28): {:?}",
        titles(&all)
    );

    let card = only(&all, "broken-pending", "will take this pod");
    assert_eq!(
        card.object.kind,
        ObjectKind::Pod,
        "D37: the subject is the pod that cannot run, and the node is named in the evidence"
    );
    let wanted = pod("pending").node_selector;
    let unmatched: Vec<&String> = wanted
        .iter()
        .filter(|(k, v)| !nodes.iter().any(|n| n.labels.get(*k) == Some(*v)))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        unmatched.len(),
        1,
        "the capture asks for two labels and the cluster has one of them — `kubernetes.io/os` \
         is on every node, and that is what stops this being a test of *any* selector: {wanted:?}"
    );
    assert!(
        card.evidence.contains(&format!(
            "it asks for a node labelled {}=ssd, and none of the {} nodes have that label",
            unmatched[0],
            nodes.len()
        )),
        "`screens/alerts.md` § N6, and it names the label nothing has rather than the whole \
         selector: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("the scheduler's own words"),
        "the quote stays: it is the only place the *other* refusals appear (D37): {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        format!(
            "change the nodeSelector, or label a node {}=ssd",
            unmatched[0]
        )
    );
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_condition(&fixture("pending"), "PodScheduled"),
            &["lastTransitionTime"]
        )),
        "the pod's own wait, never the blocking node's taint `added_at` — the two clocks \
         answer different questions and only one of them is this card's (D69)"
    );
    assert_eq!(
        card.severity,
        Severity::Critical,
        "`screens/alerts.md` draws N6 amber, and rule 10's ladder overrides it: this card is \
         the same card, and three hours unplaced is past the ten minutes anything resolves in"
    );
}

/// **The other answer: a taint every machine that could take the pod is carrying.** The
/// capture's `dedicated=gpu:NoExecute` is on one worker and the Pending pod tolerates it — so
/// the pod is given the tolerations every *other* pod in the capture has, which is what a pod
/// that was never told about the gpu nodes looks like.
#[test]
fn the_pending_pod_is_told_which_taint_is_refusing_it() {
    let gpu = captured_nodes()
        .into_iter()
        .find(|n| n.taints.iter().any(|t| t.key == "dedicated"))
        .expect("`break-nodes` taints one worker");
    let untolerating = capture_but("pending", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        // The two tolerations the admission controller writes on to every pod, taken off
        // another capture rather than typed here — the gpu one is what is being removed.
        spec.tolerations =
            serde_json::from_value(fixture("crashloop")["spec"]["tolerations"].clone())
                .expect("every captured pod carries the default pair");
        spec.node_selector = None;
    });
    let all = analyze(&cluster(vec![untolerating], vec![gpu.clone()]));
    show(&all);

    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        card.evidence.contains(&format!(
            "{} is tainted dedicated=gpu, and this pod does not tolerate that taint",
            gpu.id.name
        )),
        "`screens/alerts.md` § N6 — the node is named in the evidence, and `key=value` is \
         how `kubectl taint` spells the thing the action asks for: {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        "add a toleration for dedicated=gpu, or remove the taint"
    );

    // And the pod as captured *does* tolerate it, so the same node says nothing about taints.
    let all = analyze(&cluster(vec![pod("pending")], vec![gpu]));
    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        !card.evidence.contains("does not tolerate"),
        "the capture's own toleration matches this taint, and a rule that blamed it anyway \
         would send the reader to add a toleration they already have: {}",
        card.evidence
    );
}

/// **When the join cannot pin the refusal on one thing, the card is exactly what it was.**
/// Three shapes reach that branch, and the middle one is the one worth having a test for: a
/// taint on some of the machines but not all of them means something else is refusing the
/// rest, and a card blaming the taint sends the reader to fix half a problem.
#[test]
fn a_refusal_the_nodes_cannot_explain_leaves_rule_ten_saying_what_it_always_said() {
    let raw = fixture("pending");
    let sentence = captured_str(captured_condition(&raw, "PodScheduled"), &["message"]);
    let plain = format!("the scheduler's own words (a node is one machine): {sentence}");

    // No node list at all — a snapshot that has not been given the node watch. "None of the 0
    // nodes have that label" is the sentence this guard exists to stop.
    let all = analyze(&pods_at(vec![pod("pending")], now()));
    assert_eq!(
        only(&all, "broken-pending", "will take this pod").evidence,
        plain
    );

    // A taint on one candidate machine and not the other.
    let mixed = analyze(&cluster(
        vec![capture_but("pending", |p| {
            let spec = p.spec.as_mut().expect("a captured pod has a spec");
            spec.tolerations = None;
            spec.node_selector = None;
        })],
        vec![
            node_but("k8rs-worker2", |_| {}),
            node_but("k8rs-worker", |n| {
                n.spec.as_mut().expect("a captured node has a spec").taints = None;
            }),
        ],
    ));
    show(&mixed);
    let card = only(&mixed, "broken-pending", "will take this pod");
    assert_eq!(
        card.evidence, plain,
        "one machine is tainted and the other is not, so the taint is not the answer — and \
         an answer that is only true of half the cluster is worse than none: {}",
        card.evidence
    );
    assert!(
        card.action.contains("check what this pod asks for"),
        "and the action falls back to the one the command beside it can start: {}",
        card.action
    );
}

/// **Whether a pod puts up with a taint is upstream's `ToleratesTaint`, field for field** — and
/// every row below is a shape a real manifest writes. Getting any of them backwards is a card
/// that names a taint the pod already tolerates, or silence about the one that is refusing it.
#[test]
fn a_toleration_matches_a_taint_the_way_the_scheduler_matches_it() {
    let taint = Taint {
        key: "dedicated".to_string(),
        value: Some("gpu".to_string()),
        effect: "NoExecute".to_string(),
        added_at: None,
    };
    let toleration = |key: &str, operator: &str, value: Option<&str>, effect: Option<&str>| {
        let mut p = pod("crashloop");
        p.tolerations = vec![Toleration {
            key: Some(String::from(key)),
            operator: Some(String::from(operator)),
            value: value.map(String::from),
            effect: effect.map(String::from),
        }];
        p
    };

    for (label, pod, want) in [
        (
            "the exact pair, with the effect",
            toleration("dedicated", "Equal", Some("gpu"), Some("NoExecute")),
            true,
        ),
        (
            "`Exists` ignores the value",
            toleration("dedicated", "Exists", None, Some("NoExecute")),
            true,
        ),
        (
            "an empty effect tolerates every effect",
            toleration("dedicated", "Equal", Some("gpu"), None),
            true,
        ),
        (
            "the wrong value is not a match",
            toleration("dedicated", "Equal", Some("tpu"), Some("NoExecute")),
            false,
        ),
        (
            "nor is the wrong effect",
            toleration("dedicated", "Equal", Some("gpu"), Some("NoSchedule")),
            false,
        ),
        (
            "nor the wrong key",
            toleration("workload", "Equal", Some("gpu"), Some("NoExecute")),
            false,
        ),
        (
            "an operator nothing implements tolerates nothing",
            toleration("dedicated", "Superset", Some("gpu"), None),
            false,
        ),
    ] {
        println!("{label}: {:?}", pod.tolerations);
        assert_eq!(tolerated(&pod, &taint), want, "{label}");
    }

    // The two the API writes without an operator or without a key at all.
    let mut defaulted = pod("crashloop");
    defaulted.tolerations = vec![Toleration {
        key: Some("dedicated".to_string()),
        operator: None,
        value: Some("gpu".to_string()),
        effect: None,
    }];
    assert!(
        tolerated(&defaulted, &taint),
        "an absent operator is `Equal`, which is upstream's own default"
    );
    let mut everything = pod("crashloop");
    everything.tolerations = vec![Toleration {
        key: None,
        operator: Some("Exists".to_string()),
        value: None,
        effect: None,
    }];
    assert!(
        tolerated(&everything, &taint),
        "an empty key with `Exists` tolerates every taint there is — how a DaemonSet that \
         must run everywhere is written"
    );
    assert!(
        !tolerated(&pod("crashloop"), &taint),
        "and the default pair the admission controller writes tolerates neither"
    );
}

/// **A taint that does not stop anything is not an answer.** `PreferNoSchedule` is a
/// preference the scheduler overrules to place a pod, so a card blaming one would name a taint
/// that is not refusing anybody.
#[test]
fn a_soft_taint_is_never_named_as_the_thing_refusing_a_pod() {
    let soft = node_but("k8rs-worker2", |n| {
        n.spec
            .as_mut()
            .expect("a captured node has a spec")
            .taints
            .as_mut()
            .expect("this worker carries the operator's taint")[0]
            .effect = "PreferNoSchedule".to_string();
    });
    let all = analyze(&cluster(
        vec![capture_but("pending", |p| {
            let spec = p.spec.as_mut().expect("a captured pod has a spec");
            spec.tolerations = None;
            spec.node_selector = None;
        })],
        vec![soft],
    ));
    show(&all);
    assert!(
        !only(&all, "broken-pending", "will take this pod")
            .evidence
            .contains("does not tolerate"),
        "the scheduler places pods on `PreferNoSchedule` machines every day: {:?}",
        titles(&all)
    );
}

/// **The whole committed capture — every pod in both namespaces, every node — through
/// [`analyze`] at once.** `cargo test -- --nocapture` prints what a user would actually read,
/// and the node cards are the half the pod-only run above cannot show.
#[test]
fn the_whole_capture_including_its_nodes_through_the_rules_at_once() {
    let all = analyze(&cluster(every_captured_pod(), captured_nodes()));
    show(&all);
    println!(
        "{} critical, {} warnings, {} info",
        all.iter()
            .filter(|f| f.severity == Severity::Critical)
            .count(),
        all.iter().filter(|f| f.severity == Severity::Warn).count(),
        all.iter().filter(|f| f.severity == Severity::Info).count(),
    );

    let nodes: Vec<(&str, &str)> = all
        .iter()
        .filter(|f| f.object.kind == ObjectKind::Node)
        .map(|f| (f.object.name.as_str(), f.title.as_str()))
        .collect();
    println!("{nodes:#?}");
    assert_eq!(
        nodes.len(),
        2,
        "one node stopped answering and one is cordoned with work left on it — the two \
         states `break-nodes` puts on this cluster that reach Alerts: {nodes:?}"
    );
    for f in &all {
        assert_ne!(f.severity, Severity::Info, "D2: {}", f.title);
        assert!(
            !f.title.is_empty() && !f.action.is_empty(),
            "what happened · what it means · what to do: {f:?}"
        );
        let cmd = f
            .kubectl_cmd
            .as_deref()
            .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
        assert!(
            cmd.contains(&f.object.name),
            "invariant 4's teaching device points at the object the card is about: {cmd}"
        );
        assert_eq!(
            f.object.namespace.is_none(),
            f.object.kind == ObjectKind::Node,
            "a node is cluster-scoped and everything else here is not: {:?}",
            f.object
        );
    }
}

/// The node the capture works hardest, and what the pods on it are charged in millicores.
///
/// **Busiest by what N5 actually sums, not by pod count** — the rule adds one number per
/// container that asks for cpu, so a node carrying twelve pods that ask for nothing stresses the
/// arithmetic less than one carrying four that do, and the float control in the ordering test
/// needs values that differ before it discriminates at all.
fn busiest() -> (NodeSnapshot, Vec<PodSnapshot>, i64) {
    let all = every_captured_pod();
    let (node, here) = captured_nodes()
        .into_iter()
        .map(|n| {
            let here: Vec<PodSnapshot> = all
                .iter()
                .filter(|p| p.node.as_deref() == Some(n.id.name.as_str()))
                .cloned()
                .collect();
            (n, here)
        })
        .max_by_key(|(_, here)| {
            here.iter()
                .flat_map(|p| &p.containers)
                .filter(|c| c.cpu_request.is_some())
                .count()
        })
        .expect("the capture has nodes");
    let borrowed: Vec<&PodSnapshot> = here.iter().collect();
    let (asked, _) = promised(
        &borrowed,
        Some("1"),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
    )
    .expect("every captured request parses");
    assert!(
        asked > 0,
        "a boundary proved at zero promised cpu is not a boundary: {} pods on {}",
        here.len(),
        node.id.name
    );
    (node, here, asked)
}

/// **Exactly full is silent, one millicore over fires.** The line itself, from both sides — every
/// committed fixture is comfortably over or comfortably under it, which is how an exactly-packed
/// node fired unnoticed (NOTES § D81). `noderesources.Fit` admits while
/// `request <= allocatable - requested`, and `describe node` prints `cpu 3920m (100%)` without
/// comment.
///
/// The allocatable is not hand-written: it is this node's own pods' sum, spelled back in the unit
/// the API writes.
#[test]
fn n5_is_silent_at_the_line_and_fires_one_millicore_past_it() {
    let (node, here, asked) = busiest();
    println!("{}: {} pods promise {asked}m cpu", node.id.name, here.len());
    for (allocatable, fires) in [
        (format!("{}m", asked + 1), false),
        (format!("{asked}m"), false),
        (format!("{}m", asked - 1), true),
    ] {
        let mut n = node.clone();
        n.allocatable_cpu = Some(allocatable.clone());
        n.allocatable_memory = None;
        let snapshot = cluster(here.clone(), vec![n.clone()]);
        let got = node_overcommitted(&snapshot, &n);
        println!(
            "  allocatable {allocatable:>8} -> {:?}",
            got.as_ref().map(|f| &f.title)
        );
        assert_eq!(
            got.is_some(),
            fires,
            "promised {asked}m against allocatable {allocatable}: a node packed to exactly its \
             allocatable is legal and ordinary, and one milli past it is not"
        );
        // The blocker's second symptom: the card printed two identical numbers.
        if let Some(f) = got {
            let (promised_text, usable_text) = (cpu_text(asked), cpu_text(asked - 1));
            println!("  evidence: {}", f.evidence);
            assert_ne!(
                promised_text, usable_text,
                "a card whose two numbers print the same says nothing"
            );
            assert!(
                f.evidence
                    .contains(&format!("promised {promised_text} cpu"))
                    && f.evidence.contains(&format!("usable {usable_text} cpu")),
                "the card must print the two numbers it compared: {}",
                f.evidence
            );
        }
    }
}

/// **The memory half of the same line, and it is not a copy of the test above.** N5 has two
/// branches; with only the cpu one on the line, `cargo mutants` turned the memory comparison into
/// `>=` and the whole suite stayed green — the blocker still live on the other half of the same
/// rule, printing `promised 290Mi · usable 290Mi` (NOTES § D81).
#[test]
fn n5_is_silent_at_the_memory_line_too() {
    let all = every_captured_pod();
    let (node, here) = captured_nodes()
        .into_iter()
        .map(|n| {
            let here: Vec<PodSnapshot> = all
                .iter()
                .filter(|p| p.node.as_deref() == Some(n.id.name.as_str()))
                .cloned()
                .collect();
            (n, here)
        })
        .max_by_key(|(_, here)| here.len())
        .expect("the capture has nodes");
    let borrowed: Vec<&PodSnapshot> = here.iter().collect();
    let (asked, _) = promised(
        &borrowed,
        Some("1"),
        |p| p.memory_request.as_deref(),
        |c| c.memory_request.as_deref(),
    )
    .expect("every captured memory request parses");
    assert!(
        asked > 0,
        "a memory boundary proved at zero promised bytes is not one"
    );
    assert_eq!(
        asked % 1000,
        0,
        "a byte count times 1000 — if this is not whole the exact allocatable below cannot be \
         spelled"
    );
    println!(
        "{}: {} pods promise {asked} milli-bytes",
        node.id.name,
        here.len()
    );

    for (allocatable, fires) in [
        ((asked / 1000 + 1).to_string(), false),
        ((asked / 1000).to_string(), false),
        ((asked / 1000 - 1).to_string(), true),
    ] {
        let mut n = node.clone();
        n.allocatable_cpu = None;
        n.allocatable_memory = Some(allocatable.clone());
        let snapshot = cluster(here.clone(), vec![n.clone()]);
        let got = node_overcommitted(&snapshot, &n);
        println!(
            "  allocatable {allocatable:>14} -> {:?}",
            got.as_ref().map(|f| &f.evidence)
        );
        assert_eq!(
            got.is_some(),
            fires,
            "promised {asked} milli-bytes against allocatable {allocatable}: exactly full is \
             legal and ordinary, one byte past it is not"
        );
    }
}

/// **The same pods, summed in a different order, must reach the same verdict.** The blocker's
/// second symptom: watch events reorder `snapshot.pods`, so a sum that is not order-free makes the
/// card flap on a node sitting near the line (NOTES § D81).
#[test]
fn n5s_verdict_does_not_depend_on_the_order_the_pods_arrive_in() {
    let (node, here, asked) = busiest();
    let mut n = node.clone();
    n.allocatable_cpu = Some(format!("{asked}m"));
    n.allocatable_memory = None;

    let mut orders: Vec<Vec<PodSnapshot>> = Vec::new();
    for rotate in 0..here.len() {
        let mut o = here.clone();
        o.rotate_left(rotate);
        orders.push(o.clone());
        o.reverse();
        orders.push(o);
    }
    let mut sorted = here.clone();
    sorted.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    orders.push(sorted.clone());
    sorted.reverse();
    orders.push(sorted);

    let verdicts: Vec<Option<String>> = orders
        .iter()
        .map(|o| {
            let snapshot = cluster(o.clone(), vec![n.clone()]);
            node_overcommitted(&snapshot, &n).map(|f| f.evidence)
        })
        .collect();
    println!(
        "{} orderings of {} pods -> {} distinct verdict(s)",
        verdicts.len(),
        here.len(),
        verdicts.iter().collect::<BTreeSet<_>>().len()
    );
    assert!(
        verdicts.iter().all(|v| v == &verdicts[0]),
        "the same pods in a different order reached a different verdict: {verdicts:?}"
    );
    assert_eq!(
        verdicts[0], None,
        "at exactly the line, every order is silent"
    );

    // The float sum this replaced is *not* order-free, so the assertion above discriminates
    // rather than being true of any arithmetic at all.
    let floats: Vec<f64> = orders
        .iter()
        .map(|o| {
            o.iter()
                .flat_map(|p| &p.containers)
                .filter_map(|c| c.cpu_request.as_deref())
                .filter_map(|q| q.strip_suffix('m'))
                .filter_map(|d| d.parse::<f64>().ok())
                .map(|m| m * 1e-3)
                .sum()
        })
        .collect();
    let distinct = floats.iter().map(|f| f.to_bits()).collect::<BTreeSet<_>>();
    println!(
        "the same sums as f64: {} distinct bit patterns",
        distinct.len()
    );
    assert!(
        distinct.len() > 1,
        "if even the float sum is order-free on this node, the ordering assertion above is \
         proved on the wrong input: {floats:?}"
    );
}

/// The captured unschedulable pod with its own asks cleared, so the taint branch is the one
/// reached and one taint is the whole reason the cluster refuses it.
fn unplaceable() -> PodSnapshot {
    let mut p = pod("pending");
    p.node_selector.clear();
    p.tolerations.clear();
    p.nominated_node_name = None;
    p.deletion_timestamp = None;
    assert_eq!(
        p.scheduled.as_ref().and_then(|c| c.reason.as_deref()),
        Some("Unschedulable"),
        "the capture is a pod no node accepted, or N6 is being proved on the wrong pod"
    );
    p
}

/// Captured nodes carrying one taint and no labels — the whole cluster refusing for one reason.
/// **`count` is not decoration**: the table's actions inflect, and the case it exists for is one
/// machine, so both sides have to be drawn (NOTES § D81).
fn tainted(count: usize, key: &str, value: Option<&str>, effect: &str) -> Vec<NodeSnapshot> {
    captured_nodes()
        .into_iter()
        .take(count)
        .map(|mut n| {
            n.taints = vec![Taint {
                key: key.to_string(),
                value: value.map(str::to_string),
                effect: effect.to_string(),
                added_at: None,
            }];
            n.labels.clear();
            n
        })
        .collect()
}

fn n6_card(key: &str, value: Option<&str>) -> Finding {
    n6_card_on(1, key, value)
}

fn n6_card_on(machines: usize, key: &str, value: Option<&str>) -> Finding {
    no_node_accepted_it(
        &now(),
        &unplaceable(),
        &tainted(machines, key, value, "NoSchedule"),
    )
    .expect("a pod nothing scheduled draws a card")
}

/// **N6 never tells the reader to tolerate a taint Kubernetes manages, and every row is a key the
/// reader can actually hit** (NOTES § D81). On a single-node cluster — kind, minikube, k3s, Docker
/// Desktop, which is who this tool is for — a `kubectl cordon` and a deploy is all it takes, and
/// the old wording answered *"add a toleration for node.kubernetes.io/unschedulable"* when the
/// answer is `kubectl uncordon`. Two are worse than useless: `unreachable` asked the reader to
/// schedule onto a dead machine while N1 drew *"this node has stopped responding"* on the same
/// screen, and `ToBeDeletedByClusterAutoscaler` is a taint this same file calls *an operation in
/// progress* in N2.
///
/// **The list is read off the constants, not transcribed**, so a row added without a sentence is
/// caught here rather than shipped; and each key is required to reach *its own* answer, because a
/// table that translated every taint into one sentence would pass a test that only asked whether
/// the raw key was gone.
#[test]
fn a_taint_kubernetes_manages_is_translated_and_never_offered_as_a_toleration() {
    let machine = captured_nodes()
        .into_iter()
        .next()
        .expect("the capture has a node")
        .id
        .name;
    let answers: [(&str, &str); 11] = [
        ("node.kubernetes.io/unschedulable", "kubectl uncordon"),
        ("node.kubernetes.io/not-ready", "check that machine first"),
        ("node.kubernetes.io/unreachable", "check that machine first"),
        ("node.kubernetes.io/memory-pressure", "free up memory"),
        ("node.kubernetes.io/disk-pressure", "free up disk space"),
        ("node.kubernetes.io/pid-pressure", "so many processes"),
        ("node.kubernetes.io/network-unavailable", "network plugin"),
        (
            "node.cloudprovider.kubernetes.io/uninitialized",
            "finish joining",
        ),
        ("karpenter.sh/unregistered", "finish joining"),
        ("ToBeDeletedByClusterAutoscaler", "replacement machine"),
        ("karpenter.sh/disrupted", "replacement machine"),
    ];
    let managed: Vec<&str> = MANAGED_TAINTS
        .iter()
        .map(|&(k, _, _)| k)
        .chain(SCALE_DOWN_TAINTS)
        .collect();
    assert_eq!(
        managed,
        answers.iter().map(|&(k, _)| k).collect::<Vec<_>>(),
        "the table plus the two autoscaler taints — read off the constants, so a row added \
         without an answer below cannot ship quietly"
    );

    for (key, must_say) in answers {
        let card = n6_card(key, None);
        println!("\n{key}\n  {}\n  → {}", card.evidence, card.action);
        assert!(
            !card.action.contains("add a toleration"),
            "{key} is written by the node controller and removed by it — tolerating it is never \
             the answer, and for `unreachable` it is advice to schedule onto a dead machine: {}",
            card.action
        );
        assert!(
            !card.evidence.contains(key) && !card.action.contains(key),
            "and the raw key never reaches the screen — `{key}` printed bare is \
             `CrashLoopBackOff` printed and left (invariant 14): {} / {}",
            card.evidence,
            card.action
        );
        assert!(
            card.action.contains(must_say),
            "{key} needs the answer that actually clears it, and no other row's: {}",
            card.action
        );
        assert!(
            card.evidence.contains(&machine),
            "a translation is not a suppression: the card still says which machine ({machine}): \
             {}",
            card.evidence
        );
        // **No row promises a card that may not be on the screen.** N1 waits five minutes and
        // these taints wait not at all, so a runtime that dies at 03:02 and a deploy at 03:03
        // would have sent the reader hunting a node card that arrives at 03:07 (D81).
        assert!(
            !card.action.contains("on this screen") && !card.action.contains("card"),
            "{key}: the evidence has already named the machine, and a pointer at a card that \
             is not drawn yet is worse than no pointer: {}",
            card.action
        );
        // And no token survives into the sentence a user reads.
        assert!(
            !card.action.contains('{') && !card.action.contains('}'),
            "{key} printed an unsubstituted token: {}",
            card.action
        );
    }
}

/// **The one machine this table exists for, and the several it also has to serve.** The evidence
/// line has inflected since it was written; six of the eleven actions said *"those machines"*
/// whatever the count, on a table whose primary case is a one-node kind or minikube cluster
/// (NOTES § D81).
///
/// **And the `uncordon` line has to run as typed.** `(kubectl uncordon)` with no node is the only
/// command in this file that errors out when pasted, on a product whose pitch is *without
/// memorising long kubectl commands* (invariant 4).
#[test]
fn the_managed_actions_say_one_machine_when_there_is_one_and_name_it_in_the_command() {
    let names: Vec<String> = captured_nodes()
        .into_iter()
        .take(2)
        .map(|n| n.id.name)
        .collect();
    assert_eq!(names.len(), 2, "the capture has more than one node");

    for (key, singular, plural) in [
        (
            "node.kubernetes.io/unschedulable",
            format!(
                "allow new pods on that machine again once the work is done (kubectl uncordon {})",
                names[0]
            ),
            format!(
                "allow new pods on those machines again once the work is done (kubectl uncordon {} {})",
                names[0], names[1]
            ),
        ),
        (
            "node.kubernetes.io/disk-pressure",
            "free up disk space on that machine, or add another machine to the cluster".to_string(),
            "free up disk space on those machines, or add another machine to the cluster"
                .to_string(),
        ),
        (
            "node.kubernetes.io/not-ready",
            "check that machine first — this pod is placed on its own once a machine is ready \
             again"
                .to_string(),
            "check those machines first — this pod is placed on its own once a machine is ready \
             again"
                .to_string(),
        ),
    ] {
        let one = n6_card_on(1, key, None);
        let two = n6_card_on(2, key, None);
        println!("\n{key}\n  one:  {}\n  two:  {}", one.action, two.action);
        assert_eq!(one.action, singular, "{key}, on one machine");
        assert_eq!(two.action, plural, "{key}, on two");
    }

    // The command with its argument, checked as a command rather than as a string: what is
    // printed is what `kubectl uncordon` takes, one node or several.
    let card = n6_card_on(2, "node.kubernetes.io/unschedulable", None);
    let (_, command) = card
        .action
        .split_once("(kubectl uncordon ")
        .expect("the action carries the command");
    let argument = command.trim_end_matches(')');
    assert_eq!(
        argument.split_whitespace().collect::<Vec<_>>(),
        names.iter().map(String::as_str).collect::<Vec<_>>(),
        "every machine the card is about, and `kubectl uncordon` takes them all: {argument:?}"
    );
}

/// **The negative half, and the key the table deliberately leaves out.** A suppression broad
/// enough to swallow `node-role.kubernetes.io/control-plane` — the single-node kubeadm case the
/// toleration wording was written for — passes every positive above. The last two are keys that
/// merely *look* managed: the table is matched whole, never by prefix.
#[test]
fn an_operators_own_taint_still_says_add_a_toleration() {
    for (key, value, named) in [
        (
            "node-role.kubernetes.io/control-plane",
            None,
            "node-role.kubernetes.io/control-plane",
        ),
        ("dedicated", Some("gpu"), "dedicated=gpu"),
        (
            "node.kubernetes.io/unschedulable-by-us",
            None,
            "node.kubernetes.io/unschedulable-by-us",
        ),
        (
            "karpenter.sh/unregistered-ish",
            None,
            "karpenter.sh/unregistered-ish",
        ),
    ] {
        let card = n6_card(key, value);
        println!("\n{key}\n  {}\n  → {}", card.evidence, card.action);
        assert_eq!(
            managed_taint(key),
            None,
            "{key} is somebody's own taint and must not be in the managed table"
        );
        assert!(
            card.evidence.contains(named) && card.action.contains(named),
            "{key} is a taint a human at this cluster applied, so the card names it and offers \
             the two things kubectl accepts: {} / {}",
            card.evidence,
            card.action
        );
        assert!(
            card.action.contains("add a toleration"),
            "{key}: {}",
            card.action
        );
    }
}

/// The table itself: every row a real key, no duplicates, and no sentence that reads wrong after
/// the machine names the card puts in front of it.
#[test]
fn the_managed_taint_table_is_well_formed() {
    let keys: Vec<&str> = MANAGED_TAINTS.iter().map(|&(k, _, _)| k).collect();
    assert_eq!(
        keys.iter().collect::<BTreeSet<_>>().len(),
        keys.len(),
        "a duplicated row: {keys:?}"
    );
    for &(key, means, action) in &MANAGED_TAINTS {
        assert!(
            !SCALE_DOWN_TAINTS.contains(&key),
            "{key} is in both tables, so which sentence wins depends on the lookup order"
        );
        assert!(
            !means.is_empty() && !action.is_empty() && !means.contains(key),
            "{key} translates to nothing, or to itself: {means:?} / {action:?}"
        );
        assert!(
            !means.starts_with("is ") && !means.starts_with("are "),
            "{key}'s sentence carries its own verb and will read `node-1 is is …`: {means}"
        );
        // The three pressure rows used to end *"…on a machine that is"* — legal ellipsis, and the
        // only sentence in the table that stops mid-clause, which at a glance reads truncated
        // (NOTES § D81).
        assert!(
            !means.ends_with(" is") && !means.ends_with(" are") && !means.ends_with(" has"),
            "{key}'s sentence ends on a stranded verb and reads as if it were cut off: {means}"
        );
    }
}

/// **A pod the drain has already evicted is not work the drain still has to do** — upstream's
/// `skipDeletedFilter`, and the same false positive D43 killed for autoscalers arriving from the
/// other side: counting it puts the card on a drain that is *running* (NOTES § D81).
#[test]
fn n2_does_not_count_a_pod_the_drain_has_already_evicted() {
    let terminating = pod("stuck");
    assert!(
        terminating.deletion_timestamp.is_some(),
        "`stuck.json` is the captured pod somebody asked to shut down, which is what a drain \
         leaves behind while it waits"
    );
    // The machine is the one the capture put the pod on — the join has to close, and which
    // worker the scheduler picked moves on every trip.
    let machine = terminating
        .node
        .clone()
        .expect("the captured pod names the machine it is terminating on");
    let draining = node_but(&machine, |n| {
        n.spec
            .as_mut()
            .expect("a captured node has a spec")
            .unschedulable = Some(true);
    });

    let all = analyze(&cluster(vec![terminating.clone()], vec![draining.clone()]));
    show(&all);
    assert!(
        !all.iter().any(|f| f.title.contains("refuses new pods")),
        "one pod, already terminating: that is a drain in flight, not one that stopped half \
         way: {:?}",
        titles(&all)
    );

    // And the pod beside it that a drain has *not* reached is still counted, so the silence
    // above is the filter and not an empty join. Its neighbour is read out of the capture for
    // the reason the machine is.
    let neighbour = a_pod_a_drain_would_move_on(&machine);
    println!("beside it on {machine}: {}", neighbour.id.name);
    let all = analyze(&cluster(vec![terminating, neighbour], vec![draining]));
    show(&all);
    let card = only(&all, &machine, "refuses new pods");
    assert_eq!(
        card.evidence, "1 pod here would still have to move",
        "one of the two, and the count is what `kubectl drain` would actually still move"
    );
}

/// **A pod that finished is charged to nobody and alarms about nothing** — [`finished`], which
/// gates both `analyze`'s pod rules and [`pods_on`], so N1's list, N2's movable count and N5's sum
/// all run through it. Deleting it left the suite green (NOTES § D81): the plant that existed was
/// on a *healthy* capture, where skipping and not skipping produce the same silence.
///
/// **Captured, not planted.** This was a phase written onto a decoded copy for as long as no
/// committed object was over; the 2026-08-13 trip brought both — `succeeded.json` is a pod whose
/// container ran to `exit 0` after three failed attempts, `failed.json` one that never got there
/// and carries `exit 137` beside four restarts. Both keep their `nodeName`, which is the whole
/// reason [`finished`] exists, and both are loud enough underneath to draw two cards apiece the
/// moment their phase says they are still running — which is the control below.
#[test]
fn a_pod_that_finished_is_charged_to_nobody_and_alarms_about_nothing() {
    for name in ["succeeded", "failed"] {
        let done = pod(name);
        let phase = done
            .phase
            .clone()
            .expect("the capture says which way this pod ended");
        assert!(
            finished(&done),
            "{name}.json is a pod that is over, whatever it did on the way: {phase}"
        );

        // **The control, and it is the same object.** A restart count and a failed previous run
        // are what rules 5 and 6 read, and both captures carry them — so the same bytes with the
        // phase moved back to `Running` are loud, and the silence below is the skip rather than a
        // pod nothing was ever wrong with. Deleting `finished` left the suite green once already
        // (NOTES § D81) because the plant that stood here was on a *healthy* capture.
        let still_running = capture_but(name, |p| {
            p.status
                .as_mut()
                .expect("a captured pod has a status")
                .phase = Some("Running".to_string());
        });
        let noisy = analyze(&pods_at(vec![still_running], now()));
        show(&noisy);
        assert!(
            noisy.len() >= 2,
            "{name}.json draws cards while it is running, or the silence below proves nothing: \
             {:?}",
            titles(&noisy)
        );

        nothing(
            &analyze(&pods_at(vec![done.clone()], now())),
            format!(
                "a {phase} pod's restart counts and last exits belong to the Waste report, not \
                 to the screen that holds what is broken now (D2)"
            )
            .as_str(),
        );

        // The node half: it keeps its `nodeName`, and neither the drain count nor the node's
        // own pod list may include it. Which machine that is belongs to the scheduler, so the
        // cordon is applied to whichever node the capture names.
        let machine = done.node.clone().expect(
            "a finished pod keeps the node it ran on, which is the whole reason this \
                     filter is needed",
        );
        let cordoned = node_but(&machine, |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .unschedulable = Some(true);
        });
        let alone = analyze(&cluster(vec![done.clone()], vec![cordoned.clone()]));
        show(&alone);
        assert!(
            !alone.iter().any(|f| f.title.contains("refuses new pods")),
            "a drain moves nothing off a node whose only pod is {phase}: {:?}",
            titles(&alone)
        );

        // Beside a live pod, the count is one — so the silence above is the filter and not an
        // empty join, and N1's total is the same number from the other rule. The neighbour is
        // picked by [`a_drain_would_move`] so it cannot be a pod that is already terminating,
        // which the same filter skips for its own reason — one exclusion at a time, or neither
        // is being tested.
        let live = a_pod_a_drain_would_move_on(&machine);
        println!("beside it on {machine}: {}", live.id.name);
        let both = cluster(vec![done, live], vec![cordoned]);
        let all = analyze(&both);
        assert_eq!(
            only(&all, &machine, "refuses new pods").evidence,
            "1 pod here would still have to move",
            "one of the two: {phase} is not work a drain has left to do"
        );
        let down = node_but(&machine, |n| {
            node_condition_mut(n, "Ready").status = "Unknown".to_string();
        });
        let all = analyze(&ClusterSnapshot {
            nodes: vec![down],
            ..both
        });
        assert!(
            only(&all, &machine, "stopped responding")
                .evidence
                .contains("(1 pod)"),
            "and N1 counts what was running there, which a {phase} pod was not: {}",
            only(&all, &machine, "stopped responding").evidence
        );
    }
}

/// **The node whose labels the pod actually accepts is the one whose taints are read.** N6 filters
/// the machines by `nodeSelector` before it looks for a blocking taint, and nothing exercised that
/// filter: the captured Pending pod asks for a label no node has, so the rule answers before it
/// reaches the filter, and the managed-taint tests clear the selector, which makes `.all()`
/// vacuously true either way — inverting the comparison survived the whole suite (NOTES § D81).
///
/// So: two machines, each with a taint of its own, and only one of them labelled the way the pod
/// asks. Naming the other machine's taint is the bug, and the two cards read differently.
#[test]
fn n6_reads_the_taints_of_the_machines_the_pod_would_accept_and_not_the_others() {
    let wanted = ("disktype", "ssd");
    let candidate = node_but("k8rs-worker2", |n| {
        n.metadata
            .labels
            .get_or_insert_with(Default::default)
            .insert(wanted.0.to_string(), wanted.1.to_string());
    });
    let elsewhere = node_but("k8rs-worker", |n| {
        n.spec.as_mut().expect("a captured node has a spec").taints = Some(vec![ApiTaint {
            key: "dedicated".to_string(),
            value: Some("cpu".to_string()),
            effect: "NoSchedule".to_string(),
            time_added: None,
        }]);
    });
    assert!(
        !elsewhere.labels.contains_key(wanted.0),
        "the second machine must not carry the label, or both are candidates and the filter is \
         not being tested"
    );
    assert!(
        candidate
            .taints
            .iter()
            .any(|t| t.key == "dedicated" && t.value.as_deref() == Some("gpu")),
        "the candidate keeps the operator's own captured taint, which is the string the card \
         has to name"
    );

    let asking = capture_but("pending", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        spec.tolerations = None;
        spec.node_selector = Some(
            [(wanted.0.to_string(), wanted.1.to_string())]
                .into_iter()
                .collect(),
        );
    });
    let all = analyze(&cluster(vec![asking], vec![candidate, elsewhere]));
    show(&all);

    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        card.evidence
            .contains("k8rs-worker2 is tainted dedicated=gpu"),
        "the machine the pod's own nodeSelector accepts is the one whose taint is refusing it: {}",
        card.evidence
    );
    assert!(
        !card.evidence.contains("k8rs-worker ") && !card.evidence.contains("dedicated=cpu"),
        "and the machine the pod never asked for contributes nothing — reading its taints \
         instead names a fix that changes nothing: {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        "add a toleration for dedicated=gpu, or remove the taint"
    );
}

// --- THE WORKLOAD RULES, AGAINST THE COMMITTED CAPTURES ---
//
// The W-series is the one family whose subject is not a pod, and both positives come off the
// same kind cluster as everything else: `scripts/cluster.sh` puts a `deny-all-pods`
// ResourceQuota in `k8rs-quota`, so `broken-quota`'s ReplicaSet was refused every pod it
// asked for (W1) and its Deployment then timed out waiting for them (W2). Both conditions
// are in the committed captures, `ProgressDeadlineExceeded` included.
//
// **What no capture holds is the quiet half of W2.** Its suppression clause needs a
// Deployment that timed out *and* has a failing pod under it — and on this cluster the one
// Deployment that timed out has no pods at all, which is the entire point of it. Those go
// through the one-field-on-a-decoded-copy technique the rest of this file uses (NOTES § D40,
// § D53), and each names why the 2026-08-13 trip could not bring the object.
//
// **The negatives are the half that matters again**, and for a new reason: W2's gate is a
// *query over the findings already made*, so it can fail in two opposite directions — a
// second card for one problem, or a Deployment that goes silent because something unrelated
// fired. Both directions are asserted below, with the link deliberately cut in one of them.

/// Every Deployment in `deployments.json`, decoded — six workloads, of which exactly one has
/// given up.
fn captured_deployments() -> Vec<WorkloadSnapshot> {
    items::<Deployment>("deployments")
        .into_iter()
        .map(Into::into)
        .collect()
}

/// One committed ReplicaSet capture, decoded. `just fixtures` writes each of these as a
/// `kubectl get -A` List, so they all arrive through [`items`].
fn captured_replicasets(name: &str) -> Vec<WorkloadSnapshot> {
    items::<ReplicaSet>(name)
        .into_iter()
        .map(Into::into)
        .collect()
}

/// One captured ReplicaSet with fields moved — [`capture_but`]'s counterpart for the object W1 is
/// about. Every one of these captures is a `kubectl get -A` List whose first item is the one the
/// W tests are written around. The committed JSON is never touched (NOTES § D53).
fn replicaset_but(name: &str, edit: impl FnOnce(&mut ReplicaSet)) -> ReplicaSet {
    let mut object: ReplicaSet = serde_json::from_value(fixture(name)["items"][0].clone())
        .unwrap_or_else(|e| panic!("{name}.json's first item is not a ReplicaSet: {e}"));
    edit(&mut object);
    object
}

/// One Deployment out of `deployments.json` with one field moved — [`capture_but`]'s
/// counterpart for the object W2 is about. The committed JSON is never touched (NOTES § D53).
fn deployment_but(name: &str, edit: impl FnOnce(&mut Deployment)) -> WorkloadSnapshot {
    let mut object: Deployment =
        serde_json::from_value(captured_item(&fixture("deployments"), name).clone())
            .unwrap_or_else(|e| panic!("{name} is not a Deployment in deployments.json: {e}"));
    edit(&mut object);
    WorkloadSnapshot::from(object)
}

/// The `ReplicaFailure` entry of a captured ReplicaSet, to be written through — the one condition
/// W1 reads, and the one every `quota-replicasets` copy below moves.
fn replicaset_failure(r: &mut ReplicaSet) -> &mut ReplicaSetCondition {
    r.status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == "ReplicaFailure")
        .expect("the capture carries a ReplicaFailure condition")
}

/// One condition of a captured Deployment, to be written through — [`pod_condition`]'s twin.
fn deployment_condition<'a>(d: &'a mut Deployment, type_: &str) -> &'a mut DeploymentCondition {
    d.status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition to edit"))
}

/// **The `Progressing` condition a Deployment carries once its deadline has passed**, written onto
/// a decoded copy.
///
/// **Status, reason, message and stamp move together, and that is not tidiness.** Upstream writes
/// all four in one `NewDeploymentCondition` call, so moving only the reason produces a card that
/// says a rollout gave up and then quotes a controller saying it is still progressing — a shape no
/// cluster emits, and a test built on it proves nothing about the real one. The first draft of
/// these tests did exactly that, and only reading the printed card showed it.
///
/// **`lastTransitionTime` is the fourth, and it was the one left behind.** `Finding::timestamp`
/// reads that field, so a copy that kept the old stamp dated a dead rollout by the minute it was
/// last *progressing* — D69's wrong-field shape one step removed.
///
/// All four are `deployments.json`'s own, and the wording is asserted here so the synthesis cannot
/// drift from what the cluster actually writes (NOTES § D40, § D53).
fn timed_out(d: &mut Deployment, replicaset: &str) {
    let deployments = fixture("deployments");
    let captured = captured_condition(captured_item(&deployments, "broken-quota"), "Progressing");
    let wording = captured_str(captured, &["message"]);
    assert!(
        wording.ends_with("has timed out progressing."),
        "the wording below is the capture's, not this test's — and the capture now says {wording:?}"
    );
    let stamp = captured_time(captured, &["lastTransitionTime"]);
    let c = deployment_condition(d, "Progressing");
    c.status = "False".to_string();
    c.reason = Some("ProgressDeadlineExceeded".to_string());
    c.message = Some(format!(
        "ReplicaSet \"{replicaset}\" has timed out progressing."
    ));
    c.last_transition_time = Some(stamp);
}

/// **A captured pod hung off `broken-owned`'s ReplicaSet** — the one field moved for every
/// suppression case below, because this cluster's other pods have no controller and W2's
/// suppression is a walk up the chain from one (NOTES § D53).
///
/// **The uid is the captured ReplicaSet's**, not a fresh one: [`ObjectId`] compares on identity,
/// so a synthesized uid leaves the chain unwalkable and the suppression under test unreachable —
/// the test would then pass because nothing *could* suppress rather than because nothing should.
fn adopted_by_broken_owned(p: &mut Pod) {
    p.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "broken-owned-7bdb7645c8".to_string(),
        uid: captured_str(
            &fixture("owned-replicasets")["items"][0],
            &["metadata", "uid"],
        )
        .to_string(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    }]);
}

/// [`pods_at`] with the workload list filled in — the snapshot a W rule is actually handed.
fn with_workloads(pods: Vec<PodSnapshot>, workloads: Vec<WorkloadSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        workloads,
        ..pods_at(pods, now())
    }
}

/// **W1's positive, and the copy of its own condition that must not become a second card.**
///
/// The Deployment controller mirrors `ReplicaFailure` up onto the Deployment — the identical
/// sentence is on `broken-quota` in `deployments.json` — so the kind gate in the rule is
/// load-bearing, and this is the assertion that would notice if it were dropped.
#[test]
fn the_replicaset_that_was_refused_says_so_in_the_api_servers_own_words() {
    let mut everything = captured_replicasets("quota-replicasets");
    everything.extend(captured_replicasets("healthy-replicasets"));
    everything.extend(captured_deployments());
    let all = analyze(&with_workloads(Vec::new(), everything));
    show(&all);

    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.object.kind,
        ObjectKind::ReplicaSet,
        "what the rule looked at is the ReplicaSet, which is the object that was refused"
    );
    assert_eq!(card.owner.kind, ObjectKind::Deployment);
    assert_eq!(
        card.owner.name, "broken-quota",
        "and it files under the name the user deployed, never the hashed one (D28)"
    );
    assert_eq!(
        card.severity,
        Severity::Critical,
        "not one pod of this workload exists, so this is an outage and not a change that \
         failed to land"
    );

    let raw = fixture("quota-replicasets");
    let failure = captured_condition(&raw["items"][0], "ReplicaFailure");
    assert!(
        card.evidence.contains(captured_str(failure, &["message"])),
        "verbatim means verbatim (D37): the quota's own refusal is the whole diagnosis, and \
         a paraphrase of it sends the reader looking for a broken image — {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("exceeded quota: deny-all-pods"),
        "and the message the capture still carries has to be the one that names the quota: {}",
        card.evidence
    );
    assert!(
        card.evidence.starts_with("0 of 1 pod ready"),
        "and the card carries a count, because a dot with no number cannot tell *all three \
         refused* from *one of three* — sourced from the Deployment the header names, which \
         is the same object the band was read off (D82): {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(failure, &["lastTransitionTime"])),
        "the age is that condition's own stamp — `Available` sits one entry away in the \
         same flat list (D69)"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl get replicaset broken-quota-59654c756 -n k8rs-quota -o yaml"),
        "`kubectl describe rs` prints Type/Status/Reason and drops the message this whole \
         card is made of, so the teaching command is the one that shows it (D46, D71)"
    );

    assert_eq!(
        all.len(),
        1,
        "one refusal, one card: the healthy ReplicaSet carries no such condition, the \
         Deployment carries a mirrored copy of the failing one and must not draw a second, \
         and the rollout that timed out because of it is W2's suppressed case: {:?}",
        titles(&all)
    );
}

/// **W1's other band — the refused *rollout*, where the ReplicaSet's own counters are a
/// service that is 100% up** (NOTES § D82).
///
/// A refused rollout leaves the **new** ReplicaSet reading `0 of 1` for as long as the refusal
/// stands, while the **old** one carries every request — and there are no pods under the new
/// one, so nothing else on the screen contradicts a CRITICAL drawn off it. A band read from the
/// refused object pages for a service that never went down, which is the card that teaches a
/// user to mute the tool in week one.
///
/// **The ReplicaSet here is the committed capture, untouched**: `spec.replicas: 1`,
/// `status.replicas: 0`, no `readyReplicas` at all. Only the Deployment above it is moved, and
/// as one coherent story — three wanted, two of them still answering, the third refused
/// **The 2026-08-13 trip did not bring the other half, and this is why:** `scripts/broken.yaml`'s
/// quota denies every pod in `k8rs-quota` from the moment the namespace exists, so nothing there
/// is ever running to be scaled *up*. That shape needs a quota tightened under a Deployment that
/// is already serving, which is a second sequence rather than a second manifest.
///
/// The control is the same ReplicaSet under the captured Deployment, which really is down.
#[test]
fn a_refused_rollout_is_amber_while_the_deployment_above_it_is_still_serving() {
    let refused = captured_replicasets("quota-replicasets");
    let raw = fixture("quota-replicasets");
    assert_eq!(
        raw["items"][0]["status"]["readyReplicas"],
        serde_json::Value::Null,
        "the capture has to still be the shape this test is about — the refused ReplicaSet \
         reports no ready pods of its own"
    );

    let still_up = deployment_but("broken-quota", |d| {
        d.spec
            .as_mut()
            .expect("a captured Deployment has a spec")
            .replicas = Some(3);
        d.status
            .as_mut()
            .expect("a captured Deployment has a status")
            .ready_replicas = Some(2);
    });
    let all = analyze(&with_workloads(
        Vec::new(),
        refused.iter().cloned().chain([still_up]).collect(),
    ));
    show(&all);

    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "two of the three pods are still answering, and they answer under the Deployment \
         rather than under the ReplicaSet that was refused — a card that pages for this is a \
         card nobody reads twice (D82)"
    );
    assert!(
        card.evidence.starts_with("2 of 3 pods ready"),
        "and the count is that same Deployment's, not the refused ReplicaSet's `0 of 1` — \
         one object per line, and it is the object in the header (D82): {}",
        card.evidence
    );
    assert_eq!(
        all.len(),
        1,
        "and the Deployment's own timeout is still the same refusal, not a second card: {:?}",
        titles(&all)
    );

    // The control, and the half that makes the assertion above able to fail: the captured
    // Deployment reports nothing ready at all, and the identical ReplicaSet then pages.
    let down = analyze(&with_workloads(
        Vec::new(),
        refused
            .into_iter()
            .chain(
                captured_deployments()
                    .into_iter()
                    .filter(|w| w.id.name == "broken-quota"),
            )
            .collect(),
    ));
    show(&down);
    assert_eq!(
        only(&down, "broken-quota-59654c756", "refused to create").severity,
        Severity::Critical,
        "nothing of this workload is serving, so the same ReplicaSet is an outage"
    );
}

/// **An owner that is named and absent is *unknown*, not *down*** (NOTES § D82).
///
/// An Argo `Rollout` owns its ReplicaSets **directly**, with no Deployment in between, and
/// invariant 12 forbids decoding a CR — so [`ObjectKind::from_api`] resolves that owner to an
/// `Other(_)` which nothing will ever put in `snapshot.workloads`. A canary ReplicaSet a quota
/// refused, while the stable ReplicaSet beside it serves 100% of the traffic, is then a lookup
/// that misses. Falling back to the ReplicaSet the rule looked at reads the `readyReplicas: 0` it
/// has **by definition of having been refused**, and pages CRITICAL for a service that never went
/// down — with no pods under that ReplicaSet, so nothing else on the screen contradicts it. A 403
/// on `deployments` with `replicasets` still readable is the second way in, and it needs no CRD at
/// all.
///
/// **The 2026-08-13 trip did not bring it, and no trip will:** an owner k8rs does not decode means
/// a third-party controller installed on the fixture cluster, which is a cluster change rather
/// than a fixture — the argument
/// `the_api_group_decides_which_kind_an_owner_reference_names` already records against
/// OpenKruise. So the reference is moved on a decoded copy (NOTES § D53).
///
/// The control is the same untouched capture under an owner the snapshot *does* carry, which
/// really is down — the half that makes the band above able to fail.
#[test]
fn a_refusal_under_an_owner_the_snapshot_cannot_name_is_not_called_an_outage() {
    let canary = replicaset_but("quota-replicasets", |r| {
        r.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "argoproj.io/v1alpha1".to_string(),
            kind: "Rollout".to_string(),
            name: "broken-quota".to_string(),
            uid: "b7e41f02-9c6d-4a18-8f3b-5d2ac70e1946".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);
    });
    let canary = WorkloadSnapshot::from(canary);
    assert_eq!(
        canary.owner.kind,
        ObjectKind::Other("Rollout.argoproj.io".to_string()),
        "the copy only proves something while the owner really is a kind no watch decodes \
         and no snapshot can carry"
    );
    assert!(
        canary.ready.unwrap_or(0) == 0 && canary.desired == Some(1),
        "and while the ReplicaSet the rule looked at really does read `0 of 1` — which is \
         the number a fallback would page on"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![canary]));
    show(&all);
    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the stable version is serving every request and this tool cannot see it — an \
         unresolvable owner is unknown, and unknown is not an outage (D82)"
    );
    assert!(
        !card.evidence.contains(" ready"),
        "and with no resolvable workload there is no count to print: a number about the \
         wrong object is worse than none: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("exceeded quota: deny-all-pods"),
        "while the refusal itself, which is the whole diagnosis, still reaches the card: {}",
        card.evidence
    );

    // The control — the same ReplicaSet under the owner it was really captured with, which
    // has nothing ready at all.
    let down = analyze(&with_workloads(
        Vec::new(),
        captured_replicasets("quota-replicasets")
            .into_iter()
            .chain(
                captured_deployments()
                    .into_iter()
                    .filter(|w| w.id.name == "broken-quota"),
            )
            .collect(),
    ));
    show(&down);
    assert_eq!(
        only(&down, "broken-quota-59654c756", "refused to create").severity,
        Severity::Critical,
        "an owner the snapshot *can* name, with nothing serving under it, is the outage"
    );
}

/// **A condition with no message leaves no blank row** — the one branch of [`controller_said`]
/// that no capture reaches.
///
/// Both W rules quote a condition and both fire on conditions upstream always writes a message
/// with, so this branch exists for a server that does not. `message` is `+optional` on
/// `ReplicaSetCondition`, so the object below is one the API allows; what it must not produce is
/// an evidence line that is the empty string, which draws as a blank row under the title and
/// reads as a card that failed to load rather than one with nothing to quote.
#[test]
fn a_condition_with_nothing_to_quote_still_says_something() {
    let object = replicaset_but("quota-replicasets", |r| {
        replicaset_failure(r).message = None;
    });

    let all = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&all);
    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert!(
        !card.evidence.trim().is_empty(),
        "an empty evidence line is a blank row on the card, and a reader cannot tell it from \
         a card that failed to draw"
    );
    assert!(
        !card.evidence.contains("the reason Kubernetes gave"),
        "and it may not open a quote it has nothing to put in: {}",
        card.evidence
    );
}

/// **`FailedDelete` is not W1's condition, and the filter that keeps it out is a ruling**
/// (NOTES § D82).
///
/// Upstream's `replica_set_utils.go` writes `ReplicaFailure` under two reasons. The other one is
/// a scale-**down** the API refused — a webhook on `DELETE pods` whose backend is unreachable,
/// which is Gatekeeper, Kyverno or any admission controller having a bad day. Every sentence on
/// this card is wrong about it: nothing was refused creation, the service is up, and the counters
/// would read *"2 of 1 pod"*. **No card in v1**; a card for it is a new rule (invariant 13).
///
/// **The 2026-08-13 trip did not bring it, and this is why:** a refused scale-down needs a
/// validating admission webhook, which is a second workload deployed into the fixture cluster for
/// no purpose but to reject one request — a cluster change rather than a fixture. So the whole
/// story is written onto a decoded copy at once — reason, message and both counters
/// (NOTES § D53). They move **together** because upstream writes them together: a
/// creation refusal's message left standing under a deletion's reason is a shape no cluster
/// emits, and a test built on it proves nothing about the real one.
///
/// **The control is the committed capture, untouched**, and it comes first — a W1 that had
/// stopped firing at all would otherwise pass this test by saying nothing about everything.
#[test]
fn a_scale_down_the_api_refused_is_not_a_card_about_creating_pods() {
    let capture = replicaset_but("quota-replicasets", |_| {});
    let created = analyze(&with_workloads(Vec::new(), vec![capture.into()]));
    show(&created);
    only(&created, "broken-quota-59654c756", "refused to create");

    // The scale-down the API refused: one wanted, two still running, and the delete of the
    // second one rejected by a webhook whose backend is down.
    let object = replicaset_but("quota-replicasets", |r| {
        r.spec
            .as_mut()
            .expect("a captured ReplicaSet has a spec")
            .replicas = Some(1);
        let status = r
            .status
            .as_mut()
            .expect("a captured ReplicaSet has a status");
        status.ready_replicas = Some(2);
        status.replicas = 2;
        let c = replicaset_failure(r);
        c.reason = Some("FailedDelete".to_string());
        c.message = Some(
            "unable to delete pods: Internal error occurred: failed calling webhook \
             \"deny.example.com\": dial tcp 10.96.0.9:443: connect: connection refused"
                .to_string(),
        );
    });

    let deleted = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&deleted);
    nothing(
        &deleted,
        "a scale-down the API refused is not a card titled \"refused to create the pods\", and \
         it is not one reading \"2 of 1 pod\" either — the service is up and v1 says nothing",
    );
}

/// **W2's positive and every negative the capture can offer**, over the Deployments alone —
/// no ReplicaSets and no pods, so nothing is there to suppress anything and each Deployment
/// answers for itself.
///
/// **The negative that answers the question the box left open** is `broken-owned`: it is
/// short of pods — one wanted, none ready — and its `Progressing` never reached the
/// deadline, so it says nothing at all. Every `kubectl apply` is short of pods for a while,
/// and `progressDeadlineSeconds` is Kubernetes' own answer to when that stops being normal;
/// a third rule reading the shortfall on its own would fire on every rollout in progress.
#[test]
fn the_rollout_that_ran_out_of_time_is_the_only_deployment_that_says_anything() {
    let deployments = captured_deployments();
    let all = analyze(&with_workloads(Vec::new(), deployments.clone()));
    show(&all);

    let card = only(&all, "broken-quota", "gave up");
    assert_eq!(card.object.kind, ObjectKind::Deployment);
    assert_eq!(
        card.owner, card.object,
        "nothing controls a Deployment, so it is its own card"
    );
    assert_eq!(card.severity, Severity::Critical);

    let raw = fixture("deployments");
    let quota = captured_item(&raw, "broken-quota");
    let progressing = captured_condition(quota, "Progressing");
    assert!(
        card.evidence
            .contains(captured_str(progressing, &["message"])),
        "the controller's own sentence, verbatim (D37) — it names the revision that timed \
         out, which is where the reader goes next: {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(progressing, &["lastTransitionTime"])),
        "**the wrong stamp is one entry away and it draws**: this Deployment's \
         `ReplicaFailure` transitioned a minute earlier and `Available` earlier still (D69)"
    );
    assert_ne!(
        card.timestamp,
        Some(captured_time(
            captured_condition(quota, "ReplicaFailure"),
            &["lastTransitionTime"]
        )),
        "and the capture is what makes that assertion mean something: the two stamps differ"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl get deployment broken-quota -n k8rs-quota -o yaml"),
        "`kubectl describe deployment` reduces its conditions to Type/Status/Reason, and \
         prints `available` where this card counts `ready`"
    );
    assert!(
        card.evidence.starts_with("0 of 1 pod ready"),
        "**the counter has to follow the band** (D82): this Deployment is on its first \
         revision — `observedGeneration: 1`, no old version anywhere — so a red card reading \
         `0 of 1 pod on the new version` says an old one is still up, which is the opposite \
         triage decision at 3am: {}",
        card.evidence
    );

    let owned = deployments
        .iter()
        .find(|w| w.id.name == "broken-owned")
        .expect("the capture holds a Deployment that is short of pods and has not given up");
    println!(
        "broken-owned: desired={:?} ready={:?} progressing={:?}",
        owned.desired,
        owned.ready,
        condition(&owned.conditions, "Progressing").and_then(|c| c.reason.as_deref())
    );
    assert!(
        short_of_pods(owned),
        "the negative only proves something while this Deployment is genuinely short"
    );
    assert_eq!(
        all.len(),
        1,
        "one Deployment gave up and five did not — a rollout still running, one short of \
         pods that never reached its deadline, and three that are simply fine: {:?}",
        titles(&all)
    );
}

/// **W2 stands down when W1 has already said why** — the two cards that would otherwise
/// describe one refusal, one naming the quota and one naming the clock it ran out.
///
/// The control comes first and is the half that makes this test able to fail: alone, the
/// same Deployment draws the card.
#[test]
fn the_rollout_card_stands_down_when_the_replicaset_has_already_said_why() {
    let object: Deployment = serde_json::from_value(fixture("quota-deployment"))
        .expect("quota-deployment.json is a Deployment");
    let deployment = WorkloadSnapshot::from(object);

    let alone = analyze(&with_workloads(Vec::new(), vec![deployment.clone()]));
    show(&alone);
    only(&alone, "broken-quota", "gave up");

    let both: Vec<WorkloadSnapshot> = captured_replicasets("quota-replicasets")
        .into_iter()
        .chain([deployment])
        .collect();
    let all = analyze(&with_workloads(Vec::new(), both));
    show(&all);
    only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        all.len(),
        1,
        "the refusal is on the list, so the timeout it caused is not a second card — two \
         findings for one problem is how the list stops being believable (D28): {:?}",
        titles(&all)
    );
}

/// **The chain D28 describes, walked: a pod's crash loop explains its Deployment's rollout,
/// two steps up.** The pod's owner is a ReplicaSet and W2's subject is the Deployment above
/// it, so the link runs through `WorkloadSnapshot::owner` on the ReplicaSet — which is why a
/// snapshot that is missing the ReplicaSet cannot walk it, and the third case below is what
/// that does.
///
/// **The 2026-08-13 trip did not bring it, and `deployments.json` says why.** `broken-owned` ran
/// for over an hour with its only pod in `CrashLoopBackOff`, and its `Progressing` condition still
/// reads `True / NewReplicaSetAvailable` beside `Available: False` — the deadline never trips once
/// the ReplicaSet has managed to *create* its pod, whatever that pod then does. So `broken-owned`
/// is that Deployment in every respect but that one reason, and it is moved on a decoded copy
/// (NOTES § D40, § D53).
#[test]
fn a_crash_loop_two_steps_under_the_deployment_is_the_same_problem_and_not_a_second_card() {
    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let pods: Vec<PodSnapshot> = items::<Pod>("owned-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let replicaset = captured_replicasets("owned-replicasets");
    let chain: Vec<WorkloadSnapshot> = replicaset
        .iter()
        .cloned()
        .chain([gave_up.clone()])
        .collect();

    let name = owned_pod_name();
    let whole = analyze(&with_workloads(pods.clone(), chain.clone()));
    show(&whole);
    only(&whole, &name, "CrashLoopBackOff");
    assert!(
        whole.iter().all(|f| !f.title.contains("gave up")),
        "the pod under it already says why the rollout never finished: {:?}",
        titles(&whole)
    );

    // Control one — the same three objects minus the pod. Nothing explains the shortfall, so
    // the card is drawn, which is what makes the silence above a suppression rather than a
    // rule that never fires.
    let no_pods = analyze(&with_workloads(Vec::new(), chain));
    show(&no_pods);
    only(&no_pods, "broken-owned", "gave up");

    // Control two — the pod is there and its ReplicaSet is not, so the chain cannot be
    // walked. **W2 draws, and that is the direction this is built to fail in**: an owner that
    // cannot be resolved is unknown, not related, and reading unknown as related would let
    // any pod's crash loop anywhere in the snapshot silence every W2 in it.
    let no_link = analyze(&with_workloads(pods, vec![gave_up]));
    show(&no_link);
    only(&no_link, "broken-owned", "gave up");
    only(&no_link, &name, "CrashLoopBackOff");
}

/// **A card about a pod that is answering requests does not explain why a rollout has none**
/// (NOTES § D28, § D82).
///
/// D28's clause is *"no pod-level finding already **explains** the shortfall"*, and every finding
/// under the owner is not that list. `restarts.json` is a pod that is **Running and Ready** with
/// three restarts, and rule 5's own sentence about it says *"it is serving now"* — so a W2 it
/// silenced would be a Critical hidden behind a card that says nothing is wrong. Rule 8's
/// hostPath card, rule 12's and rule 14's are the same shape.
///
/// **The two directions are asserted against the same Deployment**, so the difference between
/// them is the pod and nothing else. **The 2026-08-13 trip did not bring it:** a rollout only
/// times out while its pods are *failing*, so a serving pod under a dead rollout is a pair the
/// cluster will not hold at once — this cluster's serving pod has no controller at all, and the
/// `ownerReference` is the one field moved (NOTES § D53).
#[test]
fn a_pod_that_is_serving_does_not_explain_a_rollout_that_has_no_pods() {
    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let replicaset = captured_replicasets("owned-replicasets");
    let chain: Vec<WorkloadSnapshot> = replicaset.iter().cloned().chain([gave_up]).collect();

    let serving = capture_but("restarts", adopted_by_broken_owned);
    assert!(
        serving.ready.as_ref().is_some_and(|c| c.status == "True"),
        "the whole test is that this pod is answering requests while it restarts"
    );
    // **Without this the serving half passes for the wrong reason.** A synthesized owner whose
    // uid did not match the captured ReplicaSet's would leave the chain unwalkable, and W2 would
    // then draw because nothing *could* suppress it rather than because nothing should.
    assert_eq!(
        serving.owner, replicaset[0].id,
        "the adopted pod has to actually hang off the ReplicaSet under the Deployment, or the \
         suppression this test is about was never reachable"
    );
    let with_serving = analyze(&with_workloads(vec![serving], chain.clone()));
    show(&with_serving);
    only(&with_serving, "broken-restarts", "restarted 3 times");
    only(&with_serving, "broken-owned", "gave up");

    // The other direction, and the one D28 is actually about: a pod that is *not* serving is
    // the reason the rollout has no pods, and the two cards would be one problem.
    let failing: Vec<PodSnapshot> = items::<Pod>("owned-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let with_failing = analyze(&with_workloads(failing, chain));
    show(&with_failing);
    only(&with_failing, &owned_pod_name(), "CrashLoopBackOff");
    assert!(
        with_failing.iter().all(|f| !f.title.contains("gave up")),
        "a pod that is not ready is exactly the shortfall, so this stays one card: {:?}",
        titles(&with_failing)
    );
}

/// **The third shape `explains_a_shortfall` has to answer for: a pod with no `Ready` condition at
/// all** (NOTES § D29, § D82).
///
/// The two cases beside this one feed the other two framings — `owned-pods` is `Ready: False` and
/// `restarts` is `Ready: True` — so the arm that reads a pod carrying no such condition was only
/// ever asserted by reading the source. `broken-pending` is that pod: no node accepted it, so the
/// kubelet never wrote a `Ready` line at all, and `pending.json`'s decode is asserted `None`
/// elsewhere in this file. It is also the shape that matters most rather than the leftover — an
/// unschedulable pod is the most common true explanation of a rollout that never finished.
///
/// **The 2026-08-13 trip did not bring it:** `broken-pending` is refused by a `nodeSelector` no
/// node carries, and `scripts/broken.yaml` puts that selector on a bare pod rather than under a
/// Deployment — so this cluster's unschedulable pod has no controller at all, and the
/// `ownerReference` is the one field moved ([`adopted_by_broken_owned`], NOTES § D53).
///
/// The control is the same chain with no pod, which draws the card.
#[test]
fn a_pod_no_node_would_take_explains_the_rollout_that_is_waiting_for_it() {
    let waiting = capture_but("pending", adopted_by_broken_owned);
    assert_eq!(
        waiting.ready, None,
        "the framing this test exists for: not `Ready: False` but no Ready condition at all, \
         which is what the kubelet writes for a pod it never saw"
    );
    let replicaset = captured_replicasets("owned-replicasets");
    assert_eq!(
        waiting.owner, replicaset[0].id,
        "and the adopted pod has to really hang off the ReplicaSet under the Deployment, or \
         the suppression this test is about was never reachable"
    );

    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let chain: Vec<WorkloadSnapshot> = replicaset.into_iter().chain([gave_up]).collect();

    let all = analyze(&with_workloads(vec![waiting], chain.clone()));
    show(&all);
    only(&all, "broken-pending", "will take this pod");
    assert!(
        all.iter().all(|f| !f.title.contains("gave up")),
        "the pod nothing would schedule *is* the shortfall, so this is one card and not two: \
         {:?}",
        titles(&all)
    );

    // The control — the same Deployment and ReplicaSet with no pod under them. Nothing
    // explains the shortfall, so the card is drawn, which is what makes the silence above a
    // suppression rather than a rule that never fires.
    let alone = analyze(&with_workloads(Vec::new(), chain));
    show(&alone);
    only(&alone, "broken-owned", "gave up");
}

/// **The hop up the chain is one hop off a ReplicaSet, never off whatever workload was found**
/// (NOTES § D82).
///
/// A Deployment that carries a controlling `ownerReference` is ordinary — ECK, OLM and most
/// operators that emit Deployments set one — and a hop taken off it lands on the operator's CR.
/// W1's finding then resolves to the CR while W2's Deployment resolves to itself, the suppression
/// stops matching, and one quota refusal draws two Criticals whose second card is headed with a
/// name its own `$ kubectl` line does not mention.
///
/// **The 2026-08-13 trip did not bring it, and no trip will:** a Deployment emitted by an operator
/// means a third-party controller installed on the fixture cluster, which is a cluster change
/// rather than a fixture. So the reference is added on a decoded copy (NOTES § D53).
#[test]
fn a_deployment_an_operator_owns_is_still_where_the_chain_stops() {
    let owned = deployment_but("broken-quota", |d| {
        d.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "example.com/v1".to_string(),
            kind: "TheOperatorsKind".to_string(),
            name: "the-operators-cr".to_string(),
            uid: "0f5b1c88-7a3e-42d1-9c0b-6e2f8a41d3b7".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);
    });
    assert_ne!(
        owned.owner, owned.id,
        "the copy only proves something while the Deployment really is owned by something else"
    );

    let all = analyze(&with_workloads(
        Vec::new(),
        captured_replicasets("quota-replicasets")
            .into_iter()
            .chain([owned])
            .collect(),
    ));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one quota refusal is one card whether or not an operator owns the Deployment — a \
         second one would be headed `the-operators-cr` while the command under it names \
         `broken-quota`: {:?}",
        titles(&all)
    );
    only(&all, "broken-quota-59654c756", "refused to create");
}

/// **W2's amber band, and the shortfall `readyReplicas` cannot see** (NOTES § D82).
///
/// Whenever `maxUnavailable` resolves to **0** a RollingUpdate Deployment never removes its old
/// pods, so `status.readyReplicas` stays equal to `spec.replicas` for the whole of a failed
/// rollout. `ready < desired` is false there, on a Deployment whose own condition says it gave up,
/// which made this band unreachable in practice and W2 silent on the most common failed rollout
/// there is.
///
/// **`scripts/broken.yaml` sets it to 0 outright, and that is what this fixture exercises** — not
/// the 25% default rounding down to the same 0 at one, two or three replicas, which reaches this
/// arithmetic by the other road. The two are worth keeping apart: the default is why the hole is
/// common, the explicit setting is why it is in the capture.
///
/// **The counters here are the committed capture's, every one of them**: `spec.replicas: 2`,
/// `maxUnavailable: 0`, `readyReplicas: 2`, `replicas: 3`, `updatedReplicas: 1`. Only the
/// `Progressing` condition is moved, because `broken-rollout` was captured before its deadline
/// expired — one field, the one no cluster held still long enough to capture (NOTES § D40, § D53).
#[test]
fn a_rollout_stuck_behind_maxunavailable_zero_still_reports_its_shortfall() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-rollout");
    println!(
        "broken-rollout capture: spec.replicas={} maxUnavailable={} status={}",
        raw["spec"]["replicas"],
        raw["spec"]["strategy"]["rollingUpdate"]["maxUnavailable"],
        raw["status"]
    );
    assert_eq!(
        raw["status"]["readyReplicas"], raw["spec"]["replicas"],
        "this test is only about something while the capture is the shape that defeats \
         `ready < desired` — every pod the Deployment asked for is ready and the rollout is \
         still stuck"
    );

    let stalled = deployment_but("broken-rollout", |d| {
        timed_out(d, "broken-rollout-5967d47d5b");
    });
    let all = analyze(&with_workloads(Vec::new(), vec![stalled]));
    show(&all);

    let card = only(&all, "broken-rollout", "gave up");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the two old pods are still answering: the change did not land, the service did \
         not go down"
    );
    assert!(
        card.evidence.starts_with("1 of 2 pods on the new version"),
        "and the counter on the card is the one that is actually short — `2 of 2 ready` is \
         true here and says nothing at all about a rollout that is dead: {}",
        card.evidence
    );

    // The same stuck rollout once the old pods have gone too: nothing is ready, one pod
    // exists on the new version. **The band flips and the counter has to flip with it** — a
    // red card reading `1 of 2 pods on the new version` says an old version is still serving
    // and leaves the number that justifies the band off the screen entirely (D82).
    let down = deployment_but("broken-rollout", |d| {
        d.status
            .as_mut()
            .expect("a captured Deployment has a status")
            .ready_replicas = Some(0);
        timed_out(d, "broken-rollout-5967d47d5b");
    });
    assert_eq!(
        (down.ready, down.updated, down.desired),
        (Some(0), Some(1), Some(2)),
        "one field moved, and the other two are the capture's"
    );
    let all = analyze(&with_workloads(Vec::new(), vec![down]));
    show(&all);
    let card = only(&all, "broken-rollout", "gave up");
    assert_eq!(card.severity, Severity::Critical);
    assert!(
        card.evidence.starts_with("0 of 2 pods ready"),
        "the number under a red band is the one the red band was read off: {}",
        card.evidence
    );
}

/// **The rollout of *one* replica, which is the size the other two counters are both blind to**
/// (NOTES § D82).
///
/// At `replicas: 1` upstream's `ResolveFenceposts` gives `maxSurge: 1, maxUnavailable: 0`: the new
/// ReplicaSet is scaled to exactly one and the old one is left at one. A second revision that
/// cannot start therefore reads `spec.replicas 1 · readyReplicas 1 · updatedReplicas 1` — the old
/// pod answers, one pod exists on the new template, and both of W2's original counters say the
/// workload is whole while its own condition says it gave up. This is the commonest Deployment
/// size there is.
///
/// **The counters here are `broken-owned`'s own, and it is captured with the two that matter**:
/// `spec.replicas: 1`, `updatedReplicas: 1`, `unavailableReplicas: 1`. Two are moved — the old
/// pod's `readyReplicas`, and the `status.replicas: 2` that goes with a surge of one, because a
/// capture where one pod exists and one is ready and one is unavailable is not a shape any cluster
/// emits (NOTES § D40, § D53). **The 2026-08-13 trip did not bring it:** `broken-rollout` is the
/// mid-rollout Deployment `scripts/broken.yaml` grows, and it runs two replicas — the one-replica
/// variant, where the surge and the shortfall land on the same single pod, is a manifest the file
/// does not have.
///
/// **The mitigation, so this is not read as bigger than it is:** in the ordinary version of this
/// shape the new pod has a card of its own — a bad image, a crash loop — and `explained` suppresses
/// W2 anyway. The hole is the one where the new pod produces no card at all, and it is narrow.
#[test]
fn a_rollout_of_one_replica_is_short_where_both_other_counters_read_whole() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-owned");
    println!(
        "broken-owned capture: spec={} status={}",
        raw["spec"], raw["status"]
    );
    assert_eq!(
        (
            raw["spec"]["replicas"].as_i64(),
            raw["status"]["updatedReplicas"].as_i64(),
            raw["status"]["unavailableReplicas"].as_i64(),
        ),
        (Some(1), Some(1), Some(1)),
        "three of the four counters are the capture's, and the test is only about something \
         while they are"
    );

    let stuck = deployment_but("broken-owned", |d| {
        let status = d
            .status
            .as_mut()
            .expect("a captured Deployment has a status");
        // The old pod, still answering — the half a first-revision capture cannot hold.
        status.ready_replicas = Some(1);
        // ...and the two pods that exist while it does, which is what a surge of one is.
        status.replicas = Some(2);
        timed_out(d, "broken-owned-7bdb7645c8");
    });
    assert_eq!(
        (stuck.desired, stuck.ready, stuck.updated, stuck.unavailable),
        (Some(1), Some(1), Some(1), Some(1)),
        "the shape this test is about: every counter W2 had before this one reads whole"
    );
    assert!(
        stuck.ready >= stuck.desired && stuck.updated >= stuck.desired,
        "neither of the two original arms of `short_of_pods` can see this rollout"
    );
    assert!(
        short_of_pods(&stuck),
        "and the third one has to, or W2 is silent on a dead rollout of the commonest \
         Deployment size there is (D82)"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![stuck]));
    show(&all);
    let card = only(&all, "broken-owned", "gave up");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the old pod is still answering: the change did not land, the service did not go down"
    );
    assert!(
        card.evidence.starts_with("1 pod not answering"),
        "and the counter on the card is the only one that is short — `1 of 1 ready` and \
         `1 of 1 on the new version` are both true here and both say a rollout that is dead \
         has finished (D82): {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(
            captured_condition(captured_item(&deployments, "broken-quota"), "Progressing"),
            &["lastTransitionTime"]
        )),
        "the card is dated by the moment the rollout gave up — the stamp that arrived with \
         the reason and the message, not the one this Deployment carried while it was still \
         progressing (D69)"
    );
    assert_ne!(
        card.timestamp,
        Some(captured_time(
            captured_condition(raw, "Progressing"),
            &["lastTransitionTime"]
        )),
        "and the capture is what makes that assertion mean something: the two stamps differ"
    );
}

/// **A workload that wants zero pods is not short of pods** (NOTES § D82).
///
/// Scaling a rollout that gave up down to zero is how someone stops the bleeding, and it leaves the
/// object in the one shape [`short_of_pods`]' third arm cannot reason its way out of.
/// `spec.replicas` is written by the user and `status.unavailableReplicas` by the controller, as
/// `sum(replicaset.spec.replicas) - availableReplicas` — never the Deployment's own
/// `spec.replicas` — so the watch delivers an explicit `0` beside the status of a moment ago, and
/// `ProgressDeadlineExceeded` is sticky enough to still be sitting on it. Before the `desired > 0`
/// gate that was a CRITICAL card reading *"1 pod not answering"* about a Deployment the user had
/// just deliberately turned off.
///
/// **One field moved on a decoded copy.** `broken-owned` is captured as the whole shape but for the
/// replica count — `unavailableReplicas: 1`, no `readyReplicas` at all — so only `spec.replicas`
/// moves, plus the `Progressing` condition every W2 test has to write (NOTES § D40, § D53).
/// **The 2026-08-13 trip did not bring it, and a trip cannot:** the shape exists only between the
/// `scale` call and the controller writing the status back, so capturing it means winning a race
/// against a controller — not something `just fixtures` can be asked to do repeatably.
#[test]
fn a_deployment_scaled_to_zero_is_not_short_of_the_pods_it_no_longer_wants() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-owned");
    assert_eq!(
        (
            raw["status"]["unavailableReplicas"].as_i64(),
            raw["status"]["readyReplicas"].as_i64(),
        ),
        (Some(1), None),
        "the capture carries the half that matters — a positive unavailable counter and no \
         ready one — and this test is only about something while it does"
    );

    let switched_off = deployment_but("broken-owned", |d| {
        d.spec
            .as_mut()
            .expect("a captured Deployment has a spec")
            .replicas = Some(0);
        timed_out(d, "broken-owned-7bdb7645c8");
    });
    println!(
        "scaled to zero: desired={:?} ready={:?} updated={:?} unavailable={:?}",
        switched_off.desired, switched_off.ready, switched_off.updated, switched_off.unavailable
    );
    assert_eq!(
        (switched_off.desired, switched_off.unavailable),
        (Some(0), Some(1)),
        "an explicit zero beside a status that has not caught up: the coexistence the doc \
         comment on `short_of_pods` used to promise was impossible"
    );
    assert!(
        !short_of_pods(&switched_off),
        "a workload that wants no pods cannot be short of them — and `unavailable > 0` is the \
         one arm that does not learn that from the arithmetic (D82)"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![switched_off]));
    show(&all);
    nothing(
        &all,
        "a Deployment the user has just deliberately turned off is not an outage, and a red \
         card about the pod it is in the middle of stopping is the screen crying wolf",
    );
}

/// **A ReplicaSet is not short of pods for having no `updatedReplicas`** (NOTES § D82).
///
/// [`short_of_pods`] is a general-looking helper on the shared [`WorkloadSnapshot`], and W2's kind
/// gate is the only thing keeping it off a ReplicaSet today — `analysis.rs` is the next file up and
/// takes the same type. Read a ReplicaSet's absent `updatedReplicas` as `unwrap_or(0)` and *the
/// question does not apply* becomes *none of them are on the current version*, which is true of
/// every healthy ReplicaSet alive. Its required `status.replicas` is the answer: a ReplicaSet is
/// one template, so every pod it has is a pod on it.
///
/// **Both buckets are asserted non-empty**, or a capture set that drifted to all-healthy or
/// all-broken would leave half of this passing by testing nothing.
#[test]
fn a_replicaset_is_short_of_pods_only_when_it_actually_is() {
    let mut short = Vec::new();
    let mut whole = Vec::new();
    for name in [
        "quota-replicasets",
        "healthy-replicasets",
        "rollout-replicasets",
        "owned-replicasets",
    ] {
        for rs in captured_replicasets(name) {
            println!(
                "{}: desired={:?} ready={:?} updated={:?} short={}",
                rs.id.name,
                rs.desired,
                rs.ready,
                rs.updated,
                short_of_pods(&rs)
            );
            assert_eq!(
                short_of_pods(&rs),
                rs.ready.unwrap_or(0) < rs.desired.unwrap_or(1),
                "a ReplicaSet is short exactly when fewer pods are passing their probes than \
                 it was told to run, and readiness is the only arm this kind has — {} \
                 disagrees",
                rs.id.name
            );
            if short_of_pods(&rs) {
                &mut short
            } else {
                &mut whole
            }
            .push(rs.id.name.clone());
        }
    }
    println!("short={short:?} whole={whole:?}");
    assert!(
        short.contains(&"broken-quota-59654c756".to_string()),
        "the refused ReplicaSet has `status.replicas: 0` against a desired 1 and has to stay \
         short, or this test passes by finding nothing: {short:?}"
    );
    assert!(
        whole.contains(&"healthy-deploy-7f84bdfb9b".to_string()),
        "and the healthy one — two of two ready, two on its one template — has to stay \
         whole: {whole:?}"
    );
    assert!(
        short.contains(&"broken-owned-7bdb7645c8".to_string()),
        "and the crash-looping one is what keeps the readiness arm from being redundant: \
         its one pod **exists**, so `updated` equals `desired` and no other arm sees it — \
         `unavailable` is a counter this kind does not have (D82): {short:?}"
    );
}

/// **W2's action names no command, because there is no command it can promise.**
///
/// `kubectl rollout undo` errors on a **single-revision** Deployment — *"no rollout history
/// found"* — which is `broken-quota`, the shipped fixture this very card is drawn on
/// (`observedGeneration: 1`, no pod ever created). On a **paused** Deployment it errors
/// differently: *"you cannot rollback a paused deployment"*. The contract carries neither
/// `spec.paused` nor a revision count, so an action line naming the command is a card telling
/// the reader to run something that will fail in front of them (NOTES § D82).
#[test]
fn the_rollout_card_promises_no_command_it_cannot_know_will_run() {
    let object: Deployment = serde_json::from_value(fixture("quota-deployment"))
        .expect("quota-deployment.json is a Deployment");
    let raw = fixture("quota-deployment");
    assert_eq!(
        raw["status"]["observedGeneration"], 1,
        "the fixture this card is drawn on is on its first revision, so `rollout undo` has \
         nothing to go back to"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&all);
    let card = only(&all, "broken-quota", "gave up");
    assert!(
        !card.action.contains("kubectl"),
        "the action may not name a command this card cannot know will run: {}",
        card.action
    );
    assert!(
        !card.action.contains("nothing else on this screen"),
        "nor may it claim the rest of the screen is silent — the suppression it is standing \
         on is narrower than that (D82): {}",
        card.action
    );
}

/// **The whole committed capture with its workloads joined on** — every pod in both
/// namespaces, every node, every Deployment and every ReplicaSet, through [`analyze`] at
/// once. `cargo test -- --nocapture` prints what a user would actually read.
#[test]
fn the_whole_capture_including_its_workloads_through_the_rules_at_once() {
    let pods: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .chain(
            items::<Pod>("owned-pods")
                .into_iter()
                .map(PodSnapshot::from),
        )
        .collect();
    let workloads: Vec<WorkloadSnapshot> = captured_deployments()
        .into_iter()
        .chain(captured_replicasets("quota-replicasets"))
        .chain(captured_replicasets("healthy-replicasets"))
        .chain(captured_replicasets("rollout-replicasets"))
        .chain(captured_replicasets("owned-replicasets"))
        .collect();
    let all = analyze(&ClusterSnapshot {
        workloads,
        ..cluster(pods, captured_nodes())
    });
    show(&all);

    let workload_cards: Vec<(&str, &str)> = all
        .iter()
        .filter(|f| {
            matches!(
                f.object.kind,
                ObjectKind::ReplicaSet | ObjectKind::Deployment
            )
        })
        .map(|f| (f.object.name.as_str(), f.title.as_str()))
        .collect();
    println!("{workload_cards:#?}");
    assert_eq!(
        workload_cards.len(),
        1,
        "over the whole capture the W-series has exactly one thing to say — the quota \
         refused `broken-quota`'s pods — and the rollout that timed out because of it is \
         the same problem, not a second card: {workload_cards:?}"
    );
    assert_eq!(workload_cards[0].0, "broken-quota-59654c756");

    for f in &all {
        assert_ne!(f.severity, Severity::Info, "D2: {}", f.title);
        let cmd = f
            .kubectl_cmd
            .as_deref()
            .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
        assert!(
            cmd.contains(&f.object.name),
            "invariant 4's teaching device points at the object the card is about: {cmd}"
        );
    }
}

// --- C1, THE ONE CARD ABOUT THE READER'S OWN MACHINE ---
//
// Three committed certificates, whose dates are pinned and asserted by
// `scripts/certs-test.sh` against the same instant [`now`] spells — 22 days left, 363
// days left, 5 days past. That script also refuses to let this file and itself disagree
// about that instant, so the numbers below are guarded rather than transcribed.
//
// **What no committed fixture can reach is built from bytes here**, and the reason is
// not convenience: `tests/fixtures/certs/` is a closed set — `certs-test.sh` fails on
// any file it does not know the dates of, and `scripts/make-certs.sh` is deliberately
// not wired into `just fixtures` — so a fourth certificate cannot be committed without
// reopening a Phase 2 box. RFC 5280's `99991231235959Z` is not a shape openssl writes
// for you either.

/// The snapshot C1 is handed: a kubeconfig and a moment, and no cluster at all. That is
/// the rule's whole character — it reads a file on the user's machine, so it is the one
/// rule that still answers when every other one has nothing to look at.
fn kubeconfig(context: Option<&str>, certificate: Option<Vec<u8>>) -> ClusterSnapshot {
    ClusterSnapshot {
        context: context.map(str::to_string),
        client_certificate: certificate,
        ..pods_at(Vec::new(), now())
    }
}

/// A PEM block around DER, **encoded with `k8s-openapi`'s own base64** — `ByteString`
/// serialises that way because that is how Secret data travels over the API.
///
/// A hand-written encoder here would be a second implementation whose **failure mode is
/// green**: bad base64 makes the parse fail, which is the same "no finding" three of the
/// tests below assert. Borrowing an encoder the dependency tree already tests is what
/// keeps `certificate_expiring_at`'s positive control able to fail.
fn pem_block(label: &str, der: &[u8]) -> Vec<u8> {
    let quoted = serde_json::to_string(&k8s_openapi::ByteString(der.to_vec()))
        .expect("a byte string serialises to base64");
    let body = quoted.trim_matches('"');
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n").into_bytes()
}

/// One DER `TAG LENGTH VALUE`, short form only — every field of the certificate below is
/// far under 128 bytes, and the assertion is what says so out loud rather than writing a
/// long-form encoder nothing here needs.
fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let length = u8::try_from(body.len())
        .expect("every field of the minimal certificate is under 128 bytes");
    let mut out = vec![tag, length];
    out.extend_from_slice(body);
    out
}

/// **The smallest certificate a parser accepts, with one interesting field** — its
/// `notAfter`, handed in whole as an ASN.1 `TAG LENGTH VALUE` so that the two cases below
/// differ in nothing but the digits.
///
/// Everything else is the minimum RFC 5280 structure: a serial, an algorithm, two empty
/// names, and a public key nobody parses. It is **not a fixture and may not become one**
/// (see the section note above).
fn certificate_expiring_at(not_after: &[u8]) -> Vec<u8> {
    // sha256WithRSAEncryption and rsaEncryption — any two OIDs would do; these are what
    // a real client certificate carries, so the shape is one a parser has seen.
    let sha256_rsa = tlv(
        0x06,
        &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b],
    );
    let rsa = tlv(
        0x06,
        &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01],
    );
    let null = tlv(0x05, &[]);
    let algorithm = tlv(0x30, &[sha256_rsa, null.clone()].concat());
    // An empty RDNSequence: this certificate has no subject and no issuer, and C1 reads
    // neither — the only string on its card that came off a file is the context name.
    let name = tlv(0x30, &[]);
    let validity = tlv(
        0x30,
        &[tlv(0x17, b"260101000000Z"), not_after.to_vec()].concat(),
    );
    let key = tlv(
        0x30,
        &[
            tlv(0x30, &[rsa, null].concat()),
            tlv(0x03, &[0x00, 0x30, 0x03, 0x02, 0x01, 0x00]),
        ]
        .concat(),
    );
    let tbs = tlv(
        0x30,
        &[
            tlv(0x02, &[0x01]),
            algorithm.clone(),
            name.clone(),
            validity,
            name,
            key,
        ]
        .concat(),
    );
    tlv(0x30, &[tbs, algorithm, tlv(0x03, &[0x00])].concat())
}

/// The one card C1 draws, or a failure naming what came out instead. C1 produces at most
/// one finding, so "it fired" and "it fired twice" cannot print the same green line.
fn only_card(all: &[Finding]) -> &Finding {
    assert_eq!(
        all.len(),
        1,
        "C1 draws exactly one card and nothing else is in this snapshot: {:?}",
        titles(all)
    );
    &all[0]
}

/// **C1's positive, on the committed certificate whose dates `certs-test.sh` pins.**
///
/// The identity is the assertion that carries the most: C1 is the one finding with no API
/// object behind it, so `Other("kubeconfig")` / no namespace / the context name / **no
/// uid** is the whole of what a later dialog has to work with (NOTES § D39, § D51).
#[test]
fn the_kubeconfig_certificate_inside_the_window_says_how_long_the_reader_has() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("expiring-client")),
    ));
    show(&all);
    let card = only_card(&all);

    assert_eq!(
        card.severity,
        Severity::Info,
        "the band is the routing, not the volume: `Info` is what sends the expiring half to \
         the Certificates report D2 put it in, and tidying it back to `Warn` would draw a C1 \
         card in Alerts that no screen spec has (NOTES § D87)"
    );
    assert!(
        card.title.contains("22 days"),
        "the certificate has 22 days left at the pinned `now` and the card says so — \
         `scripts/certs-test.sh` asserts that number against the committed bytes: {}",
        card.title
    );
    assert!(
        card.evidence.contains("2026-09-05T00:00:00Z"),
        "the evidence proves the title with the date the reader can put in a calendar: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("your own machine"),
        "the one card whose subject is not the cluster has to say so, or the reader goes \
         looking for the broken pod: {}",
        card.evidence
    );
    assert_eq!(
        card.owner,
        ObjectId {
            kind: ObjectKind::Other("kubeconfig".to_string()),
            namespace: None,
            name: "kind-k8rs".to_string(),
            uid: None,
        },
        "the card is filed under the context name the user recognises, cluster-scoped, \
         with the only `None` uid in the product"
    );
    assert_eq!(
        card.owner, card.object,
        "there is no controller above a file"
    );
    assert_eq!(
        card.kubectl_cmd, None,
        "no kubectl command shows a kubeconfig certificate's dates — `config view` prints \
         the path, and `kubeadm certs check-expiration` reads a control-plane node's disk. \
         `None` here means no such command exists (invariant 4)"
    );
    assert_eq!(
        card.timestamp, None,
        "`notAfter` is a deadline, not the moment this card's event happened, and it is \
         the exact field `age`'s future bound exists to refuse (NOTES § D69)"
    );
    assert_eq!(card.age(&now()), None, "so the card draws no age at all");
}

/// C1's negative, and it is the shape most kubeconfigs are actually in.
#[test]
fn a_kubeconfig_certificate_a_year_out_says_nothing() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("healthy-client")),
    ));
    nothing(
        &all,
        "363 days is not news — a rule that speaks here is one whose screen gets ignored",
    );
}

/// **The expired band, which is not the same card.** A certificate that has run out is not
/// *"wrong now, broken soon"*: the reader is locked out of the cluster this second, every
/// other card on the screen is missing because of it, and the tool cannot fix it for them.
#[test]
fn an_expired_kubeconfig_certificate_is_a_failure_and_not_a_warning() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("expired-client")),
    ));
    show(&all);
    let card = only_card(&all);

    assert_eq!(
        card.severity,
        Severity::Critical,
        "and the other band is the other screen: locked out this second is broken-now by D2's \
         own dividing line, so this half alone is an Alerts card — the two differ because they \
         are read in two places, not because one shouts louder (NOTES § D87)"
    );
    assert!(
        card.title.contains("expired 5 days ago"),
        "past the deadline the sentence changes tense and the number is how long ago: {}",
        card.title
    );
    assert!(
        card.evidence.starts_with("was valid until 2026-08-09"),
        "and so does the evidence — `valid until 2026-08-09` on a red card reads as though \
         it still is: {}",
        card.evidence
    );
    assert!(
        card.action.contains("whoever gave you access"),
        "the action names the only person who can fix it, because it is not the cluster \
         and it is not k8rs: {}",
        card.action
    );
    assert_eq!(
        card.timestamp, None,
        "the past-dated half draws no age either — one rule may not put a right edge on \
         one of its bands and a blank on the other"
    );
}

/// **The threshold, which no committed certificate can prove.** 22 days and 363 days sit
/// either side of a wide gap, so both fixtures pass any threshold between them; the clock
/// is the snapshot's field precisely so the same bytes can be read at a chosen moment
/// (invariant 5, NOTES § D18).
///
/// The last two rows are the boundary RFC 5280 §4.1.2.5 sets: a certificate is valid
/// *through* `notAfter`, so the deadline itself is still inside the window.
///
/// **The severity column is which screen, not how loud** (NOTES § D87) — `Info` to the
/// Certificates report the whole way down the window, `Critical` once it is past — so the last
/// two rows are one second and two screens apart.
#[test]
fn thirty_days_is_the_threshold_and_the_deadline_itself_is_not_yet_past() {
    assert_eq!(
        CERT_EXPIRY_WARN,
        SignedDuration::from_hours(30 * 24),
        "NOTES § Certificate rules: C1 warns at 30 days"
    );

    // `expiring-client` runs out at 2026-09-05T00:00:00Z (`scripts/certs-test.sh`), so the
    // first two rows are the day either side of the threshold: 31 days out is silence and
    // 30 days out is the card. Written as 31 days first and caught red — the window is
    // closed at the far end, and a rule that fired a day early would have passed a table
    // that only ever asked about 22 and 363.
    for (moment, expected) in [
        ("2026-08-05T00:00:00Z", None),
        ("2026-08-06T00:00:00Z", Some(("30 days", Severity::Info))),
        (
            "2026-09-04T00:00:01Z",
            Some(("less than a day", Severity::Info)),
        ),
        (
            "2026-09-05T00:00:00Z",
            Some(("less than a day", Severity::Info)),
        ),
        (
            "2026-09-05T00:00:01Z",
            Some(("less than a day", Severity::Critical)),
        ),
    ] {
        let all = analyze(&ClusterSnapshot {
            now: time(moment),
            ..kubeconfig(Some("kind-k8rs"), Some(certificate("expiring-client")))
        });
        let got = all
            .first()
            .map(|f| (f.title.clone(), f.severity, all.len()));
        println!("{moment} -> {got:?}");
        match expected {
            None => nothing(&all, "31 days out is outside the window C1 was given"),
            Some((says, severity)) => {
                let card = only_card(&all);
                assert!(
                    card.title.contains(says),
                    "at {moment} the card should say {says:?}: {}",
                    card.title
                );
                assert_eq!(card.severity, severity, "at {moment}: {}", card.title);
            }
        }
    }
}

/// **A certificate that never expires produces no finding** — RFC 5280 §4.1.2.5's
/// `99991231235959Z`, which is past the end of jiff's `Timestamp` range, so the conversion
/// answers `Err` and a pure rule may not propagate it. The reflex shape is `.unwrap()`,
/// the input is a kubeconfig, and a corporate PKI is exactly where a non-expiring
/// credential turns up — the panic would land on startup (NOTES § D56).
///
/// **The first row is the control, and without it this test cannot fail.** "No finding"
/// is also what a certificate this file built wrong produces, so the same builder, the
/// same tag and the same field length are fed a date inside C1's window first: if that
/// card does not draw, the `9999` row proves nothing about `9999`.
#[test]
fn a_certificate_that_never_expires_draws_no_card_rather_than_panicking() {
    for (not_after, expected) in [
        (b"20260901000000Z", Some("18 days")),
        (b"99991231235959Z", None),
    ] {
        let der = certificate_expiring_at(&tlv(0x18, not_after));
        let all = analyze(&kubeconfig(
            Some("kind-k8rs"),
            Some(pem_block("CERTIFICATE", &der)),
        ));
        show(&all);
        println!(
            "notAfter {} -> {:?}",
            String::from_utf8_lossy(not_after),
            titles(&all)
        );
        match expected {
            Some(says) => assert!(
                only_card(&all).title.contains(says),
                "the control has to draw, or `9999` drawing nothing means nothing: {:?}",
                titles(&all)
            ),
            None => nothing(
                &all,
                "a certificate with no well-defined expiry has no expiry to warn about",
            ),
        }
    }
}

/// **Everything that is not a certificate, fed to the parser.** `rules.rs` returns no
/// `Result`, so a panic in here is a crash of the whole tool — on startup, before the
/// user has pressed anything, from a file they did not know was malformed.
///
/// Four framings, because *"it is not a certificate"* is four different failures: the
/// bytes are not PEM at all, the PEM never ends, the PEM is complete but its contents are
/// not DER, and **the PEM is complete, well-formed and a private key** — the shape that
/// matters most, because the key is the file that sits next to the certificate in the
/// user's `~/.kube` and the one thing that may never be read into our own types.
#[test]
fn malformed_certificate_bytes_produce_no_finding_and_no_panic() {
    let real = certificate("expiring-client");
    let truncated = real[..real.len() / 2].to_vec();
    // A real, parseable certificate wearing the wrong label: the refusal has to be the
    // label, not luck about what the body happens to decode to.
    let mislabelled = pem_block(
        "RSA PRIVATE KEY",
        &certificate_expiring_at(&tlv(0x18, b"20260901000000Z")),
    );

    for (what, bytes) in [
        (
            "not PEM at all",
            b"where did i put that kubeconfig\n".to_vec(),
        ),
        ("a header with a truncated body", truncated),
        (
            "well-formed PEM that is not a certificate",
            pem_block("CERTIFICATE", b"this is not DER"),
        ),
        (
            "a private key wearing a certificate's file name",
            mislabelled,
        ),
        ("empty", Vec::new()),
    ] {
        let all = analyze(&kubeconfig(Some("kind-k8rs"), Some(bytes)));
        println!("{what} -> {:?}", titles(&all));
        nothing(&all, &format!("{what} is not something C1 may report on"));
    }
}

/// **No current context is a real state, not a defensive one** (NOTES § D51): a kubeconfig
/// can name none, and then C1 has no name to file its card under. Inventing one would put
/// a sentence in the field every `kubectl` line is built from.
#[test]
fn a_kubeconfig_with_no_current_context_has_nothing_to_file_the_card_under() {
    let all = analyze(&kubeconfig(None, Some(certificate("expired-client"))));
    nothing(
        &all,
        "a kubeconfig with no current context is one k8rs cannot connect with at all — \
         that screen says so already, and this card would have no name on it",
    );

    let no_certificate = analyze(&kubeconfig(Some("kind-k8rs"), None));
    nothing(
        &no_certificate,
        "a token, an exec plugin or OIDC leaves nothing to parse, and C1 says nothing \
         rather than guessing",
    );
}

/// **C1 beside every other rule, over the whole committed capture** — the card is one more
/// finding on the list, and it is the only one on it that names no API object.
///
/// The pair is the assertion: swapping the certificate for the healthy one removes exactly
/// this card and touches nothing else, which is what says the rule is reading the
/// certificate rather than the cluster it came with.
#[test]
fn the_kubeconfig_card_is_the_only_finding_with_no_object_behind_it() {
    let expiring = analyze(&ClusterSnapshot {
        client_certificate: Some(certificate("expiring-client")),
        ..fixture_snapshot()
    });
    let healthy = analyze(&fixture_snapshot());

    let unidentified: Vec<&str> = expiring
        .iter()
        .filter(|f| f.object.uid.is_none())
        .map(|f| f.title.as_str())
        .collect();
    println!("{unidentified:#?}");
    assert_eq!(
        unidentified.len(),
        1,
        "every other finding names an object the API server can be asked about: \
         {unidentified:?}"
    );
    assert!(unidentified[0].contains("kubeconfig certificate"));

    assert_eq!(
        expiring.len(),
        healthy.len() + 1,
        "the healthy certificate on the same capture draws the same screen minus one \
         card — anything else means C1 moved a finding that is not its own"
    );
    assert!(
        !titles(&healthy).iter().any(|t| t.contains("kubeconfig")),
        "and the card it removed is C1's: {:?}",
        titles(&healthy)
    );
}
