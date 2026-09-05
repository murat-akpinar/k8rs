//! The palette and the non-colour signals — the single point of change for every colour and
//! every mark k8rs draws (NOTES § Design).
//!
//! **Nothing here names a `ratatui` type, and that is the point** (NOTES § D241). This file is
//! frozen at Phase 9's close, and it has two consumers that do not share a renderer: the TUI
//! from Phase 11, and the `--once` report, which has no ratatui between the findings and the
//! terminal at all (`screens/once.md § The rule that matters most here`). So a role is *data* —
//! its Catppuccin value, the one of the sixteen it degrades to, and nothing about how either is
//! spelled on the wire. Turning an [`Ink`] into `Color::Rgb` or an SGR escape is a mechanical
//! mapping that lives in the unfrozen file that draws.
//!
//! **Colour is never the only carrier of meaning** (PRIOR-ART § K, § D2). Every meaning on the
//! screen also has a [`Signal`]: a mark that survives a monochrome terminal and a copy-paste, or
//! reverse video. Bold is not on that list — its rendering is a per-emulator setting, and
//! Windows Terminal draws it as a *bright colour*, which is how k9s's selected row became grey
//! on grey.

// Nothing outside `#[cfg(test)]` reads this file yet: the two renderers that will are Phase 11's
// `ui.rs` and the `--once` printer, and NOTES § D241 is why the mapping to a drawn colour lives
// there rather than here. `not(test)` because this file's own tests read all of them, and
// `-D warnings` rejects an unfulfilled expectation; the pattern and the accepted blind spot are
// `ops.rs`'s (NOTES § D38).
//
// **This line does not expire by itself. Phase 11 has to delete it by hand.** It claimed it did
// until 2026-09-05, and the claim was measured false: with all eighteen items read from
// `fn main()` outside `#[cfg(test)]`, `cargo check`, `clippy`, `clippy --all-targets` and
// `cargo test --bin k8rs` all exit 0 with no unfulfilled expectation, under `RUSTFLAGS=-D
// warnings` and `CARGO_INCREMENTAL=0`. The control is this same attribute in this same position
// with only the lint swapped — `expect(unused_mut, …)` *is* reported unfulfilled and exits 101 —
// so the position, the `cfg_attr` wrapper and `-D warnings` are all working, and `dead_code`
// specifically does not report an unfulfilled expectation in this crate. Why is not known and
// was not chased. `expect` is kept over `allow` because it costs nothing and would still fire if
// that ever changes; what is actually scheduled to notice a constant this file grows is the pair
// of count assertions at the foot of `theme_tests.rs`.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the renderers that read the palette are Phase 11's (NOTES § D241)"
    )
)]

use crate::rules::Severity;

// --- WHAT A COLOUR IS START ---

/// A colour as a terminal can be told to draw it, once the depth is known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    /// 24-bit. Only reachable on a terminal that said so.
    Rgb(u8, u8, u8),
    /// One of the sixteen, by its standard index (0–15).
    Ansi(u8),
    /// The terminal's own colour. Nothing is emitted and the user's scheme wins — the only
    /// honest answer for the roles that paint an area or the body text, because the
    /// background is not known to be dark (PRIOR-ART § D2).
    Default,
}

/// How much colour the terminal admits to. There is no third state: whether colour is drawn
/// *at all* is a question about the output stream (a tty, `NO_COLOR`), not about the palette,
/// and it is answered where those inputs live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    /// 24-bit RGB: the Catppuccin values are drawn as written.
    TrueColor,
    /// Sixteen colours and the terminal's own default.
    Ansi16,
}

/// One role of the palette: what it is on a truecolor terminal, and what it degrades to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
    /// The Catppuccin Mocha value from NOTES § Design.
    pub rgb: (u8, u8, u8),
    /// The one of the sixteen this role degrades to, or `None` for the terminal's own colour.
    pub ansi: Option<u8>,
}

impl Colour {
    /// What to draw for this role at `depth`.
    pub const fn ink(self, depth: Depth) -> Ink {
        match depth {
            Depth::TrueColor => Ink::Rgb(self.rgb.0, self.rgb.1, self.rgb.2),
            Depth::Ansi16 => match self.ansi {
                Some(index) => Ink::Ansi(index),
                None => Ink::Default,
            },
        }
    }
}

/// The one `COLORTERM` check (NOTES § Design). The caller passes
/// `std::env::var("COLORTERM").ok().as_deref()`; the environment is an argument rather than a
/// read so that the whole of this file can be proven without one, the same reason `Snapshot`
/// carries its own clock (invariant 5).
///
/// Only the two values the convention defines mean 24-bit — `truecolor` and `24bit`, either
/// case. Everything else degrades, *including* a set-but-unrecognised value: a terminal that
/// says `COLORTERM=1` has claimed colour, not depth, and drawing RGB at it is the failure this
/// check exists to avoid.
pub fn depth(colorterm: Option<&str>) -> Depth {
    match colorterm {
        Some(value)
            if value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit") =>
        {
            Depth::TrueColor
        }
        _ => Depth::Ansi16,
    }
}

// --- WHAT A COLOUR IS END ---

// --- THE PALETTE START ---
// Catppuccin Mocha, accent = teal. Ten roles, transcribed from NOTES § Design and re-picked
// nowhere else.
//
// **The fallbacks are the eight basic colours, never the eight bright ones.** Bright is tuned
// for a dark background and PRIOR-ART § D2 forbids assuming one; it is also what Windows
// Terminal renders "intense" as, which is the bug that made k9s unreadable. Index 8 is the one
// exception, and the reason is arithmetic rather than a survey: of the sixteen, 8 is the only
// index a scheme conventionally renders as a grey — 0 is its black, 7 its white, 15 its bright
// white — so both grey roles have nowhere else to land and both take it, and the fallback has
// one grey where the palette has two. **Whether index 8 stays readable is unverified here, in a
// light scheme or a dark one**, and a scheme that maps it near its own background hides the
// frame and the evidence line. If one is ever measured doing that, the fix is [`BORDER`] and
// [`DIM`] degrading to `None` like the fills do — never a bright index.
//
// **On `TrueColor`, [`BACKGROUND`] and [`TEXT`] are an all-or-nothing pair: a renderer that
// paints one must paint the other.** At sixteen colours the question does not arise — both are
// `Ink::Default`, the user's scheme wins, and PRIOR-ART § D2 is satisfied by not answering. At
// 24-bit they stop being independent: `TEXT` is a near-white `#cdd6f4` picked to sit on `#1e1e2e`,
// so the *minimal* Phase 11 renderer — set foregrounds, leave the background alone, which is also
// what "the terminal's own colour wins" reads like — draws near-white on a light terminal's white
// and loses the body text. Emulators export `COLORTERM=truecolor` whatever scheme the user chose,
// so a light-Solarized reader takes that branch; [`depth`] cannot tell them apart and is not
// being asked to. **Reasoned off `depth()` and these two constants, not measured on a light
// terminal** (`tui-designer`, Phase 9 close). No value changes here: if it is ever measured, the
// fix is in the file that draws.

/// The window behind everything. `base` `#1e1e2e`.
pub const BACKGROUND: Colour = Colour {
    rgb: (0x1e, 0x1e, 0x2e),
    ansi: None,
};

/// A panel, and the selected row's fill. `surface0` `#313244`. It degrades to nothing on
/// purpose — [`SELECTION`] and [`FOCUS`] carry the selection without it, which is what makes
/// the row visible on a terminal that has no shade between its background and its text.
pub const PANEL: Colour = Colour {
    rgb: (0x31, 0x32, 0x44),
    ansi: None,
};

/// The frame. `overlay0` `#6c7086`.
pub const BORDER: Colour = Colour {
    rgb: (0x6c, 0x70, 0x86),
    ansi: Some(8),
};

/// Everything a reader is meant to read first. `text` `#cdd6f4`.
pub const TEXT: Colour = Colour {
    rgb: (0xcd, 0xd6, 0xf4),
    ansi: None,
};

/// The second and last level of hierarchy — the evidence line under a finding's title.
/// `subtext0` `#a6adc8`.
pub const DIM: Colour = Colour {
    rgb: (0xa6, 0xad, 0xc8),
    ansi: Some(8),
};

/// The one accent. `teal` `#94e2d5`.
pub const ACCENT: Colour = Colour {
    rgb: (0x94, 0xe2, 0xd5),
    ansi: Some(6),
};

/// Broken right now, and nothing else. `red` `#f38ba8`.
pub const CRITICAL: Colour = Colour {
    rgb: (0xf3, 0x8b, 0xa8),
    ansi: Some(1),
};

/// Wrong now, broken soon. `peach` `#fab387`.
pub const WARN: Colour = Colour {
    rgb: (0xfa, 0xb3, 0x87),
    ansi: Some(3),
};

/// Healthy. `green` `#a6e3a1`.
pub const OK: Colour = Colour {
    rgb: (0xa6, 0xe3, 0xa1),
    ansi: Some(2),
};

/// Worth knowing, and the kubectl line in the command log. `blue` `#89b4fa`.
pub const INFO: Colour = Colour {
    rgb: (0x89, 0xb4, 0xfa),
    ansi: Some(4),
};

// --- THE PALETTE END ---

// --- COLOUR IS NEVER THE ONLY CARRIER START ---

/// The non-colour half of a meaning. Two mechanisms, because there are only two a terminal
/// gives us that are not a per-emulator setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Text drawn with the value — a glyph (`●`), a word (`read-only`), or both. It survives a
    /// monochrome terminal, a screenshot and a copy-paste, which is the whole argument
    /// (NOTES § Design). Common Unicode only, never a nerd font.
    Mark(&'static str),
    /// Foreground and background swapped. Terminal-native and colour-blind safe, and the fix
    /// k9s's own tracker asks for (PRIOR-ART § K, k9s#3955).
    Reverse,
}

/// Critical, on a finding and on a row that carries one.
pub const CRITICAL_SIGNAL: Signal = Signal::Mark("●");

/// Warning.
pub const WARN_SIGNAL: Signal = Signal::Mark("▲");

/// Nothing is broken — the band that opens `○ nothing is broken`, and an analysis row that is
/// only worth knowing.
pub const INFO_SIGNAL: Signal = Signal::Mark("○");

/// The row the keys would act on, in any list, table or picker.
///
/// **It is not yet drawn everywhere it will be, and the gap is in the mockups rather than here**
/// (measured over `screens/`, 2026-09-06): the sidebar marks its row on every screen that has one,
/// and in the *content* pane only three drawings mark theirs — the resource table
/// (`screens/resources.md`) and the two pickers (`detail.md`, `context.md`). The `▸` in
/// `alerts.md` and `analysis.md` is the sidebar's; no finding card and no report row in any mockup
/// carries a marker. Drawing it there is `tui-designer`'s change and is boxed, not done.
pub const SELECTION: Signal = Signal::Mark("▸");

/// Which pane those keys go to — the fill, not a mark.
///
/// **Not because every row is already marked** (it is not — see [`SELECTION`]), but because focus
/// is a property of a *pane* and a pane-level carrier cannot be a per-row mark: there is no row to
/// hang it on when the pane is empty, and two marks on one row would say the same thing twice.
/// The fill also has to survive [`PANEL`] degrading to nothing, which reverse video does and a
/// background colour does not.
///
/// **Reverse video does a second job Phase 11 has to draw with this same carrier**: a modal's
/// confirm button, reversed and live only once the dry-run has returned and — for a typed-name
/// dialog — once the typed string matches (`screens/widgets.md` § 5. The modal layer). One
/// mechanism, two meanings; `screens/` asks for no third.
pub const FOCUS: Signal = Signal::Reverse;

/// A mutation is in flight (`screens/dialogs.md`).
pub const CHANGING: Signal = Signal::Mark("changing…");

/// The fourth symbol, and it is **not** a severity: something is wrong with the connection to the
/// cluster, the trust in it, or the kubeconfig that describes it
/// (`screens/README.md` § The five rules every screen obeys, rule 4). It appears in the header
/// zone or a banner, never on a finding — `--once` refuses it outright for that reason, its
/// vocabulary being `● ▲ ○` and nothing else (`screens/once.md`).
///
/// **The constant is the glyph, not any one sentence it opens**, which is why it is named for the
/// mark rather than for the first meaning that needed one. Holding a single sentence would make
/// this file the single point of change for one member of a family whose other members `ui.rs`
/// would then hardcode. Measured over `screens/` on 2026-09-06, the family is drawn as
/// `⚠ disconnected, retrying`, `⚠ login expired`, `⚠ Not connected to the cluster right now.`,
/// `⚠ Your login expired.`, `⚠ your clock is behind` / `ahead` and `⚠ This computer and the
/// cluster disagree…` (`screens/states.md`), and `⚠ TLS not verified`, `⚠ not allowed`,
/// `⚠ cluster undefined` and `⚠ duplicate name` (`screens/context.md`) — no count is given here
/// because a count is the copy that goes stale, and the list is the evidence either way. The
/// sentences stay in the screen files and land in `views.rs` and `ui.rs`, which is where a string
/// that changes with the situation belongs; what freezes here is the mark in front of them.
pub const ALARM: Signal = Signal::Mark("⚠");

/// `--read-only` is on (`screens/states.md`).
pub const READ_ONLY: Signal = Signal::Mark("read-only");

// --- COLOUR IS NEVER THE ONLY CARRIER END ---

// --- THE ONE PAIRING START ---

/// The band a severity draws: its colour **and** its mark, decided here once.
///
/// Without this the pairing gets transcribed per renderer — `ui.rs` and the `--once` printer would
/// each write *critical ↔ red ↔ `●`* out longhand, and nothing would compare them. Two places
/// reading one thing and disagreeing is the defect class this repo has paid most for, and the
/// screens already assume it cannot happen here: `screens/analysis.md`
/// § How a report is drawn says *"`theme.rs` draws the glyph from `severity`"*, and
/// `screens/widgets.md` § 2. Element → widget says a badge "colours the text and adds nothing
/// else to it".
///
/// `rules.rs` sits *below* this file in the layer order (CLAUDE.md § Architecture workflow), so
/// reading its enum is the direction the pyramid allows and freezing this file does not close it.
/// Returning a pair rather than growing a `Badge` type is deliberate — nothing in `screens/` asks
/// for a type, and the sentence above is why.
///
/// **`--once` still transcribes both halves by hand**: `fn symbol` in `main.rs` holds `● ▲ ○`, and
/// `fn health` holds the literal `"○ nothing is broken"`. That file is `dev-core`'s and neither is
/// touched here (named rather than cited by line, because a line number in a frozen file is a
/// claim that goes stale on somebody else's next edit). Phase 11 routing both renderers through
/// `band` is what removes them; until then this is a second transcription, not yet the only one.
pub const fn band(severity: Severity) -> (Colour, Signal) {
    match severity {
        Severity::Critical => (CRITICAL, CRITICAL_SIGNAL),
        Severity::Warn => (WARN, WARN_SIGNAL),
        Severity::Info => (INFO, INFO_SIGNAL),
    }
}

// --- THE ONE PAIRING END ---

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
