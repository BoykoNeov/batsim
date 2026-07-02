//! The battery pack — the one public simulation entry point.
//!
//! Per the design contract, the pack is the first-class citizen and a single cell
//! is the degenerate `1S1P` pack; there is no separate cell API. This Phase 0
//! slice implements only the `1S1P` case; general series/parallel topology (with
//! the closed-form group solve, scatter, and snapshots) lands in Phase 1 behind
//! this same public API.

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::chem::ChemistryParams;
use crate::ecm::{cell_step, CellModel, EcmState};
use crate::{Demand, Env, Telemetry};

/// Current snapshot schema version. Bumped whenever [`Pack`]'s serialized layout
/// changes (see `CLAUDE.md`).
pub const SNAPSHOT_VERSION: u32 = 1;

/// Pack topology and initial conditions.
///
/// This doubles as (part of) the scenario file format. Scatter, BMS, aging, and
/// thermal configuration are added in later phases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackConfig {
    /// Number of series elements (groups). Phase 0 supports `1`.
    pub series: u16,
    /// Number of parallel cells per group. Phase 0 supports `1`.
    pub parallel: u16,
    /// Initial state of charge for every cell, in \[0, 1\].
    pub initial_soc: f64,
    /// Initial temperature for every cell \[K\].
    pub initial_temp_k: f64,
    /// Seed for the single simulation RNG (part of the snapshot).
    pub seed: u64,
}

/// Reasons [`Pack::new`] can fail.
#[derive(Debug, Error, PartialEq)]
pub enum BuildError {
    /// The requested topology is not yet implemented in this phase.
    #[error("topology {series}S{parallel}P is not supported yet (Phase 0 is 1S1P only)")]
    UnsupportedTopology {
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

/// A battery pack: the full, ground-truth simulation state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    /// Snapshot schema version.
    version: u32,
    /// Cell chemistry (config; identical for every cell in this phase).
    chem: ChemistryParams,
    /// Series element count.
    series: u16,
    /// Parallel cell count per group.
    parallel: u16,
    /// Ground-truth cell state. Phase 0: exactly one cell.
    cell: CellModel,
    /// The single seeded RNG; its state is part of the snapshot.
    rng: ChaCha8Rng,
    /// Simulation time elapsed \[s\].
    sim_time_s: f64,
}

impl Pack {
    /// Build a pack from config and a (validated-on-the-way) chemistry.
    ///
    /// # Errors
    /// Returns [`BuildError`] for an unsupported topology, out-of-range initial
    /// conditions, or an invalid chemistry.
    pub fn new(config: &PackConfig, chem: ChemistryParams) -> Result<Self, BuildError> {
        chem.validate()?;
        if config.series != 1 || config.parallel != 1 {
            return Err(BuildError::UnsupportedTopology {
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

        let state = EcmState {
            soc: config.initial_soc,
            v_rc: vec![0.0; chem.n_rc()],
            temp_k: config.initial_temp_k,
        };
        let cell = match chem.n_rc() {
            1 => CellModel::Ecm1Rc(state),
            // validate() guarantees 1 or 2 pairs.
            _ => CellModel::Ecm2Rc(state),
        };

        Ok(Self {
            version: SNAPSHOT_VERSION,
            chem,
            series: config.series,
            parallel: config.parallel,
            cell,
            rng: ChaCha8Rng::seed_from_u64(config.seed),
            sim_time_s: 0.0,
        })
    }

    /// Simulation time elapsed so far \[s\].
    #[must_use]
    pub fn sim_time_s(&self) -> f64 {
        self.sim_time_s
    }

    /// Advance the simulation by `dt` seconds under `demand`. Never panics.
    ///
    /// `env` is accepted for API stability; the thermal coupling that consumes it
    /// (cell temperature responding to ambient/coolant) arrives in Phase 2, so
    /// cell temperature is currently held at its initial value.
    pub fn step(&mut self, dt: f64, demand: Demand, _env: &Env) -> Telemetry {
        let out = cell_step(self.cell.state_mut(), &self.chem, demand, dt);
        self.sim_time_s += dt;

        let s = self.cell.state();
        Telemetry {
            v_terminal: out.v_terminal,
            i_actual: out.i,
            soc_true: s.soc,
            soc_bms: None,
            t_min: s.temp_k,
            t_max: s.temp_k,
            v_cell_min: out.v_terminal,
            v_cell_max: out.v_terminal,
            soh_capacity: 1.0,
            soh_resistance: 1.0,
            flags: out.flags,
        }
    }
}
