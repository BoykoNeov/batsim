# Step 18's two-button repair, measured

The largest item under `docs/plans/path-numbers.md`'s "Not checked" heading, and the only
one there that carries numbers. That slice measured every trajectory quantity the guided
path quotes; this paragraph it left alone, because the claim is causal — *button ordering*
— and about the page rather than about the engine.

Page prose only. No Rust, no `web/pkg` rebuild, no version constant moved
(`SNAPSHOT_VERSION` 13, `API_VERSION` 2, `WASM_API_VERSION` 5).

## The claim, in four parts

> Then the reset, which is two buttons, an order, and a Run. Nothing advances while the
> page is paused, so **Clear latched BMS fault** on its own only unlatches: press Run and
> the short, still connected, delivers a *second* 184 A tooth and it latches straight
> back. Do it the other way round — **Clear queued** first, which despite its name is a
> repair and removes the short that already fired, then **Clear latched BMS fault** — and
> the pack simply sits there at 13.16 V with nothing flowing.

1. Nothing advances while paused. **Confirmed.**
2. Clear-latch alone, then Run → a second tooth, and it re-latches. **Confirmed, with the
   tooth's size corrected.**
3. Clear queued is a repair and removes the short that already fired. **Confirmed.**
4. Clear queued first, then the latch → the pack sits at 13.16 V. **The behaviour is
   right and the number is wrong**; and the framing that makes it an *order* does not
   survive being tested.

## Instrument

Two, and they agree to every digit the page prints.

- `M:\claud_projects\temp\path-numbers`, `cargo run --release -- button-order`. Out-of-tree
  Rust, path deps on `sim-core`/`sim-data`, reading `scenarios/` and `chemistries/` from
  the repo. Five button arms × two starting states, plus three side probes.
- `M:\claud_projects\temp\path-numbers\verify_buttons.py`, driving the shipped page over
  CDP: enter the path, `Next` ×17, and press the page's own buttons.

**`Step 1` stands in for `Run` where the quantity is one step long.** The tooth is a
single 0.5 s step and the readouts are per-frame; at this step's own 10× no poll can land
on it. `#stepone` is `advance(1)` — the same code path `Run` drives — and it leaves the
tooth on screen. `Run` is then pressed anyway, to read what the pack does afterwards.

**The page's state is not reachable from a driver.** `web/app.js` has no `export`s, so
`await import('/app/app.js')` returns an empty module object and `history.i_actual` cannot
be read for a peak. This is the opposite of the `spm-scenario` slice's finding that the
page's wasm *can* be driven by importing the pkg module — `web/pkg/batsim.js` is a module
with exports; `app.js` is a script that happens to be loaded as one.

## What the measurement found

### The second tooth is not the first tooth

183.8418 A, then **184.5300 A** — 0.69 A *bigger*, on a pack that has half a percent less
charge in it. The prose's "a second 184 A tooth" sits three sentences after "the spike is
the same 183.84 A", so a reader has every reason to take it for the same number.

Two things moved between the teeth: 0.56 points of charge and about 0.87 K. Both change
`R0`, so the run itself cannot attribute the difference — it only ever visits two of the
four corners. Four purpose-built uniform-temperature packs separate them:

| pack | tooth | pack R |
| --- | --- | --- |
| 90.000 %, 25.00 °C — the first tooth's state | 183.8704 A | 0.042099 Ω |
| 89.443 %, 25.00 °C — charge alone | 183.9218 A | 0.042076 Ω |
| 90.000 %, 25.87 °C — temperature alone | 184.7730 A | 0.041746 Ω |
| 89.443 %, 25.87 °C — the second tooth's state | 184.8249 A | 0.041724 Ω |

Warming does ~95 % of it. The lost charge is worth **+0.05 A and points the same way**,
which is the part worth teaching: on LFP's plateau half a percent of charge moves the
open-circuit voltage by 0.4 mV, so it cannot pull the current down the way intuition says
a flatter battery should.

These four corners are idealisations — the real pack has a temperature spread (25.87/25.93
at the mark) — so they give the direction and the dominance, not a budget that sums to
0.69 A. The corrected prose therefore says "accounts for nearly all of it" and "about a
twentieth as much" rather than quoting two addends that do not add up.

### 13.16 V is not a voltage this pack ever sits at

The repaired pack reads **13.236 V** on its first step and drifts to **13.252 V** over the
next minute as the RC pairs finish letting go. Page and harness agree (13.236 / 13.2361;
13.252 / 13.2553).

Inverse lookup — "when did the engine actually hold 13.16 V?", the instrument that turned
step 17's six wrong figures into one mechanism — says: **never**, at rest, in the first
300 s. The closest resting sample is **13.1678 V at t = 61.0 s**, one step after the
*first* tooth and 89 seconds before any repair happens, with the RC pairs still fully
depressed. That is a reading of the latched pack, not the repaired one. Provenance is not
asserted beyond that: the panel prints three decimals (`toFixed(3)`), so 13.1678 renders
as "13.168" and no display rounding produces "13.16" either.

Two states, not one, because "do it the other way round" carries no **Restart** and a
reader arrives at it having already taken the second tooth:

| starting state | first step after the repair | settled |
| --- | --- | --- |
| fresh (Back then Next, 89.443 %) | 13.2361 V | 13.2553 V |
| after the second tooth (88.883 %) | 13.1883 V | 13.2525 V |

48 mV apart at the instant, 3 mV apart a minute later. Both disagree with 13.16 V, so the
figure is wrong independently of which state the reader is in — but the corrected prose
now names **Back** then **Next** explicitly, because the number it quotes is the fresh
one.

The step's *own* previous paragraph is the other reason to pin the state: it tells the
reader to put `dt` up to 5 s. A reader who has not put it back repairs a pack that lost
5.57 or 11.14 points instead of 0.56, and reads 13.0666 V or 12.8793 V.

### "An order" is not a claim — the two buttons commute

Measured on both instruments: pressing **Clear latched BMS fault** and then **Clear
queued**, with no Run between, gives a pack identical to the prose's recommended order —
to every digit, in every arm, in both starting states. Of course it does: both are
instantaneous edits to state and nothing advances while paused, which the same sentence
says out loud.

What does not commute is the **Run**. The real instruction is "do not Run before you have
cleared the short", and the prose now says that instead of "an order".

### The repair claim, and its free confirmation

`clear_faults()` returns **1** at the mark — the queue is empty by then, and the one thing
it removes is the fired external short. The page's note reads *"cleared 1 fault(s) — the
queue, plus anything that had already fired: the external short, …"* and not *"nothing was
queued."* That count is the evidence for "despite its name is a repair", so the corrected
prose quotes it.

Clear-queued *alone* leaves the contactor latched and the current at zero, which is what
makes the paragraph about both buttons rather than about one.

### The mechanism, read rather than inferred

`pack.rs:1474` — "the BMS acts first, on the frame sampled at the end of the previous
step" — and `pack.rs:2279`, which samples "for the *next* step". While the contactor is
open no current flows, so the frame the BMS is holding reads a healthy resting 3.309 V per
group. Clearing the latch lets that stale-but-honest frame authorise one step, the short
takes it, and the frame sampled at the end of *that* step reads 1.339 V and latches. The
second tooth is the same one-step lag the step's earlier prose is about, arriving a second
time.

## The correction

The last five sentences of `one-step-that-got-through`'s `expect`. Rewritten to: lead with
"not the order of the buttons but whether you Run between them"; quote 184.53 A and say
why it is bigger; send the reader through **Back** then **Next** before the second
ordering; quote 13.236 V drifting to 13.25 V; quote the note's *1 fault*; and state that
the two presses commute.

**Then amended, because the first draft committed the defect it was fixing.** Both new
figures are `Step 1` readings, and the draft told the reader to press **Run** — at 10×
the tooth is gone before the readouts refresh, and a Run from the repaired mark leaves
13.252 V on screen, not 13.236. That is `path-numbers.md`'s "right but unreachable"
category, written fresh into the paragraph being corrected. The prose now names **Step 1**
where a figure needs it and gives 13.25 V as what running on actually leaves. The tell was
already in this slice's own page transcript — `Step 1 → 13.236`, `after a real Run →
13.252` — and in this doc's own sentence that "at this step's own 10× no poll can land on
it". **A measurement's own caveat about the instrument is usually a caveat about the
reader too.**

## Verification

- `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`
  clean; `cargo test --workspace` green. Exit status read directly, never through a pipe —
  `cmd | tail; echo $?` reports *tail's* status, which is how the last slice asserted a
  format gate that could not fail.
- `node --check web/app.js` clean. The 19 lessons are string literals in one array and no
  Rust gate stands between a mistyped quote and a dead page.
- The corrected step re-read on the shipped page after the edit, with the browser reloading
  the file that was actually changed.

## Found on the way, not fixed here

**The guided path's emphasis markers are not rendered — every `**bold**` and `*italic*`
reaches the screen as literal asterisks.** `proseHtml` (`web/app.js:2988`) escapes HTML
and converts `` `code` `` spans, and does nothing else; the lesson prose leans on `**`
throughout. Pre-existing and page-wide, not introduced here: the function is unchanged at
`HEAD` and 108 lines of the file already carried the markers. Found only because the
corrected paragraph was read back off the panel rather than out of the source — the same
reason the last slice added `node --check`. Not fixed, because a renderer change is a
different slice from a prose correction, and because "add bold to `proseHtml`" changes how
all 19 lessons look at once and deserves to be looked at rather than slipped in. This
slice's edit keeps the file's existing convention.

## Not checked

- The three remaining families from `path-numbers.md`'s "Not checked" list: the quoted
  parameter constants, step 5's "new hottest cell", and the page-behaviour claims
  (legends, hover, status-line wording). Untouched here.
- Whether any *other* step's prose quotes a voltage read at the wrong moment in the same
  way 13.16 V was. The inverse lookup is cheap and would answer it; it was not run.

See [`path-numbers.md`](path-numbers.md) for the slice that named this item, and
[`protection-escalation.md`](protection-escalation.md) for the scenarios this step uses.
