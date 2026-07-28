//! The whole of this crate's behaviour, in plain Rust with no `godot` in sight.
//!
//! # Why the logic is not in the `#[derive(GodotClass)]` type
//! Identical reasoning to `sim_wasm::engine`, and it was verified for this crate rather
//! than inherited: a spike confirmed that a gdext crate host-tests fine with no Godot
//! process running, **provided no test touches a `godot` type**. A `Gd<T>`, a `Variant`,
//! or a `Base<Node>` outside a running engine is either absent or a stub, so any test
//! that constructed one would be testing the stub. Keeping every decision here — typed
//! errors, `Result` everywhere, no `godot` type in a signature — means `tests/driver.rs`
//! exercises the real translation layer, and [`crate::BatteryPack`] is left with nothing
//! but `?` and a conversion.
//!
//! It buys a second thing this crate needs more than `sim-wasm` did: the exit gate's
//! in-process leg can call [`PackDriver::new`] directly, so **both legs of the gate share
//! one scenario→pack path** and the only difference between them is the GDScript
//! boundary. A leg that hand-built a `PackConfig` could diverge on scatter seeding and
//! fail the gate for a reason that has nothing to do with what the gate is testing.
//!
//! # What is *not* duplicated here
//! Phase 5 deliberately did not copy two things a third time. The finiteness rule is
//! `sim_core::Demand::check_finite` / `sim_core::Env::check_finite`, and building a
//! scenario with or without its BMS is `sim_data::Scenario::build_pack_with_bms`. Both
//! moved to the type the statement is *about*, which is why this crate needs no
//! `sim-protocol` and no dependency on `sim-server`. See `docs/plans/phase-5-godot.md`.
//!
//! # The two caps, which are not the same cap
//! [`MAX_STEPS_PER_CALL`] guards an explicit [`PackDriver::step_batch`] against a caller
//! who typed a zero too many. [`PackDriver::advance_real_time`]'s `max_steps` is a
//! *per-frame policy* the game owns and tunes. Conflating them would either make a
//! fast-forward impossible or let one long frame stall the game.

use sim_core::{
    CellView, ChemistryParams, Demand, Env, EventFlags, Fault, NonFinite, Pack, Snapshot,
    Telemetry, SNAPSHOT_VERSION,
};
use sim_data::{parse_chemistry, parse_scenario, ChemistrySource, DataError, Scenario};

/// Most steps one [`PackDriver::step_batch`] call may advance the pack.
///
/// Note this is **not** the same number as `sim_server`'s `max_steps_per_command` or
/// `sim_wasm`'s `MAX_STEPS_PER_CALL`, even where the value coincides, and it is
/// deliberately not shared with them. The server's bounds how long a session lock is
/// held; `sim-wasm`'s bounds occupancy of a browser's main thread; this one bounds how
/// long a single `#[func]` call can block Godot's main loop — a frozen game window rather
/// than a slow reply. Three constants, three rationales; sharing them would put a number
/// in one place that no client could then justify changing.
pub const MAX_STEPS_PER_CALL: u32 = 1_000_000;

/// What to do with wall-clock time the per-frame cap would not let us consume.
///
/// This exists because the alternative — picking one silently — makes sim time dilate
/// under load in a way that is indistinguishable from a physics bug to whoever is looking
/// at the plot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backlog {
    /// Throw the un-consumed time away. Sim time falls behind wall time and stays behind;
    /// the game stays responsive. The right default for a game, where a stall should cost
    /// simulated seconds rather than turn into a cascade.
    #[default]
    Drop,
    /// Keep it and work it off over subsequent frames. Sim time stays true to wall time;
    /// a long stall can cascade into several capped frames in a row.
    Repay,
}

/// What one [`Accumulator::take`] decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ticks {
    /// Whole steps due this tick.
    pub steps: u32,
    /// Simulated time left un-consumed \[s\]. Under [`Backlog::Drop`] this is what was
    /// discarded; under [`Backlog::Repay`] it is what is still owed. Zero unless the cap
    /// bound.
    pub backlog_s: f64,
    /// Whether the per-frame cap bound. This is the condition a game should surface.
    pub capped: bool,
}

/// Fixed-timestep accumulator: consume whole steps, carry the remainder.
///
/// # Why this exists when `_physics_process` is already fixed-rate
/// Godot 4.7's `_physics_process` delta defaults to 1/60 s, which makes it tempting to
/// step once per call. Three measured facts say no, and all three are settable by the
/// game rather than by this crate:
///
/// - `Engine.physics_ticks_per_second` (default 60) changes the delta outright.
/// - `Engine.time_scale` (default 1.0) scales it. A slow-motion game would otherwise
///   silently change the physics timestep — the one thing `CLAUDE.md` forbids outright.
/// - `Engine.max_physics_steps_per_frame` (default 8) means Godot itself drops physics
///   ticks under load rather than letting them pile up.
///
/// So `fixed_dt` is the caller's, it is the only thing that sets a step's size, and the
/// remainder is carried here.
#[derive(Clone, Copy, Debug, Default)]
pub struct Accumulator {
    pending_s: f64,
}

impl Accumulator {
    /// Time carried over, not yet consumed \[s\].
    #[must_use]
    pub fn pending_s(self) -> f64 {
        self.pending_s
    }

    /// Forget any carried time.
    ///
    /// Called on restart and restore: the remainder describes a run that no longer
    /// exists, and carrying it across would put a fraction of the old run's wall clock
    /// into the new one's first frame.
    pub fn reset(&mut self) {
        self.pending_s = 0.0;
    }

    /// Add `delta_s` of wall-clock time and report how many whole `fixed_dt` steps are
    /// due, at most `max_steps`.
    ///
    /// A non-finite or negative `delta_s` contributes nothing rather than poisoning
    /// `pending_s` — Godot will not normally produce one, but a `NaN` that got in here
    /// would make every subsequent frame `NaN` forever, which is a far worse failure than
    /// a dropped frame. `fixed_dt` is validated by the caller ([`PackDriver::advance_real_time`]).
    pub fn take(&mut self, delta_s: f64, fixed_dt: f64, max_steps: u32, backlog: Backlog) -> Ticks {
        if delta_s.is_finite() && delta_s > 0.0 {
            self.pending_s += delta_s;
        }

        let whole = (self.pending_s / fixed_dt).floor();
        // `whole` is finite and >= 0 here: `pending_s >= 0` and `fixed_dt > 0`. The
        // `min` is what makes the `as` cast lossless.
        let uncapped = whole.min(f64::from(u32::MAX));
        let capped = uncapped > f64::from(max_steps);
        let steps = if capped {
            max_steps
        } else {
            // Truncation cannot lose anything: `uncapped` is an integral value already
            // clamped into `u32`'s range.
            uncapped as u32
        };

        self.pending_s -= f64::from(steps) * fixed_dt;
        // Floating-point subtraction can leave a value a hair below zero; a negative
        // remainder would slowly steal time from later frames.
        if self.pending_s < 0.0 {
            self.pending_s = 0.0;
        }

        let backlog_s = if capped { self.pending_s } else { 0.0 };
        if capped && backlog == Backlog::Drop {
            self.pending_s = 0.0;
        }

        Ticks {
            steps,
            backlog_s,
            capped,
        }
    }
}

/// Which flags turned on and which turned off across a step or a batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edges {
    /// Flags that went 0→1.
    pub rising: EventFlags,
    /// Flags that went 1→0.
    pub falling: EventFlags,
}

/// Hand-written because `bitflags` does not derive `Default`, and "no edges" has to mean
/// *empty*, which is not something a derive could have guessed wrong silently.
impl Default for Edges {
    fn default() -> Self {
        Self {
            rising: EventFlags::empty(),
            falling: EventFlags::empty(),
        }
    }
}

impl Edges {
    /// Whether anything changed at all — the cheap test a caller uses to skip emitting.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.rising.is_empty() && self.falling.is_empty()
    }

    /// Fold another step's edges into this one.
    ///
    /// Union, not replacement: within a batch, a flag that rose and fell again reports
    /// **both**, because a signal consumer that missed the pair entirely would be told a
    /// less true story than one told the ordering was lost. See [`FlagEdges`] for the
    /// cost this accepts.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            rising: self.rising | other.rising,
            falling: self.falling | other.falling,
        }
    }
}

/// Edge detector over [`EventFlags`].
///
/// # Why the previous mask is adapter state
/// Putting a `prev_flags` field on `Pack` would change the snapshot layout and bump
/// `SNAPSHOT_VERSION` for a purely presentational need — exactly what Phase 5's canary
/// exists to catch. It lives here instead.
///
/// The consequence, accepted and written down rather than discovered: **a restore does
/// not restore the edge detector.** [`PackDriver::restore_json`] resets it, so the first
/// step after a restore re-announces conditions that were already active in the snapshot.
/// A game hears "protection tripped" for a trip that happened before the save. The
/// alternative is engine pollution, and re-announcing is the failure a listener can
/// actually cope with.
///
/// # Why rising edges, and why batches coalesce
/// `EventFlags` is recomputed every step, so emitting whenever a flag is *set* emits at
/// 60 Hz for the whole duration of a condition — a signal storm that makes the feature
/// useless. Only transitions are reported.
///
/// Within a batch the transitions are unioned and reported once, for the same reason
/// Phase 4 decimated telemetry: a `step_batch(10_000)` that emitted per step would push
/// thousands of signals into a game's main loop. The cost is that the *ordering* of two
/// different events inside one batch is lost. That is acceptable for a fast-forward, and
/// real-time batches are a handful of steps, so it is acceptable there too.
#[derive(Clone, Copy, Debug)]
pub struct FlagEdges {
    prev: EventFlags,
}

/// See [`Edges`]'s `Default` — same reason.
impl Default for FlagEdges {
    fn default() -> Self {
        Self {
            prev: EventFlags::empty(),
        }
    }
}

impl FlagEdges {
    /// Compare `now` against the previous observation and store it.
    pub fn observe(&mut self, now: EventFlags) -> Edges {
        let edges = Edges {
            rising: now & !self.prev,
            falling: self.prev & !now,
        };
        self.prev = now;
        edges
    }

    /// The last observed mask.
    #[must_use]
    pub fn last(self) -> EventFlags {
        self.prev
    }

    /// Forget the previous observation.
    ///
    /// Deliberately resets to **empty**, not to the pack's current flags: after a restore
    /// the honest position is "this listener has been told nothing", so everything active
    /// is announced once. Seeding from the restored pack would silence a condition the
    /// listener never heard about.
    pub fn reset(&mut self) {
        self.prev = EventFlags::empty();
    }
}

/// What a [`PackDriver::step_batch`] or [`PackDriver::advance_real_time`] did.
#[derive(Clone, Copy, Debug)]
pub struct Advance {
    /// Steps actually taken. Zero is normal for a real-time tick that did not accumulate
    /// a whole `fixed_dt`.
    pub steps: u32,
    /// Flag transitions across the whole batch, coalesced. See [`FlagEdges`].
    pub edges: Edges,
    /// Telemetry at the end of the batch. When `steps == 0` this is the previous
    /// reading, unchanged.
    pub telemetry: Telemetry,
    /// Simulated time left un-consumed \[s\]; always zero for [`PackDriver::step_batch`].
    pub backlog_s: f64,
    /// Whether a per-frame cap bound; always false for [`PackDriver::step_batch`].
    pub capped: bool,
}

/// What a game needs to know about the pack it is driving, without a snapshot.
#[derive(Clone, Copy, Debug)]
pub struct PackFacts {
    /// Series elements.
    pub series: u16,
    /// Parallel cells per group.
    pub parallel: u16,
    /// Simulation time elapsed \[s\].
    pub sim_time_s: f64,
    /// Whether the pack currently has a BMS. Read from the pack, not from the scenario,
    /// so it stays true across a restart and a restore.
    pub has_bms: bool,
    /// Whether the **scenario** configures a BMS at all. When false, a BMS toggle has
    /// nothing to turn back on.
    pub scenario_has_bms: bool,
    /// How many of the scenario's faults were dropped because this build has no BMS to
    /// aim them at. See [`sim_data::Scenario::build_pack_with_bms`].
    pub sensor_faults_dropped: u32,
}

/// Everything that can go wrong on this crate's surface.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// The scenario or chemistry text did not parse or did not validate.
    #[error("{0}")]
    Data(#[from] DataError),
    /// A JSON argument or reply could not be (de)serialized.
    #[error("{context}: {source}")]
    Json {
        /// What was being (de)serialized.
        context: &'static str,
        /// The underlying serde failure.
        source: serde_json::Error,
    },
    /// A number GDScript passed was outside the range this boundary accepts.
    ///
    /// This arm exists because there is no parser between GDScript and here: `NAN` and
    /// `INF` are both spellable in GDScript and arrive as raw `f64`s intact. Same
    /// situation as the wasm boundary, and the reason the rule lives on `sim-core`'s own
    /// types rather than being copied a third time.
    #[error("out of range: {0}")]
    OutOfRange(String),
    /// One call asked for more steps than [`MAX_STEPS_PER_CALL`].
    #[error("asked for {asked} steps, cap is {cap}; split the run across calls (the trajectory is identical either way)")]
    TooManySteps {
        /// Steps requested.
        asked: u32,
        /// The cap.
        cap: u32,
    },
    /// The scenario names a chemistry id and no chemistry text was supplied.
    #[error("this scenario names chemistry {id:?}; read {id}.toml (FileAccess.get_file_as_string) and pass its text alongside the scenario (a scenario that inlines `chemistry_toml` needs no second argument)")]
    ChemistryNotSupplied {
        /// The id the scenario named.
        id: String,
    },
    /// A snapshot could not be turned back into a pack.
    #[error("{0} — this build produces and consumes snapshot version {SNAPSHOT_VERSION}")]
    Restore(#[from] sim_core::RestoreError),
    /// A fault was rejected by the engine's own validation.
    #[error("{0}")]
    Fault(#[from] sim_core::FaultError),
    /// A restore was refused because the snapshot describes a differently-shaped pack.
    #[error("snapshot is {got_series}S{got_parallel}P but this node runs {want_series}S{want_parallel}P; restoring it would silently replace the pack under everything watching it")]
    TopologyMismatch {
        /// Series count the snapshot carries.
        got_series: u16,
        /// Parallel count the snapshot carries.
        got_parallel: u16,
        /// Series count of the live pack.
        want_series: u16,
        /// Parallel count of the live pack.
        want_parallel: u16,
    },
}

impl DriverError {
    fn json(context: &'static str) -> impl FnOnce(serde_json::Error) -> Self {
        move |source| Self::Json { context, source }
    }
}

impl From<NonFinite> for DriverError {
    fn from(error: NonFinite) -> Self {
        Self::OutOfRange(error.to_string())
    }
}

/// A scenario, its chemistry, the pack the two of them built, and everything a node needs
/// to drive it.
#[derive(Debug)]
pub struct PackDriver {
    /// The scenario **as authored**. Never mutated — [`Self::restart`] rebuilds from it,
    /// so `scenario.pack.bms` stays the answer to "what did the file ask for".
    scenario: Scenario,
    chem: ChemistryParams,
    bms_enabled: bool,
    pack: Pack,
    sensor_faults_dropped: u32,
    /// The standing environment, seeded from the scenario's `initial_temp_k`.
    ///
    /// Session state, not pack state: a snapshot describes a pack, and the room it sits
    /// in is the client's business — so a restore deliberately leaves this alone. Same
    /// rule `sim-server` and `sim-wasm` apply.
    env: Env,
    /// The most recent reading. See [`Self::new`] for why this is a `Telemetry` and not
    /// an `Option<Telemetry>`.
    latest: Telemetry,
    edges: FlagEdges,
    accumulator: Accumulator,
}

impl PackDriver {
    /// Build a pack from scenario text and take one zero-length step so there is a
    /// reading before anything has run.
    ///
    /// # Why this primes, when `sim-wasm` refuses to
    /// `sim_wasm::SimEngine` deliberately stores no "latest telemetry" and tells a page
    /// that wants a reading on load to ask for `dt = 0, n_steps = 1`. That is right for a
    /// page, which makes an explicit call and can handle "nothing yet".
    ///
    /// A Godot node cannot. Its readings are **properties**, and a property has no
    /// spelling for "no reading yet" — a `soc` getter on a freshly-constructed node
    /// returns *something*, and if that something is a default `0.0` a plot draws it as a
    /// real measurement of an empty pack. So the driver asks the same question the page
    /// would, at construction, once.
    ///
    /// This does not contradict the repo's rule, which is that an adapter must not
    /// *synthesise* a frame it was not asked for. This one asked. And it is free: the
    /// engine pins a zero-length-step contract, and priming was checked rather than
    /// assumed — a 200-step run preceded by a `dt = 0` step is **byte-identical in its
    /// snapshot** to the same run without it, on a scenario with faults and a noisy
    /// sensor, so not even the RNG advances. See `docs/plans/phase-5-godot.md`.
    ///
    /// `chemistry_toml` is consulted **only** when the scenario names a chemistry id — a
    /// scenario that inlines `chemistry_toml` is self-contained and its own text wins.
    ///
    /// # Errors
    /// [`DriverError::Data`] if either text fails to parse or validate,
    /// [`DriverError::ChemistryNotSupplied`] if the scenario names an id and nothing was
    /// passed.
    pub fn new(scenario_toml: &str, chemistry_toml: Option<&str>) -> Result<Self, DriverError> {
        // `parse_scenario` validates; `Scenario` values built any other way do not. This
        // is the only constructor, so validation is not optional here.
        let scenario = parse_scenario(scenario_toml)?;
        let chem = match scenario.chemistry_source() {
            ChemistrySource::Inline(text) => parse_chemistry(text)?,
            ChemistrySource::Id(id) => match chemistry_toml {
                Some(text) if !text.trim().is_empty() => parse_chemistry(text)?,
                _ => return Err(DriverError::ChemistryNotSupplied { id: id.to_owned() }),
            },
        };

        let bms_enabled = scenario.pack.bms.is_some();
        let (mut pack, sensor_faults_dropped) =
            scenario.build_pack_with_bms(chem.clone(), bms_enabled)?;
        let env = Env {
            t_ambient: scenario.pack.initial_temp_k,
            t_coolant: None,
        };

        let latest = pack.step(0.0, Demand::Rest, &env);
        let mut edges = FlagEdges::default();
        // Observe the primed reading so construction does not immediately re-announce
        // whatever the pack starts with as a fresh transition.
        edges.observe(latest.flags);

        Ok(Self {
            scenario,
            chem,
            bms_enabled,
            pack,
            sensor_faults_dropped,
            env,
            latest,
            edges,
            accumulator: Accumulator::default(),
        })
    }

    /// Which chemistry a scenario needs, so a game can read it before constructing.
    ///
    /// `Ok(None)` means the scenario inlines its chemistry and needs nothing loaded. The
    /// id is already known to match `[a-z0-9_]+`, so it may be interpolated into a
    /// `res://` path without escaping.
    ///
    /// # Errors
    /// [`DriverError::Data`] if the scenario does not parse or validate — which means a
    /// game learns about a broken scenario here rather than one file-read later.
    pub fn chemistry_id_of(scenario_toml: &str) -> Result<Option<String>, DriverError> {
        let scenario = parse_scenario(scenario_toml)?;
        Ok(match scenario.chemistry_source() {
            ChemistrySource::Id(id) => Some(id.to_owned()),
            ChemistrySource::Inline(_) => None,
        })
    }

    /// Live facts about the pack.
    #[must_use]
    pub fn facts(&self) -> PackFacts {
        PackFacts {
            series: self.pack.series(),
            parallel: self.pack.parallel(),
            sim_time_s: self.pack.sim_time_s(),
            has_bms: self.pack.bms().is_some(),
            scenario_has_bms: self.scenario.pack.bms.is_some(),
            sensor_faults_dropped: self.sensor_faults_dropped,
        }
    }

    /// The most recent reading. Never synthetic — see [`Self::new`].
    #[must_use]
    pub fn latest(&self) -> Telemetry {
        self.latest
    }

    /// Simulated time elapsed \[s\].
    #[must_use]
    pub fn sim_time_s(&self) -> f64 {
        self.pack.sim_time_s()
    }

    /// The standing environment.
    #[must_use]
    pub fn env(&self) -> Env {
        self.env
    }

    /// Replace the standing environment.
    ///
    /// # Errors
    /// [`DriverError::OutOfRange`] if either temperature is not finite. Reachable: these
    /// arrive from GDScript as raw `f64`s with no parser in between.
    pub fn set_env(&mut self, env: Env) -> Result<(), DriverError> {
        env.check_finite()?;
        self.env = env;
        Ok(())
    }

    /// Whether the last build asked for the scenario's BMS.
    #[must_use]
    pub fn bms_enabled(&self) -> bool {
        self.bms_enabled
    }

    /// Rebuild the pack from the scenario, with or without its BMS.
    ///
    /// **This restarts the run**: simulation time returns to zero, every cell returns to
    /// the scenario's initial SOC and temperature, the RNG returns to its seed, and the
    /// scenario's faults are re-queued. That is not a limitation being apologised for — a
    /// BMS is part of what a pack *is*, and there is no honest way to grow one onto a pack
    /// that has been running without it. Contrasting the same scenario with and without
    /// protection is the teaching case, and it is a comparison of two runs.
    ///
    /// `enabled = true` restores whatever the scenario configured, which is *nothing* if
    /// the scenario ships no `[pack.bms]` — see [`PackFacts::scenario_has_bms`], which is
    /// what a game should disable its toggle on.
    ///
    /// The accumulator and the edge detector are reset with it: both describe a run that
    /// no longer exists.
    ///
    /// # Errors
    /// [`DriverError::Data`] if the rebuild fails, which given the same inputs succeeded
    /// once already means something is wrong with this crate, not the input.
    pub fn restart(&mut self, enabled: bool) -> Result<(), DriverError> {
        let (mut pack, dropped) = self
            .scenario
            .build_pack_with_bms(self.chem.clone(), enabled)?;
        self.env = Env {
            t_ambient: self.scenario.pack.initial_temp_k,
            t_coolant: None,
        };
        // Prime the new pack the same way construction did, so a restart leaves a node in
        // the same shape `new` does rather than in one with a stale reading.
        self.latest = pack.step(0.0, Demand::Rest, &self.env);
        self.pack = pack;
        self.sensor_faults_dropped = dropped;
        self.bms_enabled = enabled;
        self.edges.reset();
        self.edges.observe(self.latest.flags);
        self.accumulator.reset();
        Ok(())
    }

    /// Advance the pack by exactly `n_steps` of `dt`.
    ///
    /// **This is the deterministic path**, and it is the one the exit gate drives. Same
    /// scenario + same seed + same step count + same demand ⇒ bit-identical trajectory.
    /// [`Self::advance_real_time`] makes no such promise and cannot; see it for why.
    ///
    /// # Errors
    /// [`DriverError::OutOfRange`] for a non-finite or negative `dt`, a zero `n_steps`, or
    /// a non-finite demand; [`DriverError::TooManySteps`] for the cap.
    pub fn step_batch(
        &mut self,
        dt: f64,
        n_steps: u32,
        demand: Demand,
    ) -> Result<Advance, DriverError> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(DriverError::OutOfRange(format!(
                "dt must be finite and >= 0, got {dt}"
            )));
        }
        if n_steps == 0 {
            return Err(DriverError::OutOfRange(
                "n_steps must be >= 1; the node already holds a reading, so there is \
                 nothing a zero-step call could be asking for"
                    .into(),
            ));
        }
        if n_steps > MAX_STEPS_PER_CALL {
            return Err(DriverError::TooManySteps {
                asked: n_steps,
                cap: MAX_STEPS_PER_CALL,
            });
        }
        demand.check_finite()?;

        Ok(self.run(dt, n_steps, demand, 0.0, false))
    }

    /// Feed wall-clock time to the accumulator and take whatever whole steps it yields.
    ///
    /// # The determinism claim here is weaker, on purpose
    /// Phase 4's gate could assert bit-identity because both its legs were driven by
    /// explicit step counts. This path cannot, and pretending otherwise would be a lie:
    ///
    /// - **True:** same scenario + same seed + same *total step count* + same demand ⇒
    ///   bit-identical trajectory.
    /// - **False:** the same game run twice for the same wall-clock duration ⇒
    ///   bit-identical trajectory.
    ///
    /// The second is false because how many frames arrived, and how much time each
    /// carried, is a property of the machine. That is not a defect — it is exactly what
    /// `CLAUDE.md` principle 3 asks for. The frame rate does not define the *timestep*; it
    /// defines only how many fixed timesteps get consumed. Every step is still exactly
    /// `fixed_dt`.
    ///
    /// # Errors
    /// [`DriverError::OutOfRange`] for a non-finite or non-positive `fixed_dt`, a zero
    /// `max_steps`, or a non-finite demand. A hostile `delta_s` is absorbed by the
    /// accumulator rather than rejected — see [`Accumulator::take`].
    pub fn advance_real_time(
        &mut self,
        delta_s: f64,
        fixed_dt: f64,
        max_steps: u32,
        demand: Demand,
        backlog: Backlog,
    ) -> Result<Advance, DriverError> {
        if !fixed_dt.is_finite() || fixed_dt <= 0.0 {
            // Note `> 0`, not `>= 0`: a zero-length step is a deliberate telemetry read on
            // the explicit path, but a zero `fixed_dt` here would mean every frame divides
            // by zero and asks for infinitely many steps.
            return Err(DriverError::OutOfRange(format!(
                "fixed_dt must be finite and > 0, got {fixed_dt}"
            )));
        }
        if max_steps == 0 {
            return Err(DriverError::OutOfRange(
                "max_steps_per_frame must be >= 1; zero would freeze simulated time while \
                 the game kept running"
                    .into(),
            ));
        }
        demand.check_finite()?;

        let ticks = self.accumulator.take(delta_s, fixed_dt, max_steps, backlog);
        if ticks.steps == 0 {
            return Ok(Advance {
                steps: 0,
                edges: Edges::default(),
                telemetry: self.latest,
                backlog_s: ticks.backlog_s,
                capped: ticks.capped,
            });
        }
        Ok(self.run(fixed_dt, ticks.steps, demand, ticks.backlog_s, ticks.capped))
    }

    /// The one place steps are actually taken. Both public paths funnel through here so
    /// they cannot disagree about what a step does or how edges are folded.
    fn run(
        &mut self,
        dt: f64,
        n_steps: u32,
        demand: Demand,
        backlog_s: f64,
        capped: bool,
    ) -> Advance {
        let mut edges = Edges::default();
        for _ in 0..n_steps {
            self.latest = self.pack.step(dt, demand, &self.env);
            edges = edges.union(self.edges.observe(self.latest.flags));
        }
        Advance {
            steps: n_steps,
            edges,
            telemetry: self.latest,
            backlog_s,
            capped,
        }
    }

    /// Time carried by the accumulator, not yet consumed \[s\].
    #[must_use]
    pub fn pending_s(&self) -> f64 {
        self.accumulator.pending_s()
    }

    /// Ground truth for every cell, **series-major and parallel-minor**: the cell at
    /// series position `s`, parallel position `p` is at index `s * parallel + p`.
    ///
    /// Ground truth — every cell's true state, not what the BMS can sense. The gap between
    /// this and [`Telemetry::soc_bms`] is a feature to look at.
    #[must_use]
    pub fn cells(&self) -> Vec<CellView> {
        let (series, parallel) = (self.pack.series(), self.pack.parallel());
        let mut cells = Vec::with_capacity(usize::from(series) * usize::from(parallel));
        for s in 0..usize::from(series) {
            for p in 0..usize::from(parallel) {
                // In range by construction: both indices come from the pack's own
                // topology, which is why this cannot be an error path.
                if let Some(view) = self.pack.cell(s, p) {
                    cells.push(view);
                }
            }
        }
        cells
    }

    /// The whole engine state.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.pack.snapshot()
    }

    /// Replace the pack with a snapshot, in place.
    ///
    /// Refuses a differently-shaped pack, the same rule `sim-server` and `sim-wasm` apply
    /// and for the same reason: the overwhelmingly likely cause is the wrong snapshot, and
    /// swapping the pack under a running game silently is worse than saying no. The Godot
    /// case has an extra edge — the node's exported `series`/`parallel` facts would
    /// otherwise describe a pack it no longer holds.
    ///
    /// The check is knowingly partial. A snapshot carries its own chemistry, so an LFP
    /// node restored from a same-topology NMC snapshot passes and leaves the scenario
    /// naming a chemistry the pack no longer runs. Closing that would mean exposing the
    /// engine's chemistry purely to serve an adapter. The scenario is provenance; every
    /// live fact comes off the pack.
    ///
    /// The accumulator and edge detector are reset — see [`FlagEdges`] for what that
    /// means to a listener. The standing environment is deliberately *not*: it describes
    /// the room, not the pack.
    ///
    /// # Errors
    /// [`DriverError::Restore`] if the snapshot is not one this build can read,
    /// [`DriverError::TopologyMismatch`] if it describes a different pack shape.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<(), DriverError> {
        let mut restored = Pack::restore(snapshot)?;
        if restored.series() != self.pack.series() || restored.parallel() != self.pack.parallel() {
            return Err(DriverError::TopologyMismatch {
                got_series: restored.series(),
                got_parallel: restored.parallel(),
                want_series: self.pack.series(),
                want_parallel: self.pack.parallel(),
            });
        }
        // Prime the restored pack so `latest` describes it rather than the pack it
        // replaced. Free and non-mutating, exactly as in `new`.
        self.latest = restored.step(0.0, Demand::Rest, &self.env);
        self.pack = restored;
        self.edges.reset();
        self.accumulator.reset();
        Ok(())
    }

    /// Queue a fault to fire at a simulation time.
    ///
    /// Validation is the engine's: `Pack::schedule_fault` already rejects a non-finite
    /// `at_s`, a non-positive short resistance, and an out-of-topology cell index, so this
    /// translates rather than duplicates.
    ///
    /// # Errors
    /// [`DriverError::Fault`] with whatever the engine refused it for.
    pub fn schedule_fault(&mut self, at_s: f64, fault: Fault) -> Result<(), DriverError> {
        self.pack.schedule_fault(at_s, fault)?;
        Ok(())
    }

    /// Drop every fault that has not fired yet, returning how many. Already-active faults
    /// stay active — an internal short that has started is part of the pack now.
    pub fn clear_faults(&mut self) -> usize {
        self.pack.clear_faults()
    }

    /// Clear a latched BMS fault, returning whether there was one to clear.
    pub fn clear_bms_fault(&mut self) -> bool {
        self.pack.clear_bms_fault()
    }
}

/// The JSON-string surface, kept beside the typed one so [`crate::BatteryPack`] is a pure
/// forwarding layer.
///
/// Every one of these is exact in both directions: the workspace's `serde_json` carries
/// `float_roundtrip`, without which a snapshot handed to GDScript and handed back would
/// drift by one ULP and the resumed trajectory would stop being bit-identical.
impl PackDriver {
    /// [`Self::snapshot`] as JSON.
    ///
    /// # Errors
    /// [`DriverError::Json`] if serialization fails.
    pub fn snapshot_json(&self) -> Result<String, DriverError> {
        serde_json::to_string(&self.snapshot()).map_err(DriverError::json("serializing snapshot"))
    }

    /// [`Self::restore`] from JSON.
    ///
    /// # Errors
    /// [`DriverError::Json`] if the text is not a snapshot, plus everything
    /// [`Self::restore`] can fail with.
    pub fn restore_json(&mut self, snapshot_json: &str) -> Result<(), DriverError> {
        let snapshot: Snapshot = serde_json::from_str(snapshot_json)
            .map_err(DriverError::json("body is not a snapshot"))?;
        self.restore(&snapshot)
    }

    /// [`Self::cells`] as JSON.
    ///
    /// # Errors
    /// [`DriverError::Json`] if serialization fails.
    pub fn cells_json(&self) -> Result<String, DriverError> {
        serde_json::to_string(&self.cells()).map_err(DriverError::json("serializing cells"))
    }

    /// [`Self::schedule_fault`] with the fault arriving as JSON, externally tagged the way
    /// the scenario format already writes it:
    /// `{"SoftInternalShort": {"s": 1, "p": 0, "ohms": 5.0}}`.
    ///
    /// # Errors
    /// [`DriverError::Json`] if the text is not a fault, plus everything
    /// [`Self::schedule_fault`] can fail with.
    pub fn schedule_fault_json(&mut self, at_s: f64, fault_json: &str) -> Result<(), DriverError> {
        let fault: Fault =
            serde_json::from_str(fault_json).map_err(DriverError::json("body is not a fault"))?;
        self.schedule_fault(at_s, fault)
    }

    /// A demand from JSON, externally tagged the way every engine enum crosses a wire:
    /// `{"Current": -5.0}`, `"Rest"`.
    ///
    /// Deliberately the same dialect `sim-server` and `sim-wasm` speak — a project that
    /// drives the engine from both a game and a socket should not need two spellings for
    /// one demand.
    ///
    /// # Errors
    /// [`DriverError::Json`] if the text is not a demand.
    pub fn demand_from_json(demand_json: &str) -> Result<Demand, DriverError> {
        serde_json::from_str(demand_json).map_err(DriverError::json(
            "demand is not one of {\"Current\": A}, {\"Power\": W}, {\"Voltage\": V}, \"Rest\"",
        ))
    }
}
