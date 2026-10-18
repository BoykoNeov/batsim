# A `Dfn` cell past its edge, under a held demand

**Status: BUILT 2026-10-18; one finding awaiting the owner** (an already-broken run got far worse, below). Predictions registered before any engine code (1a418dc). Raised by Phase 12 slice B's
exit 5 (`phase-12-slice-b-reaction.md`); the owner chose this fix 2026-10-18 over the split's
per-cell range under current demands, which does not reach this failure.

## What fails, as traced

The mismatched 1S2P `Dfn` pair (one cell at 0.9 capacity), 0 °C, fine grid, plating on: 1C per cell
for one one-hour step, then 4.2 V held at one-hour steps. Every hold step finishes unconverged, 1.7,
0.35 and 0.03 mV short. A copy of the engine printing every pass and every probe
(`W:\temp\claude\phase12-split`) shows why:

* The charge step leaves both cells past an edge of the range a voltage hold keeps a `Dfn` probe in
  (`dfn::current_window`, both electrodes' bulk between the chemistry's empty and full): the small
  cell's graphite reads 1.013 of full; the other's reads 0.978, but its **positive** is past its
  charged limit — lithium plated as metal left the positive without arriving in the graphite. Their
  ranges over the hour are [0.215, 5.04] A and [0.301, 4.70] A: each must *discharge* to get back
  inside.
* The pack's own range for the hold's current is widened to take zero in
  (`pack_current_window`: "a cell past an edge may rest, but not be moved further out"), so the
  hold's pack current, 0.309 A, is allowed. Its split, 0.123 and 0.187 A, is below both cells'
  ranges, so every probe is taken **at the range's edge** (`probe_at`'s `hold`), returns the same
  line, and the next pass's pack current comes out bit for bit the last one. The split damping
  reads that as a fixed pack current, finds no blend that improves, and the stall rule stops the
  solve after two passes. Run as a current demand at the same current, the same step converges in
  four passes.

`probe_at`'s own note already names this case — "a cell resting past an edge, whose own range stops
short of zero" — as the one place `advance` can run where no probe did, reached by no test.

## The change

A held `Dfn` probe is clamped to its range **widened to take zero in**, `[min(lo, 0), max(hi, 0)]` —
the pack's rule, applied per cell. A cell past an edge may then carry any current between rest and
its range: moving back towards inside, never further out. A range that contains zero is unchanged,
so the change is reached only by a held probe of a `Dfn` cell past an edge.

Not the `Spm` or the many-particle cell: their holds keep a particle **surface** inside
`(0, c_max)`, and past that their curve is noise (`spm-pack-window.md`), which is a different
question. Not the range's own assumption that the whole cell current reaches both electrodes'
solids: with plating, the graphite receives less, so the negative's edge is approximate; the
positive's, which is what bound here, is exact.

## Predictions

1. **Exit 5 converges**: the plating pair above, all three one-hour hold steps converged, every value
   finite; at 15 min still every step converged.
2. **The same hold from the same state with the reaction switched off converges too** — the fix is
   the range's, not plating's.
3. **The pair with plating off from the start** (its one-hour charge step is unconverged, the split's
   first-hour failure under a current demand, untouched here): not predicted; reported.
4. **Nothing else moves**: every existing test passes unchanged, and the `dfn-split-nan.md` sweep
   (100 runs) is bit-identical except where a held probe of a cell past an edge is reached — which
   none of its runs does (its holds start at 50 %), so **all 100 identical**.
5. **The new sweep protocol** — each topology charged at 1C per cell for 1 h, then 4.2 V held for 2 h,
   at every step length of the sweep: no new non-finite or broken run, and fewer unconverged steps
   than the parent, the decrease all in the hold.
6. **Plating off stays bit-identical to before Phase 12** on the Phase 12 hash harness's six packs
   (none holds a cell past an edge).

## Results (run 2026-10-18)

* **1, held.** Exit 5 converges: the plating pair's three one-hour hold steps land on 4.200000 V,
  every value finite; 15 min unchanged (14 steps, none unconverged). Pinned in `plating_dfn.rs`.
* **2, missed on one step.** From the same state with the reaction switched off, two hold steps
  converge and the first ends **6 µV short**, unconverged (it was 4.33 V at zero current before).
* **3, reported: worse, not better.** The pair with plating off from the start is broken before its
  hold, at its first one-hour charge step (the split's first-hour failure under a current demand,
  untouched here; both cells overcharged to 5.16 V). Its hold was nonphysical before (−14.5 V by the
  fifth hour) and is more so now (1174 V).
* **4, held.** The `dfn-split-nan.md` sweep's 100 runs are bit-identical.
* **5, held as worded, with one already-broken run made far worse.** The new protocol's 20 runs: 14
  identical, 6 changed, each first at a step the parent flagged unconverged; none non-finite on either.
  Unconverged steps 1264 → 1068 over the sweep, and in the 20 runs' holds **266 → 70**, the charge
  legs unchanged. Four runs' holds go from 5–120 unconverged steps to none. Two remain:
  * `rec-273.15-2p-weak-chghold-60`, 120 → 62: the smaller cell, past full, is given a share a hair
    on the charging side of rest (−0.15 mA); its probe is held at rest, and a 36 µV gap stays against
    the 1 nV tolerance. Every value sensible (4.2000 V); the flag says the step did not close.
  * `fine-273.15-2p-weak-chghold-900`, 8 → 8, **already broken before its hold**: its last charge step
    drove both cells to 5.19 V, `SURFACE_OUT_OF_RANGE`, unconverged, identically on parent and fix.
    The hold then circulates current between them — one charged further past full, outside its
    widened range, while the other discharges — the one exception `probe_at` still names. It ended at
    8190 V and 144 A on the parent and ends at **160 499 V and 1.6e14 A** now: probes that used to be
    pinned now follow the circulation further.
* **6, held.** The Phase 12 hash harness's six packs are bit-identical to the tree before Phase 12.
* The workspace suite: 858 passed, 0 failed.

### The finding put to the owner

Both runs this change made more extreme are past the model's domain before the change acts: an
unconverged current-demand charge overfills both cells first. Holding each cell's **committed**
share under a held demand to its widened range would stop the escalation, but parallel cells share
one node, so clipping one cell's current breaks that node to hide a state the model cannot
describe; it is also close to the option `dfn-split-nan.md` declined. Asked: accept as recorded, or
add such a guard.
