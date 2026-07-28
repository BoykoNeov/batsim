//! **Phase 4's exit criterion.** An external script can run a full experiment over
//! WebSocket — and the transport is proven not to have touched the physics.
//!
//! # Why this is a bit-identical comparison and not a smoke test
//! "An external script can run a full experiment" is satisfiable by a test that
//! connects, sends a few demands, and eyeballs plausible numbers. That test would pass
//! while the transport quietly perturbed the trajectory: a lossy float encode, a
//! dropped step, a command applied in the wrong order. Every one of those produces
//! plausible numbers.
//!
//! So the *same* experiment runs twice — once by calling `Pack::step` in this process,
//! once by driving a real server over a real socket on an ephemeral port — and the two
//! telemetry streams are compared with `f64::to_bits`. Bits, not `==`, because
//! `-0.0 == 0.0` and `NaN != NaN`, so `==` can both launder a real difference and
//! invent one. That single assertion discharges the criterion and proves the transport
//! is physics-transparent.
//!
//! # What makes the comparison able to fail
//! Three things, each of which would otherwise let this pass while testing nothing:
//!
//! * **The in-process arm builds its pack the same way the server does** — through
//!   `Scenario::build_pack` on the same scenario file, not from a hand-written
//!   `PackConfig`. Two independently-built packs that happened to agree would be luck,
//!   not transparency.
//! * **The scenario has manufacturing scatter and a noisy current sensor.** Scatter is
//!   what fills the mantissas, and a one-ULP float bug is invisible on round numbers.
//!   The noise draws from the pack RNG every step, so RNG continuity across the restore
//!   is inside the comparison rather than beside it. Both are asserted, not assumed —
//!   either could be edited out of the scenario file by someone with an unrelated goal.
//! * **`longest_digit_run` asserts the wire text really is full-mantissa.** The failure
//!   mode of a probe like this is silence.
//!
//! # The restore leg is inside the timeline, not beside it
//! Halfway through, the WebSocket run stops and the session is snapshotted over REST
//! (`GET`), posted back into itself (`POST`), and resumed on the same socket. The
//! in-process arm splits at the *same step index* and does the equivalent
//! `snapshot` → JSON → `restore`. If the two arms split at different points the
//! comparison would be meaningless, so both constants come from the same place.
//!
//! The scenario's faults are timestamped 600 s, which at this `dt` is step 400 — on the
//! resumed side of a split at step 300, on purpose. A restore that silently dropped the
//! not-yet-fired fault queue would leave the comparison proving much less than it
//! looks, so the fault is asserted to have fired after the split.

mod common;

use common::{
    chem_dir, create_session, frame_bits, http, longest_digit_run, telemetry_bits, Client,
};
use sim_core::{Demand, Env, Pack, Snapshot};
use sim_data::{parse_chemistry, parse_scenario};
use sim_server::protocol::{Command, Event, Role, StepCommand};
use sim_server::AppState;

const SCENARIO_TOML: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");
const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

/// 1.5 s steps × 600 = 900 s of simulation, split at step 300 (t = 450 s).
///
/// One set of constants for both arms — the whole point is that they line up.
const DT_S: f64 = 1.5;
const STEPS: u64 = 600;
const SPLIT: u64 = 300;
/// The scenario's faults are timestamped 600 s, i.e. the end of step 400.
const FAULT_STEP: usize = 400;

const DEMAND: Demand = Demand::Current(2.9);

fn env() -> Env {
    // Sent explicitly on every command rather than relying on the session's standing
    // default, so this test compares transports and not two spellings of "ambient".
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

fn step_command(n_steps: u64) -> Command {
    Command::Step(StepCommand {
        dt: DT_S,
        n_steps,
        demand: DEMAND,
        env: Some(env()),
        // The gate runs undecimated. Decimation drops reports and never steps, so it
        // cannot affect this claim — but proving that is `ws.rs`'s job, not this one's.
        report_every_n_steps: 1,
    })
}

/// The scenario, with the two properties this test leans on checked rather than
/// assumed.
fn scenario() -> sim_data::Scenario {
    let scenario = parse_scenario(SCENARIO_TOML).expect("the shipped fault scenario parses");
    let bms = scenario
        .pack
        .bms
        .as_ref()
        .expect("this gate needs a BMS: its current-sensor noise is the RNG draw per step");
    assert!(
        bms.current_noise_sigma_a > 0.0,
        "current_noise_sigma_a must be > 0, or the pack RNG is never drawn from after \
         construction and this comparison stops covering RNG continuity across the restore"
    );
    assert!(
        scenario.pack.scatter.capacity_sigma > 0.0 && scenario.pack.scatter.r0_sigma > 0.0,
        "scatter is what fills the mantissas; without it a lossy float encoding has \
         nothing to round wrongly and this gate passes for the wrong reason"
    );
    scenario
}

/// The reference: the same experiment, entirely in this process.
///
/// Returns `(sim_time_s, telemetry)` per step, because the wire frames carry the time
/// and it has to be compared too — a transport that reported the time from before the
/// step rather than after would otherwise slip through.
fn in_process_run() -> Vec<(f64, sim_core::Telemetry)> {
    let chem = parse_chemistry(LFP_TOML).expect("the shipped LFP chemistry parses");
    let mut pack = scenario().build_pack(chem).expect("the scenario builds");
    let env = env();

    let mut out: Vec<(f64, sim_core::Telemetry)> = (0..SPLIT)
        .map(|_| {
            let telemetry = pack.step(DT_S, DEMAND, &env);
            (pack.sim_time_s(), telemetry)
        })
        .collect();

    // The same round trip the REST leg performs, in the same place in the timeline.
    let text = serde_json::to_string(&pack.snapshot()).expect("snapshot serializes");
    let parsed: Snapshot = serde_json::from_str(&text).expect("snapshot parses");
    let mut pack = Pack::restore(&parsed).expect("same SNAPSHOT_VERSION");

    out.extend((SPLIT..STEPS).map(|_| {
        let telemetry = pack.step(DT_S, DEMAND, &env);
        (pack.sim_time_s(), telemetry)
    }));
    out
}

/// The gate.
#[tokio::test]
async fn a_websocket_experiment_is_bit_identical_to_running_the_engine_in_process() {
    let reference = in_process_run();

    let address = common::spawn(AppState::new(chem_dir())).await;
    let id = create_session(address, SCENARIO_TOML).await;

    let (mut client, hello) = Client::attach_and_greet(address, id).await;
    match hello {
        Event::Hello {
            role, session_id, ..
        } => {
            assert_eq!(role, Role::Writer, "the first socket to attach writes");
            assert_eq!(session_id, id);
        }
        other => panic!("expected Hello, got {other:?}"),
    }

    // Leg one.
    let mut frames = client.step(&step_command(SPLIT)).await;
    assert_eq!(frames.len(), SPLIT as usize);

    // The restore leg, over REST, mid-experiment, into the same session.
    let (status, snapshot_body) = http(
        address,
        "GET",
        &format!("/sessions/{id}/snapshot"),
        None,
        "",
    )
    .await;
    assert_eq!(status, 200, "snapshot GET failed: {snapshot_body}");
    assert!(
        longest_digit_run(&snapshot_body) >= 15,
        "the snapshot's longest digit run is {}, so its values are too round for this \
         gate to discriminate on float exactness — the probe has degenerated",
        longest_digit_run(&snapshot_body)
    );

    let (status, restore_body) = http(
        address,
        "POST",
        &format!("/sessions/{id}/snapshot"),
        Some("application/json"),
        &snapshot_body,
    )
    .await;
    assert_eq!(status, 200, "snapshot POST failed: {restore_body}");

    // Leg two, on the same socket, into the pack the restore installed.
    frames.extend(client.step(&step_command(STEPS - SPLIT)).await);
    assert_eq!(frames.len(), STEPS as usize);

    // The comparison the criterion turns on.
    assert_eq!(reference.len(), frames.len());
    for (n, ((sim_time_s, want), got)) in reference.iter().zip(&frames).enumerate() {
        assert_eq!(
            telemetry_bits(*sim_time_s, want),
            frame_bits(got),
            "step {n} (t = {sim_time_s} s) differs between the in-process run and the \
             WebSocket run\n  in process: {want:?}\n  over the wire: {:?}",
            got.telemetry
        );
        assert_eq!(
            want.flags, got.telemetry.flags,
            "step {n}: flags differ between transports"
        );
    }

    // Step indices restart per batch and the simulation clock does not: a client can
    // tell which steps it got from `step`, and where the pack is from `sim_time_s`.
    assert_eq!(frames[0].step, 1);
    assert_eq!(frames[(SPLIT - 1) as usize].step, SPLIT);
    assert_eq!(
        frames[SPLIT as usize].step, 1,
        "the second batch numbers its own steps"
    );
    assert!(
        frames[SPLIT as usize].sim_time_s > frames[(SPLIT - 1) as usize].sim_time_s,
        "the simulation clock is absolute and must not restart with the batch"
    );

    // The queue of faults that had not yet fired crossed the restore. Without this the
    // bit-identity above would still hold — of a duller experiment.
    assert_eq!(
        frames[FAULT_STEP - 1].telemetry.i_internal_short_a,
        0.0,
        "no internal short should be drawing before its timestamp"
    );
    assert!(
        frames[FAULT_STEP].telemetry.i_internal_short_a > 0.0,
        "the scenario's soft internal short is timestamped inside the resumed half; if \
         it never fired, the restored pack lost its fault queue and everything above \
         was proving less than it looks"
    );

    // The BMS estimate moved, which is what makes the per-step RNG draw observable in
    // the compared values rather than merely present.
    let first = frames[0].telemetry.soc_bms.expect("the pack has a BMS");
    let last = frames[(STEPS - 1) as usize]
        .telemetry
        .soc_bms
        .expect("the pack has a BMS");
    assert!(
        (first - last).abs() > 1e-3,
        "the BMS estimate barely moved ({first} → {last}), so the noisy current sensor \
         is contributing nothing observable to the comparison"
    );
}

/// The same run, but the snapshot round trip happens over the **socket** instead of
/// over REST.
///
/// `Command::Snapshot` and `Command::Restore` are a different code path from the REST
/// routes — different framing, different error type, and a `Snapshot` nested inside an
/// enum rather than sent as a whole body. That last part is the one worth a separate
/// test: a `Snapshot` contains a `u128` (`ChaCha8Rng`'s `word_pos`), and any serde
/// shape that buffers through `Content` — an internally tagged enum, a
/// `#[serde(flatten)]` — fails on it at *runtime* while compiling perfectly. The
/// protocol is externally tagged for that reason, and this is what would notice if it
/// stopped being.
#[tokio::test]
async fn the_socket_can_snapshot_and_restore_without_leaving_the_socket() {
    let reference = in_process_run();

    let address = common::spawn(AppState::new(chem_dir())).await;
    let id = create_session(address, SCENARIO_TOML).await;
    let (mut client, _) = Client::attach_and_greet(address, id).await;

    let mut frames = client.step(&step_command(SPLIT)).await;

    let snapshot = match client.round_trip(&Command::Snapshot).await {
        Event::Snapshot { snapshot } => snapshot,
        other => panic!("expected a snapshot, got {other:?}"),
    };
    match client.round_trip(&Command::Restore { snapshot }).await {
        Event::Restored { pack } => assert_eq!(
            pack.sim_time_s.to_bits(),
            (SPLIT as f64 * DT_S).to_bits(),
            "the restored pack is not where the snapshot was taken"
        ),
        other => panic!("expected Restored, got {other:?}"),
    }

    frames.extend(client.step(&step_command(STEPS - SPLIT)).await);

    for (n, ((sim_time_s, want), got)) in reference.iter().zip(&frames).enumerate() {
        assert_eq!(
            telemetry_bits(*sim_time_s, want),
            frame_bits(got),
            "step {n}: a snapshot round trip over the socket perturbed the trajectory"
        );
    }
}
