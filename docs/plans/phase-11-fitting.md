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

## Addendum 2026-10-15 — slice E done: the engine, v27 → v28, nothing shipped moves

Three commits, each green on the whole suite and `clippy -D warnings`:

1. **The schema and the RC rule** (6ec73b2). All three fields land with the one bump; a
   present entropic table or electrolyte section was refused until the commit that reads it.
   The RC rule enters through one per-cell multiplier, `soh_resistance · R0(soc, T) /
   R0(soc, rc_t_ref_k)`, passed wherever `soh_resistance` reached the pairs (it reached
   nothing else), so the decay memo keys on it unchanged. **Every snapshot's bytes move this
   time** — every chemistry has an `[r0]` table — and a v27 blob fails at that table with one
   pair or two (`snapshot_version.rs`). The model-based estimator keeps the table's pairs and
   takes the rule as model error, as it does aging. `rc_temperature.rs`: bit-identical at the
   reference temperature, the scaled closed form to 1e-9 V away from it.
2. **Entropic tables and the potential's temperature shift** (472f2f6), in the `Spm`, the
   `Dfn` and the many-particle cell. **A design point the plan did not have**: with
   `U(T) = U + (T − t_ref)·∂U/∂T`, the heat `I·(U(T) − V) − I·T·∂U/∂T` is exactly
   `I·(U − t_ref·∂U/∂T − V)`, so every heat path reads that **stored-energy potential** in place
   of `U` and needs no separate reversible term — and the energy ledger closes against the
   same potential the film's lithium is booked at. Read at the bulk, where the irreversible
   heat always was; PyBaMM reads `∂U/∂T` at the surface, and the two part only by its change
   across the surface-to-bulk gap. A non-zero scalar `docp_dt_v_per_k` now shifts the potential
   too (none ships). `entropic.rs`, on the LG M50 with a synthetic table: no move at `t_ref` to
   the bit; a rested `Spm` and `Dfn` shift by `(T − t_ref)·(∂U_p/∂T − ∂U_n/∂T)` to 1e-12 V; the
   ledger over a 1C half hour closes to 1.25 J with the tables against 1.19 J without, at −10
   and +45 °C, while the tables move the heat by 337 and 399 J (bar 5 J, set before the run).
3. **The electrolyte** (this commit). Landesfeind 2019's ratios, precomputed per step, times
   the Nyman fits, product rule in the derivatives; exactly `(1, 0)` at `t_ref`.
   `electrolyte_temperature.rs`: the ratios are the published forms to 1e-12 (their
   derivatives to 1e-6 — **the first bar missed**, 1.0e-6 at one point, by the test's own
   central difference at a step of 0.1 % of `c`; at 0.01 % it passes, so the instrument, not
   the engine); bit-identical at `t_ref`; at −10 °C under 1C the whole section lowers the
   voltage 136.9 mV and the conductivity alone 26.7 mV.

**Perturbations**, each restored: dropping the RC factor, the stored-energy term, the `Spm`
terminal's shift, the `Dfn` residual's shift, the diffusivity ratio's derivative term, or the
conductivity ratio each reddens the test aimed at it. **Two blind spots, recorded.** The first
version of the electrolyte test could not see the conductivity at all (its ratio dropped, still
green — the diffusivity lowers a cold cell on its own); a conductivity-only chemistry fixed
that. And dropping the conductivity ratio's derivative term — or zeroing the whole
conductivity derivative, Nyman's part included — stays green on **every** Jacobian check in the
repo: the row-relative comparison at 1e-5 cannot see entries that small. That is older than
this slice, and it bounds what is at stake: the Jacobian steers Newton's convergence, not where
it converges, and the residual's conductivity is pinned.

## Addendum 2026-10-15 (later) — slice A: the fit with the RC rule, and the replay bar, set before the run

**The fit**, re-run with `fit_ecm.py` simulating the engine's rule exactly (each sample's pairs
scaled by `R0(soc, T)/R0(soc, 298.15)` at the **start-of-step** charge, as the engine reads
`R0`; the cold and warm columns fitted with the coupling inside the least squares). The first
measurement put to the owner (11.6 mV, 67 mV at −10 °C) used one mean factor per temperature;
measured per sample, time-weighted RMS above 5 % charge:

| run | rule off | **rule on (shipped)** | free pairs per temperature (unrepresentable) |
| --- | --- | --- | --- |
| 1C, 25 °C | — | **4.85 mV** (pulses 14.58, moves 8.16, rests 2.70) | — |
| 3C, 25 °C (scored, not fitted) | — | **18.81 mV** | — |
| 1C, −10 °C | 22.50 mV (pulses 128.3) | **11.16 mV** (pulses 68.6) | 10.74 mV |
| 1C, +45 °C | 5.85 mV | **3.53 mV** | 3.71 mV |

Pairs 14.735 mΩ / 36.8 s and 9.102 mΩ / 329.6 s; `λ = 0.003` by the corrected rule. The cold
column is rougher than the warm (largest second difference 8.4 against 4.1 mΩ) but falls
monotonically to its 30 % node and is flat within ±3 mΩ above it.

**The replay bar, written before `ecm_fit.rs` first ran.** The engine replays each committed
reference (`ecm_hppc_*.csv`) on the shipped file, at the CSVs' own 1–30 s steps, through its
`[ocv]` table (re-gridded to 0.92 mV) rather than the reference's own OCV. Its time-weighted
RMS above 5 % charge must be within **1.0 mV** of the fit's figure in the table above, on each
of the four runs — the table's error is the one thing the fit did not see, and 1.0 mV is a
little above its worst case.

## Addendum 2026-10-15 (last) — slice A done: the circuit fitted, the pulse lessons reworded

**The replay passed its bar** (`ecm_fit.rs`): the engine on the shipped file, through its own
`[ocv]` table and step, against the four references — 4.818 mV (fit 4.85), 18.796 (18.81),
11.104 (11.16), 3.512 (3.53); every gap under 0.06 mV against a 1.0 mV bar. Deleting
`rc_t_ref_k` from the shipped file reddens it (−10 °C 28.8 mV, +45 °C 8.9). The circuit's
30 s resistance at 50 %, 0.5C, 25 °C is **37.5 mΩ against LG's 30 ± 6** — just outside, as the
spike predicted: it is fitted to Chen2020's DFN, whose cell is stronger than the real one
(slice-E measurement: 55 A for 10 s where the sheet gives 37). Reported, not pinned.
`fit_ecm.py --fit` prints the shipped block identically on a rerun; numpy 2.4.6 and scipy
1.17.1 are pinned beside PyBaMM.

**The lesson cost, measured on the final file, and the owner's choice.** Only `path_claims`
reads these values (the whole workspace otherwise green). 25 claims moved; the ×3.42 the
spike feared was its zig-zag `R0` — on the smoothed fit step 14's ratios are ×2.99 and ×3.00,
the numbers its prose already printed. What broke **in words** was steps 12–13: with the slow
pair at 330 s against a 600 s rest the circuit's rebounds go 67.6, 68.5, 68.6, 68.6, 68.6 mV —
not "identical to four decimal places" — and step 13's "a linear system has no way to carry
anything from one pulse to the next" is false of it. **Owner, 2026-10-15: reword, same train**
(a longer rest was the alternative, at the cost of every single-particle number in steps
13–14). Step 12 now teaches that a linear circuit's only memory is a fixed fade that identical
pulses settle; step 13 contrasts the circuit's "rose and stopped" with the particle's doubling;
its "9 s and 72 s because someone picked round numbers" became the fitted 37 s and 330 s; the
"two levels" of sag, an artefact of the old table's node at 85 %, became the depth creeping
220.5 → 228.4 mV. Step 2 and step 14 are re-measured numbers only. One rule was **closed** by
the rewrite rather than moved: step 13 used to compare the particle's fifth rest with the
circuit's first and its rule admitted the frame gap was safe only while the circuit's teeth
were identical; the sentence now says "the circuit's first". Scenario headers
(`pulse_train_*`, `cc_discharge_lgm50`, `cc_discharge_3c_*`) rewritten to match; the
lgm50 header's old ECM-against-SPM figures are labelled history, not re-measured.

## Addendum 2026-10-15 — slice B done: the specification sheet, and a heat capacity

`[cell]` takes the sheet's strictest rows (decision 5): 0.3C charge, 0.5C discharge, 45 °C.
`[thermal]` was measured both ways, as planned: Chen2020's layer sum scaled to the sheet's
68.5 g is 44.75 J/K (an assumption covering 12.7 g of can, header and electrolyte), and
**ORegan2022's** — its component specific heats measured on this cell — is 51.622 J/K over
64.10 g, 55.16 J/K at 68.5 g with the remaining 4.4 g at its mean. The second rests on the
smaller assumption and a source the owner had already taken, so the file ships 55.16. The
runaway placeholders are tuned as a pair with the heat capacity (an 818 K adiabatic rise,
about a minute onset-to-vent), so they were scaled with it (×1.513: 29.8 → 45.1 kJ,
8.0 → 12.1 W) and stay labelled placeholders. **Nothing in the workspace moved**: the whole
suite, the guided path's claims checker run in report-all mode included, is green with no
claim moved — no LG M50 lesson runs with a battery manager or a thermal network.

## Addendum 2026-10-15 — slice C: the entropy and cold data, and what they surfaced

**The data**, printed by `tools/reference/extract_oregan.py` (new): ORegan2022's entropy
curves as `[spm.*.entropic]` tables (28 and 22 breakpoints, ≤ 1.7 µV/K, cross-checked at 7
points), the circuit's `[ocv] docv_dt_v_per_k` column on the same window with `t_ref_k =
298.15`, the solid-diffusion activation energies (17 393.9 and 12 047.7 J/mol, `d·R`), and
`[dfn.electrolyte_temperature]` with Landesfeind 2019's coefficients read from PyBaMM's
source. At 298.15 K none of it moves a voltage (the circuit's 25 °C references regenerated
byte-identical in their data), but reversible heat is non-zero at any temperature.

**Sorted before anything was re-pinned** (the reviewer's rule: at exactly 298.15 K only heat
may move). Of the ~20 reds: two lesson numbers, both heat (`q_gen_at` — 6.33 → 6.19 W and 22.39
→ 22.26 W, re-measured); value pins (the `[dfn]` count with the new values pinned by name, the
activation energies, the `[spm]` hash — whose value list had never read the new table, now
extended); `load.rs`'s pre-v18 rule (LG M50 exempted by name); slice E's own fixtures, whose
"plain" cell is now the shipped one stripped; the reversal ledgers, whose stored energy is now
`U − t_ref·∂U/∂T`; and the film tests. Each of the rest was a finding:

* **The circuit and both porous models read the reversible heat at one point of a long step.**
  Folding it into the stored-energy potential (slice E) was right for the instant and the
  ledger, but handed it to the two-point rules, and a coefficient that changes sign across the
  window does not follow an endpoint. Fixed exactly: the charge (circuit) or bulk stoichiometry
  (porous) moves linearly under the step's current and the tables are piecewise linear, so the
  reversible mean is the table's exact mean over the path — `ecm::interp1_mean`, carried as
  `Advanced::entropic_mean_w` into the network's heat; the reported instant is unchanged; NiMH's
  constant column is skipped so it runs bit-identically. Measured adiabatic: the circuit's
  hour-long 1 A step from 30 % booked 0.57 K of reversible heat against 2.17 K in one-minute
  steps and now 2.234 K against the tables' integral; the `Spm`'s hour matches the integral to
  four decimals (−1.0399 K). `spm_long_step.rs`'s ratio test was split, since its ratio of
  totals mixed the two: the resistive part on its old bar (0.827, as recorded), the reversible
  part against the integral. `entropic.rs` holds the circuit's.
* **The `Dfn`'s long steps under a thermal network.** With temperature-dependent transport a
  step solved at its starting temperature over-heats when the cell warms 15 K across it: a
  half-hour 1C step +10.5 % against a 10 % bar, an hour's C/2 charge's REPORTED heat +22 %
  (reversible heat moving under an end-instant report). 15-minute steps: −1.3 % and +4.1 %.
  **Owner: a recorded 15-minute limit**, no engine change; `dfn_long_step_heat.rs` runs those
  two cases at 15 minutes and pins the half-hour +10.5 % between 5 and 15 %.
* **The SEI film's potential now shifts with temperature**, so its PyBaMM references (made
  without entropy) disagreed away from 25 °C (2.96 % at 45 °C). Regenerated with the file's
  tables and activation energies as PyBaMM inputs (`sei_golden.py::with_temperature_terms`; the
  25 °C rows byte-identical), the tests are back at their old precision. The film-energy ledger
  needed `U − t_ref·∂U/∂T` too; its "touches no positive lithium" check became a comparison with
  a film-less control (bit-exact on the `Spm`; on the `Dfn` the TOTAL, since the film moves
  lithium between positive nodes at rest), because the diffusion solve's own rounding drift grew
  from 4e-15 to 3.75e-11 with the Arrhenius factor. The WMG conversion-ratio test now runs
  isothermal like the PyBaMM figure it compares with: its capacity legs used to warm the cell,
  harmless until the potentials moved with temperature (it read 1.5500 against 1.567).
  Phase 10's exit 5 against the real cells still passes — no owner question was needed.
* **The circuit's cold references** regenerated with the entropy terms and the cold columns
  refitted: −10 °C 11.15 mV, +45 °C 3.51 mV; the replay within 0.06 mV again.
* **Exit 5, the cold `Dfn`** (`dfn_cold.rs`, reference `dfn_cold_golden.py`). The bar set
  before the run — 12 mV, whole run, recommended 10/5/10 grid — **failed at 415 mV**.
  Single-term references put the whole gap on the electrolyte's diffusivity (conductivity
  alone 3 mV, solid diffusion 5, entropy 3), and refining the x-grid closes it (41 → 5 → 1.5 mV
  at 2500 s for 10/5/10, 20/10/20, 30/15/30): at −10 °C the electrolyte diffuses 0.4× as fast,
  its profile steepens, and the recommended grid is too coarse — the same limit the 25 °C 3C
  test records. The engine's ratios equal PyBaMM's own evaluation to six digits. A second bar,
  set before it ran on 20/10/20, failed on its first step (16.3 mV — the radial first-step
  error, larger cold). **The final bar was set after measurement and says so**: on 20/10/20,
  12 mV from the second step while the reference reads ≥ 2.75 V (9.15 mV measured, then 1–4),
  the first step under 20 mV (16.3), the cut-off within 1 % (2784 s against 2785). The
  recommended grid's cold cut-off, 4.4 % early, is pinned as a recorded limit.

Two more, found by the slice's full suite after the reversible-mean change:

* **The many-particle cell** had been left on the endpoint rule, and its one-particle case is
  pinned bit for bit to the `Spm` — on the LG M50 (now with entropy tables) it parted in the
  last bits. It takes the same split now (each positive particle's exact path mean, averaged;
  a regular-solution positive has no table and keeps its own `reversible_w`), and one particle
  is the `Spm` bit for bit again.
* **A rounding hair past the `Dfn`'s empty edge.** A scattered 1S1P held at an unmeetable 10 W
  for hours, one kelvin warmer than before because of the new reversible heat, was flagged
  `SOC_CLAMPED_LOW` at hour 1: the pack's range reaches a cell through its line, the mapped
  current landed 2.6e-17 A past the cell's own 0.2065 A edge, and `split` booked a 5.1e-18
  deficit. Within eight ulps of the edge, from no deficit, the current is now the edge — the
  same "rounding hair either side" convention `pack_current_window` already applies at zero.
