# Phase 5 close, step 4 — the family review, measured

`k8s-admin`, 2026-09-28. Read-only: no cluster was stood up, nothing was built,
the test host was not touched. Every number below is re-derived on this machine
from the tree at `1a9a35f` unless a commit is named.

Regions read: `src/k8s.rs` § THE INGEST GUARD · WHAT WENT WRONG · THE STORE ·
WHAT A REPORT ASKS FOR · WHAT A NODE IS USING · RESOLVING AN OWNER · THE INITIAL
LIST · WHAT A THROTTLE LOOKS LIKE · HOW OLD A CLUSTER MAY BE · THE DRIVER ·
EVERY KIND THE CLUSTER SERVES · WHAT ELSE THE CLUSTER SERVES · CONNECTING · THE
SERVER'S OWN CERTIFICATE. `src/main.rs` `load`/`take`/`render`/`header`/`health`/
`check_switched_off`/`clock`/`greeting`/`command_log`/`connect_log`/`kubectl`/
`cluster_run`/`live`/`ask_owners`/`too_slow`/`pods_unread`/`read_so_far`.
`src/views.rs` `because`/`next_step`/`watching`/`DISCOVERY`. `src/analysis.rs`
`services_reaching_nothing`/`endpoints_behind`. `docs/security.md`
§ k8rs-readonly. `scripts/twin-guard.py`.

## 1 — the 51 String fields, re-derived twice

An independent parser (Python, written for this review, mirroring
`src/k8s_tests.rs`'s `declared_types`/`reachable_from`/`words`) run over
`src/rules.rs` at HEAD and at `cd8929e`, the commit that closed the ingest-bound
box.

```
$ python3 fields.py src/rules.rs            # HEAD
types parsed from rules.rs: 30
reachable from the three watched types: 14 ['Condition', 'ContainerRole',
  'ContainerSnapshot', 'ContainerState', 'ExitRule', 'HostPathMount',
  'NodeSnapshot', 'ObjectId', 'ObjectKind', 'PodSnapshot', 'Taint',
  'Terminated', 'Toleration', 'WorkloadSnapshot']
String fields: 52

$ git show cd8929e:src/rules.rs > rules-cd8929e.rs
$ python3 fields.py rules-cd8929e.rs
types parsed from rules.rs: 30
reachable from the three watched types: 14 [... same 14 ...]
String fields: 51
```

The one field between the two lists is `PodSnapshot.reason`.

The in-tree guard's own floor, `src/k8s_tests.rs:4367`:

```rust
assert!(
    checked.len() >= 45,
    "only {} String fields were derived, which is fewer than the snapshot types carry",
    checked.len()
);
```

## 2 — the fault count, by the enum's own command

`src/k8s.rs:834` carries the command in the doc comment. Run:

```
$ awk '/^pub enum Fault \{/,/^\}/' src/k8s.rs | grep -cE '^    [A-Z][A-Za-z]+,$'
11
$ awk '/^pub enum Fault \{/,/^\}/' src/k8s.rs | grep -E '^    [A-Z][A-Za-z]+,$'
    Kubeconfig,
    NoContext,
    BadEntry,
    NoCredential,
    Rejected,
    Expired,
    Refused,
    Gone,
    Conflict,
    Unfinished,
    Unanswered,
```

The Phase 5 box says *eight, not three*. Eleven at HEAD; `Conflict` (D213) and
`Unfinished` (2026-09-03) arrived after the phase, `Rejected` on 2026-08-30.

## 3 — the capability probe's group strings

Counted off `src/k8s.rs:4624-4646`: **7 variants, 8 group strings** — `Linkerd`
matches `linkerd.io` and `policy.linkerd.io`. Groups:
`metrics.k8s.io`, `policy` (kind `PodDisruptionBudget`), `cert-manager.io`,
`monitoring.coreos.com`, `networking.istio.io`, `linkerd.io` /
`policy.linkerd.io`, `cilium.io`.

## 4 — the six kubeconfig shapes

```
$ awk 'NR>=9421 && NR<=9800' src/k8s_tests.rs | grep -n 'shape [0-9]'
45:/// **`PRIOR-ART § B1` shape 1 — there is no kubeconfig on the disk at all.**
86:/// **`PRIOR-ART § B1` shape 2 — a kubeconfig with no `current-context:`**
155:/// **`PRIOR-ART § B1` shape 3 — `KUBECONFIG` holding several paths**
250:/// **`PRIOR-ART § B1` shape 4 — a context whose name contains a space**
292:/// **`PRIOR-ART § B1` shape 5 — a context that names its own namespace**
```

Shape 6 is named at `src/k8s_tests.rs:9435-9440` as covered by
`a_credential_plugin_that_never_answers_is_a_client_that_could_not_be_built` and
`a_login_program_that_dies_mid_session_is_a_credential_fault_and_not_a_network_one`.
Six, covered.

## 5 — what `exclude` ships, re-taken

Replicating cargo's rule (git-tracked files minus the `exclude` globs in
`Cargo.toml:31-40`):

```
$ python3 -  # git ls-files, minus exclude, sizes from the working tree
git-tracked files: 272
kept after exclude: 99 files, 8475903 bytes = 8.08 MiB
top-level files kept: ['CHANGELOG.md', 'Cargo.lock', 'Cargo.toml', 'LICENSE',
  'README.md', 'README_TR.md', 'test.md']
```

By directory:

```
  7435794     7262K  src
   688812      673K  tests
   147693      144K  CHANGELOG.md
    78103       76K  Cargo.lock
    43669       43K  test.md
    35149       34K  LICENSE
    16553       16K  Cargo.toml
    15595       15K  README_TR.md
    14535       14K  README.md
```

The box records **145 files / 5.2 MiB before, 90 / 4.3 MiB after**, measured
2026-08-30. `test.md` is tracked since 2026-09-28 and is not on the exclude
list.

## 6 — five typed lists against "six report fetches"

Not two numbers for one thing. `src/k8s.rs:2427`:

```rust
// The sixth on-demand list, filed one setter over because C3 owns it
// ([`Store::certificates_fetched`]).
certificate_requests: self.certificate_requests.clone(),
```

and `src/main.rs:3730` — *"The six lists a report asks for, fetched once and only
on a run that draws reports"*. Five typed lists + CSRs.

## 7 — `insecure-skip-tls-verify`, where it is and is not surfaced

```
$ grep -rn 'insecure\|accept_invalid_certs' src/*.rs | grep -v '_tests\.rs'
src/ui.rs:747:    pub insecure: bool,
src/ui.rs:1984:            screen.insecure.then_some(tls.as_str()),
src/ui.rs:3165:    } else if row.insecure {
src/k8s.rs:7639:    pub(crate) insecure: bool,          # k8s::Choice, the picker's row
src/k8s.rs:7792-7795                                  # read off cluster.insecure_skip_tls_verify
src/k8s.rs:9022:    probe.accept_invalid_certs = config.accept_invalid_certs;
src/main.rs:7506:    insecure: bool,                     # console::Console
src/main.rs:8793:fn tls_unverified(contexts: &[k8s::Choice]) -> bool
```

`k8s::Session`'s fields, read off `src/k8s.rs:7030-7180`:

```
client, version, served, watches, renewal, context, namespace, coverage,
client_certificate, skew, serving_expiry
```

No `insecure`. `main::Input` (`src/main.rs:521-594`): `snapshot, skipped, skew,
serving_expiry, analysis, unreadable, watch_trouble`. No `insecure`.
`greeting()` (`src/main.rs:2959-3022`) builds its clauses from
`session.version` and `session.served` only.

```
$ grep -rn 'TLS not verified' src/*.rs | grep -v _tests
src/ui.rs:321, 739, 1297, 1301, 1931, 2031, 3154
```

All seven are `ui.rs`. The value is in hand at `src/k8s.rs:7418`, where
`probe(&config)` reads `config.accept_invalid_certs` one line before
`Client::try_from(bounded_reads(config))` consumes the `Config`.

## 8 — the two version sentences

`src/k8s.rs:3905-3928`, both arms:

```rust
"This cluster is Kubernetes {major}.{minor}, and k8rs has only been checked \
 against 1.{OLDEST_SERVER} and newer. …"
"This cluster is Kubernetes {major}.{minor}, and this copy of k8rs was built \
 to understand 1.{TYPES_BUILT_FOR}. …"
```

Present since the box's own commit:

```
$ git log --oneline --format='%h %ad %s' --date=short -S'has only been checked against' -- src/k8s.rs
503e408 2026-08-22 feat(k8s): name the oldest cluster k8rs supports, and make a stuck first sync a state
$ git show 503e408 -- src/k8s.rs | grep -n 'This cluster is Kubernetes'
454:+            "This cluster is Kubernetes {major}.{minor}, and k8rs has only been checked against \
463:+            "This cluster is Kubernetes {major}.{minor}, and this copy of k8rs was built to \
```

`OLDEST_SERVER = 29` (`:3874`), `TYPES_BUILT_FOR = 36` (`:3882`). The integers
come from `minor_version`, so nothing the server wrote is echoed
(`src/k8s.rs:3897-3899`).

## 9 — the "One node check is off" line

```
$ git log --oneline --format='%h %ad %s' --date=short -S'One node check is off' -- src/main.rs
c9e40ea 2026-08-30 feat(k8s): give a namespace-scoped login a working tool instead of an empty one
$ git show c9e40ea -- src/main.rs | grep -n 'fn check_switched_off'
284:+fn check_switched_off(namespace_scope: Option<&str>) -> Option<String> {
```

`src/main.rs:1030` keys it on `namespace_scope`; `src/main.rs:903` draws it.
`src/main.rs:1022-1024` states it never rode on `Input::skipped`.

## 10 — the documented read-only role, resources counted

`docs/security.md:159-228`, resource entries:

```
""                      pods, pods/log, events, services, nodes,
                        persistentvolumeclaims                       6
apps                    deployments, statefulsets, daemonsets,
                        replicasets                                  4
policy                  poddisruptionbudgets                         1
certificates.k8s.io     certificatesigningrequests                   1
discovery.k8s.io        endpointslices                               1
metrics.k8s.io          nodes                                        1
authorization.k8s.io    selfsubjectrulesreviews,
                        selfsubjectaccessreviews                     2
                                                                   ---
                                                                    16
```

plus one `nonResourceURLs` rule. The box records *the 15 this role names*; the
`authorization.k8s.io` pair arrived with Phase 13's permission probe.

The box's deferral of the browser gap reads against `docs/architecture.md:10-16`:
*"Three reads are not [wired]: the browser's server-side `Table`, the four detail
tabs, and the log stream."*

## 11 — the command log, line by line against what is sent

`src/main.rs:3140-3177` and `:3230-3269` produce, for a cluster-wide
`--once --analysis` on context `c`:

```
$ kubectl --context c get --raw '/api/v1/pods?limit=1'
$ kubectl --context c get --raw /version
$ kubectl --context c api-resources --verbs=list
$ kubectl --context c get certificatesigningrequests
$ kubectl --context c get replicasets -A
$ kubectl --context c get services -A
$ kubectl --context c get endpointslices -A
$ kubectl --context c get persistentvolumeclaims -A
$ kubectl --context c get poddisruptionbudgets -A
$ kubectl --context c top nodes
$ kubectl --context c get pods -A --watch
$ kubectl --context c get nodes --watch
$ kubectl --context c get deployments -A --watch
$ kubectl --context c get statefulsets -A --watch
$ kubectl --context c get daemonsets -A --watch
```

Read as a user would type them: every flag is one `kubectl` accepts, `--context`
sits before the verb, the raw paths are single-quoted against globbing, `nodes`
and `certificatesigningrequests` carry no scope flag (`src/k8s.rs:8586` bounds
`scoped` to `NamespaceResourceScope`, so a node watch cannot be built
namespaced), and under `-n <ns>` the four namespaced watches and the five
namespaced report lists take `-n` while those two stay bare. No `--force`, no
flag the call does not carry. The two omissions are named in the source: the C2
TLS handshake (`src/main.rs:3113-3116`) and the second `/version` read for the
`Date` header (`:3047-3049`).

## 12 — formats over a kube error or a `Config`, hand-checked

The row `scripts/security-guard.py` prints as one it cannot close (D164).

```
$ grep -nE 'format!|writeln!|println!|panic!|expect\(' src/k8s.rs src/main.rs \
    src/views.rs src/ui.rs src/ops.rs \
  | grep -E '\{(error|failure|err|e|status|config|cfg|auth)[:}]' \
  | grep -vE '^\S+:[0-9]+:\s*//'
src/main.rs:658:            take(doc, &mut input).map_err(|e| format!("{named}: {e}"))?;
```

`take`'s error is a `String` this file assembled from a path and a
`serde_json::Error`, both already through `sanitize` (`src/main.rs:644-652`).

```
$ grep -nE '\{:\?\}|\{\}|to_string\(\)' src/k8s.rs \
  | grep -iE 'error|failure|config|err\b|fault|status|problem' \
  | grep -viE '^\s*[0-9]+:\s*(///|//)'
src/k8s.rs:5693:            Err(_) => {}
src/k8s.rs:6386:  serde_yaml_ng::to_string(&self.0).map_err(|failed| failed.to_string())
```

`6386` is the YAML emitter's own error over an already-masked tree
(`src/k8s.rs:6382-6384`); not a `kube` type.

`main.rs`'s four `format!("k8rs: {problem}")` sites — `:421`, `:4417`, `:7003`,
`:9187` — are all `wall_clock()` or `load()` `String`s.

No struct in `main.rs` holds a `k8s::Session`; `Session`, `Store` and `Trouble`
derive no `Debug` (`src/k8s.rs:7022-7030`, `:1995-1998`, `:1793-1797`).

## 13 — the untrusted-field list the phase's gate names

Each of the three, found in an `impl Bounded`:

```
src/k8s.rs:386-389   ContainerState::Waiting { reason, message }  IDENTIFIER / FREE_TEXT
src/k8s.rs:460-462   PodSnapshot.finalizers                       IDENTIFIER
src/k8s.rs:422-424   HostPathMount.path / sub_path / sub_path_expr FREE_TEXT
```

`IDENTIFIER = 512`, `FREE_TEXT = 4096` (`src/k8s.rs:205`, `:213`), cut marked
`… (shortened by k8rs)` (`:220`). `unprintable` (`:251`) is `char::is_control`
plus `U+00AD`, `U+200B..=200F`, `U+202A..=202E`, `U+2060..=206F`, `U+FEFF`; a
whitespace control becomes one space, everything else is removed (`:284-308`).

## 14 — the twin constants

```
$ grep -n 'TWINS = ' -A 4 scripts/twin-guard.py
46:TWINS = {
47-    "SKEW_ALLOWANCE": (("src/rules.rs", "src/k8s.rs"), "NOTES § D69, § D176"),
48-    "CERT_EXPIRY_WARN": (("src/rules.rs", "src/k8s.rs"), "NOTES § D178"),
49-}
$ grep -n 'const SKEW_ALLOWANCE' src/rules.rs src/k8s.rs
src/rules.rs:270:const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);
src/k8s.rs:8339:const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);
```

One draw site for the clock sentence:

```
$ grep -rn 'disagree about the time' src/*.rs | grep -v _tests
src/main.rs:1101, src/main.rs:1107
$ grep -n 'clock(' src/main.rs | grep -v 'wall_clock\|local_clock'
842:    if let Some(clock) = clock(input.skew) {
1087:fn clock(skew: Option<SignedDuration>) -> Option<String> {
8328:    console.clock = clock(session.skew);
```

Rounding is half-up over the magnitude (`src/main.rs:1095-1096`); `measure`
returns the raw duration and refuses a non-2xx (`src/k8s.rs:8271-8272`).

## 15 — two backlog entries, still live at HEAD

**`410` desync.** `src/k8s.rs:1129-1147` matches 410 on no code arm and
`"Expired"` on no reason arm, so `Fault::Unanswered`. The path a reader gets:
`src/views.rs:4229` → *"nothing usable came back when k8rs tried to `list` and
`watch` pods"*, and `src/views.rs:4314-4317` → *"Check the server address this
kubeconfig names, and that this machine can reach it."* `src/main.rs:988-990`
withholds `○ nothing is broken` for the window. Behaviour pinned by
`a_page_that_fails_restarts_the_list_and_the_pages_before_it_never_land`
(`src/k8s_tests.rs:2138`), which feeds `Status{code:410, reason:"Expired"}`.
Entry: `backlog.md:1496`.

**`ExternalName` Service with a leftover selector.**
`src/analysis.rs:1797-1805` filters on `!service.selector.is_empty()` and
nothing else; `ServiceSnapshot` carries no `type`. Entry: `backlog.md:1511`.

## 16 — what could not be re-taken here, and the command that would

- **10 000-pod resident set.** Box: 128 844 KiB peak / 125 704 KiB steady.
  Needs a cluster and `K8RS_CLUSTER=review`; not run (the PM's fixture cluster
  is up). Standing evidence:
  `reports/2026-08-28-ten-thousand-pod-resident-set.md`.
- **`62 of 62 declarations parsed` / `49 can hold a token`.** Guards run on the
  test host. `ssh ubuntu 'cd ~/k8rs-src && python3 scripts/security-guard.py'`
  — the summary line carries the denominator.
- **Mutation counts** (22/27/5 mutants per box, `651 tests, 0 missed`).
  `just mutants` over `rules.rs`/`analysis.rs` is the phase-close gate only when
  the phase touched them; `git log --oneline -- src/rules.rs src/analysis.rs`
  since the last sweep is what decides.
- **`cargo package --list`** would confirm § 5 against cargo's own rule rather
  than a replication of it: `ssh ubuntu 'cd ~/k8rs-src && cargo package --list
  --allow-dirty | wc -l'`.
