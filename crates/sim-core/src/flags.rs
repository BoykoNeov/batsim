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
        ///
        /// **This flag means two different things depending on the cell model, and
        /// the difference is not guessable from the name.** For an equivalent
        /// circuit, state really was discarded: the charge is gone from the coulomb
        /// count, and [`crate::Telemetry::i_rejected_a`] says how much. For a
        /// porous-electrode cell (`Spm`, `Dfn`) nothing is discarded — the particle
        /// keeps the lithium it was pushed and the flag reports only that the *SOC
        /// readout* has run past its window, so `i_rejected_a` stays zero. See
        /// [`crate::spm::advance`].
        const SOC_CLAMPED_HIGH = 1 << 0;
        /// SOC hit the lower clamp (0.0): the cell is being discharged past empty.
        ///
        /// **Unlike [`Self::SOC_CLAMPED_HIGH`], this one now means the same thing for
        /// every cell model** — the *SOC readout* has run past its window, and nothing
        /// was discarded. On a porous-electrode cell it always meant that. On an
        /// equivalent circuit it used to mean state really was destroyed, with
        /// [`crate::Telemetry::i_rejected_a`] measuring how much; since the reversal
        /// branch the charge is carried in [`crate::EcmState::soc_deficit`] instead and
        /// `i_rejected_a` stays zero here.
        ///
        /// What the flag is *for* is unchanged and is worth stating, because "nothing
        /// was discarded" reads like "nothing happened": below this point the cell's
        /// open-circuit voltage is falling toward `reversal.floor_v` and the terminal
        /// voltage goes negative. The pack is not delivering energy any more, it is
        /// absorbing it.
        ///
        /// # Expect to see this only with `bms: None`
        /// A configured BMS trips under-voltage long before a cell empties, and then
        /// derates: measured on the shipped protected pack driven at 40 A from
        /// `soc = 0.05`, [`Self::UV`] is raised at 50 s and this flag is **never**
        /// raised at all — the pack settles asymptotically at `soc = 0.0026` over the
        /// following fifteen minutes without passing zero. So voltage reversal is a
        /// property of the unprotected mode, which `CLAUDE.md` calls "a supported,
        /// interesting mode, not an error", rather than something a protected pack
        /// reaches. A client that never turns the BMS off will never see this.
        /// See `docs/plans/low-clamp-reversal.md`.
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
        /// A nonlinear solve hit its iteration cap without reaching its tolerance;
        /// the step used the last **accepted** iterate.
        ///
        /// "Accepted" is load-bearing. The pack's solve takes a pass only if it reduces
        /// the residual, backtracking towards the last accepted current until it does
        /// (see `pack::DAMPING_ATTEMPTS`), so the current this flag is raised
        /// over is bounded by the physics rather than by wherever an extrapolation
        /// happened to land. Before that existed, an unconverged step could report
        /// 1e101 A and 1e95 V — flagged, but not a number a client can do anything
        /// with.
        ///
        /// Only reachable with a nonlinear cell model — an all-equivalent-circuit
        /// pack is solved exactly on the first pass and can never raise this. It is
        /// a *numerical* event rather than a physical one, and it is a flag rather
        /// than an `Err` for the same reason every other event here is: `step` does
        /// not fail, it reports. A client seeing this should treat the step's
        /// voltages as approximate, not the pack as broken.
        ///
        /// # Two solves can raise it, and the flag does not distinguish them
        /// Through Phase 6 this meant exactly one thing: **the pack's** current solve
        /// failed to reconcile its cells' tangents ([`crate::pack::SOLVE_ITER_CAP`],
        /// [`crate::pack::SOLVE_TOL_V`]). Phase 7's [`crate::dfn`] cell has a Newton
        /// solve of its own, and it raises the same flag on the same terms
        /// ([`crate::dfn::NEWTON_ITER_CAP`], [`crate::dfn::NEWTON_TOL`]).
        ///
        /// Widening a flag's meaning is a cost, and it was taken deliberately over
        /// the alternative of an 18th [`crate::Telemetry`] field — which the Phase 7
        /// plan rules out, because the out-of-tree trajectory instrument enumerates
        /// those fields by name and would be blind to a new one. What a client loses
        /// is the ability to tell *which* solve struggled; what it keeps is the only
        /// thing it can act on, which is that some voltage this step is approximate.
        const SOLVE_UNCONVERGED = 1 << 12;
        /// At least one parallel group was solved to a node voltage outside the
        /// chemistry's declared window, `[v_min, v_max]`.
        ///
        /// Not an error and not a numerical event: the step's arithmetic is exact and
        /// its energy ledger balances. What the flag reports is that the *place* the
        /// solve landed is off the map the chemistry declares, which is the one thing
        /// a client naming a load cannot work out for itself in advance.
        ///
        /// # Which demands ask, and which do not
        /// [`crate::Demand::Power`] and [`crate::Demand::Current`] both raise it: both
        /// name a load and let the voltage fall where it falls. The power case is the
        /// starker one — asking for 1 kW does not tell you whether you are about to
        /// draw 5 A or six million — but a current demand is not the informed choice
        /// the first version of this flag assumed. `Current(40.0)` says what the
        /// *current* will be and nothing about where the terminal ends up, and on the
        /// shipped over-discharge scenario it puts an LFP cell below `v_min` at
        /// **199.0 s**, eight and a half seconds before [`Self::SOC_CLAMPED_LOW`] says
        /// anything at all.
        ///
        /// [`crate::Demand::Voltage`] is clamped to this same window before the solve
        /// (see `pack::step`) and cannot leave it. [`crate::Demand::Rest`] is excluded
        /// deliberately rather than by omission: an open-circuit pack below `v_min` is
        /// a reversed cell, [`Self::SOC_CLAMPED_LOW`] is raised for exactly that state
        /// and its own doc explains the terminal going negative, and a second flag on
        /// one condition is the overload this crate pays for elsewhere.
        ///
        /// # It is ground truth, and [`Self::UV`]/[`Self::OV`] are not
        /// On a protected pack this co-fires with `UV` or `OV`, and they are different
        /// statements rather than a duplicate. `UV` and `OV` are the BMS's verdict on
        /// what its *sensors* reported — one voltage per group, sampled a step late,
        /// with whatever offset or fault is injected into them. This flag is the
        /// engine's own view of where the solve put each group. They can disagree, and
        /// on a pack with a lying sensor they will. With `bms: None` — a supported
        /// mode per `CLAUDE.md`, and the one every measurement behind this flag was
        /// taken in — no `UV`/`OV` exists at all and this is the only report.
        ///
        /// # Where the pack *went*, not what was asked for
        /// The predicate reads the node voltage the solve produced, which is downstream of
        /// everything the BMS did to the demand on the way in. So a demand that asks for
        /// somewhere unreachable and is **derated back inside the window raises nothing at
        /// all** — the client's ask was impossible, and this flag will not be the thing
        /// that says so; [`Self::OC`] and the returned `i_actual` are. Measured on one
        /// scenario and one demand, with only the BMS switched between the two runs:
        /// `Current(40.0)` on `soft_short_under_a_lying_sensor.toml` raises this at
        /// 335.0 s with protection off, and never raises it with protection on.
        ///
        /// That is the useful reading rather than a limitation of it. A flag on the
        /// *request* would fire on every demand a protected pack successfully talked down,
        /// which is the ordinary business of a BMS and not an event.
        ///
        /// # Per group, not on the series sum
        /// The predicate is each group's own node voltage, because the pack terminal
        /// cannot see imbalance: one group at 2.4 V and another at 3.4 V sum to a
        /// terminal that divides back to a perfectly in-window 2.9 V. On a 1S pack the
        /// two readings are the same number.
        ///
        /// # On a power demand the two sides are not symmetric, and both raise it
        /// `P = V(i)·i` on a Thévenin source has a **maximum** on the discharge side, at
        /// `i = e/(2·r0)` where `V = e/2`. Ask for more and no operating point exists;
        /// `ecm::solve_current` snaps to that maximum, which is correct physics, and the
        /// flag is what says the demand was not met.
        ///
        /// On the charge side there is no maximum at all — `V` grows without bound as
        /// `i` goes negative — so *any* power is met exactly, at an operating point
        /// arbitrarily far outside the window. A 1e12 W charge on an LG M50 cell is
        /// answered with 6.3e6 A at 162 kV, correct to five digits, and before this flag
        /// existed a short enough step reported it with nothing raised at all:
        /// [`Self::SOLVE_UNCONVERGED`] cannot reach it (the equivalent circuit runs no
        /// iteration to fail) and [`Self::SOC_CLAMPED_HIGH`] only fired when the step
        /// happened to be long enough to fill the cell.
        ///
        /// # When the discharge side does *not* fire, which is narrow but real
        /// The snap lands outside the window whenever `e < 2·v_min`, and
        /// `e = OCV(soc) − Σ V_rc − η` is at most `OCV(1.0)` for a cell not carrying a
        /// negative overpotential into the step. Every shipped chemistry satisfies
        /// `OCV(1.0) < 2·v_min`, most tightly LFP at 3.60 against 4.00. So an LFP cell
        /// entering a huge discharge-power demand with more than 0.40 V of accumulated
        /// *charging* overpotential snaps to a max-power point inside its own window and
        /// this stays down. Deliberately not enforced by `ChemistryParams::validate`:
        /// the inequality decides when a flag fires, not whether a chemistry is
        /// physical. It bounds only the *power* arm: a current demand large enough to
        /// pull any cell below `v_min` raises this whatever the chemistry, because
        /// there is no snap and no maximum — the client named the current and the
        /// voltage went where the resistance put it.
        const OPERATING_POINT_OUT_OF_WINDOW = 1 << 13;
    }
}
