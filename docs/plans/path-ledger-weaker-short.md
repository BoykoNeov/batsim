# The weaker short, ledgered — and three numbers that had to move

`nothing-to-clamp` is step 19: the same external short as step 18, three times weaker, on a
pack whose BMS has no rung the fault goes through. It is the **thirteenth of twenty-four**
steps scanned whole, and it is the densest step left in the path — thirty-four numerals, more
than any other unledgered step, and the reason it was picked anyway is the reason the recorded
ranking was wrong about it.

It cost one new tie (`Tie::Sum`), six vocabulary rules, four claims, two corrected sentences,
one number off the page and one scenario header reconciled by hand.

## The cheapest step was the densest one

`path-self-description-sweep.md` ranks the remaining steps by *unaccounted* numerals and put
this one mid-table at twelve. That number was right and the ranking it implied was not: what
makes a step expensive is not how many numbers it prints but how many of them nothing already
answers for. Twenty-seven of this step's thirty-four numerals sit inside a sentence some claim
already quotes — twenty-three of them before this slice — because the step had been claimed
twice already, once for its unprotected arm and once for its protected half. The scan asked
for seven.

The general shape, worth carrying to the eleven steps left: **a heavily claimed step is a
cheap step to ledger, whatever its numeral count.** The two claim passes did the expensive
half years before the ledger existed.

## The scan found three prose defects, and none of them was a missing arm

The gaps were enumerated by adding the step to `[ledger].steps` and patching the scan to print
and continue rather than panic on the first — the technique `path-ledger-particle-step.md`
records. Twelve gaps, and the list matched a reading by hand exactly, which is not always true
of this file: a space inside a number once steered seven slices.

Three of the twelve were not gaps in the taxonomy at all.

**"73 seconds of no flags at all" is 73.5 s.** The fault lands at t = 60.0 and the first flag
of any kind is `OT` at 133.5, so the silence is a subtraction and the subtraction is exact.
73 is true of a duration read down to the second and false of the arithmetic — and an arm for
it is a *computed* tie, compared at the prose's own precision, which reads 74 where the prose
reads 73. There was no tolerance to choose here: rounding down is not what the sentence was
doing, so the digit moved. The scenario file's own header said 73 s too, and has been
corrected with it.

**A twin that "peaks" at 299.1 K peaks at 299.112, and the arm reads 299.075.** The tie
available is a sum over the step next door — that pack's ambient plus the rise its own claim
pins — and that claim reads step 18 at *its mark*, 90 s, by which time the contactor has been
open for half a minute and the cell has been cooling since t = 60.5. Measured: the peak is
299.112117 K at 60.5 and the mark reads 299.074909. Both print `299.1`, so nothing at one
decimal could have told them apart, and a claim tying the sentence's verb to the wrong instant
would have been green for a reason the sentence does not state. The verb changed instead: *"a
twin whose run ends at 299.1 K"*.

**"Let it go to about 400 s" left the page.** Nothing in the tree decides that number. The
arm's furthest claims are read at 400 s, and a tie reading a claim's own `read_at_s` was
available and refused: every variant of `Tie` is a derived numeric fact, and "the instant I
nominated" is the author supplying both sides — the declared identity this taxonomy exists to
refuse. Retargeting the arm to stop at 400 was the other bad option, and the arm's own note
already says why it runs to 420: so the last claim is not sitting on the last row. The
instruction now describes the current instead of naming a clock, which is what a reader can
actually follow: *"let it run on until the current is down to hundredths of an amp"*. This is
the DFN step's precedent (`path-ledger-dfn-step.md`), where five numbers left the page for the
same reason.

## `Tie::Sum`, and why it is not a fourth `LedgerOp`

Two of this step's numbers are in no file:

* **343.15 K**, the probe threshold — the chemistry's `cell.t_max_k` (333.15) plus the
  scenario's `pack.bms.protection.t_hard_margin_k` (10). This is what the protection layer
  actually compares a probe against, and neither half of it is the number.
* **299.1 K**, the twin — `Tie::Elsewhere` for that step's ambient, `Tie::Quoted` for that
  step's own claim on how far its cell rose.

So the arithmetic family grew its third member beside `Ratio` and `Difference`. It is a tie
and not a `LedgerOp` because `LedgerOp` reads a sentence's own printed siblings and these read
files; and the doc says plainly what its neighbours cannot: **order says nothing here.** Both
of theirs declare "order is the claim" — reversed, a difference changes sign and a ratio
inverts — and a sum reversed is the same number, so a fence about operand order would be a
fence about nothing.

Two users on the day it shipped, which is the bar this file sets after `CCCV_PERIOD_S` shipped
pinned and consulted by nothing.

## The two stale notes this step was carrying

The advisor's read before any code was written found what the prose scan cannot: **two of the
step's nineteen claim notes made false statements about the tree.**

| the note said | true when written | true on 2026-08-17 |
| --- | --- | --- |
| the `156.0` split exists because "the `Derived` arm … is not built" | yes | **no** — and the split is now load-bearing the *other* way: 96 has a rule, so a literal that grew to cover it would be accounted twice and the ledger panics on that |
| "the accounting has no arm for a configured constant … now the ONLY one still missing" | no | **no**, twice over — `Tie::Chemistry` and `Tie::Scenario` had read configured constants for six slices, and 343.15 is not a configured constant at all |

That is the fifth recorded instance of a note about the code going stale under a green suite
(`path-self-description-sweep.md` has four). Nothing checks a sentence *about* the tree, and a
slice that reads the notes it is standing on is the only instrument there is.

## Perturbations

Registered before running, and every one reddened as predicted. The ones worth keeping are the
two that price a *choice* rather than a rule:

| change | outcome |
| --- | --- |
| the milliohm rule reads `pack.series` instead of the fault's `ohms` | red, ledger scan |
| the twin half of that rule names a different lesson | red, ledger scan |
| the prose says `73 seconds` again | red — the rule resolves 73.5 and the prose reads 73 |
| the silence rule subtracts `pack.initial_temp_k` instead of the fault instant | red, ledger scan |
| the trip point sums the *voltage* hard margin instead of the temperature one | red — 333.30 |
| the twin sum quotes `soc_lost_pts_at` instead of `t_rise_k_at` | red — 298.71 |
| the `86` claim replaced by one on a quantity the sentence does not print | red — 86 accounted by nothing |
| the `2.13` claim re-read at 60.5, the first frame of the sag | red — 2.302074 |
| the mark rule reads the speed slider instead of `until_s` | red — 20 |

The **73** case is the one that proves the digit had to move: it is the whole argument for
editing a sentence that was not false, and without it the change reads as a preference. The
**2.13** case prices the instant rather than the number — the group voltage passes through
2.13 V somewhere in the middle of the silence and the sag begins at 2.30, so a claim on this
sentence is a claim about a moment or it is nothing.

What the perturbations do **not** reach: no shared input is moved. Changing `cell.t_max_k` or
the scenario's fault resistance moves the trajectory itself, so a red would be some other
check reddening — which this project has three times mistaken for a passing test of the thing
it was changing.

## What is still not checked here

* **`scenarios/external_short_100_milliohm.toml`'s header is a second copy of eight of this
  step's numbers, and nothing compares the two files.** Two of them had already parted: "95 s
  later" against a contactor that opens 96 s after the fault, and a latch stated at t = 155.5
  s, which is the frame the probe crosses on and not the frame the flag appears on. Both
  corrected by hand, and the header now says in writing that it is unguarded. This is the same
  shape as the ratio that was asserted three times in two files and wrong in both
  (`path-ledger-last-two-steps.md`), and it is the second step in a row to hit it — the
  cheapest general fix is a scan over scenario headers, which nothing has yet.
* **The peak instants are still read instants.** Both halves of the peak sentence — this
  pack's 344.5 K at 155.5 s and the twin's — are claims at a named moment, not arguments about
  an argmax. Measured here: 344.548772 K at 155.5 *is* this run's maximum, and the twin's
  maximum is one step after its tooth. Nothing would redden if either moved while the reading
  at the claimed instant stayed put.
* **Four numbers in this step are spelled in English** — "two rungs", "half a percent", "fifty
  points", "a third of the sag" — and the scan sees digits only. One of them (`fifty`) is
  claimed through `spells`; the other three are not. The word-numeral scanner is still one
  future slice, as it has been since step 3.
* **The `86` and `93` tolerances are half an amp**, which is what a sentence rounding to the
  amp is worth. The engine is 0.107 and 0.286 away. Those are the loosest value claims on this
  step and they are loose because the prose is.

## What the ledger looks like now

Thirteen of twenty-four steps scanned whole, 235 numerals, nineteen arms. Eleven steps left,
all of them carrying claims on their claimed sentences and none scanned end to end.

The ranking to use for the next one is **unaccounted numerals, not numerals** — and after this
slice the two are further apart than the recorded table suggests, because that table predates
four arms and two claim passes. By that measure the cheapest left are `bare-curve` and
`protection-off`; the honest thing to say is that nobody has re-measured the table since three
of the arms under it were built, so it ranks and does not budget.
