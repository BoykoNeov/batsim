//! A cold 4.2 V hold, and the discharge after it, on the `Dfn` (found by Phase 12 slice C,
//! `docs/plans/phase-12-slice-c-health.md`, "Found on the way").

use sim_core::dfn::{DEFAULT_NODES_NEGATIVE, DEFAULT_NODES_POSITIVE, DEFAULT_NODES_SEPARATOR};
use sim_core::spm::{DEFAULT_SHELLS, FARADAY_C_PER_MOL as F};
use sim_core::{
    AgingConfig, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig,
    PlatingModel, Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

fn chem() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

fn recommended() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: DEFAULT_SHELLS,
        nodes_negative: DEFAULT_NODES_NEGATIVE,
        nodes_separator: DEFAULT_NODES_SEPARATOR,
        nodes_positive: DEFAULT_NODES_POSITIVE,
    }
}

fn pack(temp_k: f64, aging: Option<AgingConfig>) -> Pack {
    Pack::new(
        &PackConfig {
            series: 1,
            parallel: 1,
            initial_soc: 0.0,
            initial_temp_k: temp_k,
            seed: 0,
            scatter: Scatter::default(),
            thermal: ThermalConfig::Isothermal,
            bms: None,
            aging,
            cell_model: recommended(),
        },
        chem(),
    )
    .expect("builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

fn dfn_state(p: &Pack) -> sim_core::DfnState {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    serde_json::from_value(v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"].clone())
        .expect("cell 0 is a Dfn cell")
}

/// Outer shell over `c_max`, per node: `(min, max, node of max)` for each electrode, and the
/// electrolyte's minimum.
fn edges(s: &sim_core::DfnState, c: &ChemistryParams) -> String {
    let spm = c.spm.as_ref().unwrap();
    let outer = |prof: &[Vec<f64>], cmax: f64| -> (f64, f64, usize) {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut at = 0;
        for (i, p) in prof.iter().enumerate() {
            let x = p[p.len() - 1] / cmax;
            lo = lo.min(x);
            if x > hi {
                hi = x;
                at = i;
            }
        }
        (lo, hi, at)
    };
    let n = outer(&s.c_neg, spm.negative.c_max_mol_per_m3);
    let p = outer(&s.c_pos, spm.positive.c_max_mol_per_m3);
    let ce = s.c_e.iter().copied().fold(f64::INFINITY, f64::min);
    // The negative nodes' real surfaces, from each node's committed reaction current — the
    // expression the solve used — not the outer shell.
    let e = &spm.negative;
    let d_s = e.diffusivity_m2_per_s
        * ((e.diffusivity_ea_j_per_mol / 8.314_462_618) * (1.0 / spm.t_ref_k - 1.0 / s.temp_k)).exp();
    let kappa = sim_core::dfn::probe::spm_capacity(spm) / c.cell.capacity_ah;
    let nv = sim_core::dfn::UNKNOWNS_PER_NODE;
    let surf: Vec<f64> = s
        .c_neg
        .iter()
        .enumerate()
        .map(|(i, prof)| {
            let j = s.u[nv * i + 3];
            sim_core::spm::c_surface(prof, e.particle_radius_m, d_s, j * kappa / F) / e.c_max_mol_per_m3
        })
        .collect();
    let (smax, sat) = surf
        .iter()
        .enumerate()
        .fold((f64::NEG_INFINITY, 0), |(m, a), (i, &x)| if x > m { (x, i) } else { (m, a) });
    let j_hot = s.u[nv * sat + 3];
    format!(
        "neg outer {:.5}..{:.5} (max at node {}), surface max {:.9} at node {sat} (j {j_hot:.4e} A/m2), pos outer {:.5}..{:.5} (max at {}), c_e min {ce:.2}",
        n.0, n.1, n.2, smax, p.0, p.1, p.2
    )
}

/// Diagnostic: the hold at 0 °C and 25 °C, and the discharge after it.
#[test]
#[ignore = "diagnostic: the cold hold"]
fn report_the_cold_hold() {
    let c = chem();
    let q = c.cell.capacity_ah;
    for temp_c in [0.0, 25.0] {
        let t = 273.15 + temp_c;
        let mut p = pack(t, None);
        let mut held = false;
        let mut k = 0;
        loop {
            let d = if held {
                Demand::Voltage(4.2)
            } else {
                Demand::Current(-q)
            };
            let tel = p.step(1.0, d, &env(t));
            k += 1;
            if !held && tel.v_terminal >= 4.2 {
                held = true;
                println!("{temp_c} C: CC ends at {k} s; {}", edges(&dfn_state(&p), &c));
            }
            if held && k % 600 == 0 {
                println!(
                    "{temp_c} C hold {k} s: {:.4} A {:?}; {}",
                    tel.i_actual,
                    tel.flags,
                    edges(&dfn_state(&p), &c)
                );
            }
            if held && -tel.i_actual <= 0.05 * q {
                println!("{temp_c} C: hold ends at {k} s; {}", edges(&dfn_state(&p), &c));
                break;
            }
        }
        for j in 1..=30 {
            let tel = p.step(1.0, Demand::Current(0.5 * q), &env(t));
            if j <= 5 || j % 10 == 0 {
                println!(
                    "{temp_c} C discharge {j}: {:.5} V {:?}; {}",
                    tel.v_terminal,
                    tel.flags,
                    edges(&dfn_state(&p), &c)
                );
            }
        }
        let _ = (F, EventFlags::empty(), PlatingModel::Threshold);
    }
}

/// Diagnostic: the analytic Jacobian against a central difference at the states a discharge after
/// the 0 °C hold fails from (the start of discharge steps 1, 300 and 900).
#[test]
#[ignore = "diagnostic: the Jacobian where the discharge after a cold hold fails"]
fn report_jacobian_after_the_cold_hold() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = 273.15;
    let mut p = pack(t, None);
    let mut held = false;
    loop {
        let d = if held { Demand::Voltage(4.2) } else { Demand::Current(-q) };
        let tel = p.step(1.0, d, &env(t));
        if !held && tel.v_terminal >= 4.2 {
            held = true;
        } else if held && -tel.i_actual <= 0.05 * q {
            break;
        }
    }
    let spm = c.spm.as_ref().unwrap();
    let dfn = c.dfn.as_ref().unwrap();
    let health = sim_core::dfn::probe::Health {
        eff_r0_factor: 1.0,
        eff_capacity_ah: q,
    };
    let nv = sim_core::dfn::UNKNOWNS_PER_NODE;
    for k in 1..=900 {
        if [1, 300, 900].contains(&k) {
            let s = dfn_state(&p);
            for h_rel in [1.0e-7, 1.0e-9, 1.0e-11] {
                let (m, a, n) =
                    sim_core::dfn::probe::jacobian_pair(&s, spm, dfn, health, 0.5 * q, 1.0, h_rel);
                let mut worst = (0.0, 0, 0);
                for row in 0..m {
                    let scale = (0..m).map(|col| a[row * m + col].abs()).fold(0.0, f64::max);
                    for col in 0..m {
                        let d = (a[row * m + col] - n[row * m + col]).abs() / scale.max(1e-300);
                        if d > worst.0 {
                            worst = (d, row, col);
                        }
                    }
                }
                let (_, row, col) = worst;
                println!(
                    "step {k} h_rel {h_rel:e}: worst row-relative {:.3e} at row (node {}, var {}) col (node {}, var {}): analytic {:.6e}, numeric {:.6e}",
                    worst.0,
                    row / nv,
                    row % nv,
                    col / nv,
                    col % nv,
                    a[row * m + col],
                    n[row * m + col]
                );
            }
        }
        p.step(1.0, Demand::Current(0.5 * q), &env(t));
    }
}

/// Predictions 2 and 3 (`docs/plans/dfn-cold-hold.md`): 1C to 4.2 V, 4.2 V held to C/20, then C/2
/// to 2.5 V, 1 s steps — on the fine grid with the plating reaction at 0 °C and −10 °C, and on the
/// recommended grid with a threshold cell at 0 °C. No hold step puts a surface out of range, and
/// every discharge step converges.
#[test]
fn a_cold_hold_leaves_a_discharge_that_converges() {
    let mut c = chem();
    // The drawn fades and both dendrite hazards off, so nothing but the electrochemistry moves.
    let a = c.aging.as_mut().unwrap();
    a.cal_pre_exp = 0.0;
    a.cyc_fade_per_ah = 0.0;
    let s = c.safety.as_mut().unwrap();
    s.plating_fade_per_ah = 0.0;
    s.plating_short_hazard_per_ah = 0.0;
    c.dfn.as_mut().unwrap().plating.as_mut().unwrap().short_hazard_per_dead_ah = 0.0;
    let fine = CellModelConfig::Dfn {
        shells: sim_core::dfn::FINE_SHELLS,
        nodes_negative: sim_core::dfn::FINE_NODES_NEGATIVE,
        nodes_separator: sim_core::dfn::FINE_NODES_SEPARATOR,
        nodes_positive: sim_core::dfn::FINE_NODES_POSITIVE,
    };
    let cases = [
        ("fine, reaction, 0 C", fine, PlatingModel::Reaction, 0.0),
        ("fine, reaction, -10 C", fine, PlatingModel::Reaction, -10.0),
        ("recommended, threshold, 0 C", recommended(), PlatingModel::Threshold, 0.0),
    ];
    let q = c.cell.capacity_ah;
    let rows: Vec<_> = std::thread::scope(|sc| {
        let hs: Vec<_> = cases
            .iter()
            .map(|&(name, model, plating, temp_c)| {
                let c = &c;
                sc.spawn(move || {
                    let t = 273.15 + temp_c;
                    let mut p = Pack::new(
                        &PackConfig {
                            series: 1,
                            parallel: 1,
                            initial_soc: 0.0,
                            initial_temp_k: t,
                            seed: 0,
                            scatter: Scatter::default(),
                            thermal: ThermalConfig::Isothermal,
                            bms: None,
                            aging: Some(AgingConfig {
                                plating,
                                ..AgingConfig::default()
                            }),
                            cell_model: model,
                        },
                        c.clone(),
                    )
                    .expect("builds");
                    let (mut held, mut out, mut hold_unc) = (false, 0, 0);
                    loop {
                        let d = if held { Demand::Voltage(4.2) } else { Demand::Current(-q) };
                        let tel = p.step(1.0, d, &env(t));
                        if held {
                            out += u32::from(tel.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE));
                            hold_unc += u32::from(tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
                        }
                        if !held && tel.v_terminal >= 4.2 {
                            held = true;
                        } else if held && -tel.i_actual <= 0.05 * q {
                            break;
                        }
                    }
                    let (mut unc, mut steps) = (0, 0);
                    loop {
                        let tel = p.step(1.0, Demand::Current(0.5 * q), &env(t));
                        steps += 1;
                        unc += u32::from(tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
                        if tel.v_terminal <= 2.5 || tel.v_terminal.is_nan() || steps > 3 * 3600 {
                            break;
                        }
                    }
                    (name, out, hold_unc, unc, steps)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (name, out, hold_unc, unc, steps) in rows {
        println!("{name}: hold out of range {out}, hold unconverged {hold_unc}; discharge unconverged {unc} of {steps}");
        assert_eq!(out, 0, "{name}: the hold put a surface out of range");
        assert_eq!(hold_unc, 0, "{name}: the hold did not converge");
        assert_eq!(unc, 0, "{name}: the discharge did not converge");
        assert!(steps < 3 * 3600, "{name}: no cut-off");
    }
}
