# `Cell` is 264 bytes and the recorded hypothesis is stale

**Status:** change 1 **landed and measured**. Change 3 **declined on the evidence change 1
produced**. Change 2 **deferred unmeasured**, with the reason recorded. Predictions were
registered below **before** the first bench arm ran, and both held — see "Results".

`CLAUDE.md` budgets `Pack::step` at **< 50 µs at 100S10P** (1000 cells). Phase 3 spent
part of the margin and `docs/plans/pack-step-perf.md` has recorded the budget as
**marginal rather than met** ever since — ≈ 42–54 µs, scaled onto a fast-state anchor
this box has not reached in five consecutive sessions. This is the slice that goes after
it, and it starts by discovering that the lead it was going to follow is out of date.

## What the size probe found

Measured with `std::mem::size_of` on a throwaway unit test in `pack.rs` (removed again;
a permanent version of it is a deliverable of this slice — see below):

| type          | bytes |
| ------------- | ----- |
| `Cell`        | **264** |
| ├ `CellModel` | **136** |
| ├ `CellAging` | 88    |
| ├ `CellRunaway` | 16  |
| ├ `capacity_factor` + `r0_factor` | 16 |
| └ `shunt_g`   | 8     |
| `EcmState`    | 48    |
| `SpmState`    | 64    |
| `DfnState`    | **136** |

That accounts for all 264 bytes exactly.

## The recorded lead is stale, and this is why

`docs/plans/phase-3-aging-faults.md` (slice E) recorded the per-step lead as:

> most of `Cell`'s growth is `CellAging`'s accumulators, and none of them is read on a
> non-ticking step. Splitting them into a parallel array touched only at the aging tick
> would take `Cell` back toward 96 bytes.

That was written at `Cell` = 160 bytes, when `CellAging` was 72 of them — i.e. **before
Phases 6 and 7 existed**. Since then:

* `CellAging` grew 72 → 88 (`q_reversal` and `ah_reversed_since_tick`, from
  `docs/plans/reversal-damage.md`).
* `CellModel` grew from an ECM-only enum to one carrying `Spm` (Phase 6) and `Dfn`
  (Phase 7). An enum is sized by its largest variant, so **every ECM cell in every pack
  now carries 136 bytes of cell-model slot where `EcmState` needs 48** — 88 bytes of
  padding, per cell, paid by packs that will never instantiate a porous-electrode model.

So the largest single term is no longer aging. It is the two porous variants, and no
prior doc names them as a cost because Phase 6 and 7's own perf legs asked a different
question (what the *porous* models cost, answered as their own budgets) and correctly
found the ECM path unchanged in *instructions*. Enum width is not an instruction.

**`CellAging` is now the second term, not the first.** Nothing here contradicts the old
lead; it re-ranks it.

## A third term nobody has looked at: `v_rc` is on the heap

`EcmState` is 48 bytes = `soc` + `soc_deficit` + `temp_k` (24) + `v_rc: Vec<f64>` (24).
The RC-pair overpotentials — **one or two `f64`** — are a **separate heap allocation per
cell**. A 1000-cell ECM pack therefore holds 1000 independent 8- or 16-byte blocks, and
every read of them is a dependent pointer load.

It is read on every hot pass, not one:

| site | pass |
| ---- | ---- |
| `ecm.rs:825` `cell_source` — `state.v_rc.iter().sum()` | source/solve pass, per cell |
| `ecm.rs:911` `advance_cell` — `state.v_rc.iter_mut()` | advance pass, per cell |
| `ecm.rs:248` `overpotential_v` — `s.v_rc.iter().sum()` | reporting pass, per cell |
| `ecm.rs:414` `heat_w` — `s.v_rc.iter().sum::<f64>()` | reporting pass, per cell |

That is the countable kind of explanation `pack-step-perf.md` demands. Its own record of
item 3 says the −35 % first got "a data-layout story that was unprofiled and probably
wrong" and that the real explanation was four countable divisions. A footprint argument is
the same shape as the discarded story. **A pointer chase per cell per pass is not** —
it is a count.

The bench's chemistry has **one** RC pair, so the shipped measurement's allocation is
8 bytes per cell: the worst case for allocator scatter.

## The plan, in the order the version cost implies

Three changes, landed as **separate commits and benched separately**, because
`pack-step-perf.md` records items 1–2 as permanently unattributed for exactly the
failure of not doing that.

### 1. Box the porous variants — `Spm(Box<SpmState>)`, `Dfn(Box<DfnState>)`

Largest single term, and the cheapest to land: `Box<T>` is **serde-transparent**, so no
saved pack changes shape and `SNAPSHOT_VERSION` stays at **15**. 28 match sites across
`ecm.rs`, `pack.rs` and three test files; mechanical.

Costs the porous models one indirection on a step that costs ≈ 1 µs (SPM, N = 20) to
≈ 180 µs (DFN) **per cell**. It is not measurable there, and the DFN/SPM benches are the
check that says so rather than the assumption that says so.

### 2. Inline `v_rc` as `[f64; 2]`

`EcmState` 48 → 40, and — the point — the per-cell heap block goes away. The unused
second slot on an `Ecm1Rc` cell stays `0.0` forever, so every `.iter().sum()` site is
correct unchanged; only `advance_cell` needs to write `[..chem.rc.len()]` rather than the
whole array.

**This one changes the snapshot format** (`[x]` becomes `[x, 0.0]` at one RC pair), so it
needs `SNAPSHOT_VERSION` 15 → 16, `snapshot_version.rs`, and a **separate** check of
`sim-wasm`'s own constant — the two have parted before (`docs/plans/ui-bms-view.md`), so
the rule is to read that constant's own doc rather than assume it moves in step.

### 3. Split `CellAging`'s cold fields — deferred, possibly declined

Now the smaller term. It also changes snapshot layout *and* fights the borrow checker
(`group.cells.iter_mut()` against a sibling `Vec` on `self`). Not worth paying for 72
bytes until 1 and 2 are measured. Note the old lead's own counter-evidence still stands:
the 1S1P penalty (+12–14 %) was *larger* than the 100S10P one (+7–10 %), and at one cell
there is no footprint problem at all, so part of Phase 3's cost is fixed per-step work
that no layout change reaches.

## Predictions, registered before the first measurement

`pack-step-perf.md` and four recent slices all record the same lesson: a green number
with the wrong explanation behind it is the failure mode, and the pre-written prediction
is what catches it. So:

### Sizes — exact, and the machine cannot make these noisy

| after | `CellModel` | `Cell` |
| ----- | ----------- | ------ |
| today | 136 | 264 |
| change 1 | **56** (8 tag + 48 `EcmState`) | **184** (−30.3 %) |
| changes 1+2 | **48** (8 tag + 40 `EcmState`) | **176** (−33.3 %) |

No niche optimisation is expected: all four variants carry data, so the discriminant
cannot hide in `Vec`'s non-null pointer.

### Timings — and the discriminator that can falsify each explanation

**Change 1 is pure footprint.** At one cell there is no footprint problem.

* `100S10P/current` — expect **faster**. If it moves at all it should move by more than
  the round-to-round scatter.
* `1S1P/current` — expect **no change**, straddling zero.
* **Falsifier: if `1S1P` moves materially, the footprint explanation is wrong** even if
  the 1000-cell number is green, and this doc must say so rather than bank the win.
* DFN and SPM benches: expect **no material change** (≤ 1–2 %). A pointer deref against
  a per-cell step of 1–180 µs.

**Change 2 removes an indirection that exists at any pack size.**

* `1S1P/current` — expect **faster**. This is what separates it from change 1.
* `100S10P/current` — expect faster by **at least** as much in relative terms, since the
  1000-cell case gets the locality win on top of the indirection win.
* **Falsifier: if `1S1P` does not move, the pointer-chase explanation is wrong** and the
  change is a footprint change wearing a mechanism's label.

## What can honestly be claimed at the end

This box has been in its slow CPU state for five-plus consecutive sessions
(`pack-step-perf.md`), so **"now under 50 µs" is very unlikely to be available** and a
ratio against a mode-matched anchor is the answer. The sizes are the one thing this
machine measures exactly regardless of its mood, which is why the permanent size test
below is a deliverable and not only a guard.

## Deliverable that does not depend on the machine

A permanent test pinning `size_of::<Cell>()` **and each variant's contribution**, so a
future `CellModel` variant that re-widens the enum fails loudly instead of silently
taxing every ECM pack for two phases. This slice exists because that test did not.

## Results

### Sizes: both predictions exact

`crates/sim-core/src/pack.rs::cell_footprint` is now the standing check, and it passed on
its first run with the numbers registered above, unedited:

| | before | predicted | measured |
| --- | --- | --- | --- |
| `CellModel` | 136 B | 56 B | **56 B** |
| `Cell` | 264 B | 184 B | **184 B** (−30.3 %) |

At 100S10P that is **80 KB less** streamed per pass over the cell array.

### Correctness: the change is pure layout

Full workspace suite, `--no-fail-fast`: **64 test binaries, 0 failures, exit 0** — the DFN
and SPM goldens among them, so every model's trajectory is bit-identical. `clippy
--workspace --all-targets -D warnings` clean; `fmt --check` clean. No saved pack changed
shape, so `SNAPSHOT_VERSION` stayed at 15, as predicted.

**Not one call site needed editing.** All 22 `CellModel::Spm(s)` / `Dfn(s)` match arms
compiled unchanged: `&Box<T>` deref-coerces to `&T` in argument position and auto-derefs
for field access, so the boxing is invisible above the enum definition. That is worth
recording as the reason this trade is cheap to make *and* cheap to reverse.

### Timing: ≈ 1.1–1.5 µs at 1000 cells, and nothing at one cell

Two clean paired rounds, alternating order, both cases in one invocation per arm so the
two topologies are mode-matched to each other:

| round | order | case | base | boxed | Δ (absolute) | Δ (at this baseline) |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | base first | `100S10P/current` | 79.302 µs | 78.216 µs | **−1.086 µs** | −1.37 % |
| 2 | boxed first | `100S10P/current` | 79.279 µs | 77.771 µs | **−1.508 µs** | −1.90 % |
| 1 | base first | `1S1P/current` | 219.3 ns | 217.8 ns | −1.5 ns | −0.67 % |

The base arm reproduced to **0.03 %** across the two rounds and every CI above is ≤ 0.4 %,
so the 1000-cell effect is ~50× the round-to-round scatter and its sign is certain under
both arm orders — which is what rules out the drift that ruined everything else below.

**The absolute is the column that travels; the percentage is not.** Both rounds ran against
a 79 µs baseline, and the same two binaries were observed at 52 µs an hour later. If the
saving is a fixed cost it is ≈ −2.5 % in the fast state; if it is proportional it stays at
≈ −1.6 % everywhere. **Two rounds inside one CPU state cannot separate those**, so a future
session comparing its own ratio against "−1.4 to −1.9 %" would be comparing against a
number that silently carries this session's denominator. `pack-step-perf.md` already states
the rule for a denominator that moved because the *code* changed; this is the same failure
with the *machine* as the denominator, and it is the sharper case because nothing in the
diff warns you.

**Both registered predictions held.**

* `100S10P` moved, and by more than the scatter. ✔
* `1S1P` did **not** move: −0.67 % against combined CIs of ±0.4 % and ±0.6 % straddles
  zero. **The falsifier did not fire**, so the footprint explanation survives a test that
  could have killed it. Every other `1S1P` pair in the session is dominated by machine
  drift (±1.8–4.5 % CIs, and readings from 128 ns to 228 ns on the *same binary*), so
  round 1 is the only pair that says anything, and what it says is "nothing".

### What the small win actually tells us, and the change it kills

**A ≈ 30 % cut in per-cell footprint bought ≈ 1.5 % of step time.** That is a real win and
a small one, and what it establishes is a *bound*: at 264 B/cell the step is **not**
memory-bound, so no further layout change on this structure can be large.

That retires change 3 without building it. Splitting `CellAging`'s cold accumulators out
removes 72 B/cell — nine tenths of what change 1 removed — so change 1 bounds its benefit
**small**, and its cost is **certain and large**: a snapshot-layout change, a
`SNAPSHOT_VERSION` bump and migration, and a borrow-checker fight between
`group.cells.iter_mut()` and a sibling `Vec` on `self`. **Declined on that asymmetry.**

Deliberately *not* declined on an arithmetic "72/80 × 1.5 % = 1.4 %". That extrapolates a
single point along a curve which cache behaviour makes a step function, not a line — it
could be 0 % or it could be 3 %. The decision does not need the number and should not rest
on it: a certain large cost against a benefit bounded small is enough. Note this also
supersedes the Phase 3 lead that pointed at those accumulators in the first place — they
were never the big term, and are not a worthwhile one either.

### Change 2 is deferred unmeasured, and that is the honest call

Its entire justification is a mechanism — one heap block and one dependent load per cell
per pass — whose size is unknown, and it is the change that **costs a snapshot version
bump**. Landing a format change on an unmeasured perf claim is the thing this repo's perf
doc exists to prevent. This box could not measure a 1–2 % effect today (below), so the
measurement that would justify it is unavailable, and it waits.

The predictions for it stand as written and are still the right test: it must move `1S1P`,
or it is a footprint change wearing a mechanism's label. Change 1 sharpens the stakes
without supplying a number — it removed ten times as many bytes per cell as change 2 would
and still landed small, so **footprint alone cannot explain a material win here**. A `1S1P`
null would therefore leave nothing to bank, whatever the 1000-cell arm said.

### The box, and why no absolute is claimed

**Six consecutive sessions without a reproducible fast state, and this is the worst.**
A second batch of three rounds was run and is **discarded whole**. It measured the same
change at **+26.67 %** in one round and **−29.98 %** in the next, because the baseline arm
itself swung between **52.3 µs and 79.2 µs within the batch**:

| batch 2 | `100S10P/current` base | boxed |
| --- | --- | --- |
| round 1 | 79.153 µs | 73.575 µs |
| round 2 | 52.319 µs | 66.270 µs (CI ±4.5 %) |
| round 3 | 74.534 µs | 52.188 µs |

Every one of those is the same two binaries. This is the sharpest evidence yet for the
existing rule — and it extends it: **the swing can happen between the two arms of a single
round**, so alternating the order is not a refinement, it is what makes any of this
readable. Batch 1's two kept rounds are trusted precisely because their base arms agree to
0.03 % and their CIs are ≤ 0.4 %.

The session's 79 µs baseline is also **outside** both bands this repo has recorded (36–42 µs
fast, 51–55 µs slow), and round 2 above touched 52.3 µs, so the box has at least three
states rather than two. **No absolute is claimed and the < 50 µs budget is neither
confirmed nor refuted here.** The sizes are what this session establishes for certain.

### Owed

The DFN and SPM benches were **not** run. The registered prediction (≤ 1–2 %, an
indirection against a 1–180 µs per-cell step) is therefore **unverified**, and on a box
swinging ±30 % it could not have been verified today. It is owed to the next stable
session, alongside change 2's measurement. Recorded as owed rather than argued away: an
analytic bound is exactly the "plausible, not countable" reasoning this repo's perf doc
rejects.

## Methodology — the traps, all previously paid for

From `pack-step-perf.md` and `crates/sim-core/benches/pack_step.rs`:

* `git worktree add` per revision so alternating arms pay no rebuild.
* **Build every arm, then wait, then bench.** A 1.8× spread was traced to benching in the
  minutes after a build.
* Alternate arm order between rounds; a fixed base→change order always runs the change on
  the hottest CPU.
* Filter to one case so each arm is ~10 s; discard any round with a wide CI, and any round
  where `full` comes in cheaper than `current` (impossible — it is a strict superset).
* Run the bench arms at **normal** priority, not the standing below-normal rule
  (`run-tests-below-normal-priority`), because here the number *is* the deliverable and
  below-normal makes it unfair. Tests and builds stay below-normal.
