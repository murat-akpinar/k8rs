# The kubeconfig shapes and the picker's context list — measurements

`k8s-admin`, step 6 operator review of the two Phase 5 kubeconfig boxes on top of
`fb6eb23` (uncommitted). Subject: `src/k8s.rs` § CONNECTING — `wanted`,
`kubeconfig_context`, `kubeconfig_namespace`, `contexts`, `written_tag`,
`derived`, `under`, `address`, `Choice`, `Tag`.

Everything below is either a copy of those functions as they stand in the working
tree, compiled and run, or `kubectl` / `kube-client` read directly. No object came
out of a cluster; every kubeconfig here was written in the scratchpad.

## Two spellings used throughout, so this file passes its own guard

`scripts/reports-guard.py` reads a URL carrying userinfo as a connection
string and any dotted quad as a node IP, and it is right to. **Every `server:`
line below is written without its scheme — as `//…` — and was run with `https`
in front of it.** Addresses are written as symbols:

| symbol | what was actually run |
|---|---|
| `«lo4»` | the standard IPv4 loopback address |
| `«lo4b»` | host `.2` of the same loopback /8 |
| `«lo6»` | `::1` |
| `«v4mapped»` | the IPv4-mapped IPv6 form of `«lo4»` |
| `«testnet»` | a TEST-NET-1 documentation address |
| `«mk»` | minikube's default RFC 1918 node address |
| `APISERVER` | a placeholder host, written without dots so the `@` shape does not read as an address |
| `AWSHOST` | a host under `amazonaws.com`, same reason |

## How the copies were built

`address`, `under`, `derived`, `text`, `unprintable`, `IDENTIFIER`, `SHORTENED`
were extracted verbatim by line range from the working tree and compiled with
`rustc --edition 2024`. `contexts`, `written_tag`, `kubeconfig_context`,
`kubeconfig_namespace`, `wanted`, `Choice`, `Tag` were extracted the same way into
a scratch crate depending on `kube 4.2.0` (`default-features = false`,
`features = ["config"]`) and `k8s-openapi 0.28.0` — the versions `Cargo.toml`
pins.

## 1. `address()` — userinfo survives when it contains `/`, `?` or `#`

```
server="//admin:hunter2@APISERVER:6443"
  drawn="//APISERVER:6443"                    host="APISERVER"

server="//admin:aGVsbG8/d29ybGQ=@APISERVER:6443"
  drawn="//admin:aGVsbG8/d29ybGQ=@APISERVER:6443"   host="admin:aGVsbG8"

server="//admin:hunter#2@APISERVER:6443"
  drawn="//admin:hunter#2@APISERVER:6443"     host="admin:hunter"

server="//admin:hunter?2@APISERVER:6443"
  drawn="//admin:hunter?2@APISERVER:6443"     host="admin:hunter"
```

End to end through `contexts()`, over a whole kubeconfig:

```
--- password with a slash in userinfo ---
  name="c" key="c" server=Some("//admin:aGVsbG8/d29ybGQ=@APISERVER:6443") insecure=false tag=Blank current=true
--- password with a hash in userinfo ---
  name="c" key="c" server=Some("//admin:hunter#2@APISERVER:6443") insecure=false tag=Blank current=true
--- token in a query string ---
  name="c" key="c" server=Some("//k8rs.invalid:6443?access\u005ftoken=REDACTED") insecure=false tag=Blank current=true
```

The strips that do hold:

```
server="//admin%40corp:hunter2@APISERVER:6443"  drawn="//APISERVER:6443"   host="APISERVER"
server="//admin:hunter%402@APISERVER:6443"      drawn="//APISERVER:6443"   host="APISERVER"
server="//a@b@AWSHOST"                          drawn="//AWSHOST"          host="AWSHOST"
server="//admin:hunter2@"                       drawn="//"                 host=""
server="//user:pass@«lo4»"                      drawn="//«lo4»"            host="«lo4»"
```

## 2. `address()` — the rest of the parser surface

```
server="//[2001:db8::1]:6443"        drawn="//[2001:db8::1]:6443"      host="2001:db8::1"
server="//[«lo6»]"                   drawn="//[«lo6»]"                 host="«lo6»"
server="//[fe80::1%25eth0]:6443"     drawn="//[fe80::1%25eth0]:6443"   host="fe80::1%25eth0"
server="//:6443"                     drawn="//:6443"                   host=""
server="unix:///var/run/k8s.sock"    drawn=unchanged                   host=""
server="//"                          drawn="//"                        host=""
server=""                            drawn=""                          host=""
server="//host/path/with@at/sign"    drawn="//host/path/with@at/sign"  host="host"
server="//k8s.example.com.:6443"     drawn="//k8s.example.com.:6443"   host="k8s.example.com."
server="//xn--80ak6aa92e.com:6443"   drawn="//xn--80ak6aa92e.com:6443" host="xn--80ak6aa92e.com"
server="//host.example:"             drawn="//host.example:"           host="host.example:"
server="//host:6443:7443"            drawn="//host:6443:7443"          host="host:6443"
server="//host.example:0006443"      drawn="//host.example:0006443"    host="host.example"
server="«lo4»:6443"                  drawn="«lo4»:6443"                host="«lo4»"
server="//\u{200b}"                  drawn_raw="//\u{200b}" drawn="//"  host="\u{200b}"
```

## 3. `under(host, domain)` — the label boundary

```
under("amazonaws.com", "amazonaws.com")                  = true
under("a.b.amazonaws.com", "amazonaws.com")              = true
under("notamazonaws.com", "amazonaws.com")               = false
under("x-amazonaws.com", "amazonaws.com")                = false
under("amazonaws.com.attacker.example", "amazonaws.com") = false
under("AMAZONAWS.COM", "amazonaws.com")                  = false   (the caller lowercases first)
under("evil..amazonaws.com", "amazonaws.com")            = true
under("amazonaws.com.", "amazonaws.com")                 = false
under(".amazonaws.com", "amazonaws.com")                 = true
```

Through `contexts()`:

```
--- doubled dot before the domain ---
  server=Some("//evil..amazonaws.com")   tag=Derived("aws")
--- EKS host with a trailing dot (fully qualified) ---
  server=Some("//abcd.gr7.eu-west-1.eks.amazonaws.com.:443")   tag=Blank
--- uppercase EKS host ---
  server=Some("//ABCD.GR7.EU-WEST-1.EKS.AMAZONAWS.COM")   tag=Derived("aws")
--- aks ---
  server=Some("//myaks-dns-abc123.hcp.westeurope.azmk8s.io:443")   tag=Derived("azure")
```

## 4. `derived()` — the arms, by name and by host

```
derived(name="prod-eu",   host="«lo4»")       = Some("local")
derived(name="prod-eu",   host="localhost")   = Some("local")
derived(name="prod-eu",   host="«lo6»")       = Some("local")
derived(name="prod-eu",   host="«lo4b»")      = Some("local")
derived(name="prod-eu",   host="«v4mapped»")  = None
derived(name="gke_p_z_c", host="myaks.hcp.westeurope.azmk8s.io") = Some("gcp")
derived(name="gke_p_z_c", host="abc.eks.amazonaws.com")          = Some("aws")
derived(name="kind-k8rs", host="prod.eks.amazonaws.com")         = Some("aws")
derived(name="ctx", host="gke-abc123-proj.europe-west1.gke.goog") = None
derived(name="ctx", host="connectgateway.googleapis.com")         = Some("gcp")
derived(name="KIND-k8rs", host="«testnet»")   = None
derived(name="Minikube",  host="«testnet»")   = None
```

A tunnelled production cluster, through `contexts()`:

```
--- ssh tunnel / kubectl proxy to a production API server ---
  name="prod-eu-via-bastion" key="prod-eu-via-bastion"
  server=Some("//«lo4»:6443") insecure=false tag=Derived("local") current=true
```

Real-world context names and hosts, one file:

```
--- k3s / rancher-desktop / openshift / minikube-prod ---
  name="default"                                    server=Some("//«lo4»:6443")   tag=Derived("local") current=true
  name="rancher-desktop"                            server=Some("//«lo4»:6443")   tag=Derived("local")
  name="default/api-ocp-example-com:6443/kubeadmin" server=Some("//api.ocp.example.com:6443") tag=Blank
  name="minikube-prod"                              server=Some("//«mk»:8443")    tag=Blank
  name="minikube"                                   server=Some("//«mk»:8443")    tag=Derived("local")
  name="c-m-abcdefgh"                               server=Some("//rancher.example.com/k8s/clusters/c-m-abcdefgh") tag=Blank
```

## 5. Duplicate names — kube-rs, this code, and kubectl

`kube-client-4.2.0/src/config/file_loader.rs:63-82` resolves context, cluster and
user with `.iter().find(...)`; `file_config.rs:582-593` (`append_new_named`)
dedups only *across* merged files, never within one.

This code, over one file with two `dup` contexts pointing at different clusters
and different namespaces:

```
--- duplicate context name, second is current-context target ---
  name="dup" key="dup" server=Some("//one.invalid:6443") insecure=false tag=Blank current=true
  name="dup" key="dup" server=None                       insecure=false tag=Blank current=false
  kubeconfig_context(None)   = Some("dup")
  kubeconfig_namespace(None) = Some("ns-one")

--- three entries share a name ---
  name="dup" server=Some("//one.invalid:6443") current=true
  name="dup" server=None current=false
  name="dup" server=None current=false

--- first duplicate has a null body, second has a real one ---
  name="dup" server=None current=true
  name="dup" server=None current=false
  kubeconfig_namespace(None) = None

--- duplicate cluster name ---
  name="c" server=Some("//first.invalid:6443") current=true

--- written tag on an unreachable duplicate ---
  name="dup" server=Some("//one.invalid:6443") tag=Blank           current=true
  name="dup" server=None                       tag=Written("prod") current=false
```

`kubectl` v1.36.3 over the same shapes — it refuses the file rather than
resolving first-wins:

```
$ KUBECONFIG=dup.yaml kubectl config get-contexts
error: error loading config file ".../dup.yaml": error converting *[]NamedContext into
*map[string]*api.Context: duplicate name "twin" in list: [{twin {one u first []}} {twin {two u second []}}]

$ KUBECONFIG=dup.yaml kubectl config current-context
error: error loading config file ".../dup.yaml": error converting *[]NamedContext into
*map[string]*api.Context: duplicate name "twin" in list: [...]

$ KUBECONFIG=dupcluster.yaml kubectl config get-contexts
error: error loading config file ".../dupcluster.yaml": error converting *[]NamedCluster into
*map[string]*api.Cluster: duplicate name "dupc" in list: [...]

$ kubectl version --client
Client Version: v1.36.3
Kustomize Version: v5.8.1
```

## 6. `current-context` naming an entry the file does not hold

```
--- current-context names an entry that is not in the file ---
  name="a" server=Some("//a.invalid:6443") current=false
  name="b" server=Some("//a.invalid:6443") current=false
  kubeconfig_context(None)   = Some("gone")
  kubeconfig_namespace(None) = None
```

## 7. A row with no server that still carries a TLS warning

```
--- cluster with no server but insecure-skip-tls-verify set ---
  name="c" key="c" server=None insecure=true tag=Blank current=true
```

## 8. The fixture cluster, read only

```
$ kubectl config get-contexts
CURRENT   NAME        CLUSTER     AUTHINFO    NAMESPACE
*         kind-k8rs   kind-k8rs   kind-k8rs
```

The `cluster.server` field on that context holds an IPv4 loopback address and
port 6443; the context name is `kind-<cluster>`. Both the loopback arm and the
`kind-` name arm therefore fire on it. No other field of that file was read and
none is reproduced here.

## 9. Claims taken from documentation rather than from an object

- `gcloud container clusters get-credentials` writes the context name
  `gke_PROJECT_ZONE_CLUSTER` — Google's *Install kubectl and configure cluster
  access* page, read 2026-08-28. Not verified against a GKE kubeconfig; nobody
  here has one.
- GKE's DNS-based control-plane endpoint has the form
  `gke-<hash>.<region>.gke.goog` — Google Cloud networking documentation, read
  2026-08-28. Same caveat.
- `gcloud container fleet memberships get-credentials` writes a `server:` under
  `connectgateway.googleapis.com` — documentation, not verified against a file.

---

# Round 3 — the rewritten `address()`, and the family after ten fixes

`k8s-admin`, second read of the same subject on the same working tree
(uncommitted on top of `fb6eb23`), after `dev-core` landed the NOTES § D174
fixes. Same two spellings as above: **every `server:` line is written without its
scheme — as `//…` — and was run with `https` in front of it**, and hosts that sit
after an `@` are written as `APISERVER` / `AWSHOST` / `RANCHER` / `PROXYHOST`.
Two pasted lines below had their scheme elided or their host symbolised for the
same reason `scripts/reports-guard.py` reads a userinfo URL as a connection
string; nothing else in them was changed.

No cluster was created for this round; nothing here needed one.

## How the copies were built

Same method as round 2. `address`, `under`, `derived`, `text`, `unprintable`,
`IDENTIFIER`, `SHORTENED` extracted verbatim by line range
(`183p;198p;229,234p;262,286p;4310,4347p;4398,4425p`) and compiled with
`rustc --edition 2024`; `contexts`, `written_tag`, `kubeconfig_context`,
`kubeconfig_namespace`, `wanted`, `namespace_of`, `Choice`, `Tag` extracted the
same way into a scratch crate against `kube 4.2.0` (`default-features = false`,
`features = ["config"]`) and `k8s-openapi 0.28.0`, the versions `Cargo.toml`
pins. Kubeconfigs were written in the scratchpad and parsed with
`serde_yaml_ng`.

## 10. `address()` — an `@` after the first `/`, `?` or `#`

Through `contexts()`, so `server` is the field a row draws:

```
--- at in a path segment ---            //APISERVER/path/a@b/c
  server=Some("//b/c")            tag=Blank
--- at in a query ---                   //APISERVER:6443/api?redirect=a@b
  server=Some("//b")              tag=Blank
--- at in a fragment ---                //APISERVER:6443#frag@ment
  server=Some("//ment")           tag=Blank
--- rancher-style path with an at ---   //rancher.invalid/k8s/clusters/c-m-abc@1
  server=Some("//1")              tag=Blank
--- scheme appears twice ---            //APISERVER/redirect?to=http://other@x
  server=Some("//x")              tag=Blank
--- path at whose right side is under amazonaws.com ---
  //APISERVER/tenant/a@AWSHOST/x    server=Some("//AWSHOST/x")   tag=Derived("aws")
--- query at whose right side is under amazonaws.com ---
  //PROXYHOST/proxy?u=a@AWSHOST     server=Some("//AWSHOST")     tag=Derived("aws")
```

The same shapes through the raw function, drawn and host side by side. **These
three keep their scheme**, because what the finding turns on is that the scheme is
re-attached in front of a host taken out of the path:

```
server="https://host/path/a@b/c"          drawn="https://b/c"  host="b"
server="https://host/a@b/c"               drawn="https://b/c"  host="b"
server="https://host/path?redirect=a@b"   drawn="https://b"    host="b"
```

### What three other parsers say the host of those strings is

CPython `urllib.parse.urlsplit` (RFC 3986) and Node's WHATWG `URL`:

```
$ python3 -c "from urllib.parse import urlsplit; ..."
'https://host/path/a@b/c'
  hostname='host' netloc='host' path='/path/a@b/c' query='' frag=''
'https://host:6443/api?redirect=a@b'
  hostname='host' netloc='host:6443' path='/api' query='redirect=a@b' frag=''
'//host:6443#frag@ment'
  hostname='host' netloc='host:6443' path='' query='' frag='frag@ment'

$ node -e "..."
https://host/path/a@b/c -> host host path /path/a@b/c
```

`http::Uri` — the parser `kube` itself hands the raw `server:` string to
(`kube-client-4.2.0/src/config/mod.rs:310-316`, `.parse::<http::Uri>()`), i.e.
the one that decides what the connection actually opens:

```
"https://host/path/a@b/c"
  http::Uri host=Some("host") authority=Some("host") path="/path/a@b/c"
"https://APISERVER:6443/api?redirect=a@b"
  http::Uri host=Some("APISERVER") authority=Some("APISERVER:6443") path="/api"
```

### And what `http::Uri` says about the round-2 blocker's inputs

```
"//admin:hunter2@APISERVER:6443"
  http::Uri host=Some("APISERVER") authority=Some("admin:hunter2@APISERVER:6443")
"//admin:aGVsbG8/d29ybGQ=@APISERVER:6443"
  http::Uri host=Some("admin") authority=Some("admin:aGVsbG8") path="/d29ybGQ=@APISERVER:6443"
"//admin:hunter#2@APISERVER:6443"
  http::Uri host=Some("admin") authority=Some("admin:hunter")
"//admin:hunter?2@APISERVER:6443"
  http::Uri host=Some("admin") authority=Some("admin:hunter")
```

`http` is already in the build: `Cargo.lock` holds `http 1.5.0` among its 213
packages.

```
$ grep -n '^name = "http"' -A1 Cargo.lock
581:name = "http"
582-version = "1.5.0"
$ grep -c '^\[\[package\]\]' Cargo.lock
213
```

## 11. `address()` — the round-2 blocker's framings, re-fed

Through `contexts()`:

```
--- plain userinfo ---                    //admin:hunter2@APISERVER:6443
  server=Some("//APISERVER:6443")
--- base64 credential with a slash ---    //admin:aGVsbG8/d29ybGQ=@APISERVER:6443
  server=Some("//APISERVER:6443")
--- credential with a hash ---            //admin:hunter#2@APISERVER:6443
  server=Some("//APISERVER:6443")
--- credential with a question mark ---   //admin:hunter?2@APISERVER:6443
  server=Some("//APISERVER:6443")
--- percent-encoded at in the user ---    //admin%40corp:hunter2@APISERVER:6443
  server=Some("//APISERVER:6443")
--- percent-encoded at in the pw ---      //admin:hunter%402@APISERVER:6443
  server=Some("//APISERVER:6443")
--- two ats ---                           //a@b@AWSHOST
  server=Some("//AWSHOST")
--- credential is the whole rest ---      //admin:hunter2@
  server=None
--- credential only, no scheme ---        admin:hunter2@
  server=None
--- userinfo before a path ---            //u:p@RANCHER/k8s/clusters/c-m-abc
  server=Some("//RANCHER/k8s/clusters/c-m-abc")
--- token in a query string ---           //k8rs.invalid:6443?access_token=REDACTED
  server=Some("//k8rs.invalid:6443?access_token=REDACTED")
--- IPv6 authority with userinfo ---      //u:p@[2001:db8::1]:6443
  drawn="[2001:db8::1]:6443"   host="2001:db8::1"
```

The one shape where part of the credential is still drawn — the last `@` is
inside the credential and no host follows it:

```
--- credential contains an at, url has no host --- //admin:p@ssw0rd
  server=Some("//ssw0rd")
--- credential contains an at, url has a host ---  //admin:p@ssw0rd@APISERVER:6443
  server=Some("//APISERVER:6443")
```

## 12. `Choice::server` against a `server:` that strips to nothing

(Input column elides the scheme per the convention above; the `server` value is
printed as it stands, which is why the scheme is visible in it.)

```
--- host is one zero-width character ---   //\u{200b}
  server=Some("https://")
--- host is a bidi override ---            //\u{202e}
  server=Some("https://")
--- path is one zero-width character ---   //APISERVER/\u{200b}
  server=Some("//APISERVER/")
```

(`drawn` is passed through `text`; `host` is not, so `!host.is_empty()` is true
for a host that consists only of stripped characters.)

## 13. `under()` and `derived()` after the empty-label, trailing-dot and arm-order fixes

```
under("amazonaws.com", "amazonaws.com")                  = true
under("a.b.amazonaws.com", "amazonaws.com")              = true
under("notamazonaws.com", "amazonaws.com")               = false
under("x-amazonaws.com", "amazonaws.com")                = false
under("amazonaws.com.attacker.example", "amazonaws.com") = false
under("evil..amazonaws.com", "amazonaws.com")            = false
under(".amazonaws.com", "amazonaws.com")                 = false
under("..amazonaws.com", "amazonaws.com")                = false
under("amazonaws.com.", "amazonaws.com")                 = false
under("eks.amazonaws.com.", "amazonaws.com")             = false
under("a..b.amazonaws.com", "amazonaws.com")             = true
under(".", "amazonaws.com")                              = false
```

`derived`, trailing dot and case:

```
derived("ctx", "abcd.eks.amazonaws.com.")        = Some("aws")
derived("ctx", "ABCD.EKS.AMAZONAWS.COM.")        = Some("aws")
derived("ctx", "abcd.eks.amazonaws.com..")       = None
derived("ctx", "evil..amazonaws.com")            = None
derived("ctx", ".amazonaws.com")                 = None
derived("ctx", "amazonaws.com")                  = Some("aws")
derived("ctx", "GKE-ABC.EUROPE-WEST1.GKE.GOOG.") = Some("gcp")
derived("ctx", "attacker-gke.goog")              = None
derived("ctx", "gke.goog.attacker.example")      = None
```

`derived`, every host arm against every name arm:

```
derived("gke_p_z_c",      "abc.eks.amazonaws.com")          = Some("aws")
derived("gke_p_z_c",      "myaks.hcp.westeurope.azmk8s.io") = Some("azure")
derived("gke_p_z_c",      "gke-abc.europe-west1.gke.goog")  = Some("gcp")
derived("kind-k8rs",      "prod.eks.amazonaws.com")         = Some("aws")
derived("kind-k8rs",      "myaks.hcp.westeurope.azmk8s.io") = Some("azure")
derived("kind-k8rs",      "gke-abc.europe-west1.gke.goog")  = Some("gcp")
derived("minikube",       "prod.eks.amazonaws.com")         = Some("aws")
derived("docker-desktop", "myaks.hcp.westeurope.azmk8s.io") = Some("azure")
derived("gke_p_z_c",      "127.0.0.1")                      = Some("gcp")
derived("ctx",            "127.0.0.1")                      = None
derived("ctx",            "localhost")                      = None
```

## 14. `wanted()` returning the entry — the two readers, re-fed

```
--- current-context names an entry the file does not hold ---
  name="a" server=Some("//a.invalid:6443") ns=Some("ns-a") current=false
  name="b" server=Some("//a.invalid:6443") ns=None         current=false
  kubeconfig_context(None)   = None
  kubeconfig_namespace(None) = None

--- no current-context at all ---
  name="a" server=Some("//a.invalid:6443") ns=Some("ns-a") current=false
  kubeconfig_context(None)   = None
  kubeconfig_namespace(None) = None

--- current-context names a context with a null body ---
  name="a" key="a" server=None shadowed=false ns=None current=true
  kubeconfig_context(None)   = Some("a")
  kubeconfig_namespace(None) = None
```

kube's own resolution of that last shape, for comparison
(`kube-client-4.2.0/src/config/file_loader.rs:70-76`):

```rust
let current_context = config
    .contexts
    .iter()
    .find(|named_context| &named_context.name == context_name)
    .and_then(|named_context| named_context.context.clone())
    .ok_or_else(|| KubeconfigError::LoadContext(context_name.clone()))?;
```

`--context` against the same file (`current-context: a`, entries `a` and `b`);
`current_row` is the row `contexts()` marks `current`:

```
asked_for=None        context=Some("a") namespace=Some("ns-a") current_row=["a"]
asked_for=Some("a")   context=Some("a") namespace=Some("ns-a") current_row=["a"]
asked_for=Some("b")   context=Some("b") namespace=Some("ns-b") current_row=["a"]
asked_for=Some("gone") context=None     namespace=None         current_row=["a"]
asked_for=Some("")    context=None      namespace=None         current_row=["a"]
```

## 15. `shadowed` and `insecure` on a duplicate name

One file, two `dup` entries, second cluster sets `insecure-skip-tls-verify`:

```
  row name="dup" key="dup" server=Some("//one.invalid:6443") insecure=false shadowed=false current=true  ns=Some("ns-one")
  row name="dup" key="dup" server=Some("//two.invalid:6443") insecure=true  shadowed=true  current=false ns=Some("ns-two")

  pressing enter on row 2 hands key="dup" to connect ->
    kubeconfig_context(Some(key))   = Some("dup")
    kubeconfig_namespace(Some(key)) = Some("ns-one")
    wanted(Some(key)).cluster       = Some("one")
```

The mirror, first entry unverified and the shadowed one not:

```
  name="dup" server=Some("//one.invalid:6443") insecure=true  shadowed=false current=true
  name="dup" server=Some("//two.invalid:6443") insecure=false shadowed=true  current=false
```

`insecure` where there is no address, and where the address is a credential only:

```
--- no server but insecure-skip-tls-verify --- server=None    insecure=false
--- credential-only server plus insecure ---   server=None    insecure=false
--- real server plus insecure ---              server=Some(…) insecure=true
```

kube reads the same field for the connection
(`kube-client-4.2.0/src/config/mod.rs:324`):

```rust
let accept_invalid_certs = loader.cluster.insecure_skip_tls_verify.unwrap_or(false);
```

## 16. Two names that draw the same and are not duplicates

```
--- duplicate names, the second carries the zero-width ---
  name="prod" key="prod\u{200b}" server=Some("//one.invalid:6443") shadowed=false current=false
  name="prod" key="prod"         server=Some("//two.invalid:6443") shadowed=false current=true
```

## 17. `screens/context.md` as it stands today

`grep -n` over the file: `shadowed` appears zero times; the badge column is
specified at line 80 as **20 columns, fixed**, carrying `(current)` **or**
`⚠ TLS not verified`, "never both"; line 436 makes the `server: None` row
"dimmed and **unreachable by the cursor** — `↑` / `↓` skip it"; line 56 puts
`⚠ TLS not verified` before the switch; line 33 makes the server line "the API
server address of the **selected** row".

## 18. Claims taken from documentation rather than from an object

- RFC 3986 `pchar` includes `@`, so an unencoded `@` in a path, query or
  fragment is conformant. **Not taken from the RFC text alone** — three
  independent parsers were run on such a string (§ 10) and all three put the
  authority before the first `/`.
- The three GKE/`gcloud` claims from round 2 § 9 are unchanged and still
  unverified against a real GKE kubeconfig.
