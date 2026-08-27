# Word numerals in the ledger

The ledger scans a step's whole prose and requires every **numeral** in it to be tied to
something. Numeral means digits: `written_numbers` collects digit runs and nothing else, so
a quantity a sentence spells in English is invisible to it. A green ledger says the step's
digits are tied. It does not say the step is checked.

This is the last thing the ledger's own docs name as open, and it is now the whole of the
gap rather than half of it.

## What is actually there

Scanned all 24 lesson texts for English number words: **293 raw hits**. Most are not
quantities — `one` alone is 84 of them and `two` 62, nearly all of those pronouns and
articles ("one of these", "the one that", "two of them"). A scanner that demanded an
accounting per hit would demand ~250 of them for words that are not numbers, and this file
has no waiver variant on purpose.

**The discriminator is the noun that follows, not the number word.** A numeral followed by
a unit noun from a closed list is a quantity; `one of these` is not, and falls out with no
declaration from any author. On the target step that cuts 42 raw hits to 25.

## The target step

`the-gradient-itself` — step 17, the surface-gap lesson, the last step ledgered and the most
heavily claimed in the path. Picked because `docs/plans/path-ledger-the-gradient.md` **hand
enumerated its word quantities** when it was ledgered, which makes that list a pre-registered
expectation the scanner can be measured against.

It has already paid. The hand list has 11 entries; the scanner finds 25; and one of the 11,
*"five and a half minutes"*, **does not occur anywhere in `web/app.js`**. The prose says
*"the first minute and a half"*. So the hand enumeration was wrong in both directions before
a line of the check existed — which is the argument for the check.

## Design

1. **A second scanner, not a wider one.** `written_numbers` stays digits-only. A new
   `spelled_numbers` finds `<numeral-word><unit-noun>` and the hyphenated attributive form
   (*"an eighteen-minute discharge"*), and the two are merged in source order. Keeping them
   apart is what lets a step be digit-ledgered without being word-ledgered.
2. **Opt-in per step**, exactly as `[ledger].steps` is: a new list names the steps whose
   words are scanned. One step in it to start.
3. **`Written` grows `scale` and `phrase`.** `token` stays the numeral *in the unit the
   sentence writes it* (`"24"`, `"1.5"`), `scale` is what one of that unit is in the tie's
   own unit (60 for minutes, 3600 for hours, 1 for everything else), and `phrase` is the
   source text for error messages. Every comparison site multiplies the token by `scale`
   or divides the tie's value by it. This is what keeps two things true at once: an instant
   named exactly (*"at three minutes"* = 180 s) still matches a claim's `read_at_s` to
   1e-9, and a rounded one (*"twenty-four minutes"* for 1447 s) is compared at the
   precision the words commit to — half a minute, not half a second.
4. **No measurement-vs-setting classifier.** "Half a point" and "half an hour" are both in
   scope; which one a word *is* is answered by the arm that ties it — a claim, or
   `Tie::Setting` — and that taxonomy already exists.
5. **An unresolvable phrase panics.** A word the scanner recognises as a numeral and cannot
   turn into a value must fail loudly. A scan that silently skips what it cannot parse is the
   five-green-harness shape this repo has shipped once and keeps citing.
6. **Re-anchor `every_word_numeral_is_read_by_something`.** That guard requires every
   `WORD_NUMERALS` entry to be consulted by a claim or a rule. A scanner that iterates the
   table wholesale satisfies it trivially and it stops discriminating — so it has to be
   re-anchored on prose, or this slice disarms a live check while adding one.
7. **Update what goes stale.** `written_numbers`'s "the one scanner" comment and the module
   header's blind-spot paragraph ("four ledgered steps state six measurements that way")
   both become false the moment this lands.

## The open risk

Several of the 25 are counts of the page's own furniture — *"two numbers"*, *"two
electrodes"*, *"Four footnotes"*, *"six gap figures"*, *"three decimals"*. They are real
quantities a reader leans on and the hand list includes three of them, but it is not yet
established that each has an honest arm. Where one does not, the precedents are the DFN
step's (the sentence loses the number) and never a waiver.

---

# What was built

## The scanner

`spelled_numbers` sits beside `written_numbers` rather than inside it, and the two meet in
exactly one place (`ledger_numbers`), which is what stops the scan and the rule-usage guard
answering differently. It reads three shapes:

1. **numeral, unit** — *"three minutes"*, *"half an hour"*, *"fifty simulated seconds"*, the
   hyphenated attributive *"an eighteen-minute discharge"*, and a scale word folded into the
   value (*"three thousandths of a point"* is 0.003, not 3).
2. **unit, "and a half"** — *"the first minute and a half"*, 90 s, where English puts the
   numeral after its unit.
3. **list ellipsis** — *"5.71 at three minutes, 5.80 at six, and …"*, where the unit is
   stated once and carried. **This is the shape that mattered most.** Four of the six
   instants this step's gap claims are read at are written this way; a scanner without it
   would have certified the step while seeing one instant in six.

`Written` grew `scale` (what one of the token's unit is in the tie's unit) and `phrase` (the
source text, for messages). `scale` is what lets *"at three minutes"* match a `read_at_s` of
180 **exactly** while *"twenty-four minutes"* is compared to 1464 s at the precision the
words commit to — half a minute. Canonicalising everything to seconds would have reddened
the second; staying in minutes would have stopped the first from telling six instants apart.

## What the step's sixteen quantities turned out to be

**Seven needed no new anything.** They sit inside a claimed sentence and name the instant
that sentence's own claim is read at, so `claimed_accounting` answers them through
`Accounted::ReadAt` exactly as it does a number in digits. That is the result: the words
saying *when* each of this step's measurements was taken are now tied to the trajectory that
took them.

**Eight took vocabulary rules**, and two of those took new ties:

| the words | what decides them |
| --- | --- |
| *left to rest for half an hour* | the pulse program's off leg, 1800 s |
| *from six minutes on* | the instant a claim two sentences earlier is read at (`Tie::Instant`) |
| *three thousandths of a point* | the difference of the 360 s and 1060 s gap claims — 0.003085 |
| *more than six times the negative's* | the ratio of the two 1060 s gap claims — 6.40 |
| *an eighteen-minute discharge* | the mark less the rest leg, 1060 s → 17.7 min |
| *twenty-four minutes* | the mark less the instant the negative gap first reads zero — 1464 s |
| *four times a second* | `CELLS_PERIOD_MS` said the other way up (`Tie::PerSecond`) |
| *roughly fifty simulated seconds* | the speed slider times that period |

`Tie::Instant` reads a claim's `read_at_s` rather than its value. An earlier slice named this
arm as missing and left it — `path-ledger-weaker-short.md` records an instruction to "run to
about 400 s" leaving the page because nothing decided it, and says a tie reading a claim's
own instant "would have declared from both sides". The circularity worry is real and the
answer is that a claim's instant is **pinned by that claim's value**: move the instant and
the value check reddens. The perturbation that moves the word and the `read_at_s` together
and leaves `value` alone is in the table below precisely to prove that.

**One had no arm and could never have one.** *"an instant some thirty seconds earlier than
the label it was given"* is about a draft of this step that no longer exists. No check in
this repo can reach a deleted draft — this is not a missing arm, it is a number outside what
any arm could address. The sentence keeps its point and loses the figure: *"up to a sample
earlier"*, which is the mechanism the same paragraph already establishes.

# Findings

**1. The pre-registered list was wrong in both directions, before a line of the check
existed.** `path-ledger-the-gradient.md` hand-enumerated this step's spelled quantities when
it was ledgered. It listed 11. The scanner finds 16 in scope, of which **nine were never
listed** — and five of those nine name the instants at which the step's own claims are read.
It also listed *"five and a half minutes"*, which **occurs nowhere in `web/app.js`**. The
prose says *"the first minute and a half"*.

That phrase is now the shape `no_spelled_quantity_is_silently_skipped` exists for: a numeral,
a fraction and a unit, which none of the three shapes reads. It would have been dropped in
silence.

**2. The discriminator is the noun, not the number word.** 293 English number-words across
the path's prose; `one` is 84 of them and `two` 62, nearly all pronouns and articles. A scan
keyed on the numeral would demand ~250 accountings for words that are not quantities, and a
file with no waiver variant would have had one within a slice. Keyed on a closed list of
**measure nouns**, the target step goes from 42 raw hits to 16.

**3. Two tables of number words is a hazard, and the alternative was worse.** The obvious
move is to have the scanner read `WORD_NUMERALS`. That table's guard requires every entry to
be *used*; a scanner iterating it wholesale satisfies that by construction and the guard
stops discriminating — a live check disarmed while looking like coverage. So the scanner has
its own alphabet, and `the_two_word_tables_cannot_disagree` holds them equal where they
overlap.

**4. Merging words into the digit vector is safe, and it is worth saying why.**
`rule_matches` locates a rule's `{n}` by **byte offset**, not by index, so interleaving 16
word entries cannot shift any of the ten rules the previous slice wrote on this step.
`Operand::Sibling` matches by token string with a uniqueness assert, so the one real
interaction fails loudly rather than quietly.

**5. Two scans of one prose is the defect this file is arranged against, and it happened
here for one compile.** The ledger scan merged words; the rule-usage guard still read digits
only. The rule written for *"left to rest for half an hour"* covered its number in the
ledger and matched nothing in the guard, which reported a live rule as dead. Fixed by having
one function (`ledger_numbers`) and only one.

# What is still not seen

**The prose counting its own furniture.** *"two numbers"*, *"two electrodes"*, *"Four
footnotes"*, *"six gap figures"*, *"three decimals"* — nine positions on this step alone.
These are a list's length rather than a measure, and they want an arm that reads a list,
which is the separate open item the previous slice recorded ("a tally that pins a list's
length rather than a sentence's number"). Admitting them here would have swallowed that
project and answered each count with whatever arm happened to equal it. The limit is
declared in the module header and in `UNIT_NOUNS`, on the same terms the digit scanner's
blindness to words was declared for as long as it lasted: **nothing claims those numbers are
tied to anything.**

**Ordinals.** *"the first cycle"*, *"a second 3 C discharge"*, *"the whole second half"*.
Out of scope and unread.

**Twenty-three of the twenty-four steps.** `spelled` has one entry. Every other ledgered
step is exactly as digits-closed as it was.

# The first perturbation round measured nothing, and how it showed

The first round reported 11 of 15 cases as predicted. It was worthless, and the way it
failed is the recorded failure mode of this repo's own scratch harnesses.

`every_claim_appears_in_its_own_step` — the literal check — reddened in **every one of the
fifteen cases, including the control.** A check that fires on every case distinguishes
nothing, and a case list where the control also reddens is a list that has stopped being an
experiment. The cause: an earlier foreground run of the same harness was killed by a
two-minute timeout **mid-case**, after it had applied an edit and before it could restore.
That edit — *"the first minute and a half"* becoming *"the first minute"* — stayed in the
tree, and the background run then read the mutated file as its restore point. Every case
restored *to the broken state*, ran, and reported a red that no case had caused.

Two things follow, and both are about instruments rather than about this slice:

* **A harness must verify its baseline is green before capturing it**, not merely capture it.
  Round two refuses to run otherwise. `docs/plans/path-ledger-what-it-cost.md` records the
  same defect from a different direction and the lesson did not transfer, because it was
  written as "captured a dirty baseline" rather than as "did not check".
* **A killed run leaves the tree edited.** Any harness that mutates the tree needs its
  restore to be reachable after a kill, or the next thing to read those files inherits the
  edit silently.

## And the mutation itself was a finding

With *"and a half"* deleted, the sentence read *"5.28 points after the first minute"* — the
claim beside it is read at 90 s, so the prose was now wrong by half a minute. **The word
scanner cannot see that.** *"the first minute"* has no numeral in it: `first` is an ordinal,
and a unit noun with nothing in front of it is not a quantity. Delete the numeral from a
spelled quantity and the scan has nothing left to object to.

What caught it was the **literal check**, because the deleted words sit inside five claim
literals. That is the same division of labour the digit ledger has always had — the ledger
says every number present is tied, and the literal check says the words around it are still
there — and it is worth stating plainly here, because "the ledger now reads words" invites
the reading that the ledger alone would notice a word going missing. It would not.

## Round two

Every case that perturbs a word **inside a claimed sentence** now moves the prose *and every
literal quoting it*, so the claims and the sentence still agree and the word accounting is
the only thing left that can object. Each case declares the exact set of tests it expects to
redden, and a case that reddens a different set is reported as unexpected even if the count
matches.

## What the rounds established

Eleven cases, and the two that had to be re-run to isolate them:

| perturbation | reddened |
| --- | --- |
| `5.80 at six` → `at seven`, prose **and its four literals** | the ledger, alone |
| `at three minutes` → `at three hours`, prose and literals | the ledger, alone |
| `minute and a half` → `minute and a quarter` | the ledger, alone — **after a fix** |
| `half an hour` → `a quarter of an hour` | the ledger, and the rule whose phrase named it |
| `four times a second` → `five times a second` | the ledger, alone |
| `eighteen-minute` → `seventeen-minute` | the ledger, and its rule |
| `three thousandths` → `three hundredths` | the ledger, alone |
| `five and a half minutes` inserted | the skip guard, plus the ledger and its rule |
| `and a tenth` inserted | the skip guard, **alone** — after a second fix |
| instant → 420 s, **both** sentences naming it moved with it, `value` left alone | **the engine, alone** |
| CONTROL: prose reworded, no number touched | nothing |

**The last row is the one that answers the question `Tie::Instant` raises.** Every string in
the repo agreed with every other: the gap list said *at seven*, the stretch sentence said
*from seven minutes on*, the claim was read at 420 s, and every word arm was satisfied. The
only thing left that could object was the trajectory, and it did — `every_claim_matches_the_engine`
and nothing else. A word naming an instant is tied to a measurement, not to an author's
arithmetic.

## Two defects the perturbations found in the scanner

Both were **silence**, which is the direction that matters, and neither would have been
found by running the suite.

1. **Only `half` was read after a unit.** `minute and a quarter` was seen as no quantity at
   all — the sentence could have said 75 s where the claim beside it is read at 90 and the
   scan would have reported nothing to account for. Shape 2 now reads any fraction.
2. **The guard for that shape was keyed on the numeral, and the shape's numeral need not be
   one.** `minute and a tenth` has no word in `CARDINALS` or `FRACTIONS` in it, so the guard
   skipped it before it could look. It is keyed on the **shape** now — a unit noun followed
   by *"and a"* that no `Written` covers is a failure whatever the third word is.

The general lesson is the one this file keeps relearning from a different direction: a
scanner narrowed by accident fails toward green, and a guard written against the cases you
thought of inherits exactly the blind spot you were guarding against.
