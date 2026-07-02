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

use crate::chem::ChemistryParams;
use crate::ecm::{advance_cell, cell_source, solve_current, CellModel, EcmState};
use crate::flags::EventFlags;
use crate::{Demand, Env, Telemetry};

/// Current snapshot schema version. Bumped whenever [`Pack`]'s serialized layout
/// changes (see `CLAUDE.md`).
pub const SNAPSHOT_VERSION: u32 = 1;

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
/// This doubles as (part of) the scenario file format. BMS, aging, and thermal
/// configuration are added in later phases.
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

        Ok(Self {
            version: SNAPSHOT_VERSION,
            chem,
            series: config.series,
            parallel: config.parallel,
            groups,
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
    /// `env` is accepted for API stability; the thermal coupling that consumes it
    /// (cell temperature responding to ambient/coolant) arrives in Phase 2, so
    /// cell temperature is currently held at its initial value.
    pub fn step(&mut self, dt: f64, demand: Demand, _env: &Env) -> Telemetry {
        let cap_ah = self.chem.cell.capacity_ah;

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

        // --- split into per-cell currents and advance each cell in place.
        let mut flags = EventFlags::empty();
        for (g, group) in self.groups.iter_mut().enumerate() {
            let (e_gv, r_gv) = group_src[g];
            let v_node = e_gv - i_g * r_gv; // start-of-step shared node voltage
            for (k, cell) in group.cells.iter_mut().enumerate() {
                let (e_k, r_k) = cell_src[g][k];
                let i_k = (e_k - v_node) / r_k;
                let eff_cap = cap_ah * cell.capacity_factor;
                flags |= advance_cell(cell.model.state_mut(), &self.chem, i_k, dt, eff_cap);
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
        }

        Telemetry {
            v_terminal,
            i_actual: i_g,
            soc_true: rem_ah / nom_ah,
            soc_bms: None,
            t_min,
            t_max,
            v_cell_min,
            v_cell_max,
            soh_capacity: 1.0,
            soh_resistance: 1.0,
            flags,
        }
    }
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

/// A uniform `f64` in `[0, 1)` with full 53-bit mantissa resolution.
fn next_unit(rng: &mut ChaCha8Rng) -> f64 {
    use rand_core::RngCore;
    // Top 53 bits of a u64 → an integer in [0, 2^53), scaled into [0, 1).
    (rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Two independent standard normals via the Box–Muller transform.
fn standard_normal_pair(rng: &mut ChaCha8Rng) -> (f64, f64) {
    // Guard the radius against u1 == 0 (ln(0) = −∞); MIN_POSITIVE keeps it finite.
    let u1 = next_unit(rng).max(f64::MIN_POSITIVE);
    let u2 = next_unit(rng);
    let mag = (-2.0 * u1.ln()).sqrt();
    let angle = core::f64::consts::TAU * u2;
    (mag * angle.cos(), mag * angle.sin())
}
