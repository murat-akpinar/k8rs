# `endpoints_behind` — the join against a live cluster, and the growth rate re-taken

Operator review of the uncommitted `analysis.rs` change that replaces the nested
scan in `services_reaching_nothing` with a one-pass
`BTreeMap<(Option<&str>, &str), usize>` keyed on `(namespace, kubernetes.io/service-name)`.

**Machine:** the dev machine (CachyOS, 12 cores, 23 GiB). Not the LAN host, and
none of these figures are comparable to the 1355 ms in
[`2026-08-22-phase-4-close-cross-family-review.md`](2026-08-22-phase-4-close-cross-family-review.md)
§ 3, which was taken elsewhere.

**Clusters:** three ephemeral single-node `kind` clusters under `K8RS_CLUSTER=review`,
one at a time, each torn down with `K8RS_CLUSTER=review ./scripts/cluster.sh down`
before the next. Node image `kindest/node:v1.36.1`, the same pin the fixture
cluster uses. The PM's `k8rs` cluster was up throughout and was **read only** —
never written, never torn down. No capture was in flight (no `cluster.sh`, `just`
or `cargo` process running; `tests/fixtures/` last written 2026-08-23).

**Addresses in this file are replaced by `<addr-A>` / `<addr-B>` / `<addr-v6>`.**
The findings turn on *which* address got a rule, never on the octets.

---

## 1. The join, against the data plane

### 1a. Every slice on a live cluster carries the label

```
$ kubectl get endpointslices -A -o json | jq -r '.items[] | [.metadata.namespace, .metadata.name,
    (.metadata.labels["kubernetes.io/service-name"] // "<ABSENT>"),
    (.metadata.labels["endpointslice.kubernetes.io/managed-by"] // "<none>"),
    .addressType, (.endpoints|length)] | @tsv'
default    broken-noendpoints-j442n  broken-noendpoints  endpointslice-controller.k8s.io  IPv4  0
default    broken-sts-ks6pz          broken-sts          endpointslice-controller.k8s.io  IPv4  2
default    kubernetes                kubernetes          <none>                           IPv4  1
kube-system kube-dns-9lgx6           kube-dns            endpointslice-controller.k8s.io  IPv4  2
```

Slices with no `kubernetes.io/service-name`: **0**.

`default/kubernetes` is reconciled by the apiserver, not the endpointslice
controller — `managed-by` is absent on it, the service-name label is not.

Owner references, same cluster:

```
$ kubectl get endpointslices -A -o json | jq -r '.items[] | [.metadata.namespace+"/"+.metadata.name,
    ((.metadata.ownerReferences // []) | map(.kind+"/"+.name) | join(","))] | @tsv'
default/broken-noendpoints-j442n   Service/broken-noendpoints
default/broken-sts-ks6pz           Service/broken-sts
default/kubernetes
kube-system/kube-dns-9lgx6         Service/kube-dns
```

The ownerReference is **not** universal; the label is.

### 1b. The label is the join the data plane itself uses

Review cluster, kube-proxy in iptables mode (`server_linux.go:137 "Using iptables Proxier"`).
A Service with a selector matching nothing, plus two hand-written slices for it —
one carrying `kubernetes.io/service-name: selectorful`, one carrying no label:

```
$ kubectl -n probe get endpointslices -o custom-columns=NAME:...,SVC-LABEL:...,MANAGEDBY:...
NAME                  SVC-LABEL     MANAGEDBY
handmade-labelled     selectorful   <none>
handmade-unlabelled   <none>        <none>
selectorful-vdd24     selectorful   endpointslice-controller.k8s.io
```

Neither hand-written slice was deleted by the endpointslice controller.

```
$ docker exec review-control-plane iptables-save -t nat | grep -F <clusterIP of probe/selectorful>
-A KUBE-SERVICES -d <clusterIP>/32 -p tcp --comment "probe/selectorful:http cluster IP" --dport 80 -j KUBE-SVC-RJKLMOHA7TDX2MD2
-A KUBE-SVC-RJKLMOHA7TDX2MD2 --comment "probe/selectorful:http -> <addr-A>:80" -j KUBE-SEP-OAZ37U27PYPMZZ5H
-A KUBE-SEP-OAZ37U27PYPMZZ5H -p tcp --comment "probe/selectorful:http" -j DNAT --to-destination <addr-A>:80
```

`<addr-A>` is the address on the **labelled** slice. `<addr-B>`, the address on
the **unlabelled** slice, appears nowhere in the NAT table.

### 1c. The namespace half holds on the data plane too

A second Service of the same name in a second namespace, nothing behind it:

```
$ kubectl -n probe2 get svc selectorful -o jsonpath='{.spec.clusterIP}'   # a different clusterIP
$ docker exec review-control-plane iptables-save -t nat | grep -c <clusterIP of probe2/selectorful>
0
```

`probe/selectorful`'s endpoint does not reach `probe2/selectorful`, and a Service
with nothing behind it gets no NAT rule at all.

### 1d. The other shapes that produce slices

| shape | slice created | service-name label |
|---|---|---|
| Service with a selector, no matching pods | yes, one placeholder, `endpoints: <nil>` | present |
| headless Service (`clusterIP: None`) with a selector | yes | present |
| selectorless Service + hand-written `Endpoints` | yes, `managed-by: endpointslicemirroring-controller.k8s.io` | present |
| `type: ExternalName` with a selector | **no slice at all** | n/a |

### 1e. The API accepts a slice with no label, and with an empty label

```
$ kubectl apply --dry-run=server -f -   # EndpointSlice, no labels at all
endpointslice.discovery.k8s.io/nolabel-slice created (server dry run)     exit=0
$ kubectl apply --dry-run=server -f -   # labels: {"kubernetes.io/service-name": ""}
endpointslice.discovery.k8s.io/emptylabel-slice created (server dry run)  exit=0
```

---

## 2. `qualified()` cannot be made ambiguous — the API server's own validation

```
$ kubectl create ns "default/x"
The Namespace "default/x" is invalid:
* metadata.name: Invalid value: "default/x": a lowercase RFC 1123 label must consist of lower
  case alphanumeric characters or '-' ... regex '[a-z0-9]([-a-z0-9]*[a-z0-9])?'
exit=1

$ kubectl create service clusterip "x/y" --tcp=80:80
error: ... spec.parentRef.name: Invalid value: "x/y": may not contain '/'

$ kubectl create service clusterip <63 a's> --tcp=80:80 --dry-run=server
service/<63 a's> created (server dry run)
$ kubectl create service clusterip <64 a's> --tcp=80:80 --dry-run=server
error: ... metadata.name: must be no more than 63 characters

$ kubectl apply --dry-run=server -f -   # kubernetes.io/service-name value, 63 a's
endpointslice.discovery.k8s.io/len-probe created (server dry run)
$ kubectl apply --dry-run=server -f -   # same, 64 a's
The EndpointSlice "len-probe" is invalid: metadata.labels: Invalid value: ...
  must be no more than 63 bytes
```

Namespace max 63, Service name max 63, label value max 63 bytes, and no `/` in
any of the three.

---

## 3. Dual-stack: two slices, one label, one pod

Review cluster created with `networking: {ipFamily: dual}`. One Deployment of one
replica, one Service with `ipFamilyPolicy: RequireDualStack`:

```
$ kubectl -n ds get svc web -o jsonpath='{.spec.ipFamilies}'
["IPv4","IPv6"]
$ kubectl -n ds get endpointslices -o custom-columns=NAME:...,SVC-LABEL:...,ADDRTYPE:...,ENDPOINTS:...
NAME        SVC-LABEL   ADDRTYPE   ENDPOINTS
web-7g4sv   web         IPv6       [<addr-v6>]
web-7s62j   web         IPv4       [<addr-A>]
```

One pod behind the Service; `endpoints_behind` sums to **2**. The value is
endpoint *entries*, which on a dual-stack cluster is `pods x address families`.

Both directions through the built binary, fed the real objects of that namespace:

```
$ kubectl -n ds get svc,endpointslices -o json > ds.json
$ ./k8rs --analysis ds.json | grep "matches no pod"
  ● ds/nobody matches no pod
```

`ds/web` (one pod, two slices) stays quiet; `ds/nobody` (no pods, two placeholder
slices) is named. Every fixture slice in `tests/fixtures/endpointslices.json` is
`IPv4`, so the committed corpus cannot hold this shape.

The nested scan summed the same slices the same way, so this sum is what it was before the reviewed diff.

---

## 4. A Service the pane names that is working

Review cluster. `type: ExternalName` with a leftover `spec.selector` — the API
accepts it, and the endpointslice controller creates nothing for it.

```
$ kubectl -n en get svc -o custom-columns=NAME:.metadata.name,TYPE:.spec.type,SELECTOR:.spec.selector
NAME      TYPE           SELECTOR
extdb     ExternalName   map[app:never-matches]
healthy   ClusterIP      map[app:healthy]

$ kubectl -n en get svc extdb -o jsonpath='{"clusterIP="}{.spec.clusterIP}{"  externalName="}{.spec.externalName}'
clusterIP=  externalName=db.vendor.invalid

$ kubectl -n en get svc,endpointslices -o json > en2.json
$ ./k8rs --analysis en2.json
  Things that cost you something for nothing
  ● en/extdb matches no pod
      This Service points at nothing. Anything calling it gets a 503.
      → fix its selector, or delete it
```

`spec.clusterIP` is empty: there is no address for a caller to get a 503 from.
`ServiceSnapshot` (`src/rules.rs:1583`) carries `id` and `selector` only — no
`spec.type`.

The filter this row is selected by (`src/analysis.rs:1798`) reads `selector` and nothing else, before and after the reviewed diff; the diff changes which slices are counted, not which Services are asked about.

Not measured: that the in-cluster DNS name of `en/extdb` returns a CNAME. The pod carrying
the probe would not start on the images cached on the node, and the run was not
repeated with a pull.

---

## 5. The growth rate, re-taken

Two release binaries from the same source except the reviewed diff:
`git archive HEAD` into two trees, the working-tree copies of `src/analysis.rs`
and `src/analysis_tests/waste.rs` laid over one of them. `diff -rq` reports those
two files and no others.

Input: a `kind: List` of N Services, each with a selector, in 50 namespaces; every
third Service has no EndpointSlice (so the pane draws rows and the *and N more*
line), the rest have one slice with one endpoint.

Whole-process wall clock, best of 7, milliseconds. Load average at the start of
the run was 1.59 — the PM's fixture cluster idling at roughly a third of one core
across four containers. `cards-only` is the same binary on the same file without
`--analysis`.

| N Services | before, `--analysis` min/med/max | after, `--analysis` min/med/max | cards-only min/med/max |
|---|---|---|---|
| 1 250 | 16 / 20 / 22 | 15 / 16 / 19 | 14 / 17 / 20 |
| 2 500 | 37 / 41 / 44 | 30 / 33 / 38 | 27 / 29 / 34 |
| 5 000 | 100 / 107 / 145 | 58 / 62 / 82 | 51 / 53 / 59 |
| 10 000 | 332 / 362 / 402 | 122 / 133 / 137 | 100 / 106 / 112 |
| 20 000 | 1806 / 1885 / 1959 | 225 / 246 / 255 | 204 / 219 / 248 |

Per doubling, on the minima: **before** 2.3x, 2.7x, 3.3x, 5.4x — rising toward
the 4x-per-doubling of a quadratic and past it in the last step. **After** 2.0x,
1.9x, 2.1x, 1.8x — linear, and tracking the JSON parse floor in the third column.

Subtracting that floor gives the seven reports' own cost: before 2, 10, 49, 232,
1602 ms; after 1, 3, 7, 22, 21 ms.

Over a 200-node / 5000-pod base, which is the shape the 2026-08-22 report used:

| input | before | after | cards-only | reports (after) |
|---|---|---|---|---|
| 200 nodes, 5000 pods, 0 Services | 100 ms | 93 ms | 81 ms | 12 ms |
| + 5 000 Services | 195 ms | 146 ms | 135 ms | 11 ms |
| + 10 000 Services | 438 ms | 202 ms | 182 ms | 20 ms |

The 12 ms in the first row is this machine's reading of the `~25 ms` the
2026-08-22 report records for all seven reports at 200 nodes / 5000 pods.

**No figure here is precise.** Best-of-7 on a machine running the fixture cluster;
the spread columns are there so the minima are not quoted alone.

### Output identity

sha256 of the full `--analysis` output, before vs after:

| input | identical |
|---|---|
| N = 1250 / 2500 / 5000 / 10000 / 20000, orphan-bearing | yes, all five, and the sha differs per N |
| 200n/5000p + 0 / 5000 / 10000 Services | yes, all three |
| the real dual-stack namespace of § 3 | yes |

The first table's sha changing with N is what makes it an identity check: on an
input with no orphans the output is the same string at every N.

---

## 6. What was checked without a cluster

- `endpoints_behind` has one caller (`src/analysis.rs:1795`) and
  `snapshot.endpoint_slices` has one reader in the whole crate, so no second rule
  can disagree with this join.
- The map borrows the slice list; no name is cloned.
- `src/k8s.rs:1515` sets `endpoint_slices: None`, so on `--live` this row is
  `NotComputed` today and none of § 5 is reachable through a real cluster yet.
- The cited `reports/2026-08-22-phase-4-close-cross-family-review.md` § 3 is where
  the figures the new doc comment points at actually are.
- Row order at N = 1250 is `ns-0/svc-0, ns-0/svc-1050, ns-0/svc-1200, ns-0/svc-150,
  ns-0/svc-300` — lexicographic within namespace, which is what `kubectl get -A`
  prints.
- `kind delete cluster` is refused by this session's permission system;
  `K8RS_CLUSTER=review ./scripts/cluster.sh down` is not, and is what tore down
  all three clusters.
