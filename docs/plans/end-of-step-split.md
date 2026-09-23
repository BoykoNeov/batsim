# The split a long step can survive — end-of-step sources for the equivalent circuit

**Status: built, 2026-09-23.** Predictions below were registered before the engine
was changed and are left as written. Scoring, what the build changed, and one correction to
the measurement table come after them.

## What this is about

This was found while measuring something else. The slice being scoped was the remaining
half of H10's heat row: a mean terminal voltage on `Telemetry` beside the mean heat. The
first measurement for that ran scattered parallel packs at `dt` = 1, 60 and 3600 s, and
the 3600 s rows came back with energies thousands of times larger than the pack could hold.
That was not a reporting question. The engine had diverged.

**The defect.** An equivalent-circuit cell hands the pack solve a start-of-step Thévenin
source, `E = OCV(soc₀) − Σ V_rc,0` behind `R0`, and the solve holds the resulting current
constant for the whole step. That is explicit Euler on every coupling the cells have
through their shared node. Across a parallel group the coupling is the charge each cell
stores: a cell that gave more current ends the step at a lower SOC and a lower source, and
the next step's split over-corrects by more than it corrected. Its stiffness is roughly
`R0 · C_soc`, where `C_soc = 3600 · Q / (dOCV/dsoc)` is the cell's SOC "capacitance". For
the generic NMC 18650 in mid-range that is a few hundred seconds, and past about twice
that the error grows every step.

Measured on the tree at `6c7f960` (out-of-tree harness, `W:\temp\claude\mean-v`), a
scattered (`sigma` = 0.05) group at C/5:

| pack | `dt` | what happened |
| --- | --- | --- |
| NMC 18650 1S3P | 300 s | circulation grows: branch currents 0.47 → 1.34 A over eight steps while the group carries 1.5 A |
| NMC 18650 1S3P | 600 s | a cell charges its neighbour at −17 A by step 6; **5363 K** by step 7 |
| NMC 18650 4S3P | 3600 s | **11 459 K** by step 3 |
| LG M50 `Ecm` 1S3P | 3600 s | 55 000 K by step 5 |
| LG M50 `Spm` 1S3P | 3600 s | **−3.7e28 V** and negative absolute temperatures by step 5 |
| LG M50 `Dfn` 1S3P | 3600 s | **stable** — branch currents stay within 1.04 / 0.99 / 0.97 A to the end of charge |
| NMC 18650 1S2P, 1 Ω soft short, rest | 60 s | stable on the plateau; the surviving 1-second-step energy runs diverge once the shorted cell drains into the `[reversal]` ramp, whose 100 V/soc slope shrinks `C_soc` a hundredfold |

A single cell under a **voltage** or **power** demand has the same defect with no
neighbour at all, because the demand solve is the same explicit read of the same source:

| 1S1P NMC 18650 | `dt` | what happened |
| --- | --- | --- |
| `Voltage(4.1)` from 50 % | 60 s | chatter: −18.1 A, −0.46 A, −13.1 A, −0.30 A … on alternate steps |
| `Voltage(4.1)` from 50 % | 600 s, 3600 s | diverges; ±800 A and **90 000 K** by step 7 |
| `Power(5 W)` from 90 % | 3600 s | rings through zero current at empty |

**Why nothing caught it.** Every long-`dt` test in the suite is a single cell under a
current demand. `crates/sim-core/tests/scenario_aging.rs` fast-forwards at 3600 s with
`parallel: 1`, and a lone cell has no neighbour to swing against. The one model that
survives, the `Dfn`, survives because its `probe_at` is already a backward-Euler solve over
the step: its source is the *end-of-step* voltage as a function of current.

**Why it matters beyond the numbers.** `CLAUDE.md` forbids scripting an emergent failure.
An 11 000 K pack reached by an unstable integrator is worse than a scripted one. It looks
like the thermal runaway the lessons teach, and nothing in the telemetry says it isn't.

## The fix

Give the equivalent circuit what the `Dfn` already has. Over a step of `dt` under a
constant current `i`, a cell's **end-of-step** terminal voltage is affine in `i`, apart
from the OCV curvature:

```text
V_end(i) = E_dt − i · R_dt
E_dt     = E + Σ_j V_j,0 · (1 − d_j)                      d_j = exp(−dt / τ_j)
R_dt     = R0 + Σ_j R_j · (1 − d_j) + s · dt / (3600 · Q)
```

- The RC terms are exact. They are the same exponential `rc_update` applies.
- `s` is the slope of the OCV segment the cell sits on. It is the `[reversal]` ramp's
  `v_per_soc` for a cell at or below empty, and zero at a clamped top. Tables are
  validated monotone, so `s ≥ 0` and `R_dt ≥ R0`.
- `Q` is the capacity the coulomb count divides by, `eff_cap · soh_capacity`.

Every place the pack reads the aggregate — the demand solve, protection's input, the
external short, the per-cell split, the bleed and short currents, the operating-point
window — reads this one. The split then puts every parallel cell on one shared
end-of-step node, which is the backward-Euler condition, so a circulation mode decays by a
factor `R0 / R_dt` per step instead of being amplified. At `dt → ∞` that factor goes to
zero: the mode is removed in one step (L-stable), not left ringing as the trapezoid rule
would leave it.

**Approximations, stated.** Hysteresis memory, diffusion depletion, the OCV temperature
correction and the charge-acceptance taper are read at the start of the step, as today.
The OCV slope is the local segment's. Each only makes the step's source less exact. None
can make `R_dt` smaller than `R0`, so none can undo the stability.

**A zero-length step** takes today's `(E, R0)` through an explicit `dt > 0` guard, not
through `exp(0) = 1` arithmetic. The `SourceCache` memo keeps holding the `dt`-free
source; the correction is applied on top of it per step.

## What the reported heat means now

The start-of-step ledger could not survive this. Once the split equalises end-of-step
voltages, parallel cells no longer share a start-of-step voltage, so
`q = I·(OCV − V_start)` stops summing to one node's power. The ledger moves to the end of
the step, where they do share one:

- **`Telemetry::q_gen_w`** is the heat at the end-of-step overpotential,
  `i · (i · R0 + η₀ + ΔΣV_rc) + entropic`, with `ΔΣV_rc` the RC pairs' actual change over
  the step. Shunt and rejection heat are added as before.
- **The thermal network does not see that number.** It keeps integrating the step mean,
  start-of-step heat plus `i · excess`, exactly as `step-mean-heat.md` built it.
- **The ledger's partner is this step's `v_terminal`**, not the previous step's. That
  voltage has always been an end-of-step value. The property tests stop lagging it, and
  their demand varies from step to step, which a lagged pairing could never close.

## Predictions (registered before the engine changed)

1. **The seven diverging cases above stay bounded**: every cell under 400 K and every
   branch current under 10 A (the group's demand is a few amps).
2. **A circulation mode decays monotonically at `dt` = 1e6 s.** No sign flip between
   consecutive steps in a two-cell group released from a 20 % SOC mismatch.
3. **A 1S1P pack under a current demand is *not* bit-identical**, contrary to the first
   plan. The group aggregate `(E·G)/G` and the split `(E − V)/R` round differently with
   `R_dt` in place of `R0`, and `rc-resistance-growth.md` already recorded that a 1S1P
   pack does not hand its cell the demanded current bit-for-bit. Its cell trajectory
   moves by rounding only: `analytic_golden.rs` stays green at its 1e-9 tolerance.
4. **Every multi-parallel pack moves at order `dt/τ`.** The goldens that compare against
   PyBaMM are 1S1P, so they stay green. Guided-path claims on multi-parallel steps and on
   every CC-CV leg move, and each will be re-measured, not re-toleranced.
5. **The energy property tests redden until they switch partners**: the three named in
   `step-mean-heat.md`'s perturbation G, plus `the_reported_heat_is_still_the_first_instants`,
   which pins the meaning this slice changes on purpose.
6. **`Pack::step` at 100S10P, features off, costs more than 47.2 µs.** It adds one slope
   lookup and a few multiplies per cell, and two `exp` per step on a pack without aging.
   The prediction is that it stays under the 50 µs budget. That is not confident: the
   margin is 6 %.
7. **The `Spm` is not fixed by this slice.** Its tangent ignores `dt`, and it keeps
   diverging at 3600 s. That is the next slice, and it is recorded here so the −3.7e28 V
   row is not lost.

## Scoring

1. **Bounded — confirmed**, with one row of the measurement table corrected (below). In
   tree: `sim-core/tests/end_of_step_split.rs` holds the scattered 1S3P group at 300, 600
   and 3600 s (no branch ever pushed backwards), the 4S3P pack at an hour, and the voltage
   hold at 60, 600 and 3600 s; `sim-data/tests/end_of_step_split_shipped.rs` holds the
   shorted pair on the shipped NMC 18650. Out of tree: the LG M50 `Ecm` 1S3P at an hour
   keeps every branch between 0.96 and 1.07 A to empty (the group carries 3 A), and `Power(5 W)` at an hour runs to empty and
   stops rather than ringing.
2. **Circulation decays without a sign flip — confirmed** (`circulation_decays_without_changing_sign`,
   dt = 1e6 s, a half-capacity cell against a full one).
3. **1S1P under a current demand moves by rounding only — confirmed.** Every guided-path
   claim on a 1S1P current-demand step moved by 1e-13 to 1e-16 relative, nowhere near a
   tolerance. One test depended on the *direction* of that rounding:
   `diffusion.rs::saturation_lands_on_the_reversal_floor` sized one step to land exactly on
   empty, and the new rounding landed it 2.2e-16 past, with a deficit. The fixture now asks
   for one part in 1e13 less, so it lands short whichever way the rounding falls.
4. **Multi-parallel packs move; PyBaMM goldens stay green — confirmed, and the guided path
   moved more, and differently, than predicted.** 26 claims left their tolerance, on six
   steps, and 102 of 342 measured values changed at all (most by rounding). Two moves were
   not predicted:
   * **The window flag fires half a step earlier** on five steps (4138 → 4137.5 s,
     4111.5 → 4111.0, 335.0 → 334.5, 603.0 → 602.5, 5.0 → 4.5). `OPERATING_POINT_OUT_OF_WINDOW`
     is judged at the node the solve commits to, and that node is now the end-of-step one,
     which on a falling discharge is under the floor one step sooner. The prediction said
     "multi-parallel packs and CC-CV legs"; these are mostly 1S1P current demands, moved by
     the flag's instant rather than by the trajectory. The CC-CV legs, which *were*
     predicted to move, moved only inside their tolerances.
   * **Step 18's prose taught the defect.** It said a 5 s step's short-circuit spike "is the
     same 183.84 A, because a resistive sag is instantaneous", and that the damage is
     "exactly proportional" to the step length — 0.56, 5.57, 11.14 points, a factor of
     10.0000. That was the start-of-step solve holding its first instant's current for the
     whole step. The spike is now 182.59 A at 0.5 s and 173.16 A at 5 s, the damage 0.55,
     5.25 and 10.04 points, and the sentence says "nearly, not exactly, in proportion" and
     why. A real short's current falls as the RC pairs charge; the new tooth is the closer
     one.
   * Also re-read by hand, because nothing checks it: the unprotected short's temperature
     **peak moved from 245.5 s to 243.0 s**. The claim reads the value at a fixed instant,
     and the old instant's reading still sat inside the tolerance of the new peak — the hole
     the claim's own note had named as hypothetical. An out-of-tree scan of the whole run
     found it.
   * `protection-off` said the clamp flag "follows ten seconds later"; it is 10.5 s now, and
     the gap was dropped rather than respelled — both instants are printed beside it.
5. **The energy tests reddened — confirmed, and twice as many as predicted.** The four
   named ones did, and so did the other two of perturbation G's list, which pin the heat's
   value. Also red, unpredicted, and each updated to the end-of-step answer rather than
   re-toleranced: the analytic CV golden (`cv_demand_solves_current_from_rest`), both
   topology tests that pinned an `R0`-only split, the external short's `V/R` identity (now
   against this step's own voltage), and `arriving_at_empty_does_not_look_like_a_short` —
   which is what changed the design (below).
6. **Speed — not measurable cleanly, and most likely failed.** Two processes outside this
   session held the machine at ~60 % CPU for the whole window, so the quiet gate
   `pack-step-perf.md` requires never passed. Six alternating, pinned, ungated rounds per
   arm, minimum of rounds 2–6, registered as a ratio only: `100S10P/current` new/old
   **1.08**, `/power` 1.25, `/full` 1.22, `/full+aging` 1.10, `/full+aging_every_step`
   1.07. The loaded absolutes (65.9 → 71.2 µs on `current`) are not claimed. Scaled onto the
   last gated reading, 47.2 µs × 1.08 ≈ 51 µs: **over the 50 µs budget**, which had a 6 %
   margin. The cost is a second pass over every cell, one OCV-segment search per cell, and
   a few multiplies; the searches could come from the reporting pass that already brackets
   each cell's OCV. That is recorded under H9, not done here.
7. **The `Spm` is not fixed — confirmed** by construction: `CellModel::step_source_shift`
   returns zero for it and the nonlinear path was not touched.

## What the build changed in the design

* **The OCV slope at the ends of the table.** The plan gave a cell at exactly empty the
  `[reversal]` ramp and a cell at full a zero slope. Both were wrong for the direction the
  cell was about to move. At empty with no deficit, a *charge* climbs the table's first
  segment, and `arriving_at_empty_does_not_look_like_a_short` drew 15 A where the right
  answer is 17 A. At full, a *discharge* goes straight down the last segment, and a zero
  slope told a one-hour voltage hold that had overshot to full that it could discharge the
  whole cell in one step — it rang for three steps (−1.93, +2.67, −2.04 A) before settling.
  The rule now: the ramp only once a deficit is carried, otherwise the segment the charge
  state can move into. With it the one-hour hold overshoots once (−1.93, +0.17 A) and
  decays monotonically after — one linearisation error, spent once.
* **The synthetic short test proved nothing.** Written first in
  `sim-core/tests/end_of_step_split.rs`, it passed on the start-of-step engine too
  (perturbation A left it green). It moved to `sim-data` on the shipped cell, where the old
  engine fails it at step 78 with a branch carrying −54 A.
* **The decay memo got a debug assert.** It recomputes `exp(−dt/τ)` only when a cell's
  `soh_resistance` differs from the last cell's. Breaking that refresh reddened nothing,
  because only aged packs have cells that differ and nothing compares an aged split to an
  exact answer. It now asserts against a recompute, as the `SourceCache` memo does.

## Correction to the measurement table

The last row of the first table, written before the build, says the shorted NMC pair
"diverges once the shorted cell drains into the `[reversal]` ramp". Traced on the old
engine, it came apart *above* empty: the two branch currents alternate by a few hundredths
of an amp from the first minutes on, the swing grows as the short heats the pack to 349 K
and the cells slide down the steep bottom of the table, and at 2 % charge it explodes —
±284 A at t = 4860 s, `THERMAL_RUNAWAY` at 3530 K, 63 000 K by 6960 s. The same run at 1 s
drains quietly to empty and peaks at 349.9 K. So it is the first OCV segment (8 V per unit
of charge) and the hot, rising `R0` that shorten the coupling time under a minute, not the
ramp. And the runaway it produced is the case `CLAUDE.md` forbids by name: an emergent
failure the physics did not make.

## Perturbation table

`W:\temp\claude\mean-v\perturb.py`: one wrong edit at a time, `cargo test -p sim-core
--no-fail-fast` (row A adds `-p sim-data`), every failing test listed by name, the file's
bytes restored after.

| # | the wrong edit | what goes red |
| --- | --- | --- |
| A | the whole fix off | **13**: all four `end_of_step_split.rs` stability tests, `a_shorted_pair_drains_without_a_runaway_the_physics_did_not_make`, `arriving_at_empty_does_not_look_like_a_short`, `cv_demand_solves_current_from_rest`, `electrical_and_heat_energy_balance`, `soft_short_closes_the_energy_balance`, `external_short_conducts_at_the_solved_terminal_voltage`, both topology tests, and `every_claim_matches_the_engine`. `a_probe_step_reads_the_start_of_step_line` stays green, correctly: it pins what the fix must not change. |
| B | RC terms only, no OCV slope | **6**: the four stability tests and the two that sit on a slope (`arriving_at_empty…`, the external short). The RC terms alone do not stabilise a parallel group: its coupling is the charge, not the overpotential. |
| C | OCV slope only, no RC terms | **8**: the voltage hold, every test that pins the split or the demand to a closed form, and both energy balances. The group tests stay green — the slope is what holds them. |
| D | a clamped table end reads flat (the plan's rule) | **1**: `arriving_at_empty_does_not_look_like_a_short`. |
| D2 | only the full end reads flat | **1**: `a_voltage_hold_tapers_at_long_steps`, and only its one-hour arm. |
| E | reported heat left at the start of the step | **6**: both energy balances, the overcharge ledger, the two rejected-charge heat pins, and `the_reported_heat_is_the_end_of_the_steps`. |
| F | the decay memo never refreshes | before the assert: **nothing** — the finding that added it. after it, **5**, all aged packs whose cells' `soh_resistance` part company: `a_plating_run_replays_bit_identically_across_a_snapshot`, `a_warm_step_allocates_nothing`, `resting_mid_discharge_does_not_split_the_cycle`, `snapshot_roundtrip_survives_aging_faults_and_plating`, `the_short_roll_draws_once_per_plating_cell_in_series_major_order`. Debug builds only, like the memo assert it copies; release is covered by nothing, which is stated rather than hidden. |

## Still open

* **The `Spm` half** (H8). Same idea — a tangent taken over the step — on a model whose
  solid diffusion is itself stiff. The −3.7e28 V row is its measurement.
* **The speed budget** (H9), probably now exceeded by a few per cent on the features-off
  case, and unmeasurable cleanly while this machine is shared. The OCV-segment search is
  the obvious saving.
* **A step-mean reported pair** (H10's residue) is optional now: the end-of-step pair closes
  the ledger with no lag, and a mean one would need a current-weighted group voltage that
  has nothing to divide by when cells circulate at rest.
* **The frozen terms.** Hysteresis, diffusion depletion, the OCV temperature correction and
  the charge-acceptance taper are still read at the start of the step. None can undo the
  stability — each only makes `R_step` less exact — but the lead-acid depletion has its own
  time constant and an explicit read of it inside a parallel group has not been measured.
* **The CC-CV legs moved inside their tolerances**, as did about seventy other claims; their
  recorded values were updated where they really moved, but a claim whose tolerance is
  wide enough to absorb a change of solver is a claim worth re-reading.
