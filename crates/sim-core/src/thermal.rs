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

use crate::chem::{SafetyParams, ThermalParams};
use crate::runaway::{is_vented, reaction_power, reaction_power_slope, CellRunaway};

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

/// Largest temperature change \[K\] one sub-step may produce in any cell while a
/// reaction term is live.
///
/// This is an **accuracy** bound, and it is the one that matters during runaway — the
/// stability bound below guarantees only that the *linearised* problem does not
/// oscillate, and says nothing about the reaction rate itself growing tenfold inside
/// the sub-step just taken. The Arrhenius logarithmic sensitivity is
/// `∂lnQ/∂T = Ea/(R·T²)`, ≈ 0.067/K at 423 K and ≈ 0.012/K at 1000 K for `Ea` = 1e5
/// J/mol, so a 1 K cap holds the reaction term to under ~7 % drift within a sub-step
/// across the whole trajectory. Held constant rather than derived from `Ea` because it
/// is a discretisation choice, not a property of the chemistry.
const MAX_SUBSTEP_RISE_K: f64 = 1.0;

/// Hard cap on sub-steps per [`crate::Pack::step`] while a reaction term is live.
///
/// Separate from (and much larger than) [`MAX_SUBSTEPS`], because the two bound
/// different things. That one bounds work against a pathological client `dt`; this one
/// has to accommodate a burn whose cost is set by the *chemistry* and is very nearly
/// independent of `dt` — a runaway completes in a fraction of a second of simulation
/// time, so the whole event lands inside a single step at any realistic `dt`.
///
/// The arithmetic, per `CLAUDE.md`'s rule that a raised cap comes with its working:
/// a full burn moves a cell by its adiabatic rise `runaway_energy_j /
/// heat_capacity_j_per_k`, and [`MAX_SUBSTEP_RISE_K`] allows 1 K of that per sub-step.
///
/// | chemistry | budget | `C_th` | adiabatic rise | sub-steps for one full burn |
/// | --------- | ------ | ------ | -------------- | --------------------------- |
/// | LFP 26650 | 24 kJ | 95 J/K | 253 K | 253 |
/// | NMC 18650 | 45 kJ | 55 J/K | 818 K | 819 |
///
/// 2048 is ~2.5× the worse of the two, which leaves room for two cells igniting in
/// sequence within one step and for the stability bound binding harder than the rise
/// bound near the peak. Beyond it the integration is no longer trustworthy and the
/// same `debug_assert` treatment as [`MAX_SUBSTEPS`] applies.
const MAX_RUNAWAY_SUBSTEPS: u32 = 2048;

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

/// Worst-case total conductance \[W/K\] of any node under the *linear* physics.
///
/// A node's conductance `n·k + (4−n)/4·hA` is linear in its neighbour count `n`, so
/// over `n ∈ [0, 4]` it is maximised at an endpoint.
fn linear_a_max(params: &ThermalParams, k: f64) -> f64 {
    (GRID_MAX_NEIGHBORS * k).max(params.h_area_w_per_k)
}

/// Number of sub-steps to split `dt` into, and the resulting sub-step length.
///
/// The bound uses the worst-case total node conductance `a_max = max(4k, hA)`: a
/// node's conductance `n·k + (4−n)/4·hA` is linear in its neighbour count `n`, so
/// over `n ∈ [0, 4]` it is maximised at an endpoint. Using the bound rather than a
/// per-cell scan makes the sub-step count a function of config alone — the same for
/// every cell and every step, which keeps the trajectory independent of *where* in
/// the pack the hottest cell happens to be.
///
/// **This bound covers the linear physics only.** An Arrhenius reaction term
/// ([`crate::runaway`]) has a temperature derivative that exceeds `a_max` by orders of
/// magnitude a few hundred kelvin above onset, so a pack with a live reaction takes the
/// adaptive path in [`advance_temperatures`] instead and the config-alone property is
/// retired for the duration. Determinism is untouched either way: the sub-step count
/// remains a deterministic function of state, which is all `CLAUDE.md` requires.
fn substeps(params: &ThermalParams, k: f64, dt: f64) -> (u32, f64) {
    let a_max = linear_a_max(params, k);
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
        // The cap binds, so the sub-step is longer than the ceiling and the
        // integration is no longer guaranteed stable — past the bound explicit
        // Euler oscillates rather than merely losing accuracy. `step` must not
        // panic in release, and `EventFlags` has no bit for "your dt is absurd", so
        // this is as loud as it can be made without inventing surface area. See the
        // `dt` warning on `Pack::step`.
        debug_assert!(
            false,
            "thermal sub-step cap of {MAX_SUBSTEPS} binds at dt = {dt} s \
             (ceiling {dt_max} s): temperatures are not trustworthy"
        );
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
    /// Emergent-failure thresholds, or `None` for a chemistry with no `[safety]`
    /// section — which is exactly a chemistry that cannot run away.
    pub safety: Option<&'a SafetyParams>,
}

/// What [`advance_temperatures`] found out about the exothermic reaction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RunawayReport {
    /// Exothermic energy \[J\] released across the whole pack during this call.
    pub released_j: f64,
    /// Whether the reaction term was live on at least one cell at the start of this
    /// step — i.e. whether the adaptive path ran at all.
    ///
    /// This is what raises [`crate::EventFlags::THERMAL_RUNAWAY`], and it is `true`
    /// even on a zero-length probe step, where the reaction is real but no time passes
    /// for it to release anything.
    pub reacting: bool,
}

/// One explicit-Euler sub-step of length `h` over the whole grid.
///
/// `q` is the total per-cell heating \[W\] to hold constant across this sub-step —
/// electrochemical generation alone on the linear path, plus the reaction term on the
/// adaptive one. `scratch` carries the previous iterate so the update is a true
/// simultaneous (Jacobi) step; updating in place would make the result depend on
/// iteration order.
fn euler_substep(temps: &mut [f64], scratch: &mut Vec<f64>, q: &[f64], step: &ThermalStep, h: f64) {
    let &ThermalStep {
        series,
        parallel,
        params,
        k_neighbor_w_per_k: k,
        t_env,
        ..
    } = step;
    let c_th = params.heat_capacity_j_per_k;
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
            temps[i] = t_i + h * (q[i] + flow) / c_th;
        }
    }
}

/// Advance every cell temperature by `step.dt` seconds.
///
/// `temps` is the per-cell temperature \[K\] in series-major order (index
/// `s·parallel + p`), read and written in place. `heat_w` is the matching per-cell
/// generation \[W\], held constant across the step (the electrical solve runs once
/// per step). `runaway` is the matching per-cell exothermic state, read and written in
/// place; pass an **empty slice** for a pack that cannot react, which is what the
/// caller does whenever no cell has reached onset — that keeps a healthy pack from
/// paying to gather it.
///
/// # Two paths
///
/// With no cell reacting this runs the original uniform partition, bit-for-bit: the
/// arithmetic below is reached through the same `q` slice (`heat_w` itself, not a copy
/// with zeros added), so every pre-runaway trajectory is unchanged.
///
/// With a cell reacting it switches to a variable sub-step: the length is re-derived
/// from the current state before every sub-step, bounded by both the linear stability
/// ceiling *and* [`MAX_SUBSTEP_RISE_K`]. The rise bound is the operative one — see its
/// docs for why the stability bound alone is not enough against an exponential.
pub(crate) fn advance_temperatures(
    temps: &mut [f64],
    scratch: &mut Vec<f64>,
    heat_w: &[f64],
    runaway: &mut [CellRunaway],
    step: &ThermalStep,
) -> RunawayReport {
    let &ThermalStep {
        series,
        parallel,
        params,
        k_neighbor_w_per_k: k,
        dt,
        safety,
        ..
    } = step;
    debug_assert_eq!(temps.len(), series * parallel);
    debug_assert_eq!(heat_w.len(), temps.len());
    debug_assert!(runaway.is_empty() || runaway.len() == temps.len());

    // The gate, evaluated once from start-of-step temperatures. A cell that crosses
    // onset *during* this step therefore ignites at the start of the next one; see the
    // "ignition lags by one step" section of [`crate::runaway`] for why that is bought
    // deliberately rather than overlooked.
    let reacting = match safety {
        Some(s) if runaway.len() == temps.len() => temps
            .iter()
            .zip(runaway.iter())
            .any(|(&t, r)| reaction_power(s, t, r.energy_remaining_j) > 0.0),
        _ => false,
    };
    if !reacting {
        let (n_sub, h) = substeps(params, k, dt);
        for _ in 0..n_sub {
            euler_substep(temps, scratch, heat_w, step, h);
        }
        return RunawayReport::default();
    }

    let s = safety.expect("`reacting` is only true when safety is Some");
    let c_th = params.heat_capacity_j_per_k;
    let a_lin = linear_a_max(params, k);
    let n = temps.len();
    // Allocated per call, on a path that only runs while the pack is on fire. The
    // linear path above allocates nothing, which is the one that has a perf budget.
    let mut q_rxn = vec![0.0; n];
    let mut q_total = vec![0.0; n];
    let mut released_j = 0.0;
    let mut remaining = dt;
    let mut taken = 0u32;

    while remaining > 0.0 {
        let mut slope_max = 0.0_f64;
        let mut q_node_max = 0.0_f64;
        for i in 0..n {
            let q = reaction_power(s, temps[i], runaway[i].energy_remaining_j);
            q_rxn[i] = q;
            slope_max = slope_max.max(reaction_power_slope(s, temps[i], q));
            q_node_max = q_node_max.max((heat_w[i] + q).abs());
        }
        // Stability against the linear conductances *plus* the reaction's own
        // derivative at the hottest reacting cell, and accuracy against the reaction's
        // curvature. Whichever binds harder wins; `remaining` caps both so a sub-step
        // never overshoots the step.
        let mut h = remaining;
        let a = a_lin + slope_max;
        if a > 0.0 {
            h = h.min(SUBSTEP_SAFETY * c_th / a);
        }
        if q_node_max > 0.0 {
            h = h.min(MAX_SUBSTEP_RISE_K * c_th / q_node_max);
        }
        let usable = h > 0.0;
        let capped = taken >= MAX_RUNAWAY_SUBSTEPS;
        if capped || !usable {
            // Either the work cap bound or the bounds themselves degenerated (an
            // infinite release rate from an absurd parameter set, or a NaN). Finish the
            // step in one Euler jump so `step` still advances by exactly `dt` and never
            // hangs; the temperatures that come out are not trustworthy, and this is as
            // loud as it can be made without inventing public surface area.
            debug_assert!(
                false,
                "runaway sub-stepping degenerated at t_max = {:?} K after {taken} sub-steps \
                 (cap {MAX_RUNAWAY_SUBSTEPS}, remaining {remaining} s): temperatures are not \
                 trustworthy",
                temps.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            );
            h = remaining;
        }
        // Clip each release rate to what the cell can actually still pay over this
        // sub-step, *before* integrating. Doing it afterwards would let the temperature
        // rise on energy the budget did not have — a cell that releases more heat than
        // `runaway_energy_j` over its lifetime, visible only as an energy-balance
        // residual nobody would trace back to here.
        for i in 0..n {
            let affordable = runaway[i].energy_remaining_j / h;
            if q_rxn[i] > affordable {
                q_rxn[i] = affordable;
            }
            q_total[i] = heat_w[i] + q_rxn[i];
        }
        euler_substep(temps, scratch, &q_total, step, h);
        for i in 0..n {
            let e = q_rxn[i] * h;
            released_j += e;
            runaway[i].energy_remaining_j = (runaway[i].energy_remaining_j - e).max(0.0);
            // Latched here as well as in the caller's reporting pass: a cell that
            // crosses the vent threshold mid-step and cools back below it before the
            // step ends still vented, and the end-of-step scan alone would miss it.
            if is_vented(s, temps[i]) {
                runaway[i].vented = true;
            }
        }
        // `h <= remaining` always, and `h == remaining` exactly whenever neither bound
        // binds — so the loop terminates on an exact zero rather than a residue.
        remaining = (remaining - h).max(0.0);
        taken += 1;
    }

    RunawayReport {
        released_j,
        reacting: true,
    }
}
