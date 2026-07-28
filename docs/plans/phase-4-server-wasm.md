# Phase 4 — headless server + browser demo

**Status: slices A, B, C and D landed — the phase's exit criterion is met; slice E (wrap-up) is
what remains.** This file is written before the work so the
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
| A | wire contract, no new crates: serde on `Telemetry`/`Demand`/`Env`/`CellView`, `Scenario` + `parse_scenario` in `sim-data`, boundary validation helpers | **landed** (v9 — no bump, as designed) |
| B | `sim-server` skeleton: axum, session store, REST (create from scenario, inspect, snapshot GET/POST, delete), chemistry resolution | **landed** (v9 — no bump, as designed) |
| C | WebSocket: command/event protocol, explicit-`dt` batch stepping, report decimation, the one-writer rule. **Carries the exit criterion.** | **landed** (v9 — no bump, and no engine edit at all) |
| D | `sim-wasm` + the browser page: `wasm-bindgen` wrapper, chemistry TOML text handed in from JS, hand-rolled canvas plotting, zero external JS deps. Also — not in this line when it was written — `tower-http` and the static routes in `sim-server`, and the socket toggle | **landed** (v9 — no bump, and no engine edit at all) |
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

### Every `f64` crossing the wire needs `serde_json`'s `float_roundtrip` feature — measured, not assumed

**This is not a snapshot-only concern**, though snapshots are where it was found. The
exit gate parses `Telemetry` out of JSON to compare it bit-for-bit against an in-process
run; without the feature that comparison can fail on `v_terminal` or `q_gen_w` with no
snapshot anywhere near it. Any crate that parses engine floats out of JSON — server,
wasm module, **and the test client** — is a consumer of this requirement.


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
aging + plating configuration. That leg does draw from the pack RNG after the restore, and
it was checked rather than assumed: `PLATING_RISK` was raised on 75 steps before the split
and 75 after, and the serialized `word_pos` advanced 512 → 992 across the resumed half.
Note also that the re-serialized JSON *text* was unstable without the feature and stable
with it.

Consequences to bake in:

- Every crate that serializes a `Snapshot` as JSON — `sim-server`, and `sim-wasm` if it
  exposes snapshot strings — declares the feature. Put it in `[workspace.dependencies]`
  so it cannot be forgotten by a later crate.
- Slice B ships the regression test: snapshot → JSON → restore → continue is bit-identical
  against an uninterrupted run. Without it, a future dependency change that drops the
  feature is invisible until someone's restored session drifts.
  **Slice A's experience applies to it directly:** write it, then *verify it fails* by
  removing the feature from the workspace manifest and re-running. Slice A inherits the
  feature already on, so a snapshot test written from literals — or from a pack stepped
  too few times — will pass for the wrong reason and guard nothing.
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
- **`n_steps / report_every_n_steps` has its own cap** (frames per reply, default 10 000),
  rejected up front rather than truncated. The two caps are individually reasonable and
  jointly a footgun: a million steps at the default `k = 1` is a million-frame batch reply,
  which the "a batch delivers all its frames or errors" rule then obliges the server to
  actually send. Failing the command with "asked for 1 000 000 frames, cap is 10 000; raise
  `report_every_n_steps`" is the right answer — it names the knob, and it makes the
  fast-forward case (huge `n_steps`, coarse `k`) the one that works by construction.

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
# Exactly one of these two, and both are top-level keys — they must appear *above*
# the first table header or TOML swallows them into it. `chemistry` is an id the
# adapter resolves; `chemistry_toml` is the parameter set inlined verbatim, which is
# what makes a scenario self-contained for a server that ships no `chemistries/`
# tree. Two plainly-named optional keys, deliberately not one untagged enum: untagged
# enums in TOML are a known sharp edge, and "exactly one of these is required" is a
# two-line validation with a good error message.
chemistry = "lfp_26650_generic"
# chemistry_toml = """ [meta] ... """

[meta]
name = "overcharge with the BMS off"

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

## Learned while building — slice A (wire contract)

### The canary held, and it earned its keep once

`SNAPSHOT_VERSION` stayed at 9 and the bincode replay test passed untouched, which is
the check that actually matters (reading the constant proves nothing). But the slice
did make one engine edit, and the canary is exactly what forced it to be justified
rather than waved through.

**`BmsConfig::balancing` and `BmsConfig::protection` had no `#[serde(default)]`, and
TOML has no null.** So "a BMS that protects but never balances" — a state the engine
supports, a state `sim-core`'s own tests construct routinely — was *unwritable as a
scenario file*. Not awkward: impossible. Serde's derive treats a bare `Option<T>` field
as required, and there is no TOML literal that spells `None`.

The canary's rule is "the honest fixes are almost always 'put it in the adapter' or
'add a read-only accessor'." Neither applies here, and the third option the rule
implies — the adapter mirrors the type — is the DTO layer this plan already rejected.
So the fix went into the engine, and it is defensible on the engine's own terms:
`PackConfig` already marks `bms`, `aging`, `thermal`, and `scatter` as
`#[serde(default)]` for precisely this reason. These two were the same kind of
off-by-omission knob and were simply missed. `#[serde(default)]` affects
deserialization only — the fields are still always written — so no layout changed and
no bump was owed.

Worth stating as a general shape, because slice D will meet it again: **an `Option`
field on a config type that a scenario file can reach needs `#[serde(default)]`, or
that config's `None` case does not exist in TOML.**

### The float test had to be built to fail, and was checked by making it fail

The plan said `float_roundtrip` "fails silently and rarely." The trap that follows from
that, and which the plan did not spell out: a `Telemetry` round-trip assembled from
hand-written literals (`3.3`, `298.15`, `0.5`) passes **with or without** the feature.
Such a test looks like the regression guard the plan asked for and is worth nothing.

So `wire_json.rs` takes its numbers from a stepped pack — 4S2P LFP, scatter on, the
plan's own probe shape — because scatter is what fills the mantissas, and compares
`f64::to_bits` across twenty consecutive steps rather than one.

That it *is* discriminating was verified rather than assumed: dropping the feature from
the workspace manifest and re-running gives

```text
out: {"v_terminal":13.160750714657267, ...}
in:  Telemetry { v_terminal: 13.160750714657269, ... }
```

one ULP on `v_terminal`, plus `v_rc_sum` and `r0_factor` in `CellView`. Both tests fail;
the two shape tests still pass, which is the point — nothing except a full-mantissa
comparison sees this. Re-checked after the final seed change, since the values are
seed-dependent and "it failed once, with a different seed" is not the claim.

The cheap canary is worth knowing separately: the re-serialized JSON **text** is not a
fixed point without the feature. That is a string comparison, no floats in sight, and
it catches the same regression.

### Validation split: the boundary checks only what nothing downstream would

The plan's slice A line says "finiteness of every `f64`". Implementing that literally
would have given one condition two error messages that drift apart, because `Pack::new`
already rejects a zero topology, an out-of-range `initial_soc`, a non-positive
`initial_temp_k`, a bad thermal conductance, and every out-of-range `BmsConfig` field,
and `Pack::schedule_fault` already rejects a non-finite `at_s`, a non-positive short
resistance, and an out-of-topology cell index.

`Scenario::validate` therefore covers only the four things nothing else would:

1. exactly one of `chemistry` / `chemistry_toml`;
2. the chemistry id matches `[a-z0-9_]+` (a filesystem concern the engine has no
   business knowing about);
3. an inlined chemistry parses **and validates**, eagerly — so a self-contained
   scenario is whole or rejected, never accepted here and found broken later somewhere
   with no filename in hand;
4. the `Scatter` sigmas are finite and `>= 0`.

Number 4 is the real find. It is the one genuine gap in the engine's own checks: a NaN
sigma is not rejected anywhere, and `(1.0 + NaN·z).max(MIN_FACTOR)` returns
`MIN_FACTOR` — `f64::max` prefers the non-NaN operand — so every cell silently comes
out pinned at the minimum factor and nothing says a word. The engine could grow this
check instead; it did not, because a sigma is a *config* value and the boundary is
where config arrives.

`engine_owned_invalidity_survives_parsing_and_fails_at_build` pins the division so it
reads as a decision: a `nan` temperature parses fine and fails at `build_pack` with a
typed `DataError::Build`.

### `deny_unknown_fields` is asymmetric, on purpose

It sits on `Scenario` and `ScenarioMeta` — sim-data's own types — and is deliberately
not retrofitted onto `PackConfig`, which is an engine type with a compatibility surface
of its own. Consequence: `duration_s = 3600` beside `[pack]` is a parse error (good —
the temptation to make a scenario a demand program fails loudly), while `typo_here = 1`
*inside* `[pack]` is silently ignored. Pinned by test so finding it later reads as a
decision rather than a bug.

The related trap the plan already fixed in its own text is now structurally guarded:
`chemistry` is declared before every table-valued field on `Scenario`, so the TOML
serializer cannot emit it after `[meta]` — where re-parsing would read it as
`meta.chemistry`, a *different document that still parses*. A round-trip test would not
have caught that on its own; the field order is what makes it impossible.

### `build_pack` shipped here, not in slice B

`Scenario::build_pack(chem) -> Result<Pack, DataError>` — construct the pack, then
schedule the faults in file order. Small, needed identically by `sim-server` and
`sim-wasm`, and it is what lets the shipped fault example be *run* in slice A's tests
rather than merely parsed. That is what the plan meant by "the format has a user before
it has a server": `soft_short_example_runs_and_diverges` builds the file, steps it for
20 simulated minutes, and asserts the shorted group actually drains while the offset
sensor hides it.

Its companion `chemistry_source()` has to stay total on inputs `validate` would have
rejected, because slice B will construct `Scenario` values in Rust rather than through
`parse_scenario`. Both degenerate cases have a defined answer, and the choice is not
arbitrary: **both keys set resolves to the id**, so the error that eventually surfaces
names something the caller wrote, rather than throwing the id away and failing with a
chemistry-parse error about an empty string.

It also puts the fault-ordering rule in exactly one place. **File order is
load-bearing**: the engine's queue sorts by `at_s` and breaks ties by scheduling order,
so two faults sharing a timestamp fire top-down as written. Nothing sorts the file, and
nothing should — the plan's instinct to require sortedness would have been a
requirement the engine does not have.

### Two shipped scenarios, and what the second one is

`scenarios/cc_discharge_lfp.toml` — 1S1P, everything off, the readable one. Its comment
block spells out that each omitted section means *off*, not *on with defaults*, which
is the one thing about `PackConfig`'s serde defaults that will bite a scenario author.

`scenarios/soft_short_under_a_lying_sensor.toml` — 4S2P LFP, thermal network, full BMS,
aging on, two faults at the same timestamp: a soft internal short on cell (1,0) and a
+120 mV offset on that group's voltage sensor. It exercises every nested shape the
format has (enum-as-table `[pack.thermal.Network]`, `Option` sub-tables, a
`Vec<(u16,u16)>`, externally-tagged faults with a nested `SensorId`), and it is a real
teaching scenario rather than a syntax exhibit: ground truth and the BMS view diverge,
and the BMS never trips.

Note what it is *not*: it does not mirror a specific phase-3 Rust test's `PackConfig`.
The phase-3 fault tests build synthetic chemistries inline, so a file naming
`lfp_26650_generic` could not have reproduced one of them, and claiming it did would
have been a label the test could not cash. The equivalence assertion that does the work
is against a fully-written-out `PackConfig` literal in the test — which is what catches
a serde field name or enum shape that TOML accepts but that does not mean what the file
says. A spot-check would pass with `[pack.thermal.Network]` silently falling back to
`Isothermal`.

### Where the JSON tests live, and why not in `sim-core`

`crates/sim-data/tests/wire_json.rs`, with `serde_json` as a `sim-data` dev-dep. The
types under test are `sim-core`'s, so `sim-core` was the obvious home — but the probe
needs a real chemistry off disk to produce full-mantissa floats, and `sim-core` performs
no file I/O. Hosting them there would have meant copying a chemistry inline (as
`scenario_runaway.rs` has to) purely to test a serialization format.

`sim-core` keeps its dev-dep comment's promise: the engine still declares no
serialization format of its own. The adapter chooses, and here the adapter's choice is
what is on trial.

---

## Learned while building — slice B (`sim-server` skeleton)

### The canary held, and it bought two accessors

`SNAPSHOT_VERSION` stayed at 9 and `sim-core`'s bincode replay test passed untouched.
The slice made one engine edit: `Pack::series()` and `Pack::parallel()`.

The canary's rule offers "add a read-only accessor" as an honest fix, and this is that
case. The justification wants stating carefully, though, because the tempting version
of it does not survive contact with the rest of the slice: *"reading topology off the
stored `PackConfig` goes stale the moment a snapshot is restored"* is true in the
abstract and defused in practice by this slice's own topology check on restore. The two
arguments partly cancel, and a note that leans on the cancelled one will mislead
whoever reads it next.

What actually forces the accessors is smaller and holds: **the topology check itself
needs `restored.series()`**, which exists nowhere else — a `Snapshot`'s inner pack is
private. Having paid for that, reading the live topology from the pack everywhere is
single-source-of-truth rather than a second mechanism. Both numbers were already public
API, incidentally — `CellIndexError` reports them — merely unreachable without provoking
an error first.

### Open question resolved: shared map, and it is two levels deep

`BTreeMap<SessionId, Arc<Mutex<Session>>>` behind a registry `Mutex`, with a stated
lock order (registry → clone the `Arc`s → release → session). Not a task per session:
slice B has no streaming, so a task and a channel would be machinery with nothing to
carry yet, and slice C can convert the value type without touching a handler.

The *two-level* shape was worth building now rather than later, and not for the reason
that usually justifies it. Slice C's stepping commands hold a session's lock for the
duration of a batch — up to a million steps by the configured cap. A single mutex over
the whole map would freeze every other session and every REST request for that entire
time. That is a liveness property of "many sessions, one writer each", not a
micro-optimisation, and it costs one `Arc`.

Related and deferred, stated so it is not rediscovered as a mystery: a long batch is
CPU-bound work on a tokio worker thread. Slice C has to decide between `spawn_blocking`
and a dedicated thread per session. Nothing in slice B blocks long enough to care.

### What a restore *means*, and the check that is deliberately incomplete

`POST /sessions/{id}/snapshot` replaces the pack **in place** — same id, same session,
so anything watching it keeps watching it. A topology mismatch is a 409 rather than a
silent swap, because the overwhelmingly likely cause is the wrong snapshot file.

The check is knowingly partial, and this is the part worth writing down. A snapshot
carries its own `ChemistryParams`, so a 4S2P LFP session restored from a 4S2P **NMC**
snapshot passes the topology check and leaves the session's stored `Scenario` naming a
chemistry the pack no longer runs. Closing that would mean exposing the engine's
chemistry for comparison — engine surface added purely to serve an adapter, which is
exactly what the canary says to stop and question.

The design that makes the gap cheap is the one the accessors above exist for: every
live fact (`series`, `parallel`, `sim_time_s`, the cell array) is read from the pack,
and the stored `Scenario` is labelled **provenance** in its own doc comment and in the
JSON field's meaning. So the residual cost of the gap is one misleading provenance
field, not an endpoint that lies.

Two more things a review caught that the slice would otherwise have shipped without,
both worth recording as shapes rather than as incidents:

- **The inline-chemistry path had no coverage at all.** Both shipped scenarios name a
  chemistry *id*, so nothing exercised `chemistry_toml` — and it is the harder path,
  because an arbitrary multi-line TOML document has to survive being re-serialised as a
  JSON *string* by `GET /sessions/{id}` and parsed back. It does, and
  `a_scenario_can_inline_its_chemistry_and_survive_the_round_trip` now says so with
  `chem_dir` pointed at a directory that does not exist, so a silent fallback to the
  filesystem would fail it rather than hide in it. The general shape: **shipped example
  files decide what gets tested**, and two examples that agree on a choice leave the
  other branch invisible.
- **`create_session` returns the session, not just its id.** The handler used to look
  the session straight back up, and the registry lock is released in between — so a
  `POST /sessions` that had just succeeded could answer 404. Nothing in this slice can
  actually lose that race; the point is that a race the type system describes will
  eventually be lost by slice C, which does have concurrent deletes.

`ErrorCode`'s wire spellings are now pinned variant-by-variant
(`error_codes_have_pinned_wire_spellings`). The variant names are `snake_case`d by a
serde attribute, so a rename moves the string a client matches on while every
assertion written against the Rust name keeps passing. Slice C's `Error` event reuses
this vocabulary, which is what makes it worth a test rather than a doc comment.

`latest_telemetry` is cleared by a restore for the same reason, and is `null` on a
session that has never stepped. The engine's zero-length-step contract would have let
the server synthesise a frame with `dt = 0`; it deliberately does not. A stepping
protocol is slice C's to design, and a session that has not stepped honestly has no
telemetry.

### The float guard was built to fail, and this is what its failure looks like

Removing `float_roundtrip` from the workspace manifest fails all three tests in
`crates/sim-server/tests/snapshot_json.rs`. The corruption is in the snapshot's
*static* per-cell values:

```text
capacity_factor: 0.9720750261294301  ->  0.97207502612943
r0_factor:       0.9869400641202459  ->  0.986940064120246
v_rc:           [0.014494128773812195] -> [0.014494128773812197]
```

and it surfaces on the **very first resumed step**, not after a long drift:

```text
step 300 (t = 450 s) diverged after the restore at step 300
  uninterrupted: soh_resistance: 1.0007013776964933, q_gen_w: 0.5173449041614148
  resumed:       soh_resistance: 1.0007013776964935, q_gen_w: 0.5173449041614212
```

That immediacy is worth knowing: the mis-rounded values are the factors every step
multiplies through, so there is no incubation period. A test that only compared the
*endpoint* of the two runs would still have caught this one — but the same is not true
of a one-ULP hit on a slowly-integrating accumulator, which is why the assertion is
per-step.

The new guard slice A did not have: `longest_digit_run(snapshot_json) >= 15`. The
failure mode of this whole test is silence — a probe that drifts toward round numbers
keeps passing while testing nothing — so the probe asserts its own inputs are
full-mantissa. `fresh_pack` likewise asserts the scenario it loads still has scatter
and a non-zero `current_noise_sigma_a`, since either could be edited out of the
scenario file by someone with an unrelated goal.

That noise sigma is what makes the test cover RNG continuity: the BMS draws from the
pack RNG once per step and its estimate depends on the draw, so a restore that lost RNG
state would diverge in `soc_bms`. And the scenario's faults are timestamped 600 s
against a split at 450 s, so the not-yet-fired queue has to survive the round trip too —
asserted directly (`i_internal_short_a` is zero at step 399 and non-zero at step 400),
because a restored pack that silently dropped its queue would otherwise make the
comparison prove less than it looks.

### Validation is centralised because one of the two entry paths does not do it

`parse_scenario` validates; `serde_json::from_str::<Scenario>` does not. Since
`POST /sessions` accepts both encodings, putting validation in the parse fork would
have been two call sites with one of them easy to forget — and forgetting it on the
JSON path bypasses the `[a-z0-9_]+` charset check, which is the only thing standing
between a request body and a path walk out of the chemistry directory.

So the fork returns an unvalidated `Scenario` and `AppState::create_session` validates
unconditionally, before anything touches the filesystem.
`the_json_path_still_validates_the_chemistry_id` posts `chemistry =
"../../../etc/passwd"` as JSON and expects a 400. This is also why slice A's note that
`chemistry_source()` stays total on unvalidated input matters in practice: it is
documented not to have vetted anything, so the sequence is always validate-then-resolve.

### The papercut that only running it could find

The handler's doc comment originally promised that
`curl --data-binary @scenarios/cc_discharge_lfp.toml` worked with no ceremony, reasoning
that `--data-binary` sends no `Content-Type`. It does not: curl labels it
`application/x-www-form-urlencoded`, the same as `-d`. The single invocation the
documentation promised was the single invocation that returned 415.

Found by starting the binary and running the documented command, which took two
minutes. The fix accepts that type as TOML — nobody chooses it deliberately for a
scenario file — while `application/xml` still gets its 415, so the code path stays
real. `--data-binary` rather than `-d` remains the right advice for a separate reason:
`-d @file` strips newlines, which mangles TOML.

### What is deliberately not here

- **No stepping endpoint.** Advancing the simulation needs explicit `dt`, batching, and
  the one-writer rule; none of those survive a stateless request/response cycle. Better
  no endpoint than a `POST /step` that slice C would have to deprecate.
- **No `tower-http`.** The plan listed it under slice B for static files, but the page
  it would serve does not exist until slice D, and a `ServeDir` over a nonexistent
  directory is a dependency doing nothing. It arrives with its user.
- **No `clap`.** Two flags parsed from `std::env::args`.

### Measured

A 4S2P snapshot is **5658 bytes** of JSON, which confirms the "~5 KB" figure the open
question below was written against — so the extrapolation to ~600 KB at 100S10P, and
the answer to it, stand as written.

---

## Learned while building — slice C (WebSocket)

### The canary held, and this time it cost nothing

`SNAPSHOT_VERSION` stayed at 9, `sim-core`'s bincode replay test passed untouched, and
— unlike slices A and B, which each needed one engine edit — **slice C changed no engine
file at all**. `git diff --stat -- crates/sim-core crates/sim-data` is empty.

That is the canary's premise coming out ahead rather than merely surviving: the entire
stepping protocol is a skin over `step`, `schedule_fault`, `clear_faults`,
`clear_bms_fault`, `snapshot` and `restore`, plus two pieces of state the engine has no
business owning (the standing environment, and whether the session still exists). The
accessors slice B bought (`series`, `parallel`) were the last thing the adapter needed.

### The protocol's encoding is forced by a `u128`, not chosen for consistency

The plan said engine enums stay externally tagged for consistency and gave the demo page
"a three-line helper". The right shape for `Command` and `Event` looked like an open
question — internally tagged (`{"cmd": "step", …}`) is what a JavaScript client wants.

It is not an option. Serde's internally tagged deserializer buffers the whole value into
its private `Content` type first, and `Content` has no `u128`. A `Snapshot` has one:
`ChaCha8Rng`'s `word_pos`. Measured on this repo's own snapshot:

```text
direct                     round-trips exactly
externally tagged          round-trips exactly
adjacently tagged          round-trips exactly
via serde_json::Value      round-trips exactly
internally tagged          FAILS: u128 is not supported
#[serde(flatten)]          FAILS: u128 is not supported
```

Two things worth carrying forward:

- **The failure is a runtime error on a program that compiles perfectly**, and it only
  fires on the one command that carries a snapshot. A protocol that tagged internally
  would have passed every `Ping`/`Step` test and broken on `Restore`. Pinned by
  `command_and_event_wire_spellings_are_pinned`, which asserts the failure *and* the
  `u128` in its message — so if a future serde lifts the restriction, the test failing is
  the signal that the constraint is gone.
- **The same trap bans `#[serde(flatten)]`**, which is why `Frame` nests its telemetry in
  a field rather than flattening it. `Telemetry` alone would survive flattening; making
  it the house style would break the first time someone flattened something with a
  snapshot inside.

The *precision* fear that started this probe turned out to be unfounded — `float_roundtrip`
does reach `deserialize_any`, so `serde_json::Value` round-trips exactly. The hazard is
availability, not accuracy. Worth stating because the opposite is widely assumed.

### JSON cannot express a non-finite number, so half the boundary rule is unreachable

The plan's validation section reads as though `NaN` arrives over the wire: "every `f64`
in a `Demand`, an `Env`, or a scenario must be finite. Reject the message with a
structured error." A client cannot send one. Measured, three different refusals with one
outcome:

```text
{"dt":NaN,…}    -> expected value              (no JSON literal for NaN)
{"dt":1e400,…}  -> number out of range         (the parser refuses to overflow)
{"dt":null,…}   -> invalid type: null          (what serde_json writes for f64::NAN)
```

So over this socket a non-finite value is an `invalid_command` from serde, raised
*before* `StepCommand::validate` is ever called, and the plan's implied `out_of_range` for
it never happens. The first draft of the test asserted `out_of_range` and failed, which is
how this was found.

The checks stay, and this is the part worth being careful about, because "unreachable"
usually means "delete it": `StepCommand` is a Rust type as much as a wire type. **Slice D's
wasm client will construct one in Rust and call `validate` on it with no JSON parser in
between**, and a binary framing would carry `NaN` happily. So the tests split in two —
`json_refuses_a_non_finite_number_before_validation_can_see_it` pins what the socket
actually does, and `validate_rejects_every_non_finite_field` calls the checks the way
slice D will. Reachable range violations (a negative `dt`, `n_steps = 0`, `k = 0`, both
caps) are tested over the socket, because those *are* reachable.

### A zero-length step's frame is not a copy of the previous frame

`dt = 0` is in the protocol so a client can read telemetry without advancing. The obvious
test — "the frame equals the last one" — fails, on `q_gen_w`, and the reason is a
property of the engine that no adapter document had written down: **telemetry is computed
from start-of-step state**. Step 5's frame reports the heat implied by the state at the
*start* of step 5; a zero-length step afterwards reports the heat implied by the state at
the *end* of it. Same pack, one step apart in what the number describes.

That is exactly what makes `dt = 0` useful — it answers "what is the pack doing now",
which is a different question from "what was it doing during the last step". So the test
asserts what is actually claimed: two consecutive zero-length steps are bit-identical to
each other (nothing mutated), the clock did not move, and the *state* fields match the
previous frame while the rates need not.

### `SetEnv` only means something if the session has a standing environment

The plan lists both `Step { …, env }` and `SetEnv` without saying how they relate; taken
literally they are redundant. Resolved: the session holds a standing `Env`, seeded from
the scenario's `initial_temp_k` with no coolant; `SetEnv` replaces it; a `Step`'s `env` is
`Option` and **overrides for that batch only, without persisting**. A command's effect is
scoped to the command unless the command is `SetEnv`.

The standing environment is session state, not pack state, so a restore leaves it alone —
a snapshot describes a pack, and the room it sits in is the client's business.

Testing it needed a scenario the plan would not have picked. `cc_discharge_lfp.toml` is
isothermal (no `[pack.thermal]` section means the thermal model is *off*, not on with
defaults), so its cells sit at `initial_temp_k` whatever the ambient and every assertion
about the environment would have been vacuously true. The test uses the thermally-coupled
scenario, clears its faults so the only thing moving temperature is the room, and rests
6000 s per leg against a ~270 s time constant. **A configuration knob can only be tested
on a configuration that reads it** — obvious once written, and the failing first draft is
what wrote it.

### Two loops, because the backpressure rules should be structural

A writer's socket is strictly request/response; an observer's is pushed events while
still being watched for a close. Rather than one loop with bookkeeping, they are two
functions, and the plan's two rules then fall out of the shape instead of being enforced:

- The writer's frames go straight down its socket, so a writer that reads slowly gets TCP
  backpressure and a slow batch — never a hole. A batch's reports are the experiment's
  record.
- An observer reads from a 256-deep `broadcast`; falling behind yields `RecvError::Lagged(n)`
  which becomes `Dropped { count: n }`. One slow observer cannot freeze the session.

**Commands are never reordered because the writer's socket is not read during a batch.**
That looks like an omission in the code and is the mechanism: further commands sit in the
kernel's receive buffer and arrive in order. It carries a comment, because "we
deliberately do not read here" reads as a bug.

The `Dropped` path was the one thing in this slice with no test in the plan's list, and
it is now `an_observer_that_falls_behind_is_told_how_much_it_lost`. Forcing genuine lag
needs more bytes than the socket buffers absorb, so it floods a full-cap 10 000-frame
batch at an observer that is not reading. Measured, and repeatably: **706 delivered, 9294
dropped** — an enormous margin, not a marginal trigger. The assertion stays one-sided
(`dropped > 0`, and `seen + dropped == n`) because how many the buffers swallow first is
a property of the operating system.

### `spawn_blocking`, decided by the test harness rather than by the runtime

A million steps is tens of seconds of CPU on a tokio worker. `block_in_place` is the
tidier call and would keep the session guard in scope — but it *panics on a
current-thread runtime*, which is what plain `#[tokio::test]` gives you. Choosing it
would have silently obliged every test in this crate to carry `flavor = "multi_thread"`
forever, and the failure for anyone who forgot would be a panic in unrelated code. So:
`spawn_blocking` with a `'static` `Arc<Mutex<Session>>` and `blocking_lock` *inside* the
closure, and a typed `internal` error if the task does not come back.

Frames are collected under the lock and sent after it is released. That is what makes the
frame cap do two jobs — it bounds the reply *and* it bounds server memory — which the
plan only wrote down the first half of.

### `DELETE` needed a flag, not just an unregistration

Slice B's `remove_session` drops the session from the registry. With a socket attached
that is not enough: the socket holds its own `Arc` and would go on stepping a pack no
route can reach. So a removed session is also flagged, every command checks the flag, and
`a_deleted_session_stops_accepting_commands` pins it. The two locks are taken in the order
`AppState` already states — registry first, released before the session lock — which
matters more here than anywhere else, because a batch may hold the session lock for a
million steps.

### What the exit gate's failure actually looks like

Removing `float_roundtrip` from the workspace manifest fails both tests in
`e2e_experiment.rs`, and it fails at **step 5** — with no snapshot anywhere near it:

```text
step 5 (t = 9 s) differs between the in-process run and the WebSocket run
  in process:    v_terminal: 13.109000724359921
  over the wire: 13.10900072435992
```

That is the plan's "this is not a snapshot-only concern" confirmed rather than repeated.
The one-ULP hit is in the *test client's* parse of a telemetry frame; the plan named the
test client as a consumer of the feature and it was right to.

`ws.rs` still passes without the feature, and the reason is worth knowing: every
comparison in it is wire-to-wire (observer against writer, decimated against
undecimated), so both sides are mis-rounded identically. **Only a test that crosses the
boundary can see this.** A future slice that adds "more transport tests" is not adding
coverage of it.

### Resolved while building

- **The WebSocket client dependency is not free to pick.** `tokio-tungstenite` must be
  the version `axum`'s `ws` feature pulls in transitively (0.29 for axum 0.8.9); a
  mismatch compiles cleanly and fails at handshake time. Stated in the manifest next to
  the dev-dep so a future axum bump knows to check.
- **`futures-util` became a real dependency, not just a dev one.** Splitting an
  observer's socket needs `StreamExt::split`. The writer's socket deliberately is not
  split, which is the same asymmetry as above.
- **The test client is hand-written**, including ~40 lines of HTTP. `tests/rest.rs` drives
  the router through `tower::ServiceExt::oneshot`, which is right for testing handlers
  and wrong for a criterion that says *external script*: the claim is only true if the
  bytes go through a socket. `Connection: close` makes read-to-EOF an unambiguous end of
  response, so no length or chunk parsing is needed.

### What is deliberately not here

- **No promotion of observers.** When a writer disconnects the slot is freed and the
  *next* socket to attach takes it; existing observers stay observers. A role is announced
  once in the hello frame, and a client told it is read-only should not silently acquire
  the ability to move the pack.
- **No `Ping` token or correlation id.** `BatchComplete` is the barrier a client waits on,
  so `Ping` has no work to do beyond liveness.
- **No snapshot compression.** A 4S2P snapshot is ~5 KB; the open question below stands
  as written.

---

## Learned while building — slice D (`sim-wasm` + browser page)

### The canary held, and this time it cost nothing either

`SNAPSHOT_VERSION` stayed at 9 and `sim-core`'s bincode replay test passed untouched.
Like slice C, **slice D changed no engine file at all** — `git diff --stat -- crates/sim-core`
is empty. `sim-data` is untouched too. Everything the browser needs was already public:
`parse_scenario`, `parse_chemistry`, `Scenario::build_pack`, and the accessors slice B
bought.

The one edit outside the new crate is in `sim-server`, and it is the one slice B
deferred on purpose: `tower-http` and three `ServeDir` routes. Slice B's note said "it
arrives with its user", and this is the user.

### The slice was four deliverables, not two, and the third is the one that hides

The plan's slice-D line names the wasm wrapper and the page. Two more are real work and
are only findable by reading the *notes* around the slice list:

- **`tower-http` + the static routes in `sim-server`.** A `sim-server` edit inside a
  slice titled `sim-wasm`, which is exactly why it is easy to drop — and without it the
  page cannot load at all, because a wasm module cannot be fetched from `file://`.
- **The socket toggle in the page.** It sits under "Open questions" phrased as a
  question and is actually a decision: *"Keep the socket path in the page anyway, behind
  a toggle, so the server protocol has a live client and does not rot."*

Worth stating as a shape: **when a plan's prose and its slice table disagree about
scope, the prose wins** — it is where the reasoning was written down.

There is a third route the plan did not anticipate at all. `/chemistries` and
`/scenarios` are served as static text because the plan's own resolution for `sim-wasm`
("there is no filesystem; JS fetches the TOML and passes the *text*") has a serving end
that nobody wrote down. `--web-dir` and `--scenario-dir` join `--chem-dir`; all three
default to the repo layout, so `cargo run -p sim-server` from the workspace root is
still the whole setup.

### `SimEngine` is separate from `Sim` because the gates run on the host

`sim-wasm` stays inside `cargo test --workspace` and `clippy --all-targets`, which means
it is compiled **for the host target**. On the host a `JsError` is a stub for a browser
type that does not exist. A test that went through the `#[wasm_bindgen]` façade would be
testing the stub.

So every decision lives in `engine::SimEngine` — typed errors, `Result` everywhere, no
JS type in any signature — and `Sim` is sixteen methods of `?`. All sixteen tests are on
`SimEngine`. That split is what makes "this crate is in the gates" mean something rather
than merely compile.

Two `wasm-bindgen` facts found by hitting them:

- **`#[wasm_bindgen]` refuses to export a `const`** (it is only meaningful there for a
  `typescript_custom_section`). `WASM_API_VERSION` stays Rust-side with a
  `wasm_api_version()` accessor.
- **`wasm-bindgen` already provides `impl<E: Error> From<E> for JsError`**, which is
  `JsError::new(&e.to_string())`. Writing that impl by hand is a coherence error, not an
  improvement — and the blanket one is what makes a browser console show `EngineError`'s
  own words.

### The duplication is deliberate, and named so it cannot drift quietly

The caps (`MAX_STEPS_PER_CALL`, `MAX_FRAMES_PER_CALL`), the finiteness checks, and
`Frame`'s shape mirror `sim_server::protocol`. Sharing them would mean depending on
`sim-server`, which drags `axum` and `tokio` into a crate whose whole point is running in
a browser. So they are copied, with the module doc saying where the original is and what
to do if it happens again: **a third client is the trigger to lift
`Limits`/`StepCommand`/`Frame` into a `sim-protocol` crate**, not to add a third copy.

`Frame` being field-identical is not incidental — it is why the page plots frames from
the embedded engine and frames from the socket with the same code, and therefore why the
two paths cannot quietly disagree about what a sample is.

### Slice C predicted this caller exactly, and it was right

Slice C found that a non-finite number is unreachable over the socket — JSON has no
literal for `NaN`, `serde_json` refuses `1e400` — and kept `validate_rejects_every_non_finite_field`
anyway, on the grounds that "slice D's wasm client will construct one in Rust and call
`validate` on it with no JSON parser in between".

That is this crate. `dt` and the step counts cross the wasm boundary as raw numbers, and
`Number.NaN` needs no literal to exist. `a_non_finite_dt_reaches_validation_because_nothing_parses_it`
is the test that a socket cannot write.

### The float guard fails on the first resumed step, again

Dropping `float_roundtrip` from the workspace manifest fails
`a_snapshot_through_json_resumes_bit_identically`, at **step 1** after the restore, on
`v_cell_max`:

```text
step 1 (t = 451.5 s) diverged after the restore
  left:  …4614572410408579126…
  right: …4614572410408579127…
```

Same immediacy slice B measured, for the same reason: the mis-rounded values are the
per-cell factors every step multiplies through, so there is no incubation period. The
test carries slice B's `longest_digit_run(json) >= 15` self-check, because the failure
mode of a round-trip test is silence.

### The BMS toggle is a comparison of two runs, and it found a hole

A page control that says "BMS enabled" invites the reading "flip it mid-run". There is no
honest way to grow a BMS onto a pack that has been running without one, so `restart`
**rebuilds from the scenario** and the run returns to t = 0. The doc comment says why
rather than apologising: contrasting protected and unprotected is a comparison of two
runs, which is what makes it a teaching case.

Then the first run of the test failed, and this is the find:

```text
rebuild without the BMS: Data(Fault(NoSuchSensor { sensor: GroupVoltage(1) }))
```

**`scenarios/soft_short_under_a_lying_sensor.toml` fault-injects a sensor, and sensors
belong to the BMS.** Remove the BMS and the scenario's own second fault has nothing to
land on. Three options, and the middle one is right:

- *fail* — makes the toggle unusable on precisely the scenario it exists to illuminate;
- *drop the sensor faults and count them* — `PackFacts::sensor_faults_dropped`, surfaced
  by the page as "1 sensor fault(s) dropped: no BMS to sense them";
- *drop them silently* — lets a student compare two runs that differ in more ways than
  the label claims.

The filter applies **only** when a BMS is being removed. A scenario that ships no BMS and
a sensor fault is an authoring error and still fails loudly at construction
(`a_sensor_fault_with_no_bms_to_sense_is_an_authoring_error`).

The contrast test itself needed the same kind of correction. The obvious assertion —
"protected stays under `v_max`, unprotected sails past it" — is wrong twice. Protected
peaks at **3.6604 V against a 3.65 V limit**, because protection acts on a reading taken
at the start of a step and may overshoot by one step (a phase-2 decision, owned); and
unprotected only reaches 3.6789 V, because SOC clamps at 1.0 and the OCV table tops out
there, so an unprotected overcharge parks at `OCV(1) + I·R` rather than climbing.
Asserting a dramatic overshoot would have been asserting physics v1 does not model. The
discriminator that actually holds is **whether the pack got what it was asked for**:
unprotected, the demand passes through bit-identically on every step; protected, the BMS
takes it to zero.

The toggle is disabled in socket mode, with the reason on screen. The server builds the
pack the scenario asked for, and a page that rewrote someone's TOML to fake a variant
would be inventing a scenario the user never wrote.

### What running it found that reading it could not

Slice B's lesson was that starting the binary and running the documented command takes
two minutes and finds what no amount of reasoning does. Slice D is the same story four
more times. All four were found by loading the page in a browser and watching it.

1. **The page loaded into a row of dashes and stayed there** until Run was pressed — and
   worse, it did the same *after a successful restore*, which is the one impression a
   restore must not give. The fix is the engine feature that already existed for it: a
   zero-length step. `dt = 0, n_steps = 1` reads telemetry without advancing, which the
   plan's validation section explicitly allowed "because the browser page wants it on
   connect". The page was not using it. It now reads on load, after a restart, and after
   a restore.
2. **Pack terminal voltage and cell voltage cannot share an axis.** A 4S pack sits near
   13 V while its cells sit near 3.3 V; one auto-scaled axis flattens both against
   opposite edges and the cell *spread* — where the imbalance physics is — disappears at
   the moment it matters. Two panels.
3. **The legend ran off the right edge of the canvas**, and a canvas has no scrollbar to
   reveal what fell off it. Measure the legend and right-align it against the plot edge,
   clamped to the plot area so a long title cannot push it out.
4. **Fixed 2-decimal y labels repeat themselves** on a cell-voltage panel spanning 12 mV:
   two ticks 5 mV apart both render `3.23`. Decimals now come from the tick step. And a
   single sample has no time span, so ticking it anyway printed `0s` six times — a
   degenerate axis draws one label.

What the page shows when it does work is the scenario's whole point, visible without
reading a number: the pack terminal curve **steps down at t = 600 s** where the short
fires, the cell-voltage minimum peels away from the maximum from that instant, the
internal-short trace steps from 0 to 0.65 A, one cell runs ~2 °C hotter than the rest,
and the BMS estimate stays cheerfully above ground truth the whole way — 26.4 % true
against 33.0 % estimated by t = 75 min, with **no flags raised at any point**.

### Verifying in a browser needed a throwaway harness, and that is worth knowing

The automation window is occluded, so `document.visibilityState` is `hidden` and Chrome
**parks `requestAnimationFrame`**. The run loop is a rAF accumulator, so nothing steps.

Re-importing `app.js` under a fresh URL with a shimmed rAF *works* but leaves two live
module instances fighting over the same canvases — and the second instance appends its
own readout tiles, so the DOM lies about which numbers are current. Ten minutes went into
diagnosing a symptom that was entirely an artifact of the workaround. The clean approach
is a temporary `web/_probe.html` that installs the shim in a `<script>` **before**
`app.js` runs, and is deleted afterwards.

Not a page bug, and no code changed for it: a backgrounded tab pausing the simulation is
correct, and the accumulator's 0.25 s clamp means returning to the tab does not
fast-forward. But it is the kind of thing that costs an hour if it is rediscovered from
scratch.

### Resolved while building

- **The chemistry fetch is a two-step dance, and Rust owns the first step.**
  `chemistry_id_of(scenario_toml)` parses the scenario and tells JS which file to fetch
  (or `undefined` for an inlined one), so no TOML parsing exists in the page at all. The
  id is already charset-checked by `parse_scenario`, which is what makes interpolating it
  into a URL safe without escaping.
- **`/app` needs its own redirect to `/app/`.** Under `nest_service` the inner path for
  `/app` is the empty string — neither file nor directory — so it 404s; and every
  relative URL in the page resolves against the wrong base without the slash. Load-bearing
  twice, pinned by test.
- **A missing `web/pkg` must not take the API with it.** The bundle is a build artifact
  and is not committed, so a fresh clone is the default state. `/app/` 404s, everything
  else works, the binary warns at startup with the command to run, and `GET /` carries the
  same command in-band — the person who hits it is holding a browser, not this file.
- **The socket backend queues commands rather than rejecting concurrent ones.** Dragging
  the ambient slider during a batch would otherwise race the outstanding `Step`. The
  server's rule is that commands are never dropped and never reordered; the client should
  not be the thing that breaks it.
- **A `Float64Array` fast path was not built**, per the plan. Nothing measured says the
  JSON crossing costs anything at a few hundred plotted pixels, and the plan says not to
  pre-optimize the boundary.
- **The caps are not restated in JavaScript, and the hello frame finally has a reader.**
  The first draft of the page hardcoded `1_000_000` and `10_000` — a *third* copy of two
  numbers already duplicated between `sim-server` and `sim-wasm`, and the only copy with
  neither a compiler nor a test behind it. Worse, it was wrong for the socket path:
  `sim-server`'s limits are configurable (`AppState::with_limits`), so a server started
  with a lower cap would reject batches the page believed were fine, while the page's own
  "this is a bug in the decimation" guard checked the wrong number and never fired. Each
  backend now reports `limits()` from its own authority — the wasm module from exported
  accessors, the socket from `hello.limits`. That field's doc comment says it exists "so
  a client can size its batches instead of discovering them by being rejected"; until
  this it had no client doing so.

### A zero-length step is still a step, and that is the honest answer

Reading on load means a socket session shows `stepped: true` and a non-null
`latest_telemetry` before anyone has pressed Run, which looks like it contradicts slice
B's "a session that has not stepped honestly has no telemetry."

It does not, and the distinction is worth keeping straight: slice B's rule is that the
**server** will not synthesise a frame it was not asked for. A page sending
`{dt: 0, n_steps: 1}` *asked*. The session really has been stepped — by zero seconds, on
purpose, using a contract the engine pins — and reporting that is accurate. Nothing
changed for this.

### What is deliberately not here

- **No `console_error_panic_hook`.** A dependency for debugging convenience, on a crate
  whose whole public surface returns `Result`.
- **No per-batch `env` override on the wasm surface**, unlike the server's `Step`. A page
  owns its environment as a control; an override it would have to re-send every animation
  frame is a footgun with no user.
- **No README changes.** Slice E owns the README, the example script, and the latency
  measurement.

---

## Open questions (decide when the slice lands, not now)

- ~~**Session task vs shared map.**~~ Resolved in slice B: shared map, two levels of
  locking. See "Open question resolved" above.
- **Snapshot body size.** A 4S2P snapshot is ~5 KB of JSON (measured: 5658 bytes).
  100S10P will be ~600 KB, which is fine for REST and poor for a WebSocket frame. If it
  bites, the answer is `Content-Encoding` on the REST route, not a new binary format on
  the socket.
- ~~**Whether the browser page needs the server at all after slice D.**~~ Resolved in
  slice D, and the answer is *both halves are true*. It does not need it for physics — it
  embeds the engine, and the socket toggle is off by default. But it needs the server to
  exist at all: a wasm module cannot be fetched from `file://`, and the page has no
  filesystem, so the scenario and chemistry TOML it hands to `sim-wasm` arrive over HTTP.
  The socket path is kept behind the toggle as planned, and it exercises the whole
  protocol — session create, hello, `Step`, `SetEnv`, `Snapshot`, `Restore`, delete.
