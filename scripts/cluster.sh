#!/usr/bin/env bash
# k8rs test cluster — create it, destroy it, fill it with broken pods.
#
# Runs wherever docker + kind live. For a cluster on another machine, set
# K8RS_APISERVER_ADDRESS to that machine's address before `up`: kind writes
# 127.0.0.1 into the kubeconfig by default, which no other host can reach.
#
#   ./scripts/cluster.sh up          create the cluster
#   ./scripts/cluster.sh break       apply the deliberately broken pods
#   ./scripts/cluster.sh verify      assert each pod reached the state its rule needs
#   ./scripts/cluster.sh status      nodes, demo pods, memory
#   ./scripts/cluster.sh unbreak     remove them (clears the stuck finalizer)
#   ./scripts/cluster.sh reset       down + up + break
#   ./scripts/cluster.sh down        delete the cluster
#
# This is todo.md Phase 1's `just cluster-up` / `cluster-down`, pulled forward
# because the design needed a real cluster to check its assumptions against.

set -euo pipefail

CLUSTER="${K8RS_CLUSTER:-k8rs}"
ADDRESS="${K8RS_APISERVER_ADDRESS:-127.0.0.1}"
PORT="${K8RS_APISERVER_PORT:-6443}"
# Pinned on purpose: fixtures are only comparable against a known version, and
# the capture records this string (NOTES.md § kind test manifest).
NODE_IMAGE="${K8RS_NODE_IMAGE:-kindest/node:v1.36.1}"
WORKERS="${K8RS_WORKERS:-2}"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BROKEN="$HERE/broken.yaml"
HEALTHY="$HERE/healthy.yaml"

need() { command -v "$1" >/dev/null || { echo "$1 is not installed" >&2; exit 1; }; }

# --- CLUSTER START ---
kind_config() {
  echo "kind: Cluster"
  echo "apiVersion: kind.x-k8s.io/v1alpha4"
  echo "networking:"
  echo "  apiServerAddress: \"$ADDRESS\""
  echo "  apiServerPort: $PORT"
  echo "nodes:"
  echo "  - role: control-plane"
  # More than one node is not decoration: the N-series rules, drain safety and
  # version skew all need a cluster that can lose a node and keep going.
  for _ in $(seq "$WORKERS"); do echo "  - role: worker"; done
}

up() {
  need kind; need kubectl
  kind_config | kind create cluster --name "$CLUSTER" --image "$NODE_IMAGE" --config -
  kubectl --context "kind-$CLUSTER" wait --for=condition=Ready node --all --timeout=120s
  echo
  echo "API: https://$ADDRESS:$PORT   context: kind-$CLUSTER"
  [ "$ADDRESS" = "127.0.0.1" ] && echo "(local only — set K8RS_APISERVER_ADDRESS to reach it from another machine)"
  return 0
}

down() {
  need kind
  kind delete cluster --name "$CLUSTER"
}
# --- CLUSTER END ---

# --- FIXTURES START ---
break_it() {
  need kubectl
  kubectl --context "kind-$CLUSTER" apply -f "$BROKEN"
  # The healthy side goes up with the broken one: a rule needs both fixtures,
  # and capturing them from the same cluster at the same time is what makes
  # the negative test comparable to the positive one.
  kubectl --context "kind-$CLUSTER" apply -f "$HEALTHY"
  echo
  echo "States need a few minutes to settle — CrashLoopBackOff has to enter"
  echo "backoff and the OOM kill has to actually happen. Check with: $0 status"
  echo
  echo "Rule 12 (stuck Terminating) is not applied by this script; it is a"
  echo "capture step: kubectl delete pod broken-stuck --wait=false"
}

unbreak() {
  need kubectl
  local kc=(kubectl --context "kind-$CLUSTER")
  # broken-stuck carries a finalizer nothing ever removes — that is the point
  # of the fixture, and it is why a plain delete would hang here forever.
  "${kc[@]}" patch pod broken-stuck -p '{"metadata":{"finalizers":null}}' 2>/dev/null || true
  "${kc[@]}" delete pod -l demo=broken --wait=false --ignore-not-found
  "${kc[@]}" delete pod,deployment -l demo=healthy --wait=false --ignore-not-found
  # The W1 fixture lives in its own namespace (a pods: "0" quota would
  # otherwise block every pod above from being recreated).
  "${kc[@]}" delete namespace k8rs-quota --wait=false --ignore-not-found
}

# A fixture that never reaches the state its rule is about is a test that
# cannot fail. Assert the state on the cluster *before* anything is captured
# from it — a lie caught here costs minutes, the same lie caught after the
# fixture is committed costs a rule everyone trusts and nobody tested.
verify() {
  need kubectl; need jq
  local kc=(kubectl --context "kind-$CLUSTER")
  local deadline=$(( SECONDS + ${K8RS_VERIFY_TIMEOUT:-420} ))

  # Not every check is a pod: W1's whole point is that no pod object exists, and
  # W2 lives on the Deployment. One fetch keeps every check on the same wait
  # loop, the same timeout and the same output.
  fetch() {
    case "$1" in
      quota) "${kc[@]}" get replicasets -n k8rs-quota -o json ;;
      w2)    "${kc[@]}" get deployment broken-quota -n k8rs-quota -o json ;;
      *)     "${kc[@]}" get pod "broken-$1" -o json ;;
    esac 2>/dev/null
  }

  local names=(oom crashloop image config pending hostpath readiness restarts nolimits stuck init quota w2)
  local -A want=(
    [oom]='.status.containerStatuses[0] | (.lastState.terminated // .state.terminated // {}) | .reason=="OOMKilled" and .exitCode==137'
    [crashloop]='.status.containerStatuses[0] | .state.waiting.reason=="CrashLoopBackOff" and .lastState.terminated.exitCode==1'
    [image]='.status.containerStatuses[0].state.waiting.reason | .=="ImagePullBackOff" or . =="ErrImagePull"'
    [config]='.status.containerStatuses[0].state.waiting.reason=="CreateContainerConfigError"'
    [pending]='.status.phase=="Pending" and ([.status.conditions[]?|select(.type=="PodScheduled")|.reason]|first)=="Unschedulable"'
    [hostpath]='.status.phase=="Running" and ([.spec.volumes[]?|select(.hostPath!=null)]|length)>0'
    [readiness]='.status.phase=="Running" and .status.containerStatuses[0].ready==false'
    [restarts]='.status.phase=="Running" and (.status.containerStatuses[0] | .ready==true and .restartCount>=3)'
    [nolimits]='.status.phase=="Running" and (.spec.containers[0].resources.limits==null)'
    [stuck]='.status.phase=="Running" and ((.metadata.finalizers//[])|length)>0'
    [init]='([.status.initContainerStatuses[]?|select(.state.waiting.reason=="CrashLoopBackOff" or .lastState.terminated.exitCode==1)]|length)>0'
    [quota]='[.items[].status.conditions[]?|select(.type=="ReplicaFailure" and .status=="True")]|length>0'
    [w2]='[.status.conditions[]?|select(.type=="Progressing" and .status=="False" and .reason=="ProgressDeadlineExceeded")]|length>0'
  )
  local -A why=(
    [oom]="rule 2 — killed for exceeding its memory limit"
    [crashloop]="rules 1+6 — CrashLoopBackOff after exit 1"
    [image]="rule 3 — the image cannot be pulled"
    [config]="rule 4 — a ConfigMap the pod needs does not exist"
    [pending]="rule 10 — nothing can schedule it"
    [hostpath]="rule 8 — a writable mount of the node's filesystem"
    [readiness]="rule 7 — the container runs, but never passes its readiness probe"
    [restarts]="rule 5 — Running and ready now, but it has restarted repeatedly"
    [nolimits]="rule 9 — no limits (a Capacity row, not an alert)"
    [stuck]="rule 12 — a finalizer nothing removes (the delete is part of the capture)"
    [init]="D27 — the init container fails, so the app container never starts"
    [quota]="D28/W1 — quota denies every pod, no pod object exists"
    [w2]="D28/W2 — the rollout gave up (ProgressDeadlineExceeded)"
  )

  # What it actually is, in one line, for the FAIL case. Covers a single object
  # and a List, and drops the keys that do not apply instead of printing nulls.
  local diag='{ name:       .metadata.name,
                phase:      .status.phase,
                state:      (.status.containerStatuses // [])[0].state,
                last:       (.status.containerStatuses // [])[0].lastState,
                restarts:   (.status.containerStatuses // [])[0].restartCount,
                init:       (.status.initContainerStatuses // [])[0].state,
                conditions: [ (.status.conditions // [])[] | {type,status,reason} ],
                items:      [ (.items // [])[] | { name: .metadata.name,
                                conditions: [ (.status.conditions // [])[] | {type,status,reason} ] } ] }
              | with_entries(select(.value != null and .value != []))'

  local pending_list=("${names[@]}") still fail=0 got
  while [ ${#pending_list[@]} -gt 0 ] && [ $SECONDS -lt $deadline ]; do
    still=()
    for n in "${pending_list[@]}"; do
      fetch "$n" | jq -e "${want[$n]}" >/dev/null 2>&1 || still+=("$n")
    done
    pending_list=("${still[@]}")
    [ ${#pending_list[@]} -gt 0 ] && sleep 10
  done

  for n in "${names[@]}"; do
    if fetch "$n" | jq -e "${want[$n]}" >/dev/null 2>&1; then
      printf '  PASS  %-10s %s\n' "$n" "${why[$n]}"
    else
      printf '  FAIL  %-10s %s\n' "$n" "${why[$n]}"
      got=$(fetch "$n" | jq -c "$diag" 2>/dev/null) || got=
      printf '        got: %s\n' "${got:-object not found}"
      fail=1
    fi
  done

  [ $fail -eq 0 ] && echo "  all ${#names[@]} fixtures reached the state their rule is about"
  return $fail
}
# --- FIXTURES END ---

status() {
  need kubectl
  local kc=(kubectl --context "kind-$CLUSTER")
  "${kc[@]}" get nodes -o wide
  echo
  "${kc[@]}" get pods -l demo=broken -o wide 2>/dev/null || echo "(no demo pods — run: $0 break)"
  echo
  free -m 2>/dev/null | awk '/^Mem:/{printf "host memory: %s MiB used of %s, %s available\n", $3, $2, $7}'
}

case "${1:-}" in
  up)      up ;;
  down)    down ;;
  break)   break_it ;;
  verify)  verify ;;
  unbreak) unbreak ;;
  status)  status ;;
  reset)   down || true; up; break_it ;;
  *)       sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'; exit 1 ;;
esac
