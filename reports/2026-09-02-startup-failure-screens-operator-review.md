# The startup-failure screens, measured — `k8s-admin`, 2026-09-02

Evidence for the operator review of the uncommitted `screens/` change to
`states.md`, `once.md` and `alerts.md`. Binary `./target/debug/k8rs` at
`40d8849`. No cluster write was made; the two failure kubeconfigs are scratch
files outside the repo. Node names, addresses and file contents are named
rather than pasted
([reports/README § sanitization](README.md#the-sanitization-rule--read-it-before-pasting-cluster-output)).

## 1 — The three kubeconfig failures reach one identical line

```
$ env KUBECONFIG=/nonexistent/nope.yaml ./target/debug/k8rs --once
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
EXIT=2
```

```
$ env KUBECONFIG=<scratch file holding malformed YAML> ./target/debug/k8rs --once
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
EXIT=2
```

```
$ env -u KUBECONFIG HOME=<empty scratch dir> ./target/debug/k8rs --once
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
EXIT=2
```

Width, measured on the first of the three:

```
$ env KUBECONFIG=/nonexistent/nope.yaml ./target/debug/k8rs --once 2>&1 | awk '{print length": "$0}'
114: k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
```

The variants behind the one line, read off `src/k8s.rs:1068-1087`
(`kubeconfig_fault`): `FindPath`, `ReadConfig(io::Error, PathBuf)`, `Parse(_)`,
`KindMismatch`, `ApiVersionMismatch` all map to `Fault::Kubeconfig`.
`ReadConfig` carries a `PathBuf` and an `io::Error`; `Parse` carries the YAML
error. `k8s.rs:756` states the type rule the mapping implements — a `Fault`
"carries no string whatever".

## 2 — A dead address on the watch path never draws `pods_unread`

Scratch kubeconfig: one cluster whose `server:` is `https://127.0.0.1:1`,
`insecure-skip-tls-verify: true`, one context, a placeholder user. No real
credential in it.

```
$ env KUBECONFIG=<scratch dead-port kubeconfig> timeout -s KILL 20 ./target/debug/k8rs --live 2>&1 | head -20
k8rs: watching — could not read the server version (nothing usable came back when k8rs tried to `get /version`) · could not list what this cluster serves, so k8rs cannot show you what is in it or tell which add-ons it has (nothing usable came back when k8rs tried to `get /apis`)
▲ k8rs is not getting pods from this cluster: nothing usable came back when k8rs tried to `list` and `watch` pods. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting nodes from this cluster: nothing usable came back when k8rs tried to `list` and `watch` nodes. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting Deployments from this cluster: nothing usable came back when k8rs tried to `list` and `watch` deployments. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting StatefulSets from this cluster: nothing usable came back when k8rs tried to `list` and `watch` statefulsets. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting DaemonSets from this cluster: nothing usable came back when k8rs tried to `list` and `watch` daemonsets. It keeps asking, and until that works nothing here about them can be trusted
```

Repeated to the kill at 20s. No `What k8rs asked for:` line, no
`What happened:` line, no next action.

Where the next action lives instead — `grep -n "server address" src/main.rs`:

```
3135:            "Check the server address this kubeconfig names, and that this machine can reach it"
3206:            "Nothing came back from it at all: check the server address this kubeconfig names, \
```

`3135` is inside `pods_unread`, `3206` inside `too_slow`. Both reachable only
through the `if stopping` guard at `src/main.rs:2939` and the `Some(budget)`
arm at `:2998`; `pods_unread`'s own doc at `:3038` reads "The one watch whose
failure ends a `--once` run instead of joining its report".

## 3 — The report's indent, with and without `--analysis`

Against the live four-node `k8rs` kind cluster, read-only.

```
$ ./target/debug/k8rs --once 2>/dev/null | grep -oE '^ *' | awk '{print length}' | sort -n | uniq -c
     35 0
     48 2
```

```
$ ./target/debug/k8rs --once --analysis 2>/dev/null | grep -oE '^ *' | awk '{print length}' | sort -n | uniq -c
     51 0
    107 2
      7 4
     83 6
```

The 4- and 6-column lines are the Capacity pane's per-node rows and their
`using …` continuation. Emitters: `src/main.rs:1029` builds the card action as
`format!("  → {}", …)`; `src/main.rs:1276` builds the pane action as
`format!("      → {}", …)`.

Longest line in the same run, D201's constant re-measured:

```
$ ./target/debug/k8rs --once 2>/dev/null | awk '{print length}' | sort -rn | head -1
423
```

(The 423-character line is an image-pull evidence string carrying a registry
host and a resolver address, so it is not reproduced here.)

## 4 — Surviving "re-wrapped" claims outside `screens/`

```
$ grep -rn "re-wrap\|rewrapped\|re-wrapped" src/ NOTES.md screens/
src/main.rs:789:/// `screens/once.md` § When your clock and the cluster's disagree verbatim, re-wrapped.
NOTES.md:15317:identical strings re-wrapped, and a word that does not survive being piped to a
```

`screens/` returns nothing.

## 5 — The TUI-side wrap numbers the change preserves

```
$ grep -rn "at 34\|53 at 80" screens/alerts.md
774:banner at the content pane's width less its two-column pads (**53 at 80×24**,
777:the empty screen's centred block at 34, `--once` printing it unfolded, the same
```
