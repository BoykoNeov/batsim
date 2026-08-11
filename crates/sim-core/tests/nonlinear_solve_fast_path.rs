//! The nonlinear pack solve's **fast path** (Phase 6 slice D).
//!
//! Slice D turned `Pack::step`'s closed-form current solve into a loop that re-takes
//! every cell's Thévenin tangent at the current the previous pass assigned it. For a
//! pack whose cells are all linear the tangent is exact, so the loop must exit having
//! done one pass — the arithmetic Phase 1 shipped, unchanged.
//!
//! # Why the assertion is on the *count* and not on the answer
//! An all-equivalent-circuit pack that reached the right voltage after three passes
//! would pass every physics assertion in this repo, and the only thing separating
//! that from a working fast path is `Telemetry::solve_iterations`. The plan says so
//! outright: "an answer that matches after three iterations means the fast path is
//! gone and only the tolerance is hiding it." So this file asserts `== 1`, on every
//! demand variant and against every feature that touches the solve — protection,
//! balancing, both short types, scatter, series and parallel topology — because each
//! of those is a way the pack aggregate can change and therefore a way a future
//! change could reintroduce a second pass.
//!
//! The complementary claim, that the answer the one pass produces is *bit-for-bit*
//! what the pre-slice-D build produced, is not testable in this repo: every bit
//! comparison here is between two runs of one build. It is carried by the
//! out-of-tree cross-build baseline (see `docs/plans/phase-6-porous-electrodes.md`),
//! which diffs empty for this slice.
//!
//! The nonlinear half — that the loop converges when it *does* engage — is in
//! `sim-data/tests/nonlinear_solve.rs`, against a shipped parameter set.

use sim_core::bms::{BalancingConfig, BmsConfig, ProtectionConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Fault, Pack, PackConfig, Scatter, ThermalConfig,
};

const CAP_AH: f64 = 2.5;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// An ordinary sloped-OCV equivalent-circuit cell. Nothing here is about the
/// chemistry — it exists so that `Demand::Voltage` and `Demand::Power` have a real
/// curve to solve against.
fn chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "fast_path".into(),
            name: "Fast-path test cell".into(),
            provenance: "solver test fixture — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.50,
            v_min: 2.90,
            max_charge_c: 1.0,
            max_discharge_c: 2.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![2.60, 3.20, 3.60],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

fn bms() -> BmsConfig {
    BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: 33.0,
            // Low enough that the bleed switches actually close on this pack, so the
            // group conductance the solve aggregates really does gain a term.
            v_threshold_v: 3.10,
            v_release_band_v: 0.010,
        }),
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.50,
            t_hard_margin_k: 20.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 1.0,
        min_ocv_slope_v_per_soc: 0.5,
    }
}

fn config(series: u16, parallel: u16, scatter: Scatter, bms: Option<BmsConfig>) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc: 0.8,
        initial_temp_k: 298.15,
        seed: 7,
        scatter,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 0.5,
        },
        bms,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    }
}

/// The demand schedule every case below runs: both signs, a rest, both derived
/// variants, and a probe step. `Power` and `Voltage` matter most — they are the two
/// that reach `solve_current`'s non-trivial arms, and `Power`'s quadratic is the one
/// place a moving tangent could flip a branch.
///
/// # Why the voltage targets are scaled by `series`
/// [`Demand::Voltage`] is a **pack terminal** voltage, and `Pack::step` clamps it to
/// `series × [v_min, v_max]` — here `[2.90, 3.50]` per cell, so `[11.60, 14.00]` on the
/// 4S packs five of the six cases below use. Written as bare `3.30`/`3.05` these targets
/// sat far *below* that window, and what they actually asked a 4S3P pack resting near
/// 13.76 V for was a 392 A / 26 C discharge — legal, but not a voltage hold, and under
/// the clamp it would have become the window edge rather than a number this file chose.
/// Scaling keeps each case asking for the same thing *per cell* that the 1S1P case asks,
/// which is what the schedule was always reaching for.
///
/// The assertion is untouched by this: it is about how many passes the solve takes, and
/// a linear pack's aggregate is exact at any operating point.
fn schedule(series: u16) -> Vec<(f64, Demand)> {
    let s = f64::from(series);
    vec![
        (1.0, Demand::Current(2.5)),
        (1.0, Demand::Current(-2.0)),
        (0.0, Demand::Current(5.0)), // probe step: no time passes
        (1.0, Demand::Rest),
        (1.0, Demand::Power(8.0)),
        (1.0, Demand::Power(-6.0)),
        (1.0, Demand::Voltage(3.30 * s)),
        (1.0, Demand::Voltage(3.05 * s)),
        (0.25, Demand::Current(12.0)), // hard enough that protection derates it
    ]
}

/// Run `schedule` against `pack` and assert every step took exactly one pass.
fn assert_single_pass(pack: &mut Pack, case: &str) {
    let series = pack.series();
    for (n, (dt, demand)) in schedule(series).into_iter().enumerate() {
        let tele = pack.step(dt, demand, &env());
        assert_eq!(
            tele.solve_iterations, 1,
            "{case}: step {n} ({demand:?}) took {} solver passes; a linear pack's \
             aggregated Thévenin is exact, so one pass is the whole answer and a \
             second means the closed-form fast path is gone",
            tele.solve_iterations
        );
        assert!(
            !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "{case}: step {n} reported a non-converged solve, which a linear pack \
             cannot reach — it never evaluates a residual at all"
        );
    }
}

// ---------------------------------------------------------------------------
// One pass, whatever the pack is made of
// ---------------------------------------------------------------------------

#[test]
fn a_single_cell_solves_in_one_pass() {
    let mut pack = Pack::new(&config(1, 1, Scatter::default(), None), chem()).unwrap();
    assert_single_pass(&mut pack, "1S1P, bare");
}

#[test]
fn a_series_parallel_pack_solves_in_one_pass() {
    let mut pack = Pack::new(&config(4, 3, Scatter::default(), None), chem()).unwrap();
    assert_single_pass(&mut pack, "4S3P, bare");
}

/// Scatter is what makes the per-cell currents inside a group actually differ, so
/// this is the case where a naive convergence check on "did the currents move"
/// would have something to chew on.
#[test]
fn a_scattered_pack_solves_in_one_pass() {
    let scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.10,
    };
    let mut pack = Pack::new(&config(4, 3, scatter, None), chem()).unwrap();
    assert_single_pass(&mut pack, "4S3P, scattered");
}

/// Protection sits *inside* the solve loop, so a pack that derates is the case where
/// a second pass would be most tempting: the clamped current is a different
/// operating point from the one the sources were built at. For a linear cell it is
/// the same straight line, which is exactly why one pass still suffices.
#[test]
fn a_protected_and_balancing_pack_solves_in_one_pass() {
    let mut pack = Pack::new(&config(4, 3, Scatter::default(), Some(bms())), chem()).unwrap();
    assert_single_pass(&mut pack, "4S3P, BMS on");
}

/// Both short types change the group or pack aggregate — the internal one as a
/// conductance on the node, the external one as a transform of the pack Thévenin.
#[test]
fn a_shorted_pack_solves_in_one_pass() {
    let mut pack = Pack::new(&config(4, 3, Scatter::default(), Some(bms())), chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 1,
            p: 0,
            ohms: 5.0,
        },
    )
    .expect("the cell exists");
    pack.schedule_fault(2.0, Fault::ExternalShort { ohms: 2.0 })
        .expect("an external short needs no cell index");
    assert_single_pass(&mut pack, "4S3P, both shorts");
}

/// A latched-open contactor is the one path that leaves the loop with `i_g` forced
/// to zero. It is deliberately not a shortcut out of the iteration — an imbalanced
/// group still redistributes internally at zero pack current — so it gets the same
/// assertion as every other case.
#[test]
fn an_open_contactor_still_solves_in_one_pass() {
    let scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.10,
    };
    // A zero hard margin, so the one unclamped step every excursion gets (the BMS
    // acts on the previous step's frame) is enough to latch. With the generous
    // margin the other cases use, the derate path holds the pack just under `v_max`
    // forever and the contactor never opens — which is the correct behaviour there
    // and useless here.
    let latching = BmsConfig {
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.0,
            t_hard_margin_k: 20.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
        }),
        ..bms()
    };
    let mut pack = Pack::new(&config(4, 3, scatter, Some(latching)), chem()).unwrap();
    // Charge until the contactor latches, then keep stepping against an open pack.
    for _ in 0..2000 {
        if pack
            .step(1.0, Demand::Current(-12.0), &env())
            .flags
            .contains(EventFlags::CONTACTOR_OPEN)
        {
            break;
        }
    }
    let tele = pack.step(1.0, Demand::Current(-12.0), &env());
    assert!(
        tele.flags.contains(EventFlags::CONTACTOR_OPEN),
        "the setup did not actually latch the contactor, so the case under test \
         never happened: flags were {:?}",
        tele.flags
    );
    assert_single_pass(&mut pack, "4S3P, contactor open");
}
