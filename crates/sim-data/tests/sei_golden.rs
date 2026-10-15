//! Phase 10 slice C: the SEI film on the `Spm` against PyBaMM, exits 1–4 of
//! `docs/plans/phase-10-sei.md`.
//!
//! The reference is `tests/golden/nmc_21700_lgm50/sei_rest_year.csv`, written by
//! `tools/reference/sei_golden.py`: PyBaMM's SPM with the interstitial-diffusion limited SEI
//! law, the chemistry's own `[spm.sei]` constants, a year at rest, isothermal, at five
//! conditions, each started at the engine's own stoichiometry. Every condition is run twice
//! there, and the two columns are what the tolerances below are made of:
//!
//! * **`table`** runs PyBaMM on the engine's piecewise-linear graphite table. The engine and
//!   that reference integrate the same law on the same potential, so what separates them is
//!   the integrator — the potential frozen over each tick, PyBaMM's adaptive steps — and the
//!   tolerance against it is tight. PyBaMM is solved at 1e-10 for it; at its default 1e-6 it
//!   was itself 0.8 % off in the first days.
//! * **`function`** runs PyBaMM on Chen2020's own graphite curve, which the table
//!   approximates to 1.90 mV. The rate goes as `exp(−F·U_n/RT)`, so that error is measured in
//!   the CSV itself: after a year 0.06 % of the lithium at 85 %, 2.0 % at 30 % and 25 °C,
//!   1.4 % at 30 % and 45 °C, none on the flat top at 100 % — and on its worst day from day 7,
//!   0.08 / 0.06 / 2.87 / 2.55 / 0 %. The tolerance against it is that worst day, not a
//!   number chosen to pass.
//!
//! Compared on the lithium the film took — PyBaMM's "Loss of lithium to negative SEI", less
//! its day-0 value — read two ways on the engine's side: [`sim_core::FilmView`]'s own count,
//! and the capacity the cell reports, converted back to moles. The second is the readout a
//! client sees, so it is held to the reference too, not only to the first.

use sim_core::{
    AgingConfig, CalendarModel, CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig,
    Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const GOLDEN: &str = include_str!("../../../tests/golden/nmc_21700_lgm50/sei_rest_year.csv");

/// One condition's reference: `(day, lithium lost [mol] on the function, on the table)`.
struct Reference {
    soc: f64,
    temp_k: f64,
    rows: Vec<(f64, f64, f64)>,
}

fn references() -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    for line in GOLDEN.lines() {
        if line.starts_with('#') || line.starts_with("soc") || line.is_empty() {
            continue;
        }
        let v: Vec<f64> = line
            .split(',')
            .map(|x| x.parse().expect("a number"))
            .collect();
        let (soc, temp_k) = (v[0], v[1]);
        match out.last_mut() {
            Some(r) if r.soc == soc && r.temp_k == temp_k => r.rows.push((v[2], v[3], v[4])),
            _ => out.push(Reference {
                soc,
                temp_k,
                rows: vec![(v[2], v[3], v[4])],
            }),
        }
    }
    out
}

fn chem() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

/// A single filmed `Spm` cell at `soc`, pinned at `temp_k`.
fn filmed(chem: ChemistryParams, soc: f64, temp_k: f64) -> Pack {
    let config = PackConfig {
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
        cell_model: CellModelConfig::Spm {
            shells: sim_core::spm::DEFAULT_SHELLS,
        },
    };
    Pack::new(&config, chem).expect("a filmed Spm pack builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

/// The lithium \[mol\] a cell's reported capacity says it has lost: `(1 − soh)` of the
/// negative's window, in moles. The readout a client sees, turned back into the quantity
/// the reference counts.
fn lithium_from_capacity(chem: &ChemistryParams, soh: f64) -> f64 {
    let spm = chem.spm.as_ref().expect("[spm]");
    let n = &spm.negative;
    let volume = n.active_volume_fraction * spm.electrode_area_m2 * n.thickness_m;
    (1.0 - soh) * (n.stoich_max - n.stoich_min) * n.c_max_mol_per_m3 * volume
}

/// Each condition's engine trajectory, one value per reference day: `(film count, capacity
/// readout)` of the lithium lost \[mol\]. One-hour steps, so one aging tick an hour.
fn engine_year(r: &Reference) -> Vec<(f64, f64)> {
    let chem = chem();
    let mut pack = filmed(chem.clone(), r.soc, r.temp_k);
    let mut out = Vec::with_capacity(r.rows.len());
    let mut hours = 0_usize;
    for &(day, _, _) in &r.rows {
        // The reference reports whole days.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let target = (day * 24.0).round() as usize;
        while hours < target {
            pack.step(3600.0, Demand::Rest, &env(r.temp_k));
            hours += 1;
        }
        let film = pack.cell_film(0, 0).expect("the cell has a film");
        let soh = pack.cell(0, 0).expect("the cell exists").soh_capacity;
        out.push((film.lithium_lost_mol, lithium_from_capacity(&chem, soh)));
    }
    out
}

/// The worst relative disagreement over the year, from day 7 on (before that the growth is
/// small enough that a relative error measures rounding).
fn worst(
    engine: &[(f64, f64)],
    r: &Reference,
    pick: impl Fn(&(f64, f64, f64)) -> f64,
) -> (f64, f64) {
    let (mut film, mut cap) = (0.0_f64, 0.0_f64);
    for (e, row) in engine.iter().zip(&r.rows) {
        if row.0 < 7.0 {
            continue;
        }
        let reference = pick(row);
        film = film.max((e.0 / reference - 1.0).abs());
        cap = cap.max((e.1 / reference - 1.0).abs());
    }
    (film, cap)
}

/// What the table costs on its worst day, from day 7: `|table/function − 1|`, from the CSV.
fn table_floor(r: &Reference) -> f64 {
    r.rows
        .iter()
        .filter(|row| row.0 >= 7.0)
        .map(|row| (row.2 / row.1 - 1.0).abs())
        .fold(0.0, f64::max)
}

/// Exit 1: a year at rest matches PyBaMM at two SOCs and two temperatures (and the full
/// cell exit 3 needs), held tightly to the table reference and, against the function one,
/// to the table's own measured cost.
///
/// # The two tolerances, measured
/// Against the **table** reference the worst disagreement over the year, from day 7, is
/// 0.00002 / 0.00001 / 0.0004 / 0.0016 % at 85 % 25 °C / 85 % 45 °C / 30 % 25 °C / 30 % 45 °C,
/// the same law integrated two ways. Held to 0.01 %. Against a reference solved at PyBaMM's
/// default tolerance it had read 0.47 % at 30 % and 45 °C, which was PyBaMM's error, not the
/// engine's (see `tools/reference/sei_golden.py`).
///
/// Against the **function** reference it is whatever the table costs: 0.08 / 0.06 / 2.87 /
/// 2.55 %. Held to the table's own worst day in the CSV plus 0.01 %, so the bound is the
/// table's error measured, not a number chosen to pass.
#[test]
fn a_year_at_rest_matches_pybamm() {
    for r in references() {
        let engine = engine_year(&r);
        let (film_t, cap_t) = worst(&engine, &r, |row| row.2);
        let (film_f, cap_f) = worst(&engine, &r, |row| row.1);
        let floor = table_floor(&r);
        let what = format!("SOC {:.2} at {:.2} K", r.soc, r.temp_k);
        assert!(
            film_t < 1e-4 && cap_t < 1e-4,
            "{what}: the film is {:.5} % and the reported capacity {:.5} % from PyBaMM run on \
             the engine's own graphite table; measured at 0.0016 % at worst",
            film_t * 100.0,
            cap_t * 100.0
        );
        assert!(
            film_f < floor + 1e-4 && cap_f < floor + 1e-4,
            "{what}: the film is {:.4} % and the reported capacity {:.4} % from PyBaMM on \
             Chen2020's graphite curve, past the table's own worst day, {:.4} %",
            film_f * 100.0,
            cap_f * 100.0,
            floor * 100.0
        );
    }
}

/// The year at 45 °C and 85 %: film thickness and lithium lost at each reference day.
fn hot_year() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let r = references()
        .into_iter()
        .find(|r| r.soc == 0.85 && r.temp_k == 318.15)
        .expect("the 85 % / 45 C condition");
    let mut pack = filmed(chem(), r.soc, r.temp_k);
    let (mut days, mut l, mut li) = (Vec::new(), Vec::new(), Vec::new());
    let mut hours = 0_usize;
    for &(day, _, _) in &r.rows {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let target = (day * 24.0).round() as usize;
        while hours < target {
            pack.step(3600.0, Demand::Rest, &env(r.temp_k));
            hours += 1;
        }
        let film = pack.cell_film(0, 0).expect("film");
        days.push(day);
        l.push(film.thickness_m);
        li.push(film.lithium_lost_mol);
    }
    (days, l, li)
}

/// Exit 2: once the film is several times its starting thickness, the lithium it takes goes
/// as `√t` — a log-log slope near 0.5 — though nothing in the rate law is a `√t`. The law is
/// `L·dL/dt = k·e`, so `L² − L0²` grows linearly and `L` like `√t` once `L ≫ L0`.
///
/// # The regime, named
/// Stored at 45 °C and 85 % the film is 5 nm new, 4.9 times that by day 30 and 16.6 times by
/// a year (measured: 24.3 and 83.0 nm). Over days 30 to 365 the slope of the lithium lost is
/// 0.559, above 0.5 by what the `− L0` still costs at day 30 and by the slow rise of the
/// graphite potential as the negative loses lithium. The band is 0.5 to 0.65; a law linear in
/// time would read 1, and a curve that saturated would read well under 0.5.
#[test]
fn fade_goes_as_root_time_once_the_film_has_grown() {
    let (days, l, li) = hot_year();
    let at = |d: f64| days.iter().position(|&x| x == d).expect("a reference day");
    let (a, b) = (at(30.0), at(365.0));
    let l0 = chem()
        .spm
        .expect("spm")
        .sei
        .expect("sei")
        .initial_thickness_m;
    assert!(
        l[a] > 3.0 * l0,
        "the regime this exit names is a film several times its start by day 30; it was \
         {:.1} nm against {:.1} nm",
        l[a] * 1e9,
        l0 * 1e9
    );
    let slope = (li[b] / li[a]).ln() / (days[b] / days[a]).ln();
    assert!(
        (0.5..0.65).contains(&slope),
        "log-log slope of lithium lost over days 30 to 365: {slope:.4}"
    );
}

/// Exit 3: stored full, the cell loses measurably more than stored at 30 %, by a ratio pinned
/// at the fitted constants — the law's whole claim, that the film grows faster where the
/// graphite potential is lower.
///
/// The ratio after a year at 25 °C, PyBaMM on the engine's table, is 6.6546 (on Chen2020's own
/// curve 6.520, the difference being the table's 2 % at 30 %); the engine's is 6.6546 too, to
/// 3e-6. Held within 1e-4 of the table reference's ratio.
#[test]
fn stored_full_loses_more_than_stored_at_thirty_percent() {
    let refs = references();
    let year = |soc: f64| {
        let r = refs
            .iter()
            .find(|r| r.soc == soc && r.temp_k == 298.15)
            .expect("a 25 C condition");
        let engine = engine_year(r);
        (
            engine.last().expect("rows").0,
            r.rows.last().expect("rows").2,
        )
    };
    let (full, full_ref) = year(1.0);
    let (low, low_ref) = year(0.3);
    let (ratio, ratio_ref) = (full / low, full_ref / low_ref);
    assert!(
        ratio > 5.0,
        "stored full must cost measurably more than stored at 30 %: ratio {ratio:.3}"
    );
    assert!(
        (ratio / ratio_ref - 1.0).abs() < 1e-4,
        "full-to-30 % ratio after a year: engine {ratio:.4}, PyBaMM on the table {ratio_ref:.4}"
    );
}
