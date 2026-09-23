//! Single-particle cell model (`Spm`) — the first porous-electrode model.
//!
//! One representative spherical particle per electrode, each discretised into
//! radial finite-volume shells, coupled by Butler–Volmer kinetics at their
//! surfaces and by an electrolyte held at a constant concentration. That last
//! assumption *is* the single-particle approximation; relaxing it is `Dfn`.
//!
//! # Sign convention
//! Positive current = **discharge** (out of the terminals), as everywhere else in
//! this crate. On discharge lithium leaves the negative particle and enters the
//! positive one.
//!
//! # Why the integrator is owned rather than imported
//! Radial diffusion is solved with **backward Euler on a finite-volume grid** —
//! one tridiagonal (Thomas) solve per particle per step. Two consequences, both
//! deliberate:
//!
//! * It is **unconditionally stable**, so a months-long aging fast-forward at
//!   `dt = 3600 s` needs no sub-stepping. Explicit Euler would be stable only to
//!   `dt ≈ dr²/(2·D)`, which for the shipped chemistry is ~12 s at 10 shells —
//!   fine for real time and useless for fast-forward, which is exactly the trap
//!   `CLAUDE.md`'s "same code path serves real-time stepping and months-long aging"
//!   warns about.
//! * The **entire integrator state is the concentration vector**, so
//!   snapshot/restore is bit-identical for free. An adaptive multistep solver's
//!   state is larger than its solution vector; `diffsol` was measured against this
//!   requirement and declined because the remainder is not reachable through its
//!   public API. See `docs/plans/phase-6-porous-electrodes.md`.
//!
//! # Cell vs. pack responsibilities
//! Same split as [`crate::ecm`]: [`source`] hands the pack a linear Thévenin for
//! its solve and [`advance`] moves the state under the current the pack assigned.
//! The difference is that an SPM's `V(i)` is *nonlinear within a step* — through
//! both Butler–Volmer overpotentials and through the surface concentrations, which
//! the flux boundary condition shifts — so the source is a **tangent** rather than
//! an exact equivalent. Phase 6 slice C2 lets the pack solve that tangent once, at
//! the previous step's current; slice D iterates it. The curve the iteration solves on
//! is the **end-of-step** one — the particles diffused over the step under the current
//! being tried — which is what keeps a parallel group and a voltage hold stable at long
//! steps. See [`probe_at`] and `docs/plans/spm-end-of-step.md`.

use serde::{Deserialize, Serialize};

use crate::aging::GAS_CONSTANT_J_PER_MOL_K;
use crate::chem::{ElectrodeParams, OcpTable, SpmParams};
use crate::ecm::interp1;
use crate::flags::EventFlags;

/// Faraday constant \[C/mol\].
///
/// provenance: `N_A · e` = `6.022_140_76e23 × 1.602_176_634e-19`, both exact by the
/// 2019 SI definitions, so this is a **defined** quantity and the digits below are
/// the exact product rather than a measurement with an uncertainty.
///
/// Spelled to the shortest decimal that round-trips to that product's nearest `f64`
/// (clippy rejects the exact `…_018_4` as excessive precision, and it is right —
/// the extra digits name the same value). Truncating further, to the commonly quoted
/// `96485.33212331`, is **not** a rounding of no consequence: that is a *different*
/// `f64`, one ULP away, and every SPM trajectory moves with it. This slice's
/// assertions are all tolerances and every one of them stayed green through exactly
/// that mistake, which is why `faraday_is_the_exact_defined_product` pins the value
/// rather than trusting the literal to stay put.
pub const FARADAY_C_PER_MOL: f64 = 96_485.332_123_310_01;

/// Largest shell count a particle may be discretised into.
///
/// Not a physical limit — it is the length of the one stack-allocated scratch row
/// the tridiagonal solve needs (see [`diffuse`]). A fixed row keeps the step
/// allocation-free, which matters because it runs per particle per cell per step;
/// the cost is that the ceiling is a constant rather than config. 64 shells is far
/// past the point where refining the grid changes an answer (slice E measures the
/// accuracy-vs-cost curve), so this bounds nothing anyone wants.
pub const MAX_SHELLS: usize = 64;

/// Smallest shell count a particle may be discretised into.
///
/// Two rather than one because a single shell has no interior gradient to resolve,
/// which is the entire reason this model exists — a 1-shell "SPM" is a coulomb
/// counter wearing Butler–Volmer kinetics, and shipping it as a porous-electrode
/// model would be the kind of quiet lie the `Spm` variant is meant to end.
pub const MIN_SHELLS: usize = 2;

/// The shell count to reach for when nothing about the scenario argues otherwise.
///
/// # Measured, not chosen
/// Slice E ran batsim against a grid- and time-converged PyBaMM SPM on the shipped
/// LG M50 parameter set (`crates/sim-data/tests/spm_golden.rs`) and swept `N`. Worst
/// terminal-voltage disagreement over a whole trajectory, in mV:
///
/// | scenario | N=5 | N=10 | **N=20** | N=40 |
/// | -------- | --- | ---- | -------- | ---- |
/// | C/5 CC   | 4.0 | 2.4  | **2.6**  | 3.4  |
/// | 1C CC    | 53.1| 24.1 | **6.7**  | 5.0  |
/// | GITT     | 30.6| 10.0 | **1.9**  | 3.0  |
///
/// The curve has a **floor at 2–3 mV**, and the floor is not the grid: it is the
/// chemistry's own OCP tables, whose piecewise-linear interpolation error the file
/// documents as 1.90 mV (graphite) and 1.88 mV (NMC). Past `N ≈ 20` a finer grid
/// resolves a radial profile more accurately and then reads it off a table that has
/// not improved, so the total does not fall — and can drift slightly up, as it does
/// at N=40. Below 20 the radial gradient is genuinely under-resolved and the 1C
/// column shows it.
///
/// Cost is linear in `N` (the diffusion solve is one Thomas sweep per particle), so
/// the spike's 0.0966 / 0.2151 / 0.4739 µs at N = 5 / 10 / 20 says a step costs what
/// its accuracy costs and nothing more. 20 is where the two curves cross.
///
/// This is a **recommendation, not a default that anything applies silently.**
/// `CellModelConfig::Spm { shells }` still requires the number, because a
/// discretisation knob that fills itself in is one a caller never has to think
/// about — and `MIN_SHELLS`/`MAX_SHELLS` bracket what it may be, not what it should
/// be.
pub const DEFAULT_SHELLS: usize = 20;

/// Per-cell single-particle-model state: two concentration profiles, a
/// temperature, and the current the tangent is taken at.
///
/// Opaque to the pack layer, exactly as [`crate::EcmState`] is — the pack reaches
/// everything it needs through [`crate::CellModel`].
///
/// # This is the whole integrator state
/// Concentrations are shell **averages** \[mol/m³\], innermost first, and the
/// finite-volume scheme is a one-step method, so these vectors plus `temp_k` are a
/// complete description of the cell. Nothing is cached, nothing is derived, and a
/// restored cell continues bit-identically. That is exit criterion 3, and it is
/// why the integrator is the one in this file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpmState {
    /// Negative-particle shell concentrations \[mol/m³\], innermost first.
    pub c_neg: Vec<f64>,
    /// Positive-particle shell concentrations \[mol/m³\], innermost first.
    pub c_pos: Vec<f64>,
    /// Cell temperature \[K\]. Advanced by [`crate::thermal`], exactly as the
    /// equivalent circuit's is — no cell model integrates its own temperature.
    pub temp_k: f64,
    /// Current \[A, discharge-positive\] this cell carried over the previous step.
    ///
    /// **State, not a cache.** It is the operating point the Thévenin tangent in
    /// [`source`] is taken at, so it decides the next step's trajectory and has to
    /// survive a snapshot like anything else. A cold cell starts at `0.0`, which is
    /// where a resting cell's tangent belongs.
    pub i_last: f64,
}

impl SpmState {
    /// A fresh cell at `soc`, its particles uniform at the concentration that
    /// state of charge implies.
    ///
    /// Uniform because a cell that has never carried current has no gradient — the
    /// same statement `EcmState`'s zeroed `v_rc` makes. The two stoichiometries run
    /// in *opposite* directions with SOC (see [`ElectrodeParams::stoich_min`]).
    pub(crate) fn new(spm: &SpmParams, shells: usize, soc: f64, temp_k: f64) -> Self {
        let x = spm.negative.stoich_min + soc * (spm.negative.stoich_max - spm.negative.stoich_min);
        let y = spm.positive.stoich_max - soc * (spm.positive.stoich_max - spm.positive.stoich_min);
        Self {
            c_neg: vec![x * spm.negative.c_max_mol_per_m3; shells],
            c_pos: vec![y * spm.positive.c_max_mol_per_m3; shells],
            temp_k,
            i_last: 0.0,
        }
    }
}

/// One electrode's derived geometry: what the chemistry stores as physical
/// dimensions, in the form the step actually uses.
///
/// Recomputed per call rather than stored. It is four multiplies from numbers that
/// are already in the chemistry, and caching it would put derived state inside the
/// snapshot — the hazard [`crate::pack`]'s `SourceCache` had to be reasoned about
/// carefully and the one `diffsol` failed on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    /// Total interfacial area \[m²\] the reaction current spreads over:
    /// `3·ε_s·A·L / R_p`, i.e. the surface area of every particle in the coating.
    area_m2: f64,
    /// Active-material volume \[m³\]: `ε_s·A·L`.
    volume_m3: f64,
}

impl Geometry {
    pub(crate) fn of(spm: &SpmParams, e: &ElectrodeParams) -> Self {
        let volume_m3 = e.active_volume_fraction * spm.electrode_area_m2 * e.thickness_m;
        Self {
            area_m2: 3.0 * volume_m3 / e.particle_radius_m,
            volume_m3,
        }
    }
}

/// The charge this electrode's geometry holds between its stoichiometry limits
/// \[Ah\], from first principles: `Δx · c_max · V_active · F`.
///
/// This is the *geometric* capacity, and it is what [`Working::kappa`] measures the
/// configured capacity against.
#[must_use]
pub(crate) fn geometric_capacity_ah(e: &ElectrodeParams, g: Geometry) -> f64 {
    (e.stoich_max - e.stoich_min) * e.c_max_mol_per_m3 * g.volume_m3 * FARADAY_C_PER_MOL / 3600.0
}

/// Arrhenius correction `exp(Ea/R · (1/T_ref − 1/T))`, the form the parameter sets
/// publish. `1.0` exactly at `T = T_ref`, and exactly `1.0` for `Ea = 0` without
/// evaluating an exponential — which is the shipped chemistry's diffusivity, whose
/// activation energy Chen2020 does not publish.
#[must_use]
fn arrhenius(ea_j_per_mol: f64, t_ref_k: f64, temp_k: f64) -> f64 {
    if ea_j_per_mol == 0.0 {
        return 1.0;
    }
    (ea_j_per_mol / GAS_CONSTANT_J_PER_MOL_K * (1.0 / t_ref_k - 1.0 / temp_k)).exp()
}

/// One electrode, resolved at this cell's temperature and health.
#[derive(Clone, Copy, Debug)]
struct Side<'a> {
    p: &'a ElectrodeParams,
    g: Geometry,
    /// Solid diffusivity \[m²/s\] at the cell's temperature.
    d_s: f64,
    /// Exchange-current amplitude at the cell's temperature, **after** the
    /// resistance-growth division (see [`Working::new`]).
    m_ref: f64,
}

/// Everything a step needs that is derived from the chemistry and the cell's scale
/// factors rather than stored. Built per call; see [`Geometry`] for why.
#[derive(Clone, Copy, Debug)]
struct Working<'a> {
    spm: &'a SpmParams,
    neg: Side<'a>,
    pos: Side<'a>,
    /// Charge-to-stoichiometry scale: geometric capacity ÷ effective capacity.
    ///
    /// # Where the two capacity multipliers land, and why here
    /// The pack composes `capacity_ah × capacity_factor × soh_capacity` and hands
    /// over the product; this factor is the *only* place it enters the model. A
    /// cell configured at exactly its geometric capacity has `κ = 1` and the
    /// physics is untouched — which is true of the shipped chemistry to within its
    /// own rounding, because `capacity_ah` there was derived from this geometry.
    ///
    /// Reading `κ` as "the current the particles actually see" is the honest
    /// picture: a cell holding 10 % fewer amp-hours than its geometry says has 10 %
    /// less active material, so the same terminal current is a 10 % larger flux at
    /// each particle surface. Manufacturing scatter, `Fault::WeakCell` and aging's
    /// `soh_capacity` all arrive here, so an SPM pack shows the same weak-cell
    /// physics a Phase 1 ECM pack does rather than silently ignoring the factors.
    ///
    /// # What it deliberately does not scale
    /// Not the interfacial area, and therefore not the Butler–Volmer current
    /// density: those stay geometric. A literal loss-of-active-material would
    /// shrink both, but then `soh_capacity` would inflate the linearized
    /// charge-transfer resistance too and `soh_resistance` would stop being
    /// *exactly* the factor [`crate::Telemetry::soh_resistance`] says it is. So
    /// this models LAM's inventory effect only; the whole resistance effect is
    /// carried by `soh_resistance` below. That, and the absence of electrode
    /// slippage, are the two honest costs of aging-as-a-multiplier.
    ///
    /// Derived from the **negative** electrode, which is also the one
    /// [`soc`] reads, so amp-hours between the limits come out as the configured
    /// effective capacity exactly. The positive electrode is scaled by the same
    /// factor and traverses its own window at whatever rate its own geometry gives
    /// — for a self-consistent parameter set that is the same number (Chen2020's
    /// two electrodes agree to 3e-5 relative).
    kappa: f64,
    /// Lumped ohmic resistance \[ohms\] after the resistance-growth multiplier.
    r_contact: f64,
    /// Cell temperature \[K\].
    temp_k: f64,
}

impl<'a> Working<'a> {
    /// Resolve the chemistry against one cell's temperature and scale factors.
    ///
    /// `eff_r0_factor` is the product the pack composes — static manufacturing
    /// scatter × aging's `soh_resistance` — and it lands in **two** places:
    ///
    /// * multiplying [`SpmParams::contact_resistance_ohm`], the obvious one; and
    /// * **dividing `m_ref`, and therefore `i_0`, on both electrodes.**
    ///
    /// The second is not polish. The shipped SPM chemistry has
    /// `contact_resistance_ohm = 0` (Chen2020's own value, not an omission), so the
    /// obvious implementation alone would fade capacity with *exactly zero*
    /// resistance growth — the one thing `CLAUDE.md` forbids outright — on the only
    /// SPM chemistry that ships. Dividing `i_0` needs no new constant, and it is
    /// exact rather than approximate: the linearized charge-transfer resistance
    /// `R_ct = R·T/(F·i_0·A)` is inversely proportional to `i_0`, so dividing `i_0`
    /// by the factor multiplies `R_ct` by it. Doing it on **both** electrodes
    /// rather than only the negative (the physical SEI story) is what makes that
    /// identity hold for the cell rather than for half of it.
    fn new(
        spm: &'a SpmParams,
        temp_k: f64,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
    ) -> Working<'a> {
        let side = |p: &'a ElectrodeParams| {
            let g = Geometry::of(spm, p);
            Side {
                p,
                g,
                d_s: p.diffusivity_m2_per_s
                    * arrhenius(p.diffusivity_ea_j_per_mol, spm.t_ref_k, temp_k),
                m_ref: p.m_ref * arrhenius(p.reaction_ea_j_per_mol, spm.t_ref_k, temp_k)
                    / eff_r0_factor,
            }
        };
        let neg = side(&spm.negative);
        Working {
            spm,
            kappa: geometric_capacity_ah(&spm.negative, neg.g) / eff_capacity_ah,
            neg,
            pos: side(&spm.positive),
            r_contact: spm.contact_resistance_ohm * eff_r0_factor,
            temp_k,
        }
    }

    /// Molar flux \[mol/(m²·s)\] leaving the **negative** particle's surface at cell
    /// current `i`. Positive on discharge: lithium leaves the negative electrode.
    fn j_neg(&self, i: f64) -> f64 {
        self.kappa * i / (self.neg.g.area_m2 * FARADAY_C_PER_MOL)
    }

    /// Molar flux \[mol/(m²·s)\] leaving the **positive** particle's surface.
    /// Negative on discharge: the positive electrode is being filled.
    fn j_pos(&self, i: f64) -> f64 {
        -self.kappa * i / (self.pos.g.area_m2 * FARADAY_C_PER_MOL)
    }
}

/// Open-circuit potential \[V\] of one electrode against lithium metal, by clamped
/// linear interpolation over its stoichiometry.
///
/// The table runs **downhill** — see [`OcpTable`] — so this is decreasing in
/// `stoich`. Clamping at the ends is why the shipped tables carry a margin past the
/// usable window: a particle's *surface* stoichiometry runs outside its bulk limits
/// under load, and a table that stopped at the limits would report a flat OCP
/// exactly where the physics is steepest.
#[must_use]
pub fn ocp_lookup(table: &OcpTable, stoich: f64) -> f64 {
    interp1(&table.stoich, &table.volts, stoich)
}

/// `∂U/∂stoich` \[V\] of [`ocp_lookup`]: the slope of the segment the lookup lands in.
///
/// Piecewise **constant**, because the lookup is piecewise linear — and exactly `0` past
/// either end, because the lookup clamps there. Both facts matter to whoever differentiates
/// a cell voltage: the derivative is discontinuous at every breakpoint and a difference
/// quotient straddling one measures neither adjacent segment.
///
/// Written against the same bracket [`ocp_lookup`] uses, so the two never disagree about
/// which segment a stoichiometry is in — including at a clamped end, where the bracket
/// answers `lo == hi` and this answers zero.
#[must_use]
pub fn ocp_slope(table: &OcpTable, stoich: f64) -> f64 {
    crate::ecm::interp1_slope(&table.stoich, &table.volts, stoich)
}

/// Volume-weighted mean concentration \[mol/m³\] of a particle's shells.
///
/// The finite-volume weights are the shell volumes `(r_hi³ − r_lo³)/3` with the
/// `4π` cancelled; this is the quantity the scheme conserves, so it is the one that
/// maps onto lithium inventory.
#[must_use]
pub fn mean_concentration(c: &[f64]) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, &ci) in c.iter().enumerate() {
        let r_lo = i as f64;
        let r_hi = (i + 1) as f64;
        // dr³ is a common factor of every weight, so it cancels and the shell
        // index does the work directly.
        let vol = r_hi * r_hi * r_hi - r_lo * r_lo * r_lo;
        num += ci * vol;
        den += vol;
    }
    num / den
}

/// Surface concentration \[mol/m³\], extrapolated a half-shell out from the
/// outermost shell average along the gradient the flux boundary condition imposes.
///
/// `j_surf` is the flux \[mol/(m²·s)\] *leaving* the surface, so a discharging
/// negative particle (positive flux) reads a surface **below** its outermost shell
/// average. That gap is the concentration overpotential, and it is what makes a
/// hard discharge end early — the surface hits the cut-off while the bulk still
/// holds lithium.
#[must_use]
pub fn c_surface(c: &[f64], r_p: f64, d_s: f64, j_surf: f64) -> f64 {
    surface_from_outer(c[c.len() - 1], c.len(), r_p, d_s, j_surf)
}

/// [`c_surface`] from the outer shell's value and the shell count alone — the only two
/// things it reads off the profile. [`probe_at`] needs it for an *end-of-step* outer
/// shell that exists as a number, not as a profile.
#[must_use]
fn surface_from_outer(c_outer: f64, shells: usize, r_p: f64, d_s: f64, j_surf: f64) -> f64 {
    let dr = r_p / shells as f64;
    c_outer - 0.5 * dr * j_surf / d_s
}

/// Clamp a surface concentration into the open interval the kinetics are defined
/// on, as a fraction of `c_max`.
///
/// `i_0 ∝ √(c_s·(c_max − c_s))` is real only inside `(0, c_max)`, and an
/// overdriven cell can put a *surface* concentration outside it for a step. The
/// state itself is never clamped — an overcharged particle keeps the lithium it was
/// given and has to give it back — so this guard is confined to the lookups, which
/// is what keeps `step` free of NaNs without quietly rewriting the physics.
#[must_use]
fn clamp_surface(c_s: f64, c_max: f64) -> f64 {
    c_s.clamp(SURFACE_EDGE * c_max, (1.0 - SURFACE_EDGE) * c_max)
}

/// How close to empty or full, as a fraction of `c_max`, a surface concentration may sit
/// before [`clamp_surface`] holds it there. Not physics: `i_0 ∝ √(c_s·(c_max − c_s))` is
/// real only strictly inside `(0, c_max)`, and this is the margin that keeps it so.
/// [`current_window`] reads the same number, so the range it declares is exactly the range
/// over which no clamp is acting.
const SURFACE_EDGE: f64 = 1.0e-6;

/// Butler–Volmer overpotential \[V\] driving current density `i_s` \[A/m²\] at an
/// electrode whose surface sits at `c_s`.
///
/// # The closed form, and where it is exact
/// Butler–Volmer is
/// `i_s = i_0·[exp(αFη/RT) − exp(−(1−α)Fη/RT)]`, which inverts in closed form only
/// for **symmetric kinetics** (`α = 0.5`), where it collapses to
/// `η = (R·T/(α·F))·asinh(i_s/(2·i_0))`. Both electrodes of every parameter set
/// shipped here are symmetric, so this is exact for them. For an asymmetric set the
/// expression above is evaluated with that electrode's own `α` and is an
/// approximation — stated here rather than hidden, and preferable to refusing to
/// load such a set or to paying a per-electrode Newton solve inside every voltage
/// evaluation.
#[must_use]
fn overpotential(side: &Side<'_>, temp_k: f64, c_e: f64, c_s: f64, i_s: f64) -> f64 {
    let c_max = side.p.c_max_mol_per_m3;
    let i_0 = side.m_ref * (c_e * c_s * (c_max - c_s)).sqrt();
    let prefactor =
        GAS_CONSTANT_J_PER_MOL_K * temp_k / (side.p.charge_transfer_alpha * FARADAY_C_PER_MOL);
    prefactor * (i_s / (2.0 * i_0)).asinh()
}

/// One electrode's contribution to the terminal voltage \[V\]: its open-circuit
/// potential at the **surface** stoichiometry plus its Butler–Volmer overpotential.
///
/// `j_surf` is the flux leaving that particle and `i_s` the current density driving
/// its reaction; both carry the electrode's own sign, so the caller passes the
/// signed pair and this function needs no notion of which electrode it is.
#[must_use]
///
/// `c_outer` and `shells` are the outermost shell's concentration \[mol/m³\] and the
/// profile's shell count — all [`c_surface`] reads — so the caller decides *which* outer
/// shell: the stored one, or the end-of-step one [`probe_at`] solves for.
fn half(
    w: &Working<'_>,
    c_outer: f64,
    shells: usize,
    side: &Side<'_>,
    j_surf: f64,
    i_s: f64,
) -> f64 {
    let c_s = clamp_surface(
        surface_from_outer(c_outer, shells, side.p.particle_radius_m, side.d_s, j_surf),
        side.p.c_max_mol_per_m3,
    );
    ocp_lookup(&side.p.ocp, c_s / side.p.c_max_mol_per_m3)
        + overpotential(side, w.temp_k, w.spm.c_e_mol_per_m3, c_s, i_s)
}

/// Terminal voltage \[V\] at cell current `i` \[A, discharge-positive\], evaluated
/// from the **start-of-step** solid state.
///
/// `V = (U_p + η_p) − (U_n + η_n) − i·R_contact`. Nonlinear in `i` through both
/// overpotentials *and* through both surface concentrations, which the flux
/// boundary condition shifts — this is what ends the pack's closed-form solve.
#[must_use]
fn voltage(w: &Working<'_>, s: &SpmState, i: f64) -> f64 {
    let outer = |c: &[f64]| (c[c.len() - 1], c.len());
    voltage_from_outer(w, outer(&s.c_neg), outer(&s.c_pos), i)
}

/// [`voltage`], given each particle's outermost shell `(concentration, shell count)`
/// rather than its profile — the surface is extrapolated from that shell alone.
#[must_use]
fn voltage_from_outer(w: &Working<'_>, neg: (f64, usize), pos: (f64, usize), i: f64) -> f64 {
    let n = half(w, neg.0, neg.1, &w.neg, w.j_neg(i), i / w.neg.g.area_m2);
    let p = half(w, pos.0, pos.1, &w.pos, w.j_pos(i), -i / w.pos.g.area_m2);
    p - n - i * w.r_contact
}

/// Terminal voltage \[V\] at cell current `i` at the **end** of a step that carries `i`
/// throughout: the particles diffused by backward Euler under that current's flux,
/// exactly as [`advance`] will diffuse them, and the surface read from there.
///
/// `neg` and `pos` are the two particles' [`OuterShell`]s for the step, from
/// [`forward_sweep`]. The whole dependence on `i` — kinetics, surface extrapolation, and
/// the lithium the step moves — is in here.
#[must_use]
fn end_of_step_voltage(
    w: &Working<'_>,
    s: &SpmState,
    neg: OuterShell,
    pos: OuterShell,
    i: f64,
) -> f64 {
    voltage_from_outer(
        w,
        (neg.at(w.j_neg(i)), s.c_neg.len()),
        (pos.at(w.j_pos(i)), s.c_pos.len()),
        i,
    )
}

/// Equilibrium (open-circuit) voltage \[V\] at the particles' **mean** — bulk —
/// stoichiometry.
///
/// This is the "OCV" of the `V ≤ OCV` discharging property, and the potential the
/// cell's stored chemical energy is measured against. Deliberately not the surface
/// value: the surface-vs-bulk gap is an overpotential the cell gets back on
/// relaxation, not energy it has spent.
#[must_use]
fn equilibrium_voltage(w: &Working<'_>, s: &SpmState) -> f64 {
    let x = mean_concentration(&s.c_neg) / w.neg.p.c_max_mol_per_m3;
    let y = mean_concentration(&s.c_pos) / w.pos.p.c_max_mol_per_m3;
    ocp_lookup(&w.pos.p.ocp, y) - ocp_lookup(&w.neg.p.ocp, x)
}

/// The range of cell current \[A, discharge-positive\], `(lo, hi)`, over which both
/// particles' surfaces stay strictly inside the interval [`clamp_surface`] passes through
/// untouched — at the **end** of the step when `ends` carries that step's
/// [`OuterShell`]s, at the stored state otherwise.
///
/// # What this is the model saying about itself
/// Outside this range a surface has been driven past empty or past full, and nothing in
/// this model describes that: the clamp holds the surface at the edge, the tables stop
/// there, and `V(i)` goes **flat** — it moves only through the kinetics and the contact
/// resistance, not through the lithium the step moved. A flat curve is not a harmless
/// artefact to a solver built on tangents. A tangent taken on it has almost no slope, so
/// the next pass puts almost any current there. Measured on the shipped LG M50: a 10 W
/// demand no cell at 35 % can meet for an hour was "met" at 57 A and 0.18 V, a root that
/// exists only on the flat stretch (the old engine found it one step later, at 1290 K); and
/// a 1S2P pack rested for 1e6 s after a 5 A discharge seeded its solve on that stretch and
/// ran to 1e9 A. [`first_pass_tangent`] and [`probe_at`]'s `hold` are what is done about
/// it, and each says why it goes no further.
///
/// # Closed form
/// The surface is affine in the flux the step carries: the outer shell is
/// `((t − k·j) − m)/d` (see [`OuterShell`]) and [`c_surface`] extrapolates half a shell
/// further along `j`, so `c_s = A − B·j` with `B > 0`. The flux is linear in the current
/// with a sign of its own on each electrode, so each surface gives one interval and the
/// window is their intersection. The affine rearrangement is not bit-identical to
/// [`half`]'s evaluation order, which does not matter for a boundary.
///
/// `None` when the intersection is empty or not finite — a state already past a limit at
/// zero current, which a snapshot or an overdriven previous step can leave behind, since
/// the profile itself is never clamped. The caller then has no window to hold to.
#[must_use]
fn current_window(
    w: &Working<'_>,
    s: &SpmState,
    ends: Option<(OuterShell, OuterShell)>,
) -> Option<(f64, f64)> {
    // One electrode's interval: `c_s(i) = A − B·g·i`, with `g` the flux per ampere.
    let side = |c: &[f64], side: &Side<'_>, outer: Option<OuterShell>, g: f64| {
        let n = c.len();
        let dr = side.p.particle_radius_m / n as f64;
        let extrapolate = 0.5 * dr / side.d_s;
        let (a, b) = match outer {
            Some(o) => ((o.t - o.m) / o.d, o.k / o.d + extrapolate),
            None => (c[n - 1], extrapolate),
        };
        let c_max = side.p.c_max_mol_per_m3;
        let bg = b * g;
        let at_edge = |edge: f64| (a - edge) / bg;
        let (x, y) = (
            at_edge((1.0 - SURFACE_EDGE) * c_max),
            at_edge(SURFACE_EDGE * c_max),
        );
        (x.min(y), x.max(y))
    };
    let (neg_end, pos_end) = match ends {
        Some((n, p)) => (Some(n), Some(p)),
        None => (None, None),
    };
    let (n_lo, n_hi) = side(&s.c_neg, &w.neg, neg_end, w.j_neg(1.0));
    let (p_lo, p_hi) = side(&s.c_pos, &w.pos, pos_end, w.j_pos(1.0));
    let (lo, hi) = (n_lo.max(p_lo), n_hi.min(p_hi));
    (lo.is_finite() && hi.is_finite() && lo < hi).then_some((lo, hi))
}

/// `i` pulled inside `(lo, hi)` by at least the tangent's difference step `h`, so a
/// quotient taken there straddles no flat stretch; the middle of a range too narrow for
/// that. Unchanged, bit for bit, when it is already that far inside.
#[must_use]
fn held(i: f64, (lo, hi): (f64, f64), h: f64) -> f64 {
    if hi - lo > 2.0 * h {
        i.clamp(lo + h, hi - h)
    } else {
        0.5 * (lo + hi)
    }
}

/// Backward-Euler radial diffusion over one step, by the Thomas algorithm.
///
/// `j_surf` is the molar flux \[mol/(m²·s)\] leaving the particle surface. The
/// system is symmetric positive-definite and diagonally dominant for any `dt > 0`,
/// which is what "unconditionally stable" means here — no pivoting, no step-size
/// bound.
///
/// # Discretisation
/// Shell `i` spans `[i·dr, (i+1)·dr]` with `dr = R_p/n`; the `4π` cancels out of
/// every term, so volumes are `(r_hi³ − r_lo³)/3` and face conductances are
/// `r²·D/dr`. The flux enters as a source on the outermost shell only. Summing the
/// rows shows the scheme conserves lithium exactly up to rounding: the total moles
/// change by exactly `−dt·R_p²·j_surf`, which is what makes the charge-conservation
/// property hold.
///
/// # Two scratch rows collapsed into one
/// `rhs` is built in place in `c` (it starts as `vol·c`), and the super-diagonal is
/// recomputed on demand in the back substitution rather than stored — it is one
/// multiply and it is never modified by the sweep. The sub-diagonal needs no row at
/// all: `lo[i]` and `up[i−1]` are the same face and therefore the same number. What
/// is left is the modified diagonal, which the sweep does overwrite, and that is the
/// single fixed-size row this function stands up.
pub fn diffuse(c: &mut [f64], r_p: f64, d_s: f64, j_surf: f64, dt: f64) {
    // A zero-length step must leave the profile **bit-identically** alone, and
    // arithmetic alone does not deliver that. Every off-diagonal carries a `dt`
    // factor, so at `dt = 0` the system collapses to the diagonal one — but solving
    // it still evaluates `(c·vol)/vol`, and multiplying and dividing by the same
    // `f64` is not the identity: it lands one ULP off for a good fraction of inputs.
    // That is enough to make a *probe* step move a pack, and the suite is full of
    // probe steps (see `advance`'s note on `i_last`, and
    // `a_zero_length_step_does_not_mutate_an_spm_cell`). Found by a perturbation
    // hunting something else, which is the only reason it is not still here.
    //
    // Non-finite `dt` takes the same exit: `step` may not panic, and there is no
    // meaningful profile to solve for.
    if dt.is_nan() || dt <= 0.0 {
        return;
    }
    let n = c.len();
    let dr = r_p / n as f64;
    let mut diag = [0.0_f64; MAX_SHELLS];
    let outer = forward_sweep(c, &mut diag, r_p, d_s, dt);
    // Back substitution. Its first row is the outer shell's, which is the one row the
    // flux enters — see [`OuterShell`] for why it is evaluated there and not inline.
    c[n - 1] = outer.at(j_surf);
    for i in (0..n - 1).rev() {
        let r = (i + 1) as f64 * dr;
        let up = -dt * (r * r * d_s / dr);
        c[i] = (c[i] - up * c[i + 1]) / diag[i];
    }
}

/// The outermost shell's concentration \[mol/m³\] at the **end** of a backward-Euler
/// [`diffuse`] step, as a function of the surface flux that step carries.
///
/// The flux enters the tridiagonal system on the outer row alone, and the forward sweep
/// never carries that row's right-hand side anywhere else, so after the sweep the outer
/// shell is `((t − k·j) − m) / d` with nothing else depending on `j`. That affine form is
/// what lets [`probe_at`] ask "where would the surface be at the end of this step, at this
/// current?" for a handful of currents at the cost of one sweep — and, because
/// [`diffuse`] evaluates its own outer row through [`Self::at`] too, the answer is
/// **bit-for-bit** the outer shell `advance` then produces at that current.
#[derive(Clone, Copy, Debug)]
struct OuterShell {
    /// `vol · c_old` of the outer shell: its right-hand side before the flux.
    t: f64,
    /// `dt · R_p²`: how much a unit of flux removes from that right-hand side.
    k: f64,
    /// What eliminating the shell beneath subtracts from it.
    m: f64,
    /// The outer row's diagonal after elimination.
    d: f64,
}

impl OuterShell {
    /// The outer shell's end-of-step concentration \[mol/m³\] under surface flux `j`
    /// \[mol/(m²·s)\].
    fn at(self, j: f64) -> f64 {
        ((self.t - self.k * j) - self.m) / self.d
    }
}

/// The forward (elimination) sweep of [`diffuse`], over every row but leaving the outer
/// one's flux-dependent right-hand side unevaluated — see [`OuterShell`].
///
/// Overwrites `c[..n−1]` with the swept right-hand sides and `diag[..n]` with the swept
/// diagonal, which is exactly the working state [`diffuse`]'s back substitution needs.
/// Requires `dt > 0` and `MIN_SHELLS ≤ c.len() ≤ MAX_SHELLS`; both callers guarantee them.
fn forward_sweep(
    c: &mut [f64],
    diag: &mut [f64; MAX_SHELLS],
    r_p: f64,
    d_s: f64,
    dt: f64,
) -> OuterShell {
    let n = c.len();
    debug_assert!((MIN_SHELLS..=MAX_SHELLS).contains(&n));
    let dr = r_p / n as f64;
    // Face conductance below shell `i` — equivalently above shell `i-1`, which is
    // why one closure serves both diagonals.
    let g_face = |i: usize| {
        let r = i as f64 * dr;
        r * r * d_s / dr
    };
    let mut outer = OuterShell {
        t: 0.0,
        k: 0.0,
        m: 0.0,
        d: 1.0,
    };
    // Forward sweep, building the row and eliminating it in the same pass.
    for i in 0..n {
        let r_lo = i as f64 * dr;
        let r_hi = (i + 1) as f64 * dr;
        let vol = (r_hi * r_hi * r_hi - r_lo * r_lo * r_lo) / 3.0;
        let g_lo = if i == 0 { 0.0 } else { g_face(i) };
        let g_hi = if i == n - 1 { 0.0 } else { g_face(i + 1) };
        diag[i] = vol + dt * (g_lo + g_hi);
        // lo[i] = -dt·g_lo = up[i-1].
        let m = if i > 0 { -dt * g_lo / diag[i - 1] } else { 0.0 };
        if i > 0 {
            diag[i] -= m * (-dt * g_lo);
        }
        if i == n - 1 {
            outer = OuterShell {
                t: c[i] * vol,
                k: dt * r_hi * r_hi,
                m: m * c[i - 1],
                d: diag[i],
            };
        } else {
            c[i] *= vol;
            if i > 0 {
                c[i] -= m * c[i - 1];
            }
        }
    }
    outer
}

/// Ground-truth state of charge, in \[0, 1\]: the negative particle's mean
/// stoichiometry mapped onto its usable window.
///
/// # This is a readout, not a counter
/// An equivalent circuit *stores* SOC and coulomb-counts it, so `∫I·dt = ΔSOC·Q` is
/// arithmetic. Here it is derived from the lithium actually in the particle, so the
/// same identity is a *result* — and only an approximate one under load, because
/// the voltage cut-off arrives while the surface runs ahead of the bulk. The
/// meaning is unchanged from the one [`crate::Telemetry::soc_true`] fixed at v6:
/// the fraction of the capacity this cell has **today**. Aging does not move the
/// window, so a faded cell still reads `1.0` when full; it simply holds fewer
/// amp-hours between the same two readings.
///
/// Clamped, because the window is the *usable* range rather than a physical bound —
/// an overcharged particle really can sit past `stoich_max`, and [`advance`] flags
/// that rather than the clamp hiding it.
#[must_use]
pub(crate) fn soc(s: &SpmState, spm: &SpmParams) -> f64 {
    raw_soc(s, spm).clamp(0.0, 1.0)
}

/// [`soc`] before the clamp — the value that says whether a limit was passed.
#[must_use]
fn raw_soc(s: &SpmState, spm: &SpmParams) -> f64 {
    let e = &spm.negative;
    window_fraction_neg(mean_concentration(&s.c_neg) / e.c_max_mol_per_m3, e)
}

/// Where a **negative**-electrode stoichiometry sits on its usable window, as a
/// fraction — the mapping [`soc`] is built from, before its clamp.
///
/// Rising: the negative electrode is emptied by a discharge, so its stoichiometry runs
/// the same direction as state of charge.
///
/// **Unclamped**, and every caller has to know it. [`soc`] applies the clamp itself
/// because the window is the *usable* range rather than a physical bound; a
/// **difference** of two of these must not, because a clamp on one side and not the
/// other stops being a gradient and starts being the clamp. Measured: after a 3 C
/// overcharge at rest, a uniform pack reads `1.186667` here and `1.000000` from
/// [`soc`], so differencing the two shows a standing `+0.186667` that no rest removes
/// and that is not a gradient at all. See [`surface_gap`].
#[must_use]
pub(crate) fn window_fraction_neg(stoich: f64, e: &ElectrodeParams) -> f64 {
    (stoich - e.stoich_min) / (e.stoich_max - e.stoich_min)
}

/// Where a **positive**-electrode stoichiometry sits on its usable window, as a
/// fraction. The sibling of [`window_fraction_neg`]; read that doc for the clamp.
///
/// Falling: the positive electrode is *filled* by a discharge, so its stoichiometry
/// runs backwards against state of charge — which is the direction
/// [`SpmState::new`] and `DfnState::new` both seed it in.
///
/// That reversal is not a detail to normalise away. It is what makes a `bulk − surface`
/// gap read **positive on discharge on both electrodes** with no sign fixed up by the
/// caller: a discharging negative particle has a surface *below* its bulk and a
/// discharging positive particle has one *above*, and the two mappings turn both into
/// the same statement — the surface is further through the discharge than the bulk is.
#[must_use]
pub(crate) fn window_fraction_pos(stoich: f64, e: &ElectrodeParams) -> f64 {
    (e.stoich_max - stoich) / (e.stoich_max - e.stoich_min)
}

/// Total overpotential \[V\], discharge-positive: everything between the
/// equilibrium voltage at bulk stoichiometry and the terminal that is **not** the
/// instantaneous ohmic drop.
///
/// For this model that is the concentration overpotential (bulk OCP minus surface
/// OCP, on both electrodes) plus the two Butler–Volmer overpotentials — the
/// quantity [`crate::CellView::overpotential_v`] reports, and the direct analogue of
/// an equivalent circuit's `Σ V_rc`. Evaluated at [`SpmState::i_last`], which is the
/// current the state it is reading was produced by.
///
/// Non-zero at rest while a particle is still relaxing, and that is the point: an
/// SPM's polarization outlives the current that caused it, exactly as an RC pair's
/// does.
#[must_use]
pub(crate) fn overpotential_v(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> f64 {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let i = s.i_last;
    equilibrium_voltage(&w, s) - voltage(&w, s, i) - i * w.r_contact
}

/// Bulk minus surface stoichiometry on each electrode, `(negative, positive)`, both on
/// the window [`soc`] uses and both **discharge-positive**.
///
/// The concentration half of [`overpotential_v`], reported as the gradient itself
/// rather than as the voltage it costs. Evaluated at [`SpmState::i_last`] — the current
/// the state it reads was produced by — so a zero-length probe step, which never writes
/// `i_last`, leaves this reading whatever the last real step left it. On a pack that has
/// never stepped that is exactly `0.0` on both electrodes, and it is a true zero: a
/// uniform particle has no gradient.
///
/// # Why a difference and not a level
/// A surface *level* on this window leaves \[0, 1\] — measured at `1.161385` here on a
/// 3 C overcharge, and past `stoich_max` there is no window left to be a fraction of.
/// A difference of two unclamped fractions on the same window has no such bound to
/// break, is not a state of charge, and relaxes to exactly `0.0` at rest in every case
/// measured including that one. See [`window_fraction_neg`] for what a clamp on one
/// side alone does to it.
///
/// # Resistance growth deliberately does not enter
/// No `eff_r0_factor` argument, unlike [`overpotential_v`]: it would reach only `m_ref`
/// and `r_contact`, and a concentration gradient is set by diffusion and the flux
/// boundary — neither of which is a resistance. `eff_capacity_ah` **does** enter, through
/// `κ`: how much active material a cell holds decides what flux a given current is.
/// Same reasoning, and the same shape, as [`advance`]'s own note on the argument.
#[must_use]
pub(crate) fn surface_gap(s: &SpmState, spm: &SpmParams, eff_capacity_ah: f64) -> (f64, f64) {
    let w = Working::new(spm, s.temp_k, 1.0, eff_capacity_ah);
    let n = &spm.negative;
    let p = &spm.positive;
    let cs_n = c_surface(&s.c_neg, n.particle_radius_m, w.neg.d_s, w.j_neg(s.i_last));
    let cs_p = c_surface(&s.c_pos, p.particle_radius_m, w.pos.d_s, w.j_pos(s.i_last));
    (
        window_fraction_neg(mean_concentration(&s.c_neg) / n.c_max_mol_per_m3, n)
            - window_fraction_neg(cs_n / n.c_max_mol_per_m3, n),
        window_fraction_pos(mean_concentration(&s.c_pos) / p.c_max_mol_per_m3, p)
            - window_fraction_pos(cs_p / p.c_max_mol_per_m3, p),
    )
}

/// Heat generated inside this cell \[W\] at current `i`, given the terminal voltage
/// `v_terminal` the pack's solve produced for it.
///
/// Same two terms as the equivalent circuit's [`crate::ecm::cell_heat_w`], in the
/// *general* form that one's doc comment derives:
///
/// * **Irreversible** `I·(U_eq − V)`, the whole overpotential heat — kinetic,
///   concentration and ohmic together. There is no `I²·R0 + I·ΣV_rc` decomposition
///   here because there is no `R0` and no RC pair; the general form is the only one
///   available, and it is the one that keeps the pack energy balance closing.
/// * **Reversible (entropic)** `−I·T·∂U/∂T`, with the cell's `∂U/∂T` assembled from
///   the two half-cell entropy coefficients as `∂U_p/∂T − ∂U_n/∂T`. Zero for the
///   shipped chemistry, whose set publishes both as literally zero.
///
/// An **estimate**, read at the start-of-step equilibrium voltage against the end-of-step
/// node, and so wrong at a long step by the equilibrium voltage's fall across it.
/// [`advance`] returns the two corrections the pack adds to it, read off this cell's own
/// curve rather than the node; see `docs/plans/spm-end-of-step.md`.
#[must_use]
pub(crate) fn heat_w(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
    v_terminal: f64,
) -> f64 {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let q_irrev = i * (equilibrium_voltage(&w, s) - v_terminal);
    let docv_dt = spm.positive.docp_dt_v_per_k - spm.negative.docp_dt_v_per_k;
    let q_rev = -i * s.temp_k * docv_dt;
    q_irrev + q_rev
}

/// This cell's Thévenin **tangent** `(E, R)` at [`SpmState::i_last`]: the linear
/// source `V ≈ E − i·R` that best matches `V(i)` at the previous step's operating
/// point.
///
/// `R = −dV/di` by central difference, then `E = V(i*) + i*·R` so the tangent
/// passes through the evaluated point. Note this is exactly the reconstruction that
/// [`crate::CellModel::source`]'s doc forbids the *ECM* arm from doing — there it
/// would lose bits off an already-exact answer, here there is no exact answer to
/// lose, and `E` has to be built this way for the line to touch the curve.
///
/// # Why a numerical derivative
/// `V(i)` runs through two `asinh` terms and two piecewise-linear table lookups; an
/// analytic Jacobian would have to differentiate the tables, and a tangent taken
/// against a *different* function than the one evaluated is worse than a difference
/// quotient taken against the same one. The step is scaled to the cell's own
/// capacity (a micro-C-rate), which leaves the quotient eight significant digits of
/// headroom.
///
/// # Purity
/// A pure function of cell state and the two scale factors — `i_last` is *state*,
/// which is what lets [`crate::pack`]'s `SourceCache` memoise this the same way it
/// memoises the equivalent circuit's exact source. [`source_at`] is the same
/// function with the operating point supplied instead, and it is deliberately
/// **not** memoisable for exactly that reason.
#[must_use]
pub(crate) fn source(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
) -> (f64, f64) {
    source_at(s, spm, eff_r0_factor, eff_capacity_ah, s.i_last)
}

/// [`source`], but tangent to `V(i)` at the caller's operating point `i` rather
/// than at [`SpmState::i_last`].
///
/// This is what the pack's nonlinear solve iterates on: each pass re-takes every
/// cell's tangent at the current the previous pass assigned it, and the fixed point
/// — tangent taken at the current it predicts — is the nonlinear solution. See
/// `Pack::step`.
///
/// **Not a pure function of state**, and that is the whole difference from
/// [`source`]: `i` is an in-flight iterate, which is precisely what `SourceCache`'s
/// invariant rules out. Nothing computed here may be written into that memo.
#[must_use]
pub(crate) fn source_at(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
) -> (f64, f64) {
    // `dt = 0`: the start-of-step curve, which is what a tangent that has to be a pure
    // function of state (see [`source`]) can be taken on. The pack's iteration re-takes
    // every tangent on the end-of-step curve from its first probe on.
    probe_at(s, spm, eff_r0_factor, eff_capacity_ah, i, 0.0, false).1
}

/// Where the curve is at `i` over a step of `dt` seconds, and what straight line touches it
/// there: `(V(i), (E, R))`.
///
/// # The curve is the end-of-step one
/// With `dt > 0`, `V(i)` is the terminal voltage at the **end** of a step carrying `i`
/// throughout — the particles diffused under that current's flux by the same
/// backward-Euler sweep [`advance`] runs, bit for bit (see [`OuterShell`]). So the pack's
/// split and its demand solve are backward Euler on the cells' coupling through their
/// shared node, as they already were for the `Dfn` and became for the equivalent circuit
/// in `docs/plans/end-of-step-split.md`. The tangent's `R` therefore carries the lithium
/// the step moves as well as the kinetics: a long step's `R` is large, and a parallel
/// group's circulation is damped rather than amplified.
///
/// Read at the start of the step instead, the curve was explicit Euler on that coupling.
/// A scattered 1S3P LG M50 at a one-hour step read 10 000 K by its fourth step and
/// −1.2e9 V by its fifth; a voltage hold charged at 37.7 A whatever the step length,
/// which at ten minutes put the cell at 443 K in one step.
///
/// `dt <= 0` (and a `NaN` `dt`) reads the stored state instead: no time passes, so there
/// is no end of the step to read, and [`diffuse`] is refused the same way. That is the
/// path a zero-length probe step takes, and it keeps such a step bit-for-bit what it was.
///
/// # `hold`: the demands whose operating point the engine chooses
/// With `hold`, a probe outside [`current_window`] is taken at the nearest current inside
/// it instead — both the voltage and the tangent. The pack asks for that under
/// [`crate::Demand::Power`] and [`crate::Demand::Voltage`], and the reason is who picks the
/// current. A current demand's caller picks it, and a cell it really drives past empty is
/// solved out there on the flat curve, as it must be. Under the other two the engine picks
/// it, and the flat stretch is where its iteration gets lost:
///
/// * **Power.** The flat stretch holds a root the physics does not: a 10 W hour from an
///   LG M50 at 35 %, whose true best is under 5 W, was "met" at 57 A and 0.18 V and ran
///   away to 1000 K. Held, an unreachable power lands at the most the cell can deliver.
/// * **Voltage.** The answer is never out there, but the iteration's intermediate currents
///   were. Holding a scattered 1S3P or a 4S2P at a target low in the window for one hour,
///   the solve wandered onto the flat curve and ran to 1e8–5e9 A; 63 of 405 hour-long holds
///   across the window finished unconverged, most of them unbounded. Held: 1 of 405, at
///   4.7 A.
///
/// A fixed point inside the range is untouched by the hold, so every reachable demand
/// solves exactly as before. The cost is a target the cell cannot reach inside its range
/// within the step — a 2.5 V hold at a one-second step asks for more than 200 A — which
/// now stops at the edge of the range and raises `SOLVE_UNCONVERGED`, where it used to
/// converge out on the flat curve. That moved a 1 s sweep of the same holds from 4 to 28
/// unconverged of 405, every one bounded (≤ 660 A pack current, ≤ 312 K). A pack whose
/// demand is met only by driving one weak cell past empty fails the same way.
#[must_use]
pub(crate) fn probe_at(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    i: f64,
    dt: f64,
    hold: bool,
) -> (f64, (f64, f64)) {
    let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let end = if dt.is_nan() || dt <= 0.0 {
        None
    } else {
        // One forward sweep per particle; each copy is a stack row, so a probe allocates
        // nothing. The shell count was checked against `MAX_SHELLS` at construction.
        let sweep = |c: &[f64], r_p: f64, d_s: f64| {
            let mut row = [0.0_f64; MAX_SHELLS];
            let mut diag = [0.0_f64; MAX_SHELLS];
            let row = &mut row[..c.len()];
            row.copy_from_slice(c);
            forward_sweep(row, &mut diag, r_p, d_s, dt)
        };
        Some((
            sweep(&s.c_neg, spm.negative.particle_radius_m, w.neg.d_s),
            sweep(&s.c_pos, spm.positive.particle_radius_m, w.pos.d_s),
        ))
    };
    let curve = |i: f64| match end {
        Some((neg, pos)) => end_of_step_voltage(&w, s, neg, pos, i),
        None => voltage(&w, s, i),
    };
    let h = 1.0e-6 * eff_capacity_ah;
    let i = match current_window(&w, s, end) {
        Some(range) if hold && end.is_some() => held(i, range, h),
        _ => i,
    };
    let r = -(curve(i + h) - curve(i - h)) / (2.0 * h);
    // `V` is strictly decreasing in `i`, so `r > 0` on any state the model can
    // reach. The floor is not physics — it is the guarantee the pack solve needs
    // (it divides by `r`) held up on a state that has been driven somewhere
    // pathological, so that `step` reports a bad number rather than panicking or
    // returning NaN. See `CLAUDE.md`'s "never panics" rule.
    const R_FLOOR_OHMS: f64 = 1.0e-9;
    let r = if r.is_finite() && r > R_FLOOR_OHMS {
        r
    } else {
        R_FLOOR_OHMS
    };
    let v = curve(i);
    (v, (v + i * r, r))
}

/// The tangent a pack's **first** pass aggregates this cell from on a step of `dt`: the
/// end-of-step curve's (see [`probe_at`]), taken at the current the cell last carried —
/// **pulled back inside [`current_window`]** if the step would take it out.
///
/// The last current is the natural place to start: a steady demand converges in one
/// pass from it. But it was chosen for the previous step's length, and on a much longer
/// step it can empty or overfill the particle several times over. There the curve is
/// flat, a tangent to it has almost no slope, and the pass it seeds puts almost any
/// current anywhere. Measured on the shipped LG M50: a 1S2P pack rested for 1e6 s after
/// a 5 A discharge seeded at 5 A and ran to 1e9 A on its first rest step, and to NaN
/// after it (the start-of-step engine did the same one step later). Pulled back to the
/// range — a few milliamps at that step length — the same step converges.
///
/// Under a current demand only the **seed** is held to the range, not the iteration's own
/// probes, because a current demand that really does drive a cell past empty has its answer
/// out there, on the flat curve, and a solve held away from it would never converge on it.
/// Power and voltage demands hold their probes too; see [`probe_at`].
/// That region is where this model has no physics, which `docs/ROADMAP.md` (H8) records.
#[must_use]
pub(crate) fn first_pass_tangent(
    s: &SpmState,
    spm: &SpmParams,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    dt: f64,
) -> (f64, f64) {
    let mut i = s.i_last;
    if !dt.is_nan() && dt > 0.0 {
        let w = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
        let sweep = |c: &[f64], r_p: f64, d_s: f64| {
            let mut row = [0.0_f64; MAX_SHELLS];
            let mut diag = [0.0_f64; MAX_SHELLS];
            let row = &mut row[..c.len()];
            row.copy_from_slice(c);
            forward_sweep(row, &mut diag, r_p, d_s, dt)
        };
        let ends = (
            sweep(&s.c_neg, spm.negative.particle_radius_m, w.neg.d_s),
            sweep(&s.c_pos, spm.positive.particle_radius_m, w.pos.d_s),
        );
        // Only a cell that could rest through the step inside its range is pulled back
        // into it. One already past a limit has no range near zero to be pulled into,
        // and its answer is out on the flat stretch where the last current already is.
        if let Some(range) =
            current_window(&w, s, Some(ends)).filter(|&(lo, hi)| lo < 0.0 && 0.0 < hi)
        {
            i = held(i, range, 1.0e-6 * eff_capacity_ah);
        }
    }
    probe_at(s, spm, eff_r0_factor, eff_capacity_ah, i, dt, false).1
}

/// Advance both particles by `dt` seconds under the current `i` the pack solve
/// assigned this cell, and record it as the next tangent's operating point.
///
/// Returns the SOC-clamp flags, on the same contract as [`crate::ecm::coulomb_step`]
/// — but note what is and is not clamped. The concentration profile is **never**
/// clamped: an overcharged particle keeps the lithium it was pushed, so the flag
/// says the *readout* has run past its window rather than that state was discarded.
/// Getting that lithium back out is then a discharge, which is the physical answer
/// and a more honest one than the equivalent circuit's hard SOC clamp.
///
/// `i_last` is written only when time actually passed. A zero-length probe step
/// mutates nothing — the diffusion solve is already a no-op at `dt = 0` by
/// arithmetic, and this is the one line that would not have been.
///
/// # The heat terms it hands back
/// Alongside the flags, two voltages \[V\] the pack turns into heat by multiplying by
/// `i` — the porous-electrode counterparts of the equivalent circuit's
/// `rc_delta_v` and `rc_mean_excess_v` (see [`crate::ecm::Advanced`]). The pack's first
/// estimate of this cell's heat is [`heat_w`], `i·(U_eq,start − v_node)`, and since
/// [`probe_at`] reads the end-of-step curve, `v_node` is the step's **last** instant. That
/// pairs a start-of-step equilibrium with an end-of-step terminal, and so books the fall
/// of the equilibrium voltage across the step — stored energy leaving through the
/// terminals — as heat. Measured on a 1S3P LG M50 at C/5: 303.3 K after one one-hour step
/// against 299.5 K after sixty one-minute ones.
///
/// The two corrections replace that estimate with ones read off the **curve alone**, so
/// that `v_node` cancels out of both:
///
/// * **`delta_v`** makes what the pack *reports* `i·(U_eq,end − V_end(i))`: the
///   end-of-step heat, which pairs with the terminal voltage the pack reports for this
///   cell — also read off the end-of-step state — in the energy ledger.
/// * **`mean_excess_v`** makes what the thermal network integrates
///   `½·i·[(U_eq,start − V_start(i)) + (U_eq,end − V_end(i))]`, the mean of the step's
///   first and last instants. A **trapezoid, stated rather than hidden**: unlike an RC
///   pair, the particle's overpotential has no closed-form step mean. Its concentration
///   part grows roughly as `√t` from a relaxed cell, whose true mean is two thirds of its
///   end value, not the trapezoid's half.
///
/// Why not trust `v_node`: it is on the curve only when the pack's solve converged. When
/// it did not — `SOLVE_UNCONVERGED`, which `solve_safeguard.rs` records on ordinary
/// voltage holds — the node and the curve part company, and a correction that assumed
/// they agree fed the thermal network heat the cell never made (a cell cooled to 189 K
/// under load, measured). The cell's own curve is the physics; the node is the solver's
/// estimate of it.
///
/// Both are exactly `0.0` at `dt <= 0`, which keeps a zero-length probe step bit-for-bit
/// what it was.
#[must_use]
pub(crate) fn advance(
    s: &mut SpmState,
    spm: &SpmParams,
    i: f64,
    dt: f64,
    eff_r0_factor: f64,
    eff_capacity_ah: f64,
    v_node: f64,
) -> (EventFlags, f64, f64) {
    let moves = !dt.is_nan() && dt > 0.0;
    // The heat terms read the kinetics, so they need the real resistance factor; the
    // diffusion below does not, which is the note on its own `Working`.
    let wk = Working::new(spm, s.temp_k, eff_r0_factor, eff_capacity_ah);
    let start = if moves {
        (equilibrium_voltage(&wk, s), voltage(&wk, s, i))
    } else {
        (0.0, 0.0)
    };
    // The resistance factor is passed as `1.0` and it is not a placeholder: it
    // reaches only `m_ref` and `r_contact`, and diffusion uses neither. Handing the
    // real factor in would change nothing and would suggest, falsely, that
    // resistance growth slows transport.
    let w = Working::new(spm, s.temp_k, 1.0, eff_capacity_ah);
    diffuse(
        &mut s.c_neg,
        spm.negative.particle_radius_m,
        w.neg.d_s,
        w.j_neg(i),
        dt,
    );
    diffuse(
        &mut s.c_pos,
        spm.positive.particle_radius_m,
        w.pos.d_s,
        w.j_pos(i),
        dt,
    );
    if dt > 0.0 {
        s.i_last = i;
    }
    let raw = raw_soc(s, spm);
    let mut flags = EventFlags::empty();
    if raw > 1.0 {
        flags |= EventFlags::SOC_CLAMPED_HIGH;
    } else if raw < 0.0 {
        flags |= EventFlags::SOC_CLAMPED_LOW;
    }
    if !moves {
        return (flags, 0.0, 0.0);
    }
    let (u_start, v_start) = start;
    let u_end = equilibrium_voltage(&wk, s);
    let v_end = voltage(&wk, s, i);
    // What the pack's estimate already holds, and so what each correction takes back out.
    let estimate = u_start - v_node;
    let at_end = u_end - v_end;
    (
        flags,
        at_end - estimate,
        0.5 * ((u_start - v_start) + at_end) - estimate,
    )
}
