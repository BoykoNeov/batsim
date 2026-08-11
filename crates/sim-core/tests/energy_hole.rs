//! The arithmetic of charge that a SOC clamp refuses or invents.
//!
//! The property tests in `properties.rs` prove the two ledgers close through a clamp.
//! They cannot discriminate *how* the terms are formed, because a flat OCV — the thing
//! that makes those ledgers closed-form — collapses every OCV question into one number.
//! These are the discriminating cases, on a sloped chemistry, each written against a
//! named rival implementation that would pass everything else:
//!
//! * the heat uses `OCV(1.0)`, not `OCV(soc_at_the_start_of_the_step)`;
//! * the rejected amount is the *fraction* that did not fit, not the whole step's charge
//!   gated on a boolean — the two agree everywhere except at clamp entry, which is the
//!   one step per clamp where the difference exists;
//! * the bottom of the window adds **no** heat term, which is a deliberate asymmetry and
//!   not an oversight;
//! * a zero-length probe step rejects nothing.
//!
//! The bottom of the window then stopped being a hole at all. Below `soc = 0` the cell
//! now goes into voltage reversal — it carries the charge it delivered as a deficit,
//! drops its open-circuit voltage toward the chemistry's floor, and makes the external
//! circuit pay — so the second half of this file is about that branch, again each test
//! against a named rival that would otherwise pass:
//!
//! * a closed cycle through reversal conserves energy, at *any* `[reversal]` tuning;
//! * arriving at empty is continuous, so an empty cell is not a short circuit;
//! * the floor bounds an otherwise unbounded ramp;
//! * the cell stays linear, so the pack solve stays one closed-form pass;
//! * and none of it is visible to a pack that stays inside its window.
//!
//! See `docs/plans/energy-hole.md` and `docs/plans/low-clamp-reversal.md`.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, Telemetry, ThermalConfig,
};

/// Cell capacity \[Ah\]: 9000 As exactly, so the arithmetic below is checkable by hand.
const CAP_AH: f64 = 2.5;

/// Series resistance \[ohms\], constant over the whole `(soc, temp)` grid so that the
/// ohmic part of the heat is `I²·R0` with no interpolation to reason about.
const R0_OHMS: f64 = 0.02;

/// `OCV(1.0)` \[V\] for [`sloped_chem`].
const OCV_FULL_V: f64 = 3.60;

/// `OCV(0.0)` \[V\] for [`sloped_chem`].
const OCV_EMPTY_V: f64 = 3.00;

/// `OCV(0.99)` \[V\] — the value the rival implementation would use, and the whole
/// reason the OCV table is sloped near the top. 3.40 at 0.8 and 3.60 at 1.0 puts 0.99 at
/// `3.40 + 0.95·0.20`.
const OCV_AT_099_V: f64 = 3.59;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A chemistry whose OCV is sloped at both ends and whose `R0` is flat.
///
/// Nothing here is a physical claim: the numbers are chosen so that every quantity in
/// this file can be written down in closed form and so that `OCV(1.0)` and
/// `OCV(0.99)` differ by a comfortable 10 mV.
fn sloped_chem() -> ChemistryParams {
    ChemistryParams {
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
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
            id: "energy_hole".into(),
            name: "Energy-hole test cell".into(),
            provenance: "engine test fixture — chosen for closed-form arithmetic, not \
                         physical"
                .into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.00,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            docv_dt_v_per_k: None,
            soc: vec![0.0, 0.2, 0.5, 0.8, 1.0],
            volts: vec![3.00, 3.20, 3.30, 3.40, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![R0_OHMS], vec![R0_OHMS]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 2000.0,
        }],
    }
}

/// A 1S1P config at `soc0`, isothermal, no BMS — so the demanded current reaches the
/// cell unmodified and the only heat is the cell's own.
fn pack_config(soc0: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: 298.15,
        seed: 1,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

/// [`pack_config`] built against the unmodified [`sloped_chem`].
fn pack_at(soc0: f64) -> Pack {
    Pack::new(&pack_config(soc0), sloped_chem()).expect("fixture builds")
}

/// **The heat is `OCV(1.0)·i_rejected`, not `OCV(soc_start)·i_rejected`.**
///
/// One step from `soc = 0.99` at 180 A of charge over 1 s: 180 As offered, 90 As fit,
/// 90 As refused. The cell's overpotentials are zero at the start of the first step, so
/// the heat is exactly `I²·R0` plus the rejected-charge term and nothing else.
///
/// The rival — reading the OCV at the SOC the step began from — differs by
/// `(3.60 − 3.59)·90 = 0.9 W` out of 972, which every tolerance in the suite would
/// absorb and which this asserts against directly.
#[test]
fn the_rejected_charge_burns_at_the_windows_endpoint() {
    let mut pack = pack_at(0.99);
    let tele = pack.step(1.0, Demand::Current(-180.0), &env());

    assert!(tele.flags.contains(EventFlags::SOC_CLAMPED_HIGH));
    assert!(
        (tele.i_rejected_a - -90.0).abs() < 1e-9,
        "90 of the 180 As offered should have been refused, got {} A",
        tele.i_rejected_a
    );

    let ohmic_w = 180.0 * 180.0 * R0_OHMS; // 648 W
    let expected_w = ohmic_w + OCV_FULL_V * 90.0; // 648 + 324
    let rival_w = ohmic_w + OCV_AT_099_V * 90.0; // what OCV(soc_start) would give

    assert!(
        (tele.q_gen_w - expected_w).abs() < 1e-6,
        "expected {expected_w} W (I²R0 + OCV(1.0)·90), got {}",
        tele.q_gen_w
    );
    // Not merely "close to the right answer": far from the wrong one, by more than the
    // tolerance above, so the assertion above is doing work.
    assert!(
        (tele.q_gen_w - rival_w).abs() > 0.5,
        "the OCV(soc_start) rival would give {rival_w} W and is not distinguished"
    );
}

/// **Clamp entry rejects the fraction that did not fit; the step after rejects all of
/// it.** The boolean implementation — "clamped, so the whole step's charge was
/// refused" — agrees on the second step and is wrong on the first.
#[test]
fn only_the_fraction_that_did_not_fit_is_rejected() {
    let mut pack = pack_at(0.99);

    let entry = pack.step(1.0, Demand::Current(-180.0), &env());
    assert!(entry.flags.contains(EventFlags::SOC_CLAMPED_HIGH));
    assert!(
        (entry.i_rejected_a - -90.0).abs() < 1e-9,
        "entry step should refuse only the 90 As that did not fit, got {}",
        entry.i_rejected_a
    );
    assert!(
        entry.i_rejected_a.abs() < entry.i_actual.abs(),
        "on the entry step the refused charge ({}) must be strictly less than the \
         charge offered ({})",
        entry.i_rejected_a,
        entry.i_actual
    );

    // Already full: now every coulomb offered is refused, and the boolean version and
    // this one finally agree.
    let saturated = pack.step(1.0, Demand::Current(-180.0), &env());
    assert!(
        (saturated.i_rejected_a - saturated.i_actual).abs() < 1e-9,
        "a full cell should refuse the whole step: rejected {} vs offered {}",
        saturated.i_rejected_a,
        saturated.i_actual
    );
}

/// **The bottom of the window adds no heat, deliberately.**
///
/// The mirror term is a *cooling* one, and a cell that cools itself while being
/// over-drained would feed the thermal network a wrong-signed drive and suppress runaway
/// in the regime where it matters. The fabricated charge is reported instead
/// (`i_rejected_a` is positive here) and this pins the heat at the ohmic value alone, so
/// that adding the symmetric term "for consistency" fails a test rather than quietly
/// changing every hot trajectory in the repo.
#[test]
fn the_bottom_of_the_window_rejects_nothing_and_adds_no_heat() {
    let mut pack = pack_at(0.01);
    let tele = pack.step(1.0, Demand::Current(180.0), &env());

    assert!(tele.flags.contains(EventFlags::SOC_CLAMPED_LOW));
    assert_eq!(
        tele.i_rejected_a, 0.0,
        "the low clamp carries its shortfall as a deficit and rejects nothing; \
         the pre-reversal engine reported 90 A here"
    );

    let ohmic_w = 180.0 * 180.0 * R0_OHMS;
    assert!(
        (tele.q_gen_w - ohmic_w).abs() < 1e-6,
        "the low clamp should generate ohmic heat only ({ohmic_w} W), got {}",
        tele.q_gen_w
    );
    // Two rivals, named so the two assertions above are legible. Both would close the
    // energy ledger; both are refused, and for different reasons.
    let cooling_w = ohmic_w - OCV_EMPTY_V * 90.0;
    let heating_w = ohmic_w + OCV_EMPTY_V * 90.0;
    assert!(
        (tele.q_gen_w - cooling_w).abs() > 0.5 && (tele.q_gen_w - heating_w).abs() > 0.5,
        "neither the cooling rival ({cooling_w} W) nor the heating one ({heating_w} W) \
         is distinguished from {}",
        tele.q_gen_w
    );
}

/// **The energy ledger closes over a closed cycle in state space.**
///
/// Discharge a cell far past empty, charge exactly the same charge back, then rest until
/// the overpotentials have relaxed. The cell is where it started, so the chemical term is
/// zero *by construction* — no state-function integral appears in the assertion, which is
/// the whole reason the cycle is the instrument. Whatever the circuit put in must have
/// come back out as heat: `∫V·I dt + ∫Q dt = 0`.
///
/// This is the discriminating test for the reversal branch, and it discriminates twice.
/// The pre-reversal engine fails it before the energy is even weighed: charge pushed back
/// into a cell that was clamped at `soc = 0` is not repaying anything, so 2400 As out and
/// 2400 As in leaves that engine at `soc = 0.267` rather than back at `0.05`. **A cycle
/// that does not close in state cannot close in energy**, and the ~5.9 kJ it would
/// fabricate is the second failure rather than the first.
///
/// Run at two different `[reversal]` settings, because conservation here is structural
/// rather than parametric: `OCV_eff` is a single-valued function of the cell's extended
/// position, so stored energy is a state function of it whatever the ramp and floor are.
/// A version of this fix that got the ledger to close only for one tuning would pass the
/// first arm and fail the second.
#[test]
fn a_cycle_through_reversal_conserves_energy() {
    /// One full cycle at step length `dt`, returning `(imbalance, heat)` in joules.
    fn cycle(v_per_soc: f64, floor_v: f64, dt: f64) -> (f64, f64) {
        let mut chem = sloped_chem();
        chem.reversal.v_per_soc = v_per_soc;
        chem.reversal.floor_v = floor_v;
        let mut pack = Pack::new(&pack_config(0.05), chem).expect("fixture builds");

        // 450 As stored, 2400 As drawn: 0.2167 of capacity past empty.
        let (mut elec_j, mut heat_j) = (0.0, 0.0);
        let mut weigh = |t: &sim_core::Telemetry, dt: f64| {
            elec_j += t.v_terminal * t.i_actual * dt;
            heat_j += t.q_gen_w * dt;
        };
        let legs = (60.0 / dt).round() as u32;
        for _ in 0..legs {
            weigh(&pack.step(dt, Demand::Current(40.0), &env()), dt);
        }
        for _ in 0..legs {
            weigh(&pack.step(dt, Demand::Current(-40.0), &env()), dt);
        }
        // 20 RC time constants: the remaining overpotential is ~1e-7 V and the energy it
        // stands for is far below every tolerance here.
        for _ in 0..400 {
            weigh(&pack.step(1.0, Demand::Rest, &env()), 1.0);
        }

        let view = pack.cell(0, 0).expect("cell exists");
        assert!(
            (view.soc - 0.05).abs() < 1e-12,
            "the cycle must return the cell to where it started, got soc {} at \
             v_per_soc {v_per_soc}, dt {dt}",
            view.soc
        );
        assert!(
            view.overpotential_v.abs() < 1e-6,
            "the rest is meant to relax the overpotentials, {} V left",
            view.overpotential_v
        );
        (elec_j + heat_j, heat_j)
    }

    for (v_per_soc, floor_v) in [(100.0, 0.0), (12.0, -4.0)] {
        let (coarse, _) = cycle(v_per_soc, floor_v, 0.5);
        let (fine, heat) = cycle(v_per_soc, floor_v, 0.25);

        // The assertion that separates a discretisation residue from fabricated energy,
        // and the reason this test halves the step rather than picking a tolerance. Both
        // terms are first-order quadrature error on a voltage that moves within the step,
        // so halving `dt` halves them; the pre-reversal engine's imbalance does not move
        // with `dt` at all, because it is not error but energy the model made.
        assert!(
            fine.abs() < 0.6 * coarse.abs(),
            "imbalance must shrink with the step: {coarse} J at dt 0.5, {fine} J at \
             dt 0.25, v_per_soc {v_per_soc}"
        );
        assert!(
            fine.abs() < 0.02 * heat.abs(),
            "imbalance {fine} J is not small against the {heat} J of heat it sits \
             beside, at v_per_soc {v_per_soc}, floor {floor_v}"
        );
    }
}

/// **Reaching empty is not a discontinuity.**
///
/// The rival here is a real candidate that was spiked and rejected: collapsing the cell's
/// open-circuit voltage to zero the moment `soc` hits `0`. It closes most of the energy
/// hole for free, and it makes an ordinary *empty* cell look like a dead short to a
/// charger. The reversal branch does not, because at the instant the cell empties the
/// deficit is still zero and the source is still `OCV(0)`.
///
/// One step of 90 A for 1 s from `soc = 0.01` lands the cell exactly on empty and leaves
/// `Σ V_rc = 0.010·90·(1 − e^(−1/20))`. A `Voltage(3.3)` demand then draws
/// `(OCV(0) − Σ V_rc − 3.3) / R0`, about −17 A. The collapse rival draws −165 A.
///
/// The arrival step raises **no** flag, and that is the state this test needs rather than
/// an accident of the arithmetic: `coulomb_step` clamps on `raw < 0.0`, so a cell that
/// lands on exactly `0.0` has not gone past anything and its deficit is still zero. This
/// is the last instant at which the two candidates are distinguishable — one step later
/// the reversal branch has legitimately begun to fall.
#[test]
fn arriving_at_empty_does_not_look_like_a_short() {
    let mut pack = pack_at(0.01);
    let arrival = pack.step(1.0, Demand::Current(90.0), &env());
    assert!(
        !arrival.flags.contains(EventFlags::SOC_CLAMPED_LOW),
        "landing exactly on empty is not passing it"
    );
    assert_eq!(
        pack.cell(0, 0).expect("cell exists").soc,
        0.0,
        "the fixture is sized to land on exactly 0.0"
    );

    let v_rc = 0.010 * 90.0 * (1.0 - (-1.0_f64 / 20.0).exp());
    let expected = (OCV_EMPTY_V - v_rc - 3.3) / R0_OHMS;
    let tele = pack.step(0.25, Demand::Voltage(3.3), &env());
    assert!(
        (tele.i_actual - expected).abs() < 1e-9,
        "expected {expected} A into a just-emptied cell, got {}",
        tele.i_actual
    );
    let rival = (0.0 - v_rc - 3.3) / R0_OHMS;
    assert!(
        (tele.i_actual - rival).abs() > 100.0,
        "the collapse-at-empty rival draws {rival} A and is not distinguished"
    );
}

/// **The floor stops the ramp, and the ramp is what needs stopping.**
///
/// Unfloored the reversal voltage is unbounded — the spike measured −52 V on an LFP cell
/// in two minutes, with a step-size sensitivity worse than the defect being fixed. Here
/// the cell is driven 2.6 capacities past empty, which is far beyond where
/// `OCV(0) − v_per_soc·deficit` would have gone, and then driven twice as far again: the
/// terminal voltage must be *identical*, because both readings sit on the floor.
#[test]
fn the_reversal_floor_bounds_the_voltage() {
    let mut pack = pack_at(0.05);
    let mut deep = f64::NAN;
    for k in 0..2400 {
        let t = pack.step(0.25, Demand::Current(40.0), &env());
        if k == 1199 {
            deep = t.v_terminal;
        }
        if k == 2399 {
            // Not bit-equality: the RC overpotential is still relaxing toward its
            // steady state at 300 s (15 time constants, so ~1.2e-7 V left of the
            // 0.4 V it is heading for). The floor is what has stopped moving, and
            // 1e-6 V is below that residue rather than a tolerance for the branch.
            assert!(
                (t.v_terminal - deep).abs() < 1e-6,
                "past the floor the terminal voltage must stop moving: {deep} then {}",
                t.v_terminal
            );
            // Floor 0 V, minus the ohmic drop and the relaxed RC overpotential.
            let bound = 0.0 - 40.0 * R0_OHMS - 0.010 * 40.0;
            assert!(
                (t.v_terminal - bound).abs() < 1e-6,
                "expected the floored terminal voltage {bound} V, got {}",
                t.v_terminal
            );
        }
    }
}

/// **The cell stays linear in reversal, so the pack solve stays one closed-form pass.**
///
/// This is the property the three rejected candidates each lost, and the reason the
/// collapse is a stored deficit read at the start of a step rather than a branch taken
/// inside one. `Voltage` and `Power` are here as well as `Current` because
/// `solve_current`'s three arms are three different closed forms, and the limit cycle the
/// discontinuous rival produced showed up on the latter two only.
#[test]
fn the_solve_stays_one_pass_in_reversal() {
    for demand in [
        Demand::Current(40.0),
        Demand::Voltage(1.0),
        Demand::Power(100.0),
    ] {
        let mut pack = pack_at(0.05);
        for k in 0..400 {
            let t = pack.step(0.25, demand, &env());
            assert_eq!(
                t.solve_iterations, 1,
                "step {k} of {demand:?} took {} passes",
                t.solve_iterations
            );
            assert!(
                !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "step {k} of {demand:?} did not converge"
            );
        }
    }
}

/// **A pack in reversal survives being written out and read back.**
///
/// `soc_deficit` is what makes `SNAPSHOT_VERSION` 14 a *semantic* bump, and the argument
/// there is that a blob without it restores `0.0` and continues a different trajectory.
/// Nothing else in the suite exercises that, for a reason worth naming: `Snapshot` holds
/// `Pack` **by value**, so `Pack::restore(&pack.snapshot())` is a clone and touches no
/// serde attribute at all. This goes through `bincode` — the same route
/// `snapshot.rs`'s replay test takes — so a field that fails to serialize is caught here
/// rather than believed to be fine.
///
/// The failure mode it guards is silent: a dropped or misnamed field restores `0.0`, and
/// a pack that was 20 % of its capacity past empty simply *climbs out of the hole* and
/// resumes at `OCV(0)`. No error, no flag, a wrong trajectory — which is why the tail is
/// compared bit-for-bit rather than to a tolerance.
///
/// The vacuity guard comes first: this test proves nothing about the field unless the
/// field is non-zero at the moment the snapshot is taken.
#[test]
fn a_pack_in_reversal_survives_a_serde_round_trip() {
    const MID: usize = 240;
    const TAIL: usize = 60;
    let dt = 0.25;
    // Discharge past empty, then charge back through the ramp, so the tail exercises the
    // branch in both directions rather than sitting on the floor.
    let demand_at = |k: usize| {
        if k < MID + TAIL / 2 {
            Demand::Current(40.0)
        } else {
            Demand::Current(-40.0)
        }
    };

    let mut reference = pack_at(0.05);
    let mut ref_tail: Vec<Telemetry> = Vec::new();
    for k in 0..MID + TAIL {
        let tele = reference.step(dt, demand_at(k), &env());
        if k >= MID {
            ref_tail.push(tele);
        }
    }

    let mut replay = pack_at(0.05);
    for k in 0..MID {
        replay.step(dt, demand_at(k), &env());
    }
    let deficit = replay.cell(0, 0).expect("cell exists").soc_deficit;
    assert!(
        deficit > 0.1,
        "the fixture must be deep in reversal when it is snapshotted, or this test is \
         vacuous; got a deficit of {deficit}"
    );

    let bytes = bincode::serialize(&replay.snapshot()).expect("the snapshot serializes");
    let restored_snapshot = bincode::deserialize(&bytes).expect("the snapshot deserializes");
    let mut restored = Pack::restore(&restored_snapshot).expect("the snapshot restores");
    assert_eq!(
        restored.cell(0, 0).expect("cell exists").soc_deficit,
        deficit,
        "the deficit must cross the serde boundary unchanged"
    );

    for (i, expected) in ref_tail.iter().enumerate() {
        assert_eq!(
            &restored.step(dt, demand_at(MID + i), &env()),
            expected,
            "telemetry diverged at tail index {i} after a round trip"
        );
    }
}

/// **Nothing the `[reversal]` section says can move a pack that stays inside its window.**
///
/// Measured rather than argued: the same 200-step run at two wildly different settings,
/// asserted bit-identical field by field. The spike's own control made this claim by
/// pinning 17 digits against a second tree; this makes it without one, and without a
/// constant that a libm difference could move.
#[test]
fn the_reversal_parameters_are_inert_inside_the_window() {
    let run = |v_per_soc: f64, floor_v: f64| {
        let mut chem = sloped_chem();
        chem.reversal.v_per_soc = v_per_soc;
        chem.reversal.floor_v = floor_v;
        let mut pack = Pack::new(&pack_config(0.5), chem).expect("fixture builds");
        let mut out = Vec::new();
        for _ in 0..200 {
            let t = pack.step(0.5, Demand::Current(4.0), &env());
            assert!(!t
                .flags
                .intersects(EventFlags::SOC_CLAMPED_HIGH | EventFlags::SOC_CLAMPED_LOW));
            out.push((t.v_terminal, t.i_actual, t.soc_true, t.q_gen_w));
        }
        out
    };
    assert_eq!(
        run(100.0, 0.0),
        run(1.0e6, -50.0),
        "a run that never leaves the window must not see the reversal parameters at all"
    );
}

/// A zero-length probe step reports the pack's state without advancing it, so there is
/// nothing to reject even on a cell sitting exactly on the clamp.
///
/// This is the same `dt = 0` contract the rest of the engine keeps, and it matters here
/// because the rejected amount is divided by `dt`.
#[test]
fn a_zero_length_probe_step_rejects_nothing() {
    let mut pack = pack_at(1.0);
    let probe = pack.step(0.0, Demand::Current(-180.0), &env());

    assert_eq!(
        probe.i_rejected_a, 0.0,
        "a zero-length step cannot have rejected charge"
    );
    assert!(
        probe.q_gen_w.is_finite(),
        "a zero-length step must not divide by dt: {}",
        probe.q_gen_w
    );
}

/// An ordinary run in the middle of the window reports exactly zero, not a rounding
/// residue — the field is a flag as much as a measurement, and clients branch on it.
#[test]
fn an_unclamped_run_rejects_exactly_zero() {
    let mut pack = pack_at(0.5);
    for _ in 0..200 {
        let tele = pack.step(0.5, Demand::Current(2.0), &env());
        assert_eq!(
            tele.i_rejected_a, 0.0,
            "an unclamped step reported {} A rejected",
            tele.i_rejected_a
        );
        assert!(!tele
            .flags
            .intersects(EventFlags::SOC_CLAMPED_HIGH | EventFlags::SOC_CLAMPED_LOW));
    }
}
