//! The write path — the only file in the crate permitted to mutate cluster state.
//!
//! `clippy.toml` bans every `&self` method of `Api` and `Request` outside invariant 1's
//! allowlist, crate-wide, and the attribute below is the single visible exception to it: one
//! file to audit, one line that announces it (NOTES § Operations, "Structural consequence —
//! writes live in exactly one file"; CLAUDE.md invariant 1). The split is mechanical and not a
//! judgement about what mutates — `namespace` mutates nothing and is banned, `may_i` mutates
//! nothing and belongs here, because it is performed with `create` (NOTES § D23).
//!
//! **The ban is not the whole containment.** `Client::request` and `Client::send` are
//! verb-agnostic — the verb is data in the request object — and are off the list on purpose,
//! since Phase 5 needs both outside this file for reads. A write built as a hand-verbed request
//! is stopped by CLAUDE.md invariant 2 and by review, not by the lint (NOTES § D142).
#![allow(clippy::disallowed_methods)]
