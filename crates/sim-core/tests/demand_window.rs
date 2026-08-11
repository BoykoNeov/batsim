//! A voltage demand is clamped to the pack's declared terminal-voltage window.
//!
//! `Demand::Voltage` asks the pack to *hold* a terminal voltage, and terminal voltage
//! has no asymptote in either direction — so before this existed, a target outside the
//! cell's range was met exactly, at whatever current the arithmetic demanded. On a
//! single-particle cell that is 108 megaamps for 7 V; on an equivalent circuit a 1e30 V
//! target reported 5e31 A and a 1.07e30 V terminal. `Pack::step` now clamps the target
//! to `series × [v_min, v_max]` before solving.
//!
//! # Why this file is equivalent-circuit only
//! The clamp is applied to the *demand*, above the solve and above every cell model, so
//! one model exercises the whole of it and the equivalent circuit is the one whose
//! answer is closed-form — `i = (E − V*)/R` — and therefore checkable by hand. That the
//! clamp also rescues the porous-electrode models from currents no damping could bound
//! is measured against shipped parameter sets in `sim-data/tests/solve_safeguard.rs`,
//! which cannot live here because `sim-core` may not read a file.
//!
//! # What is deliberately *not* asserted
//! That `v_terminal` equals the clamped target. It does not, and should not: telemetry
//! reports **end-of-step** state, so a step that ran 63 A for a second has moved the SOC
//! (and with it the OCV) before the voltage is read. Where a test needs the voltage the
//! solve actually held, it asks for it with a `dt = 0` probe — this repo's instantaneous
//! read — and that is what `an_in_window_target_is_held_exactly` does.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;

/// The declared window every assertion below is written against.
const V_MIN: f64 = 2.90;
const V_MAX: f64 = 3.50;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A flat-OCV cell, so that the operating point the clamp produces is arithmetic a
/// reader can redo: at any SOC the source is `OCV − Σ V_rc` behind `R0`.
fn chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "demand_window".into(),
            name: "Demand-window test cell".into(),
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
            volts: vec![2.60, 3.20, 3.60],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
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
        // No BMS: protection would clamp the *current* and hide the thing under test,
        // which is a clamp on the demand. With `None` the only limit in the step is the
        // one this file is about.
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn pack(series: u16) -> Pack {
    Pack::new(&config(series), chem()).expect("the fixture chemistry builds")
}

/// The current a `dt = 0` probe reports for `target` on a fresh `series`-cell pack.
///
/// Zero-length so nothing moves: the answer is the start-of-step operating point and
/// nothing else, which is what makes two of these comparable bit-for-bit.
fn probe_current(series: u16, target: f64) -> f64 {
    pack(series)
        .step(0.0, Demand::Voltage(target), &env())
        .i_actual
}

// ---------------------------------------------------------------------------
// Outside the window
// ---------------------------------------------------------------------------

/// A target above the window is answered at the window, not chased — and the further
/// outside it goes, the *less* it matters, which is the whole point.
#[test]
fn a_target_above_the_window_is_answered_at_the_window() {
    let at_limit = probe_current(1, V_MAX);
    for absurd in [V_MAX + 0.01, 4.2, 7.0, 1.0e6, 1.0e30, f64::MAX] {
        let clamped = probe_current(1, absurd);
        assert_eq!(
            clamped.to_bits(),
            at_limit.to_bits(),
            "Voltage({absurd}) gave {clamped} A where the window edge {V_MAX} V gives \
             {at_limit} A; a target above the window must be indistinguishable from the \
             window itself"
        );
    }
    // And it is a charge of a size a 2.5 A·h cell could actually see, rather than the
    // 5e31 A a 1e30 V target used to report.
    assert!(
        at_limit < 0.0 && at_limit.abs() < 100.0,
        "holding {V_MAX} V on a half-charged cell should be a modest charge, got \
         {at_limit} A"
    );
}

/// The same on the low side. `f64::MIN` is included because the clamp has to hold at
/// the extreme of the type, not merely at extreme *physics*.
#[test]
fn a_target_below_the_window_is_answered_at_the_window() {
    let at_limit = probe_current(1, V_MIN);
    for absurd in [V_MIN - 0.01, 2.0, 0.0, -100.0, -1.0e30, f64::MIN] {
        let clamped = probe_current(1, absurd);
        assert_eq!(
            clamped.to_bits(),
            at_limit.to_bits(),
            "Voltage({absurd}) gave {clamped} A where the window edge {V_MIN} V gives \
             {at_limit} A"
        );
    }
    assert!(
        at_limit > 0.0 && at_limit.abs() < 100.0,
        "holding {V_MIN} V on a half-charged cell should be a modest discharge, got \
         {at_limit} A"
    );
}

// ---------------------------------------------------------------------------
// Inside the window
// ---------------------------------------------------------------------------

/// Every in-window target is passed through untouched, and the evidence is that they
/// are all *different from each other* — a clamp that had swallowed them would collapse
/// them onto one current, which is exactly what the two tests above assert happens
/// outside.
#[test]
fn in_window_targets_are_passed_through_untouched() {
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for k in 0..=10 {
        let target = V_MIN + (V_MAX - V_MIN) * f64::from(k) / 10.0;
        let i = probe_current(1, target);
        for &(prev_t, prev_i) in &seen {
            assert!(
                (i - prev_i).abs() > 1.0e-9,
                "Voltage({target}) and Voltage({prev_t}) both gave {i} A; distinct \
                 in-window targets must give distinct operating points"
            );
        }
        seen.push((target, i));
    }
    // Monotone, and in the right direction: asking for a lower terminal voltage draws
    // more discharge current.
    assert!(
        seen.windows(2).all(|w| w[0].1 > w[1].1),
        "current should fall monotonically as the voltage target rises: {seen:?}"
    );
}

/// A target exactly on the window edge behaves continuously with the targets just
/// inside it — it is not swallowed by the clamp.
///
/// This is the case a CC-CV client actually issues: its policy holds exactly `v_max`.
/// The claim is *continuity at the edge*, not bit-identity with an unclamped build —
/// there is no unclamped build to compare against here, and a bit comparison against
/// one ULP inside would be comparing two genuinely different targets. What it rules out
/// is an off-by-one in the clamp that pushed the edge itself to the other bound, which
/// would show up as a discontinuity of the size of the whole window.
#[test]
fn a_target_exactly_on_the_edge_is_not_swallowed_by_the_clamp() {
    // `V_MAX − 1 ULP` is inside the window and so provably unclamped; the edge itself
    // must land continuously with it.
    for edge in [V_MIN, V_MAX] {
        let inside = if edge == V_MIN {
            f64::from_bits(edge.to_bits() + 1)
        } else {
            f64::from_bits(edge.to_bits() - 1)
        };
        let i_edge = probe_current(1, edge);
        let i_inside = probe_current(1, inside);
        // One ULP of target moves the current by about `1 ULP / R`, i.e. far below this.
        assert!(
            (i_edge - i_inside).abs() < 1.0e-9,
            "the window edge {edge} V gave {i_edge} A but one ULP inside gave \
             {i_inside} A; the edge is not clamped and must solve identically"
        );
    }
}

/// A zero-length probe still reports a solved operating point rather than nothing, on a
/// clamped target as much as on an ordinary one — `snapshot.rs` pins that promise for
/// every demand and the clamp must not have introduced a path that dodges it.
#[test]
fn a_clamped_target_still_reports_a_finite_solve() {
    for target in [1.0e30, -1.0e30, f64::MAX, f64::MIN] {
        let tele = pack(1).step(0.0, Demand::Voltage(target), &env());
        assert!(
            tele.i_actual.is_finite() && tele.v_terminal.is_finite(),
            "Voltage({target}) reported {tele:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// The window is a *pack* window
// ---------------------------------------------------------------------------

/// `Demand::Voltage` is a pack terminal voltage, so the window scales with the series
/// count. This is the assertion that catches a future edit clamping to the per-cell
/// window: on a 4S pack that would refuse every legitimate target it has.
#[test]
fn the_window_scales_with_the_series_count() {
    for series in [1u16, 2, 4] {
        let s = f64::from(series);
        // Just inside the pack window at both ends: unclamped, and therefore distinct.
        let lo = probe_current(series, s * V_MIN + 0.01);
        let hi = probe_current(series, s * V_MAX - 0.01);
        assert!(
            lo > hi,
            "{series}S: a target near the bottom of the pack window should discharge \
             harder than one near the top, got {lo} A and {hi} A"
        );
        // And a target one cell's worth *below* the pack window is clamped, which on
        // anything above 1S is a voltage that would look perfectly reasonable per cell.
        let below = probe_current(series, s * V_MIN - 0.5);
        let at = probe_current(series, s * V_MIN);
        assert_eq!(
            below.to_bits(),
            at.to_bits(),
            "{series}S: a target below the pack window must clamp to it"
        );
    }
    // The per-cell reading of a 4S target is the mistake this guards against: 3.05 V is
    // a fine per-cell target and a deep discharge for a 4S string, and it clamps.
    let four_s_percell = probe_current(4, 3.05);
    let four_s_window = probe_current(4, 4.0 * V_MIN);
    assert_eq!(
        four_s_percell.to_bits(),
        four_s_window.to_bits(),
        "3.05 V on a 4S pack is below the 11.60 V pack window and must clamp to it"
    );
}
