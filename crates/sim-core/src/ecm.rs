//! Equivalent-circuit cell model (Thevenin, 1–2 RC pairs) and its physics.
//!
//! The physics live in small pure free functions ([`ocv_lookup`], [`r0_lookup`],
//! [`docv_dt_lookup`], [`rc_update`], [`coulomb_step`], [`cell_heat_w`]) so tests
//! and property checks can exercise them directly; [`advance_cell`] composes the
//! state-advancing ones into a single cell step.
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
//!
//! # Cell vs. pack responsibilities
//! From Phase 1 on, the *current itself* is decided by the pack-level electrical
//! solve (parallel cells share a node; series groups share a current), not by a
//! per-cell demand. This module therefore exposes the two halves separately:
//! [`cell_source`] returns a cell's start-of-step Thévenin `(E, R)` for the pack to
//! aggregate, and [`advance_cell`] advances one cell's internal state given the
//! current the pack solve assigned it. [`solve_current`] is the closed-form
//! single-Thévenin demand solve, reused by the pack on its aggregate source.

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
    /// Cell temperature \[K\]. Advanced by [`crate::thermal`] unless the pack is
    /// configured [`crate::ThermalConfig::Isothermal`], in which case it holds its
    /// initial value.
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

/// Locate `x` on ascending breakpoints `xs`, clamped at the ends.
///
/// Returns `(lo, hi, frac)`, the segment and blend weight to apply to *any*
/// `ys` sharing these breakpoints (see [`lerp_at`]). Splitting the search from
/// the blend lets [`r0_lookup`] reuse one SOC bracket across two rows of the
/// `R0` grid instead of interpolating every row. `xs` must be non-empty.
///
/// At a clamped end it returns `lo == hi`, which [`lerp_at`] reads as "take the
/// endpoint verbatim" — the blend is skipped rather than evaluated at `frac = 0`,
/// so the clamped result is the table value bit-for-bit.
#[must_use]
fn bracket(xs: &[f64], x: f64) -> (usize, usize, f64) {
    let n = xs.len();
    debug_assert!(n > 0);
    if n == 1 || x <= xs[0] {
        return (0, 0, 0.0);
    }
    if x >= xs[n - 1] {
        return (n - 1, n - 1, 0.0);
    }
    // xs is strictly ascending (validated at load), so the first breakpoint not
    // below `x` brackets it from above. `x` is interior here, so the true `hi`
    // is already in `1..=n-1`; the clamp only bites for a NaN `x`, where every
    // comparison is false and `partition_point` answers 0. Pinning that case to
    // hi = 1 keeps NaN flowing through as a NaN result instead of panicking on
    // an index underflow — `step` must never panic.
    let hi = xs.partition_point(|&v| v < x).clamp(1, n - 1);
    let lo = hi - 1;
    let span = xs[hi] - xs[lo];
    // span > 0 because xs is strictly ascending (validated) and x is interior.
    let frac = (x - xs[lo]) / span;
    (lo, hi, frac)
}

/// Apply a [`bracket`] result to one value column.
#[must_use]
fn lerp_at(ys: &[f64], (lo, hi, frac): (usize, usize, f64)) -> f64 {
    if lo == hi {
        ys[lo]
    } else {
        ys[lo] + frac * (ys[hi] - ys[lo])
    }
}

/// Linear-interpolate `ys` at `x` over ascending breakpoints `xs`, clamped at the
/// ends. `xs` must be non-empty and the same length as `ys`.
#[must_use]
fn interp1(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    debug_assert!(!xs.is_empty() && xs.len() == ys.len());
    lerp_at(ys, bracket(xs, x))
}

/// Open-circuit voltage \[V\] at the given SOC, by clamped linear interpolation.
#[must_use]
pub fn ocv_lookup(table: &OcvTable, soc: f64) -> f64 {
    interp1(&table.soc, &table.volts, soc)
}

/// Entropy coefficient `∂U/∂T` \[V/K\] at the given SOC, by clamped linear
/// interpolation — or exactly `0.0` if the chemistry supplies no
/// [`OcvTable::docv_dt_v_per_k`] column, which disables entropic heating.
#[must_use]
pub fn docv_dt_lookup(table: &OcvTable, soc: f64) -> f64 {
    match &table.docv_dt_v_per_k {
        // Same breakpoints as `volts`, so the same bracket applies.
        Some(ys) => interp1(&table.soc, ys, soc),
        None => 0.0,
    }
}

/// Heat generated inside one cell \[W\] over a step, given the current the pack
/// solve assigned it and its start-of-step state.
///
/// Two terms, both from Bernardi's energy balance, with `i` discharge-positive:
///
/// * **Irreversible** `I·(OCV − V_terminal) = I²·R0 + I·Σ V_rc`. This is the total
///   overpotential heat. Note the deviation from the `CLAUDE.md` sketch, which
///   writes `I²·(R0 + Σ R_rc)`: that form is the *steady-state* special case, true
///   only once every `V_rc` has settled to `R_rc·I`. During a transient — the
///   entire reason RC pairs exist — the two differ, and using the state we
///   actually carry keeps the pack energy balance exact (see the energy-balance
///   property test) as well as being cheaper. It can go slightly **negative** when
///   the current reverses while an overpotential is still relaxing: the RC
///   element is returning stored energy, which is physical for a lumped model.
/// * **Reversible (entropic)** `−I·T·∂U/∂T`. Zero unless the chemistry supplies an
///   entropy-coefficient table. With the usual negative `∂U/∂T`, discharge heats
///   and charge cools.
///
/// `r0` must be the cell's *effective* resistance (nominal × factors), and
/// `v_rc_sum` / `temp_k` its start-of-step values — the same ones that produced
/// `i` — so that the reported heat matches the electrical solve exactly.
#[must_use]
pub fn cell_heat_w(i: f64, r0: f64, v_rc_sum: f64, temp_k: f64, docv_dt_v_per_k: f64) -> f64 {
    let q_irrev = i * (i * r0 + v_rc_sum);
    let q_rev = -i * temp_k * docv_dt_v_per_k;
    q_irrev + q_rev
}

/// Ohmic series resistance `R0` \[ohms\] at `(soc, temp_k)`, by clamped bilinear
/// interpolation over the grid.
#[must_use]
pub fn r0_lookup(table: &R0Table, soc: f64, temp_k: f64) -> f64 {
    // Interpolate along temperature within each soc row, then across soc rows —
    // but only the two rows the SOC bracket actually blends. Interpolating every
    // row first (into a scratch Vec) would give the identical answer at the cost
    // of a heap allocation on a path that runs twice per cell per step.
    let (lo, hi, frac) = bracket(&table.soc, soc);
    let r_lo = interp1(&table.temp_k, &table.ohms[lo], temp_k);
    if lo == hi {
        return r_lo;
    }
    let r_hi = interp1(&table.temp_k, &table.ohms[hi], temp_k);
    r_lo + frac * (r_hi - r_lo)
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

/// Solve the operating current \[A\] for a [`Demand`] against a single Thévenin
/// source `e` behind resistance `r0`.
///
/// Terminal voltage at current `i` is `V(i) = e − i·r0`. This is closed-form for
/// every demand variant, including `Power` (a quadratic with a physical-root
/// selection). The pack layer calls this on its *aggregated* source
/// `(E_pack, R_pack)`: because each cell is a fixed linear Thévenin over the step,
/// the whole pack aggregates to one linear Thévenin and the same closed form is
/// exact — so Phase 1 deliberately does **not** use the Newton/bisection loop that
/// `CLAUDE.md` prescribes (that is forward-cover for models that are nonlinear
/// within a step, e.g. SPM/DFN or mid-step derating, which Phase 1 does not have).
#[must_use]
pub(crate) fn solve_current(demand: Demand, e: f64, r0: f64) -> f64 {
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

/// A cell's Thévenin equivalent for one step: source `e = OCV(soc,T) − Σ V_rc`
/// behind resistance `r = R0(soc,T)·r0_factor`, both evaluated from the cell's
/// current (start-of-step) state.
///
/// `r0_factor` folds in the cell's static manufacturing scatter / weak-cell
/// resistance multiplier (nominal × factor; aging's `soh_resistance` multiplies in
/// later). It is guaranteed `> 0` by the pack, so `r > 0`.
#[must_use]
pub(crate) fn cell_source(state: &EcmState, chem: &ChemistryParams, r0_factor: f64) -> (f64, f64) {
    let r = r0_lookup(&chem.r0, state.soc, state.temp_k) * r0_factor;
    let e = ocv_lookup(&chem.ocv, state.soc) - state.v_rc.iter().sum::<f64>();
    (e, r)
}

/// Advance one cell's internal state by `dt` seconds under the current `i`
/// \[A, discharge-positive\] that the pack solve assigned it.
///
/// Updates every RC overpotential (exact exponential update) and SOC (coulomb
/// counting). `eff_capacity_ah` is the cell's *effective* capacity — nominal ×
/// capacity_factor (× `soh_capacity` once aging lands). Returns the SOC-clamp
/// flags from the coulomb step. Terminal voltage is *not* returned here: the pack
/// recomputes each group's shared node voltage from the end-of-step state via
/// [`cell_source`] so parallel cells report one consistent voltage.
#[must_use]
pub(crate) fn advance_cell(
    state: &mut EcmState,
    chem: &ChemistryParams,
    i: f64,
    dt: f64,
    eff_capacity_ah: f64,
) -> EventFlags {
    for (k, v_rc) in state.v_rc.iter_mut().enumerate() {
        let pair = chem.rc[k];
        *v_rc = rc_update(*v_rc, i, pair.r_ohms, pair.c_farad, dt);
    }
    let (soc_new, flags) = coulomb_step(state.soc, i, dt, eff_capacity_ah, 1.0);
    state.soc = soc_new;
    flags
}
