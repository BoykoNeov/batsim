//! Phase 12 slice A: the plating law integrated **open loop** beside the shipped `Dfn` — the
//! engine's instrument for sizing the rate constant. Design and results in
//! `docs/plans/phase-12-slice-a-reference.md`; the closed-loop reference beside it is
//! `tools/reference/plating_reference.py`.
//!
//! Nothing here is engine code, and the potential the law reads is the plating-free model's: the
//! law cannot feed back. That is what makes every figure linear in the rate constant `k`, so one
//! run per charge sizes it.

use std::fmt::Write as _;

use sim_core::aging::GAS_CONSTANT_J_PER_MOL_K as R;
use sim_core::dfn::UNKNOWNS_PER_NODE;
use sim_core::spm::FARADAY_C_PER_MOL as F;
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig, Scatter,
    ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// OKane2022's plating transfer coefficient, the plan's `α`.
const ALPHA: f64 = 0.65;
/// The dead share's placeholder (the plan's slice A, step 4); it splits gross, nothing else.
const DEAD_SHARE: f64 = 0.5;
/// The charger's voltage [V], and its taper as a fraction of the 1C current.
const V_HOLD: f64 = 4.2;
const TAPER_C: f64 = 0.05;
/// A hold that has not tapered in this long is reported as such, not run forever [s].
const HOLD_LIMIT_S: f64 = 12.0 * 3600.0;

fn chem() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

fn grid(fine: bool) -> CellModelConfig {
    use sim_core::dfn::*;
    if fine {
        CellModelConfig::Dfn {
            shells: FINE_SHELLS,
            nodes_negative: FINE_NODES_NEGATIVE,
            nodes_separator: FINE_NODES_SEPARATOR,
            nodes_positive: FINE_NODES_POSITIVE,
        }
    } else {
        CellModelConfig::Dfn {
            shells: sim_core::spm::DEFAULT_SHELLS,
            nodes_negative: DEFAULT_NODES_NEGATIVE,
            nodes_separator: DEFAULT_NODES_SEPARATOR,
            nodes_positive: DEFAULT_NODES_POSITIVE,
        }
    }
}

fn dfn_state(p: &Pack) -> sim_core::DfnState {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    serde_json::from_value(v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"].clone())
        .expect("cell 0 is a Dfn cell")
}

/// Plated lithium per unit `k`, per negative node [mol per (m/s)]: gross is everything the
/// plating branch put down, `rev` what can strip back, `dead` what cannot.
#[derive(Clone, Default)]
struct Ledger {
    gross: Vec<f64>,
    rev: Vec<f64>,
    dead: Vec<f64>,
}

impl Ledger {
    fn sum(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() * F / 3600.0
    }
}

/// One charge's figures, every one per unit `k` [A·h per (m/s), or A per (m/s)].
struct Figures {
    cc_gross: f64,
    cc_net: f64,
    cc_end_s: f64,
    cc_end_soc: f64,
    cccv_gross: f64,
    cccv_net: f64,
    cccv_end_s: f64,
    cccv_end_soc: f64,
    /// Largest plating current over the cell current, per unit `k`, on each leg [1/(m/s)].
    cc_share_max: f64,
    cv_share_max: f64,
    /// The same at the hold's last step.
    cv_share_end: f64,
    tapered: bool,
    unconverged: u32,
    min_eta_mv: f64,
}

/// A charge from empty at `c_rate`, pinned at `temp_c`: constant current to [`V_HOLD`], then
/// the voltage held until the current falls to [`TAPER_C`]. 1 s steps throughout, as A1b's.
fn charge(fine: bool, temp_c: f64, c_rate: f64) -> Figures {
    let chem = chem();
    let spm = chem.spm.clone().expect("[spm]");
    let q = chem.cell.capacity_ah;
    let temp_k = 273.15 + temp_c;
    let config = PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: 0.0,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: grid(fine),
    };
    let mut pack = Pack::new(&config, chem).expect("builds");
    let env = Env {
        t_ambient: temp_k,
        t_coolant: None,
    };
    let neg = &spm.negative;
    let area = 3.0 * neg.active_volume_fraction * spm.electrode_area_m2 * neg.thickness_m
        / neg.particle_radius_m;
    let f = F / (R * temp_k);
    let mut led = Ledger::default();
    let (mut t, mut held) = (0.0, false);
    let mut out = Figures {
        cc_gross: 0.0,
        cc_net: 0.0,
        cc_end_s: 0.0,
        cc_end_soc: 0.0,
        cccv_gross: 0.0,
        cccv_net: 0.0,
        cccv_end_s: 0.0,
        cccv_end_soc: 0.0,
        cc_share_max: 0.0,
        cv_share_max: 0.0,
        cv_share_end: 0.0,
        tapered: false,
        unconverged: 0,
        min_eta_mv: f64::INFINITY,
    };
    let mut t_hold0 = 0.0;
    let soc = loop {
        t += 1.0;
        let demand = if held {
            Demand::Voltage(V_HOLD)
        } else {
            Demand::Current(-c_rate * q)
        };
        let tel = pack.step(1.0, demand, &env);
        let soc = tel.soc_true;
        out.unconverged += tel.flags.contains(EventFlags::SOLVE_UNCONVERGED) as u32;
        let st = dfn_state(&pack);
        let nn = st.c_neg.len();
        if led.gross.is_empty() {
            led = Ledger {
                gross: vec![0.0; nn],
                rev: vec![0.0; nn],
                dead: vec![0.0; nn],
            };
        }
        let node_area = area / nn as f64;
        let mut i_pl = 0.0;
        for k in 0..nn {
            let w = UNKNOWNS_PER_NODE;
            let eta = st.u[w * k + 2] - st.u[w * k + 1];
            out.min_eta_mv = out.min_eta_mv.min(1e3 * eta);
            // [mol/s per (m/s)], positive plates. The electrolyte through the solver's own
            // lookup floor: read raw it goes slightly negative at the graphite on a 2C charge, and
            // the law's sign flips with it — a node above 0 V then counts as plating. Found and
            // measured in slice B (`phase-12-slice-b-reaction.md`); before it, the two 2C rows of
            // slice A's table were inflated by it.
            let r = st.c_e[k].max(sim_core::dfn::C_E_FLOOR_MOL_PER_M3)
                * ((-ALPHA * f * eta).exp() - ((1.0 - ALPHA) * f * eta).exp())
                * node_area;
            if r > 0.0 {
                led.gross[k] += r;
                led.rev[k] += (1.0 - DEAD_SHARE) * r;
                led.dead[k] += DEAD_SHARE * r;
                i_pl += r * F;
            } else {
                let strip = (-r).min(led.rev[k]);
                led.rev[k] -= strip;
                i_pl -= strip * F;
            }
        }
        let share = i_pl.abs() / tel.i_actual.abs().max(1e-12);
        if held {
            out.cv_share_max = out.cv_share_max.max(share);
            out.cv_share_end = share;
        } else {
            out.cc_share_max = out.cc_share_max.max(share);
        }
        if !held && tel.v_terminal >= V_HOLD {
            held = true;
            t_hold0 = t;
            out.cc_gross = Ledger::sum(&led.gross);
            out.cc_net = Ledger::sum(&led.rev) + Ledger::sum(&led.dead);
            out.cc_end_s = t;
            out.cc_end_soc = soc;
        } else if held && -tel.i_actual <= TAPER_C * q {
            out.tapered = true;
            break soc;
        } else if held && t - t_hold0 >= HOLD_LIMIT_S {
            break soc;
        }
        assert!(
            t < 30.0 * 3600.0,
            "{temp_c} C {c_rate}C never reached {V_HOLD} V"
        );
    };
    out.cccv_gross = Ledger::sum(&led.gross);
    out.cccv_net = Ledger::sum(&led.rev) + Ledger::sum(&led.dead);
    out.cccv_end_s = t;
    out.cccv_end_soc = soc;
    out
}

/// The sheet's conditions, A1b's list: `(temp_c, c_rate, allowed)`.
const CONDITIONS: [(f64, f64, bool); 9] = [
    (25.0, 0.3, true),
    (45.0, 0.7, true),
    (0.0, 0.3, true),
    (25.0, 0.7, true),
    (-10.0, 0.3, false),
    (0.0, 1.0, false),
    (0.0, 2.0, false),
    (-10.0, 1.0, false),
    (-10.0, 2.0, false),
];

fn report(fine: bool) {
    let rows: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = CONDITIONS
            .iter()
            .map(|&(tc, c, ok)| s.spawn(move || (tc, c, ok, charge(fine, tc, c))))
            .collect();
        hs.into_iter().map(|h| h.join().expect("a run")).collect()
    });
    let mut txt = String::new();
    writeln!(
        txt,
        "grid {}: per unit k, A·h per (m/s); share per unit k, 1/(m/s)",
        if fine { "fine" } else { "recommended" }
    )
    .unwrap();
    writeln!(txt, "T_C,C,sheet,cc_gross,cc_net,cc_end_s,cc_end_soc,cccv_gross,cccv_net,cccv_end_s,cccv_end_soc,tapered,cc_share_max,cv_share_max,cv_share_end,unconverged,min_eta_mv").unwrap();
    for (tc, c, ok, r) in &rows {
        writeln!(
            txt,
            "{tc},{c},{},{:.6e},{:.6e},{},{:.4},{:.6e},{:.6e},{},{:.4},{},{:.4e},{:.4e},{:.4e},{},{:.2}",
            if *ok { "allows" } else { "forbids" },
            r.cc_gross,
            r.cc_net,
            r.cc_end_s,
            r.cc_end_soc,
            r.cccv_gross,
            r.cccv_net,
            r.cccv_end_s,
            r.cccv_end_soc,
            r.tapered,
            r.cc_share_max,
            r.cv_share_max,
            r.cv_share_end,
            r.unconverged,
            r.min_eta_mv
        )
        .unwrap();
    }
    println!("{txt}");
}

/// Run with `--ignored --nocapture`; the table it prints is recorded in the slice note.
#[test]
#[ignore = "report: the plating law open loop on the fine grid; recorded in phase-12-slice-a-reference.md"]
fn plating_open_loop_fine_grid() {
    report(true);
}

/// The same on the recommended grid, recorded beside the fine one, never sized on.
#[test]
#[ignore = "report: the plating law open loop on the recommended grid; recorded in phase-12-slice-a-reference.md"]
fn plating_open_loop_recommended_grid() {
    report(false);
}
