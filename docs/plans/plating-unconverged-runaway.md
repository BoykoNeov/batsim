# A failed plating solve may not pile up plated lithium

**Status: PREDICTIONS REGISTERED 2026-10-19, before any change to the engine.** Found by Phase 12
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

(Filled in after the change.)
