//! Phase 10 slice C: the SEI film's engine contracts, beside the PyBaMM comparison in
//! `sei_golden.rs` — exit 4 (resistance from the film alone), the tick, the energy ledger,
//! the refusals, the loader, the snapshot, and what a fresh film moves.
//! See `docs/plans/phase-10-sei.md`.

use serde_json::Value;
use sim_core::{
    AgingConfig, BuildError, CalendarModel, CellModelConfig, ChemistryParams, Demand, Env, Pack,
    PackConfig, Scatter, Snapshot, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");
const PRADA: &str = include_str!("../../../chemistries/lfp_26650_prada2013.toml");

fn lgm50() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

fn config(soc: f64, temp_k: f64, calendar: CalendarModel, model: CellModelConfig) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 3,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: Some(AgingConfig {
            calendar,
            ..AgingConfig::default()
        }),
        cell_model: model,
    }
}

const SPM: CellModelConfig = CellModelConfig::Spm { shells: 20 };

fn filmed(chem: ChemistryParams, soc: f64, temp_k: f64) -> Pack {
    Pack::new(&config(soc, temp_k, CalendarModel::Film, SPM), chem).expect("a filmed pack builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

/// Exit 4: the resistance a filmed cell reports rises from the film alone.
///
/// The drawn mechanisms still couple their loss to resistance through
/// `r_growth_per_capacity_loss`, so the test takes that coupling off the path two ways: at
/// rest the cell carries no current, so cycle, plating and reversal lose nothing and the
/// multiplier the model is handed stays exactly `1.0`; and a chemistry with the coupling set
/// to zero runs **bit-identically** to the shipped one, so nothing reached through it.
/// Ninety days at 45 °C and 85 % then raise the reported resistance by the film's ohms, which
/// [`sim_core::FilmView::resistance_ohm`] reports on its own: measured, 0.61 mΩ of film and a
/// ratio of 1.039, with 1.3 % of the capacity gone.
#[test]
fn resistance_rises_from_the_film_alone() {
    let mut uncoupled = lgm50();
    uncoupled
        .aging
        .as_mut()
        .expect("the LG M50 file has [aging]")
        .r_growth_per_capacity_loss = 0.0;
    let mut a = filmed(lgm50(), 0.85, 318.15);
    let mut b = filmed(uncoupled, 0.85, 318.15);
    let mut last = None;
    for _ in 0..90 * 24 {
        let ta = a.step(3600.0, Demand::Rest, &env(318.15));
        let tb = b.step(3600.0, Demand::Rest, &env(318.15));
        assert_eq!(
            serde_json::to_string(&ta).expect("telemetry"),
            serde_json::to_string(&tb).expect("telemetry"),
            "the resistance coupling of the drawn mechanisms reached a resting filmed cell"
        );
        last = Some(ta);
    }
    let tele = last.expect("stepped");
    let film = a.cell_film(0, 0).expect("film");
    assert!(
        film.resistance_ohm > 0.0,
        "the film grew and added no resistance"
    );
    assert!(
        tele.soh_resistance > 1.02,
        "ninety hot days of film must show in the reported resistance: {}",
        tele.soh_resistance
    );
    assert_eq!(
        a.cell(0, 0).expect("cell").soh_resistance,
        tele.soh_resistance,
        "a single cell's own resistance ratio and the pack's are the same quantity"
    );
}

/// The closed form is exact over a tick at a frozen potential, so the tick length changes
/// the answer only through how often the potential is re-read. Two days at 45 °C and 85 %
/// grown in 10 s ticks (the default sub-clock, 17 280 of them) and in one-hour ticks (48)
/// agree to 1.2e-7 relative (measured; held to 1e-6). The year-long golden runs one-hour
/// ticks only, so this is what
/// stands behind the 10 s case — and behind the rationalised increment, whose naive form
/// `√(L² + 2·k·e·dt) − L` loses most of its digits at a 10 s tick.
#[test]
fn ten_second_and_hour_ticks_agree() {
    let grow = |dt: f64| {
        let mut p = filmed(lgm50(), 0.85, 318.15);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (2.0 * 86_400.0 / dt).round() as usize;
        for _ in 0..n {
            p.step(dt, Demand::Rest, &env(318.15));
        }
        p.cell_film(0, 0).expect("film").lithium_lost_mol
    };
    let (fine, coarse) = (grow(10.0), grow(3600.0));
    assert!(
        (fine / coarse - 1.0).abs() < 1e-6,
        "two days of film: {fine:e} mol in 10 s ticks, {coarse:e} mol in 1 h ticks"
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

/// The one cell's `(c_neg, c_pos)` out of a snapshot, by its serde shape.
fn particles(pack: &Pack) -> (Vec<f64>, Vec<f64>) {
    let v = serde_json::to_value(pack.snapshot()).expect("the snapshot serializes");
    let spm = &v["pack"]["groups"][0]["cells"][0]["model"]["Spm"];
    let read = |k: &str| -> Vec<f64> {
        spm[k]
            .as_array()
            .expect("a profile")
            .iter()
            .map(|x| x.as_f64().expect("a number"))
            .collect()
    };
    (read("c_neg"), read("c_pos"))
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

/// The film's energy closes the cell's ledger, read per electrode against lithium metal.
///
/// At rest the terminals move nothing and the cell makes no heat, so every joule the
/// electrodes' stored energy changes by is the film's. Lithium in the negative is worth
/// `−U_n` per coulomb of the engine's charge, so withdrawing it raises the stored energy by
/// `∫U_n·dq` — and [`sim_core::FilmView::energy_last_step_j`], summed over a hot year, must
/// be that integral. Measured on the electrodes from the snapshot, not from the film's own
/// count. Booking the lithium at the cell's voltage instead would be about 40 times this;
/// booking it as a loss, the wrong sign.
#[test]
fn the_film_closes_the_energy_ledger() {
    let chem = lgm50();
    let spm = chem.spm.clone().expect("[spm]");
    let n = &spm.negative;
    let mut pack = filmed(chem.clone(), 0.85, 318.15);
    let (neg0, pos0) = particles(&pack);
    let mut booked = 0.0;
    let mut electrical = 0.0;
    let mut heat = 0.0;
    for _ in 0..365 * 24 {
        let t = pack.step(3600.0, Demand::Rest, &env(318.15));
        electrical += t.v_terminal * t.i_actual * 3600.0;
        heat += t.q_gen_w * 3600.0;
        booked += pack
            .cell_film(0, 0)
            .and_then(|f| f.energy_last_step_j)
            .expect("a filmed cell reports its film's energy every step");
    }
    let (neg1, pos1) = particles(&pack);
    // Not bit-equal: the diffusion solve rounds a uniform profile, film or no film — by
    // 4e-15 of its lithium over this year (measured). The film takes nothing from it.
    let (p0, p1) = (mean(&pos0), mean(&pos1));
    assert!(
        (p1 / p0 - 1.0).abs() < 1e-12,
        "a resting film touches no positive lithium: {p0} -> {p1}"
    );
    let (x0, x1) = (
        mean(&neg0) / n.c_max_mol_per_m3,
        mean(&neg1) / n.c_max_mol_per_m3,
    );
    // Coulombs of the engine's charge per unit of negative stoichiometry.
    let per_stoich = chem.cell.capacity_ah * 3600.0 / (n.stoich_max - n.stoich_min);
    let stored = per_stoich * integral(&n.ocp.stoich, &n.ocp.volts, x1, x0);
    assert!(
        stored > 0.0,
        "the film took lithium and the ledger saw none"
    );
    assert_eq!(electrical, 0.0, "a resting cell moved charge");
    assert_eq!(heat, 0.0, "a resting cell made heat");
    // Measured: 48.58 J either way, agreeing to 2.3e-9 — what is left is the film's
    // booking at each tick's end-of-tick potential against the exact integral.
    assert!(
        (booked / stored - 1.0).abs() < 1e-6,
        "the film booked {booked} J, the electrodes' stored energy rose by {stored} J"
    );
}

/// A filmed cell's state of charge is a fraction of the capacity it **reports**, so the two
/// together count the charge it delivers. After a hot year stored at 85 % the film has
/// shortened the negative's window by 2.8 %, and a slow discharge moves the reported state of
/// charge times the reported capacity by the amp-hours it delivers, to 1e-4. Read against the
/// fresh window instead — measured by putting that reading back — they counted 1.0013 Ah of
/// the 1.0306 Ah delivered, 2.8 % short.
#[test]
fn state_of_charge_counts_against_the_capacity_the_cell_has() {
    let mut pack = filmed(lgm50(), 0.85, 318.15);
    for _ in 0..365 * 24 {
        pack.step(3600.0, Demand::Rest, &env(318.15));
    }
    let before = pack.step(0.0, Demand::Rest, &env(318.15));
    let cap_ah = lgm50().cell.capacity_ah;
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

/// A fresh film is the chemistry's initial film, whose resistance and lithium the parameter
/// set already counts, so until the first aging tick a filmed cell is bit for bit the cell
/// with the drawn curve — whose own first tick is also still to come. After it, they part.
#[test]
fn a_fresh_film_moves_nothing_until_it_grows() {
    let curve = config(0.6, 298.15, CalendarModel::Curve, SPM);
    let film = config(0.6, 298.15, CalendarModel::Film, SPM);
    let mut a = Pack::new(&curve, lgm50()).expect("builds");
    let mut b = Pack::new(&film, lgm50()).expect("builds");
    let demands = [Demand::Current(3.0), Demand::Rest, Demand::Current(-2.0)];
    // Nine one-second steps: the 10 s sub-clock has not ticked.
    for k in 0..9 {
        let ta = a.step(1.0, demands[k % 3], &env(298.15));
        let tb = b.step(1.0, demands[k % 3], &env(298.15));
        assert_eq!(
            serde_json::to_string(&ta).expect("telemetry"),
            serde_json::to_string(&tb).expect("telemetry"),
            "step {k}: a fresh film moved the cell"
        );
        assert_eq!(
            serde_json::to_string(&a.cell(0, 0)).expect("view"),
            serde_json::to_string(&b.cell(0, 0)).expect("view"),
        );
    }
    let ta = a.step(1.0, Demand::Rest, &env(298.15));
    let tb = b.step(1.0, Demand::Rest, &env(298.15));
    assert_ne!(
        ta.soh_capacity, tb.soh_capacity,
        "after the first tick the curve and the film are different mechanisms"
    );
}

/// A filmed pack snapshot at t/2, restored through a real serialization, continues
/// bit-identically — the film's thickness is state, and its lithium is in the profile.
#[test]
fn a_filmed_pack_survives_a_snapshot() {
    let schedule = |k: usize| match k % 4 {
        0 => Demand::Current(4.0),
        1 => Demand::Rest,
        2 => Demand::Current(-3.0),
        _ => Demand::Voltage(3.9),
    };
    let mut straight = filmed(lgm50(), 0.55, 308.15);
    let reference: Vec<String> = (0..400)
        .map(|k| serde_json::to_string(&straight.step(60.0, schedule(k), &env(308.15))).expect("t"))
        .collect();
    let mut split = filmed(lgm50(), 0.55, 308.15);
    for k in 0..200 {
        split.step(60.0, schedule(k), &env(308.15));
    }
    let text = serde_json::to_string(&split.snapshot()).expect("serializes");
    let snapshot: Snapshot = serde_json::from_str(&text).expect("deserializes");
    let mut continued = Pack::restore(&snapshot).expect("restores");
    for (k, expected) in reference.iter().enumerate().skip(200) {
        let got =
            serde_json::to_string(&continued.step(60.0, schedule(k), &env(308.15))).expect("t");
        assert_eq!(&got, expected, "step {k} after a restore differs");
    }
    assert_eq!(
        continued.cell_film(0, 0).map(|f| f.thickness_m),
        straight.cell_film(0, 0).map(|f| f.thickness_m)
    );
}

/// `Pack::new` refuses the film where it cannot grow: every model but the `Spm`, and a
/// chemistry with no `[spm.sei]`. Each refusal is its own error, so a client learns which.
#[test]
fn the_film_is_refused_where_it_cannot_grow() {
    let film = |model| config(0.5, 298.15, CalendarModel::Film, model);
    for (model, name) in [
        (CellModelConfig::Ecm, "Ecm"),
        (
            CellModelConfig::Dfn {
                shells: 6,
                nodes_negative: 5,
                nodes_separator: 3,
                nodes_positive: 5,
            },
            "Dfn",
        ),
    ] {
        assert_eq!(
            Pack::new(&film(model), lgm50()).err(),
            Some(BuildError::FilmNeedsSpm { model: name })
        );
    }
    let prada = parse_chemistry(PRADA).expect("parses");
    assert_eq!(
        Pack::new(
            &film(CellModelConfig::SpmEnsemble {
                shells: 8,
                particles: 2,
                radius_sigma: 0.0
            }),
            prada.clone()
        )
        .err(),
        Some(BuildError::FilmNeedsSpm {
            model: "SpmEnsemble"
        })
    );
    // An `[spm]` chemistry with no `[spm.sei]`.
    let mut bare = lgm50();
    bare.spm.as_mut().expect("[spm]").sei = None;
    assert_eq!(
        Pack::new(&film(SPM), bare).err(),
        Some(BuildError::MissingSeiParams {
            chem_id: "nmc_21700_lgm50".into()
        })
    );
    // No `[spm]` at all: the model's own refusal answers first.
    let lfp = parse_chemistry(LFP).expect("parses");
    assert!(matches!(
        Pack::new(&film(SPM), lfp).err(),
        Some(BuildError::MissingSpmParams { .. })
    ));
    // The drawn curve is still accepted on every model, as before.
    assert!(Pack::new(
        &config(0.5, 298.15, CalendarModel::Curve, CellModelConfig::Ecm),
        lgm50()
    )
    .is_ok());
}

/// `[spm.sei]` refuses an unknown key, a missing one and a value out of range, by name.
#[test]
fn the_loader_refuses_a_bad_sei_section() {
    let with = |from: &str, to: &str| {
        assert_eq!(
            LGM50.matches(from).count(),
            1,
            "the edit's anchor is unique"
        );
        parse_chemistry(&LGM50.replacen(from, to, 1))
    };
    let typo = with("li_per_sei ", "li_per_sie ");
    assert!(typo.is_err(), "a misspelt [spm.sei] key was accepted");
    let negative = with(
        "resistivity_ohm_m                = 5.594e4",
        "resistivity_ohm_m                = -5.594e4",
    );
    assert!(
        format!("{:?}", negative.err()).contains("spm.sei.resistivity_ohm_m"),
        "a negative resistivity was not refused by name"
    );
    let zero = with(
        "initial_thickness_m              = 5.0e-9",
        "initial_thickness_m              = 0.0",
    );
    assert!(
        format!("{:?}", zero.err()).contains("spm.sei.initial_thickness_m"),
        "a zero initial film, which the growth rate divides by, was not refused"
    );
    let extra = with("li_per_sei ", "spare = 1.0\nli_per_sei ");
    assert!(extra.is_err(), "an unknown [spm.sei] key was accepted");
}

/// The selector crosses JSON as a bare string and defaults to the drawn curve, so every
/// setup written before Phase 10 still parses and still means what it meant.
#[test]
fn the_calendar_selector_defaults_to_the_curve() {
    let old: AgingConfig =
        serde_json::from_str(r#"{"sub_clock_period_s": 10.0}"#).expect("a pre-Phase-10 setup");
    assert_eq!(old.calendar, CalendarModel::Curve);
    let new: AgingConfig =
        serde_json::from_str(r#"{"sub_clock_period_s": 10.0, "calendar": "Film"}"#)
            .expect("parses");
    assert_eq!(new.calendar, CalendarModel::Film);
    let v: Value = serde_json::to_value(AgingConfig::default()).expect("serializes");
    assert_eq!(v["calendar"], "Curve");
}
