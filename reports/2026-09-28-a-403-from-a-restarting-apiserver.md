# What a restarting kube-apiserver answers, and what a refused identity answers beside it

`k8s-admin`, 2026-09-28, for the `unreadable`/`nothing_answering` box
(NOTES § D285 ruling 4, `screens/states.md` § Refused by a cluster that is not
answering anything else). Read against
`reports/2026-09-26-the-error-state-fix-review.md` § 2, which saw the frame
through the binary and could not see the wire under it.

Ephemeral single-node cluster, `kind create cluster --name review`, its own
kubeconfig in the session scratchpad so `~/.kube/config` was never touched.
Server `v1.37.0`, node image `kindest/node:v1.37.0`. Deleted at the end of the
run. The binary was not built or run here (D267); everything below is `curl` and
`kubectl` against the API server.

```
NODE=$(kind get nodes --name review | head -1)
S=$(kubectl config view --minify -o jsonpath='{.clusters[0].cluster.server}')   # https://127.0.0.1:<port>
```

## 1. Two `docker restart` runs, polled for the refusal window

A loop, one iteration per ~110 ms in the second run, each iteration doing the
LIST the initial watch does, plus `/readyz?verbose` unauthenticated:

```
curl -sS -o b1 -w '%{http_code}' --cacert ca.crt --cert admin.crt --key admin.key \
     --max-time 2 "$S/api/v1/namespaces/default/pods?limit=1"
curl -sS -o b3 -w '%{http_code}' --cacert ca.crt --max-time 2 "$S/readyz?verbose"
```

`docker restart "$NODE"`, then the samples around the window (run 2, cadence
~110 ms; `000` is curl's no-connection code):

```
00:29:53.28 list=000 readyz=000
00:29:53.39 list=000 readyz=000
00:29:53.50 list=000 readyz=000
00:29:53.62 list=403 readyz=500 reason=Forbidden | 403 | pods is forbidden: User "kubernetes-admin" cannot list resource "pods" in API group "" in the namespace "default"
   readyz-not-ok: [-]poststarthook/start-apiextensions-controllers failed [-]poststarthook/crd-informer-synced failed [-]poststarthook/start-kube-apiserver-identity-lease-controller failed [-]poststarthook/start-service-ip-repair-controllers failed
(next sample, 00:29:53.73, list=200)
```

Field values the finding turns on:

| field | value in the window |
|---|---|
| `Status.code` | `403` |
| `Status.reason` | `Forbidden` |
| `Status.message` | `pods is forbidden: User "kubernetes-admin" cannot list resource "pods" in API group "" in the namespace "default"` |
| `Status.details` | `{"kind": "pods"}` |
| `/readyz` | `500`, with four `[-]poststarthook/... failed` lines |

**Width of the window:** one sample in each of two restarts. Run 1 polled at
~1.4 s and caught one; run 2 polled at ~110 ms and caught one, with connection
refused on the sample before and `200` on the sample after. So on this cluster
the refusal lasted **under ~110 ms**, and it began at the first moment the
listener accepted a connection at all. Nothing here measures how that scales
with the number of `Role`/`ClusterRoleBinding` objects an informer has to sync.

## 2. What else answered in the same instant

Run 1 sampled six endpoints per iteration under two identities. The one
iteration that caught the window:

```
00:20:48.07 adminlist=403 scopedlist=200 version=200 readyz=500 apis=200 scopedwatch=200
```

`scopedlist` and `scopedwatch` are a ServiceAccount token whose `Role` grants
`list`/`watch` on pods in one namespace; they ran ~200 ms after `adminlist` in
the same iteration. `version`, `apis` and `readyz` are that same identity.

## 3. The same wire shape without a restart: an identity that may read nothing

ServiceAccount `nobody`, no `Role` and no binding, token from
`kubectl create token nobody`:

```
/api/v1/pods?limit=1              -> 403
/api/v1/nodes?limit=1             -> 403
/apis/apps/v1/deployments?limit=1 -> 403
/version                          -> 200
/apis                             -> 200
/readyz                           -> 200
```

`Status` for the pods refusal: `reason=Forbidden`, `code=403`,
`message=pods is forbidden: User "system:serviceaccount:default:nobody" cannot list resource "pods" in API group "" at the cluster scope`,
`details={"kind": "pods"}`.

`POST /apis/authorization.k8s.io/v1/selfsubjectrulesreviews` as that same
identity: `http=201`, `status.incomplete=false`, and the returned
`nonResourceRules` include `get` on `/healthz`, `/livez`, `/readyz`, `/version`
— measured, for an identity with no `Role` at all.

## 4. The pinned kube source, for a `403` whose body is not a `Status`

`kube-client-4.2.0/src/client/mod.rs` `handle_api_errors`:

```
let status = Status::failure(&text, "Failed to parse error data").with_code(status.as_u16());
```

`Status::with_code` sets `code` (`kube-core-4.2.0/src/response.rs:91-94`).

## 5. The namespaced-`Role` refusal, for comparison

Same scoped token as § 2:

```
kubectl get nodes
Error from server (Forbidden): nodes is forbidden: User "system:serviceaccount:default:scoped" cannot list resource "nodes" in API group "" at the cluster scope
kubectl get pods
No resources found in default namespace.
```

## Teardown

```
kind delete cluster --name review
```

The PM's fixture cluster `k8rs` was up and idle throughout (no capture in
flight, 19 GiB free); this cluster ran beside it for ~12 minutes under the name
`review` and was deleted before this report was filed.
