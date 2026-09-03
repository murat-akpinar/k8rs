# The never-listed / stale mutation, and what an empty node list is read as — 2026-08-30

`k8s-admin`, operator review of the test owed by the namespace-scoping box
(todo.md § Phase 5). No cluster was used: every run below is the built binary or
the test binary on this machine.

## M1 — the box's stated mutation, re-measured at HEAD

The box body names `MISSED src/main.rs:1646:25: delete ! in live_report`. The two
`!` in that function, read off the file:

```
$ sed -n '1641,1646p' src/main.rs
    let never_listed: Vec<ObjectKind> = troubles
        .iter()
        .filter(|trouble| !trouble.listed)      # 1643:27
        .map(|trouble| trouble.kind.clone())
        .collect();
    let watch_trouble = !troubles.is_empty();   # 1646:25
```

`1643:27` applied to a copy of the tree (tracked files plus the working
`src/main_tests.rs`), with its own `CARGO_TARGET_DIR` outside the repo:

```
$ sed -i '1643s/|trouble| !trouble\.listed/|trouble| trouble.listed/' <copy>/src/main.rs
$ CARGO_TARGET_DIR=<copy-target> cargo test --quiet --bins
test result: FAILED. 710 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
failures:
    tests::a_watch_that_never_listed_is_unreadable_and_one_that_listed_and_broke_is_merely_stale
    tests::a_watch_that_stops_delivering_is_a_line_in_the_report_and_so_is_its_recovery
```

The two failures print different symptoms for the same mutation.

The new test, header line and assertion message:

```
0 nodes

panicked at src/main_tests.rs:2282:5:
the pod watch listed and then broke, and its measured count was dropped as if it
had never been read: "0 nodes"
```

The neighbour, same mutation:

```
panicked at src/main_tests.rs:2133:5:
the cards under the warning are not the report: ""
```

`cargo test --quiet --lib` on this package is `error: no library targets found in
package k8rs` — `--bins` is the invocation (no lib target, D50).

## M2 — the new test's printed report, unmutated

```
$ CARGO_TARGET_DIR=<target> cargo test --quiet \
    a_watch_that_never_listed_is_unreadable -- --nocapture
[(Pod, true), (Node, false), (Deployment, false), (StatefulSet, false), (DaemonSet, false)]
▲ k8rs is not getting pods from this cluster: the role this kubeconfig uses needs to `list` and `watch` pods. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting nodes from this cluster: the role this kubeconfig uses needs to `list` and `watch` nodes. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting Deployments from this cluster: the role this kubeconfig uses needs to `list` and `watch` deployments. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting StatefulSets from this cluster: the role this kubeconfig uses needs to `list` and `watch` statefulsets. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting DaemonSets from this cluster: the role this kubeconfig uses needs to `list` and `watch` daemonsets. It keeps asking, and until that works nothing here about them can be trusted

0 pods
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 711 filtered out
```

Field values the finding turns on: the printed report's last line is `0 pods`,
with no age suffix; the pod watch behind it has `complete == true` and
`failure == Some(403)`. `711 filtered out` + 1 run = 712 unit tests; the box body
records 711 at the commit before this test.

## M3 — the analysis panes on the file-driven path

The shipped binary, one committed pods-only fixture, no cluster and no
kubeconfig read at all:

```
$ target/debug/k8rs --analysis tests/fixtures/crashloop.json
1 pod · 0 nodes

● default/broken-crashloop · 9 days ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  [evidence line, elided]
  → [action line, elided]

▲ default/broken-crashloop · 9 days ago
  The last run on record failed — exit 1 (the application's own error)
  [evidence line carries the fixture's verbatim controller message, elided]
  → [action line, elided]

1 critical, 1 warning
[capacity]
  What each node promised, and what it has
  Not checked. Reading what a node has needs permission to list nodes, and this login does not have it.
  Ask for permission to list nodes across the whole cluster.
  Still counted, from what you can see:
    1 workload has no memory or CPU limit
      Nothing stops one taking a whole node.
...
[drain safety]
  If you drained each node, what happens?
  Not checked. This report answers one question per node, and this login cannot list the nodes.
  Ask for permission to list nodes across the whole cluster.
...
[waste]
  Things that cost you something for nothing
  Not checked. Working out what is going to waste needs the lists of what this cluster has — its Services, the addresses behind them, the disk reservations and the replicasets — and this login could not read any of them.
  Ask for permission to list services, endpointslices, persistentvolumeclaims and replicasets.

[versions]
  What version everything here is running
  Versions
  Not checked. Every answer on this pane is measured against the version the control plane is running, and k8rs could not read it.
  Check that the cluster's API server is answering — this is the one number it tells anyone who can reach it.
exit=0
```

The three fields the finding turns on, all in one report: the header prints
`0 nodes`; `[capacity]` prints *this login does not have it*; `[drain safety]`
prints *this login cannot list the nodes*. `[versions]` names no login.

The three sites those strings are keyed on:

```
$ grep -n "nodes.is_empty()" src/analysis.rs src/rules.rs
src/analysis.rs:410:    snapshot.nodes.is_empty().then(|| Row::NotComputed {
src/analysis.rs:808:    if snapshot.nodes.is_empty() {
src/analysis.rs:2837:    if snapshot.nodes.is_empty() {
src/rules.rs:7232:    if nodes.is_empty() {
```

`ClusterSnapshot`'s field list (`src/rules.rs:1715-1790`) carries `namespace_scope`
and no field naming a kind whose list could not be read; `Input::unreadable`
(`src/main.rs:~338`) is `main.rs`-local and is not passed to `reports()`
(`src/main.rs:1685`).

## M4 — the static guard over the tree with the new test in it

```
$ python3 scripts/security-guard.py
security-guard: workflows — 1 workflow(s), 8 action(s) — OK
security-guard: no shell — 2 file(s) spawn a process — OK
security-guard: no second outbound path — 8 direct dependencies, 44980 code lines read — OK
security-guard: token hygiene — 62 structs, 18 enums, 8 aliases (80 of 80 declarations parsed), 8 can hold a token — OK
security-guard: credentials come from the kubeconfig — the class is empty — OK
security-guard: TLS verification is never disabled by us — the class is empty — OK
exit=0
```

## Not measured

* `just check` with the new test in the tree. Not run — that is `tester`'s gate,
  and this was a review measurement.
* Anything needing a cluster: the live shapes where a node list is empty because
  the watch was refused (`Fault::Refused`) or the login expired (`Fault::Expired`)
  before it ever listed. Reasoned from `Watch::settled` / `Fault::standing`, not
  observed.
* The `403` body a real API server sends for a refused `list`. The stub in
  `refusing()` omits `status.details` and sends `status.message` = `forbidden`;
  `reports/2026-08-29-namespace-scope-under-a-real-role.md` § R9 has the real
  `message` shape and NOTES § D160 the real `details` for a `nonResourceURL`.

## Left behind

`/home/shyuuhei/.cache/k8rs-review-mut` — a 7.9M copy of the tracked tree used
for M1, restored to HEAD's `src/main.rs` afterwards. Both scratch
`CARGO_TARGET_DIR`s were `cargo clean`ed (2.4 GiB + 1.4 GiB). `rm` is refused by
this session's permission system, so the directory itself is the PM's to remove.
