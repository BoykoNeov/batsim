//! Phase 1 exit-gate scenario: a weak cell caps pack capacity by the weakest
//! series element.
//!
//! In a series string every group carries the same current, so the group with the
//! least charge (here a half-capacity cell) reaches empty first and defines how
//! much the whole pack can deliver — the stronger groups still hold charge that is
//! now stranded. This is a ground-truth (BMS-off) capacity limit that falls out of
//! the series solve, not a scripted cutoff.

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::{Demand, Env, EventFlags, Pack, PackConfig, Scatter};

const CAP_AH: f64 = 2.5;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "wk".into(),
            name: "Weak-cell scenario cell".into(),
            provenance: "scenario test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        // Sloped OCV so the string is a realistic Thévenin; the capacity cap itself
        // comes from coulomb counting, not the voltage curve.
        ocv: OcvTable {
            soc: vec![0.0, 0.1, 0.5, 0.9, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.45, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
    }
}

fn config() -> PackConfig {
    PackConfig {
        series: 2,
        parallel: 1,
        initial_soc: 1.0,
        initial_temp_k: 298.15,
        seed: 0,
        scatter: Scatter::default(),
    }
}

/// Discharge a 2S1P pack at constant current until the weakest series cell empties
/// (raises `SOC_CLAMPED_LOW`). Returns (delivered Ah, weak cell SOC, strong cell
/// SOC) at that cutoff. `weak_factor` scales the second group's capacity.
fn discharge_until_first_empty(weak_factor: f64) -> (f64, f64, f64) {
    let mut pack = Pack::new(&config(), chem()).unwrap();
    // Group 0 nominal, group 1 the weak cell.
    pack.set_cell_factors(1, 0, weak_factor, 1.0).unwrap();

    let i = CAP_AH; // 1C relative to nominal
    let dt = 1.0;
    let mut delivered_ah = 0.0;
    for _ in 0..10_000 {
        let tele = pack.step(dt, Demand::Current(i), &env());
        delivered_ah += i * dt / 3600.0;
        // The strong cell must never be the one to empty first.
        let strong = pack.cell(0, 0).unwrap().soc;
        let weak = pack.cell(1, 0).unwrap().soc;
        assert!(
            weak <= strong + 1e-12,
            "weak cell must lead the discharge: weak {weak}, strong {strong}"
        );
        if tele.flags.contains(EventFlags::SOC_CLAMPED_LOW) {
            return (delivered_ah, weak, strong);
        }
    }
    panic!("weak cell never emptied");
}

#[test]
fn weak_series_cell_caps_pack_capacity() {
    // Weak group at half capacity: it empties after delivering ~half the nominal
    // charge, while the strong group is only half-drained — its remaining charge is
    // stranded by the series constraint.
    let (delivered, weak_soc, strong_soc) = discharge_until_first_empty(0.5);

    assert!(
        weak_soc <= 1e-9,
        "weak cell should be empty at cutoff, got {weak_soc}"
    );
    // Same current through both; weak has half the capacity, so when it hits 0 the
    // strong cell has drained half as much SOC → ~0.5 remaining.
    assert!(
        (strong_soc - 0.5).abs() < 0.02,
        "strong cell should retain ~half charge, got {strong_soc}"
    );
    // Pack delivered ~ weak capacity = 0.5 × nominal.
    assert!(
        (delivered - 0.5 * CAP_AH).abs() < 0.02,
        "delivered {delivered} Ah, expected ~{}",
        0.5 * CAP_AH
    );
}

#[test]
fn balanced_pack_delivers_full_capacity() {
    // Control: with both groups nominal the pack delivers ~full nominal capacity,
    // and the two cells empty together (neither is stranded). This is the contrast
    // that makes the weak-cell cap meaningful.
    let (delivered, weak_soc, strong_soc) = discharge_until_first_empty(1.0);
    assert!(weak_soc <= 1e-9);
    assert!(
        strong_soc <= 1e-6,
        "balanced: both cells empty together, strong {strong_soc}"
    );
    assert!(
        (delivered - CAP_AH).abs() < 0.02,
        "delivered {delivered} Ah, expected ~{CAP_AH}"
    );
}
