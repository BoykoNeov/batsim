# Phase 10 — a film that ages the cell, instead of a curve that was drawn

**Status: PLANNED 2026-10-09.** Written after the spike (`phase-10-spike.md`), which is the
measurement this plan stands on; where a later slice note contradicts this text, the slice
note is the measurement and wins. When written the repo was at `SNAPSHOT_VERSION` 25 and
`WASM_API_VERSION` 9.

This replaces the slice list and exit in `docs/ROADMAP.md` §Phase 10, which named a
reaction-limited film and asked for `sqrt(t)` calendar fade from it. On the spike's
measurement a reaction-limited film grows linearly on a shelf, so that slice and that exit
could not both stand.

## The owner's decisions, recorded

All three were asked and answered on 2026-10-09.

1. **The interstitial-diffusion growth law** (PyBaMM's `"interstitial-diffusion limited"`,
   Marquis thesis eq. 5.96): `j = −(D_li·c_li0·F/L)·exp(−F·Δφ/RT)·arrhenius`. Of the four laws
   measured it is the only one whose shelf fade is `sqrt(t)`-shaped once the film outgrows its
   start **and** faster at high state of charge (6.47× between 100 % and 30 % at ×100). The
   alternatives were solvent-diffusion (`sqrt(t)` but blind to SOC) and reaction-limited
   (linear).
2. **Fit its magnitude to measured data**, not hand-scale it and not ship PyBaMM's default
   (0.03 % lithium a year — 30 to 50 times too slow). The data is the WMG LG M50
   calendar-ageing set (Kuzhiyil et al. 2025; see the spike note). This adds a fitting slice
   ahead of the engine work.
3. **Keep the film's resistivity at PyBaMM's value, labelled as a placeholder.** It makes the
   resistance rise about 14 % per 1 % of capacity lost, ten times today's
   `r_growth_per_capacity_loss` = 1.5, so an aged cell will feel much stiffer than today's.
   The WMG data records pulse resistance, and slice A checks the film against it; changing
   the value on that evidence is a later decision, not this phase's default.

## What the spike established, and what the plan may therefore assume

| property | measured | what the plan does with it |
| --- | --- | --- |
| interstitial shape | slope 0.569 over days 30–365 when the film grows to ~57 nm; 0.78 at 30 % SOC where it reaches only 13 nm | the exit states the regime |
| interstitial SOC effect | 6.47 (100 vs 30 %), 4.96 (70 vs 30 %) at ×100, 25 °C | an exit criterion, at the fitted constants |
| closed-form tick | within 0.02 % of PyBaMM at 1-day ticks, 0.21 % at 30-day | the sub-clock carries the film; no sub-stepping |
| film resistance | ~63 mΩ per Ah of lithium lost, any law; 0.298 mΩ of 22.28 at 5 nm | the resistance growth falls out, at a labelled ρ |
| capacity vs lithium | C/20 capacity lost ≈ 1.45 × lithium lost % | capacity is read off the electrodes, never copied from the film |
| provenance | OKane2022's SEI block is PyBaMM's shared defaults | every SEI constant gets its own provenance line |

Not established: anything under current (the spike rested the cell), the 0 °C behaviour,
whether the WMG fade is mostly lithium loss or also active-material loss, and the cost in
the engine.

## Slices

### A — the fit (Python only, no Rust)

`tools/reference/fit_sei_wmg.py`, run in the reference venv. Pre-registered like a spike.

- Inputs: the 39 analysed WMG files (capacity, pulse resistance and C/20 curves at each
  reference test). They are not committed; the script names the record and the file names.
- Model: the spike's closed-form integrator (graphite potential from the Chen2020 OCP at the
  storage SOC, moved by the lithium the film takes), checked against a PyBaMM run at the fitted
  constants before any number is quoted.
- Fitted: `D_li` (magnitude) and the activation energy (25 against 45 °C). **0 °C is held out**
  and reported as a prediction. The initial thickness stays PyBaMM's 5 nm, labelled.
- Read from the same data: whether capacity fade tracks the graphite potential across the 13
  SOCs (the law's whole claim), and the measured pulse-resistance rise against the film's.
- Output: a `[sei]` section for `chemistries/nmc_21700_lgm50.toml` with a residual in every
  provenance line, and a note `phase-10-slice-a-fit.md`.
- **Stop and ask** if the data's SOC dependence does not follow the graphite potential, or
  its time shape is not `sqrt`-like: either would undo decision 1.

### B — `AgingModel`, unchanged physics

The semi-empirical law moves behind an `AgingModel` enum beside `CellModel`, **bit-identical**
(every golden, every snapshot pair test). Its shape is settled by where slice C's state must
live (next bullet), so B is written after A's fit lands, not before.

### C — the film on the `Spm` (engine; one snapshot bump, 25 → 26)

- State: film thickness per cell. The lithium it takes leaves the **negative particle**, which
  lives inside `SpmState`; so either the aging variant reaches into the cell model on its tick
  or the model owns the film. `aging.rs` keeps health outside `CellModel` on purpose — this is
  the first mechanism that cannot be a multiplier, and the slice note must say which way it
  went and why.
- Capacity and SOC: lost lithium shifts the electrode windows. `Telemetry::soh_capacity` must
  be read from the electrodes (what a C/20 discharge between the limits would give), not from
  the film. What `soc_true` means once the negative cannot fill is a decision for the note.
- Resistance: the film adds `ρ_sei·L/A_neg` in series. Not through the `m_ref` division
  today's multiplier uses.
- Energy: a side reaction at rest is self-discharge. The lithium's energy must land as heat or
  an explicit sink, or the energy-balance property test opens a hole.
- `Pack::new` refuses the film variant on the ECM (no electrode to take lithium from) and, until
  slice D, on the `Dfn` and the many-particle cell.
- The semi-empirical cycle, plating and reversal terms stay as they are under this variant;
  whether their `r_growth_per_capacity_loss` coupling still applies beside a physical film is
  for the slice note to measure, not assume.
- Golden: PyBaMM SPM with the interstitial law at the fitted constants, a year at rest at two
  SOCs and two temperatures, committed under `tests/golden/`.

### D — the same on the `Dfn`

### E — the lesson

A new aging chapter in the guided path (`path-chapters.md`): the same cell aged by the drawn
curve and by the film, stored full and half full. The idle-wear step (step 8) probably moves
into the chapter; that changes quoted step numbers and is this slice's call.

## Exit

Pinned by `crates/sim-data/tests/sei_golden.rs`:

1. A year at rest under the film model matches the PyBaMM interstitial reference within a
   stated tolerance, at two SOCs and two temperatures.
2. Over a span where the film has grown past several times its starting thickness, fade is
   `sqrt(t)`-shaped (log-log slope within a stated band of 0.5) with no `sqrt(t)` in the rate
   law; the test names the regime.
3. Stored full, the cell loses measurably more than stored at 30 %, by a ratio pinned at the
   fitted constants.
4. Resistance rises from the film alone, with no `r_growth_per_capacity_loss` on the path.
5. The fitted constants reproduce the WMG 25 and 45 °C capacity fade within the residual their
   provenance lines state.
