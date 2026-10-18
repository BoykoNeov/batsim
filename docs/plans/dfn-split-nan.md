# A failed `Dfn` split must not commit NaN

**Status: COMPLETE 2026-10-18** — the owner chose to land it with 11 sweep runs still broken past
the `Dfn`'s edges, recorded under ROADMAP H8, rather than widen it. Results in "The damped split,
measured" below. Found by the Phase 12 spike's control runs (`phase-12-spike.md`, A3);
listed under `docs/ROADMAP.md` H8. It is the prerequisite of Phase 12 (`phase-12-plating.md`): the
plating lesson runs on a parallel `Dfn` pack, and a plating measurement taken on a solver that can
corrupt its own state would be worth nothing. When written the repo was at `SNAPSHOT_VERSION` 28.

## The defect

Shipped engine, `chemistries/nmc_21700_lgm50.toml`, recommended grid, isothermal. A 1S2P `Dfn` pack,
one cell at 0.9 of the other's capacity, charged from empty at 1C per cell under a current demand
with **one-hour steps**, at 0 °C or 25 °C as the spike recorded it (reproduced in the tree on the fine
grid at 0 °C only; see "Reproduction and diagnosis"):

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

## Reproduction and diagnosis (2026-10-18)

The test is `crates/sim-data/tests/dfn_parallel_long_step.rs` (sim-data, not sim-core: it reads the
chemistry file). **In the tree the defect is narrower than the spike recorded**: NaN on the third hour
on the **fine grid at 0 °C only**. At 25 °C on the fine grid, and on the recommended grid at either
temperature, every value stays finite (the pack overcharges to 5.8 V past full, the open H8 gap). The
spike's "0 °C or 25 °C" is corrected in its note and in ROADMAP H8.

Traced pass by pass (temporary prints, removed):

* **Hour 1 at 25 °C never settles either.** Under a current demand every damped trial splits the same
  group current, so the damping search cannot help: the loop cycles for its 32 passes between about
  −2.4/−7.9 A and −3.6/−6.7 A and commits whichever pass the cap fell on. Nothing is full yet.
* **Hour 3 at 0 °C** cycles, then grows: −2.1/−8.2 A at pass 3 (0.74 V from the cells' curves), ±200 A
  circulating by pass 6, ±1e7 A by pass 11, ±1.2e10 A by pass 12. At that current the `Dfn`'s
  continuation past its window edge (`past_edge`) takes its slope as a difference quotient, which
  rounded to exactly **0 Ω** — and, unlike the solve's own slope, that one had **no floor**. The next
  pass divided by it, every line went NaN, and the cap committed them.
* **No cell's own update produced a non-finite state from a finite current**, in any run: the NaN
  enters `dfn::advance` as its current (`i_last = i`).

Scored:

1. **Failed.** The NaN starts in the pack's split, not in the cell: a degenerate (finite, zero) slope
   from the cell, divided by in the pack.
2. **Failed, the other way round.** With the `Dfn` exclusion lifted the settle settles **neither**
   early hour, and "settles" two overfill hours — one at +8.8/−19.1 A, a cell discharging while the
   pack charges — and the NaN still arrives, two hours later. The exclusion stays.

## The fix, and the predictions restated before measuring it

1. **The floor on every `Dfn` slope**, `past_edge`'s included (`dfn::floor_r`, the solve's existing
   `1e-9 Ω`). The `Spm` and the many-particle cell already floor what they hand the pack.
2. **The best pass.** Under a current or rest demand with no external short, where the loop caps and
   the settle does not rescue it, the step commits the pass with the lowest score it saw (a NaN never
   counts; a pass whose pack current differs, bit for bit, from the last is not eligible), rebuilt
   from the lines that pass split on exactly as the settle writes its answer back. Its lines live in
   `StepScratch`, so it allocates nothing after the first step. Still `SOLVE_UNCONVERGED`.

Prediction 4 above ("only where a non-finite value would be committed") is **withdrawn**: the best
pass acts on every capped current or rest step the settle does not rescue, finite or not. Restated:

4′. Every step that converged on the parent is bit-identical; the suite stays green; a capped step's
    committed answer may move, and the only tests that can see it are ones that pin an unconverged
    step's values.
6. The best pass makes a capped step's answer **independent of where the cap falls** wherever the best
   pass comes before pass 31: on the reproduction, the committed split is the same at pass caps 31, 32
   and 33, where on the parent it is not.

## The sweep against the fix above, and why it is not the fix (2026-10-18)

The sweep (100 runs, 4200 steps: both grids; 0 and 25 °C; a 1S2P with one cell at 0.9 and a 1S3P
scattered at σ 0.05; charge from empty and discharge from full at 1 and 2C per cell, and a 4.2 V hold
from half full; 60 s (recommended grid only), 15 min and 1 h steps; three hours each), run on the parent
and on the floor-plus-best-pass tree, compared frame by frame:

* **4′ held**: no run's first difference falls on a step the parent converged (0 of 60 differing runs).
* **5 held, far past what it said**: on the parent **32 of 100 runs commit NaN**, nearly all of them the
  discharges past empty, at every step length and on both grids. The reproduction was one of them, not
  the case.
* **The fix does not meet the exit.** Counted as broken — a non-finite value, or a cell carrying more
  than the pack's current plus 1C — the parent breaks 34 runs and the fix 23. But 21 of the fix's 23 are
  **finite nonsense** (10¹² to 10²⁹ A circulating between cells), most of them runs that were loud NaN on
  the parent, and one run clean on the parent goes NaN on the fix (`rec-298.15-3p-scatter-dis2-60`: the
  best pass committed a different split at its 31st step, from which the next steps' loops ran to −642 /
  2283 / −1610 A and then 1e11 A).
* So the claim in the best pass's comment, "a finite candidate always exists", is false: it holds for
  finite, not for sane, and a committed garbage split makes the next step's first pass garbage too. Choosing
  among one step's passes cannot fix a damage that accumulates across steps.

**What replaces it: the split is damped.** Under a current or rest demand the damping search
shortens the pack current, which is the caller's, so its 16 attempts per pass evaluated the same split
16 times and took it anyway. Instead, where the pack current did not move from the last accepted pass,
each attempt after the full step walks every cell's current, and each group's node voltage, back towards
the last accepted pass's — `i(λ) = i_acc + λ·(i_full − i_acc)`, halving `λ` — and is accepted on a strict
fall of the same residual. The blend conserves charge (each end sums to the group current less a leakage
linear in the node), its residual at `λ = 0` is the last accepted pass's exactly and at `λ = 1` the full
step's, and to first order every cell's gap shrinks by `1 − λ`, so it is a descent direction. An
accepted damped split is written back as lines through it (`(V + i·R, R)`, the settle's form), so the
reporting pass re-derives it. The best pass is removed.

Predictions, before the run:

7. **Bit-identity, restated.** Every pass whose full step was accepted is unchanged; on the parent a
   rejected full step under a current or rest demand was taken anyway, so a step can move only if it
   had one. The suite stays green. Of the sweep's steps the parent converged, some move; the count is
   stated.
8. **The sweep**: zero non-finite runs; zero runs broken on the slice that were clean on the parent;
   broken (finite past the bound) runs: few, and stated. Confidence: moderate — the damping holds the
   residual within a step, not the physics past full or empty.
9. **Cost**: no more `probe_at` calls than the parent on any step (the attempts it spends were spent
   already), and the capped steps fewer, since a damped loop converges where the cycling one did not.

## The damped split, measured (2026-10-18)

The same 100-run sweep, parent against the damped split (`StepScratch::split_acc` and its three
siblings), frame by frame:

| | parent | slice |
| --- | --- | --- |
| runs that commit a non-finite value | 32 | **0** |
| runs broken (non-finite, or a cell past the pack's current plus 1C) | 34 | **11** |
| of those, clean on the parent | — | **0** |
| steps flagged `SOLVE_UNCONVERGED` (of 4200) | 1406 | 979 |
| steps the parent converged whose frame moved | — | 1, in its last bits |

**The stall rule, found by the first run of it.** The damped split's first sweep still had one NaN
run: where no blend improved, the search took its smallest step anyway — the pack-current search's
rule — and that trial had probed NaN. The pair discharged at 2C per cell from full at 0 °C, 15-minute
steps: on its fourth step a probe of the larger cell, past the `Dfn`'s rate limit, returned a **NaN
voltage for a finite current** (−8842 A). So "no cell's own update produced a non-finite state from a
finite current" above holds for the cell's update (`advance`), not for its probe. The rule now: where
no blend improves on the last accepted split, the step keeps that split (`λ = 0`, the last accepted
residual) and stops the loop, flagged unconverged — under a fixed pack current the next pass would
re-take the same tangents and fail the same way to the cap. Pinned by
`a_cold_fast_discharge_past_empty_stays_finite`.

**A milder rule, tried and refused.** The strict rule leaves seven runs with more unconverged steps
than the parent, all of them charges carried past full (where this model's curve is flat, and the
parent "converged" on it), but for two steps of a cold discharge the parent already broke. Accepting
the full step wherever its residual is finite and below the step's first pass's, to let a flat curve
settle, broke 15 runs (11), left 1305 steps unconverged (979), and broke again the run the best pass
had broken. Reverted.

**The 11 runs still broken**, every one broken on the parent too, most of them earlier: the 1S2P at
0 °C discharged at 2C per cell (recommended grid at 60 s and 15 min; fine grid at 15 min) and at 1C
(fine, 15 min); the 1S3P at 0 °C discharged at 2C (recommended, 60 s and 15 min; fine, 15 min); and at
25 °C on the fine grid, both packs charged at 2C (15 min), the 1S2P discharged at 2C (15 min) and the
1S3P discharged at 2C (1 h). The highest per-cell rate any of them reached before it broke was between
2.1 and 6.7C. They are past empty or full at rates where the `Dfn` has no physics (ROADMAP H8); holding
each cell's share to a physical range, as a voltage or power demand's current already is, was offered
to the owner as a widening of this slice and declined in favour of landing (2026-10-18).

**Phase 12's own configurations**: every 1C charge and every 4.2 V hold in the sweep is finite and
bounded, and every hold converges. The mismatched pair's **first** one-hour step at 1C per cell does
not converge, on the parent or the slice (−3.6 and −6.7 A at 25 °C on the recommended grid, the
same split on both), so `phase-12-plating.md`'s exit 5 stays at risk.

Scored:

* **3.** The ten-step reproduction is finite on every step on both grids at both temperatures, and
  now held to the current bound too, the heated arm included. Whether its rests converge was not
  checked.
* **6. Withdrawn**: the best pass it was about is gone.
* **7. Failed once, then held.** The first full debug suite reddened one existing test,
  `nonlinear_solve.rs::a_non_finite_demand_hits_the_cap_and_reports_it_without_panicking`: a 1S1P
  pack under a NaN demand stopped at pass 2 on the stall rule instead of at the cap. A group of one
  cell has no split — its blends are its full step — so the damping is now for groups of two or more,
  and that path is the parent's again. With that the debug workspace suite is green: 845 passed,
0 failed, 3 ignored. Of the sweep's 2794
  steps the parent converged, one moved (`rec-273.15-3p-scatter-dis1-60`, step 63, in the last bits
  of its frame); the sweep is all parallel, so the restriction does not change it.
* **8. Held.** Zero non-finite runs, zero broken on the slice that were clean on the parent, 11
  broken (above).
* **Found by the debug suite: a debug build and a release build do not agree here.** The cold
  discharge above, on a debug build, splits its first quarter hour 10.449347669631525 /
  10.163444330368456 A where the release build splits it 10.449347669800147 / 10.163444330199857 A,
  and by its fourth it circulates ±4.96e5 A where the release build has 27.4 / −6.7 A. The parent does
  the same (its first quarter hour differs from the tenth digit too), so it is not this slice's. That
  step is unconverged on both, and an unconverged split amplifies whatever differs; what differs
  first was not traced (`powi`, the one function exempt from the `libm` rule, is used only at
  exponents 2–4 and as `0.5^h`, which both builds compute by the same squarings, so probably not it).
  Recorded under ROADMAP H10. `CLAUDE.md` claims determinism on the **same binary** only, which this does
  not contradict. The test's conservation check had been relative to the pack's current, and the
  debug build's ±5e5 A summed to 20.6 A within 2.7e-8 A, past it; rounding in a sum scales with its
  largest term, so the check is now relative to the cells' own currents.
* **9. Not scored as written**: the probes were not counted. What was timed: the in-tree test file,
  release build, 267 s for its first four tests under the best pass, 13.5 s for all six on the slice,
  because a step that stalls stops instead of spending its 32 passes.

**The perturbation table** (`dfn_parallel_long_step.rs`, release, one piece removed at a time):

| removed | red | how |
| --- | --- | --- |
| the stall rule (take the smallest blend, as before) | `a_cold_fast_discharge_past_empty_stays_finite` | NaN voltage on the fourth quarter hour |
| the split damping (and with it the stall) | `the_fine_grid_at_zero_celsius_stays_finite`, `…_with_heat` | ±2.5e86 A on the third hour; ±690 A and runaway on the heated second |
| the floor on `past_edge`'s slope | **none** | with the split damped, no test reaches the 1e10 A where the slope rounded to zero |

So the floor is kept as a guard nothing in the tree reaches: it costs one comparison, and it is the
same floor every other slope this model and the `Spm` hand the pack already had. And
`a_scattered_triple_discharged_past_empty_stays_bounded` reddens on none of the three: on the
parent it stayed finite, and what it pins is the best pass's failure, a design no longer in the tree.

## Exit

* The reproduction test, green; and the sweep, with **zero** committed non-finite values. **Met.**
* Every step that converged on the parent is bit-identical (the full suite, and a hash of every
  telemetry frame across the sweep, parent against slice). **Met but for one**: the suite is green,
  and one of the sweep's 2794 converged steps moved in its last bits (prediction 7).
* A perturbation table: delete the guard → which tests go red; disable the fallback's charge
  balance → which go red. **Met for what shipped** (the table above); the fallback's row is moot,
  the fallback being refused.
* ROADMAP H8's NaN bullet closed or narrowed to what remains (the `Dfn`'s physics past full stays
  open; it is not this slice's). **Met**: closed, and the split's bullet narrowed to the 11 runs and
  the unsettled first hour. The debug/release disagreement is recorded under H10.
* Prediction 3's "the rest steps converge" was not scored.

## Deliberately not done

* **Physics past full for the `Dfn`.** That is the open H8 bullet, and Phase 12's plating gives an
  overfilling graphite somewhere for the current to go (the spike's prototype converged where the
  control did not), which is the honest route to it on a charge.
* No snapshot change is expected; if the fix needs one, it is stated and bumped here, not in
  Phase 12.
