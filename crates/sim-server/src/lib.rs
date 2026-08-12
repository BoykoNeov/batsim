//! `sim-server` — a headless HTTP adapter over `sim-core`.
//!
//! One of several clients of the engine, not a layer above it. The engine is pure and
//! knows nothing about this crate; everything here is transport, session bookkeeping,
//! and the validation that a pure engine deliberately does not do (see
//! [`session::AppState::create_session`]).
//!
//! # Two version numbers
//! [`API_VERSION`] versions *this* contract — route shapes, error codes, and the JSON
//! field names of the engine types that cross the wire. [`sim_core::SNAPSHOT_VERSION`]
//! versions the engine's saved pack layout. They are independent, and both are
//! reported at `GET /`.
//!
//! # Where stepping lives
//! Not on the REST surface. Advancing the simulation is [`protocol`] over a WebSocket
//! at `GET /sessions/{id}/ws`, because that is where `dt` can be explicit in every
//! command, a run of ten thousand steps can be one message, and a session can have one
//! writer. None of those survive a stateless request/response cycle, so there is
//! deliberately no `POST /step`.

#![forbid(unsafe_code)]

pub mod error;
pub mod protocol;
pub mod routes;
pub mod session;
pub mod ws;

pub use error::{ApiError, ErrorCode};
pub use protocol::{Command, Event, Frame, Limits, PackFacts, Role, StepCommand};
pub use session::{AppState, Session, SessionId, StaticDirs};

use axum::Router;
use tokio::net::TcpListener;

/// Version of the HTTP/WebSocket contract this binary speaks.
///
/// Bumped when a client would break: a renamed route, a renamed [`ErrorCode`], or a
/// renamed field on one of the engine types that crosses the wire (`Telemetry`,
/// `CellView`, `Demand`, `Env`). Adding a field or an error code does not bump it.
///
/// Explicitly **not** [`sim_core::SNAPSHOT_VERSION`], which versions the engine's pack
/// state and knows nothing about JSON field names. Two numbers, two jobs.
///
/// v2 (Phase 6, slice C1): `CellView`'s `v_rc_sum` became `overpotential_v`. That is
/// the first case this constant's own rule names — a renamed field on an engine type
/// that crosses the wire — and the first time it has fired: v1 stood through all five
/// Phase 4 slices and all five Phase 5 ones, including Phase 6 slice B's added
/// `"spm":null`, which the rule exempts because an added field breaks no client.
/// [`sim_core::SNAPSHOT_VERSION`] deliberately does **not** move with it: `CellView` is
/// a view, not stored state, so no saved pack changed shape.
///
/// **v2 unmoved at [`sim_core::SNAPSHOT_VERSION`] 16, and recorded because it is the exact
/// mirror of what v2 was bumped for.** That bump inlined `EcmState::v_rc` — the *vector* of
/// RC overpotentials — from a `Vec<f64>` into a fixed array, changing every saved pack's
/// shape. Nothing crosses this boundary: `CellView` has never exposed the vector, only its
/// **sum**, under the name this constant went to v2 to rename (`overpotential_v`). So a
/// change to how the summands are stored is invisible here by construction. Read from this
/// constant's own rule rather than inferred from the engine's move, per
/// `docs/plans/ui-bms-view.md`.
pub const API_VERSION: u32 = 2;

/// Build the application router over a session registry.
///
/// Exposed so tests can drive the routes directly through `tower::ServiceExt::oneshot`
/// without binding a port.
pub fn app(state: AppState) -> Router {
    routes::router(state)
}

/// Serve until the process is killed.
///
/// Takes an already-bound listener so a caller (or a test) can bind port 0 and learn
/// the real address before serving starts.
///
/// # Errors
/// Whatever the underlying accept loop fails with.
pub async fn serve(listener: TcpListener, state: AppState) -> std::io::Result<()> {
    axum::serve(listener, app(state)).await
}
