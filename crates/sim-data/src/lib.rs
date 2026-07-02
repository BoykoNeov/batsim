//! `sim-data` — TOML chemistry-parameter loading and validation.
//!
//! Parses `chemistries/*.toml` parameter sets and validates them (monotone OCV
//! table, positive resistances, ordered limits, …) into `sim-core` chemistry
//! parameters. TOML parsing lives here, never in `sim-core`.
//!
//! This is a Phase 0 scaffold; loading/validation is implemented over the phased
//! build plan (see `CLAUDE.md`).
