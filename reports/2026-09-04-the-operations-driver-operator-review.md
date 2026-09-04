# The headless operations driver, measured

`k8s-admin`, 2026-09-04. Step 6 over `todo.md:3710` — the uncommitted
`// --- THE OPERATIONS DRIVER START ---` region of `src/main.rs` and the 12 tests
beside it in `src/main_tests.rs`, over HEAD `40e2739`.

No cluster was brought up and the PM's fixture cluster was not touched.

## How

The working tree was copied to a scratch directory and built with its own
`CARGO_TARGET_DIR`, so nothing in the repo tree was written and no target
directory was shared with the gate running beside it:

```
$ rsync -a --exclude target --exclude .git --exclude tmp /home/shyuuhei/GIT/k8rs/ <scratch>/tree/
$ cd <scratch>/tree && CARGO_TARGET_DIR=<cache>/target cargo build
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.44s
```

`kubectl v1.36.3` was run against a throwaway kubeconfig naming a cluster at
`https://127.0.0.1:1` — nothing listens there — with a dummy bearer credential and
a context whose namespace is `payments`. Every kubectl measurement below is
client-side (`--dry-run=client`), so no API server was contacted.

## M1 — the surface, as printed

```
$ k8rs ops
usage: k8rs ops <operation> <kind>/<name> [<value>] [-n <namespace>]
  ops scale <kind>/<name> <copies> — say yes to confirm
  ops restart <kind>/<name> — say yes to confirm
  ops delete <kind>/<name> — type the object's own name to confirm
Every operation asks before it changes anything and reads the answer from what you type — one line, on standard input, every time. There is no flag that means yes.
  [exit 2]
```

Every `ops` outcome measured in this report exits `2`, including the well-formed
line in M2.

## M2 — which operation/kind pairs reach the seam

`NOTES § Operations` gives `scale` deploy/sts/rs, `restart` deploy/sts/ds, and
`delete` any. Seven pairs run, one `not_wired` call site:

```
$ k8rs ops scale deploy/web 3 -n payments
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale ds/fluentd 3 -n logging
k8rs: k8rs read this as `scale` on daemonset/fluentd in logging, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale pod/web 3 -n payments
k8rs: k8rs read this as `scale` on pod/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale node/worker-1 3
k8rs: k8rs read this as `scale` on node/worker-1, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops restart rs/web-7d9f4 -n payments
k8rs: k8rs read this as `restart` on replicaset/web-7d9f4 in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops restart node/worker-1
k8rs: k8rs read this as `restart` on node/worker-1, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops delete node/worker-1
k8rs: k8rs read this as `delete` on node/worker-1, and there is nothing behind it yet — the operation itself is a later step
```

Five of the six operations-by-kind pairs above are outside the *Applies to*
column: `scale` on daemonset, pod and node; `restart` on replicaset and node.
`delete node` is inside it (`delete` applies to any).

## M3 — `--read-only`, by position

```
$ k8rs --read-only ops delete pod/web -n payments
k8rs: `ops` has to be the first word on the line — write it as `k8rs ops <operation> <kind>/<name>`
usage: k8rs ops <operation> <kind>/<name> [<value>] [-n <namespace>]
  ops scale <kind>/<name> <copies> — say yes to confirm
  ops restart <kind>/<name> — say yes to confirm
  ops delete <kind>/<name> — type the object's own name to confirm
Every operation asks before it changes anything and reads the answer from what you type — one line, on standard input, every time. There is no flag that means yes.
  [exit 2]

$ k8rs ops delete pod/web -n payments --read-only
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
  [exit 2]
```

The line the first refusal offers as the rewrite does not carry `--read-only`.

## M4 — the missing-value refusal names an object the reader did not

```
$ k8rs ops scale sts/api -n prod
k8rs: `ops scale` also needs the copies — write it as `k8rs ops scale deploy/web 3 -n payments`
usage: k8rs ops <operation> <kind>/<name> [<value>] [-n <namespace>]
  ...
  [exit 2]
```

The suggested line is complete and runnable, and names `deploy/web` in
`payments` rather than `sts/api` in `prod`.

## M5 — a repeated namespace flag: k8rs first-wins, kubectl last-wins

k8rs:

```
$ k8rs ops scale deploy/web 3 -n payments -n prod
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale deploy/web 3 --namespace payments -n prod
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step
```

kubectl v1.36.3, client-side, no server contacted:

```
$ kubectl create configmap probe --from-literal=a=b -n payments -n prod --dry-run=client -o yaml | grep -E 'name:|namespace'
  name: probe
  namespace: prod

$ kubectl create configmap probe --from-literal=a=b --namespace payments -n prod --dry-run=client -o yaml | grep -E 'name:|namespace'
  name: probe
  namespace: prod
```

## M6 — how long a refused word is echoed at

`BIG` is 9000 `a` characters (`python3 -c "print('a'*9000)"`). Bytes on the first
line of stderr:

| line | first line, bytes |
|---|---|
| `k8rs ops scale $BIG/web 3 -n payments` (kind) | 425 |
| `k8rs ops scale deploy/$BIG 3 -n payments` (name) | 378 |
| `k8rs ops scale deploy/web $BIG -n payments` (count) | 342 |
| `k8rs ops scale deploy/web 3 -n $BIG` (namespace) | 225 |
| `k8rs ops $BIG deploy/web 3 -n payments` (verb) | 359 |
| `k8rs ops restart deploy/web $BIG -n payments` (extra word) | 365 |
| `k8rs ops scale deploy/web 3 --$BIG` (unknown flag) | **9130** |
| `k8rs ops scale deploy/web 3 -$BIG` (unknown flag) | **9129** |
| `k8rs --$BIG pod.json` (unknown flag, pre-existing flag line) | **9032** |

Excerpt of the unknown-flag one:

```
$ k8rs ops scale deploy/web 3 -$BIG
k8rs: -aaaaaaaa… [9129 bytes on one line]
```

## M7 — the echoed word is not the word that was judged

The argument below is `--namespace` with a U+200B ZERO WIDTH SPACE between
`names` and `pace`:

```
$ k8rs ops scale deploy/web 3 '--names<U+200B>pace' payments
k8rs: --namespace is not a flag `k8rs ops` has — the only one it takes is -n or --namespace, which says which namespace the object is in

$ k8rs --once '--names<U+200B>pace' payments
k8rs: --namespace is not a flag k8rs has
```

For comparison, the same class of value reaching `shown` on the same line:

```
$ k8rs ops scale deploy/we<U+202E>b 3 -n payments
k8rs: web (with what cannot print removed) is not the name of an object — a name is letters, digits, dashes and dots, up to 253 characters
```

## M8 — the refusal and the usage under it disagree

```
$ k8rs ops scale deploy/web 3
k8rs: `ops scale` changes something, so it will not guess which namespace the deployment is in — name it with `-n <namespace>`
usage: k8rs ops <operation> <kind>/<name> [<value>] [-n <namespace>]
  ops scale <kind>/<name> <copies> — say yes to confirm
  ops restart <kind>/<name> — say yes to confirm
  ops delete <kind>/<name> — type the object's own name to confirm
Every operation asks before it changes anything and reads the answer from what you type — one line, on standard input, every time. There is no flag that means yes.
  [exit 2]
```

## M9 — the headless dialog and prompt, as printed

From the box's own tests, run with output shown:

```
$ cargo test --bin k8rs -- --nocapture --test-threads=1 the_confirmation_prints the_headless_dialog
running 2 tests
test tests::the_confirmation_prints_the_verdict_and_says_what_to_type_before_it_reads_anything ... the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it: 
the cluster checked it first and accepted it
type the object's own name and press enter to go ahead — anything else stops it: 
ok
test tests::the_headless_dialog_prints_the_consequence_above_the_command_and_no_verdict ... deployment/web in payments
This starts 1 more copy of your app. Right now: 2 copies.  After: 3 copies.
$ kubectl scale deploy/web --replicas=3 -n payments
node/worker-1
This removes the node from the cluster.
$ kubectl delete node worker-1
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 930 filtered out; finished in 0.00s
```

The name-typing prompt does not name the object; the identity one line above it
is printed in `kind/name` form, and `ask` compares against the bare name.

## M10 — other lines run

```
$ k8rs ops scale deploy/web 0 -n payments
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale deploy/web -3 -n payments
k8rs: the number of copies cannot be less than none, and -3 is

$ k8rs ops scale deploy/web 3 -n payments extra
k8rs: `ops scale` does not know what to do with extra — it reads the object and the copies and nothing else

$ k8rs ops delete pod/web -n payments --once
k8rs: --once is not a flag `k8rs ops` has — the only one it takes is -n or --namespace, which says which namespace the object is in

$ k8rs ops -n payments scale deploy/web 3
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale -n payments deploy/web 3
k8rs: k8rs read this as `scale` on deployment/web in payments, and there is nothing behind it yet — the operation itself is a later step

$ k8rs ops scale deploy/web -n 3 payments
k8rs: the number of copies has to be a whole number, and payments is not one

$ k8rs ops scale deploy/web 3 -n 3
k8rs: k8rs read this as `scale` on deployment/web in 3, and there is nothing behind it yet — the operation itself is a later step
```

`k8rs`'s own synopsis does not offer `ops`, and no file under `docs/`, `screens/`
or `tests/binary.rs` names it:

```
$ grep -rn "k8rs ops" docs/ screens/ tests/
(no matches)
```

## Environment note

The box's `/tmp` tmpfs reached 0 bytes free during this run while a gate was
running beside it; `cargo clean` on this review's own target directory returned
1.6 GiB and the rest of the run was built under `$HOME`.

```
$ CARGO_TARGET_DIR=<scratch>/target cargo clean
     Removed 1808 files, 1.6GiB total
$ df -h /tmp
tmpfs            12G   11G  1,5G  88% /tmp
```
