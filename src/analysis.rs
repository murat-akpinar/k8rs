//! The analysis reports — the cluster-wide answers no per-object rule can give
//! (NOTES § Analysis reports). One layer above `rules.rs` and pure in exactly the same way: no
//! network, no terminal, no globals, no `Result`, and **no clock call** — `now` arrives on the
//! snapshot (CLAUDE.md invariant 5). The snapshot types stay in `rules.rs`, which is *below* this
//! file, and moving them up would invert the pyramid (NOTES § D42).
//!
//! **Producers are `fn(&rules::ClusterSnapshot, &[Finding]) -> Report`** — the snapshot **and the
//! findings [`crate::rules::analyze`] has already returned**. A row that jumps to a finding has no
//! other way to reach one: the rule functions are private to `rules.rs` and this module is its
//! sibling, not its child, so under a snapshot-only signature the Certificates report's only
//! routes to C1's card would be to run `analyze` a second time or to re-derive the expiry here —
//! two implementations of one rule. The findings are already in hand; `analyze` runs continuously
//! for Alerts. **A producer picks its finding out of that slice by identity, never by title** —
//! C1 is `object.kind == crate::rules::ObjectKind::Other("kubeconfig")` (`rules.rs` § the
//! certificate rules, NOTES § D51). A [`Finding::title`] is a plain-language sentence, so the next
//! invariant-14 pass rewords it and a match on one stops matching with nothing red: the
//! Certificates row keeps drawing and quietly loses its `⏎`.
//!
//! **The permanent watch does not carry every report's input** (invariant 6). Drain safety needs
//! PodDisruptionBudgets, Waste needs Services, EndpointSlices, PVCs and ReplicaSets, Certificates
//! needs a CSR list — none of them is on [`crate::rules::ClusterSnapshot`] today and none of them
//! is watched, so they arrive on the snapshot when the pane opens. **The report box that needs one
//! adds the field**, inside the one-phase window NOTES § D42 opens for exactly this; it does not
//! meet the gap cold.
//!
//! **Two more inputs are missing and neither is a list call, so D42's window does not cover
//! them.** The API server's own serving certificate — the C-series' C2 (NOTES § Certificate
//! rules, and not PRIOR-ART's C2, which is the loading-vs-empty defect class) — is the peer
//! certificate of a TLS handshake, which kube-rs does not expose: reaching it needs **a second
//! outbound connection**, and that is a Security gate question (the gate counts the outbound paths,
//! and the only connection is the API server in the user's kubeconfig) before it is a snapshot
//! field. Capacity's `using …` line is metrics, which is not a list call either: it needs a
//! capability probe first, and what it may do afterwards is already fenced — 30s+,
//! capability-gated, and only for what is on screen (`screens/widgets.md` § 1a). A box meeting
//! one of these adds a decision, not a field.
//!
//! **Three states, not two** (PRIOR-ART § C2, NOTES § D20): a producer **never runs on partial
//! input**. A report whose inputs are still arriving is `views.rs` holding an `Option<Report>` and
//! drawing the loading pane (`screens/widgets.md` § 2) — never a `Report` carrying a
//! [`Row::NotComputed`] that says *still loading*, which draws identically to *denied* and teaches
//! the reader that k8rs cannot see their cluster.
//!
//! This box is the shape only. The reports are the boxes after it, and each may still add a
//! **field or a case** it turns out to need — the destination [`Row::Answer::jump`] owes the Waste
//! box is a new [`Jump`] case, not a field. `analysis.rs` freezes at the *end* of Phase 4, not
//! here.

// `expect` rather than `allow` because it expires by itself, and whichever box constructs the
// last item in this file deletes this line — pre-authorised, not a freeze violation
// (NOTES § D38). Its module-wide blind spot is the same accepted one: an item written after it
// can be dead and invisible.
//
// **`not(test)`, and `rules.rs` needed no such thing.** The lint is evaluated per target and
// this file's tests construct and read every item, so under `cargo test` the expectation is
// fulfilled by nothing and `-D warnings` rejects the attribute itself. Gating it keeps the
// expiry D38 asked for on the binary — where the reports genuinely have no caller yet — and
// leaves `dead_code` live in the test target, where a field no test reads is a field no test
// asserts.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the reports that fill these in are the next boxes"
    )
)]

use crate::rules::{Finding, ObjectId, Severity};

// --- THE REPORT SHAPE START ---

/// One report — **one pane's worth of content**, and not one pane and one sidebar entry. Which
/// panes exist, and which of them share one, is `screens/`'s: `Versions` has its own sidebar entry
/// but is drawn at the foot of the Certificates pane (`screens/analysis.md` § *Certificates and
/// Versions*), and Phase 4's `Posture` report is in no sketch at all. So the count of `Report`s
/// and the count of panes are two facts, and this type states neither.
///
/// **Every string reachable from here is untrusted** (invariant 9): a row's text is built from
/// names and messages the API sent. Nothing in this file strips control characters. The guard
/// is `sanitize` in `main.rs`, and it runs **where a value enters a sentence, never over the
/// finished sentence** (NOTES § D122) — the same rule [`Finding`]'s own doc states, and
/// Phase 5's ingest strip supersedes both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The pane heading, in plain language and **not** the report's name: *"What each node
    /// promised, and what it has"*, never *"Capacity"* (invariant 14). The sidebar label is
    /// `views.rs`'s and is deliberately not a field here.
    pub title: String,
    /// The value beside this report's name in the sidebar, or **nothing at all**.
    ///
    /// **`None` means nothing is drawn there, and that covers three different reports**: one
    /// that never badges (`drain safety` carries nothing in the very mockup that draws
    /// `node-2  ● BLOCKS`), one that ran and found nothing, and one that could not run. Across
    /// all five mockups exactly two entries badge — `capacity  1 ▲` and `certificates  30d`.
    ///
    /// **The badge is not the discriminator and was never meant to be**: it "has room for a
    /// number, not for a reason, so the report itself carries the reason"
    /// (`screens/widgets.md` § 1a). The one place *did not run* is recorded is a
    /// [`Row::NotComputed`] in [`Report::rows`], which the screen needs anyway — a badge valued
    /// `0` carries nothing the body does not already say, and the sidebar has no room for the
    /// reason either way. (`Some("0")` and `None` are not one fact twice: *ran and found nothing*
    /// against *did not run*. The distinction is real and the **body** is where it is drawn.)
    pub badge: Option<Badge>,
    /// The body, top to bottom, in the order it is drawn. **A report that could not be
    /// computed at all is not an empty `Vec`** — it is one [`Row::NotComputed`]; an empty
    /// `Vec` says the check ran and had nothing to say.
    ///
    /// **That second state stays legal and no pane on `screens/analysis.md` asks for it**
    /// (NOTES § D128): a report with nothing to say says so in its own words, as one
    /// [`Row::Prose`], so `views.rs` carries no per-report empty text. Unreachable is not the
    /// same as forbidden — the line the first sentence draws is the one a renderer keys on,
    /// and a screen having no use for one side of it does not erase the other.
    pub rows: Vec<Row>,
}

/// The value beside a report's name in the sidebar — `capacity  1 ▲`, `certificates  30d`.
///
/// **The value and the band, never the glyph.** The `▲` above is drawn by `theme.rs`, which is
/// the single point of change for it; no field below ever carries one. (A doc comment may
/// quote a screen — a value may not be one.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    /// What is counted, already in the words the sidebar prints — `1`, `30d`. A count and a
    /// duration share one field because the sidebar draws them identically, as a right-aligned
    /// span inside the row (`screens/widgets.md` § 2).
    ///
    /// **A count or a duration — never a ratio, a percentage or an average**
    /// (`screens/widgets.md` § 1a, PRIOR-ART § F2). "An average of 74% hides the one node at
    /// 114%", and this is the worst place in the design for such a number: it is the one string
    /// k8rs prints with no room for a unit, a denominator or a sentence beside it. The per-node
    /// numbers stay in the report, where there is room to say what they mean.
    pub value: String,
    /// The band the value falls in — what `theme.rs` colours it by.
    pub severity: Severity,
}

/// One line of a report's body: an answer, a line that is only read, or the reason there is no
/// answer there.
///
/// **The variant says whether the cursor may land on the row.** [`Row::Answer`] is the one the
/// `↑↓ move  ⏎ open` footer of `screens/analysis.md` is about; `Prose` and `NotComputed` are
/// skipped, exactly as the sidebar's own group headers are (`screens/widgets.md` § 2). Nothing
/// keys selection on a field — that was the first draft's mistake and NOTES § D127 records it.
///
/// **A report holding no [`Row::Answer`] at all therefore has no cursor**, and its pane drops
/// `⏎ open` from the footer — which costs nothing, the footer being rebuilt every frame
/// (`screens/widgets.md` § 2). The two states that reach it are ordinary, not corner cases: a
/// report that could not run is one [`Row::NotComputed`], and one that ran with nothing to say is
/// one [`Row::Prose`] in that report's own words (NOTES § D128) — which is the *body of nothing but
/// `Prose`* this sentence used to call a third case. An empty `Vec` is a fourth and is equally
/// legal; nothing on this screen builds one ([`Report::rows`]). Selecting
/// row 0 regardless would park the highlight on the *could not run* line and advertise a key that
/// opens nothing.
///
/// **What a row may not contain.** [`Finding::evidence`]'s rule holds over every string in every
/// variant here: what is absolute is what k8rs *fetches* — never Secret data, never an
/// environment variable value (CLAUDE.md § Security gate, *Secrets and local files*). The type
/// cannot enforce it; the producers do, and Waste and Posture both read pod specs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// Something the report computed, **and a row the cursor may land on**.
    Answer {
        /// How bad it is, or **nothing at all**. `None` is not a fourth band — it is a
        /// **selectable row that makes no judgement**: `node-1   7.4 of 8 cpu · 11 of 16 GiB`,
        /// `node-1 is ready to drain — 18 pods move`, `34 workloads have no memory or CPU
        /// limit`. A heading is not this; it is a [`Row::Prose`]. The scale is [`Severity`],
        /// shared with `rules.rs` and never re-declared here.
        severity: Option<Severity>,
        /// The row itself, one line before wrapping — and **one line**: a `\n` here is a wrap
        /// `views.rs` did not make, and row-height accounting is the layer above's. Alignment
        /// and column widths are `views.rs`'s too; this file pads nothing.
        text: String,
        /// The indented explanation under it — *"This Service points at nothing. Anything
        /// calling it gets a 503."* **Empty is drawn by leaving the line out**, never by
        /// drawing a blank one: the same convention [`Finding::evidence`] states, so a
        /// renderer needs one rule and not two.
        detail: String,
        /// **What to do** about it, on its own line. `views.rs` prefixes the `→ `
        /// (`screens/alerts.md` § the four parts of a card), so the value here starts at the
        /// word. Empty is drawn by leaving the line out, as `detail` is.
        action: String,
        /// **Where `⏎` goes, and nothing else** (NOTES § D127). The row is selectable because
        /// it is an `Answer`; this field says only whether a destination is recorded.
        ///
        /// `None` is **a selectable row with no destination recorded**, which today is always
        /// a counted row standing for a *set* of objects: Capacity's `34 workloads have no
        /// memory or CPU limit`, Waste's `47 pods` / `12 replicasets`, Certificates' `2
        /// kubelets are waiting to be let in`, and **every row Posture draws**, each standing
        /// for the pods that mount one host path. [`Jump`] has a case for one object and a
        /// case for one finding but none for a set — so **`— ⏎ to list` is drawn on none of
        /// them** (NOTES § D128). The cursor still lands, because the row is an `Answer`; the
        /// suffix returns to every pane in one edit once there is somewhere for it to go.
        /// **The Waste box owes that answer**, not Capacity: its per-object rows are unbounded
        /// — every Service matching no pod, every PVC bound to nothing — so what it needs is a
        /// cap and an overflow row (`and 812 more`), not merely a destination.
        /// Capacity builds its one counted row `jump: None` in the meantime, which is exactly
        /// what this field's `None` means — selectable, destination not recorded — and costs
        /// nothing before `views.rs` exists in Phase 9, so no reader meets a key that does
        /// nothing.
        jump: Option<Jump>,
    },
    /// **A line that is read, never selected.** `screens/analysis.md` § *How a report is drawn*
    /// names three — `Still counted, from what you can see:`, the `Versions` heading at the foot
    /// of the Certificates pane, and Posture's opening paragraph — and its rule 8 adds a fourth,
    /// the sentence a report with nothing to say says in its own words.
    ///
    /// **The control-plane line under `Versions` is not among them and is not added here.**
    /// Which variant carries it is the Versions box's to settle; a shape file guessing at a row
    /// is how the drawings this doc has just stopped citing came to be written.
    ///
    /// It exists because the shape had no way to say it: a heading and a counted row were both
    /// `Answer { severity: None, jump: None }`, so `views.rs` — which arrives in Phase 9, after
    /// this file freezes — would have had to park the cursor where `⏎` does nothing, or key
    /// selection on `jump.is_some()` and skip the row the screen advertises `⏎` on. The sidebar
    /// one pane to the left already answers this the same way (`screens/widgets.md` § 2).
    ///
    /// **No band and no detail.** A line the cursor cannot reach cannot be acted on, so a
    /// severity on it would be a colour with nothing behind it; a row that needs one is an
    /// [`Row::Answer`]. **And no line on this screen carries a band inside it any more**: the
    /// three that did — `9.1 cpu ▲`, `node-2   ● BLOCKS`, `1.31 (1) ▲ too far behind` — were
    /// redrawn rather than given a field, because the shape was right and the drawings were
    /// wrong (NOTES § D128, answering D127's second unexpressible pane).
    Prose(String),
    /// **A check that could not run, in the place its answer would have been** — the state
    /// `screens/analysis.md` § *What each report needs* gives every report on the screen.
    ///
    /// It is a row and not a flag on the report so that a report can switch **one** section
    /// off while its other rows still carry true answers: Capacity's promised/usable table
    /// goes, its limits row keeps counting. And it carries no severity and no jump, so
    /// nothing is drawn where nothing was computed — **no `—` in an absent column**, and no
    /// per-row *unknown* marker, which is the answer this shape refuses.
    ///
    /// **It is also the only record of *did not run***, which is why [`Report::badge`] does not
    /// try to be one. A report that could not be computed at all is this row and nothing else.
    NotComputed {
        /// **Which check is off and why**, in one plain-language sentence — *"Not checked
        /// here. Adding up what a node has promised needs every pod on it, and you can only
        /// see payments — so every number would come out too low."* It never says `403`,
        /// `RBAC` or *namespace-scoped snapshot*.
        reason: String,
        /// **What to ask for** to get the answer back — *"Ask for cluster-wide read access,
        /// or drop the `--namespace` flag if you set one."* A required half, not an
        /// afterthought: the three causes (missing capability, missing permission, missing
        /// scope) share one sentence shape, and a report that names the check without naming
        /// the way out is the half a reader cannot act on.
        ask_for: String,
    },
}

/// Where a row's `⏎` goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Jump {
    /// **To a finding** — a rule already answered this, and the report is restating it. The
    /// finding is carried whole rather than by identity: there is no registry to resolve one
    /// against, and the detail view draws the finding itself.
    ///
    /// **Boxed**, because a `Finding` is 320 bytes and almost no row has one: unboxed it made
    /// every [`Row`] 393 bytes wide against `NotComputed`'s 48, which `clippy` refuses at
    /// `-D warnings` and which a report of forty rows pays for in full.
    Finding(Box<Finding>),
    /// **To an object no finding names**, which is the whole reason some rows exist: the
    /// container that keeps dying between its restarts, the PVC bound to nothing, the
    /// ReplicaSet parked at 0. Nothing is broken, so no rule fired, and this row is the only
    /// thing that sends the reader there.
    ///
    /// **A jump is navigation, and this [`ObjectId`] never reaches an operation.** A report row
    /// is a snapshot of a moment; `uid` exists so a confirmation cannot act on the object that
    /// replaced the one the user selected (NOTES § D22), and a stale row handing its id to a
    /// dialog either refuses a legitimate action or — where the `uid` is `None` — skips that
    /// check entirely. `views.rs` re-resolves the object from the store on arrival.
    ///
    /// **Resolving an [`crate::rules::ObjectKind::Other`] kind to an API resource is
    /// `views.rs`'s**, by discovery (invariant 12). `Other("Rollout.argoproj.io")` is not how
    /// `kubectl` spells a resource (NOTES § D36), and no row here may be written as though it
    /// were.
    Object(ObjectId),
}

// --- THE REPORT SHAPE END ---

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
