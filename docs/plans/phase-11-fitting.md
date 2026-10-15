# Phase 11 — the LG M50 file, fitted where anything can fit it

**Status: PLANNED 2026-10-15.** Written after the spike (`phase-11-spike.md`), which found the
roadmap's exit unreachable and the fit's lesson cost larger in kind than a re-measure. The
owner's decisions below supersede `docs/ROADMAP.md` §Phase 11's slices and exit.

## The owner's decisions, recorded

1. **The exit is narrowed** to what has a source. Every constant with a source in hand is
   replaced by a fit or a citation; the fifteen with none (cycle fade 2, reversal 3, runaway 5,
   plating 4, the film's starting thickness 1) and the drawn calendar curve (decision 4) stay
   labelled, **on a fixed list a test holds** — the list can shrink only by a later note.
   **LG's specification sheet is cited**, with its marking ("CONFIDENTIAL", "Tentative",
   revision 0 of 2017-09-22, a retailer's copy) in every provenance line that uses it.
2. **Cold comes from ORegan2022**, for the circuit and for the porous models. Asked again after
   the spike measured what each part does (follow-up, same day): **solid diffusion's activation
   energy as data, and the electrolyte's temperature dependence as an engine change in the
   `Dfn`.** Measured on PyBaMM's Chen2020 DFN, 10 s current from 50 % to 2.50 V at −10 °C:
   54.1 A as shipped, 51.0 with ORegan's solid-diffusion energies, 42.5 with its electrolyte
   alone, 36.0 with both and its kinetics (53.6 with its kinetics alone, so the kinetics stay
   Chen2020's own published value); 54.8 A at 24 °C with all three, so warm behaviour stands.
   The real cell's figure is 14 A and ORegan2022's own 22 A: Chen2020 is too strong warm too
   (55 against 37 A), and no temperature term fixes that. Recorded, not chased.
   **And the three models describe one cell** (asked a third time, same day, once the
   consequence was stated: a circuit on ORegan2022's own cold ratio would have disagreed with the
   `Dfn` by about 2× at −10 °C): the circuit's cold columns are fitted to **the same combined
   reference** slice C commits for the `Dfn` — Chen2020 with ORegan2022's solid-diffusion and
   electrolyte temperature terms — not to ORegan2022 itself. The simulated cold cell is milder
   than the real one (~36 A against 14 A for 10 s at −10 °C), stated as a known gap.
3. **Entropic heat in all three models**, from ORegan2022. The `Spm`'s and `Dfn`'s
   `docp_dt_v_per_k` becomes a per-stoichiometry table (an engine change). **Heat and voltage**
   (owner, asked separately): as PyBaMM does, the same coefficient shifts each open-circuit
   potential, `U(T) = U(T_ref) + (T − T_ref)·∂U/∂T`, in all three models — the circuit through
   the `[ocv] t_ref_k` switch it already has, the porous models through a new term at each
   particle's surface. So the three models and the reference agree about what temperature does
   to the voltage, and every run away from 25 °C moves.
4. **The drawn calendar curve stays the placeholder it is**, deliberately: steps 38–39 teach
   against it. It goes on decision 1's list with that reason.

6. **The circuit's RC pairs take `R0`'s temperature factor** (owner, asked after slice A's first
   fit). Fitted to the cold reference with pairs that cannot change with temperature, the
   circuit's 1C pulses at −10 °C were 128 mV RMS off (15 mV at 25 °C). With each pair's
   resistance multiplied by `R0(soc, T) / R0(soc, T_ref)` and its capacitance held — the shape
   aging already gives them — that falls to 67 mV (whole run 22.5 → 11.6 mV; free pairs per
   temperature, which the schema cannot hold, 11.2 mV), and at 45 °C 5.85 → 3.78 mV. No new
   number: an opt-in switch naming `T_ref`, on in the LG M50 file only, so every other chemistry
   is bit-identical.

One snapshot bump covers all three engine changes (`SNAPSHOT_VERSION` 27 → 28): the chemistry is part
of the snapshot, and both change its shape.

5. **The `[cell]` limits are the strictest across temperatures** (owner): `max_charge_c = 0.3`
   (the sheet's 0–25 °C row), `max_discharge_c = 0.5` (its −20–10 °C row), `t_max_k = 318.15`
   (its charge ceiling, the lower of its two). The engine has one number for each; the battery
   manager never permits what the sheet forbids at any temperature, and is stricter than the
   sheet everywhere else — said so in the file.

Not part of this phase: the roadmap's lead-acid `[diffusion]` refit (its slice C). It stays under
H3. The exit is about one file.

## What the spike established

* A circuit fitted to the DFN at 1C cuts the RMS error from 10.3 to 4.0 mV at 25 °C; two RC pairs
  shared across temperatures cost 0.2 mV against per-temperature pairs. At 3C it is 18 mV (the
  placeholders 30), and a 3C fit's own `R0` is about two thirds of the 1C one. **Design rate 1C.**
* Eleven free `R0` nodes zig-zag (20.5 mΩ at 90 %, 29.8 at 80 %), and the zig-zag is what turns
  step 12's two sag levels into a staircase. A node count or a smoothness penalty is chosen in
  slice A by measurement, with the residual stated per segment (pulse, move, rest).
* The fitted slow pair (333 s) does not relax in step 12's 600 s rest, and an `R0` that varies with
  state of charge makes step 14's instant drop ×3.42 instead of ×3. **Steps 12 and 14 are
  redesigned, step 13's sentence about "round numbers" rewritten, step 2 re-measured**, in the same
  slice as the fit, because nothing else in the workspace reads these values and the suite must
  be green at every commit.

## Slices

The order changed once the RC rule was added: every engine change lands first, as one bump with
nothing moving, so each later slice is data and lessons against a fixed engine.

### E — the engine, v27 → v28, nothing moves

Three opt-in additions, each `None` / absent on every shipped file, so the suite is bit-identical:

* **The RC temperature rule** (decision 6): `[r0] rc_t_ref_k`, when stated, multiplies each RC
  resistance by `R0(soc, T) / R0(soc, rc_t_ref_k)` (capacitance held). The pack-level RC decay a
  cell reuses today assumes one `τ` per pair across the pack; with the rule on, `τ` is per cell,
  so the decay is computed per cell on those packs only.
* **Entropic tables and the potential's temperature shift** (decision 3): `[spm.*]
  docp_dt_v_per_k` as a table over stoichiometry (a scalar still loads), read at each particle's
  surface for heat **and** `U(T) = U(T_ref) + (T − T_ref)·∂U/∂T` in the `Spm`, `Dfn` and
  `SpmEnsemble`; the energy ledgers checked first (the reversible heat closes against
  `U − T·∂U/∂T`).
* **The electrolyte's temperature dependence** (decision 2): measured, one activation energy per
  property is not enough — up to 22 % off for diffusivity and 35 % for conductivity across
  0.2–2 kmol/m³ and 263–318 K, because Landesfeind 2019's diffusivity has an activation energy
  linear in concentration (exactly) and its conductivity is not Arrhenius at all (a linear `T`
  prefactor and `exp(1000/T)` terms). So `[dfn]` carries **Landesfeind 2019's published forms as
  data** — the form's name and its 4 + 6 coefficients — applied as the ratio
  `LF(c, T) / LF(c, t_ref_k)` to the existing Nyman 2008 base: exactly the combined reference's
  construction, so engine and reference agree by construction rather than within a fit.

### A — the circuit (Python + data + lessons; no Rust)

* `tools/reference/fit_ecm.py`: the HPPC train on the **converged** Chen2020 DFN (200 radial
  points, `rtol = atol = 1e-10`, `common.DFN_CONVERGED`), the fit against the reference's own OCV
  in the engine's update, printing a TOML block whose provenance names the design rate and the
  residuals (1C per temperature and per segment, and 3C). Run twice → identical output.
* **Weighting and smoothness, as measured.** Residuals are weighted by time (each second of the
  reference counts once): per-row weighting made the fit depend on how densely the CSV is sampled
  (pairs 16/115 s per row, 36/325 s per second). `R0` sits at the ten pulse levels, 10–100 %
  (nothing is measured below; the grid holds its 10 % value there), with a second-difference
  penalty `λ`: the largest `λ` whose 25 °C discharge-pulse RMS is within 10 % of **the best fit
  found** — not of `λ = 0`, which the measurement showed lands in a worse optimum (16.1 mV against
  13.7 at `λ = 0.001`). Chosen: `λ = 0.003`, 14.55 mV.
* **Cold** (decision 2): the HPPC train at 263.15 and 318.15 K on the **combined reference**
  (Chen2020 + ORegan2022's solid-diffusion and electrolyte temperature terms; the same PyBaMM
  construction slice C's cold golden uses, and entropic terms off in both, or on in both —
  whichever matches the engine at that commit). The RC pairs are shared across temperatures
  (the schema), so the temperature lives in `R0` and, through decision 6, in the pairs'
  resistances by `R0`'s own ratio: the 263 and 318 K columns are fitted with the 298 K pairs held
  and scaled, to a fixed point, and the residual is stated per temperature. Not ORegan2022's
  own fits: the spike's were broken (nodes at 0 Ω, 941 mΩ at empty) by its different window.
* `[ocv]`: a per-chemistry grid in `fit_ocv.py` for this file (the TODO's condition comes true:
  this file now carries a circuit reference). The other chemistries' grids are untouched, byte for
  byte.
* A committed reference, `tests/golden/nmc_21700_lgm50/dfn_hppc_25c.csv` (and the cold and warm
  runs the fit used), and `crates/sim-data/tests/ecm_fit.rs`: the engine's circuit on the shipped
  file replays it within a bar **written into this note before the run**, plus the real-cell
  comparison (LG's 30 ± 6 mΩ at 50 %, 0.5C, 30 s) reported, not pinned — the spike put the fit at
  ~37 mΩ, just outside.
* The lessons: steps 2, 12, 13, 14 and the two scenario headers (`pulse_train_ecm.toml`,
  `cc_discharge_lgm50.toml`). Step 12's claim becomes one the fitted circuit makes true
  (a rest long enough for its slow pair, or a claim about convergence rather than identity);
  step 14's ×3.42 is not non-linearity but **where the drop is read**: at the end of the pulse,
  after a 3C pulse has spent 5 % of the charge and a 1C pulse 1.7 %, so a state-of-charge-dependent
  `R0` reads two different resistances. Read at the pulse's **onset**, both rates start from one
  state of charge and the circuit is ×3 exactly for any `R0(soc)`, so the lesson keeps its thesis
  ("no single resistance can be 1.87 and 6.02 at once"). If `path_claims` has no onset reading,
  slice A adds one. Claims re-measured with
  `path_claims`' own instruments, not by hand.

### B — the specification sheet (data)

`[cell]` per decision 5: `max_charge_c = 0.3`, `max_discharge_c = 0.5`, `t_max_k = 318.15`, each
citing its row of the sheet and saying what the one-number schema cannot (0.7C charge at
25–45 °C, 1.5–3.0C discharge above 10 °C, 55 °C to discharge). `[thermal]`: the heat capacity's missing 12.7 g (68.5 g on
the sheet against 55.8 g in Chen2020's layers), at a specific heat stated with its own source or
labelled an assumption — measured both ways before choosing. Every LG M50 lesson and test that
reads these re-measured.

### C — entropic heat and the cold, as data (against slice E's engine)

* **Measured before slice E's design, no Rust needed** (both scalar fields exist): in a worktree,
  a representative non-zero `[spm.*] docp_dt_v_per_k` and the two activation energies, the suite
  run with `path_claims` reporting every mismatch — the count of what moves. Not only cold tests:
  everything whose temperature is not bit-for-bit `t_ref_k` (hot runs, thermal networks, the WMG
  and film tests' storage temperatures) moves, and the reported heat moves on isothermal runs too.
* ORegan2022's two entropic curves extracted by `extract_spm.py` into slice E's tables,
  cross-checked against PyBaMM at 7 points; the circuit's `[ocv] docv_dt_v_per_k` is the same pair
  on the cell's window, with `t_ref_k` stated so the voltage shift is on (decision 3).
* Landesfeind 2019's EC:EMC 3:7 coefficients into slice E's `[dfn]` section, read out of PyBaMM's
  function source by `extract_dfn.py` (the way it already reads Nyman 2008's).
* `[spm.*] diffusivity_ea_j_per_mol` from ORegan2022 (`d·R`: 17.39 kJ/mol negative, 12.05
  positive) — the field exists.
* Goldens at the reference temperature unmoved (predicted: bit-identical, since every factor is
  exactly 1 there — measured, not assumed); the energy-ledger tests run with a non-zero entropic
  term; every test the measurement above names re-measured.
* A cold reference: PyBaMM's combined DFN at 263.15 K (entropic on, as the engine then is), and
  the engine's `Dfn` against it within a bar set before the run. The circuit's cold columns are
  re-fitted against the same construction if entropic-on moves them (slice A fitted entropic-off,
  matching the engine at that commit).

### D — the list, and the documents

`crates/sim-data/tests/lgm50_provenance.rs`: every line of the shipped file labelled placeholder
is on the fixed list, the list is exactly what is labelled (so neither side drifts), and every
constant this phase replaced carries a provenance naming its fit and residual or its citation.
`CLAUDE.md`, the README's status table and `docs/ROADMAP.md` (H3, §Phase 11) updated.

## Exit

1. `fit_ecm.py` prints the shipped `[r0]` and `[[rc]]` block identically on a rerun (scipy pinned
   in `requirements.txt` beside PyBaMM), and their provenance carries the 1C and 3C residuals per
   temperature.
2. The engine's circuit replays the committed converged DFN reference within the bar this note
   records before the run.
3. The LG M50 file's placeholders are exactly decision 1's list (`lgm50_provenance.rs`).
4. All three models generate entropic heat and shift their potentials with temperature from
   ORegan2022's curves, and every energy-ledger test closes with it.
5. The `Dfn` at −10 °C tracks the cold reference within its pre-set bar.
6. The workspace suite, `clippy -D warnings` and the guided path's claims are green, with steps
   12 and 14 redesigned rather than re-numbered.
