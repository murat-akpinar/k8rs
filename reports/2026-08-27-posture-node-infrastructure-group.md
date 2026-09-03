# 2026-08-27 — Posture's "node infrastructure" group, measured

Operator review (`CLAUDE.md` § The cycle, step 6) of the working-tree change to
`src/analysis.rs` § THE POSTURE REPORT — the outsider group, the sort key and the
opening paragraph.

**No cluster was created.** The PM's fixture cluster `k8rs` was already up (four
node containers, 28 h, idle — no capture in flight, checked with `ps`), and
`CLAUDE.md` § The one hard rule of concurrency allows one cluster at a time, so a
second one under `K8RS_CLUSTER=review` would have run beside it. The two cluster
questions below were answered by **read-only** commands against the cluster that
was already running (`ls`, one `kubectl get`) and by reading the `kind` binary;
nothing was created, changed or captured. Everything else was run against
committed fixtures and against two variants derived from them in the scratchpad,
never committed.

---

## 1. What is in `/etc/kubernetes/pki` on a control-plane node and on a worker

kind v0.32.0, `kindest/node:v1.36.1`, kubeadm-provisioned.

```
$ docker exec <control-plane node> ls -1 /etc/kubernetes/pki
apiserver-etcd-client.crt
apiserver-etcd-client.key
apiserver-kubelet-client.crt
apiserver-kubelet-client.key
apiserver.crt
apiserver.key
ca.crt
ca.key
etcd
front-proxy-ca.crt
front-proxy-ca.key
front-proxy-client.crt
front-proxy-client.key
sa.key
sa.pub

$ docker exec <worker node> ls -1 /etc/kubernetes/pki
ca.crt
```

Filenames only; no file was opened and no key material was read.

## 2. Whether an ordinary pod can be scheduled where those files are

On the running four-node cluster:

```
$ kubectl get nodes -o jsonpath='{range .items[*]}{.metadata.name}{"  taints="}{.spec.taints}{"\n"}{end}'
<control-plane>  taints=[{"effect":"NoSchedule","key":"node-role.kubernetes.io/control-plane"}]
<worker 1>       taints=[{"effect":"NoSchedule","key":"node.kubernetes.io/unschedulable","timeAdded":"2026-08-22T16:20:17Z"}]
<worker 2>       taints=[{"effect":"NoExecute","key":"dedicated","value":"gpu"}]
<worker 3>       taints=
```

(workers 1 and 2 carry `break-nodes`' cordon and taint.)

The single-node case was **not** measured — it needs a second cluster. What the
`kind` binary carries, which is evidence and not a measurement:

```
$ kind version
kind v0.32.0 go1.26.5-X:nodwarf5 linux/amd64
$ strings /usr/bin/kind | grep -o "failed to remove control plane taint"
failed to remove control plane taint
$ strings /usr/bin/kind | grep -o "node-role.kubernetes.io/control-plane-"
node-role.kubernetes.io/control-plane-
```

The trailing `-` is `kubectl taint`'s removal suffix. The command a reviewer with
a free cluster would run to settle it:

```
K8RS_CLUSTER=review kind create cluster --name review --image kindest/node:v1.36.1
kubectl --context kind-review get nodes -o jsonpath='{.items[*].spec.taints}'
kind delete cluster --name review
```

---

## 3. The pane, in the three states the change distinguishes

Built at the working tree: `cargo build --release`.

**a. Every row is node infrastructure** — the committed corpus, unchanged output:

```
$ ./target/release/k8rs --analysis tests/fixtures/kube-system-pods.json tests/fixtures/nodes.json
[posture]
  Pods that can read the node's own filesystem
  Nothing here is broken. Network, storage and metrics agents are supposed to do this — the list says who can, not what to go and fix.
  ○ /lib/modules
      Read-only, mounted by 8 pods in kube-system.
  ○ /run/xtables.lock
      Mounted by 8 pods in kube-system, and at least one of them can write to it. Kubernetes runs its own node agents this way.
  …
  ○ /etc/kubernetes/pki
      Read-only, mounted by 2 pods in kube-system.
```

**b. The box's own scenario** — a `default` pod reading `/etc/kubernetes/pki`
read-only. The plant is `tests/fixtures/healthy-hostpath.json` with one field
changed (`spec.volumes[0].hostPath.path`), written to the scratchpad:

```
$ jq '(.spec.volumes[0].hostPath.path) = "/etc/kubernetes/pki"' tests/fixtures/healthy-hostpath.json > $SCRATCH/pki-reader.json
$ ./target/release/k8rs --analysis tests/fixtures/kube-system-pods.json tests/fixtures/nodes.json $SCRATCH/pki-reader.json
[posture]
  Pods that can read the node's own filesystem
  Network, storage and metrics agents are supposed to read the node's own filesystem. What's at the top of this list is not one of them. Nothing is marked broken; it still says who can, not what to go and fix.
  ○ /etc/kubernetes/pki
      Read-only, mounted by 3 pods in default and kube-system, and at least one of them is not one of the node's own agents.
  ○ /lib/modules
      Read-only, mounted by 8 pods in kube-system.
  …
```

**c. The CNI outside `kube-system`** — NOTES § D70's recorded limit. The plant is
`tests/fixtures/kube-system-pods.json` with one field changed on the four kindnet
pods (`metadata.namespace`), written to the scratchpad; nothing else moved:

```
$ jq '(.items[] | select(.metadata.name | startswith("kindnet")) | .metadata.namespace) = "calico-system"' tests/fixtures/kube-system-pods.json > $SCRATCH/cni-outside.json
$ ./target/release/k8rs --analysis $SCRATCH/cni-outside.json tests/fixtures/nodes.json
[posture]
  Pods that can read the node's own filesystem
  Network, storage and metrics agents are supposed to read the node's own filesystem. What's at the top of this list is not one of them. Nothing is marked broken; it still says who can, not what to go and fix.
  ○ /lib/modules
      Read-only, mounted by 8 pods in calico-system and kube-system, and at least one of them is not one of the node's own agents.
  ○ /run/xtables.lock
      Mounted by 4 pods in kube-system, and at least one of them can write to it. Kubernetes runs its own node agents this way.
  ○ /etc/ca-certificates
      Read-only, mounted by 2 pods in kube-system.
  …
```

`/etc/cni/net.d` and `/var/run/nri` leave the pane in (c) and `/run/xtables.lock`
drops from 8 pods to 4: those mounts are writable, so with the pods outside
`kube-system` rule 8 escalates them to Alerts cards (D70).

---

## 4. Field values the review turned on

| what | where | value |
|---|---|---|
| hostPath volumes on the running cluster, by namespace | `kubectl get pods -A -o json` + `jq`, non-terminal pods, deduplicated | `kube-system`: 13 · `default`: 1 (the demo pod) · none anywhere else |
| kind's own `local-path-provisioner` volumes | `strings /usr/bin/kind` | one `configMap`, no `hostPath` |
| rows on the committed corpus + `healthy-hostpath.json` | pane (b) | 14 |
| `RUNTIME_SOCKETS` | `src/rules.rs:2396` | 5 entries; `/run/containerd/containerd.sock` is one |
| mockup frame width, § Posture, both blocks | every content line | 70 columns, each ending in a space before the border |
