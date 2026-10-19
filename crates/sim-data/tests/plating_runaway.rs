//! A failed plating solve may not pile up plated lithium (`docs/plans/plating-unconverged-runaway.md`).
//!
//! A cold fast charge on the fine grid runs the negative's electrolyte to zero near its current
//! collector, and the rest after it does not converge — on a threshold cell too. Before the fix a
//! reaction cell booked the plating each failed solve implied, which grew tenfold a step to infinity
//! and NaN within a minute of rest. Now a sub-step the reaction cannot be solved on is solved
//! without it: the cell stays finite, its stores do not move, and the step still says it failed.

use sim_core::dfn::{FINE_NODES_NEGATIVE, FINE_NODES_POSITIVE, FINE_NODES_SEPARATOR, FINE_SHELLS};
use sim_core::{
    AgingConfig, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig,
    PlatingModel, Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

fn chem() -> ChemistryParams {
    parse_chemistry(LGM50).expect("the shipped LG M50 file parses")
}

fn config(temp_k: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: 0.0,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: Some(AgingConfig {
            plating: PlatingModel::Reaction,
            ..AgingConfig::default()
        }),
        cell_model: CellModelConfig::Dfn {
            shells: FINE_SHELLS,
            nodes_negative: FINE_NODES_NEGATIVE,
            nodes_separator: FINE_NODES_SEPARATOR,
            nodes_positive: FINE_NODES_POSITIVE,
        },
    }
}

/// Charge from empty at `c_rate` to 4.2 V at `temp_c`, then rest for `rest_s`, at steps of `dt`;
/// assert prediction 4 on the rest.
fn charge_then_rest(temp_c: f64, c_rate: f64, dt: f64, rest_s: f64) {
    let c = chem();
    let q = c.cell.capacity_ah;
    let t = 273.15 + temp_c;
    let env = Env {
        t_ambient: t,
        t_coolant: None,
    };
    let mut p = Pack::new(&config(t), c).expect("builds");
    let mut secs = 0.0;
    while p.step(dt, Demand::Current(-c_rate * q), &env).v_terminal < 4.2 {
        secs += dt;
        assert!(secs < 3600.0, "{temp_c} C {c_rate}C never reached 4.2 V");
    }
    let end = p.cell_plating(0, 0).expect("a reaction cell");
    assert!(end.dead_mol > 0.0, "the charge plated");
    let (mut rested, mut unconverged) = (0.0, 0);
    while rested < rest_s {
        let tel = p.step(dt, Demand::Rest, &env);
        rested += dt;
        let at = format!("{temp_c} C {c_rate}C dt {dt}, {rested} s into the rest");
        assert!(tel.v_terminal.is_finite(), "{at}: v {}", tel.v_terminal);
        let cell = p.cell(0, 0).unwrap();
        assert!(cell.soc.is_finite(), "{at}: soc {}", cell.soc);
        assert!(
            !tel.flags.contains(EventFlags::PLATING_RISK),
            "{at}: plated"
        );
        let now = p.cell_plating(0, 0).unwrap();
        assert!(
            now.reversible_mol <= end.reversible_mol && now.dead_mol <= end.dead_mol,
            "{at}: stores {now:?} against {end:?} at the end of the charge"
        );
        unconverged += u32::from(tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    }
    println!("{temp_c} C {c_rate}C dt {dt}: {unconverged} unconverged rest steps");
    assert!(unconverged > 0, "the rest still reports its failed solves");
}

/// The case that ran away first: a 0 °C 3C charge, 1 s steps (NaN 82 s into the rest before).
#[test]
fn a_failed_rest_after_a_cold_3c_charge_stays_finite() {
    charge_then_rest(0.0, 3.0, 1.0, 120.0);
}

/// The case booking nothing did not fix: −10 °C 2C at 5 s steps, where the reaction's term inside
/// the failed solve dragged the particles off (NaN at the 26th rest step).
#[test]
fn a_failed_rest_after_a_colder_2c_charge_at_long_steps_stays_finite() {
    charge_then_rest(-10.0, 2.0, 5.0, 600.0);
}
