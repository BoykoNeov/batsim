# A failed plating solve may not pile up plated lithium

**Status: BUILT 2026-10-19**, predictions registered before any change to the engine (417210c) and
scored under "Results": all four that were measured held. Found by Phase 12
slice D's measurements (`phase-12-plating.md` §D); the owner chose to fix it before the lesson
(decision 1 there).

## What was found

A `Dfn` cell running the plating reaction, on the fine grid, charged at constant current from empty
to 4.2 V cold and fast enough that the negative's electrolyte runs to zero near its current
collector, then rested. The rest's solves do not converge — on a threshold cell too, at the same
charges, so that half is older than Phase 12 (ROADMAP H8's family). A threshold cell commits its
unconverged iterates and stays finite. A reaction cell does not:

* `march` books each sub-step's plating off the field it solved, converged or not. On the
  unconverged rest, one node near the current collector (node 11 of 40 on the 0 °C 2C charge), where
  the electrolyte is at its floor, comes out of each failed Newton with an overpotential that implies
  a plating rate growing about tenfold a step: 0.025 % of capacity plated at the end of the charge,
  2 % sixteen seconds into the rest, 10¹² % before the voltage itself overflows and goes NaN, about
  45 s in. `PLATING_RISK` is up on a rest.
* Booking nothing on an unconverged sub-step (tried first, out of tree) stops the stores growing in
  seven of the eight failing cases and not the eighth (−10 °C 2C at 5 s steps): the plating term
  inside the solve still drags the committed particles to 10¹³ mol/m³ and the voltage to NaN at step
  26, where the same charge on a threshold cell stays finite on every one of its 120 unconverged rest
  steps.

The eight cases (fine grid, rest 600 s after the charge; `W:\temp\claude\phase12d\sweep.txt`):
0 °C 2C at 1 s; 0 °C 3C at 1 and 5 s; −10 °C 1.5C at 2 s; −10 °C 2C at 1 and 5 s; −10 °C 3C at 0.5
and 2 s. Plus 0 °C 3C at 0.5 and 2 s and −10 °C 2C at 2 s, which went NaN too and were not re-run in
the experiment. None at 1C.

## The change

**A sub-step whose plating solve fails is solved again without the reaction**, and that solve is the
one committed: no plating in its rows, none booked, `PLATING_RISK` not raised by it. Where the
reaction cannot be solved the cell is, for that sub-step, the threshold cell the measurements above
show stays finite. The sub-step still reports `SOLVE_UNCONVERGED` whether or not the solve without
the reaction converges: the reaction's solve failed, and the step is not what the model says. The
same rule runs in the probe (`probe_at` marches a throwaway copy), so the pack's curve and the
committed step stay one computation. No state changes shape: no snapshot bump.

Tried out of tree (`W:\temp\claude\phase12d\wt`, experiment E3): all eight cases finite on every
rest step, the plated stores flat across the rest, `PLATING_RISK` never raised on it.

Not changed: the failure to converge underneath, which the threshold cell shares; that stays open
with H8.

## Predictions

1. **Plating off is bit-identical**: the change is inside `march`, which only a plating cell runs.
   Every golden and every test on a pack without the reaction is unchanged.
2. **A plating run whose every solve converges — committed and probed — is bit-identical.** Every
   assertion in `plating_dfn.rs`, `plating_health.rs` and `cold_hold.rs` passes unchanged; the slice
   D probe's lesson runs (0 °C 1C, 0 °C 0.3C, −10 °C 1C, 25 °C 1C at 1 s; the same at 5 s) print the
   same text to the byte.
3. **Exit 5 still passes** (`the_mismatched_pair_converges_with_plating_on`): its pieces' cell solves
   fail for the first three and a half minutes of the first one-hour hold step, so that step moves
   (less plating booked on those pieces) and stays flagged, still with `SURFACE_OUT_OF_RANGE` — the
   overfill the one-hour charge left is in the particles, which the solve without the reaction sees too.
4. **The eight cases**, pinned by a new test on two of them (0 °C 3C at 1 s, and −10 °C 2C at 5 s,
   the one booking nothing did not fix): finite on every rest step; the reversible and dead stores
   never above their end-of-charge values on the rest; `PLATING_RISK` never raised on it;
   `SOLVE_UNCONVERGED` raised on it.
5. **Cost**: a failed sub-step costs one more solve, at most about twice the failed step.

## Results

Scored 2026-10-19, release builds below normal priority on a machine another project's suite held at
100 % load (so wall-clock times here are slow, not comparable across rows).

1. **Held.** `cargo test --workspace --release --no-fail-fast`: 110 test binaries, 876 passed,
   none failed. `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all
   --check` clean.
2. **Held.** `plating_dfn.rs`, `plating_health.rs` and `cold_hold.rs`: every assertion passes. The
   one test with unconverged trials, `the_end_of_step_curve_still_falls_with_current` (3, 5 and 7 of
   11 trials unconverged at 900 s and 3600 s), prints the same counts and passes; its trials at one
   hour read the same voltages to 1e-5 V and take the same time as on the engine before (23–59 s
   failed, 8–10 s converged, both engines side by side). The slice D probe's lesson runs print the
   same text to the byte at 1 s and at 5 s steps; the one line that moved is the 0 °C 2C case at 1 s,
   which was NaN.
3. **Held.** Exit 5: `15 min: (14, 0, 0, true); 1 h: (4, 0, 1, true)`, as before.
4. **Held**, `crates/sim-data/tests/plating_runaway.rs`: both cases finite on every rest step, stores
   flat, no `PLATING_RISK`, every rest step `SOLVE_UNCONVERGED` (120 of 120 on both).
5. **Not separately measured.** Where both solves fail the cost is two failed solves; on the one test
   that has them the time did not move measurably (item 2).

**Finite is not right.** The rest the fix makes finite is still not a rest: on the 0 °C 2C charge at
1 s steps the reaction cell's terminal falls through 3.46 V at 60 s and 2.64 V at 300 s to 0.65 V and
stays there, flagged on every step, and its reversible metal never strips (no sub-step converges, so
none is booked). The threshold cell on the same rest is unphysical too — 3.39 V at 120 s, falling —
where the reaction cell's rest after the same charge at 5 s steps, which converges on every step,
reads 3.47 V at 300 s (3.46 V on the recommended grid at 1 s, also converged). That is the older
failure the plan records (H8's family) and it is not fixed here; what is fixed is that it can no longer
turn into NaN on a plating cell.
