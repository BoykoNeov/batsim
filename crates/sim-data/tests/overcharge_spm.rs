//! Phase 13 slice A: the `Spm` charged on past full (`docs/plans/phase-13-past-full.md`).
//!
//! Before this slice a single-particle cell pushed past full put all of the charge into its
//! particles — the negative to 1.4–1.8 times what it can hold, the positive below empty — and
//! read `soc_true = 1.0` throughout (`docs/plans/phase-13-spike.md`). Now each particle stops at
//! its own edge: what a full negative cannot take deposits as lithium metal, and what an empty
//! positive cannot give — or what its potential drives past the electrolyte's onset — oxidises
//! the electrolyte. The plan registered eight predictions before the engine changed; this file
//! scores 2 to 7 as written (1 is the rest of the suite, 8 a timing, both in the plan's
//! addendum).
//!
//! Everything here is read off the **state** — the particles' shells, the metal, the oxidised
//! counter — and not off what the engine reports about it: a ledger built from telemetry alone
//! is an identity.

use sim_core::spm::{mean_concentration, ocp_lookup, DEFAULT_SHELLS, FARADAY_C_PER_MOL as F};
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, ElectrodeParams, Env, EventFlags, Pack, PackConfig,
    Scatter, SpmState, ThermalConfig,
};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

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
    let config = PackConfig {
        series: 1,
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
        cell_model: CellModelConfig::Spm {
            shells: DEFAULT_SHELLS,
        },
    };
    Pack::new(&config, chem()).expect("builds")
}

/// Every cell's `Spm` state, read out of a snapshot, group-major.
fn states(p: &Pack) -> Vec<SpmState> {
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    let mut out = Vec::new();
    for g in v["pack"]["groups"].as_array().expect("groups") {
        for c in g["cells"].as_array().expect("cells") {
            out.push(serde_json::from_value(c["model"]["Spm"].clone()).expect("an Spm cell"));
        }
    }
    out
}

/// Active-material volume \[m³\] of one electrode.
fn volume(c: &ChemistryParams, e: &ElectrodeParams) -> f64 {
    e.active_volume_fraction * c.spm.as_ref().expect("[spm]").electrode_area_m2 * e.thickness_m
}

/// The charge-to-lithium scale of an unscattered cell, geometric capacity over configured.
fn kappa(c: &ChemistryParams) -> f64 {
    let n = &c.spm.as_ref().expect("[spm]").negative;
    (n.stoich_max - n.stoich_min) * n.c_max_mol_per_m3 * volume(c, n) * F
        / 3600.0
        / c.cell.capacity_ah
}

/// The lithium book of one cell \[mol\]: `(negative, positive, metal, oxidised)`.
fn book(c: &ChemistryParams, s: &SpmState) -> (f64, f64, f64, f64) {
    let spm = c.spm.as_ref().expect("[spm]");
    (
        mean_concentration(&s.c_neg) * volume(c, &spm.negative),
        mean_concentration(&s.c_pos) * volume(c, &spm.positive),
        s.metal_mol,
        s.oxidised_mol,
    )
}

/// The furthest any shell of either particle sits outside `[0, c_max]`, as a fraction of
/// that particle's `c_max` (`0.0` when every shell is inside).
fn outside(c: &ChemistryParams, s: &SpmState) -> f64 {
    let spm = c.spm.as_ref().expect("[spm]");
    let worst = |cs: &[f64], cmax: f64| {
        cs.iter()
            .map(|&x| (-x).max(x - cmax).max(0.0) / cmax)
            .fold(0.0, f64::max)
    };
    worst(&s.c_neg, spm.negative.c_max_mol_per_m3)
        .max(worst(&s.c_pos, spm.positive.c_max_mol_per_m3))
}

/// What a BMS-off constant-current overcharge at `c_rate` did, step by step at 1 s, from
/// `soc = 0.95` until `push_ah` amp-hours past the start.
struct Overcharge {
    /// Worst [`outside`] over every step.
    worst_outside: f64,
    /// Worst `|Δbook| / charge passed` over every step, both in moles.
    worst_book: f64,
    /// The same book without the metal and the oxidised counter: what this slice's two terms
    /// carry, so a closed book is the terms doing work.
    worst_book_without: f64,
    /// Metal held and electrolyte oxidised at the end \[mol\].
    metal: f64,
    oxidised: f64,
    /// Steps on which the solve did not converge, and `soc_true` outside \[0, 1\].
    unconverged: usize,
    soc_out: usize,
    /// The pack left at the end, for what comes after.
    pack: Pack,
}

fn overcharge(c_rate: f64, push_ah: f64) -> Overcharge {
    let c = chem();
    let q = c.cell.capacity_ah;
    let k = kappa(&c);
    let mut p = pack(1, 0.95, 0.0, ThermalConfig::Isothermal);
    let s0 = &states(&p)[0];
    let (n0, p0, m0, o0) = book(&c, s0);
    let (mut worst_outside, mut worst_book, mut worst_book_without) = (0.0_f64, 0.0_f64, 0.0_f64);
    let (mut unconverged, mut soc_out) = (0, 0);
    let mut passed_mol = 0.0;
    for _ in 0..(push_ah * 3600.0 / (c_rate * q)).ceil() as usize {
        let t = p.step(1.0, Demand::Current(-c_rate * q), &env());
        passed_mol += c_rate * q * k / F;
        if t.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
            unconverged += 1;
        }
        if !(0.0..=1.0).contains(&t.soc_true) {
            soc_out += 1;
        }
        let s = &states(&p)[0];
        worst_outside = worst_outside.max(outside(&c, s));
        let (n, pp, m, o) = book(&c, s);
        let d = ((n + pp + m - o) - (n0 + p0 + m0 - o0)).abs();
        worst_book = worst_book.max(d / passed_mol);
        worst_book_without = worst_book_without.max(((n + pp) - (n0 + p0)).abs() / passed_mol);
    }
    let (_, _, metal, oxidised) = book(&c, &states(&p)[0]);
    Overcharge {
        worst_outside,
        worst_book,
        worst_book_without,
        metal,
        oxidised,
        unconverged,
        soc_out,
        pack: p,
    }
}

/// Predictions 2 and 3, at 1C and 3C: no shell of either particle outside `[0, c_max]` to
/// 1e-12 of `c_max` on any step, and negative + positive + metal − oxidised constant to 1e-9
/// of the charge passed. Pushed 3.5 A·h past the start, so both outlets run: the spike put the
/// negative's edge 0.4–0.8 A·h and the positive's 1.5–2.3 A·h past full.
#[test]
fn predictions_2_and_3_no_stock_outside_and_the_lithium_book_closes() {
    for rate in [1.0, 3.0] {
        let r = overcharge(rate, 3.5);
        println!(
            "{rate}C: worst outside {:.3e} of c_max; book {:.3e} of the charge passed \
             ({:.3e} without metal and oxidation); metal {:.4e} mol, oxidised {:.4e} mol; \
             unconverged {}, soc outside [0, 1] {}",
            r.worst_outside,
            r.worst_book,
            r.worst_book_without,
            r.metal,
            r.oxidised,
            r.unconverged,
            r.soc_out
        );
        assert!(
            r.metal > 0.0,
            "{rate}C: nothing deposited, so this proves nothing"
        );
        assert!(
            r.oxidised > 0.0,
            "{rate}C: nothing oxidised, so this proves nothing"
        );
        assert!(
            r.worst_outside <= 1e-12,
            "{rate}C: a shell sat {:.3e} of c_max outside its particle",
            r.worst_outside
        );
        assert!(
            r.worst_book <= 1e-9,
            "{rate}C: the lithium book is {:.3e} of the charge passed out",
            r.worst_book
        );
        assert!(
            r.worst_book_without > 1e3 * 1e-9,
            "{rate}C: the book closes without the two terms too ({:.3e}), so they carry nothing",
            r.worst_book_without
        );
        assert_eq!(r.soc_out, 0, "{rate}C: soc_true left [0, 1]");
    }
}

/// `∫₀ˣ U^E` over an electrode's stoichiometry on a fine trapezoid grid \[V\], `U^E` the
/// stored-energy potential (`U − t_ref·∂U/∂T`) the engine books heat against. Past either end
/// of the table the lookup is flat, as the engine's is.
struct Cum {
    dx: f64,
    c: Vec<f64>,
}

impl Cum {
    fn of(e: &ElectrodeParams, t_ref: f64) -> Self {
        let interp = |xs: &[f64], ys: &[f64], x: f64| {
            let n = xs.len();
            if x <= xs[0] {
                return ys[0];
            }
            if x >= xs[n - 1] {
                return ys[n - 1];
            }
            let k = xs.partition_point(|&v| v <= x).max(1) - 1;
            ys[k] + (x - xs[k]) / (xs[k + 1] - xs[k]) * (ys[k + 1] - ys[k])
        };
        let u = |x: f64| {
            let s = e
                .entropic
                .as_ref()
                .map_or(e.docp_dt_v_per_k, |t| interp(&t.stoich, &t.v_per_k, x));
            ocp_lookup(&e.ocp, x) - t_ref * s
        };
        let n = 200_000;
        let dx = 1.0 / n as f64;
        let mut c = vec![0.0; n + 1];
        for i in 1..=n {
            let (a, b) = ((i - 1) as f64 * dx, i as f64 * dx);
            c[i] = c[i - 1] + 0.5 * (u(a) + u(b)) * dx;
        }
        Cum { dx, c }
    }

    fn at(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let k = ((x / self.dx) as usize).min(self.c.len() - 2);
        let t = (x - k as f64 * self.dx) / self.dx;
        self.c[k] + t * (self.c[k + 1] - self.c[k])
    }
}

/// The cell's stored energy \[J\] against lithium metal, in the engine's charge units: each
/// particle's lithium at its electrode's `∫U^E`. Deposited metal is lithium at 0 V; the
/// oxidised electrolyte stores nothing.
fn stored_j(c: &ChemistryParams, s: &SpmState, cum: &(Cum, Cum)) -> f64 {
    let spm = c.spm.as_ref().expect("[spm]");
    let side = |prof: &[f64], e: &ElectrodeParams, cum: &Cum| {
        F * e.c_max_mol_per_m3
            * volume(c, e)
            * cum.at(mean_concentration(prof) / e.c_max_mol_per_m3)
    };
    (side(&s.c_neg, &spm.negative, &cum.0) + side(&s.c_pos, &spm.positive, &cum.1)) / kappa(c)
}

/// The energy ledger's residual \[J\] over a 1C overcharge from 0.95 pushed 3.5 A·h at steps of
/// `dt` — `ΔW_stored − ∫V·I − ∫Q` — and the outlets' share of the heat, estimated off the
/// state as `Σ(Δmetal·U_n^E − Δoxidised·U_p^E)·F/κ`: what the residual would carry if the
/// engine booked that share at the particles' equilibrium.
fn energy_residual(dt: f64) -> (f64, f64) {
    let c = chem();
    let spm = c.spm.clone().expect("[spm]");
    let q = c.cell.capacity_ah;
    let k = kappa(&c);
    let cum = (
        Cum::of(&spm.negative, spm.t_ref_k),
        Cum::of(&spm.positive, spm.t_ref_k),
    );
    let mut p = pack(1, 0.95, 0.0, ThermalConfig::Isothermal);
    let w0 = stored_j(&c, &states(&p)[0], &cum);
    let (mut elec, mut heat, mut path) = (0.0, 0.0, 0.0);
    let mut prev = states(&p)[0].clone();
    for _ in 0..(3.5 * 3600.0 / (q * dt)).ceil() as usize {
        let t = p.step(dt, Demand::Current(-q), &env());
        elec += t.v_terminal * t.i_actual * dt;
        heat += t.q_gen_w * dt;
        let s = states(&p)[0].clone();
        let slope = |cum: &Cum, x: f64| (cum.at(x + 1.0e-6) - cum.at(x - 1.0e-6)) / 2.0e-6;
        let x = mean_concentration(&s.c_neg) / spm.negative.c_max_mol_per_m3;
        let y = mean_concentration(&s.c_pos) / spm.positive.c_max_mol_per_m3;
        path += F / k
            * ((s.metal_mol - prev.metal_mol) * slope(&cum.0, x)
                - (s.oxidised_mol - prev.oxidised_mol) * slope(&cum.1, y));
        prev = s;
    }
    let released = stored_j(&c, &states(&p)[0], &cum) - w0;
    (released - elec - heat, path)
}

/// Prediction 4, the energy ledger, to the tolerance the `Spm`'s existing ledger test holds
/// (`porous_reversal.rs`: under 50 J at 1 s, and shrinking with the step). The control arm is
/// the outlets' share of the heat: without it booked against its own reactions the residual
/// would be far outside that.
#[test]
fn prediction_4_the_energy_ledger_closes() {
    let ((fine, path), (coarse, _)) = std::thread::scope(|s| {
        let a = s.spawn(|| energy_residual(1.0));
        let b = s.spawn(|| energy_residual(10.0));
        (a.join().unwrap(), b.join().unwrap())
    });
    println!(
        "residual {fine:.3} J at 1 s, {coarse:.3} J at 10 s; the outlets' share of the heat \
         {path:.1} J (the residual without it: {:.1} J)",
        fine - path
    );
    assert!(fine.abs() < 50.0, "the ledger is {fine} J out at 1 s");
    assert!(
        (fine - path).abs() > 10.0 * fine.abs(),
        "the outlets' share ({path} J) is not what closes the ledger"
    );
}

/// Prediction 5: a BMS-off overcharge reaches runaway under a thermal network, at 1C and 3C.
/// Onset not timed; the run stops at the flag or after three hours of charge.
#[test]
fn prediction_5_an_overcharge_reaches_runaway() {
    let c = chem();
    let q = c.cell.capacity_ah;
    for rate in [1.0, 3.0] {
        let mut p = pack(
            1,
            0.95,
            0.0,
            ThermalConfig::Network {
                k_neighbor_w_per_k: 0.0,
            },
        );
        let mut at = None;
        let mut t_max = 0.0_f64;
        for k in 0..10_800 {
            let t = p.step(1.0, Demand::Current(-rate * q), &env());
            t_max = t_max.max(t.t_max);
            if t.flags.contains(EventFlags::THERMAL_RUNAWAY) {
                at = Some(k + 1);
                break;
            }
        }
        println!(
            "{rate}C: runaway at {at:?} s ({:?} A·h charged), hottest {t_max:.1} K",
            at.map(|s| s as f64 * rate * q / 3600.0)
        );
        assert!(
            at.is_some(),
            "{rate}C: no runaway in three hours, hottest {t_max} K"
        );
    }
}

/// Prediction 6: a cell stopped at its edge gives the lithium back. After the 1C overcharge a
/// 1C discharge converges on every step, strips the metal before the negative particle gives
/// any lithium of its own, and reaches the cut-off.
#[test]
fn prediction_6_a_cell_stopped_at_its_edge_gives_the_lithium_back() {
    let c = chem();
    let q = c.cell.capacity_ah;
    let spm = c.spm.clone().expect("[spm]");
    let mut p = overcharge(1.0, 1.5).pack;
    let s = &states(&p)[0];
    assert!(s.metal_mol > 0.0, "the overcharge deposited nothing");
    let (mut unconverged, mut gave_early, mut steps) = (0, 0, 0);
    let mut last = mean_concentration(&s.c_neg);
    let mut v = f64::INFINITY;
    while v > c.cell.v_min && steps < 20_000 {
        let before = states(&p)[0].metal_mol;
        let t = p.step(1.0, Demand::Current(q), &env());
        v = t.v_terminal;
        steps += 1;
        if t.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
            unconverged += 1;
        }
        let s = &states(&p)[0];
        let now = mean_concentration(&s.c_neg);
        // While the step began with metal and ended with some left, the particle must not
        // have given lithium of its own.
        if before > 0.0 && s.metal_mol > 0.0 && now < last {
            gave_early += 1;
        }
        last = now;
    }
    let s = &states(&p)[0];
    println!(
        "discharged in {steps} s to {v:.4} V; metal left {:.3e} mol; unconverged {unconverged}; \
         negative gave lithium while metal remained on {gave_early} steps; bulk x {:.4}",
        s.metal_mol,
        mean_concentration(&s.c_neg) / spm.negative.c_max_mol_per_m3
    );
    assert_eq!(unconverged, 0);
    assert_eq!(gave_early, 0);
    assert_eq!(s.metal_mol, 0.0, "the metal was not stripped");
    assert!(v <= c.cell.v_min, "the discharge never reached the cut-off");
}

/// Prediction 7: the pack converges past full. A scattered 1S2P under a current demand and a
/// single cell under a 4.2 V hold, both started past full, at 1 s, 15 min and 1 h steps: no
/// unconverged step, every cell inside its edges.
#[test]
fn prediction_7_the_pack_converges_past_full() {
    let c = chem();
    let q = c.cell.capacity_ah;
    // Past full: a 1C-per-cell overcharge 1.5 A·h per cell past the start, at 1 s.
    let mut pair = pack(2, 0.95, 0.03, ThermalConfig::Isothermal);
    for _ in 0..(1.5 * 3600.0 / q) as usize {
        pair.step(1.0, Demand::Current(-2.0 * q), &env());
    }
    let single = overcharge(1.0, 1.5).pack;
    assert!(states(&pair).iter().all(|s| s.metal_mol > 0.0));
    let (pair0, single0) = (pair.snapshot(), single.snapshot());
    for dt in [1.0, 900.0, 3600.0] {
        for (name, snap, demand) in [
            (
                "1S2P charging at 1C a cell",
                &pair0,
                Demand::Current(-2.0 * q),
            ),
            ("1S2P discharging at C/2 a cell", &pair0, Demand::Current(q)),
            ("1S1P held at 4.2 V", &single0, Demand::Voltage(4.2)),
        ] {
            let mut p = Pack::restore(snap).expect("restores");
            let (mut unconverged, mut worst) = (0, 0.0_f64);
            for _ in 0..4 {
                let t = p.step(dt, demand, &env());
                if t.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
                    unconverged += 1;
                }
                for s in states(&p) {
                    worst = worst.max(outside(&c, &s));
                }
            }
            println!("{name}, dt {dt}: unconverged {unconverged} of 4, worst outside {worst:.3e}");
            assert_eq!(unconverged, 0, "{name} at dt {dt}");
            assert!(worst <= 1e-12, "{name} at dt {dt}: {worst:.3e} outside");
        }
    }
}

/// **The same flag, a different meaning — and since Phase 13 a second cause.** A single-particle
/// cell driven past the top of its window first only runs its *readout* past the window:
/// nothing is discarded, the lithium is in the particle, and `i_rejected_a` stays zero. Once a
/// particle reaches its edge an outlet takes what it cannot, and only the electrolyte's
/// oxidation is reported as rejected — charge that crossed the terminals and took no lithium
/// out of the positive. Deposited metal is stored, so it is not.
///
/// `Telemetry::i_rejected_a` is the equivalent circuit's conservation defect made visible
/// (`docs/plans/energy-hole.md`), and the flag alone does not say which model raised it, or
/// why. So this pins both halves: zero while the particle holds the charge, and exactly the
/// oxidation's current once that runs — never the deposition's, which a refactor booking every
/// outlet as rejected would burn as heat a second time. Moved here from `sim-core`'s
/// `spm_cell.rs`, whose `a_single_particle_clamps_its_readout_without_rejecting_charge` pinned
/// the first half alone, with the same drive (20 A from 0.98 for 400 s).
#[test]
fn a_single_particle_rejects_only_what_its_electrolyte_oxidises() {
    let mut p = pack(1, 0.98, 0.0, ThermalConfig::Isothermal);
    let (mut held_steps, mut depositing_steps, mut oxidising_steps) = (0, 0, 0);
    for _ in 0..400 {
        let t = p.step(1.0, Demand::Current(-20.0), &env());
        let s = &states(&p)[0];
        if s.i_deposit_last < 0.0 {
            depositing_steps += 1;
        }
        if s.i_oxidise_last == 0.0 {
            assert_eq!(
                t.i_rejected_a, 0.0,
                "nothing oxidised, yet {} A was reported rejected",
                t.i_rejected_a
            );
            if t.soc_true == 1.0 {
                held_steps += 1;
            }
        } else {
            oxidising_steps += 1;
            assert!(
                (t.i_rejected_a - s.i_oxidise_last).abs() <= 1e-12 * s.i_oxidise_last.abs(),
                "{} A reported rejected against an oxidation of {} A",
                t.i_rejected_a,
                s.i_oxidise_last
            );
        }
    }
    println!(
        "held past the readout {held_steps} steps, depositing {depositing_steps}, oxidising \
         {oxidising_steps}"
    );
    assert!(
        held_steps > 0,
        "never held charge past the readout: the first half proves nothing"
    );
    assert!(
        oxidising_steps > 0,
        "never oxidised: the second half proves nothing"
    );
    assert!(
        depositing_steps > 0,
        "never deposited: the deposit's exclusion proves nothing"
    );
}
