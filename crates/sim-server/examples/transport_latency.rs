//! What the WebSocket costs, measured — `cargo run --release -p sim-server --example transport_latency`.
//!
//! Phase 4's protocol is built on one claim: a batch of *n* steps is one message, so
//! the socket's per-message cost amortises away and network jitter cannot reach the
//! physics. This is the number behind that claim, rather than the assertion of it.
//!
//! Three arms, on the same pack, over the same step counts:
//!
//! * **in-process** — `Pack::step` in a loop, the floor;
//! * **ws, every step reported** (`report_every_n_steps = 1`) — the socket carrying one
//!   telemetry frame per step, which is what a bit-identical comparison needs;
//! * **ws, final step only** (`report_every_n_steps = n`) — the fast-forward mode, where
//!   the reply is one frame regardless of how far the pack moved.
//!
//! The gap between the last two is the JSON, not the transport: decimation drops
//! *reports*, never steps, so both arms run the identical trajectory.
//!
//! # Why this is an example and not a bench or a test
//! It is a wall-clock measurement, so asserting on it would make a test that fails on a
//! loaded machine and passes on a quiet one. It is also not a criterion benchmark: the
//! quantity of interest is a *ratio between two transports of the same work*, which
//! survives this laptop's ~1.4× CPU bimodality (see `docs/plans/pack-step-perf.md`),
//! whereas the absolute microseconds do not. As an example it still compiles under
//! `cargo clippy --workspace --all-targets -- -D warnings`, so it cannot rot.
//!
//! Run it in `--release`. A debug build measures the engine's debug assertions — the
//! Thévenin memo's staleness check runs on every cell of every step — and would price
//! the transport as free by comparison.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use sim_core::{Demand, Env};
use sim_data::{parse_chemistry, parse_scenario, Scenario};
use sim_server::protocol::{Command, Event, StepCommand};
use sim_server::{serve, AppState};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// A 1S1P LFP pack with everything off — the topology that makes the transport look
/// *worst*, because a single cell's step is ~200 ns and there is nothing for the
/// socket's cost to hide behind.
const SCENARIO_TOML: &str = include_str!("../../../scenarios/cc_discharge_lfp.toml");
const LFP_TOML: &str = include_str!("../../../chemistries/lfp_26650_generic.toml");

const DT_S: f64 = 0.1;

/// `Rest` rather than a current, for one reason that is not about speed: the demand is
/// one match arm in a closed-form solve, so it costs the same either way — but at zero
/// current the pack barely moves, and every repetition therefore measures the same
/// state instead of a pack that has drained into its SOC clamp by repetition three.
const DEMAND: Demand = Demand::Rest;

const ENV: Env = Env {
    t_ambient: 298.15,
    t_coolant: None,
};

const STEP_COUNTS: [u64; 5] = [1, 10, 100, 1_000, 10_000];

/// Medians, not means: one scheduler hiccup in an arm that takes 30 ms otherwise moves
/// a mean by more than the effect being measured.
///
/// Fifteen rather than a handful because the small-`n` rows are two round trips of
/// ~50 µs on a Windows scheduler, and at seven repetitions the median was still noisy
/// enough to print `ws (final)` above `ws (all)` at n = 1 — two arms that are the same
/// single round trip there, so the ordering was pure jitter.
const REPS: usize = 15;

/// Topologies. 1S1P is the floor for the engine and therefore the ceiling for the
/// transport's share; 100S10P is the size `CLAUDE.md`'s performance budget is written
/// against, where a single step already costs tens of microseconds.
const TOPOLOGIES: [(u16, u16); 2] = [(1, 1), (100, 10)];

#[tokio::main]
async fn main() {
    println!(
        "transport latency — {} repetitions, median, dt = {DT_S} s, demand = {DEMAND:?}\n\
         (release build assumed; a debug build measures the engine's debug assertions)\n",
        REPS
    );

    for (series, parallel) in TOPOLOGIES {
        measure(series, parallel).await;
    }

    println!(
        "\nReading this table — and note it says something sharper than \"batching wins\":\n\
         \x20 * The **per-message round trip** does amortise, exactly as the protocol's\n\
         \x20   batch design predicts. It is the entire cost of a one-step batch and it\n\
         \x20   is invisible by n = 1000. That is the argument for sending (dt, n_steps)\n\
         \x20   rather than one message per step.\n\
         \x20 * The **per-frame cost does not amortise**, because with k = 1 there is one\n\
         \x20   frame per step. At ~8–10 µs to encode, send and decode a telemetry frame,\n\
         \x20   `ws (all)` sits ~90x above the engine at 1S1P and ~1.5x at 100S10P, and it\n\
         \x20   stays there however large n gets. **Reporting is what a socket makes\n\
         \x20   expensive, not stepping.**\n\
         \x20 * `ws (final)` is the same trajectory reported once. It is the column that\n\
         \x20   converges on the engine, and it is the measured argument for decimation\n\
         \x20   being a protocol feature rather than an optimisation: a fast-forward of\n\
         \x20   ten thousand steps costs the physics plus a few percent.\n\
         \x20 * So the knob to reach for is `report_every_n_steps`, sized to the\n\
         \x20   resolution the client will actually plot — not to the step count.\n\
         \x20 * Ratios, not absolutes, are the portable part: this machine's CPU is\n\
         \x20   bimodal by ~1.4x (see docs/plans/pack-step-perf.md), and both arms of a\n\
         \x20   ratio are measured in the same state."
    );
}

async fn measure(series: u16, parallel: u16) {
    let chem = parse_chemistry(LFP_TOML).expect("the shipped LFP chemistry parses");
    let scenario = scenario(series, parallel);

    // Both arms build their pack from the same `Scenario`, so any difference is the
    // transport rather than two spellings of the same configuration.
    let mut local = scenario
        .clone()
        .build_pack(chem)
        .expect("the scenario builds");

    let state = AppState::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../chemistries"));
    let (id, _) = state
        .create_session(scenario)
        .await
        .expect("the scenario builds on the server too");
    let address = spawn(state).await;
    let mut socket = attach(address, id.0).await;

    // Warm both arms before timing anything. The engine memoises each cell's Thévenin
    // source across the step boundary, so a pack's *first* step is the one step that
    // cannot hit the memo — and `docs/plans/pack-step-perf.md` records that exact
    // oversight pricing an optimisation at zero. One TCP round trip is also enough to
    // get the connection past whatever the first write pays.
    local.step(DT_S, DEMAND, &ENV);
    ws_step(&mut socket, 1, 1).await;

    println!("{series}S{parallel}P");
    println!(
        "  {:>8}  {:>12}  {:>12}  {:>12}  {:>11}  {:>11}",
        "n_steps", "in-process", "ws (all)", "ws (final)", "all/local", "final/local"
    );

    for n in STEP_COUNTS {
        let in_process = median(REPS, || {
            let start = Instant::now();
            for _ in 0..n {
                local.step(DT_S, DEMAND, &ENV);
            }
            start.elapsed()
        });

        let ws_all = median_ws(&mut socket, REPS, n, 1).await;
        let ws_final = median_ws(&mut socket, REPS, n, n).await;

        let local_s = in_process.as_secs_f64().max(f64::MIN_POSITIVE);
        println!(
            "  {:>8}  {:>12}  {:>12}  {:>12}  {:>10.1}x  {:>10.1}x",
            n,
            time(in_process),
            time(ws_all),
            time(ws_final),
            ws_all.as_secs_f64() / local_s,
            ws_final.as_secs_f64() / local_s,
        );

        // The two derived costs, printed on the row they were derived from so nobody
        // has to re-do the arithmetic from the table.
        if n == 1 {
            println!(
                "           ^ one round trip: {} per message, which is the entire cost \
                 of a one-step batch.\n\
                 \x20            The two ws arms are the *same* single round trip on this \
                 row — one step\n\
                 \x20            reports one frame whatever k is — so which of them prints \
                 lower is\n\
                 \x20            scheduler noise, not a result.",
                us(ws_all.saturating_sub(in_process))
            );
        }
        if n == *STEP_COUNTS.last().expect("a non-empty sweep") {
            let per_frame = ws_all.saturating_sub(ws_final).as_secs_f64() / (n - 1) as f64;
            println!(
                "           ^ per reported frame: {:.1} µs of JSON encode, send and \
                 decode",
                per_frame * 1e6
            );
        }
    }
    println!();
}

/// The shipped 1S1P scenario, resized.
///
/// Editing the parsed `PackConfig` rather than shipping a second scenario file: the
/// topology is the only thing that varies here, and a 100S10P file would be a second
/// place for every other knob to drift.
fn scenario(series: u16, parallel: u16) -> Scenario {
    let mut scenario = parse_scenario(SCENARIO_TOML).expect("the shipped scenario parses");
    scenario.pack.series = series;
    scenario.pack.parallel = parallel;
    scenario
}

async fn spawn(state: AppState) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("an ephemeral port");
    let address = listener.local_addr().expect("the bound address");
    tokio::spawn(async move {
        let _ = serve(listener, state).await;
    });
    address
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Attach and swallow the hello frame.
async fn attach(address: SocketAddr, id: u64) -> Socket {
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{address}/sessions/{id}/ws"))
            .await
            .expect("the upgrade succeeds");
    let hello = next_event(&mut socket).await;
    assert!(
        matches!(hello, Event::Hello { .. }),
        "the first frame on every socket is Hello, got {hello:?}"
    );
    socket
}

/// One `Step` command and its whole reply, timed from before the send to after
/// `BatchComplete` — which is the barrier that means every frame has arrived.
async fn ws_step(socket: &mut Socket, n_steps: u64, report_every_n_steps: u64) -> Duration {
    let command = Command::Step(StepCommand {
        dt: DT_S,
        n_steps,
        demand: DEMAND,
        env: Some(ENV),
        report_every_n_steps,
    });
    let text = serde_json::to_string(&command).expect("commands serialize");

    let start = Instant::now();
    socket
        .send(Message::text(text))
        .await
        .expect("the socket accepts the command");
    loop {
        match next_event(socket).await {
            Event::Telemetry(_) => {}
            Event::BatchComplete { steps, .. } => {
                assert_eq!(
                    steps, n_steps,
                    "the batch did not take the steps it was asked for"
                );
                return start.elapsed();
            }
            other => panic!("expected telemetry or a batch terminator, got {other:?}"),
        }
    }
}

async fn next_event(socket: &mut Socket) -> Event {
    loop {
        let message = socket
            .next()
            .await
            .expect("the socket is still open")
            .expect("a readable frame");
        match message {
            Message::Text(text) => {
                return serde_json::from_str(&text).expect("the server sends Events")
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected frame from the server: {other:?}"),
        }
    }
}

fn median(reps: usize, mut run: impl FnMut() -> Duration) -> Duration {
    let mut samples: Vec<Duration> = (0..reps).map(|_| run()).collect();
    samples.sort_unstable();
    samples[reps / 2]
}

/// The socket's half of [`median`], written out rather than generic: a closure that
/// hands back a future borrowing the socket it captured is a lending closure, which
/// Rust does not have.
async fn median_ws(socket: &mut Socket, reps: usize, n_steps: u64, k: u64) -> Duration {
    let mut samples = Vec::with_capacity(reps);
    for _ in 0..reps {
        samples.push(ws_step(socket, n_steps, k).await);
    }
    samples.sort_unstable();
    samples[reps / 2]
}

/// Microseconds below a millisecond, milliseconds above it. A fixed `ms` format prints
/// the 1S1P single-step arm as `0.000 ms`, which is a measurement pretending to be a
/// zero.
fn time(d: Duration) -> String {
    let s = d.as_secs_f64();
    if s < 1e-3 {
        format!("{:.1} µs", s * 1e6)
    } else {
        format!("{:.3} ms", s * 1e3)
    }
}

fn us(d: Duration) -> String {
    format!("{:.0} µs", d.as_secs_f64() * 1e6)
}
