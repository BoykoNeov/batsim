//! The fuel gauge that can correct itself, and the one thing it still cannot resolve.
//!
//! Two scenario files that differ in exactly one field — the chemistry — driven through the
//! same demand programme. `scenarios/na_ion_gauge_corrects.toml` and
//! `scenarios/lfp_gauge_declines.toml`.
//!
//! Nothing here is a new mechanism. `Bms`'s open-circuit correction and its
//! `min_ocv_slope_v_per_soc` gate have shipped since Phase 2, and the guided path already
//! teaches the half where the gate refuses. What is measured here is that a **parameter
//! file alone** flips the outcome, and by how much — which is what makes this chemistry
//! worth a lesson rather than just worth loading.
//!
//! The numbers printed by these tests are the ones quoted in the two scenario files'
//! headers. Run them with
//! `cargo test -p sim-data --test na_ion_gauge -- --nocapture`.

use sim_core::{ChemistryParams, Demand, Env, Pack};

const DT: f64 = 0.5;
const DISCHARGE_S: f64 = 300.0;
// An hour. Not a round number picked for tidiness: the slow RC pair of this cell has a
// time constant of 365 s, it holds 31.5 mV at 1 C, and it relaxes in the SAME direction the
// hysteresis loop pushes. At 900 s of rest 2.6 mV of it is still there, which is a fifth of
// the residual the second test is about — enough to make the loop look bigger than it is.
// An hour is ten time constants and leaves under a microvolt.
const REST_S: f64 = 3600.0;
const ROOM_K: f64 = 298.15;
/// The current sensor's offset, identical in both scenario files \[A\].
const OFFSET_A: f64 = 0.02;

fn chemistry(id: &str) -> ChemistryParams {
    let text = match id {
        "na_ion_18650_generic" => include_str!("../../../chemistries/na_ion_18650_generic.toml"),
        "lfp_26650_generic" => include_str!("../../../chemistries/lfp_26650_generic.toml"),
        other => panic!("this test knows nothing about `{other}`"),
    };
    sim_data::parse_chemistry(text).expect("shipped chemistry loads and validates")
}

/// One arm's story: where truth and belief were at each of the three interesting instants.
#[derive(Clone, Copy, Debug)]
struct Arm {
    /// Estimate minus truth at t = 0, in points of charge.
    gap_at_start: f64,
    /// The same, at the instant the discharge stops.
    gap_at_rest_start: f64,
    /// The same, at the end of the rest.
    gap_at_end: f64,
    /// True charge state at the end, for the record.
    soc_true_end: f64,
    /// Rested terminal voltage at the end \[V\] — one cell, so this is what the BMS's one
    /// group sensor reads.
    v_rest_end: f64,
    /// The local slope of the open-circuit table at that voltage \[V per unit of charge\].
    /// This is the number the gate is compared against, computed the way `Bms` computes it.
    slope_at_rest: f64,
    /// The charge state that inverting the table at that voltage reports.
    soc_from_ocv: f64,
    /// The gate this run was configured with \[V per unit of charge\].
    gate: f64,
}

/// Discharge at 1 C, then rest, and report the gap at three instants.
///
/// The demand programme is deliberately the one a reader can drive from the page: one
/// number in the demand box, then stop. Nothing here is scheduled in the scenario file.
fn run(scenario_toml: &str) -> Arm {
    let scenario = sim_data::parse_scenario(scenario_toml).expect("scenario parses");
    let id = match scenario.chemistry_source() {
        sim_data::ChemistrySource::Id(id) => id.to_owned(),
        sim_data::ChemistrySource::Inline(_) => panic!("these scenarios name a chemistry by id"),
    };
    let chem = chemistry(&id);
    let one_c = chem.cell.capacity_ah;
    let mut pack = scenario.build_pack(chem.clone()).expect("pack builds");
    let env = Env {
        t_ambient: ROOM_K,
        t_coolant: None,
    };

    let gap = |pack: &mut Pack, env: &Env| {
        let tel = pack.step(0.0, Demand::Rest, env);
        let bms = tel.soc_bms.expect("these scenarios configure a BMS");
        ((bms - tel.soc_true) * 100.0, tel.soc_true, tel.v_terminal)
    };

    let (gap_at_start, _, _) = gap(&mut pack, &env);
    for _ in 0..(DISCHARGE_S / DT) as usize {
        pack.step(DT, Demand::Current(one_c), &env);
    }
    let (gap_at_rest_start, _, _) = gap(&mut pack, &env);
    for _ in 0..(REST_S / DT) as usize {
        pack.step(DT, Demand::Rest, &env);
    }
    let (gap_at_end, soc_true_end, v_rest_end) = gap(&mut pack, &env);

    // Exactly what `Bms::update_estimate` does with the group reading, on the same table.
    // One cell in one group, and no voltage-sensor fault, so the terminal reading above IS
    // the group reading. Recomputed here rather than trusted: the whole lesson is which
    // side of the gate each chemistry falls on, and a lesson may not assert that from the
    // outcome it is supposed to explain.
    let (soc_from_ocv, slope_at_rest) = sim_core::ecm::ocv_invert(&chem.ocv, v_rest_end);

    Arm {
        gap_at_start,
        gap_at_rest_start,
        gap_at_end,
        soc_true_end,
        v_rest_end,
        slope_at_rest,
        soc_from_ocv,
        gate: scenario_gate(scenario_toml),
    }
}

/// The gate this scenario configures, read back out of the file rather than restated.
fn scenario_gate(scenario_toml: &str) -> f64 {
    for line in scenario_toml.lines() {
        if let Some(rest) = line.trim().strip_prefix("min_ocv_slope_v_per_soc") {
            let rhs = rest
                .trim_start()
                .strip_prefix('=')
                .expect("a TOML assignment");
            return rhs
                .split('#')
                .next()
                .expect("split always yields one piece")
                .trim()
                .parse()
                .expect("the gate is a number");
        }
    }
    panic!("this scenario does not configure `min_ocv_slope_v_per_soc`");
}

/// **The correction fires on this cell and refuses on the flat one**, and the only
/// difference between the two runs is which parameter file was loaded.
///
/// The control arm is not decoration here, it is the whole measurement: coulomb counting,
/// the sensor offset, the boot error and the rest are identical, so a gap that closes in one
/// run and not the other is attributable to the open-circuit curve and to nothing else.
#[test]
fn the_gauge_corrects_on_sodium_ion_and_declines_on_lfp() {
    let na = run(include_str!(
        "../../../scenarios/na_ion_gauge_corrects.toml"
    ));
    let fe = run(include_str!("../../../scenarios/lfp_gauge_declines.toml"));

    let row = |name: &str, a: Arm| {
        println!(
            "  {name:10}  start {:+6.3}   at rest {:+6.3}   after rest {:+6.3}   (soc true {:.4})",
            a.gap_at_start, a.gap_at_rest_start, a.gap_at_end, a.soc_true_end
        );
    };
    println!(
        "estimate minus truth, in points of charge; 1 C for {DISCHARGE_S:.0} s then rest for \
         {REST_S:.0} s at dt = {DT}"
    );
    row("sodium-ion", na);
    row("LFP", fe);
    // The discriminating measurement, and the reason this pair is a lesson rather than a
    // coincidence: what the estimator computes at the voltage each cell rests at, against
    // the one number that decides whether it acts on it.
    let gate_row =
        |name: &str, a: Arm| {
            println!(
            "  {name:10}  rests at {:.4} V -> slope {:.4} V/unit against a gate of {:.2} ({}), \
             and the table inverts to {:.2} % against a truth of {:.2} %",
            a.v_rest_end,
            a.slope_at_rest,
            a.gate,
            if a.slope_at_rest >= a.gate { "corrects" } else { "declines" },
            a.soc_from_ocv * 100.0,
            a.soc_true_end * 100.0,
        );
        };
    // What the current sensor's offset ALONE is worth over the rest, which is the other
    // thing that moves an estimate while nothing is happening. Printed for both arms
    // because it is the number the LFP arm's movement has to be checked against: an
    // estimate that drifts is not an estimate that corrected.
    let offset_drift = |cap_ah: f64| 100.0 * OFFSET_A * REST_S / (3600.0 * cap_ah);
    println!(
        "  offset alone over the rest: sodium-ion {:+.3}   LFP {:+.3} points",
        -offset_drift(chemistry("na_ion_18650_generic").cell.capacity_ah),
        -offset_drift(chemistry("lfp_26650_generic").cell.capacity_ah),
    );
    println!("what the estimator finds when it looks:");
    gate_row("sodium-ion", na);
    gate_row("LFP", fe);

    assert!(
        na.slope_at_rest >= na.gate,
        "the sodium-ion curve reads {:.4} V per unit where this run rests, under a gate of \
         {:.2} - the estimator would decline and this lesson has no subject",
        na.slope_at_rest,
        na.gate
    );
    assert!(
        fe.slope_at_rest < fe.gate,
        "the LFP curve reads {:.4} V per unit where this run rests, over a gate of {:.2} - \
         the control arm would correct too and the comparison says nothing",
        fe.slope_at_rest,
        fe.gate
    );

    // Both arms boot with the same error and both drift the same way while the current
    // flows: nothing has distinguished them yet.
    assert!(
        (na.gap_at_start - fe.gap_at_start).abs() < 0.01,
        "the two arms must boot with the same error, and differ by {:.3} points",
        na.gap_at_start - fe.gap_at_start
    );

    // The rest is where they part, and the thing to measure is NOT how far each estimate
    // moved. Both move: the current sensor reads high, so coulomb counting keeps draining
    // the estimate even with the pack open, and on this run that happens to be movement
    // TOWARDS the truth. What separates a correction from more drift is where the estimate
    // ends up — on the curve, or wherever counting left it.
    let na_onto_curve = (na.gap_at_end / 100.0 + na.soc_true_end - na.soc_from_ocv) * 100.0;
    let fe_onto_curve = (fe.gap_at_end / 100.0 + fe.soc_true_end - fe.soc_from_ocv) * 100.0;
    println!(
        "  estimate minus what the table says at the resting voltage:  sodium-ion \
         {na_onto_curve:+.3}   LFP {fe_onto_curve:+.3} points"
    );
    assert!(
        na_onto_curve.abs() < 0.1,
        "the sodium-ion estimate ended {na_onto_curve:+.3} points off the reading it is \
         supposed to have converged onto"
    );
    assert!(
        fe_onto_curve.abs() > 1.0,
        "the LFP estimate ended {fe_onto_curve:+.3} points from the table's reading, which \
         is close enough to look like it corrected"
    );

    // And the attribution for the arm that did not correct: everything it did move is the
    // sensor offset. The tolerance covers the noise draw and one step of frame lag — the
    // first rest step's estimate is updated from the frame sampled at the end of the last
    // DISCHARGE step, so half a second of the discharge current is counted after the pack
    // has opened.
    let fe_moved = fe.gap_at_end - fe.gap_at_rest_start;
    let fe_predicted = -100.0 * OFFSET_A * REST_S / (3600.0 * 2.303451);
    println!(
        "  LFP moved {fe_moved:+.3} points across the rest; its offset alone is worth \
         {fe_predicted:+.3}"
    );
    assert!(
        (fe_moved - fe_predicted).abs() < 0.05,
        "the LFP arm moved {fe_moved:+.3} points where its current sensor's offset accounts \
         for {fe_predicted:+.3} — something else is moving that estimate"
    );
}

/// **The correction lands, and it lands short — by about what the hysteresis loop is worth.**
///
/// The estimator inverts the `[ocv]` table, which is the *midline* of the loop. A cell that
/// was last discharged rests below that midline, so the voltage it offers maps to a charge
/// state lower than the truth. The residual error after a converged correction should
/// therefore be negative and of order `scale_v` divided by the curve's slope, which is
/// 0.524 points here.
///
/// NOT the 1.21 points `na_ion_chemistry.rs` measures, and the first draft of this comment
/// said it was. That figure is the distance between the two BRANCHES — a cell charged to
/// 45 % against one discharged to it — and is twice this one by construction, because this
/// is one branch against the midline the table draws between them. Two quantities a factor
/// of two apart, both real, and only one of them is what a gauge inverting the table pays.
///
/// This is the second half of the lesson and the reason the first half is not just good
/// news: the steep curve that makes the gauge readable is also what converts the cell's
/// direction memory into charge nobody can resolve.
#[test]
fn the_landed_correction_is_short_by_the_hysteresis_loop() {
    let na = run(include_str!(
        "../../../scenarios/na_ion_gauge_corrects.toml"
    ));
    let chem = chemistry("na_ion_18650_generic");
    let hyst = chem.hysteresis.expect("Na-ion declares [hysteresis]");

    // Local slope of the open-circuit curve where the run ended, in volts per unit charge.
    let soc = na.soc_true_end;
    let slope = (sim_core::ecm::ocv_lookup(&chem.ocv, soc + 0.01)
        - sim_core::ecm::ocv_lookup(&chem.ocv, soc - 0.01))
        / 0.02;
    let predicted_points = -hyst.scale_v / slope * 100.0;

    println!("after the correction converges:");
    println!("  residual gap   {:+.3} points", na.gap_at_end);
    println!("  loop is worth  {predicted_points:+.3} points at this charge state");

    assert!(
        na.gap_at_end < 0.0,
        "a cell that was last discharged rests BELOW the midline the estimator inverts, so \
         the residual must be negative, and it is {:+.3}",
        na.gap_at_end
    );
    // A band, not a fit: the correction has a finite gain and a finite rest to converge in,
    // and the RC pairs are not fully settled either, so the residual is the loop's worth
    // plus whatever has not relaxed yet. What is asserted is that the loop explains the
    // right order of it.
    assert!(
        (0.3..3.0).contains(&(na.gap_at_end / predicted_points)),
        "the residual is {:+.3} points against a loop worth {predicted_points:+.3} — the \
         hysteresis does not explain it",
        na.gap_at_end
    );
}
