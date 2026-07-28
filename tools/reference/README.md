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

The venv location is yours to choose and **nothing in the repo depends on it** — the
scripts import only PyBaMM and read the repo by relative path, so an environment kept
entirely outside the tree works and keeps `git status` clean (`.venv` is not
gitignored). The pin is load-bearing rather than decorative: regenerating the LFP
goldens with `pybamm==26.6.2.0` reproduces the committed CSVs **byte-for-byte**, which
is how a refactor of this pipeline is shown not to have moved a Phase 1 reference.

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

2. **`extract_spm.py`** — prints a TOML-ready `[spm]` block: half-cell OCP tables,
   particle geometry, transport, kinetics and stoichiometry limits. Paste it into the
   matching `chemistries/*.toml`.

   ```bash
   python tools/reference/extract_spm.py nmc_21700_lgm50
   ```

   A different verb from `fit_ocv.py` on purpose. The ECM tables are **fitted** to a
   reference model's output; SPM parameters are **extracted** from the parameter set
   directly, so every number has a literal citation rather than a tolerance. Two
   wrinkles worth knowing:

   - the kinetic rate coefficient `m_ref` and its activation energy `E_r` are *not*
     parameter-set keys — they are literals inside the exchange-current-density
     function bodies, and the script parses them out of the source so a set upgrade
     raises instead of leaving a stale TOML;
   - the OCP grid is refined adaptively to a stated tolerance (2 mV by default, passable
     as a second argument) rather than laid down uniformly. Graphite's OCP rises steeply
     near zero stoichiometry, where 81 uniform points left 43.9 mV of error; 45 adaptive
     ones give 1.9 mV.

3. **`generate.py`** — runs isothermal (25 °C) scenarios and writes one CSV per
   scenario under `tests/golden/<chem_id>/`. Which reference model runs is a
   per-scenario choice, because batsim has two cell models:

   *`lfp_26650_generic`* — DFN references for batsim's **ECM**:
   - `cc_c20_25c.csv` — C/20 constant-current discharge (low-rate, tight);
   - `cc_1c_25c.csv` — 1C constant-current discharge (rate effects, looser);
   - `pulse_relax_25c.csv` — GITT-like C/2 pulses with rests.

   *`nmc_21700_lgm50`* — SPM references for batsim's **SPM**, plus one DFN:
   - `spm_cc_c5_25c.csv`, `spm_cc_1c_25c.csv`, `spm_pulse_relax_25c.csv`;
   - `dfn_cc_1c_25c.csv` — the *same* scenario as `spm_cc_1c_25c.csv` run as a DFN.
     Nothing asserts batsim against it; it is committed so the SPM-vs-DFN gap
     (≈71 mV at 1C) is visible as the physics result it is.

   ```bash
   python tools/reference/generate.py               # all chemistries
   python tools/reference/generate.py lfp_26650_generic
   ```

   Prefer the **target-scoped** form. A bare run regenerates every chemistry,
   including the committed LFP goldens that four phases of ECM tests are toleranced
   against, and a golden that drifts under you is a bad afternoon. After any run,
   `git status tests/golden/` should show only what you meant to move.

   ### Two things about the reference that are not obvious and cost 346 mV to find

   `common.Reference` carries a model kind, a particle-grid size, solver tolerances
   and a `dense_output` flag. The Phase 1 goldens use `DFN_DEFAULT`, which is
   PyBaMM's own defaults for all four and is therefore bit-for-bit the Phase 1
   pipeline; the Phase 6 goldens use converged settings. The two reasons:

   - **Solver tolerance, not grid, dominates near the cut-off.** Between PyBaMM's
     default `1e-6` and `1e-10` the terminal voltage moves by 18–75 mV at the knee,
     and *non-monotonically in the particle grid* — a finer grid at loose tolerance
     was sometimes further from the converged answer than a coarser one. A golden
     carrying that would tolerance batsim against solver noise.
   - **`t_eval=[t0, t1]` returns the solver's own steps, not a dense trajectory.**
     Resampling those onto a uniform grid draws a straight *chord* across each step,
     and an adaptive integrator's steps get very long where the solution is smooth.
     The first SPM golden fell **linearly** from soc 0.107 to the cut-off for exactly
     this reason — a 346 mV artifact that looked like a converged disagreement,
     because it did not move with anything on batsim's side. `t_interp` (and, for the
     experiment-driven pulse scenario, a per-step output `period`) fixes it and
     changes the integration not at all.

`common.py` holds the shared extraction/simulation helpers and the
`batsim chemistry id → PyBaMM parameter set` map.

The map means "the set this chemistry is *fitted against*", so a chemistry with no
PyBaMM source is deliberately absent from it rather than pointed at the nearest cell.
`nmc_18650_generic` is the case in point: it was mapped to Chen2020, which is a 21700 /
5 A.h LG M50, and PyBaMM ships no 18650-class NMC set to repoint it at (Chen2020,
OKane2022 and ORegan2022 are all LG M50; Mohtat2020 and Xu2019 are NMC532 pouch cells).
Running `fit_ocv.py` against it would have changed the cell's identity rather than fitted
it, so the entry is gone and the file's provenance says so. See
`chemistries/nmc_21700_lgm50.toml` for the honestly-labelled 21700.

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

[`spm_golden.rs`](../../crates/sim-data/tests/spm_golden.rs) does the same for the
SPM references, and needs no SOC window at all: both sides are the same model form on
the same parameter set, so the comparison holds to 2–7 mV across the whole
trajectory. Its companion
[`spm_exact_bits.rs`](../../crates/sim-data/tests/spm_exact_bits.rs) pins the shipped
`[spm]` numbers and the pure-arithmetic derived geometry to exact bits, because a
tolerance cannot see a constant change by one ULP — and one did, during Phase 6.
