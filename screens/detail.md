# Screen — Object detail (the tabs)

`⏎` on anything opens it. Four tabs, `[` and `]` to move between them — the
whole debugging loop without a typed command.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  ‹ logs › describe   yaml   events            │
│   workloads        │  ───────                                      │
│   network          │  container: app ▾          previous log: on   │
│   storage          │                                               │
│   config           │  14:21:58  starting worker pool               │
│   cluster          │  14:22:01  connected to postgres              │
│ ANALYSIS           │  14:22:06  allocating 240MB cache             │
│   capacity      1 ▲│  14:22:07  --- killed here ---                │
│   certificates  30d│                                               │
│   drain safety     │  This is the log from before the last crash,  │
│   waste            │  which is usually the one you want.           │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app --previous             │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  ⇧p previous  / search  esc back   │
└────────────────────────────────────────────────────────────────────┘
```

| Tab | Shows | Notes |
|---|---|---|
| **logs** | follow, container picker, `--previous` | The most-typed kubectl command there is. `--previous` is one keypress because that is the log a crash loop needs. |
| **describe** | the object plus its events | Assembled from what we already hold; the event list is fetched for this object only, never a global Events watch. |
| **yaml** | the object as YAML | Key order is the API's, not alphabetised. Secret values are hidden behind an explicit reveal, and a revealed value never enters the command log, the audit log or this pane's copy buffer. |
| **events** | this object's events, newest first | Plain-language reasons: `Unhealthy` reads "the health check failed". |

Rules for this screen:

- Log streams are attacker-controlled text: bounded buffer, control characters
  stripped, no unbounded line.
- The finding that brought you here stays visible at the top — you never lose
  the reason you opened the object.
- **That block draws the finding's evidence in full**, and it is the only place
  that does. The Alerts card caps it at three wrapped lines with `…`, because a
  controller's verbatim message runs past any card
  ([alerts.md § the height](alerts.md#the-height)); this is where the rest of it
  is, and the cut is only honest because this screen exists. The block wraps to
  the pane and **scrolls with it** rather than being pinned — a nine-line quote
  pinned above a log pane leaves no log pane.
- On a grouped finding, `⏎` first lists *which* pods of the group are affected,
  then opens the one you pick. **The finding block is on that step too**, for
  the same reason: the full message must never be two keypresses away, or the
  card's `…` is pointing at nothing the reader can find.
