# The guided path's numbers, asserted by a test

`docs/plans/reversal-damage-ui.md` closed with this, under "Deferred, with a price":

> **Nothing here is asserted by a test.** The path's numbers never have been; that is
> what the harness exists for, and it is out of tree.

This slice closes it, partially and on purpose. The mechanism is complete; the coverage
is eleven claims across four steps and is meant to grow.

## Why this one, out of everything that was open

Four separate slices have found defective numbers in the guided path's lesson prose:

| slice | what it found |
| --- | --- |
| `path-numbers.md` | nine wrong or unreachable claims across eight steps |
| `path-buttons.md` | a resting voltage no sample in the first 300 s actually reads |
| `reversal-ui.md` | a number measured past the mark — unreachable, not wrong |
| `reversal-damage-ui.md` | two claims true of the engine and false of the panel |

Every one of those was found by a measuring instrument that lived outside the repo
(`M:\claud_projects\temp\path-numbers`) and therefore never ran a second time. It was
not a test, it was a telescope — pointed once per slice, by hand, at whatever that
slice happened to touch. Between slices the prose was unguarded, and the record shows
what unguarded prose does.

The other candidates open at the same moment were **blocked on data nobody has** (fitting
the placeholder constants; splitting `r_growth_per_capacity_loss` into a fast and a slow
rate) or **already declined on cost** (a DFN pack scenario at ~18 ms/step). This was the
only open item with a demonstrated catch rate.

## What was rejected, and why the rejected option is the obvious one

The obvious in-tree form is a **golden table**: run each lesson, commit the numbers,
assert they do not move. It was rejected, and the reason is the whole design.

**Every one of the four historical failures was prose drifting from a correct engine.**
A golden table would have caught none of them. It watches the wrong side of the gap.

So the unit under test is not "the engine still produces X". It is **"the sentence and
the engine still agree"**, and that needs the sentence in the assertion.

A third option — templating the numbers into the prose from a single source, so they
cannot part — was rejected as scope: it changes how the page renders and puts a
substitution layer between the reader and the text.

## The design

`web/path-claims.toml` holds one entry per claim. `crates/sim-data/tests/path_claims.rs`
reads it and checks each entry **three independent ways**:

| check | catches | test |
| --- | --- | --- |
| **Literal** — the claim's text is still in that step's prose, verbatim | the prose was edited and the claim was not | `every_claim_appears_in_its_own_step` |
| **Value** — the engine, driven as `applyStep` drives it, produces the number | the engine moved under the prose | `every_claim_matches_the_engine` |
| **Reachable** — the read time is not past the step's own `until_s` | "right but unreachable" | `every_claim_is_reachable_in_its_own_step` |

### The literal is a string, and is never formatted from the value

This is the detail the design turns on. The prose writes the same kind of quantity as
`481 mV`, `4.030 V to 3.549`, `**1.2501 V**`, `the last 53 seconds`. A check that
formatted `value` and looked for the result would be a formatter obliged to agree with
how a human wrote each sentence — it would produce false failures, and a test that cries
wolf gets its tolerance widened until it says nothing.

So the claim stores the literal **as authored** and the machine value **separately**, and
the two are asserted by two tests that share no code. Neither derives from the other.

A corollary: the literal need not contain the number at all. `the last 53 seconds` is
step 1's phrasing of "the `SOC_CLAMPED_LOW` flag arrives at 4146.5 s", and the claim
pairs that phrase with that flag time. Claiming the consequence is what makes an
unnumbered sentence checkable.

### Reachability is a separate check because the value check cannot see it

Demonstrated rather than argued. With `read_at_s` pushed from 4200 to 5000 on a step
whose mark is 4200, `every_claim_matches_the_engine` **stayed green** — the nearest-row
lookup silently clamps to the last row it has. Only the reachability test went red. A
value comparison structurally cannot detect a claim about a trajectory the reader never
sees.

### The mirror, and what pins it

`Pulse` and `CcCv` are not engine demands. They are policies in `web/app.js`, and the
test reimplements them — which makes it a second source of truth. `MIRRORED` pins every
constant copied out of the page (`CCCV_PERIOD_S`, `CCCV_BAND_V_PER_CELL`, the `dt`
default in the markup) by asserting the literal declaration is still there, so a change
on the page fails here instead of diverging quietly. The `dt` default is *parsed* from
`index.html` rather than declared, extending the discipline the out-of-tree instrument
already applied to that one value.

`sim_time_s` is read back from the pack and never accumulated, because both page
controllers quantise with `Math.round(t / dt)` and that is exactly the defence against
float drift. A mirror keeping its own clock puts pulse edges a step out somewhere past
t = 1000, where the error looks like physics.

## The mechanism was reddened three ways before any of it was believed

This repo has shipped a perturbation harness that reported five greens that were all
lies (`docs/plans/surface-vs-bulk.md`). A green assertion proves the code and the test
agree; only a red one proves the test can see the defect. Each check was broken
separately, with the others watched to confirm they stayed green:

| perturbation | expected red | result |
| --- | --- | --- |
| `0.6387 V at the mark` → `0.6499` in `web/app.js` | literal | red; other four green |
| `value = 0.6387` → `0.6500` in the claims file | value | red; other four green |
| `read_at_s = 4200` → `5000` | reachable | red; other four green, **including value** |
| `step = "protection-on"` → a nonexistent id | step exists | red, with the diagnosis; the other three claim tests cascade |

All five tests have now been driven red. The fourth row was added after the first
publication of this table, which had described the reddening as complete while omitting
the one check nobody had perturbed — the same defect class the table is evidence
against, one level up.

The value failure printed `engine says 0.6387146080532274`. That is the third result and
the one that was not planned: the in-tree runner reproduces the validated out-of-tree
instrument's `0.6387` to every digit it printed, so the reimplementation of `applyStep`
is confirmed against the thing it replaces rather than only against itself.

### The scraper degrades toward failing, not toward passing

Two parses were originally written with fallbacks and have been changed, because both
degraded the wrong way:

* `lesson_text` used `unwrap_or(0)` when the `prose: [` marker was missing. Slicing from
  zero still yields text containing the sentence, so the literal check would have gone on
  passing against a scraper that had stopped knowing what it was reading. It panics now.
* `bms` treated "field not found" and "field is null" identically. A reformatted field
  would have silently flipped a BMS-on lesson to the scenario default, moving its numbers
  rather than failing. The three cases are now distinguished and an unrecognised one
  panics.

Neither was wrong against today's `app.js` — both required someone to reformat the
lessons first. They are fixed because "fails quietly in the direction of green" is the
exact shape of the five-green harness this file's reddening table exists to rule out.
Because `lessons()` parses all twenty-one steps whether or not they carry claims, the
suite passing is now also evidence that every lesson still matches the strict shape.

## Coverage — eleven claims, four steps

| step | claims |
| --- | --- |
| 1 `bare-curve` | voltage at the mark in reversal; the flag time behind "the last 53 seconds" |
| 2 `same-discharge-other-chemistry` | voltage at the mark; the empty-at time; both ends of the 481 mV fall |
| 6 `protection-on` | the derated current; the over-current flag on the first step |
| 8 `wearing-out-while-idle` | capacity at a quarter and at the mark; the resistance coupling |

Steps 2 and 8 are on the list of eight that `path-numbers.md` found wrong. Step 1 carries
the first claim wired in. Step 6 is the cheapest claim that exercises a protection trip.

### What is NOT covered, said plainly because a green test reads as a verified one

* **Seventeen of the twenty-one steps have no claim at all.** They are unchecked, not
  passing. `every_covered_step_exists` runs one way only — it catches a claim naming a
  lesson that is gone, never a lesson with no claim.
* **Panel formatting.** "Reachable" here means the simulation runs to that time. Whether
  the page can *display* the number there is a different question — `fmtTime` prints
  whole minutes above 120 s, a charge row prints one decimal, and both have produced
  true-but-unreadable claims before (`reversal-damage-ui.md`). Checking it needs the
  page's own formatters, which are JavaScript. Out of scope here, and the largest
  remaining hole.
* **The timing claims** in steps 14 and 16 (the particle model costs ~8× the circuit;
  the cheap model ~200× less). Those are measurements of a machine, not of a
  trajectory, and do not belong in a correctness test.
* **Page-behaviour claims** — what a button orders, what a legend prints. Those need a
  browser, which is what `path-buttons.md` used.
* **The out-of-tree instrument still exists and is still the way to measure a new
  claim.** This test asserts; it does not explore. Adding a claim means measuring it
  there (or by any honest means) and writing down both halves.

## Cost

| | |
| --- | --- |
| the four covered lessons, debug profile | **2.75 s** |
| same, before grouping claims by lesson | 8.63 s |

The grouping is not a micro-optimisation. Step 8 is a 200 000 s rest at `dt = 0.5` —
400 000 engine steps — and carries three claims; running it once per claim tripled the
whole test. Claims are grouped by step and each lesson runs once.

For scale: the whole 21-step path in this shape costs ~12 s at release and ~1m42s at
debug in the out-of-tree instrument, though most of that is its benchmark loops, which
nothing here reproduces. Growing coverage to every step is affordable; growing it
carelessly — a claim per lesson-run — is not.

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no rendered page.
The diff is one test, one data file, two dev-dependency lines, and this document.
`web/pkg` needs no rebuild: no Rust that the wasm bundle embeds changed.

## Deferred, with a price

* **The formatter gap is the real remaining hole**, and it is the one that produced two
  defects in the most recent slice to look. Closing it means running the page's
  formatters, so it is a browser slice or a port of `fmtTime` and friends — and a port
  is a third source of truth, which is exactly what `MIRRORED` exists to discourage.
* **Seventeen uncovered steps.** Each new claim costs a measurement, and the measuring
  instrument is still out of tree. Nothing here makes the *next* claim cheaper to
  establish — only cheaper to keep.
* **A claim's `read_at_s` for a crossing is a measured constant, not a derived one.**
  `v_at_soc_below:0.90` is read at 415.5 s because that is when the crossing happened;
  if the trajectory shifts, the value check fires but the recorded read time silently
  becomes fiction. It is used only for the reachability comparison, where being slightly
  stale is harmless, but it is not self-maintaining.
* **`tol` is set by hand** at half a unit in the last printed place, widened where the
  prose hedges ("just under 14 A" gets 0.2 A). Nothing enforces that rule; a future
  claim can be given a tolerance that makes it vacuous, and no test would notice.
