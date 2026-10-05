# The OCV segment hint — and the 8 % that was 27 %

> **Status: landed 2026-10-05.** `Pack::step` at 100S10P on the equivalent circuit is about
> 27 % faster than the commit before it, and faster than the engine before the end-of-step
> split (5–9 % on `current`), with bit-for-bit the same trajectories. The split's cost had
> been recorded as 8 %; it was about 27 % (1.17–1.40 across rounds and cases). ROADMAP H9.
> **Then, the same night, a profile and the `R0` grid** (see the follow-up section): the
> fully featured step went under the 50 µs budget on the step loop (41.9–46.0 µs); its
> criterion reading is owed.

## What was believed

`end-of-step-split.md` (scoring item 6) recorded the split's cost as new/old **1.08** on
`100S10P/current`, measured ungated on a loaded machine, and projected 47.2 µs × 1.08 ≈ 51 µs:
over the 50 µs budget by a little. That figure travelled to `pack-step-perf.md`, ROADMAP H9,
`CLAUDE.md`'s testing section, the bench's module docs and the project memory.

## What was measured

### The criterion batch could not answer

Two registered criterion batches (`W:\temp\claude\perf\REGISTRATION.md`, out of tree) ran
against `12ea82d` (the split's parent) and `e71cee5` (HEAD). The first gated nothing: two
processes outside the session held the box at ~25 %, under a 15 % gate. On the owner's
instruction — one pinned thread is barely touched by 25–40 % load on sixteen logical
processors — the gate went to 40 %, registered before any reading. The second batch then
ran three of five rounds and returned **no verdict**: one arm's admissible readings ranged
51–77 µs, and a reading CI rarely made ±1 %. What it did show was every adjacent pair with
the new arm slower, by 12–74 %.

### A long loop could

A criterion iteration is one step on a fresh clone. A loop of 1000 steps per clone, run for
four or five seconds, pinned to the quietest physical core and alternating arms, reproduced
itself to about 1 % per arm on a box that criterion could not read (the driver is
`benches/prof_step.rs` in the scratch worktree, never committed). Two cautions on what it
measures: steps are warm and back to back, which is a client's steady state rather than
criterion's one cold-cache step; and its absolutes depend on the machine's load, so only
ratios inside one round are quoted.

| arm | `100S10P/current`, µs/step, six alternating rounds |
| --- | --- |
| `12ea82d` (before the split) | 50.2 – 51.4 |
| `e71cee5` (HEAD) | 64.8 – 65.6 |

### All of it was one commit

Every engine commit from `12ea82d` to HEAD, built and looped in turn, three rounds, order
reversed each round: the step is ~51–57 µs before `905f256` (the end-of-step split) and
~61–75 µs from it on. The later slices — `Spm` end-of-step, the `Dfn` holds, the `Spm` pack
window, both porous reversals — move nothing above the noise of separately linked binaries.
One possible exception, `9643c5c`, read 4–9 % over its parent in three of four rounds of a
two-arm check, while HEAD (which contains it) read 2–8 %; that is the size code layout alone
moves a binary here, so it is recorded and not chased.

| case | split's cost, new/old inside a round |
| --- | --- |
| `current` | 1.21 – 1.40 (load-dependent: higher on a busier box) |
| `power` | 1.17 – 1.31 |
| `full` | 1.23 – 1.27 |

(Every round that had both arms, across the sweep, the pricing round and the final round.)

**So the split costs about 14 µs, not 4.** The 1.08 was a ratio taken on a box at ~60 %
load; nothing in it was wrong except the one thing that mattered.

### Where it went

Pricing arms keep the code path and stub one cost (an arm that skipped the whole shift was
also built and is **not** evidence: it put the solve back on the start-of-step line, which
changes the trajectory, not just the cost). Same round, `current`:

| arm | over `12ea82d` |
| --- | --- |
| HEAD | +32 to +40 % |
| HEAD with the slope's table search replaced by a constant | +14 to +20 % |

The split added a pass over every cell that computes `step_source_shift`, and in it
`ocv_step_slope` — a binary search of the 34-point OCV table, a chain of unpredictable
branches — on top of the search the reporting pass already made for the next step's source.
About half of the cost was that second search.

## What was built

`ecm::bracket_hinted`: `bracket`, tried first on the segment a per-cell hint names. The
check is the exact condition under which `bracket`'s search lands on `(lo, lo + 1)` — on
strictly ascending breakpoints `xs[lo] < x <= xs[lo + 1]` makes `lo + 1` the first
breakpoint not below `x` — plus `x < xs[n − 1]`, which excludes the one case that looks
interior and is not (`x` on the last breakpoint, which `bracket` returns clamped). NaN fails
every comparison. The blend weight is the same expression on the same operands. **So the
answer is `bracket`'s bit for bit whatever the hint holds**, and a stale, empty or garbage
hint costs a search, never a bit.

That is the design choice, and it is why this is a hint and not a memo. A memoised slope
written by the reporting pass would have been correct only under `SourceCache`'s invariant,
which `pack-step-perf.md` calls the riskiest thing in the pack. A checked hint has no
invariant: nothing invalidates it — not `set_cell_factors`, not a restore — and nothing
needs to.

The pack keeps the hints in `OcvHints`, a fourth kind of `#[serde(skip)]` buffer beside
`SourceCache`, `StepScratch` and `CellCurrents`: carried across steps, never serialized, and
correct in any state. Both per-cell OCV lookups on a linear pack read it — the end-of-step
slope and the reporting pass's next-step source — so the hint also speeds up the search the
engine made before the split, which is why the result beats the engine before the split.
No snapshot change, no `SNAPSHOT_VERSION` bump.

## Results

Step loop, quietest core (6), box at ~19 % total, four rounds alternating, µs/step:

| case | `12ea82d` (before the split) | `e71cee5` (HEAD) | with the hint |
| --- | --- | --- | --- |
| `current` | 50.1 – 53.4 | 64.1 – 67.7 | 47.5 – 49.2 |
| `power` | 50.2 – 55.4 | 64.2 – 65.9 | 46.0 – 46.5 |
| `full` | 56.1 – 56.7 | 70.7 – 72.1 | 52.3 – 55.3 |

With the hint over HEAD, inside each round: `current` 0.73–0.75, `power` 0.70–0.72, `full`
0.73–0.78. Over the engine before the split: `current` 0.91–0.95, `power` 0.83–0.93,
`full` 0.93–0.99.

**Criterion, registered and gated** (batch b3: gate 40 %, pinned to core 6, five rounds
alternating, round 1 discarded, minimum of the readings whose CI is within ±1 %, at least two
of them agreeing within 3 %). The arms are `12ea82d` and HEAD with the hint; HEAD without it
was not in this batch, so its criterion figure is the step loop's ratio, not a reading.

| case | `12ea82d` | with the hint | ratio | admissible readings (old / new) |
| --- | --- | --- | --- | --- |
| `100S10P/current` | 49.84 µs | **46.88 µs** | 0.94 | 4 / 2 |
| `100S10P/power` | 49.66 µs | **46.73 µs** | 0.94 | 2 / 2 |
| `100S10P/full` | 55.73 µs | **53.34 µs** | 0.96 | 2 / 2 |

The registered prediction was 0.91–0.95: `current` and `power` inside it, `full` 0.007 over.
`12ea82d` reads 49.8 µs where `846a0fc` read 47.2 on 2026-09-01 — three slices landed
between them, and the box is not the empty box of that day, so the two are not a delta.

**The budget.** `current` and `power` are now under 50 µs (46.9 and 46.7 on criterion) and
`full` is not (53.3). The budget is stated against `Pack::step` at 100S10P without a configuration,
and `full` was already over it before the split (54.6–55.1 µs on criterion, 2026-09-01). This
slice recovers the regression; it does not bring the featured case under the line. **Closed —
see "Follow-up the same night"**: the `R0`-grid change does, on the step loop.

## Verification

* `cargo test --workspace`: 86 test binaries, all green, no tolerance touched, no golden
  regenerated. A debug build checks every hinted lookup against the search (the assert in
  `bracket_hinted`), so every debug-mode test is also a hint test.
* `ecm::hinted_bracket::matches_the_search_for_every_hint` (in release too): five tables
  from one point to the shipped 34-point shape; every hint the table admits plus the
  sentinel and garbage; every breakpoint, one ULP either side, midpoints, both ends and past
  them, signed zero, the infinities and NaN; and a second lookup from whatever the first
  left behind.
* The snapshot replay tests already compare a pack carrying warm hints with one rebuilt from
  bytes, which carries none, bit for bit.
* `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean.

### Perturbation table

`W:\temp\claude\perf\perturb.py`: one wrong edit to the check at a time, `cargo test -p
sim-core --no-fail-fast` in debug and in release, the file's bytes restored after.

| # | the wrong edit | debug | release |
| --- | --- | --- | --- |
| P1 | trust the hint without checking `x` against it | **91** red | **6** red: both `hinted_bracket` tests, `a_voltage_hold_tapers_at_long_steps`, `circulation_decays_without_changing_sign`, `saturation_lands_on_the_reversal_floor`, `the_same_abuse_through_a_bms_never_gets_warm` |
| P2 | drop the `x < xs[n − 1]` guard | **36** red | **1** red: `matches_the_search_for_every_hint` |

P2's release column is the reason the unit test exists: `x` exactly on the last breakpoint
blends `ys[n−2] + 1·(ys[n−1] − ys[n−2])`, which usually rounds to `ys[n−1]` — so no
trajectory test can see it in release, and only the exhaustive comparison does.

## Follow-up the same night: the profile, and the `R0` grid

The hand arms having reached their limit, the owner approved installing the Windows
Performance Toolkit (`xperf`), which `samply` needs on Windows. Two things a next session
needs: samply's own symbolication left every function in the step loop unnamed, so the
profile was symbolicated out of tree with `llvm-symbolizer --inlines` against the PDB
(`W:\temp\claude\perf\analyze_profile.py`), and the build needs `CARGO_PROFILE_BENCH_DEBUG=true`
— `line-tables-only` produced a PDB with no function names.

`100S10P/full`, 60 000 samples at 4 kHz, share of all samples, by function the sample landed
in (inlined frames resolved):

| where | share |
| --- | --- |
| `Pack::step`'s own loops | 15 % |
| `bracket` and its binary search, **outside** the hinted OCV lookups — almost all of it the `R0` grid | ~17 % inclusive (`r0_lookup` 9.5 %) |
| `rc_step_mean_excess_v` (one division per RC pair per cell) | 5.8 % |
| the thermal integrator (`euler_substep`) | 4.7 % |

So the thermal rewrite above was aimed at under 5 % of the step, which is why it measured
nothing. The `R0` grid was the target: `r0_lookup` ran **three** binary searches per call —
the SOC axis, then the temperature axis once for each of the two rows it blends, although
both rows share that axis. Two changes, both bit-identical by construction:

* the two rows share one temperature bracket (`interp1` is `lerp_at` over `bracket`, so
  blending each row at the one bracket is the same bits);
* both axes start from a per-cell hint (`TableHints`: the OCV segment plus the two `R0`
  axes, kept in the same `OcvHints` buffer).

The division in `rc_step_mean_excess_v` is not touched: a reciprocal is not bit-identical.

**Verification.** `r0_lookup_matches_the_lookup_it_replaced` compares against a verbatim copy
of the old lookup on three grids (3×3, 1×1, 5×3), every SOC and temperature probe of the
hinted-bracket test, every pair of hints; release too. 20 000-step trajectory fingerprints
from two separately built binaries are equal in all five configurations (`current`, `power`,
`thermal`, `bms`, `full`). `cargo test --workspace` 86 binaries green, clippy and fmt clean.
Perturbations (`W:\temp\claude\perf\perturb2.py`): blending the high row at the SOC
bracket reddens **183** tests in debug and in release; **swapping the two axes' hints reddens
nothing**, debug or release — the demonstration that a hint carries no correctness.

**Speed, step loop, quietest core (6), box at ~30 %, four rounds alternating:**

| case | before (`58f2a57`) | with the `R0` change | ratio in-round |
| --- | --- | --- | --- |
| `current` | 47.0 – 48.6 µs | 35.6 – 36.1 µs | 0.73 – 0.76 |
| `power` | 48.5 µs (one undisturbed round) | 37.1 µs | ~0.76 |
| `full` | 53.6 – 55.6 µs | 41.9 – 46.0 µs | 0.75 – 0.86 |

The step loop's `full` read 52.3 – 55.3 µs where criterion read 53.3 for the same code, so on
that footing **`full` is now under the budget** — but that is an inference across
instruments. **The criterion reading is owed**: batch b4 (registered: 58f2a57 against the
change, same protocol as b3, predicted 0.75 – 0.85 / 0.80 – 0.92) returned **no verdict** —
the box was loaded enough that two rounds never gated and almost no reading had a CI inside
±1 % (the old arm read 54 – 97 µs on one case).

## Still open

* ~~**`full` is over the budget**, 53.3 µs on criterion.~~ Under it on the step loop since
  the `R0` change above (41.9 – 46.0 µs); **a criterion reading on a quiet box is owed**.
* **Pricing `full` by hand was tried the same day and reached its limit — a null, kept as
  one.** Config arms on the step loop (thermal network only, BMS only, BMS parts) read the
  thermal network as the cost and the BMS as about nothing, but in the same binary `full`
  (thermal + BMS) read ~4 µs *faster* than thermal alone, in two batches, unexplained — so
  those attributions do not hold at the few-µs level. Code arms that skipped the integrator,
  the temperature gather and scatter, or the heat push changed the pack's state too (a
  skipped integrator freezes every temperature), so they priced more than the work. On that
  footing two bit-identical rewrites were built: a branch-free update for the middle of each
  interior row of the thermal grid, and folding the temperature gather into the heat loop.
  Both were proven bit-identical — a unit test against a verbatim copy of the old update on
  nine topologies with signed zeroes, infinities and NaN, and matching 20 000-step trajectory
  fingerprints in five configurations from two separately built binaries — and both measured
  **nothing** at pack level (`full` 53.3–54.2 against 53.4–55.8 µs, step loop, same rounds),
  against a predicted 2–4 µs. The likely reason is plain: the per-cell edge branches are
  perfectly predicted, and a ten-wide row leaves eight cells to run side by side. Both were
  reverted, on the rule that a change whose effect measures nothing is removed. The
  remaining effects (1–3 µs) are below this box's round-to-round wander, so the next step is
  the profiler, not another hand arm. The fingerprint mode stays in the out-of-tree driver
  (`prof_step.rs <case> <steps> hash`): a bit-identity check across two builds that needs no
  code coupling.
* **The remaining split cost**: the extra pass over every cell, the RC loop and one division
  per cell, not separately priced. The hint now beats the engine before the split, so this
  is headroom, not a regression. (A first prototype read +3–4 % over `12ea82d` on a busy
  moment, core 5 under ~50 % box load. It is the same release code as what landed: a later
  three-arm check, prototype / landed / `12ea82d`, on core 7 at ~55 % box load, read the
  prototype and the landed code within 1 % of each other and both 0.86–0.96 of `12ea82d`.
  The +3–4 % was the moment, not the code.)
* **The `Dfn`/`Spm` paths** search their stoichiometry tables (`spm::ocp`) with the same
  `interp1`; nothing here touches them, and their budgets are their own.
* **The step loop is an instrument now** and is not in the tree. If it should be, it is a
  `benches/` target with `harness = false` and nothing else.
