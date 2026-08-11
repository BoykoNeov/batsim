//! The pack solve's damping line-search, and what it and the demand window buy together.
//!
//! Before these existed, `Pack::step`'s nonlinear solve was plain successive
//! substitution: each pass aggregated the tangents the last pass's probes took and
//! committed to whatever current that predicted. Where a tangent is shallower than the
//! secant it stands in for, the prediction overshoots — and the next pass, taking its
//! tangents further out, overshoots by more. Measured on a 1S1P LG M50 at 50 % SOC:
//!
//! | demand | reported current | reported terminal voltage |
//! | ------ | ---------------- | ------------------------- |
//! | `Voltage(-100)` on a `Dfn` | 1.6e101 A | -1.7e95 V |
//! | `Voltage(1e6)` on a `Dfn` | -6.4e105 A | 7.1e99 V |
//! | `Power(1e12)` on an `Spm` | 3.4e9 A | -1.66 V |
//!
//! Every one of those raised [`EventFlags::SOLVE_UNCONVERGED`], so no step was ever
//! silently wrong. What they were not is *usable*: 1e95 volts is not a number a client
//! can act on, and the whole point of a flag that says "treat this voltage as
//! approximate" is that the voltage is otherwise in the right neighbourhood.
//!
//! # Why here and not in `sim-core`
//! `sim-core` cannot read a file, so its porous-electrode tests run against fixture
//! chemistries with **decimated** tables — which flatters exactly what is measured here,
//! because a tangent iteration converges trivially on a curve that is already piecewise
//! linear. Every number below is only honest against a full parameter set. The
//! model-independent half of the story — that the demand window clamps a voltage target
//! at all, and clamps it to a *pack* window — is closed form and lives in
//! `sim-core/tests/demand_window.rs`.
//!
//! # The two mechanisms are separable and are separated here
//! The demand window bounds the *input*: no voltage target can ask for an operating
//! point outside `series × [v_min, v_max]`. The damping bounds the *iteration*: given
//! any input, including one the window cannot help with, the solve cannot run away.
//! `Power` has no window — a power demand that sags a pack below `v_min` is ordinary
//! physics, not a question worth refusing — so it is the demand that isolates damping,
//! and it is what the last test here uses.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The grid `dfn_cell.rs` runs on, and the one the Phase 7 spike's tables were measured
/// at.
const DFN: CellModelConfig = CellModelConfig::Dfn {
    shells: 10,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};
const SPM: CellModelConfig = CellModelConfig::Spm { shells: 10 };

/// This cell's declared window, from `[cell]` in the shipped file — Chen2020's own
/// cut-offs.
const V_MIN: f64 = 2.50;
const V_MAX: f64 = 4.20;

/// Nominal capacity \[A·h\], for reading the currents below as C-rates.
const CAP_AH: f64 = 5.153_198;

/// The largest current any assertion here will accept as "physical".
///
/// Not a tolerance and not tuned: it is **200 C** on this cell, which is about two
/// orders of magnitude past anything a scenario in this repo drives and still 7 orders
/// below the 1e9-and-up currents that motivated the safeguard. Any value in that wide
/// gap makes the same point, which is what a bound wants to be — the measured worst
/// case across the sweeps in `docs/plans/voltage-target-blowup.md` is 949 A, or 184 C.
const ABSURD_A: f64 = 200.0 * CAP_AH;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn config(model: CellModelConfig) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: 0.5,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        // No BMS: protection would clamp the current and hide what is under test.
        bms: None,
        aging: None,
        cell_model: model,
    }
}

fn pack(model: CellModelConfig) -> Pack {
    Pack::new(
        &config(model),
        parse_chemistry(LGM50).expect("the shipped LG M50 chemistry parses"),
    )
    .expect("a porous-electrode pack on a chemistry with [spm] and [dfn] builds")
}

// ---------------------------------------------------------------------------
// Voltage: the window and the damping together
// ---------------------------------------------------------------------------

/// The headline. Every one of these targets used to produce a current no cell could
/// carry — four of them past 1e12 A, one at 1.6e101 — and every one now lands on a
/// physical operating point and *converges*.
#[test]
fn no_voltage_target_however_absurd_produces_an_unphysical_current() {
    for (name, model) in [("Spm", SPM), ("Dfn", DFN)] {
        for target in [
            -1.0e30, -100.0, 0.0, 0.5, 1.0, 2.0, V_MIN, 3.0, V_MAX, 5.0, 7.0, 10.0, 1.0e6, 1.0e30,
        ] {
            let tele = pack(model).step(1.0, Demand::Voltage(target), &env());
            assert!(
                tele.i_actual.is_finite() && tele.v_terminal.is_finite(),
                "{name} Voltage({target}) reported a non-finite operating point: {tele:?}"
            );
            assert!(
                tele.i_actual.abs() < ABSURD_A,
                "{name} Voltage({target}) drew {} A ({:.0} C); the demand window should \
                 have bounded this to an operating point the cell can hold",
                tele.i_actual,
                tele.i_actual.abs() / CAP_AH
            );
            // The terminal voltage is an *end-of-step* read, so it is not the target —
            // but it cannot have left the window by more than the step itself moved it.
            assert!(
                tele.v_terminal > V_MIN - 0.5 && tele.v_terminal < V_MAX + 0.5,
                "{name} Voltage({target}) ended the step at {} V, outside the declared \
                 window by more than a step's worth of motion",
                tele.v_terminal
            );
            assert!(
                !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{name} Voltage({target}) failed to converge; with the target clamped \
                 into the window this is an ordinary operating point"
            );
        }
    }
}

/// An out-of-window target is not merely *bounded* — it is answered at the window edge,
/// identically to asking for the edge itself. Distinguishes a real clamp from a solve
/// that merely happens to stall somewhere finite.
#[test]
fn an_out_of_window_target_lands_exactly_on_the_edge() {
    for (name, model) in [("Spm", SPM), ("Dfn", DFN)] {
        for (edge, absurd) in [(V_MAX, 1.0e6), (V_MIN, -1.0e6)] {
            // `dt = 0`: the start-of-step operating point, with nothing moved.
            let at_edge = pack(model)
                .step(0.0, Demand::Voltage(edge), &env())
                .i_actual;
            let clamped = pack(model)
                .step(0.0, Demand::Voltage(absurd), &env())
                .i_actual;
            assert_eq!(
                clamped.to_bits(),
                at_edge.to_bits(),
                "{name}: Voltage({absurd}) gave {clamped} A but the window edge {edge} V \
                 gives {at_edge} A"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Power: the damping on its own
// ---------------------------------------------------------------------------

/// `Power` has no window, so this is the damping line-search with nothing else helping.
///
/// The two `Spm` cases are the measured ones: `Power(1e6)` reported **-4.16e5 A** and
/// `Power(1e12)` **3.39e9 A** before the search existed, and both are now a few hundred
/// amps. They still raise [`EventFlags::SOLVE_UNCONVERGED`] — a power demand four orders
/// past anything this cell can deliver has no operating point to find, and saying so is
/// the honest answer. What the flag now sits on top of is a current in the right
/// neighbourhood instead of one nine orders out.
#[test]
fn damping_bounds_a_power_demand_no_window_can_help() {
    for watts in [1.0e3, 1.0e6, 1.0e12] {
        let tele = pack(SPM).step(1.0, Demand::Power(watts), &env());
        assert!(
            tele.i_actual.is_finite(),
            "Spm Power({watts}) reported a non-finite current"
        );
        assert!(
            tele.i_actual.abs() < ABSURD_A,
            "Spm Power({watts}) drew {} A ({:.0} C); before the damping line-search this \
             reached 3.4e9 A, and bounding it is what the search is for",
            tele.i_actual,
            tele.i_actual.abs() / CAP_AH
        );
    }
}

/// A demand the pack *can* meet is met, and in the handful of passes the Phase 6 spike
/// measured — the guard against a safeguard that buys robustness by refusing to move.
///
/// This is the assertion that fails if the damping is ever made unconditional, or the
/// acceptance test loosened to something a zero-length step satisfies.
#[test]
fn an_ordinary_demand_still_converges_in_a_handful_of_passes() {
    for (name, model) in [("Spm", SPM), ("Dfn", DFN)] {
        for demand in [
            Demand::Current(5.0),
            Demand::Current(-2.0),
            Demand::Power(15.0),
            Demand::Power(-10.0),
            Demand::Voltage(3.9),
            Demand::Rest,
        ] {
            let tele = pack(model).step(1.0, demand, &env());
            assert!(
                !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{name} {demand:?} failed to converge on an ordinary operating point"
            );
            assert!(
                tele.solve_iterations <= 10,
                "{name} {demand:?} took {} passes; the spike measured 3 worst-case, and a \
                 damping search that engages on ordinary steps would show up here first",
                tele.solve_iterations
            );
        }
    }
}

/// A demand that cannot be solved must still leave the pack *usable*: the step reports,
/// and the steps after it describe a cell in the physical world.
///
/// # What this does not claim
/// That every unsolvable demand leaves a pristine cell. It does not, and the case that
/// does not is named rather than hidden: `Demand::Current` is unrefusable by every cell
/// model — that is its contract — so `Current(1e9)` on a `Dfn` drives the electrolyte
/// concentration to -9.2e4 mol/m³ in one step, and negative lithium is a state no later
/// step can recover from. That is reachable only by asking, in so many words, for a
/// current 54 000 times the cell's entire capacity in one second. It is not reachable
/// from any voltage target at any `dt` — see `docs/plans/voltage-target-blowup.md`,
/// which measures it — and so it is a documented limit of the unrefusable channel
/// rather than a solver defect.
#[test]
fn a_pack_survives_a_demand_it_cannot_meet() {
    for (name, model) in [("Spm", SPM), ("Dfn", DFN)] {
        let mut p = pack(model);
        // The worst voltage target there is, which the window turns into an ordinary one.
        p.step(1.0, Demand::Voltage(-1.0e30), &env());
        p.step(1.0, Demand::Voltage(1.0e30), &env());
        // Now rest it and check it comes back to somewhere a battery can be.
        let mut last = f64::NAN;
        for _ in 0..5 {
            last = p.step(1.0, Demand::Rest, &env()).v_terminal;
        }
        assert!(
            last > V_MIN - 0.5 && last < V_MAX + 0.5,
            "{name}: after two absurd voltage targets the pack rests at {last} V, which \
             is not a voltage a cell of this chemistry can hold"
        );
    }
}
