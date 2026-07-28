# batsim

A battery-pack simulator in Rust. One deterministic engine serving two purposes:

1. **Pedagogy** — experiment with chemistries, charge/discharge regimes, aging,
   faults, and battery protection, and *see* what happens.
2. **Headless engine** — other software (e.g. a Godot game via gdext) steps the
   simulation and queries voltage, current, SOC, SOH, temperature, and more.

The engine is the product. Every UI, server, and game is just a client of
[`sim-core`](crates/sim-core).

## Design contract

`sim-core` is a pure, deterministic state machine — `step(dt, demand, env) -> Telemetry` —
with no I/O, no async, no globals, and one seeded RNG whose state is part of every
snapshot. The full contract (design principles, physics spec, chemistry file format,
determinism rules, testing strategy, and the phased build plan) lives in
[`CLAUDE.md`](CLAUDE.md).

Key invariants:

- **Positive current = discharge** (current out of the pack terminals).
- SI units throughout `sim-core` (seconds, amperes, volts, ohms, farads, joules, kelvin).
- Everything is snapshotable and replayable bit-identically on the same binary.
- Chemistry is data — a TOML parameter set — never code.
- Ground truth is not the BMS's view. The engine knows every cell; the BMS only reads
  simulated sensors and keeps its own estimate. The gap between the two is a feature.

## Status

Phases 0–6 are complete. Each phase's exit criterion is a committed test rather than a
judgement call, so the right-hand column is the thing to run if you doubt a row.

| phase | what landed | exit criterion pinned by |
| ----- | ----------- | ------------------------ |
| 0 | workspace, `sim-core` types, TOML chemistries, 1RC/2RC ECM cell, exact RC update | `sim-core/tests/analytic_golden.rs` |
| 1 | series/parallel packs, closed-form group solve, seeded scatter, snapshots, PyBaMM goldens | `sim-core/tests/scenario_weak_cell.rs`, `snapshot.rs`, `sim-data/tests/pybamm_golden.rs` |
| 2 | thermal network, sensor layer, SOC estimator, protection, passive balancing | `sim-core/tests/thermal.rs`, `scenario_protection.rs`, `sim-data/tests/scenario_lfp_soc_drift.rs` |
| 3 | calendar + cycle aging, fault queue, plating flag, thermal runaway and propagation | `sim-core/tests/scenario_aging.rs`, `scenario_runaway.rs` |
| 4 | `sim-server` (REST + WebSocket), `sim-wasm`, the browser demo, the example script | `sim-server/tests/e2e_experiment.rs` |
| 5 | `sim-godot` — a `BatteryPack` GDExtension node, exported properties, fixed-`dt` accumulator, signals, and a Godot demo scene | `sim-godot/tests/godot_gate.rs` (needs Godot — see below) |
| 6 | the `Spm` porous-electrode cell model — radial solid diffusion, Butler–Volmer kinetics, a nonlinear pack solve, and an extracted LG M50 parameter set | `sim-data/tests/spm_golden.rs`, `spm_exact_bits.rs`, `sim-core/tests/spm_cell.rs` |

Three chemistries ship under [`chemistries/`](chemistries) — LFP 26650, NMC 18650, and
LG M50 21700. Every constant in them carries a provenance note, **including the ones
whose note says they are order-of-magnitude placeholders awaiting a fit**; that is the
project rule (placeholders are acceptable, unlabeled numbers are not) rather than a claim
that every number is fitted.

The LG M50 file is the first with an `[spm]` section, and the first whose two halves have
genuinely different provenance: its porous-electrode parameters are **extracted** verbatim
from PyBaMM's Chen2020 set (so each has a literal citation), while its equivalent-circuit
resistances remain labelled placeholders. Reading a voltage out of one half is reading
Chen2020; out of the other, a placeholder — and running the *same* cell through both
models is the comparison Phase 6 exists for. The NMC 18650 set is hand-fit to datasheet
curves and has no PyBaMM source; see its provenance line for why it cannot honestly
acquire one.

## Two cell models

`PackConfig::cell_model` selects between them, and both are the same public API — a pack,
a `step(dt, demand, env)`, one `Telemetry`.

- **`Ecm`** — a 1- or 2-RC Thévenin equivalent circuit, exact-exponential RC update,
  coulomb-counted SOC. Cheap (≈0.05 µs per cell per step), stable at any `dt`, and the
  model every chemistry here can run.
- **`Spm`** — one spherical particle per electrode, radial solid diffusion by backward
  Euler on a finite-volume grid, Butler–Volmer kinetics, and SOC read off the lithium
  inventory rather than counted. It reproduces what an equivalent circuit structurally
  cannot: the surface concentration running ahead of the bulk, and with it the
  end-of-discharge collapse and the relaxation after a load step. It costs ≈1.3 µs per
  cell per step at the recommended 20 shells — **≈26× the ECM**, measured on the same
  chemistry at the same topology. `docs/plans/phase-6-porous-electrodes.md` has the
  numbers and the accuracy-vs-shell-count curve behind that 20.

Against a grid- and time-converged PyBaMM SPM on the same parameter set, batsim's SPM
tracks terminal voltage to **2–7 mV over a whole discharge**, cut-off knee included —
no SOC window, unlike the Phase 1 ECM-vs-DFN goldens, which need one because the models
genuinely differ.

**`[spm]` is NMC-only on purpose.** LFP keeps its ECM path and its existing goldens and
gets no `[spm]` section: lithium iron phosphate intercalates through a moving phase
boundary, which is what produces the flat plateau this repo teaches with, and a
single-particle model with Fickian diffusion is the wrong physics for it. It could be
made to *fit*; shipping that as "porous-electrode physics for LFP" is the kind of
unlabelled claim the provenance rule exists to prevent.

**`diffsol` was evaluated for the stiff solve and declined**, because its solver state
cannot be extracted and restored bit-identically through its public API — which fails
the "everything is snapshotable" principle outright — and because a fixed-step method
whose entire state *is* the concentration vector passes that test trivially. The
measurement, and what it would take to revisit for a future `Dfn`, are in
[`docs/plans/phase-6-porous-electrodes.md`](docs/plans/phase-6-porous-electrodes.md).

Design notes for each phase, including what was measured and what was deliberately
*not* built, are under [`docs/plans/`](docs/plans).

## Workspace layout

```
batsim/
├── Cargo.toml            # workspace
├── CLAUDE.md             # full design contract
├── crates/
│   ├── sim-core/         # pure engine: types, models, solver, snapshots
│   ├── sim-data/         # TOML chemistry + scenario loading and validation
│   ├── sim-server/       # axum: REST for setup/snapshots, WebSocket for stepping
│   ├── sim-wasm/         # wasm-bindgen build of the engine, for the browser
│   └── sim-godot/        # gdext: the BatteryPack node, for Godot 4.7+
├── chemistries/          # *.toml parameter sets (LFP, NMC)
├── scenarios/            # *.toml scenarios: a pack, a chemistry, and its faults
├── web/                  # the browser demo: one HTML file, one JS file, no deps
├── godot/                # the Godot demo project: demo scene, smoke check, exit gate
├── examples/             # experiment.mjs — drive the server from outside
├── docs/plans/           # per-phase design notes and measurements
├── tools/reference/      # Python + PyBaMM scripts that generate golden CSVs
└── tests/golden/         # committed reference CSVs + tolerance tests
```

`sim-godot` and `sim-py` are added in their respective phases. `sim-core` depends on
nothing in this workspace and on no runtime — no tokio, no godot, no pyo3.

## Build and test

```bash
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

`cargo test -p sim-core` is the fast inner loop for physics work and is the command
to reach for while iterating: `axum` and `tokio` make the full workspace run
noticeably slower, and they touch nothing the engine does.

Two things are deliberately **outside** those commands, because each needs a toolchain
the Rust test run has no business requiring. Both are still compiled by the ordinary
gate, so neither can rot:

- `wasm-pack build` for the browser demo (see [The browser demo](#the-browser-demo)).
- Phase 5's exit gate, which needs a Godot 4.7 binary (see [The Godot demo](#the-godot-demo)).

## Run it

### The server

```bash
cargo run -p sim-server
```

Serves on `127.0.0.1:8080` with every path defaulting to this repo's layout, so no
arguments are needed from the workspace root. `--bind`, `--chem-dir`, `--web-dir` and
`--scenario-dir` override them; `--help` lists them.

```bash
curl -s http://127.0.0.1:8080/                                        # versions and limits
curl -s -X POST http://127.0.0.1:8080/sessions \
     --data-binary @scenarios/cc_discharge_lfp.toml                   # create a session
```

Stepping is deliberately **not** on the REST surface — it lives on the WebSocket at
`GET /sessions/{id}/ws`, because that is where `dt` can be explicit in every command,
ten thousand steps can be one message, and a session can have exactly one writer.
None of those survive a stateless request/response cycle.

### The browser demo

The page runs the engine itself, compiled to wasm. The bundle is a build artifact and
is not committed, so build it once:

```bash
wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg
```

Then start the server and open <http://127.0.0.1:8080/app/>. Pick a scenario, press
Run, and watch pack voltage, cell-voltage spread, current, SOC (true *and* as the BMS
estimates it), and temperature. There is a BMS on/off toggle — flipping it restarts
the run from t = 0, because the honest way to compare a protected pack with an
unprotected one is two runs, not one run with the rules changed halfway.

If `/app/` 404s, the bundle is missing. The `wasm-pack` line above is a copy: the
authoritative one is in `crates/sim-server/src/main.rs`, which also prints it as a
startup warning and serves it in the body of `GET /` — so the person holding a
browser gets told what to run without reading this file.

### The example experiment

[`examples/experiment.mjs`](examples/experiment.mjs) is an external script that runs a
full experiment over the WebSocket. It needs **no dependencies** — Node 22+ ships
`fetch` and `WebSocket` as globals:

```bash
cargo run -p sim-server          # in one terminal
node examples/experiment.mjs     # in another
```

It discharges a 4S2P LFP pack, snapshots it to a file mid-run and restores it,
survives a soft internal short arriving at t = 600 s behind a voltage sensor that has
been offset just enough to hide it, moves the pack into a cold room, and writes
`experiment.csv`. The BMS never raises a flag the whole way, which is the point.

The script is also the readable description of the protocol: about sixty lines of it
are the experiment and the rest is commentary on why the wire looks the way it does.

### The Godot demo

Needs **Godot 4.7 or newer** on `PATH`. The `godot` crate pins `api-4-7`, and an
extension cannot load into an engine older than the API it was built against.

```bash
cargo build -p sim-godot
godot --headless --path godot --import    # one-time, per clone: .godot/ is gitignored
godot --path godot                        # watch it run
```

There are two ways to configure the node, and it says which one it used
(`uses_topology()`):

- **Topology properties** — set `series`, `parallel`, `initial_soc`, `initial_temp_k` and
  `seed` in the inspector, paste a chemistry into `chemistry_toml`, and leave
  `scenario_toml` empty. The node synthesizes a scenario from them. Omitted means *off*:
  no scatter, no thermal coupling, no BMS, no aging, no faults.
- **A scenario** — put TOML in `scenario_toml` and it **wins**, with the topology
  properties ignored. A scenario says strictly more than they can (faults, a BMS, thermal
  coupling, aging, scatter), so the two do not merge; `effective_scenario_toml()` returns
  whichever one is actually in force, and a synthesized one is a fine starting point for an
  authored one.

Either way the node takes **text, not paths** — `res://` resolves inside a `.pck` once a
game is exported, so a path-taking node would work in the editor and fail in a shipped
build.

The demo discharges a single LFP cell at 1 C and shows live telemetry beside the signals
the node emits. It runs at `speed = 180`, so an hour of battery life takes about twenty
seconds — and `speed` is the knob that does that, **not** `fixed_dt`. Raising `fixed_dt`
makes each step cover more simulated time *and* makes the accumulator take proportionally
fewer of them, so the pack still advances one simulated second per wall second; only the
granularity changes. Every step is exactly `fixed_dt` either way, which is what keeps a
fast-forward bit-identical to a real-time run of the same step count.

Two checks live in the same project, and only the first needs Godot:

```bash
cargo test -p sim-godot                    # the pure driver: accumulator, edges, stepping
cargo test -p sim-godot -- --ignored       # the exit gate, in a real Godot process
godot --headless --path godot --script smoke.gd -- "$PWD"
```

The **exit gate** runs one scenario twice — once through the node inside a running Godot
process, once through `Pack::step` in this process — and asserts the two telemetry streams
are bit-identical, comparing `f64::to_bits` rather than `==`. It is `#[ignore]`d because it
needs a Godot binary, not because it is optional; it is still compiled by the ordinary
`cargo test --workspace`, so it cannot rot.

Numbers cross that boundary as the little-endian bytes of the `f64`, hex-encoded, because
GDScript cannot print a float without losing bits: `str(0.7995885912375074)` gives
`0.79958859123751`, which does not parse back equal.

The **smoke check** answers a narrower question the gate structurally cannot: *is the
accumulator wired up?* The gate drives the explicit `step_batch` path, since that is the
only path whose step count is reproducible enough to assert bit-identity on — so
`_physics_process` needs its own check, and that is what `smoke.gd` is.

## Two clients, not two layers

The browser page and the server are **peers**, not a stack. The page does not send
physics to the server and get results back: it embeds `sim-core` via `sim-wasm` and
steps the pack *in the browser tab*, with no round trip per frame. The server exists
for scripts, for headless experiments, and for putting one live pack on several
screens.

What the page does need the server for is files. A wasm module cannot be loaded from
`file://`, and a browser tab has no filesystem — so the scenario and chemistry TOML
the page hands to `sim-wasm` arrive over HTTP from `/scenarios` and `/chemistries`.
That is the whole dependency. (The page also has a socket toggle that switches it to
driving a real server session instead, which keeps the wire protocol honest by giving
it a live client.)

## What the socket costs

Measured with `cargo run --release -p sim-server --example transport_latency`, which
runs the same batch three ways — in process, over the socket reporting every step,
and over the socket reporting only the final step:

| pack | batch | in process | ws, every step | ws, final step only |
| ---- | ----- | ---------- | -------------- | ------------------- |
| 1S1P | 10 000 steps | ~0.84 ms | ~75 ms (≈90×) | ~1.0 ms (≈1.2×) |
| 100S10P | 10 000 steps | ~275 ms | ~390 ms (≈1.4×) | ~281 ms (≈1.02×) |

Two separate costs, and only one of them amortises:

- **A round trip is ~35–110 µs.** That is the entire cost of a one-step batch and it
  is invisible by a thousand steps. This is why a command carries `(dt, n_steps)`
  rather than one message per step.
- **A reported telemetry frame is ~7–11 µs** to encode, send and decode. That cost is
  per *frame*, so reporting every step never amortises — it stays ~90× the engine at
  1S1P however large the batch gets.

So `report_every_n_steps` is the knob that matters, and it should be sized to the
resolution a client will actually plot rather than to the step count. Decimation drops
reports, never steps: the trajectory is bit-identical either way. Ratios are the
portable part of that table; the absolute microseconds are one laptop's.

## Determinism, and how it is proven

`crates/sim-server/tests/e2e_experiment.rs` runs one scenario twice — once by calling
`Pack::step` in process, once by driving a real server over a real socket — and
compares the two telemetry streams with `f64::to_bits`, including a
snapshot → REST → restore → resume leg in the middle of the timeline. Bits rather than
`==`, because `-0.0 == 0.0` and `NaN != NaN`, so `==` can both hide a real difference
and invent one.

Same-binary determinism is the promise. Cross-platform bit-exactness is *not*
promised — `libm` differs between platforms — and nothing here claims it.

## License

Licensed under the **Boyko Non-Commercial License v1.0 (BNCL-1.0)** — see
[`LICENSE`](LICENSE) and [`NOTICE`](NOTICE). Non-commercial use only; commercial
use requires a separate license from the copyright holder.
