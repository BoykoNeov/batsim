//! Phase 3, slice C: lithium plating — the first emergent failure mode.
//!
//! Plating is not injected. Nothing in [`sim_core::PackConfig`] switches it on: a
//! chemistry that supplies `[safety]` thresholds gets the detection, and the pack
//! discovers the condition from its own state. These tests are therefore split three
//! ways:
//!
//! * **the predicate** — that all three conditions (cold, charging, above the C-rate)
//!   are genuinely required, tested directly on the pure functions;
//! * **the consequences** — fade that would not otherwise happen, and the seeded soft
//!   short, tested through `Pack::step`;
//! * **the RNG contract** — that the short roll draws once per plating cell, in
//!   series-major order, and *not at all* when the probability is zero. That contract
//!   is what keeps a trajectory a function of the seed, and none of it is visible
//!   without a test that goes looking for it.
//!
//! Every plating coefficient in every shipped chemistry is a labelled placeholder, so
//! as with the aging tests nothing here asserts a fitted number. The one quantitative
//! assertion is that the fade a plating run suffers *over* an identical non-plating run
//! is the plated charge times the coefficient — an arithmetic property of the
//! mechanism, not a physical claim about lithium.

use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams,
    ThermalParams,
};
use sim_core::plating::{plating_fade_increment, plating_risk, short_probability};
use sim_core::{
    AgingConfig, BmsConfig, CellModelConfig, Demand, Env, EventFlags, Fault, Pack, PackConfig,
    ProtectionConfig, Scatter, ThermalConfig,
};

const CAP_AH: f64 = 2.5;
/// Well below freezing: cold enough to plate against `t_plating_min_k` below.
const COLD_K: f64 = 263.15;
/// Room temperature: warm enough that nothing plates however hard it is charged.
const WARM_K: f64 = 298.15;
/// 2C on this cell — comfortably above the 0.5C plating threshold.
const HARD_CHARGE_A: f64 = -5.0;
/// 0.4C — below the threshold, so cold charging at this rate is safe.
const GENTLE_CHARGE_A: f64 = -1.0;

fn env(t_ambient: f64) -> Env {
    Env {
        t_ambient,
        t_coolant: None,
    }
}

/// Aging coefficients deliberately far faster than anything shipped, so a handful of
/// simulated hours shows what a decade would. Shape, not magnitude.
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

/// Plating thresholds matching the shipped chemistries (0 °C, 0.5C), with the three
/// cost coefficients left to each test — they are what most tests vary.
fn safety_params(
    plating_fade_per_ah: f64,
    plating_short_hazard_per_ah: f64,
    plating_short_ohms: f64,
) -> SafetyParams {
    SafetyParams {
        t_onset_k: 423.15,
        t_vent_k: 453.15,
        runaway_energy_j: 24.0e3,
        // Zero amplitude: these packs are nowhere near onset, and a plating test has no
        // business being able to catch fire if one of them ever is.
        runaway_power_w_at_onset: 0.0,
        runaway_ea_j_per_mol: 0.0,
        t_plating_min_k: 273.15,
        plating_c_threshold: 0.5,
        plating_fade_per_ah,
        plating_short_hazard_per_ah,
        plating_short_ohms,
    }
}

/// A sloped-OCV, single-RC chemistry whose `R0` grid reaches down to the cold end.
fn chem(aging: Option<AgingParams>, safety: Option<SafetyParams>) -> ChemistryParams {
    ChemistryParams {
        aging,
        safety,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "plating_test".into(),
            name: "Plating test cell".into(),
            provenance: "test fixture — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.0,
            max_charge_c: 3.0,
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
            temp_k: vec![253.15, 298.15],
            ohms: vec![vec![0.05, 0.02], vec![0.05, 0.02]],
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

/// An isothermal pack of `series`S1P at `initial_temp_k`, aging on the given clock.
fn cfg(
    series: u16,
    initial_soc: f64,
    initial_temp_k: f64,
    aging: Option<AgingConfig>,
) -> PackConfig {
    PackConfig {
        aging,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series,
        parallel: 1,
        initial_soc,
        initial_temp_k,
        seed: 11,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

/// Age on every step, so a test's step count is its tick count.
fn every_step() -> Option<AgingConfig> {
    Some(AgingConfig {
        sub_clock_period_s: 0.0,
    })
}

/// Charge `pack` at `current` for `steps` steps of `dt`, returning the last telemetry.
fn charge(pack: &mut Pack, e: &Env, current: f64, dt: f64, steps: usize) -> sim_core::Telemetry {
    let mut tele = pack.step(dt, Demand::Current(current), e);
    for _ in 1..steps {
        tele = pack.step(dt, Demand::Current(current), e);
    }
    tele
}

/// This cell's internal-short leakage conductance \[S\].
fn shunt(pack: &Pack, s: usize, p: usize) -> f64 {
    pack.cell(s, p)
        .expect("cell in range")
        .internal_short_conductance_s
}

// --- the predicate --------------------------------------------------------

/// All three conditions are load-bearing, and each one alone is enough to prevent
/// plating. The fourth case is the sign convention: discharging a frozen cell as hard
/// as you like plates nothing, because plating is lithium failing to *enter* the
/// anode.
#[test]
fn plating_needs_cold_and_charging_and_a_high_c_rate() {
    let s = safety_params(0.0, 0.0, 0.0);
    // Cold, charging at 2C: all three hold.
    assert!(plating_risk(&s, HARD_CHARGE_A, COLD_K, CAP_AH));
    // Warm, same current.
    assert!(!plating_risk(&s, HARD_CHARGE_A, WARM_K, CAP_AH));
    // Cold, but only 0.4C.
    assert!(!plating_risk(&s, GENTLE_CHARGE_A, COLD_K, CAP_AH));
    // Cold and hard, but discharging.
    assert!(!plating_risk(&s, -HARD_CHARGE_A, COLD_K, CAP_AH));
    // At rest nothing plates, however cold.
    assert!(!plating_risk(&s, 0.0, COLD_K, CAP_AH));
    // Exactly at the threshold does not plate — the condition is strictly above it.
    let at_threshold = -s.plating_c_threshold * CAP_AH;
    assert!(!plating_risk(&s, at_threshold, COLD_K, CAP_AH));
}

/// A cell whose capacity has faded reaches the plating C-rate at a *lower* absolute
/// current, because the C-rate is measured against the capacity it has today. This is
/// the one feedback path from aging back into aging, and it is deliberate: worn cells
/// really do fast-charge into trouble that new ones shrug off.
#[test]
fn the_c_rate_is_measured_against_the_capacity_the_cell_has_today() {
    let s = safety_params(0.0, 0.0, 0.0);
    // 0.45C on a healthy cell: below the 0.5C threshold, safe.
    let i = -0.45 * CAP_AH;
    assert!(!plating_risk(&s, i, COLD_K, CAP_AH));
    // The same current on a cell faded 20 % is 0.5625C, which plates.
    assert!(plating_risk(&s, i, COLD_K, CAP_AH * 0.8));
}

/// A NaN anywhere answers "not plating" rather than propagating or panicking. `step`
/// must never panic, and a pack that has already left the physical domain should not
/// be credited with a plating event on top of it.
#[test]
fn a_nan_never_plates() {
    let s = safety_params(0.0, 0.0, 0.0);
    assert!(!plating_risk(&s, f64::NAN, COLD_K, CAP_AH));
    assert!(!plating_risk(&s, HARD_CHARGE_A, f64::NAN, CAP_AH));
    assert!(!plating_risk(&s, HARD_CHARGE_A, COLD_K, f64::NAN));
    // A zero-capacity cell would divide by zero; it reports no plating instead.
    assert!(!plating_risk(&s, HARD_CHARGE_A, COLD_K, 0.0));
}

/// Plating fade is linear in plated charge and takes no depth-of-discharge argument at
/// all — unlike cycle fade, which is weighted by the excursion carrying the throughput.
/// The charge that plated is lost inventory whatever excursion it belonged to.
#[test]
fn plating_fade_is_linear_in_plated_charge() {
    let s = safety_params(1.0e-3, 0.0, 0.0);
    assert_eq!(plating_fade_increment(&s, 0.0), 0.0);
    let one = plating_fade_increment(&s, 1.0);
    assert!((one - 1.0e-3).abs() < 1e-15);
    let three = plating_fade_increment(&s, 3.0);
    assert!((three - 3.0 * one).abs() < 1e-15, "{three} vs 3 x {one}");
}

/// The short hazard is Poisson in plated charge, which is what makes the aging
/// sub-clock period irrelevant to how dangerous cold charging is: rolling once against
/// 2 Ah must be exactly as likely to short as rolling twice against 1 Ah. If this held
/// only approximately, a client could make its pack safer by choosing a coarser clock.
#[test]
fn the_short_hazard_does_not_depend_on_how_the_charge_is_split() {
    let s = safety_params(0.0, 0.3, 50.0);
    let once = short_probability(&s, 2.0);
    let twice = 1.0 - (1.0 - short_probability(&s, 1.0)).powi(2);
    assert!((once - twice).abs() < 1e-12, "{once} vs {twice}");
}

/// Exactly `0.0`, not merely small — the caller reads this as "consume no RNG draw",
/// so an epsilon here would silently start shifting the stream on healthy packs.
#[test]
fn the_short_probability_is_exactly_zero_without_hazard_or_charge() {
    assert_eq!(short_probability(&safety_params(0.0, 0.0, 0.0), 5.0), 0.0);
    assert_eq!(short_probability(&safety_params(0.0, 1.0, 50.0), 0.0), 0.0);
}

// --- the flag -------------------------------------------------------------

/// Cold-charging above the threshold raises `PLATING_RISK`, and the same pack charged
/// warm — or charged gently — does not.
#[test]
fn cold_hard_charging_raises_the_flag() {
    let e = cold_env_pack_flags(COLD_K, HARD_CHARGE_A);
    assert!(e.contains(EventFlags::PLATING_RISK), "cold and hard: {e:?}");

    let warm = cold_env_pack_flags(WARM_K, HARD_CHARGE_A);
    assert!(!warm.contains(EventFlags::PLATING_RISK), "warm: {warm:?}");

    let gentle = cold_env_pack_flags(COLD_K, GENTLE_CHARGE_A);
    assert!(
        !gentle.contains(EventFlags::PLATING_RISK),
        "cold but gentle: {gentle:?}"
    );

    let discharging = cold_env_pack_flags(COLD_K, -HARD_CHARGE_A);
    assert!(
        !discharging.contains(EventFlags::PLATING_RISK),
        "cold discharge: {discharging:?}"
    );
}

/// One step of `current` at `temp_k`, returning the flags raised.
fn cold_env_pack_flags(temp_k: f64, current: f64) -> EventFlags {
    let cfg = cfg(1, 0.5, temp_k, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(0.0, 0.0, 0.0))),
    )
    .expect("pack builds");
    pack.step(1.0, Demand::Current(current), &env(temp_k)).flags
}

/// A chemistry with no `[safety]` section never flags plating, however cold and hard
/// it is charged. Unlike a missing `[aging]` section this is not a build error: nothing
/// in the pack config asked for plating, so there is no request for the silence to
/// contradict — the same treatment the optional entropy-coefficient column gets.
#[test]
fn a_chemistry_without_safety_data_never_flags_plating() {
    let cfg = cfg(1, 0.5, COLD_K, every_step());
    let mut pack = Pack::new(&cfg, chem(Some(aging_params()), None)).expect("pack builds");
    let tele = pack.step(1.0, Demand::Current(HARD_CHARGE_A), &env(COLD_K));
    assert!(!tele.flags.contains(EventFlags::PLATING_RISK));
}

/// The flag is an *observation*, so a zero-length probe step reports it — exactly as
/// such a step already reports `q_gen_w` and `BALANCING`. And because it is only an
/// observation, that step still leaves the engine bit-for-bit as it found it, which is
/// the contract `snapshot.rs::zero_length_step_does_not_mutate_state` pins for
/// everything else.
#[test]
fn a_probe_step_reports_plating_risk_without_mutating_anything() {
    let cfg = cfg(1, 0.5, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(1.0e-2, 1.0, 50.0))),
    )
    .expect("pack builds");
    let e = env(COLD_K);
    // Settle onto a real trajectory first, so the snapshot compared below is not the
    // trivially-fresh one.
    charge(&mut pack, &e, HARD_CHARGE_A, 1.0, 5);

    let before = pack.snapshot();
    let tele = pack.step(0.0, Demand::Current(HARD_CHARGE_A), &e);
    assert!(tele.flags.contains(EventFlags::PLATING_RISK));
    assert_eq!(
        pack.snapshot(),
        before,
        "a probe step must not mutate state"
    );
}

// --- the fade -------------------------------------------------------------

/// The damage plating does is exactly the plated charge times the coefficient, over and
/// above whatever the same trajectory would have cost anyway.
///
/// The two arms are identical in every respect but `plating_fade_per_ah`, which is what
/// makes this an isolation of the mechanism rather than a confounded comparison: a cold
/// arm against a warm one would also differ in calendar stress and in `R0`, both of which
/// move the answer.
#[test]
fn plating_costs_exactly_the_plated_charge_times_the_coefficient() {
    let fade_per_ah = 1.0e-2;
    let dt = 1.0;
    let steps = 600;

    let run = |fade: f64| {
        let cfg = cfg(1, 0.05, COLD_K, every_step());
        let mut pack = Pack::new(
            &cfg,
            chem(Some(aging_params()), Some(safety_params(fade, 0.0, 0.0))),
        )
        .expect("pack builds");
        charge(&mut pack, &env(COLD_K), HARD_CHARGE_A, dt, steps).soh_capacity
    };

    let without = run(0.0);
    let with = run(fade_per_ah);
    assert!(
        with < without,
        "plating must cost something: {with} vs {without}"
    );

    // Under `Demand::Current` on a 1S1P pack the cell carries the demand exactly, so
    // the plated charge is known in closed form.
    #[allow(clippy::cast_precision_loss)]
    let ah_plated = -HARD_CHARGE_A * dt * steps as f64 / 3600.0;
    let expected = fade_per_ah * ah_plated;
    let actual = without - with;
    assert!(
        (actual - expected).abs() / expected < 0.01,
        "plating fade {actual} should be ~{expected} (= {fade_per_ah} x {ah_plated} Ah)"
    );
}

/// A pack with `aging: None` still tells the truth about the risk it is running, but
/// has nowhere to put the damage: health is the literal `1.0` it was built with.
/// Turning aging on is what makes plating bite.
#[test]
fn plating_is_reported_but_free_on_a_pack_that_cannot_age() {
    let cfg = cfg(1, 0.05, COLD_K, None);
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(1.0e-2, 1.0, 50.0))),
    )
    .expect("pack builds");
    let tele = charge(&mut pack, &env(COLD_K), HARD_CHARGE_A, 1.0, 200);
    assert!(tele.flags.contains(EventFlags::PLATING_RISK));
    assert_eq!(tele.soh_capacity, 1.0);
    assert_eq!(tele.soh_resistance, 1.0);
    assert_eq!(shunt(&pack, 0, 0), 0.0, "no aging clock, no short roll");
}

/// Plating fade drives resistance growth like any other capacity loss, at the coupling
/// ratio the chemistry gives. `CLAUDE.md` forbids modelling fade without it — and the
/// v1 simplification worth knowing is that plating gets the *same* ratio as shelf
/// aging, where real plated lithium and its SEI raise impedance disproportionately.
#[test]
fn plating_fade_grows_resistance_like_any_other_loss() {
    let cfg = cfg(1, 0.05, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(1.0e-2, 0.0, 0.0))),
    )
    .expect("pack builds");
    let tele = charge(&mut pack, &env(COLD_K), HARD_CHARGE_A, 1.0, 600);
    assert!(tele.soh_capacity < 1.0);
    assert!(
        tele.soh_resistance > 1.0,
        "capacity fell to {} but resistance stayed at {}",
        tele.soh_capacity,
        tele.soh_resistance
    );
}

/// Repeated cold cycling fades the pack heavily and *decelerates* — it does not spiral
/// into the capacity floor.
///
/// This is the test that pays for measuring the C-rate against present capacity. That
/// choice feeds aging back into aging, so the question "does it run away?" has to be
/// answered rather than argued. It does not, for two reasons the trajectory shows: the
/// C-rate enters as a **threshold**, so aging can switch plating on but never make
/// plating already happening go faster; and a faded cell moves *fewer* amp-hours per
/// cycle, so the damage per cycle shrinks as the pack wears.
#[test]
fn repeated_cold_charging_fades_without_running_away() {
    let cfg = cfg(1, 0.2, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(1.0e-2, 0.0, 0.0))),
    )
    .expect("pack builds");
    let e = env(COLD_K);

    let mut soh = vec![1.0];
    for _ in 0..40 {
        // Charge to 0.9, discharge back to 0.2 — a client-side CC policy, driven off
        // true SOC so a shrinking pack still runs a full excursion.
        let mut tele = pack.step(10.0, Demand::Current(HARD_CHARGE_A), &e);
        for _ in 0..2000 {
            if tele.soc_true >= 0.9 {
                break;
            }
            tele = pack.step(10.0, Demand::Current(HARD_CHARGE_A), &e);
        }
        for _ in 0..2000 {
            if tele.soc_true <= 0.2 {
                break;
            }
            tele = pack.step(10.0, Demand::Current(-HARD_CHARGE_A), &e);
        }
        soh.push(tele.soh_capacity);
    }

    for w in soh.windows(2) {
        assert!(w[1] < w[0], "health must fall monotonically: {soh:?}");
    }
    let final_soh = *soh.last().expect("40 cycles recorded");
    assert!(
        final_soh > 0.2,
        "40 aggressive cold cycles should wear the pack, not annihilate it: {final_soh}"
    );
    // Deceleration: the last five cycles cost less than the first five.
    let early = soh[0] - soh[5];
    let late = soh[soh.len() - 6] - soh[soh.len() - 1];
    assert!(
        late < early,
        "damage per cycle should shrink as capacity does: early {early}, late {late}"
    );
}

// --- the soft short, and the RNG contract ---------------------------------

/// Enough plating eventually leaves a soft internal short behind — the one genuinely
/// stochastic outcome in the engine's physics. It is the same object an injected
/// `SoftInternalShort` creates, so it shows up as a leakage conductance on that cell.
#[test]
fn a_plating_cell_eventually_develops_a_soft_short() {
    let cfg = cfg(1, 0.05, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(0.0, 5.0, 50.0))),
    )
    .expect("pack builds");
    let e = env(COLD_K);
    assert_eq!(shunt(&pack, 0, 0), 0.0, "a fresh cell has no leakage path");

    let tele = charge(&mut pack, &e, HARD_CHARGE_A, 10.0, 60);
    assert!(
        shunt(&pack, 0, 0) > 0.0,
        "60 ticks of hard cold charging should short a cell"
    );
    assert!(
        tele.i_internal_short_a > 0.0,
        "a shorted cell must actually leak: {}",
        tele.i_internal_short_a
    );
}

/// The short roll draws **once per plating cell, in series-major order, and not at all
/// for a cell that is not plating**.
///
/// The construction: a 1S1P pack whose only cell plates, against a 2S1P pack whose
/// *second* cell plates and whose first does not (its capacity factor is raised tenfold,
/// so the series current is a tenth of a C for it). Zero scatter and no BMS means
/// nothing else in either pack touches the RNG, so if the contract holds, the plating
/// cell in each pack consumes the identical stream and the two shunt histories match
/// step for step.
///
/// The probability is tuned near one half so that over twenty ticks the history contains
/// both outcomes. That is what closes the case an outcome-only test would miss: a roll
/// that comes up *high* still has to consume its draw, or the two streams desynchronise
/// the moment they disagree. The short resistance is made enormous so the recorded
/// events stay a pure readout of the RNG rather than feeding back into the physics.
#[test]
fn the_short_roll_draws_once_per_plating_cell_in_series_major_order() {
    let dt = 60.0;
    // p = 1 − exp(−λ·Ah) ≈ 0.5 at λ = 8.317 and the 0.0833 Ah each tick moves.
    let safety = || safety_params(0.0, 8.317, 1.0e9);

    // One cell, plating, one draw per tick.
    let cfg_a = cfg(1, 0.05, COLD_K, every_step());
    let mut a = Pack::new(&cfg_a, chem(Some(aging_params()), Some(safety()))).expect("builds");

    // Two cells in series. Cell 0 gets ten times the capacity, so at the shared series
    // current it sits at 0.2C — below the threshold — and must consume no draw.
    let cfg_b = cfg(2, 0.05, COLD_K, every_step());
    let mut b = Pack::new(&cfg_b, chem(Some(aging_params()), Some(safety()))).expect("builds");
    b.set_cell_factors(0, 0, 10.0, 1.0).expect("cell in range");

    let e = env(COLD_K);
    let (mut hist_a, mut hist_b) = (Vec::new(), Vec::new());
    let (mut prev_a, mut prev_b) = (0.0, 0.0);
    for _ in 0..20 {
        a.step(dt, Demand::Current(HARD_CHARGE_A), &e);
        b.step(dt, Demand::Current(HARD_CHARGE_A), &e);
        let (now_a, now_b) = (shunt(&a, 0, 0), shunt(&b, 1, 0));
        hist_a.push(now_a > prev_a);
        hist_b.push(now_b > prev_b);
        prev_a = now_a;
        prev_b = now_b;
        assert_eq!(
            shunt(&b, 0, 0),
            0.0,
            "a cell that never plates must never short"
        );
    }

    assert_eq!(
        hist_a, hist_b,
        "the plating cell must see the same draws whether or not a non-plating cell precedes it"
    );
    // The test only has teeth if both outcomes actually occurred.
    assert!(
        hist_a.contains(&true),
        "expected some rolls to fire: {hist_a:?}"
    );
    assert!(
        hist_a.contains(&false),
        "expected some rolls not to fire: {hist_a:?}"
    );
}

/// A zero probability consumes **no draw at all**, so merely configuring a chemistry
/// with plating coefficients cannot shift a trajectory that never gets cold.
///
/// Detected through the current sensor: the BMS's noise is the only other RNG consumer,
/// so subtracting the true current and the known offset from each reading recovers the
/// raw draw sequence. A pack that plates with a zero hazard must produce the same
/// sequence as one that never plates at all — and the arm with a live hazard must
/// produce a different one, or this test would pass without being able to see anything.
#[test]
fn a_zero_probability_consumes_no_draw() {
    let offset = 0.05;
    let sigma = 0.02;
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        current_offset_a: offset,
        current_noise_sigma_a: sigma,
        temp_probes: vec![],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.1,
        rest_time_for_ocv_s: 1.0e9,
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 1.0e9,
    };

    let noise_stream = |temp_k: f64, hazard: f64| {
        let mut cfg = cfg(1, 0.05, temp_k, every_step());
        cfg.bms = Some(bms.clone());
        let mut pack = Pack::new(
            &cfg,
            chem(Some(aging_params()), Some(safety_params(0.0, hazard, 50.0))),
        )
        .expect("pack builds");
        let e = env(temp_k);
        let mut out = Vec::new();
        for _ in 0..30 {
            let tele = pack.step(60.0, Demand::Current(HARD_CHARGE_A), &e);
            let measured = pack.bms().expect("bms configured").sensors().i_pack_a;
            out.push(measured - tele.i_actual - offset);
        }
        out
    };

    // Cold with no hazard: plates, fades nothing, rolls nothing.
    let cold_no_hazard = noise_stream(COLD_K, 0.0);
    // Warm: never plates, so nothing could have rolled even in principle.
    let never_plates = noise_stream(WARM_K, 5.0);
    assert_eq!(
        cold_no_hazard, never_plates,
        "a zero-probability roll must not touch the RNG stream"
    );

    // And the same measurement does detect a draw when one happens.
    let cold_with_hazard = noise_stream(COLD_K, 5.0);
    assert_ne!(
        cold_no_hazard, cold_with_hazard,
        "the noise stream must be sensitive enough to see a plating draw at all"
    );
}

/// A plating short is state like any other: snapshotting mid-run and continuing from
/// the restore reproduces the original trajectory exactly, including which cells short
/// afterwards. The RNG's position is part of the snapshot, which is what makes the
/// *future* rolls replay and not merely the past ones.
#[test]
fn a_plating_run_replays_bit_identically_across_a_snapshot() {
    let build = || {
        let cfg = cfg(2, 0.05, COLD_K, every_step());
        Pack::new(
            &cfg,
            chem(
                Some(aging_params()),
                Some(safety_params(1.0e-3, 3.0, 500.0)),
            ),
        )
        .expect("pack builds")
    };
    let e = env(COLD_K);

    let mut straight = build();
    let mut direct = Vec::new();
    for _ in 0..40 {
        direct.push(straight.step(30.0, Demand::Current(HARD_CHARGE_A), &e));
    }

    let mut halved = build();
    for _ in 0..20 {
        halved.step(30.0, Demand::Current(HARD_CHARGE_A), &e);
    }
    let mut resumed = Pack::restore(&halved.snapshot()).expect("same schema version");
    for (i, expected) in direct.iter().enumerate().skip(20) {
        let got = resumed.step(30.0, Demand::Current(HARD_CHARGE_A), &e);
        assert_eq!(
            (
                got.soc_true.to_bits(),
                got.soh_capacity.to_bits(),
                got.i_internal_short_a.to_bits()
            ),
            (
                expected.soc_true.to_bits(),
                expected.soh_capacity.to_bits(),
                expected.i_internal_short_a.to_bits()
            ),
            "step {i} diverged after restore"
        );
    }
    assert!(
        direct.last().expect("40 steps").i_internal_short_a > 0.0,
        "the replay is only meaningful if a short actually formed"
    );
}

/// A plating short and an injected one compose the way two injected shorts do: the
/// conductances add, because they are two leakage paths in parallel across the same
/// cell's terminals. By the time it exists a plating short *is* a
/// `Fault::SoftInternalShort` — the mechanism that created it leaves no trace on the
/// cell — so anything else would be the odd behaviour.
#[test]
fn a_plating_short_composes_with_an_injected_one() {
    let injected_ohms = 200.0;
    let plating_ohms = 50.0;
    let cfg = cfg(1, 0.05, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(
            Some(aging_params()),
            Some(safety_params(0.0, 5.0, plating_ohms)),
        ),
    )
    .expect("pack builds");
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: injected_ohms,
        },
    )
    .expect("valid fault");

    let e = env(COLD_K);
    pack.step(1.0, Demand::Rest, &e);
    assert!(
        (shunt(&pack, 0, 0) - 1.0 / injected_ohms).abs() < 1e-15,
        "only the injected short has fired yet"
    );

    charge(&mut pack, &e, HARD_CHARGE_A, 10.0, 60);
    let total = shunt(&pack, 0, 0);
    assert!(
        total > 1.0 / injected_ohms,
        "plating should have added a second leakage path: {total}"
    );
    // The plating short lands in whole multiples of its own conductance, so the total
    // is the injected path plus an integer number of plated ones.
    let plated = (total - 1.0 / injected_ohms) * plating_ohms;
    assert!(
        (plated - plated.round()).abs() < 1e-9 && plated.round() >= 1.0,
        "the excess conductance should be a whole number of plating shorts, got {plated}"
    );
}

/// `clear_faults` repairs a plating-induced short, because by the time it exists it is
/// indistinguishable from an injected one — the same way a fired `WeakCell` becomes
/// indistinguishable from an unlucky scatter draw. What it cannot undo is the capacity
/// the plating already cost.
#[test]
fn clearing_faults_repairs_a_plating_short_but_not_the_fade() {
    let cfg = cfg(1, 0.05, COLD_K, every_step());
    let mut pack = Pack::new(
        &cfg,
        chem(Some(aging_params()), Some(safety_params(1.0e-2, 5.0, 50.0))),
    )
    .expect("pack builds");
    let tele = charge(&mut pack, &env(COLD_K), HARD_CHARGE_A, 10.0, 60);
    assert!(shunt(&pack, 0, 0) > 0.0);
    let faded = tele.soh_capacity;
    assert!(faded < 1.0);

    assert!(
        pack.clear_faults() > 0,
        "a plating short is a fault to clear"
    );
    assert_eq!(shunt(&pack, 0, 0), 0.0);
    let after = pack.step(0.0, Demand::Rest, &env(COLD_K));
    assert_eq!(after.i_internal_short_a, 0.0);
    assert_eq!(
        after.soh_capacity, faded,
        "repairing the short does not restore capacity"
    );
}

// --- the scenario ---------------------------------------------------------

/// The teaching contrast: protection exists to keep a pack out of the region where
/// plating happens, and switching it off is what makes the region reachable.
///
/// Both shipped chemistries set the BMS's `t_charge_min_k` and the physics'
/// `t_plating_min_k` to the same 0 °C, which is the point — charge inhibit *is* the
/// plating guard. The protected pack still plates for exactly one step, because the BMS
/// acts on a frame one step old; a fine `dt` is used here so that one step is a
/// negligible slice of the run rather than a meaningful one, and the assertion is on
/// the ratio between the arms rather than on either number alone.
#[test]
fn charge_inhibit_keeps_a_protected_pack_out_of_plating() {
    let protected_bms = BmsConfig {
        balancing: None,
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.5,
            t_hard_margin_k: 20.0,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.1,
        rest_time_for_ocv_s: 1.0e9,
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 1.0e9,
    };

    let run = |bms: Option<BmsConfig>| {
        let mut cfg = cfg(1, 0.05, COLD_K, every_step());
        cfg.bms = bms;
        let mut pack = Pack::new(
            &cfg,
            chem(Some(aging_params()), Some(safety_params(1.0e-2, 0.0, 0.0))),
        )
        .expect("pack builds");
        let e = env(COLD_K);
        let mut flags = EventFlags::empty();
        let mut tele = pack.step(1.0, Demand::Current(HARD_CHARGE_A), &e);
        for _ in 0..599 {
            tele = pack.step(1.0, Demand::Current(HARD_CHARGE_A), &e);
            flags |= tele.flags;
        }
        (1.0 - tele.soh_capacity, flags)
    };

    let (unprotected_fade, unprotected_flags) = run(None);
    let (protected_fade, protected_flags) = run(Some(protected_bms));

    assert!(
        unprotected_flags.contains(EventFlags::PLATING_RISK),
        "an unprotected pack charged hard in the cold must plate"
    );
    assert!(
        protected_flags.contains(EventFlags::UT),
        "protection should recognise the cold as a charge inhibit: {protected_flags:?}"
    );
    assert!(
        !protected_flags.contains(EventFlags::PLATING_RISK),
        "after the one-step sampling lag the protected pack must stop plating"
    );
    assert!(
        unprotected_fade > 20.0 * protected_fade,
        "unprotected fade {unprotected_fade} should dwarf protected fade {protected_fade}"
    );
}
