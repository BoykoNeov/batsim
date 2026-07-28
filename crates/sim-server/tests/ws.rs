//! The stepping protocol: roles, the boundary checks, decimation, and the two
//! backpressure rules.
//!
//! `e2e_experiment.rs` carries the phase's exit criterion — that the transport does not
//! perturb the physics. This file covers everything the criterion does not: what
//! happens when a client sends something wrong, sends too much, attaches twice, or
//! cannot keep up.

mod common;

use common::{chem_dir, create_session, frame_bits, http, Client};
use sim_core::{Demand, Env, Fault};
use sim_server::protocol::{Command, Event, Limits, Role, StepCommand};
use sim_server::{AppState, ErrorCode};

const CC_DISCHARGE: &str = include_str!("../../../scenarios/cc_discharge_lfp.toml");
const SOFT_SHORT: &str = include_str!("../../../scenarios/soft_short_under_a_lying_sensor.toml");

fn env() -> Env {
    Env {
        t_ambient: 298.15,
        t_coolant: None,
    }
}

/// A step command with the fields these tests mostly do not vary.
fn step(dt: f64, n_steps: u64, report_every_n_steps: u64) -> Command {
    Command::Step(StepCommand {
        dt,
        n_steps,
        demand: Demand::Current(1.0),
        env: Some(env()),
        report_every_n_steps,
    })
}

/// Server, session, and an attached writer — the opening of nearly every test here.
async fn writer_on(scenario: &str) -> (std::net::SocketAddr, u64, Client) {
    let address = common::spawn(AppState::new(chem_dir())).await;
    let id = create_session(address, scenario).await;
    let (client, _) = Client::attach_and_greet(address, id).await;
    (address, id, client)
}

/// Pull the code out of an `Error` event, or say what arrived instead.
fn error_code(event: &Event) -> ErrorCode {
    match event {
        Event::Error { code, .. } => *code,
        other => panic!("expected an Error event, got {other:?}"),
    }
}

// ---------------------------------------------------------------- hello and roles

#[tokio::test]
async fn hello_reports_both_versions_the_role_and_the_caps() {
    let address = common::spawn(AppState::new(chem_dir())).await;
    let id = create_session(address, CC_DISCHARGE).await;
    let (_client, hello) = Client::attach_and_greet(address, id).await;

    let Event::Hello {
        api_version,
        snapshot_version,
        session_id,
        role,
        pack,
        env,
        limits,
    } = hello
    else {
        panic!("expected Hello")
    };

    // Two numbers with two jobs, reported together so a client never has to guess
    // which one it broke.
    assert_eq!(api_version, sim_server::API_VERSION);
    assert_eq!(snapshot_version, sim_core::SNAPSHOT_VERSION);
    assert_eq!(session_id, id);
    assert_eq!(role, Role::Writer);
    assert_eq!((pack.series, pack.parallel), (1, 1));
    assert_eq!(pack.sim_time_s, 0.0);
    // The standing environment is seeded from the scenario, so a client that never
    // sends `SetEnv` still steps into a defined room rather than absolute zero.
    assert!(
        env.t_ambient > 200.0,
        "t_ambient defaulted to {}",
        env.t_ambient
    );
    assert_eq!(env.t_coolant, None);
    // Advertised so a client can size its batches instead of discovering the caps by
    // being rejected.
    assert_eq!(limits.max_steps_per_command, 1_000_000);
    assert_eq!(limits.max_frames_per_reply, 10_000);
}

#[tokio::test]
async fn a_second_socket_observes_and_its_commands_are_refused() {
    let (address, id, mut writer) = writer_on(CC_DISCHARGE).await;
    let (mut observer, hello) = Client::attach_and_greet(address, id).await;

    match hello {
        Event::Hello { role, .. } => assert_eq!(
            role,
            Role::Observer,
            "a session has one writer; later attachers observe"
        ),
        other => panic!("expected Hello, got {other:?}"),
    }

    // A command that would move the pack is refused with a code, not ignored.
    assert_eq!(
        error_code(&observer.round_trip(&step(1.0, 1, 1)).await),
        ErrorCode::NotWriter
    );
    // ...and so is every other pack-touching command, including the harmless-looking
    // read-only one, because a snapshot is still a command against a session someone
    // else owns.
    assert_eq!(
        error_code(&observer.round_trip(&Command::Snapshot).await),
        ErrorCode::NotWriter
    );

    // `Ping` is the exception: it is the only command that does not touch the pack, and
    // a read-only client still needs to check its socket is alive.
    assert!(matches!(
        observer.round_trip(&Command::Ping).await,
        Event::Pong
    ));

    // Read-only is not a consolation prize — the observer gets the stream. This is how
    // a teaching demo puts one live pack on two screens.
    let written = writer.step(&step(2.0, 3, 1)).await;
    let mut observed = Vec::new();
    loop {
        match observer.next_event().await {
            Event::Telemetry(frame) => observed.push(frame),
            Event::BatchComplete { .. } => break,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(written.len(), observed.len());
    for (w, o) in written.iter().zip(&observed) {
        assert_eq!(
            frame_bits(w),
            frame_bits(o),
            "the observer saw a different number than the writer did"
        );
    }
}

#[tokio::test]
async fn the_writer_slot_is_freed_when_the_writer_disconnects() {
    let (address, id, writer) = writer_on(CC_DISCHARGE).await;

    // While it is held, a second socket cannot have it...
    let (second, hello) = Client::attach_and_greet(address, id).await;
    assert!(matches!(
        hello,
        Event::Hello {
            role: Role::Observer,
            ..
        }
    ));
    drop(second);

    writer.close().await;

    // ...and once the writer is gone, the next socket to attach takes it. Note that
    // this is a *new* socket: the observer above was not promoted, because a client
    // told it is read-only should not silently acquire the ability to move the pack.
    let (mut third, hello) = Client::attach_and_greet(address, id).await;
    assert!(
        matches!(
            hello,
            Event::Hello {
                role: Role::Writer,
                ..
            }
        ),
        "the freed writer slot was not handed on"
    );
    assert_eq!(third.step(&step(1.0, 1, 1)).await.len(), 1);
}

// ------------------------------------------------------------ boundary validation

/// The range checks JSON *can* reach.
///
/// The engine is permissive by contract: `step` promises never to panic, and promises
/// nothing about a demand of `1e300` or a negative `dt` producing anything meaningful.
/// Making it defensive would cost a branch per field on the hot path to guard a hazard
/// that only exists at a socket, so the socket rejects instead.
#[tokio::test]
async fn the_boundary_rejects_what_the_engine_would_accept_and_regret() {
    let (_address, _id, mut writer) = writer_on(CC_DISCHARGE).await;

    for (what, command) in [
        ("a negative dt", step(-1.0, 1, 1)),
        ("zero steps", step(1.0, 0, 1)),
        ("zero decimation", step(1.0, 10, 0)),
    ] {
        assert_eq!(
            error_code(&writer.round_trip(&command).await),
            ErrorCode::OutOfRange,
            "{what} was not rejected"
        );
    }

    // The session is still usable after all of that: a rejected command must not have
    // moved the pack or killed the socket.
    let frames = writer.step(&step(1.0, 1, 1)).await;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].sim_time_s, 1.0);
}

/// **JSON cannot express a non-finite number at all**, and this is where that was
/// found rather than assumed.
///
/// The plan's boundary rule — "every `f64` in a `Demand` or an `Env` must be finite,
/// rejected with a structured error" — is written as though `NaN` could arrive over the
/// wire. It cannot. There is no JSON literal for it (`NaN` is a syntax error), an
/// overflowing literal like `1e400` is refused by the parser as *number out of range*,
/// and a client that serializes `f64::NAN` with `serde_json` sends `null`, which then
/// fails as a type error. Three different messages, one outcome: serde refuses the
/// message before [`StepCommand::validate`] is ever called, so a non-finite value over
/// this socket is an `invalid_command`, not an `out_of_range`.
///
/// That does **not** make the finiteness checks dead code, which is why they stay and
/// why the test below calls them directly. `StepCommand` is a Rust type as much as a
/// wire type: slice D's wasm client will build one in Rust and call `validate` on it,
/// with no JSON parser in between to have already refused it. A binary framing would
/// carry `NaN` happily too.
#[tokio::test]
async fn json_refuses_a_non_finite_number_before_validation_can_see_it() {
    let (_address, _id, mut writer) = writer_on(CC_DISCHARGE).await;

    for (what, text) in [
        (
            "the NaN literal",
            r#"{"Step":{"dt":NaN,"n_steps":1,"demand":"Rest"}}"#,
        ),
        (
            "an overflowing literal",
            r#"{"Step":{"dt":1e400,"n_steps":1,"demand":"Rest"}}"#,
        ),
        (
            "what serde_json writes for NaN",
            r#"{"Step":{"dt":null,"n_steps":1,"demand":"Rest"}}"#,
        ),
        (
            "an overflowing demand",
            r#"{"Step":{"dt":1.0,"n_steps":1,"demand":{"Power":1e400}}}"#,
        ),
        (
            "an overflowing ambient",
            r#"{"Step":{"dt":1.0,"n_steps":1,"demand":"Rest","env":{"t_ambient":-1e400,"t_coolant":null}}}"#,
        ),
    ] {
        writer.send_raw(text).await;
        assert_eq!(
            error_code(&writer.next_event().await),
            ErrorCode::InvalidCommand,
            "{what} produced the wrong code"
        );
    }

    assert_eq!(writer.step(&step(1.0, 1, 1)).await.len(), 1);
}

/// The finiteness checks themselves, called the way a non-JSON client will call them.
///
/// Over the socket these are unreachable (see above). Reached directly, they are the
/// contract `sim-wasm` will lean on.
#[test]
fn validate_rejects_every_non_finite_field() {
    let limits = Limits::default();
    let ok = StepCommand {
        dt: 1.0,
        n_steps: 1,
        demand: Demand::Rest,
        env: None,
        report_every_n_steps: 1,
    };
    assert!(ok.validate(&limits).is_ok());

    let cases = [
        ("a NaN dt", StepCommand { dt: f64::NAN, ..ok }),
        (
            "an infinite dt",
            StepCommand {
                dt: f64::INFINITY,
                ..ok
            },
        ),
        (
            "a non-finite demand",
            StepCommand {
                demand: Demand::Power(f64::NAN),
                ..ok
            },
        ),
        (
            "a non-finite ambient",
            StepCommand {
                env: Some(Env {
                    t_ambient: f64::NAN,
                    t_coolant: None,
                }),
                ..ok
            },
        ),
        (
            "a non-finite coolant",
            StepCommand {
                env: Some(Env {
                    t_ambient: 298.15,
                    t_coolant: Some(f64::NEG_INFINITY),
                }),
                ..ok
            },
        ),
    ];

    for (what, command) in cases {
        let error = command
            .validate(&limits)
            .expect_err(&format!("{what} was accepted"));
        assert_eq!(error.code(), ErrorCode::OutOfRange, "{what}");
    }
}

#[tokio::test]
async fn malformed_messages_are_refused_without_killing_the_socket() {
    let (_address, _id, mut writer) = writer_on(CC_DISCHARGE).await;

    for text in [
        "not json at all",
        r#"{"Nonexistent":{}}"#,
        // Right shape, wrong tagging — the internally tagged spelling a JS client
        // would reach for first. Refused loudly rather than half-understood.
        r#"{"cmd":"step","dt":1.0,"n_steps":1}"#,
        // `deny_unknown_fields` on `StepCommand`: a typo in a field name is a rejected
        // command, not a silently ignored one.
        r#"{"Step":{"dt":1.0,"n_steps":1,"demand":"Rest","report_every":5}}"#,
    ] {
        writer.send_raw(text).await;
        assert_eq!(
            error_code(&writer.next_event().await),
            ErrorCode::InvalidCommand,
            "{text} was not refused"
        );
    }

    assert_eq!(writer.step(&step(1.0, 1, 1)).await.len(), 1);
}

// ------------------------------------------------------------------- step semantics

/// Decimation drops **reports**, never steps.
///
/// Proved by comparing against an undecimated run of the same scenario: the reported
/// frames must be bit-identical to the corresponding steps of the full run, so the
/// trajectory cannot have changed — only the sampling of it.
#[tokio::test]
async fn decimation_reports_a_subset_and_always_the_final_step() {
    const N: u64 = 100;
    const K: u64 = 7;

    let address = common::spawn(AppState::new(chem_dir())).await;

    // Two sessions from the same scenario: same seed, same trajectory.
    let full_id = create_session(address, CC_DISCHARGE).await;
    let (mut full_client, _) = Client::attach_and_greet(address, full_id).await;
    let full = full_client.step(&step(1.0, N, 1)).await;
    assert_eq!(full.len() as u64, N);

    let thin_id = create_session(address, CC_DISCHARGE).await;
    let (mut thin_client, _) = Client::attach_and_greet(address, thin_id).await;
    let thin = thin_client.step(&step(1.0, N, K)).await;

    // 100 steps by sevens is 14 multiples plus the final step, which is not one.
    assert_eq!(thin.len(), 15);

    let want: Vec<u64> = (1..=N).filter(|i| i % K == 0 || *i == N).collect();
    assert_eq!(
        thin.iter().map(|f| f.step).collect::<Vec<_>>(),
        want,
        "the reported step indices are not the ones the rule describes"
    );
    assert_eq!(
        thin.last().expect("a final frame").step,
        N,
        "the final step must always be reported, or a client's last sample is wherever \
         the modulus happened to land rather than the true end state"
    );

    for frame in &thin {
        let matching = &full[(frame.step - 1) as usize];
        assert_eq!(
            frame_bits(frame),
            frame_bits(matching),
            "step {} differs between a decimated and an undecimated run, so decimation \
             is dropping steps and not merely reports",
            frame.step
        );
    }
}

/// A frame describes the pack at the **end** of its own step.
///
/// Off by one here is exactly how off-by-one-step plots happen, and nothing downstream
/// could notice: a client that never sees `dt` has no way to tell 0, 1, 2 from 1, 2, 3.
#[tokio::test]
async fn sim_time_is_read_after_the_step_that_produced_the_frame() {
    let (_address, _id, mut writer) = writer_on(CC_DISCHARGE).await;
    let frames = writer.step(&step(1.0, 3, 1)).await;
    assert_eq!(
        frames.iter().map(|f| f.sim_time_s).collect::<Vec<_>>(),
        vec![1.0, 2.0, 3.0]
    );
}

/// `dt = 0` is allowed on purpose: it is how a client reads telemetry without
/// advancing, which is what a page wants on connect. `n_steps` bounds the loop, so it
/// cannot hang anything.
///
/// # Why this asserts idempotence rather than "same as the last frame"
/// A zero-length step's frame is *not* a copy of the preceding frame, and finding that
/// out is worth the note. The engine computes `q_gen_w` from **start-of-step** state,
/// so step 5's frame reports the heat implied by the state at the start of step 5,
/// while a following zero-length step reports the heat implied by the state at the end
/// of it. Same pack, one step apart in what the number describes.
///
/// That is exactly the property that makes `dt = 0` useful — it answers "what is the
/// pack doing *now*", which is a different question from "what was it doing during the
/// last step". So the claim under test is that the pack did not move: two consecutive
/// zero-length steps must be bit-identical to each other, and the state fields must
/// match the previous frame.
#[tokio::test]
async fn a_zero_length_step_reports_without_advancing() {
    let (_address, _id, mut writer) = writer_on(CC_DISCHARGE).await;

    let moved = writer.step(&step(10.0, 5, 1)).await;
    let after = *moved.last().expect("frames");

    let first = writer.step(&step(0.0, 1, 1)).await;
    let second = writer.step(&step(0.0, 1, 1)).await;
    assert_eq!(first.len(), 1);

    assert_eq!(
        first[0].sim_time_s, after.sim_time_s,
        "a zero-length step advanced the clock"
    );
    assert_eq!(
        frame_bits(&first[0]),
        frame_bits(&second[0]),
        "two consecutive zero-length steps disagree, so the first one mutated state"
    );

    // The state the pack carries, as opposed to the rates it reports, is untouched.
    assert_eq!(first[0].telemetry.soc_true, after.telemetry.soc_true);
    assert_eq!(first[0].telemetry.t_max, after.telemetry.t_max);
    assert_eq!(first[0].telemetry.v_terminal, after.telemetry.v_terminal);
}

/// The standing environment: inherited when a step omits one, overridden for a batch
/// that carries one, and not changed by that override.
///
/// Observed through cell temperature, which needs a thermally-coupled pack — the plain
/// CC scenario is isothermal, so its cells would sit at `initial_temp_k` whatever the
/// ambient, and every assertion here would be vacuous. The pack is rested and its
/// scheduled faults cleared first so the only thing moving temperature is the room.
#[tokio::test]
async fn the_standing_environment_is_inherited_overridden_per_batch_and_not_changed_by_it() {
    let (_address, _id, mut writer) = writer_on(SOFT_SHORT).await;

    match writer.round_trip(&Command::ClearFaults).await {
        Event::FaultsCleared { count } => assert_eq!(count, 2, "the scenario ships two faults"),
        other => panic!("expected FaultsCleared, got {other:?}"),
    }

    // Long enough to equilibrate: this chemistry's 95 J/K against 0.35 W/K is a time
    // constant near 270 s, and each leg below runs 6000 s.
    let rest = |t_ambient: Option<f64>| {
        Command::Step(StepCommand {
            dt: 10.0,
            n_steps: 600,
            demand: Demand::Rest,
            env: t_ambient.map(|t_ambient| Env {
                t_ambient,
                t_coolant: None,
            }),
            report_every_n_steps: 600,
        })
    };

    match writer
        .round_trip(&Command::SetEnv {
            env: Env {
                t_ambient: 330.0,
                t_coolant: None,
            },
        })
        .await
    {
        Event::EnvSet { env } => assert_eq!(env.t_ambient, 330.0),
        other => panic!("expected EnvSet, got {other:?}"),
    }

    let inherited = writer.step(&rest(None)).await;
    assert!(
        inherited[0].telemetry.t_max > 328.0,
        "a step with no env of its own did not inherit the standing one (t_max = {})",
        inherited[0].telemetry.t_max
    );

    let overridden = writer.step(&rest(Some(260.0))).await;
    assert!(
        overridden[0].telemetry.t_min < 262.0,
        "a step's own env did not override the standing one (t_min = {})",
        overridden[0].telemetry.t_min
    );

    let back = writer.step(&rest(None)).await;
    assert!(
        back[0].telemetry.t_max > 328.0,
        "the per-batch override leaked into the standing environment (t_max = {}); a \
         command's effect is scoped to the command unless the command is SetEnv",
        back[0].telemetry.t_max
    );
}

// -------------------------------------------------------------------------- faults

#[tokio::test]
async fn faults_can_be_scheduled_and_cleared_and_a_bad_one_is_refused() {
    let (_address, _id, mut writer) = writer_on(SOFT_SHORT).await;

    let good = Command::ScheduleFault {
        at_s: 30.0,
        fault: Fault::SoftInternalShort {
            s: 0,
            p: 0,
            ohms: 4.0,
        },
    };
    match writer.round_trip(&good).await {
        Event::FaultScheduled { at_s } => assert_eq!(at_s, 30.0),
        other => panic!("expected FaultScheduled, got {other:?}"),
    }

    // The engine already validates this — a cell index outside the pack, a
    // non-positive resistance, a non-finite time — so the socket translates the error
    // rather than duplicating the check and giving one condition two messages.
    let bad = Command::ScheduleFault {
        at_s: 30.0,
        fault: Fault::SoftInternalShort {
            s: 99,
            p: 0,
            ohms: 4.0,
        },
    };
    assert_eq!(
        error_code(&writer.round_trip(&bad).await),
        ErrorCode::FaultRejected
    );

    // The scenario ships two faults of its own, and the one just added makes three
    // pending.
    match writer.round_trip(&Command::ClearFaults).await {
        Event::FaultsCleared { count } => assert_eq!(count, 3),
        other => panic!("expected FaultsCleared, got {other:?}"),
    }

    // Nothing latched, so there is nothing to clear — reported honestly rather than as
    // a success.
    match writer.round_trip(&Command::ClearBmsFault).await {
        Event::BmsFaultCleared { cleared } => assert!(!cleared),
        other => panic!("expected BmsFaultCleared, got {other:?}"),
    }
}

// ------------------------------------------------------------------ session removal

/// A `DELETE` unregisters the session; without the flag it sets, an attached socket
/// would keep its own `Arc` and go on stepping a pack no route can reach.
#[tokio::test]
async fn a_deleted_session_stops_accepting_commands() {
    let (address, id, mut writer) = writer_on(CC_DISCHARGE).await;
    assert_eq!(writer.step(&step(1.0, 1, 1)).await.len(), 1);

    let (status, body) = http(address, "DELETE", &format!("/sessions/{id}"), None, "").await;
    assert_eq!(status, 204, "{body}");

    assert_eq!(
        error_code(&writer.round_trip(&step(1.0, 1, 1)).await),
        ErrorCode::NoSuchSession
    );
    assert_eq!(
        error_code(&writer.round_trip(&Command::Snapshot).await),
        ErrorCode::NoSuchSession
    );
}

// ------------------------------------------------------------------- backpressure

/// The observer half of the backpressure rule: **drop views, count the loss**.
///
/// A writer's batch frames are the experiment's record and are never dropped — they go
/// straight down its socket, so a slow writer gets TCP backpressure. An observer's are
/// a view, and one slow observer must not be able to freeze the session the writer is
/// driving.
///
/// Forcing the lag needs more bytes than the kernel's socket buffers will absorb, which
/// is why this floods a full-cap batch at an observer that is not reading. The
/// assertion is deliberately one-sided — *some* frames were lost and the loss was
/// reported — because exactly how many the buffers swallow first is a property of the
/// operating system, not of this protocol. Measured here for reassurance that it is not
/// a marginal trigger: 706 delivered, **9294 dropped**, identically across repeated
/// runs.
#[tokio::test]
async fn an_observer_that_falls_behind_is_told_how_much_it_lost() {
    const N: u64 = 10_000;

    let (address, id, mut writer) = writer_on(CC_DISCHARGE).await;
    let (mut observer, _) = Client::attach_and_greet(address, id).await;

    // The observer is not read at all while this runs. The writer must not be affected
    // by that, and the fact that this call returns at all is half the assertion.
    let written = writer.step(&step(1.0, N, 1)).await;
    assert_eq!(
        written.len() as u64,
        N,
        "a lagging observer stalled the writer"
    );

    let mut seen = 0_u64;
    let mut dropped = 0_u64;
    loop {
        match observer.next_event().await {
            Event::Telemetry(_) => seen += 1,
            Event::Dropped { count } => dropped += count,
            // The observer catches back up: the terminator is the last event of the
            // batch, and it is still there after the discards.
            Event::BatchComplete { steps, .. } => {
                assert_eq!(steps, N);
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    assert!(
        dropped > 0,
        "the observer received all {seen} frames without lagging, so this test is not \
         reaching the drop path any more — raise N"
    );
    assert_eq!(
        seen + dropped,
        N,
        "every frame is either delivered or counted as dropped; {seen} + {dropped} != {N}"
    );
}

// ------------------------------------------------------------------ wire contract

/// The JSON spellings clients match on, pinned.
///
/// Externally tagged is not a style choice here, it is forced. An internally tagged
/// enum (`#[serde(tag = "cmd")]`) — the shape a JavaScript client would prefer —
/// deserializes by buffering into serde's private `Content` type, which has no `u128`.
/// A `Snapshot` has one: `ChaCha8Rng`'s `word_pos`. So `Command::Restore` under
/// internal tagging fails at runtime with `u128 is not supported` while compiling
/// perfectly. `#[serde(flatten)]` fails the same way, which is why `Frame` nests its
/// telemetry instead of flattening it.
#[test]
fn command_and_event_wire_spellings_are_pinned() {
    use serde::{Deserialize, Serialize};

    assert_eq!(serde_json::to_string(&Command::Ping).unwrap(), r#""Ping""#);
    assert_eq!(
        serde_json::to_string(&Command::ClearFaults).unwrap(),
        r#""ClearFaults""#
    );
    assert_eq!(
        serde_json::to_string(&Command::Step(StepCommand {
            dt: 1.0,
            n_steps: 10,
            demand: Demand::Current(2.5),
            env: None,
            report_every_n_steps: 1,
        }))
        .unwrap(),
        r#"{"Step":{"dt":1.0,"n_steps":10,"demand":{"Current":2.5},"env":null,"report_every_n_steps":1}}"#
    );
    assert_eq!(serde_json::to_string(&Event::Pong).unwrap(), r#""Pong""#);
    assert_eq!(
        serde_json::to_string(&Event::Dropped { count: 3 }).unwrap(),
        r#"{"Dropped":{"count":3}}"#
    );

    // `env` and `report_every_n_steps` are optional on the way in, so the smallest
    // useful command a client can write really is this small.
    let minimal: Command =
        serde_json::from_str(r#"{"Step":{"dt":1.0,"n_steps":10,"demand":"Rest"}}"#).unwrap();
    let Command::Step(minimal) = minimal else {
        panic!("expected a Step")
    };
    assert_eq!(minimal.report_every_n_steps, 1);
    assert_eq!(minimal.env, None);

    // The measurement behind the encoding choice, kept executable so it reads as a
    // decision rather than a preference. If a future serde makes `Content` handle
    // `u128`, this test failing is the signal that the constraint has lifted.
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(tag = "cmd")]
    enum InternallyTagged {
        Restore { snapshot: Box<sim_core::Snapshot> },
    }

    let chem =
        sim_data::parse_chemistry(include_str!("../../../chemistries/lfp_26650_generic.toml"))
            .unwrap();
    let pack = sim_data::parse_scenario(CC_DISCHARGE)
        .unwrap()
        .build_pack(chem)
        .unwrap();
    let text = serde_json::to_string(&InternallyTagged::Restore {
        snapshot: Box::new(pack.snapshot()),
    })
    .unwrap();
    let err = serde_json::from_str::<InternallyTagged>(&text)
        .expect_err("internal tagging cannot carry a Snapshot");
    assert!(
        err.to_string().contains("u128"),
        "expected the u128 buffering failure, got {err}"
    );

    // The shape this protocol actually uses survives the same payload.
    let text = serde_json::to_string(&Command::Restore {
        snapshot: Box::new(pack.snapshot()),
    })
    .unwrap();
    serde_json::from_str::<Command>(&text).expect("externally tagged carries a Snapshot");
}
