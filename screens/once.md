# Screen — `k8rs --once` (the output that ships first)

Not a TUI screen: `--once` reads the cluster, prints what is broken, and exits.
It is also **the first thing anyone ever sees from k8rs** — it ships as v0.0.1
at [milestone M1.5](../todo.md#phase-5--live-reads--milestone-m15),
months before the console exists ([NOTES § D10](../NOTES.md#d10--m1-ships-publicly-as-v001)).
Seven TUI screens were drawn and this one was not; that is the gap this file
closes ([NOTES § D17](../NOTES.md#d17--the---once-output)).

## What it prints

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

▲ node-3 · 2 hours ago
  This node refuses new pods (cordoned)
  2 pods here would still have to move
  → allow new pods once the work is done

1 critical, 2 warnings
```

It is the Alerts view with the frame taken off: **same findings, same plain
language, same grouping by owner, same order** (severity, then recency). One
`rules.rs`, one set of strings, two renderers — if `--once` and the Alerts
screen could ever disagree, one of them is lying
([alerts.md](alerts.md)).

That includes the ages, which is where the two nearly did disagree. `· 4 min
ago` is a suffix on the title line here rather than a right-aligned column, but
it is **the same string, from the same ladder**
([widgets.md § 1b](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen))
and it obeys the same rule: **it is present only when a field says when the
event happened.** The cordon finding usually has one now — the node lifecycle
controller stamps the taint it mirrors from `spec.unschedulable`, so a plain
`kubectl cordon` leaves a time behind — but not always: a taint applied by hand
(`kubectl taint`) stamps nothing, and on that node `node-3` gets a bare title
line in both renderers — drawn, both ways, in
[alerts.md § the cordon card](alerts.md#the-cordon-card-with-and-without-its-clock).
Without the frame there is no column for a blank to sit in, which makes the
absence invisible here — one more reason the pod count stays in the card body
where both renderers show it, whether or not the age is present beside it.

**There is no age column here, so none of the console's width budget applies —
and none of it needs to.** The age is a suffix on a line that is as long as it
needs to be; the console's 14-column maximum is a fact about a right-aligned
field competing with a name for one line, and it stops at the frame
([alerts.md](alerts.md#how-wide-a-card-is-and-how-tall)). What crosses over is
the ladder itself, and one consequence of it worth naming because a reader
comparing two reports will notice: **an event between 24 and 48 hours old
prints `30 hours ago`, not `1 day ago`.** The hours rung runs to 48, which is
where `kubectl` puts it too, so a report and the `kubectl` output beside it do
not describe the same moment two different ways.

## Why nothing in the report is cut or folded

**The report does not wrap.** A body line is as long as its content needs to
be — a `→` action line, an evidence string quoted straight from a controller,
an image name — and stdout carries it whole, at whatever width that turns out
to be. Indent is structure, not a width budget, and it is not a single
universal two: a card's own body sits at a flat two columns, evidence and the
`→` trailer alike, whatever follows it — but `--once --analysis`'s Capacity
pane nests further, four widths deep (measured live: `0` for the pane
marker, `2` for its own rows, `4` for a node listed under one, `6` for that
node's own `using …` continuation), because `--analysis` is a released flag
and its output is ordinary shipped output. Same rule at every depth: `--once`
writes a line-oriented stream and every one of its documented destinations —
a pipe into `grep`, a CI log, a paste into a ticket — reads it as one, and
already soft-wraps it at its own width on the way out; none of them can undo
a fold baked into the bytes ([NOTES § D201](../NOTES.md#d201--the-report-does-not-wrap-and-the-screen-loses-the-section-that-said-it-does-2026-08-31)).

**Nothing here is truncated, and that is the one place the two renderers
deliberately differ.** The console caps a card's evidence at three lines with
`…`, because a controller's verbatim message can run to 350 characters and the
Alerts pane has 16 rows to hold a *list*
([alerts.md § the height](alerts.md#the-height)). It can afford the cut because
`⏎` reaches the full text one keypress away. **`--once` has no `⏎`**: this
output is the whole of what the user gets, so the message prints in full and
the report is as long as it is.

**That is not the two renderers disagreeing, and the difference between the two
is worth being precise about.** They produce the same findings, in the same
order, with the same strings and the same ages — the rule this whole file is
built on. What differs is how much of one string is *on screen at once*: the
console shows the first three lines of it and marks the rest with `…`, because
it has somewhere to send the reader for the remainder. A renderer that cut text
it could not restore would be the lie; a renderer that cuts what one keypress
brings back is a pane doing its job.

## When nothing is broken

```
$ k8rs --once
84 pods · 3 nodes

○ nothing is broken
```

Three lines, and it has to be true — the same claim the empty Alerts screen
makes ([states.md](states.md)). A tool that prints "0 issues" while holding a
lint list would not survive the first person who checked.

## When a check could not run

```
$ k8rs --once --namespace payments
ns: payments · 12 pods · 3 nodes

○ nothing is broken in payments

One node check is off: spotting a node someone started emptying and
did not finish needs every pod in the cluster.
```

The console says this in a banner above the list
([states.md](states.md#you-can-only-see-some-namespaces)); here there is no
banner, so it goes **last** — after `1 critical, 2 warnings` when there are
findings, after `○ nothing is broken` when there are none. That is the place a
reader is already looking to decide whether the report is complete, and the
last thing left on screen when the output is longer than the terminal. It
prints in both cases: a report with findings is no more complete than an empty
one when the same check was switched off.

- **On stdout, with the findings.** Every other line k8rs writes about itself
  goes to stderr, but this one is part of the answer: `k8rs --once >
  findings.txt` that drops it produces a file claiming a clean cluster with no
  note that a check was switched off, which is the failure the line exists to
  prevent.
- **It is the same sentence the console draws**, printed unfolded rather than
  fit inside a pane. One string, two renderers — the rule this whole file is
  built on.
- **`--namespace` and a 403 fallback print it identically**, because the scope
  is identical; the header line names which namespace, and that is the only
  place the cause shows.
- The header line gains `ns: payments` for the same reason the TUI header does:
  a report that does not say what it covered cannot be trusted after it is
  pasted into a ticket.

## When your clock and the cluster's disagree

The TUI answers this with a header pointer plus a banner, because it has a
header row to put a pointer in
([states.md § Your computer's clock is off](states.md#your-computers-clock-is-off)
— read that section first; D55 and D69 are cited there, not restated here).
`--once` has no header row, only the one-liner, so the question this file has
to answer is where the sentence goes instead — and the answer is the same
place [the check-could-not-run line](#when-a-check-could-not-run) already
found for the same problem: **last**, after the findings, on **stdout**. Both
are "how much of this report can you trust", both are read at the moment the
reader is deciding whether to look away, and inventing a second slot for the
second one would be inventing a second rule this file does not need.

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 1 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning

This computer and the cluster disagree about the time by 11 minutes
(this one is behind), so recent times are missing and older ones can
read smaller than they really are.
```

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning

This computer and the cluster disagree about the time by 9 minutes
(this one is ahead), so times can read larger than they really are.
```

The two reports show the same two findings at two different clocks. In the
first, `payments/web` — the younger of the two — loses its age exactly as
before: "it is present only when a field says when the event happened,"
stated above ([§ What it prints](#what-it-prints)) and still true, because a
blanked event is one `age` refused to guess at. `shop/api` is old enough that
it does not blank; it prints `1 min ago` instead of its true `12 min ago`
([§ What it prints](#what-it-prints)) — present, not flagged, and wrong by
most of the gap
([D177](../NOTES.md#d177--the-behind-half-does-not-only-blank-it-also-under-reports-and-a-refusals-date-is-not-the-clusters-clock-2026-08-28)).
In the second report neither card blanks; both read a plausible amount
older than they are, and nothing on either line says so.

**No `⚠`.** This file's own vocabulary is `● ▲ ○` and nothing else
([§ Colour and symbols](#colour-and-symbols)); the console's pointer borrows a
glyph from its `⚠ disconnected` / `⚠ login expired` family, but this stream
has never used that family, and starting here would be a fourth symbol a
reader of `--once` output has no legend for.

**Both sentences are exactly the strings the console draws**, printed
unfolded and flush left instead of fit inside the pane — the same treatment
[the check-could-not-run sentence](#when-a-check-could-not-run) already gets.
One string, two renderers, the rule this whole file is built on, unchanged by
this box.

**It does not touch the exit code.** `0` still means "k8rs ran and reported,"
the same as an incomplete-check notice does today
([§ Exit codes](#exit-codes)); a clock being off is a fact about the data,
not a failure to run.

### Stacked with a check that could not run

The two lines are independent — a `--namespace` run can be both scoped and
skewed — and when both apply they stack in the same order the TUI banner
does: the clock line first, because it says something about *every* fact on
the page, before the completeness line says something about *which* facts
are on it at all.

```
$ k8rs --once --namespace payments
ns: payments · 12 pods · 3 nodes

○ nothing is broken in payments

This computer and the cluster disagree about the time by 11 minutes
(this one is behind), so recent times are missing and older ones can
read smaller than they really are.

One node check is off: spotting a node someone started emptying and
did not finish needs every pod in the cluster.
```

### The two cases that print nothing

Same as the console
([states.md § When there is nothing to say](states.md#when-there-is-nothing-to-say)):
a `Date` header k8rs could not read, and a skew under five minutes, both leave
this section silent rather than guess. A `--once` run piped to a file on
either of those days looks exactly like one from a cluster with a perfectly
set clock, which is the correct answer when there is no evidence either way.

## When the API server's own certificate is running out

This is C2
([NOTES § Certificate rules](../NOTES.md#certificate-rules-c-series--and-what-is-not-reachable)):
the certificate the API server itself presents on every connection, read once
at connect the same way
[`Session::skew`](#when-your-clock-and-the-clusters-disagree) is. Like
`skew`, it is a session-level fact rather than a `Finding` — it names no
cluster object, so it carries no severity band and earns no place in the
tally by design, the same way `skew` never has
([NOTES § D178](../NOTES.md#d178--c3-lands-whole-c2s-row-cannot-be-drawn-in-a-frozen-pane-and-the-twelfth-crate-was-already-compiled-2026-08-28)).
A Certificates-pane row is a separate question — `analysis::certificates`
would have to grow a third source beside `c1_row` and
`kubelets_waiting_to_join`, and `analysis.rs` froze at Phase 4 close, one
phase before this box could reach it; that gap is
[screens/analysis.md](analysis.md)'s to record, not this file's. The TUI's
own answer to this fact — a header pointer plus a banner — is Phase 9's, the
same ruling D176 already made for the clock line; not designed here.

Because it is the same shape as the clock reading, it prints in the same
place: **last, in the trailer, after the cards.** Not because it is
low-urgency — a cluster days from refusing every connection is closer to
catastrophic than most things that do earn a card — but because this file
groups by *source*, not by severity: everything read off `Session` rather
than off a cluster object prints below every card that has one, in the order
it was added. That is [D176](../NOTES.md#d176--the-clock-skew-line-does-not-fit-in-the-header-and-the-two-halves-do-not-share-a-sentence-2026-08-28)'s
**append, do not reorder** call applied for a second reason — there it
decided which fact drops first if the header runs out of room, here it
decides where a new trailer fact joins one that already prints correctly.

**Below thirty days, nothing prints, and that already matches the rule
beside it.** `expires_at` answers `None` for a certificate more than
`CERT_EXPIRY_WARN` from running out, so C1 draws no card for a healthy
kubeconfig certificate — a 210-day reading here would be exactly that noise,
on every single run, for as long as the cluster is healthy. A working
control plane renews its own serving certificate on its own schedule; a
report that mentioned it anyway would be telling the reader to check
something that needs no checking.

**One threshold, thirty days, shared with the kubeconfig certificate.**
`CERT_EXPIRY_WARN` is reused rather than a second, unbacked number invented
beside it. The reasoning that constant already carries — "long enough to ask
a human who is on holiday, short enough not to sit on the screen for a
quarter" (`rules.rs` § the certificate rules) — is a notice period for
getting a credential renewed by a person who has to be found and asked, and a
control-plane certificate renewal is exactly that kind of operation. Two
certificates on one report, warning at two different distances with nothing
here to justify why one gets more runway than the other, is the thing
invariant 14 calls noise before it calls information.

**The sentence carries its own disambiguation, for the same reason:
`— not your kubeconfig's —`.** C1 and C2 can print on the same run — a
kubeconfig certificate and the server's own certificate running out inside
the same month is not a rare coincidence, since one team's renewal habit
often misses both — and a reader who has just read *their own* certificate's
expiry elsewhere on the page needs one clause telling them this is a second,
different certificate, not the same fact printed twice. Where that reading
sits depends on which of C1's two bands fired: **expired** is a card, same
page as this sentence; **expiring** is a Certificates-pane row, printed in
the same run under `--analysis`
([NOTES § D87](../NOTES.md#d87--c1-has-two-bands-and-they-belong-on-two-screens-d2-only-ever-ruled-on-one-of-them-2026-08-14) ·
[NOTES § D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)).
Either way, the clause stays in every drawing of this sentence,
unconditionally.

**The sentence describes what this run saw, not what the cluster has.**
"An API server" usually means one process, but a control plane can run
several behind one address — a load balancer in front of two or three
`kube-apiserver` replicas, each free to carry its own certificate on its
own renewal schedule. The check has no way to ask the load balancer what
else is behind it, or even how many replicas there are; it can only read
whatever answers. Measured against a three-replica kind cluster where one
replica's certificate had been reissued to twelve days and the other two
were healthy for a year: the same command, run eight times back to back
with nothing about the cluster changing between runs, printed this
sentence on three of the eight runs and nothing on the other five — the
load balancer routed differently each time, and only one of the three
replicas was the one worth warning about
([reports/2026-08-28-c2-c3-against-a-real-api-server.md § 2](../reports/2026-08-28-c2-c3-against-a-real-api-server.md#2-ha-control-plane-eight-consecutive-runs-of-one-command)).
**"The API server's own certificate" is the wrong noun phrase for that
result** — a definite, singular claim about a cluster that may have
several, drawn from a sample of one connection. "A certificate the API
server presented" claims exactly what was read: true on a single kind
node and true on a three-way HA control plane alike, with no
cluster-topology caveat for the common single-instance case to read past
— k8rs cannot reliably tell the two apart in the first place, since a
managed cluster's control-plane replicas are not Node objects a kubeconfig
can see. Taking more than one sample a run and keeping the soonest
deadline seen (a Phase 5 change, cheap at tens of milliseconds a
connection) narrows the miss window this measurement exposed; it does not
close it, and it does not turn "a certificate" into "the cluster's
earliest" — the sentence stays a report of what this run's samples saw,
worded the same whether there was one replica behind the address or
several.

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning

A certificate the API server presented — not your kubeconfig's —
expires in 12 days (valid until 2026-09-09T00:00:00Z). Once it runs
out, kubectl and everything else stop being able to reach this
cluster until someone on the control plane renews it — not something
k8rs can do.
```

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning

A certificate the API server presented — not your kubeconfig's —
expired 3 days ago (was valid until 2026-08-25T00:00:00Z). When that
happens, kubectl and everything else stop being able to reach a
cluster until someone on the control plane renews its certificate —
not something k8rs can do.
```

**Two different situations reach this sentence, and only one needs
anything turned off.** The first is unchanged: `insecure-skip-tls-verify:
true` in the reader's own kubeconfig is the only way *this* connection
completes a handshake with a certificate already past its `notAfter` — a
verifying kubeconfig gets
[§ When the certificate is why nothing came back](#when-the-certificate-is-why-nothing-came-back)
instead, on a single connection, measured on a real API server whose
certificate had expired three days earlier
([reports/2026-08-28-c2-c3-against-a-real-api-server.md § 3](../reports/2026-08-28-c2-c3-against-a-real-api-server.md#3-twelve-day-and-expired-serving-certificates-single-node)).

**The second needs nothing turned off: an HA control plane.** Earlier in
this section, eight consecutive runs of one command against a three-replica
control plane already measured this probe's five samples landing on
different replicas behind one load balancer
([reports/2026-08-28-c2-c3-against-a-real-api-server.md § 2](../reports/2026-08-28-c2-c3-against-a-real-api-server.md#2-ha-control-plane-eight-consecutive-runs-of-one-command)).
A fully verifying kubeconfig refuses the sample
that draws an already-expired replica and reads its date doing so — while
the report's own connection needs only one working replica, which the
balancer is just as likely to hand it. So this same sentence can print for
a reader whose kubeconfig checks everything, on a control plane that is
otherwise serving traffic fine. That combination is drawn below, and the
tense stays timeless for the same reason as every other reading here —
*"valid until … on a red card reads as though it still is"* (`rules.rs`
§ the certificate rules) — a claim about clients that check, not about
whichever connection this run happened to make.

```
$ k8rs --once
84 pods · 3 nodes

○ nothing is broken

A certificate the API server presented — not your kubeconfig's —
expired 3 days ago (was valid until 2026-08-25T00:00:00Z). When that
happens, kubectl and everything else stop being able to reach a
cluster until someone on the control plane renews its certificate —
not something k8rs can do.
```

**A clean tally does not mean every replica is current.** The other
control-plane replicas are still carrying this cluster's traffic while
this one waits on a renewal nobody has done yet — worth saying before a
second replica also runs out and the balancer has nowhere healthy left
to route around it.

**The same discipline picks the article, not only the verb.** This sentence
says kubectl and everything else "stop being able to reach *a* cluster" —
not *this* cluster — because naming this cluster beside a claim that it
cannot be reached would contradict the report the reader is holding: it
reached them, so this cluster was plainly reachable a moment ago. The
expiring-soon sentence, drawn three times elsewhere on this page, makes no
such claim — nothing has failed yet — so it names the cluster the reader
is looking at directly: "reach *this* cluster."

```
$ k8rs --once
84 pods · 3 nodes

○ nothing is broken

A certificate the API server presented — not your kubeconfig's —
expires in 12 days (valid until 2026-09-09T00:00:00Z). Once it runs
out, kubectl and everything else stop being able to reach this
cluster until someone on the control plane renews it — not something
k8rs can do.
```

**A clean cluster is the case this matters most for.** `○ nothing is broken`
reads as permission to look away, and a certificate days from taking the
whole cluster down is exactly the fact that permission would hide — the same
reason the clock sentence prints after `○ nothing is broken` too, not only
after a tally ([§ What it prints](#what-it-prints) established the parallel;
this is the second fact to use it).

**No reading at all is still one silence for two causes, and a third pulls
itself out.** A server address that cannot be parsed into something to
connect to a second time, and a certificate whose `notAfter` will not
parse, are two different failures with the same content: k8rs does not
know when this certificate runs out, and neither is common or actionable
enough to earn its own sentence. C1 already answers exactly this question
the same way — `expires_at` returns `None` alike for a truncated
certificate, a wrong PEM label and an RFC 5280 §4.1.2.5 "no well-defined
expiry", and draws no card for any of them (`rules.rs` § the certificate
rules). Collapsing those two into one silence is not a new policy; it is
the sibling rule's own policy read across, and it is a closer precedent
than [`Session::skew`](#when-your-clock-and-the-clusters-disagree)'s four,
not just an available one.

**A handshake that fails is no longer automatically a third instance of
that silence.** rustls types *why* a handshake failed, and when the reason
it gives is that the server's certificate has already expired, k8rs knows
something worth a sentence that it does not know for the other two causes
above: the date this certificate ran out, not only the fact that it did.
That one case is drawn on its own —
[§ When the certificate is why nothing came back](#when-the-certificate-is-why-nothing-came-back),
below. Every other reason a handshake can fail — the wrong CA, a reset
connection, a timeout — still folds into this same silence, because k8rs
knows no more about those than it does about an address it cannot reuse.

The analogy to `skew` is not exact, and the difference is worth stating
rather than papering over: silence about the clock is close to the truth on
its own — `--once` runs for a few seconds, so an unmeasured skew is very
likely a small one — while silence about a certificate is not, because a
healthy 210-day reading and a failed read look identical on screen and a
failed read could just as easily be hiding three days. What limits the
damage is that a second connection failing to the same host and port a
first connection just succeeded against needs an unusual cause — a server
address this probe cannot reuse, a network fault in the instant between the
two calls — rather than a common one a reader should expect to hit often.
The alternative, a fourth on-screen state that says *"unknown"* with no
number attached, would be a claim with nothing behind it, which is the exact
failure D176 already ruled out for the clock line. Printing nothing stays
the honest answer, and it needs no example: it is the absence of one, same
as [the two cases that print nothing](#the-two-cases-that-print-nothing)
above it.

**`--once` has no live/disconnected state to suppress this for.**
`Session::skew`'s doc reserves that suppression for a renderer with a stale
"last successful" reading to protect against — the TUI, which stays open
between connections. `--once` connects once and either has this reading or
does not by the time it prints anything at all, so there is no second state
here that needs one.

### When the certificate is why nothing came back

**This is the state that matters most, and it is not a trailer line.**
Every reading drawn above prints inside a report that already succeeded —
the report is the proof the connection worked. This one means the
connection did not: on a verifying kubeconfig, a server certificate that
has already expired fails the handshake the same way for the dedicated
probe as it does for every other call `--once` needs — reading
`/version`, discovering what the cluster serves, listing pods. Measured
today, before this box, on a real API server three days past its own
`notAfter`, with a verifying kubeconfig and the temporary driver that
stood in for `--once` before Phase 5 built it: `grep -c "API server's own
certificate"` over the run is `0`, and instead of a diagnosis the run
prints the same generic line once for every call that could not
complete — *"could not read the server version (nothing usable came back
when k8rs tried to `get /version`)"*, worded identically for `/apis` and
for the pods watch, with nothing tying the three together
([reports/2026-08-28-c2-c3-against-a-real-api-server.md § 3](../reports/2026-08-28-c2-c3-against-a-real-api-server.md#3-twelve-day-and-expired-serving-certificates-single-node)).
That wall is not `--once`'s own shape — `--once` collapses a total
failure to reach the cluster to one sentence already, never a list of
symptoms (below) — it is evidence of the gap this box closes: every one
of those three generic lines is `k8s::Fault::Unanswered`, "one variant on
purpose, because from the reader's side they are one fact"
([docs/architecture.md § Error handling](../docs/architecture.md#error-handling)),
and today that variant is genuinely all k8rs knows, on every one of the
three calls. It no longer has to be. rustls already knows the one fact
that ties them together; this state is k8rs saying it.

**So this is not three generic messages with one true one added above
them; it is the true one in place of all three.** `--once` already answers
every other startup failure it can name — no kubeconfig, no route to the
server, no permission to list pods — with one specific sentence and a
non-zero exit, never a list of every symptom. The words for this one live
once, in [states.md § Before the TUI ever
starts](states.md#before-the-tui-ever-starts), not redrawn a second time
here — exit code `2`, like the other three ([§ Exit codes](#exit-codes)).
A certificate the API server presented, typed as expired by rustls
itself, is exactly that kind of known cause, and it earns the same
treatment: one sentence, not a wall — and now one that names the date
rustls already knew, not only the fact.

**There is a date here too, and it is not invented.** rustls's own
refusal names it: the typed error is not the plain `CertificateError::
Expired` this section long assumed but `ExpiredContext { not_after, .. }`
— the certificate's real expiry, carried inside the very handshake that
refused to hand back anything else. A verifying kubeconfig still reads
nothing about this certificate for any other purpose, but it does not
need to for this one date: the failure already told it. So the startup
message states it in the same shape as the trailer line above it — a
relative age first, the exact timestamp beside it — worded once, in
[states.md § Before the TUI ever
starts](states.md#before-the-tui-ever-starts), and not redrawn here.
D176's rule against a placeholder still holds; what changed is that this
number is read off the failure, not invented to fill the space where one
used to be missing.

## When your own login is running out

This is C1's **expiring** band
([NOTES § Certificate rules](../NOTES.md#certificate-rules-c-series--and-what-is-not-reachable)):
the kubeconfig's own client certificate, read from the file on disk rather
than from anything the cluster answers — the login this machine uses,
not the login the cluster is running. `rules.rs` already carries this
finding and it is already drawn, one screen over,
[in the Certificates pane](analysis.md#certificates-and-versions) — the gap
this box closes is that a bare `k8rs --once` or `k8rs --live`, with no
`--analysis`, currently shows it nowhere at all. `Severity::Info` is exactly
what keeps it off a card ([§ What `--once` does not do](#what---once-does-not-do)
already names it as one of three rules with no other reader before this
box), and D87's ruling that the card block never draws that band is
unchanged — this is a trailer sentence, C2's shape, not a fourth card.

**Same threshold as C2, because it is the same rule's own number.**
`CERT_EXPIRY_WARN` is C1's threshold before it is anything C2 borrows —
thirty days, "long enough to ask a human who is on holiday, short enough
not to sit on the screen for a quarter" (`rules.rs` § the certificate
rules). Below it, nothing prints, for the same reason a healthy serving
certificate prints nothing: a 210-day reading on every run is noise before
it is information.

**The facts are C1's own, restated as one paragraph rather than copied
field by field.** The Certificates pane draws this finding's `title`,
`evidence` and `action` verbatim, three lines a row can hold; a trailer
sentence has no lines to keep separate, so this joins the same facts —
the date, what the file is, and what to do about it — the way C2's own
trailer joins its facts, rather than stitching the three clauses at their
original seams. Inventing a fourth, unrelated wording for the same fact
would be the two-sentences-that-can-disagree failure this file exists to
prevent; restating the same facts in one shape that already fits a
trailer is not that.

```
$ k8rs --once
84 pods · 3 nodes

● payments/web · 3 of 5 pods · 4 min ago
  Containers exceeded their memory limit and were killed by the kernel
  (OOMKilled)
  limit 256Mi · exit 137 · 47 restarts
  → raise limits.memory, or find the leak

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning

Your kubeconfig certificate — the file on your own machine that
proves who you are, not anything in the cluster — expires in 12
days (valid until 2026-09-09T00:00:00Z). Once it runs out the
cluster stops accepting it, so kubectl stops working for you too —
ask whoever gave you access for a new kubeconfig before that date,
because k8rs cannot renew it.
```

**No mirror of C2's `— not your kubeconfig's —` clause, and that is a
ruling and not an omission.** C2 needs that clause because *a certificate is
expiring* defaults to being read as *my own credential* — the familiar
case — so C2 has to actively deny it. This sentence opens with `Your
kubeconfig certificate`, which already commits to the one referent a
reader could have; there is no other certificate this sentence could be
mistaken for, and a clause excluding a reading nobody would reach is
noise invariant 14 rules out before it rules the sentence in. The two
sentences are already distinguishable at a glance by their opening words
alone — one names the reader's own file, the other explicitly is not it.

**The expired band is not a new sentence — it already reaches the reader,
louder, as a card.** `Severity::Critical` past the deadline is the one
band D87 sent to Alerts, so it is already in the findings list on any run
that reaches `analyze()`, drawn exactly like every other card:

```
$ k8rs --once
84 pods · 3 nodes

● prod-eu
  Your kubeconfig certificate expired 3 days ago — the cluster is
  refusing you
  was valid until 2026-08-25T00:00:00Z · this is the file on your
  own machine that proves who you are — nothing in the cluster is
  broken
  → ask whoever gave you access for a new kubeconfig — k8rs cannot
    renew it, and kubectl has stopped working for you too

▲ shop/api · 2 of 6 pods · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
  → check the app's /healthz endpoint

1 critical, 1 warning
```

No age on the title line — `timestamp` is `None` on both of C1's bands,
deliberately: an expiry is a deadline, not an event that happened at a
moment, and the past-dated half does not get one either
(`rules.rs` § the certificate rules). The bare `prod-eu` is the context
name, undecorated, because a kubeconfig is not a namespaced object and rule
5 draws no slash for the things that are not.

**And this reader may well not have got a report at all — the asymmetry
with C2 worth stating plainly.** C2's every reading, expiring or expired,
prints inside a report that already exists: *"the report is the proof the
connection worked"* ([§ When the API server's own certificate is running
out](#when-the-api-servers-own-certificate-is-running-out)). C1's expired
band carries no such guarantee, because the certificate it reads is
routinely the *same* one the connection just authenticated with — an
expired client certificate very often fails the TLS handshake outright,
which is [states.md](states.md#before-the-tui-ever-starts)'s ordinary
connection-refused wall, exit code `2`, no findings, no card, `analyze()`
never called. The card above is real and already shipped, but it is
reachable specifically when this same expired certificate is *not* what
the connection actually used — a token or an `exec` plugin carries the
real login while the old certificate sits in the file, unused and stale.
No new message is built for the more common case: unlike C2, no rustls
error names *this client's own certificate* as the reason a handshake
failed, so there is nothing today to word a dedicated sentence from, only
the generic wall every other unreachable-cluster cause already gets.

**Under `--analysis`, the pane wins and the trailer stays silent — one
fact, one place, not two.** The Certificates pane already draws this exact
finding as a row, sorted among every other certificate this cluster and
this kubeconfig carry
([analysis.md § Certificates and Versions](analysis.md#certificates-and-versions)).
Printing the trailer as well would be the same fact twice in two shapes on
one page — the thing this box exists to stop happening to C2, not
something to reintroduce for C1. So: no `--analysis`, trailer prints;
`--analysis`, the pane row prints and the trailer does not. The expired
band is unaffected either way — it is a card, not a trailer, and cards
print regardless of `--analysis`.

**A clean tally does not mean the reader's own access is fine.** The same
argument C2 makes for a clean cluster applies here without changing a
word: `○ nothing is broken` reads as permission to look away, and a login
that stops working in six days is exactly the fact that permission would
hide.

**No card, no severity band, no tally entry, no `⚠`, and no change to the
exit code — all four for the reason C2's own trailer already states**
([§ When the API server's own certificate is running
out](#when-the-api-servers-own-certificate-is-running-out)): a session
fact printed in the same slot is not a fifth vocabulary, it is the same
one used again.

## Stacked with the other trailer lines

The order is **clock, then C2, then C1, then the check-that-could-not-run
line**, and each join has its own reason rather than one blanket rule.
Clock goes first because it says something about *every* line above it,
cards included — a reader has to know whether to trust the ages before
anything else on the page. The check-that-could-not-run line stays
absolute last on purpose ([§ When a check could not
run](#when-a-check-could-not-run): "the last thing a reader is already
looking at to decide whether the report is complete"), and neither box
that has touched this order has reopened that. So the open slot was always
between clock and the last line, and that is where each new fact has
joined — the same append, do not reorder call, applied twice now to print
order rather than drop order. C1 takes the same treatment: it is the
newer fact, so it takes the next open slot — right after C2 — rather than
displacing a line that already prints correctly. Ordering the two by
urgency instead of arrival would mean weighing a certificate the cluster
answers against one the reader's own laptop holds, which no rule on this
page has ever had to do for two cards, let alone two trailer lines, and
this box does not start now. This
ordering is for the readings that print *inside* a report — the
expired-and-typed reading for C2, and the connection-refused wall C1's
expired band routinely produces instead of a report, do not join it,
because when either fires there is no report for it to join.

```
$ k8rs --once --namespace payments
ns: payments · 12 pods · 3 nodes

○ nothing is broken in payments

This computer and the cluster disagree about the time by 11 minutes
(this one is behind), so recent times are missing and older ones can
read smaller than they really are.

A certificate the API server presented — not your kubeconfig's —
expires in 12 days (valid until 2026-09-09T00:00:00Z). Once it runs
out, kubectl and everything else stop being able to reach this
cluster until someone on the control plane renews it — not something
k8rs can do.

Your kubeconfig certificate — the file on your own machine that
proves who you are, not anything in the cluster — expires in 6
days (valid until 2026-09-03T00:00:00Z). Once it runs out the
cluster stops accepting it, so kubectl stops working for you too —
ask whoever gave you access for a new kubeconfig before that date,
because k8rs cannot renew it.

One node check is off: spotting a node someone started emptying and
did not finish needs every pod in the cluster.
```

**No `⚠`, for the same reason as the clock line.** `● ▲ ○` is this file's
whole vocabulary ([§ Colour and symbols](#colour-and-symbols)); the
console's pointer family has never been drawn here and this is not where it
starts.

**It does not touch the exit code.** `0` still means "k8rs ran and
reported" — a certificate running out is a fact about the cluster, not a
failure of this run to read it, the same distinction the clock line already
draws ([§ Exit codes](#exit-codes)).

## stdout and stderr are split on purpose

**stdout is the findings. stderr is everything else** — the commands k8rs
ran, and any error.

```
$ k8rs --once 2>/dev/null        # just the report
$ k8rs --once > findings.txt     # the commands still print to the terminal
```

**The command log is the teaching device and it does not stop being one
outside the TUI** ([invariant 4](../CLAUDE.md)) — but a report that is
piped somewhere should arrive without it. Splitting the streams gives
both for free, with no flag.

**The command log already has a shape, and it comes from `--describe` and
`--yaml`, not from this file.** One `$ kubectl …` line per read, on stderr,
sanitized like every other free-text field, printed once the read is known
to have happened rather than promised in advance
([screens/detail.md](detail.md), `main.rs`'s `kubectl_get` for `--yaml`
and `describe_run`'s own line for `--describe`, printed after the pod is
read and before events are asked for). The live path — a bare `k8rs
--once` or `k8rs --live`, with no other flag — joins that same convention
instead of starting a second one, and this is the first place it is
written down rather than only drawn as two lines nobody emits:

```
$ kubectl get --raw '/api/v1/pods?limit=1'
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get pods -A --watch
$ kubectl get nodes --watch
$ kubectl get deployments -A --watch
$ kubectl get statefulsets -A --watch
$ kubectl get daemonsets -A --watch
```

That is every read a bare run performs, in the order k8rs starts them:
first, whether this login may list pods at all, then the server's own
version, then what kinds it serves, then the five permanent watches,
declared in this order — Pods, Nodes, Deployments, StatefulSets,
DaemonSets (invariant 6). **The watches are started together, not one
after another, and their real order on the wire is neither this list nor
the declaration order it happens to match** — measured directly, it
reshuffles from one run to the next. What is fixed is the *group*: the
probe before the version reads, both before discovery, discovery before
the five — and the five print in one stable order because a log a reader
searches needs one, not because the wire gives it one. A reader can still
paste any one line alone and get what k8rs got from that call; nothing
here promises which of the five a second terminal would see arrive first.

- **The first line is a probe, not a report input, and it earns a line
  anyway.** `k8s::coverage` asks *may this login list pods at all* before
  anything else runs on this path — one `LIST` capped at a single object
  (`k8s.rs`'s own `lists_pods`, `ListParams::default().limit(1)`) — and
  what its answer decides is scope, not a fact any card is built from.
  That is a real reason to leave a line off, and it is exactly the reason
  the TLS handshake below has none. But this read, unlike that one, is an
  ordinary HTTP request with an ordinary `kubectl` spelling, and the bar
  this box sets is *every read k8rs performs*, not *every read the report
  is built from* — `/version` and discovery below are already printed
  under that wider bar despite deciding a greeting and a kind list rather
  than a card. Excluding this one because its answer is scope rather than
  content would be a rule this file does not hold its own next two lines
  to. It prints.
- **Only on a run that did not type `--namespace` or its short form
  `-n`.** Either spelling already answers the question this probe exists
  to ask, so nothing is sent and nothing is printed — the scoped example
  below shows the line simply absent, not refused.
- **A second one, scoped to `default`, only when the first is refused and
  the kubeconfig's own context names no *usable* namespace to fall back
  to instead** (`k8s.rs`'s `coverage`, `FALLBACK_NAMESPACE`). "Usable" is
  doing real work in that sentence: a context that names something which
  is not a valid namespace name is the same case as one that names
  nothing at all — `context_scope` filters it out through
  `k8s::namespace_name` before `coverage` ever asks — so both send the
  second probe and both print its line. Only a context that names an
  actual namespace answers the fallback question for free and sends
  nothing further; every other case sends the second probe and prints
  its line, because there is nothing usable in the file to answer the
  question with instead:

  ```
  $ kubectl get --raw '/api/v1/pods?limit=1'
  $ kubectl get --raw '/api/v1/namespaces/default/pods?limit=1'
  ```
- **`/version` prints once, not twice.** The live path reads this path
  twice over the wire — the version string, then the same path again for
  the `Date` header the clock line is built from (`k8s.rs` § CONNECTING)
  — but it is the identical request both times, and a reader who pastes
  the line once has already reproduced both reads. A second, identical
  line under the first would read as a stutter, not a second fact — the
  kind of line this box exists to remove, not add.
- **Discovery prints as `kubectl api-resources --verbs=list`, not as the
  two raw paths it actually sends** (`/apis` and `/api`, aggregated,
  "priced at two" — `k8s.rs` § CONNECTING). This is the same shape
  `kubectl describe pod` already has in this file: one command a reader
  would actually type, standing in for more than one call underneath it.
  `kubectl get --raw /apis` alone would not answer the same question a
  reader has — it leaves out what `/api`'s own versions serve — so the
  command that answers it is the honest line, not the literal path.
  **`--verbs=list` changes nothing the wire sends and everything the
  command prints — measured, still just `/apis` and `/api`, nothing
  more.** Plain `kubectl api-resources` lists every kind the cluster
  registers, including ones with no `list` verb at all; `k8s::browsable`
  keeps only the kinds this browser could ever open a list of
  (`supports_operation(verbs::LIST)`, invariant 12's own boundary — the
  exact test the flag names). Printing the flag from the start is why
  this line and the greeting's own kind count agree without a reader
  being told why they don't; the bare command would answer a question
  k8rs did not ask.
- **The five watches print as `--watch`, because that is the closest one
  command gets — the same objects back, not the same resilience.** k8rs's
  own watch quietly re-lists and keeps going when a watch breaks
  (`StandingBackoff`, `k8s.rs`); a `kubectl get --watch` that hits the
  same break stops, and getting back to where k8rs is means running it
  again by hand. That gap is about what happens when the connection
  drops, not about what a reader gets back while it holds — pasted
  against a healthy connection, the command returns the same objects k8rs
  is showing, which is the bar this box sets.
- **`nodes` carries no `-A`.** Nodes are cluster-scoped — there is no
  namespaced node list to ask for (`k8s.rs`'s own note on the five
  watches) — so the honest command has no namespace flag on any run,
  scoped or not.
- **One line per kind, not one merged
  `deployments,statefulsets,daemonsets --watch`.** `kubectl` accepts a
  comma-separated list and would print a single plausible-looking line,
  but k8rs opens three separate watches, not one, and a reader troubleshooting
  which kind is misbehaving needs a line they can run alone — one they
  could not isolate from a merged command.

**Under `--namespace payments`, four of the five follow the scope and one
does not — the same split the watches themselves make:**

```
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get pods -n payments --watch
$ kubectl get nodes --watch
$ kubectl get deployments -n payments --watch
$ kubectl get statefulsets -n payments --watch
$ kubectl get daemonsets -n payments --watch
```

**Two reads on this path print no line at all, for two different
reasons, and neither is a gap the convention forgot.** The TLS handshake
C2 reads its certificate off sends no request — it opens a connection,
reads what the server presents, and closes (`k8s.rs` § CONNECTING;
[§ When the API server's own certificate is running
out](#when-the-api-servers-own-certificate-is-running-out) is what the
read is *for*) — and there is no `kubectl` spelling of *look at a
certificate and hang up* to print. The second `/version` read, above, has
a line: it is the first one, printed once rather than twice. A line that
repeated itself and a command for a thing no command does would both be
worse than no line — the lie this box exists to remove.

### Under `--analysis`

**Seven more reads, all started together on one deadline, printed after
discovery and before the five watches.** The group order is real — probe,
then version and discovery, then these seven as a block, then the five
watches as a block — and there is no better fixed point to hang a
guarantee on than that. **The order *within* the seven is not real, and
this file does not claim it is.** They are one `tokio::join!`
(§ WHAT A REPORT ASKS FOR), started in the same instant; which one's
answer lands first is not fixed by the argument order in `k8s.rs`'s own
source and is not stable from one run to the next — measured directly,
against one cluster, twice in a row. The seven print in one fixed order
below for the same reason the five watches do, above: a log a reader
searches needs a stable shape, and the order `k8s.rs`'s source declares
them in is as good a choice as any other true one.

```
$ kubectl get --raw '/api/v1/pods?limit=1'
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get certificatesigningrequests
$ kubectl get replicasets -A
$ kubectl get services -A
$ kubectl get endpointslices -A
$ kubectl get persistentvolumeclaims -A
$ kubectl get poddisruptionbudgets -A
$ kubectl top nodes
$ kubectl get pods -A --watch
$ kubectl get nodes --watch
$ kubectl get deployments -A --watch
$ kubectl get statefulsets -A --watch
$ kubectl get daemonsets -A --watch
```

- **Every one of the six lists is a single, unpaged request, and the
  honest command is still the plain one.** `kubectl get replicasets -A`
  pages in 500-row chunks by default; `k8s.rs`'s own read asks for the
  whole list in one request instead (§ WHAT A REPORT ASKS FOR's own
  `whole_list`, and the paragraph there that measured `kubectl`'s own
  default chunk size to make exactly this comparison). Chunking changes
  how many round trips fetch the list, not which objects come back in
  it — a reader who pastes the plain command gets the same list, which is
  the bar this file sets, not the same request count.
- **`certificatesigningrequests` carries no `-A`, and stays bare even
  under `--namespace`.** `list csr` is cluster-scoped on every cluster —
  there is no namespaced form to ask for (§ WHAT A REPORT ASKS FOR). The
  other five *do* follow `--namespace`, the same way the watches do, so a
  scoped run prints `-n payments` on those five and on none of the other
  three lines above them.
- **`kubectl top nodes`, not a raw path into `metrics.k8s.io`.** It is the
  command a reader already knows for this exact question, and it is the
  one command in this list that is not a `kubectl get`. Under `k8rs
  --live --analysis`, the same reading is polled every thirty seconds
  (`k8s::node_usage_poll`, invariant 6's one timed exception) and the
  line prints once, when the poll starts, the same as a watch's line
  marks the read starting rather than every object it delivers — a
  command log line means *this read began*, not *this stream is still
  open*.

**The command log is display text. k8rs does not execute it, and nothing
in it is fed back into a process** (the security gate).

## Colour and symbols

- ANSI colour only when stdout is a terminal **and** `NO_COLOR` is unset. Piped
  or redirected output is plain text.
- `● ▲ ○` carry the severity by themselves, exactly as in the TUI: colour only
  reinforces. This is what makes the output readable after `| less`, in a CI
  log, or by someone who does not distinguish red from green.
- Common Unicode only, no nerd fonts ([docs/tech-stack § Visual identity](../docs/tech-stack.md#visual-identity)).

## Exit codes

| Code | Meaning |
|---|---|
| `0` | k8rs ran and reported — **whether or not anything was broken** |
| `2` | k8rs could not run: no kubeconfig, unreachable cluster, not allowed to list pods |

**Findings do not change the exit code.** k8rs is a report, not a linter: a
beginner who runs it, sees three warnings and then sees `$?` = 1 will conclude
the tool failed. `1` is left unused so that a future `--exit-code` flag, if one
is ever actually asked for, has somewhere to go without moving what `0` means.

Failures print the same plain-language stderr messages the TUI prints before it
ever enters raw mode — one text, both paths
([states.md](states.md#before-the-tui-ever-starts)). That page's *not allowed
to list pods* block now answers for the *unreachable cluster* row above too,
for the shape where the cluster accepted the connection and then never
answered — one function, `pods_unread`, prints both, because either way k8rs
ends the run with no pods and the reader needs the same three answers.

## What `--once` does not do

**Not on this list any more: the seven analysis reports.** `--once --analysis`
prints them — same seven panes as the console, after the findings — because
three shipped rules have no other reader: N4, N5 and C1's expiring band
return `Severity::Info` and nothing else, and the card block above never
draws that band. It is not the default: the default is the findings, and
seven whole-cluster panes stacked under three cards would bury the thing the
run exists to show
([NOTES § D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)).

| Not offered | Why |
|---|---|
| `-o json` / `-o yaml` | Nobody has asked. It is one function over `Vec<Finding>` when someone does, and inventing an output schema now means maintaining it forever ([NOTES § Out of scope](../NOTES.md#out-of-scope-the-most-important-section)). |
| `--watch` | That is the TUI. |

`--context`, `--namespace` and `--analysis` apply unchanged. `--read-only` is
accepted here too, but a `--once` run gives it nothing to do: the flag
refuses a write when the command line asks for one — `ops scale`, `ops
restart`, `ops delete` — and a `--once` line never does. So on this path the
flag has no visible effect, not because the write path does not exist, but
because this command never reaches it.

## The rule that matters most here

Findings contain names, messages and annotations from the cluster, and there is
**no ratatui between them and the terminal**. `sanitize()` runs on the same
strings, at the same boundary, before anything is printed
([invariant 9](../CLAUDE.md) · [widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
A pod named with an escape sequence must be as boring in `--once` as it is in
the console — and `--once` is the path that ships first, so it is the path that
gets the untrusted-input test first.
