//! Phase 1 snapshot / restore / replay determinism tests.
//!
//! The exit gate: snapshot at t/2, restore, continue — the continued trajectory
//! must be bit-identical to running straight through. To make "bit-identical" mean
//! something, the snapshot is round-tripped through a byte-exact serde format
//! (`bincode`), not merely cloned — this catches any field that fails to survive
//! serialization (e.g. the RNG state).

use sim_core::bms::{BalancingConfig, BmsConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, Pack, PackConfig, RestoreError, Scatter, Telemetry,
    ThermalConfig, SNAPSHOT_VERSION,
};

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
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
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
            docv_dt_v_per_k: None,
            t_ref_k: None,
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
        aging: None,
        // A BMS with a *noisy* sensor, so the round-trip has to carry both the last
        // sensor frame and an RNG that is drawn from every step. The frame cannot be
        // recomputed on restore — its group voltages depend on a current that is not
        // stored, and its noise draw has already advanced the RNG — so if it were
        // ever left out of the snapshot, this test is what would notice.
        bms: Some(BmsConfig {
            // Balancing on, with a threshold below the resting voltage of this cell at
            // SOC 0.7, so bleed switches are genuinely closed during these tests. The
            // bleed enters the group solve, so it changes every per-cell current — which
            // makes it exactly the feature the replay and zero-length-step contracts
            // need to cover, rather than the one they skip.
            balancing: Some(BalancingConfig {
                bleed_r_ohms: 47.0,
                v_threshold_v: 3.0,
                v_release_band_v: 0.010,
            }),
            protection: None,
            current_offset_a: 0.01,
            current_noise_sigma_a: 0.05,
            temp_probes: vec![(0, 0), (1, 1)],
            initial_soc_error: 0.05,
            rest_current_threshold_a: 0.1,
            rest_time_for_ocv_s: 5.0, // short, so corrections actually fire mid-run
            ocv_correction_gain: 0.5,
            min_ocv_slope_v_per_soc: 0.1,
        }),
        thermal: ThermalConfig::Isothermal,
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
        cell_model: CellModelConfig::Ecm,
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

/// A zero-length step must leave the entire engine state untouched.
///
/// This is a *contract*, not an accident. Every state advance scales by `dt` — the
/// exponential RC update guards on `dt > 0`, coulomb counting multiplies by it, the
/// thermal integrator scales its sub-step by it — so `step(0.0, …)` reports the pack
/// solved at the current state without moving it. `properties.rs`'s energy-balance
/// test relies on exactly that to read the start-of-step terminal voltage, which
/// `Telemetry` otherwise does not expose.
///
/// It is pinned here because it is easy to break invisibly: anything added to `step`
/// that mutates unconditionally would turn the probe into a real step, and the
/// energy-balance test would keep passing while silently measuring the wrong thing.
/// That is not hypothetical — the BMS *does* mutate on information rather than on
/// elapsed time (consuming a frame resets its rest timer and can fire an OCV
/// correction; sampling a new one draws noise), so `step` gates the whole sensor
/// clock on `dt > 0`. This config has a noisy BMS specifically to cover that gate.
#[test]
fn zero_length_step_does_not_mutate_state() {
    // A live thermal network, so the temperature integrator is on the path too —
    // its sub-step length is what scales by dt there.
    let mut cfg = config();
    cfg.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&cfg, rich_chem()).unwrap();
    // Warm up first: from a fresh pack many quantities are still zero, so a
    // mutation could hide. Mid-trajectory every RC overpotential is nonzero.
    for step in 0..37 {
        pack.step(0.5, demand_at(step), &env());
    }

    for demand in [
        Demand::Current(2.0),
        Demand::Current(-2.0),
        Demand::Power(4.0),
        Demand::Voltage(6.0),
        Demand::Rest,
    ] {
        let before = pack.snapshot();
        let tele = pack.step(0.0, demand, &env());
        let after = pack.snapshot();
        assert_eq!(before, after, "step(0.0, {demand:?}) mutated the pack");
        // And it still reports a solved operating point rather than nothing.
        assert!(
            tele.v_terminal.is_finite() && tele.i_actual.is_finite(),
            "probe step should still report a solve: {tele:?}"
        );
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
