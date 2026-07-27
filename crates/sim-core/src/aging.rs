//! Semi-empirical aging: calendar fade, cycle fade, and the resistance growth that
//! comes with both.
//!
//! # Two mechanisms, one state of health
//!
//! A cell loses capacity for two reasons, and the model keeps them separate because
//! they integrate differently:
//!
//! * **Calendar fade** happens because the cell exists. It grows as `√t` — fast at
//!   first, then ever slower — with an Arrhenius temperature dependence and a SOC
//!   stress factor (storage at high SOC is worse). A cell sitting on a shelf ages.
//! * **Cycle fade** happens because charge moves. It is proportional to amp-hour
//!   throughput, weighted by how deep the excursion carrying that throughput is.
//!
//! A third mechanism lives next door in [`crate::plating`] and accumulates into the
//! same health state: charge carried while a cold cell is being charged hard plates
//! metallic lithium out of the electrolyte, and that inventory does not come back.
//! It is separate from cycle fade because it is not about how much charge moved but
//! about *the conditions it moved under*, and it is only reachable through this
//! module's sub-clock — a pack with `aging: None` reports the risk and pays nothing.
//!
//! Their sum is the capacity loss. `CLAUDE.md` forbids modelling that loss without
//! the matching **resistance growth**, so both feed one growth factor:
//! `soh_resistance = 1 + r_growth_per_capacity_loss · loss`. A pack that has faded
//! 10 % is not merely smaller, it is also harder to push current through, and that
//! is most of what an aged pack actually feels like.
//!
//! # Why the calendar increment is not a difference of square roots
//!
//! `√t` fade is path-dependent: an already-aged cell must keep aging *slowly*, and
//! moving it to a different temperature must not restart its clock. The state that
//! makes this work is the accumulated fade itself. Inverting it under the *current*
//! stress `k(T, soc)` gives an equivalent age `t_eq = (q_cal/k)²` — "how long this
//! cell would have had to sit under today's conditions to be this worn" — and the
//! step advances from there.
//!
//! The increment is then computed as
//!
//! ```text
//! dq = k · dt / (√(t_eq + dt) + √t_eq)
//! ```
//!
//! and deliberately **not** as the algebraically identical `k·(√(t_eq+dt) − √t_eq)`.
//! Subtracting two nearly-equal square roots cancels catastrophically exactly when
//! `t_eq` is large and `dt` small — which is precisely the fast-forward regime this
//! mechanism exists to serve. The rationalised form has no subtraction of like
//! quantities and is accurate at any `t_eq`.
//!
//! # The sub-clock
//!
//! Fade over one 10 ms simulation step is on the order of 1e-12 of a percent; adding
//! it to a number near 1.0 every step is both wasteful and numerically pointless.
//! Aging therefore runs on its own coarse clock ([`AgingConfig::sub_clock_period_s`]):
//! elapsed time accumulates, and when it reaches the period the whole accumulated
//! interval is applied in **one** update. That is what makes a months-long
//! fast-forward cheap — a `dt` of an hour ticks aging once, not 360 times.
//!
//! The accumulator is part of the snapshot, and a partial period is **carried**, not
//! dropped: snapshotting mid-period and restoring reproduces the original trajectory
//! exactly. It does mean the trajectory depends on the *sequence* of `dt` values a
//! client feeds, not only on their sum — the same accepted family of `dt`-dependence
//! as the BMS's one-step sampling lag (see [`crate::bms`]).

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::chem::{AgingParams, SafetyParams};
use crate::noise::uniform_unit;
use crate::plating::{plating_fade_increment, short_probability};

/// True iff `x` is strictly positive. NaN yields `false`, so `!is_positive(x)`
/// rejects NaN as well as non-positive values (and reads clear of clippy's
/// negated-comparison lint). Mirrors the helper in [`crate::chem`].
#[inline]
fn is_positive(x: f64) -> bool {
    x > 0.0
}

/// Molar gas constant `R` \[J/(mol·K)\], used by the Arrhenius temperature factor.
///
/// provenance: CODATA 2018 exact value (the mole and kelvin are both defined, so
/// this is exact by definition, not a measurement).
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.314_462_618_153_24;

/// Floor on a cell's capacity state of health.
///
/// A cell faded to literally zero capacity would divide by zero in coulomb counting,
/// so the state of health cannot reach it. The floor is set far below anything
/// physically meaningful — a cell at 1 % of nominal capacity is scrap, not a battery
/// — so it never shapes a plausible trajectory; it only keeps a pathological
/// configuration (an absurd fade coefficient, a multi-century fast-forward) finite
/// instead of producing NaN.
pub const MIN_SOH_CAPACITY: f64 = 0.01;

/// Whether aging runs, and how coarse its clock is.
///
/// Aging is a toggleable component per the design contract: `PackConfig.aging =
/// None` is a supported mode, not a degraded one, and it is the default. A pack
/// without it is a pack that never wears out, which is exactly what most electrical
/// and thermal scenarios want to hold fixed.
///
/// The *coefficients* are not here — they are chemistry, and live in the TOML
/// (`[aging]` → [`AgingParams`]). This type holds only policy, the same split as
/// [`crate::thermal::ThermalConfig`] against [`crate::chem::ThermalParams`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgingConfig {
    /// How much simulation time \[s\] accumulates before one aging update is applied.
    /// Must be finite and `>= 0`.
    ///
    /// `CLAUDE.md` suggests ~10 s, which is the default. Zero means "age on every
    /// step", which is legitimate (and convenient in tests) but buys nothing except
    /// arithmetic: the update is an integral over the elapsed interval either way.
    ///
    /// This is a **coarseness knob, not an accuracy knob** in the usual direction —
    /// a longer period does not accumulate error in the calendar term (the `√t`
    /// integral is exact over any interval at fixed stress), it only samples the
    /// temperature and SOC that set the stress less often.
    pub sub_clock_period_s: f64,
}

impl Default for AgingConfig {
    fn default() -> Self {
        Self {
            sub_clock_period_s: 10.0,
        }
    }
}

/// Pack-level aging state: the config plus the sub-clock accumulator.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aging {
    /// Configuration.
    config: AgingConfig,
    /// Simulation time accumulated since the last aging update \[s\].
    accum_s: f64,
}

impl Aging {
    /// Start a pack's aging clock at zero.
    pub(crate) fn new(config: AgingConfig) -> Self {
        Self {
            config,
            accum_s: 0.0,
        }
    }

    /// Add `dt` to the sub-clock; returns `Some(elapsed)` when an update is due,
    /// where `elapsed` is the *whole* accumulated interval since the last update.
    ///
    /// At most one update per call: a coarse `dt` produces a single update covering
    /// it, never a loop of period-sized ones. Callers must not call this with
    /// `dt == 0` — a zero-length step is an observation, not a tick.
    pub(crate) fn advance(&mut self, dt: f64) -> Option<f64> {
        self.accum_s += dt;
        if self.accum_s >= self.config.sub_clock_period_s {
            let elapsed = self.accum_s;
            self.accum_s = 0.0;
            Some(elapsed)
        } else {
            None
        }
    }

    /// The configured sub-clock period \[s\].
    #[must_use]
    pub fn sub_clock_period_s(&self) -> f64 {
        self.config.sub_clock_period_s
    }

    /// Simulation time accumulated since the last aging update \[s\].
    #[must_use]
    pub fn pending_s(&self) -> f64 {
        self.accum_s
    }
}

/// One cell's aging state: its two states of health and the accumulators behind
/// them.
///
/// This lives on the pack's `Cell` beside the static `capacity_factor` /
/// `r0_factor`, **not** inside [`crate::ecm::EcmState`]. SOH is the dynamic sibling
/// of those manufacturing factors, and keeping it out of the [`crate::CellModel`]
/// enum is what lets a later phase swap in a porous-electrode model without
/// inheriting the ECM's aging bookkeeping.
///
/// `soh_capacity` and `soh_resistance` are **derived**: they are recomputed from
/// `q_cal + q_cyc` at the end of every update and are never written independently.
/// They are stored rather than recomputed on read because they are consumed twice
/// per cell per step on the hot path.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CellAging {
    /// Capacity state of health in (0, 1\]: effective capacity = nominal × factor ×
    /// this. Starts at exactly `1.0`.
    pub(crate) soh_capacity: f64,
    /// Resistance growth factor, `>= 1`: effective `R0` = nominal × factor × this.
    /// Starts at exactly `1.0`.
    pub(crate) soh_resistance: f64,
    /// Capacity fraction lost to calendar fade so far. Inverted to an equivalent age
    /// on every update, which is what makes `√t` fade path-independent.
    q_cal: f64,
    /// Capacity fraction lost to cycle fade so far.
    q_cyc: f64,
    /// Capacity fraction lost to lithium plating so far — a third, independent
    /// mechanism (see [`crate::plating`]), kept separate from `q_cyc` for the same
    /// reason calendar and cycle fade are kept apart: they integrate differently and
    /// a reader should be able to attribute the damage.
    q_plating: f64,
    /// Charge throughput \[Ah\] since the last aging update, both directions counted.
    ah_since_tick: f64,
    /// The subset of `ah_since_tick` carried under plating conditions \[Ah\].
    ///
    /// An integral, not a state: an interval in which plating started and stopped
    /// partway is accounted exactly, because a cell that stops plating simply stops
    /// adding to this. That is why the short hazard is charged per amp-hour plated
    /// rather than per second spent cold.
    ah_plating_since_tick: f64,
    /// SOC at the last current reversal — the anchor the depth-of-discharge weight
    /// is measured from.
    soc_ref: f64,
    /// Whether the cell was last carrying discharge current. A reversal of this is
    /// what re-anchors `soc_ref`; a rest (exactly zero current) is not a reversal.
    discharging: bool,
}

impl CellAging {
    /// A fresh, unworn cell at `initial_soc`.
    pub(crate) fn new(initial_soc: f64) -> Self {
        Self {
            soh_capacity: 1.0,
            soh_resistance: 1.0,
            q_cal: 0.0,
            q_cyc: 0.0,
            q_plating: 0.0,
            ah_since_tick: 0.0,
            ah_plating_since_tick: 0.0,
            soc_ref: initial_soc,
            // Arbitrary but harmless: whichever way the pack actually starts moving,
            // the first current in the *other* direction re-anchors `soc_ref`, and a
            // pack that starts discharging never needed re-anchoring in the first
            // place (it is already anchored at its initial SOC).
            discharging: true,
        }
    }

    /// Fold one step's current into the cycle-fade accumulators.
    ///
    /// Called every step with the current the pack solve assigned this cell
    /// (`i_cell`), the current the whole pack carried (`i_pack`), and the cell's
    /// **start-of-step** SOC. Only the accumulation happens here; the fade itself is
    /// applied on the sub-clock ([`CellAging::tick`]).
    ///
    /// # Why the two currents play different roles
    ///
    /// Throughput is genuinely per-cell: a low-resistance cell in a parallel group
    /// carries more than its share and must be charged for it, and circulating
    /// current between mismatched cells is real charge moving through real electrodes.
    ///
    /// The *direction* is taken from the pack, and that is a deliberate correction to
    /// the obvious design. Anchoring the depth reference on each cell's own current
    /// sign looks more precise and is a trap: at rest, `i_cell` is **not** zero.
    /// A group's node voltage is a ratio of sums, so even a uniform pack circulates a
    /// rounding-sized current, and a pack with any manufacturing scatter circulates a
    /// real one — cells at different SOC push charge back and forth through each
    /// other. Half the cells in a group therefore see a sign flip the moment the load
    /// is removed, and an arbitrarily *small* reversal would discard the depth
    /// accounting for the large excursion in progress. Resting in the middle of a
    /// deep discharge would score it as two shallow ones. (Measured on a 1S2P pack
    /// with 5 % scatter: a ten-hour rest mid-discharge cut one cell's cycle fade by
    /// 5.8 %, for no physical reason.)
    ///
    /// On an unfaulted pack `i_pack` is exactly `0.0` under [`crate::Demand::Rest`] and
    /// under an open contactor, so resting does not re-anchor anything — which is what
    /// the model intends, and it needs no threshold to achieve. Discarding tiny
    /// reversals is what rainflow counting does properly; `CLAUDE.md` rules rainflow
    /// out for v1, and a deadband would need a magic constant with no provenance, so
    /// the honest v1 answer is that a half-cycle boundary is a **pack-level** event.
    ///
    /// Two things injected faults do to that, both intended:
    ///
    /// * An [`crate::faults::Fault::ExternalShort`] makes `i_pack` nonzero under
    ///   `Demand::Rest`, so a pack being drained by a short counts as discharging. It
    ///   *is* discharging — real charge is leaving the cells — and the half-cycle
    ///   anchor should follow the charge, not the demand.
    /// * A [`crate::faults::Fault::SoftInternalShort`] drains cells without moving
    ///   `i_pack` at all, so a self-discharging cell accrues throughput and deepens the
    ///   half-cycle in progress without ever starting a new one. That is the same
    ///   pack-level-boundary rule holding, not an exception to it.
    ///
    /// The cost is stated plainly: a cell being back-fed by its parallel neighbours
    /// while the pack as a whole discharges does not get a half-cycle boundary of its
    /// own. It inherits the pack's, and its depth is still measured from its own SOC.
    ///
    /// # The plating share
    ///
    /// `plating` is [`crate::plating::plating_risk`] evaluated for this cell on this
    /// step. When set, the step's throughput is added to the plating accumulator *as
    /// well as* the ordinary one — plating amp-hours are still amp-hours through the
    /// electrodes, so they keep paying ordinary cycle fade; the plating term is
    /// additional damage on top, not a reclassification.
    pub(crate) fn accumulate(
        &mut self,
        i_cell: f64,
        i_pack: f64,
        dt: f64,
        soc_before: f64,
        plating: bool,
    ) {
        let ah = i_cell.abs() * dt / 3600.0;
        self.ah_since_tick += ah;
        if plating {
            self.ah_plating_since_tick += ah;
        }
        if i_pack > 0.0 && !self.discharging {
            self.discharging = true;
            self.soc_ref = soc_before;
        } else if i_pack < 0.0 && self.discharging {
            self.discharging = false;
            self.soc_ref = soc_before;
        }
    }

    /// Apply one aging update covering `dt_age` seconds, using the cell's
    /// end-of-step temperature and SOC as the stress conditions for the whole
    /// interval.
    ///
    /// Returns `true` iff the plating hazard rolled a **new soft internal short** on
    /// this cell this tick; the caller owns the cell's shunt conductance and applies
    /// it. Returning the outcome rather than mutating from here keeps this type to
    /// health bookkeeping and leaves the one place that writes `shunt_g` inside
    /// [`crate::Pack`], next to where injected shorts land.
    ///
    /// # RNG contract
    ///
    /// At most **one** draw per cell per tick, and **no draw at all** unless the
    /// plating short probability is strictly positive — the same short-circuit
    /// `draw_factors` uses for zero scatter. Callers must invoke this in a fixed order
    /// over cells (series-major, parallel-minor) or the trajectory stops being a
    /// function of the seed. Both halves matter: without the short-circuit, merely
    /// *configuring* a chemistry with plating coefficients would shift every downstream
    /// draw on a pack that never gets cold.
    pub(crate) fn tick(
        &mut self,
        params: &AgingParams,
        safety: Option<&SafetyParams>,
        dt_age: f64,
        temp_k: f64,
        soc: f64,
        rng: &mut ChaCha8Rng,
    ) -> bool {
        let k = calendar_rate(params, temp_k, soc);
        self.q_cal += calendar_increment(k, self.q_cal, dt_age);

        // Depth of the half-cycle in progress: how far SOC has travelled since the
        // last reversal. See `cycle_increment` for what it weights.
        let dod = (soc - self.soc_ref).abs();
        self.q_cyc += cycle_increment(params, self.ah_since_tick, dod);
        self.ah_since_tick = 0.0;

        // Plating, if this chemistry can say what plating costs. The accumulator is
        // cleared unconditionally: a chemistry with no `[safety]` section still lets
        // the flag be raised, and letting untaxed plating throughput pile up would
        // mean adding the section later retroactively billed for it.
        let mut shorted = false;
        if let Some(s) = safety {
            let ah_plating = self.ah_plating_since_tick;
            self.q_plating += plating_fade_increment(s, ah_plating);
            let p = short_probability(s, ah_plating);
            if p > 0.0 {
                shorted = uniform_unit(rng) < p;
            }
        }
        self.ah_plating_since_tick = 0.0;

        let loss = self.q_cal + self.q_cyc + self.q_plating;
        self.soh_capacity = (1.0 - loss).max(MIN_SOH_CAPACITY);
        // Resistance keeps growing on the *unclamped* loss: a cell past the capacity
        // floor is a wreck, and reporting it as merely 1 % capacity but nominal
        // resistance would be the wrong kind of wrong.
        //
        // Plating-driven loss is coupled at the **same** ratio as calendar and cycle
        // loss. That is a v1 simplification and worth naming: plated lithium and the
        // fresh SEI that grows on it raise impedance disproportionately, so a real cell
        // that lost 1 % to plating is harder to push current through than one that lost
        // 1 % to shelf time. Splitting the coupling needs a second coefficient in
        // `[aging]` and a fit to justify it; until then the honest statement is that
        // this model under-reports the resistance cost of plating.
        self.soh_resistance = 1.0 + params.r_growth_per_capacity_loss * loss;
        shorted
    }
}

/// SOC stress factor by clamped linear interpolation over **uniformly spaced**
/// breakpoints spanning \[0, 1\].
///
/// The chemistry supplies bare numbers (`cal_soc_stress = [1.0, 1.0, 1.4]`) with no
/// SOC column, so the breakpoints are implied: `n` entries sit at
/// `0, 1/(n−1), …, 1`. Three entries therefore mean SOC 0.0 / 0.5 / 1.0, which is
/// what the shipped chemistries document. A single entry is a constant factor.
#[must_use]
pub fn soc_stress(stress: &[f64], soc: f64) -> f64 {
    let n = stress.len();
    if n == 0 {
        return 1.0;
    }
    if n == 1 {
        return stress[0];
    }
    #[allow(clippy::cast_precision_loss)]
    let pos = soc.clamp(0.0, 1.0) * (n - 1) as f64;
    // A NaN SOC saturates this cast to 0 rather than panicking (`step` must never
    // panic); the NaN then flows on through `frac` into the result, which is the
    // honest answer.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lo = (pos.floor() as usize).min(n - 2);
    #[allow(clippy::cast_precision_loss)]
    let frac = pos - lo as f64;
    stress[lo] + frac * (stress[lo + 1] - stress[lo])
}

/// Calendar fade rate `k` \[fraction of capacity per √s\] at the given conditions.
///
/// `k = cal_pre_exp · exp(−Ea/(R·T)) · soc_stress(soc)`, so accumulated fade under
/// constant conditions is `k·√t` with `t` in seconds. A non-positive or non-finite
/// temperature yields `0.0` rather than an infinite rate — `step` never panics, and
/// a pack that has left the physical domain should not be *rewarded* with infinite
/// aging.
#[must_use]
pub fn calendar_rate(params: &AgingParams, temp_k: f64, soc: f64) -> f64 {
    if !is_positive(temp_k) || !temp_k.is_finite() {
        return 0.0;
    }
    let arrhenius = (-params.cal_ea_j_per_mol / (GAS_CONSTANT_J_PER_MOL_K * temp_k)).exp();
    params.cal_pre_exp * arrhenius * soc_stress(&params.cal_soc_stress, soc)
}

/// Increment of calendar fade over `dt` seconds, given the rate `k` now and the fade
/// `q_cal` accumulated so far.
///
/// Inverts `q_cal = k·√t_eq` for the equivalent age under *today's* stress, then
/// advances `√t` from there — computed in the rationalised form
/// `k·dt/(√(t_eq+dt) + √t_eq)` so that no two nearly-equal square roots are ever
/// subtracted (see the module docs). `t_eq = 0` is fine: the denominator is `√dt`.
/// A non-positive `k` returns `0.0`, both because the inversion would divide by it
/// and because a zero rate means no aging.
#[must_use]
pub fn calendar_increment(k: f64, q_cal: f64, dt: f64) -> f64 {
    if !is_positive(k) || !is_positive(dt) {
        return 0.0;
    }
    let t_eq = (q_cal / k).powi(2);
    let denom = (t_eq + dt).sqrt() + t_eq.sqrt();
    k * dt / denom
}

/// Increment of cycle fade from `ah` amp-hours of throughput carried at
/// depth-of-discharge `dod`.
///
/// `dq = cyc_fade_per_ah · ah · dod^(cyc_dod_stress_exp − 1)`.
///
/// The exponent is offset by one on purpose. The literature parameterises cycle life
/// per *cycle*: a cycle of depth `D` costs `∝ D^exp`. That cycle moves `∝ D` amp-hours,
/// so the cost **per amp-hour** is `∝ D^(exp−1)`. With the usual `exp` slightly above
/// 1 this makes deep cycling modestly worse per amp-hour than shallow cycling, and it
/// makes `cyc_fade_per_ah` mean something concrete: the fade per amp-hour of a
/// full-depth cycle. `exp = 1` degenerates to pure throughput counting with no depth
/// dependence at all, weight exactly `1.0` including at `dod = 0`.
///
/// # Known approximation
/// `dod` is the depth of the half-cycle **in progress**, not of the completed one —
/// `CLAUDE.md` rules out rainflow counting for v1, and this is the cheap stand-in.
/// The consequence is that amp-hours early in a deep excursion are charged at the
/// shallow depth reached so far, so a deep cycle is scored slightly gently. With an
/// exponent near 1 the weight varies little over the range (`0.1^0.1 ≈ 0.79`), so the
/// error is small; it is nonetheless a systematic under-count, and the honest fix if
/// it ever matters is to credit throughput at reversal rather than as it happens.
#[must_use]
pub fn cycle_increment(params: &AgingParams, ah: f64, dod: f64) -> f64 {
    if !is_positive(ah) {
        return 0.0;
    }
    let exponent = params.cyc_dod_stress_exp - 1.0;
    let weight = if exponent == 0.0 {
        1.0
    } else if dod > 0.0 {
        dod.powf(exponent)
    } else {
        0.0
    };
    params.cyc_fade_per_ah * ah * weight
}
