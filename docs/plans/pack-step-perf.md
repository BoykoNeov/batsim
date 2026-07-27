# `Pack::step` performance — over budget at 100S10P

**Status:** open. Baseline measured and committed; no optimization applied yet.
**Baseline commit:** `5917bd9` ("Phase 1 wrap-up: criterion benchmarks for Pack::step").
**Owner decision needed:** none for items 1–2 (bit-identical); item 3 needs a design call.

## The problem

`CLAUDE.md` sets a budget of **< 50 µs per `Pack::step` at 100S10P** (1000 cells) on a
laptop, and says "it should be far below." The committed benchmark measures **83.4 µs** —
1.7× over, not a marginal miss.

Measured on the dev laptop (Windows, `release` + `lto = true, codegen-units = 1`),
`cargo bench -p sim-core`:

| case                  | time    |
| --------------------- | ------- |
| 1S1P / current        | 200 ns  |
| 10S10P / current      | 8.32 µs |
| 100S10P / current     | 83.4 µs |
| 100S10P / power       | 83.4 µs |

## What the baseline already establishes (do not re-derive)

- **The work is real.** 10S10P → 100S10P is an exact 10×, so the solve is O(cells) at
  ~83 ns/cell and the optimiser did not eliminate `step`'s end-of-step reporting pass.
  The bench `black_box`es the returned `Telemetry` specifically to prevent that.
- **1S1P is not the per-cell cost.** At one cell the per-step fixed overhead (the solve's
  scratch `Vec`s) dominates; 200 ns is a floor, not a divisor.
- **`Power` is not a cost driver.** It matches `Current` exactly — `solve_current` is a
  closed-form quadratic in Phase 1, not the Newton loop `CLAUDE.md` sketches.
- **The OCV table walk is a minor contributor.** Re-running with the bench's `SOC` const
  at `0.01` instead of `0.6` shortens `interp1`'s linear scan from ~18 iterations to ~4
  while leaving allocation count and `exp()` untouched: **83.4 → 74.7 µs, −8.8 %**.
  So ~75 µs is elsewhere.
- **Per cell per step the engine pays:** 2 × `r0_lookup` (2 heap allocations),
  2 × `ocv_lookup` (linear scan), 1 × `exp()` in `rc_update`. The ×2 is because
  `cell_source` is called once in the start-of-step aggregation loop (`pack.rs:370`) and
  again in the end-of-step reporting loop (`pack.rs:414`). At 100S10P that is **2000 heap
  allocations per step**, the leading remaining suspect.

## Candidate fixes, cheapest first

### 1. Remove the per-call `Vec` in `r0_lookup` — *bit-identical*

`ecm.rs:110` builds `per_row: Vec<f64>` by interpolating *every* SOC row over temperature,
then interpolates across rows. Instead: bracket the two SOC rows first, interpolate only
those two over temperature, then blend. That is 2 `interp1` calls plus one lerp, no
allocation.

- **Expected:** the bulk of the ~75 µs, if the allocation hypothesis holds. Unverified —
  measure, don't assume.
- **Determinism risk: none.** The same FP operations run on the same values in the same
  order, including the clamped-end branches, so goldens, snapshot replay, and the property
  tests cannot shift. Confirm by running them, not by argument.
- **Watch:** preserve the clamp behaviour at both ends of *both* axes, which the current
  code gets from `interp1` twice over. A hand-rolled bracket is where that regresses.

### 2. Binary-search (or cached segment index) in `interp1` — *bit-identical*

`ecm.rs:87` scans breakpoints linearly from the low end, so cost grows with where SOC sits
in the table. Worth ~9 % on the 34-point LFP table at mid-SOC, more on denser tables
(a fitted `R0`/OCV set will be denser than the current placeholder).

- A cached "last segment" index is faster still for the real access pattern (SOC moves
  slowly between steps) but adds mutable state to a currently-pure lookup — it would have
  to live in the cell state and therefore in the snapshot. Prefer plain binary search
  unless the measurement says otherwise.
- **Determinism risk: none** — same interpolation, same values, only the search differs.

### 3. Cache per-cell `(E, R)` across the step boundary — *structural, biggest lever*

The end-of-step `cell_source` call computes exactly what the *next* step's start-of-step
call recomputes from unchanged state. Caching `(e, r)` per cell halves the per-cell work
regardless of which micro-cost above wins.

- **Cost:** the cache becomes part of pack state, so it must be invalidated wherever state
  changes outside `step` — at minimum `set_cell_factors`, `restore`, and anything Phase 2+
  adds that mutates a cell (thermal update, balancing bleed, injected faults). A stale
  entry is a silent physics bug, not a crash.
- **Snapshot impact:** if the cache is serialized, `SNAPSHOT_VERSION` must be bumped
  (currently 2). Prefer making it `#[serde(skip)]` and recomputing on `restore` — that
  keeps the snapshot layout and the "restore reproduces the trajectory exactly" guarantee
  trivially intact.
- **Do this only if 1–2 leave us over budget.** It trades a determinism-relevant
  invariant for speed, and Phase 2 is about to add more per-cell mutation points.

## Verification protocol for any of these

1. `cargo bench -p sim-core -- --save-baseline before` on `main` first, then
   `--baseline before` after the change — criterion reports the delta directly.
2. `cargo test --workspace` — the PyBaMM goldens, analytic golden, snapshot replay, and
   proptest invariants must all pass **unchanged**. For 1 and 2 the claim is stronger than
   "within tolerance": the outputs should be bit-identical, so any golden movement at all
   means the rewrite is not what it claims to be.
3. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` clean
   (the gate now compiles the bench too).
4. Update the measured table in `benches/pack_step.rs`'s module docs and in this file.

## Note for whoever picks this up

The 50 µs budget is deliberately **not** asserted in a test — a wall-clock assertion is
machine- and CI-dependent, and `CLAUDE.md` frames it as a budget to keep, not an exit
criterion. Track it by running the bench, not by a gate.
