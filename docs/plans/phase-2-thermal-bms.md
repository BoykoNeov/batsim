# Phase 2 — thermal + BMS

**Status:** slice A (thermal network) landed. B–E open.
**Exit criteria** (from `CLAUDE.md`): centre cells run measurably hotter; the LFP
estimator-drift scenario passes; protection scenarios pass with the BMS on, and the
same demands violate limits with it off.

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | thermal network: `[thermal]` chemistry section, `ThermalConfig`, per-cell lumped nodes, heat generation, grid adjacency, Euler + sub-stepping, `Env` consumed, energy-balance property test | **done** |
| B | sensor layer + SOC estimator: `BmsConfig`, sensor frame, coulomb-count estimator with drift, rest-gated OCV correction, `soc_bms` | open |
| C | protection: OV/UV per group, OC (separate charge/discharge), OT/UT, charge inhibit, derate → contactor open, BMS-off contrast scenarios | open |
| D | passive balancing: per-group bleed resistor above a voltage threshold | open |
| E | wrap-up: scenario tests, perf re-measure | open |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean, and bumps
`SNAPSHOT_VERSION` if it changes the serialized layout (A took it 2 → 3). One bump
per layout-changing slice — do not try to share one version across the phase.

## Decisions already made (do not re-derive)

### A temperature gradient comes from exposure, not from the conduction graph

The tempting design — give interior cells more conduction neighbours and let them
run hotter — **provably cannot work**. With uniform heat generation, uniform `h·A`,
and any symmetric conduction graph, substituting `T_i = T*` into

```text
C_th·dT_i/dt = Q_i + Σ_j k_ij·(T_j − T_i) + hA_i·(T_env − T_i)
```

makes the neighbour sum vanish identically, leaving `T* = T_env + Q/hA` for every
cell regardless of its neighbour count. A pack starting uniform stays exactly
uniform forever. An exit-gate test built on that expectation would have asserted a
gradient that cannot exist.

What actually creates the gradient is **position-dependent ambient coupling**: an
interior cell is insulated *by* its neighbours, because shared surface is surface
that no longer faces the environment. `thermal::exposure` implements
`hA_i = h_area_w_per_k·(4 − n_neighbors_i)/4`, so a `1S1P` cell keeps all of it
(which is what makes `h_area_w_per_k` measurable as a bare-cell property, matching
what the chemistry TOMLs say it is) and a fully enclosed cell keeps none, shedding
heat only by conducting toward an edge.

`tests/thermal.rs::conduction_alone_creates_no_gradient` is the negative control
that pins this: with `h_area = 0` a uniform pack stays **bit-identically** uniform
across cells with 2, 3, and 4 neighbours. Keep it.

### Heat generation deviates from the `CLAUDE.md` formula, deliberately

`CLAUDE.md` sketches `Q_gen = I²·(R0 + Σ R_rc)`. That is the *steady-state* special
case, exact only once every `V_rc` has settled to `R_rc·I`. During a transient — the
whole reason RC pairs exist — it is wrong. `ecm::cell_heat_w` uses Bernardi's
irreversible term against the state the engine actually carries:

```text
Q_irrev = I·(OCV − V_node) = I²·R0 + I·Σ V_rc
```

plus the reversible term `Q_rev = −I·T·∂U/∂T` when the chemistry supplies a
`docv_dt_v_per_k` column. This is cheaper (no extra lookup) and it makes the pack
energy balance exact rather than approximate. Same spirit as `solve_current`
documenting its deviation from the prescribed Newton loop.

`properties.rs::electrical_and_heat_energy_balance` is the gate. It closes to
**floating-point rounding** (tolerance `1e-12` relative), not to a physical
tolerance, because heat and current are both evaluated from start-of-step state and
the test accumulates the electrical integral one step behind — for a constant
current, step `n`'s start-of-step node voltage *is* step `n−1`'s reported
end-of-step value. Verified to have teeth: swapping in the steady-state heat form
produces a 5.4 mJ imbalance against a 1 pJ tolerance.

### Goldens stay isothermal, and that is not a cop-out

`ThermalConfig::Isothermal` is the default. The shipped chemistries have a
temperature-dependent `R0` (LFP: 0.020 → 0.016 Ω across 298 → 318 K), so a live
thermal model shifts the voltage trajectory — ~0.2 mV at 1C, ~5 mV at 3C. The
PyBaMM references were generated isothermal, and those tests exist to check the
*electrical* model. Principle 7 ("components are toggleable") makes an explicit
isothermal mode a legitimate configuration rather than a test crutch. Isothermal
still reports `q_gen_w`; it simply does not feed temperature back.

### Slice D: the balancing bleed does **not** need a sampling approximation

A first design computed the bleed current from a lagged sampled voltage to keep the
solve closed-form. Unnecessary — the bleed is just a conductance at the group node.
With `G_b = 1/R_bleed`, KCL gives

```text
V = (Σ E_k/R_k − I) / (Σ 1/R_k + G_b)
```

so the group Thévenin stays exactly linear: `R_g' = 1/(Σ1/R_k + G_b)`,
`E_g' = (Σ E_k/R_k)·R_g'`. Series aggregation and `solve_current` are untouched,
per-cell `I_k = (E_k − V_g)/R_k` is unchanged, and the per-cell currents
automatically sum to `I + I_b,g`. One extra term in a denominator the step already
computes. Report `I_b,g = V_g·G_b` after the fact.

Split of responsibility: the *decision* to close the bleed switch runs off the
lagged sensor frame (that is the BMS control path — principle 8, sensors only); the
*physics* once closed is exact. And the resistor dissipates `V²/R` **in the
resistor**, not in the cell — do not add it to the cell's thermal node. The extra
`I²R0` inside the cell falls out of the solve on its own.

### Slice B: the sensor frame must be serialized

Sensor readings lag by one step (the frame is sampled at the end of a step and
consumed at the start of the next), which is physically honest for a discretely
sampled system and keeps the solve closed-form.

Two constraints that are easy to get wrong:

- **The frame goes in the snapshot.** Do *not* reuse the `#[serde(skip)]`-and-
  recompute trick that `pack-step-perf.md` recommends for the Thévenin cache. That
  works there because the cache is a pure function of current state. A previous-step
  frame is not: the loaded `V_g` depends on a current that is not stored, and any
  noise draw has already advanced the RNG past reproducing it. Miss this and
  snapshot replay breaks.
- **Blindness scales with `dt`.** A real BMS samples at ~10 Hz; this one samples at
  whatever `dt` the client passes. At a coarse fast-forward `dt` protection goes
  nearly inert.

## Open decision — slice C's assertion (needs an owner call)

The choice of assertion decides the design, so write the assertion first:

- **"Detects, derates, settles within limits; BMS-off runs away."** One-step lag is
  fine. Cheapest, and honest about sampling delay. But "with the BMS on, `v_max` is
  never exceeded" is *unassertable* — the first step always overshoots.
- **"Never violates."** Needs predictive clamping. The honest version costs one
  config field: give the BMS an estimated internal resistance `r_est` and clamp on
  `V_pred = V_meas − (I_req − I_meas)·r_est`. The gap between `r_est` and the true
  `R_g` is exactly the truth-vs-estimate gap principle 8 wants exposed, so it is a
  feature rather than overhead.

Recommendation: the predictive version. It costs little, it makes the stronger
assertion available, and the estimator error it introduces is pedagogically the
point.

## Slice A implementation notes

- Grid geometry is derived arithmetically from `(series, parallel)` per cell —
  nothing is stored, so the snapshot stays clean and `1S1P`/`100S1P`/`SxP` all work
  without config.
- Integration is explicit Euler with automatic sub-stepping. The ceiling is
  `0.5·C_th/max(4k, hA)` (a node's conductance is linear in neighbour count, so the
  worst case is at an endpoint — using the bound rather than a per-cell scan keeps
  the sub-step count a function of config alone, so the trajectory does not depend on
  *where* the hottest cell sits). `MAX_SUBSTEPS = 512` bounds one step's work; for
  shipped LFP parameters that cap only binds above `dt` ≈ 1.7 h.
- **Known limitation for Phase 3:** the coarse-`dt` aging fast-forward will exceed
  that ceiling. Raising the cap is the wrong answer — the coupled thermal system is
  linear in `T` over a step, so an exact/implicit integrator is available if it comes
  to that. Revisit when Phase 3 needs it, not before.
- `Env::t_coolant` replaces ambient as the sink with the same `h·A`. A separate
  coolant conductance would be a refinement; it is not modelled.
