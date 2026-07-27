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
    /// finite and `> 0`, and below [`Self::t_vent_k`]. **Not yet consumed** — the
    /// runaway slice owns it.
    pub t_onset_k: f64,
    /// Cell temperature \[K\] at which a cell vents. Must be finite and above
    /// [`Self::t_onset_k`]. **Not yet consumed.**
    pub t_vent_k: f64,
    /// Finite exothermic energy budget of one cell \[J\]. Must be finite and `>= 0`.
    /// **Not yet consumed.**
    pub runaway_energy_j: f64,
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
            let non_negative: [(&'static str, f64); 4] = [
                ("safety.runaway_energy_j", s.runaway_energy_j),
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
        }
        Ok(())
    }
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
