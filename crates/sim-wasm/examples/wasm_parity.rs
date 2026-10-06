//! The native half of the browser-versus-native parity check. The other half, and the
//! only way to run it, is `node tools/wasm-parity/parity.mjs`, which builds and runs this
//! example and then replays its demands through the committed `web/pkg`.
//!
//! `sim-core` computes every transcendental through `libm`, so the native build and the
//! wasm build compile the same source for them and must agree **bit for bit**
//! (`docs/plans/cross-platform-math.md`). `cargo test` cannot see the wasm build at all,
//! so this is what checks that promise against the package the page loads.
//!
//! Every shipped scenario, through the same [`SimEngine`] the wasm `Sim` wraps, under:
//! a 1 C discharge, a rest and a C/2 charge; on every model but the `Dfn`, a power draw
//! and a voltage hold; and a year of hour-long steps on `calendar_fade_hot`. Every call's
//! result — the frames JSON or the snapshot JSON — is written out with the scenario and
//! chemistry text and the demands that produced it, as one JSON file at the path given.
//! Frames are thinned to about fifty a call so the file stays small; the snapshot after
//! every phase carries the whole state, and a difference anywhere reaches it.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use sim_core::CellModelConfig;
use sim_data::{parse_chemistry, parse_scenario, ChemistrySource};
use sim_wasm::engine::SimEngine;

/// A call to step the engine: `n` steps of `dt` seconds under `demand`.
fn step(dt: f64, n: u32, demand: Value) -> Value {
    json!({ "op": "step", "dt": dt, "n": n, "demand": demand, "every": (n / 50).max(1) })
}

fn snapshot() -> Value {
    json!({ "op": "snapshot" })
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: wasm_parity <output.json>");
    let root = repo_root();
    let mut names: Vec<_> = std::fs::read_dir(root.join("scenarios"))
        .expect("scenarios/")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".toml"))
        .collect();
    names.sort();

    let mut runs = Vec::new();
    for file in &names {
        let scenario_toml = std::fs::read_to_string(root.join("scenarios").join(file)).unwrap();
        let scenario = parse_scenario(&scenario_toml).unwrap();
        let chemistry_toml = match scenario.chemistry_source() {
            ChemistrySource::Inline(text) => text.to_owned(),
            ChemistrySource::Id(id) => {
                std::fs::read_to_string(root.join("chemistries").join(format!("{id}.toml")))
                    .unwrap()
            }
        };
        let chem = parse_chemistry(&chemistry_toml).unwrap();
        let pack = &scenario.pack;
        let one_c = chem.cell.capacity_ah * f64::from(pack.parallel);
        let series = f64::from(pack.series);
        let (porous, dfn) = match pack.cell_model {
            CellModelConfig::Ecm => (false, false),
            CellModelConfig::Spm { .. } => (true, false),
            CellModelConfig::Dfn { .. } => (true, true),
        };
        // The porous models take a longer step only to keep the check quick.
        let dt = if porous { 2.0 } else { 1.0 };
        let steps = |seconds: f64| (seconds / dt) as u32;
        let stem = file.trim_end_matches(".toml");

        let mut arms = vec![(
            format!("{stem}__cycle"),
            vec![
                step(dt, steps(3000.0), json!({ "Current": one_c })),
                snapshot(),
                step(dt, steps(600.0), json!("Rest")),
                snapshot(),
                step(dt, steps(5400.0), json!({ "Current": -0.5 * one_c })),
                snapshot(),
            ],
        )];
        // The power and voltage solves are paths a current demand never takes. The `Dfn`
        // is left out of them only for time.
        if !dfn {
            // Read off the chemistry's own limits, so every chemistry gets a demand on its
            // own scale: a half-C draw at mid-window voltage, then a hold just under the top.
            let v_cell = 0.5 * (chem.cell.v_min + chem.cell.v_max);
            let v_hold = chem.cell.v_max - 0.05;
            arms.push((
                format!("{stem}__power_volt"),
                vec![
                    step(
                        dt,
                        steps(1800.0),
                        json!({ "Power": 0.5 * one_c * v_cell * series }),
                    ),
                    snapshot(),
                    step(dt, steps(1800.0), json!({ "Voltage": v_hold * series })),
                    snapshot(),
                ],
            ));
        }
        if stem == "calendar_fade_hot" {
            arms.push((
                format!("{stem}__fastforward"),
                vec![
                    step(3600.0, 24 * 365, json!("Rest")),
                    snapshot(),
                    step(3600.0, 2000, json!({ "Current": 0.2 * one_c })),
                    snapshot(),
                ],
            ));
        }

        for (name, ops) in arms {
            let mut engine = SimEngine::new(&scenario_toml, Some(&chemistry_toml)).unwrap();
            let lines: Vec<String> = ops
                .iter()
                .map(|op| {
                    let line = if op["op"] == "snapshot" {
                        engine.snapshot_json().map_err(|e| e.to_string())
                    } else {
                        engine
                            .step_many_json(
                                op["dt"].as_f64().unwrap(),
                                u32::try_from(op["n"].as_u64().unwrap()).unwrap(),
                                &op["demand"].to_string(),
                                u32::try_from(op["every"].as_u64().unwrap()).unwrap(),
                            )
                            .map_err(|e| e.to_string())
                    };
                    line.unwrap_or_else(|e| format!("ERR: {e}"))
                })
                .collect();
            runs.push(json!({
                "name": name,
                "scenario_toml": scenario_toml,
                "chemistry_toml": chemistry_toml,
                "ops": ops,
                "lines": lines,
            }));
        }
    }
    let n = runs.len();
    std::fs::write(
        &out,
        serde_json::to_string(&json!({ "runs": runs })).unwrap(),
    )
    .unwrap();
    eprintln!("wasm_parity: {n} native runs written to {out}");
}
