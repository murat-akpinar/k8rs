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
#      the pod↔node joins the N-series rules are built on. So a capture
#      carrying a node identifier from anywhere other than the kind test
#      cluster is refused outright, instead of quietly producing something that
#      looks safe. Fixtures come from kind — that is a decision, not a habit
#      (NOTES § Settled).
#
# **Everything here walks the whole document, never a fixed path.** Half the
# capture is `kubectl get <kind> -A -o json`, which is a `List`: its objects sit
# under `.items[]`, their pod templates two levels below that, and a filter
# written against `.metadata.annotations` scrubs the wrapper and leaves every
# real object untouched. That is the exact shape a path-based filter passes
# silently, so there is no path-based clause left in this file.

# Node identity, wherever it hides: `.nodeName` on any pod spec, and the
# `.metadata.name` of a Node. A Node is recognised by `.status.nodeInfo`, which
# no other kind has — `.kind` alone is not enough, because the items of a List
# do not always carry one.
def node_names:
  [ .. | objects
    | (.nodeName? // empty),
      (select((.kind? == "Node") or (.status?.nodeInfo? != null))
       | .metadata?.name? // empty) ]
  | map(select(type == "string"));

# Refused if *any* identifier is foreign, not only if all of them are: one real
# node name inside an otherwise-kind capture is the leak, and it is also the
# shape a half-migrated context produces.
def refuse_foreign_nodes:
  (node_names | map(select(startswith("k8rs-") | not)) | unique) as $foreign
  | if ($foreign | length) > 0
    then error("sanitize: node identifiers are not from the kind test cluster "
               + "(expected names starting with k8rs-, got \($foreign[0:3])). "
               + "Fixtures come from kind; refusing to write a capture from "
               + "anywhere else.")
    else .
    end;

refuse_foreign_nodes

# Payloads, at every depth: object metadata, a List's items, a workload's pod
# template — all the same walk.
| del(.. | objects | .managedFields?, .annotations?, .selfLink?,
                     .generateName?, .imagePullSecrets?)

# The env *name* stays, the value goes: a rule reports which variable is unset,
# never what was in it.
| walk(
    if type == "object" and (.env | type) == "array"
    then .env |= map(if has("value") then .value = "REDACTED" else . end)
    else .
    end)

| walk(
    if type == "string" and test("-----BEGIN [A-Z ]*(PRIVATE KEY|CERTIFICATE)-----")
    then "REDACTED-PEM"
    else .
    end)

# Addresses go too — `status.addresses[].address`, `podIP`, `hostIP`,
# `clusterIP`. The eyeball step in todo.md Phase 2 asks for "no node IPs", and
# an eyeball step is not a guard: it passes whenever someone is tired. No rule
# in the plan reads an address — the N-series joins on node *names*, which is
# why those are kept and refused rather than rewritten.
#
# Matched as a whole string, so a Hostname entry (`k8rs-worker`, sitting in the
# same `addresses` array as an InternalIP) survives untouched. An address quoted
# *inside* an English message is not caught; the foreign-capture refusal above
# is what covers a capture from a cluster that is not kind.
| walk(
    if type == "string" and test("^([0-9]{1,3}\\.){3}[0-9]{1,3}$|^[0-9a-fA-F:]*::[0-9a-fA-F:]*$")
    then "REDACTED-IP"
    else .
    end)
