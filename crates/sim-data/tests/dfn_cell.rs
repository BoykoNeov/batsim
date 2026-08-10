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

/// What a run to cut-off observed. A struct rather than the tuple this was, because the
/// solid-phase bounds below are a conservation claim several tests need and a sixth and
/// seventh tuple slot stops being readable.
struct Run {
    /// Elapsed simulated time \[s\].
    t: f64,
    /// Charge removed \[A·h\].
    ah: f64,
    /// Lowest electrolyte concentration seen at any node, at any step \[mol/m³\].
    min_ce: f64,
    /// Steps that raised [`EventFlags::SOLVE_UNCONVERGED`].
    unconverged: u32,
    /// Whether the *only* unconverged step, if any, was the final one.
    unconverged_only_last: bool,
    /// Highest solid concentration in any shell of any particle, as a fraction of that
    /// electrode's `c_max`. Physically this can never exceed 1.
    peak_solid_fraction: f64,
    /// Lowest solid concentration in any shell of any particle, as a fraction of `c_max`.
    /// Physically this can never go below 0.
    min_solid_fraction: f64,
    /// Telemetry from the last step taken.
    last: Telemetry,
}

/// Run to the voltage cut-off (or `max_steps`).
fn run_to_cutoff(p: &mut Pack, i: f64, dt: f64, max_steps: usize) -> Run {
    let chem = parse_chemistry(LGM50).expect("LG M50 parses");
    let spm = chem
        .spm
        .as_ref()
        .expect("the LG M50 file has an [spm] section");
    let (c_max_neg, c_max_pos) = (spm.negative.c_max_mol_per_m3, spm.positive.c_max_mol_per_m3);

    let mut r = Run {
        t: 0.0,
        ah: 0.0,
        min_ce: f64::INFINITY,
        unconverged: 0,
        unconverged_only_last: true,
        peak_solid_fraction: f64::NEG_INFINITY,
        min_solid_fraction: f64::INFINITY,
        last: p.step(0.0, Demand::Current(i), &env()),
    };
    for _ in 0..max_steps {
        r.last = p.step(dt, Demand::Current(i), &env());
        r.t += dt;
        r.ah += i * dt / 3600.0;
        let st = dfn_state(p);
        for c in st.c_e {
            r.min_ce = r.min_ce.min(c);
        }
        for (profiles, c_max) in [(&st.c_neg, c_max_neg), (&st.c_pos, c_max_pos)] {
            for prof in profiles {
                for &c in prof {
                    r.peak_solid_fraction = r.peak_solid_fraction.max(c / c_max);
                    r.min_solid_fraction = r.min_solid_fraction.min(c / c_max);
                }
            }
        }
        let done = !r.last.v_terminal.is_finite() || r.last.v_terminal <= V_CUT;
        if r.last.flags.contains(EventFlags::SOLVE_UNCONVERGED) {
            r.unconverged += 1;
            if !done {
                r.unconverged_only_last = false;
            }
        }
        if done {
            break;
        }
    }
    r
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
    let r = run_to_cutoff(&mut p, CAPACITY_AH, 2.0, 5_000);
    assert_eq!(r.unconverged, 0, "no 1C step should fail to converge");
    assert!(
        (3_000.0..4_000.0).contains(&r.t),
        "1C cut-off at {} s is nowhere near the reference's 3555 s",
        r.t
    );
    assert!(
        (4.5..5.4).contains(&r.ah),
        "1C delivered {} A·h against the reference's 4.94",
        r.ah
    );
    assert!(r.last.v_terminal <= V_CUT);
    assert!(
        r.last.soc_true < 0.1,
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
    assert!(r.min_ce > 400.0, "1C should not come close to depletion");
}

/// The cliff, which is the reason the phase exists: at 3C the DFN starves and delivers
/// materially less than the same cell modelled as an SPM.
///
/// The SPM comparison is what makes this a statement about the electrolyte rather than
/// about the discretisation — both models read the same `[spm]` block, the same OCP
/// tables and the same particle solver, and the *only* difference is whether the
/// electrolyte is solved for.
///
/// # The 0.8 threshold, and the margin that stopped being thin (Phase 7, slice D)
/// This used to record **0.69** on 10/5/10 against a 0.8 threshold, note that PyBaMM's own
/// pair manages 0.51 — so the engine *under*-stated the cliff — and warn that refining the
/// grid moved the ratio the wrong way, toward the threshold.
///
/// All three of those were symptoms of one defect, which slice D found and fixed: the
/// kinetics surface clamp was wide enough to disable the choke that ends a hard discharge,
/// so the positive electrode accepted lithium past `c_max` and the cell over-delivered. See
/// [`dfn::SURFACE_EDGE_FRACTION`], and [`the_solid_phase_never_holds_more_than_it_can`] for
/// the invariant that now catches it directly. The ratio is **0.44** here and PyBaMM's 0.51
/// is no longer beaten from the wrong side; refining the grid now moves the ratio *away*
/// from the threshold, as a converging discretisation should.
///
/// The threshold stays at 0.8 rather than tightening to the new measurement: it is a
/// "there is a cliff at all" assertion, and pinning it at 0.44 would make an ordinary grid
/// change read as a physics regression — which is precisely the trap the old note warned
/// about and then fell into.
#[test]
fn the_electrolyte_starves_at_three_c_and_an_spm_never_notices() {
    let mut d = pack(dfn_model(NODES, SHELLS), 1.0);
    let r = run_to_cutoff(&mut d, 3.0 * CAPACITY_AH, 2.0, 5_000);

    // Not `== 0`, and the difference is the physics. As the positive surface fills,
    // `i_0 → 0` and the cell's `V(i)` goes near-vertical; the step that crosses the
    // cut-off is asking the Newton to resolve a singularity, and it says so instead of
    // pretending. What must not happen is unconverged steps *during* the discharge, which
    // is what the `only_last` half asserts — and what a wider clamp used to hide by
    // removing the singularity altogether.
    assert!(
        r.unconverged <= 1 && r.unconverged_only_last,
        "3C raised SOLVE_UNCONVERGED on {} step(s), and not only on the last: a 3C \
         discharge must converge everywhere except possibly the collapse itself",
        r.unconverged
    );
    assert!(
        r.last.v_terminal.is_finite(),
        "the collapse must still produce a finite voltage, not a NaN"
    );

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
        r.ah < 0.8 * ah_spm,
        "the DFN delivered {} A·h against the SPM's {ah_spm}: no cliff",
        r.ah
    );
    assert!(
        r.t < 0.8 * t_spm,
        "the DFN lasted {} s against the SPM's {t_spm}: no cliff",
        r.t
    );
    assert!(
        r.min_ce < 1.0,
        "3C should drive the electrolyte to depletion; the minimum was {} mol/m3",
        r.min_ce
    );
}

/// A particle may not hold more lithium than it can hold. The invariant the wide surface
/// clamp violated, asserted directly on the state rather than inferred from a trajectory.
///
/// # Why this test exists and why it is separate from the golden
/// Slice D found the shipped cell driving the positive electrode's outermost shell to
/// **1.66–2.11 × `c_max`** at 3C — twice the lithium a solid phase can contain. It was
/// found by comparing against a PyBaMM reference, which is an expensive and indirect way to
/// notice a conservation violation: the golden says "the trajectory is wrong", and it took a
/// grid sweep, a floor sweep and a state dump to get from there to "the solid phase is
/// unbounded". This assertion says it in one line, needs no reference, and would have
/// failed the moment the defect was introduced.
///
/// It is also **independent of [`dfn::SURFACE_EDGE_FRACTION`]'s value**, which is what makes
/// it worth more than a test pinning that constant: any future change to the kinetics guard
/// — a different width, a smooth choke, a different functional form — is free to move the
/// trajectory but is not free to break conservation.
///
/// Both bounds carry a small tolerance because the solid diffusion is a finite-volume
/// scheme with a backward-Euler step, not an exact integrator; what it may not do is
/// overshoot by tens of percent.
///
/// # Exactly which quantity this bounds, because there are two and they differ
/// It bounds the **shell averages** — the concentrations actually stored in `DfnState`, and
/// therefore the lithium the cell is holding. That is the conservation claim.
///
/// It does **not** bound the *extrapolated surface* the kinetics evaluate,
/// `c_surf = c0 + β·j`, which is a reconstruction rather than stored lithium and is the
/// quantity [`dfn::SURFACE_EDGE_FRACTION`] actually clamps. The two are not
/// interchangeable and the gap is not small: measured over a 3C sweep of 1939 converged
/// steps, the shell average peaks at **0.918** while the extrapolated surface peaks at
/// **1.0164**, and the clamp engages at a converged solution on **12 of those 1939 steps**.
///
/// So the guard is still doing real work at the shipped constant — it is simply doing it
/// 1.6 % past full instead of 111 % past full, which is a linear reconstruction overshooting
/// slightly at the end of a hard discharge rather than an electrode accepting lithium it
/// cannot hold. A future regression could in principle push the reconstruction well past 1
/// while the shell averages stayed inside this bound. That is not left to inference: the
/// slice D perturbation table records that restoring the wide clamp fails **three** tests,
/// this one among them, so the coverage is measured rather than argued.
#[test]
fn the_solid_phase_never_holds_more_than_it_can() {
    // 3C, because that is where the reaction front is sharp enough for one x-node to
    // saturate while the rest of the electrode sits near a third full — the regime the
    // defect lived in. A 1C run never approaches either bound and would pass with the
    // clamp back at its old width.
    let mut p = pack(dfn_model(NODES, SHELLS), 1.0);
    let r = run_to_cutoff(&mut p, 3.0 * CAPACITY_AH, 2.0, 5_000);

    // Measured 0.918 peak (positive, outermost shell of the most-loaded node) and 0.048
    // minimum. The 1 % tolerance is scheme noise, not a licence: the defect this catches
    // was worth +111 %.
    assert!(
        r.peak_solid_fraction <= 1.01,
        "a stored shell concentration reached {:.4} of c_max at 3C — the solid phase is \
         holding lithium it cannot hold. The mechanism to suspect is the kinetics no longer \
         choking as the surface fills; see dfn::SURFACE_EDGE_FRACTION",
        r.peak_solid_fraction
    );
    assert!(
        r.min_solid_fraction >= -0.01,
        "a particle shell reached {:.4} of c_max at 3C — a solid phase cannot hold \
         negative lithium",
        r.min_solid_fraction
    );

    // And the guard against this test quietly becoming vacuous: if nothing ever gets near
    // full, the bound above is not being exercised and would pass with the choke removed.
    assert!(
        r.peak_solid_fraction > 0.85,
        "no particle shell got past {:.4} of c_max at 3C, so the upper bound above is \
         never approached and this test no longer discriminates",
        r.peak_solid_fraction
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
    let min_slow = run_to_cutoff(&mut slow, CAPACITY_AH, 2.0, 5_000).min_ce;
    assert!(
        min_slow > 1_000.0 * floor,
        "at 1C the minimum c_e was {min_slow} mol/m3, only {}x the floor — the floor is \
         no longer provably inert at this rate and this test no longer says what it says",
        min_slow / floor
    );

    let mut fast = pack(dfn_model(NODES, SHELLS), 1.0);
    let min_fast = run_to_cutoff(&mut fast, 3.0 * CAPACITY_AH, 2.0, 5_000).min_ce;
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

/// A DFN pack's solve now has something to chase: **two** passes, not one.
///
/// Slice B pinned this at `1` deliberately. A DFN answered the iteration with the same
/// line it had already handed over as its source, so the residual was zero by construction
/// and the loop exited having done an all-equivalent-circuit pack's arithmetic — a number
/// about that slice, not about the physics. Slice C's `probe_at` re-solves the cell at the
/// current each pass assigns, so the second pass measures a real curve against a real line.
///
/// Exactly two is worth pinning rather than "more than one". A constant-current 1S1P pack
/// is the degenerate case: `solve_current` hands back the demand whatever the aggregate
/// says, so pass two re-linearizes at the current pass one already assigned and lands on
/// it. Two is what "the tangent was one step stale and now is not" costs, and a pack that
/// suddenly needed more would be saying something changed.
#[test]
fn the_pack_solve_re_linearizes_where_it_put_the_cell() {
    let mut p = pack(dfn_model(NODES, SHELLS), 0.9);
    for _ in 0..20 {
        let t = p.step(5.0, Demand::Current(CAPACITY_AH), &env());
        assert_eq!(t.solve_iterations, 2);
        assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    }
}

/// A power demand is where a stale linearisation costs **first-order** accuracy, and this
/// is the test that says slice C shipped something.
///
/// `Demand::Power` is solved as `P = V(i)·i` on the pack's *aggregate line*, so a line
/// whose intercept is a step stale asks for the wrong current — and unlike a current
/// demand, nothing downstream corrects it. Measured on this cell before slice C: a 50 W
/// request drew 23.3 A and delivered **87.0 W** on the first step, because the only line
/// available was the never-solved cell's first-order seed; it settled to 75 mW off. The
/// iteration drives the same demand onto the curve, and what is left is the pack solve's
/// own `SOLVE_TOL_V`-sized residual.
///
/// The tolerance here is `1e-6` W against a measured worst case of ~1.2e-8 W. That is not
/// slack for its own sake: it is two orders below the *first* number this assertion has to
/// separate from (75 mW steady-state) and nine below the first step's 37 W, so the test
/// fails loudly if the iteration stops happening and does not fail on a re-tuned
/// `SOLVE_TOL_V`.
#[test]
fn a_power_demand_lands_on_the_curve_rather_than_on_a_stale_line() {
    for watts in [20.0, 50.0] {
        let mut p = pack(dfn_model(NODES, SHELLS), 1.0);
        let mut worst: f64 = 0.0;
        let mut passes = 0;
        for _ in 0..120 {
            let t = p.step(2.0, Demand::Power(watts), &env());
            passes = passes.max(t.solve_iterations);
            assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
            worst = worst.max((t.v_terminal * t.i_actual - watts).abs());
        }
        assert!(
            worst < 1.0e-6,
            "{watts} W demand delivered up to {worst} W off the request"
        );
        // The first step, where the cell has only its seed resistance, is the pass the
        // iteration works hardest on. If this ever reads 1 the loop is not running.
        assert!(
            passes >= 3,
            "{watts} W demand converged in {passes} passes, which is too few to have \
             corrected a seed-resistance line"
        );
    }
}

/// A voltage demand is the same failure with a name this repo already uses: **CV**.
///
/// `Demand::Voltage` is closed form off the aggregate line, `i = (E − V*)/R`, so an
/// intercept one step stale asks for a current wrong by `ΔE/R` — and on a taper, where the
/// current is small, that error is a large fraction of it. Measured before slice C on a
/// pack held at 3.60 V after a long 1C discharge: the first CV step drew **−4.64 A** where
/// the converged answer is −2.56 A, and the pack sat at 3.651 V — **51 mV** off the voltage
/// it was told to hold, settling to tens of microvolts.
///
/// This matters beyond the DFN: the CC-CV charge policy the browser client ships sizes its
/// switching band on how far the voltage solve lands from the target, and for this model
/// that distance was three to four orders larger than the equivalent circuit's.
#[test]
fn a_voltage_demand_actually_holds_the_voltage() {
    let mut p = pack(dfn_model(NODES, SHELLS), 1.0);
    for _ in 0..200 {
        p.step(2.0, Demand::Current(CAPACITY_AH), &env());
    }
    let mut worst: f64 = 0.0;
    for _ in 0..60 {
        let t = p.step(2.0, Demand::Voltage(3.90), &env());
        assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
        worst = worst.max((t.v_terminal - 3.90).abs());
    }
    assert!(
        worst < 1.0e-6,
        "a 3.90 V hold landed up to {worst} V away, where the pack solve's own tolerance \
         is 1e-9 V"
    );
}

/// Protection is applied **inside** the pack's iteration loop, so a DFN pack that used to
/// call it once per step now calls it once per pass — and a derate is a clamp, which is not
/// smooth.
///
/// The hazard is a demand sitting on the limit: pass one linearizes on a stale line and
/// lands under it, pass two corrects and lands over it, the clamp engages and disengages,
/// and the solve runs to `SOLVE_ITER_CAP` raising `SOLVE_UNCONVERGED` on a pack that is
/// physically fine. Slice C created that exposure and nothing else covers it — no scenario
/// in the repo or in the out-of-tree instrument pairs a nonlinear cell model with a BMS.
///
/// It does not happen, and the reason is structural rather than lucky: `apply_protection`
/// computes its allowed window from the **sensor frame and the chemistry**, never from
/// `i_req`, so every pass sees the *same* clamp to the same interval. A projection onto a
/// fixed interval cannot introduce an oscillation the unclamped map does not have. The only
/// `i_req`-dependent part is the `OC` flag, which the pack already treats as a per-pass
/// binding rather than an accumulator.
///
/// Swept across the limit at 0.90/0.98/0.999/1.0/1.001/1.02/1.10× under both a current and a
/// power demand: never unconverged, and at most 4 passes — the worst point being exactly on
/// the limit under a power demand, which is where it should be.
#[test]
fn a_derate_inside_the_iteration_does_not_chatter() {
    // 1.5C on the shipped cell: the chemistry's `max_discharge_c` times its capacity.
    let limit_a = 1.5 * CAPACITY_AH;
    for frac in [0.999, 1.0, 1.001, 1.10] {
        let mut c = cfg(dfn_model(NODES, SHELLS), 1.0);
        c.bms = Some(sim_core::BmsConfig {
            balancing: None,
            protection: Some(sim_core::ProtectionConfig {
                v_hard_margin_v: 0.1,
                t_hard_margin_k: 8.0,
                v_release_band_v: 0.08,
                t_release_band_k: 2.0,
            }),
            current_offset_a: 0.0,
            current_noise_sigma_a: 0.0,
            temp_probes: vec![(0, 0)],
            initial_soc_error: 0.0,
            rest_current_threshold_a: 0.05,
            rest_time_for_ocv_s: 300.0,
            ocv_correction_gain: 0.2,
            min_ocv_slope_v_per_soc: 0.05,
        });
        let mut p = Pack::new(&c, parse_chemistry(LGM50).expect("LG M50 parses")).expect("builds");
        // A power demand, because it is the one whose current is solved off the line and so
        // the one that can step across the clamp between passes.
        let demand = Demand::Power(limit_a * frac * 3.7);
        for n in 0..60 {
            let t = p.step(2.0, demand, &env());
            assert!(
                !t.flags.contains(EventFlags::SOLVE_UNCONVERGED),
                "a power demand at {frac}x the discharge limit failed to converge at step {n}"
            );
            assert!(
                t.solve_iterations <= 8,
                "a power demand at {frac}x the limit took {} passes at step {n}; the clamp is \
                 cycling between engaged and disengaged",
                t.solve_iterations
            );
            assert!(
                t.i_actual <= limit_a + 1.0e-9,
                "protection let {} A through a {limit_a} A limit",
                t.i_actual
            );
        }
    }
}

/// A zero-length probe step must not reach the cell's solver, and a *voltage* demand is
/// the only demand that shows it.
///
/// This test exists because removing [`sim_core::dfn`]'s `dt <= 0` guard broke **nothing**
/// in this suite. It is a real landmine: the backward-Euler mass rows carry
/// `(c − c_old)/dt`, so at `dt = 0` the transient vanishes and the solve returns the same
/// voltage at every current — a curve with `dV/di = 0`, whose tangent resistance falls onto
/// the `1e-9 Ω` floor that exists to stop the pack dividing by zero. Measured with the
/// guard removed: a 3.90 V hold asked for **1.03e9 A** and ran the pack solve to its
/// iteration cap. With it, 6.86 A in one pass.
///
/// A current demand cannot catch this — `solve_current` hands back the demand whatever the
/// aggregate resistance says — which is why the two probe steps below differ in kind rather
/// than in number. Reading an instantaneous voltage with a zero-length step is how this
/// repo does it (see the energy-balance property test), so this path is walked, not
/// hypothetical.
#[test]
fn a_zero_length_probe_step_does_not_reach_the_solver() {
    let mut p = pack(dfn_model(NODES, SHELLS), 1.0);
    for _ in 0..50 {
        p.step(2.0, Demand::Current(CAPACITY_AH), &env());
    }
    // A current demand: the current is the demand, so this only pins that nothing blows up.
    let held = p.step(0.0, Demand::Current(CAPACITY_AH), &env());
    assert!(held.v_terminal.is_finite() && !held.flags.contains(EventFlags::SOLVE_UNCONVERGED));

    // A voltage demand: here the current comes from the aggregate line's resistance, and a
    // resistance on the floor is a current off the scale.
    let cv = p.step(0.0, Demand::Voltage(3.90), &env());
    assert!(
        !cv.flags.contains(EventFlags::SOLVE_UNCONVERGED),
        "a zero-length voltage probe ran the pack solve to its cap"
    );
    assert_eq!(
        cv.solve_iterations, 1,
        "a zero-length probe has no transient to re-linearize, so one pass is the answer"
    );
    assert!(
        cv.i_actual.abs() < 10.0 * CAPACITY_AH,
        "a 3.90 V zero-length probe drew {} A, which is not a current this cell can pass — \
         the differential resistance has collapsed onto its floor",
        cv.i_actual
    );
}

/// A parallel group re-splits its current too — and the effect is **much smaller** than
/// the demand solves above, for a reason worth recording rather than discovering.
///
/// A stale line is stale mostly in its intercept, by roughly one step of `dV/dt`. Inside a
/// parallel group that staleness is common-mode: it moves every cell's `E` by nearly the
/// same amount and cancels out of the split. What survives is the differential part, and on
/// a 5 %-scatter 1S2P pack at 1C it moves the split by about 30 µA on 2.6 A — twelve parts
/// per million, rising with rate.
///
/// So this test asserts what is actually true: the group re-linearizes (three passes, where
/// a current demand on a single cell needs two), and the two cells still carry the whole
/// pack current between them. It deliberately does **not** assert a large split change,
/// because there is not one.
#[test]
fn a_parallel_group_re_splits_but_the_staleness_was_common_mode() {
    let mut c = cfg(dfn_model(NODES, SHELLS), 1.0);
    c.parallel = 2;
    c.scatter = Scatter {
        capacity_sigma: 0.05,
        r0_sigma: 0.05,
    };
    let mut p = Pack::new(&c, parse_chemistry(LGM50).expect("LG M50 parses")).expect("builds");
    let demand = 2.0 * CAPACITY_AH;
    let mut passes = 0;
    for _ in 0..40 {
        let t = p.step(2.0, Demand::Current(demand), &env());
        passes = passes.max(t.solve_iterations);
        assert!(!t.flags.contains(EventFlags::SOLVE_UNCONVERGED));
    }
    assert!(
        passes >= 3,
        "a scattered parallel group converged in {passes} passes; the split is nonlinear \
         and should cost at least one more than a single cell's two"
    );
    let cells = &snapshot_json(&p)["pack"]["groups"][0]["cells"];
    let split: Vec<f64> = (0..2)
        .map(|k| {
            cells[k]["model"]["Dfn"]["i_last"]
                .as_f64()
                .expect("i_last is a number")
        })
        .collect();
    assert!(
        (split[0] + split[1] - demand).abs() < 1.0e-9,
        "the group's cells carry {} + {} A against a {demand} A demand",
        split[0],
        split[1]
    );
    assert!(
        split[0] != split[1],
        "a 5 % scattered group split its load evenly, so the scatter reached nothing"
    );
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
