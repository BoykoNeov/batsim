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
    /// Estimate minus truth \[points\] at 60 s, the guided path's step 33 mark.
    mark: f64,
    /// The lowest estimate minus truth \[points\] at any step of the run.
    low: f64,
    sigma: [Option<f64>; 3],
    /// The estimate itself \[%\] at 1800 s and at 3900 s.
    est: [f64; 2],
}

fn run(name: &str) -> Marks {
    run_scenario(scenario(name))
}

fn run_scenario(sc: Scenario) -> Marks {
    run_with(sc, false)
}

/// `strip_hysteresis` removes the chemistry's `[hysteresis]` section — the control arm for
/// "the loop is what the filter lands short by".
fn run_with(sc: Scenario, strip_hysteresis: bool) -> Marks {
    let chem_id = sc
        .chemistry
        .clone()
        .expect("these files name their chemistry");
    let mut chem =
        sim_data::load_chemistry_file(root().join("chemistries").join(format!("{chem_id}.toml")))
            .unwrap();
    if strip_hysteresis {
        chem.hysteresis = None;
    }
    // Each cell at its own 1 C, as every gauge header measures it.
    let i_1c = chem.cell.capacity_ah;
    let mut pack = sc.build_pack(chem).unwrap();
    let env = Env {
        t_ambient: sc.pack.initial_temp_k,
        t_coolant: None,
    };
    let mut out = Marks {
        err: [f64::NAN; 3],
        mark: f64::NAN,
        low: f64::INFINITY,
        sigma: [None; 3],
        est: [f64::NAN; 2],
    };
    // Integer step counts, so the marks are exact rather than accumulated floats.
    for n in 1..=7800_u32 {
        let demand = if n <= 600 {
            Demand::Current(i_1c)
        } else {
            Demand::Rest
        };
        let tele = pack.step(0.5, demand, &env);
        out.low = out.low.min((tele.soc_bms.unwrap() - tele.soc_true) * 100.0);
        match n {
            120 => out.mark = (tele.soc_bms.unwrap() - tele.soc_true) * 100.0,
            3600 => out.est[0] = tele.soc_bms.unwrap() * 100.0,
            7800 => out.est[1] = tele.soc_bms.unwrap() * 100.0,
            _ => {}
        }
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
    // The collapse, which the guided path's step 34 states in words because the page does
    // not show sigma: from the 5 points `initial_soc_sigma` boots it with to about one by
    // 30 s, while the estimate falls fifteen points the wrong way, and on down under load.
    // What shrinks it is WHERE the estimate went, not that the readings disagreed — see
    // `the_filter_grows_sure_of_itself_where_the_curve_is_steep`.
    assert!((f.sigma[0].unwrap() - 1.1719).abs() <= TOL_PTS);
    assert!((f.sigma[1].unwrap() - 0.1396).abs() <= TOL_PTS);
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

/// Why the wrong-model filter grows sure of itself: the covariance update reads the curve's
/// slope at the ESTIMATE and never the innovation, so a filter whose estimate sits below the
/// `[ocv]` table's 0.45 node, where LFP is several times steeper than on its plateau, shrinks
/// its sigma fast whether or not its readings agree with it. The control is the exact-model
/// file with no boot error, so every reading agrees: started on the plateau (60 %) it is
/// still at 4.16 points at 30 s — identical to the shipped run with its 3-point boot error —
/// and started at 44 %, below the node, it is at 0.42, further down than the wrong model's
/// 1.17. The guided path's step 34 names this test for that sentence.
#[test]
fn the_filter_grows_sure_of_itself_where_the_curve_is_steep() {
    let at = |soc0: f64, boot: f64| {
        let mut sc = scenario("lfp_gauge_filter");
        sc.pack.initial_soc = soc0;
        sc.pack.bms.as_mut().unwrap().initial_soc_error = boot;
        run_scenario(sc)
    };
    let plateau = at(0.60, 0.0);
    let steep = at(0.44, 0.0);
    let shipped = run("lfp_gauge_filter");
    let wrong = run("lfp_gauge_filter_weak_cell");
    let s30 = |m: &Marks| m.sigma[0].unwrap();
    assert!(
        (s30(&plateau) - 4.1611).abs() <= TOL_PTS,
        "{}",
        s30(&plateau)
    );
    assert!((s30(&steep) - 0.4236).abs() <= TOL_PTS, "{}", s30(&steep));
    // To the fourth decimal rather than to the bit: the two estimates sit at different
    // charges, so the slope the update reads differs a little. What does not enter is the
    // three points of disagreement between them.
    assert!(
        (s30(&plateau) - s30(&shipped)).abs() <= TOL_PTS,
        "the boot error, and every disagreement it causes, does not move sigma: {} against {}",
        s30(&plateau),
        s30(&shipped)
    );
    assert!(
        steep.err[0].abs() < 0.2,
        "below the node with the model exact, the readings agree: {}",
        steep.err[0]
    );
    assert!(
        s30(&steep) < s30(&wrong) && s30(&wrong) < s30(&plateau) / 3.0,
        "agreeing readings on the steep part collapse it at least as far as the wrong model's"
    );
}

/// The control arm for the sodium-ion rows: the same twins on a chemistry with its
/// `[hysteresis]` section removed. The filter's end error falls from −0.563 to −0.105
/// points mid-range and from −1.002 to −0.064 near empty, so the loop is most of what it
/// lands short by; what is left is the 20 mA offset. (With the offset removed as well the
/// mid-range run ends at −0.002, measured in the slice's scratch harness, not here.)
#[test]
fn on_sodium_ion_the_loop_is_what_the_filter_lands_short_by() {
    let with_loop = run("na_ion_gauge_filter");
    let without = run_with(scenario("na_ion_gauge_filter"), true);
    assert!(
        without.err[2].abs() < 0.15 && with_loop.err[2] < -0.5,
        "mid-range: {} without the loop against {} with it",
        without.err[2],
        with_loop.err[2]
    );
    let with_loop = run("na_ion_gauge_low_filter");
    let without = run_with(scenario("na_ion_gauge_low_filter"), true);
    assert!(
        without.err[2].abs() < 0.1 && with_loop.err[2] < -0.95,
        "near empty: {} without the loop against {} with it",
        without.err[2],
        with_loop.err[2]
    );
}

/// Why the weak-cell filter stops at −6.67 points: it is pinned on the `[ocv]` table's
/// 0.45 breakpoint. Above it the plateau is 0.057 V per unit and the voltage's pull on the
/// confident filter is weaker than the 20 mA offset's push down; below it the curve is
/// 0.297 V per unit and the pull wins. So the estimate sits on 45.00 % from the half hour to
/// the end. Take the offset away and it crosses the node and keeps creeping up — still more
/// than six points low after the hour, because the collapsed sigma is what makes it slow.
/// Both halves are true, and the end figure is a fact about the node, not about the filter
/// having converged on something.
#[test]
fn on_lfp_the_wrong_model_is_held_at_a_table_node_by_the_offset() {
    let shipped = run("lfp_gauge_filter_weak_cell");
    for est in shipped.est {
        assert!(
            (est - 45.0).abs() < 0.01,
            "the shipped arm sits on the 0.45 node from 1800 s to the end: {est}"
        );
    }
    let mut sc = scenario("lfp_gauge_filter_weak_cell");
    sc.pack.bms.as_mut().unwrap().current_offset_a = 0.0;
    let no_offset = run_scenario(sc);
    assert!(
        no_offset.est[1] > 45.3 && no_offset.est[1] - no_offset.est[0] > 0.3,
        "without the offset it crosses the node and keeps moving: {:?}",
        no_offset.est
    );
    assert!(
        no_offset.err[2] < -6.0,
        "and is still confidently wrong after the hour: {}",
        no_offset.err[2]
    );
}

/// `name` with the filter's `voltage_sigma_v` replaced and nothing else.
fn with_voltage_sigma(name: &str, sigma_v: f64) -> Marks {
    let mut sc = scenario(name);
    match &mut sc.pack.bms.as_mut().unwrap().estimator {
        EstimatorConfig::Ekf(ekf) => ekf.voltage_sigma_v = sigma_v,
        other => panic!("{name} runs {other:?}, not the filter"),
    }
    run_scenario(sc)
}

/// The guided path's steps 33 and 34 run the filter at the 10 mV `voltage_sigma_v` the
/// scenario header calls a hand-picked placeholder, and both steps say their result rides on
/// it and name this test. Measured at 10, 30 and 100 mV (and in the slice's scratch sweep at
/// 5, 15, 20 and 50, which fall in order between): a filter told to trust the voltage three
/// times less is close to the truth at step 33's mark and falls later in the pulse, ending the
/// rest about as far out; told to trust it ten times less it never falls far at any step, ends close and
/// inside its own error bar, and beats the counter. Loosening it costs the exact-model LFP
/// file almost nothing by the end of the rest (about a point before that, under load, which
/// no step claims), and costs the sodium-ion filter files something: both land further out
/// at the end of the rest. See `docs/plans/path-gauge-filter-steps.md`.
#[test]
fn the_lesson_rides_on_how_far_the_filter_trusts_the_voltage() {
    let weak = "lfp_gauge_filter_weak_cell";
    let shipped = with_voltage_sigma(weak, 0.010);
    let looser = with_voltage_sigma(weak, 0.030);
    let loosest = with_voltage_sigma(weak, 0.100);
    assert!(
        shipped.mark < -14.0,
        "the step 33 reading: {}",
        shipped.mark
    );
    // Three times less trust: close at the mark, wrong later, about as far out after the hour.
    assert!(
        looser.mark > -2.5 && looser.mark < -1.0,
        "close at the mark: {}",
        looser.mark
    );
    assert!(
        looser.err[1] < -9.0,
        "but falls later in the pulse: {}",
        looser.err[1]
    );
    assert!(
        (looser.err[2] - shipped.err[2]).abs() < 1.0,
        "and ends the rest about as far out: {} against {}",
        looser.err[2],
        shipped.err[2]
    );
    // Ten times less: never falls far, ends close, honest about it, ahead of the counter.
    assert!(
        loosest.low > -1.0,
        "never falls far, at any step of the run: {}",
        loosest.low
    );
    let sigma = loosest.sigma[2].unwrap();
    assert!(
        loosest.err[2].abs() < 0.5 && loosest.err[2].abs() < sigma,
        "end of the rest: {} inside its own sigma {sigma}",
        loosest.err[2]
    );
    let mut sc = scenario(weak);
    sc.pack.bms.as_mut().unwrap().estimator = EstimatorConfig::CoulombCount;
    let counter = run_scenario(sc);
    assert!(
        loosest.err[2].abs() < counter.err[2].abs(),
        "beats the counter: {} against {}",
        loosest.err[2],
        counter.err[2]
    );
    // What loosening costs. Almost nothing on the exact-model LFP file by the end of the rest ...
    let exact = with_voltage_sigma("lfp_gauge_filter", 0.010);
    let exact_loose = with_voltage_sigma("lfp_gauge_filter", 0.100);
    assert!(
        (exact_loose.err[2] - exact.err[2]).abs() < 0.3,
        "the exact-model file: {} against {}",
        exact_loose.err[2],
        exact.err[2]
    );
    // ... and something on the sodium-ion pair, which both land further out.
    for name in ["na_ion_gauge_filter", "na_ion_gauge_low_filter"] {
        let tight = with_voltage_sigma(name, 0.010);
        let loose = with_voltage_sigma(name, 0.100);
        assert!(
            loose.err[2] < tight.err[2] - 0.3,
            "{name}: {} at 100 mV against {} at 10",
            loose.err[2],
            tight.err[2]
        );
    }
}
