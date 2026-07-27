//! Criterion benchmarks for [`Pack::step`] — the engine's hot loop.
//!
//! `CLAUDE.md` sets a budget of **< 50 µs per step at 100S10P** (1000 cells) on a
//! laptop. That is a budget to keep an eye on, not a test gate: a wall-clock
//! assertion would be machine- and CI-dependent, so nothing here fails a build.
//!
//! Run it with `cargo bench -p sim-core --bench pack_step`. The `--bench` selector
//! is not optional if you pass criterion flags: plain `cargo bench -p sim-core`
//! also runs the lib's (empty) default bench harness, which does not understand
//! `--save-baseline` and aborts the whole invocation.
//!
//! # Comparing two revisions — do not trust a saved baseline
//! Criterion's `--save-baseline` / `--baseline` across separate sessions is
//! **unreliable on this laptop**: it shows a bimodal ~1.4× swing in CPU state
//! between runs (and sometimes within one run), which is larger than any
//! optimisation measured so far. A cross-session comparison once reported −8.6 %
//! for a change that a paired measurement put at −28.5 %.
//!
//! Measure a change by running both revisions back to back and repeating until
//! two rounds agree with tight confidence intervals — a wide CI means the machine
//! was in transition and the number is noise. Absolute figures below are only
//! meaningful against each other.
//!
//! # Methodology
//! Each measured iteration is **one** `step()` on a freshly cloned pack
//! (`iter_batched_ref`, so the clone is excluded from the timing). Stepping one
//! long-lived pack instead would drain it: past SOC 0 the coulomb step takes its
//! clamp branch and the OCV walk moves to a different segment, so the reported
//! number would no longer be the code path it claims to measure.
//!
//! The clone template is **warmed with one step first** (see [`warmed`]). Without
//! that, every measured iteration is the *first* step a pack ever takes, which is
//! not what a client does and — from perf item 3 on — is a different code path.
//! Numbers taken before the warm-up landed are comparable to the ones after: the
//! only difference on the pre-item-3 engine is that the RC overpotentials start
//! nonzero, which costs nothing.
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
//!
//! # Configurations
//! `current`, `power`, and the smaller topologies run with thermal and BMS **off** —
//! the electrical baseline, and the configuration every historical number above was
//! measured on. `full` is the same 1000-cell pack with every Phase 2 feature live.
//! Compare like with like: a ratio taken across a configuration change is exactly the
//! sort of measurement the warning above is about.
//!
//! # Last measured
//! Paired same-session runs, `100S10P/current` mode-matched on both arms (see
//! `docs/plans/pack-step-perf.md` for the full evidence):
//!
//! | case            | `5917bd9` | + items 1–2 | + Phase 2 | + items 4, 3 |
//! | --------------- | --------- | ----------- | --------- | ------------ |
//! | 1S1P/current    | 219 ns    | 179 ns      | —         | −27 %        |
//! | 10S10P/current  | ~8.3 µs   | 6.22 µs     | +4.5 %    | −42.5 %      |
//! | 100S10P/current | 85.9 µs   | 61.5 µs     | ≈ 64 µs   | ≈ 36–42 µs   |
//! | 100S10P/power   | ~86 µs    | 61.8 µs     | —         | −31 %        |
//! | 100S10P/full    | —         | —           | ≈ 67 µs   | ≈ 39–49 µs   |
//!
//! Items 1–2 removed the scratch `Vec` in `r0_lookup` and binary-searched the
//! interpolation breakpoints (−28.5 %). Phase 2 then added ~4 % to the baseline. Item
//! 4 (flat scratch buffer) was worth ~4 % and item 3 (memoising each cell's Thévenin
//! source across the step boundary) ~35 %, landed and benched as separate commits so
//! the two are separately attributed. That puts the step **inside** the 50 µs budget
//! for the first time.
//!
//! `full` — thermal network, sensors, estimator, protection and balancing all live —
//! costs **≈ 4–10 µs on top of `current`**, and that absolute delta is the figure to
//! carry: as a *percentage* it appears to grow (~5 % before items 3–4, ~7–15 % after)
//! purely because item 3 shrank the shared electrical work underneath it.
//!
//! The last column is scaled, not directly read: every Phase 2 and items-3–4 session
//! sat in the machine's slow CPU state, so the *ratios* are the measured quantity and
//! the absolutes come from scaling the fast-state anchor by them. They are ranges
//! because the measured ratios spanned one. See `docs/plans/pack-step-perf.md` for the
//! raw rounds.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use sim_core::bms::{BalancingConfig, BmsConfig, ProtectionConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig};

/// Simulation timestep \[s\] — a typical real-time client step. `dt` only enters
/// through `exp(−dt/τ)` and the coulomb count, so its value does not change the
/// cost of a step.
const DT: f64 = 0.1;

/// Cell capacity \[Ah\], mirroring the chemistry below.
const CAP_AH: f64 = 2.303451;

/// Nominal-ish pack SOC to benchmark at — 0.6 is mid-plateau, the honest common
/// case for a pack in service.
///
/// This used to matter for a second reason: the breakpoint search was a linear
/// scan from the low end, so a step's cost depended on *where* on the table SOC
/// sat. It is a binary search now, so the choice is about representativeness
/// only. Keep it at a value with a full-length bracket search rather than an
/// endpoint, so the clamp fast path does not flatter the measurement.
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
        aging: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
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
            docv_dt_v_per_k: None,
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
///
/// Thermal and BMS off: this is the *electrical* baseline, and it is the
/// configuration every historical measurement in this file was taken on. Use
/// [`make_full_pack`] for the everything-on cost.
fn make_pack(series: u16, parallel: u16) -> Pack {
    Pack::new(
        &pack_config(series, parallel, ThermalConfig::Isothermal, None),
        lfp_like_chem(),
    )
    .expect("benchmark pack config is valid")
}

/// The same pack with every Phase 2 feature live: thermal network, sensors and SOC
/// estimator, protection, and passive balancing. This is what a client that actually
/// wants a simulated BMS pays.
fn make_full_pack(series: u16, parallel: u16) -> Pack {
    let bms = BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: 33.0,
            // Below the resting OCV at SOC 0.6, so bleed switches are actually closed
            // during the benchmark rather than being an untaken branch.
            v_threshold_v: 3.20,
        }),
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.2,
            t_hard_margin_k: 10.0,
        }),
        current_offset_a: 0.01,
        current_noise_sigma_a: 0.02,
        // A handful of probes, as a real pack has — not one per cell.
        temp_probes: vec![
            (0, 0),
            (series / 2, parallel / 2),
            (series - 1, parallel - 1),
        ],
        initial_soc_error: 0.02,
        rest_current_threshold_a: 0.1,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 0.5,
        min_ocv_slope_v_per_soc: 0.5,
    };
    Pack::new(
        &pack_config(
            series,
            parallel,
            ThermalConfig::Network {
                k_neighbor_w_per_k: 1.0,
            },
            Some(bms),
        ),
        lfp_like_chem(),
    )
    .expect("benchmark pack config is valid")
}

fn pack_config(
    series: u16,
    parallel: u16,
    thermal: ThermalConfig,
    bms: Option<BmsConfig>,
) -> PackConfig {
    PackConfig {
        aging: None,
        bms,
        thermal,
        series,
        parallel,
        initial_soc: SOC,
        initial_temp_k: 298.15,
        seed: 0xB0A7,
        scatter: Scatter {
            capacity_sigma: 0.02,
            r0_sigma: 0.05,
        },
    }
}

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Take one step before a pack becomes a clone template.
///
/// `iter_batched_ref` clones the template for every measured iteration, so a
/// never-stepped template would make every measured step the *first* step that pack
/// ever takes. That is not what a client does — and from perf item 3 on it is a
/// materially different code path, because `Pack` reuses each cell's Thévenin
/// `(E, R)` from the previous step's reporting pass and a first step necessarily
/// computes it cold. Benchmarking cold clones would price that cache at exactly
/// zero.
///
/// Warm up with the same `dt` and demand the case measures, so the measured step
/// runs from the state its own demand produces.
fn warmed(mut pack: Pack, demand: Demand) -> Pack {
    pack.step(DT, demand, &env());
    pack
}

/// One step of a single cell — the floor, dominated by the per-step fixed cost
/// rather than by per-cell work (see the module docs).
fn bench_single_cell(c: &mut Criterion) {
    // ~1C discharge for one cell.
    let demand = Demand::Current(CAP_AH);
    let pack = warmed(make_pack(1, 1), demand);
    let env = env();

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
    let demand = Demand::Current(10.0 * CAP_AH);
    let pack = warmed(make_pack(10, 10), demand);
    let env = env();

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
    let env = env();
    let mut group = c.benchmark_group("pack_step/100S10P");

    // ~1C discharge: 10 parallel cells × 2.3 Ah ≈ 23 Ah of pack capacity.
    let i_1c = 10.0 * CAP_AH;
    let cc_pack = warmed(make_pack(100, 10), Demand::Current(i_1c));
    group.bench_function("current", |b| {
        b.iter_batched_ref(
            || cc_pack.clone(),
            |p| black_box(p.step(DT, Demand::Current(i_1c), &env)),
            BatchSize::LargeInput,
        );
    });

    // Same operating point expressed as power, which takes `solve_current`'s
    // quadratic branch instead of returning the demand unchanged. Operating point
    // matters for this number: 100 series groups at ≈3.27 V and ≈23 A is ≈7.5 kW,
    // comfortably under max power, so the discriminant is positive and the
    // physical (lower-current) root is taken.
    let cp_pack = warmed(make_pack(100, 10), Demand::Power(7500.0));
    group.bench_function("power", |b| {
        b.iter_batched_ref(
            || cp_pack.clone(),
            |p| black_box(p.step(DT, Demand::Power(7500.0), &env)),
            BatchSize::LargeInput,
        );
    });

    group.finish();
}

/// The same 1000-cell pack with every Phase 2 feature live.
///
/// The `current` case above is the electrical baseline and the one every historical
/// number in the module docs was measured on; compare against it to price the thermal
/// network, the sensor layer, protection, and balancing together. Keep them as
/// separate cases rather than replacing the baseline — a mixed comparison across a
/// configuration change is exactly the kind of measurement this file warns about.
fn bench_full_pack(c: &mut Criterion) {
    let env = env();
    let i_1c = 10.0 * CAP_AH;
    let mut pack = make_full_pack(100, 10);
    let warm = pack.step(DT, Demand::Current(i_1c), &env);

    // The warm-up must not move the measured step onto a different branch. Two ways
    // it could: a protection trip would latch the contactor and make every measured
    // step the `i_actual == 0` path, and a bleed threshold crossing would take the
    // balancing conductances out of the group solve. Either would silently price a
    // different code path than the historical rows in the module docs.
    assert!(
        warm.flags.contains(EventFlags::BALANCING),
        "warm-up left the bleed switches open; the `full` case would not measure balancing"
    );
    assert!(
        !pack.bms().expect("full pack has a bms").contactor_open(),
        "warm-up latched the contactor; the `full` case would measure an open pack"
    );
    assert!(
        warm.i_actual != 0.0,
        "warm-up derated the demand to zero: {warm:?}"
    );

    c.bench_function("pack_step/100S10P/full", |b| {
        b.iter_batched_ref(
            || pack.clone(),
            |p| black_box(p.step(DT, Demand::Current(i_1c), &env)),
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(
    benches,
    bench_single_cell,
    bench_mid_pack,
    bench_large_pack,
    bench_full_pack
);
criterion_main!(benches);
