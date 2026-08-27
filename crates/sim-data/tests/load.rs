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
    ] {
        let chem = parse_chemistry(text).expect("shipped chemistry loads");
        let safety = chem
            .safety
            .as_ref()
            .unwrap_or_else(|| panic!("{name} must ship [safety] coefficients"));

        // A chemistry either prices plating or cannot reach it, and this arm is the
        // second half of that. LTO's anode plateau sits ~1.55 V above the potential at
        // which lithium deposits, so the cell has no plating mechanism to price — but
        // "no coefficients" is also what a half-written file looks like, and the two
        // must not be confused. So a chemistry that declares no cost has to *also*
        // show its gate is shut: `t_plating_min_k` strictly below the coldest
        // temperature the cell is rated to charge at, so the flag cannot rise anywhere
        // inside its own operating window.
        //
        // This is the second place in the repo that assumed every cell plates — the
        // first being `[safety]` itself, which has no way to express the absence of the
        // mechanism at all. See docs/plans/phase-8-slice-a-lto.md.
        let prices_plating =
            safety.plating_fade_per_ah > 0.0 || safety.plating_short_hazard_per_ah > 0.0;
        if !prices_plating {
            assert!(
                safety.t_plating_min_k < chem.cell.t_charge_min_k,
                "{name}: no plating cost is declared, so the plating gate must be unreachable — but t_plating_min_k {} is not below the cell's own charge floor {}. Either the coefficients are missing or the threshold is wrong.",
                safety.t_plating_min_k,
                chem.cell.t_charge_min_k
            );
            continue;
        }

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
