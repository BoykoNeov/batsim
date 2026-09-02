//! The bleed switch is a Schmitt trigger, and these are the facts that make it one.
//!
//! # What was wrong
//! `Bms::bleed_conductances` closed each group's bleed switch on a bare
//! `v_group > v_threshold_v`. Closing the switch is itself what pulls the reading back
//! under the threshold — the bleed current flows through the group's own resistance —
//! so the comparator re-opened on the very next sampled frame and the switch alternated
//! at the step rate for as long as the group sat near the threshold.
//!
//! # How these tests tell chatter from physics
//! **By running the same sim time at two different `dt`s.** A limit cycle driven by the
//! sampling rate produces ten times the flips when `dt` shrinks by ten; a real
//! relaxation oscillation — the switch closing, draining the group across the band, and
//! the charger carrying it back — has a period set by charge transfer and produces the
//! *same* count. That ratio is the discriminator, and it is the one assertion here that
//! does not have to be re-tuned when the shipped band changes.
//!
//! It is also why nothing below asserts `flips == 0`. With a band, a group parked under
//! a charger weaker than its own bleed *should* cycle slowly and forever; demanding zero
//! would only be satisfiable by oversizing the band, which costs balance accuracy.

use sim_core::bms::{BalancingConfig, BmsConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, Fault, Pack, PackConfig, Scatter, SensorId, Snapshot,
    ThermalConfig,
};

/// Bleed resistance \[ohms\].
const R_BLEED: f64 = 33.0;
/// Bleed threshold \[V\], a little under the fixture's resting voltage so the switch has
/// somewhere to sit.
const V_BLEED: f64 = 3.44;
/// The shipped default band \[V\]. Spelled rather than read from the config so that a
/// change to the default is visible here as a failing number, not silently absorbed.
const BAND: f64 = 0.010;

/// The bleed's own load line on this fixture \[V\]: `I_bleed · (R0 + Σ R_rc)` at one
/// cell per group, with `I_bleed ≈ 3.44/33 = 0.104 A` and `R0 + R_rc = 0.03 Ω`.
///
/// This is the quantity a band has to clear, and the sweep in
/// `docs/plans/balancing-chatter.md` puts the measured cliff either side of it on four
/// different `(parallel, R_bleed)` combinations.
const LOAD_LINE_V: f64 = 3.1e-3;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Sloped OCV (0.6 V per unit SOC) so a small charge movement is a visible voltage
/// movement, and a flat 0.02 Ω `R0` so the load line is a number this file can state
/// rather than look up.
fn chem() -> ChemistryParams {
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
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "balhyst".into(),
            name: "Balancing hysteresis test cell".into(),
            provenance: "balancing hysteresis test — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: 2.5,
            v_max: 3.65,
            v_min: 2.50,
            max_charge_c: 2.0,
            max_discharge_c: 2.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![2.90, 3.20, 3.50],
            docv_dt_v_per_k: None,
            t_ref_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

fn bms(band: f64) -> BmsConfig {
    BmsConfig {
        balancing: Some(BalancingConfig {
            bleed_r_ohms: R_BLEED,
            v_threshold_v: V_BLEED,
            v_release_band_v: band,
        }),
        protection: None, // isolate balancing; protection has its own hysteresis tests
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: Vec::new(),
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 1.0e9,
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 0.0,
    }
}

/// A pack parked under a charger too weak to out-drive its own bleed.
///
/// This is the fixture with a *standing* fixed point: the bleed draws 0.104 A against a
/// 0.05 A charge, so a closed switch always drains and an open one always fills, and the
/// switch has something to hunt around for as long as the run lasts. It is not a
/// contrived condition — a charge current below the bleed current is the ordinary
/// end-of-CV state, which is exactly when a passive balancer is working.
fn parked(parallel: u16, band: f64) -> Pack {
    let config = PackConfig {
        aging: None,
        series: 1,
        parallel,
        initial_soc: 0.90,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms(band)),
        cell_model: CellModelConfig::Ecm,
    };
    Pack::new(&config, chem()).expect("the parked fixture builds")
}

/// Number of times the bleed switch changed state over `sim_s` seconds at `dt`.
///
/// The observable is `i_balancing_a`, which is exactly "at least one switch is closed";
/// on these 1S fixtures that is one switch, so a change in it *is* a switch flip.
fn flips(pack: &mut Pack, dt: f64, sim_s: f64) -> usize {
    let steps = (sim_s / dt).round() as usize;
    let mut flips = 0;
    let mut prev: Option<bool> = None;
    for _ in 0..steps {
        let closed = pack.step(dt, Demand::Current(-0.05), &env()).i_balancing_a > 0.0;
        if prev.is_some_and(|p| p != closed) {
            flips += 1;
        }
        prev = Some(closed);
    }
    flips
}

/// Six thousand seconds — long enough for several periods of the slow oscillation a
/// banded switch settles into, so "few flips" is a measurement rather than a truncation.
const SIM_S: f64 = 6000.0;

/// A band stops the switch chattering, and the *evidence that it was chatter* is that
/// the flip count tracked the step rate.
///
/// The zero-band arm is the control, and it is doing the same work here as the zero-band
/// arm in `protection_hysteresis.rs`: without it, "the banded arm flips three times" is
/// satisfied by a switch that simply never closes.
#[test]
fn a_release_band_is_what_stops_the_bleed_switch_chattering() {
    let bare_coarse = flips(&mut parked(1, 0.0), 1.0, SIM_S);
    let bare_fine = flips(&mut parked(1, 0.0), 0.1, SIM_S);
    let banded_coarse = flips(&mut parked(1, BAND), 1.0, SIM_S);
    let banded_fine = flips(&mut parked(1, BAND), 0.1, SIM_S);

    // The control: a bare comparator alternates on essentially every step.
    assert!(
        bare_coarse > 5000,
        "the zero-band arm is supposed to be chattering, and it is this arm that makes \
         the banded one below mean anything; got {bare_coarse} flips in 6000 steps"
    );

    // The discriminator. Ten times the samples over the same simulated time gives ten
    // times the flips if and only if the step rate is what drives them.
    let bare_ratio = bare_fine as f64 / bare_coarse as f64;
    assert!(
        bare_ratio > 5.0,
        "a bare comparator's flip count must scale with the sampling rate — that is what \
         makes it chatter rather than physics; got {bare_fine}/{bare_coarse} = {bare_ratio}"
    );

    let banded_ratio = banded_fine as f64 / banded_coarse as f64;
    assert!(
        banded_ratio < 2.0,
        "with a band the flip count must be a property of the sim time, not the step \
         rate; got {banded_fine}/{banded_coarse} = {banded_ratio}"
    );
    assert!(
        banded_coarse < 20,
        "the banded switch should cycle slowly, not chatter: {banded_coarse} flips"
    );
    // Deliberately NOT `== 0`. A group under a charger it can out-bleed *should* cycle:
    // the switch closes, drains the group through the band, opens, and the charger
    // carries it back. That period is set by charge transfer, which is why the ratio
    // above is 1 rather than 10.
    assert!(
        banded_coarse > 0,
        "and it should still be cycling — a band that stops the switch moving at all is \
         oversized, not correct"
    );
}

/// The quantity the band has to clear is the **bleed's own load line**, and the proof is
/// that the cliff moves with the parallel count on one unchanged chemistry.
///
/// This is what separates the rule here from
/// `ProtectionConfig::v_release_band_v`'s. There the band must clear `v_max − OCV(1.0)`,
/// a property of the chemistry file alone, because tripping removes the external load
/// entirely and the reading falls back to a voltage the cell cannot exceed. A bleed
/// switch has no such pin: opening it returns the reading to wherever the group sits, so
/// what must be cleared is the drop the switch itself causes — `I_bleed · R_group` — and
/// `R_group` scales as `1/parallel`.
///
/// So a 2 mV band, which is *under* the 3.1 mV load line of a 1P group and *over* the
/// 0.78 mV of a 4P one, must fail on the first and work on the second. No
/// chemistry-only rule can produce that split, which is why this test is the one that
/// would fail if the sizing argument were rewritten into protection's.
#[test]
fn the_band_that_matters_is_the_bleeds_own_load_line() {
    let narrow = LOAD_LINE_V * 0.65; // 2 mV: under the 1P load line, over the 4P one
    assert!(
        narrow < LOAD_LINE_V,
        "this test needs a band below the 1P load line to be saying anything"
    );

    let one_p = flips(&mut parked(1, narrow), 1.0, SIM_S);
    let one_p_fine = flips(&mut parked(1, narrow), 0.1, SIM_S);
    let four_p = flips(&mut parked(4, narrow), 1.0, SIM_S);
    let four_p_fine = flips(&mut parked(4, narrow), 0.1, SIM_S);

    assert!(
        one_p_fine as f64 / (one_p as f64) > 5.0,
        "a band under the 1P load line must leave the chatter in place: \
         {one_p_fine}/{one_p}"
    );
    assert!(
        four_p_fine as f64 / (four_p as f64) < 2.0,
        "the same band clears the 4P load line, which is a quarter as large, so the same \
         number of millivolts must be enough there: {four_p_fine}/{four_p}"
    );
}

/// At a band of zero the Schmitt trigger *is* the bare comparator, exactly.
///
/// `rung(held, trip, release)` is `trip || (held && !release)`; at a zero band `release`
/// is `v <= threshold`, whose negation is `trip`, so the whole expression collapses to
/// `trip` and the latch can never disagree with a fresh comparison. Asserted step by
/// step rather than argued, because it is what makes a band of `0.0` a usable control in
/// every other test here and a usable teaching knob in a scenario.
#[test]
fn a_zero_band_is_the_bare_comparator_step_for_step() {
    let mut pack = parked(1, 0.0);
    let mut saw_closed = false;
    let mut saw_open = false;
    for _ in 0..2000 {
        let closed = pack.step(1.0, Demand::Current(-0.05), &env()).i_balancing_a > 0.0;
        // The frame the *next* step will decide on, which at a zero band must already
        // agree with the switch this step carried... one step later. Compare against the
        // switch state the same frame produced by stepping again below.
        let v = pack.bms().expect("bms").sensors().v_group[0];
        let next = pack.step(0.0, Demand::Rest, &env()).i_balancing_a > 0.0;
        assert_eq!(
            next,
            v > V_BLEED,
            "at a zero band the latch must equal a fresh comparison of the frame it was \
             computed from: v = {v}, threshold = {V_BLEED}"
        );
        saw_closed |= closed;
        saw_open |= !closed;
    }
    assert!(
        saw_closed && saw_open,
        "the fixture must actually exercise both switch states for this to mean anything"
    );
}

/// A zero-length step samples no frame, so it moves no switch — and it reports the bleed
/// the next real step will carry.
///
/// This is the property that kept the switch decision out of `bleed_conductances`. Had
/// the decision been made there, a probe step would have advanced the latch, and the
/// pack's nonlinear iteration would have advanced it once per solve pass.
#[test]
fn a_probe_step_moves_no_switch() {
    let mut pack = parked(1, BAND);
    for _ in 0..50 {
        pack.step(1.0, Demand::Current(-0.05), &env());
    }
    let before = pack.snapshot();
    let probe_a = pack.step(0.0, Demand::Rest, &env());
    let probe_b = pack.step(0.0, Demand::Rest, &env());
    assert_eq!(
        probe_a.i_balancing_a, probe_b.i_balancing_a,
        "two probe steps in a row must report the same bleed"
    );
    let after = pack.snapshot();
    assert_eq!(
        bincode::serialize(&before).expect("serializes"),
        bincode::serialize(&after).expect("serializes"),
        "a zero-length step must leave the engine byte-identical, latches included"
    );
}

/// A switch held closed *only by its band* survives a snapshot round trip.
///
/// The state that matters is a switch whose group has already fallen below
/// `v_threshold_v` but not yet below `v_threshold_v − band`: a fresh comparator would
/// open it, and only the latch keeps it closed. A pack that forgot that bit on restore
/// would re-derive the switch from a bare comparison — the chatter this file exists to
/// stop.
///
/// **This round trip goes through `bincode` on purpose.** `Snapshot` holds its `Pack` by
/// value, so `Pack::restore(&pack.snapshot())` is a `Clone` and exercises no `serde`
/// attribute at all — the trap that let an equivalent test in
/// `protection_hysteresis.rs` pass with its field marked `#[serde(skip)]`. A test whose
/// subject is serialization must serialize.
#[test]
fn a_held_bleed_switch_survives_a_snapshot_round_trip() {
    let mut pack = parked(1, BAND);
    // Run until the switch is closed while its group reads *below* the threshold. That
    // is the state a bare comparator cannot represent.
    let mut found = false;
    for _ in 0..2000 {
        let closed = pack.step(1.0, Demand::Current(-0.05), &env()).i_balancing_a > 0.0;
        let v = pack.bms().expect("bms").sensors().v_group[0];
        if closed && v <= V_BLEED {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "the fixture never reached a switch held closed below its threshold, so this \
         test would pass without testing anything"
    );

    let bytes = bincode::serialize(&pack.snapshot()).expect("the snapshot serializes");
    let restored: Snapshot = bincode::deserialize(&bytes).expect("and deserializes");
    let mut restored = Pack::restore(&restored).expect("and restores");

    for i in 0..200 {
        let a = pack.step(1.0, Demand::Current(-0.05), &env());
        let b = restored.step(1.0, Demand::Current(-0.05), &env());
        assert_eq!(
            a, b,
            "step {i} after the round trip diverged, which means the held switch did not \
             survive it"
        );
    }
}

/// A pack built above the threshold bleeds on its **first** step, not its second.
///
/// The latch is only recomputed when a frame is sampled, so a `Bms` that started with
/// every switch open would delay the first bleed by one step — and that one step is the
/// entire difference between a band of zero and the bare comparator this replaced.
/// `Bms::new` therefore seeds the latch from the initial open-circuit frame.
///
/// Written because the perturbation table says nothing else states it: dropping the seed
/// leaves every other test in this file green and is caught only incidentally, by
/// `properties.rs`'s energy balance. A one-step difference in a switch is exactly the
/// kind of thing that hides behind a tolerance.
#[test]
fn a_pack_built_above_the_threshold_bleeds_on_its_first_step() {
    // Not [`parked`]: that fixture is built at `soc = 0.90`, where this OCV table reads
    // `3.20 + 0.4·0.6` = **exactly** `V_BLEED`. The comparator is a strict `>`, so a
    // parked pack rests precisely *on* its threshold and only the charger lifts it over
    // — fine for measuring chatter, useless for asking what the initial latch is. This
    // one starts at 0.95, i.e. a rested 3.47 V.
    let config = PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc: 0.95,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms(BAND)),
        cell_model: CellModelConfig::Ecm,
    };
    let mut pack = Pack::new(&config, chem()).expect("builds");
    let first = pack.step(1.0, Demand::Rest, &env());
    assert!(
        first.i_balancing_a > 0.0,
        "the pack rests at OCV(0.95) = 3.47 V, above the {V_BLEED} V threshold, so the \
         very first step must already carry a bleed; got {} A",
        first.i_balancing_a
    );
}

/// The balancer switches on what its sensors say, **including what a fault made them
/// say** — it never consults the truth.
///
/// A group resting comfortably below the bleed threshold, with a positive offset on its
/// voltage sensor big enough to carry the *reading* over, must bleed. That is the whole
/// of principle 8 applied to balancing: the BMS has one voltage per group and no way to
/// know it is lying, so a miscalibrated sensor makes a balancer throw away charge from a
/// group that never needed it.
///
/// Mechanically this pins the **order** of `corrupt_sensors` and `update_bleed_latches`
/// in `Pack::step`. Swapping the two leaves every other test in this file, and in the
/// suite, green — measured, not assumed.
#[test]
fn a_lying_voltage_sensor_drives_the_balancer() {
    let config = PackConfig {
        aging: None,
        series: 2,
        parallel: 1,
        // Rested 3.38 V, a clear 60 mV under the threshold: without the fault nothing
        // here would ever bleed, which is what the control below checks.
        initial_soc: 0.80,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms(BAND)),
        cell_model: CellModelConfig::Ecm,
    };
    let mut honest = Pack::new(&config, chem()).expect("builds");
    let mut lied_to = Pack::new(&config, chem()).expect("builds");
    lied_to
        .schedule_fault(
            10.0,
            Fault::SensorOffset {
                sensor: SensorId::GroupVoltage(0),
                offset: 0.12,
            },
        )
        .expect("the fault schedules");

    let mut honest_bled = false;
    let mut lied_to_bled = false;
    for _ in 0..100 {
        honest_bled |= honest.step(1.0, Demand::Rest, &env()).i_balancing_a > 0.0;
        lied_to_bled |= lied_to.step(1.0, Demand::Rest, &env()).i_balancing_a > 0.0;
    }

    assert!(
        !honest_bled,
        "the control pack rests 60 mV below the threshold and must never bleed — \
         without that this test would pass on a pack that bleeds for its own reasons"
    );
    assert!(
        lied_to_bled,
        "a +120 mV offset carries the *reading* over the threshold, and the balancer \
         only has readings"
    );
}

/// A negative or non-finite band is refused at construction.
///
/// A negative band inverts the Schmitt trigger — release would sit *above* trip, so a
/// switch could be simultaneously tripped and released — and a `NaN` one makes every
/// comparison false, which silently welds the switch shut. Both are `Pack::new`'s job,
/// because `step` is not allowed to fail for a configuration reason.
///
/// Zero is explicitly allowed: it is the bare comparator, and the control every other
/// test in this file leans on.
///
/// Narrow note: this pins the arm added with this field. The rest of `validate_bms` has
/// no such test — a pre-existing gap this slice did not widen and did not close.
#[test]
fn a_negative_or_non_finite_band_is_refused() {
    let base = PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc: 0.90,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms(BAND)),
        cell_model: CellModelConfig::Ecm,
    };
    for bad in [-0.001, f64::NAN, f64::INFINITY] {
        let mut config = base.clone();
        config.bms = Some(bms(bad));
        assert!(
            Pack::new(&config, chem()).is_err(),
            "a band of {bad} must be refused at construction"
        );
    }
    let mut zero = base.clone();
    zero.bms = Some(bms(0.0));
    assert!(
        Pack::new(&zero, chem()).is_ok(),
        "zero is the bare comparator and must stay buildable"
    );
}

/// Balancing that never switches on is untouched by the band, and a pack with the
/// balancer disabled entirely is untouched by any of this.
///
/// The narrow claim: the latch vector is empty when `balancing` is `None`, so
/// `bleed_conductances` still yields nothing and the group solve sees no extra
/// conductance. Cheap, and it is the configuration every non-BMS test in the suite runs
/// under.
#[test]
fn a_disabled_balancer_carries_no_latch() {
    let mut config = PackConfig {
        aging: None,
        series: 2,
        parallel: 1,
        initial_soc: 0.95, // above any plausible threshold, so "off" is the only reason
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(BmsConfig {
            balancing: None,
            ..bms(BAND)
        }),
        cell_model: CellModelConfig::Ecm,
    };
    let mut off = Pack::new(&config, chem()).expect("builds");
    config.bms = None;
    let mut none = Pack::new(&config, chem()).expect("builds");

    for _ in 0..200 {
        let a = off.step(1.0, Demand::Rest, &env());
        let b = none.step(1.0, Demand::Rest, &env());
        assert_eq!(a.i_balancing_a, 0.0);
        assert_eq!(a.q_balancing_w, 0.0);
        assert_eq!(
            a.v_terminal, b.v_terminal,
            "a BMS with no balancer must load the pack exactly as no BMS does"
        );
    }
}
