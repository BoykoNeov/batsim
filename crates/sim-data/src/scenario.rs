//! Scenario files: a pack's initial condition and its pre-programmed misfortunes.
//!
//! A [`Scenario`] is **not** a demand program. It says what pack exists, on what
//! chemistry, and what faults are queued against it; what to *do* with that pack —
//! discharge it, charge it, rest it — stays with the client. That line is what keeps
//! a server a server rather than a scripting engine, and it is why this format
//! composes entirely from types the engine already defines: it cannot drift from the
//! engine, because it *is* the engine's types.
//!
//! ```toml
//! # Exactly one of `chemistry` / `chemistry_toml`, and both are top-level keys, so
//! # they must appear *above* the first table header or TOML swallows them into it.
//! chemistry = "lfp_26650_generic"
//!
//! [meta]
//! name = "a pack"
//!
//! [pack]                  # exactly PackConfig
//! series = 4
//! parallel = 2
//! initial_soc = 0.5
//! initial_temp_k = 298.15
//! seed = 42
//!
//! [[faults]]              # exactly ScheduledFault
//! at_s = 600.0
//! fault = { SoftInternalShort = { s = 1, p = 0, ohms = 5.0 } }
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};
use sim_core::{ChemistryParams, Pack, PackConfig, ScheduledFault};

use crate::{parse_chemistry, DataError};

/// Human-facing labelling for a scenario. Not consumed by the engine.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioMeta {
    /// Short name, e.g. "overcharge with the BMS off".
    pub name: String,
    /// Optional longer prose: what the scenario is meant to show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Where a scenario's chemistry parameters come from.
///
/// Returned by [`Scenario::chemistry_source`] so an adapter can branch without
/// re-deriving the "exactly one of two keys" rule that [`Scenario::validate`] has
/// already enforced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChemistrySource<'a> {
    /// A chemistry id the adapter resolves however it can — `sim-server` maps it to
    /// `<chem_dir>/<id>.toml`; a browser fetches it. The id is already known to match
    /// `[a-z0-9_]+`, so it cannot contain a path separator.
    Id(&'a str),
    /// The chemistry TOML inlined verbatim, which is what makes a scenario
    /// self-contained for a host that ships no `chemistries/` tree. Already known to
    /// parse and validate.
    Inline(&'a str),
}

/// A pack's initial condition, its chemistry, and its scheduled faults.
///
/// # Unknown keys
/// This struct is `deny_unknown_fields`, so a typo at the scenario's own level is a
/// parse error. That attribute is deliberately **not** retrofitted onto
/// [`PackConfig`] and friends: those are engine types with a compatibility surface of
/// their own, and adding it here would be sim-data quietly changing the engine's
/// contract. The asymmetry is real and worth knowing — an unknown key inside
/// `[pack]` is silently ignored, an unknown key beside `[pack]` is rejected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// Chemistry id for the adapter to resolve. Exactly one of this and
    /// [`Self::chemistry_toml`] must be set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chemistry: Option<String>,
    /// The chemistry parameter set, inlined verbatim as TOML text. Exactly one of
    /// this and [`Self::chemistry`] must be set.
    ///
    /// Two plainly-named optional keys rather than one untagged enum: untagged enums
    /// in TOML are a known sharp edge, and "exactly one of these" is a two-line check
    /// with a message a scenario author can act on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chemistry_toml: Option<String>,
    /// Human-facing labelling.
    pub meta: ScenarioMeta,
    /// Topology and initial conditions — exactly [`PackConfig`], serde as it already
    /// is.
    pub pack: PackConfig,
    /// Faults queued against the pack, in **file order**.
    ///
    /// File order is load-bearing and is preserved through parsing: the engine's
    /// queue sorts by [`ScheduledFault::at_s`] and breaks ties by scheduling order,
    /// so two faults sharing a timestamp fire in the order they are written here.
    /// Nothing requires the list to be sorted — [`Scenario::build_pack`] feeds it to
    /// the engine as written and lets the queue do the sorting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faults: Vec<ScheduledFault>,
}

impl Scenario {
    /// Which of the two chemistry keys this scenario uses.
    ///
    /// Total, because [`Self::validate`] has already rejected the zero- and two-key
    /// cases. A `Scenario` built in Rust rather than obtained from [`parse_scenario`]
    /// — which is exactly what an adapter does when it constructs a session
    /// programmatically — can still violate that, so both degenerate cases have a
    /// defined answer rather than a panic:
    ///
    /// * **both keys set** resolves to the id. Preferring the id over the inline text
    ///   is not arbitrary: an id names something the caller wrote, so the error it
    ///   eventually produces ("no such chemistry: …") points back at the input.
    ///   Returning `Inline("")` here would discard a perfectly good id and fail with a
    ///   chemistry-parse error naming nothing at all.
    /// * **neither key set** resolves to `Inline("")`, which fails to parse. There is
    ///   no better answer available; there is nothing to point at.
    ///
    /// Call [`Self::validate`] to get a message that names the actual problem.
    #[must_use]
    pub fn chemistry_source(&self) -> ChemistrySource<'_> {
        match (&self.chemistry, &self.chemistry_toml) {
            (Some(id), _) => ChemistrySource::Id(id),
            (None, Some(text)) => ChemistrySource::Inline(text),
            (None, None) => ChemistrySource::Inline(""),
        }
    }

    /// Check the parts of a scenario the **engine cannot check for itself**.
    ///
    /// Deliberately narrow. `Pack::new` already rejects a zero topology, an out-of-range
    /// `initial_soc`, a non-positive `initial_temp_k`, a bad thermal conductance, and
    /// every out-of-range `BmsConfig` field; `Pack::schedule_fault` already rejects a
    /// non-finite `at_s`, a non-positive short resistance, and an out-of-topology cell
    /// index. Mirroring those here would give one condition two error messages that
    /// could drift apart. What is left is what nothing downstream would catch:
    ///
    /// * exactly one chemistry key is set,
    /// * the chemistry id is a bare `[a-z0-9_]+` name — no separators and no dots, so
    ///   a scenario cannot walk out of a server's chemistry directory,
    /// * an inlined chemistry actually parses and validates, so a scenario is either
    ///   whole or rejected rather than failing later at a distance,
    /// * the [`sim_core::Scatter`] sigmas are finite and `>= 0`. This is the one real
    ///   gap in the engine's own checks: a NaN sigma is not rejected, and
    ///   `(1.0 + NaN·z).max(floor)` silently returns the floor, so every cell comes out
    ///   pinned at the minimum factor and nothing says so.
    ///
    /// # Errors
    /// [`DataError::Scenario`] for any of the above, or [`DataError::Toml`] /
    /// [`DataError::Invalid`] from parsing an inlined chemistry.
    pub fn validate(&self) -> Result<(), DataError> {
        match (&self.chemistry, &self.chemistry_toml) {
            (None, None) => {
                return Err(DataError::Scenario(
                    "a scenario needs a chemistry: set either `chemistry` (an id the \
                     host resolves) or `chemistry_toml` (the parameter set inlined). \
                     Both are top-level keys and must appear above the first [table] \
                     header."
                        .into(),
                ))
            }
            (Some(_), Some(_)) => {
                return Err(DataError::Scenario(
                    "`chemistry` and `chemistry_toml` are alternatives; set exactly one".into(),
                ))
            }
            (Some(id), None) => validate_chemistry_id(id)?,
            (None, Some(text)) => {
                parse_chemistry(text)?;
            }
        }

        for (field, sigma) in [
            ("capacity_sigma", self.pack.scatter.capacity_sigma),
            ("r0_sigma", self.pack.scatter.r0_sigma),
        ] {
            if !sigma.is_finite() || sigma < 0.0 {
                return Err(DataError::Scenario(format!(
                    "pack.scatter.{field} must be finite and >= 0, got {sigma}"
                )));
            }
        }

        Ok(())
    }

    /// Build the pack this scenario describes and queue its faults.
    ///
    /// The chemistry is supplied by the caller because resolving one is the adapter's
    /// job and every adapter does it differently — see [`ChemistrySource`]. For an
    /// inlined chemistry the parameters are already in hand:
    /// `parse_chemistry(text)` on [`ChemistrySource::Inline`].
    ///
    /// Faults are scheduled in file order, so equal timestamps fire in the order they
    /// were written (see [`Scenario::faults`]).
    ///
    /// # Errors
    /// [`DataError::Build`] if the topology or configuration is invalid for this
    /// chemistry, or [`DataError::Fault`] if a queued fault does not fit the pack it
    /// targets (non-finite parameter, cell index outside the topology, a sensor fault
    /// on a pack with no BMS).
    pub fn build_pack(&self, chem: ChemistryParams) -> Result<Pack, DataError> {
        let mut pack = Pack::new(&self.pack, chem)?;
        for scheduled in &self.faults {
            pack.schedule_fault(scheduled.at_s, scheduled.fault)?;
        }
        Ok(pack)
    }
}

/// A chemistry id must be a bare lowercase name: it becomes a filename on a server
/// that resolves ids against a directory, so anything that could denote a path is
/// rejected before it can be joined onto one.
fn validate_chemistry_id(id: &str) -> Result<(), DataError> {
    if id.is_empty() {
        return Err(DataError::Scenario("`chemistry` must not be empty".into()));
    }
    if let Some(bad) = id
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '_'))
    {
        return Err(DataError::Scenario(format!(
            "chemistry id {id:?} contains {bad:?}; ids must match [a-z0-9_]+ so that \
             resolving one against a directory cannot leave it"
        )));
    }
    Ok(())
}

/// Parse and validate a scenario from TOML text.
///
/// # Errors
/// [`DataError::Toml`] if the text is malformed, does not match the schema, or
/// carries an unknown key beside `[pack]`; otherwise any error from
/// [`Scenario::validate`].
pub fn parse_scenario(text: &str) -> Result<Scenario, DataError> {
    let scenario: Scenario = toml::from_str(text)?;
    scenario.validate()?;
    Ok(scenario)
}

/// Read, parse, and validate a scenario from a TOML file on disk.
///
/// # Errors
/// [`DataError::Io`] if the file cannot be read, or any error from
/// [`parse_scenario`].
pub fn load_scenario_file(path: impl AsRef<Path>) -> Result<Scenario, DataError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_scenario(&text)
}
