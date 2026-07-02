//! `sim-data` — TOML chemistry-parameter loading and validation.
//!
//! Parses `chemistries/*.toml` parameter sets into [`sim_core::ChemistryParams`]
//! and runs the engine's own [`ChemistryParams::validate`] on the result. All
//! format-specific parsing (the `toml` crate) lives here; `sim-core` stays free of
//! file formats and I/O.

use std::path::Path;

use sim_core::{ChemistryError, ChemistryParams};
use thiserror::Error;

/// Ways loading a chemistry can fail.
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
