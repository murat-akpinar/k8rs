//! `rules.rs` § THE CERTIFICATE RULES — its tests (NOTES § D91).

use super::*;

// --- C1, THE ONE CARD ABOUT THE READER'S OWN MACHINE ---
//
// Three committed certificates, whose `notBefore` and `notAfter` are pinned and asserted
// by `scripts/certs-test.sh` against the same instant [`now`] spells — 13 days left, 354
// days left, 14 days past. That script also refuses to let this file and itself disagree
// about that instant, so a pin moved in one and not the other is a red guard.
//
// **What is guarded is the assertions, and a day-count written into prose is not one**
// (NOTES § D114). Nothing compares a comment to the script's output, so the three counts
// above are transcribed — and they are therefore the **only** place below this line where a
// count taken *at the pin* is written in prose. Every other comment names the committed
// deadline instead (`2026-09-05`, `2027-08-12`, `2026-08-09`, absolute bytes a repin does
// not move) or a distance inside its own table, which the pin does not reach. Three prose
// counts in this file were found stale on 2026-08-16, each having survived the repin that
// made it wrong and one of them two, which is the drift this convention exists to stop.
//
// **The three numbers move with the pin and the certificates do not** (NOTES § D97) —
// on 2026-08-16, on 2026-08-17, on 2026-08-20 when the capture trip that added the four
// Family C fixtures pushed the pin four days, and on 2026-08-22 when one targeted capture
// pushed it two more (NOTES § D156). Their `notBefore` and `notAfter` are absolute, so a
// repin changes what the counts are *at* the pin and nothing about the committed bytes —
// and both fixtures stay on the side of [`CERT_EXPIRY_WARN`] they were made for (13 days
// is inside the 30-day window, 354 days is outside it), which is the property that would
// have forced a regeneration if it had failed.
//
// **A repin is therefore an edit in two ownership rows.** The three numbers above and the
// three in `scripts/certs-test.sh` are one fact, and that script's `now=` line is compared
// against `fn now()` on every `just check` — so a pin moved here without moving there is a
// red guard, not a silent drift.
//
// **What no committed fixture can reach is built from bytes here**, and the reason is
// not convenience: `tests/fixtures/certs/` is a closed set — `certs-test.sh` fails on
// any file it does not know the dates of, and `scripts/make-certs.sh` is deliberately
// not wired into `just fixtures` — so a fourth certificate cannot be committed without
// reopening a Phase 2 box. RFC 5280's `99991231235959Z` is not a shape openssl writes
// for you either.

/// The snapshot C1 is handed: a kubeconfig and a moment, and no cluster at all. That is
/// the rule's whole character — it reads a file on the user's machine, so it is the one
/// rule that still answers when every other one has nothing to look at.
fn kubeconfig(context: Option<&str>, certificate: Option<Vec<u8>>) -> ClusterSnapshot {
    ClusterSnapshot {
        context: context.map(str::to_string),
        client_certificate: certificate,
        ..pods_at(Vec::new(), now())
    }
}

/// A PEM block around DER, **encoded with `k8s-openapi`'s own base64** — `ByteString`
/// serialises that way because that is how Secret data travels over the API.
///
/// A hand-written encoder here would be a second implementation whose **failure mode is
/// green**: bad base64 makes the parse fail, which is the same "no finding" three of the
/// tests below assert. Borrowing an encoder the dependency tree already tests is what
/// keeps `certificate_expiring_at`'s positive control able to fail.
fn pem_block(label: &str, der: &[u8]) -> Vec<u8> {
    let quoted = serde_json::to_string(&k8s_openapi::ByteString(der.to_vec()))
        .expect("a byte string serialises to base64");
    let body = quoted.trim_matches('"');
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n").into_bytes()
}

/// One DER `TAG LENGTH VALUE`, short form only — every field of the certificate below is
/// far under 128 bytes, and the assertion is what says so out loud rather than writing a
/// long-form encoder nothing here needs.
fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
    let length = u8::try_from(body.len())
        .expect("every field of the minimal certificate is under 128 bytes");
    let mut out = vec![tag, length];
    out.extend_from_slice(body);
    out
}

/// **The smallest certificate a parser accepts, with one interesting field** — its
/// `notAfter`, handed in whole as an ASN.1 `TAG LENGTH VALUE` so that the two cases below
/// differ in nothing but the digits.
///
/// Everything else is the minimum RFC 5280 structure: a serial, an algorithm, two empty
/// names, and a public key nobody parses. It is **not a fixture and may not become one**
/// (see the section note above).
fn certificate_expiring_at(not_after: &[u8]) -> Vec<u8> {
    // sha256WithRSAEncryption and rsaEncryption — any two OIDs would do; these are what
    // a real client certificate carries, so the shape is one a parser has seen.
    let sha256_rsa = tlv(
        0x06,
        &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0b],
    );
    let rsa = tlv(
        0x06,
        &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01],
    );
    let null = tlv(0x05, &[]);
    let algorithm = tlv(0x30, &[sha256_rsa, null.clone()].concat());
    // An empty RDNSequence: this certificate has no subject and no issuer, and C1 reads
    // neither — the only string on its card that came off a file is the context name.
    let name = tlv(0x30, &[]);
    let validity = tlv(
        0x30,
        &[tlv(0x17, b"260101000000Z"), not_after.to_vec()].concat(),
    );
    let key = tlv(
        0x30,
        &[
            tlv(0x30, &[rsa, null].concat()),
            tlv(0x03, &[0x00, 0x30, 0x03, 0x02, 0x01, 0x00]),
        ]
        .concat(),
    );
    let tbs = tlv(
        0x30,
        &[
            tlv(0x02, &[0x01]),
            algorithm.clone(),
            name.clone(),
            validity,
            name,
            key,
        ]
        .concat(),
    );
    tlv(0x30, &[tbs, algorithm, tlv(0x03, &[0x00])].concat())
}

/// The one card C1 draws, or a failure naming what came out instead. C1 produces at most
/// one finding, so "it fired" and "it fired twice" cannot print the same green line.
fn only_card(all: &[Finding]) -> &Finding {
    assert_eq!(
        all.len(),
        1,
        "C1 draws exactly one card and nothing else is in this snapshot: {:?}",
        titles(all)
    );
    &all[0]
}

/// **C1's positive, on the committed certificate whose dates `certs-test.sh` pins.**
///
/// The identity is the assertion that carries the most: C1 is the one finding with no API
/// object behind it, so `Other("kubeconfig")` / no namespace / the context name / **no
/// uid** is the whole of what a later dialog has to work with (NOTES § D39, § D51).
#[test]
fn the_kubeconfig_certificate_inside_the_window_says_how_long_the_reader_has() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("expiring-client")),
    ));
    show(&all);
    let card = only_card(&all);

    assert_eq!(
        card.severity,
        Severity::Info,
        "the band is the routing, not the volume: `Info` is what sends the expiring half to \
         the Certificates report D2 put it in, and tidying it back to `Warn` would draw a C1 \
         card in Alerts that no screen spec has (NOTES § D87)"
    );
    assert!(
        card.title.contains("13 days"),
        "the certificate has 13 days left at the pinned `now` and the card says so — \
         `scripts/certs-test.sh` asserts that number against the committed bytes: {}",
        card.title
    );
    assert!(
        card.evidence.contains("2026-09-05T00:00:00Z"),
        "the evidence proves the title with the date the reader can put in a calendar: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("your own machine"),
        "the one card whose subject is not the cluster has to say so, or the reader goes \
         looking for the broken pod: {}",
        card.evidence
    );
    assert_eq!(
        card.owner,
        ObjectId {
            kind: ObjectKind::Other("kubeconfig".to_string()),
            namespace: None,
            name: "kind-k8rs".to_string(),
            uid: None,
        },
        "the card is filed under the context name the user recognises, cluster-scoped, \
         with the only `None` uid in the product"
    );
    assert_eq!(
        card.owner, card.object,
        "there is no controller above a file"
    );
    assert_eq!(
        card.kubectl_cmd, None,
        "no kubectl command shows a kubeconfig certificate's dates — `config view` prints \
         the path, and `kubeadm certs check-expiration` reads a control-plane node's disk. \
         `None` here means no such command exists (invariant 4)"
    );
    assert_eq!(
        card.timestamp, None,
        "`notAfter` is a deadline, not the moment this card's event happened, and it is \
         the exact field `age`'s future bound exists to refuse (NOTES § D69)"
    );
    assert_eq!(card.age(&now()), None, "so the card draws no age at all");
}

/// C1's negative, and it is the shape most kubeconfigs are actually in.
#[test]
fn a_kubeconfig_certificate_a_year_out_says_nothing() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("healthy-client")),
    ));
    nothing(
        &all,
        "a certificate good until 2027-08-12 is not news — a rule that speaks here is one \
         whose screen gets ignored",
    );
}

/// **The expired band, which is not the same card.** A certificate that has run out is not
/// *"wrong now, broken soon"*: the reader is locked out of the cluster this second, every
/// other card on the screen is missing because of it, and the tool cannot fix it for them.
#[test]
fn an_expired_kubeconfig_certificate_is_a_failure_and_not_a_warning() {
    let all = analyze(&kubeconfig(
        Some("kind-k8rs"),
        Some(certificate("expired-client")),
    ));
    show(&all);
    let card = only_card(&all);

    assert_eq!(
        card.severity,
        Severity::Critical,
        "and the other band is the other screen: locked out this second is broken-now by D2's \
         own dividing line, so this half alone is an Alerts card — the two differ because they \
         are read in two places, not because one shouts louder (NOTES § D87)"
    );
    assert!(
        card.title.contains("expired 14 days ago"),
        "past the deadline the sentence changes tense and the number is how long ago: {}",
        card.title
    );
    assert!(
        card.evidence.starts_with("was valid until 2026-08-09"),
        "and so does the evidence — `valid until 2026-08-09` on a red card reads as though \
         it still is: {}",
        card.evidence
    );
    assert!(
        card.action.contains("whoever gave you access"),
        "the action names the only person who can fix it, because it is not the cluster \
         and it is not k8rs: {}",
        card.action
    );
    assert_eq!(
        card.timestamp, None,
        "the past-dated half draws no age either — one rule may not put a right edge on \
         one of its bands and a blank on the other"
    );
}

/// **The threshold, which no committed certificate can prove.** The two run out on
/// `2026-09-05` and `2027-08-12` — 341 days apart, whatever the pin — so both fixtures pass any
/// threshold between them; the clock is the snapshot's field precisely so the same bytes can be
/// read at a chosen moment (invariant 5, NOTES § D18).
///
/// The last two rows are the boundary RFC 5280 §4.1.2.5 sets: a certificate is valid
/// *through* `notAfter`, so the deadline itself is still inside the window.
///
/// **The severity column is which screen, not how loud** (NOTES § D87) — `Info` to the
/// Certificates report the whole way down the window, `Critical` once it is past — so the last
/// two rows are one second and two screens apart.
#[test]
fn thirty_days_is_the_threshold_and_the_deadline_itself_is_not_yet_past() {
    assert_eq!(
        CERT_EXPIRY_WARN,
        SignedDuration::from_hours(30 * 24),
        "NOTES § Certificate rules: C1 warns at 30 days"
    );

    // `expiring-client` runs out at 2026-09-05T00:00:00Z (`scripts/certs-test.sh`), so the
    // first two rows are the day either side of the threshold: 31 days out is silence and
    // 30 days out is the card. Written as 31 days first and caught red — the window is
    // closed at the far end, and a rule that fired a day early would have passed every
    // other test in this file: they read the two committed certificates at the pin, and
    // both sit hundreds of days from the day either side of the threshold.
    for (moment, expected) in [
        ("2026-08-05T00:00:00Z", None),
        ("2026-08-06T00:00:00Z", Some(("30 days", Severity::Info))),
        (
            "2026-09-04T00:00:01Z",
            Some(("less than a day", Severity::Info)),
        ),
        (
            "2026-09-05T00:00:00Z",
            Some(("less than a day", Severity::Info)),
        ),
        (
            "2026-09-05T00:00:01Z",
            Some(("less than a day", Severity::Critical)),
        ),
    ] {
        let all = analyze(&ClusterSnapshot {
            now: time(moment),
            ..kubeconfig(Some("kind-k8rs"), Some(certificate("expiring-client")))
        });
        let got = all
            .first()
            .map(|f| (f.title.clone(), f.severity, all.len()));
        println!("{moment} -> {got:?}");
        match expected {
            None => nothing(&all, "31 days out is outside the window C1 was given"),
            Some((says, severity)) => {
                let card = only_card(&all);
                assert!(
                    card.title.contains(says),
                    "at {moment} the card should say {says:?}: {}",
                    card.title
                );
                assert_eq!(card.severity, severity, "at {moment}: {}", card.title);
            }
        }
    }
}

/// **A certificate that never expires produces no finding** — RFC 5280 §4.1.2.5's
/// `99991231235959Z`, which is past the end of jiff's `Timestamp` range, so the conversion
/// answers `Err` and a pure rule may not propagate it. The reflex shape is `.unwrap()`,
/// the input is a kubeconfig, and a corporate PKI is exactly where a non-expiring
/// credential turns up — the panic would land on startup (NOTES § D56).
///
/// **The first row is the control, and without it this test cannot fail.** "No finding"
/// is also what a certificate this file built wrong produces, so the same builder, the
/// same tag and the same field length are fed a date inside C1's window first: if that
/// card does not draw, the `9999` row proves nothing about `9999`.
///
/// **The control's count is taken at the pin and `scripts/certs-test.sh` does not reach it**
/// — the guard reads `tests/fixtures/certs/`, and this certificate is built from bytes here.
/// It is the fourth pin-derived count in the file and the only one outside the three the
/// header names, so a repin moves it too: `2026-09-01` less [`now`], in whole days.
#[test]
fn a_certificate_that_never_expires_draws_no_card_rather_than_panicking() {
    for (not_after, expected) in [
        (b"20260901000000Z", Some("9 days")),
        (b"99991231235959Z", None),
    ] {
        let der = certificate_expiring_at(&tlv(0x18, not_after));
        let all = analyze(&kubeconfig(
            Some("kind-k8rs"),
            Some(pem_block("CERTIFICATE", &der)),
        ));
        show(&all);
        println!(
            "notAfter {} -> {:?}",
            String::from_utf8_lossy(not_after),
            titles(&all)
        );
        match expected {
            Some(says) => assert!(
                only_card(&all).title.contains(says),
                "the control has to draw, or `9999` drawing nothing means nothing: {:?}",
                titles(&all)
            ),
            None => nothing(
                &all,
                "a certificate with no well-defined expiry has no expiry to warn about",
            ),
        }
    }
}

/// **Everything that is not a certificate, fed to the parser.** `rules.rs` returns no
/// `Result`, so a panic in here is a crash of the whole tool — on startup, before the
/// user has pressed anything, from a file they did not know was malformed.
///
/// Four framings, because *"it is not a certificate"* is four different failures: the
/// bytes are not PEM at all, the PEM never ends, the PEM is complete but its contents are
/// not DER, and **the PEM is complete, well-formed and a private key** — the shape that
/// matters most, because the key is the file that sits next to the certificate in the
/// user's `~/.kube` and the one thing that may never be read into our own types.
#[test]
fn malformed_certificate_bytes_produce_no_finding_and_no_panic() {
    let real = certificate("expiring-client");
    let truncated = real[..real.len() / 2].to_vec();
    // A real, parseable certificate wearing the wrong label: the refusal has to be the
    // label, not luck about what the body happens to decode to.
    let mislabelled = pem_block(
        "RSA PRIVATE KEY",
        &certificate_expiring_at(&tlv(0x18, b"20260901000000Z")),
    );

    for (what, bytes) in [
        (
            "not PEM at all",
            b"where did i put that kubeconfig\n".to_vec(),
        ),
        ("a header with a truncated body", truncated),
        (
            "well-formed PEM that is not a certificate",
            pem_block("CERTIFICATE", b"this is not DER"),
        ),
        (
            "a private key wearing a certificate's file name",
            mislabelled,
        ),
        ("empty", Vec::new()),
    ] {
        let all = analyze(&kubeconfig(Some("kind-k8rs"), Some(bytes)));
        println!("{what} -> {:?}", titles(&all));
        nothing(&all, &format!("{what} is not something C1 may report on"));
    }
}

/// **No current context is a real state, not a defensive one** (NOTES § D51): a kubeconfig
/// can name none, and then C1 has no name to file its card under. Inventing one would put
/// a sentence in the field every `kubectl` line is built from.
#[test]
fn a_kubeconfig_with_no_current_context_has_nothing_to_file_the_card_under() {
    let all = analyze(&kubeconfig(None, Some(certificate("expired-client"))));
    nothing(
        &all,
        "a kubeconfig with no current context is one k8rs cannot connect with at all — \
         that screen says so already, and this card would have no name on it",
    );

    let no_certificate = analyze(&kubeconfig(Some("kind-k8rs"), None));
    nothing(
        &no_certificate,
        "a token, an exec plugin or OIDC leaves nothing to parse, and C1 says nothing \
         rather than guessing",
    );
}

/// **C1 beside every other rule, over the whole committed capture** — the card is one more
/// finding on the list, and it is the only one on it that names no API object.
///
/// The pair is the assertion: swapping the certificate for the healthy one removes exactly
/// this card and touches nothing else, which is what says the rule is reading the
/// certificate rather than the cluster it came with.
#[test]
fn the_kubeconfig_card_is_the_only_finding_with_no_object_behind_it() {
    let expiring = analyze(&ClusterSnapshot {
        client_certificate: Some(certificate("expiring-client")),
        ..fixture_snapshot()
    });
    let healthy = analyze(&fixture_snapshot());

    let unidentified: Vec<&str> = expiring
        .iter()
        .filter(|f| f.object.uid.is_none())
        .map(|f| f.title.as_str())
        .collect();
    println!("{unidentified:#?}");
    assert_eq!(
        unidentified.len(),
        1,
        "every other finding names an object the API server can be asked about: \
         {unidentified:?}"
    );
    assert!(unidentified[0].contains("kubeconfig certificate"));

    assert_eq!(
        expiring.len(),
        healthy.len() + 1,
        "the healthy certificate on the same capture draws the same screen minus one \
         card — anything else means C1 moved a finding that is not its own"
    );
    assert!(
        !titles(&healthy).iter().any(|t| t.contains("kubeconfig")),
        "and the card it removed is C1's: {:?}",
        titles(&healthy)
    );
}
