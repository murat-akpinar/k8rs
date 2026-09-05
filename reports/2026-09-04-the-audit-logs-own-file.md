# The audit log's own file — what was measured (2026-09-04)

Operator review of `todo.md:3733`. Tree measured: HEAD `e180587` plus the box's
uncommitted working tree (`src/ops.rs` § THE AUDIT LOG, `src/main.rs`
§ THE OPERATIONS DRIVER). No cluster was brought up. Home paths below are
rewritten to `$S` (a scratch directory under `~/.cache`) and `$K` (the built
binary); both were removed after the run.

## 1. The record's real size

The record the end-to-end test writes, byte-counted:

```
$ wc -c < attempt.txt          # the attempt line, verbatim from the test
255
$ wc -c < result.txt           # the result line
170
                               # record = 425
```

The caps a record is bounded by, read off `src/k8s.rs:205,213,220`:
`IDENTIFIER` 512, `FREE_TEXT` 4096, and `text()` appends a 23-byte
`… (shortened by k8rs)` marker *after* the cut — so a cut `FREE_TEXT` field is
4119 bytes and a cut `IDENTIFIER` is 535.

| record | bytes | ×50/day, ×3652.5 days |
|---|---|---|
| typical, measured | 425 | 74 MiB |
| every call a D217-sized `422`, computed from the **uncut** 4859 | 5284 | 920 MiB |
| every call a D217-sized `422`, computed from what `said` **can write** (4096+23) | 4734 | 824 MiB |
| every field at its cap (attempt ≈ 12 037 + result ≈ 4914) | 16 951 | 2.88 GiB |

1 MiB at 425 bytes × 50/day = 49.3 days.

## 2. `flush()` on a `File` issues no fsync

`write_line`'s exact body (`write_all` then `flush`) against `write_all` then
`sync_data`, 5000 records of 425 bytes each, btrfs on nvme, same file mode
and flags `open_log` uses:

```
5000 records of 425 bytes:
  write_all + flush()        6.635ms  (1.3 us/record)
  write_all + sync_data()     8.770s  (1754.1 us/record)
  ratio 1322x
```

`strace` is not installed on this host; the timing is the measurement.

## 3. `open_log`'s behaviour on a path something is already standing at

A program replicating `open_log` exactly — `create_dir_all(parent)` then
`OpenOptions::new().create(true).append(true).mode(0o600).open(path)`:

```
1. created by k8rs           mode=0600  dir=0755
2. pre-existing 0666         mode=0666  open=OK, k8rs said nothing
   contents: planted by somebody else | k8rs appended here |
3. symlink at the log path   followed=true  target mode=0644 (unchanged)
4. k8rs/ is a symlink        create_dir_all=OK, log written through the symlink
5. File::flush()             returned Ok(())
```

Shell `umask` during the run: `022`.

## 4. The built binary, against a scratch `$XDG_STATE_HOME`

```
$ XDG_STATE_HOME=$S/a $K ops scale deploy/web 3 -n payments
k8rs: k8rs read this as `scale` on deployment/web in payments, 3 copies — and this build reads the line and does nothing else
exit=2
$ find $S/a -printf '%M %s %p\n'
drwxr-xr-x  8 $S/a
drwxr-xr-x 18 $S/a/k8rs
-rw-------  0 $S/a/k8rs/audit.log
```

```
$ XDG_STATE_HOME=$S/b $K ops bogus deploy/web -n payments
k8rs: k8rs has no operation called bogus — the ones it has are scale, restart and delete
left behind: 0 entries

$ XDG_STATE_HOME=$S/d $K --read-only ops delete pod/web -n payments
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
left behind: 0 entries

$ XDG_STATE_HOME=$S/c $K --once            # read-only against the running fixture cluster
exit=0  left behind: 0 entries
```

The three refusals, verbatim, all exit 2:

```
$ XDG_STATE_HOME=$S/e $K ops scale deploy/web 3 -n payments     # $S/e is mode 0500
k8rs: k8rs could not make a place for its audit log at $S/e/k8rs/audit.log: Permission denied (os error 13) — every change k8rs makes is written to that log before it is sent, so k8rs will not change anything until that is fixed, and reading your cluster still works

$ env -u HOME -u XDG_STATE_HOME $K ops scale deploy/web 3 -n payments
k8rs: k8rs has nowhere to keep its audit log: neither HOME nor XDG_STATE_HOME names a directory it can start from — every change k8rs makes is written to that log before it is sent, so k8rs will not change anything until that is fixed, and reading your cluster still works

$ XDG_STATE_HOME=relative env -u HOME $K ops scale deploy/web 3 -n payments
k8rs: k8rs has nowhere to keep its audit log: neither HOME nor XDG_STATE_HOME names a directory it can start from — every change k8rs makes is written to that log before it is sent, so k8rs will not change anything until that is fixed, and reading your cluster still works
```

`$XDG_STATE_HOME` set but relative, with `$HOME` writable:

```
$ HOME=$S/f XDG_STATE_HOME=oops $K ops scale deploy/web 3 -n payments
k8rs: k8rs read this as `scale` on deployment/web in payments, 3 copies — and this build reads the line and does nothing else
$ find $S/f -printf '%M %p\n'
drwxr-xr-x $S/f/.local/state/k8rs
-rw------- $S/f/.local/state/k8rs/audit.log
```

A log another local user left world-writable:

```
$ chmod 0666 $S/g/k8rs/audit.log && XDG_STATE_HOME=$S/g $K ops scale deploy/web 3 -n payments
k8rs: k8rs read this as `scale` on deployment/web in payments, 3 copies — and this build reads the line and does nothing else
$ find $S/g -type f -printf '%M %s %p\n'
-rw-rw-rw- 30 $S/g/k8rs/audit.log
```

A symlink standing where the log goes:

```
$ ln -sf $S/target/notes.txt $S/h/k8rs/audit.log
$ XDG_STATE_HOME=$S/h $K ops scale deploy/web 3 -n payments
exit=2
audit.log is: symbolic link to $S/target/notes.txt
target mode 644, unchanged, and k8rs printed nothing about it
```

A crafted `$XDG_STATE_HOME` reaching the terminal, piped through `cat -v`:

```
$ XDG_STATE_HOME="$S/i/blocked<ESC>[2Jgone" $K ops scale deploy/web 3 -n payments 2>&1 | cat -v
k8rs: k8rs could not make a place for its audit log at $S/i/blocked[2Jgone/k8rs/audit.log: Not a directory (os error 20) M-bM-^@M-^T every change k8rs makes is written to that log before it is sent, ...
```

The `ESC` is gone; the only multi-byte sequence left is k8rs's own em-dash.

## 5. What `Mutation` carries

Read off `src/ops.rs:115-143`: `context`, `namespace`, `object`, `consequence`,
`kubectl`, `verb`, `path`, `version`, `checkable`. No API server URL, no
kubeconfig path, no subject, no `uid`. `config.cluster_url` is already read at
`src/k8s.rs:8883`.

## 6. Timestamp precision

`jiff` 0.2.35 (`Cargo.lock:712`), `impl Display for Timestamp`
(`timestamp.rs:2338`) passes `f.precision()` — `None` from a plain `{now}` — to
`DateTimePrinter`, which prints minimal precision. The tests build stamps with
`Timestamp::from_second`, so every committed expectation is whole-second
(`2026-09-03T12:34:56Z`); `Timestamp::now()` carries sub-second digits.

## Housekeeping

A first attempt built into `CARGO_TARGET_DIR` under `/tmp` and filled the 12 GiB
tmpfs (D133's disk). It was cleaned with `cargo clean` (1.2 GiB) and rebuilt
under `~/.cache`; both target directories and every scratch state directory were
removed. `git status --short` shows no file this agent may not write.
