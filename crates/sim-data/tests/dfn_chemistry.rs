//! `[dfn]` chemistry-section tests: the shipped LG M50 file, and one rejection test
//! per validation rule.
//!
//! # Why a new file rather than more cases in `spm_chemistry.rs`
//! The same reason that file gives for not extending `load.rs`: a slice is gated on
//! *not* editing existing tests, so a trajectory or an assertion cannot quietly move
//! under cover of an unrelated change. This slice does edit two existing test files —
//! `spm_exact_bits.rs`'s pin and `spm_golden.rs`'s shell-count measurement — and both
//! are argued for in the commit and in `docs/plans/phase-7-dfn.md`. Everything the
//! `[dfn]` block itself needs lives here, where it is additive.
//!
//! # What this file cannot yet test
//! The interesting error: a `PackConfig` selecting `CellModel::Dfn` against a chemistry
//! that has `[dfn]` but no `[spm]`, or the reverse. `[dfn]` extends `[spm]` and a DFN
//! cell reads both, so the build error has to name whichever half is missing — and
//! there is no config variant to ask for the model yet. That lands in slice B, with the
//! variant. See the `ChemistryParams::dfn` doc comment for why the check is deliberately
//! *not* also made here.

use sim_core::ChemistryError;
use sim_data::{parse_chemistry, DataError};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The shipped LG M50 chemistry now describes one cell **three** times: as an
/// equivalent circuit, as a pair of single particles, and — with `[spm]` — as a
/// Doyle–Fuller–Newman sandwich. None of the three is a fallback for the others.
#[test]
fn lgm50_chemistry_carries_a_dfn_section_that_extends_its_spm_one() {
    let chem = parse_chemistry(LGM50).expect("LG M50 chemistry should load and validate");
    assert!(chem.spm.is_some(), "[dfn] extends [spm]; both must be here");
    assert!(
        chem.dfn.is_some(),
        "this chemistry must carry a [dfn] section"
    );
}

/// Every other shipped chemistry must still have **no** `[dfn]` section.
///
/// The optionality check, and it is what keeps the snapshot schema where it is: the
/// section had to be addable without any existing file, scenario or snapshot changing
/// shape. A `#[serde(default)]` that materialized an empty `[dfn]` would pass every
/// other test in this file and fail this one.
///
/// `nmc_18650_generic` and `lfp_26650_generic` have no `[spm]` either, so they are the
/// straightforward case. The sharper one is that a chemistry *could* legitimately carry
/// `[spm]` and no `[dfn]` — that is Phase 6's whole shipped state — and nothing here may
/// make the pair mandatory.
#[test]
fn the_other_shipped_chemistries_carry_no_dfn_section() {
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
            chem.dfn.is_none(),
            "{name} must have no [dfn] section — the field is optional, not defaulted"
        );
    }
}

/// Spot-check that the extracted values are the parameter set's, not a transcription.
///
/// Exact equalities on purpose, as in `spm_chemistry.rs`: every one is a literal
/// Chen2020 key or a coefficient of a published fit, so there is no tolerance to allow
/// for. If one of these moves, either the parameter set changed or someone edited a
/// number by hand, and both deserve a failing test.
#[test]
fn dfn_section_carries_the_extracted_chen2020_values() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let dfn = chem.dfn.as_ref().expect("[dfn]");

    assert_eq!(dfn.transference_number, 0.2594);
    // Literally 1.0 in Chen2020, so the (1 + dlnf/dlnc) term is unity on this set. The
    // field earns its keep only for a parameter set that measures it.
    assert_eq!(dfn.thermodynamic_factor, 1.0);

    assert_eq!(dfn.negative.porosity, 0.25);
    assert_eq!(dfn.separator.porosity, 0.47);
    assert_eq!(dfn.positive.porosity, 0.335);
    assert_eq!(dfn.separator.thickness_m, 1.2e-05);
    assert_eq!(dfn.negative.solid_conductivity_s_per_m, 215.0);
    assert_eq!(dfn.positive.solid_conductivity_s_per_m, 0.18);
    for b in [
        dfn.negative.bruggeman_electrolyte,
        dfn.separator.bruggeman_electrolyte,
        dfn.positive.bruggeman_electrolyte,
    ] {
        assert_eq!(b, 1.5);
    }
    // Chen2020's own value for both electrode phases, and a real value rather than a
    // missing one: the published solid conductivities are already effective.
    assert_eq!(dfn.negative.bruggeman_electrode, 0.0);
    assert_eq!(dfn.positive.bruggeman_electrode, 0.0);
}

/// The porosity is **not** one minus the active volume fraction, and this is the check
/// that says so.
///
/// A schema that derived `ε_e` from `ε_s` would look reasonable and be wrong for every
/// real cell: binder, conductive additive and carbon make up the difference. On this set
/// the negative electrode is 0.75 solid against 0.25 pore, which sums to exactly 1 and
/// would let the mistake through; the positive is 0.665 against 0.335, which also sums
/// to 1. So the arithmetic alone cannot catch it here, and the assertion is written as
/// the *independence* claim instead — the two fields are separate reads of separate
/// keys, and this test exists to fail the day someone "simplifies" one away.
#[test]
fn porosity_and_active_volume_fraction_are_independent_reads() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let dfn = chem.dfn.as_ref().expect("[dfn]");
    let spm = chem.spm.as_ref().expect("[spm]");

    assert_eq!(spm.negative.active_volume_fraction, 0.75);
    assert_eq!(dfn.negative.porosity, 0.25);
    assert_eq!(spm.positive.active_volume_fraction, 0.665);
    assert_eq!(dfn.positive.porosity, 0.335);
}

/// The two electrolyte transport fits are stored as the published coefficients, exactly.
///
/// Two claims, and the second is why `PowerTerm` is a pair rather than a coefficient
/// array: the conductivity's middle term has exponent **1.5**, so this is a sum of power
/// terms and not a polynomial. A schema indexed by degree could not spell it.
///
/// Only the *parsed* literals are asserted. Nothing evaluated through them is, and that
/// is deliberate: `x^1.5` reached through `powf` is not in the pure-IEEE set that
/// `spm_exact_bits.rs` may pin. The one exception is checked in
/// [`the_transport_fits_are_positive_where_the_cell_starts`], where the sum reduces to
/// plain addition.
#[test]
fn the_electrolyte_transport_fits_are_stored_as_published_coefficients() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let dfn = chem.dfn.as_ref().expect("[dfn]");

    let d: Vec<(f64, f64)> = dfn
        .electrolyte_diffusivity_terms
        .iter()
        .map(|t| (t.coefficient, t.exponent))
        .collect();
    assert_eq!(
        d,
        vec![(8.794e-11, 2.0), (-3.972e-10, 1.0), (4.862e-10, 0.0)],
        "Nyman 2008 D_e, in x = c_e/1000"
    );

    let k: Vec<(f64, f64)> = dfn
        .electrolyte_conductivity_terms
        .iter()
        .map(|t| (t.coefficient, t.exponent))
        .collect();
    assert_eq!(
        k,
        vec![(0.1297, 3.0), (-2.51, 1.5), (3.329, 1.0)],
        "Nyman 2008 kappa_e, in x = c_e/1000 — note the 1.5"
    );
}

/// At the concentration the cell starts from, both fits are positive — and that value
/// is exact arithmetic, so it can be stated as a number rather than a tolerance.
///
/// `x = c_e/1000` is the fit's own variable, so at Chen2020's initial 1000 mol/m³ the
/// sum is `Σ coefficient`: additions only, no `powf`, identical on every conforming
/// platform. That is why validation checks positivity *there* and nowhere else — both
/// fits are non-monotone over the range a 3C discharge visits, so no cheap sampling
/// would be a proof, and a check that pretended otherwise would be worse than an honest
/// narrow one.
#[test]
fn the_transport_fits_are_positive_where_the_cell_starts() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let dfn = chem.dfn.as_ref().expect("[dfn]");
    let spm = chem.spm.as_ref().expect("[spm]");

    // The initial concentration is [spm]'s, not a second copy in [dfn] — an SPM holds it
    // constant and a DFN takes it as an initial value, but it is one number.
    assert_eq!(spm.c_e_mol_per_m3, 1000.0);

    let sum =
        |terms: &[sim_core::chem::PowerTerm]| -> f64 { terms.iter().map(|t| t.coefficient).sum() };
    let d = sum(&dfn.electrolyte_diffusivity_terms);
    let k = sum(&dfn.electrolyte_conductivity_terms);

    assert!(d > 0.0, "D_e at 1000 mol/m3 is {d}");
    assert!(k > 0.0, "kappa_e at 1000 mol/m3 is {k}");
    // And they are the right order of magnitude for a liquid carbonate electrolyte:
    // ~1e-10 m2/s and ~1 S/m. A units slip of 1e3 in either direction fails here and
    // would otherwise only show up as a wrong trajectory two slices later.
    assert!(
        (1e-11..=1e-9).contains(&d),
        "D_e = {d} m2/s is not a plausible liquid-electrolyte diffusivity"
    );
    assert!(
        (0.1..=10.0).contains(&k),
        "kappa_e = {k} S/m is not a plausible liquid-electrolyte conductivity"
    );
}

/// Every `f64` in the shipped `[dfn]` block is individually pinned above — so this
/// checks the only thing those assertions cannot: that there are no *others*.
///
/// # Why there is no FNV hash here, unlike `spm_exact_bits.rs`
/// That file hashes because `[spm]` carries 148 numbers, most of them OCP breakpoints,
/// and a pin per value would be 300 lines nobody would keep current. `[dfn]` has 22, all
/// of them named quantities, so asserting each one by name is both feasible and
/// **strictly stronger**: a failure says which number moved instead of that one did.
///
/// What a per-value pin loses is the tripwire for a *new* field, which a hash gets for
/// free by changing. This test is that tripwire. It counts the numbers serde can see
/// rather than listing field names, so it trips on a field added anywhere in the block
/// — including inside `DfnElectrode` or `DfnSeparator`, which is where a reader looking
/// only at `DfnParams` would miss one.
#[test]
fn the_dfn_block_has_exactly_the_values_pinned_above() {
    let chem = parse_chemistry(LGM50).expect("loads");
    let dfn = chem.dfn.as_ref().expect("[dfn]");
    let json = serde_json::to_string(dfn).expect("DfnParams serializes");
    let value: serde_json::Value = serde_json::from_str(&json).expect("round-trips");

    fn count_numbers(v: &serde_json::Value) -> usize {
        match v {
            serde_json::Value::Number(_) => 1,
            serde_json::Value::Array(a) => a.iter().map(count_numbers).sum(),
            serde_json::Value::Object(m) => m.values().map(count_numbers).sum(),
            _ => 0,
        }
    }

    // 2 shared scalars + 3 diffusivity terms + 3 conductivity terms, each a pair
    // + 4 per electrode x 2 + 3 for the separator.
    assert_eq!(
        count_numbers(&value),
        2 + 2 * (3 + 3) + 4 * 2 + 3,
        "the [dfn] value count changed — a field was added or removed. Pin the new \
         value by name in this file before repinning this count; do not just move the \
         number."
    );
}

// ---------------------------------------------------------------------------
// One rejection test per validation rule
// ---------------------------------------------------------------------------

/// A minimal valid chemistry with one `{}` hole where the `[dfn]` block goes, so every
/// rejection test substitutes exactly one section and a failure can only have come from
/// it. Mirrors `spm_chemistry.rs`'s harness.
///
/// Note this fixture has **no `[spm]` section**, and that is a deliberate demonstration
/// rather than an oversight: a `[dfn]` block validates on its own terms. The pairing is
/// diagnosed at build time, where a config asks for the model. See the field doc.
fn chemistry_with_dfn(dfn: &str) -> String {
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

{dfn}
"#
    )
}

/// A well-formed `[dfn]` block, deliberately tiny and round-numbered so a rejection test
/// is reading one broken field rather than scanning a copy of the shipped file.
const GOOD_DFN: &str = r#"
[dfn]
transference_number = 0.26
thermodynamic_factor = 1.0
electrolyte_diffusivity_terms  = [{ coefficient = 5.0e-10, exponent = 0 }]
electrolyte_conductivity_terms = [{ coefficient = 1.0, exponent = 1 }]

[dfn.negative]
porosity = 0.25
bruggeman_electrolyte = 1.5
bruggeman_electrode = 0.0
solid_conductivity_s_per_m = 215.0

[dfn.separator]
thickness_m = 1.2e-5
porosity = 0.47
bruggeman_electrolyte = 1.5

[dfn.positive]
porosity = 0.335
bruggeman_electrolyte = 1.5
bruggeman_electrode = 0.0
solid_conductivity_s_per_m = 0.18
"#;

/// Break exactly one line by **substitution**, not by appending an override: TOML
/// rejects a duplicate key before validation ever runs, which would turn a validation
/// test into a parse test that passes for the wrong reason.
///
/// `replacen(.., 1)` hits the first occurrence, and `[dfn.negative]` is written before
/// `[dfn.positive]`, so a shared line (`porosity`, `bruggeman_electrolyte`) reaches the
/// negative electrode. A plain `replace` would break both at once and the error would
/// name whichever is validated first — the one we wanted, but only by luck.
fn broken(old: &str, new: &str) -> String {
    assert!(
        GOOD_DFN.contains(old),
        "template has no line `{old}` to break"
    );
    GOOD_DFN.replacen(old, new, 1)
}

fn reject(dfn: &str) -> ChemistryError {
    match parse_chemistry(&chemistry_with_dfn(dfn)) {
        Err(DataError::Invalid(e)) => e,
        Err(other) => panic!("expected a validation error, got {other}"),
        Ok(_) => panic!("expected rejection, but the chemistry validated"),
    }
}

/// The template itself must be valid, or every rejection test below is vacuous —
/// passing because the fixture is broken rather than because the rule works.
#[test]
fn the_dfn_template_is_itself_valid() {
    let chem = parse_chemistry(&chemistry_with_dfn(GOOD_DFN)).expect("template must validate");
    assert!(chem.dfn.is_some());
    // And it validates with NO [spm] section, which is the claim the harness's own doc
    // comment makes: `[dfn]` validation is local to the block.
    assert!(chem.spm.is_none());
}

#[test]
fn a_transference_number_outside_zero_to_one_is_rejected() {
    let err = reject(&broken(
        "transference_number = 0.26",
        "transference_number = 1.4",
    ));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what: "dfn.transference_number must be in [0, 1]"
        }
    );
}

/// **Both endpoints are accepted**, and this is the test that keeps them accepted.
///
/// `charge_transfer_alpha` excludes 0 and 1 because each kills a Butler–Volmer branch,
/// and the two fields look similar enough that someone will eventually "align" them.
/// Neither endpoint degenerates anything here: `t₊ = 1` is a single-ion conductor whose
/// concentration never moves, and `t₊ = 0` leaves every term finite.
#[test]
fn a_transference_number_at_either_endpoint_is_accepted() {
    for value in ["0.0", "1.0"] {
        let text = chemistry_with_dfn(&broken(
            "transference_number = 0.26",
            &format!("transference_number = {value}"),
        ));
        parse_chemistry(&text).unwrap_or_else(|e| panic!("t+ = {value} must be accepted, got {e}"));
    }
}

/// A non-positive thermodynamic factor reverses the sign of the concentration
/// overpotential — the electrolyte would push current the way it should resist it.
#[test]
fn a_non_positive_thermodynamic_factor_is_rejected() {
    let err = reject(&broken(
        "thermodynamic_factor = 1.0",
        "thermodynamic_factor = 0.0",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "dfn.thermodynamic_factor",
            value: 0.0
        }
    );
}

/// An empty transport fit is a property with no value at all, not a property that is
/// zero.
#[test]
fn an_empty_transport_fit_is_rejected() {
    let err = reject(&broken(
        "electrolyte_conductivity_terms = [{ coefficient = 1.0, exponent = 1 }]",
        "electrolyte_conductivity_terms = []",
    ));
    assert_eq!(
        err,
        ChemistryError::Empty("dfn.electrolyte_conductivity_terms")
    );
}

/// A fit that is negative at the concentration the cell starts from is broken beyond
/// argument, and that one point is checkable with plain arithmetic.
#[test]
fn a_transport_fit_negative_at_the_reference_concentration_is_rejected() {
    let err = reject(&broken(
        "electrolyte_diffusivity_terms  = [{ coefficient = 5.0e-10, exponent = 0 }]",
        "electrolyte_diffusivity_terms  = [{ coefficient = -5.0e-10, exponent = 0 }]",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "dfn.electrolyte_diffusivity_terms summed at c_e = 1000 mol/m3",
            value: -5.0e-10
        }
    );
}

/// A porosity of 1 is a domain that is pure electrolyte — an electrode with no
/// electrode in it. Both ends of the interval are excluded, unlike
/// `active_volume_fraction`, which admits 1.
#[test]
fn a_porosity_at_either_endpoint_is_rejected() {
    for (value, what) in [
        ("0.0", "dfn.negative.porosity"),
        ("1.0", "dfn.negative.porosity"),
    ] {
        let err = reject(&broken("porosity = 0.25", &format!("porosity = {value}")));
        assert_eq!(err, ChemistryError::BadRange { what });
    }
}

/// The separator gets its own error name, because it is a different domain and an error
/// that said only "porosity" would send the reader to an electrode.
#[test]
fn a_bad_separator_porosity_names_the_separator() {
    let err = reject(&broken("porosity = 0.47", "porosity = 1.2"));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what: "dfn.separator.porosity"
        }
    );
}

/// A negative Bruggeman exponent would make a *less* porous domain conduct *better*.
/// Zero stays legal — it is Chen2020's own value for both electrode phases.
#[test]
fn a_negative_bruggeman_exponent_is_rejected_and_zero_is_not() {
    let err = reject(&broken(
        "bruggeman_electrolyte = 1.5",
        "bruggeman_electrolyte = -1.5",
    ));
    assert_eq!(
        err,
        ChemistryError::Negative {
            what: "dfn.negative.bruggeman_electrolyte",
            value: -1.5
        }
    );
    // The shipped file already carries `bruggeman_electrode = 0.0`, and the template
    // does too, so `the_dfn_template_is_itself_valid` is the zero case — stated here so
    // the pair is read together rather than left to be inferred.
    let chem = parse_chemistry(&chemistry_with_dfn(GOOD_DFN)).expect("zero is legal");
    assert_eq!(
        chem.dfn.expect("[dfn]").negative.bruggeman_electrode,
        0.0,
        "zero must remain a legal Bruggeman exponent"
    );
}

/// A zero solid conductivity is an infinite resistance: the electrode could carry no
/// current at all. Rejected rather than read as "no solid conduction", the same argument
/// `m_ref` refuses zero on.
#[test]
fn a_zero_solid_conductivity_is_rejected() {
    let err = reject(&broken(
        "solid_conductivity_s_per_m = 215.0",
        "solid_conductivity_s_per_m = 0.0",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "dfn.negative.solid_conductivity_s_per_m",
            value: 0.0
        }
    );
}

/// The positive electrode's errors name the positive electrode. The two blocks are
/// identical in shape, so an unprefixed error sends the reader to the wrong one — and
/// on this parameter set the positive is the one that matters, at 0.18 S/m against 215.
#[test]
fn a_bad_positive_electrode_names_the_positive_electrode() {
    let err = reject(&GOOD_DFN.replace(
        "solid_conductivity_s_per_m = 0.18",
        "solid_conductivity_s_per_m = -0.18",
    ));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "dfn.positive.solid_conductivity_s_per_m",
            value: -0.18
        }
    );
}

/// A non-positive separator thickness is a cell whose electrodes touch.
#[test]
fn a_zero_separator_thickness_is_rejected() {
    let err = reject(&broken("thickness_m = 1.2e-5", "thickness_m = 0.0"));
    assert_eq!(
        err,
        ChemistryError::NotPositive {
            what: "dfn.separator.thickness_m",
            value: 0.0
        }
    );
}

/// TOML admits `inf` and `nan` floats, so finiteness is checked rather than assumed —
/// the same reason `[thermal]` checks it explicitly.
#[test]
fn a_non_finite_transport_coefficient_is_rejected() {
    let err = reject(&broken(
        "electrolyte_conductivity_terms = [{ coefficient = 1.0, exponent = 1 }]",
        "electrolyte_conductivity_terms = [{ coefficient = inf, exponent = 1 }]",
    ));
    assert_eq!(
        err,
        ChemistryError::BadRange {
            what: "dfn.electrolyte_conductivity_terms entries must be finite"
        }
    );
}
