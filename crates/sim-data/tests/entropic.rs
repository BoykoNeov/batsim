//! `[spm.*.entropic]`: an electrode's entropy coefficient as a table, shifting its potential
//! with temperature and generating reversible heat in the porous models (Phase 11 slice E,
//! decision 3 of `docs/plans/phase-11-fitting.md`).
//!
//! The shipped LG M50 carries no table yet (slice C adds ORegan2022's), so this file gives
//! it a synthetic one — linear in stoichiometry, a few tenths of a millivolt per kelvin, the
//! scale ORegan2022 publishes — and asserts what the table must do whatever its values are.
//! Predicted before the run:
//!
//! * **At the tables' reference temperature the voltage does not move, to the bit**: the
//!   shift is `(T − t_ref)·∂U/∂T = 0·∂U/∂T`, and adding a signed zero changes nothing.
//! * **Away from it a rested cell reads `U + (T − t_ref)·(∂U_p/∂T − ∂U_n/∂T)`** at its own
//!   stoichiometries, on the `Spm` and the `Dfn` alike, to 1e-12 V.
//! * **The energy ledger closes as well with the term as without it** — within 5 J over a
//!   half-hour 1C discharge at −10 and +45 °C, against several hundred joules of reversible
//!   heat — once stored energy is counted at `U − T·∂U/∂T`, the potential the heat implies.

use sim_core::chem::EntropicTable;
use sim_core::spm::ocp_lookup;
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const T_REF: f64 = 298.15;

// The synthetic tables, linear over the whole stoichiometry range [V/K].
const NEG: [f64; 2] = [-3.0e-4, 1.0e-4];
const POS: [f64; 2] = [2.0e-4, -1.0e-4];

fn plain() -> ChemistryParams {
    sim_data::parse_chemistry(LGM50).expect("the shipped LG M50 parses")
}

fn with_tables() -> ChemistryParams {
    let mut c = plain();
    let spm = c.spm.as_mut().expect("[spm]");
    assert_eq!(spm.t_ref_k, T_REF, "the file's reference temperature moved");
    spm.negative.entropic = Some(EntropicTable {
        stoich: vec![0.0, 1.0],
        v_per_k: NEG.to_vec(),
    });
    spm.positive.entropic = Some(EntropicTable {
        stoich: vec![0.0, 1.0],
        v_per_k: POS.to_vec(),
    });
    c.validate().expect("the tables validate");
    c
}

fn lerp(ends: [f64; 2], s: f64) -> f64 {
    ends[0] + s * (ends[1] - ends[0])
}

fn spm_model() -> CellModelConfig {
    CellModelConfig::Spm { shells: 20 }
}

fn dfn_model() -> CellModelConfig {
    CellModelConfig::Dfn {
        shells: 10,
        nodes_negative: 10,
        nodes_separator: 5,
        nodes_positive: 10,
    }
}

fn pack(chem: ChemistryParams, model: CellModelConfig, soc: f64, temp_k: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 3,
        scatter: Scatter::default(),
        cell_model: model,
    };
    Pack::new(&config, chem).expect("builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

/// The voltage a fresh, rested pack reads after one second at rest.
fn rested_v(chem: ChemistryParams, model: CellModelConfig, temp_k: f64) -> f64 {
    let mut p = pack(chem, model, 0.6, temp_k);
    p.step(1.0, Demand::Rest, &env(temp_k)).v_terminal
}

/// The stoichiometries `initial_soc` puts each electrode at.
fn stoich_at(c: &ChemistryParams, soc: f64) -> (f64, f64) {
    let spm = c.spm.as_ref().expect("[spm]");
    let (n, p) = (&spm.negative, &spm.positive);
    (
        n.stoich_min + soc * (n.stoich_max - n.stoich_min),
        p.stoich_max - soc * (p.stoich_max - p.stoich_min),
    )
}

#[test]
fn at_the_reference_temperature_the_voltage_does_not_move() {
    for model in [spm_model(), dfn_model()] {
        let a = rested_v(plain(), model, T_REF);
        let b = rested_v(with_tables(), model, T_REF);
        assert_eq!(a.to_bits(), b.to_bits(), "{model:?}: {a} against {b}");
    }
}

#[test]
fn away_from_it_a_rested_cell_shifts_by_its_coefficient() {
    let (x, y) = stoich_at(&plain(), 0.6);
    let coef = lerp(POS, y) - lerp(NEG, x);
    for model in [spm_model(), dfn_model()] {
        for temp_k in [263.15, 318.15] {
            let a = rested_v(plain(), model, temp_k);
            let b = rested_v(with_tables(), model, temp_k);
            let want = (temp_k - T_REF) * coef;
            assert!(
                ((b - a) - want).abs() < 1e-12,
                "{model:?} at {temp_k} K: shifted {} V, expected {want} V",
                b - a
            );
        }
    }
}

/// Stored energy \[J\] between two bulk positions `z0 > z1` of the negative particle
/// (fractions of its window), at each electrode's potential minus `t_ref·∂U/∂T` — the
/// potential the engine's heat implies. With `entropic = false`, the plain potentials.
fn released_j(c: &ChemistryParams, z0: f64, z1: f64, entropic: bool) -> f64 {
    let spm = c.spm.as_ref().expect("[spm]");
    let (n, p) = (&spm.negative, &spm.positive);
    let h = |z: f64| {
        let x = n.stoich_min + z * (n.stoich_max - n.stoich_min);
        let y = p.stoich_max - z * (p.stoich_max - p.stoich_min);
        let (dp, dn) = if entropic {
            (lerp(POS, y), lerp(NEG, x))
        } else {
            (0.0, 0.0)
        };
        (ocp_lookup(&p.ocp, y) - T_REF * dp) - (ocp_lookup(&n.ocp, x) - T_REF * dn)
    };
    let k = 100_000;
    let step = (z0 - z1) / k as f64;
    let mut s = 0.5 * (h(z0) + h(z1));
    for i in 1..k {
        s += h(z1 + i as f64 * step);
    }
    3600.0 * c.cell.capacity_ah * s * step
}

/// The negative particle's bulk position in its window, read off the snapshot.
fn bulk_position(c: &ChemistryParams, p: &Pack) -> f64 {
    let n = &c.spm.as_ref().expect("[spm]").negative;
    let snap = serde_json::to_value(p.snapshot()).expect("serializes");
    let shells: Vec<f64> = snap["pack"]["groups"][0]["cells"][0]["model"]["Spm"]["c_neg"]
        .as_array()
        .expect("an Spm cell's negative shells")
        .iter()
        .map(|v| v.as_f64().expect("a concentration"))
        .collect();
    let (mut s, mut w) = (0.0, 0.0);
    for (i, c_s) in shells.iter().enumerate() {
        let vol = ((i + 1) as f64).powi(3) - (i as f64).powi(3);
        s += vol * c_s / n.c_max_mol_per_m3;
        w += vol;
    }
    (s / w - n.stoich_min) / (n.stoich_max - n.stoich_min)
}

#[test]
fn the_energy_ledger_closes_with_the_term() {
    let i = plain().cell.capacity_ah; // 1C
    for temp_k in [263.15, 318.15] {
        let imbalance = |entropic: bool| {
            let c = if entropic { with_tables() } else { plain() };
            let mut p = pack(c.clone(), spm_model(), 0.8, temp_k);
            let z0 = bulk_position(&c, &p);
            let (mut elec, mut heat) = (0.0, 0.0);
            for _ in 0..1800 {
                let t = p.step(1.0, Demand::Current(i), &env(temp_k));
                elec += t.v_terminal * t.i_actual;
                heat += t.q_gen_w;
            }
            let z1 = bulk_position(&c, &p);
            (released_j(&c, z0, z1, entropic) - elec - heat, heat)
        };
        let (plain_err, plain_heat) = imbalance(false);
        let (err, heat) = imbalance(true);
        eprintln!("{temp_k} K: ledger {err:.3} J with tables, {plain_err:.3} J without; heat moved {:.1} J", heat - plain_heat);
        assert!(
            (err - plain_err).abs() < 5.0,
            "at {temp_k} K the ledger is {err} J out with the tables, {plain_err} J without"
        );
        // And the term is not a rounding error: the heat itself moved by hundreds of J.
        assert!(
            (heat - plain_heat).abs() > 100.0,
            "at {temp_k} K the tables moved the heat by only {} J",
            heat - plain_heat
        );
    }
}
