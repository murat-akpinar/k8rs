#!/usr/bin/env bash
# scripts/cluster.sh verify() holds one jq predicate per fixture, and each one
# decides whether that fixture is trustworthy. They only ever ran against a live
# cluster, where being wrong is invisible in both directions: a predicate that is
# too loose passes a pod that is in the wrong state — a rule that can never fail
# — and one that is too tight burns the whole 420s timeout and then fails a pod
# that was correct all along. jq cannot tell the two apart either; a broken
# filter exits non-zero exactly like an unmet condition does.
#
# So every predicate is run here, offline, twice: once against an object in the
# state it is about (must match) and once against an object next to that state
# but not in it (must not match). A predicate that matches everything is as
# useless as one that matches nothing, which is why the negative half is not
# optional (CLAUDE.md § Tests must not lie).
#
# --- THE JSON BELOW IS NOT A FIXTURE. ---
# It exists to exercise *shell predicates* and nothing else. Rule fixtures are
# real cluster captures and live in tests/fixtures/, written by `just fixtures`;
# CLAUDE.md forbids hand-written JSON there and this file must never become a
# source for it. Nothing here is loaded by any Rust test.
#
# Where the shapes come from, because JSON written by reading a predicate only
# proves the predicate agrees with itself:
#   - the pod objects, the ReplicaSet List and the healthy pod are cut out of
#     real `kubectl get -o json` captures taken from the kind test cluster
#     (kindest/node:v1.36.1) once its states had settled, sanitized by
#     scripts/sanitize.jq. Fields the predicates read are untouched; only noise
#     the predicates never look at (containerID, imageID, managedFields, the
#     projected token volume on pods where no predicate reads .spec.volumes) was
#     dropped to keep this file readable.
#   - the two Deployments and the rule 5 pod are assembled from those same
#     captures plus the condition strings the Deployment controller writes, per
#     the kubernetes.io Deployment page. No capture of them exists yet: the
#     manifests that produce them are new, and progressDeadlineSeconds defaults
#     to 600s — longer than verify()'s own timeout — so W2 had never fired when
#     the snapshot was taken. Re-check these two against the next real capture.
#   - the owned pod (in both halves of its crash loop), the mirror pod and the
#     coredns pod are `kubectl get -o json` captures taken from the same kind
#     cluster on 2026-08-12, through the same sanitizer, trimmed the same way.
#     Both halves, because a predicate proven in one of them is a predicate that
#     passes `verify` and then watches the capture land in the other. The six
#     objects below them are composed
#     out of those captures — never written by hand — and each names the fields
#     it moved and why the cluster does not hold that state still long enough to
#     be captured.
#   - the five objects added for the manifests this box changes — the healthy
#     pod's *spec*, broken-hostpath's volumes with their mounts, broken-oom cut
#     for its declared and enacted resources, kindnet's DaemonSet counters, and
#     the three nodes — are cut straight out of `tests/fixtures/*.json`, which
#     are captures. They are pasted rather than read at runtime on purpose: the
#     trip that follows this box rewrites those files, and a negative case that
#     is "the cluster before the manifest changed" stops being one the moment
#     the file it was read from is recaptured.
#   - and the shapes those manifests will produce are composed from them, each
#     naming the object the capture owes. Those are the entries with no capture
#     behind them *yet*; re-cut them from the real fixture once it lands.
#   - field names and nesting cross-checked against the Kubernetes API reference
#     (PodStatus / ContainerStatus / ContainerState / PodCondition) and the
#     k8s-openapi v1_36 generated types.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
command -v jq >/dev/null || { echo "verify-test: jq is not installed"; exit 127; }

# --- PREDICATES START ---
# Read out of cluster.sh rather than copied into this file: a copy would prove
# the copy, and the two would drift without a word.
declare -A want=()
eval "$(sed -n '/^declare -A want=(/,/^)$/p' "$here/cluster.sh")"

# "extracted nothing" and "nothing to extract" print the same line, so name what
# has to be there (CLAUDE.md § A derived list asserts it found something). Every
# key, not a sample of them: a table that arrives half-read is the failure this
# is for, and a sample cannot tell that from a table that is genuinely shorter.
for canary in oom crashloop image config pending hostpath readiness restarts \
              nolimits stuck init quota w2 owned resize podlimit sts rollout ds \
              healthy_init healthy_sidecar healthy_hostpath healthy_podlevel \
              cordoned tainted notready; do
  [ -n "${want[$canary]:-}" ] || {
    echo "verify-test: cluster.sh has no predicate '$canary' — the extraction broke, not the predicate"
    exit 1
  }
done
# No count check beside that list any more: the assertions at the foot of this
# file walk `want` itself and demand that every key in it has been both matched
# and refused here, which is what the count was standing in for and is not
# satisfied by adding a case that only ever passes.
# --- PREDICATES END ---

# --- CORPUS START ---
declare -A obj=()

# broken-oom, settled — the kill is in lastState and the container is in backoff
obj[oom]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-oom",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "hog",
        "resources": {
          "limits": {
            "memory": "64Mi"
          },
          "requests": {
            "memory": "64Mi"
          }
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [hog]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [hog]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "hog",
        "ready": false,
        "started": false,
        "restartCount": 5,
        "state": {
          "waiting": {
            "message": "back-off 2m40s restarting failed container=hog pod=broken-oom_default(f9aad94e-81d0-4f79-8477-640791a68e69)",
            "reason": "CrashLoopBackOff"
          }
        },
        "lastState": {
          "terminated": {
            "containerID": "containerd://e08cd74a7b42a33641786dffe5d12853b1e33bc933c936e29acf578212d5b1ab",
            "exitCode": 137,
            "finishedAt": "2026-08-11T22:46:38Z",
            "reason": "OOMKilled",
            "startedAt": "2026-08-11T22:46:38Z"
          }
        }
      }
    ]
  }
}
JSON
)

# broken-crashloop, settled — CrashLoopBackOff over exit 1 (reason Error, not OOMKilled)
obj[crashloop]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-crashloop",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "quitter",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [quitter]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [quitter]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "quitter",
        "ready": false,
        "started": false,
        "restartCount": 5,
        "state": {
          "waiting": {
            "message": "back-off 2m40s restarting failed container=quitter pod=broken-crashloop_default(8ac42865-23a6-4566-abd9-bf2b9d429f2f)",
            "reason": "CrashLoopBackOff"
          }
        },
        "lastState": {
          "terminated": {
            "containerID": "containerd://55d983126438d34c2d7417a68b024d83f3b513d03ece5118b3319611e648d76d",
            "exitCode": 1,
            "finishedAt": "2026-08-11T22:46:43Z",
            "reason": "Error",
            "startedAt": "2026-08-11T22:46:41Z"
          }
        }
      }
    ]
  }
}
JSON
)

# broken-image — the registry does not resolve
obj[image]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-image",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "nope",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Pending",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [nope]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [nope]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "nope",
        "ready": false,
        "started": false,
        "restartCount": 0,
        "state": {
          "waiting": {
            "message": "Back-off pulling image \"registry.invalid/does-not-exist:v9\": ErrImagePull: failed to pull and unpack image \"registry.invalid/does-not-exist:v9\": failed to resolve reference \"registry.invalid/does-not-exist:v9\": failed to do request: Head \"https://registry.invalid/v2/does-not-exist/manifests/v9\": dial tcp: lookup registry.invalid on 172.18.0.1:53: no such host",
            "reason": "ImagePullBackOff"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-config — the ConfigMap the pod needs does not exist
obj[config]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-config",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Pending",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": false,
        "started": false,
        "restartCount": 0,
        "state": {
          "waiting": {
            "message": "configmap \"this-configmap-does-not-exist\" not found",
            "reason": "CreateContainerConfigError"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-pending — never scheduled, so there is no containerStatuses array at all.
#
# The two tolerations are in the capture and were trimmed out of this copy, which
# made the negative weaker than the object it stands for: **no pod ever has an
# empty `tolerations`**. The `DefaultTolerationSeconds` admission plugin adds
# these two to every pod that does not already carry them, so a check written as
# "this pod has tolerations" is satisfied by every pod in the cluster, and a
# negative without them cannot catch one. Pasted back from
# tests/fixtures/pending.json.
obj[pending]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-pending",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "greedy",
        "resources": {
          "requests": {
            "cpu": "500"
          }
        }
      }
    ],
    "tolerations": [
      {
        "effect": "NoExecute",
        "key": "node.kubernetes.io/not-ready",
        "operator": "Exists",
        "tolerationSeconds": 300
      },
      {
        "effect": "NoExecute",
        "key": "node.kubernetes.io/unreachable",
        "operator": "Exists",
        "tolerationSeconds": 300
      }
    ]
  },
  "status": {
    "phase": "Pending",
    "conditions": [
      {
        "type": "PodScheduled",
        "status": "False",
        "reason": "Unschedulable",
        "message": "0/3 nodes are available: 1 node(s) had untolerated taint(s), 2 Insufficient cpu. no new claims to deallocate, preemption: 0/3 nodes are available: 3 Preemption is not helpful for scheduling."
      }
    ]
  }
}
JSON
)

# broken-hostpath — mounts / writable, beside the normal projected token volume
obj[hostpath]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-hostpath",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "nosy",
        "resources": {}
      }
    ],
    "volumes": [
      {
        "hostPath": {
          "path": "/",
          "type": ""
        },
        "name": "root"
      },
      {
        "name": "kube-api-access-jss55",
        "projected": {
          "defaultMode": 420,
          "sources": [
            {
              "serviceAccountToken": {
                "expirationSeconds": 3607,
                "path": "token"
              }
            },
            {
              "configMap": {
                "items": [
                  {
                    "key": "ca.crt",
                    "path": "ca.crt"
                  }
                ],
                "name": "kube-root-ca.crt"
              }
            },
            {
              "downwardAPI": {
                "items": [
                  {
                    "fieldRef": {
                      "apiVersion": "v1",
                      "fieldPath": "metadata.namespace"
                    },
                    "path": "namespace"
                  }
                ]
              }
            }
          ]
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "nosy",
        "ready": true,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:36Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-readiness — the container really is running, it just never passes the probe
obj[readiness]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-readiness",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": false,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:32Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-nolimits — resources is {} rather than absent
obj[nolimits]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-nolimits",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": true,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:35Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-stuck as verify() sees it: the finalizer is on, the delete has not happened yet
obj[stuck]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-stuck",
    "namespace": "default",
    "finalizers": [
      "k8rs.test/never-removed"
    ]
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": true,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:33Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# broken-init — the init container is in backoff while the app container waits at PodInitializing. Phase is Pending, not Running
obj[init]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-init",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Pending",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "False",
        "reason": "ContainersNotInitialized",
        "message": "containers with incomplete status: [migrate]"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [app]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "initContainerStatuses": [
      {
        "name": "migrate",
        "ready": false,
        "started": false,
        "restartCount": 5,
        "state": {
          "waiting": {
            "message": "back-off 2m40s restarting failed container=migrate pod=broken-init_default(44bdf259-251f-401d-abf0-cc899f43c8f5)",
            "reason": "CrashLoopBackOff"
          }
        },
        "lastState": {
          "terminated": {
            "containerID": "containerd://61abbd2e20fd2eea55ac812b77959ad35b54a62bad85fcb16561cf2da18a7775",
            "exitCode": 1,
            "finishedAt": "2026-08-11T22:46:59Z",
            "reason": "Error",
            "startedAt": "2026-08-11T22:46:57Z"
          }
        }
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": false,
        "started": false,
        "restartCount": 0,
        "state": {
          "waiting": {
            "reason": "PodInitializing"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# the healthy pod, captured from the same cluster at the same moment — the negative half of nearly every predicate
obj[healthy]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "healthy",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "app",
        "resources": {
          "limits": {
            "cpu": "100m",
            "memory": "64Mi"
          },
          "requests": {
            "cpu": "10m",
            "memory": "16Mi"
          }
        }
      }
    ],
    "volumes": [
      {
        "name": "kube-api-access-wbczb",
        "projected": {
          "defaultMode": 420,
          "sources": [
            {
              "serviceAccountToken": {
                "expirationSeconds": 3607,
                "path": "token"
              }
            },
            {
              "configMap": {
                "items": [
                  {
                    "key": "ca.crt",
                    "path": "ca.crt"
                  }
                ],
                "name": "kube-root-ca.crt"
              }
            },
            {
              "downwardAPI": {
                "items": [
                  {
                    "fieldRef": {
                      "apiVersion": "v1",
                      "fieldPath": "metadata.namespace"
                    },
                    "path": "namespace"
                  }
                ]
              }
            }
          ]
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "initContainerStatuses": [
      {
        "name": "migrate",
        "ready": true,
        "started": false,
        "restartCount": 0,
        "state": {
          "terminated": {
            "containerID": "containerd://4412f53cda454909165de2d3df5f108bee083319dde3a4bf5767d63111242606",
            "exitCode": 0,
            "finishedAt": "2026-08-11T22:43:35Z",
            "reason": "Completed",
            "startedAt": "2026-08-11T22:43:35Z"
          }
        },
        "lastState": {}
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": true,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:39Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# the ReplicaSet the quota-denied Deployment made — W1's only evidence
obj[quota_rs]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [
    {
      "apiVersion": "apps/v1",
      "kind": "ReplicaSet",
      "metadata": {
        "name": "broken-quota-59654c756",
        "namespace": "k8rs-quota"
      },
      "status": {
        "conditions": [
          {
            "lastTransitionTime": "2026-08-11T22:43:24Z",
            "message": "pods \"broken-quota-59654c756-chccd\" is forbidden: exceeded quota: deny-all-pods, requested: pods=1, used: pods=0, limited: pods=0",
            "reason": "FailedCreate",
            "status": "True",
            "type": "ReplicaFailure"
          }
        ],
        "observedGeneration": 1,
        "replicas": 0,
        "terminatingReplicas": 0
      }
    }
  ]
}
JSON
)

# a ReplicaSet that rolled out cleanly — no conditions key at all, which is what a healthy one looks like
obj[healthy_rs]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [
    {
      "apiVersion": "apps/v1",
      "kind": "ReplicaSet",
      "metadata": {
        "name": "healthy-deploy-6c9d4b7f8",
        "namespace": "default"
      },
      "status": {
        "replicas": 2,
        "fullyLabeledReplicas": 2,
        "readyReplicas": 2,
        "availableReplicas": 2,
        "terminatingReplicas": 0,
        "observedGeneration": 1
      }
    }
  ]
}
JSON
)

# an empty namespace. Not a pass: 'no ReplicaSet' is not 'the ReplicaSet failed'
obj[empty_rs]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [],
  "metadata": {
    "resourceVersion": ""
  }
}
JSON
)

# rule 5: Running and ready NOW, but it got there after three crashes
obj[restarts]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-restarts",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "flaky",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "flaky",
        "ready": true,
        "started": true,
        "restartCount": 3,
        "state": {
          "running": {
            "startedAt": "2026-08-11T22:43:39Z"
          }
        },
        "lastState": {
          "terminated": {
            "containerID": "containerd://55d983126438d34c2d7417a68b024d83f3b513d03ece5118b3319611e648d76d",
            "exitCode": 1,
            "finishedAt": "2026-08-11T22:46:43Z",
            "reason": "Error",
            "startedAt": "2026-08-11T22:46:41Z"
          }
        }
      }
    ]
  }
}
JSON
)

# W2: the rollout gave up. Progressing goes to False/ProgressDeadlineExceeded
obj[w2_deploy]=$(cat <<'JSON'
{
  "apiVersion": "apps/v1",
  "kind": "Deployment",
  "metadata": {
    "name": "broken-quota",
    "namespace": "k8rs-quota",
    "generation": 1
  },
  "spec": {
    "replicas": 1,
    "progressDeadlineSeconds": 60
  },
  "status": {
    "observedGeneration": 1,
    "replicas": 0,
    "unavailableReplicas": 1,
    "conditions": [
      {
        "type": "Available",
        "status": "False",
        "reason": "MinimumReplicasUnavailable",
        "message": "Deployment does not have minimum availability.",
        "lastTransitionTime": "2026-08-11T22:43:24Z",
        "lastUpdateTime": "2026-08-11T22:43:24Z"
      },
      {
        "type": "ReplicaFailure",
        "status": "True",
        "reason": "FailedCreate",
        "message": "pods \"broken-quota-59654c756-chccd\" is forbidden: exceeded quota: deny-all-pods, requested: pods=1, used: pods=0, limited: pods=0",
        "lastTransitionTime": "2026-08-11T22:43:24Z",
        "lastUpdateTime": "2026-08-11T22:43:24Z"
      },
      {
        "type": "Progressing",
        "status": "False",
        "reason": "ProgressDeadlineExceeded",
        "message": "ReplicaSet \"broken-quota-59654c756\" has timed out progressing.",
        "lastTransitionTime": "2026-08-11T22:44:24Z",
        "lastUpdateTime": "2026-08-11T22:44:24Z"
      }
    ]
  }
}
JSON
)

# a rollout that finished. Progressing is True — the negative half of W2
obj[healthy_deploy]=$(cat <<'JSON'
{
  "apiVersion": "apps/v1",
  "kind": "Deployment",
  "metadata": {
    "name": "healthy-deploy",
    "namespace": "default",
    "generation": 1
  },
  "spec": {
    "replicas": 2,
    "progressDeadlineSeconds": 600
  },
  "status": {
    "observedGeneration": 1,
    "replicas": 2,
    "updatedReplicas": 2,
    "readyReplicas": 2,
    "availableReplicas": 2,
    "conditions": [
      {
        "type": "Available",
        "status": "True",
        "reason": "MinimumReplicasAvailable",
        "message": "Deployment has minimum availability.",
        "lastTransitionTime": "2026-08-11T22:43:39Z",
        "lastUpdateTime": "2026-08-11T22:43:39Z"
      },
      {
        "type": "Progressing",
        "status": "True",
        "reason": "NewReplicaSetAvailable",
        "message": "ReplicaSet \"healthy-deploy-6c9d4b7f8\" has successfully progressed.",
        "lastTransitionTime": "2026-08-11T22:43:39Z",
        "lastUpdateTime": "2026-08-11T22:43:39Z"
      }
    ]
  }
}
JSON
)

# broken-owned's pod: the one broken pod in the repo that has an owner. A
# Deployment's pod has a generated name, so verify() fetches it by label and a
# List is what comes back — the shape, not just the object, is what differs.
obj[owned_pods]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [
    {
      "apiVersion": "v1",
      "kind": "Pod",
      "metadata": {
        "name": "broken-owned-6576fd8bc8-n8p4m",
        "namespace": "default",
        "ownerReferences": [
          {
            "apiVersion": "apps/v1",
            "kind": "ReplicaSet",
            "name": "broken-owned-6576fd8bc8",
            "uid": "55e9015b-6e28-424d-8602-f63d23ff5601",
            "controller": true,
            "blockOwnerDeletion": true
          }
        ]
      },
      "spec": {
        "containers": [
          {
            "name": "quitter",
            "resources": {}
          }
        ]
      },
      "status": {
        "phase": "Running",
        "conditions": [
          {
            "type": "PodReadyToStartContainers",
            "status": "True"
          },
          {
            "type": "Initialized",
            "status": "True"
          },
          {
            "type": "Ready",
            "status": "False",
            "reason": "ContainersNotReady",
            "message": "containers with unready status: [quitter]"
          },
          {
            "type": "ContainersReady",
            "status": "False",
            "reason": "ContainersNotReady",
            "message": "containers with unready status: [quitter]"
          },
          {
            "type": "PodScheduled",
            "status": "True"
          }
        ],
        "containerStatuses": [
          {
            "name": "quitter",
            "ready": false,
            "started": false,
            "restartCount": 4,
            "state": {
              "waiting": {
                "message": "back-off 1m20s restarting failed container=quitter pod=broken-owned-6576fd8bc8-n8p4m_default(ea503697-d728-4ea1-b566-dbce76015f88)",
                "reason": "CrashLoopBackOff"
              }
            },
            "lastState": {
              "terminated": {
                "containerID": "containerd://582901e141be10e6116fef33a73a76071bc450fac352ecac7fa4bdf28121364c",
                "exitCode": 1,
                "finishedAt": "2026-08-12T15:35:12Z",
                "reason": "Error",
                "startedAt": "2026-08-12T15:35:10Z"
              }
            }
          }
        ]
      }
    }
  ]
}
JSON
)

# the same pod, in the other half of the same loop: the container has died and
# the kubelet has not put it back in backoff yet, so `state` is `terminated` and
# `waiting` is gone. Captured live rather than composed, because the loop spends
# most of its time here — the state both [crashloop] and [owned] used to be
# unable to see. It stands in for broken-crashloop too: identical image, identical
# command, and neither predicate reads anything above `.status`.
obj[crashloop_terminated]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-owned-6576fd8bc8-n8p4m",
    "namespace": "default",
    "ownerReferences": [
      {
        "apiVersion": "apps/v1",
        "kind": "ReplicaSet",
        "name": "broken-owned-6576fd8bc8",
        "uid": "55e9015b-6e28-424d-8602-f63d23ff5601",
        "controller": true,
        "blockOwnerDeletion": true
      }
    ]
  },
  "spec": {
    "containers": [
      {
        "name": "quitter",
        "resources": {}
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [quitter]"
      },
      {
        "type": "ContainersReady",
        "status": "False",
        "reason": "ContainersNotReady",
        "message": "containers with unready status: [quitter]"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "quitter",
        "ready": false,
        "started": false,
        "restartCount": 9,
        "state": {
          "terminated": {
            "containerID": "containerd://778ad0f803176694b6a0ec9452727c15ac318ec8a804dfb91521e50867e6f4f1",
            "exitCode": 1,
            "finishedAt": "2026-08-12T15:54:57Z",
            "reason": "Error",
            "startedAt": "2026-08-12T15:54:55Z"
          }
        },
        "lastState": {
          "terminated": {
            "containerID": "containerd://beef448bebd05872f795dcfaccfa843c4861dc3dcf9210f1a38cbd354cd9b447",
            "exitCode": 1,
            "finishedAt": "2026-08-12T15:49:45Z",
            "reason": "Error",
            "startedAt": "2026-08-12T15:49:43Z"
          }
        }
      }
    ]
  }
}
JSON
)

# a mirror pod, straight off the kind cluster. kubelet writes an
# ownerReference of kind Node onto every static pod, which is the claim D39
# rests on and which nothing in this repo asserted until this object.
obj[mirror]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "etcd-k8rs-control-plane",
    "namespace": "kube-system",
    "ownerReferences": [
      {
        "apiVersion": "v1",
        "kind": "Node",
        "name": "k8rs-control-plane",
        "uid": "21ed7616-9a00-4cbf-829c-778435cda3fd",
        "controller": true
      }
    ]
  },
  "spec": {
    "containers": [
      {
        "name": "etcd",
        "resources": {
          "requests": {
            "cpu": "100m",
            "memory": "100Mi"
          }
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "etcd",
        "ready": true,
        "started": true,
        "restartCount": 0,
        "state": {
          "running": {
            "startedAt": "2026-08-12T13:26:37Z"
          }
        },
        "lastState": {}
      }
    ]
  }
}
JSON
)

# a healthy pod owned by a ReplicaSet: the owner half of [owned] without the
# crash half. Also a List, because that is how the fetch returns it.
obj[coredns_pods]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [
    {
      "apiVersion": "v1",
      "kind": "Pod",
      "metadata": {
        "name": "coredns-589f44dc88-bm7hq",
        "namespace": "kube-system",
        "ownerReferences": [
          {
            "apiVersion": "apps/v1",
            "kind": "ReplicaSet",
            "name": "coredns-589f44dc88",
            "uid": "a1160877-d412-4668-86a6-34d490d175af",
            "controller": true,
            "blockOwnerDeletion": true
          }
        ]
      },
      "spec": {
        "containers": [
          {
            "name": "coredns",
            "resources": {
              "limits": {
                "memory": "170Mi"
              },
              "requests": {
                "cpu": "100m",
                "memory": "70Mi"
              }
            }
          }
        ]
      },
      "status": {
        "phase": "Running",
        "conditions": [
          {
            "type": "PodReadyToStartContainers",
            "status": "True"
          },
          {
            "type": "Initialized",
            "status": "True"
          },
          {
            "type": "Ready",
            "status": "True"
          },
          {
            "type": "ContainersReady",
            "status": "True"
          },
          {
            "type": "PodScheduled",
            "status": "True"
          }
        ],
        "containerStatuses": [
          {
            "name": "coredns",
            "ready": true,
            "started": true,
            "restartCount": 0,
            "state": {
              "running": {
                "startedAt": "2026-08-12T13:26:59Z"
              }
            },
            "lastState": {}
          }
        ]
      }
    }
  ]
}
JSON
)


# --- THE OBJECTS THE NEW MANIFESTS ARE FOR ---
# Four more cuts out of committed captures, and every one of them is also the
# object a *changed* manifest replaces. They are here for the negative half: a
# predicate tightened to demand a field nothing has produced yet must refuse the
# capture as it stands today, or the trip can bring back a fixture without the
# field and every guard stays green.

# the healthy pod again, and this time its `spec` — the copy above was trimmed
# to what the predicates then read, which did not include `initContainers`. The
# init container declares `resources: {}` exactly as captured: no init container
# in this repo has ever declared any.
obj[healthy_spec]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "healthy",
    "namespace": "default"
  },
  "spec": {
    "initContainers": [
      {
        "name": "migrate",
        "resources": {}
      }
    ],
    "containers": [
      {
        "name": "app",
        "resources": {
          "limits": {
            "cpu": "100m",
            "memory": "64Mi"
          },
          "requests": {
            "cpu": "10m",
            "memory": "16Mi"
          }
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "True"
      },
      {
        "type": "ContainersReady",
        "status": "True"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "initContainerStatuses": [
      {
        "name": "migrate",
        "ready": true,
        "started": false,
        "restartCount": 0
      }
    ],
    "containerStatuses": [
      {
        "name": "app",
        "ready": true,
        "started": true,
        "restartCount": 0
      }
    ]
  }
}
JSON
)

# broken-oom, cut for its resources rather than for its kill: the only committed
# object carrying both what the spec asked for and what the kubelet enacted,
# which is the pair D51's resize is about. broken-resize does not exist yet —
# this is the shape it starts in, and the composition below is what the patch
# turns it into.
obj[resize_base]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-oom",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "hog",
        "resources": {
          "limits": {
            "memory": "64Mi"
          },
          "requests": {
            "memory": "64Mi"
          }
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "conditions": [
      {
        "type": "PodReadyToStartContainers",
        "status": "True"
      },
      {
        "type": "Initialized",
        "status": "True"
      },
      {
        "type": "Ready",
        "status": "False"
      },
      {
        "type": "ContainersReady",
        "status": "False"
      },
      {
        "type": "PodScheduled",
        "status": "True"
      }
    ],
    "containerStatuses": [
      {
        "name": "hog",
        "ready": false,
        "started": false,
        "restartCount": 5,
        "resources": {
          "limits": {
            "memory": "64Mi"
          },
          "requests": {
            "memory": "64Mi"
          }
        }
      }
    ]
  }
}
JSON
)

# kindnet, whose DaemonSet counters are the only real ones in the repo — and all
# five of them agree with each other, which is why `desired` and `ready` could
# be read from any of the others and stay green.
obj[ds_healthy]=$(cat <<'JSON'
{
  "apiVersion": "apps/v1",
  "kind": "DaemonSet",
  "metadata": {
    "name": "kindnet",
    "namespace": "kube-system"
  },
  "status": {
    "currentNumberScheduled": 3,
    "desiredNumberScheduled": 3,
    "numberAvailable": 3,
    "numberMisscheduled": 0,
    "numberReady": 3,
    "observedGeneration": 1,
    "updatedNumberScheduled": 3
  }
}
JSON
)

# the three nodes as they were captured: none cordoned, none carrying any taint
# but the control-plane's own valueless NoSchedule, all Ready. The whole of what
# N1, N2, N3 and N6 have to work with today, and the negative for all three node
# predicates at once.
obj[nodes_healthy]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "List",
  "items": [
    {
      "apiVersion": "v1",
      "kind": "Node",
      "metadata": {
        "name": "k8rs-control-plane"
      },
      "spec": {
        "taints": [
          {
            "effect": "NoSchedule",
            "key": "node-role.kubernetes.io/control-plane"
          }
        ]
      },
      "status": {
        "conditions": [
          {
            "type": "MemoryPressure",
            "status": "False",
            "reason": "KubeletHasSufficientMemory",
            "lastTransitionTime": "2026-08-11T22:42:49Z"
          },
          {
            "type": "DiskPressure",
            "status": "False",
            "reason": "KubeletHasNoDiskPressure",
            "lastTransitionTime": "2026-08-11T22:42:49Z"
          },
          {
            "type": "PIDPressure",
            "status": "False",
            "reason": "KubeletHasSufficientPID",
            "lastTransitionTime": "2026-08-11T22:42:49Z"
          },
          {
            "type": "Ready",
            "status": "True",
            "reason": "KubeletReady",
            "lastTransitionTime": "2026-08-11T22:43:13Z"
          }
        ]
      }
    },
    {
      "apiVersion": "v1",
      "kind": "Node",
      "metadata": {
        "name": "k8rs-worker"
      },
      "spec": {},
      "status": {
        "conditions": [
          {
            "type": "MemoryPressure",
            "status": "False",
            "reason": "KubeletHasSufficientMemory",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "DiskPressure",
            "status": "False",
            "reason": "KubeletHasNoDiskPressure",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "PIDPressure",
            "status": "False",
            "reason": "KubeletHasSufficientPID",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "Ready",
            "status": "True",
            "reason": "KubeletReady",
            "lastTransitionTime": "2026-08-11T22:43:23Z"
          }
        ]
      }
    },
    {
      "apiVersion": "v1",
      "kind": "Node",
      "metadata": {
        "name": "k8rs-worker2"
      },
      "spec": {},
      "status": {
        "conditions": [
          {
            "type": "MemoryPressure",
            "status": "False",
            "reason": "KubeletHasSufficientMemory",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "DiskPressure",
            "status": "False",
            "reason": "KubeletHasNoDiskPressure",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "PIDPressure",
            "status": "False",
            "reason": "KubeletHasSufficientPID",
            "lastTransitionTime": "2026-08-11T22:43:03Z"
          },
          {
            "type": "Ready",
            "status": "True",
            "reason": "KubeletReady",
            "lastTransitionTime": "2026-08-11T22:43:21Z"
          }
        ]
      }
    }
  ]
}
JSON
)

# broken-hostpath a third time, and this one keeps `spec.volumes` and the
# `volumeMounts` beside them — the copy above was trimmed to the volumes alone,
# which was everything the predicate read before this box. The projected
# service-account mount is left in on purpose: it is `readOnly: true` on every
# pod in Kubernetes, so a predicate that looks for a read-only mount without
# first asking which volume it is on passes on every pod there is.
obj[hostpath_spec]=$(cat <<'JSON'
{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "broken-hostpath",
    "namespace": "default"
  },
  "spec": {
    "containers": [
      {
        "name": "nosy",
        "volumeMounts": [
          {
            "mountPath": "/host",
            "name": "root"
          },
          {
            "mountPath": "/var/run/secrets/kubernetes.io/serviceaccount",
            "name": "kube-api-access-xkdq7",
            "readOnly": true
          }
        ]
      }
    ],
    "volumes": [
      {
        "hostPath": {
          "path": "/",
          "type": ""
        },
        "name": "root"
      },
      {
        "name": "kube-api-access-xkdq7",
        "projected": {
          "defaultMode": 420,
          "sources": [
            {
              "serviceAccountToken": {
                "expirationSeconds": 3607,
                "path": "token"
              }
            },
            {
              "configMap": {
                "items": [
                  {
                    "key": "ca.crt",
                    "path": "ca.crt"
                  }
                ],
                "name": "kube-root-ca.crt"
              }
            },
            {
              "downwardAPI": {
                "items": [
                  {
                    "fieldRef": {
                      "apiVersion": "v1",
                      "fieldPath": "metadata.namespace"
                    },
                    "path": "namespace"
                  }
                ]
              }
            }
          ]
        }
      }
    ]
  },
  "status": {
    "phase": "Running",
    "containerStatuses": [
      {
        "name": "nosy",
        "ready": true,
        "started": true,
        "restartCount": 0
      }
    ]
  }
}
JSON
)

# --- COMPOSED, EACH FROM CAPTURES IN THIS FILE ---
# Six objects the cluster will not hand over as they are: one state it holds for
# about two seconds at a time, two shapes that only arrive under a different
# fetch, and three it does not produce at all here. None is written by hand: each
# is built out of the captures above, changes one coherent group of fields, and
# stays an object the API demonstrably emits (NOTES.md D40, on a shell corpus).

# the bare crashlooper in the List shape [owned] fetches. Shape only — not one
# field of the capture is touched.
obj[crashloop_list]=$(jq -n --argjson p "${obj[crashloop]}" \
  '{apiVersion:"v1", kind:"List", items:[$p]}')

# and the terminated half of the loop in that same List shape, so [owned] is
# proven in both halves rather than in the one it was written against. Shape
# only, again.
obj[crashloop_terminated_list]=$(jq -n --argjson p "${obj[crashloop_terminated]}" \
  '{apiVersion:"v1", kind:"List", items:[$p]}')

# The next two graft one capture's whole status onto another capture's
# identity, and both need the same two corrections or the result is half an
# object rather than one the API emits. The **whole** status moves, never the
# containerStatuses alone: the first draft grafted only that array and left the
# donor pod's conditions saying Ready=True above a container in
# CrashLoopBackOff. The container name follows the pod it now belongs to, and
# the free text is deleted rather than rewritten, because a backoff message
# naming the pod it was captured from is exactly the half-real object D40
# refuses — and a message is optional in the API, where every field the
# predicates read is not.
graft='def graft($status; $container):
   .status = ( $status
               | .containerStatuses |= map( .name = $container
                                            | del(.state.waiting.message)
                                            | del(.state.terminated.message)
                                            | del(.lastState.terminated.message) )
               | .conditions |= map(del(.message)) );'

# a *static* pod in that same crash loop — an etcd that keeps dying after a
# laptop suspend, which is D39's own example. The real mirror pod's identity,
# the real crashlooper's status.
obj[mirror_crashloop]=$(jq -n --argjson m "${obj[mirror]}" --argjson c "${obj[crashloop]}" \
  "$graft"'{apiVersion:"v1", kind:"List", items:[ $m | graft($c.status; "etcd") ]}')

# broken-owned's pod living rule 5's life: three restarts behind it, up and
# ready now. The owner and the crash history without being down, which is the
# difference between "in a crash loop" and "has crashed before". Grafted onto
# the owned pod rather than the other way round — a pod called broken-restarts
# owned by ReplicaSet broken-owned-6576fd8bc8 is a name no ReplicaSet generates,
# and that first draft made it past every assertion in this file.
obj[restarts_owned]=$(jq -n --argjson o "${obj[owned_pods]}" --argjson p "${obj[restarts]}" \
  "$graft"'$o | .items |= map(graft($p.status; "quitter"))')

# the same pod under an ownerReference that owns without controlling. Legal on
# any object and written by plenty of operators, but not a shape broken.yaml can
# produce without becoming a manifest no real workload matches (NOTES.md D40) —
# so it is composed here, where the `controller == true` half of the predicate
# is otherwise a clause nothing can refuse.
obj[owned_not_controlled]=$(jq '.items |= map(.metadata.ownerReferences |= map(.controller = false))' \
  <<<"${obj[owned_pods]}")

# broken-owned at its very first exit: the container has died once and has not
# been restarted yet, so restartCount is still 0 and lastState is still empty.
# This is the object [owned] reads lastState in order to refuse — one exit is
# not yet a loop, and the fixture this box is for is a settled one.
obj[owned_first_exit]=$(jq '.items |= map(.status.containerStatuses |= map(
     .restartCount = 0 | .state = {terminated: .lastState.terminated} | .lastState = {}))' \
  <<<"${obj[owned_pods]}")

# --- COMPOSED, FOR THE MANIFESTS THIS BOX CHANGES ---
# The other side of the four cuts above: what those objects look like once the
# manifests produce the shape they are being changed to produce. Same rules as
# the six compositions above — built from a capture, one coherent group of
# fields moved, and the result still an object the API emits — with one
# difference worth saying out loud: these have no capture *yet* by definition.
# The trip that follows this box is what replaces them, and until it lands they
# are the only positive case each of these predicates has.

# broken-crashloop with the log tail. `terminationMessagePolicy:
# FallbackToLogsOnError` makes the kubelet copy the container's last lines into
# the termination, and the manifest now sets it; the string here is what the
# container prints. Both halves of the loop, and the message deliberately in a
# different place in each — `lastState` while it is in backoff, `state` in the
# seconds it is down — because the predicate reads one or the other and a single
# case would prove only the half it happened to be written against.
obj[crashloop_msg]=$(jq '.status.containerStatuses |= map(.lastState.terminated.message =
     "starting\npanic: dial tcp db.payments.svc:5432: connect: connection refused\n")' \
  <<<"${obj[crashloop]}")

obj[crashloop_terminated_msg]=$(jq '.status.containerStatuses |= map(.state.terminated.message =
     "starting\npanic: dial tcp db.payments.svc:5432: connect: connection refused\n")' \
  <<<"${obj[crashloop_terminated]}")

# broken-pending as the respun manifest declares it: the same unschedulable pod,
# kept off every node by a selector nothing matches instead of by a cpu request
# nothing can satisfy. The scheduler's message goes rather than being rewritten —
# it names the old cause ("Insufficient cpu"), no predicate reads it, and a
# message the API never wrote is the half-real object D40 refuses.
#
# The operator's toleration is *prepended* to the two the admission plugin
# writes, not swapped for them: a pod carrying only one toleration is not a shape
# the API emits, and the difference between this object and the one above is
# precisely the one N6 reads — a key with a value, against two that only say
# `Exists`. `NoExecute` matches the taint `break-nodes` applies, effect included.
obj[pending_selector]=$(jq '.spec.nodeSelector = {"disktype":"ssd","kubernetes.io/os":"linux"}
   | .spec.tolerations = [{"key":"dedicated","operator":"Equal","value":"gpu","effect":"NoExecute"}]
       + .spec.tolerations
   | del(.spec.containers[0].resources)
   | .status.conditions |= map(del(.message))' <<<"${obj[pending]}")

# the same pod with the selector and *only* the two tolerations the admission
# plugin writes — a pod pinned to a node class and not tolerating anything, which
# is the most ordinary manifest there is. It is the object the toleration clause
# is proven against: without it the clause could be deleted and every case in
# this file would still land the same way, because the pod that has no toleration
# also has no selector and fails on the first clause. (Found by deleting the
# clause and watching nothing go red — the negative was missing, not the clause.)
obj[pending_selector_only]=$(jq '.spec.tolerations |= map(select(.key != "dedicated"))' \
  <<<"${obj[pending_selector]}")

# broken-hostpath with the subPath and the second container. `shipper` is the
# captured container cloned and renamed — the same technique the Rust decode
# test uses for the same shape and for the same reason: every field but the name
# and the two under test came off the cluster. Its status entry comes along,
# because a pod whose spec has two containers and whose status has one is not an
# object any kubelet emits.
obj[hostpath_two]=$(jq '.spec.containers[0].volumeMounts =
       [ {"name":"root","mountPath":"/host/containerd","subPath":"run/containerd"},
         (.spec.containers[0].volumeMounts[] | select(.name != "root")) ]
   | .spec.containers += [ (.spec.containers[0]
       | .name = "shipper"
       | .volumeMounts = [ {"name":"root","mountPath":"/host","readOnly":true},
                           (.volumeMounts[] | select(.name != "root")) ]) ]
   | .status.containerStatuses += [ (.status.containerStatuses[0] | .name = "shipper") ]' \
  <<<"${obj[hostpath_spec]}")

# broken-resize after `kubectl patch --subresource resize`, and this is the one
# object here that was *read off the cluster by hand* rather than reasoned to:
# the spec asks for the whole allocatable memory of the node the pod landed on
# (`k8rs-worker3`, `24277408Ki`) and `status.resources` still holds the 64Mi the
# kubelet enacted, because the node cannot free that much. Values, condition and
# message are the ones a v1.36.1 kubelet wrote on 2026-08-12; the condition is
# added rather than moved, because the pod that carries it does not exist in the
# committed capture yet and this is the object that says what it must hold.
#
# `Deferred` and not `Infeasible`: a request larger than the node's allocatable
# — the 1Pi this file used to carry — is refused at admission and never reaches
# the kubelet, so the *infeasible* condition is not reachable through `break` at
# all. Measured, not assumed: `pods "broken-resize" is forbidden: node didn't
# have enough allocatable resources: memory, requested: 1125899906842624,
# allocatable: 24860065792`.
obj[resize_pending]=$(jq '.metadata.name = "broken-resize"
   | .spec.containers[0].resources = {"limits":{"memory":"24277408Ki"},"requests":{"memory":"24277408Ki"}}
   | .status.conditions += [ {"type":"PodResizePending","status":"True","reason":"Deferred",
       "message":"Node didn'"'"'t have enough resource: memory, requested: 24860065792, used: 136314880, capacity: 24860065792"} ]' \
  <<<"${obj[resize_base]}")

# the kubelet's *other* reason for the same parking, which the predicate still
# accepts and this cluster can no longer produce: a node whose allocatable
# shrinks under a pod that was already resized reaches it, `break` does not.
# Everything else about this object is the one above, so it is exactly the
# second spelling and nothing else.
obj[resize_infeasible]=$(jq '.status.conditions |= map(if .type == "PodResizePending"
     then .reason = "Infeasible" else . end)' <<<"${obj[resize_pending]}")

# the same divergence with nothing saying the kubelet parked it: `spec` has moved
# and `status.resources` has not, and no condition explains why. That is what a
# resize looks like in the seconds *before* the kubelet answers, and it is also
# what a capture of a pod mid-enactment looks like — neither is the fixture,
# because both resolve into agreement.
obj[resize_unparked]=$(jq '.status.conditions |= map(select(.type != "PodResizePending"))' \
  <<<"${obj[resize_pending]}")

# and the parked resize whose condition arrived without a reason. A PodCondition
# may legally carry none, and without it the object cannot say which parking
# this is — "not right now" and "in flight" read identically, and one of those
# two is a fixture that has already stopped being true by the time it is read.
obj[resize_no_reason]=$(jq '.status.conditions |= map(if .type == "PodResizePending"
     then del(.reason) else . end)' <<<"${obj[resize_pending]}")

# the pod that was resized *twice*: once to a size the node could fit, which the
# kubelet enacted, and then to the allocatable, which it parked. Spec and status
# disagree here too — but the enacted number is 128Mi, and broken.yaml declares
# 64Mi and `break` patches once. A capture in this state is a pod something else
# has been done to, and the rule test that reads it would be told the container
# is held at a limit no manifest in this repo asked for.
obj[resize_twice]=$(jq '.status.containerStatuses[0].resources =
     {"limits":{"memory":"128Mi"},"requests":{"memory":"128Mi"}}' <<<"${obj[resize_pending]}")

# the resize put back, which is the first thing a second `break` does: the spec
# returns to the 64Mi broken.yaml declares while the condition the kubelet wrote
# is still on the object. Nothing disagrees any more — a capture taken in this
# window is `broken-resize` before its patch wearing the evidence of the last
# run's.
obj[resize_reverted]=$(jq '.spec.containers[0].resources =
     {"limits":{"memory":"64Mi"},"requests":{"memory":"64Mi"}}' <<<"${obj[resize_pending]}")

# and the one object that reaches the spec clause with nothing in it: D53's
# shape, parked. The container declares only cpu, the pod declares the memory
# limit, and the kubelet copies that limit down into the container's status — so
# the enacted memory is 64Mi with no memory in the container's spec to compare it
# against. `null != "64Mi"` is true in jq, so without its null test this
# predicate certifies broken-podlimit as broken-resize.
obj[resize_podlevel_only]=$(jq '.spec.resources = {"limits":{"memory":"64Mi"},"requests":{"memory":"64Mi"}}
   | .spec.containers[0].resources = {"limits":{"cpu":"2"},"requests":{"cpu":"2"}}
   | .status.containerStatuses[0].resources.limits.cpu = "100m"' <<<"${obj[resize_pending]}")

# and a resize the node *could* fit, enacted: 128Mi, which is small enough that
# the kubelet simply gives it. Spec and status agree again, which is the object
# that would make a predicate reading only the spec pass — a fixture captured a
# second too late, after the resize landed, is not the fixture rule 2 needs.
obj[resize_enacted]=$(jq '.metadata.name = "broken-resize"
   | .spec.containers[0].resources = {"limits":{"memory":"128Mi"},"requests":{"memory":"128Mi"}}
   | .status.containerStatuses[0].resources = {"limits":{"memory":"128Mi"},"requests":{"memory":"128Mi"}}' \
  <<<"${obj[resize_base]}")

# D53's shape, on the same capture: a pod-level memory limit, a container that
# declares only a cpu limit, and a container *status* carrying the memory limit
# the kubelet copied down from the pod. The status keeps the pod's 128Mi because
# that is what `getMemoryLimit` writes when the container has no memory limit of
# its own — the one key an enacted map can hold that the spec never declared.
obj[podlimit_pod]=$(jq '.metadata.name = "broken-podlimit"
   | .spec.resources = {"limits":{"memory":"128Mi"},"requests":{"memory":"128Mi"}}
   | .spec.containers[0].resources = {"limits":{"cpu":"100m"},"requests":{"cpu":"10m"}}
   | .status.containerStatuses[0].resources = {"limits":{"cpu":"100m","memory":"128Mi"},"requests":{"cpu":"10m"}}' \
  <<<"${obj[resize_base]}")

# **The one object in this file that is not cut from or composed out of a
# capture, and it is here because that is exactly what the box it belongs to is
# about:** `tests/fixtures/statefulsets.json` is an empty list, no StatefulSet
# has ever been captured from this cluster, and there is therefore nothing to
# compose from. The four fields the predicate reads are spelled identically on a
# Deployment, so the healthy Deployment capture is the nearest real object and
# its identity is what this keeps; the counters are a StatefulSet stuck halfway
# through replacing its highest ordinal. It is replaced by a real cut on the
# first capture that has one, and nothing else in this file may follow its
# example.
obj[sts_partial]=$(jq '{apiVersion: "apps/v1", kind: "StatefulSet",
     metadata: (.metadata | .name = "broken-sts"),
     spec: {replicas: 2, serviceName: "broken-sts"},
     status: {observedGeneration: 2, replicas: 2, readyReplicas: 1,
              currentReplicas: 1, updatedReplicas: 1, availableReplicas: 1}}' \
  <<<"${obj[healthy_deploy]}")

# and the same StatefulSet once its second revision works — every counter equal,
# which is the state every workload in the capture is already in.
obj[sts_ready]=$(jq '.status = {"observedGeneration":2, "replicas":2, "readyReplicas":2,
     "currentReplicas":2, "updatedReplicas":2, "availableReplicas":2}' <<<"${obj[sts_partial]}")

# the healthy Deployment caught mid-rollout: two pods still serving the old
# revision, one surge pod on a revision that cannot start, and nothing taken
# down to make room for it (maxUnavailable: 0). Five counters, five values —
# which is the point, because with all five equal `desired` and `ready` can be
# read from any of the others. The conditions' messages name the ReplicaSet the
# donor rolled out from, so they go rather than being rewritten.
obj[rollout_deploy]=$(jq '.metadata.name = "broken-rollout"
   | .status.replicas = 3 | .status.readyReplicas = 2 | .status.availableReplicas = 2
   | .status.updatedReplicas = 1 | .status.unavailableReplicas = 1
   | .status.conditions |= map(del(.message))' <<<"${obj[healthy_deploy]}")

# kindnet's counters with a broken image underneath them: the pods are scheduled
# — one per node, which is what `desiredNumberScheduled` means and where a
# DaemonSet differs from everything else — and not one of them is ready.
obj[ds_broken]=$(jq '.metadata.name = "broken-ds" | .metadata.namespace = "default"
   | .status.numberReady = 0 | .status.numberAvailable = 0
   | .status.updatedNumberScheduled = .status.desiredNumberScheduled' <<<"${obj[ds_healthy]}")

# The healthy side. Each is the captured healthy pod with the one field the
# manifest now adds, and the capture itself is the negative for all three.
obj[healthy_init_limits]=$(jq '.spec.initContainers[0].resources =
     {"limits":{"memory":"128Mi"},"requests":{"cpu":"50m","memory":"16Mi"}}' <<<"${obj[healthy_spec]}")

# the native sidecar. `restartPolicy: Always` on an init container is the whole
# of it — the container is otherwise the captured one, and the manifest gives it
# a name of its own because a sidecar that runs beside the workload is not a
# migration that finished.
obj[healthy_sidecar_pod]=$(jq '.spec.initContainers[0] |= (.name = "proxy" | .restartPolicy = "Always")
   | .status.initContainerStatuses[0] |= (.name = "proxy" | .started = true)' <<<"${obj[healthy_spec]}")

# and the same pod thirty seconds earlier: the sidecar is up, the workload
# container has not passed its first readiness probe. A fixture captured here is
# a pod that is not healthy yet, and certifying it as the healthy side of every
# rule is the lie this predicate's ready clause exists to stop.
obj[healthy_sidecar_boot]=$(jq '.status.containerStatuses |= map(.ready = false)' \
  <<<"${obj[healthy_sidecar_pod]}")

# the read-only host mount, which is the shape rule 8 must *not* fire on. The
# volume and the mount are the captured broken pod's, with `readOnly` set — the
# same field the Rust decode test sets on the same object, and the reason the
# posture case belongs on the healthy side is that rule 8 fires on the writable
# one.
obj[healthy_hostpath_pod]=$(jq --argjson hp "${obj[hostpath_spec]}" '
     .metadata.name = "healthy-hostpath"
   | .spec.volumes = $hp.spec.volumes
   | .spec.containers[0].volumeMounts =
       [ ($hp.spec.containers[0].volumeMounts[] | if .name == "root" then .readOnly = true else . end) ]' \
  <<<"${obj[healthy_spec]}")

# the pod that declares its own request (KEP-2837). Both numbers survive on
# purpose: N5 replaces the container sum with the pod-level value rather than
# adding to it, and it needs to be able to see both to do that.
obj[healthy_podlevel_pod]=$(jq '.metadata.name = "healthy-podlevel"
   | .spec.resources = {"requests":{"cpu":"100m","memory":"64Mi"}}' <<<"${obj[healthy_spec]}")

# the same pod with the containers' own requests taken away — a shape the API
# accepts and N5 cannot read a container sum out of. It is the negative that
# holds the second half of the predicate honest.
obj[healthy_podlevel_only]=$(jq 'del(.spec.containers[0].resources.requests)' \
  <<<"${obj[healthy_podlevel_pod]}")

# The nodes, after `cluster.sh break-nodes`. One cordoned, one carrying the taint
# an operator typed, and a third whose kubelet has stopped posting — for which
# the node controller writes Unknown across *every* condition, not only Ready,
# which is why they all move together here. The third node is the captured worker
# under the name kind gives the third one; the cluster this was cut from had two.
#
# **The taints nobody typed are half of this object**, and leaving them out is
# what made the first draft of it a shape the API does not emit. `kubectl cordon`
# writes `spec.unschedulable`; the node controller answers by adding
# `node.kubernetes.io/unschedulable:NoSchedule`. A Ready condition of `Unknown`
# is answered the same way, with `node.kubernetes.io/unreachable` at both
# effects. And `SwapNodeControllerTaint` — the one function in the tree that
# writes `timeAdded` — stamps every taint it adds, which is why the timestamps
# below are on the controller's taints and **not** on `dedicated=gpu`:
# `kubectl taint` writes none at all (k/k #113044).
#
# Only one of those timestamps is *asserted*, and it is the one on a `NoExecute`
# taint, where no reading of upstream disagrees. The stamp on the cordon's
# `NoSchedule` taint is written here because that is what the controller does,
# and no predicate reads it — so if the next capture shows the API server
# dropping it, this line is the one to correct and nothing else moves.
obj[nodes_broken]=$(jq '.items |= (
     map(if .metadata.name == "k8rs-worker"
         then .spec.unschedulable = true
            | .spec.taints = [ {"key":"node.kubernetes.io/unschedulable","effect":"NoSchedule",
                                "timeAdded":"2026-08-11T23:09:41Z"} ]
         elif .metadata.name == "k8rs-worker2"
         then .spec.taints = [ {"key":"dedicated","value":"gpu","effect":"NoExecute"} ]
         else . end)
     + [ (.[] | select(.metadata.name == "k8rs-worker")
          | .metadata.name = "k8rs-worker3"
          | .status.conditions |= map(.status = "Unknown" | .reason = "NodeStatusUnknown"
                                      | .lastTransitionTime = "2026-08-11T23:11:00Z")
          | .spec.taints = [ {"key":"node.kubernetes.io/unreachable","effect":"NoSchedule",
                              "timeAdded":"2026-08-11T23:11:40Z"},
                             {"key":"node.kubernetes.io/unreachable","effect":"NoExecute",
                              "timeAdded":"2026-08-11T23:11:40Z"} ]) ])' \
  <<<"${obj[nodes_healthy]}")

# a node that is cordoned *because* it is dead, which is N1's finding wearing
# N2's field: draining a node that stopped answering is what an operator does
# next, and the cordoned fixture has to be the one that is otherwise fine. It
# carries the controller's taints for both facts, so the only clause it fails is
# the Ready one — a negative that failed for two reasons at once would prove
# neither.
obj[nodes_cordoned_dead]=$(jq '.items |= map(if .metadata.name == "k8rs-worker"
     then .spec.unschedulable = true
        | .spec.taints = [ {"key":"node.kubernetes.io/unschedulable","effect":"NoSchedule",
                            "timeAdded":"2026-08-11T23:09:41Z"},
                           {"key":"node.kubernetes.io/unreachable","effect":"NoSchedule",
                            "timeAdded":"2026-08-11T23:11:40Z"},
                           {"key":"node.kubernetes.io/unreachable","effect":"NoExecute",
                            "timeAdded":"2026-08-11T23:11:40Z"} ]
        | .status.conditions |= map(if .type == "Ready"
            then .status = "Unknown" | .reason = "NodeStatusUnknown" else . end)
     else . end)' <<<"${obj[nodes_healthy]}")

# the cordon in the second before the controller answers it: the field is set and
# the taint is not there yet. It is a real state and a short one, and it is what
# a capture taken too early holds — so it is the case that keeps the taint clause
# from being decoration.
obj[nodes_cordon_untainted]=$(jq '.items |= map(if .metadata.name == "k8rs-worker"
     then .spec.unschedulable = true else . end)' <<<"${obj[nodes_healthy]}")

# the same taint with the effect `kubectl taint` gives you by habit. Same key,
# same value, `NoSchedule` — which schedules nothing new onto the node and
# evicts nothing already on it, so it is not the taint N6's pod is being kept
# off a node by.
obj[nodes_taint_noschedule]=$(jq '.items |= map(if .metadata.name == "k8rs-worker2"
     then .spec.taints = [ {"key":"dedicated","value":"gpu","effect":"NoSchedule"} ]
     else . end)' <<<"${obj[nodes_healthy]}")

# and the same key at the same effect with no value at all — `kubectl taint node
# w dedicated:NoExecute`, which is a taint an operator really does write. N6 has
# to name the taint that is blocking the pod, and `dedicated` alone does not say
# which class of node this is; the value clause is what carries that, so the
# value clause needs a case that fails on it and on nothing else.
obj[nodes_taint_novalue]=$(jq '.items |= map(if .metadata.name == "k8rs-worker2"
     then .spec.taints = [ {"key":"dedicated","effect":"NoExecute"} ]
     else . end)' <<<"${obj[nodes_healthy]}")

# a kubelet that is alive and saying no. `False` with a reason of its own is a
# node the kubelet is still reporting on — a CNI that has not come up, a disk
# that filled — and it is not the "stopped posting" state N1 is about. The
# controller taints this one too, at the key it uses for `False`, so the object
# differs from the positive in the condition and not in whether it was tainted.
obj[nodes_ready_false]=$(jq '.items |= map(if .metadata.name == "k8rs-worker"
     then .status.conditions |= map(if .type == "Ready"
            then .status = "False" | .reason = "KubeletNotReady" else . end)
        | .spec.taints = [ {"key":"node.kubernetes.io/not-ready","effect":"NoSchedule",
                            "timeAdded":"2026-08-11T23:11:40Z"},
                           {"key":"node.kubernetes.io/not-ready","effect":"NoExecute",
                            "timeAdded":"2026-08-11T23:11:40Z"} ]
     else . end)' <<<"${obj[nodes_healthy]}")

# the kubelet has stopped posting and the controller has not answered yet: every
# condition is Unknown and no taint has been added. Forty seconds of every real
# outage look like this, and a capture taken in them holds no `timeAdded` at all
# — which is the whole reason the timestamp is read off the controller's taint
# and not off the operator's.
obj[nodes_unreachable_untainted]=$(jq '.items |= map(if .metadata.name == "k8rs-worker"
     then .status.conditions |= map(.status = "Unknown" | .reason = "NodeStatusUnknown")
     else . end)' <<<"${obj[nodes_healthy]}")

# and the same node with the unreachable taint typed in by hand. `kubectl taint`
# writes no `timeAdded` — that is k/k #113044, and it is the bug that made the
# taint predicate wait out the full 420s and then fail a taint that had been
# applied correctly. It is here so that the clause which replaced it is held down
# by the exact object that broke the one before.
obj[nodes_unreachable_hand_tainted]=$(jq '.items |= map(if .metadata.name == "k8rs-worker"
     then .spec.taints = [ {"key":"node.kubernetes.io/unreachable","effect":"NoExecute"} ]
     else . end)' <<<"${obj[nodes_unreachable_untainted]}")

# --- CORPUS END ---

# --- ASSERTIONS START ---
fail=0
declare -A covered=()

check() { # $1 predicate  $2 match|miss  $3 object  $4 what that object is
  local key=$1 expect=$2 doc=$3 label=$4 rc=0
  covered[$key]="${covered[$key]:-} $expect"
  [ -n "${obj[$doc]:-}" ] || { echo "FAIL  [$key] there is no corpus object called '$doc'"; fail=1; return; }
  jq -e "${want[$key]}" >/dev/null 2>&1 <<<"${obj[$doc]}" || rc=$?
  case "$expect:$rc" in
    match:0|miss:1) ;;
    match:1) echo "FAIL  [$key] did not match $label"; fail=1 ;;
    miss:0)  echo "FAIL  [$key] matched $label — a predicate that matches this cannot fail"; fail=1 ;;
    # jq exits non-zero for a broken filter too (5 for a runtime error, 4 for no
    # output), and verify() reads any non-zero as "not there yet" — so a typo
    # costs the full timeout and then blames the fixture. Never let that pass.
    *) echo "FAIL  [$key] jq could not evaluate the predicate against $label (exit $rc)"; fail=1 ;;
  esac
}
# --- ASSERTIONS END ---

# Rule 2. The kill lands in .lastState once the container is back in backoff;
# .state.terminated only holds it in the seconds before that, which is why the
# predicate reads both. The exit code is what separates it from any other crash.
check oom       match oom       "broken-oom, settled"
check oom       miss  healthy   "the healthy pod"
check oom       miss  crashloop "a pod in the same CrashLoopBackOff, killed by exit 1 instead"

# Rules 1+6. Same waiting reason as the OOM pod, different exit code — and two
# positives, because a crash loop is two states and this predicate used to name
# only one of them. The capture can land in either.
check crashloop match crashloop_msg "broken-crashloop, in backoff, with the log tail in lastState"
check crashloop match crashloop_terminated_msg "the same crash between backoffs — dead, not yet waiting, and the tail is in state"
check crashloop miss  crashloop "broken-crashloop exactly as it is captured today: no message anywhere, which is what a trip that forgot terminationMessagePolicy brings back"
check crashloop miss  crashloop_terminated "…and the same omission in the other half of the loop"
check crashloop miss  healthy   "the healthy pod"
check crashloop miss  oom       "a pod in the same CrashLoopBackOff, killed by the kernel instead"
check crashloop miss  init      "broken-init, whose app container waits at PodInitializing"
check crashloop miss  restarts  "a pod with the same crash history that is up and ready now — rule 5's pod"

# Rule 3. Both spellings count: the first pull failure is ErrImagePull and every
# one after it is ImagePullBackOff, so a predicate with only one of them passes
# or fails depending on when it was asked.
check image     match image     "broken-image"
check image     miss  healthy   "the healthy pod"
check image     miss  config    "a container waiting for a different reason"

# Rule 4.
check config    match config    "broken-config"
check config    miss  healthy   "the healthy pod"
check config    miss  image     "a container waiting for a different reason"

# Rule 10. Reads a condition, not a container: an unschedulable pod has no
# containerStatuses array at all.
check pending   match pending_selector "broken-pending as the respun manifest declares it — a selector nothing matches"
check pending   miss  pending   "broken-pending as it is captured today: Pending for a cpu request, with nothing on the pod to say so"
check pending   miss  pending_selector_only "the same pod carrying only the two tolerations every pod gets — a selector is not a toleration"
check pending   miss  healthy   "the healthy pod"
check pending   miss  init      "broken-init — also phase Pending, but scheduled and running"

# Rule 8. The negative is not a pod without volumes: every pod has the projected
# service-account volume, so the predicate has to pick hostPath out of a list
# that is never empty.
check hostpath  match hostpath_two "broken-hostpath with the subPath and the second container"
check hostpath  miss  hostpath_spec "broken-hostpath as it is captured today — one writable mount, no subPath, one container"
check hostpath  miss  hostpath  "the copy this file first cut, whose mounts were trimmed away entirely"
check hostpath  miss  healthy   "the healthy pod, which still has a projected token volume"

# Rule 7 — running, and failing its readiness probe. A crashlooping container is
# also Running-and-not-ready, and it is not this: it is not running at all.
check readiness match readiness "broken-readiness"
check readiness miss  healthy   "the healthy pod"
check readiness miss  crashloop "a pod that is unready because it keeps crashing, not because a probe fails"
check readiness miss  oom       "a pod that is unready because it keeps being killed"

# Rule 5 — the pod that looks fine now. Ready and running, and it got there the
# hard way. The interesting negative is the crashlooper: it has the restarts but
# it is not up.
check restarts  match restarts  "broken-restarts"
check restarts  miss  healthy   "the healthy pod, which has never restarted"
check restarts  miss  crashloop "a pod with the restarts but still down"

# Rule 9. A container with no resources block reports `resources: {}`, not a
# missing key.
check nolimits  match nolimits  "broken-nolimits"
check nolimits  miss  healthy   "the healthy pod, which sets limits"

# Rule 12's precondition. verify() runs before the delete that makes the pod
# Terminating, so what it can assert here is that the finalizer is attached;
# `just fixtures` asserts deletionTimestamp after its own delete.
check stuck     match stuck     "broken-stuck, before the delete"
check stuck     miss  healthy   "the healthy pod, which carries no finalizer"

# D27. The init container is a separate array — a pod stuck at
# Init:CrashLoopBackOff says nothing useful in containerStatuses, so a predicate
# that reads only that array is blind to it.
check init      match init      "broken-init"
check init      miss  healthy   "the healthy pod, whose init container completed with exit 0"
check init      miss  crashloop "a pod whose *app* container crashloops and which has no init container"

# W1. No pod exists at all, so the evidence is a condition on the ReplicaSet.
check quota     match quota_rs  "the quota-denied ReplicaSet"
check quota     miss  healthy_rs "a ReplicaSet that rolled out cleanly (no conditions key at all)"
check quota     miss  empty_rs  "an empty namespace — 'no ReplicaSet' is not 'the ReplicaSet failed'"

# W2. Progressing flips to False only once the deadline is blown; while the
# rollout is merely slow it is True/ReplicaSetUpdated, which must not count.
check w2        match w2_deploy "the Deployment that gave up"
check w2        miss  healthy_deploy "a Deployment that finished rolling out"

# D36. The one broken pod that has an owner, and this predicate has four clauses
# rather than the usual two — so it gets one negative per clause, each an object
# that differs from the positive in exactly that clause. Every case is a List:
# the pod's name is generated, so the fetch is by label, and a bare Pod object
# would make jq error out instead of answer.
check owned     match owned_pods "broken-owned's pod, in backoff behind its ReplicaSet"
check owned     match crashloop_terminated_list "the same pod between backoffs, which is where the loop spends most of its time"
check owned     miss  crashloop_list "the same crash with no owner at all"
check owned     miss  coredns_pods "a healthy pod owned by a ReplicaSet — the owner without the crash"
check owned     miss  mirror_crashloop "the same crash under an owner of kind Node, which is a mirror pod and not a workload"
check owned     miss  owned_not_controlled "an ownerReference that owns without controlling"
check owned     miss  restarts_owned "an owned pod that crashed three times and is up now — history is not a loop"
check owned     miss  owned_first_exit "the same pod at its first exit, before any restart"
check owned     miss  empty_rs "an empty List — 'no pod' is not 'a crashlooping pod'"

# D51's resize. Both sides of the divergence are asserted because either alone
# passes on a pod nobody ever resized: the kubelet is still holding what it
# enacted, and the spec has moved off it. The third clause is *why* it was not
# enacted, and its negatives are the two objects that carry the divergence
# without an answer — no condition at all, and a condition with no reason on it.
check resize    match resize_pending "broken-resize parked as Deferred, which is what the cluster actually produces"
check resize    match resize_infeasible "the same parking spelled Infeasible — reachable on a node that shrank, not through break"
check resize    miss  resize_base    "the same pod before the patch — spec and status agree"
check resize    miss  resize_enacted "a resize the node *could* fit, so there is nothing left to disagree about"
check resize    miss  resize_unparked "the same divergence with no resize condition at all — a resize nobody has answered yet"
check resize    miss  resize_no_reason "the condition with its reason missing, which cannot say parked from in flight"
check resize    miss  resize_twice   "a pod resized twice — the divergence is there, but the kubelet is holding 128Mi and no manifest here asks for that"
check resize    miss  resize_reverted "the spec put back to 64Mi with the old condition still on it, which is what a second break leaves for a moment"
check resize    miss  resize_podlevel_only "D53's pod: a memory limit enacted from the pod level that the container's own spec never declared"
check resize    miss  oom            "a pod nobody resized, captured before status.resources was read at all"

# D53's copy-down: a memory limit in the container status that its own spec
# never declared. The negative is the ordinary pod, where the status is only
# repeating what the container asked for.
check podlimit  match podlimit_pod   "broken-podlimit — pod-level memory, container-level cpu"
check podlimit  miss  resize_base    "a container that declares its own memory limit"
check podlimit  miss  healthy        "a pod with no pod-level resources at all"

# D40, the three workload kinds. Each negative is the same kind fully rolled
# out, because "ready is fewer than desired" is the only thing that separates
# these fixtures from every workload already captured.
check sts       match sts_partial    "broken-sts stuck replacing its highest ordinal"
check sts       miss  sts_ready      "the same StatefulSet once the second revision works"
check sts       miss  healthy_deploy "a fully rolled-out workload with the same field names"

check rollout   match rollout_deploy "broken-rollout mid-rollout: 2 wanted, 3 existing, 2 ready, 1 updated, 1 unavailable"
check rollout   miss  healthy_deploy "the same Deployment before the second revision"
check rollout   miss  w2_deploy      "a rollout that gave up with nothing ready — not the same thing as partially ready"

check ds        match ds_broken      "broken-ds: scheduled everywhere, ready nowhere"
check ds        miss  ds_healthy     "kindnet, whose pods all started"

# The healthy side, which had no predicate at all before this box. The captured
# healthy pod is the negative for all four: it is what a trip that forgot the
# manifest change brings back.
check healthy_init     match healthy_init_limits "the healthy pod with resources on its init container"
check healthy_init     miss  healthy_spec        "the same pod as captured, whose init container declares an empty resources block"
check healthy_init     miss  healthy             "the copy this file first cut, with no initContainers in the spec at all"

check healthy_sidecar  match healthy_sidecar_pod  "healthy-sidecar: an init container that keeps running"
check healthy_sidecar  miss  healthy_spec         "an init container that runs to completion, which is not a sidecar"
check healthy_sidecar  miss  healthy_sidecar_boot "the same pod before its workload container is ready — not yet the healthy fixture"

check healthy_hostpath match healthy_hostpath_pod "healthy-hostpath: the node's log directory, read-only"
check healthy_hostpath miss  hostpath_spec        "the writable mount of the same volume — the pod rule 8 fires on"
check healthy_hostpath miss  healthy_spec         "a pod with no host mount at all: nothing to find is not read-only"

check healthy_podlevel match healthy_podlevel_pod  "healthy-podlevel: a request on the pod and on its container"
check healthy_podlevel miss  healthy_spec          "a pod that declares no request of its own"
check healthy_podlevel miss  healthy_podlevel_only "the pod-level request with no container request left to replace"

# The three node states, each read out of the List `kubectl get nodes` returns —
# which is also the shape nodes.json is captured in. The captured cluster is the
# negative for all three at once, and each has a second one that differs in
# exactly the clause it turns on.
check cordoned  match nodes_broken       "the worker that kubectl cordon was pointed at"
check cordoned  miss  nodes_healthy      "the cluster as captured — nothing cordoned"
check cordoned  miss  nodes_cordoned_dead "a node cordoned because it is dead, which is N1 wearing N2's field"
check cordoned  miss  nodes_cordon_untainted "the same cordon a second earlier, before the controller added its taint"

check tainted   match nodes_broken       "dedicated=gpu:NoExecute — and no timeAdded, because kubectl taint writes none"
check tainted   miss  nodes_healthy      "the control-plane's own taint: a different key, no value, and NoSchedule"
check tainted   miss  nodes_taint_noschedule "the same key and value at NoSchedule — which evicts nothing and blocks nothing already running"
check tainted   miss  nodes_taint_novalue "the same key and effect with no value at all — the key alone does not say which class of node"

check notready  match nodes_broken       "the worker whose kubelet was stopped, with the taint the controller stamped a time on"
check notready  miss  nodes_healthy      "three kubelets all still posting"
check notready  miss  nodes_ready_false  "a kubelet that is alive and saying no — a different finding"
check notready  miss  nodes_unreachable_untainted "the first forty seconds of the same outage: Unknown everywhere, not yet tainted"
check notready  miss  nodes_unreachable_hand_tainted "the same taint typed in by hand, carrying no timeAdded — the object that made the old predicate hang"

# Every predicate cluster.sh runs against the cluster has to be exercised in
# both directions here, and the two loops below are what say so. A count would
# only notice that a predicate had been added; this notices that one was added
# and given a positive case and nothing that must refuse it — which is the
# state a predicate spends its life in when nobody is watching, and it is
# indistinguishable from a working one until a fixture drifts.
for key in "${!want[@]}"; do
  case " ${covered[$key]:-} " in
    *" match "*) ;;
    *) echo "FAIL  [$key] has no case it must match — this predicate has never been seen to pass"; fail=1 ;;
  esac
  case " ${covered[$key]:-} " in
    *" miss "*) ;;
    *) echo "FAIL  [$key] has no case it must refuse — a predicate that only ever matched proves nothing"; fail=1 ;;
  esac
done

if [ $fail -eq 0 ]; then
  echo "verify-test: ${#want[@]} predicates, each matched in its own state and refused in a neighbouring one"
fi
exit $fail
