# `Pack::step` performance — four items landed; budget now marginal

> **The `Cell`-size lead recorded below and in `phase-3-aging-faults.md` was STALE, and is
> now resolved — see `docs/plans/cell-size.md`.** It named `CellAging`'s accumulators as
> the bulk of `Cell`'s growth. That was true at `Cell` = 160 B and was written *before
> Phases 6 and 7 existed*: by 2026-08-12 `Cell` was **264 B**, and the largest term was
> `CellModel` at 136 B, because an enum is as wide as its largest variant and Phase 7's
> `DfnState` is 136 B. Boxing the two porous variants took `Cell` to **184 B** and the step
> **≈ −1.1 to −1.5 µs** at 100S10P (−1.4 to −1.9 % *against that session's 79 µs baseline*
> — the absolute is the half that travels, see below), with **no change at 1S1P** as
> predicted. The `CellAging` split is now **declined**: it removes nine tenths of what
> boxing removed, so change 1 bounds its benefit small, against the certain cost of a
> snapshot-layout change and a version bump.
>
> The generalisation is worth more than the win: **enum width is not an instruction**, so
> the Phase 6 and Phase 7 perf legs were right that the ECM path was unchanged and still
> missed an 88 B/cell tax. `crates/sim-core/src/pack.rs::cell_footprint` now pins it.
>
> **A second change followed and its timing is unreadable — read that as a warning about
> this file, not about the change.** `EcmState::v_rc` came off the heap into a `[f64; 2]`
> (`SNAPSHOT_VERSION` 15 → 16), taking `Cell` to **176 B**, a third below the 264 B this
> line of work opened at, and removing one heap block and one dependent load per cell per
> pass on four passes. **Ten paired alternating rounds across two batches measured
> nothing**: −13.4 % to +8.1 % on the same two binaries, sign flipping inside each batch,
> and two rounds at 173 µs and 334 µs that are contention rather than CPU state. Batch 2's
> admissibility rule was registered before it ran and **no round met it**, so it is reported
> inconclusive with no third batch, and the change is on record as landing on user direction
> plus a countable mechanism. **The point for anyone quoting a number out of this file: an
> effect this box resolved to 0.03 % one commit earlier was unmeasurable one commit later.
> Check that the base arm reproduces before trusting any ratio here.**

**Status:** all four items landed. Items 3 and 4 took the step **−34 to −43 %** against
the end-of-Phase-2 tree, which put it **inside** the < 50 µs budget for the first
time — ≈ 36–42 µs baseline, ≈ 39–49 µs fully featured, scaled to the fast-state
anchor. Ranges, not point estimates, on purpose. **Phase 3 has since spent part of that
margin — see the note below; the title of this file describes where it stands now, not
where item 4 left it.**
**Baseline commit:** `5917bd9` ("Phase 1 wrap-up: criterion benchmarks for Pack::step").

> **Superseded in part by Phase 3.** Slice E re-measured against `9da78ef`, the last
> pre-aging tree: Phase 3's four slices cost **+7–10 %** at 100S10P, which moves the
> fully-featured step to ≈ **42–54 µs** on the same scaled anchor and makes the budget
> **marginal rather than met**. The evidence, the reason the `r0_factor · soh_resistance`
> cache this doc's successors expected was *declined*, and the one open perf hypothesis
> (`Cell` grew 64 → 160 bytes; `CellAging`'s accumulators are the bulk of it and are not
> read on a non-ticking step) are in `docs/plans/phase-3-aging-faults.md` under "Learned
> while building — slice E". The methodology sections below still stand and are still
> the ones to follow — slice E discarded a round on exactly the transition they warn
> about.
>
> **Phase 4 moved it by nothing, measured.** Its slice E re-measured against `a5ce7cf`
> (the last pre-Phase-4 commit) and found `100S10P/current` at +0.9 / +0.9 / −1.6 %
> across three alternating mode-matched pairs, and `100S10P/full` at −0.6 / +2.8 /
> +1.5 %. Both straddle zero, which is what the diff predicts: Phase 4's entire
> `sim-core` change is four serde derives, two `#[serde(default)]` attributes,
> doc comments, and the `series()` / `parallel()` accessors — nothing inside `step`'s
> call graph. That leg also produced the new benching hazard below (build storms), and
> it again could **not** claim an absolute; see `docs/plans/phase-4-server-wasm.md`
> under "Learned while building — slice E".
**Owner decision needed:** none. Item 3 was taken with the design call the deferral
was waiting on — see "Items 3 and 4" below for what invariant it added and what
guards it.

## The problem (as it stood; now solved)

`CLAUDE.md` sets a budget of **< 50 µs per `Pack::step` at 100S10P** (1000 cells) on the
dev box, and says "it should be far below." The original benchmark measured **83.4 µs** —
1.7× over, not a marginal miss. Four items later it is under budget; the sections below
are in the order they were written, so read "Items 3 and 4" for the current position and
treat everything above it as the trail that got there.

Measured on the dev box (Windows, `release` + `lto = true, codegen-units = 1`),
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

## Measuring a change on this machine — read this first

**The machine is a desktop, not a laptop.** AMD Ryzen 7 7800X3D, 8 cores / 16 threads,
nominal 4.201 GHz, 96 MB L3, Windows 11 Pro 26200, Balanced power plan. Everything above
this line that calls it "the dev laptop" was stale wording, not a different box being
described; whether it is physically the same machine as the earliest sessions in this file
is not established.

### The spread is machine load, not CPU states — measured 2026-09-01

This file has said since Phase 1 that the box is **bimodal between two CPU states ~1.4×
apart**. The spread is real and it reproduces; **the mechanism was wrong**. A registered
six-round batch on one unchanged binary (`846a0fc`, `100S10P/current`, normal priority, no
pinning) had its five scored rounds read 48.5 / 51.7 / 49.7 / 50.3 / 78.9 µs — **max/min = 1.63**, squarely inside the
1.4–1.8× band this file records — while OS-reported busiest-core `% Processor Performance`
sat at **111.4–114.4 throughout**, including on the 78.9 µs round. A 2.6 % clock spread
cannot produce a 1.63× time spread, so a clock-state mechanism is **not supported**.
(`% Processor Performance` sampled at 700 ms cannot see sub-millisecond dips or SMT
contention on a sibling thread. This rules out the *stated* mechanism; it does not prove
contention is the only one left.)

What the spread does track is machine load. `_Total % Processor Time` averaged 15.4 / 32.5 /
19.3 / 28.6 / **97.8 %** across those same rounds, and the 78.9 µs round is the 97.8 % one.
Windows Defender (`MsMpEng`) gained ~310 s of CPU during the batch, and a torrent client was
resident throughout.

**A second, self-inflicted confound in the same batch:** the script called
`Get-Counter '\Processor Information(*)\…'` twice per 700 ms *while the bench ran*.
Identical work took 14–108 s of wall time. Round 4 ran 108 s at 19.3 % load and still
reported an ordinary 49.708 µs with the fewest outliers of any round — a 7.7× wall swing
with no swing in the reported quantity — but the sampler cannot be ruled out of the one
round that had a wide CI. **Do not sample anything while the bench runs.** Gate before,
never during.

### The recipe that worked — and the first directly measured absolute

Second registered batch, same binary and case, five rounds:

- **Quiet-gate before each round, never during.** Five samples of
  `\Processor Information(_Total)\% Processor Time` at 1 s; all five must read **< 15 %**.
  Retry after 20 s, up to 10 attempts. A round that never gates is **skipped, not run**.
  15 % is chosen to sit above this box's measured idle of 12.9–19.8 % and below every
  contended round above — a gate set under the idle floor can never pass, and would make
  "unmeasurable" a fact about the threshold rather than about the machine.
- **Pin to one core:** `ProcessorAffinity = 0x4` — logical processor 2, the first thread of
  core 1. Core 0 carries the interrupt load; the sibling thread (LP 3) is left free.
- **High priority**, not realtime. The standing "run cargo below normal" courtesy is the
  wrong rule here for the reason it names itself: the number *is* the deliverable.
- **Round 1 discarded as warm-up, registered in advance**, for the reason at the bottom of
  this file: this box drifts monotonically *faster* across a batch, so the earliest round is
  the least trustworthy — the opposite of the usual warm-up intuition.

| round | gate attempts | low / **point** / high | CI half-width |
| ----- | ------------- | ---------------------- | ------------- |
| 1 (discarded) | 2 | 47.492 / **47.762** / 48.088 µs | ±0.62 % |
| 2 | 3 | 47.359 / **48.753** / 50.529 µs | **±3.6 %** |
| 3 | 4 | 47.124 / **47.230** / 47.341 µs | ±0.24 % |
| 4 | 9 | 49.583 / **49.981** / 50.438 µs | ±0.86 % |
| 5 | 10 → **never gated, skipped** | — | — |

Registered estimator: **the minimum** of the point estimates, not the mean or the median.
Every mechanism left — contention, interrupt service, cache eviction by other processes —
can only *add* time, so with one-sided noise the minimum is the least-contaminated reading,
and "can this code hit 50 µs" is a question about the floor.

**Floor: 47.2 µs at `100S10P/current`, CI ±0.24 %.** That is the first *directly measured*
absolute in this file since Phase 1 — see "What this corrects" below.

Three honest notes on that table, none of which change the verdict:

- **Round 2's CI is ±3.6 %**, which the first batch's own rule would have called wide. The
  registered estimator scores only the minimum round's CI, so it does not block — recorded
  here so it is clear the tidier rule was not chosen after seeing the numbers.
- **Pinning is not what tightened it.** Unpinned rounds 2–5 gave max/min 1.066; pinned rounds
  2–4 give 1.058. The pin bought ~2 % on the floor and nothing on the spread. What the
  *gate* bought was keeping a 78.9 µs round out of the sample at all.
- **The quiet window closes.** Gate attempts went 2 → 3 → 4 → 9 → never, monotonically over
  about twenty minutes. A longer batch would have gated *less*, not more. Budget a
  measurement for the first fifteen minutes of a quiet machine, and expect fewer usable
  rounds than you planned.

### What this corrects

The position this file carried into 2026-09-01 was **"≈ 36–42 µs baseline"**. That number was
never measured: it was a ratio scaled through a fast-state anchor which was itself scaled
from an earlier one — three chained scalings deep — and it lands about **15 % optimistic**
against the first direct reading. The *ratios* in this file are sound and remain the unit of
account; the absolutes derived from them by scaling were not, and should not be quoted again
without a direct reading behind them.

**Six sessions concluded this box could not be measured on.** That conclusion was about the
protocol, not the hardware. A quiet gate plus a core pin gated in two attempts and produced
a ±0.24 % interval. The "fast state" this file records the box as having failed to reach for
five consecutive sessions looks like it was simply an empty machine.

### The fully-featured figure is now an open question

The measurement above is `100S10P/current` — `bms: None`, `Isothermal`. `CLAUDE.md` states
the budget against `Pack::step` at 100S10P without that qualification, and this file's
headline also claimed **≈ 39–49 µs fully featured**. That figure was 36–42 plus a
current↔full delta of ~3–7 µs. The base term has now moved up by ~5 µs and **the delta has
not been re-measured**, so the honest arithmetic is 47.2 + (3–10) = **roughly 50–57 µs, at or
over the line**.

It may well be better than that: six of the eight per-step allocations removed by `fd3f162`
were on the features-on paths, and that slice deliberately made no timing claim. But nobody
has run it. This is a stated open item, not a hole — one
`cargo bench -p sim-core --bench pack_step -- "100S10P/(current|full)"` under the recipe
above answers it, with both cases in one invocation so no pairing is needed.

**Attempted the same day — and it did not clear its own bar. Verdict: UNGATED.** A third
registered batch (four rounds, same gate, same pin, both cases in one invocation, minimum
estimator, round 1 discarded) required **three gated rounds** to return a verdict. **Two
gated.** Rounds 1 and 3 burned all ten gate attempts and were skipped, not run — the
closing-window effect above, arriving on schedule. So there is no verdict, and the
arithmetic in the paragraph above remains this file's position.

The readings are recorded anyway, for one reason worth stating precisely: **the batch
reproduced its own null.** `100S10P/current` read **47.126** and **47.306 µs** here against
the **47.230 µs** scored by the separate batch above — 0.2 % agreement across two
independently registered batches, on the exact quantity the earlier one scored. This file's
own rule is that an instrument check licenses the minute it ran in, so the null belongs
*inside* the batch; that is satisfied here for the first time. It is why the readings are
worth writing down. It is not why they would be a verdict — the round count is why they
are not one.

| case | round 2 | round 4 |
| ---- | ------- | ------- |
| `100S10P/current` | 47.126 µs (±0.8 %) | 47.306 µs (±0.6 %) |
| `100S10P/full` | 55.121 µs (±2.0 %) | 54.599 µs (±0.7 %) |
| `100S10P/full+aging` † | 59.220 µs | 57.283 µs |
| `100S10P/full+aging_every_step` † | 83.153 µs | 80.024 µs |

† **Unregistered.** The regex filter caught four cases, not the two the registration named.
No estimator was declared for these and no prediction was made about them, so they are
observations, not results — a number nobody registered is exactly the kind a story gets
fitted to afterwards. `aging_every_step` is additionally **not a shipped configuration**:
the aging sub-clock runs one step in a hundred at the shipped 10 s period against a 0.1 s
`dt`, so that row prices the tick, not the engine.

**Current↔full delta, within-round only** — the two readings have to come from one
invocation or they name instants that never coexisted: **8.00 µs** (round 2) and **7.29 µs**
(round 4). Mid-range of the historical 4–10 µs, and inside the 3–10 the arithmetic above
assumed, so **that arithmetic needs no revision.**

**A prediction that came out right off an argument that was wrong — recorded because the
argument will be offered again.** The registration predicted 51–55 µs *and* a delta at the
low end of its historical range, 4–6 µs, reasoning that six of the eight allocations
`fd3f162` removed sat on the features-on paths. The band was right, at its top edge. The
delta was **not narrowed at all**. Removing those allocations did not measurably close the
current↔full gap, so the reasoning that produced the correct band should not be trusted the
next time it is put forward.

### Still true: never trust a saved baseline

The single most expensive mistake available here is trusting a criterion baseline saved in
an earlier session, and nothing above changes that. A cross-session `--baseline` comparison
of exactly the change below reported **−8.6 %**; the paired measurement puts it at
**−28.5 %**. The first number nearly got the allocation hypothesis written off as refuted.

So: run both revisions back to back (`git stash push -- crates/sim-core/src/ecm.rs`,
bench, `git stash pop`, bench), repeat until two rounds agree, and **discard any round
where either arm's confidence interval is wide** — a wide CI is the signature of a machine
in transition, not of sampling error. Wall-clock cost is a few minutes; the alternative is
a wrong conclusion about your own change.

**Do not bench in the minutes after a large build.** Phase 4 slice E watched the same
tree read 45.8 → 82.3 → 62.7 → 49.6 µs on `100S10P/current` — a 1.8× spread, wider than
anything else in this file — starting immediately after `wasm-pack build` plus two
`cargo build --release` runs wrote several thousand files, with `MsMpEng` (Windows
Defender) at the top of the CPU table for the duration. It settled on its own within a
few minutes. This was the first time the spread had an identifiable trigger rather than
being weather — and the 2026-09-01 measurement above says load is the whole story, and it is the cheapest one to avoid: build first, then wait,
then bench. The wide-CI rule catches it either way, at the cost of three discarded
rounds.

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

## Items 3 and 4, measured — **−4 % then −35 %, and the budget is met**

Landed as two commits on purpose, so the two effects are separately attributed
rather than repeating items 1–2's "measured together, split unknown" regret:
`4a268e9` (item 4, flat scratch buffer) then `fa9ec20` (item 3, cross-step memo).
Three arms — `580131b` (end of Phase 2), item 4, item 3 — benched from three
worktrees, one case per arm, **arm order alternated between rounds** so the
last-arm-runs-hottest effect cannot masquerade as a win. Eight rounds; the first
five were discarded under the wide-CI rule (one round had item 4 measuring *above*
the baseline). Rounds 6–8 came in at ±1.3–1.6 % on every arm:

| pair | 100S10P/current | 10S10P/current |
| ---- | --------------- | -------------- |
| base → item 4 | −2.8 %, −3.4 %, −6.7 % | −6.8 % |
| item 4 → item 3 | −31.3 %, −37.8 %, −39.2 % | −38.1 % |
| base → both | −33.7 %, −39.5 %, −43.3 % | −42.5 % |

Also `100S10P/power` −31.2 % (item 4 → item 3, same invocation, so no pairing needed)
and `1S1P` 215 → 158 ns. `100S10P/full` is treated separately below — it does **not**
get a clean ratio, and the reason is instructive.

**Item 4 is worth ~4 %** — real but barely above this machine's noise floor, and
exactly the "correspondingly smaller win" this file predicted for removing 100
allocations rather than 2000.

**Item 3 is worth ~35 %**, more than "halves the per-cell work" would suggest given
that it removes only one of the two `cell_source` passes. The arithmetic does close,
though, and it closes on **division latency** — which is checkable by reading
`ecm.rs` rather than by profiling:

- One `cell_source` performs **four** `f64` divisions: `bracket` has one
  (`(x - xs[lo]) / span`), so `r0_lookup` — one `bracket` plus two `interp1` — is
  three, and `ocv_lookup` is one more. They are largely dependent, and division is
  ~15–20 cycles of latency, so ~20–27 ns/cell at 3 GHz.
- Measured saving is `(80.43 − 48.90) / 1000` = **31.5 ns/cell** (round 8, both arms
  in the same round). The two binary searches — 34-point OCV, 3-point `R0` — and the
  `v_rc` sum cover the rest.
- The residual is consistent too: 80.43 − 31.5 (removed) leaves ~49 ns/cell, of which
  ~31.5 ns is the *surviving* `cell_source` in the reporting pass, leaving ~17 ns for
  `exp()`, `coulomb_step`'s division, `cell_heat_w`, and a `docv_dt_lookup` that
  short-circuits to `0.0` because the bench chemistry has no entropy table.

The obvious follow-on — precomputing `1/span` at load time so the lookups multiply
instead of divide — is **not bit-identical** (`x * (1/s)` ≠ `x / s`), so it is not a
free fifth item; it would need the same equivalence-test treatment as items 1–2, and
it would fail it.

An earlier draft of this section blamed data layout (`Vec<Vec<f64>>` pointer chasing,
`v_rc`'s per-cell heap allocation). That was unprofiled *and* probably wrong: the
`R0` rows are three shared allocations that go hot after the first few cells, and
`v_rc` is touched exactly once cold per cell in **both** arms, so it cannot account
for a difference. Countable beats plausible.

Scaling the measured ratio range through this file's fast-state anchor (≈ 64 µs
baseline at the end of Phase 2) puts a baseline step at **≈ 36–42 µs — inside the
< 50 µs budget**, from 1.28× over. Stated as a range on purpose: base → both
measured −33.7 / −39.5 / −43.3 %, and a point estimate would be false precision 11 µs
from the line. The absolute is a *scaled* figure — every reading in this session sat
in the machine's slow state (base measured 83–95 µs against its documented ~64 µs
fast-state value) — so the ratios are the measured quantity, as always in this file.

### The fully-featured figure is weaker, and here is exactly how weak

`100S10P/full` deserves its own treatment because **the percentage overhead is not
comparable across items 3–4**: item 3 made the shared electrical work ~35 % cheaper
while leaving the thermal integrator and every BMS path untouched, so the same
absolute feature cost is mechanically a larger fraction of a smaller baseline. Both
cases run in one invocation, so no pairing is needed — but one invocation is one
sample, and the first attempt (−37.3 %, from a single pair) came from the same
invocation whose `10S10P` reading had to be discarded as a mode artifact.

Five same-invocation runs on the final tree, two discarded for wide CI:

| run | `current` | `full` | Δ | ratio |
| --- | --------- | ------ | - | ----- |
| 1 | 55.64 µs | 59.38 µs | 3.74 µs | +6.7 % |
| 2 | 52.06 µs | 59.30 µs | 7.24 µs | +13.9 % |
| 4 | 64.42 µs | 74.22 µs | 9.80 µs | +15.2 % |

The **absolute delta is the quantity to carry forward**, not the percentage: across
every measurement ever taken it sits at 4.0 and 7.0 µs (Phase 2), 12.7 µs (item-4
tree), and 3.7 / 7.2 / 9.8 µs (final tree) — one noisy quantity of **≈ 4–10 µs**, not
a feature cost that grew. Mode-scaled onto the fast-state anchor that is ~3–7 µs, so
a fully-featured step is **≈ 39–49 µs**.

That is under budget across the whole range, but the upper end has thin margin, and
this file should not pretend otherwise. If a tighter fully-featured number ever
matters, it is one `cargo bench -p sim-core --bench pack_step -- "100S10P/(current|full)"`
repeated on a quiet machine — not another code change.

### What item 3 costs in invariants

The memo (`SourceCache` in `pack.rs`) is a memo of a pure function of pack state,
not state: entry `i` is `cell_source(cell_i.state(), chem, cell_i.r0_factor)`, and
**empty means cold means recompute**, so every way of getting invalidation wrong in
the conservative direction costs speed, not correctness. The other direction is a
silent physics bug, and is guarded twice:

- a `debug_assert` on the warm path compares the memo against a fresh compute, every
  cell, every step — so every debug-mode test in the suite is a staleness check;
- `crates/sim-core/tests/thevenin_cache.rs` runs one trajectory twice, warm and with
  the memo dropped before *every* step, comparing `Telemetry` **and** per-cell ground
  truth as raw bits (aggregates could cancel a single-cell divergence).

Injecting a one-ULP stale entry fails both. It does **not** fail the goldens or the
proptests, which is the entire reason that test file exists.

Two consequences to know:

- `SourceCache`'s `PartialEq` is deliberately always `true`. Two packs with equal
  state are equal whether or not one has a warm memo, and a serde round-trip
  deliberately produces a cold one, so anything else would make
  `snapshot != roundtrip(snapshot)`. The price is that
  `zero_length_step_does_not_mutate_state` can no longer see memo corruption; the
  `debug_assert` is what pays for it.
- **No `SNAPSHOT_VERSION` bump**, and that is correct rather than another missed one:
  the field is `#[serde(skip)]`, emits no bytes, and v5 blobs are byte-unchanged. A
  restored pack starts cold and recomputes, which is by definition what the memo
  would have held.

`Pack::set_cell_factors` is the only invalidation point outside `step` today.
**Phase 3 must add its own**: anything that mutates a cell's state or `r0_factor`
from outside `step` — the fault queue, `WeakCell`, a soft internal short — has to
clear it.

**Aging (Phase 3 slice A) is settled and needs no invalidation.** It runs inside
`step`, between the thermal integration and the end-of-step reporting pass, so the
pass memoises post-aging sources. The invariant was also restated rather than merely
preserved: entry *i* is now `cell_source(state, chem, cell.eff_r0_factor())`, where
that method is `r0_factor · soh_resistance`. The `debug_assert` checks the composed
product, so every debug-mode aging test is a staleness check on the sequencing. If a
later slice ever moves the aging update outside that window it needs an invalidation
like any other outside mutation.

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

### 3. Cache per-cell `(E, R)` across the step boundary — **DONE, −35 %**

Landed as `fa9ec20`; measurement and the invariants it added are in "Items 3 and 4"
above. It was indeed the biggest lever, by a wide margin. The rest of this section
is the original design note, kept because its cost analysis is what the
implementation answers point by point.

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

### 4. Drop the per-group scratch `Vec` in `step` — *bit-identical* — **DONE, −4 %**

`step` built `cell_src: Vec<Vec<(f64, f64)>>` fresh every step: one inner `Vec` per
series group, so ~102 allocations per step at 100S10P. It is now one flat
`Vec<(f64, f64)>` indexed `g * parallel + k` (`4a268e9`), which item 3 then promoted to
a pack-owned buffer that allocates nothing at all on a warm step.

- **Outcome: ~−4 %**, as predicted for two orders of magnitude fewer allocations than
  item 1 removed. Barely above this machine's noise floor; it was landed *before* item 3
  and benched separately precisely so item 3 could not absorb it uncredited.
- Bit-identical by construction: same values, same order, same arithmetic, only the
  storage changed.
- `group_src` (one `Vec` of `series` entries) is still allocated per step. That is 1
  allocation, not 102, and is not worth another commit on its own.

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

Current position (2026-09-01, **directly measured** rather than scaled): **47.2 µs at
`100S10P/current`, under the 50 µs budget by ~6 %.** The **fully-featured figure is an
open question** — 47.2 plus an un-re-measured 3–10 µs delta puts it at roughly 50–57 µs, at
or over the line. See "Measuring a change on this machine" above for the number, the
recipe that produced it, and the arithmetic. The older "≈ 36–42 / 39–49 µs" position was
scaled through three anchors, not measured, and ran ~15 % optimistic. All four items are
done and separately attributed. Nothing here is blocking.

Two levers were identified and not taken. **Both are now closed** — the first was
taken, the second declined — by `docs/plans/pack-step-allocations.md` on 2026-09-01:

- ~~**The remaining per-step allocations.**~~ **TAKEN.** `group_src` always, plus
  `heat_w`, `temps` and `scratch` (three ~8 KB `Vec`s at 1000 cells) whenever the
  thermal network is live, plus `bleed_g` and `v_group` with a BMS — and an eighth this
  list missed, the `temp_probe_k` the caller had to build fresh because `Bms::sample`
  took its two `Vec`s by value. All eight are gone. Counted, not timed: a fully-featured
  step allocated **7 blocks / 27,216 bytes** at 100S10P and now allocates **nothing**,
  pinned by `crates/sim-core/tests/step_allocations.rs` and proven trajectory-neutral by
  a byte-identical 1846-line run of the out-of-tree instrument. **No timing claim was
  made**, deliberately — see that document for why the clock could not have answered it.
- ~~**Multiply-by-reciprocal in the lookups.**~~ **DECLINED, on the record.** It is
  where the remaining time demonstrably goes (see the division count above) and it is
  not bit-identical, so it trades this repo's exact-replay spine for single-digit
  percent on a box that cannot measure single-digit percent. If it is ever taken it is a
  declared numerical change with the goldens regenerated, not an optimisation.

Beyond those, get a profiler before guessing a fifth item.

**The thing most likely to break next is not speed, it is item 3's invariant.** The
memo is correct only while every mutation of a cell's state or `r0_factor` from
outside `step` clears it. Phase 3 slice A cleared aging off that list (it mutates
inside `step`, before the reporting pass — see above), but the fault queue,
`WeakCell`, and soft internal shorts all still mutate from outside. Read "What item 3
costs in invariants" above before adding any of them. The `debug_assert` will catch a miss in any debug-mode
test, which is the safety net, but it is not a substitute for knowing the rule.

Two things to know before the next re-measure:

- **Use worktrees, not `git checkout` round-trips**, and filter to one benchmark case so
  each arm is ~10 s. `git worktree add <tmp> <rev>` gives each revision its own target
  dir, so alternating runs cost only the bench itself. This is what finally produced a
  usable pair for the thermal measurement after two full-suite rounds were unusable.
- **The machine's state persisted across an entire session.** The spread is not
  per-run jitter that averages out over a few minutes; plan on reporting a *ratio* plus
  a mode-matched anchor rather than an absolute number. It does sometimes settle mid
  session: the items 3–4 measurement went from unusable (±5 % CIs, arms disagreeing on
  sign) to ±1.3 % across eight rounds without anything changing but time.
- **Alternate the arm order between rounds.** Running base → change → change every time
  means the last arm always runs on the hottest CPU, which is a free ~few-percent bias
  in favour of whatever is measured last. Items 3–4 were confirmed with the changed arm
  running both first and last.
- **Warm the bench's clone template.** `iter_batched_ref` clones a template per
  iteration, so a never-stepped template measures the *first* step a pack ever takes.
  That priced item 3 at exactly zero until it was fixed — the memo is cold on step one
  by definition. `benches/pack_step.rs`'s `warmed()` does this; if a future change makes
  a step depend on prior steps in some other way, check that helper first.
- **An instrument check licenses the minute it ran in, not the session — so interleave the
  null, do not front-load it.** On 2026-08-12 a front-loaded check passed cleanly (three
  readings of one unchanged binary at 72.6 / 70.2 / 71.6 µs, all CIs ≤ ±0.77 % — the first
  time in six sessions this box reproduced itself). The measurement it licensed ran fifteen
  minutes later on the same box, same case, same protocol, and produced CIs of ±0.99–2.85 %
  with a **53 % spread on one unchanged binary**. Both facts are true and they do not
  contradict each other: reproducibility is a property of the minute. The fix is to make the
  check and the measurement share conditions instead of merely sharing a session — put an
  unchanged reference arm *inside* the batch, run partway through, and score the batch
  against it. A batch that cannot reproduce its own null is unreadable no matter what the
  pre-flight said. See `docs/plans/cell-size.md`, "Timing, batch 3".
- **The two worktree builds this protocol needs are themselves the disturbance, and 180 s
  does not absorb them.** Across that same batch the box got monotonically *faster* —
  78 → 55 → 52 → 51 µs on the base arm over about fifteen minutes — which is background load
  draining after the builds, not thermal behaviour. Consequence for round ordering: the
  earliest rounds are the least trustworthy, and the delta shrank in lockstep with the
  baseline (−17.4 → −6.0 → −4.1 → −2.9 %), which is exactly the shape a drifting box
  produces. Either settle far longer than 180 s, or build in a prior session, or accept that
  round 1 is a warm-up and register it as discarded *before* seeing it.


## Slice A (aging) added overhead; magnitude not established

`eff_r0_factor()` puts one extra multiply on every `cell_source` call — twice per cell
per step, unconditional — plus two branch-gated accumulations. Paired alternating
worktree runs on `100S10P/full` put slice A above `HEAD` in all three passes (57.0 vs
56.6, 60.7 vs 52.0, 52.9 vs 49.1 µs), so the sign is known and the magnitude is not:
the noise band was wider than the effect.

**That session's box could not verify the budget in either arm.** The *baseline*
measured 49–57 µs where this document records 39–49 µs, i.e. the machine was running
~25 % slow, so no absolute statement about the 50 µs budget can be drawn from those
numbers — including the apparent overrun. Phase 3 slice E owns the honest re-measure.

If the overhead does need removing, the fix is to store `r0_factor · soh_resistance`
as a derived field on `Cell`, refreshed at the aging tick and in `set_cell_factors` —
the same invariant-with-a-`debug_assert` shape item 3 already uses, and the same
obligation: measure first, because it buys a field that can go stale.
