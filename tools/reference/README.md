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
     raises instead of leaving a stale TOML. It evaluates the **whole** right-hand side
     (numbers and `+ - * / **` only, anything else raises): Prada2013's LFP function
     writes `m_ref = 6 * 10 ** (-7)`, and the first pattern, which read the leading
     number, would have emitted **6** without a word;
   - `t_ref_k` is the temperature the exchange-current functions' Arrhenius factors are
     written against (parsed from the same bodies, and required to agree), **not** the
     set's `Reference temperature [K]` key: the engine reads `t_ref_k` only in that factor,
     and Prada2013's key says 298 where both functions say 298.15;
   - a chemistry listed in `REGULAR_SOLUTION` also gets
     `[spm.positive.regular_solution]`, the two-phase potential only the many-particle cell
     reads: `u0_v` cited, `omega_ev` **fitted** by the script (bisection, to the last bit)
     so the curve's turning points sit the cited rest gap apart. Today that is
     `lfp_26650_prada2013` (Phase 9); its header says so instead of "nothing here is
     fitted";
   - the OCP grid is refined adaptively to a stated tolerance (2 mV by default, passable
     as a second argument) rather than laid down uniformly. Graphite's OCP rises steeply
     near zero stoichiometry, where 81 uniform points left 43.9 mV of error; 45 adaptive
     ones give 1.9 mV.

   Since Phase 7 the tables span the **full** stoichiometry range [0, 1] rather than the
   usable window plus a 0.05 margin, and the shape of the script matters here: the
   adaptively-refined **core is still generated over the old margin window alone**, and
   the two tails are tabulated separately and concatenated. That is not a stylistic
   choice. The refinement is greedy against a dense reference on whatever interval it is
   given, so regenerating over [0, 1] in one pass would move every breakpoint and shift
   the interpolation *inside* the window the shipped SPM goldens are pinned to. The
   extension has to be append-only by construction, and it is.

3. **`extract_dfn.py`** — prints a TOML-ready `[dfn]` block: the electrolyte transport
   fits, transference number, thermodynamic factor, porosities, Bruggeman exponents,
   separator thickness and solid conductivities. Paste it into the matching
   `chemistries/*.toml`, after `[spm]` — `[dfn]` **extends** that section rather than
   replacing it, and a DFN cell reads both.

   ```bash
   python tools/reference/extract_dfn.py nmc_21700_lgm50
   ```

   The two transport properties are the whole difficulty and the answer is a good one.
   PyBaMM publishes `D_e` and `κ_e` as *callables*, but underneath they are published
   closed-form fits (Nyman 2008) with no temperature dependence, so they are stored
   **exactly** as `(coefficient, exponent)` pairs in the fit's own variable `x = c_e/1000`
   and carry no interpolation error at all. Sampling them onto a grid would have been the
   obvious move and would have been actively harmful: Phase 6 found the OCP tables' own
   1.88/1.90 mV interpolation error *was* the SPM's accuracy floor.

   Unlike `extract_spm.py`'s regex, these coefficients live in *expressions* rather than
   `name = value` assignments, so the script parses the function source with `ast` and
   then **evaluates the parsed terms against PyBaMM's own callable at seven
   concentrations**, refusing to emit anything if they disagree. That cross-check is what
   makes a silent mis-parse loud.

4. **`generate.py`** — runs isothermal (25 °C) scenarios and writes one CSV per
   scenario under `tests/golden/<chem_id>/`. Which reference model runs is a
   per-scenario choice, because batsim has two cell models:

   *`lfp_26650_generic`* — DFN references for batsim's **ECM**:
   - `cc_c20_25c.csv` — C/20 constant-current discharge (low-rate, tight);
   - `cc_1c_25c.csv` — 1C constant-current discharge (rate effects, looser);
   - `pulse_relax_25c.csv` — GITT-like C/2 pulses with rests.

   *`lfp_26650_prada2013`* — the reference for batsim's many-particle LFP cell:
   - `spm_cc_c20_25c.csv` — C/20 from full, PyBaMM's SPM on Prada2013's own (monotone)
     potential. batsim's cell runs a regular-solution potential instead, so
     `lfp_ensemble.rs` asserts a band on the **offset** between them, not a match.

   *`nmc_21700_lgm50`* — SPM references for batsim's **SPM**, plus one DFN:
   - `spm_cc_c5_25c.csv`, `spm_cc_1c_25c.csv`, `spm_pulse_relax_25c.csv`;
   - `dfn_cc_1c_25c.csv` — the *same* scenario as `spm_cc_1c_25c.csv` run as a DFN.
     Nothing asserts batsim against it; it is committed so the SPM-vs-DFN gap
     (≈71 mV at 1C) is visible as the physics result it is.

   ```bash
   python tools/reference/generate.py               # all chemistries
   python tools/reference/generate.py lfp_26650_generic
   ```

5. **`sei_golden.py`** — the SEI film's reference (Phase 10): PyBaMM's SPM with the
   interstitial-diffusion limited SEI law, the `[spm.sei]` constants of
   `chemistries/nmc_21700_lgm50.toml`, a year at rest at five SOC/temperature conditions,
   each started at the engine's own stoichiometry and each run twice — on Chen2020's graphite
   curve and on the engine's piecewise-linear table — so the table's own cost is measured in
   the CSV. Writes `tests/golden/nmc_21700_lgm50/sei_rest_year.csv`, which
   `crates/sim-data/tests/sei_golden.rs` reads.

   ```bash
   python tools/reference/sei_golden.py
   ```

6. **`fit_sei_wmg.py`** — fits `[spm.sei]` to the WMG LG M50 calendar-ageing data (Phase 10,
   slice A; how to fetch the data is in its docstring). With `--table` it skips the fit and
   writes `tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv`: the cells' storage-driven fade
   growth after the first post-storage test, data only (CC-BY-4.0, Kuzhiyil et al. 2025), which
   `crates/sim-data/tests/sei_wmg.rs` holds the engine to (exit 5).

   ```bash
   python tools/reference/fit_sei_wmg.py wmg --table tests/golden/nmc_21700_lgm50/wmg_storage_growth.csv
   ```

7. **`sei_dfn_golden.py`** — the SEI film under cycling (Phase 10, slice D): PyBaMM's DFN with
   the interstitial law on the engine's grid (10/5/10, 20 shells) and both of the engine's OCP
   tables, from one even 50 nm film, eight ±0.25-capacity cycles from 50 % at 1C and 2C, each
   run with the film frozen (the floor) and growing. Writes
   `tests/golden/nmc_21700_lgm50/sei_dfn_cycling.csv`, which `crates/sim-data/tests/sei_dfn.rs`
   reads. Solved at 1e-10.

   ```bash
   python tools/reference/sei_dfn_golden.py
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

5. **`fit_ecm.py`** (Phase 11) — fits the LG M50's equivalent circuit, `[r0]` and
   `[[rc]]`, to PyBaMM's DFN, in two stages:

   ```bash
   python tools/reference/fit_ecm.py --generate   # the DFN pulse trains -> ecm_hppc_*.csv
   python tools/reference/fit_ecm.py --fit        # the fit, from the committed CSVs
   ```

   The reference is an HPPC-style train at 1C (25, −10 and +45 °C) and 3C (25 °C), solved
   converged. Away from 25 °C it is Chen2020 with ORegan2022's solid-diffusion and
   electrolyte temperature terms added — the cell the engine's three models describe, and
   exactly Chen2020 at 25 °C. The circuit is simulated the engine's way (start-of-step
   `R0`, the `[r0] rc_t_ref_k` rule per sample, exact RC update) against the reference's
   **own** OCV, residuals weighted by time, so the `[ocv]` table's error stays out of the
   resistances; it prints the TOML block and every residual. The fit reads only the
   committed CSVs, so `--fit` reproduces the shipped block without re-running the DFN;
   numpy and scipy are pinned for that reason. `crates/sim-data/tests/ecm_fit.rs` replays
   the same CSVs through the engine.

   The same phase gave `fit_ocv.py` an adaptive grid for this chemistry only
   (`common.OCV_ADAPTIVE_TOL_V`, 1 mV): the hand-tuned grid below suits LFP's knees and
   left 13.91 mV on the NMC curve. The LFP files keep their grid byte for byte.

8. **`plating_reference.py`** (Phase 12, slice A) — the plating law the owner chose (plating
   only past 0 V vs lithium, stripping only what is plated, a fixed dead share), closed loop:
   PyBaMM's DFN with its `"reversible"` plating submodel replaced, on the cell the engine runs.
   PyBaMM ships no such law, so this is the reference for it. One charge per call; prints a
   summary line and, with `--out`, the per-second trace.

   ```bash
   python tools/reference/plating_reference.py 0 0.3 4.8e-11 --out trace.csv
   ```

   **Run it one charge at a time**: four charges run three at a time came back with an empty
   hold leg and ran cleanly alone (not traced). Writes nothing in the tree; results in
   `docs/plans/phase-12-slice-a-reference.md`.

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
