# Phase 3 — aging + faults

**Status: slice A landed; B–E planned.** This file was written before the work so the
decisions below are made once; the "learned while building" material is appended as
each slice lands, the way `phase-2-thermal-bms.md` grew.

| exit criterion (from `CLAUDE.md`) | to be met by |
| --------------------------------- | ------------ |
| A fast-forward of 500 cycles shows a plausible fade curve | `sim-core/tests/scenario_aging.rs` — LFP only (see "Exit criteria stay off NMC"), asserting curve *shape*, not a fitted number |
| "BMS off → overcharge → runaway → neighbour propagation" passes | `sim-core/tests/scenario_runaway.rs` — a `SxP` pack, BMS off, overcharged until a cell vents and at least one neighbour follows |

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | aging: `[aging]` into `ChemistryParams`, per-cell `soh_capacity`/`soh_resistance` + calendar accumulator on `Cell`, calendar **and** cycle fade, resistance growth, the aging sub-clock, pack-level SOH in `Telemetry` | **landed** (v6) |
| B | fault queue: timestamped injection API, `WeakCell`, `SoftInternalShort`, `ExternalShort`, `SensorStuck`/`SensorOffset` | planned |
| C | plating: `PLATING_RISK` from cold-charge physics, accelerated fade, seeded soft-short probability | planned |
| D | runaway: Arrhenius self-heating with a finite per-cell energy budget, `VENTED`, `THERMAL_RUNAWAY`, propagation, and the sub-step bound that makes it integrable | planned |
| E | wrap-up: the two exit scenarios, aging/fault property tests, perf re-measure | planned |

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
- **Plating's soft-short probability** draws from the pack RNG, so it must draw in a
  fixed order over cells (series-major, parallel-minor, like the scatter draws) or the
  trajectory stops being a function of the seed. It also must not draw at all when the
  probability is zero, for the same reason `draw_factors` short-circuits on zero
  scatter.
- **Does an external short bypass the contactor?** Physically it depends on which side
  of the contactor the short sits. Pick one, document it on the fault variant, and make
  sure the BMS-off contrast scenario is not accidentally testing the other.

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
