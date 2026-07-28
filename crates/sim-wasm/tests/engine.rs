//! What the browser boundary is actually on the hook for.
//!
//! These run on the **host** target, which is the whole reason [`SimEngine`] exists
//! separately from `sim_wasm::Sim`: on the host a `JsError` is a stub, so a test that
//! went through the `#[wasm_bindgen]` façade would be testing the stub. Everything the
//! façade does is `?`, so everything worth asserting is here.
//!
//! Chemistry and scenario text arrive by `include_str!`, the same way every other test
//! in this repo gets them — and it is not a cheat for a crate with no filesystem. In
//! production JS fetches those exact bytes and hands them across as strings; embedding
//! them is the closest a host test gets to that, and it keeps the parameters
//! single-sourced against `chemistries/` rather than inlined and free to drift.

use sim_core::{Demand, EventFlags, Telemetry};
use sim_wasm::engine::{Frame, SimEngine, MAX_FRAMES_PER_CALL, MAX_STEPS_PER_CALL};
use sim_wasm::EngineError;

const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");
const CC_DISCHARGE: &str = include_str!("../../../scenarios/cc_discharge_lfp.toml");
const SOFT_SHORT: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");

/// The 4S2P scattered pack, which is the one whose numbers fill their mantissas.
fn scattered() -> SimEngine {
    SimEngine::new(SOFT_SHORT, Some(LFP_TOML)).expect("the shipped scenario builds")
}

/// Every `f64` a [`Frame`] carries, as raw bits.
///
/// `to_bits`, not `==`: `-0.0 == 0.0` and `NaN != NaN`, so `==` can both hide a real
/// difference and invent one. Same shape as `sim-server`'s `frame_bits`, because the
/// two crates are asserting the same kind of claim about the same engine.
fn frame_bits(frame: &Frame) -> Vec<u64> {
    let t: &Telemetry = &frame.telemetry;
    let mut bits = vec![
        frame.sim_time_s.to_bits(),
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
    // `u64::MAX` stands in for `None` so a `Some`/`None` change cannot alias a value
    // change.
    bits.push(t.soc_bms.map_or(u64::MAX, f64::to_bits));
    bits
}

/// The longest run of digits anywhere in the text.
///
/// A crude but honest measure of "are these numbers full-mantissa?". A `3.3` writes two
/// digits; a value that has been through several hundred steps of a scattered pack
/// writes sixteen or seventeen, and those are the only ones a lossy float parser can
/// mis-round. Without this guard, a round-trip test can pass while comparing round
/// numbers that no encoding would ever damage.
fn longest_digit_run(text: &str) -> usize {
    let mut best = 0;
    let mut run = 0;
    for c in text.chars() {
        if c.is_ascii_digit() {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Construction: which text a page has to fetch, and what happens when it does not
// ---------------------------------------------------------------------------

/// The two-step dance a page does: ask which chemistry a scenario wants, fetch it, then
/// construct. Getting the answer wrong on the first call is what makes the second fail.
#[test]
fn a_scenario_naming_a_chemistry_says_so_before_anything_is_built() {
    let id = SimEngine::chemistry_id_of(CC_DISCHARGE).expect("the shipped scenario parses");
    assert_eq!(id.as_deref(), Some("lfp_26650_generic"));

    // The id is already charset-checked by `parse_scenario`, which is what makes it
    // safe for the page to interpolate straight into a URL.
    let id = id.unwrap();
    assert!(
        id.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        "an id a page pastes into a fetch URL must not need escaping, got {id:?}"
    );

    SimEngine::new(CC_DISCHARGE, Some(LFP_TOML)).expect("with its chemistry, it builds");
}

/// The error a page gets when it skipped the fetch has to name the file to fetch.
/// "Missing chemistry" would be true and useless.
#[test]
fn omitting_the_chemistry_text_names_the_file_that_was_needed() {
    let err = SimEngine::new(CC_DISCHARGE, None).expect_err("no chemistry, no pack");
    let msg = err.to_string();
    assert!(
        matches!(err, EngineError::ChemistryNotSupplied { .. }),
        "expected ChemistryNotSupplied, got {err:?}"
    );
    assert!(
        msg.contains("lfp_26650_generic.toml"),
        "the message must name the file to fetch, got {msg:?}"
    );

    // Whitespace is treated as absent. A page that built its fetch URL wrong and got a
    // blank body should hit this arm rather than a chemistry-parse error about an empty
    // document.
    let err = SimEngine::new(CC_DISCHARGE, Some("   \n ")).expect_err("blank is absent");
    assert!(matches!(err, EngineError::ChemistryNotSupplied { .. }));
}

/// A scenario that inlines its chemistry needs no second fetch, and its own text wins
/// over anything the page happened to be holding.
#[test]
fn an_inlined_scenario_is_self_contained_and_its_own_text_wins() {
    let inlined = format!(
        "chemistry_toml = '''\n{LFP_TOML}'''\n\n[meta]\nname = \"inlined\"\n\n\
         [pack]\nseries = 1\nparallel = 1\ninitial_soc = 1.0\n\
         initial_temp_k = 298.15\nseed = 7\n"
    );

    assert_eq!(
        SimEngine::chemistry_id_of(&inlined).expect("parses"),
        None,
        "an inlined scenario must tell the page there is nothing to fetch"
    );

    // Passing chemistry text alongside an inlined scenario must not override it. The
    // second argument is garbage on purpose: if it were consulted, this would fail to
    // parse rather than quietly disagree.
    let engine = SimEngine::new(&inlined, Some("this is not TOML at all ]["))
        .expect("a self-contained scenario ignores the argument, so the garbage is never parsed");
    assert_eq!(engine.facts().series, 1);
}

// ---------------------------------------------------------------------------
// The check that only exists on this boundary
// ---------------------------------------------------------------------------

/// **The reason this crate validates at all.**
///
/// Over `sim-server`'s socket a non-finite number is unreachable: JSON has no literal
/// for `NaN`, `serde_json` refuses `1e400`, and the parser rejects the message before
/// `StepCommand::validate` ever runs. `sim_server::protocol` says so in a comment and
/// keeps its checks anyway, "because slice D's wasm client will construct one in Rust
/// and call validate on it with no JSON parser in between".
///
/// This is that caller. `dt` and `report_every_n_steps` come from JS as raw numbers,
/// and `Number.NaN` needs no literal to exist.
#[test]
fn a_non_finite_dt_reaches_validation_because_nothing_parses_it() {
    let mut engine = scattered();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        let err = engine
            .step_many(bad, 1, Demand::Rest, 1)
            .expect_err("dt must be finite and >= 0");
        assert!(
            matches!(err, EngineError::OutOfRange(_)),
            "dt = {bad} should be OutOfRange, got {err:?}"
        );
    }

    // A non-finite demand can only arrive through JSON here, which refuses it — but
    // `step_many` takes a typed `Demand`, so the check is reachable from Rust and is
    // what a future non-JSON framing would lean on.
    let err = engine
        .step_many(1.0, 1, Demand::Current(f64::NAN), 1)
        .expect_err("a NaN demand is refused");
    assert!(matches!(err, EngineError::OutOfRange(_)), "got {err:?}");

    let err = engine
        .set_env(sim_core::Env {
            t_ambient: f64::NAN,
            t_coolant: None,
        })
        .expect_err("a NaN ambient is refused");
    assert!(matches!(err, EngineError::OutOfRange(_)), "got {err:?}");

    // And the pack is untouched by every one of those refusals.
    assert_eq!(engine.facts().sim_time_s, 0.0);
}

/// Both caps are rejected, not truncated, and the frame cap's message names the knob.
///
/// Truncating would be the tempting kindness and it is the wrong one: a client that
/// asked for a million frames and got ten thousand has a plot with a silent hole in it.
#[test]
fn the_two_caps_are_rejected_and_the_frame_cap_names_its_knob() {
    let mut engine = scattered();

    let err = engine
        .step_many(1.0, MAX_STEPS_PER_CALL + 1, Demand::Rest, 1)
        .expect_err("over the step cap");
    assert!(
        matches!(err, EngineError::TooManySteps { .. }),
        "got {err:?}"
    );

    // Under the step cap and over the frame cap: the two are independent, which is
    // exactly the footgun the second cap exists for.
    let n = MAX_FRAMES_PER_CALL + 1;
    assert!(n <= MAX_STEPS_PER_CALL, "this case must clear the step cap");
    let err = engine
        .step_many(1.0, n, Demand::Rest, 1)
        .expect_err("over the frame cap");
    assert!(
        matches!(err, EngineError::TooManyFrames { .. }),
        "got {err:?}"
    );
    assert!(
        err.to_string().contains("report_every_n_steps"),
        "the message must name the knob that fixes it, got {err}"
    );

    // The same batch with coarse decimation is fine — which is the point: fast-forward
    // is the case that works by construction.
    let frames = engine
        .step_many(1.0, n, Demand::Rest, 1000)
        .expect("coarse decimation clears the cap");
    assert_eq!(frames.len(), (n as usize).div_ceil(1000));

    // Zero on either count is a range error, not a no-op: silently doing nothing is
    // how a client ends up waiting forever for a batch it never actually asked for.
    assert!(matches!(
        engine.step_many(1.0, 0, Demand::Rest, 1),
        Err(EngineError::OutOfRange(_))
    ));
    assert!(matches!(
        engine.step_many(1.0, 1, Demand::Rest, 0),
        Err(EngineError::OutOfRange(_))
    ));
}

// ---------------------------------------------------------------------------
// Stepping
// ---------------------------------------------------------------------------

/// Decimation drops *reports*, never steps. The reported subset has to be bit-identical
/// to the corresponding steps of an undecimated run, and the last step is always in it.
#[test]
fn decimation_reports_a_subset_of_the_identical_trajectory() {
    const N: u32 = 250;
    const K: u32 = 7;

    let all = scattered()
        .step_many(1.5, N, Demand::Current(2.0), 1)
        .expect("undecimated run");
    let sampled = scattered()
        .step_many(1.5, N, Demand::Current(2.0), K)
        .expect("decimated run");

    // 250 / 7 = 35 whole multiples of 7, plus step 250 itself, which is not one.
    assert_eq!(sampled.len(), (N as usize).div_ceil(K as usize));
    assert_eq!(
        sampled.last().expect("non-empty").step,
        N,
        "the final step is always reported, whatever the modulus lands on"
    );

    for frame in &sampled {
        let matching = all
            .iter()
            .find(|f| f.step == frame.step)
            .expect("every reported step exists in the undecimated run");
        assert_eq!(
            frame_bits(frame),
            frame_bits(matching),
            "step {} differs between a decimated and an undecimated run",
            frame.step
        );
    }
}

/// The zero-length step is in the protocol so a page can read telemetry on load without
/// moving the pack. What it is *not* is a copy of the previous frame.
///
/// Telemetry is computed from **start-of-step** state, so step 5's frame reports the
/// heat implied by the state at the start of step 5, while a zero-length step
/// afterwards reports the heat implied by the state at the end of it. Same pack, one
/// step apart in what the number describes — which is exactly what makes `dt = 0`
/// useful. So the claim is: nothing mutates, and two consecutive zero-length reads
/// agree with each other.
#[test]
fn a_zero_length_step_reads_without_moving_anything() {
    let mut engine = scattered();
    engine
        .step_many(1.0, 5, Demand::Current(3.0), 1)
        .expect("five real steps");
    let t_after = engine.facts().sim_time_s;

    let a = engine.step_many(0.0, 1, Demand::Rest, 1).expect("read");
    let b = engine
        .step_many(0.0, 1, Demand::Rest, 1)
        .expect("read again");

    assert_eq!(engine.facts().sim_time_s, t_after, "the clock did not move");
    assert_eq!(
        frame_bits(&a[0]),
        frame_bits(&b[0]),
        "two consecutive zero-length reads must be bit-identical — if they are not, \
         something mutated"
    );
}

/// The demand crosses as externally-tagged JSON, the same spelling `sim-server` uses.
/// A page must not need two dialects for one engine.
#[test]
fn the_demand_spelling_is_the_one_the_server_already_speaks() {
    let mut engine = scattered();
    for json in [
        r#"{"Current": -5.0}"#,
        r#"{"Power": 12.5}"#,
        r#"{"Voltage": 13.2}"#,
        r#""Rest""#,
    ] {
        engine
            .step_many_json(0.5, 1, json, 1)
            .unwrap_or_else(|e| panic!("{json} should be a demand, got {e}"));
    }

    let err = engine
        .step_many_json(0.5, 1, r#"{"current": -5.0}"#, 1)
        .expect_err("the tag is case-sensitive");
    assert!(matches!(err, EngineError::Json { .. }), "got {err:?}");
    assert!(
        err.to_string().contains("Rest"),
        "the message should list the shapes that do work, got {err}"
    );
}

/// `EventFlags` crosses as a `" | "`-joined name string, `""` for none — not a bitmask
/// integer. The JS side splits on `" | "` and must treat `""` as the empty set rather
/// than as a flag named `""`, and this is what pins the format it is splitting.
#[test]
fn flags_cross_as_a_joined_name_string_and_empty_means_none() {
    let quiet = serde_json::to_string(&EventFlags::empty()).expect("serializes");
    assert_eq!(
        quiet, r#""""#,
        "no flags is the empty string, not null or 0"
    );

    let noisy =
        serde_json::to_string(&(EventFlags::OV | EventFlags::PLATING_RISK)).expect("serializes");
    assert_eq!(noisy, r#""OV | PLATING_RISK""#);

    // And it really is what a page reads off a frame.
    let mut engine = scattered();
    let frames = engine
        .step_many_json(1.0, 1, r#""Rest""#, 1)
        .expect("one step");
    assert!(
        frames.contains(r#""flags":""#),
        "a quiet step's flags field should be the empty string, got {frames}"
    );
}

// ---------------------------------------------------------------------------
// Snapshot / restore across the string boundary
// ---------------------------------------------------------------------------

/// The `float_roundtrip` guard for *this* crate.
///
/// `serde_json`'s default float parser is not correctly rounded, so a snapshot handed
/// to JS and handed straight back can come home one ULP different and the resumed
/// trajectory stops being bit-identical. The workspace turns the feature on; this is
/// what notices if a future manifest edit turns it off.
///
/// Built to fail, the way slice A's and B's were: the numbers come from a scattered
/// pack that has been running, and `longest_digit_run` asserts they really are
/// full-mantissa — because the failure mode of this whole test is silence. A probe that
/// drifted toward round numbers would keep passing while testing nothing.
#[test]
fn a_snapshot_through_json_resumes_bit_identically() {
    const SPLIT: u32 = 300;
    const REST: u32 = 300;

    let mut uninterrupted = scattered();
    uninterrupted
        .step_many(1.5, SPLIT, Demand::Current(1.5), 1)
        .expect("first half");
    let reference = uninterrupted
        .step_many(1.5, REST, Demand::Current(1.5), 1)
        .expect("second half, uninterrupted");

    let mut split = scattered();
    split
        .step_many(1.5, SPLIT, Demand::Current(1.5), 1)
        .expect("first half");

    let json = split.snapshot_json().expect("snapshot serializes");
    assert!(
        longest_digit_run(&json) >= 15,
        "the probe must be full-mantissa or it proves nothing about float parsing; \
         longest digit run was {}",
        longest_digit_run(&json)
    );

    // Restore into a *different* engine, which is what a page reloading a saved file
    // actually does — the same-object case would not exercise the parse at all.
    let mut resumed = scattered();
    resumed.restore_json(&json).expect("restore");
    assert_eq!(
        resumed.facts().sim_time_s,
        split.facts().sim_time_s,
        "the clock survives the round trip"
    );

    let after = resumed
        .step_many(1.5, REST, Demand::Current(1.5), 1)
        .expect("second half, resumed");

    for (want, got) in reference.iter().zip(after.iter()) {
        assert_eq!(
            frame_bits(want),
            frame_bits(got),
            "step {} (t = {} s) diverged after the restore",
            want.step,
            want.sim_time_s
        );
    }

    // The re-serialized text being a fixed point is the same regression seen without
    // any floats in the assertion — cheap, and it fails for the same reason.
    let again = resumed.snapshot_json().expect("re-serializes");
    let round_tripped = {
        let mut e = scattered();
        e.restore_json(&again).expect("restore again");
        e.snapshot_json().expect("and again")
    };
    assert_eq!(again, round_tripped, "snapshot JSON is not a fixed point");
}

/// A restore refuses a differently-shaped pack, the same rule `sim-server` applies at
/// `POST /sessions/{id}/snapshot` and for the same reason: the overwhelmingly likely
/// cause is the wrong file, and swapping the pack under a running plot is worse than
/// saying no.
#[test]
fn restoring_a_differently_shaped_pack_is_refused() {
    let single = SimEngine::new(CC_DISCHARGE, Some(LFP_TOML)).expect("1S1P builds");
    let foreign = single.snapshot_json().expect("snapshot");

    let mut pack = scattered();
    let err = pack
        .restore_json(&foreign)
        .expect_err("a 1S1P snapshot does not belong in a 4S2P session");
    assert!(
        matches!(err, EngineError::TopologyMismatch { .. }),
        "got {err:?}"
    );
    // And the refusal left the pack alone rather than half-replacing it.
    assert_eq!(pack.facts().series, 4);
    assert_eq!(pack.facts().parallel, 2);
}

// ---------------------------------------------------------------------------
// The BMS toggle, which is a comparison of two runs and not a switch on one
// ---------------------------------------------------------------------------

/// What the page's BMS toggle is for: the same scenario, charged the same way, with and
/// without protection.
///
/// With the BMS on, over-voltage is seen and acted on. With it off there is nothing to
/// see it — `OV` is raised only by the BMS (`bms.rs` is its only source), `soc_bms` is
/// `None`, and cell voltage sails past the chemistry's `v_max` unremarked. That
/// contrast is the teaching case, and it is why `restart` rebuilds rather than
/// pretending a BMS can be grown onto a pack mid-run.
#[test]
fn the_bms_toggle_is_the_difference_between_protected_and_not() {
    // Read off the parsed chemistry rather than written as a literal. A hardcoded 3.65
    // would keep passing against the wrong threshold the day someone refits
    // `chemistries/lfp_26650_generic.toml`, and the repo's rule is that a physical
    // constant is never restated without provenance — the file *is* the provenance.
    let v_max = sim_data::parse_chemistry(LFP_TOML)
        .expect("the shipped chemistry parses")
        .cell
        .v_max;
    let charge = Demand::Current(-5.0); // negative = charge

    let mut protected = scattered();
    assert!(
        protected.facts().has_bms && protected.facts().scenario_has_bms,
        "this scenario ships a BMS; the test is meaningless without one"
    );
    let with_bms = protected
        .step_many(2.0, 3000, charge, 100)
        .expect("charging with protection");

    let mut bare = scattered();
    bare.restart(false).expect("rebuild without the BMS");
    assert!(
        !bare.facts().has_bms,
        "restart(false) must actually remove it"
    );
    assert!(
        bare.facts().scenario_has_bms,
        "the scenario is unchanged — the page needs this to know the toggle can go back"
    );
    assert_eq!(bare.facts().sim_time_s, 0.0, "restart returns to t = 0");
    // The scenario's second fault is a `SensorOffset` on group 1's voltage sensor, and
    // sensors belong to the BMS. With no BMS there is nothing to lie, so the fault is
    // dropped — and counted, so this run is not silently a different experiment.
    assert_eq!(
        bare.facts().sensor_faults_dropped,
        1,
        "the lying-sensor fault has no instrument to land on without a BMS, and the \
         page must be able to say so"
    );
    let without_bms = bare
        .step_many(2.0, 3000, charge, 100)
        .expect("charging unprotected");

    let saw_ov = |frames: &[Frame]| {
        frames
            .iter()
            .any(|f| f.telemetry.flags.contains(EventFlags::OV))
    };
    assert!(saw_ov(&with_bms), "the BMS should have seen over-voltage");
    assert!(
        !saw_ov(&without_bms),
        "OV comes only from the BMS; without one there is nothing to raise it"
    );

    let peak = |frames: &[Frame]| {
        frames
            .iter()
            .fold(f64::NEG_INFINITY, |m, f| m.max(f.telemetry.v_cell_max))
    };
    // The sharpest discriminator is not a voltage threshold but **whether the pack got
    // what it was asked for**. Unprotected, the demand passes through untouched on
    // every single step; protected, the BMS derates it to nothing.
    assert!(
        without_bms
            .iter()
            .all(|f| f.telemetry.i_actual.to_bits() == (-5.0_f64).to_bits()),
        "with no BMS the demand passes through unclamped, every step"
    );
    assert!(
        with_bms
            .iter()
            .any(|f| f.telemetry.i_actual.to_bits() != (-5.0_f64).to_bits()),
        "the BMS should have derated the charge"
    );
    assert_eq!(
        with_bms.last().expect("non-empty").telemetry.i_actual,
        0.0,
        "by the end, protection has taken the charge current to zero"
    );

    // Both peaks are near `v_max`, and only one of them is on the right side of it.
    // The gap is small because this pack cannot go far past full: SOC clamps at 1.0 and
    // the OCV table tops out there, so an unprotected overcharge parks at
    // `OCV(1) + I·R` rather than climbing. Asserting a dramatic overshoot would be
    // asserting physics v1 does not model.
    assert!(
        peak(&without_bms) > peak(&with_bms),
        "unprotected should reach a higher cell voltage: {} vs {}",
        peak(&without_bms),
        peak(&with_bms)
    );
    assert!(
        peak(&without_bms) > v_max,
        "unprotected, the same charge should push past {v_max} V; peaked at {}",
        peak(&without_bms)
    );
    // The protected peak is *just over* `v_max` — measured 3.6604 V against 3.65 V — and
    // that is a known, owned behaviour rather than a leak: protection acts on the
    // reading it took at the start of a step, so it can overshoot a limit by one step.
    // Pinning the overshoot rather than asserting `< v_max` keeps this test honest about
    // what the engine promises. The 50 mV margin is a **generous round number, not a
    // derived bound** — the actual overshoot here is about 10 mV and scales with `dt`.
    assert!(
        peak(&with_bms) < v_max + 0.05,
        "protection should hold cell voltage to within one step's overshoot of \
         {v_max} V, peaked at {}",
        peak(&with_bms)
    );

    assert!(
        with_bms.iter().all(|f| f.telemetry.soc_bms.is_some()),
        "a BMS reports an estimate"
    );
    assert!(
        without_bms.iter().all(|f| f.telemetry.soc_bms.is_none()),
        "no BMS, no estimate — and the page must render that as absent, not as zero"
    );

    // Toggling back gives the scenario's own pack again, from t = 0 — including the
    // fault that had nowhere to land a moment ago.
    bare.restart(true).expect("rebuild with the BMS");
    assert!(bare.facts().has_bms);
    assert_eq!(bare.facts().sim_time_s, 0.0);
    assert_eq!(bare.facts().sensor_faults_dropped, 0);
}

/// A scenario that ships **no** BMS and a sensor fault is an authoring error, not a
/// case for the filter above. It must fail loudly at construction — otherwise a typo'd
/// scenario runs happily while quietly doing less than it says.
#[test]
fn a_sensor_fault_with_no_bms_to_sense_is_an_authoring_error() {
    let broken = format!(
        "chemistry_toml = '''\n{LFP_TOML}'''\n\n[meta]\nname = \"no bms, sensor fault\"\n\n\
         [pack]\nseries = 1\nparallel = 1\ninitial_soc = 1.0\n\
         initial_temp_k = 298.15\nseed = 7\n\n\
         [[faults]]\nat_s = 1.0\n\
         fault = {{ SensorOffset = {{ sensor = {{ GroupVoltage = 0 }}, offset = 0.1 }} }}\n"
    );
    let err = SimEngine::new(&broken, None).expect_err("there is no sensor to offset");
    assert!(
        err.to_string().contains("sensor"),
        "the message should say what is missing, got {err}"
    );
}

/// `restart` on a scenario that never had a BMS cannot invent one, and says so through
/// `scenario_has_bms` rather than by failing. That flag is what a page disables its
/// toggle on; without it the control would silently do nothing.
#[test]
fn a_scenario_with_no_bms_reports_that_the_toggle_has_nothing_to_offer() {
    let mut engine = SimEngine::new(CC_DISCHARGE, Some(LFP_TOML)).expect("builds");
    assert!(!engine.facts().scenario_has_bms);
    assert!(!engine.facts().has_bms);

    engine.restart(true).expect("rebuild");
    assert!(
        !engine.facts().has_bms,
        "restart(true) restores what the scenario configured, which here is nothing"
    );
}

// ---------------------------------------------------------------------------
// Faults
// ---------------------------------------------------------------------------

/// Faults arrive externally tagged, exactly as the scenario file writes them, and the
/// engine's own validation is what refuses a bad one — this crate translates rather
/// than duplicates.
#[test]
fn a_fault_is_spelled_the_way_the_scenario_file_spells_it() {
    let mut engine = scattered();
    engine.clear_faults(); // drop the scenario's own, so only ours can fire

    engine
        .schedule_fault_json(
            10.0,
            r#"{"SoftInternalShort": {"s": 1, "p": 0, "ohms": 5.0}}"#,
        )
        .expect("the scenario file's own spelling");

    let frames = engine
        .step_many(1.0, 40, Demand::Rest, 1)
        .expect("step past t = 10 s");
    assert_eq!(frames[8].telemetry.i_internal_short_a, 0.0, "not yet");
    assert!(
        frames[39].telemetry.i_internal_short_a > 0.0,
        "the short should be drawing current after it fires"
    );

    // Engine-owned validation, translated not re-implemented: an out-of-topology cell.
    let err = engine
        .schedule_fault_json(
            20.0,
            r#"{"SoftInternalShort": {"s": 99, "p": 0, "ohms": 5.0}}"#,
        )
        .expect_err("cell (99,0) is not in a 4S2P pack");
    assert!(matches!(err, EngineError::Fault(_)), "got {err:?}");

    let err = engine
        .schedule_fault_json(20.0, r#"{"NotAFault": {}}"#)
        .expect_err("not a fault at all");
    assert!(matches!(err, EngineError::Json { .. }), "got {err:?}");
}

/// The cells array is the ground-truth pedagogy view, in the order the page indexes it
/// by. Getting series-major/parallel-minor wrong draws the right numbers in the wrong
/// squares, which looks plausible and is wrong.
#[test]
fn the_cell_array_is_series_major_and_parallel_minor() {
    let mut engine = scattered();
    // The scenario's short fires at t = 600 s on cell (1,0); step past it so exactly
    // one cell is distinguishable from every other.
    engine
        .step_many(1.0, 700, Demand::Current(2.0), 700)
        .expect("past the fault");

    let cells = engine.cells();
    assert_eq!(cells.series, 4);
    assert_eq!(cells.parallel, 2);
    assert_eq!(cells.cells.len(), 8);

    // The index of cell (s = 1, p = 0) under series-major, parallel-minor ordering:
    // `s * parallel + p`.
    let (s, p) = (1_usize, 0_usize);
    let shorted = s * usize::from(cells.parallel) + p;
    for (i, cell) in cells.cells.iter().enumerate() {
        let has_short = cell.internal_short_conductance_s > 0.0;
        assert_eq!(
            has_short,
            i == shorted,
            "only index {shorted} — cell (1,0) — should carry the short, but index {i} \
             {} it",
            if has_short { "has" } else { "lacks" }
        );
    }
}
