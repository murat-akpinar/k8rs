# k8rs task runner. `just check` is byte-for-byte what CI runs — if they can
# drift, the local run stops meaning anything.
#
# Every target is declared here, in Phase 1, including the ones later phases
# use: a target invented later is a forward-only violation (NOTES § D14, D26).

default:
    @just --list

# --- the loop you run all day ---

# fmt + clippy + tests + the guards. Identical to the CI `check` job.
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-targets
    python3 scripts/check-docs.py
    python3 scripts/test-guard.py
    python3 scripts/write-guard.py --self-test
    python3 scripts/write-guard.py
    bash scripts/sanitize-test.sh

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

    for p in oom crashloop image config pending hostpath readiness nolimits stuck init; do
      "${kc[@]}" get pod "broken-$p" -o json | "${jqs[@]}" > "tests/fixtures/$p.json"
    done

    # The negative side. Every rule needs a healthy counterpart or its
    # false-positive test is fiction.
    "${kc[@]}" get pod healthy -o json | "${jqs[@]}" > tests/fixtures/healthy.json

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
