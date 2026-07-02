# tools/reference — PyBaMM golden-reference pipeline

Python + [PyBaMM](https://pybamm.org) scripts that generate the committed golden
CSVs under [`tests/golden/`](../../tests/golden). **Not shipped and never on the
Rust build or CI path** (per `CLAUDE.md`, "Testing strategy"): the CSVs are
committed, so the Rust tests never need PyBaMM. You only run these when
(re)deriving a chemistry's OCV from a parameter set or regenerating the goldens.

## Setup

Requires Python 3.11–3.13 (PyBaMM has no 3.14 wheels yet). Using
[`uv`](https://docs.astral.sh/uv):

```bash
uv venv --python 3.11 .venv
uv pip install -r tools/reference/requirements.txt
```

## What it does

The engine's v1 cell is an equivalent-circuit model (ECM), while PyBaMM's DFN is
a physics-based porous-electrode model. A meaningful golden requires the two to
share an OCV source — otherwise the comparison is dominated by an OCV mismatch,
not the ECM-vs-DFN modelling gap. So the pipeline has two stages:

1. **`fit_ocv.py`** — extracts the thermodynamic cell OCV
   `U_p(y(soc)) − U_n(x(soc))` and the usable capacity between the stoichiometry
   limits from a PyBaMM parameter set, printing a TOML-ready `[ocv]` block +
   `capacity_ah`. Paste it into the matching `chemistries/*.toml`. This is what
   makes each chemistry's "Fitted to PyBaMM …" provenance literally true.

   ```bash
   python tools/reference/fit_ocv.py lfp_26650_generic
   ```

2. **`generate.py`** — runs isothermal (25 °C) DFN scenarios and writes one CSV
   per scenario under `tests/golden/<chem_id>/`:
   - `cc_c20_25c.csv` — C/20 constant-current discharge (low-rate, tight);
   - `cc_1c_25c.csv` — 1C constant-current discharge (rate effects, looser);
   - `pulse_relax_25c.csv` — GITT-like C/2 pulses with rests.

   ```bash
   python tools/reference/generate.py               # all chemistries
   python tools/reference/generate.py lfp_26650_generic
   ```

`common.py` holds the shared extraction/simulation helpers and the
`batsim chemistry id → PyBaMM parameter set` map.

## Conventions

- SI units; **positive current = discharge** (batsim's sign convention, which
  PyBaMM already matches).
- SOC is coulomb-counted against the usable stoichiometry-window capacity, and
  the chemistry's `capacity_ah` is set to the same value, so batsim's SOC tracks
  PyBaMM's under constant current — the golden then tests the electrical model,
  not a capacity mismatch.
- The DFN initial state is pinned to SOC = 1.0 (upper cut-off) to align with the
  pack batsim builds.

## How the Rust side consumes these

[`crates/sim-data/tests/pybamm_golden.rs`](../../crates/sim-data/tests/pybamm_golden.rs)
loads the fitted chemistry, replays each CSV's `current_a` profile through
`sim-core`, and asserts terminal voltage within a documented, per-scenario
tolerance — tight across the mid-SOC plateau (and at fully-relaxed rest points),
looser where the ECM cannot follow the DFN (the end-of-discharge concentration
knee and fast kinetic transients). See that file's header for the rationale.
