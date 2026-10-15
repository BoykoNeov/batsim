//! `[dfn.electrolyte_temperature]`: the electrolyte's transport follows Landesfeind 2019's
//! published temperature forms, as a ratio to the concentration fits (Phase 11 slice E,
//! decision 2 of `docs/plans/phase-11-fitting.md`).
//!
//! The shipped LG M50 carries no section yet (slice C adds it), so this file gives it the
//! EC:EMC 3:7 coefficients PyBaMM's `ORegan2022` set uses. Predicted before the run:
//!
//! * **The ratios are the published forms**, evaluated here independently with `std`'s
//!   `exp`, to 1e-12 relative over 50–3000 mol/m³ and 253–333 K, and their concentration
//!   derivatives match a central difference to 1e-6 relative.
//! * **At the reference temperature a `Dfn` does not move, to the bit**: both ratios are
//!   exactly 1 and both derivatives exactly 0 there.
//! * **Cold, the analytic Jacobian still matches a difference quotient** (the new product-
//!   rule terms are differentiated right), and a cell under load reads lower with the
//!   section than without it — a colder electrolyte is a worse conductor.

// The reference values use `std`'s `exp` on purpose: an evaluation independent of the
// engine's `crate::math`, compared within a tolerance.
#![allow(clippy::disallowed_methods)]

use sim_core::chem::{ElectrolyteTemperature, ElectrolyteTemperatureForm};
use sim_core::dfn::probe;
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const T_REF: f64 = 298.15;
const D: [f64; 4] = [1.01e3, 1.01, -1.56e3, -4.87e2];
const K: [f64; 6] = [5.21e-1, 2.28e2, -1.06, 3.53e-1, -3.59e-3, 1.48e-3];

fn section() -> ElectrolyteTemperature {
    ElectrolyteTemperature {
        form: ElectrolyteTemperatureForm::Landesfeind2019,
        t_ref_k: T_REF,
        diffusivity: D.to_vec(),
        conductivity: K.to_vec(),
    }
}

/// The shipped LG M50 WITHOUT its electrolyte section — since Phase 11 slice C it ships
/// Landesfeind 2019's, and this file's comparisons are against a cell with none.
fn plain() -> ChemistryParams {
    let mut c = sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses");
    c.dfn.as_mut().expect("[dfn]").electrolyte_temperature = None;
    c
}

fn with_section() -> ChemistryParams {
    let mut c = plain();
    c.dfn.as_mut().expect("[dfn]").electrolyte_temperature = Some(section());
    c.validate().expect("the section validates");
    c
}

/// Landesfeind's forms as PyBaMM writes them, `c` in mol/L.
fn d_form(c: f64, t: f64) -> f64 {
    D[0] * (D[1] * c).exp() * (D[2] / t).exp() * (D[3] * c / t).exp()
}

fn k_form(c: f64, t: f64) -> f64 {
    let e = (1000.0 / t).exp();
    K[0] * (1.0 + (t - K[1])) * c * (1.0 + K[2] * c.sqrt() + K[3] * (1.0 + K[4] * e) * c)
        / (1.0 + c.powi(4) * K[5] * e)
}

#[test]
fn the_ratios_are_the_published_forms() {
    let et = section();
    for c_e in [50.0, 200.0, 500.0, 1000.0, 1500.0, 2000.0, 3000.0] {
        for t in [253.15, 263.15, 283.15, 298.15, 318.15, 333.15] {
            let ((fd, fd_c), (fk, fk_c)) = et.ratios(c_e, t);
            let c = c_e / 1000.0;
            let want_d = d_form(c, t) / d_form(c, T_REF);
            let want_k = k_form(c, t) / k_form(c, T_REF);
            assert!(
                (fd / want_d - 1.0).abs() < 1e-12,
                "D at {c_e}, {t}: {fd} vs {want_d}"
            );
            assert!(
                (fk / want_k - 1.0).abs() < 1e-12,
                "kappa at {c_e}, {t}: {fk} vs {want_k}"
            );
            let h = 1e-4 * c_e;
            let ((dp, _), (kp, _)) = et.ratios(c_e + h, t);
            let ((dm, _), (km, _)) = et.ratios(c_e - h, t);
            let (num_d, num_k) = ((dp - dm) / (2.0 * h), (kp - km) / (2.0 * h));
            let close = |a: f64, b: f64| (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1e-9);
            assert!(close(fd_c, num_d), "dD/dc at {c_e}, {t}: {fd_c} vs {num_d}");
            assert!(
                close(fk_c, num_k),
                "dkappa/dc at {c_e}, {t}: {fk_c} vs {num_k}"
            );
        }
    }
    let ((fd, fd_c), (fk, fk_c)) = et.ratios(1000.0, T_REF);
    assert_eq!(
        (fd, fk),
        (1.0, 1.0),
        "the ratios at the reference temperature"
    );
    assert_eq!(
        (fd_c, fk_c),
        (0.0, 0.0),
        "their derivatives at the reference temperature"
    );
}

fn model() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: 8,
        nodes_negative: 6,
        nodes_separator: 3,
        nodes_positive: 6,
    }
}

fn pack(chem: ChemistryParams, temp_k: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: 0.9,
        initial_temp_k: temp_k,
        seed: 5,
        scatter: Scatter::default(),
        cell_model: model(),
    };
    Pack::new(&config, chem).expect("builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

fn run(chem: ChemistryParams, temp_k: f64) -> Vec<f64> {
    let i = chem.cell.capacity_ah;
    let mut p = pack(chem, temp_k);
    (0..300)
        .map(|_| p.step(5.0, Demand::Current(i), &env(temp_k)).v_terminal)
        .collect()
}

#[test]
fn at_the_reference_temperature_the_dfn_does_not_move() {
    for (k, (a, b)) in run(plain(), T_REF)
        .iter()
        .zip(&run(with_section(), T_REF))
        .enumerate()
    {
        assert_eq!(a.to_bits(), b.to_bits(), "step {k}: {a} against {b}");
    }
}

/// [`with_section`] with the diffusivity's temperature terms zeroed (`p3 = p4 = 0`), so its
/// ratio is exactly 1 and every cold difference is the conductivity's alone. The first
/// version of this file could not see the conductivity at all: a perturbation that dropped
/// its ratio stayed green, because the diffusivity lowers a cold cell's voltage on its own.
fn conductivity_only() -> ChemistryParams {
    let mut c = with_section();
    let et = c
        .dfn
        .as_mut()
        .and_then(|d| d.electrolyte_temperature.as_mut())
        .expect("the section");
    et.diffusivity[2] = 0.0;
    et.diffusivity[3] = 0.0;
    c
}

/// The worst row-relative disagreement between the analytic Jacobian and a difference
/// quotient, for a cold cell 200 s into a 1C discharge on `chem`.
fn cold_jacobian_worst(chem: &ChemistryParams, temp_k: f64) -> f64 {
    let (spm, dfn) = (
        chem.spm.as_ref().expect("[spm]"),
        chem.dfn.as_ref().expect("[dfn]"),
    );
    let capacity = probe::spm_capacity(spm);
    let health = probe::Health {
        eff_r0_factor: 1.0,
        eff_capacity_ah: capacity,
    };
    let mut p = pack(chem.clone(), temp_k);
    for _ in 0..20 {
        p.step(10.0, Demand::Current(capacity), &env(temp_k));
    }
    let snap = serde_json::to_value(p.snapshot()).expect("serializes");
    let s: sim_core::DfnState =
        serde_json::from_value(snap["pack"]["groups"][0]["cells"][0]["model"]["Dfn"].clone())
            .expect("cell 0 is a Dfn cell");
    assert_eq!(s.temp_k, temp_k, "the state carries the cold temperature");
    let (m, analytic, numeric) = probe::jacobian_pair(&s, spm, dfn, health, capacity, 10.0, 1.0e-6);
    let mut worst = 0.0_f64;
    for row in 0..m {
        let scale = (0..m)
            .map(|c| analytic[row * m + c].abs().max(numeric[row * m + c].abs()))
            .fold(1.0e-30_f64, f64::max);
        for col in 0..m {
            worst = worst.max((analytic[row * m + col] - numeric[row * m + col]).abs() / scale);
        }
    }
    worst
}

#[test]
fn cold_the_jacobian_holds_and_the_cell_reads_lower() {
    let temp_k = 263.15;
    let (a, b) = (run(plain(), temp_k), run(with_section(), temp_k));
    let gap = a[100] - b[100];
    eprintln!(
        "whole section, 263.15 K, 1C at 500 s: {:.2} mV lower",
        1e3 * gap
    );
    assert!(
        gap > 0.0,
        "at 263.15 K the section should lower the loaded voltage; gap {gap} V"
    );
    let worst = cold_jacobian_worst(&with_section(), temp_k);
    assert!(
        worst < 1.0e-5,
        "worst row-relative Jacobian disagreement {worst:.3e}"
    );
}

#[test]
fn cold_the_conductivity_alone_lowers_the_cell() {
    let temp_k = 263.15;
    let (a, b) = (run(plain(), temp_k), run(conductivity_only(), temp_k));
    let gap = a[100] - b[100];
    eprintln!(
        "conductivity alone, 263.15 K, 1C at 500 s: {:.2} mV lower",
        1e3 * gap
    );
    assert!(
        gap > 1.0e-3,
        "the conductivity's ratio lowered the loaded voltage by only {gap} V"
    );
    let worst = cold_jacobian_worst(&conductivity_only(), temp_k);
    assert!(
        worst < 1.0e-5,
        "worst row-relative Jacobian disagreement {worst:.3e}"
    );
}
