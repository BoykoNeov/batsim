//! The heat a step really generated, rather than the heat its first instant was
//! generating.
//!
//! `docs/plans/step-mean-heat.md` is the note. The defect it closes is the last of the
//! three things a coarse `dt` used to cost, and the only one of the three that anything
//! in the repository reached: heat was solved once per step from start-of-step state and
//! held there, so a step long against an RC time constant burned the whole step at the
//! power of its first instant. `docs/plans/runaway-inside-a-coarse-step.md` measured it
//! at **6.6 K of a 20 K rise** on a day-long step, and had to prefix every one of its
//! compared runs with a 400 s warm-up to get out from under it.
//!
//! The correction is the exact step mean of the RC overpotentials — see
//! [`sim_core::ecm::rc_step_mean_excess_v`] — and it is handed to the thermal network
//! **only**. `Telemetry::q_gen_w` still reports the start-of-step instant, which is what
//! keeps it the exact partner of the start-of-step terminal voltage in the pack energy
//! ledger; the last test in this file pins that deliberately, so that moving one without
//! the other cannot happen quietly.
//!
//! Every pack here is **1S1P**, which is what makes the assertions analytic rather than
//! fitted: a lone cell has no neighbour, so its node conductance is exactly `hA` and its
//! steady rise above ambient is exactly `q/hA`.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::ecm::{rc_step_mean_excess_v, rc_update};
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const V0: f64 = 3.30;
const R0: f64 = 0.02;
const R_RC: f64 = 0.01;
const TAU_RC_S: f64 = 20.0;
/// The second pair, for the `Ecm2Rc` arm. Ten times the time constant of the first is
/// the point of it: a pair still moving after the first has settled is what makes the
/// two-pair answer distinguishable from the one-pair one.
const R_RC2: f64 = 0.005;
const TAU_RC2_S: f64 = 200.0;
const C_TH: f64 = 95.0;
const HA: f64 = 0.35;
const T_ENV: f64 = 298.15;
const LOAD_A: f64 = 10.0;

/// Long enough that the pack is dozens of convective time constants (`C/hA` = 271 s)
/// into its steady state at the end, and well past the `dt` at which the linear network
/// switches to backward Euler (6080 s for these parameters at `k` = 1).
const WINDOW_S: f64 = 20_000.0;

/// Deliberately enormous. These runs move 55 Ah and a fixture that emptied would be
/// measuring the SOC clamp.
const CAP_AH: f64 = 1.0e5;

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// Flat OCV, `R0` independent of both SOC and temperature, no entropy table, no
/// diffusion, no hysteresis. Everything that varies within a step in this cell is the RC
/// overpotential, which is exactly the quantity under test: a temperature-dependent `R0`
/// would put a second within-step variation beside it and the analytic predictions below
/// would stop being analytic.
fn chem(pairs: &[RcPair]) -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
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
            id: "step-mean-heat-test".into(),
            name: "Step-mean heat test cell".into(),
            provenance: "step-mean heat test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 5.0,
            v_min: 0.0,
            max_charge_c: 20.0,
            max_discharge_c: 20.0,
            t_charge_min_k: 250.0,
            t_max_k: 400.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![V0, V0],
            docv_dt_v_per_k: None,
            t_ref_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: pairs.to_vec(),
        thermal: ThermalParams {
            heat_capacity_j_per_k: C_TH,
            h_area_w_per_k: HA,
        },
    }
}

fn one_pair() -> Vec<RcPair> {
    vec![RcPair {
        r_ohms: R_RC,
        c_farad: TAU_RC_S / R_RC,
    }]
}

fn two_pairs() -> Vec<RcPair> {
    vec![
        RcPair {
            r_ohms: R_RC,
            c_farad: TAU_RC_S / R_RC,
        },
        RcPair {
            r_ohms: R_RC2,
            c_farad: TAU_RC2_S / R_RC2,
        },
    ]
}

fn config() -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        series: 1,
        parallel: 1,
        initial_soc: 0.9,
        initial_temp_k: T_ENV,
        seed: 0,
        scatter: Scatter::default(),
        // A lone cell has no neighbour, so `k` changes no physics at all here; it only
        // sets how conservative the sub-step bound is, and 1.0 is what puts the gate at
        // 6080 s — comfortably under `WINDOW_S`, so the coarse arm crosses it.
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        cell_model: CellModelConfig::Ecm,
    }
}

/// Run `WINDOW_S` of `LOAD_A` in steps of `dt` from a fresh pack, and return the cell's
/// end temperature \[K\].
fn run(pairs: &[RcPair], dt: f64) -> f64 {
    let mut pack = Pack::new(&config(), chem(pairs)).expect("fixture builds");
    let steps = (WINDOW_S / dt).round() as usize;
    for _ in 0..steps {
        pack.step(dt, Demand::Current(LOAD_A), &env());
    }
    pack.cell(0, 0).expect("in range").temp_k
}

/// The energy \[J\] a set of RC pairs withholds from a run that starts at rest: each
/// pair's overpotential climbs to `R·I` with time constant `tau`, and the area between
/// that ceiling and the climb is `I·R·tau` — so the heat a whole transient owes,
/// relative to a run that was settled from its first instant, is `I²·R·tau` summed over
/// pairs. It is paid once, however the step length is chosen, which is the whole reason
/// the two arms below can be compared at all.
fn transient_deficit_j(pairs: &[RcPair]) -> f64 {
    pairs
        .iter()
        .map(|p| LOAD_A * LOAD_A * p.r_ohms * (p.r_ohms * p.c_farad))
        .sum()
}

/// One step of `WINDOW_S` lands where 20 000 steps of 1 s land, to within the one
/// difference a single step genuinely cannot represent.
///
/// # What the remaining gap is, and why it is not an error
///
/// The two arms generate the same total heat — they differ in *when*. The fine arm pays
/// the RC transient's [`transient_deficit_j`] in its first minute and has relaxed it away
/// by the end (the window is 74 convective time constants long). The coarse arm has one
/// heat for the whole window, so it smears the same shortfall evenly across it and is
/// still carrying it at the end, depressed by exactly
///
/// ```text
/// deficit / WINDOW_S / hA
/// ```
///
/// which for these parameters is 2.857 mK. That is asserted here against a prediction
/// rather than against a bound, because a bound would pass on a number that was merely
/// small and this one is *derived*.
///
/// # What it was before
///
/// The pre-slice engine held the first instant's heat for the whole window, so the
/// coarse arm generated `I²·R0` = 2 W against a settled 3 W and landed
/// `I²·R_rc/hA` = **2.857 K** low — a thousand times the gap above, and the same
/// two-thirds shortfall `runaway-inside-a-coarse-step.md` measured at 6.6 K on its own
/// bigger pack. The second assertion is what would catch a regression to it.
#[test]
fn a_coarse_step_burns_the_windows_heat_not_the_first_instants() {
    let pairs = one_pair();
    let coarse = run(&pairs, WINDOW_S);
    let fine = run(&pairs, 1.0);

    let predicted = transient_deficit_j(&pairs) / WINDOW_S / HA;
    let gap = fine - coarse;
    assert!(
        (gap - predicted).abs() < 1.0e-6,
        "coarse {coarse} K vs fine {fine} K: gap {gap} K, predicted {predicted} K"
    );

    // And the contrast the slice exists for: the whole RC contribution, which is what
    // the pre-slice engine left on the floor.
    let pre_slice = LOAD_A * LOAD_A * R_RC / HA;
    assert!(
        gap < pre_slice / 100.0,
        "gap {gap} K is not far below the {pre_slice} K the frozen first instant cost"
    );
}

/// Both pairs are corrected, not just the first — and the second pair is what makes the
/// difference visible: with ten times the time constant it owes ten times its
/// resistance's share of the deficit, so an implementation that corrected only pair one
/// would land at a sixth of the predicted gap and fail here.
#[test]
fn every_rc_pair_is_corrected_not_only_the_first() {
    let pairs = two_pairs();
    let coarse = run(&pairs, WINDOW_S);
    let fine = run(&pairs, 1.0);

    let predicted = transient_deficit_j(&pairs) / WINDOW_S / HA;
    let gap = fine - coarse;
    assert!(
        (gap - predicted).abs() < 1.0e-6,
        "coarse {coarse} K vs fine {fine} K: gap {gap} K, predicted {predicted} K"
    );
    // The first pair alone would predict this, and it is six times too small.
    let first_pair_only = transient_deficit_j(&one_pair()) / WINDOW_S / HA;
    assert!(
        (gap - first_pair_only).abs() > 1.0e-3,
        "the two-pair gap {gap} K is indistinguishable from the one-pair {first_pair_only} K"
    );
}

/// [`rc_step_mean_excess_v`] is the exact mean of the trajectory [`rc_update`] walks,
/// checked against a numerical integral of that same trajectory rather than against a
/// second closed form — which would only re-derive the algebra it is meant to confirm.
#[test]
fn the_closed_form_is_the_integral_of_the_trajectory() {
    // (v0, i, r, c, dt): a climb, a climb far past tau, a decay against a reversed
    // current, a step much shorter than tau, and a pure relaxation at zero current.
    let cases = [
        (0.0, 10.0, R_RC, TAU_RC_S / R_RC, 5.0),
        (0.0, 10.0, R_RC, TAU_RC_S / R_RC, 1000.0),
        (0.15, -4.0, R_RC, TAU_RC_S / R_RC, 30.0),
        (-0.08, 7.5, R_RC2, TAU_RC2_S / R_RC2, 0.5),
        (0.2, 0.0, R_RC, TAU_RC_S / R_RC, 60.0),
    ];
    for (v0, i, r, c, dt) in cases {
        let tau = r * c;
        let v_end = rc_update(v0, i, r, c, dt);
        let closed = rc_step_mean_excess_v(v0, v_end, i, r, c, dt);
        // Midpoint rule on the analytic trajectory. 200 000 panels leaves a midpoint
        // error of order `(dt/n)²·|v0 − R·I|/(24·tau²)`, below 1e-13 V in every case
        // here — three orders under the tolerance asserted.
        let n = 200_000;
        let h = dt / f64::from(n);
        let v_ss = r * i;
        let mut sum = 0.0;
        for k in 0..n {
            let t = (f64::from(k) + 0.5) * h;
            sum += v_ss + (v0 - v_ss) * (-t / tau).exp();
        }
        let numeric = sum / f64::from(n) - v0;
        assert!(
            (closed - numeric).abs() < 1.0e-9 * numeric.abs().max(1.0e-3),
            "v0 {v0} i {i} r {r} dt {dt}: closed form {closed} V, integral {numeric} V"
        );
    }
}

/// The cases that must return an exact `0.0` rather than something small, because the
/// pack adds the term behind a `!= 0.0` guard and a `-0.0` heat would move a trajectory
/// for no physics.
///
/// A non-positive `dt` is the zero-length probe step every client takes before it runs.
/// A non-positive `tau` is a degenerate pair [`rc_update`] leaves untouched, where the
/// expression would otherwise return `R·I − V_0` — a real number, and the wrong one.
#[test]
fn the_zero_cases_are_exactly_zero() {
    let c = TAU_RC_S / R_RC;
    for dt in [0.0, -1.0, -0.0] {
        let v_end = rc_update(0.05, 10.0, R_RC, c, dt);
        let got = rc_step_mean_excess_v(0.05, v_end, 10.0, R_RC, c, dt);
        assert_eq!(got.to_bits(), 0.0_f64.to_bits(), "dt {dt} gave {got}");
    }
    for (r, cap) in [(0.0, c), (R_RC, 0.0), (-R_RC, c)] {
        let v_end = rc_update(0.05, 10.0, r, cap, 1.0);
        let got = rc_step_mean_excess_v(0.05, v_end, 10.0, r, cap, 1.0);
        assert_eq!(got.to_bits(), 0.0_f64.to_bits(), "r {r} c {cap} gave {got}");
    }
}

/// A zero-length step moves no temperature, still — the correction cannot reach one,
/// because at `dt <= 0` it is exactly zero.
#[test]
fn a_zero_length_step_still_moves_no_temperature() {
    let mut pack = Pack::new(&config(), chem(&one_pair())).expect("fixture builds");
    for _ in 0..40 {
        pack.step(1.0, Demand::Current(LOAD_A), &env());
    }
    let before = pack.cell(0, 0).expect("in range").temp_k;
    pack.step(0.0, Demand::Current(LOAD_A), &env());
    let after = pack.cell(0, 0).expect("in range").temp_k;
    assert_eq!(before.to_bits(), after.to_bits(), "{before} K -> {after} K");
}

/// **The deferral, pinned.** `q_gen_w` is still the start-of-step instant, and after a
/// coarse step the pack is warmer than that number can account for.
///
/// This is deliberate rather than overlooked: `q_gen_w` and the terminal voltage a client
/// integrates are both left-rectangle values from the same instant, which is what makes
/// `properties.rs::electrical_and_heat_energy_balance` close to rounding rather than to a
/// tolerance. Moving one and not the other opens that ledger. The next slice is where the
/// pair moves together; until it does, this test is what says so out loud — it fails the
/// moment someone reports the mean heat without reporting a mean voltage beside it.
#[test]
fn the_reported_heat_is_still_the_first_instants() {
    let mut pack = Pack::new(&config(), chem(&one_pair())).expect("fixture builds");
    let tele = pack.step(WINDOW_S, Demand::Current(LOAD_A), &env());

    // A fresh cell rests at zero overpotential, so the first instant generates `I²·R0`
    // and nothing else.
    let first_instant_w = LOAD_A * LOAD_A * R0;
    assert!(
        (tele.q_gen_w - first_instant_w).abs() < 1.0e-12,
        "q_gen_w {} W, first instant {first_instant_w} W",
        tele.q_gen_w
    );

    // What the pack actually absorbed, read off the temperature it reached: a lone cell
    // sits at `q/hA` above ambient once it has settled, and this window is 74 time
    // constants long.
    let absorbed_w = (pack.cell(0, 0).expect("in range").temp_k - T_ENV) * HA;
    let settled_w = LOAD_A * LOAD_A * (R0 + R_RC);
    assert!(
        absorbed_w > first_instant_w * 1.49 && absorbed_w < settled_w,
        "absorbed {absorbed_w} W is not between the first instant's {first_instant_w} W \
         and the settled {settled_w} W"
    );
}
