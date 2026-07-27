# Phase 3 — aging + faults

**Status: complete — A, B, C, D and E all landed.** Both exit criteria pass. This file was
written before the work so the decisions below are made once; the "learned while building"
material is appended as each slice lands, the way `phase-2-thermal-bms.md` grew.

| exit criterion (from `CLAUDE.md`) | to be met by |
| --------------------------------- | ------------ |
| A fast-forward of 500 cycles shows a plausible fade curve | `sim-core/tests/scenario_aging.rs` — LFP only (see "Exit criteria stay off NMC"), asserting curve *shape*, not a fitted number |
| "BMS off → overcharge → runaway → neighbour propagation" passes | `sim-core/tests/scenario_runaway.rs` — a `SxP` pack, BMS off, overcharged until a cell vents and at least one neighbour follows |

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | aging: `[aging]` into `ChemistryParams`, per-cell `soh_capacity`/`soh_resistance` + calendar accumulator on `Cell`, calendar **and** cycle fade, resistance growth, the aging sub-clock, pack-level SOH in `Telemetry` | **landed** (v6) |
| B | fault queue: timestamped injection API, `WeakCell`, `SoftInternalShort`, `ExternalShort`, `SensorStuck`/`SensorOffset` | **landed** (v7) |
| C | plating: `PLATING_RISK` from cold-charge physics, accelerated fade, seeded soft-short probability | **landed** (v8) |
| D | runaway: Arrhenius self-heating with a finite per-cell energy budget, `VENTED`, `THERMAL_RUNAWAY`, propagation, and the sub-step bound that makes it integrable | **landed** (v9) |
| E | wrap-up: the two exit scenarios, aging/fault property tests, perf re-measure | **landed** (no bump) |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean, and bumps
`SNAPSHOT_VERSION` if it changes the serialized layout. **A, B, C, and D all change the
layout** — that is four bumps, v5 → v9, one per slice. Phase 2 records missing exactly
this once (slice C changed the layout and did not bump; D's bump had to cover both), so
check the layout at the end of *every* slice, not just the ones that feel stateful.

## Decisions already made (do not re-derive)

### Resistance growth ships in the same slice as capacity fade

Not a sequencing preference — `CLAUDE.md`'s "Never do" list says *never model capacity
fade without the matching resistance growth*, and `r_growth_per_capacity_loss` couples
to both mechanisms. Splitting fade into one slice and growth into the next would leave
the tree transiently violating a project invariant, and would invite a reviewer to read
the intermediate state as the intended design. One slice: state, both mechanisms,
growth, sub-clock, telemetry.

### SOH lives on `Cell`, not on `EcmState`

`Cell` already carries `capacity_factor` and `r0_factor` as static multipliers. SOH is
their dynamic sibling and belongs beside them. Putting it inside `EcmState` would leak
aging into the `CellModel` enum, which Phase 6 needs to stay swappable for `Spm`/`Dfn` —
`CLAUDE.md` principle 9 says nothing outside that enum may assume ECM internals, and
the converse matters too: the enum should not accumulate things that are not the
cell *model*.

This is also what keeps slice A small. Both wire-ins are call-site multiplications with
**no signature change and no memo-shape change**:

```text
cell_source(state, chem, r0_factor * soh_resistance)
advance_cell(..., eff_capacity_ah = cap_ah * capacity_factor * soh_capacity)
```

and `coulomb_step` already takes a `soh_capacity` argument that `advance_cell` currently
hard-codes to `1.0`.

### Aging runs *before* the end-of-step reporting pass, and therefore needs no memo invalidation

`soh_resistance` is an input to `cell_source`, so an aging tick makes the `SourceCache`
stale — but only if it happens after the pass that fills it. Run the sub-clock update
between the thermal integration and the reporting pass, and the pass memoises
post-aging sources; the invariant holds with zero invalidation and zero cost. Aging
*after* the pass would silently poison the next step, and only a debug build would
notice.

### The calendar-fade increment is computed without a difference of square roots

Calendar fade is `∝ √t`, which is path-dependent: an already-aged cell must keep aging
slowly, and changing temperature or SOC must not reset the clock. The standard fix is to
carry accumulated fade `q_cal` and invert it to an equivalent age
`t_eq = (q_cal / k)²` under the *current* stress `k(T, soc)`, then advance from there.

Compute the increment as

```text
dq = k · dt / (√(t_eq + dt) + √t_eq)
```

and **not** as `k·(√(t_eq + dt) − √t_eq)`. The two are algebraically identical; the
subtraction cancels catastrophically when `t_eq` is large and `dt` small, which is
exactly the fast-forward regime this mechanism exists to serve. `t_eq = 0` is fine (the
denominator is `√dt`), and `k → 0` needs a guard because the inversion divides by it.

### `Telemetry`'s SOH is pack-level; the aggregation is pinned here

SOH is per-cell; the two `Telemetry` fields are one number each. They currently report a
hard-coded `1.0`.

- `soh_capacity` = `Σ(cap·factor·soh) / Σ(cap·factor)` — capacity-weighted, mirroring how
  `soc_true` already aggregates, and it reads as "fraction of nominal pack capacity
  still there."
- `soh_resistance` = `r_pack / r_pack_nominal`. Nearly free: the step already computes
  `r_pack` in the series aggregate.

**`nom_ah` folds in SOH**, so `soc_true` keeps meaning *fraction of present capacity*.
That is the right SOC semantics — a half-full aged pack reads 0.5, not 0.4 — and it is
consistent with per-cell coulomb counting, which already divides by the SOH-scaled
capacity. It is nonetheless a behaviour change to `soc_true` on any aged pack, so name it
in the slice-A commit rather than letting it surface as a mystery in a scenario test.

### The fault queue fires on interval containment, at start-of-step, gated on `dt > 0`

A timestamped queue against an arbitrary client `dt` needs a stated rule or it gets
re-litigated: **a fault fires on the first step whose interval `[t, t+dt)` contains its
timestamp, and is applied at start-of-step, before the electrical solve.** Granularity is
therefore `dt`, the same family of accepted imprecision as the BMS's documented
"blindness scales with `dt`" — a fault scheduled for 10.5 s in a 60 s step lands at the
start of the step covering it.

And it takes the `dt == 0` gate. Firing a fault is a reaction to *information*, not to
elapsed time, so an ungated queue would mutate state on a probe step and break the
zero-length-step contract that `snapshot.rs::zero_length_step_does_not_mutate_state`
pins and that the energy-balance property test relies on. This is the third member of
that family after the BMS sensor path and the aging sub-clock — the generalisation
Phase 2 predicted.

### `WeakCell` firing from inside `step` must land before the reporting pass

`Pack::set_cell_factors` clears the entire `SourceCache` (it is *the* documented
invalidation point outside `step`). A queued `WeakCell` applying at start-of-step is
therefore safe — the cache goes cold and the step recomputes — but the same call after
the reporting pass would discard the memo the pass just filled, costing a cold step
every time, and a variant that mutated factors *without* clearing would be a silent
physics error. Start-of-step application, per the rule above, gets this right by
construction.

### The soft internal short is a Thévenin transform, and it changes the memo's invariant

> **Superseded by slice B — see "Learned while building — slice B" below.** The
> transform is correct but does not have to be applied to the cell's *source*. Adding
> the shunt as a conductance on the group node is equivalent, and neither option below
> was taken.

An internal short is a leakage resistance `R_s` across the cell's own terminals, inside
the cell — physically distinct from the balancing bleed, which sits across the *group*
node and dissipates in an external resistor. Fold it in by transforming the cell's
source:

```text
E' = E · R_s/(R0 + R_s)        R' = R0 · R_s/(R0 + R_s)
```

Three things fall out for free, which is why this is the right model:

- **Self-discharge at rest.** With no terminal current, the node settles at `E'` and the
  cell still passes `(E − V_node)/R0 = E/(R0 + R_s)` internally. The cell drains while
  the pack sits idle, which is the whole point of the fault.
- **The SOC-draining current is `(E − V_node)/R0`**, not the terminal current. Coulomb
  counting must use it, or a shorted cell will not lose charge.
- **The dissipation heats the cell.** Unlike the bleed resistor (Phase 2, slice D:
  explicitly *not* added to the cell's thermal node), this resistor is inside the cell,
  so its `V²/R_s` goes into that cell's thermal node. Getting this backwards is an easy
  and invisible mistake — a shorted cell that does not warm up.

**This breaks `SourceCache`'s documented invariant.** Entry *i* is currently specified as
exactly `cell_source(state, chem, r0_factor)`, and the `debug_assert_eq!` in `step`'s
warm path checks it on every cell of every debug test. Slice B must pick one and say so
in code:

1. make the shunt an input to `cell_source` (signature change, invariant preserved
   verbatim, one more field to thread), or
2. restate the invariant as "`cell_source` composed with the cell's active shunt" and
   teach the `debug_assert` the same composition.

Option 1 is preferred — the assert is the guard that pays for `SourceCache`'s
always-true `PartialEq`, and it is worth keeping mechanically checkable rather than
narratively true.

### Runaway retires the "sub-step count depends on config alone" property

`thermal::substeps` derives its ceiling from the *linear* conductances,
`a_max = max(4k, hA)`, deliberately using a bound rather than a per-cell scan so the
sub-step count is a function of config alone — which keeps the trajectory independent of
*where* the hottest cell sits.

An Arrhenius self-heating term destroys that bound. Near onset the local growth rate
`∂Q_rxn/∂T / C_th` can exceed `a_max/C_th` by orders of magnitude, so the existing
ceiling stops bounding anything and explicit Euler overshoots into nonsense — the
integrator would manufacture runaway, or miss it, depending on `dt`.

**Decision: extend the ceiling with the reaction term's derivative evaluated at the
current maximum cell temperature, and retire the config-alone property whenever runaway
is enabled.** That property existed so the trajectory would not depend on hot-cell
position under *linear* physics where position genuinely does not matter. With an
exponential source term the physics itself is temperature-dependent, so the property has
served its purpose. **Determinism is untouched** — the sub-step count remains a
deterministic function of state, which is all `CLAUDE.md` requires.

Two things to verify while building slice D, both of which can invalidate the above:

- Whether `MAX_SUBSTEPS = 512` still holds once the reaction derivative is in the bound.
  If it does not, the existing `debug_assert` fires spuriously inside our own runaway
  tests, and the cap needs raising *with the arithmetic written down* — not silently.
- Pin the integrator with a scenario run at **two different `dt` values** that reaches
  vent within a documented tolerance. A single-`dt` runaway test proves nothing about
  whether the sub-stepping is adequate.

### The coarse-`dt` thermal integrator is not a slice

Phase 2 left a known limitation: the sub-step cap binds above `dt` ≈ 1.7 h, and said the
aging fast-forward would exceed it. The arithmetic says otherwise, so the work is not
scheduled:

- Shipped LFP: `C_th` = 95 J/K, `hA` = 0.35 W/K, `k` = 1 W/K ⇒ ceiling ≈ 11.9 s.
- A calendar fast-forward at `dt` = 3600 s needs `ceil(3600/11.875)` = 304 sub-steps —
  under the 512 cap, which itself binds at `512 × 11.875 s` ≈ 1.69 h.
- A 500-cycle fast-forward runs at cycling `dt` (seconds to a minute), nowhere near it.

Recorded triggers for revisiting, from `CLAUDE.md`'s "raise an integrator, not the cap":
a scenario that genuinely needs `dt` > ~1.7 h (multi-year calendar storage in one step),
or slice D's reaction term shrinking the ceiling far enough that ordinary steps hit the
cap. The coupled system is linear in `T` over a step *without* the reaction term, so an
exact/implicit integrator is available if either trigger fires.

### Exit criteria stay off NMC numbers

The NMC file has an open identity question (the chemistry claims an 18650/3 Ah cell while
the PyBaMM Chen2020 set it would be refit against is a 21700/5 Ah cell), so its
parameters may move. Both Phase 3 exit scenarios use LFP. More generally: the 500-cycle
criterion asserts the fade curve's *shape* — monotone decreasing, decelerating (the √t
signature), resistance rising as capacity falls, faster fade at higher temperature and
higher SOC — not a fitted end-of-life number. Every aging coefficient in both shipped
chemistries is a labelled placeholder, so a test asserting "7.3 % fade at 500 cycles"
would be pinning a placeholder, and would break the moment anyone does the fit the
provenance notes ask for.

## Open questions, to answer inside the slice that hits them

- ~~**Aging sub-clock period.**~~ Answered in slice A, below.
- ~~**Cycle-fade DOD accounting without rainflow.**~~ Answered in slice A, below.
- ~~**Plating's soft-short probability** draw order.~~ Answered in slice C, below.
- ~~**Does an external short bypass the contactor?**~~ Answered in slice B, below.

---

## Learned while building — slice A (aging)

Snapshot layout v5 → **v6**. New module `sim-core/src/aging.rs`; new integration test
`sim-core/tests/aging.rs` (17 tests). Everything the pre-work section decided held up:
SOH on `Cell`, aging sequenced before the reporting pass, the rationalised calendar
increment, and the pack-level SOH aggregation all shipped as written.

### The two open questions, answered

**Sub-clock period.** `AgingConfig { sub_clock_period_s }` on `PackConfig`, default
10 s, validated finite and `>= 0` (zero = age every step, which the tests use). A
partial period is **carried**, not dropped — `Aging::accum_s` is in the snapshot, and
`snapshot_mid_period_replays_bit_identically` pins it. Dropping it would have made the
act of taking a snapshot change the physics.

The update applies the *whole* accumulated interval in **one** tick, not a loop of
period-sized ones. That is what keeps fast-forward cheap: `dt` = 1 h ticks aging once,
not 360 times. It costs nothing in accuracy for the calendar term, because —

**the calendar integral turns out to be exact over any partition.** If `q = k·√t`
holds, then `q + k·dt/(√(t_eq+dt)+√t_eq)` with `t_eq = (q/k)²` is exactly `k·√(t+dt)`.
So at fixed stress the accumulated fade depends only on total elapsed time, whatever
the step sizes. `accumulated_calendar_increments_reproduce_sqrt_t` checks this at 1 to
100 000 steps to a relative 1e-12, and it is a genuinely strong test of the
rationalised form: the naive difference of square roots fails it badly at fine `dt`.
This was expected to be an approximation and is not one; only the *stress sampling*
(`k` re-evaluated per tick) is coarse.

**Cycle-fade DOD.** The depth is measured from the SOC at the **last reversal of the
pack current**, tracked per cell as `soc_ref` + a `discharging` flag. Two fields, not a
running min/max pair, because a half-cycle is monotone by construction — a reversal is
what ends it.

The *pack* current, not the cell's own, and that took a correction. Anchoring on
`i_cell` was the obvious design and is a trap: at rest `i_cell` is not zero. A group's
node voltage is a ratio of sums, so a uniform pack circulates a rounding-sized current
and a scattered pack circulates a real one, and half the cells in a group flip sign the
moment the load comes off. An arbitrarily *small* reversal would then discard the depth
accounting for the large excursion in progress — resting mid-discharge would score one
deep cycle as two shallow ones, exactly what the design says it must not do. Measured
on a 1S2P pack with 5 % scatter: a ten-hour rest cut one cell's cycle fade by **5.8 %**
before the fix, and changes it by under 1 % after (that residual is real circulation
throughput, which should count).

`i_pack` is exactly `0.0` under `Demand::Rest` and under an open contactor, so this
needs no threshold. Discarding insignificant reversals properly is what rainflow does,
`CLAUDE.md` rules rainflow out for v1, and a deadband would need a magic constant with
no provenance — so the v1 answer is that a half-cycle boundary is a **pack-level**
event. The stated cost: a cell back-fed by its parallel neighbours while the pack
discharges gets no half-cycle boundary of its own; it inherits the pack's, measured
from its own SOC. `resting_mid_discharge_does_not_split_the_cycle` pins all of it.

The weight is `dod^(exp − 1)`, **not** `dod^exp`. The literature parameterises cycle
life per cycle (a depth-`D` cycle costs `∝ D^exp`) and that cycle moves `∝ D` amp-hours,
so the cost per amp-hour carries the exponent minus one. This makes `cyc_fade_per_ah`
mean something concrete — fade per Ah at full depth — and makes `exp = 1` degenerate
cleanly to pure throughput counting. Validation now rejects `exp < 1`, where the
negative exponent would make a micro-cycle age a cell *more* than a full one.

Known approximation, documented on `cycle_increment`: the depth used is that of the
half-cycle **in progress**, so amp-hours early in a deep excursion are charged at the
shallow depth reached so far. Rainflow is what fixes this and `CLAUDE.md` rules it out
for v1; the cheaper honest fix, if it ever matters, is to credit throughput at reversal
rather than as it happens.

### Decisions slice A had to make that the pre-work did not anticipate

**Chemistry `[aging]` is `Option`, and a missing section is a build error.** Aging has
two halves — coefficients (chemistry data) and policy (`PackConfig`) — mirroring
`ThermalParams`/`ThermalConfig`. The alternative, defaulting absent coefficients to
zero, was rejected: a pack configured to age but fading at exactly zero is
indistinguishable from a working model on a very stable cell. `BuildError::
MissingAgingParams` says so instead. This is the diagnostic that `PackConfig::thermal`'s
doc comment wishes it had.

**The `[aging]` stress table's breakpoints are implied uniform.** `cal_soc_stress =
[1.0, 1.0, 1.4]` carries no SOC column, so `n` entries are read as sitting at
`0, 1/(n−1), …, 1` — three entries mean 0.0 / 0.5 / 1.0, which is what the shipped
files' comments already claimed. Stated on `soc_stress` and pinned by a test.

**The NMC placeholders were rescaled.** `cal_pre_exp` and `cal_ea_j_per_mol` only mean
anything as a *pair*, and NMC's shipped pair produced **~260 % calendar fade in a year
at 25 °C** — a pack dead within weeks the moment aging was switched on. Lowered
2.0e4 → 1.2e3 for ~16 % a year, a little worse than LFP, which is the right ordering.
The provenance note records the old value and why it moved. LFP's pair was already
plausible (~14 % a year at 25 °C and full SOC) and is untouched. General lesson for the
remaining slices: a placeholder that has never been evaluated is not known to be
order-of-magnitude anything, and `[safety]` has the same exposure waiting in slice D.

**`Telemetry::soh_resistance` excludes the balancing bleed.** The pre-work pinned
`r_pack / r_pack_nominal` and called it nearly free off the series aggregate. It is not
quite: `r_pack` includes each group's bleed conductance, and a closed bleed switch
lowers group impedance without any cell having got healthier. The aggregation therefore
runs in the reporting pass over cell conductances only, gated on aging being live so a
non-aging pack pays nothing and reports the literal `1.0`.

### A guard for the bug class the NMC rescale exposed

The 260 %/year sat in the tree for two phases because `validate()` checks each aging
number *in isolation* and every one of them passed — the failure is only visible in the
`cal_pre_exp`/`cal_ea_j_per_mol` **pair**. `sim-data`'s
`shipped_aging_coefficients_give_a_plausible_one_year_fade` now evaluates each shipped
chemistry's actual one-year calendar fade at 25 °C / full SOC, and its 500-full-cycle
fade, against a 1–50 % band. A band, not a fitted number, so it stays inside the "shape
not magnitude" rule; wide enough that any honest placeholder passes, narrow enough that
an unevaluated pair does not. Verified by reverting NMC to 2.0e4, which fails it with
"fades 264.3 %".

**Slice D should do the same for `[safety]`.** `t_onset_k`, `t_vent_k`, and
`runaway_energy_j` are placeholders that have likewise never been evaluated together,
and they interact through the Arrhenius self-heating term the same way.

### Perf: a real but unquantified regression, deferred to slice E

Two extra multiplies and two predictable branches per cell per step (`eff_r0_factor()`
is unconditional; the aging accumulation and SOH aggregation are branch-gated). Paired
alternating runs against a `HEAD` worktree, `100S10P/full`:

| pass | baseline | slice A |
| ---- | -------- | ------- |
| 1 | 56.6 µs | 57.0 µs |
| 2 | 52.0 µs | 60.7 µs |
| 3 | 49.1 µs | 52.9 µs |

Directionally consistent — slice A is slower in all three — but the noise band is wider
than the effect, so the honest reading is "single-digit percent, sign known, magnitude
not". **The machine could not verify the 50 µs budget in either arm**: the baseline
itself measured 49–57 µs where `docs/plans/pack-step-perf.md` recorded 39–49 µs, so this
session's box is running ~25 % slow and no absolute conclusion can be drawn from it.

Slice E owns the re-measure. If the overhead needs removing, the obvious move is to
cache `r0_factor · soh_resistance` as a derived field on `Cell` refreshed at the aging
tick and in `set_cell_factors` — the same invariant-with-a-`debug_assert` shape the
Thévenin memo already uses. That is an optimisation with a correctness obligation, so
it wants a measurement justifying it first.

### Behaviour change to know about

`Telemetry::soc_true` now divides by capacity that folds in `soh_capacity`, so SOC means
*fraction of the capacity the pack has today*. A half-full pack faded 20 % reads 0.5,
not 0.4. This matches per-cell coulomb counting, which already divided by the SOH-scaled
capacity, and it is why v6 is a **semantic** bump and not only a layout one: the same
stored state reports a different SOC than v5 would have. Unaged packs are unaffected
bit-for-bit — every pre-existing test passed untouched apart from the mechanical
`aging: None` field.

---

## Learned while building — slice B (faults)

Snapshot layout v6 → **v7**. New module `sim-core/src/faults.rs`; new integration test
`sim-core/tests/faults.rs` (20 tests). `Telemetry` gained `i_internal_short_a` and
`i_external_short_a`; `CellView` gained `internal_short_conductance_s`. The queue's
timing contract shipped exactly as the pre-work specified — interval containment, at
start of step, gated on `dt > 0`, past-dated faults firing late rather than being
dropped.

### The shunt is a conductance on the group node, not a transform of the cell's source

The pre-work framed this as a choice between two ways of putting the transformed
source `(E', R')` into `SourceCache`, and preferred option 1 (thread the shunt into
`cell_source`). **Neither was taken**, and the reason is not taste.

A shunt contributes conductance and **no Norton current**, so the group's node
equation only gains a denominator term:

```text
V = (Σ E_k/R_k − I_g) / (Σ 1/R_k + Σ G_s,k + G_bleed)
```

— structurally identical to the balancing bleed, but per cell instead of per group.
The per-cell internal current `(E_k − V)/R_k` then keeps its existing formula *and*
its existing meaning, so the diff in the hot loop is one addition per cell.

What settles it is the **heat term**. `cell_heat_w` needs the cell's own untransformed
`R0` for `I²·R0`. Had the memo held `(E', R')`, that `R0` would have to be recovered as
`1/R0 = 1/R' − G_s` — a subtraction of nearly-equal reciprocals precisely in the regime
a soft short lives in (`R_s ≫ R0`). The Norton form never poses the question.

Consequences, all good: `cell_source` keeps its signature, `SourceCache`'s invariant
and its `debug_assert` are **literally unchanged**, and injecting a short needs no
cache invalidation at all. The one obligation the pre-work correctly identified — that
the shunt has to appear in *both* conductance sums, 200 lines apart — is real, and the
reporting pass's `sum_g_cells` must still exclude it (a shorted cell is not a healthier
cell, exactly the argument the bleed already needed).

### The external short sits outside the contactor

Answering the open question: **load side**, so opening the contactor interrupts it.
The reasoning is that this is the placement that makes the BMS contrast an experiment
rather than a tautology — protection derates the load, discovers that derating does
nothing to a short, and is left with only one move. `protection_survives_an_external_short_by_latching_open`
pins the whole trace: the sag trips under-voltage past its hard margin within two
steps, the contactor latches, both load and short go to exactly zero, and the pack
keeps most of its charge — while the same pack with no BMS runs to `SOC_CLAMPED_LOW`.
The cell-side short, which no contactor can save you from, is what `SoftInternalShort`
already models, so nothing is lost by the choice.

It enters the solve as a shunt on the pack's *aggregate* Thévenin: the load solves
against `(E', R') = (E, R)/(1 + R·G_ext)` and the short then takes `V·G_ext` at the
terminal voltage that same solve produces. Closed form for every demand variant,
including `Power`. The identity that matters is that `E' − i_load·R'` is exactly the
terminal voltage `E_pack − i_g·R_pack` that the *total* current produces — the two
views of one node agree by construction, and if they ever stopped agreeing the symptom
would be a mystifying energy-balance residual rather than a voltage mismatch. Pinned
directly by `external_short_conducts_at_the_solved_terminal_voltage`.

`Telemetry::i_actual` is therefore now the *total* current out of the cells, load plus
short. That keeps `v_terminal · i_actual` the whole electrical outflow, which is what
keeps the energy balance a three-term identity.

### An internal short drains the whole parallel group, not just its cell

This was written as a test asserting the shorted cell drains fastest, and the test
failed with the two SOCs **bit-identical**. The model is right and the assertion was
wrong: the leakage path hangs off the cell's terminals, and in a parallel group those
terminals *are* the group node, so matched neighbours feed the short equally.

That is correct for an ideal group and it is the honest consequence of the pack model
carrying no busbar or weld resistance between a cell and its node — a real group's
shorted cell drains somewhat faster. The **heat** is never shared: it stays in the cell
containing the leakage path. A whole group draining into one hot spot is the shape of
the real failure, and that part the model does get. Recorded on the module and pinned
by `soft_short_drains_the_whole_parallel_group`, because a reader who expects the
shorted cell to empty first will otherwise read the equal SOCs as a bug.

Interconnect resistance is the fix if it ever matters, and it is a *pack-model* change
(a per-cell series resistance between cell and node), not a fault-model one.

### Sensor faults are applied after the noise draw, deliberately

A stuck sensor reads exactly its stuck value — the fault is applied after the BMS's own
offset and noise, so it is the last word. But the draw still **happens**, inside
`Bms::sample`, before the override. That is the property worth having: injecting a
sensor fault does not shift the RNG stream, so every other draw in the trajectory stays
aligned and a faulted run stays comparable to a clean one.
`sensor_offset_rides_on_top_without_shifting_the_rng` pins it by asserting the faulted
reading is bit-for-bit the clean reading plus the offset.

### Deferred: the queue is API-only

`CLAUDE.md` allows the queue "in config **or** via API" and slice B shipped the API
(`Pack::schedule_fault`), plus `Pack::clear_faults` as the repair seam mirroring
`clear_bms_fault`. A `PackConfig.faults` field belongs with the scenario file format,
which Phase 4 owns; adding it now would have churned every test's config literal for a
field nothing yet reads from a file. **Phase 4 should pick this up** — the validation
(`Pack::validate_fault`) is already factored to be callable at build time.

`clear_faults` cannot undo a fired `WeakCell`: that fault does not persist as a fault,
it is folded into the cell's static factors the moment it fires and becomes
indistinguishable from an unlucky scatter draw. Documented on the method; the way back
is `set_cell_factors`.

### A test-coverage hole slice A left, closed here

`thevenin_cache.rs`'s `cell_bits` enumerates the `CellView` fields it compares
**explicitly**, and slice A added `soh_capacity`/`soh_resistance` without adding them
there — so the bit-exactness test had been silently covering less than it claimed while
still passing. Both are now in it, along with the new short conductance and the two new
`Telemetry` fields. The general lesson for slices C and D: any new `Telemetry` or
`CellView` field must be added to `tele_bits`/`cell_bits` in the same commit, because
nothing fails if it is not.

### Injected faults deliberately raise no `EventFlags`

A client sees a fault only through `i_internal_short_a` / `i_external_short_a` and the
per-cell view, never through a flag bit. That is a decision, not an omission: the flag
set reports *physical events the pack discovered*, and an injected fault is something
the client already knows it did. The consequences do flag — the external-short scenario
raises `UV`, `CONTACTOR_OPEN`, `SOC_CLAMPED_LOW` on the way down, all from the ordinary
protection path. Slice C's `PLATING_RISK` and slice D's `VENTED` /
`THERMAL_RUNAWAY` are the opposite case (emergent, discovered by the engine) and
should flag, so the asymmetry is the rule working, not an inconsistency.

### Perf: what slice B added to the hot loop, for slice E to measure

Unquantified, like slice A's, and for the same reason — the honest re-measure is slice
E's job and this box could not verify the budget in either arm last time. What changed:

- **Two additions per cell per step**, one in each aggregation loop (`sum_g += g +
  cell.shunt_g`). Unconditional, and deliberately so: `g > 0` always, so `g + 0.0 == g`
  bit-for-bit and a healthy pack solves exactly the arithmetic it solved before.
- **One branch per cell** in the heat tally (`if cell.shunt_g > 0.0`), always
  false on a healthy pack, taken instead of an unconditional multiply-add so that a
  healthy cell's heat keeps its bits exactly.
- **One emptiness check per step** on the queue, plus a `partition_point` only when
  something is actually queued.
- Eight more bytes per `Cell` (`shunt_g`), which is the one item with a plausible
  cache-line cost at 100S10P and the only reason to expect anything measurable at all.

Slice E measures A and B together against `docs/plans/pack-step-perf.md`'s recorded
39–49 µs, with the bench traps that doc lists (paired worktrees, alternating arm order,
warmed clone template).

---

## Learned while building — slice C (plating)

Snapshot layout v7 → **v8**. New module `sim-core/src/plating.rs`; new integration test
`sim-core/tests/plating.rs` (19 tests). `ChemistryParams` gained `safety:
Option<SafetyParams>` — the `[safety]` section both shipped files have carried since
Phase 0 and that nothing had ever parsed — and `CellAging` gained `q_plating` and
`ah_plating_since_tick`. **No new `Telemetry` or `CellView` fields**, which is the
cheapest way to stay out of the `tele_bits`/`cell_bits` trap slice B documented.

### The open question, answered

**Draw order and the zero-probability short-circuit both shipped as specified**, and the
interesting part is what it took to *test* them. The contract is: at most one draw per
cell per aging tick, in series-major/parallel-minor order, and no draw at all when the
probability is zero.

The order test is a **pair of packs**. A 1S1P pack whose only cell plates, against a
2S1P pack whose *second* cell plates and whose first does not — the first is given a
tenfold capacity factor via `set_cell_factors`, so at the shared series current it sits
at 0.2C, below the threshold. Zero scatter and no BMS means nothing else in either pack
touches the RNG, so if the contract holds the plating cell in each consumes the
identical stream and the two shunt histories match step for step.

The probability is tuned to about 0.5 per tick so that twenty ticks contain **both**
outcomes. That is the part worth copying: an outcome-only test would pass against an
implementation where a roll that comes up high fails to consume its draw, because it
would never notice the streams desynchronising. The short resistance is set to 1e9 ohms
so the recorded events stay a pure readout of the RNG instead of feeding back into the
physics being compared.

The zero-probability test reads the draws through the **current sensor**: the BMS's
noise is the only other RNG consumer, so `i_pack_a − i_actual − current_offset_a`
recovers the raw draw. A cold pack with a zero hazard must produce the same sequence as
a warm one that never plates — *and* a cold pack with a live hazard must produce a
different one, which is the assertion that stops the test passing vacuously.

Both were verified by mutation: deleting the `if p > 0.0` guard fails exactly those two
tests and nothing else.

**What is *not* tested is the parallel axis.** Every pack in the order test is
`parallel: 1`, so it pins series-major traversal and the skip-a-non-plating-cell rule
and nothing about parallel-minor. That half rests on the loop structure — the same
nested `groups` → `cells` iteration the scatter draws use — rather than on a test,
because making one cell of a parallel group plate while its neighbour does not is
awkward by construction: currents in a group split by state, so the capacity-factor
trick that works in series does not transfer. Anyone touching that loop should know the
guard is structural.

### Decisions slice C had to make that the pre-work did not anticipate

**`[safety]` absent is not a build error**, unlike `[aging]`. The slice-A precedent does
not transfer: aging needed `MissingAgingParams` because `PackConfig::aging` is an
explicit request the chemistry could not honour. Plating is emergent — nothing in the
config asks for it — so there is no request for the silence to contradict. The right
analogy is `ocv.docv_dt_v_per_k`, whose absence quietly disables entropic heating. A
chemistry with no `[safety]` never raises `PLATING_RISK`, and that is the whole of it.

**Three coefficients had to be invented**, because `CLAUDE.md`'s `[safety]` sketch has
the two *thresholds* (`t_plating_min_k`, `plating_c_threshold`) but nothing saying what
crossing them costs. Added, defaulted to zero, and documented as "reported but free"
when omitted — which is exactly what a chemistry file written before this slice means,
so nobody's parameter set breaks:

- `plating_fade_per_ah` — capacity fraction per Ah carried while plating,
- `plating_short_hazard_per_ah` — Poisson hazard per Ah plated,
- `plating_short_ohms` — the resulting short's resistance, validated `> 0` **only when
  the hazard is positive**, since otherwise no short can form and requiring it would
  reject usable files.

**Plating fade is a separate additive term, not "accelerated" cycle fade.** `CLAUDE.md`
says "applies accelerated fade", and the obvious reading — a multiplier on the
cycle-fade weight — was rejected. Cycle fade is weighted by `dod^(exp−1)`, and depth of
discharge is *irrelevant* to plating: the loss is lithium that plated out of the cell,
and it does not care how deep the excursion carrying it was. Folding plating through
that weight would also have given a plating cell at a reversal (`dod = 0`) exactly zero
damage. So `q_plating` is its own accumulator, its own coefficient, and unweighted.

**The hazard is per amp-hour plated, not per second cold.** Two things fall out, both
wanted. Dendrites grow from deposited lithium, so a cell sitting cold *at rest* accrues
no hazard — right. And because the hazard is Poisson in plated charge, rolling once
against 2 Ah is exactly as likely to short as rolling twice against 1 Ah: **the aging
sub-clock period does not change how dangerous cold charging is.** A client cannot make
its pack safer by choosing a coarser clock. Pinned by
`the_short_hazard_does_not_depend_on_how_the_charge_is_split`.

`short_probability` computes `−expm1(−λ·ah)` rather than `1 − exp(−λ·ah)`, the same
cancellation-avoidance the calendar increment needed and for the same reason: `λ·ah` is
routinely ~1e-6 in service.

### The C-rate feedback loop, and why it is not one

The C-rate is measured against the capacity the cell has **today** — nominal × factor ×
`soh_capacity` — so an aged cell reaches the plating threshold at a lower absolute
current. That is real behaviour, and it is also a feedback path from aging back into
aging fed entirely by unfitted placeholders, which is the shape slice A's NMC rescale
warns about. So it was answered rather than argued:

- The C-rate enters as a **threshold**, not as a rate multiplier. Fade is
  `plating_fade_per_ah · ah_plated` however far past the line the cell is, so aging can
  switch plating *on* for a given current but can never make plating already happening
  go faster.
- The amp-hours a full charge moves **shrink** as capacity fades, so damage per cycle
  decreases with age.

`repeated_cold_charging_fades_without_running_away` pins it empirically at 40 aggressive
cold cycles: health falls monotonically, the last five cycles cost less than the first
five, and the trajectory stays far above `MIN_SOH_CAPACITY` instead of collapsing onto
it. **Recommended for slice D:** the runaway term is a genuine exponential and will not
have this defence, which is precisely why the pre-work already requires a two-`dt`
integrator check there.

### Known simplification worth naming

**Plating-driven loss grows resistance at the *same* ratio as calendar and cycle loss.**
`soh_resistance = 1 + r_growth_per_capacity_loss · (q_cal + q_cyc + q_plating)`, one
coefficient for all three. Real plated lithium and the fresh SEI that grows on it raise
impedance disproportionately, so a cell that lost 1 % to plating is harder to push
current through than one that lost 1 % to shelf time. The model therefore
**under-reports the resistance cost of plating**. Splitting the coupling needs a second
coefficient in `[aging]` and a fit to justify it; recorded on `CellAging::tick` rather
than left to be discovered.

### The guard, extended to the shipped plating numbers

`sim-data`'s `shipped_plating_coefficients_give_a_plausible_cold_charge_cost` does for
`[safety]`'s plating half what slice A's test did for `[aging]`: it evaluates one full
cold charge on each shipped chemistry against a band (0.05–5 % capacity, 0.01–5 % short
probability), plus a check that the short is at least 100× the cell's `R0` and therefore
actually *soft*. Bands, not fitted numbers, so it stays inside the shape-not-magnitude
rule.

**Slice D still owns the runaway trio.** `t_onset_k`, `t_vent_k` and `runaway_energy_j`
are now parsed and validated (finite, positive, onset below vent) but nothing consumes
them, and they have still never been evaluated *together* — the same exposure the NMC
aging pair had for two phases.

### Perf: what slice C added to the hot loop, for slice E to measure

- **One branch per cell per step** for the plating check. On a chemistry with `[safety]`
  the predicate is evaluated and its first test is `i_cell < 0.0`, which is false on any
  discharge — so the common case is one predictable, correctly-predicted branch.
- **Sixteen more bytes per `Cell`** (`q_plating`, `ah_plating_since_tick`), on top of
  slice B's eight. At 100S10P this is the item with a plausible cache-line cost, and
  `Cell` has now grown in three consecutive slices — worth looking at as a whole rather
  than one slice at a time.
- **One extra multiply-add and one branch per cell per aging tick**, which is off the
  per-step path whenever the sub-clock period is not zero.

The benchmark's `lfp_like_chem` now carries `[safety]`, matching the shipped file, so the
plating branch is measured rather than optimised away by a `None` nobody ships.

---

## Learned while building — slice D (runaway)

Snapshot layout v8 → **v9**. New module `sim-core/src/runaway.rs`; new integration test
`sim-core/tests/runaway.rs` (17 tests). `SafetyParams` gained
`runaway_power_w_at_onset` and `runaway_ea_j_per_mol`; every `Cell` gained
`runaway: CellRunaway`. `Telemetry` gained `q_runaway_w`, `CellView` gained
`runaway_energy_remaining_j` and `vented` — all three added to `tele_bits`/`cell_bits`
in the same commit, which is the trap slice B documented.

### The reaction term, and the two coefficients it needed

`CLAUDE.md`'s `[safety]` sketch has the runaway *thresholds* and the energy budget but
nothing that sets a **rate**, so slice D had to invent the same way slice C did:

```text
Q_rxn(T, a) = P_onset * a * exp( -(Ea/R) * (1/T - 1/T_onset) )
```

with `a = energy_remaining / runaway_energy_j`. Both new fields default to zero and are
documented as "onset reported but free", so a chemistry file written before this slice
still parses and simply never burns. `runaway_ea_j_per_mol` is validated `> 0` only when
the amplitude is positive — the same conditional shape `plating_short_ohms` uses, and for
the same reason: a zero activation energy is a constant heater, not a runaway.

The exponent is referenced to `T_onset`, not to absolute zero. That is the decision worth
keeping: `exp(-Ea/(R*T))` alone is ~1e-13 at these temperatures, so a pre-exponential
would have to be ~1e13 and nobody could tell a plausible value from an absurd one by
looking. Referenced to onset, `P_onset` is *exactly* the release at onset — a number a
reader can check against a plot, which is precisely what the `[aging]` pair could not
offer and what let its 260 %/year sit in the tree for two phases.

### The stability bound was not the binding constraint — a rise cap was

The pre-work proposed extending `thermal::substeps`' ceiling with the reaction's
derivative. That is necessary and not sufficient, and the difference is not academic:
`0.5*C/a` bounds the *linearised* problem's oscillation and says nothing about `a` itself
growing tenfold inside the sub-step just taken.

What ships is a second, tighter bound — `h <= MAX_SUBSTEP_RISE_K*C_th / Q_total`, at 1 K.
The Arrhenius logarithmic sensitivity `Ea/(R*T^2)` is ~0.067/K at 423 K and ~0.012/K at
1000 K, so 1 K holds the reaction term to under ~7 % drift within any sub-step across the
whole trajectory. Measured effect: removing it (leaving exactly the bound the pre-work
proposed) moves the vent-to-600 K climb by **4.6 %** between `dt` = 0.25 s and `dt` = 2 s,
against 0.9 % as shipped.

The sub-step length is re-derived before *every* sub-step, so the path is a variable-step
integrator rather than a uniform partition. It is gated on at least one cell being at or
above onset, which is what keeps a pack that never gets hot running the original uniform
partition **bit-for-bit** — verified by a test that runs the same pack with and without a
reaction amplitude and compares telemetry bits, and by the whole pre-existing suite
passing untouched.

### `MAX_SUBSTEPS = 512` does not hold, and the reason is the chemistry not the `dt`

The pre-work asked whether the cap survives. It does not, and the arithmetic says so
before any code runs: a full burn moves a cell by its adiabatic rise
`runaway_energy_j / heat_capacity_j_per_k`, and at 1 K per sub-step that is 253 sub-steps
for LFP and 819 for NMC — **independent of `dt`**, because a runaway completes in a
fraction of a second of simulation time and therefore lands inside a single `Pack::step`
at any realistic client `dt`.

So there are now two caps, not one raised cap. `MAX_SUBSTEPS` (512) is untouched and still
bounds work against a pathological `dt` on the linear path; `MAX_RUNAWAY_SUBSTEPS` (2048)
bounds the adaptive path at ~2.5x the worse chemistry's full burn. The table is in the
const's doc comment per `CLAUDE.md`'s rule that a raised cap comes with its working.
`a_whole_burn_fits_inside_one_coarse_step` runs an entire burn at `dt` = 60 s: the cap
carries a `debug_assert` and tests run in debug, so a cap that bound would fail by
panicking rather than by producing a plausible wrong number.

### The two-`dt` test the pre-work required, and two ways of getting it wrong

Both wrong versions were written first and both passed, which is the point of recording
them.

**Comparing vent times proves nothing.** Vent sits only 30 K above onset, where the
release is 5–33 W and the sub-step is `dt`-limited in both arms. Two `dt` agree there
whatever the integrator does — the vent-time comparison passes with the accuracy cap
widened a thousandfold. The accuracy cap only starts binding once the release passes
~50 W, i.e. *after* vent, so the leg from vent up to 600 K is the only window that can
discriminate.

**Comparing temperature at matched times is worse than useless.** In that climb the cell
gains ~68 K/s, so a 0.2 % disagreement in *timing* — which is convergence, not failure —
shows up as **32 K**. An exponential converts phase error into amplitude error, so
amplitude is the wrong axis entirely. The shipped test compares the climb's elapsed
*duration*, with the crossing times linearly interpolated inside the step that crosses so
the coarse arm's own observation quantisation does not swamp what is being measured.

### Ignition lags by one step, on purpose

Whether the reaction runs during a step is decided from start-of-step temperatures, so a
cell crossing onset *during* a step ignites at the start of the next one. The alternative
is scanning every cell's temperature between thermal sub-steps, which every pack in the
world would pay for so that a burning one could ignite a fraction of a step sooner. Same
family as the BMS's one-step sensor lag and the fault queue's `dt` granularity. Recorded
on the module with its revisit trigger: a scenario needing `dt` coarse enough that a cell
crosses onset and reaches vent inside one step.

### Venting is a state predicate, not a latched flag

`flags.rs` says flags are recomputed fresh each step and are not sticky, so `VENTED` is
re-derived every step from a per-cell `vented` bit — "this pack contains a vented cell",
exactly as `CONTACTOR_OPEN` means "this pack's contactor is open". The bit itself is
irreversible state. `THERMAL_RUNAWAY` comes from *inside* the integrator's gate rather
than from a start-of-step observation elsewhere, so the first step of a runaway flags it.

What v1 does **not** model is what venting *does*: a real vented cell ejects electrolyte
and gas, loses mass and heat capacity, and stops being a battery. Here it keeps
conducting, keeps its heat capacity, and keeps its charge. `VENTED` is an honest report of
a temperature having been reached and nothing more.

### Runaway does not ride the aging sub-clock

Plating's consequences do, and cost nothing on a pack with `aging: None`. Runaway cannot
inherit that: the phase exit criterion is "BMS off, overcharge, runaway, propagation" and
says nothing about aging, and a client that switched aging off to hold a scenario fixed
has not asked to be made fireproof. `CellRunaway` is therefore its own always-present
field on `Cell`, not part of `CellAging` — and not part of `EcmState`, for the Phase-6
reason slice A gave for SOH. `a_pack_that_cannot_age_can_still_burn` pins it.

### The `[safety]` runaway trio was never evaluated, and both files moved

The exposure slice A predicted was real. `sim-data`'s
`shipped_runaway_coefficients_burn_at_a_plausible_scale` checks two shape quantities per
chemistry, both against bands rather than fits:

- **Adiabatic ceiling** `runaway_energy_j / heat_capacity_j_per_k`, banded 100–1200 K.
- **Adiabatic onset-to-vent time**, integrated through the engine's own `reaction_power`
  so the test cannot drift from the model, banded 1 s–1 h. This is the check with teeth,
  because it is the only one that touches the two invented coefficients.

Both shipped files failed the first check as written. LFP's 60 kJ over 95 J/K implied a
632 K rise; NMC's 90 kJ over 55 J/K implied **1636 K**, hotter than the cell's own
materials survive. Rescaled to 24 kJ (253 K rise, ~400 degC peak — LFP peaks far below
NMC in ARC tests) and 45 kJ (818 K rise, ~990 degC peak, which is what ARC tests report
for NMC 18650s). Both provenance notes record the old value and why it moved, the same
treatment slice A gave the NMC aging pair.

### Propagation, and the control that makes it a test

`a_burning_cell_takes_its_neighbour_with_it` uses an injected `SoftInternalShort` to heat
one cell of a 2S1P pack — fault injection is the only sanctioned override, and it is used
only to *start* the fire. Everything after that cell crosses onset is physics.

The control is what gives it teeth: the identical pack with the reaction amplitude set to
zero runs the identical heater, and its neighbour never gets near onset. The fixture is
tuned so the heater's own steady state leaves cell 0 hot and cell 1 well below onset,
which is what makes that true. Without the control the test would pass on a pack where the
heater alone cooked both cells, and would be evidence of nothing.

### Overcharge with the BMS off already reaches vent — the energy hole is separate

Slice E owns the full exit scenario, but the reachability question was answered here with
a throwaway probe on the shipped LFP file, because the answer determines whether slice D
also owed a cell-model change. It does not: ohmic heating alone gets there. A 1S1P at 20C
vents, and so does a 5S5P at 8.7C per cell — an abusive fast charge, which is exactly the
regime the scenario is about.

**A real hole was found on the way, and is deliberately left open.** Charge pushed into a
cell whose SOC is clamped at 1.0 currently vanishes: it is not stored, and it generates no
heat beyond `I^2*R0`. That is silent energy destruction, and the honest shape is
`OCV * (rejected charge)/dt` driven from the *rejected fraction*
`(raw_soc - 1.0)*capacity_as/dt`, not from a boolean "clamped" — the boolean version
passes tests and is wrong at every clamp entry. The discharge side has the mirror image:
at `soc = 0` the cell keeps sourcing current at `OCV(0)` forever.

It is not slice D's: it is a change to `coulomb_step`/`cell_heat_w`, the most
golden-tested path in the tree, and it is a cell-model fix wearing a runaway costume. It
wants its own commit and its own goldens re-check.

### Perf: what slice D added to the hot loop, for slice E to measure

- **One comparison per cell per step** (`t >= onset_k`) during the temperature-gathering
  pass, on the live-thermal path only. That is the entire cost on a pack that is not on
  fire: below onset the per-cell exothermic state is not even gathered, and the integrator
  runs its original uniform partition on the original `heat_w` slice.
- **Sixteen more bytes per `Cell`** (`CellRunaway` is an `f64` plus a `bool`, padded), on
  top of slice B's eight and slice C's sixteen. `Cell` has now grown in four consecutive
  slices, and slice E should look at the total rather than the increment.
- Everything else — the per-cell reaction evaluation, the two scratch buffers, the
  variable sub-stepping — is behind the onset gate and costs a healthy pack nothing.

The benchmark's `lfp_like_chem` now carries the two new coefficients at their shipped
values, so the gate is measured on the configuration that ships rather than optimised away
by a zero amplitude nobody uses.

### Follow-up: the vent latch was the fourth member of the `dt > 0` family, and shipped ungated

Caught in review, not by a test. The reporting pass wrote `cell.runaway.vented = true`
from end-of-step temperature with no `dt > 0` gate, so a zero-length probe on a pack
already past `t_vent_k` flipped an irreversible bit that `snapshot()` then captured. The
act of *looking* at a hot pack changed its state, and a trajectory would have depended on
how often a client probed.

The distinction the code crossed is one this slice's own module docs draw: plating's flag
needs no gate **because detecting it mutates nothing**. Venting is the case where the
observation and the state change sit in the same line, and the fix is to separate them —
read the predicate for reporting, write the latch only when time passes. Both properties
are keepable, and a probe still answers "yes, this pack contains a vented cell."

Why neither existing test found it, which is the more useful half: the zero-length-step
test starts at onset, thirty kelvin below vent, so the latch never fires; the isothermal
test builds *above* vent but steps at `dt = 1.0`. Each covered one of the two conditions
and neither covered both. `a_zero_length_step_reports_venting_without_latching_it` builds
above vent *and* probes, and fails against the pre-fix code with "a probe step latched the
vent bit".

The general shape for slice E and beyond: every new piece of per-cell state wants the
question "can an observation write this?" asked explicitly, because
`snapshot.rs::zero_length_step_does_not_mutate_state` runs a pack at ordinary temperatures
and will not ask it for you.

### `MAX_RUNAWAY_SUBSTEPS` is justified per-cell, and slice E is where that could bind

The cap's doc reasons about "two cells igniting in sequence". Simultaneous burns cost
nothing extra — the rise cap uses the maximum node heating across cells, so N cells
burning together need the same sub-step count as one. **Sequential** ignitions are what
multiply it, and slice E's exit scenario is a multi-cell `SxP` pack at a coarse `dt` where
several cells can ignite one after another inside a single step.

It self-reports rather than degrading silently: the `debug_assert` fires, and slice E's
tests run in debug. Recorded here so that if it does fire, the next person reads it as the
work cap binding rather than as a physics bug.

---

## Learned while building — slice E (wrap-up)

**No snapshot bump.** Slice E adds tests and two benchmark cases and changes no engine
code, so v9 stands — the first Phase 3 slice for which the end-of-slice layout check (the
habit this plan opens by insisting on) comes back clean.

New: `sim-core/tests/scenario_runaway.rs` (4 tests), `sim-core/tests/scenario_aging.rs`
(3 tests), three properties in `sim-core/tests/properties.rs`, and two `full+aging`
benchmark cases. Both exit criteria pass.

### The exit criteria, and the fixture work each one actually needed

**The runaway scenario needed a pack with a temperature gradient, and the benchmark's
`k_neighbor` destroys one.** The chain the criterion asks for — overcharge, ignite,
propagate — only means "propagate" if one cell reaches onset appreciably before its
neighbours. But a strongly-coupled pack is nearly isothermal inside: the centre cell
conducts its whole generation `q` to four neighbours, so the gradient is `q/(4k)`, which
at the benchmark's `k` = 1 W/K and a realistic `q` is about **4 K**. Every cell would
ignite together and "a neighbour followed" would be a statement about rounding.

At `k` = 0.1 W/K the same fixture separates cleanly: centre 431.7 K, first ring 404.7 K,
corners 383.8 K. The scenario ships at that value with the reasoning on the constant,
because a reader who sees it disagree with the benchmark's 1 W/K is entitled to know it
was chosen rather than defaulted.

**The current then has to be tuned into a 30 K window, and both edges are load-bearing.**
Ohmic heating alone must plateau *above* `t_onset_k` — otherwise nothing ever ignites and
the live arm is testing a pack that cannot burn — and *below* `t_vent_k`, or the control
arm vents on `I²R` and venting proves nothing about the reaction. The first fixture
attempt (72 A) plateaued at 462 K, nine kelvin past vent, and the control obligingly
vented every cell. At 60 A — 8.7 C per cell, which is the rate slice D's throwaway probe
had already found — the plateau is 431.7 K: 8.5 K above onset, 21.5 K below vent.

What the tuned fixture then produces is the criterion almost verbatim. Runaway is flagged
at 2575 s, the centre vents at 2918 s, all four of its 4-connected neighbours at 3409 s,
and the corners at 3652 s — the fire spreading outward one ring at a time, through the
same `k_ij` links Phase 2 shipped.

A third arm was worth adding beyond the two the criterion implies: the same abuse through
a **protective BMS**, which derates 60 A to 6.91 A and leaves the pack at 298.96 K. Without
it the scenario shows only that a fixture built to burn burns; with it, the pack burns
exactly when the protection that exists to prevent it is absent, which is the contrast the
phase is built around.

**`MAX_RUNAWAY_SUBSTEPS` did not bind.** The pre-work flagged this slice as where
sequential ignitions at a coarse `dt` could hit the 2048 cap; nine cells igniting in three
waves at `dt` = 1 s did not, and the `debug_assert` that would have reported it runs in
every one of these tests.

**The 500-cycle criterion is two experiments, and this plan's own wording hid that.** The
pre-work asks the fade curve to be "monotone decreasing, decelerating (the √t signature),
resistance rising as capacity falls, faster fade at higher temperature and higher SOC".
Those cannot all hold on one trajectory: calendar fade goes as `√t` and decelerates, cycle
fade is linear in throughput, and 500 cycles is their sum. A single test asserting
"decelerating" would have been asserting whichever term the fixture happened to let
dominate.

So the shape claims live on a rested pack (`√t`, both stress orderings) and the cycling
claims on the cycled one (monotone, resistance pairing, near-linearity). Measured at the
shipped LFP coefficients: a year on the shelf at 25 °C and full SOC fades **13.7 %**, four
years fades exactly twice that — the ratio is `2.0` to within 1e-9, which is the
rationalised calendar increment being *exact* rather than approximately right — and 500
full-depth cycles fade **7.07 %**.

**The cycle term is isolated by a control arm, not by subtracting a rest run.** The obvious
control — cycle one pack, rest another for the same duration — conflates two things,
because a cycled pack also spends time at high SOC where the calendar stress factor is 1.4.
The control that works is the *same cycling run* with `cyc_fade_per_ah` set to zero:
identical demands, identical SOC history, identical elapsed time, one coefficient
different. Subtracting it leaves the cycle term alone, and the two signatures then separate
exactly as they should — the cycle term costs 1.86 % then 1.78 % over the two halves (a
ratio of **0.956**, near-linear), the calendar term underneath it 2.43 % then 1.01 % (a
ratio of **0.414**, which is the `√t` signature).

The subtraction is very slightly approximate and the test says so: the faded arm's cycles
are marginally shorter, because the same 1 C current crosses a smaller tank faster, so it
accrues marginally less calendar fade than its control. Under a percent of the term being
isolated.

One fixture detail worth copying: the cycling turnarounds are on **SOC**, not on a step
count. SOC is the fraction of the capacity the pack has *today*, so a fixed step count
would deepen every cycle as capacity fell, quietly turning a constant-depth experiment into
an increasingly abusive one.

### A property that passed for the wrong reason, and the guard added because of it

`health_never_improves` asserts capacity SOH never rises and resistance SOH never falls. It
passed on the first run. It was also worthless: the fixture's `cal_pre_exp` was set to
`1e8` to make aging visible in a property's short trajectory, and `1e8` fades a cell by
**2.46** over the longest run the generators produce — i.e. straight through
`MIN_SOH_CAPACITY` within a few steps, after which the property was asserting the
monotonicity of a clamped constant and would have passed against an engine that did no
aging at all.

The lesson generalises past this one test: **a monotonicity assertion is satisfied by a
constant**, so any property of that shape needs a companion assertion that the quantity
actually moved. Both are now there — health must have degraded, and must not be sitting on
the floor — and `aging_chem`'s doc comment records the number and why it moved. At `5e4`
the same trajectories fade between 6e-5 and 1.2e-3.

The same guard then caught a genuine engine behaviour in the round-trip property: with the
sub-clock period at 10 s, a one-step 0.5 s trajectory never ticks aging at all, so demanding
fade would have been demanding something the engine is correct not to do. Those short
trajectories are deliberately *kept* — a pack snapshotted mid-period is precisely the case
where the accumulator has to survive — so the assertion is conditioned on the sub-clock
having fired rather than tuned out of the input space.

### The round-trip property was the coverage hole four slices had been widening

Four consecutive slices added per-cell state — the SOH pair and its accumulators, the
plating charge counter, the shunt conductance, the exothermic budget and vent latch — and
the existing `snapshot_roundtrip_continues_identically` runs an unaged, fault-free,
`[safety]`-less pack. None of that state had ever crossed a serde boundary under proptest,
which is design principle 5's central claim going untested precisely where it got hardest.

`snapshot_roundtrip_survives_aging_faults_and_plating` runs the pack **cold**, below
`t_plating_min_k`, so every charging step above the C-rate threshold plates: that
accumulates `q_plating` *and* rolls the seeded hazard, so the RNG stream has to survive the
round trip and not merely the float state. It compares the tail step by step rather than
only at the end, because a restored pack whose draws resumed one step out of phase would
agree on the first step and diverge later. Verified by mutation: marking `q_plating`
`#[serde(skip)]` fails it and nothing else.

### Perf: measured at last, and the deferred optimisation is now declined

Paired alternating rounds against `9da78ef` (the last pre-slice-A tree), `100S10P`, with the
traps `docs/plans/pack-step-perf.md` lists. Five rounds; **round 3 discarded** because its
`full` arm came in *cheaper* than its `current` arm, which is impossible — `full` is a
strict superset of the work — and is therefore a machine transition mid-round rather than a
measurement. Round 2 sits well outside the other three and is treated as an outlier.

| case              | base (µs)   | slice E (µs) | Δ              |
| ----------------- | ----------- | ------------ | -------------- |
| `100S10P/current` | 52.4 – 54.9 | 57.1 – 59.6  | **+8 – 9 %**   |
| `100S10P/full`    | 58.1 – 63.8 | 64.0 – 68.0  | **+7 – 10 %**  |
| `1S1P/current`    | 144 ns      | 162 – 165 ns | **+12 – 14 %** |

So the headline is that **Phase 3 slices A–D cost the step 7–10 % at 100S10P**, sign certain
(the slice E arm was slower in every case of every kept round) and magnitude finally
pinned, which is what slices A and B each deferred to here.

**The budget is again unverifiable on this box, for the third session running.** The
baseline arm measured 51–55 µs for `100S10P/current` where `pack-step-perf.md` recorded
36–42 µs, so this machine is in its slow state at roughly 1.35×. Scaling the ratio onto the
fast-state anchor puts the fully-featured step at ≈ **42–54 µs** against a < 50 µs budget —
which is to say the budget has gone from comfortably met to **marginal**, and slice E does
not claim it is met. That is a scaled inference and the range is the answer.

**Aging itself is nearly free until the sub-clock fires, and expensive when it does.**
Measured within single runs, so these three are mode-matched to each other:

| case                          | µs (two runs) |
| ----------------------------- | ------------- |
| `full`                        | 68.8 / 68.5   |
| `full+aging` (10 s period)    | 70.8 / 68.4   |
| `full+aging_every_step` (0 s) | 104.8 / 102.3 |

`full+aging` is indistinguishable from `full`: the always-paid part of slice A —
`eff_r0_factor`'s extra multiply per cell, and the SOH aggregation in the reporting pass —
is **below this box's noise floor**. The tick is the whole cost, at **+50 %** on a step that
runs it. At the shipped 10 s default against a 0.1 s client `dt` that is one step in a
hundred, so ~0.5 % amortised; a client that sets `sub_clock_period_s = 0` is choosing to pay
it every step, which is legitimate and worth knowing.

**This declines the optimisation slice A sketched.** Caching `r0_factor · soh_resistance` as
a derived field on `Cell` would remove a multiply from the non-ticking path — the path that
already measures as free — and would add eight bytes to a `Cell` that has grown from **64 to
160 bytes** across Phase 3 (`CellAging` alone is 72 of them). Slice A asked for a measurement
to justify it; the measurement declines it, and it carries a correctness obligation that
would now buy nothing.

**What the numbers suggest instead**, recorded as a hypothesis rather than a finding because
nobody has profiled this: `Cell` at 2.5× its old size is 160 KB of hot state at 100S10P
instead of 64 KB. Most of the growth is `CellAging`'s accumulators — `q_cal`, `q_cyc`,
`q_plating`, `ah_since_tick`, `ah_plating_since_tick`, `soc_ref`, `discharging` — and **none
of those is read on a non-ticking step**. Splitting them into a parallel array touched only
at the aging tick would take `Cell` back toward 96 bytes while leaving the two `soh_*`
multipliers hot. That is a structural change with its own snapshot-layout consequences, so
it belongs in its own item, not in a wrap-up slice.

The evidence is not all one way, and the counter-evidence is worth keeping: `1S1P` — where
there is no cache pressure at all — shows a *larger* relative penalty (12–14 %) than
`100S10P` does, so per-step fixed work is clearly part of the cost too. Both effects are
present; neither is separately attributed, and the division-count style of reasoning that
`pack-step-perf.md` warns about would be exactly the wrong way to settle it. Profile first.
