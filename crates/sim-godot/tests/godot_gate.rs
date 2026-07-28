//! **Phase 5's exit gate.**
//!
//! > A scenario driven through the `BatteryPack` node inside a running Godot process
//! > produces a **bit-identical** trajectory to the same scenario driven by `Pack::step`
//! > in process.
//!
//! # Why this is `#[ignore]`d rather than absent
//! It needs two things `cargo test --workspace` has no business requiring: a built cdylib
//! and a Godot 4.7 binary. That is the same carve-out the root `Cargo.toml` already makes
//! for `wasm-pack build`. Being `#[ignore]`d rather than `#[cfg]`d-out means it is still
//! **compiled** by the default gate, so it cannot rot while nobody is looking.
//!
//! ```text
//! cargo test -p sim-godot -- --ignored
//! ```
//!
//! # Why it is a Rust test and not a GDScript one
//! Measured in the spike: a failing GDScript `assert()` abandons the enclosing function
//! without reaching `quit()`, and a headless `SceneTree` then runs forever. In an
//! unattended gate that is a stall, not a failure. So GDScript's job shrinks to "print
//! numbers and quit", and everything that can fail an assertion lives here — under a
//! timeout, because rule 1 cannot cover a runtime error in code not yet written.
//!
//! # Why the comparison is on bits
//! `str(0.7995885912375074)` in GDScript gives `0.79958859123751`, which does not parse
//! back equal. A decimal-text handoff would make this gate pass on values that differ.
//! Samples cross as the little-endian bytes of the f64s, hex-encoded, and are compared
//! with `to_bits` — so `-0.0` and `NaN` cannot launder a difference either.
//!
//! # Why both legs share one construction path
//! The in-process leg calls `PackDriver::new`, exactly as the node does. If it hand-built
//! a `PackConfig` instead, scatter seeding could diverge and this would fail for a reason
//! that has nothing to do with the boundary it exists to test. The `rlib` in
//! `Cargo.toml`'s `crate-type` is what makes that possible.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use sim_godot::driver::PackDriver;

/// How long the Godot leg may take before it is treated as hung.
const GODOT_TIMEOUT: Duration = Duration::from_secs(120);

/// One leg of the experiment. Serialized to JSON for the Godot leg, so there is exactly
/// one definition of the schedule rather than two that can drift.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Leg {
    Step {
        dt: f64,
        n: u32,
        /// Externally-tagged demand JSON, handed to the node verbatim.
        demand: String,
    },
    SnapshotRestore,
}

/// The experiment.
///
/// Every `dt` and every demand magnitude here is **exactly representable in binary**
/// (halves, quarters, small integers). That is deliberate: the schedule crosses to the
/// Godot leg as JSON, and a value like `0.1` would be a place for the two legs to disagree
/// for a reason unrelated to what is being tested.
fn schedule() -> Vec<Leg> {
    vec![
        // Discharge.
        Leg::Step {
            dt: 0.5,
            n: 60,
            demand: r#"{"Current": 2.0}"#.into(),
        },
        // Rest — relaxation, which is where the RC pairs show themselves.
        Leg::Step {
            dt: 0.5,
            n: 20,
            demand: r#""Rest""#.into(),
        },
        // The whole engine state through a GDScript String and back, mid-experiment.
        Leg::SnapshotRestore,
        // Charge. Negative current, per the sign convention.
        Leg::Step {
            dt: 0.25,
            n: 80,
            demand: r#"{"Current": -1.0}"#.into(),
        },
        // Constant power, which runs the Newton solve rather than the closed form.
        Leg::Step {
            dt: 0.5,
            n: 40,
            demand: r#"{"Power": 4.0}"#.into(),
        },
        // A second round trip, after the trajectory has been perturbed by both signs of
        // current — the state that would expose a lossy encode is richer here.
        Leg::SnapshotRestore,
        Leg::Step {
            dt: 0.125,
            n: 100,
            demand: r#"{"Current": 3.0}"#.into(),
        },
    ]
}

/// One reported sample. Field order is fixed and **must match `godot/gate.gd`'s `_emit`**.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sample {
    flags_bits: u32,
    values: [f64; 7],
}

/// Names for the seven values, so a mismatch report names the field rather than an index.
const FIELDS: [&str; 7] = [
    "sim_time_s",
    "v_terminal",
    "i_actual",
    "soc_true",
    "t_min",
    "t_max",
    "soh_capacity",
];

impl Sample {
    fn of(driver: &PackDriver) -> Self {
        let t = driver.latest();
        Self {
            flags_bits: t.flags.bits(),
            values: [
                driver.sim_time_s(),
                t.v_terminal,
                t.i_actual,
                t.soc_true,
                t.t_min,
                t.t_max,
                t.soh_capacity,
            ],
        }
    }
}

/// Decode `SAMPLE <flags> <hex>` as the Godot leg writes it.
///
/// The hex is `PackedFloat64Array::to_byte_array().hex_encode()` — little-endian IEEE-754
/// bytes, eight per value. Parsing it as one big-endian integer would be a silent
/// byte-order bug, which is why `hex_is_little_endian_ieee754` pins this separately.
fn parse_sample(line: &str) -> Result<Sample, String> {
    let rest = line
        .strip_prefix("SAMPLE ")
        .ok_or_else(|| format!("not a sample line: {line:?}"))?;
    let (flags, hex) = rest
        .split_once(' ')
        .ok_or_else(|| format!("malformed sample: {line:?}"))?;
    let flags_bits: u32 = flags
        .trim()
        .parse()
        .map_err(|e| format!("bad flags {flags:?}: {e}"))?;

    let hex = hex.trim();
    let expected = FIELDS.len() * 16;
    if hex.len() != expected {
        return Err(format!(
            "expected {expected} hex chars for {} values, got {}",
            FIELDS.len(),
            hex.len()
        ));
    }
    let mut values = [0.0; 7];
    for (i, value) in values.iter_mut().enumerate() {
        let chunk = &hex[i * 16..(i + 1) * 16];
        let mut bytes = [0u8; 8];
        for (b, slot) in bytes.iter_mut().enumerate() {
            *slot = u8::from_str_radix(&chunk[b * 2..b * 2 + 2], 16)
                .map_err(|e| format!("bad hex {chunk:?}: {e}"))?;
        }
        *value = f64::from_le_bytes(bytes);
    }
    Ok(Sample { flags_bits, values })
}

/// Run the whole schedule in process, collecting a sample per leg plus the priming one.
fn in_process(root: &Path, scenario_name: &str) -> Vec<Sample> {
    let scenario =
        std::fs::read_to_string(root.join("scenarios").join(scenario_name)).expect("scenario file");
    let chem = PackDriver::chemistry_id_of(&scenario)
        .expect("scenario parses")
        .map(|id| {
            std::fs::read_to_string(root.join("chemistries").join(format!("{id}.toml")))
                .expect("chemistry file")
        });
    let mut driver = PackDriver::new(&scenario, chem.as_deref()).expect("driver builds");

    let mut samples = vec![Sample::of(&driver)];
    for leg in schedule() {
        match leg {
            Leg::Step { dt, n, demand } => {
                let demand = PackDriver::demand_from_json(&demand).expect("demand parses");
                driver.step_batch(dt, n, demand).expect("step_batch");
            }
            Leg::SnapshotRestore => {
                let snapshot = driver.snapshot_json().expect("snapshot");
                driver.restore_json(&snapshot).expect("restore");
            }
        }
        samples.push(Sample::of(&driver));
    }
    samples
}

/// Repo root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/sim-godot has two ancestors")
        .to_path_buf()
}

/// Make sure Godot has scanned the extension at least once.
///
/// # There is deliberately no `cargo build` here
/// An earlier version ran `cargo build -p sim-godot` from inside the test. Two things
/// were wrong with that. It is a surprising side effect for a test to have, and it
/// contends for cargo's artifact-directory lock with the very `cargo test` that spawned
/// it — the run visibly blocked on "waiting for file lock on artifact directory".
///
/// It is also unnecessary. This crate's `crate-type` is `["cdylib", "rlib"]`, so
/// `cargo test -p sim-godot` already builds the cdylib as part of building this test's
/// dependencies. The artifact is there by the time this runs; if it somehow is not, say so
/// rather than trying to fix it.
///
/// The `--import` bootstrap *is* needed, exactly once per clone, because `.godot/` is
/// gitignored: without it Godot reports `Identifier "BatteryPack" not declared`. A
/// rebuilt cdylib needs no re-import, so this is skipped when the cache exists.
fn ensure_importable(root: &Path) -> Result<(), String> {
    let cdylib = root.join("target").join("debug").join(CDYLIB);
    if !cdylib.exists() {
        return Err(format!(
            "{} does not exist; run `cargo build -p sim-godot` first",
            cdylib.display()
        ));
    }

    if root.join("godot").join(".godot").exists() {
        return Ok(());
    }
    let imported = Command::new("godot")
        .args(["--headless", "--path", "godot", "--import"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("could not run godot (is it on PATH?): {e}"))?;
    if !imported.success() {
        return Err("godot --import failed".into());
    }
    Ok(())
}

/// The cdylib's platform-specific file name, matching `godot/batsim.gdextension`.
#[cfg(target_os = "windows")]
const CDYLIB: &str = "sim_godot.dll";
#[cfg(target_os = "macos")]
const CDYLIB: &str = "libsim_godot.dylib";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const CDYLIB: &str = "libsim_godot.so";

/// Drive the Godot leg and collect its samples.
fn in_godot(root: &Path, scenario_name: &str) -> Result<Vec<Sample>, String> {
    let schedule_path = std::env::temp_dir().join(format!("batsim-gate-{scenario_name}.json"));
    std::fs::write(
        &schedule_path,
        serde_json::to_string(&schedule()).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("could not write the schedule: {e}"))?;

    // Output goes to **files, not pipes**, and this is the fix for a real hang rather
    // than a style preference. Polling `try_wait` in a loop while never draining a piped
    // stdout deadlocks as soon as the child fills the pipe buffer: the child blocks
    // writing, so it never exits, so the poll never sees it exit. The symptom is
    // indistinguishable from the GDScript hang this timeout was written for — which is
    // exactly how it presented, and why it is worth a comment this long. Files have no
    // buffer to fill.
    let out_path = std::env::temp_dir().join(format!("batsim-gate-{scenario_name}.out"));
    let err_path = std::env::temp_dir().join(format!("batsim-gate-{scenario_name}.err"));
    let out_file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let err_file = std::fs::File::create(&err_path).map_err(|e| e.to_string())?;

    let mut child = Command::new("godot")
        .args(["--headless", "--path", "godot", "--script", "gate.gd", "--"])
        .arg(root)
        .arg(scenario_name)
        .arg(&schedule_path)
        .current_dir(root)
        .stdout(out_file)
        .stderr(err_file)
        .spawn()
        .map_err(|e| format!("could not run godot (is it on PATH?): {e}"))?;

    // The timeout still earns its place. A runtime error in GDScript abandons
    // `_initialize` without reaching `quit()`, and the headless SceneTree then runs
    // forever — measured at three minutes before it was killed.
    let deadline = Instant::now() + GODOT_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                // Deliberately does **not** name a cause. An earlier version confidently
                // blamed a GDScript runtime error — a real failure mode, measured, and
                // wrong that time: the hang was this harness deadlocking on an undrained
                // pipe. A message that asserts a cause it cannot distinguish sends the
                // next reader to the wrong file. Known candidates are listed as
                // candidates.
                return Err(format!(
                    "the Godot leg did not exit within {GODOT_TIMEOUT:?}. Candidates, in no \
                     particular order: a GDScript runtime error or failed `assert()` (either \
                     abandons `_initialize` without reaching `quit()`, and a headless \
                     SceneTree then runs forever); this harness blocking the child on an \
                     un-drained stdio handle; or the schedule genuinely taking that long. \
                     Reproduce by hand with:\n  \
                     godot --headless --path godot --script gate.gd -- <root> {scenario_name} \
                     <schedule.json>\nPartial output:\n{}",
                    std::fs::read_to_string(&out_path).unwrap_or_default()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let stdout = std::fs::read_to_string(&out_path).unwrap_or_default();

    if !status.success() {
        return Err(format!(
            "the Godot leg exited with {status}\n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
            std::fs::read_to_string(&err_path).unwrap_or_default()
        ));
    }
    if !stdout.contains("GATE DONE") {
        return Err(format!(
            "the Godot leg exited 0 without finishing the schedule\n--- stdout ---\n{stdout}"
        ));
    }

    stdout
        .lines()
        .filter(|line| line.starts_with("SAMPLE "))
        .map(parse_sample)
        .collect()
}

/// Compare two runs sample by sample, field by field, on bits.
fn compare(rust: &[Sample], godot: &[Sample]) -> Result<(), String> {
    if rust.len() != godot.len() {
        return Err(format!(
            "sample counts differ: in-process produced {}, Godot produced {}",
            rust.len(),
            godot.len()
        ));
    }
    if rust.is_empty() {
        return Err("no samples at all — the gate would have passed vacuously".into());
    }

    for (i, (a, b)) in rust.iter().zip(godot).enumerate() {
        if a.flags_bits != b.flags_bits {
            return Err(format!(
                "sample {i}: flags differ — in-process {:#014b}, Godot {:#014b}",
                a.flags_bits, b.flags_bits
            ));
        }
        for (f, name) in FIELDS.iter().enumerate() {
            let (x, y) = (a.values[f], b.values[f]);
            if x.to_bits() != y.to_bits() {
                return Err(format!(
                    "sample {i}, field {name}: in-process {x:.17} ({:#018x}) != \
                     Godot {y:.17} ({:#018x})",
                    x.to_bits(),
                    y.to_bits()
                ));
            }
        }
    }
    Ok(())
}

/// **The exit criterion**, on the simplest scenario the repo ships.
#[test]
#[ignore = "needs a built cdylib and a Godot 4.7 binary on PATH; run with --ignored"]
fn the_node_and_the_engine_agree_bit_for_bit() {
    let root = repo_root();
    ensure_importable(&root).expect("the extension is importable");
    let godot = in_godot(&root, "cc_discharge_lfp.toml").expect("the Godot leg");
    let rust = in_process(&root, "cc_discharge_lfp.toml");
    compare(&rust, &godot).expect("the two legs agree");
    assert_eq!(rust.len(), schedule().len() + 1, "a leg went unreported");
}

/// The same claim on a scenario with a BMS, injected faults, a lying sensor and a live
/// RNG — so the gate covers a trajectory where the *engine* has state beyond the cells,
/// and where a restore that lost the RNG's position would show up.
#[test]
#[ignore = "needs a built cdylib and a Godot 4.7 binary on PATH; run with --ignored"]
fn the_node_and_the_engine_agree_on_a_scenario_with_faults_and_an_rng() {
    let root = repo_root();
    ensure_importable(&root).expect("the extension is importable");
    let scenario = "soft_short_under_a_lying_sensor.toml";
    let godot = in_godot(&root, scenario).expect("the Godot leg");
    let rust = in_process(&root, scenario);
    compare(&rust, &godot).expect("the two legs agree");
}

/// The gate is only worth anything if it can fail. Perturbing one field of one sample by
/// **one ULP** must be reported, and must name the field and the sample.
///
/// This runs in the default gate — it needs no Godot — so the comparison logic cannot rot
/// even where the gate itself is skipped.
#[test]
fn the_comparison_reports_a_one_ulp_difference_and_names_it() {
    let a = Sample {
        flags_bits: 0,
        values: [
            1.0,
            3.279_041_277_618_07,
            2.0,
            0.975_881_598_716_21,
            298.15,
            298.15,
            1.0,
        ],
    };
    let mut b = a;
    // One ULP up on `v_terminal`, which is index 1.
    b.values[1] = f64::from_bits(a.values[1].to_bits() + 1);
    assert_ne!(a.values[1], b.values[1], "the perturbation was a no-op");

    let error = compare(&[a], &[b]).expect_err("a one-ULP difference was accepted");
    assert!(error.contains("v_terminal"), "{error}");
    assert!(error.contains("sample 0"), "{error}");

    // And the unperturbed pair passes, so the test above is not passing for free.
    compare(&[a], &[a]).expect("identical samples compared unequal");
}

/// A comparison that silently accepts nothing would let the gate pass while the Godot leg
/// printed no samples at all — the failure mode a `for` loop over an empty vector has.
#[test]
fn the_comparison_refuses_to_pass_vacuously() {
    assert!(compare(&[], &[]).is_err(), "an empty comparison passed");
}

/// Pins the interchange's byte order.
///
/// `PackedFloat64Array::to_byte_array().hex_encode()` writes **little-endian** IEEE-754
/// bytes. If that ever changed, or if `parse_sample` were rewritten to read the hex as one
/// big-endian integer, every value would decode to garbage — and a gate comparing garbage
/// to garbage could still pass. The literal below was produced by Godot 4.7 from the value
/// beside it, in the spike.
#[test]
fn hex_is_little_endian_ieee754() {
    let sample = parse_sample(&format!(
        "SAMPLE 0 {}",
        "74d533d03a96e93f".repeat(FIELDS.len())
    ))
    .expect("parses");
    for value in sample.values {
        assert_eq!(
            value, 0.7995885912375074,
            "byte order changed: Godot's hex for 0.7995885912375074 decoded as {value:.17}"
        );
    }
}

/// The parser must reject a malformed line rather than silently producing a short sample
/// that `compare` would then be unable to notice.
#[test]
fn the_parser_rejects_malformed_lines() {
    assert!(parse_sample("nonsense").is_err());
    assert!(parse_sample("SAMPLE 0").is_err());
    assert!(parse_sample("SAMPLE x 74d533d03a96e93f").is_err());
    // Right shape, too few values.
    assert!(parse_sample("SAMPLE 0 74d533d03a96e93f").is_err());
}
