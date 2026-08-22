#!/usr/bin/env bash
# Certificate fixtures for the C-series rules, generated locally — a real
# cluster's client certificate never enters this repository, and the key
# material here is throwaway by construction (REQUIREMENTS § DevSecOps).
#
# The dates are **pinned, not relative**. A fixture generated with `-days 20`
# is a test that passes today and fails in three weeks, and the usual repair
# for that is to weaken the test. `Snapshot` carries `now` (NOTES § D18), so
# the test states the date it is asking about and the fixture never expires:
# one certificate inside C1's warning window, one far outside it, one already
# past its notAfter.
#
# **Which is which, in days and from when, is not written here.** That moves
# with every repin, and a copy nothing compares is a lie with a delay on it
# (NOTES § D57). Pinning is in any case a claim about the committed bytes and
# not about the intent in this file, so both halves are asserted where the
# bytes are read: `scripts/certs-test.sh` checks these exact dates against the
# three PEMs on every `just check`, and is the one place holding the reference
# `now` and — in its `pinned[]` — the days left at it. Change a date below and
# the build is red until it changes there too — the dates here are absolute,
# so a repin changes what the certificates *mean* and never the bytes this
# script writes.
#
# All three are self-signed: C1 reads `notAfter` out of the kubeconfig's
# `client-certificate-data`, and a chain would prove nothing extra.
#
# Needs openssl >= 3.5 for -not_before / -not_after; on anything older the
# flags are rejected, loudly (see gen).
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

# A certificate that is already past its notAfter: C1 must say "expired" rather
# than counting down past zero, and the renderer must not produce a negative
# duration.
gen expired-client  "kubernetes-admin-expired"  20250812000000Z 20260809000000Z

echo "certificate fixtures written to tests/fixtures/certs (dates pinned; they do not expire)"
