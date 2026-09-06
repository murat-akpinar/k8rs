#!/usr/bin/env bash
# What the person who runs `cargo install k8rs` gets, built and compiled here
# rather than in their hands (NOTES § D242 finding 2).
#
# `Cargo.toml`'s `exclude` drops `screens/`, `scripts/`, `docs/` and six more
# entries from the published crate (NOTES § D193), and `cargo publish` verifies
# what it uploads with a **build**. A build never compiles a `#[cfg(test)]`
# module. So an `include_str!("../screens/…")` in a test file reads a real path
# on every machine in this repo, ships green through `cargo test`, `cargo
# clippy`, `cargo package` and `cargo publish` alike, and fails for the
# downloader the first time they run the suite — measured on the real `.crate`:
# `cargo build` exit 0, `cargo test --no-run` exit 101 with four `couldn't read
# src/../screens/*.md`. Phase 9 shipped exactly that and it was caught by hand,
# because no gate could see it. By *"`just check` is the whole of CI, or it is a
# lie"* that absence is a gap, not a preference.
#
# **The step is pack, unpack, `cargo test --no-run`, and each of the three is
# load-bearing.** Packing alone is what `cargo publish` already does and it is
# the thing that missed this. Compiling the *repo* proves nothing about the
# tarball. Only compiling the **test targets of the unpacked bytes** asks the
# downloader's question, and `--no-run` asks it without needing a cluster.
#
# It is a `scripts/` guard and not a line in the `check` recipe for the reason
# NOTES § D111 gives: `scripts/guards.sh` is the one list, CI runs that file, and
# a step written into `just check` instead would need a second copy in
# `.github/workflows/ci.yml` — the drift this repo has already paid for three
# times. One line pair in `guards.sh` and CI has it by construction.
set -euo pipefail

cd "$(dirname "$0")/.."

# **Cargo's output below is this guard's instrument, not something a human reads
# for colour, so it is forced plain rather than parsed for escapes.**
# `.github/workflows/ci.yml` sets `CARGO_TERM_COLOR: always` job-wide, and cargo
# then colours its status words with no tty in sight — the line PR #16's run
# 34012412083 actually printed, captured with `cat -v`, was
#
#     ^[[1m^[[92m   Compiling^[[0m k8rs v0.0.0 (…/target/package-check/k8rs-0.0.0)
#
# The reset sits *between* `Compiling` and `k8rs` and the bold/green prefix sits
# *before* the leading spaces, so neither half of `compiled_the_crate`'s anchor
# can match. That run compiled the crate for 1m17s from a cold build directory
# and the guard called it a no-op. Locally there is no tty and no
# `CARGO_TERM_COLOR`, the line is plain, and the same pattern matches — so the
# gate had never once run against the output CI produces (NOTES § D29).
#
# One `export` rather than a prefix on each cargo line: a third cargo line added
# later inherits it instead of having to remember it. The other honest fix is
# stripping escapes before the grep, and it is the worse one — an ANSI parser
# inside a guard is a thing that can be wrong, and a flag cannot. The price is
# that a compile failure's trace below is uncoloured; it is `tee`'d whole either
# way. `self_test` runs cargo twice over a throwaway crate to prove this line
# still wins over an inherited `always`.
export CARGO_TERM_COLOR=never

# Where `cargo package` writes, where this unpacks, where it builds, and the
# stamp that says which run packed. All four under `target/`, so `cargo clean`
# sweeps them and `.gitignore`'s `/target/` already covers them — and all four
# are the guard's own rather than the caller's, which for `PKG_DIR` takes the
# `--target-dir` on the pack line below to stay true.
PKG_DIR=target/package
WORK=target/package-check
BUILD="$PWD/target/package-check-build"
STAMP="$PWD/target/package-check-stamp"

# **The canary, and it is the same shape as `just cross`'s musl line.** This
# guard can only ever see the D242 class while the package genuinely *drops*
# something the repo has — if `exclude` were emptied, every run would pass having
# proved nothing, which is NOTES § D133's subject with a tarball in it. `screens`
# is named because it is the directory D242's `include_str!` read. If it leaves
# `exclude` on purpose, move this line to whatever is still excluded rather than
# deleting it.
CANARY=screens

# The one `.crate` in a directory, or a refusal. Pulled out because it is the
# line that can silently vet nothing: a glob that matches no file expands to the
# literal pattern under bash's default, and a stale tarball from an older version
# beside a new one means the wrong bytes get tested. Neither may be guessed
# through, so both refuse and both have a self-test case below.
sole_crate() { # $1 = a directory that should hold exactly one .crate
  local found=() f
  for f in "$1"/*.crate; do [ -e "$f" ] && found+=("$f"); done
  case ${#found[@]} in
    1) printf '%s\n' "${found[0]}" ;;
    0) echo "package-check: no .crate file in $1 — 'cargo package' wrote nothing, or it stopped writing there, and there is nothing to test" >&2; return 1 ;;
    *) echo "package-check: $1 holds ${#found[@]} .crate files (${found[*]}), so which bytes this guard tested would depend on the glob order — remove the stale ones" >&2; return 1 ;;
  esac
}

# Whether the package still drops what the repo has. Its own function so the
# self-test can feed it both answers without a cargo run.
drops_the_canary() { # $1 = an unpacked crate root
  [ ! -e "$1/$CANARY" ]
}

# **Whether these are the bytes *this* invocation packed — the defence for the
# `--target-dir` on the pack line, because the flag alone is silently correct and
# a defence nobody can watch fail is what this whole file is about.** `cargo
# package` honours `CARGO_TARGET_DIR`: without that flag the pack lands in
# `$CARGO_TARGET_DIR/package/` while the read below picks up whatever an earlier
# run left in `target/package/` — one `.crate`, this version's name, so neither of
# `sole_crate`'s refusals can see it, and `-m` re-stamps those stale sources so
# `compiled_the_crate` watches cargo dutifully recompile the *wrong* tarball and
# print the line the canary wants. Measured 2026-09-06 against the pre-fix script,
# a `compile_error!` planted in `tests/` and packed (`Packaged 95 files`): red with
# the variable unset, exit 101 — and **green with it set**, exit 0, having unpacked
# a `target/package/` tarball twenty-nine seconds older than the pack it had just
# run. And CLAUDE.md § *The one hard rule of concurrency*
# tells an agent that wants a clean tree to copy it and give the copy its own
# `CARGO_TARGET_DIR`, so that is the normal case here and not an exotic one.
#
# Not-older rather than strictly-newer: a filesystem with one-second timestamps
# can stamp both inside one tick, and a false red on an honest run is how a gate
# gets waved through, while a leftover tarball is minutes or days old.
written_by_this_run() { # $1 = the .crate about to be tested, $2 = a file stamped just before the pack
  [ ! "$2" -nt "$1" ]
}

# **This guard's own D133 check, and it is here because the first draft of this
# file failed it.** Measured 2026-09-06: pack, unpack, `cargo test --no-run`
# against the repo's own `target/` printed
#
#     Finished `test` profile [unoptimized + debuginfo] target(s) in 0.17s
#
# and exit 0, having compiled **nothing**. `cargo package` stamps every entry in
# the tarball with a fixed `2006-07-24 04:21` for reproducibility, so the
# extracted sources are twenty years older than the repo's artifacts; cargo gives
# the unpacked crate the *same* unit hash as the repo's (`k8rs-c969b5b7178e79b1`,
# `binary-7d4ca61b28c518f0` — read off both), the mtime check passes, and the
# guard reports the published bytes green having never looked at them. The two
# defences below are structural — `-m` on the untar and a target directory of the
# guard's own — and this is the one that *says so*, because a defence nobody can
# see fail is the thing this whole file is about.
compiled_the_crate() { # $1 = cargo's own output
  grep -qE '^[[:space:]]*Compiling k8rs v' <<<"$1"
}

self_test() {
  local d fail=0 got
  d=$(mktemp -d); trap 'rm -rf "$d"' RETURN

  mkdir -p "$d/none" "$d/one" "$d/two"
  : > "$d/one/k8rs-0.0.0.crate"
  : > "$d/two/k8rs-0.0.0.crate"
  : > "$d/two/k8rs-0.1.0.crate"
  got=$(sole_crate "$d/one") || { echo "FAIL  self-test: the one .crate in a directory was refused, so this guard would never test anything"; fail=1; }
  [ "$got" = "$d/one/k8rs-0.0.0.crate" ] || { echo "FAIL  self-test: sole_crate printed '$got' rather than the file it found"; fail=1; }
  sole_crate "$d/none" >/dev/null 2>&1 && { echo "FAIL  self-test: an empty directory yielded a crate path — bash expands an unmatched glob to the pattern itself, and tarring that is the silent pass this guard exists to refuse"; fail=1; }
  sole_crate "$d/two" >/dev/null 2>&1 && { echo "FAIL  self-test: two .crate files were accepted, so the bytes tested would be whichever the glob sorted first and not necessarily this run's"; fail=1; }
  # A directory that is not there at all — a `target/` swept by `cargo clean`
  # between the pack and this read, which is the same nothing wearing a third
  # coat.
  sole_crate "$d/never-made" >/dev/null 2>&1 && { echo "FAIL  self-test: a directory that does not exist yielded a crate path"; fail=1; }

  # The canary, both answers. The failing one is the one that matters: a package
  # that excludes nothing passes every compile this guard can run, forever, and
  # says nothing at all about the class it was written for.
  mkdir -p "$d/dropped" "$d/kept/$CANARY"
  drops_the_canary "$d/dropped" || { echo "FAIL  self-test: an unpacked crate with no $CANARY/ in it was called a package that keeps it, which would refuse every honest run"; fail=1; }
  drops_the_canary "$d/kept" && { echo "FAIL  self-test: an unpacked crate that still carries $CANARY/ passed — the package then drops nothing this guard can see, and a green run here would prove nothing (NOTES § D133, § D242)"; fail=1; }
  # And the framing that is not a directory: `exclude` takes plain files too, so a
  # canary that were one has to be caught by the same test and not by a `-d`.
  mkdir -p "$d/keptfile"; : > "$d/keptfile/$CANARY"
  drops_the_canary "$d/keptfile" && { echo "FAIL  self-test: the canary present as a *file* rather than a directory passed — the test reads the wrong thing about it"; fail=1; }

  # The stamp, three framings of the mtime pair. The middle one is the defect
  # this function exists for; the third is the one that would make it a nuisance.
  mkdir -p "$d/stamped"
  touch -d '2020-01-01 00:00:00' "$d/stamped/leftover.crate"
  touch -d '2020-01-01 00:00:01' "$d/stamped/stamp"
  touch -d '2020-01-01 00:00:02' "$d/stamped/fresh.crate"
  touch -r "$d/stamped/stamp" "$d/stamped/same-tick.crate"
  written_by_this_run "$d/stamped/fresh.crate" "$d/stamped/stamp" || { echo "FAIL  self-test: a .crate written after the stamp was called a leftover, which would refuse every honest run"; fail=1; }
  written_by_this_run "$d/stamped/leftover.crate" "$d/stamped/stamp" && { echo "FAIL  self-test: a .crate older than this run's stamp was accepted — that is the tarball an earlier run left in target/package/ while CARGO_TARGET_DIR sent this run's pack somewhere else, and it carries the right name and the right version so nothing else here can tell"; fail=1; }
  written_by_this_run "$d/stamped/same-tick.crate" "$d/stamped/stamp" || { echo "FAIL  self-test: a .crate stamped in the same tick as the marker was refused — on a one-second-granularity filesystem that is every honest run"; fail=1; }

  # --- the run that compiled nothing, and every framing of cargo's own words ---
  # **Captured, not written.** The first is this file's own false pass of
  # 2026-09-06, verbatim; the second is the same invocation once the two defences
  # were in. Guessing either would be guessing at the pattern that has to match it.
  local silent compiling
  silent=$(printf '%s\n' \
    '    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.17s' \
    '  Executable unittests src/main.rs (target/debug/deps/k8rs-c969b5b7178e79b1)' \
    '  Executable tests/binary.rs (target/debug/deps/binary-7d4ca61b28c518f0)')
  compiling=$(printf '%s\n' \
    '   Compiling k8rs v0.0.0 (/home/shyuuhei/GIT/k8rs/target/package-check/k8rs-0.0.0)' \
    '    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 04s' \
    '  Executable unittests src/main.rs (target/package-check-build/debug/deps/k8rs-4a4bd5b1d0a06f6a)')
  compiled_the_crate "$compiling" || { echo "FAIL  self-test: a run that really did compile the unpacked crate was called a no-op, which would refuse every honest run"; fail=1; }
  compiled_the_crate "$silent" && { echo "FAIL  self-test: the measured false pass — cargo finishing in 0.17s having compiled nothing because the tarball's fixed 2006 mtimes are older than the repo's artifacts — was called a compile (NOTES § D133)"; fail=1; }
  compiled_the_crate "" && { echo "FAIL  self-test: empty output was called a compile"; fail=1; }
  # A dependency compiling is not this crate compiling: on a cold build directory
  # every one of these scrolls past before the line that matters, and a pattern
  # that took any of them would pass a run that stopped before reaching k8rs.
  compiled_the_crate '   Compiling kube v4.2.0' && { echo "FAIL  self-test: a *dependency* compiling was read as the crate under test compiling"; fail=1; }
  compiled_the_crate '   Compiling k8s-openapi v0.28.0' && { echo "FAIL  self-test: 'k8s-openapi' satisfied a pattern meant for 'k8rs' — the version marker is what separates the package name from anything that starts with it"; fail=1; }

  # --- and the same words as cargo itself prints them, in this script's own
  # environment ---
  # **The only case here that reads the instrument instead of a string.** The
  # five above prove the pattern against output somebody transcribed; PR #16 was
  # red because the output CI produces is not the output anybody transcribed —
  # `CARGO_TERM_COLOR: always` is set job-wide, cargo colours its status words
  # with no tty in sight, and the escapes land inside the anchor (see the export
  # at the top of this file for the captured bytes). What has to be true is not
  # "the pattern reads the line I typed" but "the cargo this script runs prints a
  # line this pattern reads", and only cargo can say that.
  #
  # A throwaway crate, built twice. Named `k8rs 0.0.0` because that name is what
  # makes `compiled_the_crate` applicable to it; two target directories because a
  # second build into the first one prints `Finished` and no `Compiling` at all.
  # Cost, timed on this box rather than guessed: **0.20s** for the pair, against
  # a guard whose real run is 6s warm. It is also green under CI's whole
  # environment and not just its colour setting — `RUSTFLAGS=-D warnings`,
  # `CARGO_TERM_COLOR=always` and `CARGO_NET_OFFLINE=true` together, checked
  # because a scratch crate that needed the network or tripped a denied lint
  # would be a guard that only fails on the runner.
  # The setting itself, asserted before the pair below, because the pair alone
  # only goes red in a caller that asks for colour — which is CI and is not
  # `just check` on this box. A gate that defers its own regression to the
  # runner is the thing this repo calls a lie; this line is red locally the
  # moment the export at the top is dropped, and the pair below is what says
  # the setting still *does* something.
  [ "${CARGO_TERM_COLOR:-}" = never ] || { echo "FAIL  self-test: this script no longer forces CARGO_TERM_COLOR=never (it reads '${CARGO_TERM_COLOR:-<unset>}'), so under CI's job-wide 'always' cargo colours its status words and compiled_the_crate's anchor matches nothing — PR #16, run 34012412083"; fail=1; }
  local scratch out
  scratch="$d/scratch"; mkdir -p "$scratch/src"
  printf '[package]\nname = "k8rs"\nversion = "0.0.0"\nedition = "2021"\n' > "$scratch/Cargo.toml"
  echo 'fn main() {}' > "$scratch/src/main.rs"
  # The canary, and it is load-bearing: cargo still colours when it is told to.
  # A future cargo that refused colour on a pipe would leave the case below
  # passing having proved nothing (CLAUDE.md § A derived list asserts it found
  # something).
  out=$(CARGO_TERM_COLOR=always CARGO_TARGET_DIR="$d/coloured" cargo build --manifest-path "$scratch/Cargo.toml" 2>&1) || true
  case $out in
    *$'\033'*) ;;
    *) echo "FAIL  self-test: cargo printed no ANSI escape at all under CARGO_TERM_COLOR=always, so the case below cannot tell a forced-plain run from a coloured one and would pass either way"; fail=1 ;;
  esac
  # And the run this guard actually reads: nothing is set here, so it inherits
  # this script's environment — the export at the top, over whatever the caller
  # had. Run this file under `CARGO_TERM_COLOR=always` and it is CI exactly.
  out=$(CARGO_TARGET_DIR="$d/plain" cargo build --manifest-path "$scratch/Cargo.toml" 2>&1) || true
  case $out in
    *$'\033'*) echo "FAIL  self-test: cargo's output carries ANSI escapes in this script's own environment, so 'export CARGO_TERM_COLOR=never' at the top is gone or is being overridden — that is PR #16's red (run 34012412083): a 1m17s compile the guard called a no-op"; fail=1 ;;
  esac
  compiled_the_crate "$out" || { echo "FAIL  self-test: no line this guard reads came back from a cargo run in this script's own environment — $(grep -a Compiling <<<"$out" | cat -v || printf 'and cargo printed no Compiling line at all: %s' "$(cat -v <<<"$out" | tail -3)")"; fail=1; }

  [ $fail -eq 0 ] || return 1
  echo "package-check: self-test passed — exactly one .crate is accepted while none, two and a missing directory are refused; a .crate older than this run's stamp is refused while a newer one and a same-tick one are not; an unpacked crate that still carries $CANARY, as a directory or as a file, is refused while one that drops it is not; and cargo's own words for a run that compiled the crate are told from the measured 0.17s no-op, from empty output, and from a dependency compiling; this script's own environment still forces CARGO_TERM_COLOR=never, and cargo, run twice over a throwaway crate, still colours when told to and still prints a line this guard reads when run in this script's own environment"
}

case "${1:-}" in
  --self-test) self_test; exit $? ;;
  "") ;;
  *) echo "package-check: unknown argument '$1' — this guard takes --self-test or nothing" >&2; exit 1 ;;
esac

# **`--allow-dirty`, and it is a choice with a price.** `cargo package` refuses a
# tree with uncommitted changes; `just check` runs on a dirty tree all day, and a
# gate that is red by default is one everybody learns to wave through (the same
# cost decision `just cross` records). So this packs the **working tree**, not
# `HEAD` — which is what a per-change gate wants and is *not* what `cargo publish`
# will do. The gap it leaves is one shape: work that is in the tree and will never
# be committed. Phase close pushes and CI re-runs this on the committed tree, where
# `--allow-dirty` is a no-op, so that shape closes there.
#
# **`--no-verify` is not a weakening.** Verification is a `cargo build` of the
# unpacked crate — a strict subset of the `cargo test --no-run` below, which
# compiles the same bin *and* the test targets the build never reaches. Keeping it
# would buy nothing and cost a second full dependency build in a directory nothing
# else warms.
#
# **`--target-dir target` is not tidiness either.** `cargo package` writes the
# tarball to `<target-dir>/package/`, and it honours `CARGO_TARGET_DIR`; the read
# below is a fixed `target/package`. Naming the directory on the command line
# pins the two together under any environment — measured 2026-09-06 with
# `CARGO_TARGET_DIR` pointed at a scratch tree: the scratch tree stayed empty and
# `target/package/k8rs-0.0.0.crate` was rewritten at the run's timestamp. The
# other honest fix is deriving `PKG_DIR` from `${CARGO_TARGET_DIR:-target}`, and
# it is the worse one here: `WORK` and `BUILD` are deliberately the guard's own —
# `BUILD` *must* be, or it hands back the repo's artifacts (below) — so that
# version makes one of the four follow the caller and three not. `scripts/e2e.sh`
# derives, and that is not an inconsistency: it *reads* a binary somebody else's
# cargo built under the caller's environment, where following is the only correct
# answer. This guard writes all four itself.
mkdir -p "$PKG_DIR"   # also makes target/, which the stamp beside it needs
touch "$STAMP"
cargo package --target-dir target --locked --no-verify --allow-dirty

crate=$(sole_crate "$PKG_DIR")
written_by_this_run "$crate" "$STAMP" || {
  echo "package-check: $crate is older than the stamp this run wrote immediately before packing, so it is an earlier run's tarball and not the bytes just packed." >&2
  echo "        'cargo package' honours CARGO_TARGET_DIR, so the usual cause is the '--target-dir target' above" >&2
  echo "        having been dropped while the caller has that variable set — the pack then lands in" >&2
  echo "        \$CARGO_TARGET_DIR/package/ and this read finds the stale tarball, with the right name and the" >&2
  echo "        right version, and vets bytes nobody is about to publish (NOTES § D242)." >&2
  exit 1
}
# Cleared rather than extracted over: a file the previous pack carried and this
# one drops would otherwise still be sitting there, and this guard would be
# testing a tarball nobody has.
rm -rf "$WORK"
mkdir -p "$WORK"
# **`-m` is not a tidiness flag, it is one of the two defences.** Without it the
# extracted sources carry the tarball's fixed `2006-07-24 04:21` on every run
# forever, so cargo's mtime-based freshness check can never see a change.
# Measured 2026-09-06 by taking the flag back out: a `compile_error!` appended to
# an already-packaged `tests/binary.rs` — a content change to an existing file,
# nothing added and nothing removed — came back `Finished 'test' profile … in
# 0.09s` with no `Compiling k8rs` line at all, and the guard would have passed it
# had the check below not refused. `-m` stamps the sources now, so each run's are
# newer than the last build and the local package is always recompiled; with it
# in, the same plant is red in 4s. Dependencies are untouched and stay cached.
tar -xzmf "$crate" -C "$WORK"
src="$WORK/$(basename "$crate" .crate)"
[ -f "$src/Cargo.toml" ] || {
  echo "package-check: $crate did not unpack to $src — the tarball's top-level directory is no longer the crate's own name, and everything below reads a path that is not there" >&2
  exit 1
}

drops_the_canary "$src" || {
  echo "package-check: the unpacked crate still carries $CANARY/, so Cargo.toml's 'exclude' no longer drops it." >&2
  echo "        This guard can then never see the class it exists for — an include_str! reading a" >&2
  echo "        path the package leaves out compiles here and fails in the downloader's hands" >&2
  echo "        (NOTES § D242). If $CANARY left 'exclude' on purpose, move this canary to something" >&2
  echo "        the package still drops; do not delete it." >&2
  exit 1
}

# **A build directory of the guard's own, and that is the second defence.**
# Sharing the repo's `target/` looks free — the dependencies are the identical
# registry crates — and it is the thing that produced the 0.17s false pass above:
# cargo gives the unpacked crate the same unit hash as the repo's, so it hands
# back the repo's artifacts and reports them as this run's. It would also work in
# the other direction, the packaged crate's build overwriting the repo's own
# artifacts under a hash the repo will later call fresh.
#
# **The price, measured on this box 2026-09-06:** 1m17s the first time this
# directory is used, because every dependency is built into it; 6s on every run
# after that, of which ~4.7s is recompiling k8rs itself. `just check` whole is
# 71s steady-state with this guard in it. **What is not measured is CI**, where
# `Swatinem/rust-cache` may or may not carry a nested build directory across
# pushes — if it does not, the `guards` step pays the 1m17s (a runner's own
# figure, not this one) on every push, and the number to read is that step's
# duration on the first green run after this lands.
log=$(mktemp); trap 'rm -f "$log"' EXIT
set +e
CARGO_TARGET_DIR="$BUILD" cargo test --locked --no-run --manifest-path "$src/Cargo.toml" 2>&1 | tee "$log"
rc=${PIPESTATUS[0]}
set -e
[ "$rc" -eq 0 ] || {
  echo "package-check: the published bytes do not compile their own tests — the trace above is what somebody who ran 'cargo install k8rs' and then 'cargo test' would see, and it is the only place it is visible (NOTES § D242)." >&2
  exit "$rc"
}
compiled_the_crate "$(cat "$log")" || {
  echo "package-check: cargo reported success without a 'Compiling k8rs' line, so this run compiled nothing and" >&2
  echo "        vetted nothing — exactly the 0.17s no-op measured on 2026-09-06 (NOTES § D133)." >&2
  echo "        Something is handing back an earlier build: check that the untar above still passes" >&2
  echo "        -m, and that $BUILD is not shared with another build of this package." >&2
  exit 1
}
echo "package-check: the published bytes compile their own tests — $crate, unpacked to $src, $CANARY/ dropped as $PWD/Cargo.toml asks"
