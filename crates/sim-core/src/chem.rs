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
    /// Stoichiometry at the cell's lower voltage cut-off, in \[0, 1\]. Must be
    /// finite.
    ///
    /// Note "min"/"max" are ordered by *stoichiometry*, not by state of charge: the
    /// negative electrode fills as the cell charges while the positive empties, so
    /// [`Self::stoich_max`] is a full cell at the negative electrode and an empty one
    /// at the positive. Together the two pairs are what map a lithium inventory onto
    /// [`crate::Telemetry::soc_true`].
    pub stoich_min: f64,
    /// Stoichiometry at the cell's upper voltage cut-off, in \[0, 1\]. Must be finite
    /// and `> stoich_min`.
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
        if self.rc.is_empty() || self.rc.len() > 2 {
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
        Ok(())
    }
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
