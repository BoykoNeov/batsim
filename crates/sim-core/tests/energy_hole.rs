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
//! See `docs/plans/energy-hole.md`.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
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

/// A 1S1P pack of [`sloped_chem`] at `soc0`, isothermal, no BMS — so the demanded
/// current reaches the cell unmodified and the only heat is the cell's own.
fn pack_at(soc0: f64) -> Pack {
    Pack::new(
        &PackConfig {
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
        },
        sloped_chem(),
    )
    .expect("fixture builds")
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
fn the_bottom_of_the_window_adds_no_heat() {
    let mut pack = pack_at(0.01);
    let tele = pack.step(1.0, Demand::Current(180.0), &env());

    assert!(tele.flags.contains(EventFlags::SOC_CLAMPED_LOW));
    assert!(
        (tele.i_rejected_a - 90.0).abs() < 1e-9,
        "the cell sourced 90 As it did not have, got {} A",
        tele.i_rejected_a
    );

    let ohmic_w = 180.0 * 180.0 * R0_OHMS;
    assert!(
        (tele.q_gen_w - ohmic_w).abs() < 1e-6,
        "the low clamp should generate ohmic heat only ({ohmic_w} W), got {}",
        tele.q_gen_w
    );
    // The symmetric-cooling rival, named so the assertion above is legible.
    let rival_w = ohmic_w - OCV_EMPTY_V * 90.0;
    assert!(
        (tele.q_gen_w - rival_w).abs() > 0.5,
        "the symmetric-cooling rival would give {rival_w} W and is not distinguished"
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
