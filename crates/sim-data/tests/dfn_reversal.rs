//! The Doyle–Fuller–Newman cell past empty, against the **shipped** LG M50 — the `Dfn`
//! half of `docs/plans/porous-reversal.md`.
//!
//! Until that slice a `Dfn` driven past empty by a current demand kept carrying the whole
//! current: its electrodes ran into concentrations no particle holds, its Newton stopped
//! converging, and a 1S3P under a 2 Ω external short reached 1e74 A on its fourth hour; a
//! 1S3P driven through empty at 20 A reached inf at a one-second step. Past the current that
//! takes its bulk to the chemistry's empty, the solid now carries no more and the rest goes
//! down the `[reversal]` ramp into a deficit, as on the equivalent circuit and the `Spm`.
//! Past that edge the curve keeps falling by the electrodes' own kinetics at empty
//! (`spm::kinetics_at_empty_v`), which depend on no step length.
//!
//! What this does **not** cover, and no test here pretends it does: the electrolyte's own
//! limit. From half charge at 3 C the electrolyte at the positive current collector runs out
//! at 188 s with 34 % still in the cell, and the Newton fails there on this engine and the
//! one before it alike (ROADMAP H8). Every case here stays at or below 1.3 C per cell.

use serde_json::Value;
use sim_core::spm::ocp_lookup;
use sim_core::{
    AgingConfig, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Fault, Pack,
    PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

const DFN: CellModelConfig = CellModelConfig::Dfn {
    shells: 10,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};

/// The shipped cell's nameplate \[Ah\].
const Q_AH: f64 = 5.153198;

fn chem() -> ChemistryParams {
    sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses")
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn pack(parallel: u16, soc: f64, sigma: f64, thermal: ThermalConfig) -> Pack {
    pack_with(parallel, soc, sigma, thermal, chem(), None)
}

fn pack_with(
    parallel: u16,
    soc: f64,
    sigma: f64,
    thermal: ThermalConfig,
    chem: ChemistryParams,
    aging: Option<AgingConfig>,
) -> Pack {
    let config = PackConfig {
        aging,
        bms: None,
        thermal,
        series: 1,
        parallel,
        initial_soc: soc,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: sigma,
            r0_sigma: sigma,
        },
        cell_model: DFN,
    };
    Pack::new(&config, chem).expect("builds")
}

/// One cell's particles: every node's radial profile, as stoichiometries, `(negative,
/// positive)`.
type CellShells = (Vec<Vec<f64>>, Vec<Vec<f64>>);

/// Every shell of every node's particle, as stoichiometries, per cell: `(negative,
/// positive)`, read off a snapshot.
fn shells(pack: &Pack) -> Vec<CellShells> {
    let c = chem();
    let spm = c.spm.as_ref().expect("[spm]");
    let snap = serde_json::to_value(pack.snapshot()).expect("serializes");
    let mut out = Vec::new();
    fn walk(v: &Value, cn: f64, cp: f64, out: &mut Vec<CellShells>) {
        match v {
            Value::Object(m) => {
                if let (Some(Value::Array(n)), Some(Value::Array(p)), Some(_)) =
                    (m.get("c_neg"), m.get("c_pos"), m.get("c_e"))
                {
                    let read = |a: &Vec<Value>, c: f64| -> Vec<Vec<f64>> {
                        a.iter()
                            .map(|node| {
                                node.as_array()
                                    .expect("a node's profile")
                                    .iter()
                                    .map(|x| x.as_f64().expect("a concentration") / c)
                                    .collect()
                            })
                            .collect()
                    };
                    out.push((read(n, cn), read(p, cp)));
                } else {
                    for x in m.values() {
                        walk(x, cn, cp, out);
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, cn, cp, out)),
            _ => {}
        }
    }
    walk(
        &snap,
        spm.negative.c_max_mol_per_m3,
        spm.positive.c_max_mol_per_m3,
        &mut out,
    );
    out
}

/// Volume-weighted mean of one radial profile (uniform shells, innermost first).
fn shell_mean(profile: &[f64]) -> f64 {
    let (mut s, mut w) = (0.0, 0.0);
    for (i, x) in profile.iter().enumerate() {
        let vol = ((i + 1) as f64).powi(3) - (i as f64).powi(3);
        s += vol * x;
        w += vol;
    }
    s / w
}

/// The negative electrode's bulk position as a fraction of the window: the mean over its
/// nodes of each particle's mean. The nodes are uniform in width within an electrode, so the
/// plain mean is the volume mean.
fn bulk_position(p: &Pack) -> f64 {
    let c = chem();
    let n = &c.spm.as_ref().expect("[spm]").negative;
    let (neg, _) = &shells(p)[0];
    let x = neg.iter().map(|prof| shell_mean(prof)).sum::<f64>() / neg.len() as f64;
    (x - n.stoich_min) / (n.stoich_max - n.stoich_min)
}

/// One-C over two hours from half charge — an hour of it past empty: no shell anywhere
/// leaves `[0, 1]`, and the charge drawn past empty is in the deficit. The engine before
/// this slice ended this run with a negative-particle shell at −1.61.
#[test]
fn a_dfn_driven_past_empty_holds_no_lithium_it_does_not_have() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..120 {
        p.step(60.0, Demand::Current(5.0), &env());
    }
    for (neg, pos) in shells(&p) {
        for x in neg.iter().chain(pos.iter()).flatten() {
            assert!(
                (0.0..=1.0).contains(x),
                "a shell holds a stoichiometry of {x}, which no particle can"
            );
        }
    }
    let cell = p.cell(0, 0).expect("in range");
    assert!(
        cell.soc < 1e-12,
        "the solid holds {} past its edge",
        cell.soc
    );
    let owed = 5.0 * 2.0 - 0.5 * Q_AH;
    assert!(
        (cell.soc_deficit * Q_AH - owed).abs() < 1e-6,
        "deficit {} Ah, expected {owed}",
        cell.soc_deficit * Q_AH
    );
}

/// Rested an hour after that, the cell reads its chemistry's floor, and a charge repays the
/// deficit before it reaches the solid.
#[test]
fn a_rested_dfn_reads_the_floor_and_a_charge_repays_first() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..120 {
        p.step(60.0, Demand::Current(5.0), &env());
    }
    let mut v = f64::NAN;
    for _ in 0..60 {
        v = p.step(60.0, Demand::Rest, &env()).v_terminal;
    }
    let floor = chem().reversal.floor_v;
    assert!(
        (v - floor).abs() < 1e-3,
        "rested at {v} V against {floor} V"
    );
    let before = p.cell(0, 0).expect("in range").soc_deficit * Q_AH;
    for _ in 0..60 {
        let t = p.step(60.0, Demand::Current(-2.5), &env());
        assert!(t.soc_true < 1e-12, "the charge reached the solid first");
    }
    let after = p.cell(0, 0).expect("in range").soc_deficit * Q_AH;
    assert!(
        ((before - after) - 2.5).abs() < 1e-6,
        "repaid {} Ah",
        before - after
    );
}

/// The voltage past empty does not depend on the step length. The first design continued
/// the curve along the solve's own tangent, whose slope over a step carries the equilibrium
/// voltage's and the electrolyte's change across it: −0.85 V at a one-minute step against
/// −0.50 at ten seconds. Read off the electrodes' kinetics at empty instead, the two agree to
/// a millivolt.
#[test]
fn the_dfn_voltage_past_empty_does_not_depend_on_the_step() {
    let v_after_an_hour = |dt: f64| {
        let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
        let mut v = f64::NAN;
        for _ in 0..(3600.0 / dt) as usize {
            v = p.step(dt, Demand::Current(3.5), &env()).v_terminal;
        }
        v
    };
    let (minute, ten) = (v_after_an_hour(60.0), v_after_an_hour(10.0));
    assert!(minute < 0.0, "not past empty after an hour: {minute} V");
    assert!(
        (minute - ten).abs() < 1e-3,
        "{minute} V at a 60 s step against {ten} V at 10 s"
    );
}

/// The case that ran to 1e74 A: a scattered 1S3P under a 2 Ω external short at 2 A, four
/// hour-long steps. Bounded, physical, and converged now.
#[test]
fn a_shorted_dfn_pack_driven_past_empty_stays_physical() {
    let mut p = pack(3, 0.5, 0.05, ThermalConfig::Isothermal);
    p.schedule_fault(0.0, Fault::ExternalShort { ohms: 2.0 })
        .expect("schedules");
    for hour in 1..=4 {
        let t = p.step(3600.0, Demand::Current(2.0), &env());
        assert!(
            t.v_terminal.is_finite() && t.v_terminal.abs() < 5.0,
            "hour {hour}: {} V",
            t.v_terminal
        );
        assert!(
            t.i_actual.abs() < 10.0,
            "hour {hour}: {} A on a 2 A demand",
            t.i_actual
        );
        assert!(
            !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "hour {hour}: unconverged"
        );
    }
    for (neg, pos) in shells(&p) {
        for x in neg.iter().chain(pos.iter()).flatten() {
            assert!((0.0..=1.0).contains(x), "a shell at {x}");
        }
    }
}

/// A scattered 1S3P driven through empty at 20 A — 1.3 C per cell, below the electrolyte's
/// limit — converges on every step, isothermally and with the network on. The engine before
/// this slice reached inf at a one-second step and 1270–1340 K at ten and sixty.
#[test]
fn a_dfn_group_driven_through_empty_converges() {
    for thermal in [
        ThermalConfig::Isothermal,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
    ] {
        for dt in [10.0, 60.0] {
            let mut p = pack(3, 0.5, 0.05, thermal);
            for n in 0..(2400.0 / dt) as usize {
                let t = p.step(dt, Demand::Current(20.0), &env());
                assert!(
                    !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                    "dt {dt}, step {n}: unconverged"
                );
                assert!(
                    t.v_terminal.is_finite() && t.t_max < 400.0,
                    "dt {dt}, step {n}: {} V, {} K",
                    t.v_terminal,
                    t.t_max
                );
            }
        }
    }
}

/// The ledger, from the state at both ends: the solid's bulk equilibrium integrated over its
/// position, and the deficit's own store. One definition for both engines — below empty it
/// is the ramp — so on the engine before this slice, which released energy from lithium below
/// empty at a clamped voltage, the books do not close.
fn stored_j(c: &ChemistryParams, raw: f64, deficit: f64) -> f64 {
    let spm = c.spm.as_ref().expect("[spm]");
    let (n, p) = (&spm.negative, &spm.positive);
    let u = |z: f64| {
        ocp_lookup(&p.ocp, p.stoich_max - z * (p.stoich_max - p.stoich_min))
            - ocp_lookup(&n.ocp, n.stoich_min + z * (n.stoich_max - n.stoich_min))
    };
    let u_empty = u(0.0);
    let ramp = |d: f64| (u_empty - c.reversal.v_per_soc * d).max(c.reversal.floor_v);
    let extended = |z: f64| if z >= 0.0 { u(z) } else { ramp(-z) };
    let integrate = |f: &dyn Fn(f64) -> f64, a: f64, b: f64| {
        let k = 100_000;
        let h = (b - a) / k as f64;
        let mut s = 0.5 * (f(a) + f(b));
        for i in 1..k {
            s += f(a + i as f64 * h);
        }
        s * h
    };
    3600.0 * c.cell.capacity_ah * (integrate(&extended, 0.5, raw) - integrate(&ramp, 0.0, deficit))
}

/// An hour at 0.7 C from half charge, a third of it past empty: the ledger closes to an error
/// that shrinks with the step.
#[test]
fn the_dfn_energy_ledger_closes_with_the_step() {
    let c = chem();
    let imbalance = |dt: f64| {
        let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
        let e0 = stored_j(&c, bulk_position(&p), 0.0);
        let (mut elec, mut heat) = (0.0, 0.0);
        for _ in 0..(3600.0 / dt) as usize {
            let t = p.step(dt, Demand::Current(3.5), &env());
            elec += t.v_terminal * t.i_actual * dt;
            heat += t.q_gen_w * dt;
        }
        let e1 = stored_j(
            &c,
            bulk_position(&p),
            p.cell(0, 0).expect("in range").soc_deficit,
        );
        (e0 - e1) - elec - heat
    };
    let (coarse, fine) = (imbalance(60.0), imbalance(10.0));
    assert!(
        fine.abs() < 120.0,
        "{fine} J out at 10 s ({coarse} J at 60 s)"
    );
    assert!(
        fine.abs() < 0.3 * coarse.abs(),
        "the error did not shrink: {coarse} J at 60 s, {fine} J at 10 s"
    );
}

/// A snapshot taken past empty survives a real serialization and continues bit for bit.
#[test]
fn a_dfn_snapshot_past_empty_round_trips_through_json() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..40 {
        p.step(60.0, Demand::Current(5.0), &env());
    }
    assert!(
        p.cell(0, 0).expect("in range").soc_deficit > 0.0,
        "not past empty"
    );
    let text = serde_json::to_string(&p.snapshot()).expect("serializes");
    let snap = serde_json::from_str(&text).expect("deserializes");
    let mut q = Pack::restore(&snap).expect("restores");
    for d in [
        Demand::Current(5.0),
        Demand::Rest,
        Demand::Current(-2.0),
        Demand::Current(-2.0),
    ] {
        let (a, b) = (p.step(60.0, d, &env()), q.step(60.0, d, &env()));
        assert_eq!(a.v_terminal.to_bits(), b.v_terminal.to_bits());
        assert_eq!(a.q_gen_w.to_bits(), b.q_gen_w.to_bits());
    }
}

/// Over-discharge damages an aging `Dfn` as it does the other two models, against a
/// zero-coefficient control.
#[test]
fn over_discharge_damages_an_aging_dfn() {
    let run = |fade: f64| {
        let mut c = chem();
        c.reversal.fade_per_ah = fade;
        let mut p = pack_with(
            1,
            0.5,
            0.0,
            ThermalConfig::Isothermal,
            c,
            Some(AgingConfig {
                sub_clock_period_s: 0.0,
            }),
        );
        for _ in 0..40 {
            p.step(60.0, Demand::Current(5.0), &env());
        }
        let cell = p.cell(0, 0).expect("in range");
        (cell.soh_capacity, cell.soc_deficit)
    };
    let fade = chem().reversal.fade_per_ah;
    let (damaged, deficit) = run(fade);
    let (control, _) = run(0.0);
    assert!(deficit > 0.0, "the run never went past empty");
    let lost = control - damaged;
    let ah_past = deficit * Q_AH;
    assert!(
        lost >= fade * ah_past * 0.8 && lost <= fade * ah_past * 1.05,
        "lost {lost} over {ah_past} Ah past empty at {fade} per Ah"
    );
}
