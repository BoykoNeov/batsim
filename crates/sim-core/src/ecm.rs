//! Equivalent-circuit cell model (Thevenin, 1–2 RC pairs) and its physics.
//!
//! The physics live in small pure free functions ([`ocv_lookup`], [`r0_lookup`],
//! [`rc_update`], [`coulomb_step`]) so tests and property checks can exercise
//! them directly. [`cell_step`] composes them into one cell advance.
//!
//! # Sign convention
//! Positive current = **discharge** (out of the terminals). Charging is negative.
//!
//! # Step ordering (important)
//! The step is explicit: the operating current is solved from the
//! **start-of-step** internal state (`OCV(soc) − Σ V_rc` behind `R0`), which keeps
//! the electrical solve closed-form. The RC overpotentials and SOC are then
//! advanced with that solved current. All [`crate::Telemetry`] values are reported
//! from the **end-of-step** state.

use serde::{Deserialize, Serialize};

use crate::chem::{ChemistryParams, OcvTable, R0Table};
use crate::flags::EventFlags;
use crate::Demand;

/// Per-cell equivalent-circuit state. Opaque to the pack layer; the enclosing
/// [`CellModel`] variant fixes how many entries `v_rc` carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EcmState {
    /// State of charge, in \[0, 1\].
    pub soc: f64,
    /// RC-pair overpotentials \[V\], discharge-positive; one entry per RC pair.
    pub v_rc: Vec<f64>,
    /// Cell temperature \[K\]. Held constant until the thermal network (Phase 2).
    pub temp_k: f64,
}

/// Cell-model slot. Enum dispatch (not trait objects) keeps state serde-friendly.
///
/// Both current variants share [`EcmState`]; the variant records the RC-pair
/// count. Porous-electrode models (`Spm`/`Dfn`) are added in a later phase without
/// touching the pack layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellModel {
    /// Single RC pair.
    Ecm1Rc(EcmState),
    /// Two RC pairs.
    Ecm2Rc(EcmState),
}

impl CellModel {
    /// Shared read access to the underlying ECM state.
    #[must_use]
    pub fn state(&self) -> &EcmState {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s,
        }
    }

    /// Shared mutable access to the underlying ECM state.
    pub fn state_mut(&mut self) -> &mut EcmState {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s,
        }
    }
}

/// Result of advancing one cell by one step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellOut {
    /// End-of-step terminal voltage \[V\].
    pub v_terminal: f64,
    /// Current through the cell \[A\], discharge-positive.
    pub i: f64,
    /// Events raised during this step.
    pub flags: EventFlags,
}

/// Linear-interpolate `ys` at `x` over ascending breakpoints `xs`, clamped at the
/// ends. `xs` must be non-empty and the same length as `ys`.
#[must_use]
fn interp1(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    debug_assert!(n > 0 && n == ys.len());
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    // xs is ascending; find the bracketing segment.
    let mut hi = 1;
    while hi < n && xs[hi] < x {
        hi += 1;
    }
    let lo = hi - 1;
    let span = xs[hi] - xs[lo];
    // span > 0 because xs is strictly ascending (validated) and x is interior.
    let frac = (x - xs[lo]) / span;
    ys[lo] + frac * (ys[hi] - ys[lo])
}

/// Open-circuit voltage \[V\] at the given SOC, by clamped linear interpolation.
#[must_use]
pub fn ocv_lookup(table: &OcvTable, soc: f64) -> f64 {
    interp1(&table.soc, &table.volts, soc)
}

/// Ohmic series resistance `R0` \[ohms\] at `(soc, temp_k)`, by clamped bilinear
/// interpolation over the grid.
#[must_use]
pub fn r0_lookup(table: &R0Table, soc: f64, temp_k: f64) -> f64 {
    // Interpolate along temperature within each soc row, then across soc rows.
    // Reuse interp1 by materialising the per-row temperature interpolation.
    let per_row: Vec<f64> = table
        .ohms
        .iter()
        .map(|row| interp1(&table.temp_k, row, temp_k))
        .collect();
    interp1(&table.soc, &per_row, soc)
}

/// Exact exponential update of one RC-pair overpotential for piecewise-constant
/// current over `dt` seconds. Unconditionally stable at any `dt`.
///
/// `V_rc' = V_rc·e^(−dt/τ) + R·I·(1 − e^(−dt/τ))`, with `τ = R·C`. `i` is
/// discharge-positive \[A\]. A non-positive `τ` or `dt` leaves the value unchanged
/// / snaps to steady state respectively.
#[must_use]
pub fn rc_update(v_rc: f64, i: f64, r_ohms: f64, c_farad: f64, dt: f64) -> f64 {
    let tau = r_ohms * c_farad;
    if tau > 0.0 && dt > 0.0 {
        let decay = (-dt / tau).exp();
        v_rc * decay + r_ohms * i * (1.0 - decay)
    } else {
        // Non-positive tau or dt (or NaN): no well-defined exponential update.
        v_rc
    }
}

/// Coulomb-counting SOC advance over `dt` seconds.
///
/// `soc' = soc − I·dt / (3600·capacity_ah·soh_capacity)`, clamped to \[0, 1\].
/// Returns the new SOC and a flag set (`SOC_CLAMPED_HIGH`/`_LOW`) when the raw
/// update ran past a bound.
#[must_use]
pub fn coulomb_step(
    soc: f64,
    i: f64,
    dt: f64,
    capacity_ah: f64,
    soh_capacity: f64,
) -> (f64, EventFlags) {
    let capacity_as = 3600.0 * capacity_ah * soh_capacity; // amp-seconds
    let raw = soc - i * dt / capacity_as;
    let mut flags = EventFlags::empty();
    if raw > 1.0 {
        flags |= EventFlags::SOC_CLAMPED_HIGH;
        return (1.0, flags);
    }
    if raw < 0.0 {
        flags |= EventFlags::SOC_CLAMPED_LOW;
        return (0.0, flags);
    }
    (raw, flags)
}

/// Solve the single-cell operating current \[A\] for a [`Demand`] given the
/// start-of-step Thevenin source `e = OCV − Σ V_rc` behind resistance `r0`.
///
/// Terminal voltage at current `i` is `V(i) = e − i·r0`.
#[must_use]
fn solve_current(demand: Demand, e: f64, r0: f64) -> f64 {
    match demand {
        Demand::Rest => 0.0,
        Demand::Current(i) => i,
        // V = e − i·r0  ⇒  i = (e − V) / r0.
        Demand::Voltage(v) => (e - v) / r0,
        // P = V·i = (e − i·r0)·i  ⇒  r0·i² − e·i + P = 0.
        // Physical (lower-current, higher-voltage) root; snap to the max-power
        // point if the target power is unreachable.
        Demand::Power(p) => {
            let disc = e * e - 4.0 * r0 * p;
            if disc <= 0.0 {
                e / (2.0 * r0)
            } else {
                (e - disc.sqrt()) / (2.0 * r0)
            }
        }
    }
}

/// Advance a single cell by one step. See the module-level note on step ordering:
/// the current is solved from `state`'s start-of-step values; `state` is then
/// advanced in place and the returned voltage reflects the end-of-step state.
#[must_use]
pub fn cell_step(state: &mut EcmState, chem: &ChemistryParams, demand: Demand, dt: f64) -> CellOut {
    // --- solve current from start-of-step state ---
    let r0_start = r0_lookup(&chem.r0, state.soc, state.temp_k);
    let ocv_start = ocv_lookup(&chem.ocv, state.soc);
    let e = ocv_start - state.v_rc.iter().sum::<f64>();
    let i = solve_current(demand, e, r0_start);

    // --- advance internal state with the solved current ---
    for (k, v_rc) in state.v_rc.iter_mut().enumerate() {
        let pair = chem.rc[k];
        *v_rc = rc_update(*v_rc, i, pair.r_ohms, pair.c_farad, dt);
    }
    let (soc_new, flags) = coulomb_step(state.soc, i, dt, chem.cell.capacity_ah, 1.0);
    state.soc = soc_new;

    // --- report from end-of-step state ---
    let r0_end = r0_lookup(&chem.r0, state.soc, state.temp_k);
    let ocv_end = ocv_lookup(&chem.ocv, state.soc);
    let v_terminal = ocv_end - i * r0_end - state.v_rc.iter().sum::<f64>();

    CellOut {
        v_terminal,
        i,
        flags,
    }
}
