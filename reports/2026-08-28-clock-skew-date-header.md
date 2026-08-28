# The `Date` header k8rs reads its clock skew off — measured

`k8s-admin`, 2026-08-28. Operator review of the clock-skew box (`k8s::skew`,
`k8s::measure`, `main::clock`).

No cluster was created or destroyed. The fixture cluster `k8rs` was already
running and was read only — never written, never torn down. Everything below
runs against it or against a locally started proxy in front of it. Loopback
addresses are written `<loopback>` and node names `<node>`; the guard refuses a
dotted quad and `reports/README.md` refuses node identifiers.

Client credentials were read out of the kubeconfig into a scratchpad directory
for `curl`, mode 0600, outside the repo, and are not reproduced here.

## 1. Does a real kube-apiserver put a `Date` on `/version`

```
$ curl -sD- -o/dev/null --cacert ca.crt --cert cl.crt --key cl.key https://<loopback>:6443/version
HTTP/2 200
audit-id: <elided>
cache-control: no-cache, private
content-type: application/json
x-kubernetes-pf-flowschema-uid: <elided>
x-kubernetes-pf-prioritylevel-uid: <elided>
content-length: 379
date: Fri, 28 Aug 2026 12:30:39 GMT
```

Server version `v1.36.1` (kind). The header name arrives lowercase over HTTP/2
and capitalised over HTTP/1.1:

```
$ curl -sD- -o/dev/null --http1.1 ... https://<loopback>:6443/version | grep -i '^date'
Date: Fri, 28 Aug 2026 12:31:02 GMT
```

## 2. Is it the server's clock, or an echo of the request

```
$ date -u -R
Fri, 28 Aug 2026 12:31:02 +0000
$ docker exec <control-plane> date -u -R
Fri, 28 Aug 2026 12:31:02 +0000

$ curl -sD- -o/dev/null --http1.1 -H 'Date: Sun, 06 Nov 1994 08:49:37 GMT' ... /version | grep -i '^date'
Date: Fri, 28 Aug 2026 12:31:02 GMT

$ curl ... /version | grep -i '^date' ; sleep 3 ; curl ... /version | grep -i '^date'
Date: Fri, 28 Aug 2026 12:31:02 GMT
Date: Fri, 28 Aug 2026 12:31:05 GMT
```

A client-supplied `Date` is not echoed; the value advances with real time.

## 3. Through a proxy — copied, or regenerated

Upstream is a local stub on a scratch port; the proxy under test is the real
`kubectl proxy` binary.

```
$ curl -sD- -o/dev/null http://<loopback>:19100/version | grep -i '^date'     # stub, sends a 1994 Date
Date: Sun, 06 Nov 1994 08:49:37 GMT
$ curl -sD- -o/dev/null http://<loopback>:19200/version | grep -i '^date'     # same stub via kubectl proxy
Date: Sun, 06 Nov 1994 08:49:37 GMT

$ curl -sD- -o/dev/null http://<loopback>:19101/version | grep -ic '^date'    # stub sends no Date
0
$ curl -sD- -o/dev/null http://<loopback>:19201/version | grep -i '^date'     # same stub via kubectl proxy
Date: Fri, 28 Aug 2026 12:32:12 GMT
(host now: Fri, 28 Aug 2026 12:32:12 +0000)
```

`kubectl proxy` in front of an API server that is not reachable at all:

```
$ curl -sD- -o/dev/null http://<loopback>:19300/version
HTTP/1.1 500 Internal Server Error
Content-Type: text/plain; charset=utf-8
X-Content-Type-Options: nosniff
Date: Fri, 28 Aug 2026 12:33:58 GMT
Content-Length: 54
(no API server was reachable; host now: Fri, 28 Aug 2026 12:33:58 +0000)
```

## 4. A TLS-terminating middlebox with a fast clock, in front of the real cluster

`mbox.py` (scratchpad, not committed) forwards every request to the real API
server over the kubeconfig's TLS and rewrites one thing: its own `Date`, 30
minutes fast. Host clock and API-server clock are identical, as measured in § 2.

```
$ date -u -R ; curl -sD- -o/dev/null http://<loopback>:19500/version | grep -iE '^(HTTP|date)'
Fri, 28 Aug 2026 12:37:35 +0000
HTTP/1.1 200 OK
Date: Fri, 28 Aug 2026 13:07:35 GMT

$ KUBECONFIG=kc-mbox.yaml k8rs --live | tail -3
13 critical, 3 warnings

Your computer's clock is 29 minutes behind the cluster's, so times are blank rather than guessed.
```

Same single run, the cards above that sentence:

```
$ grep -E '^[●▲] ' skewed.out | grep ' · [0-9]' | head -6
● <pod> · 4 min ago
● <pod> · 5 min ago
● <pod> · 5 days ago
● <pod> · 5 days ago
● <pod> · 5 days ago
▲ <node> · 5 days ago

$ echo "with age: $(grep -cE '^[●▲] .* · [0-9]+ (min|hour|day)' skewed.out)   total: $(grep -cE '^[●▲] ' skewed.out)"
with age: 16   total: 32
```

Offset served was 1800 s; printed magnitude 29.

## 5. Silence on the healthy paths

```
$ k8rs --live                                        # direct, real cluster
stdout lines: 420; lines matching 'clock': 0
13 critical, 3 warnings

$ KUBECONFIG=kc-mbox.yaml k8rs --live                # middlebox, offset 0
stdout lines: 84; lines matching 'clock': 0
13 critical, 3 warnings
```

## 6. A gateway that denies `/version` while everything else reaches the cluster

`mbox403.py` answers `/version` itself with `403` and a `Date` 900 s fast; every
other path is proxied to the real API server untouched.

```
$ date -u -R ; curl -sD- -o/dev/null http://<loopback>:19500/version | grep -iE '^(HTTP|date)'
Fri, 28 Aug 2026 12:43:30 +0000
HTTP/1.1 403 Forbidden
Date: Fri, 28 Aug 2026 12:58:30 GMT

$ KUBECONFIG=kc-mbox.yaml k8rs --live
[stderr] k8rs: watching — could not read the server version (the role this
         kubeconfig uses needs to `get /version`) · 60 kinds · {DisruptionBudgets}
[stdout] Your computer's clock is 14 minutes behind the cluster's, so times are
         blank rather than guessed.
```

`Client::send` (kube-client 4.2.0, `src/client/mod.rs:217-235`) returns
`Ok(Response)` for any status; it does not inspect `status()`.

## 7. RBAC on `/version`

```
$ curl -sD- -o/dev/null --cacert ca.crt https://<loopback>:6443/version | head -1
HTTP/2 200

$ curl -sD- -o/dev/null --cacert ca.crt https://<loopback>:6443/apis | grep -iE '^(HTTP|date)'
HTTP/2 403
date: Fri, 28 Aug 2026 12:33:36 GMT

$ curl -s --cacert ca.crt https://<loopback>:6443/apis | jq -c '{status,reason,code,details}'
{"status":"Failure","reason":"Forbidden","code":403,"details":{}}

$ kubectl get clusterrole system:public-info-viewer -o jsonpath='{.rules}'
[{"nonResourceURLs":["/healthz","/livez","/readyz","/version","/version/"],"verbs":["get"]}]

$ kubectl get clusterrolebinding system:public-info-viewer -o jsonpath='{range .subjects[*]}{.kind}/{.name} {end}'
Group/system:authenticated Group/system:unauthenticated
```

`docs/security.md:101` lists `/version` in the read-only role's
`nonResourceURLs` rule.

## 8. The wire — path and count

`mbox.py` logging every request path over one 20 s `--live` run:

```
1787920908.448 /version
1787920908.455 /version
1787920908.463 /apis
1787920908.469 /api
1787920908.517 /api/v1/pods?&limit=500
1787920908.518 /api/v1/nodes?&limit=500
1787920908.518 /apis/apps/v1/deployments?&limit=500
1787920908.518 /apis/apps/v1/statefulsets?&limit=500
1787920908.519 /apis/apps/v1/daemonsets?&limit=500
1787920908.531 /apis/apps/v1/statefulsets?&watch=true&timeoutSeconds=290&allowWatchBookmarks=true&resourceVersion=<rv>
...

$ grep -c ' /version$' paths.log        # per run
2
$ grep -c '?$' paths.log                # paths ending in a bare '?'
0
```

Gaps: `/version` → `/version` 7.0 ms, `/version` → `/apis` 8.0 ms, `/apis` →
`/api` 6.0 ms. `/version` is requested twice at connect and never again during
the run.

Request cost against the same API server:

```
$ curl ... -w '%{time_total} appconnect=%{time_appconnect} http=%{http_version}\n' <five URLs, one connection>
0.005949 appconnect=0.004880 http=2
0.000433 appconnect=0.000000 http=2
0.000700 appconnect=0.000000 http=2
0.000582 appconnect=0.000000 http=2
0.000561 appconnect=0.000000 http=2

$ for i in 1 2 3; do curl ... ; done      # fresh connection each
0.008176 appconnect=0.007347
0.010011 appconnect=0.009199
0.008299 appconnect=0.006969
```

## 9. What `rules::age` prints while the clock is behind

Real binary, a scratchpad copy of `tests/fixtures/crashloop.json` with every
RFC3339 stamp rewritten (the committed fixture was not touched).

```
$ date -u ; # now=2026-08-28T12:35:30Z, future_stamp=12:44:30Z, past_stamp=12:26:30Z

$ k8rs fut.json | head -3          # stamps 9 min in our future
1 pod · 0 nodes

● default/broken-crashloop
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)

$ k8rs past.json | head -3         # stamps 9 min in our past
1 pod · 0 nodes

● default/broken-crashloop · 9 min ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
```

## 10. Extreme and malformed `Date` values, end to end

Served through the middlebox in front of the real cluster; grep for the sentence.

```
Date: Tue, 14 Nov 2034 12:47:54 GMT      -> Your computer's clock is 4319999 minutes behind the cluster's, so times are blank rather than guessed.
Date: Mon, 11 Jun 2018 12:47:54 GMT      -> Your computer's clock is 4320000 minutes ahead of the cluster's, so times may be wrong.
Date: Wed, 01 Jan 5000 00:00:00 GMT      -> Your computer's clock is 1563827708 minutes behind the cluster's, so times are blank rather than guessed.
Date: Fri, 31 Dec 9999 23:59:59 GMT      -> (no clock sentence)
Date: Fri, 31 Dec 9999 12:00:00 GMT      -> (no clock sentence)
Date: Sun, 28 Aug 2026 12:55:00 GMT      -> (no clock sentence)   # 28 Aug 2026 is a Friday
```

```
$ LC_ALL=C date -u -d 9999-12-31 +'%a'
Fri
$ LC_ALL=C date -u -d 2026-08-28 +'%a'
Fri
```

The last row's weekday does not match its date; the two year-9999 rows carry the
correct weekday.

## 11. Teardown

```
$ kind get clusters
k8rs
$ kubectl get nodes --no-headers | wc -l ; kubectl get pods -A --no-headers | wc -l
4
40
$ ss -lntp | grep -E ':(18001|18002|19[0-9]{3})\b'
all my listeners are gone
```
