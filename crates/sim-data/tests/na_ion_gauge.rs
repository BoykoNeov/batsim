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

use sim_core::ecm::hysteresis_half_width_v;
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
    run_on(scenario_toml, chemistry(&id))
}

/// The same run against a chemistry the caller supplies, which is how the counterfactual
/// arm of the third test gets taken.
///
/// The scenario's own `chemistry` id is ignored here rather than checked: the whole point of
/// the caller is to hand this function a parameter set that is **not** what the file names —
/// the shipped one with `[hysteresis.width_over_soc]` removed. See
/// [`the_wider_loop_costs_the_gauge_more_than_the_steeper_curve_saves`], which is the only
/// caller that passes anything but the file's own.
fn run_on(scenario_toml: &str, chem: ChemistryParams) -> Arm {
    let scenario = sim_data::parse_scenario(scenario_toml).expect("scenario parses");
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

/// **The loop is not one width, and lower down the curve it costs the gauge more than the
/// steeper curve there gives back.**
///
/// `scenarios/na_ion_gauge_low.toml` is `na_ion_gauge_corrects.toml` with one field changed
/// — `pack.initial_soc` — so the same discharge and the same rest leave the cell near empty
/// instead of mid-range. Guided-path step 32 is this measurement with a reader in front of
/// it; `docs/plans/path-wider-loop-step.md` is the slice.
///
/// # Why this test exists, and what it holds that nothing else does
///
/// The slice that shipped `[hysteresis.width_over_soc]` recorded that **the cited
/// magnitudes were held by nothing but a provenance note**: cutting the shipped `4.00`
/// multiplier to `3.00` passed the whole suite and fired nothing. The guided path's claims
/// now redden on that — the estimate row moves 0.191 points — and this test holds the two
/// halves of the mechanism *separately*, which a claim on an outcome cannot:
///
///   * the two **loop half-widths**, which are what the table decides, and
///   * the two **curve slopes**, which are what the `[ocv]` table decides.
///
/// Separately rather than as a ratio, because a ratio is satisfied by both halves moving
/// together, which is exactly what a re-fit of this chemistry would do.
///
/// # And the counterfactual, which only a test can take
///
/// The last assertions build the shipped chemistry **and** a copy with the table removed, in
/// one process, and run the same trajectory through both. Removing it does not shrink the
/// effect — it reverses it: the gap near empty becomes smaller than the mid-range one,
/// because the steeper curve is then the only thing that differs between the two runs. That
/// is the property a perturbation of the *file* structurally cannot measure, and it is why
/// the lesson's comparison is a statement about this table rather than about fuel gauges
/// near empty in general.
#[test]
fn the_wider_loop_costs_the_gauge_more_than_the_steeper_curve_saves() {
    let low_toml = include_str!("../../../scenarios/na_ion_gauge_low.toml");
    let mid_toml = include_str!("../../../scenarios/na_ion_gauge_corrects.toml");
    let chem = chemistry("na_ion_18650_generic");
    let hyst = chem
        .hysteresis
        .clone()
        .expect("Na-ion declares [hysteresis]");

    let low = run(low_toml);
    let mid = run(mid_toml);

    // The half-width each run rests in — `scale_v` times the table, read at the charge state
    // the run actually ended at rather than at the one the file starts from.
    let half_width = |arm: &Arm| hysteresis_half_width_v(&hyst, arm.soc_true_end) * 1000.0;
    let (w_low, w_mid) = (half_width(&low), half_width(&mid));

    println!("                     near empty      mid-range");
    println!(
        "  soc (true)         {:>9.4} %    {:>9.4} %",
        low.soc_true_end * 100.0,
        mid.soc_true_end * 100.0
    );
    println!(
        "  terminal           {:>9.6} V    {:>9.6} V",
        low.v_rest_end, mid.v_rest_end
    );
    println!("  loop half-width    {w_low:>9.4} mV   {w_mid:>9.4} mV");
    println!(
        "  curve slope        {:>9.4}      {:>9.4}",
        low.slope_at_rest, mid.slope_at_rest
    );
    println!(
        "  estimate - truth   {:>+9.6} pts  {:>+9.6} pts",
        low.gap_at_end, mid.gap_at_end
    );

    // Where each run rests. Not decoration: every quantity below is read AT these charge
    // states, and a scenario edit that moved either would move all of them at once.
    assert!(
        (low.soc_true_end - 1.0 / 6.0).abs() < 1e-9,
        "the near-empty run rests at {:.6}, not the 16.6667 % this test's numbers are read \
         at — 300 s at 1 C out of a 1.4558 A.h cell is 8.3333 points from a start of 0.25",
        low.soc_true_end
    );
    assert!(
        (mid.soc_true_end - 0.5166666666666667).abs() < 1e-9,
        "the mid-range run rests at {:.6}, not 51.6667 %",
        mid.soc_true_end
    );

    // THE TWO HALF-WIDTHS, which is what the table decides. The multiplier at 16.6667 % is
    // 4.00 - 3.00 * (0.166667 / 0.35) = 2.5714 by linear interpolation between the file's
    // nodes, and it is exactly 1 anywhere at or above the 0.35 breakpoint.
    assert!(
        (w_low - 25.7143).abs() < 1e-3,
        "the loop is {w_low:.4} mV wide where the near-empty run rests, against the 25.7143 \
         this lesson is written on. `[hysteresis.width_over_soc]` or `scale_v` has moved."
    );
    assert!(
        (w_mid - 10.0).abs() < 1e-9,
        "the loop is {w_mid:.4} mV wide where the mid-range run rests, and it should be \
         `scale_v` exactly: the multiplier is 1 everywhere at or above the breakpoint, so \
         this run's width is the file's scalar untouched."
    );

    // THE TWO SLOPES, which is what the `[ocv]` table decides, and the half of the
    // mechanism that works the OTHER way. The two readings sit inside the 0.15-0.20 and
    // 0.50-0.55 segments respectively, so each is one segment's slope and not an average
    // across a node.
    assert!(
        (low.slope_at_rest - 2.3960).abs() < 1e-3,
        "the curve reads {:.4} V per unit where the near-empty run rests, against 2.3960",
        low.slope_at_rest
    );
    assert!(
        (mid.slope_at_rest - 1.9100).abs() < 1e-3,
        "the curve reads {:.4} V per unit where the mid-range run rests, against 1.9100",
        mid.slope_at_rest
    );
    assert!(
        low.slope_at_rest > mid.slope_at_rest,
        "the guided path says the curve is STEEPER where the near-empty run rests, which is \
         what takes part of the wider loop back. It reads {:.4} there against {:.4}, so that \
         sentence is false.",
        low.slope_at_rest,
        mid.slope_at_rest
    );

    // WHAT THE READER SEES. Both estimates land under the truth, and the near-empty one by
    // about twice as much — 1.0 point against 0.5 on a panel that prints tenths.
    assert!(
        (low.gap_at_end + 0.975314).abs() < 5e-4,
        "the near-empty estimate lands {:+.6} points from the truth, against the -0.975314 \
         step 32 is written on",
        low.gap_at_end
    );
    assert!(
        (mid.gap_at_end + 0.494110).abs() < 5e-4,
        "the mid-range estimate lands {:+.6} points from the truth, against the -0.494110 \
         steps 30 and 31 are written on",
        mid.gap_at_end
    );

    // THE COUNTERFACTUAL. The same near-empty trajectory against a chemistry that is the
    // shipped one in every respect but the width table.
    let mut flat = chem.clone();
    flat.hysteresis
        .as_mut()
        .expect("Na-ion declares [hysteresis]")
        .width_over_soc = None;
    let low_flat = run_on(low_toml, flat);
    println!(
        "  without the table  {:>+9.6} pts  (near empty)",
        low_flat.gap_at_end
    );

    assert!(
        (low_flat.gap_at_end + 0.401123).abs() < 5e-4,
        "with `[hysteresis.width_over_soc]` removed the near-empty estimate lands {:+.6} \
         points from the truth, against the -0.401123 this test measured",
        low_flat.gap_at_end
    );
    assert!(
        low_flat.gap_at_end.abs() < mid.gap_at_end.abs(),
        "with the width table removed the near-empty gap is {:.6} points and the mid-range \
         gap is {:.6}. The lesson says the table is what makes the near-empty run the worse \
         of the two — without it the steeper curve should make it the BETTER one — and this \
         run does not show that.",
        low_flat.gap_at_end.abs(),
        mid.gap_at_end.abs()
    );
    assert!(
        low.gap_at_end.abs() > low_flat.gap_at_end.abs() * 2.0,
        "the shipped table more than doubles the near-empty gap ({:.6} against {:.6}) or it \
         does not, and here it does not",
        low.gap_at_end.abs(),
        low_flat.gap_at_end.abs()
    );
}
