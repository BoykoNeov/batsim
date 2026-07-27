//! Criterion benchmarks for [`Pack::step`] — the engine's hot loop.
//!
//! `CLAUDE.md` sets a budget of **< 50 µs per step at 100S10P** (1000 cells) on a
//! laptop. That is a budget to keep an eye on, not a test gate: a wall-clock
//! assertion would be machine- and CI-dependent, so nothing here fails a build.
//! Run it with `cargo bench -p sim-core`.
//!
//! # Methodology
//! Each measured iteration is **one** `step()` on a freshly cloned pack
//! (`iter_batched_ref`, so the clone is excluded from the timing). Stepping one
//! long-lived pack instead would drain it: past SOC 0 the coulomb step takes its
//! clamp branch and the OCV walk moves to a different segment, so the reported
//! number would no longer be the code path it claims to measure.
//!
//! The returned `Telemetry` is `black_box`ed. `step`'s end-of-step reporting pass
//! recomputes `cell_source` for every cell and feeds only the return value, so
//! dropping it unused would let the optimiser delete that whole second pass —
//! especially under this workspace's `lto = true, codegen-units = 1`.
//!
//! # Sanity check on the numbers
//! The solve is O(cells), so the three topologies must sit on a straight line
//! through `cost = fixed + per_cell·n`. `10S10P` is carried purely as that
//! anchor: if the 1000-cell case came in far *below* 100× the 100-cell one,
//! suspect dead-code elimination; far *above*, suspect batching or allocation
//! overhead leaking into the measurement. Note that the 1S1P figure is *not*
//! `per_cell` — at one cell the per-step fixed overhead (the solve's scratch
//! `Vec`s) dominates.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use sim_core::chem::{CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair};
use sim_core::{Demand, Env, Pack, PackConfig, Scatter};

/// Simulation timestep \[s\] — a typical real-time client step. `dt` only enters
/// through `exp(−dt/τ)` and the coulomb count, so its value does not change the
/// cost of a step.
const DT: f64 = 0.1;

/// Cell capacity \[Ah\], mirroring the chemistry below.
const CAP_AH: f64 = 2.303451;

/// Nominal-ish pack SOC to benchmark at. `interp1` scans the OCV breakpoints
/// linearly from the low end, so where on the table SOC sits *does* affect the
/// cost of a step; 0.6 is mid-plateau, which is the honest common case.
const SOC: f64 = 0.6;

/// Chemistry with the same *table shapes* as a real parameter set: 34-point OCV,
/// 3×3 `R0` grid, one RC pair. The flat 2-point tables used by the unit tests
/// would understate interpolation cost.
///
/// provenance: values copied verbatim from `chemistries/lfp_26650_generic.toml`
/// (see that file for the per-number provenance). Only the shape matters here —
/// nothing in this file is a physical claim.
fn lfp_like_chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "lfp_26650_generic".into(),
            name: "Generic LFP 26650".into(),
            provenance: "benchmark copy of chemistries/lfp_26650_generic.toml".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.00,
            max_charge_c: 1.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![
                0.0000, 0.0025, 0.0050, 0.0075, 0.0100, 0.0125, 0.0150, 0.0175, 0.0200, 0.0300,
                0.0400, 0.0500, 0.1000, 0.1500, 0.2500, 0.3500, 0.4500, 0.5500, 0.6500, 0.7500,
                0.8500, 0.9000, 0.9500, 0.9600, 0.9700, 0.9800, 0.9825, 0.9850, 0.9875, 0.9900,
                0.9925, 0.9950, 0.9975, 1.0000,
            ],
            volts: vec![
                2.0000, 2.0743, 2.1430, 2.2066, 2.2655, 2.3199, 2.3703, 2.4169, 2.4600, 2.6028,
                2.7077, 2.7853, 2.9781, 3.1080, 3.1857, 3.2324, 3.2621, 3.2678, 3.2700, 3.2926,
                3.3132, 3.3142, 3.3164, 3.3193, 3.3274, 3.3502, 3.3607, 3.3743, 3.3920, 3.4150,
                3.4449, 3.4838, 3.5343, 3.6000,
            ],
        },
        r0: R0Table {
            soc: vec![0.0, 0.5, 1.0],
            temp_k: vec![263.15, 298.15, 318.15],
            ohms: vec![
                vec![0.055, 0.022, 0.018],
                vec![0.048, 0.020, 0.016],
                vec![0.050, 0.021, 0.017],
            ],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

/// A pack with realistic manufacturing scatter, so the parallel-group solve does
/// real per-cell work rather than running on identical cells.
fn make_pack(series: u16, parallel: u16) -> Pack {
    let config = PackConfig {
        series,
        parallel,
        initial_soc: SOC,
        initial_temp_k: 298.15,
        seed: 0xB0A7,
        scatter: Scatter {
            capacity_sigma: 0.02,
            r0_sigma: 0.05,
        },
    };
    Pack::new(&config, lfp_like_chem()).expect("benchmark pack config is valid")
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// One step of a single cell — the floor, dominated by the per-step fixed cost
/// rather than by per-cell work (see the module docs).
fn bench_single_cell(c: &mut Criterion) {
    let pack = make_pack(1, 1);
    let env = env();
    // ~1C discharge for one cell.
    let demand = Demand::Current(CAP_AH);

    c.bench_function("pack_step/1S1P/current", |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, demand, &env)),
            BatchSize::LargeInput,
        );
    });
}

/// One step of a 100-cell pack — the linearity anchor between 1S1P and 100S10P
/// (see the module docs).
fn bench_mid_pack(c: &mut Criterion) {
    let pack = make_pack(10, 10);
    let env = env();
    let demand = Demand::Current(10.0 * CAP_AH);

    c.bench_function("pack_step/10S10P/current", |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, demand, &env)),
            BatchSize::LargeInput,
        );
    });
}

/// One step of the 1000-cell pack the `CLAUDE.md` budget is stated against.
fn bench_large_pack(c: &mut Criterion) {
    let pack = make_pack(100, 10);
    let env = env();
    let mut group = c.benchmark_group("pack_step/100S10P");

    // ~1C discharge: 10 parallel cells × 2.3 Ah ≈ 23 Ah of pack capacity.
    let i_1c = 10.0 * CAP_AH;
    group.bench_function("current", |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, Demand::Current(i_1c), &env)),
            BatchSize::LargeInput,
        );
    });

    // Same operating point expressed as power, which takes `solve_current`'s
    // quadratic branch instead of returning the demand unchanged. Operating point
    // matters for this number: 100 series groups at ≈3.27 V and ≈23 A is ≈7.5 kW,
    // comfortably under max power, so the discriminant is positive and the
    // physical (lower-current) root is taken.
    group.bench_function("power", |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, Demand::Power(7500.0), &env)),
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_single_cell, bench_mid_pack, bench_large_pack);
criterion_main!(benches);
