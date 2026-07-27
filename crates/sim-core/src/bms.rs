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
use crate::flags::EventFlags;
use crate::noise::standard_normal;

/// How the BMS responds when a measurement crosses a limit.
///
/// The limits themselves are **not** here — they live in the chemistry
/// ([`crate::chem::CellLimits`]), per the project rule that chemistry is data. This
/// type holds only *policy*: how far past a limit the response escalates from
/// derating to opening the contactor.
///
/// Response is graduated:
///
/// 1. **Derate.** A measurement at or past a chemistry limit shrinks the allowed
///    current window in the offending direction — often to zero — while leaving the
///    pack connected. Almost every excursion ends here: with the current clamped, the
///    pack relaxes back inside its limits on its own.
/// 2. **Open the contactor.** A measurement past a limit *plus* the corresponding
///    hard margin is treated as a fault rather than an operating condition. The
///    contactor **latches** open and stays open until
///    [`crate::Pack::clear_bms_fault`] is called — a safety contactor that silently
///    re-closed when a cell cooled below its threshold would be a thermostat, not a
///    protection device.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtectionConfig {
    /// How far past the chemistry's `v_max`/`v_min` \[V\] a measured group voltage
    /// must go before the contactor latches open instead of the current being
    /// derated. Must be finite and `>= 0`.
    pub v_hard_margin_v: f64,
    /// How far past the chemistry's `t_max_k` \[K\] a measured probe temperature must
    /// go before the contactor latches open. Must be finite and `>= 0`.
    pub t_hard_margin_k: f64,
}

/// Passive balancing: a bleed resistor across each parallel group, switched in when
/// that group's measured voltage runs high.
///
/// Passive balancing does not move charge from a full group to an empty one — it
/// wastes the excess as heat in the resistor so the others can catch up. That is why
/// it only helps near the end of charge, and why the pack it balances ends up with
/// *less* total energy than it started with. Demonstrating that trade-off is the
/// point of having it.
///
/// # How it enters the solve
///
/// A closed bleed switch is simply a conductance `G_b = 1/R_bleed` across the group
/// node, so the group's KCL becomes
///
/// ```text
/// V = (Σ E_k/R_k − I) / (Σ 1/R_k + G_b)
/// ```
///
/// The group Thévenin stays exactly linear (`R_g' = 1/(Σ1/R_k + G_b)`,
/// `E_g' = (Σ E_k/R_k)·R_g'`), so series aggregation and the demand solve are
/// untouched, and the per-cell currents automatically sum to `I + I_bleed`. No
/// approximation and no extra iteration — one extra term in a denominator the step
/// already computes.
///
/// The *decision* to close the switch is made from the lagged sensor frame, because
/// that is a BMS control decision and the BMS only has sensors. The *physics* once
/// closed is exact.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BalancingConfig {
    /// Bleed resistance across one group \[ohms\]. Must be finite and `> 0`. Smaller
    /// means faster balancing and more waste heat.
    pub bleed_r_ohms: f64,
    /// Measured group voltage above which that group's bleed switch closes \[V\].
    ///
    /// Set this near the top of charge: below it, bleeding just throws away energy
    /// without reducing any imbalance that matters.
    pub v_threshold_v: f64,
}

/// What the BMS is allowed to know and how badly its sensors lie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BmsConfig {
    /// Passive balancing policy, or `None` for a BMS that never balances.
    pub balancing: Option<BalancingConfig>,
    /// Protection policy, or `None` for a **monitor-only** BMS.
    ///
    /// `None` still estimates SOC and reports sensor readings, it just never clamps a
    /// demand — the BMS watches the pack drive itself past its limits. That is a
    /// distinct teaching case from having no BMS at all (where there is no estimate
    /// and no instrumentation either).
    pub protection: Option<ProtectionConfig>,
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

    /// Lowest measured probe temperature \[K\], or `None` with no probes.
    #[must_use]
    pub fn min_probe_k(&self) -> Option<f64> {
        self.temp_probe_k.iter().copied().reduce(f64::min)
    }

    /// Highest measured group voltage \[V\], or `None` with no groups.
    #[must_use]
    pub fn max_group_v(&self) -> Option<f64> {
        self.v_group.iter().copied().reduce(f64::max)
    }

    /// Lowest measured group voltage \[V\], or `None` with no groups.
    #[must_use]
    pub fn min_group_v(&self) -> Option<f64> {
        self.v_group.iter().copied().reduce(f64::min)
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
    /// Latched hard-fault state: once the contactor opens it stays open until
    /// [`crate::Pack::clear_bms_fault`] is called.
    contactor_open: bool,
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
            contactor_open: false,
        }
    }

    /// Whether the main contactor is currently latched open by a hard fault.
    #[must_use]
    pub fn contactor_open(&self) -> bool {
        self.contactor_open
    }

    /// Clear a latched hard fault, closing the contactor. Returns whether it had been
    /// open — the operator-reset seam.
    pub(crate) fn clear_fault(&mut self) -> bool {
        let was_open = self.contactor_open;
        self.contactor_open = false;
        was_open
    }

    /// Fill `out` with each group's bleed conductance \[S\] for this step: `1/R_bleed`
    /// where the switch is closed, `0.0` where it is open, in series order.
    ///
    /// Left empty when balancing is disabled, which the caller reads as "no bleed
    /// anywhere". Decided from the lagged sensor frame — a real balancer switches on
    /// what it measured, not on what is true — but note this is a pure read: it
    /// mutates nothing, so it is safe to call on a zero-length probe step, where it
    /// keeps the reported voltage consistent with the step that follows.
    pub(crate) fn bleed_conductances(&self, out: &mut Vec<f64>) {
        out.clear();
        let Some(bal) = self.config.balancing else {
            return;
        };
        let g = 1.0 / bal.bleed_r_ohms;
        out.extend(
            self.frame
                .v_group
                .iter()
                .map(|&v| if v > bal.v_threshold_v { g } else { 0.0 }),
        );
    }

    /// Apply protection to the current the demand solved for, returning the current
    /// the pack will actually carry and the events raised.
    ///
    /// Every decision here is made from [`Self::frame`] — i.e. from measurements one
    /// step old — so the first step of an excursion is not prevented, only the ones
    /// after it. That is the accepted design (see the module docs); it is why the
    /// scenario tests assert "detects, derates and settles" rather than "never
    /// violates".
    ///
    /// `pack_ah` is the pack's nominal amp-hour capacity per series element
    /// (per-cell nominal × parallel count), which sets the C-rate current limits.
    /// Temperature protection is skipped entirely when the pack has no temperature
    /// probes — a BMS cannot protect against a condition it cannot measure, and that
    /// is a legitimate (if unwise) configuration.
    pub(crate) fn apply_protection(
        &mut self,
        chem: &ChemistryParams,
        i_req: f64,
        pack_ah: f64,
    ) -> (f64, EventFlags) {
        let mut flags = EventFlags::empty();
        let Some(prot) = self.config.protection else {
            return (i_req, flags);
        };
        let limits = &chem.cell;

        // --- hard faults: past a limit *plus* its margin is a fault, not an
        // operating point. Latch the contactor and stop.
        if let Some(t_hi) = self.frame.max_probe_k() {
            if t_hi > limits.t_max_k + prot.t_hard_margin_k {
                self.contactor_open = true;
                flags |= EventFlags::OT;
            }
        }
        if let Some(v_hi) = self.frame.max_group_v() {
            if v_hi > limits.v_max + prot.v_hard_margin_v {
                self.contactor_open = true;
                flags |= EventFlags::OV;
            }
        }
        if let Some(v_lo) = self.frame.min_group_v() {
            if v_lo < limits.v_min - prot.v_hard_margin_v {
                self.contactor_open = true;
                flags |= EventFlags::UV;
            }
        }
        if self.contactor_open {
            return (0.0, flags | EventFlags::CONTACTOR_OPEN);
        }

        // --- soft limits: shrink the allowed current window, staying connected.
        // Window is [−i_charge_max, i_discharge_max] with discharge positive.
        let mut i_discharge_max = limits.max_discharge_c * pack_ah;
        let mut i_charge_max = limits.max_charge_c * pack_ah;
        // Over-current is judged against the raw C-rate window, before the
        // limit-driven reductions below, so the flag names the actual cause.
        if i_req > i_discharge_max || i_req < -i_charge_max {
            flags |= EventFlags::OC;
        }
        if let Some(v_lo) = self.frame.min_group_v() {
            if v_lo <= limits.v_min {
                i_discharge_max = 0.0; // any further discharge digs the hole deeper
                flags |= EventFlags::UV;
            }
        }
        if let Some(v_hi) = self.frame.max_group_v() {
            if v_hi >= limits.v_max {
                i_charge_max = 0.0;
                flags |= EventFlags::OV;
            }
        }
        if let Some(t_hi) = self.frame.max_probe_k() {
            if t_hi >= limits.t_max_k {
                // Too hot for either direction: all current makes more heat.
                i_discharge_max = 0.0;
                i_charge_max = 0.0;
                flags |= EventFlags::OT;
            }
        }
        if let Some(t_lo) = self.frame.min_probe_k() {
            if t_lo < limits.t_charge_min_k {
                // Charge inhibit: plating risk, not a heat problem, so discharge
                // stays available.
                i_charge_max = 0.0;
                flags |= EventFlags::UT;
            }
        }

        (i_req.clamp(-i_charge_max, i_discharge_max), flags)
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
