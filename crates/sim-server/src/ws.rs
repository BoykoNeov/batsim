//! The socket: role assignment, the command loop, and the batch that runs off the
//! async runtime.
//!
//! # Two loops, on purpose
//! A writer's socket is strictly request/response, so [`writer_loop`] reads a command,
//! answers it completely, and only then reads again. An observer's socket is pushed
//! events it did not ask for while still being watched for a close, so
//! [`observer_loop`] splits it and selects. The asymmetry is not incidental — it is
//! what makes the protocol's two backpressure rules fall out of the structure instead
//! of being enforced by bookkeeping:
//!
//! * **The writer's frames are never dropped.** They go straight down its socket, so a
//!   writer that reads slowly gets TCP backpressure and a slow batch, not a hole in its
//!   data. A batch's reports are the experiment's record.
//! * **An observer's frames may be dropped, and the loss is counted.** It reads from a
//!   bounded broadcast channel; falling behind produces [`Event::Dropped`] rather than
//!   a stall, so one slow observer cannot freeze the session the writer is driving. An
//!   observer's frames are a view.
//!
//! # Commands are never reordered, because the socket is not read during a batch
//! While a batch is running, `writer_loop` is awaiting the stepping task and is *not*
//! reading its socket. That looks like an omission and is the mechanism: further
//! commands sit in the kernel's receive buffer, arrive in order, and the client feels
//! natural backpressure. Selecting on the socket concurrently would mean either
//! queueing commands by hand or interleaving them into a batch, and interleaving is the
//! thing the one-writer rule exists to prevent.
//!
//! # Why the batch is `spawn_blocking`
//! A million steps is tens of seconds of CPU. Running that inline would block a tokio
//! worker; `block_in_place` would work but panics on a current-thread runtime, which
//! would silently oblige every `#[tokio::test]` in this crate to carry
//! `flavor = "multi_thread"` forever. `spawn_blocking` needs a `'static` closure, which
//! is exactly why the session is an `Arc<Mutex<_>>` and why the guard is taken *inside*
//! the closure with `blocking_lock`.

use std::sync::Arc;

use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use futures_util::{SinkExt, StreamExt};
use sim_core::{Pack, SNAPSHOT_VERSION};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};

use crate::error::{ApiError, ErrorCode};
use crate::protocol::{
    check_env, frame_count, Command, Event, Frame, Limits, PackFacts, Role, StepCommand,
};
use crate::session::{check_restore_fits, Session};
use crate::API_VERSION;

/// Attach a socket to a session and run it until it closes.
///
/// Claims the writer slot if it is free, and releases it on the way out. Release
/// happens on the normal return path rather than in a `Drop` guard because freeing it
/// needs the session's async lock, which a destructor cannot await; every way
/// [`writer_loop`] ends is a return. The uncovered case is the whole task being dropped
/// mid-await, which happens when the process is going away anyway.
pub async fn handle(socket: WebSocket, session: Arc<Mutex<Session>>, limits: Limits) {
    // Everything the hello frame reports, plus the observer subscription, is taken
    // under one lock: subscribing in the same critical section as the snapshot of
    // state is what guarantees an observer cannot miss an event that happened between
    // the two.
    let (role, hello, observers, subscription) = {
        let mut session = session.lock().await;
        if session.deleted {
            return;
        }
        let role = if session.has_writer {
            Role::Observer
        } else {
            session.has_writer = true;
            Role::Writer
        };
        let hello = Event::Hello {
            api_version: API_VERSION,
            snapshot_version: SNAPSHOT_VERSION,
            session_id: session.id.0,
            role,
            pack: PackFacts::of(&session.pack),
            env: session.env,
            limits,
        };
        (
            role,
            hello,
            session.observers.clone(),
            session.observers.subscribe(),
        )
    };

    match role {
        Role::Writer => {
            tracing::info!(role = "writer", "socket attached");
            writer_loop(socket, &session, limits, &hello, &observers).await;
            // Whatever ended the loop — close frame, transport error, or a deleted
            // session — the slot goes back so the next socket to attach can write.
            session.lock().await.has_writer = false;
            tracing::info!(role = "writer", "socket detached; writer slot freed");
        }
        Role::Observer => {
            tracing::info!(role = "observer", "socket attached");
            observer_loop(socket, &hello, subscription).await;
        }
    }
}

/// Serialize an event once, so the writer's copy and every observer's copy are the
/// same bytes.
fn encode(event: &Event) -> Result<Arc<str>, ApiError> {
    serde_json::to_string(event)
        .map(|text| Arc::from(text.as_str()))
        .map_err(|e| ApiError::ws(ErrorCode::Internal, format!("event not serializable: {e}")))
}

/// Whether observers should see an event the writer's command produced.
///
/// The excluded ones are all *replies*, addressed to the socket that asked:
/// `Hello` is per-socket by construction; `Snapshot` is large and private to whoever
/// requested it; `Pong` and `Error` answer a specific message. Everything that changes
/// the pack — telemetry, batch boundaries, restores, fault edits, the standing
/// environment — is a fact about the session and goes out to everyone watching it.
fn observable(event: &Event) -> bool {
    match event {
        Event::Telemetry(_)
        | Event::BatchComplete { .. }
        | Event::EnvSet { .. }
        | Event::Restored { .. }
        | Event::FaultScheduled { .. }
        | Event::FaultsCleared { .. }
        | Event::BmsFaultCleared { .. } => true,
        Event::Hello { .. }
        | Event::Snapshot { .. }
        | Event::Pong
        | Event::Dropped { .. }
        | Event::Error { .. } => false,
    }
}

/// The writer's loop: one command in, its complete answer out, repeat.
async fn writer_loop(
    mut socket: WebSocket,
    session: &Arc<Mutex<Session>>,
    limits: Limits,
    hello: &Event,
    observers: &broadcast::Sender<Arc<str>>,
) {
    if send_one(&mut socket, hello).await.is_err() {
        return;
    }

    // Not `select!`ing on anything: see this module's note on ordering. While
    // `dispatch` is awaited below, this socket is deliberately not read.
    while let Some(incoming) = socket.recv().await {
        let text = match incoming {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) | Err(_) => return,
            // axum answers protocol-level pings itself; ours is a `Command`.
            Ok(Message::Ping(_) | Message::Pong(_)) => continue,
            Ok(Message::Binary(_)) => {
                let error = Event::Error {
                    code: ErrorCode::InvalidCommand,
                    message: "commands are JSON text frames; this server sends and \
                              expects no binary frames"
                        .to_owned(),
                };
                if send_one(&mut socket, &error).await.is_err() {
                    return;
                }
                continue;
            }
        };

        let events = match parse_command(&text) {
            Ok(command) => dispatch(session, command, &limits).await,
            Err(e) => Err(e),
        };

        let events = match events {
            Ok(events) => events,
            Err(e) => vec![Event::Error {
                code: e.code(),
                message: e.message().to_owned(),
            }],
        };

        for event in &events {
            let Ok(encoded) = encode(event) else {
                // Nothing useful to say on a socket if the thing we cannot encode is
                // the message; drop the connection instead of lying about success.
                return;
            };
            if observable(event) {
                // Lag is the observer's problem by design, and "no subscribers" is the
                // normal case — neither is a failure of the writer's command.
                let _ = observers.send(Arc::clone(&encoded));
            }
            if socket
                .send(Message::Text(Utf8Bytes::from(&*encoded)))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

/// The observer's loop: pushed events out, close-and-`Ping` in.
async fn observer_loop(
    socket: WebSocket,
    hello: &Event,
    mut subscription: broadcast::Receiver<Arc<str>>,
) {
    let (mut outgoing, mut incoming) = socket.split();

    let Ok(encoded) = encode(hello) else { return };
    if outgoing
        .send(Message::Text(Utf8Bytes::from(&*encoded)))
        .await
        .is_err()
    {
        return;
    }

    loop {
        // Both arms are cancel-safe: `StreamExt::next` and `broadcast::Receiver::recv`
        // each either complete or leave nothing half-consumed, so losing the race does
        // not lose a message.
        let reply = tokio::select! {
            message = incoming.next() => match message {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(Message::Text(text))) => Some(observer_reply(&text)),
                Some(Ok(_)) => None,
            },
            event = subscription.recv() => match event {
                Ok(encoded) => {
                    if outgoing.send(Message::Text(Utf8Bytes::from(&*encoded))).await.is_err() {
                        return;
                    }
                    None
                }
                // The session outlived this socket's usefulness — it was dropped, or
                // every sender went away.
                Err(RecvError::Closed) => return,
                // The honest admission: this socket fell behind and the server threw
                // frames away rather than stall the writer. Reported so a plot can
                // draw the gap instead of a smooth line through missing data.
                Err(RecvError::Lagged(count)) => Some(Event::Dropped { count }),
            },
        };

        if let Some(reply) = reply {
            let Ok(encoded) = encode(&reply) else { return };
            if outgoing
                .send(Message::Text(Utf8Bytes::from(&*encoded)))
                .await
                .is_err()
            {
                return;
            }
        }
    }
}

/// What an observer gets for sending something.
///
/// `Ping` is allowed because it is the one command that does not touch the pack, and a
/// read-only client still needs a way to check the socket is alive. Everything else is
/// refused — that is what read-only means.
fn observer_reply(text: &str) -> Event {
    match parse_command(text) {
        Ok(Command::Ping) => Event::Pong,
        Ok(_) => Event::Error {
            code: ErrorCode::NotWriter,
            message: "this session already has a writer, so this socket is a read-only \
                      observer; it receives the telemetry stream but cannot command the \
                      pack. Reconnect after the writer disconnects to take the slot."
                .to_owned(),
        },
        Err(e) => Event::Error {
            code: e.code(),
            message: e.message().to_owned(),
        },
    }
}

/// Parse a command, with an error a person can act on.
fn parse_command(text: &str) -> Result<Command, ApiError> {
    serde_json::from_str(text).map_err(|e| {
        ApiError::ws(
            ErrorCode::InvalidCommand,
            format!(
                "not a command: {e}. Commands are externally tagged, e.g. \
                 {{\"Step\":{{\"dt\":1.0,\"n_steps\":10,\"demand\":{{\"Current\":2.5}}}}}} \
                 or the string \"Ping\""
            ),
        )
    })
}

/// Send a single event, ignoring observers. Used for the hello frame only.
async fn send_one(socket: &mut WebSocket, event: &Event) -> Result<(), ()> {
    let encoded = encode(event).map_err(|_| ())?;
    socket
        .send(Message::Text(Utf8Bytes::from(&*encoded)))
        .await
        .map_err(|_| ())
}

/// A deleted session accepts nothing further.
///
/// `DELETE /sessions/{id}` only unregisters; this is what stops an attached socket from
/// going on stepping a pack that no route can reach.
fn alive(session: &Session) -> Result<(), ApiError> {
    if session.deleted {
        Err(ApiError::no_such_session(session.id))
    } else {
        Ok(())
    }
}

/// Run one command and produce everything the client should see for it.
///
/// Returns a `Vec` rather than sending, so that the caller owns the "serialize once,
/// send to the writer, publish to observers" pass and neither policy is duplicated per
/// command.
async fn dispatch(
    session: &Arc<Mutex<Session>>,
    command: Command,
    limits: &Limits,
) -> Result<Vec<Event>, ApiError> {
    match command {
        Command::Ping => Ok(vec![Event::Pong]),

        Command::Step(step) => {
            // Validate before spawning anything: rejecting a million-step command
            // should not first cost a task.
            step.validate(limits)?;
            run_batch(Arc::clone(session), step).await
        }

        Command::SetEnv { env } => {
            check_env(env)?;
            let mut session = session.lock().await;
            alive(&session)?;
            session.env = env;
            Ok(vec![Event::EnvSet { env }])
        }

        Command::ScheduleFault { at_s, fault } => {
            let mut session = session.lock().await;
            alive(&session)?;
            // The engine already rejects a non-finite time, a non-positive short
            // resistance, and an out-of-topology index, so this translates rather than
            // duplicates. Duplicating it would give one condition two messages that
            // drift apart.
            session
                .pack
                .schedule_fault(at_s, fault)
                .map_err(|e| ApiError::ws(ErrorCode::FaultRejected, e.to_string()))?;
            Ok(vec![Event::FaultScheduled { at_s }])
        }

        Command::ClearFaults => {
            let mut session = session.lock().await;
            alive(&session)?;
            let count = session.pack.clear_faults();
            Ok(vec![Event::FaultsCleared { count }])
        }

        Command::ClearBmsFault => {
            let mut session = session.lock().await;
            alive(&session)?;
            let cleared = session.pack.clear_bms_fault();
            Ok(vec![Event::BmsFaultCleared { cleared }])
        }

        Command::Snapshot => {
            let session = session.lock().await;
            alive(&session)?;
            Ok(vec![Event::Snapshot {
                snapshot: Box::new(session.pack.snapshot()),
            }])
        }

        Command::Restore { snapshot } => {
            // Build the replacement before taking the lock: a snapshot this binary
            // cannot read should not have held up anyone else's session.
            let restored = Pack::restore(&snapshot).map_err(|e| {
                ApiError::ws(
                    ErrorCode::BadSnapshot,
                    format!("{e} — this binary produces and consumes version {SNAPSHOT_VERSION}"),
                )
            })?;
            let mut session = session.lock().await;
            alive(&session)?;
            check_restore_fits(&session.pack, &restored)?;
            session.pack = restored;
            // The stored frame described the pack that was just discarded, and a stale
            // frame beside a restored pack is worse than no frame.
            session.latest = None;
            Ok(vec![Event::Restored {
                pack: PackFacts::of(&session.pack),
            }])
        }
    }
}

/// Step the pack, off the async runtime, and collect the frames the client asked for.
///
/// The session lock is held for the whole batch and released before any of it is sent:
/// stepping is CPU-bound and sending is I/O-bound, and holding a lock across the second
/// would let a slow reader freeze REST requests and observers for as long as it liked.
/// The price is holding [`Limits::max_frames_per_reply`] frames in memory, which is the
/// cap's second job.
async fn run_batch(
    session: Arc<Mutex<Session>>,
    step: StepCommand,
) -> Result<Vec<Event>, ApiError> {
    tokio::task::spawn_blocking(move || {
        // Not an async lock: this closure is on a blocking thread, which is precisely
        // where `blocking_lock` is allowed.
        let mut session = session.blocking_lock();
        alive(&session)?;

        let env = step.env.unwrap_or(session.env);
        let k = step.report_every_n_steps;
        let n = step.n_steps;
        let reported = frame_count(n, k);

        // Exactly the frames validation already proved will fit, plus the terminator.
        let mut events = Vec::with_capacity(usize::try_from(reported).unwrap_or(0) + 1);
        for i in 1..=n {
            let telemetry = session.pack.step(step.dt, step.demand, &env);
            session.latest = Some(telemetry);
            if i % k == 0 || i == n {
                events.push(Event::Telemetry(Frame {
                    step: i,
                    // Read *after* the step, so a frame describes the pack at the end
                    // of its own step. Off by one here is how off-by-one-step plots
                    // happen, and nothing downstream could notice.
                    sim_time_s: session.pack.sim_time_s(),
                    telemetry,
                }));
            }
        }
        events.push(Event::BatchComplete {
            steps: n,
            reported,
            sim_time_s: session.pack.sim_time_s(),
        });
        Ok(events)
    })
    .await
    .unwrap_or_else(|e| {
        // `Pack::step` promises not to panic, so this is either that promise broken or
        // the runtime shutting down. Either way the session's state is now unknown to
        // this socket, and saying so is better than a silent empty batch.
        Err(ApiError::ws(
            ErrorCode::Internal,
            format!("the stepping task did not complete ({e}); this session's state is unknown"),
        ))
    })
}
