//! Chemistry-file loading tests.

use sim_data::{parse_chemistry, DataError};

/// The shipped LFP chemistry must parse and pass validation.
#[test]
fn lfp_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/lfp_26650_generic.toml");
    let chem = parse_chemistry(text).expect("LFP chemistry should load and validate");

    assert_eq!(chem.meta.id, "lfp_26650_generic");
    assert_eq!(chem.n_rc(), 1);
    assert!((chem.cell.capacity_ah - 2.5).abs() < 1e-12);
    // OCV table is monotone and spans the usable range.
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());
}

/// A non-monotone OCV table must be rejected by validation, not silently accepted.
#[test]
fn non_monotone_ocv_is_rejected() {
    let text = r#"
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
volts = [3.0, 2.9, 3.4]

[r0]
soc = [0.0, 1.0]
temp_k = [298.15]
ohms = [[0.02], [0.02]]

[[rc]]
r_ohms = 0.01
c_farad = 2000.0
"#;
    let err = parse_chemistry(text).expect_err("non-monotone OCV should be rejected");
    assert!(matches!(err, DataError::Invalid(_)), "got {err:?}");
}
