# Phase 13 — the porous models past full: the plan

**Status: PLANNED 2026-10-20, after the spike (`phase-13-spike.md`) and the owner's four decisions
there. Three questions for the owner are open (§"Owner questions"); slice A does not start before
they are answered.** ROADMAP H8's "no physics past FULL" bullet; `docs/ROADMAP.md` §3 "Phase 13".

## What the phase does

A porous cell charged on past full must stop where its particles do, and send what they cannot
take to a named reaction: **lithium deposited as metal at a full negative** and **electrolyte
oxidation at an empty positive**. Then the lithium book closes on stocks a particle can hold, the
energy ledger closes with the refused energy as heat, and a BMS-off overcharge reaches runaway by
that heat — on the `Spm` (slice A), the many-particle cell (B) and the `Dfn` (C), with the cold
4.2 V hold's kink as a slice of its own, last (D).

**Out of scope**, said first: the `Dfn`'s rate limit, where the positive *fills* on a hard discharge
(`dfn-electrolyte-limit.md`, the spike's case (c)); the 0.65 V rest after a fast cold charge from
empty, which is the negative's electrolyte running out (`plating-unconverged-runaway.md`); and the
equivalent circuit, whose clamp already refuses charge as heat.

**No outside reference.** PyBaMM 26.6 has no overcharge reaction (the spike). The oxidation's form
is the irreversible Tafel law used for solvent oxidation at a positive electrode — Lee, Park & Lu,
J. Electrochem. Soc. 166 (2019) A1340, eq. 6, after Darling & Newman, J. Electrochem. Soc. 145 (1998)
990 (the latter not read) — and every constant in it is a labelled placeholder with a stated sizing
rule. Lee 2019's own numbers are not used: they are LiMn₂O₄ aging at 55 °C with a transfer
coefficient of 0.01, a rate that barely rises with potential and cannot carry an overcharge.
Lithium deposition as the overcharge reaction of a carbon negative is Arora, Doyle & White,
J. Electrochem. Soc. 146 (1999) 3543 (cited, not read: the open copy refused an automated fetch);
the law used is Phase 12's own (`phase-12-slice-b-reaction.md`). Ren et al., J. Power Sources 364
(2017) 328, an overcharge-to-runaway model of an NMC cell, finds the electrolyte's oxidation and
the deposited lithium's reaction the largest heat sources of an overcharge; its full text was not
reachable. So **the exit promises conservation, convergence and boundedness, not a curve.**

## The design

### Two edges, two outlets

| edge | reached on LG M50 (spike) | reached on LFP (spike) | the particle | the rest goes to |
| --- | --- | --- | --- | --- |
| negative surface or bulk at `c_max` | 0.4–0.8 A·h past full | 0.25–0.55 A·h past full, once the positive's outlet carries the charge | carries at most the current that takes it to the edge over the step | **lithium deposition** (metal) |
| positive surface or bulk at 0 | 1.5–2.3 A·h past full | **at full** | the same, mirrored | **electrolyte oxidation** |

* **The edge itself is `porous-reversal.md`'s mechanism mirrored.** There, past the current that takes
  a particle's surface or bulk to *empty* over the step, the particle carries no more and the rest
  goes down the `[reversal]` ramp. Here, past the current that takes the negative's surface or bulk
  to `c_max`, or the positive's to 0, the particle carries no more and the rest goes to the outlet.
  A particle can then never hold an impossible stock. The space between the window's full and the
  particles' limits stays exactly as it is: the spike found it physically possible and already
  right.
* **The oxidation also runs by potential, before the edge**, as a reaction does: `j_ox` rises with
  `φ_s − φ_e − U_ox` at the positive. It is written to be **exactly zero below `U_ox`**:
  `j_ox = i0_ox·[exp(α·f·η) − 1 − α·f·η]` for `η > 0` and 0 otherwise. That is Tafel's law once `η` is
  more than a few `RT/αF`, continuous with its first derivative at the onset (so the Newton solves
  see no kink there), and identical to zero wherever the positive stays below `U_ox`. The `−1 − α·f·η`
  is this plan's, not the cited law's, and the note will say so.
* **The deposition on a cell that does not run Phase 12's reaction runs only at the edge** (owner
  question 1). Phase 12's lesson contrasts the threshold cell with the reaction cell on cold
  charges, and `plating_lesson.rs` and the path claims pin it. An outlet that plated on a threshold
  cell before its surface reached `c_max` would erase that contrast. At the edge it uses Phase 12's
  law (`J = F·k·c_e·[exp(−α·f·η) − exp((1 − α)·f·η)]`, against lithium at 0 V), so the cell's curve
  keeps a slope there. On a cell that runs the reaction, plating is unchanged; only intercalation
  gains the edge, which removes the 0.8–1 % creep past `c_max` the spike measured.
* **Intercalation's exchange current is never driven to exactly zero at a full surface.** That is
  how `dfn-cold-hold-taper.patch` failed: "no reaction" became a root, and an overcharged cell could
  not relax (`surface_gap.rs`). The edge caps the *current* a particle takes; it does not choke the
  reaction, so a particle at its edge gives lithium back on the next discharge as any other does.
  Every slice holds that as a bar.

### Where the oxidation's charge comes from

The oxidation takes no lithium out of the positive; it takes electrons out of the electrolyte.
The lithium that arrives at the negative in its place must come from somewhere.

* **Measured against the `Dfn`'s salt:** LG M50's electrolyte holds `0.1027 m² × (85.2 µm × 0.25 +
  12 µm × 0.47 + 75.6 µm × 0.335) × 1000 mol/m³` = 5.4 mmol of lithium ions, **0.14 A·h**. A
  1C overcharge pushes 1.5 A·h or more through the oxidation (the spike). If the oxidation made no
  cation, the salt would be gone a tenth of the way in, and the solve would walk into the
  electrolyte-limit family.
* **So the oxidation's cation is counted with lithium's** in the electrolyte's source term (on the
  `Dfn`, the total interfacial current; the `Spm` has no electrolyte to deplete), and the lithium
  book carries **an explicit source**: moles oxidised, a new per-cell counter. Physically the cation
  is a proton that reduces to gas at the negative; this model lets it arrive as lithium there,
  which is the simplification Ren 2017's class of model makes. Recorded as a modelling choice, not
  a finding.

### Heat

* **Oxidation**: its products are taken to store nothing, so all of its interfacial work is heat:
  `j_ox · (φ_s − φ_e)` against lithium at 0 V. This is the reaction that makes runaway reachable
  for the right reason, as the circuit's refused charge does.
* **Deposition**: Phase 12's booking, against lithium metal at 0 V, unchanged.
* Every ledger is checked step by step: electrical work in = change in stored energy (including the
  reversible metal) + heat.

### What the deposited metal does

* **On a cell that runs the reaction**: Phase 12's rules, unchanged (`phase-12-slice-c-health.md`):
  reversible metal counts in `soc_true` as stored charge and strips back at rest, `dead_share` is lost
  at once, the dendrite short rolls per dead amp-hour.
* **On a cell that does not**: the metal is reversible stored charge, counted in `soc_true` as
  Phase 12's reading (a) counts it, and **stripped first** on the next discharge — the mirror of a
  deficit "repaid first by a charge". What it costs is the threshold model's own per-amp-hour keys:
  `plating_fade_per_ah`, `plating_short_hazard_per_ah`, and `PLATING_RISK` while it deposits. That
  keeps one meaning per model and invents no new damage constant.

### What the readings say past full

| reading | today on a porous cell past full | after |
| --- | --- | --- |
| particle stocks | outside `[0, c_max]` | inside, always |
| `i_rejected_a` | 0 | the oxidation's current: charge passed and not stored, the circuit's meaning |
| `SOC_CLAMPED_HIGH` | every step past the window's full | a step on which an outlet carried charge |
| `soc_true` | 1.0 while the cell holds 1.6 times its negative's capacity | in `[0, 1]` as before, the reversible metal counted as stored charge |
| `SURFACE_OUT_OF_RANGE` | from 0.4–0.7 A·h past full | unreachable on the charge side; kept for the `Dfn`'s discharge-side limit |

### Data

A new section, **`[spm.overcharge]`**, required on every chemistry with `[spm]` (LG M50 and
`lfp_26650_prada2013`), unknown keys refused, each value with its provenance:

| key | meaning | sizing rule (placeholder) |
| --- | --- | --- |
| `oxidation_onset_v` | `U_ox`, against lithium | at least 0.2 V above the highest positive potential the cell's own window reaches at its rated charge rate, measured in slice A and stated — a property of the carbonate electrolyte, so the two files may share it |
| `oxidation_exchange_a_per_m2` | `i0_ox` | the value at which a 1C overcharge is carried at `η` = 0.2 V |
| `oxidation_transfer_coefficient` | `α` | 0.5, the symmetric default |
| `deposition_rate_constant_m_per_s`, `deposition_transfer_coefficient` | the deposition law's `k`, `α` at the edge | LG M50: `[dfn.plating]`'s values, cited there; LFP: the same, labelled borrowed |

The oxidation is isothermal in this phase (no activation energy); a temperature term is a
one-key change if a source turns up.

## Slices

* **A — the `Spm`.** Both edges and both outlets on the single particle, `[spm.overcharge]` in both
  files, the new counters, readings and heat; `SNAPSHOT_VERSION` 30 → 31 with its pair test. The
  spike's census re-run in the tree as `sim-data/tests/overcharge_spm.rs`.
* **B — the many-particle cell.** Its positive is the limit at full, so it is the cleanest test of
  the oxidation: each positive particle has its own edge, and the in-cell split must hand a
  particle's surplus to its neighbours before the oxidation. The many-particle cell's step loop and
  bracketed search carry it. Bump to 32.
* **C — the `Dfn`.** The edges node by node inside the Newton, the oxidation in each positive node's
  interface, deposition at threshold cells' full negative nodes, the creep removed on reaction cells.
  **The main risk of the phase**: an edge inside the node-level Newton is the cold-hold failure's
  class. Bump to 33.
* **D — the cold 4.2 V hold** (`dfn-cold-hold.md`), with its own bars (that note's: the fine grid
  with plating on at 0 °C and −10 °C, and nothing else moving). Whether A–C already removed its kink
  is a registered prediction.

## Predictions for slice A (registered before the engine changes)

1. **Bit-identical until an edge or `U_ox` is reached.** Every golden passes unchanged,
   `spm_exact_bits.rs` included; every test outside the overcharge regime passes unchanged. Tests
   that charge an `Spm` past its negative's `c_max` may move, and each that does is named with its
   old and new figure — expected: `surface_gap.rs::an_overcharged_cell_relaxes_to_no_gradient_at_all`
   (its `Spm` arm charges at 3C from half for 3000 s, to a bulk of 1.19; the `Dfn` arm waits for
   slice C), and any `Spm` case in `demand_window.rs`, `spm_pack.rs` or `spm_long_step.rs` that drives a
   cell past its negative's `c_max`.
2. **No stock outside `[0, c_max]`** on the spike's `Spm` runs (1C and 3C, BMS off): every shell of
   both particles within `[0, c_max]` to 1e-12 relative, on every step.
3. **The lithium book closes**: negative + positive + reversible metal − moles oxidised, constant to
   1e-9 of the charge passed, on every step.
4. **The energy ledger closes** step by step to the tolerance the existing `Spm` ledger tests use.
5. **A BMS-off overcharge reaches runaway** under a thermal network at 1C and 3C (onset not timed;
   no prediction of when).
6. **A cell stopped at its edge gives the lithium back**: after the 1C overcharge, a 1C discharge
   converges on every step, strips the reversible metal first, and reaches the cut-off.
7. **The pack converges past full**: a scattered 1S2P under a current demand and a single cell under
   a 4.2 V hold, both started past full, at 1 s, 15 min and 1 h steps — no unconverged step, every
   cell inside its edges.
8. **Cost**: a step that reaches no edge and stays below `U_ox` costs what it costs today (one
   comparison per particle).

Predictions for B, C and D are written at the head of each slice's own note, before its engine
change.

## Exit

On each porous model, a BMS-off constant-current overcharge keeps every particle inside
`[0, c_max]`, sends what they cannot take to deposition and oxidation, closes the lithium book (with
its oxidation source) and the energy ledger, and reaches runaway by the oxidation's heat under a
thermal network; a cell stopped at its edge discharges converged; parallel groups and voltage holds
converge past full at 1 s, 15 min and 1 h steps; every golden passes unchanged. Pinned by
`sim-data/tests/overcharge_spm.rs`, `overcharge_ensemble.rs`, `overcharge_dfn.rs`, and slice D's own
bars in `cold_hold.rs`.

## Owner questions

1. **The negative's outlet on a cell that does not run the plating reaction** — every `Spm`, every
   many-particle cell, and every `Dfn` on the default threshold. Recommended: deposit metal only
   once the graphite is truly full, so everything before that stays as today and the plating
   lesson's threshold-against-reaction contrast holds. The alternative, running the plating reaction
   on every cell, is more physical and changes every cold charge on the default cells.
2. **Ren et al. 2017.** If the owner can supply the full text, its oxidation constants replace the
   sizing rules above (cited); if not, they stay placeholders.
3. **A lesson.** Phases 9, 10 and 12 ended in a guided-path lesson. A slice E — overcharge with the
   BMS off on a porous cell: the voltage climbing onto the oxidation, the metal depositing, the
   heat, runaway — or none in this phase.
