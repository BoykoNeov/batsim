# The number English spells without a numeral

`docs/plans/path-word-batch-three.md` closed with a hand-off naming the **article shape** as
what blocks the next lessons: a quantity written as *"an hour"* or *"a tenth of a volt"*,
with no numeral word anywhere in it to key on. The ban has read that shape since the day it
was built and the reader never had, and the reader's own docs said so in six places.

**Status: LANDED 2026-09-01.** The reader reads articles now, through the shape it already
used for numerals. Two vocabulary rules landed, two sentences that state no figure at all
were reworded, the reader's filler walk was capped to the ban's, and six stale
self-descriptions were retired. `spelled` counts moved on two steps and `word_blind` counts
on two more — because the SCANNER changed, not the prose. No engine code, no snapshot bump,
no wasm rebuild.

## The scope this actually had, which is not the one it was sold with

The hand-off sold the slice as unblocking `past-empty`, `the-electrolyte-starves` and step 4.
That is downstream and none of it happened here. What changing the reader does **first** is
fire on the steps already in `spelled`, because `ledger_numbers` calls `spelled_numbers` for
every step on that list. So the forced set is the article quantities `[[english]]` had been
listing on **word-scanned** steps all along — read by the ban, tied to nothing, and sitting
in a backlog nobody had to act on.

Read by hand before any code was written, which is what set the slice's shape:

| phrase | step | what it is |
| --- | --- | --- |
| *an hour* | `and-it-is-still-in-there` | an instant — the middle reading of a three-item list |
| *an hour* | `what-it-cost` | how far empty would be from a **full** cell |
| *a point* | `belief-drifts` | the unit half of *"a fraction of a point"* — **no figure is stated** |
| *a point* | `what-it-cost` | the same hedge, pointing back at a reading two sentences up |
| *a second* | `what-it-cost` | the denominator of *"a few times a second"* |
| *a tenth of a volt* | `and-it-is-still-in-there` | a round yardstick, hedged with *"most of"* |
| *a tenth of a millivolt* | `what-it-cost` | the same, hedged with *"about"* |
| *an amp* | `nothing-to-clamp` | *"down to hundredths of an amp"* — a magnitude, not a figure |

Two of the eight are figures with an honest arm. Three state no figure at all. Three name
units this reader does not admit. **That distribution is the whole design**: `volt`,
`millivolt` and `amp` stay out of `UNIT_NOUNS`, so the bottom three rows are untouched and
stay in the backlog exactly as they were, and the slice is five phrases rather than eight.

## The premise that was wrong, recorded rather than inherited

The reason for leaving the volt family out is **not** that no arm can answer a voltage. That
was the reasoning offered when the slice was scoped, and it does not survive contact with the
taxonomy: `Tie::Ocv` answers an open-circuit voltage, `Tie::Chemistry("cell.v_min")` answers
a declared one, and a difference of two claims answers a measured one. Both yardsticks above
have honest arms already sitting in the tree, and they are written down here so the next
slice does not have to find them again:

* `and-it-is-still-in-there`'s *"most of a tenth of a volt"* is the step's claimed `1.848 V`,
  one engine step after the load comes off, less the `1.750 V` cut-off the leg ended at.
  That is 0.098, which is `0.1` at the precision the prose prints, and it moves if either end
  moves.
* `the-electrolyte-starves`'s *"a full volt higher"* is that step's own two claimed voltages,
  `3.437` less `2.422`, which is 1.015 and prints as `1`.

So the volt family is out because **admitting a noun forces every article phrase in that noun
at once**, and the two it would force sit on steps this slice was not otherwise touching. It
is a scope boundary, not a capability one. Whoever opens `past-empty` or
`the-electrolyte-starves` should admit `volt` in the same slice and use the arms above.

## The fence, and there is one of it

The estimate written into `path-claims.toml` said reading this shape needs a *"not after
`of`"* fence, so that *"a fraction of a point"* would not scan as one point. **No such fence
was needed, and building one would have been the mistake the ban's own docs record**: the ban
tried exactly that fence in its first draft and it swallowed *"empty is three and a half
minutes away instead of an hour"* the same afternoon — a real quantity sitting behind a real
`of`, and, as it turns out, one of the two this slice went on to tie.

What makes the partitive safe is that it is the same shape with a **scale word** in it. *"a
tenth of a volt"* reads as the article ONE, scaled by `tenth`, carrying the noun behind the
fillers — a tenth of that noun, which is what the words say — and the inner *"a volt"* is not
a second reading, because the overlap skip at the top of the loop already covers it. There is
nothing left for a fence to do.

*"a fraction of a point"* is a different problem and not a fence's: the sentence states **no
figure**. The fraction is the quantity and it is unsaid. So the reader is right to read `1`
there and the accounting is right to refuse it, and the repair is the sentence. Both were
reworded — *"a fraction of that"* and *"That fraction is…"* — which say the same thing about
the same gap and spell nothing. `belief-drifts`' is the weaker of the two and it is worth
saying so: *"a fraction of a point"* bounded the offset's contribution **absolutely**, where
*"a fraction of that"* bounds it against a gap the clause before already attributes mostly to
boot error, so the sentence edges toward tautology. It is still true and still states no
figure. The better repair is a digit, and it needs the offset's contribution measured —
which would want a control arm with the offset zeroed, and would then tie. Rewording a sentence that states no figure is not
`docs/plans/path-twin-arm.md`'s defect; **that** defect is deleting a sentence that states a
true one.

The one real fence is the **ordinal**. *"a second"* is a length of time and *"a second 3 C
discharge"* is the next one along, and they are the same two words — only the noun behind
decides, which is a lookahead the scanner would have to guess at, and guessing wrong reads
the number one out of a sentence stating no quantity. The negative case was already sitting
in `the_word_scanner_reads_quantities_and_not_pronouns`, pointing at prose that no longer
exists; a second case was added beside it. What watches the shape instead is `BANNED_UNITS`,
which refuses `second` behind an article as it refuses every unit — the ban wider than the
reader, which is the licensed direction, and the only place the article shape uses it.

## The two arms

Neither is a new mechanism. Both are arms the file already had, reaching a sentence whose
words the reader could not see.

**`and-it-is-still-in-there` — *"`2.005 V` at an hour"*.** The middle of a three-item list
whose other two items were tied a slice ago: the claim's own instant (4337 s) less the leg
the current was on for (737 s) is 3600 s exactly. The two neighbours use the same
subtraction. Nothing about the arm is new; only the shape of the words was, and that is why
one list of three readings was held by two rules.

**`what-it-cost` — *"empty is three and a half minutes away instead of an hour"*.** This step
starts at 5 % charge, so an hour is a length its own trajectory never visits. It is step 1's:
`bare-curve` runs the same `lfp_26650_generic` cell at the same 2 A from full and claims its
`SOC_CLAMPED_LOW` at 4146.5 s, which is 1.15 h and prints as `1`. Quoting it inherits check 7
where the claim lives rather than re-measuring here. The alternatives are worse rather than
tighter: the cell's nameplate over the demand box is the same quantity **computed** instead
of measured, and it answers in hours where every time arm in this file answers in seconds.

### The borrowed hour needed a precondition, and the arm could not be one

Tying one step's sentence to another step's measurement is only right while the two steps
run the same cell at the same current from full — which the rule's comment **states** and
nothing enforced. That is this file's own recurring shape: a note saying X, falsified by the
first edit that makes not-X true, with nothing pointing that author at the note.

**The arm cannot substitute for the precondition, which is why the assert is bespoke.** The
token prints no decimal place, so it licenses anything from 1800 s to 5400 s. Retype
`what-it-cost`'s demand box to 4 A and every claim on that step reddens, so the author
re-measures all of them — but the borrowed hour has **no claim**, so it goes on pointing at a
discharge now twice as slow as its own sentence's, and stays green. Halve step 1's discharge
and it still prints `1`. Case D of the table measures that the arm names step 1 rather than
any step; `the_borrowed_hour_is_the_same_cell_at_the_same_current_from_full` is the other
half, which is whether step 1 is still the right one to ask. Its three arms — same demand,
same chemistry, and a source that starts at **full**, because that is the whole content of
the word *"instead"* — each redden on their own assertion:

| case | verdict | reddened |
| --- | --- | --- |
| J — `what-it-cost`'s demand box 2 A → 4 A | RED | the demand equality |
| K — step 1's scenario re-pointed at another chemistry | RED | the chemistry equality |
| L — step 1 no longer starts at full charge | RED | the `initial_soc` assert |

Not folded into the vocabulary as another `Tie`: what it asserts is an equality between two
lessons' **settings**, not a number a sentence prints, and there is no numeral in the prose
for it to account for.

Both arms are loose, because both sentences are: a token with no decimal place licenses half a
unit either way. That is the rule this file keeps everywhere — a sentence is held to the
precision it prints — and it is the same tolerance step 24's *"at four"* was deliberately
given when a tighter and wrong arm was available.

## The cap nobody would have found by reading

`english_quantities` walks at most four connectives from its head; the reader's walk was
unbounded. The cap went on the **whole** of shape 1 rather than on the article branch alone,
so the numeral path is capped now too; case I says it is inert for both today. That difference was invisible while the reader keyed on numerals, because a
numeral is rarely followed by a long filler run — and an article is. A reader that walked
further than the ban would find a quantity the ban never spanned, and
`the_ban_sees_every_quantity_the_reader_reads` would have reddened on a lesson nobody had
edited. The two walks are the same length now rather than the same length by luck. Case I of
the table below is what that cap is worth today, and the answer is a green.

## The perturbation table

Nine cases, each against a control run confirmed green first. Every case's own panic text was
read rather than its exit code, because the accounting check panics in prose order and a
table of red exit codes understates itself.

| case | verdict | what reddened |
| --- | --- | --- |
| control | GREEN | — |
| A — step 24's *at an hour* → *at two hours*, prose + claim literal | RED | the ledger, on `v_at:4337` less `PulseOn` — and the ban |
| B — the tag `v_at:4337` moved one second, prose untouched | RED | `every_claim_matches_the_engine`, on the tag's own assert |
| C — `what-it-cost`'s *instead of an hour* → *two hours* | RED | the ledger, naming step 1's claim — and the ban |
| D — that hour tied to its OWN clamp instant instead of step 1's | RED | the ledger: 207.5 s is not an hour |
| E — `belief-drifts`' hedge put back | RED | four checks, the ledger among them |
| F — `what-it-cost`'s hedge put back | RED | four checks, the ledger among them |
| G — the ordinal fence lifted | RED | the scanner's own negative case **and** the ledger, on *a second* |
| H — the article head removed | RED | five checks, `no_spelled_quantity_is_silently_skipped` among them |
| I — the walk cap lifted from four to four hundred | **GREEN** | — |

**Case B is what the loose tolerance costs, stated rather than left to be discovered.** The
arm compares at the precision the prose prints, which is none, so it licenses half an hour
either way — moving the reading a second cannot redden it, and 3601 s still prints as `1`.
What catches that edit is the tag's own assert against `read_at_s`, which is why tagging the
instants was worth doing before quoting them. Two independent objections to one edit, and
only one of them is the ledger's.

**Case D is the one that says the arm is about step 1 and not about a number.** Both steps
claim `flag_first_s:SOC_CLAMPED_LOW`; pointing the tie at `what-it-cost`'s own reads 207.5 s,
which is three and a half minutes and not an hour. The arm names `(step, quantity)` and that
is what makes it a statement rather than a search.

**Case H is what the guard's new direction is for.** Taking the article head back out leaves
five checks red, and one of them is `no_spelled_quantity_is_silently_skipped` firing on *"an
hour"* — the shape it could not see before this slice. A reader that lost the shape again
would be told so by the guard rather than by a green over an empty list.

**Case I is a green, and greens are results.** Lifting the cap from four to four hundred
reddens nothing: no lesson in this path today puts five fillers between an article and its
noun, so the cap is precautionary rather than load-bearing. It stays, because what it
prevents is a cross-check reddening on a lesson nobody edited — but nothing in the tree
demonstrates it, and this row is that admission.

## The counts that moved, and one class of them is new

`spelled`: `what-it-cost` 5 → 6, `and-it-is-still-in-there` 5 → 6. The tally over the whole
list: eighteen spelled quantities → twenty. `[[english]]`: 47 phrases → 45, over the same
twelve steps.

**And `word_blind`'s counts moved without a word of prose changing on those steps**:
`the-electrolyte-starves` 0 → 2 and `past-empty` 5 → 6. Those entries record what a scan
*would* find on a step nobody scans, so a new shape moves them the way a new sentence does —
which is what that column is for, and the first time it has been demonstrated. Neither step
joins the scan here: each still holds a volt.
