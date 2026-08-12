//! Lead-acid as data alone: does the shipped ECM reproduce Peukert behaviour?
//!
//! `CLAUDE.md` promises a chemistry is data, not code, and names lead-acid with Peukert.
//! This file measures how far that promise carries. See `docs/plans/lead-acid-data-only.md`,
//! whose predictions were written *before* the parameter file existed.
//!
//! Run the table with `cargo test -p sim-data --test lead_acid_rate -- --nocapture`.

use sim_core::{
    ecm::ocv_lookup, CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter,
    ThermalConfig,
};

fn pba() -> ChemistryParams {
    let text = include_str!("../../../chemistries/pba_agm_2v_generic.toml");
    sim_data::parse_chemistry(text).expect("lead-acid chemistry loads and validates")
}

/// A bare cell: no BMS (so nothing derates the demand), no aging (so capacity does not
/// move during a 20-hour discharge), and **isothermal** — the rate dependence being
/// measured must not be tangled with self-heating, which is a separate question.
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
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    let mut ah = 0.0;
    // Bound the loop by simulated time rather than trusting the cutoff: a 0.05C
    // discharge is 20 h, so 30 h of headroom cannot be reached by any rate here.
    let max_steps = (30.0 * 3600.0 / dt) as u64;
    for _ in 0..max_steps {
        let t = pack.step(dt, Demand::Current(i), &env);
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
/// step's charge — stated in `docs/plans/lead-acid-data-only.md` rather than left implicit.
fn delivered_ah(chem: &ChemistryParams, c_rate: f64, dt: f64) -> f64 {
    delivered_bracket(chem, c_rate, dt).0
}

/// Peukert's law as the reference curve, expressed as delivered capacity relative to a
/// reference rate. `n = 1.1` is the AGM class; flooded runs 1.2–1.3.
fn peukert_fraction(c_rate: f64, c_ref: f64, n: f64) -> f64 {
    (c_ref / c_rate).powf(n - 1.0)
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

    // Inside LFP's plateau the ordering reverses, and that is the real contrast.
    let pba_flat = slope(&pba, 0.45, 0.65);
    let lfp_flat = slope(&lfp, 0.45, 0.65);
    assert!(
        pba_flat > 4.0 * lfp_flat,
        "in LFP's dead zone lead-acid must be several times more informative \
         (lead-acid {pba_flat:.3} V/soc vs LFP {lfp_flat:.3})"
    );

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

/// The measurement this slice exists for.
///
/// Asserted as **structure**, not as numbers on a curve, because every constant behind it
/// is a labelled placeholder — the rule this repo applies to every fade curve. The
/// structural claim is what survives a refit: ohmic sag produces a *flat-then-knee*
/// response where Peukert is a smooth power law, so the model reproduces none of the
/// rate loss at low rate however the resistances are chosen.
#[test]
fn ecm_underproduces_peukert_and_the_shape_is_why() {
    let chem = pba();
    let dt = 1.0;
    let c_ref = 0.05;
    let base = delivered_ah(&chem, c_ref, dt);

    let rates = [0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.0];
    println!(
        "\nlead-acid, 1S1P, isothermal 25 C, discharge to {:.2} V, dt = {dt} s",
        chem.cell.v_min
    );
    println!("C/20 reference delivery: {base:.4} Ah\n");
    println!(
        "{:<8} {:>10} {:>10} {:>12} {:>12} {:>10}",
        "rate", "engine Ah", "engine %", "Peukert1.1", "Peukert1.2", "captured"
    );
    let mut fractions = Vec::new();
    for &c in &rates {
        let ah = delivered_ah(&chem, c, dt);
        let f = ah / base;
        let p11 = peukert_fraction(c, c_ref, 1.1);
        let p12 = peukert_fraction(c, c_ref, 1.2);
        let captured = if (1.0 - p11).abs() < 1e-12 {
            f64::NAN
        } else {
            (1.0 - f) / (1.0 - p11) * 100.0
        };
        println!(
            "{:<8} {ah:>10.4} {:>9.1}% {:>11.1}% {:>11.1}% {:>9.0}%",
            format!("{c:.2}C"),
            f * 100.0,
            p11 * 100.0,
            p12 * 100.0,
            captured
        );
        fractions.push((c, f, p11));
    }

    // 1. Below 1C the model reproduces essentially NONE of the real loss. This is the
    //    structural failure: `I·R` is negligible against the OCV headroom there, so the
    //    curve is flat where Peukert has already given up a quarter of its capacity.
    for &(c, f, p11) in &fractions {
        if c <= 1.0 {
            assert!(
                f > 0.98,
                "at {c:.2}C the ECM still delivers ~full capacity by construction; \
                 got {:.1}% (real lead-acid: {:.1}%)",
                f * 100.0,
                p11 * 100.0
            );
        }
    }

    // 2. Above 1C it does produce a loss, in the right direction — so the mechanism is
    //    present, just too weak and too late.
    let (_, f3, p3) = *fractions.last().expect("3C row");
    assert!(
        f3 < 0.95,
        "at 3C ohmic sag must cost real capacity; got {:.1}%",
        f3 * 100.0
    );
    assert!(
        f3 > p3,
        "the ECM must UNDER-produce the loss (got {:.1}% delivered vs Peukert's {:.1}%); \
         if this ever fails, the resistances have been tuned to fake a diffusion limit",
        f3 * 100.0,
        p3 * 100.0
    );

    // 3. The headline, as a band rather than a point, because the inputs are placeholders.
    let captured = (1.0 - f3) / (1.0 - p3);
    println!(
        "\nover {c_ref:.2}C -> 3.00C: real AGM loses {:.1}%, engine loses {:.1}% \
         -> engine reproduces {:.0}% of it\n",
        (1.0 - p3) * 100.0,
        (1.0 - f3) * 100.0,
        captured * 100.0
    );
    assert!(
        (0.25..0.70).contains(&captured),
        "the engine should reproduce roughly a third to two thirds of lead-acid's \
         rate-dependent capacity loss; got {:.0}%",
        captured * 100.0
    );
}

/// The objection this slice has to answer: `[r0]`'s rise toward empty is a placeholder, so
/// isn't the gap above just the number I picked?
///
/// No, and this is the finding. Scaling **every** resistance until 3C lands on the Peukert
/// curve leaves the mid-range no better, because the two curves are different *shapes*:
/// ohmic sag is flat-then-knee (at low rate `I·R` is negligible against the OCV headroom),
/// while Peukert is a smooth power law that starts giving up capacity at the first rate
/// increase. Tuning moves the knee. It cannot remove the flat.
#[test]
fn tuning_the_resistances_moves_the_knee_but_not_the_flat() {
    let base_chem = pba();
    let scaled = |k: f64| {
        let mut c = pba();
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
         stand-in for a diffusion limit and the phase this slice prices is unnecessary."
    );
    // The residual sits in the middle of the range, where Peukert has already lost a
    // quarter of its capacity and ohmic sag has lost none.
    assert!(
        (0.2..=1.5).contains(&tuned_at),
        "the unfixable residual should sit mid-range, not at the extremes; got {tuned_at:.2}C"
    );
}

/// Delivered capacity quantizes at `I·dt` on the step that crosses the cutoff, so a coarse
/// timestep makes a rate look better than it is. This pins that the sweep above is resolved
/// at the `dt` it actually quotes.
///
/// **The tolerance is derived, not chosen.** One step at rate `c` moves `c·dt/3600` of the
/// cell's capacity, so halving `dt` cannot move the answer by more than the coarse step's own
/// quantum unless something other than the cutoff crossing is `dt`-sensitive. Asserting a
/// round number like "0.1 %" instead would silently re-license a real resolution defect at
/// high rate while over-constraining low rate — see `docs/plans/path-tolerance-rule.md`.
#[test]
fn rate_sweep_is_not_timestep_limited() {
    let chem = pba();
    const DT: f64 = 1.0; // the dt the sweep above quotes
    for &c in &[0.05, 1.0, 3.0] {
        let (lo, hi) = delivered_bracket(&chem, c, DT);
        let (flo, fhi) = delivered_bracket(&chem, c, DT / 4.0);
        let quantum_ah = c * chem.cell.capacity_ah * DT / 3600.0;
        let width = hi - lo;
        let converge = (lo - flo).abs();
        println!(
            "{c:.2}C: dt={DT} s -> [{lo:.4}, {hi:.4}] Ah (width {width:.4}); \
             dt={} s -> [{flo:.4}, {fhi:.4}] Ah (width {:.4}); \
             lower bound moved {converge:.4} against a quantum of {quantum_ah:.4}",
            DT / 4.0,
            fhi - flo
        );

        // 1. Convergence: refining dt cannot move the answer by more than one step's charge.
        assert!(
            converge <= quantum_ah,
            "at {c:.2}C, refining dt from {DT} s to {} s moved the delivered capacity by \
             {converge:.4} Ah, more than the {quantum_ah:.4} Ah a single step carries — so \
             something beyond the cutoff crossing is timestep-sensitive",
            DT / 4.0
        );

        // 2. Accuracy, which convergence alone CANNOT establish: the quoted figure is the
        //    bracket's lower end, and that bias points the same way at every dt, so two
        //    resolutions agreeing proves nothing about it. The bracket's own width is the
        //    honest bound on the quoted number, and it must be small enough to not matter.
        assert!(
            width <= quantum_ah * 1.001,
            "at {c:.2}C the bracket is {width:.4} Ah wide, more than the {quantum_ah:.4} Ah \
             a single step carries — the cutoff is being crossed by more than one step"
        );
        let rel = width / lo;
        assert!(
            rel < 2e-3,
            "at {c:.2}C the quoted lower bound is uncertain by {:.3}% of itself, which is \
             too coarse for the four significant figures the plan doc quotes",
            rel * 100.0
        );

        // 3. And refining dt must actually narrow the bracket, which is the direct evidence
        //    that the width is the cutoff-crossing quantum and not some other error.
        assert!(
            (fhi - flo) < width * 0.5,
            "at {c:.2}C, quartering dt should quarter the bracket; got {width:.4} -> {:.4}",
            fhi - flo
        );
    }
}
