//! The many-particle cell (`CellModelConfig::SpmEnsemble`, Phase 9 slice B).
//!
//! Two kinds of chemistry here, on purpose:
//!
//! * The shipped **LG M50**, with its table potentials, for the guard that one particle *is*
//!   the single-particle model: bit for bit, on every telemetry field and every cell view,
//!   through the cases the single-particle model's own tests cover.
//! * A **slice-B fixture** LFP cell for everything with more than one particle: the shipped
//!   `lfp_26650_generic.toml` with an `[spm]` block built here from the Phase 9 spike's
//!   parameters (Prada2013's A123 cell, the Kashkooli2017 LFP kinetics, the regular-solution
//!   LFP potential at the fitted `Ω`). It stands in for the chemistry file slice C extracts
//!   with `tools/reference/`, and nothing here is a claim about that file.
//!
//! What slice B measured that is too slow for a debug test — the rest gaps, the C/20
//! plateau, the long-step monotonicity sweep — is in `docs/plans/phase-9-slice-b-ensemble.md`
//! with its harness.

use sim_core::{
    BuildError, CellModelConfig, ChemistryParams, Demand, ElectrodeParams, Env, EventFlags,
    OcpTable, Pack, PackConfig, RegularSolutionParams, Scatter, SpmParams, Telemetry,
    ThermalConfig,
};
use sim_data::{parse_chemistry, parse_scenario};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

const FARADAY: f64 = 96_485.332_123_310_01;

fn env(t: f64) -> Env {
    Env {
        t_ambient: t,
        t_coolant: None,
    }
}

fn config(
    model: CellModelConfig,
    (series, parallel): (u16, u16),
    soc: f64,
    sigma: f64,
    thermal: ThermalConfig,
) -> PackConfig {
    PackConfig {
        series,
        parallel,
        initial_soc: soc,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter {
            capacity_sigma: sigma,
            r0_sigma: sigma,
        },
        thermal,
        bms: None,
        aging: None,
        cell_model: model,
    }
}

// --- the slice-B LFP fixture -----------------------------------------------------------

/// Prada2013's graphite potential, sampled onto a table: the function PyBaMM's
/// `Prada2013` set uses, as the spike's harness evaluated it.
fn graphite_v(x: f64) -> f64 {
    1.9793 * (-39.3631 * x).exp() + 0.2482
        - 0.0909 * (29.8538 * (x - 0.1234)).tanh()
        - 0.04478 * (14.9159 * (x - 0.2769)).tanh()
        - 0.0205 * (30.4444 * (x - 0.6103)).tanh()
}

/// The fitted interaction strength \[eV\] of `docs/plans/phase-9-lfp-ensemble.md`.
const OMEGA_EV: f64 = 0.075_917_388_974_677_12;

/// The fixture's capacity \[Ah\]: the negative window's geometric capacity, so the cell's
/// charge-to-flux factor is one.
const LFP_CAP_AH: f64 = 2.303_324_557_209_257;

fn lfp_fixture() -> ChemistryParams {
    let mut c = parse_chemistry(LFP).expect("LFP parses");
    c.cell.capacity_ah = LFP_CAP_AH;
    let xs: Vec<f64> = (0..=400).map(|k| f64::from(k) / 400.0).collect();
    c.spm = Some(SpmParams {
        t_ref_k: 298.15,
        c_e_mol_per_m3: 1200.0,
        electrode_area_m2: 0.18,
        contact_resistance_ohm: 0.0,
        negative: ElectrodeParams {
            particle_radius_m: 5e-6,
            diffusivity_m2_per_s: 3e-15,
            c_max_mol_per_m3: 30555.0,
            active_volume_fraction: 0.58,
            thickness_m: 3.4e-5,
            m_ref: 6.48e-7,
            reaction_ea_j_per_mol: 35000.0,
            diffusivity_ea_j_per_mol: 0.0,
            charge_transfer_alpha: 0.5,
            stoich_min: 0.017_617_931_791_027_094,
            stoich_max: 0.81,
            docp_dt_v_per_k: 0.0,
            ocp: OcpTable {
                volts: xs.iter().map(|&x| graphite_v(x)).collect(),
                stoich: xs,
            },
            regular_solution: None,
        },
        positive: ElectrodeParams {
            particle_radius_m: 5e-8,
            diffusivity_m2_per_s: 5.9e-18,
            c_max_mol_per_m3: 22806.0,
            active_volume_fraction: 0.374,
            thickness_m: 8e-5,
            m_ref: 6e-7,
            reaction_ea_j_per_mol: 39570.0,
            diffusivity_ea_j_per_mol: 0.0,
            charge_transfer_alpha: 0.5,
            stoich_min: 0.0038,
            stoich_max: 0.703_502_020_929_131_3,
            docp_dt_v_per_k: 0.0,
            // Required, and never read by the many-particle cell when the regular-solution
            // form is present.
            ocp: OcpTable {
                stoich: vec![0.0, 1.0],
                volts: vec![3.6, 3.3],
            },
            regular_solution: Some(RegularSolutionParams {
                u0_v: 3.42,
                omega_ev: OMEGA_EV,
            }),
        },
    });
    c
}

fn ensemble(particles: usize, sigma: f64) -> CellModelConfig {
    CellModelConfig::SpmEnsemble {
        shells: 20,
        particles,
        radius_sigma: sigma,
    }
}

fn lfp_pack(particles: usize, soc: f64) -> Pack {
    Pack::new(
        &config(
            ensemble(particles, 0.2),
            (1, 1),
            soc,
            0.0,
            ThermalConfig::Isothermal,
        ),
        lfp_fixture(),
    )
    .expect("the fixture builds")
}

/// The many-particle state of cell `(0, 0)`, read through a real serialization.
fn state(p: &Pack) -> serde_json::Value {
    let v = serde_json::to_value(p.snapshot()).expect("serializes");
    v["pack"]["groups"][0]["cells"][0]["model"]["SpmEnsemble"].clone()
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array()
        .expect("an array")
        .iter()
        .map(|x| x.as_f64().expect("a number"))
        .collect()
}

/// Volume-weighted mean of uniform shells, innermost first.
fn mean(c: &[f64]) -> f64 {
    let (mut s, mut w) = (0.0, 0.0);
    for (i, x) in c.iter().enumerate() {
        let vol = ((i + 1) as f64).powi(3) - (i as f64).powi(3);
        s += vol * x;
        w += vol;
    }
    s / w
}

/// Each positive particle's mean stoichiometry.
fn particle_stoich(p: &Pack) -> Vec<f64> {
    let s = state(p);
    let c = floats(&s["c_pos"]);
    let n = floats(&s["radii_m"]).len();
    let sh = c.len() / n;
    (0..n)
        .map(|k| mean(&c[k * sh..(k + 1) * sh]) / 22806.0)
        .collect()
}

// --- exit criterion 1: one particle is the single-particle model ----------------------

fn same(a: &Telemetry, b: &Telemetry, pa: &Pack, pb: &Pack, at: &str) {
    // JSON floats round-trip exactly, so equal text is equal bits (and `-0.0` prints).
    assert_eq!(
        serde_json::to_string(a).expect("telemetry serializes"),
        serde_json::to_string(b).expect("telemetry serializes"),
        "telemetry parted at {at}"
    );
    for s in 0..pa.series() as usize {
        for k in 0..pa.parallel() as usize {
            let (ca, cb) = (pa.cell(s, k).expect("cell"), pb.cell(s, k).expect("cell"));
            assert_eq!(
                serde_json::to_string(&ca).expect("view serializes"),
                serde_json::to_string(&cb).expect("view serializes"),
                "cell {s},{k} parted at {at}"
            );
            // Skipped by serde, so compared on its own.
            assert_eq!(
                ca.current_a.map(f64::to_bits),
                cb.current_a.map(f64::to_bits),
                "cell {s},{k} current parted at {at}"
            );
        }
    }
}

/// Drive two packs through the same program and require every reported bit to agree,
/// with a zero-length read ahead of every segment and a JSON snapshot round trip in the
/// middle of the run.
fn lockstep(mut a: Pack, mut b: Pack, program: &[(Demand, f64, usize)], name: &str) {
    for (seg, &(d, dt, n)) in program.iter().enumerate() {
        let (za, zb) = (
            a.step(0.0, Demand::Current(3.0), &env(298.15)),
            b.step(0.0, Demand::Current(3.0), &env(298.15)),
        );
        same(
            &za,
            &zb,
            &a,
            &b,
            &format!("{name} segment {seg} zero-length read"),
        );
        if seg == program.len() / 2 {
            let text = serde_json::to_string(&b.snapshot()).expect("serializes");
            b = Pack::restore(&serde_json::from_str(&text).expect("deserializes"))
                .expect("restores");
        }
        for k in 0..n {
            let (ta, tb) = (a.step(dt, d, &env(298.15)), b.step(dt, d, &env(298.15)));
            same(&ta, &tb, &a, &b, &format!("{name} segment {seg} step {k}"));
        }
    }
}

/// A named pack shape and the program it runs: name, topology, initial SOC, scatter, thermal
/// model, and `(demand, dt, steps)` segments.
type Case = (
    &'static str,
    (u16, u16),
    f64,
    f64,
    ThermalConfig,
    Vec<(Demand, f64, usize)>,
);

/// One particle, a table potential: the many-particle cell takes the single-particle
/// model's arithmetic in the same order, so a pack of each is the same pack. Through
/// empty and past it, past full, under voltage and power holds at one-second and one-hour
/// steps, in parallel groups with scatter, with the thermal network on, and across a
/// snapshot. A spread asked for on one particle draws nothing, so it is the same pack too.
#[test]
fn one_particle_is_the_single_particle_model_bit_for_bit() {
    let chem = || parse_chemistry(LGM50).expect("LG M50 parses");
    let net = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let cases: Vec<Case> = vec![
        (
            "through empty",
            (1, 1),
            0.3,
            0.0,
            ThermalConfig::Isothermal,
            vec![
                (Demand::Current(5.0), 10.0, 300),
                (Demand::Rest, 60.0, 30),
                (Demand::Current(-2.5), 60.0, 30),
            ],
        ),
        (
            "scattered 1S3P through empty",
            (1, 3),
            0.4,
            0.05,
            net,
            vec![(Demand::Current(20.0), 5.0, 400), (Demand::Rest, 600.0, 10)],
        ),
        (
            "past full and a voltage hold",
            (1, 2),
            0.9,
            0.05,
            // Isothermal: with the network on, the single-particle model runs this one into
            // runaway, which tests the thermal integrator rather than the two models.
            ThermalConfig::Isothermal,
            vec![
                (Demand::Current(-10.0), 10.0, 200),
                (Demand::Voltage(4.2), 30.0, 30),
                (Demand::Rest, 3600.0, 3),
            ],
        ),
        (
            "2S2P pulses and long holds",
            (2, 2),
            0.6,
            0.05,
            net,
            vec![
                (Demand::Current(15.0), 1.0, 30),
                (Demand::Rest, 1.0, 30),
                (Demand::Power(-20.0), 1.0, 30),
                (Demand::Rest, 100.0, 10),
                (Demand::Power(30.0), 3600.0, 2),
                (Demand::Voltage(7.0), 3600.0, 2),
            ],
        ),
    ];
    for (name, topo, soc, sigma, thermal, program) in cases {
        for (shells, radius_sigma) in [(20, 0.0), (10, 0.3)] {
            let spm = Pack::new(
                &config(CellModelConfig::Spm { shells }, topo, soc, sigma, thermal),
                chem(),
            )
            .expect("Spm builds");
            let ens = Pack::new(
                &config(
                    CellModelConfig::SpmEnsemble {
                        shells,
                        particles: 1,
                        radius_sigma,
                    },
                    topo,
                    soc,
                    sigma,
                    thermal,
                ),
                chem(),
            )
            .expect("SpmEnsemble builds");
            lockstep(spm, ens, &program, &format!("{name} ({shells} shells)"));
        }
    }
}

/// The two shipped single-particle scenarios, their packs exactly as authored and then with
/// the model swapped for one particle, driven the way their descriptions say a client
/// drives them: the pulse train's 60 s on and 600 s off, and the 3C discharge to its
/// cut-off and on past it.
#[test]
fn the_shipped_single_particle_scenarios_run_identically_on_one_particle() {
    for (file, program) in [
        (
            include_str!("../../../scenarios/pulse_train_spm.toml"),
            vec![
                (Demand::Current(5.0), 1.0, 60),
                (Demand::Rest, 1.0, 600),
                (Demand::Current(15.0), 1.0, 60),
                (Demand::Rest, 1.0, 600),
            ],
        ),
        (
            include_str!("../../../scenarios/cc_discharge_3c_spm.toml"),
            vec![(Demand::Current(15.0), 1.0, 1300), (Demand::Rest, 10.0, 60)],
        ),
    ] {
        let scenario = parse_scenario(file).expect("the scenario parses");
        let CellModelConfig::Spm { shells } = scenario.pack.cell_model else {
            panic!("a single-particle scenario");
        };
        let mut swapped = scenario.clone();
        swapped.pack.cell_model = CellModelConfig::SpmEnsemble {
            shells,
            particles: 1,
            radius_sigma: 0.0,
        };
        let chem = || parse_chemistry(LGM50).expect("LG M50 parses");
        lockstep(
            scenario.build_pack(chem()).expect("builds"),
            swapped.build_pack(chem()).expect("builds"),
            &program,
            scenario.meta.name.as_str(),
        );
    }
}

// --- construction ------------------------------------------------------------------------

#[test]
fn only_the_many_particle_cell_takes_a_regular_solution_potential() {
    let iso = ThermalConfig::Isothermal;
    for (model, name) in [
        (CellModelConfig::Spm { shells: 20 }, "Spm"),
        (
            CellModelConfig::Dfn {
                shells: 10,
                nodes_negative: 10,
                nodes_separator: 5,
                nodes_positive: 10,
            },
            "Dfn",
        ),
    ] {
        let mut chem = lfp_fixture();
        chem.dfn = parse_chemistry(LGM50).expect("LG M50 parses").dfn;
        match Pack::new(&config(model, (1, 1), 0.5, 0.0, iso), chem) {
            Err(BuildError::RegularSolutionNeedsEnsemble { model, .. }) => assert_eq!(model, name),
            other => panic!("{name} took a regular-solution potential: {other:?}"),
        }
    }
    // The equivalent circuit reads neither half-cell potential, so the same file runs it.
    assert!(Pack::new(
        &config(CellModelConfig::Ecm, (1, 1), 0.5, 0.0, iso),
        lfp_fixture()
    )
    .is_ok());
    // And the negative electrode cannot carry one at all.
    let mut chem = lfp_fixture();
    if let Some(spm) = chem.spm.as_mut() {
        spm.negative.regular_solution = Some(RegularSolutionParams {
            u0_v: 0.1,
            omega_ev: 0.0,
        });
    }
    assert!(chem.validate().is_err());
}

#[test]
fn a_particle_count_or_spread_out_of_range_is_refused() {
    let iso = ThermalConfig::Isothermal;
    for particles in [0, 65] {
        assert!(matches!(
            Pack::new(
                &config(ensemble(particles, 0.2), (1, 1), 0.5, 0.0, iso),
                lfp_fixture()
            ),
            Err(BuildError::BadParticleCount { .. })
        ));
    }
    for sigma in [-0.1, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            Pack::new(
                &config(ensemble(4, sigma), (1, 1), 0.5, 0.0, iso),
                lfp_fixture()
            ),
            Err(BuildError::BadRadiusSigma(_))
        ));
    }
}

/// The radii come from the pack's seed, after every scatter factor: the scatter a seed
/// gives is the same whatever model is chosen, the radii differ between cells and between
/// seeds, and a pack that asks for no spread leaves the generator where the
/// single-particle model leaves it.
#[test]
fn the_radius_draw_is_seeded_and_moves_no_other_draw() {
    let iso = ThermalConfig::Isothermal;
    let build = |model, seed| {
        let mut c = config(model, (1, 2), 0.5, 0.05, iso);
        c.seed = seed;
        Pack::new(&c, lfp_fixture()).expect("builds")
    };
    let rng =
        |p: &Pack| serde_json::to_value(p.snapshot()).expect("serializes")["pack"]["rng"].clone();
    let radii = |p: &Pack, k: usize| {
        let v = serde_json::to_value(p.snapshot()).expect("serializes");
        floats(&v["pack"]["groups"][0]["cells"][k]["model"]["SpmEnsemble"]["radii_m"])
    };
    let ecm = build(CellModelConfig::Ecm, 7);
    let ens = build(ensemble(20, 0.2), 7);
    for k in 0..2 {
        let (a, b) = (ecm.cell(0, k).expect("cell"), ens.cell(0, k).expect("cell"));
        assert_eq!(a.capacity_factor.to_bits(), b.capacity_factor.to_bits());
        assert_eq!(a.r0_factor.to_bits(), b.r0_factor.to_bits());
    }
    let (r0, r1) = (radii(&ens, 0), radii(&ens, 1));
    assert_ne!(r0, r1, "two cells drew the same radii");
    assert!(r0.iter().all(|&r| r > 0.0 && r.is_finite()));
    assert_eq!(
        r0,
        radii(&build(ensemble(20, 0.2), 7), 0),
        "not a function of the seed"
    );
    assert_ne!(
        r0,
        radii(&build(ensemble(20, 0.2), 8), 0),
        "the seed does not reach it"
    );
    // No spread, or one particle: nothing drawn, every radius the chemistry's.
    for model in [ensemble(20, 0.0), ensemble(1, 0.3)] {
        let p = build(model, 7);
        assert_eq!(rng(&p), rng(&ecm), "{model:?} moved the generator");
        assert!(radii(&p, 0).iter().all(|&r| r == 5e-8));
    }
    assert_ne!(rng(&ens), rng(&ecm), "the spread drew nothing");
}

// --- the many-particle cell ------------------------------------------------------------------

/// Lithium is conserved across the particles to rounding: the positive electrode gains
/// exactly what the current carried into it, whatever the split did with it — through a
/// discharge into the plateau, a rest in which the particles trade among themselves, and a
/// charge back. A split that dropped its residual would show here as a drift.
#[test]
fn the_particles_conserve_lithium() {
    let chem = lfp_fixture();
    let spm = chem.spm.clone().expect("[spm]");
    let vol_p =
        spm.positive.active_volume_fraction * spm.electrode_area_m2 * spm.positive.thickness_m;
    let vol_n =
        spm.negative.active_volume_fraction * spm.electrode_area_m2 * spm.negative.thickness_m;
    let kappa = (spm.negative.stoich_max - spm.negative.stoich_min)
        * spm.negative.c_max_mol_per_m3
        * vol_n
        * FARADAY
        / 3600.0
        / LFP_CAP_AH;
    let inventory = |p: &Pack| {
        let s = state(p);
        let c = floats(&s["c_pos"]);
        let n = floats(&s["radii_m"]).len();
        let sh = c.len() / n;
        (0..n)
            .map(|k| mean(&c[k * sh..(k + 1) * sh]) * vol_p / n as f64)
            .sum::<f64>()
    };
    let mut p = lfp_pack(20, 1.0);
    let n0 = inventory(&p);
    let mut charge_as = 0.0;
    let mut worst: f64 = 0.0;
    for (d, steps) in [
        (Demand::Current(LFP_CAP_AH), 1500),
        (Demand::Rest, 600),
        (Demand::Current(-0.5 * LFP_CAP_AH), 600),
    ] {
        for _ in 0..steps {
            let t = p.step(1.0, d, &env(298.15));
            assert!(
                !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "a split failed"
            );
            charge_as += t.i_actual;
            let moved = (inventory(&p) - n0) * FARADAY / kappa;
            worst = worst.max((moved - charge_as).abs() / (3600.0 * LFP_CAP_AH));
        }
    }
    assert!(
        worst < 1e-12,
        "the particles lost {worst:e} of the capacity"
    );
    // And the discharge did separate them, or this measured nothing.
    let y = particle_stoich(&p);
    let spread = y.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        - y.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(
        spread > 0.3,
        "the particles never separated: spread {spread}"
    );
}

/// The case that broke the first split: a hard charge into a nearly charged cell over a
/// minute, where the tangent passes ran to millions of amperes per particle in opposite
/// directions. The bracketed search takes over; the step converges and every particle stays
/// where a charge past full can put it, flagged as the single-particle model flags it.
#[test]
fn a_hard_charge_into_a_full_electrode_keeps_its_particles_bounded() {
    for (soc, c_rate) in [(0.95, -3.0), (0.97, -2.5), (0.99, -2.0)] {
        let mut p = lfp_pack(20, soc);
        let t = p.step(60.0, Demand::Current(c_rate * LFP_CAP_AH), &env(298.15));
        assert!(
            !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
            "soc {soc}, {c_rate} C: the split did not converge"
        );
        for y in particle_stoich(&p) {
            assert!(
                (-0.1..0.1).contains(&y),
                "soc {soc}, {c_rate} C: a particle reached stoichiometry {y}"
            );
        }
    }
}

/// A snapshot taken mid-rest, with the particles separated and still trading lithium,
/// survives JSON and continues bit for bit: the radii and the last shares are state.
#[test]
fn a_separated_ensemble_round_trips_through_json() {
    let mut p = lfp_pack(20, 1.0);
    for _ in 0..1800 {
        p.step(1.0, Demand::Current(LFP_CAP_AH), &env(298.15));
    }
    for _ in 0..30 {
        p.step(1.0, Demand::Rest, &env(298.15));
    }
    let text = serde_json::to_string(&p.snapshot()).expect("serializes");
    let mut q =
        Pack::restore(&serde_json::from_str(&text).expect("deserializes")).expect("restores");
    for d in [
        Demand::Rest,
        Demand::Current(0.3 * LFP_CAP_AH),
        Demand::Voltage(3.25),
    ] {
        for _ in 0..40 {
            let (a, b) = (p.step(1.0, d, &env(298.15)), q.step(1.0, d, &env(298.15)));
            assert_eq!(a.v_terminal.to_bits(), b.v_terminal.to_bits());
            assert_eq!(a.q_gen_w.to_bits(), b.q_gen_w.to_bits());
            assert_eq!(a.i_actual.to_bits(), b.i_actual.to_bits());
        }
    }
}

/// The energy ledger closes with the particles trading lithium at rest. During the rest no
/// current crosses the terminals, so `i·(U_eq − V)` books nothing — yet the stored energy
/// keeps falling as the particles relax towards each other, and that is heat. The cell
/// books it per particle (`ensemble::exchange_w`); without it the ledger here is short by
/// the whole of the rest's heat.
#[test]
fn the_energy_ledger_closes_through_a_rest() {
    let chem = lfp_fixture();
    let spm = chem.spm.clone().expect("[spm]");
    let t = 298.15;
    let kt = 8.314_462_618_153_24 * t / FARADAY;
    let phi_p =
        |y: f64| 3.42 * y - kt * (y * y.ln() + (1.0 - y) * (1.0 - y).ln()) - OMEGA_EV * (y - y * y);
    let tab = spm.negative.ocp.clone();
    let phi_n = |x: f64| {
        let mut s = 0.0;
        for k in 1..tab.stoich.len() {
            let (a, b) = (tab.stoich[k - 1], tab.stoich[k]);
            if x <= a {
                break;
            }
            let hi = x.min(b);
            let slope = (tab.volts[k] - tab.volts[k - 1]) / (b - a);
            s += tab.volts[k - 1] * (hi - a) + 0.5 * slope * (hi - a) * (hi - a);
        }
        s
    };
    let vol_p =
        spm.positive.active_volume_fraction * spm.electrode_area_m2 * spm.positive.thickness_m;
    let vol_n =
        spm.negative.active_volume_fraction * spm.electrode_area_m2 * spm.negative.thickness_m;
    // Shell by shell: a particle with a gradient holds a different energy from a uniform
    // one at the same mean.
    let stored = |p: &Pack| {
        let s = state(p);
        let (cp, cn) = (floats(&s["c_pos"]), floats(&s["c_neg"]));
        let np = floats(&s["radii_m"]).len();
        let sh = cn.len();
        let w = |i: usize| ((i + 1) as f64).powi(3) - (i as f64).powi(3);
        let total = (sh as f64).powi(3);
        let mut e = 0.0;
        for k in 0..np {
            for i in 0..sh {
                e -= FARADAY * 22806.0 * (vol_p / np as f64) * w(i) / total
                    * phi_p(cp[k * sh + i] / 22806.0);
            }
        }
        for (i, c) in cn.iter().enumerate() {
            e -= FARADAY * 30555.0 * vol_n * w(i) / total * phi_n(c / 30555.0);
        }
        e
    };
    let mut p = lfp_pack(20, 1.0);
    let e0 = stored(&p);
    let (mut elec, mut heat, mut rest_heat) = (0.0, 0.0, 0.0);
    for _ in 0..1800 {
        let tel = p.step(1.0, Demand::Current(LFP_CAP_AH), &env(t));
        elec += tel.v_terminal * tel.i_actual;
        heat += tel.q_gen_w;
    }
    for _ in 0..3600 {
        let tel = p.step(1.0, Demand::Rest, &env(t));
        heat += tel.q_gen_w;
        rest_heat += tel.q_gen_w;
    }
    let imbalance = (e0 - stored(&p)) - elec - heat;
    assert!(
        imbalance.abs() < 0.1 * rest_heat,
        "the ledger is {imbalance} J out against {rest_heat} J of rest heat"
    );
    assert!(
        rest_heat > 1.0,
        "the rest made {rest_heat} J: nothing was exchanged"
    );
}
