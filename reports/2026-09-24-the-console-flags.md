# The console flags — operator read (`--read-only`, `--context`, `--namespace`/`-n`)

`k8s-admin`, step 6, 2026-09-24. Reviewed the uncommitted working tree on
`development` (`src/main.rs`, `src/main_tests.rs`, `src/ui.rs`).

**No cluster was started and nothing ran on the test host** — the mirror was held by
`tester` for this round, so every command below is a read over the tree. Where a
claim could only be settled by a run, it is marked as such and the command is
named rather than executed.

## What was read

```
$ git diff --stat
 src/main.rs       | 442 ++++++++++++++++++++++++++++++++++++++----------------
 src/main_tests.rs | 347 ++++++++++++++++++++++++++++++++++++++++++
 src/ui.rs         |  19 +--
 3 files changed, 669 insertions(+), 139 deletions(-)
```

Regions read whole: `main()`'s console arm (`src/main.rs:117-208`), `run`/
`at_a_keyboard` (`:388-461`), `live_context`/`context_arg`/`namespace_arg`/
`live_namespace` (`:1774-1901`), `Opening`/`opening`/`console_flag`/
`cluster_reader` (`:1903-1991`), `mistyped` (`:2169-2511`), `command_log`/
`connect_log` (`:3028-3108`), `value_of` (`:4216-4234`), `ops_connected` +
`current_server` (`:6911-6980`), `audit_log_for`/`console` (`:7476-7768`),
`drawn` (`:8360-8467`), `notes` (`:8581-8609`), `keyed`/`wanting`/`mutating`
(`:8636-9300`), `zone`/`scoped` (`:8087-8105`); `src/ui.rs` `Writes` and its
impl (`:832-899`), `offered`/`withheld`/`help`/`header`/`shortened`
(`:1502-1896`), `note`/`calm` (`:3645-3717`); `src/views.rs::may_mutate`
(`:2826`); `src/k8s.rs` `Coverage`/`coverage`/`connect_with`/
`kubeconfig_context` (`:6740-6898`, `:7273-7402`);
`screens/widgets.md` § 1a, `screens/states.md` § Nothing is broken,
§ You can only see some namespaces, § The command line's own synopsis,
`screens/context.md` § What the command log shows, `NOTES.md` § D21.

## Measurement 1 — the console's two reads of *which cluster*

```
$ sed -n '7550,7555p' src/main.rs
        Ok(kubeconfig) => {
            contexts = k8s::contexts(&kubeconfig, opening.context);
            insecure = tls_unverified(&contexts);
            server = current_server(&kubeconfig);
            k8s::connect_with(kubeconfig, opening.context, opening.namespace).await
        }
```

```
$ sed -n '6971,6973p' src/main.rs
fn current_server(kubeconfig: &kube::config::Kubeconfig) -> String {
    k8s::contexts(kubeconfig, None)
        .into_iter()
```

The same function's doc, two lines above (`src/main.rs:6960-6964`):

> **The `server:` the audit line names**, off the same kubeconfig the connection
> is built from (NOTES § D220 ruling 5). **A context name does not identify a
> cluster and the record has to** (`ops::Mutation::server`).

The headless caller states the precondition that makes `None` correct there
(`src/main.rs:6920-6924`):

```
    let server = current_server(&kubeconfig);
    // `None`, because an `ops` line takes no `--context`: [`ops_words`] refuses every flag but
    // the namespace, so the context is the kubeconfig's own — which is the same argument
    // `current_server` was just asked with, and the two therefore name one entry.
    let session = match k8s::connect_with(kubeconfig, None, ready.namespace).await {
```

The console now connects with `opening.context` and still asks `current_server`
with `None`. Field the finding turns on: `ops::Mutation::server`
(`src/ops.rs:174`), written into `Record::attempt_line`'s
`… · context {} · server {} · …` (`src/ops.rs:1226-1233`).

## Measurement 2 — every `kubectl` line k8rs shows

```
$ grep -n '"\$ kubectl\|format!("\$ kubectl' src/main.rs
3033:    let cluster_wide = "$ kubectl get --raw '/api/v1/pods?limit=1'";
3052:                    "$ kubectl get --raw '/api/v1/namespaces/{}/pods?limit=1'",
3058:    log.push("$ kubectl get --raw /version".to_string());
3059:    log.push("$ kubectl api-resources --verbs=list".to_string());
3085:        log.push("$ kubectl get certificatesigningrequests".to_string());
3093:            log.push(format!("$ kubectl get {kind}{scope}"));
3100:        log.push("$ kubectl top nodes".to_string());
3102:    log.push(format!("$ kubectl get pods{scope} --watch"));
3103:    log.push("$ kubectl get nodes --watch".to_string());
3105:        log.push(format!("$ kubectl get {kind}{scope} --watch"));

$ grep -n '"kubectl scale\|"kubectl rollout\|"kubectl delete' src/ops.rs
1854:        "kubectl scale {object} --replicas={} -n {namespace}",
2365:    let kubectl = format!("kubectl rollout restart {object} -n {namespace}");
2731:        Some(namespace) => format!("kubectl delete {object} -n {namespace}"),
2732:        None => format!("kubectl delete {object}"),

$ grep -c -- '--context' <(grep -h 'kubectl ' src/main.rs src/ops.rs | grep '\$ kubectl\|"kubectl')
0
```

`{scope}` (`src/main.rs:3079-3082`) is `-n <ns>` or `-A`. No line in either file
carries `--context`.

The screen spec that governs it (`screens/context.md:447-457`):

```
$ kubectl --context staging get pods -A --watch
```
> Every command line after a switch carries `--context <name>` — honest, and it
> teaches the flag that makes `kubectl` safe to use across clusters.

## Measurement 3 — `USAGE` against the screen that specifies it

```
$ sed -n '347,359p' src/main.rs
const USAGE: &str = "usage: k8rs [--analysis] <file.json>...   |   \
     k8rs --once|--live [--analysis] [--context <name>] [--namespace <name>]   |   \
     ...
     k8rs [--read-only] ops <operation> <kind>/<name> [<value>] --namespace <name>   |   \
     ...
     Without --once, --live, --logs, --describe, --yaml or ops this build reads files only — it \
     cannot reach a cluster. --read-only refuses every operation, so a run that carries it can \
     ask (ops may-i) and never change anything.";
```

Six alternatives; none is the console. `screens/states.md:1967-2007`
(§ The command line's own synopsis) specifies seven, the console leading:

> **The ruling this box was missing: yes, the console gets a form on this line,
> and it leads.** … `k8rs [--read-only] [--context <name>] [--namespace <name>]`

and keeps, verbatim and unchanged, the trailing sentence
*"Without --once, --live, --logs, --describe, --yaml or ops this build reads
files only — it cannot reach a cluster."*

Both records are read by the same run: a piped console line prints `USAGE`
(`src/main.rs:163-167`), and a path beside a console flag prints the new refusal
sentence **with `USAGE` appended** (`src/main.rs:2503-2507`).

## Measurement 4 — the tie-break on a repeated flag

```
$ sed -n '1811,1816p' src/main.rs
fn context_arg(args: &[String]) -> Option<Option<&str>> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {

$ sed -n '4216,4232p' src/main.rs
fn value_of<'a>(args: &'a [String], flags: &[&str]) -> Option<Option<&'a str>> {
    ...
            if let Some(attached) = arg
                .strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
            {
                return Some(Some(attached));
            }
        }
        if flags.contains(&arg.as_str()) {
            return Some(rest.next().map(String::as_str));
        }
```

Both return on the **first** match. `value_of`'s own doc (`src/main.rs:4206-4209`):

> **First wins on a repeat**, which is [`live_context`]'s rule and not
> `kubectl`'s (`kubectl` is last-wins); … **Phase 12's real parsing is where the
> two should be made to agree.**

`context_arg`'s doc (`src/main.rs:1805-1810`) defers the same change with
*"it is not this box"*. Both flags are released as of this diff
(`src/main.rs:1935-1943`).

## Measurement 5 — what `--read-only` actually closes on the console

Three independent gates, read end to end:

| Gate | Where | Value under `--read-only` |
|---|---|---|
| the `Offer` the footer was drawn from | `ui::withheld` → `ui::offered` (`src/ui.rs:1659-1662`, `:1601-1604`) | `Writes::why()` is `Some` → `Offer::Move` (never `Act`) |
| the per-key liveness question | `views::App::may_mutate` (`src/views.rs:2826-2828`) ← `wanting` (`src/main.rs:9194`) | `offer.offers(op)` is `false` for `Op::Restart` and `Op::Delete` |
| the audit file the mutation frame needs | `console`'s `Halt::Mutate` arm (`src/main.rs:7724-7725`) | `audit` is `None` → `continue` |

Keys bound to a mutation, counted off the router:

```
$ sed -n '8636,8900p' src/main.rs | grep -n "wanting"
105:        return wanting(console, cards, DELETE);
190:        KeyCode::Char('r') => wanting(console, cards, RESTART),
```

Both route through `wanting`. No other call site.

Header word: `Writes::permission` (`src/ui.rs:868-874`) →
`theme::READ_ONLY` = `Signal::Mark("read-only")` (`src/theme.rs:279`), drawn in
both header shapes including the startup picker's (`src/ui.rs:1829-1842`).
Help's *Changing things* row: `withheld` → `Held::Off("k8rs was started with
--read-only — quit and start it again without it")` (`src/ui.rs:883-885`).

`X` opens the picker and nothing reconnects — `Chosen::Connect` is
`Did::Nothing` (`src/main.rs:8902-8908`) — so the once-built `Console::writes`
and `Console::context` cannot go stale in this build.

## Measurement 6 — where `-n` narrows

```
$ grep -n "async fn coverage" -A 14 src/k8s.rs
6859:async fn coverage(
...
6864-    if let Some(namespace) = asked {
6865-        return Coverage::Asked(if namespace_name(namespace) {
6866-            namespace.to_string()
...
6873-    if lists_pods(client, None, REPORT_FETCH).await {
6874-        return Coverage::Cluster;
```

`opening.namespace` → `connect_with`'s third argument (`src/main.rs:7554`) →
`coverage`'s `asked` → `Coverage::Asked`, and the cluster-wide probe is not sent.
`Coverage::namespace()` (`src/k8s.rs:6787-6794`) answers `Some` for `Asked`,
`Refused` and `Blind` alike, which is the single field both new scope surfaces
read: `zone` (`src/main.rs:8087-8097`) and the pane title
(`src/main.rs:8444-8447`), plus the `{scope}` in every command-log line
(`src/main.rs:3079-3082`).

Value the finding turns on: `FALLBACK_NAMESPACE` is `"default"`
(`src/k8s.rs:6728`) — the same word the author's reported measurement used for
its `-n` value.

## Measurement 7 — the header zone and where it erodes

```
$ grep -n "fn zone" -A 8 src/main.rs
8087:fn zone(context: Option<&str>, namespace: Option<&str>) -> String {
8088-    let mut zone = format!("ctx: {}", context.unwrap_or(views::UNNAMED));
8089-    if let Some(namespace) = namespace {
8090-        zone.push_str(" · ");
8091-        zone.push_str(&scoped(namespace));
8092-    }
8093-    zone
8094-}
```

Segment order in `ui::header` (`src/ui.rs:1832-1841`): context+scope, connection
state, permission, TLS warning, `changing…` — which is
`screens/widgets.md` § 1a's zone table order (line 59) exactly.

`ui::shortened` (`src/ui.rs:1891-1896`) cuts the **front**. Arithmetic over the
constants, not measured on a terminal: the tail `" · live · admin"` is 15
columns and `"ns: "` + `k8s::NAMESPACE_MAX` (63) is 67, so a zone whose scope
survives intact needs 82 columns. At the 80-column floor a namespace at its cap
loses its `ns:` label to the cut.

## Findings

Ranked, most severe first. Full text is in the report to the PM; the one-line
form and its location:

1. **blocking** — `src/main.rs:7553`. The console connects with
   `opening.context` and reads the audit line's `server:` with `None`, so a
   `--context` run records the wrong cluster URL beside the right context name.
2. **blocking** — `src/main.rs:3071-3107`, `src/ops.rs:1854`, `:2365`, `:2731`.
   No `kubectl` line k8rs shows carries `--context`, against
   `screens/context.md` § What the command log shows.
3. **blocking** — `src/main.rs:347-359`. `USAGE` has no console form and denies
   that a line without the six mode words can reach a cluster, against
   `screens/states.md` § The command line's own synopsis.
4. **should-fix** — `src/main.rs:1811`, `:4216`. `--context` and `--namespace`
   are first-wins on a repeat where `kubectl` is last-wins, now on released
   flags, with both docs deferring the change to the box that just shipped them.
5. **non-blocking** — `screens/states.md:1991-1995`. The synopsis section keeps
   the *"it cannot reach a cluster"* sentence it makes false two paragraphs up.
6. **non-blocking** — `src/main.rs:1990`. The refusal's subject reads
   *"--read-only opens the console, which reads a cluster"*: true of the run,
   false of the flag.
7. **non-blocking** — `docs/architecture.md:40`, `:49`. The CLI synopsis and the
   `--read-only` row predate Phase 7 and this box.
8. **nit** — `src/main.rs:1957-1969`. `console_flag` accepts `--read-only=`,
   a spelling `mistyped` refuses one gate earlier.
9. **nit** — `src/main.rs:1936`. `--analysis` and `--read-only` are accepted and
   silently dropped on lines where they mean nothing.

## What could not be settled without a run

- Whether `r restart` on a `--context <other>` console actually writes the other
  context's audit line with this kubeconfig's current server. The read above is
  conclusive on the code path; the confirming run is two contexts in one
  kubeconfig, `k8rs --context <b>`, one restart, then `tail -1
  ~/.local/state/k8rs/audit.log` compared against `kubectl config view -o
  jsonpath='{.clusters[*].cluster.server}'`. Not run this round.
- Whether the 80-column erosion of `ns:` in Measurement 7 is reachable on a real
  terminal. The confirming run is `k8rs -n <63-character namespace>` in an
  80-column window. Not run this round.
