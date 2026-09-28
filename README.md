# k8rs

**A terminal dashboard that tells you what is broken in a Kubernetes cluster,
in words you do not need a glossary for — and lets you fix it without
memorising the command.**

One binary. Nothing is installed in your cluster. It runs on your machine
against your kubeconfig, and that is the whole trust model.

```
 nodes 4/4                                      k8rs                   ctx: kind-k8rs · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────────────────────────┐
│▸ ALERTS   11 ● 12 ▲│▸ ● default/broken-sigterm                                          12s ago  │
│  RESOURCES         │    The last run on record was stopped, and Kubernetes is restarting it      │
│   workloads        │    (CrashLoopBackOff)                                                       │
│   network          │    container app · 4 restarts · ran for 1s · exit 143 (stopped with         │
│   storage          │    SIGTERM, which is an ordinary shutdown and not an error)                 │
│   config           │    → the container's own log holds a shutdown and not a crash, so check     │
│   cluster          │      the liveness and startup probes, then the pod's events for a resize    │
│  ANALYSIS          │      that restarted it, then the node, where a memory killer such as        │
│   capacity         │      earlyoom sends the same signal                                         │
│   certificates     │                                                                             │
│   drain safety     │  ● default/broken-restarts10                                       35s ago  │
│   posture          │    Container keeps crashing, and each restart waits longer                  │
│   restarts         │    (CrashLoopBackOff)                                                       │
│   waste            │    container flaky · 3 restarts · ran for under a second · exit 1 (the      │
│   versions         │    application's own error)                                                 │
│                    │    → read the last run's log — it holds the last thing written before that  │
│                    │      run ended, from the program or from the shell that started it. The     │
│                    │      command below is what fetches it, using --previous                     │
├────────────────────┴─────────────────────────────────────────────────────────────────────────────┤
│ $ kubectl --context kind-k8rs get statefulsets -A --watch                                        │
│ $ kubectl --context kind-k8rs get daemonsets -A --watch                                          │
├──────────────────────────────────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  / filter  ? all keys  q quit                                                    │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

*A real frame, captured from the binary against a four-node kind cluster with
the test manifest applied — not a mockup.*

## Why

`kubectl get pods` tells you a pod is in `CrashLoopBackOff`. It does not tell
you that the container was **stopped** rather than crashing, that exit 143 is
an ordinary shutdown signal, or that the place to look next is the liveness
probe. k8rs reads what the cluster already knows and says that part out loud.

Two rules hold everywhere in it:

- **Jargon is explained, never just printed.** `OOMKilled` becomes *container
  exceeded its memory limit*; the original word is kept in brackets so you can
  search for it.
- **Every command k8rs runs is shown, as the `kubectl` you would have typed.**
  The strip along the bottom is that log. k8rs does not execute what it
  prints — it is there so you learn the command, and so you can check the tool.

## What it does today

- **Alerts** — every check k8rs has, over pods, workloads, nodes and your
  kubeconfig's own certificate, live: a crashlooping container, a pod the
  scheduler could not place (quoting the scheduler's own reason), an image that
  will not pull, a missing ConfigMap or Secret, a container killed for its
  memory limit, a pod running but out of its Service, a certificate about to
  expire, and the rest.
- **Analysis** — seven whole-cluster reports: capacity, certificates, drain
  safety, posture, restarts, waste, versions.
- **Fixing things** — `r` restarts a workload, `ctrl-d` deletes an object. Each
  one asks first, in a box that says what will happen in plain language, and
  each one is written down. See *[What k8rs can change](#what-k8rs-can-change-in-your-cluster)*.
- **`k8rs --once`** — connect, print one report, exit `0` if it ran and `2` if
  it could not. For a terminal or a CI job; add `--analysis` for the seven
  reports.
- **`--read-only`** — every mutating key is unreachable, not merely unbound,
  and such a run opens no audit log at all, because a run that can change
  nothing owes no record.

**Not wired yet:** the **Resources** browser lists what your cluster serves,
but opening a kind lands on a pane that stays empty — the row is drawn, the
fetch behind it is not connected. The four detail tabs behind a card are the
same. Both arrive in v0.2.

## Install

k8rs is not on crates.io yet — the name is reserved by a placeholder with no
code in it, so `cargo install k8rs` today gets you nothing. Until the first
release, build it:

```sh
git clone https://github.com/murat-akpinar/k8rs
cd k8rs
cargo install --path .
```

Rust 1.98.1 or newer. No other dependency, and nothing to deploy.

## Running it

```sh
k8rs                            # the current kubeconfig context
k8rs --context prod-eu          # a context by name
k8rs --namespace payments       # one namespace (also -n)
k8rs --read-only                # nothing can be changed, structurally
k8rs --once [--analysis]        # one report on stdout, then exit
```

With more than one context and no `--context`, k8rs asks which cluster before
it connects. `?` shows every key. `q` quits.

## What k8rs can change in your cluster

Three operations exist in the code: **scale**, **restart** and **delete**. On
this build the console offers **restart** and **delete**; `s scale` is withheld
from every kind, on every login — a fact about the build, not about your
permissions.

Every one of them, without exception:

1. An object you selected — there is no bulk anything, and no operation
   without a selection.
2. A keypress.
3. A confirmation box that states the consequence in plain language, and shows
   the `kubectl` line it is equivalent to.
4. A **server-side dry run** where the operation takes one, so the cluster
   checks the change before it is made. `delete` is the one that declines it,
   and says so in its own box and in the audit log.
5. An **audit line** — every attempt, including the ones you refuse, in
   `~/.local/state/k8rs/audit.log` (mode `0600`, append-only). It records both
   the `kubectl` line and the real API call, because k8rs calls the API
   directly and the two must not be confused.

**Delete additionally makes you type the object's name.** The confirm button
stays dead until it matches.

If the object changes or disappears between the confirmation and the send,
k8rs notices and sends nothing — a dry run that passed a moment ago is not a
promise about now.

## Permissions

Two roles, so the mode you run in is enforced by the cluster and not only by
the tool. Both are copied from
[docs/security.md](docs/security.md#rbac), where every rule carries the reason
it exists and the measurement behind it.

**Read-only** — everything Alerts and Analysis need. Run under this and there
is nothing to trust k8rs about:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-readonly
rules:
  - nonResourceURLs: ["/api", "/apis", "/api/*", "/apis/*", "/version"]
    verbs: ["get"]
  - apiGroups: [""]
    resources: ["pods", "pods/log", "events", "services", "nodes",
                "persistentvolumeclaims"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets", "replicasets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["policy"]
    resources: ["poddisruptionbudgets"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["certificates.k8s.io"]
    resources: ["certificatesigningrequests"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["discovery.k8s.io"]
    resources: ["endpointslices"]
    verbs: ["get", "list", "watch"]
  - apiGroups: ["metrics.k8s.io"]
    resources: ["nodes"]
    verbs: ["get", "list"]
  - apiGroups: ["authorization.k8s.io"]
    resources: ["selfsubjectrulesreviews", "selfsubjectaccessreviews"]
    verbs: ["create"]
```

**Operations** — what the write path needs, on top of the read-only role. This
is **exactly what this build can do and nothing more**: scale, rollout restart
and delete. Cordon, drain and edit arrive in v0.2 and v0.4 and bring their own
verbs back with them — `pods/eviction`, `patch` on nodes and `update` on the
workload kinds were in this block until 2026-09-28, granting cluster-wide evict
and cluster-wide workload replace for features nobody could invoke.

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: k8rs-admin
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["delete"]
  - apiGroups: [""]
    resources: ["nodes"]
    verbs: ["delete"]
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets"]
    verbs: ["get", "patch", "delete"]           # rollout restart, delete
  - apiGroups: ["apps"]
    resources: ["replicasets"]
    verbs: ["delete"]
  - apiGroups: ["apps"]
    resources:
      ["deployments/scale", "statefulsets/scale", "replicasets/scale"]
    verbs: ["get", "patch"]          # scale
  - apiGroups: ["authorization.k8s.io"]
    resources: ["selfsubjectrulesreviews", "selfsubjectaccessreviews"]
    verbs: ["create"]
```

A refusal is never a crash and never a retry loop: k8rs names the verb and the
resource the role is missing, and the feature that needed it degrades on its
own. Where the cluster can answer *what may this login do*, keys you cannot use
are marked `no` **before** you press them.

## No telemetry

Nothing leaves your machine. The only connection k8rs opens is to the API
server your kubeconfig names. There is no analytics, no crash reporting, no
update check, and no configuration that could turn one on.

Your credentials are read from the kubeconfig current context and nowhere
else — there is no in-cluster ServiceAccount path in the code. TLS
verification is never disabled by k8rs; if your kubeconfig sets
`insecure-skip-tls-verify`, that is honoured **and said in the header**.

## What it is not

- **Not a dashboard you deploy.** Nothing is installed in the cluster, and
  there is no server component.
- **Not an admission controller.** The posture rows tell you about a mount that
  hands over the node; they do not stop anyone from creating one.
- **Not a YAML editor** — not in v0.1. `edit` arrives in v0.4, with the
  diff and the dry run that belong to it.

## Documentation

| | |
|---|---|
| How it is built, and the data flow | [docs/architecture.md](docs/architecture.md) |
| The security model, the RBAC reasoning, the audit log | [docs/security.md](docs/security.md) |
| Crates, versions, build targets | [docs/tech-stack.md](docs/tech-stack.md) |
| Every screen, key by key | [screens/](screens/) |
| Why each choice was made, numbered `D1…` | [NOTES.md](NOTES.md) |

Türkçe: [README_TR.md](README_TR.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
