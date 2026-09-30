# The single-particle cell's range, declared to the pack

Follows `dfn-long-step-holds.md`, whose first "Still open" item this is, and ROADMAP H8.

## Predictions, registered before the after-run

Written after the parent (`fc114d5`) was measured and the change was built, before the
changed engine was run on any case. Harness `W:\temp\claude\spm-window` (the working
tree) and `W:\temp\claude\spm-window-head-harness` (a worktree of the parent), the same
`main.rs` compiled against each.

1. **Nothing moves under a current demand or at rest**, on either porous model, at any
   step length, and no zero-length read moves: the `hash` fingerprints are bit-identical.
   Reason: a current demand's `i_g_full` is the same on every pass, so the miss is
   exactly `0.0`; the new score is gated on `dt > 0`, and the new range answers `None`
   there.
2. **The `Dfn` is bit-identical everywhere in the sweep** (all 2430 lines of its dump): its
   held passes already scored the miss, and no sweep case is under an external short.
3. **No silent miss is left on the `Spm`** in the isothermal sweep — a step that reports
   converged and misses its demand by more than 1 µV (voltage) or one part per million
   (power). Parent: 9 at 1 s and 14 at 3600 s under a voltage hold, 97 under an hour of
   discharge power, 0 elsewhere.
4. **Those become flagged instead**: the unconverged counts rise by about the silent ones
   (voltage 28 → ≤ 37 at 1 s, 1 → ≤ 15 at 3600 s; discharge power at 3600 s 227 → 300–330,
   near the `Dfn`'s 324, since the two share the capacity that decides reachability).
5. **Nothing gets worse in size**: the largest pack current in each sweep class and the
   hottest cell in the thermal runs are no higher than the parent's.
6. **The 10 W hour from 35 %** stops at the edge of the range every hour, flagged, with no
   cell pushed further past empty and no hotter than the parent.
7. **The workspace suite stays green**, the guided-path claims included.

## What the first design ran into (measured after the predictions above)

Two findings changed the slice after the predictions were registered; the owner chose, on
2026-09-30, to fix the root cause rather than ship around it.

1. **The particle's surface range is too wide for the pack.** Its edge is where a surface
   reaches an empty or full particle, well past the chemistry's declared empty. Holding the
   pack to it, the 10 W hour from 35 % ended at 1.92 A and 0.90 V, past empty. So the
   `Spm`'s pack range is now the surface range intersected with a **bulk** range — the
   currents that keep each particle's mean stoichiometry between `stoich_min` and
   `stoich_max`, the edges its SOC reads 0 and 1 at — which is the `Dfn`'s rule and adds no
   constant. That design was run without a second registration.
2. **Where an unreachable power stops depends on the pass cap.** The parent's `Spm` lands
   the 10 W hour at 1.607 A and 2.907 V for any cap from 30 to 40 — it converges there,
   unflagged, in 35 passes, and 1.607 A is the step's true maximum power (4.67 W; a scan of
   hour-long current steps peaks at 4.67 W near 1.62 A). With the range held, the `Spm`
   instead cycles through three passes — the range's edge, a damped step that barely moves,
   1.07 A — and stops wherever the cap falls: 1.07 A at a cap of 30 or 33, 1.80 A at 31, 32
   or 40. **The `Dfn` has cycled the same way since `dfn-long-step-holds.md`**: its
   `an_unreachable_power_stops_at_empty_and_says_so` passes because 32 falls on the "empty"
   pass; at a cap of 30, 33 or 60 the same step lands at 1.07 A and 3.36 V.

The cycle is the score's fault. A damped trial is scored by how far each cell's curve is
from the node on the *last* pass's tangents, plus, on held passes, how far the trial is
from that pass's demand solution times the pack resistance. The second term is measured on
the old line, and with a steep tangent at the range's edge it outweighs any progress: every
trial between the edge and 1.07 A scores worse than the edge, the search takes its
smallest step, and the next full step is accepted against that failed score.

## The redesign: score a trial by the step the next pass would take

Each trial current `I_t` already probes every cell and so already holds the tangents the
next pass would aggregate. Aggregating them — groups, series sum, external-short
transform, the demand solve, this pass's range, and the bound protection revealed this
pass — gives the current `Î` the next pass would commit. The trial's demand term is
`d = |Î − I_t|·R′`, `R′` the pack resistance of that aggregate: the Newton step at the
trial, in volts. The score is `max(gap, d)`, where `gap` is the existing split term.

* `d` is zero exactly at a fixed point of the iteration — a met demand, and for an
  unreachable power the fixed point of "snap to the line's maximum-power point", which is
  the curve's own maximum-power point. So there is no trivial minimum near the last probe,
  which was the false convergence, and no bias against the step toward the answer, which
  was the cycle.
* It replaces the held-pass miss, which was the same quantity measured on the old line.
* A current demand without an external short has `Î = I_t` exactly, so `d = 0` and the
  score is the old one bit for bit; so is every zero-length read, where the term is not
  computed. The equivalent circuit never reaches the loop's second pass.
* Protection is not called again for the trial: it mutates the BMS. Its allowance is an
  interval containing zero that does not depend on the iterate, and a pass that derated
  reveals the bound it hit, so `Î` is clamped to the bounds revealed; an open contactor
  gives `Î = 0`.
* An unreachable power that converges at its maximum-power point is a demand not met, and
  now says so with `SOLVE_UNCONVERGED`, as a demand stopped at the range's edge already
  does. Without it the redesign would turn the flagged 10 W hour into a silent one.

### Predictions for the redesign, registered before it is run

R1. Current demands, rest and zero-length reads: the `hash` fingerprints unchanged on both
    porous models.
R2. The 10 W hour from 35 % lands within 0.02 A of the maximum-power current — measured
    by hour-long current steps 0.01 A apart: 1.61 A (4.671 W) on the `Spm`, 1.605 A
    (4.643 W) on the `Dfn` — flagged, and the landing does not move with the cap (checked
    at 30, 32, 33, 40).
R3. No silent miss left in the isothermal sweep on either model; the 8 `Spm` maximum-power
    landings become flagged.
R4. Total passes over the sweep no higher than this slice's first design on either model.
R5. The `Dfn`'s lithium-conservation check (`W:\temp\claude\dfn-heat`, mode `cons`) stays
    at 0 bad cell-steps.
R7. Over the whole isothermal sweep, no solve's committed answer changes between caps of 31,
    32 and 33 (a solve that does is cycling or crawling to the cap). Counted on the parent
    and the first design too.
R6. `an_unreachable_power_stops_at_empty_and_says_so` fails — it asserts a cycle position —
    and is rewritten to assert what does not depend on the cap.

R7's baseline, measured before the redesign was built (the whole 4860-line isothermal dump
at caps 31, 32 and 33; a line counts if its committed current, voltage, heat or flags
differ between any two):

| | `Dfn`, hour of discharge power | `Spm`, hour of discharge power | `Spm`, elsewhere |
|---|---|---|---|
| parent | 243 of 405, all by more than 1e-4 relative | 70 by more than 1e-4, 158 crawling below it | 14 one-second voltage holds below 1e-9, 2 hour-long ones |
| first design | 243, the same lines | 235, all by more than 1e-4 | 0 |

So the `Dfn` has been cap-dependent on 60 % of its hour-long power steps since
`dfn-long-step-holds.md`, and neither design is a fair "before" for R4's pass counts on
that class: a cycling solve spends the cap.

Added after the advisor's review, still before the redesign was run:

R8. Under the 2 Ω external short (harness `short`, a scattered 1S3P, four steps per demand),
    where no range is held: the 16 silent misses the first design leaves on the `Spm` —
    every voltage hold at both step lengths and the first hour of −10 W — are met or
    flagged. The `Dfn`'s rows, and the `Spm`'s current and rest rows, move by no more than
    1e-9 relative: under a short a current demand's trial now scores the short current
    re-read off its own lines, so these rows are no longer bit-identical by construction.
R9. The next pass's current, as the trial predicts it, is bit-identical to the full step
    the next pass computes, on every pass of the sweep, the short runs and a run where
    protection derates — except where protection clamps at a bound no earlier pass hit.

### What the score alone did (measured)

* R1 held: both fingerprints bit-identical. R9 held on the sweep, both hash sets and the
  short runs: 0 mismatches (protection-binding run still to do).
* R2 held on the `Spm`: the 10 W hour lands at 1.607212 A, 4.6715 W, converged in 17 passes.
* **R8 was aimed at the wrong thing.** The 16 "silent" short-circuit misses were the
  thermal re-read of `v_terminal`: run isothermally the parent has 0 and so does the first
  design. Isothermally the redesign has 1, the `Spm`'s second hour of 10 W under the short,
  which now converges at its maximum-power point unflagged — the flag is not built yet.
* **R7 failed.** Cap-dependent lines: `Dfn` 243 (unchanged in count, but 22 now move by
  ≥ 1 mA, the rest by less), `Spm` 161 (was 235). Traced, three different failures:
  1. **Crawl** (`Spm` 1S1P 50 %, 7.5 W): a smooth but very steep knee at the maximum-power
     point — the next pass's current moves about 16 000 times faster than the trial — so
     the halving search needs `λ` ≈ 2⁻¹⁴ and the step shrinks by only ~0.47 a pass. It
     reaches 7e-10 V exactly at pass 32.
  2. **Corner** (`Dfn` 1S1P 50 %, 7.5 W): the curve itself has a corner at 2.30031 A —
     finite differences of hour-long current steps give a slope of 1.2600 V/A below it and
     1.2883 above, matching the probe's tangent on each side — and power peaks on it. Below
     it the next pass wants 2.322 A, above it 2.296 A; there is no fixed point, `d` floors
     at 5.3 mV, and the search runs to the cap within 2e-6 A of the corner.
  3. **Hill** (`Dfn` 1S1P 50 %, 10.69 W): from 1.64 A the next pass wants the range's edge
     (2.5766 A), past the corner. `d` rises from 0.23 V at 1.64 A before it falls to a few
     mV at the corner, so every halved trial lands on the hill and is refused but a tiny
     one; the solve creeps upward at about 5 mA a pass and stops at 1.73 A, far from the
     maximum. The 22 lines that move by ≥ 1 mA are all this.

### The fix for all three: bracket the sign of the next step

`g(I) = Î(I) − I` is exactly the next pass's step (R9). It changes sign across a met
demand, across a smooth maximum-power point, and across a corner; in all three traces the
first full step already lands on the other side. So once a trial of the other sign is
known, the search stops halving toward the last iterate and instead narrows the bracket —
regula falsi with the Illinois rule, falling back to bisection when two trials in a row
fail to halve it. Every point inside the bracket lies between the last accepted current
and the full step, so the damping argument (protection, the range) still holds. A trial's
demand term becomes `R′·min(|g|, w)`, `w` the bracket's width: the answer is within `w`
of it. Within a pass the search keeps narrowing while the bracket, not the split, is what
keeps the score up; the bracket is carried across passes. The full step is still tried
first on every pass, so a pass that takes it is unchanged.

Registered before it is run:

R10. R7 on the hour of discharge power: at most a handful of cap-dependent lines on either
     model (the corner's side is decided by a sign, so a line differing only in its last
     digits would be a mismatch too), none by more than 1e-6 A.
R11. The three traced cases converge in no more than 12 passes: the `Spm` 7.5 W, the `Dfn`
     7.5 W at 2.30031 A, the `Dfn` 10.69 W at the same corner as the 7.5 W (same cell,
     same state; its demand differs only in how far out of reach it is).
R12. R1, R9 and the parent's bits on every solve that never backtracks stay as they were.

## Results (measured on the working tree as committed)

Harness `W:\temp\claude\spm-window` (`sweep`, `dump`, `hash`, `short`, `one`, `fine`,
`prot`), isothermal wherever a miss is counted; parent numbers from the same `main.rs` built
against a worktree of `fc114d5`.

**R10 held: no answer depends on the cap.** Over the 4860-solve isothermal dump at caps of
31, 32 and 33, **0** lines differ (parent 487: 243 `Dfn`, 244 `Spm`; first design 478; the
score alone, before the bracket, 404). **No solve reaches the cap**; the most any takes is
22 passes. Getting there took a correction after R10 was registered, found by tracing the
lines that still moved: a 4S2P (powers 9 and 11 of the sweep) whose bracket had collapsed
onto a sign filed while its split was still settling and sat at the cap. Two guards went
in for it — the **trust rule**, a sign is filed only when the demand term it implies
outweighs the split gap, and a **stale-bracket check**, which dropped the bracket at the
start of a pass whose last accepted current the new lines read on the other side of where
it was filed. The perturbations below found the second changes nothing; a probe printed
inside it fired **0** times over both dumps (isothermal and thermal), the `sweep`, `short`
and `prot` runs and the four test binaries, while the same probe one line outside it fired
888 times in one test binary. It was removed; both 4860-line dumps are bit-identical
before and after. Should a stale bracket arise without it, the search narrows onto an
interval with no sign change, the demand term stays up, and the solve runs to the cap —
which raises `SOLVE_UNCONVERGED`, so the failure is flagged, not silent.

**One cell lands on one current; a scattered pack does not.** Over the 81 powers of an
hour of discharge a 1S1P lands on the same current to within 1e-8 A, but a scattered 1S3P
or 4S2P has several local maxima of power — one corner per cell's range edge or kinetics
clamp — within 1.4e-5 of each other in relative terms, and which one the search reaches
depends on the power asked. `unmet_powers_land_on_the_maximum_inside_the_pass_cap` checks
the power delivered on those (within 1e-4) and the current only on a single cell. The
landing is "a maximum", not "the maximum", on a multi-cell pack.

**R5 held.** The `Dfn`'s lithium-conservation check (`W:\temp\claude\dfn-heat`, mode
`cons`, thermal network, four hour-long steps per case) reports 0 bad cell-steps and a
worst per-step lithium error of 5.2e-14 A, as the run recorded at the end of
`dfn-long-step-holds.md` did (`cons_final.txt` there). Its three power cases moved, each
to more power in its first hour: the 10 W hour from 35 % lands at 1.682 A and 2.884 V,
4.85 W (then 1.889 A, 2.168 V, 4.10 W), and the scattered 1S3P's 40 W and 60 W hours at
6.919 A and 2.952 V, 20.4 W (then 7.169 A, 2.820 V, 20.2 W).

**R11 held.** The `Spm` 7.5 W case converges in 6 passes at 2.299719 A; the `Dfn` 7.5 W and
10.69 W cases in 5 and 8, both at 2.300307 A, the corner.

**R2 held**: the 10 W hour from 35 % lands at 1.607212 A, 4.6715 W on the `Spm` and
1.606883 A, 4.6439 W on the `Dfn`, flagged; the hours after draw 0.196 A (the cell's
remainder) and then 0 A, flagged.

**R3 held**: 0 silent misses in the isothermal sweep on either model, and 0 in the
isothermal short runs. Both models now flag the same 324 of 405 hour-long discharge powers,
and the same 81 of 405 hour-long charge powers.

**R4 missed on voltage holds.** Total passes against the first design: hour of discharge
power `Spm` 8212 → 3185, `Dfn` 8239 → 2294; every class of power and the one-second `Spm`
holds equal or fewer; but hour-long holds `Spm` 1879 → 1904 and `Dfn` 2463 → 2473, and
one-second `Dfn` holds 1952 → 1955. The holds' extra passes are the demand term now
scoring passes that the old miss scored `0.0`; one `Dfn` hour-long hold (1S1P 2 %, k27) went
from 7 passes to 22. Every changed `Dfn` hold moved by at most 4e-8 A and 2e-9 V from the
parent.

**R1 held** (both fingerprints bit-identical, on every build of this slice). **R12**: of the
4860 sweep solves, 4278 are bit-identical to the first design. Against the parent every
one-second power solve on both models and every `Dfn` charge-power solve is identical; the
`Spm`'s hour-long charge powers are 324 of 405 (the 81 the parent ran to its cap now stop
at the range's edge), and the `Dfn`'s changed holds are the 16 above.

**R9 held, with the registered exception.** 0 mismatches over the sweep, both hash sets and
the short runs. The protection sweep (`prot`: power demands at ±0.9–1.5 × the current
limits on a 1S1P, a scattered 1S3P and a 4S2P, at 1 s, 60 s and 3600 s, BMS on) has 10, every
one on a pass where a latched rung (`UV` or `OV`, allowance 0 on one side) clamped a current
for the first time; each of those steps still converges, in 3 or 4 passes. The derate
itself lands exactly on the limit (7.7298 A = 1.5 C of the chemistry's capacity), on the
parent and here alike.

**R8 was aimed at the wrong thing** (see above). Isothermally the short runs have 0 silent
misses. One row of them is broken on the parent and here alike: the `Dfn`'s fourth hour of
`Current(2.0)` under the short, driven past empty, reaches 1e81 A (parent) or −1.7e88 V
(here) isothermally and 830 K with the thermal network. A current demand is never held, so
that is ROADMAP H8's "current past empty" item, now with this measurement in it.

**R6 held**: `an_unreachable_power_stops_at_empty_and_says_so` asserted the cycle's
position and was rewritten to assert that the landing is flagged, holds no more charge than
the cell had, and is a maximum — an hour at 2 % more or less current delivers less. On the
parent that fails: an hour at 1.7675 A gives 4.214 W against the 3.925 W it landed on.

**Tests red on the parent.** `a_long_hold_or_power_meets_its_demand_or_says_so` (new, in
`spm_long_step.rs`): the parent reports nine hour-long holds from 98 % converged up to
0.56 V off target, and four hour-long powers converged on neither the demand nor the
maximum. `an_unreachable_power_lands_at_the_most_the_cell_can_give` passes unmodified,
as registered.

**The first design's predictions (1–7), judged on the redesign as shipped.** They were
registered for a design that was then changed twice (the bulk range, then the score and
the bracket), so they are read here as claims about what shipped, not as tests of the
reasoning under them.

1. **Held**: the current-and-rest and zero-length fingerprints are bit-identical on both
   models (thermal run; the isothermal ones too, under R1).
2. **False as written.** The `Dfn` moved: its hour-long discharge powers land on the
   maximum-power point instead of wherever the cap fell, and 16 of its holds moved by at
   most 4e-8 A (R4 above).
3. **Held**: 0 silent misses on the `Spm` in the isothermal sweep (R3).
4. **The counts held, the mechanism did not.** Voltage holds unconverged 28 at 1 s and 1 at
   3600 s (bounds ≤ 37 and ≤ 15), hour-long discharge powers 324 (bound 300–330). But the
   23 silent holds were not flagged: they now meet their target.
5. **Held**, in both thermal modes. No sweep class has a larger current or a hotter cell
   than the parent; the two classes that moved went down. Hour-long discharge power, the
   largest current: `Spm` 10.28 → 6.92 A, `Dfn` 7.73 → 6.92 A. Its hottest cell with the
   thermal network: `Spm` 381.6 → 308.7 K, `Dfn` 332.6 → 315.6 K. Every other class is
   equal to 0.01 A and 0.01 K.
6. **False as written.** The 10 W hour does not stop at the range's edge; it lands on the
   maximum-power point (1.607 A, R2), and then takes the 0.196 A the cell has left, then
   0 A. It is flagged every hour, no cell goes past empty, and with the thermal network the
   hottest the cell gets over four hours is 300.7 K, against 372.4 K on the parent, whose
   third and fourth hours drew 6.8 A from an empty cell.
7. **Held**: the workspace suite, 84 test binaries including the guided path's 69 claims
   (`path_claims`), all pass.

**R12's third part was not measured as registered.** Nothing the harness records says
whether a solve backtracked, so "the parent's bits on every solve that never backtracks"
was never counted; the class-by-class comparison above is what was measured instead.

## Perturbations

Each row breaks one piece of the change in `sim-core`, runs the `sim-data` binaries
`spm_long_step`, `dfn_long_step_holds`, `dfn_cell` and `nonlinear_solve` with
`--no-fail-fast`, and is read by exit code and failing test names
(`W:\temp\claude\spm-window\pert\pert.py`, which restores the files after each row; they
were compared against its backups afterwards). The first run, before the last three tests
existed, ran only `spm_long_step`, `dfn_long_step_holds` and `sim-core`'s own tests, and
caught two of eleven. Each green row was then
measured with the harness to see what it broke, and the tests below were written from
those measurements.

| | perturbation | what it broke, measured | caught by |
|---|---|---|---|
| a | the `Spm` declares no range | 10 W hours drain the cell past empty | `an_unreachable_power_never_drives_the_cell_past_empty` |
| b | the surface range alone, no bulk | the same, to 0.90 V | the same |
| c | no demand term in the score | more unconverged holds, and 2 silent misses | `unmet_powers_land_on_the_maximum_inside_the_pass_cap` |
| d | no margin inside the range's edge | the same drain as a | `an_unreachable_power_never_drives_the_cell_past_empty` |
| e | the range not widened to contain zero | (red on the first run too) | both files' past-empty tests |
| f | no bracket | the 10 W hour runs to the cap | `unmet_powers_land_on_the_maximum_inside_the_pass_cap` |
| g | no trust rule | the 4S2P's powers 9 and 11 sit at the cap | the same (it samples those two) |
| h | no stale-bracket check | **nothing**: both dumps bit-identical | none — so it was removed (above) |
| i | no flag on a power past reach | five tests | the flag tests in both files |
| j | protection's bounds not applied to `Î` | 192 against 77 unconverged steps in `prot` | `a_derate_inside_the_iteration_does_not_chatter`, an **existing** test in `dfn_cell.rs`, not in the first run's set |
| k | no narrowing inside a pass | one solve takes 27 passes | `unmet_powers_land_on_the_maximum_inside_the_pass_cap` |

Second run: 10 of the 11 red, each on the test named; h green, and removed.
