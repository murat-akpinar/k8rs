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
      # Where the scheduler parks a name it has not committed to yet.
      (.nominatedNodeName? // empty),
      # `kubernetes.io/hostname` is the node's name under another key, and it
      # turns up in three unrelated places: a pod's `nodeSelector`, any
      # object's `labels`, and the `values[]` of a nodeAffinity
      # `matchExpressions` entry. All three identify a node exactly as well as
      # `.nodeName` does, and none of them used to be looked at.
      (.["kubernetes.io/hostname"]? // empty),
      (select(.key? == "kubernetes.io/hostname") | (.values? // [])[]),
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
    # `has` is only defined on objects, and `.env` is not always an array of
    # them: Argo- and Tekton-shaped CRDs use plain `"NAME=value"` strings. The
    # type check is not tidiness — without it jq aborts with exit 5, and
    # `just fixtures` has already truncated the target file by then.
    then .env |= map(
      if type == "object" and has("value") then .value = "REDACTED"
      # The CRD form carries its value in the same string as its name, so the
      # name is kept up to the first `=` and everything after it goes.
      elif type == "string" then (split("=")[0] + "=REDACTED")
      else . end)
    else .
    end)

| walk(
    if type == "string" and test("-----BEGIN [A-Z ]*(PRIVATE KEY|CERTIFICATE)-----")
    then "REDACTED-PEM"
    else .
    end)

# The same key material, in the encoding it actually arrives in. Every Secret
# value is base64 in JSON, and base64 contains no `-----BEGIN`, so the rule
# above reads a `.data["tls.key"]` as ordinary text and hands it straight back.
#
# Only the **key** half is redacted. A certificate is the public half by
# definition, and `.spec.request` on a CSR is typed as a ByteString — C3's own
# fixture is one, and destroying it would leave the fixture unparseable.
#
# Decoding is guarded, not attempted: jq's @base64d is a hard error on a string
# it cannot decode, which would abort the whole capture. A string only reaches
# it after matching the encoded PEM header, the base64 alphabet, and a length
# that divides into whole groups.
#
# The replacement is deliberately *not* valid base64. Nothing in this project
# has a legitimate private key in a fixture, so the only object this can fire on
# is one that should never have been captured — and it should fail loudly at the
# next parse rather than deserialize into a tidy placeholder nobody looks at.
| walk(
    if type == "string"
       and test("^LS0tLS1CRUdJ")
       and test("^[A-Za-z0-9+/]+={0,2}$")
       and (length % 4) == 0
       and (@base64d | test("-----BEGIN [A-Z ]*PRIVATE KEY-----"))
    then "REDACTED-PEM-BASE64"
    else .
    end)

# Addresses go too — `status.addresses[].address`, `podIP`, `hostIP`,
# `clusterIP`. The eyeball step in todo.md Phase 2 asks for "no node IPs", and
# an eyeball step is not a guard: it passes whenever someone is tired. No rule
# in the plan reads an address — the N-series joins on node *names*, which is
# why those are kept and refused rather than rewritten.
#
# IPv4 is replaced *inside* strings, not only when it is the whole one. The two
# shapes that anchoring missed are both ordinary: `"10.244.0.0/24"` (a podCIDR —
# an address wearing a suffix) and `"dial tcp 10.0.0.1:6443: connection
# refused"` (an address quoted in an English message, which is where kubelet
# puts the one it could not reach). Neither is covered by refusing foreign node
# names: this cluster's own nodes are called `k8rs-*` no matter what address the
# apiserver was given, so a capture from a kind cluster reachable on a real LAN
# passes the refusal and carries that LAN address out in a message.
#
# A Hostname entry (`k8rs-worker`, sitting in the same `addresses` array as an
# InternalIP) is untouched for the reason it always was — it contains no
# address, so there is nothing here to match.
#
# IPv6 stays anchored to the whole string on purpose: unanchored, `::` matches
# a Rust path, a C++ scope operator and every `key::value` in a log line.
| walk(
    if type != "string" then .
    # Two IPv6 forms: compressed (the `::` run) and written out in full, which
    # carries no `::` at all and so matched neither branch.
    elif test("^[0-9a-fA-F:]*::[0-9a-fA-F:]*$")
      or test("^[0-9a-fA-F]{1,4}(:[0-9a-fA-F]{1,4}){7}$") then "REDACTED-IP"
    else gsub("(?<ip>([0-9]{1,3}\\.){3}[0-9]{1,3})"; "REDACTED-IP")
    end)
