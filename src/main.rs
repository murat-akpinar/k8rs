//! k8rs — placeholder.
//!
//! The build order is bottom-up (CLAUDE.md § Architecture workflow): `rules.rs`
//! and `analysis.rs` are proven against fixtures before anything touches a
//! cluster or a terminal, and `main.rs` is the last file to be wired. Until
//! then this binary exists so the crate builds, lints and publishes.

// A module no `mod` line reaches is not in the crate at all, so `rules.rs` is declared the
// moment it exists rather than when something calls it (NOTES § D34).
mod rules;

fn main() {
    println!("k8rs {} — not built yet.", env!("CARGO_PKG_VERSION"));
    println!("Progress: https://github.com/murat-akpinar/k8rs");
}
