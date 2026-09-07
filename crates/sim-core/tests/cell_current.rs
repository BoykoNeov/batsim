//! `CellView::current_a` — the parallel split, and what its absence means.
//!
//! The value itself is checked where an *independent* derivation of it already exists:
//! `properties.rs` reconstructs each cell's current from its SOC change and now compares
//! the two, and `topology.rs` does the same for the circulating currents inside a
//! mismatched group at zero pack current. Neither of those can be satisfied by a wrong
//! number, because neither reads the field to compute what it expects.
//!
//! What is left here is the part that is about the *reporting*, not the physics: what a
//! pack that has not stepped says, what a probe step does to the reading, what a restored
//! pack says, and the sum identity on a pack whose cells are leaking. Plus one guard that
//! has been missing since `SourceCache` was written — see
//! [`a_pack_equals_its_own_serde_round_trip`].
//!
//! `docs/plans/per-cell-current.md` is the plan.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::faults::Fault;
use sim_core::{
    CellModelConfig, Demand, Env, Pack, PackConfig, ReversalParams, Scatter, ThermalConfig,
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
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
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
            id: "c".into(),
            name: "Cell-current test cell".into(),
            provenance: "test — not physical".into(),
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
            t_ref_k: None,
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.40, 3.60],
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
    }
}

fn cfg(series: u16, parallel: u16) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series,
        parallel,
        initial_soc: 0.5,
        initial_temp_k: 298.15,
        seed: 0xC0FF_EE01,
        // Scattered, so the split is actually unequal and a bug that reported the
        // group's current divided by `parallel` would be visible.
        scatter: Scatter {
            capacity_sigma: 0.05,
            r0_sigma: 0.05,
        },
        cell_model: CellModelConfig::Ecm,
    }
}

/// Every cell of a pack that has never stepped reports `None` — not `0.0`.
///
/// The distinction is the field's whole reason for being an `Option`: a resting cell
/// genuinely carries `0.0 A`, so a client cannot tell "at rest" from "nothing has
/// happened yet" if the absence is spelled as a number.
#[test]
fn a_fresh_pack_reports_no_current_at_all() {
    let pack = Pack::new(&cfg(2, 3), chem()).unwrap();
    for s in 0..2 {
        for p in 0..3 {
            assert_eq!(
                pack.cell(s, p).unwrap().current_a,
                None,
                "cell {s}S{p}P on an unstepped pack"
            );
        }
    }
}

/// A zero-length probe reports the split **at the demand it was probed with**, exactly as
/// it reports that demand's terminal voltage and pack current.
///
/// # The gate this test was written the other way round for
///
/// The first version asserted the opposite — that a probe leaves the last real step's
/// reading standing, on the grounds that `step` already gates the vent latch, the BMS
/// sensor clock, the aging sub-clock and the fault queue on `dt > 0`. The gate was removed
/// before it shipped, for two reasons argued in full on the `CellCurrents` buffer: that
/// family exists to stop an observation *mutating state*, and this is not state; and the
/// browser page samples a freshly loaded session with exactly `step(0.0, …)`, so a gated
/// field would have been unreadable on precisely the frame the client needs it.
///
/// The probe below asks for a charge while the last real step was a discharge, so the
/// difference is a sign rather than a rounding: nothing can pass this by accident.
#[test]
fn a_probe_step_reports_the_split_it_was_probed_at() {
    let mut pack = Pack::new(&cfg(1, 3), chem()).unwrap();
    pack.step(1.0, Demand::Current(3.0), &env());
    let after_step: Vec<f64> = (0..3)
        .map(|p| pack.cell(0, p).unwrap().current_a.unwrap())
        .collect();
    assert!(
        after_step.iter().all(|i| *i > 0.0),
        "a discharge should leave every cell discharge-positive: {after_step:?}"
    );

    let tele = pack.step(0.0, Demand::Current(-3.0), &env());
    let after_probe: Vec<f64> = (0..3)
        .map(|p| pack.cell(0, p).unwrap().current_a.unwrap())
        .collect();
    assert!(
        after_probe.iter().all(|i| *i < 0.0),
        "the probe named a charge, so every cell should read charge-negative: {after_probe:?}"
    );
    let sum: f64 = after_probe.iter().sum();
    assert!(
        (sum - tele.i_actual).abs() < 1e-9,
        "the probed split must sum to the probed pack current: {sum} vs {}",
        tele.i_actual
    );
    // And it really is a probe: the state it read did not move, so a *second* probe at the
    // same demand comes back to the same numbers bit for bit.
    pack.step(0.0, Demand::Current(-3.0), &env());
    let again: Vec<f64> = (0..3)
        .map(|p| pack.cell(0, p).unwrap().current_a.unwrap())
        .collect();
    assert_eq!(
        after_probe, again,
        "a probe mutates nothing, so re-probing the same demand reproduces the same split"
    );
    // What a probe does **not** reproduce is the reading a *time-advancing* step left,
    // even at that step's own demand — and the first version of this test asserted that it
    // did, bit for bit, and was wrong by a part in a thousand. A real step reports the
    // split at the state it *started* from, because that is the current each cell was
    // advanced with; it then moves the cells. A probe afterwards solves at the moved
    // state. The two are the same physics one step apart, so they agree to the drift of
    // one second at about 1 C and no closer.
    pack.step(0.0, Demand::Current(3.0), &env());
    let reprobed: Vec<f64> = (0..3)
        .map(|p| pack.cell(0, p).unwrap().current_a.unwrap())
        .collect();
    assert_ne!(
        after_step, reprobed,
        "a probe reports the split at the moved state, not the step's start-of-step split"
    );
    for (p, (a, b)) in after_step.iter().zip(&reprobed).enumerate() {
        assert!(
            ((a - b) / a).abs() < 1e-2,
            "cell {p}: the step's split {a} A and the re-probe {b} A should differ by one step of drift, not more"
        );
    }
}

/// A pack rebuilt from **serialized bytes** reports `None` until it steps, and that is
/// the documented price of keeping the field out of the snapshot.
///
/// # "Restored" turned out to be two different things
///
/// The prediction this test was written for said "a restored pack reports `None`", and it
/// was false for half the packs that word covers. `Pack::restore(&pack.snapshot())` never
/// touches serde at all — `snapshot()` clones the pack and `restore` clones it back, so
/// every `#[serde(skip)]` buffer *survives*, and the first version of this test read
/// `Some(0.95…)` where it expected nothing. The distinction is real and worth pinning
/// from both sides, because the two paths belong to different clients: `sim-godot`
/// restores from a `Snapshot` value in process, while `sim-wasm` and `sim-server` restore
/// from JSON that came off a disk or a socket.
///
/// So the contract is not "after a restore" but "after a deserialization", and both arms
/// are asserted below. Neither is a bug: a clone genuinely did take that step, and a
/// deserialized pack genuinely has not.
///
/// The trajectory is unaffected either way — `snapshot_restore_replay_is_bit_identical`
/// is what pins that — because no physics reads this.
#[test]
fn a_deserialized_pack_reports_none_for_exactly_one_step() {
    let mut pack = Pack::new(&cfg(1, 3), chem()).unwrap();
    pack.step(1.0, Demand::Current(3.0), &env());
    assert!(pack.cell(0, 0).unwrap().current_a.is_some());

    // In process: a clone, and the reading comes with it.
    let cloned = Pack::restore(&pack.snapshot()).unwrap();
    for p in 0..3 {
        assert_eq!(
            cloned.cell(0, p).unwrap().current_a,
            pack.cell(0, p).unwrap().current_a,
            "cell {p} after an in-process restore, which is a clone"
        );
    }

    // Through bytes: cold, exactly as `SourceCache` and `StepScratch` come back cold.
    let bytes = bincode::serialize(&pack.snapshot()).expect("serialize");
    let snapshot: sim_core::Snapshot = bincode::deserialize(&bytes).expect("deserialize");
    let mut restored = Pack::restore(&snapshot).unwrap();
    for p in 0..3 {
        assert_eq!(
            restored.cell(0, p).unwrap().current_a,
            None,
            "cell {p} straight after a deserialization"
        );
    }
    restored.step(1.0, Demand::Current(3.0), &env());
    for p in 0..3 {
        assert!(
            restored.cell(0, p).unwrap().current_a.is_some(),
            "cell {p} one step after a deserialization"
        );
    }
}

/// Two packs whose state is equal are equal, whether or not one of them has stepped
/// since it was deserialized.
///
/// **This guard did not exist before the per-cell current slice.** `SourceCache`'s doc has
/// claimed since Phase 6 that a derived `PartialEq` on a `#[serde(skip)]` buffer "would
/// make `snapshot != roundtrip(snapshot)`", and nothing in the tree checked it: the wasm
/// crate's round-trip test compares JSON *strings*, which a skipped field cannot move, and
/// `Pack::restore(&pack.snapshot())` is a `Clone` that exercises no serde attribute at
/// all. So the round trip here goes through actual bytes, and the comparison is between a
/// pack that has run and a pack that has only been read back — which is exactly the pair
/// the three skipped buffers differ on.
#[test]
fn a_pack_equals_its_own_serde_round_trip() {
    let mut pack = Pack::new(&cfg(2, 3), chem()).unwrap();
    for _ in 0..5 {
        pack.step(1.0, Demand::Current(2.0), &env());
    }
    let snapshot = pack.snapshot();
    let bytes = bincode::serialize(&snapshot).expect("serialize");
    let back: sim_core::Snapshot = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(
        snapshot, back,
        "a warm pack must compare equal to its own cold round trip"
    );
}

/// The cells supply the load **and** the leakage, so their currents sum to more than the
/// pack current whenever anything inside is shorted.
///
/// This is the identity `CellView::current_a` documents and `CLAUDE.md`'s one-line
/// summary of the same property does not: `Σ I_k = i_actual + Σ V_node·G_shunt`. The
/// excess is `Telemetry::i_internal_short_a`, which the engine already reports, so the
/// two independent accounts of the same leakage are compared rather than asserted apart.
#[test]
fn a_shorted_cell_carries_more_than_the_terminals_take() {
    let mut pack = Pack::new(&cfg(1, 3), chem()).unwrap();
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 1,
            ohms: 2.0,
        },
    )
    .unwrap();
    let tele = pack.step(1.0, Demand::Current(2.0), &env());
    assert!(
        tele.i_internal_short_a > 0.0,
        "the short should be conducting: {}",
        tele.i_internal_short_a
    );

    let sum: f64 = (0..3)
        .map(|p| pack.cell(0, p).unwrap().current_a.unwrap())
        .sum();
    let expected = tele.i_actual + tele.i_internal_short_a;
    assert!(
        (sum - expected).abs() < 1e-9,
        "Σ I_k = {sum}, expected i_actual {} + short {} = {expected}",
        tele.i_actual,
        tele.i_internal_short_a
    );
    // And the fault-free contrast, on the same pack shape: without a short the sum is
    // the pack current exactly. Without this arm the assertion above would also pass on
    // an engine that reported terminal currents and a short current of zero.
    let mut clean = Pack::new(&cfg(1, 3), chem()).unwrap();
    let tele = clean.step(1.0, Demand::Current(2.0), &env());
    let sum: f64 = (0..3)
        .map(|p| clean.cell(0, p).unwrap().current_a.unwrap())
        .sum();
    assert!(
        (sum - tele.i_actual).abs() < 1e-9,
        "unshorted: Σ I_k = {sum}, i_actual = {}",
        tele.i_actual
    );
}
