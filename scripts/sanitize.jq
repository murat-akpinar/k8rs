# Fixture sanitization (REQUIREMENTS § DevSecOps, G-5). Applied to every object
# before it is written to tests/fixtures/ — a leak never leaves git history.
#
# Two different jobs, deliberately not mixed up:
#
#   1. Payloads are destroyed. managedFields, every annotation (especially
#      last-applied-configuration, which is a full copy of the spec with env
#      values in it), env[].value, imagePullSecrets, anything shaped like a PEM
#      block, and a certificate request's `.spec.extra` and `.spec.uid` — the
#      claims an identity provider attached to whoever asked. References are
#      kept — a rule needs to know that a pod reads the Secret `db-creds`,
#      never what is inside it.
#
#   2. Identifiers are refused, not rewritten — node names, and the requester a
#      CertificateSigningRequest names in `.spec.username` and `.spec.groups`.
#      Both are references rather than payloads: a fixture whose node names were
#      mangled would break the pod↔node joins the N-series rules are built on,
#      and a mangled requester would say a real person asked for a certificate
#      under a name nobody can trace. So a capture carrying either from anywhere
#      other than the kind test cluster is refused outright, instead of quietly
#      producing something that looks safe. Fixtures come from kind — that is a
#      decision, not a habit (NOTES § Settled).
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

# The other reference a capture carries: who asked for a certificate. It is
# recorded in `.spec.username` and `.spec.groups`, and on a real cluster that is
# an OIDC email or `system:serviceaccount:prod/deployer`.
#
# **Two kinds carry it, not one.** Kubernetes' own CertificateSigningRequest has
# `.spec.signerName`; cert-manager's CertificateRequest has `.spec.issuerRef`
# instead and the identical `username` / `groups` / `uid` / `extra` beside it —
# and it is on the roadmap, because C4 is a cert-manager rule. Keyed on either
# marker rather than on `.kind`, which is unreliable inside a List for the same
# reason node_names does not read it.
#
# Keyed on the marker and not on the payload: `has("request") and
# has("username")` reads like the better test — it needs no list of kinds — but
# it fails open exactly when it matters, because an object that carries `groups`
# and `extra` without a `username` is then not examined at all. The field that
# says *what this object is* is present whether or not the field being protected
# is.
#
# Every string *under* those two fields, rather than the array `.groups` is
# typed as: iterating it with `[]` is a hard jq error on anything else, while
# checking only `arrays` would let an unexpected shape past in silence, which is
# the worse half of the same choice. `..` has neither problem and is shorter
# than either. (A CSR never arrives through `just fixtures`, which captures no
# such kind — its one consumer is scripts/make-csr.sh, and that sanitizes into a
# temp file and moves it, so an abort here destroys nothing.)
def requester_identities:
  [ .. | objects | select(has("signerName") or has("issuerRef"))
    | (.username?, .groups?) | .. | strings ];

# Refused, not rewritten, for the reason node names are: an identity is a
# reference, and a rewritten one would say a real person asked for a
# certificate under a name nobody can trace.
#
# What passes is not a policy about which real identities are acceptable — that
# would be a judgement call re-made on every capture. It is what the cluster
# issues by itself, read off the pinned kind cluster rather than recalled: the
# two kubelet identities with their groups, and kubeadm's admin from the
# kubeconfig client certificate, `CN=kubernetes-admin, O=kubeadm:cluster-admins`,
# which is who scripts/make-csr.sh runs as.
#
# **Two limits, both real, neither engineered around.**
#
#   1. Only the `system:node:k8rs-…` entry is specific to *this* cluster.
#      `kubernetes-admin` in `kubeadm:cluster-admins` is what **every** kubeadm
#      cluster calls its admin, production included — so this list does not
#      prove a capture came from kind the way the `k8rs-` node prefix does. It
#      turns away somebody's *named* account, which is the leak worth turning
#      away, and admits an anonymous kubeadm admin.
#   2. A requester identity sits in three places on a CSR: these two fields, the
#      subject DN inside the base64 `.spec.request`, and the DN inside
#      `.status.certificate`. jq cannot decode a DER subject, so this reads one
#      of the three — a production CSR whose `.spec.request` carries
#      `CN=alice@corp.example.com` is accepted. Reading the other two needs
#      openssl, which means it belongs in fixture-audit.sh, not in a jq filter.
#
# Anchored at both ends, unlike the `k8rs-` node prefix. `k8rs-` is a family of
# names kind generates; an identity is not, and an unanchored match would let
# `kubernetes-admin@corp.example.com` through as kind's own admin.
def refuse_foreign_identities:
  (requester_identities
   | map(select(test("^(kubernetes-admin"
                     + "|kubeadm:cluster-admins"
                     + "|system:authenticated"
                     + "|system:nodes"
                     # A DNS subdomain, dots and all: `refuse_foreign_nodes`
                     # accepts `k8rs-worker.lan`, and the two rules reading the
                     # same document must not disagree about what a node is.
                     + "|system:node:k8rs-[a-z0-9.-]+"
                     + "|system:bootstrappers"
                     + "|system:bootstrappers:kubeadm:default-node-token"
                     # kind's kubeadm patch pins the bootstrap token, so the id
                     # is always `abcdef`. This is a fact about kind, not about
                     # kubeadm: a `kubeadm token create` id such as `9a08jv` is
                     # refused, and correctly so — it did not come from here.
                     + "|system:bootstrap:abcdef)$") | not))
   | unique) as $foreign
  | if ($foreign | length) > 0
    then error("sanitize: a certificate request names a requester the kind test "
               + "cluster does not issue (expected kubeadm's own admin, a "
               + "kubelet or a bootstrapper, got \($foreign[0:3])). That is a "
               + "real user or group; refusing to write a capture that names "
               + "one.")
    else .
    end;

refuse_foreign_nodes
| refuse_foreign_identities

# Payloads, at every depth: object metadata, a List's items, a workload's pod
# template — all the same walk.
| del(.. | objects | .managedFields?, .annotations?, .selfLink?,
                     .generateName?, .imagePullSecrets?)

# The rest of a CSR's requester, which is payload rather than reference and so
# is destroyed instead of refused: `.spec.extra` is where a real cluster puts
# its OIDC claims, `.spec.uid` is the auth provider's identifier for a real
# person, and no rule reads either. Neither could take the refusal above anyway
# — the `credential-id` the apiserver stamps on every request is a fresh hash
# each time, so there is nothing to allowlist.
#
# Scoped to the object that carries the marker, deliberately: `metadata.uid` is
# on every object in Kubernetes and the rule engine's identity is built on it,
# so a `del(.. | objects | .uid?)` would empty all 23 fixtures at once while
# still passing every test written about a CSR.
| walk(if type == "object" and (has("signerName") or has("issuerRef"))
       then del(.extra, .uid) else . end)

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
