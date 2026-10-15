//! Phase 11 exit 3: the LG M50 file's placeholders are exactly a fixed list, and every
//! constant the phase replaced says where it came from.
//!
//! # The list
//! `docs/plans/phase-11-fitting.md`, decision 1: the constants with no source the project
//! holds — cycle fade (2), reversal (3), runaway (5), plating (4), the film's starting
//! thickness (1) — and the drawn calendar curve (3 keys, decision 4: the shelf lessons teach
//! against it). Slice D added one the decisions did not name, `r_growth_per_capacity_loss`:
//! the WMG cells **bound** it (0.5–2.9 % per %, 1.5 inside) without fitting it — a bound is not
//! a fit, the same reason cycle fade stays listed although LG's sheet bounds it. The list may
//! only shrink, and only by a later note that says what replaced each constant it drops.
//!
//! # What "labelled" means
//! A key's own label: its inline comment and the comment block directly above it, no blank
//! line between. Section prose ("All placeholders, scaled from …") labels nothing, so a key
//! the prose sits over, like `t_plating_min_k`, is not swept in by it. The words are
//! `placeholder` and `TODO`, case-insensitive.
//!
//! # What "says where it came from" means
//! A replaced key's provenance — its own label, the comments above it up to the nearest blank
//! line or section header, and its section's preamble (the comments between the content
//! before the header and the first key) — must name its source and, for a fit, its residual.
//! The phrases are listed per key below; a deleted residual turns this red, which a grep for
//! the absence of "placeholder" would not.

use std::collections::BTreeSet;

const LGM50: &str = include_str!("../../../chemistries/nmc_21700_lgm50.toml");

/// The fixed list: `section.key`, `[[rc]]` named `rc`.
const PLACEHOLDERS: &[&str] = &[
    "reversal.v_per_soc",
    "reversal.floor_v",
    "reversal.fade_per_ah",
    "aging.cal_pre_exp",
    "aging.cal_ea_j_per_mol",
    "aging.cal_soc_stress",
    "aging.cyc_fade_per_ah",
    "aging.cyc_dod_stress_exp",
    "aging.r_growth_per_capacity_loss",
    "safety.t_onset_k",
    "safety.t_vent_k",
    "safety.runaway_energy_j",
    "safety.runaway_power_w_at_onset",
    "safety.runaway_ea_j_per_mol",
    "safety.plating_c_threshold",
    "safety.plating_fade_per_ah",
    "safety.plating_short_hazard_per_ah",
    "safety.plating_short_ohms",
    "spm.sei.initial_thickness_m",
];

/// LG's specification sheet, as every provenance that uses it must name it (decision 1).
const LG_SHEET: &[&str] = &[
    "LRB-PS-CY18.2Wh-M50",
    "2017-09-22",
    "revision 0",
    "CONFIDENTIAL",
    "Tentative",
    "retailer",
];

const FIT_ECM: &[&str] = &[
    "fit_ecm.py",
    "1C 25 degC 4.85 mV",
    "3C 25 degC",
    "18.81 mV",
    "1C -10 degC 11.15 mV",
    "1C +45 degC 3.51 mV",
];

/// Every constant Phase 11 replaced, with the phrases its provenance must carry.
fn replaced() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut v: Vec<(&str, Vec<&str>)> = vec![
        ("cell.max_charge_c", [LG_SHEET, &["4.2.7"]].concat()),
        ("cell.max_discharge_c", [LG_SHEET, &["4.2.9"]].concat()),
        ("cell.t_max_k", [LG_SHEET, &["2.7"]].concat()),
        (
            "thermal.heat_capacity_j_per_k",
            [LG_SHEET, &["ORegan2022", "68.5", "55.16"]].concat(),
        ),
        ("ocv.soc", vec!["fit_ocv.py", "0.92 mV"]),
        ("ocv.volts", vec!["fit_ocv.py", "0.92 mV"]),
        (
            "ocv.docv_dt_v_per_k",
            vec!["ORegan2022", "extract_oregan.py"],
        ),
        ("ocv.t_ref_k", vec!["ORegan2022", "decision 3"]),
        (
            "r0.rc_t_ref_k",
            [FIT_ECM, &["decision 6", "22.38"]].concat(),
        ),
        ("rc.r_ohms", vec!["FITTED (above)"]),
        ("rc.c_farad", vec!["FITTED (above)"]),
        (
            "spm.negative.diffusivity_ea_j_per_mol",
            vec!["ORegan2022", "d = 2092"],
        ),
        (
            "spm.positive.diffusivity_ea_j_per_mol",
            vec!["ORegan2022", "d = 1449"],
        ),
    ];
    for k in ["r0.soc", "r0.temp_k", "r0.ohms"] {
        v.push((k, FIT_ECM.to_vec()));
    }
    for side in ["negative", "positive"] {
        for k in ["stoich", "v_per_k"] {
            let key: &'static str = Box::leak(format!("spm.{side}.entropic.{k}").into_boxed_str());
            v.push((key, vec!["ORegan2022", "uV/K", "cross-checked at 7 points"]));
        }
    }
    for k in ["form", "t_ref_k", "diffusivity", "conductivity"] {
        let key: &'static str =
            Box::leak(format!("dfn.electrolyte_temperature.{k}").into_boxed_str());
        v.push((key, vec!["Landesfeind 2019", "EC:EMC 3:7"]));
    }
    v
}

/// One key of the file with the comment text around it.
struct Key {
    name: String,
    /// The inline comment and the comment block directly above.
    own: String,
    /// `own` and the comments above up to a blank line or header: the key's paragraph.
    para: String,
    /// `para` and the section's preamble.
    wide: String,
}

fn comment(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('#')
}

/// A line's inline comment, outside any string.
fn inline(line: &str) -> &str {
    let mut quoted = false;
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[i + 1..],
            _ => {}
        }
    }
    ""
}

fn header(line: &str) -> Option<String> {
    let t = line.trim();
    let name = t
        .strip_prefix("[[")
        .and_then(|r| r.strip_suffix("]]"))
        .or_else(|| t.strip_prefix('[').and_then(|r| r.strip_suffix(']')))?;
    Some(name.trim().to_string())
}

fn key_name(line: &str) -> Option<&str> {
    let (k, _) = line.split_once('=')?;
    let k = k.trim();
    let ok = !k.is_empty()
        && !line.trim_start().starts_with('#')
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then_some(k)
}

fn scan(text: &str) -> Vec<Key> {
    let lines: Vec<&str> = text.lines().collect();
    let mut keys = Vec::new();
    let mut section = String::new();
    let mut preamble = String::new();
    // Comments seen since the last content line, blank lines allowed: becomes the preamble
    // of the next section header.
    let mut since_content = String::new();
    let mut in_array = 0_i32;
    for (n, &line) in lines.iter().enumerate() {
        if in_array > 0 {
            in_array += bracket_depth(line);
            continue;
        }
        if let Some(c) = comment(line) {
            since_content.push_str(c);
            since_content.push('\n');
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if let Some(h) = header(line) {
            section = if h == "rc" { "rc".into() } else { h };
            preamble = std::mem::take(&mut since_content);
            continue;
        }
        let Some(k) = key_name(line) else {
            since_content.clear();
            continue;
        };
        let mut own = String::from(inline(line));
        let mut wide = own.clone();
        let mut i = n;
        let mut directly_above = true;
        while i > 0 {
            i -= 1;
            let up = lines[i];
            if up.trim().is_empty() || header(up).is_some() {
                break;
            }
            match comment(up) {
                Some(c) => {
                    if directly_above {
                        own.push('\n');
                        own.push_str(c);
                    }
                    wide.push('\n');
                    wide.push_str(c);
                }
                None => directly_above = false,
            }
        }
        let para = wide.clone();
        wide.push('\n');
        wide.push_str(&preamble);
        keys.push(Key {
            name: format!("{section}.{k}"),
            own,
            para,
            wide,
        });
        since_content.clear();
        // A value that opens an array and does not close it on this line runs on.
        let value = line.split_once('=').map_or("", |(_, v)| v);
        let value = &value[..value.len() - inline(line).len().min(value.len())];
        in_array = bracket_depth(value).max(0);
    }
    keys
}

/// Net `[` minus `]` outside strings and comments.
fn bracket_depth(line: &str) -> i32 {
    let body = &line[..line.len() - inline(line).len()];
    let mut quoted = false;
    let mut d = 0;
    for ch in body.chars() {
        match ch {
            '"' => quoted = !quoted,
            '[' if !quoted => d += 1,
            ']' if !quoted => d -= 1,
            _ => {}
        }
    }
    d
}

fn labelled(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("placeholder") || t.contains("todo")
}

/// Every key the `toml` crate parses, named as `scan` names them: an array of `[[…]]` tables
/// is walked into, an array of inline tables is one key.
fn parsed_keys(text: &str) -> BTreeSet<String> {
    fn walk(prefix: &str, t: &toml::Table, aot: &BTreeSet<String>, out: &mut BTreeSet<String>) {
        for (k, v) in t {
            let name = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            match v {
                toml::Value::Table(sub) => walk(&name, sub, aot, out),
                toml::Value::Array(a) if aot.contains(&name) => {
                    for item in a {
                        walk(&name, item.as_table().expect("table"), aot, out);
                    }
                }
                _ => {
                    out.insert(name);
                }
            }
        }
    }
    let table: toml::Table = text.parse().expect("the shipped file is TOML");
    let aot: BTreeSet<String> = text
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            t.strip_prefix("[[")?
                .strip_suffix("]]")
                .map(|n| n.trim().to_string())
        })
        .collect();
    let mut out = BTreeSet::new();
    walk("", &table, &aot, &mut out);
    out
}

fn check(text: &str) -> Result<(), String> {
    let keys = scan(text);
    let scanned: BTreeSet<String> = keys.iter().map(|k| k.name.clone()).collect();
    let parsed = parsed_keys(text);
    if scanned != parsed {
        return Err(format!(
            "the scanner and the TOML parser disagree: only scanned {:?}, only parsed {:?}",
            scanned.difference(&parsed).collect::<Vec<_>>(),
            parsed.difference(&scanned).collect::<Vec<_>>()
        ));
    }
    let found: BTreeSet<&str> = keys
        .iter()
        .filter(|k| labelled(&k.own))
        .map(|k| k.name.as_str())
        .collect();
    let list: BTreeSet<&str> = PLACEHOLDERS.iter().copied().collect();
    if found != list {
        return Err(format!(
            "labelled but not on the list: {:?}; on the list but not labelled: {:?}",
            found.difference(&list).collect::<Vec<_>>(),
            list.difference(&found).collect::<Vec<_>>()
        ));
    }
    for (name, phrases) in replaced() {
        // Every occurrence: `[[rc]]` repeats its keys, once per pair.
        let all: Vec<&Key> = keys.iter().filter(|k| k.name == name).collect();
        if all.is_empty() {
            return Err(format!("{name}: not in the file"));
        }
        for (n, k) in all.iter().enumerate() {
            if labelled(&k.own) {
                return Err(format!(
                    "{name} #{n}: replaced, but still labelled a placeholder"
                ));
            }
            for p in &phrases {
                if !k.wide.contains(p) {
                    return Err(format!("{name} #{n}: provenance does not say {p:?}"));
                }
            }
        }
    }
    // Decision 1: wherever a constant's provenance uses the sheet, its marking goes with it —
    // in the key's own paragraph, not borrowed from a section's prose (the file's header names
    // the sheet as a summary and is nobody's provenance).
    for k in &keys {
        if k.para.contains("LG sheet") || k.para.contains("LG's specification sheet") {
            for p in LG_SHEET {
                if !k.para.contains(p) {
                    return Err(format!("{}: cites LG's sheet without {p:?}", k.name));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn the_placeholders_are_the_fixed_list_and_the_rest_is_sourced() {
    assert_eq!(
        PLACEHOLDERS.len(),
        19,
        "15 + 3 for the calendar curve + 1 bounded"
    );
    if let Err(e) = check(LGM50) {
        panic!("{e}");
    }
}

/// Each rule, broken once in a copy of the file, turns the check red **for its own reason**
/// (each case names a fragment its failure must contain) — so none is satisfied by
/// construction, and none is red only because another rule fired first.
#[test]
fn each_rule_fails_when_broken() {
    // (what, text to find once, its replacement, a fragment the failure must contain)
    let cases: &[(&str, &str, &str, &str)] = &[
        (
            "a listed key loses its label",
            "# provenance: placeholder -- a declared limit rather than a measured plateau.",
            "# provenance: a declared limit rather than a measured plateau.",
            "on the list but not labelled: [\"reversal.floor_v\"]",
        ),
        (
            "an unlisted key gains one",
            "t_plating_min_k     = 273.15   # provenance: plating risk below 0 degC",
            "t_plating_min_k     = 273.15   # provenance: placeholder plating risk below 0 degC",
            "labelled but not on the list: [\"safety.t_plating_min_k\"]",
        ),
        (
            "the sheet loses its marking",
            "marked CONFIDENTIAL and Tentative (a draft",
            "(a draft",
            "cell.max_charge_c #0: provenance does not say \"CONFIDENTIAL\"",
        ),
        (
            "a constant cites the sheet without it",
            "v_max           = 4.20     # provenance: Chen2020",
            "v_max           = 4.20     # provenance: LG sheet, Chen2020",
            "cell.v_max: cites LG's sheet without",
        ),
        (
            "the fit loses a residual",
            "1C -10 degC 11.15 mV",
            "1C -10 degC",
            "provenance does not say \"1C -10 degC 11.15 mV\"",
        ),
        (
            "the second pair loses its source",
            "# Slow pair. provenance: FITTED (above)",
            "# Slow pair.",
            "rc.r_ohms #1: provenance does not say \"FITTED (above)\"",
        ),
        (
            "a key the scanner cannot see",
            "floor_v   = 0.0",
            "floor_v   = 0.0
extra.depth = 1.0",
            "the scanner and the TOML parser disagree: only scanned [], only parsed [\"reversal.extra.depth\"]",
        ),
    ];
    check(LGM50).expect("the shipped file passes, or a broken copy failing proves nothing");
    for (what, from, to, expect) in cases {
        let n = LGM50.matches(from).count();
        assert_eq!(n, 1, "{what}: {from:?} occurs {n} times");
        let err = check(&LGM50.replacen(from, to, 1))
            .expect_err(&format!("{what}: the check stayed green"));
        assert!(
            err.contains(expect),
            "{what}: red for another reason: {err}"
        );
    }
}
