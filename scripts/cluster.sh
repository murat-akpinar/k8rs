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

  local names=(oom crashloop image config pending hostpath readiness nolimits stuck init)
  local -A want=(
    [oom]='.status.containerStatuses[0] | (.lastState.terminated // .state.terminated // {}) | .reason=="OOMKilled" and .exitCode==137'
    [crashloop]='.status.containerStatuses[0] | .state.waiting.reason=="CrashLoopBackOff" and .lastState.terminated.exitCode==1'
    [image]='.status.containerStatuses[0].state.waiting.reason | .=="ImagePullBackOff" or . =="ErrImagePull"'
    [config]='.status.containerStatuses[0].state.waiting.reason=="CreateContainerConfigError"'
    [pending]='.status.phase=="Pending" and ([.status.conditions[]?|select(.type=="PodScheduled")|.reason]|first)=="Unschedulable"'
    [hostpath]='.status.phase=="Running" and ([.spec.volumes[]?|select(.hostPath!=null)]|length)>0'
    [readiness]='.status.phase=="Running" and .status.containerStatuses[0].ready==false'
    [nolimits]='.status.phase=="Running" and (.spec.containers[0].resources.limits==null)'
    [stuck]='.status.phase=="Running" and ((.metadata.finalizers//[])|length)>0'
    [init]='([.status.initContainerStatuses[]?|select(.state.waiting.reason=="CrashLoopBackOff" or .lastState.terminated.exitCode==1)]|length)>0'
  )
  local -A why=(
    [oom]="rule 2 — killed for exceeding its memory limit"
    [crashloop]="rules 1+6 — CrashLoopBackOff after exit 1"
    [image]="rule 3 — the image cannot be pulled"
    [config]="rule 4 — a ConfigMap the pod needs does not exist"
    [pending]="rule 10 — nothing can schedule it"
    [hostpath]="rule 8 — a writable mount of the node's filesystem"
    [readiness]="rules 7+11 — running, but never ready"
    [nolimits]="rule 9 — no limits (a Capacity row, not an alert)"
    [stuck]="rule 12 — a finalizer nothing removes"
    [init]="D27 — the init container fails, so the app container never starts"
  )

  local pending_list=("${names[@]}") still fail=0
  while [ ${#pending_list[@]} -gt 0 ] && [ $SECONDS -lt $deadline ]; do
    still=()
    for n in "${pending_list[@]}"; do
      if ! "${kc[@]}" get pod "broken-$n" -o json 2>/dev/null \
           | jq -e "${want[$n]}" >/dev/null 2>&1; then
        still+=("$n")
      fi
    done
    pending_list=("${still[@]}")
    [ ${#pending_list[@]} -gt 0 ] && sleep 10
  done

  for n in "${names[@]}"; do
    if "${kc[@]}" get pod "broken-$n" -o json 2>/dev/null \
       | jq -e "${want[$n]}" >/dev/null 2>&1; then
      printf '  PASS  broken-%-10s %s\n' "$n" "${why[$n]}"
    else
      printf '  FAIL  broken-%-10s %s\n' "$n" "${why[$n]}"
      "${kc[@]}" get pod "broken-$n" -o json 2>/dev/null \
        | jq -c '{phase:.status.phase, state:.status.containerStatuses[0].state, last:.status.containerStatuses[0].lastState}' \
        | sed 's/^/          got: /' || echo "          got: pod not found"
      fail=1
    fi
  done
  # W1 is not a pod: the whole point is that no pod exists. The truth lives on
  # the ReplicaSet the Deployment made, and nowhere else.
  local rf='[.items[].status.conditions[]?|select(.type=="ReplicaFailure" and .status=="True")]|length>0'
  if "${kc[@]}" get replicasets -n k8rs-quota -o json 2>/dev/null | jq -e "$rf" >/dev/null 2>&1; then
    printf '  PASS  %-17s %s\n' "broken-quota" "D28/W1 — quota denies every pod, no pod object exists"
  else
    printf '  FAIL  %-17s %s\n' "broken-quota" "D28/W1 — expected ReplicaFailure on the ReplicaSet"
    "${kc[@]}" get replicasets -n k8rs-quota -o json 2>/dev/null \
      | jq -c '[.items[]|{name:.metadata.name, conditions:.status.conditions}]' \
      | sed 's/^/          got: /' || echo "          got: no replicasets in k8rs-quota"
    fail=1
  fi

  [ $fail -eq 0 ] && echo "  all 11 fixtures reached the state their rule is about"
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
