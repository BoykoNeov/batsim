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

## Workspace layout

```
batsim/
├── Cargo.toml            # workspace
├── CLAUDE.md             # full design contract
├── crates/
│   ├── sim-core/         # pure engine: types, models, solver, snapshots
│   └── sim-data/         # TOML chemistry loading + validation
├── chemistries/          # *.toml parameter sets (LFP, NMC first)
├── tools/reference/      # Python + PyBaMM scripts that generate golden CSVs
└── tests/golden/         # committed reference CSVs + tolerance tests
```

Adapter crates (`sim-server`, `sim-wasm`, `sim-godot`, `sim-py`) are added in their
respective phases. `sim-core` depends on nothing in this workspace and on no runtime.

## Build

```bash
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

## Status

Phase 0 scaffold — compiling workspace skeleton. Physics, chemistries, and tests
land per the phased build plan in [`CLAUDE.md`](CLAUDE.md).

## License

Licensed under the **Boyko Non-Commercial License v1.0 (BNCL-1.0)** — see
[`LICENSE`](LICENSE) and [`NOTICE`](NOTICE). Non-commercial use only; commercial
use requires a separate license from the copyright holder.
