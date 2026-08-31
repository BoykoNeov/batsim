# A hysteresis loop that is not the same width everywhere

`HysteresisParams::scale_v` is one scalar, and the sodium-ion cell's measured loop is three
to four times wider below 35 % charge than above it. The shipped file therefore understates
a quantity it has a cited source for, and says so in its own provenance note. This slice
gives the section an optional multiplier table over charge state, fits it to the two cited
levels, and pays the snapshot bump.

## Why this is built now, when the slice that found it declined to build it

`docs/plans/sodium-ion-chemistry.md` recorded the gap and refused it, in these words:

> Expressing it would need either a table (an `[ocv]`-shaped `soc`/`volts` pair for the
> half-width) or a second coefficient, and either is a schema change to `HysteresisParams` —
> i.e. a snapshot version bump […] **It is not made here.** Nothing shipped needs it, one
> chemistry wanting it is not a design case, and this is the same shape as slice A's
> `[safety]` gap: found by reading and measuring, recorded, and fixed later if a second file
> wants it.

That reasoning treats the table as a **feature waiting for a second customer**. It is not.
`chemistries/na_ion_18650_generic.toml` ships today with a number its own source contradicts
over a third of its range — the source says the loop reaches ~80 mV below 35 % charge and
the file can only say 20 mV — so this is a shipped parameter file that is **wrong where it
is checked against the thing it was fitted from**, not an absent convenience. A second
chemistry is not needed to justify correcting a first one.

What the earlier reasoning got right and this slice keeps: no chemistry that does not declare
the table may move by a single bit, and that must be structural rather than measured.

## The design

Keep `scale_v` as the half-width **where the multiplier is one**, so every number in every
shipped file keeps exactly the meaning it has today, and add an optional multiplier table
beside it:

```toml
[hysteresis]
scale_v = 0.010     # half-width [V] where the multiplier below is 1
gamma   = 25.0

[hysteresis.width_over_soc]
soc  = [0.00, 0.35, 1.00]
mult = [4.00, 1.00, 1.00]
```

so that the half-width actually used is `M(soc) = scale_v · interp1(soc_axis, mult, soc)`,
clamped at the ends the way every other table in this engine is.

Four choices inside that, each with a reason:

* **An explicit `soc` axis, not the fixed three-point one.** The repo has two idioms for a
  charge-indexed quantity: `[ocv]`/`[r0]`, which carry their own axis, and
  `[aging].cal_soc_stress`, which is a three-element array read at exactly 0.0 / 0.5 / 1.0.
  The second is nearer by *kind* — it is already a multiplier over charge — and it is
  disqualified by one fact: **the cited breakpoint is 35 %**, and a fixed 0/0.5/1.0 axis
  cannot place a knee there. Nothing else decides it.
* **A multiplier, not a second column of volts.** It reads the way the source states the
  quantity ("~20 mV above 35 %, up to ~80 mV below"), and it keeps `scale_v` load-bearing
  rather than turning it into a legacy field that a table silently overrides.
* **An `Option` inside the `Option`.** A chemistry with `[hysteresis]` and no table takes a
  `match` arm that never multiplies, exactly as a chemistry with no `[hysteresis]` takes an
  arm that never subtracts. That is the argument `ecm_overpotential_v`'s own doc already
  makes for its `None` arm, and it is what makes "no file without the table moves by a ULP"
  a property of the code rather than a measurement someone took once.
* **Multipliers must be positive at every breakpoint, and that is enough.** Linear
  interpolation between positive endpoints is positive everywhere, so a breakpoint-wise
  check bounds the whole curve — no interior sampling needed. A *zero* width is expressible
  by not declaring the section, so allowing zero here would give one meaning two spellings.

Read at the same instant as the state it multiplies: `ecm_overpotential_v` already receives
the cell state and already reads `state.soc` for the diffusion term, and the solve calls it
with start-of-step values throughout, so `M(soc)` and `h` are read at the same instant by
construction. Checked, not assumed: `advance_cell` updates `hysteresis` *before* the coulomb
step precisely so those two are read together, and nothing moves a cell's charge without
passing the same current through `hysteresis_update` — the internal-short leakage is inside
`i_k`, and the balancing bleed is outside the cell entirely.

## The numbers, and which of them are cited

The source states **two levels and one breakpoint**: about 20 mV of full loop above 35 %
charge, up to about 80 mV below it. `scale_v = 0.010` is half of the first. The multiplier
at the bottom is therefore `80/20 = 4`, and that ratio is cited.

**Everything about the shape between 0 and 35 % is interpolation and is labelled as such.**
"Up to 80 mV" is a *peak*, not a plateau: the source does not say where in the bottom band
the loop is widest. This file takes the monotone reading — the multiplier rises linearly
from 1 at the breakpoint to its cited maximum at the empty endpoint — because that is the
one shape that reads "up to" without adding a claim the source does not make. A step to 4
across the whole bottom band would assert a plateau; a hump somewhere inside it would assert
a location.

One thing worth writing down because it contradicts the earlier plan document. That document
explains the widening as happening "in the bottom third, where the OCV curve flattens". On
this cell's own shipped table that clause is **backwards**: measured between adjacent
breakpoints, the curve is flattest at **7.0 mV per point across 25–35 %** and steepest at
**51.8 mV per point across 0–2 %**, so the bottom of the range is the steepest part of the
whole curve and the flat shelf sits just *above* the breakpoint. If a future refit finds the
loop widest on that shelf rather than at the endpoint, the shape here moves and only the two
cited levels survive. That is said in the file.

## Sizing it against the limits, which the validator still does not do

`HysteresisParams::scale_v`'s doc warns that a half-width approaching the headroom at either
end makes a rested cell trip its own limit or read wrong, and that warning becomes live in a
way it was not, because the bottom entry is exactly where the value quadruples. The headroom
on this cell:

| end | resting extreme | limit | headroom |
| --- | --- | --- | --- |
| empty | `OCV(0)` − M(0) = 1.9886 − 0.040 = **1.9486 V** | `v_min` 1.50 V | 449 mV |
| full | `OCV(1)` + M(1) = 4.0824 + 0.010 = **4.0924 V** | `v_max` 4.15 V | 58 mV |

The top is untouched by this slice (the multiplier is 1 there). The bottom moves by 30 mV
into 479 mV of headroom, so it stays comfortably clear. Stated rather than implied, because
the number is now the product of two fields rather than one.

## The snapshot bump

`SNAPSHOT_VERSION` 19 → 20. `ChemistryParams` is serialized inside every snapshot and
`bincode` is positional, so adding a field to `HysteresisParams` changes the byte layout of
every pack whose chemistry declares the section. The bump is unavoidable and is not the
interesting part; what is interesting is the *loudness*, and this repo has been wrong about
that before (see `docs/plans/plating-absence.md`).

Predicted, from the field order: the new `Option` tag sits immediately before
`ChemistryParams::aging`, itself an `Option`. A v19 blob retagged to v20 therefore reads
`aging`'s presence byte as the width table's presence byte, so the failure is
**value-dependent** in exactly the way v19's was — a chemistry *with* `[aging]` supplies a
`1` and the reader tries to parse two `Vec<f64>` out of aging's floats and fails loudly; a
chemistry *without* one supplies a `0`, the width reads as absent, and the shift cascades
into the following fields instead. `na_ion_18650_generic` has `[aging]`; `nimh_subc_3ah_generic`
does not. Both are checked.

## Predictions, registered before anything is run

| # | prediction |
| --- | --- |
| **P1** | **No shipped trajectory moves except sodium-ion's, and structurally so.** Every other chemistry takes a `match` arm that never multiplies. Every golden CSV, every other scenario, every property test: bit-identical. |
| **P2** | **`na_ion_gauge_corrects.toml` and `lfp_gauge_declines.toml` are bit-identical too**, because that run spans 60 % → 51.67 % charge and the multiplier is exactly 1.0 over the whole of it. So **no guided-path claim moves** and `path_claims.rs` needs no edit. This is the prediction most likely to be wrong, and if it is wrong the slice grows a client half it was scoped not to have. |
| **P3** | **`cc_discharge_na_ion.toml`'s two documented voltages fall by 29.99–30.00 mV**, to **1.6713 V** at 1 C and **1.1202 V** at 3 C. A cell discharged from full is saturated to eleven decimal places by the time it empties (`gamma·dz` = 25), so the displacement at the reading instant goes from `M = 10 mV` to `M = 40 mV` less a step's worth of the ramp. Both stay on the right side of their assertions: 1 C still empties above the 1.50 V cutoff, 3 C still crosses it first. |
| **P4** | **`a_rested_cell_remembers_which_way_it_was_driven` stays green with no edit**, because both its arms travel between 35 % and 55 % charge, where the multiplier is exactly 1.0 — including the endpoint, which is the breakpoint itself. |
| **P5** | **The measured ratio between two loops read in the two bands is exactly the multiplier at the low meeting point**, `2.714286`, because the saturation factor and the open-circuit voltage cancel between arms that met at the same charge. Absolute: **18.3583 mV** at 70 % and **49.8297 mV** at 15 %, for arms that each moved 10 % of capacity. |
| **P6** | **A table of all-ones is bit-identical to no table at all.** `lerp_at` computes `ys[lo] + frac·(ys[hi] − ys[lo])`, which is `1.0 + frac·0.0` = exactly `1.0`, and `x · 1.0` is exact. This is the perturbation that has to go **green**, and it is the one that shows the new term is a clean generalisation rather than a second code path. |
| **P7** | **A v19 blob retagged to v20 fails loudly for the sodium-ion cell and quietly for the nickel one**, for the field-order reason above. |
| **P8** | **The `[hysteresis]` half-width claim in the guided path stays true.** Step 31 quotes `scale_v` **as the named field** — *"`scale_v` in this file is `0.010` volts"* — not as a property of the cell, and reads it at 51 % charge where the multiplier is one. A sentence claiming "this cell's loop is 10 mV" would have gone false; this one does not. |

## What would falsify the whole slice

If P1 or P6 goes red, the design is wrong rather than the numbers: a mechanism that cannot be
switched off exactly is not an optional mechanism, and this repo's whole argument for
`Option`-gated physics rests on the arms being *paths*.

If P2 goes red, the fence is wrong: the slice would be touching lesson prose it was scoped
not to touch, and the honest response is to stop and re-scope rather than to edit claims.

## Deliberately not done

* **No lesson, no client change.** The effective half-width is now a product of two fields,
  and this repo's accounting check refuses a number that is both claimed in prose and read
  off configuration — a lesson about a varying loop needs a `Derived` arm and a step of its
  own. Recorded as the next slice, the way slice C handed the teaching to slice D.
* **No table on the nickel cell.** Its `scale_v` is a cited class figure with no
  charge-dependence behind it, and inventing a shape would be the unlabelled constant the
  provenance rule forbids. Its absence is also what gives P1 a second file to be true of.
* **No fit of the shape.** The ~30 MB source runs are deliberately uncommitted and
  `tools/reference/fit_na_ion_hakadi.py` reports only mean/min/max of the loop, not where in
  the range the maximum sits. Fitting the peak's location needs the data back.

---

# Measured

*Nothing above this line was edited once the engine ran.* Every number below is a reading,
and every green below is an exit code rather than an impression.

## The checks

`cargo fmt --all -- --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
clean, and `cargo test --workspace --no-fail-fast` **exit 0: 625 tests across 67 binaries,
none failing** — 72 `test result: ok` sections, five of which are empty doc-test runs. The `--no-fail-fast` matters — without it the workspace run stops at the first
failing *binary*, so a fail-fast table understates its own coverage.

Clippy caught one thing worth recording rather than silencing. The first version of the
"nothing predating v20 carries a table" guard iterated a one-element list
(`single_element_loop`). Rewriting it as a sweep over a new `SHIPPED_CHEMISTRIES` constant
made it a *stronger* assertion than the one clippy rejected: it now names all seven shipped
files and asserts exactly which of them declare `[hysteresis]` at all, so a new chemistry
arriving with a table has to come past it.

## The eight predictions

| # | outcome | what was actually measured |
| --- | --- | --- |
| **P1** | **confirmed, with the strength restated** | 625 tests green, every golden included. But a green golden is *not* proof of bit-identity — goldens compare within stated tolerances, so they could absorb a small move. The bit-identity claim rests on two other things: the `None` arm in `hysteresis_half_width_v`, which is structural, and case G below, which shows what a file that *does* take the table looks like when it moves (10 tests, 4 binaries). |
| **P2** | **confirmed for claims; one repair needed in *unclaimed* prose** | No claim moved: the gauge scenarios span 60 % → 51.67 % charge, where the multiplier is exactly 1.0, and `every_claim_matches_the_engine` is green with no edit to a `[[claim]]`. But P2 was scoped to claims, and this repo's recurring defect is a sentence no check reads — so the lesson text was swept by hand as well. See "the prose sweep" below. |
| **P3** | **confirmed to four decimal places, and it found two pre-existing defects** | The fall is **exactly 30.0000 mV at every rate and every step length** — 1 C and 3 C, dt = 0.5 and dt = 0.1 alike. New readings: 1.6741 V / 1.0868 V at dt = 0.5, **1.6713 V / 1.1202 V** at dt = 0.1, the two predicted figures. Both orderings survive with hundreds of millivolts to spare. See the two findings below. |
| **P4** | **confirmed, and case C proves it was load-bearing** | `a_rested_cell_remembers_which_way_it_was_driven` is green with no edit. Case C (breakpoint 0.35 → 0.50) reddens *exactly that test and nothing else*, which is the evidence that its greenness here is a fact about where the breakpoint sits rather than a test that cannot see the table. |
| **P5** | **confirmed exactly** | `18.36 mV` at 70 % charge and `49.83 mV` at 15 %, **ratio 2.714286** — the table's own multiplier at 0.15 to six figures. The predicted absolutes were 18.3583 and 49.8297 mV, and the test binds each to within 0.01 mV of the value recomputed from the shipped fields. |
| **P6** | **confirmed, bit-exact** | `an_all_ones_table_is_the_same_bits_as_no_table` passes. Note this is the *unit* test, not perturbation case A — see the finding on case A below. |
| **P7** | **confirmed, both arms** | `a_v19_shaped_hysteresis_section_does_not_parse_at_v20` asserts the loud arm (sodium-ion, which has `[aging]`) *and* the quiet one: for the nickel cell the width reads as absent, `scale_v` and `gamma` are unmoved, and `aging` comes back `None` — i.e. it was filled from `safety`'s tag, which is the field-slide cascading exactly as predicted. |
| **P8** | **confirmed by reading** | Guided-path step 31 says *"`scale_v` in this file is `0.010` volts"* — a claim about the named field, not about the cell — and its run sits at ~52–57 % charge where the multiplier is one. A sentence phrased "this cell's loop is 10 mV wide" would have gone false at this commit. |

## The perturbation table

Seven cases, each a single edit to the tree, each followed by a full `cargo test --workspace
--no-fail-fast` at below-normal priority. Reported by **which tests fired**, not by the exit
code alone — a red exit code is regularly the wrong check reddening.

| case | the edit | exit | tests that fired |
| --- | --- | --- | --- |
| **A** | shipped table flattened to all-ones | **red** | 1: `the_loop_is_wider_below_the_breakpoint_than_above_it` |
| **B** | `[hysteresis.width_over_soc]` deleted | **red** | 2: `exactly_one_chemistry_carries_a_hysteresis_width_table`, `the_loop_is_wider_below_…` |
| **C** | breakpoint `0.35` → `0.50` | **red** | 1: `a_rested_cell_remembers_which_way_it_was_driven` |
| **D** | positivity check removed from the validator | **red** | 1: `a_malformed_hysteresis_width_table_is_rejected` |
| **E** | wide end `4.00` → `3.00` | **GREEN** | 0 |
| **F** | `hysteresis_half_width_v` ignores the table | **red** | 3: `the_half_width_clamps_outside_its_breakpoints`, `the_loop_is_wider_where_the_table_says_it_is`, `the_loop_is_wider_below_…` |
| **G** | the NiMH cell gains a table | **red** | 10, across 4 binaries: `nimh_chemistry_loads_and_validates`, `resting_voltage_remembers_the_drive_direction`, `a_full_cell_falls_through_the_charger_termination_window`, `past_the_peak_nothing_turns_round`, `a_charging_cell_cools_before_it_warms`, `the_fall_is_shared_between_the_two_temperature_channels`, `with_temperature_pinned_the_fall_is_exactly_zero`, `every_shipped_scenario_parses_builds_and_steps`, `every_claim_matches_the_engine`, `exactly_one_chemistry_carries_a_hysteresis_width_table` |

### Case A went red, and it was predicted to go red for the wrong reason

A was written as P6's perturbation — *"a table of all-ones must be bit-identical to no
table"* — and it was said before the run that it would probably redden anyway, because the
sodium-ion test asserts the **shipped file's** table is non-trivial (`ratio > 2.0`). That is
what happened, and only that: one test, and it is the guard on the file rather than on the
mechanism. **A perturbation of the data cannot measure a property of the code.** P6 is
measured by `an_all_ones_table_is_the_same_bits_as_no_table`, which builds both cells in one
process and compares trajectories bit for bit.

Case A is still worth its 145 seconds, for the opposite reason: it says that of 625 tests,
**exactly one** notices when the sodium-ion cell's loop stops varying. Nothing else in the
suite pins that trajectory.

### Case E is the finding: the cited magnitude is held by nothing

Cutting the cited ratio by a quarter — `mult = [3.00, 1.00, 1.00]`, i.e. asserting the loop
is three times wider at empty rather than four — passes the entire suite. Zero tests fire.

That is not a hole this slice should have closed by adding an assertion, because there is
nothing to assert *against*: the number is `80/20` from the source, and a test restating
`80/20` would only be checking that the file agrees with itself. What case E actually
measures is the boundary of what the tests can defend. They hold the **shape and the
relations** — that the table is read at all (F), that the breakpoint sits at 0.35 (C), that
the two ends differ by more than rounding (A), that exactly one file carries one (B, G) —
and they hold **none of the magnitudes**. The magnitudes are held by the provenance note and
by nothing else, which is the ordinary condition of every fitted number in this repo and is
worth having measured once rather than assumed.

The same is true one level up, and the scenario file now says so in its own words: every
voltage quoted in `scenarios/cc_discharge_na_ion.toml`'s header is a reading no test holds
to a number.

### The prose sweep, which the claim check does not do

`every_claim_matches_the_engine` reads the sentences a claim quotes. It does not read the
rest, and this repo has shipped false sentences through that gap twice — *"a comparative
with no numeral is read by nothing"*, *"a sentence with no numeral was false and every check
was green"*. This slice quadrupled a resting-voltage loop and moved a sodium-ion trajectory
30 mV at the empty end, so the lesson text, the parameter files, and the plan documents were
swept by hand for three shapes: a statement of how wide the loop is, a statement of how far
a rested cell sits off its curve, and any general sentence saying one number sets the width.

**One repair, and it was in a comment rather than in reader-facing prose.** The ledger's
entry for step 31 described that step's seventh numeral as *"the half-width of the hysteresis
loop, straight off the chemistry file"*. The numeral is `0.010`, which is `scale_v`, and
after this commit `scale_v` is the half-width only **where the multiplier reads one**. Fixed
in `web/path-claims.toml` and the reason recorded there.

The distinction is exactly P8's, and it cuts the same way both times: the *reader-facing*
sentence says "`scale_v` in this file is `0.010` volts" — a claim about the named field,
which is still true — while the comment describing it had generalised to a property of the
cell, which is no longer true. **The prose was safe because it named the field. The note
about the prose was not.**

Everything else came back clean, and that is a finding rather than an absence:

* No guided-path step runs `cc_discharge_na_ion.toml` — the only shipped trajectory this
  commit moved is not taught anywhere, which is why P2 held.
* Step 31's arithmetic sentence ("the estimate settles under the truth by about that divided
  by the curve's slope") stays true: the run sits at ~52 % charge, where the multiplier is
  exactly one.
* The nickel-cell memory step is untouched — that chemistry carries no table, and its
  `24.83 mV` against a `25 mV` half-width is still a statement about a scalar.
* No plan document or parameter file claims the loop is one width everywhere. The one that
  came closest, `docs/plans/sodium-ion-chemistry.md`, is the document that *recorded the gap*
  and now carries a superseded banner pointing here.

### Case G is P1's counter-test

P1 says a file without the table cannot move. G gives the nickel cell a table and it moves
loudly — ten tests in four binaries, including the guided-path claim check. The absence is
what keeps the other six chemistries still, and G is the measurement that the absence is
doing work rather than the mechanism being inert.

## Two pre-existing defects, found by measuring P3 rather than by reading

Neither was introduced by this slice. Both were found because P3 required knowing what the
two documented voltages actually are, which meant running them, which meant reading how the
header describes them.

**The step length was mislabelled.** `scenarios/cc_discharge_na_ion.toml` said its two
voltages were "measured at dt = 0.5 (the page's default)" and quoted numbers taken at
dt = 0.1 — the step `crates/sim-data/tests/na_ion_chemistry.rs` uses. Verified pre-existing
with `git show HEAD:`: HEAD's test also stepped at 0.1, and HEAD's 1.7013 V / 1.1502 V are
the dt = 0.1 v19 figures. The reason the numbers move with the step at all is worth keeping:
the reading is taken on the step that *first* reports empty, and that step always lands
somewhat past empty on the `[reversal]` ramp, which falls 100 V per unit of charge. So a
quoted voltage is the resting empty figure less one step's worth of over-discharge — 2.8 mV
at 1 C, 33.4 mV at 3 C, each exactly the ramp times the charge one step moves at that rate.
The header now carries both rows in a table and names which is which.

**"The ohmic drop and nothing else" was false.** The header explained the 1 C / 3 C gap as
purely `I·R0`. Measured at the two reading instants at dt = 0.5, of the **587.2 mV** between
them: **436.7 mV** is ohmic, **108.8 mV** is the two RC pairs, **41.7 mV** is the two arms
having landed at different depths past empty, and **0.0** comes from the open-circuit source
(both arms read it at the same zero). At dt = 0.1 the first two are unchanged and the third
falls to 5.6 mV. The header now carries the decomposition, and the description shown by the
scenario picker was corrected from "the ohmic drop" to "the drop under load".

Both were measured with a throwaway probe crate built **outside** the repo — its own
`[workspace]`, its own `CARGO_TARGET_DIR` — so that nothing in the tree changed to take the
measurement.

## What is not done, and is the next slice

The guided-path lesson. The effective half-width is now a product of two fields, so a lesson
about it needs a `Derived` accounting arm rather than a quoted constant; that was scoped out
before the run and stays out. The one thing this run adds to that scope: step 31's existing
sentence is *safe* (P8), so the next slice adds a step rather than repairing one.
