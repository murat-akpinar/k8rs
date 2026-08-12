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
  "hunter2-in-an-annotation|the last-applied-configuration annotation"
  "someone@example.com|a private annotation"
  "sk-live-0123456789abcdef|an env value"
  "tok_live_initcontainer|an init container env value"
  "corp-registry-pull-token|an imagePullSecret"
  "BEGIN RSA PRIVATE KEY|a private key in a status message"
  "fieldsV1|managedFields"
  "/api/v1/namespaces|selfLink"
  "172.18.0.4|a node address"
  "10.244.2.2|a pod IP"
  # Both of these used to survive: the filter anchored its match to the whole
  # string, and neither of these *is* the whole string. Documentation ranges
  # (RFC 5737) on purpose — this file is public, so it must not carry a real one.
  "203.0.113.7|an address quoted inside an English message"
  "198.51.100.0|an address carrying a CIDR suffix"
  # A private key that never appears as text. Kubernetes stores every Secret
  # value base64-encoded, so `.data["tls.key"]` is the shape a real key arrives
  # in — and the PEM rule above looks for `-----BEGIN`, which base64 does not
  # contain. Needle is the encoded header, so a decode is not needed to spot it.
  "LS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0t|a base64-wrapped private key"
  # IPv6 written out in full carries no `::` to anchor on.
  "2001:0db8:0000:0000:0000:ff00:0042:8329|a fully expanded IPv6 address"
)
must_remain=(
  # A certificate is the public half by definition, and C3's own fixture is a
  # base64 CSR — destroying it would leave `.spec.request` unparseable as the
  # ByteString it is typed as. Only the key half is secret.
  "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0t|a base64-wrapped certificate (public, and the CSR fixture is made of one)"
  "db-creds|the secretKeyRef a rule reports"
  "ingress-tls|the Secret volume name"
  "API_TOKEN|the env variable name (the value is what is secret, not the name)"
  "k8rs-worker|the node name the N-series rules join on"
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
    needle=${entry%%|*}; what=${entry#*|}
    if grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [$shape] $what survived sanitization"
      fail=1
    fi
  done
  # References must survive: a rule needs to say *which* Secret a pod reads.
  for entry in "${must_remain[@]}"; do
    needle=${entry%%|*}; what=${entry#*|}
    if ! grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [$shape] $what was destroyed — the fixture is now useless"
      fail=1
    fi
  done
}

# A non-zero exit is not enough to call something refused: jq exits non-zero on
# a type error too, so a filter that *crashes* on an input reads exactly like
# one that turned it away on purpose, and a test that accepts either proves only
# that something went wrong. The refusal has to be the one this file writes, so
# the message is asserted, not only the status.
assert_refused() { # $1 = shape name, stdin = the object
  local shape=$1 err rc=0
  err=$(jq -f "$filter" 2>&1 >/dev/null) || rc=$?
  if [ $rc -eq 0 ]; then
    echo "FAIL  [$shape] a capture from a foreign cluster was sanitized instead of refused"
    fail=1
  elif ! grep -q '^jq: error.*sanitize: ' <<<"$err"; then
    echo "FAIL  [$shape] the filter failed on this input instead of refusing it: $err"
    fail=1
  fi
}

# The other half of a refusal: what the kind cluster itself produces has to keep
# passing. A refusal that also fires on kind's own captures is not a guard, it
# is a capture trip that cannot run — and for the pod and workload fixtures that
# arrives mid-`just fixtures`, whose redirect has already truncated the file it
# was about to write.
assert_accepted() { # $1 = shape name, stdin = the object
  local shape=$1 err
  if ! err=$(jq -f "$filter" 2>&1 >/dev/null); then
    echo "FAIL  [$shape] the sanitizer refused a capture the kind test cluster produces: $err"
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
    "podIPs": [{"ip": "10.244.2.2"}, {"ip": "2001:0db8:0000:0000:0000:ff00:0042:8329"}],
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

# The other four places a node name lives. `.nodeName` is where it is obvious;
# these are where it hides, and a capture is just as identifying through any of
# them. Each is asserted on its own so a partial fix cannot pass.
assert_refused "nominatedNodeName" <<<'
{"kind":"Pod","metadata":{"name":"prod-api-7d4"},
 "status":{"nominatedNodeName":"ip-10-3-44-203.eu-west-1.compute.internal"}}'

assert_refused "nodeSelector hostname" <<<'
{"kind":"Pod","metadata":{"name":"prod-api-7d4"},
 "spec":{"nodeSelector":{"kubernetes.io/hostname":"ip-10-3-44-201.eu-west-1.compute.internal"}}}'

assert_refused "hostname label" <<<'
{"kind":"List","items":[
  {"kind":"Pod","metadata":{"name":"prod-api-7d4",
   "labels":{"kubernetes.io/hostname":"ip-10-3-44-204.eu-west-1.compute.internal"}}}]}'

assert_refused "nodeAffinity matchExpressions" <<<'
{"kind":"Pod","metadata":{"name":"prod-api-7d4"},
 "spec":{"affinity":{"nodeAffinity":{"requiredDuringSchedulingIgnoredDuringExecution":
   {"nodeSelectorTerms":[{"matchExpressions":[
     {"key":"kubernetes.io/hostname","operator":"In",
      "values":["ip-10-3-44-202.eu-west-1.compute.internal"]}]}]}}}}}'

# A kind node name buried in an otherwise-foreign capture must not launder it.
assert_refused "mixed List" <<<'
{"kind":"List","items":[
  {"kind":"Pod","metadata":{"name":"a"},"spec":{"nodeName":"k8rs-worker"}},
  {"kind":"Pod","metadata":{"name":"b"},"spec":{"nodeName":"ip-10-3-44-201.eu-west-1.compute.internal"}}]}'

# --- CSR REQUESTER IDENTITY START ---
# A CertificateSigningRequest names who asked for the certificate, in
# `.spec.username` and `.spec.groups`. That is a *reference* to a real person or
# service account, not a payload, so it takes the node treatment — refused,
# never rewritten (NOTES § D52). On a real cluster those fields carry an OIDC
# email or `system:serviceaccount:prod/deployer`; the committed fixture carries
# kind's own `kubernetes-admin`, so nothing has leaked, and that is luck rather
# than a guard.
#
# What must pass was read off the pinned kind cluster (kindest/node:v1.36.1) on
# 2026-08-12 rather than recalled — `kubectl get csr -o json` returned exactly
# these two identity shapes, and the admin comes from the kubeconfig client
# certificate (`O=kubeadm:cluster-admins, CN=kubernetes-admin`), which is who
# make-csr.sh runs as. A refusal that fires on kind's own CSRs would stop the
# capture trip, so the positive side is asserted as hard as the negative one.
assert_accepted "kind's own CSRs (List)" <<<'
{"apiVersion":"v1","kind":"List","items":[
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-4gvvv"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client-kubelet",
           "username":"system:bootstrap:abcdef",
           "groups":["system:bootstrappers","system:bootstrappers:kubeadm:default-node-token","system:authenticated"]}},
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-6q7hz"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client-kubelet",
           "username":"system:node:k8rs-control-plane",
           "groups":["system:nodes","system:authenticated"]}}]}'

# The negative side, and it is a real capture rather than a written one: the
# committed fixture must still pass **and come back unchanged**. Refused-not-
# rewritten cuts both ways — kind's own identity is kept exactly like a `k8rs-`
# node name is, or C3's fixture stops saying who asked.
csr_fixture="$here/../tests/fixtures/csr-pending.json"
if [ -f "$csr_fixture" ]; then
  # One branch, not a refusal check followed by a `|| true`: an empty result
  # from a refused fixture made every needle below report that the identity had
  # been *rewritten*, which is a second failure line explaining the wrong thing
  # about the first one.
  if csr_clean=$(jq -f "$filter" "$csr_fixture" 2>&1); then
    for entry in "kubernetes-admin|the requester the CSR fixture was created by" \
                 "kubeadm:cluster-admins|the group kubeadm's own admin.conf carries"; do
      needle=${entry%%|*}; what=${entry#*|}
      if ! grep -qF -- "$needle" <<<"$csr_clean"; then
        echo "FAIL  [csr-pending.json] $what was rewritten — an identity is refused or kept, never mangled"
        fail=1
      fi
    done
  else
    echo "FAIL  [csr-pending.json] the committed CSR fixture is refused by the filter that made it: $csr_clean"
    fail=1
  fi
else
  echo "FAIL  [csr-pending.json] the committed CSR fixture is missing — the identity refusal has nothing to prove its negative side against"
  fail=1
fi

# Foreign username, every group allowed: the username clause on its own.
assert_refused "CSR username, single object" <<<'
{"kind":"CertificateSigningRequest","metadata":{"name":"csr-1"},
 "spec":{"signerName":"kubernetes.io/kube-apiserver-client",
         "username":"system:serviceaccount:prod/deployer",
         "groups":["system:authenticated"]}}'

assert_refused "CSR username, List" <<<'
{"apiVersion":"v1","kind":"List","items":[
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-ok"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client","username":"kubernetes-admin",
           "groups":["kubeadm:cluster-admins","system:authenticated"]}},
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-2"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client",
           "username":"alice@corp.example.com","groups":["system:authenticated"]}}]}'

# One foreign entry among good ones, which is the shape `.spec.groups` actually
# arrives in — `system:authenticated` is on every request ever made, so a rule
# that asked whether *all* entries are foreign would pass every real leak.
assert_refused "CSR group, single object" <<<'
{"kind":"CertificateSigningRequest","metadata":{"name":"csr-3"},
 "spec":{"signerName":"kubernetes.io/kube-apiserver-client","username":"kubernetes-admin",
         "groups":["kubeadm:cluster-admins","oidc:platform-oncall","system:authenticated"]}}'

assert_refused "CSR group, List" <<<'
{"apiVersion":"v1","kind":"List","items":[
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-ok"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client-kubelet",
           "username":"system:node:k8rs-worker2","groups":["system:nodes","system:authenticated"]}},
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-4"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client","username":"kubernetes-admin",
           "groups":["system:authenticated","engineering@corp.example.com"]}}]}'

# `.spec.groups` is typed as an array of strings, so this shape cannot come off
# an apiserver — it is here because the two obvious ways to read that field both
# fail on it, in opposite directions: `[]` aborts jq mid-capture, and a
# `type == "array"` guard drops the identity without a word. Neither is allowed
# to be the thing nobody notices.
assert_refused "CSR groups that is not the array it is typed as" <<<'
{"kind":"CertificateSigningRequest","metadata":{"name":"csr-6"},
 "spec":{"signerName":"kubernetes.io/kube-apiserver-client","username":"kubernetes-admin",
         "groups":"system:serviceaccount:prod/deployer"}}'

# The framing question (NOTES § D31), asked of the refusal instead of a
# redaction: a real identity that *begins* with one kind issues. `startswith` is
# right for node names, where `k8rs-` is a family; here it would launder an SSO
# account into an allowed one.
assert_refused "CSR username that only starts like kind's" <<<'
{"kind":"CertificateSigningRequest","metadata":{"name":"csr-5"},
 "spec":{"signerName":"kubernetes.io/kube-apiserver-client",
         "username":"kubernetes-admin@corp.example.com","groups":["system:authenticated"]}}'

# `.spec.extra` and `.spec.uid` are the other half of the same object, and they
# are payload rather than reference: no rule reads either, `extra` is where a
# real cluster puts its OIDC claims, and `uid` is the auth provider's identifier
# for a real person. They cannot take the refusal the two fields above take —
# the credential-id the apiserver stamps on every request is different on every
# capture, so there is nothing to allowlist — so they are deleted.
#
# Deleted *only* on an object carrying `signerName`, which is what the kept list
# below is really testing: `metadata.uid` is on every object in Kubernetes and
# the rule engine's identity is built on it, so a filter that reached for `.uid`
# by name would quietly destroy all 23 fixtures at once.
csr_poisoned=$(cat <<'JSON'
{
  "kind": "CertificateSigningRequest",
  "metadata": {"name": "k8rs-pending-fixture", "uid": "13d7ece0-9271-4045-9d84-f3da549f9ee0"},
  "spec": {
    "signerName": "kubernetes.io/kube-apiserver-client",
    "username": "kubernetes-admin",
    "groups": ["kubeadm:cluster-admins", "system:authenticated"],
    "uid": "8f14e45f-ceea-467a-9f0e-2c0d1f6f9a11",
    "extra": {
      "authentication.kubernetes.io/credential-id": ["X509SHA256=a939c612226ff53e"],
      "oidc.corp.example.com/email": ["sre@corp.example.com"]
    },
    "request": "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURSBSRVFVRVNULS0tLS0K",
    "usages": ["client auth"]
  },
  "status": {"conditions": [{"type": "Approved", "reason": "AutoApproved"}]}
}
JSON
)
csr_gone=(
  "X509SHA256=a939c612226ff53e|the credential id the apiserver stamps on every request"
  "sre@corp.example.com|an OIDC claim, which is what .spec.extra carries on a real cluster"
  "8f14e45f-ceea-467a|the auth provider's uid for the person who asked"
)
csr_kept=(
  # A deletion is only correct if it is narrow, so what must survive is asserted
  # harder than what must go.
  "13d7ece0-9271-4045-9d84-f3da549f9ee0|metadata.uid, which the rule engine's object identity is built on"
  "AutoApproved|the status condition C3 decides Pending on"
  "LS0tLS1CRUdJTiBDRVJUSUZJQ0FURSBSRVFVRVNULS0tLS0K|the request itself, which is the whole object"
  "kubernetes-admin|kind's own requester — refused or kept, never rewritten"
)
# cert-manager's CertificateRequest is the same object under another name: the
# same `username` / `groups` / `uid` / `extra`, with `issuerRef` where the
# Kubernetes kind has `signerName`. C4 is a cert-manager rule, so this kind is
# on the roadmap rather than hypothetical — and keyed on `signerName` alone it
# went through the filter untouched.
cm_poisoned=$(jq '.apiVersion = "cert-manager.io/v1" | .kind = "CertificateRequest"
                  | .spec.issuerRef = {"name":"corp-ca","kind":"ClusterIssuer"}
                  | del(.spec.signerName)' <<<"$csr_poisoned")
for shape in "single object" "List" "cert-manager CertificateRequest"; do
  case $shape in
    "List") doc=$(jq -n --argjson c "$csr_poisoned" '{apiVersion:"v1",kind:"List",items:[$c]}')
            marker="kubernetes.io/kube-apiserver-client|the signerName rule C3 reads" ;;
    "cert-manager"*) doc=$cm_poisoned
            marker="corp-ca|the issuerRef, which is what this kind has instead of a signerName" ;;
    *) doc=$csr_poisoned
       marker="kubernetes.io/kube-apiserver-client|the signerName rule C3 reads" ;;
  esac
  if ! clean=$(jq -f "$filter" <<<"$doc"); then
    echo "FAIL  [CSR payload/$shape] the sanitizer refused a CSR whose requester is kind's own"
    fail=1
    continue
  fi
  for entry in "${csr_gone[@]}"; do
    needle=${entry%%|*}; what=${entry#*|}
    if grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [CSR payload/$shape] $what survived sanitization"
      fail=1
    fi
  done
  for entry in "${csr_kept[@]}" "$marker"; do
    needle=${entry%%|*}; what=${entry#*|}
    if ! grep -qF -- "$needle" <<<"$clean"; then
      echo "FAIL  [CSR payload/$shape] $what was destroyed — the deletion is not narrow"
      fail=1
    fi
  done
done

# The same requester refusal, on the same two fields, one kind over.
assert_refused "cert-manager CertificateRequest username, single object" <<<'
{"apiVersion":"cert-manager.io/v1","kind":"CertificateRequest","metadata":{"name":"api-tls"},
 "spec":{"issuerRef":{"name":"corp-ca","kind":"ClusterIssuer"},
         "username":"alice@corp.example.com","groups":["system:authenticated"]}}'

assert_refused "cert-manager CertificateRequest group, List" <<<'
{"apiVersion":"v1","kind":"List","items":[
  {"apiVersion":"cert-manager.io/v1","kind":"CertificateRequest","metadata":{"name":"ok"},
   "spec":{"issuerRef":{"name":"corp-ca"},"username":"kubernetes-admin",
           "groups":["kubeadm:cluster-admins","system:authenticated"]}},
  {"apiVersion":"cert-manager.io/v1","kind":"CertificateRequest","metadata":{"name":"api-tls"},
   "spec":{"issuerRef":{"name":"corp-ca"},"username":"kubernetes-admin",
           "groups":["system:authenticated","oidc:platform-oncall"]}}]}'

# The two identifier rules read the same document and must agree about what a
# node name is. A node name is a DNS subdomain, so `k8rs-worker.lan` is one:
# `refuse_foreign_nodes` has always accepted it, and the identity clause used to
# refuse the kubelet that owns it — on this very object.
assert_accepted "a node name with a dot, in both rules at once" <<<'
{"apiVersion":"v1","kind":"List","items":[
  {"kind":"Pod","metadata":{"name":"a"},"spec":{"nodeName":"k8rs-worker.lan"}},
  {"kind":"CertificateSigningRequest","metadata":{"name":"csr-7"},
   "spec":{"signerName":"kubernetes.io/kube-apiserver-client-kubelet",
           "username":"system:node:k8rs-worker.lan","groups":["system:nodes","system:authenticated"]}}]}'
# --- CSR REQUESTER IDENTITY END ---

if [ $fail -eq 0 ]; then
  echo "sanitize-test: single object and List — every planted secret removed, every reference kept, foreign capture and foreign requester identity refused"
fi
exit $fail
