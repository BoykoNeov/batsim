//! Sessions: a live [`Pack`], the scenario it came from, and the registry they sit in.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sim_core::{ChemistryParams, Env, Pack, Telemetry};
use sim_data::{load_chemistry_file, parse_chemistry, ChemistrySource, DataError, Scenario};
use tokio::sync::{broadcast, Mutex};

use crate::error::{ApiError, ErrorCode};
use crate::protocol::Limits;

/// Handle to one session.
///
/// A plain counter value, drawn from the registry — **never** from the pack's RNG.
/// That RNG is physics state; taking a draw from it to name a session would consume
/// draws and change the trajectory. This is worth stating because "we already have an
/// RNG" is exactly the kind of shortcut that looks tidy in a diff.
///
/// Consequently an id is *not* a capability: ids are sequential and guessable, and
/// this server has no authentication. It is a localhost teaching tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub u64);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One simulation session: the pack, plus how it came to exist.
pub struct Session {
    /// This session's id.
    pub id: SessionId,
    /// The scenario the session was **created from**.
    ///
    /// Provenance, not a live description. A restore can replace the pack under it
    /// (see `routes::restore_snapshot`), so anything a client needs to be true of the
    /// pack *now* — topology, simulation time — is read from [`Self::pack`], never
    /// from here.
    pub scenario: Scenario,
    /// The live pack. Ground truth.
    pub pack: Pack,
    /// Telemetry from the most recent step, or `None` if this session has never
    /// stepped.
    ///
    /// `None` rather than a synthesised frame. The engine's zero-length-step contract
    /// would let this be filled by stepping `dt = 0`, and that is deliberately not
    /// done here: a session that has not been stepped honestly has no telemetry.
    pub latest: Option<Telemetry>,
    /// The environment any [`crate::protocol::StepCommand`] that omits its own will
    /// use.
    ///
    /// Session state, not pack state, so a restore leaves it alone — a snapshot
    /// describes a pack, and the room it sits in is the client's business. Seeded from
    /// the scenario's `initial_temp_k` with no coolant, which is the only starting
    /// value the scenario format offers; a client that cares sends `SetEnv` or puts an
    /// `env` on its first step.
    pub env: Env,
    /// Whether a socket currently holds this session's writer slot.
    ///
    /// See [`crate::protocol::Role`]. Freed when the writer's socket task returns,
    /// which covers a client disconnect, a protocol error, and a close frame.
    pub has_writer: bool,
    /// Set by [`AppState::remove_session`].
    ///
    /// A `DELETE` only unregisters the session; any socket attached to it still holds
    /// an `Arc` and would otherwise keep stepping a pack no one can reach. Every
    /// command checks this and closes instead.
    pub deleted: bool,
    /// Fan-out to read-only observers, carrying already-serialized event text.
    ///
    /// Serialized once and shared, because every observer receives the identical
    /// bytes. Deliberately *not* used for the writer's own batch replies: those are
    /// written straight to its socket, so TCP backpressure — not a dropped frame —
    /// is what happens when a writer reads slowly. A batch's frames are the
    /// experiment's record; an observer's are a view.
    pub observers: broadcast::Sender<Arc<str>>,
}

impl Session {
    /// Publish an event to the observers, if any are listening.
    ///
    /// Failure means "nobody is subscribed", which is the normal case and not an
    /// error. Serialization failure is logged rather than propagated: an event that
    /// cannot be encoded is a bug on this side, and it must not take down the writer's
    /// command that produced it.
    pub fn publish(&self, event: &crate::protocol::Event) {
        match serde_json::to_string(event) {
            Ok(text) => {
                let _ = self.observers.send(Arc::from(text.as_str()));
            }
            Err(e) => tracing::error!(%e, "an event could not be serialized for observers"),
        }
    }
}

/// How many events an observer may fall behind before the server starts discarding
/// them for it.
///
/// Small on purpose. A lagging observer is told how many it lost
/// ([`crate::protocol::Event::Dropped`]), so a deep buffer would only delay that
/// admission while spending memory per observer; the honest gap is more useful to a
/// plot than a late, smooth line.
const OBSERVER_BACKLOG: usize = 256;

/// The registry's interior. Separate from [`AppState`] so the id counter and the map
/// are guarded by one lock and an id cannot be handed out twice.
#[derive(Default)]
struct Registry {
    sessions: BTreeMap<SessionId, Arc<Mutex<Session>>>,
    next_id: u64,
}

/// Everything the HTTP layer shares.
///
/// # Locking
/// Two levels, and the order matters: **take the registry lock, clone out the
/// `Arc`s you need, drop it, then take session locks.** Never reach for the registry
/// while holding a session lock. The two-level shape is not premature — slice C's
/// stepping commands hold a session's lock for the duration of a batch (up to a
/// million steps), and a single lock over the whole map would freeze every other
/// session and every REST request for that whole time.
#[derive(Clone)]
pub struct AppState {
    registry: Arc<Mutex<Registry>>,
    chem_dir: Arc<PathBuf>,
    static_dirs: Arc<StaticDirs>,
    limits: Limits,
}

/// The two directories this server hands out verbatim, beside the chemistry directory
/// it already had.
///
/// All three exist for one client: the browser page has no filesystem, so every piece
/// of TOML it feeds to `sim-wasm` — the scenario and the chemistry — has to arrive over
/// HTTP. That is the same "JS fetches the text" resolution
/// `docs/plans/phase-4-server-wasm.md` settled on for `sim-wasm`, seen from the serving
/// end.
///
/// A missing directory is **not** an error. On a fresh clone `web/pkg/` does not exist
/// until someone runs `wasm-pack`, and a server that refused to start over it would
/// make the REST and WebSocket surfaces hostage to a build step they do not use.
#[derive(Clone, Debug)]
pub struct StaticDirs {
    /// The demo page and, under `pkg/`, the wasm bundle. Served at `/app`.
    pub web: PathBuf,
    /// Scenario TOML the page offers in its picker. Served at `/scenarios`.
    pub scenarios: PathBuf,
}

impl Default for StaticDirs {
    fn default() -> Self {
        Self {
            web: PathBuf::from("web"),
            scenarios: PathBuf::from("scenarios"),
        }
    }
}

impl AppState {
    /// A registry with no sessions, resolving chemistry ids against `chem_dir`, with
    /// the default per-message [`Limits`].
    #[must_use]
    pub fn new(chem_dir: impl Into<PathBuf>) -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::default())),
            chem_dir: Arc::new(chem_dir.into()),
            static_dirs: Arc::new(StaticDirs::default()),
            limits: Limits::default(),
        }
    }

    /// The same, with the static directories set explicitly.
    ///
    /// Consuming rather than a setter so the binary can compose it onto
    /// [`Self::new`] in one expression, and so no existing caller — every test in this
    /// crate — has to learn about directories it does not serve from.
    #[must_use]
    pub fn with_static_dirs(self, dirs: StaticDirs) -> Self {
        Self {
            static_dirs: Arc::new(dirs),
            ..self
        }
    }

    /// The same, with the stepping caps set explicitly.
    ///
    /// Exists for the tests that have to *reach* a cap: the defaults are chosen so
    /// that no reasonable client hits them, which makes them expensive to exercise
    /// honestly. A test that lowers the cap tests the same code.
    #[must_use]
    pub fn with_limits(chem_dir: impl Into<PathBuf>, limits: Limits) -> Self {
        Self {
            limits,
            ..Self::new(chem_dir)
        }
    }

    /// The directory chemistry ids are resolved against.
    #[must_use]
    pub fn chem_dir(&self) -> &Path {
        &self.chem_dir
    }

    /// The directories served as static files.
    #[must_use]
    pub fn static_dirs(&self) -> &StaticDirs {
        &self.static_dirs
    }

    /// The per-message stepping caps this server enforces.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Resolve a scenario's chemistry, build its pack, and register the session.
    ///
    /// # Errors
    /// [`ErrorCode::InvalidScenario`], [`ErrorCode::UnknownChemistry`], or
    /// [`ErrorCode::UnbuildablePack`], in that order — the scenario is validated
    /// *before* anything touches the filesystem, because that validation is what
    /// enforces the `[a-z0-9_]+` charset on a chemistry id and is therefore the only
    /// thing between a request body and a path walk out of `chem_dir`.
    ///
    /// Returns the new session **alongside its id** rather than the id alone. Handing
    /// back only an id would oblige the caller to look the session straight back up,
    /// and that lookup can legitimately fail — the registry lock is released in
    /// between — so a `POST /sessions` that had just succeeded could answer 404. A
    /// race nothing in this slice can actually lose is still a race the type system
    /// should not describe.
    pub async fn create_session(
        &self,
        scenario: Scenario,
    ) -> Result<(SessionId, Arc<Mutex<Session>>), ApiError> {
        // Always validate first, even for a `Scenario` that arrived as JSON rather
        // than through `parse_scenario`. `serde_json::from_str` does not call
        // `validate`, and `Scenario::chemistry_source` is documented to stay total on
        // inputs `validate` would have rejected — so it cannot be relied on to have
        // vetted anything.
        scenario.validate().map_err(|e| {
            ApiError::bad_request(ErrorCode::InvalidScenario, format!("invalid scenario: {e}"))
        })?;

        let chem = self.resolve_chemistry(&scenario)?;
        let pack = scenario.build_pack(chem).map_err(|e| {
            ApiError::bad_request(
                ErrorCode::UnbuildablePack,
                format!("the scenario's pack could not be built: {e}"),
            )
        })?;

        // The standing environment starts at the only ambient the scenario format
        // offers. `initial_temp_k` is the cells' starting temperature rather than the
        // room's, so this is a defensible default and not a derivation — a client that
        // cares says so with `SetEnv` or an `env` on its first step.
        let env = Env {
            t_ambient: scenario.pack.initial_temp_k,
            t_coolant: None,
        };

        let mut registry = self.registry.lock().await;
        let id = SessionId(registry.next_id);
        registry.next_id += 1;
        let session = Arc::new(Mutex::new(Session {
            id,
            scenario,
            pack,
            latest: None,
            env,
            has_writer: false,
            deleted: false,
            observers: broadcast::channel(OBSERVER_BACKLOG).0,
        }));
        registry.sessions.insert(id, Arc::clone(&session));
        Ok((id, session))
    }

    /// The session with this id, or a 404-shaped error.
    ///
    /// Returns the `Arc` rather than a guard so the caller takes the session lock
    /// *after* the registry lock has been released — see the locking note on
    /// [`AppState`].
    ///
    /// # Errors
    /// [`ErrorCode::NoSuchSession`] if the id is not registered.
    pub async fn session(&self, id: SessionId) -> Result<Arc<Mutex<Session>>, ApiError> {
        self.registry
            .lock()
            .await
            .sessions
            .get(&id)
            .cloned()
            .ok_or_else(|| ApiError::no_such_session(id))
    }

    /// Every live session, in id order (a `BTreeMap`, so the order is stable — a
    /// `HashMap` would make a list endpoint's order vary run to run for no reason).
    pub async fn all_sessions(&self) -> Vec<Arc<Mutex<Session>>> {
        self.registry
            .lock()
            .await
            .sessions
            .values()
            .cloned()
            .collect()
    }

    /// Remove a session. Returns whether there was one to remove.
    ///
    /// Unregistering is not enough on its own: a socket attached to this session holds
    /// its own `Arc` and would go on stepping a pack that no REST route can reach. So
    /// the session is also *flagged*, and every WebSocket command checks the flag.
    ///
    /// The two locks are taken in the order [`AppState`] states — registry first,
    /// released before the session lock — which matters here more than anywhere else,
    /// because the session lock may be held by a batch for as long as a million steps.
    pub async fn remove_session(&self, id: SessionId) -> bool {
        let removed = self.registry.lock().await.sessions.remove(&id);
        match removed {
            Some(session) => {
                session.lock().await.deleted = true;
                true
            }
            None => false,
        }
    }

    /// How many sessions are live.
    pub async fn session_count(&self) -> usize {
        self.registry.lock().await.sessions.len()
    }

    /// Turn a scenario's [`ChemistrySource`] into parameters.
    ///
    /// The two arms are the whole of "chemistry resolution is an adapter concern":
    /// this server maps an id onto `<chem_dir>/<id>.toml`, a browser fetches the text
    /// and inlines it. `sim-core` has no registry and must not grow one.
    fn resolve_chemistry(&self, scenario: &Scenario) -> Result<ChemistryParams, ApiError> {
        match scenario.chemistry_source() {
            ChemistrySource::Id(id) => {
                // Safe to join: `validate` has already established that `id` matches
                // `[a-z0-9_]+`, so it contains no separator, no `.` and no `..`.
                let path = self.chem_dir.join(format!("{id}.toml"));
                load_chemistry_file(&path).map_err(|e| {
                    let detail = match &e {
                        DataError::Io { .. } => {
                            format!("no chemistry {id:?} in {} ({e})", self.chem_dir.display())
                        }
                        _ => format!("chemistry {id:?} is unusable: {e}"),
                    };
                    ApiError::bad_request(ErrorCode::UnknownChemistry, detail)
                })
            }
            ChemistrySource::Inline(text) => parse_chemistry(text).map_err(|e| {
                ApiError::bad_request(
                    ErrorCode::UnknownChemistry,
                    format!("the inlined chemistry is unusable: {e}"),
                )
            }),
        }
    }
}

/// Reject a restore whose pack is not the shape of the session it is going into.
///
/// # What this does and does not catch
/// Topology only. A snapshot carries its own [`ChemistryParams`], so a 4S2P LFP
/// session restored from a 4S2P **NMC** snapshot passes this check and leaves the
/// session's stored [`Session::scenario`] naming a chemistry the pack no longer runs.
/// That is a real gap and it is stated rather than papered over: catching it would
/// need the engine to expose its chemistry for comparison, which is engine surface
/// added to serve an adapter — exactly what this phase's canary says to stop and
/// question. Everything a client needs to be true *now* is read from the pack, so the
/// gap costs a misleading provenance field and nothing else.
///
/// # Errors
/// [`ErrorCode::TopologyMismatch`] if the topologies differ.
pub fn check_restore_fits(current: &Pack, restored: &Pack) -> Result<(), ApiError> {
    if current.series() == restored.series() && current.parallel() == restored.parallel() {
        return Ok(());
    }
    Err(ApiError::new(
        StatusCode::CONFLICT,
        ErrorCode::TopologyMismatch,
        format!(
            "this session holds a {}S{}P pack; the posted snapshot is {}S{}P. Restoring \
             it would leave the session's own description of itself wrong. Create a new \
             session for a differently-shaped pack.",
            current.series(),
            current.parallel(),
            restored.series(),
            restored.parallel(),
        ),
    ))
}
