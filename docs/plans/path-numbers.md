# The guided path's numbers, measured

`docs/plans/path-wedge.md` closed with three named follow-ups. This is the first of
them, quoted from that document:

> **Verify the path's numbers.** The walk above will run every step to completion for the
> first time; anything it contradicts gets recorded, and correcting it is its own slice.

The path is nineteen steps of `web/app.js` prose carrying several hundred specific
quantities — millivolts, seconds, points of charge, watts, kelvin, flag arrival times.
Most of them had never been checked against a run. This slice checks them, and corrects
the nine claims that were wrong.

**Nothing in `crates/` changed.** No Rust, no `SNAPSHOT_VERSION`, no `API_VERSION`, no
`WASM_API_VERSION`, no `web/pkg` rebuild. The whole diff is lesson prose and two of its
authoring comments.

## The instrument

`M:\claud_projects\temp\path-numbers` — a small out-of-tree Rust binary with path
dependencies on `sim-core` and `sim-data`, the same arrangement `phase6-baseline` uses
and for the same reason: it reads the repo's own `scenarios/` and `chemistries/` so it
cannot drift from them, and it is not a committed test because it is a measuring
instrument rather than an assertion.

It reproduces each step's setup the way `applyStep` does — scenario, ambient, BMS toggle
(`Scenario::build_pack_with_bms`), `dt`, mark — and mirrors the page's three client-side
demand programs, two of which are not demands the engine has:

| program | mirrored from | notes |
| --- | --- | --- |
| `Current` / `Rest` | — | straight through |
| `Pulse` | `pulsePhase` / `pulseDemand` | phase in **steps**, off `sim_time_s` read back from the pack |
| `CcCv` | `ccCvDemand` / `ccCvDone` | the 10 s sub-clock, the 1 mV-per-cell band, the completion test |

Two details of that mirroring are load-bearing and were both nearly got wrong:

- **`sim_time_s` is read from the pack, never accumulated.** Both controllers quantise
  by `Math.round(t / dt)`, which is exactly the defence against accumulated float drift;
  a harness that kept its own clock would put pulse edges a step out somewhere past
  t = 1000 and the error would look like physics.
- **`readNow()` is a `dt = 0` probe under the demand just dialled in**, not a rest read,
  and several steps' headline numbers are that probe rather than anything the run
  produces (step 16's 2.808 V is the clearest).

### The harness was validated against the prose before it was believed

Three validations, one per demand program, chosen so that a subtly wrong mirror could
not pass:

| program | step | claims checked | result |
| --- | --- | --- | --- |
| `Current` | 15 | 3.927 / 3.918 / 3.471 / 3.449 / 3.439 / 3.418 V, 58.3 %, 6.33 W, 2.495 V at 1060 s, 4.55 A·h | every one, to the digit |
| `Pulse` | 12 | 212.8 mV sag = 132.8 + 74.8 + 5.3, 99.5 % within 300 s, teeth 4–5 nine mV deeper | every one |
| `CcCv` | 9 | CC leg ends 5420 s, 95.3 %, 0.65 A at 5700, 0.27 at 6000, stops 6210 s at 99.5 % | every one |

Those numbers were measured at the page by earlier slices, so agreement is a
cross-check of the instrument against the shipped client and not a self-consistency
loop. **A first `Pulse` reading disagreed and the harness was wrong, not the prose**:
the rebound split is defined in `docs/plans/spm-scenario.md` as a `dt = 0` `Rest` probe
taken at the on→off edge, and reading the first *resting step* instead puts 2.9 mV of
RC decay on the wrong side of the split (135.7 / 71.8 against the true 132.8 / 74.8).
Both readings are of a real thing; only one is the thing the prose names. A definition
that lives in a plan doc is part of the measurement.

### `dt` came from the markup, not from the prose

Steps 12–19 pin `dt`; steps 1–11 run at whatever the box holds, which is the default.
Step 18's authoring comment asserts that default is 0.5 s — but that comment is prose in
the file under audit, so it is not evidence. `web/index.html` says
`<input type="number" id="dt" value="0.5">`, and `advance`'s `|| 0.5` fallback fires only
when the box parses falsy, which is a different statement. Confirmed from the markup
before any step-1-to-11 number was compared.

## What was wrong

Nine claims. Two of them are a *reachability* defect rather than a wrong figure, and are
marked as such because the next reader who re-measures them will find them correct.

### 1. Step 2 — "both empty within two seconds of each other"

Measured 4146.5 s (LFP) and **4154.0 s** (NMC): seven and a half seconds, not two. The
authoring comment's "(4146 s and 4148 s, measured)" was wrong in the same place.

The cause is in the step's own framing. 2 A on 2.303451 Ah is 0.8682 C; 2.6 A on 3.0 Ah
is 0.8667 C. Two round demand-box numbers are *not* the same C-rate, and 0.15 % of an
hour-and-a-bit is 7.5 s. The comment now says so, because "the time axis is not one of
the differences" is still the point and it survives the correction.

### 2. Step 2 — the 620 mV third chemistry is unreachable as instructed

`cc_discharge_lgm50` really does fall **620.0 mV** between 90 % and 20 % — at 0.868 C,
which on a 5.153 Ah cell is 4.47 A. A reader who loads it from the picker keeps the 2.6 A
in the box, where the fall is 618 mV but takes 5708 s to arrive, and this step's mark
stops the run at 4200 s with the pack still at **41.1 %**. The number was right and could
not be seen. The prose now names the current to dial in and says what happens if you
don't.

### 3. Step 3 — "a few hundredths of a percent by the end"

Measured at the step's own 300 s mark: **0.4884 points** across the eight tiles, and
**0.2830 points** worst-case between the two cells of one parallel pair. Neither reading
is hundredths; both are tenths, and the claim was low by roughly ten to fifteen times.

Both readings were taken because the paragraph could be read either way — the sentence
before it is about the parallel pair sharing a node, so the pair spread was the charitable
interpretation. It fails too. The same paragraph's later "half-point of disagreement" is
**right** (0.49), so the step contradicted itself and the corrected sentence now names
both quantities and says which part of the spread is the current split and which is
capacity scatter between series positions that never share a node.

### 4. Step 8 — "not the 3.6× two fresh packs would show"

Two packs aged from new for the lesson's own 200 000 s lose 1.0579 points at 25 °C and
3.7489 at 45 °C: **3.54×**. At 600 ks it is 3.550×. The ratio is duration-independent to
three digits, so this is not a measuring-window disagreement.

`docs/plans/scenario-catalog.md` recorded 3.6× from "1.83 points … and 6.51 … over
600 ks". Those are this harness's numbers to three significant figures (1.8323 and
6.5052) — the 3.6 came from **dividing the rounded pair**: 6.51/1.83 = 3.557, which
rounds up, where 6.5052/1.8323 = 3.550, which rounds down. A ratio taken from two
already-rounded quantities is a different number from the ratio of the quantities.

Everything else in that step is exact: the resistance-to-capacity ratio is **1.5000**,
0.5289 points at a quarter of the way in, 1.0579 at the mark (2.0002× for 4× the time,
which is `√t` on screen), and 2.8375 points for the 45 °C leg against 1.0579 — 2.682×.

### 5. Step 11 — the unprotected charge finishes past the mark

"With the BMS off the same pack runs on to **99.5 %**" is exactly right — at **4820 s**.
The step's mark is 4100 s, where the unprotected run is at 96.68 % and still pushing
2.1 A. The protected arm stops on its own at 3990 s, so only one of the two runs the step
asks a reader to compare completes inside the step. The prose now tells the reader to
press Run again, and names where it lands.

### 6. Step 14 — the per-step cost of the particle model

Claimed "about 8× the circuit's arithmetic per step at 20 shells (0.90 µs against 0.11 on
one cell)". Measured today, best-of-three, 1S1P, `Pack::step`:

| run | `Ecm` | `Spm/20` | ratio |
| --- | --- | --- | --- |
| 1 | 0.192 µs | 1.606 µs | 8.35× |
| 2 | 0.116 | 1.317 | 11.38× |
| 3–5 | — | — | 10.09× / 10.82× / 10.38× |

The **absolutes were struck from the prose rather than updated**. This repo's own memory
records that this machine has missed its fast state for five sessions running and that
perf here is to be reported as ratios; the table above shows why — the same binary, the
same pack, five minutes apart, moves 65 %. The ratio is now "about ten times", with a
sentence saying why it is a ratio.

Worth a note for whoever next touches the porous-electrode path: the ECM's absolute cost
is unchanged from the figure the SPM-scenario slice recorded (0.11 → 0.116 µs on the fast
run) while the SPM's has moved 0.90 → 1.32. That is a 1.4× regression on the SPM arm
across Phase 7, measured incidentally here and **not** investigated. It is the one thing
in this slice that might not be a prose bug.

### 7. Step 16 — "about 200× less per step"

The sentence is about the *below-the-boundary* regime, so it was measured there: at 1 C,
`dt = 2 s`, over the run to the 3484 s cut-off, the DFN costs **138×** the SPM (138.8,
137.6 on repeats). At the pair's own 3 C the same comparison is **480×** (477–488 across
four runs), because a solve that has to work harder is the same solve run more times.
Corrected to 140×, with the 3 C figure added — the spread is the interesting part and the
old number sat between the two without belonging to either.

### 8. Step 17 — six gap readings, each taken about half a sample early

This is the substantial one, and its cause is the trap the step's own fourth footnote
describes.

The two surface-gap numbers are sampled on a 250 ms clock (`CELLS_PERIOD_MS`) while
everything beside them is redrawn every frame. At this step's 200×, one sample is about
fifty simulated seconds. Every gap figure in the step was read while the run was moving
and labelled with the *fresh* clock next to it — so each is a true measurement of an
instant earlier than the label it was given:

| the prose said | the engine actually held it at | the label said | engine value at the label |
| --- | --- | --- | --- |
| neg 4.77 / pos 16.66 | t = 56 s / 56 s | 90 s | **5.28 / 20.02** |
| neg 5.67 / pos 24.70 | 162 / 164 s | 180 s | **5.71 / 25.58** |
| neg 5.80 / pos 30.19 | 334 / 314 s | 360 s | **5.80 / 31.27** |
| pos 33.88 | 518 s | 540 s | **34.15** |
| pos 36.12 | 780 s | 780 s | 36.12 |
| pos 37.14 | 1044 s | end (1060 s) | **37.18** |

The diagnosis is in the first column, not the last: each *pair* of numbers was held by
the engine within two seconds of one common instant, which is what a single glance at one
row looks like, and that instant runs 0–34 s behind the label. The lag shrinks as the
curves flatten, which is why the thirteen-minute reading is exact and the ninety-second
one is out by 3.4 points.

Three more readings in the same step come from the same glance:

- "By the time the negative gap first reads 0.00 the terminal is at **3.323 V** … the
  positive gap at that same moment is **6.74 points**" — the gap first prints `0.00` at
  t = 1396 s, where the terminal is **3.319 V** and the positive gap is **6.97**. The two
  numbers in that sentence come from two rows sampled at different times, which is exactly
  what the sentence after it warns a reader not to do.
- "3.06 and 3.340" → the positive gap falls to 3.06 points at 3.338 V.
- "That final **29 mV** takes **twenty minutes**" → 33 mV and twenty-four minutes, both
  following from the corrected starting voltage.

The corrected numbers are the engine's values at the stated times, because that is what a
reader who **stops at the mark** sees: the 250 ms clock keeps sampling while paused, so
the row settles onto the truth within a quarter second of the run stopping. The footnote
now says the row shows a state the clock beside it has already left, and records that an
earlier draft was written on the fly and was wrong six times over.

One collateral correction. "**5.81** for the whole remaining twelve minutes … not moving
in the second decimal" is false *as printed*: the plateau runs 5.802 at six minutes to
5.80516 at the cut-off, which straddles the rounding boundary, so the panel shows `5.80`
until about t = 505 s and `5.81` after. The claim was reworded to the plateau itself
(three thousandths of a point across the whole stretch, with the printed digit ticking
over once halfway through) rather than to a digit that does move, which is both true and
the thing the sentence was trying to say.

### 9. Step 19 — the peak temperature, and a headline contradicted by its own paragraph

"the pack peaks at **344.6 K**" — measured **344.546 K**, which prints as 344.5. One
digit, corrected. The cell that is genuinely hottest reads **344.518 K** at the latch
against the claimed 344.52, and the probe threshold is 343.15, so "late by 1.3 K" is
1.37 K and stands.

The step also opened with "**86 amps**" while its own later sentence says "the same
93.29 A on the first frame after the fault" — measured 93.29 → 87.04 at thirty seconds →
85.90 when `OT` fires. Both numbers are right and the headline is the settled value, but
this is the same species of defect as step 3's, where a figure is contradicted later in
its own paragraph, and it was nearly waved through on the grounds that 86 A is a fair
summary. The twin step leads with its *peak*. Now reads "93 amps settling to 86".

## What was right

The count matters more than the list, because the next reader needs to know which numbers
are already checked. **Every trajectory quantity the harness could reach reproduces** —
that is every voltage, current, charge level, temperature, watt and flag arrival time in
all nineteen steps — most of them to every digit printed. What that sentence does *not*
cover is listed under "Not checked" below, and the distinction matters: this document
exists so the next slice can skip what is done, and an overclaim here would make it skip
claims nothing has ever touched. In particular, checked and correct:

- **Steps 15 and 16 are exact throughout** — the two hardest steps in the path, every
  voltage, every SOC, every watt, the 535 mV collapse, `SOLVE_UNCONVERGED` at 466 s, the
  2.414 → 2.379 V solver shelf, 1.99 A·h against 4.55, the twin a full volt higher at the
  same instant, and the 1 C boundary at 12 s in 3484 (0.34 %).
- **The pulse trio's ratios are exact** — ×1.872, ×6.017, ×2.483 for the particle and
  ×2.992, ×3.000 for the circuit, against ×1.87, ×6.01, ×2.48, ×2.99, ×3.00. All five
  rebounds (17.3 / 24.3 / 31.7 / 35.4 / 37.2 mV) and all five circuit climbs (74.8 mV,
  identical to four decimals in volts) land.
- **Step 18's `dt` table is exact and was previously unmeasured**: 0.5572 / 5.5719 /
  11.1438 points at 0.5 / 5 / 10 s, a 19.24 K rise against 0.96 K, and the spike is
  183.8418 A at all three step lengths — bit-identical, because a resistive sag does not
  care how long you look at it. The step's own comment calls these the two numbers in the
  path that would be wrong by a factor if `dt` were not what it assumed; they are right.
- **Step 11's protection numbers are exact**, including the 16.775 V one step before the
  trip, the 25 mV of headroom, the 11.0 mV of cell spread, the 130 mV gap after the trip
  (which is the distance to the 16.80 V target, not the imbalance), the derate to exactly
  4.2000 A, and `UT` and `PLATING_RISK` at the *same* step, 1494.5 s.
- **Step 9's CC-CV knee does not move with the frame rate.** The controller is
  re-evaluated at every `advance` sub-call, including ones a frame boundary creates, so
  the obvious worry is that a browser's scheduling gets into the physics. Measured at 4,
  26 and 1000 steps per frame: the CC leg ends at 5420.0 s and the charge stops at 6210.0 s
  and 99.5195 % in all three. The step's claim that the rule is anchored to simulation
  time and not to frames is correct, and this is now a measurement rather than a reading
  of the loop. (My first reading of that loop concluded the opposite; the measurement is
  the reason it is not in the prose.)

## Not checked

Named rather than left inside the sentence above, because each of these was reasoned
about, inherited from an earlier slice, or is a claim about the page's behaviour rather
than about a number the engine produces. None is known to be wrong; none was run here.

- **Step 18's two-button repair — now CHECKED, see [`path-buttons.md`](path-buttons.md).**
  Three of its four parts hold; the 13.16 V is wrong (13.236 V, and no resting sample in
  the first 300 s reads 13.16 at all), the second tooth is 184.53 A rather than the first
  one repeated, and the two buttons commute exactly, so "an order" was never the claim —
  the Run is. The paragraph below is what it said before that measurement.
- **Step 18's two-button repair, which is the largest of these and carries numbers.**
  "Press Run and the short, still connected, delivers a *second* 184 A tooth and it
  latches straight back… do it the other way round — **Clear queued** first, then **Clear
  latched BMS fault** — and the pack simply sits there at 13.16 V." That is a causal claim
  about button ordering plus a repeat-spike magnitude, and this repo's memory already
  records one stale belief about the clear-fault wiring. It needs the page, not the
  harness.
- **Parameter claims quoted from files and other plan docs**: step 4's current sensor
  reading "20 mA high with 10 mA of noise" (in `soft_short_under_a_lying_sensor.toml`),
  steps 12 and 13's RC time constants of 9 s and 72 s, step 14's "the circuit stops at
  1.79" past the clamp, and step 17's diffusion times of 1040 s and 6812 s (measured by
  the surface-vs-bulk slice). The harness measured *around* all of these — the behaviour
  they explain reproduces — but not the constants themselves.
- **Step 5's "the temperature grid finds a new hottest cell."** The pack does spread from
  27.2 °C to 29.2 °C after the short, which is consistent with it, but which cell is
  hottest before and after was not compared.
- **Page-behaviour claims**: step 3's legend printing both ends of the scale and the
  click-to-pin, step 17's tile hover at three decimals and the pack grid's metric menu,
  and the status-line wording in steps 9 and 10. Format claims are checkable by reading
  the renderer and several were, but none was seen on screen.

## Verification

- `cargo fmt --all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
  clean, `cargo test --workspace` green (exit 0, read from `PIPESTATUS`). Expected — no
  Rust changed — and run because the gate is the gate. The `fmt` check was run **unpiped**
  the second time: `cargo fmt --all --check | tail -3; echo $?` reports *tail's* status,
  which is 0 whatever cargo did, and the first version of this line asserted a clean
  format on a probe that could not fail. Same family as the recorded lesson about reading
  cargo's exit code rather than splitting its output.
- `node --check web/app.js` passes. The page is nineteen lessons inside one array of
  string literals, so a mistyped quote in a prose edit takes out every step and no Rust
  gate would see it — and the last two edits in this slice were made *after* the browser
  had loaded the file for the verification below.
- No version constant moved: `SNAPSHOT_VERSION` 13, `API_VERSION` 2, `WASM_API_VERSION` 5.
  `web/pkg` needs no rebuild, for the same reason.
- Step 17's corrected numbers were re-read **off the shipped page**, at rest, through the
  page's own controls (`verify.py` beside the harness). All six pairs land exactly:

  | t | page | harness |
  | --- | --- | --- |
  | 90 s | `5.28 / 20.02 pts`, 3.791 V, 92.5 % | 5.282 / 20.019 |
  | 180 s | `5.71 / 25.58` | 5.712 / 25.576 |
  | 360 s | `5.80 / 31.27` | 5.802 / 31.270 |
  | 540 s | `5.81 / 34.15` | 5.805 / 34.149 |
  | 780 s | `5.81 / 36.12` | 5.805 / 36.123 |
  | 1060 s | `5.81 / 37.18`, 2.495 V, 11.7 % | 5.805 / 37.181 |

  and the rebuilt pack reads `0.00 / 0.00 pts` beside 3.927 V and 100.0 %, which is the
  step's opening claim. Note the printed negative digit is `5.80` at six minutes and
  `5.81` at nine, which is the collateral correction above, seen on the page.

  **Two things about that verification were harder than they look.** `GET /sessions`
  returns `[]` at this step, because step 11 forces `transport: "wasm"` and nothing
  switches it back — from step 11 onward the path runs entirely in the browser and the
  server has no session to ask about. And the step arms itself *running* at 200×, so a
  driver that walks to it and then presses Pause is already a few hundred simulated
  seconds in: the first run of this script paused at t = 240 s, and every reading it took
  was that much late. It agreed with the harness perfectly once the offset was applied,
  which is a cross-check in its own right — the panel and the engine report the same
  quantity — but it is not a reading at the labelled marks. The fix is **Restart**, which
  rebuilds at t = 0 without touching the controls, followed by counting clicks on the
  page's own single-step button: with `dt` pinned at 2 s and every mark an even number,
  the clock becomes countable and no formatted time has to be read at all.

## For the next reader

- **A number read off a moving panel needs the row's own sample clock, not the run's.**
  This page has two clocks: everything is redrawn per frame except the cell row, which
  samples four times a second. At 200× that is fifty simulated seconds. Six wrong numbers
  in one step came from that single fact, and the step already carried a footnote
  describing it — a warning in the prose is not a defence against the author.
- **When two numbers in a claim disagree with measurement by the same amount in the same
  direction, look for a common instant rather than a common error.** The inverse lookup
  ("when did the engine actually hold this value?") turned six independent-looking
  discrepancies into one mechanism in a single pass, and the tell was that each *pair*
  agreed with itself to within two seconds.
- **A ratio of two rounded numbers is not the ratio of the numbers** (step 8: 3.557
  against 3.550, which round differently). If a plan doc records a ratio, record the
  operands unrounded or the ratio will not survive being re-derived.
- **"Right but unreachable" is a distinct defect from "wrong", and it needs saying out
  loud.** Two claims here are exactly correct at conditions the step does not put the
  reader in — one needs a current the reader is not told to dial in, one needs a Run past
  the mark. Both would come back "confirmed" from any harness that did not also model the
  step's mark, which is why the correction is an instruction and not a figure.
- **Validate a mirrored controller against a definition, not against agreement.** The
  pulse split disagreed at first and the harness was at fault; the definition that
  resolved it was a sentence in another plan doc, not anything in the code.

## Named follow-ups, deferred here

- **The out-of-tree trajectory instrument's anchor is stale.** `ANCHORS.md` (in
  `M:\claud_projects\temp\phase6-baseline`) still anchors on `after-surface-gap.txt`, and
  its own "Blind spots" section lists a `Demand::Voltage` far outside a cell's window —
  the 2S SPM case's `CV 3.5 V` leg, which reaches −2.9e6 V and NaN. The voltage-window fix
  (`docs/plans/voltage-target-blowup.md`) clamps exactly that demand, so the instrument
  should move and the blind spot may now be closed. Neither was checked when that slice
  landed. One run, plus whatever the diff says.
- **The SPM's per-step cost has moved 0.90 → 1.32 µs since the SPM-scenario slice** while
  the ECM's has not moved. Noticed while replacing a prose number, not investigated, and
  not necessarily real — but it is the only measurement in this slice that points at code
  rather than at prose.
