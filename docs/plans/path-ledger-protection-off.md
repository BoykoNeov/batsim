# The step with nothing watching, ledgered — and a note that was wrong about its own subject

`protection-off` is step 7: the same 40 A demand as step 6, with the BMS taken out of the
pack. It is the **fifteenth of twenty-four** steps scanned whole, and it was picked by
measurement, which for the second slice running agreed with the recorded ranking.

It cost five vocabulary rules, eight claims, six new quantities and two new fields on the
harness's row — and **no new arm**. That is the second slice in a row at nineteen, after
three that each cost a new kind of tie.

## The ranking, re-measured — and it did not move

Last slice's table was a measurement and was expected to age. It did not: with `bare-curve`
removed, every remaining row is exactly where it was.

| step | unaccounted numerals |
| --- | --- |
| `protection-off` | **15** |
| `looks-fine-from-outside` | 19 |
| `one-step-that-got-through` | 21 |
| `leg-that-is-not-there` | 22 |
| `what-protection-costs` | 25 |
| `past-empty` | 27 |
| `three-times-the-current` | 31 |
| `same-discharge-other-chemistry` | 35 |
| `what-it-cost` | 35 |
| `the-gradient-itself` | 38 |

The command is last slice's: put every unledgered step in `[ledger].steps` at once, turn the
accounting check's first-failure panic into a print-and-continue, and read the counts off one
run. It is worth stating that a table holding still is *also* a result — it means nothing in
the ten remaining steps' prose moved since the last measurement, which is what a frozen path
should look like.

## The note that was wrong, and what it was wrong about

The claims file carried this, written when step 7's five headline figures were repaired:

> **WHAT IS STILL NOT CLAIMED, and why:** "(0,0) first at 345.0 s, (1,1) last at 356.5". The
> cell indices are numbers in the sentence and no accounting arm reaches them — they are
> neither measurements, nor instants, nor settings, nor anything a row prints. […] The
> crossing times are real and reproduce; the sentence stays unclaimed until something can
> account for a coordinate.

Both halves are wrong, and they are wrong in different ways.

**An address is a measurement.** Which cell owes most at the instant the first debt appears
is a fact the engine decides, exactly as much as the `345.0` beside it is. At that instant
one cell in the pack is past empty and the other seven are not, so "the cell with the largest
deficit" and "the cell that crossed first" are the same cell. The mirror holds at the other
end: at 356.5 s the cell that owes *least* is the one that has only just started owing
anything — 0.061 points against the worst cell's 2.87, which is not a close call. So the four
coordinates needed no new arm at all. They needed a quantity, and they are ordinary `spells`
claims once they have one.

The claim to that had to be paid on the harness side rather than in the taxonomy: `Row` now
carries the address of each end of the deficit spread beside its value, off the same walk, so
an argument about *which* cell is worst can never be read off a different loop from the number
beside it.

**The splitting the note called a cost is a requirement.** The note's other objection was that
claiming the sentence would mean "splitting a literal down to fragments that do not read as
sentences". Fragments were already settled — step 6's two readings of one current are claimed
as `just under 14 A` and `pinned near 14 A` — but the real point is the opposite of the note's:
the split is not a price, it is the thing that makes the claims work. `Accounted::Spelled` asks
whether **any** claim on the sentence names the token, so a single literal carrying both
`(0,0)` and `(1,1)` would have stayed green with the two pairs swapped. Three literals is what
makes each pair answer for itself:

* `they cross over 11.5 seconds`
* `(0,0) first at 345.0 s`
* `(1,1) last at 356.5`

## What the fifteen went to

| numeral | accounted by |
| --- | --- |
| `40 A`, three times | the demand box, in three sentences and so three rules |
| `2.0 V` | the chemistry's `cell.v_min` |
| `about 14` | step 6's own clamped current, quoted |
| `450 s` | the mark |
| `step 3` | where `pack-disagrees` sits in the path |
| `345.0 s`, twice | a new claim, in each of the two sentences that print it |
| `11.5 seconds` | a new claim on the crossing spread |
| `(0,0)`, `(1,1)` | four new claims, one per coordinate |
| `356.5` | a new claim |

Five constants, one quotation, one ordinal, and eight measurements. The quotation is the one
worth pausing on: the `14` here is step 6's number, printed in step 7's prose, and the ledger's
claimed arm is *positional* — a number is accounted only inside the literal of a sentence some
claim quotes, so step 6's claim on its own prose does nothing for this sentence. `Tie::Quoted`
is what makes step 7 go red if step 6's clamp ever moves. That hazard was written down in
`path-claims.toml` when step 6 was ledgered; this is the sentence it was written about.

## Three things measured rather than assumed

**The spread is a measurement, not the sentence's own arithmetic.** `11.5` is exactly
`356.5 − 345.0`, and check 6's `[[derived]]` table could have accounted it that way. It does
not, for two reasons. A derivation's operands have to be numbers its *own* literal prints,
which would have forced the crossing pair back into one literal and undone the split above.
And a spread is the better instrument anyway: it is what the sentence is about, and it goes
red when the scatter moves even if both ends move together.

**One of the new quantities agrees with an old one by construction, and the note says so.**
`deficit_leaves_zero_s` returns the same 345.0 as `flag_first_s:SOC_CLAMPED_LOW`, and always
will: the flag is raised on the step the coulomb counter clamps, and the debt the readout
shows is what that clamp carries. The pair is not two measurements confirming each other. The
second quantity exists because there is a second *sentence* — one about a flag, one about a
readout — and this file's rule is that a sentence is tied to the thing it names. Written into
the claim rather than discovered later, because structural agreement passing for corroboration
is a shape this file has been caught by before.

**The two coordinates of an address cannot be told apart here, and the reason is the pack.**
Both digits of `(0,0)` are 0 and both of `(1,1)` are 1, so a harness that read the parallel
index where the series index belongs would agree with the prose. Each claim is tied to its own
quantity, so neither is decorative — but the *sentence* has no way to distinguish them. Two
perturbations confirm exactly this: swapping the axes in `measure_row` leaves the suite green,
where changing either pair to `(1,0)` reddens the value check. Stated in the claim notes,
because a reader would otherwise assume the pair is ordered.

## The perturbations

Twelve cases, and the two that came back green are the point of the table rather than a gap
in it.

| perturbation | result |
| --- | --- |
| `(0,0)` → `(1,0)`, prose and its three claims together | **red** — the value check |
| `(1,1)` → `(1,0)`, prose and its three claims together | **red** — the value check |
| the opening sentence's demand box, 40 → 30 | **red** — the ledger |
| the datasheet floor, 2.0 → 2.5 V | **red** — the ledger |
| step 6's clamp quoted as 20 instead of 14 | **red** — the ledger |
| the mark, 450 → 400 s | **red** — the ledger |
| the back-reference, step 3 → step 4 | **red** — the ledger |
| the crossing spread, 11.5 → 12.5 s | **red** — the value check |
| the last crossing, 356.5 → 357.0 s | **red** — the value check |
| delete the crossing-spread claim, whole block | **red** — the ledger |
| delete either second coordinate claim, whole block | red on the *tallies only* |
| read the parallel index where the series index belongs | **green** |

The last two rows are the honest ones.

**Deleting one coordinate claim of a pair reddens nothing but the file's own arithmetic about
itself.** `Accounted::Spelled` asks whether any claim on the sentence names the token, and both
digits of `(0,0)` are `0` — so the series claim alone covers both, and the parallel claim is not
*required* by the scan. It is not decorative: it asserts a fact about the engine that would be
wrong if the pack put the worst cell in the other half of its group. But its place is earned by
what it asserts, not by a hole it closes, and that is a distinction this file has had to make
before. Had the address been `(0,1)`, both claims would have been load-bearing. The redundancy
is a property of this pack, not of the design.

**Swapping the two axes in the harness leaves the suite green**, for the same reason and one
level deeper: with both digits of each pair equal, no arrangement of these claims can tell a
series index from a parallel one. Recorded in the claim notes rather than fixed, because there
is nothing here to fix — the sentence does not carry the information.

A thirteenth case is worth writing down because it was run first and was **wrong**: the
deletion was done by pointing the claim's `step` at a lesson that does not exist rather than by
removing the block. That reddened seven tests, the loudest of them `every_covered_step_exists`,
and said nothing whatever about whether the claim was needed. A deletion perturbation has to
delete.


## What the ledger looks like now

Fifteen of twenty-four steps scanned whole, 275 numerals, nineteen arms. Nine steps left, all
carrying claims on their claimed sentences and none scanned end to end.

**Nineteen for the second slice running.** The three ledger slices before last one each cost a
new kind of tie; the last two have cost none. What this one needed instead was a new
*quantity* — six of them, and two fields on the harness's row — which is a different kind of
cost and one worth naming: the taxonomy is finished, and what a step now costs is whatever
instrument its sentences are about.

By the table above the next step is `looks-fine-from-outside` at nineteen. It is also the
first of the nine where the ranking's gap is narrow enough not to matter much: nineteen,
twenty-one and twenty-two sit within three numerals of each other, and `path-ledger-weaker-short.md`
already recorded that the densest step can be the cheapest when its numbers are already
claimed. Re-measure before picking.

## Two things left where they are

**Two vocabulary rules still reach a step they were not written for.** The sweep from last
slice — match every rule's phrase against all twenty-four lesson texts, print the ones reaching
more than one — now finds two of a hundred and ten rules, down from three: the ordinal rule was
narrowed last slice, and both of step 3's scatter rules still reach `what-protection-costs`,
where they will be right because both scenarios declare the same 2 % and 3 %. Right by
coincidence of values rather than by design. None of the five rules added here reaches a second
step, which was checked before they were committed rather than after.

**The self-counts in words are swept by hand and are not checked.** Eighteen tallies moved
this slice and the derivation caught every one of them, because every one prints digits. The
sentence in this test's own module docs that says which of two counts is frozen and which
moves is written in *words*, and nothing can see it — it said the two were equal and only one
moved, which was true for exactly one slice. It is now correct again by hand. This is the
fourth slice in a row to record that word-form self-description rots invisibly, and the honest
summary is that it will keep happening until either the phrases carry digits or `WORD_NUMERALS`
is wired into the tally renderer.
