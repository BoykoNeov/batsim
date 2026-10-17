# Phase 12, the spike — does the LG M50 cell ever reach the plating line when charged cold?

**Status: SPIKE RUN 2026-10-17/18 — A1, A1b and A3 run (predictions registered before each), A4 not
runnable; A2's questions answered by the owner. No Rust in the tree: A3's prototype is
`W:\temp\claude\phase12-spike\proto\patch_dfn.py` applied to commit 951a637. Next: the phase plan, when the
owner says so** — written 2026-10-18: `phase-12-plating.md`, after `dfn-split-nan.md`. Step A1 of
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

## Results (run 2026-10-17)

"Potential" below is always the plating driving force, `φ_s − φ_e` at the separator-side negative
node (the `Dfn`) or `U_n + η_n` of the one particle (the `Spm`), in mV; "crosses at" is the charge
state when it first goes below 0 V; "4.2 V at" is the charge state the charge stopped at. In every
run the lowest potential is on the run's last step, at the 4.2 V cut-off.

### The instrument (prediction 4: held)

* **`Spm`.** The recomputed halves rebuild `Telemetry::v_terminal` to **≤ 1.8e-15 V** on every step
  of every run, so the negative half read outside the engine is the engine's own.
* **`Dfn`.** `φ_s` at the positive collector node minus `φ_s` at the negative collector node rebuilds
  `v_terminal` to ≤ 1.8e-15 V on every step: the `u` in the snapshot after a step is that step's
  committed field.
* **25 °C 1C, engine against PyBaMM on the same grid:**

| | lowest, engine / PyBaMM | crosses at, engine / PyBaMM | 4.2 V at, engine / PyBaMM |
| --- | --- | --- | --- |
| `Spm` | +6.10 / +6.01 | never / never | 0.786 / 0.787 |
| `Dfn` 10/5/10 | −14.38 / −14.47 | 0.577 / 0.575 | 0.680 / 0.680 |
| `Dfn` 40/20/40 | −17.52 / −17.59 | 0.555 / 0.553 | 0.680 / 0.679 |
| PyBaMM DFN 160/80/160 (converged) | −18.41 | 0.547 | 0.679 |

**The first finding is already here: a 1C charge at room temperature takes this cell's graphite
below 0 V**, from just past half charge to the cut-off, in the `Dfn` and in PyBaMM alike. The `Spm`
stays 6 mV above.

### The cold charges (predictions 1, 2, 3, 6, 7)

Engine / PyBaMM on the same grid; the converged column is PyBaMM alone.

| condition | `Spm` lowest | `Spm` crosses at | `Dfn` 10/5/10 lowest | `Dfn` 40/20/40 lowest | `Dfn` 40/20/40 crosses at | converged lowest / crosses at / 4.2 V at | today's flag |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 °C 1C | −21.9 / −22.1 | 0.550 / 0.564 | −83.1 / −83.6 | −88.1 / −88.5 | 0.154 / 0.154 | −89.9 / 0.151 / 0.501 | **no** |
| 0 °C 2C | −30.8 / −30.7 | 0.227 / 0.226 | −189.6 / −190.6 | −206.9 / −207.2 | 0.032 / 0.032 | −210.8 / 0.032 / 0.180 | **no** |
| −10 °C 1C | −25.7 / −25.4 | 0.273 / 0.273 | −119.9 / −120.1 | −126.7 / −126.8 | 0.074 / 0.073 | −128.5 / 0.072 / 0.387 | every step |
| −10 °C 2C | −50.5 / −51.0 | 0.166 / 0.167 | −220.7 / −228.4 | −251.5 / −252.6 | 0.024 / 0.024 | −259.0 / 0.024 / 0.077 | every step |

No `SOLVE_UNCONVERGED` and no `SURFACE_OUT_OF_RANGE` on any step of any engine run.

### At the charge rates the maker allows

LG's sheet (cited in the chemistry file) allows 0.3C charge at 0–25 °C and 0.7C at 25–45 °C, and
none below 0 °C. Not predicted; added after the cold table, because every condition above breaks
the sheet.

| condition | `Spm` lowest | `Dfn` 40/20/40 lowest (crosses at), engine | PyBaMM 40/20/40 | today's flag |
| --- | --- | --- | --- | --- |
| 25 °C 0.3C | +47.9 | +37.0 | — | no |
| 45 °C 0.7C | +47.5 | +29.7 | — | no |
| 0 °C 0.3C (cold edge of its row) | +6.3 | **−8.7** (0.705) | −9.0 (0.702) | no |
| 25 °C 0.7C (cold edge of its row) | +19.0 | **−1.6** (0.741) | −1.9 (0.738) | no |
| −10 °C 0.3C (the sheet forbids) | −11.4 | **−32.3** (0.562) | — | **no** (0.3C is under its 0.4C) |

This is **consistent with** the sheet — comfortably above at the warm end of each row, just touching
the line at the cold end, well past it where the sheet forbids charging — and no more than that:
Chen2020 was never fitted to plating, and a maker's limits carry margins for ageing and for
cell-to-cell spread.

### PyBaMM with plating on (prediction 5)

PyBaMM DFN 40/20/40 (not 160/80/160, for cost), `"lithium plating": "irreversible"`, OKane2022's
constants. "Plated" is `"Loss of capacity to negative lithium plating [A.h]"` at the cut-off, as a
share of the 5.153 A·h capacity; "before crossing" is the share of it plated before the potential
first went below 0 V; "later" is how much later 4.2 V came than with plating off.

| condition | plated, % of capacity | before crossing | later |
| --- | --- | --- | --- |
| 25 °C 0.3C | **1.73** | all (never crosses; lowest +40.3 mV) | — |
| 25 °C 0.7C | 0.90 | all (never crosses; lowest +0.7 mV) | +0.6 % |
| 45 °C 0.7C | 0.76 | all (never crosses; lowest +31.2 mV) | — |
| 25 °C 1C | 0.72 | 61 % | +0.4 % |
| 0 °C 0.3C | 2.90 | 85 % | +1.7 % |
| 0 °C 1C | 1.44 | 2.5 % | +1.8 % |
| 0 °C 2C | 0.95 | 0.06 % | +7.4 % |
| −10 °C 1C | 1.85 | 0.4 % | +4.2 % |
| −10 °C 2C | 0.58 | 0.07 % | +14.4 % |

With `"reversible"` instead: 0.50 % at 25 °C 0.3C (never crossing), 1.31 % at 0 °C 1C. In that form
the figure is lithium sitting as metal when the charge stops, not necessarily lost: it can strip back
on a later rest or discharge, which these runs do not include.

**The rate constant cannot fix it** (added after the table, on review). The same runs with OKane2022's
constant cut 100-fold, to 1e-11 m/s:

| condition | the sheet | plated at 1e-9 | plated at 1e-11 | ratio |
| --- | --- | --- | --- | --- |
| 25 °C 0.3C | allows | 1.73 % | 0.0181 % | ÷96 |
| 0 °C 0.3C | allows | 2.90 % | 0.0318 % | ÷91 |
| 0 °C 2C | forbids | 0.95 % | 0.0107 % | ÷88 |
| −10 °C 1C | forbids | 1.85 % | 0.0212 % | ÷87 |

Every row falls by nearly the same factor, so the order is the constant's to scale and not to change:
the gentle charge the sheet allows at 0 °C plates more than the 1C charge it forbids at −10 °C, at any
constant. In this form nothing makes 0 V special — the plating current goes as `k·exp(−α·F·η/RT)`, and
`η = 0` is just a point on a smooth exponential. The rows differ by less than 2× while the potentials
differ by more than 200 mV, because time spent charging counts for as much as how far past the line the
graphite goes.

**These constants plate on every charge, at any potential.** OKane2022's plating rate goes as
`exp(−α·F·η/RT)` with no stripping to oppose it while no lithium is plated, so it never stops above
0 V; at 25 °C 0.3C it is a steady ~29 mA, and a gentle charge the maker allows costs 1.7 % of
capacity — a cell that would be at 80 % in about a dozen charges. Slow charges plate more than fast
ones because they last longer. That is not this cell, and it is what the set warns of when it says
it "does not claim to be representative".

### Against the predictions

1. **Failed, in the cautious direction.** The physics reaches the line everywhere predicted and also
   where predicted not to: 0 °C 1C goes to −90 mV, and **25 °C 1C crosses too** (−18 mV).
2. **Held on the order, failed on the absolute.** The `Spm` is the optimist at every condition, by 60
   to 210 mV in the cold, and crosses far later in the charge (at 0 °C 1C: 55 % charged against
   15 %). But it does cross at all four cold conditions, including 0 °C 1C, where it was predicted
   not to.
3. **Held on the matched grid; failed on the coarse-grid bound.** On 40/20/40 the engine's lowest is
   within 1.1 mV of PyBaMM's and the crossing within 0.001 of charge state. On 10/5/10 against the
   converged reference it reads high by 4 to 38 mV, past the registered 20 at 0 °C 2C (21.2) and
   −10 °C 2C (38.3: −220.7 against −259.0), and stops early at −10 °C 2C (6.5 % charged against 7.7 %).
4. **Held** (above).
5. **Failed.** Plated lithium reaches 1.9 % of capacity in one cold charge and 2.9 % in a gentle one;
   4.2 V comes up to 14 % later; and plating is not negligible above 0 V — it is most of it on every
   charge at 1C or slower.
6. **Held, and worse than predicted.** Today's threshold misses both 0 °C conditions, 90 and 210 mV
   past the line in the converged reference, and the −10 °C 0.3C charge the maker forbids.
7. **Not needed.** The `Dfn` converged on every step, at −10 °C 2C too.

## What A1 settles

* **The engine has what plating needs, without new physics.** Both porous models put the graphite
  below lithium's potential on charges that should plate, the `Dfn` agrees with PyBaMM to about a
  millivolt on a matched grid, and the line falls where the maker's limits are consistent with it.
* **The `Spm` sees the danger late and small.** Its lowest is a quarter or less of the `Dfn`'s in the
  cold, and on the maker's cold-edge charges it reads 6 to 19 mV *above* the line where the `Dfn`
  reads below. It has no electrolyte, so it misses the drop that pushes the separator side over. That
  answers half of owner decision 2: plating on the `Spm` alone would teach that a 1C charge at 0 °C is
  nearly safe.
* **The recommended `Dfn` grid understates it in the cold** (by 38 mV at −10 °C 2C); the fine grid does
  not.
* **The only plating law to hand cannot be used, whatever its constant.** In PyBaMM's form with
  OKane2022's numbers, a reaction that ought to be a consequence of crossing the line is a steady drain
  on every charge, and the 100-fold test shows the constant only scales it: no value makes a charge the
  sheet allows cheap while one it forbids stays costly. So A2 is two questions, not one: a source for
  the constant, **and which law** — and the law turns on owner decision 1 (whether plated lithium can
  dissolve back). Only a form whose plating stops above 0 V gives 0 V a meaning, and PyBaMM's own
  `"reversible"` is not one (0.50 % at 25 °C 0.3C, never crossing): its stripping rate is proportional
  to the lithium already plated, which starts at zero, so nothing opposes the first plating.

The harness (`W:\temp\claude\phase12-spike\`: `engine\` the Rust reader, `ref\cold_charge.py` and its
two variants the PyBaMM side) is outside the tree and is deleted once the phase plan rebuilds what it
needs. **Every figure above comes from each run's printed summary line, not from a saved trace.** A
quoting slip in the run loops sent the per-step CSVs of the engine runs and of the main PyBaMM batch to
one misnamed file per condition, each run overwriting the last, and those files were deleted; what
survives in `out\` is the main batch's summary log (`pybamm_log.txt`) and the per-step CSVs of the
reversible and 100-fold runs. Any trace a plan needs is regenerated from the sources.

## A1b — does the chosen law separate what the sheet allows from what it forbids?

*Predictions registered 2026-10-18, before the run.*

The owner chose plating only past the line (A2 below) on the promise that the sheet's allowed charges
come out nearly free and its forbidden ones costly. PyBaMM's form failed exactly that test, so the
chosen one is measured before any engine code. No engine change: the A1 runs are repeated, saving
every step's potential, and the plated lithium is integrated outside the engine.

* **The law.** Plating current density `i_pl = i0·[exp(−α·f·η) − exp((1 − α)·f·η)]`, `f = F/RT`,
  `η = φ_s − φ_e` (no film), `α = 0.65` and `i0 = F·k·c_e` — OKane2022's transfer coefficient and
  exchange-current form, **with no temperature dependence in `k`** (OKane2022 has none; the only
  temperature in the law is `f`'s). Positive `i_pl` plates; negative strips, and only while lithium is
  plated, so a fresh graphite above 0 V plates nothing.
* **Read open-loop.** The potential is the plating-free model's; plating's feedback on it is ignored,
  which holds while plating takes a small share of the current — what "nearly free" asks for anyway.
* **Summed over the electrode.** On the `Dfn` 40/20/40, every negative node, by its interfacial area
  and its own `c_e`; on the `Spm`, the one particle at the nominal `c_e`.
* **Reported** as plated lithium per unit `k`, and the ratio of the mildest forbidden charge to the
  worst allowed one. That ratio is the whole sizing problem: the placeholder `k` can put the allowed
  charges at any cost, and the ratio says what the forbidden ones then cost.

Allowed: 25 °C 0.3C, 45 °C 0.7C, 0 °C 0.3C, 25 °C 0.7C. Forbidden: −10 °C 0.3C, 0 °C 1C and 2C, −10 °C
1C and 2C.

Predictions:

8. The two warm allowed charges plate exactly nothing on either model (they never cross).
9. On the `Dfn`, the worst allowed charge is 0 °C 0.3C, and the mildest forbidden one is −10 °C 0.3C;
   the ratio between them is between 3 and 30.
10. On the `Spm`, every allowed charge plates exactly nothing, and every forbidden one plates something.

### A1b results (run 2026-10-18)

Plated lithium per unit rate constant at the 4.2 V cut-off, in A·h per (m/s), and as a share of the
5.153 A·h capacity at a reference `k = 1e-12` m/s (it scales linearly with `k` while plating stays small).
Per-step traces in `W:\temp\claude\phase12-spike\out\a1b_*.csv` (column `plated_ah_per_k`).

| charge | the sheet | `Dfn` 40/20/40, A·h per (m/s) | `Dfn`, % at 1e-12 | `Spm`, A·h per (m/s) | `Spm`, % at 1e-12 |
| --- | --- | --- | --- | --- | --- |
| 25 °C 0.3C | allows | 0 | 0 | 0 | 0 |
| 45 °C 0.7C | allows | 0 | 0 | 0 | 0 |
| 25 °C 0.7C | allows | 4.74e3 | 9.2e-8 | 0 | 0 |
| 0 °C 0.3C | allows | **1.66e6** | 3.2e-5 | 0 | 0 |
| −10 °C 2C | forbids | **3.00e7** | 5.8e-4 | 8.28e7 | 1.6e-3 |
| −10 °C 0.3C | forbids | 3.01e7 | 5.8e-4 | 4.37e7 | 8.5e-4 |
| 0 °C 2C | forbids | 5.52e7 | 1.1e-3 | 4.53e7 | 8.8e-4 |
| 0 °C 1C | forbids | 5.56e7 | 1.1e-3 | 8.20e6 | 1.6e-4 |
| −10 °C 1C | forbids | 1.00e8 | 1.9e-3 | 8.87e7 | 1.7e-3 |

* **8. Held.** The warm allowed charges plate exactly nothing on either model.
* **9. Held on the ratio, missed on which case.** On the `Dfn` the worst allowed charge is 0 °C 0.3C, and
  the mildest forbidden is −10 °C 2C by 0.1 % over −10 °C 0.3C (predicted the other way round — a tie).
  **The ratio is 18**, inside the registered 3 to 30; the worst forbidden is 61 times the worst allowed.
* **10. Held.** The `Spm` plates nothing on any allowed charge and something on every forbidden one —
  which is also why it cannot be sized: on the `Spm` every `k` makes the allowed charges free.

**The chosen law does what it was chosen for**, by a factor of 18 at the boundary. Sized so that the
coldest allowed charge plates, say, 0.002 % of capacity (`k ≈ 6e-11` m/s, 16 times under OKane2022's),
the forbidden ones plate 0.035 to 0.12 % per charge, before the lost share is applied.

Two caveats the plan has to carry:

* **No constant-voltage leg.** These charges stop at 4.2 V, which a cold fast charge reaches early (−10 °C
  2C at 7.6 % charged), so a faster charge can plate *less* per charge than a slower one (−10 °C: 2C
  plates under a third of 1C). A real charger holds 4.2 V afterwards and keeps plating; the ranking
  between forbidden charges is the protocol's, not the cell's.
* **Open loop.** At these sizes plating takes well under 1 % of the current, so its feedback on the
  potential is small, as assumed.

## A3 — does the `Dfn`'s solve survive the reaction?

*Predictions registered 2026-10-18, before the prototype ran.*

A throwaway prototype in a detached worktree (`W:\temp\claude\phase12-spike\wt`), never merged: the
A1b law inside the `Dfn`'s Newton solve, as a second reaction at each negative node sharing that node's
`φ_s − φ_e` with intercalation. Plating current enters the electrolyte's mass and charge rows and the
solid's charge row beside the intercalation current; the particle sees intercalation alone. Stripping is
capped at the node's plated inventory over the step (a kink, as the reversal's edge is). Plated lithium
per node is state (`#[serde(default)]`, no version bump); a fixed share of each step's plating, 0.5 here,
is dead and cannot strip. `k = 6e-11` m/s (A1b's sizing), `α = 0.65`, no film. Heat is left as the
engine books it, so the energy ledger measures the hole the plan has to close.

The runs, on the shipped LG M50 at 40/20/40 with 40 shells:

* **The curve.** At a state just past onset (0 °C, 1C from empty for 600 s), the end-of-step voltage at
  41 trial currents from 0 to 2C charge, from a restored snapshot, at `dt` of 1 s, 15 min and 1 h.
* **The pack.** A 1S2P pack, one cell at 0.9 capacity, at 0 °C: a 1C charge (per cell) from empty to
  4.2 V under a current demand, then a hold at 4.2 V for an hour, at steps of 1 s, 60 s, 15 min and 1 h.
* **The rest.** After that charge, one 1-hour rest step, then 24 more.
* **The ledgers.** Charge: the charge passed against intercalated + plated + dead lithium. Energy: the
  electrodes' stored energy (plated lithium at lithium's own potential, 0 V) against the electrical
  energy and the booked heat.

Predictions:

11. **The curve still falls with current** at every trial and every step length.
12. **The pack converges**: no `SOLVE_UNCONVERGED` at 1 s, 60 s and 15 min, on the charge or the hold; at
    1 h at most a few steps.
13. **The plated amount depends on the step** as the voltage does: the 1C charge's plated lithium at 60 s
    within 2 % of 1 s, at 15 min within 10 %, at 1 h within 30 %.
14. **The rest strips back** most of the reversible plated lithium within a day at 0 °C, and a single
    1-hour step never takes a node's inventory below zero.
15. **The charge ledger closes** to rounding (1e-9 of the charge passed). **The energy ledger does not**:
    its hole equals the plating current's heat misbooked at the cell's equilibrium voltage instead of the
    plating reaction's, `∫ i_pl·U_n dt` (U_n the negative's potential, about 0.1 V), within 10 %.

### A3 results (run 2026-10-18)

Every run twice: the prototype, and the shipped engine as a control, from one harness source
(`W:\temp\claude\phase12-spike\proto\src\main.rs`, built against the worktree and against the tree).

**The curve (prediction 11: held).** Between any two neighbouring trial currents whose solves both
converged, the end-of-step voltage falls with current — at 1 s, 15 min and 1 h, at the state past onset
and at a state with lithium plated (whose sweep crosses the stripping cap), on the prototype and on the
control alike. What fails is a solve, not the curve: trials that would overfill the negative within the
step do not converge, 8 of 41 at 15 min and 22 at 1 h on the control, **6 and 16 on the prototype** —
plating gives the current somewhere to go once the graphite's surface is full.

**The pack (prediction 12: held for the prototype; the control fails).** 1S2P, one cell at 0.9 capacity,
0 °C, 1C per cell from empty, then an hour at 4.2 V, then rest:

| step | prototype: unconverged steps | control: unconverged steps |
| --- | --- | --- |
| 1 s, 60 s, 15 min | 0 | 0 |
| 1 h | 1 (in the hold) | **every step: the charge never moves the cells off 0 %, the hold's current is NaN, and all 25 rest steps fail** |

**That last row is a defect in the shipped engine, found by the control, not by plating.** Narrowed on
review, shipped engine, 1 h steps, 1C per cell from empty, isothermal:

| case | result |
| --- | --- |
| one cell, 0 °C or 25 °C | converges (and overcharges, as a current demand may) |
| 1S2P, equal cells, 0 °C | converges |
| 1S2P, one cell at 0.9, **0 °C or 25 °C** | **every step unconverged**; the split it commits is lopsided (34 % and 17 % charged after the first hour); the **third step**, which takes the small cell past full, commits **NaN** into both cells' `i_last`, `soc_deficit` and `tangent` — from then on the voltage is NaN and the state of charge reads 0 on every step |
| the same at 0.5C per cell, or at 15 min steps | converges |

So it is not the cold and not one cell's solve: it is the pack's split of a mismatched `Dfn` pair at a long
step — the pass loop does not settle it, and the bracketed search that settles such a split on the other
models is "not on `Dfn` packs" (ROADMAP H8, `porous-reversal.md`). That gap is recorded; **what is not
recorded is that a failed split commits NaN into the cells' state and the pack never recovers** — a
`step()` that leaves a non-finite number in the snapshot. The prototype gets through only because plating
softens the curve where the graphite fills; that is luck, not a fix.

**The rest (prediction 14: held).** The reversible plated lithium strips back completely — within the
first hour's rest at steps up to 15 min, within 25 h at 1 h steps — and no node's inventory ever goes
below zero (smallest 0 to the last bit), so the cap holds over a whole hour.

**The ledgers (prediction 15: charge held; energy failed as registered — 10.4 % against 10 % — and not
resolved: consistent with the predicted hole, too small to measure against the control's own).** 1S1P, 1C for
1500 s then 600 s rest, 1 s steps. Charge: the positive's lost lithium and the negative's + plated + dead
gain both match the charge passed to 6.3e-8 — identical to the control, so that is the harness's own
reading, not plating. Energy: the control's ledger is off by −4.96 J of 29.8 kJ (the long-step heat
already recorded for this model); the prototype's by −5.45 J. The extra **−0.49 J** is plating's, against
the predicted misbooking of the plating path's heat at the cell's equilibrium voltage instead of the
plating reaction's, `∫ i_pl·h_n dt` = 0.45 J — 10.4 % apart. But the signal is a tenth of the control's own residual, and
it is a difference between two trajectories' residuals, so part of it may be the control's step-length
heat error moving with the trajectory: consistent, not resolved. Small here because plating is small; it
grows with it, and the plan closes it by booking plating's heat against its own reaction (a run at a
100-fold rate would resolve it; not done).

**The step length (prediction 13: failed, badly).** The pack run hinted it; a clean run settled it —
the same charge at several steps, 1S1P, plated lithium as a multiple of the 1 s run's:

| charge | plated at 1 s | 10 s | 60 s | 5 min | 15 min | 30 min | 1 h |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1C for 30 min | 1.23e-4 mol, 0.064 % of capacity | ×1.009 | ×1.060 | ×1.30 | ×2.03 | ×3.60 | — |
| 0.5C for 1 h | 4.8e-8 mol, 2.5e-5 % | — | ×1.51 | — | ×8.3 | — | ×45 |

"Plated" is everything plated over the run, dead and reversible. The 0.5C row barely crosses the line
(its 1 s total is 2,600 times smaller), where any ratio explodes; **the 1C row is the one to size the plan's
bar on.**

The voltage and the charge state hardly move with the step (state of charge 0.4994 at 1 s, 0.4977 at
30 min); the plated amount does, because a backward-Euler step books the **whole step at its end
potential**, and on a charge that is the step's lowest. Plating is exponential in that potential, so the
error is first order and large. **The plan needs a step-length treatment for the plating integral of its
own** — sub-stepping it, or integrating the rate along the step — with a bar on this table.

## What A3 settles

* The solve survives the reaction: no new non-convergence at the steps the pack runs at, the curve still
  falls, the stripping cap holds over an hour, and lithium is conserved.
* Two things the plan must carry that the roadmap did not list: **(1)** the plating integral's own
  step-length error, and **(2)** the shipped engine's parallel-`Dfn` failure at one-hour steps, which the
  plating phase does not cause but would make visible — a parallel `Dfn` pack is where its lesson runs.
* A4 (LTO plates nothing) cannot run: `lto_20ah_generic.toml` has no `[spm]`, so it has no porous model to
  plate on, and keeps its "no gate" as an equivalent circuit. Under the chosen law a negative electrode
  1.55 V above lithium would need 1.55 V of polarization to plate; the plan's check is analytic instead.

## Stop-point A2

Asked of the owner on 2026-10-17, before any plan is written:

1. **The law.** Plating only past the line (a reaction that can run backwards, so lithium plated past 0 V
   strips again above it, and nothing plates on a fresh graphite above 0 V — a form PyBaMM does not
   ship: its `"partially reversible"` shares `"reversible"`'s stripping term, `plating.py` lines 90–93
   in 26.6.2.0, so this would have no reference to be checked against), or PyBaMM's forms as they are, with the
   steady drain recorded as a limit. This is owner decision 1 brought forward.
   **Answered 2026-10-17: plating only past the line** (option a), with no outside reference for the
   form accepted as its cost.
2. **The constant.** Any source for this cell's plating rate — a paper, a dataset, a measured cold-charge
   fade — or a placeholder, labelled so, sized against the sheet: nearly free at its allowed rates, costly
   past them.
   **Answered 2026-10-18: no source; a placeholder, labelled, sized against the sheet.**
3. **How much plated lithium is lost for good** (raised by answer 1: if all of it dissolves back, a
   cold charge costs nothing). A fixed share lost, the rest free to strip back; or a share that grows
   with time spent plated (O'Kane's, a second unsourced number); or all of it.
   **Answered 2026-10-18: a fixed share lost**, one labelled number.
4. **Which model** (the roadmap's owner decision 2), **the threshold's fate** (3) and **the dendrite short**
   (4), asked with A1b's numbers. **Answered 2026-10-18, as recommended:** plating on the **`Dfn` only** —
   the `Spm` plates nothing on any allowed charge, so it cannot be sized, and it keeps the threshold; the
   equivalent circuit keeps the threshold; on a `Dfn` cell with plating on (opt-in, as the film is)
   `PLATING_RISK` means "plating current above zero" and the loss is the lithium actually lost; the dendrite
   short keeps its seeded roll, its hazard tied to the lithium lost to plating.
