//! Snapshot → JSON → restore → continue must be **bit-identical** to never having
//! stopped.
//!
//! This is the regression guard for `serde_json`'s `float_roundtrip` feature, which
//! the workspace manifest declares and which nothing else would notice the loss of.
//! Without it the deserializer's fast path is not correctly rounded and can return a
//! value one ULP off the one `ryu` wrote — silently, rarely, and only on values with
//! full mantissas. A restored session then drifts from the run it claims to continue,
//! and the drift looks like a physics bug.
//!
//! # Why the numbers come from where they come from
//! Slice A's experience, stated so it is not re-learned: a round-trip test built from
//! hand-written literals (`3.3`, `298.15`, `0.5`) **passes with the feature off** and
//! guards nothing. So does one built from a pack stepped two or three times. The
//! defect only appears on values whose mantissas are full, which means a real
//! chemistry off disk, manufacturing scatter on, and enough steps for the state to
//! stop being round. This test loads a shipped scenario for exactly that reason, and
//! `longest_digit_run` asserts the probe has not quietly degenerated into round
//! numbers.
//!
//! # What the split exercises beyond floats
//! The scenario's BMS has `current_noise_sigma_a > 0`, so the pack RNG is drawn from
//! **once per step** and its estimate depends on the draw. If the snapshot did not
//! carry RNG state across the restore, the second half's `soc_bms` would diverge and
//! this test would fail. Its faults fire at t = 600 s, which the split at t = 450 s
//! puts squarely in the resumed half — so the queue of not-yet-fired faults has to
//! survive the round trip too. Both properties are asserted directly rather than
//! assumed, because either could silently stop holding.

use sim_core::{Demand, Env, Pack, Snapshot, Telemetry};
use sim_data::{parse_chemistry, parse_scenario};

const SCENARIO_TOML: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");
const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

/// 1.5 s steps × 600 = 900 s of simulation, split at step 300 (t = 450 s).
///
/// The scenario's faults are timestamped 600 s, i.e. step 400 — after the split, on
/// purpose.
const DT_S: f64 = 1.5;
const STEPS: usize = 600;
const SPLIT: usize = 300;
const FAULT_STEP: usize = 400;

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn fresh_pack() -> Pack {
    let scenario = parse_scenario(SCENARIO_TOML).expect("the shipped fault scenario parses");
    let chem = parse_chemistry(LFP_TOML).expect("the shipped LFP chemistry parses");

    // Pin the two properties that make this probe worth running. Either could be
    // edited out of the scenario file by someone with an unrelated goal, and the
    // bit-identity assertion would keep passing while testing much less.
    let bms = scenario
        .pack
        .bms
        .as_ref()
        .expect("this probe needs a BMS: its current-sensor noise is the RNG draw per step");
    assert!(
        bms.current_noise_sigma_a > 0.0,
        "current_noise_sigma_a must be > 0 or the pack RNG is never drawn from after \
         construction, and this test stops covering RNG continuity across a restore"
    );
    assert!(
        scenario.pack.scatter.capacity_sigma > 0.0 && scenario.pack.scatter.r0_sigma > 0.0,
        "scatter is what fills the mantissas; without it a lossy float parser has \
         nothing to round wrongly"
    );

    scenario.build_pack(chem).expect("the scenario builds")
}

/// Every `f64` a `Telemetry` carries, as raw bits.
///
/// `to_bits`, not `==`: `-0.0 == 0.0` and `NaN != NaN`, so `==` can both hide a real
/// difference and invent one.
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
        t.i_rejected_a.to_bits(),
    ];
    // The BMS estimate is the RNG-sensitive one; `u64::MAX` stands in for `None` so a
    // `Some`/`None` change cannot alias a value change.
    bits.push(t.soc_bms.map_or(u64::MAX, f64::to_bits));
    bits
}

/// The longest run of digits anywhere in the JSON.
///
/// A crude but honest measure of "are these numbers full-mantissa?". A `3.3` writes
/// two digits; a value that has been through several hundred steps of a scattered
/// pack writes sixteen or seventeen, and those are the only ones a lossy parser can
/// mis-round.
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

/// The load-bearing one.
#[test]
fn snapshot_json_restore_continues_the_same_trajectory_bit_for_bit() {
    let env = env();
    let demand = Demand::Current(2.9);

    // Reference: never interrupted.
    let mut reference = fresh_pack();
    let uninterrupted: Vec<Telemetry> = (0..STEPS)
        .map(|_| reference.step(DT_S, demand, &env))
        .collect();

    // Split: stopped, serialised to JSON text, parsed back, restored, resumed.
    let mut pack = fresh_pack();
    let mut split: Vec<Telemetry> = (0..SPLIT).map(|_| pack.step(DT_S, demand, &env)).collect();

    let text = serde_json::to_string(&pack.snapshot()).expect("snapshot serialises");
    assert!(
        longest_digit_run(&text) >= 15,
        "the snapshot's longest digit run is {}, so its values are too round for this \
         test to discriminate on float exactness — the probe has degenerated",
        longest_digit_run(&text)
    );

    let parsed: Snapshot = serde_json::from_str(&text).expect("snapshot parses");
    let mut resumed = Pack::restore(&parsed).expect("same SNAPSHOT_VERSION");

    assert_eq!(
        resumed.sim_time_s().to_bits(),
        pack.sim_time_s().to_bits(),
        "simulation time did not survive the round trip"
    );

    split.extend((SPLIT..STEPS).map(|_| resumed.step(DT_S, demand, &env)));

    // Every step, not just the last: a one-ULP divergence can take a while to grow
    // into something a spot check would see, and "the endpoint matches" is a weaker
    // claim than "the trajectory matches".
    for (n, (want, got)) in uninterrupted.iter().zip(&split).enumerate() {
        assert_eq!(
            telemetry_bits(want),
            telemetry_bits(got),
            "step {n} (t = {} s) diverged after the restore at step {SPLIT}\n  \
             uninterrupted: {want:?}\n  resumed:       {got:?}",
            n as f64 * DT_S
        );
        assert_eq!(want.flags, got.flags, "step {n}: flags diverged");
    }

    // The queue of faults that had not yet fired crossed the wire with the pack: the
    // short at t = 600 s is on the resumed side of the split, and it did fire there.
    assert_eq!(
        split[FAULT_STEP - 1].i_internal_short_a,
        0.0,
        "no internal short should be drawing before its timestamp"
    );
    assert!(
        split[FAULT_STEP].i_internal_short_a > 0.0,
        "the scenario's soft internal short is timestamped inside the resumed half; if \
         it never fired, the restored pack lost its fault queue and the comparison above \
         was proving less than it looks"
    );

    // The BMS estimate exists and moved, which is what makes the per-step RNG draw
    // observable in the compared values.
    let first = split[0].soc_bms.expect("the scenario's pack has a BMS");
    let last = split[STEPS - 1]
        .soc_bms
        .expect("the scenario's pack has a BMS");
    assert!(
        (first - last).abs() > 1e-3,
        "the BMS estimate barely moved ({first} → {last}), so the noisy current sensor \
         is contributing nothing observable to the comparison"
    );
}

/// The cheap canary: re-serialising a parsed snapshot reproduces the same text.
///
/// A string comparison with no float comparison in sight, and it catches the same
/// regression. Without `float_roundtrip` the text was observed not to be a fixed
/// point — the parse lands one ULP away and `ryu` then writes the shortest
/// representation of *that*, which is a different string.
#[test]
fn snapshot_json_text_is_a_fixed_point() {
    let env = env();
    let mut pack = fresh_pack();
    for _ in 0..SPLIT {
        pack.step(DT_S, Demand::Current(2.9), &env);
    }

    let once = serde_json::to_string(&pack.snapshot()).expect("serialises");
    let parsed: Snapshot = serde_json::from_str(&once).expect("parses");
    let twice = serde_json::to_string(&parsed).expect("re-serialises");

    assert_eq!(
        once, twice,
        "snapshot JSON is not a fixed point — the parse landed somewhere the writer \
         does not spell the same way"
    );
}

/// Telemetry itself survives JSON bit for bit, on a pack that has been through faults.
///
/// `sim-data` already pins this on a clean pack; the value here is the fault-affected
/// fields (`i_internal_short_a`, and the flags a lying sensor does *not* raise), which
/// no other test puts on a wire.
#[test]
fn telemetry_survives_json_bit_for_bit_through_a_fault() {
    let env = env();
    let mut pack = fresh_pack();

    for n in 0..STEPS {
        let tele = pack.step(DT_S, Demand::Current(2.9), &env);
        let text = serde_json::to_string(&tele).expect("serialises");
        let back: Telemetry = serde_json::from_str(&text).expect("parses");
        assert_eq!(
            telemetry_bits(&tele),
            telemetry_bits(&back),
            "step {n}: telemetry changed across JSON\n  out: {text}\n  in:  {back:?}"
        );
        assert_eq!(
            tele.flags, back.flags,
            "step {n}: flags changed across JSON"
        );
    }
}
