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

run *ARGS:
    cargo run -- {{ARGS}}

# Mutation testing over the two pure files — a surviving mutant is a diagnosis
# change no test objected to. Phase 3 turns this on for rules.rs, Phase 4 for
# analysis.rs (NOTES § D26).
mutants:
    cargo mutants --timeout 90 --file src/rules.rs --file src/analysis.rs

# --- the test cluster (scripts/cluster.sh does the work) ---

cluster-up:
    scripts/cluster.sh up

cluster-down:
    scripts/cluster.sh down

# Capture fixtures from the running kind cluster, sanitized before they are
# written. The recipe body lands in Phase 2, where the fixtures do; the target
# exists now so the justfile can freeze there (NOTES § D14).
fixtures:
    @echo "not yet — Phase 2 writes this recipe (capture + sanitize + K8S_VERSION stamp)"
    @exit 1

# End-to-end write path against kind, in --read-only mode as well as with the
# operations enabled. Body lands in Phase 7, the target is declared now.
e2e:
    @echo "not yet — Phase 7 writes this recipe (ops.rs against a real cluster)"
    @exit 1
