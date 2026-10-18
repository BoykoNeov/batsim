# Phase 12 — lithium plating as a reaction on the `Dfn`, instead of a threshold

**Status: PLANNED 2026-10-18; slice A done; slice B built 2026-10-18. Of its two exit misses, the owner
chose to fix both before C (2026-10-18). **Exit 3 is met**: `plating-hold-pieces.md` cuts a plating
pack's voltage hold into 5 s pieces, the hold leg within 0.10 %. **Exit 5 is not quite**: the one-hour
stall was a held probe's range, fixed by `dfn-hold-past-edge.md` (its one finding accepted by the owner),
but with the hold in pieces exit 5 is finite and on 4.200000 V at one hour, its first hold step flagged by the cells' own solves after the one-hour charge overfilled them (a miss as registered, **accepted by the owner 2026-10-18** as the cell model's limit on overfilled
cells). Slice B is done; slice C built 2026-10-18 (`phase-12-slice-c-health.md`, v30); slice D, the lesson, waits on the cold-hold fix the owner chose first (§C).** Written after the spike (`phase-12-spike.md`), which is the measurement
this plan stands on; where a later slice note contradicts this text, the slice note is the measurement
and wins. When written the repo was at `SNAPSHOT_VERSION` 28 and `WASM_API_VERSION` 10.

This replaces the slice list and exit in `docs/ROADMAP.md` §Phase 12, which were provisional on the
spike. Two of its assumptions did not survive it: there is no PyBaMM reference for the chosen law, and
the slice that put plating on the `Spm` is dropped by the owner.

**Prerequisite:** `dfn-split-nan.md` lands first — **landed 2026-10-18**. A 1S2P `Dfn` pack at one-hour
steps committed NaN into its cells' state; this phase's lesson runs on such a pack, and its convergence
exit could not be read while that was possible.

## The owner's decisions, recorded

Asked during the spike and answered 2026-10-17/18 (`phase-12-spike.md`, stop-point A2):

1. **The law: plating only past the line.** Plating current density at a negative node,
   `i_pl = i0·[exp(−α·f·η) − exp((1 − α)·f·η)]`, `f = F/RT`, `η = φ_s − φ_e` (minus the film's drop
   where the Phase 10 film is on), `i0 = F·k·c_e`. Positive `i_pl` plates; negative strips, and only
   while the node holds reversible plated lithium. A fresh graphite above 0 V plates nothing. PyBaMM
   ships no such form, and the lack of an outside reference is accepted as its cost.
2. **No source for the rate constant `k`: a labelled placeholder, sized against LG's sheet** — the
   charges it allows nearly free, the ones it forbids costly.
3. **A fixed share of plated lithium is dead** (cannot strip back); one labelled number.
4. **`Dfn` only, opt-in.** The `Spm` plates nothing on any allowed charge, so it cannot be sized; it
   keeps the threshold, as the equivalent circuit does. On a `Dfn` cell with plating on,
   `PLATING_RISK` means "plating current above zero" and the loss is the lithium actually lost; the
   dendrite short keeps its seeded roll, its hazard tied to the lithium lost to plating.

## What the spike established, and what the plan may therefore assume

| property | measured | what the plan does with it |
| --- | --- | --- |
| the line is reached | the `Dfn` matches PyBaMM's `φ_s − φ_e` to ~1 mV on a matched grid; 25 °C 1C crosses 0 V | no new physics before plating; the line is the engine's own |
| the law separates the sheet | coldest allowed (0 °C 0.3C) plates 18× less than mildest forbidden, on 40/20/40 | the sizing; re-measured on the grid the constant is sized for (slice A) |
| grid | the recommended 10/5/10 reads the line up to 38 mV high in the cold | plating is exponential in it: the constant names its grid |
| solve | the prototype converges where the control does, and at 1 h where the control fails | plating inside the `Dfn`'s Newton, as a second reaction per negative node |
| stripping | reversible lithium strips back within an hour's rest at 0 °C; the cap holds over an hour | the cap is a kink, as the reversal's edge is |
| ledgers | lithium conserved to the harness's own reading (6.3e-8) | charge closes to rounding |
| **step length** | plated lithium at 15 min ×2.03, at 30 min ×3.60 of the 1 s figure (1C for 30 min) | **slice B's own treatment, with a bar set before the run** |
| heat | a −0.49 J extra residual against a predicted 0.45 J, a tenth of the control's own | consistent, not resolved: booked against its own reaction, resolved by a 100× run |

Not established: any reference for the law, the stripping plateau on a following discharge, the cost of
plating in the engine, and the spike's charges had **no constant-voltage leg**.

## Slices

### A — the reference and the size (Python only, no Rust)

**Done 2026-10-18** (`phase-12-slice-a-reference.md`): `k = 4.8e-11` m/s on the fine grid, sized on the
charge with its hold (1.27× from the constant-current size, under the 1.5× that would have gone to the
owner); the modified PyBaMM model runs at `rtol` 1e-10, so the reference is closed loop. **For slice B's
exit 2:** the reference and the engine differ by 15 % on the allowed cold charge, from a third of a
millivolt in their potentials (not feedback, not the particle grid); on the forbidden charges they agree
to 3 % (8.5 % at −10 °C 2C, all of it plating's feedback). Set the bar on those, or against the engine's
own open loop at small `k`, and say which. **Exit 1 has 1.6 % of room:** the allowed cold charge plates
0.00197 % against the 0.002 % target, and the reference already puts it at 0.00227 %; so exit 1 is read on
the engine, on the fine grid, at 1 s steps (at most the 60 s step, whose bar is 1 %), not at the long
steps whose bars allow 10–20 % more.

Pre-registered like a spike. In the reference venv, from the spike harness's sources (moved into
`tools/reference/` so the result does not depend on `W:\temp\claude`).

1. **Try the law in PyBaMM.** A modified plating submodel: PyBaMM's `"reversible"` plating with its
   stripping exchange current replaced by `i0 = F·k·c_e` on both branches and stripping limited by the
   plated inventory. The hard cap is a kink a DAE solver may refuse; a smoothed cap is acceptable only if
   the slice states its width and shows the engine's cap and it agree to within it. **Stop rule:** if
   it cannot be made to run at `rtol` 1e-10 on the cold charges within the slice, the reference falls
   back to (2) alone, and the exit says so.
2. **The open-loop integral** (A1b's), kept as a cross-check at small `k`, where plating's feedback on
   the potential is negligible.
3. **Size `k`** on the grid the lesson runs on. **Recommended: the fine grid** (`dfn::FINE_*`), where
   A1b measured the 18× separation and which is within ~1 mV of the converged reference; the lesson
   runs on it. The same constant on the recommended grid is measured and recorded (it will plate less
   in the cold), not re-sized. Target as the spike sized it: the coldest allowed charge at about
   0.002 % of capacity per charge.
4. **Size the dead share.** No source; a round placeholder (the prototype used 0.5), labelled, with the
   lesson's numbers stated at that value.
5. **Add a constant-voltage leg**: every sizing charge also run with a 4.2 V hold to a C/20 taper. The
   ranking between forbidden charges in A1b was the protocol's (a fast charge stopped early plated
   less); with the hold it should be the cell's.

Output: a `[dfn.plating]` section draft (`k`, `α` = OKane2022's 0.65, the dead share), each line with
its provenance and, for `k`, its grid and its sizing; a note `phase-12-slice-a-reference.md`.

### B — plating inside the `Dfn` (engine; one snapshot bump, 28 → 29)

**Built 2026-10-18; two exit misses awaiting the owner** (`phase-12-slice-b-reaction.md`): the reaction is in the `Dfn`'s solve, opt-in by
`AgingConfig::plating = Reaction`; plating off is bit-identical; a plating cell's step is marched in
**5 s sub-steps** (`dfn::PLATING_SUBSTEP_S`) with the whole step's tangent carried by a forward
sensitivity, which holds the plating integral within 0.4 % of the 1 s figure at every step length
(the quadrature was measured and refused: it books nothing on a 30-minute step from rest). Exits 1,
2 (against PyBaMM, 2.4 %), 4 and 6 pass. **Recorded misses:** a cold voltage hold at steps past a
minute under-books plating by the hold's own one-current-per-step profile (29 % at 15 min; the
integral is right to 0.4 % on that profile); exit 5's one-hour hold steps finish unconverged, the
pack's voltage solve on the mismatched pair (worse with plating off); and slice A's open-loop
instrument read the electrolyte raw where it goes negative at 2C (fixed, its two 2C rows corrected).

- **State:** reversible and dead plated lithium per negative node, inside `DfnState`. Snapshots are
  positional `bincode`, so `#[serde(default)]` is no substitute for the bump (Phase 10 slice B); the
  opt-in selector lands in the same bump, with its pair test.
- **The selector:** recommended in `AgingConfig`, beside `calendar = Film` — a pack with `aging: None`
  plates nothing, as it grows no film. `Pack::new` refuses it on the ECM, the `Spm` and the many-particle
  cell, and on a chemistry with no `[dfn.plating]`.
- **The reaction**, as the prototype had it: a second current at each negative node sharing that node's
  `η` with intercalation, entering the electrolyte's mass and charge rows and the solid's charge row; the
  particle sees intercalation alone; stripping capped at the node's reversible inventory over the step;
  every transcendental through `crate::math`.
- **The step-length treatment.** The backward-Euler step books the whole step at its end potential,
  which on a charge is the step's lowest, and plating is exponential in it. The fix integrates the
  plating rate along the step rather than at its end — sub-stepping the plating integral, or a quadrature
  of `η` between the step's two ends; the slice measures both and picks. **The sub-step count depends on
  `dt` alone, never on the trial current** (Phase 9's `SUBSTEP_S` rule), so the end-of-step curve the
  pack brackets is one function of current, and the step stays deterministic.
  **Bar, set now:** on the spike's 1C-for-30-min charge, plated lithium at 60 s within 1 % of the 1 s
  figure, at 15 min within 10 %, at 30 min within 20 %; the same with a 4.2 V hold following.
- **Heat:** the plating current's heat is booked against its own reaction — lithium metal at 0 V — not
  at the cell's equilibrium voltage. **The energy ledger is resolved at `k` × 100**, where the spike's
  0.45 J hole becomes tens of joules, against the control's own residual.
- **Bit-identical with plating off:** every golden and every existing test, plus a hash of telemetry and
  snapshots across the Phase 10 slice B harness's runs.

### C — what plating does to the cell's health and its flags

**Built 2026-10-18** (`phase-12-slice-c-health.md`, v30), with the owner's decision there: the
dendrite short gets its own placeholder per dead amp-hour (0.7). Dead lithium is read off the
electrodes as capacity, once, and grows the resistance at the threshold's ratio; reversible metal is
stored charge in `soc_true` (chosen on measurement); `PLATING_RISK` is "plated this step". The
stripping step after a cold constant-current leg is 4.3 mV for about a minute (registered: under
2 mV). **For slice D:** a cold 4.2 V hold leaves the `Dfn` unable to converge the discharge after it
(older than this slice), and at 0 °C the hold strips back nearly all the metal (1.2 % of the dead
store is left; 47 % at −10 °C). **Owner decision 2026-10-18: fix the cold hold first**, as its own note with predictions
registered before the change (as slice B's two misses were); slice D starts after it.

- **Capacity read off the electrodes**: dead lithium is gone from the negative's inventory, as the film's
  is. Reversible plated lithium is lithium that will strip back; what `soc_true` and `soh_capacity` read
  while it sits as metal is the slice's decision, measured on the rest after a cold charge, and stated.
- **`PLATING_RISK`** on a reaction cell is "plating current above zero this step"; the threshold's
  `plating_fade_per_ah` is not applied there (it would bill the loss twice).
- **The dendrite short**: the existing seeded hazard, per amp-hour of **dead** lithium instead of per
  amp-hour charged under the threshold's conditions. Same RNG, same draw order on every other pack.
- **The stripping plateau.** A prediction registered before the run: whether a discharge straight after
  a cold charge shows the short voltage plateau of plated lithium stripping (the signature labs detect
  plating by, and the roadmap's most teachable one), at what rate and temperature.

### D — the lesson

The same cold fast charge on the threshold cell and on the reaction cell, in the ninth chapter or a new
one (`path-chapters.md`): where the threshold says "no" at 0 °C and the reaction plates; the sheet's
gentle charge nearly free; lithium left on the graphite stripping back on the rest; the dead share as
lost capacity. On the fine grid, by its toggle. The guided-path ledger rules apply as for every lesson.

## Exit

Pinned by `crates/sim-data/tests/plating_dfn.rs`:

1. **No threshold in the code path**: on a reaction cell, plated lithium comes from the reaction alone.
   On the sized grid, every charge LG's sheet allows plates under the sizing target, and every charge it
   forbids plates at least 10× more (A1b measured 18×).
2. **The implementation matches its reference** — the modified PyBaMM model if slice A built it, within
   bars set before the run; otherwise the open-loop integral at small `k`, and the engine's own result
   converging as `dt` shrinks. Either way this checks the code against the law, **not the law against a
   cell**: the exit says so.
3. **The step-length bar** of slice B.
4. **The ledgers**: lithium (intercalated + reversible + dead) against the charge passed to rounding;
   the energy ledger closed at `k` × 100 to within the control's own residual.
5. **Convergence**: the 1S2P mismatched pack with plating on, a 1C charge and a 4.2 V hold, at 15 min
   and 1 h steps — converged at 15 min; at 1 h, finite on every step and converged on all but a number
   stated before the run (the prototype had one).
   **At risk** (`dfn-split-nan.md`, 2026-10-18): without plating this pack's split does not settle at
   one-hour steps even in its first hour, before anything is full (ROADMAP H8). The split is damped
   since that slice, and the first hour still does not settle (−3.6 and −6.7 A at 25 °C, the same
   split as before it); the slice's sweep has every 1C charge and every 4.2 V hold on it finite and
   bounded, and every hold converged. The prototype converged where the control did not, but on a
   different engine; if exit 5's one-hour bar fails for that reason, the fix is the split's, and the
   plan says so before slice B rather than after.
6. **LTO plates nothing**, analytically: `lto_20ah_generic.toml` has no porous model, and a graphite-free
   negative 1.55 V above lithium needs 1.55 V of polarization to reach the line; a test pins the
   refusal of plating on a chemistry with no `[dfn.plating]`.
7. **Plating off is bit-identical**, and the 28 → 29 snapshot pair test passes.

## Two more owner decisions — answered 2026-10-18, both as recommended

1. **The placeholder list grows.** **Answered: the rule is amended** to "may grow only by a phase plan
   that names the key and the owner decision behind it". The slice that adds the keys to
   `chemistries/nmc_21700_lgm50.toml` amends the rule's text in `lgm50_provenance.rs` and adds them to its
   list, naming this plan.
2. **The grid.** **Answered: the fine grid.** Slice A sizes `k` on `dfn::FINE_*`, records the recommended
   grid's figure beside it, and the lesson runs on the fine grid.

As asked:

1. **The placeholder list grows.** `crates/sim-data/tests/lgm50_provenance.rs` holds the LG M50 file's
   placeholders to a fixed list that "may only shrink" (Phase 11 exit 3). The plating rate constant and
   the dead share are new placeholders by decision 2 and 3, so the list grows by two (three if `α`,
   from a set that "does not claim to be representative", is counted). Recommended: amend the rule to
   "may grow only by a phase plan that names the key and the owner decision behind it".
2. **The grid the constant is sized on** (slice A, step 3). Recommended: the fine grid, which the lesson
   then uses; the alternative is the recommended grid, cheaper in the browser, but further from the
   physics in the cold by up to 38 mV, where plating is exponential.

## The spike's instruments

**Deleted 2026-10-18 with slice B.** `W:\temp\claude\phase12-spike\` was kept until slice A had moved what it needed into `tools/reference/`
(the PyBaMM cold-charge scripts and the A1b integrator) and slice B has rebuilt the prototype's reaction
in the tree (`proto\patch_dfn.py` is its only record). It is deleted then, and the slice note says so.
