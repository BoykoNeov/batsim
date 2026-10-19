//! Phase 12 slice D: the plating lesson's sentences that print no number.
//!
//! Steps 41 to 44 of the guided path (`web/app.js`, `judged-by-a-rule`, `some-of-it-goes-back`,
//! `the-charge-the-datasheet-allows`, `colder-and-billed-throughout`) charge one LG M50 `Dfn`
//! cell cold, judged by the chemistry's threshold rule or plating as a reaction. Every number
//! they print is a claim or a ledger rule in `web/path-claims.toml`. What is here is what the
//! prose says in words: the four files are one file with one field changed, the rule is silent
//! at 0 °C because the file sits on its edge, the reaction plates first next to the separator, and
//! the cell rests higher while it strips because of the metal and not the charge it holds; and a
//! bigger current typed into the pulse box stops at the 4.2 V ceiling instead of overcharging.
//! See `docs/plans/phase-12-plating.md` §D and `docs/plans/phase-12-slice-d-lesson.md`.

use sim_core::dfn::{FINE_NODES_NEGATIVE, FINE_NODES_POSITIVE, FINE_NODES_SEPARATOR, FINE_SHELLS};
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, EventFlags, Pack, PackConfig, PlatingModel,
    ThermalConfig,
};
use sim_data::{load_chemistry_file, load_scenario_file, Scenario};
use std::path::PathBuf;

/// The lesson's step length \[s\] (`dt: 5` on all four steps).
const DT_S: f64 = 5.0;
/// The 1C charge's current \[A\], discharge-positive: the steps' `Pulse` value.
const ONE_C_A: f64 = -5.153198;
/// How long the 0 °C 1C pulse charges \[s\] (`on_s` on steps 41 and 42).
const ON_S: f64 = 1800.0;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scenario(name: &str) -> Scenario {
    load_scenario_file(root().join("scenarios").join(format!("{name}.toml")))
        .unwrap_or_else(|e| panic!("scenarios/{name}.toml loads: {e}"))
}

fn chem() -> ChemistryParams {
    load_chemistry_file(root().join("chemistries/nmc_21700_lgm50.toml"))
        .expect("the LG M50 file loads")
}

fn with_plating(mut cfg: PackConfig, plating: PlatingModel) -> PackConfig {
    cfg.aging.as_mut().expect("aging is on").plating = plating;
    cfg
}

fn with_temp(mut cfg: PackConfig, k: f64) -> PackConfig {
    cfg.initial_temp_k = k;
    cfg
}

/// The four files are one `PackConfig` with the plating model or the temperature changed,
/// as their headers and steps 41 and 44 say; and all four run the `Dfn` on its fine grid,
/// isothermal, from empty, with no BMS.
#[test]
fn plating_files_differ_only_where_their_headers_say() {
    let cold_t = scenario("plating_cold_threshold");
    let cold_r = scenario("plating_cold_reaction");
    let colder_t = scenario("plating_colder_threshold");
    let colder_r = scenario("plating_colder_reaction");
    for sc in [&cold_t, &cold_r, &colder_t, &colder_r] {
        assert_eq!(sc.chemistry.as_deref(), Some("nmc_21700_lgm50"));
        assert!(sc.faults.is_empty(), "no faults");
    }
    let base = cold_t.pack.clone();
    assert_eq!(
        base.cell_model,
        CellModelConfig::Dfn {
            shells: FINE_SHELLS,
            nodes_negative: FINE_NODES_NEGATIVE,
            nodes_separator: FINE_NODES_SEPARATOR,
            nodes_positive: FINE_NODES_POSITIVE,
        },
        "the fine grid, written into the file"
    );
    assert!(matches!(base.thermal, ThermalConfig::Isothermal));
    assert!(base.bms.is_none());
    assert_eq!(base.initial_soc, 0.0);
    assert_eq!(base.initial_temp_k, 273.15);
    assert_eq!(
        base.aging.as_ref().map(|a| a.plating),
        Some(PlatingModel::Threshold)
    );
    assert_eq!(
        with_plating(base.clone(), PlatingModel::Reaction),
        cold_r.pack,
        "plating_cold_reaction is plating_cold_threshold with the plating model changed"
    );
    assert_eq!(
        with_temp(base, 263.15),
        colder_t.pack,
        "plating_colder_threshold is plating_cold_threshold at -10 degC and nothing else"
    );
    assert_eq!(
        with_temp(cold_r.pack.clone(), 263.15),
        colder_r.pack,
        "plating_colder_reaction is plating_cold_reaction at -10 degC and nothing else"
    );
}

/// The steps' voltage stop \[V per cell\] (`stop_v` on all four; the cells are 1S1P).
const STOP_V: f64 = 4.2;

/// What one run of the steps' program saw.
struct Pulsed {
    pack: Pack,
    /// Whether `PLATING_RISK` rose on any step.
    plated: bool,
    /// Each step's end time \[s\], terminal voltage \[V\], current \[A\] and flags.
    rows: Vec<(f64, f64, f64, EventFlags)>,
    /// The zero-length probe under the charge, before the first step \[V\].
    probe_v: f64,
}

/// Run `sc` on the steps' `Pulse` (on-leg iff `round(t/dt) % (kOn + kOff) < kOn`) for `secs`,
/// with the page's voltage stop (`pulseStopNow`): a charging pulse rests to its edge from the
/// step after the first stepped frame at or above [`STOP_V`]. Never decided on a zero-length
/// probe, which on these cold charges reads above the ceiling before anything has moved; the
/// first step charges. Returns the probe's voltage beside the run, for the test that says so.
fn pulse(sc: &Scenario, i: f64, on_s: f64, off_s: f64, secs: f64) -> Pulsed {
    let mut p = sc.build_pack(chem()).expect("the lesson pack builds");
    let env = Env {
        t_ambient: sc.pack.initial_temp_k,
        t_coolant: None,
    };
    #[allow(clippy::cast_possible_truncation)]
    let (k_on, k_off) = ((on_s / DT_S).round() as i64, (off_s / DT_S).round() as i64);
    let probe_v = p.step(0.0, Demand::Current(i), &env).v_terminal;
    let mut last_v = f64::NEG_INFINITY;
    let mut stopped: Option<i64> = None;
    let (mut plated, mut rows) = (false, Vec::new());
    while p.sim_time_s() + 1e-9 < secs {
        #[allow(clippy::cast_possible_truncation)]
        let idx = (p.sim_time_s() / DT_S).round() as i64;
        let pulse_no = idx.div_euclid(k_on + k_off);
        let on = idx.rem_euclid(k_on + k_off) < k_on;
        if on && i < 0.0 && last_v >= STOP_V {
            stopped = Some(pulse_no);
        }
        let d = if on && stopped != Some(pulse_no) {
            Demand::Current(i)
        } else {
            Demand::Rest
        };
        let t = p.step(DT_S, d, &env);
        plated |= t.flags.contains(EventFlags::PLATING_RISK);
        last_v = t.v_terminal;
        rows.push((p.sim_time_s(), t.v_terminal, t.i_actual, t.flags));
    }
    Pulsed {
        pack: p,
        plated,
        rows,
        probe_v,
    }
}

/// "0 °C is not below 0 °C, so the rule says nothing; it is a cliff, and this file sits
/// exactly on its edge" (step 41): the threshold file is silent on the whole 1C charge, and the
/// same file a hundredth of a kelvin colder flags it.
#[test]
fn the_rule_is_silent_on_its_edge_and_not_below_it() {
    let sc = scenario("plating_cold_threshold");
    let on_edge = pulse(&sc, ONE_C_A, ON_S, 7200.0, ON_S).plated;
    assert!(!on_edge, "the rule flags at exactly 0 degC");
    let mut below = sc.clone();
    below.pack.initial_temp_k -= 0.01;
    let under = pulse(&below, ONE_C_A, ON_S, 7200.0, 300.0).plated;
    assert!(
        under,
        "the rule is silent a hundredth of a kelvin below its edge"
    );
}

/// "`the_resting_gap_is_the_metal` checks that the gap is gone when the reaction is made too
/// slow to plate" (step 42): a minute into the rest the reaction cell is above the threshold
/// cell by millivolts, and the same file with the rate constant scaled by 1e-6 sits on the
/// threshold cell to well under a microvolt.
#[test]
fn the_resting_gap_is_the_metal() {
    let mark = ON_S + 60.0;
    let rule = pulse(
        &scenario("plating_cold_threshold"),
        ONE_C_A,
        ON_S,
        7200.0,
        mark,
    )
    .pack;
    let reaction_sc = scenario("plating_cold_reaction");
    let reaction = pulse(&reaction_sc, ONE_C_A, ON_S, 7200.0, mark).pack;
    let v = |p: &Pack| {
        let mut probe = p.clone();
        probe
            .step(
                0.0,
                Demand::Rest,
                &Env {
                    t_ambient: 273.15,
                    t_coolant: None,
                },
            )
            .v_terminal
    };
    let gap = v(&reaction) - v(&rule);
    // The reaction file with a rate constant too small to plate: the same build, by hand.
    let mut c = chem();
    c.dfn
        .as_mut()
        .and_then(|d| d.plating.as_mut())
        .expect("[dfn.plating]")
        .rate_constant_m_per_s *= 1.0e-6;
    let mut slow = reaction_sc.build_pack(c).expect("builds");
    let env = Env {
        t_ambient: 273.15,
        t_coolant: None,
    };
    while slow.sim_time_s() + 1e-9 < mark {
        let d = if slow.sim_time_s() + 1e-9 < ON_S {
            Demand::Current(ONE_C_A)
        } else {
            Demand::Rest
        };
        slow.step(DT_S, d, &env);
    }
    let slow_gap = v(&slow) - v(&rule);
    println!(
        "a minute into the rest: gap {:.3} mV, rate constant x1e-6 {:.3e} mV",
        gap * 1e3,
        slow_gap * 1e3
    );
    assert!(gap > 1.5e-3, "the stripping cell rests higher: {gap} V");
    assert!(
        slow_gap.abs() < 1.0e-6,
        "with no plating the gap is gone: {slow_gap} V"
    );
    // And it is not the charge: the reaction cell holds less.
    assert!(reaction.cell(0, 0).unwrap().soc < rule.cell(0, 0).unwrap().soc);
}

/// "It gets there first next to the separator, where the current crowds hardest" (step 41): on
/// the first step of the 0 °C 1C charge that plates, the node holding the most metal is the
/// negative electrode's last, the one against the separator (the grid runs from the negative's
/// current collector to the separator, `dfn::Grid::region`), and the metal falls away from it.
#[test]
fn plating_starts_next_to_the_separator() {
    let sc = scenario("plating_cold_reaction");
    let mut p = sc.build_pack(chem()).expect("the lesson pack builds");
    let env = Env {
        t_ambient: sc.pack.initial_temp_k,
        t_coolant: None,
    };
    loop {
        let tel = p.step(DT_S, Demand::Current(ONE_C_A), &env);
        if tel.flags.contains(EventFlags::PLATING_RISK) {
            break;
        }
        assert!(p.sim_time_s() < ON_S, "the charge never plated");
    }
    let v = serde_json::to_value(p.snapshot()).expect("a snapshot is JSON");
    let rev: Vec<f64> = v["pack"]["groups"][0]["cells"][0]["model"]["Dfn"]["plating"]
        ["reversible_mol_m2"]
        .as_array()
        .expect("a reaction cell's stores")
        .iter()
        .map(|x| x.as_f64().expect("finite"))
        .collect();
    let last = rev.len() - 1;
    println!(
        "first plating at {} s: the last four nodes {:?}",
        p.sim_time_s(),
        &rev[last - 3..]
    );
    let argmax = rev
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, _)| k)
        .expect("nodes");
    assert_eq!(argmax, last, "the most metal is against the separator");
    assert!(rev[last] > 0.0);
    assert!(
        rev[..last].iter().all(|&q| q < rev[last]),
        "and less of it everywhere further from the separator"
    );
}

/// "A bigger current typed into the pulse box ends the charge sooner, with the cell less full,
/// instead of driving it past its ceiling" (step 41): 2C and 3C typed into each of the four
/// steps' programs, on both files, to the step's mark. Before the stop these ran the step's
/// fixed on-time and plated 43 % to 375 % of the cell's capacity (the slice D note's addendum).
///
/// Each stops on the step after the first at or above 4.2 V, within that step's own rise of the
/// ceiling; the rest after it is a rest; every value is finite; no step the page's solve notice
/// does not cover reads above where the charge stopped; and the cell plates under 0.1 % of its
/// capacity. And at each step's own current the stop never fires. "Push much harder than that
/// and even the rest after the stop is more than this model can solve; a notice under the flags
/// then says from when": the runs whose rest raises a flag the notice reads are pinned.
#[test]
fn a_bigger_current_stops_at_the_ceiling() {
    let steps = [
        (
            "plating_cold_threshold",
            "plating_cold_reaction",
            ONE_C_A,
            ON_S,
            7200.0,
        ),
        (
            "plating_cold_reaction",
            "plating_cold_threshold",
            -1.5459594,
            9440.0,
            3600.0,
        ),
        (
            "plating_colder_threshold",
            "plating_colder_reaction",
            ONE_C_A,
            1390.0,
            3600.0,
        ),
    ];
    let lost = EventFlags::SOLVE_UNCONVERGED | EventFlags::SURFACE_OUT_OF_RANGE;
    let mut noticed = Vec::new();
    for (own, other, i_own, on_s, off_s) in steps {
        for file in [own, other] {
            let sc = scenario(file);
            let at_own = pulse(&sc, i_own, on_s, off_s, on_s);
            assert!(
                at_own.rows.iter().all(|r| r.1 < STOP_V && r.2 < 0.0),
                "{file} at its own current reaches the stop"
            );
            // Why the stop must not read the probe.
            println!(
                "{file}: the probe under the charge reads {:.4} V",
                at_own.probe_v
            );
            for c in [2.0, 3.0] {
                let i = ONE_C_A * c;
                let run = pulse(&sc, i, on_s, off_s, on_s);
                let k = run
                    .rows
                    .iter()
                    .position(|r| r.1 >= STOP_V)
                    .unwrap_or_else(|| panic!("{file} at {c}C never reaches {STOP_V} V"));
                let (t_stop, v_stop) = (run.rows[k].0, run.rows[k].1);
                let rise = v_stop - run.rows[k - 1].1;
                let peak = run
                    .rows
                    .iter()
                    .map(|r| r.1)
                    .fold(f64::NEG_INFINITY, f64::max);
                let pl = run
                    .pack
                    .cell_plating(0, 0)
                    .map_or(0.0, |v| v.reversible_fraction + v.dead_fraction);
                let soc = run.pack.cell(0, 0).expect("one cell").soc;
                let flagged: Vec<&(f64, f64, f64, EventFlags)> =
                    run.rows.iter().filter(|r| r.3.intersects(lost)).collect();
                if let (Some(first), Some(last)) = (flagged.first(), flagged.last()) {
                    let vs = flagged.iter().map(|r| r.1);
                    println!(
                        "  {file} {c}C: {} of {} steps flagged, {} s to {} s, {:?}, v {:.4}..{:.4} V",
                        flagged.len(),
                        run.rows.len(),
                        first.0,
                        last.0,
                        first.3,
                        vs.clone().fold(f64::INFINITY, f64::min),
                        vs.fold(f64::NEG_INFINITY, f64::max)
                    );
                }
                println!(
                    "{file} {c}C: stops at {t_stop} s, {v_stop:.4} V (rise {:.1} mV), {:.1} % full, plated {:.4} %",
                    rise * 1e3,
                    soc * 100.0,
                    pl * 100.0
                );
                assert!(
                    v_stop - STOP_V <= rise,
                    "{file} {c}C: past the stop by more than a step"
                );
                // Where the rest after the stop cannot be solved its voltages are not the cell's,
                // and the page's notice is up from the first such step. What must hold is that
                // no step the notice does not cover reads above where the charge stopped.
                assert!(
                    run.rows
                        .iter()
                        .filter(|r| !r.3.intersects(lost))
                        .all(|r| r.1 <= v_stop),
                    "{file} {c}C: an unflagged step higher than the stop (peak {peak})"
                );
                assert!(
                    run.rows[k + 1..].iter().all(|r| r.2 == 0.0),
                    "{file} {c}C: charged again after the stop"
                );
                if !flagged.is_empty() {
                    noticed.push(format!("{file} {c}C"));
                }
                assert!(
                    run.rows.iter().all(|r| r.1.is_finite() && r.2.is_finite()),
                    "{file} {c}C: a value that is not finite"
                );
                assert!(pl.is_finite() && pl < 1.0e-3, "{file} {c}C: plated {pl}");
            }
        }
    }
    // The rest after a fast cold charge, stopped at the ceiling, is the older failure
    // `plating-unconverged-runaway.md` records (ROADMAP H8's family): flagged, so the notice
    // is up. Only 2C on the 0 °C reaction file rests clean. Pinned so a change shows.
    assert_eq!(
        noticed,
        [
            "plating_cold_threshold 2C",
            "plating_cold_threshold 3C",
            "plating_cold_reaction 3C",
            "plating_cold_reaction 3C",
            "plating_cold_threshold 2C",
            "plating_cold_threshold 3C",
            "plating_colder_threshold 2C",
            "plating_colder_threshold 3C",
            "plating_colder_reaction 2C",
            "plating_colder_reaction 3C",
        ],
        "the typed-current runs the solve notice comes up on"
    );
}
