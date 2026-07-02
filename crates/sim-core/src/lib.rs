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
//! This is a Phase 0 scaffold: the public API shape is sketched below; the physics
//! bodies are filled in over the phased build plan (see `CLAUDE.md`).

#![forbid(unsafe_code)]

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
    /// Capacity state of health in (0, 1\].
    pub soh_capacity: f64,
    /// Resistance growth factor, ≥ 1.
    pub soh_resistance: f64,
}
