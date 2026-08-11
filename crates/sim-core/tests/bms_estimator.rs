//! Phase 2 BMS tests: sensors, sampling lag, and the SOC estimator.
//!
//! The theme is that the BMS is *wrong*, in specific, traceable ways. Each test
//! isolates one error source — sampling lag, a current-sensor offset, an unknown
//! starting point, a flat OCV curve, under-sampled temperature probes — and pins the
//! consequence. Anything here that started passing "by accident" because the BMS got
//! access to ground truth would be a violation of the design's point 8.

use sim_core::bms::BmsConfig;
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::ecm::ocv_invert;
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;
const T_ENV: f64 = 298.15;

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// A cell whose OCV rises steeply and linearly with SOC (1.2 V across the range), so
/// inverting an OCV reading recovers SOC precisely. This is the *easy* chemistry for
/// an estimator — the opposite of LFP.
fn steep_chem() -> ChemistryParams {
    ChemistryParams {
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "steep".into(),
            name: "Steep-OCV test cell".into(),
            provenance: "BMS test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 4.5,
            v_min: 2.5,
            max_charge_c: 10.0,
            max_discharge_c: 10.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![3.0, 4.2],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0, // tau = 20 s
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

/// The same cell with a perfectly flat OCV: an OCV reading carries no SOC
/// information at all, which is LFP's plateau taken to its limit.
fn flat_chem() -> ChemistryParams {
    let mut c = steep_chem();
    c.ocv = OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![3.30, 3.30],
        docv_dt_v_per_k: None,
    };
    c
}

/// A BMS with perfect sensors and no correction — isolates pure coulomb counting.
fn ideal_bms() -> BmsConfig {
    BmsConfig {
        balancing: None,
        protection: None,
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: Vec::new(),
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 1.0,
        min_ocv_slope_v_per_soc: 0.5,
    }
}

fn config(series: u16, parallel: u16, soc0: f64, bms: Option<BmsConfig>) -> PackConfig {
    PackConfig {
        aging: None,
        series,
        parallel,
        initial_soc: soc0,
        initial_temp_k: T_ENV,
        seed: 0xB0B,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms,
        cell_model: CellModelConfig::Ecm,
    }
}

#[test]
fn no_bms_reports_no_estimate() {
    let mut pack = Pack::new(&config(1, 1, 0.5, None), steep_chem()).unwrap();
    let tele = pack.step(1.0, Demand::Current(1.0), &env());
    assert_eq!(tele.soc_bms, None);
    assert!(pack.bms().is_none());
}

/// Inverting the OCV table: exact on a monotone segment, clamped outside it, and —
/// the case that matters — reporting zero slope where the curve is flat, because
/// there the inverse does not exist.
#[test]
fn ocv_inversion_reports_slope_as_confidence() {
    let steep = steep_chem().ocv;
    let (soc, slope) = ocv_invert(&steep, 3.6);
    assert!((soc - 0.5).abs() < 1e-12, "soc = {soc}");
    assert!((slope - 1.2).abs() < 1e-12, "slope = {slope}");

    // Outside the table, SOC clamps to the end but the adjacent slope is reported,
    // so a correction at a steep end is still permitted.
    let (soc_lo, slope_lo) = ocv_invert(&steep, 1.0);
    assert_eq!(soc_lo, 0.0);
    assert!((slope_lo - 1.2).abs() < 1e-12);
    let (soc_hi, _) = ocv_invert(&steep, 9.9);
    assert_eq!(soc_hi, 1.0);

    // Flat: any voltage maps to "no information", signalled by a zero slope.
    let flat = flat_chem().ocv;
    let (_, flat_slope) = ocv_invert(&flat, 3.30);
    assert_eq!(flat_slope, 0.0);

    // A curve with a flat middle and steep ends — the shape of a real LFP cell.
    let lfp_like = OcvTable {
        soc: vec![0.0, 0.1, 0.9, 1.0],
        volts: vec![2.5, 3.25, 3.30, 3.60],
        docv_dt_v_per_k: None,
    };
    let (_, plateau_slope) = ocv_invert(&lfp_like, 3.27);
    let (_, knee_slope) = ocv_invert(&lfp_like, 3.4);
    assert!(
        plateau_slope < 0.1 && knee_slope > 2.0,
        "plateau {plateau_slope} should be far flatter than the knee {knee_slope}"
    );
}

/// The estimator acts on information one step old. With perfect sensors, the first
/// step therefore integrates the *initial* frame's zero current, and the SOC estimate
/// only starts moving on the second step — one step behind the truth, forever.
#[test]
fn estimator_lags_the_truth_by_exactly_one_step() {
    let dt = 1.0;
    let i = 2.0;
    let soc0 = 0.6;
    let mut pack = Pack::new(&config(1, 1, soc0, Some(ideal_bms())), steep_chem()).unwrap();

    let t1 = pack.step(dt, Demand::Current(i), &env());
    assert_eq!(
        t1.soc_bms,
        Some(soc0),
        "first step integrates the initial (zero-current) frame"
    );
    assert!(t1.soc_true < soc0, "but the truth has already moved");
    // The frame sampled at the end of step 1 does carry the real current.
    let frame = pack.bms().unwrap().sensors();
    assert!((frame.i_pack_a - i).abs() < 1e-12, "{}", frame.i_pack_a);
    assert!((frame.sampled_at_s - dt).abs() < 1e-12);

    // Step 2 integrates it, so the estimate now trails the truth by one step's worth
    // of charge and stays there.
    let t2 = pack.step(dt, Demand::Current(i), &env());
    let per_step = i * dt / (3600.0 * CAP_AH);
    assert!(
        (t2.soc_bms.unwrap() - (soc0 - per_step)).abs() < 1e-12,
        "estimate {:?}",
        t2.soc_bms
    );
    for _ in 0..50 {
        let t = pack.step(dt, Demand::Current(i), &env());
        let lag = t.soc_bms.unwrap() - t.soc_true;
        assert!(
            (lag - per_step).abs() < 1e-9,
            "lag should stay one step's charge: {lag} vs {per_step}"
        );
    }
}

/// A current-sensor offset integrates into unbounded SOC drift — the classic
/// coulomb-counting failure. It also poisons rest detection: the pack looks busy
/// while sitting still, so the OCV correction that would have caught the drift never
/// fires. Two failures from one broken sensor.
#[test]
fn current_offset_drifts_the_estimate_and_blocks_rest_detection() {
    let offset = 0.05; // A, reads high by 50 mA
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        current_offset_a: offset,
        rest_current_threshold_a: 0.01, // below the offset: never looks rested
        ..ideal_bms()
    };
    let soc0 = 0.7;
    let mut pack = Pack::new(&config(1, 1, soc0, Some(bms)), steep_chem()).unwrap();

    let dt = 1.0;
    let steps = 3600;
    let mut last = None;
    for _ in 0..steps {
        last = Some(pack.step(dt, Demand::Rest, &env()));
    }
    let tele = last.unwrap();

    // Truth never moved: the pack rested the whole time.
    assert!(
        (tele.soc_true - soc0).abs() < 1e-12,
        "true SOC should be untouched: {}",
        tele.soc_true
    );
    // The estimate fell by ∫offset dt / capacity, minus the one-step lag.
    let expected_drift = offset * dt * f64::from(steps - 1) / (3600.0 * CAP_AH);
    let drift = soc0 - tele.soc_bms.unwrap();
    assert!(
        (drift - expected_drift).abs() < 1e-9,
        "drift {drift}, expected {expected_drift}"
    );
    assert!(drift > 0.019, "drift should be visible: {drift}");
    // And the rest timer never accumulated, so no correction could ever fire.
    assert_eq!(pack.bms().unwrap().rest_time_s(), 0.0);
}

/// On a steep OCV curve, a rested reading fixes an estimate that started wrong. This
/// is the mechanism that is supposed to save coulomb counting.
#[test]
fn rested_ocv_reading_corrects_a_wrong_initial_estimate() {
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        initial_soc_error: 0.25, // BMS boots believing it is much fuller than it is
        rest_time_for_ocv_s: 300.0,
        ..ideal_bms()
    };
    let soc0 = 0.5;
    let mut pack = Pack::new(&config(1, 1, soc0, Some(bms)), steep_chem()).unwrap();

    // Before any rest has accumulated, the estimate is simply wrong.
    let first = pack.step(1.0, Demand::Current(1.0), &env());
    assert!(
        (first.soc_bms.unwrap() - 0.75).abs() < 1e-12,
        "{:?}",
        first.soc_bms
    );

    // Rest long enough for both the RC transient (tau = 20 s) and the BMS's own
    // relaxation requirement, then the reading pulls the estimate onto the truth.
    let mut last = None;
    for _ in 0..400 {
        last = Some(pack.step(1.0, Demand::Rest, &env()));
    }
    let tele = last.unwrap();
    let err = (tele.soc_bms.unwrap() - tele.soc_true).abs();
    assert!(
        err < 1e-3,
        "corrected estimate should track truth, error {err}"
    );
}

/// The same code, the same error, a flat OCV curve — and the correction never fires,
/// because inverting a flat curve would turn millivolts of sensor error into an
/// arbitrary SOC. The estimate stays wrong for as long as you rest.
///
/// This is the LFP problem in miniature, and it is intended behaviour: see
/// `sim-data/tests/scenario_lfp_soc_drift.rs` for the same effect on the real
/// shipped LFP curve.
#[test]
fn flat_ocv_curve_defeats_the_correction() {
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        initial_soc_error: 0.25,
        rest_time_for_ocv_s: 300.0,
        ..ideal_bms()
    };
    let soc0 = 0.5;
    let mut pack = Pack::new(&config(1, 1, soc0, Some(bms)), flat_chem()).unwrap();

    let mut last = None;
    for _ in 0..2000 {
        last = Some(pack.step(1.0, Demand::Rest, &env()));
    }
    let tele = last.unwrap();
    // Rest was detected — the sensor is perfect, the pack really is resting.
    assert!(pack.bms().unwrap().rest_time_s() > 300.0);
    // The estimate is nonetheless still off by the full initial error.
    let err = tele.soc_bms.unwrap() - tele.soc_true;
    assert!(
        (err - 0.25).abs() < 1e-9,
        "estimate should still be wrong by the initial error, off by {err}"
    );
}

/// Temperature probes instrument a few cells, not all of them, so the BMS can be
/// blind to the hottest cell in the pack. Ground truth knows; the BMS does not.
#[test]
fn probes_can_miss_the_hottest_cell() {
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        // Corners only — precisely the coolest cells in a block (see tests/thermal.rs).
        temp_probes: vec![(0, 0), (0, 4), (4, 0), (4, 4)],
        ..ideal_bms()
    };
    let mut cfg = config(5, 5, 0.9, Some(bms));
    cfg.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&cfg, steep_chem()).unwrap();

    let mut last = None;
    for _ in 0..1200 {
        last = Some(pack.step(1.0, Demand::Current(25.0), &env()));
    }
    let tele = last.unwrap();
    let frame = pack.bms().unwrap().sensors();
    let hottest_probe = frame.max_probe_k().expect("four probes configured");

    assert_eq!(frame.temp_probe_k.len(), 4);
    assert!(
        hottest_probe < tele.t_max - 0.5,
        "BMS should underestimate the peak: probes {hottest_probe} vs truth {}",
        tele.t_max
    );
    // It is not blind, just under-sampled: the corners did heat up.
    assert!(hottest_probe > T_ENV + 0.5, "probes {hottest_probe}");
}

/// Sensor noise is drawn from the one seeded RNG, so a noisy BMS is still perfectly
/// reproducible — and two different seeds genuinely differ.
#[test]
fn sensor_noise_is_seeded_and_reproducible() {
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        current_noise_sigma_a: 0.2,
        ..ideal_bms()
    };
    let run = |seed: u64| {
        let mut cfg = config(1, 1, 0.6, Some(bms.clone()));
        cfg.seed = seed;
        let mut pack = Pack::new(&cfg, steep_chem()).unwrap();
        let mut readings = Vec::new();
        for _ in 0..40 {
            pack.step(1.0, Demand::Current(1.0), &env());
            readings.push(pack.bms().unwrap().sensors().i_pack_a);
        }
        readings
    };
    let a = run(1);
    let b = run(1);
    let c = run(2);
    assert_eq!(a, b, "same seed must reproduce the noise sequence exactly");
    assert_ne!(a, c, "different seeds must differ");
    // The readings scatter around the true 1 A without being it.
    assert!(a.iter().all(|&x| (x - 1.0).abs() < 2.0));
    assert!(
        a.iter().any(|&x| (x - 1.0).abs() > 1e-6),
        "noise should actually perturb the reading"
    );
}

/// A noiseless BMS draws nothing from the RNG, so adding one cannot shift the
/// trajectory of a pack whose randomness comes from scatter. Determinism is per-seed,
/// and the draw *order* is part of the contract.
#[test]
fn a_noiseless_bms_consumes_no_randomness() {
    let scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.05,
    };
    let run = |bms: Option<BmsConfig>| {
        let mut cfg = config(2, 2, 0.7, bms);
        cfg.scatter = scatter;
        let mut pack = Pack::new(&cfg, steep_chem()).unwrap();
        let mut vs = Vec::new();
        for step in 0..30 {
            let demand = if step % 3 == 0 {
                Demand::Rest
            } else {
                Demand::Current(2.0)
            };
            vs.push(pack.step(0.5, demand, &env()).v_terminal);
        }
        vs
    };
    assert_eq!(
        run(None),
        run(Some(ideal_bms())),
        "a zero-noise BMS must not perturb the pack trajectory"
    );
}

#[test]
fn invalid_bms_config_is_rejected() {
    let bad_gain = BmsConfig {
        balancing: None,
        protection: None,
        ocv_correction_gain: 1.5,
        ..ideal_bms()
    };
    let err = Pack::new(&config(1, 1, 0.5, Some(bad_gain)), steep_chem()).unwrap_err();
    assert!(format!("{err}").contains("ocv_correction_gain"), "{err}");

    let bad_sigma = BmsConfig {
        balancing: None,
        protection: None,
        current_noise_sigma_a: -1.0,
        ..ideal_bms()
    };
    let err = Pack::new(&config(1, 1, 0.5, Some(bad_sigma)), steep_chem()).unwrap_err();
    assert!(format!("{err}").contains("current_noise_sigma_a"), "{err}");

    // A probe pointing outside the pack names the offending index and the topology.
    let bad_probe = BmsConfig {
        balancing: None,
        protection: None,
        temp_probes: vec![(0, 0), (0, 3)],
        ..ideal_bms()
    };
    let err = Pack::new(&config(2, 2, 0.5, Some(bad_probe)), steep_chem()).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("temp_probes[1]") && msg.contains("2S2P"),
        "{msg}"
    );
}
