#!/usr/bin/env bash
# The sanitizer is a security control, so it is tested like one: feed it an
# object poisoned with every kind of secret it is supposed to remove, and
# assert none of them survive. A sanitizer with no test is a hope
# (todo.md, Phase 2 § Security gate).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
filter="$here/sanitize.jq"
command -v jq >/dev/null || { echo "sanitize-test: jq is not installed"; exit 127; }

poisoned=$(cat <<'JSON'
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
  "status": {
    "conditions": [{"type": "Ready", "message": "-----BEGIN RSA PRIVATE KEY-----\nMIIEow==\n-----END RSA PRIVATE KEY-----"}]
  }
}
JSON
)

clean=$(jq -f "$filter" <<<"$poisoned")

fail=0
must_be_gone=(
  "hunter2-in-an-annotation:the last-applied-configuration annotation"
  "someone@example.com:a private annotation"
  "sk-live-0123456789abcdef:an env value"
  "tok_live_initcontainer:an init container env value"
  "corp-registry-pull-token:an imagePullSecret"
  "BEGIN RSA PRIVATE KEY:a private key in a status message"
  "fieldsV1:managedFields"
  "/api/v1/namespaces:selfLink"
)
for entry in "${must_be_gone[@]}"; do
  needle=${entry%%:*}
  what=${entry#*:}
  if grep -qF -- "$needle" <<<"$clean"; then
    echo "FAIL  $what survived sanitization"
    fail=1
  fi
done

# References must survive: a rule needs to say *which* Secret a pod reads.
must_remain=(
  "db-creds:the secretKeyRef a rule reports"
  "ingress-tls:the Secret volume name"
  "API_TOKEN:the env variable name (the value is what is secret, not the name)"
  "k8rs-worker:the node name the N-series rules join on"
)
for entry in "${must_remain[@]}"; do
  needle=${entry%%:*}
  what=${entry#*:}
  if ! grep -qF -- "$needle" <<<"$clean"; then
    echo "FAIL  $what was destroyed — the fixture is now useless"
    fail=1
  fi
done

# And it must refuse an object that did not come from the kind test cluster,
# rather than quietly producing something that only looks sanitized.
foreign='{"kind":"Pod","metadata":{"name":"prod-api-7d4"},"spec":{"nodeName":"ip-10-3-44-201.eu-west-1.compute.internal"}}'
if jq -f "$filter" <<<"$foreign" >/dev/null 2>&1; then
  echo "FAIL  a pod from a foreign cluster was sanitized instead of refused"
  fail=1
fi

if [ $fail -eq 0 ]; then
  echo "sanitize-test: every planted secret removed, every reference kept, foreign capture refused"
fi
exit $fail
