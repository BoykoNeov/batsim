//! Phase 3, slice A: calendar fade, cycle fade, and the resistance growth with them.
//!
//! Every aging coefficient in every shipped chemistry is a labelled placeholder, so
//! nothing here asserts a fitted number — the assertions are all about **shape**:
//! fade is monotone, it decelerates like `√t`, it is worse hot and worse full,
//! resistance rises as capacity falls, and a pack with no aging configured is
//! bit-for-bit ageless. The one place an exact number *is* asserted is the
//! path-independence of the calendar integral, which is an arithmetic property of
//! the increment formula rather than a physical claim.
//!
//! These tests also silently exercise the Thévenin memo across an aging update: the
//! `debug_assert` in `Pack::step`'s warm path recomputes every cell's source and
//! compares bits, so any test here that ages a pack in a debug build is checking
//! that the update is correctly sequenced *before* the pass that fills the memo.

use sim_core::aging::{calendar_increment, calendar_rate, cycle_increment, soc_stress};
use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::pack::BuildError;
use sim_core::{AgingConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;

fn env(t_ambient: f64) -> Env {
    Env {
        t_ambient,
        t_coolant: None,
    }
}

/// Aging coefficients scaled so a test can see fade in a handful of simulated
/// months rather than a decade — deliberately faster than anything shipped, since
/// these tests are about shape, not magnitude.
fn aging_params() -> AgingParams {
    AgingParams {
        cal_pre_exp: 1.0e4,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0, 1.0, 2.0],
        cyc_fade_per_ah: 1.0e-4,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    }
}

/// A sloped-OCV, single-RC chemistry. `aging` is a parameter so the same cell can be
/// built with and without health coefficients.
fn chem(aging: Option<AgingParams>) -> ChemistryParams {
    ChemistryParams {
        aging,
        meta: ChemMeta {
            id: "aging_test".into(),
            name: "Aging test cell".into(),
            provenance: "test fixture — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![3.00, 3.30, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

fn cfg(initial_soc: f64, initial_temp_k: f64, aging: Option<AgingConfig>) -> PackConfig {
    PackConfig {
        aging,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k,
        seed: 7,
        scatter: Scatter::default(),
    }
}

/// Age a resting pack for `steps` steps of `dt` and return its capacity SOH.
fn rest_and_age(initial_soc: f64, temp_k: f64, dt: f64, steps: usize, period_s: f64) -> f64 {
    let cfg = cfg(
        initial_soc,
        temp_k,
        Some(AgingConfig {
            sub_clock_period_s: period_s,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(temp_k);
    let mut tele = pack.step(0.0, Demand::Rest, &e);
    for _ in 0..steps {
        tele = pack.step(dt, Demand::Rest, &e);
    }
    tele.soh_capacity
}

// --- the off switch -------------------------------------------------------

/// With no `PackConfig::aging`, health is not "approximately one" — it is the
/// literal `1.0` the pack was built with, on every cell and in the telemetry, no
/// matter how hard the pack is worked. Aging is a toggleable component, and off
/// means off.
#[test]
fn a_pack_without_aging_never_wears_out() {
    let cfg = cfg(0.9, 313.15, None);
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(313.15);
    for i in 0..500_u32 {
        // Alternate discharge and charge so throughput accumulates in both
        // directions — the cycle-fade path as well as the calendar one.
        let demand = if i.is_multiple_of(2) {
            Demand::Current(2.0)
        } else {
            Demand::Current(-2.0)
        };
        let tele = pack.step(60.0, demand, &e);
        assert_eq!(
            tele.soh_capacity, 1.0,
            "capacity SOH drifted with aging off"
        );
        assert_eq!(
            tele.soh_resistance, 1.0,
            "resistance SOH drifted with aging off"
        );
    }
    let cell = pack.cell(0, 0).expect("cell exists");
    assert_eq!(cell.soh_capacity, 1.0);
    assert_eq!(cell.soh_resistance, 1.0);
}

/// A chemistry with no `[aging]` section cannot be aged, and saying so is a build
/// error rather than a pack that quietly never fades. The two are indistinguishable
/// from the outside, which is exactly why this is rejected.
#[test]
fn aging_without_chemistry_coefficients_is_a_build_error() {
    let cfg = cfg(0.5, 298.15, Some(AgingConfig::default()));
    let err = Pack::new(&cfg, chem(None)).expect_err("must reject");
    assert_eq!(
        err,
        BuildError::MissingAgingParams {
            chem_id: "aging_test".into()
        }
    );
    // ...and the same chemistry is perfectly fine for a pack that does not age.
    let cfg_off = cfg_without_aging();
    Pack::new(&cfg_off, chem(None)).expect("ageless pack builds against ageless chemistry");
}

fn cfg_without_aging() -> PackConfig {
    cfg(0.5, 298.15, None)
}

#[test]
fn a_negative_sub_clock_period_is_a_build_error() {
    let cfg = cfg(
        0.5,
        298.15,
        Some(AgingConfig {
            sub_clock_period_s: -1.0,
        }),
    );
    let err = Pack::new(&cfg, chem(Some(aging_params()))).expect_err("must reject");
    assert_eq!(err, BuildError::BadAgingPeriod(-1.0));
}

// --- calendar fade --------------------------------------------------------

/// Fade is monotone downward and **decelerating**: successive equal intervals cost
/// successively less capacity. That deceleration is the `√t` signature, and it is
/// the thing that distinguishes calendar fade from a linear leak.
#[test]
fn calendar_fade_is_monotone_and_decelerates() {
    let cfg = cfg(
        1.0,
        313.15,
        Some(AgingConfig {
            sub_clock_period_s: 0.0,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(313.15);
    let day = 86_400.0;

    let mut soh = 1.0;
    let mut increments = Vec::new();
    for _ in 0..30 {
        let tele = pack.step(day, Demand::Rest, &e);
        increments.push(soh - tele.soh_capacity);
        assert!(
            tele.soh_capacity < soh,
            "capacity SOH must strictly decrease while resting"
        );
        soh = tele.soh_capacity;
    }

    for w in increments.windows(2) {
        assert!(
            w[1] < w[0],
            "daily fade must shrink day over day (sqrt(t) shape), got {:?} then {:?}",
            w[0],
            w[1]
        );
    }
    assert!(
        soh < 1.0 && soh > 0.5,
        "fade should be visible but sane: {soh}"
    );
}

/// The calendar integral is **exact over any partition** at constant stress: one
/// step of a day fades a cell by precisely as much as 86 400 steps of a second.
///
/// This is the property the rationalised increment formula exists to protect. The
/// algebraically identical `k·(√(t+dt) − √t)` passes at coarse `dt` and quietly
/// loses most of its significant digits at fine `dt` against a large accumulated
/// age — which is the regime a fast-forward spends all its time in.
#[test]
fn calendar_fade_is_independent_of_how_the_interval_is_divided() {
    let day = 86_400.0;
    let coarse = rest_and_age(1.0, 313.15, day, 10, 0.0);
    let fine = rest_and_age(1.0, 313.15, day / 24.0, 240, 0.0);
    let rel = (coarse - fine).abs() / (1.0 - coarse);
    assert!(
        rel < 1e-9,
        "same elapsed time, different step size: {coarse} vs {fine} (rel {rel})"
    );
}

/// Arrhenius: the same cell, the same time, a warmer shelf — more fade. This is the
/// single most important qualitative fact about calendar aging.
#[test]
fn calendar_fade_is_worse_when_hot() {
    let cool = rest_and_age(0.5, 288.15, 86_400.0, 30, 10.0);
    let warm = rest_and_age(0.5, 318.15, 86_400.0, 30, 10.0);
    assert!(
        warm < cool,
        "a hot shelf must age the cell faster: {warm} (45 C) vs {cool} (15 C)"
    );
}

/// SOC stress: storing full is worse than storing half. The fixture's stress table
/// is flat from 0 to 0.5 and doubles by 1.0, so a mid-SOC and an empty cell fade
/// identically while a full one fades faster.
#[test]
fn calendar_fade_is_worse_when_full() {
    let half = rest_and_age(0.5, 313.15, 86_400.0, 30, 10.0);
    let full = rest_and_age(1.0, 313.15, 86_400.0, 30, 10.0);
    assert!(
        full < half,
        "storing at full charge must age faster: {full} vs {half}"
    );
}

// --- cycle fade and resistance growth -------------------------------------

/// Cycling costs capacity *on top of* calendar fade, and the cost scales with how
/// much charge actually moved.
///
/// The obvious version of this test — cycle hard, cycle gently, compare — has a
/// confound: a wider SOC swing also spends time at higher SOC, where the calendar
/// stress factor is larger, so some of the extra fade would not be cycle fade at all.
/// The control run zeroes `cyc_fade_per_ah` and re-runs both, which isolates exactly
/// that confound and lets the assertion be about the cycle term specifically.
#[test]
fn cycle_fade_scales_with_throughput() {
    // Fade after 40 alternating ten-minute half-cycles at ±`current`.
    let cycled = |current: f64, cyc_fade_per_ah: f64| {
        let cfg = cfg(
            0.5,
            298.15,
            Some(AgingConfig {
                sub_clock_period_s: 10.0,
            }),
        );
        let mut params = aging_params();
        params.cyc_fade_per_ah = cyc_fade_per_ah;
        let mut pack = Pack::new(&cfg, chem(Some(params))).expect("pack builds");
        let e = env(298.15);
        let mut tele = pack.step(0.0, Demand::Rest, &e);
        for i in 0..40_u32 {
            let sign = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
            for _ in 0..10 {
                tele = pack.step(60.0, Demand::Current(sign * current), &e);
            }
        }
        1.0 - tele.soh_capacity
    };

    let rate = aging_params().cyc_fade_per_ah;
    let gentle = cycled(0.25, rate);
    let hard = cycled(1.0, rate);
    assert!(
        hard > gentle,
        "four times the throughput must cost more capacity: {hard} vs {gentle}"
    );

    // The control: same trajectories, cycle fade switched off. Whatever gap remains
    // is the SOC-swing calendar confound, and it must be a small part of the gap
    // above — otherwise this test is measuring the wrong mechanism.
    let gentle_cal = cycled(0.25, 0.0);
    let hard_cal = cycled(1.0, 0.0);
    let total_gap = hard - gentle;
    let calendar_gap = hard_cal - gentle_cal;
    assert!(
        calendar_gap.abs() < 0.1 * total_gap,
        "the throughput gap {total_gap} is mostly calendar ({calendar_gap}), not cycle fade"
    );
    assert!(gentle > 0.0 && hard > 0.0, "both runs must have faded");
}

/// `CLAUDE.md` forbids modelling capacity fade without the matching resistance
/// growth. Whatever capacity a pack loses, its resistance must have risen with it,
/// by the chemistry's stated ratio.
#[test]
fn resistance_grows_in_step_with_capacity_loss() {
    let params = aging_params();
    let cfg = cfg(
        1.0,
        318.15,
        Some(AgingConfig {
            sub_clock_period_s: 10.0,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(params.clone()))).expect("pack builds");
    let e = env(318.15);
    let mut tele = pack.step(0.0, Demand::Rest, &e);
    for _ in 0..60 {
        tele = pack.step(86_400.0, Demand::Rest, &e);
    }

    let loss = 1.0 - tele.soh_capacity;
    assert!(loss > 0.01, "need visible fade to test against: {loss}");
    assert!(tele.soh_resistance > 1.0, "resistance must have grown");

    // On a 1S1P pack the pack-level ratio is exactly the cell's growth factor.
    let expected = 1.0 + params.r_growth_per_capacity_loss * loss;
    let rel = (tele.soh_resistance - expected).abs() / expected;
    assert!(
        rel < 1e-12,
        "pack resistance ratio {} should equal the cell factor {expected}",
        tele.soh_resistance
    );

    // The pack aggregates are ratios of sums, so on a 1S1P pack they *equal* the
    // cell's own factors only up to the last bit — `(x·s)/x` is not bit-exactly `s`.
    let cell = pack.cell(0, 0).expect("cell exists");
    assert!((cell.soh_resistance - tele.soh_resistance).abs() < 1e-12);
    assert!((cell.soh_capacity - tele.soh_capacity).abs() < 1e-12);
}

/// An aged pack holds less charge at the same SOC — that is what "faded" means. SOC
/// itself keeps meaning *fraction of present capacity*, so a rested aged cell still
/// reads the SOC it was left at rather than being silently rescaled.
#[test]
fn soc_is_a_fraction_of_present_capacity() {
    let cfg = cfg(
        1.0,
        318.15,
        Some(AgingConfig {
            sub_clock_period_s: 10.0,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(318.15);
    for _ in 0..60 {
        pack.step(86_400.0, Demand::Rest, &e);
    }
    let tele = pack.step(0.0, Demand::Rest, &e);
    assert!(tele.soh_capacity < 1.0, "need an aged pack");
    // Rested at full charge the whole time: SOC is untouched by the fade. Not
    // *exactly* 1.0 — a rested group's node voltage is a ratio of sums, so each cell
    // carries a rounding-sized circulating current and coulomb counting nudges SOC by
    // ~1e-12 over 60 steps. That artifact predates aging and is not what this checks.
    assert!(
        (tele.soc_true - 1.0).abs() < 1e-9,
        "resting must not move SOC, however much capacity was lost: {}",
        tele.soc_true
    );

    // The charge actually available is what shrank. Discharge the aged pack and a
    // fresh one at the same current for the same time; the aged one drops further
    // in SOC because the same coulombs are a larger fraction of a smaller tank.
    let soh = tele.soh_capacity;
    let drop = |pack: &mut Pack| {
        let before = pack.step(0.0, Demand::Rest, &e).soc_true;
        for _ in 0..60 {
            pack.step(60.0, Demand::Current(1.0), &e);
        }
        before - pack.step(0.0, Demand::Rest, &e).soc_true
    };
    let aged_drop = drop(&mut pack);
    let mut fresh = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let fresh_drop = drop(&mut fresh);
    assert!(
        aged_drop > fresh_drop,
        "the same coulombs must move SOC further on an aged pack: {aged_drop} vs {fresh_drop}"
    );
    // Quantitatively the ratio of the two drops is the capacity ratio — but only to
    // about a percent, and the residual is instructive rather than sloppy. `√t` fade
    // is steepest at the very beginning, so the *fresh* pack loses a few tenths of a
    // percent of capacity during the same measurement hour, which deepens its own SOC
    // drop. The aged pack, far out on the flat tail of the curve, barely moves.
    let ratio = fresh_drop / aged_drop;
    assert!(
        (ratio - soh).abs() < 2e-2,
        "SOC drop should scale as 1/soh_capacity: ratio {ratio} vs soh {soh}"
    );
}

// --- determinism ----------------------------------------------------------

/// A zero-length step is an observation, not a tick — including for aging. It must
/// not advance the sub-clock, accumulate throughput, or re-anchor the
/// depth-of-discharge reference, all of which react to *information* rather than to
/// elapsed time.
#[test]
fn a_zero_length_step_does_not_age_anything() {
    let cfg = cfg(
        0.8,
        313.15,
        Some(AgingConfig {
            sub_clock_period_s: 100.0,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(313.15);
    // Land mid-period with some throughput banked, so a leaked tick would show.
    for _ in 0..3 {
        pack.step(30.0, Demand::Current(1.5), &e);
    }
    let before = bincode::serialize(&pack.snapshot()).expect("snapshot serializes");
    for _ in 0..5 {
        // Reversing the demand on the probe steps would re-anchor the DOD reference
        // if the gate were missing.
        pack.step(0.0, Demand::Current(-1.5), &e);
    }
    let after = bincode::serialize(&pack.snapshot()).expect("snapshot serializes");
    assert_eq!(before, after, "a zero-length step mutated aging state");
}

/// Snapshot **mid-period** — with a partial sub-clock interval banked and unspent
/// throughput on every cell — restore, and continue. The two trajectories must be
/// bit-identical. A partial period is carried, not dropped; dropping it would make
/// the act of taking a snapshot change the physics.
#[test]
fn snapshot_mid_period_replays_bit_identically() {
    let cfg = cfg(
        0.9,
        313.15,
        Some(AgingConfig {
            sub_clock_period_s: 250.0,
        }),
    );
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()))).expect("pack builds");
    let e = env(313.15);
    let demand_at = |i: usize| {
        if (i / 7).is_multiple_of(2) {
            Demand::Current(1.25)
        } else {
            Demand::Current(-1.25)
        }
    };

    // Run to a point that is deliberately not a multiple of the sub-clock period.
    for i in 0..37 {
        pack.step(30.0, demand_at(i), &e);
    }
    let aging = pack.aging().expect("pack ages");
    assert!(
        aging.pending_s() > 0.0 && aging.pending_s() < aging.sub_clock_period_s(),
        "test must snapshot mid-period, got {} of {}",
        aging.pending_s(),
        aging.sub_clock_period_s()
    );

    let bytes = bincode::serialize(&pack.snapshot()).expect("snapshot serializes");
    let restored: sim_core::Snapshot = bincode::deserialize(&bytes).expect("snapshot round-trips");
    let mut replay = Pack::restore(&restored).expect("restore succeeds");

    for i in 37..120 {
        let a = pack.step(30.0, demand_at(i), &e);
        let b = replay.step(30.0, demand_at(i), &e);
        assert_eq!(
            (a.soh_capacity.to_bits(), a.soh_resistance.to_bits()),
            (b.soh_capacity.to_bits(), b.soh_resistance.to_bits()),
            "health diverged after restore at step {i}"
        );
        assert_eq!(
            (a.v_terminal.to_bits(), a.soc_true.to_bits()),
            (b.v_terminal.to_bits(), b.soc_true.to_bits()),
            "trajectory diverged after restore at step {i}"
        );
    }
    assert!(
        pack.cell(0, 0).expect("cell exists").soh_capacity < 1.0,
        "the replay must actually have aged, or it proves nothing"
    );
}

// --- the pure functions ---------------------------------------------------

/// The rationalised increment is the difference of square roots — where the naive
/// form still has digits to spare. Checking it against the textbook expression at a
/// modest age is how we know the algebra is right; the cancellation the rationalised
/// form avoids only bites far from here.
#[test]
fn calendar_increment_matches_the_naive_difference_where_that_is_accurate() {
    let k = 3.0e-5;
    for &(q, dt) in &[(0.0_f64, 3600.0_f64), (0.05, 3600.0), (0.2, 86_400.0)] {
        let t_eq = (q / k).powi(2);
        let naive = k * ((t_eq + dt).sqrt() - t_eq.sqrt());
        let got = calendar_increment(k, q, dt);
        let rel = (got - naive).abs() / naive;
        assert!(rel < 1e-9, "q={q} dt={dt}: {got} vs {naive} (rel {rel})");
    }
}

/// Accumulating the increment step by step reproduces `k·√t` exactly, whatever the
/// step size — the state (`q_cal`) really is a sufficient statistic for the age.
#[test]
fn accumulated_calendar_increments_reproduce_sqrt_t() {
    let k = 3.0e-5;
    let total = 30.0 * 86_400.0;
    for steps in [1_u32, 10, 1000, 100_000] {
        let dt = total / f64::from(steps);
        let mut q = 0.0;
        for _ in 0..steps {
            q += calendar_increment(k, q, dt);
        }
        let exact = k * total.sqrt();
        let rel = (q - exact).abs() / exact;
        assert!(
            rel < 1e-12,
            "{steps} steps: accumulated {q} vs exact {exact} (rel {rel})"
        );
    }
}

/// Degenerate inputs return zero rather than NaN or infinity: `step` must never
/// produce a poisoned state, and these are all reachable (a zero coefficient, a
/// probe step, a pack driven outside the physical domain).
#[test]
fn degenerate_aging_inputs_are_inert() {
    assert_eq!(calendar_increment(0.0, 0.1, 3600.0), 0.0);
    assert_eq!(calendar_increment(1e-5, 0.1, 0.0), 0.0);
    assert_eq!(calendar_increment(f64::NAN, 0.1, 3600.0), 0.0);
    let params = aging_params();
    assert_eq!(calendar_rate(&params, 0.0, 0.5), 0.0);
    assert_eq!(calendar_rate(&params, -10.0, 0.5), 0.0);
    assert_eq!(cycle_increment(&params, 0.0, 0.5), 0.0);
    assert_eq!(cycle_increment(&params, -1.0, 0.5), 0.0);
}

/// The stress table's breakpoints are implied: `n` entries sit uniformly across
/// \[0, 1\]. Three entries mean SOC 0.0 / 0.5 / 1.0, which is what the shipped
/// chemistries' comments claim.
#[test]
fn soc_stress_interpolates_over_implied_uniform_breakpoints() {
    let table = [1.0, 1.0, 1.4];
    assert_eq!(soc_stress(&table, 0.0), 1.0);
    assert_eq!(soc_stress(&table, 0.5), 1.0);
    assert_eq!(soc_stress(&table, 1.0), 1.4);
    assert!((soc_stress(&table, 0.75) - 1.2).abs() < 1e-12);
    // Clamped outside [0, 1], and a single entry is a constant.
    assert_eq!(soc_stress(&table, -1.0), 1.0);
    assert_eq!(soc_stress(&table, 2.0), 1.4);
    assert_eq!(soc_stress(&[0.7], 0.3), 0.7);
}

/// The depth-of-discharge weight is `D^(exp−1)` — per *amp-hour*, not per cycle.
/// An exponent of exactly 1 is pure throughput counting: weight 1 at every depth,
/// including zero.
#[test]
fn cycle_weight_uses_the_per_amp_hour_exponent() {
    let mut params = aging_params();
    params.cyc_dod_stress_exp = 2.0;
    // exp = 2 ⇒ weight = D. Half-depth throughput costs half as much per Ah.
    let full = cycle_increment(&params, 1.0, 1.0);
    let half = cycle_increment(&params, 1.0, 0.5);
    assert!((half / full - 0.5).abs() < 1e-12);

    params.cyc_dod_stress_exp = 1.0;
    let flat = cycle_increment(&params, 1.0, 1.0);
    assert_eq!(cycle_increment(&params, 1.0, 0.25), flat);
    assert_eq!(cycle_increment(&params, 1.0, 0.0), flat);
}

/// Resting mid-discharge must not score one deep cycle as two shallow ones.
///
/// This is the test that caught the design's first real bug. Anchoring the
/// depth-of-discharge reference on each *cell's* current sign seemed obviously right,
/// and it silently broke on any pack with manufacturing scatter: at rest the cells of
/// a parallel group circulate real current through each other, so half of them see a
/// sign flip the moment the load comes off, discarding the depth accounting for the
/// discharge in progress. On this 1S2P pack with 5 % scatter it cut one cell's cycle
/// fade by 5.8 % for no physical reason. The direction now comes from the pack
/// current, which is exactly zero at rest.
///
/// Calendar fade is switched off so the ten idle hours contribute nothing but the
/// circulation itself, and the comparison is purely about the cycle term.
#[test]
fn resting_mid_discharge_does_not_split_the_cycle() {
    let mut c = cfg(
        0.9,
        298.15,
        Some(AgingConfig {
            sub_clock_period_s: 10.0,
        }),
    );
    c.parallel = 2;
    c.scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.05,
    };
    let mut params = aging_params();
    params.cal_pre_exp = 0.0;
    let e = env(298.15);

    let mut rested = Pack::new(&c, chem(Some(params.clone()))).expect("pack builds");
    for _ in 0..60 {
        rested.step(60.0, Demand::Current(2.0), &e);
    }
    for _ in 0..600 {
        rested.step(60.0, Demand::Rest, &e);
    }
    for _ in 0..60 {
        rested.step(60.0, Demand::Current(2.0), &e);
    }

    let mut straight = Pack::new(&c, chem(Some(params))).expect("pack builds");
    for _ in 0..120 {
        straight.step(60.0, Demand::Current(2.0), &e);
    }

    for p in 0..2 {
        let a = 1.0 - rested.cell(0, p).expect("cell exists").soh_capacity;
        let b = 1.0 - straight.cell(0, p).expect("cell exists").soh_capacity;
        assert!(b > 0.0, "cell {p} must have accrued cycle fade");
        let ratio = a / b;
        // The rest is allowed to add a little fade — circulating current between
        // mismatched cells is real charge moving, and ten hours of it counts. It must
        // not *remove* any, which is what a spurious re-anchor would do.
        assert!(
            (0.97..=1.05).contains(&ratio),
            "cell {p}: resting changed cycle fade by more than circulation explains              (rested {a}, straight {b}, ratio {ratio})"
        );
    }
}
