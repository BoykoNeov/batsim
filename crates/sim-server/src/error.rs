//! The error a client sees, and the vocabulary of codes behind it.
//!
//! Every failure leaves this server as
//!
//! ```json
//! { "error": { "code": "no_such_session", "message": "no session 7" } }
//! ```
//!
//! The `code` is the machine-readable half and is stable API; the `message` is for a
//! person and may be reworded. Slice C's WebSocket `Error` event carries the same two
//! fields with the same vocabulary — a client that learns these codes over REST does
//! not have to learn a second set over the socket, which is the whole reason the codes
//! live in their own type rather than being formatted into strings at each call site.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Machine-readable failure kinds.
///
/// Serialized in `snake_case`. Adding a variant is backwards-compatible; renaming one
/// is a client-visible break and bumps [`crate::API_VERSION`].
/// `Deserialize` is here for clients, not for the server: nothing on this side ever
/// parses a code, but the WebSocket tests parse the events they receive, and a client
/// that has to match on strings by hand is a client that finds a rename at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request body was not valid UTF-8, or not valid TOML/JSON for its
    /// declared content type.
    MalformedBody,
    /// The body parsed but is not a usable scenario — see
    /// [`sim_data::Scenario::validate`].
    InvalidScenario,
    /// The scenario names a chemistry id this server cannot resolve against its
    /// chemistry directory, or an inlined chemistry that does not parse.
    UnknownChemistry,
    /// The scenario is valid but no pack can be built from it (topology, initial
    /// conditions, a fault that does not fit the pack it targets).
    UnbuildablePack,
    /// No session with that id. Also returned for a session deleted concurrently.
    NoSuchSession,
    /// A posted snapshot is not a snapshot this build understands — malformed, or a
    /// [`sim_core::SNAPSHOT_VERSION`] this binary does not speak.
    BadSnapshot,
    /// A posted snapshot describes a different pack than the session it was posted
    /// into. See [`crate::routes`] for what is and is not checked.
    TopologyMismatch,
    /// The request body's `Content-Type` is one this endpoint does not accept.
    UnsupportedMediaType,
    /// A WebSocket message was not a [`crate::protocol::Command`] — not text, not
    /// JSON, or JSON naming no command this server knows.
    InvalidCommand,
    /// A command carried a number outside what the boundary accepts: non-finite, a
    /// negative `dt`, a zero `n_steps`. Distinct from the two cap codes below, which
    /// are server *policy* rather than a nonsensical value.
    OutOfRange,
    /// `n_steps` exceeds [`crate::protocol::Limits::max_steps_per_command`]. Fix by
    /// splitting the run across messages; the trajectory is unaffected.
    TooManySteps,
    /// The batch would produce more frames than
    /// [`crate::protocol::Limits::max_frames_per_reply`]. Fix by raising
    /// `report_every_n_steps`; the trajectory is unaffected.
    TooManyFrames,
    /// A read-only observer sent a command that would move the pack. See
    /// [`crate::protocol::Role`].
    NotWriter,
    /// `Pack::schedule_fault` refused the fault — a non-finite time, a non-positive
    /// short resistance, or a cell index outside the pack.
    FaultRejected,
    /// The server failed on its own account rather than the client's. In practice:
    /// the batch-stepping task did not come back.
    Internal,
}

/// A failure on its way to a client: an HTTP status, a stable [`ErrorCode`], and a
/// sentence a person can act on.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
}

impl ApiError {
    /// Build an error with an explicit status.
    pub fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// A 400: the client sent something this server cannot use.
    pub fn bad_request(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// A 404 for a session id that is not in the registry.
    pub fn no_such_session(id: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::NoSuchSession,
            format!("no session {id}"),
        )
    }

    /// An error that will leave over a WebSocket rather than as an HTTP response.
    ///
    /// The status is still carried — [`Self::status`] stays meaningful, and a few of
    /// these do reach HTTP by way of the upgrade handshake — but on the socket only
    /// the code and the message are sent. `BAD_REQUEST` is the honest default: every
    /// use of this is the client having asked for something the server will not do.
    pub fn ws(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// The message, for the WebSocket layer, which sends it as a field rather than as
    /// a response body.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The code, for tests and for the WebSocket layer that reuses this vocabulary.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// The HTTP status this error will produce.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": { "code": self.code, "message": self.message } })),
        )
            .into_response()
    }
}
