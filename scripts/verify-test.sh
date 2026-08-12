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
eval "$(sed -n '/^  local -A want=(/,/^  )$/p' "$here/cluster.sh" | sed '1s/local -A/declare -A/')"

# "extracted nothing" and "nothing to extract" print the same line, so name what
# has to be there (CLAUDE.md § A derived list asserts it found something).
for canary in oom crashloop image config pending hostpath readiness restarts \
              nolimits stuck init quota w2 owned; do
  [ -n "${want[$canary]:-}" ] || {
    echo "verify-test: cluster.sh verify() has no predicate '$canary' — the extraction broke, not the predicate"
    exit 1
  }
done
[ ${#want[@]} -eq 14 ] || {
  echo "verify-test: cluster.sh verify() has ${#want[@]} predicates, this file covers 14."
  echo "             A new one needs a positive and a negative case here before it can be trusted."
  exit 1
}
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

# broken-pending — never scheduled, so there is no containerStatuses array at all
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

# --- CORPUS END ---

# --- ASSERTIONS START ---
fail=0

check() { # $1 predicate  $2 match|miss  $3 object  $4 what that object is
  local key=$1 expect=$2 doc=$3 label=$4 rc=0
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
check crashloop match crashloop "broken-crashloop, in backoff"
check crashloop match crashloop_terminated "the same crash between backoffs — dead, not yet waiting"
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
check pending   match pending   "broken-pending"
check pending   miss  healthy   "the healthy pod"
check pending   miss  init      "broken-init — also phase Pending, but scheduled and running"

# Rule 8. The negative is not a pod without volumes: every pod has the projected
# service-account volume, so the predicate has to pick hostPath out of a list
# that is never empty.
check hostpath  match hostpath  "broken-hostpath"
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

if [ $fail -eq 0 ]; then
  echo "verify-test: ${#want[@]} predicates, each matched in its own state and refused in a neighbouring one"
fi
exit $fail
