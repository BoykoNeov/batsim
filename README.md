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

Phases 0–7 are complete. Each phase's exit criterion is a committed test rather than a
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
| 7 | the `Dfn` cell model — electrolyte transport solved rather than assumed, an analytic banded Jacobian, and a pack tangent taken as a sensitivity solve | `sim-data/tests/dfn_golden.rs`, `dfn_cell.rs`, `dfn_chemistry.rs` |

Work continues past the phase plan. The most recent is a cell-model fix Phase 3 found
and deliberately deferred: charge pushed into a cell already at 100 % SOC used to vanish —
not stored, and generating no heat beyond `I²R0`. It is now refused and dissipated at the
top of the OCV curve, reported through `Telemetry::i_rejected_a`, and it turns out to
dominate rather than correct: on a 1C overcharge it is **41× everything the engine
previously reported**. See [`docs/plans/energy-hole.md`](docs/plans/energy-hole.md).

The over-*discharge* mirror stayed open through two attempts and is now closed. An empty
cell used to keep sourcing at `OCV(0)` forever, which is energy from nowhere; the fix is
not to refuse the current — nothing in the engine can refuse a demanded current — but to
let the cell go into **voltage reversal**, carrying what it delivered as a deficit and
dropping its open-circuit voltage through zero so the external circuit pays. A cell driven
past empty and charged back now returns to exactly where it started with the books
balanced, where before it fabricated kilojoules and did not even end up in the same state.
See [`docs/plans/low-clamp-reversal.md`](docs/plans/low-clamp-reversal.md).

Three chemistries ship under [`chemistries/`](chemistries) — LFP 26650, NMC 18650, and
LG M50 21700. Every constant in them carries a provenance note, **including the ones
whose note says they are order-of-magnitude placeholders awaiting a fit**; that is the
project rule (placeholders are acceptable, unlabeled numbers are not) rather than a claim
that every number is fitted.

The LG M50 file is the first with an `[spm]` section, and the first whose parts have
genuinely different provenance: its porous-electrode parameters are **extracted** verbatim
from PyBaMM's Chen2020 set (so each has a literal citation), while its equivalent-circuit
resistances remain labelled placeholders. Reading a voltage out of one half is reading
Chen2020; out of the other, a placeholder — and running the *same* cell through both
models is the comparison Phase 6 exists for. The NMC 18650 set is hand-fit to datasheet
curves and has no PyBaMM source; see its provenance line for why it cannot honestly
acquire one.

It also carries a `[dfn]` section — the electrolyte transport fits, porosities and solid
conductivities a Doyle–Fuller–Newman cell needs, extending `[spm]` rather than replacing
it. Its two transport properties are stored as the published Nyman 2008 coefficients
rather than sampled onto a table, so they carry no interpolation error at all.

## Three cell models

`PackConfig::cell_model` selects between them, and all three are the same public API — a
pack, a `step(dt, demand, env)`, one `Telemetry`.

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
- **`Dfn`** — the full Doyle–Fuller–Newman cell: one particle per finite volume across the
  electrodes, and the electrolyte concentration and potential **solved for** rather than
  held constant. That last clause is the whole difference. An `Spm` assumes the electrolyte
  never runs out, which is true until it isn't; a `Dfn` reproduces the reaction front that
  develops across a thick electrode at high rate, and the concentration collapse that ends
  a hard discharge long before the cell is empty. At 3C on the LG M50 set that is worth
  **2.1 A·h against the SPM's 4.5** — the same cell, the same parameters, a different
  answer, and the reference agrees with the `Dfn`.

  It costs **≈180 µs per cell per step**, about **141× the `Spm`**, with a stiff Newton
  solve behind an analytic banded Jacobian. A 1S1P `Dfn` is a study and a 10S10P one is a
  ~18 ms fast-forward; this is not a real-time model above a few cells, and it is quoted
  per *cell* because the pack solve's pass count depends on topology.

  `scenarios/cc_discharge_3c_dfn.toml` selects it, and its twin
  `cc_discharge_3c_spm.toml` differs in exactly one block — so the difference above is
  something a client can run rather than only something the goldens assert. That pair is
  also where the *boundary* is written down: at 1 C the two models' cut-offs land 12 s
  apart in 3484 (0.34 %), and at 3 C they are 128 % apart. Below the boundary the cheap
  model is right; above it, it is wrong and says nothing.

Against grid- and time-converged PyBaMM references on the same parameter set, batsim's SPM
tracks terminal voltage to **2–7 mV over a whole discharge** and its DFN to **5.8 mV at
1C**, cut-off knee included — no SOC window, unlike the Phase 1 ECM-vs-DFN goldens, which
need one because the models genuinely differ. In the 3C depletion scenario the DFN tracks
the reference to 62 mV where the SPM is off by **521 mV**, which is the clearest single
statement of what the electrolyte equations buy.

**`[spm]` and `[dfn]` are NMC-only on purpose.** LFP keeps its ECM path and its existing
goldens and gets neither section: lithium iron phosphate intercalates through a moving phase
boundary, which is what produces the flat plateau this repo teaches with, and a
single-particle model with Fickian diffusion is the wrong physics for it. It could be
made to *fit*; shipping that as "porous-electrode physics for LFP" is the kind of
unlabelled claim the provenance rule exists to prevent.

**`diffsol` was evaluated for the stiff solve and declined**, because its solver state
cannot be extracted and restored bit-identically through its public API — which fails
the "everything is snapshotable" principle outright — and because a fixed-step method
whose entire state *is* the concentration vector passes that test trivially. Phase 7
re-checked it at 0.16.1 for the `Dfn` — the harder solve the evaluation was really about —
and declined it again on the same grounds. The measurements are in
[`docs/plans/phase-6-porous-electrodes.md`](docs/plans/phase-6-porous-electrodes.md) and
[`docs/plans/phase-7-dfn.md`](docs/plans/phase-7-dfn.md).

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
curl -s http://127.0.0.1:8080/scenarios                               # what scenarios exist
curl -s -X POST http://127.0.0.1:8080/sessions \
     --data-binary @scenarios/cc_discharge_lfp.toml                   # create a session
```

`GET /scenarios` is the catalogue — topology, chemistry, and what each file switches
on — while `GET /scenarios/<file>.toml` serves one file verbatim. A scenario that does
not parse is listed carrying its error rather than quietly missing.

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

If you do not already know what to look at, press **Start** under *Guided path*
instead. It walks eighteen steps — one cell on its own, the same discharge on a
different chemistry, a pack disagreeing with itself, the BMS's estimate drifting from
the truth, a short hidden by the sensor that should have caught it, the same overload
with protection on and then off, a pack that wears out while doing nothing at all,
three that put charge back in (the two legs of a CC-CV charge, a chemistry whose
second leg never arrives, and what a BMS costs a charge it decides to stop), three
that pulse the same cell through two different cell models — a circuit that answers an
identical pulse identically, a particle that remembers the last one, and three times
the current buying ×1.87 of one part of the answer and ×6.01 of another — a pair that
pulls that same cell flat at 3 C through the two *porous-electrode* models and differs
in one block of one file, where the single-particle model runs smoothly to 4.55 A·h
with no flag raised and the Doyle–Fuller–Newman one is finished at 1.99 with 61 % still
on the readout, because the electrolyte an `Spm` holds constant has starved — and two
shorts across the terminals that separate the two rungs of the protection ladder: a
dead one the contactor catches in a single step for half a percent, and a weaker one
that costs fifty because a derate clamps demands and a short is not a demand. Each
step sets the controls for itself and outlines the panel it is about. Every control
stays live throughout; stepping back and forward re-applies a step's whole control set,
so there is nothing you can break by fiddling mid-lesson. A step reloads the pack when
it needs to start from t = 0 and otherwise keeps the run going, which is why steps 12
to 16 — whose claims are about a *first* pulse or a discharge from full — ask for the
reload explicitly rather than inheriting whatever the neighbouring step left behind.
Those five also pin the timestep, for the same class of reason: steps 15 and 16 run at
the 2 s step their golden asserts at, and without a pin that setting would leak back
into the pulse steps on the way and quietly move every millivolt they quote. Steps 3 to
5 are one continuous run on one pack, because changing what you look at teaches more
than reloading.

The scenario picker is filled from the server's own `GET /scenarios`, so adding a file
under `scenarios/` puts it in the list with no edit to the page.

Three panels show what an aggregate cannot:

- **The pack grid** — one tile per cell, series down and parallel across, coloured by
  a metric you pick (SOC, temperature, overpotential, SOH, the scatter factors, or
  internal-short conductance). This is `Pack::cell()` ground truth, which the BMS is
  never allowed to read. Hover a tile for that cell's full state; click to pin it. The
  colour scale spans the *pack's own* min-max, so a spread of a few millivolts is
  visible rather than flattened against a fixed axis — which is the point, since the
  group solve splits current by state and the disagreement between cells is the
  physics. Load the `soft_short_under_a_lying_sensor` scenario and run it: one
  parallel group falls visibly behind the others and runs hotter, while the pack
  aggregate and the BMS estimate both look unremarkable.
- **Fault injection** — queue any of the five `Fault` variants against the running
  pack: a soft internal short, an external short, a weak cell, or a stuck or offset
  sensor. A fault fires on the next *step*, so nothing happens while the run is
  paused. You can also drop the pending queue and clear a latched BMS trip. Injecting
  is the same operation a scenario file's `[[faults]]` table performs, so anything
  schedulable from a file is schedulable live.
- **The BMS view** — the same pack as the BMS measures it, channel by channel, beside
  what the engine knows. The order is the physics rather than the obvious layout: the
  **current** sensor is wrong on every step (a configured offset plus a noise draw),
  the **SOC estimate** coulomb-counts that sensor and so integrates its error, the
  **temperature probes** read exactly but only where they sit — two probes for four
  groups on the shipped scenario, and the grid rings the cells they are on, because
  that error is spatial — and only then **group voltage**, one dot per series group on
  a shared axis with the pack's true spread drawn behind it as a band. Group voltages
  are exact reads until something lies about one, which is the point of the channel:
  run `soft_short_under_a_lying_sensor` past t = 600 s and group 1's dot jumps clear
  of the band and is called out, while the band itself does not move. That scenario
  has shipped since Phase 3 and this is the first time its lie has been drawable.

All three panels work identically whether the engine runs in the tab or behind the
server (see [Two clients, not two layers](#two-clients-not-two-layers)) — though the
grid and the BMS view read their state over REST in the server mode, because per-cell
and per-group arrays are deliberately not telemetry and never ride a socket frame.

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
the page hands to `sim-wasm` arrive over HTTP from `/scenarios/` and `/chemistries`,
and the picker's contents from `/scenarios`.
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
