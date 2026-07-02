//! Event flags raised during a step.
//!
//! Flags are a bitset returned in [`crate::Telemetry::flags`] each step. They are
//! the engine's channel for reporting physical events — protection trips, clamps,
//! safety states — without panicking or returning `Err` from `step()`.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// Bitset of events raised during a single [`crate::Pack::step`].
    ///
    /// A flag being set means the condition occurred *during that step*; flags are
    /// recomputed fresh each step (they are not sticky). The full set is defined
    /// up front so downstream clients have a stable contract; phases beyond the
    /// current one begin actually raising the later flags.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct EventFlags: u32 {
        /// SOC hit the upper clamp (1.0): an over-charge attempt was truncated.
        const SOC_CLAMPED_HIGH = 1 << 0;
        /// SOC hit the lower clamp (0.0): an over-discharge attempt was truncated.
        const SOC_CLAMPED_LOW  = 1 << 1;
        /// Over-voltage: a group voltage exceeded the chemistry's `v_max`.
        const OV               = 1 << 2;
        /// Under-voltage: a group voltage fell below the chemistry's `v_min`.
        const UV               = 1 << 3;
        /// Over-current relative to the configured charge/discharge limit.
        const OC               = 1 << 4;
        /// Over-temperature relative to the chemistry's `t_max`.
        const OT               = 1 << 5;
        /// Under-temperature (e.g. charge inhibit below `t_charge_min`).
        const UT               = 1 << 6;
        /// Charging below the plating temperature above the C-rate threshold.
        const PLATING_RISK     = 1 << 7;
        /// A group is actively bleeding charge through its balancing resistor.
        const BALANCING        = 1 << 8;
        /// The main contactor is open (BMS protection or explicit command).
        const CONTACTOR_OPEN   = 1 << 9;
        /// A cell has vented (temperature exceeded `t_vent`).
        const VENTED           = 1 << 10;
        /// Thermal runaway is in progress on at least one cell.
        const THERMAL_RUNAWAY  = 1 << 11;
    }
}
