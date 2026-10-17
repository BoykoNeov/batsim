# The `Dfn`'s fine grid: a converged 3C golden, and a toggle in the client

**Status: PLANNED 2026-10-17.** Owner's decisions closing Phase 11 (`phase-11-fitting.md`, last
addenda): the opt-in fine grid (`sim_core::dfn::FINE_*`, 40/20/40 with 40 shells) stays, as a
toggle with a note in the client; the 25 °C 3C golden, made on PyBaMM's unconverged default
x-grid, is regenerated now, "in the same style" as the cold one — the bar committed before the
run that judges it.

## Part 1 — the 3C golden

**What is known before the bar.** Rerun at 25 °C 3C on finer x-grids (2026-10-17, uncommitted
scratch), PyBaMM's own cut-off moves 488.65 → 494.85 → 496.33 s on 20/20/20, 40/40/40, 80/40/80,
and its 20/20/20 voltage is up to 88.7 mV off the 80/40/80 one above 2.75 V. The engine at the
recommended 10/5/10 cuts off at 463.4 s; at 20/10/20 488.2, 40/20/40 494.8, 80/40/80 497.7. Seen
too, against that uncommitted 80/40/80 trace sampled every 5 s and above 2.75 V only: the fine
grid (40/20/40, 40 shells) within 10.85 mV worst (at 4 s), 2.67 mV RMS. The whole run, the knee
included, has not been compared on any grid against a converged 3C reference.

**The regeneration.** `common.Reference` gains an optional `x_pts`; a new `DFN_CONVERGED_X`
(`DFN_CONVERGED` plus `x_pts = (160, 80, 160)`, the cold golden's grid) is used by the 3C row
only, and the CSV header names it. `DFN_CONVERGED` is untouched, so every other golden
`generate.py` writes must come out byte-identical — checked by regenerating the whole LG M50
set and diffing. The reference's own uncertainty is stated as its 80/40/80 → 160/80/160
difference.

**The bars, set here before the judging run.** Each existing `dfn_golden.rs` assertion that
reads the 3C golden keeps its bar **in every word that judges**; where a bar was written as a
fraction of the reference, the fraction is kept and the reference is the new one:

1. `dfn_cc_3c_tracks_the_reference_into_depletion` — recommended grid, worst voltage error
   **< 100 mV** over the first 90 % of the reference's run.
2. `dfn_cc_3c_lands_on_the_references_cut_off` — recommended grid, own cut-off within
   **±12 %** of the reference's cut-off time, and delivered A·h within **±12 %** of the
   reference's (the old literals, 430–548 s and 1.85–2.35 A·h, were ±12 % of 488.7 s and
   2.098 A·h, and are re-centred, not widened).
3. `the_spm_cannot_follow_the_reference_into_depletion` — SPM error **> 4×** the DFN's and
   **> 200 mV**.
4. `refining_the_x_grid_converges_toward_the_reference` — 20/10/20 **closer** than 10/5/10, on
   the cut-off and on the in-window error.
5. **New**, the fine grid at 3C — the cold test's bar, unchanged: **12 mV worst over the whole
   replay** (first step included), own cut-off within **1 %** of the reference's.

A bar that fails is reported as failed and recorded, as exit 5's was; no grid or window is
changed to pass. Numbers in the tests' comments (62.1 mV, 464 s, 5.1 %, 521 mV, 8.4×) are
records, re-measured after the judging run.

## Part 2 — the toggle

The browser page's **BMS enabled** checkbox rebuilds the pack from the scenario with or without
its BMS, and has a note under it. The fine grid gets the same: a checkbox under it, shown only
when the loaded scenario's cell is a `Dfn`, which rebuilds the pack with `FINE_*` in place of
the scenario's grid, and a note saying what it buys (cold and high-rate accuracy) and what it
costs (about six times per step). **Off by default, and turned off by loading any scenario or
any guided-path step**, so no lesson's numbers can be read on a grid its prose was not measured
on. The socket backend cannot rebuild a session (as for the BMS), so the box is disabled there.
Engine side: `SimEngine` holds the flag beside `bms_enabled` and rebuilds through the same path;
`WASM_API_VERSION` bumps, `SNAPSHOT_VERSION` does not (the grid is already in the state).

## Part 3 — the 3C lesson says which grid it is on

The guided-path step that loads `cc_discharge_3c_dfn.toml` quotes the engine on the recommended
grid — "2.422 V at t = 464 s, past the 2.50 V cut-off" — and presents that as the cell's death.
True of the engine as configured, and the step's point (the porous model empties ~590 s before
its single-particle twin at the same current) survives refinement; but a converged model cuts
off near 496 s, about 7 % later. The step gains a sentence saying its numbers are the coarse
recommended grid's and inviting the reader to tick the fine-grid box and run it again. Any number
that sentence quotes is measured on the fine grid in the engine and held by the path's claim
ledger like every other (`path_claims.rs`); none is quoted from PyBaMM or from this note. The
toggle still resets to off on loading the step, so the step's existing numbers stay the ones
its prose was measured on.
