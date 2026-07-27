# Phase 4 — headless server + browser demo

**Status: planned.** No slice has landed. This file is written before the work so the
decisions below are made once; the "learned while building" material is appended as each
slice lands, the way `phase-2-thermal-bms.md` and `phase-3-aging-faults.md` grew.

Phase 4 is the first phase that adds **no physics**. Everything below is transport,
serialization, and presentation over an engine that is already finished for this phase's
purposes. That framing is load-bearing: see "The `SNAPSHOT_VERSION` canary".

| exit criterion (from `CLAUDE.md`) | to be met by |
| --------------------------------- | ------------ |
| An external script can run a full experiment over WebSocket | `sim-server/tests/e2e_experiment.rs` — the *same* scenario driven in-process and over a real WebSocket on an ephemeral port, asserting the two telemetry streams are **bit-identical**, plus a snapshot/restore leg mid-experiment |

## Slices

| slice | scope | state |
| ----- | ----- | ----- |
| A | wire contract, no new crates: serde on `Telemetry`/`Demand`/`Env`/`CellView`, `Scenario` + `parse_scenario` in `sim-data`, boundary validation helpers | planned |
| B | `sim-server` skeleton: axum, session store, REST (create from scenario, inspect, snapshot GET/POST, delete), chemistry resolution | planned |
| C | WebSocket: command/event protocol, explicit-`dt` batch stepping, report decimation, the one-writer rule. **Carries the exit criterion.** | planned |
| D | `sim-wasm` + the browser page: `wasm-bindgen` wrapper, chemistry TOML text handed in from JS, hand-rolled canvas plotting, zero external JS deps | planned |
| E | wrap-up: README status and run instructions, the example external script, a transport latency measurement, `sim-core` perf re-measure | planned |

Each slice keeps `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` clean. Unlike phases 2 and 3,
**no slice should bump `SNAPSHOT_VERSION`** — that is a deliberate tripwire, not an
observation (below).

---

## Decisions already made (do not re-derive)

### The `SNAPSHOT_VERSION` canary

Phases 2 and 3 bumped once per layout-changing slice and the rule was "check the layout
at the end of every slice." Phase 4 inverts it: **if a slice needs a bump, an adapter has
leaked into the engine.** Adding `Serialize` to `Telemetry` does not touch `Pack`'s
serialized layout; nothing a server or a browser needs should change what a pack *is*.

If a slice reaches for a bump, stop and re-read the slice — the honest fixes are almost
always "put it in the adapter" or "add a read-only accessor", not "add a field to
`Pack`".

### Deterministic batch stepping is the primary mode; real time is a thin layer on top

`CLAUDE.md`'s never-do list says the client's frame rate or message rate must never define
the physics `dt`, and the exit criterion is a *script* running an experiment. Those two
combine into one protocol shape:

> The client sends `(dt, n_steps, demand, env)`. The server steps exactly that and
> replies with telemetry.

`dt` is always **explicit in the command**, never `now − last_message`. A run of 10 000
steps is one message, not 10 000 messages, so network jitter cannot enter the trajectory
and a fast-forward is not rate-limited by the socket.

The browser demo's live mode is a *layer on top*: the page owns an accumulator, and at
each animation frame sends "advance `k` steps of the session's fixed `dt`" where `k` comes
from wall-clock elapsed × speed multiplier. The `dt` is the session's, configured once;
only `k` varies. That is the accumulator pattern `CLAUDE.md` prescribes for clients,
implemented on the client side of the socket, which is where it belongs — the server never
reads a clock to decide physics.

### The exit gate is a bit-identical comparison, not a smoke test

"An external script can run a full experiment" is satisfiable by a test that connects,
sends a few demands, and eyeballs plausible numbers. That test would pass while the
transport quietly perturbed the physics — a lossy float encode, a dropped step, a command
applied in the wrong order.

The gate instead runs the identical scenario twice: once by calling `Pack::step` in
process, once by driving the server over a real WebSocket, and asserts the telemetry
streams match **bit for bit** (compare `f64::to_bits`, not `==`, so `-0.0`/`NaN` cannot
launder a difference). It discharges the criterion and proves the transport is
physics-transparent in the same assertion. Include a mid-experiment
snapshot → REST GET → REST POST → resume leg in the WebSocket run, so restore-over-the-wire
is inside the same bit-identical claim.

### Snapshots over JSON need `serde_json`'s `float_roundtrip` feature — measured, not assumed

This is the finding that most deserves to be written down before anyone starts, because
it fails *silently and rarely*.

`serde_json`'s default float parser is not correctly rounded. Serializing is fine (ryu is
shortest-round-trip), but the deserializer's fast path can return a value **one ULP** off
the one that was written. Probed on this repo's own engine (4S2P LFP, scatter on, 20 steps,
snapshot → `to_string` → `from_str`):

```text
A: EcmState { soc: 0.7995885912375074, v_rc: [0.0013490351429541795], ... }
B: EcmState { soc: 0.7995885912375074, v_rc: [0.0013490351429541797], ... }
                                                                 ^^ one ULP
snapshot value round-trip == : false
continued trajectory bit-identical: false
```

With `serde_json = { version = "1", features = ["float_roundtrip"] }` the same probe gives
`true` on all three, including a 600-step run split by a JSON snapshot at step 300 on an
aging + plating configuration (i.e. one that draws from the pack RNG mid-run). Note also
that the re-serialized JSON *text* was unstable without the feature and stable with it.

Consequences to bake in:

- Every crate that serializes a `Snapshot` as JSON — `sim-server`, and `sim-wasm` if it
  exposes snapshot strings — declares the feature. Put it in `[workspace.dependencies]`
  so it cannot be forgotten by a later crate.
- Slice B ships the regression test: snapshot → JSON → restore → continue is bit-identical
  against an uninterrupted run. Without it, a future dependency change that drops the
  feature is invisible until someone's restored session drifts.
- This is `sim-core`'s dev-dep comment ("the engine never depends on a concrete
  serialization format — adapters choose") coming due. The adapter is choosing, and the
  choice has a footnote. `bincode` remains the format for the in-repo replay test; JSON is
  the *wire* format, and it is exact only with the feature on.

### Validation lives at the adapter boundary; `sim-core` stays permissive

A client can send `NaN`, `Infinity`, a negative `dt`, or a demand of `1e300`. `step()`
promises never to panic; it promises nothing about NaN propagating through every cell and
poisoning the session forever. Changing that contract to make the engine defensive would
cost a branch per field on the hot path for a hazard that only exists at a socket.

So the boundary rejects, and the engine keeps its contract:

- `dt` must be finite and `>= 0`. Zero is *allowed* — the engine has a pinned
  zero-length-step probe contract (`snapshot.rs::zero_length_step_does_not_mutate_state`)
  and exposing it lets a client read telemetry without advancing, which the browser page
  wants on connect. `n_steps` bounds the loop, so `dt = 0` cannot hang anything.
- Every `f64` in a `Demand`, an `Env`, or a scenario must be finite. Reject the message
  with a structured error; do not clamp silently.
- `n_steps` has a configured per-message cap (default 1 000 000). A single message must not
  be able to occupy the session task for minutes without the client being able to
  interleave anything.

Note the asymmetry the engine already has, because it tells you which half needs work:
`Pack::schedule_fault` **does** validate (`FaultError::NotFinite`, index bounds), so the
`ScheduleFault` command can lean on the engine and just translate the error. `step` does
not validate and should not start, so `Step` is the command that needs a real boundary
check. `CLAUDE.md` never states this because before Phase 4 nothing untrusted could reach
`step`. Phase 4 is where it becomes necessary; record it here rather than inventing it
twice.

### One writer per session

Two sockets sending step commands to one `Pack` makes the trajectory depend on network
arrival order. Determinism does not fail loudly — it fails the next time someone opens a
second browser tab and cannot reproduce yesterday's curve.

The rule: a session has at most one **writer**; later attachers are accepted as
**read-only observers** (they receive the telemetry stream, their commands are rejected
with a typed error). Read-only attach is worth having, not just a consolation prize — it is
how a teaching demo puts the same live pack on two screens.

### Session IDs come from an adapter-side source; the pack RNG is untouched

Obvious once stated, invisible if not: drawing a session id from the pack's `ChaCha8Rng`
would consume draws and change the trajectory. The pack RNG is *physics state*. Session
ids come from the server's own counter or its own RNG, and the plan says so because a
"we already have an RNG" shortcut is exactly the kind of thing that looks tidy in a diff.

### `ChemistryRegistry` never materialized — do not resurrect it from the API sketch

`CLAUDE.md`'s sketch has `Pack::new(&config, &ChemistryRegistry)`. The engine shipped
`Pack::new(&PackConfig, ChemistryParams)` — params by value, no registry. Phase 4 must not
reintroduce a registry into `sim-core` to serve the server's need to resolve a name.

Resolution is an adapter concern, and each adapter has a different one:

- **`sim-server`**: a scenario names a chemistry `id`; the server maps it to
  `<chem_dir>/<id>.toml` via the existing `sim_data::load_chemistry_file`. `chem_dir` is a
  server config/CLI argument. **Ids are validated against `^[a-z0-9_]+$`** before touching
  the filesystem — no separators, no dots, so a scenario cannot walk out of the directory.
  A scenario may alternatively inline the chemistry TOML text, which is what makes the
  server usable without shipping the `chemistries/` tree.
- **`sim-wasm`**: there is no filesystem. JS fetches the TOML and passes the *text* to
  `sim_data::parse_chemistry`. Verified: `cargo check -p sim-data --target
  wasm32-unknown-unknown` succeeds today — `std::fs` compiles on wasm and merely fails at
  runtime, so `sim-wasm` can depend on `sim-data` and simply not call
  `load_chemistry_file`. No feature gate needed. (If that ever changes, the fix is
  `#[cfg(not(target_arch = "wasm32"))]` on the one fs function, not a new crate.)

### The scenario file format: pack + chemistry reference + faults, and *not* a demand program

Nothing in the repo has a scenario *file* yet — every scenario today is a `PackConfig`
built in Rust. Phase 4 needs one, and the tempting scope creep is to make it a little
scripting language ("at t=60 s, charge at 2 A; at t=600 s, rest"), because the phase-3
fault queue already looks like one.

Don't. A `Scenario` is the **initial condition and the pack's own pre-programmed
misfortunes**:

```toml
[meta]
name = "overcharge with the BMS off"

chemistry = "lfp_26650_generic"   # id, or inline = """<toml text>"""

[pack]                             # exactly PackConfig, serde as it already is
series = 4
parallel = 2
initial_soc = 0.5
initial_temp_k = 298.15
seed = 42

[[faults]]                         # exactly ScheduledFault, serde as it already is
at_s = 600.0
fault = { SoftInternalShort = { s = 1, p = 0, ohms = 5.0 } }
```

The demand program stays on the client. That is the difference between a server and a
scripting engine, and it keeps the exit criterion honest: "an external script can run a
full experiment" means the *script* runs the experiment, not that it uploads one and
watches. It also means `Scenario` composes from types that already exist and are already
serde — the format cannot drift from the engine, because it *is* the engine's types.

`Scenario` and `parse_scenario(&str) -> Result<Scenario, DataError>` live in `sim-data`
next to `parse_chemistry`, text-based for the same reason (wasm has no files), with
`load_scenario_file` as the thin fs wrapper.

### The engine's types get `Serialize`/`Deserialize`; there is no DTO layer

`Telemetry`, `Demand`, `Env`, and `CellView` gain serde derives in `sim-core`. It adds no
dependency (serde is already there), no state, and no layout — and the alternative, a
mirrored DTO in `sim-server` plus another in `sim-wasm`, is forty fields of duplication
that will silently drift from the engine's doc comments, which are the actual
documentation of what those numbers mean.

The cost is real and gets named instead of dodged: **the JSON field names become a wire
contract**. Renaming `Telemetry::q_gen_w` becomes a client-visible break. It is *not*
covered by `SNAPSHOT_VERSION` (which versions pack state, not telemetry), so `sim-server`
carries its own `API_VERSION` constant, reported in the REST root and in the WebSocket
hello frame. Two version numbers with two different jobs; say which is which in both
places.

Enum shapes stay **serde-default (externally tagged)**, matching how `ThermalConfig` and
`Fault` already serialize — `{"Current": -5.0}`, `"Rest"`,
`{"Network": {"k_neighbor_w_per_k": 0.5}}`. Consistency with the existing scenario/snapshot
encoding beats a JS-friendlier adjacent tagging that would apply to two of the engine's
five enums and make the rest look arbitrary. The demo page gets a three-line helper.

### `EventFlags` is a string on the wire — measured

`bitflags` v2's serde impl is format-sensitive, and the browser has to parse whatever it
produces. Probed:

```text
OV | UV | THERMAL_RUNAWAY  ->  "OV | UV | THERMAL_RUNAWAY"
EventFlags::empty()        ->  ""
round-trip                 ->  true
```

So JSON gets a `" | "`-joined name string and an empty string for no flags — not a
bitmask integer. That is genuinely nicer for a pedagogy client (a student reading the raw
frame sees `"OV | PLATING_RISK"`), and it means the JS side splits on `" | "` and must
treat `""` as the empty set rather than as a flag named `""`. Note it in slice D's code,
because it is a two-minute bug and a one-line comment.

### Telemetry decimation is a protocol requirement, not an optimization

Phase 3's own exit scenario fast-forwards 500 cycles. At any useful `dt` that is millions
of steps; a frame each would be gigabytes of JSON for a plot with a few hundred visible
pixels of resolution.

Every stepping command therefore carries `report_every_n_steps` (default 1). The server
reports step `n` where `n % k == 0`, **always including the final step of the batch** so
the client's last sample is the true end state rather than whatever the modulus happened
to land on. Decimation drops *reports*, never steps: the trajectory is identical, only the
sampling of it changes — which is why it can be a default-on protocol feature without
touching the bit-identity gate (the gate runs with `k = 1`).

### Backpressure drops views, never commands

If a client cannot keep up with a live stream, the server drops telemetry frames and
**counts them**, reporting `dropped_since_last` in the next frame so a plot can show the
gap honestly instead of drawing a smooth line through missing data. Commands are never
dropped and never reordered.

Batch replies are exempt: a batch's reports are the experiment's record, so a batch either
delivers all of its (decimated) frames or the session errors. "Best effort" is right for a
live view and wrong for a result.

### `sim-wasm` stays in the workspace; `wasm-pack` stays out of `cargo test`

`wasm-bindgen`-dependent crates build for the host target fine, so `sim-wasm` is a normal
workspace member and stays inside the committed per-slice gates (`cargo test --workspace`,
`clippy --all-targets`). If a host build ever breaks, the fix is
`[target.'cfg(target_arch = "wasm32")'.dependencies]`, and only if that fails, `[workspace]
exclude` — decide when slice D lands, do not block slice A on it.

What is *not* in the gates: the actual `wasm-pack build`. It needs a toolchain the Rust
test run has no business invoking, and its output is a build artifact. Slice D documents
the command and slice E puts it in the README. (Both `wasm32-unknown-unknown` and
`wasm-pack` are present on this machine — checked — so this is a policy choice, not a
tooling limitation.)

The generated `pkg/` output is **not committed**; the demo page loads it from a path the
server serves, and the README says how to produce it.

### The demo page is served by `sim-server`, and has zero external dependencies

A wasm module cannot be loaded from `file://` (CORS), so the page needs *a* server; making
it `sim-server`'s static route means `cargo run -p sim-server` plus a browser is the whole
setup, with no second process and no `python -m http.server` in the instructions.

No CDN, no charting library, no build step for the JS. Plotting is hand-rolled on a
`<canvas>`: this page's job is voltage/current/SOC/temperature against time, which is a
polyline and two axes. A dependency-free page is also the honest pedagogy artifact — a
student can read the whole client.

The page and the server are two clients of the same engine, not layers: the page runs the
engine *in the browser* via `sim-wasm` (no socket, no server round-trip per frame), and
the server exists for scripts and for future multi-client scenarios. Slice E's README must
say this plainly or the two will read as redundant.

### Compile-time cost, stated once

`axum` + `tokio` make `cargo test --workspace` meaningfully slower. They do **not** touch
`cargo test -p sim-core`, which is the fast inner loop and stays the loop of record for
physics work. The phase-2 plan cared enough to strip criterion's default features for the
same reason; this is the same accounting, and it is a note rather than a slice because
there is nothing to do about it beyond knowing which command to run.

---

## Slice detail

### A — wire contract

- serde derives on `Telemetry`, `Demand`, `Env`, `CellView` in `sim-core`. No new
  dependency, no layout change, **no version bump**.
- `sim-data`: `Scenario`, `parse_scenario`, `load_scenario_file`, validation
  (chemistry id charset, finiteness of every `f64`, faults sorted/duplicate-free enough to
  build).
- Tests: JSON round-trip of each engine type; `parse_scenario` on a committed example;
  rejection cases (bad id, non-finite, unknown field).
- Ship `scenarios/` with two examples — one plain CC discharge, one that reproduces a
  phase-3 fault scenario — so slice B has something real to load and the format has a
  user before it has a server.

### B — `sim-server` skeleton

- New crate; `axum`, `tokio`, `serde_json` (**with `float_roundtrip`**), `tower-http` for
  static files, `tracing` for logs, `anyhow` at the binary edge (`sim-core`'s no-panic
  rule is an engine rule; binaries may use `anyhow`, per `CLAUDE.md`).
- Session store: `BTreeMap<SessionId, Session>` behind a `tokio::sync::Mutex`, or a task
  per session with a command channel — decide at implementation time; the *observable*
  contract (one writer, commands ordered) is fixed here and does not depend on which.
  Not a `HashMap`: the determinism rule bans them in simulation state, and while a session
  map is not simulation state, iteration order shows up in list endpoints, and "no
  `HashMap` anywhere near the engine" is cheaper to keep than to argue about.
- REST: `POST /sessions` (scenario in body, returns id + api version), `GET /sessions`,
  `GET /sessions/{id}` (config, sim time, latest telemetry), `GET /sessions/{id}/cells`
  (ground-truth `CellView` array — the pedagogy view the browser needs),
  `GET|POST /sessions/{id}/snapshot`, `DELETE /sessions/{id}`.
- The JSON-snapshot bit-identity regression test lands here, not in slice C.

### C — WebSocket (carries the exit criterion)

- `GET /sessions/{id}/ws` → hello frame (api version, snapshot version, session config,
  writer/observer role).
- Commands: `Step { dt, n_steps, demand, env, report_every_n_steps }`, `SetEnv`,
  `ScheduleFault`, `ClearFaults`, `ClearBmsFault`, `Snapshot`, `Restore`, `Ping`.
  `Pack`'s existing `schedule_fault`/`clear_faults`/`clear_bms_fault` are already the right
  surface; the protocol is a thin skin over them.
- Events: `Telemetry { sim_time_s, .. }`, `BatchComplete { steps, sim_time_s }`,
  `Error { code, message }`, `Dropped { count }`.
- Every telemetry frame carries `sim_time_s`. The client must never have to integrate
  `dt` itself to know where it is — that is how off-by-one-step plots happen.
- Tests: the bit-identical exit gate; one-writer enforcement; validation rejections
  (non-finite, negative `dt`, oversized `n_steps`); decimation correctness (the reported
  subset equals the corresponding steps of an undecimated run, final step always present).

### D — `sim-wasm` + browser page

- `wasm-bindgen` wrapper: construct from scenario text + chemistry text, `step_many`,
  telemetry out (JSON string first; a `Float64Array` fast path only if measurement says
  the JSON crossing matters — do not pre-optimize the boundary).
- Snapshot/restore as JSON strings, same `float_roundtrip` requirement.
- `web/index.html` + one hand-written JS file + canvas plotting; controls for demand,
  ambient temperature, speed multiplier, BMS on/off; a flags readout that splits `" | "`.
- Served from `sim-server`'s static route.

### E — wrap-up

- **README status is wrong today** — it still says "Phase 0 scaffold" after three phases
  landed. Slice E fixes it and adds: how to run the server, how to build the wasm bundle,
  how to open the page, and where the example script lives.
- `examples/experiment.py` (or `.mjs`) — the external script the exit criterion talks
  about, in a form a person can actually copy.
- Transport latency measurement: round-trip for a batch of *n* steps versus in-process, so
  the README can say what the socket costs. Expect the transport to dominate at small `n`
  (a `Pack::step` is tens of µs at 100S10P) and to vanish at large `n` — which is the
  numeric argument for batch stepping, worth having as a measured number rather than an
  assertion.
- `sim-core` perf re-measure. Phase 4 should move it by **zero** — nothing on the step path
  changes — so this is a control, not an expectation. Per
  `docs/plans/pack-step-perf.md` and the memory note: report ratios against a
  same-session baseline, not absolutes; this box has missed its fast state repeatedly.

---

## Open questions (decide when the slice lands, not now)

- **Session task vs shared map.** Both satisfy the fixed contract above. A task per session
  makes the one-writer rule structural and backpressure natural; a map is less machinery.
  Pick at slice B with the code in front of you.
- **Snapshot body size.** A 4S2P snapshot is ~5 KB of JSON. 100S10P will be ~600 KB, which
  is fine for REST and poor for a WebSocket frame. If it bites, the answer is
  `Content-Encoding` on the REST route, not a new binary format on the socket.
- **Whether the browser page needs the server at all after slice D.** It does not, for
  physics — it embeds the engine. Keep the socket path in the page anyway, behind a toggle,
  so the server protocol has a live client and does not rot.
