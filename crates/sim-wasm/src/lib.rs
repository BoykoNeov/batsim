//! `sim-wasm` — the engine, in a browser.
//!
//! A sibling of `sim-server`, not a layer under it. The demo page under `web/` runs the
//! simulation **in the tab** through this module: no socket, no server round-trip per
//! frame, no network between a control being moved and a curve responding. The server
//! exists for scripts and for putting one live pack on several screens; the page can
//! talk to it too, and does, behind a toggle — but it does not need to in order to
//! simulate.
//!
//! # The shape of this crate
//! Everything real is in [`engine`], in plain Rust with typed errors and no
//! `wasm-bindgen` type in any signature. [`Sim`] below is a forwarding layer: it turns
//! `&str` into `&str` and [`engine::EngineError`] into [`JsError`], and it makes no
//! decisions. That split is what lets `cargo test --workspace` mean something here —
//! this crate is compiled for the **host** target by the workspace gates, and on the
//! host a `JsError` is a stub for a browser type that does not exist.
//!
//! # What crosses the boundary
//! Strings and numbers, nothing else. Telemetry, snapshots, cells, sensors and faults
//! are JSON text; `dt`, step counts and temperatures are raw numbers. There is deliberately no
//! `Float64Array` fast path: the plan for this slice says not to pre-optimize the
//! boundary, and a page plotting a few hundred pixels of curve has no measurement
//! saying the JSON crossing costs anything.
//!
//! Because numbers cross raw, **`NaN` is reachable here in a way it is not over the
//! server's socket** — JSON has no literal for it, `Number.NaN` needs no literal. See
//! [`engine`] for where that is checked.
//!
//! # Encodings a JS caller needs to know
//! * Engine enums are **externally tagged**: `{"Current": -5.0}`, `"Rest"`,
//!   `{"SoftInternalShort": {"s": 1, "p": 0, "ohms": 5.0}}`. Same as `sim-server`, on
//!   purpose — one engine should not have two dialects.
//! * `EventFlags` is a `" | "`-joined **string** of flag names, and `""` means no flags
//!   — not a bitmask integer, and not a one-element array containing `""`.
//!
//! # Building
//! ```text
//! wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg
//! ```
//!
//! That command is **not** part of `cargo test --workspace`: it needs a toolchain the
//! Rust test run has no business invoking, and its output is a build artifact that is
//! not committed. The crate itself is a normal workspace member and stays inside the
//! gates, which is what keeps it compiling.

#![forbid(unsafe_code)]

pub mod engine;

pub use engine::{
    frame_count, Cells, EngineError, Frame, PackFacts, Sensors, SimEngine, MAX_FRAMES_PER_CALL,
    MAX_STEPS_PER_CALL,
};

use wasm_bindgen::prelude::*;

/// Version of the JS-facing contract: method names, and the JSON field names of the
/// engine types that cross this boundary.
///
/// Independent of both `sim_core::SNAPSHOT_VERSION` (which versions the engine's saved
/// pack layout) and `sim_server::API_VERSION` (which versions the HTTP/WebSocket
/// contract). Three numbers with three jobs; a page that loads a `pkg/` built from a
/// different revision than the server it also talks to can notice.
///
/// v2 (Phase 6, slice C1): `CellView`'s `v_rc_sum` became `overpotential_v`. That type
/// crosses this boundary verbatim inside [`Cells`], so the rename lands in the JSON a
/// page parses and this constant is owed for the same reason `sim_server::API_VERSION`
/// is — which is the whole point of there being three of these rather than one shared
/// number. `sim_core::SNAPSHOT_VERSION` stays at 9: no saved pack changed shape.
///
/// v3 (UI slice, truth beside belief): [`Sim::sensors`] is new, returning [`Sensors`] —
/// what the BMS measured, or `null` on a pack with no BMS. An added method rather than a
/// changed shape, so a v2 page keeps working against a v3 module. The number moves anyway
/// because **this module is a build artifact loaded separately from the JS that calls
/// it**: `web/pkg` is gitignored and has to be rebuilt after any Rust change, so the page
/// can be newer than the wasm it loads in a way no other version pair in this workspace
/// can. `web/app.js` compares this against a minimum and asks for a rebuild by name; that
/// check is what makes the bump load-bearing rather than decoration.
///
/// Note `sim_server::API_VERSION` deliberately did **not** move with it, the first time
/// the two have parted. Its doc states a bump *rule* with an explicit additive exemption
/// ("adding a field or an error code does not bump it") and a new route is an addition;
/// this doc states the contract's *scope* — method names — which an added method changes.
/// `docs/plans/ui-bms-view.md` records why the plan that priced them as a pair was wrong.
/// `sim_core::SNAPSHOT_VERSION` stays at 10: every accessor this needed was already
/// public, and no saved pack changed shape.
/// v4 (the energy hole): [`sim_core::Telemetry`] gains `i_rejected_a`, the charge a SOC
/// clamp refused or invented. That is an *added field*, not a rename, and it crosses
/// this boundary verbatim inside every telemetry payload the page reads.
///
/// An addition, so by `sim_server::API_VERSION`'s rule — which exempts added fields —
/// that constant stays at 2, and the two part company for the second time. This one
/// moves anyway, for the reason its v3 paragraph gives and which has nothing to do with
/// renames: `web/pkg` is a build artifact loaded separately from the JS that calls it,
/// `web/app.js` now reads `i_rejected_a` in two render paths, and against a v3 bundle
/// that field is `undefined` — a `TypeError` from inside a draw call, which names
/// neither the cause nor the fix. The page's `WASM_API_MIN` moves with it, which is what
/// makes the bump load-bearing rather than decoration.
///
/// `sim_core::SNAPSHOT_VERSION` stays at 11: the rejected amount is computed and
/// consumed inside a step, so no saved pack changed shape.
///
/// v5 (surface vs bulk): `sim_core::CellView` gains `surface_gap_neg` and
/// `surface_gap_pos`, the concentration gradient a porous-electrode cell has and an
/// equivalent circuit does not. `CellView` crosses this boundary verbatim inside
/// [`Cells`], so both land in the JSON a page parses.
///
/// Additions again, so `sim_server::API_VERSION` stays at 2 by its own explicit
/// exemption and the two part company for the **third** time — after v3's new method and
/// v4's added telemetry field. The plan that priced this slice had them moving together;
/// each constant's own doc said otherwise, and the habit of reading them before paying a
/// planned bump is the whole reason there are three numbers.
///
/// This one moves for the reason v3 and v4 give and which has nothing to do with
/// renames: `web/pkg` is a build artifact loaded separately from the JS that calls it.
/// Against a v4 bundle these two fields are `undefined`, and the page's new grid metric
/// would divide a range on them from inside a draw call. `web/app.js`'s `WASM_API_MIN`
/// moves with it, which is what makes the bump load-bearing rather than decoration.
///
/// `sim_core::SNAPSHOT_VERSION` stays at 13: both fields are pure functions of stored
/// state — a post-step concentration profile and the current that produced it — so no
/// saved pack changed shape. Note this is `CellView`'s *second* addition of a field that
/// is `None` on some models, and the second time the alternative `0.0` was rejected for
/// the same reason: an absent quantity that plots as a real measurement is worse than no
/// quantity at all.
pub const WASM_API_VERSION: u32 = 5;

/// [`WASM_API_VERSION`], reachable from JS.
///
/// A function rather than the constant itself: `#[wasm_bindgen]` refuses to export a
/// `const` outright (it is only meaningful there for a `typescript_custom_section`), so
/// the constant stays Rust-side and this is its accessor.
#[wasm_bindgen]
#[must_use]
pub fn wasm_api_version() -> u32 {
    WASM_API_VERSION
}

/// [`MAX_STEPS_PER_CALL`], reachable from JS.
///
/// Exported so a page can size its batches from the engine's own numbers rather than
/// restating them. `sim-server` reports the equivalent caps in its hello frame for the
/// same reason; between the two there is no reason for a client to hardcode either.
#[wasm_bindgen]
#[must_use]
pub fn max_steps_per_call() -> u32 {
    MAX_STEPS_PER_CALL
}

/// [`MAX_FRAMES_PER_CALL`], reachable from JS. See [`max_steps_per_call`].
#[wasm_bindgen]
#[must_use]
pub fn max_frames_per_call() -> u32 {
    MAX_FRAMES_PER_CALL
}

/// The engine's snapshot layout version this module produces and consumes.
///
/// Re-exported as a function because a page that offers snapshot download needs to
/// label the file, and reading it from the module beats hard-coding it in JS.
#[wasm_bindgen]
#[must_use]
pub fn snapshot_version() -> u32 {
    sim_core::SNAPSHOT_VERSION
}

/// Which chemistry a scenario needs, so a page can fetch it before constructing a
/// [`Sim`].
///
/// Returns `undefined` when the scenario inlines its chemistry and needs nothing
/// fetched. The returned id is already known to match `[a-z0-9_]+`, so it is safe to
/// interpolate into a URL.
///
/// # Errors
/// The scenario text failing to parse or validate — which a page learns here rather
/// than two fetches later.
#[wasm_bindgen]
pub fn chemistry_id_of(scenario_toml: &str) -> Result<Option<String>, JsError> {
    Ok(SimEngine::chemistry_id_of(scenario_toml)?)
}

/// A pack, its scenario, and its standing environment.
///
/// Every method here forwards to [`SimEngine`]; the documentation of what anything
/// *means* lives there. The `?` on each forward goes through `wasm-bindgen`'s own
/// blanket `impl<E: Error> From<E> for JsError`, which is `JsError::new(&e.to_string())`
/// — so a browser console shows [`EngineError`]'s own words, and writing that impl by
/// hand here is a coherence error rather than an improvement.
#[wasm_bindgen]
pub struct Sim {
    inner: SimEngine,
}

#[wasm_bindgen]
impl Sim {
    /// Build a pack from scenario TOML, with chemistry TOML alongside it when the
    /// scenario names an id rather than inlining one.
    ///
    /// # Errors
    /// Bad scenario text, bad chemistry text, or a scenario that names a chemistry with
    /// nothing supplied for it.
    #[wasm_bindgen(constructor)]
    pub fn new(scenario_toml: &str, chemistry_toml: Option<String>) -> Result<Sim, JsError> {
        Ok(Sim {
            inner: SimEngine::new(scenario_toml, chemistry_toml.as_deref())?,
        })
    }

    /// Live facts about the pack, as JSON.
    ///
    /// # Errors
    /// Serialization failure.
    pub fn facts(&self) -> Result<String, JsError> {
        Ok(self.inner.facts_json()?)
    }

    /// Simulation time elapsed \[s\].
    #[must_use]
    pub fn sim_time_s(&self) -> f64 {
        self.inner.facts().sim_time_s
    }

    /// The standing environment, as JSON.
    ///
    /// # Errors
    /// Serialization failure.
    pub fn env(&self) -> Result<String, JsError> {
        Ok(self.inner.env_json()?)
    }

    /// Replace the standing environment. `t_coolant` may be `undefined` for passive
    /// cooling to ambient only.
    ///
    /// # Errors
    /// Either temperature not being finite.
    pub fn set_env(&mut self, t_ambient: f64, t_coolant: Option<f64>) -> Result<(), JsError> {
        Ok(self.inner.set_env(sim_core::Env {
            t_ambient,
            t_coolant,
        })?)
    }

    /// Advance the pack and return the reported frames as a JSON array.
    ///
    /// `demand_json` is externally tagged: `{"Current": -5.0}`, `"Rest"`.
    ///
    /// # Errors
    /// An unparseable demand, a non-finite or negative `dt`, a zero count, or either
    /// per-call cap.
    pub fn step_many(
        &mut self,
        dt: f64,
        n_steps: u32,
        demand_json: &str,
        report_every_n_steps: u32,
    ) -> Result<String, JsError> {
        Ok(self
            .inner
            .step_many_json(dt, n_steps, demand_json, report_every_n_steps)?)
    }

    /// Ground truth for every cell, as JSON.
    ///
    /// # Errors
    /// Serialization failure.
    pub fn cells(&self) -> Result<String, JsError> {
        Ok(self.inner.cells_json()?)
    }

    /// What the BMS measured, as JSON — or the literal `null` on a pack with no BMS.
    ///
    /// The counterpart to [`Sim::cells`]: that is ground truth, this is belief.
    ///
    /// # Errors
    /// Serialization failure.
    pub fn sensors(&self) -> Result<String, JsError> {
        Ok(self.inner.sensors_json()?)
    }

    /// The whole engine state, as JSON.
    ///
    /// # Errors
    /// Serialization failure.
    pub fn snapshot(&self) -> Result<String, JsError> {
        Ok(self.inner.snapshot_json()?)
    }

    /// Replace the pack with a snapshot, in place. Refuses a different topology.
    ///
    /// # Errors
    /// Text that is not a snapshot, a snapshot this build cannot read, or one
    /// describing a differently-shaped pack.
    pub fn restore(&mut self, snapshot_json: &str) -> Result<(), JsError> {
        Ok(self.inner.restore_json(snapshot_json)?)
    }

    /// Rebuild the pack from the scenario, with or without its BMS.
    ///
    /// **Restarts the run.** See [`SimEngine::restart`] for why growing a BMS onto a
    /// running pack is not on offer.
    ///
    /// # Errors
    /// A rebuild failure, which given inputs that built once means a bug here rather
    /// than in the input.
    pub fn restart(&mut self, bms_enabled: bool) -> Result<(), JsError> {
        Ok(self.inner.restart(bms_enabled)?)
    }

    /// Whether the last [`Sim::restart`] asked for the scenario's BMS.
    #[must_use]
    pub fn bms_enabled(&self) -> bool {
        self.inner.bms_enabled()
    }

    /// Queue a fault to fire at a simulation time. `fault_json` is externally tagged.
    ///
    /// # Errors
    /// Text that is not a fault, or one the engine refuses.
    pub fn schedule_fault(&mut self, at_s: f64, fault_json: &str) -> Result<(), JsError> {
        Ok(self.inner.schedule_fault_json(at_s, fault_json)?)
    }

    /// Drop every fault that has not fired yet, returning how many.
    pub fn clear_faults(&mut self) -> usize {
        self.inner.clear_faults()
    }

    /// Clear a latched BMS fault, returning whether there was one to clear.
    pub fn clear_bms_fault(&mut self) -> bool {
        self.inner.clear_bms_fault()
    }
}
