//! The single-particle cell past empty, against the **shipped** LG M50.
//!
//! Until `docs/plans/porous-reversal.md` a particle driven past its edge kept carrying the
//! whole current: its bulk ran below empty into concentrations no particle can hold, its
//! surface was clamped at the edge of its tables, and the terminal voltage went flat there.
//! A single cell sat on that flat line (0.50 V at 1 C, for hours), the energy it released
//! came from lithium it did not have, and a parallel group of cells on it could not share a
//! current — a scattered 1S3P driven through empty at 20 A went to NaN at every step length
//! measured. Each test here was run against that engine and fails there; the scoring is in
//! the plan note.
//!
//! The physics: past the edge — the current that takes a particle's surface or its bulk to
//! empty over the step — the particle carries no more, and the rest goes down the
//! chemistry's `[reversal]` ramp into a deficit (`CellView::soc_deficit`), the same ramp
//! and floor the equivalent circuit uses. A charge repays the deficit before it reaches the
//! particle, and so does the particle's own lithium once its surface has room again.

use serde_json::Value;
use sim_core::spm::ocp_lookup;
use sim_core::{
    AgingConfig, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig,
    Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shell count `sim_core::spm::DEFAULT_SHELLS` recommends.
const SHELLS: usize = 20;

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
        cell_model: CellModelConfig::Spm { shells: SHELLS },
    };
    Pack::new(&config, chem).expect("builds")
}

/// Every particle's shell concentrations, as stoichiometries, read off a snapshot — the
/// state itself rather than anything the engine reports about it.
fn stoichiometries(pack: &Pack) -> Vec<(Vec<f64>, Vec<f64>)> {
    let c = chem();
    let spm = c.spm.as_ref().expect("[spm]");
    let snap = serde_json::to_value(pack.snapshot()).expect("serializes");
    let mut out = Vec::new();
    fn walk(v: &Value, cn: f64, cp: f64, out: &mut Vec<(Vec<f64>, Vec<f64>)>) {
        match v {
            Value::Object(m) => {
                if let (Some(Value::Array(n)), Some(Value::Array(p))) =
                    (m.get("c_neg"), m.get("c_pos"))
                {
                    let read = |a: &Vec<Value>, c: f64| {
                        a.iter()
                            .map(|x| x.as_f64().expect("a concentration") / c)
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

/// Two hours at 5 A from half charge, an hour of it past empty: no particle ever holds
/// less lithium than none or more than it can, and the charge drawn past empty is in the
/// deficit rather than in the particles. The old engine ended this run with the negative
/// particle's outer shell at a stoichiometry of −1.26.
#[test]
fn a_particle_driven_past_empty_holds_no_lithium_it_does_not_have() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..120 {
        p.step(60.0, Demand::Current(5.0), &env());
    }
    for (neg, pos) in stoichiometries(&p) {
        for x in neg.iter().chain(pos.iter()) {
            assert!(
                (0.0..=1.0).contains(x),
                "a shell holds a stoichiometry of {x}, which no particle can"
            );
        }
    }
    let cell = p.cell(0, 0).expect("in range");
    // Empty to rounding: the particle stops at the edge the step's closed form computes,
    // which lands within a few ULP of it rather than on it.
    assert!(
        cell.soc < 1e-12,
        "the particle holds {} past its edge",
        cell.soc
    );
    // 10 Ah out of a cell that held 2.58: the rest is the deficit, to rounding.
    let q = 5.153198;
    let owed = 5.0 * 2.0 - 0.5 * q;
    assert!(
        (cell.soc_deficit * q - owed).abs() < 1e-6,
        "deficit {} Ah, expected {owed}",
        cell.soc_deficit * q
    );
}

/// The same over-drained cell, rested for an hour, reads the floor its chemistry declares
/// — not the 1.10 V of a surface clamped at the edge of its tables, which is what the old
/// engine rested at however far the cell had been driven.
#[test]
fn a_rested_over_drained_cell_reads_the_reversal_floor() {
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
        "rested at {v} V against a floor of {floor} V"
    );
}

/// A charge repays the deficit before the particle sees any of it: an hour at 2.5 A takes
/// 2.5 Ah off a 7.42 Ah deficit, and the charge state stays at zero throughout.
#[test]
fn a_charge_repays_the_deficit_before_the_particle() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..120 {
        p.step(60.0, Demand::Current(5.0), &env());
    }
    let q = 5.153198;
    let before = p.cell(0, 0).expect("in range").soc_deficit * q;
    for _ in 0..60 {
        let t = p.step(60.0, Demand::Current(-2.5), &env());
        assert!(
            t.soc_true < 1e-12,
            "the charge reached the particle first: {}",
            t.soc_true
        );
    }
    let after = p.cell(0, 0).expect("in range").soc_deficit * q;
    assert!(
        ((before - after) - 2.5).abs() < 1e-6,
        "repaid {} Ah of 2.5",
        before - after
    );
}

/// A hard over-drive empties the particle's SURFACE while its bulk still holds charge. At
/// 5 C from half charge for 300 s the surface runs dry with about 14 % still inside; the
/// cell carries a deficit while its charge reads above zero, and rest pays it back out of
/// the particle's own lithium. The old engine carried no deficit at all.
#[test]
fn a_fast_over_drive_is_paid_back_at_rest() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..300 {
        p.step(1.0, Demand::Current(25.0), &env());
    }
    let cell = p.cell(0, 0).expect("in range");
    assert!(
        cell.soc_deficit > 0.0,
        "the surface ran dry and nothing was carried past it"
    );
    assert!(
        cell.soc > 0.05,
        "the bulk was emptied ({}), so this is not the surface case",
        cell.soc
    );
    let mut t = None;
    for _ in 0..1800 {
        t = Some(p.step(1.0, Demand::Rest, &env()));
    }
    let cell = p.cell(0, 0).expect("in range");
    assert_eq!(cell.soc_deficit, 0.0, "rest did not pay the deficit back");
    let v = t.expect("stepped").v_terminal;
    assert!(v > 3.2, "a cell with charge left rested at only {v} V");
}

/// A scattered group driven through empty at 3 C per cell settles on every step, at every
/// step length and with the thermal network on and off. Its cells do not reach their edges
/// together, and the old engine's split, which re-takes each cell's tangent and solves
/// again, cycled between them; the fallback that brackets each group's node voltage is
/// what settles it now (`settle_group` in `pack.rs`).
#[test]
fn a_scattered_group_driven_through_empty_settles() {
    for thermal in [
        ThermalConfig::Isothermal,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 1.0,
        },
    ] {
        // With the thermal network on, this group runs away thermally near empty at this
        // rate, and from a ten-second step up the burning cells reach the runaway
        // integrator's own sub-step cap — a debug assertion in `thermal.rs` that the engine
        // before this slice trips on the same run (at 1280 K, ten-second steps). A limit of
        // that integrator rather than of this split, so the thermal arm runs at one second;
        // see `docs/plans/porous-reversal.md`.
        let steps_s: &[f64] = if matches!(thermal, ThermalConfig::Isothermal) {
            &[1.0, 10.0, 60.0]
        } else {
            &[1.0]
        };
        for &dt in steps_s {
            let mut p = pack(3, 0.5, 0.05, thermal);
            let steps = (1.5 * 0.5 * 3600.0 / 3.0 / dt) as usize;
            for n in 0..steps {
                let t = p.step(dt, Demand::Current(45.0), &env());
                assert!(
                    !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                    "dt {dt}, step {n}: unconverged"
                );
                assert!(
                    t.v_terminal.is_finite() && t.t_max.is_finite(),
                    "dt {dt}, step {n}: {} V, {} K",
                    t.v_terminal,
                    t.t_max
                );
                // Bounded either way. Isothermally the worst cell carries the group's 45 A
                // (a reversed cell is charged a little by neighbours that still hold charge,
                // which repays its deficit and is real). With the thermal network on, this
                // group runs away thermally near empty at this rate — 450 K at 660 s, 1300 K
                // twenty seconds later, as it did before this slice — and cells hundreds of
                // kelvin apart redistribute up to about four times the group current at
                // converged steps. The bound is what separates either from the old split,
                // which circulated billions of amps.
                let bound = if matches!(thermal, ThermalConfig::Isothermal) {
                    2.0
                } else {
                    10.0
                } * 45.0;
                for k in 0..3 {
                    let i = p.cell(0, k).expect("in range").current_a.expect("stepped");
                    assert!(
                        i.abs() <= bound,
                        "dt {dt}, step {n}: cell {k} carries {i} A of a 45 A group"
                    );
                }
            }
        }
    }
}

/// The cell's stored energy at a state: the particles' equilibrium voltage integrated over
/// their bulk position, plus the deficit's own store — the reversal's open-circuit voltage
/// integrated over the deficit, which is energy that LEAVES storage as the deficit grows.
/// Separable, so it is a state function whatever the particles hold beside a deficit.
///
/// **One definition for every engine**, and below empty it is the reversal's: a bulk
/// position below zero — lithium a particle cannot hold, which only the old engine reached
/// — is charged at the ramp, not at whatever a clamped table lookup returns there. Read
/// that way the old engine's books do not close, which is the point: it released energy
/// at about 1.1 V from lithium that was not there.
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
    let q_as = 3600.0 * c.cell.capacity_ah;
    q_as * (integrate(&extended, 0.5, raw) - integrate(&ramp, 0.0, deficit))
}

fn bulk_position(p: &Pack) -> f64 {
    let c = chem();
    let n = &c.spm.as_ref().expect("[spm]").negative;
    let (neg, _) = &stoichiometries(p)[0];
    // Volume-weighted mean over uniform shells, innermost first.
    let (mut s, mut w) = (0.0, 0.0);
    for (i, x) in neg.iter().enumerate() {
        let vol = ((i + 1) as f64).powi(3) - (i as f64).powi(3);
        s += vol * x;
        w += vol;
    }
    (s / w - n.stoich_min) / (n.stoich_max - n.stoich_min)
}

/// The energy ledger of a fast over-drain, from the state at its two ends: stored energy
/// lost equals electrical energy out plus heat, to an error that shrinks with the step.
/// The old engine's error did not shrink — it released energy at a fixed voltage from
/// lithium below empty, ~16 kJ of it on this run whatever the step length.
#[test]
fn the_energy_ledger_of_an_over_drive_closes_with_the_step() {
    let c = chem();
    let imbalance = |dt: f64| {
        let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
        let e0 = stored_j(&c, bulk_position(&p), 0.0);
        let (mut elec, mut heat) = (0.0, 0.0);
        for _ in 0..(1200.0 / dt) as usize {
            let t = p.step(dt, Demand::Current(20.0), &env());
            elec += t.v_terminal * t.i_actual * dt;
            heat += t.q_gen_w * dt;
        }
        let cell = p.cell(0, 0).expect("in range");
        let e1 = stored_j(&c, bulk_position(&p), cell.soc_deficit);
        (e0 - e1) - elec - heat
    };
    let (coarse, fine) = (imbalance(10.0), imbalance(1.0));
    assert!(
        fine.abs() < 50.0,
        "the ledger is {fine} J out at dt = 1 s (and {coarse} J at 10 s)"
    );
    assert!(
        fine.abs() < 0.2 * coarse.abs(),
        "the error did not shrink with the step: {coarse} J at 10 s, {fine} J at 1 s"
    );
}

/// A snapshot taken mid-reversal — the surface run dry, a deficit carried, charge still
/// inside — survives a real serialization (JSON, the server's wire format, not a clone)
/// and continues bit for bit. Both new fields are state: drop either and the restored
/// cell divides its next current differently.
#[test]
fn a_snapshot_mid_reversal_round_trips_through_json() {
    let mut p = pack(1, 0.5, 0.0, ThermalConfig::Isothermal);
    for _ in 0..290 {
        p.step(1.0, Demand::Current(25.0), &env());
    }
    let cell = p.cell(0, 0).expect("in range");
    assert!(cell.soc_deficit > 0.0 && cell.soc > 0.0, "not mid-reversal");
    let text = serde_json::to_string(&p.snapshot()).expect("serializes");
    let snap = serde_json::from_str(&text).expect("deserializes");
    let mut q = Pack::restore(&snap).expect("restores");
    for _ in 0..60 {
        let d = Demand::Current(10.0);
        let (a, b) = (p.step(1.0, d, &env()), q.step(1.0, d, &env()));
        assert_eq!(a.v_terminal.to_bits(), b.v_terminal.to_bits());
        assert_eq!(a.q_gen_w.to_bits(), b.q_gen_w.to_bits());
    }
    for _ in 0..60 {
        let (a, b) = (
            p.step(1.0, Demand::Rest, &env()),
            q.step(1.0, Demand::Rest, &env()),
        );
        assert_eq!(a.v_terminal.to_bits(), b.v_terminal.to_bits());
    }
}

/// Over-discharge damages a single-particle cell that ages, as it does an equivalent
/// circuit: the chemistry's `[reversal] fade_per_ah` is billed per amp-hour past empty,
/// and the deficit is where that amp-hour count now comes from. A control arm with the
/// coefficient at zero isolates it from calendar and cycle fade.
#[test]
fn over_discharge_damages_an_aging_single_particle_cell() {
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
    assert!(
        lost > 0.0,
        "going past empty cost nothing: {damaged} against {control}"
    );
    // About `fade_per_ah` per amp-hour past empty: the deficit is billed as it grows, on a
    // capacity the damage itself is shrinking, so this is a bound and not an identity.
    let ah_past = deficit * 5.153198;
    assert!(
        lost <= fade * ah_past * 1.05,
        "lost {lost}, more than {fade} per Ah over {ah_past} Ah"
    );
    assert!(
        lost >= fade * ah_past * 0.8,
        "lost {lost}, far less than {fade} per Ah over {ah_past} Ah"
    );
}
