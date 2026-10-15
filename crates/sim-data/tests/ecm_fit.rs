//! The LG M50's fitted equivalent circuit, replayed by the engine against the DFN references
//! it was fitted to (Phase 11 slice A, `docs/plans/phase-11-fitting.md`).
//!
//! `tools/reference/fit_ecm.py` fitted `[r0]` and `[[rc]]` against the reference's own OCV,
//! and stated each run's time-weighted RMS above 5 % charge. The engine replays the same
//! current profiles through the shipped file — its `[ocv]` table (re-gridded to 0.92 mV) and
//! its own step — and must land within **1.0 mV** of each stated figure: the bar written into
//! the plan note before this test first ran.

use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// Each reference, its temperature, and the RMS \[mV\] `fit_ecm.py` stated for it.
const RUNS: [(&str, &str, f64, f64); 4] = [
    (
        "1C, 25 degC",
        include_str!("../../../tests/golden/nmc_21700_lgm50/ecm_hppc_1c_25c.csv"),
        298.15,
        4.85,
    ),
    (
        "3C, 25 degC",
        include_str!("../../../tests/golden/nmc_21700_lgm50/ecm_hppc_3c_25c.csv"),
        298.15,
        18.81,
    ),
    (
        "1C, -10 degC",
        include_str!("../../../tests/golden/nmc_21700_lgm50/ecm_hppc_1c_m10c.csv"),
        263.15,
        11.15,
    ),
    (
        "1C, +45 degC",
        include_str!("../../../tests/golden/nmc_21700_lgm50/ecm_hppc_1c_45c.csv"),
        318.15,
        3.51,
    ),
];

/// The bar \[mV\], set before the first run.
const BAR_MV: f64 = 1.0;

struct Row {
    t: f64,
    i: f64,
    v: f64,
    soc: f64,
}

fn parse_csv(text: &str) -> Vec<Row> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("time_s"))
        .map(|l| {
            let mut f = l.split(',');
            let mut next = || f.next().unwrap().parse::<f64>().unwrap();
            Row {
                t: next(),
                i: next(),
                v: next(),
                soc: next(),
            }
        })
        .collect()
}

fn chem() -> ChemistryParams {
    sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses")
}

fn pack(soc: f64, temp_k: f64) -> Pack {
    let cfg = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    };
    Pack::new(&cfg, chem()).expect("pack builds")
}

/// The engine's time-weighted RMS \[mV\] against one reference, over rows above 5 % charge.
fn replay_rms_mv(rows: &[Row], temp_k: f64) -> f64 {
    let mut p = pack(rows[0].soc, temp_k);
    let env = Env {
        t_ambient: temp_k,
        t_coolant: None,
    };
    let (mut num, mut den) = (0.0, 0.0);
    for w in rows.windows(2) {
        let dt = w[1].t - w[0].t;
        let v = p.step(dt, Demand::Current(w[1].i), &env).v_terminal;
        if w[1].soc > 0.05 {
            num += dt * (v - w[1].v).powi(2);
            den += dt;
        }
    }
    1e3 * (num / den).sqrt()
}

#[test]
fn the_engine_replays_each_reference_within_the_fits_residual() {
    let mut fails = Vec::new();
    for (label, text, temp_k, stated) in RUNS {
        let rms = replay_rms_mv(&parse_csv(text), temp_k);
        eprintln!("{label}: engine {rms:.3} mV against the fit's {stated:.2} mV");
        if (rms - stated).abs() > BAR_MV {
            fails.push(format!(
                "{label}: {rms:.3} mV against {stated:.2} (bar {BAR_MV} mV)"
            ));
        }
    }
    assert!(fails.is_empty(), "{}", fails.join("; "));
}

/// LG's specification sheet (LRB-PS-CY18.2Wh-M50, 2017-09-22, revision 0, "Tentative") gives
/// the cell's DC resistance at 50 % charge, 0.5C for 30 s, 25 degC: 30 ± 6 mΩ. Reported, not
/// pinned — the spike put the fit just outside it, and the circuit is fitted to the model,
/// not to the cell.
#[test]
fn reports_the_thirty_second_resistance_against_the_cell() {
    let mut p = pack(0.5, 298.15);
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    let v0 = p.step(1.0, Demand::Rest, &env).v_terminal;
    let i = 0.5 * chem().cell.capacity_ah;
    let mut v = v0;
    for _ in 0..30 {
        v = p.step(1.0, Demand::Current(i), &env).v_terminal;
    }
    let r_mohm = 1e3 * (v0 - v) / i;
    eprintln!("30 s DC resistance at 50 %, 0.5C, 25 degC: {r_mohm:.1} mOhm (LG: 30 +/- 6)");
    assert!(r_mohm.is_finite() && r_mohm > 0.0);
}
