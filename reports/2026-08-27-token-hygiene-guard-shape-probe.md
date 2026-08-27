# Token-hygiene guard — which declaration shapes it parses (2026-08-27)

Operator review of the two Phase 5 boxes (todo.md:2540, todo.md:2566) as they
stand unstaged in the working tree. No cluster was used; every command below is
static analysis of `scripts/security-guard.py` against Rust snippets and against
a scratch copy of `src/` + `tests/`.

## Harness

The check is called directly so one snippet can be fed at a time. Copied into
`$SCRATCH/probe.py`:

```python
import importlib.util, tempfile
from pathlib import Path
spec = importlib.util.spec_from_file_location(
    "g", "/home/shyuuhei/GIT/k8rs/scripts/security-guard.py")
g = importlib.util.module_from_spec(spec); spec.loader.exec_module(g)

def probe(label, src):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp); (root / "src").mkdir()
        f = root / "src" / "k8s.rs"; f.write_text(src)
        g.code.cache_clear()
        problems, note = g.check_token_debug([f], root)
        print(label, "->", "CAUGHT" if problems else "WALKS THROUGH")
        for p in problems: print("   ", p)
        print("    note:", note)
```

The second harness appends a snippet to a copy of the real `src/k8s.rs` and runs
the check over the whole scratch tree, so the summary count can be compared with
the unmodified baseline:

```
cp -r /home/shyuuhei/GIT/k8rs/src /home/shyuuhei/GIT/k8rs/tests $SCRATCH/tree/
python3 $SCRATCH/realplant.py
```

## Baseline — the tree as it stands

```
$ python3 -c 'import ...; check_token_debug(sources(root), root)'
49 structs, 13 enums, 5 aliases, 5 can hold a token — blind to an unqualified
`Error` import and to `{:?}` on a bare local of a foreign type, both of which
need types a regex has not got
(no FAIL)
```

The five tainted declarations, with what matched:

```
SEED (direct hit on TOKEN_TYPE):
  struct Trouble   src/k8s.rs:839   derivesDebug=False  matched=['watcher::Error']
  struct Watch     src/k8s.rs:585   derivesDebug=False  matched=['watcher::Error']

FULL TAINT SET: ['NamedStream', 'Store', 'Trouble', 'Update', 'Watch']
counts: {'struct': 49, 'enum': 13, 'type': 5} tainted: 5
```

`Store` is reached from `Watch`, and `Update` / `NamedStream` from `Store`.

Names declared more than once in the tree today:

```
$ python3 $SCRATCH/collisions.py
names declared more than once in this tree today:
  Row: src/analysis.rs:173 enum | src/k8s.rs:2722 struct
```

## Shapes fed, one at a time

Each run appends the snippet to a copy of the real `src/k8s.rs` and prints the
whole-tree summary line, so a declaration that was not parsed is visible as an
unchanged count.

```
$ python3 $SCRATCH/realplant.py
--- baseline: the real tree, unmodified
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- plant 1: connect() state with a where clause
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- plant 2: derive on the same line as the declaration
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- plant 3: cfg_attr(test, derive(Debug))
    50 structs, 13 enums, 5 aliases, 6 can hold a token — …
    (no FAIL - check reports OK)
--- plant 4: an attribute whose string holds a ]
    50 structs, 13 enums, 5 aliases, 6 can hold a token — …
    (no FAIL - check reports OK)
--- plant 5: generic bound with a paren
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- control: the plain shape, which must be caught
    50 structs, 13 enums, 5 aliases, 6 can hold a token — …
    FAIL src/k8s.rs:3142  struct Session can hold a token and derives Debug —
         write it by hand and print no auth info (§ Token hygiene)
```

The five plants, verbatim:

```rust
// plant 1
#[derive(Debug)]
pub struct Session<T>
where
    T: Clone,
{
    client: kube::Client,
    tag: T,
}

// plant 2
#[derive(Debug)] pub struct Session { client: kube::Client }

// plant 3
#[cfg_attr(test, derive(Debug))]
pub struct Session { client: kube::Client }

// plant 4
#[derive(Debug)]
#[doc = "press ] to close"]
pub struct Session { client: kube::Client }

// plant 5
#[derive(Debug)]
pub struct Session<F: Fn(&str) -> bool> {
    client: kube::Client,
    pred: F,
}

// control
#[derive(Debug)]
pub struct Session { client: kube::Client }
```

## Shapes that are caught — negative results

```
$ python3 $SCRATCH/shapes1.py $SCRATCH/shapes2.py $SCRATCH/shapes3.py
A. struct holding a Client                                    CAUGHT
B. enum holding a Client                                      CAUGHT
C. tuple struct / newtype `struct Conn(kube::Client);`         CAUGHT
D. Arc / Box / Option / Vec / HashMap around it                CAUGHT
E. generic holder instantiated in a field, `Slot<kube::Client>` CAUGHT (the owner)
H. rustfmt wrapped the field type onto the next line           CAUGHT
L. declaration nested in a `mod`                               CAUGHT
N. raw string in an attribute with a balanced [Client]         CAUGHT
O. `#[doc = "plain"]` between derive and declaration           CAUGHT
Q. `#![…]` inner attribute at the top of the file              CAUGHT
R. attribute on a field rather than the declaration            CAUGHT
S. blank line between derive and declaration                   CAUGHT
U. multi-line generic list                                     CAUGHT
V. enum struct-variant `Connected { client, cfg }`             CAUGHT
W. two hops: `Arc<Mutex<Inner>>` behind a `type` alias          CAUGHT
Y. `struct Conn(pub String, pub kube::Client);`                CAUGHT
Z. tuple-typed field `(String, kube::Client)`                  CAUGHT
enum variant merely NAMED `Config(String)` / `Client(u8)`      clean (correct)
`crate::ui::Configuration` field                               clean (correct)
```

## Shapes that walk through

```
F.  generic holder alone, instantiated only at a call site     WALKS THROUGH
G'. where clause on the declaration                            WALKS THROUGH (not parsed)
I'. derive and declaration on one line                         WALKS THROUGH (not parsed)
J.  `#[cfg_attr(test, derive(Debug))]`                         WALKS THROUGH (parsed, tainted, derive missed)
K.  generic default `struct Conn<C = kube::Client>`            WALKS THROUGH
M.  attribute whose string literal contains a `]`              WALKS THROUGH (parsed, tainted, derive missed)
N'. raw string with an unbalanced `]`                          WALKS THROUGH (parsed, tainted, derive missed)
P.  `#[cfg_attr(feature = "dbg", derive(Debug))]`              WALKS THROUGH
T.  generic bound containing a paren, `F: Fn(&str) -> bool`    WALKS THROUGH (not parsed)
X.  `use kube::Client as Kc;` then a field of type `Kc`        WALKS THROUGH
```

Effect of relaxing `DERIVES_DEBUG` from `#\[derive\([^)]*\bDebug\b` to
`\bderive\([^)]*\bDebug\b`, on plant 3:

```
--- with DERIVES_DEBUG relaxed to r'\bderive\([^)]*\bDebug\b':
    ['src/k8s.rs:2  struct Conn can hold a token and derives Debug — …']
```

## `Config\b`

```
--- AA. watcher::Config, which carries no credential
    src/k8s.rs:5  struct Spec can hold a token and derives Debug — …
```

Snippet:

```rust
#[derive(Debug)]
pub struct Spec { wc: watcher::Config }
```

`src/k8s.rs` names `watcher::Config` today only in comments (`:1470`, `:2948`),
which `strip_comments` blanks, so nothing in the tree is tainted by it yet:

```
$ grep -n "watcher::Config" src/k8s.rs
1470:// kube as `watcher::Config::default().page_size(INITIAL_LIST_PAGE)`, and there is no
2948:// watcher::watcher(api, watcher::Config::default())
```

## Same-name declarations in two files

```
$ python3 $SCRATCH/collide.py
--- collision: clean Session derives Debug in analysis.rs,
    tainted Session does NOT in k8s.rs
    FAIL src/analysis.rs:2  struct Session can hold a token and derives Debug — …
--- collision: tainted Session derives Debug in k8s.rs,
    clean one first in analysis.rs
    FAIL src/analysis.rs:1  struct Session can hold a token and derives Debug — …
```

Files fed in the first case:

```rust
// src/analysis.rs
#[derive(Debug)]
pub struct Session { label: String }

// src/k8s.rs
pub struct Session { client: kube::Client }
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut Formatter) -> Result { f.write_str("Session") }
}
```

## Format-call shapes

Appended to a copy of the real `src/k8s.rs`; whole-tree run each time.

```
$ python3 $SCRATCH/realplant2.py
--- plant 6: Display on a watcher::Error at a call site
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- plant 7: the same via anyhow, and `{c:?}` on a kube::Config local
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
--- plant 8: a hand-written Debug that formats the error whole
    50 structs, 13 enums, 5 aliases, 6 can hold a token — …
    (no FAIL - check reports OK)
--- plant 9: `.to_string()` on a kube error
    49 structs, 13 enums, 5 aliases, 5 can hold a token — …
    (no FAIL - check reports OK)
```

Plants:

```rust
// plant 6
pub fn render_failure(t: &Trouble) -> String {
    match t.failure { Some(e) => format!("watch failed: {e}"), None => String::new() }
}

// plant 7
pub async fn boot() -> anyhow::Result<()> {
    let c = kube::Config::from_kubeconfig(&Default::default()).await?;
    eprintln!("{c:?}");
    Ok(())
}

// plant 8
pub struct Held { e: watcher::Error }
impl std::fmt::Debug for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.e)
    }
}

// plant 9
pub fn why(e: &watcher::Error) -> String { e.to_string() }
```

## kube derive facts read off the crate sources

```
$ grep -n -B4 "^pub struct Client" kube-client-4.2.0/src/client/mod.rs
86-#[derive(Clone)]
87:pub struct Client {

$ grep -n -B4 "pub struct Api<K>" kube-client-4.2.0/src/api/mod.rs
50-#[derive(Clone)]
51:pub struct Api<K> {

$ grep -n -B4 "pub struct ClientBuilder" kube-client-4.2.0/src/client/builder.rs
29:pub struct ClientBuilder<Svc> {

$ sed -n '27,29p' kube-runtime-4.2.0/src/watcher.rs
#[derive(Debug, Error)]
pub enum Error {
```

`kube::Client`, `kube::Api` and `ClientBuilder` do not implement `Debug`;
`watcher::Error` and `kube::Config` do.

## Existing shapes in the tree, for the misses above

```
$ grep -rn '#\[[^]]*\][ \t]*\(pub \)\?\(struct\|enum\)' src tests
(no output)

$ grep -rn 'cfg_attr' src tests
src/analysis.rs:59:#![cfg_attr(
src/k8s.rs:100:#![cfg_attr(

$ grep -rn '^\s*where\s*$' src
src/analysis_tests.rs:140:where
src/k8s_tests.rs:632:where
src/k8s.rs:647:    where
src/k8s.rs:1822:where

$ grep -rn 'anyhow' src/*.rs
(no output)
```

The four `where` lines are on `impl` blocks and functions, not on a `struct` or
`enum` declaration. Both `cfg_attr` uses are `#![…]` inner attributes.

## What `cargo fmt` does to the three unparsed shapes

`just check` runs `cargo fmt --all -- --check` (justfile:40), so a shape rustfmt
rewrites cannot survive the gate and a shape it produces is the one the tree will
hold.

```
$ cat fmt.rs
#[derive(Debug)] pub struct SameLine { client: kube::Client }

#[derive(Debug)]
pub struct WhereClause<T>
where
    T: Clone,
{
    client: kube::Client,
    tag: T,
}

#[derive(Debug)]
pub struct ParenBound<F: Fn(&str) -> bool> {
    client: kube::Client,
    pred: F,
}

$ rustfmt --edition 2021 --emit stdout fmt.rs
#[derive(Debug)]
pub struct SameLine {
    client: kube::Client,
}

#[derive(Debug)]
pub struct WhereClause<T>
where
    T: Clone,
{
    client: kube::Client,
    tag: T,
}

#[derive(Debug)]
pub struct ParenBound<F: Fn(&str) -> bool> {
    client: kube::Client,
    pred: F,
}
```

And an inline `where` written by hand is rewritten into the unparsed form:

```
$ cat fmt2.rs
#[derive(Debug)]
pub struct Inline<T> where T: Clone { client: kube::Client, tag: T }

#[cfg_attr(test, derive(Debug))]
pub struct Gated { client: kube::Client }

$ rustfmt --edition 2021 --emit stdout fmt2.rs
#[derive(Debug)]
pub struct Inline<T>
where
    T: Clone,
{
    client: kube::Client,
    tag: T,
}

#[cfg_attr(test, derive(Debug))]
pub struct Gated {
    client: kube::Client,
}
```

## String literals are not opaque to the bracket matchers

`strip_comments` skips a string literal without blanking it, so its contents stay
in the text `ATTRS` and `balanced` read. Two symptoms, one cause. The attribute
one is plants M and N' above; the body one:

```
--- AC. a } inside a string in a field attribute truncates the body: WALKS THROUGH
    note: 2 structs, 0 enums, 0 aliases, 0 can hold a token — …
```

```rust
#[derive(Debug, Clone)]
pub struct Neighbour { name: String }

#[derive(Debug)]
pub struct Conn {
    #[serde(rename = "}")]
    tag: String,
    client: kube::Client,
}
```

The struct is counted (2 structs) and `client` is not in its field list.

Two other checks in the same file do read string contents — `check_outbound`
matches a literal `https://host` and `check_shell` matches `"sh -c …"` — so
`code()` is shared and cannot blank strings for all six.

## A cross-check on the parsed count

`DECL` and the `kinds` counter are the same regex, so the summary count cannot
reveal its own miss, and the canary fires only when `decls` is empty. A
line-anchored keyword count is exact on this tree:

```
$ python3 - <<'EOF'   # NAIVE = r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+\w+", re.M
whole tree: DECL=62 naive-keyword-count=62
EOF
```

and disagrees on two of the three unparsed shapes:

```
where clause DECL=0 naive=1  -> MISS DETECTED
same line    DECL=0 naive=0  -> agrees
paren bound  DECL=0 naive=1  -> MISS DETECTED
control ok   DECL=1 naive=1  -> agrees
```

An unanchored `\b(?:struct|enum)\s+\w+` catches the same-line shape too but is
not exact on this tree — one hit sits inside a string literal:

```
whole tree: DECL=62 unanchored=63
  src/rules_tests/pod.rs: DECL=0 unanchored=1
$ sed -n '3680p' src/rules_tests/pod.rs
             the half that was settled from a struct literal until this capture (NOTES § D93)",
```
