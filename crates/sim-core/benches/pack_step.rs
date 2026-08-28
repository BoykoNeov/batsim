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
//! measured on. `full` is the same 1000-cell pack with every Phase 2 feature live, and
//! `full+aging` / `full+aging_every_step` add Phase 3's aging on top of that.
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
//!
//! ## Phase 3 (slice E re-measure)
//! Paired alternating rounds against `9da78ef`, the last pre-aging tree. Phase 3's four
//! slices cost **+7–10 %** at `100S10P` (both `current` and `full`) and **+12–14 %** at
//! `1S1P`. This box was again in its slow state — the baseline arm measured 51–55 µs for
//! `100S10P/current` against the 36–42 µs recorded above — so the ratio is the measured
//! quantity and no absolute conclusion is drawn from the session. Scaled onto the
//! fast-state anchor the fully-featured step lands at ≈ 42–54 µs, which makes the < 50 µs
//! budget **marginal rather than met**.
//!
//! Aging splits in two, measured within single runs so the three are mode-matched:
//! `full` 68.8/68.5 µs, `full+aging` 70.8/68.4 µs, `full+aging_every_step` 104.8/102.3 µs.
//! The always-paid part of aging is below this box's noise floor; the sub-clock tick costs
//! **+50 %** on a step that runs it, which at the shipped 10 s period against this file's
//! 0.1 s `DT` is one step in a hundred. `docs/plans/phase-3-aging-faults.md` (slice E) has
//! the full evidence and the reason the `r0_factor · soh_resistance` cache was declined.
//!
//! ## Cell footprint (boxing the porous variants)
//! `Cell` had reached **264 bytes**, and the largest term was not aging but `CellModel`:
//! an enum is as wide as its largest variant, so Phase 7's `DfnState` (136 B) was being
//! carried by every ECM cell in every pack. Boxing `Spm` and `Dfn` took `CellModel`
//! 136 → 56 B and `Cell` 264 → **184 B**, worth **≈ 1.1–1.5 µs** at `100S10P/current`
//! over two alternating paired rounds whose base arms agreed to 0.03 %, and **nothing at
//! `1S1P`** — which is the pre-registered discriminator saying the win is footprint and
//! not instructions.
//!
//! **Quoted as an absolute on purpose.** Those rounds ran against a 79 µs baseline, so the
//! same delta is −1.4 to −1.9 % *there* and would read near −2.5 % in the fast state if it
//! is a fixed cost, or stay at −1.4 % if it is proportional. Two rounds in one CPU state
//! cannot tell those apart, and the percentage is the half that does not travel — the same
//! reason `docs/plans/pack-step-perf.md` says to carry absolutes when the denominator
//! moved, except that here the denominator is the machine.
//!
//! Two things to carry forward. **≈ 30 % of the footprint bought ≈ 1.5 % of the step**, so
//! this step is not memory-bound at that size; the deferred `CellAging` split (72 B/cell)
//! is declined because its cost is certain and large — a snapshot-layout change and a
//! version bump — against a benefit that measurement bounds small. And this box swung
//! **52.3–79.2 µs on the same binary within one batch**, producing a +26.7 % and a −30.0 %
//! reading of the same change in consecutive rounds — the widest yet recorded here, and
//! the reason the alternating order above is load-bearing rather than tidy. See
//! `docs/plans/cell-size.md`.
//! `crates/sim-core/src/pack.rs::cell_footprint` pins the widths so the enum cannot
//! silently re-widen; it did so for two whole phases because nothing was looking.
//!
//! ## `v_rc` inlined — a size result with no timing result, and that is the whole entry
//! `EcmState::v_rc` was a `Vec<f64>` of one or two entries: a heap block and a dependent
//! load **per cell per pass**, on four passes. As `[f64; 2]` it took `EcmState` 48 → 40 B
//! and `Cell` 184 → **176 B** — a third off the 264 B this line of work started at — and
//! removed 1000 allocations from a 1000-cell pack. Predicted exactly, before the arms ran.
//!
//! **Ten paired alternating rounds across two batches measured nothing.** The same two
//! binaries read −13.4 % to +8.1 % at `100S10P`, sign flipping *inside* each batch, base
//! arms spanning 53.8–67.6 µs. Batch 2 ran under a stopping rule registered before it
//! started (both arms' CI ≤ 1.0 %, base arms agreeing to 2 %, deltas agreeing in sign);
//! **no round was admissible**, so it is reported inconclusive and no third batch was run.
//! Two of its rounds read 173 µs and 334 µs — three to five times anything this bench has
//! recorded — so the box now has a contention failure mode on top of its CPU-state swing.
//!
//! **No timing claim is made for it in either direction**, and it is on record as landing
//! on user direction plus a countable mechanism. Read that as the standing warning it is: a
//! 30 %-footprint change *was* measurable here one commit earlier at 0.03 % reproducibility,
//! and an 8 B/cell change on the same box was not measurable at all. Before quoting any
//! number from this file, check that the box can still reproduce its own base arm. See
//! `docs/plans/cell-size.md`.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use sim_core::bms::{BalancingConfig, BmsConfig, ProtectionConfig};
use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams,
    ThermalParams,
};
use sim_core::{
    AgingConfig, CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

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
        diffusion: None,
        hysteresis: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
            fade_per_ah: 0.0,
        },
        spm: None,
        dfn: None,
        // Present, like the shipped file's `[aging]` section. A chemistry that cannot
        // age makes `PackConfig::aging = Some(..)` a build error, so the aging cases
        // below need this — and the cases that leave aging off are unaffected by it,
        // since nothing reads these coefficients until a pack asks to wear out.
        aging: Some(AgingParams {
            cal_pre_exp: 1.0e4,
            cal_ea_j_per_mol: 5.0e4,
            cal_soc_stress: vec![1.0, 1.0, 1.4],
            cyc_fade_per_ah: 2.0e-5,
            cyc_dod_stress_exp: 1.1,
            r_growth_per_capacity_loss: 1.5,
        }),
        // Present, like the shipped file's `[safety]` section, so the benchmark pays
        // the per-cell plating check *and* the per-cell onset comparison that a real
        // pack running this chemistry pays. Setting it to `None` would measure a
        // configuration nobody ships. The benched pack never approaches onset, which is
        // also the case worth measuring — a pack on fire has no perf budget.
        safety: Some(SafetyParams {
            t_onset_k: 423.15,
            t_vent_k: 453.15,
            runaway_energy_j: 24.0e3,
            runaway_power_w_at_onset: 5.0,
            runaway_ea_j_per_mol: 1.0e5,
            t_plating_min_k: Some(273.15),
            plating_c_threshold: Some(0.5),
            plating_fade_per_ah: 1.0e-3,
            plating_short_hazard_per_ah: 1.0e-3,
            plating_short_ohms: 50.0,
        }),
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
            t_ref_k: None,
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
        &pack_config(series, parallel, ThermalConfig::Isothermal, None, None),
        lfp_like_chem(),
    )
    .expect("benchmark pack config is valid")
}

/// The same pack with every Phase 2 feature live: thermal network, sensors and SOC
/// estimator, protection, and passive balancing, plus optional aging on top.
///
/// `aging` is a parameter rather than a constant because the two configurations answer
/// different questions and the module docs forbid mixing them: `None` is the
/// end-of-Phase-2 `full` case every historical row was measured on, and `Some` prices
/// what Phase 3 slice A costs a client that wants its pack to wear out.
fn make_full_pack(series: u16, parallel: u16, aging: Option<AgingConfig>) -> Pack {
    let bms = BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: 33.0,
            // Below the resting OCV at SOC 0.6, so bleed switches are actually closed
            // during the benchmark rather than being an untaken branch.
            v_threshold_v: 3.20,
            v_release_band_v: 0.010,
        }),
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.2,
            t_hard_margin_k: 10.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
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
            aging,
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
    aging: Option<AgingConfig>,
) -> PackConfig {
    PackConfig {
        aging,
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
        cell_model: CellModelConfig::Ecm,
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
    let mut pack = make_full_pack(100, 10, None);
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

/// The everything-on pack with **aging live as well** — the Phase 3 configuration.
///
/// Two cases, because the aging sub-clock splits the cost in two and averaging them
/// would hide both:
///
/// * `full+aging` runs the shipped-default 10 s period against this file's `DT` of
///   0.1 s, so the tick fires on one step in a hundred and the measured step is the
///   other ninety-nine: the always-paid part (`eff_r0_factor`'s extra multiply per
///   cell, and the SOH aggregation in the reporting pass). Every measured iteration is
///   a clone of the same warmed template, so the accumulator sits at the same 0.1 s
///   each time and the tick genuinely never fires inside the timed region — this is a
///   clean measurement of the non-ticking path, not an average over both.
/// * `full+aging_every_step` sets the period to zero, which is a legitimate
///   configuration and makes every step a ticking one. That is the upper bound, and it
///   is the number that would have to move for the "cache `r0_factor · soh_resistance`
///   as a derived field" optimisation sketched in `docs/plans/phase-3-aging-faults.md`
///   (slice A) to be worth its correctness obligation.
///
/// Neither replaces `full`. A ratio taken across a configuration change is the mistake
/// the module docs open with, so the Phase 2 row stays measurable on its own terms.
fn bench_aging_pack(c: &mut Criterion) {
    let env = env();
    let i_1c = 10.0 * CAP_AH;
    let mut group = c.benchmark_group("pack_step/100S10P");

    for (name, period_s) in [("full+aging", 10.0), ("full+aging_every_step", 0.0)] {
        let mut pack = make_full_pack(
            100,
            10,
            Some(AgingConfig {
                sub_clock_period_s: period_s,
            }),
        );
        let warm = pack.step(DT, Demand::Current(i_1c), &env);
        // The same warm-up guards the `full` case uses: a latched contactor or an open
        // bleed switch would silently price a different code path.
        assert!(
            warm.flags.contains(EventFlags::BALANCING),
            "{name}: warm-up left the bleed switches open"
        );
        assert!(
            !pack.bms().expect("full pack has a bms").contactor_open(),
            "{name}: warm-up latched the contactor"
        );
        assert!(warm.i_actual != 0.0, "{name}: warm-up derated to zero");

        group.bench_function(name, |b| {
            b.iter_batched_ref(
                || pack.clone(),
                |p| black_box(p.step(DT, Demand::Current(i_1c), &env)),
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_single_cell,
    bench_mid_pack,
    bench_large_pack,
    bench_full_pack,
    bench_aging_pack
);
criterion_main!(benches);
