//! Phase 2 thermal-network tests.
//!
//! The load-bearing one is [`center_cells_run_hotter_than_corners`], the phase exit
//! gate. Just as important is its negative control,
//! [`conduction_alone_creates_no_gradient`]: a temperature gradient does **not**
//! come from interior cells having more conduction neighbours (that cancels
//! exactly), it comes from interior cells having less surface exposed to ambient.
//! Get that backwards and the gate silently measures nothing.
//!
//! Every chemistry here uses a single-breakpoint `R0` temperature axis, so `R0` is
//! temperature-independent and heating cannot feed back into the electrical solve.
//! That keeps the analytic steady-state check exact and isolates what these tests
//! are about.

use sim_core::chem::ThermalParams;
use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::thermal::{exposure, n_neighbors};
use sim_core::{Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;
const V0: f64 = 3.30;
const R0: f64 = 0.02;
const R_RC: f64 = 0.01;
const C_TH: f64 = 95.0;
const HA: f64 = 0.35;
const T_ENV: f64 = 298.15;

fn env() -> Env {
    Env {
        t_ambient: T_ENV,
        t_coolant: None,
    }
}

/// Flat-OCV, temperature-independent-`R0`, single-RC synthetic cell. `h_area` and
/// the optional entropy coefficient vary per test.
fn chem(h_area_w_per_k: f64, docv_dt_v_per_k: Option<Vec<f64>>) -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        meta: ChemMeta {
            id: "thermal-test".into(),
            name: "Thermal test cell".into(),
            provenance: "thermal test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 5.0,
            v_min: 0.0,
            max_charge_c: 20.0,
            max_discharge_c: 20.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 1.0],
            volts: vec![V0, V0],
            docv_dt_v_per_k,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: R_RC,
            c_farad: 2000.0, // tau = 20 s
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: C_TH,
            h_area_w_per_k,
        },
    }
}

fn config(series: u16, parallel: u16, soc0: f64, thermal: ThermalConfig) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        series,
        parallel,
        initial_soc: soc0,
        initial_temp_k: T_ENV,
        seed: 0,
        scatter: Scatter::default(),
        thermal,
    }
}

/// The grid geometry the thermal model reads off the electrical topology.
#[test]
fn grid_geometry_matches_topology() {
    // A lone cell has no neighbours and is fully exposed — which is what makes
    // `h_area_w_per_k` a bare-cell property.
    assert_eq!(n_neighbors(0, 0, 1, 1), 0);
    assert_eq!(exposure(0, 0, 1, 1), 1.0);

    // 5S5P: corner has 2 neighbours, edge 3, centre 4 (fully enclosed).
    assert_eq!(n_neighbors(0, 0, 5, 5), 2);
    assert_eq!(exposure(0, 0, 5, 5), 0.5);
    assert_eq!(n_neighbors(0, 2, 5, 5), 3);
    assert_eq!(exposure(0, 2, 5, 5), 0.25);
    assert_eq!(n_neighbors(2, 2, 5, 5), 4);
    assert_eq!(exposure(2, 2, 5, 5), 0.0);

    // A 1-D string (parallel = 1): interior cells have two neighbours, so they
    // keep half their ambient coupling — a chain still develops a gradient, just a
    // shallower one than a block.
    assert_eq!(n_neighbors(1, 0, 3, 1), 2);
    assert_eq!(exposure(1, 0, 3, 1), 0.5);
    assert_eq!(exposure(0, 0, 3, 1), 0.75);
}

/// Isothermal is a real mode, not a stub: temperatures never move, but heat
/// generation is still reported.
#[test]
fn isothermal_mode_holds_temperature_but_still_reports_heat() {
    let mut pack = Pack::new(
        &config(2, 2, 0.8, ThermalConfig::Isothermal),
        chem(HA, None),
    )
    .unwrap();
    let mut last = None;
    for _ in 0..500 {
        last = Some(pack.step(1.0, Demand::Current(4.0), &env()));
    }
    let tele = last.unwrap();
    assert_eq!(tele.t_min, T_ENV);
    assert_eq!(tele.t_max, T_ENV);
    for s in 0..2 {
        for p in 0..2 {
            assert_eq!(pack.cell(s, p).unwrap().temp_k, T_ENV);
        }
    }
    // 2S2P: each group carries the full 4 A, split two ways, so 2 A per cell. With
    // the RC pair settled that is I²·(R0 + R_rc) = 0.12 W each, over four cells.
    assert!(tele.q_gen_w > 0.0, "heat is reported: {}", tele.q_gen_w);
    let expected = 4.0 * 2.0 * 2.0 * (R0 + R_RC);
    assert!(
        (tele.q_gen_w - expected).abs() < 1e-6,
        "{} W, expected {expected} W",
        tele.q_gen_w
    );
}

/// **Negative control.** With `h_area = 0` every cell is adiabatic, so the only
/// coupling left is cell-to-cell conduction. A uniform pack then stays *exactly*
/// uniform no matter how many neighbours each cell has: `Σ_j k·(T_j − T_i)` is
/// identically zero when all temperatures agree.
///
/// This is why the gradient in the next test comes from position-dependent ambient
/// exposure and not from the conduction graph. If a future change makes conduction
/// itself produce a gradient, that is a bug, and this test is what catches it.
#[test]
fn conduction_alone_creates_no_gradient() {
    let mut pack = Pack::new(
        &config(
            3,
            3,
            0.9,
            ThermalConfig::Network {
                k_neighbor_w_per_k: 1.0,
            },
        ),
        chem(0.0, None), // adiabatic
    )
    .unwrap();
    // 3 A per cell ⇒ 0.27 W ⇒ with nowhere for the heat to go, 0.27·600/95 ≈ 1.7 K.
    for _ in 0..600 {
        pack.step(1.0, Demand::Current(9.0), &env());
    }
    let reference = pack.cell(1, 1).unwrap().temp_k;
    assert!(
        reference > T_ENV + 1.0,
        "pack should have heated: {reference}"
    );
    for s in 0..3 {
        for p in 0..3 {
            // Bit-exact: identical cells see identical heat and an identically zero
            // neighbour sum, whether they have 2, 3, or 4 neighbours.
            assert_eq!(
                pack.cell(s, p).unwrap().temp_k,
                reference,
                "cell {s},{p} diverged from a uniform field"
            );
        }
    }
}

/// **Phase 2 exit gate.** In a block with ambient coupling, the middle runs hotter:
/// an interior cell's surface is blocked by its neighbours, so its heat has to
/// conduct outward through the stack to escape.
#[test]
fn center_cells_run_hotter_than_corners() {
    let mut pack = Pack::new(
        &config(
            5,
            5,
            0.9,
            ThermalConfig::Network {
                k_neighbor_w_per_k: 1.0,
            },
        ),
        chem(HA, None),
    )
    .unwrap();
    // 25 A over five parallel cells per group = 5 A per cell ≈ 0.5 W each.
    let mut tele = None;
    for _ in 0..1800 {
        tele = Some(pack.step(1.0, Demand::Current(25.0), &env()));
    }
    let tele = tele.unwrap();

    let center = pack.cell(2, 2).unwrap().temp_k;
    let edge = pack.cell(0, 2).unwrap().temp_k;
    let corner = pack.cell(0, 0).unwrap().temp_k;
    assert!(
        center > edge && edge > corner,
        "expected centre > edge > corner, got {center} / {edge} / {corner}"
    );
    assert!(
        center - corner > 1.0,
        "gradient should be clearly measurable, got {} K",
        center - corner
    );
    // Telemetry's extremes must be the actual extremes of the field.
    assert_eq!(tele.t_max, center);
    assert_eq!(tele.t_min, corner);
    // Everything stayed physical: heated above ambient, nowhere near runaway.
    assert!(corner > T_ENV, "even the coolest cell heats: {corner}");
    assert!(center < T_ENV + 100.0, "no runaway in this phase: {center}");
}

/// A lone (`1S1P`) cell is fully exposed, so its steady state is analytic:
/// `T = T_env + Q/hA` with `Q = I²·(R0 + R_rc)` once the RC pair has settled.
///
/// This pins both the exposure normalisation (a factor of 4 error here would show
/// up as a 4× temperature rise) and the integrator's fixed point.
#[test]
fn single_cell_reaches_analytic_steady_state() {
    let i = 2.0;
    let mut pack = Pack::new(
        &config(
            1,
            1,
            0.95,
            ThermalConfig::Network {
                k_neighbor_w_per_k: 0.0,
            },
        ),
        chem(HA, None),
    )
    .unwrap();
    // 3000 s ≈ 11 thermal time constants (C/hA = 271 s), and 150 RC time
    // constants, so both transients are done.
    for _ in 0..3000 {
        pack.step(1.0, Demand::Current(i), &env());
    }
    let q_ss = i * i * (R0 + R_RC);
    let expected = T_ENV + q_ss / HA;
    let got = pack.cell(0, 0).unwrap().temp_k;
    assert!(
        (got - expected).abs() < 1e-4,
        "steady state {got} K, expected {expected} K"
    );
}

/// The thermal integrator conserves energy: over one step, the heat that entered
/// the cells equals what was generated minus what escaped to ambient. Conduction
/// contributes exactly zero to the total because `k_ij = k_ji`, so any asymmetry
/// introduced into the neighbour loop breaks this identity.
///
/// The step is taken with `dt` small enough for a single thermal sub-step (the
/// ceiling here is ≈ 11.9 s), which is what makes the check exact rather than
/// approximate: with sub-steps the convective term would use intermediate
/// temperatures rather than the start-of-step ones read here.
#[test]
fn thermal_integrator_conserves_energy() {
    let (series, parallel) = (3usize, 3usize);
    let mut cfg = config(
        series as u16,
        parallel as u16,
        0.9,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
    );
    // Scatter makes the cells generate unequal heat, so the field is genuinely
    // non-uniform and the conduction terms are individually nonzero.
    cfg.scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.10,
    };
    cfg.seed = 0x5EED;
    let mut pack = Pack::new(&cfg, chem(HA, None)).unwrap();
    for _ in 0..400 {
        pack.step(1.0, Demand::Current(9.0), &env());
    }

    let before: Vec<f64> = (0..series)
        .flat_map(|s| (0..parallel).map(move |p| (s, p)))
        .map(|(s, p)| pack.cell(s, p).unwrap().temp_k)
        .collect();
    let spread = before
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(0.0)
        - before.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(
        spread > 0.01,
        "field must be non-uniform for this test to bite, spread = {spread} K"
    );

    let dt = 1.0;
    let tele = pack.step(dt, Demand::Current(9.0), &env());

    let mut stored = 0.0; // Σ C·ΔT
    let mut lost = 0.0; // Σ hA_i·(T_i − T_env)
    for s in 0..series {
        for p in 0..parallel {
            let i = s * parallel + p;
            let after = pack.cell(s, p).unwrap().temp_k;
            stored += C_TH * (after - before[i]);
            lost += exposure(s, p, series, parallel) * HA * (before[i] - T_ENV);
        }
    }
    let expected = dt * (tele.q_gen_w - lost);
    assert!(
        (stored - expected).abs() < 1e-9 * stored.abs().max(1e-6),
        "stored {stored} J, generated − lost {expected} J"
    );
}

/// Sub-stepping keeps a coarse `dt` from distorting the thermal trajectory.
///
/// The comparison is made during a **rest** phase: with identical cells at rest the
/// pack current is exactly zero, so no heat is generated and SOC does not move —
/// the two runs differ *only* in how finely the thermal relaxation is integrated.
/// Without sub-stepping the coarse run would not merely be inaccurate, it would
/// oscillate: `dt·a/C` is 1000·4/95 ≈ 42, far past the stability bound of 2.
#[test]
fn substepping_keeps_a_coarse_dt_close_to_a_fine_one() {
    let thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let build = || Pack::new(&config(3, 3, 0.9, thermal), chem(HA, None)).unwrap();
    let mut coarse = build();
    let mut fine = build();

    // Identical warm-up builds a gradient in both packs.
    for _ in 0..600 {
        coarse.step(1.0, Demand::Current(9.0), &env());
        fine.step(1.0, Demand::Current(9.0), &env());
    }
    let hot = coarse.cell(1, 1).unwrap().temp_k;
    assert!(hot > T_ENV + 1.0, "warm-up should heat the pack: {hot}");
    for s in 0..3 {
        for p in 0..3 {
            assert_eq!(
                coarse.cell(s, p).unwrap().temp_k,
                fine.cell(s, p).unwrap().temp_k
            );
        }
    }

    // Same 1000 s of rest, integrated as one coarse step vs 1000 fine ones.
    coarse.step(1000.0, Demand::Rest, &env());
    for _ in 0..1000 {
        fine.step(1.0, Demand::Rest, &env());
    }

    for s in 0..3 {
        for p in 0..3 {
            let c = coarse.cell(s, p).unwrap().temp_k;
            let f = fine.cell(s, p).unwrap().temp_k;
            assert!(c.is_finite(), "coarse step diverged at {s},{p}: {c}");
            // Both relax toward ambient; compare the remaining excursion, which is
            // the quantity that actually carries information here.
            let excursion_c = c - T_ENV;
            let excursion_f = f - T_ENV;
            assert!(
                (excursion_c - excursion_f).abs() <= 0.02 * excursion_f.abs().max(1e-3),
                "cell {s},{p}: coarse {c} vs fine {f}"
            );
        }
    }
}

/// The entropic term has the sign Bernardi's balance gives it:
/// `Q_rev = −I·T·∂U/∂T`. With the usual negative `∂U/∂T`, discharging adds heat and
/// charging removes it. Both shipped chemistries omit the column, so without this
/// test a sign error would sit undetected.
#[test]
fn entropic_term_heats_on_discharge_and_cools_on_charge() {
    // −0.2 mV/K, constant across SOC — within the range measured for real Li-ion
    // cells, and large enough here that charge is net endothermic (see below).
    let docv = -2.0e-4;
    let t0 = 300.0;
    let i = 2.0;

    // Isothermal so the reported heat is evaluated at exactly `t0`, and one step
    // from a fresh pack so both variants see bit-identical electrical state.
    let mut cfg = config(1, 1, 0.5, ThermalConfig::Isothermal);
    cfg.initial_temp_k = t0;

    let plain = |demand| {
        let mut pack = Pack::new(&cfg, chem(HA, None)).unwrap();
        pack.step(1.0, demand, &env()).q_gen_w
    };
    let entropic = |demand| {
        let mut pack = Pack::new(&cfg, chem(HA, Some(vec![docv, docv]))).unwrap();
        pack.step(1.0, demand, &env()).q_gen_w
    };

    // Discharge: Q_rev = −I·T·∂U/∂T = +0.06 W of extra heating.
    let expected_rev = -i * t0 * docv;
    assert!(expected_rev > 0.0);
    let d_plain = plain(Demand::Current(i));
    let d_entropic = entropic(Demand::Current(i));
    assert!(
        (d_entropic - d_plain - expected_rev).abs() < 1e-12,
        "discharge: {d_entropic} − {d_plain} should be {expected_rev}"
    );

    // Charge: the same term flips sign and cools. On this first step out of rest
    // the overpotential heat is only I²·R0 = 0.08 W against 0.12 W of entropic
    // cooling, so the cell is a net heat *sink* — physical, and the reason
    // `q_gen_w` is documented as possibly negative.
    let c_plain = plain(Demand::Current(-i));
    let c_entropic = entropic(Demand::Current(-i));
    assert!(
        (c_entropic - c_plain + expected_rev).abs() < 1e-12,
        "charge: {c_entropic} − {c_plain} should be {}",
        -expected_rev
    );
    assert!(
        c_entropic < 0.0,
        "endothermic charge should be a net sink here: {c_entropic}"
    );
}

/// Coolant, when supplied, replaces ambient as the sink the cells exchange heat
/// with — so a chilled coolant pulls a heated pack down below ambient.
#[test]
fn coolant_replaces_ambient_as_the_sink() {
    let thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&config(2, 2, 0.9, thermal), chem(HA, None)).unwrap();
    let chilled = Env {
        t_ambient: T_ENV,
        t_coolant: Some(T_ENV - 20.0),
    };
    for _ in 0..2000 {
        pack.step(1.0, Demand::Rest, &chilled);
    }
    let t = pack.cell(0, 0).unwrap().temp_k;
    assert!(
        t < T_ENV - 15.0,
        "cells should track the coolant, not ambient: {t}"
    );
}
