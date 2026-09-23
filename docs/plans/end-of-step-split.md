# The split a long step can survive — end-of-step sources for the equivalent circuit

**Status: in progress, 2026-09-23.** Predictions below were registered before the engine
was changed. Scoring comes after them and does not edit them.

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
