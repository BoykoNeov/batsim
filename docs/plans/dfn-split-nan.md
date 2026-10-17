# A failed `Dfn` split must not commit NaN

**Status: PLANNED 2026-10-18.** Found by the Phase 12 spike's control runs (`phase-12-spike.md`, A3);
listed under `docs/ROADMAP.md` H8. It is the prerequisite of Phase 12 (`phase-12-plating.md`): the
plating lesson runs on a parallel `Dfn` pack, and a plating measurement taken on a solver that can
corrupt its own state would be worth nothing. When written the repo was at `SNAPSHOT_VERSION` 28.

## The defect

Shipped engine, `chemistries/nmc_21700_lgm50.toml`, recommended grid, isothermal. A 1S2P `Dfn` pack,
one cell at 0.9 of the other's capacity, charged from empty at 1C per cell under a current demand
with **one-hour steps**, at 0 °C or 25 °C:

* every step's split is unconverged (`SOLVE_UNCONVERGED`), and the split it commits is lopsided
  (34 % and 17 % charged after the first hour);
* the **third** step, which takes the small cell past full, writes NaN into both cells' `i_last`,
  `soc_deficit` and `tangent`;
* from then on the voltage is NaN and the state of charge reads 0 on every step, rest included —
  the pack never recovers.

Equal cells, one cell alone, 0.5C per cell, or 15-minute steps all converge. So the trigger is a
mismatched parallel `Dfn` pair split at a long step, and the overfill of one cell inside the step.

Two things are wrong, and only one of them is this slice's:

1. **`step()` leaves a non-finite number in the state.** `CLAUDE.md`: `step()` never panics and
   reports problems by flags. A NaN in the snapshot is a quieter failure than a panic, because
   every later step reads it. **This slice fixes it.**
2. **The split does not converge.** The bracketed settle that rescues an unconverged split on the
   other models skips any pack with a `Dfn` cell (`crates/sim-core/src/pack.rs`, the settle block
   after the pass loop): "measured, it settled none of the `Dfn` steps it was tried on". And the
   step that overfills the small cell is the `Dfn`'s missing physics past full (H8, "no physics past
   FULL within a step"), which no search supplies. **This slice measures whether lifting the
   exclusion settles anything here; it does not promise convergence.**

## The plan

1. **Reproduce in the tree first.** A failing test, `crates/sim-core/tests/dfn_parallel_long_step.rs`:
   the 1S2P pack above at 25 °C (no cold needed), five 1-hour charge steps, then five 1-hour rests.
   It asserts that every `Telemetry` field, every `CellView` field and a snapshot → restore round
   trip are finite after every step. Red on the parent before any fix.
2. **Find the first non-finite value.** Which line produces it, and from which input: the cell's own
   solve (an overfilling Newton that returns NaN rather than failing), the pack's arithmetic on a
   failed probe, or the commit of a split whose sum is finite but whose parts are not.
3. **The rule: nothing non-finite is committed.** Where a cell's advance would commit a non-finite
   value, the step falls back to a committed answer that is finite, conserves charge (the cells'
   currents sum to the group's, as the energy and charge ledgers require) and raises
   `SOLVE_UNCONVERGED`. Which fallback is the step's design question, measured not assumed:
   the last finite pass of the loop, the previous step's split scaled to this step's group current,
   or an even share. Sim time always advances by `dt`.
4. **Measure the settle on `Dfn` packs** (prediction 2), and keep the exclusion unless it settles
   steps the guard alone leaves unconverged, at a cost stated.
5. **A sweep for the rule**: 1S2P and 1S3P `Dfn`, scatter `σ` 0 / 0.05 / capacity factor 0.9, charge
   and discharge at 0.5, 1 and 2C per cell, a 4.2 V hold, steps of 1 s, 60 s, 15 min and 1 h, both
   grids, 0 and 25 °C. Counted: committed non-finite values (must be 0), `SOLVE_UNCONVERGED` steps,
   charge-ledger residual.

## Predictions (registered before any run)

1. **The NaN starts in the cell, not the pack.** The overfilling cell's `probe_at`/advance returns a
   non-finite current or deficit on the third step, and the pack commits it unchecked. Confidence:
   low — the spike saw the symptom, not the source.
2. **Lifting the `Dfn` exclusion settles none of the overfill steps** (the cell's curve has no
   finite answer past full, so `current_at` fails), but **may settle the first two steps**, whose
   split is lopsided before either cell is full. Confidence: moderate on the first half, low on
   the second.
3. **With the guard, the reproduction stays finite on all ten steps**, the overfill step and the
   rest of the charge stay flagged `SOLVE_UNCONVERGED`, and the rest steps converge.
4. **No converged step moves**: every golden and every existing test is bit-identical, because the
   guard only acts where a non-finite value would otherwise be committed.
5. **The sweep finds more than this one case** at 1 h steps (parallel holds or 2C on the fine grid),
   all removed by the same guard. Confidence: low.

## Exit

* The reproduction test, green; and the sweep, with **zero** committed non-finite values.
* Every step that converged on the parent is bit-identical (the full suite, and a hash of every
  telemetry frame across the sweep, parent against slice).
* A perturbation table: delete the guard → which tests go red; disable the fallback's charge
  balance → which go red.
* ROADMAP H8's NaN bullet closed or narrowed to what remains (the `Dfn`'s physics past full stays
  open; it is not this slice's).

## Deliberately not done

* **Physics past full for the `Dfn`.** That is the open H8 bullet, and Phase 12's plating gives an
  overfilling graphite somewhere for the current to go (the spike's prototype converged where the
  control did not), which is the honest route to it on a charge.
* No snapshot change is expected; if the fix needs one, it is stated and bumped here, not in
  Phase 12.
