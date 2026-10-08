# Phase 9, slice B — the many-particle cell in the engine

**Status: BUILT 2026-10-08.** Plan: `phase-9-lfp-ensemble.md` §B. `SNAPSHOT_VERSION` 24 → 25;
`WASM_API_VERSION` stays 8. Harnesses and raw output outside the repo under
`W:/temp/claude/phase9b/` (`fp/` the bit-for-bit fingerprints, `ens/` the many-particle
measurements, `pert.py` the perturbation table, `substep_sweep.py` the sub-step sweep).

## What was built

`CellModelConfig::SpmEnsemble { shells, particles, radius_sigma }` and
`CellModel::SpmEnsemble(Box<EnsembleState>)`, in `crates/sim-core/src/ensemble.rs`. The
single-particle model with its **positive** electrode split into `N` particles of equal
active volume and lognormal radii (drawn once from the pack's seed); the negative electrode
stays one particle. Every building block — shells, backward-Euler diffusion, Butler–Volmer,
the reversal past empty, the end-of-step curve — is `spm.rs`'s, called, not copied; `spm.rs`
changed only in visibility and in one function taking the deficit as a number
(`split_from`).

`[spm.positive.regular_solution] u0_v, omega_ev` is the second form of the positive
potential, `U(y) = U0 − (RT/F)·ln(y/(1−y)) − Ω·(1−2y)` at the cell's own temperature. Only the
ensemble reads it (instead of the table, which stays required); `Pack::new` refuses it under
`Spm` and `Dfn` (`BuildError::RegularSolutionNeedsEnsemble`), validation refuses it on the
negative electrode.

### The split, inside the cell

Each (sub-)step divides the particles' current among them so every particle sits at one
potential and the shares sum to the whole. **Two halves**:

* **Tangent passes** — the pack's parallel-group solve on the particles' tangents, from the
  last step's shares. Two or three passes in ordinary use; a pass preserves the sum.
* **A bracketed search**, when the passes fail to settle in eight or start growing: a sign
  bracket on the shared potential, and inside it a sign bracket on each particle's share.
  Each particle's potential falls without bound in its current (the kinetics' `asinh`), so
  both searches have a root and neither can leave its bracket.

The second half exists because of a measurement, not a plan. The first build had only the
passes, and a 60 s, −2.3 C to −3 C charge from 95 % SOC (8 of 1 220 probes in the 60 s
monotonicity sweep) ran them to **±millions of amperes per particle** that still summed to
the demand — charge conserved, state destroyed, `SOLVE_UNCONVERGED` the only sign. With the
fallback: 0 unconverged; the fallback ran 98 times in that sweep, converged every time, and
made the sweep faster (12.0 s against 18.4 s), because the diverging passes had been burning
their cap. It ran **0 times** in the C/20, 1 C and 3 C discharges.

The pack's model check for seeding its first pass (`matches!(…, CellModel::Spm(_))`) became
`CellModel::seeds_first_pass()` — the one edit to `pack.rs`'s step, against the plan's "the
pack is not edited", made because without it the new model would have been left out of the
seeding that stops rounding growth at long rests.

### Sub-steps

`substeps(n, dt)`: `1` for one particle, else `ceil(dt / SUBSTEP_S)` capped at
`MAX_SUBSTEPS`; a function of the step and the particle count alone. The probe and the step
march the same sub-steps from the same seeds, so the curve the pack converges on is the step
the cell takes, bit for bit (`the_probe_and_the_step_agree_bit_for_bit`).

**`SUBSTEP_S` = 10 s, on measurement.** The plan asked for the fast-forward sub-step length
to be measured here, and for any length coarser than the spike's 1 s to be re-measured at
15 min. Sweep (`substep_sweep.py`, the fixture, 20 particles, seed 9): at each length, the
end-of-step voltage against current at 20 states × 61 currents, and a C/20 discharge at the
long step against the 1 s trajectory plus the rest gap that long step reaches.

| sub-step | 15 min: falls with current | worst drift from 1 s (60 s / 15 min steps) | rest gap | per 15 min step | bracketed-split calls in the sweep |
| --- | --- | --- | --- | --- | --- |
| 1 s | 20 / 20 | 0 / 0 | 19.64 mV | 93 ms | 1 141 |
| 4 s | 20 / 20 | 2.45 / 0.92 mV | 19.64 mV | 24 ms | 350 |
| **10 s** | **20 / 20** | **3.00 / 2.23 mV** | 19.64 mV | **10 ms** | 210 |
| 30 s | 20 / 20 | 3.04 / 2.68 mV | 19.64 mV | 3.5 ms | 6 750 |
| 60 s | **16 / 20** (≤ 0.23 mV rise) | 3.04 / 2.69 mV | 19.64 mV | 11 ms | 514 502 |

Between 30 s and 60 s a particle's potential starts rising within a sub-step: the split
leans on its bracketed half thirty-fold more, then the voltage stops falling with current.
10 s keeps a factor of three from that, costs a ninth of 1 s, and drifts at most 3 mV from
it; the rest gap does not move at any length. A step of 10 s or less is one sub-step, so the
real-time path is the same whatever this constant is. The drift saturating near 3 mV from
10 s up says it is the coarser backward-Euler split itself, not a sub-step artefact.

Re-measured at the chosen length, 20 states × 61 currents (`mono_s10.txt`):

| step | falls with current | largest rise | probes out of range | per step |
| --- | --- | --- | --- | --- |
| 1 s | 20 / 20 | — | 1 / 1 220 | 0.16 ms |
| 60 s | 20 / 20 | — | 9 / 1 220 | 1.4 ms |
| 15 min | **20 / 20** | — | 53 / 1 220 | 19 ms |
| 1 h | **5 / 20** | **6.13 mV** | 8 / 1 220 | 102 ms |

The hour is the documented limit of `phase-9-lfp-ensemble.md` §"Long steps" reproduced in
the engine: the spike read 13 of 20 states rising by up to 6.6 mV at 4 s sub-steps; the
engine at 10 s reads 15 of 20 by up to 6.13 mV. Steps up to 15 min hold. (Per-step times here
are pack steps under a current demand including the solve's probes, so they are not the
same quantity as the sweep's single-trajectory column.)

### What the pack holds a demand to

The negative particle's surface window at the end of the step, intersected with both
electrodes' **bulk** windows (the positive's is the mean over its particles — conservation
alone). The positive particles' surfaces are **not** in it for `N > 1`: each depends on its
own share, which the split decides, and the even split that would give an interval is wrong
exactly when it matters (a full particle refuses the current an even split hands it). They
are flagged after the step (`SURFACE_OUT_OF_RANGE`) instead — the `Dfn`'s rule. For `N = 1`
the positive surface stays in, as the `Spm`'s does. Probes out of range: 1 / 1 220 at 1 s,
9 / 1 220 at 60 s, 54 / 1 220 at 15 min (round 2's harness: 228 / 1 220 at 15 min).

### Heat

`i·(U_eq − V)` with `U_eq` the particles' mean bulk potential minus the negative's, plus the
**exchange heat** `Σ x_k·(U_k − Ū)` the particles make trading lithium among themselves,
folded into the watts the pack adds unmultiplied (`Advanced::reversal_w`). At rest it is the
whole of the heat. Measured on the fixture, 20 particles, C/2 for an hour then 2 h rest: the
ledger (stored energy from the shells, closed-form regular-solution integral) closes to
**0.0035 J of 13 723 J**, with **9.30 J** booked during the rest; without the term the rest's
9.3 J is missing. One particle: 0.067 J — the `Spm`'s own intra-particle relaxation, which
it does not book at zero current.

## Results against the plan's exit criteria

| criterion | slice B result |
| --- | --- |
| 1. `N = 1` bit-identical to `Spm` | **Held.** 13 878 / 13 878 fingerprint lines (every demand class at 1 s / 60 s / 1 h, through empty, past full, holds, 1S1P–4S2P, zero-length reads, snapshot restore, 10 and 20 shells); in tree, `one_particle_is_the_single_particle_model_bit_for_bit` and the two shipped `Spm` scenarios. And the old models themselves did not move: 15 842 / 15 842 lines of `Spm` and `Dfn` fingerprints identical before and after. |
| 2. Plateau (C/20) | Fixture, not the slice-C chemistry: 20–80 % 3.1727…3.3111 V, span **138.44 mV** (spike: 3.1728…3.3111, 138.30). The PyBaMM golden is slice C's. |
| 3. Rest gap, 298 K | Fixture, four seeds: **19.64 mV** on all four (spike: 19.64 / 19.64 / 18.43 / 18.43); discharge-arrival lower on every seed. |
| 4. Rest gap, 263 K | Fixture, four seeds: 27.44 / 27.44 / 27.44 / 23.32 mV — every one a value of the spike's set (0 / 27.44 / 23.32 / 13.35). |
| 5. Long steps | Falls with current at 1 s, 60 s and 15 min at all 20 states at the chosen 10 s sub-step; at 1 h rises at 15 / 20 by ≤ 6.13 mV — the documented limit. The test pinning the hour against the real chemistry is slice C's. |
| 6. Charge; no unsolved split C/20–3C at 1 s | Charge conserved to < 1e-12 of capacity through discharge, rest and charge (`the_particles_conserve_lithium`); 0 unsolved splits and 0 surface flags at C/20, 1 C and 3 C. |
| 7. Cost | 1 s steps, 20 particles, 20 shells, 1S1P, release: **100–125 µs per step** (C/20 100, 1 C 126, 3 C 116), ~100× an `Spm` step. Long steps: ≈ 10 ms per 15 min on one trajectory, ≈ 0.1 s per hour step with the pack's probes. |
| 8. Lesson | Slice D. |

The 1 C and 3 C discharges agree with the spike's too (1 C 20–80 %: 2.6670…3.2141 V against
2.6649…3.2140); the port was faithful.

## Perturbations

Each row breaks one thing in `ensemble.rs` and runs both test sets (`pert.py`):

| break | caught by |
| --- | --- |
| no bracketed fallback | `a_hard_charge_into_a_full_electrode_keeps_its_particles_bounded` |
| no exchange heat | `the_energy_ledger_closes_through_a_rest` |
| no exact-sum fix of the last share | `the_particles_agree_on_their_potential` |
| the step seeds its split from zero, the probe from the last shares | `the_probe_and_the_step_agree_bit_for_bit` |
| radii drawn for one particle | `one_particle_is_the_single_particle_model_bit_for_bit`, `the_radius_draw_is_seeded_and_moves_no_other_draw` |
| split tolerance 1e6× looser | `the_particles_agree_on_their_potential` |

Two findings from the first round of this table. A special case written so that one
particle's kinetics carry exactly `i` broke nothing when removed: the general expression
`i·a − (i_p·a − x)` already is `i − 0` there, bit for bit, so it was deleted. And the first
harness passed both test binaries to one `cargo test` with a name filter that silently
dropped every integration test — a "no exchange heat" row reading green on a run that never
ran the ledger. The table above is the second harness, which counts the tests each run ran.

## The snapshot bump, and a prediction that failed

v25: `ElectrodeParams::regular_solution` (an `Option` closing each electrode), and the new
model as the last variant of `CellModel` and of `CellModelConfig`. Only a chemistry with an
`[spm]` section changes bytes. The stale-blob test was written predicting that a v24 `[spm]`
section would fail at v25 for the shipped radius (its first byte, `0xf7`, is read as the new
field's tag) and **parse quietly** for a radius whose first byte is zero. The second half was
wrong: read a byte out of step, the positive electrode's potential table gets a length near
`2^57` and the read fails there too. The test and the version note say what was measured
(`snapshot_version.rs::a_v24_shaped_spm_section_fails_by_its_radius_or_its_table_length`).

Full suite: 764 tests, 763 passed on the first run; the one failure was that prediction.

## Deliberately not done

* The LFP chemistry file, its PyBaMM goldens, and the long-step test against it — slice C.
  Everything above with more than one particle runs on a **fixture** built in the tests from
  the spike's parameters.
* A particle-size distribution on the negative electrode, a non-monotone negative potential,
  the `Dfn`.
* Exchange heat past empty for `N > 1` is booked as the single-particle model books the
  reversal plus the particles' exchange; the reversal's kinetic current is spread over the
  particles by area. Not measured against a ledger past empty — an LFP cell runs out on its
  graphite first, so the negative particle's edges carry that branch.
