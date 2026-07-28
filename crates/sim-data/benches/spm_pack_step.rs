//! What an SPM step costs, and what it costs *relative to an ECM step* (Phase 6,
//! slice E).
//!
//! Run with `cargo bench -p sim-data --bench spm_pack_step`. The `--bench` selector
//! is not optional if criterion flags follow it — see `sim-core/benches/pack_step.rs`,
//! whose methodology notes apply here in full and are not repeated.
//!
//! # Why this bench exists separately from `sim-core`'s
//! `CLAUDE.md`'s `< 50 µs per step at 100S10P` is an **ECM** budget, stated for a
//! 1000-cell equivalent-circuit pack, and `docs/plans/pack-step-perf.md` already
//! calls it marginal. A physics-based cell costs what it costs; quietly widening the
//! ECM budget to cover it would be the dishonest move, so the SPM gets its own
//! budget at a **stated topology and shell count** and the ECM budget is left where
//! it is.
//!
//! # The comparison is designed to isolate the model, and only the model
//! Both arms below run the **same shipped chemistry file** —
//! `chemistries/nmc_21700_lgm50.toml`, which carries an ECM description *and* an
//! `[spm]` description of the same physical cell — at the same topology, the same
//! SOC, the same `dt` and the same demand. The only difference between an `ecm/…`
//! case and the matching `spm/…` case is `PackConfig::cell_model`. That is what
//! makes the ratio between them a statement about the cell model rather than about
//! two differently-shaped parameter sets.
//!
//! Reading the numbers: **the ratio within one invocation is the measurement.**
//! This laptop swings ~1.4× between CPU states across sessions (and sometimes within
//! one), which is larger than anything measured here, so absolute microseconds
//! quoted from one session are not comparable with another's. Both arms of every
//! pair run in the same process, minutes apart at most.
//!
//! # The shell-count arms
//! `spm/1S1P/N=5|20|40` price the one knob the model has. The diffusion solve is a
//! single Thomas sweep per particle, so cost is expected to be linear in `N` with a
//! fixed overhead — and if it ever comes out super-linear, something is allocating.
//! `sim_core::spm::DEFAULT_SHELLS` documents why 20 is the recommended value; these
//! arms are the cost half of that argument, and `spm_golden.rs`'s convergence test
//! is the accuracy half.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use sim_core::spm::DEFAULT_SHELLS;
use sim_core::{
    CellModelConfig, ChemistryParams, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Simulation timestep \[s\] — a typical real-time client step, matching
/// `sim-core`'s bench so the two files' numbers are read on the same axis.
const DT: f64 = 0.1;

/// Mid-plateau SOC, away from both stoichiometry limits and from the OCP tables'
/// steep corners, so neither model is measured on a clamp fast path.
const SOC: f64 = 0.6;

/// The shipped LG M50 file: an ECM description and an `[spm]` description of one
/// physical cell. Parsed once per case from a compile-time `include_str!`, so no
/// benchmark iteration touches the filesystem.
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

fn config(series: u16, parallel: u16, cell_model: CellModelConfig) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        // Isothermal on both arms. A live thermal network would add the same cost to
        // each, diluting exactly the ratio this file exists to measure.
        thermal: ThermalConfig::Isothermal,
        series,
        parallel,
        initial_soc: SOC,
        initial_temp_k: 298.15,
        seed: 0xB0A7,
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

/// Build and warm a pack. The warm-up matters more here than on the ECM path: an
/// SPM cell's Thévenin is a **tangent taken at `i_last`**, and a never-stepped cell
/// has `i_last = 0`, so an unwarmed template would measure every iteration as a
/// tangent re-taken from rest — a code path a running client never sees.
fn warmed(series: u16, parallel: u16, cell_model: CellModelConfig) -> Pack {
    let mut pack = Pack::new(&config(series, parallel, cell_model), lgm50())
        .expect("benchmark pack config is valid");
    let demand = Demand::Current(i_1c(parallel));
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
            // `SmallInput` rather than `LargeInput`, and the reason is specific to
            // this model: an SPM cell clones two `Vec<f64>` of `shells` elements, so
            // a 1000-cell pack at 20 shells copies ~320 kB per iteration. Criterion
            // excludes the clone from the timing either way, but batching thousands
            // of those templates up front is what makes a run swap rather than
            // measure.
            BatchSize::SmallInput,
        );
    });
}

/// Both models on the same cell, at the topologies `CLAUDE.md`'s ECM budget is
/// stated over.
fn bench_models(c: &mut Criterion) {
    let spm = CellModelConfig::Spm {
        shells: DEFAULT_SHELLS,
    };
    for (series, parallel, label) in [
        (1u16, 1u16, "1S1P"),
        (10, 10, "10S10P"),
        (100, 10, "100S10P"),
    ] {
        case(
            c,
            &format!("spm_vs_ecm/ecm/{label}"),
            series,
            parallel,
            CellModelConfig::Ecm,
        );
        case(c, &format!("spm_vs_ecm/spm/{label}"), series, parallel, spm);
    }
}

/// The shell count, priced. One cell, so the per-cell cost is not buried under pack
/// aggregation.
fn bench_shells(c: &mut Criterion) {
    for n in [5usize, DEFAULT_SHELLS, 40] {
        case(
            c,
            &format!("spm_shells/1S1P/N={n}"),
            1,
            1,
            CellModelConfig::Spm { shells: n },
        );
    }
}

criterion_group!(benches, bench_models, bench_shells);
criterion_main!(benches);
