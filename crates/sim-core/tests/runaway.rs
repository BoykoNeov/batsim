//! Phase 3, slice D: thermal runaway — the second emergent failure mode.
//!
//! Nothing here is scripted. A cell that burns does so because the ordinary thermal
//! network was handed a heat source that grows with temperature; a neighbour that
//! catches does so because heat conducted into it along the same `k_ij` every other
//! thermal test uses. `CLAUDE.md` forbids animating this, so the tests are arranged to
//! make an animation impossible to fake:
//!
//! * **the reaction term**, tested directly on the pure function, including that it is
//!   exactly the amplitude at onset and exactly zero everywhere it should be;
//! * **the integrator**, which is the part that can lie. An exponential source term
//!   retires the sub-step bound the linear network relies on, so the same trajectory is
//!   run at two different `dt` and required to agree — a single-`dt` runaway test proves
//!   nothing about whether the sub-stepping is adequate;
//! * **the energy budget**, which is what stops the exponential being unbounded;
//! * **propagation**, with a control run that differs *only* in the reaction amplitude,
//!   so the neighbour catching fire is attributable to the reaction and not to the
//!   heater that started it.
//!
//! Every `[safety]` number in every shipped chemistry is a labelled placeholder, so
//! nothing here asserts a fitted temperature or a fitted time. The quantitative
//! assertions are arithmetic properties of the mechanism: total release equals the
//! budget, the adiabatic rise equals budget over heat capacity, and two `dt` agree.

use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, SafetyParams, ThermalParams,
};
use sim_core::runaway::{is_vented, reaction_power, reaction_power_slope};
use sim_core::{
    Demand, Env, EventFlags, Fault, Pack, PackConfig, Scatter, Telemetry, ThermalConfig,
};

/// Onset and vent, matching the shipped LFP file.
const ONSET_K: f64 = 423.15;
const VENT_K: f64 = 453.15;
/// Per-cell exothermic budget \[J\] and heat capacity \[J/K\], also the shipped LFP
/// pair: 24 kJ over 95 J/K is a 252.6 K adiabatic rise.
const BUDGET_J: f64 = 24.0e3;
const C_TH: f64 = 95.0;
/// Reaction amplitude \[W\] at onset, and its Arrhenius exponent \[J/mol\].
const P_ONSET_W: f64 = 5.0;
const EA_J_PER_MOL: f64 = 1.0e5;
/// Molar gas constant, duplicated here on purpose: a test that imports the engine's
/// constant cannot catch the engine changing it.
const R_GAS: f64 = 8.314_462_618_153_24;

const AMBIENT_K: f64 = 298.15;
/// Deliberately huge, so the internal-short heater used by the propagation tests runs
/// for the whole run without the SOC clamp appearing in the trace.
const CAP_AH: f64 = 100.0;

fn env() -> Env {
    Env {
        t_ambient: AMBIENT_K,
        t_coolant: None,
    }
}

/// Shipped-LFP-like thresholds, with the reaction amplitude left to each test — it is
/// what the control runs vary.
fn safety(runaway_power_w_at_onset: f64) -> SafetyParams {
    SafetyParams {
        t_onset_k: ONSET_K,
        t_vent_k: VENT_K,
        runaway_energy_j: BUDGET_J,
        runaway_power_w_at_onset,
        runaway_ea_j_per_mol: EA_J_PER_MOL,
        // Plating off: these packs are hundreds of kelvin from the cold end, and a
        // runaway test has no business also being a plating test.
        t_plating_min_k: 273.15,
        plating_c_threshold: 0.5,
        plating_fade_per_ah: 0.0,
        plating_short_hazard_per_ah: 0.0,
        plating_short_ohms: 0.0,
    }
}

/// `h_area_w_per_k` is a parameter here because the two halves of this file want
/// opposite things: the single-cell tests want a **perfectly adiabatic** cell so the
/// budget arithmetic is exact, and the propagation tests want a real sink so that the
/// heater alone reaches a finite plateau and the control run means something.
fn chem(safety: Option<SafetyParams>, h_area_w_per_k: f64) -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety,
        spm: None,
        meta: ChemMeta {
            id: "runaway_test".into(),
            name: "Runaway test cell".into(),
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
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![3.00, 3.30, 3.60],
        },
        // Flat in temperature as well as SOC: the thermal feedback under test is the
        // reaction, and an R0 that fell with temperature would quietly assist it.
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
            h_area_w_per_k,
        },
    }
}

fn cfg(series: u16, initial_temp_k: f64, k_neighbor_w_per_k: f64) -> PackConfig {
    PackConfig {
        aging: None,
        bms: None,
        thermal: ThermalConfig::Network { k_neighbor_w_per_k },
        series,
        parallel: 1,
        initial_soc: 1.0,
        initial_temp_k,
        seed: 7,
        scatter: Scatter::default(),
    }
}

/// A single perfectly isolated cell starting exactly at onset: no neighbours, no
/// convection, no current. `dT/dt = Q_rxn/C_th` and nothing else.
fn adiabatic_cell(runaway_power_w_at_onset: f64) -> Pack {
    Pack::new(
        &cfg(1, ONSET_K, 0.0),
        chem(Some(safety(runaway_power_w_at_onset)), 0.0),
    )
    .expect("fixture builds")
}

fn temp(pack: &Pack, s: usize) -> f64 {
    pack.cell(s, 0).expect("cell in range").temp_k
}

fn budget_left(pack: &Pack, s: usize) -> f64 {
    pack.cell(s, 0)
        .expect("cell in range")
        .runaway_energy_remaining_j
}

fn vented(pack: &Pack, s: usize) -> bool {
    pack.cell(s, 0).expect("cell in range").vented
}

/// Rest `pack` for up to `max_steps` steps of `dt`, stopping as soon as `stop` holds.
/// Returns the simulation time at which it stopped, or `None` if it never did.
fn rest_until(
    pack: &mut Pack,
    dt: f64,
    max_steps: usize,
    stop: impl Fn(&Telemetry) -> bool,
) -> Option<f64> {
    for _ in 0..max_steps {
        let tele = pack.step(dt, Demand::Rest, &env());
        if stop(&tele) {
            return Some(pack.sim_time_s());
        }
    }
    None
}

// --- the reaction term ----------------------------------------------------

/// At exactly onset with a full budget the release is exactly the amplitude — the
/// property that makes `runaway_power_w_at_onset` a number a reader can sanity-check
/// against a plot, rather than a pre-exponential nobody can eyeball.
#[test]
fn the_release_at_onset_is_exactly_the_amplitude() {
    let s = safety(P_ONSET_W);
    assert_eq!(reaction_power(&s, ONSET_K, BUDGET_J), P_ONSET_W);
}

/// The Arrhenius factor is referenced to onset, and it is the closed form it claims to
/// be. Checked at a temperature far enough above onset that a wrong reference point (or
/// a missing minus sign) cannot coincidentally agree.
#[test]
fn the_release_follows_the_arrhenius_form_referenced_to_onset() {
    let s = safety(P_ONSET_W);
    let t = 600.0;
    let expected = P_ONSET_W * (-(EA_J_PER_MOL / R_GAS) * (1.0 / t - 1.0 / ONSET_K)).exp();
    let got = reaction_power(&s, t, BUDGET_J);
    assert!(
        (got - expected).abs() < 1e-9 * expected,
        "at {t} K expected {expected} W, got {got} W"
    );
    // And it really is an acceleration, not a constant heater.
    assert!(got > 1000.0 * P_ONSET_W, "{got} W is not an acceleration");
}

/// The release is first order in the unreacted fraction, which is what makes the
/// mechanism self-limiting rather than an unbounded exponential.
#[test]
fn the_release_scales_with_the_unreacted_fraction() {
    let s = safety(P_ONSET_W);
    let full = reaction_power(&s, 500.0, BUDGET_J);
    let half = reaction_power(&s, 500.0, BUDGET_J / 2.0);
    assert!(
        (half - full / 2.0).abs() < 1e-9 * full,
        "{half} vs {}",
        full / 2.0
    );
    assert_eq!(
        reaction_power(&s, 500.0, 0.0),
        0.0,
        "a burnt-out cell is inert"
    );
}

/// Every way of not reacting, including the two that a NaN would otherwise sneak past.
#[test]
fn nothing_reacts_below_onset_or_without_an_amplitude() {
    let live = safety(P_ONSET_W);
    let inert = safety(0.0);
    // A whisker below onset.
    assert_eq!(reaction_power(&live, ONSET_K - 1e-9, BUDGET_J), 0.0);
    // Hot, full budget, but the chemistry has no amplitude.
    assert_eq!(reaction_power(&inert, 800.0, BUDGET_J), 0.0);
    // A cell that has left the physical domain is not additionally set on fire.
    assert_eq!(reaction_power(&live, f64::NAN, BUDGET_J), 0.0);
    assert_eq!(reaction_power(&live, f64::INFINITY, f64::NAN), 0.0);
    assert!(!is_vented(&live, f64::NAN));
    assert_eq!(reaction_power_slope(&live, f64::NAN, 1.0), 0.0);
}

/// The slope the integrator budgets its sub-steps against really is `dQ/dT`, checked
/// against a central difference of the function it differentiates.
#[test]
fn the_slope_matches_a_numerical_derivative_of_the_release() {
    let s = safety(P_ONSET_W);
    for t in [430.0, 500.0, 700.0] {
        let h = 1e-4;
        let numeric =
            (reaction_power(&s, t + h, BUDGET_J) - reaction_power(&s, t - h, BUDGET_J)) / (2.0 * h);
        let analytic = reaction_power_slope(&s, t, reaction_power(&s, t, BUDGET_J));
        assert!(
            (numeric - analytic).abs() < 1e-5 * analytic,
            "at {t} K: numeric {numeric}, analytic {analytic}"
        );
    }
}

// --- the budget -----------------------------------------------------------

/// A perfectly isolated cell at onset burns its whole budget and stops, raising its own
/// temperature by exactly `runaway_energy_j / heat_capacity_j_per_k`.
///
/// This is the assertion that says the exponential is bounded. It is also the one that
/// would catch the integrator crediting the cell with heat its budget could not pay for
/// — the failure mode that clipping the release rate *before* the Euler update exists to
/// prevent, and which would otherwise show up only as an energy residual.
#[test]
fn an_isolated_cell_burns_exactly_its_budget_and_stops() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    let mut released_j = 0.0;
    let dt = 0.5;
    for _ in 0..4000 {
        released_j += pack.step(dt, Demand::Rest, &env()).q_runaway_w * dt;
    }
    let expected_rise = BUDGET_J / C_TH;
    assert!(
        budget_left(&pack, 0) < 1e-6 * BUDGET_J,
        "budget left {} J",
        budget_left(&pack, 0)
    );
    assert!(
        (released_j - BUDGET_J).abs() < 1e-6 * BUDGET_J,
        "released {released_j} J against a {BUDGET_J} J budget"
    );
    let rise = temp(&pack, 0) - ONSET_K;
    assert!(
        (rise - expected_rise).abs() < 1e-6 * expected_rise,
        "adiabatic rise {rise} K, expected {expected_rise} K"
    );
}

/// Both flags, and the shape of the trace between them: reacting from the first step,
/// venting later, and `VENTED` still raised long after the cell has burned out and
/// stopped reacting.
#[test]
fn a_burning_cell_flags_runaway_then_vents_and_stays_vented() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    let first = pack.step(0.5, Demand::Rest, &env());
    assert!(
        first.flags.contains(EventFlags::THERMAL_RUNAWAY),
        "the first step of a runaway must flag it: {:?}",
        first.flags
    );
    assert!(
        !first.flags.contains(EventFlags::VENTED),
        "nothing has vented yet: {:?}",
        first.flags
    );

    let vent_time = rest_until(&mut pack, 0.5, 4000, |t| {
        t.flags.contains(EventFlags::VENTED)
    })
    .expect("an adiabatic cell at onset must reach vent");
    assert!(temp(&pack, 0) >= VENT_K);
    assert!(vented(&pack, 0));

    // Burn it out, then keep resting. The reaction is over, the flag is not.
    let last = rest_until(&mut pack, 0.5, 4000, |t| t.q_runaway_w == 0.0)
        .expect("the budget is finite, so the reaction must end");
    assert!(last > vent_time);
    let after = pack.step(0.5, Demand::Rest, &env());
    assert!(!after.flags.contains(EventFlags::THERMAL_RUNAWAY));
    assert!(
        after.flags.contains(EventFlags::VENTED),
        "venting is irreversible: {:?}",
        after.flags
    );
}

// --- the integrator -------------------------------------------------------

/// Simulation times \[s\] at which an adiabatic cell run at `dt` reaches each of
/// `thresholds` (ascending), **linearly interpolated within the step that crosses**.
///
/// The interpolation is what makes the comparison mean anything. Temperature is only
/// observable at step boundaries, so a raw crossing time carries up to a whole `dt` of
/// quantisation — and against a leg lasting tens of seconds that artefact is larger than
/// the integration error being measured. Interpolating removes it to first order, which
/// is ample: within one step the trajectory is smooth.
fn crossing_times(dt: f64, thresholds: &[f64]) -> Vec<f64> {
    let mut pack = adiabatic_cell(P_ONSET_W);
    let mut out = Vec::with_capacity(thresholds.len());
    let mut next = 0;
    let mut prev = (pack.sim_time_s(), temp(&pack, 0));
    for _ in 0..(4000.0 / dt) as usize {
        pack.step(dt, Demand::Rest, &env());
        let now = (pack.sim_time_s(), temp(&pack, 0));
        while next < thresholds.len() && now.1 >= thresholds[next] {
            let span = now.1 - prev.1;
            let frac = if span > 0.0 {
                (thresholds[next] - prev.1) / span
            } else {
                0.0
            };
            out.push(prev.0 + frac * (now.0 - prev.0));
            next += 1;
        }
        if next == thresholds.len() {
            break;
        }
        prev = now;
    }
    assert_eq!(
        out.len(),
        thresholds.len(),
        "dt = {dt} never reached {:?} K",
        thresholds.get(out.len())
    );
    out
}

/// **The test the pre-work required.** An Arrhenius source term retires the linear
/// sub-step bound, so the only honest evidence that the sub-stepping is adequate is that
/// the same trajectory run at two different client `dt` agrees.
///
/// Two things about *what* is compared, both learned the hard way:
///
/// * **Timing, not temperature.** Comparing the two runs' temperature at matched times
///   looks like the obvious check and is useless: in the climb the cell gains ~68 K/s, so
///   a 0.2 % disagreement in timing — which is convergence, not failure — shows up as
///   30 K. An exponential amplifies phase error into amplitude error, so amplitude is the
///   wrong axis.
/// * **The climb's *duration*, not the vent time.** Vent sits only 30 K above onset,
///   where the release is 5–33 W and the sub-step is `dt`-limited in both arms; two `dt`
///   agree there whatever the integrator does. The accuracy cap only starts binding once
///   the release passes ~50 W, i.e. after vent. So the leg from vent up to 600 K is the
///   only window that can discriminate, and its *elapsed time* is the quantity the
///   pre-work was really asking about.
///
/// Both `dt` are clear of the work cap — a full burn of this budget needs ~253 sub-steps
/// against a cap of 2048 — so a disagreement means the *bound* is wrong rather than that
/// the limiter bound. Verified by mutation: widening `MAX_SUBSTEP_RISE_K` from 1 K to
/// 1000 K, which leaves only the linear stability bound that the pre-work proposed, fails
/// this test and nothing else in the file.
#[test]
fn the_climb_takes_the_same_time_at_two_different_timesteps() {
    let thresholds = [VENT_K, 600.0];
    let fine = crossing_times(0.25, &thresholds);
    let coarse = crossing_times(2.0, &thresholds);
    let (fine_leg, coarse_leg) = (fine[1] - fine[0], coarse[1] - coarse[0]);
    let rel = (coarse_leg - fine_leg).abs() / fine_leg;
    // Measured: 0.9 % as shipped, 4.6 % with the accuracy cap removed. The threshold sits
    // between them with room on both sides — tight enough to catch the bound the pre-work
    // proposed, loose enough that ordinary convergence noise does not trip it.
    assert!(
        rel < 0.02,
        "the vent-to-600 K climb takes {fine_leg:.2} s at dt = 0.25 and {coarse_leg:.2} s \
         at dt = 2.0 ({:.1} % apart), so the sub-stepping is not resolving the reaction",
        rel * 100.0
    );
}

/// The same agreement on the quantity the sub-step cap actually protects: the peak
/// temperature after a full burn, which is where the reaction is stiffest and where a
/// too-coarse sub-step would overshoot into nonsense.
#[test]
fn the_peak_temperature_is_the_same_at_two_different_timesteps() {
    let mut peaks = Vec::new();
    for dt in [0.25_f64, 2.0] {
        let mut pack = adiabatic_cell(P_ONSET_W);
        let steps = (4000.0 / dt) as usize;
        for _ in 0..steps {
            pack.step(dt, Demand::Rest, &env());
        }
        assert!(
            budget_left(&pack, 0) < 1e-6 * BUDGET_J,
            "not burned out at dt = {dt}"
        );
        peaks.push(temp(&pack, 0));
    }
    // The budget is what sets the ceiling, so both runs must land on it — this is a
    // check that neither `dt` manufactured or lost energy on the way up.
    let expected = ONSET_K + BUDGET_J / C_TH;
    for (dt, peak) in [0.25, 2.0].iter().zip(&peaks) {
        assert!(
            (peak - expected).abs() < 1e-6 * expected,
            "dt = {dt} peaked at {peak} K, expected {expected} K"
        );
    }
}

/// A whole burn inside a single coarse step must not hit the sub-step work cap.
///
/// This is the check the pre-work demanded before the cap could be trusted: a runaway
/// completes in seconds of simulation time, so at a coarse client `dt` the entire event
/// lands inside one `Pack::step` and the cap — not `dt` — is what bounds the sub-step
/// count. `MAX_RUNAWAY_SUBSTEPS` carries a `debug_assert`, and tests run in debug, so a
/// cap that bound would fail this test by panicking rather than by silently producing a
/// plausible-looking wrong number.
///
/// The budget assertion is the second half: hitting the cap would finish the step in one
/// unbounded Euler jump, and the energy accounting is what would notice.
#[test]
fn a_whole_burn_fits_inside_one_coarse_step() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    let dt = 60.0;
    let mut released_j = 0.0;
    for _ in 0..40 {
        released_j += pack.step(dt, Demand::Rest, &env()).q_runaway_w * dt;
    }
    assert!(
        (released_j - BUDGET_J).abs() < 1e-6 * BUDGET_J,
        "released {released_j} J against a {BUDGET_J} J budget at dt = {dt}"
    );
    let expected = ONSET_K + BUDGET_J / C_TH;
    assert!(
        (temp(&pack, 0) - expected).abs() < 1e-6 * expected,
        "peaked at {} K, expected {expected} K",
        temp(&pack, 0)
    );
}

// --- what does *not* burn -------------------------------------------------

/// A pack whose cells never reach onset is bit-for-bit the pack it would have been
/// before this slice existed. The gate is one comparison per cell; nothing else about
/// the trajectory may move.
#[test]
fn a_pack_below_onset_is_unchanged_by_the_reaction_being_available() {
    let build = |p_onset: f64| {
        Pack::new(&cfg(3, AMBIENT_K, 0.5), chem(Some(safety(p_onset)), 0.35))
            .expect("fixture builds")
    };
    let (mut live, mut inert) = (build(P_ONSET_W), build(0.0));
    for _ in 0..500 {
        let a = live.step(0.5, Demand::Current(20.0), &env());
        let b = inert.step(0.5, Demand::Current(20.0), &env());
        assert_eq!(a.v_terminal.to_bits(), b.v_terminal.to_bits());
        assert_eq!(a.t_max.to_bits(), b.t_max.to_bits());
        assert_eq!(a.q_gen_w.to_bits(), b.q_gen_w.to_bits());
        assert_eq!(a.flags, b.flags);
    }
    assert!(
        temp(&live, 1) < ONSET_K,
        "the fixture was supposed to stay cool"
    );
}

/// An isothermal pack never reacts, because the reaction is a feedback on a temperature
/// that mode does not move. Venting is the exception: it is an observation about
/// temperature and holds in both modes, so a pack *built* above the vent threshold says
/// so — while still never burning a joule of its budget.
#[test]
fn an_isothermal_pack_reports_venting_but_never_reacts() {
    let mut config = cfg(1, VENT_K + 10.0, 0.0);
    config.thermal = ThermalConfig::Isothermal;
    let mut pack = Pack::new(&config, chem(Some(safety(P_ONSET_W)), 0.35)).expect("builds");
    let tele = pack.step(1.0, Demand::Rest, &env());
    assert!(tele.flags.contains(EventFlags::VENTED));
    assert!(!tele.flags.contains(EventFlags::THERMAL_RUNAWAY));
    assert_eq!(tele.q_runaway_w, 0.0);
    assert_eq!(budget_left(&pack, 0), BUDGET_J);
}

/// A chemistry with no `[safety]` section cannot react, vent, or carry a budget — the
/// same "absent means disabled" rule plating follows, and not the build error `[aging]`
/// raises. Nothing in the config asked for runaway, so there is no request for the
/// silence to contradict.
#[test]
fn a_chemistry_without_safety_carries_no_budget() {
    let mut pack = Pack::new(&cfg(1, 900.0, 0.0), chem(None, 0.0)).expect("builds");
    let tele = pack.step(1.0, Demand::Rest, &env());
    assert!(!tele
        .flags
        .intersects(EventFlags::VENTED | EventFlags::THERMAL_RUNAWAY));
    assert_eq!(budget_left(&pack, 0), 0.0);
    assert!(!vented(&pack, 0));
}

/// Runaway does not ride the aging sub-clock, unlike plating's consequences. A pack with
/// `aging: None` can never wear out and must still be able to burn — `CLAUDE.md`'s exit
/// criterion says nothing about aging, and a client that switched aging off to hold a
/// scenario fixed has not asked to be made fireproof.
#[test]
fn a_pack_that_cannot_age_can_still_burn() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    assert!(pack.aging().is_none(), "the fixture must have aging off");
    rest_until(&mut pack, 0.5, 4000, |t| {
        t.flags.contains(EventFlags::VENTED)
    })
    .expect("a pack without aging must still be able to vent");
}

// --- the step contract ----------------------------------------------------

/// A zero-length probe step reports the reaction it finds and releases nothing, leaving
/// the budget and the temperature exactly as it found them. Same contract the BMS sensor
/// clock, the aging sub-clock and the fault queue all take.
#[test]
fn a_zero_length_step_reports_the_reaction_without_advancing_it() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    let before = pack.snapshot();
    let probe = pack.step(0.0, Demand::Rest, &env());
    assert!(probe.flags.contains(EventFlags::THERMAL_RUNAWAY));
    // At dt = 0 the reported power is the instantaneous rate, which at onset with a full
    // budget is exactly the amplitude — one definition, shared with `reaction_power`.
    assert_eq!(probe.q_runaway_w, P_ONSET_W);
    assert_eq!(budget_left(&pack, 0), BUDGET_J);
    assert_eq!(temp(&pack, 0), ONSET_K);
    assert_eq!(pack.snapshot(), before, "a probe step mutated the pack");
}

/// The same contract for the vent latch specifically, which is the one place in this
/// slice where an *observation* would otherwise write irreversible state.
///
/// A probe on a pack that is already past the vent threshold must report `VENTED` — it is
/// true, and a client asking "what is the pack doing right now" deserves the answer — and
/// must not latch it. Otherwise the act of looking at a hot pack changes what a snapshot
/// taken afterwards contains, and the trajectory would depend on how often a client
/// probed. The reporting-pass test above cannot catch this: it starts at onset, thirty
/// kelvin below vent.
#[test]
fn a_zero_length_step_reports_venting_without_latching_it() {
    let mut pack = Pack::new(
        &cfg(1, VENT_K + 10.0, 0.0),
        chem(Some(safety(P_ONSET_W)), 0.0),
    )
    .expect("builds");
    let before = pack.snapshot();
    let probe = pack.step(0.0, Demand::Rest, &env());
    assert!(
        probe.flags.contains(EventFlags::VENTED),
        "a probe must still report the pack it finds: {:?}",
        probe.flags
    );
    assert!(!vented(&pack, 0), "a probe step latched the vent bit");
    assert_eq!(pack.snapshot(), before, "a probe step mutated the pack");

    // And the first step that does advance time latches it for good.
    pack.step(1.0, Demand::Rest, &env());
    assert!(vented(&pack, 0));
}

/// Mid-burn state survives a snapshot: the unreleased half of the budget and the vent
/// latch are both genuine state, and a client that could wash either out by saving and
/// reloading would be able to put out a fire with a save file.
#[test]
fn a_snapshot_taken_mid_burn_replays_bit_identically() {
    let mut pack = adiabatic_cell(P_ONSET_W);
    for _ in 0..600 {
        pack.step(0.5, Demand::Rest, &env());
    }
    let partly_burned = budget_left(&pack, 0);
    assert!(
        partly_burned > 0.0 && partly_burned < BUDGET_J,
        "the fixture must be caught mid-burn, budget left {partly_burned} J"
    );

    let snap = pack.snapshot();
    let mut restored = Pack::restore(&snap).expect("round-trips");
    for _ in 0..600 {
        let a = pack.step(0.5, Demand::Rest, &env());
        let b = restored.step(0.5, Demand::Rest, &env());
        assert_eq!(a.t_max.to_bits(), b.t_max.to_bits());
        assert_eq!(a.q_runaway_w.to_bits(), b.q_runaway_w.to_bits());
        assert_eq!(a.flags, b.flags);
    }
    assert_eq!(
        budget_left(&pack, 0).to_bits(),
        budget_left(&restored, 0).to_bits()
    );
}

// --- propagation ----------------------------------------------------------

/// Build the propagation fixture: a 2S1P pack with a real convective sink, and an
/// injected soft short across cell 0 that heats it.
///
/// Fault injection is the only sanctioned override (`CLAUDE.md`), and it is used here
/// only to *start* the fire — everything after cell 0 crosses onset is the physics. The
/// short's steady state without any reaction leaves cell 0 hot and cell 1 well below
/// onset, which is what makes the control run below a real control.
fn propagation_pack(runaway_power_w_at_onset: f64) -> Pack {
    let mut pack = Pack::new(
        &cfg(2, AMBIENT_K, 0.5),
        chem(Some(safety(runaway_power_w_at_onset)), 0.35),
    )
    .expect("fixture builds");
    pack.schedule_fault(
        0.0,
        Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 0.1,
        },
    )
    .expect("a positive resistance on an in-range cell");
    pack
}

/// **The propagation half of the phase exit criterion.** A cell driven into runaway
/// takes its neighbour with it, through nothing but the `k_ij` conductance the thermal
/// network has had since Phase 2.
///
/// The control is what gives this teeth: the identical pack with the reaction amplitude
/// set to zero runs the identical heater and the neighbour never gets near onset. So the
/// second cell burning is attributable to the first cell's reaction, and not to the
/// injected short that lit it.
#[test]
fn a_burning_cell_takes_its_neighbour_with_it() {
    const DT: f64 = 1.0;
    const STEPS: usize = 12_000; // 200 minutes, several thermal time constants

    let mut live = propagation_pack(P_ONSET_W);
    for _ in 0..STEPS {
        live.step(DT, Demand::Rest, &env());
    }
    assert!(vented(&live, 0), "the heated cell should have vented");
    assert!(
        vented(&live, 1),
        "runaway did not propagate: neighbour reached {} K",
        temp(&live, 1)
    );
    assert!(
        budget_left(&live, 1) < BUDGET_J,
        "the neighbour vented without reacting, so it was heated rather than ignited"
    );

    let mut control = propagation_pack(0.0);
    for _ in 0..STEPS {
        control.step(DT, Demand::Rest, &env());
    }
    assert!(
        temp(&control, 1) < ONSET_K,
        "the control's neighbour reached {} K on the heater alone, so the live run \
         proves nothing about propagation",
        temp(&control, 1)
    );
}
