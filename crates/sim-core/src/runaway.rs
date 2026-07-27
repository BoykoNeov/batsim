//! Thermal runaway: the second of the two **emergent** failure modes.
//!
//! Like [`crate::plating`], nothing here is scripted. A cell that runs away does so
//! because the ordinary thermal network was handed a heat source that grows faster
//! with temperature than the pack can shed it — the same integrator, the same
//! conductances, the same ambient. `CLAUDE.md` forbids animating this outcome, and
//! the test that matters is that the *neighbours* catch fire without anything ever
//! telling them to.
//!
//! # The reaction term
//!
//! Above [`SafetyParams::t_onset_k`] a cell releases heat from the decomposition of
//! its own materials, at a rate that is Arrhenius in temperature and first order in
//! the reactant it has left:
//!
//! ```text
//! Q_rxn(T, α) = P_onset · α · exp( −(Ea/R) · (1/T − 1/T_onset) )
//! ```
//!
//! with `α = energy_remaining / runaway_energy_j` the unreacted fraction. Three
//! properties this shape buys, all of them load-bearing:
//!
//! * **`P_onset` is directly interpretable.** At `T = T_onset` on an untouched cell
//!   the exponent is exactly zero and `Q_rxn` is exactly `P_onset` watts. A parameter
//!   that can be read off a plot is a parameter a placeholder can be sanity-checked
//!   against, which is what the `[aging]` pair could not be (see slice A's NMC
//!   rescale).
//! * **It is self-limiting.** `α` falls as the cell burns, so the release ends after
//!   exactly [`SafetyParams::runaway_energy_j`] joules however hot the cell got. The
//!   adiabatic ceiling is therefore `runaway_energy_j / heat_capacity_j_per_k`, and
//!   that ratio is the honest sanity check on the pair.
//! * **The exponent is a *relative* Arrhenius factor**, referenced to `T_onset`
//!   rather than to absolute zero. `exp(−Ea/(R·T))` alone is ~1e-13 at these
//!   temperatures, so the pre-exponential would have to be ~1e13 to mean anything and
//!   nobody could tell a plausible value from an absurd one by looking.
//!
//! # The threshold, and what it costs
//!
//! The term is switched on at `T >= T_onset` rather than being evaluated everywhere.
//! `CLAUDE.md` prescribes exactly this ("above `T_onset`, add an exothermic
//! self-heating term"), and it is what keeps a pack that never gets hot bit-for-bit
//! identical to one built before this module existed. The cost is a discontinuity:
//! the release jumps from `0` to `P_onset` watts as the cell crosses the line. Real
//! decomposition is continuous, so a fitted model would smooth this — but with a
//! plausible `Ea` the reaction is already negligible a few kelvin below onset, so the
//! jump is small in absolute terms and it makes "what temperature does this start
//! at?" answerable by a student pointing at one number.
//!
//! # Known limitation: ignition lags by one step
//!
//! Whether the reaction runs during a step is decided from **start-of-step**
//! temperatures. A cell that crosses onset *during* a step therefore begins reacting
//! at the start of the next one, so the ignition time carries an `O(dt)` error. This
//! is the same family of accepted imprecision as the BMS's one-step sensor lag and
//! the fault queue's `dt` granularity, and it is bought deliberately: the alternative
//! is scanning every cell's temperature between thermal sub-steps, which every pack
//! in the world would pay for so that a burning one could ignite a fraction of a step
//! sooner. Revisit if a scenario ever needs `dt` coarse enough that a cell can cross
//! onset and reach vent inside one step.
//!
//! # Venting, and what v1 does not model
//!
//! [`is_vented`] is a pure temperature threshold, and the per-cell `vented` bit it
//! sets is irreversible. What v1 does **not** model is the consequence: a real vented
//! cell ejects electrolyte and gas, loses mass and heat capacity, and stops being a
//! battery. Here it keeps conducting, keeps its heat capacity, and keeps whatever
//! charge it had. So `VENTED` is an honest report of a temperature having been
//! reached, and nothing more — a client should read it as "this cell is destroyed",
//! not as a change in how the cell behaves.

use serde::{Deserialize, Serialize};

use crate::aging::GAS_CONSTANT_J_PER_MOL_K;
use crate::chem::SafetyParams;

/// True iff `x` is strictly positive. NaN yields `false`, so every predicate below
/// answers "no" for a cell that has left the physical domain rather than crediting it
/// with a reaction. Mirrors the helpers in [`crate::plating`] and [`crate::aging`].
#[inline]
fn is_positive(x: f64) -> bool {
    x > 0.0
}

/// One cell's runaway state: how much of its exothermic budget is left, and whether
/// it has ever been hot enough to vent.
///
/// Deliberately **not** part of [`crate::aging::CellAging`]. Plating's consequences
/// ride the aging sub-clock and cost nothing on a pack with `aging: None`; runaway
/// cannot inherit that, because `CLAUDE.md`'s exit criterion is "BMS off → overcharge
/// → runaway → propagation" and says nothing about aging. A pack that cannot wear out
/// must still be able to burn.
///
/// Also not part of [`crate::ecm::EcmState`], for the reason slice A gave for SOH:
/// the [`crate::ecm::CellModel`] enum has to stay swappable for a porous-electrode
/// model in Phase 6, so it must not accumulate things that are not the cell *model*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellRunaway {
    /// Exothermic energy \[J\] this cell has not released yet.
    ///
    /// Starts at [`SafetyParams::runaway_energy_j`], or at `0.0` for a chemistry with
    /// no `[safety]` section — which is also the [`Default`], and is exactly the state
    /// of a cell that has already burned out. Both mean the same thing to
    /// [`reaction_power`]: no heat.
    pub energy_remaining_j: f64,
    /// Whether this cell has ever reached [`SafetyParams::t_vent_k`]. Irreversible: a
    /// cell that vents and then cools stays vented.
    pub vented: bool,
}

impl CellRunaway {
    /// A fresh cell with its full exothermic budget, or an inert one if the chemistry
    /// carries no `[safety]` section.
    #[must_use]
    pub fn new(safety: Option<&SafetyParams>) -> Self {
        Self {
            energy_remaining_j: safety.map_or(0.0, |s| s.runaway_energy_j),
            vented: false,
        }
    }
}

/// Exothermic self-heating power \[W\] of one cell at `temp_k` with
/// `energy_remaining_j` of its budget left.
///
/// Returns exactly `0.0` — no heat, and no reaction for the integrator to chase —
/// unless every one of these holds:
///
/// 1. the chemistry has a positive [`SafetyParams::runaway_power_w_at_onset`],
/// 2. the cell has budget left and the chemistry has a positive
///    [`SafetyParams::runaway_energy_j`] to measure it against,
/// 3. the cell is at or above [`SafetyParams::t_onset_k`].
///
/// Every comparison is written so a NaN temperature answers `false` and the result is
/// `0.0`: a pack that has gone non-finite should not additionally be set on fire, and
/// [`crate::Pack::step`] must never panic.
///
/// The result can be `+inf` for an absurd parameter set at an absurd temperature
/// (`exp` overflows). That is left to propagate rather than clamped — the sub-step
/// selection in [`crate::thermal`] degrades to its cap and asserts loudly in debug,
/// which is a better failure than a silently plausible number.
#[must_use]
pub fn reaction_power(params: &SafetyParams, temp_k: f64, energy_remaining_j: f64) -> f64 {
    let p_onset = params.runaway_power_w_at_onset;
    if !is_positive(p_onset) || !is_positive(energy_remaining_j) {
        return 0.0;
    }
    if !is_positive(params.runaway_energy_j) {
        return 0.0;
    }
    // `>=` and not `>`: at exactly onset the exponent is zero and the release is
    // exactly `p_onset`, which is the reading that makes the parameter interpretable.
    // NaN fails this and returns 0.0.
    let at_onset = temp_k >= params.t_onset_k;
    if !at_onset {
        return 0.0;
    }
    // Capped at 1: a cell cannot be more than fully unreacted, and this keeps a
    // hand-built state (or a restored snapshot from a file someone edited) from
    // amplifying the release rather than only shortening it.
    let alpha = (energy_remaining_j / params.runaway_energy_j).min(1.0);
    let exponent = -(params.runaway_ea_j_per_mol / GAS_CONSTANT_J_PER_MOL_K)
        * (1.0 / temp_k - 1.0 / params.t_onset_k);
    p_onset * alpha * exponent.exp()
}

/// Slope `∂Q_rxn/∂T` \[W/K\] of the reaction term at `temp_k`, holding the reactant
/// fraction fixed.
///
/// Differentiating the Arrhenius factor gives `Q · Ea/(R·T²)`, so this takes the
/// already-computed `q` rather than recomputing the exponential. It is the quantity
/// that retires [`crate::thermal`]'s "sub-step count depends on config alone"
/// property: near onset it is a fraction of a W/K and utterly negligible against the
/// linear conductances, and a few hundred kelvin later it is thousands of W/K. A
/// bound derived from config cannot cover both.
///
/// Returns `0.0` for a dead reaction or a non-finite temperature.
#[must_use]
pub fn reaction_power_slope(params: &SafetyParams, temp_k: f64, q: f64) -> f64 {
    if !is_positive(q) || !is_positive(temp_k) {
        return 0.0;
    }
    q * params.runaway_ea_j_per_mol / (GAS_CONSTANT_J_PER_MOL_K * temp_k * temp_k)
}

/// Whether a cell at `temp_k` is at or above the chemistry's vent threshold.
///
/// A pure observation about temperature: it holds in every thermal mode, including
/// [`crate::thermal::ThermalConfig::Isothermal`], where it reports a cell that was
/// *built* above the threshold. NaN answers `false`.
#[must_use]
pub fn is_vented(params: &SafetyParams, temp_k: f64) -> bool {
    temp_k >= params.t_vent_k
}
