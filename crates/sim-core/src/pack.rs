//! The battery pack — the one public simulation entry point.
//!
//! Per the design contract, the pack is the first-class citizen and a single cell
//! is the degenerate `1S1P` pack; there is no separate cell API. Phase 1
//! implements general series/parallel topology behind this same API, with the
//! closed-form group solve where imbalance physics emerge.
//!
//! # Electrical solve (closed form, no iteration)
//! Over one step every cell is a fixed linear Thévenin source `E_k = OCV − Σ V_rc`
//! behind `R_k = R0·r0_factor` (evaluated from start-of-step state). A **parallel
//! group** carrying group current `I_g` has one shared node voltage
//! `V = (Σ E_k/R_k − I_g)/(Σ 1/R_k)`, and each cell then carries
//! `I_k = (E_k − V)/R_k` — so a low-resistance or high-SOC cell naturally takes
//! more load, and mismatched cells at rest circulate current. Each parallel group
//! aggregates to its own Thévenin `(E_g, R_g)`; **series** groups share one current
//! and their node voltages sum, so the whole pack aggregates to one Thévenin
//! `(E_pack, R_pack)` against which the demand is solved in closed form (see
//! [`solve_current`]).

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::bms::{Bms, BmsConfig};
use crate::chem::ChemistryParams;
use crate::ecm::{
    advance_cell, cell_heat_w, cell_source, docv_dt_lookup, solve_current, CellModel, EcmState,
};
use crate::flags::EventFlags;
use crate::noise::standard_normal_pair;
use crate::thermal::{advance_temperatures, ThermalConfig, ThermalStep};
use crate::{Demand, Env, Telemetry};

/// Current snapshot schema version. Bumped whenever [`Pack`]'s serialized layout
/// changes (see `CLAUDE.md`).
///
/// v2 (Phase 1): the single `cell: CellModel` became `groups: Vec<ParallelGroup>`
/// of per-cell `Cell`s, and config gained `scatter`. No migration ships because
/// Phase 0 had no `snapshot()` method, so no v1 snapshots can exist. Note that
/// under a self-describing-by-order format like bincode a *structural* change is
/// caught at deserialization before [`Pack::restore`]'s version check runs; the
/// version field's job is to guard future *semantic* changes to an unchanged
/// layout.
///
/// v3 (Phase 2, thermal): `Pack` gained `thermal: ThermalConfig` and
/// `ChemistryParams` gained `thermal: ThermalParams` plus the optional
/// `ocv.docv_dt_v_per_k` column, all of which sit inside the snapshot. Cell
/// temperature was already part of `EcmState`, so no *per-cell* layout changed.
///
/// Note what actually rejects a v2 snapshot here: the layout change, at
/// deserialization, exactly as the v2 note above describes. The version check never
/// sees those bytes. The bump is still correct — adapters gate on `Snapshot::version`,
/// and a tolerant format (or a future migration path) would otherwise accept the blob
/// and silently produce an isothermal pack — but this bump is not what makes v2
/// unloadable, and no test could pin it as such.
///
/// v4 (Phase 2, BMS): `Pack` gained `bms: Option<Bms>`, which carries the SOC
/// estimate, the rest timer, and the last [`crate::bms::SensorFrame`]. The frame is
/// serialized rather than recomputed on restore, and has to be: the loaded group
/// voltages in it depend on a current that is not stored, and its noise draw has
/// already advanced the RNG. Same caveat as v3 about what actually rejects an older
/// blob.
pub const SNAPSHOT_VERSION: u32 = 4;

/// Per-cell manufacturing scatter: independent Gaussian variation of capacity and
/// ohmic resistance across the cells of a pack.
///
/// Sigmas are **relative** (a fraction of the nominal value: `0.02` = 2 % 1σ).
/// `0` means no scatter. Draws come from the single pack RNG (seeded by
/// [`PackConfig::seed`]) at construction — honouring the "one RNG" rule, this type
/// carries no seed of its own (a deliberate refinement of the `CLAUDE.md` API
/// sketch, which nested the seed here).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scatter {
    /// Relative 1σ of per-cell capacity, as a fraction of nominal. `0` = uniform.
    pub capacity_sigma: f64,
    /// Relative 1σ of per-cell `R0`, as a fraction of nominal. `0` = uniform.
    pub r0_sigma: f64,
}

impl Default for Scatter {
    fn default() -> Self {
        Self {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        }
    }
}

/// Pack topology and initial conditions.
///
/// This doubles as (part of) the scenario file format. Aging configuration is added
/// in a later phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackConfig {
    /// Number of series elements (groups). Must be ≥ 1.
    pub series: u16,
    /// Number of parallel cells per group. Must be ≥ 1.
    pub parallel: u16,
    /// Initial state of charge for every cell, in \[0, 1\].
    pub initial_soc: f64,
    /// Initial temperature for every cell \[K\].
    pub initial_temp_k: f64,
    /// Seed for the single simulation RNG (part of the snapshot).
    pub seed: u64,
    /// Per-cell manufacturing scatter (defaults to none).
    #[serde(default)]
    pub scatter: Scatter,
    /// Thermal coupling.
    ///
    /// **Omitting this from a scenario file yields an isothermal pack** — not a
    /// thermally-coupled one with default parameters. Silence means the thermal
    /// model is *off*. That default is deliberate (it keeps the goldens testing the
    /// electrical model, see [`ThermalConfig`]), but it does mean a scenario author
    /// who forgets the section gets no diagnostic.
    #[serde(default)]
    pub thermal: ThermalConfig,
    /// Battery management system, or `None` for no BMS at all.
    ///
    /// `None` is a supported and interesting mode, not a broken pack: demands pass
    /// through unclamped, [`Telemetry::soc_bms`] is `None`, and the failure paths a
    /// real BMS exists to prevent become reachable. Contrasting the two is a core
    /// teaching scenario.
    #[serde(default)]
    pub bms: Option<BmsConfig>,
}

/// Reasons [`Pack::new`] can fail.
#[derive(Debug, Error, PartialEq)]
pub enum BuildError {
    /// `series` or `parallel` was zero; a pack needs at least one cell.
    #[error("topology {series}S{parallel}P is invalid: series and parallel must both be ≥ 1")]
    ZeroTopology {
        /// Requested series count.
        series: u16,
        /// Requested parallel count.
        parallel: u16,
    },
    /// `initial_soc` was outside \[0, 1\].
    #[error("initial_soc must be in [0, 1], got {0}")]
    BadInitialSoc(f64),
    /// `initial_temp_k` was not positive.
    #[error("initial_temp_k must be > 0, got {0}")]
    BadInitialTemp(f64),
    /// A [`ThermalConfig::Network`] conductance was negative or not finite.
    #[error("thermal.k_neighbor_w_per_k must be finite and >= 0, got {0}")]
    BadThermalConductance(f64),
    /// A [`BmsConfig`] field was outside its allowed range.
    #[error("bms.{field} is invalid: {reason} (got {value})")]
    BadBmsConfig {
        /// Offending field name.
        field: &'static str,
        /// What was required.
        reason: &'static str,
        /// Offending value.
        value: f64,
    },
    /// A [`BmsConfig::temp_probes`] entry named a cell outside the topology.
    #[error("bms.temp_probes[{index}] = {s}S{p}P is outside a {series}S{parallel}P pack")]
    BadTempProbe {
        /// Index into `temp_probes`.
        index: usize,
        /// Requested series index.
        s: u16,
        /// Requested parallel index.
        p: u16,
        /// Pack series count.
        series: u16,
        /// Pack parallel count.
        parallel: u16,
    },
    /// The chemistry itself failed validation.
    #[error("invalid chemistry: {0}")]
    Chemistry(#[from] crate::chem::ChemistryError),
}

/// Reasons [`Pack::restore`] can reject a [`Snapshot`].
#[derive(Debug, Error, PartialEq)]
pub enum RestoreError {
    /// The snapshot's schema version is not the one this build understands.
    #[error("snapshot schema version {found} is unsupported (this build expects {expected})")]
    VersionMismatch {
        /// Version recorded in the snapshot.
        found: u32,
        /// Version this build produces/consumes.
        expected: u32,
    },
}

/// A serializable capture of the entire engine state.
///
/// Per the design contract the whole engine is one serde value with a schema
/// `version`; this newtype is that value plus a top-level version tag so an adapter
/// can gate on it. Round-tripping a `Snapshot` through any serde format and calling
/// [`Pack::restore`] reproduces the original trajectory exactly (see the replay
/// test). The inner state is private; construct one via [`Pack::snapshot`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Schema version of the captured state (mirrors the pack's own `version`).
    pub version: u32,
    /// The full ground-truth pack state.
    pack: Pack,
}

/// A `(series, parallel)` cell index was out of range for the pack's topology.
#[derive(Debug, Error, PartialEq)]
#[error("cell index {s}S{p}P is out of range for a {series}S{parallel}P pack")]
pub struct CellIndexError {
    /// Requested series index.
    pub s: usize,
    /// Requested parallel index.
    pub p: usize,
    /// Pack series count.
    pub series: u16,
    /// Pack parallel count.
    pub parallel: u16,
}

/// One physical cell's ground-truth state plus its static manufacturing factors.
///
/// The dynamic ECM state lives in [`CellModel`]/[`EcmState`]; the two factors are
/// fixed at construction (from [`Scatter`] or an explicit weak-cell override) and
/// scale the cell's effective capacity and resistance. Aging's `soh_*` multipliers
/// compose on top of these in a later phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Cell {
    /// Dynamic equivalent-circuit state.
    model: CellModel,
    /// Static capacity multiplier: effective capacity = nominal × this. `> 0`.
    capacity_factor: f64,
    /// Static `R0` multiplier: effective `R0` = nominal × this. `> 0`.
    r0_factor: f64,
}

/// A parallel group: the cells wired in parallel that share one terminal node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ParallelGroup {
    /// The `parallel` cells in this group.
    cells: Vec<Cell>,
}

/// A read-only view of one cell's ground-truth state, returned by [`Pack::cell`].
///
/// This is the engine's *true* per-cell state — distinct from anything the BMS can
/// sense (the BMS sees group-level sensors only, from Phase 2 on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellView {
    /// Ground-truth state of charge, in \[0, 1\].
    pub soc: f64,
    /// Cell temperature \[K\].
    pub temp_k: f64,
    /// Sum of the cell's RC-pair overpotentials \[V\], discharge-positive.
    pub v_rc_sum: f64,
    /// Static capacity multiplier applied to this cell.
    pub capacity_factor: f64,
    /// Static `R0` multiplier applied to this cell.
    pub r0_factor: f64,
}

/// A battery pack: the full, ground-truth simulation state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    /// Snapshot schema version.
    version: u32,
    /// Cell chemistry (config; identical for every cell).
    chem: ChemistryParams,
    /// Series element count.
    series: u16,
    /// Parallel cell count per group.
    parallel: u16,
    /// Ground-truth topology: `series` groups, each of `parallel` cells.
    groups: Vec<ParallelGroup>,
    /// Thermal coupling (config; see [`ThermalConfig`]).
    thermal: ThermalConfig,
    /// The battery management system, if this pack has one. Holds the BMS's own
    /// beliefs (SOC estimate, rest timer, last sensor frame) — never ground truth.
    bms: Option<Bms>,
    /// The single seeded RNG; its state is part of the snapshot.
    rng: ChaCha8Rng,
    /// Simulation time elapsed \[s\].
    sim_time_s: f64,
}

impl Pack {
    /// Build a pack from config and a (validated-on-the-way) chemistry.
    ///
    /// # Errors
    /// Returns [`BuildError`] for a zero-sized topology, out-of-range initial
    /// conditions, or an invalid chemistry.
    pub fn new(config: &PackConfig, chem: ChemistryParams) -> Result<Self, BuildError> {
        chem.validate()?;
        if config.series == 0 || config.parallel == 0 {
            return Err(BuildError::ZeroTopology {
                series: config.series,
                parallel: config.parallel,
            });
        }
        if !(0.0..=1.0).contains(&config.initial_soc) {
            return Err(BuildError::BadInitialSoc(config.initial_soc));
        }
        let temp_positive = config.initial_temp_k > 0.0;
        if !temp_positive {
            return Err(BuildError::BadInitialTemp(config.initial_temp_k));
        }
        if let ThermalConfig::Network { k_neighbor_w_per_k } = config.thermal {
            let k_ok = k_neighbor_w_per_k >= 0.0 && k_neighbor_w_per_k.is_finite();
            if !k_ok {
                return Err(BuildError::BadThermalConductance(k_neighbor_w_per_k));
            }
        }
        if let Some(bms) = &config.bms {
            validate_bms(bms, config.series, config.parallel)?;
        }

        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
        let n_rc = chem.n_rc();
        let mut groups = Vec::with_capacity(config.series as usize);
        // Series-major, parallel-minor construction so the scatter draw order (and
        // thus the whole trajectory) is a fixed function of the seed and topology.
        for _ in 0..config.series {
            let mut cells = Vec::with_capacity(config.parallel as usize);
            for _ in 0..config.parallel {
                let (capacity_factor, r0_factor) = draw_factors(&mut rng, &config.scatter);
                let state = EcmState {
                    soc: config.initial_soc,
                    v_rc: vec![0.0; n_rc],
                    temp_k: config.initial_temp_k,
                };
                // validate() guarantees 1 or 2 RC pairs.
                let model = if n_rc == 1 {
                    CellModel::Ecm1Rc(state)
                } else {
                    CellModel::Ecm2Rc(state)
                };
                cells.push(Cell {
                    model,
                    capacity_factor,
                    r0_factor,
                });
            }
            groups.push(ParallelGroup { cells });
        }

        // Built after the scatter draws so that adding a BMS cannot shift the
        // per-cell factors a seed produces; `Bms::new` itself draws nothing.
        let bms = config.bms.clone().map(|bms_config| {
            Bms::new(
                bms_config,
                &chem,
                config.series,
                config.initial_soc,
                config.initial_temp_k,
            )
        });

        Ok(Self {
            version: SNAPSHOT_VERSION,
            chem,
            series: config.series,
            parallel: config.parallel,
            groups,
            thermal: config.thermal,
            bms,
            rng,
            sim_time_s: 0.0,
        })
    }

    /// Simulation time elapsed so far \[s\].
    #[must_use]
    pub fn sim_time_s(&self) -> f64 {
        self.sim_time_s
    }

    /// Capture the entire engine state as a versioned, serializable [`Snapshot`].
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: self.version,
            pack: self.clone(),
        }
    }

    /// Rebuild a pack from a [`Snapshot`]. Continuing from the result reproduces the
    /// original trajectory exactly.
    ///
    /// # Errors
    /// Returns [`RestoreError::VersionMismatch`] if the snapshot's schema version is
    /// not the one this build understands.
    pub fn restore(snapshot: &Snapshot) -> Result<Self, RestoreError> {
        if snapshot.version != SNAPSHOT_VERSION {
            return Err(RestoreError::VersionMismatch {
                found: snapshot.version,
                expected: SNAPSHOT_VERSION,
            });
        }
        Ok(snapshot.pack.clone())
    }

    /// Ground-truth view of the cell at series position `s`, parallel position `p`
    /// (both zero-based). Returns `None` if either index is out of range.
    #[must_use]
    pub fn cell(&self, s: usize, p: usize) -> Option<CellView> {
        let cell = self.groups.get(s)?.cells.get(p)?;
        let state = cell.model.state();
        Some(CellView {
            soc: state.soc,
            temp_k: state.temp_k,
            v_rc_sum: state.v_rc.iter().sum(),
            capacity_factor: cell.capacity_factor,
            r0_factor: cell.r0_factor,
        })
    }

    /// Override one cell's static manufacturing factors (capacity and `R0`
    /// multipliers).
    ///
    /// This is the deterministic "weak cell" / scatter-outlier seam — the same
    /// application point the Phase 3 `WeakCell` fault will use. Factors are clamped
    /// to a positive floor to preserve the group solve's invariants. `s`/`p` are
    /// zero-based series/parallel indices.
    ///
    /// # Errors
    /// Returns [`CellIndexError`] if `(s, p)` is out of range for the topology.
    pub fn set_cell_factors(
        &mut self,
        s: usize,
        p: usize,
        capacity_factor: f64,
        r0_factor: f64,
    ) -> Result<(), CellIndexError> {
        let (series, parallel) = (self.series, self.parallel);
        let cell = self
            .groups
            .get_mut(s)
            .and_then(|g| g.cells.get_mut(p))
            .ok_or(CellIndexError {
                s,
                p,
                series,
                parallel,
            })?;
        cell.capacity_factor = capacity_factor.max(MIN_FACTOR);
        cell.r0_factor = r0_factor.max(MIN_FACTOR);
        Ok(())
    }

    /// Advance the simulation by `dt` seconds under `demand`. Never panics.
    ///
    /// Ordering within a step: the electrical solve runs off **start-of-step**
    /// state, each cell's internal state is then advanced with the current it was
    /// assigned, temperatures are integrated against the heat that current
    /// generated (see [`crate::thermal`]), and all telemetry is reported from
    /// **end-of-step** state. `env` supplies the thermal sink — coolant if present,
    /// otherwise ambient — and is unused when the pack is
    /// [`ThermalConfig::Isothermal`].
    ///
    /// # Upper limit on `dt` with a live thermal network
    /// Temperatures are integrated with automatic sub-stepping, so any ordinary `dt`
    /// is safe. There is a bound, though: the sub-step count is capped, and above
    /// roughly **1.7 hours** of `dt` for the shipped chemistries the cap binds and
    /// the temperatures stop being trustworthy (they are not flagged — a
    /// `debug_assert` fires in debug builds). Nothing else in the step has such a
    /// limit; the electrical solve and the RC update are exact at any `dt`. Coarse
    /// fast-forward will need a different thermal integrator, not a bigger cap.
    pub fn step(&mut self, dt: f64, demand: Demand, env: &Env) -> Telemetry {
        let cap_ah = self.chem.cell.capacity_ah;
        let (series, parallel) = (self.series as usize, self.parallel as usize);

        // A zero-length step is an observation, not a tick: it must leave the engine
        // exactly as it found it (pinned by `snapshot.rs`, and relied on by the
        // energy-balance property test to read the start-of-step terminal voltage).
        // Every physics update scales by `dt` and so is self-guarding, but the BMS is
        // not — consuming a frame resets its rest timer and can fire an OCV
        // correction, and sampling a new one advances the RNG. So the sensor clock
        // only ticks when time actually passes.
        let sensor_tick = dt > 0.0;

        // --- the BMS acts first, on the frame sampled at the end of the previous
        // step. It is one step behind on purpose (see `crate::bms`), and it never
        // sees any of the ground truth computed below.
        if sensor_tick {
            if let Some(bms) = &mut self.bms {
                let pack_nominal_ah = cap_ah * f64::from(self.parallel);
                bms.update_estimate(&self.chem, dt, pack_nominal_ah);
            }
        }

        // --- start-of-step: per-cell and per-group Thévenin, then pack aggregate.
        // group_src[g] = (E_g, R_g); cell_src[g][k] = (E_k, R_k).
        let mut group_src: Vec<(f64, f64)> = Vec::with_capacity(self.groups.len());
        let mut cell_src: Vec<Vec<(f64, f64)>> = Vec::with_capacity(self.groups.len());
        for group in &self.groups {
            let mut srcs = Vec::with_capacity(group.cells.len());
            let mut sum_g = 0.0; // Σ 1/R_k  (conductance)
            let mut sum_eg = 0.0; // Σ E_k/R_k
            for cell in &group.cells {
                let (e, r) = cell_source(cell.model.state(), &self.chem, cell.r0_factor);
                let g = 1.0 / r;
                sum_g += g;
                sum_eg += e * g;
                srcs.push((e, r));
            }
            group_src.push((sum_eg / sum_g, 1.0 / sum_g));
            cell_src.push(srcs);
        }
        // Series aggregate: same current through every group; voltages add.
        let e_pack: f64 = group_src.iter().map(|&(e, _)| e).sum();
        let r_pack: f64 = group_src.iter().map(|&(_, r)| r).sum();

        // --- solve the single pack current (shared by every series group).
        let i_g = solve_current(demand, e_pack, r_pack);

        // --- split into per-cell currents, tally heat, and advance each cell.
        // Heat is tallied in every mode (an isothermal pack still reports how much
        // heat it makes); the per-cell array is only materialised when a live
        // thermal network is going to consume it.
        let thermal_live = matches!(self.thermal, ThermalConfig::Network { .. });
        let n_cells = series * parallel;
        let mut heat_w: Vec<f64> = if thermal_live {
            Vec::with_capacity(n_cells)
        } else {
            Vec::new()
        };
        let mut q_gen_w = 0.0;
        let mut flags = EventFlags::empty();
        for (g, group) in self.groups.iter_mut().enumerate() {
            let (e_gv, r_gv) = group_src[g];
            let v_node = e_gv - i_g * r_gv; // start-of-step shared node voltage
            for (k, cell) in group.cells.iter_mut().enumerate() {
                let (e_k, r_k) = cell_src[g][k];
                let i_k = (e_k - v_node) / r_k;
                // Heat from the same start-of-step state that produced `i_k`, so
                // the energy accounting closes exactly (see `cell_heat_w`).
                let state = cell.model.state();
                let q = cell_heat_w(
                    i_k,
                    r_k,
                    state.v_rc.iter().sum::<f64>(),
                    state.temp_k,
                    docv_dt_lookup(&self.chem.ocv, state.soc),
                );
                q_gen_w += q;
                if thermal_live {
                    // Series-major, parallel-minor — the index order `thermal`
                    // expects.
                    heat_w.push(q);
                }
                let eff_cap = cap_ah * cell.capacity_factor;
                flags |= advance_cell(cell.model.state_mut(), &self.chem, i_k, dt, eff_cap);
            }
        }

        // --- integrate temperatures against that heat.
        if let ThermalConfig::Network { k_neighbor_w_per_k } = self.thermal {
            let t_env = env.t_coolant.unwrap_or(env.t_ambient);
            let mut temps: Vec<f64> = Vec::with_capacity(n_cells);
            for group in &self.groups {
                for cell in &group.cells {
                    temps.push(cell.model.state().temp_k);
                }
            }
            let mut scratch = Vec::with_capacity(n_cells);
            advance_temperatures(
                &mut temps,
                &mut scratch,
                &heat_w,
                &ThermalStep {
                    series,
                    parallel,
                    params: &self.chem.thermal,
                    k_neighbor_w_per_k,
                    t_env,
                    dt,
                },
            );
            let mut i = 0;
            for group in &mut self.groups {
                for cell in &mut group.cells {
                    cell.model.state_mut().temp_k = temps[i];
                    i += 1;
                }
            }
        }
        self.sim_time_s += dt;

        // --- end-of-step reporting. Recompute each group's shared node voltage
        // from end-of-step state with the same pack current, so parallel cells
        // report one consistent terminal voltage (v_cell is per group).
        let mut v_terminal = 0.0;
        let mut v_cell_min = f64::INFINITY;
        let mut v_cell_max = f64::NEG_INFINITY;
        let mut t_min = f64::INFINITY;
        let mut t_max = f64::NEG_INFINITY;
        let mut rem_ah = 0.0; // Σ soc_k · eff_cap_k
        let mut nom_ah = 0.0; // Σ eff_cap_k
                              // Group voltages are gathered only when something will sense them.
        let mut v_group: Vec<f64> = if self.bms.is_some() {
            Vec::with_capacity(series)
        } else {
            Vec::new()
        };
        for group in &self.groups {
            let mut sum_g = 0.0;
            let mut sum_eg = 0.0;
            for cell in &group.cells {
                let (e, r) = cell_source(cell.model.state(), &self.chem, cell.r0_factor);
                let g = 1.0 / r;
                sum_g += g;
                sum_eg += e * g;
                let s = cell.model.state();
                t_min = t_min.min(s.temp_k);
                t_max = t_max.max(s.temp_k);
                let eff_cap = cap_ah * cell.capacity_factor;
                rem_ah += s.soc * eff_cap;
                nom_ah += eff_cap;
            }
            let v_g = (sum_eg - i_g) / sum_g; // = E_g' − I_g·R_g'
            v_terminal += v_g;
            v_cell_min = v_cell_min.min(v_g);
            v_cell_max = v_cell_max.max(v_g);
            if self.bms.is_some() {
                v_group.push(v_g);
            }
        }

        // --- sample the sensors from the end-of-step state, for the *next* step to
        // act on. This is the only place ground truth crosses into the BMS, and it
        // crosses through the sensor error model.
        if sensor_tick {
            if let Some(bms) = &self.bms {
                // Read the probes through a shared borrow: `groups` and `bms` are
                // distinct fields, so this needs no copy of the probe list.
                let temp_probe_k: Vec<f64> = bms
                    .config()
                    .temp_probes
                    .iter()
                    .map(|&(s, p)| {
                        // Validated in range at construction; the fallback keeps
                        // `step` panic-free should that ever stop holding.
                        self.groups
                            .get(s as usize)
                            .and_then(|g| g.cells.get(p as usize))
                            .map_or(f64::NAN, |c| c.model.state().temp_k)
                    })
                    .collect();
                let sim_time_s = self.sim_time_s;
                let bms = self.bms.as_mut().expect("matched as Some just above");
                bms.sample(v_group, temp_probe_k, i_g, sim_time_s, &mut self.rng);
            }
        }
        let soc_bms = self.bms.as_ref().map(Bms::soc_estimate);

        Telemetry {
            v_terminal,
            i_actual: i_g,
            soc_true: rem_ah / nom_ah,
            soc_bms,
            t_min,
            t_max,
            v_cell_min,
            v_cell_max,
            soh_capacity: 1.0,
            soh_resistance: 1.0,
            q_gen_w,
            flags,
        }
    }

    /// The pack's BMS, or `None` if it has none.
    ///
    /// Gives a client (or a test) the BMS's *beliefs* — its SOC estimate and the last
    /// sensor frame — for side-by-side comparison against the ground truth in
    /// [`Telemetry`] and [`Pack::cell`]. That comparison is the point.
    #[must_use]
    pub fn bms(&self) -> Option<&Bms> {
        self.bms.as_ref()
    }
}

/// Check a [`BmsConfig`]'s numeric ranges and probe positions.
fn validate_bms(bms: &BmsConfig, series: u16, parallel: u16) -> Result<(), BuildError> {
    let checks: [(&'static str, f64, &'static str, bool); 6] = [
        (
            "current_offset_a",
            bms.current_offset_a,
            "must be finite",
            bms.current_offset_a.is_finite(),
        ),
        (
            "current_noise_sigma_a",
            bms.current_noise_sigma_a,
            "must be finite and >= 0",
            bms.current_noise_sigma_a.is_finite() && bms.current_noise_sigma_a >= 0.0,
        ),
        (
            "rest_current_threshold_a",
            bms.rest_current_threshold_a,
            "must be finite and >= 0",
            bms.rest_current_threshold_a.is_finite() && bms.rest_current_threshold_a >= 0.0,
        ),
        (
            "rest_time_for_ocv_s",
            bms.rest_time_for_ocv_s,
            "must be finite and >= 0",
            bms.rest_time_for_ocv_s.is_finite() && bms.rest_time_for_ocv_s >= 0.0,
        ),
        (
            "ocv_correction_gain",
            bms.ocv_correction_gain,
            "must be in [0, 1]",
            (0.0..=1.0).contains(&bms.ocv_correction_gain),
        ),
        (
            "min_ocv_slope_v_per_soc",
            bms.min_ocv_slope_v_per_soc,
            "must be finite and >= 0",
            bms.min_ocv_slope_v_per_soc.is_finite() && bms.min_ocv_slope_v_per_soc >= 0.0,
        ),
    ];
    for (field, value, reason, ok) in checks {
        if !ok {
            return Err(BuildError::BadBmsConfig {
                field,
                reason,
                value,
            });
        }
    }
    // `initial_soc_error` is deliberately unconstrained beyond finiteness: a BMS may
    // legitimately boot believing something absurd, and the estimate is clamped to
    // [0, 1] anyway.
    if !bms.initial_soc_error.is_finite() {
        return Err(BuildError::BadBmsConfig {
            field: "initial_soc_error",
            reason: "must be finite",
            value: bms.initial_soc_error,
        });
    }
    for (index, &(s, p)) in bms.temp_probes.iter().enumerate() {
        if s >= series || p >= parallel {
            return Err(BuildError::BadTempProbe {
                index,
                s,
                p,
                series,
                parallel,
            });
        }
    }
    Ok(())
}

/// Lower bound on a scatter factor. A Gaussian has unbounded tails; a factor at or
/// below zero would divide by zero in the group solve (`1/R`) or make coulomb
/// counting blow up, so draws are clamped to this positive floor. It only bites at
/// extreme sigma (realistic manufacturing scatter is a few percent).
const MIN_FACTOR: f64 = 0.05;

/// Draw one cell's `(capacity_factor, r0_factor)` from the pack RNG.
///
/// With no scatter (both sigmas zero) this is exactly `(1.0, 1.0)` and does **not**
/// touch the RNG — a no-scatter pack leaves the RNG at its seed until something
/// genuinely random happens. Otherwise each factor is `1 + σ·z` for an independent
/// standard-normal `z` (Box–Muller), clamped to [`MIN_FACTOR`]. A zero sigma on one
/// axis still yields exactly `1.0` for that axis while the other is perturbed.
fn draw_factors(rng: &mut ChaCha8Rng, scatter: &Scatter) -> (f64, f64) {
    if scatter.capacity_sigma == 0.0 && scatter.r0_sigma == 0.0 {
        return (1.0, 1.0);
    }
    // Box–Muller yields two independent standard normals from two uniforms; use
    // one for each axis so capacity and R0 scatter are independent.
    let (z0, z1) = standard_normal_pair(rng);
    let cap = (1.0 + scatter.capacity_sigma * z0).max(MIN_FACTOR);
    let r0 = (1.0 + scatter.r0_sigma * z1).max(MIN_FACTOR);
    (cap, r0)
}
