# Phase 12 slice B — plating as a reaction inside the `Dfn`

**Status: PREDICTIONS REGISTERED 2026-10-18, before any engine code.** Plan: `phase-12-plating.md` §B;
the constant and the reference: `phase-12-slice-a-reference.md`. Snapshot 28 → 29.

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

## Results

*(to be written after the runs)*
