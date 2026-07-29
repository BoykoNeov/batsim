//! Scenario-file tests: the shipped examples, the format's rejections, and one
//! end-to-end demonstration that a file can stand in for a pack built in Rust.

use sim_core::{
    AgingConfig, BalancingConfig, BmsConfig, CellModelConfig, Demand, Env, EventFlags, Fault,
    PackConfig, ProtectionConfig, Scatter, ScheduledFault, SensorId, ThermalConfig,
};
use sim_data::{parse_chemistry, parse_scenario, ChemistrySource, DataError, Scenario};

const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");
const CC_DISCHARGE: &str = include_str!("../../../scenarios/cc_discharge_lfp.toml");
const SOFT_SHORT: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

// ---------------------------------------------------------------------------
// The shipped examples
// ---------------------------------------------------------------------------

/// The minimal example parses, and `chemistry` is read as the top-level key it is.
///
/// That last clause is the whole reason this assertion names the id: the key sits
/// above `[meta]` in the file, and moving it one line down would make TOML read it as
/// `meta.chemistry` — a silently *different* document that still parses, because
/// `chemistry` is optional and `[meta]` would just gain an unknown field. Only
/// `deny_unknown_fields` on `ScenarioMeta` and this check stand between that mistake
/// and a scenario with no chemistry at all.
#[test]
fn cc_discharge_example_parses() {
    let s = parse_scenario(CC_DISCHARGE).expect("the shipped CC example should parse");

    assert_eq!(
        s.chemistry_source(),
        ChemistrySource::Id("lfp_26650_generic")
    );
    assert_eq!(
        s.pack,
        PackConfig {
            series: 1,
            parallel: 1,
            initial_soc: 1.0,
            initial_temp_k: 298.15,
            seed: 1,
            scatter: Scatter::default(),
            thermal: ThermalConfig::Isothermal,
            bms: None,
            aging: None,
            cell_model: CellModelConfig::Ecm,
        },
        "every omitted section must mean 'off', not 'on with defaults'"
    );
    assert!(s.faults.is_empty());
}

/// The fault example is exactly the `PackConfig` a person would write in Rust — every
/// nested enum, option, and tuple included.
///
/// Written out in full rather than spot-checked: this is the assertion that catches a
/// serde field name or enum shape that TOML happens to accept but does not mean what
/// the file says. A partial check would pass with `[pack.thermal.Network]` silently
/// falling back to `Isothermal`.
#[test]
fn soft_short_example_matches_a_hand_built_config() {
    let s = parse_scenario(SOFT_SHORT).expect("the shipped fault example should parse");

    assert_eq!(
        s.pack,
        PackConfig {
            series: 4,
            parallel: 2,
            initial_soc: 0.85,
            initial_temp_k: 298.15,
            seed: 20_260_727,
            scatter: Scatter {
                capacity_sigma: 0.02,
                r0_sigma: 0.03,
            },
            thermal: ThermalConfig::Network {
                k_neighbor_w_per_k: 0.5,
            },
            aging: Some(AgingConfig {
                sub_clock_period_s: 10.0,
            }),
            bms: Some(BmsConfig {
                balancing: Some(BalancingConfig {
                    bleed_r_ohms: 33.0,
                    v_threshold_v: 3.45,
                }),
                protection: Some(ProtectionConfig {
                    v_hard_margin_v: 0.15,
                    t_hard_margin_k: 10.0,
                }),
                current_offset_a: 0.02,
                current_noise_sigma_a: 0.01,
                temp_probes: vec![(0, 0), (3, 0)],
                initial_soc_error: 0.03,
                rest_current_threshold_a: 0.05,
                rest_time_for_ocv_s: 600.0,
                ocv_correction_gain: 0.1,
                min_ocv_slope_v_per_soc: 0.15,
            }),
            cell_model: CellModelConfig::Ecm,
        }
    );

    // File order, preserved. Both faults share a timestamp, so the order they are
    // written in is the order they fire in — see `Scenario::faults`.
    assert_eq!(
        s.faults,
        vec![
            ScheduledFault {
                at_s: 600.0,
                fault: Fault::SoftInternalShort {
                    s: 1,
                    p: 0,
                    ohms: 5.0,
                },
            },
            ScheduledFault {
                at_s: 600.0,
                fault: Fault::SensorOffset {
                    sensor: SensorId::GroupVoltage(1),
                    offset: 0.12,
                },
            },
        ]
    );
}

/// The example earns its name: built and stepped, it produces the divergence it
/// claims — group 1 drains, and the BMS does not see it.
///
/// This is what makes the format a *user* of the engine rather than a document that
/// merely parses. It exercises the whole path an adapter will take — parse, resolve
/// the chemistry, build, schedule, step — and it fails if any of them silently drops
/// something the file asked for.
#[test]
fn soft_short_example_runs_and_diverges() {
    let s = parse_scenario(SOFT_SHORT).unwrap();
    let chem = parse_chemistry(LFP_TOML).unwrap();
    let mut pack = s.build_pack(chem).expect("the example must build");

    // 20 minutes at 1 s: 10 before the fault, 10 after.
    let mut v_group_1_reported_before = 0.0;
    let mut seen = EventFlags::empty();
    let mut worst_v_cell = f64::INFINITY;
    let mut worst_t_max = 0.0_f64;
    for step in 0..1200 {
        let tele = pack.step(1.0, Demand::Current(0.5), &env());
        seen |= tele.flags;
        worst_v_cell = worst_v_cell.min(tele.v_cell_min);
        worst_t_max = worst_t_max.max(tele.t_max);
        if step == 599 {
            v_group_1_reported_before = pack.bms().unwrap().sensors().v_group[1];
        }
    }

    assert!(
        !seen.contains(EventFlags::CONTACTOR_OPEN),
        "the BMS should never trip here — it is being lied to; saw {seen:?}"
    );
    // And it should not be *close* to tripping, or a later edit to the example would
    // make this test fail in a way that reads as "the file is broken" rather than
    // "the margin got tight". Protection trips at v_min − 0.15 V = 1.85 V and at
    // t_max + 10 K = 343.15 K; the run sits an order of magnitude away from both,
    // because 0.5 A on a 4.6 Ah group is ~0.11 C and a 5-ohm short across 3.3 V
    // dissipates ~2 W into a cell with 0.35 W/K of convective coupling.
    assert!(
        worst_v_cell > 3.0,
        "undervoltage headroom is gone: lowest cell reached {worst_v_cell} V"
    );
    assert!(
        worst_t_max < 320.0,
        "overtemperature headroom is gone: hottest cell reached {worst_t_max} K"
    );

    // Ground truth: the shorted group has lost more charge than its neighbours.
    let shorted = pack.cell(1, 0).unwrap().soc;
    let healthy = pack.cell(2, 0).unwrap().soc;
    assert!(
        shorted < healthy - 1e-4,
        "the shorted group should be measurably lower: {shorted} vs {healthy}"
    );
    assert!(
        pack.cell(1, 0).unwrap().internal_short_conductance_s > 0.0,
        "the scheduled short must actually have fired"
    );

    // The BMS view: the sensor on that group reads *higher* after the offset landed,
    // even though the group behind it is the one that is draining.
    let v_group_1_reported_after = pack.bms().unwrap().sensors().v_group[1];
    assert!(
        v_group_1_reported_after > v_group_1_reported_before,
        "the +120 mV offset should more than cover the sag: \
         {v_group_1_reported_before} -> {v_group_1_reported_after}"
    );
}

// ---------------------------------------------------------------------------
// Rejections
// ---------------------------------------------------------------------------

/// A scenario body with one `{}` hole above the first table header, where the
/// chemistry key(s) go. Everything else is the smallest valid scenario, so a failure
/// can only come from the substitution.
fn scenario_with_chemistry(chemistry: &str) -> String {
    format!(
        r#"
{chemistry}

[meta]
name = "test"

[pack]
series = 1
parallel = 1
initial_soc = 0.5
initial_temp_k = 298.15
seed = 0
"#
    )
}

#[test]
fn a_scenario_needs_exactly_one_chemistry_key() {
    let none = parse_scenario(&scenario_with_chemistry(""));
    assert!(
        matches!(none, Err(DataError::Scenario(ref m)) if m.contains("chemistry_toml")),
        "no chemistry at all should name both keys, got {none:?}"
    );

    let both = scenario_with_chemistry(&format!(
        "chemistry = \"lfp_26650_generic\"\nchemistry_toml = \"\"\"{LFP_TOML}\"\"\""
    ));
    assert!(
        matches!(parse_scenario(&both), Err(DataError::Scenario(m)) if m.contains("exactly one")),
        "setting both should be rejected"
    );
}

/// A `Scenario` built in Rust never went through `parse_scenario`, so
/// `chemistry_source` has to stay total on inputs `validate` would have rejected —
/// and it has to degrade toward the input that names something.
///
/// An adapter constructing a session programmatically is the realistic caller here.
/// Resolving a both-keys-set scenario to the *id* means the error it eventually
/// raises points back at what the caller wrote; resolving it to an empty inline text
/// would throw the id away and fail with a chemistry-parse error naming nothing.
#[test]
fn chemistry_source_stays_total_on_an_unvalidated_scenario() {
    let mut s = parse_scenario(&scenario_with_chemistry(
        "chemistry = \"lfp_26650_generic\"",
    ))
    .unwrap();

    s.chemistry_toml = Some(LFP_TOML.to_owned());
    assert!(s.validate().is_err(), "two keys is still invalid");
    assert_eq!(
        s.chemistry_source(),
        ChemistrySource::Id("lfp_26650_generic"),
        "a set id must not be discarded in favour of the inline text"
    );

    s.chemistry = None;
    s.chemistry_toml = None;
    assert!(s.validate().is_err());
    assert!(
        matches!(s.chemistry_source(), ChemistrySource::Inline("")),
        "with nothing to point at, the answer is the one that fails to parse"
    );
}

/// A chemistry id becomes a filename on a server that resolves ids against a
/// directory, so anything that could denote a path is rejected before it can be
/// joined onto one.
#[test]
fn traversal_shaped_chemistry_ids_are_rejected() {
    for id in [
        "../../etc/passwd",
        "sub/dir",
        "lfp_26650_generic.toml",
        "LFP_26650",
        "lfp-26650",
        "",
    ] {
        let text = scenario_with_chemistry(&format!("chemistry = \"{id}\""));
        assert!(
            matches!(parse_scenario(&text), Err(DataError::Scenario(_))),
            "id {id:?} should be rejected"
        );
    }
    // The shipped ids are, of course, fine.
    for id in ["lfp_26650_generic", "nmc_18650_generic"] {
        let text = scenario_with_chemistry(&format!("chemistry = \"{id}\""));
        parse_scenario(&text).unwrap_or_else(|e| panic!("id {id:?} should be accepted: {e}"));
    }
}

/// An inlined chemistry is parsed and validated at scenario-parse time, so a
/// self-contained scenario is whole or rejected — never accepted and then found
/// broken at build time, somewhere else, with no filename in hand.
#[test]
fn an_inlined_chemistry_is_validated_eagerly() {
    let good = scenario_with_chemistry(&format!("chemistry_toml = \"\"\"{LFP_TOML}\"\"\""));
    let s = parse_scenario(&good).expect("an inlined valid chemistry should parse");
    assert!(matches!(s.chemistry_source(), ChemistrySource::Inline(t) if t.contains("[ocv]")));
    // And it is the same parameter set as loading the file directly.
    match s.chemistry_source() {
        ChemistrySource::Inline(text) => {
            assert_eq!(
                parse_chemistry(text).unwrap(),
                parse_chemistry(LFP_TOML).unwrap()
            );
        }
        ChemistrySource::Id(_) => unreachable!(),
    }

    // Non-monotone OCV: valid TOML, invalid chemistry.
    let broken = LFP_TOML.replace("soc   = [0.0000, 0.0025", "soc   = [0.0000, 0.0020, 0.0025");
    let bad = scenario_with_chemistry(&format!("chemistry_toml = \"\"\"{broken}\"\"\""));
    assert!(
        matches!(
            parse_scenario(&bad),
            Err(DataError::Invalid(_) | DataError::Toml(_))
        ),
        "a broken inlined chemistry must fail at scenario-parse time"
    );
}

/// TOML 1.0 has `nan` and `inf` literals, so a scenario can carry one without any
/// help from Rust. The scatter sigmas are the one place the engine would not notice:
/// `(1.0 + NaN·z).max(floor)` returns the floor, so every cell comes out pinned at
/// the minimum factor and nothing says so.
#[test]
fn non_finite_scatter_is_rejected_because_nothing_downstream_would() {
    for (capacity_sigma, r0_sigma) in [
        ("nan", "0.0"),
        ("inf", "0.0"),
        ("-0.01", "0.0"),
        ("0.0", "nan"),
    ] {
        let text = format!(
            "{}\n[pack.scatter]\ncapacity_sigma = {capacity_sigma}\nr0_sigma = {r0_sigma}\n",
            scenario_with_chemistry("chemistry = \"lfp_26650_generic\"")
        );
        assert!(
            matches!(parse_scenario(&text), Err(DataError::Scenario(_))),
            "scatter ({capacity_sigma}, {r0_sigma}) should be rejected"
        );
    }
}

/// Everything else non-finite is the *engine's* to reject, and this test pins that
/// division rather than mirroring the checks.
///
/// One condition with two error messages is how the two drift apart; `Pack::new` and
/// `Pack::schedule_fault` already own these, so `parse_scenario` accepts the file and
/// `build_pack` is where it fails. The failure is typed, so an adapter can still tell
/// a client which half went wrong.
#[test]
fn engine_owned_invalidity_survives_parsing_and_fails_at_build() {
    let chem = parse_chemistry(LFP_TOML).unwrap();

    let mut text = scenario_with_chemistry("chemistry = \"lfp_26650_generic\"");
    text = text.replace("initial_temp_k = 298.15", "initial_temp_k = nan");
    let s = parse_scenario(&text).expect("sim-data does not second-guess Pack::new");
    assert!(matches!(
        s.build_pack(chem.clone()),
        Err(DataError::Build(_))
    ));

    // Same for a fault that does not fit the pack it targets.
    let text = format!(
        "{}\n[[faults]]\nat_s = 1.0\nfault = {{ SoftInternalShort = {{ s = 9, p = 9, ohms = 5.0 }} }}\n",
        scenario_with_chemistry("chemistry = \"lfp_26650_generic\"")
    );
    let s = parse_scenario(&text).expect("an out-of-range fault index is the engine's to reject");
    assert!(matches!(s.build_pack(chem), Err(DataError::Fault(_))));
}

/// A typo beside `[pack]` is a parse error; a typo *inside* `[pack]` is not.
///
/// The asymmetry is deliberate — `deny_unknown_fields` belongs on this crate's own
/// struct, not retrofitted onto engine types that have a compatibility surface of
/// their own — and it is pinned here so that finding it later reads as a decision
/// rather than as a bug.
#[test]
fn unknown_keys_are_rejected_at_the_scenario_level_only() {
    // Top-level, beside `chemistry` — a demand-program key someone will reach for.
    let text = scenario_with_chemistry("chemistry = \"lfp_26650_generic\"\nduration_s = 3600.0");
    assert!(
        matches!(parse_scenario(&text), Err(DataError::Toml(_))),
        "a scenario is not a demand program: `duration_s` must not be silently ignored"
    );

    let inside = scenario_with_chemistry("chemistry = \"lfp_26650_generic\"")
        .replace("seed = 0", "seed = 0\ntypo_here = 1");
    assert!(
        parse_scenario(&inside).is_ok(),
        "unknown keys inside [pack] are accepted — PackConfig is an engine type"
    );
}

/// A scenario round-trips through TOML: serializing one and parsing it back gives the
/// same value. This is what lets a server hand a session's scenario back to a client.
///
/// It also pins the key-ordering hazard structurally: `chemistry` is declared before
/// every table-valued field, so the serializer cannot emit it after `[meta]` — which
/// TOML would then read back as a *different* document.
///
/// This is **not** a float-exactness claim. `Scenario`'s `PartialEq` compares `f64`
/// with `==`, and it is deterministic here only because every number in the shipped
/// examples is a short decimal literal that TOML reproduces exactly. Full-mantissa
/// values on the wire are `wire_json.rs`'s subject; a scenario example that added
/// `initial_soc = 0.8237492847362819` would be testing something this test does not
/// promise.
#[test]
fn a_scenario_round_trips_through_toml() {
    for text in [CC_DISCHARGE, SOFT_SHORT] {
        let original = parse_scenario(text).unwrap();
        let emitted = toml::to_string(&original).expect("a scenario should serialize");
        let reparsed: Scenario = parse_scenario(&emitted)
            .unwrap_or_else(|e| panic!("re-parsing emitted TOML failed: {e}\n---\n{emitted}"));
        assert_eq!(original, reparsed);
    }
}

// ---------------------------------------------------------------------------
// Every shipped scenario, without naming any of them
// ---------------------------------------------------------------------------

/// Walk `scenarios/`, and put every file through what a client puts it through.
///
/// The tests above use `include_str!`, which is compile-time and per file: each one
/// names a scenario, so a new scenario is covered by nothing until somebody remembers
/// to add it. That is the same disease the hardcoded `<option>` list had one layer up,
/// and `GET /scenarios` is only trustworthy if the directory behind it is.
///
/// Parsing is not enough to be worth much. A scenario can parse and still fail to name
/// a chemistry this repo ships, or fail to build a pack, or queue a fault at a cell that
/// does not exist — `sim-server` has a distinct error code for each of those. So this
/// resolves the chemistry the way an adapter does, builds the pack, and steps it once.
#[test]
fn every_shipped_scenario_parses_builds_and_steps() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios");
    let chem_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../chemistries");

    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .expect("the repo's scenario directory")
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    assert!(
        files.len() >= 2,
        "expected the shipped scenarios, found {files:?}"
    );

    for path in files {
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        let text = std::fs::read_to_string(&path).expect("readable");
        let scenario =
            parse_scenario(&text).unwrap_or_else(|e| panic!("{name} does not parse: {e}"));

        // Exactly `sim_server::AppState::resolve_chemistry`: an id becomes a file in the
        // chemistry directory, and the `[a-z0-9_]+` charset check `validate` has already
        // run is what makes that join safe.
        let chem = match scenario.chemistry_source() {
            ChemistrySource::Id(id) => {
                let path = std::path::Path::new(chem_dir).join(format!("{id}.toml"));
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                    panic!("{name} names chemistry {id:?}, which this repo does not ship: {e}")
                });
                parse_chemistry(&text).unwrap_or_else(|e| panic!("{name}: chemistry {id:?}: {e}"))
            }
            ChemistrySource::Inline(text) => {
                parse_chemistry(text).unwrap_or_else(|e| panic!("{name}: inlined chemistry: {e}"))
            }
        };

        let mut pack = scenario
            .build_pack(chem)
            .unwrap_or_else(|e| panic!("{name} parses but builds no pack: {e}"));
        let telemetry = pack.step(1.0, Demand::Rest, &env());
        assert!(
            telemetry.v_terminal.is_finite() && telemetry.soc_true.is_finite(),
            "{name}: a resting first step produced {telemetry:?}"
        );
    }
}
