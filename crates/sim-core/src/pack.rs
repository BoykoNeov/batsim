//! The battery pack — the one public simulation entry point.
//!
//! Per the design contract, the pack is the first-class citizen and a single cell
//! is the degenerate `1S1P` pack; there is no separate cell API. Phase 1
//! implements general series/parallel topology behind this same API, with the
//! closed-form group solve where imbalance physics emerge.
//!
//! # Electrical solve (closed form, no iteration)
//! Over one step every cell is a fixed linear Thévenin source `E_k = OCV − Σ V_rc`
//! behind `R_k = R0·r0_factor` (evaluated from start-of-step state). A **parallel
//! group** carrying group current `I_g` has one shared node voltage
//! `V = (Σ E_k/R_k − I_g)/(Σ 1/R_k)`, and each cell then carries
//! `I_k = (E_k − V)/R_k` — so a low-resistance or high-SOC cell naturally takes
//! more load, and mismatched cells at rest circulate current. Each parallel group
//! aggregates to its own Thévenin `(E_g, R_g)`; **series** groups share one current
//! and their node voltages sum, so the whole pack aggregates to one Thévenin
//! `(E_pack, R_pack)` against which the demand is solved in closed form (see
//! [`solve_current`]).

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::aging::{Aging, AgingConfig, CellAging};
use crate::bms::{Bms, BmsConfig};
use crate::chem::ChemistryParams;
use crate::ecm::{solve_current, CellModel};
use crate::faults::{Fault, FaultError, FaultState, SensorFaultKind, SensorId};
use crate::flags::EventFlags;
use crate::noise::standard_normal_pair;
use crate::plating::plating_risk;
use crate::runaway::{is_vented, reaction_power, CellRunaway};
use crate::thermal::{advance_temperatures, ThermalConfig, ThermalStep};
use crate::{Demand, Env, Telemetry};

/// Current snapshot schema version. Bumped whenever [`Pack`]'s serialized layout
/// changes (see `CLAUDE.md`).
///
/// v2 (Phase 1): the single `cell: CellModel` became `groups: Vec<ParallelGroup>`
/// of per-cell `Cell`s, and config gained `scatter`. No migration ships because
/// Phase 0 had no `snapshot()` method, so no v1 snapshots can exist. Note that
/// under a self-describing-by-order format like bincode a *structural* change is
/// caught at deserialization before [`Pack::restore`]'s version check runs; the
/// version field's job is to guard future *semantic* changes to an unchanged
/// layout.
///
/// v3 (Phase 2, thermal): `Pack` gained `thermal: ThermalConfig` and
/// `ChemistryParams` gained `thermal: ThermalParams` plus the optional
/// `ocv.docv_dt_v_per_k` column, all of which sit inside the snapshot. Cell
/// temperature was already part of `EcmState`, so no *per-cell* layout changed.
///
/// Note what actually rejects a v2 snapshot here: the layout change, at
/// deserialization, exactly as the v2 note above describes. The version check never
/// sees those bytes. The bump is still correct — adapters gate on `Snapshot::version`,
/// and a tolerant format (or a future migration path) would otherwise accept the blob
/// and silently produce an isothermal pack — but this bump is not what makes v2
/// unloadable, and no test could pin it as such.
///
/// v4 (Phase 2, BMS): `Pack` gained `bms: Option<Bms>`, which carries the SOC
/// estimate, the rest timer, and the last [`crate::bms::SensorFrame`]. The frame is
/// serialized rather than recomputed on restore, and has to be: the loaded group
/// voltages in it depend on a current that is not stored, and its noise draw has
/// already advanced the RNG. Same caveat as v3 about what actually rejects an older
/// blob.
///
/// v5 (Phase 2, protection + balancing): `BmsConfig` gained `protection` and
/// `balancing`, and `Bms` gained the latched `contactor_open`. This bump covers
/// **two** slices: the protection slice changed the layout and failed to bump, and
/// this note is the correction rather than a silent renumbering. No v4 snapshot has
/// ever existed outside a single test process, so nothing needs migrating — but the
/// rule is one bump per layout-changing slice, and it was missed once.
///
/// v6 (Phase 3, aging): `Pack` gained `aging: Option<Aging>` (config plus the
/// sub-clock accumulator), every `Cell` gained a `CellAging` block,
/// and `ChemistryParams` gained the optional `aging` coefficients. This bump is also
/// **semantic**, which is the case the version field exists for: `Telemetry::soc_true`
/// now divides by capacity that folds in `soh_capacity`, so on an aged pack the same
/// stored state reports a different SOC than a v5 build would have. Same caveat as v3
/// about what actually rejects an older blob.
///
/// v7 (Phase 3, faults): `Pack` gained `faults: FaultState` (the timestamped queue
/// plus the pack-level effects — external short conductance and sensor corruptions),
/// and every `Cell` gained `shunt_g`, the internal-short leakage conductance. Both
/// are genuine state: the queue is what has *not* happened yet, and a fired fault's
/// effect has no other home. Same caveat as v3 about what actually rejects an older
/// blob.
///
/// v8 (Phase 3, plating): `ChemistryParams` gained the optional `safety` coefficients
/// (`[safety]`, previously parsed and discarded), and every cell's `CellAging` gained
/// `q_plating` and `ah_plating_since_tick`. Both cell fields are genuine state: the
/// first is fade already suffered, the second is throughput not yet charged for, and
/// dropping either across a snapshot would let a client wash out damage by saving and
/// reloading. Same caveat as v3 about what actually rejects an older blob.
///
/// v9 (Phase 3, runaway): `SafetyParams` gained `runaway_power_w_at_onset` and
/// `runaway_ea_j_per_mol`, and every `Cell` gained `runaway: CellRunaway` — the
/// unreleased half of its exothermic budget plus its vent latch. Both cell fields are
/// genuine state and neither is recoverable from anything else: a cell that has
/// already burned half its budget cannot burn it again, and a vented cell stays
/// vented. Same caveat as v3 about what actually rejects an older blob.
pub const SNAPSHOT_VERSION: u32 = 9;

/// Per-cell manufacturing scatter: independent Gaussian variation of capacity and
/// ohmic resistance across the cells of a pack.
///
/// Sigmas are **relative** (a fraction of the nominal value: `0.02` = 2 % 1σ).
/// `0` means no scatter. Draws come from the single pack RNG (seeded by
/// [`PackConfig::seed`]) at construction — honouring the "one RNG" rule, this type
/// carries no seed of its own (a deliberate refinement of the `CLAUDE.md` API
/// sketch, which nested the seed here).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scatter {
    /// Relative 1σ of per-cell capacity, as a fraction of nominal. `0` = uniform.
    pub capacity_sigma: f64,
    /// Relative 1σ of per-cell `R0`, as a fraction of nominal. `0` = uniform.
    pub r0_sigma: f64,
}

impl Default for Scatter {
    fn default() -> Self {
        Self {
            capacity_sigma: 0.0,
            r0_sigma: 0.0,
        }
    }
}

/// Pack topology and initial conditions.
///
/// This doubles as (part of) the scenario file format.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackConfig {
    /// Number of series elements (groups). Must be ≥ 1.
    pub series: u16,
    /// Number of parallel cells per group. Must be ≥ 1.
    pub parallel: u16,
    /// Initial state of charge for every cell, in \[0, 1\].
    pub initial_soc: f64,
    /// Initial temperature for every cell \[K\].
    pub initial_temp_k: f64,
    /// Seed for the single simulation RNG (part of the snapshot).
    pub seed: u64,
    /// Per-cell manufacturing scatter (defaults to none).
    #[serde(default)]
    pub scatter: Scatter,
    /// Thermal coupling.
    ///
    /// **Omitting this from a scenario file yields an isothermal pack** — not a
    /// thermally-coupled one with default parameters. Silence means the thermal
    /// model is *off*. That default is deliberate (it keeps the goldens testing the
    /// electrical model, see [`ThermalConfig`]), but it does mean a scenario author
    /// who forgets the section gets no diagnostic.
    #[serde(default)]
    pub thermal: ThermalConfig,
    /// Battery management system, or `None` for no BMS at all.
    ///
    /// `None` is a supported and interesting mode, not a broken pack: demands pass
    /// through unclamped, [`Telemetry::soc_bms`] is `None`, and the failure paths a
    /// real BMS exists to prevent become reachable. Contrasting the two is a core
    /// teaching scenario.
    #[serde(default)]
    pub bms: Option<BmsConfig>,
    /// Aging, or `None` for a pack that never wears out.
    ///
    /// `None` is the default and a supported mode — most electrical and thermal
    /// scenarios want health held fixed. When this is `Some`, the chemistry **must**
    /// supply an `[aging]` section or [`Pack::new`] fails: a pack configured to age
    /// against a chemistry that cannot say how would otherwise be silently ageless.
    #[serde(default)]
    pub aging: Option<AgingConfig>,
}

/// Reasons [`Pack::new`] can fail.
#[derive(Debug, Error, PartialEq)]
pub enum BuildError {
    /// `series` or `parallel` was zero; a pack needs at least one cell.
    #[error("topology {series}S{parallel}P is invalid: series and parallel must both be ≥ 1")]
    ZeroTopology {
        /// Requested series count.
        series: u16,
        /// Requested parallel count.
        parallel: u16,
    },
    /// `initial_soc` was outside \[0, 1\].
    #[error("initial_soc must be in [0, 1], got {0}")]
    BadInitialSoc(f64),
    /// `initial_temp_k` was not positive.
    #[error("initial_temp_k must be > 0, got {0}")]
    BadInitialTemp(f64),
    /// A [`ThermalConfig::Network`] conductance was negative or not finite.
    #[error("thermal.k_neighbor_w_per_k must be finite and >= 0, got {0}")]
    BadThermalConductance(f64),
    /// A [`BmsConfig`] field was outside its allowed range.
    #[error("bms.{field} is invalid: {reason} (got {value})")]
    BadBmsConfig {
        /// Offending field name.
        field: &'static str,
        /// What was required.
        reason: &'static str,
        /// Offending value.
        value: f64,
    },
    /// A [`BmsConfig::temp_probes`] entry named a cell outside the topology.
    #[error("bms.temp_probes[{index}] = {s}S{p}P is outside a {series}S{parallel}P pack")]
    BadTempProbe {
        /// Index into `temp_probes`.
        index: usize,
        /// Requested series index.
        s: u16,
        /// Requested parallel index.
        p: u16,
        /// Pack series count.
        series: u16,
        /// Pack parallel count.
        parallel: u16,
    },
    /// [`PackConfig::aging`] was set but the chemistry has no `[aging]` section.
    #[error("aging is configured but chemistry '{chem_id}' has no [aging] coefficients")]
    MissingAgingParams {
        /// Identifier of the chemistry that came up short.
        chem_id: String,
    },
    /// [`AgingConfig::sub_clock_period_s`] was negative or not finite.
    #[error("aging.sub_clock_period_s must be finite and >= 0, got {0}")]
    BadAgingPeriod(f64),
    /// The chemistry itself failed validation.
    #[error("invalid chemistry: {0}")]
    Chemistry(#[from] crate::chem::ChemistryError),
}

/// Reasons [`Pack::restore`] can reject a [`Snapshot`].
#[derive(Debug, Error, PartialEq)]
pub enum RestoreError {
    /// The snapshot's schema version is not the one this build understands.
    #[error("snapshot schema version {found} is unsupported (this build expects {expected})")]
    VersionMismatch {
        /// Version recorded in the snapshot.
        found: u32,
        /// Version this build produces/consumes.
        expected: u32,
    },
}

/// A serializable capture of the entire engine state.
///
/// Per the design contract the whole engine is one serde value with a schema
/// `version`; this newtype is that value plus a top-level version tag so an adapter
/// can gate on it. Round-tripping a `Snapshot` through any serde format and calling
/// [`Pack::restore`] reproduces the original trajectory exactly (see the replay
/// test). The inner state is private; construct one via [`Pack::snapshot`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Schema version of the captured state (mirrors the pack's own `version`).
    pub version: u32,
    /// The full ground-truth pack state.
    pack: Pack,
}

/// A `(series, parallel)` cell index was out of range for the pack's topology.
#[derive(Debug, Error, PartialEq)]
#[error("cell index {s}S{p}P is out of range for a {series}S{parallel}P pack")]
pub struct CellIndexError {
    /// Requested series index.
    pub s: usize,
    /// Requested parallel index.
    pub p: usize,
    /// Pack series count.
    pub series: u16,
    /// Pack parallel count.
    pub parallel: u16,
}

/// One physical cell's ground-truth state plus its static manufacturing factors.
///
/// The dynamic ECM state lives in [`CellModel`]/[`EcmState`]; the two factors are
/// fixed at construction (from [`Scatter`] or an explicit weak-cell override) and
/// scale the cell's effective capacity and resistance. Aging's `soh_*` multipliers
/// are their dynamic siblings and compose on top of them — see [`CellAging`], which
/// lives here rather than in [`EcmState`] precisely so that the [`CellModel`] enum
/// stays swappable for a porous-electrode model later.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Cell {
    /// Dynamic equivalent-circuit state.
    model: CellModel,
    /// Static capacity multiplier: effective capacity = nominal × this. `> 0`.
    capacity_factor: f64,
    /// Static `R0` multiplier: effective `R0` = nominal × this. `> 0`.
    r0_factor: f64,
    /// Health: the two `soh_*` multipliers and the accumulators behind them. Exactly
    /// `1.0`/`1.0` and inert unless the pack has aging configured.
    aging: CellAging,
    /// Internal-short leakage conductance \[S\] across this cell's own terminals;
    /// `0.0` on a healthy cell. Set by [`crate::faults::Fault::SoftInternalShort`].
    ///
    /// Stored as a conductance, not a resistance: it is added to the group's
    /// conductance sum (exactly where the balancing bleed goes, but per cell), so the
    /// solve never divides by it. See [`crate::faults`] for why the shunt is *not*
    /// folded into the cell's Thévenin source.
    #[serde(default)]
    shunt_g: f64,
    /// Exothermic budget left and vent latch. Deliberately its own field rather than
    /// part of `aging`: a pack with `aging: None` never wears out but must still be
    /// able to burn (see [`CellRunaway`]).
    #[serde(default)]
    runaway: CellRunaway,
}

impl Cell {
    /// The resistance multiplier the electrical solve actually uses: static
    /// manufacturing factor × aging's resistance growth.
    ///
    /// Every call to [`cell_source`] goes through this, including the one inside the
    /// [`SourceCache`] staleness assert — the memo's invariant is stated in terms of
    /// *this* product, not of `r0_factor` alone.
    fn eff_r0_factor(&self) -> f64 {
        self.r0_factor * self.aging.soh_resistance
    }
}

/// A parallel group: the cells wired in parallel that share one terminal node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ParallelGroup {
    /// The `parallel` cells in this group.
    cells: Vec<Cell>,
}

/// A read-only view of one cell's ground-truth state, returned by [`Pack::cell`].
///
/// This is the engine's *true* per-cell state — distinct from anything the BMS can
/// sense (the BMS sees group-level sensors only, from Phase 2 on).
///
/// Serde for the same reason as [`Telemetry`] — adapters put it on a socket — and
/// with the same consequence: these field names are a wire contract, versioned by
/// the adapter's API version rather than by [`SNAPSHOT_VERSION`]. Note this is a
/// *view*, not stored state: deserializing one produces a detached value, never a
/// handle onto a pack.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellView {
    /// Ground-truth state of charge, in \[0, 1\].
    pub soc: f64,
    /// Cell temperature \[K\].
    pub temp_k: f64,
    /// Total overpotential \[V\] across the cell's internal dynamics,
    /// discharge-positive: everything between `OCV(soc)` and the terminal that is
    /// **not** the instantaneous ohmic drop.
    ///
    /// Filled from [`crate::CellModel::overpotential_v`], and model-neutral in both
    /// name and value — for an equivalent circuit it is `Σ V_rc`, for a
    /// porous-electrode cell it is the sum of the kinetic and concentration
    /// overpotentials. Every cell model has one.
    ///
    /// Was `v_rc_sum` through v9. That name named an ECM internal an SPM cell does
    /// not have, and the only value such a cell could have reported under it is
    /// `0.0` — indistinguishable, to a plotting client, from a real measurement of
    /// an unpolarized cell. The rename waited for this slice because the field is a
    /// **wire contract** (see above) and renaming it is exactly what the adapters'
    /// API versions are bumped for: `sim_server::API_VERSION` and
    /// `sim_wasm::WASM_API_VERSION` both went 1 → 2 with it.
    pub overpotential_v: f64,
    /// Static capacity multiplier applied to this cell.
    pub capacity_factor: f64,
    /// Static `R0` multiplier applied to this cell.
    pub r0_factor: f64,
    /// Capacity state of health in (0, 1\]: effective capacity = nominal ×
    /// [`Self::capacity_factor`] × this. Exactly `1.0` without aging.
    pub soh_capacity: f64,
    /// Resistance growth factor `>= 1`: effective `R0` = nominal ×
    /// [`Self::r0_factor`] × this. Exactly `1.0` without aging.
    pub soh_resistance: f64,
    /// Internal-short leakage conductance \[S\] across this cell's terminals; `0.0`
    /// on a healthy cell.
    ///
    /// A conductance rather than a resistance because that is what is stored and what
    /// the solve adds (parallel shorts compose by addition); the equivalent
    /// resistance is its reciprocal.
    pub internal_short_conductance_s: f64,
    /// Exothermic energy \[J\] this cell has not released yet.
    ///
    /// Starts at the chemistry's `runaway_energy_j` (`0.0` without a `[safety]`
    /// section), falls only while the cell is above onset, and reaching `0.0` means the
    /// cell has burned out — it can no longer self-heat, whatever its temperature.
    pub runaway_energy_remaining_j: f64,
    /// Whether this cell has ever reached the chemistry's vent temperature.
    ///
    /// Irreversible, and deliberately *only* a report: v1 does not model what venting
    /// does to a cell (mass loss, ejected electrolyte, the end of it being a battery).
    /// See [`crate::runaway`].
    pub vented: bool,
}

/// Per-cell start-of-step Thévenin `(E, R)`, carried across the step boundary.
///
/// A **memo of a pure function of pack state**, not state itself: entry `i` is
/// exactly `cell_i.model.source(chem, cell_i.eff_r0_factor())` in
/// series-major / parallel-minor order. `step`'s end-of-step reporting pass computes those values
/// for every cell, and the next step's start-of-step aggregation would recompute
/// them from unchanged state — so the reporting pass fills this and the next step
/// reads it, halving the per-cell table lookups (perf item 3 in
/// `docs/plans/pack-step-perf.md`).
///
/// Empty means cold: recompute and refill. That is always *correct*, only slower,
/// which is what makes the invalidation rule safe to get wrong in the conservative
/// direction. Anything that mutates cell state or the effective `R0` factor outside
/// `step` must clear it (today: [`Pack::set_cell_factors`]).
///
/// Aging mutates `soh_resistance`, which is one of that product's two halves — and
/// needs no invalidation anyway, because the aging update is sequenced *before* the
/// end-of-step reporting pass that fills this. The pass therefore memoises
/// post-aging sources. Aging after the pass would poison the next step silently, and
/// only a debug build would notice.
///
/// An internal short is deliberately **not** part of this — neither an injected one nor
/// the one a plating cell can grow. Both enter the solve as a conductance on the group
/// node rather than as a transform of the cell's source (see [`crate::faults`]), so the
/// invariant above is literally unchanged by them and a short needs no invalidation
/// either. That is what lets the plating roll sit inside the aging tick, which runs
/// after the electrical solve and before the pass that fills this.
///
/// Two deliberate impls:
///
/// * **`PartialEq` is always `true`.** Two packs whose state is equal *are* equal,
///   whether or not one happens to have a warm memo — and a serde round-trip
///   deliberately produces a cold one (see below), so anything else would make
///   `snapshot != roundtrip(snapshot)`. The cost is that
///   `zero_length_step_does_not_mutate_state` can no longer see cache corruption;
///   the `debug_assert` in `step`'s warm path is the compensating guard, and it
///   turns *every* debug-mode test into a staleness check.
/// * **`Debug` prints the length only.** A thousand `(f64, f64)` pairs in every
///   `{:?}` of a pack is noise, and they are recomputable from what is printed
///   alongside.
///
/// The field is `#[serde(skip)]`: no bytes are emitted, so **no `SNAPSHOT_VERSION`
/// bump** — v5 blobs stay exactly as they were — and a restored pack starts cold
/// and recomputes, which reproduces the trajectory bit-for-bit because a cold
/// compute is by definition what the memo holds.
/// `Default` is the one derive that matters: `#[serde(skip)]` uses it to produce the
/// (cold, therefore correct) memo a deserialized pack starts with. The type needs no
/// `Serialize`/`Deserialize` of its own precisely because it is never written out.
#[derive(Clone, Default)]
struct SourceCache(Vec<(f64, f64)>);

impl PartialEq for SourceCache {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for SourceCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SourceCache({} cells)", self.0.len())
    }
}

/// A battery pack: the full, ground-truth simulation state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    /// Snapshot schema version.
    version: u32,
    /// Cell chemistry (config; identical for every cell).
    chem: ChemistryParams,
    /// Series element count.
    series: u16,
    /// Parallel cell count per group.
    parallel: u16,
    /// Ground-truth topology: `series` groups, each of `parallel` cells.
    groups: Vec<ParallelGroup>,
    /// Thermal coupling (config; see [`ThermalConfig`]).
    thermal: ThermalConfig,
    /// The battery management system, if this pack has one. Holds the BMS's own
    /// beliefs (SOC estimate, rest timer, last sensor frame) — never ground truth.
    bms: Option<Bms>,
    /// Aging policy plus its sub-clock accumulator, or `None` for a pack that never
    /// wears out. The per-cell health it drives lives on each [`Cell`].
    aging: Option<Aging>,
    /// Injected faults: the queue of what has not fired yet, plus the pack-level
    /// effects of what has. Per-cell effects live on the cells (see
    /// [`crate::faults`]). Empty and free on a healthy pack.
    #[serde(default)]
    faults: FaultState,
    /// The single seeded RNG; its state is part of the snapshot.
    rng: ChaCha8Rng,
    /// Simulation time elapsed \[s\].
    sim_time_s: f64,
    /// Memoised per-cell Thévenin sources; see [`SourceCache`]. Derived, not state.
    #[serde(skip)]
    src_cache: SourceCache,
}

impl Pack {
    /// Build a pack from config and a (validated-on-the-way) chemistry.
    ///
    /// # Errors
    /// Returns [`BuildError`] for a zero-sized topology, out-of-range initial
    /// conditions, or an invalid chemistry.
    pub fn new(config: &PackConfig, chem: ChemistryParams) -> Result<Self, BuildError> {
        chem.validate()?;
        if config.series == 0 || config.parallel == 0 {
            return Err(BuildError::ZeroTopology {
                series: config.series,
                parallel: config.parallel,
            });
        }
        if !(0.0..=1.0).contains(&config.initial_soc) {
            return Err(BuildError::BadInitialSoc(config.initial_soc));
        }
        let temp_positive = config.initial_temp_k > 0.0;
        if !temp_positive {
            return Err(BuildError::BadInitialTemp(config.initial_temp_k));
        }
        if let ThermalConfig::Network { k_neighbor_w_per_k } = config.thermal {
            let k_ok = k_neighbor_w_per_k >= 0.0 && k_neighbor_w_per_k.is_finite();
            if !k_ok {
                return Err(BuildError::BadThermalConductance(k_neighbor_w_per_k));
            }
        }
        if let Some(bms) = &config.bms {
            validate_bms(bms, config.series, config.parallel)?;
        }
        if let Some(aging) = &config.aging {
            let period_ok = aging.sub_clock_period_s.is_finite() && aging.sub_clock_period_s >= 0.0;
            if !period_ok {
                return Err(BuildError::BadAgingPeriod(aging.sub_clock_period_s));
            }
            // Refusing here is the whole point of the chemistry's `aging` being an
            // `Option`: a pack asked to age against a parameter set with no
            // coefficients would otherwise run forever at exactly zero fade, which
            // looks identical to a working aging model on a very stable chemistry.
            if chem.aging.is_none() {
                return Err(BuildError::MissingAgingParams {
                    chem_id: chem.meta.id.clone(),
                });
            }
        }

        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
        let n_rc = chem.n_rc();
        let mut groups = Vec::with_capacity(config.series as usize);
        // Series-major, parallel-minor construction so the scatter draw order (and
        // thus the whole trajectory) is a fixed function of the seed and topology.
        for _ in 0..config.series {
            let mut cells = Vec::with_capacity(config.parallel as usize);
            for _ in 0..config.parallel {
                let (capacity_factor, r0_factor) = draw_factors(&mut rng, &config.scatter);
                cells.push(Cell {
                    model: CellModel::new_ecm(n_rc, config.initial_soc, config.initial_temp_k),
                    capacity_factor,
                    r0_factor,
                    aging: CellAging::new(config.initial_soc),
                    shunt_g: 0.0,
                    runaway: CellRunaway::new(chem.safety.as_ref()),
                });
            }
            groups.push(ParallelGroup { cells });
        }

        // Built after the scatter draws so that adding a BMS cannot shift the
        // per-cell factors a seed produces; `Bms::new` itself draws nothing.
        let bms = config.bms.clone().map(|bms_config| {
            Bms::new(
                bms_config,
                &chem,
                config.series,
                config.initial_soc,
                config.initial_temp_k,
            )
        });

        Ok(Self {
            version: SNAPSHOT_VERSION,
            chem,
            series: config.series,
            parallel: config.parallel,
            groups,
            thermal: config.thermal,
            bms,
            aging: config.aging.map(Aging::new),
            faults: FaultState::default(),
            rng,
            sim_time_s: 0.0,
            // Cold: the first step computes every cell's Thévenin source and fills it.
            src_cache: SourceCache::default(),
        })
    }

    /// Simulation time elapsed so far \[s\].
    #[must_use]
    pub fn sim_time_s(&self) -> f64 {
        self.sim_time_s
    }

    /// Number of series elements (groups).
    ///
    /// Together with [`Self::parallel`] this is the topology a caller needs in order
    /// to iterate [`Self::cell`] — an adapter serving a per-cell view has to know how
    /// far to count, and reading it back off the [`PackConfig`] it was built from
    /// stops being correct the moment a [`Snapshot`] from elsewhere is restored into
    /// the same slot. Read-only and derived from state that already exists, so no
    /// layout changed and no [`SNAPSHOT_VERSION`] bump is owed.
    ///
    /// Both numbers were already public, incidentally: [`CellIndexError`] reports
    /// them. They were merely unreachable without provoking an error first.
    #[must_use]
    pub fn series(&self) -> u16 {
        self.series
    }

    /// Number of parallel cells per group. See [`Self::series`].
    #[must_use]
    pub fn parallel(&self) -> u16 {
        self.parallel
    }

    /// Capture the entire engine state as a versioned, serializable [`Snapshot`].
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: self.version,
            pack: self.clone(),
        }
    }

    /// Rebuild a pack from a [`Snapshot`]. Continuing from the result reproduces the
    /// original trajectory exactly.
    ///
    /// # Errors
    /// Returns [`RestoreError::VersionMismatch`] if the snapshot's schema version is
    /// not the one this build understands.
    pub fn restore(snapshot: &Snapshot) -> Result<Self, RestoreError> {
        if snapshot.version != SNAPSHOT_VERSION {
            return Err(RestoreError::VersionMismatch {
                found: snapshot.version,
                expected: SNAPSHOT_VERSION,
            });
        }
        Ok(snapshot.pack.clone())
    }

    /// Ground-truth view of the cell at series position `s`, parallel position `p`
    /// (both zero-based). Returns `None` if either index is out of range.
    #[must_use]
    pub fn cell(&self, s: usize, p: usize) -> Option<CellView> {
        let cell = self.groups.get(s)?.cells.get(p)?;
        Some(CellView {
            soc: cell.model.soc(),
            temp_k: cell.model.temp_k(),
            overpotential_v: cell.model.overpotential_v(),
            capacity_factor: cell.capacity_factor,
            r0_factor: cell.r0_factor,
            soh_capacity: cell.aging.soh_capacity,
            soh_resistance: cell.aging.soh_resistance,
            internal_short_conductance_s: cell.shunt_g,
            runaway_energy_remaining_j: cell.runaway.energy_remaining_j,
            vented: cell.runaway.vented,
        })
    }

    /// The pack's aging clock, or `None` if this pack does not age.
    ///
    /// Exposes the sub-clock's pending interval, which is what makes a mid-period
    /// snapshot legible: aging state is not only the per-cell health in
    /// [`CellView`], it is also how far along the pack is towards its next update.
    #[must_use]
    pub fn aging(&self) -> Option<&Aging> {
        self.aging.as_ref()
    }

    /// The pack's injected-fault state: what is queued, and what is in effect.
    ///
    /// Per-cell effects are not here — a soft internal short shows up as
    /// [`CellView::internal_short_conductance_s`], and a weak cell as that cell's
    /// static factors.
    #[must_use]
    pub fn faults(&self) -> &FaultState {
        &self.faults
    }

    /// Queue a [`Fault`] to fire at simulation time `at_s`.
    ///
    /// It fires on the first step whose interval `[t, t + dt)` contains `at_s`, at the
    /// **start** of that step, before the electrical solve — so the firing step is
    /// also the first step to feel it. A timestamp already in the past fires on the
    /// next stepping step rather than being dropped, and no fault fires on a
    /// zero-length probe step. See [`crate::faults`] for the reasoning behind all
    /// three rules.
    ///
    /// Faults are applied in timestamp order, ties in scheduling order, so a
    /// trajectory is a function of the scheduling sequence alone.
    ///
    /// # Errors
    /// Returns [`FaultError`] for a non-finite parameter, a non-positive short
    /// resistance, a cell position outside the topology, or a sensor this pack does
    /// not have (including any sensor at all, on a pack with no BMS).
    pub fn schedule_fault(&mut self, at_s: f64, fault: Fault) -> Result<(), FaultError> {
        if !at_s.is_finite() {
            return Err(FaultError::NotFinite {
                field: "at_s",
                value: at_s,
            });
        }
        self.validate_fault(&fault)?;
        self.faults.schedule(at_s, fault);
        Ok(())
    }

    /// Repair the pack: drop every queued fault and every pack-level effect. Returns
    /// how many went away.
    ///
    /// This clears the queue, the external short, the sensor corruptions, and every
    /// cell's internal-short conductance — the mirror of [`Pack::clear_bms_fault`] for
    /// injected faults rather than latched protection.
    ///
    /// **A fired [`Fault::WeakCell`] is not undone by this.** It does not persist as a
    /// fault: it is folded into that cell's static manufacturing factors the moment it
    /// fires, and becomes indistinguishable from an unlucky scatter draw. Restoring
    /// such a cell means calling [`Pack::set_cell_factors`] with the values you want.
    pub fn clear_faults(&mut self) -> usize {
        let mut n = self.faults.clear();
        for group in &mut self.groups {
            for cell in &mut group.cells {
                if cell.shunt_g != 0.0 {
                    cell.shunt_g = 0.0;
                    n += 1;
                }
            }
        }
        // The shunt is not an input to the memoised Thévenin source (see
        // [`crate::faults`]), so unlike `set_cell_factors` this needs no invalidation.
        n
    }

    /// Check a fault's parameters against this pack before it joins the queue.
    fn validate_fault(&self, fault: &Fault) -> Result<(), FaultError> {
        let finite = |field, value: f64| {
            if value.is_finite() {
                Ok(())
            } else {
                Err(FaultError::NotFinite { field, value })
            }
        };
        let cell_in_range = |s: u16, p: u16| {
            if s < self.series && p < self.parallel {
                Ok(())
            } else {
                Err(FaultError::BadCellIndex {
                    s,
                    p,
                    series: self.series,
                    parallel: self.parallel,
                })
            }
        };
        let resistance = |ohms: f64| {
            if ohms.is_finite() && ohms > 0.0 {
                Ok(())
            } else {
                Err(FaultError::BadResistance(ohms))
            }
        };
        // A sensor fault names a sensor that has to exist: group voltages are indexed
        // by series position, probes by their index in the BMS's probe list, and a
        // pack with no BMS has no sensors at all to corrupt.
        let sensor_exists = |sensor: SensorId| {
            let ok = match sensor {
                SensorId::GroupVoltage(g) => self.bms.is_some() && g < self.series,
                SensorId::TempProbe(i) => self
                    .bms
                    .as_ref()
                    .is_some_and(|b| (i as usize) < b.config().temp_probes.len()),
                SensorId::PackCurrent => self.bms.is_some(),
            };
            if ok {
                Ok(())
            } else {
                Err(FaultError::NoSuchSensor { sensor })
            }
        };

        match *fault {
            Fault::WeakCell {
                s,
                p,
                capacity_factor,
                r0_factor,
            } => {
                cell_in_range(s, p)?;
                finite("capacity_factor", capacity_factor)?;
                finite("r0_factor", r0_factor)
            }
            Fault::SoftInternalShort { s, p, ohms } => {
                cell_in_range(s, p)?;
                resistance(ohms)
            }
            Fault::ExternalShort { ohms } => resistance(ohms),
            Fault::SensorStuck { sensor, value } => {
                sensor_exists(sensor)?;
                finite("value", value)
            }
            Fault::SensorOffset { sensor, offset } => {
                sensor_exists(sensor)?;
                finite("offset", offset)
            }
        }
    }

    /// Fire every queued fault due within this step, at start of step.
    ///
    /// Called only when `dt > 0` (see [`crate::faults`]). Ordering matters and is
    /// fixed: the queue is sorted by timestamp with ties in scheduling order, and
    /// each fault is applied in full before the next is read.
    fn fire_due_faults(&mut self, dt: f64) {
        let due = self.faults.take_due(self.sim_time_s + dt);
        for scheduled in due {
            match scheduled.fault {
                Fault::WeakCell {
                    s,
                    p,
                    capacity_factor,
                    r0_factor,
                } => {
                    // Validated in range when scheduled, and topology cannot change.
                    // `set_cell_factors` clears the Thévenin memo, which is exactly
                    // why a `WeakCell` has to land here at start of step: applying it
                    // after the reporting pass would throw away the memo that pass had
                    // just filled, costing a cold step every time.
                    let _ =
                        self.set_cell_factors(s as usize, p as usize, capacity_factor, r0_factor);
                }
                Fault::SoftInternalShort { s, p, ohms } => {
                    if let Some(cell) = self
                        .groups
                        .get_mut(s as usize)
                        .and_then(|g| g.cells.get_mut(p as usize))
                    {
                        // Conductances add: a second short on the same cell is a
                        // second leakage path in parallel with the first.
                        cell.shunt_g += 1.0 / ohms;
                    }
                }
                Fault::ExternalShort { ohms } => self.faults.add_external_short(ohms),
                Fault::SensorStuck { sensor, value } => self
                    .faults
                    .add_sensor_fault(sensor, SensorFaultKind::Stuck(value)),
                Fault::SensorOffset { sensor, offset } => self
                    .faults
                    .add_sensor_fault(sensor, SensorFaultKind::Offset(offset)),
            }
        }
    }

    /// Override one cell's static manufacturing factors (capacity and `R0`
    /// multipliers).
    ///
    /// This is the deterministic "weak cell" / scatter-outlier seam, and it is the
    /// application point [`Fault::WeakCell`] uses when it fires. Factors are clamped
    /// to a positive floor to preserve the group solve's invariants. `s`/`p` are
    /// zero-based series/parallel indices.
    ///
    /// # Errors
    /// Returns [`CellIndexError`] if `(s, p)` is out of range for the topology.
    pub fn set_cell_factors(
        &mut self,
        s: usize,
        p: usize,
        capacity_factor: f64,
        r0_factor: f64,
    ) -> Result<(), CellIndexError> {
        let (series, parallel) = (self.series, self.parallel);
        let cell = self
            .groups
            .get_mut(s)
            .and_then(|g| g.cells.get_mut(p))
            .ok_or(CellIndexError {
                s,
                p,
                series,
                parallel,
            })?;
        cell.capacity_factor = capacity_factor.max(MIN_FACTOR);
        cell.r0_factor = r0_factor.max(MIN_FACTOR);
        // `r0_factor` is an input to the memoised Thévenin source, so the whole memo
        // is now suspect. Dropping it costs one cold step; keeping a stale entry
        // would be a silent physics error. This is *the* invalidation point outside
        // `step` — anything a later phase adds that mutates a cell from outside must
        // do the same (see [`SourceCache`]).
        self.src_cache.0.clear();
        Ok(())
    }

    /// Advance the simulation by `dt` seconds under `demand`. Never panics.
    ///
    /// Ordering within a step: any injected fault due this step fires first (see
    /// [`crate::faults`]), the electrical solve then runs off **start-of-step**
    /// state, each cell's internal state is advanced with the current it was
    /// assigned, temperatures are integrated against the heat that current
    /// generated (see [`crate::thermal`]), aging's sub-clock is advanced, and all
    /// telemetry is reported from **end-of-step** state. `env` supplies the thermal
    /// sink — coolant if present, otherwise ambient — and is unused when the pack is
    /// [`ThermalConfig::Isothermal`].
    ///
    /// The aging update's position is load-bearing twice over: it must come after the
    /// temperatures and SOCs it reads have settled, and before the reporting pass that
    /// memoises each cell's Thévenin source. It is also the only place in a step that
    /// can consume randomness besides the sensor sample — a cell that has been plating
    /// rolls there for a soft internal short (see [`crate::plating`]).
    ///
    /// # Upper limit on `dt` with a live thermal network
    /// Temperatures are integrated with automatic sub-stepping, so any ordinary `dt`
    /// is safe. There is a bound, though: the sub-step count is capped, and above
    /// roughly **1.7 hours** of `dt` for the shipped chemistries the cap binds and
    /// the temperatures stop being trustworthy (they are not flagged — a
    /// `debug_assert` fires in debug builds). Nothing else in the step has such a
    /// limit; the electrical solve and the RC update are exact at any `dt`. Coarse
    /// fast-forward will need a different thermal integrator, not a bigger cap.
    pub fn step(&mut self, dt: f64, demand: Demand, env: &Env) -> Telemetry {
        let cap_ah = self.chem.cell.capacity_ah;
        let (series, parallel) = (self.series as usize, self.parallel as usize);

        // A zero-length step is an observation, not a tick: it must leave the engine
        // exactly as it found it (pinned by `snapshot.rs`, and relied on by the
        // energy-balance property test to read the start-of-step terminal voltage).
        // Every physics update scales by `dt` and so is self-guarding, but the BMS is
        // not — consuming a frame resets its rest timer and can fire an OCV
        // correction, and sampling a new one advances the RNG. So the sensor clock
        // only ticks when time actually passes.
        let sensor_tick = dt > 0.0;

        // --- injected faults fire first, before anything reads pack state, so the
        // step that fires one is also the first step to feel it. Same `dt > 0` gate
        // and the same reason as the sensor clock above: firing a fault is a reaction
        // to information, not to elapsed time, so it must not happen on a probe step.
        if sensor_tick {
            self.fire_due_faults(dt);
        }

        // --- the BMS acts first, on the frame sampled at the end of the previous
        // step. It is one step behind on purpose (see `crate::bms`), and it never
        // sees any of the ground truth computed below.
        if sensor_tick {
            if let Some(bms) = &mut self.bms {
                let pack_nominal_ah = cap_ah * f64::from(self.parallel);
                bms.update_estimate(&self.chem, dt, pack_nominal_ah);
            }
        }

        // --- passive balancing: a closed bleed switch is just a conductance across
        // the group node. Reading it mutates nothing, so this runs on probe steps too,
        // keeping the reported voltage consistent with the step that would follow.
        let mut bleed_g: Vec<f64> = Vec::new();
        if let Some(bms) = &self.bms {
            bms.bleed_conductances(&mut bleed_g);
        }
        let bleed_at = |g: usize| bleed_g.get(g).copied().unwrap_or(0.0);

        // --- start-of-step: per-cell and per-group Thévenin, then pack aggregate.
        // group_src[g] = (E_g, R_g); cell_src[g·parallel + k] = (E_k, R_k).
        //
        // `cell_src` is one flat buffer in the same series-major / parallel-minor
        // order the rest of the step uses, and it is *owned by the pack* — taken out
        // here and put back at the end of the step. Two reasons: it costs no
        // allocation at all after the first step, and (see [`SourceCache`]) when it
        // arrives warm it already holds this step's values, because the previous
        // step's reporting pass computed exactly `cell_source` over exactly this
        // state. Taking it by value also sidesteps borrowing `self` twice in the
        // loops below.
        let n_cells = series * parallel;
        let mut cell_src = std::mem::take(&mut self.src_cache).0;
        let warm = cell_src.len() == n_cells;
        if !warm {
            cell_src.clear();
            cell_src.reserve(n_cells);
        }
        let mut group_src: Vec<(f64, f64)> = Vec::with_capacity(self.groups.len());
        for (g_idx, group) in self.groups.iter().enumerate() {
            // Σ 1/R_k, plus the bleed conductance if this group is balancing, plus any
            // cell's internal-short leakage. Note that both enter the *denominator
            // only*: they draw current out of the node without contributing any EMF.
            // A healthy cell's `shunt_g` is exactly `0.0`, and `g > 0` always, so
            // `g + 0.0 == g` bit-for-bit — an unshorted pack solves exactly the
            // arithmetic it solved before faults existed.
            let mut sum_g = bleed_at(g_idx);
            let mut sum_eg = 0.0; // Σ E_k/R_k
            for (k, cell) in group.cells.iter().enumerate() {
                let (e, r) = if warm {
                    let cached = cell_src[g_idx * parallel + k];
                    // The memo must be bit-for-bit what a recompute would give. In
                    // debug builds it is checked, every cell, every step — which makes
                    // every test in the suite a staleness check, and is the guard that
                    // pays for `SourceCache`'s always-equal `PartialEq`.
                    debug_assert_eq!(
                        (cached.0.to_bits(), cached.1.to_bits()),
                        {
                            let fresh = cell.model.source(&self.chem, cell.eff_r0_factor());
                            (fresh.0.to_bits(), fresh.1.to_bits())
                        },
                        "stale Thévenin memo at cell {g_idx}S{k}P"
                    );
                    cached
                } else {
                    let fresh = cell.model.source(&self.chem, cell.eff_r0_factor());
                    cell_src.push(fresh);
                    fresh
                };
                let g = 1.0 / r;
                sum_g += g + cell.shunt_g;
                sum_eg += e * g;
            }
            group_src.push((sum_eg / sum_g, 1.0 / sum_g));
        }
        // Series aggregate: same current through every group; voltages add.
        let e_pack: f64 = group_src.iter().map(|&(e, _)| e).sum();
        let r_pack: f64 = group_src.iter().map(|&(_, r)| r).sum();

        // --- an external short sits across the load-side terminals, downstream of the
        // contactor (see [`crate::faults`]). It is a shunt on the pack's aggregate
        // Thévenin, so the *load* solves against the transformed source and the short
        // takes whatever the resulting terminal voltage gives it. Every demand variant
        // stays closed form, and `E' − i_load·R'` is identically the terminal voltage
        // `E_pack − i_g·R_pack` that the total current produces — the two views of the
        // same node agree by construction, which is what keeps the energy balance
        // exact.
        let g_ext = self.faults.external_short_conductance_s();
        let (e_load, r_load) = if g_ext > 0.0 {
            let denom = 1.0 + r_pack * g_ext;
            (e_pack / denom, r_pack / denom)
        } else {
            (e_pack, r_pack)
        };

        // --- solve the single load current (shared by every series group), then let
        // protection derate or interrupt it. Clamping the *solved current* rather
        // than the demand itself means every demand variant — including `Power` and
        // `Voltage` — is protected by the same code, and the solve stays closed form.
        let i_req = solve_current(demand, e_load, r_load);
        let mut flags = EventFlags::empty();
        let i_load = match (&mut self.bms, sensor_tick) {
            (Some(bms), true) => {
                let pack_ah = cap_ah * f64::from(self.parallel);
                let (i_allowed, protection_flags) =
                    bms.apply_protection(&self.chem, i_req, pack_ah);
                flags |= protection_flags;
                i_allowed
            }
            // A latched contactor keeps the pack open even on a probe step, where the
            // BMS does not otherwise run: an open contactor is a state, not an event.
            (Some(bms), false) if bms.contactor_open() => 0.0,
            _ => i_req,
        };

        // --- total pack current: what the load was allowed, plus what the short takes.
        // Protection can derate the load but cannot derate a short, which is precisely
        // the lesson — the only thing that stops an external short is opening the
        // contactor, and that disconnects load and short together.
        let mut i_external_short_a = 0.0;
        let mut i_g = i_load;
        if self.bms.as_ref().is_some_and(Bms::contactor_open) {
            i_g = 0.0;
        } else if g_ext > 0.0 {
            i_external_short_a = (e_load - i_load * r_load) * g_ext;
            i_g += i_external_short_a;
        }

        // --- split into per-cell currents, tally heat, and advance each cell.
        // Heat is tallied in every mode (an isothermal pack still reports how much
        // heat it makes); the per-cell array is only materialised when a live
        // thermal network is going to consume it.
        let thermal_live = matches!(self.thermal, ThermalConfig::Network { .. });
        // Cycle-fade bookkeeping reacts to charge *moving*, so like every other
        // physics update it is `dt`-scaled — but the reversal detection that anchors
        // the depth-of-discharge measurement is not, so the whole accumulation takes
        // the explicit zero-length-step gate.
        let aging_accumulates = self.aging.is_some() && dt > 0.0;
        // Plating detection needs no configuration — it is emergent, so the chemistry
        // supplying `[safety]` thresholds is the whole of the opt-in (see
        // [`crate::plating`]). Hoisted out of the loop so a chemistry without the
        // section costs one `Option` check per step rather than one per cell.
        let safety = self.chem.safety.as_ref();
        let mut heat_w: Vec<f64> = if thermal_live {
            Vec::with_capacity(n_cells)
        } else {
            Vec::new()
        };
        let mut q_gen_w = 0.0;
        let mut q_balancing_w = 0.0;
        let mut i_balancing_a = 0.0;
        let mut i_internal_short_a = 0.0;
        for (g, group) in self.groups.iter_mut().enumerate() {
            let (e_gv, r_gv) = group_src[g];
            let v_node = e_gv - i_g * r_gv; // start-of-step shared node voltage
                                            // Bleed current and dissipation are evaluated from this same
                                            // start-of-step node voltage — the one that actually drove the bleed over
                                            // the step, and the one that produced `i_k` below. Using the end-of-step
                                            // voltage instead would leave the reported numbers O(dt) adrift from the
                                            // energy that was really dissipated, and would stop the pack energy
                                            // balance closing exactly (see the energy-balance property test).
            let g_bleed = bleed_at(g);
            if g_bleed > 0.0 {
                i_balancing_a += v_node * g_bleed;
                q_balancing_w += v_node * v_node * g_bleed;
                flags |= EventFlags::BALANCING;
            }
            for (k, cell) in group.cells.iter_mut().enumerate() {
                let (e_k, r_k) = cell_src[g * parallel + k];
                // The cell's *internal* branch current, unchanged in form by any
                // shunt: this is what moves charge through the electrodes, so it is
                // what drains SOC, drives the RC pairs, and is charged for throughput.
                // A shorted cell's terminal current is smaller by `v_node · shunt_g`
                // — the part that never leaves the cell.
                let i_k = (e_k - v_node) / r_k;
                // Heat from the same start-of-step state that produced `i_k`, so
                // the energy accounting closes exactly (see `cell_heat_w`).
                let mut q = cell.model.heat_w(&self.chem, i_k, r_k);
                // An internal short dissipates inside this cell, unlike the balancing
                // bleed (whose resistor is outside every cell and heats nothing). The
                // branch is what keeps a healthy pack's heat bit-for-bit what it was.
                if cell.shunt_g > 0.0 {
                    let i_shunt = v_node * cell.shunt_g;
                    i_internal_short_a += i_shunt;
                    q += v_node * i_shunt;
                }
                q_gen_w += q;
                if thermal_live {
                    // Series-major, parallel-minor — the index order `thermal`
                    // expects.
                    heat_w.push(q);
                }
                let soc_before = cell.model.soc();
                let temp_before = cell.model.temp_k();
                let eff_cap = cap_ah * cell.capacity_factor;
                let soh_cap = cell.aging.soh_capacity;
                // Plating: cold, charging, and above the C-rate threshold, judged from
                // the same start-of-step state that produced `i_k`. This is an
                // *observation*, so unlike every accumulator it is deliberately not
                // gated on `dt > 0` — a zero-length probe step reports the conditions
                // it finds, exactly as it already reports `q_gen_w` and `BALANCING`.
                // Nothing here mutates, so that stays free.
                let plating =
                    safety.is_some_and(|s| plating_risk(s, i_k, temp_before, eff_cap * soh_cap));
                if plating {
                    flags |= EventFlags::PLATING_RISK;
                }
                flags |= cell.model.advance(&self.chem, i_k, dt, eff_cap, soh_cap);
                if aging_accumulates {
                    // Throughput from the cell's own current, half-cycle direction
                    // from the pack's — see `CellAging::accumulate` for why the two
                    // differ, and what goes wrong if they do not.
                    cell.aging.accumulate(i_k, i_g, dt, soc_before, plating);
                }
            }
        }

        // --- integrate temperatures against that heat.
        //
        // Exothermic self-heating rides this integrator rather than the per-cell heat
        // tally above, because unlike every other heat term it is a function of the
        // temperature being solved for: held constant across the step it would be
        // meaningless, and the whole phenomenon is the feedback. That is also why
        // runaway is gated on a live network — an `Isothermal` pack has no temperature
        // dynamics for the reaction to act on, so it never reacts. Venting is the
        // exception and is judged in the reporting pass below, because "this cell got
        // that hot" is an observation that means the same thing in either mode.
        let mut q_runaway_w = 0.0;
        if let ThermalConfig::Network { k_neighbor_w_per_k } = self.thermal {
            let t_env = env.t_coolant.unwrap_or(env.t_ambient);
            let onset_k = safety.map_or(f64::INFINITY, |s| s.t_onset_k);
            let mut temps: Vec<f64> = Vec::with_capacity(n_cells);
            // One comparison per cell is the whole cost of runaway on a pack that is
            // not on fire: below onset nothing can react, so the per-cell exothermic
            // state is not even gathered and the integrator takes its original path.
            let mut any_at_onset = false;
            for group in &self.groups {
                for cell in &group.cells {
                    let t = cell.model.temp_k();
                    any_at_onset |= t >= onset_k;
                    temps.push(t);
                }
            }
            let mut runaway: Vec<CellRunaway> = if any_at_onset {
                let mut v = Vec::with_capacity(n_cells);
                for group in &self.groups {
                    for cell in &group.cells {
                        v.push(cell.runaway);
                    }
                }
                v
            } else {
                Vec::new()
            };
            let mut scratch = Vec::with_capacity(n_cells);
            let report = advance_temperatures(
                &mut temps,
                &mut scratch,
                &heat_w,
                &mut runaway,
                &ThermalStep {
                    series,
                    parallel,
                    params: &self.chem.thermal,
                    k_neighbor_w_per_k,
                    t_env,
                    dt,
                    safety,
                },
            );
            if report.reacting {
                flags |= EventFlags::THERMAL_RUNAWAY;
                // Mean reaction power over the step. At `dt == 0` no time passed for
                // anything to be released, so report the instantaneous rate the same
                // `reaction_power` would give — one definition, evaluated at the same
                // start-of-step temperatures the gate used, and the limit of the mean
                // as `dt` shrinks.
                q_runaway_w = if dt > 0.0 {
                    report.released_j / dt
                } else {
                    let s = safety.expect("`reacting` implies safety is Some");
                    temps
                        .iter()
                        .zip(runaway.iter())
                        .map(|(&t, r)| reaction_power(s, t, r.energy_remaining_j))
                        .sum()
                };
            }
            let mut i = 0;
            let write_runaway = runaway.len() == n_cells;
            for group in &mut self.groups {
                for cell in &mut group.cells {
                    cell.model.set_temp_k(temps[i]);
                    if write_runaway {
                        cell.runaway = runaway[i];
                    }
                    i += 1;
                }
            }
        }
        self.sim_time_s += dt;

        // --- aging, on its own coarse sub-clock. Deliberately sequenced *here*:
        // after the temperatures and SOCs it reads have settled, and before the
        // reporting pass below — which recomputes every cell's Thévenin source and
        // memoises it. Running aging after that pass would leave the memo describing
        // pre-aging resistances, poisoning the next step with no invalidation point
        // to catch it. Running it here costs nothing and needs no invalidation at all.
        if aging_accumulates {
            // Both `expect`s are structural, not hopeful: `aging_accumulates` implies
            // `self.aging.is_some()`, and `Pack::new` rejects that configuration
            // unless the chemistry supplies coefficients.
            let elapsed = self
                .aging
                .as_mut()
                .expect("aging_accumulates implies aging is configured")
                .advance(dt);
            if let Some(dt_age) = elapsed {
                let params = self
                    .chem
                    .aging
                    .as_ref()
                    .expect("Pack::new rejects aging config without chemistry coefficients");
                let safety = self.chem.safety.as_ref();
                // Borrow the two mutable fields once, up front: `groups` and `rng` are
                // disjoint, and naming them here keeps the plating roll below from
                // looking like a second mutable borrow of `self`.
                let rng = &mut self.rng;
                // Series-major, parallel-minor — the same order the scatter draws use,
                // and the order the plating short roll's determinism depends on. Any
                // reordering here silently changes which cell gets which draw and
                // therefore the whole trajectory; see `CellAging::tick`.
                for group in &mut self.groups {
                    for cell in &mut group.cells {
                        let (temp_k, soc) = (cell.model.temp_k(), cell.model.soc());
                        let shorted = cell.aging.tick(params, safety, dt_age, temp_k, soc, rng);
                        if shorted {
                            // A plating short is physically the same object an injected
                            // `SoftInternalShort` creates, so it lands the same way:
                            // conductances add, and the shunt is *not* an input to the
                            // memoised Thévenin source, so this needs no cache
                            // invalidation (see [`crate::faults`]). It takes effect from
                            // the next step's solve, because this step's currents were
                            // already computed — which is the same one-step latency
                            // every other end-of-step state change has.
                            //
                            // `plating_short_ohms > 0` is guaranteed here, but only by
                            // a chain across three files: `validate()` requires it
                            // whenever the hazard is positive, `short_probability`
                            // returns zero unless the hazard is positive, and `tick`
                            // only rolls on a positive probability. The assert makes
                            // that chain mechanically checked instead of narratively
                            // true — the same reason the Thévenin memo carries one.
                            if let Some(s) = safety {
                                debug_assert!(
                                    s.plating_short_ohms > 0.0,
                                    "a plating short rolled against a non-positive \
                                     resistance: validation should have rejected this \
                                     chemistry"
                                );
                                cell.shunt_g += 1.0 / s.plating_short_ohms;
                            }
                        }
                    }
                }
            }
        }

        // --- end-of-step reporting. Recompute each group's shared node voltage
        // from end-of-step state with the same pack current, so parallel cells
        // report one consistent terminal voltage (v_cell is per group).
        let mut v_terminal = 0.0;
        let mut v_cell_min = f64::INFINITY;
        let mut v_cell_max = f64::NEG_INFINITY;
        let mut t_min = f64::INFINITY;
        let mut t_max = f64::NEG_INFINITY;
        let mut rem_ah = 0.0; // Σ soc_k · eff_cap_k  (eff_cap folds in SOH)
        let mut nom_ah = 0.0; // Σ eff_cap_k
                              // Pack SOH aggregates. Both are gated on aging being live, so a pack
                              // that cannot age pays nothing for them and reports exactly 1.0.
                              //
                              // Capacity: Σ(cap·factor·soh) / Σ(cap·factor) — capacity-weighted, the
                              // same shape `soc_true` already uses, and it reads as "fraction of
                              // nominal pack capacity still there".
                              //
                              // Resistance: r_pack / r_pack_nominal, the ratio of what the pack's cells
                              // actually present to what unworn cells would. Accumulated from the same
                              // conductances the voltage solve needs, but **excluding the bleed
                              // conductance** — a closed balancing switch lowers the group's impedance
                              // without any cell having got better, and it would otherwise wander into
                              // a number that is supposed to be about health.
        let aging_live = self.aging.is_some();
        let mut cap_nominal_ah = 0.0; // Σ cap·factor, SOH excluded
        let mut r_pack_cells = 0.0; // Σ_g 1/Σ_k G_k
        let mut r_pack_nominal = 0.0; // Σ_g 1/Σ_k soh_k·G_k
                                      // Group voltages are gathered only when something will sense them.
        let mut v_group: Vec<f64> = if self.bms.is_some() {
            Vec::with_capacity(series)
        } else {
            Vec::new()
        };
        // Venting is judged here, from end-of-step temperatures, so it also covers an
        // `Isothermal` pack (which never enters the integrator that latches it
        // in-flight) and a cell built above the threshold. The per-cell bit is the
        // state; the flag below is re-derived from it every step rather than being
        // remembered, which is what keeps `EventFlags`'s "not sticky" contract honest —
        // `VENTED` means "this pack contains a vented cell", exactly as
        // `CONTACTOR_OPEN` means "this pack's contactor is open".
        let mut any_vented = false;
        for (g_idx, group) in self.groups.iter_mut().enumerate() {
            let mut sum_g = bleed_at(g_idx);
            let mut sum_eg = 0.0;
            let mut sum_g_cells = 0.0;
            let mut sum_g_nominal = 0.0;
            for (k, cell) in group.cells.iter_mut().enumerate() {
                let hot = safety.is_some_and(|s| is_vented(s, cell.model.temp_k()));
                // Read for reporting always; *write* only when time passed. A probe step
                // is an observation, and latching an irreversible bit on one would let
                // the act of looking at a hot pack change its state — the fourth member
                // of the `dt > 0` family after the BMS sensor clock, the aging sub-clock
                // and the fault queue. `PLATING_RISK` needs no such gate only because
                // detecting it mutates nothing; this does.
                if hot && sensor_tick {
                    cell.runaway.vented = true;
                }
                any_vented |= cell.runaway.vented || hot;
                let (e, r) = cell.model.source(&self.chem, cell.eff_r0_factor());
                // This is the *next* step's start-of-step source: nothing below
                // mutates a cell, so the state it was computed from is the state the
                // next step will find. Memoise it (see [`SourceCache`]).
                cell_src[g_idx * parallel + k] = (e, r);
                let g = 1.0 / r;
                // Same membership as the start-of-step aggregation above: the shunt is
                // part of the node's conductance, so the reported group voltage is the
                // one the next step will solve at. `sum_g_cells` below deliberately
                // excludes it — a shorted cell is not a healthier cell.
                sum_g += g + cell.shunt_g;
                sum_eg += e * g;
                t_min = t_min.min(cell.model.temp_k());
                t_max = t_max.max(cell.model.temp_k());
                let cap_nominal = cap_ah * cell.capacity_factor;
                let eff_cap = cap_nominal * cell.aging.soh_capacity;
                rem_ah += cell.model.soc() * eff_cap;
                nom_ah += eff_cap;
                if aging_live {
                    cap_nominal_ah += cap_nominal;
                    sum_g_cells += g;
                    // 1/r_nominal = soh_resistance/r, since r already carries it.
                    sum_g_nominal += cell.aging.soh_resistance * g;
                }
            }
            if aging_live {
                r_pack_cells += 1.0 / sum_g_cells;
                r_pack_nominal += 1.0 / sum_g_nominal;
            }
            let v_g = (sum_eg - i_g) / sum_g; // = E_g' − I_g·R_g'
            v_terminal += v_g;
            v_cell_min = v_cell_min.min(v_g);
            v_cell_max = v_cell_max.max(v_g);
            if self.bms.is_some() {
                v_group.push(v_g);
            }
        }

        // --- sample the sensors from the end-of-step state, for the *next* step to
        // act on. This is the only place ground truth crosses into the BMS, and it
        // crosses through the sensor error model.
        if sensor_tick {
            if let Some(bms) = &self.bms {
                // Read the probes through a shared borrow: `groups` and `bms` are
                // distinct fields, so this needs no copy of the probe list.
                let temp_probe_k: Vec<f64> = bms
                    .config()
                    .temp_probes
                    .iter()
                    .map(|&(s, p)| {
                        // Validated in range at construction; the fallback keeps
                        // `step` panic-free should that ever stop holding.
                        self.groups
                            .get(s as usize)
                            .and_then(|g| g.cells.get(p as usize))
                            .map_or(f64::NAN, |c| c.model.temp_k())
                    })
                    .collect();
                let sim_time_s = self.sim_time_s;
                let bms = self.bms.as_mut().expect("matched as Some just above");
                bms.sample(v_group, temp_probe_k, i_g, sim_time_s, &mut self.rng);
                // Injected sensor faults are the last word — applied after the
                // sensor's own offset and noise, and after the noise draw has already
                // happened, so a stuck sensor reads exactly its stuck value without
                // shifting the RNG stream (see [`crate::faults`]).
                bms.corrupt_sensors(self.faults.sensor_faults());
            }
        }
        if any_vented {
            flags |= EventFlags::VENTED;
        }
        let soc_bms = self.bms.as_ref().map(Bms::soc_estimate);
        // Hand the buffer back, now holding the next step's start-of-step sources.
        self.src_cache = SourceCache(cell_src);

        Telemetry {
            v_terminal,
            i_actual: i_g,
            soc_true: rem_ah / nom_ah,
            soc_bms,
            t_min,
            t_max,
            v_cell_min,
            v_cell_max,
            // Without aging these are exactly 1.0 by construction, not by rounding:
            // every cell's SOH is the literal 1.0 the pack was built with.
            soh_capacity: if aging_live {
                nom_ah / cap_nominal_ah
            } else {
                1.0
            },
            soh_resistance: if aging_live {
                r_pack_cells / r_pack_nominal
            } else {
                1.0
            },
            q_gen_w,
            q_runaway_w,
            q_balancing_w,
            i_balancing_a,
            i_internal_short_a,
            i_external_short_a,
            flags,
        }
    }

    /// Clear a latched BMS hard fault, closing the contactor. Returns `true` if a
    /// fault was actually cleared.
    ///
    /// This is the operator-reset seam. A hard fault (a measurement past a chemistry
    /// limit *plus* its configured margin) latches the contactor open precisely so
    /// that it does not silently re-close when the condition passes; getting the pack
    /// back requires an explicit decision from outside the engine.
    pub fn clear_bms_fault(&mut self) -> bool {
        self.bms.as_mut().is_some_and(Bms::clear_fault)
    }

    /// The pack's BMS, or `None` if it has none.
    ///
    /// Gives a client (or a test) the BMS's *beliefs* — its SOC estimate and the last
    /// sensor frame — for side-by-side comparison against the ground truth in
    /// [`Telemetry`] and [`Pack::cell`]. That comparison is the point.
    #[must_use]
    pub fn bms(&self) -> Option<&Bms> {
        self.bms.as_ref()
    }
}

/// Check a [`BmsConfig`]'s numeric ranges and probe positions.
fn validate_bms(bms: &BmsConfig, series: u16, parallel: u16) -> Result<(), BuildError> {
    let checks: [(&'static str, f64, &'static str, bool); 6] = [
        (
            "current_offset_a",
            bms.current_offset_a,
            "must be finite",
            bms.current_offset_a.is_finite(),
        ),
        (
            "current_noise_sigma_a",
            bms.current_noise_sigma_a,
            "must be finite and >= 0",
            bms.current_noise_sigma_a.is_finite() && bms.current_noise_sigma_a >= 0.0,
        ),
        (
            "rest_current_threshold_a",
            bms.rest_current_threshold_a,
            "must be finite and >= 0",
            bms.rest_current_threshold_a.is_finite() && bms.rest_current_threshold_a >= 0.0,
        ),
        (
            "rest_time_for_ocv_s",
            bms.rest_time_for_ocv_s,
            "must be finite and >= 0",
            bms.rest_time_for_ocv_s.is_finite() && bms.rest_time_for_ocv_s >= 0.0,
        ),
        (
            "ocv_correction_gain",
            bms.ocv_correction_gain,
            "must be in [0, 1]",
            (0.0..=1.0).contains(&bms.ocv_correction_gain),
        ),
        (
            "min_ocv_slope_v_per_soc",
            bms.min_ocv_slope_v_per_soc,
            "must be finite and >= 0",
            bms.min_ocv_slope_v_per_soc.is_finite() && bms.min_ocv_slope_v_per_soc >= 0.0,
        ),
    ];
    for (field, value, reason, ok) in checks {
        if !ok {
            return Err(BuildError::BadBmsConfig {
                field,
                reason,
                value,
            });
        }
    }
    if let Some(bal) = bms.balancing {
        if !(bal.bleed_r_ohms.is_finite() && bal.bleed_r_ohms > 0.0) {
            return Err(BuildError::BadBmsConfig {
                field: "balancing.bleed_r_ohms",
                reason: "must be finite and > 0",
                value: bal.bleed_r_ohms,
            });
        }
        if !bal.v_threshold_v.is_finite() {
            return Err(BuildError::BadBmsConfig {
                field: "balancing.v_threshold_v",
                reason: "must be finite",
                value: bal.v_threshold_v,
            });
        }
    }
    if let Some(prot) = bms.protection {
        let margins: [(&'static str, f64); 2] = [
            ("protection.v_hard_margin_v", prot.v_hard_margin_v),
            ("protection.t_hard_margin_k", prot.t_hard_margin_k),
        ];
        for (field, value) in margins {
            if !(value.is_finite() && value >= 0.0) {
                return Err(BuildError::BadBmsConfig {
                    field,
                    reason: "must be finite and >= 0",
                    value,
                });
            }
        }
    }
    // `initial_soc_error` is deliberately unconstrained beyond finiteness: a BMS may
    // legitimately boot believing something absurd, and the estimate is clamped to
    // [0, 1] anyway.
    if !bms.initial_soc_error.is_finite() {
        return Err(BuildError::BadBmsConfig {
            field: "initial_soc_error",
            reason: "must be finite",
            value: bms.initial_soc_error,
        });
    }
    for (index, &(s, p)) in bms.temp_probes.iter().enumerate() {
        if s >= series || p >= parallel {
            return Err(BuildError::BadTempProbe {
                index,
                s,
                p,
                series,
                parallel,
            });
        }
    }
    Ok(())
}

/// Lower bound on a scatter factor. A Gaussian has unbounded tails; a factor at or
/// below zero would divide by zero in the group solve (`1/R`) or make coulomb
/// counting blow up, so draws are clamped to this positive floor. It only bites at
/// extreme sigma (realistic manufacturing scatter is a few percent).
const MIN_FACTOR: f64 = 0.05;

/// Draw one cell's `(capacity_factor, r0_factor)` from the pack RNG.
///
/// With no scatter (both sigmas zero) this is exactly `(1.0, 1.0)` and does **not**
/// touch the RNG — a no-scatter pack leaves the RNG at its seed until something
/// genuinely random happens. Otherwise each factor is `1 + σ·z` for an independent
/// standard-normal `z` (Box–Muller), clamped to [`MIN_FACTOR`]. A zero sigma on one
/// axis still yields exactly `1.0` for that axis while the other is perturbed.
fn draw_factors(rng: &mut ChaCha8Rng, scatter: &Scatter) -> (f64, f64) {
    if scatter.capacity_sigma == 0.0 && scatter.r0_sigma == 0.0 {
        return (1.0, 1.0);
    }
    // Box–Muller yields two independent standard normals from two uniforms; use
    // one for each axis so capacity and R0 scatter are independent.
    let (z0, z1) = standard_normal_pair(rng);
    let cap = (1.0 + scatter.capacity_sigma * z0).max(MIN_FACTOR);
    let r0 = (1.0 + scatter.r0_sigma * z1).max(MIN_FACTOR);
    (cap, r0)
}
