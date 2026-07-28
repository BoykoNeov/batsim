//! `sim-godot` — a GDExtension exposing `sim-core` as a Godot `BatteryPack` node.
//!
//! # Requires Godot 4.7 or newer
//! The `godot` dependency pins `api-4-7`; see `Cargo.toml` for why that is a support
//! decision and not only a reproducibility one.
//!
//! # Where the behaviour is
//! All of it is in [`driver`], which contains no `godot` type in any signature and is
//! therefore host-testable by `cargo test --workspace`. This module is the shell: it
//! converts, forwards, and emits. If a decision appears here, it is in the wrong file.
//!
//! # Loading it
//! Godot needs a one-time bootstrap per clone, because `.godot/` is gitignored:
//!
//! ```text
//! cargo build -p sim-godot
//! godot --headless --path godot --import      # writes .godot/extension_list.cfg
//! ```
//!
//! A *rebuilt* cdylib needs no re-import — verified, not assumed. Only the first run on a
//! fresh tree does.

pub mod driver;

use godot::prelude::*;

use driver::{Advance, Backlog, DriverError, PackDriver};
use sim_core::{Demand, Env, EventFlags};

struct BatsimExtension;

#[gdextension]
unsafe impl ExtensionLibrary for BatsimExtension {}

/// A battery pack, simulated by `sim-core`.
///
/// # Two ways to drive it, and only one of them is reproducible
/// [`Self::step_batch`] advances an exact number of steps and is **deterministic**: same
/// scenario, same seed, same step count, same demand ⇒ bit-identical trajectory. It is
/// what the exit gate drives and what a script or a fast-forward should use.
///
/// `_physics_process` (enabled by [`Self::auto_step`]) drives the same engine from
/// wall-clock time through a fixed-`dt` accumulator. Every step is still exactly
/// [`Self::fixed_dt`] — the frame rate never sets the timestep — but *how many* steps a
/// given wall-clock second produces depends on the machine, so two runs of equal duration
/// are not equal trajectories. That is the design, not a defect.
///
/// # The borrow discipline, which is structural rather than stylistic
/// A signal handler in GDScript may call straight back into this node. If that happens
/// while a `&mut` borrow of the driver is live, `godot-cell` panics **at runtime** — there
/// is no compile-time error to catch it. So every method here follows one shape:
///
/// 1. do the driver work, collecting what needs announcing into a local;
/// 2. let the borrow end;
/// 3. emit.
///
/// Written this way from the start because the failure only shows up once a demo scene
/// connects a handler, which is several slices after the code that causes it.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct BatteryPack {
    base: Base<Node>,
    /// `None` until a scenario is loaded — a node dropped into a scene with no
    /// configuration is a normal editor state, not an error.
    driver: Option<PackDriver>,
    /// Last failure, for GDScript to read back. Godot has no `Result`, so a `#[func]`
    /// returns a bool or a sentinel and the detail is fetched from here — the same
    /// bargain `sim-wasm` struck with `JsError`, minus the exception.
    last_error: String,

    /// Scenario TOML. Set it in the inspector, or hand text to
    /// [`Self::load_scenario`] — this crate takes **text, not paths**, because a
    /// `res://` path is not a filesystem path once a game is exported into a `.pck`.
    #[export]
    #[var(hint = MULTILINE_TEXT)]
    scenario_toml: GString,
    /// Chemistry TOML. Leave empty when the scenario inlines its chemistry.
    #[export]
    #[var(hint = MULTILINE_TEXT)]
    chemistry_toml: GString,

    /// The physics timestep \[s\]. **The only thing that sets a step's size.**
    ///
    /// Deliberately not derived from the frame delta: `CLAUDE.md` forbids letting a
    /// client's frame rate define the timestep, and `Engine.time_scale` alone would
    /// otherwise change the physics.
    #[export]
    fixed_dt: f64,
    /// Most steps one physics frame may consume. Bounds the worst case a single long
    /// frame can cost — without it, a level load hands the node a huge delta and the
    /// pack does thousands of steps inside one frame.
    #[export]
    max_steps_per_frame: i64,
    /// How far SOC must move before [`Self::signals`]' `soc_changed` fires.
    ///
    /// Without a threshold this is a per-frame signal, because SOC changes every step.
    #[export]
    soc_signal_epsilon: f64,
    /// Whether `_physics_process` drives the pack. Off by default: a node in a scene
    /// should not start simulating because it was instantiated.
    #[export]
    auto_step: bool,
    /// What to do with time the per-frame cap would not let us consume. See
    /// [`BacklogPolicy`].
    #[export]
    backlog_policy: BacklogPolicy,
    /// The standing demand, as externally-tagged JSON: `{"Current": -5.0}`, `"Rest"`.
    ///
    /// Used by `_physics_process` only; [`Self::step_batch`] takes its own.
    #[export]
    #[var(hint = MULTILINE_TEXT)]
    demand_json: GString,

    /// SOC at the last `soc_changed` emission, for the epsilon gate.
    last_announced_soc: f64,
}

/// What to do with wall-clock time the per-frame cap would not let us consume.
///
/// Exported rather than hardcoded because the two answers suit different games and
/// neither is obviously right — see `docs/plans/phase-5-godot.md`. What is *not*
/// negotiable is that the condition is announced: sim time silently dilating is
/// indistinguishable from a physics bug to whoever is looking at the plot, which is why
/// `falling_behind` fires under both policies.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[godot(via = GString)]
pub enum BacklogPolicy {
    /// Throw the un-consumed time away: sim time falls behind wall time and stays
    /// behind, and the game stays responsive.
    #[default]
    Drop,
    /// Work it off over later frames: sim time stays true to wall time, and a long stall
    /// can cascade into several capped frames.
    Repay,
}

impl From<BacklogPolicy> for Backlog {
    fn from(policy: BacklogPolicy) -> Self {
        match policy {
            BacklogPolicy::Drop => Self::Drop,
            BacklogPolicy::Repay => Self::Repay,
        }
    }
}

#[godot_api]
impl INode for BatteryPack {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            driver: None,
            last_error: String::new(),
            scenario_toml: GString::new(),
            chemistry_toml: GString::new(),
            // 50 Hz — a round number near Godot's 60 Hz default physics tick, so the
            // out-of-the-box accumulator carries a small remainder rather than none.
            // Anything is legal; this one is only a starting point.
            fixed_dt: 0.02,
            // Matches Godot's own `max_physics_steps_per_frame` default, so the node's
            // catch-up budget and the engine's agree until a game changes one.
            max_steps_per_frame: 8,
            // 0.1 % of full charge. At a 1 C discharge that is a signal roughly every
            // 3.6 s of simulated time rather than every frame.
            soc_signal_epsilon: 0.001,
            auto_step: false,
            backlog_policy: BacklogPolicy::Drop,
            demand_json: GString::from("\"Rest\""),
            last_announced_soc: f64::NAN,
        }
    }

    /// Drive the pack from wall-clock time, in whole `fixed_dt` steps.
    ///
    /// The accumulator is the driver's; this method's only jobs are to decide whether to
    /// run at all, to convert, and to emit. See [`BatteryPack`]'s borrow-discipline note
    /// for why the emission happens after the driver work rather than inside it.
    fn physics_process(&mut self, delta: f64) {
        if !self.auto_step || self.driver.is_none() {
            return;
        }
        let demand = match PackDriver::demand_from_json(&self.demand_json.to_string()) {
            Ok(demand) => demand,
            Err(error) => {
                // A bad demand every frame would be a log flood; report it once and stop
                // stepping rather than spamming.
                self.last_error = error.to_string();
                self.auto_step = false;
                return;
            }
        };
        let fixed_dt = self.fixed_dt;
        let max_steps = u32::try_from(self.max_steps_per_frame).unwrap_or(u32::MAX);
        let backlog = self.backlog_policy.into();

        let outcome = {
            let Some(driver) = self.driver.as_mut() else {
                return;
            };
            driver.advance_real_time(delta, fixed_dt, max_steps, demand, backlog)
        };
        match outcome {
            Ok(advance) => self.announce(advance),
            Err(error) => {
                self.last_error = error.to_string();
                self.auto_step = false;
            }
        }
    }
}

impl BatteryPack {
    /// Record a failure and report it as `false`, so every fallible `#[func]` reads the
    /// same way from GDScript: check the bool, then read `last_error`.
    fn fail(&mut self, error: &DriverError) -> bool {
        self.last_error = error.to_string();
        false
    }

    /// Turn one batch's outcome into signals.
    ///
    /// **Called with no borrow of `self.driver` outstanding.** A GDScript handler may
    /// call straight back into this node, and doing that while a `&mut` borrow is live
    /// makes `godot-cell` panic at runtime with nothing at compile time to catch it.
    fn announce(&mut self, advance: Advance) {
        if advance.capped {
            self.signals().falling_behind().emit(advance.backlog_s);
        }
        if advance.steps == 0 {
            return;
        }

        let (rising, falling) = (advance.edges.rising, advance.edges.falling);
        if !advance.edges.is_empty() {
            self.signals()
                .flags_changed()
                .emit(i64::from(rising.bits()), i64::from(falling.bits()));
        }

        // Rising edges only. `EventFlags` is recomputed every step, so emitting whenever
        // a flag is *set* would emit for the whole duration of a condition — a 60 Hz
        // signal storm that makes the feature useless.
        if rising.intersects(PROTECTION) {
            // `emit` takes a `GString` argument by reference (`AsArg`), not by value.
            self.signals().protection_tripped().emit(
                i64::from((rising & PROTECTION).bits()),
                &GString::from(&names_of(rising & PROTECTION)),
            );
        }
        if falling.intersects(PROTECTION) {
            self.signals()
                .protection_cleared()
                .emit(i64::from((falling & PROTECTION).bits()));
        }
        if rising.contains(EventFlags::THERMAL_RUNAWAY) {
            self.signals().thermal_runaway_started().emit();
        }
        if rising.contains(EventFlags::VENTED) {
            self.signals().vented().emit();
        }
        if rising.contains(EventFlags::CONTACTOR_OPEN) {
            self.signals().contactor_opened().emit();
        }
        if falling.contains(EventFlags::CONTACTOR_OPEN) {
            self.signals().contactor_closed().emit();
        }

        let soc = advance.telemetry.soc_true;
        // `NAN` on the first call, and `NAN >= x` is false, so the first emission is
        // driven by the `is_nan` arm rather than by the comparison.
        if self.last_announced_soc.is_nan()
            || (soc - self.last_announced_soc).abs() >= self.soc_signal_epsilon
        {
            self.last_announced_soc = soc;
            self.signals().soc_changed().emit(soc);
        }
    }
}

/// The flags a game should hear about as "protection tripped".
///
/// `SOC_CLAMPED_*` are deliberately not here: they mean the engine truncated an
/// over-charge or over-discharge attempt, which is a modelling clamp rather than a
/// protection device acting. `BALANCING` is not a trip either. Both still reach a listener
/// through `flags_changed`, which is why that general signal exists.
const PROTECTION: EventFlags = EventFlags::OV
    .union(EventFlags::UV)
    .union(EventFlags::OC)
    .union(EventFlags::OT)
    .union(EventFlags::UT)
    .union(EventFlags::CONTACTOR_OPEN);

/// `" | "`-joined flag names, the same text the socket uses.
fn names_of(flags: EventFlags) -> String {
    flags
        .iter_names()
        .map(|(name, _)| name)
        .collect::<Vec<_>>()
        .join(" | ")
}

#[godot_api]
impl BatteryPack {
    /// Any flag transition at all, as two bitmasks.
    ///
    /// The general channel, so nothing the engine can raise is unreachable from GDScript
    /// even though only some flags get a signal of their own. Within one batch the
    /// transitions are unioned and reported once — a `step_batch(10_000)` that emitted per
    /// step would push thousands of signals into a game's main loop.
    #[signal]
    fn flags_changed(rising_bits: i64, falling_bits: i64);

    /// A protection condition began. Rising edges only.
    #[signal]
    fn protection_tripped(flags_bits: i64, flags_text: GString);

    /// A protection condition ended.
    #[signal]
    fn protection_cleared(flags_bits: i64);

    /// Thermal runaway began on at least one cell.
    #[signal]
    fn thermal_runaway_started();

    /// A cell vented.
    #[signal]
    fn vented();

    /// The main contactor opened — BMS protection, or an explicit command.
    #[signal]
    fn contactor_opened();

    /// The main contactor closed again.
    #[signal]
    fn contactor_closed();

    /// Ground-truth SOC moved by at least [`Self::soc_signal_epsilon`].
    #[signal]
    fn soc_changed(soc: f64);

    /// A physics frame hit [`Self::max_steps_per_frame`] and could not consume all the
    /// time it was given; `backlog_s` is what was dropped or is still owed, depending on
    /// [`Self::backlog_policy`].
    ///
    /// Fires under **both** policies. Simulated time falling behind wall time is the kind
    /// of thing that reads as a physics bug when it happens silently.
    #[signal]
    fn falling_behind(backlog_s: f64);

    /// Build a pack from scenario TOML, returning whether it worked.
    ///
    /// `chemistry_toml` may be empty when the scenario inlines its chemistry; use
    /// [`Self::chemistry_id_of`] first to find out whether a file needs reading.
    ///
    /// GDScript reads the texts itself — `FileAccess.get_file_as_string("res://…")` —
    /// because this crate takes **text, not paths**. A `res://` path is not a filesystem
    /// path once a game is exported into a `.pck`, so a node that took a path would work
    /// in the editor and fail in a shipped build.
    #[func]
    fn load_scenario(&mut self, scenario_toml: GString, chemistry_toml: GString) -> bool {
        let scenario = scenario_toml.to_string();
        let chemistry = chemistry_toml.to_string();
        let chemistry = (!chemistry.trim().is_empty()).then_some(chemistry);
        match PackDriver::new(&scenario, chemistry.as_deref()) {
            Ok(driver) => {
                self.driver = Some(driver);
                self.last_error = String::new();
                true
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Which chemistry a scenario needs, so a game can read that file before loading.
    ///
    /// Returns the id, or an empty string if the scenario inlines its chemistry **or**
    /// does not parse — check [`Self::last_error`] to tell those apart.
    #[func]
    fn chemistry_id_of(&mut self, scenario_toml: GString) -> GString {
        match PackDriver::chemistry_id_of(&scenario_toml.to_string()) {
            Ok(Some(id)) => {
                self.last_error = String::new();
                GString::from(&id)
            }
            Ok(None) => {
                self.last_error = String::new();
                GString::new()
            }
            Err(error) => {
                self.last_error = error.to_string();
                GString::new()
            }
        }
    }

    /// Whether a scenario has been loaded.
    #[func]
    fn has_scenario(&self) -> bool {
        self.driver.is_some()
    }

    /// The last failure, or an empty string.
    #[func]
    fn last_error(&self) -> GString {
        GString::from(&self.last_error)
    }

    /// Advance the pack by exactly `n_steps` of `dt`, returning whether it worked.
    ///
    /// This is the deterministic path: same scenario + same seed + same step count + same
    /// demand ⇒ bit-identical trajectory. The accumulator in `_physics_process`
    /// (slice C) makes no such promise and cannot.
    ///
    /// `demand_json` is externally tagged, the dialect every engine enum already crosses a
    /// wire in: `{"Current": -5.0}`, `"Rest"`.
    #[func]
    fn step_batch(&mut self, dt: f64, n_steps: i64, demand_json: GString) -> bool {
        let demand = match PackDriver::demand_from_json(&demand_json.to_string()) {
            Ok(demand) => demand,
            Err(error) => return self.fail(&error),
        };
        let Ok(n_steps) = u32::try_from(n_steps) else {
            let error = DriverError::OutOfRange(format!("n_steps must fit in u32, got {n_steps}"));
            return self.fail(&error);
        };
        self.advance(dt, n_steps, demand)
    }

    /// Simulated time elapsed \[s\], or `-1.0` with no scenario loaded.
    ///
    /// The sentinel is deliberate and deliberately impossible: simulation time is never
    /// negative, so a game can tell "no pack" from "a pack at t = 0" without a second
    /// call.
    #[func]
    fn sim_time_s(&self) -> f64 {
        self.driver.as_ref().map_or(-1.0, PackDriver::sim_time_s)
    }

    /// Terminal voltage \[V\], or `NAN` with no scenario loaded.
    ///
    /// Never synthetic: the driver takes one zero-length step at construction so this
    /// answers a real question about a real pack even before anything has run.
    #[func]
    fn v_terminal(&self) -> f64 {
        self.driver
            .as_ref()
            .map_or(f64::NAN, |d| d.latest().v_terminal)
    }

    /// Ground-truth pack SOC in \[0, 1\], or `NAN` with no scenario loaded.
    #[func]
    fn soc_true(&self) -> f64 {
        self.driver
            .as_ref()
            .map_or(f64::NAN, |d| d.latest().soc_true)
    }

    /// Event flags as a bitmask.
    ///
    /// Deliberately different from the socket, where `EventFlags` crosses as a
    /// `" | "`-joined name string: a game wants to mask-test cheaply, a browser wanted to
    /// print. Both spellings exist so neither client has to parse the other's choice —
    /// see [`Self::flags_text`].
    #[func]
    fn flags_bits(&self) -> i64 {
        self.driver
            .as_ref()
            .map_or(0, |d| i64::from(d.latest().flags.bits()))
    }

    /// Event flags as a human-readable string, `" | "`-joined, `""` for none.
    ///
    /// Built from `iter_names` rather than from the flags' serde output, which produces
    /// the identical text but would arrive wrapped in JSON quotes that a caller would
    /// then have to strip. See [`Self::flags_bits`].
    #[func]
    fn flags_text(&self) -> GString {
        let Some(driver) = self.driver.as_ref() else {
            return GString::new();
        };
        let names: Vec<&str> = driver
            .latest()
            .flags
            .iter_names()
            .map(|(name, _)| name)
            .collect();
        GString::from(&names.join(" | "))
    }

    /// The whole engine state as JSON, or an empty string with no scenario loaded.
    #[func]
    fn snapshot_json(&mut self) -> GString {
        let Some(driver) = self.driver.as_ref() else {
            self.last_error = "no scenario loaded".into();
            return GString::new();
        };
        match driver.snapshot_json() {
            Ok(json) => {
                self.last_error = String::new();
                GString::from(&json)
            }
            Err(error) => {
                self.last_error = error.to_string();
                GString::new()
            }
        }
    }

    /// Replace the pack from a snapshot, returning whether it worked.
    ///
    /// Resets the accumulator and the edge detector: both describe a run that no longer
    /// exists. A consequence worth knowing — conditions already active in the snapshot are
    /// announced again on the next step, because the previous flag mask is adapter state
    /// and is deliberately **not** in the snapshot. Putting it there would bump
    /// `SNAPSHOT_VERSION` for a presentational need.
    #[func]
    fn restore_json(&mut self, snapshot_json: GString) -> bool {
        let json = snapshot_json.to_string();
        let ok = {
            let Some(driver) = self.driver.as_mut() else {
                self.last_error = "no scenario loaded".into();
                return false;
            };
            driver.restore_json(&json)
        };
        match ok {
            Ok(()) => {
                self.last_error = String::new();
                // The restored pack's SOC is unrelated to the old one's, so the epsilon
                // gate must not measure against a reading from a different run.
                self.last_announced_soc = f64::NAN;
                true
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Build the pack from the exported [`Self::scenario_toml`] / [`Self::chemistry_toml`]
    /// properties, so a scene can be configured entirely in the inspector.
    ///
    /// Equivalent to calling [`Self::load_scenario`] with those two values.
    #[func]
    fn load_from_exports(&mut self) -> bool {
        let (scenario, chemistry) = (self.scenario_toml.clone(), self.chemistry_toml.clone());
        self.load_scenario(scenario, chemistry)
    }

    /// Rebuild the pack from its scenario, with or without the BMS.
    ///
    /// **This restarts the run**: simulated time returns to zero, every cell returns to
    /// its initial state, the RNG returns to its seed, and the scenario's faults are
    /// re-queued. There is no honest way to grow a BMS onto a pack that has been running
    /// without one — contrasting the same scenario with and without protection is a
    /// comparison of two runs, and that is the teaching case rather than a limitation.
    ///
    /// When a scenario's faults target *sensors*, turning the BMS off drops them, because
    /// sensors belong to the BMS. Read [`Self::sensor_faults_dropped`] afterwards and say
    /// so — otherwise a student compares two runs that differ in more ways than the label
    /// claims.
    #[func]
    fn restart(&mut self, with_bms: bool) -> bool {
        let ok = {
            let Some(driver) = self.driver.as_mut() else {
                self.last_error = "no scenario loaded".into();
                return false;
            };
            driver.restart(with_bms)
        };
        match ok {
            Ok(()) => {
                self.last_error = String::new();
                self.last_announced_soc = f64::NAN;
                true
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Whether the pack currently has a BMS.
    #[func]
    fn has_bms(&self) -> bool {
        self.driver.as_ref().is_some_and(|d| d.facts().has_bms)
    }

    /// Whether the **scenario** configures a BMS at all — what a UI toggle should be
    /// disabled on, since `restart(true)` cannot conjure one the file never described.
    #[func]
    fn scenario_has_bms(&self) -> bool {
        self.driver
            .as_ref()
            .is_some_and(|d| d.facts().scenario_has_bms)
    }

    /// How many of the scenario's faults were dropped for want of a sensor to aim them
    /// at. Non-zero only after `restart(false)` on a scenario that fault-injects a sensor.
    #[func]
    fn sensor_faults_dropped(&self) -> i64 {
        self.driver
            .as_ref()
            .map_or(0, |d| i64::from(d.facts().sensor_faults_dropped))
    }

    /// Series elements, or 0 with no scenario loaded.
    #[func]
    fn series(&self) -> i64 {
        self.driver
            .as_ref()
            .map_or(0, |d| i64::from(d.facts().series))
    }

    /// Parallel cells per group, or 0 with no scenario loaded.
    #[func]
    fn parallel(&self) -> i64 {
        self.driver
            .as_ref()
            .map_or(0, |d| i64::from(d.facts().parallel))
    }

    /// Set the standing environment. Temperatures in kelvin; pass `NAN` for `t_coolant`
    /// to mean "no coolant", which is not the same as a coolant at 0 K.
    #[func]
    fn set_env(&mut self, t_ambient: f64, t_coolant: f64) -> bool {
        // `NAN` is the sentinel for "no coolant" because GDScript has no `Option` and a
        // second bool argument would be easy to pass wrong. It is never a legal coolant
        // temperature — `Env::check_finite` would reject it — so the sentinel cannot
        // collide with a value someone meant.
        let env = Env {
            t_ambient,
            t_coolant: (!t_coolant.is_nan()).then_some(t_coolant),
        };
        let ok = {
            let Some(driver) = self.driver.as_mut() else {
                self.last_error = "no scenario loaded".into();
                return false;
            };
            driver.set_env(env)
        };
        match ok {
            Ok(()) => {
                self.last_error = String::new();
                true
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Queue a fault to fire at a simulated time, externally tagged the way the scenario
    /// format writes it: `{"SoftInternalShort": {"s": 1, "p": 0, "ohms": 5.0}}`.
    #[func]
    fn schedule_fault_json(&mut self, at_s: f64, fault_json: GString) -> bool {
        let json = fault_json.to_string();
        let ok = {
            let Some(driver) = self.driver.as_mut() else {
                self.last_error = "no scenario loaded".into();
                return false;
            };
            driver.schedule_fault_json(at_s, &json)
        };
        match ok {
            Ok(()) => {
                self.last_error = String::new();
                true
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Drop every fault that has not fired yet, returning how many. Already-active faults
    /// stay active — an internal short that has started is part of the pack now.
    #[func]
    fn clear_faults(&mut self) -> i64 {
        self.driver
            .as_mut()
            .map_or(0, |d| i64::try_from(d.clear_faults()).unwrap_or(i64::MAX))
    }

    /// Clear a latched BMS fault, returning whether there was one to clear.
    #[func]
    fn clear_bms_fault(&mut self) -> bool {
        self.driver
            .as_mut()
            .is_some_and(PackDriver::clear_bms_fault)
    }

    /// Ground truth for every cell as JSON, **series-major**: the cell at series position
    /// `s`, parallel position `p` is at index `s * parallel + p`.
    ///
    /// Ground truth, not what the BMS can sense — the gap between this and
    /// [`Self::soc_bms`] is a feature to look at rather than a bug to hide.
    #[func]
    fn cells_json(&mut self) -> GString {
        let Some(driver) = self.driver.as_ref() else {
            self.last_error = "no scenario loaded".into();
            return GString::new();
        };
        match driver.cells_json() {
            Ok(json) => {
                self.last_error = String::new();
                GString::from(&json)
            }
            Err(error) => {
                self.last_error = error.to_string();
                GString::new()
            }
        }
    }

    /// Actual pack current \[A\], discharge-positive. May differ from the demand if the
    /// BMS derated it, opened the contactor, or a short is drawing current.
    #[func]
    fn i_actual(&self) -> f64 {
        self.driver
            .as_ref()
            .map_or(f64::NAN, |d| d.latest().i_actual)
    }

    /// The BMS's SOC *estimate*, or `NAN` when there is no BMS. Deliberately distinct from
    /// [`Self::soc_true`]: the gap between them is the point.
    #[func]
    fn soc_bms(&self) -> f64 {
        self.driver
            .as_ref()
            .and_then(|d| d.latest().soc_bms)
            .unwrap_or(f64::NAN)
    }

    /// Hottest cell \[K\].
    #[func]
    fn t_max(&self) -> f64 {
        self.driver.as_ref().map_or(f64::NAN, |d| d.latest().t_max)
    }

    /// Coldest cell \[K\].
    #[func]
    fn t_min(&self) -> f64 {
        self.driver.as_ref().map_or(f64::NAN, |d| d.latest().t_min)
    }

    /// Capacity health in (0, 1\].
    #[func]
    fn soh_capacity(&self) -> f64 {
        self.driver
            .as_ref()
            .map_or(f64::NAN, |d| d.latest().soh_capacity)
    }

    /// Wall-clock time the accumulator is carrying, not yet consumed \[s\].
    #[func]
    fn pending_s(&self) -> f64 {
        self.driver.as_ref().map_or(0.0, PackDriver::pending_s)
    }
}

impl BatteryPack {
    /// Step the driver, then emit — with the borrow scoped so the emission cannot
    /// re-enter while it is live. See the type's borrow-discipline note.
    fn advance(&mut self, dt: f64, n_steps: u32, demand: Demand) -> bool {
        let outcome = {
            let Some(driver) = self.driver.as_mut() else {
                self.last_error = "no scenario loaded".into();
                return false;
            };
            driver.step_batch(dt, n_steps, demand)
        };
        // The borrow has ended. Only now is it safe to call into GDScript.
        match outcome {
            Ok(advance) => {
                self.last_error = String::new();
                self.announce(advance);
                true
            }
            Err(error) => self.fail(&error),
        }
    }
}
