# The single-particle cell at a long step — its curve read at the end of the step

**Status: built, 2026-09-23.** The `Spm` half of `end-of-step-split.md`, which left it open
with one measurement: a scattered 1S3P LG M50 at −3.7e28 V by its fifth one-hour step.

**Unlike most notes here, no predictions were registered before the build.** The fix was
built measurement-first against an out-of-tree harness, and two of its four parts (the
range, and heat read off the curve) were found during the build rather than planned. The
before/after tables and the perturbation table below are the record instead.

## What was wrong

The pack solves a porous-electrode cell by iterating tangents to its `V(i)` curve: each pass
re-takes every cell's tangent at the current the previous pass gave it, and stops when the
tangents agree with the curve. For the `Spm` that curve was read off the **start-of-step**
state — the particles as they were before the step moved any lithium. That is explicit
Euler on every coupling the cells have through their shared node, exactly the defect the
equivalent circuit had: the charge a cell gives up over the step lowers its voltage, and a
split that cannot see that over-corrects by more each step.

Measured on the tree at `a51f634` (out-of-tree harness, `W:\temp\claude\spm-eos`), shipped
LG M50, 20 shells:

| case | what happened |
| --- | --- |
| 1S3P, σ = 0.05, C/5, one-hour steps | branch currents −2.8 / +4.9 / +0.8 A by step 3; 10 320 K by step 4; −1.2e9 V by step 5 |
| same, ten-minute steps | fine to 6 % charge, then 1.8e10 A and 2.6e12 K at step 29 |
| 1S1P `Voltage(4.1)` from 50 % | opened at **37.7 A** whatever the step: 443 K after one ten-minute step, 557 K after one hour |
| 1S3P at rest, every cell at 50 %, one-hour steps | rounding-level branch currents growing about fivefold a step, 1.5e-14 → 1.9e-12 A in six |
| 1S2P, one cell at half capacity, 5 A for an hour, then rest at 1e6 s | 1.7e9 A on the second rest step, NaN by the seventh |
| 1S1P `Power(10 W)` from 35 %, one-hour steps | 55.5 A and 1290 K on the second step (a false root; see below) |

## The fix, in four parts

**1. The curve is the end-of-step one.** `spm::probe_at(…, i, dt)` diffuses both particles
over `dt` under `i`'s flux and reads the surface from there. It costs one forward sweep per
particle per probe, not a diffusion solve per evaluation, because the surface is affine in
the flux: after the forward sweep of `diffuse`, the outer shell is `((t − k·j) − m)/d` with
nothing else depending on `j` (`OuterShell`). `diffuse` now evaluates its own outer row
through the same expression, so the probe and `advance` agree **bit for bit**. Checked two
ways: the old and new `diffuse` agree on all 200 000 random inputs tried (shell counts 2–64,
fluxes and steps over nine decades), and an isothermal voltage hold lands on 4.1 V to 1e-10 V
at 60, 600 and 3600 s. `dt <= 0` reads the stored state, as `diffuse` does, so a
zero-length probe step is unchanged.

**2. The first pass starts from that curve.** The memo's tangent is `dt`-free — it has to
be, it is a pure function of state — and a first pass whose residual is already under
`SOLVE_TOL_V` is accepted. So a small circulation was still split on the start-of-step
line: that is the rest case above, growing until the residual crossed the tolerance. On a
step with `dt > 0` the first pass now aggregates from each cell's end-of-step tangent at the
current it last carried (`CellModel::first_pass_tangent`), held in the buffer a linear pack
uses for its own end-of-step sources and never in the memo. Not for the `Dfn`, whose probe
is a nonlinear solve and whose stored tangent is already an end-of-step one.

**3. The range the model describes.** Past empty or past full the surface is clamped and
`V(i)` goes flat, moving only through the kinetics. A tangent to a flat curve has almost no
slope and sends the next pass anywhere. `spm::current_window` gives, in closed form, the
current range over which both surfaces stay strictly inside the clamp at the end of the
step. It is used twice, and deliberately no more:

* **The seed** is pulled back inside it, when the cell could rest through the step inside
  it (zero current is in range). That is the eleven-day rest: the cells last carried 5 A,
  which empties a particle many times over in eleven days. Pulled back to the range — a few
  milliamps at that step — the exchange decays from 1.8e-4 A without reversing. A cell
  already past a limit is left alone: pulled back, a 1S3P group driven through empty at
  20 A ran to 1.5e9 A on its second step, because its answer really is out on the flat curve.
* **A power demand's probes** are held inside it. Power is the one demand whose current the
  engine, not the caller, chooses, and the flat stretch holds a root no real cell has: the
  10 W case's `P = i·V_end(i)` peaks under 5 W inside the range, collapses as the particle
  empties, and climbs again on the flat curve to reach 10 W at 57 A and 0.18 V. Held, the
  solve lands at 1.6 A and 2.91 V (4.7 W) and raises `SOLVE_UNCONVERGED`, which is the true
  statement: the demand was not met. A fixed point inside the range is untouched by the
  hold, so every reachable power solves as before.

Holding the probes to the range under **every** demand was tried first and refused on
measurement: a current demand that drives a cell past empty never converged (the caller
asked for a point the hold forbids), and a tangent merely *steepened* outside the range
still wandered on the flat curve and did not fix the rest case.

**4. Heat read off the curve.** The pack's heat estimate is `i·(U_eq,start − v_node)`, and
`v_node` is now the step's last instant, so it booked the fall of the equilibrium voltage
across the step — stored energy leaving through the terminals — as heat: a 1S3P group at
C/5 read 303.3 K after one hour-long step against 299.5 K after sixty one-minute ones. The
`Spm` arm of `advance` now hands back corrections, in the equivalent circuit's two slots,
that replace that estimate with ones read off the cell's own curve:

* reported (`q_gen_w`): `i·(U_eq,end − V_end(i))`, the end-of-step heat, which pairs with the
  end-of-step terminal voltage an `Spm` pack reports;
* integrated by the thermal network: `½·i·[(U_eq,start − V_start(i)) + (U_eq,end − V_end(i))]`.

A trapezoid, stated: the particle's overpotential has no closed-form step mean, and its
concentration part grows roughly as `√t`, whose mean is two thirds of its end value rather
than half. Measured, 1S1P C/5: 0.70 K of rise in one hour-long step against 0.85 K in sixty
one-minute ones.

Both are read off the curve and not off `v_node` on purpose. The node is on the curve only
when the solve converged; on an unconverged step (the held power demand above, and
`solve_safeguard.rs`'s 11 of 810 voltage holds) a correction that assumed it was fed the
network heat the cell never made — under the refused variant that steepened tangents
outside the range, the 10 W step cooled a cell to 189 K under load. With the final code no
test can see the difference (perturbation P6 below).

No snapshot bump: no state was added or reinterpreted. `SpmState::i_last` still means the
current the cell last carried.

## After

Same harness, same cases:

| case | before | after |
| --- | --- | --- |
| 1S3P C/5, one-hour steps | 10 320 K by step 4 | every branch 0.96–1.05 A to empty, ≤ 2.25 solve passes a step on average (was 10.25, one step at the 32-pass cap) |
| same, ten-minute steps | 1.8e10 A at step 29 | every branch 0.96–1.08 A to empty |
| 1S1P `Voltage(4.1)`, 600 s / 3600 s | 37.7 A, 443 K / 557 K | opens at 5.2 A / 1.5 A and tapers monotonically |
| 1S3P at rest, one-hour steps | ×5 a step | 2e-15 A, not growing |
| 1S2P eleven-day rest after a hard discharge | 1.7e9 A | 1.8e-4 → 9e-8 → 6e-11 A, no reversal, converged |
| 1S1P `Power(10 W)` from 35 %, first hour | 2.8 A at 0.49 V (met at the first instant, then empty) | 1.6 A at 2.91 V, `SOLVE_UNCONVERGED`, 300.7 K |
| 1S3P C/5, temperature after one hour-long step | 299.0 K | 299.2 K (sixty one-minute steps: 299.5 K) |

## Tests, and what each one answers to

`crates/sim-data/tests/spm_long_step.rs`, seven tests on the shipped LG M50. Run against the
engine at `a51f634` (a worktree), six fail there. The seventh,
`a_group_driven_through_empty_still_converges`, passes on both. It guards a rule this slice
added (the seed is pulled into the range only for a cell that could rest inside it), not
the defect this slice fixed.

`crates/sim-core/tests/spm_cell.rs::a_closed_cycle_conserves_energy` now pairs each step's
current with that step's own reported voltage, as the equivalent circuit's energy tests
have done since `end-of-step-split.md`. It passed both ways, inside its 0.5 %.

Perturbations (`W:\temp\claude\spm-eos\perturb.py`): one wrong edit at a time,
`cargo test -p sim-core -p sim-data --no-fail-fast`, every red test listed, and the tree's
diff compared byte for byte before and after the run.

| # | the wrong edit | what goes red |
| --- | --- | --- |
| P1 | the probe reads the start of the step | **5**: the group, the voltage hold, the eleven-day rest, the unreachable power, the resting group. The heat test stays green, correctly: the heat is read off the curve whatever the solve did. |
| P2 | no first-pass seed | **3**: the resting group, the eleven-day rest, the unreachable power |
| P3 | the seed not pulled into the range | **1**: the eleven-day rest |
| P4 | the seed pulled in even past a limit | **1**: the group driven through empty |
| P5 | both heat corrections zeroed | **3**: the heat test, the unreachable power, and `path_claims::every_claim_matches_the_engine` — a guided-path lesson reads an `Spm`'s heat |
| P6 | the heat corrections trust `v_node` | **nothing.** The rule that the heat is read off the curve and not off the node rests on an argument, not a test. The 189 K cell that motivated it came from a variant that steepened tangents outside the range, since refused; with the power hold in place, the unconverged power step's node sits close enough to the curve that the difference does not show. Stated rather than covered by a contrived case. |
| P7 | power not held to the range | **1**: the unreachable power |

The whole workspace suite is green with the change (`cargo test --workspace`), including
`spm_golden.rs` and the guided-path claims. No existing test reddened while the fix was
built: the suite had no `Spm` step longer than a few minutes on a parallel group or under a
held voltage.

## Still open

* **Past-empty physics.** A current demand that drives a cell out of its range is solved
  there, on the flat curve, and a cell *already* past empty under a power demand still runs
  on it: the third hour of the 10 W case draws 6.8 A at 372 K (38.9 A and 914 K before).
  The honest fix is a reversal branch for the particle, like the equivalent circuit's
  `[reversal]`, not another guard. ROADMAP H8.
* **The `Dfn` has the heat mix this slice removed from the `Spm`.** Measured: 3.7 K of rise
  in one hour-long C/5 step against 1.1 K in sixty one-minute ones. ROADMAP H8.
* **A power demand met only by driving one weak cell past empty** cannot converge under the
  hold. It raises `SOLVE_UNCONVERGED` rather than landing on the flat curve. Not measured on
  a real case.
* **Speed.** The probe adds one forward sweep per particle and the heat terms two voltage
  and two equilibrium evaluations per cell per step. Not benched; the `Spm` has no bench
  case (ROADMAP H9).
