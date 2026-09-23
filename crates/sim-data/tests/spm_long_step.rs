//! Long steps on the single-particle cell, against the **shipped** LG M50.
//!
//! The `Spm` half of `docs/plans/end-of-step-split.md`, written up in
//! `docs/plans/spm-end-of-step.md`. Until this slice the pack solved an `Spm` against its
//! *start-of-step* curve, which is explicit Euler on every coupling the cells have through
//! their node: a scattered 1S3P at a one-hour step read 10 000 K by its fourth step, and a
//! voltage hold charged at 37.7 A whatever the step length. Each test here was run against
//! that engine and fails there; the scoring is in the plan note.
//!
//! On the shipped chemistry because the coupling that diverges is set by its OCP slopes
//! and its diffusivities, and a synthetic cell was how the equivalent-circuit half of this
//! defect first got a test that passed on the broken engine too.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Fault, Pack, PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shell count `sim_core::spm::DEFAULT_SHELLS` recommends.
const SHELLS: usize = 20;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
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
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: sigma,
            r0_sigma: sigma,
        },
        cell_model: CellModelConfig::Spm { shells: SHELLS },
    };
    Pack::new(
        &config,
        sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses"),
    )
    .expect("builds")
}

fn branch(pack: &Pack, k: usize) -> f64 {
    pack.cell(0, k)
        .expect("in range")
        .current_a
        .expect("stepped")
}

/// A scattered group at C/5 (3 A over three 5 Ah cells) stays split about evenly at
/// ten-minute and one-hour steps, all the way to empty. The start-of-step engine blew up
/// at both: 1.8e10 A near empty at ten minutes, and 10 000 K by the fourth hour.
#[test]
fn a_scattered_group_discharges_evenly_at_long_steps() {
    for dt in [600.0, 3600.0] {
        let mut p = pack(3, 0.9, 0.05);
        let mut n = 0;
        loop {
            let tele = p.step(dt, Demand::Current(3.0), &env());
            if tele.soc_true <= 0.0 {
                break;
            }
            n += 1;
            assert!(n < 100, "dt {dt}: never reached empty");
            for k in 0..3 {
                let i = branch(&p, k);
                assert!(
                    (0.9..1.15).contains(&i),
                    "dt {dt}, step {n}: branch {k} carries {i} A of a 3 A group"
                );
            }
            assert!(
                tele.t_max < 310.0,
                "dt {dt}, step {n}: {} K at C/5",
                tele.t_max
            );
            assert!(!tele.flags.contains(EventFlags::SOLVE_UNCONVERGED));
        }
    }
}

/// A voltage hold from half charge tapers: its charging current falls in magnitude step
/// on step and never turns into a discharge. The start-of-step engine opened every hold
/// at 37.7 A — a 7C charge — whatever the step length, which at ten minutes put the cell
/// at 443 K in one step.
#[test]
fn a_voltage_hold_tapers_at_long_steps() {
    for (dt, first_max) in [(600.0, 6.0), (3600.0, 2.0)] {
        let mut p = pack(1, 0.5, 0.0);
        let mut last = f64::NEG_INFINITY;
        for n in 0..12 {
            let tele = p.step(dt, Demand::Voltage(4.1), &env());
            let i = tele.i_actual;
            assert!(i <= 0.0, "dt {dt}, step {n}: the hold discharged at {i} A");
            assert!(
                -i < first_max,
                "dt {dt}, step {n}: charging at {} A from half charge",
                -i
            );
            assert!(
                i >= last,
                "dt {dt}, step {n}: the charge current grew, {last} → {i} A"
            );
            assert!(tele.t_max < 320.0, "dt {dt}, step {n}: {} K", tele.t_max);
            last = i;
        }
    }
}

/// A scattered group at rest with every cell at the same charge has nothing to exchange,
/// and rounding must not grow into something. On the start-of-step engine it grew about
/// fivefold an hour, 1.5e-14 A to 1.9e-12 A in six steps.
#[test]
fn rounding_does_not_grow_in_a_resting_group() {
    let mut p = pack(3, 0.5, 0.05);
    for n in 0..24 {
        p.step(3600.0, Demand::Rest, &env());
        for k in 0..3 {
            let i = branch(&p, k);
            assert!(i.abs() < 1.0e-13, "hour {n}: branch {k} carries {i} A");
        }
    }
}

/// Two cells left at different charges by a discharge exchange charge at rest, and at an
/// eleven-day step the exchange decays without ever reversing. On the start-of-step
/// engine the second rest step carried 1.7e9 A; with the end-of-step curve but a solve
/// seeded at the 5 A the cells last carried — which empties a particle many times over
/// in eleven days — it was the first.
#[test]
fn an_imbalanced_pair_settles_at_an_eleven_day_step() {
    let mut p = pack(2, 0.9, 0.0);
    p.schedule_fault(
        0.0,
        Fault::WeakCell {
            s: 0,
            p: 0,
            capacity_factor: 0.5,
            r0_factor: 1.0,
        },
    )
    .expect("valid fault");
    for _ in 0..360 {
        p.step(10.0, Demand::Current(5.0), &env());
    }
    let mut last = f64::NAN;
    for n in 0..8 {
        let tele = p.step(1.0e6, Demand::Rest, &env());
        assert!(
            !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "rest step {n}"
        );
        let i = branch(&p, 0);
        // The weaker cell ends the discharge lower, so it is the one being charged.
        if i.abs() > 1.0e-12 {
            assert!(i < 0.0, "rest step {n}: the exchange reversed, {i} A");
            // The first rest step has nothing to compare against.
            assert!(
                last.is_nan() || i.abs() < last.abs(),
                "rest step {n}: the exchange grew, {last} → {i} A"
            );
        }
        last = i;
    }
}

/// A current demand that drives a group through empty in one step is solved out past the
/// range the model describes — the caller asked for it — and converges there. Held here
/// because the solve's first pass is pulled back into the range only for a cell that
/// could rest inside it: pulled back from a cell already past empty, the second step ran
/// to 1.5e9 A.
#[test]
fn a_group_driven_through_empty_still_converges() {
    for demand in [20.0, -20.0] {
        let mut p = pack(3, if demand > 0.0 { 0.3 } else { 0.7 }, 0.05);
        for n in 0..3 {
            let tele = p.step(3600.0, Demand::Current(demand), &env());
            assert!(
                !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{demand} A, step {n}"
            );
            for k in 0..3 {
                let i = branch(&p, k).abs();
                assert!(
                    (5.0..8.0).contains(&i),
                    "{demand} A, step {n}: branch {k} carries {i} A"
                );
            }
        }
    }
}

/// A power the cell cannot deliver for the step lands at the most it can, inside its
/// voltage window, and says it fell short. The best a 35 % LG M50 can hold for an hour is
/// under 5 W; asked for 10 W, the solve used to find a root that exists only past empty,
/// where the model's curve goes flat — 57 A at 0.18 V, and 1000 K. It is also the check
/// that heat on an unconverged step comes from the cell and not from the solver's node:
/// that step once cooled the cell to 189 K.
#[test]
fn an_unreachable_power_lands_at_the_most_the_cell_can_give() {
    let mut p = pack(1, 0.35, 0.0);
    let tele = p.step(3600.0, Demand::Power(10.0), &env());
    assert!(
        (0.5..2.5).contains(&tele.i_actual),
        "the solve drew {} A",
        tele.i_actual
    );
    assert!(tele.v_terminal >= 2.5, "{} V, under v_min", tele.v_terminal);
    assert!(
        tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
        "10 W was not delivered and nothing said so: {:?}",
        tele.flags
    );
    assert!(
        (298.15..310.0).contains(&tele.t_max),
        "{} K after an hour near 1 A",
        tele.t_max
    );
}

/// One hour-long step heats a cell about as much as sixty one-minute steps do. The heat is
/// now read off the cell's own curve, at the step's first and last instants, and averaged.
/// Taken instead as the start-of-step equilibrium minus the end-of-step terminal, it
/// booked the equilibrium voltage's fall across the hour as heat: a 1S3P group then read
/// 303.3 K where sixty short steps read 299.5 K. The average of two instants runs a little
/// low (measured 0.82 of the short-step rise here), and the bound says so.
#[test]
fn an_hour_long_step_heats_like_sixty_short_ones() {
    let rise = |dt: f64, n: usize| {
        let mut p = pack(3, 0.9, 0.05);
        let mut t = 0.0;
        for _ in 0..n {
            t = p.step(dt, Demand::Current(3.0), &env()).t_max;
        }
        t - 298.15
    };
    let (long, short) = (rise(3600.0, 1), rise(60.0, 60));
    assert!(
        (0.75 * short..1.05 * short).contains(&long),
        "one hour-long step rose {long} K, sixty one-minute steps {short} K"
    );
}
