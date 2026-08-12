# `Cell` is 264 bytes and the recorded hypothesis is stale

**Status:** change 1 **landed and measured**. Change 3 **declined on the evidence change 1
produced**. Change 2 was deferred unmeasured and then **landed on user direction** in a
second pass — its prediction was *amended and re-registered* before its arms were built,
because the falsifier as first written was sited where this box cannot read it. Every
prediction in this doc was registered before the measurement it names. See "Results" and
"Results — change 2".

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

#### Amended before change 2's arms were built — the falsifier was sited where this box cannot read it

The original above is left standing because the amendment is only honest if what it
replaced is visible. It put the whole discrimination on `1S1P`, and change 1's session
then measured `1S1P` at **128–228 ns on the same binary** with CIs of ±1.8–4.5 %, against
a predicted effect there of ~1–2 %: four L1-resident pointer chases against a ~700-cycle
step. **That is a test whose noise floor sits above its signal.** It cannot return the
pass worth banking or the fail worth honouring — it returns "cannot tell", and a
"cannot tell" that was pre-registered as *the* falsifier is a hole, not a result.

Change 1 supplied a better site for the same discrimination, at the topology where a good
round resolves to 0.03 %:

| | bytes removed per cell | measured |
| --- | --- | --- |
| change 1 | 80 B | ≈ −1.1 to −1.5 µs at 100S10P |
| change 2 | **8 B** | — |

**So the bound is: footprint alone cannot put change 2 much above a few tenths of a
percent at 100S10P.** Anything at or above ≈ 1 % there has to come from the only other
thing in the diff — the per-cell allocation and the dependent load. That gap, 0.2 % versus
≥ 1 %, is one this box can read on a clean paired round; the `1S1P` gap is not.

This is stated as a **bound, not a point estimate**, and deliberately so: this doc already
refuses to extrapolate change 1's single point along a curve that cache behaviour makes a
step function (see change 3's decline), and that refusal has to cut both ways or it was
never a rule. **"It could be 0 %" is an outcome the bound allows, not a defeat of it.**

* `100S10P/current` is now the **discriminator**. Below ≈ 0.5 %: consistent with footprint
  alone, mechanism unproven. At or above ≈ 1 %: footprint cannot explain it and the
  indirection can.
* `1S1P/current` is **demoted from falsifier to corroboration**. It is still run and still
  reported — a large clean move there would be a positive signal — but a null there no
  longer falsifies anything, because this box cannot distinguish that null from its own
  scatter.

#### Registered now: what happens if the timing is inconclusive

Written before benching, because it is the integrity risk in the slice. **This change is
user-directed, and it lands whether or not the timing reads.** So the justification on
record is *user direction plus a countable mechanism* — one heap block and one dependent
load per cell per pass, at four named sites — with whatever the bench actually said
reported beside it, including "nothing readable".

What must not happen is a noisy round being read as a measured win to retroactively
justify a snapshot-format bump. That is precisely the failure `pack-step-perf.md` exists
to prevent, and doing it while quoting that doc would be the worst version of it.

#### Registered: the version constants should move asymmetrically

`v_rc` reaches **no client**. `CellView` exposes `overpotential_v` (the sum), not the
vector, and every other mention of `v_rc` in the workspace outside `ecm.rs` is a doc
comment or a test's local variable. So:

* `sim_core::SNAPSHOT_VERSION` **moves**, 15 → 16.
* `sim_server::API_VERSION` and `sim_wasm`'s own constant **do not move**.
* The `rest.rs` / `ws.rs` assertions that read `sim_core::SNAPSHOT_VERSION` symbolically
  follow with no edit.

Registered so a surprise here is loud rather than a judgement call made mid-edit. The
standing lesson is to read each constant's own doc rather than assume it moves in step
(`docs/plans/ui-bms-view.md`); that applies in this direction too.

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

**Status after 2026-08-12's later session** (see batch 3 below, and the interleaved-null
rule now in `pack-step-perf.md`):

* **Change 2's timing — closed, as *measured and inconclusive*.** One further batch was
  spent and produced no admissible round. No claim in either direction; not to be retried.
* **The DFN/SPM arms — still owed, and now demonstrably blocked on the instrument, not on
  effort.** No bench cases exist for them at all (`criterion_group!` lists only
  `bench_single_cell`, `bench_mid_pack`, `bench_large_pack`, `bench_full_pack`,
  `bench_aging_pack`), so writing them is the first cost. But the blocking evidence is now
  concrete rather than inferred: on 2026-08-12 this box held **±0.77 % across three
  readings** of one binary, and **fifteen minutes later**, same case, same protocol, read
  **±0.99–2.85 % with a 53 % spread on one unchanged binary**. The prediction to be tested
  is **≤ 1–2 %**. An instrument whose floor moves by an order of magnitude inside one
  session cannot resolve it, and a future session should not re-litigate this as a question
  of effort. Write the cases when convenient; run them only behind an *interleaved* null.

## Results — change 2, `v_rc` inlined

Landed on user direction, which supersedes the deferral above. The deferral's stated reason
(a format change resting on an unmeasured claim) is answered by the registration written
before the arms were built: the justification of record is **user direction plus a countable
mechanism**, and whatever the bench said is reported below on its own terms.

### Sizes: the third registered prediction was exact too

| | before | predicted | measured |
| --- | --- | --- | --- |
| `EcmState` | 48 B | 40 B | **40 B** |
| `CellModel` | 56 B | 48 B | **48 B** |
| `Cell` | 184 B | 176 B | **176 B** |

`Cell` is now **176 B against the 264 B this slice started at — a third smaller.** The
per-cell heap block is gone: a 1000-cell pack made 1000 separate 8-byte allocations at
`Pack::new` and now makes none.

### Correctness: pure layout again

Full workspace suite, `--no-fail-fast`: **64 test binaries, 0 failures, exit 0**, the ECM
analytic golden and the DFN/SPM goldens among them. That is the evidence for bit-identity,
and it is the right evidence — the construction *argues* identity (a one-pair sum gains a
`+ 0.0`, which is exact), but only the goldens check it. `clippy --workspace --all-targets
-D warnings` clean; `fmt --check` clean.

### One deviation from the plan, and one guard the plan did not anticipate

The plan said `advance_cell` would write `state.v_rc[..chem.rc.len()]`. It zips the pairs
against the slots instead — `chem.rc.iter().zip(state.v_rc.iter_mut())` — which drops the
`chem.rc[k]` bounds check as well as the slice range, and keeps the loop count identical to
the `Vec` it replaced.

That introduced a hazard the `Vec` did not have, and it is worth naming because it is
**quiet**: a zip against a too-short array *truncates*. A chemistry with three RC pairs
would have had its third silently never integrated, where the old `chem.rc[k]` would have
panicked. The fix is structural rather than a test — `ecm::MAX_RC_PAIRS` is the array's
length, and `ChemistryParams::validate` now reads its upper bound from there, so loosening
the validator does not compile until the array is widened to match.

### The version bump, and why v16's check earns its keep

`SNAPSHOT_VERSION` 15 → 16. **The version asymmetry was registered in advance and held
exactly:** `sim_server::API_VERSION` stays at 2 and `sim-wasm`'s stays at 6, because
`CellView` has only ever exposed the *sum* (`overpotential_v`), never the vector — so a
change to how the summands are stored cannot cross either boundary. Both constants' own
docs now record the non-move, per the standing rule not to move them as a set.

v16 is also the first bump since v11 whose stale blob is **structurally valid**, and it is
the sharpest case yet. Under `bincode` a one-pair `Vec<f64>` is an 8-byte length followed by
one 8-byte value; `[f64; 2]` is two 8-byte values — the same sixteen bytes. A v15 field does
not fail to parse at v16, it parses, with the length reinterpreted as a subnormal `5e-324`
and the real overpotential slid one slot along. v14's and v15's stale blobs failed loudly at
deserialization; this one would restore into arithmetic on a length prefix, and only the
version check stands there.

**That claim is asserted at the level it can honestly be asserted at.** A new test in
`snapshot_version.rs` demonstrates the reinterpretation on the *field's* bytes. It is not
extended to a whole v15 snapshot, because fabricating one means inserting a length prefix at
four offsets this repo has no non-guessing way to locate — so neither the test, the module
doc, nor the `SNAPSHOT_VERSION` note claims the whole-blob case. The v14 → v15 pair test was
**replaced rather than renumbered**, and not only because its own assertion demanded it:
that fixture retags *this build's* bytes, and this build no longer produces v15-shaped ones.

### Timing, batch 1: four rounds, and not one of them is admissible

| round | order | base | inlined | Δ (absolute) | Δ (at this baseline) |
| --- | --- | --- | --- | --- | --- |
| 1 | base first | 64.164 µs | 56.673 µs | −7.491 µs | −11.67 % |
| 2 | inlined first | 60.688 µs | 52.534 µs | −8.154 µs | −13.44 % |
| 3 | base first | 53.844 µs | 60.435 µs | **+6.591 µs** | **+12.24 %** |
| 4 | inlined first | 59.585 µs | 59.708 µs | +0.123 µs | +0.21 % |

**The sign flips.** Rounds 1 and 2 say the change is a large win, round 3 says it is a large
loss, round 4 says it is nothing, and all eight readings come from the same two binaries.

This is refused on the acceptance criterion **this repo already had**, written down after
change 1 and therefore predating these numbers: *trust a round only when its base arm
reproduces and its CIs are tight.* Change 1's two kept rounds had base arms agreeing to
**0.03 %** and CIs **≤ 0.4 %**. Here the base arm alone reads 53.844, 59.585, 60.688 and
64.164 µs — a **19 % spread** — and the CIs run 1.5–4.0 %, five to ten times wider. **Zero
rounds pass.** Reporting the mean of the four (−3.7 %) would be averaging drift and calling
it a measurement.

### The stopping rule for batch 2, registered before it ran

Written into this doc before the batch was launched, because after seeing batch 1 any filter
invented afterwards is chosen by its answer:

* Protocol change: **`--measurement-time` 10 s** (was 8) and **6 rounds** (was 4), still
  alternating arm order, still both topologies in one invocation per arm. Batch 1 is
  therefore *not* pooled with batch 2 — a protocol change forfeits comparability, and the
  batch is reported alone.
* A round is **admissible** only if both arms' `100S10P` confidence intervals are ≤ 1.0 %.
* The batch is **readable** only if at least two admissible rounds exist, their base-arm
  point estimates agree to within 2 %, **and** their deltas share a sign.
* If readable, the result is the mean of the admissible rounds' **absolute** deltas, quoted
  with the baseline it was measured against.
* If not readable: **inconclusive, and there is no third batch.** The change lands on the
  justification registered before batch 1 — user direction plus a countable mechanism — with
  the sizes as the certain result.

### Timing, batch 2: refused by its own rule, and the rule is what makes this reportable

| round | order | base | inlined | Δ (absolute) | base CI | inlined CI |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | base first | 62.689 µs | 65.303 µs | +2.614 µs | ±1.5 % | ±1.5 % |
| 2 | inlined first | 61.539 µs | 66.496 µs | +4.957 µs | ±1.9 % | ±1.5 % |
| 3 | base first | 61.105 µs | 64.774 µs | +3.669 µs | ±1.8 % | ±1.5 % |
| 4 | inlined first | 67.588 µs | 64.884 µs | −2.704 µs | ±1.8 % | ±1.5 % |
| 5 | base first | 69.554 µs | **173.030 µs** | +103.5 µs | ±4.4 % | ±17.6 % |
| 6 | inlined first | 86.128 µs | **333.740 µs** | +247.6 µs | ±3.1 % | ±9.7 % |

**No round is admissible.** The registered bar was both arms' `100S10P` CI ≤ 1.0 %; the best
round here is ±1.5 %. Rounds 1–4 would also have failed the base-arm agreement test (61.1 to
67.6 µs, a 10 % spread against the 2 % allowed) and their deltas flip sign anyway. Rounds 5
and 6 are not CPU state at all — 173 µs and 334 µs are **three to five times** anything this
bench has ever recorded, on a binary that read 64.9 µs twenty minutes earlier — so the box
acquired an outright contention failure mode partway through the batch, which is new.

**Verdict: inconclusive, and by the registered rule there is no third batch.** Across both
batches the same two binaries read anywhere from −13.4 % to +8.1 % with the sign flipping
inside every batch, and the one thing the two batches agree on is that they disagree: batch
1 leaned negative, batch 2 leaned positive. That is not a small effect measured badly, it is
no measurement.

### So what is actually claimed for change 2

**Claimed, and certain:** `EcmState` 48 → 40 B, `CellModel` 56 → 48 B, `Cell` 184 → 176 B —
a third smaller than the 264 B this slice opened at — and 1000 heap allocations per
1000-cell pack removed. All three sizes were predicted exactly before the arms were built.
Trajectories bit-identical, 64 test binaries green.

**Claimed, as a mechanism and not as a measurement:** one heap block and one dependent load
per cell per pass, at four named sites, is gone. Countable, in the sense
`pack-step-perf.md` demands of an explanation — but a count of what was removed is not a
measurement of what it was worth.

**Not claimed:** any timing effect, in either direction. The registered contingency governs:
this change landed on **user direction plus that mechanism**, with the bench reported as it
came out. The amended discriminator — under ≈ 0.5 % at 100S10P means footprint alone, at or
above ≈ 1 % means the indirection — was never reached, because the box never got inside
±4 % of itself. It remains the right test for a session that can run it.

**And the `1S1P` demotion earned itself.** Under the original registration a `1S1P` null was
*the* falsifier for this change. `1S1P` read 0.134 µs to 0.492 µs across the two batches on
the same binaries — a 3.7× spread — and produced deltas of +29.6 %, −9.0 %, +1.5 %, +0.2 %,
+8.7 %, +8.7 %, +6.0 %, +3.3 %, +43.4 %, +104.8 %. Had it stayed the falsifier, this slice
would have had to read one of those as a verdict. That is the concrete payoff of amending a
prediction *before* the arms are built rather than after the numbers arrive.

### Timing, batch 3 — a later session on 2026-08-12, and why it was allowed to run at all

The two batches above closed with **"inconclusive, and by the registered rule there is no
third batch."** A later session the same day ran one anyway. The justification, registered in
full before either arm was built and not invented afterwards, is that **new information about
the instrument had arrived**: an instrument check — three readings of one unchanged binary,
nothing else varied — came back **72.623 / 70.207 / 71.574 µs, all CIs ≤ ±0.77 %**, the first
time in six sessions this box reproduced itself. The stopping rule above exists to stop
re-rolling *the same dice*; a box that has just demonstrated ±2 % reproducibility is not the
same dice. That reading is defensible, but it is a reading, and it is recorded here as one.

New registration, written before the arms existed: one case (`100S10P/current`), four
alternating rounds, **one batch and no second**, admissibility inherited unchanged from batch
2's rule. Prediction restated as a **bound, not a point** — footprint alone puts change 2
under ≈ 0.5 %, and ≥ 1 % would mean the indirection matters beyond the bytes. The
registration also stated up front that the measurement was **underpowered** (a sub-0.5 %
effect on a box reproducing to ±2 %) and that the honest deliverable was therefore an upper
bound, not a point.

| round | order | base | inlined | Δ | base CI | inlined CI | admissible? |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | base first | 77.843 µs | 64.333 µs | −17.4 % | ±1.14 % | ±1.38 % | no |
| 2 | inlined first | 55.141 µs | 51.838 µs | −6.0 % | ±2.85 % | ±1.05 % | no |
| 3 | base first | 51.569 µs | 49.458 µs | −4.1 % | ±1.07 % | ±1.15 % | no |
| 4 | inlined first | 50.868 µs | 49.385 µs | −2.9 % | ±0.99 % | ±1.08 % | no |

**Zero rounds admissible — one *arm* out of eight qualified.** Four more missed the ±1.0 %
bar by 0.05–0.15 percentage points, and a near-miss is not a licence. The batch also fails
the second criterion independently: the base arm alone reads 77.8, 55.1, 51.6, 50.9 µs, a
**53 % spread** on one unchanged binary against the 2 % allowed.

**All four deltas are negative, and that is not weak confirmation — read it the other way.**
The registration predicted this change is bounded under ≈ 0.5 %. Every observed delta is
**six to thirty-five times** that ceiling, and every one sits above the ≈ 1 % line the
registration set for "the indirection matters". A batch whose smallest reading is six times
the largest effect it could be measuring has a noise floor wider than its own question. It
cannot separate *the indirection matters enormously* from *the box moved*, and no re-reading
of it can. The deltas also shrink monotonically as the baseline falls (−17.4 → −6.0 → −4.1 →
−2.9 % while the box settles 78 → 50 µs), so any single round quoted would be a number picked
off a moving box.

Held at observation strength and no higher: **no round showed the change slower**, across
both arm orders. Alternation was meant to make that meaningful, since monotonic drift flips
the delta's sign when arm order flips. It is weaker than it reads — four same-sign rounds is
a one-in-eight coincidence under a fair coin, and the drift is not monotonic at round scale
anyway (`r1-chg` and `r2-chg` are the same binary thirty seconds apart and differ by 20 %).

**Verdict: inconclusive, again, and no upper bound is recorded either** — the registration
anticipated delivering "worth no more than X %", but a bound needs an admissible round to sit
on and there is none. Nothing above the "So what is actually claimed" section changes. The
question is now **closed rather than owed**: three batches and fourteen rounds have been
spent, and the outcome of the re-roll is itself the argument for the original stopping rule.

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
