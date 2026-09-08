# The heat a step really generated — the step-mean overpotential

**Status: built, 2026-09-08.** Predictions were registered before anything was measured
and are scored below; one of the six was wrong, and it is what gives this slice its cost.

## What this is about

`docs/ROADMAP.md` carries this in **H10**, and it is the last of the three things a coarse
`dt` used to cost:

> Heat generation is held constant across a step, so a day-long step burns the day at the
> heat of its first instant — **measured at 6.6 K of a 20 K rise**

The measurement is `docs/plans/runaway-inside-a-coarse-step.md`, which found it while
building something else. A pack pulling 12.5 A through `R0` = 0.02 ohm and one RC pair of
0.01 ohm generates 3.125 W in its first instant and 4.6875 W once the pair has settled —
exactly two thirds. Heat is solved once per step from start-of-step state and held there,
so a day-long step from a fresh pack burns the whole day at two thirds of the right power
and settles 6.6 K below where a fine `dt` puts it: 311.45 K against 318.10 K.

Both earlier slices had to *work around* it. `runaway-inside-a-coarse-step.md` prefaces
every compared run with a 400 s warm-up and says the warm-up "is load-bearing rather than
cosmetic" — it exists purely so that the arm under test does not inherit this error.

## The two halves, and why only one of them is here

The ROADMAP row said this was "more than one slice", and gave two reasons. One of them was
wrong and one of them is why this note stops where it does.

**Wrong: "the electrical solve has to run more than once per step".** It does not. The
model already assumes the current is piecewise constant over the step — that assumption is
what makes [`rc_update`] an *exact* exponential rather than an integration — and under it
the only thing in an equivalent circuit's heat that moves within the step is the RC
overpotential, whose mean over `[0, dt]` is

```text
mean V_rc = R·I + tau·(V_0 − V_end)/dt
```

That is capacitor charge conservation: `tau·(V_0 − V_end)` is `R` times the charge the
capacitor gave back. It holds however `V_end` was produced, needs no second `exp`, and
`advance_cell` has just computed `V_end`. So the correction is a subtraction and a divide,
on the value the state update already produced.

**Right: "a step-mean heat needs a step-mean electrical quantity beside it or the energy
ledger opens".** Today `Telemetry::q_gen_w` and the terminal voltage a client integrates
are both left-rectangle values read from the same instant, which is exactly what makes
`properties.rs::electrical_and_heat_energy_balance` an identity that closes to rounding
rather than to a tolerance. Move the heat to the mean and leave the voltage alone and that
identity breaks for a real reason.

So this slice takes the physics and leaves the reporting:

* **the thermal network integrates the step mean.** That is the 6.6 K.
* **`q_gen_w` keeps its meaning and its bits.** The ledger is untouched, every isothermal
  pack in the repository is bit-identical, and `the_reported_heat_is_still_the_first_instants`
  in the new test file pins the gap so it cannot close by accident.

The second half — a mean terminal voltage on `Telemetry` beside a mean `q_gen_w` — is one
slice, and it is now the whole of what H10's row describes.

## The gate this slice does not have, and why

Both predecessors bought "zero trajectory movement" by gating their new path above
`resolves_ignition_within_step` — a `dt` of 6080 s for the shipped parameters. That gate is
built from `C_th / max(4k, hA)` and the sub-step budget. **This defect is governed by
`dt/tau_RC`, and `tau` is 20 s.** The two numbers are three orders apart and have nothing
to do with each other: gate at 6080 s and a `dt` of 3600 s — the value
`crates/sim-core/tests/scenario_aging.rs` fast-forwards at — keeps the error at full size.
A granularity fence there would re-license the precision defect underneath it, and would
ship a third consecutive slice that nothing in the repository reaches.

So the correction is **ungated**, and the price is that trajectories move. They moved less
than expected and in exactly one place; the blast radius is measured below.

## What it costs where a client actually is

The error is `O(dt/tau)` and concentrated in transients, so at the browser client's
`dt` = 0.5 s it is small — but it is not ULPs, and on a hard short it is not small at all,
because the current is enormous and the whole run *is* one transient. Measured across the
suite:

| where | what moved |
| --- | --- |
| every isothermal pack, including every aging fast-forward | nothing, bit-for-bit — the correction is only computed where a live network will consume it |
| every zero-length probe step | nothing, structurally: the excess is exactly `0.0` at `dt <= 0` |
| every `Spm` and `Dfn` cell | nothing, structurally: no closed form, so no correction |
| the golden CSVs (PyBaMM, DFN, analytic) | nothing |
| 77 of the 78 test binaries | nothing |
| the guided path | **ten claims across two lessons**, both of them short-circuit steps |

## The headline, and the residual that is not an error

`runaway-inside-a-coarse-step.md`'s own fixture, reaction disabled, **no warm-up**: one
86 400 s step against 86 400 steps of 1 s.

| | pre-slice | post-slice |
| --- | --- | --- |
| coarse arm, end cell | 311.45 K | **318.100283 K** |
| fine arm, end cell | 318.10 K | 318.101822 K |
| gap | **6.6 K** | **1.539492e-3 K** |

The 1.5 mK is not integration error, and the arithmetic says so before the run does. The
two arms generate the same total heat and differ in *when*: the transient withholds
`I²·R_rc·tau` = 31.25 J relative to a run that was settled from its first instant, the fine
arm pays that in its first minute and has relaxed it away by the end, and the coarse arm
smears the same shortfall across the whole day and is still carrying it. Spread over a day
that is 3.6169e-4 W, and the fine arm's own settled rise gives the cell's exposure as
4.6875 / 19.9518 = 0.234941 W/K, so the predicted gap is

```text
3.616898e-4 / 0.234941 = 1.5394924e-3 K      measured: 1.539492e-3 K
```

Every printed digit. **A coarse step no longer generates the wrong amount of heat; it
generates the right amount at the wrong instant inside the step**, and that is a bound
nothing can improve on without sub-stepping the electrical solve.

At a 600 s step — thirty RC time constants — the two arms agree to **1.05e-11 K**, which is
rounding.

## What is in the code

Three edits.

* **`ecm::rc_step_mean_excess_v`** (new, `pub`): the closed form above, returning
  `mean − V_0` rather than the mean. That difference is what the heat tally adds, and it is
  **exactly `0.0`** in both degenerate cases — `dt <= 0`, where the expression is a division
  by zero, and `tau <= 0`, where [`rc_update`] left `V_end == V_0` and the expression would
  return `R·I − V_0`, a real number and the wrong one.
* **`Advanced::rc_mean_excess_v`**: accumulated across pairs inside `advance_cell`, from the
  `V_end` the update just produced. Exactly `0.0` on the `Spm` and `Dfn` arms, and the field
  doc says that is a stub rather than physics — a porous cell's overpotential has a step
  mean too, it just has no closed form.
* **`Pack::step`**: `heat_w.push(if excess == 0.0 { q } else { q + i_k * excess })`. The
  guard is not an optimisation — `q + i·0.0` is `q` for every value but `-0.0`, which would
  become `+0.0` and move a trajectory for no physics, the same trap the rejection tally two
  lines up is written around. It is what makes "nothing without a correction moves"
  structural instead of an argument.

`q_gen_w` is untouched, so is the ledger, and so is the snapshot: no state was added, and
`version` does not move. `sim_server::API_VERSION` and `sim_wasm::WASM_API_VERSION` do not
move either — no wire field changed shape.

## Registered predictions

* **P1 — the fixture that measured it closes to under 0.01 K.** On
  `runaway_coarse_step.rs`'s own pack with the warm-up removed, reaction off. That fixture
  has a flat `R0` in both SOC and temperature and no entropy table, so after the RC mean
  there is *nothing else* in its heat that varies within a step. If a residual above a few
  tenths of a kelvin survives, `R0(soc, T)` drift or the entropic term is a second half of
  the item and this note has to say so instead of claiming it closed.
* **P2 — `q_gen_w` does not move by a bit, anywhere.** So
  `electrical_and_heat_energy_balance` stays exact, every telemetry assertion on heat stays
  green, and every isothermal pack is bit-identical.
* **P3 — a zero-length probe step is bit-identical**, structurally.
* **P4 — the porous-electrode models are bit-identical**, for the same structural reason.
* **P5 — something reddens, and it is temperature.** Between 1 and 15 test binaries, all of
  them packs with a live network, an equivalent circuit and a current. No golden CSV moves.
  This is the prediction that distinguishes this slice from the two before it.
* **P6 — the guided path moves and its claims are checked.** At the client's `dt` = 0.5 s
  the correction is `(dt/2)·I²·R_rc` per transient, order a few millikelvin, so the
  prediction is that `crates/sim-data/tests/path_claims.rs` **stays green** — the
  temperature claims there are quoted to a tenth of a kelvin at best.

## The suite

`cargo test --workspace --no-fail-fast`: **675 tests across 79 binaries, all green**,
against a pre-slice baseline of 669 across 78 taken on a clean tree. The six new ones are
`crates/sim-core/tests/step_mean_heat.rs`; the binary is that file.
`cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`
are clean. No golden CSV was re-pinned and no tolerance was loosened — the ten claims that
moved had their *recorded measurements* corrected, which is the opposite operation.

## Predictions, scored

* **P1 — confirmed, with three orders to spare.** 1.539492e-3 K against a 0.01 K
  prediction, and attributable to four digits before it was measured. The "second half"
  clause did not fire: nothing else in that fixture's heat varies within a step, exactly as
  argued.
* **P2 — confirmed.** `q_gen_w` is arithmetic that never sees the new term; the balance
  property is green, and `the_reported_heat_is_still_the_first_instants` asserts the
  reported number is `I²·R0` to 1e-12 on a step where the pack absorbed half again as much.
* **P3 — confirmed**, and pinned twice: on the closed form's bits and on a pack's
  temperature across a zero-length step.
* **P4 — confirmed** structurally, and it is worth being honest about what that means: no
  test in this slice runs an `Spm` or `Dfn` pack, so the porous arms are held by the
  `no_rejection` closure and by reading it. Named in "Still open".
* **P5 — confirmed, and undersold.** Exactly **one** binary reddened, not one to fifteen.
  No golden CSV moved, no analytic golden moved, no property test moved, no scenario test
  moved, no server or wasm test moved.
* **P6 — FALSIFIED, and it is the cost of the slice.** `path_claims.rs` went red on
  `nothing-to-clamp`'s hottest-cell temperature: prose 344.520151 K, engine 344.564973 K,
  a 44.8 mK move against a 5 mK tolerance. Running the harness print-and-continue rather
  than fail-fast — the value assertion is a hard `assert!`, so the first mismatch hides the
  rest — showed **ten** claims out of tolerance, across two lessons. The estimate was wrong
  about magnitude for a reason the estimate itself contained and I did not follow: the
  correction scales with `I²`, and these are the two lessons about a **dead short**, where
  the current is 90 A and 184 A rather than the few amps the "few millikelvin" figure was
  computed for. On the step-length lesson the biggest single move is 1.9 K.

## What moved in the guided path

Ten claims failed their tolerance; a further thirteen on the same two steps drifted inside
theirs. All twenty-three had their recorded value re-measured, because `value` is a
recorded measurement and a stale one is the defect that file exists to catch; **eight** of
them also changed the digits the prose prints, so eight sentences in `web/app.js` changed
with them. The remaining fourteen claims on those two steps did not move at all.

| step | claim | prose was | prose is |
| --- | --- | --- | --- |
| `nothing-to-clamp` | hottest cell when protection fires | 344.52 K | **344.56 K** |
| `nothing-to-clamp` | pack peak | 344.5 K | **344.6 K** |
| `nothing-to-clamp` | current at 90.5 s, BMS off | 87.02 A | **87.03 A** |
| `nothing-to-clamp` | current at 240 s, BMS off | 40.33 A | **40.31 A** |
| `one-step-that-got-through` | temperature rise, whole cost of the short | 0.96 K | **0.97 K** |
| `one-step-that-got-through` | group voltage after the spike | 1.3336 V | **1.3337 V** |
| `one-step-that-got-through` | rise at `dt` = 10 s | 19 K | **21 K** |
| `one-step-that-got-through` | second tooth's current, unlatched | 184.53 A | **184.54 A** |

Two of those deserve a sentence each.

**The 19 K that became 21 K is the step-length lesson's headline**, and it does *not* mean
the fix made a coarse step less accurate. That number is the rise at `dt` = 10 s against
1 K at `dt` = 0.5 s, and almost all of it is protection sampling: the BMS decides once per
step, so a ten-times-longer step lets a dead short run ten times longer before the
contactor opens (the accepted item `phase-2-thermal-bms.md` records). The heat that runs
for those extra seconds was being under-counted, and counting it properly makes the coarse
arm hotter. The lesson's point — that the lag is yours to set and the damage is
proportional to it — is unchanged and slightly sharper.

**A note recording a confirmed prediction stopped being true.** The `t_gap_k_at` claim's
note said the sentence's "1.3 K" could only be right if the sensor probe sat in roughly
[343.17, 343.27] K, and recorded that the harness had since confirmed it at 343.2458 K, an
overshoot of 0.0958 K. The probe is now at 343.2883 K — an overshoot of 0.1383 K, outside
the interval. The sentence is unharmed (1.31631 still prints 1.3) and the prediction was
true of the engine it was scored against; the interval was *derived* from where the
sentence's number rounds, so it moves when the engine does. The note now says so. This is
the second time a note's neighbour has rotted while its claim stayed green, and both times
only a whole-step re-measurement found it.

## What the sweep for stale prose found

The claims harness checks the *page*. Nothing checks a doc comment or an authoring note, so
those were swept by hand for anything that named the frozen heat or one of the eight moved
numbers. Two of the finds were not this slice's fault.

* **`Pack::step`'s own doc comment was stale on two counts, and one of them predates this
  slice.** It said a coarse `dt` costs two things, "neither of them the integrator": the
  frozen heat, and a runaway ignition that "is a day late — live `[safety]` and a day-long
  `dt` do not belong in the same run". The second half was **reversed by
  `runaway-inside-a-coarse-step.md` a day earlier** and nobody moved this paragraph with it.
  It now says both what the network integrates and that ignition happens inside the step
  above the gate.
* **`thermal-implicit-integrator.md`'s "Still open" bullet priced this item wrongly.** It
  said fixing it "means solving the electrical problem more than once per step, which is a
  different and much larger change". That sentence is where the ROADMAP row's first reason
  came from, and it is what made this look like more work than it is.
* **Five passages inside `path-claims.toml`'s authoring notes quoted a moved number** —
  three of them quoting the prose sentence verbatim, including one that reproduced
  `"19 K hotter"` while arguing about which arm it belongs to. A note that quotes a
  sentence is a copy of that sentence and rots with it, and no check in the file reads one.
  One of the five could not be honestly re-measured: it lists all four group voltages at an
  instant and `Sensed` retains only the minimum, so the note now says which of the four the
  file re-measures and how to read the other three.

## Perturbation table

Each row is one wrong edit, with `cargo test --workspace --no-fail-fast` run through
`subprocess` at below-normal priority so the exit code survives, and every failing test
listed by name rather than counted by binary.

| # | the wrong edit | what goes red |
| --- | --- | --- |
| A | no correction at all — the pre-slice engine | **4 tests, 2 binaries**: `a_coarse_step_burns_the_windows_heat_not_the_first_instants`, `every_rc_pair_is_corrected_not_only_the_first`, `the_reported_heat_is_still_the_first_instants`, and `every_claim_matches_the_engine` |
| B | only the first RC pair corrected | **1**: `every_rc_pair_is_corrected_not_only_the_first`. Exactly the test written for it, and nothing else in the suite has two pairs and a coarse step. |
| C | the `dt <= 0` guard dropped | **10 tests, 8 binaries**, almost all of them the zero-length probe family — `a_zero_length_probe_moves_nothing`, `zero_length_step_does_not_mutate_state`, `priming_with_a_zero_length_step_is_unobservable`, `the_sensor_frame_is_not_resampled_by_a_zero_length_step`, and the energy balance with them. The probe step is load-bearing across the repository and it shows. |
| D | the `tau <= 0` guard dropped | **1**: `the_zero_cases_are_exactly_zero`. No shipped chemistry has a degenerate pair, so this guard is held by that unit test alone — stated rather than left to be discovered. |
| E | the `excess == 0.0` bit-identity guard removed | **nothing. This row is green, and it is a finding.** The guard defends the sign of a zero: `q + i·0.0` is `q` for every value but `-0.0`, which becomes `+0.0`. Nothing downstream can see that — an integrator adds either to the same answer — so no test can either. It is kept because it is what makes "a porous cell and a zero-length step are bit-identical" structural rather than an argument about IEEE addition, and because the rejection tally two lines above it is written the same way for the same reason. |
| F | the mean taken from the end state instead of the start (arguments swapped) | **6 tests, 4 binaries**, including `thermal_integrator_conserves_energy` and `a_charging_cell_cools_before_it_warms` — the sign of the correction is wrong on a charge, and a test about the entropic term catches it. |
| G | `q_gen_w` moved to the mean too, with no mean voltage beside it — *the next slice, done wrong* | **6 tests, 4 binaries**: `electrical_and_heat_energy_balance`, `overcharge_heat_closes_the_energy_ledger`, `soft_short_closes_the_energy_balance`, `the_rejected_charge_burns_at_the_windows_endpoint`, `the_bottom_of_the_window_rejects_nothing_and_adds_no_heat`, and `the_reported_heat_is_still_the_first_instants`. This is the row that matters most: the deferral is not a hedge, it is **four separate energy ledgers** that open the moment the reported heat moves without its voltage. |

## Still open

* **The reported pair.** `q_gen_w` is the first instant and the terminal voltage a client
  integrates is the same instant, so the two are consistent with each other and neither is
  what the pack absorbed. The next slice adds a mean terminal voltage beside a mean
  `q_gen_w`; the arithmetic is available (`V̄ = OCV − I·R0 − mean overpotential`, so
  `q̄ = I·(OCV − V̄)` and the four-term balance closes with one derived number serving both
  sides), and the work is the public surface: a `Telemetry` field, the server DTO, the wasm
  binding, `wire_json.rs`, and the property test switching partners. Note that with scatter
  the honest pack-level mean is *current-weighted* — each cell's implied terminal voltage
  drifts differently within the step while the solve holds every current fixed — which is
  the one part of it that is a design question rather than an edit.
* **Only the RC pairs are corrected.** Three other things vary within a step and are still
  frozen at its first instant, and they are frozen for three different reasons:
  * `R0(soc, T)` and the entropic term `−I·T·dOCV/dT` need a step-mean *temperature*, which
    is what the thermal solve is computing from this heat. That is circular, and breaking
    it is the sub-stepped electrical solve the ROADMAP row described — genuinely more work,
    and worth nothing until someone measures what it costs.
  * the `[diffusion]` overpotential is `−k·ln(1 − D/(D_lim·soc))`, non-linear in a `D` that
    moves exponentially. The integral is a dilogarithm; there is no closed form to reach
    for, and the lead-acid cell is the only one that has the section.
  * the hysteresis term reads a half-width at a `soc` that moves within the step.
* **`Spm` and `Dfn` take no correction and no test runs one.** The zero is structural — the
  `no_rejection` closure in `CellModel::advance` — so a regression cannot introduce a wrong
  correction, only a future implementation could forget to remove the zero. A porous cell's
  overpotential has a step mean; it has no closed form, so this is a real gap rather than a
  stub, and it belongs with the surface-vs-bulk work rather than here.
* **The 400 s warm-up in `runaway_coarse_step.rs` is now a fixture choice, not a
  workaround**, and its doc says so. It was kept because every derived tolerance in that
  file is built on the temperatures it produces; removing it would re-derive nine
  tolerances to gain nothing.
* **Nothing pins the correction at a client's `dt`.** The two comparison tests use a 20 000 s
  step because that is where the effect is large enough to predict analytically. At
  `dt` = 0.5 s the correction is a few millikelvin and it is held by the guided-path claims
  alone — which is a real hold (they reddened, which is how this was found) but an indirect
  one.
* **`electrical_and_heat_energy_balance` is structurally blind to this**, and stays so
  deliberately. Its `dt` runs 0.01 to 0.5 s against a 20 s time constant, so its RC pairs
  never settle inside a step; and it uses `flat_chem()`, the one configuration where the RC
  mean is exact. It would have stayed green whether or not the correction was right. That
  is not a defect in it — it is testing the identity, and the identity is what this slice
  deliberately did not move — but it is why the new file exists.
