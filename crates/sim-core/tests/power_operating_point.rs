//! A power demand reports whether the operating point it landed on is on the map.
//!
//! `Demand::Power` is the one demand where the *engine* picks the operating point: asking
//! for 1 kW does not tell you whether you are about to draw 5 A or six million. Before
//! `EventFlags::POWER_OUT_OF_WINDOW` existed there was nothing that said which. On the
//! shipped LG M50, a `Power(-1e12)` step of 0.001 s answered with 6.3e6 A at 162 440 V and
//! raised **nothing at all** — `SOLVE_UNCONVERGED` cannot reach it, because an equivalent
//! circuit solves `r0·i² − e·i + P = 0` in closed form and runs no iteration to fail, and
//! the `SOC_CLAMPED_HIGH` visible at longer steps fires only because the step happened to
//! be long enough to fill the cell.
//!
//! # Why this file is equivalent-circuit only
//! Because the equivalent circuit is where the defect actually is — it is the model with
//! no iteration, and therefore the one the previous slice's safeguard structurally cannot
//! cover. It is also the model whose operating point is arithmetic a reader can redo, and
//! this fixture is chosen so they can: at 50 % SOC the source is `e = 3.20 V` behind
//! `R0 = 0.02 Ω`, so
//!
//! * the in-window band is `V ∈ [2.90, 3.50]`, i.e. `i ∈ [−15, +15] A`, i.e.
//!   `P ∈ [−52.5, +43.5] W`, and
//! * the max-power point is `i = e/(2·R0) = 80 A` at `V = e/2 = 1.60 V`, delivering
//!   `e²/(4·R0) = 128 W` — and 1.60 V is below `v_min`, which is why the discharge arm
//!   flags at all. See `docs/plans/power-operating-point.md` for the inequality
//!   (`e < 2·v_min`) that makes this true of every shipped chemistry rather than of this
//!   fixture only.
//!
//! # Everything here probes at `dt = 0`
//! `Telemetry::v_terminal` is an **end-of-step** read, and a step that ran six million
//! amps has moved the SOC underneath it. The flag is raised against the *solve's* own
//! operating point, so the assertions have to read the same instant the solve did, and a
//! zero-length probe is this repo's instantaneous read. The one test that deliberately
//! uses a real `dt` is the one about step length, and it says why.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

const CAP_AH: f64 = 2.5;

/// The declared per-cell window every assertion below is written against.
const V_MIN: f64 = 2.90;
const V_MAX: f64 = 3.50;

/// The fixture's source and resistance at 50 % SOC, from which every number in this file
/// is derived by hand rather than measured.
const E: f64 = 3.20;
const R0: f64 = 0.02;

/// `e²/(4·R0)` — the most this cell can deliver at any current, at any demand.
const P_MAX_W: f64 = E * E / (4.0 * R0);

/// The power at the bottom edge of the window: `V = v_min` at `i = (e − v_min)/R0`.
const P_AT_V_MIN: f64 = (E - V_MIN) / R0 * V_MIN;

/// The power at the top edge: negative, because holding a cell above its rest voltage is
/// a charge.
const P_AT_V_MAX: f64 = (E - V_MAX) / R0 * V_MAX;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A cell whose operating point is arithmetic: flat-ish OCV, one resistance, one RC pair
/// that starts at zero so the first probe sees `e = OCV(0.5)` exactly.
fn chem() -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "power_operating_point".into(),
            name: "Power operating-point test cell".into(),
            provenance: "solver test fixture — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: V_MAX,
            v_min: V_MIN,
            max_charge_c: 1.0,
            max_discharge_c: 2.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![2.60, E, 3.60],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

fn config(series: u16) -> PackConfig {
    PackConfig {
        series,
        parallel: 1,
        initial_soc: 0.5,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        // No BMS, and this is load-bearing rather than incidental: with protection
        // configured, an out-of-window operating point also raises `UV`/`OV`. With
        // `None` — a supported mode, and the one every measurement in the plan doc was
        // taken in — this flag is the *only* thing that reports it, which is the gap the
        // slice exists to close.
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn pack(series: u16) -> Pack {
    Pack::new(&config(series), chem()).expect("the fixture chemistry builds")
}

/// Did a zero-length power probe on a fresh `series`-cell pack leave the window?
fn flagged(series: u16, watts: f64) -> bool {
    pack(series)
        .step(0.0, Demand::Power(watts), &env())
        .flags
        .contains(EventFlags::POWER_OUT_OF_WINDOW)
}

// ---------------------------------------------------------------------------
// The band, by hand
// ---------------------------------------------------------------------------

/// Every power whose operating point is inside the declared window passes unflagged, and
/// the band's edges are where the arithmetic says they are: `−52.5 W` to `+43.5 W`.
#[test]
fn the_in_window_band_is_where_the_arithmetic_puts_it() {
    assert!(
        (P_AT_V_MIN - 43.5).abs() < 1.0e-12 && (P_AT_V_MAX + 52.5).abs() < 1.0e-12,
        "the fixture's band moved: {P_AT_V_MIN} W and {P_AT_V_MAX} W, expected 43.5 and \
         −52.5. Every threshold in this file is derived from those two."
    );
    for k in 0..=20 {
        let watts = P_AT_V_MAX + (P_AT_V_MIN - P_AT_V_MAX) * f64::from(k) / 20.0;
        assert!(
            !flagged(1, watts),
            "Power({watts}) lands inside [{V_MIN}, {V_MAX}] V and must not be flagged"
        );
    }
    // Rest is the degenerate member of the band, and reaches it by a different arm of
    // `solve_current` than the sweep above.
    assert!(!flagged(1, 0.0), "Power(0) is an in-window operating point");
}

/// Just outside either edge flags, and just inside does not. The window is inclusive, so
/// the edge itself belongs to the band.
#[test]
fn the_edges_are_inclusive_and_a_step_past_them_flags() {
    // A watt is far more than the ULP-scale slop in these products, and far less than the
    // 96 W width of the band.
    assert!(
        !flagged(1, P_AT_V_MIN - 1.0),
        "one watt inside the low edge"
    );
    assert!(
        !flagged(1, P_AT_V_MAX + 1.0),
        "one watt inside the high edge"
    );
    assert!(
        flagged(1, P_AT_V_MIN + 1.0),
        "one watt past the discharge edge leaves the window and must flag"
    );
    assert!(
        flagged(1, P_AT_V_MAX - 1.0),
        "one watt past the charge edge leaves the window and must flag"
    );
}

// ---------------------------------------------------------------------------
// The two arms are not symmetric, and both raise it
// ---------------------------------------------------------------------------

/// **Discharge is bounded by the cell.** Past the max-power point no operating point
/// exists, the closed form snaps to `e/(2·R0)`, and the demand is silently short — which
/// is what the flag now says out loud. The evidence that it snapped rather than solved is
/// that every demand past the peak returns the *same* current.
#[test]
fn past_the_max_power_point_the_demand_is_short_and_says_so() {
    let at_peak = pack(1).step(0.0, Demand::Power(P_MAX_W * 2.0), &env());
    assert!(
        (at_peak.i_actual - E / (2.0 * R0)).abs() < 1.0e-9,
        "an unmeetable discharge should snap to the max-power current {} A, got {} A",
        E / (2.0 * R0),
        at_peak.i_actual
    );
    for asked in [P_MAX_W + 1.0, 200.0, 400.0, 1.0e3, 1.0e6, 1.0e12] {
        let tele = pack(1).step(0.0, Demand::Power(asked), &env());
        let delivered = tele.i_actual * tele.v_terminal;
        assert!(
            delivered < asked - 0.5,
            "Power({asked}) delivered {delivered} W, which is not short — this test is \
             about demands the cell cannot meet"
        );
        assert!(
            tele.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Power({asked}) delivered only {delivered} W at {} V and raised {:?}; an \
             unmet power demand must not be silent",
            tele.v_terminal,
            tele.flags
        );
    }
}

/// **Charge is bounded by nothing.** `V` grows without limit as `i` goes negative, so
/// every one of these is met *exactly* — and at an operating point further and further
/// off the map. The assertion that the demand was met is what distinguishes this arm
/// from the one above: nothing here is failing, and the flag is not a failure report.
#[test]
fn an_absurd_charge_is_met_exactly_and_still_flagged() {
    for asked in [-1.0e3, -1.0e6, -1.0e9, -1.0e12] {
        let tele = pack(1).step(0.0, Demand::Power(asked), &env());
        let delivered = tele.i_actual * tele.v_terminal;
        assert!(
            (delivered - asked).abs() < asked.abs() * 1.0e-9,
            "Power({asked}) delivered {delivered} W; the charge arm has no maximum and \
             must be met exactly"
        );
        assert!(
            tele.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Power({asked}) held {} V on a {V_MAX} V cell and raised {:?}",
            tele.v_terminal,
            tele.flags
        );
    }
}

/// The headline row, and the one that pins why the old signals were not enough: at a step
/// short enough that nothing fills, `SOC_CLAMPED_HIGH` never fires and the equivalent
/// circuit never iterates, so before this flag the step reported six million amps with an
/// empty flag set.
///
/// This is the one test here that uses a real `dt`, because step length *is* the variable.
#[test]
fn the_step_length_no_longer_decides_whether_anything_is_reported() {
    for dt in [1.0, 0.1, 0.001, 1.0e-6] {
        let tele = pack(1).step(dt, Demand::Power(-1.0e12), &env());
        assert!(
            tele.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "a 1e12 W charge over {dt} s raised {:?}",
            tele.flags
        );
        assert!(
            tele.i_actual < -1.0e5,
            "a 1e12 W charge should draw an enormous current, got {} A",
            tele.i_actual
        );
    }
    // And the old signals really are absent at the short step, which is what made this
    // defect invisible: if this ever starts firing, the test above stops being evidence
    // that *this* flag is what reports it.
    let short = pack(1).step(1.0e-6, Demand::Power(-1.0e12), &env());
    assert!(
        !short.flags.contains(EventFlags::SOC_CLAMPED_HIGH)
            && !short.flags.contains(EventFlags::SOLVE_UNCONVERGED),
        "the short step raised {:?}; this test's premise is that neither of the \
         pre-existing signals reaches this case",
        short.flags
    );
}

// ---------------------------------------------------------------------------
// Only this demand
// ---------------------------------------------------------------------------

/// The same operating point, reached by a current demand, is **not** flagged — and this
/// is deliberate rather than an oversight. With a current demand the client chose the
/// point and knows it; with a power demand the engine chose it.
///
/// The comparison is exact: the current is read off the power probe and handed straight
/// back, so the two steps solve the identical point and differ only in what was asked.
#[test]
fn the_same_operating_point_from_a_current_demand_is_not_flagged() {
    for watts in [1.0e3, -1.0e12, P_MAX_W * 2.0] {
        let from_power = pack(1).step(0.0, Demand::Power(watts), &env());
        assert!(
            from_power.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Power({watts}) should be out of window for this comparison to mean anything"
        );
        let from_current = pack(1).step(0.0, Demand::Current(from_power.i_actual), &env());
        assert!(
            (from_current.v_terminal - from_power.v_terminal).abs() < 1.0e-9,
            "the two demands must reach the same point: {} V vs {} V",
            from_current.v_terminal,
            from_power.v_terminal
        );
        assert!(
            !from_current.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Current({}) reached the same {} V and must stay unflagged; only a power \
             demand raises this",
            from_power.i_actual,
            from_current.v_terminal
        );
    }
}

/// A `Demand::Voltage` cannot leave the window at all — it is clamped into it before the
/// solve — so it can never raise this either, however absurd the target.
#[test]
fn a_voltage_demand_can_never_raise_it() {
    for target in [1.0e30, -1.0e30, 0.0, f64::MAX] {
        let tele = pack(1).step(0.0, Demand::Voltage(target), &env());
        assert!(
            !tele.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Voltage({target}) raised {:?}; the demand window already fences this path",
            tele.flags
        );
    }
}

// ---------------------------------------------------------------------------
// The NaN arm
// ---------------------------------------------------------------------------

/// A NaN operating point is **not** in the window, and the predicate has to say so.
///
/// This is the arm that decides how the check is spelled. `!(v >= lo && v <= hi)` answers
/// `true` for a NaN; the form clippy's `nonminimal_bool` rewrites it into,
/// `v < lo || v > hi`, answers `false` and would let a NaN pass for an in-window point.
/// The engine has paid for exactly that once already, at `soc = 0`, where `x >= 1.0`
/// answering `false` for a NaN let the value reach a Thévenin source with no flag and no
/// failing test. `Demand::check_finite` exists, but it is a check the *caller* opts into,
/// so `step` can still be handed this.
#[test]
fn a_non_finite_power_demand_is_out_of_window_not_in_it() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let tele = pack(1).step(0.0, Demand::Power(bad), &env());
        assert!(
            tele.flags.contains(EventFlags::POWER_OUT_OF_WINDOW),
            "Power({bad}) reported {:?} at {} V; a point that cannot be shown to be \
             inside the window must be reported as outside it",
            tele.flags,
            tele.v_terminal
        );
    }
}

// ---------------------------------------------------------------------------
// It is a *pack* window
// ---------------------------------------------------------------------------

/// The window scales with the series count, so the whole band scales with it: a 2S pack's
/// edges are at exactly twice the watts, because both the source and the window double.
///
/// This is the assertion that catches an edit comparing against the per-cell window,
/// which on a multi-cell string would flag every legitimate power demand it has.
#[test]
fn the_band_scales_with_the_series_count() {
    for series in [1u16, 2, 4] {
        let s = f64::from(series);
        assert!(
            !flagged(series, P_AT_V_MIN * s - 1.0),
            "{series}S: {} W is inside the pack band and must not flag",
            P_AT_V_MIN * s - 1.0
        );
        assert!(
            !flagged(series, P_AT_V_MAX * s + 1.0),
            "{series}S: {} W is inside the pack band and must not flag",
            P_AT_V_MAX * s + 1.0
        );
        assert!(
            flagged(series, P_AT_V_MIN * s + 1.0),
            "{series}S: {} W is past the discharge edge and must flag",
            P_AT_V_MIN * s + 1.0
        );
    }
    // The per-cell reading is the mistake this guards against: 43.5 W is comfortably
    // inside a single cell's band and comfortably inside a 4S string's, and a check
    // written against the per-cell window would be wrong about one of them.
    assert!(
        !flagged(4, P_AT_V_MIN * 4.0 - 1.0),
        "a 4S pack's band reaches {} W; a per-cell comparison would flag it",
        P_AT_V_MIN * 4.0
    );
}
