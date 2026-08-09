//! Doyle–Fuller–Newman cell model (`Dfn`) — the electrolyte stops being a constant.
//!
//! An [`crate::spm`] cell holds its electrolyte at one concentration everywhere; that
//! *is* the single-particle approximation. This model solves for it: a 1-D field across
//! the cell thickness (negative electrode → separator → positive electrode), coupled to
//! one representative particle **per x-node** rather than one per electrode, so the
//! reaction current can vary through the electrode and the electrolyte can starve.
//!
//! # Sign convention
//! Positive current = **discharge**, as everywhere else in this crate. On discharge the
//! local reaction current density `j` is positive in the negative electrode and negative
//! in the positive one — and unlike [`crate::spm`], **no per-electrode sign flip appears
//! anywhere in this file**. The molar flux leaving a particle surface is `j/F` at both
//! electrodes, because `j` already carries its own sign out of the solid charge balance.
//! `spm.rs` needs a flip only because its input is the *cell* current. Getting this wrong
//! in the spike made the discharge voltage **rise**, sat ~400 mV off the reference, and
//! still delivered amp-hours to cut-off within 1 % — so an energy-shaped check would have
//! passed it.
//!
//! # The four equations, and the one that is missing on purpose
//! Per x-node the unknowns are `(c_e, φ_e, φ_s, j)` and the rows are electrolyte mass,
//! electrolyte charge, solid charge, and Butler–Volmer. In the separator the last two
//! degenerate to the trivial `φ_s = 0`, `j = 0`, which keeps the block uniform.
//!
//! The solid-phase potential equation could have been dropped on the shipped parameter
//! set without anyone noticing — the spike measured the negative electrode equipotential
//! to within **36 µV even at 3C** (215 S/m is a lot of conductivity) against 668 mV of
//! the 899 mV gap living in `φ_e`. It is kept anyway: `σ_s` is chemistry data, a set with
//! a worse positive electrode would expose it, and a model named `Dfn` that silently
//! omits one of the DFN's four equations is the kind of quiet lie
//! [`crate::spm::MIN_SHELLS`] already refused once.
//!
//! # Why the particles are not in the Newton system
//! Each particle's backward-Euler radial solve is **linear in its own surface flux**, so
//! the surface concentration is an exact affine function of the local reaction current,
//! `c_surf = c0 + β·j`. Two [`crate::spm::diffuse`] calls per particle per **step** — not
//! per Newton iteration — give `c0` and `β`, after which the particles leave the system
//! entirely. That holds the unknown count at 4 per x-node instead of `4 + N_r`, and it is
//! why the Jacobian below is block-tridiagonal with a 4-wide block.
//!
//! # Why the Jacobian is analytic
//! Not an optimisation. The Phase 7 spike measured a dense *numerical* Jacobian with a
//! dense LU at **1154 µs** per cell-step at 10/5/10 — 849× an SPM step — and found the
//! Jacobian assembly, not the linear solve, to be the whole of it: 15 colours × 5.5 µs
//! is still 83 µs of residual evaluations per assembly. See `docs/plans/phase-7-dfn.md`.
//!
//! # Cost, measured rather than projected — and the projection was optimistic
//! `Pack::step` on a 1S1P pack at 10/5/10 with `N_r = 10`, priced in the same process as
//! the models it is compared against so the ratios hold where this box's absolutes do not:
//! **50–65× an `Spm` step at `N = 20`**, and ~350–500× an equivalent circuit's.
//!
//! The Phase 7 plan projected ~30× from a cost model of "one residual evaluation per
//! Jacobian assembly, ~3 Newton iterations". The analytic assembly did deliver that — the
//! measured figure is 13–23× *better* than the spike's dense-numerical 849× — but the
//! projection costed neither of the two things this implementation also pays per step:
//! the damping line-search evaluates the residual at least once more per iteration, and
//! the tangent's sensitivity solve re-assembles and re-factorises at the converged point
//! (see [`solve`]) rather than reusing a factorisation taken one Newton step short of the
//! answer. Both are deliberate; slice D re-measures and states the budget.
//!
//! Unlike the SPM there is no shell count to trade down — the cost is in the x-grid, which
//! is what the model is *for*. **A DFN pack of more than a few cells is not a real-time
//! configuration.**
//!
//! # Time-step envelope
//! Fixed-step backward Euler with a damped Newton, and the spike swept it: nothing fails
//! to converge up to **`dt = 60 s` at every rate to 3C**, including through complete
//! electrolyte depletion. Accuracy degrades well before convergence does (at 1C,
//! `dt = 60 s` overshoots the cut-off by ~50 s), which is the useful shape — the scheme
//! complains by being wrong slowly rather than by falling over. At `dt = 3600 s` it still
//! converges at 0.2C (10.4 mean iterations) and is **the wrong tool at 1C and above**,
//! where steps begin failing the tolerance. So `CLAUDE.md`'s "the same code path serves
//! real-time stepping and months-long aging fast-forward" holds here *with that stated
//! envelope*, unlike the SPM where it holds unconditionally.
//!
//! # What this cell shows the pack
//! Two things, and the split is the whole of the pack integration. [`source`] hands over a
//! **line** — the Thévenin tangent its own last solve produced, carried in
//! [`DfnState::tangent`] — which is a pure function of state and is where the pack's first
//! pass linearizes. [`probe_at`] then answers the iteration with the **curve**, by solving
//! the step again at the current that pass assigned.
//!
//! That costs a full nonlinear solve per pass, and it is one rather than three only because
//! the tangent comes off the same factorised Jacobian as a sensitivity solve. The pack asks
//! for the voltage and the tangent in a single call for exactly that reason: two calls at
//! one current would be two solves. A 1S1P constant-current step therefore runs two probes
//! and one [`advance`] — three solves where the cell alone needs one.

use serde::{Deserialize, Serialize};

use crate::aging::GAS_CONSTANT_J_PER_MOL_K;
use crate::chem::{DfnElectrode, DfnParams, ElectrodeParams, PowerTerm, SpmParams};
use crate::flags::EventFlags;
use crate::spm::{
    c_surface, diffuse, geometric_capacity_ah, mean_concentration, ocp_lookup, ocp_slope, Geometry,
    FARADAY_C_PER_MOL,
};

/// Smallest number of finite volumes a region may be discretised into.
///
/// One, and it means something different per region. A single **separator** node is an
/// ordinary coarse discretisation of a domain with no reaction in it. A single
/// **electrode** node is a cell whose reaction distribution is uniform by construction —
/// an SPM with an electrolyte — which is a defensible cheap configuration and, unlike
/// [`crate::spm::MIN_SHELLS`]'s rejected 1-shell particle, still solves every equation
/// this model claims to solve. The interesting physics (a reaction front, a starved
/// far side) needs more, and [`DEFAULT_NODES_NEGATIVE`] and its siblings say how many.
pub const MIN_NODES: usize = 1;

/// Largest number of finite volumes a region may be discretised into.
///
/// A cost ceiling rather than a physical one: unknowns are `4 ×` the total node count and
/// the banded factorisation is linear in it, so 128 per region is already ~1536 unknowns
/// and far past the point where refining changes an answer. It exists so that a
/// mistyped config fails at [`crate::Pack::new`] instead of allocating for a minute.
pub const MAX_NODES: usize = 128;

/// Negative-electrode nodes to reach for when nothing argues otherwise.
///
/// The spike's convergence and cost tables were both run at **10/5/10 with `N_r = 10`**,
/// which is where its `dt` envelope and its ~40 µs projection come from. Like
/// [`crate::spm::DEFAULT_SHELLS`] this is a recommendation, not a default anything applies
/// silently: [`crate::CellModelConfig::Dfn`] requires all three counts, because a
/// discretisation knob that fills itself in is one a caller never has to think about.
pub const DEFAULT_NODES_NEGATIVE: usize = 10;

/// Separator nodes to reach for when nothing argues otherwise. See
/// [`DEFAULT_NODES_NEGATIVE`]. Fewer than an electrode's because nothing reacts here —
/// the separator only has to carry a concentration gradient across 12 µm.
pub const DEFAULT_NODES_SEPARATOR: usize = 5;

/// Positive-electrode nodes to reach for when nothing argues otherwise. See
/// [`DEFAULT_NODES_NEGATIVE`].
pub const DEFAULT_NODES_POSITIVE: usize = 10;

/// Floor \[mol/m³\] applied to `c_e` **inside the transport and kinetics lookups only**.
///
/// # This is a physics knob wearing a numerical guard's clothes
/// `κ_e(c_e) → 0` as `c_e → 0` degenerates the electrolyte charge equation, and the
/// reference genuinely goes there: at 3C on the shipped set PyBaMM's own `c_e` reaches
/// −0.0007 mol/m³ and 90.6 % of the run has `c_e < 100 mol/m³` somewhere. So a floor is
/// needed. What it must not be is *large*, and the spike swept it at 3C:
///
/// | floor \[mol/m³\] | t to cut-off | A·h | min `c_e` | worst iterations |
/// | ---------------- | ------------ | --- | --------- | ---------------- |
/// | 100 | 1044 s | 4.483 | −343.6 | 8 |
/// | 10 | 940 s | 4.037 | −180.0 | 16 |
/// | 1 | 885 s | 3.801 | −33.6 | 18 |
/// | 0.1 | 877 s | 3.766 | −3.58 | 19 |
/// | **0.01** | **876 s** | **3.762** | −0.29 | 21 |
/// | 0.001 | 876 s | 3.762 | −0.016 | 19 |
///
/// The answer **converges in the floor** below ~0.1, and this value sits inside that
/// plateau rather than on its edge. Read the top of the table for why it is not free to
/// pick a comfortable one: a floor of 100 buys four Newton iterations and pays **0.72
/// A·h** for them, monotonically and without raising a single flag — the run simply
/// reports a healthier cell. That is what makes this a constant that owes a test pinning
/// its **inertness** rather than its value; see `dfn_cell.rs`.
///
/// Applied to the lookups, never to the state, which is [`crate::spm`]'s `clamp_surface`
/// precedent: a cell driven somewhere unphysical keeps what it was given and has to give
/// it back.
pub const C_E_FLOOR_MOL_PER_M3: f64 = 0.01;

/// Floor \[S/m\] on the *effective* ionic conductivity, after the Bruggeman correction.
///
/// Distinct from [`C_E_FLOOR_MOL_PER_M3`] and not a duplicate of it: that one keeps the
/// fit's argument inside the range it was fitted on, this one keeps a face conductance
/// finite so the charge equation stays solvable. A chemistry whose conductivity fit
/// legitimately evaluates near zero inside its own range would hit this and not the
/// other.
pub const KAPPA_FLOOR_S_PER_M: f64 = 1.0e-12;

/// Convergence tolerance for the cell's Newton solve, on a **row-scaled** infinity norm.
///
/// The scaling is not decoration. The charge rows carry A/m² and the mass row
/// mol/(m²·s), and on the shipped set they differ by ~1e5 — an unscaled norm would
/// declare the mass equation converged long before it is, which is the same class of
/// mistake as a metric measuring one thing under another thing's label. Each row is
/// divided by a representative magnitude of its own terms before the norm is taken.
pub const NEWTON_TOL: f64 = 1.0e-8;

/// How many step lengths the Newton's damping line-search may try before giving up and
/// taking the smallest of them.
///
/// **Measured, not chosen.** Each attempt halves, so the smallest step considered is
/// `2^-(this-1)` of the full Newton step. Counting the steps of a 3C discharge to cut-off
/// (10/5/10, `N_r = 10`, `dt = 2 s`) that failed to meet [`NEWTON_TOL`]:
///
/// | attempts | 11 | 12 | **13** | 14 | 16 | 20 |
/// | -------- | -- | -- | ------ | -- | -- | -- |
/// | unconverged steps | 53 | 27 | **0** | 0 | 0 | 0 |
///
/// The knee is sharp and it sits at 13, i.e. the step that gets a depleted cell moving is
/// around `2^-12` of the Newton step — small enough that a search stopping one halving
/// early looks like it has hit a local minimum of the residual and gives up. The
/// trajectory those 27 steps produced was the same to every printed digit; what moved was
/// the *flag*, which is the honest thing to have moved and exactly why the flag exists.
///
/// Set past the knee rather than on it. Attempts beyond the accepted one are never made,
/// so a higher cap costs nothing on any step that converges.
const DAMPING_ATTEMPTS: u32 = 16;

/// Iteration cap for the cell's Newton solve.
///
/// Reaching it raises [`EventFlags::SOLVE_UNCONVERGED`] and the step proceeds on the last
/// iterate — `step` reports, it does not fail. Set above the spike's worst measured case
/// (21 iterations, at 3C through depletion at this floor) with room for the damping
/// line-search, so hitting it means something has genuinely gone wrong rather than that a
/// hard step needed one pass more than usual.
pub const NEWTON_ITER_CAP: u32 = 50;

/// Unknowns per x-node: `(c_e, φ_e, φ_s, j)`.
const NVAR: usize = 4;
/// Offset of the electrolyte concentration within a node's unknowns.
const CE: usize = 0;
/// Offset of the electrolyte potential within a node's unknowns.
const PHIE: usize = 1;
/// Offset of the solid potential within a node's unknowns.
const PHIS: usize = 2;
/// Offset of the reaction current density within a node's unknowns.
const JJ: usize = 3;

/// Half-bandwidth of the assembled Jacobian, in scalar rows.
///
/// The matrix is block-tridiagonal with `NVAR`-wide blocks, so the furthest a row reaches
/// is the far corner of a neighbouring block: `2·NVAR − 1`.
const BAND: usize = 2 * NVAR - 1;

/// Per-cell Doyle–Fuller–Newman state: an electrolyte field, one particle per electrode
/// node, a temperature, and what the last solve left behind.
///
/// Opaque to the pack layer exactly as [`crate::EcmState`] and [`crate::SpmState`] are.
///
/// # The node counts are not stored, and that is deliberate
/// They are recoverable: `c_neg.len()` and `c_pos.len()` are the electrode node counts and
/// the separator's is `c_e.len()` minus the two. Storing them as well would put the same
/// fact in two places, where a hand-edited snapshot could make them disagree. The
/// subtraction is done saturating (see [`Grid::of`]) so that a corrupt blob produces a
/// degenerate grid rather than a panic — `step` may not panic on any input.
///
/// # This snapshot is an order of magnitude larger than an SPM cell's, and that is fine
/// At 10/5/10 with `N_r = 10` a DFN cell is 25 `c_e` + 200 particle shells + 100 warm-start
/// entries ≈ **325 `f64`** against an SPM cell's ~41. No compact encoding is reached for:
/// `CLAUDE.md` asks for one serde value with a version field, and the readability of a
/// snapshot has been worth more than its size at every previous phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DfnState {
    /// Electrolyte concentration per x-node \[mol/m³\], negative current collector first.
    pub c_e: Vec<f64>,
    /// Solid concentration profiles, one per **negative**-electrode node, innermost shell
    /// first \[mol/m³\].
    pub c_neg: Vec<Vec<f64>>,
    /// Solid concentration profiles, one per **positive**-electrode node \[mol/m³\].
    pub c_pos: Vec<Vec<f64>>,
    /// Last converged unknown vector — the next step's Newton starting guess.
    ///
    /// **State, not a cache**, and the exact sibling of [`crate::SpmState::i_last`]. A
    /// Newton that stops at a tolerance lands somewhere that depends on where it started,
    /// so the starting point decides the trajectory at the [`NEWTON_TOL`] level. A
    /// restored cell that re-seeded from a cold default would continue a *different*
    /// trajectory, and snapshot-restore bit-identity is an exit criterion of this phase.
    pub u: Vec<f64>,
    /// Cell temperature \[K\]. Advanced by [`crate::thermal`], as every model's is — no
    /// cell model integrates its own temperature.
    pub temp_k: f64,
    /// Current \[A, discharge-positive\] this cell carried over the previous step.
    pub i_last: f64,
    /// Thévenin tangent `(E, R)` from the last solve, or `None` on a cell that has never
    /// been advanced.
    ///
    /// `R = −dV/di` comes off the converged Jacobian as a sensitivity solve — one extra
    /// back-substitution, exact to the discretisation rather than a difference quotient —
    /// and `E = V + i·R` so the line passes through the point that was actually solved.
    /// Because the solve's converged field *is* the end-of-step state, that point is also
    /// the start-of-step operating point of the step that reads this, which is what makes
    /// a stored line a tangent rather than a stale extrapolation.
    ///
    /// `None` is not "zero": it is a cell whose curve has never been evaluated, and
    /// [`source`] answers it with a documented first-order seed rather than a solve.
    pub tangent: Option<(f64, f64)>,
}

impl DfnState {
    /// A fresh cell at `soc`: every particle uniform at the concentration that state of
    /// charge implies, the electrolyte uniform at the chemistry's nominal concentration,
    /// and a Newton guess seeded at open circuit.
    ///
    /// The electrolyte's initial value is [`SpmParams::c_e_mol_per_m3`] — the number the
    /// single-particle model holds constant *is* the DFN's initial uniform field, which is
    /// the plan's "`[dfn]` extends `[spm]`" decision cashing out rather than a field this
    /// model adds.
    pub(crate) fn new(
        spm: &SpmParams,
        nodes: (usize, usize, usize),
        shells: usize,
        soc: f64,
        temp_k: f64,
    ) -> Self {
        let (n_n, n_s, n_p) = nodes;
        let x = spm.negative.stoich_min + soc * (spm.negative.stoich_max - spm.negative.stoich_min);
        let y = spm.positive.stoich_max - soc * (spm.positive.stoich_max - spm.positive.stoich_min);
        let n = n_n + n_s + n_p;
        // Seeded at open circuit rather than at zeros, so the first step's Newton starts
        // from a physically meaningful field instead of from a state no cell is ever in.
        // `φ_e = 0` is the gauge; `φ_s` is each electrode's own OCP; `j = 0` is a cell at
        // rest, which is what a cell that has not stepped yet is.
        let mut u = vec![0.0; NVAR * n];
        for (i, chunk) in u.chunks_exact_mut(NVAR).enumerate() {
            chunk[CE] = spm.c_e_mol_per_m3;
            if i < n_n {
                chunk[PHIS] = ocp_lookup(&spm.negative.ocp, x);
            } else if i >= n_n + n_s {
                chunk[PHIS] = ocp_lookup(&spm.positive.ocp, y);
            }
        }
        Self {
            c_e: vec![spm.c_e_mol_per_m3; n],
            c_neg: vec![vec![x * spm.negative.c_max_mol_per_m3; shells]; n_n],
            c_pos: vec![vec![y * spm.positive.c_max_mol_per_m3; shells]; n_p],
            u,
            temp_k,
            i_last: 0.0,
            tangent: None,
        }
    }
}

/// Which of the three domains an x-node belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    /// Negative electrode.
    Neg,
    /// Separator: no solid phase, no reaction.
    Sep,
    /// Positive electrode.
    Pos,
}

/// The cell-centred finite-volume grid across the cell thickness, and the per-node
/// constants that ride on it.
///
/// Rebuilt per call rather than stored, on [`crate::spm::Geometry`]'s precedent: it is a
/// handful of multiplies from numbers already in the chemistry, and caching it would put
/// derived state inside the snapshot.
struct Grid {
    n_n: usize,
    n_s: usize,
    n_p: usize,
    /// Node widths \[m\].
    h: Vec<f64>,
    /// Electrolyte volume fraction per node.
    eps_e: Vec<f64>,
    /// Bruggeman factor `ε_e^b` per node, i.e. the multiplier on every bulk electrolyte
    /// transport property.
    bg: Vec<f64>,
    /// Interfacial area per unit electrode volume `3·ε_s/R_p` \[1/m\]; exactly `0` in the
    /// separator, which is what makes every reaction term vanish there without a branch.
    a_s: Vec<f64>,
    /// Effective solid conductivity `σ_s·ε_s^b` \[S/m\]; `0` in the separator.
    sigma: Vec<f64>,
}

impl Grid {
    /// Build the grid a state implies. `n_s` is recovered by subtraction and **saturates**:
    /// a hand-edited snapshot whose vectors disagree gets an empty separator rather than a
    /// panicking underflow.
    fn of(spm: &SpmParams, dfn: &DfnParams, sides: &Sides<'_>, s: &DfnState) -> Self {
        let n_n = s.c_neg.len();
        let n_p = s.c_pos.len();
        let n_s = s.c_e.len().saturating_sub(n_n + n_p);
        let n = n_n + n_s + n_p;
        let mut g = Grid {
            n_n,
            n_s,
            n_p,
            h: Vec::with_capacity(n),
            eps_e: Vec::with_capacity(n),
            bg: Vec::with_capacity(n),
            a_s: Vec::with_capacity(n),
            sigma: Vec::with_capacity(n),
        };
        let regions = [
            (
                n_n,
                spm.negative.thickness_m,
                dfn.negative.porosity,
                dfn.negative.bruggeman_electrolyte,
                sides.neg.a_s,
                sides.neg.sigma_eff,
            ),
            (
                n_s,
                dfn.separator.thickness_m,
                dfn.separator.porosity,
                dfn.separator.bruggeman_electrolyte,
                0.0,
                0.0,
            ),
            (
                n_p,
                spm.positive.thickness_m,
                dfn.positive.porosity,
                dfn.positive.bruggeman_electrolyte,
                sides.pos.a_s,
                sides.pos.sigma_eff,
            ),
        ];
        for (count, length, porosity, brug, a_s, sigma) in regions {
            if count == 0 {
                continue;
            }
            let dx = length / count as f64;
            let bg = powx(porosity, brug);
            for _ in 0..count {
                g.h.push(dx);
                g.eps_e.push(porosity);
                g.bg.push(bg);
                g.a_s.push(a_s);
                g.sigma.push(sigma);
            }
        }
        g
    }

    fn n(&self) -> usize {
        self.n_n + self.n_s + self.n_p
    }

    /// Index of the first positive-electrode node.
    fn first_pos(&self) -> usize {
        self.n_n + self.n_s
    }

    fn region(&self, i: usize) -> Region {
        if i < self.n_n {
            Region::Neg
        } else if i < self.first_pos() {
            Region::Sep
        } else {
            Region::Pos
        }
    }
}

/// One electrode resolved against this cell's temperature and scale factors.
struct Side<'a> {
    p: &'a ElectrodeParams,
    /// Solid diffusivity \[m²/s\] at the cell's temperature.
    d_s: f64,
    /// Exchange-current amplitude at the cell's temperature, after the resistance-growth
    /// division (see [`Sides::new`]).
    m_ref: f64,
    /// Interfacial area per unit electrode volume `3·ε_s/R_p` \[1/m\].
    a_s: f64,
    /// Effective solid conductivity `σ_s·ε_s^b` \[S/m\].
    sigma_eff: f64,
}

/// Everything a step needs that is derived from the chemistry and the cell's scale
/// factors rather than stored.
struct Sides<'a> {
    neg: Side<'a>,
    pos: Side<'a>,
    /// Charge-to-stoichiometry scale: geometric capacity ÷ effective capacity.
    ///
    /// The same quantity, with the same meaning and the same deliberate omissions, as
    /// [`crate::spm`]'s `Working::kappa` — read that doc for why it scales the flux the
    /// particles see and **not** the interfacial area or the Butler–Volmer current
    /// density. Here that split falls out cleanly: `κ` multiplies only the molar flux in
    /// [`particle_map`], while `j` itself stays geometric everywhere it appears in the
    /// charge balances.
    kappa: f64,
    /// Lumped ohmic resistance \[ohms\] after the resistance-growth multiplier.
    r_contact: f64,
    /// Electrode plate area \[m²\].
    area_m2: f64,
    /// Cell temperature \[K\].
    temp_k: f64,
}

impl<'a> Sides<'a> {
    fn new(
        spm: &'a SpmParams,
        dfn: &'a DfnParams,
        temp_k: f64,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
    ) -> Sides<'a> {
        // `eff_r0_factor` lands in the same two places it does for an SPM cell, and for
        // the same reason: the shipped chemistry's `contact_resistance_ohm` is Chen2020's
        // own **0**, so multiplying it alone would fade capacity with exactly zero
        // resistance growth — the one thing `CLAUDE.md` forbids outright. Dividing
        // `m_ref` multiplies the linearized charge-transfer resistance by the factor
        // exactly, on both electrodes. See `crate::spm::Working::new`.
        let side = |p: &'a ElectrodeParams, d: &'a DfnElectrode| Side {
            p,
            d_s: p.diffusivity_m2_per_s
                * arrhenius(p.diffusivity_ea_j_per_mol, spm.t_ref_k, temp_k),
            m_ref: p.m_ref * arrhenius(p.reaction_ea_j_per_mol, spm.t_ref_k, temp_k)
                / eff_r0_factor,
            // The same `3·ε_s/R_p` [`crate::spm::Geometry`] folds into a total area. Here
            // it stays *per unit electrode volume*, because a DFN spreads the reaction
            // over x rather than over one lumped particle.
            a_s: 3.0 * p.active_volume_fraction / p.particle_radius_m,
            sigma_eff: d.solid_conductivity_s_per_m
                * powx(p.active_volume_fraction, d.bruggeman_electrode),
        };
        Sides {
            kappa: geometric_capacity_ah(&spm.negative, Geometry::of(spm, &spm.negative))
                / eff_capacity_ah,
            neg: side(&spm.negative, &dfn.negative),
            pos: side(&spm.positive, &dfn.positive),
            r_contact: spm.contact_resistance_ohm * eff_r0_factor,
            area_m2: spm.electrode_area_m2,
            temp_k,
        }
    }

    fn side(&self, region: Region) -> Option<&Side<'a>> {
        match region {
            Region::Neg => Some(&self.neg),
            Region::Pos => Some(&self.pos),
            Region::Sep => None,
        }
    }
}

/// Arrhenius correction `exp(Ea/R · (1/T_ref − 1/T))`. Exactly `1.0` for `Ea = 0` without
/// evaluating an exponential, which is the shipped chemistry's diffusivity.
#[must_use]
fn arrhenius(ea_j_per_mol: f64, t_ref_k: f64, temp_k: f64) -> f64 {
    if ea_j_per_mol == 0.0 {
        return 1.0;
    }
    (ea_j_per_mol / GAS_CONSTANT_J_PER_MOL_K * (1.0 / t_ref_k - 1.0 / temp_k)).exp()
}

/// `x^p`, with the exponents the shipped fits actually use evaluated in **pure IEEE-754
/// arithmetic** rather than through `powf`.
///
/// # This choice is load-bearing and was made before any golden was committed
/// [`crate::chem::PowerTerm`]'s own doc records Phase 6's rule: only pure IEEE arithmetic
/// and decimal→`f64` parsing may be committed as an exact-bit assertion, because those are
/// identical on every conforming platform while `exp`, `asinh` and `powf` are not. Nyman's
/// conductivity fit has an `x^1.5` term; evaluated as `x·√x` it is pinnable (`sqrt` is
/// IEEE-exact), evaluated as `powf(1.5)` it is not. Changing the form later would move
/// every DFN golden, so it is fixed here.
///
/// A general exponent still falls through to `powf` — the schema allows one, and a
/// chemistry that uses it simply is not bit-pinnable, which is a property of that
/// chemistry rather than of this function.
#[must_use]
fn powx(x: f64, p: f64) -> f64 {
    if p == 0.0 {
        1.0
    } else if p == 1.0 {
        x
    } else if p == 2.0 {
        x * x
    } else if p == 3.0 {
        x * x * x
    } else if p == 0.5 {
        x.sqrt()
    } else if p == 1.5 {
        x * x.sqrt()
    } else {
        x.powf(p)
    }
}

/// A transport fit `Σ aᵢ·x^pᵢ` evaluated at `x = c_e/1000`, the fit's own variable.
#[must_use]
fn eval_terms(terms: &[PowerTerm], x: f64) -> f64 {
    terms
        .iter()
        .map(|t| t.coefficient * powx(x, t.exponent))
        .sum()
}

/// `d/dx` of [`eval_terms`].
///
/// The constant term is special-cased to exactly `0.0` rather than computed. `a·p·x^(p−1)`
/// with `p = 0` is `0 · x^-1`, which at a depleted node is `0 · ∞ = NaN` — and a depleted
/// node is precisely the regime this model exists to represent. Both shipped fits have a
/// constant term.
#[must_use]
fn eval_terms_d(terms: &[PowerTerm], x: f64) -> f64 {
    terms
        .iter()
        .map(|t| {
            if t.exponent == 0.0 {
                0.0
            } else {
                t.coefficient * t.exponent * powx(x, t.exponent - 1.0)
            }
        })
        .sum()
}

/// The affine map `c_surf = c0 + β·j` for one particle, plus the two profiles the state
/// update is reconstructed from by the same linearity.
#[derive(Clone)]
struct Particle {
    /// Surface concentration at `j = 0` \[mol/m³\].
    c0: f64,
    /// Change in surface concentration per unit reaction current density
    /// \[mol/m³ per A/m²\]. Negative: driving lithium out of a surface lowers it.
    beta: f64,
    /// Shell profile after the step at `j = 0`.
    prof0: Vec<f64>,
    /// Shell profile after the step at `j = 1 A/m²` — the other end of the affine map.
    prof1: Vec<f64>,
}

/// Build one particle's affine surface map by solving the **same** backward-Euler system
/// twice, at two values of the local reaction current. Exact, because that system is
/// linear in the flux.
///
/// `kappa` is the capacity scale: the molar flux a unit reaction current density produces
/// is `κ/F`, not `1/F`, which is the whole of how manufacturing scatter,
/// [`crate::Fault::WeakCell`] and aging's `soh_capacity` reach a porous-electrode cell.
fn particle_map(c_old: &[f64], e: &ElectrodeParams, d_s: f64, kappa: f64, dt: f64) -> Particle {
    let r_p = e.particle_radius_m;
    let mut prof0 = c_old.to_vec();
    diffuse(&mut prof0, r_p, d_s, 0.0, dt);
    let flux_unit = kappa / FARADAY_C_PER_MOL;
    let mut prof1 = c_old.to_vec();
    diffuse(&mut prof1, r_p, d_s, flux_unit, dt);
    let surf0 = c_surface(&prof0, r_p, d_s, 0.0);
    let surf1 = c_surface(&prof1, r_p, d_s, flux_unit);
    Particle {
        c0: surf0,
        beta: surf1 - surf0,
        prof0,
        prof1,
    }
}

/// Per-node electrolyte properties and their concentration derivatives, evaluated once per
/// residual or Jacobian assembly.
#[derive(Clone, Copy, Default)]
struct Transport {
    /// Concentration after the lookup floor \[mol/m³\].
    ce: f64,
    /// `1.0` while the floor is not biting, `0.0` while it is — the derivative of the
    /// clamped concentration with respect to the stored one.
    live: f64,
    /// Effective diffusivity \[m²/s\].
    d: f64,
    /// `∂d/∂c_e`.
    dd: f64,
    /// Effective ionic conductivity \[S/m\], after its own floor.
    k: f64,
    /// `∂k/∂c_e`.
    dk: f64,
    /// Diffusional conductivity `κ_D` \[A/m\].
    kd: f64,
    /// `∂κ_D/∂c_e`.
    dkd: f64,
}

/// Everything fixed for the duration of one step's solve.
struct System<'a> {
    grid: &'a Grid,
    dfn: &'a DfnParams,
    sides: &'a Sides<'a>,
    parts_neg: &'a [Particle],
    parts_pos: &'a [Particle],
    c_e_old: &'a [f64],
    dt: f64,
    /// Applied current density \[A/m²\], discharge-positive.
    i_app: f64,
    /// `2·R·T/F·(1 − t₊)·(1 + ∂ln f/∂ln c)`, the coefficient of the diffusional
    /// conductivity. Pulled out because it is the same at every node.
    kd_coef: f64,
}

/// `2·R·T/F·(1 − t₊)·(1 + ∂ln f/∂ln c)`: the coefficient turning an ionic conductivity
/// into a diffusional one. The same at every node, so it is computed once per step.
#[must_use]
fn kd_coef(dfn: &DfnParams, temp_k: f64) -> f64 {
    2.0 * GAS_CONSTANT_J_PER_MOL_K * temp_k / FARADAY_C_PER_MOL
        * (1.0 - dfn.transference_number)
        * dfn.thermodynamic_factor
}

impl<'a> System<'a> {
    /// The particle at node `i`, or `None` in the separator.
    fn particle(&self, i: usize) -> Option<&Particle> {
        match self.grid.region(i) {
            Region::Neg => self.parts_neg.get(i),
            Region::Pos => self.parts_pos.get(i - self.grid.first_pos()),
            Region::Sep => None,
        }
    }

    /// Electrolyte properties at one node.
    fn transport(&self, c: f64, i: usize) -> Transport {
        let floored = c < C_E_FLOOR_MOL_PER_M3;
        let ce = if floored { C_E_FLOOR_MOL_PER_M3 } else { c };
        let live = if floored { 0.0 } else { 1.0 };
        let bg = self.grid.bg[i];
        let x = ce / 1000.0;
        // The fits are in `x = c_e/1000`, so every derivative picks up a 1/1000.
        let d = eval_terms(&self.dfn.electrolyte_diffusivity_terms, x) * bg;
        let dd = eval_terms_d(&self.dfn.electrolyte_diffusivity_terms, x) * bg * live / 1000.0;
        let k_raw = eval_terms(&self.dfn.electrolyte_conductivity_terms, x) * bg;
        let k_floored = k_raw < KAPPA_FLOOR_S_PER_M;
        let k = if k_floored {
            KAPPA_FLOOR_S_PER_M
        } else {
            k_raw
        };
        let dk = if k_floored {
            0.0
        } else {
            eval_terms_d(&self.dfn.electrolyte_conductivity_terms, x) * bg * live / 1000.0
        };
        Transport {
            ce,
            live,
            d,
            dd,
            k,
            dk,
            kd: k * self.kd_coef,
            dkd: dk * self.kd_coef,
        }
    }

    /// Every node's transport properties for one unknown vector.
    fn transports(&self, u: &[f64], out: &mut Vec<Transport>) {
        out.clear();
        for i in 0..self.grid.n() {
            out.push(self.transport(u[NVAR * i + CE], i));
        }
    }

    /// Harmonic (series) face value between nodes `i` and `i+1` for a per-node property.
    ///
    /// Series rather than arithmetic so that the porosity jump at a separator interface is
    /// handled conservatively: it is two resistances in series, which is what a flux
    /// crossing the face actually sees.
    fn face(&self, p_lo: f64, p_hi: f64, i: usize) -> f64 {
        let h = &self.grid.h;
        1.0 / (h[i] / (2.0 * p_lo.max(1.0e-300)) + h[i + 1] / (2.0 * p_hi.max(1.0e-300)))
    }

    /// `∂face/∂p` on each side, given the face value itself.
    fn face_sens(&self, f: f64, p_lo: f64, p_hi: f64, i: usize) -> (f64, f64) {
        let h = &self.grid.h;
        let lo = p_lo.max(1.0e-300);
        let hi = p_hi.max(1.0e-300);
        (
            f * f * h[i] / (2.0 * lo * lo),
            f * f * h[i + 1] / (2.0 * hi * hi),
        )
    }

    /// Butler–Volmer pieces at one electrode node: the exchange current, the sinh
    /// argument, and the surface concentration the kinetics saw.
    fn kinetics(&self, u: &[f64], t: &Transport, i: usize) -> Kinetics {
        // Unreachable in the separator: every caller checks the region first.
        let Some(side) = self.sides.side(self.grid.region(i)) else {
            return Kinetics::default();
        };
        let Some(part) = self.particle(i) else {
            return Kinetics::default();
        };
        let c_max = side.p.c_max_mol_per_m3;
        let j = u[NVAR * i + JJ];
        let cs_raw = part.c0 + part.beta * j;
        // Same guard, same argument, as `spm::clamp_surface`: `i_0 ∝ √(c_s·(c_max − c_s))`
        // is real only inside `(0, c_max)`, and an overdriven cell can put a *surface*
        // concentration outside it for a step. The state itself is never clamped.
        const EDGE: f64 = 1.0e-6;
        let lo = EDGE * c_max;
        let hi = (1.0 - EDGE) * c_max;
        let clamped = cs_raw < lo || cs_raw > hi;
        let cs = cs_raw.clamp(lo, hi);
        let prod = cs * (c_max - cs);
        let i0 = side.m_ref * (t.ce * prod).sqrt();
        let eta = u[NVAR * i + PHIS] - u[NVAR * i + PHIE] - ocp_lookup(&side.p.ocp, cs / c_max);
        let scale = side.p.charge_transfer_alpha * FARADAY_C_PER_MOL
            / (GAS_CONSTANT_J_PER_MOL_K * self.sides.temp_k);
        Kinetics {
            i0,
            arg: scale * eta,
            scale,
            // A clamped surface no longer moves with `j`, so every derivative through it
            // is zero — the branch the analytic Jacobian takes and a central difference
            // straddles.
            dcs_dj: if clamped { 0.0 } else { part.beta },
            // d(√(c_e·c_s·(c_max − c_s)))/dc_s, factored through `i0` itself.
            di0_dcs: if prod > 0.0 {
                i0 * (c_max - 2.0 * cs) / (2.0 * prod)
            } else {
                0.0
            },
            di0_dce: if t.ce > 0.0 {
                i0 / (2.0 * t.ce) * t.live
            } else {
                0.0
            },
            docp_dcs: ocp_slope(&side.p.ocp, cs / c_max) / c_max,
        }
    }

    /// The full residual vector. Together with [`Self::jacobian`] this is the only place
    /// the physics lives, and the two are written against the same equations in the same
    /// order so a reader can check them against each other line by line.
    fn residual(&self, u: &[f64], tr: &[Transport], r: &mut [f64]) {
        let g = self.grid;
        let n = g.n();
        let t_plus = self.dfn.transference_number;
        let c = |i: usize| u[NVAR * i + CE];
        let phie = |i: usize| u[NVAR * i + PHIE];
        let phis = |i: usize| u[NVAR * i + PHIS];
        let jr = |i: usize| u[NVAR * i + JJ];

        for i in 0..n {
            // --- electrolyte mass. `q` is the diffusive flux across a face; both current
            // collectors are no-flux.
            let q_l = if i == 0 {
                0.0
            } else {
                -self.face(tr[i - 1].d, tr[i].d, i - 1) * (c(i) - c(i - 1))
            };
            let q_r = if i == n - 1 {
                0.0
            } else {
                -self.face(tr[i].d, tr[i + 1].d, i) * (c(i + 1) - c(i))
            };
            r[NVAR * i + CE] = g.eps_e[i] * g.h[i] * (c(i) - self.c_e_old[i]) / self.dt
                + (q_r - q_l)
                - g.a_s[i] * (1.0 - t_plus) * jr(i) / FARADAY_C_PER_MOL * g.h[i];

            // --- electrolyte charge. `i_e = −κ·∂φ_e/∂x + κ_D·∂ln c_e/∂x`, zero at both
            // collectors.
            let ie = |a: usize| {
                let b = a + 1;
                -self.face(tr[a].k, tr[b].k, a) * (phie(b) - phie(a))
                    + self.face(tr[a].kd, tr[b].kd, a) * (tr[b].ce.ln() - tr[a].ce.ln())
            };
            let ie_l = if i == 0 { 0.0 } else { ie(i - 1) };
            let ie_r = if i == n - 1 { 0.0 } else { ie(i) };
            r[NVAR * i + PHIE] = (ie_r - ie_l) - g.a_s[i] * jr(i) * g.h[i];
        }
        // Gauge: the `(φ_e, φ_s)` system is Neumann everywhere and singular by exactly one
        // global shift, so one redundant charge row is replaced by a datum.
        r[PHIE] = phie(0);

        // --- solid charge and Butler–Volmer.
        let last_neg = g.n_n.wrapping_sub(1);
        let first_pos = g.first_pos();
        for i in 0..n {
            if self.sides.side(g.region(i)).is_none() {
                r[NVAR * i + PHIS] = phis(i);
                r[NVAR * i + JJ] = jr(i);
                continue;
            }
            // `i_s = −σ·∂φ_s/∂x`, carrying the whole applied current at the two collectors
            // and nothing at either face onto the separator. The order of these tests
            // matters: at `n_n == 1` the single negative node is both node 0 and
            // `last_neg`, and it must take the collector on its left and the separator on
            // its right.
            let is = |a: usize| -self.face(g.sigma[a], g.sigma[a + 1], a) * (phis(a + 1) - phis(a));
            let is_l = if i == 0 {
                self.i_app
            } else if i == first_pos {
                0.0
            } else {
                is(i - 1)
            };
            let is_r = if i == n - 1 {
                self.i_app
            } else if i == last_neg {
                0.0
            } else {
                is(i)
            };
            r[NVAR * i + PHIS] = (is_r - is_l) + g.a_s[i] * jr(i) * g.h[i];

            let k = self.kinetics(u, &tr[i], i);
            r[NVAR * i + JJ] = jr(i) - 2.0 * k.i0 * k.arg.sinh();
        }
    }

    /// The analytic Jacobian of [`Self::residual`], assembled straight into band storage.
    fn jacobian(&self, u: &[f64], tr: &[Transport], band: &mut Band) {
        let g = self.grid;
        let n = g.n();
        let t_plus = self.dfn.transference_number;
        let c = |i: usize| u[NVAR * i + CE];
        let phie = |i: usize| u[NVAR * i + PHIE];
        band.clear();

        for i in 0..n {
            let row_ce = NVAR * i + CE;
            let row_pe = NVAR * i + PHIE;

            // --- electrolyte mass.
            band.add(row_ce, NVAR * i + CE, g.eps_e[i] * g.h[i] / self.dt);
            band.add(
                row_ce,
                NVAR * i + JJ,
                -g.a_s[i] * (1.0 - t_plus) / FARADAY_C_PER_MOL * g.h[i],
            );
            if i + 1 < n {
                // q_r = −F·(c_{i+1} − c_i), F = face(d_i, d_{i+1}).
                let f = self.face(tr[i].d, tr[i + 1].d, i);
                let (df_lo, df_hi) = self.face_sens(f, tr[i].d, tr[i + 1].d, i);
                let dc = c(i + 1) - c(i);
                band.add(row_ce, NVAR * i + CE, f - df_lo * tr[i].dd * dc);
                band.add(row_ce, NVAR * (i + 1) + CE, -f - df_hi * tr[i + 1].dd * dc);
            }
            if i > 0 {
                // −q_l = +F·(c_i − c_{i−1}), F = face(d_{i−1}, d_i).
                let f = self.face(tr[i - 1].d, tr[i].d, i - 1);
                let (df_lo, df_hi) = self.face_sens(f, tr[i - 1].d, tr[i].d, i - 1);
                let dc = c(i) - c(i - 1);
                band.add(row_ce, NVAR * i + CE, f + df_hi * tr[i].dd * dc);
                band.add(row_ce, NVAR * (i - 1) + CE, -f + df_lo * tr[i - 1].dd * dc);
            }

            // --- electrolyte charge. Row 0 is the gauge and is written after this loop.
            band.add(row_pe, NVAR * i + JJ, -g.a_s[i] * g.h[i]);
            // `sign` is +1 for the right-hand face (which enters as `+i_e`) and −1 for the
            // left-hand one, so one body serves both.
            for (a, sign) in [(i, 1.0_f64), (i.wrapping_sub(1), -1.0)] {
                if sign > 0.0 && i + 1 >= n {
                    continue;
                }
                if sign < 0.0 && i == 0 {
                    continue;
                }
                let b = a + 1;
                let kf = self.face(tr[a].k, tr[b].k, a);
                let (dk_lo, dk_hi) = self.face_sens(kf, tr[a].k, tr[b].k, a);
                let kdf = self.face(tr[a].kd, tr[b].kd, a);
                let (dkd_lo, dkd_hi) = self.face_sens(kdf, tr[a].kd, tr[b].kd, a);
                let dphi = phie(b) - phie(a);
                let dln = tr[b].ce.ln() - tr[a].ce.ln();
                band.add(row_pe, NVAR * a + PHIE, sign * kf);
                band.add(row_pe, NVAR * b + PHIE, -sign * kf);
                band.add(
                    row_pe,
                    NVAR * a + CE,
                    sign * (-dk_lo * tr[a].dk * dphi + dkd_lo * tr[a].dkd * dln
                        - kdf * tr[a].live / tr[a].ce),
                );
                band.add(
                    row_pe,
                    NVAR * b + CE,
                    sign * (-dk_hi * tr[b].dk * dphi
                        + dkd_hi * tr[b].dkd * dln
                        + kdf * tr[b].live / tr[b].ce),
                );
            }
        }
        // The gauge row replaces everything written into row `PHIE` above.
        band.zero_row(PHIE);
        band.add(PHIE, PHIE, 1.0);

        // --- solid charge and Butler–Volmer.
        let last_neg = g.n_n.wrapping_sub(1);
        let first_pos = g.first_pos();
        // Indexed rather than iterated: the body reaches `g.sigma` at `i − 1` and `i + 1`
        // as well as at `i`, so an enumeration over one of them would only move the
        // arithmetic rather than remove it.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let row_ps = NVAR * i + PHIS;
            let row_j = NVAR * i + JJ;
            if self.sides.side(g.region(i)).is_none() {
                band.add(row_ps, NVAR * i + PHIS, 1.0);
                band.add(row_j, NVAR * i + JJ, 1.0);
                continue;
            }
            band.add(row_ps, NVAR * i + JJ, g.a_s[i] * g.h[i]);
            // Mirrors the residual's branch structure exactly; a collector face carries a
            // constant and so contributes nothing here.
            if !(i == 0 || i == first_pos) {
                let s = self.face(g.sigma[i - 1], g.sigma[i], i - 1);
                band.add(row_ps, NVAR * (i - 1) + PHIS, -s);
                band.add(row_ps, NVAR * i + PHIS, s);
            }
            if !(i == n - 1 || i == last_neg) {
                let s = self.face(g.sigma[i], g.sigma[i + 1], i);
                band.add(row_ps, NVAR * i + PHIS, s);
                band.add(row_ps, NVAR * (i + 1) + PHIS, -s);
            }

            let k = self.kinetics(u, &tr[i], i);
            let sh = k.arg.sinh();
            let ch = k.arg.cosh();
            // r_j = j − 2·i0(c_e, c_s(j))·sinh(scale·η(φ_s, φ_e, c_s(j)))
            band.add(row_j, NVAR * i + PHIS, -2.0 * k.i0 * ch * k.scale);
            band.add(row_j, NVAR * i + PHIE, 2.0 * k.i0 * ch * k.scale);
            band.add(row_j, NVAR * i + CE, -2.0 * k.di0_dce * sh);
            let deta_dj = -k.docp_dcs * k.dcs_dj;
            band.add(
                row_j,
                NVAR * i + JJ,
                1.0 - 2.0 * (k.di0_dcs * k.dcs_dj * sh + k.i0 * ch * k.scale * deta_dj),
            );
        }
    }
}

/// Butler–Volmer pieces at one node, and the derivatives the Jacobian needs.
#[derive(Default)]
struct Kinetics {
    i0: f64,
    /// The sinh argument `α·F·η/(R·T)`.
    arg: f64,
    /// `α·F/(R·T)`.
    scale: f64,
    /// `∂c_s/∂j`, exactly `0` past the surface clamp.
    dcs_dj: f64,
    /// `∂i₀/∂c_s`.
    di0_dcs: f64,
    /// `∂i₀/∂c_e`, already carrying the lookup floor's derivative.
    di0_dce: f64,
    /// `∂U/∂c_s` — the OCP table's own segment slope, divided by `c_max`.
    docp_dcs: f64,
}

/// A square band matrix in LAPACK's `dgbtrf` storage, factorised in place.
///
/// Lower and upper half-bandwidths are both [`BAND`], and the extra `BAND` rows above hold
/// the fill-in that row interchanges create. Entry `(i, j)` lives at storage row
/// `2·BAND + i − j`, which keeps a column contiguous and is what makes the pivot swaps
/// index-safe: a swap only ever moves entries within one column.
struct Band {
    m: usize,
    data: Vec<f64>,
    piv: Vec<usize>,
}

impl Band {
    fn new(m: usize) -> Self {
        Self {
            m,
            data: vec![0.0; (3 * BAND + 1) * m],
            piv: vec![0; m],
        }
    }

    fn clear(&mut self) {
        self.data.fill(0.0);
    }

    /// Storage index of `(i, j)`, or `None` if the entry lies outside the band — which for
    /// a correct assembly never happens, and which a debug build asserts.
    fn index(&self, i: usize, j: usize) -> Option<usize> {
        let r = 2 * BAND + i;
        if r < j || r - j > 3 * BAND {
            return None;
        }
        Some((r - j) * self.m + j)
    }

    fn add(&mut self, i: usize, j: usize, v: f64) {
        match self.index(i, j) {
            Some(k) => self.data[k] += v,
            None => debug_assert!(false, "Jacobian entry ({i}, {j}) is outside the band"),
        }
    }

    fn get(&self, i: usize, j: usize) -> f64 {
        self.index(i, j).map_or(0.0, |k| self.data[k])
    }

    fn set(&mut self, i: usize, j: usize, v: f64) {
        if let Some(k) = self.index(i, j) {
            self.data[k] = v;
        }
    }

    fn zero_row(&mut self, i: usize) {
        let lo = i.saturating_sub(BAND);
        for j in lo..(i + 2 * BAND + 1).min(self.m) {
            self.set(i, j, 0.0);
        }
    }

    /// Band LU with partial pivoting. `false` on a singular pivot, which the caller treats
    /// as a failed iteration rather than an error — `step` may not fail.
    fn factor(&mut self) -> bool {
        let m = self.m;
        for j in 0..m {
            let last_row = (j + BAND).min(m - 1);
            let mut p = j;
            let mut best = self.get(j, j).abs();
            for i in (j + 1)..=last_row {
                let v = self.get(i, j).abs();
                if v > best {
                    best = v;
                    p = i;
                }
            }
            if best == 0.0 || !best.is_finite() {
                return false;
            }
            self.piv[j] = p;
            if p != j {
                for jj in j..=(j + 2 * BAND).min(m - 1) {
                    let a = self.get(j, jj);
                    let b = self.get(p, jj);
                    self.set(j, jj, b);
                    self.set(p, jj, a);
                }
            }
            let pivot = self.get(j, j);
            for i in (j + 1)..=last_row {
                let f = self.get(i, j) / pivot;
                if f == 0.0 {
                    continue;
                }
                self.set(i, j, f);
                for jj in (j + 1)..=(j + 2 * BAND).min(m - 1) {
                    let v = self.get(i, jj) - f * self.get(j, jj);
                    self.set(i, jj, v);
                }
            }
        }
        true
    }

    /// Solve `A·x = b` in place, using the factorisation [`Self::factor`] left behind.
    fn solve(&self, b: &mut [f64]) {
        let m = self.m;
        for j in 0..m {
            let p = self.piv[j];
            if p != j {
                b.swap(j, p);
            }
            let last_row = (j + BAND).min(m - 1);
            for i in (j + 1)..=last_row {
                let f = self.get(i, j);
                b[i] -= f * b[j];
            }
        }
        for j in (0..m).rev() {
            b[j] /= self.get(j, j);
            let lo = j.saturating_sub(2 * BAND);
            for i in lo..j {
                b[i] -= self.get(i, j) * b[j];
            }
        }
    }
}

/// Row scaling for the convergence norm: a representative magnitude of each row's own
/// terms. See [`NEWTON_TOL`] for why an unscaled norm would be wrong.
fn row_scale(grid: &Grid, c_e0: f64, i_app: f64, dt: f64) -> Vec<f64> {
    let n = grid.n();
    let i_ref = i_app.abs().max(1.0);
    let mut s = vec![1.0; NVAR * n];
    for i in 0..n {
        s[NVAR * i + CE] = (grid.eps_e[i] * grid.h[i] * c_e0 / dt).max(1.0e-12);
        s[NVAR * i + PHIE] = i_ref;
        s[NVAR * i + PHIS] = i_ref;
        s[NVAR * i + JJ] = i_ref;
    }
    s
}

/// What one cell solve produced.
struct Solved {
    /// Converged (or last) unknown vector.
    u: Vec<f64>,
    /// Terminal voltage \[V\] the solve settled on, contact resistance included.
    v_terminal: f64,
    /// `−dV/di` \[ohms\] at the solved operating point, from the sensitivity solve.
    r_tangent: f64,
    /// Whether [`NEWTON_TOL`] was actually met.
    converged: bool,
}

/// Everything one step's solve needs that is built before it starts: the resolved
/// chemistry, the grid those two imply, and the particle maps, which depend on `dt` but
/// not on the current and so are built once per step rather than once per iteration.
struct StepSetup<'a> {
    sides: &'a Sides<'a>,
    grid: &'a Grid,
    dfn: &'a DfnParams,
    parts_neg: Vec<Particle>,
    parts_pos: Vec<Particle>,
    /// Nominal electrolyte concentration \[mol/m³\], for the residual's row scaling.
    c_e0: f64,
}

/// Assemble the per-step, per-`dt` half of a solve's inputs from the cell's state.
///
/// Split out of [`advance`] so that [`probe_at`] builds *exactly* the same setup: a probe
/// at the current the pack finally assigns has to be the same computation `advance` then
/// runs, or the argument that a probe needs no flag channel of its own (see [`probe_at`])
/// would not hold. The caller still builds `sides` and `grid` itself, because it has to
/// check the grid against the state before there is anything to set up.
fn setup_for<'a>(
    s: &DfnState,
    spm: &SpmParams,
    dfn: &'a DfnParams,
    sides: &'a Sides<'a>,
    grid: &'a Grid,
    dt: f64,
) -> StepSetup<'a> {
    StepSetup {
        sides,
        grid,
        dfn,
        parts_neg: s
            .c_neg
            .iter()
            .map(|c| particle_map(c, &spm.negative, sides.neg.d_s, sides.kappa, dt))
            .collect(),
        parts_pos: s
            .c_pos
            .iter()
            .map(|c| particle_map(c, &spm.positive, sides.pos.d_s, sides.kappa, dt))
            .collect(),
        c_e0: spm.c_e_mol_per_m3,
    }
}

/// Solve one step of the coupled system at applied current `i` \[A, discharge-positive\].
///
/// Fixed-step backward Euler, damped Newton, analytic banded Jacobian. Never panics and
/// never returns an error: a step that runs out of iterations says so in
/// [`Solved::converged`] and the caller raises a flag.
fn solve(s: &DfnState, setup: &StepSetup<'_>, i: f64, dt: f64) -> Solved {
    let grid = setup.grid;
    let sides = setup.sides;
    let n = grid.n();
    let m = NVAR * n;
    let i_app = i / sides.area_m2;
    let sys = System {
        grid,
        dfn: setup.dfn,
        sides,
        parts_neg: &setup.parts_neg,
        parts_pos: &setup.parts_pos,
        c_e_old: &s.c_e,
        dt,
        i_app,
        kd_coef: kd_coef(setup.dfn, sides.temp_k),
    };

    // Start from the previous step's converged vector, but re-seed the concentration slots
    // from the stored field: they are the same quantity, and a step that ended badly must
    // not be able to poison the guess with a `c_e` the state does not hold.
    let mut u = if s.u.len() == m {
        s.u.clone()
    } else {
        vec![0.0; m]
    };
    for (idx, &c) in s.c_e.iter().enumerate() {
        u[NVAR * idx + CE] = c;
    }

    let scale = row_scale(grid, setup.c_e0, i_app, dt);
    let norm = |r: &[f64]| {
        r.iter()
            .zip(&scale)
            .map(|(a, sc)| (a / sc).abs())
            .fold(
                0.0_f64,
                |acc, v| if v > acc || v.is_nan() { v } else { acc },
            )
    };

    let mut tr: Vec<Transport> = Vec::with_capacity(n);
    let mut r = vec![0.0; m];
    let mut rp = vec![0.0; m];
    let mut du = vec![0.0; m];
    let mut trial = vec![0.0; m];
    let mut band = Band::new(m);
    let mut converged = false;

    // The residual and its norm are carried across the loop boundary rather than
    // recomputed at the top of each pass: the damping search below has already evaluated
    // both at the point it accepted, and re-deriving them would double this solve's
    // residual evaluations — which are the dominant cost once the Jacobian is analytic.
    sys.transports(&u, &mut tr);
    sys.residual(&u, &tr, &mut r);
    let mut resid = norm(&r);

    for _ in 0..NEWTON_ITER_CAP {
        if resid < NEWTON_TOL {
            converged = true;
            break;
        }
        if !resid.is_finite() {
            break;
        }
        sys.jacobian(&u, &tr, &mut band);
        if !band.factor() {
            break;
        }
        for (d, v) in du.iter_mut().zip(r.iter()) {
            *d = -*v;
        }
        band.solve(&mut du);
        // Damping: accept the full Newton step only if it reduces the scaled residual, and
        // halve until it does. The electrolyte's `sinh` kinetics make a full step overshoot
        // badly at high rate, which is where the spike's unconverged steps came from
        // before this was added.
        //
        // The halving happens at the *top* of each attempt rather than the bottom, which
        // is what keeps `trial`, `tr`, `rp` and `nr` describing the same point when the
        // search gives up: a search that halved on the way out would leave the carried
        // residual belonging to a step twice the size of the one taken, and every
        // subsequent convergence verdict would be about a point the solve is not at.
        let mut lambda = 1.0;
        let mut nr = f64::INFINITY;
        for attempt in 0..DAMPING_ATTEMPTS {
            if attempt > 0 {
                lambda *= 0.5;
            }
            for k in 0..m {
                trial[k] = u[k] + lambda * du[k];
            }
            sys.transports(&trial, &mut tr);
            sys.residual(&trial, &tr, &mut rp);
            nr = norm(&rp);
            if nr.is_finite() && nr < resid {
                break;
            }
        }
        // Whether or not the search succeeded, the smallest step it tried is taken: the
        // next iteration's residual is what decides whether that was recoverable.
        u.copy_from_slice(&trial);
        r.copy_from_slice(&rp);
        resid = nr;
    }

    // --- the tangent, off the Jacobian at the *converged* point.
    //
    // `∂R/∂i_app` is nonzero only where the applied current enters, which is the solid
    // charge row at each current collector: `−1` at the negative one (where it is the
    // inbound face) and `+1` at the positive. So `du/di_app = −J⁻¹·∂R/∂i_app` is one
    // back-substitution against a factorisation this solve has already paid for, and
    // `dV/di` reads straight off it — exact to the discretisation rather than a difference
    // quotient, and a small fraction of one residual evaluation.
    //
    // The Jacobian is re-assembled here rather than reusing the last iteration's, which
    // was taken one Newton step short of the answer. That costs one assembly and one
    // factorisation, and buys a tangent that is actually tangent to the curve at the point
    // reported.
    sys.transports(&u, &mut tr);
    sys.jacobian(&u, &tr, &mut band);
    let mut dv_di = 0.0;
    if band.factor() {
        let mut rhs = vec![0.0; m];
        rhs[PHIS] = 1.0;
        rhs[NVAR * (n - 1) + PHIS] = -1.0;
        band.solve(&mut rhs);
        // V = φ_s(last) − φ_s(0), and `i_app = i/area`.
        dv_di = (rhs[NVAR * (n - 1) + PHIS] - rhs[PHIS]) / sides.area_m2;
    }
    let v_solid = u[NVAR * (n - 1) + PHIS] - u[PHIS];
    // `V` is strictly decreasing in `i` on any state this model can reach, so `r > 0`. The
    // floor is not physics: the pack solve divides by `r`, and this is the guarantee it
    // needs held up on a state driven somewhere pathological, so that `step` reports a bad
    // number rather than panicking. Same constant and same argument as `spm::source_at`.
    const R_FLOOR_OHMS: f64 = 1.0e-9;
    let r_tangent = -dv_di + sides.r_contact;
    let r_tangent = if r_tangent.is_finite() && r_tangent > R_FLOOR_OHMS {
        r_tangent
    } else {
        R_FLOOR_OHMS
    };
    Solved {
        u,
        v_terminal: v_solid - i * sides.r_contact,
        r_tangent,
        converged,
    }
}

/// Mean stoichiometry of one electrode's particles across x, each particle's own profile
/// already volume-averaged over r by [`mean_concentration`].
///
/// # A plain mean is the exact volume weighting here, and only here
/// [`Grid::of`] gives every node in a region the same width, so weighting by node width
/// would multiply and divide by one constant. This takes the mean directly rather than
/// carrying a weight vector that is always uniform: a signature that promises weighting
/// its callers do not supply reads as working, and it would allocate a throwaway `Vec` on
/// a path the pack walks twice per cell per step. **A future non-uniform grid has to change
/// this function**, which is the trade, and is why the constraint is written down here
/// rather than left in `Grid`.
fn bulk_stoich(profiles: &[Vec<f64>], c_max: f64) -> f64 {
    if profiles.is_empty() {
        return 0.0;
    }
    let sum: f64 = profiles.iter().map(|p| mean_concentration(p)).sum();
    sum / profiles.len() as f64 / c_max
}

/// Ground-truth state of charge, in \[0, 1\]: the negative electrode's mean stoichiometry
/// mapped onto its usable window.
///
/// A readout rather than a counter, exactly as [`crate::spm::soc`] is — read that doc for
/// what that costs and why it is still the right meaning. The only difference here is that
/// the mean runs over x as well as r, because a DFN has a lithium distribution through the
/// electrode and not just through a particle.
#[must_use]
pub(crate) fn soc(s: &DfnState, spm: &SpmParams) -> f64 {
    raw_soc(s, spm).clamp(0.0, 1.0)
}

/// [`soc`] before the clamp — the value that says whether a limit was passed.
#[must_use]
fn raw_soc(s: &DfnState, spm: &SpmParams) -> f64 {
    let e = &spm.negative;
    let x = bulk_stoich(&s.c_neg, e.c_max_mol_per_m3);
    (x - e.stoich_min) / (e.stoich_max - e.stoich_min)
}

/// Equilibrium (open-circuit) voltage \[V\] at the particles' **bulk** stoichiometry,
/// averaged across x.
///
/// Deliberately not the surface value, and deliberately not a function of the electrolyte:
/// this is the potential the cell's stored chemical energy is measured against, and every
/// gradient the load created is an overpotential the cell gets back on relaxation.
#[must_use]
fn equilibrium_voltage(s: &DfnState, spm: &SpmParams) -> f64 {
    let x = bulk_stoich(&s.c_neg, spm.negative.c_max_mol_per_m3);
    let y = bulk_stoich(&s.c_pos, spm.positive.c_max_mol_per_m3);
    ocp_lookup(&spm.positive.ocp, y) - ocp_lookup(&spm.negative.ocp, x)
}

/// A first-order Thévenin resistance \[ohms\] for a cell that has never been solved.
///
/// Used on exactly one step — the first — and only where the pack needs an `R` before this
/// cell has a real one. Under `Demand::Current` at any topology the assigned current does
/// not depend on it at all (see [`crate::ecm::solve_current`]); what it does reach is the
/// reported terminal voltage of that first step, a `Power`/`Voltage` demand's solve, and
/// the current split in a multi-cell pack.
///
/// The estimate is the textbook porous-electrode decomposition at the cell's initial
/// uniform state: two linearized charge-transfer resistances `R·T/(α·F·i₀·A_int)`, the
/// electrolyte's ohmic path with the standard `L/3` effective length in each electrode and
/// the full `L` across the separator, the solid phase's `L/3` in each electrode, and the
/// contact resistance. It is an *estimate* and is labelled one; the cell replaces it with a
/// measured tangent the moment it takes a step.
#[must_use]
fn seed_resistance(s: &DfnState, spm: &SpmParams, dfn: &DfnParams, sides: &Sides<'_>) -> f64 {
    let mut r = sides.r_contact;
    let area = sides.area_m2;
    let rt_over_f = GAS_CONSTANT_J_PER_MOL_K * sides.temp_k / FARADAY_C_PER_MOL;
    let ce = spm.c_e_mol_per_m3;
    let bulk = |profiles: &[Vec<f64>], c_max: f64| bulk_stoich(profiles, c_max) * c_max;
    for (side, dfn_e, profiles) in [
        (&sides.neg, &dfn.negative, &s.c_neg),
        (&sides.pos, &dfn.positive, &s.c_pos),
    ] {
        let c_max = side.p.c_max_mol_per_m3;
        let cs = bulk(profiles, c_max).clamp(1.0e-6 * c_max, (1.0 - 1.0e-6) * c_max);
        let i0 = side.m_ref * (ce * cs * (c_max - cs)).sqrt();
        let a_int = side.a_s * side.p.thickness_m * area;
        if i0 > 0.0 && a_int > 0.0 {
            r += rt_over_f / (side.p.charge_transfer_alpha * i0 * a_int);
        }
        // Electrolyte and solid ohmic paths, both with the uniform-reaction `L/3`.
        let kappa_e = eval_terms(&dfn.electrolyte_conductivity_terms, ce / 1000.0)
            * powx(dfn_e.porosity, dfn_e.bruggeman_electrolyte);
        if kappa_e > 0.0 {
            r += side.p.thickness_m / (3.0 * kappa_e * area);
        }
        if side.sigma_eff > 0.0 {
            r += side.p.thickness_m / (3.0 * side.sigma_eff * area);
        }
    }
    let kappa_sep = eval_terms(&dfn.electrolyte_conductivity_terms, ce / 1000.0)
        * powx(dfn.separator.porosity, dfn.separator.bruggeman_electrolyte);
    if kappa_sep > 0.0 {
        r += dfn.separator.thickness_m / (kappa_sep * area);
    }
    r
}

/// This cell's Thévenin line `(E, R)` for the pack's solve: the tangent its own last solve
/// produced, or a seed if it has never taken one.
///
/// # This is the *start* of the pack's iteration, not its answer
/// The line stored in [`DfnState::tangent`] is tangent to the curve at the current the cell
/// carried over the previous step, so it is where the pack's first pass linearizes and
/// nothing more. [`probe_at`] is what the iteration then measures that pass against, and it
/// re-solves. This function stays a pure function of state so that `SourceCache` can
/// memoise it; the probe deliberately is not one.
///
/// # Purity
/// A pure function of cell state and the two scale factors, which is what lets
/// [`crate::pack`]'s `SourceCache` memoise it — the tangent is *state*, exactly as
/// [`crate::SpmState::i_last`] is.
#[must_use]
pub(crate) fn source(
    s: &DfnState,
    spm: &SpmParams,
    dfn: &DfnParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> (f64, f64) {
    if let Some(line) = s.tangent {
        return line;
    }
    let sides = Sides::new(spm, dfn, s.temp_k, eff_r0_factor, eff_capacity_ah);
    (
        equilibrium_voltage(s, spm),
        seed_resistance(s, spm, dfn, &sides),
    )
}

/// The stored line [`source`] returns, evaluated at `i` \[A, discharge-positive\].
///
/// **Not the curve** — [`probe_at`] is the curve, and it costs a solve. This is the cheap
/// readout, used where a caller wants the cell's own claim about itself without asking it
/// to integrate a step: [`overpotential_v`] reads it at [`DfnState::i_last`], which is the
/// current the state it is decomposing was produced by.
#[must_use]
pub(crate) fn terminal_v(
    s: &DfnState,
    spm: &SpmParams,
    dfn: &DfnParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
) -> f64 {
    let (e, r) = source(s, spm, dfn, eff_r0_factor, eff_capacity_ah);
    e - i * r
}

/// Evaluate this cell's real `V(i)` over a step of length `dt`, and take the tangent there:
/// returns `(V, (E, R))` with `E = V + i·R` so the line passes through the point solved.
///
/// This is the whole of Phase 7 slice C. The pack's nonlinear iteration measures its
/// aggregate against each cell's curve and re-linearizes there; until this existed a DFN
/// answered with the line [`source`] had already handed over, so the residual was zero by
/// construction and the iteration exited on its first pass with nothing to chase.
///
/// # It costs a full solve, and that is the honest price
/// For an SPM, `V(i)` is a handful of table lookups and [`crate::spm::source_at`] takes its
/// tangent by central difference — three evaluations. **For a DFN, `V(i)` is a coupled
/// nonlinear solve**, so the same contract would have cost three of them per cell per pass.
/// It costs one instead: the tangent comes off the *same* factorised Jacobian as a
/// sensitivity solve (see [`solve`]), which is one back-substitution rather than a second
/// and third solve. That is why `(V, (E, R))` is returned together rather than through two
/// calls — two calls at the same current would be two solves, and the pack asks for both.
///
/// # Why no flag channel
/// A probe's own Newton can fail exactly as [`advance`]'s can, and this returns no
/// [`EventFlags`]. It does not need to: the pack probes the converged pass at the same
/// current it then hands `advance` — bit-for-bit, which `pack::step` pins with a
/// `debug_assert` — and this function and `advance` build the same [`setup_for`] from the
/// same state, so the failing solve is the one `advance` raises the flag for. Probes on
/// *intermediate* iterates are deliberately not reported, on the same reasoning that keeps
/// the pack's protection flags a per-pass binding rather than an accumulator: an
/// intermediate operating point the converged answer does not visit did not happen.
///
/// # `dt <= 0` does not reach the solver
/// A zero-length probe step is how this repo reads an instantaneous voltage, and the
/// backward-Euler mass rows carry `(c − c_old)/dt`. So a non-positive or `NaN` `dt` — which
/// [`advance`] already refuses, without ever having had to survive it below — answers with
/// the stored line instead. That is the right answer as well as the safe one: no time
/// passes, so there is no transient to solve, and the cell reports what its last real solve
/// concluded. The `NaN` test is spelled out rather than folded into a negated comparison,
/// which is [`advance`]'s spelling and the one clippy accepts.
#[must_use]
pub(crate) fn probe_at(
    s: &DfnState,
    spm: &SpmParams,
    dfn: &DfnParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
    dt: f64,
) -> (f64, (f64, f64)) {
    let stored = |s: &DfnState| {
        let line = source(s, spm, dfn, eff_r0_factor, eff_capacity_ah);
        (line.0 - i * line.1, line)
    };
    if dt.is_nan() || dt <= 0.0 {
        return stored(s);
    }
    let sides = Sides::new(spm, dfn, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let grid = Grid::of(spm, dfn, &sides, s);
    if grid.n() == 0 || grid.n() != s.c_e.len() {
        // A grid that does not describe the state it came from, reachable only from a
        // hand-edited snapshot. `advance` does nothing on it rather than indexing off the
        // end; the matching answer here is the line, not a solve over a bad grid.
        return stored(s);
    }
    let setup = setup_for(s, spm, dfn, &sides, &grid, dt);
    let solved = solve(s, &setup, i, dt);
    (
        solved.v_terminal,
        (solved.v_terminal + i * solved.r_tangent, solved.r_tangent),
    )
}

/// Total overpotential \[V\], discharge-positive: everything between the equilibrium
/// voltage at bulk stoichiometry and the terminal that is **not** the instantaneous ohmic
/// drop.
///
/// For this model that is the concentration overpotential in both solid phases *and* in
/// the electrolyte, plus the two Butler–Volmer overpotentials, plus the electrolyte's own
/// ohmic drop — which on the shipped set at 3C is 668 mV of it. Evaluated at
/// [`DfnState::i_last`], the current the state it reads was produced by.
#[must_use]
pub(crate) fn overpotential_v(
    s: &DfnState,
    spm: &SpmParams,
    dfn: &DfnParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> f64 {
    let i = s.i_last;
    let v = terminal_v(s, spm, dfn, eff_r0_factor, eff_capacity_ah, i);
    let sides = Sides::new(spm, dfn, s.temp_k, eff_r0_factor, eff_capacity_ah);
    equilibrium_voltage(s, spm) - v - i * sides.r_contact
}

/// Heat generated inside this cell \[W\] at current `i`, given the terminal voltage
/// `v_terminal` the pack's solve produced for it.
///
/// Same two terms and the same general form as [`crate::spm::heat_w`]: irreversible
/// `I·(U_eq − V)`, which is the whole overpotential heat with no `I²R0 + I·ΣV_rc`
/// decomposition available, plus the entropic `−I·T·∂U/∂T` assembled from the two
/// half-cell coefficients. Taking `v_terminal` from the pack rather than re-deriving it
/// is what keeps the energy balance answering for the voltage the terminals delivered.
#[must_use]
pub(crate) fn heat_w(s: &DfnState, spm: &SpmParams, i: f64, v_terminal: f64) -> f64 {
    let q_irrev = i * (equilibrium_voltage(s, spm) - v_terminal);
    let docv_dt = spm.positive.docp_dt_v_per_k - spm.negative.docp_dt_v_per_k;
    q_irrev - i * s.temp_k * docv_dt
}

/// Advance the whole cell by `dt` seconds under the current `i` the pack assigned it, and
/// record the tangent the solve produced.
///
/// Returns the SOC-clamp flags on [`crate::ecm::coulomb_step`]'s contract, plus
/// [`EventFlags::SOLVE_UNCONVERGED`] if this cell's own Newton hit [`NEWTON_ITER_CAP`]
/// without meeting [`NEWTON_TOL`]. No concentration is ever clamped: an overcharged
/// particle keeps the lithium it was pushed, so the flag says the *readout* left its
/// window rather than that state was discarded.
///
/// A zero-length probe step mutates nothing at all — no solve, no tangent, no `i_last`.
/// That is the same contract [`crate::spm::advance`] holds, and the suite is full of probe
/// steps that depend on it.
#[must_use]
pub(crate) fn advance(
    s: &mut DfnState,
    spm: &SpmParams,
    dfn: &DfnParams,
    i: f64,
    dt: f64,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> EventFlags {
    let mut flags = EventFlags::empty();
    if dt.is_nan() || dt <= 0.0 {
        return flags | soc_flags(s, spm);
    }
    let sides = Sides::new(spm, dfn, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let grid = Grid::of(spm, dfn, &sides, s);
    if grid.n() == 0 || grid.n() != s.c_e.len() {
        // Only reachable from a hand-edited snapshot; `step` may not panic, so a grid that
        // does not describe the state it came from does nothing rather than indexing off
        // the end of it.
        return flags | soc_flags(s, spm);
    }
    let setup = setup_for(s, spm, dfn, &sides, &grid, dt);

    let solved = solve(s, &setup, i, dt);
    if !solved.converged {
        flags |= EventFlags::SOLVE_UNCONVERGED;
    }

    // Commit: the electrolyte straight off the solved field, the particles reconstructed by
    // the same linearity the affine map was built on.
    for (idx, c) in s.c_e.iter_mut().enumerate() {
        *c = solved.u[NVAR * idx + CE];
    }
    for (idx, prof) in s.c_neg.iter_mut().enumerate() {
        let j = solved.u[NVAR * idx + JJ];
        let p = &setup.parts_neg[idx];
        for (k, v) in prof.iter_mut().enumerate() {
            *v = p.prof0[k] + j * (p.prof1[k] - p.prof0[k]);
        }
    }
    for (idx, prof) in s.c_pos.iter_mut().enumerate() {
        let j = solved.u[NVAR * (grid.first_pos() + idx) + JJ];
        let p = &setup.parts_pos[idx];
        for (k, v) in prof.iter_mut().enumerate() {
            *v = p.prof0[k] + j * (p.prof1[k] - p.prof0[k]);
        }
    }
    s.u = solved.u;
    s.i_last = i;
    s.tangent = Some((solved.v_terminal + i * solved.r_tangent, solved.r_tangent));
    flags | soc_flags(s, spm)
}

/// The SOC-window flags for the state as it stands.
fn soc_flags(s: &DfnState, spm: &SpmParams) -> EventFlags {
    let raw = raw_soc(s, spm);
    if raw > 1.0 {
        EventFlags::SOC_CLAMPED_HIGH
    } else if raw < 0.0 {
        EventFlags::SOC_CLAMPED_LOW
    } else {
        EventFlags::empty()
    }
}

/// Test-only window onto one solve, so that the analytic Jacobian can be checked against a
/// difference quotient of the residual it claims to differentiate.
///
/// Not part of the model: nothing in `step` calls it. It exists because an analytic
/// Jacobian is the one piece of this file that can be *silently* wrong — a bad entry costs
/// Newton iterations and, past the damping, nothing else — so it needs a check that reads
/// the same two functions the solve does.
#[doc(hidden)]
pub mod probe {
    use super::{
        particle_map, Band, DfnParams, DfnState, Grid, Particle, Sides, SpmParams, System,
        Transport, NVAR,
    };

    /// Assemble the residual and the analytic Jacobian at one state, and return the
    /// Jacobian together with a central-difference approximation of it.
    ///
    /// Both are dense `m × m` in row-major order. `h_rel` scales the perturbation.
    #[must_use]
    pub fn jacobian_pair(
        s: &DfnState,
        spm: &SpmParams,
        dfn: &DfnParams,
        i: f64,
        dt: f64,
        h_rel: f64,
    ) -> (usize, Vec<f64>, Vec<f64>) {
        let sides = Sides::new(spm, dfn, s.temp_k, 1.0, spm_capacity(spm));
        let grid = Grid::of(spm, dfn, &sides, s);
        let parts_neg: Vec<Particle> = s
            .c_neg
            .iter()
            .map(|c| particle_map(c, &spm.negative, sides.neg.d_s, sides.kappa, dt))
            .collect();
        let parts_pos: Vec<Particle> = s
            .c_pos
            .iter()
            .map(|c| particle_map(c, &spm.positive, sides.pos.d_s, sides.kappa, dt))
            .collect();
        let sys = System {
            grid: &grid,
            dfn,
            sides: &sides,
            parts_neg: &parts_neg,
            parts_pos: &parts_pos,
            c_e_old: &s.c_e,
            dt,
            i_app: i / sides.area_m2,
            kd_coef: super::kd_coef(dfn, sides.temp_k),
        };
        let m = NVAR * grid.n();
        let mut u = s.u.clone();
        u.resize(m, 0.0);
        for (idx, &c) in s.c_e.iter().enumerate() {
            u[NVAR * idx + super::CE] = c;
        }
        let mut tr: Vec<Transport> = Vec::new();
        let mut band = Band::new(m);
        sys.transports(&u, &mut tr);
        sys.jacobian(&u, &tr, &mut band);
        let mut analytic = vec![0.0; m * m];
        for row in 0..m {
            for col in 0..m {
                analytic[row * m + col] = band.get(row, col);
            }
        }
        let mut numeric = vec![0.0; m * m];
        let mut rp = vec![0.0; m];
        let mut rm = vec![0.0; m];
        let mut probe = u.clone();
        for col in 0..m {
            let orig = u[col];
            let h = h_rel * orig.abs().max(1.0);
            probe[col] = orig + h;
            sys.transports(&probe, &mut tr);
            sys.residual(&probe, &tr, &mut rp);
            probe[col] = orig - h;
            sys.transports(&probe, &mut tr);
            sys.residual(&probe, &tr, &mut rm);
            probe[col] = orig;
            for row in 0..m {
                numeric[row * m + col] = (rp[row] - rm[row]) / (2.0 * h);
            }
        }
        (m, analytic, numeric)
    }

    /// The geometric capacity of the negative electrode \[A·h\], i.e. the capacity at which
    /// the model's `κ` is exactly 1.
    #[must_use]
    pub fn spm_capacity(spm: &SpmParams) -> f64 {
        super::geometric_capacity_ah(&spm.negative, super::Geometry::of(spm, &spm.negative))
    }
}

/// Unknowns this model carries per x-node: `(c_e, φ_e, φ_s, j)`.
///
/// Public so that a caller sizing or inspecting [`DfnState::u`] — a test, or an adapter
/// reporting snapshot size — reads the layout constant rather than duplicating a `4`.
pub const UNKNOWNS_PER_NODE: usize = NVAR;
