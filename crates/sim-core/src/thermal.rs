//! Thermal network: one lumped temperature node per cell.
//!
//! Each cell is a single node with heat capacity `C_th`, obeying
//!
//! ```text
//! C_th · dT_i/dt = Q_i + Σ_j k_ij·(T_j − T_i) + hA_i·(T_env − T_i)
//! ```
//!
//! where `Q_i` is the cell's own heat generation (see [`crate::ecm::cell_heat_w`]),
//! `k_ij` couples neighbouring cells, and `hA_i` couples the cell to ambient (or
//! coolant). Integration is explicit Euler with automatic sub-stepping, which is
//! ample: thermal time constants are minutes while `dt` is typically well under a
//! second.
//!
//! # Why a temperature gradient exists at all
//!
//! It is tempting to expect that giving interior cells more conduction neighbours
//! makes them run hotter. It does not. With uniform heat generation, uniform
//! `hA`, and any *symmetric* conduction graph, substituting `T_i = T*` into the
//! equation above makes the neighbour sum vanish identically, leaving
//! `T* = T_env + Q/hA` for **every** cell no matter how it is wired. A pack
//! starting from a uniform temperature would stay exactly isothermal forever, and
//! conduction would be pure decoration.
//!
//! The gradient comes from **ambient coupling being position-dependent**: an
//! interior cell is insulated *by* its neighbours — the surface it shares with them
//! is surface that no longer faces the environment. That is what [`exposure`]
//! models, and it is why heat has to travel outward through the stack to escape,
//! leaving the middle hottest. It is also the seed of runaway-in-the-middle in a
//! later phase.
//!
//! # Geometry
//!
//! Cell positions come from the electrical topology: the `(series, parallel)` index
//! pair is read as a 2-D grid, 4-connected (up/down along the series axis,
//! left/right along the parallel axis). `CLAUDE.md` prescribes exactly this — "a
//! simple grid adjacency derived from topology". A `1S1P` pack has no neighbours
//! and is fully exposed; a `100S1P` string is a 1-D chain; a large `SxP` block has
//! a genuinely enclosed core.

use serde::{Deserialize, Serialize};

use crate::chem::ThermalParams;

/// Neighbour count of a cell with every side occupied, i.e. the coordination number
/// of the 4-connected grid. Used to normalise [`exposure`] so that a lone cell is
/// fully exposed and a fully surrounded cell is fully insulated.
const GRID_MAX_NEIGHBORS: f64 = 4.0;

/// Fraction of `C_th/a` used as the explicit-Euler sub-step ceiling, where `a` is a
/// node's total conductance. Stability alone needs `< 2`; the margin down to `0.5`
/// buys accuracy, since the point of sub-stepping is to keep a coarse client `dt`
/// from distorting the trajectory rather than merely to avoid divergence.
const SUBSTEP_SAFETY: f64 = 0.5;

/// Hard cap on thermal sub-steps per [`crate::Pack::step`], so a pathological `dt`
/// cannot make one step unbounded work.
///
/// For the shipped LFP parameters (`C_th` = 95 J/K, `hA` = 0.35 W/K) with a 1 W/K
/// neighbour conductance the sub-step ceiling is ≈ 11.9 s, so the cap only binds
/// for `dt` above roughly 1.7 hours — far beyond any real-time or scenario use.
/// Beyond it the integration is no longer guaranteed stable; the coarse-`dt`
/// fast-forward of a later phase will need a different integrator rather than a
/// bigger cap.
const MAX_SUBSTEPS: u32 = 512;

/// How cells exchange heat with each other and the environment.
///
/// Per the design contract components are toggleable: [`ThermalConfig::Isothermal`]
/// is a supported mode, not a degraded one. It is also the default, which keeps the
/// analytic and PyBaMM goldens testing the *electrical* model in isolation — the
/// references they compare against were generated isothermal, and the shipped
/// chemistries have a temperature-dependent `R0`, so a live thermal model would
/// shift those trajectories for reasons unrelated to what the goldens check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ThermalConfig {
    /// Every cell stays at its initial temperature forever. Heat generation is
    /// still reported, it simply does not feed back into temperature.
    #[default]
    Isothermal,
    /// Live lumped-node network over the topology grid.
    Network {
        /// Conductance `k_ij` \[W/K\] between two 4-connected grid neighbours.
        /// Must be finite and `>= 0`; `0` thermally isolates the cells from each
        /// other while still letting each exchange heat with the environment.
        k_neighbor_w_per_k: f64,
    },
}

/// Number of 4-connected grid neighbours of the cell at series index `s`, parallel
/// index `p`, in an `series`×`parallel` grid.
///
/// Both extents must be `>= 1` and the indices in range (guaranteed by the pack).
#[must_use]
pub fn n_neighbors(s: usize, p: usize, series: usize, parallel: usize) -> usize {
    usize::from(s > 0)
        + usize::from(s + 1 < series)
        + usize::from(p > 0)
        + usize::from(p + 1 < parallel)
}

/// Fraction of the bare-cell convective conductance that the cell at `(s, p)`
/// retains: `(4 − n_neighbors)/4`, in \[0, 1\].
///
/// A lone cell (`1S1P`) keeps all of it, which is what makes
/// [`ThermalParams::h_area_w_per_k`] measurable as a bare-cell property. A cell
/// with neighbours on all four sides keeps none and can only shed heat by
/// conducting toward the edge of the pack. See the module docs for why this, not
/// the conduction graph, is what creates a gradient.
#[must_use]
pub fn exposure(s: usize, p: usize, series: usize, parallel: usize) -> f64 {
    let n = n_neighbors(s, p, series, parallel) as f64;
    (GRID_MAX_NEIGHBORS - n) / GRID_MAX_NEIGHBORS
}

/// Number of sub-steps to split `dt` into, and the resulting sub-step length.
///
/// The bound uses the worst-case total node conductance `a_max = max(4k, hA)`: a
/// node's conductance `n·k + (4−n)/4·hA` is linear in its neighbour count `n`, so
/// over `n ∈ [0, 4]` it is maximised at an endpoint. Using the bound rather than a
/// per-cell scan makes the sub-step count a function of config alone — the same for
/// every cell and every step, which keeps the trajectory independent of *where* in
/// the pack the hottest cell happens to be.
fn substeps(params: &ThermalParams, k: f64, dt: f64) -> (u32, f64) {
    let a_max = (GRID_MAX_NEIGHBORS * k).max(params.h_area_w_per_k);
    let has_coupling = a_max > 0.0;
    if !has_coupling {
        // Fully adiabatic and uncoupled: dT/dt = Q/C has no stability bound.
        return (1, dt);
    }
    let dt_max = SUBSTEP_SAFETY * params.heat_capacity_j_per_k / a_max;
    // `dt_max` is finite and positive here. A NaN `dt` gives a NaN ratio, which
    // fails the test below and falls through to a single sub-step so the NaN
    // propagates into the temperatures — `step` must never panic.
    let ratio = dt / dt_max;
    let needs_split = ratio > 1.0;
    if !needs_split {
        return (1, dt);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = if ratio >= f64::from(MAX_SUBSTEPS) {
        MAX_SUBSTEPS
    } else {
        // 1 < ratio < MAX_SUBSTEPS, so the ceiling fits in u32.
        ratio.ceil() as u32
    };
    (n, dt / f64::from(n))
}

/// Everything [`advance_temperatures`] needs besides the state it mutates: the pack
/// geometry, the cell's thermal properties, the coupling, and this step's
/// environment.
pub(crate) struct ThermalStep<'a> {
    /// Series extent of the topology grid.
    pub series: usize,
    /// Parallel extent of the topology grid.
    pub parallel: usize,
    /// Per-cell thermal properties, from the chemistry.
    pub params: &'a ThermalParams,
    /// Neighbour conductance \[W/K\] (see [`ThermalConfig::Network`]).
    pub k_neighbor_w_per_k: f64,
    /// Sink temperature \[K\]: coolant if the environment supplies one, else ambient.
    pub t_env: f64,
    /// Step length \[s\].
    pub dt: f64,
}

/// Advance every cell temperature by `step.dt` seconds.
///
/// `temps` is the per-cell temperature \[K\] in series-major order (index
/// `s·parallel + p`), read and written in place. `heat_w` is the matching per-cell
/// generation \[W\], held constant across the step (the electrical solve runs once
/// per step). `scratch` is used as the previous-iterate buffer so the update is a
/// true simultaneous (Jacobi) step — updating in place would make the result depend
/// on iteration order.
pub(crate) fn advance_temperatures(
    temps: &mut [f64],
    scratch: &mut Vec<f64>,
    heat_w: &[f64],
    step: &ThermalStep,
) {
    let &ThermalStep {
        series,
        parallel,
        params,
        k_neighbor_w_per_k: k,
        t_env,
        dt,
    } = step;
    debug_assert_eq!(temps.len(), series * parallel);
    debug_assert_eq!(heat_w.len(), temps.len());
    let c_th = params.heat_capacity_j_per_k;
    let (n_sub, h) = substeps(params, k, dt);

    for _ in 0..n_sub {
        scratch.clear();
        scratch.extend_from_slice(temps);
        for s in 0..series {
            for p in 0..parallel {
                let i = s * parallel + p;
                let t_i = scratch[i];
                // Conduction to the (up to four) grid neighbours. k_ij = k_ji by
                // construction, so internal exchange cancels pack-wide — the
                // energy-balance test relies on that symmetry.
                let mut flow = 0.0;
                if s > 0 {
                    flow += k * (scratch[i - parallel] - t_i);
                }
                if s + 1 < series {
                    flow += k * (scratch[i + parallel] - t_i);
                }
                if p > 0 {
                    flow += k * (scratch[i - 1] - t_i);
                }
                if p + 1 < parallel {
                    flow += k * (scratch[i + 1] - t_i);
                }
                // Convection to ambient/coolant, scaled by how much of this cell's
                // surface is not blocked by neighbours.
                flow += exposure(s, p, series, parallel) * params.h_area_w_per_k * (t_env - t_i);
                temps[i] = t_i + h * (heat_w[i] + flow) / c_th;
            }
        }
    }
}
