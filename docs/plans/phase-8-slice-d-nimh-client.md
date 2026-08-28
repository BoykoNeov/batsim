# Phase 8 slice D — teaching the nickel cell, and closing the phase

**Status: LANDED 2026-08-28.** Four scenario files, three guided-path steps, twenty-four
claims, two arms, eight ledger rules, two new claim quantities, one new derivation
operator, and four engine-side tests. **No engine code changed** — the only Rust touched is
tests. `SNAPSHOT_VERSION` stays at 18, `WASM_API_VERSION` at 6 and `sim_server::API_VERSION`
at 2; each was checked against its own doc comment rather than inherited from slice C's
sentence, because those two constants have parted before
(`docs/plans/ui-bms-view-slice.md`).

Slice C (`phase-8-slice-c-hysteresis.md`) shipped the `[hysteresis]` state, the
`ocv.t_ref_k` temperature correction and `chemistries/nimh_subc_3ah_generic.toml`. It
closed nothing about the phase's **second** exit criterion, which by the owner's scoping
decision is that a chemistry is done **when a guided-path lesson teaches it**. This slice
is that lesson, and it turned out to be three.

---

## What shipped

| file | what it is |
| ---- | ---------- |
| `scenarios/nimh_overcharge.toml` | 1S1P NiMH, 10 % charge, 25 °C, **thermally live**, no BMS |
| `scenarios/nimh_overcharge_isothermal.toml` | the same file with `[pack.thermal]` removed |
| `scenarios/nimh_memory_charged.toml` | 1S1P NiMH, 30 % charge, isothermal, **monitor-only BMS** |
| `scenarios/nimh_memory_discharged.toml` | the same file with `initial_soc` 0.3 → 0.7 |
| step 27, `the-charge-is-over` | the instant the cell fills, and that nothing outside says so |
| step 28, `and-then-it-falls` | the −ΔV, with the isothermal control beside it |
| step 29, `which-way-it-was-driven` | the resting-voltage memory, and the gauge it fools |

All three steps are **ledgered**, so `[ledger].unledgered` is still empty, and all three are
in `[ledger].spelled` at zero — prose written after the digits rule has no English
quantities in it by construction, and the scan says so rather than the list implying it.

---

## The structural finding: only a mark stops the page, so the −ΔV needs two steps

This is the thing that decided the shape of the slice, it was measured rather than assumed,
and it contradicts the plan's own sketch.

`phase-8-chemistries.md` scopes slice D as "a **-ΔV in millivolts on a telemetry row**, with
an isothermal control arm beside it". A fall needs **two** readings — the peak and the value
at the instant a charger fires — and both have to be readable. The first design put the mark
at the peak and the second reading on a `run-on` arm.

**That arm is unwalkable, and the reason generalises.** `frame()` clamps the step count to
`path.until`, calls `pathArrived()` and clears it; after that Run is unbounded and *nothing
else on the page ever pauses a run*. An arm gives the claims harness a trajectory and gives
the **reader** no stopped instant. On this trajectory `terminal` moves one printed digit
about every 15 s of simulation, which at any watchable speed is a fraction of a second of
wall clock. A claim on it would have been true and unreachable — the defect class this repo
has hit in `path-numbers.md`, `reversal-ui.md` and `path-ledger-idle-step.md`.

What does work is already in the file: `applyStep` does **not** reload a step whose mark is
ahead of the clock on the same scenario, which is why "lessons 2 to 4 are one continuous
run". So steps 27 and 28 are one run on one scenario with ascending marks, and both ends of
the subtraction are read stopped. **A second mark is the only instrument this page has for a
quantity that changes while you watch it**, and that is worth knowing before the next lesson
that needs one.

Two consequences, both checked rather than reasoned:

* Every control step 28 declares that a trajectory can see — `scenario`, `demand`,
  `ambient_c`, `bms`, `dt` — is identical to step 27's, and `reload` is absent. The speed
  slider is the one thing that differs and the one thing no trajectory can see.
* Step 28 files a claim at `read_at_s = 3240.5`, which is **another step's mark**. That is a
  first for this file. Check 6 has no arm for "the step next door said so", and the sentence
  prints both ends of its own subtraction, so the reading is claimed again here — where it
  is genuinely reachable, since this step's own run passes through it.

---

## The three lessons

### Step 27 — the charge is over, and nothing outside says so

Mark at **3240.5 s**, which is one step *past* the clamp at 3240.0. That is deliberate: at
the clamp itself only a sliver of the current is refused, so `clamp` reads `refused 0.000 A`
and `heat` is still 0.04 W. A mark there shows the transition rather than the state.

| row | prints |
| --- | ------ |
| `sim time` | `54m` |
| `terminal` | `1.518 V` |
| `current` | `-3.000 A` |
| `soc (true)` | `100.0 %` |
| `cell t` | `25.9 / 25.9 °C` |
| `heat` | `4.24 W` |
| `clamp` | `refused 3.000 A` |

The lesson is the gap between the rows a charger has and the rows it does not. Current
unchanged, voltage at its highest and still climbing to this step, no flag of any kind —
and the charge has been over since the step before. The only flag on this whole trajectory
is `SOC_CLAMPED_HIGH`; the peak is 1.518 V against a `v_max` of 1.60, so
`OPERATING_POINT_OUT_OF_WINDOW` never arrives and the prose does not promise it.

**The peak is the mark's own reading, not the run's maximum**, and the note on that claim
says so. The maximum is 1.517831515 at 3240.0, one step earlier and 0.034 mV higher; both
print `1.518 V`. Claiming the reading a reader can stop on rather than the extremum is the
same choice `path-ledger-past-empty.md` had to make, and pre-empting the objection cost one
sentence in a note.

### Step 28 — the signal a charger stops on

Mark at **3376.5 s**, the first step whose cell is 10 K above the peak's temperature. That
instant is the assertion. The spike recorded scoring a pre-registered prediction green over
a twenty-minute run where the honest figure at the moment a charger fires was under half of
it, so moving this mark somewhere the digits are larger would be that defect again.

`terminal` reads **`1.509 V`** against the last step's **`1.518 V`**: the two printed rows
are **9 mV** apart, and the engine's own difference is 8.772 mV because both rows round to
the millivolt in the same direction. The prose says that out loud rather than leaving a
reader to find it — the idiom step 23 uses for its heat ratio.

The isothermal arm reads **`1.519 V`** at the same instant, and has read it since the cell
filled: between the clamp and the end of the run the value moves 0.0006 mV. So the fall is
temperature and nothing else. The arm is unusually forgiving to walk, which is worth noting
because most are not: past the clamp that cell's terminal does not move at all, so stopping
anywhere in the overcharge reads the same string.

Two honest wrinkles are in the prose rather than left to be discovered:

* The pinned cell's reading is **higher** than the live cell's own peak, 1.5186 against
  1.5178. The live cell has already warmed 0.87 K by the time it fills, so it arrives at the
  clamp with a little of the fall spent.
* A peak followed by a fall is **structurally guaranteed** here — the spike measured that —
  so the shape of the trace is evidence about nothing and only the size counts.

### Step 29 — the cell remembers which way it was driven

Mark at **4320 s**: 720 s at 1 C, then an hour of open circuit. The twin starts at 70 % and
takes the same 20 points out. Both land on exactly 50 %.

| row | this file | the twin |
| --- | --------- | -------- |
| `terminal` | `1.290 V` | `1.240 V` |
| `soc (true)` | `50.0 %` | `50.0 %` |
| `soc (bms)` | `79.8 %` | `20.2 %` |

50 mV apart off the two panels (49.663 mV in the engine), on two cells at the same charge
and the same temperature, and it does not decay. Under load, at the end of the two legs,
the same pair is **224 mV** apart and the memory is a fiftieth of that — so the thing is
only legible once everything else has stopped, which is exactly when a gauge tries to read
it.

**The estimator half is the sharpest thing in the slice, and it is free.** `ocv_invert`
knows nothing about the hysteresis state — slice C left it that way deliberately, on
principle 8 — so each BMS inverts its own cell's displaced resting voltage through a table
that has one curve where the cell has two. Measured on the shipped table:

| reading | inverts to | segment slope |
| ------- | ---------- | ------------- |
| 1.289832 V (charged, rested) | 0.798320 | 0.100 |
| 1.240168 V (discharged, rested) | 0.201680 | 0.100 |
| 1.265 V (the true midline) | 0.500000 | **0.050** |

`min_ocv_slope_v_per_soc` is the guard that refuses a reading taken on a flat curve, and
**the guard refuses the reading that would have been right and accepts both that are
wrong** — because hysteresis has displaced the resting voltage onto a steeper part of the
table. Each gauge is out by 29.8 points, in opposite directions.

---

## Two numbers in that scenario needed an argument, not a value

Both are unusual choices for this repo and both are argued in the file, with a test holding
them.

**`min_ocv_slope_v_per_soc = 0.08` reads as tuned to produce 79.8 % and is not.** *Any*
threshold in (0.05, 0.10] gives identical behaviour, because the guard's whole decision is
which segment of the table the reading lands on. The shipped value is the midpoint of that
measured interval, and `the_memory_pair_differs_only_in_where_it_starts` asserts the
**interval** rather than the value. Worth keeping beside it: the 0.15 every other scenario
here uses blocks the correction on this chemistry at every state of charge, so a NiMH gauge
that kept the lithium threshold would be pure coulomb counting with no reference — which is
what makes lowering it a designer's reasonable move rather than an author's convenience.

**`rest_time_for_ocv_s = 1800`, against the 600 the protection scenarios use.** At 600 s the
correction fires at 1320 s while this cell's 300 s RC pair is still relaxing, lands on a
0.20-slope segment, and produces the same headline **for a different reason** — an
unrelaxed overpotential read as a state of charge, which is the failure that field's own doc
comment warns about. Two mechanisms in one number is not an attribution. At 1800 s the
terminal has been printing its settled value for eleven minutes.

**The current sensor is perfect, which is the opposite of what this repo's other BMS
scenarios do.** Their subject is drift; this file's subject is the correction, so the
coulomb count has to be exact for every point of error after it to belong to one thing.

---

## What the checks caught, and what measurement caught

**The advisor caught a sentence that was about to ship false.** The draft said the `heat`
row "sat at 0.04 W for the whole charge" — unmeasured, and wrong. It is **negative** for
most of the climb: −0.23 W on the first step, and the cell cools to 24.74 °C around
t = 300 s before it starts warming. That is the entropic term running backwards on charge,
it is a real NiMH behaviour, and it is the observable the shipped `docv_dt_v_per_k` was
*sized* against. It is now a sentence in step 27 and a test,
`a_charging_cell_cools_before_it_warms`, which is the nearest thing in this repo to a check
on that derivation.

**The digits rule refused one sentence**, as it did on the first prose written after it in
slice B: `a week` in step 29's "both cells would still be reading those numbers a week from
now". Rewritten without a quantity. A false alarm costs one sentence and is visible at once,
which is the whole argument for the ban.

**The double-cover panic caught three rules in a row**, each a phrase general enough to
account for a number in a step it was not written for: `starting at {n} % charge` reached
step 21's, `The demand box says {n}` reached step 25's `200 A`, and `starts at {n} % charge`
reached step 26's. Each was narrowed by the words around it. That fence has now caught
mis-pointed rules in five consecutive slices and it is the most productive check in the
file.

**A negative control needs its sign in the phrase, not in the token.** `The demand box says
-3` failed to account for anything, because the scanner's tokens carry no sign. The fix is
the shape step 20's charge-leg rule already uses: the minus goes in the phrase and the tie
takes the magnitude.

**Nineteen self-stated counts went stale**, repaired mechanically from the test's own
messages with the loop `phase-8-slice-b-lto-client.md` recorded. Adding steps to this path
costs about that many every time, and the script is at
`M:\claud_projects\temp\phase8b\fixcounts.py`.

---

## The browser walk found a stale engine, and the rows that disagreed were the right ones

The claims harness is a mirror of the page written in Rust, so it can be right about a
trajectory the page never shows. Both -ΔV steps were therefore walked end to end in
headless Chrome against `sim-server` — and the first walk **disagreed with every claim that
was about this slice's physics and agreed with every claim that was not**:

| row, at step 27's mark | the walk showed | the harness says |
| ---------------------- | --------------- | ---------------- |
| `sim time` | `54m` | `54m` |
| `soc (true)` | `100.0 %` | `100.0 %` |
| `clamp` | `refused 3.000 A` | `refused 3.000 A` |
| `terminal` | **`1.495 V`** | `1.518 V` |
| `cell t` | **`24.0 / 24.0 °C`** | `25.9 / 25.9 °C` |
| `heat` | **`4.17 W`** | `4.24 W` |

`web/pkg/sim_wasm_bg.wasm` was dated **2026-08-13**, fifteen days and one `SNAPSHOT_VERSION`
behind the engine: the bundle is untracked, so it is a local build artifact, and nothing
rebuilds it. The page was running a **pre-v18 engine** that has no `[hysteresis]` and no OCV
temperature correction, silently ignoring both sections of the chemistry file it had just
fetched.

The size of the disagreement is the diagnosis, and it is worth writing down because it is
how the next one will be recognised. A cell that is 1.9 K **cooler** should read *higher* on
charge through both temperature channels, and this one read 23 mV **lower** — which is very
nearly the 25 mV of charge-side hysteresis the old engine cannot apply. The missing
displacement also costs `M·|I|` = 0.075 W of dissipation, which is why its cell never got as
warm. Rebuilding with `wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg`
and walking again is what settled it.

This is the third recording of the same trap (`ui-bms-view-slice.md`,
`lead-acid-client.md`), and the useful half is which rows can tell: **the clock, the charge
state and the flag column agreed throughout**, exactly as they did the last time. A stale
engine is not visible in the rows a reader would check first.

**Walked again against a rebuilt bundle, every row agreed**, and these are the strings a
reader sees:

| row | step 27, at its mark | step 28, at its mark |
| --- | -------------------- | -------------------- |
| `sim time` | `54m` | `56m` |
| `terminal` | `1.518 V` | `1.509 V` |
| `current` | `-3.000 A` | `-3.000 A` |
| `soc (true)` | `100.0 %` | `100.0 %` |
| `cell t` | `25.9 / 25.9 °C` | `35.9 / 35.9 °C` |
| `heat` | `4.24 W` | `4.20 W` |
| `clamp` | `refused 3.000 A` | `refused 3.000 A` |
| flags | `SOC_CLAMPED_HIGH` | `SOC_CLAMPED_HIGH` |

Each is the string its claim declares, and **the subtraction the lesson asks the reader to
do is the one the page supports**: `1.518` then `1.509`, 9 mV, off two stopped marks. That
was the load-bearing thing the walk existed to settle.

Two environment notes, both costly here and both recorded before: a plain reload is not
enough because the browser caches the bundle — the driver has to send
`Network.setCacheDisabled` first — and **stale headless Chromes hold the profile**, so a
"new" launch with the same `--user-data-dir` silently delegates to the old browser and drives
a page from a previous run.

It also confirmed the structural claim nothing else covers. Step 28 read `55m` on the clock
**before** its Run was pressed — the pack carried over from step 27 rather than being rebuilt
at zero, which is what the whole two-mark design depends on.

---

## What the harness needed

Three additions, each the smallest thing that would carry a sentence, and each in an idiom
the file already argues for elsewhere.

* **`soc_bms_at`** — the estimate itself. `soc_gap_pts_at` (the error) existed and no
  quantity read the row, so `soc (bms)` could not be display-claimed. Step 29 prints both
  and neither could carry the other.
* **`t_max_c_at`** — the same temperature in Celsius. This is the interesting one: a claim's
  `spells_pow10` carries a *scale*, and Celsius against kelvin is an **origin shift**, which
  no power of ten expresses. Every earlier temperature claim in this file states kelvin,
  which is fine for a sentence about the engine and wrong for one that tells a reader to
  look at a row printing 25.9. `States::Displayed` was the other candidate and does not fit:
  it forbids `spells`, and the only thing that then accounts for the number is
  `Accounted::Shown`, which is documented as `sim time`-only.
* **`Op::Difference` and `Derivation::pow10`** — `Op::Ratio` was the only operator, and this
  slice's two headline numbers are subtractions. The `pow10` field is what makes them
  honest: `derived_value` reads its operands out of the **sentence**, so the difference is
  the one a reader computes off two printed rows, and carrying it into millivolts is what
  makes the rounding visible instead of hidden.

---

## The exit criteria this closes

**Criterion 2 — both new chemistries are taught. CLOSED.** LTO in slices A and B, NiMH
here. Each has a parameter file with provenance on every constant, scenario files, guided
path steps, and claims under the digits rule.

**Criterion 4 — the nickel cell's end-of-charge signature is emergent. CLOSED.** The
emergence was measured in slice C: the isothermal control gives *exactly* zero and the fall
splits roughly 40/60 between the OCV temperature correction and `R0(T)`. What this slice
adds is that it is **reachable in the client**, which is what the owner's scoping decision
requires of a chemistry, and that the control is on the page beside it rather than only in a
Rust test. Nothing here re-litigates the physics.

**Criterion 3 — the floor did not move.** Untouched again: no engine code, and the four new
scenario files are files no existing pack loads.

**Criterion 1** closed on slice A. **So Phase 8 is complete**, and by its own stopping rule
rather than by the list of interesting chemistries running out — which it has not, and
`phase-8-chemistries.md` names NiCd, LMFP, lithium-sulfur and solid-state as the ones
deliberately left.

---

## Deliberately not done

* **No charge-acceptance taper**, still. The spike cut it and slice C took the cut; the
  lesson is written about the millivolts and says so.
* **No fourth step on what an unstopped overcharge does.** It was drafted and dropped: the
  run reaches about 155 °C and 1.454 V, and every reading on the way is mid-run and
  therefore unclaimable. The fact is in the scenario file's comment and in
  `past_the_peak_nothing_turns_round`, and the lesson's closing sentence is qualitative —
  which is a shape `phase-8-slice-b-lto-client.md` recorded shipping *false* twice, so it is
  backed by that test by name rather than left to be believed.
* **`ocv_invert` is still not taught about hysteresis**, and should not be. Principle 8 says
  the BMS sees sensors and nothing else; the gap is the lesson.
* **No lead-acid `[hysteresis]` section.** Slice C's reason stands: no fitted constant
  exists, and adding one would both invent a number and put criterion 3 in play.
