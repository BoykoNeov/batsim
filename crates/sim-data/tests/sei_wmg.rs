//! Phase 10, exit 5 of `docs/plans/phase-10-sei.md`: the engine's SEI film against the WMG LG
//! M50 cells it was fitted to, measured the way WMG measured them.
//!
//! The reference is `tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv`, written by
//! `tools/reference/fit_sei_wmg.py --table` from the WMG calendar-ageing data (CC-BY-4.0;
//! Kuzhiyil et al. 2025): each storage condition's capacity fade, less the 0 %-SOC cells' fade
//! at the same temperature, as growth after the condition's first post-storage test. Data
//! only — no fit, no model.
//!
//! On the engine's side a filmed `Spm` cell rests at the storage condition and is measured, at
//! every test day, on a copy of itself put through WMG's capacity steps at 25 °C: 1.67 A down
//! to 2.5 V, 30 min rest, 1.67 A up to 4.2 V with no hold, 30 min rest, 1.67 A down to 2.5 V,
//! the last discharge being the capacity. The fit converted lithium into that capacity with a
//! ratio (1.567) measured in PyBaMM; this test does not use it.

use sim_core::{
    AgingConfig, CalendarModel, CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack,
    PackConfig, Scatter, Telemetry, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const GOLDEN: &str = include_str!("../../../tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv");

/// WMG's capacity-test current \[A\]: C/3 of the 5 Ah cell.
const I_TEST_A: f64 = 1.67;
/// WMG's capacity-test temperature \[K\]. The test chamber's: the raw test files of cells
/// stored at 0 °C log 23.7–29 °C on the cell during the test.
const T_TEST_K: f64 = 298.15;

/// The chemistry under test, with the drawn cycle fade switched off. The stored cell never
/// cycles, so this changes nothing about its storage; it keeps the capacity test, run on a
/// copy, from wearing that copy by a mechanism the data's 0 %-SOC subtraction removes.
fn chem() -> ChemistryParams {
    let mut chem = parse_chemistry(LGM50).expect("the shipped LG M50 file parses");
    chem.aging.as_mut().expect("[aging]").cyc_fade_per_ah = 0.0;
    chem
}

/// A single filmed `Spm` cell at `soc`, at `temp_k`, on a live thermal network so it can be
/// brought to the test chamber's temperature for each test and back.
fn stored(soc: f64, temp_k: f64) -> Pack {
    let config = PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc,
        initial_temp_k: temp_k,
        seed: 1,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 0.0,
        },
        bms: None,
        aging: Some(AgingConfig {
            calendar: CalendarModel::Film,
            ..AgingConfig::default()
        }),
        cell_model: CellModelConfig::Spm {
            shells: sim_core::spm::DEFAULT_SHELLS,
        },
    };
    Pack::new(&config, chem()).expect("a filmed Spm pack builds")
}

fn env(temp_k: f64) -> Env {
    Env {
        t_ambient: temp_k,
        t_coolant: None,
    }
}

/// The flags a capacity test must not raise. Not the operating-point window: [`leg`] treats
/// that flag as the cut-off, so no step it keeps can carry it.
fn bad() -> EventFlags {
    EventFlags::SOLVE_UNCONVERGED | EventFlags::SURFACE_OUT_OF_RANGE
}

/// One constant-current leg at `i` \[A, discharge positive\] until the terminal voltage
/// reaches `cutoff_v`, the crossing found by bisecting the last step's length. Returns the
/// charge moved \[Ah\]; the flags of every step kept are added to `flags`. A leg that
/// starts past its cut-off is empty.
///
/// A step counts as past the cut-off when its end-of-step voltage is, or when it raises
/// [`EventFlags::OPERATING_POINT_OUT_OF_WINDOW`]: the cut-offs are the cell's own `v_min` and
/// `v_max`, and the flag reads the solve's node voltage, which on the cut-off step sat a
/// rounding below the window while the end-of-step voltage sat a rounding inside it.
fn leg(pack: &mut Pack, i: f64, cutoff_v: f64, flags: &mut EventFlags) -> f64 {
    let past = |t: &Telemetry| {
        t.flags.contains(EventFlags::OPERATING_POINT_OUT_OF_WINDOW)
            || if i > 0.0 {
                t.v_terminal <= cutoff_v
            } else {
                t.v_terminal >= cutoff_v
            }
    };
    let e = env(T_TEST_K);
    if past(&pack.clone().step(0.0, Demand::Current(i), &e)) {
        return 0.0;
    }
    let mut ah = 0.0;
    loop {
        let before = pack.clone();
        let t = pack.step(60.0, Demand::Current(i), &e);
        if !past(&t) {
            *flags |= t.flags;
            ah += t.i_actual * 60.0 / 3600.0;
            assert!(ah.abs() < 10.0, "a leg ran past twice the cell's capacity");
            continue;
        }
        let (mut lo, mut hi) = (0.0_f64, 60.0_f64);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if past(&before.clone().step(mid, Demand::Current(i), &e)) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        // The last step stops just short of the cut-off (40 halvings: 5e-11 s short).
        *pack = before;
        let t = pack.step(lo, Demand::Current(i), &e);
        *flags |= t.flags;
        return ah + t.i_actual * lo / 3600.0;
    }
}

fn rest(pack: &mut Pack, seconds: f64, temp_k: f64) {
    let steps = (seconds / 300.0).round() as usize;
    for _ in 0..steps {
        pack.step(300.0, Demand::Rest, &env(temp_k));
    }
}

/// WMG's capacity test on a copy of `pack`: settle in the 25 °C chamber, then discharge,
/// charge with no hold, discharge. Returns the last discharge \[Ah\].
fn capacity(pack: &Pack) -> f64 {
    let mut p = pack.clone();
    rest(&mut p, 3.0 * 3600.0, T_TEST_K);
    let t = p.cell(0, 0).expect("cell").temp_k;
    assert!(
        (t - T_TEST_K).abs() < 0.05,
        "the cell did not settle at the chamber's temperature: {t} K"
    );
    let mut flags = EventFlags::empty();
    leg(&mut p, I_TEST_A, 2.5, &mut flags);
    rest(&mut p, 1800.0, T_TEST_K);
    leg(&mut p, -I_TEST_A, 4.2, &mut flags);
    rest(&mut p, 1800.0, T_TEST_K);
    let q = leg(&mut p, I_TEST_A, 2.5, &mut flags);
    assert!(
        (flags & bad()).is_empty(),
        "the capacity test raised {flags:?}"
    );
    q
}

/// Rest `pack` at `temp_k` from `from_day` to `to_day`, in steps of at most a day.
fn store(pack: &mut Pack, from_day: f64, to_day: f64, temp_k: f64) {
    let mut s = from_day * 86400.0;
    let end = to_day * 86400.0;
    while s < end {
        let dt = (end - s).min(86400.0);
        pack.step(dt, Demand::Rest, &env(temp_k));
        s += dt;
    }
}

/// The cyclable lithium \[mol\] at `soc`: both electrodes' particles, at the stoichiometries
/// the engine starts an `Spm` cell at.
fn lithium_mol(chem: &ChemistryParams, soc: f64) -> f64 {
    let spm = chem.spm.as_ref().expect("[spm]");
    let moles = |e: &sim_core::ElectrodeParams, x: f64| {
        x * e.c_max_mol_per_m3 * e.active_volume_fraction * e.thickness_m * spm.electrode_area_m2
    };
    let n = &spm.negative;
    let p = &spm.positive;
    let x = n.stoich_min + soc * (n.stoich_max - n.stoich_min);
    let y = p.stoich_max - soc * (p.stoich_max - p.stoich_min);
    moles(n, x) + moles(p, y)
}

/// The capacity lost per lithium lost on WMG's capacity test, a year at 85 % and 25 °C:
/// what the fit's 1.567 measured in PyBaMM, and what exit 5 below does not use.
///
/// Measured: 1.4847 % of capacity for 0.9498 % of lithium (PyBaMM: 0.9505 %), a ratio of
/// 1.5632, 0.24 % under PyBaMM's. Held within 0.5 %: past that the engine reads capacity
/// differently from the reference the fit converted with, and exit 5's agreement would be
/// luck rather than the same physics.
#[test]
fn capacity_lost_per_lithium_lost_on_wmg_steps() {
    let chem = chem();
    let fresh = stored(0.85, 298.15);
    let q0 = capacity(&fresh);
    let mut aged = fresh;
    store(&mut aged, 0.0, 365.0, 298.15);
    let q1 = capacity(&aged);
    let li = aged.cell_film(0, 0).expect("film").lithium_lost_mol;
    let lli_pct = li / lithium_mol(&chem, 0.85) * 100.0;
    let cap_pct = (1.0 - q1 / q0) * 100.0;
    let ratio = cap_pct / lli_pct;
    assert!(
        (ratio / 1.567 - 1.0).abs() < 5e-3,
        "fresh {q0:.5} Ah, aged {q1:.5} Ah: capacity lost {cap_pct:.4} % for {lli_pct:.4} % of          lithium, a ratio of {ratio:.4} against PyBaMM's 1.567"
    );
}

/// One storage condition's reference: `(day, growth [%])` from its first post-storage test on.
struct Condition {
    temp_c: f64,
    soc_pct: f64,
    rows: Vec<(f64, f64)>,
}

fn conditions() -> Vec<Condition> {
    let mut out: Vec<Condition> = Vec::new();
    for line in GOLDEN.lines() {
        if line.starts_with('#') || line.starts_with("temp_c") || line.is_empty() {
            continue;
        }
        let v: Vec<f64> = line
            .split(',')
            .map(|x| x.parse().expect("a number"))
            .collect();
        match out.last_mut() {
            Some(c) if c.temp_c == v[0] && c.soc_pct == v[1] => c.rows.push((v[2], v[3])),
            _ => out.push(Condition {
                temp_c: v[0],
                soc_pct: v[1],
                rows: vec![(v[2], v[3])],
            }),
        }
    }
    out
}

/// The engine's growth \[%\] at each of the condition's test days: its capacity fade on
/// WMG's steps, against its own day-0 test, less that fade at the first post-storage test.
fn engine_growth(c: &Condition) -> Vec<f64> {
    let temp_k = c.temp_c + 273.15;
    let mut pack = stored(c.soc_pct / 100.0, temp_k);
    let q0 = capacity(&pack);
    let mut day = 0.0;
    let mut fade = Vec::with_capacity(c.rows.len());
    for &(d, _) in &c.rows {
        store(&mut pack, day, d, temp_k);
        day = d;
        fade.push((1.0 - capacity(&pack) / q0) * 100.0);
    }
    fade.iter().map(|f| f - fade[0]).collect()
}

/// `growth_rms` of `tools/reference/fit_sei_wmg.py`, on the engine: the mean square per
/// condition, averaged over conditions, square-rooted.
fn growth_rms(results: &[(&Condition, Vec<f64>)]) -> f64 {
    let per: Vec<f64> = results
        .iter()
        .map(|(c, e)| {
            let n = c.rows.len() as f64;
            c.rows
                .iter()
                .zip(e)
                .map(|(&(_, g), m)| (g - m) * (g - m))
                .sum::<f64>()
                / n
        })
        .collect();
    (per.iter().sum::<f64>() / per.len() as f64).sqrt()
}

/// Exit 5: the fitted film, run in the engine and measured on WMG's own capacity steps,
/// reproduces the cells' storage-driven fade growth at 25 and 45 °C, and predicts the held-out
/// 0 °C cells, to the residuals the provenance line states — 0.6241 % against its 0.622 %, and
/// 0.4046 % against its 0.405 % — inside a bar of each plus 0.02 points.
///
/// # The bar, set before the first full run
/// The `[spm.sei]` provenance states 0.622 % RMS at 25 + 45 °C and 0.405 % on the held-out
/// 0 °C cells — the fit's closed form's residuals, converted to capacity with 1.567. The
/// engine differs from that closed form by its graphite table (at most 2.9 % of growth, at
/// 30 % where growth is smallest) and its own capacity ratio (0.24 %); on growths of at most
/// ~3 points either moves a residual by under 0.01 points. The bar is the stated residual
/// plus 0.02 points (`docs/plans/phase-10-sei.md`, exit 5 addendum).
///
/// Measured: **0.6241 %** at 25 + 45 °C over 24 conditions, **0.4046 %** at 0 °C over 12.
#[test]
fn the_film_reproduces_the_wmg_cells() {
    let all = conditions();
    let results: Vec<(&Condition, Vec<f64>)> = all.iter().map(|c| (c, engine_growth(c))).collect();
    for (c, e) in &results {
        let (last_day, g) = *c.rows.last().expect("rows");
        println!(
            "{:2} C {:3} %: day {last_day:6.1}  cells {g:+.3}  engine {:+.3}",
            c.temp_c,
            c.soc_pct,
            e.last().expect("rows")
        );
    }
    let fitted: Vec<_> = results
        .iter()
        .filter(|(c, _)| c.temp_c != 0.0)
        .map(|(c, e)| (*c, e.clone()))
        .collect();
    let held: Vec<_> = results
        .iter()
        .filter(|(c, _)| c.temp_c == 0.0)
        .map(|(c, e)| (*c, e.clone()))
        .collect();
    let (rms_fit, rms_held) = (growth_rms(&fitted), growth_rms(&held));
    assert_eq!(
        (fitted.len(), held.len()),
        (24, 12),
        "every condition with SOC above 0 %"
    );
    assert!(
        rms_fit < 0.642,
        "growth RMS at 25 + 45 C: {rms_fit:.4} %, against the 0.622 % the fit states"
    );
    assert!(
        rms_held < 0.425,
        "growth RMS on the held-out 0 C cells: {rms_held:.4} %, against the fit's 0.405 %"
    );
}
