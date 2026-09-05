#!/usr/bin/env bash
# The cluster leg of the `--read-only` box (NOTES § D236 ruling 2), and the body of
# `just e2e`. `tests/binary.rs` § THE WIRE is the other half of that box: it watches
# the socket, because a kind apiserver cannot say what one client sent it. This half
# runs the real binary against a real apiserver and asserts what a cluster *can*
# show — the object did not change, `--read-only` opened no audit log at all, and the
# one row that is a question came back with the cluster's answer under the flag.
#
# **Each row is vetted for what it is, and the row itself says which** (`ops.rs` holds
# one call that is not a write, NOTES § D23 and § D230 ruling 3). An operation is
# refused under the flag and cancelled without it; the question is permitted and
# *answered* under both, which is the carve-out's own property and nothing else here
# proves it. The split is read off the row's tail exactly as `Advertised::mutates` reads
# it in tests/binary.rs, never matched against `may-i`.
#
# **It writes nothing into the cluster.** Every operation below is either refused by the
# flag or cancelled by an empty stdin, the question sends a `SelfSubjectAccessReview` and
# changes nothing, and the last thing it does is prove the object it was pointed at is
# byte for byte what it was.
#
# **Why it is a script and not the recipe's own body.** A `justfile` recipe cannot be
# run against fakes, and this one is all decisions — which context, which failure
# sentence, whether a row was vetted at all — that no cluster is needed to get wrong.
# `--self-test` drives every one of them with a fake `kubectl` and a fake binary, and
# it is in `scripts/guards.sh`, so `just check` covers the logic on a machine with no
# cluster (the house pattern: `mutants.sh`, `cluster.sh`, `fixture-audit.sh`).
#
# **The real run is not in `just check` and is not a CI job**, and that is not an
# oversight: REQUIREMENTS § CI refuses a kind job in v1 and requires the tests pass
# with no KUBECONFIG. It is the PM's to run (CLAUDE.md § The boxes no agent can run).
set -euo pipefail

# **The one knob, and it is the self-test's.** Everything else this script talks to
# is found on PATH, which is what lets the fakes stand in front of it.
default_bin="${CARGO_TARGET_DIR:-target}/debug/k8rs"

e2e() {
  local k8rs="${K8RS_E2E_BIN:-$default_bin}"

  # **The binary picks the context, not this script** — an `ops` line takes no
  # `--context` (src/main.rs § THE OPERATIONS DRIVER), so k8rs uses whatever is
  # current and the only honest thing to do is follow it and say which one. Refusing
  # anything that is not kind is what stops `just e2e`, typed in the wrong terminal,
  # pointing three operations at somebody's real cluster.
  local ctx
  ctx=$(kubectl config current-context 2>/dev/null || true)
  if [ -z "$ctx" ]; then
    echo "e2e: no kubeconfig context is current, so there is no apiserver to prove anything against — 'just cluster-up' brings the test cluster up" >&2
    return 1
  fi
  case "$ctx" in
    kind-*) ;;
    *) echo "e2e: the current context is '$ctx' and this only runs against kind — an ops line takes no --context, so the binary would use that context too. Switch with 'kubectl config use-context kind-${K8RS_CLUSTER:-k8rs}'." >&2; return 1 ;;
  esac
  if ! kubectl --context "$ctx" get --raw /version >/dev/null 2>&1; then
    echo "e2e: '$ctx' is the current context but its apiserver did not answer — 'just cluster-up' brings it back" >&2
    return 1
  fi

  local ns=default name=healthy-deploy
  if ! kubectl --context "$ctx" get deployment "$name" -n "$ns" >/dev/null 2>&1; then
    echo "e2e: there is no deployment/$name in $ns on '$ctx' — this needs the workload scripts/healthy.yaml creates, and a cluster without it would vet a refusal instead of an operation" >&2
    return 1
  fi

  # Every run gets a state directory of its own, so nothing here can append to the
  # reader's own audit log — and whether it exists afterwards is the assertion.
  # Cleanup is in the trap and not on the last line (NOTES § D185).
  local work
  work=$(mktemp -d)
  trap "rm -rf '$work'" EXIT
  export XDG_STATE_HOME="$work/state"

  # **What "the object did not change" is read off, and what is deliberately not in
  # it.** `status` and `metadata.resourceVersion` both move when the deployment's own
  # controller writes, which falsifies nothing anybody agreed to (NOTES § D228) — a
  # witness carrying either would go red on a healthy cluster. `generation` moves on
  # a spec write, which is exactly what scale and restart are; `uid` and
  # `deletionTimestamp` are what a delete leaves behind.
  witness() {
    kubectl --context "$ctx" get deployment "$name" -n "$ns" -o json \
      | jq -S '{uid: .metadata.uid, generation: .metadata.generation, deleting: .metadata.deletionTimestamp, spec: .spec}'
  }
  local before after copies rows row form promise mutates line how code
  local audits=0 answered=0 operations=0 questions=0
  before=$(witness)

  # A count the object is *not* already at, or a scale that leaked would patch
  # spec.replicas to the value it already holds, bump no generation, and leave this
  # whole run green.
  copies=$(jq -r 'if .spec.replicas == 1 then 2 else 1 end' <<<"$before")

  # **The operations are the binary's own** (NOTES § D234 ruling 2): `ops_usage` is
  # built from OPERATIONS, so a fourth one joins this loop with no edit here.
  # `tests/binary.rs` § THE WIRE fills the same placeholders for the same reason; the
  # table is four entries, and both ends refuse a placeholder they do not know rather
  # than running a line the driver throws out for its shape — which would vet a
  # refusal and print exactly the same success line.
  rows=$("$k8rs" ops 2>&1 | sed -n 's/^  ops //p') || true
  if [ -z "$rows" ]; then
    echo "e2e: 'k8rs ops' advertised nothing, so the loop below is about nothing — 'extracted none' and 'there were none' print the same line" >&2
    return 1
  fi

  # **One leg of one row**, with the flag or without it. `$line` is unquoted on purpose —
  # it is the argument vector, not one word. Nothing an `ops` line says goes to stdout
  # (NOTES § D220 ruling 3), so a byte there is a defect whichever row produced it, and
  # that is the one check both kinds of row share.
  #
  # **`how` is the whole line and not the part this loop built**, because every sentence
  # below quotes it and an operator who retypes one without `-n` is answered about the
  # namespace rather than about the thing that failed.
  leg() {
    how="ops $line -n $ns"
    [ $# -gt 0 ] && how="$* $how"
    rm -rf "$XDG_STATE_HOME"
    code=0
    # shellcheck disable=SC2086
    "$k8rs" "$@" ops $line -n "$ns" </dev/null >"$work/out" 2>"$work/said" || code=$?
    if [ -s "$work/out" ]; then
      echo "e2e: '$how' wrote to stdout, where a report goes" >&2
      return 1
    fi
  }

  # **What the question owes, and it is the same both times** — `--read-only` is the whole
  # difference between the two legs and NOTES § D230 ruling 3 says there is to be none.
  #
  # **An assertion and not a pass-through.** *Permitted and answered* is exit 0 or 1,
  # which is a verdict off a cluster; a refusal and a could-not-tell are both 2
  # (`may_i_ended`, src/main.rs). Clearing "wrote no stdout, opened no audit log" is also
  # what a row that never left the process does, and under the flag that is precisely the
  # regression this leg exists to catch.
  answer() {
    if [ -e "$XDG_STATE_HOME/k8rs" ]; then
      echo "e2e: '$how' opened an audit log, and the one ops line that is a question records nothing:" >&2
      cat "$XDG_STATE_HOME/k8rs/audit.log" >&2 || true
      return 1
    fi
    if [ "$code" -gt 1 ]; then
      echo "e2e: '$how' did not come back with an answer (exit $code) — the question is the one ops line --read-only permits (NOTES § D230 ruling 3), and this run was refused or could not tell:" >&2
      cat "$work/said" >&2
      return 1
    fi
  }

  while IFS= read -r row; do
    # **The row says which side of this it is on, and its tail is what says it** — the
    # same test `Advertised::mutates` makes in tests/binary.rs § THE ONE DOOR. Every
    # operation's line ends in how it is confirmed and the one row that is not an
    # operation ends in *changes nothing*; read off the text rather than matched against
    # `may-i`, so a fourth operation joins the strict leg with no edit here, and a tail
    # that stops parsing lands on the operation side — the side with the stricter
    # assertions. The two counts are what makes that loud rather than silent.
    form=${row%% — *}
    promise=${row#* — }
    case "$promise" in
      *"changes nothing"*) mutates=0; questions=$((questions + 1)) ;;
      *)                   mutates=1; operations=$((operations + 1)) ;;
    esac

    # The squeeze at the end is the `[...]` strip's leftover: `[--subresource <name>]`
    # goes and both of the spaces around it stay, which is a double space in the line
    # every sentence below quotes.
    line=$(printf '%s' "$form" | sed \
      -e "s#<kind>/<name>#deploy/$name#" -e "s/<copies>/$copies/" \
      -e 's/<verb>/list/' -e 's#<resource>\.<group>\[/<name>\]#pods.#' \
      -e 's/\[[^]]*\]//g' -e 's/  */ /g' -e 's/ *$//')
    case "$line" in
      *'<'*) echo "e2e: '$row' carries a placeholder this script does not know, so the line built from it would be refused for its shape: '$line'" >&2; return 1 ;;
    esac

    # **The question, both legs, and it is the only row here that proves the binary
    # reached an apiserver at all.** Nothing is refused, nothing is recorded, and the
    # cluster answers twice.
    if [ "$mutates" = 0 ]; then
      leg --read-only || return 1
      answer || return 1
      leg || return 1
      answer || return 1
      answered=$((answered + 1))
      echo "e2e: ops $line — answered under --read-only and without it (exit $code)"
      continue
    fi

    # Leg 1: --read-only. Nothing may be attempted, so no audit log may be opened — and
    # the refusal is an exit 2 and not merely an absence, because *refused* and *did
    # nothing at all* leave the same empty state directory behind.
    leg --read-only || return 1
    if [ -e "$XDG_STATE_HOME/k8rs" ]; then
      echo "e2e: '$how' opened an audit log, so something was attempted:" >&2
      cat "$XDG_STATE_HOME/k8rs/audit.log" >&2 || true
      return 1
    fi
    if [ "$code" -ne 2 ]; then
      echo "e2e: '$how' exited $code, and an operation the flag refused exits 2:" >&2
      cat "$work/said" >&2
      return 1
    fi

    # Leg 2: the operations enabled and nobody at the keyboard. `</dev/null` is a CI
    # step, a cron entry and a pipeline, and it is the unattended default
    # NOTES § D218 says must be *no* — the whole difference between reading a
    # confirmation off stdin and a --yes flag, and the one a regression removes in
    # silence.
    #
    # **The audit log is required and not merely inspected if present.** A cancelled
    # operation writes an attempt and a result (invariant 2), so a row that recorded
    # nothing is a defect — and while it was only inspected, it was also what made the
    # count in the last line true by luck rather than by the reason it gives.
    #
    # **`-s` and not `-e`, because an empty one is a state a run reaches.** Measured on
    # the built binary with an unreadable kubeconfig: `ops::audit_log` opens the file
    # before the call, so `$XDG_STATE_HOME/k8rs/audit.log` is there at mode 0600 with
    # nothing in it. *Opened and never written* is *recorded nothing*, and it is the
    # sentence below rather than the one under it complaining about a record it cannot
    # read.
    leg || return 1
    if [ ! -s "$XDG_STATE_HOME/k8rs/audit.log" ]; then
      echo "e2e: '$how' was cancelled and recorded nothing, and every attempt reaches the audit log (invariant 2):" >&2
      cat "$work/said" >&2
      return 1
    fi
    audits=$((audits + 1))
    if [ "$code" -eq 0 ]; then
      echo "e2e: '$how' was never confirmed and exited 0, so 'k8rs ops … && …' runs on" >&2
      return 1
    fi
    if ! tail -1 "$XDG_STATE_HOME/k8rs/audit.log" | grep -q "nothing was changed"; then
      echo "e2e: '$how' was never confirmed and the record does not say so:" >&2
      cat "$XDG_STATE_HOME/k8rs/audit.log" >&2
      return 1
    fi
    echo "e2e: ops $line — refused under --read-only, cancelled and recorded without it (exit $code)"
  done <<<"$rows"

  # **Both sides of the split were vetted** (CLAUDE.md § A derived list asserts it found
  # something). `mutates` is a string test on a row's own tail: if that wording drifts,
  # every row lands on one side, every check above still runs, and the run goes green
  # about half of what its last line says it covered.
  if [ "$operations" -eq 0 ]; then
    echo "e2e: not one advertised row said how it is confirmed, so leg 2 vetted no operation — either the operations are gone or the tail they are read off has moved" >&2
    return 1
  fi
  if [ "$questions" -eq 0 ]; then
    echo "e2e: not one advertised row said it changes nothing, so nothing here vetted the question --read-only permits (NOTES § D230 ruling 3)" >&2
    return 1
  fi

  after=$(witness)
  if [ "$before" != "$after" ]; then
    echo "e2e: deployment/$name in $ns changed while nothing was ever confirmed:" >&2
    diff <(printf '%s\n' "$before") <(printf '%s\n' "$after") >&2 || true
    return 1
  fi
  # **Both numbers are counted off what happened**, not off the length of the row list:
  # `audits` is incremented where a cancelled attempt was found in a log and `answered`
  # where a verdict came back, and the checks in the loop are what make each of them equal
  # to its half of the advertised rows.
  echo "e2e: $ctx — deploy/$name unchanged, --read-only opened no audit log, $audits operations refused then cancelled, $answered questions answered under the flag and without it"
}

# --- SELF-TEST START ---
#
# One case per way this can be wrong plus two on what the happy path prints, driven by a
# fake `kubectl` on PATH and a fake binary. All but those two are failures that must be
# *loud*: a `just e2e` that exits 0 because there was no cluster, no deployment or no
# advertised operation is the invisible gap this whole file exists to close.
#
# **The count is printed and not written here**, because the last one written down went
# stale the turn a case was added.

self_test() {
  local sandbox pass=0
  sandbox=$(mktemp -d)
  trap "rm -rf '$sandbox'" EXIT
  mkdir -p "$sandbox/bin"

  cat > "$sandbox/bin/kubectl" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  "config current-context") [ -n "${FAKE_CTX:-}" ] || exit 1; echo "$FAKE_CTX" ;;
  *"get --raw /version"*) [ "${FAKE_UP:-1}" = 1 ] || exit 1; echo '{"gitVersion":"v1.36.1"}' ;;
  *"-o json"*) cat "$FAKE_OBJECT" ;;
  *"get deployment"*) [ "${FAKE_OBJECT_EXISTS:-1}" = 1 ] || exit 1 ;;
  *) exit 1 ;;
esac
FAKE

  # The fake binary. `FAKE_ROWS` is what it advertises; `FAKE_MODE` is which of the
  # leaks it performs. Its audit log and its object are files, which is exactly what
  # the real ones are.
  cat > "$sandbox/bin/k8rs" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
readonly=0
for arg in "$@"; do [ "$arg" = "--read-only" ] && readonly=1; done
if [ "$*" = "ops" ]; then printf '%s\n' "${FAKE_ROWS}"; exit 2; fi
record() { mkdir -p "$XDG_STATE_HOME/k8rs"; printf '%s\n' "$1" >> "$XDG_STATE_HOME/k8rs/audit.log"; }
# The question answers under the flag and without it, which is what the real one does
# (NOTES § D230 ruling 3) — so it is decided before `--read-only` is read, exactly as
# `ops_line` decides it.
case " $* " in
  *" may-i "*)
    case "${FAKE_MODE:-}" in
      question_refused) exit 2 ;;
      question_records) record "attempt · asked something"; exit 0 ;;
    esac
    exit 0 ;;
esac
if [ "$readonly" = 1 ]; then
  [ "${FAKE_MODE:-}" = "audit_under_readonly" ] && record "attempt · nothing was changed"
  [ "${FAKE_MODE:-}" = "op_not_refused" ] && exit 0
  exit 2
fi
case "${FAKE_MODE:-}" in
  exit0)   record "result · nothing was changed"; exit 0 ;;
  silent)  exit 2 ;;
  # The log opened and nothing written — what the real binary leaves behind when the call
  # never went out (measured, unreadable kubeconfig).
  empty)   mkdir -p "$XDG_STATE_HOME/k8rs"; : > "$XDG_STATE_HOME/k8rs/audit.log"; exit 2 ;;
  # One operation records and one does not — the shape a per-row requirement catches and
  # a count compared against zero does not.
  half)    case " $* " in *" scale "*) exit 2 ;; esac
           record "result · nothing was changed"; exit 2 ;;
  lies)    record "result · the change was made"; exit 2 ;;
  changes) record "result · nothing was changed"
           jq '.metadata.generation += 1' "$FAKE_OBJECT" > "$FAKE_OBJECT.new"
           mv "$FAKE_OBJECT.new" "$FAKE_OBJECT"; exit 2 ;;
  *)       record "result · nothing was changed"; exit 2 ;;
esac
FAKE
  chmod +x "$sandbox/bin/kubectl" "$sandbox/bin/k8rs"

  cat > "$sandbox/object.json" <<'OBJ'
{"metadata":{"uid":"9f0b","generation":3},"spec":{"replicas":2}}
OBJ

  # `run <name> <expected exit> <expected words> [VAR=value ...]`
  run() {
    local name="$1" want="$2" says="$3"; shift 3
    local out code=0
    out=$(env PATH="$sandbox/bin:$PATH" K8RS_E2E_BIN="$sandbox/bin/k8rs" \
              FAKE_CTX=kind-k8rs FAKE_OBJECT="$sandbox/object.json" \
              "$@" bash "$0" 2>&1) || code=$?
    if [ "$code" != "$want" ] || ! grep -qF -- "$says" <<<"$out"; then
      echo "e2e --self-test: [$name] wanted exit $want saying '$says', got exit $code:" >&2
      printf '%s\n' "$out" >&2
      exit 1
    fi
    pass=$((pass + 1))
  }

  local question="  ops may-i <verb> <resource>.<group>[/<name>] [--subresource <name>] — changes nothing"
  local operation="  ops scale <kind>/<name> <copies> — say yes to confirm"
  local rows="$operation
  ops delete <kind>/<name> — type the object's own name to confirm
$question"

  run "no context"        1 "no kubeconfig context is current"      FAKE_CTX= FAKE_ROWS="$rows"
  run "not kind"          1 "only runs against kind"                FAKE_CTX=production FAKE_ROWS="$rows"
  run "apiserver down"    1 "did not answer"                        FAKE_UP=0 FAKE_ROWS="$rows"
  run "no deployment"     1 "there is no deployment/healthy-deploy" FAKE_OBJECT_EXISTS=0 FAKE_ROWS="$rows"
  run "advertises none"   1 "advertised nothing"                    FAKE_ROWS=""
  run "unknown hole"      1 "carries a placeholder this script does not know" \
      FAKE_ROWS="  ops purge <kind>/<name> <age> — say yes to confirm"
  run "audit under flag"  1 "opened an audit log, so something was attempted" \
      FAKE_MODE=audit_under_readonly FAKE_ROWS="$rows"
  run "not refused"       1 "and an operation the flag refused exits 2" \
      FAKE_MODE=op_not_refused FAKE_ROWS="$rows"
  run "cancelled exit 0"  1 "exited 0"               FAKE_MODE=exit0   FAKE_ROWS="$rows"
  run "record lies"       1 "the record does not say so" FAKE_MODE=lies FAKE_ROWS="$rows"
  run "nothing recorded"  1 "was cancelled and recorded nothing" FAKE_MODE=silent FAKE_ROWS="$rows"
  # **The one the count could not catch.** While the audit log was inspected-if-present
  # and the only aggregate check was `audits -eq 0`, one operation recording and one not
  # was green — and the last line then said *1 cancelled attempts recorded* about two.
  run "one of two silent" 1 "was cancelled and recorded nothing" FAKE_MODE=half FAKE_ROWS="$rows"
  run "log opened empty" 1 "was cancelled and recorded nothing"  FAKE_MODE=empty FAKE_ROWS="$rows"
  run "object changed"    1 "changed while nothing was ever confirmed" FAKE_MODE=changes FAKE_ROWS="$rows"

  # **The question's own four**: it is refused under the flag, it records, and either half
  # of the split ends up empty. The last two are the canary — with `mutates` read off a
  # row's tail, a wording drift lands every row on one side and every check above still
  # passes.
  run "question refused"  1 "did not come back with an answer" \
      FAKE_MODE=question_refused FAKE_ROWS="$rows"
  run "question records"  1 "the one ops line that is a question records nothing" \
      FAKE_MODE=question_records FAKE_ROWS="$rows"
  run "no operation"      1 "leg 2 vetted no operation"  FAKE_ROWS="$question"
  run "no question"       1 "nothing here vetted the question" FAKE_ROWS="$operation"

  run "the happy path"    0 "unchanged, --read-only opened no audit log, 2 operations refused then cancelled, 1 questions answered under the flag and without it" \
      FAKE_ROWS="$rows"
  # The question's own line, which is what this box is about: it says what it did rather
  # than borrowing the operations' sentence, and the `[...]` strip no longer leaves a
  # double space in it.
  run "the question's line" 0 "e2e: ops may-i list pods. — answered under --read-only and without it (exit 0)" \
      FAKE_ROWS="$rows"

  echo "e2e --self-test: $pass cases"
}
# --- SELF-TEST END ---

if [ "${1:-}" = "--self-test" ]; then self_test; else e2e; fi
