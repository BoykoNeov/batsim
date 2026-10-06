//! The model-based gauge on shipped chemistries: the four `*_filter*.toml` scenarios
//! against the gauge files they are twins of. See `docs/plans/model-based-estimator.md`.
//!
//! Two kinds of assertion. Each twin is its partner with exactly one thing added — the
//! estimator section, or for the weak-cell file one fault — checked on the parsed config,
//! so "differs by one field" is not a claim the header makes alone. And each file's
//! header table is re-measured here, on the protocol the header states, to the four
//! decimals it prints: booted 3 points high, 1 C for 300 s, rest for 3600 s, `dt` 0.5 s.

use std::path::{Path, PathBuf};

use sim_core::{Demand, Env, EstimatorConfig};
use sim_data::Scenario;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenario(name: &str) -> Scenario {
    sim_data::load_scenario_file(root().join("scenarios").join(format!("{name}.toml")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// Estimate minus truth \[points\] and the filter's sigma \[points\], at 30 s, at the end
/// of the discharge (300 s) and at the end of the rest (3900 s).
struct Marks {
    err: [f64; 3],
    sigma: [Option<f64>; 3],
}

fn run(name: &str) -> Marks {
    run_scenario(scenario(name))
}

fn run_scenario(sc: Scenario) -> Marks {
    let chem_id = sc
        .chemistry
        .clone()
        .expect("these files name their chemistry");
    let chem =
        sim_data::load_chemistry_file(root().join("chemistries").join(format!("{chem_id}.toml")))
            .unwrap();
    // Each cell at its own 1 C, as every gauge header measures it.
    let i_1c = chem.cell.capacity_ah;
    let mut pack = sc.build_pack(chem).unwrap();
    let env = Env {
        t_ambient: sc.pack.initial_temp_k,
        t_coolant: None,
    };
    let mut out = Marks {
        err: [f64::NAN; 3],
        sigma: [None; 3],
    };
    // Integer step counts, so the marks are exact rather than accumulated floats.
    for n in 1..=7800_u32 {
        let demand = if n <= 600 {
            Demand::Current(i_1c)
        } else {
            Demand::Rest
        };
        let tele = pack.step(0.5, demand, &env);
        let slot = match n {
            60 => 0,
            600 => 1,
            7800 => 2,
            _ => continue,
        };
        out.err[slot] = (tele.soc_bms.unwrap() - tele.soc_true) * 100.0;
        out.sigma[slot] = pack.bms().unwrap().soc_sigma().map(|s| s * 100.0);
    }
    out
}

/// Half a unit of the fourth decimal the headers print.
const TOL_PTS: f64 = 5e-5;

fn assert_marks(name: &str, got: &[f64; 3], want: [f64; 3]) {
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(
            (g - w).abs() <= TOL_PTS,
            "{name}, mark {k}: measured {g:.6} points, the header prints {w:.4}"
        );
    }
}

#[test]
fn each_twin_is_its_partner_with_the_estimator_alone_changed() {
    for (base, twin) in [
        ("na_ion_gauge_corrects", "na_ion_gauge_filter"),
        ("lfp_gauge_declines", "lfp_gauge_filter"),
        ("na_ion_gauge_low", "na_ion_gauge_low_filter"),
    ] {
        let b = scenario(base);
        let mut t = scenario(twin);
        let bms = t.pack.bms.as_mut().expect("the twin has a BMS");
        assert!(
            matches!(bms.estimator, EstimatorConfig::Ekf(_)),
            "{twin} runs the filter"
        );
        bms.estimator = EstimatorConfig::CoulombCount;
        assert_eq!(
            t.pack, b.pack,
            "{twin} differs from {base} beyond the estimator"
        );
        assert_eq!(t.faults, b.faults, "{twin} schedules different faults");
        assert_eq!(t.chemistry, b.chemistry);
    }
    let filter = scenario("lfp_gauge_filter");
    let weak = scenario("lfp_gauge_filter_weak_cell");
    assert_eq!(weak.pack, filter.pack);
    assert_eq!(filter.faults.len(), 0);
    assert_eq!(
        weak.faults.len(),
        1,
        "the weak-cell file adds exactly one fault"
    );
}

#[test]
fn the_counter_arms_still_measure_what_their_headers_say() {
    // The partner files' own headers print these, at the end of the rest; their counters
    // are the control every twin is read against, so a drift here moves every row below.
    let na = run("na_ion_gauge_corrects");
    assert!((na.err[2] - (-0.4941)).abs() <= TOL_PTS, "{}", na.err[2]);
    assert!(na.sigma[2].is_none(), "a counter reports no sigma");
    let low = run("na_ion_gauge_low");
    assert!((low.err[2] - (-0.9753)).abs() <= TOL_PTS, "{}", low.err[2]);
    let lfp = run("lfp_gauge_declines");
    assert!((lfp.err[2] - 2.0574).abs() <= TOL_PTS, "{}", lfp.err[2]);
}

/// Sodium-ion, mid-range: the boot error gone under load within half a minute, and the
/// end of the rest where the counter ends — both fooled by the hysteresis loop.
#[test]
fn on_sodium_ion_the_filter_closes_the_boot_at_once_and_lands_where_the_counter_does() {
    let cc = run("na_ion_gauge_corrects");
    let f = run("na_ion_gauge_filter");
    assert_marks("na_ion_gauge_filter", &f.err, [0.0465, -0.2445, -0.5630]);
    assert!((f.sigma[2].unwrap() - 0.0147).abs() <= TOL_PTS);
    assert!(cc.err[0] > 2.9 && f.err[0].abs() < 0.1);
    assert!((f.err[2] - cc.err[2]).abs() < 0.15);

    let cc = run("na_ion_gauge_low");
    let f = run("na_ion_gauge_low_filter");
    assert_marks(
        "na_ion_gauge_low_filter",
        &f.err,
        [0.0050, -0.6012, -1.0015],
    );
    assert!((f.sigma[2].unwrap() - 0.0131).abs() <= TOL_PTS);
    assert!((f.err[2] - cc.err[2]).abs() < 0.05);
}

/// LFP with an exact model: the filter corrects where the counter declines, but slowly.
#[test]
fn on_lfp_with_an_exact_model_the_filter_corrects_slowly() {
    let cc = run("lfp_gauge_declines");
    let f = run("lfp_gauge_filter");
    assert_marks("lfp_gauge_filter", &f.err, [2.9864, 1.9534, 0.6286]);
    assert!((f.sigma[2].unwrap() - 0.2055).abs() <= TOL_PTS);
    assert!(
        f.err[1] > 1.0,
        "still more than a point out when the discharge ends"
    );
    assert!(
        f.err[2] < cc.err[2] / 3.0,
        "but well inside the counter by the hour's end"
    );
}

/// LFP with the cell's resistance 20 % above the table: the filter is the worse estimator
/// by a wide margin, and sure of itself while it is.
#[test]
fn on_lfp_a_wrong_model_makes_the_filter_confidently_wrong() {
    let f = run("lfp_gauge_filter_weak_cell");
    assert_marks(
        "lfp_gauge_filter_weak_cell",
        &f.err,
        [-14.7430, -11.4428, -6.6661],
    );
    let sigma = f.sigma[2].unwrap();
    assert!((sigma - 0.0646).abs() <= TOL_PTS);
    assert!(
        f.err[2].abs() > 100.0 * sigma,
        "wrong by more than a hundred of its own error bars: {} against {sigma}",
        f.err[2]
    );
    // The counter on the same weak cell: this file with the estimator section taken out.
    // It never reads the voltage under load and declines at rest, so the weak cell costs
    // it nothing, and it is the header's counter row.
    let mut sc = scenario("lfp_gauge_filter_weak_cell");
    sc.pack.bms.as_mut().unwrap().estimator = EstimatorConfig::CoulombCount;
    let cc = run_scenario(sc);
    assert_marks("the weak cell's counter", &cc.err, [3.0067, 2.9420, 2.0574]);
    assert!(f.err[0].abs() > 4.0 * cc.err[0].abs());
}
