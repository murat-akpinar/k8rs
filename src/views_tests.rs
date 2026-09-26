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

/// **Every state the detail slot can be in** — what a footer sweep has to walk if it is claiming
/// something about *every* mode (NOTES § D270; `tester`, 2026-09-18: the which-pods step was in
/// none of them, so nothing held it to the width, the anchor pair, or the in-flight rule).
const SLOTS: [Detailing; 3] = [
    Detailing::Closed,
    Detailing::Tabs {
        containers: 2,
        from_step: false,
    },
    Detailing::Pods,
];

/// **A detail tab open over the view, or nothing** — over a pod with two containers, so that *a
/// tab is open* and *there is something to pick* stay one answer apart rather than being spelled
/// at every call.
fn opened(detail: bool) -> Detailing {
    match detail {
        true => Detailing::Tabs {
            containers: 2,
            from_step: false,
        },
        false => Detailing::Closed,
    }
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

/// **Invariant 9 on the strings `ui::Screen` takes from its caller** (todo.md § Phase 12): the
/// header's two zones, a note paragraph, the clock sentence, `--namespace` and a command-log line
/// met no strip on the way in, and [`Stripped`] is the type that makes one unavoidable. What a
/// test can hold is the transformation and the bound; that an *unstripped* value cannot reach the
/// field is the private tuple field, which is the compiler's to hold and not this file's.
///
/// **It is `k8s::text` and not [`sanitize`], which differ on exactly one class** — a whitespace
/// control character becomes one space instead of vanishing — and every value this wraps is one
/// line, where removing a `\n` would glue two words into one.
#[test]
fn a_caller_cannot_hand_the_screen_a_string_that_was_not_stripped_and_bounded() {
    // Every class `k8s::text` removes: a bidi override, ESC, NUL, DEL, a C1 control and a
    // zero-width space — the same set `Input::push` refuses one keypress at a time.
    assert_eq!(
        Stripped::of("prod\u{202e}\u{1b}\u{0}\u{7f}\u{9b}\u{200b}-eu"),
        "prod-eu",
        "an unprintable character survived the strip"
    );
    // **And it is a comparison and not a formality** — `prod-eu` and `prod-eu-2` differ by one
    // character, which is the whole reason the header front-cuts (`screens/widgets.md` § 1a).
    assert_ne!(Stripped::of("prod-eu"), "prod-eu-2");

    // **A break becomes one space and is not deleted**, which is the half `sanitize` does
    // differently: a header zone, a note paragraph and a command-log line are each one line, so a
    // deleted `\n` would read as one word and a kept one would forge a second row.
    assert_eq!(Stripped::of("nodes 3/3\nnodes 9/9"), "nodes 3/3 nodes 9/9");
    assert_eq!(
        sanitize("nodes 3/3\nnodes 9/9"),
        "nodes 3/3nodes 9/9",
        "the two strips are two transformations, and this is the difference"
    );

    // **Bounded, which is the security gate's own row about sizes** — nothing between a 50MB
    // annotation and one of these sentences has a length opinion of its own.
    let marker = "… (shortened by k8rs)";
    let huge = Stripped::of(&"c".repeat(FREE_TEXT * 2));
    assert!(
        huge.as_str().len() <= FREE_TEXT + marker.len(),
        "a caller's string was held whole: {} bytes",
        huge.as_str().len()
    );
    assert!(
        huge.as_str().ends_with(marker),
        "and the cut was silent: {:?}",
        huge.as_str()
    );

    // **A blank line is one space as well, and that is what makes `ui::banner`'s split on
    // `"\n\n"` permanently dead for a value of this type** (NOTES § D271). The clock sentence is
    // one paragraph by design (`screens/states.md` § Your computer's clock is off) and
    // `Pane::Denied`, which carries the *server's* sentence and is not one of these, keeps its
    // break.
    assert_eq!(
        Stripped::of("your computer's clock is 4 minutes fast\n\nk8rs reads ages from it"),
        "your computer's clock is 4 minutes fast k8rs reads ages from it",
        "a paragraph break survived into a value that is one line"
    );

    // A value that already came through the ingest strip is untouched, which is what makes this a
    // second door and not a second opinion.
    let ingested = "ctx: prod-eu · ns: payments";
    assert_eq!(Stripped::of(ingested), ingested);
}

/// **The command log's own lines are stripped by the type and no longer by three callers**
/// (todo.md § Phase 12). [`Log::push`] is the single door every one of [`Log::ran`], [`Log::sent`]
/// and [`Log::outcome`]'s rewrite goes through, so a fourth builder cannot arrive without it —
/// which is the whole difference from the three promises this type's doc used to list.
#[test]
fn every_command_log_line_is_stripped_by_the_type_and_not_by_its_callers() {
    let mut log = Log::default();
    log.ran("$ kubectl get pod pay\u{1b}[2Jments\u{202e}".to_owned());
    assert_eq!(
        log.lines()[0],
        "$ kubectl get pod pay[2Jments",
        "a manifest line reached the strip unstripped"
    );

    // A line still waiting for its outcome, and the same line once it has one: both go through
    // `push`, and the second is joined from two halves that have each already been through it
    // rather than re-bounded as a whole ([`Stripped::assembled`]).
    let mut running = Log::default();
    running.sent("$ kubectl scale deployment/we\u{0}b --replicas=3".to_owned());
    assert_eq!(
        running.lines()[0],
        "$ kubectl scale deployment/web --replicas=3   …"
    );
    running.outcome("rejected");
    assert_eq!(
        running.lines()[0],
        "$ kubectl scale deployment/web --replicas=3   → rejected"
    );

    // **A newline is a space here and never a second row** — a command log line that broke in two
    // would draw a command nobody ran on the strip's second line.
    let mut broken = Log::default();
    broken.ran("$ kubectl get pods\n$ kubectl delete pod web".to_owned());
    assert_eq!(
        broken.lines()[0],
        "$ kubectl get pods $ kubectl delete pod web",
        "a line the renderer draws one of held two"
    );
}

/// **An outcome is never cut off a line that fitted while it was running** (`tester`, 2026-09-19).
///
/// [`Log::outcome`] rebuilds the line around the [`RUNNING`] mark, and `→ rejected` is longer than
/// the mark it replaces — so a second [`Stripped::of`] over the result re-bounds a line that was
/// inside [`FREE_TEXT`] a moment before, and the bytes a bound takes are the last ones: the
/// outcome. What that drew was `→… (shortened by k8rs)`, an arrow pointing at k8rs's own
/// shortening mark, which reads as an outcome and is not one.
///
/// **The assertion is the claim and not the arithmetic.** It reads *what became of the call*, which
/// is the thing the bound may not swallow; a length assertion passes just as happily on a line that
/// kept the arrow and lost the word. The `sent` line is asserted to sit exactly *at* the bound
/// first, because a line that fitted with room to spare cannot fail either way.
#[test]
fn an_outcome_is_not_cut_off_a_line_that_fitted_while_it_was_running() {
    let mut tight = Log::default();
    tight.sent("y".repeat(FREE_TEXT - OUTCOME_GAP.len() - RUNNING.len()));
    assert_eq!(
        tight.lines()[0].as_str().len(),
        FREE_TEXT,
        "the line under test is not the one sitting at the bound"
    );
    tight.outcome("rejected");
    let tail = tight.lines()[0].as_str().trim_start_matches('y');
    println!("{tail:?}");
    assert_eq!(
        tail, "   → rejected",
        "the outcome was cut off the line it belongs to"
    );
}

/// **Three columns before an outcome, whatever the caller's line ends in** —
/// `screens/widgets.md` § 2, and [`OUTCOME_GAP`] is the one place it is spelled.
///
/// **The strip substitutes rather than deletes** (NOTES § D198), so composing the gap *before*
/// [`Log::push`] turned a caller's trailing break into a fourth column — and a kubectl line copied
/// out of a file ends in `\n` (`tester`, 2026-09-19). Every spelling of the class is fed rather
/// than the one that was reported (NOTES § D29): the three ASCII breaks, a run of two, and
/// the plain trailing space that could always do this.
///
/// **Plus the four `k8s::text` deliberately keeps** (NOTES § D154, `k8s-admin`, 2026-09-19): NBSP,
/// `\u{2028}`, `\u{2003}` and `\u{3000}` are `char::is_whitespace` and not
/// `crate::k8s::unprintable`, so they reach [`Log::sent`] intact and it is `trim_end` alone that
/// answers for them. They are the shapes the comment on that line was reasoned about rather than
/// fed, which is the half of D29 a same-looking class hides.
///
/// **Both expectations are spelled out rather than built from [`OUTCOME_GAP`]**, because three
/// columns is `screens/widgets.md` § 2's rule and an expectation built from the constant the code
/// reads passes whatever that constant becomes.
///
/// **Both states are asserted, running and resolved.** The gap is drawn by `sent` and *kept* by
/// `outcome`, and a fix that got one right while moving the other would pass a test that only read
/// the first.
#[test]
fn the_gap_before_an_outcome_is_three_columns_whatever_the_line_ends_in() {
    let ran = "$ kubectl get pods";
    for tail in [
        "", "\n", "\t", "\r", "\r\n", "\n\n", " ", "\u{a0}", "\u{2028}", "\u{2003}", "\u{3000}",
    ] {
        let mut log = Log::default();
        log.sent(format!("{ran}{tail}"));
        assert_eq!(
            log.lines()[0],
            "$ kubectl get pods   …",
            "a line ending in {tail:?} drew a gap that is not three columns"
        );
        log.outcome("rejected");
        assert_eq!(
            log.lines()[0],
            "$ kubectl get pods   → rejected",
            "a line ending in {tail:?} lost the gap when its outcome arrived"
        );
    }

    // **The other half of that asymmetry, measured rather than reasoned**: [`Log::ran`] expects no
    // outcome, so it has no gap to defend and does not trim — and `k8s::text` keeps an NBSP
    // (NOTES § D154). The two methods therefore differ over one, which is what the comment on
    // `sent`'s `trim_end` now says (`k8s-admin`, 2026-09-19).
    let mut kept = Log::default();
    kept.ran(format!("{ran}\u{a0}"));
    assert_eq!(
        kept.lines()[0].as_str(),
        format!("{ran}\u{a0}"),
        "`ran` trimmed a character the ingest strip keeps"
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

/// **The one word a field is named by, in the three places a key is worded off it** — the typing
/// footer's label, `esc`'s own second word, and the at-rest line `ui.rs` draws. Asserted against
/// `screens/help.md`'s own `/ n   filter · namespace` row, not against what the match returns.
#[test]
fn a_filter_field_is_named_the_word_the_key_map_names_it() {
    assert_eq!(Typing::Text.label(), "filter");
    assert_eq!(Typing::Namespace.label(), "namespace");
}

/// A buffer with something in it — [`Input`] is only ever filled one character at a time, which
/// is the guard `push` carries, so the tests fill it the same way the keyboard does.
fn buffer(text: &str) -> Input {
    let mut input = Input::default();
    for character in text.chars() {
        input.push(character);
    }
    input
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

/// `screens/states.md` § The filter hides every row — **the word every surface uses for `esc` is
/// read off the field that key really empties**, and [`App::escape`]'s order is what defines it.
#[test]
fn the_field_esc_clears_next_is_text_before_namespace() {
    let mut filters = Filters::default();
    assert_eq!(filters.clears(), None, "nothing set has nothing to clear");
    assert!(!filters.any());

    filters.namespace = buffer("pay");
    assert_eq!(filters.clears(), Some(Typing::Namespace));
    assert!(filters.any());

    filters.text = buffer("web");
    assert_eq!(
        filters.clears(),
        Some(Typing::Text),
        "`esc` would have cleared the outer field first"
    );

    filters.text.clear();
    assert_eq!(filters.clears(), Some(Typing::Namespace));
    filters.namespace.clear();
    assert_eq!(filters.clears(), None);
    assert!(!filters.any());
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
    assert!(app.may_quit() && app.may_switch_cluster());
    // **`(false, true)` is the baseline now, not `(true, true)`** — `s` is withheld from every
    // offer ([`SCALE_IS_BUILT`]), so what a live ordinary frame offers is `r` alone. The rows
    // below are about what the *run* takes away, so each starts from this pair.
    assert_eq!(pressable(&app, ORDINARY), (false, true));

    app.modal = Some(Modal::Help);
    assert!(app.may_quit(), "q was refused merely because help was open");
    assert!(!app.may_switch_cluster(), "X stayed bound under a modal");
    assert_eq!(
        pressable(&app, ORDINARY),
        (false, false),
        "a second dialog could open over the first"
    );

    app.modal = None;
    app.changing = Some(dialog(None).object);
    assert!(!app.may_quit(), "q was allowed mid-write");
    assert!(!app.may_switch_cluster());
    assert_eq!(
        pressable(&app, ORDINARY),
        (false, false),
        "a second mutation was allowed"
    );
}

/// `screens/dialogs.md` § The verdict line: **for `scale` and `restart`, `esc` is inert for as
/// long as a real round trip to the cluster takes** (NOTES § D214's *"`esc` is inert until the
/// verdict arrives"*). [`App::footer`] has named no key beside that box since `1f687fc` and
/// [`App::escape`]'s catch-all arm took the dialog and dropped it anyway — so the press the screen
/// did not offer closed a confirmation with the dry-run still on the wire behind it.
///
/// **The filter underneath is not what the refused press lands on either**: `esc` did not fall
/// through one level, it did nothing, which is what *inert* means.
///
/// **And the moment the verdict lands the same press cancels**, which is what makes this a wait
/// and not a modal that traps the reader.
#[test]
fn escape_is_inert_on_a_confirmation_whose_check_has_not_answered() {
    let mut app = App {
        modal: Some(Modal::Confirm(dialog(None))),
        ..App::default()
    };
    for character in "web".chars() {
        app.filters.text.push(character);
    }

    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert!(
        matches!(app.modal, Some(Modal::Confirm(_))),
        "esc closed a confirmation the cluster had not answered for yet"
    );
    assert_eq!(
        app.filters.text.text(),
        "web",
        "the refused press fell through onto the filter underneath it"
    );

    let Some(Modal::Confirm(mut answered)) = app.modal.take() else {
        unreachable!("the confirmation is the modal that is open");
    };
    answered.verdict = Some("The cluster checked it first and accepted it.");
    app.modal = Some(Modal::Confirm(answered));

    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert!(
        app.modal.is_none(),
        "esc did not cancel the dialog once its verdict had arrived"
    );
    assert_eq!(
        app.filters.text.text(),
        "web",
        "the press that cancelled the dialog cleared a filter with it"
    );
}

/// `screens/widgets.md` § 5: **`esc` closes exactly one level, always** — and the two filters are
/// two levels, so one press clears one of them (NOTES § D246). Scoped to `n payments` and then
/// narrowed with `/web`, one `esc` used to jump all the way back to every namespace in the
/// cluster: one press undoing two.
///
/// **That *always* has one named exception, and that page now names it itself** — a `Confirm`
/// whose check has not answered, which is the test above; the dialog this one opens is therefore
/// one whose check has.
#[test]
fn escape_closes_one_level_per_press_and_never_two() {
    let mut app = App::default();
    for character in "payments".chars() {
        app.filters.namespace.push(character);
    }
    for character in "web".chars() {
        app.filters.text.push(character);
    }
    // **Answered, because a confirmation whose check is still out is the one modal `esc` does
    // not close** ([`Dialog::waiting`], the test below) — and the subject here is levels.
    let mut answered = dialog(None);
    answered.verdict = Some("The cluster checked it first and accepted it.");
    app.modal = Some(Modal::Confirm(answered));

    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert!(app.modal.is_none(), "esc did not close the modal");
    assert_eq!(
        app.filters.text.text(),
        "web",
        "esc closed the modal and cleared a filter in one press"
    );

    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert!(
        app.filters.text.is_empty(),
        "esc did not clear the text filter"
    );
    assert_eq!(
        app.filters.namespace.text(),
        "payments",
        "one press dropped the text filter and the namespace scope together"
    );

    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
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
    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert!(app.filters.namespace.is_empty());
}

// --- THE CLUSTER PICKER ---

/// **The rows come out of `k8s::contexts` over a kubeconfig this file wrote**, never a hand-built
/// `Choice`: which row is shadowed, undefined, current or derived is that function's answer, and a
/// literal would be this file's guess at it.
fn listed(yaml: &str) -> Vec<Choice> {
    crate::k8s::contexts(
        &kube::config::Kubeconfig::from_yaml(yaml).expect("a kubeconfig this file wrote itself"),
        None,
    )
}

/// The picker opened over `prod-eu` while it is live — the ordinary `X`.
fn live() -> Connection {
    Connection::Live(Some("prod-eu".to_owned()))
}

/// **`X` after a switch away from `prod-eu` failed** — nothing is live, and `prod-eu` is the
/// context that last was (NOTES § D264 ruling 15).
fn dropped() -> Connection {
    Connection::Dropped(Some("prod-eu".to_owned()))
}

/// Six contexts, one of each thing the cursor has a rule about, in this order: **0** `prod-eu`,
/// current · **1** `old-cluster`, whose cluster the file does not define · **2** `staging`, whose
/// name carries a zero-width space the drawn name does not · **3** `prod-eu` again, shadowed ·
/// **4** `kind-k8rs` · **5** `dev-cluster`.
const SIX: &str = "apiVersion: v1\n\
     kind: Config\n\
     current-context: prod-eu\n\
     clusters:\n\
     - {name: prod, cluster: {server: 'https://prod-eu.example:6443'}}\n\
     - {name: staging, cluster: {server: 'https://staging.invalid:6443'}}\n\
     - {name: kind, cluster: {server: 'https://kind.invalid:41234'}}\n\
     - {name: dev, cluster: {server: 'dev.gr7.eu-west-1.eks.amazonaws.com'}}\n\
     contexts:\n\
     - {name: prod-eu, context: {cluster: prod, user: u, \
       extensions: [{name: k8rs, extension: {tag: 'aws · prod'}}]}}\n\
     - {name: old-cluster, context: {cluster: nowhere, user: u}}\n\
     - {name: \"stag\\u200Bing\", context: {cluster: staging, user: u}}\n\
     - {name: prod-eu, context: {cluster: staging, user: u}}\n\
     - {name: kind-k8rs, context: {cluster: kind, user: u}}\n\
     - {name: dev-cluster, context: {cluster: dev, user: u}}\n\
     users: [{name: u, user: {token: k8rs-tests-fake-static-token}}]\n";

/// **The fixture is what its doc says**, so a test below that reads *row 1 is skipped* is reading
/// a row that is really undefined rather than one this file assumed was.
#[test]
fn the_picker_fixture_holds_one_row_of_every_shape() {
    let rows = listed(SIX);
    assert_eq!(rows.len(), 6);
    assert!(rows[0].current && !rows[0].shadowed);
    assert_eq!(rows[1].server, Address::Undefined);
    assert_eq!(
        (rows[2].name.as_deref(), rows[2].key.as_str()),
        (Some("staging"), "stag\u{200b}ing")
    );
    assert!(rows[3].shadowed && !rows[3].current);
    assert_eq!(rows[4].tag, Tag::Derived("local"));
    assert_eq!(rows[5].tag, Tag::Derived("aws"));
    assert!(rows.iter().filter(|row| row.current).count() == 1);
}

/// **Opened on `(current)`, and the cursor skips an undefined row while landing on a shadowed
/// one** (`screens/context.md` § A context whose cluster the file does not define, § A context
/// defined twice). Both ends clamp.
#[test]
fn the_cursor_skips_an_undefined_row_and_lands_on_a_shadowed_one() {
    let rows = listed(SIX);
    let mut picker = Picker::new(&rows, live());
    assert_eq!(
        picker.selected(&rows),
        Some(0),
        "the picker did not open on (current)"
    );

    picker.down(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(2),
        "↓ landed on the undefined row"
    );
    picker.down(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(3),
        "↓ skipped the shadowed row"
    );
    picker.down(&rows);
    picker.down(&rows);
    picker.down(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(5),
        "↓ did not clamp at the last row"
    );

    for _ in 0..3 {
        picker.up(&rows);
    }
    assert_eq!(picker.selected(&rows), Some(2));
    picker.up(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(0),
        "↑ landed on the undefined row"
    );
    picker.up(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(0),
        "↑ did not clamp at the first row"
    );

    // **A duplicate whose own entry names no cluster is still landed on** — the one shape where
    // *shadowed* and *undefined* are both true, and where skipping it would hide the only row
    // that says the file has a duplicate.
    let both = SIX.replace(
        "- {name: prod-eu, context: {cluster: staging, user: u}}",
        "- {name: prod-eu, context: {cluster: nowhere, user: u}}",
    );
    let rows = listed(&both);
    assert!(rows[3].shadowed && rows[3].server == Address::Undefined);
    let mut picker = Picker::new(&rows, live());
    picker.down(&rows);
    picker.down(&rows);
    assert_eq!(
        picker.selected(&rows),
        Some(3),
        "a duplicate was skipped because its own cluster is undefined"
    );
}

/// **No row is selected when no row is both current and landable, and `⏎` does nothing until one
/// is** (`screens/context.md` § No row is both current and landable, NOTES § D264 ruling 3). A
/// `current-context` naming an undefined entry, one `kubectl config delete-context` left dangling,
/// and none at all each opened on row 0, where one keypress connected to a context nobody named.
/// `↓` then selects the first row the cursor may land on and `↑` the last.
#[test]
fn no_row_is_selected_when_none_is_both_current_and_landable() {
    for (shape, yaml) in [
        (
            "an undefined current-context",
            SIX.replace("current-context: prod-eu", "current-context: old-cluster"),
        ),
        (
            "a deleted current-context",
            SIX.replace("current-context: prod-eu", "current-context: deleted"),
        ),
        (
            "no current-context",
            SIX.replace("current-context: prod-eu\n", ""),
        ),
    ] {
        let rows = listed(&yaml);
        for connection in [Connection::Never, dropped(), live()] {
            let picker = Picker::new(&rows, connection);
            assert_eq!(picker.selected(&rows), None, "{shape}: a row was selected");
            assert!(
                matches!(picker.chosen(&rows), Chosen::Stay),
                "{shape}: ⏎ did something with no row selected"
            );
            assert!(picker.inert(&rows), "{shape}");

            let mut down = picker.clone();
            down.down(&rows);
            assert_eq!(down.selected(&rows), Some(0), "{shape}: ↓");
            let mut up = picker;
            up.up(&rows);
            assert_eq!(up.selected(&rows), Some(5), "{shape}: ↑");
            assert!(
                !up.inert(&rows),
                "{shape}: ↑ selected a row ⏎ still refuses"
            );
        }
    }
    assert!(
        listed(&SIX.replace("current-context: prod-eu", "current-context: old-cluster"))[1].current,
        "the fixture edit did not move current-context"
    );

    // **One context, and its cluster undefined, stays unselected under both keys** — the page's
    // *current-context itself with only that one context present* — and so does an empty list.
    let rows = listed(&SIX.replace("cluster: prod,", "cluster: nowhere,"));
    assert!(rows[0].current && !landable(&rows[0]));
    for rows in [&rows[..1], &[][..]] {
        let mut picker = Picker::new(rows, Connection::Never);
        picker.down(rows);
        assert_eq!(picker.selected(rows), None);
        picker.up(rows);
        assert_eq!(picker.selected(rows), None);
    }
}

/// **What `⏎` means, row by row** (`screens/context.md`): `(current)` closes only while it is live
/// — after a failed switch it is the retry — a shadowed row stays open, and every other row
/// connects to **the row itself**, whose key is the file's own spelling and not the drawn name
/// (NOTES § D264 rulings 4 and 8).
#[test]
fn enter_closes_only_on_a_live_current_row_and_connects_to_the_row() {
    let rows = listed(SIX);
    assert!(matches!(
        Picker::new(&rows, live()).chosen(&rows),
        Chosen::Close
    ));
    for (connection, expected) in [
        (
            Connection::Never,
            Before::Picking(Picker::new(&rows, Connection::Never)),
        ),
        (dropped(), Before::Connected(Some("prod-eu".to_owned()))),
    ] {
        match Picker::new(&rows, connection.clone()).chosen(&rows) {
            Chosen::Connect { row, before } => {
                assert_eq!(row.key, "prod-eu", "{connection:?}");
                assert_eq!(
                    before, expected,
                    "{connection:?}: the failure would not go back to what is behind it"
                );
            }
            _ => panic!("{connection:?}: ⏎ on a (current) nothing is connected to did not connect"),
        }
    }

    let mut switching = Picker::new(&rows, live());
    switching.down(&rows);
    match switching.chosen(&rows) {
        Chosen::Connect { row, before } => {
            assert_eq!(
                (row.key.as_str(), row.name.as_deref()),
                ("stag\u{200b}ing", Some("staging")),
                "⏎ did not hand back the row, with the spelling a lookup in the file finds"
            );
            assert_eq!(
                before,
                Before::Connected(Some("prod-eu".to_owned())),
                "the failure would not name the cluster that was live"
            );
        }
        _ => panic!("⏎ on staging did not connect"),
    }

    let mut starting = Picker::new(&rows, Connection::Never);
    starting.down(&rows);
    match starting.chosen(&rows) {
        Chosen::Connect {
            before: Before::Picking(tried),
            ..
        } => assert_eq!(
            tried.selected(&rows),
            Some(2),
            "the list would reopen on a row that was not the one tried"
        ),
        _ => panic!("⏎ on staging at startup did not connect"),
    }

    for mut picker in [switching, starting] {
        picker.down(&rows);
        assert_eq!(picker.selected(&rows), Some(3));
        assert!(
            matches!(picker.chosen(&rows), Chosen::Stay),
            "⏎ on a duplicate did something"
        );
        assert!(picker.inert(&rows));
    }
}

/// **Which variant is drawn is read off what has connected in this run, never off who opened the
/// picker** (NOTES § D264 ruling 4): the startup words hold across a failed first attempt and the
/// window before its answer, and `X` after a failed switch keeps the mid-session ones.
#[test]
fn the_variant_is_what_has_connected_and_not_who_opened_the_picker() {
    let rows = listed(SIX);
    for (connection, startup, verbs) in [
        (Connection::Never, true, ("connect", "quit")),
        (dropped(), false, ("switch", "cancel")),
        (live(), false, ("switch", "cancel")),
    ] {
        let picker = Picker::new(&rows, connection.clone());
        assert_eq!(picker.startup(), startup, "{connection:?}");
        assert_eq!(picker.verbs(), verbs, "{connection:?}");
    }
}

/// **A switch that fails after the run has connected names the context that was last live, however
/// many fail in a row** (NOTES § D264 ruling 15). The chain is the one Phase 12's caller walks: `X`
/// over live `prod-eu`, `⏎` on `staging`, which fails; `X` again, now over nothing live, `⏎` on
/// `kind-k8rs`, which fails; and a third time on the `(current)` row, which is a retry and not a
/// close. Every failure answers `prod-eu` — never `Picking`, whose box says *nothing has connected
/// yet* to a reader who was on `prod-eu` a minute ago. **A run that never connected answers
/// `Picking` every time**, the other half.
#[test]
fn a_failed_switch_names_the_last_live_context_however_many_fail_in_a_row() {
    let rows = listed(SIX);
    let mut connection = live();
    for (attempt, down) in [(1, 1), (2, 3), (3, 0)] {
        let mut picker = Picker::new(&rows, connection.clone());
        for _ in 0..down {
            picker.down(&rows);
        }
        let Chosen::Connect {
            before: Before::Connected(last),
            ..
        } = picker.chosen(&rows)
        else {
            panic!("attempt {attempt}: ⏎ over {connection:?} did not answer a live context");
        };
        assert_eq!(
            last.as_deref(),
            Some("prod-eu"),
            "attempt {attempt}: the failure does not name the context that was last live"
        );
        connection = Connection::Dropped(last);
    }

    let mut picker = Picker::new(&rows, Connection::Never);
    picker.down(&rows);
    for attempt in 1..=3 {
        let Chosen::Connect {
            before: Before::Picking(tried),
            ..
        } = picker.chosen(&rows)
        else {
            panic!("attempt {attempt}: a run that never connected named a context as fine");
        };
        assert!(tried.startup(), "attempt {attempt}");
        picker = tried;
    }
}

/// **A picker with no row it may land on offers no key that moves** (NOTES § D264 rulings 16 and
/// 18): every context names a cluster the file does not define, `/` left only such rows, `/` hid
/// every row, or the kubeconfig has no contexts at all. `↑`/`↓` select nothing and `⏎` stays inert.
/// **A list with a row it may land on is never this** — a picker with no row selected that has one
/// to find, a selected shadowed row, a filter that keeps a landable row beside an undefined one.
#[test]
fn a_picker_with_no_row_to_land_on_is_nowhere_and_only_then() {
    let undefined = listed(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: dev-cluster\n\
         contexts:\n\
         - {name: dev-cluster, context: {cluster: dev, user: u}}\n\
         - {name: old-cluster, context: {cluster: old, user: u}}\n\
         users: [{name: u, user: {token: k8rs-tests-fake-static-token}}]\n",
    );
    assert!(undefined.iter().all(|row| row.server == Address::Undefined));
    let rows = listed(SIX);
    let none: Vec<Choice> = Vec::new();
    let typed = |mut picker: Picker, text: &str| {
        for character in text.chars() {
            picker.filter.push(character);
        }
        picker
    };
    for (what, picker, contexts) in [
        (
            "every row undefined, at startup",
            Picker::new(&undefined, Connection::Never),
            &undefined,
        ),
        (
            "every row undefined, on X",
            Picker::new(&undefined, live()),
            &undefined,
        ),
        (
            "a filter that shows only an undefined row",
            typed(Picker::new(&rows, live()), "old"),
            &rows,
        ),
        (
            "a filter that hides every row",
            typed(Picker::new(&rows, live()), "zzz"),
            &rows,
        ),
        (
            "a kubeconfig with no contexts",
            Picker::new(&none, live()),
            &none,
        ),
    ] {
        assert!(picker.nowhere(contexts), "{what}");
        assert!(picker.inert(contexts), "{what}");
        let mut moved = picker.clone();
        moved.down(contexts);
        moved.up(contexts);
        assert_eq!(
            moved.selected(contexts),
            None,
            "{what}: a key moved the cursor"
        );
    }

    let dangling = listed(&SIX.replace("current-context: prod-eu", "current-context: deleted"));
    let mut shadowed = Picker::new(&rows, live());
    for _ in 0..2 {
        shadowed.down(&rows);
    }
    for (what, picker, contexts) in [
        (
            "no row selected, one to find",
            Picker::new(&dangling, live()),
            &dangling,
        ),
        ("a shadowed row selected", shadowed, &rows),
        (
            "a filter that shows an undefined row beside a landable one",
            typed(Picker::new(&rows, live()), "cluster"),
            &rows,
        ),
        (
            "the same list with nothing typed",
            typed(Picker::new(&rows, live()), ""),
            &rows,
        ),
    ] {
        assert!(!picker.nowhere(contexts), "{what}");
    }
}

/// **`/` matches what the row draws and nothing else** — the name as drawn, `(unnamed)` included,
/// the tag as drawn, `~` included — never the server or a badge (NOTES § D264 ruling 7). The cursor
/// is never left on a row it hid, nor on one it may not land on, and a filter nothing matches
/// leaves `⏎` inert.
#[test]
fn a_filter_matches_what_the_row_draws_and_takes_the_cursor_with_it() {
    let rows = listed(SIX);
    let mut picker = Picker::new(&rows, live());
    for _ in 0..3 {
        picker.down(&rows);
    }
    assert_eq!(picker.selected(&rows), Some(4));
    let typed = |picker: &mut Picker, text: &str| {
        picker.filter.clear();
        for character in text.chars() {
            picker.filter.push(character);
        }
    };

    typed(&mut picker, "PROD");
    assert_eq!(
        picker.shown(&rows),
        [0, 3],
        "the name is not matched case-blind"
    );
    assert_eq!(
        picker.selected(&rows),
        Some(0),
        "the cursor stayed on a row the filter hid"
    );
    for (text, expected) in [
        ("~loc", &[4][..]),
        ("~", &[4, 5]),
        ("aws", &[0, 5]),
        ("aws · p", &[0]),
        ("cluster", &[1, 5]),
    ] {
        typed(&mut picker, text);
        assert_eq!(picker.shown(&rows), expected, "/{text}");
    }
    for text in ["6443", "invalid", "https", "current", "duplicate", "TLS"] {
        typed(&mut picker, text);
        assert!(
            picker.shown(&rows).is_empty(),
            "/{text} matched something the row does not draw as its name or tag"
        );
    }
    let unnamed = listed(&SIX.replace("name: kind-k8rs", "name: \"\\u200B\""));
    assert_eq!(
        unnamed[4].name, None,
        "the fixture edit did not strip a name"
    );
    let mut blank = Picker::new(&unnamed, live());
    typed(&mut blank, "unnamed");
    assert_eq!(
        blank.shown(&unnamed),
        [4],
        "(unnamed) is drawn and not matched"
    );

    typed(&mut picker, "old");
    assert_eq!(picker.shown(&rows), [1], "an undefined row is still drawn");
    assert_eq!(
        picker.selected(&rows),
        None,
        "the cursor landed on an undefined row"
    );
    assert!(picker.inert(&rows));

    typed(&mut picker, "nothing-is-called-this");
    assert!(picker.shown(&rows).is_empty());
    picker.down(&rows);
    picker.up(&rows);
    assert!(picker.inert(&rows));

    picker.filter.clear();
    assert_eq!(
        picker.selected(&rows),
        Some(4),
        "clearing the filter lost the row the cursor was put on"
    );
}

/// **`esc` on the picker: the typed filter first, then the picker** — which on `X` goes back to
/// what is behind it and at startup ends the run, because nothing is. A failure's `esc` goes where
/// the `Before` [`Picker::chosen`] built says: dismissed over a live cluster, back to the list
/// otherwise — never a dead end (NOTES § D264 ruling 4).
#[test]
fn escape_clears_the_filter_then_cancels_or_quits_and_a_failure_goes_back_where_it_came_from() {
    let rows = listed(SIX);
    for connection in [live(), dropped()] {
        let mut switching = App {
            modal: Some(Modal::ContextPick(Picker::new(&rows, connection.clone()))),
            ..App::default()
        };
        if let Some(Modal::ContextPick(picker)) = &mut switching.modal {
            picker.filter.push('s');
        }
        assert!(!switching.escape(Detailing::Closed));
        assert!(
            matches!(
                &switching.modal,
                Some(Modal::ContextPick(picker)) if picker.filter.is_empty()
            ),
            "{connection:?}: esc closed the picker with a filter still typed into it"
        );
        assert!(
            !switching.escape(Detailing::Closed),
            "{connection:?}: esc on X's picker ended the run"
        );
        assert!(switching.modal.is_none(), "esc did not cancel the picker");
    }

    let mut starting = App {
        modal: Some(Modal::ContextPick(Picker::new(&rows, Connection::Never))),
        ..App::default()
    };
    if let Some(Modal::ContextPick(picker)) = &mut starting.modal {
        picker.filter.push('s');
    }
    assert!(
        !starting.escape(Detailing::Closed),
        "esc over a typed filter quit at startup"
    );
    let before = starting.clone();
    assert!(
        starting.escape(Detailing::Closed),
        "esc on the startup picker did not end the run"
    );
    assert_eq!(
        starting, before,
        "the esc that ends the run changed the screen it ends on"
    );

    let failed = |connection: Connection| {
        let mut picker = Picker::new(&rows, connection);
        picker.down(&rows);
        let Chosen::Connect { before, .. } = picker.chosen(&rows) else {
            panic!("⏎ on staging did not connect");
        };
        App {
            modal: Some(Modal::Unconnected {
                to: Some("staging".to_owned()),
                before,
                sent: true,
                fault: Fault::Refused,
                said: None,
                coverage: Coverage::Cluster,
                renewal: None,
            }),
            ..App::default()
        }
    };
    let mut app = failed(Connection::Never);
    assert!(
        !app.escape(Detailing::Closed),
        "esc on a failure ended the run"
    );
    assert!(
        matches!(
            &app.modal,
            Some(Modal::ContextPick(picker))
                if picker.selected(&rows) == Some(2) && picker.startup()
        ),
        "esc did not go back to the startup list it came from, on the row tried"
    );
    // **A failure after something has connected is dismissed, whether that context was still live
    // or a switch had already failed** (NOTES § D264 ruling 15) — the way back is `X`.
    for connection in [live(), dropped()] {
        let mut dismissed = failed(connection.clone());
        assert!(!dismissed.escape(Detailing::Closed), "{connection:?}");
        assert!(
            dismissed.modal.is_none(),
            "{connection:?}: esc did not dismiss the failure"
        );
    }
}

/// **Nothing the reader did on the old cluster survives a switch** (`screens/context.md`
/// § What happens on `⏎` step 5).
///
/// **The command log is not part of it, and that is the claim this test changed to make**
/// (NOTES § D280 item 2): a switch is *asked for* here, before the outcome is known, and a strip
/// emptied at that moment draws blank for as long as `connect_with` is out — then has nothing true
/// to put there at all if the connect failed, since `sent` is `false` for every fault that box can
/// draw. The strip keeps what was already there until a new context has connected, and the caller
/// that learns it did is the one that empties it (`screens/context.md` § When the new cluster does
/// not work).
#[test]
fn a_switch_puts_the_view_back_on_alerts_and_leaves_the_command_log_alone() {
    let rows = listed(SIX);
    let mut app = App {
        view: View::Resources(7),
        expanded: Some(Group::Network),
        tab: Tab::Yaml,
        scroll: [40, 40, 40, 40],
        following: true,
        modal: Some(Modal::ContextPick(Picker::new(&rows, live()))),
        ..App::default()
    };
    app.content.select(3, &[None, None, None, None]);
    app.nav.select(1, &[None, None]);
    app.filters.text.push('w');
    app.filters.namespace.push('p');
    let mut log = Log::default();
    log.ran(GET_CONTEXTS.to_owned());
    log.sent("$ kubectl get pods -A --watch".to_owned());
    let before = log.clone();

    app.switched();
    assert_eq!(
        app,
        App::default(),
        "something of the old cluster's survived the switch"
    );
    assert_eq!(
        log, before,
        "the switch emptied a strip the screen says keeps its last line"
    );
    // **And the line that was waiting is still the one an outcome lands on** — `Log::sent` marked
    // it, nothing has replaced it, so the record cannot come apart (invariant 4).
    log.outcome("not allowed");
    assert!(
        log.lines()
            .last()
            .is_some_and(|line| line.as_str().contains("not allowed")),
        "the outcome had nowhere true to go: {:?}",
        log.lines()
    );
}

/// **Only a picker nothing has connected behind, and the failure it led to, are drawn over
/// nothing** — the frame draws no sidebar and no vitals for exactly these two.
#[test]
fn only_a_picker_with_nothing_connected_and_its_failure_are_drawn_over_nothing() {
    let rows = listed(SIX);
    let failed = |before| App {
        modal: Some(Modal::Unconnected {
            to: None,
            before,
            sent: false,
            fault: Fault::Unanswered,
            said: None,
            coverage: Coverage::Cluster,
            renewal: None,
        }),
        ..App::default()
    };
    let over = |modal| App {
        modal: Some(modal),
        ..App::default()
    };
    assert!(over(Modal::ContextPick(Picker::new(&rows, Connection::Never))).connecting_first());
    assert!(failed(Before::Picking(Picker::new(&rows, Connection::Never))).connecting_first());
    assert!(!failed(Before::Picking(Picker::new(&rows, dropped()))).connecting_first());
    assert!(!over(Modal::ContextPick(Picker::new(&rows, dropped()))).connecting_first());
    assert!(!over(Modal::ContextPick(Picker::new(&rows, live()))).connecting_first());
    assert!(!failed(Before::Connected(None)).connecting_first());
    assert!(!over(Modal::Help).connecting_first());
    assert!(!App::default().connecting_first());
}

/// **`X` is unbound while the picker or its failure is open** (NOTES § D16 ruling 1) — a picker
/// over a picker is the stacking the modal enum exists to refuse.
#[test]
fn the_switcher_is_refused_under_its_own_picker_and_its_own_failure() {
    let rows = listed(SIX);
    for modal in [
        Modal::ContextPick(Picker::new(&rows, live())),
        unconnected(Before::Connected(None)),
    ] {
        let app = App {
            modal: Some(modal),
            ..App::default()
        };
        assert!(!app.may_switch_cluster());
        assert_eq!(pressable(&app, ORDINARY), (false, false));
    }
}

/// A `403` on staging's pods, over whatever `before` says was behind it.
fn unconnected(before: Before) -> Modal {
    Modal::Unconnected {
        to: Some("staging".to_owned()),
        before,
        sent: true,
        fault: Fault::Refused,
        said: None,
        coverage: Coverage::Cluster,
        renewal: None,
    }
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
    let line = log.lines()[0].as_str();
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
        !log.lines()
            .iter()
            .any(|line| line.as_str().contains("→ done")),
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
    assert_eq!(app.scroll[Tab::Logs.at()], 5);
    assert!(!app.following, "follow mode survived a manual scroll");

    app.scroll_by(-99);
    assert_eq!(
        app.scroll[Tab::Logs.at()],
        0,
        "the offset went below the top of the buffer"
    );
}

/// `screens/widgets.md` § 4: **one offset per tab, not one shared by all four** — and the tab that
/// is open is the only one a scroll may move (NOTES § D272 § 1, `tester`'s measured case: yaml at
/// row 400, over to a three-row tab, back to yaml used to read row 0).
#[test]
fn a_scroll_moves_the_open_tabs_own_offset_and_leaves_the_other_three() {
    let mut app = App {
        tab: Tab::Yaml,
        ..App::default()
    };
    app.scroll_by(400);
    assert_eq!(app.scroll[Tab::Yaml.at()], 400);
    for tab in [Tab::Logs, Tab::Describe, Tab::Events] {
        assert_eq!(
            app.scroll[tab.at()],
            0,
            "{} moved with yaml's own offset",
            tab.label()
        );
    }

    // The other three keep their own places while yaml keeps its four hundredth line.
    app.tab = Tab::Describe;
    app.scroll_by(3);
    assert_eq!(app.scroll[Tab::Describe.at()], 3);
    assert_eq!(
        app.scroll[Tab::Yaml.at()],
        400,
        "describe's own scroll clamped yaml's offset"
    );
}

/// `screens/widgets.md` § 4: **a resize discards all four, back to the top of whichever tab redraws
/// next** — and follow mode is untouched, because a followed pane ignores its stored offset every
/// frame (`screens/detail.md` § Switching tabs keeps your place, last paragraph).
#[test]
fn a_rewind_empties_every_tabs_offset_and_leaves_follow_mode_alone() {
    let mut app = App {
        scroll: [11, 22, 33, 44],
        following: true,
        ..App::default()
    };
    app.rewound();
    assert_eq!(app.scroll, [0, 0, 0, 0]);
    assert!(app.following, "a rewind turned follow mode off");
}

/// `screens/widgets.md` § 2b — **`/` and `n` open on the field they name and nothing else is a
/// command while one has focus.** `s`, `r`, `q`, `X` and `?` are letters a filter can hold, so the
/// three questions that decide whether a key fires all answer no through one field.
#[test]
fn no_key_is_a_command_while_a_filter_is_being_typed() {
    for field in [Typing::Text, Typing::Namespace] {
        let mut app = App::default();
        assert!(app.may_quit() && app.may_switch_cluster());
        assert_eq!(pressable(&app, ORDINARY), (false, true));

        app.typing = Some(field);
        assert!(!app.may_quit(), "q quit while typing {field:?}");
        assert!(!app.may_switch_cluster(), "X while typing {field:?}");
        assert_eq!(
            pressable(&app, ORDINARY),
            (false, false),
            "s or r would have fired while typing {field:?}"
        );
    }
}

/// **The buffer `/` or `n` has focus on is the filter itself**, never a third copy that would have
/// to be written back — which is what lets the list narrow one keystroke at a time.
#[test]
fn the_focused_buffer_is_the_filter_field_itself() {
    let mut app = App::default();
    assert!(app.typed().is_none(), "nothing has focus while browsing");
    assert!(app.typed_mut().is_none());

    app.typing = Some(Typing::Text);
    app.typed_mut().expect("`/` has focus").push('w');
    assert_eq!(app.filters.text.text(), "w");
    assert!(app.filters.namespace.is_empty(), "`n` was written into");

    app.typing = Some(Typing::Namespace);
    app.typed_mut().expect("`n` has focus").push('p');
    assert_eq!(app.filters.namespace.text(), "p");
    assert_eq!(app.typed().map(Input::text), Some("p"));
    assert_eq!(
        app.filters.text.text(),
        "w",
        "focus moved and took the other buffer with it"
    );
}

/// `screens/widgets.md` § 2b — **`esc` while typing acts on the field that has focus**: it empties
/// a buffer that has something in it and stays open, and it closes the session once that buffer is
/// already empty, leaving the *other* field exactly as it was. That last clause is the one a
/// global order would get wrong.
#[test]
fn esc_while_typing_empties_the_focused_field_then_closes_the_session() {
    let mut app = App {
        filters: Filters {
            text: buffer("web"),
            namespace: buffer("pay"),
        },
        typing: Some(Typing::Namespace),
        ..App::default()
    };

    assert!(!app.escape(Detailing::Closed));
    assert!(app.filters.namespace.is_empty(), "`n` was not emptied");
    assert_eq!(
        app.filters.text.text(),
        "web",
        "`esc` reached a field that did not have focus"
    );
    assert_eq!(app.typing, Some(Typing::Namespace), "typing closed early");

    assert!(!app.escape(Detailing::Closed));
    assert_eq!(app.typing, None, "typing stayed open over an empty buffer");
    assert_eq!(
        app.filters.text.text(),
        "web",
        "closing the session cleared the other field"
    );
}

/// **A filter does not survive into a view that cannot draw it** (`screens/widgets.md` § 2b): the
/// list is *changing*, not being drawn over, and `web` typed for Alerts' cards means nothing
/// against a ConfigMap table. **Re-opening what is already open is not a change** — the same test
/// the content cursor already uses.
#[test]
fn a_filter_does_not_survive_a_view_change_and_does_survive_a_press_that_changes_nothing() {
    let filtered = || Filters {
        text: buffer("web"),
        namespace: buffer("pay"),
    };
    let mut app = App {
        filters: filtered(),
        ..App::default()
    };

    app.typing = Some(Typing::Text);
    app.open(NavItem::Kind(2));
    assert_eq!(app.filters, Filters::default(), "Alerts → a kind kept them");
    // **The session goes with the buffer it was typing into** — left open, focus would sit on a
    // field just emptied under it, on a list it was never about, with every printable key still
    // text and a footer saying otherwise (`k8s-admin`, 2026-09-18).
    assert_eq!(
        app.typing, None,
        "a typing session survived onto a different list"
    );

    app.filters = filtered();
    app.typing = Some(Typing::Namespace);
    app.open(NavItem::Kind(2));
    assert_eq!(
        app.filters,
        filtered(),
        "re-opening the kind already open cleared them"
    );
    assert_eq!(
        app.typing,
        Some(Typing::Namespace),
        "a press that changed no list closed the session anyway"
    );

    app.open(NavItem::Group(Group::Network));
    assert_eq!(
        app.filters,
        filtered(),
        "a disclosure triangle is not a different list"
    );

    app.open(NavItem::Report(1));
    assert_eq!(
        app.filters,
        Filters::default(),
        "a kind → a report kept them"
    );

    app.filters = filtered();
    app.open(NavItem::Alerts);
    assert_eq!(
        app.filters,
        Filters::default(),
        "a report → Alerts kept them"
    );
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
    let list = "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit";
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
        let (keys, quit) = app.footer(opened(detail), ORDINARY, Refused::default(), "", &[]);
        // **The which-pods step is a mode of this slot too** — one line, whatever view it was
        // opened over and whatever tab was last on (`screens/detail.md` § Picking a pod).
        let (stepping, stepped) =
            app.footer(Detailing::Pods, ORDINARY, Refused::default(), "", &[]);
        assert_eq!(
            (stepping.as_ref(), stepped),
            ("↑↓ move  ⏎ open  esc back  ? all keys  q quit", ""),
            "{view:?} · the which-pods step · {tab:?}"
        );
        assert_eq!(
            (keys.as_ref(), quit),
            (expected, ""),
            "{view:?} · detail {detail} · {tab:?}"
        );
    }
}

/// **The ordinary offer on a kind the console can act on** — a Deployment, a StatefulSet, the
/// running example everywhere in `screens/`. Every condition on [`App::may_mutate`] but the kind's
/// own kills every mutating key together, so a test about one of *those* says `ORDINARY` and names
/// no key.
///
/// **`scalable` is `false`, because `Offer::act` cannot produce anything else** (`SCALE_IS_BUILT`,
/// `screens/widgets.md` § 2a's *"`s scale` is never part of it"*). It was `true` while the footer
/// still drew `s`, and a fixture that keeps a value the constructor can no longer return tests a
/// footer no reader can reach — which is what made nine of these assertions fail against the
/// settled screen files rather than against the code.
const ORDINARY: Offer = Offer::Act {
    scalable: false,
    restartable: true,
};

/// **Both mutating keys, asked separately** ([`Op`]) — `(s, r)`, so a test can assert that a
/// condition takes both and not merely that it took one.
fn pressable(app: &App, offer: Offer) -> (bool, bool) {
    (
        app.may_mutate(offer, Op::Scale),
        app.may_mutate(offer, Op::Restart),
    )
}

/// **The pair that never gives way** (`screens/widgets.md` § 2a): `? all keys` and `q quit`,
/// drawn last and in that order, on every ordinary footer there is.
///
/// **Asserted as a suffix and not as *contains*** — the rule is about where they sit, and a
/// footer that named them first would pass a containment check while drawing the one thing the
/// section forbids.
///
/// **Ordinary means nothing on the wire, and that scope is now written down rather than left to
/// [`App::default`]** (`tester`, 2026-09-12): `changing` is `None` in the literal below, on
/// purpose. While a call is running these same footers lose exactly one word of this pair —
/// [`a_call_in_flight_leaves_every_other_footer_whole_but_for_the_quit`] is where that is
/// asserted, and what *this* test guarantees it is that there is a `  q quit` to strip at all. A
/// mode that stopped ending in the pair would drop nothing there and go green.
///
/// **Every [`Offer`] is walked, and [`every_offer`] is what makes that sentence true rather than a
/// claim about a hand-written list** (`tester`, 2026-09-18). It was one: seven literals, written
/// when there were seven shapes, and `Offer::Hidden` arrived as the eighth and was walked by
/// nothing — with this doc still saying *every*. **The pair holds on all of them and
/// `screens/states.md` says so in as many words** — *`? all keys` and `q quit` stay too; they
/// hold on this pane and every
/// other one on this page*, their one named exception being a mutation call in flight, which is
/// what `changing: None` excludes. The narrowest state of all, [`Offer::Nothing`], is the pair and
/// nothing else.
///
/// **Every refusal is walked with them, and the ceiling asserted is the real one.** This loop fed
/// `Refused::default()` only, so the one shape that is legitimately wider than the 66-column page
/// never reached it — `screens/widgets.md` § 2a says so in as many words: *"One fixed footer
/// already does not fit that budget: the both-refused row is 71 real columns"*, and the ceiling it
/// is measured against is `ui::indented`'s **76** at the 80×24 floor, which that section then calls
/// *inside the ceiling with 5 columns to spare* (`k8s-admin`, 2026-09-12). The exact four widths
/// are `the_list_footer_marks_the_keys_this_login_may_not_use`'s, read off that section's own
/// table; what this one adds is that no combination of shape and refusal passes it.
/// **Every [`Offer`] there is, as a list a new variant cannot be left out of.**
///
/// **The `match` is what makes it mechanical**: it names each variant and has no `_` arm, so a
/// ninth shape is a compile error in this function rather than a row every sweep below quietly
/// stops walking. That is exactly how `Offer::Hidden` arrived unwalked behind a doc comment
/// claiming otherwise (`tester`, 2026-09-18).
///
/// **[`Offer::Act`] is four rows and not one** since which of `s` and `r` a kind supports moved
/// onto it: a sweep fed only the both-supported shape would leave the three shorter footers walked
/// by nothing, which is the same hole `Offer::Hidden` fell through.
fn every_offer() -> Vec<Offer> {
    fn exhaustive(offer: Offer) {
        match offer {
            Offer::Nothing { .. }
            | Offer::Filter { .. }
            | Offer::Hidden { .. }
            | Offer::Move { .. }
            | Offer::Act { .. } => {}
        }
    }
    let mut all = Vec::new();
    for scalable in [false, true] {
        for restartable in [false, true] {
            all.push(Offer::Act {
                scalable,
                restartable,
            });
        }
    }
    for switch in [false, true] {
        all.push(Offer::Nothing { switch });
        all.push(Offer::Filter { switch });
        all.push(Offer::Move { switch });
        for namespace in [false, true] {
            for browsing in [false, true] {
                all.push(Offer::Hidden {
                    switch,
                    namespace,
                    browsing,
                });
            }
        }
    }
    for offer in &all {
        exhaustive(*offer);
    }
    all
}

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
            changing: None,
            ..App::default()
        };
        for offer in every_offer() {
            for (scale, restart) in [(false, false), (true, false), (false, true), (true, true)] {
                let refused = Refused::of(
                    "deployments",
                    [scale.then_some(&Verdict::No); 2],
                    [restart.then_some(&Verdict::No)],
                    [None],
                );
                // **Every state of the detail slot, the which-pods step included** — a sweep
                // that claims something about every footer has to walk every mode (NOTES § D270).
                for open in SLOTS.into_iter().chain([opened(detail)]) {
                    let (keys, quit) = app.footer(open, offer, refused, "", &[]);
                    assert!(
                        keys.ends_with("? all keys  q quit"),
                        "{view:?} · {tab:?} · {open:?} · {offer:?} — {keys:?} has no anchor pair"
                    );
                    assert_eq!(quit, "", "an ordinary footer grew a right-hand zone");
                    // **`ui::indented`'s own ceiling at the 80×24 floor**, measured the way
                    // ratatui measures — `↑↓`, `⏎` and `·` are not one byte each.
                    let columns = ratatui::text::Span::raw(keys.as_ref()).width();
                    assert!(
                        columns <= 76,
                        "{keys:?} is {columns} columns, past ui::indented's ceiling at the floor"
                    );
                }
            }
        }
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
    let (keys, quit) = app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[]);
    assert_eq!((keys.as_ref(), quit), ("? or esc to close", "q quit"));
    assert!(
        !app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0
            .contains("all keys"),
        "the footer still pointed at a screen the reader is already on"
    );
}

/// **The picker's footers and its failure's two, byte for byte** — `screens/widgets.md` § 2a's
/// mode list for the picker, `screens/context.md` for the failures — and none carries `X`, which
/// cannot fire under a modal (NOTES § D16 ruling 1), nor the anchor pair. **`⏎` is dropped from
/// the line wherever it would do nothing** — a shadowed row, no row selected — and kept on a
/// `(current)` nothing is connected to, where it is the retry (NOTES § D264 rulings 2 and 4);
/// **`↑↓ move` goes with it wherever no row can be landed on**, a filter that hides every row and
/// a kubeconfig with no contexts included (rulings 16 and 18). **While `/` holds text, `esc` reads
/// `clear filter` on both pickers and in every one of those shapes** (ruling 27) — and the same
/// shapes with nothing typed keep `cancel` and `quit`.
#[test]
fn the_picker_and_its_failure_each_say_the_keys_valid_inside_them() {
    let rows = listed(SIX);
    let dangling = listed(&SIX.replace("current-context: prod-eu", "current-context: deleted"));
    let undefined = listed(&SIX.replace("cluster: prod,", "cluster: nowhere,"))[..2].to_vec();
    assert!(undefined.iter().all(|row| row.server == Address::Undefined));
    let at = |connection: Connection, down: usize, filter: &str| {
        let mut picker = Picker::new(&rows, connection);
        for _ in 0..down {
            picker.down(&rows);
        }
        for character in filter.chars() {
            picker.filter.push(character);
        }
        Modal::ContextPick(picker)
    };
    for (modal, contexts, expected) in [
        (
            at(live(), 0, ""),
            &rows[..],
            "↑↓ move  type to filter  ⏎ switch  esc cancel",
        ),
        (
            at(Connection::Never, 0, ""),
            &rows,
            "↑↓ move  type to filter  ⏎ connect  esc quit",
        ),
        (
            at(dropped(), 0, ""),
            &rows,
            "↑↓ move  type to filter  ⏎ switch  esc cancel",
        ),
        (
            at(live(), 2, ""),
            &rows,
            "↑↓ move  type to filter  esc cancel",
        ),
        (
            at(Connection::Never, 2, ""),
            &rows,
            "↑↓ move  type to filter  esc quit",
        ),
        (
            Modal::ContextPick(Picker::new(&[], live())),
            &[],
            "type to filter  esc cancel",
        ),
        (
            Modal::ContextPick(Picker::new(&[], Connection::Never)),
            &[],
            "type to filter  esc quit",
        ),
        // **A filter that still shows the row the cursor is on**, both pickers: every key stays,
        // and only `esc`'s word moves.
        (
            at(live(), 0, "prod"),
            &rows,
            "↑↓ move  type to filter  ⏎ switch  esc clear filter",
        ),
        (
            at(Connection::Never, 0, "prod"),
            &rows,
            "↑↓ move  type to filter  ⏎ connect  esc clear filter",
        ),
        // **On the shadowed row it still shows**, where `⏎` is dropped.
        (
            at(live(), 2, "prod"),
            &rows,
            "↑↓ move  type to filter  esc clear filter",
        ),
        (
            at(Connection::Never, 2, "prod"),
            &rows,
            "↑↓ move  type to filter  esc clear filter",
        ),
        // **A filter that hides every row, and one that shows only undefined rows** — the page's
        // `[8]` and `[7]`'s filter-only trigger (ruling 30's last paragraph).
        (
            at(live(), 0, "zzz"),
            &rows,
            "type to filter  esc clear filter",
        ),
        (
            at(Connection::Never, 0, "zzz"),
            &rows,
            "type to filter  esc clear filter",
        ),
        (
            at(live(), 0, "old"),
            &rows,
            "type to filter  esc clear filter",
        ),
        (
            at(Connection::Never, 0, "old"),
            &rows,
            "type to filter  esc clear filter",
        ),
        // **A filter typed over a kubeconfig with no contexts** still holds text for `esc` to
        // clear.
        (
            Modal::ContextPick({
                let mut picker = Picker::new(&[], live());
                picker.filter.push('p');
                picker
            }),
            &[],
            "type to filter  esc clear filter",
        ),
        (
            Modal::ContextPick(Picker::new(&undefined, live())),
            &undefined,
            "type to filter  esc cancel",
        ),
        (
            Modal::ContextPick(Picker::new(&dangling, Connection::Never)),
            &dangling,
            "↑↓ move  type to filter  esc quit",
        ),
        (unconnected(Before::Connected(None)), &[], "esc dismiss"),
        (
            unconnected(Before::Picking(Picker::new(&rows, Connection::Never))),
            &[],
            "esc back to the list",
        ),
    ] {
        for view in [View::Alerts, View::Resources(0), View::Analysis(1)] {
            let app = App {
                view,
                modal: Some(modal.clone()),
                ..App::default()
            };
            // Every state of the detail slot, the which-pods step included (NOTES § D270).
            for open in SLOTS {
                assert_eq!(
                    app.footer(open, ORDINARY, Refused::default(), "", contexts),
                    (Cow::Borrowed(expected), ""),
                    "{modal:?} · {open:?}"
                );
            }
        }
    }
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
        for open in SLOTS.into_iter().chain([opened(detail)]) {
            assert_eq!(
                app.footer(open, ORDINARY, Refused::default(), "", &[]),
                (Cow::Borrowed("? or esc to close"), "q quit"),
                "{view:?} · {open:?} · {tab:?}"
            );
        }
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
    assert_eq!(
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0,
        "? or esc to close"
    );
    assert!(
        !app.escape(Detailing::Closed),
        "an esc with no startup picker open ended the run"
    );
    assert_eq!(
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0,
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
        // **A name field does not make `esc` live while the check is still out** — the waiting
        // arm is asked first, so the one dialog that could hold both (`drain`, v0.2) cannot
        // offer a key [`App::escape`] refuses ([`Dialog::waiting`]).
        (
            App {
                modal: Some(Modal::Confirm(dialog(Some("web")))),
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
        assert_eq!(
            app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
                .0,
            expected
        );
        assert_eq!(
            app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
                .1,
            "",
            "a dialog grew the right-hand zone only `?` has"
        );
        // **The same answer over an open detail tab**, which is the other side of the hole the
        // replaced test pinned: a dialog is opened from a detail pane as readily as from a list,
        // and a footer that fell through for one of them would fall through for both.
        assert_eq!(
            app.footer(
                Detailing::Tabs {
                    containers: 2,
                    from_step: false,
                },
                ORDINARY,
                Refused::default(),
                "",
                &[]
            )
            .0,
            expected
        );
    }
}

/// **One line replaces the ordinary footer of Alerts and Resources while a write is on the
/// wire — and of no other mode** (`screens/dialogs.md` § *While the call is running* and its
/// § *Detail tabs and Analysis keep their own footer, not this line*, `screens/widgets.md` § 2a's
/// closed mode list).
///
/// Read off that file's own mockup: `?` reads `? keys`, `q quit` is absent rather than marked, and
/// `↑↓ move` and `⏎ open` stay — *navigation stays free* is what this state promises, so the two
/// keys that mean it do not give way.
///
/// **Those two modes and not the rest, because those two are the only ordinary footers that name
/// `s` and `r`** — the keys a call in flight makes inactionable, which neither `no` (a permission
/// this login lacks) nor silence can honestly say.
///
/// **The healthy case is the same `App` with nothing running**, asserted first, so a footer that
/// drew the in-flight line unconditionally could not pass. **A refusal on top changes nothing**:
/// the line has no `s` or `r` left on it to mark.
#[test]
fn a_call_in_flight_replaces_the_two_footers_that_name_s_and_r() {
    for view in [View::Alerts, View::Resources(3), View::Resources(0)] {
        let mut app = App {
            view,
            ..App::default()
        };
        assert!(
            app.footer(
                Detailing::Closed,
                ORDINARY,
                Refused::default(),
                "payments/web",
                &[]
            )
            .0
            .ends_with("? all keys  q quit"),
            "{view:?} — nothing is running and the ordinary footer went"
        );

        app.changing = Some(dialog(None).object);
        let no = |refused: bool| refused.then_some(&Verdict::No);
        for (scale, restart) in [(false, false), (true, false), (false, true), (true, true)] {
            let refused = Refused::of("deployments", [no(scale); 2], [no(restart)], [None]);
            let (keys, quit) =
                app.footer(Detailing::Closed, ORDINARY, refused, "payments/web", &[]);
            assert_eq!(
                (keys.as_ref(), quit),
                (
                    "↑↓ move  ⏎ open  ? keys  ·  changing payments/web first",
                    ""
                ),
                "{view:?} · s refused {scale} · r refused {restart}"
            );
        }
    }
}

/// **Analysis and all four detail tabs keep their own footer and lose exactly one word: `q quit`**
/// (`screens/dialogs.md` § *Detail tabs and Analysis keep their own footer, not this line*,
/// `screens/detail.md`'s own sentence above the tab sections).
///
/// **The assertion is the relationship and not five more literals**, which is what makes *no mode
/// may lose more than that one word* provable rather than promised: the in-flight line plus
/// `  q quit` back is the ordinary line, character for character. A mode that dropped `esc back`
/// as well, or that drew the in-flight replacement, fails here whatever its literal says.
///
/// **This is the defect `k8s-admin` found in the box's first draft** (2026-09-12): the logs tab
/// over a call confirmed on Alerts drew `⏎ open` on a pane with nothing to select, dropped the
/// `esc back` that is the only way out of the tab, and left `[ ] tabs`, `f follow` and
/// `c container` bound and unnamed — three of five keys failing `screens/widgets.md` § 2a's own
/// rule that a footer is *the keys valid right now*.
#[test]
fn a_call_in_flight_leaves_every_other_footer_whole_but_for_the_quit() {
    for (view, detail, tab) in [
        (View::Analysis(2), false, Tab::Yaml),
        (View::Analysis(0), false, Tab::Logs),
        (View::Alerts, true, Tab::Logs),
        (View::Alerts, true, Tab::Describe),
        (View::Alerts, true, Tab::Yaml),
        (View::Resources(1), true, Tab::Events),
    ] {
        let app = App {
            view,
            tab,
            ..App::default()
        };
        // **The which-pods step keeps its own footer too, losing exactly `q quit`** — the same as
        // the detail tabs and Analysis. `screens/dialogs.md` names those two and not this step,
        // and the test that page states is whether a call on the wire makes anything on the line
        // false: the step's line names no mutating key, so nothing on it is (NOTES § D270).
        for open in [opened(detail), Detailing::Pods] {
            let mut app = app.clone();
            let ordinary = app
                .footer(open, ORDINARY, Refused::default(), "payments/web", &[])
                .0;

            app.changing = Some(dialog(None).object);
            let (keys, quit) = app.footer(open, ORDINARY, Refused::default(), "payments/web", &[]);
            assert_eq!(
                format!("{keys}  q quit"),
                ordinary,
                "{view:?} · {open:?} · {tab:?} — more than the one word gave way"
            );
            assert_eq!(
                quit, "",
                "a mode that keeps its own footer grew a second zone"
            );
            assert!(
                !keys.contains("quit") && !keys.contains("changing"),
                "{view:?} · {open:?} · {tab:?} — {keys:?}"
            );
        }
    }
}

/// **The arm is [`App::changing`]'s and never the argument's** (PM ruling, 2026-09-12).
///
/// A caller that hands over a name with nothing running may not turn this footer on, and one that
/// hands over an empty name with a call running may not turn it off — which is what lets
/// `ui::footer` ask for this same line with `""` purely to measure its own fixed parts. That
/// second string is asserted whole here, because it **is** the prefix and suffix `room` is
/// measured off: a word changed in either of them moves a number `ui.rs` never restates.
#[test]
fn the_in_flight_arm_is_changings_and_never_the_names() {
    let mut app = App::default();
    assert_eq!(
        app.footer(
            Detailing::Closed,
            ORDINARY,
            Refused::default(),
            "payments/web",
            &[]
        )
        .0,
        "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        "a name alone turned the in-flight footer on"
    );

    app.changing = Some(dialog(None).object);
    assert_eq!(
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0,
        "↑↓ move  ⏎ open  ? keys  ·  changing  first",
        "an empty name turned the in-flight footer off"
    );
}

/// **Help over a call in flight drops `q quit`, and does not mark it** (`screens/help.md`
/// § *While the call is running*, `screens/widgets.md` § 2a).
///
/// The `no` this product spells beside a key is a permission this login lacks; a call finishing is
/// a wait, and the two stay two different facts. The right zone empties instead — and Help's
/// ordinary footer keeps `q quit` exactly as it does today, which is the healthy case here.
#[test]
fn help_over_a_call_in_flight_drops_the_quit_it_cannot_promise() {
    let mut app = App {
        modal: Some(Modal::Help),
        ..App::default()
    };
    assert_eq!(
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[]),
        (Cow::Borrowed("? or esc to close"), "q quit"),
        "help's ordinary footer changed"
    );

    app.changing = Some(dialog(None).object);
    let (keys, quit) = app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[]);
    assert_eq!(
        keys.as_ref(),
        "? or esc to close",
        "the body of help's own line moved"
    );
    assert_eq!(quit, "", "help promised a quit the running call refuses");
    assert!(
        !format!("{keys} {quit}").contains("no quit"),
        "a wait was marked as a permission this login lacks: {keys:?} {quit:?}"
    );
}

/// **What every other modal does while a call is running, stated rather than left implicit.**
///
/// The combination is unreachable by the wiring Phase 12 will write — `Modal::Confirm` closes
/// when the yes is given, which is *before* [`App::changing`] is set, and `Modal::Refused` and
/// `Modal::Gone` are built from what the call answered, which is *after* it is cleared. What is
/// asserted is that the modal arms still answer first if it ever happens: a closed local set is
/// the one honest thing to draw under an open box (`screens/widgets.md` § 2a), and a footer
/// offering `⏎ open` on a screen whose box says *Already gone* would be the defect the next box
/// inherits.
#[test]
fn a_modal_keeps_its_own_closed_set_even_with_a_call_running_under_it() {
    let running = dialog(None).object;
    for (modal, expected) in [
        (Modal::Confirm(dialog(None)), "waiting for the cluster"),
        (
            Modal::Refused {
                sent: true,
                fault: crate::k8s::Fault::Rejected,
                said: None,
            },
            "esc dismiss  ⏎ open",
        ),
        (
            Modal::Gone {
                object: running.clone(),
                recreated: false,
            },
            "esc dismiss",
        ),
    ] {
        let app = App {
            modal: Some(modal.clone()),
            changing: Some(running.clone()),
            ..App::default()
        };
        let (keys, quit) = app.footer(
            Detailing::Closed,
            ORDINARY,
            Refused::default(),
            "payments/web",
            &[],
        );
        assert_eq!((keys.as_ref(), quit), (expected, ""), "{modal:?}");
        assert!(
            !keys.contains("changing"),
            "the in-flight line reached a modal's closed set: {keys:?}"
        );
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
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::ContextPick(Picker::new(&listed(SIX), live()))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::ContextPick(Picker::new(
                &listed(SIX),
                Connection::Never,
            ))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(unconnected(Before::Connected(None))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(unconnected(Before::Picking(Picker::new(
                &listed(SIX),
                Connection::Never,
            )))),
        ),
    ] {
        let app = App {
            view,
            tab,
            modal,
            ..App::default()
        };
        for open in SLOTS.into_iter().chain([opened(detail)]) {
            let (keys, quit) = app.footer(open, ORDINARY, Refused::default(), "", &[]);
            let width = ratatui::text::Span::raw(keys.as_ref()).width()
                + usize::from(!quit.is_empty())
                + ratatui::text::Span::raw(quit).width();
            assert!(
                width <= 66,
                "{keys:?} + {quit:?} is {width} columns, past the 66 the mockups draw"
            );
        }
        seen += 1;
    }
    assert_eq!(seen, 16, "a mode stopped being measured");
}

/// **`screens/widgets.md` § 2a's own two tables, read as the fixture** — the footer string and the
/// column count that section counted it at, in the file's order. The first four are a kind that
/// supports both operations: neither refused · `s` · `r` · both. The five after them are the kinds
/// that support one or neither, where the key that is missing is off the line rather than marked.
///
/// **The screen file is the fixture, which is the point** (`ui_tests::mockup`'s own reason). A
/// test that compared these nine lines with the nine literals `App::footer` returns would compare
/// the implementation with itself.
///
/// **Both tables are read and not just the first.** They carry the same header row, so the parser
/// that stopped at the first one went on passing when the second arrived — four rows asserted,
/// five drawn by nothing.
fn mockup_footers() -> Vec<(String, usize)> {
    let path = format!("{}/screens/widgets.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let lines: Vec<&str> = text.lines().collect();
    let rows: Vec<(String, usize)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.trim_start()
                .starts_with("| State | Alerts / Resources footer")
        })
        .flat_map(|(at, _)| {
            lines[at + 2..]
                .iter()
                .take_while(|line| line.trim_start().starts_with('|'))
        })
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
        3,
        "screens/widgets.md § 2a no longer tabulates the three states of the list footer"
    );
    rows
}

/// **The list footer marks exactly the keys this login may not use and draws only the keys the
/// selected kind can use, in `screens/widgets.md` § 2a's own three strings and at its own three
/// column counts** (NOTES § D23, § D229, § D261 ruling 8).
///
/// **Three and not nine, because the `s` axis is gone** (that section, 2026-09-24: *"`s scale` is
/// never part of it: `s` is withheld from `Offer::Act` for every kind, every login and every
/// run"*). The four both-support rows and the two scale-only rows went with it; what is left is `r`
/// supported, `r` supported and refused, and `r` not supported. `Offer::act` never producing
/// `scalable: true` is its own assertion, one test down — this one is about what the footer draws
/// for the offers that exist.
///
/// **The width is measured the way ratatui measures**, because `↑↓`, `⏎` and `·` are not one byte
/// each — and it is checked against the section's counted number rather than an inequality, so a
/// row that fits but says the wrong thing still fails.
///
/// **A refusal on a key the kind has not got must change nothing**, which is the half that makes
/// *refused* and *unsupported* two shapes rather than one: `may_i_in` was never asked about that
/// key, so a `Verdict::No` sitting in [`Refused`] for it has no line to reach. Every row is
/// therefore walked with *both* answers for the refusal it does not pin, and the same string has
/// to come back.
#[test]
fn the_list_footer_marks_the_keys_this_login_may_not_use() {
    let table = mockup_footers();
    // The file's own order: which operations the kind supports, then the refusal that row is
    // about. `None` is a refusal the row is *not* about — the key is off the line, so both answers
    // must draw the same string.
    let states = [
        (false, true, None, Some(false)),
        (false, true, None, Some(true)),
        (false, false, None, None),
    ];
    for (nth, (scalable, restartable, pinned_scale, pinned_restart)) in
        states.into_iter().enumerate()
    {
        let (expected, columns) = &table[nth];
        let walked = |pinned: Option<bool>| pinned.map_or(vec![false, true], |only| vec![only]);
        for scale in walked(pinned_scale) {
            for restart in walked(pinned_restart) {
                let refused = Refused::of(
                    "deployments",
                    [scale.then_some(&Verdict::No); 2],
                    [restart.then_some(&Verdict::No)],
                    [None],
                );
                for view in [View::Alerts, View::Resources(0)] {
                    let app = App {
                        view,
                        ..App::default()
                    };
                    let offer = Offer::Act {
                        scalable,
                        restartable,
                    };
                    let (keys, quit) = app.footer(Detailing::Closed, offer, refused, "", &[]);
                    assert_eq!(
                        (keys.as_ref(), quit),
                        (expected.as_str(), ""),
                        "{view:?} · {offer:?} · s refused {scale} · r refused {restart}"
                    );
                    assert_eq!(
                        ratatui::text::Span::raw(keys.as_ref()).width(),
                        *columns,
                        "{keys:?} is not the width screens/widgets.md § 2a counted"
                    );
                    // **Drawn and live are one fact** (invariant 2): the key that is on the line
                    // is exactly the key that can be pressed, per key.
                    assert_eq!(
                        pressable(&app, offer),
                        (scalable, restartable),
                        "{offer:?} — a key was drawn and dead, or dead and drawn"
                    );
                }
            }
        }
        // The ceiling `ui::indented` leaves at the 80×24 floor (`screens/widgets.md` § 2a). The
        // 66-column page the mockups are drawn at is not this row's budget: § 2a's own table puts
        // both-refused at 71 and calls it *inside the ceiling with 5 columns to spare*.
        assert!(*columns <= 76, "{expected:?} is {columns} columns");
    }
}

/// **`may_mutate` is asked per key, because the two keys no longer have one answer**
/// (`screens/widgets.md` § 2a, extended 2026-09-18): a DaemonSet's `r` is live in the same frame
/// its `s` is not, and a press of `s` there must reach nothing.
///
/// **And every shape that is not [`Offer::Act`] refuses both**, whichever key is named — the
/// catch-all in `Offer::offers` is the safe direction, and this is what says so for all fourteen
/// of them rather than for the one a reader thought of.
#[test]
fn a_key_the_selected_kind_cannot_use_is_not_pressable() {
    for (scalable, restartable) in [(false, false), (true, false), (false, true), (true, true)] {
        let offer = Offer::Act {
            scalable,
            restartable,
        };
        assert_eq!(
            pressable(&App::default(), offer),
            (scalable, restartable),
            "{offer:?} — the key the footer withheld was live behind it"
        );
    }
    let mut withheld = 0;
    for offer in every_offer()
        .into_iter()
        .filter(|offer| !matches!(offer, Offer::Act { .. }))
    {
        assert_eq!(
            pressable(&App::default(), offer),
            (false, false),
            "{offer:?} draws neither key and left one pressable"
        );
        withheld += 1;
    }
    assert_eq!(
        withheld, 14,
        "a shape that offers no mutating key stopped being walked"
    );
}

/// **A kind word is not an address, and [`Offer::act`] takes both halves** (NOTES § D51,
/// PRIOR-ART § F4, `reports/2026-09-18-offer-per-kind-operator-read.md` § 1). `ops::scalable` and
/// `ops::restartable` answer with `apps/v1` and nothing else, so the same word under another group
/// is another object and gets no key.
///
/// **It needs no CRD to be reachable** — a stock cluster serves `v1 Event` beside
/// `events.k8s.io/v1 Event`, two resources sharing one kind word — and the sidebar row a reader
/// opens is the plural alone, so nothing on screen tells them apart. OpenKruise's
/// `apps.kruise.io/v1beta1 StatefulSet` is the measured example.
///
/// **Asserted at this layer as well as through a drawn frame**, because this is where the rule
/// lives: `ui::offered` can only hand over what it reads, and the comparison it depends on is
/// here.
#[test]
fn a_kind_word_under_another_group_is_another_object_and_gets_no_key() {
    // **The `s` column is `false` on every row, including the three kinds `ops::scalable` serves**
    // ([`SCALE_IS_BUILT`], `screens/widgets.md` § 2a): the group question this test is about is
    // asked of both operations and answered for `r` alone until the count step exists. A bare
    // ReplicaSet — which scales and does not restart — therefore has no mutating key at all here,
    // which is the row that would silently start passing again if the withholding were reverted
    // without this table.
    for (group, kind, expected) in [
        ("apps", "deployment", (false, true)),
        ("apps", "statefulset", (false, true)),
        ("apps", "daemonset", (false, true)),
        ("apps", "replicaset", (false, false)),
        // The same four words, owned by somebody else.
        ("apps.kruise.io", "statefulset", (false, false)),
        ("apps.kruise.io", "daemonset", (false, false)),
        ("example.com", "deployment", (false, false)),
        ("", "deployment", (false, false)),
        // And the core group, which serves none of them.
        ("", "pod", (false, false)),
        ("", "node", (false, false)),
        ("", "", (false, false)),
    ] {
        assert_eq!(
            pressable(&App::default(), Offer::act(group, kind)),
            expected,
            "{group}/{kind}"
        );
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
            for open in SLOTS.into_iter().chain([opened(detail)]) {
                assert_eq!(
                    app.footer(open, ORDINARY, refused, "", &[]),
                    app.footer(open, ORDINARY, Refused::default(), "", &[]),
                    "{view:?} · {open:?}"
                );
            }
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
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::ContextPick(Picker::new(&listed(SIX), live()))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(Modal::ContextPick(Picker::new(
                &listed(SIX),
                Connection::Never,
            ))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(unconnected(Before::Connected(None))),
        ),
        (
            View::Alerts,
            false,
            Tab::Logs,
            Some(unconnected(Before::Picking(Picker::new(
                &listed(SIX),
                Connection::Never,
            )))),
        ),
    ] {
        let app = App {
            view,
            tab,
            modal,
            ..App::default()
        };
        // **This table's own rows and the which-pods step, never every slot** — the whole claim
        // is about the footers that name neither `s` nor `r`, and a list footer is not one of
        // them; sweeping `Detailing::Closed` over Alerts in here would be asserting the opposite
        // of what `screens/widgets.md` § 2a says a marked list draws.
        for open in [opened(detail), Detailing::Pods] {
            assert_eq!(
                app.footer(open, ORDINARY, all, "", &[]),
                app.footer(open, ORDINARY, Refused::default(), "", &[]),
                "{:?} · {open:?} · {tab:?}",
                app.view
            );
        }
        seen += 1;
    }
    assert_eq!(seen, 14, "a footer stopped being measured");
}

/// `screens/widgets.md` § 2b — **the four footers that section draws, byte for byte.** The whole
/// line is replaced rather than curated, the label is the field's own word, and `esc`'s second
/// word drops to `cancel` exactly when the focused buffer is empty.
///
/// **The typed text arrives as an argument, the way the in-flight line's object name does** —
/// `ui.rs` measures the fixed parts by asking for this same line with an empty one, and the arm is
/// chosen by [`App::typing`] and never by the argument.
#[test]
fn a_filter_being_typed_replaces_the_whole_footer() {
    let ask = |app: &App, cut: &str| {
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), cut, &[])
            .0
            .into_owned()
    };

    let mut app = App {
        typing: Some(Typing::Text),
        ..App::default()
    };
    assert_eq!(ask(&app, ""), "filter:   ⏎ done  esc cancel");

    app.filters.text = buffer("payments");
    assert_eq!(
        ask(&app, "payments"),
        "filter: payments  ⏎ done  esc clear filter"
    );

    app.typing = Some(Typing::Namespace);
    assert_eq!(
        ask(&app, ""),
        "namespace like:   ⏎ done  esc cancel",
        "`esc`'s word read the field that did not have focus"
    );
    app.filters.namespace = buffer("pay");
    assert_eq!(
        ask(&app, "pay"),
        "namespace like: pay  ⏎ done  esc clear namespace"
    );

    // **Nothing else can be open at once, and the answer is still the typing line** — `?`, `X`
    // and `⏎` are characters while a filter has focus, so the arm answers first on purpose.
    app.typing = Some(Typing::Text);
    app.modal = Some(Modal::Help);
    assert_eq!(
        ask(&app, "payments"),
        "filter: payments  ⏎ done  esc clear filter"
    );
}

/// `screens/states.md` § The filter hides every row — **the one list footer that can afford
/// `esc`'s own word, and it affords it because it has just dropped the four keys with nothing to
/// act on.** The word follows [`Filters::clears`], so it names the field that key really empties.
#[test]
fn the_footer_over_a_filter_that_hides_every_row_names_the_field_esc_clears() {
    let app = App::default();
    let ask = |offer| {
        app.footer(Detailing::Closed, offer, Refused::default(), "", &[])
            .0
            .into_owned()
    };
    let hidden = |switch, namespace, browsing| {
        ask(Offer::Hidden {
            switch,
            namespace,
            browsing,
        })
    };

    // **Alerts keeps the cursor keys and the browser drops them** — each screen's own zero-row
    // precedent, not one rule applied twice (`screens/states.md` § The filter hides every row).
    assert_eq!(
        hidden(false, false, false),
        "↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit"
    );
    assert_eq!(
        hidden(false, true, false),
        "↑↓ move  ⏎ open  / filter  esc clear namespace  ? all keys  q quit"
    );
    assert_eq!(
        hidden(false, false, true),
        "/ filter  esc clear filter  ? all keys  q quit"
    );
    assert_eq!(
        hidden(false, true, true),
        "/ filter  esc clear namespace  ? all keys  q quit"
    );
    assert_eq!(
        hidden(true, false, true),
        "X switch cluster  / filter  esc clear filter  ? all keys  q quit"
    );
    assert_eq!(
        hidden(true, true, true),
        "X switch cluster  / filter  esc clear namespace  ? all keys  q quit"
    );

    // **`X switch cluster` is not on Alerts' pair, and that is arithmetic**: with it the line is
    // 84 columns against `ui::indented`'s 76 at the floor. An expired login draws the same two
    // lines as a live one there, and `X` stays where `? all keys` points.
    for namespace in [false, true] {
        assert_eq!(
            hidden(true, namespace, false),
            hidden(false, namespace, false),
            "Alerts' zero-match line moved for a key it has no room for"
        );
    }

    // **The two mutating keys are on none of the six** — nothing is selected, and the cursor keys
    // Alerts keeps are exactly the ones the page rules it keeps.
    for switch in [false, true] {
        for namespace in [false, true] {
            for browsing in [false, true] {
                let line = hidden(switch, namespace, browsing);
                for gone in ["s scale", "r restart", "s no"] {
                    assert!(!line.contains(gone), "{line:?} still promises {gone:?}");
                }
                assert_eq!(
                    line.contains("↑↓ move") && line.contains("⏎ open"),
                    !browsing,
                    "the two panes did not differ on the cursor keys: {line:?}"
                );
            }
        }
    }
}

/// `screens/detail.md` § Choosing a container — **`c container` is on the logs footer only where
/// there is more than one answer**, and a pod whose snapshot has not arrived draws the same line a
/// single-container pod does rather than a key that would do nothing.
#[test]
fn c_container_is_offered_only_where_there_is_something_to_choose() {
    let app = App::default();
    let ask = |containers| {
        app.footer(containers, ORDINARY, Refused::default(), "", &[])
            .0
            .into_owned()
    };

    assert_eq!(
        ask(Detailing::Tabs {
            containers: 2,
            from_step: false,
        }),
        "[ ] tabs  f follow  c container  esc back  ? all keys  q quit"
    );
    for many in [0, 1] {
        assert_eq!(
            ask(Detailing::Tabs {
                containers: many,
                from_step: false,
            }),
            "[ ] tabs  f follow  esc back  ? all keys  q quit",
            "{many} containers still offered a picker"
        );
    }
}

/// `screens/detail.md` § Choosing a container, § The pod disappears while the picker is open —
/// **the picker's closed set, and what is drawn when the pod it was about has gone.** There is no
/// second *already gone* box for it: the picker is not drawn, and the footer under it is the logs
/// tab's own, because that is what the reader is looking at.
#[test]
fn the_container_picker_offers_three_keys_and_none_once_the_pod_has_gone() {
    let app = App {
        modal: Some(Modal::ContainerPick(Cursor::default())),
        ..App::default()
    };
    let ask = |containers| {
        app.footer(containers, ORDINARY, Refused::default(), "", &[])
            .0
            .into_owned()
    };

    assert_eq!(
        ask(Detailing::Tabs {
            containers: 3,
            from_step: false,
        }),
        "↑↓ move  ⏎ pick  esc cancel"
    );
    for many in [0, 1] {
        assert_eq!(
            ask(Detailing::Tabs {
                containers: many,
                from_step: false,
            }),
            "[ ] tabs  f follow  esc back  ? all keys  q quit",
            "a picker with {many} containers kept a footer of its own"
        );
    }
}

/// `screens/widgets.md` § 5 — **`esc` closes the container picker like any other modal**, one
/// level per press, and it leaves the filters alone on the way out.
///
/// **A picker with nothing left to pick is not a level, and one press does both** — the box is not
/// drawn once the containers have gone (`screens/detail.md` § The pod disappears while the picker
/// is open), so spending a press closing it is a key that visibly does nothing (`k8s-admin`,
/// 2026-09-18). It is dropped and the press goes on to what the drawn footer promises.
#[test]
fn esc_closes_the_container_picker_and_touches_nothing_else() {
    let picking = || App {
        modal: Some(Modal::ContainerPick(Cursor::default())),
        filters: Filters {
            text: buffer("web"),
            namespace: Input::default(),
        },
        ..App::default()
    };

    let mut app = picking();
    assert!(!app.escape(Detailing::Tabs {
        containers: 3,
        from_step: false,
    }));
    assert_eq!(app.modal, None);
    assert_eq!(
        app.filters.text.text(),
        "web",
        "closing a modal reached past it into a filter"
    );

    // **Nothing left to pick: the invisible modal goes, and the press does the visible thing in
    // the same press.** What that is depends on what is drawn under it — a logs tab is still open
    // for `Tabs(0)`/`Tabs(1)`, so this press is the `esc back` its footer promises and the filter
    // is untouched; with no tab at all it is the ordinary at-rest `esc`.
    for containers in [
        Detailing::Tabs {
            containers: 0,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 1,
            from_step: false,
        },
    ] {
        let mut app = picking();
        assert!(!app.escape(containers));
        assert_eq!(app.modal, None, "{containers:?}");
        assert_eq!(
            app.filters.text.text(),
            "web",
            "{containers:?}: `esc back` out of the tab cleared its filter"
        );
    }

    let mut app = picking();
    assert!(!app.escape(Detailing::Closed));
    assert_eq!(app.modal, None);
    assert!(
        app.filters.text.is_empty(),
        "the press was spent on a box nobody can see"
    );
}

/// **What a press meant is the caller's to read, and only [`picking`] answers it** (`k8s-admin`,
/// 2026-09-18).
///
/// `App::escape` leaves [`App::modal`] `None` whether it cancelled a **drawn** picker — whose own
/// footer said `esc cancel`, so the tab behind it stays — or dropped an **undrawn** one, where the
/// footer the reader was looking at was the logs tab's `esc back` and the tab closes. `modal` is
/// the same value after both and `modal.is_some()` is the same before both, so a caller reading
/// either one closes the detail tab out from under a picker somebody had just cancelled.
#[test]
fn only_picking_tells_a_cancelled_picker_from_a_dropped_one() {
    // The predicate is what the two rows differ on, and it is what the caller can reach.
    assert!(
        picking(Detailing::Tabs {
            containers: 2,
            from_step: false,
        }) && picking(Detailing::Tabs {
            containers: 9,
            from_step: false,
        })
    );
    for none in [
        Detailing::Closed,
        Detailing::Tabs {
            containers: 0,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 1,
            from_step: false,
        },
    ] {
        assert!(
            !picking(none),
            "{none:?} has nothing to pick and said it had"
        );
    }

    let open = || App {
        modal: Some(Modal::ContainerPick(Cursor::default())),
        filters: Filters {
            text: buffer("web"),
            namespace: Input::default(),
        },
        ..App::default()
    };

    for containers in [
        Detailing::Tabs {
            containers: 3,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 1,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 0,
            from_step: false,
        },
        Detailing::Closed,
    ] {
        let mut app = open();
        assert!(app.modal.is_some(), "both rows start with a modal open");
        assert!(!app.escape(containers));
        assert_eq!(
            app.modal, None,
            "{containers:?}: `modal` is the same after both rows, which is the whole finding"
        );
        // The press's *effect* differs, and it follows `picking`, not `modal`: a drawn picker
        // spends the press on itself, an undrawn one lets it through to what was drawn under it.
        assert_eq!(
            !app.filters.text.is_empty(),
            picking(containers) || containers != Detailing::Closed,
            "{containers:?}: what the press reached does not follow `picking`"
        );
    }
}

/// `screens/widgets.md` § 2b — **a filter survives `esc` back from Detail**, which is the whole of
/// why that page draws Detail as an overlay: going back is not going somewhere else.
///
/// `App` cannot see that a tab is open, so it is handed the same fact [`App::footer`] is
/// (`k8s-admin`, 2026-09-18 — this arm cleared the filter the page promises survives it).
#[test]
fn esc_out_of_a_detail_tab_leaves_the_filter_where_it_was() {
    let filtered = || App {
        filters: Filters {
            text: buffer("web"),
            namespace: buffer("pay"),
        },
        ..App::default()
    };

    for containers in [
        Detailing::Tabs {
            containers: 0,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 1,
            from_step: false,
        },
        Detailing::Tabs {
            containers: 4,
            from_step: false,
        },
    ] {
        let mut app = filtered();
        assert!(!app.escape(containers));
        assert_eq!(
            app.filters,
            filtered().filters,
            "{containers:?}: `esc back` cleared a filter Detail was drawn over"
        );
    }

    // And with no tab open it is the ordinary at-rest `esc`, narrow to wide.
    let mut app = filtered();
    assert!(!app.escape(Detailing::Closed));
    assert!(app.filters.text.is_empty() && !app.filters.namespace.is_empty());
    assert!(!app.escape(Detailing::Closed));
    assert!(app.filters.namespace.is_empty());
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
///
/// **And `3 restarts` beside it is the same count without the comma**, which the container picker
/// draws in a column of its own (`screens/detail.md` § Choosing a container). Both are asserted
/// off one status, because a second count is exactly what this pair exists to stop.
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

    assert_eq!(restart_count(None), "");
    assert_eq!(restart_count(Some(&counted(0))), "");
    assert_eq!(restart_count(Some(&counted(1))), "1 restart");
    assert_eq!(restart_count(Some(&counted(12))), "12 restarts");
    assert_eq!(restart_count(Some(&counted(-1))), "");
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
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0
            .contains(armed.confirm()),
        "the footer does not name the button's own word: {:?}",
        app.footer(Detailing::Closed, ORDINARY, Refused::default(), "", &[])
            .0
    );
}

// --- WHY A CALL DID NOT WORK ---

/// **Every fault's sentence, written out** — the driver's own words, moved down whole
/// (NOTES § D264 ruling 1), so this is the grid `main_tests.rs` pins them with plus the arms it
/// reads only through a caller. A literal per arm because a reworded frame is invisible to a
/// predicate (`main_tests.rs`'s grid says why), and in the two framings the picker's failure box
/// hands over: [`watching`] and [`REACH`].
#[test]
fn every_fault_reads_the_sentence_the_driver_prints() {
    let pods = watching("pods");
    assert_eq!(pods, "`list` and `watch` pods");
    let grid = [
        (
            Fault::Refused,
            "the role this kubeconfig uses needs to `list` and `watch` pods",
            "the role this kubeconfig uses needs to reach this cluster",
        ),
        (
            Fault::Gone,
            "this server says there is no such thing when k8rs tries to `list` and `watch` pods",
            "this server says there is no such thing when k8rs tries to reach this cluster",
        ),
        (
            Fault::Rejected,
            "this cluster would not accept the request k8rs made to `list` and `watch` pods — \
             that is a fault in k8rs, and nothing is wrong with the cluster or with this login",
            "this cluster would not accept the request k8rs made to reach this cluster — that is \
             a fault in k8rs, and nothing is wrong with the cluster or with this login",
        ),
        (
            Fault::Unanswered,
            "nothing usable came back when k8rs tried to `list` and `watch` pods",
            "nothing usable came back when k8rs tried to reach this cluster",
        ),
        (
            Fault::Unfinished,
            "the request k8rs made to `list` and `watch` pods had not been answered",
            "the request k8rs made to reach this cluster had not been answered",
        ),
        (
            Fault::Kubeconfig,
            "the kubeconfig itself could not be read — it is missing, unreadable, or not valid \
             YAML",
            "the kubeconfig itself could not be read — it is missing, unreadable, or not valid \
             YAML",
        ),
        (
            Fault::NoContext,
            "this kubeconfig has no such context — check the `current-context` line in the \
             file, and any `--context` on the command line",
            "this kubeconfig has no such context — check the `current-context` line in the \
             file, and any `--context` on the command line",
        ),
        (
            Fault::BadEntry,
            "this kubeconfig loaded, and something it points at did not — a certificate file it \
             names, a `server:` line, or a cluster one of its contexts refers to",
            "this kubeconfig loaded, and something it points at did not — a certificate file it \
             names, a `server:` line, or a cluster one of its contexts refers to",
        ),
        (
            Fault::NoCredential,
            "the program this kubeconfig logs in with gave k8rs nothing to sign in with",
            "the program this kubeconfig logs in with gave k8rs nothing to sign in with",
        ),
        (
            Fault::Expired,
            "this cluster no longer accepts this login — this kubeconfig needs a new one",
            "this cluster no longer accepts this login — this kubeconfig needs a new one",
        ),
        (
            Fault::Conflict,
            "something else changed this object while k8rs was working on it — nothing was \
             changed, and reading it again shows what it looks like now",
            "something else changed this object while k8rs was working on it — nothing was \
             changed, and reading it again shows what it looks like now",
        ),
    ];
    for (fault, watched, reached) in grid {
        assert_eq!(because(fault, &pods, None, None), watched, "{fault:?}");
        assert_eq!(because(fault, REACH, None, None), reached, "{fault:?}");
    }

    // **The login program is named where the kubeconfig has one** — and only by the two arms whose
    // fix is on the reader's own machine.
    assert_eq!(
        because(Fault::Expired, &pods, Some("aws"), None),
        "this cluster no longer accepts this login — it comes from `aws`, so renew it there"
    );
    assert_eq!(
        because(Fault::NoCredential, REACH, Some("aws"), None),
        "the program this kubeconfig logs in with (`aws`) gave k8rs nothing to sign in with"
    );
    // **The cluster's words are quoted by the rejected call and by nothing else.**
    let wrote = "container \"app\" in pod \"broken-config\" is waiting to start: \
                 CreateContainerConfigError";
    assert_eq!(
        because(Fault::Rejected, &pods, None, Some(wrote)),
        "this cluster would not accept the request k8rs made to `list` and `watch` pods, and \
         said: container \"app\" in pod \"broken-config\" is waiting to start: \
         CreateContainerConfigError"
    );
    for (fault, _, _) in grid {
        if fault != Fault::Rejected {
            assert_eq!(
                because(fault, &pods, Some("aws"), Some(wrote)),
                because(fault, &pods, Some("aws"), None),
                "{fault:?} quoted the cluster"
            );
        }
    }
}

/// **The scope is the place, and the next step is per [`Coverage`]** — a namespace the reader named
/// and one k8rs had to guess read the same place and opposite next steps
/// (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1, NOTES § D264 ruling 1).
///
/// **Not running — `--once` — first, and its bytes are the ones this grid held before k8rs could
/// be running** (NOTES § D264 ruling 23). Running, the two arms that end in the flag say to quit
/// and start again, in the words `screens/context.md` § What `Refused` draws, if something ever
/// hands it here writes — all three of them, which `ui_tests.rs` holds the drawn box to byte for
/// byte; nothing else moves. **The middle arm has no `running` half at all**: a namespace the
/// reader named by typing `--namespace` is a door spent whichever surface refused it, so `--once`
/// prints the identical words. **Three faults answer either way** (NOTES § D264 ruling 32), `Gone`
/// in that ruling's bytes.
///
/// **That subsection is where these sentences live now** (NOTES § D280): the section that used to
/// quote them went with the `Fault::Refused` mockups, which cannot reach a failed connect, and for
/// a round the three were written in no screen at all.
#[test]
fn the_next_step_is_per_coverage_and_only_three_faults_have_one() {
    let payments = || "payments".to_owned();
    assert_eq!(scope(&Coverage::Cluster), "across the whole cluster");
    for coverage in [
        Coverage::Asked(payments()),
        Coverage::Refused(payments()),
        Coverage::Blind(payments()),
    ] {
        assert_eq!(
            scope(&coverage),
            "in the namespace payments",
            "{coverage:?}"
        );
    }

    let named = "Ask whoever runs this cluster for a role that may read pods in payments — the \
                 same rules as `k8rs-readonly` in the k8rs docs, granted in one namespace instead \
                 of all of them";
    for (running, coverage, expected) in [
        (
            false,
            Coverage::Cluster,
            "Ask whoever runs this cluster for a role that may read pods in every namespace — \
             `k8rs-readonly` in the k8rs docs is that role — or run k8rs in one namespace you can \
             read: --namespace <name>",
        ),
        (false, Coverage::Asked(payments()), named),
        (false, Coverage::Refused(payments()), named),
        (
            false,
            Coverage::Blind(payments()),
            "This kubeconfig names no namespace, so k8rs had to guess payments and was refused \
             there too. Say which namespace you work in: --namespace <name>",
        ),
        (
            true,
            Coverage::Cluster,
            "Ask whoever runs this cluster for a role that may read pods in every namespace — \
             `k8rs-readonly` in the k8rs docs is that role — or quit and start k8rs again in one \
             namespace you can read: --namespace <name>",
        ),
        (true, Coverage::Asked(payments()), named),
        (true, Coverage::Refused(payments()), named),
        (
            true,
            Coverage::Blind(payments()),
            "This kubeconfig names no namespace, so k8rs had to guess payments and was refused \
             there too. Quit and start k8rs again in the namespace you work in: --namespace <name>",
        ),
    ] {
        let at = || format!("running {running}, {coverage:?}");
        assert_eq!(
            next_step(Fault::Refused, &coverage, "pods", running).as_deref(),
            Some(expected),
            "{}",
            at()
        );
        for (fault, expected) in [
            (
                Fault::Unanswered,
                "Check the server address this kubeconfig names, and that this machine can reach \
                 it",
            ),
            (
                Fault::Gone,
                "Check the server address this kubeconfig names — as written, it does not lead to \
                 a Kubernetes API server",
            ),
        ] {
            assert_eq!(
                next_step(fault, &coverage, "pods", running).as_deref(),
                Some(expected),
                "{fault:?}, {}",
                at()
            );
        }
        for fault in [
            Fault::Expired,
            Fault::Rejected,
            Fault::Conflict,
            Fault::Unfinished,
            Fault::Kubeconfig,
            Fault::NoContext,
            Fault::BadEntry,
            Fault::NoCredential,
        ] {
            assert_eq!(
                next_step(fault, &coverage, "pods", running),
                None,
                "{fault:?}, {}",
                at()
            );
        }
    }

    // **A namespace nobody stripped is stripped here, running or not** — `--namespace` is argv, and
    // argv never met `k8s::text` (invariant 9).
    let crafted = Coverage::Blind("pay\u{1b}\u{202e}ments".to_owned());
    assert_eq!(scope(&crafted), "in the namespace payments");
    for running in [false, true] {
        let next = next_step(Fault::Refused, &crafted, "pods", running)
            .expect("a refusal has a next step");
        assert!(next.contains("guess payments and"), "{next:?}");
        assert!(!next.chars().any(unprintable), "{next:?}");
    }
}

// --- WHICH POD OF A GROUP, AND THE SLOT THAT HOLDS THE STEP ---

/// `screens/detail.md` § Picking a pod — **the count and the rows under it are one list.**
///
/// [`Card::affected`] is the numerator of `3 of 5 pods` and [`Card::pods`] is what the step draws
/// a row of each; a second scan for either is how a card comes to say `3 pods` over four rows.
/// The rule it carries is NOTES § D39's: distinct over the **whole** `ObjectId`, uid included, and
/// only `Pod`-kind objects counted at all.
#[test]
fn the_pod_count_and_the_rows_the_step_draws_are_one_scan() {
    let owner = id(ObjectKind::Deployment, Some("payments"), "web", Some("u-w"));
    let pod = |name: &str, uid: &str| id(ObjectKind::Pod, Some("payments"), name, Some(uid));
    let findings = vec![
        finding(Severity::Critical, owner.clone(), pod("web-a", "u-a"), None),
        // **The same name, a different uid: a pod deleted and recreated is two objects**, which is
        // the whole reason distinct is the id and not the name.
        finding(Severity::Warn, owner.clone(), pod("web-a", "u-a2"), None),
        // A second finding about a pod already counted adds no row and no count.
        finding(Severity::Warn, owner.clone(), pod("web-a", "u-a"), None),
        // An object that is not a pod is neither counted nor listed (NOTES § D39).
        finding(
            Severity::Warn,
            owner.clone(),
            id(
                ObjectKind::ReplicaSet,
                Some("payments"),
                "web-7d9",
                Some("u-r"),
            ),
            None,
        ),
    ];

    let built = cards(&findings, &[], &now());
    let [card] = &built[..] else {
        panic!("one owner is one card: {built:?}");
    };
    assert_eq!(
        card.pods()
            .iter()
            .map(|pod| (pod.name.as_str(), pod.uid.as_deref()))
            .collect::<Vec<_>>(),
        vec![("web-a", Some("u-a")), ("web-a", Some("u-a2"))],
        "the step would draw a row the count does not know about"
    );
    assert_eq!(
        card.affected,
        card.pods().len(),
        "`3 of 5 pods` and the rows under it disagree"
    );
}

/// `screens/detail.md` § Picking a pod — **the step's own closed set: five keys, and no `/`.**
///
/// That section refuses a filter by name — a picker over one card's own pods is already narrower
/// than the list D3 was written to shrink — and it names no tab key either, because there is no
/// tab to be on until a pod is chosen. **Whatever view it was opened over**, since the step is
/// drawn over a view and not instead of one.
#[test]
fn the_which_pods_step_offers_five_keys_and_never_a_filter() {
    for view in [View::Alerts, View::Resources(2), View::Analysis(1)] {
        for tab in Tab::ALL {
            let app = App {
                view,
                tab,
                ..App::default()
            };
            let (keys, quit) = app.footer(Detailing::Pods, ORDINARY, Refused::default(), "", &[]);
            assert_eq!(
                (keys.as_ref(), quit),
                ("↑↓ move  ⏎ open  esc back  ? all keys  q quit", ""),
                "{view:?} · {tab:?}"
            );
        }
    }
}

/// `screens/detail.md` § Picking a pod — **nothing is pickable *inside* the step.**
///
/// [`picking`] answers *is there a container to choose between*, and the step has no object and
/// therefore no container. A value that said yes here would put `c container` on a footer with no
/// tab under it and open a picker over nothing.
#[test]
fn the_step_is_not_a_container_picker() {
    assert!(!picking(Detailing::Pods));
    assert!(!picking(Detailing::Closed));
    for many in [0, 1] {
        assert!(
            !picking(Detailing::Tabs {
                containers: many,
                from_step: false,
            }),
            "{many}"
        );
    }
    for many in [2, 40] {
        assert!(
            picking(Detailing::Tabs {
                containers: many,
                from_step: false,
            }),
            "{many}"
        );
    }
}

/// `screens/widgets.md` § 2b, `screens/detail.md` § Picking a pod — **a filter survives `esc` back
/// from the step**, exactly as it survives `esc` back from a detail tab: the step is drawn *over*
/// the view whose filter it is, so going back is not going somewhere else.
#[test]
fn esc_out_of_the_which_pods_step_leaves_the_filter_where_it_was() {
    let filtered = || App {
        filters: Filters {
            text: buffer("web"),
            namespace: buffer("pay"),
        },
        ..App::default()
    };

    let mut app = filtered();
    assert!(!app.escape(Detailing::Pods));
    assert_eq!(
        app.filters,
        filtered().filters,
        "`esc back` cleared a filter the step was drawn over"
    );

    // And with the step closed it is the ordinary at-rest `esc`, narrow to wide.
    let mut app = filtered();
    assert!(!app.escape(Detailing::Closed));
    assert!(app.filters.text.is_empty() && !app.filters.namespace.is_empty());
}

/// `screens/alerts.md` § A card with more than one finding — **newer first, and *no drawable age*
/// last**, which is the half of the sort every derived ordering available here gets backwards.
///
/// It is asserted on [`recency`] itself because two lists now share it — the card list and the
/// which-pods step's rows — and a copy in either would be the one that drifts (NOTES § D69).
#[test]
fn recency_puts_the_newer_first_and_the_ageless_last() {
    let old = at(10);
    let new = at(20);
    assert_eq!(recency(Some(&new), Some(&old)), Ordering::Less);
    assert_eq!(recency(Some(&old), Some(&new)), Ordering::Greater);
    assert_eq!(
        recency(None, Some(&old)),
        Ordering::Greater,
        "an ageless row sorted above one with an age"
    );
    assert_eq!(recency(Some(&old), None), Ordering::Less);
    assert_eq!(recency(None, None), Ordering::Equal);
}

/// `screens/detail.md` § A group of one pod, or none at all — **`⏎` opens the step only where
/// there is a group**, and it puts the step's cursor back at the top when it does (NOTES § D270).
///
/// **The three shapes that are not a group**: a node card counts no pods, a bare pod's card is the
/// pod itself, and `affected == 1` leaves one candidate. A node card reached the step and rendered
/// **zero rows** under a footer promising `↑↓ move  ⏎ open` — *"a key that does nothing is a bug
/// already shipped once here"* (`tester`, 2026-09-18).
#[test]
fn enter_opens_the_step_only_where_the_card_is_about_two_pods_or_more() {
    let owner = id(ObjectKind::Deployment, Some("payments"), "web", Some("u-w"));
    let pod = |name: &str| id(ObjectKind::Pod, Some("payments"), name, Some(name));
    let of = |owner: ObjectId, pods: &[&str], total| Card {
        findings: pods
            .iter()
            .map(|name| finding(Severity::Critical, owner.clone(), pod(name), None))
            .collect(),
        affected: pods.len(),
        owner,
        total,
    };

    let mut app = App::default();
    assert!(
        app.pick_pods(&of(owner.clone(), &["web-a", "web-b"], Some(5))),
        "two pods of five is the group this step exists for"
    );

    // A node card: `affected == 0`, so `Card::count` is `None` and there is nothing to list.
    let node = id(ObjectKind::Node, None, "node-3", Some("u-n"));
    let mut cordoned = of(node.clone(), &[], None);
    cordoned
        .findings
        .push(finding(Severity::Warn, node.clone(), node, None));
    assert!(!app.pick_pods(&cordoned), "a node card has no pods to pick");

    // A bare pod: `owner == object`, so nothing owns it and there is no group to speak of.
    let bare = pod("broken-pending");
    let mut alone = of(bare.clone(), &[], None);
    alone
        .findings
        .push(finding(Severity::Critical, bare.clone(), bare, None));
    assert!(!app.pick_pods(&alone), "a bare pod is its own card");

    // One pod, however many findings name it: `⏎` opens that pod directly.
    assert!(
        !app.pick_pods(&of(owner.clone(), &["web-a"], Some(5))),
        "one candidate is not a group"
    );
}

/// `screens/detail.md` § Picking a pod — **the step's cursor starts at the top of the group it was
/// opened on, never where the last group left it** (NOTES § D270).
///
/// [`App::open`] resets [`App::content`] on a view change; nothing resets [`App::pods`] between two
/// cards of one view. Landing on row 7 of card A, `esc`, then opening card B left
/// [`Cursor::follow`] missing A's anchor and falling back to `select(self.index)` — **row 7 of a
/// different group, with `⏎` armed on it** (`tester`, 2026-09-18, measured).
#[test]
fn opening_the_step_on_a_second_card_does_not_inherit_the_first_cards_row() {
    let owner = id(ObjectKind::Deployment, Some("payments"), "web", Some("u-w"));
    let group = |names: &[&str]| Card {
        findings: names
            .iter()
            .map(|name| {
                finding(
                    Severity::Critical,
                    owner.clone(),
                    id(ObjectKind::Pod, Some("payments"), name, Some(name)),
                    None,
                )
            })
            .collect(),
        affected: names.len(),
        owner: owner.clone(),
        total: Some(40),
    };
    let first = group(&["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"]);
    fn anchors(card: &Card) -> Vec<Option<&str>> {
        card.pods()
            .into_iter()
            .map(|pod| Some(pod.name.as_str()))
            .collect()
    }

    let mut app = App::default();
    assert!(app.pick_pods(&first));
    let keys = anchors(&first);
    app.pods.select(7, &keys);
    assert_eq!(app.pods.selected(&keys), Some(7));

    // A second card, whose names the first card's anchor does not appear in.
    let second = group(&["b0", "b1", "b2", "b3", "b4", "b5", "b6", "b7"]);
    assert!(app.pick_pods(&second));
    let keys = anchors(&second);
    assert_eq!(
        app.pods.selected(&keys),
        Some(0),
        "the step opened on row 7 of a group the reader had not looked at"
    );
}

/// **`Offer::act` never offers `s`, for any kind, any group and any login** — `SCALE_IS_BUILT`, and
/// `screens/widgets.md` § 2a's own *"`s scale` is never part of it"* (`screens/help.md` § Rules,
/// `screens/dialogs.md` § Choosing how many, before the confirm box, which is the step that has to
/// exist first).
///
/// **The kinds walked are the ones `ops::scalable` serves**, so the assertion is over exactly the
/// inputs that used to answer `true` — a list written here would be a second opinion about which
/// kinds scale, which is the thing `Offer::act`'s own doc refuses.
#[test]
fn no_kind_is_offered_the_scale_key_while_the_count_step_does_not_exist() {
    for (group, kind) in [
        ("apps", "deployment"),
        ("apps", "statefulset"),
        ("apps", "replicaset"),
        ("apps", "daemonset"),
        ("", "pod"),
        ("", "node"),
    ] {
        let offer = Offer::act(group, kind);
        let Offer::Act { scalable, .. } = offer else {
            panic!("{group}/{kind} is not an Act at all: {offer:?}")
        };
        assert!(
            !scalable,
            "{group}/{kind} was offered `s` while there is nowhere to type a copy count"
        );
        // And the key cannot be pressed either, which is the other half of the same fact.
        let app = App::default();
        assert!(
            !app.may_mutate(offer, Op::Scale),
            "{group}/{kind} left `s` pressable off the line"
        );
    }
    // **`restartable` is untouched by the ruling**, which is what keeps this a withholding of one
    // key rather than of the pair: a DaemonSet still restarts.
    assert!(matches!(
        Offer::act("apps", "daemonset"),
        Offer::Act {
            restartable: true,
            ..
        }
    ));
}
