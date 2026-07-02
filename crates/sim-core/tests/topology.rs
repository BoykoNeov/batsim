//! Phase 1 topology + closed-form group-solve tests.
//!
//! These exercise the pack electrical solve where imbalance physics live:
//!   * series groups share one current and their voltages add;
//!   * parallel cells share one node voltage and split current by resistance;
//!   * mismatched parallel cells circulate current even at zero external load.
//!
//! Cells are made asymmetric deterministically via [`Pack::set_cell_factors`] (the
//! weak-cell seam) so the expected split is a hand-computable closed form.

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::{Demand, Env, Pack, PackConfig, Scatter};

const CAP_AH: f64 = 2.5;
const R0: f64 = 0.02;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A flat-OCV, flat-R0 synthetic chemistry: OCV is constant `v0`, `R0` is `R0`
/// everywhere, one RC pair. Flatness makes the closed-form current split exact.
fn flat_chem(v0: f64) -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "flat".into(),
            name: "Flat synthetic cell".into(),
            provenance: "topology test — not physical".into(),
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
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![v0, v0],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
    }
}

fn config(series: u16, parallel: u16, initial_soc: f64) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 0,
        scatter: Scatter::default(),
    }
}

#[test]
fn parallel_group_splits_current_by_resistance() {
    // 1S2P, flat identical OCV. Cell A has R0×1, cell B has R0×3 (both from rest,
    // so E_a = E_b = OCV). Under a group current I_g the shared node is
    //   V = (E/R_a + E/R_b − I_g)/(1/R_a + 1/R_b),
    // and I_k = (E − V)/R_k splits inversely to resistance: I_a/I_b = R_b/R_a = 3.
    let mut pack = Pack::new(&config(1, 2, 0.5), flat_chem(3.30)).unwrap();
    pack.set_cell_factors(0, 0, 1.0, 1.0).unwrap();
    pack.set_cell_factors(0, 1, 1.0, 3.0).unwrap();

    // The split is solved once from the (rested, first-step) start state, so it is
    // exact at any dt; use dt = 1 s to keep the SOC deltas well clear of float
    // cancellation. With R_a = R0, R_b = 3·R0 the current splits inversely to
    // resistance: I_a = 3 A, I_b = 1 A (sum = 4 A).
    let i_g = 4.0;
    let dt = 1.0;
    let soc0 = 0.5;
    let tele = pack.step(dt, Demand::Current(i_g), &env());
    assert!((tele.i_actual - i_g).abs() < 1e-12);

    // ΔSOC_k = I_k·dt / (3600·cap); both caps equal, so I_k = ΔSOC_k·3600·cap/dt.
    let cap_as = 3600.0 * CAP_AH;
    let i_a = (soc0 - pack.cell(0, 0).unwrap().soc) * cap_as / dt;
    let i_b = (soc0 - pack.cell(0, 1).unwrap().soc) * cap_as / dt;
    assert!((i_a - 3.0).abs() < 1e-9, "i_a = {i_a}");
    assert!((i_b - 1.0).abs() < 1e-9, "i_b = {i_b}");
    assert!(
        (i_a + i_b - i_g).abs() < 1e-9,
        "currents must sum to group current"
    );
}

#[test]
fn rest_circulates_current_between_mismatched_parallel_cells() {
    // 1S2P at Rest (I_g = 0), but the two cells sit at different SOC → different
    // OCV → different E. The solve must give nonzero, opposite-sign per-cell
    // currents (the higher-E cell pushes into the lower-E cell). This is the
    // imbalance physics that a per-cell "Rest ⇒ zero current" shortcut would erase.
    //
    // Use a sloped OCV so a SOC difference produces an E difference.
    let mut chem = flat_chem(3.30);
    chem.ocv = OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![3.0, 3.6],
    };
    // We cannot address a single cell's demand, so we create the SOC mismatch via
    // unequal capacities: under a shared group discharge the smaller-capacity cell
    // drops SOC faster, opening a gap.
    let mut pack = Pack::new(&config(1, 2, 0.8), chem).unwrap();
    pack.set_cell_factors(0, 0, 1.0, 1.0).unwrap();
    pack.set_cell_factors(0, 1, 0.5, 1.0).unwrap(); // half capacity → drains faster

    // Discharge the group to build a SOC gap between the two cells (B, at half
    // capacity, drops faster), then rest a while so the discharge RC transient
    // relaxes and the remaining E difference is OCV-driven (robustly A > B).
    for _ in 0..50 {
        pack.step(1.0, Demand::Current(2.0), &env());
    }
    for _ in 0..30 {
        pack.step(1.0, Demand::Rest, &env());
    }
    let soc_a = pack.cell(0, 0).unwrap().soc;
    let soc_b = pack.cell(0, 1).unwrap().soc;
    assert!(
        soc_a > soc_b,
        "cap mismatch should leave A above B: {soc_a} vs {soc_b}"
    );

    // Now measure the per-cell currents over one small rest step.
    let dt = 1e-3;
    let cap_as = 3600.0 * CAP_AH;
    let a0 = pack.cell(0, 0).unwrap().soc;
    let b0 = pack.cell(0, 1).unwrap().soc;
    let tele = pack.step(dt, Demand::Rest, &env());
    assert_eq!(tele.i_actual, 0.0, "external group current is zero at rest");

    let i_a = (a0 - pack.cell(0, 0).unwrap().soc) * (cap_as * 1.0) / dt;
    let i_b = (b0 - pack.cell(0, 1).unwrap().soc) * (cap_as * 0.5) / dt;
    // Higher-SOC (higher-OCV) cell A discharges into lower-SOC cell B: I_a > 0,
    // I_b < 0, and they cancel (external current is zero).
    assert!(i_a > 1e-6, "cell A should source current at rest: {i_a}");
    assert!(i_b < -1e-6, "cell B should sink current at rest: {i_b}");
    assert!(
        (i_a + i_b).abs() < 1e-6,
        "circulating currents must cancel: {i_a} + {i_b}"
    );
}

#[test]
fn series_stacks_voltage_and_shares_current() {
    // 2S1P: terminal voltage is the sum of the two group voltages, and both groups
    // carry the same current. With identical flat cells each group sits at
    // V = OCV − I·R0, so terminal = 2·(OCV − I·R0).
    let v0 = 3.30;
    let mut pack = Pack::new(&config(2, 1, 0.5), flat_chem(v0)).unwrap();
    // leave factors nominal
    let i = 1.0;
    let tele = pack.step(1e-6, Demand::Current(i), &env());
    let expected = 2.0 * (v0 - i * R0);
    assert!(
        (tele.v_terminal - expected).abs() < 1e-6,
        "got {}, expected {expected}",
        tele.v_terminal
    );
    // Both series groups share the same current; the ground-truth SOCs match.
    assert!((pack.cell(0, 0).unwrap().soc - pack.cell(1, 0).unwrap().soc).abs() < 1e-15);
    let _ = pack.set_cell_factors(2, 0, 1.0, 1.0).unwrap_err(); // OOB index rejected
}

#[test]
fn parallel_reduces_pack_resistance_vs_single_cell() {
    // A 1S2P pack of identical cells has half the series resistance of 1S1P, so at
    // the same current its terminal voltage sags half as much below OCV.
    let v0 = 3.30;
    let i = 2.0;
    let mut single = Pack::new(&config(1, 1, 0.5), flat_chem(v0)).unwrap();
    let mut dual = Pack::new(&config(1, 2, 0.5), flat_chem(v0)).unwrap();
    let t1 = single.step(1e-6, Demand::Current(i), &env());
    let t2 = dual.step(1e-6, Demand::Current(i), &env());
    let sag1 = v0 - t1.v_terminal; // = i·R0
    let sag2 = v0 - t2.v_terminal; // = i·(R0/2)
    assert!((sag1 - i * R0).abs() < 1e-6);
    assert!((sag2 - i * R0 / 2.0).abs() < 1e-6);
}
