//! The model-based estimator (`EstimatorConfig::Ekf`): what it fixes, what it cannot, and
//! that it is put together honestly. See `docs/plans/model-based-estimator.md`.
//!
//! Every test here that makes a claim about the filter runs the coulomb counter on the
//! same pack beside it. How far an estimate *moved* says nothing on its own; where it ends
//! against the truth, next to where the other estimator ends, does.

use sim_core::bms::BmsConfig;
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    BuildError, CellModelConfig, Demand, EkfConfig, Env, EstimatorConfig, Pack, PackConfig,
    Scatter, ThermalConfig,
};

const CAP_AH: f64 = 2.5;
const T_ENV: f64 = 298.15;

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// The steep linear cell `bms_estimator.rs` uses: OCV rises 1.2 V across the range, `R0`
/// is constant, one RC pair. On it the filter's model is the engine's cell exactly, which
/// is what lets [`the_filter_tracks_an_exact_model_to_rounding_at_any_dt_pattern`] demand
/// agreement to rounding.
fn steep_chem() -> ChemistryParams {
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
            t_ref_k: None,
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

/// The same cell with a perfectly flat OCV: a voltage carries no information about charge.
fn flat_chem() -> ChemistryParams {
    let mut c = steep_chem();
    c.ocv = OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![3.30, 3.30],
        docv_dt_v_per_k: None,
        t_ref_k: None,
    };
    c
}

/// The filter settings the scenario twins use. Each is a tuning choice, not a property of
/// any cell: see `EkfConfig`.
fn ekf() -> EkfConfig {
    EkfConfig {
        current_sigma_a: 0.05,
        voltage_sigma_v: 0.010,
        initial_soc_sigma: 0.05,
    }
}

/// Exact sensors, no boot error, one probe — and the estimator given.
fn bms(estimator: EstimatorConfig) -> BmsConfig {
    BmsConfig {
        balancing: None,
        protection: None,
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 1.0,
        min_ocv_slope_v_per_soc: 0.5,
        estimator,
    }
}

fn config(soc0: f64, bms: BmsConfig) -> PackConfig {
    PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: T_ENV,
        seed: 0xB0B,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms),
        cell_model: CellModelConfig::Ecm,
    }
}

/// The filter predicts over the interval the frame's current was measured over, so on a
/// cell its model matches exactly it never sees a miss — at **any** pattern of step
/// lengths, and through a load that changes sign and a rest.
///
/// The estimate is one frame behind, as the coulomb counter's is: the BMS acts on the
/// frame sampled at the end of the previous step. So step `n`'s estimate is compared with
/// step `n − 1`'s truth. A filter that predicted over *this* step's length instead — the
/// coulomb counter's convention — pairs a current with the wrong interval whenever the
/// length changes, and misses by millivolts on every change.
#[test]
fn the_filter_tracks_an_exact_model_to_rounding_at_any_dt_pattern() {
    let mut pack = Pack::new(&config(0.6, bms(EstimatorConfig::Ekf(ekf()))), steep_chem()).unwrap();
    let mut prev_truth = 0.6;
    let mut worst = 0.0_f64;
    for n in 0..2000 {
        let dt = if n % 2 == 0 { 0.3 } else { 1.7 };
        let demand = match (n / 150) % 4 {
            0 => Demand::Current(2.5),
            1 => Demand::Current(-1.5),
            2 => Demand::Rest,
            _ => Demand::Current(4.0),
        };
        let tele = pack.step(dt, demand, &env());
        let est = tele.soc_bms.unwrap();
        worst = worst.max((est - prev_truth).abs());
        prev_truth = tele.soc_true;
    }
    assert!(
        worst < 1e-9,
        "an exact model under exact sensors must track the truth to rounding; worst miss \
         {worst:e}"
    );
}

/// The same exactness off the reference temperature: an `R0` grid that triples from warm to
/// cold and an OCV temperature correction, with the pack held cold. The filter reads both
/// tables at its **probe's** temperature, so on an isothermal pack its model still matches
/// the engine to rounding — and a filter that read its tables at any other temperature (the
/// reference, the ambient it was never told) would miss by tens of millivolts.
#[test]
fn the_filter_reads_its_tables_at_the_probe_temperature() {
    let mut chem = steep_chem();
    chem.r0 = R0Table {
        soc: vec![0.0, 1.0],
        temp_k: vec![263.15, 298.15],
        ohms: vec![vec![0.06, 0.02], vec![0.06, 0.02]],
    };
    chem.ocv.docv_dt_v_per_k = Some(vec![0.0005, 0.0005]);
    chem.ocv.t_ref_k = Some(298.15);
    let mut c = config(0.6, bms(EstimatorConfig::Ekf(ekf())));
    c.initial_temp_k = 273.15;
    let mut pack = Pack::new(&c, chem).unwrap();
    let cold = Env {
        t_ambient: 273.15,
        t_coolant: None,
    };
    let mut prev_truth = 0.6;
    let mut worst = 0.0_f64;
    for n in 0..1200 {
        let demand = if (n / 200) % 2 == 0 {
            Demand::Current(2.5)
        } else {
            Demand::Current(-1.0)
        };
        let tele = pack.step(1.0, demand, &cold);
        worst = worst.max((tele.soc_bms.unwrap() - prev_truth).abs());
        prev_truth = tele.soc_true;
    }
    assert!(worst < 1e-9, "worst miss {worst:e}");
}

/// A 2S3P pack of identical cells is the average cell three times over in each group and
/// twice in series: the filter divides the pack current by the parallel count and reads the
/// mean group voltage, so its model is still exact — and a filter that put the whole pack
/// current through one cell's resistance would miss by two thirds of the drop.
#[test]
fn the_filter_models_the_average_cell_of_a_series_parallel_pack() {
    let mut c = config(0.6, bms(EstimatorConfig::Ekf(ekf())));
    c.series = 2;
    c.parallel = 3;
    let mut pack = Pack::new(&c, steep_chem()).unwrap();
    let mut prev_truth = 0.6;
    let mut worst = 0.0_f64;
    for n in 0..1200 {
        let demand = if (n / 200) % 2 == 0 {
            Demand::Current(7.5)
        } else {
            Demand::Current(-3.0)
        };
        let tele = pack.step(1.0, demand, &env());
        worst = worst.max((tele.soc_bms.unwrap() - prev_truth).abs());
        prev_truth = tele.soc_true;
    }
    assert!(worst < 1e-9, "worst miss {worst:e}");
}

/// The boot error: three points high on a steep cell under a 1 C discharge. The counter
/// has no way to learn it is wrong while current flows; the filter reads it off the
/// voltage within seconds.
#[test]
fn a_wrong_boot_is_closed_under_load_on_a_steep_cell() {
    let run = |estimator| {
        let mut b = bms(estimator);
        b.initial_soc_error = 0.03;
        let mut pack = Pack::new(&config(0.6, b), steep_chem()).unwrap();
        let mut err_at_60 = f64::NAN;
        for n in 1..=600 {
            let tele = pack.step(1.0, Demand::Current(CAP_AH), &env());
            if n == 60 {
                err_at_60 = tele.soc_bms.unwrap() - tele.soc_true;
            }
        }
        err_at_60
    };
    let cc = run(EstimatorConfig::CoulombCount);
    let filter = run(EstimatorConfig::Ekf(ekf()));
    assert!(
        (cc - 0.03).abs() < 2e-3,
        "the counter holds its boot error under load (plus one step of lag): {cc}"
    );
    assert!(
        filter.abs() < 1e-3,
        "the filter closes the boot error to under a tenth of a point within a minute: \
         {filter}"
    );
}

/// On a perfectly flat curve the voltage says nothing about charge, so the filter's gain on
/// charge is zero and it is exactly as wrong as the counter, for as long as it runs. Its
/// own uncertainty knows it: the reported sigma does not shrink below the boot sigma.
#[test]
fn a_flat_curve_gives_the_filter_nothing_to_correct_with() {
    let mut b = bms(EstimatorConfig::Ekf(ekf()));
    b.initial_soc_error = 0.03;
    let mut pack = Pack::new(&config(0.6, b), flat_chem()).unwrap();
    let mut last_err = f64::NAN;
    for n in 0..1200 {
        let demand = if n < 600 {
            Demand::Current(CAP_AH)
        } else {
            Demand::Rest
        };
        let tele = pack.step(1.0, demand, &env());
        last_err = tele.soc_bms.unwrap() - tele.soc_true;
    }
    assert!(
        (last_err - 0.03).abs() < 1e-9,
        "nothing on a flat curve can move the charge estimate except the current: {last_err}"
    );
    let sigma = pack.bms().unwrap().soc_sigma().unwrap();
    assert!(
        sigma >= 0.05,
        "and the filter knows it learnt nothing — its sigma only grew: {sigma}"
    );
}

/// A current-sensor offset is not noise, and the filter has no state for it. What it does
/// instead is settle: each frame's voltage pulls the estimate back by as much as the offset
/// pushed it, so the error stops growing at a level set by how much current noise the
/// filter was told to expect — while the counter's grows without limit.
///
/// Measured (`docs/plans/model-based-estimator.md`, prediction 8, which said "under 0.2
/// point" at 0.05 A and was wrong): at an assumed 0.05 A the error settles at −0.737 point
/// by the third hour; at 0.2 A, −0.117; at 1 A, +0.035. The counter is 8 points out by
/// the fourth hour.
#[test]
fn an_offset_drifts_the_counter_and_settles_the_filter_on_a_steep_cell() {
    let run = |estimator| {
        let mut b = bms(estimator);
        b.current_offset_a = 0.05;
        let mut pack = Pack::new(&config(0.6, b), steep_chem()).unwrap();
        let mut at_3h = f64::NAN;
        let mut last = f64::NAN;
        for n in 0..(4 * 3600) {
            // Ten-minute legs of 1 C either way, so the cell stays mid-range.
            let i = if (n / 600) % 2 == 0 { CAP_AH } else { -CAP_AH };
            let tele = pack.step(1.0, Demand::Current(i), &env());
            last = tele.soc_bms.unwrap() - tele.soc_true;
            if n + 1 == 3 * 3600 {
                at_3h = last;
            }
        }
        (at_3h, last)
    };
    let (_, cc) = run(EstimatorConfig::CoulombCount);
    // 50 mA for four hours against 2.5 A.h is eight points, less one step of lag.
    assert!(
        (cc + 0.08).abs() < 1e-3,
        "the counter drifts by the integrated offset: {cc}"
    );
    let (filter_3h, filter) = run(EstimatorConfig::Ekf(ekf()));
    assert!(
        (filter - filter_3h).abs() < 1e-4,
        "the filter's error has stopped moving by the third hour: {filter_3h} then {filter}"
    );
    assert!(
        filter.abs() < 0.01,
        "and settled under a point, where the counter is eight out: {filter}"
    );
    let (_, wider) = run(EstimatorConfig::Ekf(EkfConfig {
        current_sigma_a: 0.2,
        ..ekf()
    }));
    assert!(
        wider.abs() < 0.25 * filter.abs(),
        "telling it to expect more current noise makes it lean on the voltage harder, and          the settled error shrinks: {wider} against {filter}"
    );
}

/// The default is the counter, bit for bit: a config that names it and one that omits it
/// (serde's default) build the same pack and run the same trajectory. The wider claim —
/// that no test or scenario moved — is the suite itself.
#[test]
fn the_default_estimator_is_the_coulomb_counter() {
    assert_eq!(EstimatorConfig::default(), EstimatorConfig::CoulombCount);
    let mut b = bms(EstimatorConfig::CoulombCount);
    b.initial_soc_error = 0.02;
    let mut pack = Pack::new(&config(0.6, b), steep_chem()).unwrap();
    assert_eq!(pack.bms().unwrap().soc_sigma(), None);
    let tele = pack.step(1.0, Demand::Current(1.0), &env());
    assert!(tele.soc_bms.is_some());
}

/// Snapshot mid-run with the filter live, restore, continue: bit-identical, through serde
/// (bincode) and not merely through a clone.
#[test]
fn a_filter_snapshot_round_trips_bit_for_bit() {
    let mut b = bms(EstimatorConfig::Ekf(ekf()));
    b.initial_soc_error = 0.03;
    b.current_noise_sigma_a = 0.02;
    b.current_offset_a = 0.01;
    let mut a = Pack::new(&config(0.7, b), steep_chem()).unwrap();
    let demand = |n: usize| {
        if (n / 40).is_multiple_of(2) {
            Demand::Current(2.0)
        } else {
            Demand::Rest
        }
    };
    for n in 0..200 {
        a.step(0.5, demand(n), &env());
    }
    let bytes = bincode::serialize(&a.snapshot()).unwrap();
    let mut b = Pack::restore(&bincode::deserialize(&bytes).unwrap()).unwrap();
    for n in 200..400 {
        let ta = a.step(0.5, demand(n), &env());
        let tb = b.step(0.5, demand(n), &env());
        assert_eq!(
            ta.soc_bms.unwrap().to_bits(),
            tb.soc_bms.unwrap().to_bits(),
            "step {n}"
        );
        assert_eq!(
            a.bms().unwrap().soc_sigma().unwrap().to_bits(),
            b.bms().unwrap().soc_sigma().unwrap().to_bits(),
            "step {n}"
        );
    }
}

/// A zero-length step samples no frame, so the filter does not move — not its estimate,
/// not its uncertainty.
#[test]
fn a_zero_length_step_moves_nothing() {
    let mut b = bms(EstimatorConfig::Ekf(ekf()));
    b.initial_soc_error = 0.03;
    let mut pack = Pack::new(&config(0.6, b), steep_chem()).unwrap();
    for _ in 0..10 {
        pack.step(1.0, Demand::Current(CAP_AH), &env());
    }
    let before = (
        pack.bms().unwrap().soc_estimate(),
        pack.bms().unwrap().soc_sigma(),
    );
    for _ in 0..5 {
        pack.step(0.0, Demand::Current(CAP_AH), &env());
    }
    let after = (
        pack.bms().unwrap().soc_estimate(),
        pack.bms().unwrap().soc_sigma(),
    );
    assert_eq!(before, after);
}

/// The filter needs a temperature for its tables, and a probe is the only one it may read.
#[test]
fn the_filter_is_refused_without_a_probe_or_with_a_degenerate_tuning() {
    let mut b = bms(EstimatorConfig::Ekf(ekf()));
    b.temp_probes.clear();
    assert!(matches!(
        Pack::new(&config(0.5, b), steep_chem()),
        Err(BuildError::BadBmsConfig {
            field: "temp_probes",
            ..
        })
    ));
    for (cfg, field) in [
        (
            EkfConfig {
                voltage_sigma_v: 0.0,
                ..ekf()
            },
            "estimator.Ekf.voltage_sigma_v",
        ),
        (
            EkfConfig {
                current_sigma_a: -1.0,
                ..ekf()
            },
            "estimator.Ekf.current_sigma_a",
        ),
        (
            EkfConfig {
                initial_soc_sigma: f64::NAN,
                ..ekf()
            },
            "estimator.Ekf.initial_soc_sigma",
        ),
    ] {
        let got = Pack::new(&config(0.5, bms(EstimatorConfig::Ekf(cfg))), steep_chem());
        assert!(
            matches!(got, Err(BuildError::BadBmsConfig { field: f, .. }) if f == field),
            "{field}: {got:?}"
        );
    }
}
