use super::*;

use crate::rules::{NodeUsage, ObjectKind, WorkloadSnapshot};

use std::collections::{BTreeMap, BTreeSet};

use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::core::v1::{Node, PersistentVolumeClaim, Pod, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

// --- ONE MODULE PER REGION OF analysis.rs ---
//
// Each child below holds the tests for the `analysis.rs` region it is named after
// (invariant 11, NOTES § D91). **The product file did not split with them**, which is the half
// of that rule that matters: two producers reading one container and disagreeing is the defect
// this repo has paid most for, and a module boundary is where the second copy of a shared
// helper grows back.
//
// What stays in *this* file is what more than one of them reads — the corpus loaders, the
// plant helper, the row builders, the read-back helpers, `pane`, and the two sweeps that run
// over every producer at once. A helper copied into two modules is the divergence the split is
// not allowed to grow.

#[path = "analysis_tests/capacity.rs"]
mod capacity;

#[path = "analysis_tests/drain.rs"]
mod drain;

#[path = "analysis_tests/waste.rs"]
mod waste;

#[path = "analysis_tests/posture.rs"]
mod posture;

#[path = "analysis_tests/versions.rs"]
mod versions;

#[path = "analysis_tests/certificates.rs"]
mod certificates;

#[path = "analysis_tests/restarts.rs"]
mod restarts;

// --- THE CORPUS THIS FILE READS ---
//
// **A producer is asserted against a capture, never against a literal typed out of a mockup** —
// which is what the builders below used to be, and what each of them stops being as its report's
// box lands. The loaders are this module's own: `rules_tests` has the same three and they are
// private to it, and no `lib.rs` exists to share them through (invariant 11, NOTES § D50).
//
// **Not the whole corpus, and each report names the slice it needs.** Capacity is a per-node sum
// and a per-pod count, so what it needs is nodes, a pod list spread across them, and the four pods
// that each break a different naive version of the limits count. Drain safety adds the two
// PodDisruptionBudgets and the Deployment's pods they protect; Waste adds the four on-demand
// lists. Naming the wider set here would be a second copy of `rules_tests`' `CAPTURED_PODS` to
// keep in step.

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
}

/// A capture with one field changed on the way in — the D40 plant `rules_tests` uses, and the
/// only way to reach a shape `just fixtures` has never captured.
fn captured_pod_but(name: &str, edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    let mut object: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    edit(&mut object);
    PodSnapshot::from(object)
}

fn captured_pod(name: &str) -> PodSnapshot {
    let pod: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    PodSnapshot::from(pod)
}

/// `kubectl get -A` answers with `kind: List`, which `k8s_openapi::List<T>` refuses — it wants
/// `PodList` — so the items come out by hand.
fn captured_items<T: k8s_openapi::serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
    fixture(name)["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}.json has no items array"))
        .iter()
        .map(|v| {
            serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
        })
        .collect()
}

fn captured_nodes() -> Vec<NodeSnapshot> {
    captured_items::<Node>("nodes")
        .into_iter()
        .map(Into::into)
        .collect()
}

/// The four single-pod captures the limits row is about, plus every `kube-system` pod — which is
/// what puts pods on all four nodes and is where the DaemonSet and static pods live.
fn captured_pods() -> Vec<PodSnapshot> {
    [
        "overhead",
        "nolimits",
        "podlimit",
        "healthy",
        "healthy-podlevel",
    ]
    .iter()
    .map(|n| captured_pod(n))
    .chain(
        captured_items::<Pod>("kube-system-pods")
            .into_iter()
            .map(PodSnapshot::from),
    )
    .collect()
}

/// The two committed PodDisruptionBudgets — `broken-pdb-floor` at its floor and
/// `healthy-pdb-room` with a pod to spare (`reports/2026-08-21-family-c-corpus-drain-and-\
/// capacity.md` § 1).
fn captured_budgets() -> Vec<DisruptionBudgetSnapshot> {
    captured_items::<PodDisruptionBudget>("poddisruptionbudgets")
        .into_iter()
        .map(Into::into)
        .collect()
}

/// **One item of a captured list, with one field moved on the way in** — [`captured_pod_but`]'s
/// mechanism (NOTES § D40) for the four on-demand lists, and the only way to reach a shape
/// `just fixtures` has never captured. One helper for all four kinds, keyed on the name the
/// capture wrote, because a per-kind copy of six lines is where a second reading of a field
/// grows back.
fn captured_item_but<T, S>(fixture: &str, name: &str, edit: impl FnOnce(&mut T)) -> S
where
    T: k8s_openapi::Metadata<Ty = k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta>
        + k8s_openapi::serde::de::DeserializeOwned,
    S: From<T>,
{
    let mut object = captured_items::<T>(fixture)
        .into_iter()
        .find(|o| o.metadata().name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("{fixture}.json has no {name}"));
    edit(&mut object);
    S::from(object)
}

/// The same, for the object every drain plant is made from: the disruption controller answers
/// well inside the time a capture takes, so a photographed budget always has its status caught up
/// with its spec, and every interesting counter shape has to be planted.
fn captured_budget_but(
    name: &str,
    edit: impl FnOnce(&mut PodDisruptionBudget),
) -> DisruptionBudgetSnapshot {
    captured_item_but("poddisruptionbudgets", name, edit)
}

/// The Deployment's two pods — **the only workload in the corpus a budget row can be read off**
/// (NOTES § D130): they carry `app: healthy-deploy`, which is what `broken-pdb-floor` selects,
/// and they sit on two different nodes.
fn captured_deploy_pods() -> Vec<PodSnapshot> {
    captured_items::<Pod>("healthy-deploy-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect()
}

fn captured_services() -> Vec<ServiceSnapshot> {
    captured_items::<Service>("services")
        .into_iter()
        .map(Into::into)
        .collect()
}

fn captured_slices() -> Vec<EndpointSliceSnapshot> {
    captured_items::<EndpointSlice>("endpointslices")
        .into_iter()
        .map(Into::into)
        .collect()
}

fn captured_claims() -> Vec<ClaimSnapshot> {
    captured_items::<PersistentVolumeClaim>("persistentvolumeclaims")
        .into_iter()
        .map(Into::into)
        .collect()
}

fn captured_replica_sets() -> Vec<WorkloadSnapshot> {
    captured_items::<ReplicaSet>("healthy-replicasets")
        .into_iter()
        .map(Into::into)
        .collect()
}

/// **The pin `rules_tests` uses, for the same reason**: a moment of its own would make every
/// captured timestamp read differently on every run. Nothing in Capacity reads the clock — it is
/// here because `ClusterSnapshot` has no `Default` and `now` cannot be invented away.
fn now() -> Time {
    Time(
        "2026-08-23T00:00:00Z"
            .parse()
            .expect("the pin is a timestamp"),
    )
}

/// A snapshot with the two lists Capacity reads and nothing else fetched. Spread over, so a
/// fifteenth field on `ClusterSnapshot` cannot arrive here in silence.
fn snapshot(pods: Vec<PodSnapshot>, nodes: Vec<NodeSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        now: now(),
        pods,
        nodes,
        workloads: Vec::new(),
        server_version: None,
        context: None,
        client_certificate: None,
        namespace_scope: None,
        replica_sets: None,
        services: None,
        endpoint_slices: None,
        claims: None,
        disruption_budgets: None,
        certificate_requests: None,
        metrics: None,
    }
}

/// The whole committed corpus this file reads, cluster-wide — the snapshot Capacity is handed on
/// a cluster nobody has broken.
fn corpus() -> ClusterSnapshot {
    snapshot(captured_pods(), captured_nodes())
}

// --- NOTHING ON THIS SCREEN IS BUILT BY HAND ANY MORE ---
//
// The last pane written out of a mockup was `restarts`, and it went the way the other six did:
// its box landed and its producer replaced it. Every claim below is now read off a `Report` a
// producer built from a snapshot, so a pane and its test can no longer agree with each other
// while both disagree with the screen.
//
// **The glyphs are absent on purpose.** `● ▲ ○` belong to `theme.rs`, `→ ` and `⏎` to
// `views.rs`; what a row carries is the band and the sentence (CLAUDE.md § single point of
// change). So is the line break: a row is one line, and where it wraps is measured a layer up.
// `nothing_a_report_carries_spells_a_glyph_or_breaks_its_own_line` is the sweep that holds
// both, over every string in every report this file builds — titles and badges included.

// --- READING A ROW BACK ---
//
// A pane is asserted through these, never through the literal it was built from: what the
// test claims is that the shape carries the fact, not that a `String` round-tripped.
//
// **They answer for an `Answer` and panic on anything else**, which is now a claim and not a
// convenience: a line the cursor cannot reach has no band, no explanation and nowhere to go, so
// a test asking one of these questions about a `Prose` or a `NotComputed` is asking the wrong
// row — an unselectable line is asserted as the variant it is, never read through here.
//
// **Exhaustive on purpose, catch-all nowhere.** A `_` arm would swallow whatever variant a
// later report box adds; naming both makes it a compile error in five places at once.

fn severity_of(row: &Row) -> Option<Severity> {
    match row {
        Row::Answer { severity, .. } => *severity,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it carries no band")
        }
    }
}

fn text_of(row: &Row) -> &str {
    match row {
        Row::Answer { text, .. } => text,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it has no `text` to read back")
        }
    }
}

fn detail_of(row: &Row) -> &[String] {
    match row {
        Row::Answer { detail, .. } => detail,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it has no explanation under it")
        }
    }
}

fn action_of(row: &Row) -> &str {
    match row {
        Row::Answer { action, .. } => action,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so there is nothing to do about it")
        }
    }
}

fn jump_of(row: &Row) -> Option<&Jump> {
    match row {
        Row::Answer { jump, .. } => jump.as_ref(),
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so `⏎` never lands on it")
        }
    }
}

/// Every string a report carries, **title and badge included**. The sweeps read this rather than
/// the rows directly, so the exhaustive `match` makes a new [`Row`] variant a compile error here
/// instead of a silently unswept string (`tester` planted a `●` in `Report::title` and the
/// whole suite stayed green).
///
/// **The `Row::Answer` arm names every field and carries no `..`**, which is the other half of
/// that: under a `..` a new *field* on an existing variant compiles and goes unswept, and only a
/// new variant is an error. The two that hold no string are named and dropped — `severity` is a
/// band and `jump` an identity.
///
/// **It is a derived list, so [`CANARIES`] asserts it found something**: what this returns is only
/// ever checked for absences, and *extracted nothing* and *nothing to extract* print the same
/// green (CLAUDE.md § Tests must not lie).
fn strings_of(report: &Report) -> Vec<&str> {
    let mut out = vec![report.title.as_str()];
    out.extend(report.badge.iter().map(|badge| badge.value.as_str()));
    for row in &report.rows {
        match row {
            Row::Answer {
                severity: _,
                text,
                detail,
                action,
                jump: _,
            } => {
                out.push(text.as_str());
                out.extend(detail.iter().map(String::as_str));
                out.push(action.as_str());
            }
            Row::Prose(text) => out.push(text.as_str()),
            Row::NotComputed { reason, ask_for } => out.extend([reason.as_str(), ask_for.as_str()]),
        }
    }
    out
}

/// **The pane as a reader would see it**, for `cargo test -- --nocapture`. The glyphs belong to
/// `theme.rs` and the `→ ` to `views.rs`; they are spelled here — and only here in this file — so
/// that running it prints something a human can read, exactly as `rules_tests`' `card` does. The
/// sweep at the foot of this file reads [`strings_of`] and not this, so nothing below can smuggle
/// a glyph into a report through it.
fn pane(report: &Report) -> String {
    let mark = |severity: Option<Severity>| match severity {
        Some(Severity::Critical) => "\u{25cf} ",
        Some(Severity::Warn) => "\u{25b2} ",
        Some(Severity::Info) => "\u{25cb} ",
        None => "  ",
    };
    // **The sidebar label is `views.rs`'s and is deliberately not a field on `Report`**
    // ([`Report::title`]), so this printer cannot name the entry — it prints the badge beside it
    // and nothing else. It said `capacity` while Capacity was the only producer, which read as a
    // lie the moment a second pane was printed through it.
    // **A badge that is a count draws its band as a glyph; a badge that is a duration does not**
    // (`screens/widgets.md` § 2). On a count the glyph is the *unit* — `capacity  1` has lost what
    // the number was of — while `certificates  30d` and `certificates  out` already state the
    // fact in words that survive a monochrome terminal, so the band only colours them. This
    // printer drew `15d▲` and `out●`, which is the shape the rule forbids; the rule stands and
    // the printer is what was wrong
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 9).
    let mut out = match &report.badge {
        Some(badge) => format!(
            "[sidebar]  {}{}\n",
            badge.value,
            match badge.value.parse::<u64>() {
                Ok(_) => mark(Some(badge.severity)).trim_end_matches(' '),
                Err(_) => "",
            }
        ),
        None => "[sidebar]\n".to_string(),
    };
    out.push_str(&format!("\n{}\n", report.title));
    for row in &report.rows {
        match row {
            Row::Answer {
                severity,
                text,
                detail,
                action,
                ..
            } => {
                out.push_str(&format!("{}{text}\n", mark(*severity)));
                for paragraph in detail {
                    out.push_str(&format!("      {paragraph}\n"));
                }
                if !action.is_empty() {
                    out.push_str(&format!("      \u{2192} {action}\n"));
                }
            }
            Row::Prose(text) => out.push_str(&format!("\n{text}\n")),
            Row::NotComputed { reason, ask_for } => {
                out.push_str(&format!("\n{reason}\n\n{ask_for}\n"));
            }
        }
    }
    out
}

/// Where a node sits in the snapshot's list, by name — the way a plant reaches one field of one
/// machine without rebuilding the corpus around it.
fn index_of(cluster: &ClusterSnapshot, name: &str) -> usize {
    cluster
        .nodes
        .iter()
        .position(|n| n.id.name == name)
        .unwrap_or_else(|| panic!("the capture has no node {name}"))
}

/// The row whose text begins with this node's name.
fn row_for<'a>(report: &'a Report, name: &str) -> &'a Row {
    report
        .rows
        .iter()
        .find(|row| {
            matches!(row, Row::Answer { text, .. }
                if text.split_whitespace().next() == Some(name))
        })
        .unwrap_or_else(|| panic!("no row on this pane names {name}"))
}

/// The two sentences of every `NotComputed` row on a pane, in order — what is switched off
/// and what to ask for. Read as a pair, because a report that names the check without naming
/// the way out is the half a reader cannot act on.
fn not_computed(report: &Report) -> Vec<(&str, &str)> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::NotComputed { reason, ask_for } => Some((reason.as_str(), ask_for.as_str())),
            _ => None,
        })
        .collect()
}

/// One label map, spelled the way a capture writes one.
fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// The rows the cursor may land on, in order — the `Answer`s, by their text.
fn selectable(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// --- THE SHAPE ITSELF ---

/// Every `Report` this file builds, named. What each one *draws* is its own test's claim; the
/// rules below hold over all of them.
fn every_report() -> Vec<(&'static str, Report)> {
    vec![
        // **Capacity is computed, not built** — the four hand-written panes that stood here
        // were replaced by its producer when its box landed, and the sweeps below now read the
        // real strings. Five states: a flagged node among healthy ones, a namespace scope, a
        // login that may not list nodes, a cluster with nothing to say, and a metrics-server
        // that answered.
        (
            "capacity",
            super::capacity(&capacity::one_node_over_on_cpu("k8rs-worker3"), &[]),
        ),
        (
            "capacity, one namespace",
            super::capacity(&capacity::scoped("payments"), &[]),
        ),
        (
            "capacity, no nodes",
            super::capacity(&capacity::no_nodes(), &[]),
        ),
        (
            "capacity, nothing overcommitted",
            super::capacity(&corpus(), &[]),
        ),
        ("capacity, metrics answering", {
            let cluster = capacity::one_node_over_on_cpu("k8rs-worker3");
            super::capacity(
                &capacity::with_metrics(cluster.clone(), capacity::metrics_read(&cluster, &[])),
                &[],
            )
        }),
        // **Drain safety and Waste are computed too**, as of their box — the four hand-written
        // panes that stood here are gone and the sweeps below read the real strings. Seven states
        // between them: the pane with all three row kinds, the one namespace scope that switches
        // the whole report off, the four wasteful rows, the cut section with its overflow line,
        // the scoped title, the cluster with nothing wasteful on it, and the login that could
        // read none of the four lists.
        (
            "drain safety",
            super::drain_safety(&drain::drain_corpus(), &[]),
        ),
        (
            "drain safety, one namespace",
            super::drain_safety(&drain::scoped_drain("payments"), &[]),
        ),
        (
            "drain safety, every node ready",
            super::drain_safety(&drain::nothing_to_drain(), &[]),
        ),
        // **The one entry built with findings in hand**, because one row on this pane is: a node
        // whose kubelet stopped posting is N1's card restated as *would never finish draining*,
        // and with an empty slice there is no card to restate.
        (
            "drain safety, a node that went quiet",
            drain::a_node_that_went_quiet(),
        ),
        ("waste", super::waste(&waste::waste_corpus(), &[])),
        (
            "waste, a section cut short",
            super::waste(&waste::overflowing(), &[]),
        ),
        (
            "waste, one namespace",
            super::waste(
                &ClusterSnapshot {
                    namespace_scope: Some("payments".to_string()),
                    ..waste::waste_corpus()
                },
                &[],
            ),
        ),
        (
            "waste, nothing wasted",
            super::waste(&waste::nothing_wasted(), &[]),
        ),
        (
            "waste, nothing could be read",
            super::waste(&waste::nothing_could_be_read(), &[]),
        ),
        // **Posture, Versions and Certificates are computed too**, as of their box — the
        // hand-built certificates pane that stood here is gone, and its C2 row with it
        // (NOTES § D129). Ten states between the three: the host-path list, its namespace scope
        // and the cluster that mounts nothing; a cluster whose kubelets all match, one with a
        // machine too far behind, one whose control-plane version could not be read and one
        // whose node list could not be; and a kubeconfig running out beside an unread CSR list,
        // an expired one beside machines waiting to join, and a pane with nothing to say.
        ("posture", super::posture(&posture::posture_corpus(), &[])),
        (
            "posture, one namespace",
            super::posture(
                &ClusterSnapshot {
                    namespace_scope: Some("kube-system".to_string()),
                    ..posture::posture_corpus()
                },
                &[],
            ),
        ),
        (
            "posture, nothing mounted",
            super::posture(
                &ClusterSnapshot {
                    pods: vec![captured_pod("healthy")],
                    ..corpus()
                },
                &[],
            ),
        ),
        (
            "versions",
            super::versions(&versions::version_corpus(), &[]),
        ),
        (
            "versions, a machine behind",
            super::versions(
                &versions::kubelet_at(versions::version_corpus(), "k8rs-worker3", "v1.32.4"),
                &[],
            ),
        ),
        (
            "versions, no control plane version",
            super::versions(
                &ClusterSnapshot {
                    server_version: None,
                    ..versions::version_corpus()
                },
                &[],
            ),
        ),
        (
            "versions, no nodes",
            super::versions(
                &ClusterSnapshot {
                    nodes: Vec::new(),
                    ..versions::version_corpus()
                },
                &[],
            ),
        ),
        (
            "certificates",
            certificates::certificates_pane(&certificates::with_kubeconfig(
                corpus(),
                "expiring-client",
            )),
        ),
        (
            "certificates, expired and machines waiting",
            certificates::certificates_pane(&certificates::with_requests(
                certificates::with_kubeconfig(corpus(), "expired-client"),
                vec![certificates::a_kubelet_waiting("csr-one")],
            )),
        ),
        (
            "certificates, nothing to say",
            certificates::certificates_pane(&certificates::with_requests(
                certificates::with_kubeconfig(corpus(), "healthy-client"),
                Vec::new(),
            )),
        ),
        // **Restarts, computed** — the pane that folds, the namespace scope, and the cluster
        // where nothing has restarted enough to matter.
        (
            "restarts",
            super::restarts(&restarts::restarts_corpus(), &[]),
        ),
        (
            "restarts, one namespace",
            super::restarts(
                &ClusterSnapshot {
                    namespace_scope: Some("payments".to_string()),
                    ..restarts::restarts_corpus()
                },
                &[],
            ),
        ),
        (
            "restarts, nothing qualifying",
            super::restarts(
                &ClusterSnapshot {
                    pods: vec![captured_pod("healthy")],
                    ..corpus()
                },
                &[],
            ),
        ),
    ]
}

#[test]
fn every_pane_the_screen_draws_is_expressible() {
    // **What this covers, exactly**: all seven reports on the screen, computed, in every state
    // `screens/analysis.md` gives them — the panes it sketches, the *could not run* rows its
    // § *What each report needs* table names, and the empty pane rule 8 gives each one — plus the
    // states the shape reaches that no mockup draws, which today is the login that could read
    // none of Waste's four lists.
    //
    // **Seven reports and six panes is not a defect**: `Versions` is drawn at the foot of the
    // Certificates pane rather than as a pane of its own, and which panes exist is `screens/`'s
    // ruling and not this type's ([`Report`]).
    for (name, report) in every_report() {
        assert!(!report.title.is_empty(), "{name} has a pane heading");
        // Invariant 14 reaches every string here: a report never titles itself with its own
        // sidebar label, which is the jargon the heading exists to replace.
        assert!(
            !report.title.eq_ignore_ascii_case(name),
            "{name}'s heading is a sentence, not its name"
        );
    }
    // **No report here draws no rows any more**, and that is a stronger claim than the one this
    // assertion used to make. The empty `Vec` stays legal ([`Report::rows`]) and no pane asks for
    // it (NOTES § D128): the hand-built `waste, nothing wasted` that used to be the one name in
    // this list is now Waste's own producer, which says it has nothing to say in one
    // `Row::Prose` — rule 8. A producer that quietly returned nothing at all now shows up here
    // rather than passing a sweep over an empty list.
    assert_eq!(
        every_report()
            .iter()
            .filter(|(_, report)| report.rows.is_empty())
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        Vec::<&str>::new()
    );
}

/// **One string per place [`strings_of`] reaches** — the canaries both sweeps below are proven
/// against, `scripts/write-guard.py`'s own mechanism. Neither sweep asserts anything is *present*:
/// they check that no swept string spells a glyph and that none breaks a line, and a
/// [`strings_of`] that returned an empty `Vec` — or an [`every_report`] that quietly lost
/// entries — passes both.
///
/// The first three are the three string fields of one row: Waste's orphan Service, which is on
/// that pane in the corpus's own state.
const CANARIES: [&str; 8] = [
    // Row::Answer's text, detail and action.
    "default/broken-noendpoints matches no pod",
    "This Service points at nothing. Anything calling it gets a 503.",
    "fix its selector, or delete it",
    // Row::Prose — the heading the Versions report draws for the pane it shares.
    "Versions",
    // Row::NotComputed's two halves.
    "Not checked. Reading what a node has needs permission to list nodes, and this login does not \
     have it.",
    "Ask for permission to list nodes across the whole cluster.",
    // The report's own two. The badge is C1's countdown off the committed certificate and the pin
    // `scripts/certs-test.sh` holds, asserted for itself in `certificates.rs` — so if this line
    // and that one go red together, the certificate moved and this sweep is the echo.
    "Things that cost you something for nothing",
    "13d",
];

#[test]
fn both_sweeps_are_reading_something() {
    // **A derived list asserts it found something** (CLAUDE.md § Tests must not lie). Two ways the
    // sweeps below degrade in silence, and this is the test for both: [`strings_of`] stopping at
    // one of the places a string can sit, and [`every_report`] losing an entry.
    let reports = every_report();
    assert_eq!(
        reports.len(),
        27,
        "a pane that left this list takes both sweeps below with it, and every claim they make \
         about it goes quiet rather than red: {:?}",
        reports.iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    let swept: BTreeSet<&str> = reports
        .iter()
        .flat_map(|(_, report)| strings_of(report))
        .collect();
    for canary in CANARIES {
        assert!(
            swept.contains(canary),
            "nothing sweeps {canary:?} any more, so whatever field it sits in is unchecked"
        );
    }
}

#[test]
fn a_duration_badge_draws_no_glyph_and_a_count_draws_one() {
    // **The one rule this file's own printer has to obey**, and it did not: it drew `15d▲` and
    // `out●`, which `screens/widgets.md` § 2 forbids — a duration badge already states the fact
    // in words, so the band only colours it, while on a count the glyph is the unit. This is a
    // test artefact and not a product string, which is why it is asserted here and not swept by
    // [`strings_of`] (`theme.rs` owns the glyph; nothing in `analysis.rs` spells one).
    let badged = |value: &str| {
        pane(&Report {
            title: "t".to_string(),
            badge: Some(Badge {
                value: value.to_string(),
                severity: Severity::Warn,
            }),
            rows: Vec::new(),
        })
        .lines()
        .next()
        .expect("the printer opens on the sidebar line")
        .to_string()
    };
    assert_eq!(
        badged("1"),
        "[sidebar]  1\u{25b2}",
        "a count carries its unit"
    );
    assert_eq!(badged("15d"), "[sidebar]  15d");
    assert_eq!(badged("out"), "[sidebar]  out");
}

#[test]
fn nothing_a_report_carries_spells_a_glyph_or_breaks_its_own_line() {
    for (name, report) in every_report() {
        for s in strings_of(&report) {
            // No theme symbol reaches this file (CLAUDE.md § single point of change), and `⏎`
            // is `views.rs`'s footer and its `— ⏎ to list` suffix — the one glyph the screen
            // prints *inside* a row, which is why it is easiest to leave in by accident.
            assert!(
                !s.contains(['\u{25cf}', '\u{25b2}', '\u{25cb}', '\u{2192}', '\u{23ce}']),
                "{name} spells a glyph that belongs to theme.rs or views.rs: {s}"
            );
            // A row is one line before wrapping. A newline in it is a wrap this layer made,
            // and it is the layer that cannot see the pane's width — the row-height accounting
            // it breaks is Phase 9's (PRIOR-ART § D3).
            assert!(
                !s.contains(['\n', '\r']),
                "{name} breaks a line the renderer has not measured: {s}"
            );
        }
    }
}
