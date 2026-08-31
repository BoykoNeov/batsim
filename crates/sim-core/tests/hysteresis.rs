//! The two mechanisms `SNAPSHOT_VERSION` 18 added: OCV hysteresis, and the temperature
//! correction of the open-circuit voltage.
//!
//! These are engine tests on hand-built chemistries chosen so each effect is separable —
//! nothing here is a claim about a real cell. The shipped NiMH parameter set is measured in
//! `sim-data`'s `nimh_chemistry.rs`, and the argument for both mechanisms is in
//! `docs/plans/phase-8-slice-c-hysteresis.md`.
//!
//! # What each half is for
//! * **Hysteresis** is a memory of drive direction that does not decay at rest. Everything
//!   below turns on that one property: it is what makes the state new rather than a third
//!   `[[rc]]` entry, and it is what makes the `#[serde(default)]` on the field a semantic
//!   version bump rather than a cosmetic one.
//! * **The temperature correction** is the other half of a `CLAUDE.md` sentence the code
//!   only ever did half of. It is gated on `ocv.t_ref_k` rather than on the coefficient
//!   column, and one test below is entirely about that gate.
//!
//! Every claim carries the same control: **the same experiment on a chemistry with the
//! section absent.** That is the only way to tell a mechanism from an artifact of the SOC
//! clamp, which `phase-8-slice-c-spike.md` measured produces a peak-and-fall all by itself.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, HysteresisParams, HysteresisWidth, OcvTable, R0Table,
    RcPair, ReversalParams, ThermalParams,
};
use sim_core::{
    ecm::{hysteresis_half_width_v, hysteresis_update},
    CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Cell capacity \[Ah\], so 1 C is 2.0 A.
const CAP_AH: f64 = 2.0;
const ROOM_K: f64 = 298.15;
/// Half-width of the test loop \[V\]. Deliberately large and round so a displacement is
/// unmistakable against the OCV table's own slope.
const M_V: f64 = 0.050;
/// Loop-crossing rate. `exp(-4)` of the way left after 10 % of capacity has moved.
const GAMMA: f64 = 40.0;
/// The test entropy coefficient \[V/K\]. Round, and an order above a real one, so the
/// correction is separable from an `R0` change by inspection.
const DOCV_DT: f64 = -1.0e-3;
/// Where the width table's knee sits. Round and well clear of both ends, so an arm can meet
/// itself on either side of it without approaching a clamp.
const KNEE: f64 = 0.50;
/// How much wider the loop is at the empty endpoint than at the knee.
const WIDE_MULT: f64 = 3.0;

fn env() -> Env {
    Env {
        t_ambient: ROOM_K,
        t_coolant: None,
    }
}

/// A deliberately boring cell: a straight OCV ramp, a **temperature-flat and SOC-flat**
/// `R0`, and one fast RC pair. Flat `R0` is the whole point — it removes the engine's only
/// other temperature-to-voltage channel, so anything temperature does below is the new
/// term and cannot be an ohmic side-effect.
fn base_chem() -> ChemistryParams {
    ChemistryParams {
        meta: ChemMeta {
            id: "hysteresis".into(),
            name: "Hysteresis test cell".into(),
            provenance: "engine test fixture — chosen so each new term is separable, not \
                         physical"
                .into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 4.20,
            v_min: 2.50,
            max_charge_c: 2.0,
            max_discharge_c: 3.0,
            t_charge_min_k: 273.15,
            t_max_k: 333.15,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![3.00, 3.50, 4.00],
            docv_dt_v_per_k: None,
            t_ref_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            // One temperature breakpoint, so the lookup clamps to it everywhere and `R0`
            // cannot move with temperature at all.
            temp_k: vec![ROOM_K],
            ohms: vec![vec![0.020], vec![0.020]],
        },
        rc: vec![RcPair {
            r_ohms: 0.010,
            c_farad: 1000.0, // tau = 10 s
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
        reversal: ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        diffusion: None,
        hysteresis: None,
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
    }
}

fn with_hysteresis() -> ChemistryParams {
    ChemistryParams {
        hysteresis: Some(HysteresisParams {
            scale_v: M_V,
            gamma: GAMMA,
            width_over_soc: None,
        }),
        ..base_chem()
    }
}

/// The same cell with a width table: the loop is `WIDE_MULT` times wider at empty than at
/// `KNEE` charge and above, which is the shape `SNAPSHOT_VERSION` 20 exists for.
fn with_width_table(mult: &[f64]) -> ChemistryParams {
    ChemistryParams {
        hysteresis: Some(HysteresisParams {
            scale_v: M_V,
            gamma: GAMMA,
            width_over_soc: Some(HysteresisWidth {
                soc: vec![0.0, KNEE, 1.0],
                mult: mult.to_vec(),
            }),
        }),
        ..base_chem()
    }
}

/// The base cell with an entropy coefficient supplied **for heat only** — no `t_ref_k`, so
/// the voltage correction stays off. This is the pre-v18 configuration and the control for
/// the gate.
fn with_docv_dt_only() -> ChemistryParams {
    let mut chem = base_chem();
    chem.ocv.docv_dt_v_per_k = Some(vec![DOCV_DT; chem.ocv.soc.len()]);
    chem
}

/// The same cell with the reference temperature stated, which switches the correction on.
fn with_temperature_correction() -> ChemistryParams {
    let mut chem = with_docv_dt_only();
    chem.ocv.t_ref_k = Some(ROOM_K);
    chem
}

fn config(soc0: f64, temp_k: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc: soc0,
        initial_temp_k: temp_k,
        seed: 0,
        scatter: Scatter::default(),
        // Isothermal throughout: every test here is about voltage, and a cell that heats
        // itself would make the two new terms interact through a channel none of them is
        // about.
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    }
}

/// Run `secs` seconds of `demand` at `dt = 1 s`, then read a zero-length probe.
fn drive(pack: &mut Pack, demand: Demand, secs: usize) -> f64 {
    for _ in 0..secs {
        pack.step(1.0, demand, &env());
    }
    pack.step(0.0, Demand::Rest, &env()).v_terminal
}

// ---------------------------------------------------------------------------------------
// The update rule itself
// ---------------------------------------------------------------------------------------

/// `hysteresis_update` is a convex combination toward `-sgn(i)`, and the degenerate cases
/// return the state untouched.
///
/// The last two are the ones that matter: **no charge moved means no memory changed**, and
/// that is the model rather than a fallback.
#[test]
fn the_update_moves_toward_the_opposite_of_the_drive_and_stays_in_range() {
    // Discharge drives toward -1, charge toward +1, from the unpolarized midpoint.
    let after_discharge = hysteresis_update(0.0, 2.0, CAP_AH, GAMMA, 3600.0);
    let after_charge = hysteresis_update(0.0, -2.0, CAP_AH, GAMMA, 3600.0);
    assert!(after_discharge < 0.0 && after_charge > 0.0);
    // One full capacity at gamma = 40 is exp(-40) from the endpoint: numerically arrived.
    assert!((after_discharge + 1.0).abs() < 1e-15);
    assert!((after_charge - 1.0).abs() < 1e-15);

    // It never leaves [-1, 1], from any starting point inside it, either way.
    for &h0 in &[-1.0, -0.4, 0.0, 0.4, 1.0] {
        for &i in &[-6.0, -0.1, 0.1, 6.0] {
            let h = hysteresis_update(h0, i, CAP_AH, GAMMA, 60.0);
            assert!(
                (-1.0..=1.0).contains(&h),
                "h left the interval: {h0} -> {h} at i = {i}"
            );
        }
    }

    // **Rest changes nothing.** Not approximately: the same bits back.
    let h0 = 0.37;
    assert_eq!(hysteresis_update(h0, 0.0, CAP_AH, GAMMA, 86_400.0), h0);
    // And neither does a zero-length step under load, which is the probe every client uses.
    assert_eq!(hysteresis_update(h0, 5.0, CAP_AH, GAMMA, 0.0), h0);
    // A NaN current is refused by the same guard rather than poisoning the state — the
    // `!(dz > 0.0)` spelling is what makes that true.
    assert_eq!(hysteresis_update(h0, f64::NAN, CAP_AH, GAMMA, 1.0), h0);
}

// ---------------------------------------------------------------------------------------
// Hysteresis, in a pack
// ---------------------------------------------------------------------------------------

/// The headline: two cells at the **same state of charge and the same temperature** rest at
/// different voltages, because they arrived from opposite directions — and the difference
/// is the full loop width the chemistry declares.
#[test]
fn two_cells_at_one_state_of_charge_rest_at_two_voltages() {
    let chem = with_hysteresis();
    // A full capacity in each direction, so both arms are numerically at their endpoints.
    let mut up = Pack::new(&config(0.0, ROOM_K), chem.clone()).expect("pack builds");
    let v_up = drive(&mut up, Demand::Current(-CAP_AH), 1800); // charge to 0.5
    let mut down = Pack::new(&config(1.0, ROOM_K), chem.clone()).expect("pack builds");
    let v_down = drive(&mut down, Demand::Current(CAP_AH), 1800); // discharge to 0.5

    // Let both relax for an hour so no RC overpotential is left in either reading.
    let v_up = {
        let _ = v_up;
        drive(&mut up, Demand::Rest, 3600)
    };
    let v_down = {
        let _ = v_down;
        drive(&mut down, Demand::Rest, 3600)
    };

    let (soc_up, soc_down) = (
        up.step(0.0, Demand::Rest, &env()).soc_true,
        down.step(0.0, Demand::Rest, &env()).soc_true,
    );
    assert!((soc_up - 0.5).abs() < 1e-12 && (soc_down - 0.5).abs() < 1e-12);

    // OCV(0.5) is 3.50 exactly, and each arm sits one half-width either side of it.
    assert!((v_up - (3.50 + M_V)).abs() < 1e-9, "charge branch: {v_up}");
    assert!(
        (v_down - (3.50 - M_V)).abs() < 1e-9,
        "discharge branch: {v_down}"
    );

    // The control: the identical experiment with the section absent puts both arms on one
    // voltage. The tolerance is for the coulomb count arriving at 0.5 from two directions,
    // not for a mechanism.
    let bare = base_chem();
    let mut up = Pack::new(&config(0.0, ROOM_K), bare.clone()).expect("pack builds");
    let mut down = Pack::new(&config(1.0, ROOM_K), bare).expect("pack builds");
    drive(&mut up, Demand::Current(-CAP_AH), 1800);
    drive(&mut down, Demand::Current(CAP_AH), 1800);
    let a = drive(&mut up, Demand::Rest, 3600);
    let b = drive(&mut down, Demand::Rest, 3600);
    assert!(
        (a - b).abs() < 1e-9,
        "without the section both arms must rest together: {a} vs {b}"
    );
}

/// **The property that made this a state and not an RC pair.** A week of open circuit does
/// not move the polarized cell by one bit, where an RC overpotential of the same size would
/// have decayed away in minutes.
#[test]
fn the_memory_does_not_decay_at_rest() {
    let chem = with_hysteresis();
    let mut pack = Pack::new(&config(0.2, ROOM_K), chem).expect("pack builds");
    drive(&mut pack, Demand::Current(-CAP_AH), 1800);
    // An hour first, which is 360 RC time constants: whatever is left after this is not the
    // RC pair.
    let after_1h = drive(&mut pack, Demand::Rest, 3600);
    let after_1w = drive(&mut pack, Demand::Rest, 7 * 24 * 3600);
    assert_eq!(
        after_1h, after_1w,
        "a week of rest must not move the displacement by a bit"
    );
    // And it really is displaced, so the equality above is not two identical zeroes.
    let soc = pack.step(0.0, Demand::Rest, &env()).soc_true;
    let ocv = 3.00 + soc; // the straight ramp this fixture declares
    assert!(
        (after_1w - (ocv + M_V)).abs() < 1e-9,
        "rested at {after_1w}, expected {} ",
        ocv + M_V
    );
}

/// Hysteresis is **dissipative**, and that is why it is routed through the overpotential
/// rather than through the open-circuit voltage.
///
/// A cell driven at a steady current in either direction reports *more* heat with the
/// section than without it, by exactly `M·|I|` once the state has settled — the loop's
/// enclosed area arriving on the thermal side. Had the term been added to
/// `open_circuit_v`, the voltage would have moved and the heat would not, and the pack
/// energy balance would have quietly stopped closing on this one chemistry.
#[test]
fn the_loop_costs_heat_in_both_directions() {
    for (label, i) in [("discharge", CAP_AH), ("charge", -CAP_AH)] {
        let with = with_hysteresis();
        let without = base_chem();
        let mut a = Pack::new(&config(0.5, ROOM_K), with).expect("pack builds");
        let mut b = Pack::new(&config(0.5, ROOM_K), without).expect("pack builds");
        // 900 s at 1 C moves a quarter of the capacity: `h` is within exp(-10) of its
        // endpoint, and the RC pair has long settled.
        let (mut q_with, mut q_without) = (0.0, 0.0);
        for _ in 0..900 {
            q_with = a.step(1.0, Demand::Current(i), &env()).q_gen_w;
            q_without = b.step(1.0, Demand::Current(i), &env()).q_gen_w;
        }
        let extra = q_with - q_without;
        // The settled value, and a tolerance DERIVED rather than chosen: `h` is within
        // `exp(-gamma*dz)` of its endpoint after `dz` of capacity has moved, so the reading
        // falls short of `M*|I|` by that fraction. The factor two is headroom for which step
        // inside the loop the reading lands on, and nothing else -- shrink the run and this
        // tolerance widens with it, which is the property a free number would not have.
        const DZ: f64 = 900.0 / 3600.0;
        let settled = M_V * i.abs();
        let residue = settled * 2.0 * (-GAMMA * DZ).exp();
        println!(
            "{label}: {q_with:.9} W vs {q_without:.9} W, extra {extra:.9} W against a \
             settled {settled:.9} W (residue bound {residue:.3e})"
        );
        assert!(
            extra > 0.0,
            "{label}: the loop must cost heat, not return it ({extra} W)"
        );
        assert!(
            (extra - settled).abs() < residue,
            "{label}: extra heat {extra:.12} W against the {settled:.12} W the parameters \
             predict, outside the {residue:.3e} W the state's own settling allows"
        );
    }
}

/// **The refused charge is booked against the cell's own source, not against the table.**
///
/// This test exists because a perturbation found nothing watching it, and the first version
/// of it asserted the wrong quantity — recorded here because the correction is the
/// interesting part.
///
/// `Pack::step` used to hoist `ocv_lookup(chem.ocv, 1.0)` out of the cell loop, which was
/// right while the endpoint a refused charge is pushed against was a property of the
/// chemistry alone. At v18 it is a property of the cell, and **only through the temperature
/// correction**:
///
/// * **Hysteresis is not the case this guards**, though it looks like it should be. The
///   displacement lives in [`ecm_overpotential_v`], so it already reaches the heat through
///   `q_irrev = I·(OCV − V)`; and `open_circuit_v` at `soc = 1.0` with no `t_ref_k` *is* the
///   bare table lookup, bit for bit. Booking the rejection at the hysteretic source instead
///   would count the loop twice. The first draft of this test asserted exactly that and was
///   right to fail.
/// * **The temperature correction is.** It moves `open_circuit_v` itself, so a hot
///   overcharged cell is being force-fed against a *lower* equilibrium potential than the
///   table states, and the hoisted lookup cannot see it.
///
/// Two arms, both carrying the entropy column so the reversible heat term is identical, and
/// differing in one field: whether the reference temperature is stated. Held at a fixed
/// temperature away from that reference, the whole difference in generated heat is the
/// rejection term, and it is `∂U/∂T · (T − T_ref) · |I|` exactly. Restore the hoisted lookup
/// and it is zero.
#[test]
fn the_refused_charge_is_booked_at_the_corrected_open_circuit_voltage() {
    const HOT_K: f64 = ROOM_K + 20.0;
    // Isothermal at a temperature away from the reference: the correction is live and
    // constant, and nothing else in the cell moves with temperature (the fixture's `R0`
    // grid has one breakpoint).
    let run = |chem: ChemistryParams| {
        let mut pack = Pack::new(&config(0.5, HOT_K), chem).expect("pack builds");
        let mut last = 0.0;
        // 1800 s fills the cell exactly; 900 s more is pure overcharge, every amp refused.
        for _ in 0..2700 {
            last = pack.step(1.0, Demand::Current(-CAP_AH), &env()).q_gen_w;
        }
        last
    };
    let q_gated = run(with_temperature_correction());
    let q_heat_only = run(with_docv_dt_only());
    let seen = q_gated - q_heat_only;
    let expect = DOCV_DT * (HOT_K - ROOM_K) * CAP_AH;

    println!(
        "refused-charge heat: {q_gated:.9} W with the correction, {q_heat_only:.9} W \
         without, difference {seen:.9} W against the {expect:.9} W the coefficient predicts"
    );
    assert!(
        (seen - expect).abs() < 1e-12,
        "the refused charge is booked with {seen:.12} W of temperature correction where the \
         cell's own open-circuit voltage says {expect:.12} W. A difference of zero means the \
         endpoint is being read from the chemistry's table rather than from the cell."
    );
    // Directional, so the number above cannot be satisfied by a sign error: a hot cell has a
    // lower equilibrium potential here, so refusing charge against it costs less heat.
    assert!(
        seen < 0.0,
        "a negative coefficient must lower the booked heat"
    );
}

// ---------------------------------------------------------------------------------------
// The OCV temperature correction
// ---------------------------------------------------------------------------------------

/// The correction is exactly `∂U/∂T · (T − t_ref_k)`, and it is **gated on `t_ref_k`, not
/// on the coefficient column**.
///
/// Three arms at the same state of charge and the same raised temperature: no column at
/// all, the column with no reference temperature (the pre-v18 configuration, which must be
/// bit-identical to having no column), and both.
#[test]
fn the_temperature_correction_is_gated_on_the_reference_temperature() {
    const HOT_K: f64 = ROOM_K + 20.0;
    let rest = |chem: ChemistryParams| {
        let mut pack = Pack::new(&config(0.5, HOT_K), chem).expect("pack builds");
        pack.step(0.0, Demand::Rest, &env()).v_terminal
    };

    let bare = rest(base_chem());
    let heat_only = rest(with_docv_dt_only());
    let corrected = rest(with_temperature_correction());

    // OCV(0.5) with a flat R0 and no current: the table value, exactly.
    assert_eq!(bare, 3.50);
    // **The gate.** A coefficient with no stated origin changes nothing about the voltage,
    // bit for bit — which is what lets a file keep the column for heat alone.
    assert_eq!(
        heat_only, bare,
        "an entropy column without ocv.t_ref_k must not move the voltage"
    );
    // And with the origin stated, the shift is the coefficient times the excursion.
    let expect = 3.50 + DOCV_DT * (HOT_K - ROOM_K);
    assert!(
        (corrected - expect).abs() < 1e-12,
        "corrected {corrected} against {expect}"
    );
    // At the reference temperature itself the correction is identically zero, so a
    // chemistry that declares it is unchanged there.
    let at_ref = {
        let mut pack =
            Pack::new(&config(0.5, ROOM_K), with_temperature_correction()).expect("pack builds");
        pack.step(0.0, Demand::Rest, &env()).v_terminal
    };
    assert_eq!(at_ref, bare, "no excursion, no correction");
}

/// The correction sits **above** the reversal ramp: an over-discharged cell in the cold
/// falls from a colder open-circuit voltage, rather than from the table's.
///
/// This is the only interaction between the two sections, and it follows from `[reversal]`
/// being defined as a drop *from* open-circuit voltage. Pinned so a future edit that
/// reorders the two shows up here rather than in a scenario.
#[test]
fn the_correction_applies_under_the_reversal_ramp_too() {
    const COLD_K: f64 = ROOM_K - 30.0;
    // Drive well past empty at 1 C from a nearly-flat start, in the cold.
    let run = |chem: ChemistryParams| {
        let mut pack = Pack::new(&config(0.01, COLD_K), chem).expect("pack builds");
        for _ in 0..120 {
            pack.step(1.0, Demand::Current(CAP_AH), &env());
        }
        pack.step(0.0, Demand::Rest, &env())
    };
    let bare = run(base_chem());
    let corrected = run(with_temperature_correction());
    assert!(
        bare.soc_true == 0.0 && corrected.soc_true == 0.0,
        "both arms must be past empty for this to be about the ramp"
    );
    // Same deficit, same ramp, so the whole difference is the correction — and it is the
    // same number the un-reversed cell would show, because the ramp subtracts from the
    // corrected potential rather than from the table.
    let expect = DOCV_DT * (COLD_K - ROOM_K);
    let seen = corrected.v_terminal - bare.v_terminal;
    assert!(
        (seen - expect).abs() < 1e-12,
        "below empty the correction is {seen}, expected {expect}"
    );
    assert!(seen > 0.0, "a colder cell must sit above a warmer one here");
}

// ---------------------------------------------------------------------------------------
// The version bump
// ---------------------------------------------------------------------------------------

/// A snapshot taken mid-loop restores onto the same trajectory — which is what makes the
/// field state rather than a cache, and is the argument `SNAPSHOT_VERSION` 18 rests on.
///
/// The save is taken **after a rest**, deliberately. That is where the field is at its most
/// load-bearing and where `#[serde(default)]`'s zero is at its most wrong: every other
/// history in `EcmState` has decayed away by then, so a restore that lost this one would
/// come back on the midline with nothing else to give it away.
#[test]
fn a_snapshot_taken_after_a_rest_restores_onto_the_same_trajectory() {
    let chem = with_hysteresis();
    let mut pack = Pack::new(&config(0.2, ROOM_K), chem.clone()).expect("pack builds");
    drive(&mut pack, Demand::Current(-CAP_AH), 900);
    drive(&mut pack, Demand::Rest, 6 * 3600);

    let snap = pack.snapshot();
    assert_eq!(snap.version, sim_core::SNAPSHOT_VERSION);
    let mut restored = Pack::restore(&snap).expect("a snapshot this build wrote restores");

    // Continue both for a while, through a direction change so the state is actually
    // exercised rather than merely carried.
    for k in 0..600 {
        let demand = if k < 300 {
            Demand::Current(CAP_AH)
        } else {
            Demand::Current(-CAP_AH)
        };
        let a = pack.step(1.0, demand, &env());
        let b = restored.step(1.0, demand, &env());
        assert_eq!(a.v_terminal, b.v_terminal, "step {k}");
        assert_eq!(a.q_gen_w, b.q_gen_w, "step {k}");
    }
}

/// **The v20 headline: the same cell's loop is wider at one end of its range than the
/// other, and the width is read at the charge state the memory is read at.**
///
/// Two meeting points, one either side of the table's knee, each reached by two arms that
/// moved *identical* charge in opposite directions. That symmetry is what makes the
/// comparison a measurement of the table and nothing else: the state `h` each arm settles
/// at depends only on how much charge passed through it and on `gamma`, so it is the same
/// magnitude at both meeting points and cancels out of the ratio. What is left is the
/// multiplier.
///
/// The arms below the knee travel through a *varying* half-width on their way, and that is
/// deliberately not visible in the answer: the resting voltage is `OCV(soc) ∓ M(soc)·h`, and
/// neither `soc` (a coulomb count) nor `h` (a function of charge moved) depends on `M` at
/// all. A reading that did depend on the path would mean the width had leaked into the
/// state, which is the failure this test would catch.
#[test]
fn the_loop_is_wider_where_the_table_says_it_is() {
    /// Charge moved by each arm, as a fraction of capacity. The same for all four so the
    /// saturation factor is shared.
    const DZ: f64 = 0.15;
    let secs = (DZ * 3600.0) as usize; // at 1 C

    let chem = with_width_table(&[WIDE_MULT, 1.0, 1.0]);
    // Both arms meet at `soc`, one arriving from above and one from below.
    let loop_width = |chem: &ChemistryParams, soc: f64| {
        let mut down = Pack::new(&config(soc + DZ, ROOM_K), chem.clone()).expect("pack builds");
        let mut up = Pack::new(&config(soc - DZ, ROOM_K), chem.clone()).expect("pack builds");
        drive(&mut down, Demand::Current(CAP_AH), secs);
        drive(&mut up, Demand::Current(-CAP_AH), secs);
        // An hour is 360 time constants of this fixture's only RC pair, so what is left is
        // the memory and not the relaxation.
        let v_down = drive(&mut down, Demand::Rest, 3600);
        let v_up = drive(&mut up, Demand::Rest, 3600);
        let (s_down, s_up) = (
            down.step(0.0, Demand::Rest, &env()).soc_true,
            up.step(0.0, Demand::Rest, &env()).soc_true,
        );
        // The two arms must actually have met, or a voltage gap is an SOC gap in disguise.
        assert!(
            (s_down - s_up).abs() < 1e-12 && (s_down - soc).abs() < 1e-12,
            "arms did not meet at {soc}: {s_down} vs {s_up}"
        );
        v_up - v_down
    };

    // How far across the loop `DZ` of charge carries a cell that started at `h = 0`.
    let crossed = 1.0 - (-GAMMA * DZ).exp();
    let above = loop_width(&chem, KNEE + 0.25);
    let below = loop_width(&chem, KNEE - 0.25);

    // Above the knee the multiplier is one, so this must be the width the bare section
    // gives — which is the claim that `scale_v` did not change meaning.
    let bare = loop_width(&with_hysteresis(), KNEE + 0.25);
    assert!(
        (above - bare).abs() < 1e-12,
        "above the knee the table must be inert: {above} vs {bare} without it"
    );
    assert!(
        (above - 2.0 * M_V * crossed).abs() < 1e-9,
        "above the knee: {above}, expected {}",
        2.0 * M_V * crossed
    );
    // Halfway between the endpoint and the knee, so the interpolated multiplier is halfway
    // between `WIDE_MULT` and one. Spelled as the interpolation rather than as a literal,
    // so that moving the fixture's constants cannot leave a stale expectation behind.
    let mult_below = 1.0 + (WIDE_MULT - 1.0) * 0.5;
    assert!(
        (below - 2.0 * M_V * mult_below * crossed).abs() < 1e-9,
        "below the knee: {below}, expected {}",
        2.0 * M_V * mult_below * crossed
    );
    assert!(
        (below / above - mult_below).abs() < 1e-9,
        "the ratio of the two loops must be the multiplier itself: {}",
        below / above
    );
}

/// **A table of all ones is the same bits as no table at all**, which is what makes the new
/// term a generalisation of the old one rather than a second code path beside it.
///
/// This is the perturbation that has to go *green*. `hysteresis_half_width_v` returns
/// `scale_v` untouched on the `None` arm rather than multiplying by an interpolated one, so
/// the equality here is not what keeps a chemistry without the table still — that is
/// structural. What it does check is the other direction: that a file which writes the
/// table out explicitly, flat, gets exactly what it would have got by omitting it, so
/// declaring the section costs nothing until it says something.
///
/// A trajectory rather than a single reading, and one that crosses the knee, because a
/// difference that only shows up under load would be invisible to a rested comparison.
#[test]
fn an_all_ones_table_is_the_same_bits_as_no_table() {
    let bare = with_hysteresis();
    let flat = with_width_table(&[1.0, 1.0, 1.0]);
    let mut a = Pack::new(&config(0.9, ROOM_K), bare).expect("pack builds");
    let mut b = Pack::new(&config(0.9, ROOM_K), flat).expect("pack builds");
    // Discharge across the knee, reverse, and charge back over it, so the state is moving
    // and the width is being looked up on both sides of the breakpoint.
    for (secs, amps) in [(2400usize, CAP_AH), (600, 0.0), (1200, -CAP_AH)] {
        for _ in 0..secs {
            let ta = a.step(1.0, Demand::Current(amps), &env());
            let tb = b.step(1.0, Demand::Current(amps), &env());
            assert_eq!(
                ta.v_terminal.to_bits(),
                tb.v_terminal.to_bits(),
                "a flat table moved the terminal voltage: {} vs {}",
                ta.v_terminal,
                tb.v_terminal
            );
            assert_eq!(ta.soc_true.to_bits(), tb.soc_true.to_bits());
        }
    }
}

/// The lookup clamps outside its breakpoints, like every other table here — so a cell
/// driven past empty into its reversal ramp, where `soc` is pinned at zero but the deficit
/// keeps growing, keeps the endpoint's width rather than extrapolating off the end of the
/// table into a negative one.
#[test]
fn the_half_width_clamps_outside_its_breakpoints() {
    let chem = with_width_table(&[WIDE_MULT, 1.0, 1.0]);
    let h = chem.hysteresis.as_ref().expect("the fixture declares one");
    assert!((hysteresis_half_width_v(h, 0.0) - M_V * WIDE_MULT).abs() < 1e-15);
    assert!((hysteresis_half_width_v(h, -1.0) - M_V * WIDE_MULT).abs() < 1e-15);
    assert!((hysteresis_half_width_v(h, 1.0) - M_V).abs() < 1e-15);
    assert!((hysteresis_half_width_v(h, 2.0) - M_V).abs() < 1e-15);
    // And with no table the same call is `scale_v` at every charge state, including ones
    // outside [0, 1].
    let bare = with_hysteresis();
    let h = bare.hysteresis.as_ref().expect("the fixture declares one");
    for soc in [-1.0, 0.0, 0.37, 1.0, 2.0] {
        assert_eq!(hysteresis_half_width_v(h, soc).to_bits(), M_V.to_bits());
    }
}
