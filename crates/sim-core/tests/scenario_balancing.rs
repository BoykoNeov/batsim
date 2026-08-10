//! Scenario: **why passive balancing exists, and what it costs.**
//!
//! A series string is only as usable as its weakest element: charging stops when the
//! *first* group hits `v_max`, and discharging stops when the first one hits `v_min`.
//! A group that runs high therefore strands capacity in all the others. Passive
//! balancing fixes that by burning the high group's excess in a resistor until the
//! rest catch up — it does not move charge anywhere useful, it throws it away.
//!
//! Both halves matter and the tests assert both: balancing converges the string, and
//! it does so by destroying energy.

use sim_core::bms::{BalancingConfig, BmsConfig};
use sim_core::chem::{
    CellLimits, ChemMeta, ChemistryParams, OcvTable, R0Table, RcPair, ThermalParams,
};
use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};

const CAP_AH: f64 = 2.5;
/// Bleed threshold \[V\]. On the OCV curve below (3.20 V at SOC 0.5, rising 0.6 V per
/// unit SOC) this sits at SOC 0.883, chosen so that after the charge phase the weak
/// group is above it and the healthy groups are not — even under load, where the
/// ohmic and polarisation drops add ~30 mV at 1 A.
const V_BLEED: f64 = 3.43;
const R_BLEED: f64 = 33.0;
/// Bleed-switch release band \[V\], at the shipped default. The bleed's own load line
/// on this cell is `I_bleed · (R0 + Σ R_rc)` = `(3.43/33) · 0.03` = **3.1 mV**, so the
/// default clears it by 3.2×. See `BalancingConfig::v_release_band_v`.
const BAND: f64 = 0.010;
/// Charge duration \[s\] before resting. Kept short enough that the healthy groups
/// stay under the bleed threshold, so their behaviour isolates "never bled".
const CHARGE_S: usize = 1000;
/// Rest duration \[s\]. The weak group needs ~1700 s to bleed down to the threshold.
const REST_S: usize = 6000;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// Sloped OCV so a SOC difference between groups shows up as a voltage difference the
/// balancer can actually see.
fn chem() -> ChemistryParams {
    ChemistryParams {
        aging: None,
        safety: None,
        spm: None,
        dfn: None,
        meta: ChemMeta {
            id: "bal".into(),
            name: "Balancing test cell".into(),
            provenance: "balancing scenario — not physical".into(),
        },
        cell: CellLimits {
            capacity_ah: CAP_AH,
            v_max: 3.65,
            v_min: 2.50,
            max_charge_c: 2.0,
            max_discharge_c: 2.0,
            t_charge_min_k: 250.0,
            t_max_k: 350.0,
        },
        ocv: OcvTable {
            soc: vec![0.0, 0.5, 1.0],
            volts: vec![2.90, 3.20, 3.50],
            docv_dt_v_per_k: None,
        },
        r0: R0Table {
            soc: vec![0.0, 1.0],
            temp_k: vec![298.15],
            ohms: vec![vec![0.02], vec![0.02]],
        },
        rc: vec![RcPair {
            r_ohms: 0.01,
            c_farad: 2000.0,
        }],
        thermal: ThermalParams {
            heat_capacity_j_per_k: 95.0,
            h_area_w_per_k: 0.35,
        },
    }
}

fn bms(balancing: Option<BalancingConfig>) -> BmsConfig {
    BmsConfig {
        balancing,
        protection: None, // isolate balancing; protection has its own scenarios
        current_offset_a: 0.0,
        current_noise_sigma_a: 0.0,
        temp_probes: Vec::new(),
        initial_soc_error: 0.0,
        rest_current_threshold_a: 0.01,
        rest_time_for_ocv_s: 1.0e9, // effectively never correct: keep this test about balancing
        ocv_correction_gain: 0.0,
        min_ocv_slope_v_per_soc: 0.0,
    }
}

fn config(balancing: Option<BalancingConfig>) -> PackConfig {
    PackConfig {
        aging: None,
        series: 3,
        parallel: 1,
        initial_soc: 0.70,
        initial_temp_k: 298.15,
        seed: 11,
        scatter: Scatter::default(),
        thermal: ThermalConfig::Isothermal,
        bms: Some(bms(balancing)),
        cell_model: CellModelConfig::Ecm,
    }
}

fn balancing() -> BalancingConfig {
    BalancingConfig {
        bleed_r_ohms: R_BLEED,
        v_threshold_v: V_BLEED,
        v_release_band_v: BAND,
    }
}

/// Build a 3S1P string whose middle group runs high: half the capacity, so the
/// identical series current lifts its SOC twice as fast. This is the imbalance a real
/// string develops with age, and it is what strands capacity in the healthy groups.
fn imbalanced_pack(balancing: Option<BalancingConfig>) -> Pack {
    let mut pack = Pack::new(&config(balancing), chem()).unwrap();
    pack.set_cell_factors(1, 0, 0.50, 1.0).unwrap();
    pack
}

/// Charge the string briefly to open up a spread, then rest — which is when a passive
/// balancer does its work. Returns the total energy the balancer dissipated \[J\].
fn charge_then_rest(pack: &mut Pack) -> f64 {
    let mut wasted_j = 0.0;
    for _ in 0..CHARGE_S {
        wasted_j += pack.step(1.0, Demand::Current(-1.0), &env()).q_balancing_w;
    }
    for _ in 0..REST_S {
        wasted_j += pack.step(1.0, Demand::Rest, &env()).q_balancing_w;
    }
    wasted_j
}

/// The spread between the highest and lowest group SOC.
fn soc_spread(pack: &Pack) -> f64 {
    let socs: Vec<f64> = (0..3).map(|s| pack.cell(s, 0).unwrap().soc).collect();
    socs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - socs.iter().copied().fold(f64::INFINITY, f64::min)
}

/// Charging an imbalanced string drives the high group over the bleed threshold; the
/// balancer switches in, burns its excess, and the string converges. Without it the
/// spread only grows.
#[test]
fn balancing_converges_a_string_that_otherwise_diverges() {
    let mut balanced = imbalanced_pack(Some(balancing()));
    let mut unbalanced = imbalanced_pack(None);

    // Charge alone opens the spread; note it in both packs before the rest phase.
    let mut saw_balancing = false;
    for _ in 0..CHARGE_S {
        let tele = balanced.step(1.0, Demand::Current(-1.0), &env());
        saw_balancing |= tele.flags.contains(EventFlags::BALANCING);
        unbalanced.step(1.0, Demand::Current(-1.0), &env());
    }
    let spread_after_charge = soc_spread(&unbalanced);
    assert!(
        spread_after_charge > 0.05,
        "charging a mismatched string should open a real spread, got {spread_after_charge}"
    );

    for _ in 0..REST_S {
        let tele = balanced.step(1.0, Demand::Rest, &env());
        saw_balancing |= tele.flags.contains(EventFlags::BALANCING);
        unbalanced.step(1.0, Demand::Rest, &env());
    }

    assert!(saw_balancing, "the BALANCING flag should have been raised");
    let with = soc_spread(&balanced);
    let without = soc_spread(&unbalanced);
    // Resting alone changes nothing — an unbalanced string stays exactly as skewed as
    // the charge left it.
    assert!(
        (without - spread_after_charge).abs() < 1e-9,
        "rest alone should not converge anything: {without} vs {spread_after_charge}"
    );
    assert!(
        with < without - 0.01,
        "balancing should narrow the spread meaningfully: {with} vs {without}"
    );
}

/// Balancing is not free: the charge the high group loses is dissipated in a resistor,
/// so the balanced pack holds *less* total charge. Passive balancing buys usable
/// string capacity by destroying stored energy, and the telemetry says so.
#[test]
fn balancing_costs_energy_and_reports_it() {
    let mut balanced = imbalanced_pack(Some(balancing()));
    let mut unbalanced = imbalanced_pack(None);

    let wasted_j = charge_then_rest(&mut balanced);
    let wasted_none = charge_then_rest(&mut unbalanced);

    assert_eq!(wasted_none, 0.0, "a pack with no balancer wastes nothing");
    assert!(
        wasted_j > 100.0,
        "balancing should report real dissipated energy, got {wasted_j} J"
    );

    // The high group specifically is the one that lost charge.
    let high_balanced = balanced.cell(1, 0).unwrap().soc;
    let high_unbalanced = unbalanced.cell(1, 0).unwrap().soc;
    assert!(
        high_balanced < high_unbalanced,
        "the bleeding group should end lower: {high_balanced} vs {high_unbalanced}"
    );

    // Groups below the threshold were never bled, so they are untouched.
    for s in [0usize, 2] {
        let a = balanced.cell(s, 0).unwrap().soc;
        let b = unbalanced.cell(s, 0).unwrap().soc;
        assert!(
            (a - b).abs() < 1e-6,
            "group {s} was below the threshold and should be unaffected: {a} vs {b}"
        );
    }
}

/// Below the threshold nothing bleeds — no flag, no dissipation, and the trajectory is
/// bit-identical to a pack with balancing disabled entirely.
#[test]
fn no_bleed_below_the_threshold() {
    // Start well below the bleed voltage and discharge, so no group ever approaches it.
    let mut cfg = config(Some(balancing()));
    cfg.initial_soc = 0.4;
    let mut with_balancer = Pack::new(&cfg, chem()).unwrap();

    let mut cfg_off = cfg.clone();
    cfg_off.bms = Some(bms(None));
    let mut without = Pack::new(&cfg_off, chem()).unwrap();

    for _ in 0..500 {
        let a = with_balancer.step(1.0, Demand::Current(1.0), &env());
        let b = without.step(1.0, Demand::Current(1.0), &env());
        assert!(!a.flags.contains(EventFlags::BALANCING), "{:?}", a.flags);
        assert_eq!(a.q_balancing_w, 0.0);
        // Bit-identical: an open bleed switch contributes exactly zero conductance.
        assert_eq!(a, b, "an idle balancer must not perturb the trajectory");
    }
}

/// The bleed current is exactly `V/R` at the group node, and it is *additional* to the
/// pack current rather than stolen from it — so the bleeding group carries more
/// current than its series neighbours, which is the whole mechanism.
#[test]
fn bleed_current_is_v_over_r_and_adds_to_the_group() {
    // Every group above the threshold, so the comparison is clean.
    let mut cfg = config(Some(balancing()));
    cfg.series = 1;
    cfg.initial_soc = 0.95; // OCV 3.47 > 3.35 threshold
    let mut pack = Pack::new(&cfg, chem()).unwrap();

    // First step establishes the frame the balancer decides on; the second bleeds.
    pack.step(1.0, Demand::Rest, &env());
    let soc_before = pack.cell(0, 0).unwrap().soc;
    let dt = 1.0;
    let tele = pack.step(dt, Demand::Rest, &env());

    assert!(
        tele.flags.contains(EventFlags::BALANCING),
        "{:?}",
        tele.flags
    );
    assert_eq!(
        tele.i_actual, 0.0,
        "the pack terminal current is still zero"
    );

    // Charge left the cell even though no external current flowed: that is the bleed.
    let soc_after = pack.cell(0, 0).unwrap().soc;
    let i_cell = (soc_before - soc_after) * 3600.0 * CAP_AH / dt;
    let expected = tele.v_terminal / R_BLEED;
    assert!(
        (i_cell - expected).abs() < 1e-3,
        "bleed current {i_cell} A should match V/R = {expected} A"
    );
    // And the reported dissipation matches V²/R. The tolerance is not rounding slack:
    // `q_balancing_w` is evaluated at the *start-of-step* node voltage (the one that
    // actually drove the bleed, which is what makes the energy balance close exactly),
    // while `v_terminal` here is the end-of-step value. They differ by one step of
    // voltage drift — microvolts at this bleed current, but not zero.
    let expected_w = tele.v_terminal * tele.v_terminal / R_BLEED;
    assert!(
        (tele.q_balancing_w - expected_w).abs() < 1e-4,
        "{} W vs {expected_w} W",
        tele.q_balancing_w
    );
    // The reported bleed current is the same quantity divided by that voltage.
    assert!(
        (tele.i_balancing_a - expected).abs() < 1e-4,
        "{} A vs {expected} A",
        tele.i_balancing_a
    );
}
