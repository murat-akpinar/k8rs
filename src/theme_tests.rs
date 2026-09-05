//! Tests for [`super`] — the palette against NOTES § Design, and every non-colour signal
//! against the carrier the screen file promises it.

use super::*;

/// `theme.rs` as text, read at compile time — `k8s_tests.rs` derives a field list off the type
/// that owns it the same way. [`PALETTE`] and [`SIGNALS`] below are hand-written, so a constant
/// added to the product file and left out of them escapes every assertion in this file; the two
/// counts at the foot are what notices.
const SOURCE: &str = include_str!("theme.rs");

/// The ten roles, paired with the hex NOTES § Design writes them as. The literal in the product
/// file and the literal here are two transcriptions of one table, which is the only way a
/// swapped pair (`WARN` and `OK` crossed) is caught at all.
const PALETTE: [(&str, Colour, &str); 10] = [
    ("background", BACKGROUND, "#1e1e2e"),
    ("panel", PANEL, "#313244"),
    ("border", BORDER, "#6c7086"),
    ("text", TEXT, "#cdd6f4"),
    ("dim", DIM, "#a6adc8"),
    ("accent", ACCENT, "#94e2d5"),
    ("critical", CRITICAL, "#f38ba8"),
    ("warn", WARN, "#fab387"),
    ("ok", OK, "#a6e3a1"),
    ("info", INFO, "#89b4fa"),
];

/// Every meaning on the screen, paired with the carrier its screen file promises — transcribed
/// by hand out of `screens/once.md` (`● ▲ ○`), `alerts.md` (`▸`), `dialogs.md` (`changing…`)
/// and `states.md` (`⚠`, `read-only`), so the literal here and the literal in the product file
/// are two transcriptions of one table, like [`PALETTE`]'s hexes.
///
/// **The screen files are deliberately not read at compile time.** `Cargo.toml`'s `exclude`
/// keeps `screens/` out of the published package (NOTES § D193), so an
/// `include_str!("../screens/…")` here compiles for us and fails for anyone who runs the suite on
/// the crate they downloaded — and `cargo publish` verifies with a *build*, which never compiles
/// a `#[cfg(test)]` module, so it would ship green and break in their hands. Reading `screens/`
/// belongs to a `just check` guard, where the file is always on disk.
///
/// Focus is the one row that is not a mark and may not become one — not because every row is
/// already marked (`alerts.md` and `analysis.md` mark nothing but the sidebar) but because focus
/// belongs to a *pane*, which has no row to hang a mark on when it is empty. `alarm` is the
/// opposite case: only the glyph is pinned here, because the sentence after it differs per
/// situation across `states.md` and `context.md` and belongs to the caller (see [`ALARM`]).
const SIGNALS: [(&str, Signal, Signal); 8] = [
    ("critical", CRITICAL_SIGNAL, Signal::Mark("●")),
    ("warn", WARN_SIGNAL, Signal::Mark("▲")),
    ("info", INFO_SIGNAL, Signal::Mark("○")),
    ("selection", SELECTION, Signal::Mark("▸")),
    ("focus", FOCUS, Signal::Reverse),
    ("changing", CHANGING, Signal::Mark("changing…")),
    ("alarm", ALARM, Signal::Mark("⚠")),
    ("read-only", READ_ONLY, Signal::Mark("read-only")),
];

/// `str::trim` strips `White_Space` only, and U+200B is not in it — a zero-width space passes an
/// emptiness check while drawing nothing (`tester`, 2026-09-05). These are the invisible
/// characters a mark could plausibly pick up: soft hyphen, zero-width space, the two joiners,
/// word joiner and the byte-order mark.
const INVISIBLE: [char; 6] = [
    '\u{00ad}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}',
];

fn hex(colour: Colour) -> String {
    let (r, g, b) = colour.rgb;
    format!("#{r:02x}{g:02x}{b:02x}")
}

// --- THE PALETTE ---

#[test]
fn every_role_carries_the_catppuccin_value_notes_design_gives_it() {
    for (role, colour, expected) in PALETTE {
        assert_eq!(
            hex(colour),
            expected,
            "{role} is not the hex NOTES § Design writes"
        );
    }
}

#[test]
fn no_two_roles_are_the_same_colour() {
    for (i, (role, colour, _)) in PALETTE.iter().enumerate() {
        for (other, other_colour, _) in &PALETTE[i + 1..] {
            assert_ne!(
                colour.rgb, other_colour.rgb,
                "{role} and {other} draw the same colour"
            );
        }
    }
}

#[test]
fn truecolor_draws_the_rgb_value() {
    assert_eq!(CRITICAL.ink(Depth::TrueColor), Ink::Rgb(0xf3, 0x8b, 0xa8));
    assert_eq!(BACKGROUND.ink(Depth::TrueColor), Ink::Rgb(0x1e, 0x1e, 0x2e));
}

/// The negative half of the one above: at sixteen colours no role may still ask for RGB, and
/// no fallback may name an index the sixteen do not have.
#[test]
fn the_fallback_never_asks_for_rgb_and_stays_inside_the_sixteen() {
    for (role, colour, _) in PALETTE {
        match colour.ink(Depth::Ansi16) {
            Ink::Rgb(..) => panic!("{role} still draws 24-bit on a terminal that has 16 colours"),
            Ink::Ansi(index) => assert!(index <= 15, "{role} falls back to index {index}"),
            Ink::Default => {}
        }
    }
}

/// The three roles with no band to state — two fills and the body text — hand the terminal
/// back its own scheme, because the background is not known to be dark (PRIOR-ART § D2).
#[test]
fn the_background_roles_degrade_to_the_terminals_own() {
    assert_eq!(BACKGROUND.ink(Depth::Ansi16), Ink::Default);
    assert_eq!(PANEL.ink(Depth::Ansi16), Ink::Default);
    assert_eq!(TEXT.ink(Depth::Ansi16), Ink::Default);
}

/// The seven roles that do name one of the sixteen take a *basic* colour, never a bright one:
/// bright is tuned for a dark background, and it is what an emulator renders "intense" as.
/// Index 8 — bright black — is the single exception and both greys land on it; index 7 is barred
/// separately because a role that falls back to white is invisible on a light terminal.
#[test]
fn no_fallback_is_bright_or_white_apart_from_index_8_for_the_two_greys() {
    for (role, colour, _) in PALETTE {
        if let Ink::Ansi(index) = colour.ink(Depth::Ansi16) {
            assert!(index <= 8, "{role} falls back to a bright colour ({index})");
            assert!(
                index != 7,
                "{role} falls back to white, invisible on a light terminal"
            );
        }
    }
    assert_eq!(ACCENT.ink(Depth::Ansi16), Ink::Ansi(6));
    assert_eq!(CRITICAL.ink(Depth::Ansi16), Ink::Ansi(1));
    assert_eq!(WARN.ink(Depth::Ansi16), Ink::Ansi(3));
    assert_eq!(OK.ink(Depth::Ansi16), Ink::Ansi(2));
    assert_eq!(INFO.ink(Depth::Ansi16), Ink::Ansi(4));
    assert_eq!(BORDER.ink(Depth::Ansi16), Ink::Ansi(8));
    assert_eq!(DIM.ink(Depth::Ansi16), Ink::Ansi(8));
}

// --- THE COLORTERM CHECK ---

#[test]
fn the_two_values_the_convention_defines_mean_24_bit() {
    for value in ["truecolor", "24bit", "TrueColor", "TRUECOLOR", "24BIT"] {
        assert_eq!(depth(Some(value)), Depth::TrueColor, "COLORTERM={value}");
    }
}

#[test]
fn anything_else_degrades_including_a_value_that_only_claims_colour() {
    for value in [
        None,
        Some(""),
        Some("1"),
        Some("256color"),
        Some("xterm-256color"),
        Some("yes"),
        Some("truecolour"),
    ] {
        assert_eq!(depth(value), Depth::Ansi16, "COLORTERM={value:?}");
    }
}

// --- COLOUR IS NEVER THE ONLY CARRIER ---

/// The rule the box exists for, stated as an assertion: `Signal::Mark("")` is representable
/// and is exactly "colour alone" wearing a carrier's name.
///
/// **"Empty" has to mean *invisible*, not *`trim`s to nothing*.** This asserted
/// `!mark.trim().is_empty()` until 2026-09-05, and `str::trim` strips `White_Space`, a property
/// U+200B does not have — so a zero-width space was a carrier that drew nothing and passed
/// (`tester`). [`INVISIBLE`] is the rest of that class.
#[test]
fn no_meaning_is_left_to_colour_alone() {
    for (meaning, signal, _) in SIGNALS {
        if let Signal::Mark(mark) = signal {
            assert!(
                mark.chars()
                    .any(|c| !c.is_whitespace() && !INVISIBLE.contains(&c)),
                "{meaning} carries nothing but its colour"
            );
        }
    }
}

/// Common Unicode only — no nerd font, no emoji. Both live at or above U+E000.
#[test]
fn no_mark_needs_a_font_the_reader_may_not_have() {
    for (meaning, signal, _) in SIGNALS {
        if let Signal::Mark(mark) = signal {
            for c in mark.chars() {
                assert!(c < '\u{e000}', "{meaning} draws {c:?} (U+{:04X})", c as u32);
            }
        }
    }
}

/// The code matches the screen file, or the screen file changes first. The two transcriptions are
/// compared *whole*: this asserted `screen.contains(mark)` until 2026-09-05, and a substring
/// search passes anything a mark is truncated to — `read-only` cut to `read`, or to `r`, and six
/// such plantings survived the suite (`tester`).
#[test]
fn every_carrier_is_drawn_verbatim() {
    for (meaning, carrier, promised) in SIGNALS {
        assert_eq!(
            carrier, promised,
            "{meaning}'s carrier is not what the screen file promises"
        );
    }
}

// --- THE ONE PAIRING ---

/// Each severity with the colour *and* the mark the screens pair it with — transcribed from
/// `screens/once.md`'s band and NOTES § Design's hexes, **never from [`band`]'s own match arms**.
/// Copying the arms would pass a crossed pairing (`Critical` handed `WARN_SIGNAL`) straight
/// through, which is the one defect this mapping exists to make impossible.
const BANDS: [(Severity, &str, &str); 3] = [
    (Severity::Critical, "#f38ba8", "●"),
    (Severity::Warn, "#fab387", "▲"),
    (Severity::Info, "#89b4fa", "○"),
];

/// Exhaustive over the three variants, and the *pairing* is what is asserted: that critical draws
/// `CRITICAL` **and** `●`, not merely that the call returns something.
///
/// A fourth `Severity` needs no test of its own — [`band`]'s match is exhaustive, so `rules.rs`
/// adding one is a compile error in the product file, which is louder than an assertion here.
#[test]
fn every_severity_draws_the_colour_and_the_mark_the_screens_pair_it_with() {
    for (severity, colour, mark) in BANDS {
        let (drawn, signal) = band(severity);
        assert_eq!(hex(drawn), colour, "{severity:?} draws the wrong colour");
        assert_eq!(
            signal,
            Signal::Mark(mark),
            "{severity:?} draws the wrong mark"
        );
    }
}

#[test]
fn the_three_severity_marks_are_distinct() {
    assert_ne!(CRITICAL_SIGNAL, WARN_SIGNAL);
    assert_ne!(WARN_SIGNAL, INFO_SIGNAL);
    assert_ne!(CRITICAL_SIGNAL, INFO_SIGNAL);
}

// --- THE TWO ARRAYS ARE HAND-WRITTEN, AND THIS IS WHAT NOTICES ---

/// A role added to the product file and not to [`PALETTE`] is asserted about by nothing — not its
/// hex, not its fallback. Today the module's `expect(dead_code)` catches it by accident; that
/// expectation goes when Phase 11 reads the file, and it does not expire on its own
/// (`theme.rs`, the module attribute). Counting the declarations in the source is what is left.
#[test]
fn every_colour_const_in_the_file_is_in_the_palette_array() {
    assert_eq!(
        SOURCE.matches(": Colour = Colour {").count(),
        PALETTE.len(),
        "a colour in theme.rs is missing from PALETTE, or PALETTE names one that is gone"
    );
}

/// The same hole on the other array — and the constant that would fall through it is the one the
/// box exists to forbid: a `Signal::Mark("")`, which is colour alone wearing a carrier's name.
#[test]
fn every_signal_const_in_the_file_is_in_the_signals_array() {
    assert_eq!(
        SOURCE.matches(": Signal = Signal::").count(),
        SIGNALS.len(),
        "a signal in theme.rs is missing from SIGNALS, or SIGNALS names one that is gone"
    );
}
