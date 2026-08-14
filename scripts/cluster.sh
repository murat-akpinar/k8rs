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
#   ./scripts/cluster.sh break-nodes cordon / taint / stop a kubelet, and assert it
#   ./scripts/cluster.sh status      nodes, demo pods, memory
#   ./scripts/cluster.sh unbreak     remove them (clears the stuck finalizer,
#                                    uncordons, untaints, restarts the kubelet)
#   ./scripts/cluster.sh reset       down + up + break
#   ./scripts/cluster.sh down        delete the cluster
#
# `break-nodes` is deliberately not part of `break`, and it runs *after* the pod
# fixtures have been captured: it makes one node unschedulable, evicts whatever
# does not tolerate a NoExecute taint from a second, and kills the kubelet on a
# third. Any of those changes a pod fixture that is still being settled, so
# `verify` runs before it and the node capture after it (see the `fixtures`
# recipe in the justfile, which is the only caller that gets the order right).
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
# Three, because `break-nodes` gives each worker one broken state and doubling
# two of them onto one node would make each fixture ambiguous: a cordoned node
# that is also NotReady is not the "cordoned and forgotten" object N2 is about,
# and a taint on a node whose conditions are all Unknown proves nothing about
# either rule. A smaller box can still set K8RS_WORKERS=2 — everything except
# `break-nodes` works, and that one refuses out loud rather than doubling up.
WORKERS="${K8RS_WORKERS:-3}"

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
# A second revision that cannot start is what puts a workload in the one state
# nothing captured so far has: partially ready, and with two ReplicaSets under
# one Deployment. It has to be the *second* revision — a workload created broken
# has no ready pods to be partially ready with — so waiting for the first one is
# not politeness, it is the fixture: `set image` before those pods are up
# replaces a revision nobody was ever serving from.
#
# Waiting unconditionally is worse than useless on a workload an earlier `break`
# already broke: `rollout status` on a rollout that can never finish blocks for
# the whole timeout and then ends the script, which is how `break` came to work
# only on a cluster nobody had broken yet. What "already broke" can be read off,
# and when, is the whole of `scan_second_revisions` below.
#
# The wait stays a `rollout status` and does not become a `kubectl wait` on a
# counter: what it has to assert is "every replica is up on the current
# revision", which is what that one command already means, while the counter is
# named differently per kind (`readyReplicas`, `numberReady`) and would have to
# be spelled out three times. It was never this command's meaning that failed —
# only asking it where the answer could never be yes.
#
# The image is the same one `broken-image` and `broken-ds` use, spelled again
# because a manifest cannot read a shell variable — a registry that does not
# resolve, so the failure is the pull and never something on the network.
BAD_IMAGE=registry.invalid/does-not-exist:v9

# The workloads whose second revision is a *change* to a running object, in one
# list because the scan and the break must never name different sets: a workload
# added to the second and not the first walks straight back into the wait it
# cannot survive.
SECOND_REVISION=(deployment/broken-rollout statefulset/broken-sts)
ALREADY_BROKEN=""

# --- WHAT A SECOND `break` CANNOT LEARN AFTER THE APPLY ---
# `kubectl apply` puts the *good* template back on both of these workloads —
# broken.yaml is deliberately the first revision — so after it the image always
# reads busybox, and a guard that asks the template whether this workload is
# already on its second revision can only ever answer no. That guard was here,
# one line below the apply, and it was dead code: every second `break` walked
# into the wait it existed to skip. So this runs first, and carries its answer
# past the apply in $ALREADY_BROKEN.
#
# It reads the *pods* rather than the template, because the template is not what
# blocks. Measured on the run that took this script down: the apply had already
# put busybox back on `broken-sts` — `updateRevision` and `currentRevision` both
# naming the busybox revision — while `broken-sts-1` was still on the previous
# run's bad revision, not Ready, and the set no longer had that revision at all.
# Under `OrderedReady` the controller will not step past an unhealthy pod, so it
# never deletes pod-1, so pod-1 never comes back on a revision that could start:
# the newest event `describe` had was the create half an hour earlier, and the
# apply was followed by none at all. That is not slow convergence, and `rollout
# status` waits out its whole timeout on it every time.
# A template read cannot see that cluster at all — the template is good and the
# pod is not.
#
# `broken-rollout` cannot reach that state, and it is structure rather than luck.
# Measured on the same run: two lines before the StatefulSet hung, the Deployment
# printed "successfully rolled out" from the identical starting position. The
# reason is that a Deployment's replicas have no identity — the old revision's
# unready pod is deleted as soon as the new one has room, and it has room here,
# because `maxUnavailable: 0` holds the two busybox pods in place while the
# unready one is a surge pod *above* the desired count, so removing it costs no
# availability. `broken-ds` is never given a second revision at all: it is born
# on $BAD_IMAGE in the manifest and nothing here waits on it. Both are scanned
# anyway, because a guard that is only correct for the kind that happened to
# break is one nobody can trust with the third.
#
# Breaking it again is not a repair of the leftover pod, it is the same edit as
# the first time: the template returns to exactly what it was, so its revision
# hash does too, and the pod the earlier run left is already on it. What comes
# back is the state a first run leaves — one ready replica on the first revision,
# one that cannot start on the second — which is what `verify` has to agree with.
# That last step is where the ceiling of this guard is, and it is deliberate: if
# broken.yaml's template has changed since the run that left the pod, the hash no
# longer matches and the StatefulSet stays stuck. `break` then finishes anyway
# and `verify` says `FAIL sts` with the object printed under it, which is the
# same "unbreak first" answer the apply gives one screen up — loud and named,
# rather than five minutes of a wait that was never going to end.
#
# The selector is read off the workload instead of being spelled here, so it
# cannot drift from broken.yaml, and an empty one is skipped rather than handed
# to `-l`, where it would match every pod in the namespace — `broken-image` is
# one of them and it carries $BAD_IMAGE, so a *first* run would then skip the
# wait that is the whole reason there is a first revision to break.
scan_second_revisions() {
  local kc=(kubectl --context "kind-$CLUSTER") w sel img
  for w in "$@"; do
    # Silent only where silence is correct: no such workload yet is a first run.
    # Anything else that could fail these two reads — a dead API server, a
    # kubeconfig without the verb — is loud one command later, at the apply.
    sel=$("${kc[@]}" get "$w" -o json 2>/dev/null \
          | jq -r '(.spec.selector.matchLabels // {}) | to_entries
                   | map("\(.key)=\(.value)") | join(",")') || sel=
    [ -n "$sel" ] || continue
    # Every container, not `containers[0]`: which slot the app sits in is
    # broken.yaml's business, and a probe that only reads the first one is a
    # guard that stops working the day a sidecar is added above it.
    img=$("${kc[@]}" get pods -l "$sel" \
            -o jsonpath='{.items[*].spec.containers[*].image}' 2>/dev/null) || img=
    case " $img " in *" $BAD_IMAGE "*) ALREADY_BROKEN="$ALREADY_BROKEN $w " ;; esac
  done
}

second_revision() { # $1 kind/name — its container is called `app`
  local kc=(kubectl --context "kind-$CLUSTER")
  case "$ALREADY_BROKEN" in
    *" $1 "*)
      echo "  $1 still carries a pod from an earlier break — breaking it again without waiting" ;;
    *)
      echo "  waiting for the first revision of $1 before breaking the second..."
      "${kc[@]}" rollout status "$1" --timeout=300s ;;
  esac
  # Never skipped, on either path: the guard above takes away one wait and
  # nothing else, and a `set image` that fails still ends the run.
  "${kc[@]}" set image "$1" "app=$BAD_IMAGE"
}

break_it() {
  # jq for the scan below — `break` is the last subcommand here that did not
  # need it, and a capture trip needs it at every other step anyway.
  need kubectl; need jq
  local kc=(kubectl --context "kind-$CLUSTER") w

  # --- WHAT A SECOND `break` NEEDS, AND WHAT IT CANNOT HAVE ---
  # A pod spec is almost entirely immutable: `updatablePodSpecFields` is image,
  # activeDeadlineSeconds, tolerations (additions only), the grace period, and
  # — through one subresource — resources. So `apply` can be re-run over pods
  # *these* manifests created and over nothing else. Against a cluster carrying
  # an earlier generation of this file (a different command, a nodeSelector, a
  # mount, an init container's resources) the API rejects the update and `set
  # -e` ends `break` right here, before the healthy side, before the second
  # revisions, before the resize. That is the correct direction to fail in and
  # there is no patching around it — `unbreak` first, then this.
  #
  # The resize is the one case that is not the manifest's doing, and it made
  # every second run fail at the first command: a previous run left the live pod
  # at its node's whole memory while broken.yaml still says 64Mi, and
  # `spec.containers[*].resources` may not change through a pod update. Putting
  # it back through the one subresource that may change it makes the apply a
  # no-op change again. Guarded by a `get` rather than `|| true`, so a *real*
  # failure here is still loud —
  # there being no pod yet on a first run is the only silence that is correct.
  if "${kc[@]}" get pod broken-resize >/dev/null 2>&1; then
    "${kc[@]}" patch pod broken-resize --subresource resize --patch \
      '{"spec":{"containers":[{"name":"app","resources":{"requests":{"memory":"64Mi"},"limits":{"memory":"64Mi"}}}]}}'
  fi

  # Before the apply, and the comment on the function is why: the apply is what
  # destroys the evidence this reads.
  scan_second_revisions "${SECOND_REVISION[@]}"

  "${kc[@]}" apply -f "$BROKEN"
  # The healthy side goes up with the broken one: a rule needs both fixtures,
  # and capturing them from the same cluster at the same time is what makes
  # the negative test comparable to the positive one.
  "${kc[@]}" apply -f "$HEALTHY"

  # --- THE STATES A MANIFEST CANNOT DECLARE ---
  # Three fixtures are a *change* to a running object rather than an object, so
  # they are applied here and not in the yaml.
  for w in "${SECOND_REVISION[@]}"; do second_revision "$w"; done

  # The in-place resize the node cannot fit (D51). It goes through the `resize`
  # subresource, which is the only path that changes the resources of a running
  # pod, and it needs the pod to be running first — an unscheduled pod has no
  # enacted resources for the spec to disagree with. The kubelet then parks it
  # as PodResizePending instead of enacting it, which is the whole point: `spec`
  # and `status.resources` genuinely disagree and rule 2 has to name the second
  # one.
  #
  # --- WHY THE NUMBER IS READ OFF THE NODE AND NOT WRITTEN HERE ---
  # A constant cannot work, in either direction. Anything *above* the node's
  # allocatable is refused at admission and never reaches the kubelet at all —
  # measured on v1.36.1, which is what took `break` down at its last command:
  #
  #   Error from server (Forbidden): pods "broken-resize" is forbidden: node
  #   didn't have enough allocatable resources: memory, requested:
  #   1125899906842624, allocatable: 24860065792
  #
  # so `1Pi` produces no fixture and no pod, and `100Gi` produces the same
  # refusal on any machine smaller than 100Gi. Anything below the node's *free*
  # memory is simply enacted, and then nothing disagrees. What is left is the
  # window `(available, allocatable]`: large enough that the kubelet cannot fit
  # it, small enough that the apiserver admits it. Its top edge is the one point
  # in that window which does not depend on how much the other pods on that node
  # happen to be holding — and something is always holding some, because kindnet
  # is a DaemonSet and requests 50Mi on every worker there is.
  #
  # The request moves with the limit, because feasibility is judged on the
  # request and a resize that raised only the limit would be enacted happily;
  # the two stay equal to each other, because a resize that changes the pod's
  # QoS class is rejected outright.
  "${kc[@]}" wait --for=condition=Ready pod/broken-resize --timeout=300s
  # Ready implies scheduled, so `.spec.nodeName` is set by the time this reads
  # it. Asserted rather than assumed: an empty node name would send an empty
  # quantity to the apiserver, and the error would name the patch rather than
  # the missing field behind it.
  local node mem
  node=$("${kc[@]}" get pod broken-resize -o jsonpath='{.spec.nodeName}')
  [ -n "$node" ] || { echo "broken-resize is Ready but has no .spec.nodeName — there is no node to read an allocatable off" >&2; exit 1; }
  mem=$("${kc[@]}" get node "$node" -o jsonpath='{.status.allocatable.memory}')
  [ -n "$mem" ] || { echo "node $node reports no allocatable memory — the resize target cannot be computed" >&2; exit 1; }
  echo "  resizing broken-resize to the whole allocatable memory of $node ($mem)"
  "${kc[@]}" patch pod broken-resize --subresource resize --patch \
    "{\"spec\":{\"containers\":[{\"name\":\"app\",\"resources\":{\"requests\":{\"memory\":\"$mem\"},\"limits\":{\"memory\":\"$mem\"}}}]}}"

  echo
  echo "States need a few minutes to settle — CrashLoopBackOff has to enter"
  echo "backoff and the OOM kill has to actually happen. Check with: $0 status"
  echo
  echo "Two of them need about 26 minutes, not a few: broken-restarts10 and"
  echo "broken-restarts10serving have to climb past ten restarts, and the kubelet's"
  echo "backoff caps at 5 minutes a restart. '$0 verify' waits for them in a second"
  echo "pass after every other fixture has reported, so run it now rather than later —"
  echo "the wait is the same clock either way."
  echo
  echo "Rule 12 (stuck Terminating) is not applied by this script; it is a"
  echo "capture step: kubectl delete pod broken-stuck --wait=false"
}

unbreak() {
  need kubectl; need jq
  local kc=(kubectl --context "kind-$CLUSTER")
  # broken-stuck carries a finalizer nothing ever removes — that is the point
  # of the fixture, and it is why a plain delete would hang here forever.
  "${kc[@]}" patch pod broken-stuck -p '{"metadata":{"finalizers":null}}' 2>/dev/null || true
  # Every kind that owns a pod, not only Pod: deleting the pod of an owned
  # fixture deletes nothing, because the controller above it puts one straight
  # back. The Service is the StatefulSet's required headless one.
  local kinds=pod,deployment,statefulset,daemonset,service
  "${kc[@]}" delete "$kinds" -l demo=broken --wait=false --ignore-not-found
  "${kc[@]}" delete "$kinds" -l demo=healthy --wait=false --ignore-not-found
  # The W1 fixture lives in its own namespace (a pods: "0" quota would
  # otherwise block every pod above from being recreated).
  "${kc[@]}" delete namespace k8rs-quota --wait=false --ignore-not-found

  # And everything `break-nodes` did to the machines, or `cluster-down` is the
  # only way back to a cluster that can schedule anything. Undone for every
  # worker rather than for the three it picked: this has to work when the caller
  # is a person who ran `break-nodes` once, hours ago, and does not remember
  # which node got which. Each may legitimately find nothing to undo, which is
  # why none of them is allowed to end the script.
  #
  # **What it undid is printed, and the docker step is not swallowed.** With
  # `2>/dev/null || true` on all three, a denied docker socket and a node that
  # was never touched printed the same nothing — so the one failure that leaves
  # a kubelet stopped looked exactly like success. The state is read first and
  # the undo is run only where there is something to undo, which is what lets
  # the output say which it was.
  local w state undone=0
  for w in $(workers 2>/dev/null); do
    state=$("${kc[@]}" get node "$w" -o json 2>/dev/null) || {
      echo "  $w is not answering — the API server cannot see it, so nothing here can be undone through it" >&2
      continue
    }
    if jq -e '.spec.unschedulable == true' >/dev/null <<<"$state"; then
      "${kc[@]}" uncordon "$w"; undone=$((undone + 1))
    fi
    if jq -e '[.spec.taints[]? | select(.key == "dedicated")] | length > 0' >/dev/null <<<"$state"; then
      "${kc[@]}" taint node "$w" dedicated-; undone=$((undone + 1))
    fi
    # The one thing a *manifest* leaves behind on a machine, and the only undo
    # here that is not `break-nodes`': `broken-socket` mounts
    # /var/run/docker.sock with `type: FileOrCreate`, so the kubelet creates an
    # empty file on whichever worker that pod landed on, and deleting the pod
    # does not take it away. Read first and removed only where there is
    # something to remove, like the undos around it, so the output can say which
    # it was. The read is quiet — a node this login cannot reach through docker
    # is skipped, and the file it leaves is empty and on a tmpfs — but the
    # removal itself is not swallowed, because a `rm` that fails on a node we
    # *can* reach is a different fact and has to be readable as one.
    if docker exec "$w" test -e /var/run/docker.sock 2>/dev/null; then
      if docker exec "$w" rm -f /var/run/docker.sock; then
        echo "  removed the empty docker.sock broken-socket had the kubelet create on $w"
        undone=$((undone + 1))
      else
        echo "  could not remove /var/run/docker.sock on $w — an empty file on a tmpfs, harmless," >&2
        echo "  but still there: docker exec $w rm -f /var/run/docker.sock" >&2
      fi
    fi
    # The kubelet is stopped inside the node's own container, so this is the one
    # undo that does not go through the API server — and it must not be skipped
    # when the API server cannot see the node, which is exactly the state a
    # stopped kubelet produces (hence the `continue` above logging rather than
    # skipping the machine silently).
    if docker exec "$w" systemctl is-active --quiet kubelet 2>/dev/null; then
      : # already running, and saying so every time is noise
    elif docker exec "$w" systemctl start kubelet; then
      echo "  restarted the kubelet on $w"; undone=$((undone + 1))
    else
      echo "  could not reach the kubelet on $w through docker — if break-nodes stopped it," >&2
      echo "  it is still stopped. Check docker access, then: docker exec $w systemctl start kubelet" >&2
    fi
  done
  echo "  undid $undone node change(s)"
}

# Every worker, in the order the three node states are handed out. Sorted, so
# `break-nodes` and `unbreak` and a second run of either all mean the same node
# by the same word.
workers() {
  kubectl --context "kind-$CLUSTER" get nodes \
    -l '!node-role.kubernetes.io/control-plane' \
    -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}' | sort
}

# --- PREDICATES START ---
# One jq predicate per fixture, and each decides whether that fixture is
# trustworthy: a fixture that never reached the state its rule is about is a
# test that cannot fail. Asserted on the cluster *before* anything is captured
# from it — a lie caught here costs minutes, the same lie caught after the
# fixture is committed costs a rule everyone trusts and nobody tested.
#
# **Every value is a literal, single-quoted string, and none of them may
# interpolate a shell variable.** scripts/verify-test.sh reads this table out of
# this file and evaluates it in a shell that has none of this file's variables,
# so a `$fragment` shared between two predicates would silently expand to
# nothing there — the predicates would then be *proven* in a weaker form than
# the one that runs against the cluster, which is the worst direction to be
# wrong in. Repetition is the price.
#
# Global rather than local to a function because two callers share them:
# `verify` runs the pod set, `break-nodes` runs the node set after it.
POD_STATES=(oom crashloop image config pending hostpath readiness restarts
            nolimits stuck init quota w2 owned resize podlimit sts rollout ds
            healthy_init healthy_sidecar healthy_hostpath healthy_podlevel
            exit0 sigterm socket succeeded failed startup notfound wedged
            unjudged oomserving healthy_retry healthy_unreadysidecar)
# The two fixtures that cost more than every other one put together, split out
# so that a failure in the fast set is reported in the usual few minutes instead
# of behind a half-hour wait. See `verify` for the arithmetic — there is no
# mechanism that raises a restart count faster than the kubelet's own backoff
# lets it rise.
SLOW_POD_STATES=(restarts10 restarts10serving)
NODE_STATES=(cordoned tainted notready)

declare -A want=(
  [oom]='.status.containerStatuses[0] | (.lastState.terminated // .state.terminated // {}) | .reason=="OOMKilled" and .exitCode==137'
  # The `or` is the same fix, and for the same measured reason, as [owned]
  # below: a crash loop is not one state, and this predicate used to name only
  # the half of it that was on screen when it was written.
  #
  # The message clause is D51's: `terminationMessagePolicy:
  # FallbackToLogsOnError` is what makes the kubelet copy the container's last
  # log line into the termination, and without it every capture carries a
  # `terminated` with no `message` at all. Read from whichever half of the loop
  # the fetch landed in, for the same reason the exit code is.
  [crashloop]='.status.containerStatuses[0] | .lastState.terminated.exitCode==1 and (.state.waiting.reason=="CrashLoopBackOff" or .state.terminated.exitCode==1) and ((.lastState.terminated.message // .state.terminated.message) != null)'
  [image]='.status.containerStatuses[0].state.waiting.reason | .=="ImagePullBackOff" or . =="ErrImagePull"'
  [config]='.status.containerStatuses[0].state.waiting.reason=="CreateContainerConfigError"'
  # Pending is half of it. The other half is *why*, which is what N6 reports and
  # what the old 500-cpu request could not say: a nodeSelector nothing matches,
  # and the toleration written beside it. Both are set at admission, so neither
  # clause can make this predicate wait for anything.
  [pending]='.status.phase=="Pending" and ([.status.conditions[]?|select(.type=="PodScheduled")|.reason]|first)=="Unschedulable" and ((.spec.nodeSelector // {})|length)>0 and ([.spec.tolerations[]?|select(.key=="dedicated" and .value=="gpu")]|length)>0'
  # Two mounts of one hostPath volume, told apart by the two fields D46 found
  # missing: the writable one narrowed by a `subPath` (which is what the volume
  # path alone does not say), and the read-only one beside it. The projected
  # service-account volume every pod carries is why this picks hostPath out of
  # the list by name rather than counting mounts.
  [hostpath]='.status.phase=="Running" and ([.spec.volumes[]?|select(.hostPath!=null)|.name] as $hp | [.spec.containers[].volumeMounts[]?|select(.name as $n|$hp|index($n))] | (length==2 and any(.subPath!=null) and any(.readOnly==true) and any(.readOnly!=true)))'
  [readiness]='.status.phase=="Running" and (.status.containerStatuses[0] | .ready==false and .state.running!=null)'
  [restarts]='.status.phase=="Running" and (.status.containerStatuses[0] | .ready==true and .restartCount>=3)'
  [nolimits]='.status.phase=="Running" and (.spec.containers[0].resources.limits==null)'
  [stuck]='.status.phase=="Running" and ((.metadata.finalizers//[])|length)>0'
  [init]='([.status.initContainerStatuses[]?|select(.state.waiting.reason=="CrashLoopBackOff" or .lastState.terminated.exitCode==1)]|length)>0'
  [quota]='[.items[].status.conditions[]?|select(.type=="ReplicaFailure" and .status=="True")]|length>0'
  [w2]='[.status.conditions[]?|select(.type=="Progressing" and .status=="False" and .reason=="ProgressDeadlineExceeded")]|length>0'
  # Spelled differently from [crashloop] on purpose, and the difference was
  # measured, not guessed: sampled 70 times on this cluster the container was
  # in `state.terminated` 39 times, in `waiting: CrashLoopBackOff` 29 and
  # `running` twice — and while the backoff was still short, in 1 sample of
  # 30. Demanding the waiting reason therefore fails a pod that is
  # crashlooping correctly, the too-tight half of what verify-test.sh exists
  # to catch. What holds across the whole loop: it has already restarted
  # after an exit 1, and it is not up right now (in backoff, or just died).
  # The ~2s window where it *is* up stays excluded: a capture taken then is a
  # Running pod, and certifying that as crashlooping is the lie this function
  # exists to prevent.
  [owned]='[.items[]?|select((.status.containerStatuses[0]|.lastState.terminated.exitCode==1 and (.state.waiting.reason=="CrashLoopBackOff" or .state.terminated.exitCode==1)) and ([.metadata.ownerReferences[]?|select(.controller==true and .kind=="ReplicaSet")]|length)>0)]|length>0'
  # D51's divergence, and it is asserted from both sides because either one
  # alone is satisfied by a pod nobody resized: the kubelet is still holding the
  # 64Mi it enacted, and the spec now says something else. The pending marker
  # moved between releases — a `.status.resize` string before 1.34, a condition
  # after it — and both spellings mean "asked for, not given", so both count.
  #
  # Both of the kubelet's reasons count, and the cluster is why. `Infeasible` is
  # "not ever on this node" and it is the state this predicate used to demand —
  # but a request the node can never fit is refused at *admission*, so it never
  # reaches the kubelet and no such condition is ever written on this path (see
  # break_it, which measured the refusal). What `break` can produce is a request
  # the node admits and cannot currently free: `Deferred`, "not right now". Both
  # mean the same thing about the object in front of us — the spec asked, the
  # kubelet did not give — and that is the whole of what this fixture needs.
  # `Infeasible` stays because a node whose allocatable shrinks under a pod that
  # was already resized still produces it, and because a predicate that names
  # only the reachable half is one nobody can read a year from now.
  #
  # What the reason clause is actually holding out is the resize that is *in
  # flight*: `PodResizeInProgress` (and, on servers before 1.34, the `.status.resize`
  # string carrying "InProgress" or "Proposed") is a divergence that resolves
  # itself in the next second, and certifying that as this fixture is the lie
  # the reason clause exists to stop. A condition that arrived without a reason
  # cannot say which of the two it is, so it does not count either.
  [resize]='(.status.containerStatuses[0].resources.limits.memory=="64Mi") and (.spec.containers[0].resources.limits.memory | . != null and . != "64Mi") and ((.status.resize=="Deferred" or .status.resize=="Infeasible") or ([.status.conditions[]?|select(.type=="PodResizePending" and .status=="True" and (.reason=="Deferred" or .reason=="Infeasible"))]|length)>0)'
  # D53: the key the kubelet enacted that the spec never declared. The pod
  # declares a memory limit, the container declares only a cpu one, and the
  # kubelet writes the pod-level memory limit into the container's status
  # anyway — so `status.resources.limits` holds a key `spec` does not, which is
  # the one shape that separates a per-key fallback from a per-side one.
  [podlimit]='.status.phase=="Running" and (.spec.resources.limits.memory != null) and (((.spec.containers[0].resources.limits // {})|has("memory"))|not) and ((.status.containerStatuses[0].resources.limits // {})|has("memory"))'
  # The three workload kinds, each in the state that separates `desired` from
  # `ready` from the counters next to them. Exact numbers rather than a
  # relation: the manifests fix them, and "ready is less than desired" is also
  # true of a workload that is merely slow, which is not the fixture.
  [sts]='.spec.replicas==2 and .status.readyReplicas==1 and .status.updatedReplicas==1'
  [rollout]='.spec.replicas==2 and .status.replicas==3 and .status.readyReplicas==2 and .status.updatedReplicas==1 and .status.unavailableReplicas==1'
  [ds]='.status.desiredNumberScheduled>0 and .status.numberReady==0 and .status.currentNumberScheduled==.status.desiredNumberScheduled'
  # The healthy side, which had no predicate at all until now — every fixture
  # here is some rule's negative, and a "healthy" pod that is not running and
  # ready makes the false-positive test it backs prove nothing. Each carries one
  # more clause for the field it is the fixture for.
  [healthy_init]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and (.spec.initContainers[0].resources.limits.memory != null)'
  [healthy_sidecar]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.spec.initContainers[]?|select(.restartPolicy=="Always")]|length)>0'
  [healthy_hostpath]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.spec.volumes[]?|select(.hostPath!=null)|.name] as $hp | [.spec.containers[].volumeMounts[]?|select(.name as $n|$hp|index($n))] | (length>0 and all(.readOnly==true)))'
  [healthy_podlevel]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and (.spec.resources.requests.cpu != null) and (.spec.containers[0].resources.requests.cpu != null)'
  # The three node states, read from the List `kubectl get nodes` returns —
  # which is also the shape `nodes.json` is captured in, so the predicate and
  # the fixture are looking at the same document.
  #
  # Ready is asserted `True` on the cordoned one deliberately: N2 is "cordoned
  # and forgotten", and a node that is cordoned *because* it is dead is N1's
  # finding wearing N2's field. The three states are on three nodes so that each
  # fixture has one cause.
  #
  # The taint clause is not decoration. `kubectl cordon` writes
  # `spec.unschedulable` and nothing else; it is the node controller that then
  # adds `node.kubernetes.io/unschedulable:NoSchedule` beside it. A node with
  # the field and not the taint is a capture taken in the moment before the
  # controller answered — a shape no cluster is ever *read* in, and one no rule
  # should be tested against.
  [cordoned]='[.items[]|select(.spec.unschedulable==true and ([.status.conditions[]|select(.type=="Ready")|.status]|first)=="True" and ([.spec.taints[]?|select(.key=="node.kubernetes.io/unschedulable" and .effect=="NoSchedule")]|length)>0)]|length>0'
  # Key, value and effect — and deliberately **no `timeAdded`**. `kubectl taint`
  # writes none: the only writer in the tree is the node controller's
  # `SwapNodeControllerTaint`, which stamps the taints it adds *for itself*
  # (k/k #113044; PR #131644 deleted the "only written for NoExecute taints"
  # sentence from the API type as inaccurate). Demanding it here waited out the
  # full 420s on a taint that had been applied correctly, and then aborted the
  # capture at its last step. The timestamp is asserted where one is genuinely
  # written — see [notready], whose taint the controller does write.
  [tainted]='[.items[]|select([.spec.taints[]?|select(.key=="dedicated" and .value=="gpu" and .effect=="NoExecute")]|length>0)]|length>0'
  # `Unknown`, not `False`: a kubelet that stopped posting is what the node
  # controller writes Unknown for, and it is the single most common node
  # failure there is. `False` is a kubelet that is alive and saying no.
  #
  # And the taint that follows it, which is this capture's only `timeAdded`:
  # `doNoExecuteTaintingPass` adds `node.kubernetes.io/unreachable:NoExecute`
  # through `SwapNodeControllerTaint`, and that function stamps a time on every
  # taint it adds. Nobody types this one, which is exactly why a decode of the
  # field can be proven against it and not against `dedicated=gpu`.
  [notready]='[.items[]|select(([.status.conditions[]|select(.type=="Ready" and .status=="Unknown")]|length)>0 and ([.spec.taints[]?|select(.key=="node.kubernetes.io/unreachable" and .effect=="NoExecute" and .timeAdded!=null)]|length)>0)]|length>0'
  # --- THE BRANCHES NO COMMITTED FIXTURE COULD REACH ---
  # Every predicate below asserts `ready` or `started` as well as the field its
  # branch is about, and that is not padding: the rule these fixtures are for is
  # silenced by several clauses `or`-ed together, so a capture taken while the
  # container happened to be up would satisfy a *different* clause and the one
  # under test could be deleted without a red run (NOTES § D71).
  [exit0]='.status.containerStatuses[0] | .restartCount>=1 and .ready==false and .lastState.terminated.exitCode==0 and .lastState.terminated.reason=="Completed"'
  # 143 and not 137, which is the same kill after the grace period ran out on a
  # PID 1 that had no handler for it — the shape D71 says rule 6 must print the
  # memory sentence about, i.e. this fixture inverted.
  [sigterm]='.status.containerStatuses[0] | .restartCount>=1 and .ready==false and .lastState.terminated.exitCode==143'
  # Exactly one mount of the socket volume and it is read-only: rule 8 escalates
  # on the path and not on the mode, so a writable mount would satisfy the same
  # predicate through the branch this fixture is not for (NOTES § D78).
  [socket]='.status.phase=="Running" and ([.spec.volumes[]?|select(.hostPath.path=="/var/run/docker.sock")|.name] as $hp | [.spec.containers[].volumeMounts[]?|select(.name as $n|$hp|index($n))] | length==1 and all(.readOnly==true))'
  # Three restarts and not two: RESTARTS_WARN is 3, so a pod with two would be
  # below rule 5's own threshold and the skip would be proven for rule 6 alone.
  [succeeded]='.status.phase=="Succeeded" and (.status.containerStatuses[0] | .restartCount>=3 and .state.terminated.exitCode==0 and .lastState.terminated.exitCode==1)'
  [failed]='.status.phase=="Failed" and (.status.containerStatuses[0] | .restartCount>=3 and .lastState.terminated.exitCode==1)'
  # Rule 5's CRITICAL band, and the `&& !serving` half beside it: the same
  # history on the same image, told apart by whether the container is serving at
  # the end of it. `.state.running` as well as the count, because a container in
  # backoff is not what either card is about.
  [restarts10]='.status.phase=="Running" and (.status.containerStatuses[0] | .restartCount>=10 and .ready==false and .state.running!=null)'
  [restarts10serving]='.status.phase=="Running" and (.status.containerStatuses[0] | .restartCount>=10 and .ready==true and .state.running!=null)'
  # All three readings at once, which is the only way to tell rule 7's state
  # gate from its `started` suppressor: up, not ready, and not started.
  [startup]='.status.phase=="Running" and (.status.containerStatuses[0] | .started==false and .ready==false and .state.running!=null) and (.spec.containers[0].startupProbe != null)'
  # The message clause is the fixture: rule 6 has three actions and the log-line
  # one answers first whenever a message exists, so this arm needs a termination
  # with none.
  [notfound]='.status.containerStatuses[0] | .ready==false and .lastState.terminated.exitCode==127 and ((.lastState.terminated.message // null) == null)'
  # Scheduled and stuck before the sandbox: the condition reads False for a
  # volume failure because the kubelet mounts before it creates the sandbox
  # (NOTES § D76). `ContainerCreating` is what separates it from broken-config,
  # whose envFrom is resolved one step later and belongs to rule 4.
  [wedged]='.status.phase=="Pending" and ([.status.conditions[]?|select(.type=="PodScheduled" and .status=="True")]|length)>0 and ([.status.conditions[]?|select(.type=="PodReadyToStartContainers" and .status=="False")]|length)>0 and .status.containerStatuses[0].state.waiting.reason=="ContainerCreating"'
  # The absence *is* the signal, so the predicate counts a condition rather than
  # reading one: a pod refused a machine carries PodScheduled False, and this
  # one has no such line at all. creationTimestamp is asserted because it is the
  # only clock rule 14 has.
  [unjudged]='.status.phase=="Pending" and ([.status.conditions[]?|select(.type=="PodScheduled")]|length)==0 and .metadata.creationTimestamp != null'
  # Serving *and* carrying the kill, which is the pair no capture holds: oom.json
  # is crashlooping, so rule 2 stays quiet on it for the state rather than for
  # the age.
  [oomserving]='.status.phase=="Running" and (.status.containerStatuses[0] | .ready==true and .state.running!=null and .restartCount>=1 and .lastState.terminated.reason=="OOMKilled" and .lastState.terminated.exitCode==137)'
  # The wait-for-dependency loop, finished: the failed history is in lastState,
  # the successful exit is in state, and the pod is serving. Nothing may fire.
  [healthy_retry]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and (.status.initContainerStatuses[0] | .restartCount>=3 and .state.terminated.exitCode==0 and .lastState.terminated.exitCode==1)'
  # A sidecar that is running and not ready, with the workload container serving
  # beside it — started, because a restartable init container must start before
  # the next one runs, and only ready is missing.
  [healthy_unreadysidecar]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.spec.initContainers[]?|select(.restartPolicy=="Always")]|length)>0 and (.status.initContainerStatuses[0] | .ready==false and .started==true and .state.running!=null)'
)

declare -A why=(
  [oom]="rule 2 — killed for exceeding its memory limit"
  [crashloop]="rules 1+6 — CrashLoopBackOff after exit 1, with the log tail the kubelet kept"
  [image]="rule 3 — the image cannot be pulled"
  [config]="rule 4 — a ConfigMap the pod needs does not exist"
  [pending]="rule 10 + N6 — nothing can schedule it, and the selector says why"
  [hostpath]="rule 8 — two mounts of the node's filesystem, one writable and narrowed by a subPath"
  [readiness]="rule 7 — the container runs, but never passes its readiness probe"
  [restarts]="rule 5 — Running and ready now, but it has restarted repeatedly"
  [nolimits]="rule 9 — no limits (a Capacity row, not an alert)"
  [stuck]="rule 12 — a finalizer nothing removes (the delete is part of the capture)"
  [init]="D27 — the init container fails, so the app container never starts"
  [quota]="D28/W1 — quota denies every pod, no pod object exists"
  [w2]="D28/W2 — the rollout gave up (ProgressDeadlineExceeded)"
  [owned]="D36 — a crashlooping pod owned by a ReplicaSet (the grouping key's workload branch)"
  [resize]="D51 — the resize the node cannot fit: spec and status disagree about the limit"
  [podlimit]="D53 — the kubelet enacted a memory limit the container's spec never declared"
  [sts]="D40 — a StatefulSet at all, and a partially ready one (statefulsets.json was empty)"
  [rollout]="D40 — a Deployment mid-rollout: two ReplicaSets, five counters, five values"
  [ds]="D40 — a DaemonSet whose pods cannot start (desired is per node, not spec.replicas)"
  [healthy_init]="D40 — the negative side, and an init container that declares resources"
  [healthy_sidecar]="D46 — the native sidecar: restartPolicy Always on an init container"
  [healthy_hostpath]="D46 — the posture case: a host mount that is read-only"
  [healthy_podlevel]="D51 — a pod that declares its own request beside its containers'"
  [cordoned]="N2 — a worker cordoned, still healthy, still carrying pods a drain would move"
  [tainted]="N6 — dedicated=gpu:NoExecute, a taint with a value (kubectl stamps no time on it)"
  [notready]="N1 — a worker whose kubelet stopped posting, and the taint the controller timestamps"
  [exit0]="rule 6 — a program that finished, restarted forever: exit 0 is not a failure"
  [sigterm]="rule 6 — killed by SIGTERM (143), which is every rolling update and not a failure"
  [socket]="rule 8 — the runtime socket itself, read-only, under its /var/run name (D78)"
  [succeeded]="D71 — a pod that finished, carrying the restarts analyze() must skip"
  [failed]="D71 — the Failed half of that skip, which Evicted pods arrive in"
  [restarts10]="rule 5 — past ten restarts and not serving: the CRITICAL band"
  [restarts10serving]="rule 5 — past ten restarts and serving: WARN, because red must mean broken"
  [startup]="rule 7 — a startup probe still failing, so readiness has not been asked (D71)"
  [notfound]="rule 6 — exit 127 with no termination message: the command-not-in-the-image action"
  [wedged]="rule 13 — placed, and stuck before the sandbox on a ConfigMap that does not exist (D72/D76)"
  [unjudged]="rule 14 — no PodScheduled line at all: nothing has looked at this pod (D74)"
  [oomserving]="rule 2 — OOMKilled once and serving since, which is the recency clause (D75)"
  [healthy_retry]="D75 — the wait-for-dependency init loop that finished: rules 5 and 6 must be silent"
  [healthy_unreadysidecar]="D75 — a sidecar running but not ready: rule 7 is regular containers only"
)
# --- PREDICATES END ---

# Not every check is a pod: W1's whole point is that no pod object exists, W2
# lives on the Deployment, and the three node states are read from the node
# List. One fetch keeps every check on the same wait loop, the same timeout and
# the same output.
fetch() {
  local kc=(kubectl --context "kind-$CLUSTER")
  case "$1" in
    quota)   "${kc[@]}" get replicasets -n k8rs-quota -o json ;;
    w2)      "${kc[@]}" get deployment broken-quota -n k8rs-quota -o json ;;
    # A Deployment's pod has a generated name, so this one is fetched by
    # label and arrives as a List — the shape, not just the object, is the
    # difference from every line above.
    owned)   "${kc[@]}" get pods -l app=broken-owned -o json ;;
    sts)     "${kc[@]}" get statefulset broken-sts -o json ;;
    rollout) "${kc[@]}" get deployment broken-rollout -o json ;;
    ds)      "${kc[@]}" get daemonset broken-ds -o json ;;
    healthy_init) "${kc[@]}" get pod healthy -o json ;;
    healthy_*)    "${kc[@]}" get pod "healthy-${1#healthy_}" -o json ;;
    cordoned|tainted|notready) "${kc[@]}" get nodes -o json ;;
    *)       "${kc[@]}" get pod "broken-$1" -o json ;;
  esac 2>/dev/null
}

# What it actually is, in one line, for the FAIL case. Covers a single object
# and a List, and drops the keys that do not apply instead of printing nulls.
diagnose() {
  jq -c '{ name:       .metadata.name,
           phase:      .status.phase,
           state:      (.status.containerStatuses // [])[0].state,
           last:       (.status.containerStatuses // [])[0].lastState,
           restarts:   (.status.containerStatuses // [])[0].restartCount,
           init:       (.status.initContainerStatuses // [])[0].state,
           enacted:    (.status.containerStatuses // [])[0].resources,
           declared:   (.spec.containers // [])[0].resources,
           replicas:   ({ want: .spec.replicas } + (.status // {} | {replicas, readyReplicas, updatedReplicas, unavailableReplicas, desiredNumberScheduled, numberReady})
                        | with_entries(select(.value != null))),
           conditions: [ (.status.conditions // [])[] | {type,status,reason} ],
           items:      [ (.items // [])[] | { name: .metadata.name,
                           owner:      [ (.metadata.ownerReferences // [])[] | {kind,controller} ],
                           state:      (.status.containerStatuses // [])[0].state,
                           unschedulable: .spec.unschedulable,
                           taints:     [ (.spec.taints // [])[] | {key,value,effect,timeAdded}
                                         | with_entries(select(.value != null)) ],
                           conditions: [ (.status.conditions // [])[] | select(.type=="Ready") | {type,status,reason} ] }
                         # Dropped per item as well as at the top level: on a
                         # node List the pod keys are all null, and three lines
                         # of nulls is where the taint stops being visible.
                         | with_entries(select(.value != null and .value != [])) ] }
         | with_entries(select(.value != null and .value != [] and .value != {}))' 2>/dev/null
}

# The wait-and-report loop, shared by `verify` and `break-nodes` so that a node
# state is held to exactly the discipline a pod state is: polled until it
# arrives or the timeout is spent, then printed one line per fixture with what
# the object actually was underneath any failure.
assert_states() {
  need kubectl; need jq
  # $SETTLE is `verify`'s second pass asking for a longer deadline than the
  # fixtures that settle in seconds get. It is a caller's `local`, so it cannot
  # outlive the call and cannot reach `break-nodes`.
  local deadline=$(( SECONDS + ${SETTLE:-${K8RS_VERIFY_TIMEOUT:-420}} ))
  local names=("$@") pending_list=("$@") still fail=0 got n

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
      printf '  PASS  %-17s %s\n' "$n" "${why[$n]}"
    else
      printf '  FAIL  %-17s %s\n' "$n" "${why[$n]}"
      got=$(fetch "$n" | diagnose) || got=
      printf '        got: %s\n' "${got:-object not found}"
      fail=1
    fi
  done

  [ $fail -eq 0 ] && echo "  all ${#names[@]} fixtures reached the state their rule is about"
  return $fail
}

# Two passes, fast first, and the split is arithmetic rather than taste.
#
# Every fixture but two reaches its state in seconds to a couple of minutes. The
# two restart-count pods have to climb past ten restarts, and a restart count is
# only raised by the kubelet restarting a container — which it does on a backoff
# that doubles from 10s and caps at 5 minutes. (Not read off a doc: `init.json`
# carries the kubelet's own sentence, "back-off 5m0s restarting failed
# container", and `restarts.json` shows three restarts taking 50 seconds.) Ten
# restarts is therefore 10+20+40+80+160+300+300+300+300 = 1510 seconds of
# backoff plus about four seconds per container start: **just over 26 minutes**,
# and nothing shortens it. Running the container for longer between exits only
# makes it worse — the backoff resets after ten idle minutes, so a slow loop
# costs more, not less.
#
# So the slow pair gets its own deadline (35 minutes, which is the 26 plus room
# for a cold image pull and a busy machine), and it runs *second* so that a
# fixture that is genuinely wrong is on screen in the usual few minutes instead
# of behind a half-hour wait. `set -e` ends the run at the first pass's failure,
# which is the point of the ordering.
verify() {
  assert_states "${POD_STATES[@]}"
  echo
  echo "  the two restart-count fixtures need ten CrashLoopBackOff restarts, which is about"
  echo "  26 minutes of the kubelet's own backoff from the moment 'break' created them. Waiting"
  echo "  (this pass exits as soon as they arrive; K8RS_SLOW_TIMEOUT is the ceiling)."
  local SETTLE="${K8RS_SLOW_TIMEOUT:-2100}"
  assert_states "${SLOW_POD_STATES[@]}"
}

# The three node states, and the only step here that damages what is already
# running: one worker stops taking new pods, a second evicts everything that
# does not tolerate a NoExecute taint, and a third loses its kubelet — after
# which every pod on it reads Unknown and, minutes later, is marked for
# deletion. So this runs *after* the pod fixtures are captured and immediately
# before the node capture, and `unbreak` puts all three back.
break_nodes() {
  need kubectl; need docker; need jq
  local kc=(kubectl --context "kind-$CLUSTER") w n
  mapfile -t w < <(workers)
  [ ${#w[@]} -ge 3 ] || {
    echo "break-nodes: this cluster has ${#w[@]} worker(s) and the three states need three." >&2
    echo "             Doubling two of them onto one node is what makes each fixture" >&2
    echo "             ambiguous, so refusing rather than improvising: recreate with" >&2
    echo "             K8RS_WORKERS=3 (the default) — scripts/cluster.sh reset." >&2
    return 1
  }

  # --- WHICH NODE GETS WHICH STATE ---
  # N2 is not "a cordoned node". It is a cordoned node **with pods a drain would
  # still move** (NOTES § N-series; D46 spends a paragraph on the false positive
  # that made it so), and a worker carrying only DaemonSet and mirror pods is
  # N2's *negative* wearing N2's name. Cordoning whichever worker sorts first
  # decides which of the two the committed fixture is by luck, so the node is
  # chosen by what is on it and the choice is asserted rather than assumed.
  #
  # `demo`-labelled, not merely movable: the pods a rule test ever sees are the
  # ones `just fixtures` captured minutes earlier, and the snapshot it joins is
  # {those captures} ∪ {nodes.json}. A movable pod no capture holds is not in
  # that snapshot and cannot make N2 fire in it. The other two exclusions are
  # N2's own words — Succeeded/Failed pods and DaemonSet- or Node-owned ones are
  # not pods a drain moves.
  #
  # The pick is stable across runs, which matters because `just fixtures` is
  # re-run after a trip that failed at a guard, and a second run that cordoned a
  # *different* worker would leave the first one cordoned too — one node wearing
  # two states, the exact thing three workers exist to prevent. It holds because
  # this set only ever shrinks: a cordon evicts nothing, so the chosen node keeps
  # its pods, while the tainted and the kubelet-less node lose theirs; and no
  # worker can gain a demo pod in between, because after a first run all three
  # are unschedulable (cordoned, NoExecute-tainted, NotReady). The first worker
  # in sorted order that had one therefore still is.
  local movable cordon=""
  movable=" $("${kc[@]}" get pods -A -o json | jq -r '
      [ .items[]
        | select(.metadata.labels.demo != null)
        | select(.status.phase != "Succeeded" and .status.phase != "Failed")
        | select(([.metadata.ownerReferences[]?
                   | select(.controller == true and (.kind == "DaemonSet" or .kind == "Node"))]
                  | length) == 0)
        | .spec.nodeName // empty ] | unique | .[]' | tr '\n' ' ')"
  for n in "${w[@]}"; do
    case "$movable" in *" $n "*) cordon="$n"; break ;; esac
  done
  [ -n "$cordon" ] || {
    echo "break-nodes: no worker is running a captured pod a drain would move, so cordoning" >&2
    echo "             one here would produce N2's *negative* under N2's name." >&2
    echo "             The pods come first: $0 break, then let them schedule." >&2
    return 1
  }
  local rest=(); for n in "${w[@]}"; do [ "$n" = "$cordon" ] || rest+=("$n"); done

  # docker is the third step and the only one that can fail on permissions:
  # `need docker` proves the binary is on PATH and says nothing about the
  # socket, which is per-login on the machine this runs on. A denial discovered
  # after the cordon and the taint leaves the cluster damaged, the capture
  # unwritten and nothing undone — so it is discovered here, with the same call
  # that has to work below.
  docker exec "${rest[1]}" true 2>/dev/null || {
    echo "break-nodes: docker cannot reach the node container ${rest[1]}, and stopping a" >&2
    echo "             kubelet is the one step that does not go through the API server." >&2
    echo "             Refusing before anything is cordoned or tainted: fix docker access" >&2
    echo "             (the docker group is per-login here) and run this again." >&2
    return 1
  }

  "${kc[@]}" cordon "$cordon"
  # NoExecute, matching the toleration broken-pending carries: N6 reads the pair,
  # and a taint nothing tolerates beside a toleration that matches no taint makes
  # the joined snapshot say nothing about either. It evicts what does not
  # tolerate it, which is why this whole function runs after the pod captures.
  #
  # It carries no `timeAdded` and none is asserted of it: `kubectl taint` writes
  # none (k/k #113044), and the only writer in the tree is the node controller's
  # `SwapNodeControllerTaint`, for the taints it adds itself — the one that
  # answers the cordon above, and the two that answer the stopped kubelet below.
  # Those are where this capture's timestamp comes from.
  "${kc[@]}" taint node "${rest[0]}" dedicated=gpu:NoExecute --overwrite
  # Inside the node's own container, because a kubelet is not an API object.
  # kind names the container after the node, which is why `workers` can be read
  # from the API and handed to docker unchanged.
  docker exec "${rest[1]}" systemctl stop kubelet
  echo "  cordoned $cordon · tainted ${rest[0]} · stopped the kubelet on ${rest[1]}"
  echo "  (the node controller takes about a minute to notice the third, and it is the"
  echo "   controller — never kubectl — that stamps a timeAdded on a taint)"

  assert_states "${NODE_STATES[@]}"
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
  up)          up ;;
  down)        down ;;
  break)       break_it ;;
  verify)      verify ;;
  break-nodes) break_nodes ;;
  unbreak)     unbreak ;;
  status)      status ;;
  reset)       down || true; up; break_it ;;
  # The usage text is this file's header, so there is one copy of it and not
  # two — read as "every comment line after the shebang, up to the first line
  # that is not one". It was a fixed `2,26` range with a comment claiming
  # verify-test.sh asserted the range still ended on the last comment line.
  # Nothing asserted that anywhere, and an invented guard is worse than none:
  # the next editor trusts it and grows the header. Asking awk where the
  # comments stop needs no guard and no number to keep correct.
  *)           awk 'NR > 1 { if (!/^#/) exit; sub(/^# ?/, ""); print }' \
                 "${BASH_SOURCE[0]}"; exit 1 ;;
esac
