//! `analysis.rs` § THE CERTIFICATES REPORT — its tests (NOTES § D91).

use super::*;

use crate::rules::{CertificateRequestSnapshot, analyze};

use k8s_openapi::api::certificates::v1::{
    CertificateSigningRequest, CertificateSigningRequestCondition,
};

// --- CERTIFICATES ---
//
// **C1 arrives through `analyze`, not through a hand-built `Finding`.** The producer's whole
// contract is that it picks one card out of the slice the rule engine already returned, so a test
// that handed it a card built in this file would prove the picking and not the pipeline
// (NOTES § D29). `scripts/certs-test.sh` pins the three committed certificates against the same
// instant [`now`] spells — 15 days left, 356 days left, 12 days past — and refuses to let that
// script and the test files disagree about it.
//
// **C2 has no row and no test asserting one**: the API server's serving certificate needs a TLS
// peer certificate kube-rs does not expose, it is a Phase 5 box, and the screen already knows
// (NOTES § D129).

/// C1's input, as `k8s.rs` will hand it over: the kubeconfig's context name and the PEM bytes of
/// its client certificate.
pub(super) fn with_kubeconfig(cluster: ClusterSnapshot, certificate: &str) -> ClusterSnapshot {
    let path = format!(
        "{}/tests/fixtures/certs/{certificate}.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    ClusterSnapshot {
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(
            std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path}: {e}")),
        ),
        ..cluster
    }
}

/// The pane as the producer is really driven: the snapshot, and the findings
/// [`crate::rules::analyze`] returned for it.
pub(super) fn certificates_pane(cluster: &ClusterSnapshot) -> Report {
    super::certificates(cluster, &analyze(cluster))
}

/// The committed CSR with its signer and its verdict set on the way in — the plant mechanism
/// (NOTES § D40), because `csr-pending.json` is a *human* asking for a kubeconfig and this row is
/// about machines.
fn csr_but(
    name: &str,
    edit: impl FnOnce(&mut CertificateSigningRequest),
) -> CertificateRequestSnapshot {
    let mut object: CertificateSigningRequest = serde_json::from_value(fixture("csr-pending"))
        .expect("csr-pending.json is a CertificateSigningRequest");
    object.metadata.name = Some(name.to_string());
    edit(&mut object);
    CertificateRequestSnapshot::from(object)
}

/// A request nobody has ruled on yet, from the machine signer — the row's subject.
pub(super) fn a_kubelet_waiting(name: &str) -> CertificateRequestSnapshot {
    csr_but(name, |c| {
        c.spec.signer_name = "kubernetes.io/kube-apiserver-client-kubelet".to_string();
    })
}

pub(super) fn with_requests(
    cluster: ClusterSnapshot,
    requests: Vec<CertificateRequestSnapshot>,
) -> ClusterSnapshot {
    ClusterSnapshot {
        certificate_requests: Some(requests),
        ..cluster
    }
}

#[test]
fn the_kubeconfig_row_is_picked_by_identity_and_jumps_to_the_finding_a_rule_already_made() {
    let cluster = with_kubeconfig(corpus(), "expiring-client");
    let report = certificates_pane(&cluster);
    println!("{}", pane(&report));

    assert_eq!(report.title, "What expires, soonest first");
    let row = &report.rows[0];

    // **The identity, and it is the whole of the picking.** A `Finding::title` is a
    // plain-language sentence: the next invariant-14 pass rewords it, and a producer matching on
    // one stops matching with nothing red — the row keeps drawing and quietly loses its `⏎`.
    let Some(Jump::Finding(finding)) = jump_of(row) else {
        panic!("this is the one row on the pane whose ⏎ goes to a finding");
    };
    assert_eq!(
        finding.object.kind,
        ObjectKind::Other("kubeconfig".to_string()),
        "the identity the producer reads, and the one finding in the product with no API object \
         behind it"
    );
    assert_eq!(
        finding.object.uid, None,
        "a file on a laptop never had a uid"
    );
    assert_eq!(
        finding.owner, finding.object,
        "there is nothing above a file"
    );
    assert_eq!(finding.timestamp, None, "and so this card draws no age");
    assert_eq!(
        finding.kubectl_cmd, None,
        "no kubectl command shows this, which is why C1 exists"
    );

    // **The wording is the rule's, verbatim** (NOTES § D46): the row is its title, the paragraph
    // is its evidence, the way out is its action. A report and the rule behind it saying two
    // different things about one certificate is what this project refuses.
    assert_eq!(text_of(row), finding.title);
    assert_eq!(detail_of(row), std::slice::from_ref(&finding.evidence));
    assert_eq!(action_of(row), finding.action);
    assert_eq!(
        text_of(row),
        "Your kubeconfig certificate expires in 15 days",
        "the committed certificate is 15 days from the pin `scripts/certs-test.sh` holds"
    );

    // **The band is the pane's and the rule's `Info` is its routing** (NOTES § D87): on a
    // `Finding`, `Info` means *this lives in a report rather than in Alerts*, and once it is in
    // the report the band says how loud the row is. `screens/analysis.md` draws it `▲`.
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(severity_of(row), Some(Severity::Warn));

    // **A namespace scope changes nothing here and the title claims none.** C1 is a file on the
    // reader's own disk and a CSR is cluster-scoped, so neither answer narrows with the view.
    let scoped = super::certificates(
        &ClusterSnapshot {
            namespace_scope: Some("payments".to_string()),
            ..cluster.clone()
        },
        &analyze(&cluster),
    );
    assert_eq!(scoped, report);
}

#[test]
fn an_expired_kubeconfig_keeps_the_rules_band_because_it_is_broken_now() {
    let report = certificates_pane(&with_kubeconfig(corpus(), "expired-client"));
    println!("{}", pane(&report));
    let row = &report.rows[0];

    assert_eq!(
        severity_of(row),
        Some(Severity::Critical),
        "being locked out this second is broken-now, and this is the one band of C1 that also \
         reaches Alerts (NOTES § D87)"
    );
    assert_eq!(
        text_of(row),
        "Your kubeconfig certificate expired 12 days ago — the cluster is refusing you"
    );
    assert!(
        action_of(row).contains("kubectl has stopped working for you too"),
        "the tense is the rule's too: {}",
        action_of(row)
    );
}

#[test]
fn a_certificate_with_a_year_left_draws_no_row_at_all() {
    // **The negative that lets the two above fail.** C1 fires from 30 days out and says nothing
    // before that, so a producer that drew a row whenever a kubeconfig was present would pass
    // both tests above and this one is the only thing that catches it.
    let cluster = with_kubeconfig(corpus(), "healthy-client");
    assert!(
        !analyze(&cluster)
            .iter()
            .any(|f| matches!(&f.object.kind, ObjectKind::Other(k) if k == "kubeconfig")),
        "356 days out, the rule says nothing — and this report may not say it for it"
    );
    let report = certificates_pane(&cluster);
    println!("{}", pane(&report));
    assert!(
        !report.rows.iter().any(|row| matches!(
            row,
            Row::Answer {
                jump: Some(Jump::Finding(_)),
                ..
            }
        )),
        "no card, no row"
    );

    // And with no kubeconfig read at all — the ordinary state for a token or an exec plugin.
    let none = certificates_pane(&corpus());
    assert_eq!(none.rows, report.rows);
}

#[test]
fn the_pending_csr_row_is_one_not_computed_while_nothing_fetches_the_list() {
    // **`certificate_requests` is `None` through the whole of Phase 4 on purpose**
    // (NOTES § D129): C3's fetch is a Phase 5 box, and `list certificatesigningrequests` is a
    // cluster-scoped verb most namespaced roles do not have, so `None` stays the ordinary answer
    // on a real cluster afterwards. The pane is honest about it rather than silent.
    let cluster = with_kubeconfig(corpus(), "expiring-client");
    assert_eq!(cluster.certificate_requests, None);
    let report = certificates_pane(&cluster);
    println!("{}", pane(&report));

    let [(reason, ask_for)] = not_computed(&report)[..] else {
        panic!("one NotComputed, in the place its answer would have been");
    };
    assert!(reason.starts_with("Machines waiting to join are not checked."));
    // **It names no cause**, because the field cannot tell *nobody fetched it* from *this login
    // may not list them* — and the way out below works for either.
    assert!(
        !reason.contains("k8rs has not") && !reason.contains("not allowed"),
        "the sentence says what is missing, not whose fault it is: {reason}"
    );
    assert_eq!(
        ask_for,
        "Ask for permission to list certificatesigningrequests across the whole cluster."
    );
    // **Not the same fact as *nothing is waiting***: an empty list is an answer, and it draws no
    // row at all.
    let empty = certificates_pane(&with_requests(cluster.clone(), Vec::new()));
    assert!(
        not_computed(&empty).is_empty(),
        "`Some(vec![])` is *nobody is waiting*, and a row saying the check did not run over it \
         is the sentence this screen exists not to print"
    );
    assert_eq!(empty.rows.len(), 1, "just C1: {:?}", empty.rows);
}

#[test]
fn the_machines_waiting_to_join_are_counted_and_the_humans_are_not() {
    let cluster = with_kubeconfig(corpus(), "expiring-client");
    let report = certificates_pane(&with_requests(
        cluster.clone(),
        vec![
            a_kubelet_waiting("csr-one"),
            a_kubelet_waiting("csr-two"),
            // **A human asking for a kubeconfig, which is the committed object untouched.** The
            // row says *kubelets*, and `signerName` is the only field that tells the two apart.
            csr_but("csr-human", |_| ()),
        ],
    ));
    println!("{}", pane(&report));

    let row = &report.rows[1];
    assert_eq!(text_of(row), "2 kubelets are waiting to be let in");
    assert_eq!(
        detail_of(row),
        ["2 machines cannot join the cluster until someone approves their requests."]
    );
    assert_eq!(severity_of(row), Some(Severity::Critical));
    assert_eq!(
        jump_of(row),
        None,
        "a counted row stands for a set, and `Jump` has no case for one"
    );
    assert!(action_of(row).starts_with("approve each request"));

    // One is its own sentence, not a number with an `(s)` after it.
    let one = certificates_pane(&with_requests(cluster, vec![a_kubelet_waiting("csr-one")]));
    assert_eq!(text_of(&one.rows[1]), "1 kubelet is waiting to be let in");
    assert_eq!(
        detail_of(&one.rows[1]),
        ["A machine cannot join the cluster until someone approves its request."]
    );
}

#[test]
fn a_request_that_has_a_verdict_is_not_waiting_for_one() {
    // **Pending is the absence of all three**, which is why the snapshot carries the conditions
    // rather than a `pending: bool` — and an approved-but-not-yet-issued request is waiting on
    // the *signer* rather than on a person, so **approve it** is not its way out and it is not
    // this row.
    let verdict = |type_: &str| {
        csr_but("csr-ruled", |c| {
            c.spec.signer_name = "kubernetes.io/kube-apiserver-client-kubelet".to_string();
            c.status
                .get_or_insert_with(Default::default)
                .conditions
                .get_or_insert_with(Vec::new)
                .push(CertificateSigningRequestCondition {
                    type_: type_.to_string(),
                    status: "True".to_string(),
                    reason: Some("AutoApproved".to_string()),
                    message: None,
                    last_transition_time: None,
                    last_update_time: None,
                });
        })
    };
    let cluster = with_kubeconfig(corpus(), "expiring-client");
    for type_ in ["Approved", "Denied", "Failed"] {
        let report = certificates_pane(&with_requests(cluster.clone(), vec![verdict(type_)]));
        assert_eq!(
            report.rows.len(),
            1,
            "{type_} is a verdict, so nothing here is waiting for one: {:?}",
            report.rows
        );
    }
    // And the same object without the condition is the row, or the loop above proves nothing.
    let report = certificates_pane(&with_requests(
        cluster,
        vec![a_kubelet_waiting("csr-ruled")],
    ));
    assert_eq!(
        text_of(&report.rows[1]),
        "1 kubelet is waiting to be let in"
    );
}

#[test]
fn the_badge_is_c1s_own_countdown_and_the_pane_never_disagrees_with_it() {
    // **The badge is C1's value and C1's band** — the sidebar's `certificates  30d`, and the one
    // alerting mechanism the expiring band has, because it never reaches Alerts (NOTES § D87).
    let report = certificates_pane(&with_kubeconfig(corpus(), "expiring-client"));
    println!("{}", pane(&report));
    let badge = report
        .badge
        .clone()
        .expect("a certificate 15 days out is badged");

    assert_eq!(badge.value, "15d");
    assert_eq!(
        badge.severity,
        Severity::Warn,
        "C1's own band on this pane, and deliberately not the worst row: the `●` CSR row beside \
         a `▲` badge is what `screens/analysis.md` draws"
    );
    // **The row and the badge are the same subtraction, spelled twice** — the row in C1's own
    // sentence, the badge in the sidebar's three columns. This is the assertion that catches
    // them drifting: `expires_at` and `now` are the same on both sides, so only the *spelling*
    // may differ (NOTES § D46).
    let days = badge.value.trim_end_matches('d');
    assert!(
        text_of(&report.rows[0]).contains(&format!("in {days} days")),
        "the sidebar says {} and the row says {:?}",
        badge.value,
        text_of(&report.rows[0])
    );
}

#[test]
fn a_certificate_that_has_run_out_badges_a_word_and_never_a_number() {
    // **The expired case, and why it is not a duration.** `in_days` drops the sign because the
    // *card's sentence* carries the direction — *expired 12 days ago*. A badge has no sentence
    // beside it, so every numeric spelling is wrong in the dangerous direction: `0d` reads as
    // *expires today*, which is *still valid*; `12d` is indistinguishable from twelve days left;
    // and `-12d` is a minus sign a beginner has to be taught (invariant 14). So the expired band
    // leaves the number behind altogether and says the one thing the card says — you are locked
    // out.
    let report = certificates_pane(&with_kubeconfig(corpus(), "expired-client"));
    println!("{}", pane(&report));
    let badge = report
        .badge
        .clone()
        .expect("being locked out is still badged");

    assert_eq!(badge.value, "out");
    assert_eq!(
        badge.severity,
        Severity::Critical,
        "the expired band, which also reaches Alerts — the sidebar is not its only home, and it \
         still may not read quieter than the pane"
    );
    assert!(
        !badge.value.chars().any(|c| c.is_ascii_digit()),
        "no number, because every number here reads as time you still have: {}",
        badge.value
    );
    // Three columns is the whole budget: `certificates` is twelve of the sidebar's twenty
    // (`screens/widgets.md` § 1), which is the same measurement that keeps a `▲` off this badge.
    assert!(badge.value.len() <= 3, "{} does not fit", badge.value);

    // And the row still carries the count, so nothing is lost — only the sidebar drops it.
    assert!(text_of(&report.rows[0]).contains("expired 12 days ago"));
}

#[test]
fn a_certificate_with_hours_left_badges_no_days_and_is_not_the_expired_one() {
    // **The boundary, and the one place the two spellings could have collided.** RFC 5280 makes
    // a certificate valid *through* `notAfter`, and C1's own test is `left < ZERO` — so the
    // deadline itself is still inside the window. Six hours before it, the honest badge is `0d`:
    // no whole days left, and still valid. That is a different fact from `out`, and the two have
    // to stay different or the sidebar tells a locked-out reader they have until tonight.
    //
    // **The clock is moved, not the certificate** (NOTES § D53): `scripts/certs-test.sh` pins
    // `expiring-client` at `2026-09-05T00:00:00Z`, and this is the only test in the file that
    // does not read it from the shared pin.
    let six_hours_before = ClusterSnapshot {
        now: Time(
            "2026-09-04T18:00:00Z"
                .parse()
                .expect("the pin is a timestamp"),
        ),
        ..with_kubeconfig(corpus(), "expiring-client")
    };
    let report = certificates_pane(&six_hours_before);
    println!("{}", pane(&report));

    let badge = report
        .badge
        .clone()
        .expect("still inside the window, still badged");
    assert_eq!(badge.value, "0d", "no whole days left — and not `out`");
    assert_eq!(badge.severity, Severity::Warn, "it has not run out yet");
    assert!(
        text_of(&report.rows[0]).contains("less than a day"),
        "and the row says it in the rule's own words: {}",
        text_of(&report.rows[0])
    );

    // **The deadline itself, to the second — the one instant `<` and `<=` disagree about.** RFC
    // 5280 makes the certificate valid *through* `notAfter`, and C1's own test is `left < ZERO`,
    // so at exactly `notAfter` the rule still says *expires in less than a day*. A badge reading
    // `out` there would be the sidebar calling a reader locked out while the pane on the other
    // side of the divider says they are not — one certificate, two answers (NOTES § D46).
    let on_the_deadline = ClusterSnapshot {
        now: Time(
            "2026-09-05T00:00:00Z"
                .parse()
                .expect("the pin is a timestamp"),
        ),
        ..with_kubeconfig(corpus(), "expiring-client")
    };
    let report = certificates_pane(&on_the_deadline);
    println!("{}", pane(&report));
    assert_eq!(
        report.badge.map(|b| (b.value, b.severity)),
        Some(("0d".to_string(), Severity::Warn)),
        "valid *through* notAfter, so the deadline itself is still inside the window"
    );
    assert!(text_of(&report.rows[0]).contains("expires in less than a day"));
}

#[test]
fn nothing_to_count_is_no_badge_at_all() {
    // **The ordinary state on most clusters.** No C1 finding — a certificate with a year left, a
    // token or an exec plugin instead of one, or no current context — is nothing to badge, and
    // `Some("0")` in its place would be a number the sidebar has no room to explain
    // ([`Report::badge`]). The 30-day threshold stays `CERT_EXPIRY_WARN`'s: this producer never
    // asks how far away the deadline is, only whether the rule already answered.
    for cluster in [
        with_kubeconfig(corpus(), "healthy-client"),
        corpus(),
        ClusterSnapshot {
            context: None,
            ..with_kubeconfig(corpus(), "expiring-client")
        },
    ] {
        let report = certificates_pane(&cluster);
        assert_eq!(report.badge, None, "no card, no badge: {:?}", report.rows);
    }

    // **And a CSR section that could not be checked changes it not at all** — the question
    // `screens/analysis.md` hands to this box. The badge is the alerting mechanism for the one
    // finding with no other home; *did not run* is recorded by the `Row::NotComputed` in the
    // body, which is the only place it is ever recorded (`Report::badge`). A badge that moved
    // because a *different* section could not run would be the sidebar carrying a reason it has
    // no room for.
    let unread = certificates_pane(&with_kubeconfig(corpus(), "expiring-client"));
    let checked = certificates_pane(&with_requests(
        with_kubeconfig(corpus(), "expiring-client"),
        vec![a_kubelet_waiting("csr-one")],
    ));
    assert_eq!(
        unread.badge, checked.badge,
        "the CSR section running, or not running, is not the badge's subject"
    );
    assert_ne!(
        unread.rows, checked.rows,
        "and the difference between the two panes is in the body, where there is room for a \
         reason"
    );
    assert!(
        not_computed(&unread).len() == 1 && not_computed(&checked).is_empty(),
        "which is exactly the difference: one pane could not check the CSR list and the other \
         could"
    );
}

#[test]
fn a_pane_with_nothing_to_say_says_so_and_only_when_there_is_nothing_else() {
    // Rule 8, in this report's own words — and like Waste, **only when there is nothing else at
    // all**: a pane carrying one `NotComputed` has not established that nothing expires soon.
    let quiet = certificates_pane(&with_requests(
        with_kubeconfig(corpus(), "healthy-client"),
        Vec::new(),
    ));
    println!("{}", pane(&quiet));
    assert_eq!(
        quiet.rows,
        vec![Row::Prose(
            "Nothing here expires soon, and no machine is waiting to be let in.".to_string()
        )]
    );

    let unread = certificates_pane(&with_kubeconfig(corpus(), "healthy-client"));
    assert!(
        !unread.rows.iter().any(|row| matches!(row, Row::Prose(_))),
        "the CSR list was not read, so *nothing is waiting* has not been established: {:?}",
        unread.rows
    );
}

#[test]
fn the_soonest_thing_leads_and_c2_is_on_no_row() {
    let report = certificates_pane(&with_requests(
        with_kubeconfig(corpus(), "expiring-client"),
        vec![a_kubelet_waiting("csr-one")],
    ));
    println!("{}", pane(&report));

    // The dated row is first — *what expires, soonest first* — and the counted one, which has no
    // date at all, follows it.
    assert!(matches!(jump_of(&report.rows[0]), Some(Jump::Finding(_))));
    assert!(text_of(&report.rows[1]).contains("waiting to be let in"));
    assert_eq!(report.rows.len(), 2);

    // **C2 draws nothing** (NOTES § D129): the API server's serving certificate is the peer
    // certificate of a TLS handshake, kube-rs does not expose it, and reaching it needs a second
    // outbound connection — a Security gate question before it is a snapshot field.
    for row in &report.rows {
        let text = match row {
            Row::Answer { text, .. } => text.as_str(),
            Row::Prose(text) => text.as_str(),
            Row::NotComputed { reason, .. } => reason.as_str(),
        };
        assert!(
            !text.contains("API server certificate"),
            "a row promising a number k8rs cannot read: {text}"
        );
    }
}
