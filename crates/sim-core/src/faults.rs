//! Injected faults: the sanctioned way to override the physics.
//!
//! `CLAUDE.md` draws a hard line between two kinds of failure. **Emergent** failures
//! (plating, thermal runaway) must fall out of the physics and may never be scripted.
//! **Injected** faults are the one exception: a client says "this cell develops a
//! 50 Ω internal short at t = 600 s", and from that moment the physics carries it.
//! Nothing here animates an outcome — every variant lands as a term in the same
//! equations the healthy pack solves.
//!
//! # When a fault fires
//!
//! Faults are scheduled against **simulation** time and held in a queue ordered by
//! timestamp. A fault fires on the first step whose interval `[t, t + dt)` contains
//! its timestamp, and is applied at **start of step**, before the electrical solve —
//! so the step that fires it is also the first step to feel it. Granularity is
//! therefore `dt`: a fault scheduled for 10.5 s in a 60 s step lands at the start of
//! the step covering it. That is the same family of accepted imprecision as the BMS's
//! one-step sampling lag (see [`crate::bms`]).
//!
//! A fault whose timestamp is already in the past fires on the next stepping step
//! rather than being silently dropped — scheduling into the past is a client error,
//! and losing the fault is a worse answer than firing it late.
//!
//! Firing is gated on `dt > 0`. A zero-length step is an observation, not a tick, and
//! it must leave the engine exactly as it found it (pinned by
//! `snapshot.rs::zero_length_step_does_not_mutate_state`). Firing a fault is a
//! reaction to *information*, not to elapsed time, so — like the BMS sensor path and
//! the aging sub-clock — it needs the guard explicitly rather than getting it free
//! from `dt` scaling.
//!
//! # The soft internal short is a conductance, not a scripted drain
//!
//! [`Fault::SoftInternalShort`] adds a leakage conductance `G_s = 1/R_s` across one
//! cell's own terminals, *inside* the cell. That is physically distinct from the
//! balancing bleed, which sits across the whole parallel group and dissipates in an
//! external resistor. Three behaviours fall out of the one term, which is why it is
//! the right model:
//!
//! * **Self-discharge at rest.** With no terminal current the group node settles
//!   lower, and the cell keeps pushing `(E − V)/R0` through its own internal branch.
//!   The cell drains while the pack sits idle — the whole point of the fault.
//! * **The SOC-draining current is the internal branch current**, not the terminal
//!   current. The two differ by exactly `V·G_s`, and coulomb counting must use the
//!   former or a shorted cell never loses charge.
//! * **The dissipation heats that cell.** `V²·G_s` goes into the shorted cell's own
//!   thermal node, unlike the balancing bleed (which heats a resistor outside the
//!   cells). A shorted cell that does not warm up is an easy and invisible mistake.
//!
//! ## What it does not model: interconnect resistance
//!
//! The leakage path hangs off the cell's terminals, and in a parallel group those
//! terminals *are* the group node — so the neighbours feed the short too, and matched
//! cells share the drain exactly equally. That is right for an ideal group and it is
//! the honest consequence of the pack model having no busbar or weld resistance
//! between a cell and its node: a real group's shorted cell drains somewhat faster
//! than its neighbours. What is never shared is the **heat**, which stays in the cell
//! containing the leakage path — a whole group draining into one hot spot is the
//! shape of the real failure, and that part the model does get.
//!
//! ## Why it is added as a conductance rather than folded into the cell's source
//!
//! A shunt across a Thévenin source `(E, R0)` transforms it to
//! `E' = E·R_s/(R0+R_s)`, `R' = R0·R_s/(R0+R_s)`. Doing that inside
//! [`crate::ecm::cell_source`] would put the transformed pair in the pack's
//! memoised source cache — and then the cell's *own* `R0` would have to be recovered
//! from it as `1/R0 = 1/R' − G_s` to compute `I²·R0` heat, which is a subtraction of
//! nearly-equal reciprocals precisely in the regime a soft short lives in
//! (`R_s ≫ R0`).
//!
//! The Norton form avoids the question entirely. A shunt contributes conductance and
//! **no** Norton current, so the group's node equation only gains a term in its
//! denominator:
//!
//! ```text
//! V = (Σ E_k/R_k − I_g) / (Σ 1/R_k + Σ G_s,k + G_bleed)
//! ```
//!
//! exactly like the balancing bleed, but per cell instead of per group. The per-cell
//! internal current `(E_k − V)/R_k` keeps its existing formula and its existing
//! meaning, `cell_source` keeps its signature, and the source cache's invariant is
//! untouched — so injecting a short needs no cache invalidation at all.
//!
//! # The external short sits outside the contactor
//!
//! [`Fault::ExternalShort`] is a resistance across the pack's **load-side**
//! terminals, downstream of the main contactor. Opening the contactor therefore
//! interrupts it. That choice is deliberate: it is what makes the BMS-on / BMS-off
//! contrast a real experiment (protection sees the voltage sag, derates, and
//! eventually latches the contactor open, saving the pack — where an unprotected pack
//! runs the short until it empties or overheats). The cell-side short, which no
//! contactor can save you from, is what [`Fault::SoftInternalShort`] already models.
//!
//! # Sensor faults are the last word
//!
//! [`Fault::SensorStuck`] and [`Fault::SensorOffset`] corrupt the [`crate::SensorFrame`]
//! *after* the BMS's own error model (offset, noise) has been applied — a stuck
//! sensor is stuck, not stuck-plus-noise. The noise draw still happens either way, so
//! injecting a sensor fault does not shift the RNG stream and every other draw in the
//! trajectory stays aligned.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Which sensor an injected sensor fault targets.
///
/// These are the only sensors that exist — one voltage per parallel group, `n`
/// temperature probes at configured cell positions, and one pack-current sensor (see
/// [`crate::SensorFrame`]). A pack with no BMS has no sensors, so a sensor fault
/// cannot be scheduled against one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorId {
    /// The voltage sensor on the parallel group at this **series** index.
    GroupVoltage(u16),
    /// The temperature probe at this index into [`crate::BmsConfig::temp_probes`] —
    /// an index into the probe list, not a cell position.
    TempProbe(u16),
    /// The single pack-current sensor.
    PackCurrent,
}

/// A fault that can be injected into a running pack.
///
/// Cell positions are zero-based `(series, parallel)` indices, matching
/// [`crate::Pack::cell`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fault {
    /// Replace one cell's static manufacturing factors — a deterministic scatter
    /// outlier.
    ///
    /// The new factors **replace** whatever the cell had (its scatter draw, or an
    /// earlier `WeakCell`); they are not multiplied onto it. Both are clamped to a
    /// positive floor, as [`crate::Pack::set_cell_factors`] does.
    WeakCell {
        /// Series index of the cell.
        s: u16,
        /// Parallel index of the cell.
        p: u16,
        /// New capacity multiplier (effective capacity = nominal × this × SOH).
        capacity_factor: f64,
        /// New `R0` multiplier (effective `R0` = nominal × this × SOH).
        r0_factor: f64,
    },
    /// A leakage resistance across one cell's own terminals, inside the cell.
    ///
    /// Drains that cell even at rest and heats it with its own dissipation; see the
    /// module docs. Injecting two shorts on the same cell puts them in parallel (the
    /// conductances add), which is the physically honest composition.
    SoftInternalShort {
        /// Series index of the cell.
        s: u16,
        /// Parallel index of the cell.
        p: u16,
        /// Leakage resistance \[ohms\]. Must be finite and `> 0`.
        ohms: f64,
    },
    /// A resistance across the pack's load-side terminals, **downstream of the
    /// contactor** — opening the contactor interrupts it (see the module docs).
    ///
    /// Multiple external shorts compose in parallel.
    ExternalShort {
        /// Short resistance \[ohms\]. Must be finite and `> 0`.
        ohms: f64,
    },
    /// Force one sensor's reading to a fixed value, whatever the pack is doing.
    SensorStuck {
        /// Which sensor.
        sensor: SensorId,
        /// The value the sensor now reports — volts, kelvin, or amps
        /// (discharge-positive) depending on the sensor. Must be finite.
        value: f64,
    },
    /// Add a fixed error to one sensor's reading.
    SensorOffset {
        /// Which sensor.
        sensor: SensorId,
        /// Added to every reading, in that sensor's units. Must be finite.
        offset: f64,
    },
}

/// A [`Fault`] with the simulation time it is due to fire at.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduledFault {
    /// Simulation time \[s\] this fault is due at.
    pub at_s: f64,
    /// The fault itself.
    pub fault: Fault,
}

/// Reasons [`crate::Pack::schedule_fault`] can reject a fault.
#[derive(Debug, Error, PartialEq)]
pub enum FaultError {
    /// A numeric parameter was NaN or infinite.
    #[error("fault parameter '{field}' must be finite, got {value}")]
    NotFinite {
        /// Offending parameter name.
        field: &'static str,
        /// Offending value.
        value: f64,
    },
    /// A short resistance was zero or negative.
    #[error("fault resistance must be > 0 ohms, got {0}")]
    BadResistance(f64),
    /// A cell position was outside the pack's topology.
    #[error("fault targets cell {s}S{p}P, outside a {series}S{parallel}P pack")]
    BadCellIndex {
        /// Requested series index.
        s: u16,
        /// Requested parallel index.
        p: u16,
        /// Pack series count.
        series: u16,
        /// Pack parallel count.
        parallel: u16,
    },
    /// A sensor fault named a sensor this pack does not have.
    #[error("fault targets {sensor:?}, which this pack has no such sensor for")]
    NoSuchSensor {
        /// The sensor that was asked for.
        sensor: SensorId,
    },
}

/// How a sensor fault corrupts a reading.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum SensorFaultKind {
    /// Reading is replaced by this value.
    Stuck(f64),
    /// This is added to the reading.
    Offset(f64),
}

/// One active sensor fault: which sensor, and what it does to the reading.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SensorFault {
    /// The affected sensor.
    pub sensor: SensorId,
    /// What it does to that sensor's reading.
    pub(crate) kind: SensorFaultKind,
}

impl SensorFault {
    /// Apply this fault to one reading, returning the corrupted value.
    ///
    /// `Stuck` discards whatever was measured; `Offset` adds to it. Composition of
    /// several faults on one sensor is therefore order-dependent, and the order is
    /// the order they fired in.
    #[must_use]
    pub(crate) fn corrupt(&self, reading: f64) -> f64 {
        match self.kind {
            SensorFaultKind::Stuck(v) => v,
            SensorFaultKind::Offset(d) => reading + d,
        }
    }
}

/// The pack's fault state: what is scheduled, and what is currently in effect.
///
/// Per-cell effects do **not** live here — a [`Fault::SoftInternalShort`] becomes a
/// conductance on the cell it targets, and a [`Fault::WeakCell`] is folded into that
/// cell's static factors the moment it fires. This type holds the queue plus the two
/// pack-level effects (the external short and the sensor corruptions), so it is small
/// and its per-step cost is a single emptiness check on a healthy pack.
///
/// No `HashMap`: the queue is a `Vec` kept sorted by timestamp, with ties firing in
/// the order they were scheduled, so the trajectory is a function of the scheduling
/// sequence alone.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaultState {
    /// Faults not yet due, ascending by [`ScheduledFault::at_s`]; equal timestamps
    /// keep insertion order.
    pending: Vec<ScheduledFault>,
    /// Total conductance \[S\] across the pack's load-side terminals from fired
    /// [`Fault::ExternalShort`]s. `0.0` = no short.
    external_short_g: f64,
    /// Active sensor corruptions, in the order they fired.
    sensors: Vec<SensorFault>,
}

impl FaultState {
    /// Faults still waiting to fire, ascending by timestamp.
    #[must_use]
    pub fn pending(&self) -> &[ScheduledFault] {
        &self.pending
    }

    /// Total external-short conductance \[S\] across the pack's load-side terminals.
    /// `0.0` when there is no external short.
    ///
    /// Reported as a conductance because that is the form the solve uses and the form
    /// parallel shorts compose in; the equivalent resistance is its reciprocal.
    #[must_use]
    pub fn external_short_conductance_s(&self) -> f64 {
        self.external_short_g
    }

    /// The sensor corruptions currently in effect, in the order they fired.
    #[must_use]
    pub fn sensor_faults(&self) -> &[SensorFault] {
        &self.sensors
    }

    /// Whether anything at all has been scheduled or is in effect **at pack level**.
    ///
    /// Deliberately does not know about per-cell shunts, which live on the cells.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.pending.is_empty() && self.sensors.is_empty() && self.external_short_g == 0.0
    }

    /// Insert a fault into the queue, keeping it sorted by timestamp and keeping
    /// insertion order among equal timestamps.
    pub(crate) fn schedule(&mut self, at_s: f64, fault: Fault) {
        // `<=` (not `<`) is what makes ties stable: the new fault goes after every
        // fault already scheduled for the same instant.
        let idx = self.pending.partition_point(|f| f.at_s <= at_s);
        self.pending.insert(idx, ScheduledFault { at_s, fault });
    }

    /// Remove and return every fault due strictly before `t_end`, in fire order.
    ///
    /// The queue is sorted, so the due faults are a prefix. Faults dated before the
    /// current time are part of that prefix and fire now (see the module docs).
    /// Allocates nothing when nothing is due.
    pub(crate) fn take_due(&mut self, t_end: f64) -> Vec<ScheduledFault> {
        if self.pending.is_empty() {
            return Vec::new();
        }
        let n = self.pending.partition_point(|f| f.at_s < t_end);
        if n == 0 {
            return Vec::new();
        }
        self.pending.drain(..n).collect()
    }

    /// Add an external short of `ohms` in parallel with any already present.
    pub(crate) fn add_external_short(&mut self, ohms: f64) {
        self.external_short_g += 1.0 / ohms;
    }

    /// Record a sensor corruption.
    pub(crate) fn add_sensor_fault(&mut self, sensor: SensorId, kind: SensorFaultKind) {
        self.sensors.push(SensorFault { sensor, kind });
    }

    /// Drop the queue and every pack-level effect. Returns how many entries went
    /// away (pending plus active sensor faults; an external short counts as one).
    pub(crate) fn clear(&mut self) -> usize {
        let n = self.pending.len() + self.sensors.len() + usize::from(self.external_short_g != 0.0);
        self.pending.clear();
        self.sensors.clear();
        self.external_short_g = 0.0;
        n
    }
}
