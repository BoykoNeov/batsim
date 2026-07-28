# UI / pedagogy — making the engine's own capabilities visible

Not a numbered phase. Phases 0–6 built an engine; this is the client catching up to it.

## The finding this work starts from

Three engine capabilities are wired all the way to the client boundary and then dropped
on the floor:

| capability | reachable from | used by `web/app.js` |
| --- | --- | --- |
| per-cell ground truth | `Sim::cells()` (wasm), `GET /sessions/{id}/cells` (REST) | **no** |
| fault injection | `Sim::schedule_fault` (wasm), `Command::ScheduleFault` (WS) | **no** |
| clearing faults / latched BMS trips | `clear_faults`, `clear_bms_fault`, both transports | **no** |

The consequence is the headline: **the pack simulator's own UI cannot show you a pack.**
Every plot is an aggregate or a min/max envelope. Imbalance, the current split inside a
parallel group, the thermal gradient that makes centre cells run hot — all simulated,
none visible. `CLAUDE.md`'s principle 2 says the pack is the first-class citizen; the
client renders it as a single number and two envelopes.

## Scope of this slice

Two items, chosen because they need **no Rust change and no version bump** — the
capability is already at the boundary.

1. **Per-cell pack grid.** An S×P tile grid of ground truth, one metric at a time,
   hover/click for a cell's full `CellView`.
2. **Live fault panel.** Inject any of the five `Fault` variants at the current sim
   time or a delay; clear the pending queue; clear a latched BMS fault.

Both work on **both** backends. `SNAPSHOT_VERSION`, `API_VERSION` and
`WASM_API_VERSION` are all untouched — this slice adds no wire contract.

## Two limitations found while scoping, stated rather than worked around

**`CellView` carries no per-cell voltage and no per-cell current.** Its fields are
`soc`, `temp_k`, `overpotential_v`, `capacity_factor`, `r0_factor`, `soh_capacity`,
`soh_resistance`, `internal_short_conductance_s`, `runaway_energy_remaining_j`,
`vented`. Phase 6 slice D explicitly declined a per-cell current accessor ("public API
added for a test's convenience"). So the grid shows the SOC spread and the temperature
gradient — which *is* the imbalance story and *is* the thermal story — but "which cell
is taking the most current right now" is not answerable in this slice, and the plan does
not pretend otherwise. Adding it is a `sim-core` change and belongs with the version
bump below.

**The BMS's *sensed* per-group voltage is not exposed by any adapter.**
`Bms::sensors() -> &SensorFrame` is public in `sim-core` and `SensorFrame.v_group` holds
exactly the fault-injected, offset, lying values the BMS actually reads — and a grep of
`sim-wasm`, `sim-server` and `sim-godot` returns nothing. So the flagship principle-8
demo (ground truth beside what the BMS believes, *per group*) cannot be drawn today. The
page can already show `soc_bms` against `soc_true` as scalars, and that stays the whole
of the truth-vs-estimate story until an adapter exposes the frame.

That exposure is the natural next slice and is where the first version bump should go:
`API_VERSION` 2→3 and `WASM_API_VERSION` 2→3 together.

## Semantics the panel must respect, checked in the engine rather than assumed

- **A past-dated fault fires on the next step, not retroactively.** `FaultQueue::take_due`
  partitions on `at_s < t_end`, and its doc says faults dated before now are part of that
  prefix. So "inject now" means "scheduled at the current sim time, fires when the pack
  is next stepped" — and while the run is paused, nothing happens. The UI says so instead
  of looking broken.
- **A zero-length read does not fire it.** `readNow()` steps with `dt = 0`, so
  `take_due(sim_time_s)` finds nothing at `at_s == sim_time_s`. That is the zero-length
  contract working as designed (a read must not mutate), not a missed fault. The panel
  therefore reports "queued", never "applied".
- **Sensor faults need a BMS to land on.** `facts.sensor_faults_dropped` already counts
  the ones dropped for want of one. The panel warns before injecting rather than letting
  the count silently tick up.
- **`WeakCell` replaces, it does not multiply.** Its doc is explicit: the new factors
  replace the cell's scatter draw or an earlier `WeakCell`. The field labels say so.

## Colour, and the rule that decided it

The grid is a **sequential** encoding (magnitude), so it is **one hue per metric,
never a rainbow** — the first instinct here was a blue→cyan→yellow→red ramp and that is
exactly the anti-pattern. The surface is dark, so each ramp runs dark→bright rather than
the light-mode light→dark. Temperature gets the page's existing warm token, every other
metric the accent token; two scales with different hues is not a rainbow, a single scale
with several is.

Two consequences that are requirements, not polish:

- **Every tile renders its value as text**, so identity is never colour-alone.
- **The ink flips** past the bright end of the ramp, because light text on a bright tile
  fails contrast. The flip point is a constant next to the ramp.

The range is the pack's own min→max for the selected metric, not a fixed domain: a 3 mV
SOC spread is the thing worth seeing, and a [0, 1] axis renders it as fifty identical
tiles. The legend prints both ends so the scale is never guessed.

## Deliberately not in this slice

- **No new scenarios.** The `<option>` list is hardcoded at `index.html`, and the server
  has no listing route, so every added scenario costs an HTML edit. The route comes
  first; then scenarios are cheap. Both are separate slices.
- **No CC-CV charge button.** Genuinely client-side policy per `CLAUDE.md`, genuinely
  wanted, and independent of these two — it does not need the grid and the grid does not
  need it.
- **No per-cell history.** The grid samples the present. Plotting one cell's trace over
  time means a second history buffer keyed by cell, and the pinned-cell detail readout
  answers most of what it would have.
- **No `web/pkg` commit.** It is gitignored and always has been. The bundle must be
  rebuilt to see any of this — `start-frontend.bat` does it.

## Gate

No test file is modified, and the out-of-tree trajectory instrument cannot move: it
builds its nine cases from scenario TOML through `parse_scenario`/`build_pack`, and this
slice adds no Rust. `cargo test --workspace`, clippy and fmt still run — a client-only
change should leave them exactly where they were, and saying so is worth the minutes.
