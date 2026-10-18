//! A parallel `Dfn` pair split at a long step must never commit a non-finite number — nor a
//! finite one no physics could produce.
//!
//! `docs/plans/dfn-split-nan.md`. Found by the Phase 12 spike's control runs: a 1S2P pack of the
//! shipped LG M50, one cell at 0.9 of the other's capacity, charged from empty at 1C per cell with
//! one-hour steps. Reproduced in the tree on the fine grid at 0 °C: the pack's pass loop, which
//! under a current demand then had no damping, swung from a plausible split to 1.2e10 A circulating
//! between the two cells on the third hour, where a slope rounded to zero and the next pass divided
//! by it. That step wrote NaN into both cells' state, and from then on the voltage was NaN on every
//! step, rest included. `step()` reports trouble by flags — it may fail to converge, and say so,
//! but what it commits has to be a state the next step can start from.
//!
//! Fixed by a floor on the `Dfn`'s slope past its window edge and by damping the split itself:
//! under a current or rest demand each pass's split is walked back towards the last accepted one
//! until the residual falls, and where none does the loop keeps that one and stops, flagged
//! unconverged.
//!
//! After every step:
//!
//! * **Finite**, read three ways: every `Telemetry` number, every `CellView` number, and the
//!   snapshot. A snapshot's JSON spells a non-finite float `null`, so its nulls are compared, by
//!   path, against those of a pack that has taken one ordinary step: a `None` is a null there too,
//!   and is the same null on every step.
//! * **Conserving**: the two cells' currents sum to the pack's. Nothing bleeds or shorts here, so
//!   a split that does not is charge appearing from nowhere.
//! * **Not blown up**: no cell carries more than the pack's current plus 1C. A finite split can
//!   still be nonsense — the loop passed through ±200 A and ±1e7 A circulating before it reached
//!   NaN — and a floor on the slope alone would have committed one of those. This bound catches a
//!   blow-up, **not a wrong split**: the steps here are unconverged and say so, and what they
//!   commit is the last split the damping accepted, which at one-hour steps past full can still be
//!   lopsided. 1C is about twice the most two parallel cells can circulate on their own voltage
//!   difference (about 1 V over 0.4 Ω), so a split past it is the solver, not the pair.
//!
//! One arm runs with the thermal network on, so that a blown-up current would also be a non-finite
//! temperature; it is held to the same bound. What none of this closes is the `Dfn` past its edges
//! — no physics past full, and fast runs past empty that still blow up past the bound, finitely
//! (`docs/ROADMAP.md` H8) — nor a recorded limit of this model under a thermal network at long
//! steps (`docs/plans/phase-11-fitting.md`).

use std::collections::BTreeSet;

use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, Telemetry, ThermalConfig};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

const T0_K: f64 = 298.15;
const T_COLD_K: f64 = 273.15;
const HOUR_S: f64 = 3600.0;

const RECOMMENDED: CellModelConfig = CellModelConfig::Dfn {
    shells: sim_core::spm::DEFAULT_SHELLS,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};

const FINE: CellModelConfig = CellModelConfig::Dfn {
    shells: sim_core::dfn::FINE_SHELLS,
    nodes_negative: sim_core::dfn::FINE_NODES_NEGATIVE,
    nodes_separator: sim_core::dfn::FINE_NODES_SEPARATOR,
    nodes_positive: sim_core::dfn::FINE_NODES_POSITIVE,
};

fn env(t_k: f64) -> Env {
    Env {
        t_ambient: t_k,
        t_coolant: None,
    }
}

fn chem() -> sim_core::ChemistryParams {
    sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses")
}

/// 1S2P at `soc0`, the second cell at 0.9 of the first's capacity.
fn mismatched_pair(
    cell_model: CellModelConfig,
    t_k: f64,
    thermal: ThermalConfig,
    soc0: f64,
) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal,
        series: 1,
        parallel: 2,
        initial_soc: soc0,
        initial_temp_k: t_k,
        seed: 0,
        scatter: Scatter::default(),
        cell_model,
    };
    let mut p = Pack::new(&config, chem()).expect("builds");
    p.set_cell_factors(0, 1, 0.9, 1.0).expect("cell exists");
    p
}

/// Every path in `v` whose value is `null`, array indices dropped.
fn null_paths(v: &serde_json::Value, path: &str, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Null => {
            out.insert(path.to_owned());
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| null_paths(x, path, out)),
        serde_json::Value::Object(m) => m
            .iter()
            .for_each(|(k, x)| null_paths(x, &format!("{path}.{k}"), out)),
        _ => {}
    }
}

fn snapshot_nulls(p: &Pack) -> BTreeSet<String> {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot serializes");
    let mut out = BTreeSet::new();
    null_paths(&v, "", &mut out);
    out
}

/// What is wrong after a step, by name; empty when nothing is.
fn defects(
    p: &Pack,
    t: &Telemetry,
    parallel: usize,
    allowed_nulls: &BTreeSet<String>,
    bound_c: Option<f64>,
) -> Vec<String> {
    let mut bad = Vec::new();
    for (name, x) in [
        ("v_terminal", t.v_terminal),
        ("i_actual", t.i_actual),
        ("soc_true", t.soc_true),
        ("t_min", t.t_min),
        ("t_max", t.t_max),
        ("v_cell_min", t.v_cell_min),
        ("v_cell_max", t.v_cell_max),
        ("soh_capacity", t.soh_capacity),
        ("soh_resistance", t.soh_resistance),
        ("q_gen_w", t.q_gen_w),
    ] {
        if !x.is_finite() {
            bad.push(format!("telemetry.{name} = {x}"));
        }
    }
    let mut sum_i = 0.0;
    let mut sum_abs_i = 0.0;
    for k in 0..parallel {
        let c = p.cell(0, k).expect("cell");
        let i = c.current_a.expect("a stepped pack reports its split");
        for (name, x) in [
            ("soc", c.soc),
            ("soc_deficit", c.soc_deficit),
            ("temp_k", c.temp_k),
            ("overpotential_v", c.overpotential_v),
            ("current_a", i),
        ] {
            if !x.is_finite() {
                bad.push(format!("cell {k}.{name} = {x}"));
            }
        }
        let bound = bound_c.map_or(f64::INFINITY, |c_a| t.i_actual.abs() + c_a);
        if i.abs() > bound {
            bad.push(format!(
                "cell {k} carries {i} A against a pack current of {} A (bound {bound} A)",
                t.i_actual
            ));
        }
        sum_i += i;
        sum_abs_i += i.abs();
    }
    // Rounding in a sum scales with its largest term, not with the result: an unbounded split
    // with ±5e5 A circulating (the cold discharge below, on a debug build) sums to the pack's
    // 20.6 A within 2.7e-8 A.
    if (sum_i - t.i_actual).abs() > 1e-9 * (1.0 + sum_abs_i) {
        bad.push(format!(
            "the cells carry {sum_i} A between them and the pack {} A",
            t.i_actual
        ));
    }
    for path in snapshot_nulls(p).difference(allowed_nulls) {
        bad.push(format!("snapshot{path} is null (a non-finite float)"));
    }
    bad
}

/// Five one-hour charge steps at 1C per cell — both cells are past full by the third — then five
/// one-hour rests; nothing wrong after any of them.
fn charges_and_rests(cell_model: CellModelConfig, t_k: f64, thermal: ThermalConfig) {
    let c_a = chem().cell.capacity_ah;
    let allowed = {
        let mut p = mismatched_pair(cell_model, t_k, thermal, 0.0);
        p.step(1.0, Demand::Current(-2.0 * c_a), &env(t_k));
        snapshot_nulls(&p)
    };
    let mut p = mismatched_pair(cell_model, t_k, thermal, 0.0);
    for n in 0..10 {
        let demand = if n < 5 {
            Demand::Current(-2.0 * c_a)
        } else {
            Demand::Rest
        };
        let t = p.step(HOUR_S, demand, &env(t_k));
        let bad = defects(&p, &t, 2, &allowed, Some(c_a));
        assert!(
            bad.is_empty(),
            "{t_k} K, {thermal:?}, hour {} ({demand:?}), flags {:?}: {bad:#?}",
            n + 1,
            t.flags
        );
    }
}

/// The case the spike found: NaN on the third hour before the fix.
#[test]
fn the_fine_grid_at_zero_celsius_stays_finite() {
    charges_and_rests(FINE, T_COLD_K, ThermalConfig::Isothermal);
}

#[test]
fn the_fine_grid_at_room_temperature_stays_finite() {
    charges_and_rests(FINE, T0_K, ThermalConfig::Isothermal);
}

#[test]
fn the_recommended_grid_stays_finite_cold_and_warm() {
    charges_and_rests(RECOMMENDED, T_COLD_K, ThermalConfig::Isothermal);
    charges_and_rests(RECOMMENDED, T0_K, ThermalConfig::Isothermal);
}

/// With heat on, a nonsense current would be a nonsense temperature.
#[test]
fn the_fine_grid_at_zero_celsius_stays_finite_with_heat() {
    charges_and_rests(
        FINE,
        T_COLD_K,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
    );
}

/// Found by the slice's sweep, where 32 of 100 parallel `Dfn` runs committed NaN on the parent,
/// most of them discharges past empty: a scattered 1S3P discharged from full at 2C per cell for
/// three hours of one-minute steps, at 25 °C. On the parent this one stayed finite; with the
/// slope floor and a "best pass" fallback instead of a damped split it went to 1e17 A
/// circulating by its 35th step and NaN by its 166th.
#[test]
fn a_scattered_triple_discharged_past_empty_stays_bounded() {
    let chem = chem();
    let c_a = chem.cell.capacity_ah;
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 3,
        initial_soc: 1.0,
        initial_temp_k: T0_K,
        seed: 3,
        scatter: Scatter {
            capacity_sigma: 0.05,
            r0_sigma: 0.05,
        },
        cell_model: RECOMMENDED,
    };
    let demand = Demand::Current(2.0 * c_a * 3.0);
    let allowed = {
        let mut p = Pack::new(&config, chem.clone()).expect("builds");
        p.step(1.0, demand, &env(T0_K));
        snapshot_nulls(&p)
    };
    let mut p = Pack::new(&config, chem).expect("builds");
    for n in 0..180 {
        let t = p.step(60.0, demand, &env(T0_K));
        let bad = defects(&p, &t, 3, &allowed, Some(c_a));
        assert!(
            bad.is_empty(),
            "minute {}, flags {:?}: {bad:#?}",
            n + 1,
            t.flags
        );
    }
}

/// Where no damped split improves on the last accepted one, the loop keeps that one and stops:
/// the pair discharged at 2C per cell from full at 0 °C, at 15-minute steps, drives the larger cell
/// past what the `Dfn` can answer, and on its fourth step a probe returned a NaN voltage for a
/// finite current (−8842 A). Taking the smallest damped step anyway, as the pack-current search does, carried that
/// NaN into every later pass and committed it. Held to finite and conserving only: this run is
/// still among the sweep's broken ones (a cell past the current bound), as it was on the parent.
#[test]
fn a_cold_fast_discharge_past_empty_stays_finite() {
    let c_a = chem().cell.capacity_ah;
    let allowed = {
        let mut p = mismatched_pair(RECOMMENDED, T_COLD_K, ThermalConfig::Isothermal, 1.0);
        p.step(1.0, Demand::Current(4.0 * c_a), &env(T_COLD_K));
        snapshot_nulls(&p)
    };
    let mut p = mismatched_pair(RECOMMENDED, T_COLD_K, ThermalConfig::Isothermal, 1.0);
    for n in 0..4 {
        let t = p.step(900.0, Demand::Current(4.0 * c_a), &env(T_COLD_K));
        let bad = defects(&p, &t, 2, &allowed, None);
        assert!(
            bad.is_empty(),
            "quarter hour {}, flags {:?}: {bad:#?}",
            n + 1,
            t.flags
        );
    }
}

/// A hold that starts with a cell past its edge settles (`docs/plans/dfn-hold-past-edge.md`): the
/// mismatched pair charged at 1C per cell for an hour — the smaller cell past full — then held at
/// 4.2 V, 15-minute steps, recommended grid, 25 °C. Each hold probe of a cell past its edge used to be
/// clamped to that edge, every pass saw the same line, and five of the eight hold steps stalled
/// unconverged; with the probe's range widened to take zero in, as the pack's is, none does.
#[test]
fn a_hold_from_past_full_settles() {
    use sim_core::EventFlags;
    let c_a = chem().cell.capacity_ah;
    let mut p = mismatched_pair(RECOMMENDED, T0_K, ThermalConfig::Isothermal, 0.0);
    let dt = 900.0;
    for _ in 0..4 {
        p.step(dt, Demand::Current(-2.0 * c_a), &env(T0_K));
    }
    let past_full = p.cell(0, 1).expect("cell exists").soc;
    assert!(
        past_full >= 1.0,
        "the smaller cell must end the charge at full ({past_full})"
    );
    for k in 0..8 {
        let t = p.step(dt, Demand::Voltage(4.2), &env(T0_K));
        assert!(
            !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "hold step {k} unconverged at {} V",
            t.v_terminal
        );
        assert!(
            (t.v_terminal - 4.2).abs() < 1.0e-6,
            "hold step {k}: {} V",
            t.v_terminal
        );
    }
}
