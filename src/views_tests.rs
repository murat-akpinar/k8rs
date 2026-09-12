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
        verb: "scale",
        object: Object::new(
            "deployment",
            Some("payments".to_owned()),
            "web".to_owned(),
            Some("8656c3ec-0f0e-4d0e-9f0b-2a1d3c4b5a69".to_owned()),
        ),
        consequence: "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies."
            .to_owned(),
        warning: None,
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

// --- THE COMMAND LOG ---

/// NOTES § D233: the manifest is the first kind of line, and the panel draws it oldest first —
/// `screens/alerts.md`'s strip at startup.
#[test]
fn the_manifest_is_kept_in_the_order_it_was_handed_over() {
    let mut log = Log::default();
    for line in [
        "$ kubectl get --raw /version",
        "$ kubectl get pods -A --watch",
        "$ kubectl get nodes --watch",
    ] {
        log.ran(line.to_owned());
    }
    assert_eq!(
        log.lines(),
        [
            "$ kubectl get --raw /version",
            "$ kubectl get pods -A --watch",
            "$ kubectl get nodes --watch",
        ]
    );
}

/// NOTES § D257: a read the reader asked for is a line, and `screens/detail.md` draws each of
/// them byte for byte.
#[test]
fn a_detail_tab_spells_the_read_it_asked_for() {
    let web = pod("web-7d9f4", "a-uid");
    assert_eq!(
        describe_line(&web),
        "$ kubectl describe pod web-7d9f4 -n payments"
    );
    assert_eq!(
        events_line("pod", &web, "payments"),
        "$ kubectl events --for pod/web-7d9f4 -n payments"
    );
    assert_eq!(
        yaml_line("pod", &web),
        "$ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields"
    );
}

/// NOTES § D36: `-n ""` is a command that does not work, printed in a record that may not lie.
///
/// **[`events_line`] is the one that still names a namespace, and that is the fix rather than an
/// inconsistency**: `kubectl get node` is cluster-scoped, while a Node's *events* are objects in
/// a namespace the cluster chose — `default` — which is the namespace `k8s::events` sent the
/// request to. This test froze the wrong answer in until 2026-09-07: it asserted the Node's
/// events line carried no `-n`, which on a context scoped to `payments` prints *No resources
/// found in payments namespace* beneath a pane showing the events (`k8s-admin`, 2026-09-07).
///
/// **[`describe_line`] is not here on purpose** — its pane draws a pod and a pod is always
/// namespaced, so a cluster-scoped id is an input it cannot be handed, and asserting what it
/// would print for one is asserting a command nobody should ever see.
#[test]
fn a_cluster_scoped_object_gets_no_namespace_flag() {
    let node = id(ObjectKind::Node, None, "node-3", Some("a-uid"));
    assert_eq!(
        yaml_line("node", &node),
        "$ kubectl get node node-3 -o yaml --show-managed-fields"
    );
    assert_eq!(
        events_line("node", &node, "default"),
        "$ kubectl events --for node/node-3 -n default",
        "a Node's events live where the cluster put them, and the line has to go there too"
    );
}

/// NOTES § D36 — **and the `Option` alone does not get there.** `k8s::text` can hand back
/// `Some("")` from a namespace that was entirely control characters, and a bare `-n ` is *worse*
/// than the `-n ""` that doc argued about: it swallows the next token, so
/// `$ kubectl get pod web -n  -o yaml …` reads `-o` as the namespace (`k8s-admin`, 2026-09-07).
/// Unreachable through today's callers; the record invariant 4 says may not lie does not get to
/// depend on that.
#[test]
fn an_emptied_namespace_names_no_namespace_at_all() {
    let empty = id(ObjectKind::Pod, Some(""), "web", Some("a-uid"));
    assert_eq!(describe_line(&empty), "$ kubectl describe pod web");
    assert_eq!(
        yaml_line("pod", &empty),
        "$ kubectl get pod web -o yaml --show-managed-fields"
    );
    assert_eq!(
        events_line("pod", &empty, ""),
        "$ kubectl events --for pod/web",
        "and the same holds for the namespace the fetch was given"
    );
}

/// `screens/dialogs.md` § While the call is running: the `…` is replaced by the outcome, never
/// removed (NOTES § D20).
#[test]
fn a_mutation_carries_the_running_mark_until_its_outcome_arrives() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());
    assert_eq!(
        log.lines(),
        ["$ kubectl scale deployment/web --replicas=3 -n payments   …"]
    );
    log.outcome("rejected");
    assert_eq!(
        log.lines(),
        ["$ kubectl scale deployment/web --replicas=3 -n payments   → rejected"]
    );
}

/// NOTES § D233 ruling 1: navigation stays free while a mutation is on the wire, so the line that
/// is running is not the last line. Rewriting the last one puts a scale's outcome onto a read.
#[test]
fn the_outcome_lands_on_the_call_that_was_running_and_not_on_the_last_line() {
    let mut log = Log::default();
    log.sent("$ kubectl delete pod/web-7d9f4 -n payments".to_owned());
    log.ran(describe_line(&pod("web-7d9f4", "a-uid")));
    log.outcome("not sent");
    assert_eq!(
        log.lines(),
        [
            "$ kubectl delete pod/web-7d9f4 -n payments   → not sent",
            "$ kubectl describe pod web-7d9f4 -n payments",
        ]
    );
}

/// Invariant 4: an outcome written onto a line no call is attached to is the record lying.
#[test]
fn an_outcome_with_nothing_running_writes_nothing() {
    let mut log = Log::default();
    log.ran("$ kubectl get pods -A --watch".to_owned());
    log.outcome("rejected");
    log.outcome("done");
    assert_eq!(log.lines(), ["$ kubectl get pods -A --watch"]);

    // **A line that ends in `…` and was never `sent` is still not running.** [`Log::ran`] is
    // `pub` and takes an arbitrary `String`, so *the mark is only ever there because `sent` put
    // it there* is an assumption about the caller set and not a property of this type — and it is
    // what an index falling back to the last line rests on. No producer emits a trailing `…`
    // today; the contract sits here rather than on the four that do not (`tester`, 2026-09-07).
    let mut trailing = Log::default();
    trailing.ran("$ kubectl get pods -A --watch …".to_owned());
    trailing.outcome("done");
    assert_eq!(trailing.lines(), ["$ kubectl get pods -A --watch …"]);

    log.sent("$ kubectl rollout restart deployment/web -n payments".to_owned());
    log.outcome("done");
    // The second one has nothing left to answer for and may not edit the line again.
    log.outcome("rejected");
    assert_eq!(
        log.lines()[1],
        "$ kubectl rollout restart deployment/web -n payments   → done"
    );
}

/// NOTES § D257 and `screens/detail.md` § The events fetch could not be completed: **a read the
/// reader asked for carries an outcome exactly the way a mutation does.** The property this panel
/// needs is *does this line get an outcome*, not *is this a mutation* — and until 2026-09-07 the
/// only method that reserved one was documented as mutation-only, which left three approved
/// mockups undrawable (`k8s-admin`, 2026-09-07).
#[test]
fn a_read_the_reader_asked_for_carries_its_outcome() {
    let mut log = Log::default();
    log.sent(events_line("pod", &pod("web-7d9f4", "a-uid"), "payments"));
    assert_eq!(
        log.lines(),
        ["$ kubectl events --for pod/web-7d9f4 -n payments   …"]
    );
    log.outcome("refused");
    assert_eq!(
        log.lines(),
        ["$ kubectl events --for pod/web-7d9f4 -n payments   → refused"]
    );
}

/// And so does a manifest line — `screens/states.md` § Your login expired draws the pods watch
/// answering `→ login expired` long after the manifest was handed over.
#[test]
fn a_manifest_line_carries_an_outcome_too() {
    let mut log = Log::default();
    log.sent("$ kubectl get pods -A --watch".to_owned());
    log.ran("$ kubectl get nodes --watch".to_owned());
    log.outcome("login expired");
    assert_eq!(
        log.lines(),
        [
            "$ kubectl get pods -A --watch   → login expired",
            "$ kubectl get nodes --watch",
        ]
    );
}

/// The security gate's *a Secret value never enters the command log*, reached through the one
/// string on this panel that never passed an ingest strip (NOTES § D217, [`SAID`]).
#[test]
fn an_outcome_is_bounded_and_stripped_however_long_the_cluster_was() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=9 -n payments".to_owned());
    // D217's shape: a `fieldValidation=Strict` rejection hands back the object that was sent.
    let whole_object = format!(
        "Deployment in version \"v1\" cannot be handled: {}",
        "x".repeat(4859)
    );
    log.outcome(&whole_object);
    let line = &log.lines()[0];
    println!("{line}");
    assert_eq!(
        line,
        concat!(
            "$ kubectl scale deployment/web --replicas=9 -n payments",
            "   → Deployment in version \"v1\" canno… (shortened by k8rs)"
        ),
        "the cluster's whole sentence reached the panel"
    );

    // Invariant 9, on the fourth string this file's module doc did not name until 2026-09-07.
    let mut escaped = Log::default();
    escaped.sent("$ kubectl delete pod/web-7d9f4 -n payments".to_owned());
    escaped.outcome("not \u{1b}[2Jsent");
    assert_eq!(
        escaped.lines(),
        ["$ kubectl delete pod/web-7d9f4 -n payments   → not [2Jsent"]
    );
}

/// **An outcome with no word left is not an outcome.** `→ ` with nothing behind it says strictly
/// less than the `…` it would replace, so the mark stays and the line stays resolvable.
#[test]
fn an_empty_outcome_leaves_the_running_mark_alone() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());
    log.outcome("");
    // Everything `k8s::unprintable` removes and nothing it keeps — the `[2J` of an escape
    // sequence is ordinary text once the `ESC` is gone, and an outcome reading `[2J` is a word.
    log.outcome("\u{1b}\u{0}\u{200b}");
    assert_eq!(
        log.lines(),
        ["$ kubectl scale deployment/web --replicas=3 -n payments   …"]
    );
    log.outcome("done");
    assert_eq!(
        log.lines(),
        ["$ kubectl scale deployment/web --replicas=3 -n payments   → done"]
    );
}

/// The security gate's *sizes are bounded*: a session nothing prunes grows for as long as it runs.
///
/// **The numbers are written out, and that is the point of this test rather than a style
/// choice.** Every assertion here read [`KEPT`] back from the implementation until 2026-09-07, so
/// the whole suite asserted *bounded* and never *bounded at a hundred* — `tester` set `KEPT = 3`
/// and all 91 `views` tests stayed green, and `cargo mutants` does not mutate a `const`, so the
/// gate could not see it either.
#[test]
fn the_log_is_bounded_and_drops_the_oldest_first() {
    let mut log = Log::default();
    for line in 0..105 {
        log.ran(format!("$ line {line}"));
    }
    assert_eq!(log.lines().len(), 100);
    assert_eq!(log.lines()[0], "$ line 5");
    assert_eq!(log.lines()[99], "$ line 104");
}

/// The bound moves lines; the one that is running has to move with them, or its outcome lands on
/// somebody else's command.
#[test]
fn the_running_line_keeps_its_outcome_across_the_bound() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());
    for line in 0..KEPT - 1 {
        log.ran(format!("$ line {line}"));
    }
    log.outcome("done");
    assert_eq!(
        log.lines()[0],
        "$ kubectl scale deployment/web --replicas=3 -n payments   → done"
    );
}

/// And where the burst was longer than the whole window, the line it belonged to is gone and the
/// outcome has nowhere true to go.
#[test]
fn an_outcome_whose_line_was_dropped_writes_nothing() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());
    for line in 0..KEPT {
        log.ran(format!("$ line {line}"));
    }
    log.outcome("done");
    assert!(
        !log.lines().iter().any(|line| line.contains("→ done")),
        "an outcome was written onto a command it did not belong to"
    );
    assert_eq!(log.lines().len(), KEPT);
}

/// A `…` that stays claims less than the line under it, never more — the reading `App::may_mutate`
/// makes unreachable and this type still has to be honest about.
#[test]
fn a_second_call_leaves_the_first_running_mark_standing() {
    let mut log = Log::default();
    log.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());
    log.sent("$ kubectl rollout restart deployment/web -n payments".to_owned());
    log.outcome("done");
    assert_eq!(
        log.lines(),
        [
            "$ kubectl scale deployment/web --replicas=3 -n payments   …",
            "$ kubectl rollout restart deployment/web -n payments   → done",
        ]
    );
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

// --- THE FOOTER ---

/// **Five of the sixteen rows of `screens/widgets.md` § 2a's closed list — this box's five** —
/// each against the mockup that draws it: the strings are read off `screens/alerts.md`,
/// `screens/resources.md`, `screens/analysis.md` and `screens/detail.md`, not off what the
/// function happens to return.
///
/// **The other eleven rows are not tested here and are not implemented here.** The eight states'
/// footers (`screens/states.md`), the dialogs' closed sets and the in-flight line
/// (`screens/dialogs.md`), the two pickers (`screens/context.md`, `screens/detail.md`) and the
/// Secret's `v reveal` are each their own box in todo.md § Phase 11 — and each needs an input
/// [`App::footer`] does not take today, which is why none of them fell out of this one for free.
#[test]
fn every_mode_draws_the_footer_its_own_screen_file_draws() {
    let list = "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit";
    let logs = "[ ] tabs  f follow  c container  esc back  ? all keys  q quit";
    let tabbed = "[ ] tabs  esc back  ? all keys  q quit";

    for (view, detail, tab, expected) in [
        // `screens/alerts.md` and `screens/resources.md` — one footer, because both are "a list
        // with a selected object" in the same sense.
        (View::Alerts, false, Tab::Logs, list),
        (View::Resources(0), false, Tab::Logs, list),
        (View::Resources(4), false, Tab::Events, list),
        // `screens/analysis.md` — fixed regardless of which of the seven reports is open.
        (
            View::Analysis(0),
            false,
            Tab::Logs,
            "↑↓ move  ⏎ open  esc back  ? all keys  q quit",
        ),
        (
            View::Analysis(6),
            false,
            Tab::Yaml,
            "↑↓ move  ⏎ open  esc back  ? all keys  q quit",
        ),
        // `screens/detail.md` — the logs tab keeps `f` and `c`; the other three have neither.
        (View::Alerts, true, Tab::Logs, logs),
        (View::Alerts, true, Tab::Describe, tabbed),
        (View::Alerts, true, Tab::Yaml, tabbed),
        (View::Alerts, true, Tab::Events, tabbed),
        // A detail is drawn *over* a view, so which view it was opened from changes nothing.
        (View::Resources(2), true, Tab::Logs, logs),
        (View::Analysis(1), true, Tab::Describe, tabbed),
    ] {
        let app = App {
            view,
            tab,
            ..App::default()
        };
        let (keys, quit) = app.footer(detail, Refused::default());
        assert_eq!(
            (keys.as_ref(), quit),
            (expected, ""),
            "{view:?} · detail {detail} · {tab:?}"
        );
    }
}

/// **The pair that never gives way** (`screens/widgets.md` § 2a): `? all keys` and `q quit`,
/// drawn last and in that order, on every ordinary footer there is.
///
/// **Asserted as a suffix and not as *contains*** — the rule is about where they sit, and a
/// footer that named them first would pass a containment check while drawing the one thing the
/// section forbids.
#[test]
fn the_anchor_pair_ends_every_ordinary_footer() {
    for (view, detail, tab) in [
        (View::Alerts, false, Tab::Logs),
        (View::Resources(0), false, Tab::Logs),
        (View::Analysis(3), false, Tab::Logs),
        (View::Alerts, true, Tab::Logs),
        (View::Alerts, true, Tab::Describe),
        (View::Alerts, true, Tab::Yaml),
        (View::Alerts, true, Tab::Events),
    ] {
        let app = App {
            view,
            tab,
            ..App::default()
        };
        let (keys, quit) = app.footer(detail, Refused::default());
        assert!(
            keys.ends_with("? all keys  q quit"),
            "{view:?} · detail {detail} · {tab:?} — {keys:?} does not end in the anchor pair"
        );
        assert_eq!(quit, "", "an ordinary footer grew a right-hand zone");
    }
}

/// **Help is the one modal that keeps `q quit`, and the only footer with two zones**
/// (`screens/widgets.md` § 2a, `screens/help.md`) — `? all keys` is replaced by the map itself,
/// so the string that points at Help must not survive into Help.
#[test]
fn help_replaces_the_pointer_with_the_map_and_keeps_the_quit() {
    let app = App {
        modal: Some(Modal::Help),
        ..App::default()
    };
    let (keys, quit) = app.footer(false, Refused::default());
    assert_eq!((keys.as_ref(), quit), ("? or esc to close", "q quit"));
    assert!(
        !app.footer(false, Refused::default()).0.contains("all keys"),
        "the footer still pointed at a screen the reader is already on"
    );
}

/// **Help wins over whatever is underneath it, and over a detail tab**, because it is drawn over
/// the whole body region and the keys valid inside it are its own.
#[test]
fn help_is_the_footer_whatever_it_was_opened_from() {
    for (view, detail, tab) in [
        (View::Alerts, false, Tab::Logs),
        (View::Resources(1), false, Tab::Logs),
        (View::Analysis(2), false, Tab::Logs),
        (View::Alerts, true, Tab::Logs),
        (View::Resources(0), true, Tab::Events),
    ] {
        let app = App {
            view,
            tab,
            modal: Some(Modal::Help),
            ..App::default()
        };
        assert_eq!(
            app.footer(detail, Refused::default()),
            (Cow::Borrowed("? or esc to close"), "q quit"),
            "{view:?} · detail {detail} · {tab:?}"
        );
    }
}

/// **`esc` closes Help, and the footer goes back to the mode underneath** — the same one press
/// `App::escape` already documents, seen from the footer's side.
#[test]
fn closing_help_hands_the_footer_back_to_the_mode_underneath() {
    let mut app = App {
        view: View::Analysis(0),
        modal: Some(Modal::Help),
        ..App::default()
    };
    assert_eq!(app.footer(false, Refused::default()).0, "? or esc to close");
    app.escape();
    assert_eq!(
        app.footer(false, Refused::default()).0,
        "↑↓ move  ⏎ open  esc back  ? all keys  q quit"
    );
}

/// **A dialog's footer is its own closed set and the mode underneath is not asked**
/// (`screens/dialogs.md`, which draws one under every box on it). This replaces the test that
/// pinned the hole while `Modal::Confirm` fell through, and the hole is what it says it was: the
/// footers below are the four that page draws.
///
/// **A typed-name dialog answers with what is true right now**, which is what a footer is
/// (`screens/widgets.md` § 2a). Half a name typed is `type the name to enable`; the whole of it
/// is the same `⏎ do it` every other confirmation has, because by then the key the first line
/// names has already been pressed.
#[test]
fn every_dialog_footer_is_the_closed_set_the_screen_file_draws() {
    let armed = |asks: Option<&str>, typed: &str| {
        let mut dialog = dialog(asks);
        dialog.verdict = Some("The cluster checked it first and accepted it.");
        for character in typed.chars() {
            dialog.typed.push(character);
        }
        App {
            modal: Some(Modal::Confirm(dialog)),
            ..App::default()
        }
    };

    for (app, expected) in [
        // **No verdict yet, so neither key is offered** — the dry-run is a real round trip and
        // `esc` is inert until it answers (NOTES § D214). This arm used to read `⏎ do it` over a
        // dim button, naming a key that did nothing; the typed-name arm below already had the
        // two-state shape and now both do.
        (
            App {
                modal: Some(Modal::Confirm(dialog(None))),
                ..App::default()
            },
            "waiting for the cluster",
        ),
        (armed(None, ""), "⏎ do it  esc cancel"),
        (
            armed(Some("web"), ""),
            "type the name to enable  esc cancel",
        ),
        (
            armed(Some("web"), "we"),
            "type the name to enable  esc cancel",
        ),
        // **The armed typed-name footer names the button's own word** (`Dialog::confirm`): the
        // footer read `⏎ do it` while the button beside it read `[ delete ]` — two words for one
        // action, one frame apart.
        (armed(Some("web"), "web"), "⏎ scale  esc cancel"),
        (
            App {
                modal: Some(Modal::Refused {
                    sent: false,
                    fault: crate::k8s::Fault::Rejected,
                    said: None,
                }),
                ..App::default()
            },
            "esc dismiss  ⏎ open",
        ),
        (
            App {
                modal: Some(Modal::Gone {
                    object: dialog(None).object,
                    recreated: true,
                }),
                ..App::default()
            },
            "esc dismiss",
        ),
    ] {
        assert_eq!(app.footer(false, Refused::default()).0, expected);
        assert_eq!(
            app.footer(false, Refused::default()).1,
            "",
            "a dialog grew the right-hand zone only `?` has"
        );
        // **The same answer over an open detail tab**, which is the other side of the hole the
        // replaced test pinned: a dialog is opened from a detail pane as readily as from a list,
        // and a footer that fell through for one of them would fall through for both.
        assert_eq!(app.footer(true, Refused::default()).0, expected);
    }
}

/// **No footer needs the six columns the real floor has over this file's 70-column page**
/// (`screens/widgets.md` § 2a). The ceiling is `ui::indented`'s 76 at 80×24; the mockups are
/// drawn at 66, and the section's claim is that no fixed footer has ever needed the difference.
///
/// **Measured the way ratatui measures**, because `↑↓`, `⏎` and `·` are not one byte each and
/// `str::len` would pass a footer that does not fit.
#[test]
fn no_footer_is_wider_than_the_page_the_mockups_are_drawn_at() {
    let mut seen = 0;
    for (view, detail, tab, modal) in [
        (View::Alerts, false, Tab::Logs, None),
        (View::Resources(0), false, Tab::Logs, None),
        (View::Analysis(0), false, Tab::Logs, None),
        (View::Alerts, true, Tab::Logs, None),
        (View::Alerts, true, Tab::Describe, None),
        (View::Alerts, true, Tab::Yaml, None),
        (View::Alerts, true, Tab::Events, None),
        (View::Alerts, false, Tab::Logs, Some(Modal::Help)),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Confirm(dialog(None))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Confirm(dialog(Some("web")))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: None,
            }),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Gone {
                object: dialog(None).object,
                recreated: true,
            }),
        ),
    ] {
        let app = App {
            view,
            tab,
            modal,
            ..App::default()
        };
        let (keys, quit) = app.footer(detail, Refused::default());
        let width = ratatui::text::Span::raw(keys.as_ref()).width()
            + usize::from(!quit.is_empty())
            + ratatui::text::Span::raw(quit).width();
        assert!(
            width <= 66,
            "{keys:?} + {quit:?} is {width} columns, past the 66 the mockups draw"
        );
        seen += 1;
    }
    assert_eq!(seen, 12, "a mode stopped being measured");
}

/// **`screens/widgets.md` § 2a's own four-row table, read as the fixture** — the footer string and
/// the column count that section counted it at, in the file's order: neither refused · `s` · `r` ·
/// both.
///
/// **The screen file is the fixture, which is the point** (`ui_tests::mockup`'s own reason). A
/// test that compared these four lines with the four literals `App::footer` returns would compare
/// the implementation with itself.
fn mockup_footers() -> Vec<(String, usize)> {
    let path = format!("{}/screens/widgets.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let rows: Vec<(String, usize)> = text
        .lines()
        .skip_while(|line| {
            !line
                .trim_start()
                .starts_with("| State | Alerts / Resources footer")
        })
        .skip(2)
        .take_while(|line| line.trim_start().starts_with('|'))
        .map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
            (
                cells[1].trim().trim_matches('`').to_owned(),
                cells[2]
                    .trim()
                    .parse()
                    .expect("a column count in the last cell"),
            )
        })
        .collect();
    assert_eq!(
        rows.len(),
        4,
        "screens/widgets.md § 2a no longer tabulates the four states of the list footer"
    );
    rows
}

/// **The list footer marks exactly the keys this login may not use, in `screens/widgets.md`
/// § 2a's own four strings and at its own four column counts** (NOTES § D23, § D229).
///
/// **The width is measured the way ratatui measures**, because `↑↓`, `⏎` and `·` are not one byte
/// each — and it is checked against the section's counted number rather than an inequality, so a
/// row that fits but says the wrong thing still fails.
#[test]
fn the_list_footer_marks_the_keys_this_login_may_not_use() {
    let table = mockup_footers();
    let states = [(false, false), (true, false), (false, true), (true, true)];
    for (nth, (scale, restart)) in states.into_iter().enumerate() {
        let refused = Refused::of(
            "deployments",
            [scale.then_some(&Verdict::No); 2],
            [restart.then_some(&Verdict::No)],
            [None],
        );
        let (expected, columns) = &table[nth];
        for view in [View::Alerts, View::Resources(0)] {
            let app = App {
                view,
                ..App::default()
            };
            let (keys, quit) = app.footer(false, refused);
            assert_eq!(
                (keys.as_ref(), quit),
                (expected.as_str(), ""),
                "{view:?} · s refused {scale} · r refused {restart}"
            );
            assert_eq!(
                ratatui::text::Span::raw(keys.as_ref()).width(),
                *columns,
                "{keys:?} is not the width screens/widgets.md § 2a counted"
            );
        }
        // The ceiling `ui::indented` leaves at the 80×24 floor (`screens/widgets.md` § 2a). The
        // 66-column page the mockups are drawn at is not this row's budget: § 2a's own table puts
        // both-refused at 71 and calls it *inside the ceiling with 5 columns to spare*.
        assert!(*columns <= 76, "{expected:?} is {columns} columns");
    }
}

/// **Only `Verdict::No` marks a key — a probe may never be the reason a permitted action is
/// marked** (NOTES § D229 ruling 4). All four answers, for each of the three keys.
///
/// **`None` is two facts and neither may mark**: a probe still in flight, and an operation the
/// selected kind does not support, which is never asked at all (`screens/help.md` § *When a key is
/// refused*). Both reach `Refused::of` as the same absent answer, which is why one case tests
/// both.
///
/// **Each key is set alone**, so a constructor that wired `restart`'s answer into `scale`'s field
/// fails here rather than drawing a plausible screen.
#[test]
fn a_verdict_marks_a_key_only_when_it_is_a_no() {
    let could_not_tell = Verdict::CouldNotTell("k8rs could not read the answer".to_owned());
    for (what, answer, marked) in [
        (
            "no answer yet — in flight, or an operation this kind has not got",
            None,
            false,
        ),
        ("yes", Some(&Verdict::Yes), false),
        ("could not tell", Some(&could_not_tell), false),
        ("no", Some(&Verdict::No), true),
    ] {
        let scale = Refused::of("deployments", [answer; 2], [None], [None]);
        let restart = Refused::of("deployments", [None; 2], [answer], [None]);
        let delete = Refused::of("deployments", [None; 2], [None], [answer]);
        assert_eq!(
            (scale.scale(), scale.restart(), scale.delete()),
            (marked, false, false),
            "s — {what}"
        );
        assert_eq!(
            (restart.scale(), restart.restart(), restart.delete()),
            (false, marked, false),
            "r — {what}"
        );
        assert_eq!(
            (delete.scale(), delete.restart(), delete.delete()),
            (false, false, marked),
            "ctrl-d — {what}"
        );
        assert_eq!(scale.resource(), "deployments", "{what}");
    }
}

/// **An operation is refused when *any* permission it needs came back `Verdict::No`, and lit when
/// none did** (`k8s-admin`, 2026-09-12). Every pair of answers `scale`'s two slots can hold —
/// sixteen — against the one rule, so *granted most of it* is not *lit*.
///
/// **`scale` is the only operation with two slots today and is the whole reason this test
/// exists.** It needs `get` before `patch` on `<plural>/scale` because `ops::scale` reads the
/// current replica count for the dialog's *"Right now: N copies"* sentence; a login granted only
/// `patch` on `deployments/scale` was measured answering the probe yes and then failing the
/// operation with `cannot get resource "deployments/scale"`. The other half of that defect — a
/// clause telling the operator to ask for the grant that does not work — is
/// `ui_tests::every_clause_names_the_plural_its_own_operation_would_send`'s and
/// `ui_tests::all_three_refused_is_the_block_the_screen_file_draws`'s.
///
/// **Both slots are also fed the other three answers**, because *any* is only half the rule: an
/// operation whose every permission came back anything but `No` — including two probes that could
/// not be answered at all — is still lit (NOTES § D229 ruling 4).
#[test]
fn an_operation_is_refused_when_any_one_of_its_permissions_is() {
    let could_not_tell = Verdict::CouldNotTell("k8rs could not read the answer".to_owned());
    let answers = [
        ("not asked", None),
        ("yes", Some(&Verdict::Yes)),
        ("could not tell", Some(&could_not_tell)),
        ("no", Some(&Verdict::No)),
    ];
    let mut seen = 0;
    for (read, get) in answers {
        for (write, patch) in answers {
            let refused = Refused::of("deployments", [get, patch], [None], [None]);
            // The expectation is read off this loop's own table — the word beside the answer —
            // and not re-derived from the `Verdict`, so it is the requirement's sentence and not
            // the implementation's `matches!` written out a second time.
            assert_eq!(
                refused.scale(),
                read == "no" || write == "no",
                "get {read} · patch {write}"
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 16, "an answer stopped being fed to one of the slots");
}

/// **`ctrl-d delete` never reaches a footer, refused or not** (`screens/widgets.md` § 2a): D259
/// took that key off both list footers for width before this box existed, so it is marked only
/// where it is drawn, behind `?`.
#[test]
fn a_refused_delete_changes_no_footer() {
    let refused = Refused::of("deployments", [None; 2], [None], [Some(&Verdict::No)]);
    for view in [View::Alerts, View::Resources(0), View::Analysis(0)] {
        for detail in [false, true] {
            let app = App {
                view,
                ..App::default()
            };
            assert_eq!(
                app.footer(detail, refused),
                app.footer(detail, Refused::default()),
                "{view:?} · detail {detail}"
            );
        }
    }
}

/// **No footer but the two lists' can be marked** — Analysis and the four detail tabs name neither
/// `s` nor `r`, and a modal's footer is its own closed set with the mode underneath not asked
/// (`screens/widgets.md` § 2a). All three refused, which is the loudest input there is.
#[test]
fn a_refusal_reaches_no_footer_that_does_not_draw_the_key() {
    let all = Refused::of(
        "deployments",
        [Some(&Verdict::No); 2],
        [Some(&Verdict::No)],
        [Some(&Verdict::No)],
    );
    let mut seen = 0;
    for (view, detail, tab, modal) in [
        (View::Analysis(0), false, Tab::Logs, None),
        (View::Alerts, true, Tab::Logs, None),
        (View::Alerts, true, Tab::Describe, None),
        (View::Alerts, true, Tab::Yaml, None),
        (View::Alerts, true, Tab::Events, None),
        (View::Alerts, false, Tab::Logs, Some(Modal::Help)),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Confirm(dialog(None))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Confirm(dialog(Some("web")))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: None,
            }),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::Gone {
                object: dialog(None).object,
                recreated: true,
            }),
        ),
    ] {
        let app = App {
            view,
            tab,
            modal,
            ..App::default()
        };
        assert_eq!(
            app.footer(detail, all),
            app.footer(detail, Refused::default()),
            "{:?} · detail {detail} · {tab:?}",
            app.view
        );
        seen += 1;
    }
    assert_eq!(seen, 10, "a footer stopped being measured");
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

/// **`Object::new` refuses an empty `uid`** — the state that is strictly worse than `None`
/// (`k8s::owner_uid`, which already refuses one a layer down).
///
/// `Some("")` sends `preconditions: { uid: "" }`, a `409` no re-read can ever clear, and a
/// `Modal::Gone` check against it flips a healthy object to *Already gone* the instant the dialog
/// opens. `k8s::Row::uid` does not filter, so this is where it is caught.
#[test]
fn an_empty_uid_is_no_uid_at_all() {
    let empty = Object::new("pod", None, "web".to_owned(), Some(String::new()));
    assert_eq!(empty.uid(), None, "an empty uid survived construction");

    let absent = Object::new("pod", None, "web".to_owned(), None);
    assert_eq!(absent.uid(), None);

    let real = Object::new("pod", None, "web".to_owned(), Some("u-1".to_owned()));
    assert_eq!(real.uid(), Some("u-1"), "a real uid was filtered away");
}

/// **The confirm word is spelled once** (`Dialog::confirm`) — the footer and the button beside it
/// read the same action, which they did not: `⏎ do it` under `[ delete ]`.
#[test]
fn the_confirm_word_is_the_same_one_the_footer_and_the_button_use() {
    assert_eq!(dialog(None).confirm(), "do it");
    assert_eq!(
        dialog(Some("web")).confirm(),
        "scale",
        "the verb is the word"
    );

    let mut armed = dialog(Some("web"));
    armed.verdict = Some("the cluster checked it first and accepted it");
    for character in "web".chars() {
        armed.typed.push(character);
    }
    let app = App {
        modal: Some(Modal::Confirm(armed.clone())),
        ..App::default()
    };
    assert!(
        app.footer(false, Refused::default())
            .0
            .contains(armed.confirm()),
        "the footer does not name the button's own word: {:?}",
        app.footer(false, Refused::default()).0
    );
}
