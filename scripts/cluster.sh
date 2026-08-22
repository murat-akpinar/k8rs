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
#   ./scripts/cluster.sh break-runtime reboot a node out from under a container
#                                    that never crashed, and assert it
#   ./scripts/cluster.sh break-nodes cordon / taint / stop a kubelet, place the pod
#                                    nothing will ever start on that last node, and
#                                    assert all four
#   ./scripts/cluster.sh status      nodes, demo pods, memory
#   ./scripts/cluster.sh unbreak     remove them (clears the stuck finalizer, forces
#                                    the one pod no kubelet is left to confirm,
#                                    uncordons, untaints, starts a node whose
#                                    container is stopped, restarts the kubelet)
#   ./scripts/cluster.sh reset       down + up + break
#   ./scripts/cluster.sh down        delete the cluster
#
# `break-runtime` and `break-nodes` are deliberately not part of `break`, and
# both run *after* the pod fixtures have been captured. `break-runtime` reboots
# the node one pod is on; `break-nodes` makes one node unschedulable, evicts
# whatever does not tolerate a NoExecute taint from a second, kills the kubelet on
# a third and binds `broken-unstarted` to that third one — the only pod capture
# this script places itself, and the only one that cannot be taken before this
# step (NOTES § D156). Any of those changes a pod fixture that is still being
# settled — a reboot alone raises `restartCount` on every pod on that worker — so
# `verify` runs before them, the two run in that order, and the node capture
# comes last (see the `fixtures` recipe in the justfile, which is the only caller
# that gets the order right).
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

# The early, loud half of the refusal scripts/sanitize.jq anchors. kind names a
# node `<cluster>-<role>`, and the sanitizer accepts exactly the names *this*
# cluster produces — so a cluster called `k8rs-review` builds
# `k8rs-review-control-plane`, whose capture is refused hours later, at the end
# of a trip, by a filter the person has no reason to be thinking about.
#
# Refused: a name wearing the fixture cluster's own as a prefix. `review` — the
# name D92 gives an ephemeral measurement cluster — is not one and must keep
# working, which is the whole reason this is a prefix check and not an allowlist
# of one.
#
# It is not the primary refusal and must not be read as one: a reviewer raising a
# cluster with `kind create cluster` never runs this file. The sanitizer is what
# makes D92 mechanical; this is what makes it early (todo.md, Phase 4).
#
# `up` only, so a cluster somebody already built under a refused name can still
# be torn down with `down` — a guard that traps a running cluster on the host is
# one people work around by deleting the guard.
refuse_family_name() { # $1 = cluster name
  case "$1" in
    k8rs) return 0 ;;
    k8rs*)
      echo "cluster.sh: refusing to build a cluster named '$1'." >&2
      echo "  kind would name its nodes '$1-control-plane' and '$1-worker…', which" >&2
      echo "  wear the fixture cluster's name without being it — scripts/sanitize.jq" >&2
      echo "  refuses exactly those, so any capture from here dies at the end of the" >&2
      echo "  trip instead of now (NOTES § D92, § D94)." >&2
      echo "  The fixture cluster is 'k8rs'. An ephemeral review cluster is" >&2
      echo "  K8RS_CLUSTER=review — no prefix." >&2
      return 1 ;;
  esac
}

up() {
  # Before `need`, so the name is refused on a machine that has not installed
  # kind yet: the wrong answer to "which cluster am I building" does not become
  # more or less wrong when a binary is missing.
  refuse_family_name "$CLUSTER"
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

  # **The one field in broken.yaml that names a node, made relative to this
  # cluster.** `broken-overhead` carries `nodeName: k8rs-worker` (its manifest says
  # why), and kind names nodes after the cluster — so on the ephemeral review
  # cluster, `K8RS_CLUSTER=review`, that node does not exist and the pod sits
  # Pending forever while `[overhead]` waits for `Running`. A trip on the fixture
  # cluster substitutes `k8rs` for `k8rs` and nothing changes, so the file stays
  # exactly what `kubectl apply -f scripts/broken.yaml` does by hand; only a
  # differently-named cluster sees a difference, which is the only place there is
  # one. Anchored to the `nodeName:` key so it cannot touch an object *name* that
  # happens to start `k8rs-`, and `$CLUSTER` is safe in the replacement because
  # `refuse_unusable_name` has already refused every character that is not.
  #
  # **The node name is spelled in three places** and they agree by construction
  # only on the fixture cluster: `scripts/broken.yaml`'s `nodeName: k8rs-worker`,
  # the `k8rs-` prefix in this expression, and the `.spec.nodeName == "k8rs-worker"`
  # clause in `justfile` § fixtures. Change one and the other two do not follow.
  sed "s/^\( *nodeName: \)k8rs-/\1$CLUSTER-/" "$BROKEN" | "${kc[@]}" apply -f -
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
  # The other pod a plain delete does not remove, and for the opposite reason:
  # `broken-unstarted` is bound to the worker `break-nodes` stopped the kubelet
  # on, and the kubelet that would confirm a delete is that one. Measured, a plain
  # `kubectl delete pod` printed `deleted`, blocked to a 40 s timeout, and left the
  # pod `Terminating` indefinitely
  # (reports/2026-08-22-rule-13-the-pod-with-no-container-status.md § Undoing it).
  # The kubelet restart at the foot of this function *does* reap it — but that is
  # the one step here that is allowed to fail (docker access is per-login on this
  # machine), and a failure there would leave the pod outliving the promise this
  # subcommand makes. So it comes down through the API server alone, which is what
  # PodGC would do to it eventually anyway. Named on its own rather than folded
  # into the label-selected delete below: `--force` there would change how forty
  # other fixtures come down, including the finalizer this line above unpicks.
  "${kc[@]}" delete pod broken-unstarted --force --grace-period=0 --ignore-not-found
  # Every kind that owns a pod, not only Pod: deleting the pod of an owned
  # fixture deletes nothing, because the controller above it puts one straight
  # back. The Service is the StatefulSet's required headless one.
  #
  # **And the cluster-wide reports' inputs, which this list did not cover** — a
  # PDB, two claims, the static PV under one of them and a RuntimeClass all
  # survived `unbreak` and were residue its own promise did not mention
  # (NOTES § D129, § D130). Two of them are cluster-scoped (`persistentvolume`,
  # `runtimeclass`); kubectl resolves each type's scope for itself, so they ride
  # in the same label-selected delete as the rest.
  #
  # Order is not a concern here because `--wait=false` is: the claim's
  # `pvc-protection` finalizer holds until the pod that mounts it is gone, and
  # the volume's `pv-protection` until the claim is, and none of that blocks a
  # delete that does not wait. The static PV is `Retain`, so deleting its claim
  # would leave it `Released` rather than gone — which is exactly why it is named
  # here rather than left to a reclaim policy that never runs.
  local kinds=pod,deployment,statefulset,daemonset,service,poddisruptionbudget,persistentvolumeclaim,persistentvolume,runtimeclass
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
    # The one residue `break-runtime` can leave, and it is checked before
    # anything that goes through the API server: `docker restart` is a stop and a
    # start, so a run that died between the two leaves a node container that is
    # simply not running. Every `docker exec` below would then fail with the
    # message about a denied socket, which is the wrong fact — and the kubelet
    # step in particular would report a stopped kubelet on a machine that is not
    # switched on. Read first and started only where there is something to start,
    # like the undos beneath it.
    if [ "$(docker inspect -f '{{.State.Running}}' "$w" 2>/dev/null)" = "false" ]; then
      if docker start "$w" >/dev/null; then
        echo "  started $w — its container was stopped, which is a break-runtime reboot that did not finish"
        undone=$((undone + 1))
      else
        echo "  $w exists as a container and will not start; nothing below can reach it either:" >&2
        echo "  docker start $w" >&2
      fi
    fi
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
            unjudged oomserving neverback healthy_retry healthy_unreadysidecar
            probe0 neverrules gang
            overhead healthy_disk pdb_floor pdb_room pvc_orphan pvc_used)
# The two fixtures that cost more than every other one put together, split out
# so that a failure in the fast set is reported in the usual few minutes instead
# of behind a half-hour wait. See `verify` for the arithmetic — there is no
# mechanism that raises a restart count faster than the kubelet's own backoff
# lets it rise.
SLOW_POD_STATES=(restarts10 restarts10serving)
# The one state whose producer is the machine under the pod and not the pod: a
# node rebooted out from under a container that never crashed. `break-runtime`
# makes it and asserts it, the way `break-nodes` does the node set — and for the
# same reason it is a subcommand of its own: it is destructive to whatever else
# is on that worker, so it may not run before the pod captures are on disk.
# (An array of one, not a scalar: `assert_states` takes a list, and the reboot
# stopped being one of two only because D90's Init arm was measured unreachable
# — see the header of scripts/broken.yaml's last section.)
RUNTIME_STATES=(reboot)
NODE_STATES=(cordoned tainted notready)

declare -A want=(
  [oom]='.status.containerStatuses[0] | (.lastState.terminated // .state.terminated // {}) | .reason=="OOMKilled" and .exitCode==137'
  # The `or` is the same fix, and for the same measured reason, as [owned]
  # below: a crash loop is not one state, and this predicate used to name only
  # the half of it that was on screen when it was written.
  #
  # **The looseness is deliberate and it does not travel.** This predicate asks
  # about a *live* pod, where either face is the pod cycling correctly — the
  # 70-sample measurement is under [owned]. `just fixtures` § `fetch_until` asks a
  # different question of the same container: whether the bytes it is about to
  # commit can carry the tests written against them, and there `crashloop.json`
  # and `exit0.json` must land in `waiting: CrashLoopBackOff` because that is the
  # face rule 1's card is drawn from. That guard is tight, this one stays loose,
  # and unifying them breaks whichever end it is moved to — tightening here fails
  # a correct pod and burns the 420s timeout, which is the too-tight half
  # verify-test.sh exists to catch.
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
  # The same two clauses as [crashloop] and [owned], on the init list — an exit 1
  # already behind it *and* not up right now. It used to say "waiting
  # CrashLoopBackOff **or** exit 1 behind it", whose second half says nothing
  # about the current state: the ~2s window [owned]'s comment measures excluded
  # was let straight through, and so was `healthy-retry`, whose init container
  # carries exactly that history and finished.
  [init]='([.status.initContainerStatuses[]?|select(.lastState.terminated.exitCode==1 and (.state.waiting.reason=="CrashLoopBackOff" or .state.terminated.exitCode==1))]|length)>0'
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
  # D96's shape, and all three containers are asserted because all three are the
  # fixture: the one that stopped for good, the one that stopped correctly beside
  # it, and the one holding the pod out of a terminal phase. Read **by name**
  # and not by index: every other predicate here reads `containerStatuses[0]`
  # because the container it is about is the only one that matters on that pod,
  # and here all three matter and nothing promises which lands first.
  #
  # `restartPolicy` is in the predicate rather than left to the manifest because
  # it is the entire condition: the same three statuses under `Always` are a
  # container the kubelet is about to bring back, which is the false positive the
  # rule this fixture is for must not ship. `restartCount==0` is not the same
  # claim read twice — `ContainerRestartRules` is beta and on by default at
  # v1.36, so a *container* may override the pod upward, and the only field that
  # says it did is the count.
  #
  # `keeper` running is asserted beside `phase=="Running"` and not instead of it:
  # the phase is what a person reads, and the container is what makes it true. A
  # capture taken in the second after the last container stopped still says
  # `Running`, which is precisely the "capture taken too early" this table
  # exists to refuse.
  [neverback]='.spec.restartPolicy=="Never" and .status.phase=="Running" and ([.status.containerStatuses[]?|select(.name=="broke" and .restartCount==0 and .state.terminated.exitCode==1)]|length)==1 and ([.status.containerStatuses[]?|select(.name=="done" and .restartCount==0 and .state.terminated.exitCode==0)]|length)==1 and ([.status.containerStatuses[]?|select(.name=="keeper" and .state.running!=null)]|length)==1'
  # The wait-for-dependency loop, finished: the failed history is in lastState,
  # the successful exit is in state, and the pod is serving. Nothing may fire.
  [healthy_retry]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and (.status.initContainerStatuses[0] | .restartCount>=3 and .state.terminated.exitCode==0 and .lastState.terminated.exitCode==1)'
  # A sidecar that is running and not ready, with the workload container serving
  # beside it — started, because a restartable init container must start before
  # the next one runs, and only ready is missing.
  [healthy_unreadysidecar]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.spec.initContainers[]?|select(.restartPolicy=="Always")]|length)>0 and (.status.initContainerStatuses[0] | .ready==false and .started==true and .state.running!=null)'
  # --- THE THREE THIS TRIP DECLARES (D90, D97, D100) ---
  # **The duration is in the predicate because the duration is the object.**
  # `exit0.json` already carries a clean exit on a container that is not serving;
  # what it does not carry is a run on the far side of `PROBE_FLOOR` (20s), and
  # `finished_action` orders its doors by exactly that comparison (NOTES § D113).
  # A `broken-probe0` whose kill landed early is `exit0.json` again under a new
  # name. `> 25` and not `> 20`: the manifest's probe fires at 30s, so this
  # leaves five seconds of slack on both sides and fails loudly if that field is
  # ever tuned down into the arm this fixture is not for.
  #
  # The subtraction is written to be **total**, not merely correct on the object
  # it is about: `fromdateiso8601` is a hard jq error on null, and jq's own error
  # exit is indistinguishable to `assert_states` from a state that has not
  # arrived — so a missing stamp has to arrive here as `false` and not as a
  # 420-second wait ending in a message about the wrong thing. The epoch default
  # does that, and it makes a record with only one stamp negative rather than
  # enormous. `Completed` beside the `0`, because that is the reason the kubelet
  # writes for a clean exit and a fixture that lost it is a fixture whose
  # `exit_meaning` row is no longer being exercised.
  [probe0]='.status.containerStatuses[0] | .restartCount>=1 and .ready==false and .lastState.terminated.exitCode==0 and .lastState.terminated.reason=="Completed" and ((((.lastState.terminated.finishedAt // "1970-01-01T00:00:00Z")|fromdateiso8601) - ((.lastState.terminated.startedAt // "1970-01-01T00:00:00Z")|fromdateiso8601)) > 25)'
  # Rule 15 reads `state.terminated` — the run the container is in **now** — so
  # this asserts the *settled* run and not the loop: `retry` stopped at exit 1,
  # which no rule of its own matches, and the count behind it is the restart the
  # `exit 3` rule bought. Every one of rule 15's four conditions is named except
  # the one under test, which is the point of the object: pod `Never`, terminated,
  # a failing ending — and `restartCount == 1`, the guard that is all that stands
  # between this pod and a card saying nothing will start it again.
  #
  # The rule itself is read out of the spec rather than assumed from the count,
  # because the count alone is `broken-restarts` with a different policy: the
  # field this fixture exists to put on disk is `restartPolicyRules`, and a
  # capture that lost it would still satisfy every other clause here.
  # `exitCodes.values` is checked by membership rather than equality — `[3]` and
  # `[3,4]` are the same rule as far as this object is concerned.
  [neverrules]='.spec.restartPolicy=="Never" and .status.phase=="Running" and ([.spec.containers[]?|select(.name=="retry" and ([.restartPolicyRules[]?|select(.action=="Restart" and .exitCodes.operator=="In" and (.exitCodes.values|index(3)!=null))]|length)>0)]|length)==1 and ([.status.containerStatuses[]?|select(.name=="retry" and .restartCount==1 and .state.terminated.exitCode==1)]|length)==1 and ([.status.containerStatuses[]?|select(.name=="keeper" and .state.running!=null)]|length)==1'
  # D100's settled gang restart, and the two null stamps are asserted as
  # deliberately as the reason is: the record is **synthesized** by the kubelet
  # rather than observed, which is why rule 5's age has to come from
  # `state.running.startedAt` instead — a capture whose `lastState` carried real
  # stamps would be a different object and would retire nothing.
  #
  # Both containers are held to running, ready and past `RESTARTS_WARN`, and the
  # reason is demanded of only **one**: what the kubelet writes into the
  # triggering container's own record is measured one way (NOTES § D93) and is
  # not what this fixture is for — the sibling that was restarted *for* somebody
  # else's exit is. Asserting it of both would fail a correct capture; asserting
  # it of none would pass a capture of any restarting pod at all.
  [gang]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.status.containerStatuses[]?|select(.state.running.startedAt!=null and .restartCount>=3)]|length)==2 and ([.status.containerStatuses[]?|select(.lastState.terminated.exitCode==137 and .lastState.terminated.reason=="RestartingAllContainers" and .lastState.terminated.startedAt==null and .lastState.terminated.finishedAt==null)]|length)>0 and ([.spec.containers[]?|select([.restartPolicyRules[]?|select(.action=="RestartAllContainers")]|length>0)]|length)==1'
  # --- THE ONE break-runtime MAKES ---
  # Rule 5's producer without rule 1's: up, serving, and restarted anyway.
  #
  # **`255` / `Unknown` and not `137`, and it is measured rather than argued.**
  # `docker restart` on the node container was run twice — an 11-second outage
  # and a three-minute one — and both wrote
  # `exitCode: 255, reason: Unknown` from containerd's own restart path
  # (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 2,
  # `internal/cri/server/restart.go`). `137` with `ContainerStatusUnknown` is the
  # **other** producer, `crictl rmp -f` on the sandbox, which is what `rules.rs`
  # says beside `STATUS_LOST` in as many words. Naming both here would pass a
  # capture from either and leave the fixture unable to say which ending it is
  # for: this one is `Ending::CodeUnknown`.
  [reboot]='.status.phase=="Running" and (.status.containerStatuses[0] | .ready==true and .state.running!=null and .restartCount>=3 and .lastState.terminated.exitCode==255 and .lastState.terminated.reason=="Unknown")'
  # --- THE CLUSTER-WIDE REPORTS' INPUTS (D129, D130) ---
  # Six states that are not pod rules: the analysis reports join over kinds no
  # pod carries, and three of those joins had no object at all. They are here
  # rather than only behind the justfile's `guard` lines because a claim that
  # never bound is caught at minute two by this table and after the 26-minute
  # restart pass by that one — and the trip has an hour from `break`.
  #
  # **`spec.overhead` is asserted with the value the apiserver writes, not the
  # one the manifest asks for**, because the manifest may not ask: the
  # RuntimeClass admission controller autopopulates the field and rejects a
  # create request that already carries it. So a match here is proof the plugin
  # ran, and `runtimeClassName` beside it is what says which class it read.
  [overhead]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and .spec.runtimeClassName=="broken-overhead" and .spec.overhead.cpu=="250m" and .spec.overhead.memory=="120Mi"'
  [healthy_disk]='.status.phase=="Running" and ([.status.containerStatuses[].ready]|all) and ([.spec.volumes[]?|select(.persistentVolumeClaim.claimName=="healthy-disk")]|length)==1'
  # Exact numbers rather than a relation, for [sts]'s reason: the manifest fixes
  # them, and `disruptionsAllowed==0` alone is also true of a budget blocked
  # because its workload is unhealthy — which is a different row. Two healthy of
  # two expected against a minAvailable of 2 is the floor itself.
  [pdb_floor]='.spec.minAvailable==2 and .status.expectedPods==2 and .status.currentHealthy==2 and .status.desiredHealthy==2 and .status.disruptionsAllowed==0'
  [pdb_room]='.status.disruptionsAllowed>=1 and .status.currentHealthy>.status.desiredHealthy'
  # `Bound`, never `Pending` — a claim that reserved nothing is a different row,
  # and `WaitForFirstConsumer` is exactly what would leave this one Pending
  # forever if the static PV under it ever stopped being static. The capacity
  # clause is the second half: a bound claim reports the **PV's** size, so 128Mi
  # against a 64Mi request is what separates a decode of `status.capacity` from
  # one of `spec.resources.requests`.
  [pvc_orphan]='.status.phase=="Bound" and .status.capacity.storage=="128Mi" and .spec.resources.requests.storage=="64Mi"'
  # `// ""` and not a bare `!= ""`: in jq `null != ""` is **true**, so a claim
  # whose storageClassName is absent entirely — which is what a cluster with no
  # default class writes — would have read as dynamically provisioned. The
  # phase clause hides that today; a predicate that is right for a reason its
  # neighbour supplies is one the neighbour can stop supplying.
  [pvc_used]='.status.phase=="Bound" and (.spec.storageClassName // "")!=""'
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
  [neverback]="D96 — stopped for good under restartPolicy Never, in a pod still Running (with its own clean-exit negative beside it)"
  [healthy_retry]="D75 — the wait-for-dependency init loop that finished: rules 5 and 6 must be silent"
  [healthy_unreadysidecar]="D75 — a sidecar running but not ready: rule 7 is regular containers only"
  [probe0]="D90/D113 — a probe kill the program reported as exit 0, on a run past the 20s probe floor"
  [neverrules]="D97 — restarted under restartPolicy Never by a rule on its own exit code: rule 15's false positive"
  [gang]="D100 — a settled gang restart: 137/RestartingAllContainers with no stamps, beside a live startedAt"
  [reboot]="D90 — a restart count raised by a node reboot: rule 5's producer, on a container that never crashed"
  [overhead]="D46/D130 — spec.overhead, written by the RuntimeClass admission controller: the charge the scheduler counts and a spec-only sum does not"
  [healthy_disk]="D129 — a pod that mounts a claim: the half of Waste's orphan-disk row that lives on a pod"
  [pdb_floor]="D46/D129 — a PDB at its floor, so a drain of its node never finishes (Drain safety's whole reason for existing)"
  [pdb_room]="D129 — a PDB with slack, which is what lets the one above fail"
  [pvc_orphan]="D129 — a claim that is Bound and mounted by nothing (Waste); Bound matters, a Pending one is a different row"
  [pvc_used]="D129 — a Bound claim a pod does mount, so the join has both sides"
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
    # The cluster-wide report inputs. `healthy_disk` needs no case of its own —
    # it is a pod and the `healthy_*` line below already fetches it.
    pdb_floor)  "${kc[@]}" get poddisruptionbudget broken-pdb-floor -o json ;;
    pdb_room)   "${kc[@]}" get poddisruptionbudget healthy-pdb-room -o json ;;
    pvc_orphan) "${kc[@]}" get persistentvolumeclaim broken-unused-disk -o json ;;
    pvc_used)   "${kc[@]}" get persistentvolumeclaim healthy-disk -o json ;;
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
           # The three keys above read container [0], which is the container
           # every predicate here is about — except `broken-neverback`, whose
           # whole subject is *which* of three containers is in which state, and
           # whose FAIL therefore printed the one container that was fine
           # (broken-hostpath has two as well, and the second was equally
           # invisible; nothing had needed it yet). Only
           # reached when there is more than one, so every existing FAIL line is
           # byte-identical: the empty array is dropped by the filter at the
           # foot of this function.
           # (No apostrophes in these comments: the whole filter is one
           # single-quoted shell string, and one would end it.)
           containers: [ (.status.containerStatuses // []) | select(length > 1) | .[]
                         | { name, restarts: .restartCount, state: (.state | keys[0]?),
                             exit: .state.terminated.exitCode }
                         | with_entries(select(.value != null)) ],
           init:       (.status.initContainerStatuses // [])[0].state,
           enacted:    (.status.containerStatuses // [])[0].resources,
           declared:   (.spec.containers // [])[0].resources,
           replicas:   ({ want: .spec.replicas } + (.status // {} | {replicas, readyReplicas, updatedReplicas, unavailableReplicas, desiredNumberScheduled, numberReady})
                        | with_entries(select(.value != null))),
           conditions: [ (.status.conditions // [])[] | {type,status,reason} ],
           # The cluster-wide report inputs (D129, D130). Without these a FAIL on
           # one of the six printed the name and nothing else, which is the
           # failure this whole function exists to prevent: an operator with an
           # hour of trip budget left and no idea which number was wrong. Same
           # shape as `replicas` above and dropped the same way, so every
           # existing FAIL line is byte-identical.
           overhead:   .spec.overhead,
           runtimeClass: .spec.runtimeClassName,
           mounts:     [ (.spec.volumes // [])[] | .persistentVolumeClaim.claimName // empty ],
           budget:     (.status // {} | {expectedPods, currentHealthy, desiredHealthy, disruptionsAllowed}
                        | with_entries(select(.value != null))),
           # `class` is kept when it is the empty string and not only when it is
           # set: an empty storageClassName is what says no provisioner was
           # involved, which is the difference between the two claims.
           claim:      ({ asked: .spec.resources.requests.storage, class: .spec.storageClassName,
                          got: .status.capacity.storage }
                        | with_entries(select(.value != null))),
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

# broken-reboot's restart count, or `0` while the object cannot be read at all.
# Its own function because the loop below reads it three times a pass and an
# empty string in an arithmetic test is a syntax error rather than a zero — which
# is exactly what a `kubectl get` against a node that is still coming back
# returns.
reboot_restarts() {
  local n
  n=$(kubectl --context "kind-$CLUSTER" get pod broken-reboot \
        -o jsonpath='{.status.containerStatuses[0].restartCount}' 2>/dev/null) || n=
  echo "${n:-0}"
}

# The one state whose producer is the machine and not the manifest (NOTES
# § D90). One knob, `K8RS_RUNTIME_TIMEOUT` (default 300s), bounds every wait in
# here — each loop below is a wait on a machine, and a machine that has stopped
# answering has stopped answering for all of them.
#
# It is damage to a **node**, so this runs after every pod capture is on disk and
# before `break-nodes` — the `fixtures` recipe in the justfile is the only caller
# that gets the order right.
#
# Nothing here needs undoing and `unbreak` grows no step for it: a rebooted node
# comes back by itself. The one residue is a reboot that died between
# `docker stop` and `docker start`, which `unbreak` now starts, because a stopped
# node container is not something the API server can be asked about.
break_runtime() {
  need kubectl; need docker; need jq
  local kc=(kubectl --context "kind-$CLUSTER")
  local rnode target=3 before deadline

  rnode=$("${kc[@]}" get pod broken-reboot -o jsonpath='{.spec.nodeName}' 2>/dev/null) || rnode=
  [ -n "$rnode" ] || {
    echo "break-runtime: broken-reboot has to be scheduled before the node it is on can be" >&2
    echo "               read off it. The pod comes first:" >&2
    echo "                 $0 break, then let it schedule." >&2
    return 1
  }
  # docker is the whole of this step, and the only thing in it that can fail on
  # permissions: `need docker` proves the binary is on PATH and says nothing
  # about the socket, which is per-login on the machine this runs on. Discovered
  # here, with the same call that has to work below — a denial found halfway
  # through leaves a node stopped and the capture unwritten, which is the same
  # trap `break_nodes` opens with.
  docker exec "$rnode" true 2>/dev/null || {
    echo "break-runtime: docker cannot reach the node container $rnode, and a reboot does not" >&2
    echo "               go through the API server. Refusing before anything is stopped:" >&2
    echo "               fix docker access (the docker group is per-login here)." >&2
    return 1
  }

  # --- THE NODE REBOOT ---
  # Counted by the object rather than by the loop: `docker restart` returning
  # says the container came back, not that the kubelet noticed its containers
  # were gone. Three, because `RESTARTS_WARN` is 3 and the fixture is rule 5's
  # band — and the loop re-reads rather than assuming a reboot is worth exactly
  # one, so a node that came back twice as far still lands on the right side.
  while [ "$(reboot_restarts)" -lt "$target" ]; do
    before=$(reboot_restarts)
    echo "  rebooting $rnode — broken-reboot is at $before of $target restarts"
    docker restart "$rnode" >/dev/null
    deadline=$(( SECONDS + ${K8RS_RUNTIME_TIMEOUT:-300} ))
    while [ $SECONDS -lt $deadline ] && [ "$(reboot_restarts)" -le "$before" ]; do sleep 5; done
    [ "$(reboot_restarts)" -gt "$before" ] || {
      echo "break-runtime: $rnode was rebooted and broken-reboot's restart count did not move before" >&2
      echo "               K8RS_RUNTIME_TIMEOUT ran out. Nothing else here can raise it, so this is" >&2
      echo "               the node not coming back rather than a slow one:" >&2
      echo "                 kubectl --context kind-$CLUSTER get node $rnode" >&2
      return 1
    }
  done
  # `break-nodes` runs next and needs three healthy workers, so this one is not
  # left half-returned. The pod's own Ready is asserted beside the node's because
  # the fixture is a container that is **serving** with the restarts behind it.
  "${kc[@]}" wait --for=condition=Ready node/"$rnode" --timeout=300s
  "${kc[@]}" wait --for=condition=Ready pod/broken-reboot --timeout=300s

  echo "  rebooted $rnode"
  assert_states "${RUNTIME_STATES[@]}"
}

# The three node states, and the only step here that damages what is already
# running: one worker stops taking new pods, a second evicts everything that
# does not tolerate a NoExecute taint, and a third loses its kubelet — after
# which every pod on it reads Unknown and, minutes later, is marked for
# deletion. So this runs *after* the pod fixtures are captured and immediately
# before the node capture, and `unbreak` puts all three back.
#
# **And one pod capture, which is here because it cannot be anywhere else**
# (NOTES § D156): `broken-unstarted` is bound by hand to the kubelet-less worker,
# so that it is placed and yet never started. It is the one pod in the file the
# sentence above does not apply to — its NoExecute tolerations carry no
# `tolerationSeconds`, so it is the pod on that node that is *not* marked for
# deletion five minutes later, which is the whole reason it survives to be
# captured.
break_nodes() {
  need kubectl; need docker; need jq
  local kc=(kubectl --context "kind-$CLUSTER") w n got
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

  # --- THE POD THE KUBELET NEVER SEES (NOTES § D156) ---
  # `broken-unstarted` was created by `break` carrying `schedulerName:
  # does-not-exist`, so nothing has placed it. Placing it here, by hand, is what
  # gives rule 13 its empty-status fixture: `PodScheduled: True` with a stamp,
  # and no `status.containerStatuses` key at all.
  #
  # **Through the `binding` subresource, and after the kubelet stop above.** Both
  # halves are measured
  # (reports/2026-08-22-rule-13-the-pod-with-no-container-status.md). The
  # subresource, because it is the only thing that writes the condition — a
  # create carrying `spec.nodeName` writes none at all, which is rule 14's shape
  # and not this one's (report § 1 against § 2). After the stop, because a
  # running kubelet would pull the pod and write a container status for it within
  # seconds, and the whole fixture is that no such status exists. The pod's own
  # infinite NoExecute tolerations (scripts/broken.yaml states why) are what stop
  # the taint manager evicting it once this node goes Unknown a minute from now.
  #
  # `default` is spelled out because a raw path has no context to take a
  # namespace from; every pod in broken.yaml lands there. `-f -` is stdin, which
  # `kubectl create --raw` names in its own refusal message ("--raw can only use
  # a single local file or stdin") — no temp file to clean up on a failure.
  #
  # **Read the whole pod first, and take three different exits from it**, like the
  # resize in `break_it` and every undo in `unbreak`. There are three states here
  # and a probe that only asks *is it bound* collapses two of them:
  #
  #   - **not there at all** — `break` was never run, or was run against another
  #     cluster. Nothing to place, and it is this function that has to say so:
  #     an empty jsonpath reads exactly like an unbound pod, so the bind below
  #     would fire and the POST would 404 with `set -e` ending the run on
  #     kubectl's message. That was this block's own defect, found by the
  #     operator review (reports/2026-08-22-rule-13-family-review.md § 5).
  #   - **there and unbound** — the first `break-nodes` of a trip. Bind it.
  #   - **there and already bound** — the second `break-nodes` of a trip that
  #     failed at a guard and was re-run, which the cordon pick above goes to some
  #     length to keep working. A binding POST against a pod that already has one
  #     is a 409 and `set -e` would end the run on that instead, so it is skipped.
  #
  # The assertion below runs in the last two cases alike, so a re-run re-proves
  # the shape rather than inheriting the first run's claim about it.
  # stderr is **not** swallowed here, unlike the reads in `unbreak`: kubectl has
  # already said whether this is a NotFound or an API server that stopped
  # answering, and those need different things done about them. Guessing between
  # them in our own words would be the same mistake one line down — a message
  # that names the wrong cause is worse than the raw one it replaced.
  local pod
  pod=$("${kc[@]}" get pod broken-unstarted -o json) || {
    echo "break-nodes: the line above is why — there is nothing to place on ${rest[1]}." >&2
    echo "             broken-unstarted is rule 13's empty-status fixture — scripts/broken.yaml" >&2
    echo "             § broken-unstarted, NOTES § D156 — and 'break' is what creates it." >&2
    echo "             The three node states above are already applied and ${rest[1]}'s kubelet" >&2
    echo "             is stopped, so the way back is: $0 unbreak, then $0 break." >&2
    return 1
  }
  if [ -z "$(jq -r '.spec.nodeName // ""' <<<"$pod")" ]; then
    "${kc[@]}" create -f - --raw "/api/v1/namespaces/default/pods/broken-unstarted/binding" >/dev/null <<EOF
{"apiVersion":"v1","kind":"Binding","metadata":{"name":"broken-unstarted","namespace":"default"},"target":{"apiVersion":"v1","kind":"Node","name":"${rest[1]}"}}
EOF
  fi
  # Asserted here rather than through the predicate table `assert_states` polls,
  # and the difference is not style: the bind writes the condition synchronously
  # (report § 2), so there is nothing to wait for, and a shape that is wrong is
  # wrong *now* — polling it for seven minutes would only delay the message.
  #
  # Four clauses, one per way this capture goes quietly wrong. The node, because
  # a pod bound to any *other* worker is the positive of D156 ruling 2 wearing the
  # negative's name, and on a re-run it is the one thing the `if` above could have
  # skipped past. Then: a bind that wrote no condition, a kubelet that was in fact
  # still running and wrote a status, and a pod the taint manager already marked
  # (rule 13 is silent on a deletionTimestamp, so that capture is a fixture of
  # nothing). `has` and never a length test, because the **absent key** is the
  # fixture — `containerStatuses: []` is a different object, and one the API
  # server will not even accept (D156 § 1). `.status // {}` so a pod with no
  # status block at all fails on the first clause with this message rather than on
  # a jq type error.
  "${kc[@]}" get pod broken-unstarted -o json | jq -e --arg n "${rest[1]}" '
      .spec.nodeName == $n
      and ([.status.conditions[]? | select(.type == "PodScheduled" and .status == "True"
                                           and .lastTransitionTime != null)] | length) == 1
      and (.status // {} | has("containerStatuses") | not)
      and .metadata.deletionTimestamp == null' >/dev/null || {
    echo "break-nodes: broken-unstarted did not land on ${rest[1]} as {PodScheduled True, no" >&2
    echo "             container status, not deleting}, which is the whole of rule 13's" >&2
    echo "             empty-status fixture (NOTES § D156)." >&2
    # Not `diagnose`: that one is shared by every other fixture here and prints
    # neither the node nor the difference between an absent containerStatuses and
    # a present one with no state — which is three of these four clauses. Its own
    # line, naming exactly what was asked.
    got=$("${kc[@]}" get pod broken-unstarted -o json 2>/dev/null \
          | jq -c '{node: .spec.nodeName,
                    conditions: [.status.conditions[]? | {type, status, lastTransitionTime}],
                    containerStatuses: (if (.status // {} | has("containerStatuses"))
                                        then [.status.containerStatuses[].name] else "ABSENT" end),
                    deletionTimestamp: .metadata.deletionTimestamp}') || got=
    echo "             got: ${got:-the pod is gone — something deleted it between the bind and this read}" >&2
    return 1
  }
  echo "  cordoned $cordon · tainted ${rest[0]} · stopped the kubelet on ${rest[1]}"
  echo "  (the node controller takes about a minute to notice the third, and it is the"
  echo "   controller — never kubectl — that stamps a timeAdded on a taint)"
  echo "  broken-unstarted is on ${rest[1]}: placed, and no kubelet left there to start it"

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

# **The second refusal, and it is not the one above.** That one is *policy* — a
# name whose node names `sanitize.jq` refuses — and it is `up`-only on purpose, so
# a cluster already built under it can still be torn down. This one is
# *arithmetic*, and it applies to every subcommand because it can trap nothing: a
# name outside the class below cannot become a kind cluster at all. kind builds a
# container called `<name>-control-plane`, and the docker daemon refuses it in
# these words, measured against the daemon on 2026-08-21 rather than read off a
# doc:
#
#   Invalid container name (a/b-control-plane), only [a-zA-Z0-9][a-zA-Z0-9_.-] are allowed
#
# It exists because `break_it` substitutes `$CLUSTER` into a **sed replacement**
# (the `nodeName:` rewrite), where `&` is the whole match and `/` ends the
# expression — and `K8RS_CLUSTER` is user-supplied, which is the entire reason
# that rewrite exists. Escaping at the one site would also work; refusing here is
# the same length, and the two sets are identical — docker's class holds no `/`,
# no `&` and no backslash — so this is not a guard invented for a case that cannot
# arise. It is docker's own rule, said early enough to be a sentence instead of a
# Pending pod at minute two.
#
# `LC_ALL=C` because a bracket range is collation-ordered and not byte-ordered: a
# locale where `[A-Za-z]` means something else would refuse a name docker accepts,
# which is the one direction this must not fail in. A here-string and not a pipe,
# because `grep -q` closes early and `set -o pipefail` would read the writer's
# EPIPE as a failed match.
refuse_unusable_name() { # $1 = cluster name
  LC_ALL=C grep -qE '^[A-Za-z0-9][A-Za-z0-9_.-]*$' <<<"$1" && return 0
  echo "cluster.sh: '$1' cannot name a kind cluster." >&2
  echo "  kind would build a container called '$1-control-plane', and the docker daemon" >&2
  echo "  allows only [a-zA-Z0-9][a-zA-Z0-9_.-] in a container name — so no cluster can" >&2
  echo "  exist under this name and there is nothing here to operate on." >&2
  echo "  It is also the name this file substitutes into a sed replacement (the nodeName" >&2
  echo "  rewrite in \`break\`), where '&' and '/' would rewrite the manifest into garbage." >&2
  echo "  The fixture cluster is 'k8rs'; an ephemeral review cluster is 'review'." >&2
  return 1
}

# A guard nobody has seen fail is not a guard (todo.md, Phase 1). It needs no
# cluster, no kind and no network — the thing under test is a name.
self_test() {
  local n rc sfail=0
  for n in k8rs review k8s-review my-cluster reviewk8rs; do
    rc=0; refuse_family_name "$n" 2>/dev/null || rc=$?
    [ "$rc" -eq 0 ] || { echo "FAIL  self-test: '$n' is a name cluster.sh must build"; sfail=1; }
  done
  # `k8rs-review` is the name three agents in a row typed; the rest are the same
  # mistake spelled differently, and every one of them produces node names
  # sanitize.jq refuses.
  for n in k8rs-review k8rs-review-2 k8rs2 k8rs-test k8rs-; do
    rc=0; refuse_family_name "$n" 2>/dev/null || rc=$?
    [ "$rc" -eq 1 ] || { echo "FAIL  self-test: '$n' wears the fixture cluster's name and was accepted"; sfail=1; }
  done
  # The names docker's own message says it will and will not accept — including
  # the three that break a sed replacement, which is why this refusal exists.
  for n in k8rs review my-cluster A_b.c-1 x; do
    rc=0; refuse_unusable_name "$n" 2>/dev/null || rc=$?
    [ "$rc" -eq 0 ] || { echo "FAIL  self-test: '$n' is a name docker accepts as a container and this refused it"; sfail=1; }
  done
  for n in 'a/b' 'a&b' 'a b' '-lead' '.lead' '' 'a$b' 'a\\b' "a'b"; do
    rc=0; refuse_unusable_name "$n" 2>/dev/null || rc=$?
    [ "$rc" -eq 1 ] || { echo "FAIL  self-test: '$n' cannot name a container and was accepted — and it reaches a sed replacement"; sfail=1; }
  done
  [ $sfail -eq 0 ] && echo "cluster.sh: self-test passed — 'k8rs' and any name without that prefix build; 'k8rs-review' and the rest of the family are refused before kind runs; and a name outside docker's container-name class, which is every name that would break the nodeName rewrite, is refused on every subcommand"
  return $sfail
}

# Before the dispatch, so it covers every subcommand rather than `up` alone, and
# before anything has run. See the function for why this one is universal and
# `refuse_family_name` is not.
refuse_unusable_name "$CLUSTER" || exit 1

case "${1:-}" in
  --self-test) self_test ;;
  up)          up ;;
  down)        down ;;
  break)       break_it ;;
  verify)      verify ;;
  break-runtime) break_runtime ;;
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
