# C2 and C3 against a real API server (operator review, 2026-08-28)

Every run behind the C2/C3 box was against a hand-built Python/TLS stub. This is
the same code met by kube-apiserver v1.36.1 in kind, single-node and then
three-control-plane HA behind kind's own load balancer.

Cluster: `K8RS_CLUSTER=review`, `kindest/node:v1.36.1`, API on `127.0.0.1:6444`
(single node) and on kind's external load balancer (HA). Torn down at the end.
Binary: `cargo build` debug, at the uncommitted C2/C3 tree.

Nothing here was captured into `tests/`; no fixture was produced.

## 1. Does an API server complete a TLS handshake with no client certificate?

The claim under D178: *"an API server **requests** a client certificate and does
not **require** one"* — recorded there as reasoned, not measured.

```
$ openssl s_client -connect 127.0.0.1:6444 -CAfile ca.crt -servername kubernetes \
    -tls1_3 -verify_return_error </dev/null
depth=1 CN=kubernetes
verify return:1
depth=0 CN=kube-apiserver
verify return:1
CONNECTED(00000003)
subject=CN=kube-apiserver
issuer=CN=kubernetes
Acceptable client certificate CA names
SSL handshake has read 2718 bytes and written 1573 bytes
New, TLSv1.3, Cipher is TLS_AES_128_GCM_SHA256
Protocol: TLSv1.3
Verify return code: 0 (ok)
$ echo $?
0
```

No `-cert` / `-key` was passed. The handshake completed, the leaf came back, and
the server printed `Acceptable client certificate CA names` — it asked and did
not insist.

The same thing through the product code, on a cluster whose serving certificate
was reissued to twelve days (see § 3):

```
$ KUBECONFIG=<a bearer-only kubeconfig> ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
11 pods · 2 nodes

○ nothing is broken

The API server's own certificate — not your kubeconfig's — expires in 11 days
(valid until 2026-09-09T18:20:19Z). Once it runs out, kubectl and everything
else stop being able to reach this cluster until someone on the control plane
renews it — not something k8rs can do.
```

The kubeconfig carried a ServiceAccount bearer credential and no client key at
all, and the probe still read the certificate.

## 2. HA control plane: eight consecutive runs of one command

Three control-plane replicas behind kind's `review-external-load-balancer`
(haproxy, TCP round-robin). One replica's serving certificate was reissued to
twelve days; the other two kept theirs. Read directly, bypassing the balancer:

```
replica A -> notAfter=Aug 28 18:34:31 2027 GMT
replica B -> notAfter=Aug 28 18:34:50 2027 GMT
replica C -> notAfter=Sep  9 18:35:41 2026 GMT
```

Then the same command, eight times, against the balancer:

```
$ for i in $(seq 1 8); do timeout 20 ./target/debug/k8rs --live --context kind-review \
    2>/dev/null | grep -m1 "API server's own certificate" | cut -c1-95; done
run 1: (no certificate line)
run 2: (no certificate line)
run 3: (no certificate line)
run 4: (no certificate line)
run 5: The API server's own certificate — not your kubeconfig's — expires in 11 days (valid until 2026
run 6: The API server's own certificate — not your kubeconfig's — expires in 11 days (valid until 2026
run 7: (no certificate line)
run 8: The API server's own certificate — not your kubeconfig's — expires in 11 days (valid until 2026
```

3 of 8 runs printed the line; 5 of 8 printed nothing. No cluster state changed
between runs.

## 3. Twelve-day and expired serving certificates, single node

Reissued in the control-plane container against the cluster CA, same key, same
SANs, then the apiserver container was stopped so the static pod restarted.

```
$ docker exec review-control-plane openssl x509 -in /etc/kubernetes/pki/apiserver.crt \
    -noout -dates
notBefore=Aug 28 18:20:19 2026 GMT
notAfter=Sep  9 18:20:19 2026 GMT
```

k8rs printed the *expires in 11 days* sentence quoted in § 1 (12 days minus the
minutes since issue; `in_days` truncates).

Reissued again with `notAfter` three days in the past:

```
$ openssl x509 -in apiserver.crt -noout -dates
notBefore=Jul 19 18:21:34 2026 GMT
notAfter=Aug 25 18:21:34 2026 GMT

$ openssl s_client -connect 127.0.0.1:6444 -CAfile ca.crt -servername kubernetes </dev/null
Verify return code: 10 (certificate has expired)

$ kubectl --context kind-review get --raw /version
Unable to connect to the server: tls: failed to verify certificate: x509:
certificate has expired or is not yet valid: current time 2026-08-28T21:22:09+03:00
is after 2026-08-25T18:21:34Z
```

k8rs against that same server, with a verifying kubeconfig:

```
$ ./target/debug/k8rs --live --context kind-review
k8rs: watching — could not read the server version (nothing usable came back when
k8rs tried to `get /version`) · could not list what this cluster serves, so k8rs
cannot show you what is in it or tell which add-ons it has (nothing usable came
back when k8rs tried to `get /apis`)
▲ k8rs is not getting pods from this cluster: nothing usable came back when k8rs
tried to `list` and `watch` pods. …
```

No certificate sentence appeared: `grep -c "API server's own certificate"` over
that run's stdout is `0`.

The same server with a kubeconfig setting `insecure-skip-tls-verify`:

```
$ ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
11 pods · 2 nodes

○ nothing is broken

The API server's own certificate — not your kubeconfig's — expired 3 days ago
(was valid until 2026-08-25T18:21:34Z). When that happens, kubectl and everything
else stop being able to reach a cluster until someone on the control plane renews
its certificate — not something k8rs can do.
```

## 4. `tls-server-name` with an IP `server:` — the bare-metal kubeadm shape

The serving certificate was reissued with DNS SANs only, no IP SANs.

```
$ KUBECONFIG=<server: https://127.0.0.1:6444, no tls-server-name> kubectl get --raw /version
Unable to connect to the server: tls: failed to verify certificate: x509: cannot
validate certificate for 127.0.0.1 because it doesn't contain any IP SANs

$ KUBECONFIG=<same, plus tls-server-name: kubernetes> kubectl get --raw /version
{
  "major": "1",
```

k8rs on the second kubeconfig:

```
$ ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
11 pods · 2 nodes

○ nothing is broken

The API server's own certificate — not your kubeconfig's — expires in 11 days
(valid until 2026-09-09T18:24:20Z). Once it runs out, …
```

## 5. What the probe costs at startup

Time from process launch to the first byte on stderr.

Healthy cluster, three runs:

```
run 1: .035693820 s
run 2: .023740067 s
run 3: .023530421 s
```

Kubeconfig with `proxy-url` set, `server:` pointing at an unroutable TEST-NET
address (a local CONNECT proxy served `kubectl` against the same kubeconfig
successfully, so the cluster was reachable):

```
launch  18:27:16.697
18:27:26.705  k8rs: no cluster to watch — nothing usable came back when k8rs tried
              to reach this cluster
```

10.008 s, matching `SERVING_PROBE`. Repeated as a wall-clock measurement:

```
proxied: 10.008545571 s ; message: k8rs: no cluster to watch — nothing usable came
back when k8rs tried to reach this cluster
```

A direct TCP connect to that address black-holes:

```
$ time timeout 12 bash -c 'exec 3<>/dev/tcp/<TEST-NET address>/6443'
12,002 total
```

`kube = { … features = ["client","runtime","rustls-tls","ring"] }` does not
include kube's `http-proxy` feature; `kube-client-4.2.0/src/client/builder.rs:189`
returns `Error::ProxyProtocolUnsupported` for a proxy scheme it was not compiled
for.

## 6. The four session calls with no read deadline

A TLS server that completes every handshake, reads the request and answers
nothing, holding the connection open (Python `ssl`, a locally generated leaf).

```
$ KUBECONFIG=<kubeconfig naming that server> timeout 120 ./target/debug/k8rs --live
exit=124 elapsed=120.007125650 s
--- stderr ---
--- stdout ---
--- server log ---
silent TLS server on 127.0.0.1:8443
handshake done, request read, answering nothing
handshake done, request read, answering nothing
```

Nothing on either stream for the full 120 s.

## 7. C3 — pending CSRs, the granted case

Two auto-approved kubelet CSRs on a fresh cluster:

```
$ kubectl --context kind-review get csr
NAME        SIGNERNAME                                    CONDITION
csr-6sqpd   kubernetes.io/kube-apiserver-client-kubelet   Approved,Issued
csr-xv9z2   kubernetes.io/kube-apiserver-client-kubelet   Approved,Issued
```

```
[certificates]
  What expires, soonest first
  Nothing here expires soon, and no machine is waiting to be let in.
```

One genuinely pending kubelet CSR added (a CSR whose subject the
auto-approver will not sign):

```
$ kubectl --context kind-review get csr review-pending-node
NAME                  CONDITION
review-pending-node   Pending
```

```
[certificates]
  What expires, soonest first
  ● 1 kubelet is waiting to be let in
      A machine cannot join the cluster until someone approves its request.
      → approve each request once you know which machine it came from
```

## 8. C3 — RBAC

The read-only ClusterRole from `docs/security.md`, extracted verbatim from the
fenced block, applied and bound to a ServiceAccount:

```
$ kubectl auth can-i list certificatesigningrequests
Warning: resource 'certificatesigningrequests' is not namespace scoped in group
'certificates.k8s.io'
yes
$ kubectl auth can-i list pods -A
yes
$ kubectl auth can-i delete pods -A
no
```

k8rs under that role printed the same panes as the admin kubeconfig, including
the C3 row.

The `certificates.k8s.io` rule was then deleted from the role:

```
$ kubectl auth can-i list certificatesigningrequests
no
$ kubectl get csr
Error from server (Forbidden): certificatesigningrequests.certificates.k8s.io is
forbidden: User "system:serviceaccount:default:k8rs-ro" cannot list resource
"certificatesigningrequests" in API group "certificates.k8s.io" at the cluster scope
```

```
[certificates]
  What expires, soonest first
  Machines waiting to join are not checked. Seeing them takes a cluster-wide list
  of joining requests, and k8rs does not have one.
  Ask for permission to list certificatesigningrequests across the whole cluster.
```

No crash, and the run continued.

### How often the refused call is re-asked

`apiserver_request_total` does not count the RBAC-denied LIST on this build —
three forbidden `kubectl get csr` calls left the counter at 7 — so the denial was
counted through `authorization_attempts_total`:

```
no-opinion before: 86
no-opinion after a 90s run: 87
delta over 90 seconds: 1
```

One attempt in ninety seconds of watching.

## 9. Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
```
