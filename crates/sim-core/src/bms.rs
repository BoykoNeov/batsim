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
use crate::faults::{SensorFault, SensorId};
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
///
/// # The soft rungs are Schmitt triggers, not bare comparators
///
/// Each derate rung trips at the chemistry's limit and then **holds** until the
/// measurement has fallen back past a release band. That band is the difference
/// between a protection device and an oscillator: with no band, cutting the current
/// removes the load, the measured voltage drops back under the limit within one
/// sample, the rung releases, and the pack chatters. See [`Self::v_release_band_v`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtectionConfig {
    /// How far past the chemistry's `v_max`/`v_min` \[V\] a measured group voltage
    /// must go before the contactor latches open instead of the current being
    /// derated. Must be finite and `>= 0`.
    pub v_hard_margin_v: f64,
    /// How far past the chemistry's `t_max_k` \[K\] a measured probe temperature must
    /// go before the contactor latches open. Must be finite and `>= 0`.
    pub t_hard_margin_k: f64,
    /// How far a measured group voltage must fall back **below** `v_max` — or rise
    /// back **above** `v_min` — before the corresponding derate rung releases \[V\].
    /// Must be finite and `>= 0`.
    ///
    /// # Why this is not zero
    /// Zero reproduces a bare comparator exactly (`trip || (held && !release)`
    /// collapses to `trip` when the two thresholds coincide), and a bare comparator
    /// at the top of charge is a **two-step limit cycle**: one step admits the full
    /// derated current and pushes the group over `v_max`, the next derates to zero,
    /// the load comes off and the reading falls back under, repeat. It is not
    /// cosmetic — with the refused-charge heat term of `docs/plans/energy-hole.md`
    /// an admitted step deposits 73.6 W where it used to deposit 1.3 W, and a 42 %
    /// duty cycle on that walks a protected pack to its temperature limit.
    ///
    /// # How to size it, which is not what it looks like
    /// The obvious quantity to clear is the loaded-to-idle swing in the measured group
    /// voltage — `i_derated · (R0 + Σ R_rc)` per cell, **39.9 mV** on the shipped LFP
    /// file at its derated 2.30 A per cell. **That is the wrong quantity**, and a band
    /// sized against it does essentially nothing: a sweep of the shipped LFP overcharge
    /// scenario admits 2461 of 6000 steps at a band of 0, 2461 at 20 mV and 2454 at
    /// 40 mV, then falls off a cliff to 625 at 60 mV and 447 at 80 mV.
    ///
    /// The cliff is at `v_max − OCV(1.0)`, which for that file is `3.65 − 3.60` = **50
    /// mV exactly**. The rung releases when the *rested* group voltage falls below
    /// `v_max − band`, and a saturated pack rests at its own `OCV(1.0)`; so while the
    /// band is under that gap the reading drops through the release threshold every
    /// time the load comes off, whatever the band is, and the cycle re-arms. The band
    /// must exceed **the distance between the protection threshold and the cell's own
    /// full-charge open-circuit voltage** — not the load line.
    ///
    /// **Provenance for the default (0.08 V):** that sweep. 80 mV is where the run
    /// saturates (447 steps, and 450 at 100 mV and 443 at 150 mV are the same answer),
    /// giving 60 % margin on the shipped LFP's 50 mV gap. A chemistry whose `v_max`
    /// sits further above its `OCV(1.0)` than this needs a larger band, and will
    /// chatter until it gets one — the default is sized against a shipped file, not
    /// derived from one.
    ///
    /// # What it costs
    /// The band is also how much charge the pack declines to take: charging stops when
    /// the *rested* group voltage reaches `v_max − band`, not when the loaded one
    /// reaches `v_max`. On that same scenario the final SOC goes from 0.999846 at a
    /// band of 0 to 0.999315 at 80 mV. Setting it to `0.0` restores the bang-bang
    /// behaviour bit-for-bit, which makes the contrast a scenario can show rather than
    /// something only this doc comment asserts.
    #[serde(default = "default_v_release_band_v")]
    pub v_release_band_v: f64,
    /// How far a measured probe temperature must fall back below `t_max_k` — or rise
    /// back above `t_charge_min_k` — before the corresponding derate rung releases
    /// \[K\]. Must be finite and `>= 0`. See [`Self::v_release_band_v`].
    ///
    /// **Provenance for the default (2.0 K):** unlike the voltage band there is no
    /// measured chatter to size this against — a pack at its temperature limit is on a
    /// thermal time constant of minutes, so the rung does not oscillate at the step
    /// rate no matter how it is written. 2 K is a placeholder in the sense
    /// `CLAUDE.md` allows: order-of-magnitude only, chosen as a conventional thermostat
    /// deadband, and labelled rather than left to look measured.
    #[serde(default = "default_t_release_band_k")]
    pub t_release_band_k: f64,
}

/// See [`ProtectionConfig::v_release_band_v`].
fn default_v_release_band_v() -> f64 {
    0.08
}

/// See [`ProtectionConfig::t_release_band_k`].
fn default_t_release_band_k() -> f64 {
    2.0
}

/// Which soft protection rungs are currently *held* by their release band.
///
/// One bool per rung rather than one per limit, because the two temperature rungs
/// respond in opposite directions (`ot` derates both ways, `ut` inhibits charge only)
/// and would otherwise share a latch that means two different things.
///
/// This is snapshot state, not a cache: a rung held at the instant a snapshot is taken
/// is still held after the restore, and a pack that forgot would admit one more
/// full-current step — the very step the band exists to prevent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
struct SoftLatches {
    /// Over-voltage: charge inhibited.
    ov: bool,
    /// Under-voltage: discharge inhibited.
    uv: bool,
    /// Over-temperature: both directions inhibited.
    ot: bool,
    /// Under-temperature: charge inhibited (plating, not heat).
    ut: bool,
}

/// One Schmitt-trigger rung: trip on `trip`, hold until `release`.
///
/// `trip` and `release` are mutually exclusive for any band `>= 0`, so a zero band
/// makes `!release` identical to `trip` and the whole expression collapses to the bare
/// comparator this replaced. That is what makes a zero band bit-for-bit the old
/// behaviour rather than approximately it.
fn rung(held: bool, trip: bool, release: bool) -> bool {
    trip || (held && !release)
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
///
/// The decision is a **Schmitt trigger**, not a bare comparator — close above
/// [`Self::v_threshold_v`], reopen only below `v_threshold_v − v_release_band_v` — and it
/// is made once per sampled frame in `Bms::update_bleed_latches`, never in the read that
/// the solve uses. Without the band it could not be otherwise stable: the bleed current
/// flows through the group's own resistance, so **closing the switch is itself what pulls
/// the reading back under the threshold**. See [`Self::v_release_band_v`].
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
    /// How far a measured group voltage must fall back **below** [`Self::v_threshold_v`]
    /// before that group's bleed switch opens again \[V\]. Must be finite and `>= 0`.
    ///
    /// # Why this is not zero
    /// Zero reproduces a bare comparator exactly — the switch closes on
    /// `v > v_threshold_v` and opens the moment it is not — and a bare comparator here
    /// is a **two-step limit cycle**, because closing the switch is itself what pushes
    /// the reading back under the threshold. Measured on a 1S1P fixture charged at
    /// 0.05 A against a 33 Ω bleed drawing 0.104 A: **5999 flips in 6000 steps at
    /// `dt` = 1 s, and 59999 in 60000 at `dt` = 0.1 s** — the same 6000 s of simulated
    /// time. A flip count that tracks the step rate is the definition of numerical
    /// chatter rather than physics.
    ///
    /// # How to size it, which is *not* how [`ProtectionConfig::v_release_band_v`] is sized
    /// The band must clear the reading's response to the switch's own action. For
    /// protection that is `v_max − OCV(1.0)`, because tripping removes the external
    /// load entirely and the reading falls back to a rested voltage the cell cannot
    /// exceed. A bleed switch has no such pin: opening it returns the reading to
    /// wherever the group actually sits, so the quantity to clear is the **bleed's own
    /// load line**, `I_bleed · (R0 + Σ R_rc)` per cell — and the *settled* value, not
    /// the instantaneous `I_bleed · R0`, because hysteresis lengthens the closed dwell
    /// into exactly the regime where the RC pairs finish developing. On the fixture
    /// above that is 2.1 mV instantaneous growing to 4.2 mV settled.
    ///
    /// Two consequences that `ProtectionConfig`'s band does not have:
    ///
    /// * **It cannot be derived in code.** The BMS knows `R_bleed` and the measured
    ///   voltage, so it knows `I_bleed` — but the group resistance is ground truth, and
    ///   the BMS never reads ground truth (see the module docs). Hence a configured
    ///   voltage rather than a computed one.
    /// * **It is not a property of the chemistry alone.** The load line scales as
    ///   `1/parallel` and with `1/R_bleed`, so a band sized for a 1P string is roughly
    ///   an order of magnitude larger than a 10P pack needs, and a stiffer bleed
    ///   resistor needs more. `soh_resistance` also grows the load line over pack life,
    ///   so a margin measured on new cells shrinks as the pack ages.
    ///
    /// **Provenance for the default (10 mV):** a sweep of the parked fixture over four
    /// `(parallel, R_bleed)` combinations, each run at `dt` = 1 s and `dt` = 0.1 s across
    /// the same 6000 s. The ratio of the two flip counts is the reading — 10 while the
    /// switch is chattering, 1 once it is not — and it falls off a cliff at the load
    /// line every time:
    ///
    /// | parallel | `R_bleed` | `I_bleed · (R0 + Σ R_rc)` | measured cliff |
    /// | --- | --- | --- | --- |
    /// | 1 | 33 Ω | 3.1 mV | between 2 and 4 mV |
    /// | 4 | 33 Ω | 0.78 mV | between 0 and 2 mV |
    /// | 10 | 33 Ω | 0.31 mV | between 0 and 2 mV |
    /// | 1 | 5 Ω | 20.6 mV | between 10 and 20 mV |
    ///
    /// 10 mV clears the first row by 3.2×. Note the last row: the shipped default is
    /// **not** enough for a 5 Ω bleed on a 1P string, which will chatter until it is
    /// raised. Like [`ProtectionConfig::v_release_band_v`], this number is sized against
    /// a fixture rather than derived from one — but unlike that one it is not even a
    /// property of the chemistry file, so a pack that changes `bleed_r_ohms` or its
    /// parallel count is changing what this has to clear.
    ///
    /// # What it costs
    /// The band is extra depth of discharge on the bled group: the switch keeps
    /// bleeding until the reading reaches `v_threshold_v − band` rather than
    /// `v_threshold_v`, so a larger band overshoots the balance target. Setting it to
    /// `0.0` restores the bang-bang behaviour bit-for-bit, which makes the contrast
    /// something a scenario can show.
    #[serde(default = "default_bleed_release_band_v")]
    pub v_release_band_v: f64,
}

/// See [`BalancingConfig::v_release_band_v`].
fn default_bleed_release_band_v() -> f64 {
    0.010
}

/// What the BMS is allowed to know and how badly its sensors lie.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BmsConfig {
    /// Passive balancing policy, or `None` for a BMS that never balances.
    ///
    /// `#[serde(default)]` for the same reason [`crate::PackConfig`]'s own optional
    /// slots carry it: TOML has no null, so without it "a BMS that never balances"
    /// would be a state a scenario file could not express at all. Omission means off,
    /// matching every other optional component. Deserialization-only — the field is
    /// still always written, so no snapshot layout changes.
    #[serde(default)]
    pub balancing: Option<BalancingConfig>,
    /// Protection policy, or `None` for a **monitor-only** BMS.
    ///
    /// `None` still estimates SOC and reports sensor readings, it just never clamps a
    /// demand — the BMS watches the pack drive itself past its limits. That is a
    /// distinct teaching case from having no BMS at all (where there is no estimate
    /// and no instrumentation either).
    ///
    /// `#[serde(default)]`: see [`Self::balancing`].
    #[serde(default)]
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
/// Voltages and temperatures are otherwise *exact* readings of the true state at the
/// probe positions — the always-on error is in the current sensor and in the spatial
/// under-sampling of the probes. Injected sensor faults ([`crate::faults::Fault::SensorStuck`],
/// [`crate::faults::Fault::SensorOffset`]) corrupt this frame on top of that, which is
/// why it is a distinct type rather than a handful of fields on [`Bms`].
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
    /// Which soft derate rungs are currently held by their release band.
    ///
    /// `#[serde(default)]` so that a snapshot written before the bands existed
    /// deserializes as "nothing held", which is what a pack that never had hysteresis
    /// was. The [`crate::SNAPSHOT_VERSION`] check still refuses such a blob — this is
    /// belt-and-braces on a field whose absence has a correct reading, not an
    /// exemption from the bump.
    #[serde(default)]
    latches: SoftLatches,
    /// Which groups' bleed switches are currently *held* closed by their release band,
    /// in series order. Empty when balancing is disabled.
    ///
    /// Snapshot state for the same reason [`Self::latches`] is: a switch held at the
    /// instant a snapshot is taken is still held after the restore, and a pack that
    /// forgot would re-derive it from a bare comparator — which is the chatter this
    /// band exists to stop.
    ///
    /// `#[serde(default)]` reads a pre-band snapshot as "nothing held"; the
    /// [`crate::SNAPSHOT_VERSION`] check still refuses such a blob (see
    /// `tests/snapshot_version.rs` for why that is the only thing which does).
    #[serde(default)]
    bleed_held: Vec<bool>,
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
        // Seed the bleed latches from the initial frame rather than from "nothing
        // held". The latch is only recomputed when a frame is sampled, so an unseeded
        // pack whose first frame is already above the threshold would delay its first
        // bleed by one step — and that one step is the whole difference between a band
        // of zero and today's bare comparator. Seeding is what makes `0.0` reproduce
        // the old trajectory bit-for-bit, which is the control every test here leans on.
        let bleed_held = config.balancing.map_or_else(Vec::new, |bal| {
            frame
                .v_group
                .iter()
                .map(|&v| v > bal.v_threshold_v)
                .collect()
        });
        Self {
            soc_est: (initial_soc + config.initial_soc_error).clamp(0.0, 1.0),
            rest_time_s: 0.0,
            config,
            frame,
            contactor_open: false,
            latches: SoftLatches::default(),
            bleed_held,
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
    /// anywhere".
    ///
    /// This is a **pure read** of the latch state, and must stay one. The switch
    /// decision itself lives in [`Self::update_bleed_latches`], which runs once per
    /// sampled frame; deciding here instead would mutate the BMS on a zero-length probe
    /// step, and would also make the decision depend on how many times the pack's
    /// nonlinear iteration happened to call it. As it stands a probe step reports
    /// exactly the bleed the next real step will carry.
    pub(crate) fn bleed_conductances(&self, out: &mut Vec<f64>) {
        out.clear();
        let Some(bal) = self.config.balancing else {
            return;
        };
        let g = 1.0 / bal.bleed_r_ohms;
        out.extend(
            self.bleed_held
                .iter()
                .map(|&held| if held { g } else { 0.0 }),
        );
    }

    /// Re-decide every group's bleed switch from the frame just sampled.
    ///
    /// Called once per sampled frame, immediately after [`Self::corrupt_sensors`], so
    /// the balancer switches on what its sensors say — including what an injected
    /// sensor fault made them say. A `dt` of zero samples no frame and so moves no
    /// switch.
    ///
    /// Each switch is the same Schmitt trigger the protection rungs use: close above
    /// [`BalancingConfig::v_threshold_v`], reopen only below
    /// `v_threshold_v − v_release_band_v`. At a band of zero the two thresholds
    /// coincide and [`rung`] collapses to the bare comparator this replaced.
    pub(crate) fn update_bleed_latches(&mut self) {
        let Some(bal) = self.config.balancing else {
            self.bleed_held.clear();
            return;
        };
        // A frame is always `series` long, but resize rather than assume it: this is
        // also the path a restored snapshot takes on its first step.
        self.bleed_held.resize(self.frame.v_group.len(), false);
        let release = bal.v_threshold_v - bal.v_release_band_v;
        for (held, &v) in self.bleed_held.iter_mut().zip(self.frame.v_group.iter()) {
            *held = rung(*held, v > bal.v_threshold_v, v <= release);
        }
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
    ///
    /// The soft rungs carry hysteresis ([`ProtectionConfig::v_release_band_v`]), so
    /// this **mutates** the BMS beyond the contactor latch: it is called once per
    /// solve pass, and the pack's nonlinear iteration calls it on every pass. That is
    /// safe for the same reason the contactor latch is — a rung's trip and release
    /// conditions read [`Self::frame`], which no pass touches, so every pass reaches
    /// the same verdict and re-deciding it is idempotent.
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
        // Each rung below is a Schmitt trigger: it trips at the chemistry's limit and
        // holds until the measurement has come back past the release band. A rung with
        // no sensor to read is left exactly as it was rather than being cleared — a
        // BMS that lost a probe has not learnt that the condition went away.
        let v_band = prot.v_release_band_v;
        let t_band = prot.t_release_band_k;
        if let Some(v_lo) = self.frame.min_group_v() {
            self.latches.uv = rung(
                self.latches.uv,
                v_lo <= limits.v_min,
                v_lo > limits.v_min + v_band,
            );
        }
        if self.latches.uv {
            i_discharge_max = 0.0; // any further discharge digs the hole deeper
            flags |= EventFlags::UV;
        }
        if let Some(v_hi) = self.frame.max_group_v() {
            self.latches.ov = rung(
                self.latches.ov,
                v_hi >= limits.v_max,
                v_hi < limits.v_max - v_band,
            );
        }
        if self.latches.ov {
            i_charge_max = 0.0;
            flags |= EventFlags::OV;
        }
        if let Some(t_hi) = self.frame.max_probe_k() {
            self.latches.ot = rung(
                self.latches.ot,
                t_hi >= limits.t_max_k,
                t_hi < limits.t_max_k - t_band,
            );
        }
        if self.latches.ot {
            // Too hot for either direction: all current makes more heat.
            i_discharge_max = 0.0;
            i_charge_max = 0.0;
            flags |= EventFlags::OT;
        }
        if let Some(t_lo) = self.frame.min_probe_k() {
            self.latches.ut = rung(
                self.latches.ut,
                t_lo < limits.t_charge_min_k,
                t_lo >= limits.t_charge_min_k + t_band,
            );
        }
        if self.latches.ut {
            // Charge inhibit: plating risk, not a heat problem, so discharge
            // stays available.
            i_charge_max = 0.0;
            flags |= EventFlags::UT;
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

    /// Corrupt the freshly sampled frame with any active injected sensor faults.
    ///
    /// Called immediately after [`Self::sample`], and deliberately **after** it: an
    /// injected fault is the last word, so a stuck sensor reads exactly its stuck
    /// value rather than that value plus this step's noise. The noise draw still
    /// happens (it is inside `sample`), which is the property that matters for
    /// determinism — injecting a sensor fault does not shift the RNG stream, so every
    /// other draw in the trajectory stays where it was.
    ///
    /// An out-of-range sensor index is ignored rather than panicking. Indices are
    /// validated when the fault is scheduled, so this is the same belt-and-braces the
    /// temperature-probe read uses: `step` must never panic.
    pub(crate) fn corrupt_sensors(&mut self, faults: &[SensorFault]) {
        for fault in faults {
            let slot = match fault.sensor {
                SensorId::GroupVoltage(g) => self.frame.v_group.get_mut(g as usize),
                SensorId::TempProbe(i) => self.frame.temp_probe_k.get_mut(i as usize),
                SensorId::PackCurrent => Some(&mut self.frame.i_pack_a),
            };
            if let Some(slot) = slot {
                *slot = fault.corrupt(*slot);
            }
        }
    }
}
