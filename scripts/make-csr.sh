#!/usr/bin/env bash
# The pending-CSR fixture for rule C3 (todo.md Phase 2), created deliberately —
# no cluster produces one by accident. kind's own CSRs are the two kubelet
# bootstrap requests plus a control-plane renewal, and all three arrive
# `Approved,Issued` (NOTES § Verified, item 7).
#
# **Why it stays Pending.** The signer is `kubernetes.io/kube-apiserver-client`.
# kube-controller-manager runs exactly one approver, `csrapproving`, and it only
# ever looks at `kubernetes.io/kube-apiserver-client-kubelet` requests from
# `system:bootstrappers` / `system:nodes`; every other signerName is left for a
# human. The signing controller only signs what an approver already approved, so
# nothing in a stock kind cluster can move this CSR. That is also the real
# operational shape C3 reports: someone requested a client certificate and
# nobody noticed. Which is a claim about the cluster, so it is asserted against
# the captured bytes below rather than trusted here.
#
# **The key does not survive.** openssl writes the private key before it
# validates anything, so a failure mid-run leaves key material behind and
# `set -e` skips the cleanup — make-certs.sh learned that the hard way. Here the
# key is never inside the repository at all: the whole working directory is a
# mktemp under $TMPDIR, removed by the trap on every exit path.
#
# **Not wired into `just fixtures`**, for the same reason make-certs.sh is not:
# a re-run mints a new keypair and a new creationTimestamp, so it would rewrite
# the fixture's bytes on every capture for no reason. Run it if the fixture is
# lost, or when the CSR API changes shape.
#
# The cluster is left as it was found: the CSR is deleted again once captured.
# Re-runnable — the name is fixed and a stale one is deleted before the create.
#
#   K8RS_SSH=user@host   run kubectl over ssh (the kind cluster is not local)
#   K8RS_CLUSTER=k8rs    kind cluster name, as everywhere else
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="$here/../tests/fixtures/csr-pending.json"
ctx="kind-${K8RS_CLUSTER:-k8rs}"
name="k8rs-pending-fixture"

# Every other script here assumes kubectl is on this machine. This one is the
# step that may not be: the cluster can live on another host entirely.
ssh_prefix=()
if [ -n "${K8RS_SSH:-}" ]; then ssh_prefix=(ssh -o BatchMode=yes "$K8RS_SSH"); fi
kc() { "${ssh_prefix[@]}" kubectl --context "$ctx" "$@"; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# --- THROWAWAY REQUEST START ---
# CN/O are fixture identities, not a cluster's. `kubernetes.io/kube-apiserver-client`
# puts no constraint on the subject (unlike the kubelet signer, which demands
# `system:node:<name>` in `system:nodes`) — and a name that cannot be mistaken
# for a real identity is the point.
openssl req -quiet -new -newkey rsa:2048 -nodes \
  -keyout "$work/fixture.key.pem" \
  -out "$work/fixture.csr.pem" \
  -subj "/CN=k8rs-fixture/O=k8rs-fixtures"
# --- THROWAWAY REQUEST END ---

kc delete csr "$name" --ignore-not-found >/dev/null

# `.spec.request` is base64 of the PEM block, not DER — the API server rejects
# anything else.
kc create -f - <<YAML >/dev/null
apiVersion: certificates.k8s.io/v1
kind: CertificateSigningRequest
metadata:
  name: $name
spec:
  request: $(base64 -w0 < "$work/fixture.csr.pem")
  signerName: kubernetes.io/kube-apiserver-client
  usages: ["client auth"]
YAML

# An approver is a watch, so it acts in milliseconds; the settle is only here so
# "nothing approved it" is a statement about a cluster that had the chance to.
sleep 5

# --- IT MUST BE PENDING START ---
# Captured once, asserted on those exact bytes, and the fixture is those bytes.
# Asserting against a second read would leave the possibility that the file
# written is not the object that was checked.
kc get csr "$name" -o json > "$work/csr.json"

if ! jq -e '.status.certificate == null and (.status.conditions // []) == []' "$work/csr.json" >/dev/null; then
  echo "make-csr: $name is not Pending — got $(jq -c '{conditions: [.status.conditions[]?.type], certificate: (.status.certificate != null)}' "$work/csr.json")" >&2
  echo "          something in this cluster approves $(jq -r .spec.signerName "$work/csr.json"); C3's fixture cannot come from here." >&2
  exit 1
fi
# --- IT MUST BE PENDING END ---

# Through scripts/sanitize.jq, exactly like `just fixtures` — a fixture that has
# not met the filter is not a fixture (REQUIREMENTS § DevSecOps, G-5).
jq -f "$here/sanitize.jq" "$work/csr.json" > "$out"

kc delete csr "$name" --ignore-not-found >/dev/null

printf 'tests/fixtures/csr-pending.json  signer %s  created %s  conditions: none (Pending)\n' \
  "$(jq -r .spec.signerName "$out")" "$(jq -r .metadata.creationTimestamp "$out")"
echo "the CSR was deleted from $ctx again — the fixture is the record, not the cluster"
