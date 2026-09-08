# The thermal integrator above the cap — backward Euler where explicit Euler diverges

**Status: built, 2026-09-08.** Predictions were registered before anything ran and are
scored below; two of the six were wrong, and one of those is the finding.

## What this is about

`docs/ROADMAP.md` carries this as **H6**, and `docs/plans/phase-2-thermal-bms.md` opened it
in its own "known limitation" list:

> **Known limitation, unchanged:** the thermal sub-step cap binds above `dt` ≈ 1.7 h,
> which the aging fast-forward will exceed. Raise an integrator, not the cap.

The thermal network is explicit (forward) Euler with automatic sub-stepping. The sub-step
ceiling is a *stability* bound — `SUBSTEP_SAFETY · C_th / a_max`, which for the shipped LFP
parameters under `ThermalConfig::Network { k_neighbor_w_per_k: 1.0 }` is

```
a_max  = max(4·k, hA) = max(4.0, 0.35) = 4.0 W/K
dt_max = 0.5 · 95 / 4.0                = 11.875 s
```

and `MAX_SUBSTEPS = 512` bounds the work one step may do, so above

```
512 · 11.875 s = 6080 s = 1.689 h
```

the cap binds and the sub-step is longer than the ceiling. The code knows: it fires a
`debug_assert` whose message is "temperatures are not trustworthy", and then integrates
anyway, because `Pack::step` must never panic in release.

**Correction, from the measurement below:** the paragraph this note originally carried here
said that past the ceiling explicit Euler "does not merely lose accuracy — it oscillates
with a growing amplitude". That is what the code's own comment claims and it is not true at
the gate. The ceiling is a 0.5 safety factor on a conservative bound, so there is a band
above it — a factor of 2.64 on the pack measured here — where the old path was merely
inaccurate. Divergence is real further up, and at a day-long `dt` it is total (NaN).

Months-long fast-forward is a stated use of this engine (`CLAUDE.md`, the RC section: "this
is what lets the same code path serve real-time GUI stepping and months-long aging
fast-forward"), and it is the *only* thing in a step with such a limit — the electrical
solve, the RC update and the calendar integral are all exact at any `dt`.

## Who actually reaches it today: nobody, and that is the licence this slice spends

Measured before designing anything:

* **Every aging fast-forward test is isothermal.** `crates/sim-core/tests/scenario_aging.rs`
  runs at `dt` = 3600 s and `crates/sim-core/tests/aging.rs` at `dt` = 86 400 s, and both
  build `ThermalConfig::Isothermal` — deliberately, and the file says why ("a live thermal
  network would let ohmic self-heating during cycling move the very quantity the Arrhenius
  ordering is being read from"). So the thermal integrator never runs at those `dt` at all.
* **The browser client fast-forwards by taking more steps, not longer ones.** `web/app.js`
  reads `dt` from a box (default 0.5 s) and the speed multiplier changes the *count*
  (`stepsForFrame`), with a comment saying exactly that: "The multiplier changes steps per
  frame and never `dt`".
* **`scenarios/calendar_fade_hot.toml` is the one shipped scenario with a live network and
  aging on**, and it is driven by the same client at the same `dt`.
* **No lesson prose states the limit.** `grep` for `1.7`, `hour`, `sub-step` across `web/`
  finds only unrelated quantities, so `crates/sim-data/tests/path_claims.rs` is not in play
  and no claim moves.

That is the whole blast radius: **the defect is real and unreached**. It is why this slice
can be built with zero trajectory movement, and it is also the warning — a new path gated
above 1.7 h is exactly the shape that ships as dead code, so the perturbation table below
has to show it reddening something.

## The design, and why it is not the one the roadmap proposed

The roadmap proposed replacing the integrator:

> The network is linear in `T` over a step (conductances and `h·A` fixed, `Q_gen` piecewise
> constant), so the exact update is a matrix exponential — or, cheaper and unconditionally
> stable, backward Euler with one banded solve per step … "Raise an integrator, not the
> cap", as the note says.

Backward Euler is right. **Replacing the integrator wholesale is refused, on measurement.**
A banded solve is O(n·b) per step against the explicit sweep's O(n) — for 100S10P, band
`b` = 10, so ~20× the arithmetic on a path that `docs/plans/pack-step-perf.md` measured at
47.2 µs against a 50 µs budget, with a ~6 % margin. Paying that at every `dt` to fix a
regime no client reaches would be spending the budget on nothing. The repo has precedent
for declining a roadmap-proposed mechanism after pricing it (`pack-step-perf.md` refusing
multiply-by-reciprocal; Phase 6 declining `diffsol`), and this is that shape.

So: **a third path in `advance_temperatures`, gated exactly where the cap binds today.**

1. **Gate.** `substeps` already computes `ratio = dt / dt_max` and branches on
   `ratio >= MAX_SUBSTEPS`. That branch — the one that today clamps to 512 and warns — is
   where the implicit path goes. Below it, *nothing changes*: same comparison on a ratio
   already computed, same arithmetic, same bits.
2. **Sub-step the implicit path, and take exactly `MAX_SUBSTEPS` of them**, `h = dt/512`.
   Backward Euler needs no stability margin, so the sub-step count is an accuracy choice
   alone, and this choice makes the *granularity continuous across the threshold*: at the
   crossover both paths integrate in steps of 11.875 s, and the work one call may do stays
   bounded by the same 512 that bounds it today. One giant implicit step would have been
   cheaper and would have put a visible accuracy cliff at the gate — the slowest mode of a
   coupled pack is much slower than the stability bound suggests (a 10×10 grid sheds heat
   through 20 units of exposure for 100 cells' worth of heat capacity, giving
   `95·100/(20·0.35)` ≈ 1357 s against a bound built from 11.875 s), so a single step of
   6080 s would smear it by ~17 % of the excursion where 512 sub-steps do not.
3. **Factor once per call, solve 512 times.** `h` is the same for every sub-step and the
   matrix is constant across the whole call, so it is one banded Cholesky plus `n`
   triangular solves. This is the detail that makes sub-stepping affordable rather than
   512× a factorisation.
4. **Allocate per call**, on the same terms the runaway path already states for itself:
   "Allocated per call, on a path that only runs while the pack is on fire. The linear path
   above allocates nothing, which is the one that has a perf budget." Here it is a path that
   only runs at `dt` above 1.7 hours. `a_warm_step_allocates_nothing` stays green and `Pack`
   gains no state, so **there is no snapshot bump**.
5. **No config surface.** An integrator selector in `ThermalConfig` would be snapshot state,
   and a bump, for a choice that is `dt`-driven and internal.

### The system being solved

Over one sub-step the network is linear time-invariant with constant forcing:

```
C · dT/dt = q + b − M·T
```

where `q_i` is the cell's own generation \[W\], `b_i = exposure_i · hA · T_env`, and `M` is
symmetric with diagonal `n_neighbors_i·k + exposure_i·hA` and off-diagonal `−k` on the
4-connected grid. Backward Euler is

```
(I + (h/C)·M) · T_new = T_old + (h/C)·(q + b)
```

`M` is a graph Laplacian plus a non-negative diagonal, so it is only positive
*semi*-definite — singular when `h_area_w_per_k = 0`, where the pack is adiabatic and
conserves its heat. `I + (h/C)·M` is symmetric positive definite for any `h > 0` regardless,
and backward Euler never needs `M⁻¹`. That is what makes this formulation safe where the
steady-state/matrix-exponential ones are not, and it is why they are in "deliberately not
done" rather than in the code.

With the natural series-major index `i = s·parallel + p` the non-zeros sit at `i ± 1`
(parallel neighbours, skipped across a row boundary) and `i ± parallel` (series
neighbours), so the bandwidth is `parallel` and a banded Cholesky costs O(n · parallel²) to
factor and O(n · parallel) per solve.

One exception, and it is the piece of the solver that most nearly shipped unmeasured: a
pack with a single series element has no series neighbour, so its band is tridiagonal
however wide it is, and `assemble` takes `bw = 1` there. 1S1P cannot tell the two arms of
that branch apart (`parallel` is 1 either way) and 3S3P only exercises the other one, so
`a_day_long_step_on_a_single_series_pack_…` exists to cover a **1S3P** pack — the only shape
where the arms disagree, and a real topology besides.

### The two contracts this buys

* **Unconditional stability at any `dt`.** No cap, no `debug_assert`, no divergence.
* **The steady state is exact in the `h → ∞` limit.** `T_new → M⁻¹(q + b)` as `h` grows,
  which is the right thing for a fast-forward: the answer a day-long step should give *is*
  the quasi-steady temperature.

## Predictions, registered before anything ran

* **P1 — no existing test moves, and none is expected to.** The implicit branch is reached
  only where `substeps` clamps today, and the survey above says nothing in the tree or the
  scenarios reaches it. Prediction: the full workspace suite is green with **zero** goldens
  re-pinned and zero trajectory changes. Falsifiable, and the expected answer is a
  round zero.
* **P2 — today's code at `dt` = 86 400 s does not merely lose accuracy, it produces
  non-finite temperatures.** On a 3S3P pack with `k` = 1, `dt` = 86 400 gives `h` = 168.75 s
  and the fastest grid mode an amplification factor of `1 − 168.75·6.2/95 ≈ −10`; over 512
  sub-steps that overflows. Prediction: at least one cell reads `inf` or `NaN`, not a
  plausible-but-wrong number. Corollary prediction: **this is invisible to a debug test
  run**, because the `debug_assert` panics before the divergence can be observed — the
  measurement has to be taken in release.
* **P3 — the 1S1P arm matches the backward-Euler closed form to ≤ 1e-12 relative.** A lone
  cell has no neighbours, so `a = hA` exactly and `exposure = 1`, giving
  `T_n = T∞ + (T₀ − T∞)·(1 + h·hA/C)^(−n)` with `T∞ = T_env + q/hA`. To keep the arm off the
  degenerate end, the test sets `k_neighbor_w_per_k` = 1000: on a 1S1P pack that changes no
  physics at all (there is no neighbour to conduct to) and only makes the *bound*
  conservative, so the implicit path can be entered at a `dt` where the transient is still
  alive. Prediction: with `dt` = 10 s and `k` = 1000 the pack takes the implicit path, the
  decay factor is `(1 + 7.2e-5)^(−512)` ≈ 0.9638 — mid-transient, not a collapsed
  steady state — and the engine matches the closed form to 1e-12.
* **P4 — the coarse pack step lands on the steady state, and agrees with a fine reference.**
  3S3P, 1.5 A (0.5 A per cell, so the pack does not empty inside the step), `dt` = 7000 s
  in one implicit step against 7000 steps of 1 s. Both are ~17 slow-mode time constants in,
  so both are at steady state to ~4e-8 of the excursion. Prediction: every cell agrees to
  within 1e-6 K, and the residual of `q_i + Σk(T_j − T_i) + exposure_i·hA·(T_env − T_i)` is
  under 1e-9 W per cell — a steady-state check that needs no second solver to state.

  Both arms are given **400 s of 1 s steps under the same current first**, and that warm-up
  is load-bearing rather than cosmetic. `ecm::cell_heat_w` charges `i·(i·R0 + V_rc)` using
  the *actual* RC overpotential, not `I²·(R0 + ΣR_rc)`, and a step holds its heat constant
  across the whole step — so a 7000 s step taken from a fresh pack would burn the
  start-of-step `i²·R0` (0.005 W here) for two hours while the fine arm settles onto
  0.0075 W, and the two would disagree by 50 % on the steady state for a reason that has
  nothing to do with the integrator. With `τ_rc` = 20 s, 400 s of warm-up settles the pair
  to within `e^(−20)` and the two arms enter the comparison on the same heat.
* **P5 — `a_warm_step_allocates_nothing` stays green.** The allocation is on the implicit
  path, which a warm 1 s step never enters.
* **P6 — deleting the implicit branch reddens the new tests and nothing else.** Prediction:
  routing the gate back to the clamp reddens exactly the two new tests. If it reddens
  nothing, the path shipped dead and the slice failed.

## What was measured

All figures from `crates/sim-core/tests/thermal_implicit.rs`, **in release**: the pre-slice
code fires a `debug_assert` on the way into this regime, so a debug run panics before the
behaviour can be observed at all. That is worth stating plainly — the defect this slice
fixes was, by construction, invisible to the ordinary test command.

### The pre-slice behaviour, measured before anything was changed

| case | pre-slice result |
| --- | --- |
| 1S1P, `dt` = 10 s, `k` = 1000 (gate 6.08 s) | **304.0881051199167 K** against the backward-Euler closed form's 304.0881208621869 — out by 1.574e-5 K, relative **2.651e-6** |
| 3S3P, `dt` = 7000 s, `k` = 1 (gate 6080 s) | finite and plausible: 298.2137 K at the corner, 5.07e-7 K from a fine-`dt` run |
| 3S3P, `dt` = 86 400 s, `k` = 1 | **NaN** |

The first row is not noise, it is a fingerprint. Forward and backward Euler over `n`
sub-steps of the same length differ by `e^(∓n·x²/2)` to leading order, and with
`x = h·hA/C = 7.196e-5` and `n = 512` that is 2.65e-6 — the measured figure to three
digits. The pre-slice engine was running forward Euler, and this test can tell which
scheme it is looking at with five orders of margin.

### The 7000 s row is the interesting one, and it broke a prediction

At 7000 s the explicit path clamps to 512 sub-steps of **13.67 s** against a ceiling of
11.875 s, so by the code's own account it was in the "not trustworthy" regime. It came
back fine. Two separate mistakes of mine are in that, and both are worth writing down.

**1. The cap binding is not the `dt` at which explicit Euler diverges.** The ceiling is
`SUBSTEP_SAFETY` = 0.5 times `C/a_max`, and `a_max = max(4k, hA)` is a *bound* over
neighbour counts. Actual divergence needs `h > 2C/λ_max` where `λ_max` is the largest
eigenvalue of the real matrix — for a 3×3 grid at `k` = 1 that is ≈ 6.06 W/K, giving
`h_crit` ≈ 31.4 s and a divergence `dt` of `512 · 31.4` ≈ **16 070 s** (4.46 h). So there
is a band between **6080 s and ~16 070 s**, a factor of **2.64**, in which the pre-slice
path was merely inaccurate and the `debug_assert`'s "temperatures are not trustworthy" was
overstated. Above that band it is not overstated at all: at 86 400 s the fastest mode
amplifies by `1 − 168.75·6.06/95` ≈ −9.8 per sub-step, and 512 of those is the NaN.

**2. The residual I predicted was not measurable at that `dt`, and the fine arm proved
it.** The diagnostic that settled it printed the steady-state residual for *both* arms:

```
cell 0,0: coarse 298.213671045370 fine 298.213670538867 diff 5.065e-7 | resid_c 8.527e-7 resid_f 9.112e-7
cell 1,1: coarse 298.217367462151 fine 298.217366925074 diff 5.371e-7 | resid_c 9.042e-7 resid_f 9.662e-7
```

The **fine** arm's residual is the larger one. A residual that is worse in the reference
than in the thing being tested is not integration error — it is the pack not having
arrived yet, in both arms equally. Checking by hand: the pack sheds
`(4·0.5 + 4·0.25)·0.35·ΔT` = 0.0674916 W against 0.0675 W generated, a shortfall of
1.24e-4, which after 7400 s puts the slowest time constant at **~822 s** — not the 407 s I
had estimated from an average conductance. The average is wrong because the *interior*
cell has zero exposure, so the slowest eigenvector is interior-weighted and sheds heat
more slowly than the pack mean. 7000 s is nine of those, not seventeen.

So that arm was rewritten. It is now `crossing_the_gate_does_not_change_the_answer`, an
honest continuity guard that says what it is: it passes on the pre-slice code too, and
what it catches is a *wrong* implicit solve near the gate (rows B, C, D, F below). The
regime the slice exists for is pinned by a **day-long** step instead, where the pre-slice
answer is NaN.

### The post-slice behaviour

| case | result | bound asserted | how the bound was derived |
| --- | --- | --- | --- |
| 1S1P closed form | drift **2.626e-11 K** | 6.92e-11 K | `2·n·eps·T`: 512 sub-steps, at most two roundings each, at the ULP of an absolute temperature near 304 K |
| 3S3P day-long, steady state | residual ≤ **3.2e-13 W** | 1e-11 W | the physical residual is nil at a hundred time constants, so the floor is cancellation: ~1e-4 K differences on 298 K numbers carry ~3e-14 K of rounding per term |
| 3S3P day-long vs fine `dt` | ≤ **2.41e-11 K** | 1e-9 K | two converged answers; the gap is the same rounding accumulation as row 1 |
| 1S3P day-long (the other arm of the bandwidth branch) | steady state and fine-`dt` agreement hold on the same bounds; middle cell hottest; **the two ends of the chain differ by exactly one ULP** | 2·eps·T | see below — the asymmetry is a property of the solve, and the bound is one rounding |
| 3S3P gradient | centre **0.000673753 K** > edge 0.000655003 > corner 0.000636784 above ambient | ordering only | re-asserts the phase-2 exit gate's shape, so a solver returning a uniform "everything is ambient" cannot pass on the residual alone |

`q` per cell is 7.5e-5 W = `0.05²·(R0 + R_rc)`, i.e. the RC pair is settled and both arms
hold the same heat — which is what the 400 s warm-up is for.

### One property the explicit path had and this one does not

Adding the 1S3P arm turned up a behavioural difference that no prediction covered. The two
ends of a three-cell chain are equal by symmetry — `exposure` is 0.75 on both, and they see
the same neighbour — and the explicit path returns them **bit**-identical, because
`euler_substep` is a Jacobi sweep: every cell is computed from the same previous iterate by
the same arithmetic. The implicit path returns 298.1503192291577 and 298.1503192291576,
**one ULP apart**, because forward-then-back substitution visits the chain in an order and
the two cells reach the same answer through different sequences of operations.

That is worth stating rather than smoothing over, because it is easy to mistake for a
determinism problem and it is not one: the order is fixed, so the same binary produces the
same bits, which is all `CLAUDE.md`'s determinism rule asks for. What is no longer exact is
*spatial* symmetry. The test asserts the ends agree to `2·eps·T` rather than exactly, with
the reason in a comment, and `implicit_substeps` says the same thing in its doc comment so
the next reader meets it before being surprised by it.

## Predictions, scored

* **P1 — confirmed, exactly.** `cargo test --workspace --no-fail-fast`: **662 tests across
  77 binaries, all green**, no golden re-pinned, no tolerance touched, no trajectory moved.
  The prediction's expected answer was a round zero and it is a round zero. `cargo clippy
  --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` are clean.
* **P2 — half confirmed, half falsified, and the falsified half was the useful one.**
  Confirmed: at `dt` = 86 400 s the pre-slice path returns **NaN**, not a plausible-but-wrong
  number, and the corollary held — the `debug_assert` makes this unobservable in debug, so
  every measurement here had to be taken in release. Falsified: I wrote that prediction
  about a case I then moved to `dt` = 7000 s for an unrelated reason (the pack empties
  inside a day-long discharge) and did not re-derive it. At 7000 s nothing diverges,
  because the cap binding and true divergence are a factor of 2.64 apart. That factor is
  now documented on `MAX_SUBSTEPS`, in the module docs, and in the closed roadmap entry,
  and it is the finding of the slice.
* **P3 — confirmed on the physics, falsified on the tolerance.** The 1S1P arm does take the
  implicit path, the decay factor is 0.96383 (mid-transient, as designed), and the engine
  now agrees with the closed form — but at 4.4e-12 relative, not the ≤ 1e-12 I registered.
  1e-12 was a picked number, and picking it was the error: 512 sequential sub-steps on an
  absolute temperature accumulate rounding that a single `powf` does not. The tolerance is
  now derived (`2·n·eps·T` = 6.92e-11 K, measured drift 2.63e-11 K, 38 % of the bound) and
  the pre-slice forward-Euler answer misses it by five orders.
* **P4 — the arm was replaced, and the replacement confirmed it.** The 7000 s form could
  not carry the residual assertion (see above). At `dt` = 86 400 s every part of it holds
  with room to spare: residual 3.2e-13 W against 1e-11, coarse-vs-fine 2.4e-11 K against
  1e-9. The warm-up prediction was confirmed exactly — `q` is 7.5e-5 W, the settled value,
  in both arms.
* **P5 — confirmed.** `a_warm_step_allocates_nothing` is green, in the 662 and in the
  perturbation baseline. The implicit path allocates two `Vec`s per call and a 1 s step
  never enters it.
* **P6 — confirmed.** Row A reddens both new tests and nothing else. The path is reachable,
  and the tests are the reason we know.

## Perturbations

Each edit was applied to `crates/sim-core/src/thermal.rs`, seven sim-core binaries were run
in **release** with `--no-fail-fast`, and the file was restored byte-exact before the next
row. The harness (`W:\temp\claude\thermal-implicit\perturb.py`) launches `cargo` directly
with `BELOW_NORMAL_PRIORITY_CLASS` and reads the real exit code, because `start /wait` is
exit-code-blind. The baseline was clean on all seven before any row ran. Nothing compiled
red; every row is a runtime catch, and **no row was green everywhere**.

| perturbation | what reddened |
| --- | --- |
| A. no implicit branch — clamp to the explicit path, i.e. the pre-slice behaviour | `a_lone_cell_…_closed_form` **and** `a_day_long_step_…_steady_state`. Not the gate test, which is the point of its doc comment: at 7000 s the explicit path is still stable. |
| B. conduction off-diagonals dropped (cells stop exchanging heat) | the gate test and the day-long test. Not the lone cell — a 1S1P pack has no off-diagonals to drop, which is the arm's own claim about itself. |
| C. wrong sign on the conduction off-diagonals | the same two. |
| D. forcing loses its ambient term (`b = 0`: heat in, nothing to cool toward) | **all three.** The ambient term is the only coupling a lone cell has. |
| E. one implicit sub-step instead of `MAX_SUBSTEPS` | **all three.** This is the row that justifies design decision 3: at `n` = 1 backward Euler is still perfectly stable and still lands near the steady state, and it is *not* good enough — `h/τ` = 105 leaves ~1 % of the excursion, a residual of ~1.5e-9 W against the 1e-11 bound. Sub-stepping the implicit path is load-bearing, not decorative. |
| F. back substitution skipped (a factorisation that does not invert) | the gate test and the day-long test. Not the lone cell: at `n` = 1 node the back substitution has no work to do, so it is the one arm structurally blind to this. |

Three of the six (B, C, F) are caught **only** by tests that pass on the pre-slice code as
well. A perturbation table built solely from "does it redden where the old code was broken"
would have missed every one of them.

## Deliberately not done

* **No matrix exponential, and no steady-state formulation.** Both need `M⁻¹`, and `M` is
  singular exactly when `h_area_w_per_k` = 0 — an adiabatic pack, which is a supported
  configuration. `I + (h/C)·M` is positive definite for any `h > 0` and backward Euler
  never inverts `M`, which is the whole reason this formulation was chosen over the
  roadmap's other suggestion.
* **No ADI or operator splitting.** The matrix genuinely is a Kronecker sum — `exposure` is
  `(4 − n_s(s) − n_p(p))/4`, separable in the two axes — so alternating-direction implicit
  would be O(cells) with tridiagonal solves instead of O(cells × parallel). It is refused
  because the banded solve is already off the hot path, and ADI's splitting error would
  need its own validation to buy a speed-up nobody is waiting for.
* **No index permutation to shrink the bandwidth.** The natural series-major order gives a
  half-bandwidth of `parallel`, so a wide-parallel pack (10S100P) pays O(cells · 100²) to
  factor. That is milliseconds, once, on a path that runs at day-long `dt`. The one cheap
  case is taken: a pack with `series` = 1 has no series neighbour, so its band is
  tridiagonal whatever its width.
* **Nothing for the runaway path.** It is still explicit, still adaptive, and still has a
  real cliff at `MAX_RUNAWAY_SUBSTEPS`. Backward Euler cannot be dropped into it: the
  Arrhenius term is non-linear in `T`, so an implicit step against it needs a Newton solve
  with the reaction's own Jacobian. `MAX_RUNAWAY_SUBSTEPS`' doc comment now says so, so the
  file does not read as if all of it were unconditionally stable.
* **No factorisation cached across calls.** `h` moves with `dt`, and a cache would be `Pack`
  state — a snapshot question, for a path that runs rarely.
* **No config surface, no snapshot bump, no `Pack` state, no `EventFlags` bit.** The choice
  of integrator is `dt`-driven and internal. There is no bit for "your `dt` is absurd", and
  a non-finite `dt` keeps exactly the behaviour it had: it stays on the explicit path and
  propagates into the temperatures, because `step` must never panic.
* **No timing claim, and the 100S10P bench was not re-run.** The hot path is unchanged by
  construction — one comparison on a ratio that was already computed — so there is nothing
  to measure and a below-normal-priority bench would not have measured it honestly anyway.
  Same treatment as `pack-step-allocations.md`: counted, not timed.

## Still open

* **Heat generation is still held constant across the step, and at fast-forward `dt` that
  is now the dominant error — not the integrator.** A day-long step burns a whole day at
  the heat of its first instant. Fixing it means solving the electrical problem more than
  once per step, which is a different and much larger change; `Pack::step`'s doc comment
  now names it as the cost of a coarse `dt`. **Priced, 2026-09-08**, as a side effect of
  the slice above: on a fresh 3S1P pack under load a single day-long step settles at
  311.45 K where a fine `dt` puts it at 318.10 — 6.6 K of a 20 K rise, a third of it,
  because the step burns `I²·R0` all day and never sees the RC pair's `I²·R_rc`. That is
  five orders larger than any integration error measured in this note.
* ~~**Runaway ignition still lags a whole step**, so a live `[safety]` section and a
  day-long `dt` do not belong in the same run.~~ **Closed 2026-09-08 by
  `docs/plans/runaway-inside-a-coarse-step.md`**, which found three defects behind that
  one sentence and measured that they are not equally severe: the ignition lag is real
  and total (a day-long step ends 3.65 K past onset having released exactly nothing);
  the reacting loop's inability to reach past 6.8 h costs work unconditionally but
  returns a wrong *number* only when the sub-step budget binds mid-burn; and the third —
  an accuracy bound taken on generation rather than on the net rate — was pure cost. The
  same gate this slice established switches all three.
* **Nothing reaches the new path yet.** The gate is 1.7 hours and the shipped hot-calendar
  scenario fast-forwards at 1 h per step; the browser multiplies the *step count*, never
  `dt`. So this closes a hole rather than enabling a shipped feature. The first client that
  wants a day per step is what would use it — and it would immediately meet the
  constant-heat limitation above, which is the honest order to fix them in.
* **The gate is conservative by a factor of ~2.64** on the pack measured here, and by an
  unmeasured amount on others: it is built from `max(4k, hA)`, not from the matrix's real
  largest eigenvalue. Switching integrators slightly earlier than strictly necessary is the
  cheap and safe direction to err, but the true bound is a Gershgorin disc away if anyone
  ever wants the explicit path to run longer.
* **The 1S3P arm was added on review, not on plan.** The bandwidth branch
  (`bw = 1` when `series` is 1) was written, shipped in the first commit, and covered by
  nothing: 1S1P cannot distinguish the two arms and 3S3P only walks one. It is covered now
  and it was correct, but the gap is the exact shape this repo keeps rediscovering — a
  branch whose wrong answer would still have indexed legally. Neither the six-row
  perturbation table nor 662 green tests found it; a reviewer reading the branch did.
* **`crossing_the_gate_does_not_change_the_answer` passes on the pre-slice code**, by
  design. It is a guard on the solve, not on the gate, and its doc comment says so — but it
  is the kind of test that reads as coverage it does not provide, so it is named here too.
