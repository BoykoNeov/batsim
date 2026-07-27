//! Scenario: **an LFP SOC estimate drifts mid-range and cannot be corrected there.**
//!
//! One of the Phase 2 exit criteria. This runs against the *shipped* LFP chemistry
//! rather than a synthetic curve, because the whole point is a property of the real
//! material: LFP's discharge plateau is so flat that a rested open-circuit voltage
//! reading — the standard fix for coulomb-counting drift — carries almost no
//! information about state of charge.
//!
//! Slopes of the shipped `lfp_26650_generic` OCV table, per unit SOC:
//!
//! | SOC range   | dOCV/dSOC     |
//! | ----------- | ------------- |
//! | 0.55 – 0.65 | 0.022 V       |
//! | 0.45 – 0.55 | 0.057 V       |
//! | 0.25 – 0.35 | 0.467 V       |
//! | 0.9875 – 0.99 | 9.2 V       |
//! | 0.0 – 0.0025 | 29.7 V       |
//!
//! Three orders of magnitude between the plateau and the end knees. A BMS trust
//! threshold of 0.5 V per unit SOC therefore rejects every reading between roughly
//! SOC 0.25 and 0.95 and accepts the knees — which is not a tuning trick, it is what
//! any competent LFP BMS does.
//!
//! The same engine code on the synthetic steep-OCV cell in
//! `sim-core/tests/bms_estimator.rs` corrects perfectly. The difference is entirely
//! in the chemistry data.

use sim_core::bms::BmsConfig;
use sim_core::{ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

fn lfp_chem() -> ChemistryParams {
    let text = include_str!("../../../chemistries/lfp_26650_generic.toml");
    sim_data::parse_chemistry(text).expect("LFP chemistry loads")
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A plausible mid-range BMS: an honest current sensor, an initial estimate that is
/// 10 % too low (a BMS that just booted), and an OCV correction it is willing to
/// apply after 10 minutes of rest — but only where the curve is steep enough to mean
/// something.
fn bms() -> BmsConfig {
    BmsConfig {
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        // Negative, so the estimate stays clear of the [0, 1] clamp at high SOC and
        // the error remains visible rather than being masked.
        initial_soc_error: -0.10,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 1.0,
        // Rejects the entire LFP plateau (steepest segment there ≈ 0.47 V) and
        // accepts the end knees (> 4 V). See the module docs.
        min_ocv_slope_v_per_soc: 0.5,
    }
}

fn config(soc0: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: 298.15,
        seed: 0xF0CA,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms()),
    }
}

/// Rest until the BMS has accumulated `at_least_s` of rest time, plus margin for the
/// RC transient to relax. Returns the final telemetry.
fn rest_for(pack: &mut Pack, seconds: usize) -> sim_core::Telemetry {
    let mut last = None;
    for _ in 0..seconds {
        last = Some(pack.step(1.0, Demand::Rest, &env()));
    }
    last.expect("at least one step")
}

#[test]
fn lfp_estimate_drifts_in_the_plateau_and_is_corrected_only_at_the_knee() {
    let mut pack = Pack::new(&config(0.60), lfp_chem()).unwrap();

    // --- Phase 1: mid-plateau. Discharge a little, then rest well past the BMS's
    // relaxation requirement. The pack *is* resting and the BMS *knows* it is
    // resting — the reading is simply useless here.
    for _ in 0..600 {
        pack.step(1.0, Demand::Current(1.15), &env()); // ~C/2
    }
    let tele = rest_for(&mut pack, 1800);

    let rest_seen = pack.bms().unwrap().rest_time_s();
    assert!(
        rest_seen > 600.0,
        "the BMS should have detected a long rest, saw {rest_seen} s"
    );
    let plateau_err = tele.soc_bms.unwrap() - tele.soc_true;
    assert!(
        (plateau_err + 0.10).abs() < 1e-3,
        "mid-plateau the initial −10 % error should survive intact, got {plateau_err}"
    );
    // Sanity: we really are on the plateau, where a 10 % SOC error is worth
    // single-digit millivolts of OCV — which is why no sensor could resolve it.
    let v = tele.v_terminal;
    assert!(
        (3.25..3.30).contains(&v),
        "expected a plateau voltage, got {v} V"
    );

    // --- Phase 2: same pack, same BMS, charged up to the top knee where the curve
    // finally has slope. Now the identical correction logic fires and works.
    let mut steps = 0;
    while pack.cell(0, 0).unwrap().soc < 0.99 && steps < 20_000 {
        pack.step(1.0, Demand::Current(-1.15), &env());
        steps += 1;
    }
    assert!(steps < 20_000, "charging should reach the knee");
    let tele = rest_for(&mut pack, 1800);

    let knee_err = tele.soc_bms.unwrap() - tele.soc_true;
    assert!(
        knee_err.abs() < 5e-3,
        "at the knee the correction should converge, error {knee_err}"
    );
    assert!(
        knee_err.abs() < plateau_err.abs() / 10.0,
        "the knee correction should be at least 10x better than the plateau: \
         {knee_err} vs {plateau_err}"
    );
}

/// The drift is *unbounded* in the plateau: a current-sensor offset integrates
/// without limit, and no amount of resting mid-range recovers it. This is the failure
/// mode that makes LFP state-of-charge estimation genuinely hard.
#[test]
fn lfp_plateau_drift_grows_without_bound() {
    let mut cfg = config(0.70);
    cfg.bms = Some(BmsConfig {
        current_offset_a: 0.02, // 20 mA, under 1 % of a C/2 current
        initial_soc_error: 0.0,
        // Generous: the sensor offset is *below* the rest threshold, so the BMS
        // correctly detects rest and genuinely tries to correct. It still cannot.
        rest_current_threshold_a: 0.05,
        ..bms()
    });
    let mut pack = Pack::new(&cfg, lfp_chem()).unwrap();

    let mut errors = Vec::new();
    // Three cycles of: mild discharge, then a long mid-plateau rest.
    for _ in 0..3 {
        for _ in 0..600 {
            pack.step(1.0, Demand::Current(1.15), &env());
        }
        let tele = rest_for(&mut pack, 1200);
        errors.push(tele.soc_bms.unwrap() - tele.soc_true);
    }

    // Monotonically worse, never recovered by any rest.
    for w in errors.windows(2) {
        assert!(
            w[1] < w[0] - 1e-4,
            "each plateau cycle should add drift: {errors:?}"
        );
    }
    assert!(
        errors[2] < -0.01,
        "drift should exceed 1 % SOC after three cycles: {errors:?}"
    );
    // The pack is still comfortably inside the plateau, so this is not an artefact of
    // wandering into a knee.
    let soc = pack.cell(0, 0).unwrap().soc;
    assert!((0.3..0.75).contains(&soc), "still mid-plateau: {soc}");
}
