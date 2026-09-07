//! Tests for [`super`] — every navigation and filter case, with no terminal and no cluster in
//! the room, which is the whole of todo.md § Phase 10's done-when.
//!
//! **The fixtures here are constructed, and that is not a breach of NOTES § D53.** That rule is
//! about *captures* — a hand-edited API body, made to say what a test needs. Nothing below is an
//! API body: [`Finding`], [`WorkloadSnapshot`] and [`Browsable`] are k8rs's own types, produced by
//! `rules.rs` and `k8s.rs` from captures that are tested where they are decoded. What this file
//! proves is what `views.rs` does with them once they exist.
//!
//! **The one thing that is therefore unproven here: whether a real cluster's discovery list
//! places the way [`Group::of`] says it does.** There is no committed discovery capture in
//! `tests/fixtures/`, so the kind lists below are transcribed from the API groups by hand.

use super::*;
use crate::analysis::{Jump, Row as ReportRow};
use crate::rules::ObjectKind;
use k8s_openapi::jiff::Timestamp;

// --- BUILDING THE THINGS THE VIEWS ARE MADE OF ---

/// A moment, `n` seconds after the epoch — far enough from it that no ladder rung sees a zero
/// stamp, and ordered by the number so a test can read the sort it expects.
fn at(second: i64) -> Time {
    Time(Timestamp::from_second(1_700_000_000 + second).expect("a representable second"))
}

/// The reader's clock, for the tests that only care that it is **after** every stamp they build —
/// which is every test about folding and ordering. Days ahead of the largest `at` below, so
/// nothing they build is inside `rules::age`'s skew refusal by accident. The tests that are *about*
/// the clock pass their own moment (`a_clock_ahead_of_the_reader_…`).
fn now() -> Time {
    at(1_000_000)
}

fn id(kind: ObjectKind, namespace: Option<&str>, name: &str, uid: Option<&str>) -> ObjectId {
    ObjectId {
        kind,
        namespace: namespace.map(str::to_owned),
        name: name.to_owned(),
        uid: uid.map(str::to_owned),
    }
}

/// One finding, filed under `owner`, about `object`. Only the four fields this file reasons about
/// vary; the three sentences are constant because nothing here draws them.
fn finding(
    severity: Severity,
    owner: ObjectId,
    object: ObjectId,
    timestamp: Option<Time>,
) -> Finding {
    Finding {
        severity,
        title: "something happened".to_owned(),
        evidence: "some evidence".to_owned(),
        action: "do something".to_owned(),
        kubectl_cmd: None,
        owner,
        object,
        timestamp,
    }
}

/// A Deployment `payments/web` and a pod under it, the pair every card in this file is built from.
fn web() -> ObjectId {
    id(ObjectKind::Deployment, Some("payments"), "web", Some("d-1"))
}

fn pod(name: &str, uid: &str) -> ObjectId {
    id(ObjectKind::Pod, Some("payments"), name, Some(uid))
}

/// **`id` and `owner` are handed in separately, and that is the point** (NOTES § D246 ruling 6).
/// Every fixture here used to set `owner: id.clone()`, which made the two indistinguishable to the
/// whole suite — a mutation swapping [`cards`]' lookup from `.id` to `.owner` left all 60 green,
/// while in real data a ReplicaSet's snapshot carries `id: the ReplicaSet, owner: the Deployment`
/// and would lend its own `desired` to its Deployment's card.
fn workload(id: ObjectId, owner: ObjectId, desired: Option<i32>) -> WorkloadSnapshot {
    WorkloadSnapshot {
        id,
        owner,
        desired,
        ready: None,
        updated: None,
        unavailable: None,
        terminating: None,
        conditions: Vec::new(),
    }
}

/// The ordinary case: a workload nothing controls, so it is its own owner.
fn owned_by_itself(id: ObjectId, desired: Option<i32>) -> WorkloadSnapshot {
    workload(id.clone(), id, desired)
}

fn browsable(group: &str, plural: &str, namespaced: bool) -> Browsable {
    Browsable {
        group: group.to_owned(),
        version: "v1".to_owned(),
        kind: plural.to_owned(),
        plural: plural.to_owned(),
        namespaced,
        verbs: vec!["list".to_owned()],
    }
}

// --- LOADING, EMPTY AND DENIED ARE THREE THINGS ---

/// **What this checks is that a reader of a [`Pane`] has to answer for all three, and it checks it
/// at compile time** (PRIOR-ART § C2, NOTES § D246). The `assert_ne!`s the first draft made are a
/// tautology under a derived `PartialEq` — three variants of one enum are never equal, and no
/// change to `views.rs` could make that assertion fail. What *cannot* be written past is the
/// exhaustive `match` below: collapsing `Denied` into `Ready`, or growing a fourth state, stops
/// this file compiling, which is the failure mode a `bool`-per-pane would have shipped silently.
///
/// The `_ => ` arm is deliberately absent for the same reason. The strings are the three
/// `screens/states.md` sentences, so a collapse that *did* compile still has to say the same thing
/// twice to pass.
#[test]
fn every_reader_of_a_pane_has_to_answer_for_all_three_states() {
    fn sentence(pane: &Pane<Vec<u8>>) -> &str {
        match pane {
            Pane::Loading => "reading the cluster…",
            Pane::Denied(said, _) => said,
            Pane::Ready(rows) if rows.is_empty() => "nothing is broken",
            Pane::Ready(_) => "rows",
        }
    }

    let panes = [
        Pane::Loading,
        Pane::Denied(
            "You can't list pods across the whole cluster".to_owned(),
            Vec::new(),
        ),
        Pane::Ready(Vec::new()),
        Pane::Ready(vec![1]),
    ];
    let drawn: Vec<&str> = panes.iter().map(sentence).collect();

    assert_eq!(drawn.len(), 4);
    let mut distinct = drawn.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        4,
        "two pane states drew the same sentence: {drawn:?}"
    );
}

// --- WHAT THE USER TYPES ---

#[test]
fn typing_accumulates_and_backspace_removes_one_character() {
    let mut input = Input::default();
    assert!(input.is_empty());
    for character in "web".chars() {
        input.push(character);
    }
    assert_eq!(input.text(), "web");
    input.pop();
    assert_eq!(input.text(), "we");
    input.clear();
    assert!(input.is_empty(), "clear empties it");
}

/// The security gate's *sizes are bounded* row, on the one string in the file that did not come
/// through `k8s::text`. A bracketed paste is one event and it can be a whole file.
#[test]
fn a_typed_line_stops_at_the_identifier_bound_however_much_is_pasted() {
    let mut input = Input::default();
    for _ in 0..(IDENTIFIER * 4) {
        input.push('a');
    }
    assert_eq!(
        input.text().len(),
        IDENTIFIER,
        "the buffer grew past the bound a stripped identifier is held to"
    );
}

/// **Invariant 9 on the one string that never met `k8s::text`** (NOTES § D246 ruling 5). A
/// bracketed paste is one event and can carry an escape sequence straight into a buffer the
/// renderer draws — `screens/widgets.md` § 7's *an escape sequence in a pod name reaches the
/// terminal and rewrites it*, on the same widget the length bound was written for. The refusal is
/// silent, exactly as the bound's is.
#[test]
fn a_pasted_escape_sequence_leaves_nothing_that_can_rewrite_the_terminal() {
    let mut input = Input::default();
    for character in "\u{1b}[2Jweb".chars() {
        input.push(character);
    }
    assert_eq!(
        input.text(),
        "[2Jweb",
        "an escape character reached a buffer the renderer draws"
    );

    // Every class `k8s::text` removes, refused by the same call rather than a second list: NUL,
    // DEL, a C1 control, a zero-width space and a bidi override.
    let mut every = Input::default();
    for character in "a\u{0}\u{7f}\u{9b}\u{200b}\u{202e}b".chars() {
        every.push(character);
    }
    assert_eq!(
        every.text(),
        "ab",
        "an unprintable character survived into a typed line"
    );

    // A name a delete asks for is still typeable back: `k8s::text` keeps these, so must this.
    let mut ordinary = Input::default();
    for character in "web-7d9f4.öç".chars() {
        ordinary.push(character);
    }
    assert_eq!(
        ordinary.text(),
        "web-7d9f4.öç",
        "the guard refused a character a real object name can contain"
    );
}

/// **Bytes, but never half a character.** A three-byte char at the boundary is refused whole, so
/// the buffer can stop one or two bytes short of the bound and never holds a broken code point.
#[test]
fn a_multi_byte_character_is_refused_whole_at_the_bound_and_popped_whole() {
    let mut input = Input::default();
    for _ in 0..(IDENTIFIER - 1) {
        input.push('a');
    }
    input.push('ü'); // two bytes, and only one byte of room
    assert_eq!(
        input.text().len(),
        IDENTIFIER - 1,
        "a character was cut in half at the bound"
    );

    let mut multi = Input::default();
    multi.push('ü');
    multi.push('ç');
    multi.pop();
    assert_eq!(multi.text(), "ü", "pop took a byte instead of a character");
}

// --- A CURSOR THAT STAYS ON THE SAME OBJECT ---

#[test]
fn an_empty_list_has_no_selection_at_all() {
    let cursor = Cursor::default();
    assert_eq!(cursor.selected(&[]), None);
}

#[test]
fn a_one_item_list_selects_it_and_neither_key_moves_off_it() {
    let rows = [Some("a")];
    let mut cursor = Cursor::default();
    assert_eq!(cursor.selected(&rows), Some(0));
    cursor.up(&rows);
    assert_eq!(cursor.selected(&rows), Some(0), "up wrapped off the top");
    cursor.down(&rows);
    assert_eq!(
        cursor.selected(&rows),
        Some(0),
        "down wrapped off the bottom"
    );
}

#[test]
fn the_keys_move_the_cursor_and_clamp_at_both_ends() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let mut cursor = Cursor::default();
    cursor.down(&rows);
    cursor.down(&rows);
    assert_eq!(cursor.selected(&rows), Some(2));
    cursor.down(&rows);
    assert_eq!(
        cursor.selected(&rows),
        Some(2),
        "the list wrapped to the top"
    );
    cursor.up(&rows);
    cursor.up(&rows);
    cursor.up(&rows);
    assert_eq!(
        cursor.selected(&rows),
        Some(0),
        "the list wrapped to the end"
    );
}

/// The whole reason the anchor is a uid: a re-fetch reorders the rows and the cursor stays on the
/// object the user selected, not on the position it happened to be at.
#[test]
fn a_refetch_that_reorders_the_rows_carries_the_cursor_with_its_object() {
    let before = [Some("a"), Some("b"), Some("c")];
    let mut cursor = Cursor::default();
    cursor.select(1, &before);

    let after = [Some("c"), Some("a"), Some("b")];
    cursor.follow(&after);
    assert_eq!(
        cursor.selected(&after),
        Some(2),
        "the cursor stayed at index 1 while its object moved to 2"
    );
}

/// The negative half of the one above, and it is the one that is easy to get wrong: a *different*
/// object arriving where the selected one was must not attract the cursor, and the cursor must
/// re-anchor on what is now under it rather than keep hunting for a ghost.
#[test]
fn when_the_selected_object_is_gone_the_cursor_holds_position_and_re_anchors() {
    let before = [Some("a"), Some("b"), Some("c")];
    let mut cursor = Cursor::default();
    cursor.select(1, &before);

    let after = [Some("a"), Some("z"), Some("c")];
    cursor.follow(&after);
    assert_eq!(cursor.selected(&after), Some(1), "position was not held");

    // Re-anchored on `z`: a further reorder now follows `z`, not the vanished `b`.
    let later = [Some("z"), Some("a"), Some("c")];
    cursor.follow(&later);
    assert_eq!(
        cursor.selected(&later),
        Some(0),
        "the cursor kept chasing an object that no longer exists"
    );
}

#[test]
fn a_list_that_shrank_under_the_cursor_clamps_instead_of_pointing_past_the_end() {
    let before = [Some("a"), Some("b"), Some("c")];
    let mut cursor = Cursor::default();
    cursor.select(2, &before);

    let after = [Some("x")];
    cursor.follow(&after);
    assert_eq!(cursor.selected(&after), Some(0));

    cursor.follow(&[]);
    assert_eq!(cursor.selected(&[]), None, "an emptied list still selects");
}

/// `?includeObject=None` is a real shape (`tests/fixtures/table-deployments.json`), so a row with
/// nothing to follow has to behave rather than be assumed away.
#[test]
fn rows_with_no_identity_fall_back_to_the_index() {
    let rows = [None, None, None];
    let mut cursor = Cursor::default();
    cursor.select(1, &rows);
    assert_eq!(cursor.selected(&rows), Some(1));
    cursor.follow(&rows);
    assert_eq!(
        cursor.selected(&rows),
        Some(1),
        "an unanchorable cursor lost its place"
    );
}

/// **`↑` and `↓` move from where the highlight is drawn, not from the raw index** (NOTES § D246).
/// A list shrinks under the cursor between a filter keystroke and the [`Cursor::follow`] that
/// answers it — `selected()` clamps for the renderer, the field does not. Measured at index 49
/// against three rows: the first `↑` moved 48 → clamped back to 2, so nothing appeared to happen,
/// and only the second press worked.
#[test]
fn the_keys_move_from_where_the_highlight_is_after_the_list_shrank_underneath() {
    let long: Vec<Option<&str>> = (0..50).map(|_| Some("row")).collect();
    let mut cursor = Cursor::default();
    cursor.select(49, &long);

    let short = [Some("a"), Some("b"), Some("c")];
    cursor.up(&short);
    assert_eq!(
        cursor.selected(&short),
        Some(1),
        "the first press moved nothing the reader could see"
    );

    let mut down = Cursor::default();
    down.select(49, &long);
    down.down(&short);
    assert_eq!(down.selected(&short), Some(2), "`down` from the last row");
}

#[test]
fn selecting_past_the_end_clamps_rather_than_being_refused() {
    let rows = [Some("a"), Some("b")];
    let mut cursor = Cursor::default();
    cursor.select(99, &rows);
    assert_eq!(cursor.selected(&rows), Some(1));
}

// --- ONE CARD PER OWNER ---

#[test]
fn findings_with_no_cards_produce_no_cards() {
    assert!(cards(&[], &[], &now()).is_empty());
}

/// NOTES § D3, the requirement itself: forty findings under one owner are one card.
#[test]
fn every_finding_under_one_owner_collapses_into_a_single_card() {
    let findings: Vec<Finding> = (0..40)
        .map(|n| {
            finding(
                Severity::Critical,
                web(),
                pod(&format!("web-{n}"), &format!("p-{n}")),
                Some(at(n)),
            )
        })
        .collect();

    let cards = cards(&findings, &[owned_by_itself(web(), Some(40))], &now());
    assert_eq!(cards.len(), 1, "D3: one card per owner, never per pod");
    assert_eq!(cards[0].findings.len(), 40);
    assert_eq!(cards[0].count().as_deref(), Some("40 of 40 pods"));
}

/// The negative half: two owners are two cards, and the card is keyed on `group_key` — kind,
/// namespace and name — so the same name in two namespaces does not collapse.
#[test]
fn two_owners_are_two_cards_and_the_namespace_is_part_of_the_identity() {
    let shop = id(ObjectKind::Deployment, Some("shop"), "web", Some("d-2"));
    let findings = vec![
        finding(Severity::Critical, web(), pod("web-a", "p-a"), Some(at(1))),
        finding(
            Severity::Critical,
            shop.clone(),
            id(ObjectKind::Pod, Some("shop"), "web-b", Some("p-b")),
            Some(at(2)),
        ),
    ];
    let cards = cards(&findings, &[], &now());
    assert_eq!(cards.len(), 2, "payments/web and shop/web merged");
}

/// `Finding::object`'s spec, followed literally: **distinct over the whole `ObjectId`, uid
/// included**. Two findings about one pod count once.
#[test]
fn the_numerator_counts_each_pod_once_however_many_findings_it_has() {
    let findings = vec![
        finding(Severity::Critical, web(), pod("web-a", "p-a"), Some(at(1))),
        finding(Severity::Warn, web(), pod("web-a", "p-a"), Some(at(2))),
        finding(Severity::Warn, web(), pod("web-b", "p-b"), Some(at(3))),
    ];
    let cards = cards(&findings, &[owned_by_itself(web(), Some(5))], &now());
    assert_eq!(cards[0].affected, 2, "one pod was counted twice");
    assert_eq!(cards[0].count().as_deref(), Some("2 of 5 pods"));
}

/// The other side of *distinct is the whole `ObjectId`*: a name deleted and recreated is a
/// different object, so the same name under two uids is two pods.
#[test]
fn one_name_under_two_uids_is_two_pods() {
    let findings = vec![
        finding(Severity::Critical, web(), pod("web-a", "old"), Some(at(1))),
        finding(Severity::Critical, web(), pod("web-a", "new"), Some(at(2))),
    ];
    assert_eq!(cards(&findings, &[], &now())[0].affected, 2);
}

/// NOTES § D39 and `screens/alerts.md`: that count counts pods, and a node card is about one
/// machine. No `n of m` at all.
#[test]
fn a_card_about_a_node_carries_no_pod_count() {
    let node = id(ObjectKind::Node, None, "node-3", Some("n-1"));
    let findings = vec![finding(Severity::Warn, node.clone(), node, Some(at(1)))];
    let cards = cards(&findings, &[], &now());
    assert_eq!(cards[0].affected, 0);
    assert_eq!(cards[0].count(), None, "a node card grew a pod fraction");
}

/// `screens/alerts.md`: the denominator is a second permission, and the card survives losing it.
#[test]
fn without_the_workload_watch_the_card_keeps_the_numerator_and_drops_the_total() {
    let findings = vec![
        finding(Severity::Critical, web(), pod("web-a", "p-a"), Some(at(1))),
        finding(Severity::Critical, web(), pod("web-b", "p-b"), Some(at(2))),
        finding(Severity::Critical, web(), pod("web-c", "p-c"), Some(at(3))),
    ];
    assert_eq!(
        cards(&findings, &[], &now())[0].count().as_deref(),
        Some("3 pods")
    );
    assert_eq!(
        cards(&findings, &[owned_by_itself(web(), Some(5))], &now())[0]
            .count()
            .as_deref(),
        Some("3 of 5 pods")
    );
    assert_eq!(
        cards(&findings, &[owned_by_itself(web(), None)], &now())[0]
            .count()
            .as_deref(),
        Some("3 pods"),
        "a workload that reported no desired count is the same state as no workload"
    );
}

/// A workload for a *different* owner must not lend its number to this card.
#[test]
fn the_denominator_comes_from_this_owners_workload_and_no_other() {
    let other = id(ObjectKind::Deployment, Some("shop"), "api", Some("d-9"));
    let findings = vec![finding(
        Severity::Critical,
        web(),
        pod("web-a", "p-a"),
        Some(at(1)),
    )];
    assert_eq!(
        cards(&findings, &[owned_by_itself(other, Some(99))], &now())[0]
            .count()
            .as_deref(),
        Some("1 pods"),
        "another workload's desired count was borrowed"
    );
}

/// The string `screens/alerts.md` draws itself, at the one count where English would want a
/// singular and the screen file does not.
#[test]
fn a_single_pod_card_is_drawn_with_the_plural_the_screen_file_uses() {
    let job = id(ObjectKind::Job, Some("data"), "migrate-job", Some("j-1"));
    let findings = vec![finding(
        Severity::Critical,
        job.clone(),
        id(ObjectKind::Pod, Some("data"), "migrate-job-x", Some("p-x")),
        Some(at(1)),
    )];
    assert_eq!(
        cards(&findings, &[owned_by_itself(job, Some(1))], &now())[0]
            .count()
            .as_deref(),
        Some("1 of 1 pods")
    );
}

#[test]
fn a_card_is_drawn_as_the_worst_thing_on_it() {
    let findings = vec![
        finding(Severity::Warn, web(), pod("web-a", "p-a"), Some(at(1))),
        finding(Severity::Critical, web(), pod("web-b", "p-b"), Some(at(2))),
        finding(Severity::Info, web(), pod("web-c", "p-c"), Some(at(3))),
    ];
    assert_eq!(
        cards(&findings, &[], &now())[0].severity(),
        Severity::Critical
    );
}

/// `screens/alerts.md`: ordering is severity, then recency. `Severity`'s derived `Ord` runs the
/// other way round from the screen, which is the trap this asserts against.
#[test]
fn cards_sort_critical_first_then_warning() {
    let warn_owner = id(ObjectKind::Deployment, Some("shop"), "api", Some("d-2"));
    let findings = vec![
        finding(
            Severity::Warn,
            warn_owner,
            id(ObjectKind::Pod, Some("shop"), "api-a", Some("p-1")),
            Some(at(500)),
        ),
        finding(Severity::Critical, web(), pod("web-a", "p-a"), Some(at(1))),
    ];
    let cards = cards(&findings, &[], &now());
    assert_eq!(cards[0].severity(), Severity::Critical);
    assert_eq!(
        cards[1].severity(),
        Severity::Warn,
        "an older critical sorted below a newer warning"
    );
}

#[test]
fn inside_one_band_the_most_recent_card_is_first() {
    let old = id(ObjectKind::Deployment, Some("a"), "old", Some("d-1"));
    let new = id(ObjectKind::Deployment, Some("b"), "new", Some("d-2"));
    let findings = vec![
        finding(
            Severity::Critical,
            old,
            id(ObjectKind::Pod, Some("a"), "old-1", Some("p-1")),
            Some(at(10)),
        ),
        finding(
            Severity::Critical,
            new,
            id(ObjectKind::Pod, Some("b"), "new-1", Some("p-2")),
            Some(at(900)),
        ),
    ];
    assert_eq!(cards(&findings, &[], &now())[0].owner.name, "new");
}

/// NOTES § D69's trap, and the reason [`newest_first`] is written out by hand: `Option`'s derived
/// `Ord` puts `None` **first**, and `screens/alerts.md` wants ageless cards **last inside their own
/// band**. The negative assertion is the one that catches the reflex.
#[test]
fn a_card_with_no_age_sorts_last_inside_its_band_and_not_first() {
    let ageless = id(ObjectKind::Node, None, "node-3", Some("n-1"));
    let dated = id(ObjectKind::Node, None, "node-4", Some("n-2"));
    let findings = vec![
        finding(Severity::Warn, ageless.clone(), ageless, None),
        finding(Severity::Warn, dated.clone(), dated, Some(at(1))),
    ];
    let cards = cards(&findings, &[], &now());
    assert_eq!(
        cards[0].owner.name, "node-4",
        "the ageless card sorted to the top of its band"
    );
    assert_eq!(cards[1].owner.name, "node-3");
    assert_eq!(cards[1].age(&at(60)), None, "an ageless card drew an age");
}

/// A card whose band it shares must not push an ageless card out of its own band — the blanks
/// collect at the bottom of *their* band, not at the bottom of the list.
#[test]
fn an_ageless_critical_still_outranks_every_warning() {
    let ageless = id(ObjectKind::Deployment, Some("a"), "crit", Some("d-1"));
    let warn = id(ObjectKind::Deployment, Some("b"), "warn", Some("d-2"));
    let findings = vec![
        finding(
            Severity::Warn,
            warn,
            id(ObjectKind::Pod, Some("b"), "w", Some("p-2")),
            Some(at(9)),
        ),
        finding(Severity::Critical, ageless.clone(), ageless, None),
    ];
    assert_eq!(cards(&findings, &[], &now())[0].owner.name, "crit");
}

/// The card's right edge is the *newest* event on it, and it goes through `Finding::age`.
#[test]
fn the_cards_age_is_the_most_recent_event_filed_under_it() {
    let findings = vec![
        finding(Severity::Critical, web(), pod("web-a", "p-a"), Some(at(0))),
        finding(
            Severity::Critical,
            web(),
            pod("web-b", "p-b"),
            Some(at(240)),
        ),
        finding(Severity::Critical, web(), pod("web-c", "p-c"), None),
    ];
    let cards = cards(&findings, &[], &now());
    assert_eq!(
        cards[0].newest(&now()).and_then(|f| f.timestamp.clone()),
        Some(at(240))
    );
    assert_eq!(
        cards[0].age(&at(480)).as_deref(),
        Some("4 min ago"),
        "the card dated itself from an older finding on it"
    );
}

// --- THE TWO FILTERS ---

#[test]
fn no_filter_matches_everything() {
    let filters = Filters::default();
    assert!(filters.matches(Some("payments"), &["web"]));
    assert!(filters.matches(None, &["node-3"]));
}

#[test]
fn the_text_filter_is_a_substring_and_ignores_case_on_both_sides() {
    let mut filters = Filters::default();
    for character in "WEB".chars() {
        filters.text.push(character);
    }
    assert!(filters.matches(Some("payments"), &["payments/web-7d9f4"]));
    assert!(filters.matches(Some("payments"), &["WEBSERVER"]));
    assert!(
        !filters.matches(Some("payments"), &["api"]),
        "a row that does not contain the needle survived"
    );
}

/// `/` filters *within the current list*, so it reads every field the row shows — a card's title
/// as well as its name.
#[test]
fn the_text_filter_reads_every_field_the_row_shows() {
    let mut filters = Filters::default();
    for character in "memory".chars() {
        filters.text.push(character);
    }
    assert!(filters.matches(Some("payments"), &["web", "exceeded their memory limit"]));
    assert!(!filters.matches(Some("payments"), &["web", "not answering"]));
}

#[test]
fn the_namespace_filter_is_a_substring_of_the_namespace_only() {
    let mut filters = Filters::default();
    for character in "pay".chars() {
        filters.namespace.push(character);
    }
    assert!(filters.matches(Some("payments"), &["web"]));
    assert!(
        !filters.matches(Some("shop"), &["payments-mirror"]),
        "the namespace filter matched against the row's text"
    );
}

/// A cluster-scoped row has no namespace to be `payments`. Matching one would be a list answering
/// a question nobody asked.
#[test]
fn a_row_with_no_namespace_is_refused_by_any_namespace_filter() {
    let mut filters = Filters::default();
    filters.namespace.push('p');
    assert!(!filters.matches(None, &["node-3"]));

    filters.namespace.clear();
    assert!(
        filters.matches(None, &["node-3"]),
        "a cluster-scoped row vanished with no namespace filter set"
    );
}

#[test]
fn both_filters_have_to_pass() {
    let mut filters = Filters::default();
    for character in "web".chars() {
        filters.text.push(character);
    }
    for character in "payments".chars() {
        filters.namespace.push(character);
    }
    assert!(filters.matches(Some("payments"), &["web"]));
    assert!(
        !filters.matches(Some("shop"), &["web"]),
        "namespace ignored"
    );
    assert!(!filters.matches(Some("payments"), &["api"]), "text ignored");
}

/// The everything-filtered-out state `screens/states.md` requires to be representable: a real
/// list, a real filter, and nothing left.
#[test]
fn a_filter_that_matches_nothing_leaves_an_empty_list_and_an_empty_cursor() {
    let rows = [("payments", "web"), ("payments", "api"), ("shop", "worker")];
    let mut filters = Filters::default();
    for character in "zzz".chars() {
        filters.text.push(character);
    }
    let shown: Vec<&(&str, &str)> = rows
        .iter()
        .filter(|(namespace, name)| filters.matches(Some(namespace), &[name]))
        .collect();
    assert!(shown.is_empty());

    let mut cursor = Cursor::default();
    cursor.select(2, &[Some("a"), Some("b"), Some("c")]);
    cursor.follow(&[]);
    assert_eq!(cursor.selected(&[]), None);
}

// --- THE SIDEBAR ---

/// The brief's own point: the core group alone holds kinds belonging to four of the five buckets,
/// so *by API group* cannot be the whole rule — and this is the assertion that says so.
#[test]
fn the_core_group_alone_reaches_four_of_the_five_buckets() {
    for (plural, group) in [
        ("pods", Group::Workloads),
        ("services", Group::Network),
        ("persistentvolumeclaims", Group::Storage),
        ("configmaps", Group::Config),
        ("nodes", Group::Cluster),
    ] {
        assert_eq!(
            Group::of(&browsable("", plural, plural != "nodes")),
            group,
            "core kind {plural} landed in the wrong sidebar group"
        );
    }
}

/// **A core kind the plural table does not name still draws, under `cluster` — and it is
/// namespaced** (NOTES § D246 ruling 6). Only a cluster-scoped one was ever fed, where the core
/// group's `_ => Cluster` and the file's own last resort agree by accident: adding a CRD fallback
/// above it changed nothing any of the 60 tests could see. `events` is the one that tells them
/// apart, and it is on every cluster ever built.
#[test]
fn a_namespaced_core_kind_nobody_listed_still_lands_under_cluster() {
    assert_eq!(
        Group::of(&browsable("", "events", true)),
        Group::Cluster,
        "an unlisted core kind fell to the CRD last resort and landed in workloads"
    );
    assert_eq!(
        Group::of(&browsable("", "componentstatuses", false)),
        Group::Cluster
    );
}

/// **Every group string the table names, fed** — ten of the thirteen `cluster` ones, `autoscaling`
/// and `gateway.networking.k8s.io` had no test at all, so a deleted arm was only visible where it
/// happened to disagree with the last resort (NOTES § D246 ruling 6).
///
/// **`namespaced: true` throughout, and that is what makes this a test of the table.** The last
/// resort sends a namespaced kind to `workloads`, so every `Group::Cluster` row here fails if its
/// arm is deleted — `policy`/`poddisruptionbudgets` is the real namespaced one and the rest are
/// forced namespaced to put them under the same pressure.
#[test]
fn a_named_api_group_places_its_kinds_by_the_group() {
    for (api_group, plural, group) in [
        ("apps", "deployments", Group::Workloads),
        ("batch", "jobs", Group::Workloads),
        ("autoscaling", "horizontalpodautoscalers", Group::Workloads),
        ("networking.k8s.io", "ingresses", Group::Network),
        ("discovery.k8s.io", "endpointslices", Group::Network),
        ("gateway.networking.k8s.io", "httproutes", Group::Network),
        ("storage.k8s.io", "storageclasses", Group::Storage),
        ("rbac.authorization.k8s.io", "roles", Group::Cluster),
        (
            "certificates.k8s.io",
            "certificatesigningrequests",
            Group::Cluster,
        ),
        (
            "admissionregistration.k8s.io",
            "validatingadmissionpolicybindings",
            Group::Cluster,
        ),
        (
            "apiextensions.k8s.io",
            "customresourcedefinitions",
            Group::Cluster,
        ),
        ("apiregistration.k8s.io", "apiservices", Group::Cluster),
        ("policy", "poddisruptionbudgets", Group::Cluster),
        ("coordination.k8s.io", "leases", Group::Cluster),
        ("node.k8s.io", "runtimeclasses", Group::Cluster),
        ("scheduling.k8s.io", "priorityclasses", Group::Cluster),
        (
            "flowcontrol.apiserver.k8s.io",
            "flowschemas",
            Group::Cluster,
        ),
        (
            "authentication.k8s.io",
            "selfsubjectreviews",
            Group::Cluster,
        ),
        (
            "authorization.k8s.io",
            "selfsubjectaccessreviews",
            Group::Cluster,
        ),
        ("events.k8s.io", "events", Group::Cluster),
    ] {
        assert_eq!(
            Group::of(&browsable(api_group, plural, true)),
            group,
            "{api_group}/{plural} landed in the wrong sidebar group"
        );
    }
}

/// **`metrics.k8s.io` is aggregated, not a CRD, and its resource is spelled `pods`**
/// (NOTES § D246 ruling 5). It is namespaced and listable, so it survives `k8s::browsable()`, and
/// the step-3 last resort put it in `workloads` — where `k8s.rs` sorts by plural and it landed
/// **next to the core `pods`**, two adjacent rows reading the same word, on every cluster running
/// metrics-server.
#[test]
fn the_aggregated_metrics_api_does_not_put_a_second_pods_row_under_workloads() {
    assert_eq!(
        Group::of(&browsable("metrics.k8s.io", "pods", true)),
        Group::Cluster,
        "metrics-server's `pods` sat beside the core `pods` in workloads"
    );
    assert_eq!(
        Group::of(&browsable("metrics.k8s.io", "nodes", false)),
        Group::Cluster
    );
}

/// Invariant 12's whole point: a kind nobody here has ever heard of gets a row, in a **named**
/// bucket, decided by the one field that carries a fact rather than a spelling.
#[test]
fn a_crd_nobody_has_heard_of_is_placed_and_never_dropped() {
    assert_eq!(
        Group::of(&browsable("argoproj.io", "rollouts", true)),
        Group::Workloads
    );
    assert_eq!(
        Group::of(&browsable("cert-manager.io", "clusterissuers", false)),
        Group::Cluster
    );

    let kinds = [browsable("example.com", "widgets", true)];
    let nav = sidebar(&kinds, 0, Some(Group::Workloads));
    assert!(
        nav.contains(&NavItem::Kind(0)),
        "an unknown CRD was dropped from the sidebar"
    );
}

#[test]
fn the_sidebar_draws_all_five_groups_even_on_a_cluster_that_serves_nothing() {
    let nav = sidebar(&[], 0, None);
    for group in Group::ALL {
        assert!(
            nav.contains(&NavItem::Group(group)),
            "{} vanished from the sidebar",
            group.label()
        );
    }
    assert_eq!(nav.first(), Some(&NavItem::Alerts), "ALERTS is not first");
}

#[test]
fn only_the_open_group_lists_its_kinds() {
    let kinds = [
        browsable("apps", "deployments", true),
        browsable("", "services", true),
    ];
    let closed = sidebar(&kinds, 0, None);
    assert!(
        !closed.iter().any(|item| matches!(item, NavItem::Kind(_))),
        "a closed sidebar listed kinds"
    );

    let open = sidebar(&kinds, 0, Some(Group::Workloads));
    assert!(open.contains(&NavItem::Kind(0)), "deployments is missing");
    assert!(
        !open.contains(&NavItem::Kind(1)),
        "the network group listed its kinds while workloads was the open one"
    );
}

/// **[`NavItem::Kind`] carries the index into `kinds`, not the index within the open group**
/// (NOTES § D246 ruling 6) — which is why [`sidebar`] enumerates *before* it filters. Swapping the
/// two survived every one of the 60 tests, because they all opened `workloads`, whose first kind
/// is also index 0 of the whole list; the mutant opens a **different kind** under every other
/// group. Opening `network` and finding `Kind(1)` there is what says so.
#[test]
fn an_open_group_names_its_kinds_by_their_place_in_the_whole_discovery_list() {
    let kinds = [
        browsable("apps", "deployments", true),
        browsable("", "services", true),
        browsable("storage.k8s.io", "storageclasses", false),
    ];

    let network = sidebar(&kinds, 0, Some(Group::Network));
    assert!(
        network.contains(&NavItem::Kind(1)),
        "the open group's kind was numbered within its group, so it would open `deployments`"
    );
    assert!(!network.contains(&NavItem::Kind(0)));

    let storage = sidebar(&kinds, 0, Some(Group::Storage));
    assert!(
        storage.contains(&NavItem::Kind(2)),
        "the third kind in the discovery list was not numbered 2"
    );
}

#[test]
fn the_reports_are_appended_under_their_own_header() {
    let nav = sidebar(&[], 7, None);
    let analysis = nav
        .iter()
        .position(|item| *item == NavItem::Header("ANALYSIS"))
        .expect("no ANALYSIS header");
    assert_eq!(nav.len() - analysis - 1, 7, "a report row is missing");
    assert_eq!(nav.last(), Some(&NavItem::Report(6)));
}

/// `screens/widgets.md` § 2: group headers are unselectable rows and `↑↓` skips them.
#[test]
fn the_cursor_walks_past_the_section_headers() {
    let nav = sidebar(&[], 2, None);
    let landable = selectable(&nav, |item| item.selectable());
    assert!(
        landable
            .iter()
            .all(|at| !matches!(nav[*at], NavItem::Header(_))),
        "a header was offered to the cursor"
    );
    assert_eq!(
        landable.len(),
        nav.len() - 2,
        "exactly the two headers should have been skipped"
    );
}

/// NOTES § D127: the variant decides selection, never a field — so a counted row with `jump: None`
/// is still selectable and a `Prose` line with anything on it is not.
#[test]
fn only_an_answer_row_of_a_report_can_be_selected() {
    let rows = vec![
        ReportRow::Prose("Still counted, from what you can see:".to_owned()),
        ReportRow::Answer {
            severity: None,
            text: "34 workloads have no memory or CPU limit".to_owned(),
            detail: Vec::new(),
            action: String::new(),
            jump: None,
        },
        ReportRow::NotComputed {
            reason: "Not checked here.".to_owned(),
            ask_for: "Ask for cluster-wide read access.".to_owned(),
        },
        ReportRow::Answer {
            severity: Some(Severity::Warn),
            text: "node-1 is over-committed".to_owned(),
            detail: Vec::new(),
            action: String::new(),
            jump: Some(Jump::Object(web())),
        },
    ];
    assert_eq!(selectable(&rows, answers), vec![1, 3]);
}

/// A report that could not run has no cursor at all, and its pane drops `⏎ open`.
#[test]
fn a_report_that_could_not_run_offers_the_cursor_nothing() {
    let rows = vec![ReportRow::NotComputed {
        reason: "Not checked here.".to_owned(),
        ask_for: "Ask for cluster-wide read access.".to_owned(),
    }];
    assert!(selectable(&rows, answers).is_empty());
}

// --- THE MODAL LAYER ---

fn dialog(asks: Option<&str>) -> Dialog {
    Dialog {
        object: "deployment/web".to_owned(),
        namespace: Some("payments".to_owned()),
        consequence: "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies."
            .to_owned(),
        kubectl: "kubectl scale deployment/web --replicas=3 -n payments".to_owned(),
        verdict: None,
        asks: asks.map(str::to_owned),
        typed: Input::default(),
    }
}

/// `screens/dialogs.md` rule 3: the verdict is shown *before* the button is live.
#[test]
fn the_confirm_button_is_not_live_until_the_check_has_answered() {
    let mut press = dialog(None);
    assert!(!press.armed(), "the button was live with no verdict");
    press.verdict = Some("The cluster checked it first and accepted it.");
    assert!(press.armed());
}

/// Invariant 2: a delete needs the object's name typed back, and nothing less does.
#[test]
fn a_typed_name_dialog_is_only_live_on_an_exact_match() {
    let mut delete = dialog(Some("web"));
    delete.verdict = Some("Nothing was checked first.");
    assert!(!delete.armed(), "an empty box armed a delete");

    for (typed, live) in [("w", false), ("we", false), ("WEB", false), ("webb", false)] {
        delete.typed.clear();
        for character in typed.chars() {
            delete.typed.push(character);
        }
        assert_eq!(delete.armed(), live, "{typed:?} should not arm this delete");
    }

    delete.typed.clear();
    for character in "web".chars() {
        delete.typed.push(character);
    }
    assert!(delete.armed(), "the exact name did not arm the delete");
}

/// `ops::Checked::typed`'s own hole, closed on the drawing side too: `typed == name` holds for
/// `("", "")`, which is *typing the object name* satisfied by typing nothing.
#[test]
fn an_empty_name_never_arms_anything_however_little_is_typed() {
    let mut delete = dialog(Some(""));
    delete.verdict = Some("Nothing was checked first.");
    assert!(delete.typed.is_empty());
    assert!(
        !delete.armed(),
        "a name stripped to nothing was confirmed by an empty box"
    );
}

/// NOTES § D12 and `screens/dialogs.md` § *While the call is running*: `q` is refused while a
/// write is in flight, and `X` is unbound while a modal is open.
#[test]
fn quit_and_the_cluster_switcher_are_refused_exactly_where_the_key_map_says() {
    let mut app = App::default();
    assert!(app.may_quit() && app.may_switch_cluster() && app.may_mutate());

    app.modal = Some(Modal::Help);
    assert!(app.may_quit(), "q was refused merely because help was open");
    assert!(!app.may_switch_cluster(), "X stayed bound under a modal");
    assert!(
        !app.may_mutate(),
        "a second dialog could open over the first"
    );

    app.modal = None;
    app.changing = true;
    assert!(!app.may_quit(), "q was allowed mid-write");
    assert!(!app.may_switch_cluster());
    assert!(!app.may_mutate(), "a second mutation was allowed");
}

/// `screens/widgets.md:273`: **`esc` closes exactly one level, always** — and the two filters are
/// two levels, so one press clears one of them (NOTES § D246). Scoped to `n payments` and then
/// narrowed with `/web`, one `esc` used to jump all the way back to every namespace in the
/// cluster: one press undoing two.
#[test]
fn escape_closes_one_level_per_press_and_never_two() {
    let mut app = App::default();
    for character in "payments".chars() {
        app.filters.namespace.push(character);
    }
    for character in "web".chars() {
        app.filters.text.push(character);
    }
    app.modal = Some(Modal::Confirm(dialog(None)));

    app.escape();
    assert!(app.modal.is_none(), "esc did not close the modal");
    assert_eq!(
        app.filters.text.text(),
        "web",
        "esc closed the modal and cleared a filter in one press"
    );

    app.escape();
    assert!(
        app.filters.text.is_empty(),
        "esc did not clear the text filter"
    );
    assert_eq!(
        app.filters.namespace.text(),
        "payments",
        "one press dropped the text filter and the namespace scope together"
    );

    app.escape();
    assert!(
        app.filters.namespace.is_empty(),
        "the second press did not clear the namespace scope"
    );
}

/// The other order: with no text filter set, the one press goes straight to the namespace, so
/// `esc` is never a press that does nothing.
#[test]
fn escape_clears_the_namespace_scope_when_no_text_filter_is_set() {
    let mut app = App::default();
    for character in "payments".chars() {
        app.filters.namespace.push(character);
    }
    app.escape();
    assert!(app.filters.namespace.is_empty());
}

// --- WHICH VIEW IS OPEN ---

#[test]
fn the_default_view_is_alerts() {
    assert_eq!(App::default().view, View::Alerts);
}

#[test]
fn opening_a_sidebar_row_changes_the_view_and_resets_the_content_cursor() {
    let mut app = App::default();
    app.content.select(3, &[Some("a"); 5]);

    app.open(NavItem::Kind(2));
    assert_eq!(app.view, View::Resources(2));
    assert_eq!(
        app.content.selected(&[Some("a"); 5]),
        Some(0),
        "the cursor kept a position from a list it was never in"
    );

    app.open(NavItem::Report(4));
    assert_eq!(app.view, View::Analysis(4));

    app.open(NavItem::Alerts);
    assert_eq!(app.view, View::Alerts);
}

#[test]
fn opening_a_group_toggles_it_and_leaves_the_view_where_it_was() {
    let mut app = App::default();
    app.open(NavItem::Kind(1));

    app.open(NavItem::Group(Group::Network));
    assert_eq!(app.expanded, Some(Group::Network));
    assert_eq!(app.view, View::Resources(1), "opening a group changed view");

    app.open(NavItem::Group(Group::Network));
    assert_eq!(app.expanded, None, "the group did not close again");

    app.open(NavItem::Group(Group::Storage));
    app.open(NavItem::Group(Group::Config));
    assert_eq!(
        app.expanded,
        Some(Group::Config),
        "two groups were open at once"
    );
}

#[test]
fn a_header_row_opens_nothing() {
    let mut app = App::default();
    app.open(NavItem::Kind(3));
    app.content.select(2, &[Some("a"); 4]);

    app.open(NavItem::Header("RESOURCES"));
    assert_eq!(app.view, View::Resources(3));
    assert_eq!(
        app.content.selected(&[Some("a"); 4]),
        Some(2),
        "a header reset the content cursor"
    );
}

#[test]
fn the_detail_tabs_clamp_at_both_ends() {
    assert_eq!(Tab::default(), Tab::Logs);
    assert_eq!(Tab::Logs.previous(), Tab::Logs, "[ wrapped off the left");
    assert_eq!(Tab::Logs.next(), Tab::Describe);
    assert_eq!(Tab::Describe.next().next(), Tab::Events);
    assert_eq!(Tab::Events.next(), Tab::Events, "] wrapped off the right");
    assert_eq!(Tab::Events.previous(), Tab::Yaml);
}

/// `screens/widgets.md` § 4: any manual scroll turns follow mode off. The two halves are one
/// action, so no key handler can do the first and forget the second.
#[test]
fn a_manual_scroll_turns_follow_mode_off() {
    let mut app = App {
        following: true,
        ..App::default()
    };
    app.scroll_by(5);
    assert_eq!(app.scroll, 5);
    assert!(!app.following, "follow mode survived a manual scroll");

    app.scroll_by(-99);
    assert_eq!(app.scroll, 0, "the offset went below the top of the buffer");
}

// --- THE SHAPES THE MUTATION GATE PROVED WERE NOT BEING FED ---

/// **The clamp in [`Cursor::selected`] is the last line of defence, and it is reachable.** Every
/// writer clamps, so the clamp looks dead — but `selected` can be asked about a list the cursor
/// was never set against, which is the ordinary state between a filter keystroke and the
/// [`Cursor::follow`] that answers it. Without the clamp the renderer indexes past the end.
///
/// `just mutants-diff` found this: two survivors at `Cursor::selected` (`- with +`, `- with /`),
/// because no test ever asked about a shorter list than the one it had selected in.
#[test]
fn a_cursor_asked_about_a_shorter_list_than_it_was_set_against_stays_in_range() {
    let long = [Some("a"), Some("b"), Some("c"), Some("d")];
    let mut cursor = Cursor::default();
    cursor.select(3, &long);

    let short = [Some("a")];
    let at = cursor
        .selected(&short)
        .expect("a one-row list still has a selection");
    assert!(
        at < short.len(),
        "selected() returned {at} for a list of {}, which indexes past the end",
        short.len()
    );
    assert_eq!(at, 0);
}

/// A sidebar group is a disclosure triangle, not a destination: opening one must not throw away
/// the reader's place in the table they were reading.
#[test]
fn expanding_a_sidebar_group_leaves_the_content_cursor_where_it_was() {
    let rows = [Some("a"), Some("b"), Some("c")];
    let mut app = App::default();
    app.open(NavItem::Kind(0));
    app.content.select(2, &rows);

    app.open(NavItem::Group(Group::Network));
    assert_eq!(
        app.content.selected(&rows),
        Some(2),
        "expanding a group reset the cursor of a pane it did not touch"
    );

    // Re-opening the kind already open is not a change either.
    app.open(NavItem::Kind(0));
    assert_eq!(
        app.content.selected(&rows),
        Some(2),
        "re-opening the kind already open reset the cursor"
    );

    // A real change does reset it.
    app.open(NavItem::Kind(1));
    assert_eq!(app.content.selected(&rows), Some(0));
}

/// The five labels are the strings `screens/` draws, so they are transcribed here by hand — two
/// transcriptions of one table, which is the only way a renamed row is caught. `theme_tests.rs`
/// pins the palette the same way.
#[test]
fn the_five_group_labels_are_the_words_the_sidebar_draws() {
    let drawn = [
        (Group::Workloads, "workloads"),
        (Group::Network, "network"),
        (Group::Storage, "storage"),
        (Group::Config, "config"),
        (Group::Cluster, "cluster"),
    ];
    for (group, label) in drawn {
        assert_eq!(
            group.label(),
            label,
            "{group:?} draws the wrong sidebar row"
        );
    }
    assert_eq!(
        drawn.len(),
        Group::ALL.len(),
        "a group was added without a label to draw it with"
    );
}

/// **The group decides, and `namespaced` is only the last resort** — which is invisible while
/// every kind in `apps` and `batch` happens to be namespaced, because the fallback then agrees
/// with the rule by accident. `just mutants-diff` deleted that arm and no test noticed.
///
/// A cluster-scoped kind in one of those groups is what tells them apart. There is no built-in
/// one today; the value below is constructed precisely because it is the shape that distinguishes
/// the intent from the coincidence.
#[test]
fn a_workload_group_places_its_kinds_even_when_they_are_cluster_scoped() {
    assert_eq!(
        Group::of(&browsable("apps", "deployments", false)),
        Group::Workloads,
        "the group was ignored and `namespaced` decided instead"
    );
    assert_eq!(
        Group::of(&browsable("batch", "jobs", false)),
        Group::Workloads
    );
    // The contrast: the same `namespaced: false` on an unknown group *does* fall to the last
    // resort, which is what makes the two assertions above about the group and not about luck.
    assert_eq!(
        Group::of(&browsable("example.com", "widgets", false)),
        Group::Cluster
    );
}

/// **A surge puts more pods on a card than `spec.replicas` asks for, and then the denominator is
/// dropped** (NOTES § D246 ruling 1). The requirement is `screens/alerts.md` § *the third form*,
/// which drops it for `unavailableReplicas` for this reason in these words — *a denominator here
/// would eventually print `2 of 1 pod not answering`* — and PRIOR-ART § F2, *never divide by a
/// denominator that is not guaranteed complete*.
///
/// **Neither of the other two answers is available.** Clamping to `3 of 3` hides a pod that has a
/// finding, which is the one direction that file forbids outright; printing `4 of 3` is the number
/// this product exists to not print.
#[test]
fn a_card_with_more_affected_pods_than_the_workload_wants_drops_the_denominator() {
    let findings: Vec<Finding> = (0..4)
        .map(|n| {
            finding(
                Severity::Critical,
                web(),
                pod(&format!("web-{n}"), &format!("p-{n}")),
                Some(at(n)),
            )
        })
        .collect();
    assert_eq!(
        cards(&findings, &[owned_by_itself(web(), Some(3))], &now())[0]
            .count()
            .as_deref(),
        Some("4 pods"),
        "the card printed a fraction whose halves count different things"
    );
    // The boundary: exactly as many as were asked for still gets its pair.
    assert_eq!(
        cards(&findings, &[owned_by_itself(web(), Some(4))], &now())[0]
            .count()
            .as_deref(),
        Some("4 of 4 pods"),
        "the denominator was dropped where it holds"
    );
}

/// **The one-replica shape, which reaches the same arm by a different route** (NOTES § D246
/// ruling 1) — and it is the commonest Deployment size there is, so it is not the exotic case.
/// `spec.replicas: 1`, the old pod stuck `Terminating` behind a finalizer (rule 12) while its
/// replacement cannot pull its image (rule 3): two pod objects under one owner at `desired: 1`.
/// A surge is a rollout getting *ahead*; this is a delete that never finished, and only one test
/// per route says so.
#[test]
fn a_stuck_terminating_pod_beside_its_replacement_drops_the_denominator_at_one_replica() {
    let findings = vec![
        finding(
            Severity::Warn,
            web(),
            pod("web-old", "p-old"),
            Some(at(100)),
        ),
        finding(
            Severity::Critical,
            web(),
            pod("web-new", "p-new"),
            Some(at(200)),
        ),
    ];
    let card = &cards(&findings, &[owned_by_itself(web(), Some(1))], &now())[0];
    assert_eq!(card.affected, 2, "the stuck pod was not counted");
    assert_eq!(
        card.count().as_deref(),
        Some("2 pods"),
        "the card read `2 of 1 pods`"
    );
}

/// **A bare pod owns itself, and that card draws no count at all** (NOTES § D246 ruling 2).
/// `screens/alerts.md` draws it twice off committed captures — `default/broken-pending` and
/// `default/broken-hostpath` — with the identity line ending after the name, and says it twice in
/// prose: *"a bare pod, so there is no owner and no `n of m`"*. `1 of 1 pods` about a pod out of
/// itself is not a fact, and `1 pods` is the string it used to draw.
#[test]
fn a_card_whose_owner_is_the_pod_itself_draws_no_count() {
    let bare = id(
        ObjectKind::Pod,
        Some("default"),
        "broken-pending",
        Some("p-1"),
    );
    let findings = vec![finding(Severity::Critical, bare.clone(), bare, Some(at(1)))];
    let card = &cards(&findings, &[], &now())[0];
    assert_eq!(card.affected, 1, "the pod itself is still one affected pod");
    assert_eq!(
        card.count(),
        None,
        "a bare pod card drew a fraction of itself"
    );
}

/// The second shape of the same card, and it is on every kubeadm and kind cluster: a **mirror
/// pod**, which the kubelet owns and no controller does.
///
/// **The workload snapshot in the slice could not come off the watch** — `k8s.rs` builds one only
/// for a Deployment, StatefulSet, DaemonSet or ReplicaSet, never for a pod. It is constructed here
/// so the assertion is about `owner.kind` and cannot pass merely because the lookup found nothing:
/// without the kind check this card reads `1 of 1 pods`.
#[test]
fn a_mirror_pod_card_draws_no_count_even_with_a_workload_in_the_slice() {
    let etcd = id(
        ObjectKind::Pod,
        Some("kube-system"),
        "etcd-k8rs-control-plane",
        Some("p-etcd"),
    );
    let findings = vec![finding(
        Severity::Warn,
        etcd.clone(),
        etcd.clone(),
        Some(at(1)),
    )];
    let card = &cards(&findings, &[owned_by_itself(etcd, Some(1))], &now())[0];
    assert_eq!(
        card.count(),
        None,
        "a workload snapshot lent a mirror pod a denominator"
    );
}

/// **A clock ahead of the reader must not erase the age column or reorder the screen**
/// (NOTES § D246 ruling 4). `rules::age` refuses a stamp more than five minutes into the future
/// and returns `None`; a comparator reading the raw timestamp then sorted that card **first** in
/// its band, where `screens/alerts.md:120-126` puts ageless cards **last** — *an unknown time
/// cannot claim to be more recent than a known one*.
///
/// The worse half is on one card: `Card::age` is `newest()?.age(now)`, so **one** future finding
/// suppressed a perfectly drawable age beside it. A laptop resumed from suspend before NTP catches
/// up reads every finding as future and loses the whole right-hand column.
#[test]
fn a_clock_ahead_of_the_reader_neither_dates_a_card_nor_sorts_it_first() {
    let reader = at(1_000);
    let ahead = at(1_000 + 600); // ten minutes ahead, past the five-minute allowance
    let mixed = id(ObjectKind::Deployment, Some("a"), "mixed", Some("d-1"));
    let skewed = id(ObjectKind::Deployment, Some("b"), "skewed", Some("d-2"));
    let dated = id(ObjectKind::Deployment, Some("c"), "dated", Some("d-3"));

    let findings = vec![
        // Listed first, and with the newest raw stamp of the three cards.
        finding(
            Severity::Critical,
            skewed.clone(),
            id(ObjectKind::Pod, Some("b"), "s-1", Some("p-1")),
            Some(ahead.clone()),
        ),
        finding(
            Severity::Critical,
            mixed.clone(),
            id(ObjectKind::Pod, Some("a"), "m-1", Some("p-2")),
            Some(ahead),
        ),
        finding(
            Severity::Critical,
            mixed,
            id(ObjectKind::Pod, Some("a"), "m-2", Some("p-3")),
            Some(at(400)),
        ),
        finding(
            Severity::Critical,
            dated,
            id(ObjectKind::Pod, Some("c"), "d-1", Some("p-4")),
            Some(at(940)),
        ),
    ];

    let cards = cards(&findings, &[], &reader);
    assert_eq!(
        cards
            .iter()
            .map(|c| c.owner.name.as_str())
            .collect::<Vec<_>>(),
        ["dated", "mixed", "skewed"],
        "a card the renderer cannot date claimed to be the most recent"
    );
    assert_eq!(
        cards[1].age(&reader).as_deref(),
        Some("10 min ago"),
        "one future finding suppressed a drawable age beside it"
    );
    assert_eq!(
        cards[2].age(&reader),
        None,
        "a stamp the age ladder refuses was drawn anyway"
    );
}

/// **The lookup keys on `WorkloadSnapshot::id`, not on its `owner`** (NOTES § D246 ruling 6), and
/// this is the shape that tells them apart: the watch reports a ReplicaSet *and* its Deployment, a
/// ReplicaSet's snapshot carries `owner: the Deployment`, and the ReplicaSet arrives first. Keyed
/// on `owner`, the Deployment's card borrows the ReplicaSet's `desired` — `1 of 1 pods` about a
/// Deployment that wants three.
#[test]
fn the_denominator_is_the_workloads_own_and_never_one_its_owner_field_points_at() {
    let deployment = web();
    let replicaset = id(
        ObjectKind::ReplicaSet,
        Some("payments"),
        "web-7d9f4",
        Some("rs-1"),
    );
    let findings = vec![finding(
        Severity::Critical,
        deployment.clone(),
        pod("web-a", "p-a"),
        Some(at(1)),
    )];
    let watched = [
        workload(replicaset, deployment.clone(), Some(1)),
        owned_by_itself(deployment, Some(3)),
    ];
    assert_eq!(
        cards(&findings, &watched, &now())[0].count().as_deref(),
        Some("1 of 3 pods"),
        "a ReplicaSet's desired count was borrowed for its Deployment's card"
    );
}

/// [`Card::newest`] promises `None` when nothing on the card carries a time, and a card of
/// nothing but ageless findings is the shape that proves it — `max_by` over `Option<Time>` would
/// otherwise hand back a finding whose timestamp is `None`.
#[test]
fn a_card_where_nothing_carries_a_time_has_no_newest_finding() {
    let node = id(ObjectKind::Node, None, "node-3", Some("n-1"));
    let findings = vec![
        finding(Severity::Warn, node.clone(), node.clone(), None),
        finding(Severity::Warn, node.clone(), node, None),
    ];
    let cards = cards(&findings, &[], &now());
    assert!(
        cards[0].newest(&now()).is_none(),
        "an ageless card named a newest"
    );
    assert_eq!(cards[0].age(&at(60)), None);
}

// --- THE WORDING A DETAIL TAB DRAWS ---
//
// **The sentences here have two consumers and this is where they are proven once** (NOTES § D254):
// `ui.rs` draws them and the temporary `main.rs` prints them, and the whole reason they live in
// `views.rs` is that a second wording is a second thing that can be wrong.

fn stopped(reason: Option<&str>, exit_code: i32) -> ContainerState {
    ContainerState::Terminated(crate::rules::Terminated {
        reason: reason.map(str::to_owned),
        exit_code,
        started_at: None,
        finished_at: None,
        message: None,
    })
}

fn waiting(reason: Option<&str>) -> ContainerState {
    ContainerState::Waiting {
        reason: reason.map(str::to_owned),
        message: None,
    }
}

/// One event, in the five fields [`crate::k8s::Happening`] carries.
fn happening(count: Option<i32>, first: Option<Time>) -> crate::k8s::Happening {
    crate::k8s::Happening {
        at: Some(at(0)),
        reason: "Unhealthy".to_owned(),
        message: "Readiness probe failed".to_owned(),
        count,
        first,
    }
}

/// **`happened 2,383 times since 4 days ago`** — both numbers where both are known, the count
/// alone where the first stamp did not survive, and silence at one.
#[test]
fn a_repeated_event_says_how_often_and_over_what_span() {
    let now = at(345_600);
    assert_eq!(
        repeated(&happening(Some(2383), Some(at(0))), &now).as_deref(),
        Some("happened 2,383 times since 4 days ago"),
        "exact, with a comma at the thousand, and the span off the one age ladder"
    );
    assert_eq!(
        repeated(&happening(Some(2383), None), &now).as_deref(),
        Some("happened 2,383 times"),
        "no first stamp is the count alone, never a span this file guessed"
    );
    assert_eq!(
        repeated(&happening(Some(1), Some(at(0))), &now),
        None,
        "a thing that happened once needs no sentence saying so"
    );
    // The boundary the screen fixes in words is *more than one*, not the mockup's 2,383: two is
    // the first count that earns the line, and `<` is what makes it so.
    assert_eq!(
        repeated(&happening(Some(2), Some(at(0))), &now).as_deref(),
        Some("happened 2 times since 4 days ago"),
        "`screens/detail.md` § A repeated event: the line appears when the count is more than one"
    );
    assert_eq!(repeated(&happening(None, None), &now), None);
    assert_eq!(
        repeated(&happening(Some(-4), None), &now),
        None,
        "a count the API server never sets is drawn as none, not as its absolute value"
    );
}

/// **Nothing left is not nothing happened**, and the emptiness is decided here so both surfaces
/// ask one function.
#[test]
fn an_empty_events_read_gets_the_sentence_and_a_full_one_does_not() {
    assert_eq!(no_events(&crate::k8s::Happened::default()), Some(NO_EVENTS));
    assert!(NO_EVENTS.starts_with("Kubernetes only keeps events"));
    assert!(
        !NO_EVENTS.starts_with("k8rs:"),
        "the stderr prefix is the driver's, not part of the sentence"
    );
    assert_eq!(
        no_events(&crate::k8s::Happened {
            lines: vec![happening(None, None)],
            cut: false,
        }),
        None
    );
}

/// **The heading is where the newest-first claim is made, so a cut read is where it is withdrawn**
/// — with `k8s::EVENTS_KEPT` interpolated rather than a second copy of the number.
#[test]
fn the_events_heading_takes_back_newest_first_when_the_read_was_cut() {
    let full = crate::k8s::Happened {
        lines: vec![happening(None, None)],
        cut: false,
    };
    assert_eq!(events_heading(&full), "events (newest first)");
    assert_eq!(
        events_heading(&crate::k8s::Happened::default()),
        "events",
        "an empty list promises no order because it has none"
    );
    let cut = crate::k8s::Happened { cut: true, ..full };
    assert_eq!(
        events_heading(&cut),
        format!(
            "events (the first {} k8rs was given — there are more, and these are not the newest)",
            crate::k8s::EVENTS_KEPT
        )
    );
    assert!(
        !events_heading(&cut).ends_with(':'),
        "the punctuation is the caller's — one surface draws a colon and one does not"
    );
}

/// **The raw word beside the message, and no empty brackets when there is no word.**
#[test]
fn a_state_word_is_drawn_beside_its_message_and_never_instead_of_it() {
    assert_eq!(
        raw_and_message("Unhealthy", Some("Readiness probe failed")),
        "(Unhealthy) Readiness probe failed"
    );
    assert_eq!(
        raw_and_message("Evicted", None),
        "(Evicted)",
        "a missing message costs the space and nothing else"
    );
    assert_eq!(
        raw_and_message("", Some("the node was low on memory")),
        "the node was low on memory",
        "an event with no reason draws no empty brackets"
    );
}

/// **Every container state `screens/detail.md` names, and the fall-through for the rest.**
#[test]
fn a_containers_state_is_a_word_a_beginner_reads() {
    assert_eq!(
        container_state(Some(&ContainerState::Running { started_at: None })),
        ("running".to_owned(), None)
    );
    assert_eq!(
        container_state(Some(&stopped(None, 0))),
        ("done".to_owned(), None),
        "a clean exit is the healthy case and is not renamed to failed"
    );
    assert_eq!(
        container_state(Some(&stopped(Some("OOMKilled"), 137))),
        (
            "failed".to_owned(),
            Some("container exceeded its memory limit — exit 137".to_owned())
        )
    );
    assert_eq!(
        container_state(Some(&stopped(Some("Error"), 1))),
        ("failed".to_owned(), Some("exit 1".to_owned())),
        "a reason no table names falls through to the exit code, never a guessed word"
    );
    assert_eq!(
        container_state(Some(&stopped(None, 255))),
        ("failed".to_owned(), Some("exit 255".to_owned()))
    );
    assert_eq!(
        container_state(Some(&waiting(Some("CrashLoopBackOff")))),
        ("keeps crashing and restarting".to_owned(), None)
    );
    assert_eq!(
        container_state(Some(&waiting(Some("ContainerCreating")))),
        ("not started".to_owned(), None),
        "the ordinary first second of every pod is not dressed up as a problem"
    );
    assert_eq!(
        container_state(Some(&waiting(Some("SomethingNew")))),
        ("SomethingNew".to_owned(), None),
        "a reason this table does not know prints its own raw word"
    );
    assert_eq!(
        container_state(Some(&ContainerState::Waiting {
            reason: Some("InvalidImageName".to_owned()),
            message: Some("couldn't parse image reference".to_owned()),
        })),
        (
            "InvalidImageName".to_owned(),
            Some("couldn't parse image reference".to_owned())
        ),
        "and keeps the kubelet's sentence under it — the events table's rule, not a second one"
    );
    assert_eq!(
        container_state(Some(&ContainerState::Waiting {
            reason: Some("CrashLoopBackOff".to_owned()),
            message: Some("back-off 5m0s restarting failed container".to_owned()),
        })),
        ("keeps crashing and restarting".to_owned(), None),
        "a reason the table does name says the phrase alone, which is what the mockup draws"
    );
    assert_eq!(
        container_state(Some(&ContainerState::Waiting {
            reason: Some("InvalidImageName".to_owned()),
            message: Some(String::new()),
        })),
        ("InvalidImageName".to_owned(), None),
        "an empty message is no message, not a blank line carrying the restart count"
    );
    assert_eq!(
        container_state(Some(&waiting(None))),
        ("waiting".to_owned(), None)
    );
    assert_eq!(
        container_state(None),
        ("not started".to_owned(), None),
        "a container the kubelet has not reported on"
    );
}

/// **`, 3 restarts`, or nothing at all** — one spelling of a fact two surfaces draw, and a count
/// no API server produces is drawn as none.
#[test]
fn a_restart_count_is_spelled_once_and_a_negative_one_is_not_a_count() {
    let counted = |restarts: i32| ContainerSnapshot {
        name: "app".to_owned(),
        image: "app:1".to_owned(),
        role: crate::rules::ContainerRole::Regular,
        restart_policy: None,
        restart_rules: Vec::new(),
        restarts,
        state: ContainerState::Running { started_at: None },
        last_terminated: None,
        ready: true,
        started: true,
        cpu_request: None,
        memory_request: None,
        cpu_limit: None,
        memory_limit: None,
        allocated_cpu: None,
        allocated_memory: None,
    };
    assert_eq!(restarts(None), "");
    assert_eq!(restarts(Some(&counted(0))), "");
    assert_eq!(restarts(Some(&counted(1))), ", 1 restart");
    assert_eq!(restarts(Some(&counted(12))), ", 12 restarts");
    assert_eq!(restarts(Some(&counted(-1))), "");
}

/// **`⇧p` with no previous run to show** falls back and says so — and stays silent when it was not
/// asked for, or when the container really has restarted.
#[test]
fn asking_for_a_previous_run_that_does_not_exist_is_answered_in_one_sentence() {
    assert_eq!(
        no_previous_run("app", 0, true).as_deref(),
        Some(
            "app hasn't restarted, so there's no previous run to show. Showing the current run \
             instead."
        )
    );
    assert_eq!(no_previous_run("app", 0, false), None, "not asked for");
    assert_eq!(no_previous_run("app", 3, true), None, "it has restarted");
    assert!(
        no_previous_run("app", -1, true).is_some(),
        "a count no API server produces reads as *no restarts* here exactly as it does in the \
         display sites, and never as a previous run this pane cannot show"
    );
}

/// **`Pod · running · created 3 days ago`, and each part dropped rather than guessed at.**
#[test]
fn a_pods_identity_line_drops_what_it_cannot_read_rather_than_guessing() {
    let mut pod = PodSnapshot::from(k8s_openapi::api::core::v1::Pod::default());
    assert_eq!(
        identity(&pod, &now()),
        vec!["Pod".to_owned()],
        "no phase and no stamp is `Pod`, never `Pod · unknown`"
    );
    pod.phase = Some("Running".to_owned());
    pod.creation_timestamp = Some(at(1_000_000 - 259_200));
    assert_eq!(
        identity(&pod, &now()),
        vec!["Pod · running · created 3 days ago".to_owned()]
    );
    pod.phase = Some("Failed".to_owned());
    pod.reason = Some("Evicted".to_owned());
    assert_eq!(
        identity(&pod, &now()),
        vec![
            "Pod · failed · created 3 days ago".to_owned(),
            "removed by the node to take back room".to_owned(),
            "(Evicted)".to_owned(),
        ],
        "the phrase, then the raw word — the same shape an event's row uses"
    );
    pod.reason = Some("Shutdown".to_owned());
    assert_eq!(
        identity(&pod, &now())[1],
        "(Shutdown)",
        "a reason no table names falls through to its raw word with no phrase above it"
    );
}
