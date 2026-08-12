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
#
# Usage:
#     fixture-audit.sh             # audit tests/fixtures/
#     fixture-audit.sh <dir>       # audit some other tree (the self-test uses this)
#     fixture-audit.sh --self-test # prove the guard fails when it should
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
command -v jq >/dev/null || { echo "fixture-audit: jq is not installed"; exit 127; }
command -v openssl >/dev/null || { echo "fixture-audit: openssl is not installed"; exit 127; }
tmpd=$(mktemp -d); trap 'rm -rf "$tmpd"' EXIT

# --- SELF-TEST START ---
# A guard nobody has seen fail is not a guard (todo.md, Phase 1) — and this one
# was green over a real private key for the whole of Phase 2, in a line
# byte-identical to a clean run. So the six framings are planted here, one at a
# time, and the script is re-run over them: the thing under test is this file
# itself, not a copy of its logic.
if [ "${1:-}" = "--self-test" ]; then
  d=$tmpd/f; mkdir -p "$d/certs"
  # An EC key on purpose: it is the *smallest* private key anything real
  # produces, so the length thresholds below are exercised at their edge
  # instead of by a comfortably large RSA one.
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$tmpd/k.pem" 2>/dev/null
  openssl pkey -in "$tmpd/k.pem" -outform DER -out "$tmpd/k.der"
  openssl req -x509 -key "$tmpd/k.pem" -subj /CN=selftest -days 1 -out "$d/certs/c.crt.pem" 2>/dev/null
  echo '{"kind":"Pod","metadata":{"name":"a"},"spec":{"nodeName":"k8rs-worker"}}' > "$d/p.json"
  echo "v1.36.1" > "$d/K8S_VERSION"
  b64=$(base64 -w0 "$tmpd/k.pem")

  sfail=0
  expect() { # $1 = the exit status this tree must produce, $2 = what it is
    local out rc=0
    out=$(bash "$0" "$d" 2>&1) || rc=$?
    if [ "$rc" -ne "$1" ]; then
      echo "FAIL  self-test: $2 — expected exit $1, got $rc"
      printf '%s\n' "$out" | sed 's/^/      /'
      sfail=1
    fi
  }

  expect 0 "a clean tree: a real certificate in certs/ is public and must pass"

  cat "$d/certs/c.crt.pem" "$tmpd/k.pem" > "$d/certs/x.crt.pem"
  expect 1 "armoured: a key appended to a certificate, in the directory this check used to skip"
  rm "$d/certs/x.crt.pem"

  { cat "$d/certs/c.crt.pem"; echo "# client-key-data: $b64"; } > "$d/certs/x.crt.pem"
  expect 1 "base64-wrapped: how a kubeconfig carries client-key-data"
  rm "$d/certs/x.crt.pem"

  cp "$tmpd/k.der" "$d/certs/x.der"
  expect 1 "DER: no armour at all, so there is nothing to grep for"
  rm "$d/certs/x.der"

  { echo "-----BEGIN CERTIFICATE-----"; sed -e 1d -e '$d' "$tmpd/k.pem"; echo "-----END CERTIFICATE-----"; } > "$d/certs/x.crt.pem"
  expect 1 "mislabeled: key bytes under a CERTIFICATE header"
  rm "$d/certs/x.crt.pem"

  base64 "$tmpd/k.pem" > "$d/wrapped.txt"
  expect 1 "whole-file base64: wrapped at 76 columns, so no single line decodes"
  rm "$d/wrapped.txt"

  jq -n --arg k "$b64" '{kind:"Secret",data:{"tls.key":$k}}' > "$d/secret.json"
  expect 1 "inside JSON: the shape every Secret value arrives in"
  rm "$d/secret.json"

  if [ $sfail -eq 0 ]; then
    echo "fixture-audit: self-test passed — a private key is refused armoured," \
         "base64-wrapped, DER, mislabeled, whole-file base64 and inside JSON;" \
         "a real certificate is not"
  fi
  exit $sfail
fi
# --- SELF-TEST END ---

fixtures="${1:-$here/../tests/fixtures}"

# Recursive, and JSON is only the shape that gets *parsed*. A key does not stop
# being a key because it was written as `admin.key.pem`, a kubeconfig, or one
# directory down — and "no key material" was printed over exactly that.
files=() all_files=()
while IFS= read -r -d '' f; do
  all_files+=("$f")
  case "$f" in *.json) files+=("$f") ;; esac
done < <(find "$fixtures" -type f -print0 2>/dev/null | sort -z)

if [ ${#all_files[@]} -eq 0 ]; then
  echo "fixture-audit: no fixtures captured yet — nothing to audit. The capture" \
       "lands in Phase 2 (\`just fixtures\`); this guard is what checks it when it does."
  exit 0
fi

fail=0
note() { echo "FAIL  $*"; fail=1; }

# --- KEY MATERIAL, IN EVERY FRAMING IT ARRIVES IN START ---
# A regex proves only the framing it was written for (NOTES § D31), and a
# private key arrives in four:
#
#   armoured     -----BEGIN PRIVATE KEY-----        a grep sees this one
#   base64       LS0tLS1CRUdJTi…                    a kubeconfig's
#                                                   client-key-data, and every
#                                                   Secret value
#   DER          raw bytes, no armour at all        nothing to grep for
#   mislabeled   key bytes under a CERTIFICATE      the header lies
#                header
#
# Only the first was checked, and certs/ — the one directory where key material
# is actually generated — was exempted even from that, because certs/ holds
# *certificates* and those are public. The exemption covered the private key
# sitting next to one just as well: a key appended to expiring-client.crt.pem
# read as "no key material", in a line byte-identical to a clean run.
#
# So openssl decides, not a regex. It is already required by certs-test.sh and
# make-certs.sh, so this adds no dependency. `-passin pass:` is not optional: an
# encrypted key would otherwise prompt on the terminal and hang `just check`.
armour='-----BEGIN [A-Z ]*PRIVATE KEY-----'

is_key() { # $1 = file of raw bytes; prints the framing, 0 when it holds a key
  LC_ALL=C grep -qaE -- "$armour" "$1" && { echo "armoured"; return 0; }
  # DER is asked first because `-in` without `-inform` auto-detects both, so a
  # DER key answered to the PEM branch and got reported as "PEM". A framing
  # named wrongly is a small lie in the one line someone will read at 3am.
  openssl pkey -in "$1" -inform DER -passin pass: -noout >/dev/null 2>&1 && { echo "DER"; return 0; }
  openssl pkey -in "$1" -passin pass: -noout >/dev/null 2>&1 && { echo "PEM"; return 0; }
  return 1
}

decoded_is_key() { # $1 = base64 text
  printf '%s' "$1" | base64 -d > "$tmpd/decoded" 2>/dev/null || return 1
  is_key "$tmpd/decoded" >/dev/null
}

any_is_key() { # stdin = one base64 candidate per line
  local body
  while IFS= read -r body; do
    if [ ${#body} -ge 64 ] && decoded_is_key "$body"; then return 0; fi
  done
  return 1
}

has_key() { # $1 = file; prints the framing it was found in, 0 when found
  local f=$1 what
  what=$(is_key "$f") && { echo "$what"; return 0; }
  # Every PEM body, whatever its header claims to be — openssl reads the label,
  # the label is the part whoever wrote the file controls, and the bytes are the
  # part that is secret. Anything reaching here already failed the armour grep,
  # so a body that decodes to a key is by definition under the wrong header.
  if any_is_key < <(awk '/-----BEGIN /{c=1;b="";next} /-----END /{if(c)print b; c=0; next} c{b=b $0}' "$f" 2>/dev/null); then
    echo "under a PEM header that does not say PRIVATE KEY"; return 0
  fi
  # Every long base64 run: one `client-key-data:` line, one Secret value, one
  # JSON string.
  if any_is_key < <(LC_ALL=C grep -oaE '[A-Za-z0-9+/]{100,}={0,2}' "$f" 2>/dev/null); then
    echo "base64-wrapped"; return 0
  fi
  # A file that is *entirely* base64, wrapped at 64 or 76 columns — what
  # `base64 key.pem > f` leaves behind. Each line on its own decodes to noise,
  # so the runs above cannot see it; only the file joined back up can.
  if ! LC_ALL=C grep -qav '^[A-Za-z0-9+/=]*$' "$f" &&
     decoded_is_key "$(LC_ALL=C tr -d '[:space:]' < "$f")"; then
    echo "base64-wrapped, whole file"; return 0
  fi
  return 1
}
# --- KEY MATERIAL, IN EVERY FRAMING IT ARRIVES IN END ---

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
  # Unanchored on purpose: an address is just as readable with a `/24` after it
  # or an English sentence around it, and both shapes reached tests/fixtures/
  # while this check was anchored to the whole string.
  "IP addresses|[.. | strings | select(test(\"([0-9]{1,3}\\\\.){3}[0-9]{1,3}\"))]"
  "PEM blocks|[.. | strings | select(test(\"-----BEGIN [A-Z ]*(PRIVATE KEY|CERTIFICATE)-----\"))]"
  # The same material base64-wrapped, which is how every Secret value arrives —
  # the regex above cannot see it, because base64 contains no \`-----BEGIN\`.
  # Certificates are left alone: they are public, and csr-pending.json is one.
  "base64 key material|[.. | strings | select(test(\"^LS0tLS1CRUdJ\") and test(\"^[A-Za-z0-9+/]+={0,2}\$\") and (length % 4) == 0 and (@base64d | test(\"-----BEGIN [A-Z ]*PRIVATE KEY-----\")))]"
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

# Every file, in every framing, with nothing exempted. A private key does not
# become safe by being written as `admin.key.pem`, tucked inside a kubeconfig,
# base64-wrapped, stripped of its armour, or appended to a certificate in the
# one directory this check used to skip.
for file in "${all_files[@]}"; do
  rel=${file#"$fixtures"/}
  if framing=$(has_key "$file"); then
    note "[$rel] contains a private key ($framing) — key material never leaves the machine that made it"
  fi
done

# Node identity is in every pod fixture, not only nodes.json — ten of them carry
# `spec.nodeName`, and none of them used to be checked. Same rule as the
# sanitizer's: a foreign name is refused, never rewritten.
for file in "${files[@]}"; do
  rel=${file#"$fixtures"/}
  while read -r n; do
    [ -z "$n" ] && continue
    case "$n" in k8rs-*) ;; *) note "[$rel] names node '$n', which is not from the kind test cluster" ;; esac
  done < <(jq -r '[.. | objects | .nodeName? // empty] | .[]' "$file" 2>/dev/null)
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
  # Both counts, because they are different claims: the JSON checks reach
  # ${#files[@]} files and the key-material check reaches all of them. One
  # number covering both is how "no key material" got printed over a directory
  # the scan never entered.
  echo "fixture-audit: ${#all_files[@]} committed fixtures (${#files[@]} parsed as JSON) —" \
       "no annotations, no env values, no addresses; no key material in any framing" \
       "(armoured, base64-wrapped, DER, mislabeled); node names intact"
fi
exit $fail
