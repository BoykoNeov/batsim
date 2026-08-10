//! Phase 3, slice B: injected faults.
//!
//! Faults are the one sanctioned override of the physics (`CLAUDE.md` forbids
//! scripting an *emergent* failure, and everything here is injection, not script).
//! So the assertions are about two things: that the queue's timing contract holds
//! exactly — interval containment, no firing on a probe step, survival across a
//! snapshot — and that each fault lands as a *term in the equations* rather than as
//! an outcome, which is what makes its side effects fall out for free.
//!
//! The soft internal short gets the most attention because it is the one whose
//! consequences are easy to get half-right: a shorted cell must drain at rest, must
//! lose charge at its **internal** branch current rather than its terminal current,
//! and must heat *itself* with the dissipation. A model that gets the group solve
//! right and the heat wrong looks entirely plausible until a cell fails to warm up.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::faults::{Fault, FaultError, SensorId};
use sim_core::{
    BmsConfig, CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, ProtectionConfig,
    Scatter, Telemetry, ThermalConfig,
};

const CAP_AH: f64 = 2.5;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A sloped-OCV, single-RC chemistry. Nothing here is chemistry-specific.
fn chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "fault_test".into(),
            name: "Fault test cell".into(),
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

/// Constant OCV of [`flat_chem`] \[V\].
const FLAT_V0: f64 = 3.30;

/// The same cell with OCV constant in SOC, so the chemical energy given up over a run
/// is exactly `V0 · (charge that left the cells)` with no path dependence — which is
/// what turns the energy balance into a closed-form check (see the energy-balance
/// property test, which reasons the same way).
fn flat_chem() -> ChemistryParams {
    let mut c = chem();
    c.ocv = OcvTable {
        soc: vec![0.0, 1.0],
        volts: vec![FLAT_V0, FLAT_V0],
        docv_dt_v_per_k: None,
    };
    c
}

fn cfg(series: u16, parallel: u16, initial_soc: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series,
        parallel,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 0xF00D_BEEF,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

/// A BMS that protects but does not balance, with one probe on cell (0,0).
fn protecting_bms() -> BmsConfig {
    BmsConfig {
        balancing: None,
        protection: Some(ProtectionConfig {
            v_hard_margin_v: 0.2,
            t_hard_margin_k: 10.0,
            v_release_band_v: 0.08,
            t_release_band_k: 2.0,
        }),
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: vec![(0, 0)],
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.05,
        rest_time_for_ocv_s: 1.0e9,
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 0.0,
    }
}

/// Every `f64` a step reports, as raw bits — for the replay comparisons, where "equal
/// to a tolerance" would not be a replay guarantee at all.
fn tele_bits(t: &Telemetry) -> Vec<u64> {
    vec![
        t.v_terminal.to_bits(),
        t.i_actual.to_bits(),
        t.soc_true.to_bits(),
        t.soc_bms.unwrap_or(f64::NAN).to_bits(),
        t.t_min.to_bits(),
        t.t_max.to_bits(),
        t.v_cell_min.to_bits(),
        t.v_cell_max.to_bits(),
        t.q_gen_w.to_bits(),
        t.q_balancing_w.to_bits(),
        t.i_balancing_a.to_bits(),
        t.i_internal_short_a.to_bits(),
        t.i_external_short_a.to_bits(),
        t.flags.bits().into(),
    ]
}

// ---------------------------------------------------------------------------
// The queue's timing contract
// ---------------------------------------------------------------------------

/// A fault fires on the first step whose interval `[t, t+dt)` contains its timestamp
/// — not on the step that *reaches* the timestamp, and not on the one after.
#[test]
fn fault_fires_on_the_step_whose_interval_contains_it() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    pack.schedule_fault(10.5, Fault::ExternalShort { ohms: 1.0 })
        .unwrap();

    // Steps 1..=10 cover [0,1) … [9,10); none of them contains 10.5.
    for step in 1..=10 {
        let tele = pack.step(1.0, Demand::Rest, &env());
        assert_eq!(
            tele.i_actual,
            0.0,
            "step {step} covers [{}, {}) and must not have fired the short",
            step - 1,
            step
        );
        assert_eq!(pack.faults().pending().len(), 1);
    }
    // Step 11 covers [10, 11), which contains 10.5.
    let tele = pack.step(1.0, Demand::Rest, &env());
    assert!(
        tele.i_external_short_a > 0.0,
        "the short should be conducting on the step containing its timestamp"
    );
    assert!(pack.faults().pending().is_empty());
}

/// A zero-length step is an observation, not a tick: a fault due within it must not
/// fire, and nothing may change.
///
/// This is the third member of the family after the BMS sensor path and the aging
/// sub-clock — every one of them reacts to *information* rather than to elapsed time,
/// so none of them gets the `dt` guard for free.
#[test]
fn probe_step_does_not_fire_a_due_fault() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    pack.schedule_fault(0.0, Fault::ExternalShort { ohms: 1.0 })
        .unwrap();

    let before = pack.snapshot();
    let tele = pack.step(0.0, Demand::Rest, &env());
    assert_eq!(tele.i_external_short_a, 0.0);
    assert_eq!(pack.faults().pending().len(), 1, "still queued");
    assert_eq!(pack.snapshot(), before, "a probe step mutated the pack");

    // And it fires the moment time actually advances.
    let tele = pack.step(1.0, Demand::Rest, &env());
    assert!(tele.i_external_short_a > 0.0);
}

/// A fault scheduled into the past fires on the next stepping step rather than being
/// silently dropped. Scheduling into the past is a client error; losing the fault is
/// the worse answer.
#[test]
fn past_dated_fault_fires_on_the_next_step() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    for _ in 0..10 {
        pack.step(1.0, Demand::Rest, &env());
    }
    assert_eq!(pack.sim_time_s(), 10.0);

    pack.schedule_fault(1.0, Fault::ExternalShort { ohms: 1.0 })
        .unwrap();
    let tele = pack.step(1.0, Demand::Rest, &env());
    assert!(
        tele.i_external_short_a > 0.0,
        "a past-dated fault must fire"
    );
}

/// A pending fault is state, so it has to cross a snapshot boundary intact: restoring
/// and continuing must fire it at the same moment on the same trajectory, bit for bit.
///
/// The analogue of the aging sub-clock's mid-period replay test — the hazard is the
/// same one, a queue that is silently derived rather than stored.
#[test]
fn pending_fault_survives_snapshot_and_restore() {
    let mut original = Pack::new(&cfg(1, 2, 0.6), chem()).unwrap();
    original
        .schedule_fault(
            20.0,
            Fault::SoftInternalShort {
                s: 0,
                p: 0,
                ohms: 5.0,
            },
        )
        .unwrap();
    for _ in 0..10 {
        original.step(1.0, Demand::Current(1.0), &env());
    }
    assert_eq!(original.faults().pending().len(), 1, "not yet due");

    let bytes = bincode::serialize(&original.snapshot()).unwrap();
    let snap: sim_core::Snapshot = bincode::deserialize(&bytes).unwrap();
    let mut restored = Pack::restore(&snap).unwrap();

    let mut fired = false;
    for step in 0..20 {
        let a = original.step(1.0, Demand::Current(1.0), &env());
        let b = restored.step(1.0, Demand::Current(1.0), &env());
        assert_eq!(tele_bits(&a), tele_bits(&b), "diverged at step {step}");
        fired |= a.i_internal_short_a > 0.0;
    }
    assert!(fired, "the short should have fired after the restore point");
}

/// Firing a `WeakCell` is exactly the same thing as calling `set_cell_factors` at that
/// moment — the fault is a scheduling wrapper around the existing seam, not a second
/// implementation of it.
#[test]
fn weak_cell_fault_matches_calling_set_cell_factors() {
    let config = cfg(2, 2, 0.7);
    let mut scheduled = Pack::new(&config, chem()).unwrap();
    scheduled
        .schedule_fault(
            5.0,
            Fault::WeakCell {
                s: 1,
                p: 0,
                capacity_factor: 0.6,
                r0_factor: 2.5,
            },
        )
        .unwrap();
    let mut manual = Pack::new(&config, chem()).unwrap();

    for step in 0..20 {
        // The fault fires at the start of the step covering [5, 6), i.e. step index 5.
        if step == 5 {
            manual.set_cell_factors(1, 0, 0.6, 2.5).unwrap();
        }
        let a = scheduled.step(1.0, Demand::Current(2.0), &env());
        let b = manual.step(1.0, Demand::Current(2.0), &env());
        assert_eq!(tele_bits(&a), tele_bits(&b), "diverged at step {step}");
    }
    let view = scheduled.cell(1, 0).unwrap();
    assert_eq!(view.capacity_factor, 0.6);
    assert_eq!(view.r0_factor, 2.5);
}

// ---------------------------------------------------------------------------
// Soft internal short
// ---------------------------------------------------------------------------

/// The point of the fault: a shorted cell drains while the pack sits idle. The pack
/// carries no terminal current at all, and the cell still loses charge.
#[test]
fn soft_short_drains_a_resting_cell() {
    let mut pack = Pack::new(&cfg(1, 1, 0.8), chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 20.0,
        },
    )
    .unwrap();

    let mut last = None;
    for _ in 0..3600 {
        last = Some(pack.step(1.0, Demand::Rest, &env()));
    }
    let tele = last.unwrap();
    assert_eq!(tele.i_actual, 0.0, "the pack is at rest");
    assert!(
        tele.i_internal_short_a > 0.0,
        "the short should be conducting: {}",
        tele.i_internal_short_a
    );
    assert!(
        pack.cell(0, 0).unwrap().soc < 0.79,
        "a shorted cell must self-discharge, soc = {}",
        pack.cell(0, 0).unwrap().soc
    );
}

/// Inside a parallel group a short drains the **whole group**, not just its own cell:
/// matched neighbours share the leakage current equally, because they share the node
/// the leakage path hangs off.
///
/// That is the physics of a group with no interconnect resistance, and it is the
/// modelling limit worth knowing (see [`sim_core::faults`]): real busbar and weld
/// resistance between a cell and its group node would make the shorted cell drain
/// somewhat faster than its neighbours. What is *not* shared is the heat — that lands
/// in the cell containing the leakage path, which is what makes a group-wide drain
/// dangerous in one specific place.
#[test]
fn soft_short_drains_the_whole_parallel_group() {
    let mut config = cfg(1, 2, 0.8);
    config.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 0.0, // isolate the cells thermally to see where heat lands
    };
    let mut pack = Pack::new(&config, chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 5.0,
        },
    )
    .unwrap();
    for _ in 0..3600 {
        pack.step(1.0, Demand::Rest, &env());
    }
    let (shorted, healthy) = (pack.cell(0, 0).unwrap(), pack.cell(0, 1).unwrap());
    assert!(
        healthy.soc < 0.79,
        "the whole group drains: {}",
        healthy.soc
    );
    assert!(
        (shorted.soc - healthy.soc).abs() < 1e-12,
        "matched cells on one node share the leakage equally: {} vs {}",
        shorted.soc,
        healthy.soc
    );
    assert!(
        shorted.temp_k > healthy.temp_k + 0.5,
        "the dissipation is not shared: {} K vs {} K",
        shorted.temp_k,
        healthy.temp_k
    );
}

/// The dissipation lands in the shorted cell's own thermal node — unlike the
/// balancing bleed, whose resistor is outside every cell and heats nothing.
///
/// A shorted cell that does not warm up is the invisible version of this mistake: the
/// group solve looks right, the SOC drops, and only the temperature says otherwise.
#[test]
fn soft_short_heats_the_cell_it_is_in() {
    let mut config = cfg(1, 2, 0.8);
    config.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&config, chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 2.0,
        },
    )
    .unwrap();
    for _ in 0..1800 {
        pack.step(1.0, Demand::Rest, &env());
    }
    let shorted = pack.cell(0, 0).unwrap().temp_k;
    let healthy = pack.cell(0, 1).unwrap().temp_k;
    assert!(
        shorted > healthy,
        "the shorted cell must run hotter: {shorted} K vs {healthy} K"
    );
    assert!(
        shorted > config.initial_temp_k + 0.5,
        "the short should be a real heat source, not a rounding one: {shorted} K"
    );
}

/// Shorts on the same cell compose in parallel — they are two leakage paths, and
/// conductances add.
#[test]
fn shorts_on_one_cell_compose_in_parallel() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 10.0,
        },
    )
    .unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 10.0,
        },
    )
    .unwrap();
    pack.step(1.0, Demand::Rest, &env());
    let g = pack.cell(0, 0).unwrap().internal_short_conductance_s;
    assert!(
        (g - 0.2).abs() < 1e-15,
        "two 10 Ω paths should give 0.2 S, got {g}"
    );
}

/// A dead short (`R_s → 0`) must stay finite and integrable, not produce NaN. The
/// node voltage collapses toward zero, the dissipation migrates out of the shunt term
/// and into the cell's own `I²·R0`, and the cell empties.
#[test]
fn dead_short_stays_finite() {
    let mut config = cfg(1, 1, 0.9);
    config.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&config, chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 1.0e-9,
        },
    )
    .unwrap();

    let mut flags = EventFlags::empty();
    for step in 0..200 {
        let tele = pack.step(0.5, Demand::Rest, &env());
        flags |= tele.flags;
        assert!(
            tele.v_terminal.is_finite()
                && tele.q_gen_w.is_finite()
                && tele.i_internal_short_a.is_finite()
                && tele.soc_true.is_finite(),
            "non-finite telemetry at step {step}: {tele:?}"
        );
    }
    assert!(
        flags.contains(EventFlags::SOC_CLAMPED_LOW),
        "a dead short should empty the cell"
    );
    assert_eq!(pack.cell(0, 0).unwrap().soc, 0.0);
}

/// Energy balance with a soft short: the chemical energy the cells give up equals the
/// electrical energy out of the terminals plus the heat inside.
///
/// The short adds one term to the *chemical* side and none to the loss side, which is
/// the whole accounting claim of the model: charge leaves the cells through the shunt
/// without crossing the terminals, and the energy it carries is dissipated inside the
/// same cells, so it is already in `q_gen_w`. Getting either half wrong shows up here
/// and essentially nowhere else.
#[test]
fn soft_short_closes_the_energy_balance() {
    let mut config = cfg(2, 2, 0.6);
    config.thermal = ThermalConfig::Network {
        k_neighbor_w_per_k: 1.0,
    };
    let mut pack = Pack::new(&config, flat_chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 5.0,
        },
    )
    .unwrap();

    let dt = 0.5;
    let i = 1.5;
    let mut chemical = 0.0;
    let mut electrical = 0.0;
    let mut heat = 0.0;
    // One step to fire the fault before the accounting starts. A probe step will not
    // fire it, so priming `v_start` first would prime it with the *unshorted* terminal
    // voltage and leave one step's worth of residual — which is exactly the size of
    // the error this test is sensitive enough to catch.
    pack.step(dt, Demand::Current(i), &env());
    // Start-of-step terminal voltage, from a zero-length probe step (see the
    // energy-balance property test for why the electrical integral has to lag).
    let mut v_start = pack.step(0.0, Demand::Current(i), &env()).v_terminal;
    for _ in 0..400 {
        let tele = pack.step(dt, Demand::Current(i), &env());
        chemical +=
            FLAT_V0 * (f64::from(config.series) * tele.i_actual + tele.i_internal_short_a) * dt;
        electrical += v_start * tele.i_actual * dt;
        heat += tele.q_gen_w * dt;
        v_start = tele.v_terminal;
    }
    let imbalance = chemical - electrical - heat;
    let tol = 1e-12 * chemical.abs().max(1.0);
    assert!(
        imbalance.abs() < tol,
        "chemical {chemical} J vs electrical {electrical} J + heat {heat} J \
         (imbalance {imbalance} J, tol {tol} J)"
    );
}

// ---------------------------------------------------------------------------
// External short
// ---------------------------------------------------------------------------

/// An external short conducts whatever the terminal voltage gives it, even under
/// `Demand::Rest` — the demand describes the load, and the short is not the load.
///
/// The identity asserted here is the one the whole pack-level transform rests on: the
/// load solves against a *shunted* source `(E', R')`, and the short's current is
/// `V·G` at the terminal voltage that same solve produces. If the two views of the
/// node ever disagreed the symptom would be a mystifying energy-balance residual, not
/// a voltage mismatch — so it is worth pinning directly.
#[test]
fn external_short_conducts_at_the_solved_terminal_voltage() {
    let ohms = 0.5;
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    pack.schedule_fault(0.0, Fault::ExternalShort { ohms })
        .unwrap();
    pack.step(1.0, Demand::Rest, &env());

    for demand in [Demand::Rest, Demand::Current(3.0), Demand::Current(-2.0)] {
        // A probe step reports the terminal voltage at the current state under this
        // demand while mutating nothing, so the step that follows starts from it.
        let v = pack.step(0.0, demand, &env()).v_terminal;
        let tele = pack.step(1.0, demand, &env());
        let i_load = match demand {
            Demand::Current(i) => i,
            _ => 0.0,
        };
        assert!(
            (tele.i_external_short_a - v / ohms).abs() < 1e-9,
            "{demand:?}: short carried {} A, expected V/R = {} A",
            tele.i_external_short_a,
            v / ohms
        );
        assert!(
            (tele.i_actual - (i_load + tele.i_external_short_a)).abs() < 1e-9,
            "{demand:?}: pack current {} should be load {i_load} + short {}",
            tele.i_actual,
            tele.i_external_short_a
        );
    }
}

/// The BMS-off contrast, both halves.
///
/// The short sits downstream of the contactor, so protection *can* save the pack —
/// but only by opening it, because derating the load does nothing to a short. The
/// unprotected pack runs the short until the cell is empty. That contrast is the
/// reason the fault is placed on that side of the contactor.
#[test]
fn protection_survives_an_external_short_by_latching_open() {
    let short = Fault::ExternalShort { ohms: 0.01 };

    // --- with a BMS: the sag trips under-voltage past its hard margin and latches.
    let mut config = cfg(1, 1, 0.5);
    config.bms = Some(protecting_bms());
    let mut protected = Pack::new(&config, chem()).unwrap();
    protected.schedule_fault(0.0, short).unwrap();

    let mut latched_at = None;
    for step in 0..50 {
        let tele = protected.step(0.1, Demand::Rest, &env());
        if protected.bms().unwrap().contactor_open() {
            assert_eq!(tele.i_actual, 0.0, "an open contactor carries no current");
            assert_eq!(
                tele.i_external_short_a, 0.0,
                "the short is downstream of the contactor, so it opens too"
            );
            latched_at.get_or_insert(step);
        }
    }
    let latched_at = latched_at.expect("the sag must actually latch the contactor open");
    assert!(latched_at <= 2, "latched late, at step {latched_at}");
    assert!(
        protected.cell(0, 0).unwrap().soc > 0.45,
        "the pack should have been saved with most of its charge: soc = {}",
        protected.cell(0, 0).unwrap().soc
    );

    // --- without one: nothing interrupts it and the cell empties.
    let mut unprotected = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    unprotected.schedule_fault(0.0, short).unwrap();
    let mut flags = EventFlags::empty();
    for _ in 0..1000 {
        flags |= unprotected.step(0.1, Demand::Rest, &env()).flags;
    }
    assert!(
        flags.contains(EventFlags::SOC_CLAMPED_LOW),
        "an unprotected pack should run the short until it is flat"
    );
    assert_eq!(unprotected.cell(0, 0).unwrap().soc, 0.0);
}

// ---------------------------------------------------------------------------
// Sensor faults
// ---------------------------------------------------------------------------

/// A stuck sensor reads exactly its stuck value — not that value plus this step's
/// noise. The fault is applied after the sensor's own error model, so it is the last
/// word.
#[test]
fn stuck_sensor_reads_exactly_its_value() {
    let mut config = cfg(2, 1, 0.5);
    config.bms = Some(BmsConfig {
        current_noise_sigma_a: 0.5,
        current_offset_a: 0.1,
        ..protecting_bms()
    });
    let mut pack = Pack::new(&config, chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SensorStuck {
            sensor: SensorId::GroupVoltage(1),
            value: 3.21,
        },
    )
    .unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SensorStuck {
            sensor: SensorId::PackCurrent,
            value: 0.0,
        },
    )
    .unwrap();

    for _ in 0..5 {
        pack.step(1.0, Demand::Current(2.0), &env());
        let frame = pack.bms().unwrap().sensors();
        assert_eq!(frame.v_group[1], 3.21, "stuck means stuck");
        assert_eq!(frame.i_pack_a, 0.0, "stuck through offset and noise alike");
        assert!(
            frame.v_group[0] < 3.3,
            "the healthy group's sensor still reads the truth"
        );
    }
}

/// An offset sensor reads the true measurement plus the offset, and — the property
/// that matters for determinism — injecting the fault does **not** shift the RNG
/// stream. The noise draw happens either way, so every other draw in the trajectory
/// stays exactly where it was.
#[test]
fn sensor_offset_rides_on_top_without_shifting_the_rng() {
    let offset = 0.25;
    let mut config = cfg(1, 1, 0.5);
    config.bms = Some(BmsConfig {
        // Protection off: the corrupted reading must not feed back into the physics,
        // or this compares two different trajectories rather than two readings.
        protection: None,
        current_noise_sigma_a: 0.3,
        ..protecting_bms()
    });
    let mut clean = Pack::new(&config, chem()).unwrap();
    let mut faulted = Pack::new(&config, chem()).unwrap();
    faulted
        .schedule_fault(
            0.0,
            Fault::SensorOffset {
                sensor: SensorId::PackCurrent,
                offset,
            },
        )
        .unwrap();

    for step in 0..30 {
        let a = clean.step(1.0, Demand::Current(1.0), &env());
        let b = faulted.step(1.0, Demand::Current(1.0), &env());
        let i_clean = clean.bms().unwrap().sensors().i_pack_a;
        let i_faulted = faulted.bms().unwrap().sensors().i_pack_a;
        assert_eq!(
            (i_clean + offset).to_bits(),
            i_faulted.to_bits(),
            "step {step}: same noise draw plus the offset, exactly"
        );
        assert_eq!(
            a.i_actual.to_bits(),
            b.i_actual.to_bits(),
            "a corrupted reading must not move the pack"
        );
    }
}

// ---------------------------------------------------------------------------
// Repair, and rejection of nonsense
// ---------------------------------------------------------------------------

/// `clear_faults` drops the queue and every effect, per-cell shunts included, and the
/// pack goes back to behaving like a healthy one.
#[test]
fn clear_faults_repairs_the_pack() {
    let mut pack = Pack::new(&cfg(1, 2, 0.8), chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 5.0,
        },
    )
    .unwrap();
    pack.schedule_fault(0.0, Fault::ExternalShort { ohms: 2.0 })
        .unwrap();
    pack.schedule_fault(100.0, Fault::ExternalShort { ohms: 2.0 })
        .unwrap();
    pack.step(1.0, Demand::Rest, &env());
    assert!(pack.cell(0, 0).unwrap().internal_short_conductance_s > 0.0);

    // One shunt + one external short + one still-pending fault.
    assert_eq!(pack.clear_faults(), 3);
    assert!(pack.faults().is_clear());
    assert_eq!(pack.cell(0, 0).unwrap().internal_short_conductance_s, 0.0);

    let tele = pack.step(1.0, Demand::Rest, &env());
    assert_eq!(tele.i_actual, 0.0);
    assert_eq!(tele.i_internal_short_a, 0.0);
    assert_eq!(tele.i_external_short_a, 0.0);
}

#[test]
fn out_of_range_cell_is_rejected() {
    let mut pack = Pack::new(&cfg(2, 2, 0.5), chem()).unwrap();
    let err = pack
        .schedule_fault(
            1.0,
            Fault::SoftInternalShort {
                s: 2,
                p: 0,
                ohms: 5.0,
            },
        )
        .unwrap_err();
    assert_eq!(
        err,
        FaultError::BadCellIndex {
            s: 2,
            p: 0,
            series: 2,
            parallel: 2
        }
    );
    assert!(pack.faults().pending().is_empty(), "nothing was queued");
}

#[test]
fn non_positive_resistance_is_rejected() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    assert_eq!(
        pack.schedule_fault(1.0, Fault::ExternalShort { ohms: 0.0 })
            .unwrap_err(),
        FaultError::BadResistance(0.0)
    );
    assert_eq!(
        pack.schedule_fault(
            1.0,
            Fault::SoftInternalShort {
                s: 0,
                p: 0,
                ohms: -1.0
            }
        )
        .unwrap_err(),
        FaultError::BadResistance(-1.0)
    );
}

#[test]
fn non_finite_parameters_are_rejected() {
    let mut pack = Pack::new(&cfg(1, 1, 0.5), chem()).unwrap();
    assert!(matches!(
        pack.schedule_fault(f64::NAN, Fault::ExternalShort { ohms: 1.0 }),
        Err(FaultError::NotFinite { field: "at_s", .. })
    ));
    assert!(matches!(
        pack.schedule_fault(
            1.0,
            Fault::WeakCell {
                s: 0,
                p: 0,
                capacity_factor: f64::INFINITY,
                r0_factor: 1.0
            }
        ),
        Err(FaultError::NotFinite {
            field: "capacity_factor",
            ..
        })
    ));
}

/// A sensor fault has to name a sensor that exists — and a pack with no BMS has none
/// at all, which is a scenario-authoring error worth a diagnostic rather than a fault
/// that silently does nothing.
#[test]
fn sensor_faults_need_a_sensor_to_target() {
    let mut no_bms = Pack::new(&cfg(2, 1, 0.5), chem()).unwrap();
    assert_eq!(
        no_bms
            .schedule_fault(
                1.0,
                Fault::SensorStuck {
                    sensor: SensorId::PackCurrent,
                    value: 0.0
                }
            )
            .unwrap_err(),
        FaultError::NoSuchSensor {
            sensor: SensorId::PackCurrent
        }
    );

    let mut config = cfg(2, 1, 0.5);
    config.bms = Some(protecting_bms()); // exactly one temperature probe
    let mut with_bms = Pack::new(&config, chem()).unwrap();
    assert_eq!(
        with_bms
            .schedule_fault(
                1.0,
                Fault::SensorStuck {
                    sensor: SensorId::TempProbe(1),
                    value: 300.0
                }
            )
            .unwrap_err(),
        FaultError::NoSuchSensor {
            sensor: SensorId::TempProbe(1)
        }
    );
    // The probe that does exist, and a group index inside the topology, are fine.
    with_bms
        .schedule_fault(
            1.0,
            Fault::SensorStuck {
                sensor: SensorId::TempProbe(0),
                value: 300.0,
            },
        )
        .unwrap();
    with_bms
        .schedule_fault(
            1.0,
            Fault::SensorOffset {
                sensor: SensorId::GroupVoltage(1),
                offset: 0.1,
            },
        )
        .unwrap();
    assert_eq!(with_bms.faults().pending().len(), 2);
}
