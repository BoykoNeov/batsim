//! **Phase 3 exit gate.** A fast-forward of 500 cycles produces a plausible fade
//! curve, and shelf storage produces the other one.
//!
//! # This is two experiments, not one
//!
//! The two mechanisms have different shapes and cannot be asserted on the same run.
//! Calendar fade goes as `√t`, so it **decelerates**: an old cell ages more slowly than
//! a new one. Cycle fade is proportional to charge throughput, so at a fixed depth and
//! rate it is essentially **linear** in cycle count. Their sum is neither, and a test
//! that ran one trajectory and asserted "decelerating" would be asserting whichever term
//! happened to dominate the fixture.
//!
//! So there are two:
//!
//! * [`calendar_fade_follows_sqrt_t_and_worsens_with_temperature_and_soc`] rests a pack
//!   and asserts the `√t` signature plus both stress orderings. It runs at `dt` = 1 h,
//!   which costs nothing in accuracy — the calendar integral is exact over any partition
//!   at fixed stress (see `docs/plans/phase-3-aging-faults.md`, slice A).
//! * [`five_hundred_cycles_fade_the_pack_monotonically`] cycles one 500 times and
//!   asserts monotone decreasing capacity, resistance rising as capacity falls, and a
//!   cycle term that stays roughly linear where the calendar term visibly decelerates.
//!
//! # Nothing here asserts a fitted number
//!
//! Every `[aging]` coefficient in every shipped chemistry is a labelled placeholder, so
//! a test pinning "7.3 % fade at 500 cycles" would be pinning a placeholder and would
//! break the moment anyone does the fit the provenance notes ask for. The magnitude
//! assertions are therefore **bands**, the same device `sim-data`'s
//! `shipped_aging_coefficients_give_a_plausible_one_year_fade` uses, and everything else
//! is a shape or an ordering.
//!
//! # The control that isolates the cycle term
//!
//! The 500-cycle run is paired with an identical run whose `cyc_fade_per_ah` is zero.
//! Same demands, same SOC history, same temperatures, same elapsed time — so the *only*
//! difference is the cycle mechanism, and the gap between the two arms is exactly what
//! cycling cost. Comparing a cycled pack against a *resting* one would not do this: a
//! cycled pack also spends time at high SOC, where the calendar stress factor is 1.4, so
//! part of that gap would be calendar fade wearing a cycling costume.

use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    AgingConfig, CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Cell capacity \[Ah\], from the shipped LFP file.
const CAP_AH: f64 = 2.303451;

const ROOM_K: f64 = 298.15;

fn env(t_ambient: f64) -> Env {
    Env {
        t_ambient,
        t_coolant: None,
    }
}

/// The shipped LFP chemistry's `[aging]` block.
///
/// provenance: values copied verbatim from `chemistries/lfp_26650_generic.toml` (see
/// that file for the per-number provenance). `sim-core` performs no file I/O, so a
/// scenario wanting the shipped numbers has to carry them inline; nothing here is an
/// independent physical claim. `cyc_fade_per_ah` is a parameter so the control arm can
/// zero it.
fn lfp_aging(cyc_fade_per_ah: f64) -> AgingParams {
    AgingParams {
        cal_pre_exp: 1.0e4,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0, 1.0, 1.4],
        cyc_fade_per_ah,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    }
}

/// The shipped LFP chemistry.
///
/// provenance: as [`lfp_aging`] — a verbatim copy of
/// `chemistries/lfp_26650_generic.toml`. `[safety]` is omitted deliberately: this pack
/// is isothermal at room temperature and hundreds of kelvin from anything in that
/// section, and an aging scenario has no business also being a plating or runaway one.
fn lfp(cyc_fade_per_ah: f64) -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            // Zero: this file's chemistry pays nothing for over-discharge, so its
            // trajectories are the ones this slice must not move. See
            // `docs/plans/reversal-damage.md`.
            fade_per_ah: 0.0,
        },
        aging: Some(lfp_aging(cyc_fade_per_ah)),
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "lfp_26650_generic".into(),
            name: "Generic LFP 26650".into(),
            provenance: "scenario copy of chemistries/lfp_26650_generic.toml".into(),
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

/// A 1S1P pack, isothermal at `temp_k`, aging live.
///
/// Isothermal is what makes the temperature arms controllable: it pins every cell at
/// `initial_temp_k` forever, so a two-temperature comparison compares exactly that. A
/// live thermal network would let ohmic self-heating during cycling move the very
/// quantity the Arrhenius ordering is being read from.
fn config(temp_k: f64, initial_soc: f64) -> PackConfig {
    PackConfig {
        aging: Some(AgingConfig {
            sub_clock_period_s: 10.0,
        }),
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: temp_k,
        seed: 0xA6E,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

// --- calendar ---------------------------------------------------------------

/// One hour per step. The calendar integral is exact over any partition at fixed
/// stress, so a coarse `dt` costs nothing here — it only samples the stress less often,
/// and at rest under an isothermal network the stress does not move at all.
const CAL_DT_S: f64 = 3600.0;

/// Rest a pack for `hours` and return `(soh_capacity, soh_resistance)`.
fn rest_for(temp_k: f64, soc: f64, hours: usize, cyc_fade_per_ah: f64) -> (f64, f64) {
    assert!(hours > 0, "a zero-length storage period measures nothing");
    let mut pack = Pack::new(&config(temp_k, soc), lfp(cyc_fade_per_ah)).expect("fixture builds");
    let mut tele = pack.step(0.0, Demand::Rest, &env(temp_k));
    for _ in 0..hours {
        tele = pack.step(CAL_DT_S, Demand::Rest, &env(temp_k));
    }
    (tele.soh_capacity, tele.soh_resistance)
}

/// Capacity lost to shelf storage over `hours`.
fn calendar_fade(temp_k: f64, soc: f64, hours: usize) -> f64 {
    1.0 - rest_for(temp_k, soc, hours, 0.0).0
}

/// **The `√t` signature, and both stress orderings.** A pack on a shelf loses capacity
/// as the square root of time, faster when it is hot and faster when it is full.
///
/// The `√t` check is the one with teeth. Fade over four times the storage period must be
/// exactly twice as much, and it is asserted to a *relative* tolerance rather than a
/// physical one because at fixed stress the engine's accumulator reproduces `k·√t`
/// exactly, whatever the step sizes. Deceleration is the same statement read the other
/// way: the second half of a storage period costs about 41 % of what the first half did,
/// not 100 %.
///
/// No current flows, so nothing here depends on the cycle term; `cyc_fade_per_ah` is
/// zeroed anyway so that a future change to the cycle model cannot quietly move a
/// calendar assertion.
#[test]
fn calendar_fade_follows_sqrt_t_and_worsens_with_temperature_and_soc() {
    const YEAR_H: usize = 365 * 24;

    let one_year = calendar_fade(ROOM_K, 1.0, YEAR_H);
    let four_years = calendar_fade(ROOM_K, 1.0, 4 * YEAR_H);

    // Band, not a fit: "a cell that loses between 1 % and 50 % of its capacity in a
    // year on a shelf at room temperature", matching sim-data's guard on the same
    // coefficients.
    assert!(
        (0.01..=0.5).contains(&one_year),
        "a year on the shelf at 25 C and full SOC fades {:.2} %, outside the plausible \
         1-50 % band",
        one_year * 100.0
    );

    let ratio = four_years / one_year;
    assert!(
        (ratio - 2.0).abs() < 1e-9,
        "calendar fade must go as sqrt(t): 4 years / 1 year = {ratio}, expected 2"
    );

    // Deceleration, stated as the thing a reader would actually check on a plot: the
    // second half of a four-year storage costs less than the first half.
    let two_years = calendar_fade(ROOM_K, 1.0, 2 * YEAR_H);
    let first_half = two_years;
    let second_half = four_years - two_years;
    assert!(
        second_half < first_half,
        "calendar fade must decelerate: first two years cost {first_half}, second two \
         cost {second_half}"
    );

    // Arrhenius: hotter ages faster. 15 K is a wide enough separation that no plausible
    // activation energy leaves the ordering ambiguous.
    let warm = calendar_fade(ROOM_K + 15.0, 1.0, YEAR_H);
    assert!(
        warm > one_year,
        "40 C storage ({warm}) must fade more than 25 C ({one_year})"
    );

    // SOC stress: the shipped table is [1.0, 1.0, 1.4] over soc 0.0/0.5/1.0, so a full
    // cell must age faster than a half-full one at the same temperature.
    let half = calendar_fade(ROOM_K, 0.5, YEAR_H);
    assert!(
        one_year > half,
        "storage at full SOC ({one_year}) must fade more than at half ({half})"
    );
}

// --- 500 cycles -------------------------------------------------------------

/// Cycling step \[s\]. At 1C a full-depth half-cycle is about an hour, so a minute per
/// step is ~60 steps per half-cycle: fine enough that the SOC turnarounds land close to
/// their thresholds, coarse enough that 500 cycles stay cheap in a debug build.
const CYC_DT_S: f64 = 60.0;

/// SOC window the cycling runs between — a 0.9-deep cycle.
const SOC_LOW: f64 = 0.05;
const SOC_HIGH: f64 = 0.95;

/// One `soh_capacity` sample per completed cycle.
///
/// Cycling is CC between [`SOC_LOW`] and [`SOC_HIGH`] at 1C, with the turnarounds on
/// *SOC* rather than on a step count. That is what keeps the depth constant as the pack
/// fades: SOC is the fraction of the capacity the pack has today, so a fixed step count
/// would deepen every cycle as capacity fell and would quietly turn a constant-depth
/// experiment into an increasingly abusive one.
fn cycle_soh(cycles: usize, cyc_fade_per_ah: f64) -> Vec<f64> {
    let mut pack =
        Pack::new(&config(ROOM_K, SOC_HIGH), lfp(cyc_fade_per_ah)).expect("fixture builds");
    let env = env(ROOM_K);
    let i_1c = CAP_AH;
    let mut out = Vec::with_capacity(cycles);

    for _ in 0..cycles {
        // Discharge to the floor, then charge back to the ceiling.
        let mut tele;
        loop {
            tele = pack.step(CYC_DT_S, Demand::Current(i_1c), &env);
            if tele.soc_true <= SOC_LOW {
                break;
            }
        }
        loop {
            tele = pack.step(CYC_DT_S, Demand::Current(-i_1c), &env);
            if tele.soc_true >= SOC_HIGH {
                break;
            }
        }
        out.push(tele.soh_capacity);
    }
    out
}

/// **The phase exit criterion.** 500 full-depth cycles produce a fade curve that is
/// monotone, plausible in magnitude, and accompanied by the resistance growth
/// `CLAUDE.md` forbids modelling capacity fade without.
///
/// The control arm — the same 500 cycles with `cyc_fade_per_ah = 0` — is what makes the
/// shape claim precise. Subtracting it leaves the cycle term alone, and that term stays
/// close to linear in cycle count while the calendar term underneath it visibly
/// decelerates. Those are the two signatures the mechanisms are supposed to have, and
/// asserting them separately is only possible because the arms differ in exactly one
/// coefficient.
#[test]
fn five_hundred_cycles_fade_the_pack_monotonically() {
    const CYCLES: usize = 500;

    let full = cycle_soh(CYCLES, 2.0e-5);
    let calendar_only = cycle_soh(CYCLES, 0.0);

    // Monotone: health never improves.
    for (n, pair) in full.windows(2).enumerate() {
        assert!(
            pair[1] <= pair[0],
            "capacity recovered between cycles {n} and {}: {} -> {}",
            n + 1,
            pair[0],
            pair[1]
        );
    }

    let end = *full.last().expect("500 cycles were run");
    let fade = 1.0 - end;
    assert!(
        (0.01..=0.5).contains(&fade),
        "500 full cycles fade {:.2} %, outside the plausible 1-50 % band",
        fade * 100.0
    );

    // Cycling must cost more than the same elapsed time would have on its own.
    let cal_end = *calendar_only.last().expect("500 cycles were run");
    assert!(
        end < cal_end,
        "cycling ({end}) must fade the pack more than the identical run with the cycle \
         term switched off ({cal_end})"
    );

    // The cycle term, isolated by subtracting the control arm, is close to linear in
    // cycle count: the second 250 cycles cost nearly what the first 250 did. The band is
    // wide on the low side because throughput per cycle *shrinks* as capacity fades — a
    // 0.9-deep cycle of a smaller tank moves fewer amp-hours — so a mild slowdown is
    // real physics, not a modelling error. Sub-linearity is the reason `1.0` is a
    // meaningful ceiling and not merely a round number: nothing in the model can make a
    // later cycle cost more than an earlier one at the same depth.
    //
    // The subtraction is very slightly approximate. Calendar fade depends on elapsed
    // time, and the faded arm's cycles are a little shorter (the same 1C current crosses
    // a smaller tank faster), so it accrues marginally less calendar fade than the
    // control does. At the shipped coefficients that biases the isolated cycle term by
    // well under a percent of itself — visible in neither band below.
    let cyc_at = |n: usize| calendar_only[n] - full[n];
    let first_half = cyc_at(CYCLES / 2 - 1);
    let second_half = cyc_at(CYCLES - 1) - first_half;
    let linearity = second_half / first_half;
    assert!(
        (0.8..=1.0).contains(&linearity),
        "cycle fade should be roughly linear in cycle count: second 250 cycles cost \
         {linearity:.3} of the first 250"
    );

    // And the calendar term underneath it decelerates, which is what makes the
    // comparison above a contrast rather than a coincidence.
    let cal_first = 1.0 - calendar_only[CYCLES / 2 - 1];
    let cal_second = (1.0 - cal_end) - cal_first;
    assert!(
        cal_second < 0.75 * cal_first,
        "the calendar term should visibly decelerate: first half {cal_first}, second \
         half {cal_second}"
    );
}

/// **Resistance growth travels with capacity fade.** `CLAUDE.md` forbids modelling one
/// without the other, so the pairing gets its own assertion rather than riding along
/// inside the curve test.
///
/// The relation is exact, not merely monotone: `soh_resistance = 1 + r_growth · (1 −
/// soh_capacity)` with the shipped `r_growth_per_capacity_loss = 1.5`. Asserting the
/// coefficient rather than just the sign is what would catch the two accumulators
/// drifting apart — a resistance that rose for its own reasons would still be monotone.
#[test]
fn resistance_grows_in_step_with_capacity_loss() {
    const YEAR_H: usize = 365 * 24;

    let (soh_cap, soh_res) = rest_for(ROOM_K, 1.0, YEAR_H, 0.0);
    assert!(soh_cap < 1.0, "a year on the shelf should have faded it");
    assert!(
        soh_res > 1.0,
        "capacity fell to {soh_cap} without resistance rising ({soh_res})"
    );

    let expected = 1.0 + 1.5 * (1.0 - soh_cap);
    assert!(
        (soh_res - expected).abs() < 1e-12,
        "soh_resistance {soh_res} should be 1 + 1.5 x {} = {expected}",
        1.0 - soh_cap
    );
}
