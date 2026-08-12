//! Lead-acid and Peukert: what the equivalent circuit could not do, and what closed it.
//!
//! `CLAUDE.md` promises a chemistry is data, not code, and names lead-acid with Peukert.
//! `docs/plans/lead-acid-data-only.md` measured how far that promise carried and found it
//! stopping short in a way no parameter could fix: ohmic sag is flat-then-knee, Peukert is
//! a power law, and below 1C the shipped model reproduced **none** of the real capacity
//! loss. The fix was a code change after all — one extra state per cell, a `[diffusion]`
//! section, `docs/plans/diffusion-overpotential.md`.
//!
//! # This file runs both models, and the second one is a control arm
//! [`pba`] is the shipped chemistry. [`pba_ohmic_only`] is the same file with the
//! `[diffusion]` section removed, which is exactly the model as it shipped before. The
//! original findings are kept and re-run against that arm rather than deleted, for three
//! reasons: the measurements are still true and were expensive; the contrast between the
//! two arms *is* the pedagogy; and the stripped arm is a direct check that the engine's
//! no-section path really is the pre-change behaviour rather than something close to it.
//!
//! Run the tables with `cargo test -p sim-data --test lead_acid_rate -- --nocapture`.

use sim_core::{
    ecm::ocv_lookup, CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter,
    ThermalConfig,
};

/// The shipped chemistry, `[diffusion]` section and all.
fn pba() -> ChemistryParams {
    let text = include_str!("../../../chemistries/pba_agm_2v_generic.toml");
    sim_data::parse_chemistry(text).expect("lead-acid chemistry loads and validates")
}

/// The same cell with the diffusion term removed: the model exactly as it shipped before
/// `docs/plans/diffusion-overpotential.md`, and the arm every pre-existing finding in this
/// file is now measured against.
fn pba_ohmic_only() -> ChemistryParams {
    let mut chem = pba();
    chem.diffusion = None;
    chem
}

/// A bare cell: no BMS (so nothing derates the demand), no aging (so capacity does not
/// move during a 20-hour discharge), and **isothermal** — the rate dependence being
/// measured must not be tangled with self-heating, which is a separate question.
///
/// Isothermal still matters, and by less than a first estimate suggested: the diffusion
/// overpotential raises this cell's **peak** heat at 3C by 1.20× and its average over a
/// discharge by 1.07×, and *lowers* the total, because the run ends sooner. Measured by
/// [`the_term_adds_heat_and_the_peak_is_not_the_average`], after an estimate of "roughly
/// doubles" was written here and turned out to be four times the truth.
fn config() -> PackConfig {
    PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc: 1.0,
        initial_temp_k: 298.15,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        cell_model: CellModelConfig::Ecm,
    }
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Discharge at a constant C-rate to the chemistry's `v_min` and **bracket** the amp-hours
/// delivered, returning `(lower, upper)`.
///
/// The bracket is not decoration. The step that carries the terminal below `v_min` has
/// already moved charge — `soc` advanced and current flowed — but a real cutoff happens
/// *partway through* it, so neither excluding nor including that step is the answer:
/// excluding it undercounts, including it overcounts, and the truth is between. The two
/// bounds differ by exactly that step's charge, `I·dt`.
///
/// This matters because the error is **systematic in one direction at every resolution**,
/// which is precisely what a `dt`-versus-`dt/4` convergence check cannot detect — both
/// runs are biased the same way and agree with each other. Reporting the bracket makes the
/// bias visible instead; `rate_sweep_is_not_timestep_limited` asserts on its width.
fn delivered_bracket(chem: &ChemistryParams, c_rate: f64, dt: f64) -> (f64, f64) {
    let i = c_rate * chem.cell.capacity_ah;
    let v_min = chem.cell.v_min;
    let mut pack = Pack::new(&config(), chem.clone()).expect("pack builds");
    let mut ah = 0.0;
    // Bound the loop by simulated time rather than trusting the cutoff: a 0.05C
    // discharge is 20 h, so 30 h of headroom cannot be reached by any rate here.
    let max_steps = (30.0 * 3600.0 / dt) as u64;
    for _ in 0..max_steps {
        let t = pack.step(dt, Demand::Current(i), &env());
        let crossed = t.v_terminal < v_min;
        let step_ah = t.i_actual * dt / 3600.0;
        if crossed {
            return (ah, ah + step_ah);
        }
        ah += step_ah;
    }
    (ah, ah)
}

/// The conservative end of [`delivered_bracket`]. Every table in this file quotes this
/// bound, so every figure here is short of the true delivered capacity by at most one
/// step's charge — stated in the plan docs rather than left implicit.
fn delivered_ah(chem: &ChemistryParams, c_rate: f64, dt: f64) -> f64 {
    delivered_bracket(chem, c_rate, dt).0
}

/// Peukert's law as the reference curve, expressed as delivered capacity relative to a
/// reference rate. `n = 1.1` is the AGM class; flooded runs 1.2–1.3.
fn peukert_fraction(c_rate: f64, c_ref: f64, n: f64) -> f64 {
    (c_ref / c_rate).powf(n - 1.0)
}

/// The rates every sweep in this file uses, reference first.
const RATES: [f64; 7] = [0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0];

/// Delivered capacity at every rate, as a fraction of the C/20 reference, plus the
/// reference itself in amp-hours.
fn sweep(chem: &ChemistryParams, dt: f64) -> (f64, Vec<(f64, f64, f64)>) {
    let base = delivered_ah(chem, RATES[0], dt);
    let rows = RATES
        .iter()
        .map(|&c| {
            let f = delivered_ah(chem, c, dt) / base;
            (c, f, peukert_fraction(c, RATES[0], 1.1))
        })
        .collect();
    (base, rows)
}

fn print_sweep(label: &str, base: f64, rows: &[(f64, f64, f64)]) {
    println!("\n{label}   (C/20 reference: {base:.4} Ah)");
    println!(
        "{:<8} {:>10} {:>12} {:>10}",
        "rate", "model %", "Peukert1.1", "error"
    );
    for &(c, f, p) in rows {
        println!(
            "{:<8} {:>9.1}% {:>11.1}% {:>+9.1}",
            format!("{c:.2}C"),
            f * 100.0,
            p * 100.0,
            (f - p) * 100.0
        );
    }
}

#[test]
fn lead_acid_chemistry_loads_and_validates() {
    let chem = pba();
    assert_eq!(chem.meta.id, "pba_agm_2v_generic");
    assert_eq!(chem.n_rc(), 1);
    assert!((chem.cell.capacity_ah - 7.2).abs() < 1e-12);
    // The omission is the point: lead-acid has no lithium to plate, so inventing the
    // plating constants would be a fabricated number. See the file's closing comment.
    assert!(
        chem.safety.is_none(),
        "[safety] models lithium plating and must stay absent for lead-acid"
    );
    // And the presence is the point of this slice. It is the only shipped chemistry with
    // one; both lithium files leave it out, which is what keeps their trajectories
    // bit-identical across SNAPSHOT_VERSION 17.
    let diffusion = chem
        .diffusion
        .expect("lead-acid ships a [diffusion] section — it is the whole slice");
    assert!(
        diffusion.tau_s > 1800.0,
        "lead-acid acid diffusion is an HOURS process; a sub-half-hour tau would be a \
         second RC pair wearing the section's name, got {} s",
        diffusion.tau_s
    );
    assert!(
        (diffusion.max_overpotential_v - (ocv_lookup(&chem.ocv, 0.0) - chem.reversal.floor_v))
            .abs()
            < 1e-12,
        "the ceiling is DERIVED as OCV(0) − reversal.floor_v, so that a saturated cell and \
         a fully over-discharged one land on the same voltage; got {} V against {} V",
        diffusion.max_overpotential_v,
        ocv_lookup(&chem.ocv, 0.0) - chem.reversal.floor_v
    );

    // Aging is present and its SOC stress is *inverted* relative to lithium: a lead-acid
    // cell left flat sulfates, where a lithium cell left full degrades. Same knob, opposite
    // sign, and no code change — which is the whole "chemistry is data" claim in one line.
    let aging = chem
        .aging
        .as_ref()
        .expect("lead-acid ships an [aging] section");
    assert!(
        aging.cal_soc_stress[0] > aging.cal_soc_stress[1],
        "lead-acid calendar fade must be worst at LOW soc (sulfation), unlike lithium"
    );
}

/// The pedagogical claim, pinned so the guided path cannot inherit the wrong version of it.
///
/// Lead-acid is **not** steeper than LFP overall — it spans 180 mV against LFP's 1600 mV, so
/// LFP moves 8.9× *more* voltage end to end. Nor is it uniform in absolute terms: its slope
/// varies 1.7× across the range, because the electrolyte is a reactant and the curve is
/// concave. What is true is *comparative*: 1.7× against LFP's 248×, and lead-acid's flattest
/// decile still beats LFP's flattest several times over. It never goes flat, and that — not
/// steepness — is why a resting voltmeter reads charge on lead-acid and cannot on LFP.
#[test]
fn lead_acid_ocv_is_uniform_not_steep() {
    let pba = pba();
    let lfp =
        sim_data::parse_chemistry(include_str!("../../../chemistries/lfp_26650_generic.toml"))
            .expect("LFP loads");

    let slope = |c: &ChemistryParams, lo: f64, hi: f64| {
        (ocv_lookup(&c.ocv, hi) - ocv_lookup(&c.ocv, lo)) / (hi - lo)
    };

    // Full span: LFP moves far MORE voltage, not less. The common phrasing is backwards.
    let pba_span = slope(&pba, 0.0, 1.0);
    let lfp_span = slope(&lfp, 0.0, 1.0);
    assert!(
        lfp_span > 8.0 * pba_span,
        "LFP spans {lfp_span:.3} V/soc vs lead-acid {pba_span:.3}; the 'lead-acid is steeper' \
         claim is false end-to-end and must not be written anywhere"
    );

    // The "inside LFP's plateau the ordering reverses" claim is deliberately NOT asserted
    // over a hand-picked window such as 0.45–0.65: LFP's table has breakpoints at 0.55 and
    // 0.65, so such a window straddles OCV nodes and measures the nodes as much as the
    // curve. The node-independent form of the same claim — flattest decile against flattest
    // decile — is asserted below and is what the plan doc quotes.

    // Uniformity is the property being claimed, and it must be measured decile by decile
    // rather than over wide windows: averaging across several table segments hides exactly
    // the variation being tested. (An earlier draft of this test claimed lead-acid's slope
    // was uniform to within 2 % — an artifact of the windows it chose. It is not uniform;
    // it is *comparatively* uniform, which is a weaker and true claim.)
    let deciles = |c: &ChemistryParams| -> (f64, f64) {
        let s: Vec<f64> = (0..10)
            .map(|i| {
                let lo = i as f64 / 10.0;
                slope(c, lo, lo + 0.1)
            })
            .collect();
        (
            s.iter().cloned().fold(f64::INFINITY, f64::min),
            s.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (pba_lo, pba_hi) = deciles(&pba);
    let (lfp_lo, lfp_hi) = deciles(&lfp);

    assert!(
        pba_hi / pba_lo < 2.5,
        "lead-acid's OCV slope must vary little across the range; got {:.1}x \
         ({pba_lo:.4} to {pba_hi:.4} V/soc)",
        pba_hi / pba_lo
    );
    assert!(
        lfp_hi / lfp_lo > 50.0,
        "LFP's OCV slope must vary hugely across the range; got {:.1}x",
        lfp_hi / lfp_lo
    );
    // The teachable consequence: lead-acid never goes flat. Its *worst* decile is still
    // several times more informative than LFP's worst, which is the dead zone a
    // voltage-based charge estimate falls into.
    assert!(
        pba_lo > 3.0 * lfp_lo,
        "lead-acid's flattest decile ({pba_lo:.4} V/soc) must still beat LFP's \
         ({lfp_lo:.4} V/soc) by several times"
    );
}

// ---------------------------------------------------------------------------------
// The control arm: the model as it shipped before the diffusion term.
// ---------------------------------------------------------------------------------

/// **Without the diffusion term the shape is wrong, and no parameter fixes it.**
///
/// The finding `docs/plans/lead-acid-data-only.md` was written for, kept and re-run against
/// the stripped chemistry. Asserted as **structure**, not as numbers on a curve, because
/// every constant behind it is a labelled placeholder: ohmic sag is *flat-then-knee* where
/// Peukert is a smooth power law, so this arm reproduces none of the rate loss at low rate
/// however the resistances are chosen.
///
/// It is also, now, the check that the engine's no-`[diffusion]` path is the *old* path.
/// Every assertion here passed before the section existed and passes with it stripped; if
/// one ever fails, the term is leaking into chemistries that did not ask for it.
#[test]
fn without_the_diffusion_term_the_shape_is_wrong() {
    let chem = pba_ohmic_only();
    let (base, rows) = sweep(&chem, 1.0);
    print_sweep("CONTROL ARM — [diffusion] stripped", base, &rows);

    // 1. Below 1C the model reproduces essentially NONE of the real loss. This is the
    //    structural failure: `I·R` is negligible against the OCV headroom there, so the
    //    curve is flat where Peukert has already given up a quarter of its capacity.
    for &(c, f, p11) in &rows {
        if c <= 1.0 {
            assert!(
                f > 0.98,
                "at {c:.2}C the ohmic-only ECM still delivers ~full capacity by \
                 construction; got {:.1}% (real lead-acid: {:.1}%)",
                f * 100.0,
                p11 * 100.0
            );
        }
    }

    // 2. Above 1C it does produce a loss, in the right direction — so the mechanism is
    //    present, just too weak and too late.
    let (_, f3, p3) = *rows.last().expect("3C row");
    assert!(
        f3 < 0.95,
        "at 3C ohmic sag must cost real capacity; got {:.1}%",
        f3 * 100.0
    );
    assert!(
        f3 > p3,
        "the ohmic-only arm must UNDER-produce the loss (got {:.1}% delivered vs \
         Peukert's {:.1}%); if this ever fails, the resistances have been tuned to fake a \
         diffusion limit",
        f3 * 100.0,
        p3 * 100.0
    );

    // 3. The headline, as a band rather than a point, because the inputs are placeholders.
    let captured = (1.0 - f3) / (1.0 - p3);
    println!(
        "\nover 0.05C -> 3.00C: real AGM loses {:.1}%, the ohmic-only arm loses {:.1}% \
         -> it reproduces {:.0}% of it\n",
        (1.0 - p3) * 100.0,
        (1.0 - f3) * 100.0,
        captured * 100.0
    );
    assert!(
        (0.25..0.70).contains(&captured),
        "the ohmic-only arm should reproduce roughly a third to two thirds of lead-acid's \
         rate-dependent capacity loss; got {:.0}%",
        captured * 100.0
    );
}

/// The objection the data-only slice had to answer: `[r0]`'s rise toward empty is a
/// placeholder, so isn't the gap above just the number I picked?
///
/// No, and this is the finding. Scaling **every** resistance until 3C lands on the Peukert
/// curve leaves the mid-range no better, because the two curves are different *shapes*:
/// ohmic sag is flat-then-knee (at low rate `I·R` is negligible against the OCV headroom),
/// while Peukert is a smooth power law that starts giving up capacity at the first rate
/// increase. Tuning moves the knee. It cannot remove the flat.
///
/// Run on the control arm, where it is the argument it always was. The whole point of the
/// diffusion term is that it does what no amount of this could.
#[test]
fn tuning_the_resistances_moves_the_knee_but_not_the_flat() {
    let base_chem = pba_ohmic_only();
    let scaled = |k: f64| {
        let mut c = pba_ohmic_only();
        for row in c.r0.ohms.iter_mut() {
            for v in row.iter_mut() {
                *v *= k;
            }
        }
        for pair in c.rc.iter_mut() {
            pair.r_ohms *= k;
            // Hold tau fixed so this is a pure resistance change, not a relaxation change.
            pair.c_farad /= k;
        }
        c
    };

    let rates = [0.1, 0.2, 0.5, 1.0, 2.0, 3.0];
    let worst_error = |chem: &ChemistryParams| {
        let base = delivered_ah(chem, 0.05, 1.0);
        rates
            .iter()
            .map(|&c| {
                let f = delivered_ah(chem, c, 1.0) / base;
                (c, (f - peukert_fraction(c, 0.05, 1.1)) * 100.0)
            })
            .fold((0.0f64, 0.0f64), |acc, (c, e)| {
                if e.abs() > acc.1.abs() {
                    (c, e)
                } else {
                    acc
                }
            })
    };

    // Find the scale that best matches Peukert at 3C — i.e. give the tuning its best shot.
    let (mut lo, mut hi) = (1.0f64, 8.0f64);
    let target = peukert_fraction(3.0, 0.05, 1.1);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        let c = scaled(mid);
        if delivered_ah(&c, 3.0, 1.0) / delivered_ah(&c, 0.05, 1.0) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let k = 0.5 * (lo + hi);
    let tuned = scaled(k);

    let (base_at, base_err) = worst_error(&base_chem);
    let (tuned_at, tuned_err) = worst_error(&tuned);
    let tuned_3c = delivered_ah(&tuned, 3.0, 1.0) / delivered_ah(&tuned, 0.05, 1.0);

    println!(
        "\nresistances x{k:.2} makes 3C land at {:.1}% against Peukert's {:.1}%",
        tuned_3c * 100.0,
        target * 100.0
    );
    println!("  worst error, untuned: {base_err:+.1} points at {base_at:.2}C");
    println!("  worst error, tuned:   {tuned_err:+.1} points at {tuned_at:.2}C\n");

    // The tuning does what it was asked to do at the single rate it was fitted on.
    assert!(
        (tuned_3c - target).abs() < 0.02,
        "the search should land 3C on the Peukert curve; got {:.1}% vs {:.1}%",
        tuned_3c * 100.0,
        target * 100.0
    );
    // And it buys almost nothing overall, which is the point.
    assert!(
        tuned_err.abs() > 0.8 * base_err.abs(),
        "fitting one rate must NOT fix the curve (worst error went {base_err:+.1} -> \
         {tuned_err:+.1} points). If this ever fails, ohmic sag has become an adequate \
         stand-in for a diffusion limit and the [diffusion] section is unnecessary."
    );
    // The residual sits in the middle of the range, where Peukert has already lost a
    // quarter of its capacity and ohmic sag has lost none.
    assert!(
        (0.2..=1.5).contains(&tuned_at),
        "the unfixable residual should sit mid-range, not at the extremes; got {tuned_at:.2}C"
    );
}

// ---------------------------------------------------------------------------------
// The shipped model.
// ---------------------------------------------------------------------------------

/// **With the diffusion term the engine tracks Peukert, and the improvement is an order
/// of magnitude.**
///
/// The measurement this slice exists for, and the one the fit was scored on. Both arms are
/// run here so the comparison is a subtraction rather than a claim, and the bound on the
/// stripped arm is asserted too — a change that quietly made the *control* better would
/// otherwise make this test easier to pass while proving less.
///
/// **The tolerance is derived from two measurements, not rounded up from one.** A
/// three-parameter fit reported at the rates it was fitted on measures itself, so the
/// number that means anything is the fit's held-out error: **3.28 points**. The engine is
/// not the harness that produced it — it reports the terminal from end-of-step state where
/// the harness tested it at the start of one — and that disagreement was measured too, at
/// **0.02 points** across the whole sweep. So the budget is 3.28 + 0.02, and the assertion
/// is set at **3.5**, which is that sum with a tenth of a point of headroom.
///
/// An earlier draft said 8.0 and justified it as "roughly twice the hold-out, plus room for
/// step ordering". Both inputs were already measured and neither is anywhere near what the
/// extra 4.5 points would have covered — which is the granularity fence
/// `docs/plans/path-tolerance-rule.md` names: a round number quietly re-licensing precision
/// the slice actually has, and enough slack to stay green with the term half working.
#[test]
fn the_diffusion_term_tracks_peukert() {
    let (base_on, rows_on) = sweep(&pba(), 1.0);
    let (base_off, rows_off) = sweep(&pba_ohmic_only(), 1.0);
    print_sweep("SHIPPED — [diffusion] active", base_on, &rows_on);

    let worst = |rows: &[(f64, f64, f64)]| {
        rows.iter()
            .map(|&(_, f, p)| (f - p).abs() * 100.0)
            .fold(0.0f64, f64::max)
    };
    let (on, off) = (worst(&rows_on), worst(&rows_off));
    println!("\nworst error against Peukert n=1.1:");
    println!("  [diffusion] stripped: {off:.1} points");
    println!("  [diffusion] active:   {on:.1} points\n");

    assert!(
        off > 20.0,
        "the control arm must still be badly wrong ({off:.1} points), or this test is \
         measuring a Peukert curve that has been made easy to hit"
    );
    assert!(
        on < 3.5,
        "the fitted term must track Peukert across 0.05C -> 3C; worst error {on:.2} points \
         against a budget of 3.28 (the fit's held-out error) + 0.02 (engine-vs-harness step \
         ordering) + 0.10 headroom"
    );
    assert!(
        on < 0.25 * off,
        "and it must be a different kind of answer, not a better tuning: {on:.1} points \
         against {off:.1}"
    );

    // The absolute anchor, which was in the fit's objective rather than left as a free
    // output — the specific under-constraint `docs/plans/diffusion-overpotential.md`
    // records getting wrong once. `capacity_ah` is the COULOMBIC capacity and the
    // datasheet's 7.2 Ah is a delivery to 1.75 V at the 20-hour rate, so the two are the
    // same number only if the cell empties exactly at the cut-off. It cannot: the
    // overpotential diverges as soc -> 0 at any current, so the cell always stops a little
    // short. How short is a stated model error, and it is stated here.
    let anchor = base_on / 7.2;
    println!(
        "C/20 delivers {base_on:.4} Ah of the {:.1} Ah declared ({:.1} %); the ohmic-only \
         arm delivers {base_off:.4} Ah ({:.1} %)",
        7.2,
        anchor * 100.0,
        base_off / 7.2 * 100.0
    );
    assert!(
        (0.93..1.0).contains(&anchor),
        "the C/20 delivery must stay within a few points of the declared 7.2 Ah — it was \
         in the fit's objective, so drifting means the parameters no longer are the fitted \
         ones; got {:.1} %",
        anchor * 100.0
    );
}

/// **Rest recovery, which nothing in the fit scored.**
///
/// The acceptance test `docs/plans/lead-acid-data-only.md` named for this mechanism and
/// never measured: discharge hard to cut-off, rest, and the cell delivers materially more
/// on a second discharge. It falls out of the depletion relaxing and was not an objective,
/// which is what makes it evidence rather than a fit.
///
/// Two things beyond the headline, both of which a term tuned to pass a rate sweep could
/// get wrong: a **harder** discharge must recover **more**, and the recovery must be on
/// the scale of hours. The control arm recovers only what its RC pair holds, which is
/// minutes and almost nothing.
#[test]
fn a_rest_recovers_capacity_and_a_harder_discharge_recovers_more() {
    /// Discharge to cut-off, rest `rest_s`, discharge again; return the second delivery as
    /// a fraction of the first.
    fn recovery(chem: &ChemistryParams, c_rate: f64, rest_s: f64) -> f64 {
        let i = c_rate * chem.cell.capacity_ah;
        let v_min = chem.cell.v_min;
        let mut pack = Pack::new(&config(), chem.clone()).expect("pack builds");

        let run = |pack: &mut Pack| {
            let mut ah = 0.0;
            for _ in 0..(30 * 3600) {
                let t = pack.step(1.0, Demand::Current(i), &env());
                if t.v_terminal < v_min {
                    break;
                }
                ah += t.i_actual / 3600.0;
            }
            ah
        };
        let first = run(&mut pack);
        let mut rested = 0.0;
        while rested < rest_s {
            pack.step(60.0, Demand::Rest, &env());
            rested += 60.0;
        }
        run(&mut pack) / first
    }

    let chem = pba();
    println!("\nsecond discharge after a rest, as a fraction of the first:");
    println!("{:<8} {:>10} {:>10} {:>10}", "rate", "0 h", "1 h", "4 h");
    let mut four_hour = Vec::new();
    for &c in &[1.0, 3.0] {
        let r: Vec<f64> = [0.0, 3600.0, 4.0 * 3600.0]
            .iter()
            .map(|&s| recovery(&chem, c, s))
            .collect();
        println!(
            "{:<8} {:>9.1}% {:>9.1}% {:>9.1}%",
            format!("{c:.2}C"),
            r[0] * 100.0,
            r[1] * 100.0,
            r[2] * 100.0
        );
        four_hour.push(r[2]);
    }

    let (one_c, three_c) = (four_hour[0], four_hour[1]);
    assert!(
        one_c > 0.10,
        "a four-hour rest after a 1C discharge must return real capacity; got {:.1}%",
        one_c * 100.0
    );
    assert!(
        three_c > one_c,
        "a HARDER discharge must recover MORE — 3C got {:.1}% against 1C's {:.1}%. If this \
         inverts, the depletion is not tracking the current that built it.",
        three_c * 100.0,
        one_c * 100.0
    );

    // The control arm: an RC pair alone recovers next to nothing over four hours, because
    // it relaxed within the first few minutes and holds only millivolts anyway.
    let control = recovery(&pba_ohmic_only(), 3.0, 4.0 * 3600.0);
    println!(
        "\ncontrol arm ([diffusion] stripped), 3C, 4 h rest: {:.1}%\n",
        control * 100.0
    );
    assert!(
        control < 0.5 * three_c,
        "the control arm must recover far less ({:.1}%) than the diffusion arm ({:.1}%), \
         or rest recovery is coming from the RC pair rather than the new term",
        control * 100.0,
        three_c * 100.0
    );
}

/// **How much heat the term actually adds, measured rather than asserted.**
///
/// Three places in this repo were about to say "the diffusion term roughly doubles this
/// cell's heat at 3C" — this file's `config` doc, the chemistry file, and the plan doc —
/// on an estimate rather than a run. `q_gen_w` is already in telemetry and this sweep
/// already exists, so it was cheap to stop estimating, and the estimate was **wrong by a
/// factor of four**: the peak heat goes up by 1.20×, not 2×, and the *average* over a
/// discharge by 1.07×.
///
/// Three quantities, because they disagree and prose has to pick one:
///
/// * **Peak** rises most. The depletion builds from zero, so the term contributes nothing
///   at the start of a run and all of its contribution at the end.
/// * **Mean** rises barely, for the same reason.
/// * **Total heat over the discharge goes DOWN**, which is the counter-intuitive one and
///   the reason a "hotter cell" summary would be false. The term ends the discharge sooner
///   — 736 s against 1008 s at 3C — so the cell spends less time making heat at all, and
///   never reaches the low-SOC region where `[r0]` is highest. That last part is why the
///   peaks are not even being compared at the same cell state.
///
/// The measured ratios are printed rather than asserted; what is asserted is the *ordering*
/// between them, which is the part that makes the three words non-interchangeable.
#[test]
fn the_term_adds_heat_and_the_peak_is_not_the_average() {
    let report = |label: &str, chem: &ChemistryParams| -> (f64, f64, f64) {
        let i = 3.0 * chem.cell.capacity_ah;
        let mut pack = Pack::new(&config(), chem.clone()).expect("pack builds");
        let (mut peak, mut total_j, mut secs) = (0.0f64, 0.0, 0.0);
        for _ in 0..(30 * 3600) {
            let t = pack.step(1.0, Demand::Current(i), &env());
            if t.v_terminal < chem.cell.v_min {
                break;
            }
            peak = peak.max(t.q_gen_w);
            total_j += t.q_gen_w;
            secs += 1.0;
        }
        println!(
            "{label:<10} 3C: peak {peak:.2} W, mean {:.2} W over {secs:.0} s, \
             {total_j:.0} J total",
            total_j / secs
        );
        (peak, total_j / secs, total_j)
    };

    println!("\nheat at 3C:");
    let (peak_on, mean_on, total_on) = report("shipped", &pba());
    let (peak_off, mean_off, total_off) = report("control", &pba_ohmic_only());
    println!(
        "  peak {:.2}x, mean {:.2}x, total {:.2}x\n",
        peak_on / peak_off,
        mean_on / mean_off,
        total_on / total_off
    );

    assert!(
        peak_on > 1.1 * peak_off,
        "the term must add real heat and not only volts — energy that leaves the electrical \
         side has to arrive on the thermal side. Peak went {peak_off:.2} W -> {peak_on:.2} W"
    );
    assert!(
        peak_on / peak_off > 1.05 * (mean_on / mean_off),
        "the peak ratio ({:.2}x) must exceed the mean ratio ({:.2}x) — the depletion builds \
         from zero, so a run's average is NOT its peak, and prose quoting one for the other \
         is wrong. If these ever converge the term has stopped being a slow state.",
        peak_on / peak_off,
        mean_on / mean_off
    );
    assert!(
        total_on < total_off,
        "total heat over the discharge must FALL ({total_on:.0} J against {total_off:.0} J): \
         the term ends the run sooner, so the cell makes heat for less time. Any summary \
         that calls this a hotter cell is false, and this assertion is what stops one being \
         written."
    );
}

/// **The saturation ceiling never binds inside the window this term was fitted on.**
///
/// `docs/plans/diffusion-overpotential.md` checked this on the fitting harness, whose
/// discharge loop stops at `soc > 0` and so could not have found the case where it *does*
/// bind. In the engine it binds — at `soc = 0`, where `D_lim·soc` is zero — so the claim is
/// re-scoped and re-measured here: **not** "the guard never fires", but "no discharge in
/// the swept range comes near it".
///
/// That distinction is the point. If the ceiling were load-bearing inside the sweep, the
/// rate fit above would be resting on a declared limit rather than on the mechanism —
/// which is the exact failure `docs/plans/phase-7-dfn.md` records as a guard documented as
/// numerical turning out to be physics.
///
/// `CellView::overpotential_v` is the sum of the RC pair and the diffusion term, and on
/// discharge both are non-negative, so the sum is an **upper bound** on the diffusion term
/// alone. Bounding the sum therefore bounds what this test is about, with room to spare.
#[test]
fn the_saturation_ceiling_never_binds_inside_the_sweep() {
    let chem = pba();
    let ceiling = chem.diffusion.expect("shipped section").max_overpotential_v;

    let mut worst = 0.0f64;
    for &c in &RATES {
        let i = c * chem.cell.capacity_ah;
        let mut pack = Pack::new(&config(), chem.clone()).expect("pack builds");
        for _ in 0..(30 * 3600) {
            let t = pack.step(1.0, Demand::Current(i), &env());
            if t.v_terminal < chem.cell.v_min {
                break;
            }
            worst = worst.max(pack.cell(0, 0).expect("cell").overpotential_v);
        }
    }

    println!(
        "\nlargest overpotential anywhere in the sweep: {worst:.4} V against a ceiling of \
         {ceiling:.3} V\n"
    );
    assert!(
        worst < 0.2 * ceiling,
        "the fit must rest on the mechanism and not on the declared ceiling; the sweep \
         reached {worst:.4} V against {ceiling:.3} V"
    );
}

/// Delivered capacity quantizes at `I·dt` on the step that crosses the cutoff, so a coarse
/// timestep makes a rate look better than it is. This pins that the sweeps above are
/// resolved at the `dt` they actually quote.
///
/// **The tolerance is derived, not chosen.** One step at rate `c` moves `c·dt/3600` of the
/// cell's capacity, so halving `dt` cannot move the answer by more than the coarse step's own
/// quantum unless something other than the cutoff crossing is `dt`-sensitive. Asserting a
/// round number like "0.1 %" instead would silently re-license a real resolution defect at
/// high rate while over-constraining low rate — see `docs/plans/path-tolerance-rule.md`.
///
/// Run on **both** arms. The diffusion term is advanced by an exact exponential and so is
/// unconditionally stable at any `dt`, exactly like the RC pairs — but "should be by
/// construction" is what a convergence check exists to stop anyone asserting.
#[test]
fn rate_sweep_is_not_timestep_limited() {
    const DT: f64 = 1.0; // the dt the sweeps above quote
    for (label, chem) in [("shipped", pba()), ("control", pba_ohmic_only())] {
        for &c in &[0.05, 1.0, 3.0] {
            let (lo, hi) = delivered_bracket(&chem, c, DT);
            let (flo, fhi) = delivered_bracket(&chem, c, DT / 4.0);
            let quantum_ah = c * chem.cell.capacity_ah * DT / 3600.0;
            let width = hi - lo;
            let converge = (lo - flo).abs();
            println!(
                "{label} {c:.2}C: dt={DT} s -> [{lo:.4}, {hi:.4}] Ah (width {width:.4}); \
                 dt={} s -> [{flo:.4}, {fhi:.4}] Ah (width {:.4}); \
                 lower bound moved {converge:.4} against a quantum of {quantum_ah:.4}",
                DT / 4.0,
                fhi - flo
            );

            // 1. Convergence: refining dt cannot move the answer by more than one step's
            //    charge.
            assert!(
                converge <= quantum_ah,
                "{label} at {c:.2}C, refining dt from {DT} s to {} s moved the delivered \
                 capacity by {converge:.4} Ah, more than the {quantum_ah:.4} Ah a single \
                 step carries — so something beyond the cutoff crossing is \
                 timestep-sensitive",
                DT / 4.0
            );

            // 2. Accuracy, which convergence alone CANNOT establish: the quoted figure is
            //    the bracket's lower end, and that bias points the same way at every dt, so
            //    two resolutions agreeing proves nothing about it. The bracket's own width
            //    is the honest bound on the quoted number, and it must be small enough to
            //    not matter.
            assert!(
                width <= quantum_ah * 1.001,
                "{label} at {c:.2}C the bracket is {width:.4} Ah wide, more than the \
                 {quantum_ah:.4} Ah a single step carries — the cutoff is being crossed by \
                 more than one step"
            );
            let rel = width / lo;
            assert!(
                rel < 2e-3,
                "{label} at {c:.2}C the quoted lower bound is uncertain by {:.3}% of \
                 itself, which is too coarse for the four significant figures the plan doc \
                 quotes",
                rel * 100.0
            );

            // 3. And refining dt must actually narrow the bracket, which is the direct
            //    evidence that the width is the cutoff-crossing quantum and not some other
            //    error.
            assert!(
                (fhi - flo) < width * 0.5,
                "{label} at {c:.2}C, quartering dt should quarter the bracket; got \
                 {width:.4} -> {:.4}",
                fhi - flo
            );
        }
    }
}
