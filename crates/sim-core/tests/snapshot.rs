//! Phase 1 snapshot / restore / replay determinism tests.
//!
//! The exit gate: snapshot at t/2, restore, continue — the continued trajectory
//! must be bit-identical to running straight through. To make "bit-identical" mean
//! something, the snapshot is round-tripped through a byte-exact serde format
//! (`bincode`), not merely cloned — this catches any field that fails to survive
//! serialization (e.g. the RNG state).

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::{Demand, Env, Pack, PackConfig, RestoreError, Scatter, Telemetry, SNAPSHOT_VERSION};

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A deliberately non-trivial chemistry: sloped OCV, temperature-varying R0 grid,
/// two RC pairs — so a broken round-trip of almost any field shows up.
fn rich_chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "rich".into(),
            name: "Rich synthetic cell".into(),
            provenance: "snapshot test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: 2.5,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.40, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 0.5, 1.0],
            temp_k: vec![283.15, 298.15, 318.15],
            ohms: vec![
                vec![0.030, 0.022, 0.018],
                vec![0.028, 0.020, 0.016],
                vec![0.029, 0.021, 0.017],
            ],
        },
        rc: vec![
            RcPair {
                r_ohms: 0.010,
                c_farad: 2000.0,
            },
            RcPair {
                r_ohms: 0.006,
                c_farad: 5000.0,
            },
        ],
    }
}

fn config() -> PackConfig {
    PackConfig {
        series: 2,
        parallel: 2,
        initial_soc: 0.7,
        initial_temp_k: 298.15,
        seed: 0xC0FFEE,
        // Scatter on: per-cell factors are drawn from the RNG, so the RNG state is
        // part of what must round-trip, and the cells are genuinely asymmetric.
        scatter: Scatter {
            capacity_sigma: 0.03,
            r0_sigma: 0.05,
        },
    }
}

/// A mixed demand schedule (discharge, rest, charge, power discharge) keyed on the
/// step index, so both runs drive the identical sequence.
fn demand_at(step: usize) -> Demand {
    match step % 40 {
        0..=14 => Demand::Current(2.0),   // discharge
        15..=19 => Demand::Rest,          // relax
        20..=29 => Demand::Current(-1.5), // charge
        _ => Demand::Power(4.0),          // power discharge
    }
}

#[test]
fn snapshot_restore_replay_is_bit_identical() {
    const TOTAL: usize = 120;
    const MID: usize = 60;
    let dt = 0.5;

    // Reference run: straight through, recording the tail (steps MID..TOTAL).
    let mut reference = Pack::new(&config(), rich_chem()).unwrap();
    let mut ref_tail: Vec<Telemetry> = Vec::new();
    for step in 0..TOTAL {
        let tele = reference.step(dt, demand_at(step), &env());
        if step >= MID {
            ref_tail.push(tele);
        }
    }

    // Replay run: step to MID, snapshot → serialize to bytes → deserialize →
    // restore, then continue and record the same tail.
    let mut replay = Pack::new(&config(), rich_chem()).unwrap();
    for step in 0..MID {
        replay.step(dt, demand_at(step), &env());
    }
    let snapshot = replay.snapshot();
    let bytes = bincode::serialize(&snapshot).expect("serialize snapshot");
    let restored_snapshot = bincode::deserialize(&bytes).expect("deserialize snapshot");
    let mut restored = Pack::restore(&restored_snapshot).expect("restore");

    assert!(
        (restored.sim_time_s() - (MID as f64) * dt).abs() < 1e-12,
        "restored sim time must match"
    );

    let mut replay_tail: Vec<Telemetry> = Vec::new();
    for step in MID..TOTAL {
        replay_tail.push(restored.step(dt, demand_at(step), &env()));
    }

    assert_eq!(ref_tail.len(), replay_tail.len(), "tail lengths must match");
    // Bit-identical: derived PartialEq on Telemetry compares every f64 with ==,
    // which is exact for the finite values a healthy trajectory produces.
    for (i, (a, b)) in ref_tail.iter().zip(replay_tail.iter()).enumerate() {
        assert_eq!(a, b, "telemetry diverged at tail index {i}");
    }
}

#[test]
fn restore_rejects_unknown_version() {
    let pack = Pack::new(&config(), rich_chem()).unwrap();
    let mut snap = pack.snapshot();
    assert_eq!(snap.version, SNAPSHOT_VERSION);
    snap.version = SNAPSHOT_VERSION + 1;
    let err = Pack::restore(&snap).unwrap_err();
    assert_eq!(
        err,
        RestoreError::VersionMismatch {
            found: SNAPSHOT_VERSION + 1,
            expected: SNAPSHOT_VERSION,
        }
    );
}
