//! A long step solved against end-of-step sources, and the divergences it removed.
//!
//! `docs/plans/end-of-step-split.md` is the note. Before it, an equivalent-circuit cell
//! handed the pack solve its **start-of-step** Thévenin source, and the solve held the
//! resulting current for the whole step. That is explicit Euler on every coupling the
//! cells have through their node, and past a few hundred seconds it diverged: a scattered
//! 1S3P group reached 5363 K at a 600 s step, a 4S3P pack 11 459 K at an hour, and a
//! single cell holding a voltage chattered at 60 s and reached 90 000 K at 600 s. Nothing
//! caught it, because every long-step test was a single cell under a current demand.
//!
//! Every test here reddens on the start-of-step engine. The cell is a generic NMC-like
//! one — sloped OCV, two RC pairs a decade apart — written in the file so that the
//! instability's two ingredients, a SOC "capacitance" and an `R0` to couple through, are
//! both on the page.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, Telemetry, ThermalConfig};

const CAP_AH: f64 = 3.0;
const T_ENV: f64 = 298.15;

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// Sloped over its whole range, as a lithium-ion cell is, with `R0` of 16 mΩ and two RC
/// pairs (6.4 s and 48 s). The coupling time across a parallel group is `R0 · C_soc`,
/// with `C_soc = 3600 · Q / (dOCV/dsoc)` ≈ 10 800 / 0.8 F here: a few hundred seconds.
///
/// The soft-short-into-reversal case is **not** here. On this cell it stays bounded on the
/// start-of-step engine too — it was written here first and passed on both, which made it
/// a test of nothing — so it lives in `sim-data/tests/end_of_step_split_shipped.rs`, on the
/// shipped NMC 18650 whose divergence it was measured on.
fn chem() -> ChemistryParams {
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
            heat_capacity_j_per_k: 45.0,
            h_area_w_per_k: 0.2,
        },
        meta: ChemMeta {
            id: "end-of-step-split-test".into(),
            name: "End-of-step split test cell".into(),
            provenance: "engine test fixture — NMC-shaped, not fitted to anything".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 4.2,
            v_min: 2.5,
            max_charge_c: 1.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0],
            volts: vec![3.00, 3.45, 3.62, 3.72, 3.87, 4.05, 4.18],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![T_ENV],
            ohms: vec![vec![0.016], vec![0.016]],
        },
        rc: vec![
            RcPair {
                r_ohms: 0.008,
                c_farad: 800.0,
            },
            RcPair {
                r_ohms: 0.006,
                c_farad: 8000.0,
            },
        ],
    }
}

fn config(series: u16, parallel: u16, soc0: f64, sigma: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        series,
        parallel,
        initial_soc: soc0,
        initial_temp_k: T_ENV,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: sigma,
            r0_sigma: sigma,
        },
        cell_model: CellModelConfig::Ecm,
    }
}

/// Every branch current in the pack, from the per-cell report.
fn branch_currents(pack: &Pack, series: usize, parallel: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(series * parallel);
    for s in 0..series {
        for p in 0..parallel {
            let c = pack.cell(s, p).expect("in range");
            out.push(c.current_a.expect("a step has run"));
        }
    }
    out
}

/// The bound the note's first prediction registered: every cell under 400 K, every
/// branch under 10 A. The demands here are a few amps; the diverged engine exceeded both
/// by orders of magnitude.
fn assert_bounded(label: &str, tele: &Telemetry, currents: &[f64]) {
    assert!(
        tele.t_max < 400.0 && tele.t_max.is_finite(),
        "{label}: t_max {} K",
        tele.t_max
    );
    for (k, &i) in currents.iter().enumerate() {
        assert!(
            i.abs() < 10.0 && i.is_finite(),
            "{label}: branch {k} carries {i} A"
        );
    }
}

/// **A scattered parallel group stays a parallel group at a long step.** The cells share
/// the load in proportion to their conductance and nothing circulates beyond what the
/// scatter implies — at a 300 s, 600 s and one-hour step. The start-of-step engine was
/// already growing at 300 s and past 5000 K by 600 s.
#[test]
fn a_scattered_group_stays_bounded_at_long_steps() {
    for dt in [300.0, 600.0, 3600.0] {
        let mut pack = Pack::new(&config(1, 3, 0.95, 0.05), chem()).expect("builds");
        // C/5 on three cells, to empty and a little past.
        let steps = (6.0 * 3600.0 / dt) as usize;
        for n in 0..steps {
            let tele = pack.step(dt, Demand::Current(1.8), &env());
            let i = branch_currents(&pack, 1, 3);
            assert_bounded(&format!("dt {dt} step {n}"), &tele, &i);
            // No cell is ever pushed backwards while the group discharges: that is the
            // circulation the explicit split created and then amplified.
            for (k, &ik) in i.iter().enumerate() {
                assert!(
                    ik > 0.0,
                    "dt {dt} step {n}: branch {k} is charging at {ik} A"
                );
            }
        }
    }
}

/// The same at pack scale, the case recorded at 11 459 K: 4S3P, an hour a step.
#[test]
fn a_four_by_three_pack_stays_bounded_at_an_hours_step() {
    let mut pack = Pack::new(&config(4, 3, 0.95, 0.05), chem()).expect("builds");
    for n in 0..6 {
        let tele = pack.step(3600.0, Demand::Current(1.8), &env());
        assert_bounded(&format!("step {n}"), &tele, &branch_currents(&pack, 4, 3));
    }
}

/// **A circulation mode is removed, not rung.** Two cells at different charge states,
/// released to rest at a step a million seconds long. Backward Euler on the coupling
/// damps the mode by `R0 / R_step` per step, which at this `dt` is almost nothing left:
/// the circulating current must fall monotonically and never change sign. The trapezoid
/// rule — a split solved at the step's *mean* voltage — would leave it ringing at
/// constant amplitude, and the start-of-step split amplifies it.
#[test]
fn circulation_decays_without_changing_sign() {
    let mut pack = Pack::new(&config(1, 2, 0.8, 0.0), chem()).expect("builds");
    // Half the capacity on one cell, then a discharge: the small cell ends lower.
    pack.set_cell_factors(0, 1, 0.5, 1.0).expect("in range");
    for _ in 0..60 {
        pack.step(60.0, Demand::Current(3.0), &env());
    }
    let mut prev: Option<f64> = None;
    for n in 0..6 {
        pack.step(1.0e6, Demand::Rest, &env());
        let i0 = pack
            .cell(0, 0)
            .expect("in range")
            .current_a
            .expect("stepped");
        if let Some(p) = prev {
            assert!(
                p == 0.0 || i0 == 0.0 || p.signum() == i0.signum(),
                "step {n}: circulation flipped sign, {p} A -> {i0} A"
            );
            assert!(
                i0.abs() <= p.abs(),
                "step {n}: circulation grew, {p} A -> {i0} A"
            );
        }
        prev = Some(i0);
    }
    let first = pack.cell(0, 0).expect("in range");
    let second = pack.cell(0, 1).expect("in range");
    assert!(
        (first.soc - second.soc).abs() < 0.02,
        "a day of rest should have all but equalised the two: {} vs {}",
        first.soc,
        second.soc
    );
}

/// **A voltage hold holds.** A single cell charging to 4.1 V at a 60 s, 600 s and one-hour
/// step. The start-of-step engine alternated between −18 A and −0.5 A at 60 s and reached
/// 90 000 K by 600 s.
///
/// What is asserted is what the end-of-step solve can promise, which is not "never
/// overshoots". The source's OCV term is the *local* segment's slope, and an hour-long
/// step from half full moves more than half the cell's charge — across two segments of
/// this table, the second 1.7 times steeper than the first. So the first hour charges
/// at 1.93 A and lands past 4.1 V, and the second hands 0.17 A back. That is one
/// linearisation error, spent once: after it the current may not change sign and may not
/// grow, which is the ringing the explicit engine could not stop doing, and the hold ends
/// at its target. (At 60 s and 600 s there is no hand-back at all.)
#[test]
fn a_voltage_hold_tapers_at_long_steps() {
    for dt in [60.0, 600.0, 3600.0] {
        let mut pack = Pack::new(&config(1, 1, 0.5, 0.0), chem()).expect("builds");
        let mut currents = Vec::new();
        let mut last = None;
        for n in 0..12 {
            let tele = pack.step(dt, Demand::Voltage(4.1), &env());
            // Unprotected, so the first step of the hold draws whatever the line gives
            // it — about 12 A from 3.72 V — and the 10 A bound the other tests use does
            // not apply. The divergence this pins was ±800 A.
            assert!(
                tele.t_max < 400.0,
                "dt {dt} step {n}: t_max {} K",
                tele.t_max
            );
            assert!(
                tele.i_actual.abs() < 20.0,
                "dt {dt} step {n}: {} A",
                tele.i_actual
            );
            currents.push(tele.i_actual);
            last = Some(tele);
        }
        for n in 2..currents.len() {
            let (p, i) = (currents[n - 1], currents[n]);
            assert!(
                p == 0.0 || i == 0.0 || p.signum() == i.signum(),
                "dt {dt} step {n}: the hold rang, {p} A -> {i} A ({currents:?})"
            );
            assert!(
                i.abs() <= p.abs() + 1e-9,
                "dt {dt} step {n}: current magnitude grew, {p} A -> {i} A ({currents:?})"
            );
        }
        let last = last.expect("ran");
        assert!(
            (last.v_terminal - 4.1).abs() < 5e-3,
            "dt {dt}: ended at {} V, not the 4.1 V held",
            last.v_terminal
        );
    }
}

/// **A zero-length step still solves the start-of-step line.** The end-of-step source is
/// behind an explicit `dt > 0` guard, so a probe step reads exactly the voltage the pack
/// presents *now*: `OCV − Σ V_rc − I·R0`, with no share of any RC pair.
#[test]
fn a_probe_step_reads_the_start_of_step_line() {
    let mut pack = Pack::new(&config(1, 1, 0.5, 0.0), chem()).expect("builds");
    let probe = pack.step(0.0, Demand::Current(3.0), &env());
    let ocv = 3.72; // the table at 0.5
    let expected = ocv - 3.0 * 0.016;
    assert!(
        (probe.v_terminal - expected).abs() < 1e-12,
        "probe read {} V, the start-of-step line says {expected} V",
        probe.v_terminal
    );
}
