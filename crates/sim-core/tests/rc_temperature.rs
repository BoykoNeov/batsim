//! `[r0] rc_t_ref_k`: the RC pairs follow `R0`'s temperature ratio (Phase 11, decision 6 of
//! `docs/plans/phase-11-fitting.md`).
//!
//! Three claims, each predicted before the run:
//!
//! * **At the reference temperature the rule moves nothing, to the bit.** The factor is
//!   `R0(soc, T)/R0(soc, T)`, and IEEE division of a finite non-zero number by itself is
//!   exactly `1.0`, so the multiplier is `soh · 1.0 = soh` exactly.
//! * **Away from it, a constant current gives the closed form with both the pair's
//!   resistance and its time constant scaled by the factor** (capacitance held), to the
//!   1e-9 V the analytic golden holds the engine to.
//! * **A reference temperature that is not a positive finite kelvin is refused.**

// The expected values use `std`'s `exp` on purpose: a reference independent of the
// engine's `crate::math`, compared within a tolerance (see analytic_golden.rs).
#![allow(clippy::disallowed_methods)]

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, ThermalConfig};

const R0_COLD: f64 = 0.05; // ohms at 263.15 K
const R0_WARM: f64 = 0.02; // ohms at 298.15 K
const R1: f64 = 0.01; // ohms, at the reference temperature
const C1: f64 = 2000.0; // farads => tau = 20 s at the reference temperature
const V0: f64 = 3.30;

fn chem(rc_t_ref_k: Option<f64>) -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "synthetic".into(),
            name: "Synthetic test cell".into(),
            provenance: "rc temperature rule — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: 2.5,
            v_max: 5.0,
            v_min: 0.0,
            max_charge_c: 10.0,
            max_discharge_c: 10.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        // Flat, so the terminal voltage carries only the two resistances.
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![V0, V0],
            docv_dt_v_per_k: None,
            t_ref_k: None,
        },
        // Flat in state of charge, so the factor is one number per temperature:
        // 0.05 / 0.02 = 2.5 at 263.15 K.
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![263.15, 298.15],
            ohms: vec![vec![R0_COLD, R0_WARM], vec![R0_COLD, R0_WARM]],
            rc_t_ref_k,
        },
        rc: vec![RcPair {
            r_ohms: R1,
            c_farad: C1,
        }],
    }
}

fn run(rc_t_ref_k: Option<f64>, temp_k: f64) -> Vec<f64> {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: 0.5,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: sim_core::Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    };
    let env = Env {
        t_ambient: temp_k,
        t_coolant: None,
    };
    let mut pack = Pack::new(&config, chem(rc_t_ref_k)).expect("the pack builds");
    let mut v = Vec::new();
    for k in 0..200 {
        // A pulse and a rest, so both directions of the pair's response are compared.
        let i = if k < 100 { 1.0 } else { 0.0 };
        v.push(pack.step(1.0, Demand::Current(i), &env).v_terminal);
    }
    v
}

#[test]
fn at_the_reference_temperature_the_rule_moves_nothing() {
    let off = run(None, 298.15);
    let on = run(Some(298.15), 298.15);
    for (k, (a, b)) in off.iter().zip(&on).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "step {k}: {a} against {b}");
    }
}

#[test]
fn away_from_it_the_pair_scales_with_r0() {
    let f = R0_COLD / R0_WARM;
    let (r, tau) = (R1 * f, R1 * f * C1);
    let on = run(Some(298.15), 263.15);
    let off = run(None, 263.15);
    for (k, v) in on.iter().enumerate() {
        let t = (k + 1) as f64;
        let expected = if k < 100 {
            V0 - R0_COLD - r * (1.0 - (-t / tau).exp())
        } else {
            let v_end = r * (1.0 - (-100.0 / tau).exp());
            V0 - v_end * (-(t - 100.0) / tau).exp()
        };
        assert!(
            (v - expected).abs() < 1e-9,
            "t = {t} s: got {v}, expected {expected}"
        );
    }
    // And the rule is what moved it: without it the cold pair is the table's.
    let at_pulse_end = |v: &[f64]| V0 - R0_COLD - v[99];
    assert!(
        (at_pulse_end(&on) / at_pulse_end(&off)
            - (1.0 - (-100.0 / tau).exp()) * f / (1.0 - (-100.0 / (R1 * C1)).exp()))
        .abs()
            < 1e-6,
        "the pair's sag at the end of the pulse should be the scaled closed form's"
    );
}

#[test]
fn a_reference_temperature_must_be_a_positive_kelvin() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            chem(Some(bad)).validate().is_err(),
            "rc_t_ref_k = {bad} validated"
        );
    }
    chem(Some(298.15))
        .validate()
        .expect("a positive kelvin validates");
}
