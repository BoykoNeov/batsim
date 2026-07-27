//! `sim-core` — the pure battery-pack simulation engine.
//!
//! This crate is a deterministic state machine: `step(dt, demand, env) -> Telemetry`.
//! It performs no file I/O, no networking, no threading, no wall-clock reads, and
//! holds no global state. All I/O lives in adapter crates. See `CLAUDE.md` for the
//! full design contract.
//!
//! # Sign convention
//! Positive current = **discharge** (current flowing out of the pack terminals).
//! Charging is negative current.
//!
//! # Units
//! SI throughout: seconds, amperes, volts, ohms, farads, joules, kelvin.
//!
//! The public API shape is sketched below; the physics behind it is filled in over
//! the phased build plan (see `CLAUDE.md`). Fields for a not-yet-implemented phase
//! carry their eventual meaning and a placeholder value, so downstream clients code
//! against a stable contract — [`EventFlags::VENTED`] cannot be raised until the
//! runaway slice lands, for instance. Note that a placeholder-looking value is not
//! always a placeholder: [`Telemetry::soh_capacity`] reads exactly `1.0` on a pack
//! with no aging configured, and that is the real answer, not a stub.

#![forbid(unsafe_code)]

pub mod aging;
pub mod bms;
pub mod chem;
pub mod ecm;
pub mod flags;
mod noise;
pub mod pack;
pub mod thermal;

pub use aging::{Aging, AgingConfig};
pub use bms::{BalancingConfig, Bms, BmsConfig, ProtectionConfig, SensorFrame};
pub use chem::{AgingParams, ChemistryError, ChemistryParams, ThermalParams};
pub use ecm::{CellModel, EcmState};
pub use flags::EventFlags;
pub use pack::{
    BuildError, CellIndexError, CellView, Pack, PackConfig, RestoreError, Scatter, Snapshot,
    SNAPSHOT_VERSION,
};
pub use thermal::ThermalConfig;

/// What the outside world asks of the pack this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Demand {
    /// Constant current. Positive = discharge \[A\].
    Current(f64),
    /// Constant power. Positive = discharge \[W\].
    Power(f64),
    /// Hold terminal voltage (e.g. CV charge phase) \[V\].
    Voltage(f64),
    /// Open circuit / rest.
    Rest,
}

/// Environment for this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// Ambient temperature \[K\].
    pub t_ambient: f64,
    /// Optional coolant temperature \[K\] (None = passive cooling to ambient only).
    pub t_coolant: Option<f64>,
}

/// Cheap per-step summary of pack state. Per-cell arrays are available on request
/// via the ground-truth accessors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Telemetry {
    /// Terminal voltage \[V\].
    pub v_terminal: f64,
    /// Actual pack current \[A\]; may differ from demand if the BMS derates or opens.
    pub i_actual: f64,
    /// Ground-truth state of charge, in \[0, 1\].
    ///
    /// This is the fraction of the capacity the pack has **today**, not of the
    /// capacity it shipped with: the denominator folds in each cell's
    /// [`Self::soh_capacity`]. A half-full pack that has faded 20 % reads `0.5`, not
    /// `0.4`. That is the same convention per-cell coulomb counting uses, and it is
    /// what makes a SOC readout mean "how much of the tank is left" rather than
    /// quietly encoding the tank's age.
    pub soc_true: f64,
    /// BMS state-of-charge estimate in \[0, 1\] (None if the BMS is disabled).
    pub soc_bms: Option<f64>,
    /// Minimum cell temperature \[K\].
    pub t_min: f64,
    /// Maximum cell temperature \[K\].
    pub t_max: f64,
    /// Minimum cell voltage \[V\].
    pub v_cell_min: f64,
    /// Maximum cell voltage \[V\].
    pub v_cell_max: f64,
    /// Capacity state of health in (0, 1\]: pack capacity now over pack capacity
    /// when new, capacity-weighted across cells. Exactly `1.0` on a pack without
    /// aging configured.
    pub soh_capacity: f64,
    /// Resistance growth factor, ≥ 1: the pack's present series resistance over what
    /// unworn cells in the same topology would present. Balancing bleed resistors are
    /// excluded — they lower impedance without anything having got healthier. Exactly
    /// `1.0` on a pack without aging configured.
    pub soh_resistance: f64,
    /// Total heat generated across every cell this step \[W\].
    ///
    /// Irreversible overpotential heat plus the entropic term, summed over cells
    /// (see [`ecm::cell_heat_w`]). Reported in every thermal mode — an
    /// [`thermal::ThermalConfig::Isothermal`] pack still says how much heat it
    /// makes, it just does not warm up. Can be slightly negative while an
    /// overpotential relaxes against a reversed current, or under a dominant
    /// endothermic entropic term.
    pub q_gen_w: f64,
    /// Power burned in the passive balancing resistors this step \[W\].
    ///
    /// Zero unless at least one group's bleed switch is closed. This is energy
    /// *thrown away* to bring high groups down — it is dissipated in the resistors,
    /// not inside the cells, so it does not feed the thermal network. Watching it is
    /// the honest way to see what passive balancing costs.
    pub q_balancing_w: f64,
    /// Total current drawn by the passive balancing resistors this step \[A\],
    /// discharge-positive (it flows out of the cells).
    ///
    /// This current is *additional* to [`Telemetry::i_actual`], not part of it: a
    /// bleeding group carries `i_actual + its share of this`, which is exactly how
    /// balancing brings a high group down relative to its series neighbours. Together
    /// with [`Telemetry::q_balancing_w`] it closes the pack energy balance, which with
    /// balancing active has four terms rather than three.
    pub i_balancing_a: f64,
    /// Events raised during this step (protection trips, clamps, safety states).
    pub flags: EventFlags,
}
