//! The `[diagram]` section: what a cell is made of, for a client that draws it.
//!
//! Read by **no engine code**. `sim_core::ChemistryParams` does not deny unknown tables,
//! so the section is invisible to every solve and every test the engine runs; it is
//! parsed here, separately, by [`crate::parse_chemistry_facts`], and lands on the wire as
//! part of [`ChemistryFacts`]. That is the whole reason it lives in this crate and not in
//! `sim-core`: it is chemistry *knowledge*, not chemistry *physics*, and `CLAUDE.md`'s rule
//! that a chemistry is data applies to both. A new lithium chemistry gets a diagram by
//! writing seven strings; a client needs new code only for a new *mechanism*.
//!
//! Two of its captions are tied to the file's physics and validated against it, because a
//! caption that names a mechanism the file does not model is a false claim on screen:
//!
//! * `cold_charge` is required exactly when `[safety].t_plating_min_k` is present — the
//!   flag it explains (`EventFlags::PLATING_RISK`) can rise only then.
//! * `runaway` is required exactly when `[safety]` is present — thermal runaway is
//!   modelled only then, and a nickel file that carries no `[safety]` ships no such story.

use serde::{Deserialize, Serialize};

/// Which of the client's mechanism drawings this chemistry uses.
///
/// Three, and the set is closed on purpose: the *drawing* of a mechanism is code, so the
/// enum is the list of drawings the client has. A chemistry that fits none of them is a
/// client slice, not a data change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramFamily {
    /// The carrier ion shuttles between two host lattices through an inert electrolyte
    /// (every lithium chemistry, sodium-ion). Full means the carrier sits in the negative.
    Intercalation,
    /// Hydrogen shuttles between a metal-hydride negative and a nickel positive; the
    /// electrolyte carries it as hydroxide going the *other* way and is not consumed.
    /// Overcharge evolves oxygen at the positive that recombines, as heat, at the negative.
    Nickel,
    /// Both plates convert to lead sulfate on discharge and the sulfate comes *out of the
    /// electrolyte*: full means the carrier is in the acid, empty means it is on the
    /// plates. Overcharge splits water into gas.
    LeadAcid,
}

/// The `[diagram]` table of a chemistry file.
///
/// `deny_unknown_fields`, unlike the engine's own sections: a misspelled key here cannot
/// be caught by a matched-pair rule the way `[safety]`'s plating pair is, and the cost of
/// an ignored key is a label that silently falls back to a generic word on screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramParams {
    /// Which drawing the client uses.
    pub family: DiagramFamily,
    /// The species that crosses the separator, as the client labels it (`"Li⁺"`).
    pub carrier: String,
    /// Negative electrode material, as labelled (`"graphite"`).
    pub negative: String,
    /// Positive electrode material, as labelled (`"LiFePO₄"`).
    pub positive: String,
    /// The electrolyte, as labelled.
    pub electrolyte: String,
    /// Negative current collector material (`"copper"`). It is what corrodes below empty
    /// on a lithium cell, so it is drawn and named.
    pub negative_collector: String,
    /// Positive current collector material (`"aluminium"`).
    pub positive_collector: String,
    /// One sentence for the moment a full cell is offered more charge: what the refused
    /// current turns into. Prose, not physics; the engine decides *when*, this says *what*.
    pub overcharge: String,
    /// One sentence for a cell driven past empty into reversal.
    pub deep_discharge: String,
    /// One sentence for a charge below the plating temperature. Present exactly when the
    /// file's `[safety].t_plating_min_k` is — see the module doc.
    #[serde(default)]
    pub cold_charge: Option<String>,
    /// One sentence for a cell above its runaway onset. Present exactly when the file has
    /// a `[safety]` section — see the module doc.
    #[serde(default)]
    pub runaway: Option<String>,
}

/// The `[diagram]` table alone, read off a chemistry file's text.
///
/// Its own struct rather than a field on `ChemistryParams`, so `sim-core` never sees it.
#[derive(Debug, Deserialize)]
pub(crate) struct DiagramFile {
    #[serde(default)]
    pub(crate) diagram: Option<DiagramParams>,
}

/// What a client needs to know about a chemistry to draw it and to scale what it draws,
/// without a pack and without a snapshot.
///
/// Every number is a copy of a validated `ChemistryParams` field — the client never
/// parses TOML — plus the `[diagram]` section. The `has_*` booleans are the optional
/// sections a client might draw differently (a hysteresis loop, a Peukert penalty, a
/// particle), reported as presence only: their values are the engine's business.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChemistryFacts {
    /// `[meta].id`.
    pub id: String,
    /// `[meta].name`.
    pub name: String,
    /// Nominal capacity \[Ah\]; what turns a current into a C-rate.
    pub capacity_ah: f64,
    /// Protection upper voltage \[V\].
    pub v_max: f64,
    /// Protection lower voltage \[V\].
    pub v_min: f64,
    /// Charge-inhibit temperature \[K\].
    pub t_charge_min_k: f64,
    /// Over-temperature limit \[K\].
    pub t_max_k: f64,
    /// Runaway onset \[K\], when the file has `[safety]`.
    pub t_onset_k: Option<f64>,
    /// Vent temperature \[K\], when the file has `[safety]`.
    pub t_vent_k: Option<f64>,
    /// Plating temperature gate \[K\], when the chemistry can plate.
    pub t_plating_min_k: Option<f64>,
    /// C-rate above which a cold charge plates, present exactly with `t_plating_min_k`.
    pub plating_c_threshold: Option<f64>,
    /// `[charge_acceptance].soc_onset`, when present: above it the cell turns a growing
    /// share of a charging current into heat rather than stored charge.
    pub charge_acceptance_onset: Option<f64>,
    /// Whether the file carries `[hysteresis]`.
    pub has_hysteresis: bool,
    /// Whether the file carries `[diffusion]`.
    pub has_diffusion: bool,
    /// Whether the file carries `[spm]`.
    pub has_spm: bool,
    /// Whether the file carries `[dfn]`.
    pub has_dfn: bool,
    /// Whether the file carries `[aging]`.
    pub has_aging: bool,
    /// The `[diagram]` section, or `None` for a file that has none — a client draws a
    /// generic, unlabelled cell then, and says so.
    pub diagram: Option<DiagramParams>,
}
