#!/usr/bin/env bash
# Every scripts/ guard, in one list: its --self-test where it has one, then its
# real run. Two callers name this file and neither keeps a copy of what is in
# it — `just guards` (and `just check` through it), and CI's single
# `bash scripts/guards.sh` step. Adding a guard is one edit: one line below.
#
# It is a script and not the body of the `guards` recipe because CI would then
# need `just` on the runner, and `just` has no other use there — a third-party
# action bought purely to reach an entry point, against the rule that a
# dependency is asked for and not reflexed (NOTES § D111, CLAUDE.md invariant
# 10). When a fix needs a new dependency to reach an entry point, the entry
# point is the thing to change: a shebang costs nothing.
#
# Shell and Python guards sit in one list on purpose. Splitting them by language
# would leave the shell half needing a list of its own, which is the same drift
# in a different coat — nobody adding a guard thinks about what it is written
# in, only about what it checks.
#
# `set -x` is what lets CI run all of this as one step: the last traced line on
# a red build is the guard that failed. It also keeps every guard visible
# locally, the way it was when `check` listed them itself and `just` echoed each
# line — a step name buys a label the trace already gives at 3am.
set -euo pipefail

# Both callers already run from the repository root; this makes the relative
# paths below true for a human who typed the path from somewhere else.
cd "$(dirname "$0")/.."

# The one way the drift this file closes reopens: a guard written into `just
# check` instead of into the list below. So `check`'s recipe body is read out of
# the justfile — with its comments dropped, so the prose around it cannot
# satisfy the match — and asserted to name no script at all. The justfile is
# read as text rather than through `just --show`, because the whole point of
# this file is that CI does not have `just`.
#
# The `guards` clause doubles as the extraction's canary: an empty read fails it,
# so the `scripts/` clause can never pass by vetting nothing (CLAUDE.md § A
# derived list asserts it found something). `|| true` is what lets that empty
# read reach the message — grep exits 1 when it selects no lines, and under
# `set -e` that would kill the script with no explanation at all.
#
# Ceiling: `*guards*` is a substring, so a `check` that called some other recipe
# with `guards` in its name would satisfy it. That is deliberate — the clause's
# job is to make an empty read fail, and it cannot be fooled into passing on one.
# A pattern pinned to the exact line would go red on a reformat, and a gate that
# is red for nothing is one people learn to wave through.
body=$(awk '/^check:/{f=1;next} f && /^[^[:space:]]/{exit} f' justfile \
       | grep -v '^[[:space:]]*#' || true)
case "$body" in *guards*) ;; *)
  echo "guards: the justfile's 'check' recipe no longer calls 'just guards' — either it was renamed, or the extraction above read nothing and was about to vet nothing" >&2; exit 1 ;; esac
case "$body" in *scripts/*)
  echo "guards: 'just check' runs a scripts/ guard directly. Move that line into scripts/guards.sh — it is the one list CI runs, and a guard only 'check' knows about never runs on a push" >&2; exit 1 ;; esac

set -x
# First, because it decides whether anything above it meant anything. `just
# check` ran fmt, clippy and the tests before reaching this file, and all three
# are only as good as the toolchain that ran them: CI asked for `toolchain:
# stable`, stable moved twice in one phase, and clippy 1.98's `result_large_err`
# was a red push the local gate could not reproduce for seven days
# (NOTES § D211). On the runner this asserts the action honoured the pin.
python3 scripts/toolchain-guard.py --self-test
python3 scripts/toolchain-guard.py
python3 scripts/check-docs.py --self-test
python3 scripts/check-docs.py
python3 scripts/todo-guard.py --self-test
python3 scripts/todo-guard.py
python3 scripts/screens-check.py --self-test
python3 scripts/screens-check.py
# Green tests are not the same as tests that ran (NOTES § D26).
python3 scripts/test-guard.py --self-test
python3 scripts/test-guard.py
# Invariant 1, as an allowlist derived from the kube version in the lock file —
# never a hand-written denylist. And the other half of the same invariant: that
# the one `#![allow(clippy::disallowed_methods)]` exempting `ops.rs` from it is
# still the only one in the crate. An allowed lint never fires, so clippy cannot
# report the file that turns it off.
python3 scripts/write-guard.py --self-test
python3 scripts/write-guard.py
# The mechanizable half of CLAUDE.md § Security gate — including CI itself: the
# action pins and the top-level permissions are checked by it, so a "quick CI
# fix" that unpins an action is a red build rather than a line nobody re-reads.
python3 scripts/security-guard.py --self-test
python3 scripts/security-guard.py
# Every fixture is trusted because a jq predicate in cluster.sh said it reached
# the state its rule is about. Those predicates only ever ran against a live
# cluster, where too-loose and too-tight look identical.
bash scripts/verify-test.sh
# The sanitizer is a security control: fed a poisoned object it must hand back
# nothing secret (todo.md, Phase 2 § Security gate).
bash scripts/sanitize-test.sh
# The certificate fixtures claim pinned dates; C1's positive and negative tests
# can only fail while that claim stays true (NOTES § D30).
bash scripts/certs-test.sh
# sanitize-test proves the filter; this proves the committed bytes. A fixture
# can reach tests/fixtures/ without ever meeting the filter.
bash scripts/fixture-audit.sh --self-test
bash scripts/fixture-audit.sh
# The mutation gate's own honesty: cargo-mutants files a failed build as
# `unviable`, so anything that breaks the build without being a mutation result
# reads as a pass — a full disk (NOTES § D133) and, since 2026-08-21, a lint
# denied by the toolchain flags. Self-test only — the real run is the phase-close
# sweep and is eleven minutes long.
bash scripts/mutants.sh --self-test
# The other half of D92's "a review cluster cannot produce a committed fixture":
# sanitize.jq anchors the refusal, and this is the loud early one, on the path
# that builds the cluster in the first place.
bash scripts/cluster.sh --self-test
# `just e2e` needs a kind cluster and CI has none, so the only thing that ever
# runs on a push is this — and it is where the recipe's decisions live: which
# context it refuses, which failure is loud, and whether it vetted a single row
# (NOTES § D236 ruling 2). A `just e2e` that exits 0 because there was no cluster
# is the invisible gap, and it is invisible on exactly the machines CI runs on.
bash scripts/e2e.sh --self-test
# reports/ carries real cluster output into a committed file with no filter in
# front of it (NOTES § D108). This is that filter, for prose rather than JSON.
python3 scripts/reports-guard.py --self-test
python3 scripts/reports-guard.py
# `rules.rs` is frozen and its constants are private, so `k8s.rs` keeps a second
# copy of each threshold both layers need — five minutes of clock skew, thirty
# days of certificate. Each file's tests pin only its own copy, so a re-tune of
# one is green with the two out of step (measured for `CERT_EXPIRY_WARN`,
# 2026-08-28: 639 tests pass with `k8s.rs`'s at sixty days).
python3 scripts/twin-guard.py --self-test
python3 scripts/twin-guard.py
# `cargo fmt` reflows code and leaves comments alone, so the 100-column rule was
# a convention until this ran. rustfmt's own options for it are nightly-only.
python3 scripts/width-guard.py --self-test
python3 scripts/width-guard.py
