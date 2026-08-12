#!/usr/bin/env bash
# Certificate fixtures for the C-series rules, generated locally — a real
# cluster's client certificate never enters this repository, and the key
# material here is throwaway by construction (REQUIREMENTS § DevSecOps).
#
# The dates are **pinned, not relative**. A fixture generated with `-days 20`
# is a test that passes today and fails in three weeks, and the usual repair
# for that is to weaken the test. `Snapshot` carries `now`
# (NOTES § D18), so the test states the date it is asking about and the
# fixture never expires:
#
#   reference now  = 2026-08-13
#   expiring cert  = notAfter 2026-09-05  → 23 days left  → C1 warns (< 30)
#   healthy cert   = notAfter 2027-08-12  → 364 days left → C1 says nothing
#
# Both are self-signed: C1 reads `notAfter` out of the kubeconfig's
# `client-certificate-data`, and a chain would prove nothing extra.
#
# Pinning is a claim about the committed bytes, so it is asserted there rather
# than trusted here: `scripts/certs-test.sh` reads the three PEMs and checks
# these exact dates on every `just check`. Change a date below and the build is
# red until it changes there too. Needs openssl >= 3.5 for -not_before /
# -not_after; on anything older the flags are rejected, loudly (see gen).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$here/../tests/fixtures/certs"
mkdir -p "$out"

# openssl writes the key before it validates the dates, so a rejected date
# leaves key material sitting in the fixture directory and `set -e` skips the
# `rm` below. The repo is the one place it must never survive.
trap 'rm -f "$out"/*.key.pem' EXIT

gen() {
  local name=$1 cn=$2 not_before=$3 not_after=$4
  # -quiet drops the key-generation progress dots; stderr is *not* discarded,
  # because the failures that land there are the ones worth reading — a
  # malformed date, or an openssl older than 3.5 that has no -not_after and
  # would send the next reader reaching for the relative -days.
  openssl req -quiet -x509 -newkey rsa:2048 -nodes \
    -keyout "$out/$name.key.pem" \
    -out "$out/$name.crt.pem" \
    -subj "/CN=$cn/O=k8rs-fixtures" \
    -not_before "$not_before" -not_after "$not_after" \
    -sha256
  # The private key is written because openssl insists on one; it is not a
  # fixture and nothing reads it, so it does not survive this script.
  rm -f "$out/$name.key.pem"
  printf '  %-18s notAfter %s\n' "$name" "$(openssl x509 -in "$out/$name.crt.pem" -noout -enddate | cut -d= -f2)"
}

gen expiring-client "kubernetes-admin-expiring" 20260812000000Z 20260905000000Z
gen healthy-client  "kubernetes-admin"          20260812000000Z 20270812000000Z

# A certificate that is already past its notAfter: C1 must say "expired", not
# "expires in -4 days", and the renderer must not produce a negative duration.
gen expired-client  "kubernetes-admin-expired"  20250812000000Z 20260809000000Z

echo "certificate fixtures written to tests/fixtures/certs (dates pinned; they do not expire)"
