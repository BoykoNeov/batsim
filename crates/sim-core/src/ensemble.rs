//! Many-particle cell model (`SpmEnsemble`) — Phase 9.
//!
//! The single-particle model ([`crate::spm`]) with its **positive** electrode replaced by
//! `N` particles of different sizes that share one electrode potential. The negative
//! electrode stays one particle. Everything else — the shells, the backward-Euler diffusion,
//! the Butler–Volmer kinetics, the reversal past empty, the end-of-step curve the pack solves
//! on — is the single-particle model's, called rather than copied.
//!
//! # Why several particles
//! An LFP particle separates into a full and an empty phase, and its open-circuit potential
//! ([`crate::RegularSolutionParams`]) rises through its middle instead of falling. One
//! particle on that curve gives a voltage that climbs during a discharge. Many particles on
//! it fill one at a time: whichever is furthest along takes the current, so the electrode's
//! potential stays on a flat plateau while the particles take turns, and it rests at a
//! different voltage depending on the direction it arrived from. Both effects come out of
//! the particles; neither is put in by hand. Measured before this model was built, in
//! `docs/plans/phase-9-slice-a-spike.md`.
//!
//! # The split, inside the cell
//! Each step the cell's current is divided among its particles so that every particle's
//! surface sits at the same electrode potential and the shares add up to the whole. That is a
//! parallel group in miniature, solved the way the pack solves one: each particle's potential
//! against its current is replaced by its tangent, the shared potential follows in closed
//! form, and the tangents are re-taken until the shares stop moving ([`solve_split`]). The
//! pack never sees the particles: principle 9 of `CLAUDE.md`.
//!
//! # Sub-steps, fixed before the split
//! Over a long step lithium moves between particles inside the step, and one backward-Euler
//! step cannot follow it. So a step is cut into [`substeps`] equal pieces, and **the count
//! depends on the step length and the particle count alone** — never on the current being
//! tried. The spike's first harness halved a sub-step whenever a trial failed, and the jump
//! in sub-step count between two neighbouring trial currents alone turned a 6 mV property
//! into a 30 mV one. A split that does not solve at the fixed count is flagged
//! ([`EventFlags::SOLVE_UNCONVERGED`]), not retried finer.
//!
//! # One particle is the single-particle model, bit for bit
//! With `N = 1`, no size spread and a table potential, every expression here reduces to the
//! one [`crate::spm`] evaluates, in the same order: one particle carries the whole current,
//! so there is no split to solve and nothing to sub-step for. That is exit criterion 1 of
//! `docs/plans/phase-9-lfp-ensemble.md`, and it is checked against the single-particle model
//! on every scenario its tests run.

use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::aging::GAS_CONSTANT_J_PER_MOL_K;
use crate::chem::{ElectrodeParams, ReversalParams, SpmParams};
use crate::flags::EventFlags;
use crate::math;
use crate::noise;
use crate::spm::{
    self, clamp_surface, diffuse, forward_sweep, mean_concentration, ocp_lookup, overpotential,
    surface_from_outer, window_fraction_neg, window_fraction_pos, Geometry, OuterShell, Side,
    Working, FARADAY_C_PER_MOL, MAX_SHELLS, SURFACE_EDGE,
};

/// Fewest particles a positive electrode may be split into. One is the single-particle
/// model, and is allowed so that the guard above can be run.
pub const MIN_PARTICLES: usize = 1;

/// Most particles a positive electrode may be split into.
///
/// Not physics: the length of the stack rows the split and the probe use, which keep the
/// step allocation-free (the reason [`crate::spm::MAX_SHELLS`] is a constant too). The
/// spike measured up to 100 particles on the half-cell and 40 on the full cell; 20 is what
/// the plan builds on.
pub const MAX_PARTICLES: usize = 64;

/// Longest sub-step \[s\] a step of a many-particle cell is cut into.
///
/// # Measured, on the slice-B LFP fixture, 20 particles
/// End-of-step voltage against current at 20 states along a C/20 discharge, 61 currents
/// each; and a C/20 discharge at the long step against the 1 s trajectory, with the rest gap
/// it reaches. At a 15 min step:
///
/// | sub-step | falls with current | worst drift from 1 s | rest gap | per step |
/// | -------- | ------------------ | -------------------- | -------- | -------- |
/// | 1 s      | 20 / 20            | 0                    | 19.64 mV | 93 ms    |
/// | 4 s      | 20 / 20            | 2.45 mV              | 19.64 mV | 24 ms    |
/// | **10 s** | **20 / 20**        | **3.0 mV**           | 19.64 mV | **10 ms**|
/// | 30 s     | 20 / 20            | 3.0 mV               | 19.64 mV | 3.5 ms   |
/// | 60 s     | **16 / 20**        | 3.0 mV               | 19.64 mV | 11 ms    |
///
/// At 30 s the split already needs its bracketed half 6 750 times in the sweep where 10 s
/// needs it 210, and at 60 s half a million times — the potential is starting to rise
/// within a sub-step — and the voltage rises with current at four states. 10 s keeps a
/// factor of three from where that starts, costs a ninth of 1 s, and drifts 3 mV from it. A
/// step of 10 s or less is one sub-step, so the real-time path is the same at any length
/// here. See `docs/plans/phase-9-slice-b-ensemble.md`.
pub const SUBSTEP_S: f64 = 10.0;

/// Most sub-steps one step is cut into. Above `SUBSTEP_S · MAX_SUBSTEPS` the sub-step grows
/// with the step instead, which is past the step length this model's long-step limit
/// already excludes (`docs/plans/phase-9-lfp-ensemble.md` §"Long steps").
pub const MAX_SUBSTEPS: usize = 3600;

/// The split has converged when no particle's share moved by more than this fraction of the
/// cell's capacity in amperes on the last pass \[A per Ah\]. At a tangent slope of the order
/// the kinetics give (≈ 7 mΩ per particle at 20 particles on the LFP cell) that is a few
/// femtovolts of disagreement between particles: far below what the pack's own difference
/// quotient can see, which is the requirement.
const SPLIT_TOL_A_PER_AH: f64 = 1.0e-12;

/// Floor \[ohms\] on one particle's tangent resistance inside [`solve_split`], so a particle
/// whose potential has stopped falling with its current cannot divide by zero. The same
/// role, and the same value, as the floor in [`crate::spm::probe_at`].
const R_FLOOR_OHMS: f64 = 1.0e-9;

/// Per-cell many-particle state: one negative particle, `N` positive particles with their
/// radii, and the currents the last step left each particle carrying.
///
/// # This is the whole state
/// As for [`crate::SpmState`], concentrations are shell averages \[mol/m³\], innermost
/// first, and nothing here is a cache. `radii_m` was drawn once, at [`crate::Pack::new`],
/// from the pack's seeded generator; a restore does not run that constructor again, which is
/// why the radii are stored rather than re-drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnsembleState {
    /// Negative-particle shell concentrations \[mol/m³\], innermost first.
    pub c_neg: Vec<f64>,
    /// Positive-particle shell concentrations \[mol/m³\], particle by particle, each
    /// innermost first: particle `k` is `c_pos[k·shells .. (k+1)·shells]`.
    pub c_pos: Vec<f64>,
    /// Each positive particle's radius \[m\]. Every particle holds the same **volume** of
    /// active material whatever its radius; the radius sets only its surface area,
    /// `3·volume/radius`. With equal volumes the electrode's resting voltage depends only on
    /// how many particles are full, which is what the cold rest gap is made of.
    pub radii_m: Vec<f64>,
    /// The current \[A, discharge-positive\] each positive particle carried over the last
    /// sub-step of the last step. They add up to the particles' share of
    /// [`Self::i_last`].
    ///
    /// **State, not a cache**, for two reasons. The split's tangent passes start from them,
    /// so they decide where the next step's split lands to within its tolerance. And a read
    /// where no time passes divides a current among the particles from them.
    pub i_particles_last: Vec<f64>,
    /// Cell temperature \[K\], advanced by [`crate::thermal`].
    pub temp_k: f64,
    /// Current \[A, discharge-positive\] this cell carried over the previous step. See
    /// [`crate::SpmState::i_last`].
    pub i_last: f64,
    /// Charge drawn past empty, as a fraction of the cell's effective capacity. See
    /// [`crate::SpmState::soc_deficit`].
    pub soc_deficit: f64,
    /// The share of [`Self::i_last`] \[A\] the reversal carried rather than the particles.
    /// See [`crate::SpmState::i_reversal_last`].
    pub i_reversal_last: f64,
}

impl EnsembleState {
    /// A fresh cell at `soc`: every particle uniform at the concentration that state of
    /// charge implies, every radius at the chemistry's `particle_radius_m`. [`draw_radii`]
    /// spreads the radii afterwards.
    ///
    /// Uniform is not where an ensemble on a two-phase curve wants to sit — in the middle
    /// of the curve it is unstable, and the first current that flows separates it. A cell
    /// started at either end of its window starts where the material is a single phase.
    pub(crate) fn new(
        spm: &SpmParams,
        shells: usize,
        particles: usize,
        soc: f64,
        temp_k: f64,
    ) -> Self {
        let x = spm.negative.stoich_min + soc * (spm.negative.stoich_max - spm.negative.stoich_min);
        let y = spm.positive.stoich_max - soc * (spm.positive.stoich_max - spm.positive.stoich_min);
        Self {
            c_neg: vec![x * spm.negative.c_max_mol_per_m3; shells],
            c_pos: vec![y * spm.positive.c_max_mol_per_m3; shells * particles],
            radii_m: vec![spm.positive.particle_radius_m; particles],
            i_particles_last: vec![0.0; particles],
            temp_k,
            i_last: 0.0,
            soc_deficit: 0.0,
            i_reversal_last: 0.0,
        }
    }
}

/// Spread `radii` lognormally around `median_m`: `R = median · exp(σ·z)`, `z` standard
/// normal, two particles per Box–Muller pair.
///
/// **Draws nothing** for one particle or for `σ = 0`, so a pack that asks for no spread
/// leaves the generator exactly where it was — and with it every later draw (scatter,
/// sensor noise, plating) of every pack that does not use this model.
pub(crate) fn draw_radii(rng: &mut ChaCha8Rng, median_m: f64, sigma: f64, radii: &mut [f64]) {
    if radii.len() <= 1 || sigma == 0.0 {
        return;
    }
    let mut k = 0;
    while k < radii.len() {
        let (z0, z1) = noise::standard_normal_pair(rng);
        radii[k] = median_m * math::exp(sigma * z0);
        if k + 1 < radii.len() {
            radii[k + 1] = median_m * math::exp(sigma * z1);
        }
        k += 2;
    }
}

/// The sub-step count for a step of `dt` seconds on a cell of `n` particles: `1` for one
/// particle — it has no split for a long step to outrun — and otherwise `dt / SUBSTEP_S`
/// rounded up, capped at [`MAX_SUBSTEPS`]. A function of the step and the cell's shape
/// alone, never of the current being tried; see the module note.
#[must_use]
pub fn substeps(n: usize, dt: f64) -> usize {
    if n <= 1 || dt.is_nan() || dt <= 0.0 {
        return 1;
    }
    let m = (dt / SUBSTEP_S).ceil();
    if m >= MAX_SUBSTEPS as f64 {
        MAX_SUBSTEPS
    } else if m >= 1.0 {
        m as usize
    } else {
        1
    }
}

/// The positive electrode's open-circuit potential \[V\] at stoichiometry `y`: the
/// regular-solution form where the chemistry gives one, the table otherwise.
///
/// The regular-solution form is read with `y` held inside `[SURFACE_EDGE, 1 − SURFACE_EDGE]`
/// — the band the surface clamp already keeps every surface in — because its logarithm is
/// infinite at both ends.
#[must_use]
pub fn positive_ocp(p: &ElectrodeParams, temp_k: f64, y: f64) -> f64 {
    match &p.regular_solution {
        None => ocp_lookup(&p.ocp, y),
        Some(rs) => {
            let y = y.clamp(SURFACE_EDGE, 1.0 - SURFACE_EDGE);
            let kt_v = GAS_CONSTANT_J_PER_MOL_K * temp_k / FARADAY_C_PER_MOL;
            rs.u0_v - kt_v * math::ln(y / (1.0 - y)) - rs.omega_ev * (1.0 - 2.0 * y)
        }
    }
}

/// The positive particles as one step sees them: each one's radius, surface area and share
/// of that area, derived per call from the radii and the chemistry.
#[derive(Clone, Copy, Debug)]
struct Particles {
    n: usize,
    shells: usize,
    /// Each particle's volume of active material \[m³\]: the electrode's, divided evenly.
    volume_m3: f64,
    radius_m: [f64; MAX_PARTICLES],
    /// `3·volume/radius` \[m²\].
    area_m2: [f64; MAX_PARTICLES],
    /// Each particle's share of the electrode's whole surface area. What the reversal's
    /// current is spread by past empty (see [`solve_split`]); unused at `N = 1`.
    weight: [f64; MAX_PARTICLES],
}

impl Particles {
    fn of(w: &Working<'_>, s: &EnsembleState) -> Self {
        let n = s.radii_m.len();
        // `/ 1.0` is exact, so one particle's volume and area are the bits
        // `spm::Geometry::of` produces for the whole electrode.
        let volume_m3 = w.pos.g.volume_m3 / n as f64;
        let mut radius_m = [0.0; MAX_PARTICLES];
        let mut area_m2 = [0.0; MAX_PARTICLES];
        let mut total = 0.0;
        for k in 0..n {
            radius_m[k] = s.radii_m[k];
            area_m2[k] = 3.0 * volume_m3 / radius_m[k];
            total += area_m2[k];
        }
        let mut weight = [0.0; MAX_PARTICLES];
        for k in 0..n {
            weight[k] = area_m2[k] / total;
        }
        Self {
            n,
            shells: s.c_neg.len(),
            volume_m3,
            radius_m,
            area_m2,
            weight,
        }
    }

    /// Particle `k` as one electrode of the single-particle model: the chemistry's material,
    /// at this particle's geometry.
    fn side<'a>(&self, w: &Working<'a>, k: usize) -> Side<'a> {
        Side {
            p: w.pos.p,
            g: Geometry {
                area_m2: self.area_m2[k],
                volume_m3: self.volume_m3,
            },
            d_s: w.pos.d_s,
            m_ref: w.pos.m_ref,
        }
    }

    /// Molar flux \[mol/(m²·s)\] leaving particle `k`'s surface when it carries `x` \[A,
    /// discharge-positive\]: negative on discharge, as [`Working::j_pos`]. The same
    /// expression, at the particle's own area.
    fn j(&self, w: &Working<'_>, k: usize, x: f64) -> f64 {
        -w.kappa * x / (self.area_m2[k] * FARADAY_C_PER_MOL)
    }

    fn profile<'c>(&self, c_pos: &'c [f64], k: usize) -> &'c [f64] {
        &c_pos[k * self.shells..(k + 1) * self.shells]
    }
}

/// One positive particle's contribution to the terminal voltage \[V\]: its open-circuit
/// potential at the surface plus its Butler–Volmer overpotential — [`spm::half`] with the
/// particle's own radius and [`positive_ocp`] in place of the table.
#[must_use]
fn pos_half(
    w: &Working<'_>,
    side: &Side<'_>,
    c_outer: f64,
    shells: usize,
    radius_m: f64,
    j_surf: f64,
    i_s: f64,
) -> f64 {
    let c_max = side.p.c_max_mol_per_m3;
    let c_s = clamp_surface(
        surface_from_outer(c_outer, shells, radius_m, side.d_s, j_surf),
        c_max,
    );
    positive_ocp(side.p, w.temp_k, c_s / c_max)
        + overpotential(side, w.temp_k, w.spm.c_e_mol_per_m3, c_s, i_s)
}

/// An [`OuterShell`] that is the identity: its "end of step" is the outer shell as stored,
/// whatever the flux. What a read where no time passes splits the current on.
fn stored_shell(c_outer: f64) -> OuterShell {
    OuterShell {
        t: c_outer,
        k: 0.0,
        m: 0.0,
        d: 1.0,
    }
}

/// [`spm`]'s forward sweep on a stack copy of `c`: the outer shell's end-of-step
/// concentration as a function of flux, for a step of `dt > 0`.
fn sweep(c: &[f64], r_p: f64, d_s: f64, dt: f64) -> OuterShell {
    let mut row = [0.0_f64; MAX_SHELLS];
    let mut diag = [0.0_f64; MAX_SHELLS];
    let row = &mut row[..c.len()];
    row.copy_from_slice(c);
    forward_sweep(row, &mut diag, r_p, d_s, dt)
}

/// What [`solve_split`] answers: the electrode potential the particles share \[V\], and
/// whether the shares converged. The shares themselves go to the caller's buffer.
#[derive(Clone, Copy, Debug)]
struct Split {
    phi: f64,
    converged: bool,
}

/// Divide the particles' current `i_p` \[A, discharge-positive\] among the positive
/// particles over one (sub-)step whose end-of-step outer shells are `outers`, so that every
/// particle's potential is the same and the shares add up to `i_p`. The shares are written
/// to `x`; `seed` is where the tangent passes start, shifted evenly by area so it sums to
/// `i_p`.
///
/// `i` is the cell's whole current. Inside the window it equals `i_p`. Past empty the
/// reversal carries `i − i_p` across the same interfaces (see [`spm::split_from`]), and it
/// is spread over the particles by area: particle `k`'s kinetics carry `x_k + (i − i_p)·a_k`,
/// written `i·a_k − (i_p·a_k − x_k)`: for one particle `a = 1` and `x = i_p`, so the bracket
/// is an exact zero and the kinetics carry exactly `i`, as the single-particle model's do.
///
/// # How
/// Each pass replaces every particle's potential by its tangent at its current share, solves
/// the resulting parallel group for the shared potential in closed form — the pack's own
/// group solve — and moves each share to its tangent's answer. A pass preserves the sum, so
/// only the shares' agreement is iterated. The last share is then re-derived from the others
/// so the sum is exact to rounding, which is what charge conservation rests on.
///
/// One particle carries everything: no pass runs, and its potential is the single-particle
/// model's positive half, bit for bit.
fn solve_split(
    w: &Working<'_>,
    pp: &Particles,
    outers: &[OuterShell],
    i_p: f64,
    i: f64,
    seed: &[f64],
    x: &mut [f64],
) -> Split {
    let n = pp.n;
    let v_at = |k: usize, xk: f64| {
        let side = pp.side(w, k);
        let j = pp.j(w, k, xk);
        // One particle's weight is 1 and its share is `i_p`, so this is `i − 0`: exactly `i`.
        let kin = i * pp.weight[k] - (i_p * pp.weight[k] - xk);
        pos_half(
            w,
            &side,
            outers[k].at(j),
            pp.shells,
            pp.radius_m[k],
            j,
            -kin / pp.area_m2[k],
        )
    };
    if n == 1 {
        x[0] = i_p;
        return Split {
            phi: v_at(0, i_p),
            converged: true,
        };
    }
    let seed_sum: f64 = seed[..n].iter().sum();
    let mut start = [0.0_f64; MAX_PARTICLES];
    for k in 0..n {
        start[k] = seed[k] + (i_p - seed_sum) * pp.weight[k];
    }
    let tol = SPLIT_TOL_A_PER_AH * w.capacity_ah;
    let hx = 1.0e-6 * w.capacity_ah / n as f64;
    let slope = |k: usize, xk: f64| {
        let rk = -(v_at(k, xk + hx) - v_at(k, xk - hx)) / (2.0 * hx);
        if rk.is_finite() && rk > R_FLOOR_OHMS {
            rk
        } else {
            R_FLOOR_OHMS
        }
    };
    x[..n].copy_from_slice(&start[..n]);
    let mut converged = tangent_passes(&v_at, &slope, n, i_p, tol, x);
    if !converged {
        // The tangent passes overshoot where a particle's potential bends hard — a surface
        // running into the clamp, an exchange current vanishing at an empty particle — and
        // from there can run to millions of amperes in opposite directions that still add
        // up to `i_p`. Start again from the same seed on the bracketed search, which cannot.
        x[..n].copy_from_slice(&start[..n]);
        converged = bracketed_split(&v_at, &slope, n, i_p, tol, x);
    }
    let head: f64 = x[..n - 1].iter().sum();
    x[n - 1] = i_p - head;
    let (mut sum_vr, mut sum_inv) = (0.0, 0.0);
    for (k, &xk) in x.iter().enumerate().take(n) {
        let rk = slope(k, xk);
        sum_vr += v_at(k, xk) / rk;
        sum_inv += 1.0 / rk;
    }
    let phi = sum_vr / sum_inv;
    Split {
        phi,
        converged: converged && phi.is_finite(),
    }
}

/// Most tangent passes [`solve_split`] tries before it hands the split to
/// [`bracketed_split`]. A seed from the last step's shares converges in two or three; a
/// pass that is still moving after this many is diverging, not converging slowly.
const TANGENT_PASS_CAP: usize = 8;

/// The fast half of [`solve_split`]: tangent passes from the shares in `x`, each one the
/// pack's closed-form group solve on the particles' tangents. `true` when a pass moved no
/// share by more than `tol`; `false` as soon as one moved further than the pass before it,
/// went non-finite, or the cap ran out — and then `x` is not to be trusted.
fn tangent_passes(
    v_at: &impl Fn(usize, f64) -> f64,
    slope: &impl Fn(usize, f64) -> f64,
    n: usize,
    i_p: f64,
    tol: f64,
    x: &mut [f64],
) -> bool {
    let mut v = [0.0_f64; MAX_PARTICLES];
    let mut r = [R_FLOOR_OHMS; MAX_PARTICLES];
    let mut last_moved = f64::INFINITY;
    for _ in 0..TANGENT_PASS_CAP {
        let (mut sum_vr, mut sum_inv, mut sum_x) = (0.0, 0.0, 0.0);
        for k in 0..n {
            v[k] = v_at(k, x[k]);
            r[k] = slope(k, x[k]);
            sum_vr += v[k] / r[k];
            sum_inv += 1.0 / r[k];
            sum_x += x[k];
        }
        let phi = (sum_vr + sum_x - i_p) / sum_inv;
        let mut moved = 0.0_f64;
        for k in 0..n {
            let dx = (v[k] - phi) / r[k];
            x[k] += dx;
            moved = moved.max(dx.abs());
        }
        if !moved.is_finite() || moved > last_moved {
            return false;
        }
        if moved <= tol {
            return true;
        }
        last_moved = moved;
    }
    false
}

/// The safe half of [`solve_split`]: the shared potential `φ` found by a bracketed search on
/// `Σ x_k(φ) = i_p`, each `x_k(φ)` itself found by a bracketed search on `v_k(x) = φ`.
///
/// Every particle's potential falls with its current — the kinetics' `asinh` alone
/// guarantees it falls without bound, so each inner search has a root — and so their sum
/// falls with the potential, and the outer search has exactly one. Both searches keep a
/// sign bracket the whole way, so neither can leave it. Slower than the tangent passes by
/// an order of magnitude, which is why it runs only where they fail.
///
/// Where a particle's potential does *not* fall with its current — the two-phase curve read
/// over a step long enough for the bulk's slope to beat the kinetics, past the sub-step
/// length this model uses — the inner search still returns a root of the bracket it found,
/// deterministically, and the step is what [`SUBSTEP_S`] is measured to keep away from.
fn bracketed_split(
    v_at: &impl Fn(usize, f64) -> f64,
    slope: &impl Fn(usize, f64) -> f64,
    n: usize,
    i_p: f64,
    tol: f64,
    x: &mut [f64],
) -> bool {
    // The potential the seed's tangents agree on: where the outer search starts.
    let (mut sum_vr, mut sum_inv, mut sum_x) = (0.0, 0.0, 0.0);
    let mut r = [R_FLOOR_OHMS; MAX_PARTICLES];
    for k in 0..n {
        r[k] = slope(k, x[k]);
        sum_vr += v_at(k, x[k]) / r[k];
        sum_inv += 1.0 / r[k];
        sum_x += x[k];
    }
    let phi0 = (sum_vr + sum_x - i_p) / sum_inv;
    if !phi0.is_finite() {
        return false;
    }
    let inner_tol = tol / n as f64;
    // `Σ x_k(φ) − i_p`, falling in `φ`. Each inner search starts from the share the last
    // evaluation found, so the outer search's later evaluations start almost on the root.
    let g = |phi: f64, x: &mut [f64]| {
        let mut sum = 0.0;
        for k in 0..n {
            let f = |xk: f64| v_at(k, xk) - phi;
            let step = ((v_at(k, x[k]) - phi) / r[k]).abs().max(inner_tol);
            x[k] = root_falling(f, x[k], step, inner_tol);
            sum += x[k];
        }
        sum - i_p
    };
    let mut lo = (phi0, g(phi0, x));
    if lo.1 == 0.0 {
        return true;
    }
    // Expand a bracket from `phi0`: `g` falls in `φ`, so a positive `g` needs a higher `φ`.
    let dir = if lo.1 > 0.0 { 1.0 } else { -1.0 };
    let mut step = 1.0e-3;
    let mut hi = lo;
    for _ in 0..64 {
        let phi = lo.0 + dir * step;
        let gv = g(phi, x);
        if !gv.is_finite() {
            return false;
        }
        hi = (phi, gv);
        if gv.signum() != lo.1.signum() || gv == 0.0 {
            break;
        }
        lo = hi;
        step *= 2.0;
    }
    if hi.1.signum() == lo.1.signum() && hi.1 != 0.0 {
        return false;
    }
    // Illinois on the bracket.
    let (mut a, mut b) = (lo, hi);
    let mut side = 0_i8;
    for _ in 0..200 {
        let phi = (a.0 * b.1 - b.0 * a.1) / (b.1 - a.1);
        let phi = if phi.is_finite() && (phi - a.0) * (phi - b.0) < 0.0 {
            phi
        } else {
            0.5 * (a.0 + b.0)
        };
        let gv = g(phi, x);
        if gv.abs() <= tol || phi == a.0 || phi == b.0 {
            return gv.is_finite();
        }
        if gv.signum() == a.1.signum() {
            a = (phi, gv);
            if side == -1 {
                b.1 *= 0.5;
            }
            side = -1;
        } else {
            b = (phi, gv);
            if side == 1 {
                a.1 *= 0.5;
            }
            side = 1;
        }
    }
    false
}

/// A root of `f`, a function that falls with its argument, near `x0`: a sign bracket grown
/// from `x0` in steps that start at `step` and double, then narrowed by Illinois' false
/// position until it is narrower than `tol`. Deterministic, and never leaves the bracket.
fn root_falling(f: impl Fn(f64) -> f64, x0: f64, step: f64, tol: f64) -> f64 {
    let f0 = f(x0);
    if f0 == 0.0 || !f0.is_finite() {
        return x0;
    }
    // A falling function above zero needs a larger argument.
    let dir = if f0 > 0.0 { 1.0 } else { -1.0 };
    let mut near = (x0, f0);
    let mut far = near;
    let mut d = step;
    for _ in 0..200 {
        let x = x0 + dir * d;
        let fx = f(x);
        far = (x, fx);
        if fx.signum() != f0.signum() || fx == 0.0 || !fx.is_finite() {
            break;
        }
        near = far;
        d *= 2.0;
    }
    if !far.1.is_finite() || (far.1.signum() == near.1.signum() && far.1 != 0.0) {
        return near.0;
    }
    let (mut a, mut b) = (near, far);
    let mut side = 0_i8;
    for _ in 0..200 {
        if (b.0 - a.0).abs() <= tol {
            break;
        }
        let x = (a.0 * b.1 - b.0 * a.1) / (b.1 - a.1);
        let x = if x.is_finite() && (x - a.0) * (x - b.0) < 0.0 {
            x
        } else {
            0.5 * (a.0 + b.0)
        };
        let fx = f(x);
        if fx == 0.0 {
            return x;
        }
        if fx.signum() == a.1.signum() {
            a = (x, fx);
            if side == -1 {
                b.1 *= 0.5;
            }
            side = -1;
        } else {
            b = (x, fx);
            if side == 1 {
                a.1 *= 0.5;
            }
            side = 1;
        }
    }
    if a.1.abs() <= b.1.abs() {
        a.0
    } else {
        b.0
    }
}

/// The positive electrode through a step of `m` sub-steps of `h` seconds at particle current
/// `i_p` and cell current `i`. `first` holds each particle's [`OuterShell`] for the first
/// sub-step, taken from `c_pos` as it is on entry. Every sub-step but the last diffuses
/// `c_pos` in place under the shares its split found; the last does too when `commit`.
/// Writes the last sub-step's shares to `x`.
///
/// The probe and the advance both come through here with the same inputs, so the curve the
/// pack converges on and the step the cell then takes are the same arithmetic.
#[allow(clippy::too_many_arguments)]
fn march_positive(
    w: &Working<'_>,
    pp: &Particles,
    c_pos: &mut [f64],
    first: &[OuterShell],
    (i_p, i): (f64, f64),
    (h, m): (f64, usize),
    seed: &[f64],
    x: &mut [f64],
    commit: bool,
) -> Split {
    let n = pp.n;
    let sh = pp.shells;
    let mut outers = [stored_shell(0.0); MAX_PARTICLES];
    outers[..n].copy_from_slice(&first[..n]);
    let mut seeds = [0.0_f64; MAX_PARTICLES];
    seeds[..n].copy_from_slice(&seed[..n]);
    let mut out = Split {
        phi: 0.0,
        converged: true,
    };
    for sub in 0..m {
        if sub > 0 {
            for k in 0..n {
                outers[k] = sweep(&c_pos[k * sh..(k + 1) * sh], pp.radius_m[k], w.pos.d_s, h);
            }
        }
        let split = solve_split(w, pp, &outers[..n], i_p, i, &seeds[..n], x);
        out = Split {
            phi: split.phi,
            converged: out.converged && split.converged,
        };
        if sub + 1 < m || commit {
            for k in 0..n {
                let j = pp.j(w, k, x[k]);
                diffuse(
                    &mut c_pos[k * sh..(k + 1) * sh],
                    pp.radius_m[k],
                    w.pos.d_s,
                    j,
                    h,
                );
            }
        }
        seeds[..n].copy_from_slice(&x[..n]);
    }
    out
}

/// The negative particle's outer shell \[mol/m³\] at the end of a step of `m` sub-steps of
/// `h` seconds under flux `j`, from `first` — its [`OuterShell`] for the first sub-step. One
/// sub-step reads `first` directly, which is [`crate::spm::probe_at`]'s arithmetic.
fn neg_outer_end(
    w: &Working<'_>,
    c_neg: &[f64],
    first: OuterShell,
    j: f64,
    (h, m): (f64, usize),
) -> f64 {
    if m == 1 {
        return first.at(j);
    }
    let mut row = [0.0_f64; MAX_SHELLS];
    let row = &mut row[..c_neg.len()];
    row.copy_from_slice(c_neg);
    let r = w.neg.p.particle_radius_m;
    for _ in 0..m - 1 {
        diffuse(row, r, w.neg.d_s, j, h);
    }
    sweep(row, r, w.neg.d_s, h).at(j)
}

/// Every particle's [`OuterShell`] for the first sub-step of a step whose sub-steps are `h`
/// seconds long: one sweep each, independent of the current, so a probe takes them once.
struct Ends {
    neg: OuterShell,
    pos: [OuterShell; MAX_PARTICLES],
}

impl Ends {
    fn of(w: &Working<'_>, pp: &Particles, s: &EnsembleState, h: f64) -> Self {
        let mut pos = [stored_shell(0.0); MAX_PARTICLES];
        for (k, shell) in pos.iter_mut().enumerate().take(pp.n) {
            *shell = sweep(pp.profile(&s.c_pos, k), pp.radius_m[k], w.pos.d_s, h);
        }
        Self {
            neg: sweep(&s.c_neg, w.neg.p.particle_radius_m, w.neg.d_s, h),
            pos,
        }
    }

    /// The identity shells of a read where no time passes.
    fn stored(pp: &Particles, s: &EnsembleState) -> Self {
        let mut pos = [stored_shell(0.0); MAX_PARTICLES];
        for (k, shell) in pos.iter_mut().enumerate().take(pp.n) {
            let c = pp.profile(&s.c_pos, k);
            *shell = stored_shell(c[c.len() - 1]);
        }
        Self {
            neg: stored_shell(s.c_neg[s.c_neg.len() - 1]),
            pos,
        }
    }
}

/// The cell's equilibrium voltage \[V\] at its particles' **bulk** stoichiometries: the
/// positive particles' potentials averaged — every particle holds the same volume — minus
/// the negative particle's. One particle's average is its own potential, exactly.
#[must_use]
fn equilibrium_voltage(w: &Working<'_>, pp: &Particles, s: &EnsembleState) -> f64 {
    let x = mean_concentration(&s.c_neg) / w.neg.p.c_max_mol_per_m3;
    let mut sum = 0.0;
    for k in 0..pp.n {
        let y = mean_concentration(pp.profile(&s.c_pos, k)) / w.pos.p.c_max_mol_per_m3;
        sum += positive_ocp(w.pos.p, w.temp_k, y);
    }
    sum / pp.n as f64 - ocp_lookup(&w.neg.p.ocp, x)
}

/// Heat \[W\] the particles make trading lithium among themselves: `Σ x_k·(U_k − Ū)`, each
/// particle's share times how far its own bulk potential sits from the electrode's average.
///
/// `i·(U_eq − V)` books a single particle's whole irreversible heat; with several it misses
/// this, because the shares do not each equal `i`. At rest it is all there is: the cell
/// carries nothing, the particles still exchange lithium, and that exchange is the
/// hysteresis relaxing. Exactly `0.0` for one particle.
#[must_use]
fn exchange_w(w: &Working<'_>, pp: &Particles, s: &EnsembleState, x: &[f64]) -> f64 {
    if pp.n == 1 {
        return 0.0;
    }
    let mut u = [0.0_f64; MAX_PARTICLES];
    let mut mean = 0.0;
    for (k, uk) in u.iter_mut().enumerate().take(pp.n) {
        let y = mean_concentration(pp.profile(&s.c_pos, k)) / w.pos.p.c_max_mol_per_m3;
        *uk = positive_ocp(w.pos.p, w.temp_k, y);
        mean += *uk;
    }
    mean /= pp.n as f64;
    let mut q = 0.0;
    for k in 0..pp.n {
        q += x[k] * (u[k] - mean);
    }
    q
}

/// Reversible heat \[W\] of the regular-solution potential: `−T·Σ x_k·∂U_k/∂T`, each
/// particle's share times its own entropy coefficient at its bulk stoichiometry.
///
/// The regular-solution form depends on temperature through its mixing term alone, so
/// `∂U/∂T = −(R/F)·ln(y/(1 − y))`: lithium entering a nearly empty particle releases heat
/// and lithium entering a nearly full one absorbs it, and a particle at half-filling does
/// neither. That is not the chemistry's scalar `docp_dt_v_per_k`, which validation refuses
/// beside this form so the two cannot be counted twice. Like [`exchange_w`] it runs at rest
/// too — the particles trading lithium carry entropy with it — which is why it travels in
/// the watts the pack adds rather than as a voltage times `i`.
///
/// `0.0` on a table potential, whose temperature dependence is the scalar's alone.
#[must_use]
fn reversible_w(w: &Working<'_>, pp: &Particles, s: &EnsembleState, x: &[f64]) -> f64 {
    if w.pos.p.regular_solution.is_none() {
        return 0.0;
    }
    let mut sum = 0.0;
    for (k, &xk) in x.iter().enumerate().take(pp.n) {
        let y = (mean_concentration(pp.profile(&s.c_pos, k)) / w.pos.p.c_max_mol_per_m3)
            .clamp(SURFACE_EDGE, 1.0 - SURFACE_EDGE);
        sum += xk * math::ln(y / (1.0 - y));
    }
    w.temp_k * GAS_CONSTANT_J_PER_MOL_K / FARADAY_C_PER_MOL * sum
}

/// The cell's equilibrium voltage at the chemistry's declared empty — [`spm::empty_voltage`]
/// through [`positive_ocp`].
#[must_use]
fn empty_voltage(w: &Working<'_>) -> f64 {
    positive_ocp(w.pos.p, w.temp_k, w.pos.p.stoich_max)
        - ocp_lookup(&w.neg.p.ocp, w.neg.p.stoich_min)
}

/// [`spm::reversal_drop`] from [`empty_voltage`].
#[must_use]
fn reversal_drop(w: &Working<'_>, rev: &ReversalParams, d: f64) -> f64 {
    if d > 0.0 {
        (rev.v_per_soc * d)
            .min(empty_voltage(w) - rev.floor_v)
            .max(0.0)
    } else {
        0.0
    }
}

/// [`spm::reversal_ocv`] from [`empty_voltage`].
#[must_use]
fn reversal_ocv(w: &Working<'_>, rev: &ReversalParams, d: f64) -> f64 {
    empty_voltage(w) - reversal_drop(w, rev, d)
}

/// The particles' terminal voltage \[V\] — before any reversal drop — read off the state as
/// stored, no time passing: the current `i_p` divided among the positive particles at their
/// stored surfaces, and the cell current `i` through the kinetics and the contact. The
/// shares are written to `x`.
///
/// [`spm`]'s `voltage` for one particle.
fn read_now(
    w: &Working<'_>,
    pp: &Particles,
    s: &EnsembleState,
    i_p: f64,
    i: f64,
    x: &mut [f64],
) -> (f64, bool) {
    let ends = Ends::stored(pp, s);
    let split = solve_split(w, pp, &ends.pos[..pp.n], i_p, i, &s.i_particles_last, x);
    let n = spm::half(
        w,
        s.c_neg[s.c_neg.len() - 1],
        pp.shells,
        &w.neg,
        w.j_neg(i_p),
        i / w.neg.g.area_m2,
    );
    (split.phi - n - i * w.r_contact, split.converged)
}

/// One surface's current interval `(lo, hi)` \[A\] over which `c_s = A − B·g·i` stays strictly
/// inside the band [`clamp_surface`] passes untouched: [`spm`]'s `current_window` side, on
/// `(A, B)` handed in.
fn surface_interval(a: f64, b: f64, c_max: f64, g: f64) -> (f64, f64) {
    let bg = b * g;
    let at_edge = |edge: f64| (a - edge) / bg;
    let (x, y) = (
        at_edge((1.0 - SURFACE_EDGE) * c_max),
        at_edge(SURFACE_EDGE * c_max),
    );
    (x.min(y), x.max(y))
}

/// `(A, B)` of a particle's end-of-step surface `c_s(j) = A − B·j`, from its first-sub-step
/// [`OuterShell`] at one sub-step — [`spm`]'s `current_window` arithmetic — and by marching
/// the profile at two fluxes otherwise.
fn neg_surface_affine(
    w: &Working<'_>,
    c_neg: &[f64],
    first: OuterShell,
    (h, m): (f64, usize),
) -> (f64, f64) {
    let shells = c_neg.len();
    let dr = w.neg.p.particle_radius_m / shells as f64;
    let extrapolate = 0.5 * dr / w.neg.d_s;
    if m == 1 {
        return (
            (first.t - first.m) / first.d,
            first.k / first.d + extrapolate,
        );
    }
    let a0 = neg_outer_end(w, c_neg, first, 0.0, (h, m));
    let a1 = neg_outer_end(w, c_neg, first, 1.0, (h, m));
    (a0, -(a1 - a0) + extrapolate)
}

/// The range of cell current \[A, discharge-positive\] over which the surfaces stay inside
/// the band [`clamp_surface`] leaves alone at the end of the step — the negative particle's,
/// and, for **one** positive particle, its surface too, exactly as [`crate::spm`]'s
/// `current_window`.
///
/// With several positive particles their surfaces are left out. Each one's surface depends
/// on its own share, which the split decides, so there is no interval per particle in
/// closed form; and the even split that would give one is wrong exactly when it matters,
/// because a full particle refuses the current an even split would hand it. So the
/// positive electrode's range is its bulk alone — the `Dfn`'s rule — and an out-of-range
/// particle surface is flagged after the step ([`EventFlags::SURFACE_OUT_OF_RANGE`]).
fn surface_window(
    w: &Working<'_>,
    pp: &Particles,
    s: &EnsembleState,
    ends: &Ends,
    hm: (f64, usize),
) -> Option<(f64, f64)> {
    let (a, b) = neg_surface_affine(w, &s.c_neg, ends.neg, hm);
    let (n_lo, n_hi) = surface_interval(a, b, w.neg.p.c_max_mol_per_m3, w.j_neg(1.0));
    let (lo, hi) = if pp.n == 1 {
        let o = ends.pos[0];
        let dr = pp.radius_m[0] / pp.shells as f64;
        let extrapolate = 0.5 * dr / w.pos.d_s;
        let (p_lo, p_hi) = surface_interval(
            (o.t - o.m) / o.d,
            o.k / o.d + extrapolate,
            w.pos.p.c_max_mol_per_m3,
            w.j_pos(1.0),
        );
        (n_lo.max(p_lo), n_hi.min(p_hi))
    } else {
        (n_lo, n_hi)
    };
    (lo.is_finite() && hi.is_finite() && lo < hi).then_some((lo, hi))
}

/// The range of cell current \[A\] over which a step of `dt > 0` leaves both electrodes'
/// **bulk** between the chemistry's empty and full: [`spm`]'s `bulk_window`, with the
/// positive electrode's bulk the mean over its particles. Conservation alone, whatever the
/// split: the electrode gains exactly the lithium its current carries.
fn bulk_window(w: &Working<'_>, pp: &Particles, s: &EnsembleState, dt: f64) -> (f64, f64) {
    let interval = |now: f64, per_amp: f64, e: &ElectrodeParams| {
        let c_max = e.c_max_mol_per_m3;
        let at_edge = |stoich: f64| (now - stoich * c_max) / per_amp;
        let (x, y) = (at_edge(e.stoich_min), at_edge(e.stoich_max));
        (x.min(y), x.max(y))
    };
    let (n_lo, n_hi) = interval(
        mean_concentration(&s.c_neg),
        3.0 * dt * w.j_neg(1.0) / w.neg.p.particle_radius_m,
        w.neg.p,
    );
    let (now, per_amp) = if pp.n == 1 {
        (
            mean_concentration(pp.profile(&s.c_pos, 0)),
            3.0 * dt * w.j_pos(1.0) / pp.radius_m[0],
        )
    } else {
        let mut sum = 0.0;
        for k in 0..pp.n {
            sum += mean_concentration(pp.profile(&s.c_pos, k));
        }
        (
            sum / pp.n as f64,
            -dt * w.kappa / (w.pos.g.volume_m3 * FARADAY_C_PER_MOL),
        )
    };
    let (p_lo, p_hi) = interval(now, per_amp, w.pos.p);
    (n_lo.max(p_lo), n_hi.min(p_hi))
}

/// The most discharge current \[A\] the particles can carry over a step of `dt > 0`; past it
/// the reversal carries the rest. [`spm`]'s `discharge_edge`.
fn discharge_edge(
    w: &Working<'_>,
    pp: &Particles,
    s: &EnsembleState,
    ends: &Ends,
    hm: (f64, usize),
    dt: f64,
) -> f64 {
    let bulk = bulk_window(w, pp, s, dt).1;
    match surface_window(w, pp, s, ends, hm) {
        Some((_, surf)) => surf.min(bulk),
        None => bulk,
    }
}

/// Everything a probe over one step needs that does not depend on the current tried.
struct StepFrame {
    ends: Ends,
    h: f64,
    m: usize,
    edge: f64,
}

impl StepFrame {
    fn of(w: &Working<'_>, pp: &Particles, s: &EnsembleState, dt: f64) -> Self {
        let m = substeps(pp.n, dt);
        let h = dt / m as f64;
        let ends = Ends::of(w, pp, s, h);
        let edge = discharge_edge(w, pp, s, &ends, (h, m), dt);
        Self { ends, h, m, edge }
    }
}

/// The terminal voltage \[V\] at the end of a step of `dt > 0` carrying `i` throughout,
/// marched on a copy of the state: the many-particle [`crate::spm::probe_at`] curve.
fn curve_end(
    w: &Working<'_>,
    rev: &ReversalParams,
    pp: &Particles,
    s: &EnsembleState,
    f: &StepFrame,
    i: f64,
    dt: f64,
) -> f64 {
    let (i_p, d) = spm::split_from(w, s.soc_deficit, f.edge, i, dt);
    let mut x = [0.0_f64; MAX_PARTICLES];
    let phi = if f.m == 1 {
        solve_split(
            w,
            pp,
            &f.ends.pos[..pp.n],
            i_p,
            i,
            &s.i_particles_last,
            &mut x,
        )
        .phi
    } else {
        let mut c = [0.0_f64; MAX_PARTICLES * MAX_SHELLS];
        let c = &mut c[..s.c_pos.len()];
        c.copy_from_slice(&s.c_pos);
        march_positive(
            w,
            pp,
            c,
            &f.ends.pos,
            (i_p, i),
            (f.h, f.m),
            &s.i_particles_last,
            &mut x,
            false,
        )
        .phi
    };
    let jn = w.j_neg(i_p);
    let n_outer = neg_outer_end(w, &s.c_neg, f.ends.neg, jn, (f.h, f.m));
    let n = spm::half(w, n_outer, pp.shells, &w.neg, jn, i / w.neg.g.area_m2);
    phi - n - i * w.r_contact - reversal_drop(w, rev, d)
}

/// The terminal voltage \[V\] read where no time passes, at cell current `i`: the current the
/// last step left the reversal carrying is held, the particles take the rest. [`crate::spm`]'s
/// stored-state read.
fn curve_now(
    w: &Working<'_>,
    rev: &ReversalParams,
    pp: &Particles,
    s: &EnsembleState,
    i: f64,
) -> f64 {
    let mut x = [0.0_f64; MAX_PARTICLES];
    read_now(w, pp, s, i - s.i_reversal_last, i, &mut x).0 - reversal_drop(w, rev, s.soc_deficit)
}

/// Ground-truth state of charge, in \[0, 1\]: the negative particle's, exactly as
/// [`crate::spm`]'s — the negative electrode is one particle here too.
#[must_use]
pub(crate) fn soc(s: &EnsembleState, spm: &SpmParams) -> f64 {
    raw_soc(s, spm).clamp(0.0, 1.0)
}

fn raw_soc(s: &EnsembleState, spm: &SpmParams) -> f64 {
    let e = &spm.negative;
    window_fraction_neg(mean_concentration(&s.c_neg) / e.c_max_mol_per_m3, e)
}

/// Total overpotential \[V\], discharge-positive, at [`EnsembleState::i_last`]: equilibrium
/// voltage minus the terminal voltage, less the ohmic drop. [`crate::spm`]'s.
#[must_use]
pub(crate) fn overpotential_v(
    s: &EnsembleState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> f64 {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let pp = Particles::of(&w, s);
    let i = s.i_last;
    let mut x = [0.0_f64; MAX_PARTICLES];
    equilibrium_voltage(&w, &pp, s)
        - read_now(&w, &pp, s, i - s.i_reversal_last, i, &mut x).0
        - i * w.r_contact
}

/// Bulk minus surface stoichiometry on each electrode, `(negative, positive)`, on the window
/// [`soc`] uses, discharge-positive: [`crate::spm`]'s `surface_gap`, with the positive
/// electrode's the mean over its particles of each one's gap, each along the current its
/// kinetics last carried.
#[must_use]
pub(crate) fn surface_gap(s: &EnsembleState, spm: &SpmParams, eff_capacity_ah: f64) -> (f64, f64) {
    let w = Working::new(spm, s.temp_k, 1.0, eff_capacity_ah);
    let pp = Particles::of(&w, s);
    let n = &spm.negative;
    let p = &spm.positive;
    let cs_n = spm::c_surface(&s.c_neg, n.particle_radius_m, w.neg.d_s, w.j_neg(s.i_last));
    let i_p: f64 = s.i_particles_last.iter().sum();
    let mut gap_p = 0.0;
    for k in 0..pp.n {
        let kin = s.i_last * pp.weight[k] - (i_p * pp.weight[k] - s.i_particles_last[k]);
        let c = pp.profile(&s.c_pos, k);
        let cs = spm::c_surface(c, pp.radius_m[k], w.pos.d_s, pp.j(&w, k, kin));
        gap_p += window_fraction_pos(mean_concentration(c) / p.c_max_mol_per_m3, p)
            - window_fraction_pos(cs / p.c_max_mol_per_m3, p);
    }
    (
        window_fraction_neg(mean_concentration(&s.c_neg) / n.c_max_mol_per_m3, n)
            - window_fraction_neg(cs_n / n.c_max_mol_per_m3, n),
        gap_p / pp.n as f64,
    )
}

/// Each positive particle's mean stoichiometry, the fraction of its sites holding lithium, in
/// the order the particles are stored: `0` empty, `1` full.
///
/// The raw stoichiometry, not [`soc`]'s window: what a particle's state *is* on the
/// regular-solution curve, so a client can count which side of the spinodal each sits on.
/// The midpoint `0.5` is the curve's centre for any `Ω`, which is why a count of particles
/// above it is the number the cold rest gap is set by.
#[must_use]
pub(crate) fn positive_stoichiometry(s: &EnsembleState, spm: &SpmParams) -> Vec<f64> {
    let shells = s.c_pos.len() / s.radii_m.len();
    s.c_pos
        .chunks(shells)
        .map(|c| mean_concentration(c) / spm.positive.c_max_mol_per_m3)
        .collect()
}

/// Heat \[W\] estimate at current `i` given the node voltage `v_terminal`: `i·(U_eq − V)` plus
/// the entropic term, [`crate::spm`]'s `heat_w`. The particles' exchange and the end-of-step
/// corrections arrive from [`advance`].
#[must_use]
pub(crate) fn heat_w(
    s: &EnsembleState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
    v_terminal: f64,
) -> f64 {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let pp = Particles::of(&w, s);
    let q_irrev = i * (equilibrium_voltage(&w, &pp, s) - v_terminal);
    let docv_dt = spm.positive.docp_dt_v_per_k - spm.negative.docp_dt_v_per_k;
    let q_rev = -i * s.temp_k * docv_dt;
    q_irrev + q_rev
}

/// This cell's tangent `(E, R)` at [`EnsembleState::i_last`] on the start-of-step curve: a
/// pure function of state, which is what lets the pack memoise it. [`crate::spm::source`].
#[must_use]
pub(crate) fn source(
    s: &EnsembleState,
    spm: &SpmParams,
    rev: &ReversalParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> (f64, f64) {
    probe_at(
        s,
        spm,
        rev,
        eff_r0_factor,
        eff_capacity_ah,
        s.i_last,
        0.0,
        false,
    )
    .1
}

/// Where the end-of-step curve is at `i` over a step of `dt` seconds, and the line touching
/// it there: `(V(i), (E, R))`. [`crate::spm::probe_at`], with the positive electrode marched
/// through its sub-steps and split among its particles at every one; `hold` keeps the probe
/// inside [`surface_window`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn probe_at(
    s: &EnsembleState,
    spm: &SpmParams,
    rev: &ReversalParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
    dt: f64,
    hold: bool,
) -> (f64, (f64, f64)) {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let pp = Particles::of(&w, s);
    let frame = (!(dt.is_nan() || dt <= 0.0)).then(|| StepFrame::of(&w, &pp, s, dt));
    let curve = |i: f64| match &frame {
        Some(f) => curve_end(&w, rev, &pp, s, f, i, dt),
        None => curve_now(&w, rev, &pp, s, i),
    };
    let h = 1.0e-6 * eff_capacity_ah;
    let i = match &frame {
        Some(f) if hold => match surface_window(&w, &pp, s, &f.ends, (f.h, f.m)) {
            Some(range) => spm::held(i, range, h),
            None => i,
        },
        _ => i,
    };
    let r = -(curve(i + h) - curve(i - h)) / (2.0 * h);
    let r = if r.is_finite() && r > R_FLOOR_OHMS {
        r
    } else {
        R_FLOOR_OHMS
    };
    let v = curve(i);
    (v, (v + i * r, r))
}

/// The range of cell current \[A\] the pack holds a voltage or power demand to over a step of
/// `dt`: [`surface_window`] intersected with [`bulk_window`], pulled in by the tangent's
/// difference step. [`crate::spm::step_current_window`].
#[must_use]
pub(crate) fn step_current_window(
    s: &EnsembleState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    dt: f64,
) -> Option<(f64, f64)> {
    if dt.is_nan() || dt <= 0.0 {
        return None;
    }
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let pp = Particles::of(&w, s);
    let m = substeps(pp.n, dt);
    let h = dt / m as f64;
    let ends = Ends::of(&w, &pp, s, h);
    let (s_lo, s_hi) = surface_window(&w, &pp, s, &ends, (h, m))?;
    let (b_lo, b_hi) = bulk_window(&w, &pp, s, dt);
    let (lo, hi) = (s_lo.max(b_lo), s_hi.min(b_hi));
    if !(lo.is_finite() && hi.is_finite() && lo < hi) {
        return None;
    }
    let hh = 1.0e-6 * eff_capacity_ah;
    Some(if hi - lo > 2.0 * hh {
        (lo + hh, hi - hh)
    } else {
        let mid = 0.5 * (lo + hi);
        (mid, mid)
    })
}

/// The tangent a pack's first pass aggregates this cell from on a step of `dt`: the
/// end-of-step curve's at the last current, pulled inside [`surface_window`] when the cell
/// could rest through the step inside it. [`crate::spm::first_pass_tangent`].
#[must_use]
pub(crate) fn first_pass_tangent(
    s: &EnsembleState,
    spm: &SpmParams,
    rev: &ReversalParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    dt: f64,
) -> (f64, f64) {
    let mut i = s.i_last;
    if !dt.is_nan() && dt > 0.0 {
        let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
        let pp = Particles::of(&w, s);
        let m = substeps(pp.n, dt);
        let h = dt / m as f64;
        let ends = Ends::of(&w, &pp, s, h);
        if let Some(range) =
            surface_window(&w, &pp, s, &ends, (h, m)).filter(|&(lo, hi)| lo < 0.0 && 0.0 < hi)
        {
            i = spm::held(i, range, 1.0e-6 * eff_capacity_ah);
        }
    }
    probe_at(s, spm, rev, eff_r0_factor, eff_capacity_ah, i, dt, false).1
}

/// Advance the cell by `dt` seconds under the current `i` the pack assigned it. Returns the
/// flags and the heat terms on [`crate::spm::advance`]'s contract — with the particles'
/// exchange heat ([`exchange_w`]) added to the watts it hands back, which is what the
/// pack adds without multiplying by `i`.
///
/// Flags, beyond the single-particle model's: [`EventFlags::SOLVE_UNCONVERGED`] when a
/// sub-step's split did not converge (its shares still add up to the current, so no charge
/// is lost, but the particles' potentials disagree by more than the tolerance), and
/// [`EventFlags::SURFACE_OUT_OF_RANGE`] for any particle's surface outside `[0, c_max]`.
///
/// The last value is the terminal voltage \[V\] at the end of the step, read off the state
/// the step produced. The pack does not read it — it has its own node — and it is returned
/// so a test can hold it against [`probe_at`]'s curve at the same current: they are the
/// same arithmetic, so they are the same bits.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance(
    s: &mut EnsembleState,
    spm: &SpmParams,
    rev: &ReversalParams,
    i: f64,
    dt: f64,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    v_node: f64,
) -> (EventFlags, f64, f64, (f64, f64), f64) {
    let moves = !dt.is_nan() && dt > 0.0;
    let wk = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let pp = Particles::of(&wk, s);
    let frame = moves.then(|| StepFrame::of(&wk, &pp, s, dt));
    let (i_p, d_end) = match &frame {
        Some(f) => spm::split_from(&wk, s.soc_deficit, f.edge, i, dt),
        None => (i, s.soc_deficit),
    };
    let d_start = s.soc_deficit;
    let mut x_start = [0.0_f64; MAX_PARTICLES];
    let start = if moves {
        let u = equilibrium_voltage(&wk, &pp, s);
        let v = read_now(&wk, &pp, s, i_p, i, &mut x_start).0 - reversal_drop(&wk, rev, d_start);
        (
            u,
            v,
            exchange_w(&wk, &pp, s, &x_start) + reversible_w(&wk, &pp, s, &x_start),
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let mut x = [0.0_f64; MAX_PARTICLES];
    let mut phi_end = 0.0;
    let mut split_ok = true;
    if let Some(f) = &frame {
        // A stack copy: `march_positive` diffuses `c_pos` in place while it reads the seeds,
        // and `Pack::step` allocates nothing.
        let mut seed = [0.0_f64; MAX_PARTICLES];
        seed[..pp.n].copy_from_slice(&s.i_particles_last);
        let split = march_positive(
            &wk,
            &pp,
            &mut s.c_pos,
            &f.ends.pos,
            (i_p, i),
            (f.h, f.m),
            &seed,
            &mut x,
            true,
        );
        phi_end = split.phi;
        split_ok = split.converged;
        let jn = wk.j_neg(i_p);
        for _ in 0..f.m {
            diffuse(
                &mut s.c_neg,
                spm.negative.particle_radius_m,
                wk.neg.d_s,
                jn,
                f.h,
            );
        }
    }
    if dt > 0.0 {
        s.i_last = i;
        s.i_reversal_last = i - i_p;
        s.i_particles_last.copy_from_slice(&x[..pp.n]);
    }
    if moves {
        s.soc_deficit = d_end;
    }
    let raw = raw_soc(s, spm);
    let mut flags = EventFlags::empty();
    if raw > 1.0 {
        flags |= EventFlags::SOC_CLAMPED_HIGH;
    } else if raw < 0.0 || s.soc_deficit > 0.0 {
        flags |= EventFlags::SOC_CLAMPED_LOW;
    }
    if !moves {
        return (flags, 0.0, 0.0, (0.0, 0.0), f64::NAN);
    }
    if !split_ok {
        flags |= EventFlags::SOLVE_UNCONVERGED;
    }
    if !surfaces_in_range(&wk, &pp, s, i_p, &x) {
        flags |= EventFlags::SURFACE_OUT_OF_RANGE;
    }
    let (u_start, v_start, ex_start) = start;
    let u_end = equilibrium_voltage(&wk, &pp, s);
    let n_half = spm::half(
        &wk,
        s.c_neg[s.c_neg.len() - 1],
        pp.shells,
        &wk.neg,
        wk.j_neg(i_p),
        i / wk.neg.g.area_m2,
    );
    let v_end = phi_end - n_half - i * wk.r_contact - reversal_drop(&wk, rev, s.soc_deficit);
    let i_d = i - i_p;
    let rev_at = |u: f64, d: f64| {
        if i_d == 0.0 {
            0.0
        } else {
            i_d * (reversal_ocv(&wk, rev, d) - u)
        }
    };
    let (mut w_end, mut w_mean) = {
        let (rev_start, rev_end) = (rev_at(u_start, d_start), rev_at(u_end, s.soc_deficit));
        (rev_end, 0.5 * (rev_start + rev_end))
    };
    // Exactly `0.0` from both for one particle on a table potential, and added only where
    // either can be non-zero, so that cell's heat is the single-particle model's bit for bit.
    if pp.n > 1 || wk.pos.p.regular_solution.is_some() {
        let ex_end = exchange_w(&wk, &pp, s, &x) + reversible_w(&wk, &pp, s, &x);
        w_end += ex_end;
        w_mean += 0.5 * (ex_start + ex_end);
    }
    let estimate = u_start - v_node;
    let at_end = u_end - v_end;
    (
        flags,
        at_end - estimate,
        0.5 * ((u_start - v_start) + at_end) - estimate,
        (w_end, w_mean),
        v_end,
    )
}

/// Whether every surface lies inside `[0, c_max]` along the flux its share carried: the
/// negative particle's along `i_p`, each positive particle's along its own share `x_k`.
/// [`crate::spm`]'s `surfaces_in_range`, per particle.
fn surfaces_in_range(
    w: &Working<'_>,
    pp: &Particles,
    s: &EnsembleState,
    i_p: f64,
    x: &[f64],
) -> bool {
    let in_band = |c: &[f64], r: f64, d_s: f64, c_max: f64, j: f64| {
        let c_s = surface_from_outer(c[c.len() - 1], c.len(), r, d_s, j);
        (0.0..=c_max).contains(&c_s)
    };
    if !in_band(
        &s.c_neg,
        w.neg.p.particle_radius_m,
        w.neg.d_s,
        w.neg.p.c_max_mol_per_m3,
        w.j_neg(i_p),
    ) {
        return false;
    }
    (0..pp.n).all(|k| {
        in_band(
            pp.profile(&s.c_pos, k),
            pp.radius_m[k],
            w.pos.d_s,
            w.pos.p.c_max_mol_per_m3,
            pp.j(w, k, x[k]),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chem::{OcpTable, RegularSolutionParams};

    /// The slice-B LFP fixture of `sim-data/tests/ensemble.rs`, with a coarser graphite
    /// table: Prada2013's graphite potential sampled at sixteen points.
    fn lfp() -> SpmParams {
        SpmParams {
            t_ref_k: 298.15,
            c_e_mol_per_m3: 1200.0,
            electrode_area_m2: 0.18,
            contact_resistance_ohm: 0.0,
            negative: ElectrodeParams {
                particle_radius_m: 5e-6,
                diffusivity_m2_per_s: 3e-15,
                c_max_mol_per_m3: 30555.0,
                active_volume_fraction: 0.58,
                thickness_m: 3.4e-5,
                m_ref: 6.48e-7,
                reaction_ea_j_per_mol: 35000.0,
                diffusivity_ea_j_per_mol: 0.0,
                charge_transfer_alpha: 0.5,
                stoich_min: 0.017_617_931_791_027_094,
                stoich_max: 0.81,
                docp_dt_v_per_k: 0.0,
                ocp: OcpTable {
                    stoich: vec![
                        0.0, 0.01, 0.02, 0.04, 0.07, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8,
                        0.9, 1.0,
                    ],
                    volts: vec![
                        2.383542, 1.739382, 1.304718, 0.812987, 0.522842, 0.406516, 0.25683,
                        0.216986, 0.162973, 0.13524, 0.133086, 0.118751, 0.092194, 0.09202,
                        0.09202, 0.09202,
                    ],
                },
                regular_solution: None,
            },
            positive: ElectrodeParams {
                particle_radius_m: 5e-8,
                diffusivity_m2_per_s: 5.9e-18,
                c_max_mol_per_m3: 22806.0,
                active_volume_fraction: 0.374,
                thickness_m: 8e-5,
                m_ref: 6e-7,
                reaction_ea_j_per_mol: 39570.0,
                diffusivity_ea_j_per_mol: 0.0,
                charge_transfer_alpha: 0.5,
                stoich_min: 0.0038,
                stoich_max: 0.703_502_020_929_131_3,
                docp_dt_v_per_k: 0.0,
                ocp: OcpTable {
                    stoich: vec![0.0, 1.0],
                    volts: vec![3.6, 3.3],
                },
                regular_solution: Some(RegularSolutionParams {
                    u0_v: 3.42,
                    omega_ev: 0.075_917_388_974_677_12,
                }),
            },
        }
    }

    fn rev() -> ReversalParams {
        ReversalParams {
            v_per_soc: 100.0,
            floor_v: 0.0,
            fade_per_ah: 0.0,
        }
    }

    const CAP_AH: f64 = 2.303_324_557_209_257;

    /// Twenty particles of spread radii, driven from full into the two-phase plateau so
    /// that they have separated: the state the tests below start from.
    fn separated() -> EnsembleState {
        let spm = lfp();
        let mut s = EnsembleState::new(&spm, 20, 20, 1.0, 298.15);
        for (k, r) in s.radii_m.iter_mut().enumerate() {
            *r = 5e-8 * (0.7 + 0.03 * k as f64);
        }
        for _ in 0..1800 {
            let v = probe_at(&s, &spm, &rev(), 1.0, CAP_AH, CAP_AH, 1.0, false).0;
            let _ = advance(&mut s, &spm, &rev(), CAP_AH, 1.0, 1.0, CAP_AH, v);
        }
        s
    }

    /// The curve the pack converges on is the step the cell then takes: [`probe_at`]'s
    /// voltage at a current and the voltage [`advance`] leaves behind at that current are
    /// the same bits, at one sub-step and at many, charging, discharging and at rest.
    #[test]
    fn the_probe_and_the_step_agree_bit_for_bit() {
        let spm = lfp();
        let s0 = separated();
        for dt in [1.0, 5.0, 37.5, 300.0] {
            for i in [-2.0 * CAP_AH, 0.0, 0.3 * CAP_AH, 2.0 * CAP_AH] {
                let probed = probe_at(&s0, &spm, &rev(), 1.0, CAP_AH, i, dt, false).0;
                let mut s = s0.clone();
                let (flags, .., v_end) = advance(&mut s, &spm, &rev(), i, dt, 1.0, CAP_AH, probed);
                assert!(!flags.contains(EventFlags::SOLVE_UNCONVERGED));
                assert_eq!(
                    probed.to_bits(),
                    v_end.to_bits(),
                    "dt {dt}, i {i}: probed {probed}, stepped {v_end}"
                );
            }
        }
    }

    /// Every particle sits at the shared potential to well under what the pack's difference
    /// quotient can see (`R·h`, tens of nanovolts here), and the bracketed half of the split
    /// lands on the same shares as the tangent passes.
    #[test]
    fn the_particles_agree_on_their_potential() {
        let spm = lfp();
        let s = separated();
        let w = Working::new(&spm, s.temp_k, 1.0, CAP_AH);
        let pp = Particles::of(&w, &s);
        let ends = Ends::of(&w, &pp, &s, 1.0);
        for i_p in [-2.0 * CAP_AH, 0.0, 0.05 * CAP_AH, 3.0 * CAP_AH] {
            let v_at = |k: usize, xk: f64| {
                let j = pp.j(&w, k, xk);
                pos_half(
                    &w,
                    &pp.side(&w, k),
                    ends.pos[k].at(j),
                    pp.shells,
                    pp.radius_m[k],
                    j,
                    -xk / pp.area_m2[k],
                )
            };
            let mut x = [0.0_f64; MAX_PARTICLES];
            let split = solve_split(
                &w,
                &pp,
                &ends.pos[..pp.n],
                i_p,
                i_p,
                &s.i_particles_last,
                &mut x,
            );
            assert!(split.converged);
            let worst = (0..pp.n)
                .map(|k| (v_at(k, x[k]) - split.phi).abs())
                .fold(0.0, f64::max);
            assert!(worst < 1e-10, "i_p {i_p}: a particle sits {worst} V off");
            let sum: f64 = x[..pp.n].iter().sum();
            assert!(
                (sum - i_p).abs() <= 1e-12 * CAP_AH,
                "shares sum to {sum}, not {i_p}"
            );

            // The bracketed half on its own, from the same seed.
            let tol = SPLIT_TOL_A_PER_AH * CAP_AH;
            let hx = 1.0e-6 * CAP_AH / pp.n as f64;
            let slope = |k: usize, xk: f64| {
                (-(v_at(k, xk + hx) - v_at(k, xk - hx)) / (2.0 * hx)).max(R_FLOOR_OHMS)
            };
            let mut y = [0.0_f64; MAX_PARTICLES];
            y[..pp.n].copy_from_slice(&s.i_particles_last);
            let shift = (i_p - y[..pp.n].iter().sum::<f64>()) / pp.n as f64;
            for v in &mut y[..pp.n] {
                *v += shift;
            }
            assert!(bracketed_split(&v_at, &slope, pp.n, i_p, tol, &mut y));
            for k in 0..pp.n {
                assert!(
                    (y[k] - x[k]).abs() < 1e-9 * CAP_AH,
                    "i_p {i_p}: the two halves split particle {k} as {} and {}",
                    y[k],
                    x[k]
                );
            }
        }
    }

    #[test]
    fn the_substep_count_depends_on_the_step_and_the_particles_alone() {
        assert_eq!(substeps(1, 3600.0), 1);
        assert_eq!(substeps(20, 0.0), 1);
        assert_eq!(substeps(20, f64::NAN), 1);
        assert_eq!(substeps(20, 0.5), 1);
        assert_eq!(substeps(20, 1.0), 1);
        assert_eq!(substeps(20, 10.0), 1);
        assert_eq!(substeps(20, 10.5), 2);
        assert_eq!(substeps(20, 900.0), 90);
        assert_eq!(substeps(20, 1.0e9), MAX_SUBSTEPS);
    }
}
