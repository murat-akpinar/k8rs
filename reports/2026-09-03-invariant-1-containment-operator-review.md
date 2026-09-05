# Invariant 1's containment, measured — `ops.rs`, `clippy.toml`, `write-guard.py`

Operator review of Phase 7 box 1 (`ops.rs` with the single
`#![allow(clippy::disallowed_methods)]`, plus `write-guard.py`'s new
single-exception check). Everything below ran on the dev machine against a copy
of the working tree in the scratchpad, with its own `CARGO_TARGET_DIR`
([D185](../NOTES.md#d185--cleanup-on-the-last-line-is-not-cleanup-and-the-resource-is-not-always-a-file-2026-08-30)).
No cluster was involved. Copy made from `src/`, `Cargo.toml`, `Cargo.lock`,
`clippy.toml`, `scripts/`, `justfile` as they stood with the box's four
uncommitted files in place.

Toolchain, both from this machine:

    $ rustc -V
    rustc 1.97.1 (8bab26f4f 2026-07-14) (Arch Linux rust 1:1.97.1-1.1)
    $ clippy-driver -V ; cargo clippy -V
    clippy 0.1.97
    clippy 0.1.97

## 1 — the guard and its self-test, on the real repo

    $ python3 scripts/write-guard.py
    write-guard: 72 methods known across 2 types (kube::Api 48, kube::core::Request 24),
    45 banned outside src/ops.rs, clippy.toml names exactly those, and src/ops.rs is the
    only file in 5 cargo roots that silences any of the 4 lints whose `allow` would turn
    the ban off — OK
    $ echo $?
    0

The derived silencing set, read out of the guard's own functions against this
machine's clippy:

    $ python3 -c "<load scripts/write-guard.py>; print(sorted(silencing_lints(clippy_help())))"
    ['clippy::all', 'clippy::disallowed_methods', 'clippy::style', 'warnings']

`clippy-driver -W help` prints the group table on **both** stdout and stderr on
this build — `grep -c '^ *clippy::style  '` returns 1 against each stream — so
`out.stdout + out.stderr` sees it twice and the set is unaffected.

## 2 — the ban is live, both directions

Baseline: the unmodified copy is green.

    $ cargo clippy --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.97s

`Api::delete` appended to `src/k8s.rs`:

    error: use of a disallowed method `kube::Api::delete`
    error: could not compile `k8rs` (bin "k8rs") due to 1 previous error

The identical call appended to `src/ops.rs` instead, under the box's attribute:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.03s

## 3 — a complete DELETE from `src/k8s.rs`, green

Appended to `src/k8s.rs`. Every line except the middle one is a line
`skew()` (k8s.rs:8157-8160) already contains; no new import, no `Api`, no method
on `clippy.toml`'s list.

    #[allow(dead_code)]
    async fn operator_probe_delete(client: &Client, path: &str, name: &str) -> Option<()> {
        let mut asked = Request::new(path).get(name, &GetParams::default()).ok()?;
        *asked.method_mut() = "DELETE".parse().ok()?;
        let answered = client.send(asked.map(Body::from)).await.ok()?;
        answered.status().is_success().then_some(())
    }

    $ cargo clippy --all-targets -- -D warnings
    Checking k8rs v0.0.0 (…/probe)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.10s

`python3 scripts/write-guard.py` over the same tree: exit 0.

## 4 — two lines in `clippy.toml` close it, and cost nothing

Added to `disallowed-methods`:

    "http::Request::method_mut",
    "http::request::Builder::method",

Against the probe above:

    error: use of a disallowed method `http::Request::method_mut`
    error: could not compile `k8rs` (bin "k8rs") due to 1 previous error

Against the unmodified tree with the same two entries present:

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.15s

The `http` crate cannot be named from this crate, so `Request::from_parts` is
not an escape from the two above:

    fn operator_probe_name_http() { let _ = http::Method::DELETE; }
    error[E0433]: cannot find module or crate `http` in this scope

`grep -rn "pub use http" ~/.cargo/registry/src/*/kube-*4.2.0/src/` returns
nothing, so kube 4.2.0 re-exports no path to it either.

Note: `write-guard.py` requires `clippy.toml` to name **exactly** the derived
set, so these two entries make it fail as written.

## 5 — `RUSTFLAGS`, and where it is actually set

`justfile:14` and `.github/workflows/ci.yml:17` each set `RUSTFLAGS` for every
build of this project. Patching the justfile line to
`"-D warnings -A clippy::disallowed_methods"`, with `Api::delete` sitting in
`src/k8s.rs`:

    $ python3 scripts/write-guard.py
    write-guard: … src/ops.rs is the only file in 5 cargo roots that silences any of
    the 4 lints whose `allow` would turn the ban off — OK
    exit=0
    $ just clippyonly            # `cargo clippy --locked --all-targets --all-features -- -D warnings`
    cargo clippy --locked --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
    exit=0

The same flag on the command line, for the shape without `just`:

    $ RUSTFLAGS="-D warnings -A clippy::disallowed_methods" cargo clippy --all-targets --all-features -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.04s

A specific-lint `-A` beats the group-level `-D warnings` that follows it on the
same rustc invocation.

## 6 — two `.cargo/config.toml` shapes that are **not** holes

`[env] RUSTFLAGS = "-D warnings -A clippy::disallowed_methods"`, with
`Api::delete` in `src/k8s.rs`:

    error: use of a disallowed method `kube::Api::delete`

(cargo reads `RUSTFLAGS` from its own environment, not from `[env]`. The
guard's `"rustflags" in text` match is case-sensitive and does not fire on this
file; both facts point the same way here.)

`[alias] clippy = "check"`:

    Usage: check [OPTIONS]
    For more information, try '--help'.

The `-- -D warnings` suffix `just check` and CI both pass makes the aliased
command fail to parse, so this one is loud.

## 7 — comments and string literals are counted as attributes

Against the guard's own `attributes()` / `silences()` with the derived set:

    CARRIER  line comment quoting the attribute        // … #![allow(clippy::disallowed_methods)]
    CARRIER  doc comment quoting it                    /// One file, one `#![allow(clippy::disallowed_methods)]`.
    CARRIER  module doc quoting it                     //! … #![allow(clippy::disallowed_methods)]
    CARRIER  string literal in a test                  const S: &str = "#![allow(clippy::disallowed_methods)]";
    CARRIER  block comment                             /* #[allow(clippy::all)] */

In `src/ops.rs` specifically, any one of those makes `exception_drift` report
`src/ops.rs carries 2 silencing attributes`.

One false negative found, adversarial rather than accidental: a comment inside
the path, `#[allow(clippy:: /* c */ disallowed_methods)]` — the `\s*::\s*`
un-spacing leaves `clippy::/*` and the token scan yields `disallowed_methods`
bare, which is not in the derived set.

## 8 — what `toolchain-guard.py` compares

`scripts/toolchain-guard.py` reads `rustc --version` and `cargo clippy
--version` and refuses a mismatch. It does not read `clippy-driver --version`,
which is the binary `write-guard.py` now derives its lint set from.
