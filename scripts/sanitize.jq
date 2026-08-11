# Fixture sanitization (REQUIREMENTS § DevSecOps, G-5). Applied to every object
# before it is written to tests/fixtures/ — a leak never leaves git history.
#
# Two different jobs, deliberately not mixed up:
#
#   1. Payloads are destroyed. managedFields, every annotation (especially
#      last-applied-configuration, which is a full copy of the spec with env
#      values in it), env[].value, imagePullSecrets, and anything shaped like a
#      PEM block. References are kept — a rule needs to know that a pod reads
#      the Secret `db-creds`, never what is inside it.
#
#   2. Node identifiers are refused, not rewritten. Node names carry real
#      infrastructure, and a fixture whose node names were mangled would break
#      the pod↔node joins the N-series rules are built on. So an object that
#      *carries a node identifier* — a Node, or anything with `spec.nodeName` —
#      is refused outright unless that identifier came from the kind test
#      cluster, instead of quietly producing something that looks safe.
#      Objects with no node identifier (a Deployment, a Service) are sanitized
#      normally; there is nothing there to refuse.
#      Fixtures come from kind — that is a decision, not a habit (NOTES § Settled).

def refuse_foreign_nodes:
  ( [ .. | objects | (.nodeName? // empty), (.metadata?.name? // empty) ]
    | map(select(type == "string")) ) as $names
  | if ($names | any(startswith("k8rs-") | not) and ($names | length) > 0)
       and (($names | map(select(startswith("k8rs-"))) | length) == 0)
       and (.kind == "Node" or (.spec?.nodeName? // "") != "")
    then error("sanitize: node identifiers are not from the kind test cluster "
               + "(expected names starting with k8rs-). Fixtures come from kind; "
               + "refusing to write one captured somewhere else.")
    else .
    end;

def strip_pem:
  walk(
    if type == "string" and test("-----BEGIN [A-Z ]*(PRIVATE KEY|CERTIFICATE)-----")
    then "REDACTED-PEM"
    else .
    end
  );

refuse_foreign_nodes
| del(.metadata.managedFields, .metadata.annotations)
| del(.. | objects | .managedFields?)
| ( .spec.containers[]?, .spec.initContainers[]?, .spec.ephemeralContainers[]?,
    .spec.template?.spec?.containers[]?, .spec.template?.spec?.initContainers[]? )
  |= ( if .env then .env |= map(if has("value") then .value = "REDACTED" else . end) else . end )
| del(.spec.imagePullSecrets, .spec.template?.spec?.imagePullSecrets)
| del(.metadata.selfLink, .metadata.generateName)
| strip_pem
