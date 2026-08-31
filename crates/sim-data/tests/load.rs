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

/// The shipped LTO chemistry must parse and pass validation. Added with **zero lines of
/// engine code changed** — the first chemistry in this repo for which that is true, and the
/// test of `CLAUDE.md` principle 10. See `docs/plans/phase-8-slice-a-lto.md`, and
/// `tests/lto_chemistry.rs` for what the file can do that the others cannot.
#[test]
fn lto_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/lto_20ah_generic.toml");
    let chem = parse_chemistry(text).expect("LTO chemistry should load and validate");

    assert_eq!(chem.meta.id, "lto_20ah_generic");
    assert_eq!(chem.n_rc(), 2);
    assert!((chem.cell.capacity_ah - 20.0).abs() < 1e-12);
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());
    // The whole point of the file: a 2.7 V lithium cell validates against rules written for
    // 3.3 V and 3.7 V ones, exactly as the 2 V lead-acid file already showed they would.
    assert!(chem.cell.v_max < 3.0);
}

/// The shipped NiMH chemistry must parse and pass validation, and it is the first shipped
/// file to exercise either of `SNAPSHOT_VERSION` 18's two new schema additions.
///
/// Deliberately **not** a zero-code chemistry, unlike the LTO file above: this one is what
/// the version bump exists for. What it can do lives in `tests/nimh_chemistry.rs`.
#[test]
fn nimh_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/nimh_subc_3ah_generic.toml");
    let chem = parse_chemistry(text).expect("NiMH chemistry should load and validate");

    assert_eq!(chem.meta.id, "nimh_subc_3ah_generic");
    assert_eq!(chem.n_rc(), 2);
    assert!((chem.cell.capacity_ah - 3.0).abs() < 1e-12);
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());
    // A 1.2 V cell validates against rules written for 3.3 V and 3.7 V ones — the same
    // point the 2 V lead-acid and 2.7 V LTO files already made, one step further down.
    assert!(chem.cell.v_max < 2.0);

    // The two new sections, and the reason this file is not zero-code.
    let hyst = chem.hysteresis.expect("NiMH declares [hysteresis]");
    assert!(hyst.scale_v > 0.0 && hyst.gamma > 0.0);
    assert_eq!(chem.ocv.t_ref_k, Some(298.15));
    let docv_dt = chem
        .ocv
        .docv_dt_v_per_k
        .as_ref()
        .expect("t_ref_k is meaningless without the coefficient, and validation says so");
    assert_eq!(docv_dt.len(), chem.ocv.soc.len());

    // Three absences that are decisions rather than omissions; see the file's own header.
    assert!(chem.aging.is_none(), "no fitted NiMH cycle life is in hand");
    assert!(
        chem.safety.is_none(),
        "a cell with no lithium in it neither plates nor runs away, and dropping the \\
         section is how this file says so"
    );
    assert!(chem.diffusion.is_none(), "no fitted NiMH Peukert exponent");
}

/// The shipped sodium-ion chemistry must parse and pass validation. Added with **zero lines
/// of engine code changed**, like the LTO file, and the first chemistry added after Phase 8
/// closed — against that phase's written recipe. See `docs/plans/sodium-ion-chemistry.md`,
/// and `tests/na_ion_chemistry.rs` for what the file can do that the others cannot.
///
/// It is also the first shipped file whose capacity, OCV table, `[r0]` SOC axis and RC pairs
/// are **fitted from raw laboratory measurements of a physical cell** rather than placed by
/// hand or extracted from someone else's model.
#[test]
fn na_ion_chemistry_loads_and_validates() {
    let text = include_str!("../../../chemistries/na_ion_18650_generic.toml");
    let chem = parse_chemistry(text).expect("Na-ion chemistry should load and validate");

    assert_eq!(chem.meta.id, "na_ion_18650_generic");
    assert_eq!(chem.n_rc(), 2);
    // Measured discharge throughput of the source's incremental-OCV run, not the rated
    // 1.5 Ah — the OCV table's SOC axis is normalised to this same number.
    assert!((chem.cell.capacity_ah - 1.4558).abs() < 1e-12);
    assert_eq!(chem.ocv.soc.len(), chem.ocv.volts.len());

    // The second file to declare [hysteresis], and the first written by a slice other than
    // the one that built the mechanism.
    let hyst = chem.hysteresis.expect("Na-ion declares [hysteresis]");
    assert!((hyst.scale_v - 0.010).abs() < 1e-12);
    // ...but it does NOT take the other v18 addition: no dU/dT was measured for this cell.
    assert!(chem.ocv.t_ref_k.is_none());

    // Unlike LTO, this cell keeps a plating gate: hard carbon has no structural argument
    // that it cannot plate, so omitting the pair would assert what the sources do not say.
    let safety = chem.safety.expect("Na-ion declares [safety]");
    assert!(safety.t_plating_min_k.is_some() && safety.plating_c_threshold.is_some());
}

/// **No chemistry that predates `SNAPSHOT_VERSION` 18 may have gained either new section.**
/// This is Phase 8's exit criterion 3 written as a test rather than as an argument: both
/// absences are *paths* in the engine rather than neutral zeros, so an absence here is what
/// makes "no existing trajectory moved across v18" structural. If a future edit adds either
/// section to one of these files, that claim stops holding and this is where it says so.
///
/// The list is closed on purpose and is **not** "every file but the NiMH one" any more:
/// `na_ion_18650_generic` declares `[hysteresis]` legitimately, and a file that no pack
/// loaded before v18 cannot move a v17 trajectory whatever it declares. What this test
/// guards is the files that *did* exist.
#[test]
fn no_chemistry_predating_v18_carries_the_v18_sections() {
    for (id, text) in [
        (
            "lfp_26650_generic",
            include_str!("../../../chemistries/lfp_26650_generic.toml"),
        ),
        (
            "nmc_18650_generic",
            include_str!("../../../chemistries/nmc_18650_generic.toml"),
        ),
        (
            "nmc_21700_lgm50",
            include_str!("../../../chemistries/nmc_21700_lgm50.toml"),
        ),
        (
            "pba_agm_2v_generic",
            include_str!("../../../chemistries/pba_agm_2v_generic.toml"),
        ),
        (
            "lto_20ah_generic",
            include_str!("../../../chemistries/lto_20ah_generic.toml"),
        ),
    ] {
        let chem = parse_chemistry(text).unwrap_or_else(|e| panic!("{id} should load: {e}"));
        assert!(
            chem.hysteresis.is_none(),
            "{id} gained a [hysteresis] section, which moves its trajectories"
        );
        assert!(
            chem.ocv.t_ref_k.is_none(),
            "{id} gained an ocv.t_ref_k, which moves its trajectories"
        );
    }
}

/// A reference temperature with no coefficient column describes a correction that is
/// identically zero — a file saying something it does not mean — and is refused.
///
/// This is the one **cross-section** check in the OCV block, and it is there because it
/// catches a file going *silent*. Compare the `[diffusion]` block, which deliberately makes
/// no cross-checks at all because a badly sized value there binds visibly in the telemetry.
#[test]
fn a_reference_temperature_without_a_coefficient_is_rejected() {
    let toml = chemistry_with_ocv(
        r#"
[ocv]
soc = [0.0, 1.0]
volts = [3.0, 4.2]
t_ref_k = 298.15
"#,
    );
    let err = parse_chemistry(&toml).expect_err("t_ref_k alone must be refused");
    assert!(
        matches!(err, DataError::Invalid(_)),
        "expected a validation error, got {err:?}"
    );
}

/// And the same pair the other way round is *accepted*, because it is the pre-v18
/// configuration every shipped file with an entropy column would be in: the coefficient
/// drives heat and nothing else.
#[test]
fn a_coefficient_without_a_reference_temperature_is_accepted() {
    let toml = chemistry_with_ocv(
        r#"
[ocv]
soc = [0.0, 1.0]
volts = [3.0, 4.2]
docv_dt_v_per_k = [-1.0e-4, -2.0e-4]
"#,
    );
    let chem = parse_chemistry(&toml).expect("a heat-only entropy column stays legal");
    assert!(chem.ocv.docv_dt_v_per_k.is_some());
    assert!(chem.ocv.t_ref_k.is_none());
}

/// A non-positive reference temperature is not a temperature. Kelvin, like every other
/// temperature in the schema.
#[test]
fn a_non_positive_reference_temperature_is_rejected() {
    for bad in ["0.0", "-1.0"] {
        let toml = chemistry_with_ocv(&format!(
            r#"
[ocv]
soc = [0.0, 1.0]
volts = [3.0, 4.2]
docv_dt_v_per_k = [-1.0e-4, -2.0e-4]
t_ref_k = {bad}
"#
        ));
        let err = parse_chemistry(&toml).expect_err("a non-positive t_ref_k must be refused");
        assert!(
            matches!(err, DataError::Invalid(_)),
            "expected a validation error for t_ref_k = {bad}, got {err:?}"
        );
    }
}

/// The `[hysteresis]` section's own two numbers, checked positive-and-finite the way
/// `[diffusion]`'s four are — and, like those, checked against nothing else. A loop wide
/// enough to push a rested cell outside its own voltage limits is legal here and binds
/// visibly in the telemetry instead; the sizing rule lives in the field's doc where a
/// reader can apply judgement to it.
#[test]
fn a_non_positive_hysteresis_parameter_is_rejected() {
    for (label, section) in [
        ("zero half-width", "scale_v = 0.0\ngamma = 25.0"),
        ("negative rate", "scale_v = 0.02\ngamma = -1.0"),
        ("infinite half-width", "scale_v = inf\ngamma = 25.0"),
        ("nan rate", "scale_v = 0.02\ngamma = nan"),
    ] {
        let toml = chemistry_with_ocv(&format!(
            "\n[ocv]\nsoc = [0.0, 1.0]\nvolts = [3.0, 4.2]\n\n[hysteresis]\n{section}\n"
        ));
        let err = parse_chemistry(&toml)
            .expect_err("a non-positive or non-finite hysteresis parameter must be refused");
        assert!(
            matches!(err, DataError::Invalid(_)),
            "expected a validation error for {label}, got {err:?}"
        );
    }
}

/// The plating gate is a matched pair, and half of it is refused.
///
/// `t_plating_min_k` and `plating_c_threshold` became optional at `SNAPSHOT_VERSION` 19 so
/// that a chemistry can say it has no plating mechanism by omitting them. Omitting exactly
/// one says nothing coherent: a threshold with no temperature gate parameterises nothing,
/// and a gate with no threshold cannot be evaluated.
///
/// **This rule is also the typo guard, and that is the larger half of its value.** The
/// loader does not deny unknown TOML keys, so a misspelled key is indistinguishable from an
/// absent one — and an absent gate now *means* something, where before it was a parse
/// error. Misspelling one key of a matched pair is caught here; misspelling both is not a
/// plausible accident. See `docs/plans/plating-absence.md`.
#[test]
fn half_a_plating_gate_is_rejected() {
    for (label, section) in [
        (
            "a temperature with no threshold",
            "t_plating_min_k = 273.15",
        ),
        (
            "a threshold with no temperature",
            "plating_c_threshold = 0.5",
        ),
    ] {
        let toml = chemistry_with_ocv(&format!(
            "\n[ocv]\nsoc = [0.0, 1.0]\nvolts = [3.0, 4.2]\n\n[safety]\n\
             t_onset_k = 423.15\nt_vent_k = 453.15\nrunaway_energy_j = 60.0e3\n{section}\n"
        ));
        let err = parse_chemistry(&toml).expect_err("half a plating gate must be refused");
        assert!(
            matches!(err, DataError::Invalid(_)),
            "expected a validation error for {label}, got {err:?}"
        );
    }
}

/// A chemistry with no plating gate must not price plating.
///
/// **Deliberately stricter than the convention the surrounding fields use**, and the
/// strictness is the point. `plating_fade_per_ah`, `plating_short_hazard_per_ah` and
/// `runaway_power_w_at_onset` all treat zero as "this mechanism is inert", so the local
/// precedent would be to ignore a stated cost rather than refuse it. Two reasons not to.
/// A fade-per-amp-hour quoted for a mechanism the cell does not have is an unlabelled
/// physical constant describing nothing, which `CLAUDE.md`'s provenance rule forbids
/// outright. And it is what **replaces a tripwire this schema change deleted**: the LTO
/// file used to spell "never" as a one-kelvin sentinel with a deliberately permissive
/// threshold behind it, so that anyone "correcting" the absurd temperature upward got a
/// loud wrong answer instead of a plausible one. There is no number to correct now, and
/// what is left to catch is the other half — a file that prices plating while claiming to
/// have none.
#[test]
fn plating_costs_without_a_plating_gate_are_rejected() {
    for (label, cost) in [
        ("a fade", "plating_fade_per_ah = 2.0e-4"),
        ("a short hazard", "plating_short_hazard_per_ah = 1.0e-4"),
        ("a short resistance", "plating_short_ohms = 5.0"),
    ] {
        let toml = chemistry_with_ocv(&format!(
            "\n[ocv]\nsoc = [0.0, 1.0]\nvolts = [3.0, 4.2]\n\n[safety]\n\
             t_onset_k = 423.15\nt_vent_k = 453.15\nrunaway_energy_j = 60.0e3\n{cost}\n"
        ));
        let err = parse_chemistry(&toml)
            .expect_err("a plating cost with no plating gate must be refused");
        assert!(
            matches!(err, DataError::Invalid(_)),
            "expected a validation error for {label} with no gate, got {err:?}"
        );
    }
}

/// The case the whole change exists for: `[safety]` with the runaway half and **no plating
/// gate at all** loads, validates, and reads back as an absent mechanism.
///
/// This is the shape `chemistries/lto_20ah_generic.toml` ships. Before v19 it was
/// impossible — a chemistry that wanted thermal runaway had to accept a plating gate too,
/// because one `Option` covers both mechanisms — and the LTO file spelled "never" as a
/// one-kelvin sentinel instead. The positive assertion matters as much as the two
/// rejections above it: a rule that refuses everything is not a rule.
#[test]
fn safety_without_a_plating_gate_loads() {
    let toml = chemistry_with_ocv(
        "\n[ocv]\nsoc = [0.0, 1.0]\nvolts = [3.0, 4.2]\n\n[safety]\n\
         t_onset_k = 423.15\nt_vent_k = 453.15\nrunaway_energy_j = 60.0e3\n",
    );
    let chem = parse_chemistry(&toml).expect("a safety section with no plating gate is valid");
    let safety = chem.safety.expect("the section is present");
    assert_eq!(
        safety.t_plating_min_k, None,
        "the gate is absent, which is how this cell says it cannot plate"
    );
    assert_eq!(safety.plating_c_threshold, None, "and so is its threshold");
    // The runaway half is untouched by the absence, which is the thing that was not
    // separable before v19.
    assert_eq!(safety.t_onset_k, 423.15);
    assert_eq!(safety.t_vent_k, 453.15);
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

[reversal]
v_per_soc = 100.0
floor_v   = 0.0
# Zero: this fixture pays nothing for over-discharge, so it exercises the loader without
# moving any trajectory. See docs/plans/reversal-damage.md.
fade_per_ah = 0.0

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

/// Every shipped chemistry's aging coefficients must produce a *plausible* fade
/// curve, not merely a valid one.
///
/// This test exists because of a bug it would have caught two phases earlier. Every
/// `[aging]` number is a labelled placeholder, and `validate()` checks each one in
/// isolation — finite, non-negative, exponent ≥ 1. But `cal_pre_exp` and
/// `cal_ea_j_per_mol` only mean anything as a **pair**: the pre-exponential is the
/// Arrhenius factor's scale at the chosen activation energy, so changing either alone
/// moves the answer by orders of magnitude. NMC shipped a pair giving ~260 % calendar
/// fade in a year at 25 °C — a pack dead within weeks — and nothing noticed, because
/// nothing had ever evaluated the pair. Each number looked fine on its own, and each
/// carried a provenance note.
///
/// So the guard is a **band**, not a fitted number, which keeps it consistent with the
/// project rule that scenarios assert shape rather than magnitude. It says only "a
/// battery that loses between 1 % and 50 % of its capacity in a year on the shelf at
/// room temperature", which is wide enough that any honest placeholder passes and
/// narrow enough that an unevaluated coefficient pair does not. The same shape of
/// check is what `[safety]`'s onset/vent/energy-budget numbers will want when they are
/// wired in.
#[test]
fn shipped_aging_coefficients_give_a_plausible_one_year_fade() {
    const YEAR_S: f64 = 365.0 * 24.0 * 3600.0;
    const ROOM_K: f64 = 298.15;

    for (name, text) in [
        (
            "lfp",
            include_str!("../../../chemistries/lfp_26650_generic.toml"),
        ),
        (
            "nmc",
            include_str!("../../../chemistries/nmc_18650_generic.toml"),
        ),
        (
            "lto",
            include_str!("../../../chemistries/lto_20ah_generic.toml"),
        ),
        (
            "na-ion",
            include_str!("../../../chemistries/na_ion_18650_generic.toml"),
        ),
    ] {
        let chem = parse_chemistry(text).expect("shipped chemistry loads");
        let aging = chem
            .aging
            .as_ref()
            .unwrap_or_else(|| panic!("{name} must ship [aging] coefficients"));

        // Worst realistic storage condition the coefficients describe: room
        // temperature, fully charged.
        let k = sim_core::aging::calendar_rate(aging, ROOM_K, 1.0);
        let fade = k * YEAR_S.sqrt();
        assert!(
            (0.01..=0.5).contains(&fade),
            "{name}: one year at 25 C and full SOC fades {:.1} % — outside the \
             plausible 1–50 % band, so cal_pre_exp and cal_ea_j_per_mol are not a \
             sane pair",
            fade * 100.0
        );

        // Cycle fade, same treatment, but expressed as the quantity a datasheet
        // actually quotes: how many full cycles this cell survives before losing a
        // fifth of its capacity. Throughput counts both directions, hence the factor
        // of two.
        //
        // This used to read "500 full cycles fade 1–50 %", which is the same statement
        // for a graphite cell and the *wrong* statement for a cell built to outlive
        // one. The LTO file rates 20,000 cycles, so 500 of them cost it 0.75 % and it
        // fell out of the bottom of that band while being exactly right. Rewriting the
        // check per-cell rather than per-500-cycles tightens the floor (200 → 300
        // cycles) and loosens the ceiling (10,000 → 50,000), and the loosening is the
        // part LTO needs: a band that cannot admit a long-life chemistry is asserting
        // a chemistry assumption, not a plausibility one.
        let per_cycle = sim_core::aging::cycle_increment(aging, 2.0 * chem.cell.capacity_ah, 1.0);
        let cycles_to_20_percent = 0.2 / per_cycle;
        assert!(
            (300.0..=50_000.0).contains(&cycles_to_20_percent),
            "{name}: {cycles_to_20_percent:.0} full cycles to 20 % capacity loss — outside the plausible 300–50,000 band, so cyc_fade_per_ah is not a sane value for this cell"
        );
    }
}

/// The same treatment for the shipped `[safety]` plating coefficients, and for the
/// same reason: three numbers that each pass `validate()` in isolation can still be an
/// implausible set together.
///
/// The unit of comparison is **one full cold charge above the C-rate threshold** — the
/// event these coefficients exist to describe. Two bands, both wide:
///
/// * Fade per cold charge in 0.05–5 %. Below the floor, plating is a rounding error and
///   nothing a student does will show it; above the ceiling, twenty cold charges scrap
///   the pack and the fade curve is a cliff rather than a curve.
/// * Short probability per cold charge in 0.01–5 %. The same argument: a hazard that
///   never fires teaches nothing, and one that fires on the second charge stops being
///   the rare stochastic outcome it is meant to model.
///
/// Both bands deliberately say nothing about where in them a fitted value would land —
/// this is a guard against an unevaluated set, not a substitute for the fit the
/// provenance notes still ask for. See
/// `shipped_runaway_coefficients_burn_at_a_plausible_scale` for the same treatment of
/// the runaway half.
#[test]
fn shipped_plating_coefficients_give_a_plausible_cold_charge_cost() {
    for (name, text) in [
        (
            "lfp",
            include_str!("../../../chemistries/lfp_26650_generic.toml"),
        ),
        (
            "nmc",
            include_str!("../../../chemistries/nmc_18650_generic.toml"),
        ),
        (
            "lto",
            include_str!("../../../chemistries/lto_20ah_generic.toml"),
        ),
        (
            "na-ion",
            include_str!("../../../chemistries/na_ion_18650_generic.toml"),
        ),
    ] {
        let chem = parse_chemistry(text).expect("shipped chemistry loads");
        let safety = chem
            .safety
            .as_ref()
            .unwrap_or_else(|| panic!("{name} must ship [safety] coefficients"));

        // A chemistry either prices plating or has no plating gate at all, and this arm
        // is the second half of that. LTO's anode plateau sits ~1.55 V above the
        // potential at which lithium deposits, so the cell has no plating mechanism to
        // price, and it says so by omitting the gate — which `validate` already ties to
        // the cost fields being absent. Nothing is left to check on that branch beyond
        // restating the tie, which is done here rather than assumed because this test's
        // subject is the *shipped files*, not the schema.
        //
        // **This arm got stricter when the schema learned to say "does not plate".** It
        // used to read "no cost declared, so the gate must be unreachable" — a
        // temperature strictly below the cell's own charge floor — because a file with
        // no plating had no other way to be written, and "no coefficients" is also what
        // a half-written file looks like. The two are now distinguishable at load, so
        // the weaker rule is replaced by the stronger one: a shipped file that carries a
        // gate must price it. The schema still permits a priced-at-zero gate ("the flag
        // is raised and costs nothing"); a shipped file with a `TODO fit` where its
        // plating cost should be is what this catches. See docs/plans/plating-absence.md.
        let prices_plating =
            safety.plating_fade_per_ah > 0.0 || safety.plating_short_hazard_per_ah > 0.0;
        if safety.t_plating_min_k.is_none() {
            assert!(
                !prices_plating,
                "{name}: no plating gate, so nothing may price plating — and `validate` \
                 should have refused this file before the test saw it"
            );
            continue;
        }
        assert!(
            prices_plating,
            "{name}: a plating gate is declared at {:?} K but nothing prices plating. \
             Either the coefficients are missing, or this cell does not plate and the \
             gate should be dropped entirely.",
            safety.t_plating_min_k
        );

        // One full charge's worth of plated throughput.
        let ah = chem.cell.capacity_ah;
        let fade = sim_core::plating::plating_fade_increment(safety, ah);
        assert!(
            (0.000_5..=0.05).contains(&fade),
            "{name}: one full cold charge fades {:.3} % — outside the plausible \
             0.05–5 % band, so plating_fade_per_ah is not a sane value for this cell",
            fade * 100.0
        );

        let p = sim_core::plating::short_probability(safety, ah);
        assert!(
            (0.000_1..=0.05).contains(&p),
            "{name}: one full cold charge carries a {:.3} % chance of a soft short — \
             outside the plausible 0.01–5 % band",
            p * 100.0
        );

        // A soft short must be soft: far above the cell's own ohmic resistance, or it
        // is a hard short wearing the wrong name, and the cell dies the instant it
        // forms rather than draining over hours.
        let r0_mid = sim_core::ecm::r0_lookup(&chem.r0, 0.5, 298.15);
        assert!(
            safety.plating_short_ohms > 100.0 * r0_mid,
            "{name}: a {} ohm short against a {r0_mid} ohm cell is not a *soft* short",
            safety.plating_short_ohms
        );
    }
}

/// And the same treatment for the runaway half of `[safety]`, which had the identical
/// exposure: five numbers that each pass `validate()` alone but only mean anything as a
/// set. Both shipped files failed one of the checks below when this test was written —
/// `runaway_energy_j` was set with no reference to `heat_capacity_j_per_k`, so NMC's
/// budget implied a cell reaching 1636 K above onset.
///
/// Two shape checks, both derived from quantities a reader can look up rather than from
/// a fit:
///
/// * **Adiabatic ceiling** = `runaway_energy_j / heat_capacity_j_per_k`, the temperature
///   rise a fully-reacted cell produces with nowhere to put the heat. Banded 100–1200 K.
///   Below the floor the cell cannot even reach its own vent threshold and "runaway" is
///   a misnomer; above it the cell is hotter than steel melts and the budget was written
///   without looking at the heat capacity beside it.
/// * **Adiabatic onset-to-vent time**, integrated through the *engine's own*
///   [`sim_core::runaway::reaction_power`] so the test cannot drift from the model.
///   Banded 1 s–1 h. This is the check with teeth, because it is the only one that
///   touches `runaway_power_w_at_onset` and `runaway_ea_j_per_mol` — the two invented
///   coefficients, which are exactly the ones nobody can eyeball.
#[test]
fn shipped_runaway_coefficients_burn_at_a_plausible_scale() {
    for (name, text) in [
        (
            "lfp",
            include_str!("../../../chemistries/lfp_26650_generic.toml"),
        ),
        (
            "nmc",
            include_str!("../../../chemistries/nmc_18650_generic.toml"),
        ),
        (
            "lto",
            include_str!("../../../chemistries/lto_20ah_generic.toml"),
        ),
        (
            "na-ion",
            include_str!("../../../chemistries/na_ion_18650_generic.toml"),
        ),
    ] {
        let chem = parse_chemistry(text).expect("shipped chemistry loads");
        let safety = chem
            .safety
            .as_ref()
            .unwrap_or_else(|| panic!("{name} must ship [safety] coefficients"));
        let c_th = chem.thermal.heat_capacity_j_per_k;

        let ceiling_k = safety.runaway_energy_j / c_th;
        assert!(
            (100.0..=1200.0).contains(&ceiling_k),
            "{name}: a full burn raises the cell {ceiling_k:.0} K — outside the \
             plausible 100–1200 K band, so runaway_energy_j was not set against this \
             cell's heat_capacity_j_per_k"
        );
        let margin_k = safety.t_vent_k - safety.t_onset_k;
        assert!(
            ceiling_k > margin_k,
            "{name}: a full burn ({ceiling_k:.0} K) cannot even span onset to vent \
             ({margin_k:.0} K), so no cell can ever reach t_vent_k by reacting"
        );

        // Adiabatic integration from onset to vent: dT/dt = Q(T, α)/C_th with the cell
        // isolated. Fixed 1 ms steps, far below the reaction's own timescale here, and
        // bounded so a non-reacting set fails the band rather than hanging.
        let mut t = safety.t_onset_k;
        let mut energy_left = safety.runaway_energy_j;
        let h = 1.0e-3;
        let mut elapsed = 0.0;
        while t < safety.t_vent_k && elapsed < 7200.0 {
            let q = sim_core::runaway::reaction_power(safety, t, energy_left);
            t += h * q / c_th;
            energy_left = (energy_left - q * h).max(0.0);
            elapsed += h;
        }
        assert!(
            (1.0..=3600.0).contains(&elapsed),
            "{name}: an adiabatic cell sitting exactly at onset vents after {elapsed:.1} s \
             — outside the plausible 1 s–1 h band, so runaway_power_w_at_onset and \
             runaway_ea_j_per_mol were not evaluated together"
        );
    }
}
