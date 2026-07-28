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

use driver::{DriverError, PackDriver};
use sim_core::Demand;

struct BatsimExtension;

#[gdextension]
unsafe impl ExtensionLibrary for BatsimExtension {}

/// A battery pack, simulated by `sim-core`.
///
/// Slice B ships the type, its registration, and enough surface to prove the whole path
/// from GDScript to `Pack::step` and back. The exported properties, the accumulator in
/// `_physics_process`, and the signals are slice C.
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
}

#[godot_api]
impl INode for BatteryPack {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            driver: None,
            last_error: String::new(),
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
}

#[godot_api]
impl BatteryPack {
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
    #[func]
    fn restore_json(&mut self, snapshot_json: GString) -> bool {
        let json = snapshot_json.to_string();
        let Some(driver) = self.driver.as_mut() else {
            self.last_error = "no scenario loaded".into();
            return false;
        };
        match driver.restore_json(&json) {
            Ok(()) => {
                self.last_error = String::new();
                true
            }
            Err(error) => self.fail(&error),
        }
    }
}

impl BatteryPack {
    /// Step the driver, keeping the borrow scoped so a future signal emission cannot
    /// re-enter while it is live. See the type's borrow-discipline note.
    fn advance(&mut self, dt: f64, n_steps: u32, demand: Demand) -> bool {
        let Some(driver) = self.driver.as_mut() else {
            self.last_error = "no scenario loaded".into();
            return false;
        };
        match driver.step_batch(dt, n_steps, demand) {
            // Slice C turns `advance.edges` into signals here, after this borrow ends.
            Ok(_advance) => {
                self.last_error = String::new();
                true
            }
            Err(error) => self.fail(&error),
        }
    }
}
