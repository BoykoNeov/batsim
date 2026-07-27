//! Battery management system: the sensor-limited view of the pack.
//!
//! # The BMS never reads ground truth
//!
//! This is the design's non-negotiable point 8. The engine knows every cell's exact
//! state; the BMS knows only what a [`SensorFrame`] tells it — one voltage per
//! parallel group, a few temperature probes at configured positions, and one pack
//! current reading with configurable offset and noise. Everything the BMS believes
//! about the pack, including its state of charge, is derived from that. The gap
//! between [`Bms::soc_estimate`] and the true SOC is a feature to expose, not an
//! error to minimise.
//!
//! # Sampling lag
//!
//! A frame is sampled at the **end** of a step and consumed at the **start** of the
//! next one, so the BMS always acts on information one step old — exactly like a real
//! discretely sampled controller. Two consequences, both accepted by design (see
//! `docs/plans/phase-2-thermal-bms.md`):
//!
//! * Protection can overshoot a limit for one step before it reacts.
//! * The effective sample rate *is* the client's `dt`. A coarse `dt` makes the BMS
//!   proportionally more sluggish. Clients that care should drive the engine from a
//!   fixed-`dt` accumulator, which `CLAUDE.md` already prescribes for other reasons.
//!
//! The frame is part of the snapshot and **must** stay that way. It cannot be
//! recomputed on restore: the loaded group voltages depend on a current that is not
//! stored, and any noise draw has already advanced the RNG past reproducing it.
//!
//! A `dt` of zero is not a sample tick, so the BMS is skipped entirely on such a
//! step — otherwise a zero-length "observation" step would reset the rest timer,
//! possibly fire an OCV correction, and consume a noise draw. Unlike the physics,
//! which all scales by `dt` and is therefore self-guarding, the BMS reacts to
//! *information* rather than to elapsed time, so it needs the explicit guard.
//!
//! # Why the SOC estimate drifts
//!
//! [`Bms::soc_estimate`] is coulomb-counted from the *measured* current against
//! *nominal* capacity. Three independent error sources follow, and none of them is
//! a bug:
//!
//! 1. A current-sensor offset integrates into unbounded drift — the classic failure.
//! 2. Sensor noise integrates into a random walk.
//! 3. Nominal capacity is not true capacity (manufacturing scatter now, aging later).
//!
//! The corrective mechanism is an OCV reading taken after the pack has rested long
//! enough to relax, which is only informative where the OCV curve is steep. On LFP
//! it mostly is not, so the estimate stays wrong through the middle of the range.
//! That contrast between chemistries is a teaching goal.

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::chem::ChemistryParams;
use crate::ecm::ocv_invert;
use crate::noise::standard_normal;

/// What the BMS is allowed to know and how badly its sensors lie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BmsConfig {
    /// Systematic error added to every pack-current reading \[A\],
    /// discharge-positive. This is the error that *integrates*: a milliamp of offset
    /// is a percent of SOC after an hour.
    pub current_offset_a: f64,
    /// Standard deviation of the per-step current-sensor noise \[A\]. Must be `>= 0`;
    /// `0` gives a noiseless (but possibly still biased) sensor and draws no RNG.
    pub current_noise_sigma_a: f64,
    /// Cell positions `(series_idx, parallel_idx)` carrying a temperature probe.
    ///
    /// A real pack instruments a handful of cells, not all of them, so the BMS sees
    /// an under-sampled temperature field — and can miss the hottest cell entirely.
    /// May be empty (a pack with no thermal instrumentation).
    pub temp_probes: Vec<(u16, u16)>,
    /// Error in the initial SOC estimate, added to the true initial SOC.
    ///
    /// A BMS powering on mid-life does not know where it is; this seeds that
    /// ignorance. Positive means the BMS thinks it has more charge than it does.
    pub initial_soc_error: f64,
    /// A measured current at or below this magnitude \[A\] counts as "resting" for
    /// the purpose of accumulating rest time. Must be `>= 0`.
    pub rest_current_threshold_a: f64,
    /// How long the pack must have rested \[s\] before an OCV reading is considered
    /// relaxed enough to correct against. Must be `>= 0`. Too short and the BMS
    /// corrects against an unrelaxed overpotential, making its estimate *worse*.
    pub rest_time_for_ocv_s: f64,
    /// Fraction of the gap to the OCV-derived SOC to close per correction, in
    /// \[0, 1\]. `1.0` snaps to the reading; small values filter it.
    pub ocv_correction_gain: f64,
    /// Minimum OCV slope \[V per unit SOC\] for a reading to be trusted at all.
    ///
    /// Below this the correction is skipped entirely, because inverting a flat curve
    /// turns sensor error into enormous SOC error. This single threshold is what
    /// makes an LFP estimator behave differently from an NMC one on identical code.
    pub min_ocv_slope_v_per_soc: f64,
}

/// Everything the BMS measured at the end of a step.
///
/// Voltages and temperatures are, in this phase, *exact* readings of the true state
/// at the probe positions — the modelled error is in the current sensor and in the
/// spatial under-sampling of the probes. Injected sensor faults (stuck, offset)
/// arrive in a later phase and apply here, which is why this is a distinct type
/// rather than a handful of fields on [`Bms`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SensorFrame {
    /// Measured voltage of each parallel group \[V\], in series order. Parallel cells
    /// share one node, so this is the finest voltage resolution any real pack has —
    /// a weak cell hiding inside a healthy group is invisible here.
    pub v_group: Vec<f64>,
    /// Measured temperature at each configured probe \[K\], in config order.
    pub temp_probe_k: Vec<f64>,
    /// Measured pack current \[A\], discharge-positive, including offset and noise.
    pub i_pack_a: f64,
    /// Simulation time at which this frame was sampled \[s\].
    pub sampled_at_s: f64,
}

impl SensorFrame {
    /// Mean measured group voltage \[V\], the BMS's best single-number handle on
    /// "where is this pack on its OCV curve". `None` if the pack has no groups.
    #[must_use]
    pub fn mean_group_v(&self) -> Option<f64> {
        if self.v_group.is_empty() {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        let n = self.v_group.len() as f64;
        Some(self.v_group.iter().sum::<f64>() / n)
    }

    /// Highest measured probe temperature \[K\], or `None` with no probes.
    ///
    /// Note this is the hottest *instrumented* cell, which need not be the hottest
    /// cell — compare against [`crate::Telemetry::t_max`], which is ground truth.
    #[must_use]
    pub fn max_probe_k(&self) -> Option<f64> {
        self.temp_probe_k.iter().copied().reduce(f64::max)
    }
}

/// The BMS's own state: its beliefs, not the pack's facts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bms {
    /// Configuration, including the sensor error model.
    config: BmsConfig,
    /// Coulomb-counted state-of-charge estimate, in \[0, 1\].
    soc_est: f64,
    /// How long the pack has continuously measured as resting \[s\].
    rest_time_s: f64,
    /// The most recent sensor frame — sampled at the end of the previous step, which
    /// is all the BMS gets to act on this step.
    frame: SensorFrame,
}

impl Bms {
    /// Build a BMS for a pack starting uniformly at `initial_soc` and
    /// `initial_temp_k`.
    ///
    /// The initial frame is an open-circuit read: every cell starts at the same SOC
    /// with zero overpotential, so each group node sits exactly at `OCV(initial_soc)`
    /// regardless of resistance scatter, and the current sensor has not been read yet
    /// (reported as an exact zero, consuming no RNG so that a pack's scatter draws
    /// stay the only construction-time randomness).
    pub(crate) fn new(
        config: BmsConfig,
        chem: &ChemistryParams,
        series: u16,
        initial_soc: f64,
        initial_temp_k: f64,
    ) -> Self {
        let ocv0 = crate::ecm::ocv_lookup(&chem.ocv, initial_soc);
        let frame = SensorFrame {
            v_group: vec![ocv0; series as usize],
            temp_probe_k: vec![initial_temp_k; config.temp_probes.len()],
            i_pack_a: 0.0,
            sampled_at_s: 0.0,
        };
        Self {
            soc_est: (initial_soc + config.initial_soc_error).clamp(0.0, 1.0),
            rest_time_s: 0.0,
            config,
            frame,
        }
    }

    /// The BMS's state-of-charge estimate, in \[0, 1\]. Compare with
    /// [`crate::Telemetry::soc_true`] to see the estimator error.
    #[must_use]
    pub fn soc_estimate(&self) -> f64 {
        self.soc_est
    }

    /// The most recent sensor frame — the entirety of what the BMS can see.
    #[must_use]
    pub fn sensors(&self) -> &SensorFrame {
        &self.frame
    }

    /// The configuration this BMS was built with.
    #[must_use]
    pub fn config(&self) -> &BmsConfig {
        &self.config
    }

    /// How long the pack has continuously measured as resting \[s\].
    #[must_use]
    pub fn rest_time_s(&self) -> f64 {
        self.rest_time_s
    }

    /// Advance the SOC estimate by `dt` seconds using the frame sampled at the end of
    /// the previous step.
    ///
    /// `pack_nominal_ah` is the capacity the BMS *believes* the pack has: nominal
    /// per-cell capacity times the parallel count. It is deliberately not the true
    /// effective capacity — that difference is one of the drift sources.
    pub(crate) fn update_estimate(
        &mut self,
        chem: &ChemistryParams,
        dt: f64,
        pack_nominal_ah: f64,
    ) {
        let i_meas = self.frame.i_pack_a;

        // --- coulomb counting on the imperfect reading.
        let capacity_as = 3600.0 * pack_nominal_ah;
        self.soc_est = (self.soc_est - i_meas * dt / capacity_as).clamp(0.0, 1.0);

        // --- rest tracking, also on the imperfect reading: a sensor offset can make
        // a genuinely resting pack look busy, or vice versa.
        if i_meas.abs() <= self.config.rest_current_threshold_a {
            self.rest_time_s += dt;
        } else {
            self.rest_time_s = 0.0;
        }

        // --- OCV correction, when the pack has rested long enough *and* the curve is
        // steep enough there for the reading to mean anything.
        if self.rest_time_s < self.config.rest_time_for_ocv_s {
            return;
        }
        let Some(v_mean) = self.frame.mean_group_v() else {
            return;
        };
        let (soc_ocv, slope) = ocv_invert(&chem.ocv, v_mean);
        if slope < self.config.min_ocv_slope_v_per_soc || !soc_ocv.is_finite() {
            // Flat plateau (or a poisoned reading): the measurement carries no usable
            // SOC information, so the estimate is left alone to keep drifting.
            return;
        }
        let gain = self.config.ocv_correction_gain;
        self.soc_est = (self.soc_est + gain * (soc_ocv - self.soc_est)).clamp(0.0, 1.0);
    }

    /// Replace the stored frame with a fresh sample of the (end-of-step) pack state.
    ///
    /// `v_group` and `temp_probe_k` are already-gathered true values; the current
    /// reading is corrupted here, drawing from the pack RNG exactly once per step and
    /// only when `current_noise_sigma_a > 0`.
    pub(crate) fn sample(
        &mut self,
        v_group: Vec<f64>,
        temp_probe_k: Vec<f64>,
        i_true: f64,
        sim_time_s: f64,
        rng: &mut ChaCha8Rng,
    ) {
        let sigma = self.config.current_noise_sigma_a;
        let noise = if sigma > 0.0 {
            sigma * standard_normal(rng)
        } else {
            0.0
        };
        self.frame = SensorFrame {
            v_group,
            temp_probe_k,
            i_pack_a: i_true + self.config.current_offset_a + noise,
            sampled_at_s: sim_time_s,
        };
    }
}
