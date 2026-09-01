# The lesson for a loop that is not one width

`docs/plans/hysteresis-width-over-soc.md` shipped `[hysteresis.width_over_soc]` at
`SNAPSHOT_VERSION` 20 and closed with one sentence about what it had not done:

> ## What is not done, and is the next slice
>
> The guided-path lesson.

This is that slice. It adds one scenario file, one guided-path step, one test, and no
engine code at all.

## What the reader is shown, and why it is the gauge rather than the loop

The direct measurement of a loop's width is two cells brought to the same charge from
opposite directions: their rested terminal voltages differ by twice the half-width, with no
other quantity in the number. Measuring the width at **two** charges that way needs four
scenario files and four trips through the picker, because a pair of files fixes exactly one
meeting point — the two arms start `2q` apart and both move `q`, so `q` is decided and the
meeting charge with it.

This step measures the width through its **consequence for the fuel gauge** instead, which
costs one file and one trip:

* The step's own run rests near empty, where the shipped multiplier is `2.5714`.
* The control arm is `scenarios/na_ion_gauge_corrects.toml` — already taught as steps 30 and
  31 — which rests at `51.6667 %`, where the multiplier is `1`.
* The two files differ in **one field**, `pack.initial_soc`, which is the same
  attribution-by-subtraction the sodium-ion/LFP pair next door already uses.

The slope of the open-circuit curve enters the number too, and that is a feature rather than
a confound. What the estimator does with a rested voltage is divide it by the local slope,
so the honest statement of the whole mechanism is

```
gap [points] = h * scale_v * M(soc) / slope(soc)
```

and a sentence that decomposes it teaches more than a pure width ratio would. Both factors
move between the two runs, in opposite directions, and the step says so.

## The numbers, measured

Measured with a throwaway probe crate built **outside** the repo — its own `[workspace]`,
its own `CARGO_TARGET_DIR` — so nothing in the tree changed to take the measurement. Both
the page's shape (the `Pulse` program, rows at the mark) and the test's shape
(`Demand::Current` then `Demand::Rest`, read on a zero-length probe) were run, and they
agree to every digit printed below.

| | this step | the control arm |
| --- | --- | --- |
| `pack.initial_soc` | `0.25` | `0.60` |
| `soc (true)` at the mark | `16.6667 %` | `51.6667 %` |
| `terminal` at the mark | `2.622120 V` | `3.112878 V` |
| the `[ocv]` midline there | `2.644633 V` | `3.121633 V` |
| loop half-width there | `25.7143 mV` | `10.0000 mV` |
| curve slope at the reading | `2.3960 V` per unit | `1.9100 V` per unit |
| inverting the table gives | `15.7270 %` | `51.2082 %` |
| `soc (bms)` at the mark | `15.6914 %` | `51.1726 %` |
| the gap | `-0.975314` points | `-0.494110` points |

The loop is **2.5714 times wider** at the lower reading and the curve is **1.2545 times
steeper**, so the gap is 2.05 times larger — and the panel, which prints a tenth of a point,
shows `1.0` against `0.5`.

Nothing trips. No flag is raised on either run, and the lowest the terminal reaches under
load is `2.4374 V` against this cell's `v_min` of `1.50`.

### The reading sits clear of an `[ocv]` node, at both ends

`16.6667 %` and the `15.7270 %` the inversion returns are both inside the `0.15`–`0.20`
segment, so the slope the estimator uses is one segment's and not an average across a node.
`spm-scenario.md` is where that hazard is written down.

## What the file's own numbers are worth, and what this step pins

`hysteresis-width-slice`'s central finding was that **the cited magnitudes are held by
nothing but the provenance note**: cutting the shipped `4.00` to `3.00` passed all 625 tests
and fired none. This step closes that, three times over and with no assertion of a number
against itself:

1. **The estimate row.** At `mult = 3.00` the gauge lands at `15.8827 %` rather than
   `15.6914 %` — 0.191 points, against a spelled-rule tolerance of 0.05 on a claim that
   prints `15.7 %`. Both the value check and the display check redden.
2. **The terminal row.** `2.626706 V` rather than `2.622120 V`, against a claim printing
   `2.622 V`.
3. **`Tie::Member` on `4.00`.** The step's prose states the multiplier the file declares at
   the empty end, and the ledger ties it to a node of `hysteresis.width_over_soc.mult`. A
   file that no longer has a `4.00` in that array fails the scan.

The third is existential — it says `4.00` is *a* node, not that it is the first — which is
`Tie::Member`'s own documented cost, and the first two are what make the pin a measurement
rather than a restatement.

### And the counterfactual, which only a test can take

The new test `the_wider_loop_costs_the_gauge_more_than_the_steeper_curve_saves` builds the
shipped chemistry **and** a copy with `width_over_soc` removed, in one process, and runs the
same trajectory through both. Removing the table does not shrink the effect, it **reverses**
it: the gap near empty becomes `-0.401123` points, which is *smaller* than the mid-charge
run's `-0.494110`. So the lesson's comparison is true only because the table exists.

This is the property `hysteresis-width-slice` recorded as unmeasurable by a data
perturbation — flattening the shipped file's table reddens the guard that asserts the
shipped file's table is non-trivial, which says nothing about the code path. Two chemistries
alive at once in one process is the only instrument for it.

## What the prose may not say, and why

**The step does not print the mid-charge gap.** The natural headline — *"`1.0` point here
against `0.5` there, twice as far"* — is refused by the accounting scan, and the refusal is
correct. `unit_is_time("")` is **true**: a number written in digits reaches the two arms of
`accounting_without_arithmetic` that compare numbers, and one of them compares against the
step length of a trajectory this sentence's claims read. That step length is `0.5 s`.
Step 18's *"0.56 points at 0.5 s"* is the sentence that proves the arm matches. So a claim
spelling `0.5` on this step would be a token with two readings, and `accounting_for` panics
rather than picking one.

The honest fix is a **unit kind per noun** — `0.5 point` is a charge and `0.5 s` is a
duration, and the arm that reads a step length should decline a charge — which
`path-word-batch-two.md` already nominates as the shape the gate should grow into. It is
**not built here**, for a reason that is about measurement rather than effort: the only
sentence that would exercise it is the sentence being written in the same commit, so the
perturbation that proves the gate works would be the one that proves this slice's own prose
passes. That is the "the predicate had a test and the plumbing into it did not" shape twice
over. It wants its own slice and a perturbation table across the other steps.

What the step writes instead:

* The gap on its own run is **`1.0`**, derived by `Op::Difference` from the two rows it
  prints — the subtraction a reader actually performs.
* The control arm's two rows are printed and **their difference is not named**.
* The doubling is stated in a sentence with **no numeral**, which names the test that
  measures it. Per `hysteresis-width-slice` and `path-uniqueness-rule-slice`, such a
  sentence is read by nothing, so it is written to name the *mechanism* — wider loop,
  steeper curve, part of it given back — rather than to make a bare comparative that no
  check and no reader can settle.

## What lands

| file | what |
| --- | --- |
| `scenarios/na_ion_gauge_low.toml` | the new file: `na_ion_gauge_corrects.toml` with `pack.initial_soc = 0.25` |
| `web/app.js` | step 32, and the file in the picker's list |
| `web/path-claims.toml` | one arm, the claims, one `[[derived]]`, both `[ledger]` partitions |
| `crates/sim-data/tests/path_claims.rs` | the ledger rules the new step's numerals need |
| `crates/sim-data/tests/na_ion_gauge.rs` | the test that holds the magnitudes and the counterfactual |
| `chemistries/na_ion_18650_generic.toml` | a pointer to the lesson, in the `[hysteresis]` provenance |

No engine code, no snapshot bump, no `WASM_API_VERSION` bump: nothing in the engine's state
or the client's wire format moves. The wasm bundle needs no rebuild either — `web/pkg` is
built from Rust that does not change.

## What was measured, case by case

Every case edits one thing, runs `cargo test --workspace --no-fail-fast`, and records the
**names** of the tests that redden. Never the exit code alone: it has pointed at the wrong
check in four slices here. Run below normal priority, through a launcher that keeps the real
exit code, because `start /wait` is exit-code-blind.

| case | reddened |
| --- | --- |
| **A** the shipped multiplier `4.00` → `3.00` | `every_claim_matches_the_engine`, `every_numeral_in_a_ledgered_step_is_accounted_for`, `the_wider_loop_costs_the_gauge_more_than_the_steeper_curve_saves` |
| **B** `[hysteresis.width_over_soc]` deleted | those three, plus `exactly_one_chemistry_carries_a_hysteresis_width_table` and `the_loop_is_wider_below_the_breakpoint_than_above_it` |
| **C** the new lesson's block deleted | 16 tests — but see below: this row measures INTEGRATION, not the claims |
| **D** the scenario rests somewhere else (`initial_soc` `0.25` → `0.30`) | `every_claim_matches_the_engine`, the ledger, and the new test |
| **E** the prose's own subtraction changed and nothing else | `every_claim_appears_in_its_own_step`, the ledger |
| **F** CONTROL: an unclaimed sentence reworded, no number touched | **nothing** |
| **G** the subtraction stated wrong *and declared consistently* — prose, literals and `spells` all moved to `1.1` | `every_derivation_is_a_sentence_doing_arithmetic`, alone on the arithmetic |

**Case A is the one the slice is for**, and the three reddenings are independent rather than
one check reported three ways: the value/display check fails on `terminal` (2.6267 against a
prose 2.622, 4.586e-3 past a 5.0e-4 tolerance) and on the estimate row; the ledger fails
because the prose states a `4.00` the file no longer has; and the test fails on the
half-width and on the gap. Before this slice **none** of them existed and that edit was
silent.

**Case C's sixteen is not sixteen independent results, and the log says so.**
`every_claim_matches_the_engine` fails there with `no lesson
`the-loop-is-wider-down-here`` — it never reaches a number. Most of the sixteen are
structural in the same way: both `[ledger]` partitions, three self-count tallies, and
every claim, arm and derivation left pointing at a step that is gone. That is worth
having — it is what stops a step being quietly deleted, which
`path-ledger-last-step-slice` found is exactly where a deletion case can go green on
the wrong check — but the row says *this step is wired into the files that describe
it*, and nothing about whether its numbers are right. **A and D are where the values
are held**, and each of those is three reddenings that had to compute something first.

**G is why E is not enough.** E moves the prose out from under its own claims, so it fails on
the literal before the arithmetic is ever reached — a red that says nothing about the derived
arm. G moves the sentence, the literals and the declared `spells` together, which is what an
author who miscounted would actually produce, and there the derivation check fires by itself.
An arm is only live if a perturbation that keeps everything else consistent reddens it.

## Two things found by sweeping rather than by building

* **`web/index.html`'s start button said `Start — 29 steps`.** The page overwrites it from
  `LESSONS.length` at boot, so no reader ever saw it and no check reads it; it had been stale
  since the sodium-ion pair landed. Corrected to `32` here. It is **not** derived, so it will
  rot again — the tally machinery reads `web/path-claims.toml` and this test file, and
  teaching it a third file is the cheap version of the fix whenever a slice next touches it.
* **Nine of the two files' own self-counts moved**, and every one had to be re-derived by
  hand from the check's own message: the claim total, the `quoted`, `spelled` and `same`
  tallies, the sentence and numeral totals, the ledger's step count in words in three places,
  and the word-scanned list's. `HEADER_WORDS` had no entry for `thirty-two`, which is the
  table growing one word per slice exactly as its own comment predicts.

## Step 31's sentence is the hook, not a casualty

Step 31 says:

> `scale_v` in this file is **`0.010`** volts, and the estimate settles under the truth by
> about that divided by the curve's slope.

It is true of that run — the multiplier there is `1` — and it reads as a general rule that
this step contradicts. It is reworded to say where it holds, and step 32 opens on the other
side of it. `hysteresis-width-slice` found the same shape one document over and recorded the
lesson: sweep the **comments and notes** around a derived sentence, not only the prose.
