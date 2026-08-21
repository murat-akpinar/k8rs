//! `analysis.rs` § THE REPORT SHAPE — its tests (NOTES § D91).
//!
//! The two sweeps that read every producer at once stay in `analysis_tests.rs`, beside the list
//! they sweep.

use super::*;

// --- THE RESTART ROW ---

/// Phase 4's later box: a container that keeps dying between its restarts draws nothing from
/// rules 1, 2, 5 or 6, so the row exists precisely because there is no finding
/// (NOTES § D101). Its title and its home are `tui-designer`'s to settle; what is asserted
/// here is only the shape's half of the claim.
pub(super) fn restarts() -> Report {
    Report {
        title: "Containers that keep dying and coming back".to_string(),
        badge: None,
        rows: vec![answer(
            None,
            "payments/web-7d9f4 · retry  47 restarts  this run 3 min",
            &[],
            Some(Jump::Object(object(
                ObjectKind::Pod,
                Some("payments"),
                "web-7d9f4",
            ))),
        )],
    }
}

#[test]
fn the_restart_row_jumps_to_a_pod_and_never_to_a_finding() {
    let report = restarts();
    let row = &report.rows[0];

    let Some(Jump::Object(id)) = jump_of(row) else {
        panic!("there is no finding here — that is the whole reason the row exists");
    };
    assert_eq!(id.kind, ObjectKind::Pod);
    assert_eq!(id.name, "web-7d9f4");

    // The container is not broken right now, so the row makes no judgement.
    assert_eq!(severity_of(row), None);

    // It may not re-spell how the last run ended: `ending` and `exit_meaning` are private to
    // `rules.rs` and a raw `exit 137` here is the defect D85 exists to prevent.
    assert!(
        !text_of(row).contains("exit "),
        "no exit code is spelled in a report row"
    );
}
