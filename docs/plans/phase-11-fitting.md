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

One snapshot bump covers both engine changes (`SNAPSHOT_VERSION` 27 → 28): the chemistry is part
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

### A — the circuit (Python + data + lessons; no Rust)

* `tools/reference/fit_ecm.py`: the HPPC train on the **converged** Chen2020 DFN (200 radial
  points, `rtol = atol = 1e-10`, `common.DFN_CONVERGED`), the fit against the reference's own OCV
  in the engine's update, printing a TOML block whose provenance names the design rate and the
  residuals (1C per temperature and per segment, and 3C). Run twice → identical output.
* **Cold** (decision 2): the HPPC train at 263.15 and 318.15 K on the **combined reference**
  (Chen2020 + ORegan2022's solid-diffusion and electrolyte temperature terms; the same PyBaMM
  construction slice C's cold golden uses, and entropic terms off in both, or on in both —
  whichever matches the engine at that commit). The RC pairs are shared across temperatures
  (the schema), so the temperature lives in `R0` alone: the 263 and 318 K columns are fitted
  with the 298 K pairs held, and the residual that costs is stated per temperature. Not ORegan2022's
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

### C — entropic heat and the cold, in the engine (v27 → v28)

* **Measured before the design, no Rust needed** (both scalar fields exist): in a worktree, a
  representative non-zero `[spm.*] docp_dt_v_per_k` and the two activation energies, the suite run
  with `path_claims` reporting every mismatch — the count of what moves. Not only cold tests:
  everything whose temperature is not bit-for-bit `t_ref_k` (hot runs, thermal networks, the WMG
  and film tests' storage temperatures) moves, and the reported heat moves on isothermal runs too.
* **The energy ledger first**: no shipped porous chemistry has had a non-zero entropic term, so
  that path is untested. The reversible heat closes against `U − T·∂U/∂T`, not `U`; check how the
  circuit's ledger already handles NiMH's column, and whether the porous ledgers' stored-energy
  term carries the `T·∂U/∂T` part. If not, the ledger change is part of this slice.
* `[spm.*] docp_dt_v_per_k` accepts a table over stoichiometry (a scalar file still loads, as a
  one-value table or by an explicit migration — decided in the slice's design addendum); the
  `Spm`, `Dfn` and `SpmEnsemble` read it at each particle's surface (PyBaMM's convention) for heat
  **and** for the potential's temperature shift (decision 3).
  ORegan2022's two curves extracted by `extract_spm.py`, cross-checked against PyBaMM at 7 points;
  the circuit's `[ocv] docv_dt_v_per_k` is the same pair on the cell's window.
* `[dfn]` electrolyte transport gains a temperature dependence; its form (an activation energy
  per property, fitted to Landesfeind 2019's ratio with a stated residual, or the published form
  itself) chosen by measurement over 0.2–2 kmol/m³ and 263–318 K.
* `[spm.*] diffusivity_ea_j_per_mol` from ORegan2022 (`d·R`: 17.39 kJ/mol negative, 12.05
  positive) — data; the field exists.
* The snapshot pair test and an explicit refusal of v27 blobs. Goldens at the reference
  temperature unmoved (predicted: bit-identical, since every Arrhenius factor is exactly 1 there —
  measured, not assumed); the energy ledger tests now run with a non-zero entropic term; cold
  `Spm`/`Dfn` tests re-measured.
* A cold reference: PyBaMM's Chen2020 DFN with ORegan2022's solid-diffusion and electrolyte
  temperature terms at 263.15 K, and the engine's `Dfn` against it within a bar set before the run.

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
