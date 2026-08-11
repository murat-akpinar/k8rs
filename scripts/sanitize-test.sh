#!/usr/bin/env bash
# The sanitizer is a security control, so it is tested like one: feed it an
# object poisoned with every kind of secret it is supposed to remove, and
# assert none of them survive. A sanitizer with no test is a hope
# (todo.md, Phase 2 § Security gate).
#
# Fed in **both shapes the capture actually produces** — a single object, and
# the `List` that `kubectl get <kind> -A -o json` returns. The first version of
# this test only ever fed it a Pod, and the sanitizer was a near no-op on every
# List fixture for exactly that reason: a test that only covers the shape that
# was already working cannot fail (CLAUDE.md § Tests must not lie).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
filter="$here/sanitize.jq"
command -v jq >/dev/null || { echo "sanitize-test: jq is not installed"; exit 127; }

fail=0

# name:description pairs, shared by every shape — the same secrets are planted
# in each, so a shape that quietly skips the filter shows up immediately.
must_be_gone=(
  "hunter2-in-an-annotation:the last-applied-configuration annotation"
  "someone@example.com:a private annotation"
  "sk-live-0123456789abcdef:an env value"
  "tok_live_initcontainer:an init container env value"
  "corp-registry-pull-token:an imagePullSecret"
  "BEGIN RSA PRIVATE KEY:a private key in a status message"
  "fieldsV1:managedFields"
  "/api/v1/namespaces:selfLink"
  "172.18.0.4:a node address"
  "10.244.2.2:a pod IP"
  # Both of these used to survive: the filter anchored its match to the whole
  # string, and neither of these *is* the whole string. Documentation ranges
  # (RFC 5737) on purpose — this file is public, so it must not carry a real one.
  "203.0.113.7:an address quoted inside an English message"
  "198.51.100.0:an address carrying a CIDR suffix"
  # A private key that never appears as text. Kubernetes stores every Secret
  # value base64-encoded, so `.data["tls.key"]` is the shape a real key arrives
  # in — and the PEM rule above looks for `-----BEGIN`, which base64 does not
  # contain. Needle is the encoded header, so a decode is not needed to spot it.
  "LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0t:a base64-wrapped private key"
)
must_remain=(
  # A certificate is the public half by definition, and C3's own fixture is a
  # base64 CSR — destroying it would leave `.spec.request` unparseable as the
  # ByteString it is typed as. Only the key half is secret.
  "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0t:a base64-wrapped certificate (public, and the CSR fixture is made of one)"
  "db-creds:the secretKeyRef a rule reports"
  "ingress-tls:the Secret volume name"
  "API_TOKEN:the env variable name (the value is what is secret, not the name)"
  "k8rs-worker:the node name the N-series rules join on"
)

# --- ASSERTIONS START ---
assert_clean() { # $1 = shape name, stdin = the object
  local shape=$1 clean entry needle what
  if ! clean=$(jq -f "$filter"); then
    echo "FAIL  [$shape] the sanitizer refused an object captured from kind"
    fail=1
    return
  fi
  for entry in "${must_be_gone[@]}"; do
    needle=${entry%%:*}; what=${entry#*:}
    if grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [$shape] $what survived sanitization"
      fail=1
    fi
  done
  # References must survive: a rule needs to say *which* Secret a pod reads.
  for entry in "${must_remain[@]}"; do
    needle=${entry%%:*}; what=${entry#*:}
    if ! grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [$shape] $what was destroyed — the fixture is now useless"
      fail=1
    fi
  done
}

assert_refused() { # $1 = shape name, stdin = the object
  local shape=$1
  if jq -f "$filter" >/dev/null 2>&1; then
    echo "FAIL  [$shape] a capture from a foreign cluster was sanitized instead of refused"
    fail=1
  fi
}
# --- ASSERTIONS END ---

# --- SHAPE: a single object, as `kubectl get pod X -o json` returns it ---
# It carries a Secret's `data` block, which a real Pod would not: every needle
# above has to be present in every shape, and base64 is the only encoding in
# which key material ever reaches a capture. The List below reuses it as a real
# Secret item.
pod=$(cat <<'JSON'
{
  "kind": "Pod",
  "metadata": {
    "name": "poisoned",
    "generateName": "poisoned-",
    "selfLink": "/api/v1/namespaces/default/pods/poisoned",
    "managedFields": [{"manager": "kubectl", "fieldsV1": {"f:spec": {}}}],
    "annotations": {
      "kubectl.kubernetes.io/last-applied-configuration":
        "{\"env\":[{\"name\":\"DB_PASSWORD\",\"value\":\"hunter2-in-an-annotation\"}]}",
      "internal.example.com/oncall": "someone@example.com"
    }
  },
  "spec": {
    "nodeName": "k8rs-worker",
    "imagePullSecrets": [{"name": "corp-registry-pull-token"}],
    "initContainers": [
      {"name": "migrate", "env": [{"name": "MIGRATION_TOKEN", "value": "tok_live_initcontainer"}]}
    ],
    "containers": [
      {
        "name": "app",
        "env": [
          {"name": "API_TOKEN", "value": "sk-live-0123456789abcdef"},
          {"name": "DB_HOST", "valueFrom": {"secretKeyRef": {"name": "db-creds", "key": "host"}}}
        ]
      }
    ],
    "volumes": [
      {"name": "tls", "secret": {"secretName": "ingress-tls"}}
    ]
  },
  "data": {
    "tls.key": "LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0tCk1JSUV2Z0lCQURBTkJna3Foa2lHOXcwQkFRRUZBQVNDCi0tLS0tRU5EIFBSSVZBVEUgS0VZLS0tLS0K",
    "tls.crt": "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0tCk1JSURhekNDQWxPZ0F3SUJBZ0lVZEdWemRBPT0KLS0tLS1FTkQgQ0VSVElGSUNBVEUtLS0tLQo="
  },
  "status": {
    "podIP": "10.244.2.2",
    "podIPs": [{"ip": "10.244.2.2"}],
    "hostIP": "172.18.0.4",
    "conditions": [
      {"type": "Ready", "message": "-----BEGIN RSA PRIVATE KEY-----\nMIIEow==\n-----END RSA PRIVATE KEY-----"},
      {"type": "ContainersReady", "message": "Readiness probe failed: dial tcp 203.0.113.7:6443: connect: connection refused"}
    ]
  }
}
JSON
)

# --- SHAPE: a List, as `kubectl get deployments -A -o json` returns it ---
# Same secrets, one level deeper, plus the pod template nesting a workload adds
# and a Node carrying its own identity. Half of `just fixtures` looks like this.
list=$(jq -n --argjson pod "$pod" '
  { "apiVersion": "v1",
    "kind": "List",
    "metadata": {"resourceVersion": "1"},
    "items": [
      { "kind": "Deployment",
        "metadata": $pod.metadata,
        "spec": {"template": {"metadata": $pod.metadata, "spec": $pod.spec}} },
      { "kind": "Secret", "type": "kubernetes.io/tls",
        "metadata": {"name": "ingress-tls"},
        "data": $pod.data },
      { "kind": "Node",
        "metadata": {"name": "k8rs-worker", "annotations": {"internal.example.com/oncall": "someone@example.com"}},
        "spec": {"podCIDR": "198.51.100.0/24", "podCIDRs": ["198.51.100.0/24"]},
        "status": {"nodeInfo": {"kubeletVersion": "v1.36.1"},
                   "conditions": [{"type": "Ready", "message": "Kubelet stopped posting node status: dial tcp 203.0.113.7:10250: i/o timeout"}],
                   "addresses": [{"type": "InternalIP", "address": "172.18.0.4"},
                                 {"type": "Hostname", "address": "k8rs-worker"}]} }
    ] }')

assert_clean "single object" <<<"$pod"
assert_clean "List"          <<<"$list"

# And a capture that did not come from the kind test cluster must be refused,
# rather than quietly producing something that only looks sanitized — in both
# shapes, because the List is the one that used to slip through.
assert_refused "single object" <<<'
{"kind":"Pod","metadata":{"name":"prod-api-7d4"},
 "spec":{"nodeName":"ip-10-3-44-201.eu-west-1.compute.internal"}}'

assert_refused "List" <<<'
{"kind":"List","items":[
  {"kind":"Node","metadata":{"name":"ip-10-3-44-201.eu-west-1.compute.internal"},
   "status":{"nodeInfo":{"kubeletVersion":"v1.36.1"}}}]}'

# A kind node name buried in an otherwise-foreign capture must not launder it.
assert_refused "mixed List" <<<'
{"kind":"List","items":[
  {"kind":"Pod","metadata":{"name":"a"},"spec":{"nodeName":"k8rs-worker"}},
  {"kind":"Pod","metadata":{"name":"b"},"spec":{"nodeName":"ip-10-3-44-201.eu-west-1.compute.internal"}}]}'

if [ $fail -eq 0 ]; then
  echo "sanitize-test: single object and List — every planted secret removed, every reference kept, foreign capture refused"
fi
exit $fail
