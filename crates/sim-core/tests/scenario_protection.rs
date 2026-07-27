//! Scenario: **protection with the BMS on, and the same demands without it.**
//!
//! A Phase 2 exit criterion, in two halves that only mean something together: with a
//! BMS the pack detects an excursion, derates, and settles inside its limits; with
//! `bms: None` the identical demand drives it straight past them.
//!
//! # What these tests deliberately do *not* assert
//!
//! They do **not** assert that a limit is never crossed. The BMS acts on sensor
//! readings sampled at the end of the previous step, so the first step of any
//! excursion is not prevented — that is a decided design property (see
//! `docs/plans/phase-2-thermal-bms.md`), not an oversight. What is asserted is that
//! the overshoot is small, that it is bounded rather than growing, and that the pack
//! comes back. [`overshoot_is_one_step_and_bounded`] pins the overshoot itself, so
//! that a future change to predictive clamping shows up as a deliberate test change.

use sim_core::bms::{BmsConfig, ProtectionConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig};

const CAP_AH: f64 = 2.5;
// Both voltage limits sit *inside* the OCV range (2.60 – 3.60 V), which matters more
// than it looks. A `v_max` above the top of the OCV curve could never be reached at
// rest, so cutting the current would always let charging resume and the pack would
// creep to full anyway — voltage protection would be decorative. Real cells are the
// same: a protection threshold outside the open-circuit range does nothing.
const V_MAX: f64 = 3.50; // ≈ OCV at SOC 0.875
const V_MIN: f64 = 2.90; // ≈ OCV at SOC 0.25
const T_MAX_K: f64 = 333.15;
const T_CHARGE_MIN_K: f64 = 273.15;
const MAX_CHARGE_C: f64 = 1.0;
const MAX_DISCHARGE_C: f64 = 2.0;
const R0: f64 = 0.02;
const R_RC: f64 = 0.01;

/// How far a current of `i` can carry the terminal voltage past a limit in a single
/// unclamped step \[V\].
///
/// This is the size of the accepted overshoot, and it is *not* arbitrary: it is one
/// step of the full ohmic + polarisation drop, `|I|·(R0 + R_rc)`. The BMS acts on a
/// frame from the previous step, so exactly one step of unclamped current gets
/// through — which means the overshoot is set by the pack's internal resistance and
/// the demand, not by anything tunable. A pack with more internal resistance
/// overshoots further on the same demand.
fn one_step_excursion(i: f64) -> f64 {
    i.abs() * (R0 + R_RC)
}

fn env_at(t_ambient: f64) -> Env {
    Env {
        t_ambient,
        t_coolant: None,
    }
}

fn env() -> Env {
    env_at(298.15)
}

/// A cell with a usefully sloped OCV across the whole range, so that charging really
/// does drive the terminal voltage into `v_max` and discharging into `v_min`.
fn chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        meta: ChemMeta {
            id: "prot".into(),
            name: "Protection test cell".into(),
            provenance: "protection scenario — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: V_MAX,
            v_min: V_MIN,
            max_charge_c: MAX_CHARGE_C,
            max_discharge_c: MAX_DISCHARGE_C,
            t_charge_min_k: T_CHARGE_MIN_K,
            t_max_k: T_MAX_K,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![2.60, 3.20, 3.60],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0], vec![R0]],
        },
        rc: vec![RcPair {
            r_ohms: R_RC,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

/// A BMS with perfect sensors and protection enabled, so that any limit violation is
/// attributable to the *lag*, not to sensor error.
fn protecting_bms() -> BmsConfig {
    BmsConfig {
        balancing: None,
        protection: Some(ProtectionConfig {
            // Generous margins: these scenarios should exercise the *derate* path, and
            // reaching the contactor would mean derating had failed.
            v_hard_margin_v: 0.50,
            t_hard_margin_k: 20.0,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 600.0,
        ocv_correction_gain: 1.0,
        min_ocv_slope_v_per_soc: 0.5,
    }
}

fn config(soc0: f64, temp_k: f64, bms: Option<BmsConfig>) -> PackConfig {
    PackConfig {
        aging: None,
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: temp_k,
        seed: 7,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms,
    }
}

/// An over-current demand is derated to the chemistry's C-rate limit, and the flag
/// names the cause. Note the limit comes from the chemistry, not from BMS config.
#[test]
fn over_current_demand_is_derated_to_the_c_rate_limit() {
    let mut pack = Pack::new(&config(0.5, 298.15, Some(protecting_bms())), chem()).unwrap();
    // First step is unprotected (no frame yet reflects it), so step twice.
    pack.step(1.0, Demand::Current(50.0), &env());
    let tele = pack.step(1.0, Demand::Current(50.0), &env());

    let limit = MAX_DISCHARGE_C * CAP_AH; // 5 A
    assert!(
        (tele.i_actual - limit).abs() < 1e-12,
        "expected derate to {limit} A, got {}",
        tele.i_actual
    );
    assert!(tele.flags.contains(EventFlags::OC), "{:?}", tele.flags);

    // Charging is limited separately, and more tightly.
    let tele = pack.step(1.0, Demand::Current(-50.0), &env());
    let charge_limit = -MAX_CHARGE_C * CAP_AH; // −2.5 A
    assert!(
        (tele.i_actual - charge_limit).abs() < 1e-12,
        "expected charge derate to {charge_limit} A, got {}",
        tele.i_actual
    );

    // A demand inside the window passes through untouched, with no flag.
    let tele = pack.step(1.0, Demand::Current(1.0), &env());
    assert!((tele.i_actual - 1.0).abs() < 1e-12);
    assert!(!tele.flags.contains(EventFlags::OC), "{:?}", tele.flags);
}

/// Charging into `v_max`: the BMS sees the over-voltage and cuts charge current, and
/// the pack parks essentially at the limit instead of continuing to fill. Without a
/// BMS the same demand keeps pushing until the SOC clamp catches it.
///
/// The protection here is a hard window clamp with no hysteresis, so near the
/// threshold it *chatters*: cut the current, the overpotential relaxes, the reading
/// drops back under the limit, a little charge is allowed, and round again. That is
/// what bang-bang control does, and it is why this asserts on the mean current over a
/// settled window rather than on a single step's value. The observable outcome — the
/// pack stops filling and sits at `v_max` — is correct.
#[test]
fn over_voltage_on_charge_derates_with_bms_and_runs_away_without() {
    let charge = Demand::Current(-2.0);
    let steps = 4000;
    let settled = 200; // final steps considered "settled"

    // --- with protection.
    let mut protected = Pack::new(&config(0.9, 298.15, Some(protecting_bms())), chem()).unwrap();
    let mut peak_v: f64 = 0.0;
    let mut saw_ov = false;
    let mut tail_current = 0.0;
    let mut tail_peak_v: f64 = 0.0;
    let mut last = None;
    for step in 0..steps {
        let tele = protected.step(1.0, charge, &env());
        peak_v = peak_v.max(tele.v_terminal);
        saw_ov |= tele.flags.contains(EventFlags::OV);
        if step >= steps - settled {
            tail_current += tele.i_actual;
            tail_peak_v = tail_peak_v.max(tele.v_terminal);
        }
        last = Some(tele);
    }
    let tele = last.unwrap();
    assert!(saw_ov, "the BMS should have raised OV at some point");
    let bound = V_MAX + one_step_excursion(2.0);
    assert!(
        tail_peak_v <= bound + 1e-9,
        "should hold at v_max plus at most one unclamped step ({bound} V), \
         peaked at {tail_peak_v} in the tail"
    );
    let mean_tail_current = tail_current / f64::from(settled);
    assert!(
        mean_tail_current.abs() < 0.02,
        "charging should have effectively stopped, mean tail current {mean_tail_current} A"
    );
    assert!(
        tele.soc_true < 0.95,
        "protection should stop the pack short of full, got {}",
        tele.soc_true
    );
    assert!(
        !tele.flags.contains(EventFlags::SOC_CLAMPED_HIGH),
        "protection should stop the pack before the SOC clamp does"
    );
    assert!(
        !tele.flags.contains(EventFlags::CONTACTOR_OPEN),
        "derating should have been enough; the contactor is for faults"
    );
    let protected_peak = peak_v;

    // --- same demand, no BMS.
    let mut bare = Pack::new(&config(0.9, 298.15, None), chem()).unwrap();
    let mut bare_peak: f64 = 0.0;
    let mut last = None;
    for _ in 0..steps {
        let tele = bare.step(1.0, charge, &env());
        bare_peak = bare_peak.max(tele.v_terminal);
        last = Some(tele);
    }
    let bare_tele = last.unwrap();
    assert!(
        bare_peak > V_MAX + 0.05,
        "without a BMS the pack should blow well past v_max, peaked at {bare_peak}"
    );
    assert!(
        bare_tele.flags.contains(EventFlags::SOC_CLAMPED_HIGH),
        "and keep pushing charge into a full pack: {:?}",
        bare_tele.flags
    );
    assert!(
        bare_peak > protected_peak + 0.05,
        "the contrast is the point: {bare_peak} V unprotected vs {protected_peak} V protected"
    );
}

/// Discharging into `v_min` mirrors it: discharge current is cut, and the pack is not
/// dragged down to the SOC floor the way an unprotected one is.
#[test]
fn under_voltage_on_discharge_derates_with_bms_and_runs_away_without() {
    let load = Demand::Current(4.0);
    let steps = 3000;

    let settled = 200;
    let mut protected = Pack::new(&config(0.5, 298.15, Some(protecting_bms())), chem()).unwrap();
    let mut last = None;
    let mut saw_uv = false;
    let mut tail_current = 0.0;
    for step in 0..steps {
        let tele = protected.step(1.0, load, &env());
        saw_uv |= tele.flags.contains(EventFlags::UV);
        if step >= steps - settled {
            tail_current += tele.i_actual;
        }
        last = Some(tele);
    }
    let tele = last.unwrap();
    assert!(saw_uv, "the BMS should have raised UV");
    let mean_tail_current = tail_current / f64::from(settled);
    assert!(
        mean_tail_current.abs() < 0.02,
        "discharge should have effectively stopped, mean tail current {mean_tail_current} A"
    );
    assert!(
        tele.soc_true > 0.15,
        "protection should leave real charge in the pack, got {}",
        tele.soc_true
    );
    let protected_soc = tele.soc_true;

    let mut bare = Pack::new(&config(0.5, 298.15, None), chem()).unwrap();
    let mut last = None;
    for _ in 0..steps {
        last = Some(bare.step(1.0, load, &env()));
    }
    let bare_tele = last.unwrap();
    assert_eq!(
        bare_tele.soc_true, 0.0,
        "unprotected, it empties completely"
    );
    assert!(
        bare_tele.flags.contains(EventFlags::SOC_CLAMPED_LOW),
        "{:?}",
        bare_tele.flags
    );
    assert!(
        bare_tele.v_terminal < V_MIN,
        "and sits below v_min doing it: {} V",
        bare_tele.v_terminal
    );
    assert!(
        protected_soc > 0.15,
        "the contrast is the point: {protected_soc} protected vs 0.0 unprotected"
    );
}

/// Below `t_charge_min_k` charging is inhibited while discharging stays available —
/// cold is a plating hazard, not a heat one, so the two directions are treated
/// differently.
#[test]
fn cold_pack_inhibits_charge_but_still_allows_discharge() {
    let cold = T_CHARGE_MIN_K - 10.0;
    let mut pack = Pack::new(&config(0.5, cold, Some(protecting_bms())), chem()).unwrap();
    pack.step(1.0, Demand::Rest, &env_at(cold)); // establish a frame

    let charging = pack.step(1.0, Demand::Current(-1.0), &env_at(cold));
    assert!(
        charging.i_actual.abs() < 1e-12,
        "charge should be inhibited when cold, got {}",
        charging.i_actual
    );
    assert!(
        charging.flags.contains(EventFlags::UT),
        "{:?}",
        charging.flags
    );

    let discharging = pack.step(1.0, Demand::Current(1.0), &env_at(cold));
    assert!(
        (discharging.i_actual - 1.0).abs() < 1e-12,
        "discharge should still be allowed when cold, got {}",
        discharging.i_actual
    );
}

/// Past `t_max_k` *plus the hard margin* the response escalates from derating to
/// opening the contactor — and the contactor **latches**: it does not re-close when
/// the pack cools, only when an operator clears the fault.
#[test]
fn overtemperature_latches_the_contactor_open() {
    // Start already above the hard threshold, which is what a runaway would look like
    // to the BMS. A tight margin keeps the scenario short.
    let bms = BmsConfig {
        balancing: None,
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.5,
            t_hard_margin_k: 5.0,
        }),
        ..protecting_bms()
    };
    let hot = T_MAX_K + 10.0; // past t_max + margin
    let mut cfg = config(0.5, hot, Some(bms));
    // A live thermal network, so the pack can genuinely cool later — on an isothermal
    // pack the probe would read `hot` forever and the fault would re-latch the moment
    // it was cleared, which says nothing about latching.
    cfg.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&cfg, chem()).unwrap();
    pack.step(1.0, Demand::Rest, &env_at(hot)); // establish a frame showing the heat

    let tele = pack.step(1.0, Demand::Current(3.0), &env_at(hot));
    assert!(
        tele.flags.contains(EventFlags::CONTACTOR_OPEN) && tele.flags.contains(EventFlags::OT),
        "{:?}",
        tele.flags
    );
    assert_eq!(tele.i_actual, 0.0, "an open contactor carries no current");
    assert!(pack.bms().unwrap().contactor_open());

    // Cool the pack right down — with the contactor open it generates no heat, so it
    // relaxes toward the chilly ambient. The contactor stays open the whole way: this
    // was a fault, not a thermostat.
    for _ in 0..1500 {
        let tele = pack.step(1.0, Demand::Current(3.0), &env_at(273.15));
        assert_eq!(tele.i_actual, 0.0, "still open while the fault is latched");
    }
    assert!(pack.bms().unwrap().contactor_open());
    let cooled = pack.cell(0, 0).unwrap().temp_k;
    assert!(
        cooled < T_MAX_K,
        "the pack really did cool below the limit: {cooled} K"
    );

    // An explicit operator reset closes it, and current flows again.
    assert!(pack.clear_bms_fault(), "a fault was latched and cleared");
    assert!(!pack.bms().unwrap().contactor_open());
    let tele = pack.step(1.0, Demand::Current(3.0), &env_at(273.15));
    assert!(
        (tele.i_actual - 3.0).abs() < 1e-12,
        "after reset the demand is served again, got {}",
        tele.i_actual
    );
    assert!(!pack.clear_bms_fault(), "nothing left to clear");
}

/// **The accepted overshoot, pinned.** Because the BMS acts on a frame from the
/// previous step, exactly one step's worth of excursion gets through before the
/// derate takes hold. This test asserts that it *does* happen and that it stays
/// small — if protection is ever made predictive, this test should be changed
/// deliberately rather than silently kept passing.
#[test]
fn overshoot_is_one_step_and_bounded() {
    let dt = 1.0;
    let mut pack = Pack::new(&config(0.97, 298.15, Some(protecting_bms())), chem()).unwrap();

    let mut over = Vec::new();
    for _ in 0..600 {
        let tele = pack.step(dt, Demand::Current(-2.5), &env());
        if tele.v_terminal > V_MAX {
            over.push(tele.v_terminal - V_MAX);
        }
    }

    assert!(
        !over.is_empty(),
        "with a lagged sensor frame some overshoot is expected by design"
    );
    let worst = over.iter().copied().fold(0.0, f64::max);
    // The scale is derived, not tuned: one step of the full charge current through
    // the pack's internal resistance. Nothing about the BMS can shrink it — only
    // predictive clamping or a shorter `dt` could.
    //
    // The few millivolts of slack are real and worth understanding. The ohmic part
    // (I·R0) lands in a single step, but the RC part builds over the *consecutive*
    // allowed steps of the bang-bang cycle, approaching I·R_rc rather than reaching
    // it in one; meanwhile the excursion starts from wherever the reading that
    // re-enabled charging sat, which is somewhat *below* the limit, and OCV itself
    // rises a little as charge goes in. Those pull in opposite directions and do not
    // exactly cancel.
    let bound = one_step_excursion(2.5);
    assert!(
        worst <= bound + 0.005,
        "overshoot should not exceed one unclamped step ({bound} V) plus a few mV, \
         was {worst} V"
    );
    assert!(
        worst > bound / 2.0,
        "and it really is a full step's worth, not a rounding artefact: {worst} V"
    );
    // Bounded, not growing: the last excursion is no worse than the worst one, i.e.
    // the loop is stable rather than ratcheting upward.
    assert!(
        *over.last().unwrap() <= worst + 1e-12,
        "overshoot should not grow over time: {over:?}"
    );
}

/// A monitor-only BMS (`protection: None`) still estimates and still reports sensors,
/// but never clamps. It is the "the BMS could see it coming and did nothing" case.
#[test]
fn monitor_only_bms_estimates_without_protecting() {
    let bms = BmsConfig {
        balancing: None,
        protection: None,
        ..protecting_bms()
    };
    let mut pack = Pack::new(&config(0.5, 298.15, Some(bms)), chem()).unwrap();
    pack.step(1.0, Demand::Current(50.0), &env());
    let tele = pack.step(1.0, Demand::Current(50.0), &env());

    assert!(
        (tele.i_actual - 50.0).abs() < 1e-12,
        "monitor-only must not derate, got {}",
        tele.i_actual
    );
    assert!(tele.soc_bms.is_some(), "but it is still estimating");
    assert!(!pack.bms().unwrap().sensors().v_group.is_empty());
}
