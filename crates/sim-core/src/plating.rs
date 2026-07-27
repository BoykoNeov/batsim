//! Lithium plating: the first of the two **emergent** failure modes.
//!
//! `CLAUDE.md` draws a hard line between injected faults ([`crate::faults`]), which a
//! client asks for, and emergent failures, which the engine *discovers*. Nothing here
//! is scripted. Every quantity below is read off the same electrical and thermal state
//! a healthy pack produces, compared against thresholds that live in the chemistry
//! (`[safety]` → [`SafetyParams`]).
//!
//! # The physics, and what v1 keeps of it
//!
//! Charging pushes lithium into the graphite anode. When the anode is cold its
//! intercalation kinetics are slow, so above some current the lithium arriving at the
//! surface cannot get in fast enough and deposits as metal instead. Two things follow,
//! and this module models both:
//!
//! * **Lost inventory.** Plated lithium is largely unrecoverable, so the charge that
//!   plated is capacity gone — a *separate, additive* fade mechanism alongside the
//!   calendar and cycle terms in [`crate::aging`].
//! * **Dendrites.** The deposit is not a smooth film; it grows filaments that can
//!   eventually reach through the separator. That is a soft internal short, and it is
//!   the one genuinely stochastic outcome in the engine's physics — see
//!   [`short_probability`].
//!
//! What v1 does **not** model is the continuum underneath: the real onset is a smooth
//! function of temperature, current, SOC and electrode design, not a corner. `CLAUDE.md`
//! prescribes a threshold ("charging below `t_plating_min` at C-rate above a
//! threshold"), and a threshold is what [`plating_risk`] implements. The cost is that a
//! cell at 0.499 C plates nothing and one at 0.501 C plates fully; the benefit is that
//! a student can see exactly which knob moved the pack across the line.
//!
//! # Why the C-rate is measured against the capacity the cell has *today*
//!
//! [`plating_risk`] divides by the cell's **effective** capacity — nominal × its
//! manufacturing factor × its capacity state of health. An aged cell therefore reaches
//! the plating C-rate at a lower absolute current, which is the real and well-attested
//! behaviour: a worn cell fast-charges into trouble that a new one shrugs off.
//!
//! That is a feedback path from aging back into aging, so it is worth stating why it
//! cannot run away. The C-rate enters as a **threshold**, not as a rate multiplier: the
//! fade is `plating_fade_per_ah · ah_plated` however far past the threshold the cell
//! is. Aging can therefore switch plating *on* for a given charge current, but it can
//! never make plating that is already happening go faster. And the amp-hours a full
//! charge moves *shrink* as capacity fades, so the fade per cold charge decreases with
//! age. Monotone, self-limiting, and pinned by
//! `plating.rs::repeated_cold_charging_fades_without_running_away`, which fast-forwards
//! cold cycles and checks the trajectory stays far above
//! [`crate::aging::MIN_SOH_CAPACITY`].
//!
//! # Where the consequences land
//!
//! Detection is per cell per step (the flag is an observation, so it is reported even
//! on a zero-length probe step). The *consequences* ride the aging sub-clock: throughput
//! carried under plating conditions accumulates on the cell and is consumed at the next
//! aging tick, which is where the fade is applied and where the short is rolled. Two
//! consequences of that placement:
//!
//! * Plating costs nothing on a pack with `aging: None`. The flag still tells the truth
//!   — the pack *is* plating — but a pack that cannot wear out has nowhere to put the
//!   damage. Turning aging on is what makes plating bite.
//! * The hazard is charged per amp-hour plated, not per second spent cold, so an
//!   interval in which plating stopped partway is accounted exactly: the accumulator is
//!   the integral, and a cell that stopped plating simply stopped adding to it.

use crate::chem::SafetyParams;

/// True iff `x` is strictly positive. NaN yields `false`, so `!is_positive(x)` rejects
/// NaN as well as non-positive values (and reads clear of clippy's negated-comparison
/// lint). Mirrors the helpers in [`crate::chem`] and [`crate::aging`].
#[inline]
fn is_positive(x: f64) -> bool {
    x > 0.0
}

/// True iff `x` is strictly negative — i.e. the cell is being charged, under the
/// discharge-positive convention. NaN yields `false`, same as [`is_positive`].
#[inline]
fn is_charging(x: f64) -> bool {
    x < 0.0
}

/// True iff `x` is strictly below `limit`. NaN yields `false`, so a cell whose
/// temperature has gone non-finite is not treated as cold.
#[inline]
fn is_below(x: f64, limit: f64) -> bool {
    x < limit
}

/// Whether one cell is charging hard enough, cold enough, to plate lithium.
///
/// Three conditions, all against **start-of-step** ground truth (the same state that
/// produced `i_cell`, so the answer is consistent with the heat and the currents
/// reported for the same step):
///
/// 1. the cell is charging (`i_cell < 0`, per the discharge-positive convention),
/// 2. its temperature is below [`SafetyParams::t_plating_min_k`],
/// 3. its C-rate exceeds [`SafetyParams::plating_c_threshold`].
///
/// `eff_capacity_ah` is the cell's capacity *today* — nominal × manufacturing factor ×
/// capacity state of health — for the reason given in the module docs.
///
/// Every comparison is written so that a NaN answers `false`. A pack that has left the
/// physical domain should not be credited with a plating event on top of whatever else
/// has gone wrong, and `step` must never panic.
#[must_use]
pub fn plating_risk(params: &SafetyParams, i_cell: f64, temp_k: f64, eff_capacity_ah: f64) -> bool {
    // Charging is negative current; a rest or discharge cannot plate.
    if !is_charging(i_cell) {
        return false;
    }
    if !is_below(temp_k, params.t_plating_min_k) {
        return false;
    }
    if !is_positive(eff_capacity_ah) {
        return false;
    }
    let c_rate = -i_cell / eff_capacity_ah;
    c_rate > params.plating_c_threshold
}

/// Capacity fraction lost to plating from `ah_plating` amp-hours carried under plating
/// conditions.
///
/// `dq = plating_fade_per_ah · ah_plating`. Deliberately **unweighted** by depth of
/// discharge, unlike [`crate::aging::cycle_increment`]: the loss here is lithium that
/// plated out of the cell, and it does not care how deep the excursion carrying it
/// happened to be.
#[must_use]
pub fn plating_fade_increment(params: &SafetyParams, ah_plating: f64) -> f64 {
    if !is_positive(ah_plating) {
        return 0.0;
    }
    params.plating_fade_per_ah * ah_plating
}

/// Probability in \[0, 1) that `ah_plating` amp-hours of plating produced a soft
/// internal short.
///
/// A Poisson hazard in *plated charge*: `p = 1 − exp(−λ·ah)`. That shape is what makes
/// the accumulation interval irrelevant — rolling once against 2 Ah is exactly as
/// likely to short as rolling twice against 1 Ah each — so the aging sub-clock period
/// does not quietly change how dangerous cold charging is.
///
/// Computed as `−expm1(−λ·ah)` rather than `1 − exp(−λ·ah)`: the same catastrophic
/// cancellation the calendar-fade increment avoids ([`crate::aging`]) would otherwise
/// bite here, where `λ·ah` is routinely ~1e-6 and the subtraction throws away most of
/// the significant digits.
///
/// Returns exactly `0.0` for a zero hazard or zero throughput — which is the property
/// the *caller* depends on, since a zero probability must consume no RNG draw at all.
#[must_use]
pub fn short_probability(params: &SafetyParams, ah_plating: f64) -> f64 {
    let lambda = params.plating_short_hazard_per_ah;
    if !is_positive(lambda) || !is_positive(ah_plating) {
        return 0.0;
    }
    -(-lambda * ah_plating).exp_m1()
}
