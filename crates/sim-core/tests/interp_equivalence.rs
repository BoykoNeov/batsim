//! Pins the table lookups to their naive reference implementation, **bit for bit**.
//!
//! `ocv_lookup` and `r0_lookup` run twice per cell per step, so they are the
//! natural place for a performance rewrite (`docs/plans/pack-step-perf.md`). The
//! two that landed — binary search in place of a linear breakpoint scan, and
//! interpolating only the two bracketing `R0` rows instead of materialising every
//! row into a scratch `Vec` — are both *supposed* to be pure speed: same
//! arithmetic, same operand order, same answer down to the last mantissa bit.
//!
//! The golden and property tests cannot enforce that, because they assert within
//! a tolerance; a rewrite that shifted a ULP would sail through them and quietly
//! break the "snapshot replay is bit-identical" guarantee on some other trajectory.
//! So this file carries the pre-optimisation code as `naive_*` and compares
//! `f64::to_bits`, which is exact and also distinguishes `+0.0` from `-0.0`.
//!
//! If a future optimisation genuinely needs to change the arithmetic, this test is
//! the thing that should fail — and the plan doc's determinism note is why that
//! failure matters rather than being a nuisance to re-bless.

use sim_core::chem::{OcvTable, R0Table};
use sim_core::ecm::{ocv_lookup, r0_lookup};

/// The original `interp1`: clamp at both ends, then scan breakpoints linearly
/// from the low end. Copied verbatim from before the rewrite.
fn naive_interp1(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut hi = 1;
    while hi < n && xs[hi] < x {
        hi += 1;
    }
    let lo = hi - 1;
    let span = xs[hi] - xs[lo];
    let frac = (x - xs[lo]) / span;
    ys[lo] + frac * (ys[hi] - ys[lo])
}

/// The original `r0_lookup`: interpolate *every* soc row over temperature into a
/// scratch `Vec`, then interpolate across rows. Copied verbatim.
fn naive_r0_lookup(table: &R0Table, soc: f64, temp_k: f64) -> f64 {
    let per_row: Vec<f64> = table
        .ohms
        .iter()
        .map(|row| naive_interp1(&table.temp_k, row, temp_k))
        .collect();
    naive_interp1(&table.soc, &per_row, soc)
}

/// 34-point LFP OCV curve — the real table shape, with the tight clusters near
/// both ends that make "where does the scan stop" a non-trivial question.
///
/// provenance: copied from `chemistries/lfp_26650_generic.toml`; only the shape
/// matters here, nothing in this file is a physical claim.
fn ocv_table() -> OcvTable {
    OcvTable {
        soc: vec![
            0.0000, 0.0025, 0.0050, 0.0075, 0.0100, 0.0125, 0.0150, 0.0175, 0.0200, 0.0300, 0.0400,
            0.0500, 0.1000, 0.1500, 0.2500, 0.3500, 0.4500, 0.5500, 0.6500, 0.7500, 0.8500, 0.9000,
            0.9500, 0.9600, 0.9700, 0.9800, 0.9825, 0.9850, 0.9875, 0.9900, 0.9925, 0.9950, 0.9975,
            1.0000,
        ],
        volts: vec![
            2.0000, 2.0743, 2.1430, 2.2066, 2.2655, 2.3199, 2.3703, 2.4169, 2.4600, 2.6028, 2.7077,
            2.7853, 2.9781, 3.1080, 3.1857, 3.2324, 3.2621, 3.2678, 3.2700, 3.2926, 3.3132, 3.3142,
            3.3164, 3.3193, 3.3274, 3.3502, 3.3607, 3.3743, 3.3920, 3.4150, 3.4449, 3.4838, 3.5343,
            3.6000,
        ],
    }
}

/// The shipped 3x3 `R0` grid: only two soc segments, so it exercises the
/// bracketing at the coarse end.
fn r0_grid_3x3() -> R0Table {
    R0Table {
        soc: vec![0.0, 0.5, 1.0],
        temp_k: vec![263.15, 298.15, 318.15],
        ohms: vec![
            vec![0.055, 0.022, 0.018],
            vec![0.048, 0.020, 0.016],
            vec![0.050, 0.021, 0.017],
        ],
    }
}

/// A denser 5x4 grid with non-monotone interior rows. A fitted parameter set will
/// be denser than the placeholder above, and more rows means the naive version
/// interpolates rows the bracket never blends — precisely the work the rewrite
/// drops, so this is where a wrong-row bug would show up.
fn r0_grid_5x4() -> R0Table {
    R0Table {
        soc: vec![0.0, 0.15, 0.5, 0.85, 1.0],
        temp_k: vec![253.15, 273.15, 298.15, 323.15],
        ohms: vec![
            vec![0.090, 0.061, 0.024, 0.019],
            vec![0.081, 0.055, 0.021, 0.017],
            vec![0.074, 0.049, 0.019, 0.015],
            vec![0.078, 0.052, 0.020, 0.016],
            vec![0.086, 0.058, 0.023, 0.018],
        ],
    }
}

/// Probe points around every breakpoint of `xs`: the breakpoint itself, one ULP
/// either side of it, the segment midpoint, and well outside both ends. The ULP
/// neighbours are the point of the exercise — that is where a `<` / `<=` slip
/// between a linear scan and a binary search would land on a different segment.
fn probes(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![
        f64::NEG_INFINITY,
        -1.0,
        xs[0] - 0.1,
        xs[xs.len() - 1] + 0.1,
        1e9,
        f64::INFINITY,
    ];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
    }
    for &x in xs {
        out.push(x);
        out.push(f64::from_bits(x.to_bits() + 1));
        if x != 0.0 {
            out.push(f64::from_bits(x.to_bits() - 1));
        }
    }
    out
}

/// Every bit of both results must match, including the sign of zero — hence
/// `to_bits` rather than `==`.
#[track_caller]
fn assert_bit_identical(got: f64, want: f64, what: &str) {
    assert_eq!(
        got.to_bits(),
        want.to_bits(),
        "{what}: optimised {got:?} ({:#018x}) != naive {want:?} ({:#018x})",
        got.to_bits(),
        want.to_bits(),
    );
}

#[test]
fn ocv_lookup_matches_naive_bit_for_bit() {
    let table = ocv_table();
    for soc in probes(&table.soc) {
        assert_bit_identical(
            ocv_lookup(&table, soc),
            naive_interp1(&table.soc, &table.volts, soc),
            &format!("ocv_lookup(soc = {soc:?})"),
        );
    }
}

#[test]
fn r0_lookup_matches_naive_bit_for_bit() {
    for table in [r0_grid_3x3(), r0_grid_5x4()] {
        // Both axes are bracketed independently, so sweep the cross product.
        for soc in probes(&table.soc) {
            for temp_k in probes(&table.temp_k) {
                assert_bit_identical(
                    r0_lookup(&table, soc, temp_k),
                    naive_r0_lookup(&table, soc, temp_k),
                    &format!("r0_lookup(soc = {soc:?}, temp_k = {temp_k:?})"),
                );
            }
        }
    }
}

/// A fine sweep of interior points, to catch a bracket that is only wrong away
/// from the breakpoints themselves.
#[test]
fn interior_sweep_matches_naive_bit_for_bit() {
    let ocv = ocv_table();
    let r0 = r0_grid_5x4();
    for k in 0..=2000 {
        let soc = f64::from(k) / 2000.0;
        assert_bit_identical(
            ocv_lookup(&ocv, soc),
            naive_interp1(&ocv.soc, &ocv.volts, soc),
            &format!("ocv_lookup(soc = {soc:?})"),
        );
        // Walk temperature across the grid's full span alongside soc.
        let temp_k = 248.15 + f64::from(k) * (80.0 / 2000.0);
        assert_bit_identical(
            r0_lookup(&r0, soc, temp_k),
            naive_r0_lookup(&r0, soc, temp_k),
            &format!("r0_lookup(soc = {soc:?}, temp_k = {temp_k:?})"),
        );
    }
}

/// `Pack::step` must never panic, so a NaN reaching a lookup has to flow through
/// as NaN rather than index out of bounds. The naive scan happened to do this;
/// `partition_point` answers 0 for a NaN needle, so the rewrite has to pin the
/// segment explicitly. This is the one input where the rewrite is *stronger* than
/// the original rather than equal to it — the original panicked on a
/// single-breakpoint table, which validation does not currently forbid.
#[test]
fn nan_inputs_return_nan_without_panicking() {
    let ocv = ocv_table();
    assert!(ocv_lookup(&ocv, f64::NAN).is_nan());

    let r0 = r0_grid_5x4();
    assert!(r0_lookup(&r0, f64::NAN, 298.15).is_nan());
    assert!(r0_lookup(&r0, 0.5, f64::NAN).is_nan());
    assert!(r0_lookup(&r0, f64::NAN, f64::NAN).is_nan());

    // Degenerate single-breakpoint tables: clamp to the only value, never index
    // past the end.
    let flat_ocv = OcvTable {
        soc: vec![0.5],
        volts: vec![3.3],
    };
    assert_eq!(ocv_lookup(&flat_ocv, 0.2), 3.3);
    assert_eq!(ocv_lookup(&flat_ocv, 0.9), 3.3);
    // A one-point table has nothing to interpolate, so even NaN clamps to it.
    assert_eq!(ocv_lookup(&flat_ocv, f64::NAN), 3.3);
}
