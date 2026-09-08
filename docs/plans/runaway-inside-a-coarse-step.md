# Ignition inside the step — thermal runaway at fast-forward `dt`

**Status: built, 2026-09-08.** Predictions were registered before anything ran and are
scored below. One of the seven was wrong, and the wrong one is the finding.

## What this is about

`docs/plans/thermal-implicit-integrator.md` closed the integrator hole (roadmap **H6**)
and left two items open in the same paragraph. This note takes the second:

> **Runaway ignition still lags a whole step**, so a live `[safety]` section and a
> day-long `dt` do not belong in the same run. Also documented on `Pack::step` rather
> than left implied.

`crates/sim-core/src/runaway.rs` states the limitation itself, and states the condition
under which it should be revisited:

> Whether the reaction runs during a step is decided from **start-of-step**
> temperatures. … Revisit if a scenario ever needs `dt` coarse enough that a cell can
> cross onset and reach vent inside one step.

That is exactly the `dt` the previous slice made legitimate for everything *else* in a
step, so the condition it named has arrived.

## The three defects, and their arithmetic

All figures for the shipped LFP thermal parameters (`C_th` = 95 J/K, `hA` = 0.35 W/K)
under `ThermalConfig::Network { k_neighbor_w_per_k: 1.0 }`, where
`a_max = max(4k, hA)` = 4.0 W/K and the explicit stability ceiling is
`SUBSTEP_SAFETY·C/a_max` = 11.875 s.

**D1 — ignition lags one whole step.** `advance_temperatures` decides `reacting` once,
from start-of-step temperatures. A cell that crosses `t_onset_k` during the step
therefore releases nothing until the *next* call. At `dt` = 0.5 s that is a rounding
error on an event that takes seconds; at `dt` = 86 400 s the pack sits a day past onset
with the reaction switched off, and `EventFlags::THERMAL_RUNAWAY` is a day late.

**D2 — a reacting pack cannot integrate a coarse step at all.** The adaptive path bounds
*every* sub-step by `SUBSTEP_SAFETY·C/(a_lin + slope)`, and `a_lin` = 4.0 W/K is there
whether or not anything is reacting. So its whole reach is

```
MAX_RUNAWAY_SUBSTEPS · 11.875 s = 2048 · 11.875 = 24 320 s = 6.76 h
```

A day-long step is **3.55×** past that. Beyond it the loop takes one explicit Euler jump
of 62 080 s on a system whose single-node stability limit is `2C/hA` = 543 s — an
amplification factor of ~228 in one jump, guarded by a `debug_assert` and nothing else,
so in release it is silent.

**Correction, from the measurement below.** This paragraph originally ended "the result
is not a temperature", and that is only true some of the time. Explicit Euler amplifies
the distance from the fixed point of the *frozen* system, and a pack that has spent
6.8 hours relaxing is sitting on it, so the jump multiplies a residual of ~1e-11 K and
returns a correct answer. The failure is real but it needs the cap to bind **mid-burn**,
which needs a burn bigger than the whole budget. Measured there: 1.8 million kelvin. The
cost, on the other hand, is unconditional — the loop spends 2048 sub-steps on stability
it does not need in every one of these cases.

**D3 — the stability bound is paid where it buys nothing.** The 11.875 s ceiling is a
property of the *linear* conductances, and the previous slice already built the
integrator that removes it. A pack hovering just above onset with a slow release burns
its entire sub-step budget on stability it does not need: 86 400 / 11.875 = 7276
sub-steps for a step whose reaction accuracy would have been satisfied by a handful.

D1 is the sentence the note wrote down. D2 and D3 are what make fixing D1 alone useless:
detecting ignition inside a day-long step only to hand the remainder to a loop that
cannot integrate it would move the failure, not remove it.

## Who reaches this today: nobody, which is again the licence

Measured before designing anything:

* **Every runaway test in the tree runs at `dt` ≤ 60 s.**
  `crates/sim-core/tests/runaway.rs` uses 0.25, 0.5, 2.0 and 60 s;
  `crates/sim-core/tests/scenario_runaway.rs` uses `DT` = 1.0 s. All are three orders
  below the 6080 s gate and two below the 24 320 s adaptive reach.
* **No shipped scenario combines a coarse `dt` with `[safety]`.** The client reads `dt`
  from a box (default 0.5 s) and fast-forwards by taking *more* steps, never longer ones
  — `web/app.js` says so in a comment.
* **No guided-path claim is in play.** The claims machinery reads `q_gen_w`, `t_max`
  and their siblings on trajectories driven at the client's `dt`; nothing in `web/` or
  `crates/sim-data/tests/path_claims.rs` reaches a step above the gate.

So the blast radius is the same shape as the integrator slice's: a real defect that
nothing currently reaches. That is the licence to gate the new behaviour where the old
one runs out, and it is also the warning — the perturbation table has to show the new
path reddening something.

## The design

One idea, applied three times: **the sub-step bound that exists for the linear network
is not needed above the `dt` where backward Euler already replaced it.**

### The gate

```rust
fn resolves_ignition_within_step(params, k, dt) -> bool {
    matches!(linear_plan(params, k, dt), LinearPlan::Implicit { .. })
}
```

Exactly the threshold the previous slice established, evaluated from the decision it
already computes. Below it *nothing* changes — same branch structure, same partition,
same arithmetic, same bits. Above it, all three fixes switch on together. One sentence
says why they are one gate: a step long enough to need a different integrator is a step
long enough for a cell to cross onset and burn inside it.

The gate is deliberately **not** the adaptive path's own 24 320 s reach, even though D2
only bites past that. Two thresholds inside one function is how a reader ends up
believing the wrong one, and the band between them (1.69 h to 6.76 h) contains no test,
no scenario and no client. What changes there is that a *reacting* pack's sub-step
partition becomes implicit-with-a-looser-bound rather than explicit-at-11.875 s — an
improvement in both accuracy and cost, on a trajectory nothing walks.

### 1. The linear path watches for onset (D1)

`implicit_substeps` and the explicit sub-step loop gain an optional watch: after each
sub-step, if any cell now satisfies the same predicate the gate uses
(`reaction_power(...) > 0`), stop and report the time integrated. The caller then
continues into the reacting loop with the remainder. Ignition therefore lands on a
**sub-step** boundary — 1/512 of the step at the gate — instead of a step boundary.

The predicate is the existing one, not a new threshold: `reaction_power` returns `0.0`
below onset before evaluating any exponential, so the watch costs two comparisons per
cell per sub-step and only where it is armed.

`Pack::step` must supply the per-cell runaway state for the watch to read, and today it
gathers it only when a cell is *already* at onset. It will gather it whenever the
chemistry has `[safety]` **and** the gate is open — i.e. on coarse steps only, which are
already paying for 512 banded solves, so the ordinary path gathers nothing and allocates
nothing.

The cells still hold their start-of-step temperatures when the thermal call returns
(`temps` is a copy, written back afterwards), so nothing needs saving to make this work.

### 2. Inert stretches inside the reacting loop take the linear plan (D2)

When the reacting loop finds that *no* cell is releasing heat right now — before
ignition, between two cells catching, after a burn completes — the remaining physics is
linear, so it hands the remainder to `linear_plan` under the watch instead of stepping
at 11.875 s. A day-long step through one cell's burn then costs a few hundred reacting
sub-steps plus two implicit stretches, rather than the 7276 sub-steps it does not have.
Measured, for the ignition case: a 506 s stretch, 458 reacting sub-steps, and one
85 713 s stretch.

Stretches do not count against `MAX_RUNAWAY_SUBSTEPS`. That budget exists to bound the
work a *burn* can demand, and a burn's cost is set by the chemistry
(`runaway_energy_j / C_th` kelvin at `MAX_SUBSTEP_RISE_K` = 1 K per sub-step), not by
`dt`. Counting stretches against it would reintroduce the `dt` dependence the budget was
written to be free of.

### 3. The reacting sub-step drops the linear stability bound (D3)

Above the gate the reacting sub-step integrates the linear part with backward Euler and
the reaction term explicitly — the standard IMEX split — so its length is bounded by the
reaction alone:

```
h = min(remaining, SUBSTEP_SAFETY·C/slope_max, MAX_SUBSTEP_RISE_K/net_rate_max)
```

**Two changes, not one**, and they were not both in the first draft of this design — the
second was added after measuring that the first alone does not buy the reach it promises.

`a_lin` drops out of the stability bound, because the linear part is no longer being
integrated by the scheme that bound protects. That is the obvious half.

The accuracy bound changes shape as well: `MAX_SUBSTEP_RISE_K · C / q_node_max` bounds the
rise a cell *would* make if it were adiabatic, and a pack at its quasi-steady temperature
generates watts while moving nowhere. Dividing by the **net** rate — generation plus
conduction plus convection, which `net_rate_max` computes in one O(cells) sweep — bounds
the rise the cell actually makes. Without it the hovering pack in the measurements below
would still have needed 4277 sub-steps for a day; with it, three.

`MAX_SUBSTEP_RISE_K` itself is untouched and remains the operative accuracy bound during a
burn, which is the regime it was derived for — there the cell really is heating at nearly
its adiabatic rate and the two forms of the bound nearly coincide.

The reaction stays explicit because it is non-linear in `T`: an implicit step against it
needs a Newton solve with the Arrhenius Jacobian, which is what
`docs/plans/thermal-implicit-integrator.md` put in "deliberately not done" and which is
still not done here. What *is* done is refusing to pay the linear scheme's stability bound
for a linear part that is no longer being integrated explicitly.

### The cap, and what happens when it still binds

With stretches excluded, `MAX_RUNAWAY_SUBSTEPS` = 2048 now bounds burning alone. At 1 K
per sub-step that is 2048 K of total temperature rise across the pack in one step —
about **eight** full LFP cell burns (253 K adiabatic rise each) or **two and a half**
NMC ones (818 K). A coarse step containing a cascade wider than that still exhausts the
budget.

Where it does, the remainder is finished with **one backward-Euler jump holding the
reaction at its current, budget-clipped rate**, instead of today's one explicit Euler
jump. That is still wrong — the reaction rate is frozen across the tail — but it is
wrong in a bounded way: backward Euler cannot amplify, and the release is clipped to
what each cell can still afford, so the pack cannot exceed its adiabatic ceiling or
produce a non-finite temperature. The `debug_assert` stays.

**Decided deliberately, and recorded here rather than left implied:** no `EventFlags`
bit is added for "the sub-step budget bound". The condition is a work-budget artifact of
a configuration this note bounds and documents, not a physical event, and a new bit
costs public surface in `flags.rs`, `wire_json`, `sim-godot` and `web/app.js` to report
something a client cannot act on. It is listed under "Still open" instead, with the
arithmetic above for anyone who wants to size a cap against a bigger pack.

### What is not built

No snapshot bump: no new `Pack` state, no config surface, no change to `CellRunaway`.
No public API change at all — `resolves_ignition_within_step` is `pub(crate)`.

## Predictions, registered before anything ran

* **P1 — no existing test moves, and none is expected to.** Everything new is behind a
  gate at `dt` = 6080 s and the coarsest runaway `dt` in the tree is 60 s. Prediction:
  the full workspace suite is green with **zero** goldens re-pinned, zero tolerances
  touched and zero trajectories moved. The expected answer is a round zero, as it was
  for the integrator slice.
* **P2 — pre-slice, a pack that crosses onset during a day-long step ignites a day
  late.** On a 3S1P pack started at ambient under a heating current with
  `t_onset_k` set just above ambient, prediction: step 1 (`dt` = 86 400 s) ends with the
  cells above onset, `q_runaway_w` exactly `0.0`, no `THERMAL_RUNAWAY` flag, and every
  cell's `runaway_energy_remaining_j` still at its full budget. Post-slice the same
  single step raises the flag and releases energy.
* **P3 — pre-slice, a pack already at onset at the start of a day-long step produces a
  temperature that is not a temperature.** 2048 sub-steps reach 24 320 s; the remaining
  62 080 s go in one explicit Euler jump with an amplification factor of ~228.
  Prediction: at least one cell comes back non-finite or hundreds of kelvin outside
  `[0, 1e4]`. Corollary, and it is the same corollary the last slice met: **this is
  invisible to a debug test run**, because the `debug_assert` panics first — so the
  measurement has to be taken in release.
* **P4 — post-slice, a day-long step burns exactly the budget and agrees with a fine
  reference.** One 86 400 s step against 8640 steps of 10 s, same pack, same demand.
  Prediction: both release the whole `runaway_energy_j` per cell to within 1e-6 of it,
  both end vented, and the end-of-step temperatures agree to within 1e-3 K — both arms
  are hundreds of convective time constants (`C/hA` = 271 s) past the burn, so both are
  at the same quasi-steady state and the comparison is not an integration-error race.
* **P5 — the sub-step budget is not reached by a single-cell burn in a day-long step.**
  Prediction: the burn costs on the order of 253 reacting sub-steps, so `q_runaway_w·dt`
  equals the budget exactly rather than being truncated by the cap, and the two stretches
  around it cost 512 implicit sub-steps each. Falsifiable by P4 failing on the energy.
* **P6 — `a_warm_step_allocates_nothing` stays green.** The gather, the watch and the
  IMEX solve are all behind the gate; a 1 s step reaches none of them.
* **P7 — deleting each of the three mechanisms reddens something, and the three sets are
  not the same.** Specifically: removing the watch (fix 1) reddens the ignition test but
  not the already-burning one; removing the stretch (fix 2) reddens the already-burning
  one; removing the IMEX split (fix 3) reddens the hovering-above-onset one. If any of
  the three is green everywhere it shipped dead.

## What was measured

All figures from `crates/sim-core/tests/runaway_coarse_step.rs`, **in release**. The
pre-slice code fires a `debug_assert` on the way into this regime, so a debug run panics
before the behaviour can be observed — the same constraint the integrator slice met, and
worth repeating: the defect this slice fixes was invisible to the ordinary test command.

### The pre-slice behaviour, measured before anything was changed

The fixture is a 3S1P chain with the shipped LFP thermal parameters, `k` = 1 W/K, onset
moved down to 315 K so a 12.5 A load can reach it, and a 400 s warm-up (see below).

| case | pre-slice result |
| --- | --- |
| crosses onset inside one 86 400 s step | **no flag, `q_runaway_w` exactly 0, all three budgets still 24 000 J** — and `t_max` 318.65 K, i.e. 3.65 K *past* onset. A full day late. |
| already at onset, 86 400 s of rest | burns, vents, returns to 298.15 K. **Correct.** |
| hovering above onset under load, 86 400 s | 318.102 452 821 K, within **1.4e-8 K** of a fine-`dt` reference. **Correct.** |
| a 300 kJ budget at onset, 86 400 s of rest | **1 779 787 K / −3 897 368 K / 1 779 787 K** |

### The second row is the one that broke a prediction

I registered P3 as "a pack already at onset at the start of a day-long step produces a
temperature that is not a temperature", with the arithmetic: 2048 sub-steps reach
24 320 s, the remaining 62 080 s go in one explicit Euler jump, amplification ~658. The
arithmetic is right and the conclusion is wrong.

Explicit Euler's instability amplifies the distance from the **fixed point of the frozen
system**, and after 6.8 hours against a 271 s convective time constant the pack is *on*
that fixed point. 658 times nothing is nothing. The pre-slice answer came back correct to
1.4e-8 K, which is exactly that residual amplified.

Divergence needs the cap to bind while a cell is still **burning**, because a burning cell
is nowhere near the fixed point its own frozen release implies. That needs a burn longer
than the budget: 300 kJ over 95 J/K is a 3158 K adiabatic rise, and the 1 K rise bound
makes that 3158 sub-steps for one cell against a 2048 budget for the whole pack. There it
is 1.8 million kelvin, and it is a genuine, reachable, release-silent failure.

This is the second time in two slices that a cap's own "not trustworthy" warning turned
out to overstate what happens where it binds. The first was the linear path's 2.64×
band; this is the same shape, and the lesson is now recorded twice: **a stability bound
being exceeded is not the same event as an answer going wrong, and only the second one is
worth a test.**

### The post-slice behaviour

| case | result | bound asserted | how the bound was derived |
| --- | --- | --- | --- |
| ignition inside the step | flag raised, released **71 999.999 999 999 971 J** of 72 000 | 1e-6 relative | the release is self-limiting, so "it ran to completion" is exact rather than a tolerance |
| the same, against 8640 steps of 10 s | cells agree to **1.37e-8 K** | 1e-6 K | both arms 212 convective time constants past the burn, so this is rounding accumulation, not a race between transients |
| already at onset, day-long rest | released exactly 72 000 J; every cell back to ambient within **3.4e-13 K** | 1e-9 K | a day is 318 time constants, so "back to ambient" is exact and the bound is rounding on a 298 K number |
| hovering above onset, released | **9.74e-4** relative to the fine arm | `exp(β·Δ) − 1` = **2.97e-2**, computed in the test | the frozen rate can be stale by at most the Arrhenius sensitivity `β = Ea/(R·T²)` = 0.1189 /K times the 0.2463 K excursion the pack still had to make. Measured is **3.3 %** of it. |
| hovering above onset, temperature | **1.19e-5 K** | **7.26e-4 K**, computed in the test | what the *entire* reaction is worth in steady state: 4.449e-4 W over the pack's 0.6125 W/K of exposure. Measured is **1.6 %** of it. |
| a 300 kJ budget past the cap | **312.90 / 313.30 / 312.90 K**, finite, released exactly 900 kJ | finite, ≥ ambient, ≤ ambient + the whole pack budget's rise | physical bounds, not tolerances — and the pre-slice answer misses the ceiling by a factor of 182 |

### What it costs in sub-steps

Measured with a temporary probe in `advance_temperatures`, in release, for the single
86 400 s step in each case. The pre-slice loop would have asked for 86 400 / 11.875 =
**7276** sub-steps in every one of them and been cut off at 2048.

| case | inert stretches | reacting sub-steps |
| --- | --- | --- |
| crosses onset inside the step | 506.25 s, then 85 713.39 s | 458 |
| already at onset | 85 877.89 s | 404 |
| hovering above onset | none — it reacts throughout | **3** |
| 300 kJ budget, past the cap | none | 2049 (the budget, plus the tail jump) |

The hovering row is the whole argument for taking the accuracy bound on the net rate: the
pack generates 4.7 W per cell and does not move, so three sub-steps resolve a day of it.

### The 6.6 K that belongs to the *other* open item

The compared runs take a 400 s warm-up at 1 s steps before the coarse step begins, and it
is load-bearing rather than cosmetic — the same discovery, in the same place, that the
integrator slice made. Without it the coarse arm settled at **311.45 K** against the fine
arm's **318.10 K**.

That 6.6 K is not the integrator and not the reaction. Heat is solved once per step and
held constant across it, so a day-long step taken from a fresh pack burns the whole day at
`I²·R0` = 3.125 W and never sees the RC pair's `I²·R_rc` — 3.125 W where the settled value
is 4.6875 W, exactly two thirds, and the steady rise is proportional to it. At a 100 s
warm-up the residue was still 0.045 K, which is the `e^−5` of unsettled overpotential the
coarse arm would have frozen for a day; 400 s (twenty RC time constants) puts it below
1.4e-8 K.

So this slice priced the next one for free: **at fast-forward `dt` the constant-heat error
is a 21 % error in the steady temperature of a fresh pack**, against integration errors
down at 1e-8 K. The note that opened both items said the constant-heat one was now the
dominant cost of a coarse step; that is a measured claim now rather than an argued one.

## Predictions, scored

* **P1 — confirmed, exactly.** `cargo test --workspace --no-fail-fast`: **669 tests
  across 73 binaries, all green**, no golden re-pinned, no tolerance touched, no
  trajectory moved. 663 of those existed before the slice and six are new.
  `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`
  are clean.
* **P2 — confirmed exactly.** The pre-slice single step ends 3.65 K past onset with the
  flag clear, `q_runaway_w` exactly `0.0`, and all three budgets untouched at 24 000 J.
  Post-slice the same single step raises the flag and burns all 72 000 J.
* **P3 — falsified, and it is the finding of the slice.** See above: after 6.8 h the pack
  is on the frozen fixed point, so the degenerate explicit jump amplifies nothing and
  returns a correct answer. It is catastrophic only when the cap binds mid-burn. The
  corollary — that the measurement had to be taken in release — held.
* **P4 — confirmed with room.** Released energy matches the fine reference to 2.0e-16
  relative and the temperatures to 1.37e-8 K against a 1e-6 K bound.
* **P5 — confirmed.** A burn costs 458 and 404 reacting sub-steps against the 2048
  budget, and the stretches around it are one call each.
* **P6 — confirmed.** `a_warm_step_allocates_nothing` is green in the 669. The gather,
  the watch and the IMEX solve are all behind the gate, and a 1 s step reaches none.
* **P7 — confirmed, and scoring it needed a finer instrument than the one planned.** At
  binary granularity all three mechanism perturbations redden the same single file, which
  cannot tell them apart at all; re-run against the six tests individually they redden
  three different pairs, in exactly the directions the prediction named. Its last clause —
  "if any of the three is green everywhere it shipped dead" — is answered: none of the
  three is. But two *cheapening* components (rows D and E) are green everywhere, so what
  holds those in place is a cost argument and a design argument rather than a test.

## Perturbations

Each row edits one engine source, runs tests in **release** with `--test-threads=1`,
records what reddened, and restores the file byte-exact before the next row. The
harnesses (`W:\temp\claude\runaway-coarse\perturb.py` for the sweep across binaries,
`perturb3.py` and `perturb4.py` for the per-test re-runs) launch `cargo` directly with
`BELOW_NORMAL_PRIORITY_CLASS` and read the real exit code and the per-test result lines,
because `start /wait` is exit-code-blind. Each baseline was clean before any row ran, and
every file was verified byte-identical afterwards.

The sweep runs ten binaries: `thermal`, `thermal_implicit`, `runaway`,
`runaway_coarse_step`, `scenario_runaway`, `properties`, `snapshot`, `step_allocations`,
`faults`, `boundary`.

| perturbation | what reddened |
| --- | --- |
| A. the ignition watch is never handed to the linear stretch (fix 1 gone, nothing else) | `runaway_coarse_step` |
| A'. the ignition predicate itself always answers "no" | `runaway`, `runaway_coarse_step`, `scenario_runaway` |
| B. no inert stretch — the reacting loop keeps every second (fix 2 gone) | `runaway_coarse_step` |
| C. no IMEX split — the reacting sub-step stays explicit above the gate | `runaway_coarse_step` |
| D. the stability bound keeps `a_lin` above the gate | **nothing** |
| E. the accuracy bound is taken on generation, not on the net rate | **nothing** |
| F. the gate is closed at every `dt` — i.e. the pre-slice engine | `runaway_coarse_step` |
| G. the gate is open at every `dt` | `runaway_coarse_step` |
| H. `Pack::step` does not gather runaway state for the watch | `runaway_coarse_step` |
| I. inert stretches count against `MAX_RUNAWAY_SUBSTEPS` | **nothing** |

Row A' was the *first* attempt at row A and it is kept because getting it wrong was
instructive: making `ignited` always answer "no" does not disable the watch, it disables
the reaction, because the same predicate is the start-of-step gate. It reddens three
binaries for a reason that has nothing to do with this slice. Row A is the isolated
version, and the site it edits — the single `Some((s, &*runaway))` handed to
`linear_stretch` — is the only place in the file where the watch is ever armed, so it
removes the mechanism whole rather than half of it.

### Binary granularity cannot score P7, so the rows were re-run per test

Rows A, B and C redden the same one binary and nothing else. P7's claim is precisely that
the three mechanisms are *not* interchangeable, and a table whose finest resolution is the
file they all live in cannot say that either way. Recording "confirmed, see the table"
over a table that could not have distinguished them is the failure this repository keeps
rediscovering, so the rows were re-run against the six tests individually.

| perturbation | which of the six tests in `runaway_coarse_step.rs` reddened |
| --- | --- |
| A. the watch is never handed to the stretch | `a_pack_that_crosses_onset_inside_one_day_long_step_ignites_inside_it`, `the_day_long_ignition_agrees_with_a_fine_reference` |
| B. no inert stretch | `a_pack_already_at_onset_burns_out_inside_one_day_long_step`, `the_day_long_ignition_agrees_with_a_fine_reference` |
| C. no IMEX split | `a_pack_hovering_above_onset_integrates_a_day_in_one_step`, `a_burn_past_the_work_cap_still_ends_the_step_with_temperatures` |
| F. the gate closed everywhere (the pre-slice engine) | `a_pack_that_crosses_onset_inside_one_day_long_step_ignites_inside_it`, `the_day_long_ignition_agrees_with_a_fine_reference`, `a_burn_past_the_work_cap_still_ends_the_step_with_temperatures` |
| G. the gate open everywhere | `below_the_gate_ignition_still_waits_for_the_next_step`, and nothing else |

Three mechanisms, three different sets, and they differ in the directions P7 named: the
watch is what the *ignition* test is about, and removing it leaves the already-burning
pack untouched; the stretch is what the already-burning pack needs, and removing it leaves
ignition detection working; the IMEX split is what the hovering pack and the past-the-cap
burn need, and removing it leaves both ignition tests green. The one overlap is
`the_day_long_ignition_agrees_with_a_fine_reference`, which compares a whole coarse step
against 8640 fine ones and is therefore sensitive to any of the three — that is what a
comparison test is *for*, and it is the reason it is not the only test in the file.

### Row F is P1 from the other side

Closing the gate is the pre-slice engine, and it reddens **only the new file**. Every
other trajectory in the ten binaries is bit-identical with the slice in or out, which is
the same claim P1 makes from the other direction and a stronger way to make it.

Per test it reaches three of the six, and *which* three is the measured pre-slice table
restated from the other side: the already-at-onset burn and the hovering pack stay green,
because the pre-slice engine answers both of those correctly — the first burns and vents
and returns to ambient, the second lands within 1.4e-8 K of a fine reference. Only the
ignition cases and the mid-burn cap case need the slice at all.

### Row G is the finding about coverage

Opening the gate at every `dt` — arming ignition detection, inert stretches and the IMEX
split on a 0.5 s step — also reddens **only the new file**, and inside it exactly one
test: the one written to pin the gate from underneath. So the existing runaway suite
would not have noticed if this slice had changed every runaway trajectory in the
repository. `runaway.rs` compares two `dt` against each other, asserts a self-limiting
budget, and asserts qualitative propagation; `scenario_runaway.rs` asserts that cells vent
and that neighbours catch. None of that is sensitive to *how* the sub-steps were
partitioned.

That is worth stating plainly: the only thing standing between this slice and a silent
global trajectory move was the deliberate gate, plus the test written on purpose to fail
if someone removes it.

### Row D says fix 3 is two changes, and only one of them is about answers

The reacting sub-step's two changes above the gate were argued together in the design, and
they do not behave the same way under perturbation. Row C — keeping the sub-step
*explicit* while it still takes the loosened bound — reddens two tests. Row D — keeping
the tightened `a_lin` bound while the sub-step is solved *implicitly* — is green
everywhere.

That is not a contradiction; it is the shape of the pair. Dropping `a_lin` from the
stability bound is safe only *because* the linear part is no longer integrated by the
scheme that bound protects. Take one without the other and the answers go wrong, which is
what row C measures. Take both, or take neither and pay for it, and the answers are right.
So the correctness content of fix 3 is narrowly **"do not take the loose bound while still
explicit"**; the loose bound on its own buys sub-steps, not accuracy.

Fix 3 therefore did not ship dead — row C reddens two tests — but what holds its
cheapening half in place is a cost argument and a design argument, not a test. The same is
true of rows E and I.

### Row E is green everywhere, and it cannot be otherwise

The net-rate accuracy bound (design point 3) reddens nothing. That is not a gap in the
tests, and no test could close it, because the two bounds differ exactly where the
difference does not matter:

* `MAX_SUBSTEP_RISE_K · C / q_node_max` is the time a cell would need to rise 1 K **if it
  were adiabatic**. `MAX_SUBSTEP_RISE_K / net_rate_max` is the time it needs to rise 1 K
  at the rate it is actually rising.
* The two are nearly equal whenever generation dominates — which is a cell that really is
  heating, i.e. a burn, which is the regime `MAX_SUBSTEP_RISE_K` was derived for.
* They differ by orders only when conduction and convection nearly cancel generation —
  which is a pack sitting on its quasi-steady temperature. And a pack sitting on its
  quasi-steady temperature is a pack sitting on the fixed point of the frozen system, so
  the cap's tail jump lands on top of it and returns the right answer anyway.

Measured, on the hovering fixture: with the net-rate bound the day costs **3** reacting
sub-steps; with the generation bound it needs 4265, is cut off at 2048, and finishes with
a 44 907 s frozen-reaction jump — and comes back inside the same derived tolerance.

So the bound is kept on a cost argument and a design argument, not on a correctness one,
and this note says so rather than letting the perturbation table imply otherwise. The cost
argument is the count above (3 against 2048 banded solves, on a path where each solve is
O(cells · parallel²)). The design argument is that a pack above onset under load at
fast-forward `dt` is an ordinary state, and an engine that answers it by routinely
entering a branch its own comment calls degenerate is badly built even when the answer
comes out right.

### Row I says the same about a smaller decision

Counting inert stretches against `MAX_RUNAWAY_SUBSTEPS` reddens nothing either, and the
reason is arithmetic rather than structural: a step takes at most one stretch per ignition
plus one, so the fixtures here spend two of a 2048 budget. The "deliberately not counted"
decision is reasoned — it keeps a chemistry-set budget free of `dt` — but it is not
load-bearing at any pack size a test reaches, and it should not be presented as if it
were.

### What the table says about the file as a whole

No test here is decorative: five of the six are reddened by a mechanism perturbation and
the sixth by a gate one. But three of the ten rows are green everywhere, and all three are
decisions about *cost* rather than about answers. This file pins what the slice computes.
It does not pin what the slice spends, and nothing in the repository does.

## Deliberately not done

* **No `EventFlags` bit for "the sub-step budget bound".** The condition is a work-budget
  artifact of a configuration this note bounds and documents, not a physical event, and a
  new bit costs surface in `crates/sim-core/src/flags.rs`, the wire contract
  `crates/sim-data/tests/wire_json.rs` pins, `sim-godot`'s rising-edge signals and
  `web/app.js`'s severity set, to
  report something no client can act on. `SOLVE_UNCONVERGED` already carries two meanings
  and was refused a third by `power-operating-point.md`; this would be a fourth meaning on
  a new bit for a rarer case. The `debug_assert` and the physical bounds asserted in
  `a_burn_past_the_work_cap_still_ends_the_step_with_temperatures` are what stand in its
  place, and "Still open" names it.
* **No Newton solve for the reaction.** The Arrhenius term is non-linear in `T`, so making
  it implicit needs its own Jacobian; `thermal-implicit-integrator.md` put that in
  "deliberately not done" and it stays there. What changed is only that the *linear* half
  of a reacting sub-step is now solved implicitly above the gate, which is the half that
  already had an integrator.
* **`MAX_RUNAWAY_SUBSTEPS` was not raised.** It now bounds burning alone, which is 2048 K
  of pack-wide temperature rise per step — about eight full LFP cell burns. A cap that
  covered an arbitrary pack-wide cascade would have to scale with cell count, and the work
  per step would become O(cells²): 100S10P would be 2 × 10⁶ sub-steps over 1000 cells.
  `CLAUDE.md` asks a raised cap to come with its working, and the working here says not to
  raise it.
* **No second gate at the adaptive path's own 6.76 h reach**, even though that is where
  D2 alone bites. One threshold in one function is what a reader can hold; the band
  between the two (1.69 h to 6.76 h) contains no test, no scenario and no client, and what
  changes there is an improvement in both accuracy and cost.
* **No config surface, no snapshot bump, no `Pack` state, no public API change.** The
  choice is `dt`-driven and internal, exactly as the integrator choice is.
  `resolves_ignition_within_step` is `pub(crate)`.
* **No factorisation reuse across reacting sub-steps.** Each one picks its own `h`, so the
  banded matrix changes and is assembled and factored afresh. That is O(cells · parallel²)
  per reacting sub-step on a pack that is on fire, against O(cells) for the explicit sweep
  it replaces — deliberately paid, because the alternative is the sub-step count the
  measurement above shows it saves.
* **The 100S10P bench was not re-run, and no timing is claimed.** Counted, not timed, on
  the precedent of `pack-step-allocations.md`. What a warm sub-gate step now pays that it
  did not before is **two evaluations of `linear_plan`** — one in `Pack::step` to decide
  whether to gather runaway state, one in `advance_temperatures` — each a handful of float
  ops independent of pack size, and only on a pack that has both `[safety]` and a live
  network. That is a real addition to the hot path and it is stated rather than waved at;
  it is not measured, and the note does not claim it is free.

## Still open

* **Heat generation is still held constant across the step, and this slice measured what
  that costs.** 6.6 K of a 20 K temperature rise on a fresh pack at a day-long `dt`,
  against integration errors down at 1e-8 K. It is now the only remaining cost of a coarse
  `dt` and it is squarely the largest. Fixing it is more than one slice: the RC pair's
  contribution has an exact closed-form mean over the step (`mean V_rc = R·I + τ·(V₀ −
  V_end)/dt`, which is capacitor charge conservation and holds however `V_end` was
  produced), but the rest of the overpotential does not, and — the part that makes it a
  design question rather than an edit — **a step-mean heat needs a step-mean electrical
  quantity beside it or the energy ledger opens.** Today `q_gen_w` and the terminal
  voltage a client integrates are both left-rectangle values from the same instant, which
  is what makes `electrical_and_heat_energy_balance` an identity. Move one and not the
  other and the identity breaks for a real reason. Listed in `docs/ROADMAP.md` H10.
* **A cascade wider than the sub-step budget still freezes its reaction rate for the
  tail.** Bounded — finite, at or above ambient, below what the pack budget could raise it
  to, and never releasing more than each cell can afford — but smeared, and reported by
  nothing but a `debug_assert`. The arithmetic for sizing a cap against a bigger pack is
  on `MAX_RUNAWAY_SUBSTEPS`.
* **Below the gate the one-step ignition lag is unchanged**, and deliberately so: that is
  what buys zero trajectory movement. It is bounded by the gate — at most 6080 s for the
  shipped parameters — and `below_the_gate_ignition_still_waits_for_the_next_step` pins
  it, so the day someone wants it gone, the test that has to change says so.
* **Nothing reaches the new path yet**, exactly as with the integrator slice. No shipped
  scenario and no client takes a step above 1.7 h. This closes a hole rather than enabling
  a shipped feature; the first client that wants a day per step is what would use it, and
  the constant-heat item above is what it would meet next.
* **The accuracy bound on a reacting sub-step uses the net rate at the sub-step's start**
  as a proxy for the move the sub-step makes. That is conservative for a backward-Euler
  relaxation with the reaction frozen — the move is damped relative to `h` times the
  initial rate — but it is an argument, not a proof, and it is not conservative in general
  for a configuration whose fastest mode is being excited rather than relaxed. The
  measured margin on the one case that exercises it is 3.3 % of a derived bound.
* **The gate is still built from `max(4k, hA)`**, not from the matrix's true largest
  eigenvalue, so it is conservative by an unmeasured factor — inherited unchanged from
  `thermal-implicit-integrator.md`, which measured it at ~2.64 on one pack.
* **Nothing pins the sub-step *counts*.** Rows D, E and I are green everywhere because
  they change cost and not answers, so the three cheapening decisions in this slice are
  held by argument alone. A regression that quietly restored the old bounds would be
  invisible to the suite. Pinning them needs a counter the engine does not expose, and
  this note declined to add public surface for it.
