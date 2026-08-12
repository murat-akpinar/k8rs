# screens/ — what each screen looks like

Mockups of every screen in the TUI, one file per menu. These are **design-phase
artifacts**: the code has to match them, so a change here is a change to the
plan and belongs in the same edit as the decision behind it
([NOTES.md](../NOTES.md)).

| File | Screen |
|---|---|
| [alerts.md](alerts.md) | **Alerts** — the default view, findings grouped by owner |
| [resources.md](resources.md) | **Resources** — the browser over every kind, server-side columns |
| [analysis.md](analysis.md) | **Analysis** — capacity · drain safety · waste · certificates · versions |
| [detail.md](detail.md) | Object detail — logs · describe · yaml · events |
| [dialogs.md](dialogs.md) | Confirmations, the typed-name delete, and a refused write |
| [help.md](help.md) | The `?` key map |
| [context.md](context.md) | **Switching cluster** (`X`) — the picker and a failed switch |
| [once.md](once.md) | **`k8rs --once`** — the printed report that ships as v0.0.1, before the TUI |
| [states.md](states.md) | Empty · loading · disconnected · namespace-scoped · startup errors |
| [widgets.md](widgets.md) | **How they are drawn** — element → ratatui widget → who owns the state |

## How to read them

Every mockup is drawn **70 columns wide** so that it fits inside **80×24**, the
minimum supported terminal: 68 inner columns split 20 (sidebar) + 47 (content),
plus the two outer borders. The sidebar stays 20 wide at any terminal size —
the spare columns of a wider terminal all go to the content pane
([widgets.md § The frame](widgets.md#1-the-frame)). If a layout only works
wider than the minimum, it does not work.

The unbordered line above each frame is the **header row**: cluster vitals
left, `k8rs` centred, context and connection state right
([widgets.md § The header row](widgets.md#1a-the-header-row)).

Two panes, never more — navigation on the left, one content pane on the right,
plus the header row, the command log strip and the key footer. What makes k9s
feel complex is its panel layering; the discipline here is lazygit's.

## The four rules every screen obeys

1. **Plain language.** Every visible string is written for someone who does not
   yet know the jargon. `CrashLoopBackOff` gets explained, not printed.
2. **The keys are on screen.** The footer shows what is valid right now; `?`
   shows everything. Nothing important is reachable only from memory.
3. **The command is shown.** Every command k8rs runs appears in the log strip
   as the user would have typed it — the teaching device and the audit trail
   in one panel.
4. **Symbol *and* colour.** `● ▲ ○` carry the severity on their own; colour
   only reinforces it. Copy-pasteable, and readable when colour is not.
   `⚠` is the fourth symbol and it is not a severity: it marks a connection or
   trust problem — disconnected, login expired, TLS not verified — and appears
   in the header row or a banner, never on a finding.
