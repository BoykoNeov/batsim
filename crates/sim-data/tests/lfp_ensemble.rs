//! The shipped many-particle LFP chemistry, `lfp_26650_prada2013` (Phase 9 slice C).
//!
//! What the file claims and what the many-particle cell does with it, against the exit
//! criteria of `docs/plans/phase-9-lfp-ensemble.md`:
//!
//! * the extracted `[spm]` numbers, to the bit, with the two the extractor used to get wrong;
//! * the C/20 plateau against PyBaMM's SPM on the same set (criterion 2) — an **offset**, not a
//!   match, because PyBaMM's LFP potential is monotone and this cell's is not;
//! * the rest gap at 298 K and at 263 K (criteria 3 and 4), and the mechanism behind both: the
//!   resting voltage is set by **how many particles end full**;
//! * the long step (criterion 5): 15 min still falls with current at the state where the hour
//!   rises, and the hour's rise is pinned so a change to the cell cannot move the documented
//!   limit silently;
//! * no unsolved split at 1 C and 3 C on a 1 s step (criterion 6; C/20 is the plateau test).
//!
//! Every number quoted here was measured out of tree first, in release, with the harness under
//! `W:/temp/claude/phase9c/` — see `docs/plans/phase-9-slice-c-chemistry.md`. The tests run at
//! 10 s steps where the claim allows it, and the note records that the step length moves none
//! of them by more than the stated amount.

use sim_core::{
    BuildError, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig,
    Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LFP: &str = include_str!("../../../chemistries/lfp_26650_prada2013.toml");
const GOLDEN_C20: &str =
    include_str!("../../../tests/golden/lfp_26650_prada2013/spm_cc_c20_25c.csv");

/// The engine's constants (`sim_core::aging::GAS_CONSTANT_J_PER_MOL_K`,
/// `sim_core::spm::FARADAY_C_PER_MOL`), for the regular solution's `kT`.
const GAS_CONSTANT: f64 = 8.314_462_618_153_24;
const FARADAY: f64 = 96_485.332_123_310_01;

/// The four seeds the plan's criteria name (the spike's).
const SEEDS: [u64; 4] = [9, 1, 2, 3];

fn chem() -> ChemistryParams {
    parse_chemistry(LFP).expect("the many-particle LFP chemistry parses")
}

fn cap() -> f64 {
    chem().cell.capacity_ah
}

fn env(t: f64) -> Env {
    Env {
        t_ambient: t,
        t_coolant: None,
    }
}

/// One cell, 20 particles of lognormal radius (σ 0.2), 20 shells, held at `t`.
fn pack(seed: u64, soc: f64, t: f64) -> Pack {
    let cfg = PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: t,
        seed,
        scatter: Scatter {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        },
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::SpmEnsemble {
            shells: 20,
            particles: 20,
            radius_sigma: 0.2,
        },
    };
    Pack::new(&cfg, chem()).expect("the many-particle cell builds")
}

/// Each positive particle's mean stoichiometry, read through a real serialization.
fn particle_stoich(p: &Pack) -> Vec<f64> {
    let v = serde_json::to_value(p.snapshot()).expect("serializes");
    let s = &v["pack"]["groups"][0]["cells"][0]["model"]["SpmEnsemble"];
    let floats = |v: &serde_json::Value| -> Vec<f64> {
        v.as_array()
            .expect("an array")
            .iter()
            .map(|x| x.as_f64().expect("a number"))
            .collect()
    };
    let c = floats(&s["c_pos"]);
    let n = floats(&s["radii_m"]).len();
    let sh = c.len() / n;
    let c_max = chem().spm.expect("[spm]").positive.c_max_mol_per_m3;
    (0..n)
        .map(|k| {
            // Volume-weighted mean of uniform shells, innermost first.
            let (mut s, mut w) = (0.0, 0.0);
            for (i, x) in c[k * sh..(k + 1) * sh].iter().enumerate() {
                let vol = ((i + 1) as f64).powi(3) - (i as f64).powi(3);
                s += vol * x;
                w += vol;
            }
            s / w / c_max
        })
        .collect()
}

// --- the file -----------------------------------------------------------------------------

/// The `[spm]` section is what `tools/reference/extract_spm.py` emitted, to the bit, and the
/// two values the extractor first got wrong are the right ones.
///
/// * The positive rate is written `6 * 10 ** (-7)` inside PyBaMM's function, and a pattern
///   that read the leading number would have put **6** here — ten million times the rate,
///   with every load-time check passing.
/// * `t_ref_k` is the temperature the exchange-current functions are written at (298.15), not
///   the set's `Reference temperature [K]` key (298): the engine reads `t_ref_k` only in that
///   Arrhenius factor.
///
/// Exact, because every one is a literal of the set, a stated product, or (Ω) a fit that is
/// reproducible to the last bit; a tolerance could not see one of them change by one ULP.
#[test]
fn the_extracted_section_is_the_extractors_output_to_the_bit() {
    let c = chem();
    assert_eq!(c.meta.id, "lfp_26650_prada2013");
    assert_eq!(c.cell.capacity_ah.to_bits(), 2.303_451_f64.to_bits());
    let spm = c.spm.expect("[spm]");
    let (n, p) = (&spm.negative, &spm.positive);
    for (what, got, want) in [
        ("t_ref_k", spm.t_ref_k, 298.15),
        ("c_e", spm.c_e_mol_per_m3, 1200.0),
        ("area", spm.electrode_area_m2, 0.18),
        ("neg m_ref", n.m_ref, 6.48e-7),
        ("pos m_ref", p.m_ref, 6e-7),
        ("neg E_r", n.reaction_ea_j_per_mol, 35_000.0),
        ("pos E_r", p.reaction_ea_j_per_mol, 39_570.0),
        ("neg x_min", n.stoich_min, 0.017_617_931_791_027_226),
        ("neg x_max", n.stoich_max, 0.810_043_495_265_194_7),
        ("pos y_min", p.stoich_min, 0.003_761_592_107_925_635_2),
        ("pos y_max", p.stoich_max, 0.703_502_020_929_131_3),
        ("pos radius", p.particle_radius_m, 5e-8),
        ("pos D", p.diffusivity_m2_per_s, 5.9e-18),
    ] {
        assert_eq!(got.to_bits(), f64::to_bits(want), "{what}: {got} != {want}");
    }
    let rs = p.regular_solution.as_ref().expect("the regular solution");
    assert_eq!(rs.u0_v.to_bits(), 3.42_f64.to_bits());
    assert_eq!(
        rs.omega_ev.to_bits(),
        0.075_917_388_973_750_97_f64.to_bits()
    );
    // Beside a regular solution the scalar entropic coefficient must be zero (the form
    // carries its own temperature dependence; validation refuses anything else).
    assert_eq!(p.docp_dt_v_per_k, 0.0);

    // And the whole section, tables included, as `spm_exact_bits.rs` pins the LG M50's: the
    // scalars above are the ones with a story; this is every number.
    let mut values = vec![
        spm.t_ref_k,
        spm.c_e_mol_per_m3,
        spm.electrode_area_m2,
        spm.contact_resistance_ohm,
    ];
    for e in [n, p] {
        values.extend([
            e.particle_radius_m,
            e.diffusivity_m2_per_s,
            e.c_max_mol_per_m3,
            e.active_volume_fraction,
            e.thickness_m,
            e.m_ref,
            e.reaction_ea_j_per_mol,
            e.diffusivity_ea_j_per_mol,
            e.charge_transfer_alpha,
            e.stoich_min,
            e.stoich_max,
            e.docp_dt_v_per_k,
        ]);
        values.extend(e.ocp.stoich.iter().copied());
        values.extend(e.ocp.volts.iter().copied());
    }
    values.extend([rs.u0_v, rs.omega_ev]);
    assert_eq!(
        values.len(),
        4 + 2 * 12 + (47 + 47) + (43 + 43) + 2,
        "the [spm] value count changed: extend this list before repinning"
    );
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    assert_eq!(
        h, 0xf6f9_361f_1360_aab3,
        "a number in chemistries/lfp_26650_prada2013.toml's [spm] section changed. Allowed (a \
         re-extraction is exactly the case), but deliberate: say which and why, then repin. \
         Do not delete the assertion."
    );
}

/// Ω is the fit it says it is: the regular-solution curve's turning points sit 20 mV apart at
/// 298.15 K, to well under a microvolt. Evaluated here, independently of the extractor.
#[test]
fn the_fitted_interaction_puts_the_turning_points_twenty_millivolts_apart() {
    let rs = chem()
        .spm
        .expect("[spm]")
        .positive
        .regular_solution
        .expect("rs");
    let kt = GAS_CONSTANT * 298.15 / FARADAY;
    let u = |y: f64| rs.u0_v - kt * (y / (1.0 - y)).ln() - rs.omega_ev * (1.0 - 2.0 * y);
    let y = 0.5 - (0.25 - kt / (2.0 * rs.omega_ev)).sqrt();
    let gap = u(1.0 - y) - u(y);
    assert!((gap - 0.020).abs() < 1e-9, "turning points {gap} V apart");
}

/// The charge the configured capacity says the cell holds is the charge its negative
/// electrode's geometry holds between its limits, to the fit's rounding: the many-particle
/// cell scales its currents by their ratio, so a mismatch would be a hidden rate error.
#[test]
fn the_capacity_is_the_negative_electrodes_geometry() {
    let c = chem();
    let spm = c.spm.expect("[spm]");
    let n = &spm.negative;
    let vol = n.active_volume_fraction * spm.electrode_area_m2 * n.thickness_m;
    let geometric = (n.stoich_max - n.stoich_min) * n.c_max_mol_per_m3 * vol * FARADAY / 3600.0;
    let rel = (geometric / c.cell.capacity_ah - 1.0).abs();
    assert!(
        rel < 1e-5,
        "geometric {geometric} Ah against {} Ah",
        c.cell.capacity_ah
    );
}

/// The single-particle model and the full electrolyte model refuse this file: one particle
/// cannot carry a non-monotone potential (the spike), so the section that makes the file
/// what it is also closes those two models to it. The equivalent circuit runs it.
#[test]
fn only_the_many_particle_cell_and_the_circuit_run_this_file() {
    let mut cfg = PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: 0.5,
        initial_temp_k: 298.15,
        seed: 9,
        scatter: Scatter {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        },
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Spm { shells: 20 },
    };
    assert!(matches!(
        Pack::new(&cfg, chem()),
        Err(BuildError::RegularSolutionNeedsEnsemble { .. })
    ));
    cfg.cell_model = CellModelConfig::Ecm;
    assert!(Pack::new(&cfg, chem()).is_ok());
}

// --- criterion 2: the plateau -----------------------------------------------------------------

/// C/20 from full against PyBaMM's SPM on the same set: the many-particle cell sits **above**
/// it by a near-constant offset across 20–80 %, and spans about as much.
///
/// PyBaMM has no phase-separating LFP model, so its SPM runs the set's monotone potential
/// (Afshar 2017, mean ≈ 3.40 V over the plateau) where this cell runs a regular solution
/// centred on Bai 2011's U0 = 3.42 V — kept, not fitted, by owner decision. The offset is that
/// choice, so the test is a band on it rather than a match.
///
/// **The bands are the seed spread, not chosen to pass.** Measured over 20 seeds at 10 s steps
/// (harness, interpolated every step): mean offset +9.49 … +9.84 mV, span 137.40 … 138.96 mV
/// (PyBaMM's own: 145.41). Each band is that range widened by its own width on both sides:
/// +9.14 … +10.19 mV and 135.8 … 140.5 mV. The spike read +10.3 mV against its own PyBaMM
/// run; the two references agree to 0.03 mV, and the engine on slice B's fixture reads +9.80,
/// so the shift is the engine against the spike's toy (~0.5 mV) and the shipped graphite table
/// against the fixture's (0.16 mV), not the reference.
///
/// **Step length.** 10 s here, one sub-step, the real-time path with a longer backward-Euler
/// step. Against 1 s the mean offset moves by ≤ 0.05 mV and the span by ≤ 0.2 mV (seeds
/// 9/1/2/3), and 1 s and 10 s cost the same per step — so the test is a tenth of the work.
/// The cell is compared at the reference's own rows (every 30 s), with no interpolation.
#[test]
fn the_plateau_sits_above_pybamm_by_the_cited_u0() {
    let rows: Vec<[f64; 4]> = GOLDEN_C20
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("time_s") && !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l
                .split(',')
                .map(|x| x.trim().parse().expect("a number"))
                .collect();
            [v[0], v[1], v[2], v[3]]
        })
        .collect();
    let i = 0.05 * cap();
    // The reference's current is the same C/20, to the CSV's six decimals.
    assert!((rows[1][1] - i).abs() < 1e-6);
    for seed in SEEDS {
        let mut p = pack(seed, 1.0, 298.15);
        let (mut sum, mut n) = (0.0, 0usize);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for row in &rows[1..] {
            let mut v = f64::NAN;
            for _ in 0..3 {
                let t = p.step(10.0, Demand::Current(i), &env(298.15));
                assert!(
                    !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                    "seed {seed}: a split failed at {} s",
                    row[0]
                );
                v = t.v_terminal;
            }
            if (0.2..=0.8).contains(&row[3]) {
                sum += v - row[2];
                n += 1;
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        let offset_mv = sum / n as f64 * 1e3;
        let span_mv = (hi - lo) * 1e3;
        println!("seed {seed}: mean offset {offset_mv:+.3} mV, span {span_mv:.3} mV over {n} rows");
        assert!(
            (9.14..=10.19).contains(&offset_mv),
            "seed {seed}: mean offset {offset_mv} mV"
        );
        assert!(
            (135.8..=140.5).contains(&span_mv),
            "seed {seed}: 20-80 % span {span_mv} mV"
        );
    }
}

// --- criteria 3 and 4: the rest gap ---------------------------------------------------------

/// The resting positive potential of an ensemble of `n` equal-volume particles, `k` of them
/// full, mean stoichiometry `ybar`, at `t`: every empty particle at one composition and every
/// full one at another, at one potential. Solved here from the regular solution alone —
/// independent of the engine — by bisection on the empty composition.
///
/// `None` when no two-phase state with `k` full exists (both compositions must sit outside
/// the spinodal).
fn count_table(k: usize, n: usize, ybar: f64, t: f64) -> Option<f64> {
    let rs = chem()
        .spm
        .expect("[spm]")
        .positive
        .regular_solution
        .expect("rs");
    let kt = GAS_CONSTANT * t / FARADAY;
    let u = |y: f64| rs.u0_v - kt * (y / (1.0 - y)).ln() - rs.omega_ev * (1.0 - 2.0 * y);
    if k == 0 || k == n {
        return Some(u(ybar));
    }
    let (nf, kf) = (n as f64, k as f64);
    let s1 = 0.5 - (0.25 - kt / (2.0 * rs.omega_ev)).sqrt();
    let hi_of = |ylo: f64| (nf * ybar - (nf - kf) * ylo) / kf;
    let f = |ylo: f64| u(hi_of(ylo)) - u(ylo);
    // The empty composition below the lower spinodal edge, the full one above the upper.
    let mut a = 1e-12_f64.max((nf * ybar - kf * (1.0 - 1e-12)) / (nf - kf));
    let mut b = s1.min((nf * ybar - kf * (1.0 - s1)) / (nf - kf));
    if b <= a || f(a) >= 0.0 || f(b) <= 0.0 {
        return None;
    }
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if f(m) < 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    Some(u(0.5 * (a + b)))
}

/// One arm: C/20 from `soc0` for half the capacity, then a 2 h rest, at 10 s steps. Returns
/// the resting terminal voltage and each particle's stoichiometry.
fn arm(seed: u64, t: f64, soc0: f64, c_rate: f64) -> (f64, Vec<f64>) {
    let mut p = pack(seed, soc0, t);
    let i = c_rate * cap();
    for _ in 0..3600 {
        let tel = p.step(10.0, Demand::Current(i), &env(t));
        assert!(!tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    }
    let mut v = f64::NAN;
    for _ in 0..720 {
        let tel = p.step(10.0, Demand::Rest, &env(t));
        assert!(!tel.flags.contains(EventFlags::SOLVE_UNCONVERGED));
        v = tel.v_terminal;
    }
    (v, particle_stoich(&p))
}

/// Both arms at one seed and temperature: `(gap [V], the count table's gap [V], full counts)`,
/// asserting on the way that every particle has finished switching — none is left inside the
/// spinodal, where the count table does not apply.
fn gap_and_table(seed: u64, t: f64) -> (f64, f64, (usize, usize)) {
    let (vd, yd) = arm(seed, t, 1.0, 0.05);
    let (vc, yc) = arm(seed, t, 0.0, -0.05);
    let rs = chem()
        .spm
        .expect("[spm]")
        .positive
        .regular_solution
        .expect("rs");
    let s1 = 0.5 - (0.25 - GAS_CONSTANT * t / FARADAY / (2.0 * rs.omega_ev)).sqrt();
    for y in yd.iter().chain(&yc) {
        assert!(
            *y < s1 || *y > 1.0 - s1,
            "seed {seed} at {t} K: a particle at {y} has not finished switching"
        );
    }
    let mean = |ys: &[f64]| ys.iter().sum::<f64>() / ys.len() as f64;
    let full = |ys: &[f64]| ys.iter().filter(|&&y| y > 0.5).count();
    let (kd, kc) = (full(&yd), full(&yc));
    let table = count_table(kc, 20, mean(&yc), t).expect("a two-phase state")
        - count_table(kd, 20, mean(&yd), t).expect("a two-phase state");
    (vc - vd, table, (kd, kc))
}

/// **Criterion 3.** Approach SOC 0.5 at C/20 from each side, rest 2 h at 298 K: the
/// charge-arrival voltage sits above the discharge-arrival one by 18.0–20.0 mV on each of the
/// four seeds, and the gap is the count table's for the two arms' full counts.
///
/// Measured 19.64 mV on all four (discharge 4 of 20 full, charge 8). The room-temperature gap
/// is discrete as well, as the plan's table recorded — and over 20 seeds it is not always in
/// this band: 19.64 mV on 16, 18.43 on 3 (5 full on discharge) and **14.14 mV on one** (seed
/// 17: 7 full on charge), each equal to the count table, unchanged at 1 s, 10 s and 60 s
/// steps and after an 8 h rest. The band is the plan's, on the plan's four seeds, chosen
/// before this measurement; seed 17 is recorded in the slice note, not tested away.
#[test]
fn the_room_temperature_rest_gap_depends_on_the_direction_of_arrival() {
    for seed in SEEDS {
        let (gap, table, (kd, kc)) = gap_and_table(seed, 298.15);
        println!(
            "298 K seed {seed}: gap {:.3} mV (count table {:.3}, {kd} | {kc} full)",
            gap * 1e3,
            table * 1e3
        );
        assert!((0.0180..=0.0200).contains(&gap), "seed {seed}: gap {gap} V");
        assert!(
            (gap - table).abs() < 1e-4,
            "seed {seed}: gap {gap} V, count table {table} V"
        );
    }
}

/// **Criterion 4.** The same in the cold, 263 K: the gap is one of a few fixed values, set by
/// how many particles each arm leaves full — equal to the count table to 0.1 mV, and never
/// negative.
///
/// Measured 27.44 mV on seeds 9/1/2 (5 | 8 full) and 23.32 mV on seed 3 (6 | 8). Over 20
/// seeds: 27.44 on 11, 23.32 on 4, 17.48 on 4, 13.35 on 1 — one of the eleven (seed 0) only
/// after 4 h: at 2 h it read 27.18 mV, with two particles still switching.
/// That is why every particle is checked to be out of the spinodal before the table is.
#[test]
fn the_cold_rest_gap_is_set_by_how_many_particles_end_full() {
    for seed in SEEDS {
        let (gap, table, (kd, kc)) = gap_and_table(seed, 263.15);
        println!(
            "263 K seed {seed}: gap {:.3} mV (count table {:.3}, {kd} | {kc} full)",
            gap * 1e3,
            table * 1e3
        );
        assert!(
            gap >= 0.0,
            "seed {seed}: discharge-arrival above charge-arrival"
        );
        assert!(
            (gap - table).abs() < 1e-4,
            "seed {seed}: gap {gap} V, count table {table} V"
        );
    }
}

// --- criterion 5: the long step --------------------------------------------------------------

/// The pack brackets a root on each cell's end-of-step voltage against current, assuming it
/// falls. On this cell it does at 15 min and does not at an hour — the documented limit
/// (`CLAUDE.md`, pack solve; `phase-9-lfp-ensemble.md` §"Long steps").
///
/// Measured over 20 states × 61 currents (harness `mono`): falls everywhere at 1 s, 60 s and
/// 15 min; at 1 h rises at 15 of 20 states (61 of the 1 200 neighbouring pairs), by up to
/// 6.11 mV, almost all at charging currents between −0.83 C and −0.18 C — the two on the
/// discharge side are 0.03 and 0.11 mV, near full. The largest is at SOC 0.65 between
/// −0.267 C and −0.25 C, where an
/// hour's charge walks the particles across one of the potential's teeth and the graphite's
/// slope no longer outweighs it.
///
/// Pinned here at that state, reached at 10 s steps (where the rise reads 6.72 mV between
/// −0.26 C and −0.25 C): the hour must still rise, by the measured amount within 2 mV, and
/// 15 min must fall across −0.40 … −0.10 C at the same state. If a change to the cell removes
/// the rise, this fails on purpose — the limit in `CLAUDE.md` has to move with it.
#[test]
fn the_hour_step_rises_where_fifteen_minutes_still_falls() {
    let mut p = pack(9, 1.0, 298.15);
    for _ in 0..2520 {
        p.step(10.0, Demand::Current(0.05 * cap()), &env(298.15));
    }
    let snap = p.snapshot();
    let probe = |c_rate: f64, dt: f64| {
        let mut q = Pack::restore(&snap).expect("restores");
        let t = q.step(dt, Demand::Current(c_rate * cap()), &env(298.15));
        assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
        t.v_terminal
    };
    let rise = probe(-0.25, 3600.0) - probe(-0.26, 3600.0);
    println!("1 h: V(-0.25 C) - V(-0.26 C) = {:+.4} mV", rise * 1e3);
    assert!(
        (0.0047..=0.0087).contains(&rise),
        "the hour step rose by {rise} V (measured 6.72 mV)"
    );
    let mut last = f64::INFINITY;
    for k in 0..=30 {
        let c = -0.40 + 0.01 * f64::from(k);
        let v = probe(c, 900.0);
        assert!(v < last, "15 min: V rose to {v} at {c:+.2} C");
        last = v;
    }
}

// --- criterion 6: no unsolved split ----------------------------------------------------------

/// 1 C and 3 C from full to the 2.0 V cut-off at 1 s steps, seed 9: no split goes unsolved and
/// no particle surface leaves its range. (C/20 is covered at 10 s by the plateau test, and was
/// measured at 1 s out of tree on four seeds: 0 unsolved, 0 out of range.)
#[test]
fn no_split_goes_unsolved_at_one_and_three_c() {
    for c_rate in [1.0, 3.0] {
        let mut p = pack(9, 1.0, 298.15);
        let mut steps = 0;
        loop {
            let t = p.step(1.0, Demand::Current(c_rate * cap()), &env(298.15));
            steps += 1;
            assert!(
                !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "{c_rate} C: unsolved at {steps} s"
            );
            assert!(
                !t.flags.contains(EventFlags::SURFACE_OUT_OF_RANGE),
                "{c_rate} C: a surface out of range at {steps} s"
            );
            if t.v_terminal < 2.0 {
                break;
            }
            assert!(steps < 4000, "{c_rate} C never reached the cut-off");
        }
        println!("{c_rate} C: cut-off after {steps} s");
    }
}
