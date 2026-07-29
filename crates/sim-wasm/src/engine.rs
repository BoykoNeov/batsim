//! The whole of this crate's behaviour, in plain Rust with no `wasm-bindgen` in sight.
//!
//! # Why the logic is not in the `#[wasm_bindgen]` type
//! `sim-wasm` stays inside `cargo test --workspace` and `clippy --all-targets`, which
//! means it is compiled and run for the **host** target. On the host a `JsError` is a
//! stub for a thing that does not exist, so any test that had to construct or inspect
//! one would be testing the stub. Keeping every decision in [`SimEngine`] — typed
//! errors, `Result` everywhere, no JS type in a signature — means `tests/engine.rs`
//! exercises the real translation layer, and [`crate::Sim`] is left with nothing but
//! `?` and a string conversion.
//!
//! # Where the duplication is, and why it is deliberate
//! The caps below mirror `sim_server::protocol`. They are copied rather than shared
//! because the alternative is depending on `sim-server` — which would pull `axum` and
//! `tokio` into a crate whose whole point is to run in a browser. The copy is small (two
//! constants) and the shapes it guards are pinned by tests on both sides.
//!
//! An earlier version of this comment promised that **a third client would trigger a
//! `sim-protocol` lift** of `Limits`/`StepCommand`/`Frame`. Phase 5 produced that third
//! client — `sim-godot` — and the promise turned out to name the wrong trigger, so it is
//! recorded here rather than quietly dropped. `sim-godot` wants none of those shapes:
//! it has no wire, so no `StepCommand` (GDScript calls a `#[func]` with typed arguments —
//! there is no message to parse or to `deny_unknown_fields` on); it exposes properties and
//! emits signals rather than returning a batch of samples, so no `Frame`; and it reports
//! only the latest state, so no decimation and no `frame_count`. Even the caps do not
//! unify: all three clients bound steps per call, but the server bounds a lock hold time,
//! this crate bounds main-thread occupancy in a tab, and `sim-godot` bounds a frame
//! budget. Three constants with three rationales are not one constant.
//!
//! So the rule is now a criterion rather than a count: **lift when a client needs the
//! wire *shapes*, not when the third client arrives.** See
//! `docs/plans/phase-5-godot.md`.
//!
//! What genuinely did recur — the finiteness checks — moved to where the rule actually
//! lives, which is `sim-core` itself; see below.
//!
//! # The check that only exists here — and where it went
//! Over the server's socket a non-finite number is unreachable — JSON has no literal
//! for `NaN` and `serde_json` refuses `1e400`, so the parser rejects it before
//! validation runs. Across the wasm boundary there is no parser: JS hands `dt` to this
//! crate as a raw `f64`, and `Number.NaN` arrives intact. `sim_server::protocol`'s
//! `validate_rejects_every_non_finite_field` was written for exactly this caller.
//!
//! GDScript is a third boundary with no parser, which made this the same rule in three
//! places. It is now `sim_core::Demand::check_finite` and `sim_core::Env::check_finite` —
//! a statement about what the engine's own types accept, not about any protocol, so it
//! needed no new crate. [`check_demand`] and [`check_env`] below are the mapping into
//! [`EngineError`], and that mapping is all that is left here.

use serde::Serialize;
use sim_core::{
    CellView, ChemistryParams, Demand, Env, Fault, NonFinite, Pack, Snapshot, Telemetry,
    SNAPSHOT_VERSION,
};
use sim_data::{parse_chemistry, parse_scenario, ChemistrySource, DataError, Scenario};

/// Most steps one [`SimEngine::step_many`] call may advance the pack.
///
/// The same number `sim-server` enforces per WebSocket message. It bounds how long a
/// single call can occupy the browser's main thread — the wasm module runs there, so an
/// uncapped call is a frozen tab rather than a slow response.
pub const MAX_STEPS_PER_CALL: u32 = 1_000_000;

/// Most telemetry frames one [`SimEngine::step_many`] call may produce.
///
/// Also the same number the server enforces. Here it bounds the JSON string this crate
/// builds and hands to JS, which is copied out of wasm memory in one piece.
pub const MAX_FRAMES_PER_CALL: u32 = 10_000;

/// How many frames a batch of `n_steps` decimated by `k` produces.
///
/// Steps are numbered `1..=n_steps`; a step is reported when `i % k == 0` **or** it is
/// the last step of the batch. That final-step rule is what makes this `div_ceil`, and
/// it is why a client's last sample is the true end state rather than wherever the
/// modulus happened to land. Identical to `sim_server::protocol::frame_count`.
#[must_use]
pub fn frame_count(n_steps: u32, report_every_n_steps: u32) -> u32 {
    n_steps.div_ceil(report_every_n_steps)
}

/// One reported step.
///
/// Field-for-field the same shape as `sim_server::protocol::Frame`, and that is the
/// point: the demo page plots frames from the embedded engine and frames from the
/// server's socket with the same code, so the two paths cannot quietly disagree about
/// what a sample is. `sim_time_s` is on the frame because a client that integrates `dt`
/// itself to know where it is will eventually be off by one step and draw it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Frame {
    /// 1-based index of this step **within its batch**. With decimation on, the gaps
    /// in this sequence are exactly the steps that were not reported.
    pub step: u32,
    /// Simulation time at the end of this step \[s\]. Absolute, not batch-relative.
    pub sim_time_s: f64,
    /// The step's telemetry.
    pub telemetry: Telemetry,
}

/// What a page needs to know about the pack it is driving, without a snapshot.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PackFacts {
    /// Series elements.
    pub series: u16,
    /// Parallel cells per group.
    pub parallel: u16,
    /// Simulation time elapsed \[s\].
    pub sim_time_s: f64,
    /// Whether the pack currently has a BMS. Read from the pack, not from the
    /// scenario, so it stays true across [`SimEngine::restart`] and a restore.
    pub has_bms: bool,
    /// Whether the **scenario** configures a BMS at all. When this is false the page's
    /// BMS toggle has nothing to turn back on — see [`SimEngine::restart`].
    pub scenario_has_bms: bool,
    /// How many of the scenario's faults were dropped because this build has no BMS to
    /// aim them at.
    ///
    /// Always zero on a pack built as the scenario authored it; non-zero only after
    /// `restart(false)` on a scenario that fault-injects a *sensor*. A page must
    /// surface this — see [`SimEngine::restart`] for why the alternative was neither
    /// "fail" nor "drop silently".
    pub sensor_faults_dropped: u32,
}

/// Ground truth for every cell, in the engine's own order.
#[derive(Clone, Debug, Serialize)]
pub struct Cells {
    /// Series elements.
    pub series: u16,
    /// Parallel cells per group.
    pub parallel: u16,
    /// Every cell, **series-major and parallel-minor**: the cell at series position
    /// `s`, parallel position `p` is at index `s * parallel + p`.
    ///
    /// Ground truth — every cell's true state, not what the BMS can sense. The gap
    /// between this and `Telemetry::soc_bms` is a feature to look at.
    pub cells: Vec<CellView>,
}

/// Everything the BMS measured, as the BMS sees it.
///
/// The counterpart to [`Cells`]: that is ground truth, this is belief. A page holding
/// both plus a [`Telemetry`] can draw every channel of the gap principle 8 exists to
/// expose. Field-for-field the same shape as `sim_server`'s `SensorsResponse`, for the
/// same reason [`Cells`] and its `CellsResponse` match — one engine, one dialect.
///
/// # Which of these actually lies, and when
/// `v_group` and `temp_probe_k` are **exact** reads of the true state at the sensed
/// positions: `sim_core`'s pack solve computes each group's node voltage and moves it
/// straight into the sensor frame. Their error is not in the value, it is in the
/// *sampling* — one voltage for a whole parallel group, and temperature only where a
/// probe sits. `i_pack_a` is the one always-wrong channel, carrying the configured
/// offset and a noise draw, and `soc_est` inherits that error by coulomb-counting it.
///
/// Injected sensor faults ([`Fault::SensorStuck`], [`Fault::SensorOffset`]) corrupt this
/// frame on top of all that, and are the only way a voltage or a probe temperature here
/// stops matching the truth.
///
/// # No `series` field
/// Unlike [`Cells`], which carries `parallel` because its consumer does index
/// arithmetic. `v_group.len()` *is* the series count and cannot disagree with itself; a
/// restated one could.
#[derive(Clone, Debug, Serialize)]
pub struct Sensors {
    /// Measured voltage of each parallel group \[V\], in series order.
    pub v_group: Vec<f64>,
    /// Measured temperature at each configured probe \[K\], in config order.
    pub temp_probe_k: Vec<f64>,
    /// Which cell each probe sits on, as `(series, parallel)`, in the same order as
    /// `temp_probe_k`.
    ///
    /// Static config rather than a measurement, and it rides here rather than in
    /// [`PackFacts`] only because that type is `Copy` and this is a `Vec`. It is what
    /// lets a client show the probes' *spatial* under-sampling — the reason
    /// `max_probe_k` and `Telemetry::t_max` part company — as positions rather than as
    /// a number.
    pub temp_probe_at: Vec<(u16, u16)>,
    /// Measured pack current \[A\], discharge-positive, including offset and noise.
    pub i_pack_a: f64,
    /// Simulation time at which this frame was sampled \[s\].
    ///
    /// Sampling is gated on `dt > 0`, so a zero-length probe read does **not** resample
    /// and this lags `PackFacts::sim_time_s` on a paused pack. That is the same
    /// zero-length-read contract that means a probe step fires no queued fault, and a
    /// client should say so rather than let a stale frame read as a broken one.
    pub sampled_at_s: f64,
    /// The BMS's own state-of-charge estimate, in \[0, 1\].
    ///
    /// The same number [`Telemetry::soc_bms`] reports; repeated here so this payload is
    /// a complete picture of what the BMS believes without a telemetry frame beside it.
    pub soc_est: f64,
    /// Whether the main contactor is latched open by a hard fault.
    pub contactor_open: bool,
}

/// Everything that can go wrong on this crate's surface.
///
/// One flat enum rather than per-method types: every one of these ends up as a string
/// in a browser, so the value of a rich type here is the message it carries.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
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
    /// A number a JS caller passed was outside the range this boundary accepts.
    ///
    /// This is the arm that exists because there is no JSON parser between JS and here.
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
    /// One call would have produced more frames than [`MAX_FRAMES_PER_CALL`].
    #[error("asked for {asked} frames, cap is {cap}; raise report_every_n_steps (currently {k}) — decimation drops reports, not steps, so the trajectory is unaffected")]
    TooManyFrames {
        /// Frames the call would have produced.
        asked: u32,
        /// The cap.
        cap: u32,
        /// The decimation factor that was in effect.
        k: u32,
    },
    /// The scenario names a chemistry id and no chemistry text was supplied.
    #[error("this scenario names chemistry {id:?}; fetch {id}.toml and pass its text alongside the scenario (a scenario that inlines `chemistry_toml` needs no second argument)")]
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
    #[error("snapshot is {got_series}S{got_parallel}P but this session runs {want_series}S{want_parallel}P; restoring it would silently replace the pack under everything watching it")]
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

impl EngineError {
    fn json(context: &'static str) -> impl FnOnce(serde_json::Error) -> Self {
        move |source| Self::Json { context, source }
    }
}

/// A scenario, its chemistry, and the pack the two of them built.
///
/// Owns the scenario text's parsed form permanently, which is what makes
/// [`Self::restart`] possible without the page re-fetching anything.
#[derive(Debug)]
pub struct SimEngine {
    /// The scenario **as authored**. Never mutated — [`Self::restart`] clones it and
    /// edits the clone, so `scenario.pack.bms` stays the answer to "what did the file
    /// ask for", which is what [`PackFacts::scenario_has_bms`] reports.
    scenario: Scenario,
    chem: ChemistryParams,
    bms_enabled: bool,
    pack: Pack,
    /// See [`PackFacts::sensor_faults_dropped`].
    sensor_faults_dropped: u32,
    /// The standing environment, seeded from the scenario's `initial_temp_k` with no
    /// coolant.
    ///
    /// Session state, not pack state: a snapshot describes a pack, and the room it sits
    /// in is the client's business — so a restore deliberately leaves this alone. Same
    /// rule `sim-server` applies to its own standing environment.
    env: Env,
}

impl SimEngine {
    /// Build a pack from scenario text.
    ///
    /// `chemistry_toml` is consulted **only** when the scenario names a chemistry id —
    /// a scenario that inlines `chemistry_toml` is self-contained and its own text
    /// wins, so a page that passes both cannot accidentally override a self-contained
    /// scenario with whatever it happened to have fetched.
    ///
    /// # Errors
    /// [`EngineError::Data`] if either text fails to parse or validate,
    /// [`EngineError::ChemistryNotSupplied`] if the scenario names an id and nothing
    /// was passed.
    pub fn new(scenario_toml: &str, chemistry_toml: Option<&str>) -> Result<Self, EngineError> {
        // `parse_scenario` validates; `Scenario` values built any other way do not.
        // This is the only constructor, so validation is not optional here.
        let scenario = parse_scenario(scenario_toml)?;
        let chem = match scenario.chemistry_source() {
            ChemistrySource::Inline(text) => parse_chemistry(text)?,
            ChemistrySource::Id(id) => match chemistry_toml {
                Some(text) if !text.trim().is_empty() => parse_chemistry(text)?,
                _ => {
                    return Err(EngineError::ChemistryNotSupplied { id: id.to_owned() });
                }
            },
        };

        let bms_enabled = scenario.pack.bms.is_some();
        let (pack, sensor_faults_dropped) = build(&scenario, chem.clone(), bms_enabled)?;
        let env = Env {
            t_ambient: scenario.pack.initial_temp_k,
            t_coolant: None,
        };
        Ok(Self {
            scenario,
            chem,
            bms_enabled,
            pack,
            sensor_faults_dropped,
            env,
        })
    }

    /// Which chemistry a scenario needs, so a page can fetch it before constructing.
    ///
    /// `Ok(None)` means the scenario inlines its chemistry and needs nothing fetched.
    /// The id is already known to match `[a-z0-9_]+` — [`parse_scenario`] enforces that
    /// — so a page may interpolate it into a URL without escaping it.
    ///
    /// # Errors
    /// [`EngineError::Data`] if the scenario does not parse or validate. Note this
    /// means a page learns about a broken scenario here rather than two fetches later.
    pub fn chemistry_id_of(scenario_toml: &str) -> Result<Option<String>, EngineError> {
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

    /// The standing environment.
    #[must_use]
    pub fn env(&self) -> Env {
        self.env
    }

    /// Replace the standing environment.
    ///
    /// # Errors
    /// [`EngineError::OutOfRange`] if either temperature is not finite. Reachable:
    /// these arrive from JS as raw `f64`s with no parser in between.
    pub fn set_env(&mut self, env: Env) -> Result<(), EngineError> {
        check_env(env)?;
        self.env = env;
        Ok(())
    }

    /// Rebuild the pack from the scenario, with or without its BMS.
    ///
    /// **This restarts the run**: simulation time returns to zero, every cell returns
    /// to the scenario's initial SOC and temperature, the RNG returns to its seed, and
    /// the scenario's faults are re-queued. That is not a limitation being apologised
    /// for — a BMS is part of what a pack *is*, and there is no honest way to grow one
    /// onto a pack that has been running without it. Contrasting the same scenario with
    /// and without protection is the teaching case, and it is a comparison of two runs.
    ///
    /// `enabled = true` restores whatever the scenario configured, which is *nothing*
    /// if the scenario ships no `[pack.bms]` section — see
    /// [`PackFacts::scenario_has_bms`], which is what a page should disable its toggle
    /// on.
    ///
    /// The standing environment is reset too, because it was seeded from the scenario
    /// and this is a return to the scenario.
    ///
    /// # Sensor faults have nowhere to land without a BMS
    /// Found by running this, not by reasoning about it: removing the BMS from
    /// `scenarios/soft_short_under_a_lying_sensor.toml` makes its own
    /// `SensorOffset { GroupVoltage(1) }` unschedulable — `Pack::schedule_fault` returns
    /// `NoSuchSensor`, because sensors *belong to* the BMS. The whole sensor layer is
    /// the BMS's instrumentation; with no BMS there is no instrument to lie.
    ///
    /// So `restart(false)` drops the scenario's sensor faults rather than failing, and
    /// **counts them** into [`PackFacts::sensor_faults_dropped`] so a page can say
    /// which of the scenario's misfortunes it is no longer reproducing. Failing instead
    /// would make the toggle unusable on exactly the scenario it exists to illuminate;
    /// dropping them silently would let a student compare two runs that differ in more
    /// ways than the label claims.
    ///
    /// A scenario that ships **no** BMS and a sensor fault is a different thing — an
    /// authoring error — and is left to fail loudly at construction. Only the removal
    /// of a BMS the scenario actually configured triggers the filter.
    ///
    /// # Errors
    /// [`EngineError::Data`] if the rebuild fails, which given the same inputs
    /// succeeded once already means something is wrong with this crate, not the input.
    pub fn restart(&mut self, enabled: bool) -> Result<(), EngineError> {
        let (pack, dropped) = build(&self.scenario, self.chem.clone(), enabled)?;
        self.pack = pack;
        self.sensor_faults_dropped = dropped;
        self.bms_enabled = enabled;
        self.env = Env {
            t_ambient: self.scenario.pack.initial_temp_k,
            t_coolant: None,
        };
        Ok(())
    }

    /// Whether the last [`Self::restart`] asked for the scenario's BMS.
    #[must_use]
    pub fn bms_enabled(&self) -> bool {
        self.bms_enabled
    }

    /// Advance the pack and return the reported frames.
    ///
    /// The batch runs under the standing environment; there is deliberately no
    /// per-batch override here, unlike the server's `Step` command. A page owns its
    /// environment as a control, so an override it would have to re-send on every
    /// animation frame is a footgun with no user.
    ///
    /// There is likewise no stored "latest telemetry". A pack that has not stepped
    /// honestly has none, and this crate will not synthesise one — the same call
    /// `sim-server` makes for its `latest_telemetry` field. A page that wants a reading
    /// on load asks for `dt = 0, n_steps = 1`, which the engine's zero-length-step
    /// contract makes free and non-mutating.
    ///
    /// # Errors
    /// [`EngineError::OutOfRange`] for a non-finite or negative `dt`, a zero `n_steps`,
    /// a zero `report_every_n_steps`, or a non-finite demand;
    /// [`EngineError::TooManySteps`] and [`EngineError::TooManyFrames`] for the two
    /// caps. The frame cap is rejected rather than truncated, and its message names the
    /// knob that fixes it.
    pub fn step_many(
        &mut self,
        dt: f64,
        n_steps: u32,
        demand: Demand,
        report_every_n_steps: u32,
    ) -> Result<Vec<Frame>, EngineError> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(EngineError::OutOfRange(format!(
                "dt must be finite and >= 0, got {dt}"
            )));
        }
        if n_steps == 0 {
            return Err(EngineError::OutOfRange(
                "n_steps must be >= 1; to read telemetry without advancing, pass \
                 n_steps = 1 with dt = 0"
                    .into(),
            ));
        }
        if n_steps > MAX_STEPS_PER_CALL {
            return Err(EngineError::TooManySteps {
                asked: n_steps,
                cap: MAX_STEPS_PER_CALL,
            });
        }
        if report_every_n_steps == 0 {
            return Err(EngineError::OutOfRange(
                "report_every_n_steps must be >= 1".into(),
            ));
        }
        let frames = frame_count(n_steps, report_every_n_steps);
        if frames > MAX_FRAMES_PER_CALL {
            return Err(EngineError::TooManyFrames {
                asked: frames,
                cap: MAX_FRAMES_PER_CALL,
                k: report_every_n_steps,
            });
        }
        check_demand(demand)?;

        let mut out = Vec::with_capacity(frames as usize);
        for step in 1..=n_steps {
            let telemetry = self.pack.step(dt, demand, &self.env);
            if step % report_every_n_steps == 0 || step == n_steps {
                out.push(Frame {
                    step,
                    // Read after the step, so a frame describes the pack at the end of
                    // the step that produced it.
                    sim_time_s: self.pack.sim_time_s(),
                    telemetry,
                });
            }
        }
        Ok(out)
    }

    /// Ground truth for every cell.
    #[must_use]
    pub fn cells(&self) -> Cells {
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
        Cells {
            series,
            parallel,
            cells,
        }
    }

    /// What the BMS measured, or `None` for a pack that has no BMS.
    ///
    /// `None` is a supported mode, not an error: a pack with no BMS has no sensors, and
    /// running one that way is one of the engine's teaching scenarios. See [`Sensors`]
    /// for which of these channels lie and when.
    #[must_use]
    pub fn sensors(&self) -> Option<Sensors> {
        let bms = self.pack.bms()?;
        let frame = bms.sensors();
        Some(Sensors {
            v_group: frame.v_group.clone(),
            temp_probe_k: frame.temp_probe_k.clone(),
            temp_probe_at: bms.config().temp_probes.clone(),
            i_pack_a: frame.i_pack_a,
            sampled_at_s: frame.sampled_at_s,
            soc_est: bms.soc_estimate(),
            contactor_open: bms.contactor_open(),
        })
    }

    /// The whole engine state.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.pack.snapshot()
    }

    /// Replace the pack with a snapshot, in place.
    ///
    /// Refuses a differently-shaped pack, which is the same rule
    /// `sim-server`'s `POST /sessions/{id}/snapshot` applies and for the same reason:
    /// the overwhelmingly likely cause is the wrong snapshot file, and swapping the
    /// pack under a running plot silently is worse than saying no.
    ///
    /// The check is knowingly partial. A snapshot carries its own chemistry, so an LFP
    /// session restored from a same-topology NMC snapshot passes and leaves
    /// [`Self::scenario`] naming a chemistry the pack no longer runs. Closing that
    /// would mean exposing the engine's chemistry purely to serve an adapter. The
    /// scenario is provenance; every live fact comes off the pack.
    ///
    /// # Errors
    /// [`EngineError::Restore`] if the snapshot is not one this build can read,
    /// [`EngineError::TopologyMismatch`] if it describes a different pack shape.
    pub fn restore(&mut self, snapshot: &Snapshot) -> Result<(), EngineError> {
        let restored = Pack::restore(snapshot)?;
        if restored.series() != self.pack.series() || restored.parallel() != self.pack.parallel() {
            return Err(EngineError::TopologyMismatch {
                got_series: restored.series(),
                got_parallel: restored.parallel(),
                want_series: self.pack.series(),
                want_parallel: self.pack.parallel(),
            });
        }
        self.pack = restored;
        Ok(())
    }

    /// Queue a fault to fire at a simulation time.
    ///
    /// Validation is the engine's: `Pack::schedule_fault` already rejects a non-finite
    /// `at_s`, a non-positive short resistance, and an out-of-topology cell index, so
    /// this translates rather than duplicates.
    ///
    /// # Errors
    /// [`EngineError::Fault`] with whatever the engine refused it for.
    pub fn schedule_fault(&mut self, at_s: f64, fault: Fault) -> Result<(), EngineError> {
        self.pack.schedule_fault(at_s, fault)?;
        Ok(())
    }

    /// Drop every fault that has not fired yet, returning how many. Already-active
    /// faults stay active — an internal short that has started is part of the pack now.
    pub fn clear_faults(&mut self) -> usize {
        self.pack.clear_faults()
    }

    /// Clear a latched BMS fault, returning whether there was one to clear.
    pub fn clear_bms_fault(&mut self) -> bool {
        self.pack.clear_bms_fault()
    }
}

/// Build a pack from a scenario, optionally without its BMS, reporting how many of the
/// scenario's faults had to be dropped for want of a sensor to aim them at.
///
/// This was this crate's own logic until Phase 5 needed the identical behaviour in
/// `sim-godot`; it now lives on [`Scenario`] itself, where the statement it makes belongs.
/// See [`Scenario::build_pack_with_bms`] for the reasoning that used to be here, and
/// [`SimEngine::restart`] for what it means to a caller.
fn build(
    scenario: &Scenario,
    chem: ChemistryParams,
    bms_enabled: bool,
) -> Result<(Pack, u32), DataError> {
    scenario.build_pack_with_bms(chem, bms_enabled)
}

/// Every `f64` a demand can carry must be finite.
///
/// The rule is `sim-core`'s — see [`Demand::check_finite`]. This is the mapping into this
/// crate's error type, which is the only part that is this crate's business.
fn check_demand(demand: Demand) -> Result<(), EngineError> {
    demand.check_finite().map_err(non_finite)
}

/// Every `f64` an environment can carry must be finite.
fn check_env(env: Env) -> Result<(), EngineError> {
    env.check_finite().map_err(non_finite)
}

/// The one place a [`NonFinite`] becomes an [`EngineError`].
fn non_finite(error: NonFinite) -> EngineError {
    EngineError::OutOfRange(error.to_string())
}

/// The JSON-string surface, kept beside the typed one so [`crate::Sim`] is a pure
/// forwarding layer.
///
/// Every one of these is exact in both directions: the workspace's `serde_json` carries
/// `float_roundtrip`, without which a snapshot handed to JS and handed back would drift
/// by one ULP and the resumed trajectory would stop being bit-identical.
impl SimEngine {
    /// [`Self::step_many`] with the demand arriving as JSON and the frames leaving as
    /// JSON.
    ///
    /// The demand is externally tagged, the way every engine enum already crosses a
    /// wire: `{"Current": -5.0}`, `"Rest"`. Deliberately not a friendlier shape — a
    /// page that talks to both this crate and `sim-server` would otherwise need two
    /// dialects for one engine.
    ///
    /// # Errors
    /// [`EngineError::Json`] if the demand does not parse, plus everything
    /// [`Self::step_many`] can fail with.
    pub fn step_many_json(
        &mut self,
        dt: f64,
        n_steps: u32,
        demand_json: &str,
        report_every_n_steps: u32,
    ) -> Result<String, EngineError> {
        let demand: Demand = serde_json::from_str(demand_json).map_err(EngineError::json(
            "demand is not one of {\"Current\": A}, {\"Power\": W}, {\"Voltage\": V}, \"Rest\"",
        ))?;
        let frames = self.step_many(dt, n_steps, demand, report_every_n_steps)?;
        serde_json::to_string(&frames).map_err(EngineError::json("serializing frames"))
    }

    /// [`Self::facts`] as JSON.
    ///
    /// # Errors
    /// [`EngineError::Json`] — not reachable for this shape, but the surface stays
    /// uniform rather than hiding one `unwrap` among nine `?`s.
    pub fn facts_json(&self) -> Result<String, EngineError> {
        serde_json::to_string(&self.facts()).map_err(EngineError::json("serializing pack facts"))
    }

    /// [`Self::cells`] as JSON.
    ///
    /// # Errors
    /// [`EngineError::Json`] if serialization fails.
    pub fn cells_json(&self) -> Result<String, EngineError> {
        serde_json::to_string(&self.cells()).map_err(EngineError::json("serializing cells"))
    }

    /// [`Self::sensors`] as JSON, or the literal `null` for a pack with no BMS.
    ///
    /// The `null` is the payload, not a failure: JS gets `null` from `JSON.parse` and
    /// can branch on it directly, which is exactly what the `Option` means.
    ///
    /// Non-finite values are unreachable here even though `sim_core` uses `f64::NAN` as
    /// its probe-read fallback and JSON cannot spell one: `Pack::new` rejects any probe
    /// outside the topology, so the fallback never fires. Guarding it anyway would imply
    /// that range check is not trusted — [`Self::cells_json`] guards nothing for the
    /// same reason.
    ///
    /// # Errors
    /// [`EngineError::Json`] if serialization fails.
    pub fn sensors_json(&self) -> Result<String, EngineError> {
        serde_json::to_string(&self.sensors()).map_err(EngineError::json("serializing sensors"))
    }

    /// [`Self::env`] as JSON.
    ///
    /// # Errors
    /// [`EngineError::Json`] if serialization fails.
    pub fn env_json(&self) -> Result<String, EngineError> {
        serde_json::to_string(&self.env()).map_err(EngineError::json("serializing env"))
    }

    /// [`Self::snapshot`] as JSON. Exact — see this block's note.
    ///
    /// # Errors
    /// [`EngineError::Json`] if serialization fails.
    pub fn snapshot_json(&self) -> Result<String, EngineError> {
        serde_json::to_string(&self.snapshot()).map_err(EngineError::json("serializing snapshot"))
    }

    /// [`Self::restore`] from JSON. Exact — see this block's note.
    ///
    /// # Errors
    /// [`EngineError::Json`] if the text is not a snapshot, plus everything
    /// [`Self::restore`] can fail with.
    pub fn restore_json(&mut self, snapshot_json: &str) -> Result<(), EngineError> {
        let snapshot: Snapshot = serde_json::from_str(snapshot_json)
            .map_err(EngineError::json("body is not a snapshot"))?;
        self.restore(&snapshot)
    }

    /// [`Self::schedule_fault`] with the fault arriving as JSON, externally tagged the
    /// way the scenario format already writes it:
    /// `{"SoftInternalShort": {"s": 1, "p": 0, "ohms": 5.0}}`.
    ///
    /// # Errors
    /// [`EngineError::Json`] if the text is not a fault, plus everything
    /// [`Self::schedule_fault`] can fail with.
    pub fn schedule_fault_json(&mut self, at_s: f64, fault_json: &str) -> Result<(), EngineError> {
        let fault: Fault =
            serde_json::from_str(fault_json).map_err(EngineError::json("body is not a fault"))?;
        self.schedule_fault(at_s, fault)
    }
}
