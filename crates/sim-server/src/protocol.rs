//! The WebSocket vocabulary: what a client may say, what the server says back, and
//! the boundary checks that stand between an untrusted socket and a permissive engine.
//!
//! # Why every enum here is externally tagged
//! [`Command`] and [`Event`] serialize the serde default way — `{"Step": {…}}`,
//! `"ClearFaults"` — matching how the engine's own enums already cross the wire
//! (`{"Current": -5.0}`, `"Rest"`). That is the encoding
//! `docs/plans/phase-4-server-wasm.md` chose for consistency, and it turns out to be
//! **forced** rather than merely tidy.
//!
//! An internally tagged enum (`#[serde(tag = "cmd")]`) — the shape a JavaScript client
//! would prefer — deserializes by buffering the whole value into serde's private
//! `Content` type first. `Content` has no `u128`, and a [`Snapshot`] contains one:
//! `ChaCha8Rng`'s `word_pos`. So `Command::Restore` under internal tagging fails at
//! runtime with `u128 is not supported` while compiling perfectly. Measured, along
//! with the other candidate shapes:
//!
//! ```text
//! direct                     round-trips exactly
//! externally tagged          round-trips exactly
//! adjacently tagged          round-trips exactly
//! via serde_json::Value      round-trips exactly
//! internally tagged          FAILS: u128 is not supported
//! #[serde(flatten)]          FAILS: u128 is not supported
//! ```
//!
//! The same trap is why [`Frame`] nests its telemetry in a field instead of flattening
//! it, even though `Telemetry` alone would survive: establishing `flatten` as the house
//! style here would break the first time someone flattened something with a snapshot in
//! it. Note also that `serde_json::Value` *is* exact — the `float_roundtrip` feature
//! reaches `deserialize_any`, so buffering costs correctness only where it costs
//! compilation-silent breakage.
//!
//! # Why validation lives here and not in the engine
//! `Pack::step` promises never to panic. It does not promise that a `NaN` demand will
//! not propagate through every cell and poison the session forever, and making it
//! defensive would cost a branch per field on the hot path to guard a hazard that only
//! exists at a socket. So the socket rejects and the engine keeps its contract.
//!
//! Note the asymmetry the engine already has, because it decides which commands need
//! work here: `Pack::schedule_fault` **does** validate, so [`Command::ScheduleFault`]
//! leans on it and only translates the error. `Pack::step` does not, so
//! [`StepCommand::validate`] is where the real boundary is.

use serde::{Deserialize, Serialize};
use sim_core::{Demand, Env, Fault, NonFinite, Pack, Snapshot, Telemetry};

use crate::error::{ApiError, ErrorCode};

/// Server policy on how much work one message may ask for.
///
/// Both caps are individually reasonable and jointly a footgun, which is why they are
/// checked separately and reported to the client in the hello frame: a million steps
/// at the default `report_every_n_steps = 1` is a million-frame reply, and the
/// "a batch delivers all of its frames or the session errors" rule would then oblige
/// the server to actually send it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Limits {
    /// Most steps one [`Command::Step`] may advance the pack.
    ///
    /// Bounds how long a session's lock is held, and therefore how long that session
    /// is unresponsive to REST and to its observers. It does **not** bound the total
    /// experiment — a client fast-forwards by sending more messages.
    pub max_steps_per_command: u64,
    /// Most telemetry frames one [`Command::Step`] may produce.
    ///
    /// Two jobs, and only the first is obvious: it protects the client from a reply it
    /// did not mean to ask for, *and* it bounds server memory, because a batch's frames
    /// are collected under the session lock and only sent once the lock is released.
    /// Raising this raises both costs.
    pub max_frames_per_reply: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_steps_per_command: 1_000_000,
            max_frames_per_reply: 10_000,
        }
    }
}

/// How many telemetry frames a batch of `n_steps` decimated by `k` will produce.
///
/// Steps are numbered `1..=n_steps`; a step is reported when `i % k == 0` **or** it is
/// the last step of the batch. That final-step rule is what makes this `div_ceil`
/// rather than a plain division, and it is the reason a client's last sample is the
/// true end state rather than wherever the modulus happened to land.
#[must_use]
pub fn frame_count(n_steps: u64, report_every_n_steps: u64) -> u64 {
    n_steps.div_ceil(report_every_n_steps)
}

/// Advance the pack.
///
/// `dt` is **always explicit and always the client's**. The server never derives it
/// from message arrival times; that is the one way network jitter could enter a
/// trajectory, and `CLAUDE.md` forbids it outright.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepCommand {
    /// Timestep \[s\]. Finite and `>= 0`.
    ///
    /// Zero is deliberately allowed: the engine has a pinned zero-length-step contract
    /// (stepping by `dt = 0` does not mutate state), so `{dt: 0, n_steps: 1}` is how a
    /// client reads telemetry without advancing — which is exactly what a page wants on
    /// connect. `n_steps` bounds the loop, so `dt = 0` cannot hang anything.
    pub dt: f64,
    /// How many steps to take. At least 1, at most [`Limits::max_steps_per_command`].
    pub n_steps: u64,
    /// What to ask of the pack, held constant for the whole batch.
    pub demand: Demand,
    /// Environment for this batch only.
    ///
    /// `None` uses the session's standing environment (see [`Command::SetEnv`]). A
    /// value here overrides it **for this batch and does not persist** — a command's
    /// effect is scoped to the command unless the command is `SetEnv`.
    #[serde(default)]
    pub env: Option<Env>,
    /// Report every `k`-th step, plus always the final one. Default 1 (report all).
    ///
    /// Decimation drops *reports*, never steps: the trajectory is bit-identical either
    /// way, only the sampling of it changes. That is what makes it safe as a default-on
    /// protocol feature.
    #[serde(default = "one")]
    pub report_every_n_steps: u64,
}

fn one() -> u64 {
    1
}

impl StepCommand {
    /// Reject anything the engine would take without complaint and regret.
    ///
    /// # Errors
    /// [`ErrorCode::OutOfRange`] for a non-finite or negative `dt`, a zero `n_steps`, a
    /// zero `report_every_n_steps`, or a non-finite number inside the demand or the
    /// environment; [`ErrorCode::TooManySteps`] and [`ErrorCode::TooManyFrames`] for
    /// the two caps. The frame cap is rejected rather than truncated, and its message
    /// names the knob that fixes it.
    pub fn validate(&self, limits: &Limits) -> Result<(), ApiError> {
        if !self.dt.is_finite() || self.dt < 0.0 {
            return Err(ApiError::ws(
                ErrorCode::OutOfRange,
                format!("dt must be finite and >= 0, got {}", self.dt),
            ));
        }
        if self.n_steps == 0 {
            return Err(ApiError::ws(
                ErrorCode::OutOfRange,
                "n_steps must be >= 1; to read telemetry without advancing, send \
                 n_steps = 1 with dt = 0",
            ));
        }
        if self.n_steps > limits.max_steps_per_command {
            return Err(ApiError::ws(
                ErrorCode::TooManySteps,
                format!(
                    "asked for {} steps, cap is {}; split the run across messages \
                     (the trajectory is identical either way)",
                    self.n_steps, limits.max_steps_per_command
                ),
            ));
        }
        if self.report_every_n_steps == 0 {
            return Err(ApiError::ws(
                ErrorCode::OutOfRange,
                "report_every_n_steps must be >= 1",
            ));
        }
        let frames = frame_count(self.n_steps, self.report_every_n_steps);
        if frames > limits.max_frames_per_reply {
            return Err(ApiError::ws(
                ErrorCode::TooManyFrames,
                format!(
                    "asked for {frames} frames, cap is {}; raise report_every_n_steps \
                     (currently {}) — decimation drops reports, not steps, so the \
                     trajectory is unaffected",
                    limits.max_frames_per_reply, self.report_every_n_steps
                ),
            ));
        }
        check_demand(self.demand)?;
        if let Some(env) = self.env {
            check_env(env)?;
        }
        Ok(())
    }
}

/// Every `f64` a demand can carry must be finite.
///
/// The rule itself is `sim-core`'s — see [`Demand::check_finite`] for why it lives on the
/// engine's type rather than here or in a shared adapter crate. This function is the
/// mapping into *this* protocol's error taxonomy, and that mapping is the only part that
/// is the server's business.
fn check_demand(demand: Demand) -> Result<(), ApiError> {
    demand.check_finite().map_err(non_finite)
}

/// Every `f64` an environment can carry must be finite.
///
/// # Errors
/// [`ErrorCode::OutOfRange`] naming the offending field.
pub(crate) fn check_env(env: Env) -> Result<(), ApiError> {
    env.check_finite().map_err(non_finite)
}

/// The one place a [`NonFinite`] becomes a protocol error.
///
/// `ErrorCode::OutOfRange` is a **wire contract** and does not change; the message text
/// is not, and now comes from the engine so all three clients say the same thing about
/// the same field.
fn non_finite(error: NonFinite) -> ApiError {
    ApiError::ws(ErrorCode::OutOfRange, error.to_string())
}

/// What a client may send.
///
/// A thin skin over the engine's existing mutating surface — `step`, `schedule_fault`,
/// `clear_faults`, `clear_bms_fault`, `snapshot`, `restore` — plus the two pieces of
/// session state the engine does not own (the standing environment, and liveness).
/// Nothing here needed a new engine method.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Command {
    /// Advance the pack. See [`StepCommand`].
    Step(StepCommand),
    /// Replace the session's standing environment, used by any [`Command::Step`] that
    /// omits its own.
    SetEnv {
        /// The new standing environment.
        env: Env,
    },
    /// Queue a fault to fire at a simulation time.
    ///
    /// Validation is the engine's: `Pack::schedule_fault` already rejects a non-finite
    /// `at_s`, a non-positive short resistance, and an out-of-topology cell index, so
    /// this arm translates rather than duplicates.
    ScheduleFault {
        /// Simulation time at which the fault fires \[s\].
        at_s: f64,
        /// The fault itself.
        fault: Fault,
    },
    /// Drop every fault that has not fired yet. Already-active faults stay active.
    ClearFaults,
    /// Clear a latched BMS fault, closing the contactor if the pack is fit to close it.
    ClearBmsFault,
    /// Reply with the whole engine state as JSON.
    Snapshot,
    /// Replace the pack with a snapshot, in place.
    ///
    /// Boxed because a `Snapshot` dwarfs every other variant, and an unboxed one would
    /// make every `Command` — including a `Ping` — that size.
    Restore {
        /// The pack to install. Must have the same topology as the one it replaces.
        snapshot: Box<Snapshot>,
    },
    /// Liveness check. The only command an observer may send, because it is the only
    /// one that does not touch the pack.
    Ping,
}

/// The role a socket was accepted in.
///
/// A session has at most one **writer**. Two sockets stepping one pack would make the
/// trajectory depend on network arrival order, and that does not fail loudly — it fails
/// the next time someone opens a second browser tab and cannot reproduce yesterday's
/// curve.
///
/// A role is announced once, in the hello frame, and never changes for the life of the
/// socket. When a writer disconnects the slot is freed and the *next* socket to attach
/// becomes the writer; existing observers are not promoted, because a client that was
/// told it is read-only should not silently acquire the ability to move the pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// May send every command.
    Writer,
    /// Receives the telemetry stream; every command except `Ping` is rejected with
    /// [`ErrorCode::NotWriter`].
    Observer,
}

/// One reported step.
///
/// `sim_time_s` is on the frame, not inferred by the client, because a client that has
/// to integrate `dt` itself to know where it is will eventually be off by one step and
/// draw it. It is read **after** the step that produced the frame, so a frame describes
/// the pack at the end of its step.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Frame {
    /// 1-based index of this step **within its batch**. With decimation on, the gaps in
    /// this sequence are exactly the steps that were not reported.
    pub step: u64,
    /// Simulation time at the end of this step \[s\]. Absolute, not batch-relative.
    pub sim_time_s: f64,
    /// The step's telemetry. Nested rather than flattened — see this module's note on
    /// `Content` buffering.
    pub telemetry: Telemetry,
}

/// Live facts about a pack, read from the pack rather than from the scenario that
/// created it (a restore can replace one under the other).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PackFacts {
    /// Series elements.
    pub series: u16,
    /// Parallel cells per group.
    pub parallel: u16,
    /// Simulation time elapsed \[s\].
    pub sim_time_s: f64,
}

impl PackFacts {
    /// Read the live facts off a pack.
    #[must_use]
    pub fn of(pack: &Pack) -> Self {
        Self {
            series: pack.series(),
            parallel: pack.parallel(),
            sim_time_s: pack.sim_time_s(),
        }
    }
}

/// What the server says.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    /// First frame on every socket, before anything else.
    Hello {
        /// Version of this HTTP/WebSocket contract.
        api_version: u32,
        /// Version of the engine's saved pack layout. Independent of `api_version`:
        /// two numbers, two jobs.
        snapshot_version: u32,
        /// Which session this socket is attached to.
        session_id: u64,
        /// Whether this socket may command the pack.
        role: Role,
        /// The pack as it stands right now.
        pack: PackFacts,
        /// The session's standing environment.
        env: Env,
        /// The caps this server will enforce, so a client can size its batches instead
        /// of discovering them by being rejected.
        limits: Limits,
    },
    /// One reported step of a batch.
    Telemetry(Frame),
    /// Sent after the last frame of a batch, and only then.
    ///
    /// This is the barrier a client waits on: every frame the batch will produce has
    /// already been sent when this arrives.
    BatchComplete {
        /// Steps actually taken (always the `n_steps` that was asked for — decimation
        /// never drops steps).
        steps: u64,
        /// Frames sent for this batch.
        reported: u64,
        /// Simulation time after the batch \[s\].
        sim_time_s: f64,
    },
    /// The standing environment was replaced.
    EnvSet {
        /// The new standing environment.
        env: Env,
    },
    /// The whole engine state, in reply to [`Command::Snapshot`].
    Snapshot {
        /// The pack state. Exact: the workspace's `serde_json` carries
        /// `float_roundtrip`, without which this would drift by one ULP on restore.
        snapshot: Box<Snapshot>,
    },
    /// A snapshot was installed, in reply to [`Command::Restore`].
    Restored {
        /// The pack as it stands after the restore.
        pack: PackFacts,
    },
    /// A fault was queued.
    FaultScheduled {
        /// When it will fire \[s\].
        at_s: f64,
    },
    /// Pending faults were dropped.
    FaultsCleared {
        /// How many were dropped.
        count: usize,
    },
    /// A latched BMS fault was cleared, or there was none to clear.
    BmsFaultCleared {
        /// Whether a latched fault was actually cleared.
        cleared: bool,
    },
    /// Reply to [`Command::Ping`].
    Pong,
    /// This socket could not keep up and the server discarded frames for it.
    ///
    /// Observers only, and it exists so a plot can show the gap honestly instead of
    /// drawing a smooth line through missing data. A writer never sees this: its
    /// batch replies are the experiment's record, and "best effort" is right for a
    /// live view and wrong for a result.
    Dropped {
        /// Frames lost since the last event this socket received.
        count: u64,
    },
    /// Something was refused. Same code vocabulary as the REST surface, on purpose —
    /// a client that learned these over HTTP does not learn a second set here.
    Error {
        /// Machine-readable kind.
        code: ErrorCode,
        /// Sentence for a person.
        message: String,
    },
}
