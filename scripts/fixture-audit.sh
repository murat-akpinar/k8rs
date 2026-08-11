#!/usr/bin/env bash
# Audit the *committed* fixtures, not the filter that made them.
#
# `sanitize-test.sh` proves scripts/sanitize.jq removes what it is supposed to
# remove. That is a different question from whether the files in
# tests/fixtures/ are actually clean — a fixture can arrive there without ever
# meeting the filter: hand-edited, copied from a bug report, captured with an
# older sanitizer, or written by a `kubectl get` someone ran outside
# `just fixtures`. Phase 2's checklist covers that case with "eyeball every
# fixture once", and an eyeball step is not a guard: it passes whenever the
# person running it is tired, which at the end of a capture is everyone.
#
# So this asks the committed bytes directly. It is the last line before a leak
# is in git history for good (REQUIREMENTS G-5).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fixtures="$here/../tests/fixtures"
command -v jq >/dev/null || { echo "fixture-audit: jq is not installed"; exit 127; }

shopt -s nullglob
files=("$fixtures"/*.json)
if [ ${#files[@]} -eq 0 ]; then
  echo "fixture-audit: no fixtures captured yet — nothing to audit. The capture" \
       "lands in Phase 2 (\`just fixtures\`); this guard is what checks it when it does."
  exit 0
fi

fail=0
note() { echo "FAIL  $*"; fail=1; }

# --- WHAT MUST NOT BE THERE START ---
# Each entry: label | jq expression returning the offending values, over the
# whole document so a `List`'s .items[] and a workload's pod template are
# covered the same way a bare object is (NOTES § D29).
checks=(
  "annotations|[.. | objects | .annotations? | select(.)]"
  "managedFields|[.. | objects | .managedFields? | select(.)]"
  "selfLink|[.. | objects | .selfLink? | select(.)]"
  "imagePullSecrets|[.. | objects | .imagePullSecrets? | select(.)]"
  "env values|[.. | objects | select(.env? | type == \"array\") | .env[] | select(has(\"value\") and .value != \"REDACTED\")]"
  "IP addresses|[.. | strings | select(test(\"^([0-9]{1,3}\\\\.){3}[0-9]{1,3}$\"))]"
  "PEM blocks|[.. | strings | select(test(\"-----BEGIN [A-Z ]*(PRIVATE KEY|CERTIFICATE)-----\"))]"
)

for file in "${files[@]}"; do
  name=$(basename "$file")
  jq -e . "$file" >/dev/null 2>&1 || { note "[$name] is not valid JSON"; continue; }
  for entry in "${checks[@]}"; do
    what=${entry%%|*}; expr=${entry#*|}
    n=$(jq "$expr | length" "$file")
    [ "$n" -eq 0 ] || note "[$name] carries $n $what — this file never met scripts/sanitize.jq"
  done
done
# --- WHAT MUST NOT BE THERE END ---

# --- WHAT MUST STILL BE THERE START ---
# A fixture stripped of the references the rules read is not safe, it is
# useless — and it fails silently, because every rule then reports nothing.
if [ -f "$fixtures/nodes.json" ]; then
  names=$(jq -r '[.items[]?.metadata.name // empty] | join(" ")' "$fixtures/nodes.json")
  [ -n "$names" ] || note "[nodes.json] has no node names — the N-series rules join on these"
  for n in $names; do
    case "$n" in
      k8rs-*) ;;
      *) note "[nodes.json] node '$n' is not from the kind test cluster" ;;
    esac
  done
fi

# The capture stamps the server version it came from; without it nobody can
# tell whether a fixture predates a k8s-openapi bump.
[ -f "$fixtures/K8S_VERSION" ] || note "K8S_VERSION is missing — the fixtures record no cluster version"
# --- WHAT MUST STILL BE THERE END ---

if [ $fail -eq 0 ]; then
  echo "fixture-audit: ${#files[@]} committed fixtures — no annotations, no env values," \
       "no addresses, no key material; node names intact"
fi
exit $fail
