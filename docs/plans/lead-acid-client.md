# The lead-acid client slice: a cell that is not empty, and will not give you the rest

The last item `docs/plans/diffusion-overpotential.md` closes with:

> **Not reachable from any client.** The chemistry is loadable by id, as it was before, and
> no scenario, page or catalogue entry uses it. Presenting this cell in the guided path is
> a UI slice; it is now worth doing, which it was not before.

Not a numbered phase. The phased plan in `CLAUDE.md` is exhausted — Phase 7 went past it and
closed — so this is the client catching up to an engine change, the same shape as
`docs/plans/spm-scenario.md` (Phase 6) and `docs/plans/dfn-scenario.md` (Phase 7).

## What is missing, stated as source

`chemistries/pba_agm_2v_generic.toml` has shipped since `lead-acid-data-only.md` and gained
a fitted `[diffusion]` section in `diffusion-overpotential.md`. A `grep` for `pba_` across
the repo returns the chemistry file, `crates/sim-core/src/ecm.rs`, and two test files.
**No scenario names it, so no picker entry, no page and no lesson can reach it.**

The prior objection to a lesson is in `lead-acid-data-only.md` and is the thing the last
three commits removed:

> a teaching page that presented this cell's rate behaviour as accurate would be presenting
> the half the engine gets wrong.

## Why this cell earns three steps

Peukert's law is the most familiar fact about a battery that most people have actually
owned: a car battery that cranks fine in summer and not in the cold, a UPS that claims
thirty minutes and gives eight. The engine now reproduces it, and — unlike the two porous
models — the reason is legible in one line of algebra rather than four coupled PDEs.

And the lesson has a second half no other chemistry in this repo can teach: **the charge
that would not come out is still in there, and a rest gets some of it back.**

## What is already committed, and what that buys

This slice starts from a better place than either porous-model slice. Both of its arms and
its rest recovery are **already asserted** by `crates/sim-data/tests/lead_acid_rate.rs`:

| what | where |
| ---- | ----- |
| the rate sweep 0.05C → 3C, worst error vs Peukert n = 1.1 under 3.5 points | `the_diffusion_term_tracks_peukert` |
| the C/20 delivery within a few points of the declared 7.2 Ah | same test's absolute anchor |
| a rest returns real capacity, and a **harder** discharge recovers **more** | `a_rest_recovers_capacity_and_a_harder_discharge_recovers_more` |
| the control arm ( `[diffusion]` stripped ) still badly wrong | both |

So unlike the DFN pair — where one arm had a golden and the other had a sentence
apologising for not having one — **this slice has no asymmetry to confess.** Every number a
lesson quotes sits inside a range a committed test already fences.

The harness that produced the tables below is a scratch bin crate outside the repo
(`M:\claud_projects\temp\leadacid-measure`, path deps on `sim-core`/`sim-data`), not a test.
It agrees with the committed test where they overlap: the second discharge after a four-hour
rest delivers **32.2 %** of the first, which is `a_rest_recovers_capacity…`'s own 3C/4 h
figure reproduced through the page's `Pulse` demand rather than through the test's helper.
That agreement is the check that the harness measures the quantity the repo already asserts.

## The measurement, before a word of prose

All 1S1P, isothermal, 25 °C, no BMS, no aging, from 100 % SOC, `pba_agm_2v_generic`,
`dt = 0.5` (the page's default). "Cut-off" is the first step whose terminal falls below the
chemistry's own `v_min = 1.75 V`.

| rate | I \[A\] | cut-off | panel shows | soc left | delivered | % of C/20 |
| ---- | ------- | ------- | ----------- | -------- | --------- | --------- |
| C/20 | 0.360 | 69620.5 s | `19.3h` | **3.3 %** | 6.9620 A·h | 100.0 |
| C/10 | 0.720 | 33626 s | `9.3h` | 6.6 % | 6.7250 A·h | 96.6 |
| C/5 | 1.440 | 15694 s | `4.4h` | 12.8 % | 6.2772 A·h | 90.2 |
| C/2 | 3.600 | 5489 s | `91m` | 23.8 % | 5.4880 A·h | 78.8 |
| 1C | 7.200 | 2527 s | `42m` | 29.8 % | 5.0520 A·h | 72.6 |
| 2C | 14.400 | 1181 s | `20m` | 34.4 % | 4.7200 A·h | 67.8 |
| 3C | 21.600 | 737 s | `12m` | **38.6 %** | 4.4190 A·h | 63.4 |

**The headline is the last column but one.** A slow discharge stops with 3 % of the cell's
charge unused; a hard one stops with **more than a third of it still there**. Peukert as a
capacity ratio (63.4 % against the law's 66.3 %) is the same fact in the units a datasheet
uses, and it is the quantity the committed test fences — but "a third of it is still in
there" is what a reader can read off one row of the panel.

### The control arm, which is not client-reachable and is the argument for the file

Same runs with `[diffusion]` deleted — the model exactly as it shipped before the last slice:

| rate | cut-off | soc left | delivered |
| ---- | ------- | -------- | --------- |
| C/20 | 20.0h | 0.00 % | 7.2142 A·h |
| C/5 | 5.0h | 0.00 % | 7.2124 A·h |
| 1C | 60m | 0.00 % | 7.2040 A·h |
| 3C | 17m | 15.92 % | 6.0480 A·h |

It empties completely at every rate up to 1C and loses nothing at all. That is the "none of
it below 1C" finding from `lead-acid-data-only.md`, and it is why the section exists.

### Timestep sensitivity: this cell is not the DFN

| rate | dt = 0.5 | dt = 2 | dt = 10 |
| ---- | -------- | ------ | ------- |
| C/20 | 69620.5 s, 3.30 % | 69622 s, 3.30 % | 69630 s, 3.29 % |
| 3C | 737.0 s, 38.58 % | 738 s, 38.50 % | 740 s, 38.33 % |

Twenty-fold in `dt` moves the slow arm's cut-off by 9 s in 69 621 and its charge readout by
a hundredth of a point. So `dt = 0.5` is affordable even on the 19-hour arm (139 241 steps,
~7 ms of arithmetic) and **both lessons share the page's default**, which is the strongest
version of "the two runs differ in exactly one field".

### The overpotential tile is NOT a clean read of the fitted term

`CellView::overpotential_v` — the pack grid's `overpotential [mV]` metric — is
`Σ V_rc + η`: a **placeholder** RC pair plus the **fitted** diffusion term. Read out of the
snapshot rather than reasoned about:

| soc | slow: rc / η / tile \[mV\] | fast: rc / η / tile \[mV\] |
| --- | -------------------------- | -------------------------- |
| 90 % | 1.44 / 1.93 / 3.37 | 34.10 / 4.05 / 38.15 |
| 75 % | 1.44 / 2.81 / 4.25 | 61.70 / 12.64 / 74.34 |
| 50 % | 1.44 / 4.32 / 5.76 | 79.32 / 46.25 / 125.57 |
| 38.6 % | 1.44 / 5.65 / 7.09 | 82.39 / **101.90** / 184.29 |

On the fast arm at its cut-off **45 % of the tile is the placeholder**, so no lesson may
point at that number and call it the mechanism. On the slow arm it is 0.7 % placeholder and
the tile is the term.

The clean separation is in the **rest**, and it falls out of the two time constants the
chemistry file deliberately separates (240 s against 4218 s):

| into the rest | terminal | rc \[mV\] | η \[mV\] |
| ------------- | -------- | --------- | -------- |
| 0 (still loaded) | 1.750 V | 82.39 | 101.90 |
| +0.5 s | 1.848 V | 82.22 | 101.87 |
| +4 min | 1.912 V | 30.31 | 89.73 |
| +30 min | 1.984 V | **0.05** | 47.66 |
| +60 min | 2.005 V | 0.00 | 27.02 |
| +4 h | 2.030 V | 0.00 | 1.74 |

**Half an hour in, the RC pair is spent and the fitted term still has 47.7 mV to give.**
After that instant the tile *is* the diffusion overpotential, and the voltage goes on
climbing for hours. That is the two-stage recovery the chemistry file promises, and it is
the one place in this slice where a reader can watch the fitted mechanism alone.

### Rest recovery through the page's own `Pulse` mode

737 s on at 3C, 4 h off, `dt = 0.5`:

* leg 1 ends **exactly at its cut-off**, 1.750 V with 38.6 % showing;
* `soc (true)` is **flat at 38.6 % for the whole four hours** — nothing is added;
* leg 2, the identical demand, reaches 1.750 V again after **237.5 s**, at 18.8 %;
* leg 2 delivered **1.4250 A·h against leg 1's 4.4190** = **32.2 %**.

### What the panel prints at each of the three marks

Read through the page's own formatters, because the panel's precision is not the engine's:

| mark | `sim time` | `terminal` | `soc (true)` | `heat` | engine |
| ---- | ---------- | ---------- | ------------ | ------ | ------ |
| step 22, C/20 | `19.3h` | `1.750 V` | `3.3 %` | `0.07 W` | 1.749774 V, 3.304861 % |
| step 23, 3C | `12m` | `1.750 V` | `38.6 %` | `6.09 W` | 1.749968 V, 38.583333 % |
| step 24, leg 2 | `4.3h` | `1.750 V` | `18.8 %` | `5.20 W` | 1.749601 V, 18.791667 % |

All three marks print the same three-decimal `1.750 V`, which is what makes the three
readings comparable on the one row a reader is looking at.

**`19.3h` is not 20 h.** The twenty-hour rate is the datasheet's convention and the control
arm's answer; this cell stops a little short of empty at every current, and prose must not
let the panel imply otherwise.

**Heat is a free second channel**, as it was in the DFN slice: 6.09 W against 0.07 W at the
same state of charge, from the same cell — 87×, and `q_gen` is current times the gap between
equilibrium and the terminal, which is exactly where the disagreement lives.

## Design decisions

**One scenario file, not three.** A scenario is an initial condition, not a demand program;
the rate and the pulse train are the client's business. All three lessons name the same
file and differ only in the demand box, which is the strongest form of the house idiom.

**Append at 22, 23, 24 — do not insert.** Step 1's `expect` cites "step 20 of this path, and
step 21", step 2's cites "step 20", and both steps carry claims, so an insertion would
redden the claims suite and not merely the prose. Every digit citation in `web/app.js`
points at steps ≤ 21; verified by grep before and after, not by this paragraph.

**No BMS, and mark by time.** Letting under-voltage protection stop the run was considered
and rejected: the graduated response derates before it opens, a derate clamps the `Current`
demand, and a lower current *relaxes* the depletion state — so the cell would deliver more
and the contrast would shrink in exactly the arm meant to show it.

**Each mark sits at that arm's measured cut-off.** Past it this cell collapses hard: the
overpotential saturates at `max_overpotential_v` and the 3C arm reaches **−2.36 V** by
`soc = 0`. That is `low-clamp-reversal.md`'s territory and steps 20 and 21's subject, and a
reader meeting it here — at 25 % state of charge — would read it as contradicting them.

**No charge leg on this chemistry.** Steps 20 and 21 both end by telling the reader to put
the demand box to −2 and press Run. These must not. The diffusion term's charge direction
is unvalidated and `crates/sim-core/src/ecm.rs` says so; a lesson that instructed a charge
would be teaching from the one part of this mechanism nobody has measured.

## Part A — the scenario file

`scenarios/cc_discharge_pba.toml`. 1S1P, `initial_soc = 1.0`, 298.15 K,
`ThermalConfig::Isothermal`, seed 0, no scatter, no BMS, no aging, no faults — the same
shape as the three other `cc_discharge_*` files, so it is comparable to them by
construction.

Its comment carries, in the house shape: which parts of the chemistry file it reads and
their provenance (**`[diffusion]` fitted, `[ocv]` conventional, `[r0]` and `[[rc]]` labelled
placeholders**); what it shows; and the four live limitations, none of which may be papered
over — the 3.3 % delivery shortfall at C/20, the unvalidated charge direction, the
isothermal sweep, and `[r0]`'s low-SOC rise still being placeholder shape.

## Part B — three lessons

* **Step 22 — the slow discharge.** `Current` 0.360 A, `dt: 0.5`, `until_s: 69620.5`,
  `reload: true`. The cell delivers essentially all of its charge and stops with 3.3 %
  showing. This is the arm that **looks unremarkable**, and it must not be spoiled: it is
  the reference the next step is measured against.
* **Step 23 — the same cell, sixty times the current.** `Current` 21.600 A, same file, same
  `dt`, `until_s: 737`. Same cut-off voltage, and **38.6 % of the charge is still in it**.
  The prose states the tile split rather than pointing at the tile.
* **Step 24 — and it is still in there.** `Pulse` 21.600 A, 737 s on, 14400 s off,
  `until_s: 15374.5`. The rest, the flat charge readout, the two-stage recovery, and the
  second leg reaching the same cut-off 237.5 s later with 1.4250 A·h delivered.

`speed_x` per step is set from the run length; step 22 needs the slider's maximum (10000,
which step 8 already uses) and must be confirmed **on the page** rather than by arithmetic.

## Part C — the prose sweep, by capability not by literal

The capability that changes is **"a client can reach the lead-acid cell"**. Grepping `pba_`
would miss most of it. Candidates, each because of what it *claims*:

* `chemistries/pba_agm_2v_generic.toml` — its header, which describes a file no client uses;
* `docs/plans/lead-acid-data-only.md` and `docs/plans/diffusion-overpotential.md` — the
  bullets deferring exactly this;
* `README.md` — **"Three chemistries ship"** is already stale (there are four), the claims
  count says "forty-eight" against 49 in the file, and "twenty-one steps" appears twice;
* `web/index.html` — the pre-script `Start — 21 steps` fallback, which has drifted before;
* `crates/sim-core/src/ecm.rs` — the diffusion functions' docs, if any claim no client
  reaches them;
* the neighbouring lessons, steps 20 and 21, which are about a cell driven *past* empty and
  now sit beside three about a cell that stops *short* of it.

## Verification

1. `cargo test --workspace` — `every_shipped_scenario_parses_builds_and_steps` picks the new
   file up automatically. `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo fmt --all`. All below normal process priority.
2. **Drive the page.** Headless Chrome over CDP. Every number in every `expect` block read
   off the panel, never off this harness. Confirm step 22 is reachable in wall clock.
3. **Walk the path in both directions** across steps 20–24, reading 20's and 21's numbers on
   the way past each time: they are `dt`-pinned, and this slice is the first thing appended
   after them.
4. Confirm the picker lists the new file with no page edit (`GET /scenarios` fills it).
5. Claims: extend `web/path-claims.toml`. Budget 15–25 across the three steps.

## Versions

Expected to move: **nothing**. `SNAPSHOT_VERSION` 17, `API_VERSION` 2, `WASM_API_VERSION` 6
— one TOML file, three JavaScript records, claims and prose. Each constant's own doc gets
read individually anyway; that pair has parted once (`ui-bms-view`).

## Deferred, with a price

* **A 6S 12 V battery scenario.** The obvious next file — a real lead-acid battery is six of
  these in series — and deliberately not this slice, which is about a rate effect that a
  series string multiplies by six without changing.
* **`[r0]`'s low-SOC rise**, still placeholder shape and still owed a fit to what it
  actually is. Named in the scenario comment rather than quietly inherited.
* **The charge direction**, above.
