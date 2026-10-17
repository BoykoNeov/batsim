//! Phase 11 exit 5: the engine's `Dfn` on the shipped LG M50 tracks a cold PyBaMM DFN.
//!
//! `tests/golden/nmc_21700_lgm50/dfn_cc_1c_m10c.csv` is PyBaMM's DFN on Chen2020 with
//! ORegan2022's solid-diffusion, electrolyte and entropy terms — the cell the shipped file
//! describes since Phase 11 slice C — at 1C from full, isothermal at −10 °C
//! (`tools/reference/dfn_cold_golden.py`, cut-off at 2813.2 s), converged in the particles
//! (200 points) and across the cell (160/80/160).
//!
//! # The bar, and its history
//! The bar is the one set before the first run on 2026-10-15, unchanged in every word that
//! judges: **12 mV worst over the whole run, and the cut-off within 1 %**. What it judges
//! changed twice, and both are written down in `docs/plans/phase-11-fitting.md`:
//!
//! * **The reference.** The first golden was made on PyBaMM's default x-grid, 20/20/20, which
//!   is not converged cold: PyBaMM's own cut-off moves 2787.3 → 2807.5 → 2812.1 → 2813.2 s on
//!   20/20/20, 40/40/40, 80/40/80, 160/80/160. The engine converges to the same place (2664.8,
//!   2783.6, 2806.7, 2812.2 s on 10/5/10, 20/10/20, 40/20/40, 80/40/80), so an earlier "pass"
//!   at 20/10/20 was the two under-resolved alike, and the bar the owner was offered on
//!   2026-10-15 — set after measurement — was declined. The reference's own uncertainty is
//!   its 80/40/80 → 160/80/160 difference: 0.9 mV above 2.75 V, 1.1 s at the cut-off.
//! * **The grid.** At −10 °C the electrolyte diffuses 0.4× as fast and the recommended
//!   10/5/10 is too coarse; the owner chose a finer grid as an opt-in, the recommendation
//!   unchanged. [`sim_core::dfn::FINE_NODES_NEGATIVE`] and its siblings (40/20/40, 40 shells)
//!   were fixed before this test first ran, and the bar with them, in the same commit.
//!
//! # The judging run, 2026-10-17: the bar FAILED, on the voltage, at the knee
//! Whole run worst **66.96 mV at 2810 s** — the last replayed row, the reference at 2.544 V and
//! falling ~6 mV/s; the cut-off **2808 s against 2813.2 (−0.18 %)**, inside its 1 %. The miss is
//! the knee's timing on a grid five seconds early: the same run is within 4.97 mV while the
//! reference reads ≥ 2.75 V (97 % of the discharge), 21.8 mV at 2.60 V, 33.1 at 2.55. Exit 5 is
//! **failed as judged**, and the owner accepted that record as the exit (2026-10-17, Phase 11
//! complete); the grid was not changed to pass (80/40/80 with 40 shells reads 11.47 mV whole
//! run, measured after the judging run and inside the bar by less than the reference's own
//! uncertainty). [`the_fine_grid_misses_the_bar_at_the_knee`] pins the record, not a bar.
//!
//! The recommended grid's cold figure stays visible in
//! [`the_recommended_grid_is_the_recorded_cold_limit`].

use sim_core::dfn::{
    DEFAULT_NODES_NEGATIVE, DEFAULT_NODES_POSITIVE, DEFAULT_NODES_SEPARATOR, FINE_NODES_NEGATIVE,
    FINE_NODES_POSITIVE, FINE_NODES_SEPARATOR, FINE_SHELLS,
};
use sim_core::spm::DEFAULT_SHELLS;
use sim_core::{CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const CC_1C_COLD: &str = include_str!("../../../tests/golden/nmc_21700_lgm50/dfn_cc_1c_m10c.csv");
const TEMP_K: f64 = 263.15;
const V_CUT: f64 = 2.5;
/// The reference's cut-off \[s\]: the event time `dfn_cold_golden.py` reported. The CSV is
/// sampled every 5 s and stops at its last sample before the event.
const REF_CUTOFF_S: f64 = 2813.2;

const FINE: ((usize, usize, usize), usize) = (
    (
        FINE_NODES_NEGATIVE,
        FINE_NODES_SEPARATOR,
        FINE_NODES_POSITIVE,
    ),
    FINE_SHELLS,
);
const RECOMMENDED: ((usize, usize, usize), usize) = (
    (
        DEFAULT_NODES_NEGATIVE,
        DEFAULT_NODES_SEPARATOR,
        DEFAULT_NODES_POSITIVE,
    ),
    DEFAULT_SHELLS,
);

struct Row {
    t: f64,
    i: f64,
    v: f64,
}

fn parse_csv(text: &str) -> Vec<Row> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("time_s"))
        .map(|l| {
            let mut f = l.split(',');
            let mut next = || f.next().unwrap().parse::<f64>().unwrap();
            Row {
                t: next(),
                i: next(),
                v: next(),
            }
        })
        .collect()
}

fn pack(grid: ((usize, usize, usize), usize)) -> Pack {
    let (nodes, shells) = grid;
    let config = PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: 1.0,
        initial_temp_k: TEMP_K,
        seed: 0,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Dfn {
            shells,
            nodes_negative: nodes.0,
            nodes_separator: nodes.1,
            nodes_positive: nodes.2,
        },
    };
    Pack::new(&config, sim_data::parse_chemistry(LGM50).expect("parses")).expect("builds")
}

fn env() -> Env {
    Env {
        t_ambient: TEMP_K,
        t_coolant: None,
    }
}

/// The time \[s\] the engine reaches the cut-off at 1C from full, in 1 s steps.
fn cutoff_s(grid: ((usize, usize, usize), usize)) -> f64 {
    let mut p = pack(grid);
    let i = sim_data::parse_chemistry(LGM50)
        .expect("parses")
        .cell
        .capacity_ah;
    let mut t = 0.0;
    while t < 4000.0 {
        t += 1.0;
        if p.step(1.0, Demand::Current(i), &env()).v_terminal <= V_CUT {
            break;
        }
    }
    t
}

/// The judging run's record (see the module doc). Each window holds a measured figure, so a
/// change that moves the knee — either way — is noticed: the whole-run worst stays a failure
/// of the 12 mV bar (66.96 mV), the cut-off stays inside its 1 % (−0.18 %), and the run above
/// 2.75 V stays where it was (4.97 mV). None of those windows is a bar the engine passed.
#[test]
fn the_fine_grid_misses_the_bar_at_the_knee() {
    let rows = parse_csv(CC_1C_COLD);
    assert!(rows.len() > 100, "too few rows — CSV format changed?");
    let mut p = pack(FINE);
    let (mut worst, mut at, mut above) = (0.0_f64, f64::NAN, 0.0_f64);
    for w in rows.windows(2) {
        let v = p
            .step(w[1].t - w[0].t, Demand::Current(w[1].i), &env())
            .v_terminal;
        assert!(v.is_finite(), "non-finite voltage at {} s", w[1].t);
        if w[1].v >= 2.75 {
            above = above.max((v - w[1].v).abs());
        }
        if (v - w[1].v).abs() > worst {
            worst = (v - w[1].v).abs();
            at = w[1].t;
        }
    }
    let t_eng = cutoff_s(FINE);
    eprintln!(
        "cold 1C on the fine grid: worst {:.2} mV (at {at} s) over the whole run, {:.2} mV while the reference reads >= 2.75 V; cut-off {t_eng} s against {REF_CUTOFF_S} s",
        worst * 1e3,
        above * 1e3
    );
    assert!(
        (0.040..0.100).contains(&worst),
        "the whole-run worst moved from its recorded 66.96 mV to {:.2} mV at {at} s{}",
        worst * 1e3,
        if worst < 0.012 {
            " — inside the 12 mV bar: exit 5 may now pass; take it to the owner"
        } else {
            ""
        }
    );
    assert!(
        (0.002..0.008).contains(&above),
        "the run above 2.75 V moved from its recorded 4.97 mV to {:.2} mV",
        above * 1e3
    );
    assert!(
        (t_eng / REF_CUTOFF_S - 1.0).abs() < 0.01,
        "the cold cut-off came at {t_eng} s against the reference's {REF_CUTOFF_S} s"
    );
}

/// The recommended 10/5/10 grid at −10 °C, recorded rather than hidden: the cut-off comes
/// early: 5.27 % (2665 s against 2813.2), measured against the converged reference. Against
/// the first, unconverged one it read 4.4 % (against 2787.3); the window, 3–6 %, held both and
/// was not moved. A finer recommended
/// grid, or an electrolyte discretisation that converges faster, would move it, and that is
/// the notice it did.
#[test]
fn the_recommended_grid_is_the_recorded_cold_limit() {
    let early = 1.0 - cutoff_s(RECOMMENDED) / REF_CUTOFF_S;
    eprintln!(
        "the recommended grid's cold cut-off is {:.2} % early",
        100.0 * early
    );
    assert!(
        (0.03..0.06).contains(&early),
        "the recommended grid's cold cut-off is {:.2} % early: the recorded limit moved",
        100.0 * early
    );
}
