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

Designed 2026-10-15 against `70ffca0`; see the addendum "slice D designed" at the end of this
note. Not started.

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

## Addendum 2026-10-10 (later) — slice C designed, not started; exit 5 split out

No code was written. Designed against the code at `cd08db2`, reviewed once, and recorded here so
the slice can start from it.

**Owner, 2026-10-10: exit criterion 5 gets a slice of its own, right after C.** The engine
reports capacity read off its electrodes, a slow-rate figure; the fit converted lithium into
WMG's capacity with 1.567, a ratio that belongs to WMG's own steps (1.67 A CC to 4.2 V, no hold,
1.67 A down to 2.5 V) and depends on `ρ_sei`. Pinning exit 5 as lithium × 1.567 would re-run the
fit's closed form, which is circular. The honest check runs those steps **in the engine** against
a small committed table of the cells' growth after the first post-storage test, written by
`fit_sei_wmg.py` from the CC-BY data (cite Kuzhiyil et al. 2025 beside it). That is the new
slice; C delivers exits 1–4.

**The design C starts from:**

- **Constants** in `[spm.sei]` (inside `SpmParams`, so only `[spm]` chemistries change bytes),
  every field required, unknown keys rejected, `[spm]` implied. From the last fit run
  (`W:/temp/claude/phase10-spike/fit_repo_run3.txt`): `D_li` 1.4795e-18 m²/s, `E` 42.54 kJ/mol,
  `ρ_sei` 5.594e4 Ω·m; Chen2020 defaults, labelled: 5 nm, `V̄` 9.585e-5 m³/mol, `c_li0`
  15 mol/m³, Li:SEI 2. The reference temperature is `spm.t_ref_k`; check it is the fit's 298.15.
- **Selector**: `AgingConfig` gains a calendar choice (drawn curve, the default, or film),
  `serde(default)` so scenario TOMLs, JSON setups and Godot's `[pack.aging]` still parse. Under
  the film `CellAging::tick` skips `q_cal`; cycle, plating and reversal are unchanged.
  `Pack::new` refuses the film on the ECM, the `Dfn`, the many-particle cell, and a chemistry
  without `[spm.sei]`.
- **State**: `SpmState` gains the film thickness as an `Option`, `None` unless the film is
  selected. Bump 25 → 26, with the stale-blob route of each new field measured in
  `snapshot_version.rs` as v22–v25 were.
- **Tick** (one `CellModel` hook, on the aging sub-clock): freeze the negative's potential and the
  temperature over the tick and grow the film in closed form, written **rationalised**,
  `δL = 2·k·e·dt / (L′ + L)` (`calendar_increment`'s precedent; a difference of square roots
  cancels at 10 s ticks). The potential is `Δφ = U_n(surface) + η_n` at `i_last` — PyBaMM's —
  which at rest is the bulk value the fit used; the golden cannot tell them apart. The lithium
  `Z·δL·A_neg/V̄` leaves every negative shell uniformly, so a resting profile stays flat.
- **Resistance**: `ρ_sei·(L − L0)/A_neg`, growth only (how `ρ` was fitted), added to the lumped
  series resistance at every `Working` built from a state, outside the handed multiplier. So a
  fresh film cell is bit-identical to a film-off one until its first tick, which slice E needs.
  The pack's resistance ratio divides it out with the unworn conductance `mult/(r − r_film)`.
  Per-cell `CellView::soh_resistance` needs a reference resistance; any new `CellView` field is
  a wire change.
- **Capacity and SOC**: today's full is the fresh full moved down by the lithium lost,
  `δx = 3·Z·(L − L0)/(V̄·c_max·R_p)`; empty and the reversal edge stay at `stoich_min`. `soc` reads
  `(x − x_min)/(x_max − δx − x_min)`, so a filmed cell reads 1 at full and `SOC_CLAMPED_HIGH`
  still means the positive passed its limit. Reported `soh_capacity` = multiplier × `(1 − δx/Δx)`;
  `soc_true` weights by it. The plating C-rate keeps the handed capacity (stated, not changed).
- **Energy**: the lithium leaves the cell's chemistry rather than crossing to the positive, so it
  is an explicit sink at the graphite's potential, `F·U_n·δN` — **not** heat at the cell voltage
  (30–40× larger), and not a tick's energy dumped into one step's `q_gen_w`, which would spike
  the client's heat plot every tick. The SEI reaction's own heat is not in the data (order
  1e-5 W) and is stated as such. The existing ledger helpers tie both electrodes to one SOC
  coordinate; a film ledger needs per-electrode stored energy.
- **Film off moves nothing**: every new expression branches on `None` (no `+ 0.0`;
  `mult/(r − 0.0)` and `mult·(1/r)` round differently). Re-run slice B's hash harness in a
  worktree under `W:/temp/claude`, stripping the fields that legitimately change from the final
  snapshot rather than dropping it, and re-check that a 1e-15 perturbation still moves the hash.
- **Golden** (exits 1–4): PyBaMM SPM, interstitial law, fitted constants, a year at rest at 85
  and 30 % and 25 and 45 °C, compared on film thickness or moles. Start PyBaMM at the
  engine's stoichiometry rather than `get_initial_stoichiometries(soc)`. **The tolerance floor
  is the engine's graphite table, not the closed form**: the rate goes as `exp(−F·U_n/RT)`, so
  the table's 1.90 mV interpolation error is ≈ 7–8 % in rate and ≈ 4 % in growth. Measure that
  table-versus-function ratio at the golden's stoichiometries before choosing a tolerance; the
  fit's "within 0.04 % of PyBaMM" used PyBaMM's own OCP function and says nothing about it.

## Addendum 2026-10-15 — slice C done: the film on the `Spm`, v25 → v26

Built from the design above; exits 1–4 pass, pinned by `crates/sim-data/tests/sei_golden.rs`
(PyBaMM) and `crates/sim-data/tests/sei_film.rs` (the engine's own contracts). Exit 5 (the WMG
cells, run on WMG's own capacity steps in the engine) is the next slice, as decided.

**What landed.**

- `[spm.sei]` in `chemistries/nmc_21700_lgm50.toml` (`sim_core::SeiParams`, every key required,
  unknown keys refused, each value with its provenance). Read by nothing unless a pack selects
  the film; a `Dfn` pack on the same file ignores it (the bit-identity run below includes one).
- `AgingConfig::calendar: CalendarModel` (`Curve`, the default, or `Film`), `serde(default)`.
  Under `Film`, `CellAging::tick` adds no drawn calendar fade; cycle, plating and reversal are
  unchanged. `Pack::new` refuses `Film` on `Ecm`, `Dfn` and `SpmEnsemble` (`FilmNeedsSpm`) and
  on a chemistry without `[spm.sei]` (`MissingSeiParams`).
- `SpmState::film_thickness_m: Option<f64>`; `spm::grow_film` grows it on the aging tick in the
  rationalised closed form at `Δφ = U_n(surface) + η_n` at `i_last`, and takes `Z·δL·A_neg/V̄`
  moles uniformly from every negative shell.
- Readouts: the negative's window shortened at the top by `δx` for `soc` and `surface_gap`;
  reported `soh_capacity` = multiplier × `(1 − δx/Δx)`, which `soc_true` now weights by;
  `ρ·(L − L0)/A_neg` added to the lumped resistance of every voltage read; the pack's resistance
  ratio divides with `mult/(r − r_film)`, and `CellView::soh_resistance` is the same ratio per
  cell (equal to the pack's on a single cell, pinned).
- `Pack::cell_film(s, p) -> Option<FilmView>`: thickness, lithium lost, film resistance, and the
  energy the last step's film took. Not a `CellView` field, so no wire change; neither API
  version moved.
- `SNAPSHOT_VERSION` 26. What a v25 blob does at v26, by field (`snapshot_version.rs`): a cell
  state parses quietly when the cell's capacity factor has a zero low byte (an unscattered cell)
  and fails otherwise; an aging config reads the accumulator's low four bytes as the selector,
  so a round accumulator parses as `Curve` and `0.1 s` fails; an `[spm]` section reads the
  `[dfn]` tag after it as its film's, and on the LG M50 takes the electrolyte's next 56 bytes as
  film constants. The version check is what stands in front of all three.

**Measured.**

| | 85 %, 25 °C | 85 %, 45 °C | 30 %, 25 °C | 30 %, 45 °C | 100 %, 25 °C |
| --- | --- | --- | --- | --- | --- |
| table vs function, PyBaMM, after a year | 0.06 % | 0.05 % | 2.03 % | 1.44 % | 0 |
| table vs function, PyBaMM, worst day ≥ 7 (the floor) | 0.08 % | 0.06 % | 2.87 % | 2.55 % | 0 |
| engine vs PyBaMM on the table, worst day ≥ 7 | 0.00002 % | 0.00001 % | 0.0004 % | 0.0016 % | ≈ 0 |
| engine vs PyBaMM on Chen2020's curve, worst day ≥ 7 | 0.08 % | 0.06 % | 2.87 % | 2.55 % | ≈ 0 |

**PyBaMM is solved at 1e-10 for this.** At its default tolerance (1e-6) the reference itself
under-grew the film in the first days — 0.82 % at day 3 at 30 % / 45 °C, mostly gone by day 30 —
and the engine first read 0.47 % off it. It was not the engine: 10-minute and 1-hour engine steps
agreed to 0.001 %, 5 and 20 shells identically, and PyBaMM's own particle surface sat 3e-8 in
stoichiometry from its bulk, too little to matter; at 1e-10 PyBaMM moved onto the engine to about
0.002 % on every day checked. The plan's "≈ 4 % in growth" for the table floor was an estimate;
measured it is at most 2.9 %, and nothing on the 85 % and 100 % plateau. The tolerances are
0.01 % against the table reference and each condition's worst-day floor + 0.01 % against the
function one. Also measured: the log-log slope over days 30–365 at 45 °C / 85 % is 0.559 with the
film at 4.9× its start by day 30 (24.3 nm) and 16.6× by a year (83.0 nm); full-to-30 % after a year
at 25 °C is 6.6546 in both the engine and PyBaMM on the table (3e-6 apart; on the function
6.520); 10 s and 1 h ticks agree to 1.2e-7 over two days; the film's energy
closes the per-electrode ledger to 2.3e-9 over a hot year (48.6 J); ninety days at 45 °C / 85 %
add 0.61 mΩ and a reported resistance ratio of 1.039, bit-identically with
`r_growth_per_capacity_loss` at zero.

**Film off moves nothing**, measured beyond the suite: an out-of-tree harness ran aged packs of
every model — `Ecm` 2S3P with a BMS and 1S2P, `Spm` 1S2P and 2S1P, `Dfn`, `SpmEnsemble` — through
cycling, a voltage hold, a power demand, 200 h hot at rest, a cold charge and an over-discharge,
hashing every telemetry frame, every `CellView` and the final snapshot (as JSON, with the version
and the three new keys stripped). HEAD and slice C hash identically on all six; a 1e-15
perturbation of the initial SOC moves each hash. A fresh film is the curve's cell bit for bit
until its first tick (`a_fresh_film_moves_nothing_until_it_grows`).

**Decisions the design left open, as taken.**

- **The energy sink's sign.** Read per electrode against lithium metal (lithium in the negative
  worth `−U_n` per coulomb), withdrawing lithium into the film *raises* the electrodes' stored
  energy by `U_n·dq`; `FilmView::energy_last_step_j` is that, in the engine's charge units
  (`δn·F/κ`), booked at the bulk potential after the tick. It is not heat and not in `q_gen_w`.
  The SEI reaction's own heat is not in the parameter set and is not modelled.
- **The plating C-rate** keeps dividing by the handed capacity, as the design said.
- **`r_growth_per_capacity_loss`** still couples cycle, plating and reversal loss to the
  multiplier under the film; the film's own loss never enters it.

**Recorded, not fixed.**

- **The reversal channel's voltage on an aged filmed cell.** `empty_voltage` reads the positive at
  `stoich_max`, which a filmed cell no longer reaches when its negative empties (the lost lithium
  leaves the positive short of full lithiation). It does **not** put a step in the terminal
  voltage — measured, a cell aged a hot year at 85 % passes through empty at C/20 as smoothly as a
  fresh one (5.8 against 5.2 mV over the first 10 s past the edge, both the ramp itself), because
  past the edge the curve is the particles' own minus the ramp. What reads it is the reversal
  channel's open-circuit voltage, which books that channel's heat and sets where the ramp stops:
  on the aged cell it is low by `U_p(y) − U_p(stoich_max)`, of the order of the 12.5 mV by which
  that cell reaches empty higher than the fresh one (measured at C/20). Not reached by exits 1–4.
- **`soc_deficit` on a filmed cell** is a fraction of the handed (fresh-window) capacity, while
  `soc` is a fraction of today's: the two are no longer on one scale. Stated, not changed.
- **85 % stores like 100 %.** The graphite potential is flat at about 0.092 V above roughly 70 %
  SOC, so the film grows at the same rate from there to full: PyBaMM's year at 25 °C loses
  0.002699 mol at both, to 0.01 %. That is the law's, not this implementation's; whether WMG's
  cells show the same plateau is not checked here. **Slice E's lesson must not teach "store at
  80 %, not full"; full against 30 % (6.7×) is the contrast this model can show.** Noted by a side
  review of the golden, 2026-10-15.

## Addendum 2026-10-15 (later) — exit 5 started; its pass bar, written before the full run

**What the WMG cells' capacity test was, read off the raw files.** Cells stored at 0 °C log
23.7–29 °C on the cell during their tests (`LogTemp001`, three cells, both test types), so the
tests ran in a ~25 °C chamber whatever the storage temperature; the fit's 25 °C ratio assumed
this without checking it. The capacity steps are as the fit ran them: 1.67 A to 2.5 V, 30 min,
1.67 A to 4.2 V with no hold, 30 min, 1.67 A to 2.5 V, the last discharge being the capacity.

**The check** (`crates/sim-data/tests/sei_wmg.rs`): a filmed `Spm` cell rests at the storage
condition on a live thermal network; at each test day a copy of it settles 3 h at 25 °C and runs
those steps, cut-offs found by bisecting the last step's length. The copy is thrown away, so the
stored cell never cycles — as in the fit's model, where the stored cell's state of charge is
never reset either; the drawn cycle fade is switched off on the test's chemistry so the copy
does not wear during its own test (the data's 0 % subtraction removes that wear from the cells).
Scored exactly as `growth_rms`: per condition the mean square of the cells' growth after the
first post-storage test against the engine's, averaged over conditions, square-rooted. The
reference is `tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv`, data only, written by
`fit_sei_wmg.py --table`; the engine's own 0 % cell is **not** subtracted (the fit did not; a
film on near-empty graphite grows under a thousandth of the high-SOC rate).

**Measured first, one condition**: a year at 85 % / 25 °C costs 1.4847 % of capacity on those
steps for 0.9498 % of lithium (PyBaMM: 0.9505 %), a ratio of **1.5632** against the fit's 1.567,
0.24 % apart. No unconverged solve and no surface out of range in either test.

**The bar, set now.** The constants minimise the fit's closed form's residual, not the engine's,
and the two differ by what separates them measurably: the graphite table (at most 2.9 % of
growth, at 30 % SOC where growth is smallest; 0.08 % on the plateau) and the ratio (0.24 %). On
growths of at most ~3 points either moves a residual by under 0.01 points. The bar is the stated
residual plus 0.02 points: **0.642 % RMS at 25 + 45 °C, 0.425 % on the held-out 0 °C cells**,
asserted separately. Past it is a finding for the owner, not a tolerance to widen.

## Addendum 2026-10-15 (last) — exit 5 passes: the film reproduces the WMG cells in the engine

Run once against the bar above, unchanged after it: **0.6241 % RMS at 25 + 45 °C** over 24
conditions (bar 0.642, the fit's closed form 0.622) and **0.4046 % on the held-out 0 °C cells**
over 12 (bar 0.425, closed form 0.405). That the two scores sit 0.002 apart says little on its
own (the score is coarse, below), so the predictions were compared condition by condition, at
every test day, the closed form at the fitted constants times 1.567 against the engine on WMG's
steps: the largest gap is **0.031 points** (45 °C / 95 %, of a 3.7-point growth), and the engine
is within 2.3 % of the closed form at every condition whose growth exceeds 0.2 points — 1.1 %
low on the 0 °C plateau, 6.4 % low at 0 °C / 30 % where the graphite table costs most. The
conversion through 1.567 did not hide anything the engine does differently. Pinned by
`crates/sim-data/tests/sei_wmg.rs::the_film_reproduces_the_wmg_cells`, with the ratio pinned
beside it (`capacity_lost_per_lithium_lost_on_wmg_steps`, 1.5632 within 0.5 % of 1.567). No
capacity test raised an unconverged solve or a surface out of range. 12 s in a debug build.

**What the check can and cannot see.** The residual is mostly the cells' own scatter, so the bar
is coarse: `D_li` raised 30 % (growth about 14 % faster) reads 0.6493 % at 25 + 45 °C and fails;
a change much smaller than that would pass. The ratio test is not moved by `D_li`, as it should
not be. Perturbed once, restored, green again.

**Where the engine and the cells part, per condition** (last test day, growth in points; recorded,
not scored separately):

- **Mid charge at 45 °C runs ahead of the film**: 30 / 50 / 60 % grew 1.72 / 2.64 / 2.61 in the
  cells against 0.61 / 1.56 / 1.35 in the engine. The same rows at 25 °C are closer (0.57 / 0.84 /
  1.10 against 0.31 / 0.81 / 0.78).
- **The top of the charge range is not flat in the cells.** The engine grows the film at the
  same rate from about 80 % to full (the graphite plateau), so its 85–100 % rows agree to 0.01
  points. The cells do not. At the same test, 25 °C day 483–496: 85 / 90 / 95 % grew 1.41 / 1.38
  / 1.35, 100 % only 0.71 (engine 1.44 at day 496). At 45 °C day 363–367: 85 / 90 % grew 1.96 /
  1.94, 95 / 100 % only 0.84 / 0.98. **Slice E's lesson should claim only what this model shows
  (full against 30 %), and not present "stored full ages fastest" as a fact about these cells**,
  whose fullest rows grew about half as much as their 85–90 % ones after the first test (what
  they did before it, the early step, is not scored here and was not looked at). Not a dropout:
  each of those rows averages 3 cells at 25 °C and 3–6 at 45 °C, the same as its neighbours.
- **0 °C**: the engine is flat at 0.67–0.68 from 80 % up; the cells scatter between 0.33 and
  1.19 with no order in SOC above 70 %.

Exit criteria 1–5 now all pass on the `Spm`. Next is slice D (the film on the `Dfn`), then E
(the lesson).

## Addendum 2026-10-15 — slice D designed: the film on the `Dfn`

No code written. Designed against `70ffca0`, reviewed once. Two forks were the owner's.
**Owner, 2026-10-15: fork 1 is (b), the film's resistance per node inside the reaction; fork 2
is report only.** Both are recorded where they are argued below.

**What is different about the `Dfn`.** It has one particle per negative-electrode node, a
reaction current that varies across the electrode, and a stored Thévenin line (`tangent`) that
the pack reads as the cell's source instead of rebuilding one from state. The `Spm`'s film is
one thickness on one particle, its resistance one lump in series, its source rebuilt by the
reporting pass after every tick (`pack.rs`, the comment above the aging tick). None of those
three carries over as it stands.

**State.** `DfnState::film_thickness_m: Option<Vec<f64>>`, one thickness per negative node
(`c_neg.len()`), `None` unless the pack selects the film. bincode is positional, so the field
costs a bump, **26 → 27**, with its stale-blob route measured in `snapshot_version.rs` as v22–v26
were. `Pack::new` stops refusing `Film` on the `Dfn` (it still needs `[spm.sei]`, which the LG M50
file has); `FilmNeedsSpm` becomes false as named and as worded, so it is renamed (its two uses
are in `sei_film.rs`; it is a public enum variant, so the rename is an API change, stated in the
commit).

**Growth, per node.** On the aging tick each node grows in the `Spm`'s rationalised closed form
at its own `Δφ_i`, read off the stored field `u` — the last converged solve, at `i_last`, so it
costs no solve. Node `i` loses `Z·δL_i·a_s/V̄` moles per unit electrode volume, spread evenly over
its own shells, i.e. `3·Z·δL_i/(V̄·R_p)` in concentration, independent of the node's width.
`bulk_stoich` is an unweighted mean over nodes, which is the volume-weighted mean only because
the negative's grid is uniform (its doc says so); the film's lithium count and `soc` reconcile
on that same assumption, and the note on `bulk_stoich` gains the film as a second reader of it.
At rest every node sits at one potential and the film grows uniformly.

**Fork 1: where the film's resistance goes — decided (b).**

- **(a) One lump in series**, `ρ·(L̄ − L0)/A_neg` on `L̄` the node mean, added to
  `Sides::r_contact` — what the `Spm` does. No change to the solve or its Jacobian. Right while
  the reaction is spread evenly; under load the `Dfn`'s reaction crowds toward the separator, and
  the lump cannot follow it. The rate's `Δφ_i = φ_s − φ_e` then excludes the film's own drop,
  which is the `Spm`'s choice.
- **(b) Per node, inside the Butler–Volmer row**: `η_i = φ_s − φ_e − U − j_i·ρ·(L_i − L0)`, as
  PyBaMM's DFN does. A thicker film at one node pushes the reaction to others, and the rate's
  `Δφ_i` includes the drop, as PyBaMM's does. It changes the analytic Jacobian, which this model
  needs (`phase-7-dfn.md`): `∂η/∂j` gains `−ρ·(L_i − L0)`. So `jacobian_pair`'s
  finite-difference check is extended to a filmed cell with **uneven** thicknesses, and that one
  term is perturbed once to show the check reddens.

Recommended: **(b)**. The `Dfn` exists to show what the electrode's thickness does; a film that
cannot follow the reaction across it is the `Spm`'s film on a slower model. (a) is roughly the
`Spm`'s work again; (b) adds the Jacobian term and its check. Either way the pack still needs one
number for the film's resistance (`mult/(r − r_film)`, `FilmView::resistance_ohm`): under (b) it
is the nodes' films **in parallel**, `ρ/(a_s·A·Σ h_i/(L_i − L0))`, which equals the `Spm`'s
`ρ·(L − L0)/A_neg` when the film is even. The pack's division treats it as series, which is
exact for (a) and, for (b), exact at an even film and approximate otherwise — stated on the
function.

**Whether the rate includes the film's drop, recorded.** Under (b) the `Dfn` reads PyBaMM's
`Δφ`; the `Spm` reads `U_n(surface) + η_n` with the film lumped outside it. Identical at rest
(the golden and exit 5 cannot tell them apart). Under current they part: an 80 nm film at about
1 A/m² of interface drops a few millivolts, and the rate is exponential in it (one thermal voltage
is 25.7 mV). The `Spm` is not reopened; the difference is recorded beside the cycling result.

**The stored line after a tick.** The tick runs before the reporting pass so that the pass
rebuilds every source from the aged state. The `Dfn`'s `source` returns its stored `tangent`
instead, so after a tick the reported line — and a zero-length step, which reads it — still
describes the cell before the film took its lithium and added its resistance. Decided:
**measure first** (the gap a zero-length probe reads after a 1 h tick at 45 °C / 85 %, and after
a 10 s one). If it is below a microvolt it is stated and left; otherwise the tick shifts the
stored line by what it changed (`E` by the equilibrium voltage's move, `R` by the film's added
resistance) rather than clearing it, because clearing it would put every filmed cell on the
first-order seed after every tick and move the next step's first pass.

**Mirrored from the `Spm`.** The window shortened at the top by `δx = mean_i 3·Z·(L_i −
L0)/(V̄·c_max·R_p)` in `raw_soc`, `surface_gap` and `soc_flags`; reported capacity
`multiplier × (1 − δx/Δx)`, which `soc_true` weights by. `current_window` and
`surfaces_in_range` keep the negative's edge at `stoich_max`: under the film the positive reaches
its full first, so the positive's edge binds on a charge, which is the `Spm`'s behaviour (checked
in the build, not assumed). The energy sink is per node, each node's lithium at that node's bulk
`U_n` after the tick, over `κ`, summed into `FilmView::energy_last_step_j`; there is no
per-electrode ledger helper for the `Dfn` (`dfn_reversal.rs`'s ledger test is a terminal one), so
the film's ledger test reads the snapshot's per-node profiles as `sei_film.rs` reads the `Spm`'s.
`empty_voltage`'s reading at `stoich_max` on an aged cell, recorded for the `Spm` in slice C, is
the same here; recorded, not fixed.

**Film off moves nothing.** Every new expression branches on `None`. Slice C's hash harness
(six models, cycling, a voltage hold, a power demand, 200 h hot at rest, a cold charge, an
over-discharge) re-runs in a worktree under `W:/temp/claude`, with the new key stripped from the
final snapshot, and the 1e-15 perturbation re-checked to move each hash.

**Exit for slice D, set before the first run.** Pinned in a new `crates/sim-data/tests/sei_dfn.rs`.

1. **At rest the `Dfn` is the `Spm`.** At rest the electrolyte is uniform and the reaction zero,
   so a filmed `Dfn` and a filmed `Spm` held a year at the golden's five conditions must agree on
   lithium lost, and the `Dfn` must replay `sei_rest_year.csv`'s table column within the `Spm`'s
   own tolerance (0.01 %). The bar for `Dfn` against `Spm` is set from the first day's reading of
   the two **before** the year is run, not after. This runs first; if it fails nothing else
   proceeds. It cannot see fork 1.
2. **Under current, against PyBaMM.** A PyBaMM DFN with the interstitial law, on the engine's
   node counts (10/5/10, `N_r` 10), the fitted constants, cycling at 1 C or below (clear of the
   3 C electrolyte limit, H8), isothermal, drawn cycle fade off — compared on total lithium lost
   and on the film's spread across the negative (thickest node over thinnest). Solved at 1e-10
   (slice C: the default under-grew the film). Under (b) the bar is what the frozen-potential tick
   costs, measured as the engine at 10 s against 60 s ticks on the same run, plus 0.01 %. Under (a)
   the lithium total is held to that bar and the spread is **not** asserted — the lump makes the
   reaction, and so the film, more even than PyBaMM's — and the test records by how much. This is
   the only exit that sees anything the `Dfn` does that the `Spm` does not.
3. **Resistance from the film alone**, as exit 4 on the `Spm`: with `r_growth_per_capacity_loss`
   at zero, ninety days hot add resistance and a reported ratio above 1, both read through
   `FilmView` and `Telemetry::soh_resistance`.
4. **The ledger closes** at rest over a hot year, per node, to the `Spm`'s 1e-6.
5. **Snapshot round trip** of a filmed `Dfn` mid-year, bit-identical continuation.

**Fork 2: the WMG cells on the `Dfn` — decided, report only.** Exit 5 ran WMG's own capacity steps on a filmed
`Spm`. On the `Dfn` the same film gives the same lithium at rest (exit 1), but the capacity the
steps read differs through the electrolyte, so the score will move. The plan's exits do not
require it. Recommended: **report it, not gate on it** — run it once, write both scores and the
per-condition largest gap beside the `Spm`'s, and treat a result past the `Spm`'s bar as a finding
for the owner. The `Spm` run took 12 s in a debug build; a `Dfn` cell costs on the order of a
hundred times more per step, so **one condition is timed in release before the 36 are run**.
