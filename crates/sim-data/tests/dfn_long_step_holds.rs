//! Voltage and power demands held over a long step on the porous-electrode (`Dfn`) cell,
//! against the **shipped** LG M50.
//!
//! `docs/plans/dfn-long-step-holds.md`. Before it, 269 of 405 one-hour voltage holds across
//! the window finished unconverged from a fresh pack, and they were not bounded: single steps
//! reached 1e179 K, and a scattered 1S3P held at 3.3 V ran to 3e146 A on its second hour. Three
//! things were wrong, and each test here answers to one of them:
//!
//! * the cell's own Newton failed on long steps well inside the range it can carry — from
//!   98 % at 4 A, and at every current from a cell just drained — which the two retries in
//!   `dfn::solve` fix;
//! * the pack committed cells to currents their step could not converge on, which holding a
//!   voltage or power demand's current to the cells' range fixes;
//! * the pack's damped search could stop on a pass that had not met the demand and call it
//!   converged, which the held-pass rules fix.
//!
//! "Conserves" below is the one check that sees a bad step whatever its flags say: a
//! converged step moves exactly `i·dt` of charge, so a cell's state of charge falls by
//! `i·dt / Q`. `Q` is measured, not assumed — one converged hour at 1 A.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

const DFN: CellModelConfig = CellModelConfig::Dfn {
    shells: 10,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};

const T0_K: f64 = 298.15;
const HOUR_S: f64 = 3600.0;

fn env() -> Env {
    Env {
        t_ambient: T0_K,
        t_coolant: None,
    }
}

fn pack(series: u16, parallel: u16, initial_soc: f64, sigma: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
        series,
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

/// The charge \[A·h\] one unit of state of charge holds on an unscattered cell, from one
/// converged hour at 1 A.
fn soc_capacity_ah() -> f64 {
    let mut p = pack(1, 1, 0.5, 0.0);
    let tele = p.step(HOUR_S, Demand::Current(1.0), &env());
    assert!(!tele.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    1.0 / (0.5 - p.cell(0, 0).expect("cell").soc)
}

fn unconverged(flags: EventFlags) -> bool {
    flags.contains(EventFlags::SOLVE_UNCONVERGED)
}

/// Every cell's state of charge before a step.
fn socs(p: &Pack, series: u16, parallel: u16) -> Vec<f64> {
    (0..usize::from(series))
        .flat_map(|g| (0..usize::from(parallel)).map(move |k| (g, k)))
        .map(|(g, k)| p.cell(g, k).expect("cell").soc)
        .collect()
}

/// Every cell whose state of charge is off its clamps before and after moved by exactly the
/// charge its reported current carried over `dt`.
fn assert_conserves(what: &str, p: &Pack, before: &[f64], series: u16, parallel: u16, dt: f64) {
    let q_ah = soc_capacity_ah();
    let mut idx = 0;
    for g in 0..usize::from(series) {
        for k in 0..usize::from(parallel) {
            let cell = p.cell(g, k).expect("cell");
            let b = before[idx];
            idx += 1;
            let i = cell.current_a.expect("a stepped pack reports its split");
            let inside = |x: f64| x > 1e-9 && x < 1.0 - 1e-9;
            if inside(b) && inside(cell.soc) {
                let moved_ah = (b - cell.soc) * q_ah * cell.capacity_factor;
                let carried_ah = i * dt / HOUR_S;
                assert!(
                    (moved_ah - carried_ah).abs() < 1e-9,
                    "{what}: cell {g}S{k}P carried {carried_ah} A·h but its charge moved \
                     {moved_ah} A·h — a step that did not conserve lithium"
                );
            }
        }
    }
}

/// The one-hour hold the pack used to stop on a damped pass: "converged" at 4.127 V,
/// charging at 1.56 A, for a 3.393 V target.
#[test]
fn a_one_hour_voltage_hold_meets_its_target() {
    let mut p = pack(1, 1, 0.5, 0.0);
    let before = socs(&p, 1, 1);
    let tele = p.step(HOUR_S, Demand::Voltage(3.393), &env());
    assert!(!unconverged(tele.flags), "flags {:?}", tele.flags);
    assert!(
        (tele.v_terminal - 3.393).abs() < 1e-6,
        "held at {} V, asked 3.393 V",
        tele.v_terminal
    );
    assert!(
        tele.i_actual > 0.0,
        "a hold below the resting voltage discharges"
    );
    assert_conserves("1S1P 3.393 V", &p, &before, 1, 1, HOUR_S);
}

/// The same fault at a one-second step: six holds from 98 % "converged" up to 45 mV off.
#[test]
fn a_one_second_voltage_hold_meets_its_target() {
    for target in [2.5, 3.07375, 3.095] {
        let mut p = pack(1, 1, 0.98, 0.0);
        let tele = p.step(1.0, Demand::Voltage(target), &env());
        assert!(
            !unconverged(tele.flags),
            "{target} V: flags {:?}",
            tele.flags
        );
        assert!(
            (tele.v_terminal - target).abs() < 1e-6,
            "held at {} V, asked {target} V",
            tele.v_terminal
        );
    }
}

/// The cell's own Newton, well inside its range: a 0.8C hour from 98 % ends near 20 %.
#[test]
fn a_long_discharge_from_full_converges() {
    let mut p = pack(1, 1, 0.98, 0.0);
    let before = socs(&p, 1, 1);
    let tele = p.step(HOUR_S, Demand::Current(4.0), &env());
    assert!(!unconverged(tele.flags), "flags {:?}", tele.flags);
    assert_conserves("1S1P 4 A from 98 %", &p, &before, 1, 1, HOUR_S);
}

/// A cell drained to 2.5 V in an hour, then left an hour: every current failed from here,
/// rest included, while a one-minute step converged.
#[test]
fn a_just_drained_cell_can_rest_for_an_hour() {
    let mut p = pack(1, 1, 0.98, 0.05);
    p.step(HOUR_S, Demand::Voltage(2.5), &env());
    let before = socs(&p, 1, 1);
    let tele = p.step(HOUR_S, Demand::Rest, &env());
    assert!(!unconverged(tele.flags), "flags {:?}", tele.flags);
    assert!(
        tele.v_terminal > 2.5 && tele.v_terminal < 3.5,
        "a drained cell relaxes a little above its cut-off, not to {} V",
        tele.v_terminal
    );
    assert_conserves("drained, resting", &p, &before, 1, 1, HOUR_S);
}

/// Four consecutive held hours on parallel packs, each ending near a limit and starting the
/// next from there. The scattered 1S3P at 3.3 V is the case that reached 3e146 A.
#[test]
fn parallel_packs_hold_a_voltage_hour_after_hour() {
    for (series, parallel, target) in [(1u16, 3u16, 3.3), (1, 3, 2.5), (1, 3, 4.15), (4, 2, 10.0)] {
        let mut p = pack(series, parallel, 0.5, 0.05);
        for hour in 0..4 {
            let before = socs(&p, series, parallel);
            let tele = p.step(HOUR_S, Demand::Voltage(target), &env());
            let what = format!("{series}S{parallel}P at {target} V, hour {hour}");
            assert!(
                tele.i_actual.is_finite() && tele.t_max < 340.0,
                "{what}: {tele:?}"
            );
            assert_conserves(&what, &p, &before, series, parallel, HOUR_S);
        }
    }
}

/// A power no cell can deliver for the hour stops where the chemistry calls the cell empty,
/// says it was not met, and stays there: no cell is ever driven past empty to find it.
#[test]
fn an_unreachable_power_stops_at_empty_and_says_so() {
    let q_ah = soc_capacity_ah();
    let mut p = pack(1, 1, 0.35, 0.0);
    let tele = p.step(HOUR_S, Demand::Power(10.0), &env());
    assert!(
        unconverged(tele.flags),
        "10 W for an hour from 35 % is not met"
    );
    // Held to the range, the current is at most the charge the cell had, and the search ends
    // next to that edge: measured 1.803597 of 1.803619 A·h, 0.0012 % short, where its passes
    // ran out. Never past it — that is the step the range exists to forbid.
    let delivered_ah = tele.i_actual * HOUR_S / HOUR_S;
    let held_ah = 0.35 * q_ah;
    assert!(
        delivered_ah <= held_ah * (1.0 + 1e-12) && delivered_ah >= held_ah * (1.0 - 1e-4),
        "delivered {delivered_ah} A·h of the {held_ah} A·h the cell held"
    );
    // Then the hours after: the scrap left over, and rest. Over all four the cell gives no
    // more than it held.
    let mut total_ah = delivered_ah;
    for hour in 1..4 {
        let tele = p.step(HOUR_S, Demand::Power(10.0), &env());
        assert!(unconverged(tele.flags), "hour {hour}: still not met");
        assert!(
            tele.i_actual >= 0.0 && tele.t_max < 300.0,
            "hour {hour}: an empty cell is not charged, or heated, by a discharge demand: \
             {} A at {} K",
            tele.i_actual,
            tele.t_max
        );
        total_ah += tele.i_actual * HOUR_S / HOUR_S;
    }
    assert!(
        total_ah <= held_ah * (1.0 + 1e-12),
        "four hours delivered {total_ah} A·h from a cell that held {held_ah} A·h"
    );
    // A scattered cell at empty: its own range then ends a rounding hair either side of
    // zero, and the pack's has to take zero in or the cell is pushed on past empty (0.5 mA,
    // `SOC_CLAMPED_LOW`, measured without that).
    let mut p1 = pack(1, 1, 0.35, 0.05);
    for hour in 0..4 {
        let tele = p1.step(HOUR_S, Demand::Power(10.0), &env());
        assert!(
            !tele.flags.contains(EventFlags::SOC_CLAMPED_LOW),
            "scattered 1S1P, hour {hour}: driven past empty ({} A, {:?})",
            tele.i_actual,
            tele.flags
        );
    }
    let mut p3 = pack(1, 3, 0.5, 0.05);
    for hour in 0..4 {
        let before = socs(&p3, 1, 3);
        let tele = p3.step(HOUR_S, Demand::Power(40.0), &env());
        let what = format!("1S3P 40 W, hour {hour}");
        assert!(unconverged(tele.flags), "{what}: not met");
        assert!(
            tele.i_actual.is_finite() && tele.t_max < 320.0,
            "{what}: {tele:?}"
        );
        assert_conserves(&what, &p3, &before, 1, 3, HOUR_S);
    }
}

/// Hour-long holds across the window from a scattered 1S3P, a subset of the note's sweep of
/// 405: every one converges. Before, 57 of the full 81 on this pack did not.
#[test]
fn hour_long_holds_across_the_window_converge() {
    let mut failed = Vec::new();
    for k in (0..81).step_by(8) {
        let target = 2.5 + (4.2 - 2.5) * f64::from(k) / 80.0;
        let mut p = pack(1, 3, 0.5, 0.05);
        let tele = p.step(HOUR_S, Demand::Voltage(target), &env());
        if unconverged(tele.flags) || (tele.v_terminal - target).abs() > 1e-6 {
            failed.push(format!("{target:.4} V -> {:.4} V", tele.v_terminal));
        }
    }
    assert!(failed.is_empty(), "unmet holds: {failed:?}");
}
