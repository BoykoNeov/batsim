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

use crate::chem::{ChemistryParams, DfnParams, OcvTable, R0Table, SpmParams};
use crate::dfn::{self, DfnState};
use crate::flags::EventFlags;
use crate::spm::{self, SpmState};
use crate::Demand;

/// Per-cell equivalent-circuit state. Opaque to the pack layer; the enclosing
/// [`CellModel`] variant fixes how many entries `v_rc` carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EcmState {
    /// State of charge, in \[0, 1\].
    pub soc: f64,
    /// Charge withdrawn beyond empty, as a fraction of today's capacity; `0.0` on any
    /// cell that has not been over-discharged.
    ///
    /// **Invariant: `> 0.0` only while [`Self::soc`] is exactly `0.0`.** The pair is one
    /// number — the cell's *extended* position `soc − soc_deficit`, which is what the
    /// coulomb count actually advances — split so that `soc` itself never leaves
    /// `[0, 1]`. That split is deliberate and is the whole reason this is cheap: the
    /// `R0` lookup's bracket, aging's SOC-stress table, the BMS estimator, plating and
    /// [`crate::Telemetry::soc_true`] all keep reading a `soc` in the range their own
    /// tables and thresholds were written against, and none of them needed a line
    /// changed. A negative `soc` would put every one of them in play for no physics.
    ///
    /// Physically this is how far the cell is into **voltage reversal**. It feeds
    /// [`cell_source`], which drops the open-circuit voltage by
    /// `reversal.v_per_soc · soc_deficit` down to `reversal.floor_v`, so an
    /// over-discharged cell presents a falling and then negative source and the external
    /// circuit pays for the current it is forcing. Before this field the cell sourced at
    /// `OCV(0)` forever and the energy was fabricated; see
    /// `docs/plans/low-clamp-reversal.md`.
    ///
    /// It is *state*, not a cache: a restored snapshot without it continues a different
    /// trajectory, which is what makes it a semantic [`crate::SNAPSHOT_VERSION`] bump.
    #[serde(default)]
    pub soc_deficit: f64,
    /// RC-pair overpotentials \[V\], discharge-positive; one entry per RC pair.
    pub v_rc: Vec<f64>,
    /// Cell temperature \[K\]. Advanced by [`crate::thermal`] unless the pack is
    /// configured [`crate::ThermalConfig::Isothermal`], in which case it holds its
    /// initial value.
    pub temp_k: f64,
}

/// Cell-model slot. Enum dispatch (not trait objects) keeps state serde-friendly.
///
/// The two equivalent-circuit variants share [`EcmState`]; the variant records the
/// RC-pair count. [`CellModel::Spm`] is the first porous-electrode model, added in
/// Phase 6 without touching the pack layer — which was the claim slice A's accessor
/// removal existed to make checkable, and this variant is what cashes it.
///
/// # This enum is the whole boundary
/// `CLAUDE.md` requires that nothing outside this enum assume ECM-only internals.
/// That is enforced by shape rather than by convention: there is **no accessor
/// returning [`EcmState`]**, so the pack layer physically cannot read `v_rc` or
/// write `soc`. Everything it needs is one of the methods below, and adding a
/// non-ECM variant means adding arms to them and nothing else.
///
/// Each method's ECM arm is deliberately the *original expression, moved* rather
/// than re-derived. `heat_w` is the one where that matters most: the irreversible
/// heat is `I·(OCV − V)` in principle and `I·(I·R0 + Σ V_rc)` in this code, and
/// while those agree algebraically they do **not** agree bit-for-bit, so
/// generalizing the formula rather than relocating it would move every ECM
/// trajectory in the repo. That constraint outlived slice A: the arguments the
/// `Spm` arm needed — the effective capacity, and the terminal voltage the solve
/// produced — were **added** to these signatures rather than the ECM arms being
/// rewritten to use them. See `docs/plans/phase-6-porous-electrodes.md`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CellModel {
    /// Single RC pair.
    Ecm1Rc(EcmState),
    /// Two RC pairs.
    Ecm2Rc(EcmState),
    /// Single-particle porous-electrode model. See [`crate::spm`].
    ///
    /// **Boxed, and every ECM pack is why.** An enum is as wide as its largest variant,
    /// so an un-boxed `DfnState` (136 B) made *every* cell in *every* pack 136 bytes of
    /// cell-model slot where [`EcmState`] needs 48 — 88 bytes per cell of padding paid
    /// by packs that will never build a porous-electrode cell. The indirection costs a
    /// porous model one dependent load against a step that costs ≈ 1 µs (`Spm`, 20
    /// shells) to ≈ 180 µs (`Dfn`) **per cell**, which is why the trade goes this way and
    /// not the other. `Box<T>` is serde-transparent, so no saved pack changed shape and
    /// [`crate::SNAPSHOT_VERSION`] did not move for it. See `docs/plans/cell-size.md`.
    Spm(Box<SpmState>),
    /// Doyle–Fuller–Newman model: the single-particle model with the electrolyte solved
    /// for rather than held constant. See [`crate::dfn`].
    ///
    /// Boxed for the reason on [`CellModel::Spm`] — this is the variant that was setting
    /// the enum's width.
    Dfn(Box<DfnState>),
}

impl CellModel {
    /// A fresh equivalent-circuit cell with `n_rc` RC pairs (1 or 2), at rest.
    ///
    /// Every overpotential starts at zero: a cell that has never carried current
    /// has no polarization to relax.
    pub(crate) fn new_ecm(n_rc: usize, soc: f64, temp_k: f64) -> Self {
        let state = EcmState {
            soc,
            // A fresh cell has never been over-discharged. `PackConfig::initial_soc` is
            // validated into [0, 1], so there is no way to *build* a pack already in
            // reversal — it has to be driven there.
            soc_deficit: 0.0,
            v_rc: vec![0.0; n_rc],
            temp_k,
        };
        // `ChemistryParams::validate` guarantees 1 or 2 RC pairs, so the `else`
        // arm is `n_rc == 2` and not a silent default.
        if n_rc == 1 {
            CellModel::Ecm1Rc(state)
        } else {
            CellModel::Ecm2Rc(state)
        }
    }

    /// A fresh single-particle cell with `shells` finite volumes per particle, at
    /// rest and uniform. See [`SpmState::new`].
    ///
    /// `spm` is the chemistry's `[spm]` block; [`crate::Pack::new`] refuses to
    /// select this model against a chemistry that has none, so there is no arm here
    /// for a missing section.
    pub(crate) fn new_spm(spm: &SpmParams, shells: usize, soc: f64, temp_k: f64) -> Self {
        CellModel::Spm(Box::new(SpmState::new(spm, shells, soc, temp_k)))
    }

    /// A fresh Doyle–Fuller–Newman cell: `nodes` finite volumes across
    /// `(negative, separator, positive)` and `shells` per particle, at rest and uniform.
    /// See [`DfnState::new`].
    ///
    /// Requires **both** the chemistry's `[spm]` and `[dfn]` blocks;
    /// [`crate::Pack::new`] refuses the combination against a chemistry missing either,
    /// naming which one, so there is no arm here for a missing section.
    pub(crate) fn new_dfn(
        spm: &SpmParams,
        nodes: (usize, usize, usize),
        shells: usize,
        soc: f64,
        temp_k: f64,
    ) -> Self {
        CellModel::Dfn(Box::new(DfnState::new(spm, nodes, shells, soc, temp_k)))
    }

    /// The chemistry's `[spm]` block, for the arms that need it.
    ///
    /// A missing section cannot happen on a built pack — [`crate::Pack::new`]
    /// rejects the combination — but `step` may not panic, so the fallback is a
    /// value rather than an `expect`. It is unreachable by construction and the
    /// tests that prove the build error say so.
    fn spm_params(chem: &ChemistryParams) -> Option<&SpmParams> {
        chem.spm.as_ref()
    }

    /// The chemistry's `[spm]` **and** `[dfn]` blocks, which a DFN cell needs together:
    /// the electrode geometry, kinetics and OCPs live in the first and only the
    /// electrolyte in the second. Same unreachable-by-construction argument as
    /// [`Self::spm_params`], now over a pair.
    fn dfn_params(chem: &ChemistryParams) -> Option<(&SpmParams, &DfnParams)> {
        Some((chem.spm.as_ref()?, chem.dfn.as_ref()?))
    }

    /// Ground-truth state of charge, in \[0, 1\].
    ///
    /// For an equivalent circuit this is the coulomb-counted state variable
    /// itself. A porous-electrode model has no such variable and *derives* it from
    /// the lithium actually in its particles — which is why the pack reads it
    /// through a method rather than a field, and why the method needs the
    /// chemistry: the mapping from stoichiometry to SOC is the electrode's usable
    /// window, and that is chemistry data.
    #[must_use]
    pub fn soc(&self, chem: &ChemistryParams) -> f64 {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s.soc,
            CellModel::Spm(s) => Self::spm_params(chem).map_or(0.0, |spm| spm::soc(s, spm)),
            CellModel::Dfn(s) => Self::spm_params(chem).map_or(0.0, |spm| dfn::soc(s, spm)),
        }
    }

    /// How far past empty this cell has been driven, as a fraction of its capacity.
    ///
    /// `0.0` for a porous-electrode model, and that is physics rather than a stub: those
    /// models never clamp, so there is no truncation for a deficit to record. Their
    /// lithium simply keeps moving and [`Self::soc`] reports the readout leaving its
    /// window — which is now what `SOC_CLAMPED_LOW` means for every model. See
    /// [`EcmState::soc_deficit`].
    #[must_use]
    pub fn soc_deficit(&self) -> f64 {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s.soc_deficit,
            CellModel::Spm(_) | CellModel::Dfn(_) => 0.0,
        }
    }

    /// Cell temperature \[K\].
    #[must_use]
    pub fn temp_k(&self) -> f64 {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s.temp_k,
            CellModel::Spm(s) => s.temp_k,
            CellModel::Dfn(s) => s.temp_k,
        }
    }

    /// Overwrite the cell temperature \[K\].
    ///
    /// Temperature is owned by [`crate::thermal`], not by the cell model — every
    /// model carries it and none of them integrate it.
    pub(crate) fn set_temp_k(&mut self, temp_k: f64) {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s.temp_k = temp_k,
            CellModel::Spm(s) => s.temp_k = temp_k,
            CellModel::Dfn(s) => s.temp_k = temp_k,
        }
    }

    /// Total overpotential \[V\] across the cell's internal dynamics,
    /// discharge-positive — everything between the cell's equilibrium voltage and
    /// the terminal that is *not* the instantaneous ohmic drop.
    ///
    /// For an ECM that is `Σ V_rc`; for a single-particle cell it is the
    /// concentration and Butler–Volmer overpotentials together. The name is
    /// model-neutral on purpose: it is the quantity [`crate::CellView`] reports, and
    /// every cell model has one.
    #[must_use]
    pub fn overpotential_v(
        &self,
        chem: &ChemistryParams,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
    ) -> f64 {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => s.v_rc.iter().sum(),
            CellModel::Spm(s) => Self::spm_params(chem).map_or(0.0, |spm| {
                spm::overpotential_v(s, spm, eff_r0_factor, eff_capacity_ah)
            }),
            CellModel::Dfn(s) => Self::dfn_params(chem).map_or(0.0, |(spm, d)| {
                dfn::overpotential_v(s, spm, d, eff_r0_factor, eff_capacity_ah)
            }),
        }
    }

    /// Bulk minus surface stoichiometry on each electrode, `(negative, positive)`, both
    /// discharge-positive and on the scale [`crate::CellView::soc`] uses — or `None` on a
    /// model that has no surface.
    ///
    /// The concentration gradient itself, where [`Self::overpotential_v`] is the voltage
    /// it costs. That one is model-neutral because every cell model has an overpotential;
    /// this one is not, and the difference is the point of it.
    ///
    /// # `None` rather than `0.0` for an equivalent circuit
    /// An ECM has no electrodes, no particles and no surface — not a flat gradient, no
    /// gradient. `0.0` would be indistinguishable, to a client that plots it, from a real
    /// measurement of a fully relaxed porous cell, which is *precisely* the trap the
    /// `v_rc_sum` → `overpotential_v` rename was paid to remove. `None` is also what a
    /// porous cell configured against a chemistry with no `[spm]`/`[dfn]` section
    /// answers: no parameters, no gradient, and no invented zero.
    ///
    /// Takes no `eff_r0_factor`, unlike its siblings: see [`crate::spm::surface_gap`] for
    /// why resistance growth cannot reach a diffusion gradient while `eff_capacity_ah`
    /// can.
    #[must_use]
    pub fn surface_gap(&self, chem: &ChemistryParams, eff_capacity_ah: f64) -> Option<(f64, f64)> {
        match self {
            CellModel::Ecm1Rc(_) | CellModel::Ecm2Rc(_) => None,
            CellModel::Spm(s) => {
                Self::spm_params(chem).map(|spm| spm::surface_gap(s, spm, eff_capacity_ah))
            }
            CellModel::Dfn(s) => {
                Self::dfn_params(chem).map(|(spm, d)| dfn::surface_gap(s, spm, d, eff_capacity_ah))
            }
        }
    }

    /// This cell's Thévenin source `(E, R)` for the pack's linear solve, from its
    /// start-of-step state. See [`cell_source`].
    ///
    /// For a linear model this is exact and the pack's closed-form solve is
    /// exact with it. A nonlinear model returns a **tangent** here, and the pack
    /// iterates — but the ECM arm must keep answering with `cell_source`'s own
    /// expression rather than being reconstructed from an evaluated voltage, or
    /// the closed-form path stops being bit-identical to itself.
    ///
    /// `eff_r0_factor` is the cell's static resistance multiplier × aging's
    /// `soh_resistance`; `eff_capacity_ah` is its nominal capacity × its static
    /// capacity multiplier × aging's `soh_capacity`. The equivalent circuit ignores
    /// the second — its source has no capacity in it — and a porous-electrode cell
    /// needs it, because the flux a current produces depends on how much active
    /// material the cell is configured to hold.
    #[must_use]
    pub(crate) fn source(
        &self,
        chem: &ChemistryParams,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
    ) -> (f64, f64) {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => cell_source(s, chem, eff_r0_factor),
            CellModel::Spm(s) => Self::spm_params(chem).map_or((0.0, 1.0), |spm| {
                spm::source(s, spm, eff_r0_factor, eff_capacity_ah)
            }),
            CellModel::Dfn(s) => Self::dfn_params(chem).map_or((0.0, 1.0), |(spm, d)| {
                dfn::source(s, spm, d, eff_r0_factor, eff_capacity_ah)
            }),
        }
    }

    /// Where this cell's curve is at current `i` \[A, discharge-positive\] over a step of
    /// `dt` seconds, and the straight line touching it there: `(V(i), (E, R))`.
    ///
    /// The pack's nonlinear iteration needs both at the same operating point — the curve
    /// to measure its aggregate against, and the tangent to aggregate from on the next
    /// pass — and this returns them from one evaluation. **That is a cost decision, not
    /// tidiness.** Two calls at one current are two evaluations, and for
    /// [`crate::dfn::probe_at`] an evaluation is a coupled nonlinear solve. For every arm
    /// the merged answer is bit-for-bit what the two separate ones were.
    ///
    /// The equivalent circuit **ignores both `i` and `dt`**, and that is the point rather
    /// than an omission: a linear cell's Thévenin source is the same line at every
    /// current, so the tangent at any operating point is `cell_source`'s existing
    /// expression, byte for byte, and `V` is that line evaluated. That is what makes the
    /// pack's nonlinear iteration collapse to today's closed form on an
    /// all-equivalent-circuit pack — see [`Self::is_linear`], which is the flag the pack
    /// actually branches on.
    ///
    /// `dt` is here for the DFN alone, and it is the argument that made this a *contract*
    /// change rather than a merge: an equivalent circuit's and a single particle's
    /// `V(i)` are start-of-state readouts with no step length in them, while a DFN's is
    /// the backward-Euler solve over the step. See [`crate::dfn::probe_at`] for what it
    /// does with `dt <= 0`, which is the path a zero-length probe step takes.
    ///
    /// Not memoisable: `i` is an in-flight iterate, not state. See
    /// [`crate::spm::source_at`].
    #[must_use]
    pub(crate) fn probe_at(
        &self,
        chem: &ChemistryParams,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
        i: f64,
        dt: f64,
    ) -> (f64, (f64, f64)) {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => {
                let (e, r) = cell_source(s, chem, eff_r0_factor);
                (e - i * r, (e, r))
            }
            CellModel::Spm(s) => Self::spm_params(chem).map_or((0.0, (0.0, 1.0)), |spm| {
                spm::probe_at(s, spm, eff_r0_factor, eff_capacity_ah, i)
            }),
            CellModel::Dfn(s) => Self::dfn_params(chem).map_or((0.0, (0.0, 1.0)), |(spm, d)| {
                dfn::probe_at(s, spm, d, eff_r0_factor, eff_capacity_ah, i, dt)
            }),
        }
    }

    /// Whether this cell's terminal voltage is a straight line in its current over
    /// the step — so that [`Self::source`] is *exact* rather than a tangent.
    ///
    /// The pack branches on this and on nothing else. When every cell answers `true`
    /// the aggregated Thévenin is exact too, the closed-form solve is the whole
    /// answer, and the iteration exits on its first pass having done literally the
    /// arithmetic Phase 1 did. Deciding that structurally rather than by measuring a
    /// residual is deliberate: `E − ((E − V)/R)·R` is not bit-identically `V`, so a
    /// tolerance-gated exit would leave the equivalent circuit's bit-identity resting
    /// on the tolerance instead of on the algebra.
    #[must_use]
    pub(crate) fn is_linear(&self) -> bool {
        match self {
            CellModel::Ecm1Rc(_) | CellModel::Ecm2Rc(_) => true,
            CellModel::Spm(_) | CellModel::Dfn(_) => false,
        }
    }

    /// Heat generated inside this cell \[W\] at current `i`
    /// \[A, discharge-positive\] and effective resistance `r` \[ohms\], from its
    /// start-of-step state. See [`cell_heat_w`].
    ///
    /// `v_terminal` is the node voltage the pack's solve settled on for this cell.
    /// The equivalent circuit does not read it: its heat is
    /// `I·(I·R0 + Σ V_rc)`, which is `I·(OCV − V)` algebraically and **not**
    /// bit-for-bit, so passing the general form through the ECM arm would move
    /// every trajectory in the repo. A model with neither an `R0` nor an RC pair has
    /// only the general form available, and this argument is how it gets it.
    #[must_use]
    pub(crate) fn heat_w(
        &self,
        chem: &ChemistryParams,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
        i: f64,
        r: f64,
        v_terminal: f64,
    ) -> f64 {
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => cell_heat_w(
                i,
                r,
                s.v_rc.iter().sum::<f64>(),
                s.temp_k,
                docv_dt_lookup(&chem.ocv, s.soc),
            ),
            CellModel::Spm(s) => Self::spm_params(chem).map_or(0.0, |spm| {
                spm::heat_w(s, spm, eff_r0_factor, eff_capacity_ah, i, v_terminal)
            }),
            CellModel::Dfn(s) => {
                Self::spm_params(chem).map_or(0.0, |spm| dfn::heat_w(s, spm, i, v_terminal))
            }
        }
    }

    /// Advance this cell's internal state by `dt` seconds under the current the
    /// pack solve assigned it. See [`advance_cell`].
    ///
    /// `eff_r0_factor` is ignored by every arm but the DFN's, and is here on the same
    /// terms `eff_capacity_ah` arrived on one phase earlier: the argument a new model
    /// needs is **added** to the signature rather than the existing arms being rewritten
    /// around it. A DFN's state update *is* its voltage solve, so unlike an SPM's
    /// diffusion it genuinely depends on the resistance multiplier — which reaches its
    /// kinetics and its contact resistance.
    ///
    /// `soh_resistance` arrives **split out** of `eff_r0_factor` rather than folded into
    /// it, mirroring `soh_capacity` beside `eff_capacity_ah`, and the split carries
    /// meaning: the equivalent circuit's RC pairs grow with *aging's* resistance factor
    /// and deliberately **not** with the cell's static `r0_factor`, which is manufacturing
    /// scatter and the `WeakCell` fault and is named for `R0` in the public config. The
    /// DFN arm wants the product and takes `eff_r0_factor` unchanged.
    /// See `docs/plans/rc-resistance-growth.md`.
    #[must_use]
    // The eighth argument is what trips this, and bundling the four multipliers into a
    // struct to silence it would hide the one thing this signature exists to say: which
    // factors arrive combined and which arrive split. Each arm takes a different
    // combination, deliberately, and the compiler checking that at every call site is
    // worth more here than an argument count.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn advance(
        &mut self,
        chem: &ChemistryParams,
        i: f64,
        dt: f64,
        eff_r0_factor: f64,
        eff_capacity_ah: f64,
        soh_capacity: f64,
        soh_resistance: f64,
    ) -> Advanced {
        // Only the equivalent circuit can reject charge, so only its arm carries a
        // non-zero amount out. The porous-electrode arms are wrapped here rather than
        // each growing a return type, which keeps `spm::advance`/`dfn::advance`
        // answering exactly what they answered before.
        let no_rejection = |flags| Advanced {
            flags,
            rejected_as: 0.0,
        };
        match self {
            CellModel::Ecm1Rc(s) | CellModel::Ecm2Rc(s) => advance_cell(
                s,
                chem,
                i,
                dt,
                eff_capacity_ah,
                soh_capacity,
                soh_resistance,
            ),
            // The two multipliers arrive here split, because `soc_true`'s contract
            // makes the split meaningful to the equivalent circuit. A single-particle
            // cell wants only their product: it has no SOC scale to preserve, just an
            // amount of lithium the geometry has to be reconciled against.
            CellModel::Spm(s) => {
                no_rejection(chem.spm.as_ref().map_or(EventFlags::empty(), |spm| {
                    spm::advance(s, spm, i, dt, eff_capacity_ah * soh_capacity)
                }))
            }
            CellModel::Dfn(s) => no_rejection(Self::dfn_params(chem).map_or(
                EventFlags::empty(),
                |(spm, d)| {
                    dfn::advance(
                        s,
                        spm,
                        d,
                        i,
                        dt,
                        eff_r0_factor,
                        eff_capacity_ah * soh_capacity,
                    )
                },
            )),
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
pub(crate) fn interp1(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    debug_assert!(!xs.is_empty() && xs.len() == ys.len());
    lerp_at(ys, bracket(xs, x))
}

/// `d(ys)/d(xs)` of [`interp1`] at `x`: the slope of the segment the lookup lands in, and
/// exactly `0.0` outside the breakpoints, where the lookup clamps.
///
/// Shares [`bracket`] with `interp1` rather than re-deriving the segment, so the value and
/// its derivative can never disagree about which segment they are on. A clamped end returns
/// `lo == hi`, which is the "take the endpoint verbatim" case — a constant, hence zero.
#[must_use]
pub(crate) fn interp1_slope(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    debug_assert!(!xs.is_empty() && xs.len() == ys.len());
    let (lo, hi, _) = bracket(xs, x);
    if lo == hi {
        0.0
    } else {
        (ys[hi] - ys[lo]) / (xs[hi] - xs[lo])
    }
}

/// Open-circuit voltage \[V\] at the given SOC, by clamped linear interpolation.
#[must_use]
pub fn ocv_lookup(table: &OcvTable, soc: f64) -> f64 {
    interp1(&table.soc, &table.volts, soc)
}

/// Invert the OCV table: the SOC whose open-circuit voltage is `v`, together with
/// how steep the curve is there.
///
/// Returns `(soc, dv_dsoc)`. The slope is the **confidence** in the answer, and
/// callers are expected to use it: a BMS correcting its SOC estimate from a rested
/// OCV reading learns a lot on a steep knee and almost nothing on a plateau, where a
/// millivolt of sensor error maps to a huge SOC interval. On a perfectly flat
/// segment the inverse does not exist at all — this returns that segment's lower
/// breakpoint and a slope of exactly `0.0`, which a caller must read as "no
/// information", not as an estimate. That is not a degenerate corner case: it is
/// most of an LFP discharge curve.
///
/// `v` outside the table clamps to the corresponding end of the SOC range, reporting
/// the adjacent segment's slope (so a correction at a steep end is still allowed).
/// The table is non-decreasing in `volts` by validation, which is what makes the
/// search well defined.
#[must_use]
pub fn ocv_invert(table: &OcvTable, v: f64) -> (f64, f64) {
    let (soc, volts) = (&table.soc, &table.volts);
    let n = volts.len();
    debug_assert!(n > 0 && n == soc.len());
    if n == 1 {
        return (soc[0], 0.0);
    }
    // First breakpoint whose voltage reaches `v`; the clamp handles both ends and,
    // as in `bracket`, a NaN needle (every comparison false ⇒ index 0).
    let hi = volts.partition_point(|&x| x < v).clamp(1, n - 1);
    let lo = hi - 1;
    let span_v = volts[hi] - volts[lo];
    let span_soc = soc[hi] - soc[lo]; // > 0: soc is strictly ascending by validation
    if span_v > 0.0 {
        let frac = ((v - volts[lo]) / span_v).clamp(0.0, 1.0);
        (soc[lo] + frac * span_soc, span_v / span_soc)
    } else {
        (soc[lo], 0.0)
    }
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

/// What one coulomb-counting step did, including the charge it could not account for.
///
/// The third field is the whole reason this is a struct rather than a pair: a clamped
/// step moves less charge than the current that produced it implies, and the difference
/// has to leave this function or it is destroyed silently (see
/// `docs/plans/energy-hole.md`).
#[derive(Clone, Copy, Debug)]
pub struct CoulombStep {
    /// New state of charge, clamped to \[0, 1\].
    pub soc: f64,
    /// New deficit below empty, as a fraction of capacity. See
    /// [`EcmState::soc_deficit`], whose invariant this upholds: non-zero only when
    /// [`Self::soc`] is `0.0`.
    pub soc_deficit: f64,
    /// Charge \[As, discharge-positive\] that crossed the terminals over this step
    /// without changing the stored charge.
    ///
    /// Exactly `0.0` on any step that did not clamp, and **negative** at the upper clamp
    /// (charge pushed in and refused), so that
    /// `stored change = −(i − i_rejected)·dt / capacity_as` holds through that clamp as
    /// well as away from one.
    ///
    /// **The lower clamp no longer contributes to it.** Charge drawn past empty is not
    /// rejected — it is carried in [`Self::soc_deficit`] and repaid on the way back up,
    /// which is what stops the cell inventing the energy it delivers there. This field
    /// is therefore a one-sided quantity now (`<= 0`), and the asymmetry is the
    /// deliberate one described on [`crate::Telemetry::i_rejected_a`].
    ///
    /// This is the *rejected fraction*, not the whole step's charge: on the step where
    /// the clamp is first entered only part of the current is refused, and reporting the
    /// whole of it would be wrong at every clamp entry while still passing every
    /// tolerance in the suite.
    pub rejected_as: f64,
    /// `SOC_CLAMPED_HIGH`/`_LOW` when the raw update ran past a bound.
    pub flags: EventFlags,
}

/// Coulomb-counting SOC advance over `dt` seconds.
///
/// Runs on the cell's **extended position** `soc − deficit`, not on `soc`:
/// `x' = (soc − deficit) − I·dt / (3600·capacity_ah·soh_capacity)`. Above zero that is
/// the ordinary count and `deficit` is `0`; below it the shortfall is carried out as
/// [`CoulombStep::soc_deficit`] rather than discarded, so the charge a reversed cell
/// delivers is accounted for and repaid when it is charged again.
///
/// The upper bound still clamps hard and still reports what it refused — see
/// [`CoulombStep::rejected_as`] and `docs/plans/low-clamp-reversal.md` for why the two
/// ends of the window are deliberately not symmetric.
#[must_use]
pub fn coulomb_step(
    soc: f64,
    deficit: f64,
    i: f64,
    dt: f64,
    capacity_ah: f64,
    soh_capacity: f64,
) -> CoulombStep {
    let capacity_as = 3600.0 * capacity_ah * soh_capacity; // amp-seconds
    let raw = (soc - deficit) - i * dt / capacity_as;
    if raw > 1.0 {
        return CoulombStep {
            soc: 1.0,
            soc_deficit: 0.0,
            // Negative: the refused charge was flowing *in*. `(raw − 1)` is the
            // fraction of the cell's capacity that did not fit.
            rejected_as: -(raw - 1.0) * capacity_as,
            flags: EventFlags::SOC_CLAMPED_HIGH,
        };
    }
    if raw < 0.0 {
        return CoulombStep {
            soc: 0.0,
            // Carried, not rejected. The reported `soc` is still clamped — a client
            // asking "how full is this cell" gets 0 — but the engine remembers how far
            // past empty it went, which is what [`cell_source`] turns into a falling
            // open-circuit voltage.
            soc_deficit: -raw,
            rejected_as: 0.0,
            flags: EventFlags::SOC_CLAMPED_LOW,
        };
    }
    CoulombStep {
        soc: raw,
        soc_deficit: 0.0,
        rejected_as: 0.0,
        flags: EventFlags::empty(),
    }
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
/// `r0_factor` is the cell's **effective** resistance multiplier: its static
/// manufacturing scatter / weak-cell factor times aging's `soh_resistance`. The pack
/// composes the two and guarantees the product is `> 0`, so `r > 0`.
#[must_use]
pub(crate) fn cell_source(state: &EcmState, chem: &ChemistryParams, r0_factor: f64) -> (f64, f64) {
    let r = r0_lookup(&chem.r0, state.soc, state.temp_k) * r0_factor;
    let e = open_circuit_v(chem, state) - state.v_rc.iter().sum::<f64>();
    (e, r)
}

/// Open-circuit voltage \[V\] of a cell that may be below empty: the chemistry's OCV
/// table above `soc = 0`, and the `[reversal]` ramp under it.
///
/// ```text
/// OCV_eff = max(OCV(soc) − v_per_soc · soc_deficit,  floor_v)
/// ```
///
/// # Why this is a state and not a branch
/// Three earlier candidates for the bottom of the window each collapsed the cell's
/// voltage from *within* the step — keyed on the sign of the current, or on a blocking
/// resistance — and each broke something: a strict diode makes a parallel group's
/// `Σ 1/R` zero and the aggregation returns `-inf`; a direction-blind collapse draws a
/// spurious −180 A from a charger; a direction-aware one is a kink that pins the
/// nonlinear solve at its 32-pass cap. All three are `docs/plans/low-clamp-solve-side.md`.
///
/// This reads only `soc_deficit`, which the *previous* step wrote. Within the step the
/// cell is still a fixed line, so [`crate::CellModel::is_linear`] stays `true`, the pack
/// solve stays one closed-form pass, and nothing here allocates or iterates. The
/// non-linearity is spread across steps instead of packed inside one, which is the same
/// trick the exact RC update uses.
///
/// The clamp is `max`, so a `floor_v` at or above `OCV(0)` would make the branch inert;
/// `ChemistryParams::validate` refuses that configuration rather than shipping a cell
/// that silently sources forever.
#[must_use]
pub(crate) fn open_circuit_v(chem: &ChemistryParams, state: &EcmState) -> f64 {
    let ocv = ocv_lookup(&chem.ocv, state.soc);
    if state.soc_deficit == 0.0 {
        // The overwhelmingly common case, and written as an early return so that a cell
        // inside its window takes bit-for-bit the expression it took before this
        // branch existed. `max` of a value against a floor below it is that value, so
        // the general path would agree — but only for a floor the validator happens to
        // guarantee, and "agrees today" is not the same as "cannot move a trajectory".
        return ocv;
    }
    (ocv - chem.reversal.v_per_soc * state.soc_deficit).max(chem.reversal.floor_v)
}

/// What advancing one cell by a step produced, beyond the state change itself.
///
/// Two fields because the pack needs both and neither is derivable from the other: the
/// flags it merges into its own set, and the charge the cell could not account for,
/// which the pack turns into heat and reports as
/// [`crate::Telemetry::i_rejected_a`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Advanced {
    /// Flags raised by the advance (currently the SOC clamps).
    pub flags: EventFlags,
    /// Charge \[As, discharge-positive\] that crossed the terminals without changing
    /// stored charge. See [`CoulombStep::rejected_as`].
    ///
    /// Always `0.0` for `Spm` and `Dfn`, and that is physics rather than a stub: a
    /// porous-electrode cell never discards the lithium it was pushed, so there is
    /// nothing for it to reject. See [`crate::spm::advance`]. Since the reversal branch
    /// the equivalent circuit no longer discards anything at the *bottom* of its window
    /// either, so this is non-zero only on an over-charge.
    pub rejected_as: f64,
}

/// Advance one cell's internal state by `dt` seconds under the current `i`
/// \[A, discharge-positive\] that the pack solve assigned it.
///
/// Updates every RC overpotential (exact exponential update) and SOC (coulomb
/// counting). `eff_capacity_ah` is the cell's capacity after its *static* factor
/// (nominal × capacity_factor); `soh_capacity` is aging's dynamic multiplier on top,
/// kept as a separate argument so that SOC keeps meaning "fraction of the capacity
/// this cell has **today**" — an aged cell empties sooner without its SOC scale
/// changing. `soh_resistance` is aging's resistance-growth factor, and grows the RC
/// pairs exactly as it grows `R0` (see below). Returns the SOC-clamp flags from the
/// coulomb step. Terminal voltage is *not* returned here: the pack recomputes each
/// group's shared node voltage from the end-of-step state via [`cell_source`] so
/// parallel cells report one consistent voltage.
#[must_use]
pub(crate) fn advance_cell(
    state: &mut EcmState,
    chem: &ChemistryParams,
    i: f64,
    dt: f64,
    eff_capacity_ah: f64,
    soh_capacity: f64,
    soh_resistance: f64,
) -> Advanced {
    for (k, v_rc) in state.v_rc.iter_mut().enumerate() {
        let pair = chem.rc[k];
        // Aging grows the slow resistances along with the instant one, which is what
        // `CLAUDE.md`'s physics spec has always said and what this line did not do until
        // `docs/plans/rc-resistance-growth.md`. Three things about the expression:
        //
        // * The factor is `soh_resistance` **alone**, not the cell's `eff_r0_factor`.
        //   Manufacturing scatter and the `WeakCell` fault are `R0` multipliers by name
        //   in the public config, and folding them in here would move fresh packs — five
        //   shipped scenarios set `r0_sigma`.
        // * `c_farad` is left alone, so `tau = r·c` grows with age too: an aged cell
        //   relaxes slower as well as further. The spec says *resistances* and says
        //   nothing about capacitance, so this follows from the sentence rather than
        //   adding a claim to it.
        // * The multiply is here rather than inside `rc_update`, whose signature is
        //   `pub` and directly tested. It is also unconditional: at `soh_resistance ==
        //   1.0` — every pack without aging, and every aged pack before its first
        //   tick — `x * 1.0` is bit-identical, so a branch would guard nothing and cost
        //   what the multiply costs.
        *v_rc = rc_update(*v_rc, i, pair.r_ohms * soh_resistance, pair.c_farad, dt);
    }
    let step = coulomb_step(
        state.soc,
        state.soc_deficit,
        i,
        dt,
        eff_capacity_ah,
        soh_capacity,
    );
    state.soc = step.soc;
    state.soc_deficit = step.soc_deficit;
    Advanced {
        flags: step.flags,
        rejected_as: step.rejected_as,
    }
}
