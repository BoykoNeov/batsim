//! The `[diagram]` section: every shipped chemistry carries one, the loader ties its
//! captions to the file's physics, and the engine never sees it.
//!
//! What the section is *for* is a browser panel no test here can look at; what a test can
//! hold is the contract between the file and the page — that the seven files parse, that
//! their families are what the page's three drawings expect, and that the two caption
//! rules in `sim_data::diagram` reject exactly the mismatches they claim to.

use sim_core::ChemistryParams;
use sim_data::{parse_chemistry, parse_chemistry_facts, DataError, DiagramFamily};

const LFP: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");
const NMC: &str = include_str!("../../../chemistries/nmc_18650_generic.toml");
const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");
const NA_ION: &str = include_str!("../../../chemistries/na_ion_18650_generic.toml");
const LTO: &str = include_str!("../../../chemistries/lto_20ah_generic.toml");
const NIMH: &str = include_str!("../../../chemistries/nimh_subc_3ah_generic.toml");
const PBA: &str = include_str!("../../../chemistries/pba_agm_2v_generic.toml");

const SHIPPED: [(&str, &str, DiagramFamily); 7] = [
    ("lfp_26650_generic", LFP, DiagramFamily::Intercalation),
    ("nmc_18650_generic", NMC, DiagramFamily::Intercalation),
    ("nmc_21700_lgm50", LGM50, DiagramFamily::Intercalation),
    ("na_ion_18650_generic", NA_ION, DiagramFamily::Intercalation),
    ("lto_20ah_generic", LTO, DiagramFamily::Intercalation),
    ("nimh_subc_3ah_generic", NIMH, DiagramFamily::Nickel),
    ("pba_agm_2v_generic", PBA, DiagramFamily::LeadAcid),
];

/// Drop the one line that sets exactly `key` (not a key that merely starts with it —
/// `runaway` must not take `[safety]`'s `runaway_energy_j` with it). Line endings are
/// normalised first: the shipped files are a mix of LF and CRLF in a checkout.
fn without_line(text: &str, key: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let mut seen = false;
    let out: Vec<&str> = text
        .lines()
        .filter(|l| {
            let rest = l.trim_start().strip_prefix(key);
            let hit = rest.is_some_and(|r| r.trim_start().starts_with('='));
            if hit {
                seen = true;
            }
            !hit
        })
        .collect();
    assert!(seen, "no `{key}` line to drop");
    out.join("\n")
}

/// Add one line to the top of a file's `[diagram]` table.
fn with_line(text: &str, line: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let out = text.replace("[diagram]\n", &format!("[diagram]\n{line}\n"));
    assert_ne!(out, text, "no `[diagram]` header to add under");
    out
}

/// Cut the whole `[diagram]` table (it is the last table in every shipped file).
fn without_diagram(text: &str) -> String {
    let at = text.find("[diagram]").expect("a [diagram] table");
    text[..at].to_owned()
}

#[test]
fn every_shipped_chemistry_carries_a_diagram_of_the_family_the_page_draws_it_as() {
    for (id, text, family) in SHIPPED {
        let facts = parse_chemistry_facts(text).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(facts.id, id);
        let d = facts
            .diagram
            .as_ref()
            .unwrap_or_else(|| panic!("{id}: no [diagram]"));
        assert_eq!(d.family, family, "{id}");
        for (what, s) in [
            ("carrier", &d.carrier),
            ("negative", &d.negative),
            ("positive", &d.positive),
            ("electrolyte", &d.electrolyte),
            ("negative_collector", &d.negative_collector),
            ("positive_collector", &d.positive_collector),
            ("overcharge", &d.overcharge),
            ("deep_discharge", &d.deep_discharge),
        ] {
            assert!(!s.trim().is_empty(), "{id}: empty `{what}`");
        }
    }
}

#[test]
fn the_captions_are_present_exactly_where_the_physics_they_describe_is() {
    for (id, text, _) in SHIPPED {
        let chem = parse_chemistry(text).unwrap();
        let d = parse_chemistry_facts(text).unwrap().diagram.unwrap();
        let can_plate = chem
            .safety
            .as_ref()
            .is_some_and(|s| s.t_plating_min_k.is_some());
        assert_eq!(
            d.cold_charge.is_some(),
            can_plate,
            "{id}: cold_charge vs plating gate"
        );
        assert_eq!(
            d.runaway.is_some(),
            chem.safety.is_some(),
            "{id}: runaway vs [safety]"
        );
    }
    // And the split is not degenerate: both answers occur among the seven.
    let plating: Vec<bool> = SHIPPED
        .iter()
        .map(|(_, t, _)| parse_chemistry_facts(t).unwrap().t_plating_min_k.is_some())
        .collect();
    assert!(plating.contains(&true) && plating.contains(&false));
    let safety: Vec<bool> = SHIPPED
        .iter()
        .map(|(_, t, _)| parse_chemistry_facts(t).unwrap().t_onset_k.is_some())
        .collect();
    assert!(safety.contains(&true) && safety.contains(&false));
}

#[test]
fn a_missing_cold_charge_caption_on_a_cell_that_plates_is_rejected() {
    let err = parse_chemistry_facts(&without_line(LFP, "cold_charge")).unwrap_err();
    assert!(matches!(err, DataError::Diagram(_)), "{err}");
    assert!(err.to_string().contains("cold_charge"), "{err}");
}

#[test]
fn a_cold_charge_caption_on_a_cell_that_cannot_plate_is_rejected() {
    let text = with_line(LTO, "cold_charge = \"this cell does not plate\"");
    let err = parse_chemistry_facts(&text).unwrap_err();
    assert!(matches!(err, DataError::Diagram(_)), "{err}");
    assert!(err.to_string().contains("cold_charge"), "{err}");
}

#[test]
fn a_missing_runaway_caption_on_a_cell_with_safety_is_rejected() {
    let err = parse_chemistry_facts(&without_line(LFP, "runaway")).unwrap_err();
    assert!(matches!(err, DataError::Diagram(_)), "{err}");
    assert!(err.to_string().contains("runaway"), "{err}");
}

#[test]
fn a_runaway_caption_on_a_cell_without_safety_is_rejected() {
    let text = with_line(NIMH, "runaway = \"nickel does not\"");
    let err = parse_chemistry_facts(&text).unwrap_err();
    assert!(matches!(err, DataError::Diagram(_)), "{err}");
    assert!(err.to_string().contains("runaway"), "{err}");
}

#[test]
fn an_unknown_diagram_key_is_a_parse_error_not_a_silent_default() {
    let text = with_line(LFP, "carier = \"typo\"");
    let err = parse_chemistry_facts(&text).unwrap_err();
    assert!(matches!(err, DataError::Toml(_)), "{err}");
    assert!(err.to_string().contains("carier"), "{err}");
}

#[test]
fn a_file_with_no_diagram_is_not_an_error_and_says_so() {
    let facts = parse_chemistry_facts(&without_diagram(LFP)).unwrap();
    assert!(facts.diagram.is_none());
    assert_eq!(facts.id, "lfp_26650_generic");
}

#[test]
fn the_engine_never_sees_the_section() {
    // Same `ChemistryParams` with and without the table — the engine's own parse ignores
    // it, so no solve, no golden and no snapshot can depend on a word of it.
    let with: ChemistryParams = parse_chemistry(LFP).unwrap();
    let without: ChemistryParams = parse_chemistry(&without_diagram(LFP)).unwrap();
    assert_eq!(
        serde_json::to_string(&with).unwrap(),
        serde_json::to_string(&without).unwrap()
    );
}

#[test]
fn the_facts_copy_the_validated_limits_and_report_the_optional_sections_as_presence() {
    let lfp = parse_chemistry_facts(LFP).unwrap();
    let chem = parse_chemistry(LFP).unwrap();
    assert_eq!(lfp.capacity_ah, chem.cell.capacity_ah);
    assert_eq!(lfp.v_max, chem.cell.v_max);
    assert_eq!(lfp.v_min, chem.cell.v_min);
    assert_eq!(lfp.t_plating_min_k, Some(273.15));
    assert!(lfp.t_onset_k.is_some() && lfp.t_vent_k.is_some());
    assert!(!lfp.has_spm && !lfp.has_dfn && !lfp.has_hysteresis && !lfp.has_diffusion);
    assert!(lfp.charge_acceptance_onset.is_none());

    let lgm50 = parse_chemistry_facts(LGM50).unwrap();
    assert!(lgm50.has_spm && lgm50.has_dfn);

    let nimh = parse_chemistry_facts(NIMH).unwrap();
    assert!(nimh.has_hysteresis);
    assert!(nimh.charge_acceptance_onset.is_some());
    assert!(nimh.t_onset_k.is_none() && nimh.t_plating_min_k.is_none());

    let pba = parse_chemistry_facts(PBA).unwrap();
    assert!(pba.has_diffusion);
    assert!(pba.t_onset_k.is_none());

    let lto = parse_chemistry_facts(LTO).unwrap();
    assert!(lto.t_onset_k.is_some() && lto.t_plating_min_k.is_none());
}
