# k8rs task runner. `just check` runs every step CI runs — if the two can
# drift, the local run stops meaning anything. The last row where they still
# disagreed was the cross-compile matrix; `check` calls `cross` for it now
# (NOTES § D66), and `cross` reads CI's own target list rather than keeping a
# second copy of it.
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

# fmt + clippy + tests + the guards + the cross-compile matrix. Every step CI
# runs is here, and nothing here is skipped by CI. The two drifted three times
# already (the self-test below and cargo-deny were CI-only, so `cargo deny`
# first failed on a push nobody could have caught locally; then the cross matrix
# was CI-only for a phase and a half, NOTES § D66; then `todo-guard.py` was
# local-only, because CI kept a second hand-written copy of the guard list).
# Each was one list written twice, and each was closed by deleting the copy
# rather than by re-syncing it: the guards live in `scripts/guards.sh` and both
# this file and CI call it, the release targets live in CI's matrix and `cross`
# reads that.
# The last two run last because they are the two that need something `cargo`
# alone does not give you — `cargo install cargo-deny`, and a cross std — and
# when either is missing you still want everything above them to have
# reported. `cross` is after `deny` because on a green run its report is the
# last thing on screen, which is the entire reason a skipped target stays
# visible: see the recipe.

# Everything CI runs: fmt, clippy, tests, the guards, cargo-deny, cross-compile
check:
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets --all-features -- -D warnings
    cargo test --locked --all-targets
    {{just_executable()}} guards
    cargo deny check advisories licenses sources bans
    {{just_executable()}} cross

# The guard list exists exactly once, in `scripts/guards.sh`, and every caller
# names the file rather than its contents: this recipe, and CI's single
# `bash scripts/guards.sh` step. `todo-guard.py` was in `check` and not in CI for
# exactly as long as CI kept a second hand-written copy of that list, so the
# guard that had just been proved red-then-green did not run on a push
# (NOTES § D26, D111). Adding a guard is one edit: one line in the script.
#
# The list is a script and not the body of this recipe so that CI needs no
# `just` on the runner — installing it would have been a fourth third-party
# action bought purely to reach an entry point (NOTES § D111). The script also
# carries the assertion that `check` still calls this recipe and names no
# `scripts/` guard of its own; it lives there and not here because CI runs the
# script, and a local-only assertion about CI's coverage is the same hole again.
#
# Every scripts/ guard: its --self-test where it has one, then its real run
guards:
    bash scripts/guards.sh

# The row where `just check` was not CI (NOTES § D66). CI runs
# `cargo check --locked --target <t> --all-targets` over a four-way matrix and
# nothing local did, so a break that only appears for musl or darwin was
# discoverable only after a push — cross-compilation breaks at link time and it
# breaks late, which is exactly the failure "`just check` is the whole of CI, or
# it is a lie" exists to prevent.
#
# THE COST DECISION, and it went to the skip. Requiring the four targets makes
# the gate red on every machine that has not run `rustup target add` — including
# this one, where /usr/bin/cargo is a distro rust with no rustup at all — and a
# gate that is red by default is one everybody learns to wave through, which
# costs more than the gap does. So a target whose std is not installed is
# skipped — and on a machine that has all four, `just check` grows four full
# dependency builds, which is the other half of the price and the reason this
# recipe is at the end rather than in the middle. A skip is only worth having
# if it survives a *green* run, so it is
# paid for three ways: `cross` is the last thing `check` runs, so on a green run
# the banner is the last thing on screen; the banner names every target that did
# not run, not just a count; and it prints on stderr, so it is still there when
# stdout went to a log file.
#
# Two things are deliberately NOT skips, because either would delete a target
# from the gate in silence — the same invisible gap wearing a different coat:
#   · a triple rustc has never heard of is a typo in CI's matrix, and fails.
#   · a matrix this recipe cannot read out of ci.yml fails, rather than
#     cheerfully checking the empty list. The list is read from the workflow
#     instead of copied here for the same reason: two copies of it is how this
#     row would reopen.
#
# Cross-compile check for every release target CI builds
cross:
    #!/usr/bin/env bash
    set -euo pipefail

    targets=$(sed -n 's/^[[:space:]]*- target:[[:space:]]*//p' .github/workflows/ci.yml)
    # "extracted nothing" and "nothing to extract" print the same line, so the
    # derived list is asserted and not trusted (CLAUDE.md § A derived list
    # asserts it found something). Rename the matrix key upstream and this
    # recipe would otherwise pass by checking nothing at all. The canary is the
    # one target REQUIREMENTS names as the primary release artifact, so it is
    # also the one whose removal should stop and be looked at rather than
    # silently shrink the gate — if it went on purpose, move this line to
    # whatever replaced it.
    echo "$targets" | grep -qx x86_64-unknown-linux-musl \
      || { echo "cross: x86_64-unknown-linux-musl is not in .github/workflows/ci.yml's matrix — either the matrix moved and this recipe was about to check nothing, or the target was dropped on purpose and this line has to move with it" >&2; exit 1; }

    skipped=
    for t in $targets; do
      # Two different failures that look alike from here: a non-zero rustc means
      # the triple does not exist, a zero rustc with a directory that is not
      # there means the triple is real and its std was never installed. Only the
      # second one is a skip.
      libdir=$(rustc --print target-libdir --target "$t") \
        || { echo "cross: rustc does not know the target '$t' — that is a typo in CI's matrix, not a missing toolchain" >&2; exit 1; }
      if [ -d "$libdir" ]; then
        cargo check --locked --target "$t" --all-targets
      else
        skipped="$skipped $t"
      fi
    done

    if [ -z "$skipped" ]; then
      echo "cross: every release target in CI's matrix was checked"
      exit 0
    fi
    {
      echo
      echo "###############################################################################"
      echo "#  GREEN WITHOUT THE CROSS-COMPILE MATRIX — these targets were NOT checked:"
      for t in $skipped; do echo "#      $t"; done
      echo "#"
      echo "#  Their std is not installed on this machine, so the step was skipped, not"
      echo "#  passed. CI runs all of them: a break that only shows up for musl or darwin"
      echo "#  is still ahead of you, and it will land on the push instead of here."
      echo "#"
      echo "#  To close it locally:  rustup target add$skipped"
      echo "###############################################################################"
    } >&2

# Run the binary with the given arguments
run *ARGS:
    cargo run -- {{ARGS}}

# Phase 3 turns this on for rules.rs, Phase 4 for analysis.rs (NOTES § D26).
#
# **Both mutation recipes go through `scripts/mutants.sh`**, which names the
# scratch volume and reads the run's logs afterwards. cargo-mutants builds a whole
# copy of the tree per mutant and files *any* failed build as `unviable`, so a run
# on a full disk reports untested mutants in a word that reads like a pass — that
# is what happened on 2026-08-21 with `/tmp` a 12 GiB tmpfs at 94% and `/home` 916
# GB free (NOTES § D133). The script is the one place that knows it; a flag typed
# at the gate reaches cargo-mutants unchanged.
#
# `*ARGS` is what makes the phase-close sweep's `--shard k/4` (D118) go through
# the guard instead of round it — sharding stays a flag typed at the gate and does
# not become a recipe, which is what D118 ruled.
#
# Mutation testing over the two pure files: a surviving mutant is a diagnosis change no test objected to
mutants *ARGS:
    bash scripts/mutants.sh --timeout 90 --file src/rules.rs --file src/analysis.rs {{ARGS}}

# The per-turn gate CLAUDE.md § step 4 names — the author's own diff, not the
# whole file. It is a recipe rather than a command in a doc for one reason: typed
# by hand it inherits whatever `TMPDIR` the box hands it, which is the tmpfs, and
# it is the invocation that runs most often. `--file` is deliberately absent —
# the diff decides the scope, and a filter beside it would silently drop a mutant
# in a file the turn actually changed.
#
# Mutation testing over the author's own diff — the per-turn gate, not the sweep
mutants-diff:
    #!/usr/bin/env bash
    set -euo pipefail
    diff=$(mktemp); trap 'rm -f "$diff"' EXIT
    git diff HEAD > "$diff"
    # An empty diff yields `0 mutants tested` and exit 0, which is this file's own
    # subject wearing a different hat: nothing to test reads exactly like nothing
    # got past. Refuse instead.
    [ -s "$diff" ] || { echo "mutants-diff: 'git diff HEAD' is empty, so there are no mutants to run and a green run would prove nothing (NOTES § D26, § D133)" >&2; exit 1; }
    bash scripts/mutants.sh --timeout 90 --in-diff "$diff"

# --- the test cluster (scripts/cluster.sh does the work) ---

# One worker per node state `break-nodes` produces, so no fixture has two
# causes.

# Bring up the four-node kind test cluster (1 control-plane + 3 workers)
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

    # Every capture below is followed by an assertion about the bytes that
    # landed, and this is the one place to write one. It runs *after*
    # sanitize.jq, so it covers both halves of the same failure: a cluster that
    # never produced the shape, and a filter that learned to destroy it. Naming
    # the field rather than the file is the whole point — a capture of the wrong
    # object writes perfectly valid JSON, and "found none" reads exactly like
    # "there were none" (CLAUDE.md § A derived list asserts it found something).
    guard() { # $1 file  $2 what has to be in it  $3 the jq that finds it
      jq -e "$3" "tests/fixtures/$1" >/dev/null \
        || { echo "fixtures: $1 carries no $2 — that is what this capture is for" >&2; exit 1; }
    }

    # Rule 12 needs a pod that is Terminating and stays that way: the delete is
    # part of the capture, not of `cluster.sh break`.
    "${kc[@]}" delete pod broken-stuck --wait=false --ignore-not-found
    # `--wait=false` returns once the API server accepts the DELETE, not once the
    # object shows it. Rule 12 reads deletionTimestamp, so a capture taken before
    # it appears is a fixture the rule cannot fire on — assert it, do not hope.
    "${kc[@]}" get pod broken-stuck -o json \
      | jq -e '.metadata.deletionTimestamp != null and ((.metadata.finalizers // []) | length > 0)' >/dev/null \
      || { echo "fixtures: broken-stuck is not Terminating behind a finalizer — rule 12 has no fixture" >&2; exit 1; }

    # `crashloop`, `exit0`, `sigterm`, `probe0`, `restarts` and `init` are not in
    # this list: they are the six fixtures whose container is still cycling *and*
    # whose tests name the face it has to be caught in, and they are fetched under
    # their own retry below. One writer per file — a bulk fetch here and a re-fetch
    # there means the bytes that land depend on which one ran last.
    for p in oom image config pending hostpath readiness nolimits stuck \
             resize podlimit \
             socket succeeded failed restarts10 restarts10serving startup \
             notfound wedged unjudged oomserving neverback neverrules gang \
             overhead; do
      "${kc[@]}" get pod "broken-$p" -o json | "${jqs[@]}" > "tests/fixtures/$p.json"
    done

    # `verify` proved the *live* object reached its state, minutes ago. A crash
    # loop has two faces — `waiting: CrashLoopBackOff` and the `terminated` run it
    # just left — plus a ~2s window where the container is up, and a capture taken
    # in that window is a Running pod carrying a crash history.
    #
    # **The live predicate and the byte guard ask different questions, and asking
    # them with one jq is what shipped the 2026-08-20 defect.** `cluster.sh`
    # § `[crashloop]` asks whether a *live* pod cycles, and it is right to accept
    # either face: sampled 70 times on the owned crashlooping pod the container was
    # `terminated` 39 times and `waiting` 29, so demanding the waiting reason there
    # fails a pod that is crashlooping correctly. This asks whether the *committed
    # bytes* can carry the tests written against them, and there the faces are not
    # interchangeable: `crashloop.json` in the terminated face takes
    # `crashloop_pod_decodes_what_rules_1_5_and_6_read` red with "a crashlooping
    # container must decode as waiting", and `exit0.json` in that face draws rule
    # 5's card where its test reads rule 1's. Both landed wrong on 2026-08-20,
    # under the disjunction this replaces, and were found hours later by the suite.
    #
    # So the face is named per fixture — the answer is `git show
    # HEAD:tests/fixtures/<name>.json`, never a guess — and the fetch retries until
    # the bytes carry it. Bounded and loud: a silent give-up commits the fixture
    # the tests cannot read, which is the bug. **Do not carry the tightening back
    # into `cluster.sh`** — there it is the too-tight half `verify-test.sh` exists
    # to catch.
    #
    # **The 300s is one full backoff cap plus a sample, not a measurement.** Nobody
    # has timed how long a single face persists on `broken-crashloop` itself — the
    # 39/29/2 split above is the owned pod, sampled, and says nothing about run
    # lengths. So if this ever fails on the bound, suspect the bound before the
    # cluster: watch `kubectl get pod broken-<stem> -o json` for a cycle and raise
    # `FETCH_BUDGET`, rather than loosening the face back into a disjunction.
    #
    # **One budget for all six calls, not six of them** (the 2026-08-21 operator
    # review). Six independent 300s timers is thirty minutes nobody added up, inside
    # a trip that has to finish within an hour of `break` and that carries
    # `broken-restarts`, whose `sleep 3600` fuse makes `restartCount == 3` expire on
    # a wall clock — the retry that saved one fixture would have quietly spent the
    # budget of another. A shared deadline is also the honest statement of what is
    # being bought: five minutes of this trip, total, spent waiting for faces. It is
    # still one full backoff cap, so every fixture's face recurs at least once
    # inside it.
    FETCH_BUDGET=300
    fetch_deadline=$((SECONDS + FETCH_BUDGET))
    # **The capture that fails never reaches the tree.** The old shape wrote
    # `tests/fixtures/$1.json` on every attempt and then exited, leaving on disk
    # exactly the file whose own error message says committing it is the defect —
    # one `git add -A` away from being the bug it refuses. The fetch lands in a temp
    # and is copied over the fixture only once the bytes pass, so a failed trip
    # leaves the previous good capture in place and `git status` clean for that file.
    #
    # **Copied through a redirect and not `mv`d**, which is not a style choice:
    # `mktemp` creates at 0600 and `mv` carries that mode across, so the six loop
    # fixtures would sit at 0600 beside every other capture at 0644 — and git tracks
    # only the executable bit, so that drift is invisible forever. The redirect is
    # what every other capture in this recipe uses, so the mode matches by
    # construction and there is no second rule to remember. The cost is a truncate
    # window of one `cat` on local disk, entered only after the bytes have already
    # passed the guard, against a permanent silent difference: worth it.
    fetch_until() { # $1 fixture stem  $2 the face the bytes must carry  $3 the jq that finds it
      local tmp; tmp=$(mktemp) || return 1
      while :; do
        "${kc[@]}" get pod "broken-$1" -o json | "${jqs[@]}" > "$tmp"
        if jq -e "$3" "$tmp" >/dev/null; then
          cat "$tmp" > "tests/fixtures/$1.json"
          rm -f "$tmp"
          return 0
        fi
        [ "$SECONDS" -lt "$fetch_deadline" ] || {
          echo "fixtures: $1.json never landed on $2 inside the ${FETCH_BUDGET}s this trip budgets for all six loop fixtures together — the capture is on a face its own tests cannot read, and committing it is the 2026-08-20 defect again. tests/fixtures/$1.json is untouched; the bytes that failed are in $tmp" >&2
          exit 1
        }
        sleep 3
      done
    }

    # Rule 1's card is drawn from `state.waiting.reason` — the word the reader saw in
    # `kubectl get pods`. The exit 1 is the run behind it, which is what separates a
    # loop from a container that has never started.
    fetch_until crashloop "rule 1's card — the CrashLoopBackOff reason, over an exit 1 on record" \
      '[.status.containerStatuses[]? | select(.lastState.terminated.exitCode == 1 and .state.waiting.reason == "CrashLoopBackOff")] | length > 0'
    # The same card over the opposite ending, and that pairing is the whole fixture
    # (D71/D88): a program that finished and is being restarted forever, not one
    # that crashed. `ready == false` is rule 6's exemption clause — the guard that
    # used to sit further down this file, folded in here because a second guard
    # weaker than this one is a guard that cannot fail.
    fetch_until exit0 "rule 1's card over a previous run that ended exit 0, on a container that is not serving (D71)" \
      '[.status.containerStatuses[]? | select(.ready == false and .lastState.terminated.exitCode == 0 and .state.waiting.reason == "CrashLoopBackOff")] | length > 0'
    # The third ending, same card. `sigterm.json` was not part of the 2026-08-20
    # defect — it landed on the waiting face, as it has every trip — but its guard
    # named no face at all, and three tests demand one:
    # `the_three_ways_into_a_restart_loop_do_not_get_the_same_card` asserts
    # `matches!(waiting(c), Some(("CrashLoopBackOff", _)))` for `crashloop`, `exit0`
    # and `sigterm` alike, and `broken-sigterm`'s own test reads rule 1's card off it. A
    # 13-restart container in a 5-minute backoff is one sample away from
    # `crashloop.json`'s Tuesday, so it is fetched the same way rather than left to
    # luck. Its old guard — `ready == false` with exit 143 — is folded in here for
    # the reason `exit0.json`'s was: strictly weaker than this, it could not fail.
    fetch_until sigterm "rule 1's card over a run that ended on exit 143 — 128 + SIGTERM, the container asked to stop and did, and not the 137 a PID 1 with no handler produces after the grace period" \
      '[.status.containerStatuses[]? | select(.ready == false and .lastState.terminated.exitCode == 143 and .state.waiting.reason == "CrashLoopBackOff")] | length > 0'
    # **`init.json` is the one that must not be tightened**, and the reason is in
    # git rather than in an opinion. Read `migrate`'s state across every committed
    # revision of this file: `waiting` on 2026-08-12, -08-12, -08-13 and -08-14,
    # `terminated` on -08-16 and again on -08-20. `crashloop.json`'s `quitter` is
    # `waiting` in all five and the -08-20 capture is the first terminated one; that
    # is the difference. So this fixture genuinely arrives on both faces, its decode
    # test reads whichever one it holds by design (NOTES § D114), and demanding
    # `waiting` here would be a new bug wearing this one's clothes. What the retry
    # still buys is the ~2s window: an init container that is up right now is
    # history, not a loop.
    fetch_until init "an init container in a crash loop — an exit 1 on record and the container not up right now, either face (D27, D114)" \
      '[.status.initContainerStatuses[]? | select(.lastState.terminated.exitCode == 1 and (.state.waiting.reason == "CrashLoopBackOff" or .state.terminated.exitCode == 1))] | length > 0'
    # **`probe0.json` is the mirror: the one cycling fixture whose face must not be
    # `CrashLoopBackOff`.** 13 restarts, so it cycles like the three above it and can
    # land in backoff on any trip — and if it does, rule 1 fires and
    # `each_ending_sends_the_reader_somewhere_the_answer_can_be` loses the rule 5
    # card it reads. `src/rules_tests/pod.rs` § `restarts10_ending` names the shape
    # positively — "13 restarts, `lastState.terminated` `0` / `Completed`,
    # `state.running`, `ready: false`", the container that reached RESTARTS_WARN by
    # *finishing* and is out of `CrashLoopBackOff` — so `state.running` is what the
    # tests read and what this names. Only the guard ever spelled it as an absence.
    #
    # The duration clause is the old guard's, unchanged (D90/D113): the exit code
    # alone is `exit0.json`'s 2s run, the arm `finished_action` demotes, so the long
    # arm is the run length and not the code. Same epoch defaults the cluster.sh
    # predicate uses — `fromdateiso8601` is a hard jq error on a missing stamp, and
    # under a retry an error would spin out the whole bound instead of printing.
    fetch_until probe0 "a container running and not serving, over a previous run that ended exit 0 and lasted longer than the 20s probe floor — the long arm of finished_action, out of CrashLoopBackOff (D90/D113)" \
      '[.status.containerStatuses[]? | select(.state.running != null and .ready == false and .lastState.terminated.exitCode == 0 and ((((.lastState.terminated.finishedAt // "1970-01-01T00:00:00Z") | fromdateiso8601) - ((.lastState.terminated.startedAt // "1970-01-01T00:00:00Z") | fromdateiso8601)) > 25))] | length > 0'
    # The sweep's own find, and the same shape as `probe0`: `flaky` has restarted
    # three times and is up, so its face can move, and two tests read it. The decode
    # asserts `ContainerState::Running { started_at }` outright, and rule 5's card is
    # *dated* from `state.running.startedAt` (D100) — a capture caught between runs
    # has no such stamp and no such card, and `all.len() == 1` breaks besides, rule 1
    # having joined in.
    #
    # **The restart count is deliberately not in here.** It only ever goes up, so
    # re-fetching cannot mend it: `guard restarts.json` keeps asking that question
    # further down, immediately and in its own words, because the answer to a four
    # is unbreak-and-recapture and not another five minutes of waiting.
    fetch_until restarts "the container up and serving — rule 5's 'looks healthy now, but something is wrong', which is read off state.running and dated from its startedAt (D100)" \
      '[.status.containerStatuses[]? | select(.state.running != null and .ready == true and (.state.running.startedAt // "") != "")] | length > 0'

    # The fields the first capture could not produce. Each one is a decode that
    # today reads correctly whatever it does, because every committed object
    # leaves the field absent — so each is asserted here by name, and a capture
    # taken from the manifests as they were before this box fails loudly instead
    # of quietly retiring a synthesis with an object that does not carry it.
    # The noun phrase is what gets printed after "carries no", so it may not
    # contain a negation of its own: this one read "carries no nodeSelector, and
    # no toleration written beside it" and told the reader the opposite of what
    # the failure was. The toleration clause names the key and value on purpose —
    # every pod carries the two tolerations the DefaultTolerationSeconds
    # admission plugin adds, so "has tolerations" is true of all of them.
    guard pending.json  "nodeSelector with an operator's own toleration beside it (N6's pod side)" \
      '((.spec.nodeSelector // {}) | length) > 0 and ([.spec.tolerations[]? | select(.key == "dedicated" and .value == "gpu")] | length) > 0'
    # D46's second Phase 4 field, and the only one in this file the *apiserver*
    # writes rather than the manifest: `spec.overhead` is autopopulated by the
    # RuntimeClass admission controller, which rejects a create request that
    # already carries it. So a capture holding it is proof the plugin ran — and
    # `runtimeClassName` beside it names the class it was read from. The request
    # clause is not padding: the pod's own 100m is deliberately smaller than the
    # 250m overhead, which is what makes a sum that ignores the field and one
    # that counts it give visibly different answers on that node.
    #
    # **And the node is asserted by name**, because the whole point of the
    # `nodeName` the manifest gained on 2026-08-21 is that this placement stops
    # being a lottery (the operator review; `scripts/broken.yaml` § broken-overhead
    # carries the reasoning). The 2026-08-20 trip landed it on `k8rs-worker2`,
    # which `nodes.json` — captured after `break-nodes`, correctly — shows carrying
    # `dedicated=gpu:NoExecute`: the object built to make a per-node sum provably
    # wrong, on the one node the same snapshot says evicts everything on it. A
    # `nodeName` nothing checks is a `nodeName` a future edit drops in silence.
    guard overhead.json "pod carrying the RuntimeClass charge the scheduler counts and a spec-only sum does not, on the cordoned worker its manifest pins it to and not on the node break-nodes evicts (D46)" \
      '.spec.runtimeClassName == "broken-overhead" and .spec.overhead.cpu == "250m" and .spec.overhead.memory == "120Mi" and .spec.containers[0].resources.requests.cpu == "100m" and .spec.nodeName == "k8rs-worker"'
    guard hostpath.json "pair of mounts of one hostPath volume, one narrowed by a subPath and one read-only (D46)" \
      '[.spec.volumes[]? | select(.hostPath) | .name] as $hp | [.spec.containers[].volumeMounts[]? | select(.name as $n | $hp | index($n))] | length == 2 and any(.subPath != null) and any(.readOnly == true) and any(.readOnly != true)'
    # The string, not its existence: `"REDACTED-IP"` is also non-null, and it is
    # exactly what a sanitizer that treated this line as an address would leave
    # behind. The manifest prints a hostname for that reason — so the assertion
    # has to be the one thing a filter could destroy without emptying the field.
    # **The waiting `message` is checked here and not in the retry above, and the split is the
    # same one `restarts.json`'s count gets.** The 2026-08-21 review was right that
    # rule 1 never reads it — `crash_looping` does `let (reason, _) = waiting(c)?`
    # and drops the sentence — but the rule is not the only reader: the very test
    # this whole hunk exists for asserts it, at `src/rules_tests/snapshot.rs:431`,
    # `message…contains("back-off")` inside
    # `crashloop_pod_decodes_what_rules_1_5_and_6_read`. Drop the check and that
    # test goes red on a capture the byte guard just certified, which is the defect
    # this file is fixing.
    #
    # It is a plain `guard` because the string does not alternate. The face does —
    # retrying is the right answer to a face. `back-off` is the kubelet's own
    # `fmt.Sprintf` and not an API contract, so if it is ever reworded upstream no
    # amount of re-fetching mends it, and retrying would spend the shared budget and
    # then fail with a message about a face. Here it fails at once, naming the
    # string, which is the sentence whoever has to change `snapshot.rs:431` needs.
    guard crashloop.json "kubelet back-off sentence beside the reason — the string src/rules_tests/snapshot.rs:431 asserts, which no rule reads and one test does" \
      '[.status.containerStatuses[]? | select((.state.waiting.message // "") | contains("back-off"))] | length > 0'
    guard crashloop.json "log tail in a termination, which is what terminationMessagePolicy FallbackToLogsOnError makes the kubelet write (D51)" \
      '[.status.containerStatuses[]? | .lastState.terminated.message | select(type == "string" and contains("db.payments.svc:5432"))] | length > 0'
    guard resize.json   "limit the spec asks for and the kubelet did not enact (D51)" \
      '.status.containerStatuses[0].resources.limits.memory != .spec.containers[0].resources.limits.memory and (.status.containerStatuses[0].resources.limits.memory | . != null)'
    guard podlimit.json "memory limit in the container status that its spec never declared (D53)" \
      '((.spec.containers[0].resources.limits // {}) | has("memory") | not) and ((.status.containerStatuses[0].resources.limits // {}) | has("memory"))'

    # The fixture this trip can damage without touching it. `broken-restarts`
    # ends on `sleep 3600`: an hour after that sleep starts the shell exits 0,
    # the container restarts, and rule 5's WARN-boundary fixture quietly becomes
    # a four. The two restart-count pods added for this box make the trip a good
    # half-hour longer, which is what brings that hour within reach — so the
    # number is asserted rather than trusted. If this fires, the fixture is not
    # wrong, the clock is: unbreak, break, and capture without the long pause.
    guard restarts.json "container at exactly three restarts — rule 5's WARN boundary, and a trip that idled past broken-restarts' own hour would have moved it" \
      '.status.containerStatuses[0].restartCount == 3'

    # --- THE BRANCHES THAT SHIPPED WITH NO TEST THAT COULD FAIL ---
    # Every guard below names the field the *rule* reads, not the file, for the
    # reason the guards above it do: a capture of the wrong object writes
    # perfectly valid JSON, and "found none" reads exactly like "there were
    # none". Each also asserts the *state* the container is in, which is the
    # half that is easy to leave out — the rules these fixtures are for are
    # silenced by several clauses at once, so a capture taken while the
    # container happened to be up would satisfy a different clause and the
    # branch under test would still have nothing behind it (NOTES § D71).
    #
    # `exit0.json`'s clause — rule 6's exemption, which doing_its_job would
    # otherwise be the one silencing (D71) — moved into `fetch_until exit0` above,
    # which asserts it and the face together. Left here it was strictly weaker than
    # a check that had already run: a guard that cannot fail.
    # `sigterm.json`'s clause moved into `fetch_until sigterm` above, with exit0's
    # and for the same reason.
    guard socket.json "read-only mount of the runtime socket under its /var/run spelling — the fold and the exact match, neither of which hostpath.json reaches (D78)" \
      '[.spec.volumes[]? | select(.hostPath.path == "/var/run/docker.sock") | .name] as $s | [.spec.containers[].volumeMounts[]? | select(.name as $n | $s | index($n))] | length == 1 and all(.readOnly == true)'
    guard succeeded.json "Succeeded pod whose container still carries three restarts and a failed previous run — the two cards analyze() must skip" \
      '.status.phase == "Succeeded" and ([.status.containerStatuses[]? | select(.restartCount >= 3 and .lastState.terminated.exitCode == 1)] | length > 0)'
    guard failed.json "Failed pod carrying the same restarts and the same failed run — the half of that skip D71 records as missed, and the phase an Evicted pod arrives in" \
      '.status.phase == "Failed" and ([.status.containerStatuses[]? | select(.restartCount >= 3 and .lastState.terminated.exitCode == 1)] | length > 0)'
    guard restarts10.json "container past ten restarts, up and not serving — rule 5's CRITICAL band" \
      '[.status.containerStatuses[]? | select(.restartCount >= 10 and .ready == false and .state.running != null)] | length > 0'
    guard restarts10serving.json "container past ten restarts that is serving — the && !serving half of the same branch, which stays WARN" \
      '[.status.containerStatuses[]? | select(.restartCount >= 10 and .ready == true and .state.running != null)] | length > 0'
    guard startup.json "container reporting started: false while it runs — the only field that tells rule 7's suppressor from its state gate (D71)" \
      '[.status.containerStatuses[]? | select(.started == false and .ready == false and .state.running != null)] | length > 0'
    guard notfound.json "exit 127 with no termination message — rule 6's command-not-in-the-image action, which the log-line arm answers first whenever a message exists" \
      '[.status.containerStatuses[]? | select(.lastState.terminated.exitCode == 127 and ((.lastState.terminated.message // null) == null))] | length > 0'
    guard wedged.json "scheduled pod stuck at ContainerCreating with PodReadyToStartContainers False — rule 13's positive, and the storage branch of its evidence line (D72/D76)" \
      '([.status.conditions[]? | select(.type == "PodScheduled" and .status == "True")] | length) > 0 and ([.status.conditions[]? | select(.type == "PodReadyToStartContainers" and .status == "False")] | length) > 0 and ([.status.containerStatuses[]? | select(.state.waiting.reason == "ContainerCreating")] | length) > 0'
    guard unjudged.json "Pending pod with no PodScheduled condition at all, and the creationTimestamp rule 14 measures its grace from" \
      '.status.phase == "Pending" and (([.status.conditions[]? | select(.type == "PodScheduled")] | length) == 0) and .metadata.creationTimestamp != null'
    guard oomserving.json "OOM kill in lastState on a container that is serving now — rule 2's recency clause, whose two directions are both read off this one object" \
      '[.status.containerStatuses[]? | select(.ready == true and .state.running != null and .lastState.terminated.reason == "OOMKilled")] | length > 0'
    # D96's shape, and the three fields are asserted separately because a capture
    # taken a second early, or one taken after `keeper` was gone, writes
    # perfectly valid JSON that proves none of it: a terminated container without
    # `Never` beside it is one the kubelet is about to restart, and the same
    # object at `phase: Failed` is a pod that is over, which every rule already
    # skips. The clean exit is named too — `done` is this fixture's own negative,
    # and a two-container capture would silently retire it.
    guard neverback.json "container stopped for good — a terminated run at a non-zero exit, restartCount still 0, under spec.restartPolicy Never, in a pod that is still Running, beside a container that exited 0 and one that is still up (D96)" \
      '.spec.restartPolicy == "Never" and .status.phase == "Running" and ([.status.containerStatuses[]? | select(.restartCount == 0 and (.state.terminated.exitCode // 0) != 0)] | length) > 0 and ([.status.containerStatuses[]? | select(.state.terminated.exitCode == 0)] | length) > 0 and ([.status.containerStatuses[]? | select(.state.running != null)] | length) > 0'

    # `probe0.json`'s clause — NOTES § D90's first door, exit 0 with somebody else's
    # hand on it on a run past PROBE_FLOOR — moved into `fetch_until probe0` above,
    # which asserts it and the `state.running` face together.
    # D97's named false positive for rule 15, and both halves are named because
    # either alone is another fixture: the field in the spec is a pod nobody
    # restarted, and the count without it is `broken-restarts` under a different
    # policy.
    guard neverrules.json "container restarted under spec.restartPolicy Never by a rule on its own exit code — the restartPolicyRules entry in the spec and the restartCount it bought, on a container now terminated at an exit no rule matches (D97)" \
      '.spec.restartPolicy == "Never" and ([.spec.containers[]? | select([.restartPolicyRules[]? | select(.action == "Restart" and .exitCodes.operator == "In" and (.exitCodes.values | index(3) != null))] | length > 0)] | length) == 1 and ([.status.containerStatuses[]? | select(.restartCount == 1 and .state.terminated.exitCode == 1)] | length) == 1 and ([.status.containerStatuses[]? | select(.state.running != null)] | length) == 1'
    # D100's settled gang restart. The two null stamps are asserted as hard as
    # the reason: they are why rule 5 reads its age off `state.running.startedAt`
    # instead, and a record that arrived with stamps would retire nothing.
    guard gang.json "137/RestartingAllContainers carrying neither stamp, beside a live state.running.startedAt on a container that is serving — the synthesized record rule 5 can read no clock off, and the reason nothing else in the corpus holds (D100)" \
      '([.status.containerStatuses[]? | select(.lastState.terminated.exitCode == 137 and .lastState.terminated.reason == "RestartingAllContainers" and .lastState.terminated.startedAt == null and .lastState.terminated.finishedAt == null and .ready == true and .state.running.startedAt != null and .restartCount >= 3)] | length) > 0 and ([.status.containerStatuses[].ready] | all)'

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
      guard "$f.json" "controlling ownerReference" \
        '[.items[]? | select(([.metadata.ownerReferences[]? | select(.controller==true)] | length) > 0)] | length > 0'
    done

    # D40: the Deployment whose second revision cannot start. One object gives
    # both halves — a workload that is *partially* ready, which is the only
    # state that separates `desired` from `ready` from the three counters beside
    # them, and two ReplicaSets under one Deployment, which is the shape a
    # rollout actually has. Every workload captured before this was either
    # entirely ready or entirely absent.
    "${kc[@]}" get replicasets -l app=broken-rollout -o json | "${jqs[@]}" > tests/fixtures/rollout-replicasets.json
    guard rollout-replicasets.json "pair of revisions, one serving and one that never started (D40)" \
      '[.items[]? | select([.metadata.ownerReferences[]? | select(.controller == true and .kind == "Deployment")] | length > 0)] | length == 2 and any((.status.readyReplicas // 0) > 0) and any((.status.readyReplicas // 0) == 0)'

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
    guard kube-system-pods.json "pod owned by a controlling Node (the static-pod shape D39 rules on, and N2 reads as mirror)" \
      '[.items[] | select(any(.metadata.ownerReferences[]?; .kind == "Node" and .controller == true))] | length > 0'
    guard kube-system-pods.json "pod owned by a controlling DaemonSet with a writable hostPath (rule 8 false-positive class)" \
      '[.items[] | select(any(.metadata.ownerReferences[]?; .kind == "DaemonSet" and .controller == true)) | [.spec.volumes[]? | select(.hostPath) | .name] as $hp | .spec.containers[].volumeMounts[]? | select(.readOnly != true) | select(.name as $n | $hp | index($n))] | length > 0'
    guard kube-system-pods.json "read-only hostPath mount (the half of rule 8 that stays out of Alerts)" \
      '[.items[] | [.spec.volumes[]? | select(.hostPath) | .name] as $hp | .spec.containers[].volumeMounts[]? | select(.readOnly == true) | select(.name as $n | $hp | index($n))] | length > 0'

    # The negative side. Every rule needs a healthy counterpart or its
    # false-positive test is fiction — and three of these four exist because the
    # shape they carry is one no *broken* pod can carry: rule 8 fires on a
    # writable host mount, so the read-only one is a healthy object; a sidecar
    # that keeps running is not a failure; and a pod-level request is ordinary
    # capacity accounting. They are separate pods rather than fields on
    # `healthy`, which is the negative fixture for every rule at once: a host
    # mount on it would leave rule 8 with two positives and no negative, and an
    # init container that restarts forever would end its "never restarted".
    for h in healthy healthy-hostpath healthy-sidecar healthy-podlevel \
             healthy-retry healthy-unreadysidecar healthy-disk; do
      "${kc[@]}" get pod "$h" -o json | "${jqs[@]}" > "tests/fixtures/$h.json"
    done
    guard healthy.json "resources on its init container — the list the spec lookup would otherwise never read (D40)" \
      '.spec.initContainers[0].resources.limits.memory != null'
    guard healthy-hostpath.json "read-only hostPath mount, which is the half of rule 8 that must not fire (D46)" \
      '[.spec.volumes[]? | select(.hostPath) | .name] as $hp | [.spec.containers[].volumeMounts[]? | select(.name as $n | $hp | index($n))] | length > 0 and all(.readOnly == true)'
    guard healthy-sidecar.json "init container with restartPolicy Always — the native sidecar (D46)" \
      '[.spec.initContainers[]? | select(.restartPolicy == "Always")] | length > 0'
    guard healthy-podlevel.json "pod-level request beside the container's own (KEP-2837, D51)" \
      '.spec.resources.requests.cpu != null and .spec.containers[0].resources.requests.cpu != null'
    # The two silences D75 asks for, and both are on the healthy side because the
    # assertion is that **nothing** fires: a wait-for-dependency loop that
    # finished keeps its restart count and its failed lastState for the life of
    # the pod, and a sidecar that is up and not ready is not rule 7's — that
    # rule reads regular containers only.
    guard healthy-retry.json "init container that failed three times and then succeeded — the count is what makes rule 5's silence mean something (RESTARTS_WARN is 3)" \
      '[.status.initContainerStatuses[]? | select(.restartCount >= 3 and .state.terminated.exitCode == 0 and .lastState.terminated.exitCode == 1)] | length > 0'
    guard healthy-unreadysidecar.json "sidecar that is running and not ready, beside a workload container that is serving — the third container role in the state no capture holds (D75)" \
      '([.spec.initContainers[]? | select(.restartPolicy == "Always")] | length) > 0 and ([.status.initContainerStatuses[]? | select(.ready == false and .started == true and .state.running != null)] | length > 0) and ([.status.containerStatuses[].ready] | all)'
    # Waste's orphan-PVC row is a **join**, and this is the half of it that lives
    # on a pod: in a corpus where no pod mounts any claim, a report that does the
    # join and one that names every Bound claim print the same row (NOTES § D129).
    # The pod is asserted ready as well as mounting, because it is a `healthy-*`
    # fixture and nothing may fire on it.
    guard healthy-disk.json "pod that mounts a PersistentVolumeClaim, ready — the half of Waste's orphan-disk row that lives on a pod" \
      '([.spec.volumes[]? | select(.persistentVolumeClaim.claimName == "healthy-disk")] | length) == 1 and ([.status.containerStatuses[].ready] | all)'
    # W1 and W2 read a ReplicaSet, so their negative has to be one too — the
    # healthy Deployment in deployments.json cannot show the absence of a
    # ReplicaFailure condition that only ever appears on the ReplicaSet.
    "${kc[@]}" get replicasets -l app=healthy-deploy -o json | "${jqs[@]}" > tests/fixtures/healthy-replicasets.json
    # **The other side of Drain safety's join** (NOTES § D130). `broken-pdb-floor`
    # selects `app=healthy-deploy`, and until this line the corpus held the budget
    # and never a pod it covers — the headline row is *a budget, the pods it covers,
    # the node they are on*, and a report that does that join and one that prints
    # the budget's own counters back were indistinguishable on disk. `healthy-deploy`
    # is the workload D130 picked for the budget precisely because all four of its
    # numbers agree and its zero comes from the floor and nothing else, so its pods
    # are the ones the row is about. **`broken-rollout`'s are deliberately not
    # captured**: D130 rules that workload blocked by being unhealthy rather than by
    # its budget, so it is a different row, and three broken pods would add cards to
    # the whole-capture run for a join nothing needs today.
    #
    # **The position is the assertion.** It is here, in the pod-capture phase, and
    # not down beside `nodes.json`, because `break-nodes`' NoExecute taint evicts
    # them: read live after that step they are `Pending` with no node at all, against
    # a committed `poddisruptionbudgets.json` that says `currentHealthy: 2`. A capture
    # taken there contradicts the very object it exists to be joined against.
    "${kc[@]}" get pods -l app=healthy-deploy -o json | "${jqs[@]}" > tests/fixtures/healthy-deploy-pods.json
    # Exactly two, for the reason the `statefulsets.json` guard gives: the partner
    # object fixes the number. `poddisruptionbudgets.json` is asserted at
    # `expectedPods: 2` / `currentHealthy: 2`, so a list of one or three is a fixture
    # that contradicts its own other half, and `all` over an empty list is `true` —
    # "extracted nothing" and "nothing to extract" print the same line, and the count
    # is what separates them. The label is asserted because it is the join key and a
    # capture of the wrong selector writes perfectly valid JSON; `nodeName` and Ready
    # because the row's whole output is *which node a drain would block on*, and a
    # Pending pod names no node and is counted by no budget.
    #
    # **No `cluster.sh verify` predicate lands with this, and that is a ruling with
    # evidence, not an omission.** `verify` proves the live object; `[pdb_floor]`
    # already does it for these exact pods, through the same selector: it demands
    # `expectedPods == 2` and `currentHealthy == 2` on the budget whose selector *is*
    # `app=healthy-deploy`, and `scripts/broken.yaml` § broken-pdb-floor states in
    # the repo's own words that those counters are over *ready* pods (it rules
    # `broken-owned` out for having `currentHealthy` below `desiredHealthy` while it
    # crashloops). A pod-list predicate beside it would restate the budget's own
    # arithmetic against the same objects and could not fail while `[pdb_floor]`
    # passes — a fourth of the kind this file shed three of today, added rather than
    # inherited. What
    # `verify` cannot cover is drift between it and this fetch minutes later, and
    # that is the byte guard's half of the split, below.
    guard healthy-deploy-pods.json "pair of ready pods on nodes carrying the label broken-pdb-floor selects — the covered side of Drain safety's join, which the budget alone cannot show (D130)" \
      '([.items[]?] | length) == 2 and all(.items[]?; .metadata.labels.app == "healthy-deploy" and .status.phase == "Running" and ((.spec.nodeName // "") != "") and ([.status.conditions[]? | select(.type == "Ready" and .status == "True")] | length) == 1)'

    # W1: no pod exists at all — the truth is on the ReplicaSet.
    "${kc[@]}" get deployment broken-quota -n k8rs-quota -o json | "${jqs[@]}"       > tests/fixtures/quota-deployment.json
    "${kc[@]}" get replicasets -n k8rs-quota -o json | "${jqs[@]}"       > tests/fixtures/quota-replicasets.json

    # The cluster-wide snapshot analysis.rs reports are computed from. `nodes` is
    # not in this loop: it is captured last, after the nodes have been broken.
    #
    # **`endpointslices` rides beside `services` and not on its own line**, because
    # Waste's headline row reads the pair: a Service says which pods it *wants*,
    # and only the slice says whether anything is behind it. Capturing one without
    # the other is what left `services.json` unable to prove the row at all
    # (NOTES § D129) — the three Services in it all matched pods, and nothing on
    # disk said so.
    for kind in deployments statefulsets daemonsets services endpointslices \
                persistentvolumeclaims poddisruptionbudgets; do
      "${kc[@]}" get "$kind" -A -o json | "${jqs[@]}" > "tests/fixtures/$kind.json"
    done
    # The three workload kinds, each in the state that separates `desired` from
    # `ready` from the counters next to them. `statefulsets.json` was an empty
    # list until this box, which is the one hole no synthesis could fill — an
    # empty list is also what a capture from the wrong context writes, so the
    # assertion names the object rather than counting items.
    guard statefulsets.json "partially ready StatefulSet — the kind From<StatefulSet> had no object for at all (D40)" \
      '[.items[]? | select(.metadata.name == "broken-sts" and .spec.replicas == 2 and .status.readyReplicas == 1)] | length == 1'
    guard deployments.json "Deployment mid-rollout, its five replica counters holding five different values (D40)" \
      '[.items[]? | select(.metadata.name == "broken-rollout" and .spec.replicas == 2 and .status.replicas == 3 and .status.readyReplicas == 2 and .status.updatedReplicas == 1 and .status.unavailableReplicas == 1)] | length == 1'
    guard daemonsets.json "DaemonSet whose pods cannot start — desired is per node, and nothing captured had it disagree with ready (D40)" \
      '[.items[]? | select(.metadata.name == "broken-ds" and .status.desiredNumberScheduled > 0 and .status.numberReady == 0)] | length == 1'

    # --- THE TWO TABLES, AND THE PROXY THEY NEED (Phase 5, src/k8s.rs § THE BROWSER'S ROWS) ---
    # A `Table` is not an object, it is what the API server *prints* for
    # `kubectl get` — and `kubectl` has no flag that hands one back, because it
    # consumes the Table itself and writes columns. So this is the one capture in
    # the file that does not go through `kubectl get`: `kubectl proxy` puts an
    # authenticated local endpoint in front of the same API server, and `curl`
    # asks it with the Accept header `src/k8s.rs`'s `TABLE_ACCEPT` sends, byte for
    # byte. The two committed Tables were taken by hand this way on 2026-08-22;
    # this is that trip written down so the next one reproduces both.
    #
    # **Here, and not later in the file**, for the reason every other placement in
    # this recipe has one: `table-pods.json` is the same fourteen kube-system pods
    # `kube-system-pods.json` holds — same uids, checked — and both are on disk
    # before `break-runtime` reboots a node (which raises RESTARTS in the Table's
    # own column) and before `break-nodes` stops a kubelet (which turns the STATUS
    # column of everything on that node into `Unknown`).
    #
    # **The port is asked for rather than chosen.** `--port=0` makes the kernel
    # pick one and the proxy print it, which removes the failure a fixed port
    # has: a proxy leaked by an earlier failed trip is still listening on it,
    # still pointed at whatever context *it* was started with, and a Deployments
    # Table carries no node name for `sanitize.jq` to refuse — so a foreign
    # capture would land silently. The trap is what stops this trip leaking one
    # in turn, on the failure path as much as on the way out.
    table_log=$(mktemp)
    kubectl --context "$ctx" proxy --port=0 >"$table_log" 2>&1 &
    table_proxy=$!
    trap 'kill "$table_proxy" 2>/dev/null || true; rm -f "$table_log"' EXIT
    # **Anchored to the sentence kubectl prints on success, and that is not
    # tidiness.** `kubectl proxy` writes `Starting to serve on 127.0.0.1:<port>`
    # when it is up (`kubectl/pkg/cmd/proxy`) — and on a *bind failure* it writes
    # `error: listen tcp 127.0.0.1:8001: bind: address already in use`, which
    # carries an address in the same framing. An unanchored match reads the port
    # out of the error and this recipe then curls a port nothing is serving, or
    # somebody else's proxy. Measured on both lines before this was written.
    table_port=""
    for _ in $(seq 100); do
      table_port=$(sed -n 's/^Starting to serve on 127\.0\.0\.1:\([0-9][0-9]*\)[[:space:]]*$/\1/p' "$table_log" | head -1)
      [ -n "$table_port" ] && break
      kill -0 "$table_proxy" 2>/dev/null || break
      sleep 0.1
    done
    [ -n "$table_port" ] || { echo "fixtures: kubectl proxy never said it was serving — $(cat "$table_log")" >&2; exit 1; }
    # The exact string `TABLE_ACCEPT` is. The `,application/json` half is not
    # decoration: without it a server with no Table support answers 406 instead
    # of the list, and `the_table_fetch_is_one_path_one_header...` asserts this
    # header character for character.
    table_accept='Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json'
    # **Through a temp, for `fetch_until`'s reason and one that is sharper here.**
    # A redirect straight onto the fixture truncates it before the pipeline runs,
    # so the file is already empty when `sanitize.jq` refuses — and the refusal
    # this capture is most likely to meet is a Table from the *wrong cluster*,
    # reached through a proxy somebody else left running. That is the one case
    # where the previous good capture must survive on disk. Copied through a
    # redirect rather than `mv`d so the mode stays 0644 like every other capture.
    table() { # $1 fixture stem  $2 path with its query, if any
      local tmp; tmp=$(mktemp)
      curl -sf --retry 10 --retry-connrefused --retry-delay 1 -H "$table_accept" \
        "http://127.0.0.1:$table_port$2" | "${jqs[@]}" > "$tmp"
      cat "$tmp" > "tests/fixtures/$1.json"
      rm -f "$tmp"
    }
    # The default `includeObject`, which is what k8rs sends (no query string at
    # all — `src/k8s.rs` § THE BROWSER'S ROWS keeps the server default rather
    # than asking for it). Every row carries a `PartialObjectMetadata`.
    table table-pods /api/v1/namespaces/kube-system/pods
    # And `?includeObject=None`, which k8rs does *not* send: it is here so the
    # decode is proven against both shapes a Table arrives in rather than the one
    # this repo happens to ask for.
    table table-deployments '/apis/apps/v1/deployments?includeObject=None'

    # The bytes, and every guard names the field rather than the file, for the
    # reason the rest of this recipe does: a capture of the wrong thing writes
    # perfectly valid JSON.
    guard table-pods.json "Table with columns on it — the whole of invariant 12 is that these come off the wire" \
      '.kind == "Table" and (.columnDefinitions | length) > 0 and (.rows | length) > 0'
    # Both sides of the priority split, because `screens/resources.md` draws the
    # `priority: 0` set and drops the rest: a capture where every column is
    # narrow proves the filter does nothing, and one where none is draws a blank
    # screen. `the_columns_come_from_the_server_and_two_kinds_never_share_one_list`
    # reads five and four off this file.
    guard table-pods.json "columns on both sides of priority 0 — the narrow set a screen draws and the wide set kubectl -o wide adds" \
      '([.columnDefinitions[] | select(.priority == 0)] | length) > 0 and ([.columnDefinitions[] | select(.priority > 0)] | length) > 0'
    # The identity `screens/resources.md` matches a finding onto a row by, and
    # every dialog names. It is the entire reason the default `includeObject` is
    # kept and its 19× paid, so a capture that lost it retires the decision
    # silently.
    guard table-pods.json "PartialObjectMetadata under every row, carrying the uid a finding is matched onto a row by" \
      'all(.rows[]; .object.kind == "PartialObjectMetadata" and (.object.metadata.uid | type) == "string" and (.object.metadata.name | type) == "string")'
    # **The canary for the sanitizer's own newest clause.** `node_names` learned
    # to read the cell under the column `columnDefinitions` calls `Node`
    # (scripts/sanitize.jq); if this capture carried no such column, or no node
    # name under it, that clause would be running on nothing and a foreign Table
    # would be refused by nobody — "found none" and "there were none" print the
    # same line.
    guard table-pods.json "node name in the cell under the Node column — the only thing scripts/sanitize.jq's Table clause can be exercised by" \
      '[.columnDefinitions | to_entries[] | select(.value.name == "Node") | .key] as $n
       | ($n | length) == 1 and ([.rows[] | .cells[$n[0]] | select(test("^k8rs-"))] | length) > 0'
    # The second shape, and the field whose absence is the point: `"object": null`
    # is what `?includeObject=None` sends, and the decode has to survive it
    # without inventing an identity.
    guard table-deployments.json "rows with no object under them — the ?includeObject=None shape, decoded by the same type as the one above" \
      '.kind == "Table" and (.rows | length) > 0 and all(.rows[]; .object == null)'
    # **A cell is not a string**, and this is the shape whose absence lets
    # `cells: Vec<String>` back in: a Deployment's `Up-to-date` and `Available`
    # arrive as JSON numbers, so a corpus without one would compile a decode that
    # fails on every Deployment table a real cluster serves.
    guard table-deployments.json "cell that is a JSON number — the shape a Vec<String> would have refused to deserialise" \
      '[.rows[].cells[] | select(type == "number")] | length > 0'

    kill "$table_proxy" 2>/dev/null || true
    rm -f "$table_log"
    trap - EXIT

    # --- THE THREE REPORT INPUTS THAT HAD NO POSITIVE AT ALL (NOTES § D129) ---
    # `poddisruptionbudgets.json` and `persistentvolumeclaims.json` were both
    # `"items": []` and every Service in `services.json` matched pods, so Drain
    # safety's whole reason for existing and Waste's headline row had nothing to
    # be proven on. Each of the three is asserted **with its negative in the same
    # List**, because that is the shape these arrive in: a file with one item in
    # it cannot tell a report that reads the field from one that names everything
    # it finds.
    #
    # Exact numbers rather than relations, for `statefulsets.json`'s reason: the
    # manifest fixes them, and `disruptionsAllowed == 0` is also true of a budget
    # that is blocked because its workload is broken — which is a different row.
    # 2 healthy of 2 expected against a `minAvailable` of 2 is the floor itself.
    guard poddisruptionbudgets.json "PDB at its floor — minAvailable equal to the replica count of a workload whose pods are all healthy, which is the budget that makes a drain of its node loop until the operator gives up (D46)" \
      '[.items[]? | select(.metadata.name == "broken-pdb-floor" and .spec.minAvailable == 2 and .status.expectedPods == 2 and .status.currentHealthy == 2 and .status.desiredHealthy == 2 and .status.disruptionsAllowed == 0)] | length == 1'
    guard poddisruptionbudgets.json "PDB with room left in it, in the same List — the negative that makes the row above falsifiable" \
      '[.items[]? | select(.metadata.name == "healthy-pdb-room" and .status.disruptionsAllowed >= 1)] | length == 1'

    # `Bound`, never `Pending`: a claim that reserved nothing is somebody else's
    # row. And the two sizes disagree because `ClaimSnapshot` reads
    # `status.capacity.storage` — what was actually provisioned — while
    # `spec.resources.requests.storage` is what was asked for; with both at 64Mi
    # a decode reading the request would be indistinguishable from the right one.
    guard persistentvolumeclaims.json "claim that is Bound and mounted by nothing — Waste's row for a disk that was reserved and then used by nothing, with the provisioned size differing from the requested one" \
      '[.items[]? | select(.metadata.name == "broken-unused-disk" and .status.phase == "Bound" and .status.capacity.storage == "128Mi" and .spec.resources.requests.storage == "64Mi")] | length == 1'
    guard persistentvolumeclaims.json "Bound claim a captured pod does mount (healthy-disk.json is the other half) — without it the report cannot be shown to do the join" \
      '[.items[]? | select(.metadata.name == "healthy-disk" and .status.phase == "Bound")] | length == 1'

    # Both sides of Waste's headline row on one capture, and the second guard is
    # the one that is easy to lose: `kubernetes` in `default` carries **no
    # selector at all**, which `ServiceSnapshot` says in so many words is not a
    # thing to report — a rule that flagged every Service with no endpoints would
    # flag it, and this is what keeps that object in the file.
    guard services.json "Service whose selector matches no pod — Waste's headline row, the 503 nobody can explain, beside the three whose selectors do match" \
      '[.items[]? | select(.metadata.name == "broken-noendpoints" and ((.spec.selector // {}) | length) > 0)] | length == 1'
    guard services.json "selector-less Service — endpoints managed elsewhere, which is never this row" \
      '[.items[]? | select(((.spec.selector // {}) | length) == 0)] | length > 0'

    # *Matches no pod* is a claim about what is behind the Service, and the
    # endpoint controller is what writes it down: a placeholder slice carrying no
    # endpoints. Keyed on the `kubernetes.io/service-name` label and never on the
    # slice's name — the controller generates that from `generateName`, which the
    # sanitizer deletes, so the name differs on every capture and the label is
    # the only stable handle (it is also the field `EndpointSliceSnapshot` reads).
    guard endpointslices.json "empty EndpointSlice behind that Service — the Service on its own cannot say whether anything answers it" \
      '[.items[]? | select(.metadata.labels["kubernetes.io/service-name"] == "broken-noendpoints")] | length == 1 and ((.[0].endpoints // []) | length) == 0'
    # And a populated one, which is both the negative of the row and the proof
    # that the sanitizer kept a node identifier it had never been shown in this
    # kind: `.endpoints[].nodeName` is a node name in a field no other capture
    # has, refused when it is foreign and kept intact when it is kind's own.
    guard endpointslices.json "EndpointSlice that does carry endpoints, one of them naming the node it runs on" \
      '[.items[]? | .endpoints[]? | select(.nodeName != null)] | length > 0'

    # --- THE ONE THE MACHINE HAS TO MAKE, AND IT GOES HERE ---
    # Everything above is on disk before a node is touched, and this is the first
    # step that touches one: `break-runtime` reboots the node broken-reboot is
    # on. A reboot raises `restartCount` on **every** pod on that worker, and
    # `restarts.json` is guarded at exactly three a hundred lines up — so this
    # cannot move earlier, and it is before `break-nodes` because a cordoned,
    # tainted or kubelet-less worker is not a machine it can be made on.
    scripts/cluster.sh break-runtime
    "${kc[@]}" get pod broken-reboot -o json | "${jqs[@]}" > tests/fixtures/reboot.json
    # `(255, "Unknown")` as a pair, because `ending` reads it as one: 255 with any
    # other reason is a program that called `exit 255`, which is an ordinary
    # failure and not a node event.
    guard reboot.json "restart count past RESTARTS_WARN on a container that is serving, over a last run the runtime ended with 255/Unknown — rule 5's producer without rule 1's, and the capture that retires the plant for Ending::CodeUnknown (D90)" \
      '[.status.containerStatuses[]? | select(.ready == true and .state.running != null and .restartCount >= 3 and .lastState.terminated.exitCode == 255 and .lastState.terminated.reason == "Unknown")] | length > 0'

    # --- THE NODES, LAST ---
    # Every pod capture is on disk before the step above rebooted anything, and
    # every capture of any kind is on disk before this one, which is where the
    # damage stops being repairable by the machine itself: a reboot ends with the
    # node back and the pods running, a cordon does not. The ordering is the
    # whole design: a cordon changes where a pod would go, a
    # NoExecute taint evicts what is already there, and a stopped kubelet turns
    # every pod on that node Unknown within a minute. Any of those lands in a
    # pod capture as a state no manifest asked for. `break-nodes` asserts all
    # three states itself (it shares cluster.sh's predicate table), so what is
    # left here is the same assertion made against the sanitized bytes — plus
    # the two claims that can only be made once the bytes exist: the `timeAdded`
    # the sanitizer could quietly drop, and the join with the pod captures, which
    # is the only place both halves of N2 are on disk at the same time.
    scripts/cluster.sh break-nodes
    "${kc[@]}" get nodes -A -o json | "${jqs[@]}" > tests/fixtures/nodes.json
    guard nodes.json "cordoned worker that is otherwise healthy, carrying the taint the controller adds beside the field kubectl sets (N2)" \
      '[.items[] | select(.spec.unschedulable == true and ([.status.conditions[] | select(.type == "Ready") | .status] | first) == "True" and ([.spec.taints[]? | select(.key == "node.kubernetes.io/unschedulable" and .effect == "NoSchedule")] | length) > 0)] | length > 0'
    guard nodes.json "worker tainted dedicated=gpu:NoExecute — key, value and effect, which is all kubectl taint writes (N6)" \
      '[.items[] | select([.spec.taints[]? | select(.key == "dedicated" and .value == "gpu" and .effect == "NoExecute")] | length > 0)] | length > 0'
    # The timestamp, and it is read off a taint *nobody typed*: `kubectl taint`
    # writes no timeAdded (k/k #113044), the node controller stamps one on every
    # taint it adds for itself, and this is the only decode of that field a
    # capture can hold honestly.
    guard nodes.json "taint carrying a timeAdded — the node controller stamps one, kubectl stamps none" \
      '[.items[] | select([.spec.taints[]? | select(.key == "node.kubernetes.io/unreachable" and .effect == "NoExecute" and .timeAdded != null)] | length > 0)] | length > 0'
    guard nodes.json "node whose kubelet stopped posting (N1)" \
      '[.items[] | select([.status.conditions[] | select(.type == "Ready" and .status == "Unknown")] | length > 0)] | length > 0'

    # N2's positive is a join, and this is the only place both halves of it are
    # on disk. "Cordoned" alone is N2's *negative* when the only pods left are a
    # DaemonSet's — so the cordoned node has to be one a committed pod capture is
    # actually running on, or the snapshot a rule test builds proves the opposite
    # of what its name says (NOTES § N-series, D46). `break-nodes` picks such a
    # node; this is the same claim made against the bytes, after the sanitizer,
    # and it is what fails if the pick ever drifts from the capture.
    #
    # Fed every fixture: `.spec.nodeName` is null on the List captures and on the
    # unschedulable pod, so what is left is exactly the single-pod captures, all
    # of which are bare or ReplicaSet-owned — pods a drain moves.
    jq -e -n --slurpfile nodes tests/fixtures/nodes.json \
      '[$nodes[0].items[] | select(.spec.unschedulable == true) | .metadata.name] as $cordoned
       | any(inputs | .spec.nodeName? // empty; IN($cordoned[]))' \
      tests/fixtures/*.json >/dev/null \
      || { echo "fixtures: no captured pod is running on the cordoned node — the joined snapshot is N2's negative under N2's name" >&2; exit 1; }

    "${kc[@]}" version -o json | jq -r .serverVersion.gitVersion > tests/fixtures/K8S_VERSION
    echo "captured $(ls tests/fixtures | wc -l) fixtures from $(cat tests/fixtures/K8S_VERSION)"
    echo "the cluster is left broken on purpose — scripts/cluster.sh unbreak puts the nodes back"

# Body lands in Phase 7, the target is declared now.
#
# End-to-end write path against kind, in --read-only mode and with the operations enabled
e2e:
    @echo "not yet — Phase 7 writes this recipe (ops.rs against a real cluster)"
    @exit 1
