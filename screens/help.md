# Screen — Help (`?`)

The footer always shows the keys valid *right now*; `?` shows all of them. A
tool for beginners may not hide its verbs behind memory.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌ Keys ──────────────────────────────────────────────────────────────┐
│  Moving around                                                     │
│    ↑ ↓ / j k    move            ⏎     open the selected thing      │
│    tab          next panel      esc   back / close                 │
│    X            switch cluster                                     │
│    [ ]          detail tabs     / n   filter · namespace           │
│                                                                    │
│  Looking at things (always available)                              │
│    l  logs, with the log from before a crash                       │
│       in the log tab:  f follow · c container · ⇧p previous        │
│    d  describe — the object and what happened to it                │
│    y  view as YAML                                                 │
│                                                                    │
│  Changing things (each one asks first, and shows the command)      │
│    s       run more or fewer copies       (scale)                  │
│    r       restart, at its own pace       (rollout restart)        │
│    ctrl-d  delete — you type the name to confirm                   │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                              │
│ $ kubectl get daemonsets -A --watch                                │
├────────────────────────────────────────────────────────────────────┤
│ ? or esc to close                                          q quit  │
└────────────────────────────────────────────────────────────────────┘
```

**The command log strip is not covered, and it is not cleared.** Opening
`?` runs no command of its own, so the two-line block keeps showing whatever
was already there — here, the two permanent watches Alerts starts on top of
the primary pod watch, because this mockup opens help from the default Alerts
screen. The body above (16 lines, exactly [§1's own
budget](widgets.md#1-the-frame)), the log strip (4 rows — border, two lines,
border, [`LOG_LINES`](widgets.md#2-element--widget) never a mockup's
choice) and the footer (1 row) are the same three regions every other screen
draws in the same order; only the footer's own content and the body's own
frame — one bordered block titled `Keys`, the full body width, no sidebar —
differ from an ordinary screen ([widgets.md § 5](widgets.md#5-the-modal-layer)).
Header (1) + top border (1) + body (16) + log block (4) + footer (1) + bottom
border (1) is 24, the floor, not 23 — a fact about this screen's own fixed
content, since nothing here depends on cluster state the way a card list or a
sidebar count does.

Rules:

- **One key, one meaning, everywhere.** `/` always filters or searches the
  pane you are in, `n` is always namespace, `r` is always restart and never
  "retry". The two keys that collided — a severity filter and a manual
  reconnect — were deleted rather than rebound; the reasoning is in
  [NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted). This screen
  is the full map — `q` sits in the footer with the other keys that are valid
  right now, the same place every other screen puts it.
- Grouped by **what you are doing**, not by keycode order, and the jargon is
  in brackets — a newcomer reads the sentence, and learns the term for free.
- Only keys that exist in this build appear. Under `--read-only` the
  *Changing things* block is replaced by one line: *"read-only mode — nothing
  can be changed from here"*.
- v0.2+ operations join this screen as they land (cordon, drain, rollout undo,
  then exec and port-forward, then edit) — see
  [NOTES § Operations](../NOTES.md#operations--the-full-admin-surface).
