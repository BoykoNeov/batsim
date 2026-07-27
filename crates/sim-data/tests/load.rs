//! Chemistry-file loading tests.

use sim_data::{parse_chemistry, DataError};

/// The shipped LFP chemistry must parse and pass validation.
#[test]
fn lfp_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/lfp_26650_generic.toml");
    let chem = parse_chemistry(text).expect("LFP chemistry should load and validate");

    assert_eq!(chem.meta.id, "lfp_26650_generic");
    assert_eq!(chem.n_rc(), 1);
    // Usable capacity between the Prada2013 stoichiometry limits (see the golden
    // pipeline in tools/reference); fitted, not the old datasheet-round 2.5 Ah.
    assert!((chem.cell.capacity_ah - 2.303451).abs() < 1e-6);
    // OCV table is monotone and spans the usable range.
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());
}

/// The shipped NMC chemistry must parse and pass validation (two RC pairs).
#[test]
fn nmc_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/nmc_18650_generic.toml");
    let chem = parse_chemistry(text).expect("NMC chemistry should load and validate");

    assert_eq!(chem.meta.id, "nmc_18650_generic");
    assert_eq!(chem.n_rc(), 2);
    assert!((chem.cell.capacity_ah - 3.0).abs() < 1e-12);
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());
}

/// A minimal but *valid* chemistry, as a format string with one `{}` hole where a
/// section can be swapped out. Every rejection test below substitutes exactly one
/// section, so a failure can only come from that section — and so a missing
/// required section shows up as a parse error in every test at once rather than
/// silently turning a validation test into a schema test.
fn chemistry_with_ocv(ocv: &str) -> String {
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

{ocv}

[r0]
soc = [0.0, 1.0]
temp_k = [298.15]
ohms = [[0.02], [0.02]]

[[rc]]
r_ohms = 0.01
c_farad = 2000.0

[thermal]
heat_capacity_j_per_k = 95.0
h_area_w_per_k = 0.35
"#
    )
}

/// A non-monotone OCV table must be rejected by validation, not silently accepted.
#[test]
fn non_monotone_ocv_is_rejected() {
    let text = chemistry_with_ocv(
        r#"
[ocv]
soc   = [0.0, 0.5, 1.0]
volts = [3.0, 2.9, 3.4]
"#,
    );
    let err = parse_chemistry(&text).expect_err("non-monotone OCV should be rejected");
    assert!(matches!(err, DataError::Invalid(_)), "got {err:?}");
}

/// The optional entropy-coefficient column, when present, must match the length of
/// the SOC axis it shares. A short column would otherwise interpolate against the
/// wrong breakpoints.
#[test]
fn mismatched_docv_dt_length_is_rejected() {
    let text = chemistry_with_ocv(
        r#"
[ocv]
soc            = [0.0, 0.5, 1.0]
volts          = [3.0, 3.2, 3.4]
docv_dt_v_per_k = [-1.0e-4, -2.0e-4]
"#,
    );
    let err = parse_chemistry(&text).expect_err("short docv_dt column should be rejected");
    assert!(matches!(err, DataError::Invalid(_)), "got {err:?}");
}

/// A matching entropy-coefficient column loads, and is *not* sign-constrained:
/// ∂U/∂T legitimately changes sign across the SOC range.
#[test]
fn docv_dt_column_loads_with_mixed_signs() {
    let text = chemistry_with_ocv(
        r#"
[ocv]
soc             = [0.0, 0.5, 1.0]
volts           = [3.0, 3.2, 3.4]
docv_dt_v_per_k = [1.5e-4, -1.0e-4, -3.0e-4]
"#,
    );
    let chem = parse_chemistry(&text).expect("mixed-sign entropy coefficients are valid");
    let docv = chem
        .ocv
        .docv_dt_v_per_k
        .expect("entropy column should be present");
    assert_eq!(docv.len(), 3);
    assert!(docv[0] > 0.0 && docv[2] < 0.0);
}

/// A chemistry with no `[thermal]` section is a *schema* error, not a validation
/// error: the thermal properties are required from Phase 2 on.
#[test]
fn missing_thermal_section_is_a_parse_error() {
    let full = chemistry_with_ocv(
        r#"
[ocv]
soc   = [0.0, 1.0]
volts = [3.0, 3.4]
"#,
    );
    let without_thermal = full
        .split("[thermal]")
        .next()
        .expect("split always yields a first part");
    let err = parse_chemistry(without_thermal).expect_err("missing [thermal] should be rejected");
    assert!(matches!(err, DataError::Toml(_)), "got {err:?}");
}

/// A negative convective conductance is physically meaningless (it would pump heat
/// *into* a cell from a colder environment) and must be rejected.
#[test]
fn negative_h_area_is_rejected() {
    let full = chemistry_with_ocv(
        r#"
[ocv]
soc   = [0.0, 1.0]
volts = [3.0, 3.4]
"#,
    );
    let text = full.replace("h_area_w_per_k = 0.35", "h_area_w_per_k = -0.1");
    let err = parse_chemistry(&text).expect_err("negative h*A should be rejected");
    assert!(matches!(err, DataError::Invalid(_)), "got {err:?}");
}

/// Zero convective conductance is *not* an error: a perfectly insulated cell is a
/// legitimate configuration (and a useful pedagogical one).
#[test]
fn zero_h_area_is_accepted() {
    let full = chemistry_with_ocv(
        r#"
[ocv]
soc   = [0.0, 1.0]
volts = [3.0, 3.4]
"#,
    );
    let text = full.replace("h_area_w_per_k = 0.35", "h_area_w_per_k = 0.0");
    let chem = parse_chemistry(&text).expect("an adiabatic cell is valid");
    assert_eq!(chem.thermal.h_area_w_per_k, 0.0);
}
