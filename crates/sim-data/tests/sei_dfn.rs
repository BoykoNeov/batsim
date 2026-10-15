//! Phase 10 slice D: the SEI film on the `Dfn`. Exit criteria in `docs/plans/phase-10-sei.md`
//! (the slice D addendum).

use sim_core::{
    AgingConfig, CalendarModel, CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig,
    Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

fn chem() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

fn dfn_model() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: sim_core::spm::DEFAULT_SHELLS,
        nodes_negative: sim_core::dfn::DEFAULT_NODES_NEGATIVE,
        nodes_separator: sim_core::dfn::DEFAULT_NODES_SEPARATOR,
        nodes_positive: sim_core::dfn::DEFAULT_NODES_POSITIVE,
    }
}

fn spm_model() -> CellModelConfig {
    CellModelConfig::Spm {
        shells: sim_core::spm::DEFAULT_SHELLS,
    }
}

/// A single filmed cell of `model` at `soc`, pinned at `temp_k`.
fn filmed(model: CellModelConfig, soc: f64, temp_k: f64) -> Pack {
    Pack::new(&config(model, soc, temp_k), chem()).expect("a filmed pack builds")
}

/// [`filmed`]'s configuration, for a test that builds it on another chemistry.
fn config(model: CellModelConfig, soc: f64, temp_k: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 1,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: Some(AgingConfig {
            calendar: CalendarModel::Film,
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

const GOLDEN: &str = include_str!("../../../tests/golden/nmc_21700_lgm50/sei_rest_year.csv");

/// `(soc, temp_k, [(day, lithium lost [mol])])`, one golden condition.
type Condition = (f64, f64, Vec<(f64, f64)>);

/// One condition of `sei_rest_year.csv`: `(soc, temp_k, [(day, lithium lost on the table
/// reference [mol])])`. The function column is the `Spm`'s concern (`sei_golden.rs`); here the
/// `Dfn` is held to the reference the `Spm` is held to tightly.
fn references() -> Vec<Condition> {
    let mut out: Vec<Condition> = Vec::new();
    for line in GOLDEN.lines() {
        if line.starts_with('#') || line.starts_with("soc") || line.is_empty() {
            continue;
        }
        let v: Vec<f64> = line
            .split(',')
            .map(|x| x.parse().expect("a number"))
            .collect();
        match out.last_mut() {
            Some(r) if r.0 == v[0] && r.1 == v[1] => r.2.push((v[2], v[4])),
            _ => out.push((v[0], v[1], vec![(v[2], v[4])])),
        }
    }
    out
}

/// Exit 1 of slice D: **at rest the `Dfn` is the `Spm`.** With no current the electrolyte is
/// uniform and every node's reaction zero, so each node's film grows at the potential the
/// `Spm`'s one particle sits at, and a year at rest at the golden's five conditions must give
/// the same lithium; and the `Dfn` replays PyBaMM's table reference to the `Spm`'s own
/// 0.01 %, from day 7.
///
/// # The bar is the `Dfn`'s Newton tolerance, and the first one missed
/// The `Dfn` reads each node's potential off its last solve, which stops once the reaction
/// row is within [`sim_core::dfn::NEWTON_TOL`] of 1 A/m². So at rest a node's potential can
/// sit up to `NEWTON_TOL/(2·i₀·αF/RT)` from its OCP, and the film's rate, `exp(−FΔφ/RT)`, up
/// to about `NEWTON_TOL/i₀` from the `Spm`'s — some 4e-8 on the plateau, where `i₀` is about
/// 0.26 A/m². The bar is 1e-7. The plan first set 1e-9 from the first day's reading
/// (1.7e-13); the year read 6.5e-9 at 85 % and 25 °C, all of it arriving between days 30 and
/// 100, while the four other conditions stayed under 1e-13. With the tolerance tightened to
/// 1e-12 for one run the same year read 4e-16, which is what names the cause.
///
/// This cannot tell where the film's resistance sits — at rest no current crosses it.
#[test]
fn at_rest_the_dfn_film_is_the_spm_film() {
    for (soc, temp_k, rows) in references() {
        let mut d = filmed(dfn_model(), soc, temp_k);
        let mut s = filmed(spm_model(), soc, temp_k);
        let mut hours = 0_usize;
        let (mut worst_spm, mut worst_ref) = (0.0_f64, 0.0_f64);
        for &(day, reference) in &rows {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let target = (day * 24.0).round() as usize;
            while hours < target {
                d.step(3600.0, Demand::Rest, &env(temp_k));
                s.step(3600.0, Demand::Rest, &env(temp_k));
                hours += 1;
            }
            if day == 0.0 {
                continue;
            }
            let li_d = d
                .cell_film(0, 0)
                .expect("the Dfn has a film")
                .lithium_lost_mol;
            let li_s = s
                .cell_film(0, 0)
                .expect("the Spm has a film")
                .lithium_lost_mol;
            worst_spm = worst_spm.max((li_d / li_s - 1.0).abs());
            if day >= 7.0 {
                worst_ref = worst_ref.max((li_d / reference - 1.0).abs());
            }
        }
        println!(
            "SOC {soc} at {temp_k} K: Dfn vs Spm {worst_spm:.3e}, Dfn vs PyBaMM table {:.5} %",
            worst_ref * 100.0
        );
        assert!(
            worst_spm < 1e-7,
            "SOC {soc} at {temp_k} K: the Dfn's film parts from the Spm's at rest by \
             {worst_spm:.3e}, past the 1e-7 its Newton tolerance allows (measured 6.5e-9 at \
             worst)"
        );
        assert!(
            worst_ref < 1e-4,
            "SOC {soc} at {temp_k} K: the Dfn's film is {:.5} % from PyBaMM on the engine's \
             table, past the Spm's 0.01 %",
            worst_ref * 100.0
        );
    }
}

/// The cell's `Dfn` state, read out of its snapshot.
fn dfn_state(p: &Pack) -> sim_core::DfnState {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    serde_json::from_value(v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"].clone())
        .expect("cell 0 is a Dfn cell")
}

/// The film's term in the analytic Jacobian matches a difference quotient of the residual,
/// on a film that is **uneven** across the negative — the case only the `Dfn` has, and the
/// one where a wrong `∂η/∂j` would move the reaction between nodes rather than only slow a
/// Newton. Each node's film is set by hand, from 5 nm (fresh) at the collector to 245 nm at
/// the separator, so every node carries a different `ρ·(L − L0)`; then the same
/// row-relative comparison as `dfn_cell.rs`'s, at its tolerance, under discharge, charge and
/// rest. Measured at 0.9–1.2e-9. Perturbed once at build time: dropping the film's `− r_film`
/// from the Jacobian's `∂η/∂j` reads 1.3e-2 on the discharge case, the first it reaches.
#[test]
fn the_film_term_in_the_jacobian_matches_a_difference_quotient() {
    use sim_core::dfn::probe;
    let chem = chem();
    let (spm, d) = (
        chem.spm.as_ref().expect("[spm]"),
        chem.dfn.as_ref().expect("[dfn]"),
    );
    let health = probe::Health {
        eff_r0_factor: 1.0,
        eff_capacity_ah: probe::spm_capacity(spm),
    };
    let capacity = chem.cell.capacity_ah;
    for (label, i) in [
        ("discharge", capacity),
        ("charge", -capacity),
        ("rest", 0.0),
    ] {
        let mut p = filmed(dfn_model(), 0.6, 298.15);
        for _ in 0..20 {
            p.step(10.0, Demand::Current(i), &env(298.15));
        }
        let mut s = dfn_state(&p);
        let n = s.c_neg.len();
        #[allow(clippy::cast_precision_loss)]
        let film: Vec<f64> = (0..n)
            .map(|k| 5.0e-9 + 240.0e-9 * k as f64 / (n - 1) as f64)
            .collect();
        s.film_thickness_m = Some(film);
        let (m, analytic, numeric) = probe::jacobian_pair(&s, spm, d, health, i, 10.0, 1.0e-6);
        let mut worst = 0.0_f64;
        for row in 0..m {
            let scale = (0..m)
                .map(|c| analytic[row * m + c].abs().max(numeric[row * m + c].abs()))
                .fold(1.0e-30_f64, f64::max);
            for col in 0..m {
                worst = worst.max((analytic[row * m + col] - numeric[row * m + col]).abs() / scale);
            }
        }
        println!("{label}: worst row-relative disagreement {worst:.3e}");
        assert!(
            worst < 1.0e-5,
            "{label}: worst row-relative Jacobian disagreement {worst:.3e} on an uneven film"
        );
    }
}

/// Exit 3 of slice D: the resistance a filmed `Dfn` reports rises from the film alone — the
/// `Spm`'s exit 4 on this model. At rest no current reaches the drawn mechanisms, and a
/// chemistry with `r_growth_per_capacity_loss` at zero runs bit-identically to the shipped
/// one, so nothing reached through that coupling; ninety days at 45 °C and 85 % then raise the
/// reported resistance, and the film reports ohms of its own. On an even film (rest) the
/// nodes' films in parallel are the `Spm`'s series value, so the two models report the same
/// film resistance to rounding.
#[test]
fn resistance_rises_from_the_film_alone() {
    let mut uncoupled = chem();
    uncoupled
        .aging
        .as_mut()
        .expect("the LG M50 file has [aging]")
        .r_growth_per_capacity_loss = 0.0;
    let build = |chem: ChemistryParams, model: CellModelConfig| {
        Pack::new(&config(model, 0.85, 318.15), chem).expect("builds")
    };
    let mut a = build(chem(), dfn_model());
    let mut b = build(uncoupled, dfn_model());
    let mut s = build(chem(), spm_model());
    let mut last = None;
    for _ in 0..90 * 24 {
        let ta = a.step(3600.0, Demand::Rest, &env(318.15));
        let tb = b.step(3600.0, Demand::Rest, &env(318.15));
        s.step(3600.0, Demand::Rest, &env(318.15));
        assert_eq!(
            serde_json::to_string(&ta).expect("telemetry"),
            serde_json::to_string(&tb).expect("telemetry"),
            "the resistance coupling of the drawn mechanisms reached a resting filmed Dfn"
        );
        last = Some(ta);
    }
    let tele = last.expect("stepped");
    let film = a.cell_film(0, 0).expect("film");
    let spm_film = s.cell_film(0, 0).expect("film");
    println!(
        "Dfn film {:.6e} ohm, Spm film {:.6e} ohm, ratio {:.6}",
        film.resistance_ohm, spm_film.resistance_ohm, tele.soh_resistance
    );
    assert!(
        film.resistance_ohm > 0.0,
        "the film grew and added no resistance"
    );
    assert!(
        (film.resistance_ohm / spm_film.resistance_ohm - 1.0).abs() < 1e-7,
        "an even film's nodes in parallel are the Spm's film in series: {} against {} ohm",
        film.resistance_ohm,
        spm_film.resistance_ohm
    );
    assert!(
        tele.soh_resistance > 1.0,
        "ninety hot days of film must show in the reported resistance: {}",
        tele.soh_resistance
    );
    assert_eq!(
        a.cell(0, 0).expect("cell").soh_resistance,
        tele.soh_resistance,
        "a single cell's own resistance ratio and the pack's are the same quantity"
    );
}

/// Volume-weighted mean of a particle's shells, the quantity the scheme conserves.
fn mean(c: &[f64]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (i, &ci) in c.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let (lo, hi) = (i as f64, (i + 1) as f64);
        let vol = hi * hi * hi - lo * lo * lo;
        num += ci * vol;
        den += vol;
    }
    num / den
}

/// `∫ U dx` over a piecewise-linear table between `a` and `b`, exactly (trapezoids on the
/// table's own segments).
fn integral(xs: &[f64], vs: &[f64], a: f64, b: f64) -> f64 {
    let (lo, hi, sign) = if a <= b { (a, b, 1.0) } else { (b, a, -1.0) };
    let u = |x: f64| {
        let k = xs.partition_point(|&p| p < x).clamp(1, xs.len() - 1);
        let t = (x - xs[k - 1]) / (xs[k] - xs[k - 1]);
        vs[k - 1] + t * (vs[k] - vs[k - 1])
    };
    let mut points = vec![lo];
    points.extend(xs.iter().copied().filter(|&x| x > lo && x < hi));
    points.push(hi);
    let sum: f64 = points
        .windows(2)
        .map(|w| 0.5 * (u(w[0]) + u(w[1])) * (w[1] - w[0]))
        .sum();
    sign * sum
}

/// Exit 4 of slice D: the film's energy closes the cell's ledger, per node. At rest the
/// terminals move nothing and the cell makes no heat, so every joule the electrodes' stored
/// energy changes by is the film's: each negative node's lithium, read against lithium metal,
/// rises by `∫U_n·dq` over that node's own fall in stoichiometry, and
/// [`sim_core::FilmView::energy_last_step_j`] summed over a hot year must be the sum over
/// nodes. Measured on the snapshot's profiles, not on the film's own count. Held to the
/// `Spm`'s 1e-6.
#[test]
fn the_film_closes_the_energy_ledger() {
    let chem = chem();
    let n = &chem.spm.as_ref().expect("[spm]").negative;
    let mut pack = filmed(dfn_model(), 0.85, 318.15);
    let s0 = dfn_state(&pack);
    let (mut booked, mut electrical, mut heat) = (0.0, 0.0, 0.0);
    for _ in 0..365 * 24 {
        let t = pack.step(3600.0, Demand::Rest, &env(318.15));
        electrical += t.v_terminal * t.i_actual * 3600.0;
        heat += t.q_gen_w * 3600.0;
        booked += pack
            .cell_film(0, 0)
            .and_then(|f| f.energy_last_step_j)
            .expect("a filmed cell reports its film's energy every step");
    }
    let s1 = dfn_state(&pack);
    // Against a control cell with no film. The TOTAL, not node by node: the film sits inside
    // each negative node's reaction, so at rest it moves lithium between positive nodes
    // through the electrolyte while conserving the sum. The solve's own rounding of a resting
    // profile grew when Phase 11 gave the diffusivity an Arrhenius factor (the old bar here
    // was on that drift); the claim is about the film, so the control carries the same drift.
    let mut cfg = config(dfn_model(), 0.85, 318.15);
    cfg.aging = Some(AgingConfig {
        calendar: CalendarModel::Curve,
        ..AgingConfig::default()
    });
    let mut control = Pack::new(&cfg, chem.clone()).expect("a control pack builds");
    for _ in 0..365 * 24 {
        control.step(3600.0, Demand::Rest, &env(318.15));
    }
    let pos = |s: &sim_core::DfnState| s.c_pos.iter().map(|p| mean(p)).sum::<f64>();
    let (filmed_pos, control_pos) = (pos(&s1), pos(&dfn_state(&control)));
    println!("positive lithium: {filmed_pos} filmed, {control_pos} without a film");
    assert!(
        (filmed_pos / control_pos - 1.0).abs() < 1e-12,
        "a resting film touches no positive lithium: {filmed_pos} against {control_pos}"
    );
    #[allow(clippy::cast_precision_loss)]
    let nodes = s0.c_neg.len() as f64;
    // Coulombs of the engine's charge per unit of one node's stoichiometry.
    let per_stoich = chem.cell.capacity_ah * 3600.0 / (n.stoich_max - n.stoich_min) / nodes;
    let stored: f64 = s0
        .c_neg
        .iter()
        .zip(&s1.c_neg)
        .map(|(a, b)| {
            let (x0, x1) = (mean(a) / n.c_max_mol_per_m3, mean(b) / n.c_max_mol_per_m3);
            // At `U − t_ref·∂U/∂T`, as `sei_film.rs` and the engine book it (Phase 11).
            let entropic = n
                .entropic
                .as_ref()
                .map_or(0.0, |t| integral(&t.stoich, &t.v_per_k, x1, x0));
            per_stoich
                * (integral(&n.ocp.stoich, &n.ocp.volts, x1, x0)
                    - chem.spm.as_ref().expect("[spm]").t_ref_k * entropic)
        })
        .sum();
    println!("booked {booked} J, stored rose by {stored} J");
    assert!(
        stored > 0.0,
        "the film took lithium and the ledger saw none"
    );
    assert_eq!(electrical, 0.0, "a resting cell moved charge");
    assert_eq!(heat, 0.0, "a resting cell made heat");
    assert!(
        (booked / stored - 1.0).abs() < 1e-6,
        "the film booked {booked} J, the electrodes' stored energy rose by {stored} J"
    );
}

/// Exit 5 of slice D: a filmed `Dfn` snapshot taken mid-run, restored through a real
/// serialization, continues bit-identically under current, rest and a voltage hold — the
/// per-node film is state, and its lithium is in the profiles.
#[test]
fn a_filmed_dfn_survives_a_snapshot() {
    let schedule = |k: usize| match k % 4 {
        0 => Demand::Current(4.0),
        1 => Demand::Rest,
        2 => Demand::Current(-3.0),
        _ => Demand::Voltage(3.9),
    };
    let mut straight = filmed(dfn_model(), 0.55, 308.15);
    let reference: Vec<String> = (0..200)
        .map(|k| serde_json::to_string(&straight.step(60.0, schedule(k), &env(308.15))).expect("t"))
        .collect();
    let mut split = filmed(dfn_model(), 0.55, 308.15);
    for k in 0..100 {
        split.step(60.0, schedule(k), &env(308.15));
    }
    let text = serde_json::to_string(&split.snapshot()).expect("serializes");
    let snapshot: sim_core::Snapshot = serde_json::from_str(&text).expect("deserializes");
    let mut continued = Pack::restore(&snapshot).expect("restores");
    for (k, expected) in reference.iter().enumerate().skip(100) {
        let got =
            serde_json::to_string(&continued.step(60.0, schedule(k), &env(308.15))).expect("t");
        assert_eq!(&got, expected, "step {k} after a restore differs");
    }
    assert_eq!(dfn_state(&continued), dfn_state(&straight));
}

/// A fresh film on a `Dfn` is the chemistry's initial film, which adds nothing, so until the
/// first aging tick a filmed `Dfn` is bit for bit the `Dfn` with the drawn curve — the
/// per-node term in each reaction is `j·0.0` until a node grows. After the tick they part.
#[test]
fn a_fresh_film_moves_nothing_until_it_grows() {
    let mut curve_config = config(dfn_model(), 0.6, 298.15);
    curve_config.aging = Some(AgingConfig::default());
    let mut a = Pack::new(&curve_config, chem()).expect("builds");
    let mut b = filmed(dfn_model(), 0.6, 298.15);
    let demands = [Demand::Current(3.0), Demand::Rest, Demand::Current(-2.0)];
    // Nine one-second steps: the 10 s sub-clock has not ticked.
    for k in 0..9 {
        let ta = a.step(1.0, demands[k % 3], &env(298.15));
        let tb = b.step(1.0, demands[k % 3], &env(298.15));
        assert_eq!(
            serde_json::to_string(&ta).expect("telemetry"),
            serde_json::to_string(&tb).expect("telemetry"),
            "step {k}: a fresh film moved the Dfn"
        );
    }
    let ta = a.step(1.0, Demand::Rest, &env(298.15));
    let tb = b.step(1.0, Demand::Rest, &env(298.15));
    assert_ne!(
        ta.soh_capacity, tb.soh_capacity,
        "after the first tick the curve and the film are different mechanisms"
    );
}

/// A filmed `Dfn`'s state of charge is a fraction of the capacity it reports, as the `Spm`'s
/// is (`sei_film.rs`): after a hot year at 85 % a slow discharge moves the reported state of
/// charge times the reported capacity by the amp-hours it delivers, to 1e-4.
#[test]
fn state_of_charge_counts_against_the_capacity_the_cell_has() {
    let mut pack = filmed(dfn_model(), 0.85, 318.15);
    for _ in 0..365 * 24 {
        pack.step(3600.0, Demand::Rest, &env(318.15));
    }
    let before = pack.step(0.0, Demand::Rest, &env(318.15));
    let cap_ah = chem().cell.capacity_ah;
    let i = cap_ah / 20.0;
    let mut ah = 0.0;
    let mut after = before;
    for _ in 0..4 * 60 {
        after = pack.step(60.0, Demand::Current(i), &env(318.15));
        ah += after.i_actual * 60.0 / 3600.0;
    }
    assert!(
        before.soh_capacity < 0.98,
        "the year should have cost a few percent of capacity: {}",
        before.soh_capacity
    );
    let counted = (before.soc_true - after.soc_true) * before.soh_capacity * cap_ah;
    assert!(
        (counted / ah - 1.0).abs() < 1e-4,
        "delivered {ah} Ah, the reported state of charge and capacity count {counted} Ah"
    );
}

/// The film shortens the negative's window at the top by the lithium it took, so a filmed
/// cell is full when its **positive** is: charged past it, a hot-year-aged `Dfn` raises
/// `SOC_CLAMPED_HIGH` on the step its positive electrode's bulk passes the chemistry's full
/// (`stoich_min`), not before and not one step late. The plan's "the positive's edge binds on
/// a charge", checked rather than assumed.
#[test]
fn an_aged_dfn_is_full_when_its_positive_is() {
    let chem = chem();
    let p_min = chem.spm.as_ref().expect("[spm]").positive.stoich_min;
    let p_cmax = chem.spm.as_ref().expect("[spm]").positive.c_max_mol_per_m3;
    let mut pack = filmed(dfn_model(), 0.85, 318.15);
    for _ in 0..365 * 24 {
        pack.step(3600.0, Demand::Rest, &env(318.15));
    }
    let i = -chem.cell.capacity_ah / 20.0;
    let y = |p: &Pack| {
        let s = dfn_state(p);
        s.c_pos.iter().map(|c| mean(c)).sum::<f64>() / s.c_pos.len() as f64 / p_cmax
    };
    let mut flagged = None;
    for k in 0..40 * 60 {
        let y0 = y(&pack);
        let t = pack.step(60.0, Demand::Current(i), &env(298.15));
        if t.flags.contains(sim_core::EventFlags::SOC_CLAMPED_HIGH) {
            flagged = Some((k, y0, y(&pack)));
            break;
        }
    }
    let (k, y0, y1) = flagged.expect("a charge past full raises SOC_CLAMPED_HIGH");
    println!("flagged at step {k}: positive bulk {y0:.7} -> {y1:.7}, full {p_min}");
    assert!(
        y0 >= p_min && y1 < p_min,
        "the flag rose on step {k}, where the positive went {y0} -> {y1}; it should be the \
         step it passed its full, {p_min}"
    );
}

const CYCLING: &str = include_str!("../../../tests/golden/nmc_21700_lgm50/sei_dfn_cycling.csv");

/// One row of `sei_dfn_cycling.csv`.
struct CycRow {
    rate: f64,
    growing: bool,
    t_s: f64,
    li_mol: f64,
    dphi: Vec<f64>,
    dl: Vec<f64>,
}

fn cycling_rows() -> Vec<CycRow> {
    CYCLING
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("rate_c") && !l.is_empty())
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().expect("a number")).collect();
            let n = (v.len() - 4) / 2;
            CycRow {
                rate: v[0],
                growing: v[1] == 1.0,
                t_s: v[2],
                li_mol: v[3],
                dphi: v[4..4 + n].to_vec(),
                dl: v[4 + n..].to_vec(),
            }
        })
        .collect()
}

/// The engine's side of one cycling case: the chemistry with the fresh film at 1e-12 and, when
/// not `growing`, `D_li` at 1e-300 (the loader refuses zero); a filmed `Dfn` started from an
/// even 50 nm. Eight cycles of ±0.25 of capacity from 50 %, in steps of a sixtieth of the
/// reference's sample spacing (1 s at 1C, 0.9375 s at 2C), sampled at the reference's times:
/// `(t_s, lithium grown [mol], φ_s − φ_e per node [V], film grown per node [m])`.
fn engine_cycling(rate: f64, growing: bool, times: &[f64]) -> Vec<(f64, f64, Vec<f64>, Vec<f64>)> {
    let mut chem = chem();
    {
        let sei = chem
            .spm
            .as_mut()
            .expect("[spm]")
            .sei
            .as_mut()
            .expect("[spm.sei]");
        sei.initial_thickness_m = 1e-12;
        if !growing {
            sei.d_li_m2_per_s = 1e-300;
        }
    }
    let cap = chem.cell.capacity_ah;
    let fresh = Pack::new(&config(dfn_model(), 0.5, 298.15), chem).expect("builds");
    let mut v = serde_json::to_value(fresh.snapshot()).expect("a snapshot is JSON");
    let n = dfn_state(&fresh).c_neg.len();
    v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"]["film_thickness_m"] =
        serde_json::json!(vec![50e-9; n]);
    let snap: sim_core::Snapshot = serde_json::from_value(v).expect("a snapshot");
    let mut p = Pack::restore(&snap).expect("restores");
    let read = |p: &Pack| {
        let s = dfn_state(p);
        let dphi: Vec<f64> = (0..n).map(|i| s.u[4 * i + 2] - s.u[4 * i + 1]).collect();
        let l = s.film_thickness_m.expect("a film");
        let li = p.cell_film(0, 0).expect("a film").lithium_lost_mol;
        (dphi, l, li)
    };
    let (_, l0, li0) = read(&p);
    let dt = times[0] / 60.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let leg = (900.0 / rate / dt).round() as usize;
    let mut out = Vec::new();
    let mut t = 0_usize;
    let mut next = times.iter().peekable();
    for _ in 0..8 {
        for sign in [1.0, -1.0] {
            for _ in 0..leg {
                p.step(dt, Demand::Current(sign * rate * cap), &env(298.15));
                t += 1;
                #[allow(clippy::cast_precision_loss)]
                let now = t as f64 * dt;
                if next.peek().is_some_and(|&&x| (x - now).abs() < 1e-6) {
                    next.next();
                    let (dphi, l, li) = read(&p);
                    let dl = l.iter().zip(&l0).map(|(a, b)| a - b).collect();
                    out.push((now, li - li0, dphi, dl));
                }
            }
        }
    }
    out
}

/// Exit 2 of slice D: **under current, the film grows across the electrode as PyBaMM's DFN
/// grows it.** PyBaMM's DFN with the interstitial law, on the engine's grid and OCP tables, from
/// one even 50 nm film (both sides' film resistance is then the whole film's), eight cycles of
/// ±0.25 of capacity from 50 % at 25 °C, at 1C and 2C. The only exit that sees what the `Dfn`
/// does and the `Spm` cannot: the reaction crowds toward the separator, the film there grows
/// several times faster, and a thicker film at a node — its resistance inside that node's
/// reaction — pushes the reaction away from it.
///
/// # The bar, set before this comparison was looked at
/// The two DFNs are not one discretisation, and the film's rate is exponential in the
/// potential, so the bar is measured: with the film **frozen** on both sides (`D_li` 0 in
/// PyBaMM, 1e-300 here), their negative `φ_s − φ_e` differ by at most 0.167 mV at 1C and
/// 0.531 mV at 2C — rate floors of 0.65 % and 2.09 % — and the engine's 10 s frozen-potential
/// tick costs 0.80 % and 0.15 % on the worst node against one tick per step. The bar is 2.0 %
/// at 1C and 3.0 % at 2C on the lithium grown and on every node's growth, from the end of the
/// first cycle, and twice that on the thickest-over-thinnest ratio. The floor is re-measured
/// here each run and must stay under what the bar was built from; the tick's cost was measured
/// once, at build time, and is not.
///
/// # What a broken film resistance does here, measured
/// With the film's drop taken out of the reaction for one run, the floor check fails first
/// (5.16 mV). With that check also disabled, the lithium grown fails on its own (15.9 % at 1C,
/// against 2 %); the spread does **not** (it moved 1.8 %, inside its 4 %), so the spread
/// assertion alone would not catch a misplaced film resistance.
#[test]
fn under_current_the_film_grows_as_pybamm_grows_it() {
    let rows = cycling_rows();
    let f_rt = 96_485.332_12 / (8.314_462_618 * 298.15);
    for (rate, bar, floor_mv) in [(1.0, 0.02, 0.167), (2.0, 0.03, 0.532)] {
        let frozen: Vec<&CycRow> = rows
            .iter()
            .filter(|r| r.rate == rate && !r.growing && r.t_s > 0.0)
            .collect();
        let times: Vec<f64> = frozen.iter().map(|r| r.t_s).collect();
        let engine = engine_cycling(rate, false, &times);
        assert_eq!(
            engine.len(),
            frozen.len(),
            "{rate}C: every reference time sampled"
        );
        let gap = engine
            .iter()
            .zip(&frozen)
            .flat_map(|(e, r)| e.2.iter().zip(&r.dphi).map(|(a, b)| (a - b).abs()))
            .fold(0.0_f64, f64::max);
        assert!(
            gap * 1e3 < floor_mv,
            "{rate}C: with the film frozen the two DFNs' potentials now part by {:.4} mV, past \
             the {floor_mv} mV the bar was built from",
            gap * 1e3
        );

        let growing: Vec<&CycRow> = rows
            .iter()
            .filter(|r| r.rate == rate && r.growing && r.t_s > 0.0)
            .collect();
        let times: Vec<f64> = growing.iter().map(|r| r.t_s).collect();
        let engine = engine_cycling(rate, true, &times);
        assert_eq!(
            engine.len(),
            growing.len(),
            "{rate}C: every reference time sampled"
        );
        let first_cycle = 1800.0 / rate;
        let (mut worst_li, mut worst_node) = (0.0_f64, 0.0_f64);
        for (e, r) in engine.iter().zip(&growing) {
            if r.t_s < first_cycle - 1e-6 {
                continue;
            }
            worst_li = worst_li.max((e.1 / r.li_mol - 1.0).abs());
            for (a, b) in e.3.iter().zip(&r.dl) {
                worst_node = worst_node.max((a / b - 1.0).abs());
            }
        }
        let (e, r) = (
            engine.last().expect("sampled"),
            growing.last().expect("rows"),
        );
        let spread = |dl: &[f64]| {
            let hi = dl.iter().copied().fold(0.0_f64, f64::max);
            let lo = dl.iter().copied().fold(f64::INFINITY, f64::min);
            hi / lo
        };
        let (s_e, s_r) = (spread(&e.3), spread(&r.dl));
        println!(
            "{rate}C: floor {:.4} mV ({:.3} % rate); lithium {:.4} %, worst node {:.4} %, spread \
             engine {s_e:.4} PyBaMM {s_r:.4} ({:.4} %)",
            gap * 1e3,
            ((f_rt * gap).exp() - 1.0) * 100.0,
            worst_li * 100.0,
            worst_node * 100.0,
            (s_e / s_r - 1.0) * 100.0
        );
        assert!(
            worst_li < bar,
            "{rate}C: the lithium the film took is {:.3} % from PyBaMM's, past the {:.1} % bar",
            worst_li * 100.0,
            bar * 100.0
        );
        assert!(
            worst_node < bar,
            "{rate}C: a node's film growth is {:.3} % from PyBaMM's, past the {:.1} % bar",
            worst_node * 100.0,
            bar * 100.0
        );
        assert!(
            (s_e / s_r - 1.0).abs() < 2.0 * bar,
            "{rate}C: the film's spread across the negative is {s_e:.4} here and {s_r:.4} in \
             PyBaMM"
        );
    }
}
