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

use crate::rules::{
    CertificateRequestSnapshot, ClaimSnapshot, ClusterSnapshot, ContainerSnapshot, ContainerState,
    DisruptionBudgetSnapshot, EndpointSliceSnapshot, Finding, HostPathMount, Metrics,
    NODE_NAMESPACE, NodeSnapshot, ObjectId, ObjectKind, PodSnapshot, RESTARTS_WARN, Selector,
    SelectorRequirement, ServiceSnapshot, Severity, a_drain_would_move, age, bytes, container_fact,
    cpu_text, doing_its_job, expires_at, finished, is_runtime_socket, kubelet_too_far_behind,
    listed, minor_version, mounted_path, node_overcommitted, pods_on, promised, qualified,
    quantity_milli,
};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

use std::collections::{BTreeMap, BTreeSet, HashSet};

// --- THE REPORT SHAPE START ---

/// One report — **one pane's worth of content**, and not one pane and one sidebar entry. Which
/// panes exist, and which of them share one, is `screens/`'s, and its opening line counts the
/// reports, the sidebar entries and the panes separately: `Versions` has its own sidebar entry but
/// is drawn at the foot of the Certificates pane (`screens/analysis.md`, head and § *Certificates
/// and Versions*). So the count of `Report`s and the count of panes are two facts, and this type
/// states neither.
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
    /// `node-2  ● BLOCKS`), one that ran and found nothing, and one that could not run. Only two
    /// ANALYSIS entries are drawn with a value anywhere in `screens/` — `capacity  1 ▲` and
    /// `certificates  30d`; every other one is blank in every sidebar drawn.
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
        /// renderer needs one rule and not two, and an empty `Vec` is that empty.
        ///
        /// **One element per paragraph, and that is why it is a `Vec` and not a `String`**
        /// (NOTES § D129). Capacity's flagged node draws *two* indented paragraphs — the
        /// measurement `using 3.4 cpu and 12 GiB`, then the sentence that says what the
        /// numbers mean — a healthy node draws one, and a node on a cluster with no
        /// metrics-server draws none. A `String` could hold both only with a `\n` in it, which
        /// is the wrap [`Row::Answer::text`] forbids for the reason that applies here too:
        /// this layer cannot see the pane's width, so it cannot be the layer that breaks a
        /// line. **Each element obeys `text`'s rule on its own** — one line before wrapping,
        /// no `\n`, no glyph — and `views.rs` wraps each and leaves a blank line between them.
        ///
        /// **Not folded into the row's `text`, and not folded into the explanation.** D128 put
        /// the measurement in `detail` because a value absent on most clusters may not ride
        /// the always-present line; joining the two paragraphs into one sentence would put it
        /// back on a line the reader reads as a single fact.
        detail: Vec<String>,
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

// --- THE CAPACITY REPORT START ---

/// **What each node has promised the pods on it, against what it has** — plus the workloads
/// nothing stops from taking a whole node, which is the old rule 9 and lives here because it is a
/// risk and not an outage (`screens/analysis.md` § Capacity, NOTES § D2).
///
/// **`findings` is unread, and that is the shape answering rather than a field forgotten.** N5 is
/// `Severity::Info` and [`crate::rules::analyze`] deliberately does not return it, so there is no
/// card on the slice to pick up; the verdict comes from [`crate::rules::node_overcommitted`],
/// which *is* N5, called here for the one reason the module doc gives — the report and the rule
/// may not answer differently about one node (NOTES § D46).
///
/// **Three sections, in the order the pane draws them**: the node rows, then the one rendering of
/// a missing live-usage number, then the limits row. **The limits row is counted from pods**, so
/// it survives both states in which the node section does not — a namespace scope, and a login
/// that may not list nodes at all.
pub fn capacity(snapshot: &ClusterSnapshot, _findings: &[Finding]) -> Report {
    let title = "What each node promised, and what it has".to_string();
    let uncapped = uncapped_workloads(snapshot);

    // **The two states that switch the node section off, and the scope wins when both hold** —
    // one `NotComputed` per section, and the wider fact is the one drawn (`screens/analysis.md`
    // § *How a report is drawn*, rule 7). A scope also narrows the pod list the limits row counts,
    // which is why the line above it says *from what you can see*.
    if let Some(row) = node_section_off(snapshot) {
        let mut rows = vec![row];
        if uncapped > 0 {
            rows.push(Row::Prose(
                "Still counted, from what you can see:".to_string(),
            ));
            rows.push(limits_row(uncapped));
        }
        return Report {
            title,
            badge: None,
            rows,
        };
    }

    let mut lines: Vec<NodeLine> = snapshot
        .nodes
        .iter()
        .map(|n| node_row(snapshot, n))
        .collect();
    // **Flagged nodes first, then node name** (`screens/analysis.md` § Capacity, *Many nodes*).
    // On a two-hundred node cluster the alternative puts the one answer this report exists to
    // give below the fold, and the badge that says *there is one in here* gives no way to find it.
    lines.sort_by(|a, b| b.over.cmp(&a.over).then_with(|| a.name.cmp(b.name)));

    let flagged = lines.iter().filter(|line| line.over).count();
    let unreadable = lines.iter().filter(|line| !line.readable).count();
    let mut rows: Vec<Row> = lines.into_iter().map(|line| line.row).collect();
    // **Nothing names metrics-server on a cluster that answered** — a dependency that is working
    // is not news, so this row is absent exactly when the `using …` paragraphs are present.
    rows.extend(live_usage_row(snapshot.metrics.as_ref()));
    if uncapped > 0 {
        rows.push(limits_row(uncapped));
    } else if flagged == 0 && unreadable == 0 {
        // The report ran and has nothing to say, in its own words — the one `Row::Prose` rule 8
        // asks for, so `views.rs` carries no per-report empty text (NOTES § D128).
        rows.push(Row::Prose(
            "Every node has room to spare, and every workload here has a memory and CPU limit \
             set. Nothing to do."
                .to_string(),
        ));
    }

    Report {
        title,
        // **The badge counts flagged nodes and nothing else** — never a percentage, never an
        // average (`screens/widgets.md` § 1a, PRIOR-ART § F2). Nothing to count is no badge:
        // `Some("0")` and `None` are two facts and the body carries the other one.
        badge: (flagged > 0).then(|| Badge {
            value: flagged.to_string(),
            severity: Severity::Warn,
        }),
        rows,
    }
}

/// The one row that stands in for the whole node section, or `None` when it runs.
///
/// **An empty node list is *no permission to list nodes at all***, and it can be, because
/// [`crate::rules::ClusterSnapshot::nodes`] is a `Vec` and a cluster always has nodes
/// (`screens/analysis.md` § *Capacity's remaining states*). It is a different shape from a
/// namespace scope — this login may read pods everywhere and nodes nowhere.
fn node_section_off(snapshot: &ClusterSnapshot) -> Option<Row> {
    if let Some(namespace) = snapshot.namespace_scope.as_deref() {
        return Some(Row::NotComputed {
            reason: format!(
                "Not checked here. Adding up what a node has promised needs every pod on it, and \
                 you can only see {namespace} — so every number would come out too low."
            ),
            // **Both causes in one sentence, because the screen cannot tell them apart and does
            // not need to**: a scope arrives from `--namespace` or from the 403 fallback as one
            // field (NOTES § D46).
            ask_for: "Ask for cluster-wide read access, or drop the --namespace flag if you set \
                      one."
                .to_string(),
        });
    }
    snapshot.nodes.is_empty().then(|| Row::NotComputed {
        reason: "Not checked. Reading what a node has needs permission to list nodes, and this \
                 login does not have it."
            .to_string(),
        ask_for: "Ask for permission to list nodes across the whole cluster.".to_string(),
    })
}

/// One node's line: the row, and the two facts about it the pane needs once the row is a string —
/// whether it is over its allocatable (the sort order, and the badge) and whether its numbers
/// could be read at all. Neither is recoverable from the row afterwards, and a tuple of two bools
/// is the thing nobody can read at 3am.
struct NodeLine<'a> {
    over: bool,
    readable: bool,
    name: &'a str,
    row: Row,
}

/// One node's row, and the two facts beside it.
///
/// **Both dimensions on every row, always** (`screens/analysis.md` § Capacity): CPU
/// overcommitment stops the next pod that asks for CPU from fitting, memory overcommitment gets a
/// running one killed, and a report that names one and not the other teaches the wrong lesson
/// about which number to watch. **One band for both**, because this whole screen is *risky later*
/// and the kill itself is Alerts' rule 2 (NOTES § D2); the sentence under the row is where they
/// differ.
fn node_row<'a>(snapshot: &ClusterSnapshot, node: &'a NodeSnapshot) -> NodeLine<'a> {
    let pods = pods_on(snapshot, node);
    let cpu = promised(
        &pods,
        node.allocatable_cpu.as_deref(),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
        |p| p.overhead_cpu.as_deref(),
    );
    let memory = promised(
        &pods,
        node.allocatable_memory.as_deref(),
        |p| p.memory_request.as_deref(),
        |c| c.memory_request.as_deref(),
        |p| p.overhead_memory.as_deref(),
    );
    let name = node.id.name.as_str();
    // **A jump is navigation and never reaches an operation** ([`Jump::Object`]): `views.rs`
    // re-resolves the node from the store on arrival.
    let jump = Some(Jump::Object(node.id.clone()));

    // **A node whose numbers cannot be read keeps its row.** [`promised`] answers `None` when the
    // node does not say what it has, or when one quantity in the sum does not parse — and a node
    // dropped from the pane instead is one machine silently absent from the report, which is the
    // defect NOTES § D81 paid for once already.
    //
    // **It answers per dimension, and so does this row.** A node whose CPU sum has one
    // unparseable quantity in it may have a memory sum that came out perfectly and is over the
    // line — and [`node_overcommitted`] says so about that node, so a row that drew *could not be
    // worked out* over the whole machine was the report and the rule disagreeing, which is the
    // divergence NOTES § D46 is about
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § nit 11). Both dimensions
    // are still on every row (`screens/analysis.md` § Capacity): each one says either its numbers
    // or that it could not be read.
    let text = match (cpu, memory) {
        // Neither side of the machine could be read, so there is nothing to name a dimension
        // about — the screen's own row (`screens/analysis.md` § *Capacity's remaining states*).
        (None, None) => format!("{name}   could not be worked out"),
        (cpu, memory) => format!(
            "{name}   {} · {}",
            match cpu {
                Some((asked, has)) => format!("{} of {} cpu", cpu_text(asked), cpu_text(has)),
                None => "cpu could not be worked out".to_string(),
            },
            match memory {
                Some((asked, has)) => format!("{} of {}", bytes(asked), bytes(has)),
                None => "memory could not be worked out".to_string(),
            }
        ),
    };

    let over = node_overcommitted(snapshot, node);
    // **The measurement first, then what the numbers mean** (NOTES § D128, § D129): the reader
    // meets the number before the consequence it is about, and a node the metrics API did not
    // report on draws no measurement while every other node keeps its own.
    let mut detail: Vec<String> = using(snapshot.metrics.as_ref(), name).into_iter().collect();
    // **What the consequence is, per dimension over the line.** The comparison is N5's own, on
    // N5's own numbers, and the test beside this asserts the two agree on every node of the
    // corpus — the report may not flag a node the rule calls fine, or the other way round.
    if cpu.is_some_and(|(asked, has)| asked > has) {
        detail.push(
            "A pod that asks for CPU will not fit here until something moves off.".to_string(),
        );
    }
    if memory.is_some_and(|(asked, has)| asked > has) {
        detail.push("If these pods use what they asked for, one of them is killed.".to_string());
    }
    // **Last, because it is about what is missing rather than about what was found** — and drawn
    // whenever either side is missing, which is the same sentence for one dimension as for two:
    // what the reader does about it does not depend on which number it was.
    if cpu.is_none() || memory.is_none() {
        detail.push(
            "One of the numbers here — what this node has to give, or what a pod on it asked \
             for — is written in a way k8rs could not read."
                .to_string(),
        );
    }

    NodeLine {
        over: over.is_some(),
        readable: cpu.is_some() && memory.is_some(),
        name,
        row: Row::Answer {
            severity: over.is_some().then_some(Severity::Warn),
            // **One string, and nothing here is a column** (`screens/analysis.md` rule 3): this
            // file pads nothing and `views.rs` never splits a rendered string back into values.
            text,
            detail,
            // N5's own sentence, not a second one written here: a row and the rule behind it
            // telling a reader to do two different things is the divergence D46 is about.
            action: over.map(|f| f.action).unwrap_or_default(),
            jump,
        },
    }
}

/// **The one slot on this page that says why there is no live-usage number**, and `None` when
/// there is one — `screens/analysis.md` § *Live usage*, whose whole point is that a missing
/// metrics-server is said **once**, under the node rows, and never as a per-row `—`.
///
/// **Five states reach this slot and four of them draw a row.** [`crate::rules::Metrics::Read`]
/// draws nothing, because a dependency that is working is not news. Three of the four are the
/// cluster's and their wording is `screens/analysis.md`'s, verbatim. The fourth is the `Option`
/// around [`crate::rules::Metrics`] — *k8rs did not ask* — which the screen does not draw yet
/// because until Phase 5's poll lands it is the only one that happens; it is the one sentence
/// here that names k8rs rather than the cluster, because it is the only one where the cluster is
/// not the reason.
///
/// A missing capability, a missing permission and a missing dependency are three causes with one
/// sentence shape and one slot: a feature that silently disappears teaches a beginner the tool is
/// unreliable, and four different ways of saying it is missing teaches them it is arbitrary.
fn live_usage_row(metrics: Option<&Metrics>) -> Option<Row> {
    let (reason, ask_for) = match metrics {
        // Answered — nothing is drawn here at all, and the `using …` paragraphs carry it.
        Some(Metrics::Read(_)) => return None,
        // Nobody probed, which is every cluster until Phase 5 polls.
        None => (
            "What each node is actually using is not shown. That number comes from \
             metrics-server, and k8rs does not read it.",
            "Nothing to ask for — the numbers above are complete without it.",
        ),
        Some(Metrics::NotInstalled) => (
            "What each node is actually using is not shown. That number comes from \
             metrics-server, and this cluster does not have it installed.",
            "Install metrics-server if you want it — the numbers above are complete without it.",
        ),
        Some(Metrics::Silent) => (
            "metrics-server is installed here but did not answer.",
            "Check that its pods are running.",
        ),
        Some(Metrics::Denied) => (
            "You are not allowed to read what each node is using.",
            "Ask for permission to list nodes in the metrics.k8s.io API group.",
        ),
    };
    Some(Row::NotComputed {
        reason: reason.to_string(),
        ask_for: ask_for.to_string(),
    })
}

/// **One node's `using …` paragraph**, or nothing at all — the measurement that sits directly
/// under its row (`screens/analysis.md` § Capacity).
///
/// **`None` covers three different clusters and the row draws the same nothing for all three**:
/// nobody probed, the probe failed, and the probe answered without this node in it — the last
/// being a node that joined between polls. The *reason* is [`live_usage_row`]'s and is said once
/// for the pane, never per row: nothing is drawn where nothing was computed.
///
/// Parsed with [`quantity_milli`] and printed with [`cpu_text`] / [`bytes`], which is what the row
/// above it uses — the API's own string here would put two spellings of one number on adjacent
/// lines. An unparseable quantity draws no paragraph, the same direction the row itself takes.
fn using(metrics: Option<&Metrics>, node: &str) -> Option<String> {
    let Metrics::Read(nodes) = metrics? else {
        return None;
    };
    let usage = nodes.get(node)?;
    Some(format!(
        "using {} cpu and {}",
        cpu_text(quantity_milli(&usage.cpu)?),
        bytes(quantity_milli(&usage.memory)?)
    ))
}

/// **The old rule 9, as one counted row** — a cluster has hundreds of these and none of them is
/// broken, so it is a row here and not an alarm (`screens/analysis.md` § Capacity).
///
/// **`jump: None` is a selectable row with no destination recorded** ([`Row::Answer::jump`]): it
/// stands for a *set* of objects and [`Jump`] has no case for one. The `— ⏎ to list` suffix is
/// drawn on none of them until it does (NOTES § D128).
fn limits_row(count: usize) -> Row {
    let (noun, verb) = if count == 1 {
        ("workload", "has")
    } else {
        ("workloads", "have")
    };
    Row::Answer {
        severity: None,
        text: format!("{count} {noun} {verb} no memory or CPU limit"),
        detail: vec!["Nothing stops one taking a whole node.".to_string()],
        action: String::new(),
        jump: None,
    }
}

/// **How many *workloads* are missing a CPU limit or a memory limit** — counted over everything
/// still running, because a pod that finished is charged to nobody and takes no node
/// ([`finished`]), and **counted as controllers and not as pods**.
///
/// **The number and the noun have to agree, and they did not.** This counted pods while the row
/// said *workloads*: on the four-node fixture cluster that is `41 workloads` about ten of them,
/// and on a cluster with 50-replica Deployments it is the replica count over and over
/// (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 5). The fix is the noun's,
/// not the denominator's — *"which of my workloads has no limit set"* is the question a reader
/// acts on, and it is answered once per Deployment rather than once per copy of it
/// (PRIOR-ART § F2).
///
/// **The key is [`crate::rules::ObjectId::group_key`] on the pod's `owner`** — the identity D3
/// already groups a card by, so a Deployment deleted and recreated under a new uid is one
/// workload here too. A pod with no controller is its own owner
/// ([`crate::rules::PodSnapshot::owner`]) and counts as one workload, which is right: a pod
/// somebody started by hand is a workload nothing else stands for. **By Phase 5 the ReplicaSet
/// named here resolves up to its Deployment** and the count becomes exactly the Deployments;
/// until then two ReplicaSets of one Deployment count twice, which is the same shape every card
/// on Alerts has today.
///
/// **A workload counts when either dimension is missing**, which is what the row says: *no memory
/// **or** CPU limit*. Which declarations are read, and what an unreported pod answers, are
/// [`capped`]'s.
fn uncapped_workloads(snapshot: &ClusterSnapshot) -> usize {
    snapshot
        .pods
        .iter()
        .filter(|pod| {
            !finished(pod)
                && !(capped(pod, |p| p.cpu_limit.as_deref(), |c| c.cpu_limit.as_deref())
                    && capped(
                        pod,
                        |p| p.memory_limit.as_deref(),
                        |c| c.memory_limit.as_deref(),
                    ))
        })
        .map(|pod| pod.owner.group_key())
        .collect::<HashSet<_>>()
        .len()
}

/// Whether this pod is capped in one dimension: a pod-level limit, or one on **every** container
/// it declares — init containers included, because an init container with no limit can take the
/// whole machine for as long as it runs.
///
/// **The pod-level half is not redundant, even though the kubelet usually makes it look so.** On a
/// running pod the kubelet writes the pod-level limit down into
/// `status.containerStatuses[].resources`, which `effective` then reports as the container's own —
/// so `broken-podlimit` answers *capped* through its containers alone. Where it does not is the
/// gap `rules.rs` names for virtual-kubelet, serverless nodes and sandboxed runtimes: a status
/// carrying no `resources` whose name matches no spec entry decodes with nothing at all, and there
/// the pod-level limit is the only thing capping the pod
/// ([`crate::rules::PodSnapshot::cpu_limit`], NOTES § D51).
///
/// **A pod with no containers answers *capped*, and that is the wanted answer.**
/// [`crate::rules::PodSnapshot::containers`] is built from `status.containerStatuses`, so a pod
/// the kubelet has not reported on — every Pending one — decodes with an empty list rather than
/// with all-`None` containers, and the snapshot says nothing about what it declared. `all` over
/// nothing is `true`, so it is left out of the count rather than guessed at: every workload the
/// row counts is one that provably has no limit (PRIOR-ART § F2).
fn capped(
    pod: &PodSnapshot,
    of_pod: impl Fn(&PodSnapshot) -> Option<&str>,
    of_container: impl Fn(&ContainerSnapshot) -> Option<&str>,
) -> bool {
    of_pod(pod).is_some() || pod.containers.iter().all(|c| of_container(c).is_some())
}

// --- THE CAPACITY REPORT END ---

// --- THE DRAIN SAFETY REPORT START ---

/// **What a drain of each node would do, and what would stop it** — the report that pays for
/// itself, because a drain that never finishes is normally discovered forty minutes in
/// (`screens/analysis.md` § Drain safety).
///
/// **`findings` carries N1**, and that is the one thing on this pane no snapshot field can
/// answer on its own: a node whose kubelet is not saying `Ready` is a node a drain cordons and
/// then either waits on forever or cannot be judged about at all, and *which* nodes those are is
/// [`crate::rules::analyze`]'s answer, not a second reading of `conditions[Ready]` here
/// ([`not_ready`], NOTES § D46, § D131, § D134). N2 is still unread — a *cordon* left half-done is
/// a different sentence built on the same join — and [`a_drain_would_move`] is the piece this
/// report and N2 share, called rather than re-derived.
///
/// **One row per node, seven kinds of row, drawn in band order**: the node a drain would never
/// finish on, the node whose drain finishes and throws away files, the node carrying pods nothing
/// would restart, the node whose drain needs one more flag and loses nothing, and the three that
/// carry no band — the node that cannot be checked until it is ready again, the node waiting on a
/// budget's counters, and the node that is ready. The last three are still [`Row::Answer`]s:
/// there is nothing to judge and `⏎` still opens the node.
///
/// **A node can be more than one of those, and the row still shows one text**
/// (`screens/analysis.md` § Drain safety): the highest band supplies the row's single line, and
/// every other true reason about that node is a further paragraph in `detail`, in the same order.
///
/// **Under one namespace the whole pane is one [`Row::NotComputed`]**, which this report says
/// more loudly than the others: *"18 pods move, node-1 is ok"* is a green light for an operation
/// that then hangs on a pod the report could not see.
pub fn drain_safety(snapshot: &ClusterSnapshot, findings: &[Finding]) -> Report {
    let title = "If you drained each node, what happens?".to_string();
    if let Some(row) = drain_not_computed(snapshot) {
        return Report {
            title,
            badge: None,
            rows: vec![row],
        };
    }
    // Checked by `drain_not_computed` one line up; `unwrap_or_default` rather than an `expect`
    // because a panic in a pure function is the thing invariant 5 exists to prevent.
    let budgets = snapshot.disruption_budgets.as_deref().unwrap_or_default();

    let mut lines: Vec<DrainLine> = snapshot
        .nodes
        .iter()
        .map(|node| drain_row(snapshot, budgets, findings, node))
        .collect();
    // **Worst band first, then node name** — Capacity's order (`screens/analysis.md` § Capacity,
    // *Many nodes*) for its reason: on a two-hundred node cluster the alternative puts the one
    // answer this report exists to give below the fold.
    lines.sort_by(|a, b| b.band.cmp(&a.band).then_with(|| a.name.cmp(b.name)));

    // **Not `band == 0`**: the node waiting on a budget's counters shares that band with the
    // ready ones and is not one of them, and *"every node could be drained right now"* is false
    // about a node nothing could check ([`DrainLine::ready`]).
    let all_clear = lines.iter().all(|line| line.ready);
    // **The one flag this pane assumes, named once, above every row that assumes it**
    // (`screens/analysis.md` § *The DaemonSet flag, said once*). A bare `kubectl drain` refuses on
    // DaemonSet-managed pods on every cluster that runs a CNI — which is every cluster — so a
    // pane answering *what happens if you drain this* with the bare command answers *nothing
    // happens* about all four nodes. `--ignore-daemonsets` deletes nothing, so it is safe to
    // assume **provided the pane says it is assuming it**; `--delete-emptydir-data` deletes the
    // reader's files, so that one is a row and never an assumption.
    //
    // **Not the command log**: the strip only ever shows a command k8rs actually ran
    // (invariant 4), and this pane never calls `kubectl drain` at all. **Not a per-node note**:
    // the fact is true of nearly every node on nearly every cluster, and repeating it per row
    // would make it the loudest line on the busiest pane in the product.
    let mut rows: Vec<Row> = vec![Row::Prose(
        "A drain below assumes --ignore-daemonsets, so DaemonSet pods never count as moving."
            .to_string(),
    )];
    rows.extend(lines.into_iter().map(|line| line.row));
    if all_clear {
        // The report ran and has nothing to say, in its own words — rule 8, and the sentence is
        // `screens/analysis.md` § Drain safety's own.
        // **Three clauses, one per class this sentence rests on.** It named two of the three and
        // a node that would throw away files read as *all clear* (NOTES § D134). The third
        // covers both emptyDir mediums in one clause on purpose: either kind is enough to give
        // that node a row of its own and take this sentence away, so *keeps its own files* is
        // true either way without naming a medium.
        rows.push(Row::Prose(
            "Every node could be drained right now. Nothing on this cluster is protected by a \
             rule a drain would wait on, nothing on it was started by hand, and nothing on it \
             keeps its own files, on disk or in memory."
                .to_string(),
        ));
    }
    Report {
        title,
        // **This report never badges**, which is [`Report::badge`]'s own example: the sidebar
        // has room for a number and not for a reason, and the reason is the whole row here.
        badge: None,
        rows,
    }
}

/// The one row that is the whole pane, or `None` when the report runs.
///
/// **Three causes, three ways out, and the widest one wins** (`screens/analysis.md` rule 7). A
/// namespace scope is wider than a missing node list, which is wider than a missing budget list:
/// each of the later two is still true under the first, and stacking two reasons over one empty
/// pane is two ways out for a reader who can only take one.
fn drain_not_computed(snapshot: &ClusterSnapshot) -> Option<Row> {
    if let Some(namespace) = snapshot.namespace_scope.as_deref() {
        return Some(Row::NotComputed {
            reason: format!(
                "Not checked here. Working out whether a drain finishes needs every pod on every \
                 node, and the rules that say how many copies must stay up — you can only see \
                 {namespace}, and a half-answer here would call a node safe that is not."
            ),
            ask_for: "Ask for cluster-wide read access, or drop the --namespace flag if you set \
                      one."
                .to_string(),
        });
    }
    if snapshot.nodes.is_empty() {
        return Some(Row::NotComputed {
            reason: "Not checked. This report answers one question per node, and this login \
                     cannot list the nodes."
                .to_string(),
            ask_for: "Ask for permission to list nodes across the whole cluster.".to_string(),
        });
    }
    snapshot
        .disruption_budgets
        .is_none()
        .then(|| Row::NotComputed {
            reason: "Not checked. Working out whether a drain finishes needs the rules that say \
                     how many copies of a workload must stay up, and k8rs could not read them — \
                     without them every node would look safe."
                .to_string(),
            ask_for: "Ask for permission to list poddisruptionbudgets across the whole cluster."
                .to_string(),
        })
}

/// One node's line: the row, and the two facts the pane needs once the row is a string. `band` is
/// the sort key and **not** a second spelling of the severity — it is an order, and
/// [`Severity`]'s own `Ord` runs the other way (`Critical < Warn < Info`), so keying the sort on
/// it directly would draw the ready nodes first.
struct DrainLine<'a> {
    band: u8,
    /// **Whether this node is *ready to drain*, which is not the same as `band == 0`.** Two rows
    /// share band 0 with the ready ones and are not ready: the node whose budget numbers have not
    /// caught up, and the node whose kubelet answered and said no. Neither has any urgency to
    /// signal, so neither has a reason to outrank the ready nodes (`screens/analysis.md` § *A
    /// budget that has not caught up yet*, § *`Ready: False`, reversed*) — and the empty state's
    /// sentence claims every node could be drained *right now*, which is exactly what nobody
    /// knows about either of them.
    ready: bool,
    name: &'a str,
    row: Row,
}

/// **One node's verdict.** Seven kinds of row, and the order they are asked in is *not* quite the
/// order they are banded in. Band order runs: a drain that cannot finish is worse news than one
/// that finishes and throws away files, which is worse news than one that deletes a pod nothing
/// recreates, which is worse news than one that only needs a flag, which is worse news than one
/// that is fine. **The one row asked out of that order is the node whose kubelet said `Ready:
/// False`** — it is asked straight after the genuine budget block and before every verdict below,
/// because what it says is that there *is* no verdict, and a verdict drawn under it would be one
/// k8rs cannot stand behind (NOTES § D134).
///
/// **[`a_drain_would_move`] is the whole narrowing** — a mirror pod, a DaemonSet pod and a pod
/// already terminating are not moved by a drain, so a node running only those is *ready to
/// drain* and not busy, and a budget protecting only those does not block it. `kubectl drain
/// --dry-run=client`'s own output on two of the corpus's nodes is the ground truth for three of
/// the seven (`reports/2026-08-21-family-c-corpus-drain-and-capacity.md` § 3,
/// `reports/2026-08-21-family-c-analysis-report-family-review.md` § 1).
///
/// **`--ignore-daemonsets` is assumed and said once, as the pane's opening line** — the same
/// narrowing, named. It deletes nothing, so assuming it costs a reader nothing;
/// `--delete-emptydir-data` deletes their files, so it is a row and never an assumption
/// (`screens/analysis.md` § *The DaemonSet flag, said once*).
fn drain_row<'a>(
    snapshot: &ClusterSnapshot,
    budgets: &[DisruptionBudgetSnapshot],
    findings: &[Finding],
    node: &'a NodeSnapshot,
) -> DrainLine<'a> {
    let name = node.id.name.as_str();
    // **A jump is navigation and never reaches an operation** ([`Jump::Object`]), which on this
    // pane is worth saying twice: the row beside it is about `kubectl drain`.
    let jump = Some(Jump::Object(node.id.clone()));
    let moving: Vec<&PodSnapshot> = pods_on(snapshot, node)
        .into_iter()
        .filter(|pod| a_drain_would_move(pod))
        .collect();
    // **Nothing would restart these** — no controlling owner at all, which is what `owner`
    // being the pod itself means ([`crate::rules::PodSnapshot::owner`]) and what `kubectl
    // drain` itself refuses to delete without `--force`. Mirror pods reach the same shape and
    // are already gone: a drain does not move one.
    let orphans = moving.iter().filter(|pod| pod.owner == pod.id).count();
    // **And these keep files on the machine itself** — `kubectl drain`'s own `localStorageFilter`
    // ([`crate::rules::PodSnapshot::local_storage_disk`]). **Counted over the same `moving` list
    // and deliberately not deduplicated against the orphans**: a pod can be both, and the two
    // counts are two different facts a reader needs rather than one fact counted twice
    // (`screens/analysis.md` § *A node that would throw away files*).
    let local = moving.iter().filter(|pod| pod.local_storage_disk).count();
    // **And these keep them in memory only**, which is the same refusal and not the same loss:
    // upstream's filter never reads `medium`, so a bare drain stops on a tmpfs exactly as it
    // stops on a disk-backed volume — and there is nothing to copy off it
    // ([`crate::rules::PodSnapshot::local_storage_memory`], NOTES § D134). A third independent
    // tally over the same list, deduplicated against neither of the other two.
    let memory = moving.iter().filter(|pod| pod.local_storage_memory).count();

    // **Sorted before anything is read off them, on `(namespace, name)`** — the order the
    // reader's own `kubectl get pdb -A` prints, which the joined `namespace/name` this used to
    // sort is not: `'-'` (0x2D) sorts before `'/'` (0x2F), so `team-a/api` came out before
    // `team/web` while kubectl prints `team web` first
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 7). Every other list on
    // this screen keys the tuple; this one is now the same answer.
    let mut relevant: Vec<&DisruptionBudgetSnapshot> = budgets
        .iter()
        .filter(|budget| protects_anything_moving(budget, &moving))
        .collect();
    relevant.sort_by(|a, b| (&a.id.namespace, &a.id.name).cmp(&(&b.id.namespace, &b.id.name)));
    // **The generation question is asked here and never inside [`blocks_a_drain`]**, which is
    // what keeps it a single reading: while a budget's spec is ahead of its status the three
    // counters beside it are not a measurement of anything, so the sentence that would be built
    // from them may not be built at all (NOTES § D130).
    //
    // **Unless the controller has already said why it stopped, and then that answer wins**
    // ([`could_not_be_counted`]). `failSafe` writes `SyncFailed` and deliberately does *not*
    // advance `status.observedGeneration`, so a budget whose first sync failed sits behind its
    // spec forever — the permanent case arrives wearing the transient one's shape, and asked in
    // the other order it drew this pane's quietest row over a drain that hangs until a human
    // fixes the budget (NOTES § D139). Only the
    // `SyncFailed` branch is reachable this way and it reads no counter, so *the counters are
    // never read while the spec is ahead* still holds.
    let mut stale: Vec<NotCaughtUp> = Vec::new();
    let mut blocked: Vec<Blocked> = Vec::new();
    for budget in relevant {
        match has_not_caught_up(budget) {
            Some(waiting) if !could_not_be_counted(budget) => stale.push(waiting),
            _ => blocked.extend(blocks_a_drain(budget)),
        }
    }

    // **N1's own card, not a second reading of the node's `Ready` condition** — so this row and
    // the Alerts card about the same machine cannot come to disagree about which nodes are not
    // ready, or about the five minutes before one is said to be ([`not_ready`]).
    let unready = not_ready(findings, node);
    let silent = match unready {
        Some(NotReady::Silent(card)) => Some(card),
        Some(NotReady::SaidNo) | None => None,
    };

    if silent.is_some() || !blocked.is_empty() {
        // **The node's own paragraph first when it has one**: nothing on a machine that is not
        // answering can be trusted, its budgets' counters included
        // (`screens/analysis.md` § *A node that has stopped responding*).
        let mut detail: Vec<String> = silent.iter().map(|_| NODE_SILENT.to_string()).collect();
        detail.extend(blocked.first().map(|first| first.explanation.clone()));
        // **Every other blocking budget is named, never counted.** A count sends a reader who
        // cleared the named budget straight into a second one whose name appeared nowhere on the
        // pane (NOTES § D134). Capped by [`listed`] — `rules.rs`'s own *up to two, then and N
        // more*, which N1's evidence line already spells — so this line never grows past three
        // clauses whatever the cluster's budget count is. Identity only: a reader who clears the
        // first one and looks again meets the next name with its own paragraph.
        let others: Vec<String> = blocked.iter().skip(1).map(|b| b.budget.clone()).collect();
        if !others.is_empty() {
            detail.push(format!(
                "{} block{} the drain too.",
                listed(&others),
                if others.len() == 1 { "s" } else { "" }
            ));
        }
        // **The second problem is not dropped because the first one is louder.** A node can
        // block *and* throw away files *and* carry pods nothing would restart — a reader who
        // clears the block would meet each of the others with no warning, which is the silent
        // miss this project refuses.
        detail.extend(the_other_problems(local, orphans, memory, &stale));
        return DrainLine {
            band: 4,
            ready: false,
            name,
            row: Row::Answer {
                severity: Some(Severity::Critical),
                text: format!("{name} would never finish draining"),
                // One line, so it belongs to the paragraph that leads: a machine that is not
                // answering is checked before any budget on it means anything, and otherwise it
                // is the first problem's — the one that stops the drain before anything else can.
                action: match silent {
                    Some(n1) => n1.action.clone(),
                    None => blocked
                        .first()
                        .map(|first| first.action.clone())
                        .unwrap_or_default(),
                },
                detail,
                jump,
            },
        };
    }

    // **A genuine budget block is asked first and has already returned above.** A budget refuses
    // at the API server, before the kubelet is ever asked to confirm anything, so *would never
    // finish draining* stays true about a node whether or not its kubelet is answering; only a
    // node with no such block reaches here (NOTES § D134). Everything below this point is a
    // verdict about the drain, and a verdict is exactly what a `Ready: False` node has none of —
    // so this row wins the text over the local-storage and orphan rows, and the facts they would
    // have drawn are folded under it.
    if matches!(unready, Some(NotReady::SaidNo)) {
        return DrainLine {
            // **Band 0, and `ready` is false** — the same pair the stale-budget row draws, for the
            // same reason: nothing here is urgent by k8rs's own account, because k8rs's own
            // account is what is missing; and *every node could be drained right now* is false
            // about a node nobody knows about ([`DrainLine::ready`]).
            band: 0,
            ready: false,
            name,
            row: Row::Answer {
                severity: None,
                text: format!("{name} can't be checked until it is ready again"),
                detail: std::iter::once(CANNOT_TELL.to_string())
                    .chain(the_other_problems(local, orphans, memory, &stale))
                    .collect(),
                // **Not N1's own action, unlike the `Unknown` row.** N1's `False` action ends
                // *"what the kubelet says is wrong is above"*, and there is no *above* here: this
                // pane never repeats the kubelet's message, the Alerts card does. Pointing at
                // that card is what keeps the two screens from carrying two diagnoses of one node.
                action: "check the node's Alerts card for what is wrong, then look again once it \
                         says ready"
                    .to_string(),
                jump,
            },
        };
    }

    if local > 0 {
        return DrainLine {
            band: 3,
            ready: false,
            name,
            row: Row::Answer {
                // **Critical, beside *would never finish draining* and below it.** Completing is
                // not the same danger as never completing, but it is a worse one than *nothing
                // recreates this pod*: the reader may not know there was anything on the pod's
                // own disk to lose (`screens/analysis.md` § *A node that would throw away
                // files*).
                severity: Some(Severity::Critical),
                text: format!(
                    "{name} drains, but throws away files on {local} {}",
                    if local == 1 { "pod" } else { "pods" }
                ),
                detail: std::iter::once(local_storage_paragraph(local, Position::OwnRow))
                    .chain(the_other_problems(0, orphans, memory, &stale))
                    .collect(),
                // **Not the orphan row's sentence.** An orphan pod never comes back; a pod with
                // local storage usually does, behind whatever owns it — what does not come back
                // is only what was sitting on this one machine's disk.
                action: if local == 1 {
                    "copy what you need off it first — the replacement pod starts with an empty \
                     disk"
                        .to_string()
                } else {
                    "copy what you need off them first — the replacement pods start with an \
                     empty disk"
                        .to_string()
                },
                jump,
            },
        };
    }

    if orphans > 0 {
        return DrainLine {
            band: 2,
            ready: false,
            name,
            row: Row::Answer {
                severity: Some(Severity::Warn),
                text: format!(
                    "{name} has {orphans} {} nothing would restart",
                    if orphans == 1 { "pod" } else { "pods" }
                ),
                detail: std::iter::once(orphan_paragraph(orphans, Position::OwnRow))
                    .chain(the_other_problems(0, 0, memory, &stale))
                    .collect(),
                action: if orphans == 1 {
                    "save what you need off it first".to_string()
                } else {
                    "save what you need off them first".to_string()
                },
                jump,
            },
        };
    }

    if memory > 0 {
        return DrainLine {
            // **`Info`, below the orphan row and above the no-band ones.** Nothing here is lost,
            // so ranking it above a real permanent loss would teach the wrong lesson about which
            // glyph means *act now*; ranking it beside *is ready to drain* would hide a refusal
            // the reader is about to hit for real (`screens/analysis.md` § *One volume kind, two
            // mediums*).
            band: 1,
            // **Not *drainable right now*, the same as the disk row** — a bare
            // `kubectl drain --ignore-daemonsets` genuinely refuses on these pods even though
            // nothing on them is at risk.
            ready: false,
            name,
            row: Row::Answer {
                severity: Some(Severity::Info),
                text: format!(
                    "{name} drains, but needs one more flag for {memory} {}",
                    if memory == 1 { "pod" } else { "pods" }
                ),
                detail: std::iter::once(memory_paragraph(memory, Position::OwnRow))
                    .chain(the_other_problems(0, 0, 0, &stale))
                    .collect(),
                // **The one thing the undifferentiated row got wrong, and the whole reason this
                // is its own row rather than a note under the disk one.** *"Copy what you need
                // off it first"* is advice about a volume with nothing to copy.
                action: if memory == 1 {
                    "add --delete-emptydir-data when you drain — there is nothing on this pod to \
                     copy off first"
                        .to_string()
                } else {
                    "add --delete-emptydir-data when you drain — there is nothing on these pods \
                     to copy off first"
                        .to_string()
                },
                jump,
            },
        };
    }

    if let Some(first) = stale.first() {
        // **No band at all, and it sorts with the ready nodes** — a controller that has not
        // finished counting is normally under a second behind and resolves by itself, and
        // dressing that in this pane's loudest band teaches a reader to distrust the band the
        // next time it is genuinely urgent (`screens/analysis.md` § *A budget that has not caught
        // up yet*). It is the same family `node   could not be worked out` sits in on Capacity: a
        // fact k8rs cannot answer yet, not a verdict.
        let mut detail = vec![first.sentence.clone()];
        if stale.len() > 1 {
            let others = stale.len() - 1;
            detail.push(if others == 1 {
                "One other rule on this node has not caught up either.".to_string()
            } else {
                format!("{others} other rules on this node have not caught up either.")
            });
        }
        return DrainLine {
            // **Band 0, the ready nodes' own** — it sorts *with* them and not above them, so the
            // two differ only in which sentence a reader sees ([`DrainLine::ready`]).
            band: 0,
            ready: false,
            name,
            row: Row::Answer {
                severity: None,
                text: format!("{name} needs a moment before it can be checked"),
                detail,
                action: "wait a few seconds and look again — if it never catches up, check that \
                         the cluster's controller manager is running"
                    .to_string(),
                jump,
            },
        };
    }

    DrainLine {
        band: 0,
        ready: true,
        name,
        row: Row::Answer {
            severity: None,
            text: match moving.len() {
                // A node carrying only static and DaemonSet pods is ready to drain, and saying
                // *0 pods move* about it reads as an error rather than as an answer.
                0 => format!("{name} is ready to drain — nothing on it would move"),
                1 => format!("{name} is ready to drain — 1 pod moves"),
                n => format!("{name} is ready to drain — {n} pods move"),
            },
            detail: Vec::new(),
            action: String::new(),
            jump,
        },
    }
}

/// **What the pane says about a machine that has stopped answering**, written once here because
/// it is the row's own sentence about a *drain* and not N1's about the node — N1's card says what
/// is wrong with the machine, and this says what that costs the operation this pane is about
/// (`screens/analysis.md` § *A node that has stopped responding*). The way out is N1's own,
/// verbatim, and is read off the card rather than copied.
const NODE_SILENT: &str = "This node has stopped responding. A drain cannot confirm a pod is gone \
                           until it answers again, so it waits forever.";

/// **Which half of N1 is true about this node.** N1 fires on `conditions[Ready]` at anything but
/// `True` past `NODE_DOWN_GRACE`, and its two branches cost a drain two different things.
enum NotReady<'a> {
    /// **`Ready: Unknown`, and any status this code cannot read** — the kubelet stopped posting,
    /// so a drain cordons the node, evicts, and waits forever for a confirmation only that
    /// kubelet can give. Carries N1's own card, because the row reuses its way out verbatim.
    Silent(&'a Finding),
    /// **`Ready: False`** — the kubelet answered and said no, and `conditions[Ready].status` does
    /// not say which kind of no. `KubeletNotReady: container runtime is down` and `PLEG is not
    /// healthy` are kubelets that post status and cannot stop a container, so their evicted pods
    /// sit `Terminating` forever exactly as the `Unknown` case does; `NetworkPluginNotReady` is
    /// one that can. Neither *would never finish draining* nor *is ready to drain* is defensible
    /// about it, so the pane says what it knows: not enough (NOTES § D134).
    SaidNo,
}

/// **N1's card about this node, or nothing** — picked out of [`crate::rules::analyze`]'s own
/// output, so this pane and the Alerts card about one machine read the same fact and cannot come
/// to disagree about it, `NODE_DOWN_GRACE`'s five minutes included (NOTES § D46, § D131).
///
/// **Both halves of N1, and no sibling rule to keep in step with it.** The finding's presence is
/// the whole of *is something wrong, and has it been wrong long enough to say so*; the node's own
/// `Ready` status only picks which of the two rows to draw. Reading the status without the finding
/// would be a second five-minute grace to keep in step with N1's; reading the finding without the
/// status cannot tell the two branches apart, because a [`Finding::title`] is a plain-language
/// sentence and a match on one stops matching the next time invariant 14 rewords it.
///
/// **N1's tri-state, read N1's way**: `False` is a kubelet that answered, and anything else — the
/// `Unknown` the API server writes, or a status this code does not know — is one that did not.
/// A value nobody can read is not evidence that the machine replied.
///
/// **The identity is the kind, the name and the band, and the band is load-bearing.** N1 is the
/// only `Critical` node rule in `rules.rs`; N2 and N3 are both `Warn`, which is what makes those
/// three fields enough today, and the test beside this one pins it so that a second `Critical`
/// node rule turns red here rather than putting another rule's sentence under this row.
fn not_ready<'a>(findings: &'a [Finding], node: &NodeSnapshot) -> Option<NotReady<'a>> {
    let card = findings.iter().find(|finding| {
        finding.severity == Severity::Critical
            && finding.object.kind == ObjectKind::Node
            && finding.object.name == node.id.name
    })?;
    let answered = node
        .conditions
        .iter()
        .any(|c| c.type_ == "Ready" && c.status == "False");
    Some(if answered {
        NotReady::SaidNo
    } else {
        NotReady::Silent(card)
    })
}

/// **What the pane says about a machine whose kubelet answered and said no** — the row's own
/// sentence, not N1's. N1's card says what is wrong with the machine; this says that the one
/// question this pane exists to answer cannot be answered while that is true
/// (`screens/analysis.md` § *`Ready: False`, reversed*).
const CANNOT_TELL: &str = "This node says it cannot run pods right now — the same thing its \
                           Alerts card says. A kubelet that is still talking might still confirm \
                           an eviction, or it might not. k8rs cannot tell which from here, so \
                           this pane will not guess.";

/// **Every *other* true reason about this node**, in the same band order the rows are in — the
/// paragraphs that sit under whichever row won the text (`screens/analysis.md` § Drain safety).
///
/// **A count of `0` is how a caller says *this one is already the row's own text***: the node
/// whose loudest problem is its local storage passes `local: 0`, because the row above these
/// paragraphs draws it in [`Position::OwnRow`] and a second, self-contained copy under it is the
/// same fact twice. Every paragraph this builds is [`Position::Folded`] by construction — that is
/// what *under a row somebody else won* means.
fn the_other_problems(
    local: usize,
    orphans: usize,
    memory: usize,
    stale: &[NotCaughtUp],
) -> Vec<String> {
    let mut out = Vec::new();
    if local > 0 {
        out.push(local_storage_paragraph(local, Position::Folded));
    }
    if orphans > 0 {
        out.push(orphan_paragraph(orphans, Position::Folded));
    }
    if memory > 0 {
        out.push(memory_paragraph(memory, Position::Folded));
    }
    // **The transient fact is not lost, only never the loudest thing on the row.** One line, and
    // it names the first budget: what the reader does about it is *look again*, and the row above
    // is where the thing they act on now is.
    out.extend(stale.first().map(|first| {
        format!(
            "and {}'s numbers have not caught up yet — check again in a moment",
            first.budget
        )
    }));
    out
}

/// **Where a paragraph is drawn, which is what decides whether it says its own count.**
///
/// Under its own row the count is already in the text a line above — *"throws away files on 2
/// pods"* over *"2 pods here keep files"* is the same number twice on adjacent lines, which is
/// the fix the `action` strings already had (*"copy what you need off them first"* restates
/// nothing). Under a **louder** row there is no *they* for the sentence to point at, so the
/// paragraph has to name what it is about (NOTES § D134, `screens/analysis.md` § *A paragraph
/// reads differently depending on whether it is the row's own text*).
#[derive(Clone, Copy)]
enum Position {
    /// The row directly above says this paragraph's own count.
    OwnRow,
    /// Something louder won the row, and this paragraph is one of the facts folded under it.
    Folded,
}

/// **The sentence under a node whose drain would delete files off the machine itself** — written
/// once because four of the row kinds draw it, in the two [`Position`]s it can be drawn in.
fn local_storage_paragraph(count: usize, position: Position) -> String {
    match (count, position) {
        (1, Position::OwnRow) => "It keeps files on this machine's own disk — what Kubernetes \
                                  calls an emptyDir volume — and a drain deletes it with the pod."
            .to_string(),
        (1, Position::Folded) => "1 pod here keeps files on this machine's own disk — what \
                                  Kubernetes calls an emptyDir volume — and a drain deletes them \
                                  with the pod."
            .to_string(),
        (_, Position::OwnRow) => "They keep files on this machine's own disk — what Kubernetes \
                                  calls an emptyDir volume — and a drain deletes them with the \
                                  pods."
            .to_string(),
        (count, Position::Folded) => format!(
            "{count} pods here keep files on this machine's own disk — what Kubernetes calls an \
             emptyDir volume — and a drain deletes them with the pods."
        ),
    }
}

/// **The sentence under a node whose drain would stop on a tmpfs**, in the two [`Position`]s it
/// can be drawn in.
///
/// **Two facts in one paragraph, and they do not point the same way**: the drain refuses without
/// the flag, and nothing is lost. A reader who sees only one of them is either warned about a
/// loss that will not happen or not warned about a refusal they are about to hit (NOTES § D134).
///
/// **The folded form says *the same extra flag*** — under a louder row the disk paragraph above
/// it has already named `--delete-emptydir-data`'s job, so this one points at it rather than
/// repeating the refusal as if it were new.
fn memory_paragraph(count: usize, position: Position) -> String {
    let tail = "Nothing is lost: that storage empties every time the container restarts anyway.";
    let volume = "what Kubernetes calls an emptyDir volume set to use memory";
    match (count, position) {
        (1, Position::OwnRow) => format!(
            "It keeps files in memory only — {volume} — and a bare drain refuses to touch it. \
             {tail}"
        ),
        (1, Position::Folded) => format!(
            "1 pod here keeps files in memory only — {volume} — and a drain needs the same extra \
             flag to touch it. {tail}"
        ),
        (_, Position::OwnRow) => format!(
            "They keep files in memory only — {volume} — and a bare drain refuses to touch them. \
             {tail}"
        ),
        (count, Position::Folded) => format!(
            "{count} pods here keep files in memory only — {volume} — and a drain needs the same \
             extra flag to touch them. {tail}"
        ),
    }
}

/// **The sentence under a node carrying pods no controller would recreate**, in the two
/// [`Position`]s it can be drawn in.
///
/// **`1 pod here`, not `One pod here`** — the folded singular was the only *counted paragraph* on
/// this pane that spelled its number as a word, and every counted row here uses the digit
/// (NOTES § D134). [`drain_row`]'s trailing *"One other rule … has not caught up either"* is a
/// different line and was not in that turn's scope.
fn orphan_paragraph(count: usize, position: Position) -> String {
    match (count, position) {
        (1, Position::OwnRow) => "It was started by hand, with no Deployment behind it. A drain \
                                  deletes it and nothing brings it back."
            .to_string(),
        (1, Position::Folded) => "1 pod here was started by hand, with no Deployment behind it. \
                                  A drain deletes it and nothing brings it back."
            .to_string(),
        (_, Position::OwnRow) => "They were started by hand, with no Deployment behind them. A \
                                  drain deletes them and nothing brings them back."
            .to_string(),
        (count, Position::Folded) => format!(
            "{count} pods here were started by hand, with no Deployment behind them. A drain \
             deletes them and nothing brings them back."
        ),
    }
}

/// **Does this budget have anything to say about a drain of this node?** — it protects at least
/// one pod the drain would actually move.
///
/// A PodDisruptionBudget only ever protects pods in its own namespace, so the namespace is half
/// the join and [`selects`] is the other half.
fn protects_anything_moving(budget: &DisruptionBudgetSnapshot, moving: &[&PodSnapshot]) -> bool {
    moving.iter().any(|pod| {
        pod.id.namespace == budget.id.namespace && selects(budget.selector.as_ref(), &pod.labels)
    })
}

/// **A budget the controller has not finished counting**, and the sentence that says so.
struct NotCaughtUp {
    /// `namespace/name` — the object the trailing line names when something louder won the row.
    budget: String,
    /// The whole paragraph, for the row whose own text is *needs a moment before it can be
    /// checked*.
    sentence: String,
}

/// **Has this budget's status caught up with its spec?** — `None` when it has, and the sentence
/// for the row when it has not.
///
/// Upstream's eviction handler compares `metadata.generation` against `status.observedGeneration`
/// and refuses *every* eviction while the spec is ahead — with the same `TooManyRequests` it
/// returns for a full budget, so the failure does not explain itself (NOTES § D130).
///
/// **It is not [`blocks_a_drain`]'s fourth branch any more, and the reband is the point.** The
/// refusal is real, but a controller that is merely behind is normally over in well under a
/// second and resolves without an operator: drawing that in this pane's loudest band, under
/// *would never finish draining*, put
/// *"look again in a moment"* beneath the most urgent thing the screen can say
/// (`screens/analysis.md` § *A budget that has not caught up yet*). It keeps its row and loses
/// its band.
///
/// **`observed_generation: None` counts as behind**: the field is an `int64` upstream, absent
/// decodes as 0, and `0 < generation` is the comparison the API server makes. `generation: None`
/// makes no comparison at all and says nothing — there is no number to be behind.
///
/// **Behind is not the same as still catching up, and the caller asks the second question too.**
/// A budget whose sync failed never advances `status.observedGeneration`, so it answers this one
/// the same way a budget edited a second ago does and stays that way until a human fixes it —
/// [`could_not_be_counted`] is what tells the two apart, at [`drain_row`].
fn has_not_caught_up(budget: &DisruptionBudgetSnapshot) -> Option<NotCaughtUp> {
    let generation = budget.generation?;
    if budget
        .observed_generation
        .is_some_and(|seen| seen >= generation)
    {
        return None;
    }
    let name = qualified(&budget.id);
    let seen = match budget.observed_generation {
        Some(seen) => format!("the count is from version {seen}"),
        None => "the count has not been worked out at all".to_string(),
    };
    Some(NotCaughtUp {
        sentence: format!(
            "{name} was just changed and Kubernetes has not finished counting its healthy pods — \
             the change is version {generation}, {seen}."
        ),
        budget: name,
    })
}

/// Why one budget stops a drain, and what to do about it.
///
/// **The name is back, and it is the *other* budgets that need it.** It was the sort key here
/// until the order moved onto the budget list itself, where it is `(namespace, name)` like every
/// other list on this screen (`reports/2026-08-21-family-c-analysis-report-family-review.md`
/// § 7) — and both sentences below already name the budget the row's own paragraph is about. What
/// has no name without this field is every *further* budget blocking the same node, which a
/// reader who clears the first one hits next (NOTES § D134).
struct Blocked {
    /// `namespace/name` — the object the trailing line names, the same shape [`NotCaughtUp`]
    /// carries for the same reason.
    budget: String,
    explanation: String,
    action: String,
}

/// **Has the controller reported that it tried to count this budget and could not?** — the one
/// place the `DisruptionAllowed` condition is read, called from both [`drain_row`] and
/// [`blocks_a_drain`], because two readings of one condition is how two rules come to disagree
/// about one object.
///
/// **Only `failSafe` writes this reason, and every successful sync overwrites it.** Upstream
/// (`release-1.34`, `pkg/controller/disruption/disruption.go`): `sync` calls `failSafe` when
/// `trySync` returns a non-conflict error, and `failSafe` sets `DisruptionsAllowed = 0` and this
/// condition with `ObservedGeneration: newPdb.Status.ObservedGeneration` — the value the status
/// already had. A sync that succeeds goes through `updatePdbStatus`, which writes
/// `ObservedGeneration: pdb.Generation` and then calls `UpdateDisruptionAllowedCondition`, whose
/// only two reasons are `SufficientPods` and `InsufficientPods`. So this reason means *the last
/// thing the controller did to this budget was fail*, which is why it outranks the generation gap
/// at the caller ([`drain_row`]) rather than being ordered behind it.
///
/// **And it is the only field that separates the two, which was read rather than assumed.** Both
/// upstream writers copy `status.observedGeneration` into the condition's own
/// `observedGeneration`, so the per-condition number carries nothing the two carried generations
/// do not — [`crate::rules::Condition`] drops it (NOTES § D46) and loses nothing here.
fn could_not_be_counted(budget: &DisruptionBudgetSnapshot) -> bool {
    budget
        .conditions
        .iter()
        .find(|c| c.type_ == "DisruptionAllowed")
        .and_then(|c| c.reason.as_deref())
        == Some("SyncFailed")
}

/// **Would the eviction API refuse this budget's pods right now, and why?** — `None` when it
/// would not.
///
/// **Three refusals, and the counters are read only about a budget whose status has caught up
/// with its spec** — [`has_not_caught_up`] is the caller's question and is not repeated here,
/// because while the spec is ahead none of the counters below is a measurement of anything
/// (NOTES § D130, `reports/2026-08-21-family-c-corpus-drain-and-capacity.md` §§ 1, 13.4). The one
/// budget that arrives here still behind its spec is the one [`could_not_be_counted`] answers for,
/// and branch 1 returns before a counter is touched:
///
/// 1. **`SyncFailed`** — the controller could not resolve the workload's `scale` subresource, so
///    the three counters beside it are not a measurement of anything and a sentence built from
///    them would be invented. Asked before the counters for exactly that reason.
/// 2. **At its floor** — `disruptions_allowed == 0` with as many healthy pods as the budget
///    demands. This is the row the report exists for.
/// 3. **Below its floor** — the same zero, and it is *not* the same row: the workload is already
///    down, *"a drain takes one away"* is false about it, and **run one more copy** is not the
///    way out. D130 built the negative for this and § 13.4 caught the live cluster in it.
///
/// **`disruptions_allowed: None` refuses nothing** — the field is absent until the controller has
/// looked at the budget, and reading that as zero calls every freshly created budget blocking
/// ([`crate::rules::DisruptionBudgetSnapshot::disruptions_allowed`]). [`has_not_caught_up`]
/// already covers the shape that matters, because a budget the controller has not reached has no
/// `observedGeneration` either — and a budget it reached and failed on carries `SyncFailed`, which
/// branch 1 answers above without reading this counter at all.
///
/// **`spec.minAvailable` is read nowhere here** — it is an `IntOrString`, `minAvailable: "50%"`
/// is legal and common, and the API server resolves it *and* `maxUnavailable` into
/// `status.desiredHealthy` (NOTES § D130).
fn blocks_a_drain(budget: &DisruptionBudgetSnapshot) -> Option<Blocked> {
    let name = qualified(&budget.id);
    let blocked = |explanation: String, action: String| {
        Some(Blocked {
            budget: name.clone(),
            explanation,
            action,
        })
    };

    if could_not_be_counted(budget) {
        return blocked(
            format!(
                "Kubernetes could not work out how many copies of the pods {name} protects are \
                 healthy, so it will not let any of them be moved. The numbers on it are not a \
                 measurement of anything."
            ),
            format!(
                "check what {name} points at — this happens when it names something Kubernetes \
                 cannot count copies of"
            ),
        );
    }

    if budget.disruptions_allowed != Some(0) {
        return None;
    }
    match (budget.current_healthy, budget.desired_healthy) {
        (Some(current), Some(desired)) if current >= desired => blocked(
            format!(
                "{name} keeps at least {} of the pods it protects, and right now exactly {}. A \
                 drain has to take one away, so it waits forever.",
                copies(desired),
                healthy(current)
            ),
            "run one more copy of what it protects, or lower the minimum it must keep".to_string(),
        ),
        (Some(current), Some(desired)) => blocked(
            format!(
                "{name} keeps at least {} of the pods it protects, and right now {}. It will not \
                 let any be moved until they are back — a drain would wait on pods that are \
                 already down.",
                copies(desired),
                healthy(current)
            ),
            "get the pods it protects healthy again first, then drain".to_string(),
        ),
        // The controller answered the one question that matters and not the two the sentence
        // above is built from. The row still fires — the drain still hangs — and says only what
        // it can show.
        _ => blocked(
            format!("{name} will not let any of the pods it protects be moved right now."),
            "run one more copy of what it protects, or lower the minimum it must keep".to_string(),
        ),
    }
}

/// `1 copy` · `5 copies` — the floor a budget keeps, in words rather than in a number with an
/// `(s)` after it (invariant 14).
fn copies(count: i32) -> String {
    if count == 1 {
        "1 copy".to_string()
    } else {
        format!("{count} copies")
    }
}

/// `none are healthy` · `1 is healthy` · `4 are healthy` — the other half of the same sentence.
/// **Zero gets its own word**, because *"and right now exactly 0 are healthy"* is the line a
/// reader has to parse twice at 3am.
fn healthy(count: i32) -> String {
    match count {
        0 => "none are healthy".to_string(),
        1 => "1 is healthy".to_string(),
        count => format!("{count} are healthy"),
    }
}

/// **Does this selector pick this object's labels?** — the first `LabelSelector` matcher in this
/// repository, written here because Drain safety is the first reader of one
/// ([`crate::rules::Selector`], whose doc says matching is the report's and not the type's).
///
/// **Absent and empty are two different answers, and that is the whole of the `Option`.** `None`
/// is a `null` selector and picks no pods — `policy/v1`'s reading, the reverse of
/// `policy/v1beta1`. A `Some` with nothing in it is upstream's `labels.Everything()` and picks
/// every pod in the budget's namespace ([`crate::rules::DisruptionBudgetSnapshot::selector`],
/// which quotes both halves off upstream's own docs).
///
/// **The second is not a special case below**: `all` over an empty list is `true`, which is
/// exactly what *matches everything* means. The early `return false` that used to stand there
/// was the defect — it made a budget written `{}` protect nothing and put *"ready to drain"* in
/// front of a drain that hangs.
///
/// **Both halves are ANDed, and an unknown operator matches nothing** — upstream's
/// `LabelSelectorAsSelector` errors on an operator it does not know, and a caller that cannot
/// build the selector matches no object at all.
fn selects(selector: Option<&Selector>, labels: &BTreeMap<String, String>) -> bool {
    let Some(selector) = selector else {
        return false;
    };
    selector
        .match_labels
        .iter()
        .all(|(key, value)| labels.get(key) == Some(value))
        && selector
            .match_expressions
            .iter()
            .all(|requirement| satisfies(requirement, labels))
}

/// One `matchExpressions[]` entry against one object's labels, on upstream's own truth table
/// (`k8s.io/apimachinery/pkg/labels`, `Requirement.Matches`).
///
/// **`NotIn` on a key that is absent is `true`**, which is the one row of that table a
/// hand-written matcher gets backwards: *the label is not one of these* is satisfied by an object
/// that does not carry the label at all.
fn satisfies(requirement: &SelectorRequirement, labels: &BTreeMap<String, String>) -> bool {
    let held = labels.get(&requirement.key);
    match requirement.operator.as_str() {
        "In" => held.is_some_and(|value| requirement.values.contains(value)),
        "NotIn" => held.is_none_or(|value| !requirement.values.contains(value)),
        "Exists" => held.is_some(),
        "DoesNotExist" => held.is_none(),
        _ => false,
    }
}

// --- THE DRAIN SAFETY REPORT END ---

// --- THE WASTE REPORT START ---

/// **Things that cost something and give nothing back** — the Service nobody can reach, the disk
/// nobody mounted, the pods a node removed, the pods that finished and stayed, the ReplicaSets
/// left at zero (`screens/analysis.md` § Waste).
///
/// **Five rows from four sections**: the pods that are over are one section drawing two rows, one
/// per cause (NOTES § D155, [`finished_pods_left_behind`]), which is why [`UNREADABLE_SECTIONS`]
/// is still three — that section is counted straight off the pods and can contribute no
/// [`Row::NotComputed`] to fold.
///
/// **It runs unchanged when the view is scoped, and only the title changes.** Every input it has
/// is namespaced, and every number on it is the length of a list rather than a share of a total,
/// so a narrower view is a shorter list and never a wrong number — which is exactly the
/// difference from Capacity, whose promised sum comes out silently low
/// (PRIOR-ART § F2, `screens/analysis.md` § *Waste under one namespace*).
///
/// **The Service matching no pod is first on purpose**: it is the 503 nobody can explain. It is a
/// report row and not an alert because promoting it would cost a permanent Services +
/// EndpointSlices watch, and the watch budget is why k8rs is lighter than k9s (NOTES § D9).
pub fn waste(snapshot: &ClusterSnapshot, _findings: &[Finding]) -> Report {
    // Rule 6: a title names a namespace only where there is one. The dangerous state is the
    // narrow one, so it is the labelled one.
    let title = match snapshot.namespace_scope.as_deref() {
        Some(namespace) => format!("Things in {namespace} that cost you something for nothing"),
        None => "Things that cost you something for nothing".to_string(),
    };
    let mut rows = services_reaching_nothing(snapshot);
    rows.extend(disks_nobody_mounts(snapshot));
    rows.extend(finished_pods_left_behind(snapshot));
    rows.extend(replica_sets_parked_at_zero(snapshot));
    // **A pane of nothing but excuses is one excuse** — Drain safety's shape, reached here for
    // rule 7's stated reason rather than its letter: three sections drawing one `NotComputed` each
    // obeys *one per section*, and stacked over an empty pane they are three ways out for a reader
    // who can only take one. The shape is one ordinary namespaced role with none of the three
    // cluster verbs, not a corner.
    //
    // **Both halves of the condition are load-bearing.** No [`Row::Answer`] surviving is *nothing
    // answered*; the length is *nothing was even asked* — each unread section draws exactly one
    // row ([`UNREADABLE_SECTIONS`]), so two excuses beside a section that ran and found nothing is
    // a pane where something did answer, and the folded sentence below, which names all four
    // lists, would be false about the one that was read. Those keep their per-section rows.
    if rows.len() == UNREADABLE_SECTIONS
        && rows
            .iter()
            .all(|row| matches!(row, Row::NotComputed { .. }))
    {
        rows = vec![Row::NotComputed {
            reason: "Not checked. Working out what is going to waste needs the lists of what this \
                     cluster has — its Services, the addresses behind them, the disk reservations \
                     and the replicasets — and this login could not read any of them."
                .to_string(),
            ask_for: "Ask for permission to list services, endpointslices, \
                      persistentvolumeclaims and replicasets."
                .to_string(),
        }];
    }
    if rows.is_empty() {
        // Rule 8, in this report's own words. **Only when there is nothing else at all**: a pane
        // carrying one `NotComputed` has not established that nothing is going to waste, and
        // saying so over a section that did not run is the sentence this screen exists not to
        // print.
        rows.push(Row::Prose(
            "Nothing here is going to waste. Every Service reaches a pod, every disk that was \
             reserved is mounted, and no pod — finished or removed by a node — is left lying \
             around."
                .to_string(),
        ));
    }
    Report {
        title,
        // No badge, like Drain safety and for the same reason ([`Report::badge`]): the sidebar
        // has room for a number and this pane's rows count four different things.
        badge: None,
        rows,
    }
}

/// **How many of this report's sections can go unread** — the three that need a list fetched when
/// the pane opens. The fourth is counted straight off [`crate::rules::ClusterSnapshot::pods`],
/// which is always there, so a pane on which nothing answered is exactly this many
/// [`Row::NotComputed`]s and never more: each unread section draws one.
const UNREADABLE_SECTIONS: usize = 3;

/// **The most per-object rows one section of this report may draw**, and the answer
/// [`Row::Answer::jump`] says the Waste box owes.
///
/// **Read off the pane, not picked.** The report region is 16 body lines at the 80×24 floor
/// (`screens/analysis.md` § *How a report is drawn*), and a per-object row there is its text, one
/// or two wrapped `detail` lines and sometimes an action — three to five lines, which is what
/// the § Waste mockup's four rows take in fourteen of its sixteen. Five is therefore one
/// pane-height of one section: past it the reader is scrolling a list they cannot act on item by
/// item, and the answer they need is the count. On a cluster with 812 broken Services this pane
/// is five rows and a line that says so, not 812 rows and 3200 lines of scrolling.
///
/// **Per section and not per pane**, so a cluster with 812 broken Services still shows its three
/// orphaned disks: one loud section may not starve the others.
///
/// **The line is per-object against aggregate, and it is why two panes on this screen cap
/// nothing.** What is cut here is the rows that are one *per object*, which grow with the
/// cluster's object count. A counted row does not — this pane's three (`4 pods were removed by
/// a node`, `47 pods finished`, `12 replicasets`) are one row each whatever the number is — and
/// neither does a Posture row, which is one per host path with its pod count inside it
/// ([`posture`]). Capacity's node list is unbounded and is not cut either, because
/// `screens/analysis.md` § Capacity rules that that pane scrolls.
const MOST_ROWS_PER_SECTION: usize = 5;

/// The section's rows, cut to [`MOST_ROWS_PER_SECTION`] with a line saying what was left off.
///
/// **The overflow line is a [`Row::Prose`], and that is the half of the answer about the
/// variant** (NOTES § D127). It is not a [`Row::Answer`]: the cursor landing on it would
/// advertise `⏎` over nothing openable — not one object, and not a *set* a future [`Jump`] case
/// could be built for either, because the set it names is *the remainder of a list*, which has no
/// identity of its own. The counted rows of this report are different and stay `Answer`s: `47
/// pods finished` is the report's own answer to its own question, and one day `⏎` lists those 47.
/// Promoting a `Prose` later is one edit; a selectable row that opens nothing is a key that does
/// nothing today.
fn at_most(rows: Vec<Row>, left_off: impl FnOnce(usize) -> String) -> Vec<Row> {
    let over = rows.len().saturating_sub(MOST_ROWS_PER_SECTION);
    let mut kept: Vec<Row> = rows.into_iter().take(MOST_ROWS_PER_SECTION).collect();
    if over > 0 {
        kept.push(Row::Prose(left_off(over)));
    }
    kept
}

/// **The 503 nobody can explain** — a Service with a selector that no pod is behind.
///
/// **Two fields and one row, so both must be `Some`**
/// ([`crate::rules::ClusterSnapshot::endpoint_slices`]):
/// Services present with the slices missing reads as *every Service matches nothing*, which is
/// the loudest possible wrong answer this pane could give.
///
/// **An empty selector is not a defect and is skipped.** A Service with no selector has its
/// endpoints managed by hand or by another controller — `kubernetes` in `default` is one on every
/// cluster ever built — so *matches no pod* is not a thing to say about it
/// ([`crate::rules::ServiceSnapshot::selector`]). This is the equality-only map upstream gives a
/// Service and is deliberately **not** the [`crate::rules::Selector`] [`selects`] reads.
fn services_reaching_nothing(snapshot: &ClusterSnapshot) -> Vec<Row> {
    let (Some(services), Some(slices)) = (
        snapshot.services.as_deref(),
        snapshot.endpoint_slices.as_deref(),
    ) else {
        return vec![Row::NotComputed {
            reason: "Services that match no pod are not checked. That takes both the list of \
                     Services and the list of the addresses behind them, and one of the two \
                     could not be read."
                .to_string(),
            ask_for: "Ask for permission to list services and endpointslices.".to_string(),
        }];
    };
    let behind = endpoints_behind(slices);
    let mut orphans: Vec<&ServiceSnapshot> = services
        .iter()
        .filter(|service| !service.selector.is_empty())
        .filter(|service| {
            behind
                .get(&(service.id.namespace.as_deref(), service.id.name.as_str()))
                .copied()
                .unwrap_or(0)
                == 0
        })
        .collect();
    // (namespace, name) — the order the reader's own `kubectl get -A` prints, and not
    // `ObjectId::group_key`, whose `ObjectKind` has no `Ord` and would order one kind by nothing.
    orphans.sort_by(|a, b| (&a.id.namespace, &a.id.name).cmp(&(&b.id.namespace, &b.id.name)));
    at_most(
        orphans
            .into_iter()
            .map(|service| Row::Answer {
                severity: Some(Severity::Critical),
                text: format!("{} matches no pod", qualified(&service.id)),
                detail: vec![
                    "This Service points at nothing. Anything calling it gets a 503.".to_string(),
                ],
                action: "fix its selector, or delete it".to_string(),
                jump: Some(Jump::Object(service.id.clone())),
            })
            .collect(),
        |over| format!("and {over} more Services match no pod"),
    )
}

/// **How many endpoints are behind each Service**, ready or not
/// ([`crate::rules::EndpointSliceSnapshot::endpoints`]): the row is *matches no pod*, so what it
/// asks is whether anything is behind the Service at all. A pod that exists and is failing its
/// readiness probe is Alerts' rule 7 and is already on the other screen.
///
/// **Endpoints and not pods, and on a dual-stack Service the two differ.** One replica behind an
/// `ipFamilyPolicy: RequireDualStack` Service is two slices, IPv4 and IPv6, each listing it once,
/// so this sums to 2 for one pod. That is right for the only question asked of it here — the call
/// site reads `== 0` — and wrong for any consumer that reads the number as *pods behind the
/// Service*. Every slice in the corpus is IPv4, so no fixture can show it; the dual-stack shape
/// is a plant (`analysis_tests/waste.rs`, NOTES § D40).
///
/// A slice carries its Service's **name** and lives in its namespace, so both halves are the key
/// — a `payments/web` slice says nothing about `staging/web`, and a name-only key would answer
/// *matches no pod* about whichever of the two the other one's slice did not cover. A Service
/// absent from the map has no slice at all, which is the same answer as a slice holding no
/// endpoint: both are `0` at the call site.
///
/// **One pass over the slices, and the whole join in it.** Asking the slice list once per Service
/// is quadratic in a count nothing here bounds — [`MOST_ROWS_PER_SECTION`] caps the rows drawn,
/// not the objects visited — and it is the one join on this screen with that shape; the others
/// are measured and are not to be "optimised" with it
/// (`reports/2026-08-22-phase-4-close-cross-family-review.md` § 3, where the figures are).
///
/// A slice carrying no Service name is hand-managed and says nothing about any Service
/// ([`crate::rules::EndpointSliceSnapshot::service`]), so it is not in the map — the label is the
/// only thing that puts a slice behind a Service, and the object's own name is not it.
///
/// **A label that is present and empty is a different shape and does land in the map**, under
/// `(namespace, "")`: the API server accepts `kubernetes.io/service-name: ""` and the decode
/// keeps it as `Some("")` (`rules.rs` § the snapshot types). Nothing the API server can return
/// looks that key up, because a Service always has a name — and kube-proxy programs no route for
/// such a slice either, so this pane and the data plane give the same answer.
fn endpoints_behind(slices: &[EndpointSliceSnapshot]) -> BTreeMap<(Option<&str>, &str), usize> {
    let mut behind = BTreeMap::new();
    for slice in slices {
        if let Some(service) = slice.service.as_deref() {
            *behind
                .entry((slice.id.namespace.as_deref(), service))
                .or_insert(0) += slice.endpoints;
        }
    }
    behind
}

/// **A disk that was reserved and nothing mounted** — [`crate::rules::ClaimSnapshot`] read from
/// the pod side ([`crate::rules::PodSnapshot::claims`]).
///
/// **`Bound` only.** A `Pending` claim has reserved no disk yet and is somebody else's problem,
/// and a `Lost` one is broken rather than wasteful; billing a reader for storage that was never
/// provisioned is the number this report may not print.
///
/// **Any pod naming it counts, finished ones included.** A `Succeeded` Job pod is not using the
/// disk this second, but it is evidence that something mounts it every run — and *"nobody is
/// using it"* about a disk a CronJob mounts hourly is a row that gets a volume deleted.
///
/// **The two lists have to cover the same scope, and keeping them that way is `k8s.rs`'s**
/// (Phase 5). This row is the only place in the report where one fetched list is subtracted from
/// another: a namespaced pod list against a cluster-wide claim list would call every claim
/// outside the scope unmounted, which is the *shorter list, never a wrong number* promise this
/// report makes broken from the other end (`screens/analysis.md` § *Waste under one
/// namespace*).
fn disks_nobody_mounts(snapshot: &ClusterSnapshot) -> Vec<Row> {
    let Some(claims) = snapshot.claims.as_deref() else {
        return vec![Row::NotComputed {
            reason: "Disks nobody is using are not checked. That takes the list of disk \
                     reservations, and this login could not read it."
                .to_string(),
            ask_for: "Ask for permission to list persistentvolumeclaims.".to_string(),
        }];
    };
    // A claim name is namespaced to the pod's own namespace — a PVC cannot be mounted across one
    // — so the pair is the key.
    let mounted: BTreeSet<(Option<&str>, &str)> = snapshot
        .pods
        .iter()
        .flat_map(|pod| {
            pod.claims
                .iter()
                .map(move |claim| (pod.id.namespace.as_deref(), claim.as_str()))
        })
        .collect();
    let mut idle: Vec<&ClaimSnapshot> = claims
        .iter()
        .filter(|claim| claim.phase.as_deref() == Some("Bound"))
        .filter(|claim| !mounted.contains(&(claim.id.namespace.as_deref(), claim.id.name.as_str())))
        .collect();
    idle.sort_by(|a, b| (&a.id.namespace, &a.id.name).cmp(&(&b.id.namespace, &b.id.name)));
    at_most(
        idle.into_iter()
            .map(|claim| Row::Answer {
                severity: Some(Severity::Warn),
                // **The size is what was provisioned and not what was asked for**
                // ([`crate::rules::ClaimSnapshot::capacity`]), spelled by the same [`bytes`] the
                // Capacity rows use — the API's own string here would put two spellings of one
                // number on one pane. A size k8rs cannot read costs the row its number and not
                // its row.
                text: match claim.capacity.as_deref().and_then(quantity_milli) {
                    Some(size) => format!(
                        "{} is {} nobody is using",
                        qualified(&claim.id),
                        bytes(size)
                    ),
                    None => format!(
                        "{} is reserved and nobody is using it",
                        qualified(&claim.id)
                    ),
                },
                // **The StatefulSet sentence is on every row of this kind, not on a row k8rs
                // could tell apart** — `whenScaled` defaults to `Retain`, so a StatefulSet
                // scaled down for the weekend or caught mid rolling-update has its pods' own
                // database volumes here, and nothing on a claim says which of those it is.
                // Deleting one is the classic irrecoverable mistake, so the caveat is said on
                // all of them (NOTES § D134, `screens/analysis.md` § Waste). The band stays
                // `Warn`: an idle disk with a real cost is still worth a look, and the caveat
                // only stops the sentence pushing a reader at the delete key.
                detail: vec![
                    "A disk was reserved for it and no pod is mounting it. It stays reserved \
                     until somebody deletes it. A StatefulSet keeps its pods' disks by default, \
                     even after it is scaled down, so some of this is normal."
                        .to_string(),
                ],
                // No way out is offered on purpose: deleting a claim deletes what is on it, and
                // this report does not know whether that matters.
                action: String::new(),
                jump: Some(Jump::Object(claim.id.clone())),
            })
            .collect(),
        |over| format!("and {over} more disks nobody is using"),
    )
}

/// **The pods that are over and were never removed, in two rows, one per cause**
/// (`screens/analysis.md` § *The pileup splits in two, one per cause*, NOTES § D155).
///
/// **[`finished`] is the outer gate and does not move.** It is `Succeeded | Failed`, and a
/// node-pressure eviction is `Failed` — so it decides whether a pod reaches this section at all
/// and the two rows *partition* what it let through: they always sum to the count one row used to
/// draw, no pod lands on both, and none falls through and lands on neither. The `if`/`else` below
/// is that partition, and it is why the second count is not a second filter.
///
/// **Only the literal `Evicted` splits off** ([`crate::rules::PodSnapshot::reason`]).
/// `DeadlineExceeded`, `NodeAffinity`, `Terminated`, `NodeShutdown`, `OutOfcpu` and every other
/// `Failed` reason stay in the completed row: this pane draws a row for a shape it has measured,
/// not one it is guessing at.
///
/// **The word `Evicted` is said once, in parentheses, and the translation comes first.** The
/// translation lives in NOTES § Positioning item 4 and is deliberately not copied here: that line
/// holds both the words and the constraint that shapes them, that it name no cause. The copy that
/// used to sit in this comment outlived the sentence it was taken from, which is why it is a
/// citation now (NOTES § D158, invariant 14). The row's text and the whole of its explanation are
/// written from that line; the API's own word follows in brackets at the end, the shape
/// `rules.rs` has used for every term this project translates since Phase 3
/// (`… (CrashLoopBackOff)`, `… (OOMKilled)`). It has to be said somewhere: `printPod` overwrites
/// `status.reason` with the container's own terminated reason, so `kubectl get pods` prints
/// `Error` for the capture behind this row and the parenthetical is the only place on the screen
/// the word appears (`reports/2026-08-23-waste-evicted-row-operator-review.md` § 6).
///
/// **Both rows are `Info`.** The killing is what deserved a look and it already happened; what is
/// left behind today costs an etcd entry and a longer `kubectl get pods`, which is exactly the
/// completed row's own cost. An evicted pod is collected only once a node passes 12 500 finished
/// pods (NOTES § D71), so a `Warn` here would stay lit for good after one bad half-hour, clearable
/// only by deleting this pane's own evidence. And it is the completed row's own argument applied
/// to the row that used to be exempt from it: `Warn` over a fact that is often deliberate teaches
/// the wrong lesson the first time a reader chases it and finds nothing to fix — a pod that
/// overran a disk limit it declared for itself is exactly that kind of fact
/// (`screens/analysis.md` § *The pileup splits in two, one per cause*).
///
/// **The removed row is still first, on different ground.** Both are `Info` now, so *louder
/// first* no longer orders them: it leads because it is the more specific statement — it names a
/// cause where the completed row names the absence of one — and because it is the row carrying an
/// action. `severity` and `action` are independent fields on [`Row::Answer`] and the renderer
/// proves it: `main.rs`'s [`Row::Answer`] arm prints the action without reading `severity` at
/// all.
///
/// **The action is the one row's and not the other's, and it points at the object.** These pods
/// did not finish; something killed them. `status.reason: Evicted` has two producers in the
/// kubelet — node pressure, and the pod's own declared storage limit, which consults no node
/// threshold and runs first — so a row that sent the reader to N3 would be sending them to a
/// screen that is silent for the commoner cause
/// (`reports/2026-08-23-waste-evicted-row-operator-review.md` §§ 2–4). What does name the exact
/// resource and moment is each pod's own `status.message`, a field this pane deliberately does
/// not decode ([`crate::rules::PodSnapshot::reason`]).
///
/// **Counted rows and no per-object rows**, so no cap on either: each is the length of a list,
/// honest at any scope (PRIOR-ART § F2). **No threshold either** — the box says *pileup* and every
/// number that could stand for one would be invented here; one pod left behind is one row saying
/// so, and a cluster with none draws nothing.
///
/// **[`Row::Answer::jump`] is `None` on both** — a selectable row with no destination recorded,
/// standing for a set (NOTES § D128).
fn finished_pods_left_behind(snapshot: &ClusterSnapshot) -> Vec<Row> {
    let (mut removed, mut completed) = (0usize, 0usize);
    for pod in snapshot.pods.iter().filter(|pod| finished(pod)) {
        if pod.reason.as_deref() == Some("Evicted") {
            removed += 1;
        } else {
            completed += 1;
        }
    }
    let mut rows = Vec::new();
    if removed > 0 {
        rows.push(Row::Answer {
            severity: Some(Severity::Info),
            text: if removed == 1 {
                "1 pod was removed by a node and remains".to_string()
            } else {
                format!("{removed} pods were removed by a node and remain")
            },
            // One sentence for one pod and for four: nothing in it is counted, because nothing
            // this row can see says which node or which resource. **Both causes, because the
            // capture this row is measured against is the second one** — a pod over its own
            // `8Mi` ephemeral-storage limit, on a node whose three pressure conditions were all
            // `False` at the same moment.
            detail: vec![
                "Either the node was short, or the pod went over its own disk limit (Evicted)."
                    .to_string(),
            ],
            action: "look at one of the pods — its own message names what ran out".to_string(),
            jump: None,
        });
    }
    if completed > 0 {
        let (text, detail) = if completed == 1 {
            (
                "1 pod finished and was never removed".to_string(),
                "Kubernetes keeps a few finished Jobs by default, so some of this is normal. It \
                 uses no CPU or memory — it only makes every pod list longer.",
            )
        } else {
            (
                format!("{completed} pods finished and were never removed"),
                "Kubernetes keeps a few finished Jobs by default, so some of this is normal. They \
                 use no CPU or memory — they only make every pod list longer.",
            )
        };
        rows.push(Row::Answer {
            severity: Some(Severity::Info),
            text,
            detail: vec![detail.to_string()],
            action: String::new(),
            jump: None,
        });
    }
    rows
}

/// **ReplicaSets left at zero when a Deployment moved on** — the quietest row on the pane, and
/// `Info` because nothing here is broken.
///
/// **`Some(0)` and never `None`.** An absent `spec.replicas` is defaulted to **1** by the API
/// server, so `desired.unwrap_or(0)` would count every workload whose field the prune dropped
/// ([`crate::rules::WorkloadSnapshot::desired`]).
fn replica_sets_parked_at_zero(snapshot: &ClusterSnapshot) -> Vec<Row> {
    let Some(sets) = snapshot.replica_sets.as_deref() else {
        return vec![Row::NotComputed {
            reason: "Replicasets parked at 0 replicas are not checked. That takes the list of \
                     replicasets, and this login could not read it."
                .to_string(),
            ask_for: "Ask for permission to list replicasets.".to_string(),
        }];
    };
    let count = sets.iter().filter(|set| set.desired == Some(0)).count();
    if count == 0 {
        return Vec::new();
    }
    vec![Row::Answer {
        severity: Some(Severity::Info),
        text: if count == 1 {
            "1 replicaset is parked at 0 replicas".to_string()
        } else {
            format!("{count} replicasets are parked at 0 replicas")
        },
        detail: vec!["Left behind when deployments moved on.".to_string()],
        action: String::new(),
        jump: None,
    }]
}

// --- THE WASTE REPORT END ---

// --- THE POSTURE REPORT START ---

/// **The host paths that are mounted and are *not* rule 8's** — a list to review, not an alarm to
/// answer (`screens/analysis.md` § Posture, NOTES § D2, § D14).
///
/// **Computed here and not in `rules.rs`.** It reads pod fields, like a rule does, but it
/// produces one whole-cluster list rather than one card per object — and `rules.rs` is frozen
/// (NOTES § D14).
///
/// **The line between this pane and rule 8 is the whole of the report.** Rule 8 keeps the
/// escalated case — the machine's root, a container runtime socket, or a writable mount outside
/// the node infrastructure it stays silent about — and **everything it leaves is here**
/// ([`left_by_rule_8`]). A mount drawing an Alerts card *and* a Posture row would be one pod on
/// two screens saying two different things, which is the divergence NOTES § D46 is about; a mount
/// in neither is a hostPath k8rs never mentions at all. **One shape is deliberately in neither**
/// and it is named where it is dropped: a path that normalises to the empty string
/// ([`host_paths`]).
///
/// **`findings` is unread, and here that is load-bearing.** The partition is decided on the
/// mount, through the two helpers rule 8 itself reads ([`crate::rules::mounted_path`],
/// [`crate::rules::is_runtime_socket`]) — never by subtracting cards off the slice, which would
/// make this pane's contents depend on what some other rule did about the same pod.
///
/// **A pod that has finished is on neither screen**, which is [`crate::rules::analyze`]'s own
/// line: it skips the pod rules for one, so rule 8 draws no card, and a `Succeeded` pod is
/// reading nothing off its node either. The partition below is over the mounts of pods that are
/// still running, which is exactly rule 8's subject.
///
/// **It runs unchanged when the view is scoped** — a hostPath is a pod field and needs no
/// permission Alerts does not already have — so there is no [`Row::NotComputed`] on this pane at
/// all, and only the title changes.
///
/// **Every row here is an aggregate, which is why this pane caps nothing** — one row per host
/// path, with the pod count inside it, so a DaemonSet across two hundred nodes is still one row.
/// [`MOST_ROWS_PER_SECTION`] caps Waste's rows that are one *per object* — a Service, a PVC,
/// unbounded in the cluster's object count — and caps neither of its counted rows for the same
/// reason as this: Capacity's node list is unbounded too, and `screens/analysis.md` § Capacity
/// rules that the pane scrolls rather than that the list is cut.
pub fn posture(snapshot: &ClusterSnapshot, _findings: &[Finding]) -> Report {
    // Rule 6: a title names a namespace only where there is one. The dangerous state is the
    // narrow one, so it is the labelled one.
    let title = match snapshot.namespace_scope.as_deref() {
        Some(namespace) => format!("Pods in {namespace} that can read the node's own filesystem"),
        None => "Pods that can read the node's own filesystem".to_string(),
    };
    let mounted = host_paths(snapshot);
    if mounted.is_empty() {
        // Rule 8's empty state, in this report's own words — and **the opening paragraph is not
        // drawn beside it**: *"the list says who can"* over no list at all is a sentence about
        // nothing.
        return Report {
            title,
            badge: None,
            rows: vec![Row::Prose(
                "Nothing here mounts a path from the node it runs on. That is rarer than it \
                 sounds — most clusters run a network or storage agent that does."
                    .to_string(),
            )],
        };
    }

    let mut paths: Vec<(&String, &Mounters)> = mounted.iter().collect();
    // **A row with a pod the check does not clear sorts above every row without one**
    // ([`Mounters::outside_kube_system`]), and inside each of the two groups the key is
    // unchanged: most widely mounted first, then the path — Capacity's and Drain safety's order
    // (`screens/analysis.md` § Capacity, *Many nodes*) for the reason that applies here too: how
    // widely a path is exposed is the review this pane is for, and the alternative puts it below
    // the fold on the cluster that has most of it. A tie is not a coin flip in either group, so a
    // re-render of an unchanged cluster never reorders the pane.
    //
    // **The group has to come first, because the pod count is what buries the row worth
    // looking at**: a pod the check clears mounts its paths on every node it runs on, so one
    // pod reading one directory sorts last of fourteen behind them on the committed corpus
    // (`screens/analysis.md` § Posture). A row leaves the cleared group the moment one
    // contributing pod fails the check, whatever else mounts the same path — not because that
    // pod is guilty of anything, but because it is the one thing on the row the check cannot
    // clear (NOTES § D70).
    paths.sort_by(|a, b| {
        b.1.outside_kube_system
            .cmp(&a.1.outside_kube_system)
            .then_with(|| b.1.pods.cmp(&a.1.pods))
            .then_with(|| a.0.cmp(b.0))
    });

    // **The opening paragraph is part of the report**, not a caption `views.rs` adds. Without it
    // the pane reads as an accusation, and every row on it is something the cluster is supposed
    // to have (`screens/analysis.md` § Posture).
    //
    // **It stops asserting *nothing here is broken* when at least one pod on the pane runs
    // outside `kube-system`.** NOTES § D2 still keeps a plain read-only hostPath off Alerts —
    // this is not a reversal of it, the row stays `○`/`Info` and the pane still badges nothing —
    // but a pane that opens by saying nothing is broken while holding a row it cannot vouch for
    // is telling two stories at once.
    //
    // **The subject is the pod and not the row**, which is the whole of what the check answers:
    // the flag is true when *any one* contributor fails it, so a top row of three pods can be
    // two the check cleared and one it did not, and a sentence about *the row* would be false of
    // two thirds of it (`screens/analysis.md` § Posture).
    //
    // **Read off the top of the sorted list and not counted a second time**: the sort above has
    // just put such a row there, so *"the top row"* is that same fact rather than a second answer
    // to it. The wording names no proportion on purpose — an ordinary app namespace has no pods
    // in `kube-system` at all, so a scoped view is routinely *every* row and the sentence has to
    // stay true of that render too.
    let opening = if paths
        .first()
        .is_some_and(|(_, who)| who.outside_kube_system)
    {
        "Network, storage and metrics agents are supposed to do this. The top row has a pod \
         outside kube-system, so k8rs cannot tell what it is. Nothing is marked broken; it still \
         says who can, not what to go and fix."
    } else {
        "Nothing here is broken. Network, storage and metrics agents are supposed to do this — \
         the list says who can, not what to go and fix."
    };
    let mut rows = vec![Row::Prose(opening.to_string())];
    rows.extend(paths.into_iter().map(|(path, who)| Row::Answer {
        // **`Info` on every row, always.** The pane makes no judgement; a band that varied would
        // be one (`screens/analysis.md` § Posture).
        severity: Some(Severity::Info),
        text: path.clone(),
        detail: vec![who.sentence()],
        // Nothing to do, which is the pane's whole point.
        action: String::new(),
        // **A row stands for a set of pods, so it records no destination** — [`Jump`] has a case
        // for one object and one for one finding, and none for a set (NOTES § D128).
        jump: None,
    }));
    Report {
        title,
        // **No badge, ever.** A permanent number beside `posture` would nag about a list that is
        // correct (`screens/analysis.md` § Posture, [`Report::badge`]).
        badge: None,
        rows,
    }
}

/// Who mounts one host path — **counted per pod and not per mount**, so a pod that mounts one
/// directory into three of its containers is one pod here, and a DaemonSet across two hundred
/// nodes is two hundred pods and still one row.
#[derive(Default)]
struct Mounters {
    pods: usize,
    /// Every namespace a mounting pod sits in, deduplicated and in the order `kubectl get -A`
    /// prints them. A pod without a namespace cannot exist in the API, so nothing is invented
    /// for one that decoded without.
    namespaces: BTreeSet<String>,
    /// Whether **any** mount of this path is writable. Every writable mount that reaches this
    /// pane is one rule 8 stayed silent about — node infrastructure in `kube-system` — and a
    /// path that is read-only for nine pods and writable for one is not a read-only path.
    ///
    /// **Which is only true because [`host_paths`] drops the pod that has both**: this is `or`-ed
    /// over the mounts rule 8 left behind, so a pod mounting one path read-only *and* writably
    /// outside the node infrastructure would have contributed the read-only half alone and the
    /// sentence would have said *Read-only* about a path that pod can write. That pod is on
    /// Alerts, with rule 8's card, and is not counted here at all.
    writable: bool,
    /// Whether **any** pod behind this row fails [`node_agent`]'s check — runs outside
    /// `kube-system`, or inside it without being a DaemonSet or a mirror pod.
    ///
    /// **Named for the observable and not for a verdict.** k8rs cannot say what such a pod is:
    /// it could be a workload reading a path it has no reason to, or exactly the kind of agent
    /// every other row here is, installed somewhere the one check this pane runs does not look —
    /// Rook, Longhorn, Cilium and every CSI node plugin are that second case (NOTES § D70,
    /// `screens/analysis.md` § Posture). Every string keyed on this field says so.
    ///
    /// **It is the row's group in the sort and the pane's opening paragraph**, and it is a
    /// binary and not a proportion: the pane draws no third sentence for *how many* of a row's
    /// pods failed, because *at least one* is what an operator acts on and all the check knows.
    ///
    /// **A pod rule 8 escalated is not one of these** — it contributes nothing to this row at
    /// all, and a row does not change group because some other pod's mount went to Alerts.
    outside_kube_system: bool,
}

impl Mounters {
    /// **How many pods, in which namespaces, whether any of them can write and whether any runs
    /// outside `kube-system`** — the sentence under a Posture row.
    ///
    /// **Up to three namespaces, then `and N more`** (`screens/analysis.md` § Posture): which
    /// namespaces can read a path is the half an operator acts on, and a list of every one of
    /// them is the half that makes the pane unreadable.
    fn sentence(&self) -> String {
        let pods = if self.pods == 1 {
            "1 pod".to_string()
        } else {
            format!("{} pods", self.pods)
        };
        let named: Vec<&str> = self
            .namespaces
            .iter()
            .take(NAMESPACES_NAMED)
            .map(String::as_str)
            .collect();
        let over = self.namespaces.len() - named.len();
        let places = if named.is_empty() {
            String::new()
        } else {
            format!(" in {}", and_list(&named, over))
        };
        match (self.writable, self.outside_kube_system, self.pods) {
            (false, false, _) => format!("Read-only, mounted by {pods}{places}."),
            // **The reorder alone is not legible** — *near the top* means nothing to a reader
            // who does not know the sort key — so the row's own sentence says what the check
            // found, and never more than that (`screens/analysis.md` § Posture). The em dash is
            // the whole reason this arm exists separately: a `which` clause would bind to the
            // namespace beside it and say `default` is outside `kube-system`.
            (false, true, 1) => format!(
                "Read-only, mounted by {pods}{places} — outside kube-system, so k8rs cannot tell \
                 what it is."
            ),
            // *At least one* is true whether one pod of three failed the check or all three did,
            // and the pane draws no third sentence for the difference.
            (false, true, _) => format!(
                "Read-only, mounted by {pods}{places}. At least one of them is outside \
                 kube-system, so k8rs cannot tell what it is."
            ),
            // **Writable and still not an alarm**, which needs saying or the row reads as one
            // rule 8 missed: the only writable mounts that reach this pane are `kube-system`'s
            // own, and that silence is rule 8's on purpose (NOTES § D70).
            (true, false, 1) => format!(
                "Mounted by {pods}{places}, which can write to it. Kubernetes runs its own node \
                 agents this way."
            ),
            (true, false, _) => format!(
                "Mounted by {pods}{places}, and at least one of them can write to it. Kubernetes \
                 runs its own node agents this way."
            ),
            // **Unreachable today, and it still gets a sentence that is true either way.** It is
            // unreachable *because* of the `kube-system` clause NOTES § D70 records as too
            // narrow: whoever widens that clause makes this row buildable, and an arm that fell
            // through to the one above it would tell a reader that a pod in `longhorn-system` is
            // one of the node's own agents — in a release build, where the assertion in
            // [`host_paths`] is compiled out and nothing else would catch it.
            (true, true, 1) => format!(
                "Mounted by {pods}{places}, which can write to it. That pod is outside \
                 kube-system, so k8rs cannot tell what it is."
            ),
            // **The reassurance clause, claiming only what the code checked.** Every writer here
            // did clear the check, but the sentence cannot point at *that one* the way the
            // single-pod row does — two DaemonSets can write to one path — so it names the
            // writers as a group and then says plainly that the row holds more than them.
            (true, true, _) => format!(
                "Mounted by {pods}{places}, and at least one of them can write to it. The ones \
                 that write are in kube-system; not every pod here is."
            ),
        }
    }
}

/// **How many namespaces a Posture row names before it stops naming them** — three,
/// `screens/analysis.md` § Posture's own number, and a readability budget rather than a measured
/// one: *which* namespaces can read a path is the half an operator acts on, and every one of them
/// is the half that makes the pane unreadable.
const NAMESPACES_NAMED: usize = 3;

/// `a` · `a and b` · `a, b and c` · `a, b, c and 2 more` — a list inside a sentence.
///
/// **`over` is what was left off, not the total**, and `0` leaves the tail out altogether.
/// `rules.rs` spells a list of its own for N1's evidence and stops at two; this one stops at
/// [`NAMESPACES_NAMED`], so they are two sentences with two budgets rather than one function
/// asked to hold both.
fn and_list(named: &[&str], over: usize) -> String {
    let tail = (over > 0).then(|| format!("{over} more"));
    let parts: Vec<&str> = named.iter().copied().chain(tail.as_deref()).collect();
    match parts.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_string(),
        Some((last, head)) => format!("{} and {last}", head.join(", ")),
    }
}

/// **Every host path this cluster mounts that rule 8 does not draw a card about**, keyed by the
/// path the container actually receives — bar one, the path that has no name to key on.
///
/// The key is [`crate::rules::mounted_path`]'s answer and nothing else — the same string rule 8
/// compares, normalised the same way. A second normaliser for `..`, a repeated separator or a
/// trailing `/` would be a second answer to *is this the same path* (NOTES § D71).
fn host_paths(snapshot: &ClusterSnapshot) -> BTreeMap<String, Mounters> {
    let mut paths: BTreeMap<String, Mounters> = BTreeMap::new();
    for pod in snapshot.pods.iter().filter(|pod| !finished(pod)) {
        // **Deduplicated inside the pod first, and the partition is per (pod, path)** — not per
        // mount, which is where it was wrong. Two containers mounting one directory are one pod
        // that can read it, and counting the mounts would make a sidecar look like a second
        // reader; the writable bit is `or`-ed, because one writable container is enough.
        //
        // `escalated` is the half [`left_by_rule_8`] cannot answer on its own: a pod that mounts
        // one path *twice*, read-only in one container and writable in another outside the node
        // infrastructure, has one mount on Alerts and one here — so rule 8's card says *writable*
        // while this pane says *Read-only, mounted by 1 pod*, about one pod and one directory
        // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 6). The pod is
        // already answered for on the other screen, so it contributes nothing to this one.
        let mut here: BTreeMap<String, (bool, bool)> = BTreeMap::new();
        for mount in &pod.host_path_mounts {
            let path = mounted_path(mount);
            // **A path that normalises to nothing draws no row.** `hostPath: {path: "."}` is
            // the shape, and it reaches [`crate::rules::mounted_path`] as a relative path that
            // empties out — a row whose text is the empty string is a blank line with a
            // sentence indented under it, which reads as a defect rather than as an answer.
            if path.is_empty() {
                continue;
            }
            let (escalated, writable) = here.entry(path).or_default();
            if left_by_rule_8(pod, mount) {
                *writable |= !mount.read_only;
            } else {
                *escalated = true;
            }
        }
        for (path, (escalated, writable)) in here {
            if escalated {
                continue;
            }
            let who = paths.entry(path).or_default();
            who.pods += 1;
            who.writable |= writable;
            // **Per contributing pod, so one is enough** — the pod the check cannot clear is
            // the whole reason to look at the row (`screens/analysis.md` § Posture).
            who.outside_kube_system |= !node_agent(pod);
            if let Some(namespace) = pod.id.namespace.as_deref() {
                who.namespaces.insert(namespace.to_string());
            }
        }
    }
    // **No row this pane builds is writable, single-pod and outside `kube-system` at once**: that
    // pod is the row's only contributor and it wrote, so [`left_by_rule_8`] let a non-`read_only`
    // mount through, which it does only when [`node_agent`] held. The claim is asserted where the
    // row is built rather than where it is worded, which leaves [`Mounters::sentence`] a plain
    // formatter its tests can call for every arm — including the one this forbids
    // (`screens/analysis.md` § Posture).
    debug_assert!(
        !paths
            .values()
            .any(|who| who.writable && who.outside_kube_system && who.pods == 1),
        "a lone writable pod outside kube-system is rule 8's card, not a row here"
    );
    paths
}

/// **Is this mount the one rule 8 leaves behind?** — the exact complement of rule 8's three
/// escalators, asked from this side.
///
/// The two that are about *what* is mounted are [`crate::rules::mounted_path`]'s and
/// [`crate::rules::is_runtime_socket`]'s, called rather than re-read. The third is rule 8's
/// silence over node infrastructure, which is the one clause of it that has no exported reader —
/// so this file spells it, once, in [`node_agent`].
///
/// **A second spelling of a predicate is what this project pays most for**, so the guard is not
/// this comment: the test beside this asserts, over every captured mount, that rule 8's cards and
/// this pane's rows partition the list with neither an overlap nor a gap. A clause that drifted
/// apart from rule 8's shows up there rather than on a screen.
fn left_by_rule_8(pod: &PodSnapshot, mount: &HostPathMount) -> bool {
    let path = mounted_path(mount);
    path != "/" && !is_runtime_socket(&path) && (mount.read_only || node_agent(pod))
}

/// **Does this pod run in `kube-system` as a DaemonSet or a mirror pod?** — rule 8's third
/// escalator, asked from this side. The mirror half is why `etcd` and `kube-apiserver` clear it
/// ([`PodSnapshot::mirror`]).
///
/// **It is a check and not a verdict, and the name is rule 8's use of it rather than what it
/// proves.** Rook, Longhorn, Cilium, node-exporter and every CSI node plugin are real node agents
/// that fail it, because none of them install into `kube-system` (NOTES § D70). Nothing this file
/// prints may claim more than the check itself — which is why every string keyed on
/// [`Mounters::outside_kube_system`] names the namespace and then says k8rs cannot tell
/// (`screens/analysis.md` § Posture).
///
/// **One spelling, read by both of this pane's questions**: which mounts rule 8 leaves here
/// ([`left_by_rule_8`]) and which rows have a pod that fails it. Product code holds two copies of
/// the clause, this one and `rules.rs`'s, which exports no reader for it; the third is in the
/// test beside this, which spells the requirement out rather than calling the code it checks.
fn node_agent(pod: &PodSnapshot) -> bool {
    pod.id.namespace.as_deref() == Some(NODE_NAMESPACE)
        && (pod.mirror || pod.owner.kind == ObjectKind::DaemonSet)
}

// --- THE POSTURE REPORT END ---

// --- THE RESTARTS REPORT START ---

/// **The containers that are serving right now and keep dying** — the hole rules 1, 2, 5 and 6
/// leave visible (NOTES § D101, `screens/analysis.md` § Restarts).
///
/// **A row and never a card.** A container that is serving at the instant the snapshot was taken
/// has rules 2, 5 and 6 stood down on it by [`doing_its_job`] — the three that share it as a
/// suppressor — and rule 1 never reaches it at all, because a `CrashLoopBackOff` reason is only
/// readable off a [`ContainerState::Waiting`] container. So Alerts says nothing until the next
/// restart. A point sample cannot tell a container still cycling from one that hiccuped once
/// a month ago, so this pane prints two facts and asserts nothing: how many times, and how long
/// the run it is in has lasted.
///
/// **This pane and rule 5's cards overlap by construction, and the overlap is the first
/// `NOT_READY_GRACE` after every restart.** Rule 5 stands down on a serving container only once
/// its *current run* is older than ten minutes (`restarting_repeatedly`); this pane qualifies it
/// the moment it is serving. So a container that restarted two minutes ago is on both screens at
/// once, measured on a real cluster: `default/cycler` carried *"Container has been restarted 8
/// times — it is serving now, but something keeps killing it"* on Alerts while this pane drew a
/// row for it. The committed corpus shows none of that only because its five runs are all past
/// the grace — a property of five pinned timestamps, not of a cluster, and not something any
/// sentence on this pane may lean on. **That is what the opening paragraph is worded around**,
/// and it is why `_findings` stays unread rather than subtracted: nothing here depends on the two
/// sets being disjoint, because they are not.
///
/// **Both numbers, never divided** (PRIOR-ART § F2). They sit one under the other in `detail`, one
/// paragraph each, and are never combined into a rate, never summed across a workload's pods —
/// whose count and age would be two different domains — and never grouped on
/// [`crate::rules::PodSnapshot::owner`], which is the ReplicaSet until Phase 5 and loses the count
/// on every deploy.
///
/// **The run's age comes from `state.running.started_at` and not from
/// [`crate::rules::ContainerSnapshot::last_terminated`]**: the two synthesized `137`s of a gang
/// restart leave `finished_at` null, so the field that looks equivalent is the one that is absent
/// on the shape this pane most exists for (NOTES § D100).
///
/// **A count is not always evidence about the container the row names, and this pane cannot say
/// so.** `broken-gang`'s `bystander` never failed: the pod's `RestartAllContainers` rule restarted
/// it because a *sibling* exited, and its count went up for its neighbour's fault. Saying that
/// would mean naming the ending, which is the line below — so the row prints a number that is true
/// and an explanation that is not available. Neither NOTES § D101's cost list nor this doc named
/// it before 2026-08-22, and it is exactly the shape this pane most exists for.
///
/// **What may not appear: how the run ended.** `ending` and `exit_meaning` are private to
/// `rules.rs`, and no row here spells a reason or an `exit 137` — re-spelling that translation in
/// a second file is the defect NOTES § D85 exists to prevent. There is nothing to fix in this
/// row's own words, which is what a card is for and this container has none.
///
/// **A set that qualifies and cannot yet be drawn draws nothing, and says nothing either — and
/// that state is transient.** A container in a run, healthy in it and above the threshold, but
/// whose `state.running.started_at` has not arrived — under eight seconds after a restart
/// (NOTES § D100) — or whose start sits past [`crate::rules::age`]'s future-skew allowance has no
/// second paragraph to print. The empty sentence would be false about it, so this pane keeps its
/// opening paragraph and draws neither.
///
/// **`findings` is unread on purpose, and not for Capacity's reason.** The pane does not
/// cross-check against what Alerts is currently showing: the row's claim is narrower than a
/// card's — count and age, nothing about current health — so a container appears here whether or
/// not it also carries a live card, and there is nothing to reconcile.
///
/// **No [`Row::NotComputed`], ever.** This reads pod data alone, which is permanently watched and
/// needs no permission Alerts does not already have; a namespace scope narrows the list and never
/// switches the check off (`screens/analysis.md` § Restarts).
///
/// **This pane scrolls and does not cap.** [`MOST_ROWS_PER_SECTION`] is a *per-section* budget:
/// Waste's four sections share one pane's lines, and cutting the loudest is what stops it starving
/// the other three. This pane has one section, so there is nothing left to starve — and the cap it
/// borrowed from Waste's number without Waste's reason broke on a one-node kind cluster, where
/// three node reboots took the qualifying set from 6 to 17 and the five kept slots went to five
/// containers that had stopped restarting for good, while the one still on a live ten-minute cycle
/// became the `and 1 more` line. A [`Row::Prose`] is not selectable, so a folded row is not one
/// keypress away — it is off the screen. Capacity's node list and Posture's rows already scroll
/// for the same reason (`screens/analysis.md` § Restarts).
///
/// **No badge**, for Posture's reason carried one step further: the only band this pane could
/// offer is `Info`, and the count of qualifying containers only grows — a settled restart from a
/// node reboot last month stays in the tally until its pod is replaced, so on any cluster with
/// real age the badge would read nonzero permanently.
pub fn restarts(snapshot: &ClusterSnapshot, _findings: &[Finding]) -> Report {
    // Rule 6: a title names a namespace only where there is one.
    let title = match snapshot.namespace_scope.as_deref() {
        Some(namespace) => format!("Containers in {namespace} that keep restarting"),
        None => "Containers that keep restarting".to_string(),
    };
    // **The qualifying set is decided before any age is asked for**, because the empty sentence
    // below claims that *nothing* qualifies by serving and count — and a container above the line
    // whose run has no printable age qualifies, it just cannot be drawn yet
    // (`screens/analysis.md` § Restarts).
    let qualifying = serving_and_restarting(snapshot);
    if qualifying.is_empty() {
        // **One sentence, and it has to stay true on every cluster it can be drawn on**
        // (`screens/analysis.md` § *Empty, and nothing qualifies*): nothing has restarted at all,
        // something has but stayed under the threshold, or something is above the threshold and
        // not serving — crash-looping, or `Running` and failing its readiness check — and so was
        // never in this pane's set. **So it quantifies over this producer's own filter and
        // nothing wider** — *every container serving right now*, which is [`doing_its_job`] and
        // not `Running`. `broken-probe0` sits at 13 restarts, `Running`, not ready, and carrying
        // rule 5's non-serving card — the one that fires on that shape for any role and never ages
        // out: *running right now* would have swept it into *2 or fewer*.
        //
        // **The number is computed off [`RESTARTS_WARN`] and never retyped**, so if rule 5's
        // threshold moves this sentence moves with it instead of going stale in a file nothing
        // recompiles. It is drawn as a digit, which is every count on this page
        // (`screens/analysis.md` § *Certificates and Versions*).
        //
        // **And it names the namespace, which the rows do not have to.** A row carries its scope
        // in its own `namespace/pod` prefix; this line has no row to carry it, so under a scope —
        // `--namespace`, or the 403 fallback that fills the same field — it would otherwise assert
        // something about every serving container in the cluster while the title above it says
        // one namespace. `kube-system/etcd` at forty restarts and serving is what makes that
        // false (`screens/analysis.md` § *Restarts under one namespace*).
        return Report {
            title,
            badge: None,
            rows: vec![Row::Prose(format!(
                "Nothing here has restarted enough to matter. Every container serving right \
                 now{} has restarted {} or fewer times since its pod started.",
                match snapshot.namespace_scope.as_deref() {
                    Some(namespace) => format!(" in {namespace}"),
                    None => String::new(),
                },
                RESTARTS_WARN - 1
            ))],
        };
    }
    // **What can actually be drawn** — a qualifying container whose current run has a printable
    // age. The rest keep the opening paragraph and draw no row and no claim either way: the row
    // is both numbers, and the pane draws them on the next redraw once Kubernetes reports the
    // timestamp (NOTES § D100, `screens/analysis.md` § Restarts).
    let mut cycling: Vec<Cycling<'_>> = qualifying
        .into_iter()
        .filter_map(|(pod, container)| {
            let ContainerState::Running {
                started_at: Some(started),
            } = &container.state
            else {
                return None;
            };
            Some(Cycling {
                pod,
                container,
                run: age(&snapshot.now, started)?,
                started,
            })
        })
        .collect();
    // **Worst first, and a tie no longer throws away the second number to get there.** The count
    // is the pane's subject and D101's own *worst*, so it stays primary; a tie then breaks on the
    // **younger current run** — the one that started more recently, still mid-cycle rather than
    // long settled — which this producer had already computed one line above for the row's own
    // second paragraph and was discarding at the comparator. Then `namespace/pod` as the screen
    // spells it, then the container's own name, so the two containers of one gang-restarted pod
    // come out in a fixed order rather than in whichever order the kubelet listed them.
    cycling.sort_by(|a, b| {
        b.container
            .restarts
            .cmp(&a.container.restarts)
            .then_with(|| b.started.cmp(a.started))
            .then_with(|| qualified(&a.pod.id).cmp(&qualified(&b.pod.id)))
            .then_with(|| a.container.name.cmp(&b.container.name))
    });
    // **The opening paragraph is part of the report**, not a caption `views.rs` adds: without it
    // a pane of restart counts reads as a list of things to go and fix, and every container on it
    // is serving.
    //
    // **It may not tell the reader nothing is broken.** It said so until 2026-08-22, and the
    // sentence was false for the first ten minutes after every restart — the window rule 5 is
    // still carding, and the reader most likely to open this pane is the one who just came from
    // that card. What it says instead is what the pane *is*, and which of the two numbers carries
    // the signal (`screens/analysis.md` § Restarts).
    let mut rows = vec![Row::Prose(
        "Every container below is serving right now. A restart count never clears itself — the \
         second number, how long this run has lasted, is the signal."
            .to_string(),
    )];
    rows.extend(cycling.into_iter().map(Cycling::row));
    Report {
        title,
        badge: None,
        rows,
    }
}

/// One container that qualifies, with the run age already spelled — **the age is resolved before
/// the row is built and not inside it**, because a run whose age cannot be spelled is not drawn
/// at all ([`serving_and_restarting`]).
struct Cycling<'a> {
    pod: &'a PodSnapshot,
    container: &'a ContainerSnapshot,
    /// [`crate::rules::age`]'s own string — `6 hours ago`, the one ladder every age on every
    /// screen uses.
    run: String,
    /// **The same moment `run` spells, kept as a moment** — the tie-break orders by it, and a
    /// rung of the ladder cannot be compared: `50 min ago` and `1 hour ago` sort by their first
    /// character, and every run inside one rung compares equal.
    started: &'a Time,
}

impl Cycling<'_> {
    /// **Always names the container, even where the pod has only one.**
    /// [`crate::rules::ContainerSnapshot::restarts`] is a per-container count, and *name it only
    /// when there is more than one* is a branch this producer would carry to save one word. Two
    /// qualifying containers in one pod draw two rows and never one merged one.
    fn row(self) -> Row {
        Row::Answer {
            // **`Info` on every row, always.** The pane makes no judgement — that is the whole of
            // NOTES § D101 — and a band that varied would be one.
            severity: Some(Severity::Info),
            // **The identity is [`container_fact`] verbatim, gloss and all** — never a second
            // wording of a role, which [`crate::rules::ContainerRole`]'s own doc calls wrong
            // rather than unclear. **And the gloss is why the numbers are not on this line**: a
            // reader asked to parse *"(it runs beside the app the whole time) restarted 9
            // times"* in one breath loses the count, so the row is built around the gloss and
            // both numbers move down (`screens/analysis.md` § Restarts).
            text: format!(
                "{} · {}",
                qualified(&self.pod.id),
                container_fact(self.container)
            ),
            // **Two paragraphs, one clock each, and the words keep them apart on purpose.**
            // *this pod* answers for the count — `restarts` resets to 0 on a new pod, so without
            // that qualifier a reader could take it as the container's whole history and read a
            // young pod as calm. *this run* answers for the run, which began at the last restart
            // and not at the pod's creation.
            detail: vec![
                format!(
                    "Restarted {} times since this pod started.",
                    self.container.restarts
                ),
                format!("This run started {}.", self.run),
            ],
            // Nothing to do — the container is serving.
            action: String::new(),
            // **To the pod, and there is no finding to go to** — which is the whole reason the
            // row exists ([`Jump::Object`]'s own doc). Two qualifying containers in one pod jump
            // to the same pod, and the reader sees both from there.
            jump: Some(Jump::Object(self.pod.id.clone())),
        }
    }
}

/// **In a run right now, healthy in it, and at rule 5's own threshold** — the pane's whole filter,
/// in one place, and its three clauses answer three different questions.
///
/// **[`doing_its_job`] answers *healthy*, and `analysis.rs` never re-derives it.** It is the
/// suppressor rules 2, 5 and 6 already share, so a container that is up but failing its readiness
/// check is excluded here for the same reason those rules stand down elsewhere. `broken-probe0` at
/// 13 restarts and `broken-restarts10` at 10 are both that shape in the committed corpus.
///
/// **What makes that exclusion safe is rule 5's own non-serving branch, and not rule 7's card.**
/// `restarting_repeatedly` ages out only when the container *is* serving, so a `Running && !ready`
/// container at or above [`RESTARTS_WARN`] keeps its card permanently, whatever its role. Rule 7's
/// *"Running, but not receiving traffic"* is the extra card a **regular** container also gets:
/// `running_but_not_ready` opens `if c.role != ContainerRole::Regular { return None; }`, so a
/// native sidecar failing the identical probe — the Istio/Linkerd shape the role split exists for
/// — carries no rule 7 card at all, and only rule 5's branch still catches it. Measured on a real
/// cluster: two pods in the identical state produced one rule 7 card, and it was not the
/// sidecar's.
///
/// **[`ContainerState::Running`] answers *in a run right now*, which is a different question and
/// is what this pane's second number is.** [`doing_its_job`]'s `Init` arm answers *finished well*
/// — an init container that exited `0` is doing its job — and that container is `Terminated`, so
/// it has no current run to age and never can have one. Without this clause
/// `healthy-retry/wait-for-db` sat in the qualifying set permanently, suppressing the empty
/// sentence on a cluster it was the only member of: an opening paragraph over nothing, for ever,
/// instead of the true sentence (NOTES § D101).
///
/// **It answers for the empty sentence as well as for the rows**, which is why it stops here and
/// does not also demand an age: a container that qualifies and cannot yet be drawn must not let
/// the pane claim that nothing qualifies. With the state clause in place that gap is D100's
/// eight-second window and the skew allowance, and nothing else — transient, which is what
/// `screens/analysis.md` § Restarts says it is ([`restarts`]).
fn serving_and_restarting(snapshot: &ClusterSnapshot) -> Vec<(&PodSnapshot, &ContainerSnapshot)> {
    snapshot
        .pods
        .iter()
        .flat_map(|pod| pod.containers.iter().map(move |container| (pod, container)))
        .filter(|(_, container)| {
            matches!(container.state, ContainerState::Running { .. })
                && doing_its_job(container)
                && container.restarts >= RESTARTS_WARN
        })
        .collect()
}

// --- THE RESTARTS REPORT END ---

// --- THE VERSIONS REPORT START ---

/// **The control plane's version, and every machine too far behind it to be supported**
/// (`screens/analysis.md` § *Certificates and Versions*, NOTES § N-series).
///
/// **N4 is called, never re-derived** ([`crate::rules::kubelet_too_far_behind`]). The rule is
/// `Info` and [`crate::rules::analyze`] does not return it, so there is no card on the `findings`
/// slice to pick up — this is the consumer it was written for and could not reach until D129
/// opened the door. The window is **three** minor versions, upstream's own and the rule's own
/// constant: NOTES § D81 corrected the N-series on it, and the mockup that said two flagged a
/// healthy cluster mid-upgrade.
///
/// **Two reads, and they fail separately.** The control-plane line comes from
/// [`crate::rules::ClusterSnapshot::server_version`] and stands on its own; the comparison needs
/// the node list as well. So a login that cannot list nodes still sees which version the control
/// plane is, and only the kubelet half says it could not run (`screens/analysis.md` § *What each
/// report needs*).
///
/// **A namespace scope changes nothing here**, unlike Capacity and Drain safety: this report
/// joins no pods, and a node object read under a narrow view is the same node object. So the
/// title names no namespace — nodes are cluster-scoped, and a namespace on this heading would be
/// a claim about a scope the answer does not have.
pub fn versions(snapshot: &ClusterSnapshot, _findings: &[Finding]) -> Report {
    let title = "What version everything here is running".to_string();
    // **The pane's own heading, and this report emits it because nothing else can.** Two reports
    // share the Certificates pane, so `views.rs` draws that pane's heading from the first report's
    // `title` and this one's title is simply not drawn — the count of reports and the count of
    // panes are two facts ([`Report`]). `screens/analysis.md` § *How a report is drawn* assigns
    // the literal `Versions` at the foot of that pane to a [`Row::Prose`], and the only other way
    // to reach it is a per-report string hard-coded in `views.rs`, which is what [`Report::rows`]
    // refuses for the empty state for this reason. Read and never selected, like every `Prose`;
    // the title above it stays the plain-language sentence (invariant 14).
    let heading = Row::Prose("Versions".to_string());
    let Some(server) = snapshot.server_version.as_deref() else {
        // **The widest cause wins and it is the only row** (`screens/analysis.md` rule 7): with
        // no control-plane version there is neither a line to draw nor anything to compare a
        // kubelet against, and N4 says nothing rather than comparing against a guess.
        return Report {
            title,
            badge: None,
            rows: vec![
                heading,
                Row::NotComputed {
                    reason: "Not checked. Every answer on this pane is measured against the \
                             version the control plane is running, and k8rs could not read it."
                        .to_string(),
                    ask_for: "Check that the cluster's API server is answering — this is the \
                              one number it tells anyone who can reach it."
                        .to_string(),
                },
            ],
        };
    };

    // **The control-plane line is a [`Row::Prose`]**, which is the question `Row::Prose`'s own
    // doc left to this box. It is read and never selected: there is no object behind *the
    // control plane* that `⏎` could open — the API server is not in the node list on a managed
    // cluster and is a mirror pod on a self-hosted one — and a row the cursor lands on that
    // opens nothing is the key that does nothing this screen refuses to draw (NOTES § D127,
    // § D128). It carries no band for the same reason: nothing here is a judgement.
    let mut rows = vec![
        heading,
        Row::Prose(control_plane_line(server, &snapshot.nodes)),
    ];
    if snapshot.nodes.is_empty() {
        // **The line above stays**, which is this report's whole difference from Capacity's
        // empty node list: one read failed and the other did not.
        rows.push(Row::NotComputed {
            reason: "Which machines are behind is not checked. That needs the list of nodes, and \
                     this login cannot read it."
                .to_string(),
            ask_for: "Ask for permission to list nodes across the whole cluster.".to_string(),
        });
        return Report {
            title,
            badge: None,
            rows,
        };
    }

    let mut behind: Vec<BehindLine> = snapshot
        .nodes
        .iter()
        .filter_map(|node| behind_row(server, node))
        .collect();
    // **Furthest behind first, then node name** — the pane's order everywhere else
    // (`screens/analysis.md` § Capacity, *Many nodes*): the machine that has to be upgraded first
    // is the one that must not be below the fold.
    behind.sort_by(|a, b| b.gap.cmp(&a.gap).then_with(|| a.name.cmp(b.name)));

    let nothing_to_say = behind.is_empty();
    rows.extend(behind.into_iter().map(|line| line.row));
    if nothing_to_say {
        rows.push(Row::Prose(nothing_to_do(server, &snapshot.nodes)));
    }

    Report {
        title,
        // **No badge.** The one badge this pane carries is `certificates`', and it is C1's — the
        // sidebar has a `versions` entry of its own and every mockup on `screens/analysis.md`
        // draws it bare.
        badge: None,
        rows,
    }
}

/// One machine's line: the row, and the distance the pane is ordered by. `gap` is not
/// recoverable from the row once the row is a string, which is [`DrainLine`]'s reason too.
struct BehindLine<'a> {
    gap: u32,
    name: &'a str,
    row: Row,
}

/// **One machine's row, or nothing at all** — and N4 is the gate, never a comparison rewritten
/// here ([`crate::rules::kubelet_too_far_behind`], NOTES § D46).
///
/// **[`versions_behind`] is a strict weakening of that rule and cannot disagree with it**: the
/// rule answers `Some` only after the same two parses, the same major check and the same
/// subtraction, and then asks one more question. So the `?` below drops no row the rule flagged
/// — it says the distance is there to be printed whenever the card is there to print it.
fn behind_row<'a>(server: &str, node: &'a NodeSnapshot) -> Option<BehindLine<'a>> {
    let finding = kubelet_too_far_behind(Some(server), node)?;
    let gap = versions_behind(server, node)?;
    let kubelet = node.kubelet_version.as_deref()?;
    Some(BehindLine {
        gap,
        name: node.id.name.as_str(),
        row: Row::Answer {
            // **The band is the pane's, and the rule's `Info` is its routing** — the same reading
            // Capacity's node row already lands on for N5 (NOTES § D87): `Severity::Info` on a
            // `Finding` means *this lives in a report, not in Alerts*, and once it is in the
            // report the band says how loud the row is. `screens/analysis.md` draws this row `▲`.
            severity: Some(Severity::Warn),
            text: format!("{} runs kubelet {kubelet}", node.id.name),
            detail: vec![format!(
                "{} behind the control plane, which is further back than Kubernetes supports.",
                releases(gap)
            )],
            // **N4's own way out, not a second one written here.** A row and the rule behind it
            // telling a reader to do two different things is the divergence NOTES § D46 is
            // about, and the rule's sentence is the one that cites upstream's window.
            action: finding.action,
            // **A jump is navigation and never reaches an operation** ([`Jump::Object`]).
            jump: Some(Jump::Object(node.id.clone())),
        },
    })
}

/// **`Control plane 1.34 · 2 of 3 kubelets match`** — the line that stands on its own, whatever
/// the node list did.
///
/// **The version is printed as the API server wrote it**, never re-spelled from the
/// `(major, minor)` [`crate::rules::minor_version`] parses out: a `v1.34.2+k3s1` printed back as
/// `1.34` is a number the reader cannot find in their own `kubectl version` output.
///
/// **`N of M` is drawn only when every machine was measured, and that is the denominator fix.**
/// `3 of 4 kubelets match` beside *"it could not work out how far behind some of these machines
/// are"* is two claims about the same fourth node — the first counts it as a non-match, the
/// second says nothing is known about it (`screens/analysis.md` § *Certificates and Versions*).
/// With one or more unmeasured the line separates the two facts instead of folding an unknown
/// into a non-match, and the unmeasured count is [`kubelet_minors`]' own: a machine missing from
/// there was not compared at all.
///
/// **An empty node list draws the first half alone** rather than `0 of 0`, which reads as an
/// answer when it is the absence of one — the [`Row::NotComputed`] beside it is where that is
/// said.
fn control_plane_line(server: &str, nodes: &[NodeSnapshot]) -> String {
    let Some((_, server_minor)) = minor_version(server) else {
        // Nothing was compared, so nothing is counted — the version string is still printed,
        // because it is what the reader's own `kubectl version` shows.
        return format!("Control plane {server}");
    };
    let measured = kubelet_minors(server, nodes);
    let unmeasured = nodes.len() - measured.len();
    let matching = measured
        .into_iter()
        .filter(|minor| *minor == server_minor)
        .count();
    match (nodes.len(), unmeasured) {
        (0, _) => format!("Control plane {server}"),
        // **The one-node cluster is not a rounding case, it is who this tool is for** — kind,
        // minikube, k3s, Docker Desktop — and *1 of 1 kubelets match* is the line a beginner
        // reads twice (invariant 14).
        (1, 0) if matching == 1 => {
            format!("Control plane {server} · its kubelet is the same version")
        }
        (1, 0) => format!("Control plane {server} · its kubelet is a different version"),
        (1, _) => format!("Control plane {server} · its kubelet could not be checked"),
        (total, 0) => format!("Control plane {server} · {matching} of {total} kubelets match"),
        (_, unmeasured) => format!(
            "Control plane {server} · {}, {unmeasured} could not be checked",
            if matching == 1 {
                "1 kubelet matches".to_string()
            } else {
                format!("{matching} kubelets match")
            }
        ),
    }
}

/// **What k8rs could actually measure** — one minor version per machine it could compare against
/// this control plane at all: both version strings parsed, and the same major, which is the pair
/// [`crate::rules::kubelet_too_far_behind`] needs before it can answer anything.
///
/// **A machine missing from here was not checked, and both readers turn on that.**
/// [`control_plane_line`] counts how many of them *match*, and [`nothing_to_do`] asks whether the
/// list is as long as the node list before it says anything about every machine. A node whose
/// version cannot be read is not counted as matching and is not counted as fine either — the same
/// direction every unreadable number takes on this screen (`analysis.rs` § Capacity, the node row
/// that keeps its line and its `could not be worked out`).
fn kubelet_minors(server: &str, nodes: &[NodeSnapshot]) -> Vec<u32> {
    let Some((server_major, _)) = minor_version(server) else {
        return Vec::new();
    };
    nodes
        .iter()
        .filter_map(|node| minor_version(node.kubelet_version.as_deref()?))
        .filter(|(major, _)| *major == server_major)
        .map(|(_, minor)| minor)
        .collect()
}

/// **The sentence a pane that flagged nobody closes on** — rule 8, in this report's own words,
/// and **four of them rather than one**.
///
/// *Every kubelet matches* is false on a cluster mid-upgrade whose kubelets are one release back
/// and perfectly supported, which is the state NOTES § D81 says the old drawing got wrong. And
/// *every machine is inside the window* is false the moment one machine could not be measured at
/// all: nothing is known about it, so the pane may not fold it into a sentence that says nothing
/// is wrong.
///
/// **The last two are one cause each, and folding them was a sentence that lied** — the reason
/// there are four. A kubelet on another major is *read* and not *compared*
/// ([`crate::rules::kubelet_too_far_behind`] refuses to measure across one), and a control-plane
/// version this file cannot parse is printed on the line above by [`control_plane_line`]: telling
/// either reader that k8rs *could not read the version* is a record that says the wrong thing
/// about why, which is invariant 4 in the small.
fn nothing_to_do(server: &str, nodes: &[NodeSnapshot]) -> String {
    let Some((_, server_minor)) = minor_version(server) else {
        return NOTHING_COMPARABLE.to_string();
    };
    let minors = kubelet_minors(server, nodes);
    if minors.len() < nodes.len() {
        SOME_UNMEASURED.to_string()
    } else if minors.iter().all(|minor| *minor == server_minor) {
        "Every machine is running the same version as the control plane. Nothing to do.".to_string()
    } else {
        "Every machine is inside the window Kubernetes supports. Nothing to do.".to_string()
    }
}

/// **When some machine could not be measured against the control plane** — its kubelet version is
/// missing, or does not start with two numbers, or is on another major, which
/// [`crate::rules::kubelet_too_far_behind`] does not compare across. One sentence for the three
/// because the reader does the same thing about all of them, and it names the comparison rather
/// than the read: two of the three shapes were read perfectly well.
const SOME_UNMEASURED: &str = "Nothing k8rs could measure is outside the window Kubernetes \
                               supports. It could not work out how far behind some of these \
                               machines are.";

/// **When the control plane's own version is the thing that cannot be compared against** — the
/// cause is one machine's on the line above and every machine's here, so it is its own sentence.
/// It does not say the version could not be *read*: [`control_plane_line`] prints it one row up.
const NOTHING_COMPARABLE: &str = "Nothing here could be measured. The version the control plane \
                                  reported is not written in a way k8rs can compare against, so \
                                  how far behind each machine is could not be worked out.";

/// **How many minor versions this node's kubelet is behind the control plane**, or `None` when
/// either side cannot be read or the kubelet is not behind at all.
///
/// The same two comparisons [`crate::rules::kubelet_too_far_behind`] makes, and it is only ever
/// asked about a node that rule has already flagged — so the arithmetic here decides the wording
/// of a row, never whether the row exists.
fn versions_behind(server: &str, node: &NodeSnapshot) -> Option<u32> {
    let (server_major, server_minor) = minor_version(server)?;
    let (major, minor) = minor_version(node.kubelet_version.as_deref()?)?;
    (major == server_major).then_some(())?;
    server_minor.checked_sub(minor)
}

/// `1 release` · `4 releases` — a distance in words rather than a number with an `(s)` after it
/// (invariant 14).
fn releases(count: u32) -> String {
    if count == 1 {
        "1 release".to_string()
    } else {
        format!("{count} releases")
    }
}

// --- THE VERSIONS REPORT END ---

// --- THE CERTIFICATES REPORT START ---

/// **What expires, soonest first** (`screens/analysis.md` § *Certificates and Versions*,
/// NOTES § C-series).
///
/// **C1 comes out of the `findings` slice by identity and never by title.** It is
/// `object.kind == ObjectKind::Other("kubeconfig")` — the one identity in the product with no
/// API object behind it and the only `None` uid (`rules.rs` § the certificate rules). A
/// [`Finding::title`] is a plain-language sentence, so the next invariant-14 pass rewords it and
/// a match on one stops matching with nothing red: the row keeps drawing and quietly loses its
/// `⏎` (this module's own doc).
///
/// **It is the one row on this pane whose `⏎` is a [`Jump::Finding`]** — a rule already answered
/// this, and the report is restating it. Every other row here stands for a set or for nothing the
/// reader can open.
///
/// **C2 draws no row and the screen is not changed for it** (NOTES § D129): the API server's own
/// serving certificate is the peer certificate of a TLS handshake, kube-rs does not expose it,
/// and reaching it needs a second outbound connection — a Security gate question before it is a
/// snapshot field. It is a Phase 5 box.
///
/// **The badge is C1's value and C1's band** ([`expiry_badge`]) — the sidebar's only route to a
/// reader who has not opened this pane, and the expiring band's only route anywhere, because it
/// never reaches Alerts (NOTES § D87).
///
/// **C3's row is one [`Row::NotComputed`] through the whole of Phase 4**, because
/// [`crate::rules::ClusterSnapshot::certificate_requests`] is `None` until Phase 5 fetches it —
/// and `list certificatesigningrequests` is a cluster-scoped verb most namespaced roles do not
/// have, so `None` stays the ordinary answer on a real cluster afterwards.
pub fn certificates(snapshot: &ClusterSnapshot, findings: &[Finding]) -> Report {
    let title = "What expires, soonest first".to_string();
    let mut rows: Vec<Row> = c1_row(findings).into_iter().collect();
    rows.extend(kubelets_waiting_to_join(
        snapshot.certificate_requests.as_deref(),
    ));
    if rows.is_empty() {
        // Rule 8, in this report's own words, and — like Waste — **only when there is nothing
        // else at all**: a pane carrying one `NotComputed` has not established that nothing
        // expires soon.
        rows.push(Row::Prose(
            "Nothing here expires soon, and no machine is waiting to be let in.".to_string(),
        ));
    }
    Report {
        title,
        // **C1's, and only C1's** — never the worst row on the pane. The `●` CSR row beside a
        // `▲` badge is what `screens/analysis.md` draws, and a CSR section that could not be
        // checked changes this not at all: the badge is the alerting mechanism for the one
        // finding with no other home, and *did not run* is recorded by the
        // [`Row::NotComputed`] in the body, which is the only place it is ever recorded
        // ([`Report::badge`]). A badge that moved because a *different* section could not run
        // would be the sidebar carrying a reason it has no room for.
        badge: expiry_badge(findings, snapshot),
        rows,
    }
}

/// **C1's row** — the login on this machine, restated from the card the rule already drew.
///
/// **The wording is the rule's, not a second copy of it**: the row is [`Finding::title`], the
/// paragraph under it is [`Finding::evidence`] and the way out is [`Finding::action`], all
/// verbatim. A report and the rule behind it telling a reader two different things about one
/// certificate is the divergence NOTES § D46 is about, and C1's sentences already carry the date
/// and the tense.
///
/// **The band is the pane's and the rule's is its routing** (NOTES § D87): `Severity::Info` on
/// C1 means *expiring, so it lives in this report rather than in Alerts*, and once it is here the
/// band says how loud the row is — `screens/analysis.md` draws it `▲`. The expired band is
/// `Critical` on both screens, because being locked out this second is broken-now.
fn c1_row(findings: &[Finding]) -> Option<Row> {
    let finding = c1(findings)?;
    Some(Row::Answer {
        severity: Some(band(finding)),
        text: finding.title.clone(),
        detail: vec![finding.evidence.clone()],
        action: finding.action.clone(),
        jump: Some(Jump::Finding(Box::new(finding.clone()))),
    })
}

/// **C1's card off the slice, by identity** — written once because the row and the badge are two
/// spellings of one finding and may not disagree about which finding that is.
fn c1(findings: &[Finding]) -> Option<&Finding> {
    findings
        .iter()
        .find(|f| matches!(&f.object.kind, ObjectKind::Other(kind) if kind == KUBECONFIG))
}

/// **How loud C1 is on this pane**, shared by the row and the badge for the reason above.
/// `Severity::Info` on the finding is its *routing* — it means *expiring, so it lives in this
/// report rather than in Alerts* (NOTES § D87) — and once it is here the band says how loud the
/// row is.
fn band(finding: &Finding) -> Severity {
    match finding.severity {
        Severity::Critical => Severity::Critical,
        _ => Severity::Warn,
    }
}

/// **The sidebar's `certificates  30d`** — C1's own countdown, and the only route the expiring
/// band has to a reader who has not opened this pane, because it never reaches Alerts
/// (NOTES § D87).
///
/// **Only when C1 fired.** The thirty-day threshold is `CERT_EXPIRY_WARN`'s and stays the rule's:
/// this asks whether the card exists, never how far away the deadline is. No card — no
/// certificate, token or exec-plugin auth, no current context, or simply more than thirty days
/// left — is no badge, which is the ordinary state of most clusters.
///
/// **Not a second implementation of C1** (NOTES § D129's fifteenth widening): both sides call
/// [`crate::rules::expires_at`] on the same bytes and subtract the same
/// [`crate::rules::ClusterSnapshot::now`], so the two are deterministic and identical and only
/// the *spelling* differs — `22 days` in the card's sentence, `22d` in three columns of sidebar.
/// That divergence is the point: `in_days`' wording does not fit beside a twelve-character label
/// (`screens/widgets.md` § 1). What would have been a second implementation is re-parsing the PEM
/// here, and calling the rule's own parser is exactly what avoids it.
///
/// **The expired band drops the number rather than signing it.** [`crate::rules::in_days`]
/// discards the sign because the *card's sentence* carries the direction — *expired 12 days
/// ago* — and a badge has no sentence beside it, so every numeric spelling is wrong in the
/// dangerous direction: `0d` reads as *expires today*, which is *still valid*; `12d` is
/// indistinguishable from twelve days left; and `-12d` is a minus sign a beginner has to be
/// taught (invariant 14). `out` is the one thing the card says in the three columns there are.
fn expiry_badge(findings: &[Finding], snapshot: &ClusterSnapshot) -> Option<Badge> {
    let severity = band(c1(findings)?);
    let deadline = expires_at(snapshot.client_certificate.as_deref()?)?;
    // **RFC 5280 §4.1.2.5 again, and the same boundary the rule draws**: the certificate is
    // valid *through* `notAfter`, so the deadline itself is still inside the window and only
    // what is past it has run out. Six hours before it, `0d` is the honest answer — no whole
    // days left, and still valid — and it is a different fact from `out`.
    Some(Badge {
        value: if deadline < snapshot.now.0 {
            "out".to_string()
        } else {
            format!(
                "{}d",
                deadline.duration_since(snapshot.now.0).as_hours() / 24
            )
        },
        severity,
    })
}

/// **How `rules.rs` spells the one finding that is about a file on the reader's own machine.**
/// The string is the identity C1 is picked out of the slice by, and it is written here once so
/// the match is a comparison rather than four words repeated inside one.
const KUBECONFIG: &str = "kubeconfig";

/// **C3 — the machines that cannot join until a human approves them**, as one counted row, or
/// the reason there is no answer.
///
/// **Pending is the absence of a verdict**, which is why
/// [`crate::rules::CertificateRequestSnapshot`] carries the conditions rather than a
/// `pending: bool`: a request that has been approved but not yet issued is waiting on the
/// *signer* and not on a person, so **approve it** is not its way out and it is not this row.
///
/// **Only the kubelet signer.** `kubernetes.io/kube-apiserver-client-kubelet` is a node trying to
/// join; `kubernetes.io/kube-apiserver-client` is a human asking for a kubeconfig, and the row
/// says *kubelets*.
///
/// **`Some(vec![])` and `None` are two different answers** — nothing is waiting, against nobody
/// looked — and only the second draws the row that says a check did not run.
///
/// **That row names no cause, and this is the one place on the screen where that is right.**
/// `None` is *nobody fetched it* through the whole of Phase 4 and *this login may not list them*
/// on a real cluster afterwards ([`crate::rules::ClusterSnapshot::certificate_requests`]), and
/// the field cannot tell them apart — so the sentence says what is missing rather than whose
/// fault it is, and the way out is the one that works either way.
fn kubelets_waiting_to_join(requests: Option<&[CertificateRequestSnapshot]>) -> Vec<Row> {
    let Some(requests) = requests else {
        return vec![Row::NotComputed {
            reason: "Machines waiting to join are not checked. Seeing them takes a cluster-wide \
                     list of joining requests, and k8rs does not have one."
                .to_string(),
            ask_for: "Ask for permission to list certificatesigningrequests across the whole \
                      cluster."
                .to_string(),
        }];
    };
    let waiting = requests
        .iter()
        .filter(|r| r.signer_name == KUBELET_SIGNER)
        .filter(|r| {
            !r.conditions
                .iter()
                .any(|c| matches!(c.type_.as_str(), "Approved" | "Denied" | "Failed"))
        })
        .count();
    if waiting == 0 {
        return Vec::new();
    }
    let (subject, sentence) = if waiting == 1 {
        (
            "1 kubelet is waiting to be let in".to_string(),
            "A machine cannot join the cluster until someone approves its request.".to_string(),
        )
    } else {
        (
            format!("{waiting} kubelets are waiting to be let in"),
            format!(
                "{waiting} machines cannot join the cluster until someone approves their \
                 requests."
            ),
        )
    };
    vec![Row::Answer {
        // A machine that cannot join is not a risk for later, which is what puts this row above
        // C1's band on a pane C1 badges (`screens/analysis.md` § *Certificates and Versions*).
        severity: Some(Severity::Critical),
        text: subject,
        detail: vec![sentence],
        action: "approve each request once you know which machine it came from".to_string(),
        // **A counted row stands for a set, so it records no destination** (NOTES § D128).
        jump: None,
    }]
}

/// **The signer a node uses to ask to join** — `kubernetes.io/kube-apiserver-client-kubelet`. The
/// other one a cluster sees is a human asking for a kubeconfig, and this row is not about them
/// ([`crate::rules::CertificateRequestSnapshot::signer_name`]).
const KUBELET_SIGNER: &str = "kubernetes.io/kube-apiserver-client-kubelet";

// --- THE CERTIFICATES REPORT END ---

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
