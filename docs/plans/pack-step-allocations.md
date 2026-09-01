# The per-step allocations, removed — and counted rather than timed

**Landed 2026-09-01.** A fully-featured `Pack::step` asked the allocator for seven heap
blocks totalling **27,216 bytes** at 100S10P, on every single step. It now asks for
none, on every configuration. No trajectory moved: the out-of-tree bit instrument is
**byte-identical across 1846 lines**, and the workspace suite is green in all 73 test
binaries. No snapshot bump — the new field is `#[serde(skip)]`.

This takes the first of the two levers `pack-step-perf.md` left identified-and-not-taken
("the remaining per-step allocations"), declines the second on the record, and kills a
third lead that turns out to have been mis-priced when it was written.

## Why this slice measures allocations and not microseconds

`pack-step-perf.md` records six-plus sessions of trying to time this box, and the
history is not encouraging: the same unchanged binary read 52.3 and 79.2 µs inside one
batch, two same-binary arms thirty seconds apart differed by 20 %, and the machine has
failed to reach its recorded fast state five consecutive sessions. The doc's own summary
is that an absolute budget claim is **unavailable by default**. Against that noise
floor, a lever the same doc bounds at "single-digit percent at best" is not measurable
here, and no amount of re-rolling changes that.

An allocation count has none of that problem. It is **deterministic** — the same code
counts the same on any run, any CPU state, any day — so it answers the question the
clock cannot: did this change remove the work it claims to remove. It is also a
*permanent* answer, because it can be a test, where a number in a document rots.

What is given up is stated plainly: **this slice makes no timing claim at all.** It
does not say the step got faster by X, and it does not say the < 50 µs budget is now
met. The direction is known by construction (the same arithmetic runs, with fewer
allocator calls in front of it, so it cannot be slower), and the magnitude is not
measured. Anyone wanting the magnitude should read the measurement protocol in
`pack-step-perf.md` first, and should expect "inconclusive" to be the honest result.

## The instrument

`crates/sim-core/tests/step_allocations.rs` — a counting `GlobalAlloc` (an atomic
counter in front of `System`) and one `#[test]` function. Three choices in it are
load-bearing:

- **One test function in the file, and it must stay one.** The counter is process-wide,
  so two tests running concurrently in the same binary would count each other.
- **`realloc` and `alloc_zeroed` are deliberately not overridden**, so the trait's
  default bodies route them through `alloc`. A `Vec` that grows is exactly the case this
  file exists to catch; forwarding those to `System` would hide it.
- **Steady state, and per step.** Several buffers are legitimately allocated once and
  reused forever (`SourceCache`, the BMS's own frame), so the run warms the pack for
  twelve steps and then counts sixteen more **individually**. A total can be a single
  step doing all the work; sixteen equal readings cannot.

`#![forbid(unsafe_code)]` in `src/lib.rs` does not reach it — an integration test is its
own crate — and no `unsafe` is added to the engine.

## Measured

Same binary, same test, engine parked at `098f179` for the left column via
`git stash`. Every reading was identical across all sixteen counted steps.

| configuration | before | after |
| ------------- | ------ | ----- |
| bare (no thermal, no BMS) | 1 alloc, 64 B | **0** |
| thermal network only | 4 allocs, 352 B | **0** |
| BMS only | 4 allocs, 144 B | **0** |
| thermal + BMS | 7 allocs, 432 B | **0** |
| thermal + BMS + aging | 7 allocs, 432 B | **0** |
| **100S10P, thermal + BMS** | **7 allocs, 27,216 B** | **0** |

Two things fall out of the table that were not the point of it:

- **Aging adds no allocation**, which independently confirms a claim
  `phase-3-aging-faults.md` made from timing alone ("the always-paid part of aging is
  below the noise floor"). Here it is exactly zero rather than below a floor.
- **The byte count is almost all three buffers.** `heat_w`, `temps` and the integrator's
  scratch copy are one `f64` per cell each, so they are 24 kB of the 27,216 B at 1000
  cells and 288 B of the 432 B at twelve. The 4S3P rows understate the change by two
  orders of magnitude, which is why the 100S10P row is in the test rather than derived.

## The eight buffers, and the one rule that makes this safe

Seven are now fields of a `StepScratch` on the pack, borrowed field-by-field at the top
of `step` (disjoint borrows, so the `&mut self.groups` loops still compile). The eighth
is not a pack buffer at all: `Bms::sample` used to take two `Vec`s **by value** and move
them into the stored frame, so the caller had to build fresh ones every sampled step. It
now takes slices and overwrites the frame's own buffers in place.

**Every field is written before it is read, within a single step.** That is the whole
safety argument and it is not decoration. A buffer that carried a value across the step
boundary would be *state*; it is `#[serde(skip)]`, so it would be missing from the
snapshot, and a restored pack would diverge from a live one on its next step. That is
the same hazard `SourceCache` manages with the opposite resolution — the memo is
deliberately carried over and is correct because a cold recompute reproduces it exactly,
whereas nothing in `StepScratch` may be carried at all.

The sharp edge is that **three of these buffers carry meaning in their length**:
`bleed_g` empty means "nothing bleeds anywhere", `v_group` empty means "nothing sensed
this pack", `heat_w` empty means "isothermal". The old code expressed that by
constructing `Vec::new()` on the arm that leaves them unfilled. A reused buffer has to
express it by **clearing unconditionally and pushing conditionally** — every one of them
does, and getting that backwards would not be a slow step, it would be last step's data
read as this step's.

## The instrument was made to fail before its green was believed

The bit-exact evidence is a byte-identical 1846-line dump from the out-of-tree
instrument at `M:\claud_projects\temp\phase6-baseline`, run against a worktree of
`098f179` and against this tree. A green from a harness nobody has tried to redden is
not evidence, so three perturbations were run, each deleting exactly one `clear()`, with
the verdict **predicted in writing first**:

| perturbation | predicted | observed | lines differing (of 1846) |
| ------------ | --------- | -------- | ------------------------- |
| delete `v_group.clear()` | RED | **RED** | 21 |
| delete `heat_w.clear()` | RED | **RED** | 985 |
| delete `bleed_g.clear()` | GREEN | **GREEN** | 0 |

The third is a finding, not a formality: `bleed_g`'s clear is **unreachable as a
correctness requirement today**. `bleed_conductances` clears the buffer itself on the
arm that fills it, and the arm that does not fill it (`bms: None`) never filled it in
the first place, so no configuration in the instrument can tell the difference. It stays
in, as the one place the write-before-read rule is defence against a future config
change rather than a live requirement — and it is now on the record as such, so nobody
has to re-derive that it is dead.

The first is worth a second look too. Deleting the clear on the *BMS* buffer moves only
21 lines, because most cases in the dump have no BMS at all. A perturbation that reddens
by 21 lines out of 1846 is a thin margin, and it is thin for a reason that has bitten
this repo four times before (`ANCHORS.md`): the instrument's discriminating power is
uneven across the features it covers. It was enough here. It would not obviously be
enough for a change confined to the sensor path.

## Two leads closed, both without writing code

**The `powf` lead is mis-priced and is now dead.** `pack-step-perf.md` recorded the
aging tick's +50 % as "likely `cycle_increment`'s `dod.powf(exp − 1.0)` … and the
exponent is a chemistry constant, so it is hoistable". Reading
`aging.rs::cycle_increment` settles it: what is hoistable is `params.cyc_dod_stress_exp
- 1.0`, **one f64 subtraction per cell per tick**. The call itself has `dod` as its
base, which is per-cell and per-step by definition, so the `powf` cannot be hoisted at
all. The lead named the right function and the wrong quantity — it promised ~20–50 ns
per cell and can deliver a subtraction. Do not pick it up.

**The multiply-by-reciprocal lever is declined on the record.** It is where the
remaining time demonstrably goes (four dependent divisions per `cell_source`, counted in
`pack-step-perf.md`), and it is *not* bit-identical: `x * (1.0/y)` and `x / y` differ in
the last place. This repo's spine is exact replay — snapshot round-trip bit-identity, a
committed golden set, and an out-of-tree anchor whose whole job is catching a moved ULP.
Trading that for single-digit percent, on a machine that cannot measure single-digit
percent, is a bad trade in both halves. If it is ever taken it should be taken as a
declared numerical change with the goldens regenerated, not slipped in as an
optimisation.

## What is left

- **The budget itself is still unverified**, exactly as it was this morning. `< 50 µs`
  at 100S10P has not been reproduced on this box since 2026-07-27, and this slice does
  not change that. It is a measurement problem, not an engine problem.
- **The fire path still allocates.** Once a cell reaches onset, `step` gathers the
  per-cell exothermic state and `advance_temperatures` allocates two more buffers per
  call. Left deliberately: a pack in thermal runaway has no performance budget, and
  keeping them out keeps the healthy path's story a simple "zero".
- **The nonlinear (SPM/DFN) iteration buffers are untouched.** They stay empty and
  unallocated on an ECM pack — which is what the budget is about — and are allocated per
  step on a porous one. A porous pack costs 26× (SPM) to 141× (DFN) per cell, so its
  allocations are not the thing to fix first.
