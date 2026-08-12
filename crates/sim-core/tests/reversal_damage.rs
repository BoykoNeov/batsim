//! Over-discharge that leaves a mark.
//!
//! `docs/plans/low-clamp-reversal.md` made a cell driven past empty pay for the charge it
//! delivers there with a falling open-circuit voltage, closing the engine's last
//! conservation hole. What it left open, and named in its own deferred list, is that the
//! payment is *only* electrical: pump a pack below empty and charge it back and it returns
//! to the state it started in. A real cell does not. Past empty the anode has no lithium
//! left to give and its copper current collector oxidises instead, and that does not come
//! back.
//!
//! This file pins the fourth fade mechanism that models it. Three of the four assertions
//! are about **shape** — the coefficient is a labelled placeholder like every other number
//! in the shipped chemistries — but the first is an exact arithmetic identity, and it is
//! the one that would fail silently without a test.
//!
//! # Why the identity needs a test rather than an argument
//!
//! The damage is charged per amp-hour delivered past empty, and that quantity is a
//! *difference across the step* (`Δsoc_deficit × capacity`) rather than a reading. Two
//! things can go wrong invisibly:
//!
//! * Multiplying the deficit by any capacity other than the one `coulomb_step` divided by
//!   on that same step yields amp-hours that do not correspond to charge which crossed the
//!   terminals. Every existing test would still pass — nothing else in the suite asserts on
//!   reversal amp-hours at all.
//! * The step that *crosses* `soc = 0` delivers only part of its charge past the boundary.
//!   Gating `|i|·dt` on "below empty" would charge for all of it or none of it depending on
//!   which end of the step the test looked at, and a run whose steps happened to land on
//!   the boundary would hide it.
//!
//! [`reversal_ah_matches_current_integral`] straddles the boundary deliberately and inverts
//! the state of health to recover the amp-hours the engine actually billed.
//!
//! # Reading the state of health backwards
//!
//! `CellAging`'s per-mechanism accumulators are private, which is right — they are
//! bookkeeping, not API. So these tests zero the calendar and cycle coefficients, leave
//! `safety` unset so nothing plates, and run the aging sub-clock every step. Then
//! `soh_capacity = 1 − fade_per_ah · ah_reversed` exactly, and the amp-hours come back out
//! by inversion. Any of those three left on would fold another mechanism into the same
//! number and turn an identity into a tolerance.
//!
//! See `docs/plans/reversal-damage.md`.

use sim_core::aging::MIN_SOH_CAPACITY;
use sim_core::chem::{
    AgingParams, CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ReversalParams,
    ThermalParams,
};
use sim_core::{
    AgingConfig, CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Nominal cell capacity \[Ah\].
const CAP_AH: f64 = 2.5;

/// Capacity fraction lost per amp-hour delivered past empty.
///
/// Round, and far above anything shipped (the LFP file's is 2.2e-1), so that a handful of
/// simulated minutes produces a percent-scale loss instead of a rounding error. These
/// tests assert shape and arithmetic, never magnitude.
const FADE_PER_AH: f64 = 1.0;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Aging with **only** the reversal mechanism live: no calendar term, no cycle term, and
/// (via `safety: None` below) no plating. The resistance coupling stays at its usual
/// placeholder, because the whole point of one assertion here is that it is not zero.
fn aging_params() -> AgingParams {
    AgingParams {
        cal_pre_exp: 0.0,
        cal_ea_j_per_mol: 5.0e4,
        cal_soc_stress: vec![1.0],
        cyc_fade_per_ah: 0.0,
        cyc_dod_stress_exp: 1.1,
        r_growth_per_capacity_loss: 1.5,
    }
}

/// A sloped-OCV, single-RC cell. `fade_per_ah` is a parameter so the same cell can be
/// built with over-discharge free (every version before this one) and with it costly.
fn chem(fade_per_ah: f64) -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        reversal: ReversalParams {
            // OCV(0) is 3.0 V and the floor is 0, so the collapse spans 3 % of capacity —
            // the same "sized against this cell's own OCV(0)" rule the shipped files use.
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah,
        },
        aging: Some(aging_params()),
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "reversal_damage_test".into(),
            name: "Over-discharge test cell".into(),
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

/// A 1S1P pack. Isothermal, no BMS: nothing may derate the demand or open a contactor,
/// because every assertion here is about charge that the cell was *made* to deliver.
///
/// `sub_clock_period_s: 0.0` ages on every step — legitimate per [`AgingConfig`], and
/// necessary here so that no reversal amp-hours are still sitting in the accumulator,
/// unbilled, when a run ends.
fn pack(initial_soc: f64, fade_per_ah: f64, aging: bool) -> Pack {
    let cfg = PackConfig {
        aging: aging.then_some(AgingConfig {
            sub_clock_period_s: 0.0,
        }),
        bms: None,
        thermal: ThermalConfig::Isothermal,
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 7,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    };
    Pack::new(&cfg, chem(fade_per_ah)).expect("pack builds")
}

// --- the identity ---------------------------------------------------------

/// The amp-hours billed as over-discharge equal the charge that actually came out below
/// empty — across a step that straddles the boundary.
///
/// The arithmetic is arranged so the expected value needs no knowledge of where the steps
/// landed. A constant `I` for total time `T` moves `I·T/3600` amp-hours; the cell starts at
/// `soc0` and nothing has damaged it yet on the way down, so exactly `soc0 · CAP_AH` of
/// that is delivered *above* empty at full capacity, and every amp-hour after it is past
/// empty. `dt` is chosen so that the crossing falls strictly inside a step (180 s of
/// charge to empty against a 7 s step: 25.71 steps), which is the case a naive
/// `|i|·dt`-while-below-empty accumulator gets wrong.
///
/// The identity survives capacity shrinking mid-run, and that is not luck. Each step bills
/// `Δdeficit × C` where `C` is the same capacity `coulomb_step` divided by on that step, so
/// the product is that step's charge whatever `C` was, and the sum telescopes to the
/// integral. Using a post-tick state of health instead would break exactly here — the run
/// loses over a percent of capacity while it is below empty.
#[test]
fn reversal_ah_matches_current_integral() {
    const SOC0: f64 = 0.05;
    const I: f64 = 2.5; // 1C
    const DT: f64 = 7.0;
    const STEPS: usize = 40;

    let mut p = pack(SOC0, FADE_PER_AH, true);
    let e = env();
    let mut tele = p.step(0.0, Demand::Rest, &e);
    for _ in 0..STEPS {
        tele = p.step(DT, Demand::Current(I), &e);
    }

    #[allow(clippy::cast_precision_loss)]
    let total_ah = I * (STEPS as f64 * DT) / 3600.0;
    let expected_ah = total_ah - SOC0 * CAP_AH;
    let billed_ah = (1.0 - tele.soh_capacity) / FADE_PER_AH;

    assert!(
        expected_ah > 0.0,
        "the run must actually go past empty, or this test asserts nothing"
    );
    assert!(
        p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc_deficit > 0.0,
        "the cell must end below empty for the deficit path to have been exercised"
    );
    let rel = (billed_ah - expected_ah).abs() / expected_ah;
    assert!(
        rel < 1e-12,
        "billed {billed_ah} Ah past empty, charge delivered past empty was {expected_ah} Ah \
         (relative error {rel:e}). A mismatch here is a capacity mixed up between the \
         coulomb count and the aging bill, or a straddling step counted whole."
    );
}

/// Repayment is free: charging a reversed cell back up bills nothing further.
///
/// The damage is done on the way down. Once the deficit is being repaid the copper is
/// already in solution, and billing the return leg would charge twice for one excursion —
/// which is also what would happen if the accumulator took `|Δdeficit|` instead of its
/// positive part.
#[test]
fn repaying_the_deficit_costs_nothing_more() {
    let e = env();
    let mut p = pack(0.05, FADE_PER_AH, true);
    p.step(0.0, Demand::Rest, &e);
    for _ in 0..40 {
        p.step(7.0, Demand::Current(2.5), &e);
    }
    let after_discharge = p.step(0.0, Demand::Rest, &e).soh_capacity;

    // Charge back until the deficit is gone and then some.
    let mut tele = p.step(0.0, Demand::Rest, &e);
    for _ in 0..60 {
        tele = p.step(7.0, Demand::Current(-2.5), &e);
    }
    assert_eq!(
        p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc_deficit,
        0.0,
        "the deficit must be fully repaid, or this test never exercised the return leg"
    );
    assert_eq!(
        tele.soh_capacity, after_discharge,
        "charging a reversed cell back up must bill nothing further"
    );
}

// --- the exit criterion ---------------------------------------------------

/// A cell driven past empty and charged back is permanently worse — where before this
/// slice it was exactly, bit-for-bit, as new.
///
/// Both arms run the identical demand on the identical cell; the only difference is
/// `fade_per_ah`. The zero arm is the assertion that the old behaviour was what this
/// describes: not "approximately one" but the literal `1.0` it was built with.
#[test]
fn over_discharge_leaves_a_mark() {
    let e = env();
    let cycle = |fade: f64| {
        let mut p = pack(0.05, fade, true);
        p.step(0.0, Demand::Rest, &e);
        for _ in 0..40 {
            p.step(7.0, Demand::Current(2.5), &e);
        }
        for _ in 0..60 {
            p.step(7.0, Demand::Current(-2.5), &e);
        }
        p.step(0.0, Demand::Rest, &e)
    };

    let free = cycle(0.0);
    assert_eq!(
        (free.soh_capacity, free.soh_resistance),
        (1.0, 1.0),
        "with `fade_per_ah = 0` over-discharge must cost exactly nothing — this is the \
         behaviour every version before v15 had, and it is what the arm below changes"
    );

    let costly = cycle(FADE_PER_AH);
    assert!(
        costly.soh_capacity < 1.0,
        "a cell driven past empty must lose capacity, got {}",
        costly.soh_capacity
    );
    assert!(
        costly.soh_resistance > 1.0,
        "`CLAUDE.md` forbids capacity fade without the matching resistance growth, got {}",
        costly.soh_resistance
    );
    // The coupling is the shared one, so this is arithmetic rather than a second
    // mechanism — pinned because a future slice giving reversal its own coefficient must
    // notice that it is changing this.
    let loss = 1.0 - costly.soh_capacity;
    let rel = (costly.soh_resistance - (1.0 + 1.5 * loss)).abs();
    assert!(
        rel < 1e-12,
        "resistance growth must come from the shared `r_growth_per_capacity_loss`"
    );
}

/// A pack with `aging: None` reverses exactly as it always did and pays nothing.
///
/// The same contrast plating draws: the physics of the deficit is not gated on aging — the
/// cell still sources at a falling voltage — but a pack that cannot wear out has nowhere to
/// put the damage. Asserting the *deficit* matches between the two arms is what makes this
/// a statement about where the damage lands rather than about whether reversal happened.
#[test]
fn aging_off_pays_nothing_and_still_reverses() {
    let e = env();
    let run = |aging: bool| {
        let mut p = pack(0.05, FADE_PER_AH, aging);
        p.step(0.0, Demand::Rest, &e);
        let mut tele = p.step(0.0, Demand::Rest, &e);
        for _ in 0..40 {
            tele = p.step(7.0, Demand::Current(2.5), &e);
        }
        (
            tele,
            p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc_deficit,
        )
    };

    let (aged, deficit_aged) = run(true);
    let (ageless, deficit_ageless) = run(false);

    assert_eq!(
        (ageless.soh_capacity, ageless.soh_resistance),
        (1.0, 1.0),
        "aging off means off, however far past empty the pack is driven"
    );
    assert!(
        aged.soh_capacity < 1.0,
        "the aged arm must have been damaged, or the contrast is vacuous"
    );
    assert!(
        deficit_ageless > 0.0 && deficit_aged > 0.0,
        "both arms must actually reverse"
    );
    // Not equal: the damaged cell has less capacity, so the same charge is a larger
    // fraction of it and the deficit runs deeper. That is the feedback, and it is in the
    // direction that says the damage is real rather than cosmetic.
    assert!(
        deficit_aged > deficit_ageless,
        "damage must deepen the deficit for the same charge ({deficit_aged} vs \
         {deficit_ageless})"
    );
}

// --- the feedback, measured rather than argued ----------------------------

/// Drive the pack to `target_deficit` past empty, recover to `recover_soc`, repeat —
/// returning the capacity state of health at the end of each cycle.
///
/// The legs are closed loops on *state* rather than fixed durations, which is what makes
/// the excursion a fixed fraction of whatever capacity is left.
fn fixed_depth_cycles(fade: f64, target_deficit: f64, cycles: usize) -> Vec<f64> {
    let e = env();
    let mut p = pack(0.05, fade, true);
    p.step(0.0, Demand::Rest, &e);
    let deficit = |p: &Pack| p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc_deficit;
    let soc = |p: &Pack| p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc;

    let mut soh = Vec::with_capacity(cycles);
    for _ in 0..cycles {
        let mut guard = 0;
        while deficit(&p) < target_deficit {
            p.step(1.0, Demand::Current(2.5), &e);
            guard += 1;
            assert!(guard < 10_000, "the discharge leg never reached its depth");
        }
        guard = 0;
        while soc(&p) < 0.05 {
            p.step(1.0, Demand::Current(-2.5), &e);
            guard += 1;
            assert!(guard < 10_000, "the charge leg never recovered");
        }
        soh.push(p.step(0.0, Demand::Rest, &e).soh_capacity);
    }
    soh
}

/// Repeated over-discharge to a **fixed depth** decays geometrically. It does not run away,
/// and — measured rather than assumed — it does not level off either.
///
/// This is the mirror of `plating.rs::repeated_cold_charging_fades_without_running_away`,
/// and the self-limiting mechanism is **not** the same one, so the claim had to be
/// re-derived. Plating is bounded because its C-rate threshold does not scale with damage.
/// Reversal has no threshold. What bounds it is that the charge a fixed-*fraction* excursion
/// delivers past empty is proportional to the capacity still there, so each cycle costs a
/// fixed fraction of what is left: `soh(n+1) = soh(n)·(1 − fade·depth·capacity)`.
///
/// **That is exponential decay, and the honest statement is not "it stays far from the
/// floor".** It approaches the floor asymptotically, and for a large enough coefficient it
/// gets there — this test's `FADE_PER_AH` is 4.5× the shipped LFP figure and drives the cell
/// to 4 % of nominal in 60 excursions. What it never does is overshoot, oscillate, or
/// accelerate. The magnitude claim is made separately below, at the coefficient that
/// actually ships, where "far from the floor" is true and worth stating.
#[test]
fn repeated_over_discharge_fades_without_running_away() {
    let soh = fixed_depth_cycles(FADE_PER_AH, 0.02, 60);

    let last = *soh.last().expect("cycles > 0");
    assert!(
        last < soh[0],
        "repeated over-discharge must fade the cell ({} then {last})",
        soh[0]
    );
    for (i, s) in soh.iter().enumerate() {
        assert!(s.is_finite(), "health went non-finite at cycle {i}: {s}");
        assert!(
            *s >= MIN_SOH_CAPACITY,
            "the capacity floor must hold at cycle {i}, got {s}"
        );
    }
    // Monotone and decelerating, asserted over every consecutive triple rather than
    // end-to-end so that a single accelerating cycle cannot hide inside a falling average.
    // Deceleration in absolute terms is what "geometric" looks like from here, and it is
    // the whole no-runaway claim.
    for w in soh.windows(3) {
        assert!(w[1] < w[0] && w[2] < w[1], "fade must be monotone: {w:?}");
        assert!(
            w[0] - w[1] >= w[1] - w[2] - 1e-15,
            "fade must decelerate at fixed depth, but a cycle cost more than its \
             predecessor: {w:?}"
        );
    }
}

/// At the coefficient that actually ships, sixty full reversals leave a working cell.
///
/// The sibling above deliberately runs an exaggerated coefficient to make the *shape*
/// legible, which costs it the right to say anything about magnitude. This one says the
/// magnitude, using the LFP file's `fade_per_ah` on a cell with the same 2.5 Ah class of
/// capacity: sixty excursions to 2 % past empty — abuse that would ruin a real pack — take
/// roughly half the capacity and leave the cell far above the floor.
///
/// It is a scale check, in the sense `docs/plans/phase-3-aging-faults.md` established for
/// the aging and runaway placeholders: not a fitted number, but a statement that the
/// placeholder is plausible against the others in the same file rather than off by orders
/// of magnitude. The band is wide because the number inside it is not fitted.
#[test]
fn the_shipped_coefficient_is_the_right_order_of_magnitude() {
    /// `chemistries/lfp_26650_generic.toml`, `[reversal] fade_per_ah`.
    const SHIPPED: f64 = 2.2e-1;

    let soh = fixed_depth_cycles(SHIPPED, 0.02, 60);
    let last = *soh.last().expect("cycles > 0");

    assert!(
        (0.3..0.8).contains(&last),
        "sixty excursions to 2 % past empty should cost a serious but survivable fraction \
         of capacity at the shipped coefficient; got {last}. Outside this band the \
         placeholder has drifted far enough from the rest of the file that the lesson \
         changes — either one over-discharge is unnoticeable, or the first one is fatal."
    );
    assert!(
        last > 30.0 * MIN_SOH_CAPACITY,
        "and the cell must still be a long way from the capacity floor, got {last}"
    );
}

/// Repeated over-discharge under a **fixed absolute draw** accelerates instead — and that
/// is physics, not a modelling artifact.
///
/// The two readings of "repeat the abuse" disagree, so both are pinned. Here the load takes
/// the same amp-hours out every cycle whatever the cell has left, and the charger refills to
/// the same *relative* level. The charge needed to reach empty falls with capacity, so more
/// of that unchanged draw lands past empty each time and the damage per cycle **grows**. A
/// shrinking cell under an unchanged load really is destroyed faster and faster, and the
/// engine should not flatter it.
///
/// What this test defends is the tail: the acceleration stays finite and lands on
/// [`MIN_SOH_CAPACITY`] rather than producing a negative capacity that the coulomb count
/// would then divide by. It is the pathological configuration that floor exists for, run
/// deliberately rather than reasoned about.
#[test]
fn a_fixed_absolute_draw_accelerates_instead() {
    /// Long enough to pass empty from `soc = 0.05` on a healthy 2.5 Ah cell: 300 s at
    /// 2.5 A is 0.208 Ah against the 0.125 Ah that reaching empty costs. A shorter leg
    /// never reverses at all and the test would assert on a flat line of `1.0`.
    const LEG_S: f64 = 300.0;
    const CYCLES: usize = 40;

    let e = env();
    let mut p = pack(0.05, FADE_PER_AH, true);
    p.step(0.0, Demand::Rest, &e);
    let soc = |p: &Pack| p.cell(0, 0).expect("1S1P has a cell at (0, 0)").soc;

    let mut soh = Vec::with_capacity(CYCLES);
    for _ in 0..CYCLES {
        let mut t = 0.0;
        while t < LEG_S {
            p.step(1.0, Demand::Current(2.5), &e);
            t += 1.0;
        }
        let mut guard = 0;
        while soc(&p) < 0.05 {
            p.step(1.0, Demand::Current(-2.5), &e);
            guard += 1;
            assert!(guard < 100_000, "the charge leg never recovered");
        }
        soh.push(p.step(0.0, Demand::Rest, &e).soh_capacity);
    }

    let accelerated = soh
        .windows(3)
        .any(|w| (w[1] - w[2]) > (w[0] - w[1]) + 1e-15);
    assert!(
        accelerated,
        "a fixed absolute draw on a shrinking cell must cost more each cycle at some point; \
         if this stops being true the feedback described above has gone, and the sibling \
         fixed-depth test is then the only remaining statement about it. Series: {soh:?}"
    );
    for (i, s) in soh.iter().enumerate() {
        assert!(s.is_finite(), "health went non-finite at cycle {i}: {s}");
        assert!(
            *s >= MIN_SOH_CAPACITY,
            "the capacity floor must hold at cycle {i}, got {s}"
        );
    }
}
