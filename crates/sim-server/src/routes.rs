//! The REST surface: create a session from a scenario, inspect it, snapshot it,
//! restore it, delete it.
//!
//! There is deliberately **no stepping endpoint here**. Advancing the simulation is
//! the WebSocket protocol's job (slice C), because stepping is where `dt`, batching
//! and the one-writer rule live, and none of those survive contact with a stateless
//! request/response cycle.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sim_core::{CellView, Pack, Snapshot, Telemetry, SNAPSHOT_VERSION};
use sim_data::{parse_scenario, Scenario};

use crate::error::{ApiError, ErrorCode};
use crate::session::{check_restore_fits, AppState, Session, SessionId};
use crate::API_VERSION;

/// Every route this server serves.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(api_root))
        .route("/sessions", post(create_session).get(list_sessions))
        .route("/sessions/{id}", get(get_session).delete(delete_session))
        .route("/sessions/{id}/cells", get(get_cells))
        .route(
            "/sessions/{id}/snapshot",
            get(get_snapshot).post(restore_snapshot),
        )
        .with_state(state)
}

/// What `GET /` reports.
///
/// The two version numbers do different jobs and the `note` says so in-band, because
/// a client reading this JSON is not reading these doc comments.
#[derive(Debug, Serialize)]
struct ApiRoot {
    /// Version of *this HTTP/WebSocket contract*: route shapes, error codes, and the
    /// JSON field names of the engine types that cross the wire. Bumped when a client
    /// would break.
    api_version: u32,
    /// Version of the *engine's snapshot layout* ([`sim_core::SNAPSHOT_VERSION`]).
    /// A snapshot taken from a different value cannot be restored into this binary.
    /// Entirely independent of `api_version`: the wire contract can change without
    /// the pack's state layout changing, and vice versa.
    snapshot_version: u32,
    /// A one-line reminder of the above, for whoever is poking at this with curl.
    note: &'static str,
    /// Live session count.
    sessions: usize,
    /// Directory chemistry ids are resolved against, as given on the command line.
    chem_dir: String,
}

async fn api_root(State(state): State<AppState>) -> Json<ApiRoot> {
    Json(ApiRoot {
        api_version: API_VERSION,
        snapshot_version: SNAPSHOT_VERSION,
        note: "api_version versions this HTTP contract; snapshot_version versions the \
               engine's saved pack layout. They move independently.",
        sessions: state.session_count().await,
        chem_dir: state.chem_dir().display().to_string(),
    })
}

/// Live facts read from the pack itself.
///
/// Deliberately not read from the scenario the session was created from: a restore can
/// replace the pack, and then the scenario is provenance rather than description.
#[derive(Debug, Serialize)]
struct PackFacts {
    /// Series elements.
    series: u16,
    /// Parallel cells per group.
    parallel: u16,
    /// Simulation time elapsed \[s\].
    sim_time_s: f64,
}

impl PackFacts {
    fn of(pack: &Pack) -> Self {
        Self {
            series: pack.series(),
            parallel: pack.parallel(),
            sim_time_s: pack.sim_time_s(),
        }
    }
}

/// `POST /sessions` response.
#[derive(Debug, Serialize)]
struct SessionCreated {
    id: SessionId,
    api_version: u32,
    snapshot_version: u32,
    pack: PackFacts,
}

/// One line of `GET /sessions`.
#[derive(Debug, Serialize)]
struct SessionSummary {
    id: SessionId,
    /// The scenario's `[meta] name`.
    name: String,
    pack: PackFacts,
    /// Whether this session has ever been stepped.
    stepped: bool,
}

/// `GET /sessions/{id}` response.
#[derive(Debug, Serialize)]
struct SessionDetail {
    id: SessionId,
    /// The scenario this session was created from — **provenance**. After a restore
    /// it describes how the session started, not necessarily the pack it now holds;
    /// `pack` below is always live.
    scenario: Scenario,
    pack: PackFacts,
    /// Telemetry from the most recent step, or `null` if this session has never been
    /// stepped. Not synthesised — see [`Session::latest`].
    latest_telemetry: Option<Telemetry>,
}

/// `GET /sessions/{id}/cells` response: the ground-truth pedagogy view.
#[derive(Debug, Serialize)]
struct CellsResponse {
    series: u16,
    parallel: u16,
    /// Every cell, **series-major and parallel-minor**: the cell at series position
    /// `s`, parallel position `p` is at index `s * parallel + p`. Same order the
    /// engine uses internally.
    ///
    /// This is ground truth — every cell's true state, not what the BMS can sense.
    /// The gap between this and `Telemetry::soc_bms` is a feature to look at.
    cells: Vec<CellView>,
}

/// `POST /sessions/{id}/snapshot` response.
#[derive(Debug, Serialize)]
struct RestoreResult {
    id: SessionId,
    pack: PackFacts,
}

/// Create a session from a scenario.
///
/// # Body encoding
/// Both formats the rest of the repo already uses, chosen by `Content-Type`:
///
/// * `application/json` → the [`Scenario`] struct as JSON;
/// * `application/toml`, `text/plain`, `application/x-www-form-urlencoded`, or **no
///   `Content-Type` at all** → scenario TOML, which is what every file under
///   `scenarios/` is.
///
/// TOML is the fallback because that is the on-disk format, so
/// `curl --data-binary @scenarios/cc_discharge_lfp.toml …` works with no ceremony.
/// (`--data-binary`, not `-d`: `-d @file` strips newlines, which mangles TOML.)
///
/// `application/x-www-form-urlencoded` is in the TOML set for one reason, found by
/// running the documented command rather than by reasoning about it: **that is what
/// curl sends by default**, for `--data-binary` as much as for `-d`. Nobody chooses
/// that type deliberately for a scenario file, so treating it as a form encoding
/// would reject the single most likely way a person reaches this endpoint. A type
/// that *was* chosen deliberately and that this server cannot read — `application/xml`
/// — still gets a 415.
///
/// Both paths validate. That is not automatic — `parse_scenario` validates for you,
/// `serde_json::from_str` does not, and forgetting it on the JSON path would let a
/// chemistry id like `../../etc/passwd` through the charset check that exists to stop
/// exactly that.
async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<(StatusCode, Json<SessionCreated>), ApiError> {
    let scenario = parse_body_as_scenario(&headers, &body)?;
    let id = state.create_session(scenario).await?;

    let session = state.session(id).await?;
    let session = session.lock().await;
    tracing::info!(%id, name = %session.scenario.meta.name, "session created");

    Ok((
        StatusCode::CREATED,
        Json(SessionCreated {
            id,
            api_version: API_VERSION,
            snapshot_version: SNAPSHOT_VERSION,
            pack: PackFacts::of(&session.pack),
        }),
    ))
}

/// The content-type fork, kept out of the handler so it can be read in one sitting.
fn parse_body_as_scenario(headers: &HeaderMap, body: &str) -> Result<Scenario, ApiError> {
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            // Drop any `; charset=…` parameter before matching.
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });

    match content_type.as_deref() {
        Some("application/json") => {
            let scenario: Scenario = serde_json::from_str(body).map_err(|e| {
                ApiError::bad_request(
                    ErrorCode::MalformedBody,
                    format!("body is not a JSON scenario: {e}"),
                )
            })?;
            // `from_str` does not validate; `create_session` will. Returning the
            // unvalidated value here is safe only because that is guaranteed — the
            // check is not duplicated, it is centralised.
            Ok(scenario)
        }
        None
        | Some(
            "application/toml"
            | "text/toml"
            | "text/plain"
            // curl's default for -d / --data-binary; see this handler's doc comment.
            | "application/x-www-form-urlencoded",
        ) => parse_scenario(body)
            .map_err(|e| {
                let hint = if body.trim_start().starts_with('{') {
                    " (the body starts with '{' — did you mean Content-Type: application/json?)"
                } else {
                    ""
                };
                ApiError::bad_request(
                    ErrorCode::MalformedBody,
                    format!("body is not a TOML scenario: {e}{hint}"),
                )
            }),
        Some(other) => Err(ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::UnsupportedMediaType,
            format!(
                "Content-Type {other:?} is not accepted here; send scenario TOML as \
                 application/toml (or with no Content-Type) or the Scenario struct as \
                 application/json"
            ),
        )),
    }
}

async fn list_sessions(State(state): State<AppState>) -> Json<Vec<SessionSummary>> {
    let mut out = Vec::new();
    // Registry lock is already released — `all_sessions` returns clones of the Arcs,
    // so the per-session locks below are taken without holding it. See `AppState`.
    for session in state.all_sessions().await {
        let session = session.lock().await;
        out.push(SessionSummary {
            id: session.id,
            name: session.scenario.meta.name.clone(),
            pack: PackFacts::of(&session.pack),
            stepped: session.latest.is_some(),
        });
    }
    Json(out)
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<SessionDetail>, ApiError> {
    let session = state.session(SessionId(id)).await?;
    let session = session.lock().await;
    Ok(Json(SessionDetail {
        id: session.id,
        scenario: session.scenario.clone(),
        pack: PackFacts::of(&session.pack),
        latest_telemetry: session.latest,
    }))
}

async fn get_cells(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<CellsResponse>, ApiError> {
    let session = state.session(SessionId(id)).await?;
    let session = session.lock().await;
    Ok(Json(cells_of(&session)))
}

fn cells_of(session: &Session) -> CellsResponse {
    let pack = &session.pack;
    let (series, parallel) = (pack.series(), pack.parallel());
    let mut cells = Vec::with_capacity(usize::from(series) * usize::from(parallel));
    for s in 0..usize::from(series) {
        for p in 0..usize::from(parallel) {
            // In range by construction: both indices come from the pack's own
            // topology, which is why this cannot be an error path.
            if let Some(view) = pack.cell(s, p) {
                cells.push(view);
            }
        }
    }
    CellsResponse {
        series,
        parallel,
        cells,
    }
}

/// The whole engine state, as JSON.
///
/// Exact: the workspace's `serde_json` carries `float_roundtrip`, without which the
/// parser's fast path can return a value one ULP off the one that was written and a
/// restored session drifts silently. `tests/snapshot_json.rs` is the guard.
async fn get_snapshot(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<Snapshot>, ApiError> {
    let session = state.session(SessionId(id)).await?;
    let session = session.lock().await;
    Ok(Json(session.pack.snapshot()))
}

/// Restore a snapshot **into this session**, replacing its pack.
///
/// Not a create: the id, and any client watching it, stay pointed at the same session.
/// The posted pack must have the same topology as the one it replaces — see
/// [`check_restore_fits`] for what that does and does not cover.
///
/// The session's `latest_telemetry` is cleared rather than kept: the stored frame
/// described the pack that was just discarded, and a stale frame beside a restored
/// pack is worse than no frame.
///
/// The body is taken as a string and parsed here rather than through axum's `Json`
/// extractor so that a malformed snapshot produces this server's error shape instead
/// of the framework's.
async fn restore_snapshot(
    State(state): State<AppState>,
    Path(id): Path<u64>,
    body: String,
) -> Result<Json<RestoreResult>, ApiError> {
    let snapshot: Snapshot = serde_json::from_str(&body).map_err(|e| {
        ApiError::bad_request(
            ErrorCode::BadSnapshot,
            format!("body is not a snapshot: {e}"),
        )
    })?;

    let restored = Pack::restore(&snapshot).map_err(|e| {
        ApiError::bad_request(
            ErrorCode::BadSnapshot,
            format!("{e} — this binary produces and consumes version {SNAPSHOT_VERSION}"),
        )
    })?;

    let session = state.session(SessionId(id)).await?;
    let mut session = session.lock().await;
    check_restore_fits(&session.pack, &restored)?;

    session.pack = restored;
    session.latest = None;
    tracing::info!(%id, sim_time_s = session.pack.sim_time_s(), "session restored");

    Ok(Json(RestoreResult {
        id: session.id,
        pack: PackFacts::of(&session.pack),
    }))
}

async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    if state.remove_session(SessionId(id)).await {
        tracing::info!(%id, "session deleted");
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::no_such_session(id))
    }
}
