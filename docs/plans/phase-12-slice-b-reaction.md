# Phase 12 slice B — plating as a reaction inside the `Dfn`

**Status: BUILT 2026-10-18; two of the plan's exits missed, awaiting the owner** (see "Still
open"). Predictions registered before any engine code (5882a72), scored below:
the reaction is in the `Dfn`, plating off is bit-identical, the plating integral meets its
step-length bar with 5 s sub-steps, and exits 1, 2, 4 and 6 pass. **Three registered bars missed,
each with its cause measured**: the hold leg of the step-length bar (the long-step voltage hold's
one-current-per-step profile, not the plating integral), the small-`k` comparison against slice A's
recorded open loop at 2C (a defect in slice A's instrument, now fixed), and exit 5 at one-hour steps
(the pack's voltage hold on the mismatched pair, worse with plating off). Plan:
`phase-12-plating.md` §B; the constant and the reference: `phase-12-slice-a-reference.md`.
Snapshot 28 → 29. Tests: `crates/sim-data/tests/plating_dfn.rs`.

## What is built

* **Data.** `[dfn.plating]` in `chemistries/nmc_21700_lgm50.toml`, slice A's draft: `rate_constant_m_per_s
  = 4.8e-11` (placeholder, sized on the fine grid), `transfer_coefficient = 0.65` (OKane2022),
  `dead_share = 0.5` (placeholder). An `Option` appended to `DfnParams`, validated, unknown keys refused.
  `lgm50_provenance.rs`'s rule is amended to "may grow only by a phase plan that names the key and the
  owner decision behind it", and the list gains the two placeholders, naming `phase-12-plating.md`.
* **The selector**, appended to `AgingConfig` with `serde(default)`: `plating = Threshold` (today's,
  the default) or `Reaction`. `Pack::new` refuses `Reaction` on the equivalent circuit, the `Spm` and
  the many-particle cell, and on a chemistry with no `[dfn.plating]`.
* **The state**, appended to `DfnState`: an `Option` holding reversible and dead plated lithium per
  negative node \[mol per m² of particle interface\]. `None` on every cell whose pack does not select
  the reaction.
* **The reaction**, per negative node, sharing the node's `φ_s − φ_e` with intercalation:
  `J = i0·[exp(−α·f·η) − exp((1 − α)·f·η)]`, `i0 = F·k·c_e` (the floored `c_e` the kinetics read),
  positive plates, as slice A wrote it; in the solve's discharge-positive sign, `j_pl = −J`, capped
  where it strips at the node's reversible inventory over the step, `F·q/dt`. It enters the
  electrolyte's mass and charge rows and the solid's charge row; the particle sees intercalation alone.
  The inventory is booked only in `advance`, from the converged field — never by a probe.
* **The film.** Where the Phase 10 film is on, plating reads the same `φ_s − φ_e − j·r_film` the
  intercalation reaction does: the film's drop made by the **intercalation** current. Plating's own
  current through the film is left out, which keeps `j_pl` an explicit function of the node's unknowns
  (counting it would make `j_pl` depend on itself and want a fifth unknown per node). The error is
  `j_pl·r_film`, bounded by the share of the current plating carries — at most 2.6 % on any charge
  slice A ran — of a drop that is zero on a fresh cell (the film's resistance is its growth's only).
  The combination is covered by the Jacobian check, not by a reference.
* **Heat.** The plating current's share is booked against its own reaction, lithium metal at 0 V,
  not the cell's equilibrium voltage: a term beside the reversal's, `I_pl·U_n` at the negative's bulk
  stored-energy potential (`I_pl` the cell's plating current, discharge-positive).
* **Slice C's, not this slice's:** health (`soh_capacity`, what `soc_true` reads while lithium sits as
  metal), `PLATING_RISK`, the threshold's `plating_fade_per_ah` and the dendrite short. **Until slice C
  the threshold still runs on a reaction cell**, beside the reaction: its flag and its fade are
  untouched. No test in this slice pins any of them.

## The step-length treatment — the two candidates

1. **Sub-steps.** A step with plating on is cut into `ceil(dt / PLATING_SUBSTEP_S)` backward-Euler
   sub-steps at the step's current, each a full solve, committed in turn; the count depends on `dt`
   alone (Phase 9's rule). The pack sees one end-of-step curve per cell as before. Its tangent is the
   **composite's**, `dV_end/di` through every sub-step, carried by a forward sensitivity: each
   sub-step's start state (electrolyte, particle profiles, plated inventory) moves with `i` through the
   last, and its factorised Jacobian takes that in one more back-substitution. A difference quotient
   of a chain of Newton solves would read the solver's tolerance as slope.
2. **A quadrature of `η`.** One solve, its plating current the exact mean of `J` along `η` linear in
   time from the stored start-of-step value (the last solve's) to the end-of-step unknown.

Candidate 2 costs nothing extra and is first in line on cost. It is expected to fail where a step
starts from a different current — its first instant is the previous current's potential, as the
`Dfn`'s heat trapezoid was (`dfn-end-of-step-heat.md`) — and the bar's 30-minute case is one step
from rest.

## Predictions

The bar charge is the spike's: 1S1P, shipped LG M50 at 40/20/40 with 40 shells, isothermal at 0 °C,
from empty, 1C charge for 30 min; and the same charge to 4.2 V followed by a 4.2 V hold to C/20.
"Plated" is gross (everything the plating branch put down, `dead / dead_share`).

1. **Uncorrected, the bar fails** as the spike measured (×1.06 at 60 s, ×2.0 at 15 min, ×3.6 at 30 min).
2. **The quadrature fails the 30-minute bar by under-booking**: below 0.8× of the 1 s figure.
3. **Sub-steps meet the bar** — plated within 1 % at 60 s, 10 % at 15 min, 20 % at 30 min, of the 1 s
   figure — on both legs. `PLATING_SUBSTEP_S` is chosen from {2, 5, 10} s as the longest that keeps
   every bar at half its width or better (the spike's 10 s *steps* were ×1.009, too near 1 %; 5 s is
   expected).
4. **The composite tangent is the curve's slope**: within 1e-4 relative of a central difference taken
   at ±1e-3 of the capacity in amperes, at 15 min and 1 h steps.
5. **The curve still falls with current** at 1 s, 15 min and 1 h — at the spike's state past onset and
   at a state holding plated lithium — between every two converged neighbouring trials.
6. **The Jacobian matches a central difference** (`probe::jacobian_pair`) to 1e-6 relative on every
   entry above 1e-8 of the largest, in four states: plating live; the stripping cap binding; plated
   lithium above zero with `η > 0` and the cap not binding; and plating with a grown film.
7. **Exit 2, against the reference.** At the sized `k`, 1 s steps, CC–CV, fine grid: the engine's
   gross on every **forbidden** charge within **5 %** of PyBaMM's closed loop (slice A's column),
   expected within 3 %. The allowed cold charge is **recorded, not judged** — slice A put 15 % between
   the models' potentials there, not the law. And against the engine's own open loop at `k/100`, ×100:
   within **0.2 %** on every charge that plates (closed loop at small `k` is the open loop plus `O(k)`
   feedback, and both book at the end-of-step potential at 1 s).
8. **Exit 1, on the engine** (fine grid, 1 s steps, CC–CV): every allowed charge plates under the
   0.002 % target gross (the cold one ≤ 0.00197 %, slightly under the open loop's figure, as plating's
   feedback lowers the potential's excursion); the warm ones exactly nothing; every forbidden one at
   least 10× the worst allowed (expected ~24×).
9. **The lithium ledger closes**: lithium leaving the positive's solid equals the negative solid's gain
   plus reversible plus dead, to 1e-9 of the charge passed, at 1 s and at 15 min.
10. **The energy ledger, at `k` × 100**, 1S1P, 0 °C, 1C charge then a rest, 1 s steps: with plating's
    heat booked against its own reaction, the residual is within 2× the control's (the same protocol,
    plating off); with that term removed it is more than 10× the control's.
11. **Exit 5**: the 1S2P pack, one cell at 0.9 capacity, 0 °C, fine grid, plating on: 1C per cell until
    the terminal reaches 4.2 V, then 4.2 V held for 3 h. At 15 min steps every step converges. At 1 h
    steps every step is finite, and at most **2** steps are unconverged, all on the constant-current
    leg — that leg's split does not settle at an hour with plating off either (`dfn-split-nan.md`); if
    it fails there, the failure is the split's, and it is recorded so.
12. **Plating off is bit-identical.** Every golden and every existing test passes unchanged; and a hash
    harness (old tree against new, every `Telemetry` frame and `CellView` of a run on each cell model,
    plus the final snapshot as JSON with its version and this slice's new keys stripped) hashes alike,
    with a 1e-15 perturbation of the initial SOC shown to move every hash. Raw snapshot bytes are
    expected to differ — every `AgingConfig` gains a tag, every `[dfn]` an `Option` tag — so they are
    not the test.
13. **The 28 → 29 pair test**: a v28-shaped blob fails at v29, at the field (`snapshot_version.rs`).
14. **Cost**: at steps no longer than the sub-step, a plating step is one solve, within 20 % of a
    plating-off step; a 15 min step costs about `900 / PLATING_SUBSTEP_S` solves.

## Results (run 2026-10-18)

All on the shipped LG M50 file with its drawn fade and its threshold's dendrite hazard zeroed in the
test (aging has to be on for the selector; nothing but the reaction then moves the cell), fine grid
unless said. Release builds. The instruments are `plating_dfn.rs`'s tests and its `#[ignore]`d
`report_*` functions.

### The step length, and which treatment

Gross plated on the bar's charge, as a multiple of the 1 s figure (`report_step_length`), for the
candidates as built — sub-steps of 2, 5 and 10 s (each a build with that constant, out of tree), and
the quadrature (one solve, the law's exact mean along `η` linear from the stored start):

| charge | treatment | 10 s | 60 s | 5 min | 15 min | 30 min |
| --- | --- | --- | --- | --- | --- | --- |
| 1C 30 min | sub-steps 2 s | ×1.0010 | ×1.0010 | ×1.0010 | ×1.0010 | ×1.0010 |
| | **sub-steps 5 s** | **×1.0040** | **×1.0040** | **×1.0040** | **×1.0040** | **×1.0040** |
| | sub-steps 10 s | ×1.0091 | ×1.0091 | ×1.0091 | ×1.0091 | ×1.0091 |
| | quadrature | ×0.9996 | ×0.9963 | ×0.9542 | ×0.8038 | **×0.000** |
| then 4.2 V 2 h | sub-steps 5 s | ×0.9997 | ×0.9849 | ×0.8971 | ×0.7123 | ×0.5377 |
| | sub-steps 2 s / 10 s | ×0.9983 / 1.0022 | ×0.9834 / 0.9876 | ×0.8952 / 0.9001 | ×0.7107 / 0.7152 | ×0.5362 / 0.5402 |
| | quadrature | ×1.0033 | ×1.0215 | ×1.0964 | ×1.2565 | ×1.2051 |

No step of any run was unconverged. The 1 s figure is 2.639e-3 A·h on the charge and 6.012e-3 A·h
with the hold (the quadrature's own 1 s figures differ in the fourth digit).

* **Prediction 1** was not re-run: the spike's table (×1.06, ×2.03, ×3.60) is the uncorrected figure.
* **Prediction 2, held.** The quadrature books *nothing* on a single 30-minute step from rest — its
  first instant is the rest potential, 0.2 V above the line, and the exponential's mean along a
  straight path from there never reaches the plating the real path does — and 0.80× at 15 min; with
  the hold it over-books by a quarter. Refused.
* **Prediction 3, held on the charge, missed on the hold.** Sub-steps hold the charge within 0.4 %
  (5 s), 0.1 % (2 s) and 0.9 % (10 s) at every step length — the integral's step-length error is now
  the sub-step's, independent of `dt`. **`PLATING_SUBSTEP_S = 5 s`** by the registered rule (the
  longest of the three within half of every bar on the charge: 10 s's 0.91 % is not). The hold leg
  misses every bar past 10 s, by the same amount in every variant — so it is not the integral.
  `report_hold_replay` says what it is: each long-step hold's own currents, replayed as a current
  demand at 1 s steps, plate what the long step plated to **0.31–0.38 %** at 60 s, 5, 15 and 30 min.
  A voltage hold at a long step is one constant current that ends the step at 4.2 V; the 1 s hold
  starts high and tapers, and plating is where the current is high (the long-step hold also charges
  2.5 % less at 30 min). A cell-level sub-step runs at the current the pack chose and cannot see a
  profile the step does not have. **What this leaves:** a cold voltage hold at steps past a minute
  under-books plating by the hold's own discretisation, 1.5 % at 60 s and 29 % at 15 min. Resolving
  it would mean sub-stepping the *pack's* demand, not the cell's integral; not in this slice.

### The reaction against its law and its references

* **Prediction 6, held**: the Jacobian's plating terms match a difference quotient to 1.86e-9
  (plating), 1.62e-9 (cap binding at 1 h), 1.62e-9 (stripping below the cap) and 1.86e-9 (a grown,
  uneven film), worst row-relative.
* **Prediction 4, held**: the stored tangent of a sub-stepped step is the whole step's slope — 3.1e-5
  and −2.5e-5 from a central difference at 15 min and 1 h.
* **Prediction 5, held**: no rise between converged neighbours in any sweep (11 trials, 2C charge to
  rest, at 1 s, 15 min and 1 h, past onset and with lithium plated). Unconverged trials: 0 and 0 at
  1 s, 0 and 2 at 15 min, 5 and 7 at 1 h — the ones that overfill the graphite within the step, as
  the spike found on the control.
* **Prediction 9, held**: the positive's lost lithium equals the charge passed, and the graphite's
  gain plus everything plated equals it, to the tenth digit printed, at 1 s and at 60 s (a cold 2C
  charge from 60 % on the recommended grid and a 600 s rest; 36 % of the charge plated, half of it
  dead).
* **Prediction 8 (exit 1), held**: allowed charges gross 0 (25 °C 0.3C), 0 (45 °C 0.7C), 6e-6 %
  (25 °C 0.7C) and **0.001961 %** (0 °C 0.3C, under the 0.002 % target; the open loop's 0.00197 %);
  the mildest forbidden (−10 °C 0.3C, 0.0478 %) is **24.4×** the worst allowed. No step unconverged.
* **Prediction 7 (exit 2) against PyBaMM, held**: engine ÷ PyBaMM closed loop on the forbidden
  charges 0.9765 (−10 °C 0.3C), 0.9887 (0 °C 1C), 0.9905 (0 °C 2C), 0.9907 (−10 °C 1C), 0.9847
  (−10 °C 2C) — within 2.4 % against the 5 % bar. The allowed cold charge, recorded: 0.864.
* **Prediction 7 against the open loop at `k/100`, missed as registered — on slice A's instrument.**
  At 0 °C 2C the closed loop ×100 came to 0.9955 of slice A's recorded figure, outside 0.2 %.
  Integrated on the run's *own* states, the 1C charges agreed to 1.00000 and the 2C ones did not
  (0.9958, 0.9840), and the parting began on the step at which the electrolyte at the graphite went
  **negative** (−1.2e-3 mol/m³, falling to −0.02). The solve reads `c_e` through its lookup floor;
  slice A's open-loop instrument read it raw, so the law's sign flipped with it and a node above 0 V
  counted as plating. With the floor in the instrument, closed and open loop agree to **1.0000000
  on all seven charges that plate**, and that is what is pinned. `plating_open_loop.rs` is fixed and
  slice A's two 2C rows are corrected in its note (addendum); against the corrected table every
  charge is within 0.12 %. The sizing charge did not move.

### Heat, convergence, bits and cost

* **Prediction 10, half held**: 1C 1500 s then 600 s rest at 1 s steps. Control (plating off)
  residual −4.95 J. At `k` × 100 (gross 1.69 % of capacity) the residual with plating's heat booked
  against its own reaction is **−4.00 J** — within 2× — but plating's heat is only 28.8 J, so
  without the term the residual is −32.8 J, **6.6×** the control's, not the 10× registered. At
  `k` × 1000 (5.07 %, run after): −3.01 J with the term, −97.2 J without, 19.6×. Both are pinned.
* **Prediction 11 (exit 5), held at 15 min, missed at 1 h.** 15 min: 14 steps, none unconverged. 1 h:
  every value finite, the charge step converged, and **all three hold steps unconverged**, 1.7, 0.35
  and 0.03 mV short of 4.2 V — against "at most 2, all on the charge leg". Re-run as current demands
  at the currents the hold found, every step converges, so it is the pack's voltage solve. From the
  same state with the reaction switched off in the snapshot the hold misses by far more (4.33 V at
  zero current, then 4.12 and 4.08 V), and the pair run with plating off from the start fails every
  step and goes nonphysical by its fifth hour (−14.5 V, −67 A). So the miss is the mismatched pair's
  one-hour split (`dfn-split-nan.md`, ROADMAP H8), which the plan said beforehand would be the
  split's to fix; with plating on the pair is better behaved than without it. Pinned as measured.
* **Prediction 12, held**: every golden and existing test unchanged; six packs (ECM, `Spm` with the
  film, `Dfn` with aging and a thermal network, `Dfn` with the film, `Dfn` with no aging, the
  many-particle cell; charge, hold, rest, discharge at 1 s to 15 min steps) hash identically on the
  parent and the slice, and a 1e-15 nudge of the initial SOC moves all six.
* **Prediction 13, held**: `the_version_field_is_what_rejects_a_v28_snapshot` and the two
  `a_v28_shaped_*` tests; the v25 and v26 tests re-read for the appended fields.
* **Prediction 14, held on the bar**: a 1 s plating step costs 5.4 % more than a plating-off one
  (927 against 879 µs, best of six). A 15 min step is 180 sub-steps and costs 237 ms against 12 ms
  for one solve — 20×, not 180×: a 5 s solve converges in fewer iterations than a 15-minute one.

### Found on the way

* **The open-loop instrument's sign flip** (above): a check reading the electrolyte raw where the
  model reads it floored is measuring a different law exactly where the electrolyte runs out.
* **The `Dfn`'s electrolyte goes negative at the graphite on a 2C cold charge** (−0.02 mol/m³ at
  0 °C), held there by the floor's design (`C_E_FLOOR_MOL_PER_M3` is on lookups, not state). Not
  new, but plating is the first reaction whose sign depends on it.

### Added after review

* **Snapshot, restore, continue with the reaction on**: a mismatched pair holding reversible and
  dead lithium, restored through bincode and through JSON (not `restore(&snapshot())`, a clone),
  continues bit for bit across 60 s sub-stepped current steps and a 4.2 V hold at 60 s and 1 s
  (`a_plating_pack_snapshot_restores_and_continues_bit_for_bit`).
* **Debug build**: every run above was a release build, so the pack's `debug_assert` that the
  converged pass's probe and `advance` agree bit for bit had not run on a plating cell — and the
  probe now marches a copy. The fast tests (refusals, store, Jacobian, ledger, the snapshot replay
  on the parallel pair under a hold) pass in a debug build.

## Still open, for the owner and for slices C and D

* **Two of the plan's exits missed** — exit 3's hold leg and exit 5's one-hour hold — and whether
  they are accepted as recorded or fixed first is the owner's decision, not this slice's. The fixes
  in reach: for the hold, sub-stepping the **pack's** demand when a plating cell is present (each
  sub-step a held voltage, so the current can taper inside a long step), which no cell-level change
  can do; for exit 5, the split's own fix (`dfn-split-nan.md` left "hold each cell's share to a
  physical range" open). The hold leg's bar was not reachable as the plan wrote it: a voltage hold
  at one long step is one current.
* **Fast-forward cost.** The sub-step count depends on `dt` alone, so it is paid at rest too: at a
  day-long step a plating cell runs `dfn::PLATING_MAX_SUBSTEPS` = 3600 sub-steps per probe of 24 s each
  (the step-length error is then the 24 s sub-step's, a few per cent), and at a week 168 s. A
  shelf lesson at the Phase 10 speeds with plating on would pay that on every step. A count that
  also reads the start-of-step state (no reversible lithium and no charging current possible) would
  not break the one-curve rule, which forbids only the trial current; not built.
* **Slice C's**: what `soc_true` and `soh_capacity` read while lithium sits as metal; `PLATING_RISK`
  on a reaction cell and the threshold's fade switched off there (it still runs beside the reaction
  now); the dendrite short per amp-hour of dead lithium; the stripping plateau.

The spike's harness `W:\temp\claude\phase12-spike\` is deleted with this slice, as the plan said it
would be: its PyBaMM scripts and its A1b integrator were moved into the tree by slice A, and its
prototype reaction is rebuilt here.
