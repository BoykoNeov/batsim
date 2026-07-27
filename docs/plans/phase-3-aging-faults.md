# Phase 3 — aging + faults

**Status: planned.** No slice has landed. This file is written before the work so the
decisions below are made once; append the "learned while building" material to it as
each slice lands, the way `phase-2-thermal-bms.md` grew.

| exit criterion (from `CLAUDE.md`) | to be met by |
| --------------------------------- | ------------ |
| A fast-forward of 500 cycles shows a plausible fade curve | `sim-core/tests/scenario_aging.rs` — LFP only (see "Exit criteria stay off NMC"), asserting curve *shape*, not a fitted number |
| "BMS off → overcharge → runaway → neighbour propagation" passes | `sim-core/tests/scenario_runaway.rs` — a `SxP` pack, BMS off, overcharged until a cell vents and at least one neighbour follows |

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | aging: `[aging]` into `ChemistryParams`, per-cell `soh_capacity`/`soh_resistance` + calendar accumulator on `Cell`, calendar **and** cycle fade, resistance growth, the aging sub-clock, pack-level SOH in `Telemetry` | planned |
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

- **Aging sub-clock period.** `CLAUDE.md` suggests ~10 s of sim time. It must be a
  config value, and the accumulator has to survive snapshot/restore or a snapshot taken
  mid-period changes the trajectory. Whether a partial period is dropped or carried at
  restore is a determinism question, not a taste question.
- **Cycle-fade DOD accounting without rainflow.** `CLAUDE.md` explicitly rules rainflow
  counting out for v1 in favour of throughput × stress weights. What plays the role of
  DOD in a weighting applied per sub-clock tick — instantaneous SOC, or a running
  min/max since the last current reversal — needs one answer, stated where the fade
  function lives.
- **Plating's soft-short probability** draws from the pack RNG, so it must draw in a
  fixed order over cells (series-major, parallel-minor, like the scatter draws) or the
  trajectory stops being a function of the seed. It also must not draw at all when the
  probability is zero, for the same reason `draw_factors` short-circuits on zero
  scatter.
- **Does an external short bypass the contactor?** Physically it depends on which side
  of the contactor the short sits. Pick one, document it on the fault variant, and make
  sure the BMS-off contrast scenario is not accidentally testing the other.
