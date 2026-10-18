# A voltage hold on a plating pack, cut into pieces

**Status: PREDICTIONS REGISTERED 2026-10-18, before any engine code.** Phase 12 slice B's exit 3
missed on its hold leg (`phase-12-slice-b-reaction.md`); the owner chose 2026-10-18 to fix it before
slice C, by the pack cutting its own step.

## Why the cell's sub-steps cannot do it

At a long step a voltage hold is **one** pack current, chosen so the step ends at the target. A cold
hold starts high and tapers, and plating is where the current is high. Replaying a long-step hold's
own one-current-per-step profile at 1 s plated what the long step plated to 0.4 %, so the cell's
plating integral is right and the profile is not: at 15 min the hold under-booked plating by 29 %.
No sub-step inside the cell can see a current the pack never chose.

## The change

`Pack::step` under a `Demand::Voltage` on a pack holding at least one cell that runs the plating
reaction cuts the step into `dfn::plating_substeps(dt)` pieces of equal length — the cells' own count,
so a piece is at most `dfn::PLATING_SUBSTEP_S` (5 s) and one cell sub-step — and runs each as a whole
pack step at the same demand. Every piece is a complete step: the electrical solve, protection, the
thermal network, the BMS, aging and faults all run per piece, so a sensor's noise is drawn once per
piece. Nothing new is stored, and the count depends on `dt` alone.

**Every other step is the one it was**: a pack with no plating cell, any demand but a voltage hold, and
any step of 5 s or less.

### What the step reports

One frame for the step, by each field's own meaning:

* **States, from the last piece**: `v_terminal`, `soc_true`, `soc_bms`, the temperatures, the cell
  voltages, the health fields, and every `CellView` (each cell's `current_a` is its last piece's).
* **Rates, as the mean over the pieces**: `i_actual`, `q_gen_w`, `q_runaway_w`, `q_balancing_w`,
  `i_balancing_a`, `i_internal_short_a`, `i_external_short_a`, `i_rejected_a`. A held step's pieces all
  end at the target, so `v_terminal · i_actual · dt` is the step's electrical energy and
  `i_actual · dt` its charge, exactly — what every consumer that integrates them (the tests, the
  example client) relies on. The cost: a CC–CV client watching `i_actual` for its taper sees the
  step's mean, which lags the end of the step.
* **Flags**: the union over the pieces.
* **The SEI film's `energy_last_step_j`**: the sum over the pieces.

Not `Demand::Power`: its voltage varies inside the step, so mean rates against the last voltage would
break the frame's energy identity, and no measurement asks for it.

## Predictions

1. **The hold-leg bar is met** — the slice B bar charge (1S1P, fine grid, 0 °C, from empty, 1C for
   30 min, then 4.2 V for 2 h): plated within 1 % at 60 s, 10 % at 15 min, 20 % at 30 min of the 1 s
   figure; expected within 0.5 % at each (5 s pieces, as the charge leg's 0.40 %).
2. **Bit-identical elsewhere**: every existing test passes unchanged except those pinning a plating
   pack's long-step voltage hold; the Phase 12 hash harness's six packs (no plating) hash as before;
   `dfn_parallel_long_step.rs::a_hold_from_past_full_settles` (no plating) unchanged.
3. **Exit 5 still converges** at 15 min and 1 h (now through pieces).
4. **Charge and energy close on a sub-stepped hold**: a 1S1P plating cell held at 4.2 V at 15 min steps
   — `Σ i_actual·dt` equals the lithium the cells took in (graphite + plated) to 1e-9, and the step
   frames' `Σ (v·i + q_gen)·dt` against the stored-energy change is within 2× the same ledger run at
   5 s steps.
5. **Snapshot → restore → continue** through a sub-stepped hold (bincode and JSON) is bit for bit.
6. **Cost**: a 15 min held step on a 1S1P fine-grid plating cell costs about 180 five-second pack
   steps, 0.1–0.5 s.

## Results

*(to be written after the runs)*
