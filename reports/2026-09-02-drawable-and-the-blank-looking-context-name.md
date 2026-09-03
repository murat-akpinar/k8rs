# `drawable` and the blank-looking context name — operator review measurements (2026-09-02)

Subject: the uncommitted `src/k8s.rs` / `src/k8s_tests.rs` change that introduces
`fn drawable(String) -> Option<String>` and turns `Choice::name` into
`Option<String>` (NOTES § D202).

**No cluster was touched.** Every measurement below is against a kubeconfig the
test file writes itself; nothing here came off an API server, so there is no
cluster output, no node name and no credential in this file.

## How these were run

The shared working tree was never edited. The tree was copied to an agent
scratch directory with its own `CARGO_TARGET_DIR`
(CLAUDE.md § *a sweep that edits in place is a writer*, NOTES § D185); probe
tests were appended to the **copy** only.

```
rsync -a --exclude target --exclude tmp --exclude 'mutants.out*' --exclude .git \
      /home/shyuuhei/GIT/k8rs/ $SCRATCH/repo/
cd $SCRATCH/repo && CARGO_TARGET_DIR=$SCRATCH/target cargo test --offline ...
```

## 1. The recorded test counts, re-run

```
$ cargo test --offline --bin k8rs
running 852 tests
test result: ok. 852 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.68s

$ cargo test --offline          # tests/binary.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
```

852 = the recorded **849** plus the three probe tests this review appended to the
copy. The `23` is unchanged.

## 2. Red run — the box's own test against the pre-change `contexts()`

`name: drawable(named.name.clone())` replaced in the copy by the shape the diff
removed, with the field type left as `Option<String>`:

```rust
name: Some({ let mut n = named.name.clone(); text(&mut n, IDENTIFIER); n }),
```

```
$ cargo test --offline a_context_name_that_strips_to_nothing -- --nocapture
name Some("") · key "" · current true
name Some("") · key "\u{200b}\u{202e}" · current false
name Some("(unnamed)") · key "(unnamed)" · current false
name Some("k8rs-tests-plain") · key "k8rs-tests-plain" · current false

panicked at src/k8s_tests.rs:9455:5:
assertion `left == right` failed: a row whose name strips to nothing is not `None`, ...
  left: [Some(""), Some(""), Some("(unnamed)"), Some("k8rs-tests-plain")]
 right: [None, None, Some("(unnamed)"), Some("k8rs-tests-plain")]
test result: FAILED. 0 passed; 1 failed
```

Restored, green.

## 3. `drawable` over every shape that draws as a blank name slot

```
drawable(empty = "")                          = None
drawable(zero-width + bidi = "\u{200b}\u{202e}") = None
drawable(newline only = "\n")                 = None
drawable(tab only = "\t")                     = None
drawable(soft hyphen only = "\u{ad}")         = None
drawable(ascii space only = " ")              = Some(" ")
drawable(two ascii spaces = "  ")             = Some("  ")
drawable(nbsp only = "\u{a0}")                = Some("\u{a0}")
drawable(ideographic space only = "\u{3000}") = Some("\u{3000}")
drawable(healthy = "k8rs-tests-plain")        = Some("k8rs-tests-plain")
```

The same four shapes through the real pipeline, `current-context: " "`:

```
key " "        -> name Some(" ")        · current true  · shadowed false
key "\u{a0}"   -> name Some("\u{a0}")   · current false · shadowed false
key ""         -> name None             · current false · shadowed false
key "\u{200b}" -> name None             · current false · shadowed false
kubeconfig_context(None) = Some(" ")
```

## 4. `renewal` measured beside `drawable`

`renewal` (`src/k8s.rs:7519-7523`) still carries the three lines `drawable` now
owns, and was not routed through it:

```
renewal(command="")          = None        · drawable(same) = None
renewal(command="\u{200b}")  = None        · drawable(same) = None
renewal(command=" ")         = Some(" ")   · drawable(same) = Some(" ")
renewal(command="aws")       = Some("aws") · drawable(same) = Some("aws")
```

## 5. `namespace_of` and `written_tag` through `contexts()` after the refactor

One context per row, `namespace:` and the `k8rs` extension's `tag:` varied:

```
key "a" (namespace "",        tag "")        -> namespace None            · tag Blank
key "b" (namespace "​",  tag "‮")  -> namespace None            · tag Blank
key "d" (namespace " ",       tag " ")       -> namespace Some(" ")       · tag Written(" ")
key "e" (namespace kube-system, tag prod)    -> namespace Some("kube-system") · tag Written("prod")
key "f" (no namespace, no extensions)        -> namespace None            · tag Blank
key "g" (extension named `other`)            -> namespace None            · tag Blank
key "h" (k8rs extension, key `notatag`)      -> namespace None            · tag Blank
key "i" (k8rs extension, tag: 7 — not a string) -> namespace None         · tag Blank
kubeconfig_namespace(None) = None
```

## 6. Reach of the type change

```
$ grep -rn "Choice" --include="*.rs" . | grep -v '^./target/' \
      | grep -v '^./src/k8s.rs:' | grep -v '^./src/k8s_tests.rs:'
(no matches)

$ grep -rn "contexts(" src/ --include="*.rs" | grep -v 'k8s.rs:\|k8s_tests.rs:'
(no matches)

$ grep -rn "unnamed" src/
src/k8s.rs:6803,6804,6924   (doc comments only)
src/k8s_tests.rs:9420,9429,9444,9460,9462,9469  (test prose and test data only)
```

`main.rs` declares `mod k8s;` and imports no item from it by `use`; every other
product file reads `k8s::` by qualified path. No `String` placeholder is emitted
from `src/`.

## 7. C1's input path, read end to end

```
kubeconfig_context()  src/k8s.rs:6832  -> Option<String>
Session::context      src/k8s.rs:6469 / assigned 6781
Identity::context     src/k8s.rs:1627 / assigned 1664  (session.context.clone())
ClusterSnapshot::context  src/k8s.rs:2022 (identity.context.clone())
rules::kubeconfig_certificate_expiring  src/rules.rs:7844  `snapshot.context.as_deref()?`
```

No line on that path changed in the diff.
