# Phase 12, the spike — does the LG M50 cell ever reach the plating line when charged cold?

**Status: PREDICTIONS REGISTERED 2026-10-17, before any run. No Rust in the tree.** Step A1 of
`docs/ROADMAP.md` §3 "Phase 12". The harness lives outside the tree (`W:\temp\claude\phase12-spike\`);
only this note is committed.

## The question

Lithium plates on graphite when the graphite's surface sits below lithium metal's potential:
`φ_s − φ_e < 0` at a negative-electrode particle surface (no SEI film here, so no film drop). Before
anything is built, A1 asks whether the engine's porous models, on the shipped
`chemistries/nmc_21700_lgm50.toml` (v28), ever put the graphite there on a cold charge, and whether
they agree with PyBaMM about when.

## The runs

* **Protocol.** Constant-current charge (negative current in the engine) from empty (SOC 0) to the
  4.2 V cut-off, no constant-voltage leg, **isothermal** at the ambient — a worst case: a real 2C
  cold charge warms the cell, and warming moves the graphite away from the line. 1 s steps.
* **Conditions.** 0 °C and −10 °C, at 1C and 2C (C = `capacity_ah` = 5.153198 A). Plus a 25 °C 1C
  calibration run, which must agree before any cold figure counts.
* **What is read.**
  * Engine `Dfn`: `φ_s − φ_e` at the **last negative node** (the one beside the separator), read from
    the snapshot's `DfnState::u` after each committed step. Grids 10/5/10 with 20 shells (the
    recommendation) and 40/20/40 with 40 shells (`dfn::FINE_*`).
  * Engine `Spm`: `U_n(c_surf, T) + η_n` for the one negative particle, recomputed outside the engine
    from the snapshot's shells. **Trusted only if** the same recomputation of both halves, minus the
    contact drop, rebuilds `Telemetry::v_terminal` to about 1 µV on every step.
  * PyBaMM 26.6.2.0, Chen2020 with ORegan2022's terms (`fit_ecm.combined_parameter_values`), IDAKLU
    `rtol = atol = 1e-10`, 200 radial points: DFN `"Negative electrode surface potential
    difference [V]"` at its last `x_n` point, on 10/5/10, 40/20/40 and 160/80/160 (truth); SPM
    `"X-averaged negative electrode surface potential difference [V]"`.
  * Both codes are finite volume, so "the last negative node" is a cell centre, not the separator
    interface; a coarse grid reads it high. Matched grids compare the codes; 160/80/160 is the
    physics.
* **Recorded per run.** The minimum of that potential and when it happens; the time and charge state
  at which it first crosses 0 V, if it does; the charge state at the 4.2 V cut-off (so "never crossed"
  can be told from "stopped first"); every step's `SOLVE_UNCONVERGED`, `SURFACE_OUT_OF_RANGE` and
  `PLATING_RISK`.
* **PyBaMM with plating on.** The converged DFN again with `"lithium plating": "irreversible"` and
  OKane2022's plating keys (rate constant 1e-9 m/s, transfer coefficient 0.65; the set "does not
  claim to be representative"). Read: plated lithium as a share of capacity, and whether any plates
  while the potential is still above 0 V — whether "onset" is a sharp crossing or a smooth rise.

## Predictions (written before the first run)

1. **The physics reaches the line in the cold.** The converged PyBaMM DFN goes below 0 V at the
   separator side at −10 °C at both 1C and 2C, and at 0 °C 2C. At 0 °C 1C it stays above, with a
   minimum between 0 and +15 mV. Confidence: moderate on −10 °C; low on the two 0 °C calls.
2. **The `Spm` is the optimist.** It has no electrolyte potential drop and spreads the reaction
   evenly, so its minimum is higher than the `Dfn`'s at every condition — by at least 10 mV at −10 °C
   2C — and it crosses 0 V later or not at all. At 0 °C 1C it stays above 0 V.
3. **The engine's `Dfn` agrees with PyBaMM on a matched grid**: minimum within 5 mV, crossing time
   within 2 % of the charge time, on 40/20/40. On 10/5/10 against the converged 160/80/160 the
   minimum reads high by up to 20 mV.
4. **The calibration agrees.** At 25 °C 1C the engine `Dfn` matches PyBaMM's matched-grid minimum
   within 2 mV, and the `Spm` recomputation rebuilds the terminal voltage to about 1 µV.
5. **Plating on barely moves the cell.** With OKane2022's constants the terminal voltage and the time
   to 4.2 V change by under 1 %, and plated lithium stays under 1 % of capacity even at −10 °C 2C.
   Some lithium plates while the potential is still slightly above 0 V (Butler–Volmer has no hard
   threshold), but the amount is negligible until it crosses.
6. **Today's threshold disagrees at 0 °C.** It fires strictly below 273.15 K, so it flags both
   −10 °C runs and neither 0 °C run. If prediction 1 holds at 0 °C 2C, the threshold misses a case
   the physics plates in.
7. **The engine may struggle at −10 °C 2C.** The `Dfn` may report `SOLVE_UNCONVERGED` or
   `SURFACE_OUT_OF_RANGE` near the end of that charge; if so, those rows are kept and reported, not
   dropped.

## Results

*Not run yet.*

## Stop-point A2

Whatever A1 finds, the next step asks the owner whether any source for this cell's plating constants
exists, and stops there before any plan is written.
