# How long a legitimate `dryRun=All` takes under an admission webhook chain (2026-09-20)

Measured for the operator review of `ops::CHECK_DEADLINE` (NOTES § D273), which
bounds `perform`'s `dryRun=All` at ten seconds and argues the floor from
`ValidatingWebhook::timeout_seconds` being `1..=30`, default 10. The question
this run answers is what a **healthy** cluster spends on that check — a webhook
that does not answer is `failurePolicy: Ignore` throughout, so every run below
that returns `rc=0` is a dry-run the cluster **accepted**.

Ephemeral review cluster, `K8RS_CLUSTER=review`, one node, `kind create cluster
--name review`, torn down in a `trap` (D185). Nothing was written into `tests/`.
The PM's fixture cluster was up and idle beside it (nothing running against it;
19.9 GiB of 23.7 GiB available before, 19.0 GiB after).

Script: `webhook-deadline.sh` / `webhook2.sh` in the run's scratchpad. Versions:
`kubectl v1.36.4`, server `v1.37.0` (kind).

## What the black hole is

Every webhook below points at an address that is dropped inside the node, so the
call hangs at the connect and the apiserver's own `timeoutSeconds` is what ends
it — the *slow admission chain* case, not the *denied* case:

```
$ docker exec review-control-plane sh -c 'iptables -I OUTPUT -d <blackholed-ip> -j DROP && iptables -S OUTPUT | head -2'
-P OUTPUT ACCEPT
-A OUTPUT -d <blackholed-ip>/32 -j DROP
```

Each webhook is `clientConfig.url: "https://<blackholed-ip>:8443/hook"`,
`failurePolicy: Ignore`, `sideEffects: None`, rules
`apps/v1 UPDATE deployments` (or `deployments/scale` where stated).

## The timings

`kubectl patch deployment web --dry-run=server -p '{"spec":{"replicas":N}}'`,
wall clock around the whole `kubectl` process. Baseline shows the client-side
share: ~70–90 ms of every number below is `kubectl` itself.

| case | webhooks | elapsed | rc | what the server said |
|---|---|---|---|---|
| A1 | none | **93 ms** | 0 | `deployment.apps/web patched` |
| A2 | none, `--subresource=scale` | **76 ms** | 0 | `scale.autoscaling/web patched` |
| B1 | 1 validating, `timeoutSeconds: 30` | **30 090 ms** | 0 | `deployment.apps/web patched` |
| B2 | same hook, `--subresource=scale` | **70 ms** | 0 | `scale.autoscaling/web patched` |
| C1 | 1 validating, `timeoutSeconds: 10` (the default) | **10 078 ms** | 0 | `deployment.apps/web patched` |
| C2 | same hook, **real** patch (no `--dry-run`) | **10 082 ms** | 0 | `deployment.apps/web patched` |
| D1 | 3 **mutating**, `timeoutSeconds: 10` each | **30 078 ms** | 0 | `deployment.apps/web patched (no change)` |
| E1 | 3 **validating**, `timeoutSeconds: 10` each | **10 086 ms** | 0 | `deployment.apps/web patched (no change)` |
| F1 | 1 validating scoped to `deployments/scale`, `timeoutSeconds: 30`, `--subresource=scale` | **30 087 ms** | 0 | `scale.autoscaling/web patched (no change)` |
| G1 | 2 mutating (10) + 1 validating (30) | **34 080 ms** | **1** | `Error from server (Timeout): Timeout: request did not complete within requested timeout` |

Read off those rows:

- **A webhook's configured timeout is spent in full and the request still
  succeeds** (B1, C1, F1). The dry-run the cluster accepts arrives at
  `timeoutSeconds`, not before it.
- **Mutating webhooks accumulate, validating ones do not** — three mutating at
  10 s each is 30 s (D1); three validating at 10 s each is 10 s (E1).
- **The apiserver has a ceiling of its own and it is far above ten seconds**: at
  a configured chain worth 50 s it cut the request at **34 s** with
  `Timeout: request did not complete within requested timeout` and `rc=1` (G1).
  That ceiling is a server flag, not something a client can read.
- **A webhook scoped to `deployments` does not fire for `deployments/scale`**
  (B2: 70 ms while B1 on the same hook was 30 s), and one scoped to
  `deployments/scale` does (F1). So `ops::scale`'s `patch_scale` is exposed only
  to hooks that name the subresource; `ops::restart`'s `PATCH` on the object is
  exposed to the whole chain.
- **The real call pays the same admission cost as the check** (C2 vs C1).

## The value the citation in `CHECK_DEADLINE`'s doc rests on

```
$ grep -n -B1 -A1 'timeout_seconds' ~/.cargo/registry/src/*/k8s-openapi-0.28.0/src/v1_32/api/admissionregistration/v1/validating_webhook.rs
79-    /// TimeoutSeconds specifies the timeout for this webhook. ... The timeout value must be between 1 and 30 seconds. Default to 10 seconds.
80:    pub timeout_seconds: Option<i32>,
```

Accurate as cited. It is a doc comment over an `Option<i32>`, i.e. apiserver
validation rather than a type bound; `30` was accepted by the cluster above,
`31` was not tried.

Kyverno's `webhookTimeoutSeconds` documents the same range and the same default
of 10 s
([kyverno.io](https://kyverno.io/docs/policy-types/cluster-policy/policy-settings/)) —
documentation, not measured here.

## Teardown

```
=== CLEANUP (trap) ===
Deleting cluster "review" ...
Deleted nodes: ["review-control-plane"]
clusters left: k8rs
containers left: k8rs-worker3 k8rs-worker k8rs-control-plane k8rs-worker2
```

## What was not measured

- A real policy engine (Kyverno, Gatekeeper) installed and answering — the black
  hole measures the timeout path, which is the path that produces the long
  waits, but not the latency of a webhook that does answer.
- k8rs's own binary against any of this: nothing builds on this machine
  ([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)),
  and `tester` held the test host during this run.
- Whether `timeoutSeconds: 31` is refused by the apiserver.
