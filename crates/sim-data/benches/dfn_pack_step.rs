//! What a DFN step costs, and what it costs *relative to an SPM and an ECM step*
//! (Phase 7, slice D).
//!
//! Run with `cargo bench -p sim-data --bench dfn_pack_step`. The `--bench` selector is not
//! optional if criterion flags follow it — see `sim-core/benches/pack_step.rs`, whose
//! methodology notes apply here in full and are not repeated.
//!
//! # A third budget, not a widened one
//! `CLAUDE.md`'s `< 50 µs per step at 100S10P` is an **ECM** budget over a 1000-cell pack,
//! and `docs/plans/pack-step-perf.md` already calls it marginal. Phase 6 declined to widen
//! it to cover the SPM and stated a second budget instead; Phase 7 states a **third** for
//! the same reason. A porous-electrode model costs what it costs, and the honest move is a
//! separate number at a stated topology and grid rather than a budget quietly relaxed until
//! everything fits inside it.
//!
//! **The DFN budget is ≈180 µs per cell per step** at 10/5/10 with `N_r = 20`, and it is
//! quoted **per cell rather than per step** — see `bench_topologies` for why that
//! distinction is load-bearing rather than pedantic.
//!
//! # Reading the numbers: the ratio within one invocation is the measurement
//! This laptop swings ~1.4× between CPU states across sessions and sometimes within one,
//! which is larger than most effects measured here, so absolute microseconds from one
//! session are not comparable with another's. Every arm of every comparison runs in the
//! same process, minutes apart at most.
//!
//! # The `ecm/` arm is a contamination detector, not a data point
//! An ECM pack's solve loop breaks on `is_linear` before any probe, so **no change to the
//! DFN or SPM code paths can move it**. Slice C had a run report the ECM arm at 2× its
//! previous value, which is impossible, and that impossibility is what identified the run
//! as build-contaminated rather than sending someone hunting for a regression that was not
//! there. An arm that provably cannot move earns its place by failing loudly.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use sim_core::dfn::{DEFAULT_NODES_NEGATIVE, DEFAULT_NODES_POSITIVE, DEFAULT_NODES_SEPARATOR};
use sim_core::spm::DEFAULT_SHELLS;
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Simulation timestep \[s\] — matching the other two benches so all three files' numbers
/// are read on the same axis.
const DT: f64 = 0.1;

/// Mid-plateau SOC, away from both stoichiometry limits and from the OCP tables' steep
/// corners, so no arm is measured on a clamp fast path.
const SOC: f64 = 0.6;

/// The shipped recommendation, which is what this budget is stated over.
const NODES: (usize, usize, usize) = (
    DEFAULT_NODES_NEGATIVE,
    DEFAULT_NODES_SEPARATOR,
    DEFAULT_NODES_POSITIVE,
);

fn lgm50() -> ChemistryParams {
    let text = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
    sim_data::parse_chemistry(text).expect("LG M50 chemistry loads")
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn dfn(nodes: (usize, usize, usize), shells: usize) -> CellModelConfig {
    CellModelConfig::Dfn {
        shells,
        nodes_negative: nodes.0,
        nodes_separator: nodes.1,
        nodes_positive: nodes.2,
    }
}

fn config(series: u16, parallel: u16, cell_model: CellModelConfig) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        // Isothermal on every arm: a live thermal network would add the same cost to each,
        // diluting exactly the ratios this file exists to measure.
        thermal: ThermalConfig::Isothermal,
        series,
        parallel,
        initial_soc: SOC,
        initial_temp_k: 298.15,
        seed: 0xB0A7,
        // Non-zero scatter on purpose. A parallel group of identical cells converges in
        // fewer passes than a real one, and the pass count is the thing that makes a DFN's
        // per-step cost topology-dependent — benching only identical cells would report a
        // per-cell figure that does not survive contact with a scattered pack.
        scatter: Scatter {
            capacity_sigma: 0.02,
            r0_sigma: 0.05,
        },
        cell_model,
    }
}

/// Nominal 1C for a pack of this shape \[A\], from the chemistry's own capacity.
fn i_1c(parallel: u16) -> f64 {
    f64::from(parallel) * 5.153198
}

/// Build and warm a pack.
///
/// The warm-up matters more for a DFN than for any other model here. `DfnState::tangent` is
/// `None` on a cell that has never been advanced, and `DfnState::u` — the Newton warm start
/// — is seeded at open circuit. An unwarmed template would measure every iteration as a
/// cold solve from an open-circuit guess, which is a code path a running client takes
/// exactly once.
fn warmed(series: u16, parallel: u16, cell_model: CellModelConfig) -> Pack {
    let mut pack = Pack::new(&config(series, parallel, cell_model), lgm50())
        .expect("benchmark pack config is valid");
    let demand = Demand::Current(i_1c(parallel));
    // Twice: the first step fills the tangent, the second is the first step whose warm
    // start came from a converged solve rather than from the open-circuit seed.
    pack.step(DT, demand, &env());
    pack.step(DT, demand, &env());
    pack
}

fn case(c: &mut Criterion, name: &str, series: u16, parallel: u16, cell_model: CellModelConfig) {
    let pack = warmed(series, parallel, cell_model);
    let env = env();
    let demand = Demand::Current(i_1c(parallel));
    c.bench_function(name, |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, demand, &env)),
            // A DFN cell clones several `Vec<f64>` per particle plus the unknown vector, so
            // the templates are large; batching thousands up front is what makes a run swap
            // rather than measure.
            BatchSize::SmallInput,
        );
    });
}

/// The three models on the same physical cell, at 1S1P.
///
/// One cell, because that is where a model's own cost is not buried under pack aggregation,
/// and because a 10S10P DFN is ~18 ms per step — a fast-forward, not something to hand a
/// criterion sampling loop.
fn bench_models(c: &mut Criterion) {
    // Order matters for the contamination check: the ECM arm runs first and last so a run
    // whose machine state drifted mid-invocation shows it as a disagreement between two
    // measurements of the arm that cannot change.
    case(c, "dfn_vs/ecm/1S1P", 1, 1, CellModelConfig::Ecm);
    case(
        c,
        "dfn_vs/spm/1S1P",
        1,
        1,
        CellModelConfig::Spm {
            shells: DEFAULT_SHELLS,
        },
    );
    case(c, "dfn_vs/dfn/1S1P", 1, 1, dfn(NODES, DEFAULT_SHELLS));
    case(c, "dfn_vs/ecm_again/1S1P", 1, 1, CellModelConfig::Ecm);
}

/// Why the budget is quoted **per cell** and not per step.
///
/// A DFN pack's solve is a fixed-point iteration over the cells, and the number of passes it
/// needs is not a constant: a scattered parallel group needs three where a single cell needs
/// two. So a per-*step* figure measured at 1S1P silently under-states a 1S2P pack by more
/// than the cell count alone predicts, and slice C published exactly that mistake before
/// catching it. These four arms are the measurement that keeps the budget honest — divide
/// each by its cell count and the spread is what "≈180 µs per cell" is stated over.
///
/// Measured 2026-08-09 in one invocation, criterion defaults: 1S1P 181.6, 2S1P 350.4,
/// 1S2P 352.8, 2S2P 729.0 µs — i.e. **175–182 µs per cell and roughly linear**. Quotable
/// as absolutes only because the preconditions held in that run: the `spm/` arm reproduced
/// its recorded 1.22 µs (1.27), the two `ecm/` arms agreed to 15 %, and the two independent
/// measurements of the same 1S1P DFN config (`dfn_vs/dfn` and `dfn_topology/1S1P`) agreed
/// to 1.4 %. An earlier run in the same session had the two ECM arms 1.6× apart and those
/// same two DFN measurements 1.3× apart, and was discarded.
fn bench_topologies(c: &mut Criterion) {
    let model = dfn(NODES, DEFAULT_SHELLS);
    for (series, parallel, label) in [
        (1u16, 1u16, "1S1P"),
        (2, 1, "2S1P"),
        (1, 2, "1S2P"),
        (2, 2, "2S2P"),
    ] {
        case(c, &format!("dfn_topology/{label}"), series, parallel, model);
    }
}

/// The two discretisation knobs, priced.
///
/// Unlike the SPM — whose only knob is the shell count — a DFN has an x-grid *and* a radial
/// grid, and they are not interchangeable. The banded factorisation is linear in the total
/// node count while the particle solves are linear in shells per node, so doubling the
/// x-grid is expected to cost roughly double and doubling the shells rather less. The
/// accuracy half of this argument is `dfn_golden.rs`'s
/// `refining_the_x_grid_converges_toward_the_reference`, which measures what each knob buys
/// and on which scenario; neither default moves without both halves.
fn bench_grids(c: &mut Criterion) {
    for (nodes, label) in [
        ((5usize, 3usize, 5usize), "5/3/5"),
        (NODES, "10/5/10"),
        ((20, 10, 20), "20/10/20"),
    ] {
        case(
            c,
            &format!("dfn_nodes/1S1P/{label}"),
            1,
            1,
            dfn(nodes, DEFAULT_SHELLS),
        );
    }
    for n in [10usize, DEFAULT_SHELLS, 40] {
        case(c, &format!("dfn_shells/1S1P/N_r={n}"), 1, 1, dfn(NODES, n));
    }
}

criterion_group!(benches, bench_models, bench_topologies, bench_grids);
criterion_main!(benches);
