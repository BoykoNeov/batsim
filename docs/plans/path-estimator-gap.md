# The estimator gap, measured — and the first number this path spells in letters

`docs/plans/path-ledger-scenario-arm.md` closed three steps' digits and then named four
quantities its scanner cannot see, because they are written as words rather than as
figures. One of them was singled out as the cheapest measurement left anywhere in the
eleven unchecked lessons: step 4's "a gap of about **three points** that simply never
closes". It needs no reading taken with the clock stopped and no instructed continuation —
one run to the mark, and a claim.

This slice makes that run and writes the claim. It also does the smaller of the two things
that were blocking it, and deliberately not the larger one.

## What landed

* **Two claims on step 4's estimator gap**, both on the same sentence:

  | claim | quantity | reads | value |
  | --- | --- | --- | --- |
  | the gap at the mark | `soc_gap_pts_at` | 600 s | **3.0651 points** |
  | the narrowest it ever gets | `soc_gap_pts_min` | 0.5 s | **3.0182 points** |

  The first is "a gap of about three points". The second is "simply never closes", and it
  is the half a reading at the mark cannot make: an estimator that closed the gap at 300 s
  and re-opened it by 600 would pass the first claim and fail the second. It is also what
  checks the rest of the sentence — "the estimate sits above the truth from the first step
  to the last" is a statement about every step, and a minimum is the only way to say it.

* **`spells` may now hold a word.** `WORD_NUMERALS` in `path_claims.rs` is the
  translation, and it has one entry: `three`. A word commits to no decimal place, so the
  rule it licenses is half a unit of the unit it is written in — half a point here.
  Resolution happens in one place (`spells_as_number`), because `spelled_rule_tol` and
  `spelled_value` each used to parse the string themselves and two resolution sites is how
  a word comes to be a number for the tolerance and not for the value.

* **A word nothing spells fails.** `every_word_numeral_is_spelled_by_a_claim`, the same
  guard `every_ledger_rule_is_a_phrase_and_is_used` keeps over the ledger's vocabulary, for
  the same reason: `CCCV_PERIOD_S` sat pinned and unread for six slices while the mirror it
  was meant to guard was wrong.

* **A word is bounded like a number.** `contains_number` refuses a match flanked by a
  letter, so `three` inside `threefold` is not this number. That branch is load-bearing and
  the measurement below says so: with the old digit-only flanker, a sentence reworded to
  `threefold` leaves the whole suite green.

* **The ledger's fence came down.** A ledgered step may now carry claims, and
  `belief-drifts` is the first that does.

## The fence, and why nothing replaced it

The ledger refused to scan a step that carries claims, on the grounds that a number a claim
ties to the engine has no accounting in the scan and the author would meet a confusing
failure. That was true and is no longer the situation here: the quantity these two claims
are about is spelled in letters, the scan finds digits, and this step's three digits are
the scenario constants they always were. The combination the fence forbade is now in the
tree and the scan is unchanged by it.

**The `claimed` arm was not built, and that is the decision this slice spent the most on.**
The arm is five lines — check 6's `accounting_for` is written and tested — and it would be
reached **zero times**: no numeral in any ledgered step is decided by a claim. A pinned,
plausible, unread mechanism is exactly the shape this file rejects everywhere else, and the
ledger's existing failure message already routes an author to it in the words they will
need ("If the engine does, it needs a claim in web/path-claims.toml and the `claimed`
arm"). It goes in when a number needs it.

## The word scanner was not built either, and the reason is step 3

Teaching the ledger to read word quantities is what would let this new claim account for a
sentence rather than sit beside it. It is one slice with three parts, not one part:

* A vocabulary that sees "three points" also sees "**half a point** across the whole grid"
  and "**a quarter of a point**" between the cells of one pair — step 3's per-cell SOC
  spread, which nothing in this repo has measured.
* So those two need claims, which need per-cell state in the harness's sampled rows, and
  "between the two cells of a single pair" needs a reading chosen (the worst pair? a
  typical one?) before it can be a number.
* And behind both, the `claimed` arm, which then finally has a consumer.

Drawing the vocabulary short of that — cardinals only, say — would be drawing it exactly
where the claim that already passes happens to sit. The three parts are one future slice,
named in `path-claims.toml` and in the test's own docs. "A fraction of a point" of sensor
drift is a fourth thing again: it spells no number at all, only a bound.

## Two things measured rather than assumed

**The tolerance is not the sentence's.** Half a unit in `three` is half a point, which is
what the phrase licenses and far more than the claim needs, so both are `tighter` at
**0.05 points**. That number has a meaning: the panel has no gap row — the reader
subtracts `soc (bms)` from `soc (true)`, and each prints a tenth of a point — so 0.05 is
half a unit in the last place a reader can compute this quantity to at all. At the mark
those rows read 66.2 % and 63.2 %, so what a reader gets is 3.0. It is also why neither
claim names a `display`: there is no row to render, and the two rows behind it belong to
two other quantities.

**The minimum being the first row is a fact about the run, not arithmetic.** The gap is
*not* monotone — the BMS's current sensor carries 10 mA of noise, so it wobbles from step
to step — and what the run shows is that the wobble never takes it below where it started.
Had it been monotone, the minimum claim would have been the first row's reading under
another name. `soc_gap_pts_min` therefore asserts that the minimum lands on the row its
claim names, rather than ignoring `read_at_s`: a reduction over every row has no natural
instant, and a decorative one would let the shape of the run change under a green claim.

The gap also starts at 3.0182 rather than at the exact 3.0000 the scenario's
`initial_soc_error` hands the BMS, because the estimate lags the truth by one step.

## What was measured

Thirteen perturbation cases, each launched directly at below-normal priority with a real
exit code — `start /wait` is exit-code-blind and this repo has recorded it twice — and each
recording *which* test reddened rather than only that something did.

| perturbation | reddens |
| --- | --- |
| no perturbation at all — the null | nothing, exit 0 |
| prose says `four points`, claim untouched | literal |
| prose, literal and `spells` all say four together | **stated** — and nothing else |
| `spells` says four, the sentence says three | stated, tolerance, word-numeral |
| `value` moves 0.135 points | value |
| `tol` widened to the rule (0.5) | tolerance |
| the minimum claim reads at 300 s | value, on the instant assertion |
| a word numeral no claim spells | word-numeral |
| the scenario's boot error moves to 4 % | value **and** ledger |
| CONTROL: the old fence restored, with the claims present | ledger |
| CONTROL: `belief-drifts` prose gains `17 kg` | ledger |
| `threefold`, word boundaries on (this slice) | tolerance |
| CONTROL: `threefold`, digit-only flanker (before) | **nothing** — green |

The third row is the one worth reading twice. Rewording the sentence, the literal and the
`spells` together is the drift every other check is blind to by construction — the literal
is a substring test against the page and the value a comparison against the engine — and
only check 5 catches it. It is the reason `states` exists, exercised here on a word.

The two control arms are about the fence rather than about the claim. The first says the
fence really was the blocker: with it restored and the claims present, the ledger goes red.
The second says the scan still bites on a step that now carries claims — the whole risk of
narrowing a fence is that the thing behind it quietly stops working.

The null case is in the table on purpose. `docs/plans/surface-vs-bulk.md` records five
green perturbations that were all lies.

## Found on the way: a harness that fails mid-edit leaves the tree perturbed

The perturbation script applies its edits to the working tree and restores from bytes it
captured at startup. A case whose edit did not match raised out of the loop *after* one
file had been written, so the restore never ran — and the next invocation captured the
perturbed file as its baseline. Nothing was lost (the change was one word in one sentence,
and `git status` found it immediately), but a harness that can leave the tree dirty is a
harness that can measure the wrong tree. The edit now runs under a `try`/`except` that
restores before re-raising, and each edit asserts its own match count for the reason a
scripted edit matching nothing fails silently.

## Deferred, with a price

* **The ledger still cannot see this sentence.** The two claims check it; the scan does
  not know it is there. `belief-drifts` is now digits-closed *and* claimed, and those are
  two separate statements about the same step — neither implies the other. The word
  vocabulary is what would join them, and it is the slice above.
* **The three other English quantities are still checked by nothing.** Step 3's half a
  point and quarter of a point, and step 4's "a fraction of a point" of sensor drift. The
  first two are the per-cell spread measurement; the third is a bound rather than a value
  and needs a frame this file does not have.
* **Ten of the twenty-four steps remain wholly unchecked**, and what they need is unchanged
  by this slice: the zero-length probe row, instructed continuations, and control changes
  other than one demand-box edit. `[ledger].unledgered` names them one line each.
* **A claim on a gap has no display half.** That is honest — there is no row — but it means
  these two are the value-only shape, and the display check's independent evidence (a step
  function of the value crossing a printed digit) is unavailable here. What a reader
  actually sees is two rows to a tenth of a point, which is where the tolerance came from.
