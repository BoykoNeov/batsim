# `Pack::step` performance — over budget at 100S10P

**Status:** items 1 and 2 landed; still over budget, item 3 still deferred. All of
Phase 2 added ~4 % to the electrical baseline, and ~5 % more with every feature
switched on — measured, see "Phase 2 impact" below.
**Baseline commit:** `5917bd9` ("Phase 1 wrap-up: criterion benchmarks for Pack::step").
**Owner decision needed:** none outstanding. Item 3 needs a design call *if* it is ever
picked up, and the case for deferring it got **stronger**, not weaker — see below.

## The problem

`CLAUDE.md` sets a budget of **< 50 µs per `Pack::step` at 100S10P** (1000 cells) on a
laptop, and says "it should be far below." The original benchmark measured **83.4 µs** —
1.7× over, not a marginal miss.

Measured on the dev laptop (Windows, `release` + `lto = true, codegen-units = 1`),
`cargo bench -p sim-core --bench pack_step`:

| case                  | before  | after   |
| --------------------- | ------- | ------- |
| 1S1P / current        | 219 ns  | 179 ns  |
| 10S10P / current      | 8.32 µs | 6.22 µs |
| 100S10P / current     | 85.9 µs | 61.5 µs |
| 100S10P / power       | ~86 µs  | 61.8 µs |

"after" is items 1 and 2 below: **−28.5 %** at 100S10P, which moves the miss from 1.7×
over budget to **1.23×**. Read the measurement caveat before adding a row to this table —
the "before" column mixes two sessions on purpose (see below).

## What the baseline already establishes (do not re-derive)

- **The work is real.** 10S10P → 100S10P is an exact 10×, so the solve is O(cells) at
  ~83 ns/cell and the optimiser did not eliminate `step`'s end-of-step reporting pass.
  The bench `black_box`es the returned `Telemetry` specifically to prevent that.
- **1S1P is not the per-cell cost.** At one cell the per-step fixed overhead (the solve's
  scratch `Vec`s) dominates; 200 ns is a floor, not a divisor.
- **`Power` is not a cost driver.** It matches `Current` exactly — `solve_current` is a
  closed-form quadratic in Phase 1, not the Newton loop `CLAUDE.md` sketches.
- **The OCV table walk was a minor contributor.** Re-running with the bench's `SOC` const
  at `0.01` instead of `0.6` shortened `interp1`'s linear scan from ~18 iterations to ~4
  while leaving allocation count and `exp()` untouched: **83.4 → 74.7 µs, −8.8 %**.
  Moot now — the scan is a binary search, so SOC position no longer affects cost. Do not
  re-run this probe expecting it to mean anything.
- **Per cell per step the engine pays:** 2 × `r0_lookup`, 2 × `ocv_lookup`, 1 × `exp()` in
  `rc_update`. The ×2 is because `cell_source` is called once in the start-of-step
  aggregation loop (`pack.rs:370`) and again in the end-of-step reporting loop
  (`pack.rs:414`) — this is what item 3 would halve, and it is still there.
- **The 2000-allocations-per-step suspicion was correct.** `r0_lookup` allocated twice per
  cell per step; removing that (with item 2) bought −28.5 %. What remains at ~61 ns/cell is
  the doubled `cell_source`, roughly a dozen dependent `f64` divisions per cell, and one
  `exp()` — none of which has been separately attributed. **Nobody has profiled this**; the
  division count is arithmetic from reading the code, not a measurement. Treat it as the
  next hypothesis to test, not as an established fact.

## Measuring a change on this laptop — read this first

The single most expensive mistake available here is trusting a criterion baseline saved in
an earlier session. This machine is **bimodal**: the same binary on the same code lands in
one of two CPU states about **1.4× apart**, and it can flip *between* two arms of a single
script. Observed for `100S10P/current`: the pre-change tree measured 85.6 / 86.3 / 90.4 µs
in one state and 120.2 / 121.2 µs in the other; the post-change tree measured 61.3 / 61.8 /
62.4 / 62.5 µs and 82.8 / 83.2 µs. Mode-matched the ratio is stable (62/86 = −28 %,
83/120.5 = −31 %); cross-mode pairings range from −25 % to −48 % and mean nothing.

This is not theoretical. A cross-session `--baseline` comparison of exactly the change
below reported **−8.6 %**; the paired measurement puts it at **−28.5 %**. The first number
nearly got the allocation hypothesis written off as refuted.

So: run both revisions back to back (`git stash push -- crates/sim-core/src/ecm.rs`,
bench, `git stash pop`, bench), repeat until two rounds agree, and **discard any round
where either arm's confidence interval is wide** — a wide CI is the signature of a machine
in transition, not of sampling error. Wall-clock cost is a few minutes; the alternative is
a wrong conclusion about your own change.

Also note `cargo bench -p sim-core -- --save-baseline x` **fails outright**
(`Unrecognized option`): the lib's default bench harness sees the flag first. Use
`cargo bench -p sim-core --bench pack_step -- ...`.

## Phase 2 impact, end of phase — **+4 % baseline, +5 % more with everything on**

Re-measured against `79e0c87` (pre-Phase-2) after all five slices landed, same
worktree-paired protocol, three pairs:

| pair | pre | post | ratio |
| ---- | --- | ---- | ----- |
| 1 | 99.8 µs | 101.2 µs | +1.3 % |
| 2 | 98.0 µs | 100.3 µs | +2.3 % |
| 3 | 94.7 µs | 103.5 µs | +9.3 % |

Slow state again throughout (pre readings 94–100 µs), and the ratio spread is wide
enough that the honest summary is **~4 %, somewhere in 1–9 %** rather than a point
estimate. It is consistent with the slice-A measurement of ~6 %: slices B–D add
essentially nothing to *this* benchmark, because it runs `bms: None` and `Isothermal`,
where the only new work is an empty-`Vec` lookup and an `is_some()` per group.

What a client that actually turns the features on pays is a separate question, and the
new `100S10P/full` case answers it — thermal network, sensors, estimator, protection,
and balancing all live. Both cases run in the same invocation, so this comparison needs
no pairing:

| round | `current` (baseline) | `full` (everything on) | ratio |
| ----- | -------------------- | ---------------------- | ----- |
| 1 | 105.6 µs | 112.6 µs | +6.6 % |
| 2 | 104.9 µs | 108.9 µs | +3.8 % |

So the whole Phase 2 feature set costs about **5 %** on top of the electrical solve.
That is lower than it might look like it should be, and the reason is structural: the
thermal integration is a handful of flops per cell with no table lookups, and every
piece of BMS work is O(groups) or O(1), not O(cells). Scaling the fast-state anchor
through both factors puts a fully-featured 100S10P step at roughly **67 µs, ~1.34×
over** the 50 µs budget.

## Phase 2 impact — thermal (slice A, commit `9bc4656`): **+6 %**

Measured against `79e0c87` (the tree with items 1–2) using two git worktrees so
alternating runs pay no rebuild, filtered to `100S10P/current` so each arm takes ~10 s
and a mode flip cannot straddle a pair:

| pair | pre (79e0c87) | post (thermal) | ratio | CI width |
| ---- | ------------- | -------------- | ----- | -------- |
| 1 | 122.9 µs | 127.9 µs | +4.0 % | wide both arms (slow mode) |
| 2 | 84.9 µs | 89.5 µs | +5.5 % | pre ±6 % — marginal |
| 3 | 84.9 µs | 92.6 µs | +9.1 % | ±2 % both arms — **the usable pair** |

**The whole session ran in the machine's slow state.** Every `pre` reading landed at
84.9–122.9 µs, never near the 61.5 µs of the earlier fast-state measurement, and 84.9
matches this file's documented slow-state value of ~83 µs almost exactly. Per the
measurement section below, the *ratio* is what survives across modes, so scaling the
fast-state figure by it: **≈ 65–67 µs at 100S10P, about 1.31× over the 50 µs budget**
(up from 1.23×). Do not read 92.6 µs as the headline number — it is a slow-state
reading, comparable only to the 82.8–83.2 µs slow-state row.

The cost is per-cell work in the reporting path, present in **every** mode: `q_gen_w`
is reported even when the pack is `Isothermal` (a deliberate feature — a client can
watch heat generation without thermal feedback), which costs a `v_rc` re-sum, an
entropy-coefficient lookup that short-circuits to `0.0` when the chemistry has no
table, and a handful of flops per cell. Reclaiming it would mean giving up that
contract; at 6 % that is not worth it.

**A phantom to not chase:** an early non-interleaved round showed `10S10P/current`
going 7.98 → 13.9 µs, an apparent +74 %. That is a mode artifact — the `pre` arm
caught the fast state and the `post` arm the slow one. A later slow-state `pre`
reading of 13.28 µs for the same case puts the real ratio at ~+4.5 %, in line with
100S10P. This is the fourth time in this file's history that a cross-mode pairing
produced a number that meant nothing.

## Candidate fixes, cheapest first

### 1. Remove the per-call `Vec` in `r0_lookup` — *bit-identical* — **DONE**

`ecm.rs` built `per_row: Vec<f64>` by interpolating *every* SOC row over temperature, then
interpolated across rows. Now `bracket()` finds the SOC segment first and only the two
bracketing rows are interpolated over temperature, then blended — 2 `interp1` calls plus
one lerp, no allocation.

- **Outcome:** together with item 2, **85.9 → 61.5 µs at 100S10P (−28.5 %)**. The
  allocation hypothesis held: 2000 `Vec`s per step were the bulk of the remaining cost.
- **Determinism:** verified, not argued. `tests/interp_equivalence.rs` pins both lookups
  against a verbatim copy of the pre-change code and compares `f64::to_bits` over every
  breakpoint, both ULP neighbours of every breakpoint, every segment midpoint, out-of-range
  values on both sides, and a 2001-point interior sweep. All bit-identical.
- **One deliberate divergence:** a NaN input to a *single-breakpoint* table used to panic
  (index underflow) and now clamps to the sole value. `step` must never panic, so this is
  the intended direction; it is pinned by test rather than left implicit. Whether
  `sim-data` validation can even produce a 1-point table was not checked — the test holds
  either way.

### 2. Binary-search in `interp1` — *bit-identical* — **DONE**

The linear scan is replaced by `slice::partition_point`, shared by both lookups through
`bracket()`. Cost no longer depends on where SOC sits in the table.

- `partition_point` answers `0` for a NaN needle, where the old scan stopped at `1`; the
  index is clamped to `1..=n-1` to reproduce the old NaN-propagating behaviour instead of
  underflowing. That clamp is load-bearing, not defensive noise — see the test.
- A cached "last segment" index would be faster still for the real access pattern (SOC
  moves slowly between steps) but adds mutable state to a pure lookup, and it would have to
  live in the cell state and therefore in the snapshot. Not pursued.
- **Not separately attributed.** Items 1 and 2 were measured together; the split between
  them is unknown. If that matters later, bench them one at a time.

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
- **Still deferred, and now more comfortably so.** 1–2 took the miss from 1.7× to 1.23×
  over. That is close enough that Phase 2's own per-cell work — which will change the hot
  loop anyway — should land before anyone reopens a design that makes a cache invariant
  load-bearing across every future per-cell mutation point.

### 4. Drop the per-group scratch `Vec` in `step` — *bit-identical, unmeasured*

`pack.rs:364` builds `cell_src: Vec<Vec<(f64, f64)>>` fresh every step: one inner `Vec` per
series group, so ~102 allocations per step at 100S10P. Two orders of magnitude fewer than
the 2000 that item 1 removed, so expect a correspondingly smaller win — but it is the same
*kind* of fix, equally bit-identical, and the obvious next thing to try below item 3. A
flat `Vec<(f64, f64)>` indexed by a running offset, or a scratch buffer owned by `Pack` and
cleared per step, both work. Measure before believing.

## Verification protocol for any of these

1. Bench both revisions **back to back in one session** — see the measurement section
   above. Do not use a baseline saved in an earlier session.
2. `cargo test --workspace` — the PyBaMM goldens, analytic golden, snapshot replay, and
   proptest invariants must all pass **unchanged**. For anything claiming bit-identity that
   is not sufficient on its own: those tests assert within a tolerance, so a one-ULP shift
   passes them while still breaking the snapshot-replay guarantee on some other trajectory.
   Add a case to `tests/interp_equivalence.rs` (or its equivalent for whatever you touched)
   that compares `to_bits` against a verbatim copy of the pre-change code.
3. `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` clean
   (the gate now compiles the bench too).
4. Update the measured table in `benches/pack_step.rs`'s module docs and in this file.

## Note for whoever picks this up

The 50 µs budget is deliberately **not** asserted in a test — a wall-clock assertion is
machine- and CI-dependent, and `CLAUDE.md` frames it as a budget to keep, not an exit
criterion. Track it by running the bench, not by a gate.

Current position: **≈ 64 µs baseline / ≈ 67 µs fully featured at 100S10P (fast-state
equivalent), 1.28–1.34× over.** Items 1–2 are done, item 3 is deferred behind Phase 2
by choice, item 4 is small and unmeasured, and Phase 2 is now measured end to end.
Nothing here is blocking.

**Item 3's deferral should be revisited now, and the case for taking it has improved.**
It was deferred because Phase 2 was about to add per-cell mutation points that a cached
Thévenin would have to invalidate. Phase 2 has landed, and there is exactly one such
point: the thermal integrator writing `temp_k` at the end of a step. That is a single,
obvious invalidation site in `Pack::step`, not the scattered set the deferral feared.
Phase 3 (aging, faults) will add more, so the window is now, or after Phase 3 — not in
the middle of it.

Two things to know before the next re-measure:

- **Use worktrees, not `git checkout` round-trips**, and filter to one benchmark case so
  each arm is ~10 s. `git worktree add <tmp> <rev>` gives each revision its own target
  dir, so alternating runs cost only the bench itself. This is what finally produced a
  usable pair for the thermal measurement after two full-suite rounds were unusable.
- **The machine's state persisted across an entire session.** The bimodality is not
  per-run jitter that averages out over a few minutes; plan on reporting a *ratio* plus
  a mode-matched anchor rather than an absolute number.
