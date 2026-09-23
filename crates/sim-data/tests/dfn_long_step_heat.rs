//! Heat at a long step on the porous-electrode (`Dfn`) cell, against the **shipped** LG M50.
//!
//! `docs/plans/dfn-end-of-step-heat.md`. The pack's heat estimate for a `Dfn` was
//! `i·(U_eq,start − v_node)` with `v_node` the step's last instant, so it booked the fall of
//! the equilibrium voltage across the step — stored energy leaving through the terminals —
//! as heat. Each case here compares one long step against the same interval in one-minute
//! steps, on the temperature the thermal network integrated and on the heat `q_gen_w`
//! reported.
//!
//! # The tolerance
//! Measured on the fixed engine (the plan note's table), the long step's temperature rise
//! sits within **5.4 %** of the one-minute run in every case here, and its reported heat
//! within **8.9 %** (both worst on the charge). The engine before the fix was off by
//! **+185 % to +322 %** on the rise and **+181 % to +338 %** on the heat. So the bounds are
//! about twice the fix's own error and far inside the defect's: 10 % on the rise, 20 % on
//! the heat.

use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

const DFN: CellModelConfig = CellModelConfig::Dfn {
    shells: 10,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};

const T0_K: f64 = 298.15;

fn env() -> Env {
    Env {
        t_ambient: T0_K,
        t_coolant: None,
    }
}

fn pack(parallel: u16, initial_soc: f64, sigma: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        series: 1,
        parallel,
        initial_soc,
        initial_temp_k: T0_K,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: sigma,
            r0_sigma: sigma,
        },
        cell_model: DFN,
    };
    Pack::new(
        &config,
        sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses"),
    )
    .expect("builds")
}

/// `(hottest cell's rise [K], reported heat summed over the interval [J])` after `total`
/// seconds at `amps` in steps of `dt`.
fn run(parallel: u16, soc: f64, sigma: f64, amps: f64, dt: f64, total: f64) -> (f64, f64) {
    let mut p = pack(parallel, soc, sigma);
    let steps = (total / dt).round() as usize;
    let (mut rise, mut heat) = (0.0, 0.0);
    for _ in 0..steps {
        let tele = p.step(dt, Demand::Current(amps), &env());
        rise = tele.t_max - T0_K;
        heat += tele.q_gen_w * dt;
    }
    (rise, heat)
}

fn assert_long_step_heats_like_short_ones(
    what: &str,
    parallel: u16,
    soc: f64,
    sigma: f64,
    amps: f64,
    total: f64,
) {
    let (rise_long, heat_long) = run(parallel, soc, sigma, amps, total, total);
    let (rise_short, heat_short) = run(parallel, soc, sigma, amps, 60.0, total);
    assert!(
        rise_short > 0.5,
        "{what}: the reference must heat measurably ({rise_short} K)"
    );
    let rise_err = rise_long / rise_short - 1.0;
    assert!(
        rise_err.abs() < 0.10,
        "{what}: one {total} s step rose {rise_long:.4} K against {rise_short:.4} K \
         in one-minute steps ({:+.1} %)",
        100.0 * rise_err
    );
    let heat_err = heat_long / heat_short - 1.0;
    assert!(
        heat_err.abs() < 0.20,
        "{what}: one {total} s step reported {heat_long:.1} J of heat against \
         {heat_short:.1} J in one-minute steps ({:+.1} %)",
        100.0 * heat_err
    );
}

/// C/5 for an hour from 90 %. Before the fix: 3.71 K against 1.12 K.
#[test]
fn an_hour_long_discharge_step_heats_like_sixty_short_ones() {
    assert_long_step_heats_like_short_ones("1S1P C/5", 1, 0.9, 0.0, 1.0, 3600.0);
}

/// 1C for half an hour from 90 %. Before the fix: 55.4 K against 19.5 K.
#[test]
fn a_half_hour_one_c_step_heats_like_thirty_short_ones() {
    assert_long_step_heats_like_short_ones("1S1P 1C", 1, 0.9, 0.0, 5.0, 1800.0);
}

/// C/2 charge for an hour from 20 %: the equilibrium voltage *rises* across a charging
/// step, and the charge current is negative, so the booked energy has the same sign as on
/// discharge. Before the fix: 27.4 K against 6.5 K.
#[test]
fn an_hour_long_charge_step_heats_like_sixty_short_ones() {
    assert_long_step_heats_like_short_ones("1S1P C/2 charge", 1, 0.2, 0.0, -2.5, 3600.0);
}

/// A scattered parallel group at C/5, where each cell's correction is read off its own
/// solve at its own branch current. Before the fix: 5.44 K against 1.64 K.
#[test]
fn a_scattered_group_heats_like_sixty_short_steps() {
    assert_long_step_heats_like_short_ones("1S3P σ=0.05 C/5", 3, 0.9, 0.05, 3.0, 3600.0);
}
