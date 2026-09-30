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

const HOUR_S: f64 = 3600.0;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn pack(parallel: u16, initial_soc: f64, sigma: f64) -> Pack {
    pack_with(
        1,
        parallel,
        initial_soc,
        sigma,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
    )
}

fn pack_with(
    series: u16,
    parallel: u16,
    initial_soc: f64,
    sigma: f64,
    thermal: ThermalConfig,
) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal,
        series,
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

/// A parallel pack held at a voltage low in its window for one hour stays bounded. The
/// answer is inside the range the model describes, but the solve's intermediate currents
/// were not: before a voltage demand's probes were held to that range, a scattered 1S3P
/// held at 2.585 V ran to 2e7 A and a 4S2P at 10.17 V to 1.4e6 A, both at billions of
/// kelvin. The single-cell hold test above could not see it.
#[test]
fn a_parallel_pack_holds_a_low_voltage_for_an_hour() {
    for (series, parallel, per_cell_v) in [(1u16, 3u16, 2.585), (1, 3, 2.84), (4, 2, 2.5425)] {
        let config = PackConfig {
            aging: None,
            bms: None,
            thermal: ThermalConfig::Network {
                k_neighbor_w_per_k: 1.0,
            },
            series,
            parallel,
            initial_soc: 0.5,
            initial_temp_k: 298.15,
            seed: 7,
            scatter: Scatter {
                capacity_sigma: 0.05,
                r0_sigma: 0.05,
            },
            cell_model: CellModelConfig::Spm { shells: SHELLS },
        };
        let mut p = Pack::new(
            &config,
            sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses"),
        )
        .expect("builds");
        let target = per_cell_v * f64::from(series);
        let tele = p.step(3600.0, Demand::Voltage(target), &env());
        let label = format!("{series}S{parallel}P at {target} V");
        for s in 0..usize::from(series) {
            for k in 0..usize::from(parallel) {
                let i = p.cell(s, k).expect("in range").current_a.expect("stepped");
                assert!(i.abs() < 10.0, "{label}: cell {s}S{k}P carries {i} A");
            }
        }
        assert!(tele.t_max < 330.0, "{label}: {} K", tele.t_max);
        assert!(
            (2.4 * f64::from(series)..4.3 * f64::from(series)).contains(&tele.v_terminal),
            "{label}: the pack reads {} V",
            tele.v_terminal
        );
    }
}

/// A zero-length step is how this repo reads an instantaneous voltage, and it must read
/// the start-of-step curve exactly as before this slice — no end of the step to diffuse
/// to, and no range to hold a demand's probes to. Held anyway, a 2.5 V target read at
/// `dt = 0` from half charge stopped at the edge of the start-of-step range, unconverged;
/// 738 zero-length demands on fresh packs agreed with
/// the start-of-step engine bit for bit only once the hold was gated on `dt > 0`.
#[test]
fn a_zero_length_step_is_not_held_to_the_range() {
    for target in [2.5, 2.6, 4.2] {
        let tele = pack(1, 0.5, 0.0).step(0.0, Demand::Voltage(target), &env());
        assert!(
            !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "a zero-length read at {target} V did not converge: {tele:?}"
        );
    }
}

/// An hour-long voltage hold or power demand either meets its demand or says it did not.
/// Isothermal, because a thermal pack re-reads its terminal voltage after the step's heat
/// and that moves it off a demand the solve met. The parent of
/// `docs/plans/spm-pack-window.md` reported every case here converged: the holds from 98 %
/// up to 0.56 V off target, the powers on a point that was neither the demand nor the
/// cell's maximum.
#[test]
fn a_long_hold_or_power_meets_its_demand_or_says_so() {
    let iso = |parallel, soc, sigma| pack_with(1, parallel, soc, sigma, ThermalConfig::Isothermal);
    let mut silent = Vec::new();
    for target in [
        2.56375, 2.585, 2.60625, 2.69125, 2.7125, 2.77625, 2.8825, 3.07375, 3.11625,
    ] {
        let tele = iso(1, 0.98, 0.0).step(HOUR_S, Demand::Voltage(target), &env());
        if !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED)
            && (tele.v_terminal - target).abs() > 1e-6
        {
            silent.push(format!("98 % held at {target} V: {} V", tele.v_terminal));
        }
    }
    for (parallel, soc, sigma, watts) in [
        (1, 0.02, 0.0, 11.07),
        (1, 0.5, 0.0, 8.07375),
        (1, 0.5, 0.0, 11.00625),
        (3, 0.5, 0.05, 22.5),
    ] {
        let tele = iso(parallel, soc, sigma).step(HOUR_S, Demand::Power(watts), &env());
        let delivered = tele.v_terminal * tele.i_actual;
        if !tele.flags.contains(EventFlags::SOLVE_UNCONVERGED)
            && (delivered - watts).abs() > 1e-6 * watts
        {
            silent.push(format!(
                "1S{parallel}P from {soc} at {watts} W: {delivered} W"
            ));
        }
    }
    assert!(silent.is_empty(), "missed and said nothing: {silent:#?}");
}

/// Every power past what the pack can give over the hour lands on the most it can give,
/// says so, and gets there well inside the pass cap — whatever the power asked.
///
/// The maximum is a steep knee on a single cell and, on a scattered group, one corner per
/// cell. Halving towards the last iterate crawled up the knee to the cap, and a search
/// scored against the last tangent cycled and landed wherever the cap fell (1.07 A or
/// 1.80 A for the 10 W hour). A single cell lands on one current; a scattered pack's power
/// curve has several peaks within 1.4e-5 of each other (measured over the 81 powers of the
/// plan note's sweep), and which one the search reaches depends on the power, so there the
/// check is the power delivered. Powers 9 and 11 of the sweep are where a 4S2P's bracket
/// once collapsed onto a sign filed while its split was still settling, and sat at the cap.
/// See `docs/plans/spm-pack-window.md`.
#[test]
fn unmet_powers_land_on_the_maximum_inside_the_pass_cap() {
    for (series, parallel, sigma) in [(1u16, 1u16, 0.0), (1, 3, 0.05), (4, 2, 0.05)] {
        let mut landed: Vec<(f64, f64)> = Vec::new();
        for k in (0..81).step_by(10).chain([9, 11]) {
            let volts = (2.5 + (4.2 - 2.5) * f64::from(k) / 80.0) * f64::from(series);
            let watts = 3.0 * volts * f64::from(parallel);
            let mut p = pack_with(series, parallel, 0.5, sigma, ThermalConfig::Isothermal);
            let tele = p.step(HOUR_S, Demand::Power(watts), &env());
            let what = format!("{series}S{parallel}P at {watts} W");
            assert!(
                tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{what}: not met and not said: {tele:?}"
            );
            assert!(
                tele.solve_iterations < sim_core::pack::SOLVE_ITER_CAP,
                "{what}: ran to the cap"
            );
            landed.push((tele.i_actual, tele.v_terminal * tele.i_actual));
        }
        let most = landed.iter().map(|l| l.1).fold(f64::MIN, f64::max);
        let least = landed.iter().map(|l| l.1).fold(f64::MAX, f64::min);
        assert!(
            most - least <= 1e-4 * most,
            "{series}S{parallel}P: landed between {least} and {most} W: {landed:?}"
        );
        if parallel == 1 {
            let hi = landed.iter().map(|l| l.0).fold(f64::MIN, f64::max);
            let lo = landed.iter().map(|l| l.0).fold(f64::MAX, f64::min);
            assert!(hi - lo <= 1e-8, "one cell, landed from {lo} to {hi} A");
        }
    }
}

/// Hour after hour of a power the cell cannot give takes what it has left and then
/// nothing: the pack's current is held to the range that keeps each particle's bulk
/// between the chemistry's empty and full. Without that range the second hour of 10 W
/// drew 0.26 A from a cell holding 0.196 A·h and the third and fourth kept draining it
/// past empty, to 1.19 V; before the range was declared to the pack at all, the third hour
/// ran 6.8 A and 372 K. See `docs/plans/spm-pack-window.md`.
#[test]
fn an_unreachable_power_never_drives_the_cell_past_empty() {
    let mut p = pack(1, 0.35, 0.0);
    for hour in 0..4 {
        let tele = p.step(HOUR_S, Demand::Power(10.0), &env());
        assert!(
            tele.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "hour {hour}: 10 W was not met and nothing said so: {:?}",
            tele.flags
        );
        assert!(
            !tele.flags.contains(EventFlags::SOC_CLAMPED_LOW) && tele.i_actual >= 0.0,
            "hour {hour}: driven past empty ({} A, {:?})",
            tele.i_actual,
            tele.flags
        );
    }
}
