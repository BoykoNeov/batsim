# The solver blowing up on a voltage target outside a cell's range

Status: **done**. `SNAPSHOT_VERSION` unmoved at 13; no `Telemetry` field added; no new
`EventFlags` variant.

## The report

A `Demand::Voltage` whose target lies outside a cell's range made the pack solve produce
numbers no client could use. Reproduced on a 1S1P LG M50 at 50 % SOC, `dt = 1 s`:

| demand | model | reported current | reported terminal voltage |
| ------ | ----- | ---------------- | ------------------------- |
| `Voltage(-100)` | `Dfn` | **1.58e101 A** | -1.75e95 V |
| `Voltage(1e6)` | `Dfn` | **-6.36e105 A** | 7.06e99 V |
| `Voltage(-100)` | `Spm` | 2.84e12 A | -2.36 V |
| `Voltage(1e6)` | `Spm` | -3.03e16 A | 9.00 V |
| `Voltage(0)` | `Spm` | -2.88e8 A | 7.10 V — **wrong sign**, charging when asked to discharge |
| `Voltage(1e30)` | `Ecm` | -5e31 A | 1.07e30 V |

Every porous-electrode case raised `SOLVE_UNCONVERGED`, so no step was ever *silently*
wrong. But a flag meaning "treat this voltage as approximate" is only useful when the
voltage is otherwise in the right neighbourhood, and 1e95 V is not.

## What the measurement changed about the diagnosis

Three things were assumed at the start and all three were wrong.

**1. "Outside the cell's range" is not what separates the failures.** `Spm Voltage(7)`
*converged* — residual under `SOLVE_TOL_V` — at 107 megaamps, hitting 7.00000 V exactly.
`Dfn Voltage(0)` converged at 283 A while `Dfn Voltage(0.5)` diverged to 1e97. Convergence
of the fixed point is not evidence of a physical operating point, and the diverging set is
not the out-of-range set. Anything built on "detect the unreachable target" would have been
built on a distinction that does not exist.

**2. There is no "strongest current the pack can deliver" latent in the physics.** The
first design was to clamp where the `V(i)` curve goes flat. It never goes flat. Measured by
`dt = 0` probes at `i = ±2^k`, `k = 0..39` — every one of which *converged*, because
`Demand::Current` needs no root find:

| model | shape of `V(i)` at large \|i\| |
| ----- | ------------------------------ |
| `Ecm` | linear, unbounded (`V = E + \|i\|·R`) |
| `Spm` | **logarithmic — a fixed 71.23 mV per doubling of current**, unchanged out to 5.5e11 A |
| `Dfn` | asymptotically linear, unbounded |

So every voltage is reachable and "unreachable" is not a claim this engine can make. The
`Spm`'s 71.23 mV per doubling is the Butler–Volmer `asinh`, and it is why 7 V costs 1e8 A
and 10 V costs 1e30: the cost is exponential in the target, which is exactly the shape that
makes the question worth refusing rather than answering.

**3. The `Dfn`'s unrecoverable state is not a poisoned Newton seed.** `DfnState::u` is the
next step's starting guess and is committed unconditionally, so the first theory was that a
diverged `u` poisoned every later step — `solve` already re-seeds the `c_e` slots for
exactly this reason. Dumping the state showed the concentrations themselves are impossible:

| field | fresh | after `Current(1e9)` |
| ----- | ----- | -------------------- |
| `c_e` | 1000 | **-9.17e4** … 1.06e5 mol/m³ |
| `c_neg` | 15 522 | **-1.41e5** … 15 522 mol/m³ |
| `c_pos` | 35 269 | 35 269 … 2.72e5 mol/m³ |

There is nothing to re-seed *from*: the guess is rebuilt from `c_e`, and `c_e` is negative.

## What was built

### A. A backtracking damping line-search on the pack solve — `pack.rs`

The loop was plain successive substitution. Where a tangent is shallower than the secant it
stands in for, the prediction overshoots, and the next pass — taking its tangents further
out — overshoots by more. Each pass is now accepted only if it *reduces* the residual,
backtracking towards the last accepted current by halving until it does. The accepted
residuals are therefore a strictly decreasing sequence and the iterate cannot run away.

The halving happens at the top of each attempt, following `dfn::solve`, so `i_cell`,
`probed` and `residual_v` all describe the current finally taken — which is what keeps the
reporting pass's existing `debug_assert` (that each cell advances at the current it was
probed at) satisfied.

*Damping cannot un-refuse a refused current.* The trial is a convex combination of two
currents protection already passed, and `apply_protection`'s **allowance** does not depend
on the iterate — its limits come from the chemistry and the pack capacity, and its hard
trips read the sensor frame. Both endpoints lie in one interval, so every point between
them does. A latched contactor is the degenerate case: both endpoints are exactly `0.0`. A
`debug_assert` pins `λ ∈ [0, 1]`, which is the part a future edit could break.

`DAMPING_ATTEMPTS = 16`, measured. Sweeping the whole in-window voltage band (81 targets
from `v_min` to `v_max`) across five pack states — 1S1P at 2 %, 50 % and 98 % SOC, a
scattered 1S3P, and a 4S2P — under both porous-electrode models, so 810 solves, and counting
those finishing on `SOLVE_UNCONVERGED`:

| attempts | 1 (off) | 4 | 8 | 12 | 13 | **16** | 20 | 24 |
| -------- | ------- | - | - | -- | -- | ------ | -- | -- |
| unconverged | 55 | 40 | 34 | 12 | 11 | **11** | 13 | 13 |

The knee is at 13. **Deeper is worse**, which is the interesting part: past 16 the count
goes back up, because a search allowed more halvings can accept a step small enough to be no
step at all — it satisfies "reduced the residual" while barely moving, and the solve then
runs `SOLVE_ITER_CAP` out creeping. The constant is bounded on both sides by measurement.

### C. A demand window on `Demand::Voltage` — `pack.rs`, `lib.rs`

The target is clamped to `series × [chem.cell.v_min, chem.cell.v_max]` before the solve,
hoisted outside the iteration so in-window arithmetic is untouched. No new constant:
`v_min`/`v_max` are required `[cell]` fields carrying provenance (Chen2020's own cut-offs
for the shipped LG M50). No new flag: the step reports the voltage it held and the current
it drew, which is the difference itself, and `SOLVE_UNCONVERGED` already carries two
meanings.

Scaling by `series` is what makes it a *pack* terminal window, which is what
`Demand::Voltage` means. This is what moved `nonlinear_solve_fast_path.rs`: its schedule
asked 4S3P packs for 3.30 V and 3.05 V against an 11.60–14.00 V window — legal, but a 392 A
/ 26 C discharge rather than the voltage hold the numbers look like. `schedule()` is now
series-aware, so each case asks the same thing *per cell* that the 1S1P case asks.

**The capability given up, stated out loud:** a voltage demand can no longer drive a pack
outside its declared window, so it can no longer express "hold this cell above `v_max`
until it vents". That is moved, not lost — `Demand::Current` is unrefusable by every cell
model and reaches every one of those states.

## What was not built, and why

### B. The `Dfn`'s unrecoverable state

`Demand::Current(1e9)` for one second pushes 1e9 A·s through a cell holding ~18 551 A·s —
54 000 times its entire lithium. The negative concentrations are the correct integral of an
impossible demand, and every later step then solves a cell containing negative lithium:
`Current(1e9)` leaves a `Dfn` reporting **-1105.62 V forever**, on rest steps, indefinitely.

It was scoped out on two measurements.

**It is not reachable from a voltage target, at any `dt`.** The worry was coarse-`dt`
fast-forward: `Voltage(2.5)` draws ~102 A at `dt = 1`, which at `dt = 3600` would be twenty
times the cell's lithium. It does not happen, because a voltage target *self-limits* — as
the cell depletes it can no longer hold 2.5 V at high current, so the current falls:

| dt | V\* | current | charge moved | min `c_e` | recovers on rest? |
| -- | --- | ------- | ------------ | --------- | ----------------- |
| 1 | 2.5 | 102.1 A | 1.02e2 A·s | 716 | yes |
| 60 | 2.5 | 25.3 A | 1.52e3 A·s | **-11.05** | yes → 3.454, 3.455, 3.457 V |
| 600 | 2.5 | 39.3 A | 2.36e4 A·s | 634 | yes |
| 3600 | 2.5 | 26.7 A | 9.61e4 A·s (5× capacity) | 901 | yes |

**And the obvious guard is disqualified by that same table.** "Refuse to commit a state
whose concentration crossed zero" would fire on the `dt = 60` row — a case that works
correctly today and recovers on its own — and `C_E_FLOOR_MOL_PER_M3`'s own doc records the
3C reference run reaching `-0.0007`. So crossing zero is not the admissibility line. The
line between `-11` (fine) and `-91 676` (fatal) is a *magnitude*, i.e. an invented constant,
which this repo's provenance rule makes expensive and which nothing here justifies.

Gating on `!converged` instead is also wrong: `cc_discharge_3c_dfn.toml`'s depletion tail
has unconverged steps that commit correctly today, and refusing them would move that golden.

So this is a documented limit of the deliberately-unrefusable `Demand::Current` channel,
asserted as such in `solve_safeguard.rs`'s `a_pack_survives_a_demand_it_cannot_meet`.

### A bracketed root find, to close the last 11 of 810

The remaining unconverged in-window solves are all one configuration: a scattered 1S3P `Spm`
asked to hold a voltage on the knee of its own discharge curve, where a small voltage change
needs a large current change. They are **bounded and flagged** — worst 949 A and 4.207 V.

Closing them wants a bracketed root find on a *demand* residual. Declined: bracketing needs
a monotone scalar residual, and what this loop measures is tangent self-consistency, which
is not that. Adding a demand-satisfaction residual is a different solve, and on a scattered
parallel group the per-cell split is coupled to `i_g`, so the "1-D monotone" premise is not
established for the one case that actually fails. It would also cost the containment the
damping has and a redesign could not: the search touches exactly one pre-existing test
file and no golden at all (see Verification), where replacing the solve would put every
committed porous-electrode trajectory in scope at once.

### A window on `Demand::Power`

`Power` still reaches large numbers — `Spm Power(-1e12)` gives -1.2e11 A, `Dfn Power(-1e6)`
reports 2952 V, both flagged. The symmetric fix (solve, then re-solve at the violated
voltage bound) was declined because a power demand that sags a pack below `v_min` is
**ordinary physics** and the `UV` flag exists for it, unlike an out-of-window voltage
*hold*, whose answer is dominated by the arithmetic of the overpotential. Bounding it would
cost a real teaching case to fix a demand nobody issues at 1e12 W.

> **Corrected 2026-08-12 by `docs/plans/power-operating-point.md`. Two of the three claims
> in that paragraph do not survive measurement.**
>
> * *"both flagged"* was an accident of which model was probed. The equivalent circuit is
>   the one that matters and it is **silent**: `Power(-1e12)` over a step short enough that
>   nothing fills answers 6.3e6 A at 162 kV with an empty flag set. `SOLVE_UNCONVERGED`
>   cannot reach it — the closed form runs no iteration to fail — and the
>   `SOC_CLAMPED_HIGH` seen at longer steps is incidental to step length.
> * *"the `UV` flag exists for it"* is true only of a pack with protection configured.
>   `EventFlags::UV` is raised in exactly two places, both in `bms.rs`; with `bms: None`
>   nothing reported an out-of-window operating point at all.
>
> The conclusion — **do not bound it** — stands, and for a reason this paragraph did not
> give: the two directions are not symmetric. Discharge power has a maximum at
> `V = e/2`, so the closed form's snap is correct physics; charge power has none at all,
> so any magnitude is met exactly at an unbounded operating point. The fix is therefore a
> report (`EventFlags::POWER_OUT_OF_WINDOW`), not a clamp, and no capability is lost.

## Result

Every voltage target on the original sweep now converges, on all three cell models,
including -100 V, 1e6 V and 1e30 V. Currents run -39 A to +286 A; passes run 1 to 6.

Across the 810-solve in-window sweep, worst case over the whole sweep:

| | before | after |
| - | ------ | ----- |
| unconverged | 55 / 810 | 11 / 810 |
| max \|i\| (1S1P `Spm` at 98 % SOC) | **2.858e9 A** | 5.975e2 A |
| max \|i\| (scattered 1S3P `Spm`) | **2.382e9 A** | 9.490e2 A |
| max \|v\| | 7.337 V | 4.200 V |

Four of the five pack states that previously failed now pass entirely, including one that
had been returning the **wrong sign** (charging at -43.9 A when asked to discharge to
2.840 V) and one at -2.86e9 A.

## Verification

- **Damping fires 465 times on the pre-existing suite, and every one of them is in
  `sim-data/tests/nonlinear_solve.rs`** — the one file whose job is the nonlinear solve.
  Every golden file fires **zero** times (analytic, SPM, DFN, the PyBaMM comparisons),
  which is why no committed trajectory moved. `nonlinear_solve.rs` is green throughout,
  including its `worst <= 8` pass-count bound and its `gap < 1e-5` convergence assertion:
  the search changes the *path* the iteration takes, not the fixed point it reaches, and
  that file's assertions are bounds and tolerances rather than bit pins.

  **The first version of this measurement was a lie, and the shape of the lie is worth
  keeping.** `cargo test` captures `eprintln!` for *passing* tests, so an instrumented
  run counted **zero fires everywhere** — including in the new tests that exist
  specifically to make damping engage. That zero was read as "provably inert" and written
  into this document before the contradiction was noticed. The fix is `--nocapture`, and
  the guard is a validation case with a *known non-zero* answer: `solve_safeguard.rs`
  alone fires 830 times, and `demand_window.rs` — equivalent-circuit only, so a linear
  pack that never enters the loop — fires 0. A probe that cannot be seen failing is not a
  probe. (This is the same family as the existing lesson that `cargo` splits `Running` to
  stderr and `test result:` to stdout: never trust a count from a harness you have not
  seen produce a non-zero.)
- **Both new test files were perturbed and both caught it, by exit code.** Neutering the
  clamp fails 4 of 6 in `demand_window.rs` (exit 101); the 2 that survive are the in-window
  pass-through assertions, which *should* hold with no clamp — they are inertness guards,
  not clamp guards. Neutering the damping acceptance test to `if true` fails
  `damping_bounds_a_power_demand_no_window_can_help` at -415 746 A (80 677 C), and leaves
  the other four green — confirming the window and the search are separable and separately
  tested.
- `cargo test --workspace` green, exit 0.
- `SNAPSHOT_VERSION` unmoved: no field was added to any serialized type, and the damping
  state (`residual_prev`, `i_g_prev`, `i_short_prev`) is three locals inside `step`.

## For the next reader

- `Telemetry::v_terminal` is an **end-of-step** read. A voltage-demand test must never
  assert `v_terminal ≈ target` at `dt > 0` — a step that ran 121 A for a second has moved
  the SOC, and through the `Spm`'s discharge knee that is 186 mV. Assert at `dt = 0`.
- A `Demand::Current` probe *always* converges, because it needs no root find. Sweeping
  `V(i)` that way measures the curve, never the solver — which is what made it the right
  instrument for finding out that the curve has no asymptote.
