//! The Doyle–Fuller–Newman cell driven against the **shipped** LG M50 chemistry.
//!
//! # Why these live in `sim-data` rather than in `sim-core`
//! The same reason `spm_pack.rs` gives, only more so: `sim-core` cannot read a file, and a
//! DFN needs *both* the `[spm]` and `[dfn]` sections of
//! `chemistries/nmc_21700_lgm50.toml` — two OCP tables, the electrode geometry and
//! kinetics, and the electrolyte fits. A hand-built fixture for that is a benchmark of a
//! typo waiting to happen, and every claim below is about the shipped file specifically.
//!
//! Reaching into cell state goes through a snapshot and `serde_json`, because `Pack`
//! deliberately exposes no accessor onto [`sim_core::CellModel`] — that absence is what
//! `CLAUDE.md`'s "nothing outside the enum assumes cell internals" is enforced by, and a
//! test is not a reason to open it.

use sim_core::dfn::{self, probe};
use sim_core::{
    BuildError, CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, Telemetry,
    ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

/// Nominal capacity of the shipped cell \[A·h\], so a C-rate below is written as one.
///
/// Not `5.0`. `CLAUDE.md`'s parameter block is a *shape*, not a source, and this file's
/// `capacity_ah` is 5.153198 — a lesson already paid for once (see the guided-path slice).
const CAPACITY_AH: f64 = 5.153198;

/// Discharge cut-off \[V\] for the shipped cell.
const V_CUT: f64 = 2.5;

/// The grid every test here runs on unless it is measuring the grid. 10/5/10 with
/// `N_r = 10` is what the Phase 7 spike's convergence and cost tables were measured at.
const NODES: (usize, usize, usize) = (10, 5, 10);
const SHELLS: usize = 10;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn dfn_model(nodes: (usize, usize, usize), shells: usize) -> CellModelConfig {
    CellModelConfig::Dfn {
        shells,
        nodes_negative: nodes.0,
        nodes_separator: nodes.1,
        nodes_positive: nodes.2,
    }
}

fn cfg(model: CellModelConfig, initial_soc: f64) -> PackConfig {
    PackConfig {
        series: 1,
        parallel: 1,
        initial_soc,
        initial_temp_k: 298.15,
        seed: 1,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: None,
        aging: None,
        cell_model: model,
    }
}

fn pack(model: CellModelConfig, soc: f64) -> Pack {
    Pack::new(
        &cfg(model, soc),
        parse_chemistry(LGM50).expect("LG M50 parses"),
    )
    .expect("pack builds")
}

/// The whole snapshot as JSON, which is the only way into cell state from outside the
/// crate. `serde_json` carries full float precision here (the workspace enables
/// `float_roundtrip`), so a value read back is the value stored.
fn snapshot_json(p: &Pack) -> serde_json::Value {
    serde_json::to_value(p.snapshot()).expect("snapshot serializes")
}

/// The first cell's DFN state.
fn dfn_state(p: &Pack) -> sim_core::DfnState {
    serde_json::from_value(
        snapshot_json(p)["pack"]["groups"][0]["cells"][0]["model"]["Dfn"].clone(),
    )
    .expect("cell 0 is a DFN cell")
}

/// Run to the voltage cut-off (or `max_steps`), returning `(seconds, amp-hours, min c_e,
/// unconverged step count, last telemetry)`.
fn run_to_cutoff(
    p: &mut Pack,
    i: f64,
    dt: f64,
    max_steps: usize,
) -> (f64, f64, f64, u32, Telemetry) {
    let mut t = 0.0;
    let mut ah = 0.0;
    let mut min_ce = f64::INFINITY;
    let mut unconverged = 0;
    let mut last = p.step(0.0, Demand::Current(i), &env());
    for _ in 0..max_steps {
        last = p.step(dt, Demand::Current(i), &env());
        t += dt;
        ah += i * dt / 3600.0;
        for c in dfn_state(p).c_e {
            min_ce = min_ce.min(c);
        }
        if last.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
            unconverged += 1;
        }
        if !last.v_terminal.is_finite() || last.v_terminal <= V_CUT {
            break;
        }
    }
    (t, ah, min_ce, unconverged, last)
}

// ---------------------------------------------------------------------------
// Build-time refusals
// ---------------------------------------------------------------------------

/// A DFN needs **both** sections, and the error says which one is missing.
///
/// The two halves fail differently on purpose. A chemistry with neither (the shipped LFP)
/// is not a porous-electrode parameter set at all and is told so in `[spm]`'s terms; one
/// with `[spm]` and no `[dfn]` describes its electrodes fully and is one electrolyte block
/// short. A single "invalid config" would cost its reader an hour deciding which.
#[test]
fn a_dfn_pack_refuses_a_chemistry_missing_either_section() {
    let lfp = parse_chemistry(LFP).expect("LFP parses");
    assert!(lfp.spm.is_none() && lfp.dfn.is_none());
    match Pack::new(&cfg(dfn_model(NODES, SHELLS), 1.0), lfp) {
        Err(BuildError::MissingSpmParams { chem_id }) => assert_eq!(chem_id, "lfp_26650_generic"),
        other => panic!("expected MissingSpmParams, got {other:?}"),
    }

    let mut half = parse_chemistry(LGM50).expect("LG M50 parses");
    assert!(half.spm.is_some(), "the shipped file has an [spm] section");
    half.dfn = None;
    match Pack::new(&cfg(dfn_model(NODES, SHELLS), 1.0), half) {
        Err(BuildError::MissingDfnParams { chem_id }) => assert_eq!(chem_id, "nmc_21700_lgm50"),
        other => panic!("expected MissingDfnParams, got {other:?}"),
    }
}

/// Every discretisation count is bracketed, and the error names which one.
#[test]
fn the_discretisation_counts_are_bracketed() {
    let chem = || parse_chemistry(LGM50).expect("LG M50 parses");
    let too_many = dfn::MAX_NODES + 1;
    for (model, region, nodes) in [
        (dfn_model((0, 5, 10), SHELLS), "negative", 0),
        (dfn_model((10, 0, 10), SHELLS), "separator", 0),
        (dfn_model((10, 5, too_many), SHELLS), "positive", too_many),
    ] {
        match Pack::new(&cfg(model, 1.0), chem()) {
            Err(BuildError::BadNodeCount {
                region: r,
                nodes: n,
                min,
                max,
            }) => {
                assert_eq!((r, n), (region, nodes));
                assert_eq!((min, max), (dfn::MIN_NODES, dfn::MAX_NODES));
            }
            other => panic!("expected BadNodeCount for {region}, got {other:?}"),
        }
    }
    // Shells are bracketed by the *same* rule the SPM's are, because it is the same
    // discretisation of the same particle.
    match Pack::new(&cfg(dfn_model(NODES, 1), 1.0), chem()) {
        Err(BuildError::BadShellCount { shells, .. }) => assert_eq!(shells, 1),
        other => panic!("expected BadShellCount, got {other:?}"),
    }
    // And the minimum node count really is buildable rather than merely documented.
    assert!(Pack::new(&cfg(dfn_model((1, 1, 1), 2), 1.0), chem()).is_ok());
}

/// The new model is selectable from a **scenario file**, which is the only surface any
/// client has for choosing one.
///
/// The Phase 7 plan asserts this rather than testing it — "`CellModelConfig::Dfn` is an
/// enum *variant* on an existing field, which is reachable from scenario TOML, so new DFN
/// cases are additive and valid from slice B forward". It is the claim the out-of-tree
/// trajectory instrument's coverage of this phase rests on, since that instrument builds
/// every case through `parse_scenario`, so it is worth a test rather than a sentence.
/// Note what it also pins: the four field names, which are a file-format contract the
/// moment a scenario names them.
#[test]
fn a_dfn_pack_is_selectable_from_a_scenario_file() {
    let scenario = sim_data::parse_scenario(
        r#"
        chemistry = "nmc_21700_lgm50"

        [meta]
        name = "dfn reachability"
        description = "Selecting the DFN from a scenario file, and nothing else."

        [pack]
        series = 1
        parallel = 1
        initial_soc = 0.9
        initial_temp_k = 298.15
        seed = 1

        [pack.cell_model.Dfn]
        shells = 10
        nodes_negative = 10
        nodes_separator = 5
        nodes_positive = 10
        "#,
    )
    .expect("a scenario may select Dfn");
    assert_eq!(
        scenario.pack.cell_model,
        dfn_model((10, 5, 10), 10),
        "the scenario's four node/shell fields must land where the config expects them"
    );
}

// ---------------------------------------------------------------------------
// The model runs, and does the thing an SPM cannot
// ---------------------------------------------------------------------------

/// A 1C discharge of the shipped cell reaches the cut-off where the spike said it would,
/// with an electrolyte gradient that an SPM has no way to represent.
///
/// The time window is deliberately wide: this is a "the model is wired up and the physics
/// is the right size" assertion, not a golden. Phase 7 slice D commits a PyBaMM DFN
/// reference and a tolerance built to fail. What is pinned here is that the run ends by
/// running out of lithium at roughly the right moment (PyBaMM's DFN: 3555 s, 4.938 A·h)
/// and that every step converged.
#[test]
fn the_shipped_dfn_cell_discharges_at_one_c() {
    let mut p = pack(dfn_model(NODES, SHELLS), 1.0);
    let (t, ah, min_ce, unconverged, last) = run_to_cutoff(&mut p, CAPACITY_AH, 2.0, 5_000);
    assert_eq!(unconverged, 0, "no 1C step should fail to converge");
    assert!(
        (3_000.0..4_000.0).contains(&t),
        "1C cut-off at {t} s is nowhere near the reference's 3555 s"
    );
    assert!(
        (4.5..5.4).contains(&ah),
        "1C delivered {ah} A·h against the reference's 4.94"
    );
    assert!(last.v_terminal <= V_CUT);
    assert!(
        last.soc_true < 0.1,
        "a full discharge should empty the cell"
    );

    // The gradient itself: a concentration spread across x is precisely the quantity the
    // single-particle approximation sets to zero.
    let c_e = dfn_state(&p).c_e;
    let lo = c_e.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = c_e.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        hi - lo > 100.0,
        "the electrolyte should be visibly graded at 1C, spread was {} mol/m3",
        hi - lo
    );
    assert!(min_ce > 400.0, "1C should not come close to depletion");
}

/// The cliff, which is the reason the phase exists: at 3C the DFN starves and delivers
/// materially less than the same cell modelled as an SPM.
///
/// The SPM comparison is what makes this a statement about the electrolyte rather than
/// about the discretisation — both models read the same `[spm]` block, the same OCP
/// tables and the same particle solver, and the *only* difference is whether the
/// electrolyte is solved for.
#[test]
fn the_electrolyte_starves_at_three_c_and_an_spm_never_notices() {
    let mut d = pack(dfn_model(NODES, SHELLS), 1.0);
    let (t_dfn, ah_dfn, min_ce, unconverged, _) =
        run_to_cutoff(&mut d, 3.0 * CAPACITY_AH, 2.0, 5_000);
    assert_eq!(unconverged, 0, "no 3C step should fail to converge either");

    let mut s = pack(CellModelConfig::Spm { shells: SHELLS }, 1.0);
    let mut t_spm = 0.0;
    let mut ah_spm = 0.0;
    for _ in 0..5_000 {
        let tm = s.step(2.0, Demand::Current(3.0 * CAPACITY_AH), &env());
        t_spm += 2.0;
        ah_spm += 3.0 * CAPACITY_AH * 2.0 / 3600.0;
        if !tm.v_terminal.is_finite() || tm.v_terminal <= V_CUT {
            break;
        }
    }

    assert!(
        ah_dfn < 0.8 * ah_spm,
        "the DFN delivered {ah_dfn} A·h against the SPM's {ah_spm}: no cliff"
    );
    assert!(
        t_dfn < 0.8 * t_spm,
        "the DFN lasted {t_dfn} s against the SPM's {t_spm}: no cliff"
    );
    assert!(
        min_ce < 1.0,
        "3C should drive the electrolyte to depletion; the minimum was {min_ce} mol/m3"
    );
}

// ---------------------------------------------------------------------------
// The `c_e` floor
// ---------------------------------------------------------------------------

/// The lookup floor is **inert at 1C and live at 3C**, which is the pair of facts that
/// makes its value defensible.
///
/// # Why this shape and not a sweep
/// The obvious test — run twice with two floors and assert the answers agree — is green
/// whenever the run never approaches the floor at all, which is the same failure as an
/// assertion measuring the artefact it was written to bound. And the floor is a `const`,
/// so a sweep is not available to a test anyway.
///
/// What *is* available is stronger. If the minimum concentration a 1C run ever reaches is
/// four orders of magnitude above the floor, then no lookup on that run was floored, and
/// the constant is inert there **whatever its value below that minimum** — a claim a
/// two-point sweep could only sample. The 3C half then shows the constant is not merely
/// unreachable: it is reached, it is doing work, and the spike's 0.72 A·h measurement of
/// what a careless value costs applies to a regime this engine actually enters.
#[test]
fn the_electrolyte_floor_is_inert_at_one_c_and_live_at_three_c() {
    let floor = dfn::C_E_FLOOR_MOL_PER_M3;

    let mut slow = pack(dfn_model(NODES, SHELLS), 1.0);
    let (_, _, min_slow, _, _) = run_to_cutoff(&mut slow, CAPACITY_AH, 2.0, 5_000);
    assert!(
        min_slow > 1_000.0 * floor,
        "at 1C the minimum c_e was {min_slow} mol/m3, only {}x the floor — the floor is \
         no longer provably inert at this rate and this test no longer says what it says",
        min_slow / floor
    );

    let mut fast = pack(dfn_model(NODES, SHELLS), 1.0);
    let (_, _, min_fast, _, _) = run_to_cutoff(&mut fast, 3.0 * CAPACITY_AH, 2.0, 5_000);
    assert!(
        min_fast < floor,
        "at 3C the minimum c_e was {min_fast} mol/m3, above the floor — the guard is \
         never exercised and its value is untested by anything here"
    );
}

// ---------------------------------------------------------------------------
// The analytic Jacobian
// ---------------------------------------------------------------------------

/// The analytic Jacobian agrees with a central difference of the residual it claims to
/// differentiate.
///
/// # Kinks, and why the states are chosen rather than swept
/// Three places make the residual non-differentiable: the surface clamp inside the
/// kinetics, the `c_e` lookup floor, and every breakpoint of the two OCP tables. At each
/// one the analytic derivative takes a branch and a central difference straddles it, so a
/// disagreement there is arithmetic rather than a bug. The states below are therefore
/// chosen away from the floor (asserted, not assumed — see the `min c_e` check) and the
/// comparison is row-relative, because the rows carry units that differ by ~1e5.
///
/// An OCP breakpoint can still be straddled by chance, which is what the tolerance's
/// distance from the observed agreement is for: the worst disagreement measured across
/// these states is ~1e-9, and `1e-5` would still catch a wrong sign, a missing term or a
/// transposed index — the failures this test exists for.
#[test]
fn the_analytic_jacobian_matches_a_difference_quotient() {
    let chem = parse_chemistry(LGM50).expect("LG M50 parses");
    let spm = chem.spm.as_ref().expect("[spm]");
    let d = chem.dfn.as_ref().expect("[dfn]");
    let capacity = probe::spm_capacity(spm);

    for (label, steps, i, dt) in [
        ("fresh, discharge", 0, CAPACITY_AH, 10.0),
        ("mid-discharge", 20, CAPACITY_AH, 10.0),
        ("charging", 20, -CAPACITY_AH, 10.0),
        ("at rest", 20, 0.0, 10.0),
        ("hard discharge", 20, 3.0 * CAPACITY_AH, 5.0),
    ] {
        let mut p = pack(dfn_model((6, 3, 6), 8), 1.0);
        for _ in 0..steps {
            p.step(dt, Demand::Current(i), &env());
        }
        let s = dfn_state(&p);
        let min_ce = s.c_e.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(
            min_ce > 10.0 * dfn::C_E_FLOOR_MOL_PER_M3,
            "{label}: this state has c_e down to {min_ce}, at the floor's kink — the \
             comparison below would be measuring the branch, not the Jacobian"
        );

        let (m, analytic, numeric) = probe::jacobian_pair(&s, spm, d, i, dt, 1.0e-6);
        assert_eq!(m, 4 * (6 + 3 + 6));
        let mut worst = 0.0_f64;
        for row in 0..m {
            let scale = (0..m)
                .map(|c| analytic[row * m + c].abs().max(numeric[row * m + c].abs()))
                .fold(1.0e-30_f64, f64::max);
            for col in 0..m {
                worst = worst.max((analytic[row * m + col] - numeric[row * m + col]).abs() / scale);
            }
        }
        assert!(
            worst < 1.0e-5,
            "{label}: worst row-relative Jacobian disagreement {worst:.3e}"
        );
    }
    assert!(capacity > 0.0);

    // Past the floor the two are *not* comparable — the analytic derivative is the branch
    // and the difference quotient straddles it — so what a depleted state owes is the
    // weaker property that actually matters there: every entry finite. A NaN in the
    // Jacobian is how a floored transport term or a `0·x^-1` would announce itself, and it
    // would announce itself as a step that silently stopped converging.
    let mut deep = pack(dfn_model((6, 3, 6), 8), 1.0);
    for _ in 0..120 {
        deep.step(5.0, Demand::Current(3.0 * CAPACITY_AH), &env());
    }
    let s = dfn_state(&deep);
    let min_ce = s.c_e.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        min_ce < dfn::C_E_FLOOR_MOL_PER_M3,
        "this state was meant to be past the floor; its minimum c_e is {min_ce}"
    );
    let (_, analytic, _) = probe::jacobian_pair(&s, spm, d, 3.0 * CAPACITY_AH, 5.0, 1.0e-6);
    assert!(
        analytic.iter().all(|v| v.is_finite()),
        "the Jacobian of a depleted cell contains a non-finite entry"
    );
}

// ---------------------------------------------------------------------------
// Snapshots — exit criterion 3
// ---------------------------------------------------------------------------

/// Telemetry compared by bits, so that "identical" means identical and not "close".
fn same_bits(a: &Telemetry, b: &Telemetry) -> bool {
    let key = |t: &Telemetry| {
        (
            t.v_terminal.to_bits(),
            t.i_actual.to_bits(),
            t.soc_true.to_bits(),
            t.v_cell_min.to_bits(),
            t.v_cell_max.to_bits(),
            t.q_gen_w.to_bits(),
            t.flags,
        )
    };
    key(a) == key(b)
}

/// Snapshot at t/2, restore, continue: the two telemetry streams are bit-identical.
#[test]
fn a_restored_dfn_pack_continues_bit_identically() {
    let mut original = pack(dfn_model(NODES, SHELLS), 0.9);
    for _ in 0..40 {
        original.step(5.0, Demand::Current(CAPACITY_AH), &env());
    }
    let snapshot = original.snapshot();
    let mut restored = Pack::restore(&snapshot).expect("restores at the current version");

    for k in 0..40 {
        let a = original.step(5.0, Demand::Current(CAPACITY_AH), &env());
        let b = restored.step(5.0, Demand::Current(CAPACITY_AH), &env());
        assert!(same_bits(&a, &b), "trajectories parted at step {k}");
    }
}

/// The Newton warm start is **state**, and this test is built to fail without it.
///
/// # Why the obvious test proves nothing
/// "Snapshot, restore, continue, compare" passes whether or not `u` is in the snapshot: if
/// it were dropped, `Pack::restore` would rebuild a cell whose Newton starts somewhere
/// else, and the two runs would agree to the solver tolerance — which on most steps looks
/// exactly like agreement. So this test *removes* it: the snapshot's warm-start vectors
/// are zeroed in the serialized form, restored, and continued. If a cold start were
/// harmless, the continuation would still match and this assertion would fail.
///
/// It does not match, which is the whole argument for the field. A Newton that stops at a
/// tolerance lands where it started from, so the starting point decides the trajectory at
/// the [`sim_core::dfn::NEWTON_TOL`] level — and a snapshot that forgets it continues a
/// different run while looking like it continued the same one.
#[test]
fn zeroing_the_warm_start_moves_the_trajectory() {
    let mut original = pack(dfn_model(NODES, SHELLS), 0.9);
    for _ in 0..40 {
        original.step(5.0, Demand::Current(CAPACITY_AH), &env());
    }

    let mut json = snapshot_json(&original);
    let u = &mut json["pack"]["groups"][0]["cells"][0]["model"]["Dfn"]["u"];
    let len = u.as_array().expect("u is an array").len();
    assert!(len > 0, "the warm start must be in the snapshot at all");
    *u = serde_json::Value::Array(vec![serde_json::json!(0.0); len]);
    let cold: sim_core::Snapshot = serde_json::from_value(json).expect("still a valid snapshot");

    let mut warm = Pack::restore(&original.snapshot()).expect("restores");
    let mut chilled = Pack::restore(&cold).expect("restores");
    let mut parted = false;
    for _ in 0..40 {
        let a = warm.step(5.0, Demand::Current(CAPACITY_AH), &env());
        let b = chilled.step(5.0, Demand::Current(CAPACITY_AH), &env());
        if !same_bits(&a, &b) {
            parted = true;
            break;
        }
    }
    assert!(
        parted,
        "a cold-started Newton produced a bit-identical trajectory, so this test cannot \
         tell whether the warm start is in the snapshot"
    );
}

/// A zero-length step is an observation, not a tick: it must leave the cell exactly as it
/// found it, including the tangent and the warm start.
#[test]
fn a_zero_length_step_does_not_mutate_a_dfn_cell() {
    let mut p = pack(dfn_model(NODES, SHELLS), 0.8);
    for _ in 0..10 {
        p.step(5.0, Demand::Current(CAPACITY_AH), &env());
    }
    let before = p.snapshot();
    p.step(0.0, Demand::Current(3.0 * CAPACITY_AH), &env());
    assert_eq!(
        serde_json::to_string(&before).unwrap(),
        serde_json::to_string(&p.snapshot()).unwrap(),
        "a probe step moved the cell"
    );
}

// ---------------------------------------------------------------------------
// What the pack sees in this slice
// ---------------------------------------------------------------------------

/// A fresh DFN cell has no tangent and reports a seed; after one step it reports the line
/// its own solve produced.
///
/// The seed is a first-order estimate and is labelled one — what is pinned here is that it
/// is *replaced*, and that it was in the right order of magnitude to begin with, so a
/// first step is not a discontinuity dressed as physics.
#[test]
fn the_tangent_is_state_and_the_seed_is_only_ever_the_first_answer() {
    let mut p = pack(dfn_model(NODES, SHELLS), 0.9);
    assert!(
        dfn_state(&p).tangent.is_none(),
        "a cell that has never solved has no tangent to report"
    );
    let first = p.step(0.0, Demand::Current(0.0), &env());
    // At rest and unsolved, the seed's `E` is the equilibrium voltage, so a zero-current
    // probe reads the open-circuit voltage exactly.
    assert!(
        (3.5..4.3).contains(&first.v_terminal),
        "seeded open-circuit voltage {} V is not a charged LG M50",
        first.v_terminal
    );

    p.step(5.0, Demand::Current(CAPACITY_AH), &env());
    let (e, r) = dfn_state(&p).tangent.expect("a solved cell has a tangent");
    assert!(
        r > 0.0 && r.is_finite(),
        "tangent resistance {r} is not usable"
    );
    assert!(
        (0.005..0.5).contains(&r),
        "a 5 A·h cell's differential resistance should be tens of milliohms, got {r} ohm"
    );
    assert!(
        (3.0..4.3).contains(&e),
        "tangent intercept {e} V is implausible"
    );
}

/// A DFN pack reports `solve_iterations == 1`, and that number is about **this slice**
/// rather than about the physics.
///
/// [`sim_core::CellModel::is_linear`] answers `false` for a DFN, so the pack's nonlinear
/// iteration really does run — and exits on its first pass, because the curve it measures
/// its aggregate against is the same line it aggregated. Slice C is what gives those
/// passes something to do; pinning the number here is what will make that change visible
/// rather than silent.
#[test]
fn the_pack_solve_has_nothing_to_chase_yet() {
    let mut p = pack(dfn_model(NODES, SHELLS), 0.9);
    for _ in 0..20 {
        let t = p.step(5.0, Demand::Current(CAPACITY_AH), &env());
        assert_eq!(t.solve_iterations, 1);
        assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    }
}

/// Charge in, charge out: driving the cell down and back returns the lithium to the
/// particles, which is the conservation statement a finite-volume scheme owes.
#[test]
fn a_discharge_and_recharge_returns_the_lithium() {
    let mut p = pack(dfn_model(NODES, SHELLS), 0.6);
    let soc0 = p.step(0.0, Demand::Rest, &env()).soc_true;
    for _ in 0..60 {
        p.step(5.0, Demand::Current(CAPACITY_AH), &env());
    }
    let low = p.step(0.0, Demand::Rest, &env()).soc_true;
    for _ in 0..60 {
        p.step(5.0, Demand::Current(-CAPACITY_AH), &env());
    }
    let back = p.step(0.0, Demand::Rest, &env()).soc_true;
    assert!(low < soc0 - 0.05, "the discharge did not move the cell");
    assert!(
        (back - soc0).abs() < 1.0e-3,
        "SOC returned to {back} from {soc0} after a symmetric cycle"
    );
}
