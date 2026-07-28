//! Perf item 3: the memoised per-cell Thévenin source must change nothing.
//!
//! `Pack` carries each cell's start-of-step `(E, R)` across the step boundary —
//! the end-of-step reporting pass computes exactly what the next step's
//! aggregation would recompute, so the next step reads it instead (see
//! `docs/plans/pack-step-perf.md`). That is only sound if a warm memo is
//! **bit-for-bit** what a cold recompute produces, on every cell, on every step.
//!
//! Tolerance-based tests cannot pin that. The goldens and the proptests assert
//! within an epsilon, so a one-ULP divergence between the warm and cold paths
//! passes them all while still breaking the snapshot-replay guarantee on some
//! other trajectory. So this file runs the *same* trajectory twice — once warm,
//! once with the memo dropped before every single step — and compares raw bits.
//!
//! Two things are compared, and the second is not redundant: `Telemetry` reports
//! aggregates (`soc_true` is a capacity-weighted mean, `v_terminal` a sum over
//! groups), so a divergence in one cell could in principle cancel in the total.
//! [`Pack::cell`] exposes the per-cell ground truth, which cannot cancel.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    BalancingConfig, BmsConfig, Demand, Env, Pack, PackConfig, ProtectionConfig, Scatter,
    Telemetry, ThermalConfig,
};

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Sloped OCV, a temperature-varying `R0` grid, two RC pairs, and an entropy
/// coefficient — every table the memoised `cell_source` touches is non-flat, so a
/// divergence has somewhere to show up.
fn rich_chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        spm: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "rich".into(),
            name: "Rich synthetic cell".into(),
            provenance: "cache-equivalence test — not physical".into(),
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
            docv_dt_v_per_k: Some(vec![-1.0e-4, -0.8e-4, -0.5e-4, -0.2e-4, 0.3e-4]),
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

/// A live thermal network *and* a full BMS. The thermal integrator is the one thing
/// inside `step` that mutates a cell after the electrical solve, so it is the memo's
/// sharpest hazard: it writes `temp_k`, and `temp_k` is an input to `R0`. If the
/// reporting pass ever ran before the temperature update, this config is what would
/// catch it — a memo written from pre-update temperatures would diverge on the very
/// next step.
fn config() -> PackConfig {
    PackConfig {
        aging: None,
        bms: Some(BmsConfig {
            balancing: Some(BalancingConfig {
                bleed_r_ohms: 47.0,
                v_threshold_v: 3.0, // below the resting voltage here, so bleeds close
            }),
            protection: Some(ProtectionConfig {
                v_hard_margin_v: 0.2,
                t_hard_margin_k: 10.0,
            }),
            current_offset_a: 0.01,
            current_noise_sigma_a: 0.05,
            temp_probes: vec![(0, 0), (2, 1)],
            initial_soc_error: 0.05,
            rest_current_threshold_a: 0.1,
            rest_time_for_ocv_s: 5.0,
            ocv_correction_gain: 0.5,
            min_ocv_slope_v_per_soc: 0.1,
        }),
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        series: 3,
        parallel: 3,
        initial_soc: 0.7,
        initial_temp_k: 298.15,
        seed: 0xCAFE_F00D,
        scatter: Scatter {
            capacity_sigma: 0.03,
            r0_sigma: 0.05,
        },
    }
}

/// Discharge, rest, charge, power discharge — the memo has to survive a current
/// reversal (where an RC overpotential is still relaxing) and a rest (where the BMS
/// may fire an OCV correction), not just a steady drain.
///
/// Deliberately biased toward discharge: a schedule that nets to zero leaves SOC and
/// temperature parked at their initial values, so the `R0` grid never leaves one
/// bilinear cell and 200 bit-comparisons say very little. The end-of-test
/// assertions pin that the sweep actually happened.
fn demand_at(step: usize) -> Demand {
    match step % 40 {
        0..=24 => Demand::Current(20.0),   // ≈2.7C discharge
        25..=29 => Demand::Rest,           // relax
        30..=34 => Demand::Current(-15.0), // ≈2C charge, at the protection limit
        _ => Demand::Power(60.0),          // power discharge
    }
}

/// Drop the memo without touching a single physical quantity.
///
/// [`Pack::set_cell_factors`] is the invalidation point, and re-setting a cell's
/// factors to the values it already has is an exact no-op *because* the clamp it
/// applies (`max(MIN_FACTOR)`, `MIN_FACTOR = 0.05`) is idempotent on any factor
/// already at or above the floor — which every scatter draw is, by construction.
/// So this is not a "roughly equivalent" pack: it is the same bits, minus the memo.
/// Do not simplify the call away as a no-op; being a no-op is the point.
fn force_cold(pack: &mut Pack, series: u16, parallel: u16) {
    for s in 0..series as usize {
        for p in 0..parallel as usize {
            let view = pack.cell(s, p).expect("index in range");
            pack.set_cell_factors(s, p, view.capacity_factor, view.r0_factor)
                .expect("index in range");
        }
    }
}

/// Every `f64` a step produces, as raw bits — `PartialEq` on `f64` is exact for
/// finite values, but going through `to_bits` also makes a NaN mismatch loud rather
/// than vacuously equal-or-not.
fn tele_bits(t: &Telemetry) -> Vec<u64> {
    vec![
        t.v_terminal.to_bits(),
        t.i_actual.to_bits(),
        t.soc_true.to_bits(),
        t.soc_bms.unwrap_or(f64::NAN).to_bits(),
        t.t_min.to_bits(),
        t.t_max.to_bits(),
        t.v_cell_min.to_bits(),
        t.v_cell_max.to_bits(),
        t.q_gen_w.to_bits(),
        t.q_runaway_w.to_bits(),
        t.q_balancing_w.to_bits(),
        t.i_balancing_a.to_bits(),
        t.i_internal_short_a.to_bits(),
        t.i_external_short_a.to_bits(),
        t.flags.bits().into(),
    ]
}

fn cell_bits(pack: &Pack, series: u16, parallel: u16) -> Vec<u64> {
    let mut out = Vec::new();
    for s in 0..series as usize {
        for p in 0..parallel as usize {
            let v = pack.cell(s, p).expect("index in range");
            out.extend([
                v.soc.to_bits(),
                v.temp_k.to_bits(),
                v.v_rc_sum.to_bits(),
                v.capacity_factor.to_bits(),
                v.r0_factor.to_bits(),
                v.soh_capacity.to_bits(),
                v.soh_resistance.to_bits(),
                v.internal_short_conductance_s.to_bits(),
                v.runaway_energy_remaining_j.to_bits(),
                u64::from(v.vented),
            ]);
        }
    }
    out
}

#[test]
fn warm_and_cold_thevenin_paths_are_bit_identical() {
    const STEPS: usize = 200;
    let dt = 2.0;
    let cfg = config();
    let (series, parallel) = (cfg.series, cfg.parallel);

    // A: the ordinary trajectory — memo warm from step 1 onward.
    let mut warm = Pack::new(&cfg, rich_chem()).unwrap();
    // B: the same trajectory with the memo dropped before *every* step, so every
    // step takes the cold recompute path. Once at the start would only prove the
    // first step matches.
    let mut cold = Pack::new(&cfg, rich_chem()).unwrap();

    for step in 0..STEPS {
        let demand = demand_at(step);
        let a = warm.step(dt, demand, &env());
        force_cold(&mut cold, series, parallel);
        let b = cold.step(dt, demand, &env());

        assert_eq!(
            tele_bits(&a),
            tele_bits(&b),
            "telemetry diverged at step {step} ({demand:?}): warm {a:?} vs cold {b:?}"
        );
        // Aggregates can cancel a single-cell divergence; ground truth cannot.
        assert_eq!(
            cell_bits(&warm, series, parallel),
            cell_bits(&cold, series, parallel),
            "per-cell ground truth diverged at step {step} ({demand:?})"
        );
    }

    // The trajectory has to have actually gone somewhere, or the comparison above is
    // 200 assertions about a pack sitting still.
    let end = warm.cell(0, 0).expect("index in range");
    assert!(
        (end.soc - cfg.initial_soc).abs() > 0.05,
        "trajectory barely moved (soc {} → {}); the test is not exercising anything",
        cfg.initial_soc,
        end.soc
    );
    assert!(
        end.temp_k > cfg.initial_temp_k + 0.1,
        "cells never heated (temp {} → {}); R0's temperature axis went unexercised",
        cfg.initial_temp_k,
        end.temp_k
    );
}

/// A zero-length probe step must not disturb the memo either.
///
/// `step(0.0, …)` is contractually a pure observation, but it still runs the whole
/// electrical solve and the whole reporting pass — so it rewrites the memo. Since
/// `SourceCache`'s `PartialEq` is deliberately always-true, the snapshot equality
/// assertion in `snapshot.rs` cannot see a probe step corrupting it; this can.
#[test]
fn probe_step_leaves_the_memo_usable() {
    let cfg = config();
    let (series, parallel) = (cfg.series, cfg.parallel);
    let mut warm = Pack::new(&cfg, rich_chem()).unwrap();
    let mut cold = Pack::new(&cfg, rich_chem()).unwrap();

    for step in 0..20 {
        warm.step(0.5, demand_at(step), &env());
        force_cold(&mut cold, series, parallel);
        cold.step(0.5, demand_at(step), &env());
    }

    // Probe the warm pack, then step it again: the step after the probe is the one
    // that would break if the probe had left a stale memo behind.
    warm.step(0.0, Demand::Current(4.0), &env());
    let a = warm.step(0.5, Demand::Current(4.0), &env());

    force_cold(&mut cold, series, parallel);
    cold.step(0.0, Demand::Current(4.0), &env());
    force_cold(&mut cold, series, parallel);
    let b = cold.step(0.5, Demand::Current(4.0), &env());

    assert_eq!(
        tele_bits(&a),
        tele_bits(&b),
        "probe step corrupted the memo"
    );
    assert_eq!(
        cell_bits(&warm, series, parallel),
        cell_bits(&cold, series, parallel),
        "probe step corrupted the memo (per-cell)"
    );
}
