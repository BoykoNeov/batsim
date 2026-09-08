//! Runaway inside one coarse step: ignition that does not wait for the next call, and
//! a burn that a fast-forward `dt` can actually integrate.
//!
//! `docs/plans/thermal-implicit-integrator.md` left this behind when it closed the
//! integrator hole: *"runaway ignition still lags a whole step, so a live `[safety]`
//! section and a day-long `dt` do not belong in the same run."*
//! `docs/plans/runaway-inside-a-coarse-step.md` is the note for the slice that answers
//! it. Three separate things were wrong, and they are **not** equally wrong — which is
//! the finding of the slice and the reason this file is arranged the way it is:
//!
//! * **the ignition gate was read once, from start-of-step temperatures.** A cell that
//!   crossed onset during a day-long step released nothing for a day. Two tests here
//!   fail against the pre-slice engine on exactly this.
//! * **the reacting loop bounded every sub-step by the explicit stability ceiling**,
//!   whether or not anything was reacting, so its whole reach was
//!   `MAX_RUNAWAY_SUBSTEPS · 11.875 s` ≈ 6.8 h, after which it took one explicit Euler
//!   jump it could not vouch for. Measured: that jump is **harmless in most of the
//!   cases you would build to catch it**, because after 6.8 h the pack has relaxed onto
//!   the fixed point and amplifying zero gives zero. It is catastrophic when the cap
//!   binds *mid-burn* — 1.8 million kelvin, on the fixture in the last test here.
//! * **the accuracy bound was taken on generation rather than on the net rate**, so a
//!   pack sitting at its quasi-steady temperature paid sub-steps for a temperature that
//!   was not moving. That is a cost, and on its own it produced right answers.
//!
//! So three of the tests below **pass against the pre-slice engine too**, and say so in
//! their own doc comments. They are guards on the new solve, not demonstrations of the
//! old defect, and the perturbation table in the plan note is where their value is
//! recorded. Writing them as if they proved more would be the mistake this repository
//! keeps rediscovering.
//!
//! Every fix is gated at the `dt` where the linear network already switches to backward
//! Euler (6080 s for the parameters below), so one test here pins the gate from
//! *underneath*: at a `dt` just below it the one-step lag is still there, deliberately.
//!
//! # Why the comparison had to be taken in release
//!
//! The pre-slice code fires a `debug_assert` on the way into this regime, so a debug run
//! panics before the behaviour can be observed at all. Every pre/post number quoted in
//! this file was measured in release. The assertions themselves pass in both profiles.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

/// Ambient \[K\], and the temperature every pack here starts at unless it says
/// otherwise.
const AMBIENT_K: f64 = 298.15;
/// Onset \[K\], deliberately only 16.85 K above ambient rather than the shipped LFP's
/// 423.15 K. A test that had to heat a pack 125 K before anything happened would be
/// measuring the heater, not the ignition gate. It sits above where [`WARM_UP_S`] leaves
/// a loaded pack (310.75 K) and below its quasi-steady 318.24 K, so a warmed pack is
/// still cold when a coarse step starts and crosses onset a few sub-steps into it.
const ONSET_K: f64 = 315.0;
/// Vent \[K\]: above onset, and below where a burning cell gets to.
const VENT_K: f64 = 330.0;
/// Per-cell exothermic budget \[J\] and heat capacity \[J/K\] — the shipped LFP pair, so
/// the adiabatic rise is the same 252.6 K the rest of the suite reasons about.
const BUDGET_J: f64 = 24.0e3;
const C_TH: f64 = 95.0;
/// Convection \[W/K\] and neighbour conductance \[W/K\], also the shipped values. `k` is
/// what puts the explicit stability ceiling at `SUBSTEP_SAFETY·C/max(4k, hA)` = 11.875 s,
/// and therefore the whole arithmetic this file is about.
const H_AREA_W_PER_K: f64 = 0.35;
const K_NEIGHBOR_W_PER_K: f64 = 1.0;
/// Reaction amplitude \[W\] at onset for a cell that runs away, and for one that only
/// smoulders. The second is four orders down: it releases, it never accelerates, and it
/// is how the "hovering above onset" case is built without the pack burning out.
const P_ONSET_W: f64 = 5.0;
const P_SMOULDER_W: f64 = 1.0e-4;
/// Arrhenius exponent \[J/mol\].
const EA_J_PER_MOL: f64 = 1.0e5;
/// Molar gas constant \[J/(mol·K)\], duplicated here rather than imported for the reason
/// `runaway.rs` gives: a test that imports the engine's constant cannot catch the engine
/// changing it.
const R_GAS: f64 = 8.314_462_618_153_24;

/// A day \[s\]: the `dt` the plan note is about, and 14.2× the linear gate.
const DAY_S: f64 = 86_400.0;
/// A `dt` just **below** the gate (`512 · 11.875` = 6080 s), used to pin that nothing
/// changed underneath it.
const BELOW_GATE_S: f64 = 6_000.0;
/// The reference `dt` every coarse arm is compared against: three orders below the gate,
/// so it takes the same explicit path it always did.
const FINE_DT_S: f64 = 10.0;

/// Discharge current \[A\] for the heated packs. With `R0 + R_rc` = 0.03 Ω each and the
/// exposures of a three-cell chain (0.75, 0.5, 0.75), the settled steady rise is
/// `3·0.03·I² / (1.75·0.35)` = 20.09 K, i.e. a quasi-steady pack at 318.24 K — safely
/// past `ONSET_K` and nowhere near a runaway on its own.
const LOAD_A: f64 = 12.5;

/// Deliberately enormous: these runs are a simulated day long at 12.5 A, which is 300 Ah
/// of throughput. A fixture that emptied would be measuring the SOC clamp.
const CAP_AH: f64 = 1.0e5;

/// Seconds of 1 s steps every compared run takes **before** the arm under test begins,
/// and the reason it is not optional.
///
/// Heat generation is solved once per step and held constant across it — the open item
/// `docs/plans/thermal-implicit-integrator.md` names as the remaining cost of a coarse
/// `dt`, and the one this file is *not* about. A day-long step taken from a fresh pack
/// therefore burns the whole day at `I²·R0` = 3.125 W and never sees the RC pair's
/// `I²·R_rc`, settling 6.6 K below where a fine `dt` puts it. Measured, before this
/// warm-up existed: 311.45 K against the fine arm's 318.10 K.
///
/// 400 s is twenty RC time constants (`τ` = 20 s), which is what it takes for the
/// residue to stop mattering: at 100 s the same pair still disagreed by 0.045 K, which
/// is exactly the `e^−5` of unsettled overpotential the coarse arm would then have
/// frozen for a day. It is 12.6 K of the 20 K thermal rise, which is why [`ONSET_K`]
/// sits where it does.
const WARM_UP_S: f64 = 400.0;

fn env() -> Env {
    Env {
        t_ambient: AMBIENT_K,
        t_coolant: None,
    }
}

fn safety(runaway_power_w_at_onset: f64) -> SafetyParams {
    SafetyParams {
        t_onset_k: ONSET_K,
        t_vent_k: VENT_K,
        runaway_energy_j: BUDGET_J,
        runaway_power_w_at_onset,
        runaway_ea_j_per_mol: EA_J_PER_MOL,
        // Plating off. These packs discharge, so nothing here could plate anyway, but
        // saying so keeps the fixture readable.
        t_plating_min_k: Some(273.15),
        plating_c_threshold: Some(0.5),
        plating_fade_per_ah: 0.0,
        plating_short_hazard_per_ah: 0.0,
        plating_short_ohms: 0.0,
    }
}

/// A deliberately dull cell: flat `R0` in both SOC and temperature, one RC pair, no
/// aging, no hysteresis, no diffusion. Everything interesting in this file is the
/// thermal integrator, and a temperature-dependent `R0` would quietly assist or oppose
/// the reaction being measured.
fn chem(runaway_power_w_at_onset: f64) -> ChemistryParams {
    ChemistryParams {
        diffusion: None,
        hysteresis: None,
        charge_acceptance: None,
        reversal: sim_core::ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        },
        aging: None,
        safety: Some(safety(runaway_power_w_at_onset)),
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "runaway_coarse_step_test".into(),
            name: "Coarse-step runaway test cell".into(),
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
            t_ref_k: None,
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![3.00, 3.30, 3.60],
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![273.15, 373.15],
            ohms: vec![vec![0.02, 0.02], vec![0.02, 0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: C_TH,
            h_area_w_per_k: H_AREA_W_PER_K,
        },
    }
}

/// Three cells in series with a live network. Three is the smallest chain in which the
/// middle cell is less exposed than its neighbours, so it is the smallest pack that
/// ignites somewhere in particular rather than everywhere at once.
fn cfg(initial_temp_k: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: K_NEIGHBOR_W_PER_K,
        },
        series: 3,
        parallel: 1,
        initial_soc: 1.0,
        initial_temp_k,
        seed: 7,
        scatter: Scatter::default(),
        cell_model: CellModelConfig::Ecm,
    }
}

fn pack(initial_temp_k: f64, runaway_power_w_at_onset: f64) -> Pack {
    Pack::new(&cfg(initial_temp_k), chem(runaway_power_w_at_onset)).expect("fixture builds")
}

/// What a run leaves behind, in the four quantities every test here reads.
struct Outcome {
    /// Exothermic energy \[J\] released across the pack over the whole run.
    released_j: f64,
    /// Per-cell temperature \[K\] when the warm-up ended and the window began. What the
    /// coarse arm still had to move, which is what its derived tolerances are built on.
    start_temps_k: Vec<f64>,
    /// Per-cell temperature \[K\] at the end.
    temps_k: Vec<f64>,
    /// Per-cell exothermic budget \[J\] left.
    budget_left_j: Vec<f64>,
    /// Whether every cell has vented.
    all_vented: bool,
    /// Whether `THERMAL_RUNAWAY` was ever reported.
    ever_flagged: bool,
}

/// Warm the pack for `warm_up_s` at 1 s steps, then integrate `window_s` seconds in
/// steps of `dt`, accumulating the release across both.
///
/// The warm-up is identical in every arm, so two arms that differ only in `dt` enter the
/// window in bit-identical state and the comparison is about the window alone. A run
/// under [`Demand::Rest`] passes `0.0`: there is no ohmic heat for a warm-up to settle,
/// and warming a pack that is *already at onset* would burn it out before the window it
/// is supposed to burn in.
fn run(mut pack: Pack, dt: f64, window_s: f64, demand: Demand, warm_up_s: f64) -> Outcome {
    let mut released_j = 0.0;
    let mut ever_flagged = false;
    let mut tick = |pack: &mut Pack, dt: f64| {
        let tele = pack.step(dt, demand, &env());
        released_j += tele.q_runaway_w * dt;
        ever_flagged |= tele.flags.contains(EventFlags::THERMAL_RUNAWAY);
    };
    for _ in 0..(warm_up_s as usize) {
        tick(&mut pack, 1.0);
    }
    let start_temps_k: Vec<f64> = (0..3)
        .map(|s| pack.cell(s, 0).expect("in range").temp_k)
        .collect();
    let steps = (window_s / dt).round() as usize;
    for _ in 0..steps {
        tick(&mut pack, dt);
    }
    // The closure borrows the accumulators; let it go before they are read.
    let _ = tick;
    let cells: Vec<_> = (0..3).map(|s| pack.cell(s, 0).expect("in range")).collect();
    Outcome {
        released_j,
        start_temps_k,
        temps_k: cells.iter().map(|c| c.temp_k).collect(),
        budget_left_j: cells.iter().map(|c| c.runaway_energy_remaining_j).collect(),
        all_vented: cells.iter().all(|c| c.vented),
        ever_flagged,
    }
}

/// The pack-wide budget \[J\]: three cells.
const PACK_BUDGET_J: f64 = 3.0 * BUDGET_J;

// --- 1. ignition no longer waits for the next call ------------------------

/// A pack that crosses onset **inside** a day-long step ignites inside that step.
///
/// The chain reaches onset during sub-step 3 of the 512 the linear path takes at this
/// `dt`, i.e. between 337.5 s and 506.25 s in, and burns out long before the day ends.
/// Measured, for the single 86 400 s step: a 506.25 s stretch, then 458 reacting
/// sub-steps for the burn, then one 85 713 s stretch back to quasi-steady. Pre-slice the same single step
/// finished a day past onset having released exactly nothing, because the ignition gate
/// was read once from the start-of-step temperatures. That is the defect
/// `docs/plans/runaway-inside-a-coarse-step.md` calls D1, and this is the only test here
/// that fails when only the ignition watch is removed.
#[test]
fn a_pack_that_crosses_onset_inside_one_day_long_step_ignites_inside_it() {
    let out = run(
        pack(AMBIENT_K, P_ONSET_W),
        DAY_S,
        DAY_S,
        Demand::Current(LOAD_A),
        WARM_UP_S,
    );

    assert!(
        out.ever_flagged,
        "one day-long step crossed onset and never raised THERMAL_RUNAWAY"
    );
    // The release is self-limiting, so "it ignited and ran to completion" is an exact
    // statement about energy rather than a tolerance on a temperature.
    assert!(
        (out.released_j - PACK_BUDGET_J).abs() < 1e-6 * PACK_BUDGET_J,
        "released {} J against a {PACK_BUDGET_J} J pack budget",
        out.released_j
    );
    for (s, left) in out.budget_left_j.iter().enumerate() {
        assert_eq!(*left, 0.0, "cell {s} kept {left} J of its budget");
    }
    assert!(
        out.all_vented,
        "a cell that burned its whole budget never vented"
    );
    for t in &out.temps_k {
        assert!(t.is_finite(), "temperature {t} is not a temperature");
    }
}

/// …and it lands in the same place a fine `dt` puts it.
///
/// One 86 400 s step against 8640 steps of 10 s, same pack, same current. Both arms end
/// ~212 convective time constants past the burn, so both are at the same quasi-steady
/// temperature and this is a statement about the coarse arm having integrated the same
/// physics, not a race between two transients.
#[test]
fn the_day_long_ignition_agrees_with_a_fine_reference() {
    let coarse = run(
        pack(AMBIENT_K, P_ONSET_W),
        DAY_S,
        DAY_S,
        Demand::Current(LOAD_A),
        WARM_UP_S,
    );
    let fine = run(
        pack(AMBIENT_K, P_ONSET_W),
        FINE_DT_S,
        DAY_S,
        Demand::Current(LOAD_A),
        WARM_UP_S,
    );

    assert!(
        (coarse.released_j - fine.released_j).abs() < 1e-6 * PACK_BUDGET_J,
        "coarse released {} J, fine {} J",
        coarse.released_j,
        fine.released_j
    );
    assert_eq!(
        coarse.all_vented, fine.all_vented,
        "the arms disagree on venting"
    );
    for (s, (c, f)) in coarse.temps_k.iter().zip(&fine.temps_k).enumerate() {
        assert!((c - f).abs() < 1e-6, "cell {s}: coarse {c} K vs fine {f} K");
    }
}

// --- 2. a burning pack can be handed a coarse step ------------------------

/// A pack **already** at onset survives a day-long step, burns exactly its budget, and
/// comes back to ambient.
///
/// **This passes against the pre-slice engine too, and that is the finding.** The old
/// reacting loop did fall off its own reach here — 2048 sub-steps at the 11.875 s
/// explicit ceiling cover 24 320 s, and the remaining 62 080 s went in one explicit Euler
/// jump whose amplification factor is ~658. But by then the pack had been cooling for
/// 6.8 h against a 271 s time constant, so it was *on* the frozen fixed point, and 658
/// times nothing is nothing. Divergence needs the cap to bind while a cell is still
/// burning, which is what `a_burn_past_the_work_cap_still_ends_the_step_with_temperatures`
/// builds.
///
/// So what this test guards is the new solve, not the old defect: the burn happens inside
/// the step, the hours after it are an inert stretch on the linear integrator, and the
/// budget arithmetic still closes. The perturbation table in the plan note is where its
/// value is recorded — it is one of the tests that reddens when the stretch is removed.
#[test]
fn a_pack_already_at_onset_burns_out_inside_one_day_long_step() {
    let out = run(pack(ONSET_K, P_ONSET_W), DAY_S, DAY_S, Demand::Rest, 0.0);

    assert!(
        out.ever_flagged,
        "a pack starting at onset never flagged runaway"
    );
    assert!(
        (out.released_j - PACK_BUDGET_J).abs() < 1e-6 * PACK_BUDGET_J,
        "released {} J against a {PACK_BUDGET_J} J pack budget",
        out.released_j
    );
    assert!(out.all_vented, "a pack that burned out never vented");
    // A day of resting is 318 convective time constants (`C/hA` = 271 s), so "back to
    // ambient" is exact rather than approximate — and it is the assertion the pre-slice
    // divergence could not have passed by luck.
    for (s, t) in out.temps_k.iter().enumerate() {
        assert!(
            (t - AMBIENT_K).abs() < 1e-9,
            "cell {s} rested a day and came back at {t} K, not ambient"
        );
    }
}

// --- 3. hovering above onset costs sub-steps proportional to the reaction --

/// A pack held just above onset by ohmic heat, with a reaction four orders too weak to
/// run away, integrates a day in the sub-steps its *reaction* needs — not the ones the
/// linear network's stability would have needed.
///
/// Nothing here is ever inert, so the stretch of the previous test never runs: this pack
/// is reacting at every instant of the step. What makes it integrable is that above the
/// gate the sub-step drops the linear stability bound (the linear part is solved
/// implicitly) and takes its accuracy bound on the *net* rate of change rather than on
/// generation. A pack at quasi-steady generates 4.7 W per cell and moves nowhere, and
/// the pre-slice bound could not tell the difference.
///
/// **This one also passes against the pre-slice engine**, for the same reason as the
/// test above and with the same lesson: the old path burned 7276 sub-steps' worth of
/// stability bound it did not need, hit the 2048 cap, and jumped — from a state that was
/// already the fixed point. What it cost was work, not accuracy. The two arms agreed to
/// 1.4e-8 K pre-slice and to 1.2e-5 K post-slice, and the *post*-slice figure is the
/// larger one: three honest sub-steps are a coarser quadrature of the reaction than 2048
/// tiny ones, and a far better use of the step. Both are four orders inside the bound
/// below.
///
/// Measured sub-step counts for the coarse arm's single 86 400 s step: **3** reacting
/// sub-steps, against the 7276 the old stability bound asks for and the 2048 it would
/// have been cut off at.
#[test]
fn a_pack_hovering_above_onset_integrates_a_day_in_one_step() {
    let coarse = run(
        pack(318.0, P_SMOULDER_W),
        DAY_S,
        DAY_S,
        Demand::Current(LOAD_A),
        WARM_UP_S,
    );
    let fine = run(
        pack(318.0, P_SMOULDER_W),
        FINE_DT_S,
        DAY_S,
        Demand::Current(LOAD_A),
        WARM_UP_S,
    );

    assert!(
        coarse.ever_flagged,
        "a smouldering pack never flagged runaway"
    );
    assert!(
        !coarse.all_vented,
        "a smouldering pack is not supposed to reach the vent threshold"
    );

    // --- the release tolerance, derived rather than picked.
    //
    // The coarse arm holds the reaction rate constant across each sub-step, so wherever
    // the temperature is still moving its rate is stale. The Arrhenius logarithmic
    // sensitivity `∂lnQ/∂T = Ea/(R·T²)` is 0.119 /K here, and the largest temperature
    // error any frozen rate can carry is the excursion the pack still had left when the
    // window opened — the fine arm's settled temperature minus the temperature both arms
    // started from. So `exp(β·Δ) − 1` is an upper bound on the relative release error,
    // and it is computed from the run rather than pinned so it moves with the fixture.
    let excursion_k = fine
        .temps_k
        .iter()
        .zip(&coarse.start_temps_k)
        .map(|(settled, start)| (settled - start).abs())
        .fold(0.0_f64, f64::max);
    let t_ref = fine.temps_k[0];
    let beta_per_k = EA_J_PER_MOL / (R_GAS * t_ref * t_ref);
    let bound = (beta_per_k * excursion_k).exp() - 1.0;
    let rel = (coarse.released_j - fine.released_j).abs() / fine.released_j;
    assert!(
        rel < bound,
        "coarse released {} J, fine {} J: {rel:.3e} apart against a {bound:.3e} bound \
         from a {excursion_k:.4} K excursion at {beta_per_k:.4} /K",
        coarse.released_j,
        fine.released_j
    );

    // --- and the temperature tolerance, from the same side.
    //
    // The two arms' end temperatures can differ only through their disagreement about
    // the reaction, so the honest bound is what the *whole* reaction is worth in steady
    // state: the pack sheds `Σ exposure·hA` = 0.6125 W/K, so 1 W of release buys 1.63 K.
    // A coarse arm that had mis-integrated the reaction entirely would miss by that
    // much; anything far below it means the two arms are integrating the same physics.
    let q_pack_w = fine.released_j / (WARM_UP_S + DAY_S);
    let whole_reaction_k = q_pack_w / (1.75 * H_AREA_W_PER_K);
    for (s, (c, f)) in coarse.temps_k.iter().zip(&fine.temps_k).enumerate() {
        assert!(
            (c - f).abs() < whole_reaction_k,
            "cell {s}: coarse {c} K vs fine {f} K, against the {whole_reaction_k:.3e} K \
             the entire reaction is worth"
        );
    }
}

// --- the gate, pinned from underneath -------------------------------------

/// Below the gate the one-step ignition lag is **still there**, and that is deliberate.
///
/// At `dt` = 6000 s the linear path takes 506 explicit sub-steps — 6 short of the 512
/// that would switch it to backward Euler — so none of the machinery above is armed and
/// the step behaves exactly as it did before this slice. The pack crosses onset 363 s
/// in and still releases nothing until the following step.
///
/// This is the test that fails if someone arms the ignition watch unconditionally, which
/// is the change that would move every runaway trajectory in the suite. The lag it pins
/// is bounded by the gate: 6080 s, and `Pack::step` says so.
#[test]
fn below_the_gate_ignition_still_waits_for_the_next_step() {
    let mut p = pack(AMBIENT_K, P_ONSET_W);
    // The same warm-up the compared runs take, and needed for the same reason in the
    // other direction: a 6000 s step from a fresh pack holds `I²·R0` for the whole step
    // and settles at 311.5 K, which never reaches `ONSET_K` at all — the test would pass
    // by never setting up the lag it exists to pin. The `t_max` assertion below is what
    // keeps that failure mode visible.
    for _ in 0..(WARM_UP_S as usize) {
        p.step(1.0, Demand::Current(LOAD_A), &env());
    }

    let first = p.step(BELOW_GATE_S, Demand::Current(LOAD_A), &env());
    assert!(
        !first.flags.contains(EventFlags::THERMAL_RUNAWAY),
        "the sub-gate step ignited inside itself; the gate has moved"
    );
    assert_eq!(first.q_runaway_w, 0.0, "released heat without reacting");
    assert!(
        first.t_max >= ONSET_K,
        "the step ended at {} K, below onset — it never set up the lag it is pinning",
        first.t_max
    );
    for s in 0..3 {
        assert_eq!(
            p.cell(s, 0).expect("in range").runaway_energy_remaining_j,
            BUDGET_J,
            "cell {s} spent budget on a step that had not ignited"
        );
    }

    let second = p.step(BELOW_GATE_S, Demand::Current(LOAD_A), &env());
    assert!(
        second.flags.contains(EventFlags::THERMAL_RUNAWAY),
        "the step after the crossing did not ignite either"
    );
}

// --- the work cap, where it still binds ----------------------------------

/// A budget so large that the climb alone outruns the sub-step budget: 300 kJ over
/// 95 J/K is a 3158 K adiabatic rise, and the engine's 1 K per-sub-step rise bound allows 1 K of
/// it per sub-step, so one cell needs 3158 sub-steps against a 2048 budget for the whole
/// step. Three of them need 9474.
///
/// Nothing shipped is anywhere near this. It exists to reach the one branch a coarse
/// step can still fall off, which is the point of the test below.
const HUGE_BUDGET_J: f64 = 300.0e3;

/// A pack whose burn cannot fit inside one step's sub-step budget however the sub-steps
/// are chosen, started exactly at onset so the burn begins immediately.
fn cap_bound_pack() -> Pack {
    let mut c = chem(P_ONSET_W);
    c.safety
        .as_mut()
        .expect("the fixture always supplies safety")
        .runaway_energy_j = HUGE_BUDGET_J;
    Pack::new(&cfg(ONSET_K), c).expect("fixture builds")
}

/// When the sub-step budget binds anyway, the step still ends with temperatures.
///
/// This is the branch the slice does **not** remove, and the reason it is tested rather
/// than only documented. `MAX_RUNAWAY_SUBSTEPS` bounds burning, a burn's cost is set by
/// the chemistry, and a chemistry can always be written whose burn is bigger than the
/// budget. What changed is what happens there: the tail used to be one *explicit* Euler
/// jump, and above the gate it is a backward-Euler one with the reaction frozen at its
/// budget-clipped rate.
///
/// Measured on this fixture, in release, with a single 86 400 s step:
///
/// | | pre-slice | post-slice |
/// | --- | --- | --- |
/// | cell temperatures \[K\] | 1 779 787 / −3 897 368 / 1 779 787 | 312.90 / 313.30 / 312.90 |
/// | released \[J\] | 900 000 | 900 000 |
///
/// Both arms release exactly the pack budget — the clip that stops a cell paying out
/// more than it has was never the problem. What the explicit jump did was multiply the
/// distance from its own frozen fixed point by ~658 in one step, and a pack that is
/// *mid-burn* when the cap binds is nowhere near that fixed point. The three assertions
/// below are the physical bounds that catches: a temperature is finite, a pack with an
/// ambient sink and only heat sources never ends below ambient, and no cell exceeds
/// ambient plus what the whole pack's budget could possibly have raised it by.
///
/// The remaining error is real and is named in the plan note: the reaction rate is
/// frozen across the tail, so the release is smeared over it rather than placed where it
/// happened. Bounded and wrong beats unbounded and wrong; neither is right.
#[test]
fn a_burn_past_the_work_cap_still_ends_the_step_with_temperatures() {
    let mut p = cap_bound_pack();
    let tele = p.step(DAY_S, Demand::Rest, &env());

    let temps: Vec<f64> = (0..3)
        .map(|s| p.cell(s, 0).expect("in range").temp_k)
        .collect();
    // Ambient plus the entire pack budget dumped into one cell: the loosest bound that
    // is still a physical statement, and 500x below what the pre-slice jump returned.
    let ceiling_k = AMBIENT_K + 3.0 * HUGE_BUDGET_J / C_TH;
    for (s, t) in temps.iter().enumerate() {
        assert!(
            t.is_finite(),
            "cell {s} came back at {t}, which is not a temperature"
        );
        assert!(
            *t >= AMBIENT_K,
            "cell {s} ended at {t} K, below the ambient it can only be warmed away from"
        );
        assert!(
            *t <= ceiling_k,
            "cell {s} ended at {t} K, above the {ceiling_k} K the whole pack budget buys"
        );
    }
    // And the clip still holds: the pack pays out what it has and not a joule more.
    let released_j = tele.q_runaway_w * DAY_S;
    let pack_budget_j = 3.0 * HUGE_BUDGET_J;
    assert!(
        released_j <= pack_budget_j * (1.0 + 1e-12),
        "released {released_j} J against a {pack_budget_j} J pack budget"
    );
}
