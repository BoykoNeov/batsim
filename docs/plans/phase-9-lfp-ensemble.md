# Phase 9 — an LFP cell whose plateau and hysteresis come from its particles

**Status: PLANNED 2026-10-06, nothing built.** Written after slice A's spike and its second
round (`phase-9-slice-a-spike.md`), which is the measurement this plan stands on. Where a
later slice note contradicts this text, **the slice note is the measurement and wins**. The
repo is at `SNAPSHOT_VERSION` 24 and `WASM_API_VERSION` 8.

This replaces the slice list in `docs/ROADMAP.md` §Phase 9, which assumed the published
interaction strength would give the plateau and the hysteresis. On the spike's measurement it
gives neither at an affordable particle count.

## The owner's decisions, recorded

All four were asked and answered on 2026-10-06.

1. **Build on a fitted interaction strength.** Ω = 0.0759 eV, chosen so that the two
   extremes of the regular-solution curve sit 20 mV apart at 298.15 K. That matches the rest
   gap Dreyer 2010 (Nat. Mater. 9, 448) measured. Bai 2011's 0.183 eV is the published
   value, and with it 20 particles give a 168 mV span over 20–80 % and no rest gap at all.
   The alternatives were a Cahn–Hilliard model inside every particle, a shrinking-core
   model, or declining H1.
2. **Keep U0 = 3.42 V (Bai 2011).** Do not fit it to PyBaMM's Prada2013 curve (mean
   ≈ 3.40 V). The cell then reads +10.3 mV above PyBaMM's SPM at C/20, and that offset is
   documented, not tuned away. Ω stays the only constant we fitted.
3. **The 1 h step's non-monotone voltage is a documented limit, not a fix.** See
   §"Long steps" below. No pack code changes for it, and the cell declares no new range for
   it.
4. **The lesson shows the cold too, as a range.** At 263 K the rest gap is one of a few
   fixed values set by how many particles end full, so a single number would be one draw.

## What the spike established, and what the plan may therefore assume

All numbers come from the full cell: a 20-particle LFP ensemble against one Prada2013
graphite particle, 20 shells, engine `spm::diffuse`, charge conserved to ≤ 9e-13. Harness
and outputs are in `W:/temp/claude/phase9-spike/`.

| property | measured | what the plan does with it |
| --- | --- | --- |
| plateau from particles filling one at a time | yes, at the fitted Ω with 20 particles; not below ~100 at Bai's Ω | the model's whole point; an exit criterion |
| C/20 vs PyBaMM SPM (Prada2013, Afshar's monotone OCP), 20–80 % | +10.3 mV mean (+6.3…+17.5), span 138 vs 145 mV | golden, with the offset attributed to U0 |
| LFP window in this cell | graphite runs out first; LFP y 0.0038–0.7035, 2.3034 Ah | the chemistry's stoichiometry window and capacity |
| rest gap at 298 K (C/20 approach to SOC 0.5, 2 h rest) | 14.2–19.6 mV over 4 seeds, σ 0.1/0.3, N 10/40, SOC 0.3/0.7; discharge-arrival always lower; seed spread 1.2 mV | exit criterion with a band |
| rest gap at 263 K | 0.00 / 27.44 / 23.32 / 13.35 mV on seeds 9/1/2/3, each the count table's value to 0.01 mV | the lesson shows a range; the test pins the mechanism |
| 1 s steps, C/20 to 3C, 10 and 20 shells | no sub-step needed anywhere | the real-time path needs no internal sub-steps |
| surface read at 10 shells | folds while a particle transforms; 20 shells clear N = 20, N = 100 needs 40 | minimum shell count is a validated setting, not a default |
| whole cell's end-of-step V(I), same sub-steps for every probe | falls with current at 1 s, 60 s and 15 min at all 20 states; rises at 13 of 20 states at 1 h, by up to 6.6 mV | §"Long steps" |
| the same, with sub-steps chosen by halving a failed trial | rises by up to 30 mV at 15 min | a design rule for slice B (below) |

Not established, and not assumed: the cost of an N-particle cell inside the engine, the
knee's rate dependence, any PyBaMM reference for phase separation (Prada2013's OCP is
monotone by construction), and particle counts above 40 at the full-cell level.

## Slices

### B — the ensemble cell (engine; one snapshot bump, 24 → 25)

- `CellModel::SpmEnsemble(Box<EnsembleState>)` beside `Spm`, selected by
  `CellModelConfig::SpmEnsemble { shells, particles, radius_sigma }`. The **positive**
  electrode is the ensemble; the negative stays one particle, as in the spike.
- Radii: lognormal around `[spm.positive].particle_radius_m`, drawn once at `Pack::new` from
  the pack's seeded RNG. **Every particle gets an equal volume share; its radius sets only
  its area** (3V/R). That is the spike's choice, and the cold result depends on it: with
  equal shares the rest voltage depends only on how many particles are full. A
  radius-weighted share is a different model and needs its own measurement.
- The OCP gains a second form. `[spm.positive.ocp]` stays a table, and a new
  `[spm.positive.regular_solution] u0_v, omega_ev` gives `U(y) = U0 − kT·ln(y/(1−y)) −
  Ω(1−2y)`. That form is non-monotone, so the plain `Spm` **refuses** it at load (the spike
  showed one particle cannot carry it), and the ensemble accepts either form. kT uses the
  cell's own temperature. Ω is held constant across temperature, as in the spike.
- **The split lives inside the cell**, and the pack is not edited (principle 9). Each step
  solves for particle currents that share one electrode potential and sum to the cell
  current. Each particle's end-of-step outer shell is affine in its flux (two `diffuse`
  calls), as in the spike. The residual is audited, never discarded: the spike's first run
  discarded it and lost charge.
- **Sub-division rule (from round 2): a step's internal sub-steps are fixed from the step
  length and state before the split. They are never found by halving a trial that failed.**
  Halving made the sub-step count jump between neighbouring trial currents, and that jump
  alone turned a 6 mV property into a 30 mV one. The real-time path needs none (1 s: no
  sub-step at any rate). The fast-forward sub-step length gets measured in this slice.
- `source`, `probe_at` and the current window keep `Spm`'s contracts. The window is the
  hard part: with an internal split, "every surface stays in range" is no longer one
  interval per electrode by closed form. In round 2, 228 of 1 220 probes at 15 min left
  the range. Measure before choosing.
- **`N = 1` with a table OCP is bit-identical to `Spm`** on every existing `Spm` golden and
  scenario. That is the guard that the new variant did not quietly change the old physics.
- Not in this slice: `Dfn`, a non-monotone OCP on the negative electrode, particle-size
  distributions on graphite.

### C — the LFP chemistry and its goldens (data plus `tools/reference/`)

- A new chemistry file, Prada2013's A123 26650 LFP/graphite (2.3 Ah). It needs a new file
  because `lfp_26650_generic.toml` is a 2.5 Ah ECM fit, and moving its capacity would move
  every ECM golden and guided-path number built on it. Its `[spm]` section is extracted by
  `tools/reference/`, with the positive OCP replaced by the regular-solution form (U0 3.42,
  Ω 0.0759 fitted, with provenance saying so). Its ECM sections are placeholders, labelled
  as such.
- Goldens (`sim-data/tests/`):
  - C/20 against PyBaMM's Prada2013 SPM, offset band anchored on round 2's +10.3 mV;
  - the 298 K rest gap on four seeds;
  - the 263 K rest gaps against the count table;
  - charge conservation;
  - `N = 1` bit-identity (carried from B).
- Long steps: the measured 1 h non-monotonicity is pinned by a test. The limit goes into
  `CLAUDE.md`'s pack-solve section and the ROADMAP, so the limit and the measurement cannot
  drift apart.

### D — the scenario and the lesson (client)

- A scenario on the new chemistry, the ensemble model, and its default seed.
- Guided-path steps:
  - the flat plateau (the particles fill one at a time, which can be seen in the carrier
    diagram if it shows particles);
  - the rest gap depending on the direction of arrival, at room temperature;
  - the same in the cold, **as a range across seeds**. The page states that the cold gap
    is one of a few fixed values, and which one is a draw.
- The lesson is ledgered like the others. Phase 8's rule holds: the chemistry is done when a
  lesson teaches it.

## Long steps — the documented limit

The pack's split and its voltage and power holds bracket a root on each cell's end-of-step
curve, assuming it falls with current. On this cell, measured with the same sub-steps for
every trial current, it falls at 1 s, 60 s and 15 min at all 20 states. At 1 h it rises at
13 of 20 states, by 1.5–6.6 mV, at small charging currents (−0.25 C at SOC 0.65). There an
hour's charge walks the LFP across one of its teeth, and the graphite's slope no longer
outweighs it.

**Decided: documented, not fixed.** A pack of these cells at ≥ 1 h steps is not guaranteed
to land on the right root if it has cells in parallel or is under a voltage or power demand.
A single series string under a current demand is unaffected, because that solve brackets
nothing. The limit is stated where the pack's assumption is stated, with the measurement,
and checked by a test so a later change to the cell cannot silently move it.

## Exit criteria

1. `N = 1`, table OCP: bit-identical to `Spm` on every existing `Spm` golden and scenario.
2. The plateau: C/20 discharge of the 20-particle LFP cell against PyBaMM's Prada2013 SPM.
   The mean offset over 20–80 % lies within a stated band around +10.3 mV, attributed to U0.
   The 20–80 % span is within a stated band of 138 mV. Bands are set in slice C from the
   seed spread, not chosen to pass.
3. Hysteresis at 298 K: C/20 approach to SOC 0.5 from each side, then 2 h rest. The gap is
   14–20 mV on four seeds, and discharge-arrival reads below charge-arrival on every seed.
4. Hysteresis at 263 K: on each of four seeds the gap equals a count-table value to
   0.1 mV, and is never negative.
5. Long steps: end-of-step V(I) falls with current at 1 s, 60 s and 15 min at 20 states. The
   1 h non-monotonicity is pinned by a test and documented as the limit above.
6. Charge conserved to 1e-12 of capacity; no unsolved split from C/20 to 3C at a 1 s step.
7. Per-cell step cost measured and recorded (no budget is set: the 50 µs budget is for a
   100S10P ECM pack).
8. A guided-path lesson teaches it, ledgered.

## What this phase does not close (H1)

- The gap's size at 298 K is capped by the fitted Ω: a gap below a bound we fitted is
  guaranteed, not confirmed.
- The knee's rate dependence was not measured.
- There is no external reference for phase separation. Any number about the plateau's shape
  beyond "flat, offset by U0" is the model's own.
