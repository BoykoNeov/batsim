//! The single-particle cell model driven against the **shipped** LG M50 chemistry.
//!
//! # Why these live in `sim-data` rather than in `sim-core`
//! `sim-core` cannot read a file — it is the purity rule — so a `sim-core` test can
//! only exercise a chemistry hand-built in Rust. Everything here is an assertion
//! about `chemistries/nmc_21700_lgm50.toml` *specifically*, and naming the file is
//! part of the claim rather than an incidental fixture. The engine-invariant half of
//! the slice (grid convergence, `dt`-independence, snapshot round-trip, the energy
//! balance) is in `sim-core/tests/spm_cell.rs`, where it belongs.
//!
//! The sharpest example is [`aging_grows_the_dc_resistance_of_the_shipped_spm_cell`]:
//! this chemistry has `contact_resistance_ohm = 0`, Chen2020's own value, so an
//! implementation that let `soh_resistance` multiply only that field would fade
//! capacity with **exactly zero** resistance growth — the one thing `CLAUDE.md`
//! forbids outright. Written against a chemistry with a nonzero contact resistance
//! the same test passes while the shipped file is broken.

use sim_core::{
    AgingConfig, CellModelConfig, Demand, Env, Pack, PackConfig, Scatter, ThermalConfig,
};
use sim_data::{parse_chemistry, parse_scenario};

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

/// Shell count for every test here. Ten resolves the radial gradient well enough
/// that the quasi-static assertions below are about the model rather than about the
/// grid, and cheaply enough that a 3000-step discharge is instant. Slice E measures
/// the accuracy-vs-cost curve and picks a documented default; this is not it.
const SHELLS: usize = 10;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn cfg(initial_soc: f64) -> PackConfig {
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
        cell_model: CellModelConfig::Spm { shells: SHELLS },
    }
}

fn lgm50_pack(initial_soc: f64) -> Pack {
    Pack::new(
        &cfg(initial_soc),
        parse_chemistry(LGM50).expect("LG M50 parses"),
    )
    .expect("an SPM pack on a chemistry with an [spm] section builds")
}

/// Terminal voltage of a resting pack, from a zero-length probe step: no state
/// moves, and telemetry still reports the voltage the pack solves at.
fn rest_voltage(pack: &mut Pack) -> f64 {
    pack.step(0.0, Demand::Rest, &env()).v_terminal
}

// ---------------------------------------------------------------------------
// The mapping, the tables and the signs — all in one assertion
// ---------------------------------------------------------------------------

/// The open-circuit curve this model builds out of two half-cell OCPs lands on the
/// cell's own voltage window and rises monotonically between the ends.
///
/// # Why this is the first test in the slice
/// It is the cheapest check that covers the most ways to be wrong. Passing it
/// requires **all** of: the SOC-to-stoichiometry mapping running the right way on
/// each electrode (they run in *opposite* directions — see
/// `ElectrodeParams::stoich_min`), both OCP tables being interpolated on the right
/// axis, and the terminal voltage being assembled as `U_p − U_n` rather than the
/// negation. Get any one backwards and the window inverts or collapses; every harder
/// assertion in this slice is built on top of it.
///
/// The tolerance is not arbitrary. Chen2020's stoichiometry limits come from
/// PyBaMM's `get_min_max_stoichiometries`, which *defines* them as the windows that
/// put the cell at exactly its published cut-offs — so agreement is expected to
/// within the OCP tables' own interpolation error, which the chemistry file
/// documents as 1.90 mV and 1.88 mV. 30 mV leaves room for both tables plus the
/// adaptive grid's placement, and is still an order of magnitude tighter than the
/// ~1.7 V window it is checking.
#[test]
fn shipped_spm_spans_the_cells_own_voltage_window() {
    let (v_max, v_min) = {
        let chem = parse_chemistry(LGM50).expect("LG M50 parses");
        (chem.cell.v_max, chem.cell.v_min)
    };
    const TOL_V: f64 = 0.030;

    let full = rest_voltage(&mut lgm50_pack(1.0));
    assert!(
        (full - v_max).abs() < TOL_V,
        "a full SPM cell should rest at the chemistry's own upper cut-off \
         {v_max} V, got {full} V — check the stoichiometry mapping direction on \
         each electrode before the OCP tables"
    );

    let empty = rest_voltage(&mut lgm50_pack(0.0));
    assert!(
        (empty - v_min).abs() < TOL_V,
        "an empty SPM cell should rest at the lower cut-off {v_min} V, got {empty} V"
    );

    // Monotone in between: charging drives the negative stoichiometry up and the
    // positive one down together, and both half-cell OCPs run downhill, so the cell
    // voltage rises. A sign error on one electrode alone leaves the endpoints
    // roughly plausible and this sweep is what catches it.
    let mut previous = f64::NEG_INFINITY;
    for k in 0..=20 {
        let soc = f64::from(k) / 20.0;
        let v = rest_voltage(&mut lgm50_pack(soc));
        assert!(
            v > previous,
            "open-circuit voltage must rise with state of charge: at soc = {soc} \
             it fell from {previous} V to {v} V"
        );
        previous = v;
    }
}

// ---------------------------------------------------------------------------
// The two aging multipliers, against this file by name
// ---------------------------------------------------------------------------

/// An aged SPM cell pushes back harder — measured as DC resistance, on the shipped
/// chemistry, whose only ohms-valued field is zero.
///
/// # This is the test the finding was made for
/// `nmc_21700_lgm50.toml` sets `contact_resistance_ohm = 0` because that is
/// Chen2020's published value. So the obvious way to apply `soh_resistance` — scale
/// the model's one resistance — evaluates to `1.0 × 0 = 0` and an aged cell fades
/// capacity with no resistance growth at all. The implementation instead divides the
/// exchange-current density `i_0` on **both** electrodes, which multiplies the
/// linearized charge-transfer resistance `R·T/(F·i_0·A)` by exactly the factor. This
/// test would pass on a chemistry with a nonzero contact resistance whatever the
/// implementation did, which is why it names this one.
///
/// DC resistance is measured as `ΔV/ΔI` between two probe steps at the same state —
/// zero-length, so neither moves the cell and the comparison is of one state under
/// two loads.
#[test]
fn aging_grows_the_dc_resistance_of_the_shipped_spm_cell() {
    let dc_resistance = |soh_resistance: f64| {
        let chem = parse_chemistry(LGM50).expect("LG M50 parses");
        let mut pack = Pack::new(&cfg(0.6), chem).expect("pack builds");
        pack.set_cell_factors(0, 0, 1.0, soh_resistance)
            .expect("cell 0S0P exists");
        let open = pack.step(0.0, Demand::Current(0.0), &env()).v_terminal;
        let loaded = pack.step(0.0, Demand::Current(5.0), &env()).v_terminal;
        (open - loaded) / 5.0
    };

    let healthy = dc_resistance(1.0);
    let aged = dc_resistance(1.5);
    assert!(
        healthy > 0.0,
        "a healthy SPM cell must have a positive DC resistance, got {healthy} ohms"
    );
    assert!(
        aged > healthy * 1.2,
        "a 1.5x resistance-growth factor must show up as a substantially higher DC \
         resistance: healthy {healthy} ohms -> aged {aged} ohms. The shipped \
         chemistry's contact_resistance_ohm is 0, so this only moves if the factor \
         reaches the exchange-current density."
    );
}

/// The capacity multipliers reach the particles: a cell configured to hold less
/// delivers proportionally fewer amp-hours between the same two SOC readings.
///
/// # Why the protocol is quasi-static
/// The equality `measured Ah = nominal × capacity_factor × soh_capacity` is exact
/// only at **equilibrium**. Under load the surface concentration runs ahead of the
/// bulk, the voltage cut-off arrives before the stoichiometry window is traversed,
/// and the shortfall is real physics rather than a bug. So this discharges gently
/// (C/10 at `dt` = 10 s) and measures between two *SOC* readings rather than to a
/// voltage cut-off — which takes the surface-vs-bulk gap out of the comparison
/// almost entirely and leaves an assertion about the conversion factor.
///
/// A test that discharged hard and asserted an exact ratio would be flaky in a way
/// that reads like a diffusion bug.
#[test]
fn capacity_multipliers_scale_the_amp_hours_of_the_shipped_spm_cell() {
    // Amp-hours drawn between soc = 0.9 and soc = 0.2 at a gentle current.
    let measure = |capacity_factor: f64| {
        let chem = parse_chemistry(LGM50).expect("LG M50 parses");
        let mut pack = Pack::new(&cfg(0.9), chem).expect("pack builds");
        pack.set_cell_factors(0, 0, capacity_factor, 1.0)
            .expect("cell 0S0P exists");
        let (i, dt) = (0.5, 10.0);
        let mut ah = 0.0;
        for _ in 0..100_000 {
            let tele = pack.step(dt, Demand::Current(i), &env());
            ah += tele.i_actual * dt / 3600.0;
            if tele.soc_true <= 0.2 {
                return ah;
            }
        }
        panic!("the pack never reached soc = 0.2");
    };

    let full = measure(1.0);
    let weak = measure(0.8);
    let ratio = weak / full;
    assert!(
        (ratio - 0.8).abs() < 0.01,
        "a cell configured at 80 % capacity must deliver 80 % of the amp-hours over \
         the same SOC span: {weak} Ah / {full} Ah = {ratio}. If this is ~1.0 the \
         capacity factors are not reaching the flux conversion, and manufacturing \
         scatter and Fault::WeakCell are no-ops on SPM packs."
    );

    // And the absolute number is the chemistry's own capacity over that span, which
    // is what pins the geometry-to-capacity reconciliation rather than merely its
    // proportionality. 0.7 of nominal, within the quasi-static protocol's residual.
    let chem = parse_chemistry(LGM50).expect("LG M50 parses");
    let expected = 0.7 * chem.cell.capacity_ah;
    assert!(
        (full - expected).abs() < 0.02 * chem.cell.capacity_ah,
        "0.9 -> 0.2 on a nominal cell should draw {expected} Ah, got {full} Ah"
    );
}

/// Aging still ages: the SOH multipliers a live `[aging]` block produces reach an
/// SPM pack, rather than the model quietly ignoring the health it is handed.
#[test]
fn a_configured_spm_pack_actually_fades() {
    let chem = parse_chemistry(LGM50).expect("LG M50 parses");
    let mut config = cfg(0.9);
    config.aging = Some(AgingConfig {
        sub_clock_period_s: 10.0,
    });
    let mut pack = Pack::new(&config, chem).expect("pack builds");

    // A month at 45 degC and high SOC: calendar fade's worst corner, and enough of
    // it to be visible without asserting a placeholder coefficient's magnitude.
    let hot = Env {
        t_ambient: 318.15,
        t_coolant: None,
    };
    let mut pack_hot = Pack::new(
        &{
            let mut c = config.clone();
            c.initial_temp_k = 318.15;
            c
        },
        parse_chemistry(LGM50).expect("LG M50 parses"),
    )
    .expect("pack builds");
    for _ in 0..(24 * 30) {
        pack.step(3600.0, Demand::Rest, &env());
        pack_hot.step(3600.0, Demand::Rest, &hot);
    }

    let cool = pack.step(0.0, Demand::Rest, &env());
    let warm = pack_hot.step(0.0, Demand::Rest, &hot);
    assert!(
        cool.soh_capacity < 1.0 && cool.soh_resistance > 1.0,
        "a month on the shelf must cost an SPM pack capacity and add resistance, \
         got soh_capacity {} and soh_resistance {}",
        cool.soh_capacity,
        cool.soh_resistance
    );
    assert!(
        warm.soh_capacity < cool.soh_capacity,
        "calendar fade is Arrhenius, so the hot pack must fade further: {} (45 degC) \
         vs {} (25 degC)",
        warm.soh_capacity,
        cool.soh_capacity
    );
}

// ---------------------------------------------------------------------------
// The two build errors the selector brought with it
// ---------------------------------------------------------------------------

/// Selecting `Spm` against a chemistry with no `[spm]` section fails the build, and
/// the message names both the chemistry and what it is missing.
///
/// The message is asserted rather than only the variant. Whoever hits this has
/// configured a pack against the wrong chemistry *or* asked for the wrong model, and
/// the two fixes are opposite; an error reading "invalid config" costs them an hour.
#[test]
fn selecting_spm_against_a_chemistry_without_one_is_a_build_error() {
    let err = Pack::new(&cfg(0.5), parse_chemistry(LFP).expect("LFP parses"))
        .expect_err("LFP has no [spm] section, so this must not build");
    let message = err.to_string();
    assert!(
        message.contains("lfp_26650_generic"),
        "the error should name the chemistry that came up short, got: {message}"
    );
    assert!(
        message.contains("[spm]"),
        "the error should name the missing section, got: {message}"
    );
}

/// The shell count is range-checked, at both ends and by the same error.
///
/// The ceiling is not a physical limit — it is the length of the fixed scratch row
/// the tridiagonal solve stands up — so it is checked here rather than trusted, and a
/// count past it must be a `BuildError` rather than a truncation or a panic.
#[test]
fn an_out_of_range_shell_count_is_a_build_error() {
    for shells in [0, 1, sim_core::spm::MAX_SHELLS + 1, usize::MAX] {
        let mut config = cfg(0.5);
        config.cell_model = CellModelConfig::Spm { shells };
        let err = Pack::new(&config, parse_chemistry(LGM50).expect("LG M50 parses"))
            .expect_err("{shells} shells is out of range and must not build");
        assert!(
            err.to_string().contains("shells"),
            "the error should name the offending field, got: {err}"
        );
    }
    for shells in [sim_core::spm::MIN_SHELLS, sim_core::spm::MAX_SHELLS] {
        let mut config = cfg(0.5);
        config.cell_model = CellModelConfig::Spm { shells };
        Pack::new(&config, parse_chemistry(LGM50).expect("LG M50 parses"))
            .unwrap_or_else(|e| panic!("{shells} shells is in range and must build: {e}"));
    }
}

// ---------------------------------------------------------------------------
// The selector reaches the scenario file format
// ---------------------------------------------------------------------------

/// A scenario file can ask for porous-electrode physics, and one that does not asks
/// for the equivalent circuit without saying so.
///
/// `PackConfig` doubles as the scenario format (`CLAUDE.md`), so a config field that
/// only Rust callers can reach would be half a feature. The default matters as much
/// as the selector: every scenario written before Phase 6 omits `cell_model`
/// entirely, and each one has to keep meaning exactly what it meant.
#[test]
fn a_scenario_file_can_select_the_single_particle_model() {
    let spm = parse_scenario(
        r#"
chemistry = "nmc_21700_lgm50"
[meta]
name = "spm 1S1P"
[pack]
series = 1
parallel = 1
initial_soc = 0.8
initial_temp_k = 298.15
seed = 3
[pack.cell_model.Spm]
shells = 12
"#,
    )
    .expect("a scenario selecting Spm parses");
    assert_eq!(
        spm.pack.cell_model,
        CellModelConfig::Spm { shells: 12 },
        "the shell count must survive the round trip through TOML"
    );
    spm.build_pack(parse_chemistry(LGM50).expect("LG M50 parses"))
        .expect("and the pack it describes builds");

    let silent = parse_scenario(
        r#"
chemistry = "lfp_26650_generic"
[meta]
name = "no cell_model at all"
[pack]
series = 1
parallel = 1
initial_soc = 0.8
initial_temp_k = 298.15
seed = 3
"#,
    )
    .expect("a scenario with no cell_model parses");
    assert_eq!(
        silent.pack.cell_model,
        CellModelConfig::Ecm,
        "silence means the equivalent circuit — every scenario written before \
         Phase 6 depends on it"
    );
}
