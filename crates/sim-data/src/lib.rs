//! `sim-data` — TOML loading and validation for the engine's input files.
//!
//! Two formats, both text-first because a browser has no filesystem:
//!
//! * **chemistries** (`chemistries/*.toml`) → [`sim_core::ChemistryParams`], validated
//!   by the engine's own [`ChemistryParams::validate`];
//! * **scenarios** (`scenarios/*.toml`) → [`Scenario`], a pack's initial condition and
//!   its queued faults.
//!
//! All format-specific parsing (the `toml` crate) lives here; `sim-core` stays free of
//! file formats and I/O. The `load_*_file` functions are thin `std::fs` wrappers over
//! the `parse_*` ones, so a host without files simply does not call them.

use std::path::Path;

use sim_core::{BuildError, ChemistryError, ChemistryParams, FaultError};
use thiserror::Error;

pub mod diagram;
pub mod scenario;

pub use diagram::{ChemistryFacts, DiagramFamily, DiagramParams};
pub use scenario::{load_scenario_file, parse_scenario, ChemistrySource, Scenario, ScenarioMeta};

/// Ways loading a chemistry or a scenario can fail.
#[derive(Debug, Error)]
pub enum DataError {
    /// The file could not be read.
    #[error("reading {path}: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The TOML text was malformed or did not match the schema.
    #[error("parsing TOML: {0}")]
    Toml(#[from] toml::de::Error),
    /// The parsed parameters failed physical/structural validation.
    #[error("invalid chemistry: {0}")]
    Invalid(#[from] ChemistryError),
    /// A scenario parsed as TOML but is not a usable scenario.
    ///
    /// Its own variant rather than a reuse of [`Self::Invalid`]: these are the checks
    /// no engine type could make (which chemistry key is set, whether an id is safe to
    /// join onto a directory), so borrowing [`ChemistryError`]'s name for them would
    /// misreport what failed.
    #[error("invalid scenario: {0}")]
    Scenario(String),
    /// A scenario's pack could not be built for the chemistry it names.
    #[error("building the scenario's pack: {0}")]
    Build(#[from] BuildError),
    /// A chemistry's `[diagram]` section disagrees with its physics.
    ///
    /// Its own variant for the same reason [`Self::Scenario`] is: these are checks no
    /// engine type can make, because the engine never reads the section. See
    /// [`diagram`] for the two rules.
    #[error("invalid [diagram]: {0}")]
    Diagram(String),
    /// A scenario's queued fault does not fit the pack it targets.
    #[error("scheduling a scenario fault: {0}")]
    Fault(#[from] FaultError),
}

/// Parse and validate a chemistry from TOML text.
///
/// # Errors
/// Returns [`DataError::Toml`] if the text is malformed or does not match the
/// schema, or [`DataError::Invalid`] if it parses but violates a physical
/// invariant (non-monotone OCV, non-positive resistance, out-of-order limits, …).
pub fn parse_chemistry(text: &str) -> Result<ChemistryParams, DataError> {
    let params: ChemistryParams = toml::from_str(text)?;
    params.validate()?;
    Ok(params)
}

/// Parse a chemistry and its `[diagram]` section into what a client needs to draw it.
///
/// Validates the chemistry exactly as [`parse_chemistry`] does — a file that fails there
/// fails here the same way — and then checks the two rules [`diagram`] states: the
/// `cold_charge` caption is present exactly when the file can plate, and the `runaway`
/// caption exactly when the file has `[safety]`. A file with no `[diagram]` at all is not
/// an error; `diagram` is `None` and a client draws a generic cell.
///
/// # Errors
/// Any error from [`parse_chemistry`]; [`DataError::Toml`] if the `[diagram]` table has
/// a wrong or unknown key; [`DataError::Diagram`] if a caption is present without the
/// mechanism it describes, or absent with it.
pub fn parse_chemistry_facts(text: &str) -> Result<ChemistryFacts, DataError> {
    let chem = parse_chemistry(text)?;
    let file: diagram::DiagramFile = toml::from_str(text)?;
    let safety = chem.safety.as_ref();
    let plating = safety.and_then(|s| s.t_plating_min_k);
    if let Some(d) = file.diagram.as_ref() {
        let rule = |caption: &str, present: bool, modelled: bool, gate: &str| {
            match (present, modelled) {
            (true, false) => Err(DataError::Diagram(format!(
                "`{caption}` caption on a chemistry with no {gate}: it describes a mechanism this file does not model"
            ))),
            (false, true) => Err(DataError::Diagram(format!(
                "{gate} is present but `[diagram]` has no `{caption}` caption: the state it raises would be drawn with nothing said"
            ))),
            _ => Ok(()),
        }
        };
        rule(
            "cold_charge",
            d.cold_charge.is_some(),
            plating.is_some(),
            "`[safety].t_plating_min_k`",
        )?;
        rule(
            "runaway",
            d.runaway.is_some(),
            safety.is_some(),
            "`[safety]`",
        )?;
    }
    Ok(ChemistryFacts {
        id: chem.meta.id.clone(),
        name: chem.meta.name.clone(),
        capacity_ah: chem.cell.capacity_ah,
        v_max: chem.cell.v_max,
        v_min: chem.cell.v_min,
        t_charge_min_k: chem.cell.t_charge_min_k,
        t_max_k: chem.cell.t_max_k,
        t_onset_k: safety.map(|s| s.t_onset_k),
        t_vent_k: safety.map(|s| s.t_vent_k),
        t_plating_min_k: plating,
        plating_c_threshold: safety.and_then(|s| s.plating_c_threshold),
        charge_acceptance_onset: chem.charge_acceptance.as_ref().map(|c| c.soc_onset),
        has_hysteresis: chem.hysteresis.is_some(),
        has_diffusion: chem.diffusion.is_some(),
        has_spm: chem.spm.is_some(),
        has_dfn: chem.dfn.is_some(),
        has_aging: chem.aging.is_some(),
        diagram: file.diagram,
    })
}

/// Read, parse, and validate a chemistry from a TOML file on disk.
///
/// # Errors
/// Returns [`DataError::Io`] if the file cannot be read, or any error from
/// [`parse_chemistry`].
pub fn load_chemistry_file(path: impl AsRef<Path>) -> Result<ChemistryParams, DataError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|source| DataError::Io {
        path: path.display().to_string(),
        source,
    })?;
    parse_chemistry(&text)
}
