//! Phase 12 slice C: what plating does to a reaction cell's health and its flags
//! (`docs/plans/phase-12-slice-c-health.md`, whose numbered predictions the tests name).
//!
//! As in `plating_dfn.rs` the drawn fades are zeroed — calendar, cycle, the threshold's plating
//! fade and both dendrite hazards — so the reaction is the only thing moving the cell, unless a
//! test says it puts one back.

use sim_core::dfn::{
    probe, DEFAULT_NODES_NEGATIVE, DEFAULT_NODES_POSITIVE, DEFAULT_NODES_SEPARATOR,
    FINE_NODES_NEGATIVE, FINE_NODES_POSITIVE, FINE_NODES_SEPARATOR, FINE_SHELLS,
};
use sim_core::spm::{mean_concentration, DEFAULT_SHELLS, FARADAY_C_PER_MOL as F};
use sim_core::{
    AgingConfig, CalendarModel, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack,
    PackConfig, PlatingModel, Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shipped LG M50 with every drawn fade and both dendrite hazards zeroed.
fn chem() -> ChemistryParams {
    let mut c = parse_chemistry(LGM50).expect("the shipped LG M50 file parses");
    let a = c.aging.as_mut().expect("[aging]");
    a.cal_pre_exp = 0.0;
    a.cyc_fade_per_ah = 0.0;
    let s = c.safety.as_mut().expect("[safety]");
    s.plating_fade_per_ah = 0.0;
    s.plating_short_hazard_per_ah = 0.0;
    pl(&mut c).short_hazard_per_dead_ah = 0.0;
    c
}

fn pl(c: &mut ChemistryParams) -> &mut sim_core::PlatingParams {
    c.dfn
        .as_mut()
        .and_then(|d| d.plating.as_mut())
        .expect("[dfn.plating]")
}

fn recommended() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: DEFAULT_SHELLS,
        nodes_negative: DEFAULT_NODES_NEGATIVE,
        nodes_separator: DEFAULT_NODES_SEPARATOR,
        nodes_positive: DEFAULT_NODES_POSITIVE,
    }
}

fn fine() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: FINE_SHELLS,
        nodes_negative: FINE_NODES_NEGATIVE,
        nodes_separator: FINE_NODES_SEPARATOR,
        nodes_positive: FINE_NODES_POSITIVE,
    }
}

fn config(model: CellModelConfig, plating: PlatingModel, soc: f64, temp_k: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: Some(AgingConfig {
            plating,
            ..AgingConfig::default()
        }),
        cell_model: model,
    }
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

fn kelvin(temp_c: f64) -> f64 {
    273.15 + temp_c
}

/// The cell's snapshot JSON, `[pack][groups][0][cells][0]`.
fn cell_json(p: &Pack) -> serde_json::Value {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    v["pack"]["groups"][0]["cells"][0].clone()
}

fn dfn_state(p: &Pack) -> sim_core::DfnState {
    serde_json::from_value(cell_json(p)["model"]["Dfn"].clone()).expect("cell 0 is a Dfn cell")
}

/// The fresh electrodes' window capacity \[A·h\]: what `soc_true` is a fraction of on a new cell.
fn window_ah(c: &ChemistryParams) -> f64 {
    probe::spm_capacity(c.spm.as_ref().unwrap())
}

/// What one charge left, and on how many of its steps `PLATING_RISK` rose.
struct Charged {
    pack: Pack,
    flagged: u32,
}

/// CC at `c_rate` from empty to 4.2 V, then 4.2 V held to C/20, 1 s steps, pinned at `temp_c`:
/// slice A's protocol, on `model` with `plating`.
fn cccv(
    c: &ChemistryParams,
    model: CellModelConfig,
    plating: PlatingModel,
    temp_c: f64,
    c_rate: f64,
) -> Charged {
    let q = c.cell.capacity_ah;
    let t = kelvin(temp_c);
    let mut p = Pack::new(&config(model, plating, 0.0, t), c.clone()).expect("builds");
    let (mut held, mut secs, mut flagged) = (false, 0.0, 0);
    loop {
        let d = if held {
            Demand::Voltage(4.2)
        } else {
            Demand::Current(-c_rate * q)
        };
        let tel = p.step(1.0, d, &env(t));
        secs += 1.0;
        flagged += u32::from(tel.flags.contains(EventFlags::PLATING_RISK));
        if !held && tel.v_terminal >= 4.2 {
            held = true;
        } else if held && -tel.i_actual <= 0.05 * q {
            break;
        }
        assert!(secs < 30.0 * 3600.0, "{temp_c} C {c_rate}C never tapered");
    }
    Charged { pack: p, flagged }
}

/// `(δx_rev, δx_dead)` over the fresh window `Δx`, read off the state.
fn plated_fractions(s: &sim_core::DfnState, c: &ChemistryParams) -> (f64, f64) {
    let e = &c.spm.as_ref().unwrap().negative;
    let pl = s.plating.as_ref().unwrap();
    #[allow(clippy::cast_precision_loss)]
    let frac = |q: &[f64]| {
        3.0 * q.iter().sum::<f64>()
            / q.len() as f64
            / (e.particle_radius_m * e.c_max_mol_per_m3)
            / (e.stoich_max - e.stoich_min)
    };
    (frac(&pl.reversible_mol_m2), frac(&pl.dead_mol_m2))
}

/// Reading (b): the graphite alone, over the window the dead lithium shortened.
fn reading_b(s: &sim_core::DfnState, c: &ChemistryParams) -> f64 {
    let e = &c.spm.as_ref().unwrap().negative;
    #[allow(clippy::cast_precision_loss)]
    let x = s.c_neg.iter().map(|p| mean_concentration(p)).sum::<f64>()
        / s.c_neg.len() as f64
        / e.c_max_mol_per_m3;
    let (_, dead) = plated_fractions(s, c);
    (x - e.stoich_min) / (e.stoich_max - e.stoich_min) / (1.0 - dead)
}

// --- capacity, and what the cell holds while lithium sits as metal --------------------------

/// Predictions 3 and 4, on the registered 0 °C 1C CC–CV charge (fine grid, 1 s): the dead lithium
/// is in the reported capacity exactly once, and through an hour's rest reading (a) stays flat
/// while what metal the hold left strips back. Prediction 5 is not run on this charge (below).
#[test]
fn dead_lithium_is_capacity_once_and_reversible_metal_is_stored_charge() {
    let c = chem();
    let t = kelvin(0.0);
    let charged = cccv(&c, fine(), PlatingModel::Reaction, 0.0, 1.0).pack;
    let s0 = dfn_state(&charged);
    let (rev0, dead) = plated_fractions(&s0, &c);
    let dead_ah = charged.cell_plating(0, 0).unwrap().dead_mol * F / 3600.0;
    let view = charged.cell(0, 0).unwrap();
    let handed = cell_json(&charged)["aging"]["soh_capacity"]
        .as_f64()
        .unwrap();
    println!(
        "0 C 1C CC-CV: dead {:.4} % of the window, reversible {:.4} %; soh_capacity {:.9}",
        dead * 100.0,
        rev0 * 100.0,
        view.soh_capacity
    );
    // 3: once.
    assert_eq!(
        handed, 1.0,
        "the multiplier handed to the model carries no plating"
    );
    assert!(
        (view.soh_capacity - (1.0 - dead)).abs() <= 1.0e-12,
        "{} against 1 - {dead}",
        view.soh_capacity
    );
    assert!(
        (dead - dead_ah / window_ah(&c)).abs() <= 1.0e-9 * dead,
        "the stoichiometry conversion: {dead} against {}",
        dead_ah / window_ah(&c)
    );
    assert!(dead > 3.0e-4 && dead < 9.0e-4, "about 0.059 %: {dead}");
    // What the 0 °C hold left as metal, pinned as measured (1.2 % of the dead store).
    assert!(
        rev0 < 0.02 * dead,
        "the 0 C hold strips back nearly all: {rev0} against {dead}"
    );

    // 4: an hour at rest.
    let mut rest = charged.clone();
    let soc0 = rest.cell(0, 0).unwrap().soc;
    let b0 = reading_b(&s0, &c);
    let mut drift: f64 = 0.0;
    for _ in 0..3600 {
        rest.step(1.0, Demand::Rest, &env(t));
        drift = drift.max((rest.cell(0, 0).unwrap().soc - soc0).abs());
    }
    let s1 = dfn_state(&rest);
    let (rev1, _) = plated_fractions(&s1, &c);
    let b1 = reading_b(&s1, &c);
    println!(
        "rest 1 h: reversible {:.4} % -> {:.4} %; reading (a) drift {drift:.3e}; reading (b) {b0:.9} -> {b1:.9}",
        rev0 * 100.0,
        rev1 * 100.0
    );
    assert!(rev1 < 0.5 * rev0, "the metal strips back within the hour");
    assert!(drift <= 1.0e-9, "reading (a) moved {drift:e} at rest");
    assert!(
        b1 - b0 > 0.5 * (rev0 - rev1),
        "reading (b) rises by what stripped"
    );

    // 5, as registered — straight into C/2 after this charge — is not run: at 0 °C the
    // discharge after a 4.2 V hold leaves the `Dfn`'s solve unconverged on nearly every step
    // (`report_discharge_after_a_hold_extent`; a threshold cell does the same), and would test
    // that, not the reading. See `cc_leg_rest_and_discharge_keep_reading_a`.
}

/// Changed after registration (`phase-12-slice-c-health.md`, prediction 5): the same checks as
/// predictions 4 and 5 on the 0 °C 1C charge's constant-current leg alone, to 4.2 V, where the
/// reversible store is ~0.05 % of capacity rather than the hold's leftover. Through an hour's rest
/// reading (a) stays flat while reading (b) rises by what stripped; and on a C/2 discharge
/// straight after (every step converged), reading (a)'s change times the capacity it is weighted
/// by is the charge passed.
#[test]
fn cc_leg_rest_and_discharge_keep_reading_a() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(0.0);
    let mut charged =
        Pack::new(&config(fine(), PlatingModel::Reaction, 0.0, t), c.clone()).expect("builds");
    loop {
        if charged.step(1.0, Demand::Current(-q), &env(t)).v_terminal >= 4.2 {
            break;
        }
    }
    let s0 = dfn_state(&charged);
    let (rev0, dead) = plated_fractions(&s0, &c);
    println!(
        "0 C 1C CC to 4.2 V: reversible {:.4} %, dead {:.4} %; window / capacity_ah - 1 = {:.4e}",
        rev0 * 100.0,
        dead * 100.0,
        window_ah(&c) / q - 1.0
    );
    assert!(rev0 > 1.0e-4, "the leg must leave metal to strip");

    // 4 on this charge.
    let mut rest = charged.clone();
    let soc0 = rest.cell(0, 0).unwrap().soc;
    let mut drift: f64 = 0.0;
    for _ in 0..3600 {
        rest.step(1.0, Demand::Rest, &env(t));
        drift = drift.max((rest.cell(0, 0).unwrap().soc - soc0).abs());
    }
    let s1 = dfn_state(&rest);
    let (rev1, _) = plated_fractions(&s1, &c);
    let (b0, b1) = (reading_b(&s0, &c), reading_b(&s1, &c));
    println!(
        "rest 1 h: reversible {:.4} % -> {:.4} %; reading (a) drift {drift:.3e}; reading (b) {b0:.9} -> {b1:.9} (+{:.3e})",
        rev0 * 100.0,
        rev1 * 100.0,
        b1 - b0
    );
    assert!(rev1 < 0.5 * rev0, "the metal strips back within the hour");
    assert!(drift <= 1.0e-9, "reading (a) moved {drift:e} at rest");
    assert!(
        b1 - b0 > 0.5 * (rev0 - rev1),
        "reading (b) rises by what stripped"
    );

    // 5 on this charge.
    let mut dis = charged.clone();
    let soc_a = dis.cell(0, 0).unwrap().soc;
    let (mut passed_ah, mut unconverged) = (0.0, 0);
    loop {
        let tel = dis.step(1.0, Demand::Current(0.5 * q), &env(t));
        passed_ah += tel.i_actual / 3600.0;
        unconverged += u32::from(tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
        if tel.v_terminal <= 2.5 || tel.v_terminal.is_nan() {
            break;
        }
    }
    let after = dis.cell(0, 0).unwrap();
    // The capacity `soc_true` is weighted by: the chemistry's, times the health the cell
    // reports (`Pack::step`'s reporting pass) — not the electrodes' window, which the model
    // scales its current to the chemistry's capacity against.
    let moved_ah = (soc_a - after.soc) * q * after.soh_capacity;
    let stripped = rev0 - plated_fractions(&dfn_state(&dis), &c).0;
    println!(
        "C/2: passed {passed_ah:.9} A·h, reading (a) moved {moved_ah:.9} A·h; stripped on the way {stripped:.3e} of the window; unconverged {unconverged}; soc at cut-off {:.4}",
        after.soc
    );
    assert_eq!(unconverged, 0);
    assert!(after.soc > 0.0, "the cut-off comes before empty");
    assert!(
        (moved_ah - passed_ah).abs() <= 1.0e-9 * passed_ah,
        "reading (a): {moved_ah} against {passed_ah}"
    );
}

// --- the flag -------------------------------------------------------------------------------

/// Prediction 6, the contrast: the two allowed charges on which the reaction cell's flag rises
/// (`plating_dfn.rs`'s exit test pins that, on all nine charges, the reaction's flag rises on
/// exactly the steps that plated) never raise it on a threshold `Dfn` cell — 0 °C is not below
/// its `t_plating_min_k`, and 25 °C is warm.
#[test]
fn the_threshold_is_silent_where_the_reaction_flags() {
    let c = chem();
    let rows: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = [(0.0, 0.3), (25.0, 0.7)]
            .iter()
            .map(|&(tc, cr)| {
                let c = &c;
                s.spawn(move || {
                    (
                        tc,
                        cr,
                        cccv(c, fine(), PlatingModel::Threshold, tc, cr).flagged,
                    )
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (tc, cr, flagged) in rows {
        println!("{tc} C {cr}C: threshold flagged {flagged} steps");
        assert_eq!(
            flagged, 0,
            "{tc} C {cr}C: the threshold flags an allowed charge"
        );
    }
}

/// Prediction 6, the rest: the flag never rises on a zero-length step — at 0 °C 1C mid-charge,
/// where the same step at 1 s does — nor on a rest or a discharge after a cold charge.
#[test]
fn the_flag_is_down_on_a_probe_step_a_rest_and_a_discharge() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(0.0);
    let mut p = Pack::new(
        &config(recommended(), PlatingModel::Reaction, 0.6, t),
        c.clone(),
    )
    .expect("builds");
    let mut seen = false;
    for _ in 0..1200 {
        let probe_step = p.step(0.0, Demand::Current(-q), &env(t));
        assert!(
            !probe_step.flags.contains(EventFlags::PLATING_RISK),
            "a zero-length step plates nothing"
        );
        seen |= p
            .step(1.0, Demand::Current(-q), &env(t))
            .flags
            .contains(EventFlags::PLATING_RISK);
    }
    assert!(seen, "the fixture must plate");
    for (d, n) in [(Demand::Rest, 600), (Demand::Current(q), 600)] {
        for _ in 0..n {
            let tel = p.step(1.0, d, &env(t));
            assert!(!tel.flags.contains(EventFlags::PLATING_RISK), "{d:?}");
        }
    }
}

// --- no threshold fade, and the resistance ---------------------------------------------------

/// Predictions 7 and 8 on −10 °C 1C CC–CV (fine grid): with the threshold's fade and hazard at
/// their shipped values the reaction cell's run is bit-identical to the zeroed run; its
/// resistance multiplier is `1 + r_growth · δx_dead/Δx`, with the film on too; and the same
/// charge on a threshold cell bills its fade (recorded).
#[test]
fn no_threshold_fade_and_the_resistance_follows_the_dead_lithium() {
    let zeroed = chem();
    let mut shipped = zeroed.clone();
    let real = parse_chemistry(LGM50).unwrap();
    shipped.safety = real.safety;
    let r_growth = zeroed.aging.as_ref().unwrap().r_growth_per_capacity_loss;
    let (a, b, th) = std::thread::scope(|s| {
        let a = s.spawn(|| cccv(&zeroed, fine(), PlatingModel::Reaction, -10.0, 1.0).pack);
        let b = s.spawn(|| cccv(&shipped, fine(), PlatingModel::Reaction, -10.0, 1.0).pack);
        let th = s.spawn(|| cccv(&shipped, fine(), PlatingModel::Threshold, -10.0, 1.0).pack);
        (a.join().unwrap(), b.join().unwrap(), th.join().unwrap())
    });
    // 7: everything but the chemistry itself.
    let strip = |p: &Pack| {
        let mut v = serde_json::to_value(p.snapshot()).unwrap();
        v["pack"]["chem"] = serde_json::Value::Null;
        v
    };
    assert_eq!(
        strip(&a),
        strip(&b),
        "the threshold's fade or hazard reached a reaction cell"
    );
    let th_soh = th.cell(0, 0).unwrap().soh_capacity;
    // 8: settle a tick at rest, where nothing plates, then read it.
    let mut a = a;
    for _ in 0..20 {
        a.step(1.0, Demand::Rest, &env(kelvin(-10.0)));
    }
    let (_, dead) = plated_fractions(&dfn_state(&a), &zeroed);
    let view = a.cell(0, 0).unwrap();
    println!(
        "-10 C 1C CC-CV: reaction lost {:.4} % (soh_resistance {:.9}); threshold cell soh_capacity {th_soh:.6}",
        dead * 100.0,
        view.soh_resistance
    );
    assert!(
        (view.soh_resistance - (1.0 + r_growth * dead)).abs() <= 1.0e-12,
        "{} against 1 + {r_growth}·{dead}",
        view.soh_resistance
    );
    assert!(th_soh < 1.0, "the threshold cell bills its fade");
}

/// Prediction 8 with the film: the multiplier the film's reported ratio divides out is the dead
/// lithium's growth, so a filmed reaction cell carries `1 + r_growth · δx_dead/Δx` too.
#[test]
fn a_filmed_reaction_cell_grows_the_same_multiplier() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(-10.0);
    let mut cfg = config(recommended(), PlatingModel::Reaction, 0.6, t);
    cfg.aging = Some(AgingConfig {
        calendar: CalendarModel::Film,
        plating: PlatingModel::Reaction,
        ..AgingConfig::default()
    });
    let mut p = Pack::new(&cfg, c.clone()).expect("builds");
    for _ in 0..600 {
        p.step(1.0, Demand::Current(-2.0 * q), &env(t));
    }
    for _ in 0..20 {
        p.step(1.0, Demand::Rest, &env(t));
    }
    let (_, dead) = plated_fractions(&dfn_state(&p), &c);
    let mult = cell_json(&p)["aging"]["soh_resistance"].as_f64().unwrap();
    let r_growth = c.aging.as_ref().unwrap().r_growth_per_capacity_loss;
    assert!(dead > 0.0 && p.cell_film(0, 0).is_some());
    assert!(
        (mult - (1.0 + r_growth * dead)).abs() <= 1.0e-12,
        "{mult} against 1 + {r_growth}·{dead}"
    );
}

// --- the dendrite short ----------------------------------------------------------------------

/// Prediction 10 (i) and (ii): with no tick inside the charge, the accumulator holds exactly the
/// dead lithium in amp-hours, at 1 s and at 60 s; and at the shipped hazard −10 °C 1C CC–CV
/// carries about 0.61 %.
#[test]
fn the_short_is_billed_per_dead_amp_hour() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(-10.0);
    for dt in [1.0, 60.0] {
        let mut cfg = config(recommended(), PlatingModel::Reaction, 0.6, t);
        cfg.aging = Some(AgingConfig {
            sub_clock_period_s: 1.0e9,
            plating: PlatingModel::Reaction,
            ..AgingConfig::default()
        });
        let mut p = Pack::new(&cfg, c.clone()).expect("builds");
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        for _ in 0..(1200.0 / dt) as usize {
            p.step(dt, Demand::Current(-q), &env(t));
        }
        let pending = cell_json(&p)["aging"]["ah_plating_since_tick"]
            .as_f64()
            .unwrap();
        let dead_ah = p.cell_plating(0, 0).unwrap().dead_mol * F / 3600.0;
        println!("dt {dt}: pending {pending:.12e} A·h, dead {dead_ah:.12e} A·h");
        assert!(dead_ah > 0.0);
        assert!(
            (pending - dead_ah).abs() <= 1.0e-12 * dead_ah,
            "dt {dt}: {pending} against {dead_ah}"
        );
    }
    // (ii) on the sizing charge.
    let lambda = parse_chemistry(LGM50)
        .unwrap()
        .dfn
        .unwrap()
        .plating
        .unwrap()
        .short_hazard_per_dead_ah;
    let r = cccv(&c, fine(), PlatingModel::Reaction, -10.0, 1.0).pack;
    let dead_ah = r.cell_plating(0, 0).unwrap().dead_mol * F / 3600.0;
    let p_short = -(-lambda * dead_ah).exp_m1();
    println!(
        "-10 C 1C CC-CV: dead {dead_ah:.4e} A·h, short {:.3} %",
        p_short * 100.0
    );
    assert!((0.0050..0.0075).contains(&p_short), "{p_short}");
}

/// Prediction 10 (iii) and (iv): a warm charge that plates nothing draws nothing at the shipped
/// hazard; and at a hazard of 1e6 the short lands on the first tick after lithium dies, as the
/// `[safety]` resistance.
#[test]
fn the_short_draws_only_on_dead_lithium_and_lands_as_the_safety_short() {
    let mut c = chem();
    pl(&mut c).short_hazard_per_dead_ah = 0.7;
    let q = c.cell.capacity_ah;
    let rng = |p: &Pack| serde_json::to_value(p.snapshot()).unwrap()["pack"]["rng"].clone();
    // (iii)
    let t = kelvin(25.0);
    let mut warm = Pack::new(
        &config(recommended(), PlatingModel::Reaction, 0.2, t),
        c.clone(),
    )
    .expect("builds");
    let fresh = rng(&warm);
    for _ in 0..3600 {
        warm.step(1.0, Demand::Current(-0.3 * q), &env(t));
    }
    assert_eq!(warm.cell_plating(0, 0).unwrap().dead_mol, 0.0);
    assert_eq!(rng(&warm), fresh, "a draw on a pack with no dead lithium");
    // (iv)
    pl(&mut c).short_hazard_per_dead_ah = 1.0e6;
    let ohms = c.safety.as_ref().unwrap().plating_short_ohms;
    let t = kelvin(-10.0);
    let mut p = Pack::new(
        &config(recommended(), PlatingModel::Reaction, 0.6, t),
        c.clone(),
    )
    .expect("builds");
    let mut died_at = None;
    for k in 1_u32..=600 {
        p.step(1.0, Demand::Current(-2.0 * q), &env(t));
        let dead = p.cell_plating(0, 0).unwrap().dead_mol;
        let g = p.cell(0, 0).unwrap().internal_short_conductance_s;
        if died_at.is_none() && dead > 0.0 {
            died_at = Some(k);
        }
        match died_at {
            // Ticks every 10 s: the first one at or after the death rolls it, and lands one
            // `[safety]` short. Every later tick that sees more lithium die rolls again and, at
            // this hazard, adds another, as the threshold's shorts add.
            Some(d) if k == d.div_ceil(10) * 10 => {
                assert_eq!(g, 1.0 / ohms, "step {k}: the short is the [safety] one");
            }
            Some(d) if k > d.div_ceil(10) * 10 => {
                let n = g * ohms;
                assert!(
                    n >= 1.0 && (n - n.round()).abs() < 1.0e-9,
                    "step {k}: {n} shorts"
                );
            }
            _ => assert_eq!(g, 0.0, "step {k}: a short before any tick saw dead lithium"),
        }
    }
    assert!(died_at.is_some(), "the fixture must plate");
}

// --- the stripping plateau -------------------------------------------------------------------

/// Prediction 11, as registered — a discharge straight after −10 °C 1C CC–CV — could not be read:
/// the discharge after a cold 4.2 V hold runs on unconverged solves
/// (`report_discharge_after_a_hold_extent`), so it would measure that. What the hold leaves as
/// metal, pinned as measured: at −10 °C (fine grid) the reversible store is 47 % of the dead
/// one, where at 0 °C it is 1.2 % (`dead_lithium_is_capacity_once_and_reversible_metal_is_stored_charge`)
/// — the colder hold strips back far less. The plateau itself is measured after the
/// constant-current leg, by `a_short_stripping_step_after_the_constant_current_leg`.
#[test]
fn what_a_cold_hold_leaves_as_metal() {
    let c = chem();
    let charged = cccv(&c, fine(), PlatingModel::Reaction, -10.0, 1.0).pack;
    let pv = charged.cell_plating(0, 0).unwrap();
    let ratio = pv.reversible_mol / pv.dead_mol;
    println!(
        "-10 C 1C CC-CV: reversible {:.4e} mol, dead {:.4e} mol, ratio {ratio:.3e}",
        pv.reversible_mol, pv.dead_mol
    );
    assert!((0.40..0.55).contains(&ratio), "{ratio}");
}

/// Prediction 11's measurement, on the store at its largest: −10 °C 1C to 4.2 V with no hold
/// (fine grid), then straight into C/2 to 2.5 V at −10 °C, against the same state with the
/// reaction taken out of the snapshot. **A miss as registered** (under 2 mV over the control's
/// first 90 %): the reaction arm starts 4.3 mV above the control, the reversible metal stripping
/// beside the intercalation at up to 9 % of the current, and the gap fades as the store empties
/// (1.3 mV at 60 s, the store empty at 76 s, 0.2 mV at 120 s). Pinned as measured: the peak on the
/// first step within 4.0–4.6 mV, the store empty between 60 and 100 s, the gap under 0.5 mV at
/// 120 s, stripping's share under the registered 10 %. The extra charge against the metal
/// stripped is judged within two steps' charge — changed after registration from 1e-6 of
/// capacity, which is below one 1 s step's own charge. C/10 is not run: ten hours on this grid
/// is not a run this slice can afford.
#[test]
fn a_short_stripping_step_after_the_constant_current_leg() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(-10.0);
    let mut charged =
        Pack::new(&config(fine(), PlatingModel::Reaction, 0.0, t), c.clone()).expect("builds");
    loop {
        let tel = charged.step(1.0, Demand::Current(-q), &env(t));
        if tel.v_terminal >= 4.2 {
            break;
        }
    }
    let pv = charged.cell_plating(0, 0).unwrap();
    println!(
        "-10 C 1C CC to 4.2 V: reversible {:.4e} mol ({:.4e} A·h), dead {:.4e} mol",
        pv.reversible_mol,
        pv.reversible_mol * F / 3600.0,
        pv.dead_mol
    );
    let control = {
        let mut v = serde_json::to_value(charged.snapshot()).unwrap();
        v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"]["plating"] = serde_json::Value::Null;
        Pack::restore(&serde_json::from_value(v).unwrap()).expect("restores without the store")
    };
    let c_rate = 0.5;
    let run = |mut p: Pack, arm: &str| {
        let (mut v, mut share, mut ah, mut emptied) = (Vec::new(), 0.0_f64, 0.0, None);
        let mut rev = p.cell_plating(0, 0).map_or(0.0, |pv| pv.reversible_mol);
        let rev0 = rev;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cap = (1.3 * 3600.0 / c_rate) as usize;
        for k in 1..=cap {
            let tel = p.step(1.0, Demand::Current(c_rate * q), &env(t));
            ah += tel.i_actual / 3600.0;
            v.push(tel.v_terminal);
            if let Some(pv) = p.cell_plating(0, 0) {
                share = share.max((rev - pv.reversible_mol) * F / tel.i_actual);
                if rev > 0.0 && pv.reversible_mol == 0.0 {
                    println!("{arm}: the store is empty after {k} s");
                    emptied = Some(k);
                }
                rev = pv.reversible_mol;
            }
            if k % 600 == 0 {
                println!(
                    "{arm} {k} s: {:.4} V, {ah:.4} A·h, reversible {rev:.3e} mol",
                    tel.v_terminal
                );
            }
            // A NaN voltage ends the run too.
            if tel.v_terminal <= 2.5 || tel.v_terminal.is_nan() {
                break;
            }
            assert!(k < cap, "{arm}: no cut-off in {cap} s");
        }
        (v, share, ah, (rev0 - rev) * F / 3600.0, emptied)
    };
    let ((va, share, ah_a, stripped, emptied), (vc, _, ah_c, _, _)) = std::thread::scope(|s| {
        let a = s.spawn(|| run(charged.clone(), "reaction"));
        let b = s.spawn(|| run(control.clone(), "control"));
        (a.join().unwrap(), b.join().unwrap())
    });
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let n = (0.9 * vc.len() as f64) as usize;
    let (dv, at) = va
        .iter()
        .zip(&vc)
        .take(n)
        .map(|(a, b)| (a - b).abs())
        .enumerate()
        .fold(
            (0.0, 0),
            |(m, i), (k, d)| if d > m { (d, k) } else { (m, i) },
        );
    let step_ah = c_rate * q / 3600.0;
    for k in [1, 2, 5, 10, 20, 30, 60, 120, 300, 600, 1200, 1800] {
        if let (Some(a), Some(b)) = (va.get(k - 1), vc.get(k - 1)) {
            println!(
                "  {k:>5} s: reaction {a:.5} V, control {b:.5} V, dV {:+.3} mV",
                (a - b) * 1.0e3
            );
        }
    }
    println!(
        "C/2: max |dV| {:.3} mV at step {} over 90 %; stripping peak share {:.2} %; extra charge {:.4e} A·h against stripped {stripped:.4e} A·h (one step {step_ah:.1e})",
        dv * 1.0e3,
        at + 1,
        share * 100.0,
        ah_a - ah_c
    );
    assert!(
        va.iter().chain(&vc).all(|x| x.is_finite()),
        "a non-finite voltage"
    );
    assert_eq!(at, 0, "the gap peaks on the first step");
    assert!((4.0e-3..4.6e-3).contains(&dv), "the peak: {dv} V");
    assert!(
        emptied.is_some_and(|k| (60..100).contains(&k)),
        "{emptied:?}"
    );
    assert!((va[119] - vc[119]).abs() < 0.5e-3, "the gap at 120 s");
    assert!(share < 0.10, "stripping carried {share}");
    assert!((ah_a - ah_c - stripped).abs() <= 2.0 * step_ah);
}

// --- snapshot, mid-tick ----------------------------------------------------------------------

/// Prediction 12: a reaction pack holding dead amp-hours no tick has consumed yet restores
/// through bincode and through JSON and continues bit for bit, with a hazard large enough that
/// the next ticks roll.
#[test]
fn a_reaction_pack_restores_mid_tick_and_continues_bit_for_bit() {
    let mut c = chem();
    pl(&mut c).short_hazard_per_dead_ah = 30.0;
    let q = c.cell.capacity_ah;
    let t = kelvin(-10.0);
    let mut p =
        Pack::new(&config(recommended(), PlatingModel::Reaction, 0.6, t), c).expect("builds");
    for _ in 0..307 {
        p.step(1.0, Demand::Current(-2.0 * q), &env(t));
    }
    let pending = cell_json(&p)["aging"]["ah_plating_since_tick"]
        .as_f64()
        .unwrap();
    assert!(
        pending > 0.0,
        "the fixture must hold dead amp-hours between ticks"
    );
    let snap = p.snapshot();
    let by_bincode =
        Pack::restore(&bincode::deserialize(&bincode::serialize(&snap).unwrap()).unwrap())
            .expect("restores from bincode");
    let by_json =
        Pack::restore(&serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap())
            .expect("restores from JSON");
    let run = |mut p: Pack| -> Vec<String> {
        let mut out = Vec::new();
        for (dt, d, n) in [
            (1.0, Demand::Current(-2.0 * q), 60),
            (60.0, Demand::Current(-q), 3),
            (1.0, Demand::Rest, 30),
        ] {
            for _ in 0..n {
                out.push(format!("{:?}", p.step(dt, d, &env(t))));
                out.push(format!("{:?}", p.cell(0, 0)));
            }
        }
        out.push(serde_json::to_string(&p.snapshot()).unwrap());
        out
    };
    let original = run(p);
    assert_eq!(run(by_bincode), original, "bincode restore diverged");
    assert_eq!(run(by_json), original, "JSON restore diverged");
}

// --- diagnostic -----------------------------------------------------------------------------

/// Diagnostic: the cost of a C/2 discharge step straight after a 0 °C 1C charge, with the 4.2 V
/// hold and without it, on a reaction cell, on the same state with the reaction taken out, and on
/// a threshold cell charged the same way.
#[test]
#[ignore = "diagnostic: why a discharge after a hold is slow"]
fn report_discharge_after_a_hold_cost() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = kelvin(0.0);
    for model in [recommended(), fine()] {
        for hold in [false, true] {
            for plating in [PlatingModel::Reaction, PlatingModel::Threshold] {
                let mut p = Pack::new(&config(model, plating, 0.0, t), c.clone()).expect("builds");
                let mut held = false;
                loop {
                    let d = if held {
                        Demand::Voltage(4.2)
                    } else {
                        Demand::Current(-q)
                    };
                    let tel = p.step(1.0, d, &env(t));
                    if !held && tel.v_terminal >= 4.2 {
                        if !hold {
                            break;
                        }
                        held = true;
                    } else if held && -tel.i_actual <= 0.05 * q {
                        break;
                    }
                }
                let mut arms = vec![("as charged", p.clone())];
                if plating == PlatingModel::Reaction {
                    let mut v = serde_json::to_value(p.snapshot()).unwrap();
                    v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"]["plating"] =
                        serde_json::Value::Null;
                    arms.push((
                        "store removed",
                        Pack::restore(&serde_json::from_value(v).unwrap()).unwrap(),
                    ));
                }
                for (name, mut a) in arms {
                    let start = std::time::Instant::now();
                    let mut unconverged = 0;
                    let mut flags = EventFlags::empty();
                    for _ in 0..200 {
                        let tel = a.step(1.0, Demand::Current(0.5 * q), &env(t));
                        unconverged += u32::from(tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
                        flags |= tel.flags;
                    }
                    println!(
                        "{} hold {hold} {plating:?} {name}: {:.0} us/step, unconverged {unconverged}, flags {flags:?}, soc {:.6}",
                        if matches!(model, CellModelConfig::Dfn { shells, .. } if shells == FINE_SHELLS) { "fine" } else { "recommended" },
                        start.elapsed().as_secs_f64() / 200.0 * 1.0e6,
                        a.cell(0, 0).unwrap().soc
                    );
                }
            }
        }
    }
}

/// Diagnostic: how long a discharge after a hold stays unconverged, at 0 °C and 25 °C, on a
/// threshold cell (no reaction), the recommended grid.
#[test]
#[ignore = "diagnostic: how long a discharge after a hold stays unconverged"]
fn report_discharge_after_a_hold_extent() {
    let c = chem();
    let q = c.cell.capacity_ah;
    for temp_c in [0.0, 25.0] {
        let t = kelvin(temp_c);
        let mut p = Pack::new(
            &config(recommended(), PlatingModel::Threshold, 0.0, t),
            c.clone(),
        )
        .expect("builds");
        let mut held = false;
        let (mut hold_unc, mut hold_out, mut first_out) = (0, 0, None);
        let mut secs = 0;
        loop {
            let d = if held {
                Demand::Voltage(4.2)
            } else {
                Demand::Current(-q)
            };
            let tel = p.step(1.0, d, &env(t));
            secs += 1;
            hold_unc += u32::from(held && tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
            if tel.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE) {
                hold_out += u32::from(held);
                first_out.get_or_insert((secs, held));
            }
            if !held && tel.v_terminal >= 4.2 {
                held = true;
            } else if held && -tel.i_actual <= 0.05 * q {
                break;
            }
        }
        let (mut unc, mut last_unc) = (0, 0);
        for k in 1..=1200 {
            let tel = p.step(1.0, Demand::Current(0.5 * q), &env(t));
            if tel.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
                unc += 1;
                last_unc = k;
            }
            if k <= 3 || k % 300 == 0 {
                println!(
                    "{temp_c} C step {k}: {:.4} V, {:.4} A, {:?}",
                    tel.v_terminal, tel.i_actual, tel.flags
                );
            }
        }
        println!("{temp_c} C: charge {secs} s; hold unconverged {hold_unc}, hold steps out of range {hold_out}, first out of range (s, on the hold) {first_out:?}; discharge unconverged {unc} of 1200, last at {last_unc}");
        println!(
            "electrode window / capacity_ah - 1 = {:.4e}",
            window_ah(&c) / q - 1.0
        );
    }
}
