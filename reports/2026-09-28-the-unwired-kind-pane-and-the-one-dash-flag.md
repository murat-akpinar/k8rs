# 2026-09-28 — the unwired kind pane, `?`'s three marks, and the one-dash flag

Measured against the working tree at `/home/shyuuhei/GIT/k8rs`, `development`,
uncommitted D310/D311 fix in place. **Nothing was compiled and no cluster was
started** — `tester` held the mirror this turn, so every figure below is read off
the source and the screen files with `grep`, `sed` and `python3`. Each row says
which.

## 1 — where `View::Resources` is entered, and what touches `App::focus`

```
$ grep -n "View::Resources" src/*.rs | grep -v _tests
src/ui.rs:1697:        View::Resources(at) => match screen.browser {
src/ui.rs:3595:        View::Resources(nth) => browser(frame, area, app, screen, screen.kinds.get(nth)),
src/views.rs:3181:            (Detailing::Closed, _, View::Alerts | View::Resources(_))
src/views.rs:3190:            (Detailing::Closed, _, View::Alerts | View::Resources(_)) => match offer {
src/views.rs:3498:            NavItem::Kind(at) => self.view = View::Resources(at),
```

(Doc-comment hits at `src/ui.rs:751`, `:1719`, `:3672` and `src/main.rs:9377`
omitted — comments, not code.)

```
$ grep -n "Panel::Sidebar\|Panel::Content\|pub focus" src/views.rs
2171:pub enum Panel {
2185:            Panel::Sidebar => Panel::Content,
2186:            Panel::Content => Panel::Sidebar,
2257:    pub focus: Panel,
3432:            None if back => self.focus = Panel::Sidebar,
```

```
$ grep -n "focus" src/main.rs
9898:    // filter that has focus or a dialog that is open, and a shell the reader cannot get back to is
9912:    // **While a filter has focus every printable key is text** — `s`, `q`, `X` and `?` included
9925:                // that here** — a filter has focus, so no picker is open — but the value is
10016:            console.app.focus = console.app.focus.next();
10332:    match console.app.focus {
10454:        views::Detailing::Closed => match console.app.focus {
```

```
$ grep -n "\.open(\|app\.open\b" src/main.rs src/ui.rs src/views.rs | grep -v _tests
src/main.rs:10465:                console.app.open(*item);
```

`src/main.rs:10465` sits inside `views::Panel::Sidebar =>` (arm opened at
`:10454`). `App::open` (`src/views.rs:3494`–`:3512`) assigns `view`, `content`,
`filters` and `typing`; the string `focus` does not appear in its body.
`console.app.focus` is written in exactly one place in `main.rs`, `:10016`, the
`KeyCode::Tab` arm.

## 2 — `HELP`'s sixteen rows, display columns

```
$ python3 - <<'EOF'
import re, unicodedata
src = open('src/ui.rs').read()
body = re.search(r'const HELP: &str = "(.*?)";\n', src, re.S).group(1)
def w(s): return sum(2 if unicodedata.east_asian_width(c) in 'WF' else 1 for c in s)
for i, line in enumerate(body.split('\n'), 1):
    print(f"{i:2} {w(line):3} |{line}|")
print("rows:", len(body.split('\n')))
EOF
 1  15 |  Moving around|
 2  62 |    ↑ ↓ / j k    move            ⏎     open the selected thing|
 3  51 |    tab          next panel      esc   back / close|
 4  31 |    X            switch cluster|
 5  57 |    [ ]          detail tabs     / n   filter · namespace|
 6  19 |  Looking at things|
 7  61 |    l  logs, with the log from before a crash (not built yet)|
 8  66 |       log tab: not built yet — f follow, c container, ⇧p previous|
 9  68 |    d  describe — the object and what happened to it (not built yet)|
10  74 |    y  view as YAML (not built yet)   ctrl-z  back to your shell — type fg|
11  62 |  Changing things (each one asks first, and shows the command)|
12  68 |    s       not built yet — there is no way yet to type a copy count|
13  65 |            works on a deployment, a statefulset and a replicaset|
14  60 |    r       restart, at its own pace       (rollout restart)|
15  64 |            works on a deployment, a statefulset and a daemonset|
16  49 |    ctrl-d  delete — you type the name to confirm|
rows: 16
```

`screens/help.md`'s intro claims sixteen rows and a widest row of 74 against a
78-column ceiling. Both figures reproduce.

## 3 — mockup frame widths per fenced block

```
$ python3 - <<'EOF'   # widths of every fenced block, help.md and states.md
... (full script in § 2's shape; per-block distinct widths)
EOF
screens/help.md    block 1: rows=24 widths=[80]
screens/help.md    block 2: rows=24 widths=[80]
screens/states.md  block  8: rows=21 widths=[68, 80]
screens/states.md  block 25: rows=23 widths=[79, 80]
screens/states.md  block 33: rows=23 widths=[79, 80]
```

The three 80-column full-screen blocks in `states.md`, first row only:

```
block  8   68 | nodes 3/3                      k8rs     ctx: prod-eu · live · admin|
block 25   79 | nodes 3/3     ctx: prod-eu · ns: payments · read-only · ⚠ your clock is behind|
block 33   79 | nodes 3/3     ctx: prod-eu · ns: payments · read-only · ⚠ your clock is behind|
```

Block 8 is the new § *A kind the browser cannot list yet* 80-column redraw.

## 4 — fenced-block order inside the new section

```
$ awk '/^## A kind the browser cannot list yet/,/^## The filter hides every row/' \
      screens/states.md | grep -n '^```\|^ nodes\|^┌'
14:```
15: nodes 3/3                      k8rs     ctx: prod-eu · live · admin
16:┌────────────────────┬───────────────────────────────────────────────┐
36:```
108:```
109:┌───────────────────────────────────────────────┐
115:```
125:```
126: nodes 3/3                      k8rs     ctx: prod-eu · live · read-only
127:┌────────────────────┬───────────────────────────────────────────────┐
147:```
166:```
167: nodes 3/3                      k8rs     ctx: prod-eu · live · admin
168:┌────────────────────┬─────────────────────────────────────────────────────────┐
188:```
```

Four blocks. The footer-less one (title row alone, 49 columns wide) is the
**second**; `src/ui_tests.rs`'s new loop comment names it the fourth.

## 5 — `ui::unwired`'s sentence against the pane widths the section states

`screens/states.md` § *A kind the browser cannot list yet* states the content
pane is 47 columns at the file's 70-column page and 57 at the 80-column floor.

```
$ python3 - <<'EOF'
import unicodedata
def w(s): return sum(2 if unicodedata.east_asian_width(c) in 'WF' else 1 for c in s)
prefix = "not built yet — k8rs cannot list "
print("prefix columns:", w(prefix))
for p in [...]:
    n = w(prefix + p); print(f"{n:3} {'fits' if n<=57 else 'WRAPS'}@57 {'fits' if n<=47 else 'WRAPS'}@47  {prefix+p}")
EOF
prefix columns: 33
 37  fits@57  fits@47   not built yet — k8rs cannot list jobs
 37  fits@57  fits@47   not built yet — k8rs cannot list pods
 43  fits@57  fits@47   not built yet — k8rs cannot list csidrivers
 53  fits@57  WRAPS@47  not built yet — k8rs cannot list csistoragecapacities
 55  fits@57  WRAPS@47  not built yet — k8rs cannot list persistentvolumeclaims
 56  fits@57  WRAPS@47  not built yet — k8rs cannot list volumeattributesclasses
 58  WRAPS@57 WRAPS@47  not built yet — k8rs cannot list customresourcedefinitions
 62  WRAPS@57 WRAPS@47  not built yet — k8rs cannot list mutatingwebhookconfigurations
 64  WRAPS@57 WRAPS@47  not built yet — k8rs cannot list validatingwebhookconfigurations
 66  WRAPS@57 WRAPS@47  not built yet — k8rs cannot list validatingadmissionpolicybindings
```

Every plural above is a Kubernetes built-in, not a name from any cluster.
`ui::dimly` (`src/ui.rs`) wraps on whitespace via `wrapped(said, area.width)`;
`wrapped` (`src/ui.rs:6548`) breaks at the first whitespace, so the plural
becomes its own line. `Browsable::plural` is bounded at `IDENTIFIER` = 512 bytes
(`src/k8s.rs:205`, `:645`–`:654`), not at the sidebar's column count.

## 6 — the flag doors: which parser sees which line

```
$ grep -n "mistyped(\|ops_line(" src/main.rs | head
132:        Ok(args) => match ops_line(&args, ops::audit_log, ops_performed, may_i_started) {
134:            None => Ended::refused(match mistyped(&args) {
2215:fn mistyped(args: &[String]) -> Option<String> {
5949:fn ops_line(
```

`ops_line` returns `Some(Ended)` on every line where `ops_at` (`src/main.rs:5859`)
finds `OPS`; `mistyped` is reached only through its `None`. `flag_word`
(`:6003`) excludes an all-digit tail, so `-3` reaches `ops_words`' word list and
then `refuse_count` (`:6138`, `below_none()`).

`known` inside `mistyped` (`src/main.rs:2389`–`:2406`) lists, in order:
`--analysis --once --read-only --context --namespace --logs --describe --yaml
--object --container --kind --previous --follow -n`, plus the `=`-attached forms
of `--context --namespace -n --object --container --kind`. `--subresource`
(`:5599`) is not in it.

The refusal is `args.iter().find(|arg| arg.starts_with('-') && !known(arg))`
(`src/main.rs:2418`), scanning every word on the line including flag values. The
`--context`-followed-by-a-flag check above it (`:2216`–`:2224`) still tests
`pair[1].starts_with(FLAG)` with `FLAG == "--"`.

## 7 — the test helpers' `App::focus` on the new assertions

```
$ grep -n "fn opened()" -A 4 src/ui_tests.rs
4567:fn opened() -> App {
4568-    let mut app = App::default();
4569-    app.open(NavItem::Kind(0));
4570-    app
4571-}
```

`App::default()` derives `Panel`'s `#[default]`, which is `Panel::Content`
(`src/views.rs:2176`). `src/views_tests.rs`'s new `browsing()` closure sets
`focus: Panel::Content` explicitly.
