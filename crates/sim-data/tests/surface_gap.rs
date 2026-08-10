//! The surface-vs-bulk stoichiometry gap, driven against the **shipped** LG M50 file.
//!
//! `CellView::surface_gap_neg` / `surface_gap_pos` are the gradient an equivalent circuit
//! cannot have — where `overpotential_v` reports what that gradient costs in volts, these
//! report the gradient itself.
//!
//! # Why here rather than in `sim-core`
//! `dfn_cell.rs`'s reason, unchanged: `sim-core` cannot read a file, a DFN needs both the
//! `[spm]` and `[dfn]` sections, and a hand-built fixture for that is a typo waiting to
//! happen. The claims below are also *quantitative* — the positive electrode carries
//! several times the negative's gradient — and a decimated fixture would not support them.
//!
//! # Each test names the mistake it exists to catch
//! Written against the spike's own measurement table
//! (`docs/plans/surface-vs-bulk.md`), and each one is the assertion that fails if the
//! corresponding thing is done wrong:
//!
//! * an equivalent circuit reporting `0.0` instead of `None`;
//! * either electrode's sign dropped, so a discharge and a charge read alike;
//! * the two electrodes swapped, which understates the headline by ~6×;
//! * either side of the difference clamped to \[0, 1\], which is invisible on a discharge
//!   and shows a standing gradient that is pure clamp on a charge;
//! * the DFN's surface reduced over x as an extreme rather than as a mean, which never
//!   relaxes.

use sim_core::{
    CellModelConfig, Demand, Env, EventFlags, Pack, PackConfig, Scatter, ThermalConfig,
};
use sim_data::parse_chemistry;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// Nominal capacity of the shipped cell \[A·h\]. Not `5.0` — see `dfn_cell.rs`.
const CAPACITY_AH: f64 = 5.153198;
/// 3 C, the rate the goldens and the guided path both run at.
const I_3C: f64 = 3.0 * CAPACITY_AH;
/// Discharge cut-off \[V\].
const V_CUT: f64 = 2.5;

/// The DFN grid `dfn_cell.rs` runs on: the Phase 7 spike's convergence and cost tables
/// were measured at 10/5/10 with 10 shells.
const DFN: CellModelConfig = CellModelConfig::Dfn {
    shells: 10,
    nodes_negative: 10,
    nodes_separator: 5,
    nodes_positive: 10,
};
const SPM: CellModelConfig = CellModelConfig::Spm { shells: 10 };

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn pack(model: CellModelConfig, initial_soc: f64) -> Pack {
    Pack::new(
        &PackConfig {
            series: 1,
            parallel: 1,
            initial_soc,
            initial_temp_k: 298.15,
            seed: 1,
            scatter: Scatter::default(),
            // Isothermal on purpose: diffusivity is temperature-dependent, so a warming
            // cell would move the gap for a reason that is not the gradient.
            thermal: ThermalConfig::Isothermal,
            bms: None,
            aging: None,
            cell_model: model,
        },
        parse_chemistry(LGM50).expect("LG M50 parses"),
    )
    .expect("pack builds")
}

/// `(negative, positive)` gap on the pack's only cell, which must have one.
fn gap(p: &Pack) -> (f64, f64) {
    let c = p.cell(0, 0).expect("a 1S1P pack has a cell (0,0)");
    (
        c.surface_gap_neg.expect("a porous-electrode cell has one"),
        c.surface_gap_pos.expect("a porous-electrode cell has one"),
    )
}

/// Step `n` times at `dt` under a constant current, stopping early at the cut-off.
///
/// Stopping there is not tidiness: past the end of a hard discharge a DFN's Newton runs
/// its cap out and a step costs 23× a converged one, so a test that ran on would spend
/// most of its time measuring the solver.
fn run(p: &mut Pack, i: f64, dt: f64, n: usize) -> EventFlags {
    let mut flags = EventFlags::empty();
    for _ in 0..n {
        let t = p.step(dt, Demand::Current(i), &env());
        flags |= t.flags;
        if i > 0.0 && (!t.v_terminal.is_finite() || t.v_terminal <= V_CUT) {
            break;
        }
    }
    flags
}

fn rest(p: &mut Pack, dt: f64, n: usize) {
    for _ in 0..n {
        let _ = p.step(dt, Demand::Rest, &env());
    }
}

// ---------------------------------------------------------------------------
// The model that has no surface
// ---------------------------------------------------------------------------

/// An equivalent circuit reports `None`, under load and at rest alike — **not** `0.0`.
///
/// The distinction is the whole reason the field is an `Option`. A circuit has no
/// electrodes and no particles; a `0.0` would be indistinguishable, to a client that
/// plots it, from a real measurement of a relaxed porous cell, which is exactly the trap
/// the `v_rc_sum` → `overpotential_v` rename was paid to remove.
///
/// Asserted under load as well as at rest because a suite that only ever looked at a
/// fresh pack would pass on an ECM arm hardcoded to `Some(0.0)` — the value a fresh
/// porous cell genuinely has.
#[test]
fn an_equivalent_circuit_reports_no_surface_at_all() {
    let mut p = pack(CellModelConfig::Ecm, 0.9);
    let fresh = p.cell(0, 0).expect("cell (0,0)");
    assert_eq!(fresh.surface_gap_neg, None);
    assert_eq!(fresh.surface_gap_pos, None);

    let _ = run(&mut p, I_3C, 1.0, 60);
    let loaded = p.cell(0, 0).expect("cell (0,0)");
    assert_eq!(
        loaded.surface_gap_neg, None,
        "an ECM under load still has no surface"
    );
    assert_eq!(loaded.surface_gap_pos, None);
    assert!(
        loaded.overpotential_v.abs() > 1.0e-4,
        "the fixture must actually be polarized, or this proves nothing: {} V",
        loaded.overpotential_v
    );
}

/// A porous cell that has never been stepped has no gradient, and a zero-length probe
/// step does not give it one.
///
/// The page depends on this: `applyStep` finishes by stepping `dt = 0` at the demand it
/// just dialled in, so if that probe wrote `i_last` the readout would show a full 3 C
/// gradient on a pack the reader has not yet run. Neither `spm::advance` nor
/// `dfn::advance` mutates on a zero-length step, and this is the assertion that keeps it
/// that way.
///
/// # Why this is `< 1e-15` and not `== 0.0`
/// A uniform particle reads `-1.11e-16` on the negative electrode, not a hard zero: the
/// bulk side goes through `mean_concentration`, whose volume weighting sums and divides,
/// while the surface side is `c_surface` at zero flux, which returns the outermost shell
/// unchanged. Two exact-in-principle routes to the same number, one of which rounds. It
/// is a *representation* residual and not a gradient — three orders of magnitude below
/// anything the page prints — and pinning it at zero would mean contriving one of the two
/// paths to match the other.
#[test]
fn a_fresh_cell_has_no_gradient_and_a_probe_step_does_not_create_one() {
    const NOISE: f64 = 1.0e-15;
    for model in [SPM, DFN] {
        let mut p = pack(model, 1.0);
        let (n, q) = gap(&p);
        assert!(
            n.abs() < NOISE && q.abs() < NOISE,
            "a uniform particle has no gradient: neg {n}, pos {q}"
        );
        let _ = p.step(0.0, Demand::Current(I_3C), &env());
        let (n, q) = gap(&p);
        assert!(
            n.abs() < NOISE && q.abs() < NOISE,
            "a zero-length probe at 3 C must not manufacture a gradient: neg {n}, pos {q}"
        );
    }
}

// ---------------------------------------------------------------------------
// Sign, and which electrode is which
// ---------------------------------------------------------------------------

/// Discharge-positive on **both** electrodes, and negative on a charge.
///
/// The two electrodes' stoichiometries move in opposite directions with state of charge,
/// so this is a claim about the two window mappings agreeing, not about one sign. Drop
/// either mapping's direction and one electrode reads backwards here.
#[test]
fn a_discharge_drives_both_electrodes_positive_and_a_charge_both_negative() {
    for model in [SPM, DFN] {
        let mut d = pack(model, 0.9);
        let _ = run(&mut d, I_3C, 2.0, 60);
        let (n, q) = gap(&d);
        assert!(n > 0.0 && q > 0.0, "discharge: neg {n}, pos {q}");

        let mut c = pack(model, 0.5);
        let _ = run(&mut c, -I_3C, 2.0, 60);
        let (n, q) = gap(&c);
        assert!(n < 0.0 && q < 0.0, "charge: neg {n}, pos {q}");
    }
}

/// The **positive** electrode carries the larger gradient, by several times.
///
/// Measured at the 3 C cut-off: 0.331 against 0.058 on the DFN, 0.372 against 0.058 on
/// the SPM — roughly 6×. This is the assertion that fails if the two electrodes are
/// swapped, and the reason the field is not negative-only: `soc` is defined from the
/// negative electrode, so the obvious single field would have named the wrong electrode
/// as the reason a hard discharge stops.
///
/// The bound is 3× rather than 5.7× so it is a claim about which electrode dominates
/// rather than a pin on the shipped parameters.
#[test]
fn the_positive_electrode_carries_the_larger_gradient() {
    for model in [SPM, DFN] {
        let mut p = pack(model, 1.0);
        let _ = run(&mut p, I_3C, 2.0, 2000);
        let (n, q) = gap(&p);
        assert!(
            q > 3.0 * n && n > 0.0,
            "the positive electrode should dominate: neg {n}, pos {q}"
        );
    }
}

/// The radial gap is set by the **current**, not by the model — so the two porous models
/// agree about it to within a fraction of a percent at the same rate.
///
/// A characterization rather than a requirement, and it is here because it falsifies a
/// purpose the field could otherwise be believed to serve: this quantity does *not*
/// distinguish a DFN from a single particle. What does is the spread across x, which
/// this slice deliberately does not report. Measured at 5 parts in 10⁵; asserted at 1 %
/// so it is a claim about the mechanism and not about the discretisation.
#[test]
fn the_two_porous_models_agree_about_the_radial_gradient() {
    let (mut s, mut d) = (pack(SPM, 1.0), pack(DFN, 1.0));
    let _ = run(&mut s, I_3C, 2.0, 60);
    let _ = run(&mut d, I_3C, 2.0, 60);
    let (sn, _) = gap(&s);
    let (dn, _) = gap(&d);
    assert!(
        (sn - dn).abs() < 0.01 * sn.abs(),
        "the same current should make the same radial gradient: spm {sn}, dfn {dn}"
    );
}

// ---------------------------------------------------------------------------
// The clamp, and the reduction over x
// ---------------------------------------------------------------------------

/// A cell charged **past full** and then rested has no gradient left — and that is only
/// true because neither side of the difference is clamped.
///
/// This is the fixture that discriminates, and a plain discharge is not. On a discharge
/// nothing leaves \[0, 1\] and a clamp on either side is inert; charged past `stoich_max`
/// the surface reaches 1.16 on the shipped cell, so a clamp pins it at 1.0 against a bulk
/// of 1.19 and reports a standing 0.19 of gradient on a pack that is uniformly at rest.
///
/// `SOC_CLAMPED_HIGH` is asserted first: without it the run might not have gone past the
/// window at all, and the test would be vacuous — which is a mistake this repo has made
/// twice and recorded both times.
#[test]
fn an_overcharged_cell_relaxes_to_no_gradient_at_all() {
    for model in [SPM, DFN] {
        let mut p = pack(model, 0.5);
        let flags = run(&mut p, -I_3C, 10.0, 300);
        assert!(
            flags.contains(EventFlags::SOC_CLAMPED_HIGH),
            "the fixture must actually leave the window, or the clamp is untested"
        );
        let (n, q) = gap(&p);
        assert!(n < -0.01 && q < -0.01, "charging: neg {n}, pos {q}");

        // Long enough for the slower of the two electrodes. The positive's diffusion
        // timescale is the longer one on this cell, which is why this is 2 h and not the
        // 600 s the negative needs.
        rest(&mut p, 60.0, 120);
        let (n, q) = gap(&p);
        assert!(
            n.abs() < 1.0e-6 && q.abs() < 1.0e-6,
            "a rested cell has no gradient anywhere, however far past full it is: \
             neg {n}, pos {q}"
        );
    }
}

/// The DFN averages its surface across x, exactly as its bulk is averaged — so its gap
/// relaxes to nothing, and quickly.
///
/// An x-*extreme* differenced against the x-mean bulk differences two different spatial
/// reductions. Nothing moves solid lithium between x positions except the small reaction
/// currents a relaxing electrolyte drives, so that version carries a standing offset that
/// rest does not remove: measured at 0.0289 two hours in, where the mean is gone by
/// 300 s. The bound below is three orders of magnitude under that offset and two above
/// the mean's own residual.
#[test]
fn the_dfn_reduces_its_surface_over_x_as_a_mean_not_an_extreme() {
    let mut p = pack(DFN, 1.0);
    // Stopped short of the cut-off deliberately: the gradient is already large here and
    // the solve is still converging in a handful of iterations.
    let _ = run(&mut p, I_3C, 2.0, 200);
    let (n, _) = gap(&p);
    assert!(n > 0.02, "the fixture needs a real gradient to relax: {n}");

    rest(&mut p, 30.0, 40);
    let (n, _) = gap(&p);
    assert!(
        n.abs() < 1.0e-5,
        "twenty minutes of rest leaves no radial gradient in the x-mean; an x-extreme \
         would still read about 0.03 here: {n}"
    );
}
