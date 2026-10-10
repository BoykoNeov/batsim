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

## Addendum 2026-10-09 — slice A ran; the owner re-decided

Slice A (`phase-10-slice-a-fit.md`) fitted the film's rate to the WMG cells: `D_li` 939 times
PyBaMM's default, activation energy 48–53 kJ/mol, the one-year ranking across SOC right (rank
correlation 0.81 at 25 °C). Its stop rule fired on the time shape: about half of the cells'
storage-charge effect arrives in the first month, at every temperature including 0 °C, and has
mostly stopped growing by day 200, which no film can do. The film's resistance came out 15–25
times the cells'. The owner's answers, which amend the decisions above:

- **Decision 2 stands and gains a mechanism**: the film stays, and a second, fast, levelling-off
  mechanism carries the front-loaded part. Its physical identity (the anode overhang is the lead
  candidate) is settled by a spike, pre-registered, **before** slice B; the slice list below is
  extended by that spike's result, not rewritten now.
- **Decision 3 is reversed**: the film's resistivity is fitted to the data's resistance rise
  (2.5–4.0 mΩ per Ah of storage-driven loss), not kept at PyBaMM's value.
- Exit criterion 5 now reads against the film **plus** the fast mechanism.

## Addendum 2026-10-09 (later) — slice A done; the film only

The fast part was spiked (`phase-10-slice-a-fit.md` §"The fast part, spiked"): a one-time step
fits the cells as well as an anode overhang and predicts the held-out 0 °C cells better, so the
owner chose **the film only**, with the step recorded as unexplained. That replaces the first
bullet of the addendum above (no second mechanism). Fitted on Chen2020 by
`tools/reference/fit_sei_wmg.py`: `D_li` 5.93e-19 m²/s, 96.9 kJ/mol, `ρ_sei` 5.47e4 Ω·m. Exit
criterion 5 therefore reads against the film with the step set aside (0.85 % RMS on 25 + 45 °C,
0.48 % on 0 °C). The next slice is B (`AgingModel`, unchanged physics), and C carries the
`[sei]` section with its loader.

## Addendum 2026-10-09 (last) — slice A's constants corrected

The constants in the addendum above are superseded (`phase-10-slice-a-fit.md`, last addendum):
the held-out figure there tested the discarded step, and the capacity ratio came from the wrong
protocol. Refitted to growth after the first post-storage test, with the ratio measured on WMG's
capacity steps (1.567): `D_li` 1.480e-18 m²/s, **42.5 kJ/mol**, `ρ_sei` 5.59e4 Ω·m (0.54–2.53 % of
resistance per 1 % capacity across the per-temperature fits). **Exit criterion 5 is restated:**
the engine's film reproduces the WMG cells' storage-driven fade *growth after the first
post-storage test* within 0.62 % RMS at 25 + 45 °C and predicts the held-out 0 °C cells within
0.41 %; the early step is not modelled and the criterion does not score it.

## Addendum 2026-10-10 — slice B: the film lives in the cell model, and health has two doors

**Owner, 2026-10-10: the film's state lives inside the porous cell model** (`SpmState` first),
not in `aging.rs`. Everything the film touches is inside the model already — the negative
surface's potential, which sets its rate; the negative's lithium, which it consumes; and a
series resistance `ρ_sei·L/A_neg`. Inside, that resistance joins the model's own lumped
resistance and no signature moves. Outside, an absolute resistance would have to be threaded
through the eleven model calls in `pack.rs` (nine of them in `Pack::step`) and the
`SourceCache` invariant. Aging stays the
**driver**: the film grows only on the aging sub-clock, through one hook on `CellModel`, so a
pack with `aging: None` grows none. `CellAging`'s reason for living outside the model — a porous
model must not inherit the ECM's bookkeeping — still holds: only the film moves in; calendar's
drawn curve, cycle, plating and reversal stay where they are.

**Slice B was reshaped; it has no enum.** The selector between the drawn calendar curve and
the film must be per pack (slice E ages the same cell both ways), so it belongs in
`AgingConfig`. Snapshots are `bincode`, which is positional: even a one-variant enum there
writes new bytes into every aging pack's snapshot and would cost a bump of its own. It lands in
C beside the film's state, so Phase 10 still bumps once, 25 → 26.

**What B did.** One pair of numbers per cell had two jobs: what the cell model is *handed*
and what the pack *reports*. The film answers them differently — its loss is real lithium gone
from the negative, already inside the model, so multiplying it in again would bill it twice —
so B split them, bit-identically:

- handed to the model: `CellAging::capacity_multiplier` / `resistance_multiplier` (seven reads:
  the two `Cell::eff_*` products; the RC decays in the split and in the advance; `advance`'s
  capacity, which also feeds the plating C-rate and the reversal amp-hours; and in the report,
  `soc_true`'s weight and the resistance ratio's divisor, both listed below);
- reported: `Cell::soh_capacity_reported` / `soh_resistance_reported` (`CellView`, and
  `Telemetry::soh_capacity` through an accumulator of its own);
- calendar fade is marked in `CellAging::tick` as the one mechanism the film replaces.

**Measured bit-identical**, beyond the suite (whose tolerances could hide a last-bit move): a
harness ran aged packs of all five cell models — 2S3P and 1S2P ECM, `Spm`, `Dfn`,
`SpmEnsemble` — through cycling, a voltage hold, a power demand, 200 h hot at rest, a cold
charge and an over-discharge, hashing every telemetry frame, every `CellView` and the final
snapshot; HEAD and B hash identically, and a one-part-in-10¹⁵ perturbation of the reported
capacity changes the hash (`W:/temp/claude/phase10-slice-b/`, not kept).

**Found while sorting, for C to decide** (none is a B defect; each is where the two doors part):

- `soc_true` weights by the capacity the model is handed. Under the film, what it weights by is
  the note's call (the plan's open question on `soc_true`).
- The pack's `Telemetry::soh_resistance` divides each cell's conductance by exactly the
  multiplier the model was handed. A film resistance in series is not divided out, so the model
  must supply its unworn conductance before the ratio can report the film.
- The plating C-rate divides by the handed capacity, which under the film omits the lithium the
  film took. Whether a filmed cell plates at a lower current is C's call.
- The reversal amp-hours use the handed capacity and stay right as they are: they must be what
  the coulomb count divided by.
- With the film off, the model must add **exactly** zero — not the initial 5 nm film's ≈ 0.08 mΩ
  — or every `Spm` golden moves. An `Option` in `SpmState`, `None` unless selected.
