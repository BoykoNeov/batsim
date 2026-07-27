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

pub mod scenario;

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
