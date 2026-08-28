//! The protection rungs are Schmitt triggers, and the release band is what makes them
//! protection rather than an oscillator.
//!
//! A bare comparator at the top of charge is a two-step limit cycle: one step admits the
//! full derated current and lands the group above `v_max`, the next derates to zero, the
//! load comes off and the reading falls back under, repeat. It was measured at
//! `1365808` and it is measured at the commit before this one; what made it stop being
//! cosmetic is that closing the energy hole raised the price of an admitted step from
//! 1.3 W to 73.6 W. See `docs/plans/protection-chatter.md`.
//!
//! # What each test here is for
//! * The cycle exists at a zero band and is *gone* at the shipped one — the same fixture,
//!   the same demand, one number different, so nothing else can be credited.
//! * The band that matters is `v_max − OCV(1.0)`, not the load-line swing. A band sized
//!   against the swing is measurably useless, and that is asserted rather than left in a
//!   doc comment, because it is the mistake this design nearly shipped.
//! * A held rung survives a snapshot round-trip. It is state; a pack that forgot would
//!   admit exactly the step the band exists to refuse.
//! * The other three rungs (`UV`, `OT`, `UT`) hold too, because a band wired into one
//!   comparator and not the others would pass every test above.

use sim_core::bms::{BmsConfig, ProtectionConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Cell capacity \[Ah\].
const CAP_AH: f64 = 2.5;
/// The protection limit the charge tests drive into \[V\].
const V_MAX: f64 = 3.65;
/// The protection limit the discharge test drives into \[V\].
const V_MIN: f64 = 2.60;
/// `OCV(1.0)` \[V\]. The gap `V_MAX − OCV_FULL` = 50 mV is the quantity a release band
/// has to exceed; it is written as a constant here because two tests are about it.
const OCV_FULL_V: f64 = 3.60;
/// `OCV(0.0)` \[V\]. Mirrors `OCV_FULL_V`: the discharge-side gap is
/// `OCV_EMPTY_V − V_MIN`, also 50 mV.
const OCV_EMPTY_V: f64 = 2.55;
/// Over-temperature limit \[K\].
const T_MAX_K: f64 = 320.15;
/// Charge-inhibit floor \[K\].
const T_CHARGE_MIN_K: f64 = 283.15;

const AMBIENT_K: f64 = 298.15;

fn env_at(t_ambient: f64) -> Env {
    Env {
        t_ambient,
        t_coolant: None,
    }
}

/// A cell whose OCV runs from [`OCV_EMPTY_V`] to [`OCV_FULL_V`], so both voltage limits
/// sit exactly 50 mV outside the rested range and the two directions are symmetric.
fn chem() -> ChemistryParams {
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
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        meta: ChemMeta {
            id: "hysteresis".into(),
            name: "Protection-hysteresis test cell".into(),
            provenance: "engine test fixture — limits placed 50 mV outside the rested \
                         OCV range so the sizing rule is exact, not physical"
                .into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: V_MAX,
            v_min: V_MIN,
            max_charge_c: 2.0,
            max_discharge_c: 2.0,
            t_charge_min_k: T_CHARGE_MIN_K,
            t_max_k: T_MAX_K,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            t_ref_k: None,
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![OCV_EMPTY_V, 3.10, OCV_FULL_V],
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
    }
}

fn bms(v_band: f64, t_band: f64) -> BmsConfig {
    BmsConfig {
        balancing: None,
        protection: Some(ProtectionConfig {
            // Generous, so every test here exercises the derate rung rather than
            // discovering the contactor.
            v_hard_margin_v: 1.0,
            t_hard_margin_k: 40.0,
            v_release_band_v: v_band,
            t_release_band_k: t_band,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 1.0e9, // never correct; this file is not about the estimator
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 0.5,
    }
}

fn pack_with(soc0: f64, temp_k: f64, v_band: f64, t_band: f64, thermal: ThermalConfig) -> Pack {
    Pack::new(
        &PackConfig {
            aging: None,
            series: 1,
            parallel: 1,
            initial_soc: soc0,
            initial_temp_k: temp_k,
            seed: 7,
            scatter: Scatter::default(),
            thermal,
            bms: Some(bms(v_band, t_band)),
            cell_model: CellModelConfig::Ecm,
        },
        chem(),
    )
    .expect("fixture builds")
}

/// The voltage tests want temperature held still, so nothing but the rung under test can
/// move.
fn pack(soc0: f64, temp_k: f64, v_band: f64, t_band: f64) -> Pack {
    pack_with(soc0, temp_k, v_band, t_band, ThermalConfig::Isothermal)
}

/// The temperature tests need the opposite: a cell that actually tracks its ambient.
/// `k_neighbor_w_per_k` is irrelevant at 1S1P (there are no neighbours) and is here only
/// because the variant carries it.
fn pack_thermal(soc0: f64, temp_k: f64, t_band: f64) -> Pack {
    pack_with(
        soc0,
        temp_k,
        0.08,
        t_band,
        ThermalConfig::Network {
            k_neighbor_w_per_k: 0.1,
        },
    )
}

/// How many of `steps` carried current, and the last telemetry.
fn admitted(p: &mut Pack, demand: Demand, steps: usize, t_ambient: f64) -> usize {
    let e = env_at(t_ambient);
    let mut n = 0;
    for _ in 0..steps {
        if p.step(1.0, demand, &e).i_actual.abs() > 1e-12 {
            n += 1;
        }
    }
    n
}

/// **The limit cycle, and its absence.** One number different between the two arms.
///
/// The zero-band arm keeps admitting current for the whole run — that is the chatter,
/// and it is what shipped until this commit. The shipped-band arm admits only the steps
/// it takes to fill the pack and then holds.
///
/// The assertion is on the *count of admitting steps in the tail*, not on a single step:
/// bang-bang control is defined by admitting on some steps and not others, so any
/// single-step assertion is satisfied by whichever phase it happens to land on. That is
/// the same trap that made a 20-step sampling stride read "heat with no source" while
/// this cycle ran underneath it.
#[test]
fn a_release_band_is_what_stops_the_top_of_charge_chattering() {
    let steps = 1200;
    let tail = 400;

    let mut chattering = pack(0.9, AMBIENT_K, 0.0, 0.0);
    admitted(
        &mut chattering,
        Demand::Current(-2.0),
        steps - tail,
        AMBIENT_K,
    );
    let chatter_tail = admitted(&mut chattering, Demand::Current(-2.0), tail, AMBIENT_K);

    let mut damped = pack(0.9, AMBIENT_K, 0.08, 2.0);
    admitted(&mut damped, Demand::Current(-2.0), steps - tail, AMBIENT_K);
    let damped_tail = admitted(&mut damped, Demand::Current(-2.0), tail, AMBIENT_K);

    assert!(
        chatter_tail > tail / 4,
        "the zero-band arm is supposed to be chattering, so the fixture is wrong rather \
         than the fix working: only {chatter_tail} of {tail} tail steps admitted current"
    );
    assert_eq!(
        damped_tail, 0,
        "with a release band past the v_max − OCV(1.0) gap the rung must hold for good; \
         {damped_tail} of {tail} tail steps still admitted current"
    );
}

/// **The band has to clear `v_max − OCV(1.0)`, not the load-line swing.**
///
/// The swing between a loaded and an unloaded reading is `i·(R0 + Σ R_rc)` — 60 mV at
/// this fixture's derated 2 A — and sizing the band against it is the obvious move and
/// the wrong one. What decides is where the pack *rests*: a saturated cell sits at its
/// own `OCV(1.0)`, so unless the band reaches down past that, the reading crosses the
/// release threshold every time the load comes off, however big the band is.
///
/// Here the gap is 50 mV by construction. 40 mV must still chatter and 60 mV must not —
/// which also pins that the threshold is the gap and not the swing, since a 60 mV band
/// exactly equals the swing and *works*, while a 40 mV band is two thirds of the swing
/// and does nothing.
#[test]
fn the_band_that_matters_is_the_gap_to_the_rested_voltage() {
    let gap = V_MAX - OCV_FULL_V;
    assert!(
        (gap - 0.05).abs() < 1e-12,
        "this test's arithmetic is written around a 50 mV gap; the fixture now has {gap}"
    );
    let tail = 300;
    let run = |band: f64| {
        let mut p = pack(0.9, AMBIENT_K, band, 2.0);
        admitted(&mut p, Demand::Current(-2.0), 800, AMBIENT_K);
        admitted(&mut p, Demand::Current(-2.0), tail, AMBIENT_K)
    };
    assert!(
        run(gap - 0.01) > 0,
        "a band inside the gap cannot hold the rung, however close it gets"
    );
    assert_eq!(
        run(gap + 0.01),
        0,
        "a band past the gap holds it, and 10 mV past is enough — the load-line swing \
         here is 60 mV, so if the swing were the deciding quantity this would fail"
    );
}

/// **A held rung is snapshot state.** Restore mid-hold and the pack must keep holding.
///
/// If the latch were not serialized the restored pack would re-evaluate from its frame,
/// find the rested voltage under `v_max`, and admit one full-current step — precisely
/// the step the band exists to refuse, and precisely the kind of difference the replay
/// tests exist to catch.
///
/// # Why this goes through `bincode` rather than `Pack::restore(&p.snapshot())`
/// [`sim_core::Snapshot`] holds a `Pack` **by value**, so the direct pair is a *clone*
/// and no `serde` attribute on any field is exercised by it. Written that way first,
/// this test passed with the latch marked `#[serde(skip)]` — i.e. it asserted a property
/// of `Clone` while claiming one about serialization, and only the perturbation table in
/// `docs/plans/protection-chatter.md` said so. Serializing for real is what makes the
/// claim in the test's name the claim the test checks.
#[test]
fn a_held_rung_survives_a_snapshot_round_trip() {
    let mut p = pack(0.9, AMBIENT_K, 0.08, 2.0);
    // Long enough to fill the pack and settle into the hold.
    let admitted_before = admitted(&mut p, Demand::Current(-2.0), 900, AMBIENT_K);
    assert!(
        admitted_before > 0,
        "the fixture never charged at all, so nothing was ever held"
    );

    let bytes = bincode::serialize(&p.snapshot()).expect("the snapshot serializes");
    let decoded: sim_core::Snapshot =
        bincode::deserialize(&bytes).expect("the snapshot deserializes");
    let mut restored = Pack::restore(&decoded).expect("the snapshot restores");
    let after_live = admitted(&mut p, Demand::Current(-2.0), 50, AMBIENT_K);
    let after_restored = admitted(&mut restored, Demand::Current(-2.0), 50, AMBIENT_K);
    assert_eq!(after_live, 0, "the live pack should still be holding");
    assert_eq!(
        after_restored, after_live,
        "the restored pack admitted {after_restored} steps where the live one admitted \
         {after_live}: the held rung did not survive the round trip"
    );
}

/// **The other three rungs hold too.** A band wired into `OV` alone would pass every
/// test above.
///
/// Each arm drives one rung to its limit and asserts the flag is still raised on a step
/// where the measurement has come back inside the limit but not past the band. `UV`
/// mirrors `OV` across a fixture built to be symmetric; `OT` and `UT` are driven by the
/// ambient rather than by current, so they exercise the temperature band on its own.
#[test]
fn every_soft_rung_carries_its_band() {
    let e_hot = env_at(T_MAX_K + 5.0);
    let e_cold = env_at(T_CHARGE_MIN_K - 5.0);

    // --- UV: discharge into v_min, then rest. The rested cell climbs back above v_min
    // but not past v_min + band, so discharge stays inhibited.
    let mut p = pack(0.1, AMBIENT_K, 0.08, 2.0);
    let held = (0..900)
        .map(|_| p.step(1.0, Demand::Current(2.0), &env_at(AMBIENT_K)))
        .last()
        .expect("at least one step");
    assert!(
        held.flags.contains(EventFlags::UV),
        "the fixture never reached the under-voltage rung: {:?}",
        held.flags
    );
    let rested = p.step(1.0, Demand::Rest, &env_at(AMBIENT_K));
    assert!(
        rested.v_terminal > V_MIN,
        "a rested cell should be back above v_min ({V_MIN} V), it reads {}",
        rested.v_terminal
    );
    assert!(
        rested.flags.contains(EventFlags::UV),
        "and the rung must still be held there — that is the whole band: {:?}",
        rested.flags
    );

    // --- OT: soak above t_max_k, then hold the ambient one degree *inside* the limit
    // and let the cell settle there. A bare comparator releases on the first step under
    // the limit; a banded one never releases at all, because 1 K is inside the 2 K band.
    let mut p = pack_thermal(0.5, T_MAX_K + 5.0, 2.0);
    let hot = p.step(1.0, Demand::Current(1.0), &e_hot);
    assert!(
        hot.flags.contains(EventFlags::OT),
        "the fixture never reached the over-temperature rung: {:?}",
        hot.flags
    );
    let cool_ambient = env_at(T_MAX_K - 1.0);
    let mut cooling = hot;
    for _ in 0..4000 {
        cooling = p.step(1.0, Demand::Rest, &cool_ambient);
    }
    assert!(
        cooling.t_max < T_MAX_K,
        "the pack should have settled inside its limit, it reads {} K",
        cooling.t_max
    );
    assert!(
        cooling.flags.contains(EventFlags::OT),
        "and the rung must still be held after 4000 steps, since 1 K is inside the 2 K \
         band: {:?}",
        cooling.flags
    );

    // --- UT: charge inhibit below t_charge_min_k, held across a one-degree recovery.
    let mut p = pack_thermal(0.5, T_CHARGE_MIN_K - 5.0, 2.0);
    let cold = p.step(1.0, Demand::Current(-1.0), &e_cold);
    assert!(
        cold.flags.contains(EventFlags::UT),
        "the fixture never reached the charge-inhibit rung: {:?}",
        cold.flags
    );
    let warm_ambient = env_at(T_CHARGE_MIN_K + 1.0);
    let mut warming = cold;
    for _ in 0..4000 {
        warming = p.step(1.0, Demand::Rest, &warm_ambient);
    }
    assert!(
        warming.t_min > T_CHARGE_MIN_K,
        "the pack should have settled above the charge floor, it reads {} K",
        warming.t_min
    );
    assert!(
        warming.flags.contains(EventFlags::UT),
        "and charging must still be inhibited, since 1 K is inside the 2 K band: {:?}",
        warming.flags
    );
}
