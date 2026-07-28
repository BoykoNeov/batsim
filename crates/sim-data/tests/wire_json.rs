//! The JSON wire contract: shapes an adapter's client has to parse, and the float
//! exactness the whole transport rests on.
//!
//! These live in `sim-data` rather than `sim-core` for one reason: the values under
//! test have to be *real*. A `Telemetry` assembled from hand-written literals
//! (`3.3`, `298.15`, `0.5`) round-trips through `serde_json` with or without the
//! `float_roundtrip` feature, because the defect is one ULP on full-mantissa values.
//! Getting those means stepping an actual pack on an actual chemistry, and reading a
//! chemistry off disk is exactly what this crate is for.
//!
//! `sim-core` still declares no serialization format of its own — the adapter
//! chooses, and here the adapter's choice is being tested.

use serde_json::json;
use sim_core::{
    CellModelConfig, CellView, Demand, Env, EventFlags, Pack, PackConfig, Scatter, Telemetry,
    ThermalConfig,
};
use sim_data::parse_chemistry;

const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

/// A 4S2P LFP pack with scatter on, stepped far enough that nothing in its telemetry
/// is a round number: the plan's own probe configuration.
///
/// Scatter matters here — it is what makes the per-cell resistances irrational-looking
/// and therefore what fills the mantissas that a lossy float parser mangles.
fn stepped_pack(steps: usize) -> Pack {
    let chem = parse_chemistry(LFP_TOML).expect("shipped LFP chemistry");
    let config = PackConfig {
        series: 4,
        parallel: 2,
        initial_soc: 0.8,
        initial_temp_k: 298.15,
        seed: 0x0BA7_7E47,
        scatter: Scatter {
            capacity_sigma: 0.02,
            r0_sigma: 0.03,
        },
        thermal: ThermalConfig::Network {
            k_neighbor_w_per_k: 0.5,
        },
        bms: None,
        aging: None,
        cell_model: CellModelConfig::Ecm,
    };
    let mut pack = Pack::new(&config, chem).expect("4S2P LFP pack");
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    for _ in 0..steps {
        pack.step(0.37, Demand::Current(3.1), &env);
    }
    pack
}

/// Every `f64` a `Telemetry` carries, as raw bits. `to_bits`, not `==`: `-0.0 == 0.0`
/// and `NaN != NaN`, so `==` can both hide a real difference and invent one.
fn telemetry_bits(t: &Telemetry) -> Vec<u64> {
    let mut bits = vec![
        t.v_terminal.to_bits(),
        t.i_actual.to_bits(),
        t.soc_true.to_bits(),
        t.t_min.to_bits(),
        t.t_max.to_bits(),
        t.v_cell_min.to_bits(),
        t.v_cell_max.to_bits(),
        t.soh_capacity.to_bits(),
        t.soh_resistance.to_bits(),
        t.q_gen_w.to_bits(),
        t.q_runaway_w.to_bits(),
        t.q_balancing_w.to_bits(),
        t.i_balancing_a.to_bits(),
        t.i_internal_short_a.to_bits(),
        t.i_external_short_a.to_bits(),
    ];
    // `Option<f64>` too: the `None`/`Some` distinction plus the payload's bits.
    match t.soc_bms {
        None => bits.push(u64::MAX),
        Some(v) => bits.push(v.to_bits()),
    }
    bits
}

fn cell_view_bits(c: &CellView) -> Vec<u64> {
    vec![
        c.soc.to_bits(),
        c.temp_k.to_bits(),
        c.overpotential_v.to_bits(),
        c.capacity_factor.to_bits(),
        c.r0_factor.to_bits(),
        c.soh_capacity.to_bits(),
        c.soh_resistance.to_bits(),
        c.internal_short_conductance_s.to_bits(),
        c.runaway_energy_remaining_j.to_bits(),
    ]
}

/// The load-bearing one: a stepped pack's telemetry survives JSON **bit for bit**.
///
/// Without `serde_json`'s `float_roundtrip` feature this fails. The default parser's
/// fast path is not correctly rounded and can return a value one ULP off the one ryu
/// wrote — silently, rarely, and only on values with full mantissas, which is why the
/// numbers here come from a stepped pack rather than from literals. The exit gate for
/// this phase compares an in-process run against a run driven over a socket; a
/// one-ULP transport is exactly the failure it would otherwise report as a physics
/// bug.
#[test]
fn telemetry_survives_json_bit_for_bit() {
    let mut pack = stepped_pack(20);
    let env = Env {
        t_ambient: 298.15,
        t_coolant: Some(295.65),
    };

    // Several steps, not one: the defect is data-dependent, so one sample is a coin
    // flip and twenty is a test.
    for step in 0..20 {
        let tele = pack.step(0.37, Demand::Power(9.7), &env);
        let text = serde_json::to_string(&tele).expect("telemetry serializes");
        let back: Telemetry = serde_json::from_str(&text).expect("telemetry parses");

        assert_eq!(
            telemetry_bits(&tele),
            telemetry_bits(&back),
            "step {step}: telemetry changed across JSON\n  out: {text}\n  in:  {back:?}"
        );
        assert_eq!(tele.flags, back.flags, "step {step}: flags changed");

        // The re-serialized *text* is byte-stable too. Without the feature it was
        // observed not to be, which is the cheapest possible canary: a diff of two
        // strings, no float comparison in sight.
        assert_eq!(
            text,
            serde_json::to_string(&back).unwrap(),
            "step {step}: JSON text is not a fixed point"
        );
    }
}

/// Ground-truth per-cell views cross the wire intact for the same reason — this is
/// the array the browser's pedagogy view reads.
#[test]
fn cell_views_survive_json_bit_for_bit() {
    let pack = stepped_pack(50);
    for s in 0..4 {
        for p in 0..2 {
            let view = pack.cell(s, p).unwrap();
            let text = serde_json::to_string(&view).unwrap();
            let back: CellView = serde_json::from_str(&text).unwrap();
            assert_eq!(
                cell_view_bits(&view),
                cell_view_bits(&back),
                "cell {s}S{p}P changed across JSON: {text}"
            );
            assert_eq!(view.vented, back.vented);
        }
    }
}

/// The enum encodings a JS client has to write by hand, pinned as literal JSON.
///
/// Serde-default (externally tagged), matching how the engine's other enums already
/// serialize in snapshots and scenarios. Consistency with the existing encoding beats
/// a JS-friendlier adjacent tagging that would apply to two of five enums and make
/// the rest look arbitrary — the demo page gets a three-line helper instead.
#[test]
fn demand_and_env_have_the_documented_json_shapes() {
    for (demand, expected) in [
        (Demand::Current(-5.0), json!({ "Current": -5.0 })),
        (Demand::Power(9.5), json!({ "Power": 9.5 })),
        (Demand::Voltage(3.65), json!({ "Voltage": 3.65 })),
        (Demand::Rest, json!("Rest")),
    ] {
        assert_eq!(serde_json::to_value(demand).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<Demand>(expected).unwrap(),
            demand,
            "the shape a client sends must parse back to what it meant"
        );
    }

    // `t_coolant: None` is `null`, not an absent key — a client that omits it gets a
    // parse error rather than a silently ambient-cooled pack.
    let passive = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    assert_eq!(
        serde_json::to_value(passive).unwrap(),
        json!({ "t_ambient": 298.15, "t_coolant": null })
    );
    let cooled = Env {
        t_ambient: 313.15,
        t_coolant: Some(293.15),
    };
    assert_eq!(
        serde_json::to_value(cooled).unwrap(),
        json!({ "t_ambient": 313.15, "t_coolant": 293.15 })
    );
    assert_eq!(
        serde_json::from_value::<Env>(json!({ "t_ambient": 313.15, "t_coolant": 293.15 })).unwrap(),
        cooled
    );
}

/// `EventFlags` crosses as a `" | "`-joined name string, and the empty set is `""`.
///
/// The empty case is the one a naive client breaks on: splitting `""` on `" | "`
/// yields `[""]`, one element, which reads as a flag named `""` rather than as no
/// flags. It is a two-minute bug and it belongs in a test rather than in someone's
/// afternoon.
#[test]
fn event_flags_cross_as_a_joined_name_string() {
    assert_eq!(
        serde_json::to_value(EventFlags::empty()).unwrap(),
        json!(""),
        "no flags is the empty string, not 0 and not an empty array"
    );
    assert_eq!(
        serde_json::to_value(EventFlags::OV | EventFlags::UV | EventFlags::THERMAL_RUNAWAY)
            .unwrap(),
        json!("OV | UV | THERMAL_RUNAWAY")
    );

    for flags in [
        EventFlags::empty(),
        EventFlags::OV,
        EventFlags::OV | EventFlags::PLATING_RISK,
        EventFlags::all(),
    ] {
        let text = serde_json::to_string(&flags).unwrap();
        let back: EventFlags = serde_json::from_str(&text).unwrap();
        assert_eq!(flags, back, "flags {flags:?} did not survive {text}");
    }
}

/// Every `CellView` field's spelling on the wire, pinned to its Rust name.
///
/// This is the mirror of `sim-server`'s `error_codes_have_pinned_wire_spellings`, and
/// it exists because the same rule was aspirational here. `sim_server::API_VERSION`'s
/// doc says a renamed field on `CellView` bumps it — but nothing checked. The bit
/// comparisons above read the *Rust struct* (`cell_view_bits`), and `rest.rs` only
/// indexes `cells["cells"]`, so renaming a field flowed through the entire suite
/// green. Slice C1 renamed one (`v_rc_sum` -> `overpotential_v`) and had to bump two
/// API versions by hand; the next such rename fails here first.
///
/// It asserts each key is **present**, not that the key set is exact, because that is
/// what the rule actually says: *"Adding a field or an error code does not bump it."*
/// An exact-set assertion would fail on a field addition that is explicitly allowed —
/// it would be a stricter test of the wrong thing.
///
/// Deliberately not covered, and stated rather than left to be discovered: `Telemetry`,
/// `Demand` and `Env` are named by the same rule and are still unpinned. This slice
/// renamed a `CellView` field, so `CellView` is what it owes.
#[test]
fn cell_view_fields_have_pinned_wire_spellings() {
    let pack = stepped_pack(20);
    let view = pack.cell(0, 0).expect("a 4S2P pack has a cell 0S0P");
    let json = serde_json::to_value(view).expect("CellView serializes");
    let object = json.as_object().expect("CellView crosses as a JSON object");

    for key in [
        "soc",
        "temp_k",
        "overpotential_v",
        "capacity_factor",
        "r0_factor",
        "soh_capacity",
        "soh_resistance",
        "internal_short_conductance_s",
        "runaway_energy_remaining_j",
        "vented",
    ] {
        assert!(
            object.contains_key(key),
            "CellView lost or renamed the wire field `{key}` — that breaks every \
             client parsing it, so it bumps sim_server::API_VERSION and \
             sim_wasm::WASM_API_VERSION. Present keys: {:?}",
            object.keys().collect::<Vec<_>>()
        );
    }

    assert!(
        !object.contains_key("v_rc_sum"),
        "`v_rc_sum` is the pre-C1 name for `overpotential_v` and must not come back: \
         it named an ECM internal that a porous-electrode cell does not have"
    );
}

/// Every `Telemetry` field's spelling on the wire, pinned to its Rust name.
///
/// The sibling of the `CellView` pin above, and it exists for a sharper reason than
/// symmetry. `sim_server::API_VERSION`'s rule names four engine types — `Telemetry`,
/// `CellView`, `Demand`, `Env` — and slice C1 pinned only the one it renamed. That left
/// `Telemetry` the *asymmetric* risk: it is the type the next slices add fields to (an
/// SPM cell's readouts), and every existing check reads it through the Rust struct
/// (`tele_bits` here, `cell_bits` in `thevenin_cache.rs`), so a rename during that work
/// would go out under an unchanged API version with a green suite.
///
/// Same presence-not-exact-set rule as the `CellView` pin, and for the same reason:
/// *"Adding a field or an error code does not bump it."* A new field is expected and
/// must not fail here; a renamed one must.
#[test]
fn telemetry_fields_have_pinned_wire_spellings() {
    let mut pack = stepped_pack(20);
    let env = Env {
        t_ambient: 298.15,
        t_coolant: None,
    };
    let telemetry = pack.step(0.37, Demand::Current(3.1), &env);
    let json = serde_json::to_value(telemetry).expect("Telemetry serializes");
    let object = json
        .as_object()
        .expect("Telemetry crosses as a JSON object");

    for key in [
        "v_terminal",
        "i_actual",
        "soc_true",
        "soc_bms",
        "t_min",
        "t_max",
        "v_cell_min",
        "v_cell_max",
        "soh_capacity",
        "soh_resistance",
        "q_gen_w",
        "q_runaway_w",
        "q_balancing_w",
        "i_balancing_a",
        "i_internal_short_a",
        "i_external_short_a",
        "flags",
    ] {
        assert!(
            object.contains_key(key),
            "Telemetry lost or renamed the wire field `{key}` — that breaks every client \
             parsing it, so it bumps sim_server::API_VERSION and \
             sim_wasm::WASM_API_VERSION. Present keys: {:?}",
            object.keys().collect::<Vec<_>>()
        );
    }
}
