# Phase 9, slice C — the LFP many-particle chemistry and its goldens

**Status: BUILT 2026-10-08.** Plan: `phase-9-lfp-ensemble.md` §C. No snapshot bump, no engine
code changed. Predictions written before measuring are in `W:/temp/claude/phase9c/PREREG.md`;
the harness (`ens/`, slice B's, pointed at the shipped file) and every raw output sit beside it.

## What was built

* **`chemistries/lfp_26650_prada2013.toml`.** The same cell as `lfp_26650_generic.toml` — the
  same PyBaMM set, the same fitted `[ocv]`, the same 2.303451 Ah — with an `[spm]` section
  extracted from Prada2013 by `tools/reference/extract_spm.py`, and on its positive electrode
  `[spm.positive.regular_solution] u0_v = 3.42, omega_ev = 0.07591738897375097`. Its `[r0]`,
  `[[rc]]`, `[reversal]`, `[thermal]`, `[aging]`, `[safety]` and `[diagram]` are copied from the
  generic file and keep their placeholder labels.
* **The extractor**, fixed in two places (below) and taught the regular-solution block: U0
  cited (Bai 2011), Ω **fitted** by bisection so the curve's turning points sit 20 mV apart at
  298.15 K (Dreyer 2010). It reproduces the spike's Ω to eleven digits; the last bits differ
  because the script uses the engine's gas constant (8.31446261815324, not 8.314462618).
  `common.PARAM_SETS` maps the new id to Prada2013, so `fit_ocv.py` gives the same `[ocv]`.
* **The golden**, `tests/golden/lfp_26650_prada2013/spm_cc_c20_25c.csv`: PyBaMM's SPM at C/20
  on the set's own monotone potential, on the converged settings (`SPM_CONVERGED`).
* **`crates/sim-data/tests/lfp_ensemble.rs`**, nine tests: the section to the bit, Ω's fit,
  the capacity against the negative electrode's geometry, which models run the file, and
  criteria 2–6 on the shipped file.
* **Slice B's tests moved onto the file.** `ensemble.rs` ran its many-particle tests on a
  fixture built from the spike's parameters; that fixture is gone, its eleven tests pass on
  the shipped file unchanged, and one is added (the capacity they use is the file's). The
  energy ledger reads 0.48 J of 258 J (0.48 of 257 on the fixture). **Green is not the same as
  still guarding**, so slice B's engine perturbation table (`W:/temp/claude/phase9b/pert.py`,
  eight breaks in `ensemble.rs` and `ecm.rs`) was rerun on the moved tests: every row is
  caught by exactly the tests that caught it on the fixture — including the bracketed
  fallback, whose only guard (`a_hard_charge_into_a_full_electrode_keeps_its_particles_bounded`)
  depends on the tangent passes diverging at three hard-charge points, which they still do.
* The sweeps over every shipped chemistry (`load.rs`, `diagram.rs`) include the new file;
  `CLAUDE.md`, `README.md` and `ROADMAP.md` count eight.

### Why a new file, and the plan's reason that was wrong

The plan said `lfp_26650_generic.toml` is "a 2.5 Ah ECM fit". It is not: it is already fitted to
Prada2013 at 2.303451 Ah. The 2.5 Ah is `CLAUDE.md`'s example block, which describes a shape,
not the file. The plan is corrected. The new file stays, for a reason that holds: a
`[spm.positive.regular_solution]` makes `Pack::new` refuse the plain `Spm` and the `Dfn`
(`RegularSolutionNeedsEnsemble`), so putting it in the generic file would close two models to
it, and every LFP scenario and guided-path step on that file would carry a section it never
reads.

### Two silent misreads in the extractor

Neither would have failed a load-time check.

1. **`m_ref = 6 * 10 ** (-7)`.** PyBaMM writes the LFP exchange-current coefficient as an
   expression. The extractor's pattern read the leading number: **6**, ten million times the
   rate. It now evaluates the whole right-hand side (numbers and `+ - * / **` only; anything
   else raises).
2. **`t_ref_k`.** The extractor printed the set's `Reference temperature [K]` key, which for
   Prada2013 is **298**. Both exchange-current functions are written against **298.15**, and
   the engine reads `t_ref_k` only in that Arrhenius factor; at 25 °C the key would have
   shifted the positive rate by 0.7 %. It now parses the reference from the function bodies
   and requires the two electrodes to agree.

The LG M50 output is byte-identical before and after both fixes (Chen2020's key and its
functions both say 298.15, and its literals are plain numbers).

## Measured, against the predictions

All on the shipped file, release, 20 particles, σ 0.2, 20 shells, isothermal.

| | prediction | measured |
| --- | --- | --- |
| P1 C/20 span over 20–80 % | 138.4 ± 2 mV | 137.40–138.96 mV over 20 seeds; held |
| P2 C/20 mean offset above PyBaMM | +8…+12 mV | **+9.49…+9.84 mV** over 20 seeds; held |
| P3 298 K rest gap, seeds 9/1/2/3 | 18.0–20.0 mV, discharge-arrival lower | 19.64 mV on all four; held. **Not on every seed** — below |
| P4 263 K, each gap = count table to 0.1 mV | held | 27.44 / 27.44 / 27.44 / 23.32 mV, each equal to the table at the printed digit; held on the four |
| P5 rest gap vs step length (1 / 10 / 60 s) | 298 K within 0.05 mV | identical to the printed digit at both temperatures |
| P6 end-of-step V(I), 20 states × 61 currents | falls at 1 s, 60 s, 15 min; 1 h rises at 10–17 states by 5–7 mV | falls everywhere at 1 s, 60 s, 15 min; 1 h rises at **15** of 20 by up to **6.11 mV**; held |
| P7 cost, 1 s step | 100–130 µs; 0 unsolved | 111–119 µs at C/20; 0 unsolved at C/20, 1 C, 3 C; held |
| P8 C/20 at 10 s vs 1 s | < 0.5 mV | mean offset ≤ 0.05 mV, span ≤ 0.2 mV; held |

### The plateau (criterion 2)

C/20 from full, compared at the reference's own 30 s rows. The cell sits above PyBaMM's SPM by
+9.49…+9.84 mV on average over 20–80 % (worst row +6.1…+16.8), and spans 137.4–139.0 mV
against PyBaMM's 145.4. The band in the test is the 20-seed range widened by its own width on
each side: **+9.14…+10.19 mV** and **135.8…140.5 mV**.

The plan anchored on the spike's +10.3 mV. The two PyBaMM references agree to 0.03 mV over
20–80 % (the spike's Casadi 1e-9 default-grid run against this IDAKLU 1e-10, 200-point one),
so the anchor did not move because of the reference. On batsim's side, slice B's fixture reads
+9.80 mV against the new golden, and the shipped file +9.65. So about 0.16 mV is the shipped
graphite table and window against the fixture's, and the remaining ~0.5 mV is the engine
against the spike's separate toy code. That last part was not isolated further.

**Step length.** At 10 s (one sub-step, the real-time path) the mean offset moves ≤ 0.05 mV
from 1 s and the span ≤ 0.2 mV; 60 s moves neither further. 1 s and 10 s cost the same per
step (~110 µs release, ~360 µs debug), so the test runs at 10 s: 2.6 s a seed in debug.

### The rest gap (criteria 3 and 4)

C/20 to SOC 0.5 from each side, 2 h rest. The gap is set by **how many particles end full** at
both temperatures: every measured gap equals the count table, a resting ensemble of 20
equal-volume particles with k full, solved from the regular solution alone (`count_table` in
the test, independent of the engine).

| T | seeds 9 / 1 / 2 / 3 | over 20 seeds |
| --- | --- | --- |
| 298 K | 19.64 mV × 4 (4 \| 8 full) | 19.64 on 16; 18.43 on 3 (5 \| 8); **14.14 on 1** (seed 17, 4 \| 7) |
| 263 K | 27.44 × 3 (5 \| 8), 23.32 (6 \| 8) | 27.44 on 11 (seed 0 only after 4 h); 23.32 on 4; 17.48 on 4; 13.35 on 1 |

Two things the plan did not have:

* **One room-temperature seed in 20 is outside the criterion's band.** Seed 17's charge arm
  leaves 7 particles full instead of 8, and the gap is 14.14 mV. It is a stable state, not a
  transient: identical at 1 s, 10 s and 60 s steps and after an 8 h rest. The criterion names
  four seeds (the spike's, chosen before this measurement), and on those it holds; the band was
  not widened. The plan's table already said the warm gap is discrete; what is new is that a
  draw can fall 5 mV short of the fitted 20. For slice D, a room-temperature lesson quoting
  "about 20 mV" is quoting the likely draw, not every draw.
* **Two hours is not always long enough in the cold.** Seed 0 at 263 K read 27.18 mV: two
  particles were still inside the spinodal, switching. Given 4 h it reads the table's 27.44.
  So the test checks that no particle is left inside the spinodal before it compares with the
  table; on the four named seeds none is.

The spike read 0.00 mV on seed 9 at 263 K; the engine reads 27.44 (slice B's fixture did too).
No seed of 20 reads zero here. The seeds draw different radii in the engine from the spike's
toy, so its counts were never going to carry over. That is why criterion 4 pins the mechanism
rather than four numbers.

### The long step (criterion 5)

20 states along a C/20 discharge × 61 currents, every probe on the engine's own sub-step rule:

| step | falls with current | largest rise | out of range |
| --- | --- | --- | --- |
| 1 s | 20 / 20 | — | 1 / 1 220 |
| 60 s | 20 / 20 | — | 9 / 1 220 |
| 15 min | 20 / 20 | — | 53 / 1 220 |
| 1 h | **5 / 20** | **6.11 mV** | 8 / 1 220 |

At an hour, 61 neighbouring pairs rise, at currents between −0.83 C and −0.18 C; two on the
discharge side (+0.2 C and +0.4 C, near full) by 0.03 and 0.11 mV. The largest is at SOC 0.65
between −0.267 C and −0.25 C. Slice B read 15 of 20 by up to 6.13 mV on its fixture; the
spike, 13 of 20 by up to 6.6. `CLAUDE.md`'s "≤ 6 mV" now says 6.1 mV and names the test.

The test reaches SOC 0.65 at 10 s steps (2 520 of them) and pins the hour's rise there: 6.72 mV
between −0.26 C and −0.25 C, asserted inside 4.7…8.7 mV. At the same state 15 min falls across
−0.40…−0.10 C in 0.01 C steps. If a change to the cell removes the rise, the test fails, and the
limit in `CLAUDE.md` moves with it.

### Unsolved splits and cost (criteria 6 and 7)

0 unsolved and 0 surfaces out of range at C/20 (four seeds, 1 s and 10 s), 1 C (cut-off at
3 029 s) and 3 C (688 s), 1 s steps. Cost on the shipped file: 111–119 µs per 1 s step at C/20
in release, the same at 10 s; ~0.7 ms at 60 s (six sub-steps).

## Perturbations

Each row edits the shipped file one way, runs `lfp_ensemble.rs` and `ensemble.rs` (21 tests,
counted every run, `--no-fail-fast`), and restores it byte for byte (`pert.py`).

| break | caught by |
| --- | --- |
| `u0_v` 3.42 → 3.43 | the bit pin; the plateau |
| `omega_ev` → Bai's 0.183 | 8: the bit pin, Ω's fit, the plateau, the 298 K gap, the hour, **no unsolved split at 1 C / 3 C**, conservation, the ledger |
| `omega_ev` +1 % | 5: the bit pin, Ω's fit, the plateau, the 298 K gap, the hour |
| `[spm.positive.regular_solution]` removed | 10, including both rest gaps and the model-refusal tests |
| positive `m_ref` 6e-7 → **6** (the extractor's misread) | 8: the bit pin, the plateau, both rest gaps, the hour, the unsolved-split test, conservation, the ledger |
| `t_ref_k` 298.15 → **298** (the set's key) | **the bit pin alone** |
| positive `stoich_max` 0.7035 → 0.69 | the bit pin; both rest gaps |
| positive radius × 2 | the bit pin; the 263 K gap; the radius-draw test |
| one graphite potential +2 mV | **the bit pin alone** (its whole-section fingerprint) |

Two rows are caught only by the bit pin, and that is what it is for: a 0.7 % rate shift at
25 °C and a 2 mV table point both sit inside every physical band here. The first harness run
passed both test binaries to one `cargo test` without `--no-fail-fast`, and four rows read
"ran 12" — the second binary never ran once the first failed; the table above is the rerun.

## Results against the plan's exit criteria

| criterion | slice C |
| --- | --- |
| 1. `N = 1` bit-identical to `Spm` | Held (slice B's tests, unchanged, still pass). |
| 2. Plateau vs PyBaMM | Held: +9.58…+9.74 mV on the four seeds, band +9.14…+10.19 from 20 seeds; span 138.1–139.0 in 135.8–140.5. Anchor +9.65, not +10.3; references agree to 0.03 mV. |
| 3. 298 K rest gap | Held on the four seeds (19.64 mV, discharge lower). 1 of 20 seeds reads 14.14 mV. |
| 4. 263 K rest gap = count table | Held on the four seeds (27.44 × 3, 23.32), and on every seed whose particles finished switching in 2 h (19 of 20). |
| 5. Long steps | Held: falls at 1 s, 60 s, 15 min at all 20 states; the hour's rise pinned. |
| 6. Charge; no unsolved split | Charge conservation on the file via slice B's test (< 1e-12); 0 unsolved at C/20 (10 s, in tree; 1 s, harness), 1 C and 3 C (1 s, in tree). |
| 7. Cost | 111–119 µs per 1 s step (release). |
| 8. Lesson | Slice D. |

## Deliberately not done

* No scenario file and no client change: slice D. The new chemistry reaches no client until a
  scenario names it.
* No `[dfn]` for this file, and no PyBaMM DFN golden for it.
* The ECM sections stay copied placeholders; nothing was fitted for this file that the generic
  one does not already have.
* The ~0.5 mV between the spike's toy and the engine on the plateau offset was not isolated.
