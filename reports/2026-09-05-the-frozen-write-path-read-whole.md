# The write path read whole, before `ops.rs` freezes — Phase 7 family review

`k8s-admin`, 2026-09-05. Everything below was run on the dev machine against the
**built binary at `target/debug/k8rs`** (mtime 13:08, newer than `src/main.rs`
and `src/ops.rs` at 12:43, so it is the tree under review). No kind cluster was
brought up: every measurement here is reachable without one, and the cluster
runs this phase already has are
`reports/2026-09-05-every-operation-against-a-real-cluster.md` and its
neighbours. `kind-k8rs` was not touched.

Two sandboxes, both outside the reader's own state directory:

- `KUBECONFIG=/nonexistent/kubeconfig` for everything that is refused before a
  connection.
- a hand-written kubeconfig naming `http://127.0.0.1:1` (a loopback port with
  nothing listening) for the three runs that reach `perform`.
- `XDG_STATE_HOME` under the scratch directory for every run.

## 1 — The refusal surface, and how many lines each refusal costs

```
$ KUBECONFIG=/nonexistent/kubeconfig k8rs ops <line>   # stdin closed
```

| line | exit | stderr lines | state dir made |
|---|---|---|---|
| `ops frobnicate deploy/web -n payments` | 2 | **9** | no |
| `ops scale deploy/web -n payments` (no count) | 2 | **9** | no |
| `ops restart service/web -n payments` | 2 | **9** | no |
| `ops delete deploy/web` (no `-n`) | 2 | **9** | no |
| `ops delete node/n1 -n payments` | 2 | **9** | no |
| `ops scale deploy/web 3 -n payments --subresource=scale` | 2 | 9 | no |
| `ops scale deploy/web 3 -n payments -n other` | 2 | 9 | no |
| `ops scale deploy/web 3 -n payments --force` | 2 | 9 | no |
| `ops may-i patch deployments -n payments` | 2 | 9 | no |
| `ops restart rs/x -n payments` | 2 | **2** | no |
| `ops scale pod/web 3 -n payments` | 2 | **2** | no |
| `ops restart node/n1` | 2 | **2** | no |
| `ops scale node/n1 3` | 2 | **2** | no |
| `--read-only ops delete pod/web -n payments` | 2 | **1** | no |
| `ops delete pod/web -n payments --read-only` | 2 | **1** | no |

The two-line rows are the four `applies` refusals; the nine-line rows are the
synopsis. Full text of the two that sit next to each other:

```
$ k8rs ops restart service/web -n payments
k8rs: k8rs does not work on a kind called service — the ones an operation can be pointed at are deployment, statefulset, daemonset, replicaset, pod and node
usage: k8rs ops <operation> <kind>/<name> [<value>] -n <namespace>
  ops scale <kind>/<name> <copies> — say yes to confirm
  ops restart <kind>/<name> — say yes to confirm
  ops delete <kind>/<name> — type the object's own name to confirm
  ops may-i <verb> <resource>.<group>[/<name>] [--subresource <name>] — changes nothing
The namespace is required — an operation will not guess which object it is about. A node belongs to the whole cluster and takes none.
Every operation asks before it changes anything and reads the answer from what you type — one line, on standard input, every time. There is no flag that means yes.
`ops may-i` asks the cluster what this login is allowed to do and sends no change. Spell the API group — `deployments.apps`, or `pods.` for the core group. The `/` is the object's own name, as in `kubectl auth can-i`. Without -n it asks about the whole cluster.

$ k8rs ops restart rs/x -n payments
k8rs: k8rs cannot restart a replicaset: a replicaset is normally made by a deployment, and restarting that deployment is what replaces its copies. k8rs restarts a deployment, a statefulset and a daemonset
Run `k8rs ops` on its own to see everything it can do.
```

The `-n`-on-a-node row, whose sentence and whose seventh synopsis line say the
same thing:

```
$ k8rs ops delete node/n1 -n payments
k8rs: a node belongs to the whole cluster and is in no namespace, so `ops delete` will not take -n — leave it off
usage: k8rs ops <operation> <kind>/<name> [<value>] -n <namespace>
  ...
The namespace is required — an operation will not guess which object it is about. A node belongs to the whole cluster and takes none.
  ...
```

## 2 — `--read-only` in both positions, and on a question

```
$ k8rs --read-only ops delete pod/web -n payments          → exit 2, 1 line, no state dir
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation

$ k8rs ops delete pod/web -n payments --read-only          → exit 2, 1 line, no state dir
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation

$ k8rs --read-only --once ops delete pod/web -n payments   → exit 2
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation

$ k8rs --once ops delete pod/web -n payments               → exit 2
k8rs: `ops` has to be the first word on the line — write it as `k8rs ops <operation> <kind>/<name>`
(+ the 8-line synopsis)
```

The question, in all four flag positions, with an unreadable kubeconfig:

```
$ k8rs --read-only ops may-i list pods. -n payments   → exit 2
$ k8rs ops may-i --read-only list pods.               → exit 2
$ k8rs ops may-i list pods. --read-only               → exit 2
$ k8rs ops may-i list pods.                           → exit 2
k8rs: nothing was changed — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
```

Four identical sentences; the flag changes nothing about the question, and the
sentence begins `nothing was changed` on the one `ops` row whose own usage line
ends `— changes nothing`.

## 3 — Three runs that reach `perform`, against `http://127.0.0.1:1`

### 3a — `delete`, cancelled at end of input

```
$ k8rs ops delete pod/web-7d9f4 -n payments </dev/null
pod/web-7d9f4 in payments
This removes the pod. Whatever created it will normally replace it — k8rs has not checked whether anything did.
$ kubectl delete pod/web-7d9f4 -n payments
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: nobody confirmed it, so nothing was changed
exit 2
```

Audit log, both lines, verbatim:

```
2026-09-05T10:17:07.612175472Z attempt · pod/web-7d9f4 · context deadport · server http://127.0.0.1:1 · namespace payments · no uid was read · kubectl: kubectl delete pod/web-7d9f4 -n payments · call: DELETE /api/v1/namespaces/payments/pods/web-7d9f4 · resourceVersion not sent
result · attempt 2026-09-05T10:17:07.612175472Z · recorded 2026-09-05T10:17:07.612228185Z · pod/web-7d9f4 · dry-run: k8rs did not check this one with the cluster first · nobody confirmed it, so nothing was changed
```

Modes, on a fresh `XDG_STATE_HOME`:

```
$ stat -c "%a %n" $XDG_STATE_HOME/k8rs $XDG_STATE_HOME/k8rs/audit.log
700 .../state/k8rs
600 .../state/k8rs/audit.log
```

### 3b — `restart`, confirmed, socket refused

```
$ echo yes | k8rs ops restart deploy/web -n payments
deployment/web in payments
This asks Kubernetes to replace every copy of your app with a new one. How many stop at the same time is a setting on this deployment — it can be a few, or all of them at once. A paused deployment will not start until you resume it.
$ kubectl rollout restart deployment/web -n payments
k8rs: the change was never sent — k8rs could not reach the cluster
exit 2
```
```
... attempt · deployment/web · context deadport · server http://127.0.0.1:1 · namespace payments · no uid was read · kubectl: kubectl rollout restart deployment/web -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent
result · ... · deployment/web · dry-run: k8rs does not know whether the check reached the cluster · the change was never sent — k8rs could not reach the cluster
```

Note the two records do not print the dry-run verdict line to the operator at
all here: the run ends after `show` because `call(DRY_RUN)` errored, so no
`verdict`/prompt pair was printed.

### 3c — `delete`, confirmed, socket refused

```
$ echo web-7d9f4 | k8rs ops delete pod/web-7d9f4 -n payments
...
k8rs: k8rs does not know whether the change was made — k8rs could not reach the cluster
exit 2
```
```
result · ... · pod/web-7d9f4 · dry-run: k8rs did not check this one with the cluster first · k8rs does not know whether the change was made — k8rs could not reach the cluster
```

### 3d — `scale`, same kubeconfig

```
$ echo yes | k8rs ops scale deploy/web 3 -n payments
k8rs: k8rs could not read how many copies of deployment/web in payments are running right now — k8rs could not reach the cluster
exit 2
```

The audit log was byte-for-byte unchanged across this run: `scale` fails above
`perform` and writes no line.

## 4 — The typed name, against what the screen shows

```
$ echo "pod/web-7d9f4" | k8rs ops delete pod/web-7d9f4 -n payments
...
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: nobody confirmed it, so nothing was changed
exit 2
```

The title line prints `pod/web-7d9f4`; the token `Confirm::Type` holds is
`web-7d9f4`; the prompt names neither.

## 5 — Field-by-field comparison of the audit line against `docs/security.md`

`docs/security.md` (line 481) prints this as the attempt line:

```
2026-09-04T09:12:31.44Z attempt · deployment/web · context prod-eu · namespace payments · kubectl: kubectl scale deployment/web --replicas=3 -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion 88213
```

Fields the binary writes today, in order, off § 3a and § 3b above:

`<stamp> attempt · <object> · context <ctx> · server <url> · <namespace|cluster-wide> · <uid clause> · kubectl: … · call: <verb> <path> · resourceVersion <value|not sent>`

Present in the binary and absent from the documented line: `server <url>`,
`<uid clause>`. Present in the documented line: `resourceVersion 88213` on a
`scale`.

## 6 — Source facts read (not measured), with file and line

- `src/ops.rs:925` — `write_line(audit, &record.attempt_line(attempt))` runs
  before `show` (935) and before any `call` (938, 963).
- `src/ops.rs:1235-1243` — `which_uid`'s `sent` arm renders
  `uid <x> (the cluster checked this was the object)`.
- `src/main.rs:7411` — the headless driver passes `uid: None`, so `uid_sent` is
  `false` on every delete this binary can perform.
- `src/ops_tests.rs:4629` — the only assertion on that sentence is on the
  `200 OK` arm; the `409 Conflict` arm asserts `outcome`, `plainly()` and the
  server message, and not the attempt line.
- `src/ops.rs` — no occurrence of `read_only`/`read-only` other than three
  prose mentions at 2619, 3107, 3119; no parameter, type or token in the file
  is about the flag.
- `src/main.rs:7013,7145-7147` — `ops::scale`, `ops::restart`, `ops::delete`
  are `pub` and called from `main.rs`; the `--read-only` refusal is at
  `src/main.rs:6083`, in `ops_line`, and nowhere else.
- `PRIOR-ART.md:528` — `**immune.** Invariant 2's --read-only makes the write
  path *unreachable*, not merely unbound, so a new view cannot forget to check
  a flag — there is nothing to call.` `PRIOR-ART.md` is not in this phase's
  working tree diff.
- `docs/security.md:280` — `**k8rs-readonly** deliberately does not carry that
  last rule. No read-only code path calls may_i today`.
- `src/main.rs:6488-6494` — `applies` has a `_ => Ok(())` arm; `src/main.rs:7052`
  — `wired` has `_ => None`.

## 7 — Invariant 9 on the ops path, and what the echo says after the strip

Crafted argv, `KUBECONFIG=/nonexistent/kubeconfig`, output piped through
`cat -v`. No `ESC`, `BEL` or `CR` reached stderr in any run — the strip holds.
What the sentences then say:

```
$ k8rs ops restart $'dep\e[2Jloyment/web' -n payments
k8rs: k8rs does not work on a kind called dep[2Jloyment (with what cannot print removed) — the ones an operation can be pointed at are deployment, statefulset, daemonset, replicaset, pod and node

$ k8rs ops delete pod/$'web\e]0;pwn\a' -n payments
k8rs: web]0;pwn (with what cannot print removed) is not the name of an object — a name is letters, digits, dashes and dots, up to 253 characters

$ k8rs ops delete pod/web -n $'pay\rments'
k8rs: --namespace needs the name of a namespace, and payments (with what cannot print removed) is not one — a namespace is lowercase letters, digits and dashes, up to 63 characters
```

With a zero-width space (`U+200B`) rather than a control character, three
refusals name the exact word they have just said k8rs does not have. All three
measured, all three reachable from a command line:

```
$ k8rs ops $'sca​le' deploy/web 3 -n payments
k8rs: k8rs has no operation called scale (with what cannot print removed) — the ones it has are scale, restart and delete

$ k8rs ops scale    $'dep​loyment'/web 3 -n payments
$ k8rs ops restart  $'dep​loyment'/web 3 -n payments
$ k8rs ops delete   $'dep​loyment'/web 3 -n payments
k8rs: k8rs does not work on a kind called deployment (with what cannot print removed) — the ones an operation can be pointed at are deployment, statefulset, daemonset, replicaset, pod and node
   (identical for all three verbs)

$ k8rs ops scale deploy/web 3 $'--names​pace=payments'
k8rs: --namespace=payments (with what cannot print removed) is not a flag `k8rs ops` has — the ones it takes are -n or --namespace, which says which namespace this line is about, and --subresource, which `ops may-i` alone reads
```

The `ops.rs` sibling of the middle case, for the same input, is
`src/ops.rs:1461` (`a_kind`), which returns `that kind` rather than quoting:

```
k8rs cannot restart that kind — restarting replaces the copies an object is running, and k8rs does that for a deployment, a statefulset and a daemonset
```

`a_kind` is reached only when `ops::scalable` / `ops::restartable` / `ops::removal`
are handed a word the driver did not canonicalise, which no argv does today
(`src/main.rs:6531`, `known_kind`).
