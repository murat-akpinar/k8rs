# The Phase 8 spike, re-reviewed — the four regions that changed after the first round

`k8s-admin`, 2026-09-05. Bounded re-review of `examples/spike_tui.rs` at
`509ef94`, covering only the regions the seven-finding round rewrote:
`App::watched` / `filling`, the `answered` guard, `clean()` / `unprintable()`,
and `because()` / `refuse()` in `main`.

Everything below was run against a **copy** of the tree at
`/home/shyuuhei/.cache/k8s-admin-review-copy`, with
`CARGO_TARGET_DIR=/home/shyuuhei/.cache/k8s-admin-review-target`. The working
tree was not touched. The live run used the PM's `kind-k8rs` cluster
**read-only** (one pod watch); pod count before and after: 41 and 41.

## 1. `unprintable()` — is it character-for-character `src/k8s.rs:251-256`?

    $ sed -n '67,71p' examples/spike_tui.rs > /tmp-scratch/a.txt
    $ sed -n '252,256p' src/k8s.rs        > /tmp-scratch/b.txt
    $ diff -u a.txt b.txt && echo "BODIES IDENTICAL"
    BODIES IDENTICAL
    $ md5sum a.txt b.txt
    71a86ae44921d207c4aa2fcb7a413d95  a.txt
    71a86ae44921d207c4aa2fcb7a413d95  b.txt

Bodies are byte-identical. The only difference across the whole item is the
signature line: `pub(crate) fn` in the product, `fn` in the spike.

## 2. The `answered` guard — is it `src/k8s.rs`'s?

Spike lines 250-257 against product lines 1560-1567, with the `watcher::`
path prefix and leading indentation normalised:

    $ sed -n '250,257p' examples/spike_tui.rs | sed 's/watcher:://g;s/^ *//' > sa.txt
    $ sed -n '1560,1567p' src/k8s.rs          | sed 's/^ *//'                > sb.txt
    $ diff -u sa.txt sb.txt && echo IDENTICAL
    IDENTICAL

## 3. `App::watched` — the event sequences

Driver: a `#[cfg(test)]` module appended to the **copy's** example file, run
with `cargo test --example spike_tui -- --nocapture --test-threads=1`. Each
sequence starts from `App::default()` with `failure = Some("boom")` so the
clear is observable.

    watched[init,a,b,done]              rows=["d/a","d/b"] cursor=0 filling=None failure=None    events=4
    watched[init,a,INIT,b,done]         rows=["d/b"]       cursor=0 filling=None failure=None    events=5
    watched[DONE only]                  rows=[]            cursor=3 filling=None failure=Some("boom") events=1
    watched[init,a,DEL(a),done]         rows=["d/a"]       cursor=0 filling=None failure=None    events=4
    watched[init,a,APPLY(z),done]       rows=["d/a"]       cursor=0 filling=None failure=None    events=4
    watched[3 rows, cursor 2, relist to 1]
                                        rows=["d/a","d/b","d/c"] cursor=2 filling=None failure=None events=5
    watched[after shrink relist]        rows=["d/a"]       cursor=0 filling=None failure=None    events=8
    watched[del to 1]                   rows=["d/a"]       cursor=0 filling=None failure=None    events=3
    watched[del to 0]                   rows=[]            cursor=0 filling=None failure=None    events=4
    watched[init,a,init,b,init]         rows=[]            cursor=0 filling=Some([]) failure=Some("boom") events=5
    watched[APPLY]                      rows=["d/a"]       cursor=0 filling=None failure=None    events=1
    watched[DELETE of an unknown pod]   rows=[]            cursor=0 filling=None failure=None    events=1

    test result: ok. 4 passed; 0 failed

Assertions that held: a second `Init` discards the partial list; a stray
`InitDone` publishes nothing and does not clear the failure; an unfinished
relist publishes nothing and does not clear the failure; a relist that shrinks
the list clamps the cursor; deletes down to an empty list leave `cursor = 0`.

`DONE only` shows `cursor=3` against `rows=[]` — that state was set by the
driver, not reached by the code. In the program `rows` shrinks in exactly two
places (`InitDone`, `Delete`) and both clamp; `key`'s `Down` clamps; `Up` only
decreases.

`init,a,DEL(a),done` publishes `d/a`: a `Delete` arriving mid-relist is
applied to the published map and then overwritten by the swap. `src/k8s.rs`
does the same (`Event::Delete` → `self.live.remove`). Under kube's `ListWatch`
strategy neither `Apply` nor `Delete` is emitted between `Init` and `InitDone`,
so the shape is unreachable in both.

## 4. `clean()` — the shapes

    clean[empty]                        chars=0   bytes=0   ellipsis=false  ""
    clean[all whitespace \n\t\r]        chars=0   bytes=0   ellipsis=false  ""
    clean[all unprintable non-ws]       chars=0   bytes=0   ellipsis=false  ""
    clean[mixed ws-unprintable]         chars=3   bytes=3   ellipsis=false  "a b"
    clean[leading break]                chars=1   bytes=1   ellipsis=false  "a"
    clean[trailing break]               chars=1   bytes=1   ellipsis=false  "a"
    clean[bidi trojan U+202E]           chars=7   bytes=7   ellipsis=false  "prodgnp"
    clean[nbsp kept U+00A0]             chars=3   bytes=4   ellipsis=false  "a\u{a0}b"
    clean[U+2028 kept]                  chars=3   bytes=5   ellipsis=false  "a\u{2028}b"
    clean[exactly 120 ascii]            chars=120 bytes=120 ellipsis=false
    clean[121 ascii]                    chars=120 bytes=122 ellipsis=true
    clean[300 4-byte chars]             chars=120 bytes=479 ellipsis=true
    clean[119 ascii + 1 emoji at 120]   chars=120 bytes=123 ellipsis=false
    clean[120 ascii + 1 emoji]          chars=120 bytes=122 ellipsis=true
    clean[50k newlines]                 chars=0   bytes=0   ellipsis=false  ""

A 120-character value is not cut; 121 becomes 119 characters plus `…`. The cut
is `chars().take(119)`, so a multi-byte character straddling the boundary is
never split — 300 emoji come back as 120 characters and 479 bytes, i.e. the
character cap admits ~4x the product's byte cap in bytes.

Draw under the same conditions, `ratatui::backend::TestBackend`:

    empty+dialog 80x24: drew, no panic
    empty+dialog 1x1:   drew, no panic
    empty+dialog 10x3:  drew, no panic
    empty+dialog 62x9:  drew, no panic
    row() of an all-unprintable pod -> key=["/"]
    all-unprintable row: drew, no panic

## 5. `because()` — the arm count and the classification

    $ awk 'NR>=130 && NR<=150' examples/spike_tui.rs | grep -c '=>'
    5

The five `=>` lines, by source line: 131, 134, 140, 146, 149. The doc comment
above the function says *"All six arms were reached and each printed its own
line."*

Reached, over the eight `KubeconfigError` variants that can be constructed
without a dependency the manifest does not carry:

    because[FindPath              ] -> the kubeconfig file is missing, or cannot be read
    because[ReadConfig            ] -> the kubeconfig file is missing, or cannot be read
    because[CurrentContextNotSet  ] -> the kubeconfig is valid YAML but does not describe a usable current context
    because[LoadContext           ] -> the kubeconfig is valid YAML but does not describe a usable current context
    because[LoadClusterOfContext  ] -> the kubeconfig is valid YAML but does not describe a usable current context
    because[MissingClusterUrl     ] -> the kubeconfig is valid YAML but does not describe a usable current context
    because[KindMismatch          ] -> the kubeconfig could not be used
    because[ApiVersionMismatch    ] -> the kubeconfig could not be used
    distinct sentences reached from these 8 variants: 3

`KubeconfigError` has fifteen variants (`kube-client-4.2.0/src/config/mod.rs:35-95`).
Thirteen are named explicitly by `because`; the two that reach the catch-all are
`KindMismatch` and `ApiVersionMismatch`.

Side by side with the product's classification
(`src/k8s.rs:1215` → `src/main.rs:2570-2588`):

| variant | product `Fault` | product sentence, in short | spike sentence |
|---|---|---|---|
| `LoadClusterOfContext` | `BadEntry` | this kubeconfig loaded, and something it points at did not | …does not describe a usable current context |
| `MissingClusterUrl` | `BadEntry` | same | …does not describe a usable current context |
| `ParseClusterUrl` | `BadEntry` | same | …does not describe a usable current context |
| `ParseProxyUrl` | `BadEntry` | same | …does not describe a usable current context |
| `KindMismatch` | `Kubeconfig` | the kubeconfig itself could not be read — missing, unreadable, or not valid YAML | the kubeconfig could not be used |
| `ApiVersionMismatch` | `Kubeconfig` | same | the kubeconfig could not be used |

`src/k8s.rs:1208-1210` records the first row deliberately: *"`LoadClusterOfContext`
is a `Fault::BadEntry` and not a `Fault::NoContext` … the context was found, and
the cluster block it names is missing. The context is not the thing to fix."*

Both `refuse` call sites (`main.rs` of the spike, lines 437 and 445) are above
`ratatui::init()` at line 449.

## 6. The initial-LIST window, on the live cluster

`kind-k8rs`, 41 pods, one read-only pod watch. The pane header sampled in a
tight loop from process start; duplicates collapsed:

    $ tmux new-session -d -s k8sadmin -x 100 -y 24 ".../examples/spike_tui"
    $ # capture-pane head -1, deduped, 400 samples

     0 pods · 0 watch events · cursor 0        (list box empty)
     0 pods · 1 watch events · cursor 0        (list box empty)
     0 pods · 4 watch events · cursor 0        (list box empty)
     0 pods · 14 watch events · cursor 0       (list box empty)
     0 pods · 23 watch events · cursor 0       (list box empty)
     0 pods · 34 watch events · cursor 0       (list box empty)
     41 pods · 43 watch events · cursor 0      (rows appear)

    === settled header ===
     41 pods · 43 watch events · cursor 0

42 of the 43 frames render `0 pods` inside a bordered `pods` box for a cluster
that has 41. Teardown: `tmux ls` → *no server running*; no `spike_tui` process;
cluster pod count unchanged at 41.

## 7. The dialog's target, under watch traffic and no keypress

    dialog open,                cursor=1 names="prod/c-app"
    after Apply(prod/a-app),    cursor=1 names="prod/b-app"  dialog=Some(0)
    after Delete(prod/b-app),   cursor=1 names="prod/c-app"  dialog=Some(0)
    after all deleted,          cursor=0 names="-"           dialog=Some(0)

Synthetic pods, not cluster objects. Independent reproduction of the retarget
already recorded in `PRIOR-ART § G1` and `NOTES § D240`, by a different
mechanism (an unrelated pod appearing rather than time passing).
