#!/usr/bin/env bash
# The mutation gate's scratch volume, and the failures the gate cannot report
# about itself (NOTES § D133).
#
# cargo-mutants builds a full copy of the tree per mutant — measured **499-510 MB
# each** on 2026-08-21, eight of them left behind in /tmp — and it files *any*
# build failure as `unviable`. So a disk that fills turns untested mutants into a
# word that reads like a pass, one line apart in the summary from the honest one.
# That is NOTES § D26's green build wearing the tool's clothes, and
# NOTES § D104 handed the proof to this tool precisely because it has no
# incentive to lie: the honesty turns out to be conditional on a resource nobody
# was watching.
#
# What happened: `/tmp` here is a **12 GiB tmpfs and it was at 94%**, `/home` had
# 916 GB free the whole time, and the same mutant that came back `unviable` came
# back `caught` once TMPDIR moved. The gate was pointed at the smallest filesystem
# on the box.
#
# **A full disk is not the only way to lose a build**, which is why the same
# shape walked in a second time on 2026-08-21 through the toolchain flags — see
# `lint_denied_logs` below. So: **three checks, none subsuming another.** Before,
# refuse to start without headroom on the volume named below — cheap, and it
# fails in a second rather than after eleven minutes of sharded sweep. After,
# read the run's own logs twice, once for the filesystem's own words and once for
# a lint raised to an error; those are the only checks that can tell *nothing to
# test* from *could not test*, and the only ones that survive a disk filled by
# something else mid-sweep. Counting `unviable` cannot do it: 55 were legitimate
# at the last phase close, and a legitimate one names a type (`the trait bound …
# is not satisfied`), never a filesystem and never a lint.
#
# **And all three read a report, so a fourth thing has to be true before any of
# them means anything: that the report is this run's** (NOTES § D182). It was
# not — a run that tested nothing printed the log count and the unviable list of
# a run thirty-four minutes earlier, at exit 0. `lock_id` / `own_report` below
# decide that, and nothing under `$OUT` is read until they do.
#
# Every caller goes through here — `just mutants` (whole, or `--shard k/4`, D118)
# and `just mutants-diff` (the per-turn `--in-diff` gate). A flag typed at the
# gate reaches cargo-mutants unchanged; what it cannot do is inherit a tmpfs. The
# **one** exception is `--gate`, which is this file's own and is consumed here:
# whether a run that mutated nothing is a failure belongs to the caller, and only
# `just mutants-diff` says yes (NOTES § D182).
set -euo pipefail

cd "$(dirname "$0")/.."

# The scratch volume. `$HOME` rather than a path off a mount table, because it is
# the one directory guaranteed to exist and to be writable on every machine this
# runs on — this box, the LAN host, and CI — and on none of them is it a tmpfs.
# It is *named* rather than trusted: the headroom check below runs against
# whatever this resolves to, so a `$XDG_CACHE_HOME` that is itself small is
# refused like any other.
SCRATCH="${K8RS_MUTANTS_TMPDIR:-${XDG_CACHE_HOME:-$HOME/.cache}/k8rs-mutants}"
# Four times the largest scratch tree measured on 2026-08-21 (510 MB), which
# leaves room for a `--jobs` above 1 without the number having to know about it.
# Move it with a measurement, not a hunch: `du -sh "$SCRATCH"/cargo-mutants-*`
# during a run is where the 510 MB came from.
NEED_GIB="${K8RS_MUTANTS_NEED_GIB:-2}"
# cargo-mutants' default report directory. Nothing in it is read below until
# `own_report` says this run wrote it (NOTES § D182).
#
# Ceiling: a run given `--output` writes somewhere else, so this directory would
# not move and every scan below would sit out. No caller passes it today, and this
# line is where to look if one ever does — but note which way the failure now
# points: it used to be *the wrong tree read as this run's*, and it is now *this
# run reported as having started nothing*, which under `--gate` is a refusal
# somebody has to come and read. Loud beats quiet, which is the whole of D133.
OUT=mutants.out

# `df -Pk` and not `--output=avail`: the POSIX form is the one that works with
# `-P` on the coreutils here, and the arithmetic is one awk field either way.
#
# Split in two so the arithmetic can be proven against a **captured `df` line**
# rather than against whatever the machine happens to have free. The self-test
# used to read `.`, which made it assert something about the box instead of about
# the code — it would have gone red on a full disk, in the one file whose whole
# subject is that a full disk must not be mistaken for a result.
avail_field() { awk '{print int($4/1048576)}'; }          # KiB column 4 -> whole GiB
avail_gib() { df -Pk "$1" | tail -1 | avail_field; }

# The filesystem's own words, in the logs of a run that has already finished.
# Both spellings, because the message reaches the log through two different
# writers — rustc's own error and the `os error 28` a std::io error renders as.
enospc_logs() { # $1 = a mutants.out directory
  # No `[ -d ]` fast path: grep on a directory that is not there already returns
  # non-zero, and a branch whose removal changes nothing is a branch that cannot
  # fail. The missing-directory case is *said out loud* at the bottom of this
  # file instead, where it has its own sentence.
  grep -rlF -e "No space left on device" -e "os error 28" "$1/log" 2>/dev/null
}

# The **second** cause of the same lie, measured 2026-08-21: a mutated body
# leaves its parameters unused, `-D warnings` (the justfile exports it, CI sets
# it job-wide) makes that a build failure, and a build failure is filed
# `unviable`. Same tree, same 141 mutants — **77 unviable with the flag
# inherited, 18 without** — and one of the 59 it hid
# (`analysis.rs drain_row: replace > with >=`) was a real `MISSED`. The run below
# caps lints so the class cannot arise; this reads the logs anyway, because a
# count is what D133 says cannot tell you which kind you got.
#
# **Severity and identity, on two different lines.** Not a refinement: a first
# draft matched the note alone and refused a *green* run, because `--cap-lints`
# downgrades the diagnostic rather than deleting it and the identical note sits
# under a `warning:` header in the log of every mutant that is now `caught`.
# Identity is rustc's level note and never the lint's own text — `unused
# variable` is one of dozens and the next flag will name a different one, while
# every escalated lint arrives saying what raised it. `error[E….]` carries no
# such note, which is what keeps the honest unviable out.
#
# Ceiling: `forbid` is not matched, only `deny`. Nothing here forbids a lint.
lint_denied_logs() { # $1 = a mutants.out directory
  # `grep .` at the end so the exit status means what `enospc_logs`' grep means:
  # non-zero when nothing was found, which is what both callers branch on. awk
  # exits 0 whether or not it printed, and an `if hits=$(…)` on that would fire
  # the refusal on every clean run with an empty list of files under it.
  find "$1/log" -type f 2>/dev/null | while read -r f; do
    awk '
      /^error/   { err = ($0 ~ /^error:/); next }   # error[E….] is not a lint
      /^warning/ { err = 0; next }
      err && /implied by `-D |implied by `#\[deny\(|requested on the command line with `-D / \
                 { print FILENAME; exit }
    ' "$f"
  done | grep .
}

# The comparison, pulled out so it can be proven without a filesystem: an
# inverted `-ge` is the difference between a gate that refuses and one that never
# does, and it is one character.
enough_room() { [ "$1" -ge "$2" ]; } # $1 GiB available  $2 GiB required

# **Whose report is sitting in `$OUT`.** cargo-mutants writes no report at all
# when it finds no mutants — measured 2026-08-29 for both spellings it prints,
# `Diff changes no Rust source files` and `No mutants to filter` — so the previous
# run's logs, `unviable.txt` and `outcomes.json` are still there to be counted,
# named and printed as if they were this one's, which is NOTES § D182.
#
# `lock.json` is cargo-mutants' own per-run stamp: it carries a nanosecond
# `start_time`, it is written when the tool takes the lock and starts, and the
# tool rotates any previous report to `$OUT.old` itself at that moment. So a
# stamp that did not change across the run is the tool saying it started nothing
# here.
#
# **Read and never written, which is the whole reason it is this and not a
# rotation of `$OUT` by hand.** A hand rotation would move the *live* report of a
# sweep another process is running in this tree — carrying its `lock.json` with
# it — and cargo-mutants would then create a fresh one and run beside it, which
# is the lock defeated by the file that exists to make the gate trustworthy. That
# second run is not hypothetical: it happened on 2026-08-22
# (`reports/2026-08-22-phase-3-reclose-family-review.md` § 8) and the lock is
# what saved it. Measured here 2026-08-29 with a real run holding it: the blocked
# invocation printed `Waiting for lock … os error 11` and then `interrupted`, and
# `lock.json` came out with the same content, the same mtime and the same inode.
# So the held-lock case reads as *not this run's* for free, and nothing about the
# run that owns it is touched.
lock_id() { # $1 = the report directory; empty when there is no report at all
  cat "$1/lock.json" 2>/dev/null || true
}

# The decision, pulled out for the reason `enough_room` is — it is one character
# from never firing, and the character is the difference between reading this
# run's logs and reading somebody else's.
own_report() { # $1 = the lock stamp before the run  $2 = the stamp after it
  [ -n "$2" ] && [ "$1" != "$2" ]
}

# What *this run's own* report claims, or `none` when there is no result to read.
# Two different sentences, and D182 is what a script that cannot tell them apart
# prints: a previous run's twenty, under this run's name.
#
# **`outcomes.json` and not the directory**, because a run can leave one without
# the other — measured 2026-08-29: `--file src/theme.rs`, a file Phase 5 has not
# written yet, printed `Found 0 mutants to test`, created `mutants.out/` with an
# empty `log/` in it, and wrote no `outcomes.json` at all. A first draft of the
# message here said the directory did not exist and the directory was sitting
# right there, which is this box's own defect inside the fix for it.
#
# This answers a different question from `own_report` above and neither may
# answer the other's: that one says whether the directory is this run's, this one
# says whether there is a result in it.
mutant_count() { # $1 = the report directory
  [ -s "$1/outcomes.json" ] || { echo none; return 0; }
  # `|| echo none` and jq's own error left on stderr: a run killed mid-write
  # leaves a JSON that ends in the middle, and this is a gate, so an unreadable
  # result is refused rather than exited on. Without it the `count=$(…)`
  # assignment below dies under `set -e` carrying jq's status, and the gate never
  # gets to say anything at all.
  jq -r '.total_mutants // 0' "$1/outcomes.json" || echo none
}

# The gate's decision, pulled out for the reason `enough_room` is: it is one
# character from never firing. **Fail-closed** — `none`, an empty string and
# anything jq did not hand back as a number are all *this run gated nothing*,
# never *carry on*. Only `just mutants-diff` asks it (NOTES § D182).
gated_ok() { # $1 = a mutant_count reading
  case "$1" in ''|*[!0-9]*|0) return 1 ;; *) return 0 ;; esac
}

self_test() {
  local d fail=0
  d=$(mktemp -d); trap 'rm -rf "$d"' RETURN
  mkdir -p "$d/honest/log" "$d/full/log" "$d/empty/log"
  # An unviable that is telling the truth: a type error, which is a mutation
  # result. Cut from mutants.out/log on 2026-08-21.
  printf '%s\n' 'error[E0277]: the trait bound `rules::Condition: Default` is not satisfied' \
                'error: could not compile `k8rs` (bin "k8rs") due to 1 previous error' \
                '*** result: Failure(101)' > "$d/honest/log/src__rules.rs_line_556_col_9.log"
  # The same classification with a filesystem underneath it — **the two spellings
  # on two logs, not one**. The line the tool actually prints carries both at once
  # (`No space left on device (os error 28)`), so a single fixture is green with
  # either half of the pattern deleted: it proves the pattern matches something,
  # never that both spellings are covered (D29).
  printf '%s\n' 'error: failed to write bytecode' \
                'Caused by: No space left on device' \
                '*** result: Failure(101)' > "$d/full/log/src__rules.rs_line_1298_col_9.log"
  mkdir -p "$d/full28/log"
  printf '%s\n' 'error: failed to write bytecode' \
                'Caused by: os error 28' \
                '*** result: Failure(101)' > "$d/full28/log/src__rules.rs_line_1300_col_9.log"

  enospc_logs "$d/full" >/dev/null || { echo "FAIL  self-test: a log spelling the message out was not caught — the whole point of this script"; fail=1; }
  enospc_logs "$d/full28" >/dev/null || { echo "FAIL  self-test: a log carrying only the 'os error 28' spelling was not caught"; fail=1; }
  enospc_logs "$d/honest" >/dev/null && { echo "FAIL  self-test: an honest unviable (a type error) was called a disk failure"; fail=1; }
  enospc_logs "$d/empty" >/dev/null && { echo "FAIL  self-test: an empty log directory reported a hit"; fail=1; }
  enospc_logs "$d/missing" >/dev/null && { echo "FAIL  self-test: a mutants.out with no log/ at all reported a hit"; fail=1; }

  # --- the lint class, the second cause of an untested mutant reading as a pass ---
  # Three ways a lint can arrive already denied, and **all three are captured, not
  # written**: the first from the run this was found on
  # (`cargo mutants -F 'replace selects'` with `RUSTFLAGS=-D warnings` inherited,
  # `2 unviable`, then `2 caught` once `--cap-lints=true` was passed), the other
  # two off `rustc --edition 2021` on a four-line file, because nothing in this
  # repo denies a lint those two ways and there was therefore nothing here to
  # cut. Guessing the wording would have been guessing at the pattern that has to
  # match it.
  mkdir -p "$d/lint/log" "$d/lintattr/log" "$d/lintcli/log" "$d/typed/log"
  printf '%s\n' 'error: unused variable: `selector`' \
                '    --> src/analysis.rs:1078:12' \
                '     = note: `-D unused-variables` implied by `-D warnings`' \
                '     = help: to override `-D warnings` add `#[allow(unused_variables)]`' \
                'error: could not compile `k8rs` (bin "k8rs") due to 2 previous errors' \
                '*** result: Failure(101)' > "$d/lint/log/src__analysis.rs_line_1079_col_5.log"
  # The same denial written into the source rather than into the flags. Note the
  # **flush-left `note:` line between the header and the one that matches**: an
  # attribute denial renders one and a flag denial does not, so this is the
  # framing that proves the scan reads more than the two lines under a header
  # (D31). From `#![deny(warnings)]` over `fn f(x: i32) {}`.
  printf '%s\n' 'error: unused variable: `x`' \
                ' --> a.rs:2:6' \
                'note: the lint level is defined here' \
                ' --> a.rs:1:9' \
                '  = note: `#[deny(unused_variables)]` implied by `#[deny(warnings)]`' \
                'error: aborting due to 1 previous error' \
                '*** result: Failure(101)' > "$d/lintattr/log/src__rules.rs_line_88_col_5.log"
  # And one lint denied by its own name instead of through the group, the
  # spelling neither of the other two produces. From `rustc -D unused_variables`.
  printf '%s\n' 'error: unused variable: `x`' \
                ' --> b.rs:1:6' \
                '  = note: requested on the command line with `-D unused-variables`' \
                'error: aborting due to 1 previous error' \
                '*** result: Failure(101)' > "$d/lintcli/log/src__analysis.rs_line_1099_col_5.log"
  # A real compiler error is a mutation *result* and must survive the scan: refuse
  # these and the gate refuses every run it was built to make trustworthy. **Two
  # of them**, because an E-code is the easy one — the 18th honest unviable of the
  # 2026-08-21 sweep renders as a *bare* `error:`, the same header a denied lint
  # uses, and it is the shape a severity-reading scan is most likely to trip on.
  printf '%s\n' 'error[E0603]: module `inner` is private' \
                '*** result: Failure(101)' > "$d/typed/log/src__k8s.rs_line_12_col_1.log"
  printf '%s\n' 'error: `||` operators are not supported in let chain conditions' \
                'error: could not compile `k8rs` (bin "k8rs") due to 1 previous error' \
                'warning: build failed, waiting for other jobs to finish...' \
                'error: could not compile `k8rs` (bin "k8rs" test) due to 1 previous error' \
                '*** result: Failure(101)' > "$d/typed/log/src__analysis.rs_line_972_col_9.log"

  # The one this guard's own first draft got wrong: `--cap-lints=warn` leaves the
  # note in place under a `warning:` header, so every *passing* run on this repo
  # carries these lines. Cut from the log of the run that came back `2 caught`.
  mkdir -p "$d/capped/log"
  printf '%s\n' 'warning: unused variable: `selector`' \
                '    --> src/analysis.rs:1078:12' \
                '     = note: `-D unused-variables` implied by `-D warnings`' \
                '     = note: the `unused_variables` lint ignores `-D warnings`' \
                '*** result: Success' > "$d/capped/log/src__analysis.rs_line_1079_col_5_001.log"

  lint_denied_logs "$d/capped" >/dev/null && { echo "FAIL  self-test: a capped lint — a warning in the log of a mutant that was caught — was called a lint denial, which would refuse every green run"; fail=1; }
  lint_denied_logs "$d/lint" >/dev/null || { echo "FAIL  self-test: a log carrying rustc's '-D warnings' level note was not caught — that is the flag the justfile exports"; fail=1; }
  lint_denied_logs "$d/lintattr" >/dev/null || { echo "FAIL  self-test: a lint denied by a #[deny(warnings)] attribute in the source was not caught"; fail=1; }
  lint_denied_logs "$d/lintcli" >/dev/null || { echo "FAIL  self-test: a lint denied by name on the command line was not caught"; fail=1; }
  lint_denied_logs "$d/typed" >/dev/null && { echo "FAIL  self-test: a real compiler error — an E-code, or the bare 'error:' a parse failure prints — was called a lint denial, and those are mutation results"; fail=1; }
  lint_denied_logs "$d/honest" >/dev/null && { echo "FAIL  self-test: an honest unviable (a type error) was called a lint denial"; fail=1; }
  lint_denied_logs "$d/empty" >/dev/null && { echo "FAIL  self-test: an empty log directory reported a lint denial"; fail=1; }
  lint_denied_logs "$d/missing" >/dev/null && { echo "FAIL  self-test: a mutants.out with no log/ at all reported a lint denial"; fail=1; }
  # The two scans answer different questions and neither may answer the other's —
  # a pattern loose enough to catch both would report the wrong remedy for both.
  lint_denied_logs "$d/full" >/dev/null && { echo "FAIL  self-test: a disk failure was reported as a lint denial"; fail=1; }
  enospc_logs "$d/lint" >/dev/null && { echo "FAIL  self-test: a lint denial was reported as a disk failure"; fail=1; }

  # The other framing: the string inside a longer rustc line rather than alone on
  # one, which is how it actually arrives (D31 — a check is proven only for the
  # framing it was written for).
  printf 'error: linking with `cc` failed: No space left on device (os error 28) while writing target/debug/deps\n' \
    > "$d/empty/log/inline.log"
  enospc_logs "$d/empty" >/dev/null || { echo "FAIL  self-test: the string was only caught alone on a line, not inside one"; fail=1; }

  # --- the report this run did not write (NOTES § D182) ---
  # The measurement: a `--in-diff` run over a diff with no Rust in it left
  # `mutants.out/` byte-identical, mtime `12:17:31` before and after, and the
  # script printed *21 log(s)* and *7 unviable* from a run thirty-four minutes
  # earlier, exit 0. Nothing about that run was readable from the run itself, so
  # what is read here is cargo-mutants' own per-run stamp.
  #
  # **Both stamps are real** — `mutants.out/lock.json` off the 2026-08-29 run whose
  # numbers the stale read printed, and off the run measured against it an hour
  # later — with `hostname` and `username` taken out, the same two strings
  # `reports/` elides. Neither is read by anything here; `start_time` is what
  # differs between two runs and it is untouched.
  mkdir -p "$d/prev/log"
  printf '%s\n' '*** result: Success' > "$d/prev/log/src__k8s.rs_line_1766_col_9.log"
  printf '%s\n' '{' \
                '  "cargo_mutants_version": "27.1.0",' \
                '  "start_time": "2026-08-29T09:12:41.257896054Z"' \
                '}' > "$d/prev/lock.json"
  local stamp_a stamp_b
  stamp_a=$(lock_id "$d/prev")
  [ -n "$stamp_a" ] || { echo "FAIL  self-test: lock_id read nothing out of a real lock.json — every comparison below then reads two empty strings and no run is ever this run's"; fail=1; }
  [ -z "$(lock_id "$d/never-ran")" ] || { echo "FAIL  self-test: lock_id invented a stamp for a directory that does not exist — a first-ever run would then look like somebody else's report"; fail=1; }
  stamp_b=$(printf '%s\n' '{' '  "cargo_mutants_version": "27.1.0",' '  "start_time": "2026-08-29T10:21:55.802600136Z"' '}')

  # The D182 measurement itself: the stamp did not move, so cargo-mutants started
  # nothing here and the twenty, the twenty-one logs and the seven unviable in
  # that directory are somebody else's. **Same reading covers the held lock** —
  # measured 2026-08-29 against a real run that owned it: the blocked invocation
  # printed `Waiting for lock … os error 11` then `interrupted`, and lock.json
  # came out with the same content, mtime and inode.
  own_report "$stamp_a" "$stamp_a" && { echo "FAIL  self-test: an unchanged lock.json was called this run's report — that is the D182 measurement exactly, and it is also how another process's live sweep gets reported as this run's"; fail=1; }
  own_report "$stamp_a" "$stamp_b" || { echo "FAIL  self-test: a run that wrote its own stamp over a previous one was called somebody else's, so every real run would print no logs, no count and no unviable list at all"; fail=1; }
  own_report "" "$stamp_b" || { echo "FAIL  self-test: the first run ever in a tree — no report before it, its own stamp after — was called somebody else's"; fail=1; }
  own_report "" "" && { echo "FAIL  self-test: no report before and none after was called a report this run wrote"; fail=1; }
  own_report "$stamp_a" "" && { echo "FAIL  self-test: a report that is gone after the run was called this run's, and there is nothing there to read"; fail=1; }

  # `mutant_count` answers the other question, and neither may answer the other's:
  # `own_report` says whose the directory is, this says whether a result is in it.
  got=$(mutant_count "$d/prev")
  [ "$got" = none ] || { echo "FAIL  self-test: a report directory with a log and a lock and no outcomes.json read as '$got' rather than 'none' — cargo-mutants makes that directory before it knows it has anything to run, so reading the directory instead of the result passes an empty run"; fail=1; }
  # The whole top level of the same run's `mutants.out/outcomes.json`, its
  # twenty-element `outcomes` array dropped. The field this reads is untouched.
  printf '%s\n' '{' \
                '  "total_mutants": 20,' \
                '  "missed": 0,' \
                '  "caught": 13,' \
                '  "timeout": 0,' \
                '  "unviable": 7,' \
                '  "success": 0,' \
                '  "start_time": "2026-08-29T09:12:41.257993247Z",' \
                '  "end_time": "2026-08-29T09:32:19.394816574Z",' \
                '  "cargo_mutants_version": "27.1.0"' \
                '}' > "$d/prev/outcomes.json"
  got=$(mutant_count "$d/prev")
  [ "$got" = 20 ] || { echo "FAIL  self-test: a real outcomes.json reporting 20 mutants read as '$got' — the field moved, and a gate that cannot read a real count refuses every real run"; fail=1; }
  [ "$(mutant_count "$d/never-ran")" = none ] || { echo "FAIL  self-test: a report directory that is not there read as a count rather than 'none'"; fail=1; }

  # The two ways a result can be there and unreadable, which is what a run killed
  # mid-write leaves. The bytes are the real file's own head with the tail it never
  # got to write missing — an interrupted write is not a capture of anything and
  # cannot be one.
  mkdir -p "$d/empty-result" "$d/torn"
  : > "$d/empty-result/outcomes.json"
  got=$(mutant_count "$d/empty-result")
  [ "$got" = none ] || { echo "FAIL  self-test: a zero-byte outcomes.json read as '$got' rather than 'none' — that is what a run killed between creating the file and writing it leaves"; fail=1; }
  printf '%s\n' '{' '  "total_mutants": 20,' '  "missed": 0,' > "$d/torn/outcomes.json"
  # `|| got=…` so this arm *reports* instead of taking the whole self-test down
  # with jq's exit status, which is what the shape without the guard does.
  got=$(mutant_count "$d/torn" 2>/dev/null) || got="(mutant_count exited non-zero and printed nothing)"
  case "$got" in *none*) ;; *) echo "FAIL  self-test: a truncated outcomes.json read as '$got' — jq cannot parse it, and a gate that exits on that instead of refusing never prints why"; fail=1 ;; esac
  gated_ok "$got" && { echo "FAIL  self-test: a truncated outcomes.json passed the gate"; fail=1; }

  # The gate's decision, over every reading `mutant_count` can produce and over
  # the shapes a jq that misbehaved could hand it.
  gated_ok 20 || { echo "FAIL  self-test: a run of 20 mutants was refused by the gate, which would make every real turn red and teach people to skip it"; fail=1; }
  gated_ok 1  || { echo "FAIL  self-test: a single mutant was refused — one mutant is a gate"; fail=1; }
  gated_ok 0    && { echo "FAIL  self-test: a run of 0 mutants passed the gate — that is NOTES § D26's green build with cargo-mutants' signature on it"; fail=1; }
  gated_ok none && { echo "FAIL  self-test: a run that wrote no result at all passed the gate, which is the D182 measurement's own exit 0"; fail=1; }
  gated_ok ''   && { echo "FAIL  self-test: an unreadable count passed the gate — a gate that fails open is not a gate"; fail=1; }
  gated_ok 20x  && { echo "FAIL  self-test: a reading that is not a number passed the gate — jq printing something unexpected must refuse, not wave through"; fail=1; }

  # The headroom arm, proven against captured `df` output rather than by filling
  # a disk. Two real `df -Pk` lines off this box on 2026-08-21 — the roomy volume and the
  # tmpfs D133 is about, which really does read 0 GiB. Every column is a different
  # wrong answer and each one is a different broken gate: the *name* renders as 0
  # and would refuse every run, *size* and *used* both exceed the requirement and
  # would refuse none. Asserting the exact number pins all four at once, which
  # reading a live filesystem cannot do.
  local roomy=915 tight=0 got
  got=$(printf '%s\n' '/dev/nvme0n1p2   999678260 37354848 959901520       4% /home' | avail_field)
  [ "$got" = "$roomy" ] || { echo "FAIL  self-test: a captured df line with 959901520 KiB free read as $got GiB, not $roomy — the awk field moved"; fail=1; }
  got=$(printf '%s\n' 'tmpfs             12138708 11387892    750816      94% /tmp' | avail_field)
  [ "$got" = "$tight" ] || { echo "FAIL  self-test: the 94%-full tmpfs read as $got GiB, not $tight — a gate that cannot see a full disk is the defect this file is for"; fail=1; }
  # And the live reader still has to reach a real filesystem, which is the half
  # the captured lines cannot cover.
  local here; here=$(avail_gib "$d")
  case "$here" in ''|*[!0-9]*) echo "FAIL  self-test: avail_gib returned '$here' for a real directory, which is not a number of GiB"; fail=1 ;; esac
  # The decision itself, which is one character away from never firing.
  enough_room 0 2 && { echo "FAIL  self-test: 0 GiB free was called enough for a 2 GiB gate"; fail=1; }
  enough_room 1 2 && { echo "FAIL  self-test: 1 GiB free was called enough for a 2 GiB gate"; fail=1; }
  enough_room 2 2 || { echo "FAIL  self-test: exactly the required space was refused, so the gate is off by one"; fail=1; }
  enough_room 915 2 || { echo "FAIL  self-test: an empty disk was refused"; fail=1; }

  [ $fail -eq 0 ] || return 1
  echo "mutants: self-test passed — both spellings of the filesystem's message are refused, alone on a line and inside one; all three spellings of a denied lint are refused while the same note under a 'warning:' header is not, and neither scan answers the other's question; an honest unviable, a real compiler error with an E-code and one without, an empty log directory and a missing one are refused by neither; the headroom reader turns a captured df line into $roomy GiB and a 94%-full tmpfs into $tight; and the refusal fires below the requirement and not at it; a report whose lock stamp did not move across the run is refused as this run's while a stamp that moved and a first run in an empty tree are not, and neither is a report that vanished; a real outcomes.json reads 20 while a directory with no result in it, a missing one, an empty one and a truncated one all read 'none'; and the gate passes a count and refuses zero, no report, an empty reading and a non-number"
}

# `--gate` is read here and not passed on, because it is the *caller's* policy and
# not cargo-mutants': `just mutants-diff` is the per-turn gate CLAUDE.md § step 4
# names, and a green exit over zero mutants is D26's green build; `just mutants`
# whole or sharded, and a hand-typed call, only want the statement. That split is
# NOTES § D182's ruling, and this variable is the whole of it. **First argument** —
# everything after it belongs to cargo-mutants and is passed through untouched.
REQUIRE_MUTANTS=0
case "${1:-}" in
  --self-test) self_test; exit $? ;;
  --gate) REQUIRE_MUTANTS=1; shift ;;
esac

mkdir -p "$SCRATCH"
have=$(avail_gib "$SCRATCH")
case "$have" in ''|*[!0-9]*)
  echo "mutants: could not read the free space on $SCRATCH — df said '$have'. Refusing rather than" >&2
  echo "         running blind, which is the whole point of this file (NOTES § D133)." >&2
  exit 1 ;;
esac
if ! enough_room "$have" "$NEED_GIB"; then
  echo "mutants: refusing to start — $SCRATCH has ${have} GiB free and the gate needs ${NEED_GIB}." >&2
  echo "         cargo-mutants builds a whole copy of the tree per mutant (measured 499-510 MB" >&2
  echo "         each) and files a failed build as 'unviable', so a run that fills this volume" >&2
  echo "         reports untested mutants as a word that reads like a pass (NOTES § D133)." >&2
  echo "         Free space here, or point K8RS_MUTANTS_TMPDIR at a volume that has it." >&2
  exit 1
fi
echo "mutants: scratch $SCRATCH (${have} GiB free, ${NEED_GIB} required)"

export TMPDIR="$SCRATCH"
# `--cap-lints=true` is cargo-mutants' own flag for the class `lint_denied_logs`
# refuses. It sits **here** and not in the justfile because `bash
# scripts/mutants.sh` typed by hand is a caller too, and it *beats* an inherited
# `RUSTFLAGS=-D warnings` rather than merely avoiding one — proven 2026-08-21,
# the same two mutants going `2 unviable` -> `2 caught` with the flag still set.
# Nothing is lost: linting is `just check`'s job, over the unmutated tree, and a
# mutant's unused parameter is not a lint finding, it is the mutation. First on
# the line so a caller can still override it — clap takes the last — which is
# why the scan runs afterwards regardless.
# Read before the run, compared after it. Everything below reads `$OUT` only if
# the two differ (NOTES § D182).
before_lock=$(lock_id "$OUT")
rc=0
cargo mutants --cap-lints=true "$@" || rc=$?

# **Nothing in $OUT is read until it is this run's.** cargo-mutants leaves the
# previous report exactly where it was whenever it starts nothing here — a diff
# with no mutatable Rust in it, or another process in this tree holding the lock —
# and reading it then is NOTES § D182: 21 logs and 7 unviable, off a run
# thirty-four minutes earlier, printed under this run's name at exit 0.
count=none
if own_report "$before_lock" "$(lock_id "$OUT")"; then

  # After the run, and *before* $rc decides anything: cargo-mutants exits non-zero
  # for a MISSED mutant too, and a `set -e` that stopped at the run would skip the
  # one check this file exists for on exactly the runs worth checking.
  if hits=$(enospc_logs "$OUT"); then
    echo "mutants: THE DISK RAN OUT DURING THIS RUN — its result is not a result." >&2
    echo "         These mutant logs name the filesystem rather than a type, so cargo-mutants" >&2
    echo "         filed a build that never happened as 'unviable' (NOTES § D133):" >&2
    sed 's/^/           /' <<<"$hits" >&2
    echo "         Free space on $SCRATCH and run it again. Do not read the summary line." >&2
    exit 1
  fi
  # The same shape with a different cause, and its own remedy — which is why it is
  # its own message rather than a second pattern in the one above.
  if hits=$(lint_denied_logs "$OUT"); then
    echo "mutants: A DENIED LINT MADE MUTANTS UNVIABLE — this run's unviable count is not a result." >&2
    echo "         cargo-mutants replaces a body with a constant, so that function's parameters go" >&2
    echo "         unused; a toolchain that denies warnings turns that into a build failure, and" >&2
    echo "         cargo-mutants files any build failure as 'unviable' (NOTES § D133, second cause)." >&2
    echo "         These logs name a lint level rather than a type:" >&2
    sed 's/^/           /' <<<"$hits" >&2
    echo "         The run above passes --cap-lints=true for exactly this, so something overrode it —" >&2
    echo "         a --cap-lints=false typed at the gate, or a deny attribute in the source. Do not" >&2
    echo "         read the summary line." >&2
    exit 1
  fi

  # **The path named is the path read.** This line used to name `$SCRATCH`, the
  # build volume, where no log has ever been written, beside a count taken from
  # the repo root — the other half of how a stale report passed for this one
  # (NOTES § D182).
  if [ -d "$OUT/log" ]; then
    echo "mutants: no log names the filesystem or a denied lint — $(ls "$OUT/log" | wc -l) log(s) read on $PWD/$OUT/log"
  else
    echo "mutants: $OUT/log does not exist, so nothing was scanned — this is not a clean scan" >&2
  fi

  # **A run that tested nothing says so, and cannot say it in somebody else's
  # numbers.** The old form asked `outcomes.json` whether the count was zero, and
  # a stale `outcomes.json` saying 20 is exactly what kept it quiet on the run
  # that tested nothing. **Stated and not failed here**, because this file is the
  # shared driver and its callers differ; the caller that must refuse passes
  # `--gate` (NOTES § D182).
  count=$(mutant_count "$OUT")
  case "$count" in
    none) echo "mutants: this run wrote no result — there is no $PWD/$OUT/outcomes.json, so it tested nothing whatever the directory beside it holds. Correct for a run with no mutants under its filters, and not a gate passed (NOTES § D182)." ;;
       0) echo "mutants: 0 mutants — this run tested nothing. Correct for a diff with no mutatable Rust in it, and not a gate passed." ;;
  esac

  # `unviable` is read rather than skipped (D133): an honest one names a type, and
  # a count that moves is a count whose reasons somebody has to look at.
  if [ -s "$OUT/unviable.txt" ]; then
    echo "mutants: $(wc -l < "$OUT/unviable.txt") unviable — each of these is a claim that there was nothing to test:"
    sed 's/^/           /' "$OUT/unviable.txt"
  fi
else
  echo "mutants: cargo-mutants started no run in $PWD/$OUT — the lock.json in it is the one that was already there, so every log, count and unviable line in that directory belongs to an earlier run or to another process that holds the lock in this tree, and none of them is printed here. That is what a diff with no mutatable Rust in it looks like, and what a run that never got the lock looks like, and neither is a gate passed (NOTES § D182)."
fi

# The gate half of D182's split — the only refusal in this file that reads a count
# rather than a log, and the only one a caller has to ask for.
if [ "$REQUIRE_MUTANTS" = 1 ] && ! gated_ok "$count"; then
  echo "mutants: nothing to gate — this run produced no mutants, so it tested nothing, and" >&2
  echo "         exiting 0 on it would be NOTES § D26's green build with cargo-mutants'" >&2
  echo "         signature on it (NOTES § D182)." >&2
  echo "         cargo-mutants does not mutate #[cfg(test)] code, so this is what a turn that" >&2
  echo "         changed only tests looks like — the author's own verification is the part" >&2
  echo "         that went unverified. Nothing here says the code is wrong; it says this turn" >&2
  echo "         has no gate to pass, and CLAUDE.md § step 4 needs one." >&2
  echo "         From 'just mutants-diff': either the product change is missing from" >&2
  echo "         'git diff HEAD', or the turn changed no product Rust and the report has to" >&2
  echo "         say that in those words." >&2
  exit 1
fi
exit $rc
