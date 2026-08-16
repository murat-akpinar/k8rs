#!/usr/bin/env bash
# The C1 fixtures (kubeconfig client certificate expiry, warns under 30 days)
# are only fixtures because their dates are pinned. "Pinned" is a claim about
# the committed bytes, not about the intent in make-certs.sh: regenerate them
# with `-days 20` and they still look right on the day they were made, then
# quietly stop testing anything three weeks later — and the usual repair for
# that is to weaken C1's test.
#
# So this asserts the dates the committed certificates actually carry, against
# the same reference `now` the rule's tests pass in `Snapshot` (NOTES § D18 —
# the clock is an input). Exact dates, not "is it in the future": a
# relative-dated regeneration passes that weaker question every single time.
#
# It also asserts what C1 needs to be *true* of those dates, because a fixture
# can be perfectly pinned and still useless: an "expiring" cert outside the
# 30-day window, or a "healthy" one inside it, leaves the rule with a test that
# cannot fail (CLAUDE.md § Tests must not lie).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
certs="$here/../tests/fixtures/certs"

now="2026-08-17 00:00:00Z"   # the reference `now` C1's tests ask about
warn_days=30                 # C1's threshold
now_s=$(date -u -d "$now" +%s)

fail=0
note() { echo "FAIL  $*"; fail=1; }
declare -A left              # fixture name -> days of validity at `now`

# --- ONE INSTANT, TWO FILES START ---
# `now` above and `fn now()` in src/rules_tests.rs are the same fact written twice —
# the dates below are asserted against this one, and C1's `Snapshot` is built
# with that one. Nothing else compares them, so a drift is silent: both files
# keep passing while "22 days left" and "expires in" are computed from
# different instants. Compared as seconds, because the two spell the same
# moment differently (`... 00:00:00Z` here, RFC 3339 `...T00:00:00Z` there).
#
# An extraction that finds nothing must fail loudly, not pass quietly
# (CLAUDE.md — a derived list asserts it found something).
#
# `fn now()` is a top-level item of that file, so its closing brace is in column
# 0. The old range end `/^    }/` — right while the tests were a nested `mod
# tests` — matches nothing there and runs on to the next four-space brace, dozens
# of lines past the function. It still extracts the correct value today, purely
# because the next `time("...")` call site happens to be thousands of lines away:
# a second time helper landing under this one hands two pins to `date` and the
# comparison stops meaning what it says.
rust_pin=$(sed -n '/fn now() -> Time {/,/^}/s/^ *time("\([^"]*\)").*/\1/p' "$here/../src/rules_tests.rs")
if [ -z "$rust_pin" ]; then
  note "no pin found in src/rules_tests.rs — this check expects one time(\"...\") inside fn now(), and without it compares nothing"
elif [ "$(date -u -d "$rust_pin" +%s 2>/dev/null || echo unparseable)" != "$now_s" ]; then
  note "src/rules_tests.rs pins now at '$rust_pin', this file at '$now' — C1's certificates are asserted against one instant and its Snapshot built from the other"
fi
# --- ONE INSTANT, TWO FILES END ---

# name | notBefore | notAfter | days left at the reference `now`
pinned=(
  "expiring-client|2026-08-12 00:00:00Z|2026-09-05 00:00:00Z|19"
  "healthy-client |2026-08-12 00:00:00Z|2027-08-12 00:00:00Z|360"
  "expired-client |2025-08-12 00:00:00Z|2026-08-09 00:00:00Z|-8"
)

# --- PINNED DATES START ---
for entry in "${pinned[@]}"; do
  IFS='|' read -r name want_nb want_na want_days <<<"$entry"
  name=${name// /}
  pem="$certs/$name.crt.pem"

  if [ ! -f "$pem" ]; then
    note "[$name] fixture is missing — C1 has nothing to test against"
    continue
  fi

  dates=$(openssl x509 -in "$pem" -noout -dateopt iso_8601 -dates)
  got_nb=${dates#notBefore=}; got_nb=${got_nb%%$'\n'*}
  got_na=${dates##*notAfter=}

  [ "$got_nb" = "$want_nb" ] || note "[$name] notBefore drifted: want '$want_nb', got '$got_nb'"
  [ "$got_na" = "$want_na" ] || note "[$name] notAfter drifted: want '$want_na', got '$got_na' — the dates are no longer pinned"

  left[$name]=$(( ($(date -u -d "$got_na" +%s) - now_s) / 86400 ))
  [ "${left[$name]}" = "$want_days" ] ||
    note "[$name] has ${left[$name]} days left at $now, not $want_days"

  # A notBefore in the future is a state of its own — not yet valid — that C1
  # does not model and nobody planned a fixture for.
  [ "$(date -u -d "$got_nb" +%s)" -le "$now_s" ] ||
    note "[$name] is not valid yet at $now — that is a different C1 case than expiring"

  # This file's premise is that a real cluster's client certificate never
  # enters the repository, and until now nothing asserted it. A throwaway from
  # make-certs.sh is self-signed; anything issued by a real CA is not, and that
  # is the difference worth catching.
  subject=$(openssl x509 -in "$pem" -noout -subject -nameopt RFC2253)
  issuer=$(openssl x509 -in "$pem" -noout -issuer -nameopt RFC2253)
  [ "${subject#subject=}" = "${issuer#issuer=}" ] ||
    note "[$name] was issued by someone else (${issuer#issuer=}) — fixtures are self-signed throwaways, never a real cluster's certificate"
done
# --- PINNED DATES END ---

# An extra certificate dropped in here would be tested by nothing at all: the
# loop above reads the three it knows and never looks at what else is present.
for pem in "$certs"/*.pem; do
  base=$(basename "$pem")
  case "$base" in
    expiring-client.crt.pem|healthy-client.crt.pem|expired-client.crt.pem) ;;
    *) note "[$base] is in tests/fixtures/certs/ and no test reads it — either pin its dates here or delete it" ;;
  esac
done

# --- WHAT C1 NEEDS TO BE TRUE START ---
if [ ${#left[@]} -eq 3 ]; then
  e=${left[expiring-client]} h=${left[healthy-client]} x=${left[expired-client]}
  [ "$e" -gt 0 ] && [ "$e" -lt "$warn_days" ] ||
    note "expiring-client is $e days out, outside C1's $warn_days-day window — the rule's positive test cannot fail"
  # Strictly outside the window, so the assertion does not hinge on whether the
  # threshold is read as `< 30` or `<= 30`.
  [ "$h" -gt "$warn_days" ] ||
    note "healthy-client is only $h days out, inside C1's $warn_days-day window — the negative test is a false negative"
  [ "$x" -lt 0 ] ||
    note "expired-client has not expired at $now — C1's 'expired' branch and the renderer's negative-duration guard are untested"
fi
# --- WHAT C1 NEEDS TO BE TRUE END ---

# Key material never survives generation (REQUIREMENTS § DevSecOps), and a leak
# does not leave git history — so it is checked while it is still preventable.
if grep -rlE -- "-----BEGIN [A-Z ]*PRIVATE KEY-----" "$certs" 2>/dev/null | grep .; then
  note "a private key is committed under tests/fixtures/certs (listed above)"
fi

if [ $fail -eq 0 ]; then
  echo "certs-test: dates pinned at $now (src/rules_tests.rs pins the same instant) — expiring $e days (C1 warns), healthy $h days (C1 silent), expired $x days (C1 says expired); no key material"
fi
exit $fail
