# CLAUDE.md — batsim

Battery pack simulator in Rust. Two purposes, one engine:

1. **Pedagogy** — let users experiment with chemistries, charge/discharge regimes,
   aging, faults, and battery protection, and *see* what happens.
2. **Headless engine** — other software (eventually a Godot game via gdext) can
   step the simulation and query voltage, current, SOC, SOH, temperature, etc.

The engine is the product. Every UI, server, and game is just a client of `sim-core`.

---

## Non-negotiable design principles

1. **`sim-core` is pure.** No file I/O, no network, no threads, no wall-clock reads,
   no global state, no async. It is a deterministic state machine:
   `step(dt, demand, env) -> Telemetry`. All I/O lives in adapter crates.
2. **The pack is the first-class citizen.** A single cell is the degenerate `1S1P`
   pack. There is one public API, not a cell API and a pack API.
3. **Simulation time is decoupled from wall time.** The engine always advances by an
   explicit `dt`. Clients (GUI, Godot, server) use an accumulator to feed it
   wall-clock time. The frame rate must never define the timestep.
4. **Determinism.** Same config + same seed + same demand sequence ⇒ bit-identical
   trajectory on the same binary. All randomness comes from one seeded RNG whose
   state is part of the snapshot.
5. **Everything is snapshotable.** The entire engine state is one
   `#[derive(Serialize, Deserialize)]` value with a schema `version` field.
   Snapshot → restore → continue must reproduce the original trajectory exactly.
6. **Enum dispatch, not trait objects,** for model slots (`CellModel`, aging, etc.).
   Small known set of implementations; enums keep the state serde-friendly.
7. **Components are swappable and toggleable.** Aging, protection/BMS, thermal
   coupling, and faults are `Option`s / config-selected variants. "BMS off" is a
   supported, interesting mode, not an error.
8. **Ground truth ≠ BMS view.** The engine knows every cell's true state. The BMS
   module only consumes simulated *sensor* readings (one voltage per parallel
   group, a few temperature probes, a noisy current sensor) and maintains its own
   SOC *estimate*. The gap between truth and estimate is a feature to expose, not
   a bug to hide.
9. **Start simple, keep the door open.** v1 cell model is an equivalent-circuit
   model (ECM). The `CellModel` enum and per-cell opaque state must allow adding
   `Spm` / `Dfn` (porous electrodes, likely via the `diffsol` crate) later without
   touching the pack layer.
10. **Chemistry is data, not code.** A chemistry is a TOML parameter set. Adding a
    chemistry must never require a code change.

---

## Workspace layout

```
batsim/
├── Cargo.toml                  # workspace
├── CLAUDE.md                   # this file
├── crates/
│   ├── sim-core/               # pure engine: types, models, solver, snapshots
│   ├── sim-data/               # TOML loading/validation -> sim_core::ChemistryParams
│   ├── sim-server/             # axum: REST (setup/snapshots) + WebSocket (stream/commands)
│   ├── sim-wasm/               # wasm-bindgen build of the engine for the browser pedagogy client
│   ├── sim-godot/              # gdext GDExtension: BatteryPack node, signals (Phase 5)
│   └── sim-py/                 # PyO3 bindings, dev/validation only, never shipped (optional)
├── chemistries/                # *.toml parameter sets (LFP, NMC first)
├── tools/reference/            # Python + PyBaMM scripts that GENERATE golden CSVs (not shipped)
└── tests/golden/               # committed reference CSVs + tolerance tests
```

Dependency rule: adapter crates depend on `sim-core` (and `sim-data`); `sim-core`
depends on nothing in this workspace and on no runtime (no tokio, no godot, no pyo3).

---

## Units and conventions

- SI throughout `sim-core`: seconds, amperes, volts, ohms, farads, joules, kelvin.
  Convert to °C / minutes / percent only at adapter boundaries.
- **Sign convention: positive current = discharge** (current flowing out of the
  pack terminals). Charging is negative current. State this in doc comments on
  every current-carrying field.
- SOC ∈ [0, 1], SOH fields ∈ (0, 1] (capacity) and ≥ 1 (resistance growth factor).
- Every public numeric field gets a doc comment with its unit, e.g.
  `/// Terminal voltage \[V\]`.
- Plain `f64` everywhere. Do not introduce the `uom` crate; enforce units by
  naming (`heat_capacity_j_per_k`) and doc comments.

---

## Core API sketch (`sim-core`)

This is the intended shape; refine signatures as needed but keep the semantics.

```rust
/// What the outside world asks of the pack this step.
pub enum Demand {
    /// Positive = discharge [A]
    Current(f64),
    /// Positive = discharge [W]; solved with 1-D Newton on top of the current solve
    Power(f64),
    /// Hold terminal voltage (e.g. CV charge phase) [V]
    Voltage(f64),
    /// Open circuit / rest
    Rest,
}

/// Environment for this step.
pub struct Env {
    /// Ambient temperature [K]
    pub t_ambient: f64,
    /// Optional coolant temperature [K] (None = passive cooling to ambient only)
    pub t_coolant: Option<f64>,
}

pub struct Pack { /* groups: Vec<ParallelGroup>, bms: Option<Bms>, thermal: ThermalNet,
                     faults: FaultState, rng: ChaCha8Rng, sim_time_s: f64, version: u32 */ }

impl Pack {
    pub fn new(config: &PackConfig, chems: &ChemistryRegistry) -> Result<Self, BuildError>;
    /// Advance simulation by dt seconds. Never panics; hard faults are reported in Telemetry.
    pub fn step(&mut self, dt: f64, demand: Demand, env: &Env) -> Telemetry;
    pub fn snapshot(&self) -> Snapshot;             // serde value, versioned
    pub fn restore(s: &Snapshot) -> Result<Self, RestoreError>;
    pub fn cell(&self, series_idx: usize, parallel_idx: usize) -> CellView; // ground truth
}

/// Cheap summary returned every step. Per-cell arrays available on request.
pub struct Telemetry {
    pub v_terminal: f64,           // [V]
    pub i_actual: f64,             // [A] may differ from demand if BMS derates/opens
    pub soc_true: f64,             // ground truth
    pub soc_bms: Option<f64>,      // BMS estimate (None if BMS disabled)
    pub t_min: f64, pub t_max: f64,        // [K]
    pub v_cell_min: f64, pub v_cell_max: f64,
    pub soh_capacity: f64, pub soh_resistance: f64,
    pub flags: EventFlags,         // bitflags: OV, UV, OC, OT, UT, PLATING_RISK,
                                   // BALANCING, CONTACTOR_OPEN, VENTED, THERMAL_RUNAWAY, ...
}

pub enum CellModel { Ecm1Rc(EcmState), Ecm2Rc(EcmState) /* later: Spm(...), Dfn(...) */ }
```

Topology is config: `PackConfig { series: u16, parallel: u16, chemistry: ChemistryId,
scatter: Scatter { capacity_sigma, r0_sigma, seed }, bms: Option<BmsConfig>,
aging: Option<AgingConfig>, thermal: ThermalConfig, ... }`. Config structs are serde
and double as the scenario file format.

---

## Physics spec — v1

### ECM cell (Thevenin, 1–2 RC pairs)

- `V = OCV(soc, T) − I·R0(soc, T) − Σ V_rc,k` (discharge-positive I).
- OCV: monotone lookup table over SOC with linear interpolation; optional
  `dOCV/dT` table for temperature correction and entropic heating. Optional
  simple hysteresis term per chemistry (needed to do NiMH/lead-acid justice later;
  can be stubbed for LFP/NMC v1).
- **RC pairs use the exact exponential update** for piecewise-constant current
  over the step — no numerical integration, unconditionally stable at any dt:
  `V_rc ← V_rc·exp(−dt/τ) + R·I·(1 − exp(−dt/τ))`, with `τ = R·C`.
  This is what lets the same code path serve real-time GUI stepping and
  months-long aging fast-forward.
- SOC by coulomb counting: `soc ← soc − I·dt / (3600·capacity_ah·soh_capacity)`,
  clamped to [0, 1] with a flag when clamping occurs (over/under-charge attempt).
- Health applies as multipliers: effective capacity = nominal × `soh_capacity`;
  effective R0 and RC resistances = nominal × `soh_resistance`.

### Pack electrical solve (closed form — no iterative solver in v1)

- Over one step, each cell is a Thevenin source `E_k = OCV_k − Σ V_rc,k` behind
  `R_k = R0_k`.
- **Parallel group** carrying group current `I_g` (discharge-positive):
  node voltage `V = (Σ E_k/R_k − I_g) / (Σ 1/R_k)`, then per-cell
  `I_k = (E_k − V)/R_k`. Currents naturally split by state — a low-resistance or
  high-SOC cell takes more load. This is where imbalance physics emerges; do not
  shortcut it by averaging cells.
- **Series**: identical current through every group; terminal voltage = Σ group V.
- `Demand::Power(P)`: Newton-iterate on pack current with `P = V(I)·I`
  (converges in a few iterations; guard with bisection fallback and iteration cap).
- `Demand::Voltage(V)`: solve for I from the same linear Thevenin aggregate
  (closed form). Used for CV charging; combined CC-CV is a client-side policy.

### Thermal network

- One lumped node per cell: `C_th·dT/dt = Q_gen + Σ_j k_ij·(T_j − T_i)
  + h·A·(T_env − T_i)` where `T_env` is ambient or coolant.
- `Q_gen = I²·(R0 + Σ R_rc)` plus optional entropic term `I·T·dOCV/dT`.
- Neighbor conductances `k_ij` from a simple grid adjacency derived from
  topology (configurable); this is what makes center cells run hot and enables
  runaway propagation. Explicit Euler is fine (thermal time constants are long);
  sub-step if `dt` exceeds a stability bound computed from `C_th` and total
  conductance.

### Aging (semi-empirical; runs on a coarse sub-clock, e.g. every 10 s of sim time)

- **Calendar fade**: `dQ_cal ∝ k_cal(T, soc)·d(√t)` with Arrhenius temperature
  dependence `k ∝ exp(−Ea/(R_gas·T))` and a SOC stress factor (worse at high SOC).
- **Cycle fade**: proportional to charge throughput, weighted by DOD and C-rate
  stress factors (rainflow counting is overkill for v1; throughput + stress
  weights is enough and is transparent to students).
- Both mechanisms reduce `soh_capacity` **and** increase `soh_resistance`
  (roughly: each % capacity lost adds a configurable % resistance). Resistance
  growth is pedagogically important — do not model capacity fade alone.
- All aging coefficients live in the chemistry TOML with provenance comments.

### Protection / BMS (toggleable; sensor-limited)

- **Sensors** (the only inputs the BMS sees): one voltage per parallel group,
  `n` temperature probes mapped to configured cell positions, one pack current
  sensor with configurable offset/noise. Sensor faults are injected here.
- **SOC estimator**: coulomb counting on the (imperfect) current sensor, with
  drift; OCV-based correction only when the pack has rested long enough for a
  valid OCV read. On LFP the flat curve makes correction weak mid-range — this
  is intended and should be visible.
- **Protection**: over/under-voltage per group, over-current (separate charge and
  discharge limits), over/under-temperature, charge inhibit below `t_charge_min`.
  Graduated response: derate (clamp demand) → open contactor. All thresholds from
  chemistry TOML; all trips raise flags/events.
- **Balancing**: passive bleed resistor per group above a voltage threshold near
  end of charge. Enough to demonstrate why balancing exists.
- With `bms: None`, demands pass through unclamped and the emergent-failure paths
  below become reachable. That contrast is a core teaching scenario.

### Faults

- **Injectable** (timestamped queue in config or via API):
  `SoftInternalShort { cell, ohms }` (parallel leakage resistance draining the
  cell and self-heating it), `ExternalShort { ohms }`,
  `SensorStuck / SensorOffset { sensor, value }`, `WeakCell { cell, capacity_factor,
  r0_factor }` (deterministic scatter outlier).
- **Emergent** (from physics, never scripted):
  - *Lithium plating risk*: charging below `t_plating_min` at C-rate above a
    threshold sets a flag and applies accelerated fade + soft-short probability
    (drawn from the seeded RNG).
  - *Thermal runaway*: above `T_onset`, add an exothermic self-heating term
    (Arrhenius-shaped heat release with a finite per-cell energy budget); venting
    flag at `T_vent`; propagation happens through the thermal network to
    neighbors. Overcharge with BMS off must be able to reach this state.

---

## Chemistry parameter files (`chemistries/*.toml`)

One file per chemistry. `sim-data` parses and validates (monotone OCV table,
positive resistances, limits ordered, etc.) into `sim_core::ChemistryParams`.
Ship LFP and NMC first; lead-acid (Peukert) and NiMH (−ΔV, hysteresis) later.

```toml
[meta]
id          = "lfp_26650_generic"
name        = "Generic LFP 26650"
provenance  = "Fitted to PyBaMM Prada2013 outputs; see tools/reference/fit_lfp.py"

[cell]
capacity_ah      = 2.5
v_max            = 3.65
v_min            = 2.0
max_charge_c     = 1.0
max_discharge_c  = 3.0
t_charge_min_k   = 273.15   # charge inhibit below 0 degC
t_max_k          = 333.15

[ocv]                        # monotone in soc; linear interpolation
soc   = [0.00, 0.02, 0.05, 0.10, 0.20, 0.40, 0.60, 0.80, 0.90, 0.98, 1.00]
volts = [2.00, 2.90, 3.15, 3.20, 3.26, 3.29, 3.31, 3.33, 3.34, 3.42, 3.60]
# optional: docv_dt_v_per_k table for entropic heating

[r0]                         # ohms, grid over soc x temp_k
soc    = [0.0, 0.5, 1.0]
temp_k = [263.15, 298.15, 318.15]
ohms   = [[0.055, 0.022, 0.018],
          [0.048, 0.020, 0.016],
          [0.050, 0.021, 0.017]]

[[rc]]                       # 1..2 pairs
r_ohms  = 0.010
c_farad = 2000.0

[thermal]
heat_capacity_j_per_k = 95.0
h_area_w_per_k        = 0.35

[aging]
cal_pre_exp = 1.0e4          # provenance comment required for every value
cal_ea_j_per_mol = 5.0e4
cal_soc_stress = [1.0, 1.0, 1.4]     # at soc = 0.0 / 0.5 / 1.0
cyc_fade_per_ah = 2.0e-5
cyc_dod_stress_exp = 1.1
r_growth_per_capacity_loss = 1.5

[safety]
t_onset_k = 423.15
t_vent_k  = 453.15
runaway_energy_j = 60.0e3
t_plating_min_k = 273.15
plating_c_threshold = 0.5
```

Rule: **never invent a physical constant silently.** Every number gets a
`provenance` note (paper, PyBaMM parameter set, datasheet, or "placeholder —
order-of-magnitude only, TODO fit"). Placeholders are acceptable; unlabeled
numbers are not.

---

## Determinism rules (enforced, not aspirational)

- No `HashMap`/`HashSet` anywhere in simulation state or per-step iteration
  (nondeterministic order). Use `Vec` indexed by topology, or `BTreeMap`.
- One RNG: `rand_chacha::ChaCha8Rng` (serde feature on), seeded from config,
  serialized inside the snapshot. Never `thread_rng()`.
- No reading of wall-clock time, environment variables, or files in `sim-core`.
- No fast-math flags; plain IEEE `f64` ops. Same-binary determinism is
  guaranteed; cross-platform bit-exactness is *not* promised (libm differences)
  — do not claim it in docs.
- Regression test: run a scenario, snapshot at t/2, restore, continue; the two
  telemetry streams must be bit-identical.

---

## Testing strategy

1. **Analytic goldens** (Phase 0): constant-current discharge of a 1RC ECM has a
   closed-form solution; assert the engine matches to ~1e-9.
2. **PyBaMM goldens**: `tools/reference/` (Python, requires `pybamm`, never in the
   Rust build or CI path) generates CSVs — CC discharge at several C-rates and
   temperatures, pulse/relaxation (GITT-like), a drive-cycle profile — committed
   under `tests/golden/`. Rust integration tests compare within stated tolerances
   (ECM vs DFN reference won't match exactly; tolerances are per-scenario and
   documented in the test).
3. **Property tests** (proptest): charge conservation (∫I·dt matches ΔSOC·capacity);
   SOC stays in [0,1]; terminal voltage ≤ OCV during discharge and ≥ OCV during
   charge; parallel-group currents sum to the group current; pack energy balance
   (electrical energy out + heat = stored energy change within tolerance);
   snapshot round-trip equality.
4. **Scenario tests**: named TOML scenarios under `tests/scenarios/` (e.g.
   "overcharge with BMS off reaches runaway", "weak cell caps pack capacity",
   "LFP SOC estimate drifts mid-range") asserting on flags and key outcomes.
5. Benchmarks (criterion) for `Pack::step` at 100S10P once Phase 1 lands; keep a
   budget (< 50 µs per step at that size on a laptop; it should be far below).

---

## Phased build plan

Work strictly in phases; each has an exit criterion. Do not start a phase before
the previous one's tests pass.

- **Phase 0 — skeleton + single cell.** Workspace, `sim-core` types (`Demand`,
  `Env`, `Telemetry`, config structs), `sim-data` TOML loading, 1RC/2RC ECM cell,
  exact RC update, coulomb counting, CC/CV via `Demand`. *Exit:* analytic golden
  passes; LFP + NMC TOMLs load and validate.
- **Phase 1 — packs + determinism.** Series/parallel topology, closed-form group
  solve, seeded scatter, snapshots, replay + property tests, PyBaMM golden
  pipeline and first committed CSVs. *Exit:* weak-cell scenario shows pack
  capacity capped by weakest series element; snapshot replay bit-identical.
- **Phase 2 — thermal + BMS.** Thermal network, heat generation, sensor layer,
  SOC estimator, protection with derate/contactor, passive balancing.
  *Exit:* center cells run measurably hotter; LFP estimator-drift scenario passes;
  protection scenarios pass with BMS on, and the same demands violate limits with
  BMS off.
- **Phase 3 — aging + faults.** Calendar + cycle fade on sub-clock, resistance
  growth, fault queue, plating flag, runaway + propagation. *Exit:* fast-forward
  of 500 cycles shows plausible fade curve; "BMS off overcharge → runaway →
  neighbor propagation" scenario passes.
- **Phase 4 — headless server + browser demo.** `sim-server` (axum): REST for
  scenario setup and snapshots, WebSocket for telemetry stream + demand commands;
  `sim-wasm` build with a minimal HTML page plotting live curves (pedagogy MVP).
  *Exit:* an external script can run a full experiment over WebSocket.
- **Phase 5 — Godot adapter.** `sim-godot` (gdext): `BatteryPack` node, exported
  chemistry/topology properties, fixed-dt accumulator in `_physics_process`,
  signals (`protection_tripped`, `thermal_runaway_started`, `soc_changed`, …).
- **Phase 6 (future) — porous electrodes.** Add `Spm`/`Dfn` variants to
  `CellModel`, evaluate `diffsol` for the stiff DAE solve, validate against
  PyBaMM directly. Nothing in earlier phases may assume ECM-only internals
  outside the `CellModel` enum.

---

## Coding conventions

- Rust stable, 2021+ edition. `cargo fmt` and `cargo clippy --workspace
  --all-targets -- -D warnings` must be clean before any commit.
- `#![forbid(unsafe_code)]` in `sim-core`.
- No panics on the public `sim-core` API path: constructors return `Result`
  (`thiserror`); `step()` reports problems via flags/events, it does not panic
  or return `Err` for physical events. Binaries may use `anyhow`.
- Doc comments on all public items; units in every numeric doc comment.
- Keep `sim-core` dependencies minimal: `serde`, `rand_chacha` (+`rand_core`),
  `bitflags`, `thiserror`. TOML parsing lives in `sim-data`, not core.
- Prefer small pure functions for physics steps so property tests can hit them
  directly.

## Common commands

```bash
cargo test --workspace                 # everything incl. golden + property tests
cargo test -p sim-core                 # fast inner loop
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
python tools/reference/generate.py     # regenerate golden CSVs (needs pybamm; commit results)
```

## Never do

- Never add async, tokio, godot, or pyo3 types/deps to `sim-core`.
- Never let a client's frame rate or message rate define the physics `dt`.
- Never use `thread_rng()` or any RNG outside the snapshotted seeded one.
- Never use `HashMap` in sim state or per-step iteration order.
- Never model capacity fade without the matching resistance growth.
- Never let the BMS read ground-truth state directly — sensors only.
- Never script an "emergent" failure as an animation/state override; it must fall
  out of the physics (fault *injection* is the only sanctioned override).
- Never change snapshot layout without bumping `version` and adding a migration
  or an explicit incompatibility error.
- Never commit an unlabeled physical constant (see provenance rule).
