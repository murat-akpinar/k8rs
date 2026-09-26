# 2026-09-19 — `views::Stripped` and the header's connection word, measured

Operator review of the uncommitted Phase 12 box (`src/views.rs`, `src/ui.rs`
and their test files). Every frame below was rendered on the test host
(`ssh ubuntu`, NOTES § D267) from a `rsync` copy of the working tree at
`~/k8rs-review` with `CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target`; both
were removed from a shell trap when the run ended. No cluster was started —
nothing in this box reads one.

Four probe `#[test]`s were appended to the **copy's** `src/ui_tests.rs` to
render frames the committed tests do not print. They were never written into
the repository and went with the copy.

## Setup

```
$ rsync -a --delete --exclude=/target ~/GIT/k8rs/ ubuntu:k8rs-review/
$ ssh ubuntu 'cd ~/k8rs-review && touch src/*.rs'
$ ssh ubuntu 'cd ~/k8rs-review && CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --bin k8rs --no-run'
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3m 13s
[exited with code 0]
```

Teardown, from the trap:

```
--- before ---
91M	/home/murat/k8rs-review
2.5G	/home/murat/.cache/k8rs-review-target
k8rs-src untouched, still present
--- after ---
ls: cannot access '/home/murat/k8rs-review': No such file or directory
ls: cannot access '/home/murat/.cache/k8rs-review-target': No such file or directory
```

## M1 — the header once `esc` has dismissed the failure box

`ui::header` drops the joined connection word only while
`views::Modal::Unconnected` is open (`let failed = matches!(…)`).
`views::App::escape` clears that modal for a `Before::Connected` (the
`Some(_) => {}` arm), and the probe confirms it. The caller's zone is the page's
own, `screens/context.md` § When the new cluster does not work less the
` · admin` the header joins.

```
$ cargo test --bin k8rs -- ui::tests::probe_the_failure_header_after_esc --nocapture

context.md § When the new cluster does not work draws:
   ctx: staging · ⚠ not allowed · admin

box open:
                                      k8rs  ctx: staging · ⚠ not allowed · admin

after esc, app.modal is false
Connecting:
                              ctx: staging · ⚠ not allowed · connecting… · admin
Live:
                                     ctx: staging · ⚠ not allowed · live · admin
Lost:
                 ctx: staging · ⚠ not allowed · ⚠ disconnected, retrying · admin
Expired:
                          ctx: staging · ⚠ not allowed · ⚠ login expired · admin
```

Field values the finding turns on: `ui.rs:1702`
`let failed = matches!(&app.modal, Some(views::Modal::Unconnected { .. }))`;
`views.rs:2998` `escape`'s `Some(_) => {}` arm; `Link` has four variants and
`Link::state` returns a non-empty `String` for every one of them.

## M2 — the clock pointer `screens/states.md` draws in the header

```
$ cargo test --bin k8rs -- ui::tests::probe_the_clock_pointer_in_the_header --nocapture

states.md draws: ctx: prod-eu · live · admin · ⚠ your clock is behind
states.md draws: nodes 3/3       ctx: prod-eu · live · admin · ⚠ your clock is behind
states.md draws: nodes 3/3        ctx: prod-eu · live · admin · ⚠ your clock is ahead
states.md draws: nodes 3/3       ctx: prod-eu · live · admin · ⚠ your clock is behind
k8rs draws: nodes 3/3                            k8rs           ctx: prod-eu · live · admin
```

The frame fed to `k8rs draws` carries `Screen::clock = Some(…)` — the same
sentence `screens/states.md` § Your computer's clock is off puts in the pane.

```
$ grep -rn "your clock" src/*.rs | grep -v _tests
src/theme.rs:270:/// `⚠ Your login expired.`, `⚠ your clock is behind` / `ahead` and `⚠ This computer and the
src/main.rs:733:    // (`screens/once.md` § When your clock and the cluster's disagree): it is *how much of this
src/main.rs:943:/// `screens/once.md` § When your clock and the cluster's disagree verbatim, re-wrapped.

$ grep -c "your clock is" screens/states.md
8
```

`screens/widgets.md` § 1a's zone table lists six segments and the clock pointer
is not one of them: *context · namespace scope · connection state ·
`admin`/`read-only` · a TLS warning · `changing…`*.

## M3 — the command log, four shapes, drawn at the 80×24 floor

`raw` is what `views::Log` holds; `drawn` is the strip row out of the rendered
buffer, `|`-delimited at the frame's border.

```
$ cargo test --bin k8rs -- ui::tests::probe_the_command_log_shapes --nocapture

raw   [1 long -n, running] "$ kubectl scale deployment/checkout-api --replicas=3 -n payments-prod-eu-west-1-generated   …" (93 cols)
drawn [1 long -n, running] |│ $ kubectl scale deployment/checkout-api --replicas=3 -n payments-prod...   … │|

raw   [1 long -n, rejected] "$ kubectl scale deployment/checkout-api --replicas=3 -n payments-prod-eu-west-1-generated   → rejected" (102 cols)
drawn [1 long -n, rejected] |│ $ kubectl scale deployment/checkout-api --replicas=3 -n paym...   → rejected │|

raw   [2 --subresource] "$ kubectl get deployment checkout-api -n payments --subresource=scale -o yaml" (77 cols)
drawn [2 --subresource] |│ $ kubectl get deployment checkout-api -n payments --subresource=scale...     │|

raw   [3 at the 76-column bound, running] "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0   …" (80 cols)
drawn [3 at the 76-column bound, running] |│ $ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payment...   … │|

raw   [3 at the 76-column bound, login expired] "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0   → login expired" (94 cols)
drawn [3 at the 76-column bound, login expired] |│ $ kubectl get pod ...ckout-api-canary-7d9f4bc86d-x2k9pqrst   → login expired │|

raw   [4 nothing ever answered] "$ kubectl --context staging get pods -A --watch   …" (51 cols)
raw   [4 nothing ever answered] "$ kubectl get events -n payments --field-selector involvedObject.name=web   …" (77 cols)
drawn [4 nothing ever answered] |│ $ kubectl --context staging get pods -A --watch   …                          │|
drawn [4 nothing ever answered] |│ $ kubectl get events -n payments --field-selector involvedObject.name...   … │|
```

The gap is three columns in every raw line; the outcome is reserved before the
command is cut in every drawn line; `…` becomes `→ <word>` in place.

## M4 — `Log::sent`'s `trim_end` against `k8s::text`'s own split

The comment added at `views.rs:1671` says `trim_end` is *"the same split
`k8s::text` uses to decide what becomes a space"*. `char::is_whitespace` and
`k8s::unprintable` are read off each character and the two halves of `Log`
compared on the same input.

```
$ cargo test --bin k8rs -- ui::tests::probe_trim_end_against_the_ingest_strip --nocapture

LF                         is_whitespace=true  unprintable=true
    ran  -> "$ kubectl get pods -n payments"
    sent -> "$ kubectl get pods -n payments   …"
TAB                        is_whitespace=true  unprintable=true
    ran  -> "$ kubectl get pods -n payments"
    sent -> "$ kubectl get pods -n payments   …"
NBSP U+00A0                is_whitespace=true  unprintable=false
    ran  -> "$ kubectl get pods -n payments\u{a0}"
    sent -> "$ kubectl get pods -n payments   …"
LINE SEPARATOR U+2028      is_whitespace=true  unprintable=false
    ran  -> "$ kubectl get pods -n payments\u{2028}"
    sent -> "$ kubectl get pods -n payments   …"
IDEOGRAPHIC SPACE U+3000   is_whitespace=true  unprintable=false
    ran  -> "$ kubectl get pods -n payments\u{3000}"
    sent -> "$ kubectl get pods -n payments   …"
EM SPACE U+2003            is_whitespace=true  unprintable=false
    ran  -> "$ kubectl get pods -n payments\u{2003}"
    sent -> "$ kubectl get pods -n payments   …"
ZERO WIDTH SPACE U+200B    is_whitespace=false unprintable=true
    ran  -> "$ kubectl get pods -n payments"
    sent -> "$ kubectl get pods -n payments   …"
```

Four of the seven — U+00A0, U+2028, U+3000, U+2003 — are `is_whitespace` and
**not** `unprintable`: `trim_end` removes them and `k8s::text` keeps them. The
gap is three columns in all seven cases.

## M5 — real cluster strings through `Stripped::of`

```
$ cargo test --bin k8rs -- ui::tests::probe_real_strings_through_the_strip --nocapture

GKE context                kept  in="gke_my-project-123456_europe-west1-b_prod-cluster-01"
EKS ARN                    kept  in="arn:aws:eks:eu-west-1:123456789012:cluster/production-eu"
AKS context                kept  in="prod-eu-aks-admin"
generated namespace        kept  in="payments-pr-4821-ephemeral-7d9f4bc86d"
253-char Deployment name   kept  (253 bytes in, 253 bytes out)
server sentence            kept  in="admission webhook \"vpa.k8s.io\" denied the request: pod \"web\" is invalid"
openshift project          kept  in="openshift-cluster-node-tuning-operator"
a name with a soft hyphen  CHANGED in="prod\u{ad}eu"                     out="prodeu"
a Persian namespace        CHANGED in="دی\u{200c}تابیس"                  out="دیتابیس"
an emoji family in a note  CHANGED in="👨\u{200d}👩\u{200d}👧"            out="👨👩👧"
a tab in a message         CHANGED in="quota exceeded:\tcpu"             out="quota exceeded: cpu"
```

`k8s::FREE_TEXT` is 4096 bytes; the longest value above is 253.

## M6 — the header's right zone at 80 columns, per link state

Context zone handed over:
`ctx: arn:aws:eks:eu-west-1:123456789012:cluster/production-eu · ns: payments`,
`Writes::ReadOnly`, vitals `nodes 3/3 (40s ago)`.

```
$ cargo test --bin k8rs -- ui::tests::probe_the_header_at_eighty_columns --nocapture

Connecting / plain
|…t-1:123456789012:cluster/production-eu · ns: payments · connecting… · read-only|
Connecting / tls + changing
|…on-eu · ns: payments · connecting… · read-only · ⚠ TLS not verified · changing…|
Live / plain
|…:eu-west-1:123456789012:cluster/production-eu · ns: payments · live · read-only|
Live / tls + changing
|…roduction-eu · ns: payments · live · read-only · ⚠ TLS not verified · changing…|
Lost / plain
|…012:cluster/production-eu · ns: payments · ⚠ disconnected, retrying · read-only|
Lost / tls + changing
|…ayments · ⚠ disconnected, retrying · read-only · ⚠ TLS not verified · changing…|
Expired / plain
|…123456789012:cluster/production-eu · ns: payments · ⚠ login expired · read-only|
Expired / tls + changing
|…u · ns: payments · ⚠ login expired · read-only · ⚠ TLS not verified · changing…|
```

The `ctx: ` label is inside the front-cut in all eight rows. In the four
`tls + changing` rows nothing of the cluster's own name is left.

A first run of this probe forgot `screen.link = link` and printed `live` under
all four states; the line was added and the run repeated. The output above is
the second run.

## M7 — the committed header test, for comparison

```
$ cargo test --bin k8rs the_header_of_every_link_state_is_the_pages_own -- --nocapture

## Still loading · Live
 nodes …                              k8rs    ctx: prod-eu · connecting… · admin
## The connection dropped · Live
 nodes 3/3 (40s ago)             ctx: prod-eu · ⚠ disconnected, retrying · admin
## Your login expired · Live
 nodes 3/3 (2 min ago)                    ctx: prod-eu · ⚠ login expired · admin
test ui::tests::the_header_of_every_link_state_is_the_pages_own ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1459 filtered out
```

## Source lines the findings quote

- `src/ui.rs:1702` — `let failed = matches!(&app.modal, Some(views::Modal::Unconnected { .. }));`
- `src/ui.rs:1557` — `Link::Live | Link::Connecting => clock(screen).map(…)` in `withheld`
- `src/ui.rs:3441` — `.filter(|_| screen.link == Link::Live)` in `clock`
- `src/ui.rs:3141` — `Pane::Ready(cards) if cards.is_empty() && screen.link == Link::Live`
- `src/ui.rs:1394` — `let switch = screen.link == Link::Expired;`
- `src/views.rs:1671` — `self.push(format!("{}{OUTCOME_GAP}{RUNNING}", line.trim_end()));`
- `src/views.rs:1531` — `const OUTCOME_GAP: &str = "   ";`
- `src/views.rs:1391–1452` — `Dialog`'s caller-built `String` fields: `consequence`, `warning`, `kubectl`, `asks`
- `src/k8s.rs:7375` — `fn drawable(mut value: String) -> Option<String> { text(&mut value, IDENTIFIER); … }`
- `src/k8s.rs:7469` — `drawable(context?.namespace.clone()?)`
- `src/k8s.rs:1173` — `fn message(status: &Status) … text(&mut said, FREE_TEXT)`
- `src/ops.rs:1456` — `fn cleaned(value: &str, cap: usize)`, which is `k8s::text`
- `src/main.rs:2051` — `Some(Some(value)) if !k8s::namespace_name(value) => return Some(not_a_namespace(…))`
