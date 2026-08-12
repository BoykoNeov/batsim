//! Chemistry parameters: the data-driven description of a single cell type.
//!
//! A chemistry is *data*, never code (see `CLAUDE.md`). These structs are the
//! in-memory form of a `chemistries/*.toml` file. They derive [`serde`]
//! (de)serialization so `sim-data` can parse TOML directly into
//! [`ChemistryParams`]; format-specific parsing (the `toml` crate) lives in
//! `sim-data`, not here.
//!
//! All quantities are SI: seconds, amperes, volts, ohms, farads, kelvin.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// True iff `x` is strictly positive. NaN yields `false`, so `!is_positive(x)`
/// rejects NaN as well as non-positive values (and reads clear of clippy's
/// negated-comparison lint).
#[inline]
fn is_positive(x: f64) -> bool {
    x > 0.0
}

/// True iff `x` is zero or positive. NaN yields `false`, so `!is_non_negative(x)`
/// rejects NaN as well as negative values.
#[inline]
fn is_non_negative(x: f64) -> bool {
    x >= 0.0
}

/// Full parameter set for one cell chemistry.
///
/// The field grouping mirrors the TOML section layout (`[meta]`, `[cell]`,
/// `[ocv]`, `[r0]`, `[[rc]]`, `[thermal]`, `[aging]`, `[safety]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChemistryParams {
    /// Identity and provenance (`[meta]`).
    pub meta: ChemMeta,
    /// Per-cell limits and nominal capacity (`[cell]`).
    pub cell: CellLimits,
    /// Open-circuit-voltage lookup table (`[ocv]`).
    pub ocv: OcvTable,
    /// Ohmic series resistance table over (soc, temperature) (`[r0]`).
    pub r0: R0Table,
    /// 1–2 RC (Thevenin) pairs (`[[rc]]`).
    pub rc: Vec<RcPair>,
    /// Lumped thermal properties of one cell (`[thermal]`).
    pub thermal: ThermalParams,
    /// How an equivalent-circuit cell behaves below empty (`[reversal]`).
    pub reversal: ReversalParams,
    /// Diffusion overpotential (`[diffusion]`), or `None` for a chemistry whose
    /// equivalent circuit carries no such term.
    ///
    /// `None` is the common case and the case for every chemistry shipped before this
    /// section existed. It is not "this cell has no diffusion limit" — the RC pairs are
    /// already a diffusion transient — it is "this parameter set does not describe one
    /// that **depletes**, so a hard discharge costs this cell no capacity beyond the
    /// ohmic sag". For lithium at the rates these files cover that is a good
    /// approximation; for lead-acid it is the whole of what the model was missing. See
    /// [`DiffusionParams`] and `docs/plans/diffusion-overpotential.md`.
    ///
    /// Unlike [`Self::aging`] and [`Self::spm`], the absence is **not** diagnosable and
    /// not an error: nothing in [`crate::PackConfig`] can ask for this term, exactly as
    /// nothing can ask for plating. It is a property of the chemistry, switched on by the
    /// chemistry, and a file without it simply never generates one — the same standing
    /// [`Self::safety`] has.
    ///
    /// **The absence is a path, not a multiplier.** [`crate::ecm::ecm_overpotential_v`]
    /// matches on this `Option` and returns `Σ V_rc` unchanged when it is `None`, rather
    /// than adding a neutral zero, so no chemistry without the section can move by so much
    /// as a ULP. That is the same argument [`crate::ecm::open_circuit_v`] makes for a zero
    /// deficit, and it is what makes "LFP and NMC are bit-identical across this version"
    /// a structural claim instead of a measurement.
    #[serde(default)]
    pub diffusion: Option<DiffusionParams>,
    /// Semi-empirical aging coefficients (`[aging]`), or `None` for a chemistry
    /// that carries no aging data.
    ///
    /// `None` is not "this cell does not age" — it is "this parameter set cannot say
    /// how". Configuring a pack with [`crate::aging::AgingConfig`] against such a
    /// chemistry is a build error rather than a silently ageless pack, because
    /// silence there is indistinguishable from coefficients that happen to be zero.
    #[serde(default)]
    pub aging: Option<AgingParams>,
    /// Single-particle-model parameters (`[spm]`), or `None` for a chemistry that
    /// only describes an equivalent circuit.
    ///
    /// `None` is the common case and costs nothing: the ECM sections above fully
    /// describe a cell, and every chemistry shipped before Phase 6 has no `[spm]`
    /// block. Like [`Self::aging`] and unlike [`Self::safety`], the absence is
    /// **diagnosable** — a [`crate::PackConfig`] that selects a porous-electrode
    /// model against a chemistry that cannot parameterize one is a build error, not
    /// a silent fallback to the equivalent circuit. That check belongs with the
    /// config field that can request the model, which this slice does not add; see
    /// `docs/plans/phase-6-porous-electrodes.md`.
    ///
    /// A chemistry may carry **both** an ECM description and this one. That is the
    /// point rather than a redundancy: running the same cell through both models and
    /// watching where the cheap one goes wrong is the pedagogy this phase exists for.
    #[serde(default)]
    pub spm: Option<SpmParams>,
    /// Electrolyte parameters (`[dfn]`) — everything a Doyle–Fuller–Newman cell needs
    /// that a single-particle one does not — or `None` for a chemistry that cannot
    /// parameterize one.
    ///
    /// **This section extends [`Self::spm`]; it does not stand alone.** Every electrode
    /// geometry, kinetic coefficient and OCP table a DFN needs is already in `[spm]`,
    /// and a DFN cell reads both. A file carrying `[dfn]` and no `[spm]` therefore
    /// parameterizes nothing, but it is *not* rejected here: the same rule
    /// [`Self::spm`]'s own absence follows applies, which is that the mismatch is
    /// diagnosed where the configuration asks for the model, with a build error naming
    /// whichever half is missing. Validation on this struct stays local to the block.
    ///
    /// The `[spm]` field this one leans on hardest is
    /// [`SpmParams::c_e_mol_per_m3`], and its meaning changes: for a single-particle
    /// model it is the electrolyte concentration *held constant*, and for a DFN it is
    /// the **initial** value of a field the model then solves for. Same Chen2020 key,
    /// same number, and the difference between those two readings is the entire
    /// difference between the models.
    #[serde(default)]
    pub dfn: Option<DfnParams>,
    /// Emergent-failure thresholds (`[safety]`), or `None` for a chemistry that
    /// carries no safety data.
    ///
    /// Unlike [`Self::aging`], `None` here is **not** a build error, because nothing
    /// in [`crate::PackConfig`] ever asks for plating or runaway — they are emergent,
    /// discovered by the engine from the physics rather than switched on. A chemistry
    /// with no `[safety]` section simply never raises
    /// [`crate::EventFlags::PLATING_RISK`], the same way one with no
    /// [`OcvTable::docv_dt_v_per_k`] column never generates entropic heat. There is no
    /// configuration for the absence to contradict, so there is nothing to diagnose.
    #[serde(default)]
    pub safety: Option<SafetyParams>,
}

/// Thresholds for the two **emergent** failure modes (`[safety]`).
///
/// These are not injected faults. Nothing in this struct scripts an outcome: each
/// number is a threshold or a rate that the ordinary physics is compared against, and
/// the failure — if it happens — falls out of the same equations a healthy pack
/// solves. See [`crate::plating`] for the cold-charge mechanism this slice wires in;
/// the runaway trio is validated here and consumed by the runaway slice.
///
/// Every value in the shipped chemistries is a labelled placeholder. As with
/// [`AgingParams`], scenarios should assert the *shape* of an outcome, never a number
/// on it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafetyParams {
    /// Cell temperature \[K\] above which exothermic self-heating begins. Must be
    /// finite and `> 0`, and below [`Self::t_vent_k`]. See [`crate::runaway`].
    pub t_onset_k: f64,
    /// Cell temperature \[K\] at which a cell vents. Must be finite and above
    /// [`Self::t_onset_k`].
    pub t_vent_k: f64,
    /// Finite exothermic energy budget of one cell \[J\]. Must be finite and `>= 0`.
    ///
    /// Divided by [`ThermalParams::heat_capacity_j_per_k`] this is the **adiabatic
    /// temperature rise** a fully-reacted cell produces, which is the number to sanity
    /// check the placeholder against — the two are only meaningful as a pair, the same
    /// trap [`AgingParams::cal_pre_exp`] and [`AgingParams::cal_ea_j_per_mol`] set.
    pub runaway_energy_j: f64,
    /// Exothermic power \[W\] of one *unreacted* cell held exactly at
    /// [`Self::t_onset_k`]. Must be finite and `>= 0`; `0` (the default) means onset
    /// is reported but produces no heat, so the chemistry can never run away.
    ///
    /// This is the amplitude of the Arrhenius term in [`crate::runaway::reaction_power`];
    /// [`Self::runaway_ea_j_per_mol`] is its steepness. Neither means anything alone.
    #[serde(default)]
    pub runaway_power_w_at_onset: f64,
    /// Activation energy \[J/mol\] of the exothermic decomposition reaction. Must be
    /// finite and `> 0` **when [`Self::runaway_power_w_at_onset`] is positive**;
    /// ignored (and allowed to be the `0.0` default) when it is not.
    ///
    /// Required to be positive with a live reaction because a zero activation energy
    /// makes the release rate temperature-independent — a constant heater, not a
    /// runaway. The accelerating feedback *is* the phenomenon.
    #[serde(default)]
    pub runaway_ea_j_per_mol: f64,
    /// Cell temperature \[K\] below which charging risks plating metallic lithium.
    /// Must be finite and `> 0`.
    ///
    /// Distinct from [`CellLimits::t_charge_min_k`], which is the *BMS's* charge-inhibit
    /// threshold — a policy the protection layer enforces from lagged sensor readings.
    /// This one is physics, evaluated against ground truth. The two coincide in both
    /// shipped chemistries, which is exactly what makes the BMS-on/BMS-off contrast
    /// legible: protection exists to keep the pack out of this region.
    pub t_plating_min_k: f64,
    /// C-rate above which charging below [`Self::t_plating_min_k`] plates. Must be
    /// finite and `>= 0`; `0` means any charge current at all plates when cold.
    pub plating_c_threshold: f64,
    /// Capacity fraction lost per amp-hour of charge carried under plating
    /// conditions. Must be finite and `>= 0`; `0` (the default) means plating is
    /// reported but costs nothing.
    ///
    /// Compare [`AgingParams::cyc_fade_per_ah`]: plating is a *separate, additive*
    /// mechanism, not a multiplier on ordinary cycle fade, because the charge plated
    /// is lost lithium inventory whatever excursion happened to be carrying it. It is
    /// therefore deliberately **not** weighted by depth of discharge.
    #[serde(default)]
    pub plating_fade_per_ah: f64,
    /// Poisson hazard rate \[1/Ah\] of a plating cell developing a soft internal
    /// short, per amp-hour carried under plating conditions. Must be finite and
    /// `>= 0`; `0` (the default) means plating never shorts a cell.
    ///
    /// Per amp-hour *plated*, not per second cold: dendrites grow from deposited
    /// lithium, so a cell sitting cold at rest accrues no hazard at all.
    #[serde(default)]
    pub plating_short_hazard_per_ah: f64,
    /// Leakage resistance \[ohms\] of the soft internal short a plating cell develops.
    /// Must be finite and `> 0` **when [`Self::plating_short_hazard_per_ah`] is
    /// positive**; ignored (and allowed to be the `0.0` default) when it is not, since
    /// then no short can ever form.
    ///
    /// Should be far above the cell's own `R0` — that is what makes it a *soft* short,
    /// draining and heating the cell over hours rather than instantly.
    #[serde(default)]
    pub plating_short_ohms: f64,
}

/// Single-particle-model parameters (`[spm]`).
///
/// Everything a porous-electrode cell needs that an equivalent circuit does not: two
/// half-cell electrodes, the geometry that turns a current into a flux at a particle
/// surface, and the electrolyte concentration the kinetics see.
///
/// # Why these are extracted rather than fitted
/// The ECM tables above are *fitted* to a reference model's output, which is why they
/// carry an interpolation error and a tolerance. These are **read out of a published
/// parameter set** — particle radii, diffusivities and volume fractions are the cell's
/// physical identity, so each one has a literal citation instead of an
/// order-of-magnitude apology. See `tools/reference/extract_spm.py`, which emits this
/// whole block, and `CLAUDE.md`'s provenance rule.
///
/// # What is deliberately absent
/// No electrolyte transport, no potential distribution through the electrode
/// thickness. That is the single-particle approximation: one representative particle
/// per electrode, a uniform reaction rate across it, and an electrolyte held at
/// [`Self::c_e_mol_per_m3`]. A model that relaxes those is `Dfn`, which needs its own
/// parameters and its own section.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpmParams {
    /// Reference temperature \[K\] for the Arrhenius corrections in
    /// [`ElectrodeParams::reaction_ea_j_per_mol`] and
    /// [`ElectrodeParams::diffusivity_ea_j_per_mol`]. Must be finite and `> 0`.
    ///
    /// This is the temperature the transport and kinetic values were *measured* at,
    /// so the correction is exactly `1.0` there by construction.
    pub t_ref_k: f64,
    /// Electrolyte lithium concentration \[mol/m³\], held constant. Must be finite
    /// and `> 0`.
    ///
    /// Constant because that *is* the single-particle approximation — the model has
    /// no electrolyte transport to make it vary. It is not ignorable: it enters the
    /// exchange-current density, so it sets how hard the kinetics push back.
    pub c_e_mol_per_m3: f64,
    /// Geometric electrode plate area \[m²\], shared by both electrodes. Must be
    /// finite and `> 0`.
    ///
    /// Together with each electrode's thickness, volume fraction and particle radius
    /// this gives the interfacial area the reaction current spreads over. Stored as
    /// the geometry rather than as a precomputed area per volume so the numbers stay
    /// the ones a parameter set actually publishes.
    pub electrode_area_m2: f64,
    /// Lumped ohmic resistance \[ohms\] outside the two electrode reactions —
    /// current collectors, tabs, and contact. Must be finite and `>= 0`; `0` (the
    /// default) means the model carries no series resistance at all.
    ///
    /// The single-particle model has no other ohmic term, so this is the only place a
    /// terminal voltage loses volts to something that is not electrode kinetics or
    /// diffusion. Its ECM sibling is the whole [`R0Table`].
    #[serde(default)]
    pub contact_resistance_ohm: f64,
    /// The negative electrode (graphite in every set shipped here).
    pub negative: ElectrodeParams,
    /// The positive electrode.
    pub positive: ElectrodeParams,
}

/// One half-cell electrode of a [`SpmParams`] (`[spm.negative]` / `[spm.positive]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElectrodeParams {
    /// Representative particle radius \[m\]. Must be finite and `> 0`.
    pub particle_radius_m: f64,
    /// Solid-phase lithium diffusivity \[m²/s\] at [`SpmParams::t_ref_k`]. Must be
    /// finite and `> 0`.
    pub diffusivity_m2_per_s: f64,
    /// Maximum lithium concentration in the solid \[mol/m³\] — the concentration at
    /// stoichiometry 1. Must be finite and `> 0`.
    pub c_max_mol_per_m3: f64,
    /// Volume fraction of the electrode that is active material, in (0, 1\]. The
    /// remainder is binder, conductive additive and pore space.
    pub active_volume_fraction: f64,
    /// Electrode coating thickness \[m\]. Must be finite and `> 0`.
    pub thickness_m: f64,
    /// Butler–Volmer reaction-rate coefficient
    /// \[(A·m⁻²)·(m³·mol⁻¹)^1.5\] at [`SpmParams::t_ref_k`]. Must be finite and
    /// `> 0`.
    ///
    /// Named `m_ref` rather than something unit-bearing because that is what the
    /// literature and the parameter sets call it, and a traceable name is worth more
    /// here than a conforming one — the compound unit does not fit in an identifier.
    /// It is the amplitude of `i_0 = m_ref · c_e^0.5 · c_surf^0.5 · (c_max −
    /// c_surf)^0.5`, so the exponents are baked into its units.
    pub m_ref: f64,
    /// Arrhenius activation energy \[J/mol\] for [`Self::m_ref`]. Must be finite and
    /// `>= 0`; `0` (the default) means the kinetics are temperature-independent.
    ///
    /// Applied as `exp(Ea/R · (1/t_ref − 1/T))`, so a positive value makes a cold
    /// cell's kinetics slower — which is most of why a cold cell is a worse battery.
    #[serde(default)]
    pub reaction_ea_j_per_mol: f64,
    /// Arrhenius activation energy \[J/mol\] for [`Self::diffusivity_m2_per_s`],
    /// same form as [`Self::reaction_ea_j_per_mol`]. Must be finite and `>= 0`; `0`
    /// (the default) means diffusion is temperature-independent.
    ///
    /// Defaulting to zero is load-bearing rather than lazy. Chen2020 — the set the
    /// shipped SPM chemistry is extracted from — fits solid diffusivity as a
    /// *constant* at 298.15 K and publishes no activation energy for it, while it
    /// does publish one for the kinetics. Filling this with a plausible number would
    /// be precisely the unlabeled constant `CLAUDE.md` forbids, so the field exists,
    /// the chemistry omits it, and the omission is documented in the file.
    #[serde(default)]
    pub diffusivity_ea_j_per_mol: f64,
    /// Butler–Volmer charge-transfer coefficient, in (0, 1). Must be finite.
    ///
    /// `0.5` — symmetric kinetics — in every parameter set shipped here, which
    /// collapses the Butler–Volmer equation to a `sinh`. It is a parameter anyway
    /// because asymmetric sets exist and the model should not have to be edited to
    /// read one.
    pub charge_transfer_alpha: f64,
    /// Lower end of this electrode's usable stoichiometry window, in \[0, 1\]. Must
    /// be finite.
    ///
    /// **Ordered by stoichiometry, not by state of charge**, and the two are not the
    /// same direction on both electrodes: charging fills the negative electrode while
    /// it empties the positive. So this is the *discharged* cell at the negative
    /// electrode and the *charged* cell at the positive — and reading it as "the
    /// cell's lower voltage cut-off" is right for one electrode and exactly backwards
    /// for the other. Together the two pairs are what map a lithium inventory onto
    /// [`crate::Telemetry::soc_true`], which is why the direction has to be stated
    /// rather than inferred.
    pub stoich_min: f64,
    /// Upper end of this electrode's usable stoichiometry window, in \[0, 1\]. Must be
    /// finite and `> stoich_min`. See [`Self::stoich_min`] for why this is not
    /// "the charged cell" on both electrodes.
    pub stoich_max: f64,
    /// Entropy coefficient `∂U/∂T` \[V/K\] of this electrode's open-circuit
    /// potential. Must be finite; `0` (the default) disables this electrode's
    /// contribution to reversible heat.
    ///
    /// The half-cell sibling of [`OcvTable::docv_dt_v_per_k`], and a scalar rather
    /// than a table because the parameter sets publish it that way. Not
    /// sign-constrained, for the same reason as its full-cell counterpart.
    #[serde(default)]
    pub docp_dt_v_per_k: f64,
    /// Open-circuit potential of this electrode against lithium metal
    /// (`[spm.*.ocp]`).
    pub ocp: OcpTable,
}

/// Half-cell open-circuit potential as a function of stoichiometry (`[spm.*.ocp]`).
///
/// # This table runs downhill, and that is not a typo
/// [`OcvTable`] is monotone **non-decreasing**: a fuller cell has a higher voltage.
/// A half-cell OCP is monotone **non-increasing** in its own stoichiometry — adding
/// lithium to either electrode *lowers* that electrode's potential against lithium
/// metal. The full-cell voltage `U_p(y) − U_n(x)` still rises with state of charge
/// because charging moves the two electrodes in opposite directions: `x` up, `y`
/// down. So validating this table with [`OcvTable`]'s rule would reject a correct
/// extraction, which is why the check is a separate one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcpTable {
    /// Stoichiometry breakpoints, strictly ascending, in \[0, 1\].
    pub stoich: Vec<f64>,
    /// Potential at each breakpoint \[V\], monotone **non-increasing**, same length
    /// as `stoich`.
    pub volts: Vec<f64>,
}

/// Electrolyte parameters for a Doyle–Fuller–Newman cell (`[dfn]`).
///
/// # What a DFN adds, in one sentence
/// An [`SpmParams`] cell holds its electrolyte at one constant concentration — that
/// *is* the single-particle approximation. This section is what it takes to stop doing
/// that: transport properties as functions of concentration, the porous geometry the
/// electrolyte occupies, and the solid-phase conductivities that let the reaction
/// current vary through the electrode thickness.
///
/// Phase 7's spike measured what the assumption costs on the shipped LG M50 set. At C/5
/// and 1C the two models reach the cut-off within 0.3 % of each other; at 3C the DFN
/// reaches it in **51.4 %** of the SPM's time and delivers **2.32 A·h against 4.52**,
/// because the electrolyte starves. It is a cliff between 1C and 3C, not a slope.
///
/// # These are extracted, and the transport fits are stored exactly
/// Like `[spm]` and unlike the ECM tables, every number here is a Chen2020 key or a
/// literal in one of its functions. The two transport properties are the interesting
/// case: PyBaMM publishes them as *callables*, but underneath they are closed-form
/// published fits (Nyman 2008) with no temperature dependence, so they are stored as
/// the power terms they are rather than sampled onto a grid. That matters beyond
/// tidiness — Phase 6 found the OCP tables' 1.88/1.90 mV interpolation error *was* the
/// SPM's accuracy floor, and a second sampled table would have raised it for nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DfnParams {
    /// Cation (Li⁺) transference number `t₊`, in \[0, 1\]. Must be finite.
    ///
    /// The fraction of the ionic current carried by lithium rather than by the anion.
    /// It enters twice: the concentration equation's source is `(1 − t₊)·j/F`, and the
    /// diffusion-potential term in the electrolyte carries the same factor.
    ///
    /// Both endpoints are **allowed**, unlike [`ElectrodeParams::charge_transfer_alpha`]
    /// where 0 and 1 each kill a Butler–Volmer branch. Here neither degenerates
    /// anything: `t₊ = 1` is a single-ion conductor whose concentration simply never
    /// moves, and `t₊ = 0` leaves every term finite. A value outside \[0, 1\] is not a
    /// transference number at all, which is what the range check is for.
    pub transference_number: f64,
    /// Thermodynamic factor `1 + ∂ln f±/∂ln c`, dimensionless. Must be finite and
    /// `> 0`.
    ///
    /// The correction for a non-ideal electrolyte, multiplying the diffusion-potential
    /// term. **Chen2020 publishes it as exactly 1.0**, so on the shipped set the term
    /// reduces to the ideal-solution form and this field earns its keep only for a
    /// parameter set that measures it.
    ///
    /// Rejected at zero and below rather than treated as "off": a non-positive factor
    /// reverses the sign of the concentration overpotential, which makes the
    /// electrolyte push current the way it should resist it. That is a sign error
    /// wearing a parameter's clothes, not a modelling choice.
    pub thermodynamic_factor: f64,
    /// Electrolyte diffusivity `D_e` \[m²/s\] as a sum of power terms in
    /// `x = c_e/1000` (`c_e` in mol/m³). Must be non-empty with finite entries.
    ///
    /// Stored in the fit's own variable, so the shipped coefficients are the published
    /// numbers rather than a rescale of them. See [`PowerTerm`] for why a
    /// `(coefficient, exponent)` pair rather than a coefficient array: the
    /// conductivity fit has an `x^1.5` term and is not a polynomial.
    pub electrolyte_diffusivity_terms: Vec<PowerTerm>,
    /// Electrolyte ionic conductivity `κ_e` \[S/m\] as a sum of power terms in
    /// `x = c_e/1000`. Must be non-empty with finite entries.
    ///
    /// `κ_e → 0` as `c_e → 0` degenerates the electrolyte potential equation, and the
    /// reference genuinely goes there — at 3C on this set, 90.6 % of the run has
    /// `c_e < 100 mol/m³` somewhere. Whatever floor the solver applies to *lookups*
    /// through this fit is a numerical guard that belongs with the solver, not a
    /// number this section may quietly carry: the spike measured a floor of
    /// 100 mol/m³ buying four Newton iterations and paying 0.72 A·h for them,
    /// monotonically and without raising anything.
    pub electrolyte_conductivity_terms: Vec<PowerTerm>,
    /// The negative electrode's porous-phase parameters (`[dfn.negative]`).
    pub negative: DfnElectrode,
    /// The separator (`[dfn.separator]`) — the one domain with no solid phase.
    pub separator: DfnSeparator,
    /// The positive electrode's porous-phase parameters (`[dfn.positive]`).
    pub positive: DfnElectrode,
}

/// One term `coefficient · x^exponent` of a transport-property fit.
///
/// # Why a pair rather than a coefficient array
/// A coefficient array indexed by degree would cover the diffusivity fit, which is a
/// plain quadratic. It would **not** cover the conductivity, whose middle term is
/// `x^1.5` — Nyman's fit is a sum of power terms, not a polynomial, and a schema that
/// could not spell a fractional exponent would force the one shipped chemistry to be
/// stored as something it is not.
///
/// # A note for whoever writes an exact-bit pin
/// Phase 6's rule is that only pure IEEE-754 arithmetic and decimal→`f64` parsing may
/// be committed as an exact-bit assertion, because those are identical on every
/// conforming platform while `exp`, `asinh` and `powf` are not. **A value computed
/// through these terms is generally not pinnable**: `x^1.5` evaluated as `x·√x` would
/// be (`sqrt` is IEEE-exact), but as `powf(1.5)` it is not. Pin the parsed coefficients
/// and exponents, not anything evaluated from them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerTerm {
    /// Multiplier, carrying the whole term's unit (the variable is dimensionless).
    /// Must be finite; may be negative — both shipped fits have negative terms.
    pub coefficient: f64,
    /// Power the dimensionless concentration is raised to. Must be finite.
    pub exponent: f64,
}

/// The porous-phase parameters of one electrode (`[dfn.negative]` / `[dfn.positive]`).
///
/// The geometry, kinetics and OCP of the same electrode live in the matching
/// `[spm.*]` block; this is only what a DFN adds. In particular the electrode
/// *thickness* is [`ElectrodeParams::thickness_m`] and is not repeated here.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DfnElectrode {
    /// Electrolyte volume fraction `ε_e` (porosity), in (0, 1). Must be finite.
    ///
    /// **A different number from [`ElectrodeParams::active_volume_fraction`]** (`ε_s`),
    /// and the two do not sum to one: on the shipped set the negative electrode is
    /// `ε_s = 0.75` against `ε_e = 0.25`, and the positive `0.665` against `0.335`,
    /// with binder, conductive additive and carbon making up whatever is left. A schema
    /// that derived one from the other would be wrong for every real cell.
    pub porosity: f64,
    /// Bruggeman exponent for the **electrolyte** phase. Must be finite and `>= 0`.
    ///
    /// Effective transport is `ε_e^bruggeman` times bulk, so this is the tortuosity of
    /// the pore network expressed as a power law. 1.5 is the classical value and the
    /// one Chen2020 publishes for all three domains. Setting it to 0 makes the pores
    /// straight pipes and changes every effective transport property by a factor of
    /// 0.12–0.32 on this set — large, physical, and invisible to any test that does not
    /// run at a rate where the electrolyte matters, which is what makes it a good
    /// deliberate perturbation.
    pub bruggeman_electrolyte: f64,
    /// Bruggeman exponent for the **solid** phase, applied to
    /// [`ElectrodeParams::active_volume_fraction`]. Must be finite and `>= 0`.
    ///
    /// A separate exponent because it describes a different network. Chen2020 publishes
    /// **0** for both electrodes — i.e. the solid conductivity below is already the
    /// effective one and needs no correction — which is a real value rather than a
    /// missing one, and is why this field is not folded into the electrolyte's.
    pub bruggeman_electrode: f64,
    /// Solid-phase electronic conductivity `σ_s` \[S/m\]. Must be finite and `> 0`.
    ///
    /// Zero is rejected rather than read as "no solid conduction": a zero conductivity
    /// is an infinite resistance, so the electrode could carry no current at all. That
    /// is a divide-by-zero dressed as a parameter choice, the same argument
    /// [`ElectrodeParams::m_ref`] refuses zero on.
    ///
    /// The two shipped values are four orders of magnitude apart — 215 S/m for the
    /// graphite, **0.18** for the NMC — and the spike measured what that buys: the
    /// negative electrode's solid phase is equipotential to within **36 µV even at
    /// 3C**, while the positive's costs 2–42 mV. It would therefore be possible to ship
    /// "an SPM plus an electrolyte" and drop the solid-phase equations entirely without
    /// anyone noticing *on this parameter set*. It is refused: `σ_s` is chemistry data,
    /// a set with a worse positive conductivity would expose it, and a model named
    /// `Dfn` that silently omits one of the DFN's four equations is a quiet lie.
    pub solid_conductivity_s_per_m: f64,
}

/// The separator (`[dfn.separator]`).
///
/// Its own type rather than a [`DfnElectrode`] with two fields left blank, because the
/// separator genuinely has no solid phase to conduct through and no active material to
/// react at — the `φ_s` and `j` equations there are the trivial `φ_s = 0`, `j = 0`. A
/// shared type would have to carry a solid conductivity that means nothing, and the
/// first reader to fill it in would be describing a cell that cannot exist.
///
/// The thickness lives here rather than in `[spm]` because a single-particle model has
/// no separator: it is one of the things this section adds, not one it extends.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DfnSeparator {
    /// Separator thickness \[m\]. Must be finite and `> 0`.
    pub thickness_m: f64,
    /// Electrolyte volume fraction `ε_e`, in (0, 1). Must be finite.
    ///
    /// Higher than either electrode's on any real cell (0.47 here), which is why the
    /// separator is rarely where the electrolyte starves first.
    pub porosity: f64,
    /// Bruggeman exponent for the electrolyte phase. Must be finite and `>= 0`. See
    /// [`DfnElectrode::bruggeman_electrolyte`].
    pub bruggeman_electrolyte: f64,
}

/// Semi-empirical aging coefficients (`[aging]`).
///
/// These are the *chemistry's* numbers; whether aging runs at all, and how coarse
/// its clock is, is pack configuration ([`crate::aging::AgingConfig`]). See
/// [`crate::aging`] for what each coefficient does to the fade.
///
/// Every value in the shipped chemistries is a labelled placeholder. They are
/// order-of-magnitude plausible — the LFP set gives roughly 10 % calendar fade over
/// a year at 25 °C and 100 % SOC — but nothing here is fitted, so scenarios should
/// assert the *shape* of a fade curve, never a number on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgingParams {
    /// Arrhenius pre-exponential factor for calendar fade
    /// \[capacity fraction per √s\]. Must be finite and `>= 0`.
    pub cal_pre_exp: f64,
    /// Activation energy for calendar fade \[J/mol\]. Must be finite and `>= 0`;
    /// larger means more strongly temperature-dependent.
    pub cal_ea_j_per_mol: f64,
    /// Multiplicative SOC stress on calendar fade, over **uniformly spaced** SOC
    /// breakpoints spanning \[0, 1\] (three entries = SOC 0.0 / 0.5 / 1.0). Must be
    /// non-empty with finite, non-negative entries. See
    /// [`crate::aging::soc_stress`].
    pub cal_soc_stress: Vec<f64>,
    /// Cycle fade per amp-hour of throughput at full depth
    /// \[capacity fraction per Ah\]. Must be finite and `>= 0`.
    pub cyc_fade_per_ah: f64,
    /// Depth-of-discharge exponent for cycle fade, in the per-*cycle* convention
    /// (fade of a depth-`D` cycle `∝ D^exp`). Must be finite and `>= 1`; `1` means
    /// pure throughput counting. See [`crate::aging::cycle_increment`] for why the
    /// per-amp-hour weight is `D^(exp−1)`.
    pub cyc_dod_stress_exp: f64,
    /// Resistance growth per unit of capacity lost: `soh_resistance = 1 + this ·
    /// loss`. Must be finite and `>= 0`. Typically above 1 — resistance grows faster
    /// than capacity fades, which is most of what an aged pack feels like.
    pub r_growth_per_capacity_loss: f64,
}

/// Identity and provenance metadata (`[meta]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChemMeta {
    /// Stable identifier, e.g. `"lfp_26650_generic"`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Where the numbers came from (paper, PyBaMM set, datasheet, or placeholder).
    pub provenance: String,
}

/// Per-cell nominal capacity and operating limits (`[cell]`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellLimits {
    /// Nominal capacity \[Ah\].
    pub capacity_ah: f64,
    /// Maximum allowed terminal voltage \[V\].
    pub v_max: f64,
    /// Minimum allowed terminal voltage \[V\].
    pub v_min: f64,
    /// Maximum continuous charge rate \[C\] (multiples of `capacity_ah` per hour).
    pub max_charge_c: f64,
    /// Maximum continuous discharge rate \[C\].
    pub max_discharge_c: f64,
    /// Charge is inhibited below this cell temperature \[K\].
    pub t_charge_min_k: f64,
    /// Absolute maximum cell temperature \[K\].
    pub t_max_k: f64,
}

/// Open-circuit voltage as a function of SOC (`[ocv]`).
///
/// `soc` must be strictly ascending and span the usable range; `volts` must be
/// the same length and monotone non-decreasing (OCV rises with SOC). Lookup is
/// linear interpolation, clamped at the table ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OcvTable {
    /// SOC breakpoints, strictly ascending, in \[0, 1\].
    pub soc: Vec<f64>,
    /// OCV at each breakpoint \[V\], monotone non-decreasing, same length as `soc`.
    pub volts: Vec<f64>,
    /// Optional entropy coefficient `∂U/∂T` \[V/K\] at each `soc` breakpoint.
    ///
    /// Drives the reversible (entropic) heat term `Q_rev = −I·T·∂U/∂T` in the
    /// thermal network — typically **negative** for Li-ion over most of the SOC
    /// range, which makes discharge (positive `I`) exothermic and charge
    /// endothermic. Not sign-constrained by validation: real coefficients change
    /// sign across the SOC range, so a chemistry may legitimately supply either.
    ///
    /// `None` (the default, and the case for both shipped chemistries) disables
    /// the entropic term entirely; the thermal model then carries irreversible
    /// heat only. When present it must have the same length as `soc`. It is *not*
    /// used to temperature-correct OCV itself — `ocv_lookup` remains a pure
    /// function of SOC in this phase.
    #[serde(default)]
    pub docv_dt_v_per_k: Option<Vec<f64>>,
}

/// Ohmic series resistance `R0` over a (soc, temperature) grid (`[r0]`).
///
/// `ohms[i][j]` is the resistance at `soc[i]`, `temp_k[j]`. Both axes must be
/// strictly ascending; lookup is bilinear, clamped at the grid edges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct R0Table {
    /// SOC breakpoints, strictly ascending, in \[0, 1\].
    pub soc: Vec<f64>,
    /// Temperature breakpoints \[K\], strictly ascending.
    pub temp_k: Vec<f64>,
    /// Resistance grid \[ohms\]: outer index = soc, inner index = temperature.
    pub ohms: Vec<Vec<f64>>,
}

/// Lumped thermal properties of a single cell (`[thermal]`).
///
/// These describe the cell in isolation; how cells couple to each other and how
/// much of each cell's surface actually faces the environment inside a pack is
/// topology, and lives in [`crate::thermal::ThermalConfig`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThermalParams {
    /// Lumped heat capacity `C_th` \[J/K\] of one cell (mass × specific heat).
    /// Must be `> 0`.
    pub heat_capacity_j_per_k: f64,
    /// Convective conductance `h·A` \[W/K\] from one **fully exposed** cell to the
    /// environment — i.e. the bare-cell value, as measured on a 1S1P pack.
    ///
    /// Inside a pack this is scaled down per cell by how much of its surface is
    /// blocked by neighbours (see [`crate::thermal::exposure`]). Must be `>= 0`;
    /// `0` means a perfectly insulated cell (adiabatic), which is a legitimate
    /// configuration, not an error.
    pub h_area_w_per_k: f64,
}

/// How an equivalent-circuit cell behaves below empty (`[reversal]`).
///
/// A cell driven past `soc = 0` does not stop delivering current — under
/// [`crate::Demand::Current`] nothing in the engine *can* refuse a demanded current, as
/// `docs/plans/low-clamp-solve-side.md` measured. What a real cell does instead is go
/// into **voltage reversal**: its open-circuit voltage falls through zero and the
/// external circuit starts paying for the current it is forcing. These two numbers are
/// that fall.
///
/// # Required, not optional
/// Unlike [`AgingParams`] and [`SpmParams`], this section has no `None` meaning. A
/// chemistry that omitted it would silently inherit a curve from code, and every cell
/// built from it would fabricate energy at the bottom of its window — which is the
/// defect this section exists to close. Saying so loudly at load time is the cheaper
/// failure.
///
/// # Conservation does not depend on these values
/// `OCV_eff` is a single-valued function of the cell's extended position
/// (`soc − soc_deficit`), so stored energy is a state function of it and the energy
/// ledger closes for *any* setting here — including a [`Self::floor_v`] above the
/// chemistry's empty-endpoint OCV, which makes the branch inert. These parameters set
/// how deep the reversal goes, not whether the books balance. See
/// `docs/plans/low-clamp-reversal.md`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReversalParams {
    /// How steeply open-circuit voltage falls below empty \[V per unit SOC\].
    ///
    /// Applied to the deficit, not to `soc`: `OCV(0) − v_per_soc · soc_deficit`, so a
    /// cell one percent of its capacity past empty has lost `0.01 · v_per_soc` volts.
    /// Must be `> 0` — a zero or negative slope is a cell that keeps sourcing at its
    /// empty-endpoint OCV forever, which is the pre-fix behaviour.
    pub v_per_soc: f64,
    /// Where the fall stops \[V\].
    ///
    /// Without it the ramp is unbounded and a long over-drain reaches voltages that
    /// dominate every other term in the run — measured at −52 V on an LFP cell in two
    /// minutes, with a step-size sensitivity worse than the defect being fixed. Must be
    /// below the chemistry's OCV at `soc = 0`, or the branch never descends into it.
    ///
    /// This is a **declared limit** in the sense `docs/plans/voltage-target-blowup.md`
    /// established, not a measured plateau: a real reversed cell continues to roughly
    /// −1 to −2 V on copper dissolution before it fails outright.
    pub floor_v: f64,
    /// Capacity fraction lost per amp-hour delivered **past empty**. Must be finite and
    /// `>= 0`; `0` means reversal is survivable at no cost, which is what every version
    /// before this field did.
    ///
    /// The copper dissolution the two fields above only *mention* is what this one
    /// charges for. It is a third fade mechanism alongside calendar, cycle, and plating
    /// loss — see [`crate::aging::reversal_fade_increment`] — and like plating it is a
    /// cost of the *conditions* charge moved under, not of the amount that moved.
    ///
    /// # Required, with no `#[serde(default)]`, unlike [`SafetyParams::plating_fade_per_ah`]
    /// That field may default to zero because `[safety]` is an optional section: a
    /// chemistry without one is declaring it cannot say what plating costs. `[reversal]`
    /// is required, so a file that reaches the loader has already declared how its cell
    /// behaves below empty — a missing damage coefficient there is an omission, and the
    /// value silently supplied would be "over-discharge is free", the exact defect this
    /// field exists to remove. See `docs/plans/reversal-damage.md`.
    ///
    /// # Resistance
    /// Loss booked here grows resistance through the shared
    /// [`AgingParams::r_growth_per_capacity_loss`], at the same ratio as every other
    /// mechanism. That under-reports it: over-discharge attacks the anode current
    /// collector, so it is a *contact* failure first and an inventory failure second, and
    /// a real cell that lost 1 % this way is harder to push current through than one that
    /// lost 1 % on the shelf. Splitting the coupling needs a second coefficient and a fit
    /// to justify it; the fit is what is missing, and the measured legibility argument for
    /// adding the coefficient anyway is refused in the plan.
    pub fade_per_ah: f64,
}

/// A **depleting** diffusion overpotential (`[diffusion]`) — the voltage a sustained
/// current costs because the reactant at the reaction site runs down faster than it is
/// replenished, and which relaxes back when the current stops.
///
/// # What this adds that the RC pairs do not
/// An [`RcPair`] is already a diffusion transient, and it already relaxes at rest. What it
/// cannot do is *cost capacity*: it is a fixed resistance, so its settled drop is
/// `R·I` — proportional to current, independent of how full the cell is, and negligible at
/// low rate against the headroom between a cell's empty-endpoint OCV and its cut-off. A
/// real lead-acid cell gives up a quarter of its rated capacity between a trickle and a 1C
/// draw, and `docs/plans/lead-acid-data-only.md` measured the shipped equivalent circuit
/// reproducing **none** of that below 1C, whatever the resistances were tuned to. The term
/// here is what closes it, and the closure is measured rather than asserted: worst
/// leave-one-out error against Peukert `n = 1.1` over 0.05C → 3C goes from 25.9 points to
/// 3.8. See `docs/plans/diffusion-overpotential.md`.
///
/// # The mechanism, in two lines
/// One extra state per cell — [`crate::EcmState::depletion`], written `D` — advanced by
/// the same exact exponential update the RC pairs use, and one voltage read off it:
///
/// ```text
/// D  ←  D_ss + (D − D_ss)·e^(−dt/τ_d),     D_ss = I / capacity_ah     [C-rate, 1/h]
/// η  =  −k · ln(1 − D / (D_lim · soc))                                [V]
/// ```
///
/// `D` is a **filtered C-rate**: left at a steady current long enough it settles at
/// exactly that current expressed in C, and at rest it decays to zero with time constant
/// `τ_d`. So the three parameters read as *a rate the cell can sustain when full*
/// ([`Self::limit_c_rate`]), *how long it takes to get there* ([`Self::tau_s`]), and *what
/// approaching it costs in volts* ([`Self::scale_v`]).
///
/// # Why the `soc` in the denominator is the whole mechanism
/// Three simpler forms were tried and all three fail, in the same way: they leave the
/// delivered capacity at 0.05C, 0.1C and 0.2C at **exactly** their no-diffusion values,
/// because nothing that grows with current alone can lose capacity at a rate where the
/// current is small. The limit has to *fall as the cell empties* — which for lead-acid is
/// not a modelling convenience but the chemistry: the acid is a reactant, so a flatter
/// cell has less of it to move. Dividing by `soc` is that sentence.
///
/// # Read from the previous step, like `soc_deficit`
/// `η` is evaluated in [`crate::ecm::cell_source`] from the **start-of-step** `D`, and `D`
/// is advanced at the end of [`crate::ecm::advance_cell`]. Within a step the cell is
/// therefore still a fixed line, [`crate::CellModel::is_linear`] stays `true`, and the
/// pack's closed-form solve is untouched. The non-linearity is spread across steps rather
/// than packed inside one — the same trick the exact RC update and the reversal ramp both
/// use.
///
/// # What is fitted, what is measured, and what is neither
/// All three parameters are **fitted**, against a capacity-versus-rate sweep with the
/// absolute C/20 delivery in the objective. [`Self::scale_v`] in particular is *not* a
/// thermodynamic constant despite the Nernstian form — see its own doc. Rest recovery was
/// never in the objective and is the check the fit is judged by. The **charge direction is
/// unvalidated**: `D` goes negative on charge and the same expression returns a negative
/// `η` (a cell that has been resting sources slightly above OCV as its acid re-equalises),
/// which is the right sign and a plausible magnitude, and nothing here measured it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffusionParams {
    /// Relaxation time `τ_d` \[s\] of the depletion state. Must be finite and `> 0`.
    ///
    /// How long a cell must rest before it has recovered, and equally how long a sustained
    /// current takes to bite. For lead-acid this is **hours**, not the minutes of an
    /// [`RcPair`] or the seconds of a lithium double layer, because the process is acid
    /// diffusing through the pore structure of a thick plate.
    ///
    /// This is the parameter a rate sweep is least likely to constrain and the one that
    /// decides the answer: `docs/plans/diffusion-overpotential.md` records three searches
    /// that returned "this mechanism cannot work" because an approximation adopted for
    /// speed had quietly removed `τ_d` from the problem. Halving or doubling it from the
    /// fitted value costs 13 points of accuracy against 1.8 at the fit, so it is sharply
    /// determined once it is actually varied.
    pub tau_s: f64,
    /// The depletion `D_lim` at which the overpotential diverges, **at full charge**
    /// \[C-rate, i.e. 1/h\]. Must be finite and `> 0`.
    ///
    /// The limiting sustained C-rate a full cell could carry if nothing else stopped it;
    /// at state of charge `soc` the limit is `D_lim · soc`, so a half-empty cell can
    /// sustain half as much. A physical, checkable quantity — for the shipped AGM cell it
    /// lands near the rate at which a datasheet stops quoting continuous discharge.
    ///
    /// The divergence itself is never reached in normal operation (the cell hits its
    /// cut-off first), which is why [`Self::max_overpotential_v`] exists as a declared
    /// bound rather than as a working part of the model.
    pub limit_c_rate: f64,
    /// Voltage scale `k` \[V\] of the logarithm. Must be finite and `> 0`.
    ///
    /// # This is fitted, and it must not be labelled thermodynamic
    /// The form `η = −k·ln(1 − x)` is Nernstian, and the temptation is to read `k` as
    /// `RT/nF` = 25.7 mV. **It is not.** The fitted value for the shipped lead-acid set is
    /// three to four times that, because this one-state model has no separate charge-
    /// transfer term and `k` absorbs the electrode kinetics along with the concentration
    /// thermodynamics. Writing `RT/F` in this field would be a fabricated constant wearing
    /// a citation, which is the one thing `CLAUDE.md`'s provenance rule forbids outright.
    pub scale_v: f64,
    /// Ceiling on `|η|` \[V\] — where the collapse stops. Must be finite and `> 0`.
    ///
    /// # A fourth constant the plan did not budget, and why it is data rather than code
    /// `docs/plans/diffusion-overpotential.md` priced three parameters and measured that
    /// the divergence is never reached inside the window it swept. It is reached *outside*
    /// it, and the engine has no such window: `soc` genuinely arrives at `0.0`, where
    /// `D/(D_lim·0)` is `+∞` for a loaded cell and `0/0 = NaN` for a rested one. Something
    /// has to answer there, and a bare number in the physics would be exactly the guard
    /// `docs/plans/phase-7-dfn.md` records as documented-numerical-and-actually-load-bearing.
    /// So it is declared, per chemistry, with provenance, like [`ReversalParams::floor_v`].
    ///
    /// **Derived, not chosen.** Set it to `OCV(soc = 0) − reversal.floor_v`: the depth of
    /// the reversal ramp, so a saturated cell can cost at most as many volts as being
    /// driven the whole way past empty costs, and a cell in both states at once presents a
    /// bounded source. Validation only requires it to be positive and finite — the
    /// derivation is a sizing rule for whoever writes the file, not a constraint the
    /// engine can check without deciding what the two sections mean together.
    ///
    /// It is symmetric (`η` is clamped to `±this`) because the charge side is the same
    /// logarithm mirrored, and an unbounded charge-side term would be a hole left open for
    /// the sake of a direction nobody fitted.
    pub max_overpotential_v: f64,
}

/// One RC (Thevenin) pair modelling a diffusion/charge-transfer overpotential.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RcPair {
    /// Pair resistance \[ohms\].
    pub r_ohms: f64,
    /// Pair capacitance \[farads\]. Time constant `tau = r_ohms * c_farad`.
    pub c_farad: f64,
}

/// Ways a chemistry parameter set can be invalid.
#[derive(Debug, Error, PartialEq)]
pub enum ChemistryError {
    /// A table's two axes/columns had mismatched lengths.
    #[error("{table}: length mismatch ({a} vs {b})")]
    LengthMismatch {
        /// Which table.
        table: &'static str,
        /// First length.
        a: usize,
        /// Second length.
        b: usize,
    },
    /// A monotonicity requirement was violated at a given index.
    #[error("{what}: not monotone (strict={strict}) at index {index}")]
    NotMonotone {
        /// What was expected to be monotone.
        what: &'static str,
        /// Whether strict ascent was required (vs. non-decreasing).
        strict: bool,
        /// Index where the violation occurred.
        index: usize,
    },
    /// A value that must be positive was not.
    #[error("{what} must be > 0, got {value}")]
    NotPositive {
        /// Which quantity.
        what: &'static str,
        /// Offending value.
        value: f64,
    },
    /// A value that must be non-negative was negative (or NaN).
    #[error("{what} must be >= 0, got {value}")]
    Negative {
        /// Which quantity.
        what: &'static str,
        /// Offending value.
        value: f64,
    },
    /// A pair of limits was out of order.
    #[error("{what}")]
    BadRange {
        /// Human-readable description.
        what: &'static str,
    },
    /// Wrong number of RC pairs (must be 1 or 2).
    #[error("expected 1 or 2 RC pairs, got {0}")]
    RcCount(usize),
    /// A table was empty where at least one entry is required.
    #[error("{0} is empty")]
    Empty(&'static str),
}

impl ChemistryParams {
    /// Number of RC pairs (1 or 2 after validation).
    #[must_use]
    pub fn n_rc(&self) -> usize {
        self.rc.len()
    }

    /// Validate physical and structural invariants.
    ///
    /// Checks: monotone OCV table with matching lengths (including the optional
    /// entropy-coefficient column); strictly ascending, dimensionally consistent,
    /// positive `R0` grid; 1–2 positive RC pairs; ordered, positive cell limits;
    /// finite, positive thermal properties. Pure — no I/O.
    ///
    /// # Errors
    /// Returns the first [`ChemistryError`] encountered.
    pub fn validate(&self) -> Result<(), ChemistryError> {
        // --- OCV table ---
        if self.ocv.soc.is_empty() {
            return Err(ChemistryError::Empty("ocv.soc"));
        }
        if self.ocv.soc.len() != self.ocv.volts.len() {
            return Err(ChemistryError::LengthMismatch {
                table: "ocv",
                a: self.ocv.soc.len(),
                b: self.ocv.volts.len(),
            });
        }
        check_strictly_ascending("ocv.soc", &self.ocv.soc)?;
        check_non_decreasing("ocv.volts", &self.ocv.volts)?;
        // The entropy coefficient is optional, but when present it shares the
        // `soc` breakpoints, so its length is load-bearing for the lookup.
        // Deliberately no monotonicity or sign check: ∂U/∂T legitimately changes
        // sign across the SOC range.
        if let Some(docv_dt) = &self.ocv.docv_dt_v_per_k {
            if docv_dt.len() != self.ocv.soc.len() {
                return Err(ChemistryError::LengthMismatch {
                    table: "ocv.docv_dt_v_per_k",
                    a: docv_dt.len(),
                    b: self.ocv.soc.len(),
                });
            }
        }

        // --- R0 grid ---
        if self.r0.soc.is_empty() {
            return Err(ChemistryError::Empty("r0.soc"));
        }
        if self.r0.temp_k.is_empty() {
            return Err(ChemistryError::Empty("r0.temp_k"));
        }
        check_strictly_ascending("r0.soc", &self.r0.soc)?;
        check_strictly_ascending("r0.temp_k", &self.r0.temp_k)?;
        if self.r0.ohms.len() != self.r0.soc.len() {
            return Err(ChemistryError::LengthMismatch {
                table: "r0.ohms (rows)",
                a: self.r0.ohms.len(),
                b: self.r0.soc.len(),
            });
        }
        for row in &self.r0.ohms {
            if row.len() != self.r0.temp_k.len() {
                return Err(ChemistryError::LengthMismatch {
                    table: "r0.ohms (cols)",
                    a: row.len(),
                    b: self.r0.temp_k.len(),
                });
            }
            for &v in row {
                if !is_positive(v) {
                    return Err(ChemistryError::NotPositive {
                        what: "r0.ohms entry",
                        value: v,
                    });
                }
            }
        }

        // --- RC pairs ---
        // The upper bound is `EcmState::v_rc`'s slot count, read from there rather than
        // written twice: the overpotentials live in a fixed array, and `advance_cell` zips
        // the pairs against it, so a chemistry with more pairs than slots would have its
        // extras silently dropped rather than rejected. See `ecm::MAX_RC_PAIRS`.
        if self.rc.is_empty() || self.rc.len() > crate::ecm::MAX_RC_PAIRS {
            return Err(ChemistryError::RcCount(self.rc.len()));
        }
        for pair in &self.rc {
            if !is_positive(pair.r_ohms) {
                return Err(ChemistryError::NotPositive {
                    what: "rc.r_ohms",
                    value: pair.r_ohms,
                });
            }
            if !is_positive(pair.c_farad) {
                return Err(ChemistryError::NotPositive {
                    what: "rc.c_farad",
                    value: pair.c_farad,
                });
            }
        }

        // --- Cell limits ---
        let c = &self.cell;
        if !is_positive(c.capacity_ah) {
            return Err(ChemistryError::NotPositive {
                what: "cell.capacity_ah",
                value: c.capacity_ah,
            });
        }
        let voltages_ordered = c.v_min < c.v_max;
        if !voltages_ordered {
            return Err(ChemistryError::BadRange {
                what: "cell.v_min must be < cell.v_max",
            });
        }
        if !is_positive(c.max_charge_c) {
            return Err(ChemistryError::NotPositive {
                what: "cell.max_charge_c",
                value: c.max_charge_c,
            });
        }
        if !is_positive(c.max_discharge_c) {
            return Err(ChemistryError::NotPositive {
                what: "cell.max_discharge_c",
                value: c.max_discharge_c,
            });
        }
        let temps_ordered = c.t_charge_min_k < c.t_max_k;
        if !temps_ordered {
            return Err(ChemistryError::BadRange {
                what: "cell.t_charge_min_k must be < cell.t_max_k",
            });
        }

        // --- Thermal ---
        // Finiteness is checked explicitly (TOML admits `inf`/`nan` floats)
        // because these two numbers set the thermal sub-step stability bound: an
        // infinite conductance or heat capacity would make that bound degenerate.
        let t = &self.thermal;
        if !t.heat_capacity_j_per_k.is_finite() || !t.h_area_w_per_k.is_finite() {
            return Err(ChemistryError::BadRange {
                what: "thermal.heat_capacity_j_per_k and thermal.h_area_w_per_k must be finite",
            });
        }
        if !is_positive(t.heat_capacity_j_per_k) {
            return Err(ChemistryError::NotPositive {
                what: "thermal.heat_capacity_j_per_k",
                value: t.heat_capacity_j_per_k,
            });
        }
        if !is_non_negative(t.h_area_w_per_k) {
            return Err(ChemistryError::Negative {
                what: "thermal.h_area_w_per_k",
                value: t.h_area_w_per_k,
            });
        }

        // --- Reversal ---
        //
        // Checked against `ocv.volts[0]` rather than in isolation: the floor's whole job
        // is to stop a ramp that starts at the empty-endpoint OCV, and a floor at or
        // above that endpoint is a ramp the cell can never descend into — i.e. silently
        // the pre-fix behaviour, which is the one outcome this section exists to make
        // impossible. The OCV table is validated non-empty above, so the index is safe.
        let rev = &self.reversal;
        if !rev.v_per_soc.is_finite() || !rev.floor_v.is_finite() {
            return Err(ChemistryError::BadRange {
                what: "reversal.v_per_soc and reversal.floor_v must be finite",
            });
        }
        // Folded finiteness, for the reason the `[aging]` block gives: this number
        // multiplies into a state of health the whole solve then divides by, so a NaN
        // here is not a bad reading, it is a dead pack.
        if !rev.fade_per_ah.is_finite() || rev.fade_per_ah < 0.0 {
            return Err(ChemistryError::BadRange {
                what: "reversal.fade_per_ah must be finite and >= 0",
            });
        }
        if !is_positive(rev.v_per_soc) {
            return Err(ChemistryError::NotPositive {
                what: "reversal.v_per_soc",
                value: rev.v_per_soc,
            });
        }
        if rev.floor_v >= self.ocv.volts[0] {
            return Err(ChemistryError::BadRange {
                what: "reversal.floor_v must be below the OCV at soc = 0",
            });
        }

        // --- Diffusion (optional) ---
        //
        // All four are checked positive-and-finite together, and none is checked against
        // another section. That is deliberate and is the difference from `[reversal]`
        // above, whose floor is checked against the OCV table: there the cross-check
        // catches a configuration that makes the *branch inert*, which is a silent
        // failure. Here nothing goes silent — a badly sized `max_overpotential_v` binds
        // visibly, in volts, in the telemetry — so the sizing rule stays in the field's
        // doc where a reader can apply judgement, rather than being frozen into a
        // validator that would have to decide what `[reversal]` and `[diffusion]` mean
        // together.
        if let Some(d) = &self.diffusion {
            let positive: [(&'static str, f64); 4] = [
                ("diffusion.tau_s", d.tau_s),
                ("diffusion.limit_c_rate", d.limit_c_rate),
                ("diffusion.scale_v", d.scale_v),
                ("diffusion.max_overpotential_v", d.max_overpotential_v),
            ];
            for (what, value) in positive {
                if !is_positive(value) || !value.is_finite() {
                    return Err(ChemistryError::NotPositive { what, value });
                }
            }
        }

        // --- Aging (optional) ---
        if let Some(a) = &self.aging {
            // Finiteness is folded into each check: an infinite pre-exponential or
            // activation energy would make the fade rate NaN/inf, and these numbers
            // multiply into a state of health the whole solve then divides by.
            let non_negative: [(&'static str, f64); 4] = [
                ("aging.cal_pre_exp", a.cal_pre_exp),
                ("aging.cal_ea_j_per_mol", a.cal_ea_j_per_mol),
                ("aging.cyc_fade_per_ah", a.cyc_fade_per_ah),
                (
                    "aging.r_growth_per_capacity_loss",
                    a.r_growth_per_capacity_loss,
                ),
            ];
            for (what, value) in non_negative {
                if !is_non_negative(value) || !value.is_finite() {
                    return Err(ChemistryError::Negative { what, value });
                }
            }
            if a.cal_soc_stress.is_empty() {
                return Err(ChemistryError::Empty("aging.cal_soc_stress"));
            }
            for &value in &a.cal_soc_stress {
                if !is_non_negative(value) || !value.is_finite() {
                    return Err(ChemistryError::Negative {
                        what: "aging.cal_soc_stress entry",
                        value,
                    });
                }
            }
            // Below 1 the per-amp-hour weight `D^(exp−1)` has a negative exponent and
            // diverges as the depth goes to zero — a micro-cycle would age the cell
            // more than a full one. That is not a parameter choice, it is a sign
            // error, so it is rejected rather than clamped.
            let dod_exp_ok = a.cyc_dod_stress_exp >= 1.0 && a.cyc_dod_stress_exp.is_finite();
            if !dod_exp_ok {
                return Err(ChemistryError::BadRange {
                    what: "aging.cyc_dod_stress_exp must be finite and >= 1",
                });
            }
        }

        // --- Safety (optional) ---
        // The runaway trio is validated here even though nothing reads it yet: the
        // checks belong with the data, and a later slice should find them already in
        // place rather than discovering a chemistry that parsed but cannot integrate.
        if let Some(s) = &self.safety {
            let positive: [(&'static str, f64); 2] = [
                ("safety.t_onset_k", s.t_onset_k),
                ("safety.t_vent_k", s.t_vent_k),
            ];
            for (what, value) in positive {
                if !is_positive(value) || !value.is_finite() {
                    return Err(ChemistryError::NotPositive { what, value });
                }
            }
            if !is_positive(s.t_plating_min_k) || !s.t_plating_min_k.is_finite() {
                return Err(ChemistryError::NotPositive {
                    what: "safety.t_plating_min_k",
                    value: s.t_plating_min_k,
                });
            }
            let non_negative: [(&'static str, f64); 5] = [
                ("safety.runaway_energy_j", s.runaway_energy_j),
                (
                    "safety.runaway_power_w_at_onset",
                    s.runaway_power_w_at_onset,
                ),
                ("safety.plating_c_threshold", s.plating_c_threshold),
                ("safety.plating_fade_per_ah", s.plating_fade_per_ah),
                (
                    "safety.plating_short_hazard_per_ah",
                    s.plating_short_hazard_per_ah,
                ),
            ];
            for (what, value) in non_negative {
                if !is_non_negative(value) || !value.is_finite() {
                    return Err(ChemistryError::Negative { what, value });
                }
            }
            let vent_above_onset = s.t_vent_k > s.t_onset_k;
            if !vent_above_onset {
                return Err(ChemistryError::BadRange {
                    what: "safety.t_onset_k must be < safety.t_vent_k",
                });
            }
            // Conditional on purpose: a chemistry file written before plating shorts
            // existed omits this field entirely and defaults it to zero. That is a
            // valid parameter set — it just cannot short — so requiring a positive
            // resistance unconditionally would reject files that are perfectly usable.
            // Once the hazard is positive the resistance is load-bearing, and a zero
            // one would mean an infinite-conductance short: a dead cell, instantly.
            if s.plating_short_hazard_per_ah > 0.0
                && (!is_positive(s.plating_short_ohms) || !s.plating_short_ohms.is_finite())
            {
                return Err(ChemistryError::NotPositive {
                    what:
                        "safety.plating_short_ohms (required when plating_short_hazard_per_ah > 0)",
                    value: s.plating_short_ohms,
                });
            }
            // Same conditional shape, same reason: a file written before the runaway
            // slice omits both fields and defaults them to zero, which is a valid
            // parameter set that simply never self-heats. Once the amplitude is
            // positive the exponent is load-bearing — see the field docs for why zero
            // is not an acceptable value for it then.
            if s.runaway_power_w_at_onset > 0.0
                && (!is_positive(s.runaway_ea_j_per_mol) || !s.runaway_ea_j_per_mol.is_finite())
            {
                return Err(ChemistryError::NotPositive {
                    what:
                        "safety.runaway_ea_j_per_mol (required when runaway_power_w_at_onset > 0)",
                    value: s.runaway_ea_j_per_mol,
                });
            }
        }

        // --- Single-particle model (optional) ---
        if let Some(spm) = &self.spm {
            check_spm(spm)?;
        }

        // --- Electrolyte / DFN (optional) ---
        // Deliberately not cross-checked against `[spm]`, though a `[dfn]` block alone
        // parameterizes nothing: see the field's doc comment. The pairing is diagnosed
        // where a config asks for the model, which is the same place `[spm]`'s own
        // absence is diagnosed, and splitting that rule across two layers would give
        // one mistake two different error messages.
        if let Some(dfn) = &self.dfn {
            check_dfn(dfn)?;
        }
        Ok(())
    }
}

/// Validate a `[dfn]` block. Split out for the same reason [`check_spm`] is: it checks
/// two structurally identical electrodes plus a third domain that is *nearly* identical,
/// and would otherwise be written three times.
fn check_dfn(dfn: &DfnParams) -> Result<(), ChemistryError> {
    // A transference number outside [0, 1] is not a fraction of a current. Neither
    // endpoint is excluded — see the field doc for why this differs from
    // `charge_transfer_alpha`, whose endpoints are genuinely degenerate.
    let t_plus_ok = (0.0..=1.0).contains(&dfn.transference_number);
    if !t_plus_ok {
        return Err(ChemistryError::BadRange {
            what: "dfn.transference_number must be in [0, 1]",
        });
    }
    if !is_positive(dfn.thermodynamic_factor) || !dfn.thermodynamic_factor.is_finite() {
        return Err(ChemistryError::NotPositive {
            what: "dfn.thermodynamic_factor",
            value: dfn.thermodynamic_factor,
        });
    }
    check_transport(
        "dfn.electrolyte_diffusivity_terms",
        &dfn.electrolyte_diffusivity_terms,
    )?;
    check_transport(
        "dfn.electrolyte_conductivity_terms",
        &dfn.electrolyte_conductivity_terms,
    )?;
    check_dfn_electrode("dfn.negative", &dfn.negative)?;
    check_dfn_electrode("dfn.positive", &dfn.positive)?;

    let sep = &dfn.separator;
    if !is_positive(sep.thickness_m) || !sep.thickness_m.is_finite() {
        return Err(ChemistryError::NotPositive {
            what: "dfn.separator.thickness_m",
            value: sep.thickness_m,
        });
    }
    check_porosity("dfn.separator.porosity", sep.porosity)?;
    check_bruggeman(
        "dfn.separator.bruggeman_electrolyte",
        sep.bruggeman_electrolyte,
    )?;
    Ok(())
}

/// Validate one transport-property fit: a non-empty sum of finite power terms whose
/// value at the reference concentration is positive.
///
/// # The one point this can check exactly, and what it deliberately does not claim
/// A transport property must be positive everywhere the solver evaluates it, and
/// nothing here knows that range — both shipped fits are **non-monotone** over the
/// concentrations a 3C discharge visits (`D_e` falls 10.8× from 200 to 2200 mol/m³ and
/// then rises; `κ_e` peaks near 1000 and falls 2.3× by 2600), so no cheap sampling
/// would be a proof either.
///
/// What is checked is the fit at `x = 1`, i.e. `c_e = 1000 mol/m³` — the concentration
/// the fit's own variable is written in, and the initial concentration of every
/// parameter set that uses this form. There the sum is `Σ coefficient`: **plain
/// arithmetic, no `powf`**, so it is exact, platform-independent, and the one value
/// derived from this section that could be pinned bit-for-bit. A fit that is negative
/// where the cell *starts* is broken beyond argument; a fit that goes negative
/// somewhere in the middle is a physical question this layer cannot answer and does not
/// pretend to.
fn check_transport(what: &'static str, terms: &[PowerTerm]) -> Result<(), ChemistryError> {
    if terms.is_empty() {
        return Err(ChemistryError::Empty(what));
    }
    let mut at_reference = 0.0;
    for term in terms {
        if !term.coefficient.is_finite() || !term.exponent.is_finite() {
            return Err(ChemistryError::BadRange {
                what: if what.contains("diffusivity") {
                    "dfn.electrolyte_diffusivity_terms entries must be finite"
                } else {
                    "dfn.electrolyte_conductivity_terms entries must be finite"
                },
            });
        }
        at_reference += term.coefficient;
    }
    if !is_positive(at_reference) {
        return Err(ChemistryError::NotPositive {
            what: if what.contains("diffusivity") {
                "dfn.electrolyte_diffusivity_terms summed at c_e = 1000 mol/m3"
            } else {
                "dfn.electrolyte_conductivity_terms summed at c_e = 1000 mol/m3"
            },
            value: at_reference,
        });
    }
    Ok(())
}

/// Validate one `[dfn.negative]` / `[dfn.positive]` block. `side` prefixes every error,
/// for the reason [`check_electrode`] gives: the two blocks are identical in shape, so
/// an unprefixed error sends the reader to the wrong one.
fn check_dfn_electrode(side: &'static str, e: &DfnElectrode) -> Result<(), ChemistryError> {
    let negative = side == "dfn.negative";
    check_porosity(
        if negative {
            "dfn.negative.porosity"
        } else {
            "dfn.positive.porosity"
        },
        e.porosity,
    )?;
    check_bruggeman(
        if negative {
            "dfn.negative.bruggeman_electrolyte"
        } else {
            "dfn.positive.bruggeman_electrolyte"
        },
        e.bruggeman_electrolyte,
    )?;
    check_bruggeman(
        if negative {
            "dfn.negative.bruggeman_electrode"
        } else {
            "dfn.positive.bruggeman_electrode"
        },
        e.bruggeman_electrode,
    )?;
    if !is_positive(e.solid_conductivity_s_per_m) || !e.solid_conductivity_s_per_m.is_finite() {
        return Err(ChemistryError::NotPositive {
            what: if negative {
                "dfn.negative.solid_conductivity_s_per_m"
            } else {
                "dfn.positive.solid_conductivity_s_per_m"
            },
            value: e.solid_conductivity_s_per_m,
        });
    }
    Ok(())
}

/// A porosity is an open-interval volume fraction: 0 is a domain with no electrolyte in
/// it (no ionic path at all) and 1 is a domain that is pure electrolyte (no separator,
/// or an electrode with no electrode in it). Both ends are excluded on purpose, unlike
/// [`ElectrodeParams::active_volume_fraction`], which admits 1.
fn check_porosity(what: &'static str, value: f64) -> Result<(), ChemistryError> {
    let ok = value > 0.0 && value < 1.0;
    if !ok {
        return Err(ChemistryError::BadRange { what });
    }
    Ok(())
}

/// A Bruggeman exponent multiplies a volume fraction in (0, 1), so a negative one would
/// make a *less* porous domain conduct *better*. Zero is allowed and is Chen2020's own
/// value for both electrode phases: it means the published conductivity is already the
/// effective one.
fn check_bruggeman(what: &'static str, value: f64) -> Result<(), ChemistryError> {
    if !is_non_negative(value) || !value.is_finite() {
        return Err(ChemistryError::Negative { what, value });
    }
    Ok(())
}

/// Validate a `[spm]` block. Split out of [`ChemistryParams::validate`] because it
/// checks two structurally identical electrodes and would otherwise be written twice.
fn check_spm(spm: &SpmParams) -> Result<(), ChemistryError> {
    let positive: [(&'static str, f64); 3] = [
        ("spm.t_ref_k", spm.t_ref_k),
        ("spm.c_e_mol_per_m3", spm.c_e_mol_per_m3),
        ("spm.electrode_area_m2", spm.electrode_area_m2),
    ];
    for (what, value) in positive {
        if !is_positive(value) || !value.is_finite() {
            return Err(ChemistryError::NotPositive { what, value });
        }
    }
    if !is_non_negative(spm.contact_resistance_ohm) || !spm.contact_resistance_ohm.is_finite() {
        return Err(ChemistryError::Negative {
            what: "spm.contact_resistance_ohm",
            value: spm.contact_resistance_ohm,
        });
    }
    check_electrode("spm.negative", &spm.negative)?;
    check_electrode("spm.positive", &spm.positive)?;
    Ok(())
}

/// Validate one `[spm.*]` electrode. `side` prefixes every error so a failure names
/// which electrode it came from — the two blocks are identical in shape, so an error
/// that said only "particle_radius_m" would send the reader to the wrong one.
fn check_electrode(side: &'static str, e: &ElectrodeParams) -> Result<(), ChemistryError> {
    // The `&'static str` error fields cannot carry a formatted prefix, so each
    // electrode gets its own table of names. Two arrays of literals is duller than
    // building the strings, and it keeps `ChemistryError` allocation-free.
    let names: [&'static str; 4] = if side == "spm.negative" {
        [
            "spm.negative.particle_radius_m",
            "spm.negative.diffusivity_m2_per_s",
            "spm.negative.c_max_mol_per_m3",
            "spm.negative.thickness_m",
        ]
    } else {
        [
            "spm.positive.particle_radius_m",
            "spm.positive.diffusivity_m2_per_s",
            "spm.positive.c_max_mol_per_m3",
            "spm.positive.thickness_m",
        ]
    };
    let values = [
        e.particle_radius_m,
        e.diffusivity_m2_per_s,
        e.c_max_mol_per_m3,
        e.thickness_m,
    ];
    for (what, value) in names.into_iter().zip(values) {
        if !is_positive(value) || !value.is_finite() {
            return Err(ChemistryError::NotPositive { what, value });
        }
    }
    // `m_ref` is the amplitude of the exchange-current density. Zero is rejected
    // rather than treated as "inert" (the convention the optional `[safety]` fields
    // use) because it is not an opt-out: a cell with no reaction rate has infinite
    // kinetic overpotential at any current at all, which is a divide-by-zero dressed
    // as a parameter choice.
    let m_ref_name = if side == "spm.negative" {
        "spm.negative.m_ref"
    } else {
        "spm.positive.m_ref"
    };
    if !is_positive(e.m_ref) || !e.m_ref.is_finite() {
        return Err(ChemistryError::NotPositive {
            what: m_ref_name,
            value: e.m_ref,
        });
    }
    let ea_ok = |x: f64| is_non_negative(x) && x.is_finite();
    if !ea_ok(e.reaction_ea_j_per_mol) || !ea_ok(e.diffusivity_ea_j_per_mol) {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative activation energies must be finite and >= 0"
            } else {
                "spm.positive activation energies must be finite and >= 0"
            },
        });
    }
    let frac_ok = e.active_volume_fraction > 0.0 && e.active_volume_fraction <= 1.0;
    if !frac_ok {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative.active_volume_fraction must be in (0, 1]"
            } else {
                "spm.positive.active_volume_fraction must be in (0, 1]"
            },
        });
    }
    // Open interval at both ends: `alpha` and `1 − alpha` are both exponents in the
    // Butler-Volmer equation, so either endpoint kills one of the two branches.
    let alpha_ok = e.charge_transfer_alpha > 0.0 && e.charge_transfer_alpha < 1.0;
    if !alpha_ok {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative.charge_transfer_alpha must be in (0, 1)"
            } else {
                "spm.positive.charge_transfer_alpha must be in (0, 1)"
            },
        });
    }
    if !e.docp_dt_v_per_k.is_finite() {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative.docp_dt_v_per_k must be finite"
            } else {
                "spm.positive.docp_dt_v_per_k must be finite"
            },
        });
    }
    let limits_ok = (0.0..=1.0).contains(&e.stoich_min)
        && (0.0..=1.0).contains(&e.stoich_max)
        && e.stoich_min < e.stoich_max;
    if !limits_ok {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative stoichiometry limits must satisfy 0 <= stoich_min < stoich_max <= 1"
            } else {
                "spm.positive stoichiometry limits must satisfy 0 <= stoich_min < stoich_max <= 1"
            },
        });
    }

    // --- OCP table ---
    let ocp_axis = if side == "spm.negative" {
        "spm.negative.ocp.stoich"
    } else {
        "spm.positive.ocp.stoich"
    };
    let ocp_volts = if side == "spm.negative" {
        "spm.negative.ocp.volts"
    } else {
        "spm.positive.ocp.volts"
    };
    if e.ocp.stoich.is_empty() {
        return Err(ChemistryError::Empty(ocp_axis));
    }
    if e.ocp.stoich.len() != e.ocp.volts.len() {
        return Err(ChemistryError::LengthMismatch {
            table: if side == "spm.negative" {
                "spm.negative.ocp"
            } else {
                "spm.positive.ocp"
            },
            a: e.ocp.stoich.len(),
            b: e.ocp.volts.len(),
        });
    }
    check_strictly_ascending(ocp_axis, &e.ocp.stoich)?;
    // Non-*increasing*, the opposite of `[ocv]`. See [`OcpTable`] for why.
    check_non_increasing(ocp_volts, &e.ocp.volts)?;
    // The table must cover the window the cell actually operates over, or lookups at
    // the ends clamp to a flat potential and the model quietly loses its end-of-charge
    // and end-of-discharge behaviour — the two places it is most interesting. A
    // surface stoichiometry can run past the *bulk* limits under load, so a real
    // extraction leaves margin either side; this only insists the limits themselves
    // are inside.
    let covers =
        e.ocp.stoich[0] <= e.stoich_min && e.ocp.stoich[e.ocp.stoich.len() - 1] >= e.stoich_max;
    if !covers {
        return Err(ChemistryError::BadRange {
            what: if side == "spm.negative" {
                "spm.negative.ocp.stoich must span [stoich_min, stoich_max]"
            } else {
                "spm.positive.ocp.stoich must span [stoich_min, stoich_max]"
            },
        });
    }
    Ok(())
}

fn check_strictly_ascending(what: &'static str, xs: &[f64]) -> Result<(), ChemistryError> {
    for i in 1..xs.len() {
        let ascends = xs[i] > xs[i - 1];
        if !ascends {
            return Err(ChemistryError::NotMonotone {
                what,
                strict: true,
                index: i,
            });
        }
    }
    Ok(())
}

/// The mirror of [`check_non_decreasing`], for half-cell OCP tables.
///
/// Not a generalization with a direction flag: two five-line functions read better
/// than one with a boolean argument at every call site, and the name is what tells a
/// reader that `[spm.*.ocp]` genuinely runs the other way from `[ocv]` rather than
/// having been validated by accident. See [`OcpTable`].
fn check_non_increasing(what: &'static str, xs: &[f64]) -> Result<(), ChemistryError> {
    for i in 1..xs.len() {
        let increases = xs[i] > xs[i - 1];
        if increases {
            return Err(ChemistryError::NotMonotone {
                what,
                strict: false,
                index: i,
            });
        }
    }
    Ok(())
}

fn check_non_decreasing(what: &'static str, xs: &[f64]) -> Result<(), ChemistryError> {
    for i in 1..xs.len() {
        let decreases = xs[i] < xs[i - 1];
        if decreases {
            return Err(ChemistryError::NotMonotone {
                what,
                strict: false,
                index: i,
            });
        }
    }
    Ok(())
}
