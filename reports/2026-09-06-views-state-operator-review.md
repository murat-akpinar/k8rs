# `views.rs` read against the screens — the measurements behind the Phase 10 operator review

`k8s-admin`, 2026-09-06. No cluster was brought up; the family is pure state
with no network in it. Every number below came out of the landed code, run in a
throwaway copy of the tree.

## How it was run

The working tree is the PM's, so the probes were appended to a copy and the copy
was given its own target directory:

```
$ rsync -a --exclude target --exclude .git --exclude tmp /home/shyuuhei/GIT/k8rs/ $SCRATCH/tree/
$ cd $SCRATCH/tree && CARGO_TARGET_DIR=$SCRATCH/target cargo test --quiet review_ -- --nocapture
```

Probes were appended to the copy's `src/views_tests.rs` and exist nowhere else.
Nothing under `src/`, `tests/` or `scripts/` in the real tree was touched.

```
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 1131 filtered out; finished in 0.08s
exit=0
```

## 1. `Card::count()` on the cards `screens/alerts.md` draws with no count

Input: one finding whose `owner` and `object` are the same `ObjectId`, kind
`Pod` — the bare-pod and mirror-pod shape (`rules.rs` § `ObjectId`,
NOTES § D39).

```
bare pod card:   affected=1 count=Some("1 pods")
mirror pod card: count=Some("1 pods")
```

`screens/alerts.md` draws `default/broken-hostpath` (line 468) and
`default/broken-pending` (line 456, and again at line 903) with the identity
line ending after the name, and states the rule twice — *"It is a bare pod, so
no owner and no `n of m`"* (line 430) and *"a bare pod, so there is no owner and
no `n of m`"* (line 900).

## 2. `Card::count()` when the numerator runs past the denominator

`total` is `WorkloadSnapshot::desired`, i.e. `spec.replicas`; `affected` counts
distinct pod `object`s carrying a finding.

Input: `spec.replicas: 1`, two distinct pods under the same owner (the
stuck-`Terminating` + replacement shape, rules 12 and any pod rule):

```
stuck-terminating card: count=Some("2 of 1 pods")
```

The committed test `a_card_with_more_affected_pods_than_the_workload_wants_is_not_clamped`
(`src/views_tests.rs:1114`) asserts the same shape at four pods against
`desired: 3`:

```
Some("4 of 3 pods")
```

Two neighbouring texts about the same screen:

- `screens/alerts.md:1029-1035` — on `unavailableReplicas`: *"a denominator here
  would eventually print `2 of 1 pod not answering`, which is the same shape as
  the `2 of 1 pod ready` D82 records as a number on a screen whose whole promise
  is that its numbers can be believed."*
- `PRIOR-ART.md` § F2 (tagged **covered**) — *"never divide by a denominator that
  is not guaranteed complete… undefined prints as `n/a`, not as a large number."*

## 3. A future timestamp: the sort and the age disagree about which findings have one

`rules::age` returns `None` past `SKEW_ALLOWANCE` (`src/rules.rs:270`, five
minutes ahead). `views::newest_first` (`src/views.rs:363`) compares
`Finding::timestamp` directly and never consults `now` — `cards()` takes no
`now`.

Input: two Critical cards, one whose newest finding is 240 s in the past, one
whose newest finding is 600 s in the future (a node clock ahead of the reader's,
or a reader's laptop behind after a suspend).

```
card  skewed age=None            newest_stamp=Some(Some(Time(2023-11-14T22:23:20Z)))
card     web age=Some("4 min ago") newest_stamp=Some(Some(Time(2023-11-14T22:09:20Z)))
```

`out[0]` is `skewed`, so the card that draws a blank right edge sorts first
inside its band. `screens/alerts.md:120-126`: *"Cards with no age sort last
inside their severity band."*

Second input: one card, two findings — one 240 s in the past, one 600 s in the
future.

```
one card, two findings (4 min ago + 10 min ahead): age=None
```

`Card::age` is `self.newest()?.age(now)` (`src/views.rs:275`), so the drawable
age on the same card is not reached.

## 4. `Cursor` after a shrink with no `follow()`

`Cursor::selected` clamps the index for the answer and does not write it back
(`src/views.rs:167-169`); `Cursor::up` subtracts from the unclamped field
(`src/views.rs:185`).

Input: cursor set at index 49 of a 50-row list, then asked about a 3-row list.

```
before up: selected=Some(2)
after  up: selected=Some(2)
```

The second `up` moves to `Some(1)`.

## 5. Where `Group::of` puts kinds a stock cluster serves

Run through `Group::of` with `Browsable { group, plural, namespaced }` as
transcribed from the API groups by hand — **not** off a discovery capture, and
there is no committed one in `tests/fixtures/`:

```
           pods                             -> workloads
metrics.k8s.io pods                             -> workloads
metrics.k8s.io nodes                            -> cluster
           nodes                            -> cluster
    policy poddisruptionbudgets             -> cluster
rbac.authorization.k8s.io roles                            -> cluster
           serviceaccounts                  -> config
resource.k8s.io resourceclaims                   -> workloads
resource.k8s.io deviceclasses                    -> cluster
apiserverinternal.k8s.io storageversions                  -> cluster
storagemigration.k8s.io storageversionmigrations         -> cluster
cert-manager.io certificates                     -> workloads
monitoring.coreos.com servicemonitors                  -> workloads
networking.istio.io virtualservices                  -> workloads
argoproj.io applications                     -> workloads
external-secrets.io externalsecrets                  -> workloads
kustomize.toolkit.fluxcd.io kustomizations                   -> workloads
           endpoints                        -> network
           resourcequotas                   -> config
scheduling.k8s.io priorityclasses                  -> cluster
admissionregistration.k8s.io validatingwebhookconfigurations  -> cluster
coordination.k8s.io leases                           -> cluster
events.k8s.io events                           -> cluster
           events                           -> cluster
autoscaling horizontalpodautoscalers         -> workloads
     batch cronjobs                         -> workloads
networking.k8s.io ingresses                        -> network
networking.k8s.io networkpolicies                  -> network
           persistentvolumes                -> storage
storage.k8s.io storageclasses                   -> storage
```

The first column is the API group, empty for the core one. Five of these rows
belong to groups that ship with Kubernetes or with metrics-server and are not
CustomResourceDefinitions: `metrics.k8s.io`, `resource.k8s.io`,
`apiserverinternal.k8s.io`, `storagemigration.k8s.io`. They reach `Group::of`'s
final two arms (`src/views.rs:510-511`), whose doc reads *"Anything left is a
CRD"*.

`k8s.rs` sorts the browsable list by plural, then group, then version
(`src/k8s.rs:4312-4314`), so the two rows spelled `pods` are adjacent.

The claim in step 2 of the same doc — a CRD's group must contain a dot — is the
apiextensions validation `should be a domain with at least one dot`. Read from
the API's documented validation, not measured here.

## 6. Two costs, measured because they were about to be guessed

`cards()`, release build, one card per owner (the failing-Job shape), findings
and workloads both at `n`:

```
cards(): 100 findings / 100 cards   -> 158.606µs
cards(): 500 findings / 500 cards   -> 2.733155ms
cards(): 1000 findings / 1000 cards -> 6.381088ms
cards(): 2000 findings / 2000 cards -> 17.22685ms
```

`Filters::matches`, release build, one keystroke over a 5000-row table of five
cells each, text filter `web`, namespace filter empty:

```
Filters::matches over 5000 rows x 5 cells -> 1.117073ms (0 kept)
```

## 7. What the screens ask for that `App` and `Modal` have no field for

Read, not run:

- `screens/context.md:14` — *"One modal, one list, one key map"* for the cluster
  switcher, opened by `X` and also at startup. `views::Modal` is
  `Help | Confirm(Dialog)` (`src/views.rs:610-615`).
- `screens/detail.md:3` — `⏎` opens an object; the footer carries `c container`,
  `⇧p previous`, `/ search`, `esc back`. `views::View` is
  `Alerts | Resources(usize) | Analysis(usize)` (`src/views.rs:692-700`);
  `App` carries `tab`, `scroll` and `following` and no object, no container, no
  previous flag, no per-pane search, and no *is the detail pane open*.
- `todo.md:4286-4291` (a Phase 11 box) — *"A dialog tracks its object while
  open… **It holds the `uid`**"*. `views::Dialog` (`src/views.rs:627-657`) has
  `object`, `namespace`, `consequence`, `kubectl`, `verdict`, `asks`, `typed`.
- `todo.md:4252` — **Frozen after:** `views.rs`.

`View::Resources(usize)` and `NavItem::Kind(usize)` index the discovery list
`sidebar()` was given. `App::open` resets `App::content` only when
`self.view != before` (`src/views.rs:829`).

## Teardown

Nothing was created in the repository except this file. The copied tree and its
target directory live only in this session's scratchpad, outside the repo and
outside git; deleting them was refused by the sandbox, so they expire with the
session rather than by hand. `git status --short` over the real tree is
unchanged by this run.
