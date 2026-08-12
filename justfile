# k8rs task runner. `just check` runs every step CI runs except the
# cross-compile matrix, which is `just cross` — if the two can drift, the local
# run stops meaning anything.
#
# Every target is declared here, in Phase 1, including the ones later phases
# use: a target invented later is a forward-only violation (NOTES § D14, D26).

# CI sets this job-wide, so every rustc it runs denies warnings — including
# `cargo test`, which runs without `--all-features`. Setting it only on the
# clippy line left anything that warns in the default feature set invisible
# locally and red on push, which is the drift this file exists to prevent.
export RUSTFLAGS := "-D warnings"

default:
    @just --list

# --- the loop you run all day ---

# fmt + clippy + tests + the guards. Every step CI runs except the cross-compile
# matrix, which is `just cross` because it needs rustup — nothing else CI runs is
# missing here, and nothing here is skipped by CI. The two drifted once already
# (the self-test below and cargo-deny were CI-only, so `cargo deny` first failed
# on a push nobody could have caught locally).
# cargo-deny runs last: it needs `cargo install cargo-deny`, and when it is
# missing you still want the fifteen checks above it to have reported.
check:
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-targets
    python3 scripts/check-docs.py --self-test
    python3 scripts/check-docs.py
    python3 scripts/screens-check.py --self-test
    python3 scripts/screens-check.py
    python3 scripts/test-guard.py --self-test
    python3 scripts/test-guard.py
    python3 scripts/write-guard.py --self-test
    python3 scripts/write-guard.py
    bash scripts/verify-test.sh
    bash scripts/sanitize-test.sh
    bash scripts/certs-test.sh
    bash scripts/fixture-audit.sh --self-test
    bash scripts/fixture-audit.sh
    cargo deny check advisories licenses sources bans

# The one thing CI runs that `check` does not, because it needs `rustup target
# add` and rustup is not everywhere cargo is. Kept as its own recipe rather
# than silently dropped: cross-compilation breaks at link time and it breaks
# late, so the step has to be nameable and runnable by hand.
#
# Cross-compile check for every release target
cross:
    #!/usr/bin/env bash
    set -euo pipefail
    for t in x86_64-unknown-linux-musl aarch64-unknown-linux-musl \
             x86_64-apple-darwin aarch64-apple-darwin; do
      cargo check --locked --target "$t" --all-targets
    done

# Run the binary with the given arguments
run *ARGS:
    cargo run -- {{ARGS}}

# Phase 3 turns this on for rules.rs, Phase 4 for analysis.rs (NOTES § D26).
#
# Mutation testing over the two pure files: a surviving mutant is a diagnosis change no test objected to
mutants:
    cargo mutants --timeout 90 --file src/rules.rs --file src/analysis.rs

# --- the test cluster (scripts/cluster.sh does the work) ---

# Bring up the three-node kind test cluster
cluster-up:
    scripts/cluster.sh up

# Tear the kind test cluster down
cluster-down:
    scripts/cluster.sh down

# scripts/sanitize.jq runs on every object, never after the fact, and
# `cluster.sh verify` runs first: a fixture that never reached the state its
# rule is about is a test that cannot fail.
#
# When the k8s-openapi feature is bumped, re-capture against a matching kind
# version — the K8S_VERSION stamp is what makes that drift visible.
#
# Capture every fixture from the running kind cluster, sanitized before it is written
fixtures:
    #!/usr/bin/env bash
    set -euo pipefail
    ctx="kind-${K8RS_CLUSTER:-k8rs}"
    kc=(kubectl --context "$ctx")
    jqs=(jq -f scripts/sanitize.jq)
    mkdir -p tests/fixtures

    scripts/cluster.sh verify
    bash scripts/sanitize-test.sh

    # Rule 12 needs a pod that is Terminating and stays that way: the delete is
    # part of the capture, not of `cluster.sh break`.
    "${kc[@]}" delete pod broken-stuck --wait=false --ignore-not-found
    # `--wait=false` returns once the API server accepts the DELETE, not once the
    # object shows it. Rule 12 reads deletionTimestamp, so a capture taken before
    # it appears is a fixture the rule cannot fire on — assert it, do not hope.
    "${kc[@]}" get pod broken-stuck -o json \
      | jq -e '.metadata.deletionTimestamp != null and ((.metadata.finalizers // []) | length > 0)' >/dev/null \
      || { echo "fixtures: broken-stuck is not Terminating behind a finalizer — rule 12 has no fixture" >&2; exit 1; }

    for p in oom crashloop image config pending hostpath readiness restarts nolimits stuck init; do
      "${kc[@]}" get pod "broken-$p" -o json | "${jqs[@]}" > "tests/fixtures/$p.json"
    done

    # D36: the one broken pod that has an owner — every other pod capture above
    # is a bare pod, so the grouping key's workload branches have no positive
    # fixture at all. A Deployment's pod has a generated name, so this one is
    # fetched by label and lands as a List; the ReplicaSet beside it is what
    # carries the second half of the chain, the ownerReference naming the
    # Deployment (the Deployment itself is already in deployments.json).
    "${kc[@]}" get pods -l app=broken-owned -o json | "${jqs[@]}"        > tests/fixtures/owned-pods.json
    "${kc[@]}" get replicasets -l app=broken-owned -o json | "${jqs[@]}" > tests/fixtures/owned-replicasets.json
    # A label that matches nothing writes `{"items":[]}` and says nothing about
    # it — "extracted nothing" and "nothing to extract" print the same line, and
    # the owner is the whole reason these two files exist.
    for f in owned-pods owned-replicasets; do
      jq -e '[.items[]? | select(([.metadata.ownerReferences[]? | select(.controller==true)] | length) > 0)] | length > 0' \
        "tests/fixtures/$f.json" >/dev/null \
        || { echo "fixtures: $f.json carries no controlling ownerReference — the owner is what this capture is for" >&2; exit 1; }
    done

    # D39/D46: the one namespace nothing in broken.yaml can imitate. kubelet
    # writes an ownerReference of kind Node onto every static pod — the only
    # shape in Kubernetes that makes a Node an owner, and where N2's
    # `mirror: true` comes from — and every CNI/CSI/node agent beside them
    # mounts a writable hostPath perfectly legitimately, which is rule 8's
    # entire false-positive class. Captured whole rather than trimmed: a
    # hand-edited fixture is not a capture.
    "${kc[@]}" get pods -n kube-system -o json | "${jqs[@]}" > tests/fixtures/kube-system-pods.json
    # Asserted after the filter has run, so this covers both halves: a capture
    # of the wrong namespace, and a sanitizer that learned to destroy one of
    # the three things the file exists for. Either writes perfectly valid JSON
    # that proves nothing, and "found none" reads exactly like "there were
    # none". (`kubernetes.io/config.mirror` is deliberately not among these —
    # the filter destroys every annotation, which is why D46 takes the mirror
    # bit off the ownerReference instead.)
    # `.controller == true` on both owner clauses, for the reason the owned-*
    # guard above has it: a non-controlling reference is a garbage-collection
    # link and says nothing about who writes the pod, so a Node reference that
    # does not control does not exempt the pod from N2's count (D46). Asserting
    # the looser claim would pass a capture that yields zero mirror pods.
    ks_wants=(
      'pod owned by a controlling Node (the static-pod shape D39 rules on, and N2 reads as mirror)|[.items[] | select(any(.metadata.ownerReferences[]?; .kind == "Node" and .controller == true))] | length > 0'
      'pod owned by a controlling DaemonSet with a writable hostPath (rule 8 false-positive class)|[.items[] | select(any(.metadata.ownerReferences[]?; .kind == "DaemonSet" and .controller == true)) | [.spec.volumes[]? | select(.hostPath) | .name] as $hp | .spec.containers[].volumeMounts[]? | select(.readOnly != true) | select(.name as $n | $hp | index($n))] | length > 0'
      'read-only hostPath mount (the half of rule 8 that stays out of Alerts)|[.items[] | [.spec.volumes[]? | select(.hostPath) | .name] as $hp | .spec.containers[].volumeMounts[]? | select(.readOnly == true) | select(.name as $n | $hp | index($n))] | length > 0'
    )
    for want in "${ks_wants[@]}"; do
      jq -e "${want#*|}" tests/fixtures/kube-system-pods.json >/dev/null \
        || { echo "fixtures: kube-system-pods.json carries no ${want%%|*} — that is what this capture is for" >&2; exit 1; }
    done

    # The negative side. Every rule needs a healthy counterpart or its
    # false-positive test is fiction.
    "${kc[@]}" get pod healthy -o json | "${jqs[@]}" > tests/fixtures/healthy.json
    # W1 and W2 read a ReplicaSet, so their negative has to be one too — the
    # healthy Deployment in deployments.json cannot show the absence of a
    # ReplicaFailure condition that only ever appears on the ReplicaSet.
    "${kc[@]}" get replicasets -l app=healthy-deploy -o json | "${jqs[@]}" > tests/fixtures/healthy-replicasets.json

    # W1: no pod exists at all — the truth is on the ReplicaSet.
    "${kc[@]}" get deployment broken-quota -n k8rs-quota -o json | "${jqs[@]}"       > tests/fixtures/quota-deployment.json
    "${kc[@]}" get replicasets -n k8rs-quota -o json | "${jqs[@]}"       > tests/fixtures/quota-replicasets.json

    # The cluster-wide snapshot analysis.rs reports are computed from.
    for kind in nodes deployments statefulsets daemonsets services persistentvolumeclaims poddisruptionbudgets; do
      "${kc[@]}" get "$kind" -A -o json | "${jqs[@]}" > "tests/fixtures/$kind.json"
    done

    "${kc[@]}" version -o json | jq -r .serverVersion.gitVersion > tests/fixtures/K8S_VERSION
    echo "captured $(ls tests/fixtures | wc -l) fixtures from $(cat tests/fixtures/K8S_VERSION)"

# Body lands in Phase 7, the target is declared now.
#
# End-to-end write path against kind, in --read-only mode and with the operations enabled
e2e:
    @echo "not yet — Phase 7 writes this recipe (ops.rs against a real cluster)"
    @exit 1
