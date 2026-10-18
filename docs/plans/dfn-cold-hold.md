# A cold 4.2 V hold, and the `Dfn` discharge after it

**Status: DIAGNOSED 2026-10-18; the prototyped fix FAILED its registered predictions and is
withdrawn. **Owner decision 2026-10-18: design slice D's lesson without the hold** — each cold charge stops at 4.2 V, then rests or discharges; the defect stays open (ROADMAP H8).** The taper below fixes the recommended grid's case but not the fine
grid with plating on — the case slice D needs — and it changes results in the overcharge regime that
four test families pin. No narrower fix passes both of the bars set for one (below, "What is left").
The engine is unchanged; the prototype is kept as a patch beside this note. Found by Phase 12 slice C (`phase-12-slice-c-health.md`, "Found on the
way"); the owner chose to fix it before slice D (`phase-12-plating.md` §C). Tests:
`crates/sim-data/tests/cold_hold.rs`; the diagnostics `plating_health.rs::report_discharge_after_a_hold_*`.

## The defect

0 °C, 1C to 4.2 V, then 4.2 V held to C/20, 1 s steps, shipped LG M50, recommended grid, a threshold
cell (no plating reaction — the defect is older than Phase 12): 1413 of the hold's steps flag
`SURFACE_OUT_OF_RANGE`, every one converged; the C/2 discharge after it is unconverged on 1146 of its
first 1200 steps, 30 ms a step against 0.2 ms after the constant-current leg alone (190–250 ms on the
fine grid). At 25 °C: nothing out of range, nothing unconverged.

## What it is (measured)

* **One node.** The negative electrode's node at the separator (node 9 of 10) fills first. In the hold
  its surface climbs to within 1e-9 of `c_max` by 6600 s, and from 6883 s past it, to 8.9e-7 above at
  the hold's end, while its reaction current is a few µA/m² (`cold_hold.rs::report_the_cold_hold`).
* **Why it can pass full.** `System::kinetics` clamps the surface to `[ε, 1 − ε]·c_max`
  (`SURFACE_EDGE_FRACTION`, `ε = 1e-10`) before the exchange current reads it, so past the band the
  exchange current is frozen at its value on the edge instead of falling to zero, and the surface no
  longer moves with `j`. The constant's own note says so: past it "the node accepts lithium without
  bound". A long hold at a temperature where the graphite fills slowly is enough to get there.
* **Why the discharge does not converge.** A temporary trace inside the Newton loop (removed) showed
  every failing solve stalling at a scaled residual of 1.4e-8 to 6.5e-8 against `NEWTON_TOL` 1e-8, the
  worst row always node 9's Butler–Volmer row, its surface on every iterate within ±1e-10 of the band's
  edge — the solution sits on the kink the clamp makes, at the start of the discharge and still at
  steps 300 and 900. The line search ran to its last halving on several iterations.
  * **Not the Jacobian**: against a central difference at the failing states it agrees to 3.9e-5
    (step 300) and 1.3e-4 (step 900) at `h_rel` 1e-9; at 1e-7 the difference straddles the edge.
  * **Not the iteration cap**: at 500 iterations the same 1146 steps fail, and the voltages are the
    same to four digits as at 50. The residual has a floor; the voltages are not wrong.

## The fix, as prototyped

**Past the band at a full negative surface, the exchange current falls linearly to zero at `c_max`,
and the surface keeps moving with `j`** (`i0 = i0(1 − ε)·(c_max − c_s)/(ε·c_max)`, zero at and past
full; the open-circuit potential read at the band's edge, its slope zero there). Inside the band
nothing changes, so a solve whose iterates never leave it is bit-identical. A full graphite surface
then takes no more lithium, as it physically cannot, and the node's equation has no kink at its
solution.

Measured while choosing:

| variant | hold steps out of range | discharge unconverged (of 1200) | hash harness, six packs |
| --- | --- | --- | --- |
| today | 1413 | 1146 | — |
| taper at both edges of both electrodes | 0 | 0 | four alike; two `Dfn` packs part, one of them collapsing to −60.8 V on its final cold 2C discharge |
| taper at the full edge of both electrodes | 0 | 0 | the same two part, the same collapse |
| **taper at the full edge of the negative only** | **0** | **0** | **four alike; the two part by ≤ 1.2e-8 V by their last step** |

* The two packs that part are 2S1P `Dfn`s at 0 °C driven by a **current** demand past the 4.2 V
  ceiling at 1C (60 s steps, `OPERATING_POINT_OUT_OF_WINDOW`), then held and discharged at 2C. Before
  the change they first differ on a step that converged, at 2e-13 V — the overcharge putting a
  negative surface into the band, ROADMAP H8's "no physics past full", which this changes by design.
* The −60.8 V collapse is the positive electrode's full edge: on a hard cold discharge its surface
  fills, the taper chokes the node, and under a forced current the voltage falls without limit, where
  the clamp let it overfill and the step sat at ~4.7 V, unconverged. That is the `Dfn`'s rate limit
  (H8: the pinched positive electrode), a different defect, and choking it is a change to the 3C
  goldens' regime too. **So the taper is the negative electrode's only**, and the positive's full
  edge keeps the clamp — recorded as the asymmetry it is.

## Predictions (registered before the runs below)

1. **Bit-identical wherever no negative surface comes within `ε` of full.** Every golden passes
   unchanged, and every test outside `cold_hold.rs` passes unchanged except possibly those that drive a
   negative surface to full: a charge past the 4.2 V ceiling under a current demand, a long cold hold,
   a one-hour step that overfills (`plating_dfn.rs`'s exit 5 pair and `the_end_of_step_curve_still_falls_with_current`,
   `dfn-hold-past-edge.md`'s fixture). Each of those that moves is named, with its old and new figure.
2. **The cold hold, fine grid, plating on** (`cold_hold.rs`): 0 °C and −10 °C, 1C CC–CV then C/2 to
   2.5 V, 1 s steps — no hold step out of range, every discharge step converged.
3. **The same on the recommended grid with a threshold cell**, 0 °C, as measured above: 0 and 0.
4. **The plating figures barely move.** `plating_dfn.rs`'s exit test: every gross plated figure within
   0.1 % of today's (the taper engages only near the end of a cold hold, where plating has stopped).
5. **Exit 5's flagged first one-hour hold step** (accepted by the owner as "cells' own solves after the
   overfill"): it converges, or its flag moves to a positive surface — whichever, recorded.
6. **Unconverged trials of `the_end_of_step_curve_still_falls_with_current`** (0/0 at 1 s, 0/3 at
   15 min, 5/7 at 1 h): none rises; the curve still falls with current between every two converged
   neighbours.
7. **3C past its cut-off** (`dfn-scenario.md`'s unconverged steps, the positive's choke): unchanged —
   the positive keeps its clamp.
8. **Cost**: a converged step pays nothing (one comparison).

## Results (run 2026-10-18, the negative-only taper, release, `--no-fail-fast`)

* **1, missed.** Besides `cold_hold.rs`, four test files fail, every failure in the regime past full:
  `dfn_parallel_long_step.rs` (all four "stays finite" runs: on the second hour of a 2C charge driven
  past full, unconverged, the cells' currents no longer sum to the pack's — −10.3063964 against
  −10.306396 A), `plating_dfn.rs` (`the_sub_stepped_tangent_is_the_whole_steps_slope`: its central
  difference's trials no longer converge; `the_mismatched_pair_converges_with_plating_on` at 1 h),
  and `surface_gap.rs::an_overcharged_cell_relaxes_to_no_gradient_at_all` (gradients of −0.059 and
  −0.385 remain at rest). **The cause is the taper's own:** where a node's zero-current surface is at
  or past full, an exchange current of exactly zero makes "no reaction" a root, and a node can freeze
  there — an overcharged cell then cannot relax. Every golden passed.
* **2, missed.** Fine grid, plating on, 0 °C: no hold step out of range, but **1507 of 5777**
  discharge steps unconverged. (−10 °C was not reached: the test stops at the first case.)
* **3, held**: recommended grid, threshold cell, 0 °C — 0 and 0 (the prototyping run).
* **4–8**: not scored; the exit test passed, the rest was not read once 1 and 2 had missed.

**What the fine grid's case is** (the committed engine, a temporary trace, removed): plating on at
0 °C and −10 °C and a threshold cell at 0 °C, 1C CC–CV then C/2 — **every one of the first 300
discharge steps unconverged in all three**, the stalling row a negative node near the separator
(nodes 34–36 of 40), its Butler–Volmer row, the surface within ±1e-10 of the clamp's edge — the same
mechanism as on the recommended grid. Not plating's stripping cap: the reversible store is empty
within the first 300 s, and the threshold cell does the same. But the residual floor is **4.5e-7 to
4.4e-5** scaled, not just over `NEWTON_TOL` as on the coarse grid.

## What is left

The two bars any fix was held to: it fixes the fine grid with plating on at 0 °C and −10 °C, and it
changes only steps that do not converge today (commit-or-nothing, everything else bit-identical).

* **The taper** fails both, as above. A floor under it, or a smoother ramp, is the same move again.
* **Accepting the stall** on a node in the clamp band carrying almost no current: the fine grid's
  floor is up to 4.4e-5 — three thousand times the tolerance — and there is no tighter reference to
  check the voltages against, since the solve cannot get below the floor at any iteration count.
* **So it is a model change or no change**, and that is the owner's: a change to how a full surface
  reacts (the taper, with its frozen-node root dealt with, and the H8 tests re-pinned as measured),
  or a lesson designed around it (no hold, a discharge after the constant-current leg only, which
  converges at normal speed — `plating_health.rs::cc_leg_rest_and_discharge_keep_reading_a`), or a
  lesson that runs on the flagged steps (190–250 ms each on the fine grid).

The prototype is kept beside this note as `dfn-cold-hold-taper.patch` (a `git apply`-able diff of `crates/sim-core/src/dfn.rs` against 531f069), not in the engine.
