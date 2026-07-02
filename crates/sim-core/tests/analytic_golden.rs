//! Phase 0 analytic goldens for the 1RC ECM single cell.
//!
//! Constant-current discharge of a 1RC Thevenin cell from a rested state
//! (`V_rc(0) = 0`) has a closed form:
//!
//! ```text
//! V(t) = OCV(soc(t)) − I·R0 − I·R1·(1 − e^(−t/τ)),  τ = R1·C1,
//! soc(t) = soc0 − I·t / (3600·Q)
//! ```
//!
//! To keep the comparison exact we isolate the mechanisms:
//!   (a) a constant-OCV chemistry validates the RC exponential + coulomb counting
//!       with no interpolation error;
//!   (b) a single linear OCV segment additionally validates interpolation.
//! A dt-invariance check confirms the exact update is stable/consistent at any dt.

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::ecm::{ocv_lookup, r0_lookup};
use sim_core::{Demand, Env, Pack, PackConfig};

const R0: f64 = 0.02; // ohms
const R1: f64 = 0.01; // ohms
const C1: f64 = 2000.0; // farads  => tau1 = 20 s
const R2: f64 = 0.006; // ohms
const C2: f64 = 5000.0; // farads  => tau2 = 30 s
const CAP_AH: f64 = 2.5; // Ah
const TAU: f64 = R1 * C1;
const TAU2: f64 = R2 * C2;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Build a synthetic chemistry with a caller-supplied OCV table and flat R0.
fn synthetic_chem(ocv: OcvTable) -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "synthetic".into(),
            name: "Synthetic test cell".into(),
            provenance: "analytic golden — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 5.0,
            v_min: 0.0,
            max_charge_c: 10.0,
            max_discharge_c: 10.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv,
        // Flat R0 over soc and a single temperature: R0 is constant everywhere.
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: R1,
            c_farad: C1,
        }],
    }
}

/// Build a synthetic two-RC-pair chemistry with a caller-supplied OCV table and
/// flat R0. Reuses the same limits as [`synthetic_chem`].
fn synthetic_chem_2rc(ocv: OcvTable) -> ChemistryParams {
    let mut chem = synthetic_chem(ocv);
    chem.rc = vec![
        RcPair {
            r_ohms: R1,
            c_farad: C1,
        },
        RcPair {
            r_ohms: R2,
            c_farad: C2,
        },
    ];
    chem
}

fn config(initial_soc: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 0,
    }
}

/// Closed-form terminal voltage for constant-current discharge from rest.
fn v_analytic(ocv_at_t: f64, i: f64, t: f64) -> f64 {
    ocv_at_t - i * R0 - i * R1 * (1.0 - (-t / TAU).exp())
}

#[test]
fn constant_ocv_cc_discharge_matches_closed_form() {
    let v0 = 3.30;
    let chem = synthetic_chem(OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![v0, v0],
    });
    let mut pack = Pack::new(&config(0.5), chem).unwrap();

    let i = 1.0; // 1 A discharge
    let dt = 1.0;
    for _ in 0..100 {
        let tele = pack.step(dt, Demand::Current(i), &env());
        let t = pack.sim_time_s();
        let expected = v_analytic(v0, i, t);
        assert!(
            (tele.v_terminal - expected).abs() < 1e-9,
            "t={t}: got {}, expected {expected}",
            tele.v_terminal
        );
        // OCV is flat so SOC never leaves the interior here.
        assert!(tele.flags.is_empty(), "unexpected flags at t={t}");
    }
}

#[test]
fn linear_ocv_segment_cc_discharge_matches_closed_form() {
    // One strictly-increasing OCV segment; the run stays inside [0.2, 0.8].
    let ocv = OcvTable {
        soc: vec![0.2, 0.8],
        volts: vec![3.20, 3.40],
    };
    let chem = synthetic_chem(ocv.clone());
    let soc0 = 0.6;
    let mut pack = Pack::new(&config(soc0), chem).unwrap();

    let i = 1.0;
    let dt = 1.0;
    let cap_as = 3600.0 * CAP_AH;
    for _ in 0..100 {
        let tele = pack.step(dt, Demand::Current(i), &env());
        let t = pack.sim_time_s();
        let soc_t = soc0 - i * t / cap_as;
        // Inline the linear-segment OCV so `expected` does not depend on the
        // function under test: OCV = 3.20 + (soc − 0.2)/(0.8 − 0.2)·(3.40 − 3.20).
        let ocv_at_t = 3.20 + (soc_t - 0.2) / 0.6 * 0.20;
        let expected = v_analytic(ocv_at_t, i, t);
        assert!(
            (tele.v_terminal - expected).abs() < 1e-9,
            "t={t}: got {}, expected {expected}",
            tele.v_terminal
        );
    }
}

#[test]
fn constant_ocv_2rc_cc_discharge_matches_closed_form() {
    // Two-RC cell (CellModel::Ecm2Rc). From rest the overpotential is the sum of
    // two independent exponentials, so:
    //   V(t) = V0 − I·R0 − I·R1·(1 − e^(−t/τ1)) − I·R2·(1 − e^(−t/τ2)).
    let v0 = 3.30;
    let chem = synthetic_chem_2rc(OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![v0, v0],
    });
    let mut pack = Pack::new(&config(0.5), chem).unwrap();

    let i = 1.0;
    let dt = 1.0;
    for _ in 0..100 {
        let tele = pack.step(dt, Demand::Current(i), &env());
        let t = pack.sim_time_s();
        let expected =
            v0 - i * R0 - i * R1 * (1.0 - (-t / TAU).exp()) - i * R2 * (1.0 - (-t / TAU2).exp());
        assert!(
            (tele.v_terminal - expected).abs() < 1e-9,
            "t={t}: got {}, expected {expected}",
            tele.v_terminal
        );
    }
}

#[test]
fn dt_invariance_to_matching_sim_time() {
    // Same trajectory sampled with dt and dt/2 must agree at equal sim-time.
    let ocv = OcvTable {
        soc: vec![0.2, 0.8],
        volts: vec![3.20, 3.40],
    };
    let soc0 = 0.6;
    let i = 1.0;

    let make = || Pack::new(&config(soc0), synthetic_chem(ocv.clone())).unwrap();

    let mut coarse = make();
    let mut fine = make();

    // Advance both to t = 40 s: 40 steps of 1 s vs 80 steps of 0.5 s. Compare the
    // final discharge telemetry (both end at t = 40 s under the same current).
    let mut coarse_last = None;
    for _ in 0..40 {
        coarse_last = Some(coarse.step(1.0, Demand::Current(i), &env()));
    }
    let mut fine_last = None;
    for _ in 0..80 {
        fine_last = Some(fine.step(0.5, Demand::Current(i), &env()));
    }
    let coarse_last = coarse_last.unwrap();
    let fine_last = fine_last.unwrap();

    assert!((coarse.sim_time_s() - 40.0).abs() < 1e-12);
    assert!((fine.sim_time_s() - 40.0).abs() < 1e-12);
    assert!(
        (coarse_last.soc_true - fine_last.soc_true).abs() < 1e-9,
        "soc: coarse {} vs fine {}",
        coarse_last.soc_true,
        fine_last.soc_true
    );
    assert!(
        (coarse_last.v_terminal - fine_last.v_terminal).abs() < 1e-9,
        "v: coarse {} vs fine {}",
        coarse_last.v_terminal,
        fine_last.v_terminal
    );
}

#[test]
fn ocv_lookup_interpolates_and_clamps() {
    let table = OcvTable {
        soc: vec![0.0, 0.5, 1.0],
        volts: vec![3.0, 3.5, 3.6],
    };
    // Interior of the first segment: 3.0 + 0.5·(3.5 − 3.0) = 3.25.
    assert!((ocv_lookup(&table, 0.25) - 3.25).abs() < 1e-12);
    // Interior of the second segment: 3.5 + 0.5·(3.6 − 3.5) = 3.55.
    assert!((ocv_lookup(&table, 0.75) - 3.55).abs() < 1e-12);
    // Exact breakpoint.
    assert!((ocv_lookup(&table, 0.5) - 3.5).abs() < 1e-12);
    // Clamped below/above the table ends.
    assert!((ocv_lookup(&table, -0.1) - 3.0).abs() < 1e-12);
    assert!((ocv_lookup(&table, 1.5) - 3.6).abs() < 1e-12);
}

#[test]
fn r0_lookup_bilinear_and_axis_orientation() {
    // ohms[soc_idx][temp_idx]: corners are (soc=0,T=280)=0.05, (0,320)=0.03,
    // (1,280)=0.04, (1,320)=0.02.
    let table = R0Table {
        soc: vec![0.0, 1.0],
        temp_k: vec![280.0, 320.0],
        ohms: vec![vec![0.05, 0.03], vec![0.04, 0.02]],
    };
    // Corners pin the axis orientation (a transposed grid would fail these).
    assert!((r0_lookup(&table, 0.0, 280.0) - 0.05).abs() < 1e-12);
    assert!((r0_lookup(&table, 0.0, 320.0) - 0.03).abs() < 1e-12);
    assert!((r0_lookup(&table, 1.0, 280.0) - 0.04).abs() < 1e-12);
    assert!((r0_lookup(&table, 1.0, 320.0) - 0.02).abs() < 1e-12);
    // Center of the grid, bilinear:
    //   T=300 in soc=0 row: 0.05 + 0.5·(0.03 − 0.05) = 0.04
    //   T=300 in soc=1 row: 0.04 + 0.5·(0.02 − 0.04) = 0.03
    //   soc=0.5 across rows: 0.04 + 0.5·(0.03 − 0.04) = 0.035
    assert!((r0_lookup(&table, 0.5, 300.0) - 0.035).abs() < 1e-12);
    // Interpolate along one axis only (soc clamped low): temp midpoint of row 0.
    assert!((r0_lookup(&table, 0.0, 300.0) - 0.04).abs() < 1e-12);
    // Clamp on both axes to a corner.
    assert!((r0_lookup(&table, -0.5, 260.0) - 0.05).abs() < 1e-12);
}

#[test]
fn rest_holds_ocv_and_soc() {
    let v0 = 3.30;
    let chem = synthetic_chem(OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![v0, v0],
    });
    let mut pack = Pack::new(&config(0.5), chem).unwrap();

    let tele = pack.step(10.0, Demand::Rest, &env());
    assert_eq!(tele.i_actual, 0.0);
    assert!((tele.soc_true - 0.5).abs() < 1e-15);
    assert!((tele.v_terminal - v0).abs() < 1e-12);
}

#[test]
fn cv_demand_solves_current_from_rest() {
    // From rest, V_rc = 0, so I = (OCV − V_target) / R0.
    let v0 = 3.30;
    let chem = synthetic_chem(OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![v0, v0],
    });
    let mut pack = Pack::new(&config(0.5), chem).unwrap();

    let v_target = 3.20;
    let tele = pack.step(0.001, Demand::Voltage(v_target), &env());
    let expected_i = (v0 - v_target) / R0;
    assert!(
        (tele.i_actual - expected_i).abs() < 1e-9,
        "got {}, expected {expected_i}",
        tele.i_actual
    );
    assert!(tele.i_actual > 0.0, "discharge current should be positive");
}

#[test]
fn charge_raises_soc_and_clamps_high() {
    let v0 = 3.30;
    let chem = synthetic_chem(OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![v0, v0],
    });
    let mut pack = Pack::new(&config(0.99), chem).unwrap();

    // Negative current = charge. Push long/hard enough to overrun the top.
    let tele = pack.step(3600.0, Demand::Current(-CAP_AH), &env());
    assert!(
        (tele.soc_true - 1.0).abs() < 1e-15,
        "soc should clamp to 1.0"
    );
    assert!(tele.flags.contains(sim_core::EventFlags::SOC_CLAMPED_HIGH));
}
