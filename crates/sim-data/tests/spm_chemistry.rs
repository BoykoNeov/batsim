//! `[spm]` chemistry-section tests: the shipped LG M50 file, and one rejection test
//! per validation rule.
//!
//! # Why this is a new file rather than more cases in `load.rs`
//! `load.rs`'s band tests (`shipped_aging_coefficients_…`, `shipped_plating_…`,
//! `shipped_runaway_…`) each loop over a hard-coded list of shipped chemistries, and
//! the natural way to cover a new chemistry is to add it to those three lists. This
//! slice deliberately does not: Phase 6's slices are gated on *not* editing existing
//! tests, so that a trajectory or an assertion cannot quietly move under cover of an
//! unrelated change. The bands are therefore re-applied here, against the same engine
//! functions, for the new file only — see `spm_chemistry_placeholders_are_plausible`.
//!
//! The one thing this file cannot yet test is the interesting error: a `PackConfig`
//! that selects a porous-electrode model against a chemistry with no `[spm]` block.
//! There is no such config field yet — adding it means adding a field to `PackConfig`,
//! which ~23 exhaustive struct literals across 22 existing test files construct, and
//! the model it would select does not exist either. Both land in slice C, together.
//! See `docs/plans/phase-6-porous-electrodes.md`.

use sim_core::ChemistryError;
use sim_data::{parse_chemistry, DataError};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shipped LG M50 chemistry parses, validates, and carries both descriptions.
///
/// "Both" is the point: this is the first chemistry that describes one cell twice, as
/// an equivalent circuit and as a pair of porous electrodes. Neither section is a
/// fallback for the other.
#[test]
fn lgm50_chemistry_loads_and_validates() {
    let chem = parse_chemistry(LGM50).expect("LG M50 chemistry should load and validate");

    assert_eq!(chem.meta.id, "nmc_21700_lgm50");
    // Usable capacity between the Chen2020 stoichiometry limits, per
    // tools/reference/fit_ocv.py — not the parameter set's 5.0 A.h nameplate.
    assert!((chem.cell.capacity_ah - 5.153198).abs() < 1e-6);
    assert_eq!(chem.n_rc(), 2);
    assert!(
        chem.spm.is_some(),
        "this chemistry must carry an [spm] section"
    );
    assert!(chem.aging.is_some() && chem.safety.is_some());
}

/// Every chemistry shipped before this phase must still have **no** `[spm]` section.
///
/// This is the optionality check, and it is what keeps the snapshot schema at v9: the
/// section had to be addable without any existing file, scenario or snapshot changing
/// shape. A default that materialized an empty `[spm]` would pass every other test in
/// this file and fail this one.
#[test]
fn the_ecm_only_chemistries_carry_no_spm_section() {
    for (name, text) in [
        (
            "lfp",
            include_str!("../../../chemistries/lfp_26650_generic.toml"),
        ),
        (
            "nmc18650",
            include_str!("../../../chemistries/nmc_18650_generic.toml"),
        ),
    ] {
        let chem = parse_chemistry(text).expect("shipped chemistry loads");
        assert!(
            chem.spm.is_none(),
            "{name} must have no [spm] section — the field is optional, not defaulted"
        );
    }
}

/// Spot-check that the extracted values are the parameter set's, not a transcription.
///
/// These are exact equalities on purpose. Every one is a literal Chen2020 key or a
/// literal inside one of its functions, so there is no fit and no tolerance to allow
/// for — if one of these moves, either the parameter set changed or someone edited a
/// number by hand, and both deserve a failing test.
#[test]
fn spm_section_carries_the_extracted_chen2020_values() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let spm = chem.spm.as_ref().expect("[spm]");

    assert_eq!(spm.t_ref_k, 298.15);
    assert_eq!(spm.c_e_mol_per_m3, 1000.0);
    assert_eq!(spm.electrode_area_m2, 0.1027); // 0.065 m x 1.58 m
    assert_eq!(spm.contact_resistance_ohm, 0.0);

    assert_eq!(spm.negative.particle_radius_m, 5.86e-06);
    assert_eq!(spm.negative.diffusivity_m2_per_s, 3.3e-14);
    assert_eq!(spm.negative.c_max_mol_per_m3, 33133.0);
    assert_eq!(spm.negative.active_volume_fraction, 0.75);
    // The two kinetic numbers that are NOT parameter-set keys: both are literals in
    // the body of graphite_LGM50_electrolyte_exchange_current_density_Chen2020.
    assert_eq!(spm.negative.m_ref, 6.48e-07);
    assert_eq!(spm.negative.reaction_ea_j_per_mol, 35000.0);

    assert_eq!(spm.positive.particle_radius_m, 5.22e-06);
    assert_eq!(spm.positive.diffusivity_m2_per_s, 4e-15);
    assert_eq!(spm.positive.c_max_mol_per_m3, 63104.0);
    assert_eq!(spm.positive.m_ref, 3.42e-06);
    assert_eq!(spm.positive.reaction_ea_j_per_mol, 17800.0);

    // Chen2020 fits solid diffusivity as a constant and publishes no activation energy
    // for it. The zero is the documented absence, not an oversight — see the field's
    // doc comment and the note in the TOML.
    assert_eq!(spm.negative.diffusivity_ea_j_per_mol, 0.0);
    assert_eq!(spm.positive.diffusivity_ea_j_per_mol, 0.0);
    // Likewise both entropic-change keys are literally 0.0 in Chen2020.
    assert_eq!(spm.negative.docp_dt_v_per_k, 0.0);
    assert_eq!(spm.positive.docp_dt_v_per_k, 0.0);
}

/// The half-cell OCP tables run **downhill**, and cover the window the cell works in.
///
/// The direction is the whole point of having a separate check from `[ocv]`'s: adding
/// lithium to an electrode lowers its potential against lithium metal, so both tables
/// descend even though the full-cell voltage rises with state of charge. Validating
/// these with `[ocv]`'s non-decreasing rule would reject a correct extraction.
#[test]
fn spm_ocp_tables_run_downhill_and_span_the_operating_window() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let spm = chem.spm.as_ref().expect("[spm]");

    for (name, e) in [("negative", &spm.negative), ("positive", &spm.positive)] {
        let (s, v) = (&e.ocp.stoich, &e.ocp.volts);
        assert_eq!(s.len(), v.len());
        assert!(
            s.len() >= 2,
            "{name}: a one-point OCP table cannot interpolate"
        );
        for i in 1..s.len() {
            assert!(
                s[i] > s[i - 1],
                "{name}: stoich must strictly ascend at {i}"
            );
            assert!(v[i] <= v[i - 1], "{name}: OCP must not rise at {i}");
        }
        // And it genuinely descends overall — a table that merely never rises could be
        // a constant, which would satisfy the assertion above and pin nothing.
        assert!(
            v[v.len() - 1] < v[0],
            "{name}: OCP must actually fall across the table"
        );
        assert!(
            s[0] <= e.stoich_min && s[s.len() - 1] >= e.stoich_max,
            "{name}: OCP table must span [{}, {}]",
            e.stoich_min,
            e.stoich_max
        );
    }

    // The two electrodes move in opposite directions with state of charge, which is why
    // the cell voltage rises while both tables fall. Check the full-cell potential at
    // the charged end exceeds the one at the discharged end.
    let u = |e: &sim_core::ElectrodeParams, x: f64| {
        let (s, v) = (&e.ocp.stoich, &e.ocp.volts);
        let i = s.partition_point(|&b| b < x).clamp(1, s.len() - 1);
        let f = (x - s[i - 1]) / (s[i] - s[i - 1]);
        v[i - 1] + f * (v[i] - v[i - 1])
    };
    let full =
        u(&spm.positive, spm.positive.stoich_min) - u(&spm.negative, spm.negative.stoich_max);
    let empty =
        u(&spm.positive, spm.positive.stoich_max) - u(&spm.negative, spm.negative.stoich_min);
    assert!(
        full > empty,
        "a charged cell ({full:.3} V) must sit above a discharged one ({empty:.3} V)"
    );
    // And both ends land near the parameter set's own voltage cut-offs.
    assert!(
        (full - chem.cell.v_max).abs() < 0.15,
        "charged end {full:.3} V vs v_max"
    );
    assert!(
        (empty - chem.cell.v_min).abs() < 0.15,
        "discharged end {empty:.3} V vs v_min"
    );
}

/// The new file's placeholder `[aging]` and `[safety]` sets get the same plausibility
/// bands `load.rs` applies to the other two shipped chemistries, for the same reason:
/// numbers that each pass `validate()` alone can still be an implausible set together,
/// and this file rescaled two of them by capacity.
#[test]
fn spm_chemistry_placeholders_are_plausible() {
    const YEAR_S: f64 = 365.0 * 24.0 * 3600.0;
    const ROOM_K: f64 = 298.15;

    let chem = parse_chemistry(LGM50).expect("loads");
    let aging = chem.aging.as_ref().expect("[aging]");
    let safety = chem.safety.as_ref().expect("[safety]");

    let fade = sim_core::aging::calendar_rate(aging, ROOM_K, 1.0) * YEAR_S.sqrt();
    assert!(
        (0.01..=0.5).contains(&fade),
        "one year at 25 C and full SOC fades {:.1} % — outside the 1-50 % band",
        fade * 100.0
    );

    // 500 full cycles; throughput counts both directions. This is the number the
    // capacity rescale in the TOML exists to protect: leaving cyc_fade_per_ah at the
    // 18650 file's value would age this larger cell 1.7x faster per cycle.
    let throughput_ah = 500.0 * 2.0 * chem.cell.capacity_ah;
    let cyc = sim_core::aging::cycle_increment(aging, throughput_ah, 1.0);
    assert!(
        (0.01..=0.5).contains(&cyc),
        "500 full cycles fade {:.1} % — outside the 1-50 % band",
        cyc * 100.0
    );

    let ah = chem.cell.capacity_ah;
    let plate_fade = sim_core::plating::plating_fade_increment(safety, ah);
    assert!(
        (0.000_5..=0.05).contains(&plate_fade),
        "one full cold charge fades {:.3} % — outside the 0.05-5 % band",
        plate_fade * 100.0
    );
    let p = sim_core::plating::short_probability(safety, ah);
    assert!(
        (0.000_1..=0.05).contains(&p),
        "one full cold charge carries a {:.3} % short chance — outside the 0.01-5 % band",
        p * 100.0
    );
    let r0_mid = sim_core::ecm::r0_lookup(&chem.r0, 0.5, 298.15);
    assert!(
        safety.plating_short_ohms > 100.0 * r0_mid,
        "a {} ohm short against a {r0_mid} ohm cell is not a *soft* short",
        safety.plating_short_ohms
    );

    // Adiabatic ceiling, the same 100-1200 K band load.rs uses. This file's runaway
    // budget was set FROM the derived heat capacity, so the check is closing a loop
    // rather than discovering anything — which is exactly when it is cheapest to add.
    let ceiling = safety.runaway_energy_j / chem.thermal.heat_capacity_j_per_k;
    assert!(
        (100.0..=1200.0).contains(&ceiling),
        "a fully-reacted cell rises {ceiling:.0} K — outside the 100-1200 K band"
    );
}

/// A minimal valid chemistry with one `{}` hole where the `[spm]` block goes, so every
/// rejection test below substitutes exactly one section and a failure can only have
/// come from it. Mirrors `load.rs`'s `chemistry_with_ocv` harness.
fn chemistry_with_spm(spm: &str) -> String {
    format!(
        r#"
[meta]
id = "bad"
name = "Bad"
provenance = "test"

[cell]
capacity_ah = 2.5
v_max = 3.65
v_min = 2.0
max_charge_c = 1.0
max_discharge_c = 3.0
t_charge_min_k = 273.15
t_max_k = 333.15

[ocv]
soc   = [0.0, 0.5, 1.0]
volts = [3.0, 3.2, 3.4]

[r0]
soc = [0.0, 1.0]
temp_k = [298.15]
ohms = [[0.02], [0.02]]

[[rc]]
r_ohms = 0.01
c_farad = 2000.0

[reversal]
v_per_soc = 100.0
floor_v   = 0.0

[thermal]
heat_capacity_j_per_k = 95.0
h_area_w_per_k = 0.35

{spm}
"#
    )
}

/// A well-formed `[spm]` block. Kept deliberately tiny — three OCP points, round
/// numbers — so a rejection test is reading one broken field rather than scanning a
/// copy of the shipped file.
///
/// Tests break exactly one field by *substituting* its line (see [`neg_broken`])
/// rather than by appending an override, because TOML rejects a duplicate key before
/// validation ever runs — which turns a validation test into a parse test that passes
/// for the wrong reason.
fn good_spm(neg_ocp: &str) -> String {
    format!(
        r#"
[spm]
t_ref_k = 298.15
c_e_mol_per_m3 = 1000.0
electrode_area_m2 = 0.1
contact_resistance_ohm = 0.0

[spm.negative]
particle_radius_m = 5.0e-6
diffusivity_m2_per_s = 3.0e-14
c_max_mol_per_m3 = 33000.0
active_volume_fraction = 0.75
thickness_m = 8.0e-5
m_ref = 6.0e-7
charge_transfer_alpha = 0.5
stoich_min = 0.1
stoich_max = 0.9
[spm.negative.ocp]
{neg_ocp}

[spm.positive]
particle_radius_m = 5.0e-6
diffusivity_m2_per_s = 4.0e-15
c_max_mol_per_m3 = 63000.0
active_volume_fraction = 0.665
thickness_m = 7.5e-5
m_ref = 3.4e-6
charge_transfer_alpha = 0.5
stoich_min = 0.2
stoich_max = 0.9
[spm.positive.ocp]
stoich = [0.0, 0.5, 1.0]
volts  = [4.4, 4.0, 3.5]
"#
    )
}

/// The default `neg_ocp`: a correct, descending table spanning [0, 1].
const GOOD_NEG_OCP: &str = "stoich = [0.0, 0.5, 1.0]\nvolts  = [1.5, 0.3, 0.1]";

/// Break exactly one line of the **negative** electrode by substitution.
///
/// `replacen(.., 1)` hits the first occurrence, and the negative block is written
/// first, so this reaches the negative electrode even for the lines both electrodes
/// share (`charge_transfer_alpha`, `stoich_max`, `particle_radius_m`). That is load
/// bearing: a plain `replace` would break both electrodes at once and the error would
/// name whichever is validated first, which happens to be the one we wanted only by
/// luck.
fn neg_broken(old: &str, new: &str) -> String {
    let spm = good_spm(GOOD_NEG_OCP);
    assert!(spm.contains(old), "template has no line `{old}` to break");
    spm.replacen(old, new, 1)
}

fn reject(spm: &str) -> ChemistryError {
    let text = chemistry_with_spm(spm);
    match parse_chemistry(&text) {
        Err(DataError::Invalid(e)) => e,
        Err(other) => panic!("expected a validation error, got {other}"),
        Ok(_) => panic!("expected rejection, but the chemistry validated"),
    }
}

/// The template itself must be valid, or every rejection test below is vacuous —
/// passing because the fixture is broken rather than because the rule works.
#[test]
fn the_spm_template_is_itself_valid() {
    let text = chemistry_with_spm(&good_spm(GOOD_NEG_OCP));
    let chem = parse_chemistry(&text).expect("the template must validate");
    assert!(chem.spm.is_some());
}

/// **The direction check.** An OCP table that ascends — i.e. one validated as if it
/// were an `[ocv]` table — must be rejected.
///
/// This is the rule most likely to be "fixed" into wrongness later, because
/// non-decreasing is what the sibling table requires and the two look identical from a
/// distance. A rising half-cell OCP is not a plausible curve with a sign error; it is
/// the claim that lithiating an electrode raises its potential.
#[test]
fn an_ascending_ocp_table_is_rejected() {
    let err = reject(&good_spm(
        "stoich = [0.0, 0.5, 1.0]\nvolts  = [0.1, 0.3, 1.5]",
    ));
    assert_eq!(
        err,
        ChemistryError::NotMonotone {
            what: "spm.negative.ocp.volts",
            strict: false,
            index: 1,
        }
    );
}

/// A non-ascending stoichiometry axis breaks the lookup's bracketing, exactly as it
/// does for `[ocv]`.
#[test]
fn a_non_ascending_ocp_stoich_axis_is_rejected() {
    let err = reject(&good_spm(
        "stoich = [0.0, 0.5, 0.5]\nvolts  = [1.5, 0.3, 0.1]",
    ));
    assert_eq!(
        err,
        ChemistryError::NotMonotone {
            what: "spm.negative.ocp.stoich",
            strict: true,
            index: 2,
        }
    );
}

/// An empty OCP table has its own arm, and it needs its own test: the "spans the
/// operating window" check below indexes `stoich[0]`, so without this guard an empty
/// table would panic rather than be rejected — and `validate` is on the no-panic path.
#[test]
fn an_empty_ocp_table_is_rejected() {
    let err = reject(&good_spm("stoich = []\nvolts  = []"));
    assert_eq!(err, ChemistryError::Empty("spm.negative.ocp.stoich"));
}

#[test]
fn a_length_mismatched_ocp_table_is_rejected() {
    let err = reject(&good_spm("stoich = [0.0, 0.5, 1.0]\nvolts  = [1.5, 0.1]"));
    assert_eq!(
        err,
        ChemistryError::LengthMismatch {
            table: "spm.negative.ocp",
            a: 3,
            b: 2,
        }
    );
}

/// An OCP table that stops short of the operating window silently clamps to a flat
/// potential at the ends — losing the model's behaviour exactly where it matters most.
#[test]
fn an_ocp_table_that_misses_the_operating_window_is_rejected() {
    // stoich_max is 0.9 but the table stops at 0.6.
    let err = reject(&good_spm(
        "stoich = [0.0, 0.3, 0.6]\nvolts  = [1.5, 0.3, 0.1]",
    ));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what: "spm.negative.ocp.stoich must span [stoich_min, stoich_max]",
        }
    );
}

/// Zero `m_ref` is rejected rather than read as "no kinetics": it is a
/// divide-by-zero — infinite kinetic overpotential at any current — dressed as a
/// parameter choice. Contrast the `[safety]` fields, where zero legitimately means
/// inert.
#[test]
fn a_zero_reaction_rate_is_rejected() {
    let err = reject(&neg_broken("m_ref = 6.0e-7", "m_ref = 0.0"));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "spm.negative.m_ref",
            value: 0.0,
        }
    );
}

#[test]
fn a_non_positive_particle_radius_is_rejected() {
    let err = reject(&neg_broken(
        "particle_radius_m = 5.0e-6",
        "particle_radius_m = 0.0",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "spm.negative.particle_radius_m",
            value: 0.0,
        }
    );
}

/// Both endpoints are excluded: `alpha` and `1 - alpha` are both Butler-Volmer
/// exponents, so either one kills a branch of the equation.
#[test]
fn an_out_of_range_charge_transfer_coefficient_is_rejected() {
    for bad in ["0.0", "1.0", "1.5"] {
        let err = reject(&neg_broken(
            "charge_transfer_alpha = 0.5",
            &format!("charge_transfer_alpha = {bad}"),
        ));
        assert_eq!(
            err,
            ChemistryError::BadRange {
                what: "spm.negative.charge_transfer_alpha must be in (0, 1)",
            },
            "alpha = {bad} must be rejected"
        );
    }
}

#[test]
fn an_out_of_range_active_volume_fraction_is_rejected() {
    for bad in ["0.0", "1.5"] {
        let err = reject(&neg_broken(
            "active_volume_fraction = 0.75",
            &format!("active_volume_fraction = {bad}"),
        ));
        assert_eq!(
            err,
            ChemistryError::BadRange {
                what: "spm.negative.active_volume_fraction must be in (0, 1]",
            },
            "volume fraction = {bad} must be rejected"
        );
    }
    // The closed end is legal: an electrode that is all active material is physically
    // silly but not a schema violation, and rejecting it would be the validator having
    // an opinion rather than enforcing an invariant.
    let text = chemistry_with_spm(&neg_broken(
        "active_volume_fraction = 0.75",
        "active_volume_fraction = 1.0",
    ));
    assert!(
        parse_chemistry(&text).is_ok(),
        "a fraction of exactly 1 must be accepted"
    );
}

#[test]
fn out_of_order_stoichiometry_limits_are_rejected() {
    let err = reject(&neg_broken("stoich_min = 0.1", "stoich_min = 0.95"));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what:
                "spm.negative stoichiometry limits must satisfy 0 <= stoich_min < stoich_max <= 1",
        }
    );
}

/// A stoichiometry outside [0, 1] is not a limit, it is a unit error: stoichiometry is
/// a fraction of `c_max` by definition.
#[test]
fn stoichiometry_limits_outside_zero_one_are_rejected() {
    let err = reject(&neg_broken("stoich_max = 0.9", "stoich_max = 1.4"));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what:
                "spm.negative stoichiometry limits must satisfy 0 <= stoich_min < stoich_max <= 1",
        }
    );
}

#[test]
fn a_negative_activation_energy_is_rejected() {
    let err = reject(&neg_broken(
        "m_ref = 6.0e-7",
        "m_ref = 6.0e-7\nreaction_ea_j_per_mol = -1.0",
    ));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what: "spm.negative activation energies must be finite and >= 0",
        }
    );
}

/// TOML admits `inf` and `nan` floats, and an infinite diffusivity would propagate
/// straight through the solve, so finiteness is checked rather than assumed.
#[test]
fn a_non_finite_transport_value_is_rejected() {
    let err = reject(&neg_broken(
        "diffusivity_m2_per_s = 3.0e-14",
        "diffusivity_m2_per_s = inf",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "spm.negative.diffusivity_m2_per_s",
            value: f64::INFINITY,
        }
    );
}

/// Section-level fields are validated too, not just the electrodes.
#[test]
fn a_non_positive_section_level_value_is_rejected() {
    let text =
        chemistry_with_spm(&good_spm(GOOD_NEG_OCP).replace("t_ref_k = 298.15", "t_ref_k = 0.0"));
    match parse_chemistry(&text) {
        Err(DataError::Invalid(ChemistryError::NotPositive { what, value })) => {
            assert_eq!(what, "spm.t_ref_k");
            assert_eq!(value, 0.0);
        }
        other => panic!("expected spm.t_ref_k rejection, got {other:?}"),
    }
}

/// A negative contact resistance would *add* energy to the cell. Zero is fine.
#[test]
fn a_negative_contact_resistance_is_rejected() {
    let text = chemistry_with_spm(&good_spm(GOOD_NEG_OCP).replace(
        "contact_resistance_ohm = 0.0",
        "contact_resistance_ohm = -0.01",
    ));
    match parse_chemistry(&text) {
        Err(DataError::Invalid(ChemistryError::Negative { what, .. })) => {
            assert_eq!(what, "spm.contact_resistance_ohm");
        }
        other => panic!("expected contact-resistance rejection, got {other:?}"),
    }
}

/// The positive electrode is validated by the same code path, and its errors must name
/// *it* — the two blocks are structurally identical, so an error that said only
/// "particle_radius_m" would send the reader to the wrong electrode.
#[test]
fn a_positive_electrode_fault_names_the_positive_electrode() {
    let text =
        chemistry_with_spm(&good_spm(GOOD_NEG_OCP).replace("m_ref = 3.4e-6", "m_ref = -1.0"));
    match parse_chemistry(&text) {
        Err(DataError::Invalid(ChemistryError::NotPositive { what, value })) => {
            assert_eq!(what, "spm.positive.m_ref");
            assert_eq!(value, -1.0);
        }
        other => panic!("expected a positive-electrode rejection, got {other:?}"),
    }
}
