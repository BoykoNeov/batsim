# The dense DFN step, scanned whole, and the five numbers that left the page

`docs/plans/path-ledger-particle-step.md` measured step 16 (`the-electrolyte-starves`) and
did not ledger it. Its verdict: *"11 numerals already claimed, 13 reachable by arms that
exist, 1 reworded, 5 written as claims on its own pack, and 13 that are not work but
decisions."* This slice ledgers it. **The decisions turned out to be most of the work, and
two of the four piles were mis-priced in both directions.**

Step 16 is the densest step in the path — 43 numerals when it was measured, against a
previous high of 26 — and its whole argument is about the step next door: *"the same cell, at
the same current, at the same instant"*, with one block of the scenario file swapped. So most
of its numbers are not its own, and the question the slice had to answer for each was not
"what does the engine say" but **"whose measurement is this, and what in the tree can be held
to it."**

It now prints **33 numerals and every one of them is tied to something**: 12 claimed on its
own pack, 9 that step 15 measured or that are arithmetic over step 15's claims, 4 worked out
from this step's own claims in sentences no claim quotes, and 8 constants and controls.

## What the previous measurement got wrong

**Two of the "13 reachable by arms that exist" were not reachable.** `15.46 A` and
`this cell's 5.15` are the demand box and the cell's nameplate printed at two decimal places,
and a constant tie is compared **exactly** — `tie_agrees` gives the rounding treatment to
computed ties only, on the stated grounds that "a constant printed in prose either is the
file's number or is wrong about it". A rounded restatement of a constant is neither, and it
had no arm. The fix is the prose: it prints `15.459594` and `5.153198` whole, which the step
already does twice elsewhere, and **an arm that would have existed to accept rounded
constants does not get built.** That is the better outcome — the arm's failure mode is that
`5` would account for `5.153198`.

**One of the "5 written as claims on its own pack" cannot be a claim at all.** The plan listed
"the two instants of *over the 64 seconds from 400 s to 464 s*" as claim-writing. Neither is
writable: `400` can only be accounted as an instant some claim in that sentence is *read at*,
and a claim in that sentence has to state something the sentence prints — and the only numbers
it prints are `64`, `400`, `464` and `535`, of which the engine names exactly one
(`t_at_v_below`, which is 464). `States::Nothing` needs a literal with **no digits at all**,
and `TolFrom::Grid` is fenced to flag arrivals, so there is no admissible tolerance rule for a
silent claim on a voltage. The clause is reworded to "over the minute that ends at 464 s",
where `464` is this step's own crossing claim and the minute is a word.

**And the blocker the last slice named as the expensive one had already been paid.** It
recorded that step 15's eight voltages shared one name and could not be quoted; the slice
after it tagged them (`v_at:400`), so six of this step's numbers were quotations waiting for a
sentence. That is what made this slice affordable at all.

## The two new arms

Both are the same shape as the arms beside them, and each was built because a sentence needed
it rather than to fill a slot.

**`Tie::Difference`** — `Tie::Ratio`'s sibling: the first of two ties less the second, order
load-bearing, compared at the prose's own precision. Three sentences need it and not one is
reachable any other way, because each prints *the answer and neither operand*:

| the sentence | the difference |
| --- | --- |
| "it drops **535 mV**" | this step's own claims at 400 s and 464 s |
| "the single-particle arm falls 34 mV" | step 15's claims at the same two instants |
| "has 596 seconds still to run" | step 15's cut-off crossing less this step's |

`Tie::Derived`'s `Difference` is the same arithmetic over a sentence's **own printed tokens**,
which is why it cannot serve here — the distinction between the two families is exactly
whether the operands are on the page.

**`Tie::Hours`** — a wrapper reading a duration in hours where the file reads seconds, for
*"15.459594 A for 464 s is 1.99 A·h"*. A rule's `pow10` cannot carry it: 3600 is not a power
of ten, and nothing else in this file converts one. It is a **unit and not a number**, on the
same footing as `to_c` and the `pow10` that turns a fraction into a percentage, so no author
supplies a value. It is compared like a computed tie rather than delegating to what it wraps,
which is the opposite choice from `Tie::Elsewhere` and deliberate: a conversion by a
non-decimal factor never lands on a round number.

The alternative reading of that sentence — capacity times the complement of the state of
charge, which comes to the same 1.9926 by construction — was refused for step 23's reason
against tying a C-rate to `max_discharge_c`: it is right off the wrong reading. The sentence
names a current and a duration.

### `Tie::Hours` is the first arm no rule uses at the top level

It sits inside a `Tie::Product`, and `n_ledger_arms` counted only each rule's outermost ties —
indistinguishable from the truth for as long as every arm was some rule's outermost one. Left
alone, the file's own prose would have listed one more arm than the count beside it, which is
the drift those tallies exist to catch. The walk now descends into `Product`, `Ratio`,
`Difference`, `Hours` and `Elsewhere`; the count is unchanged for every arm that existed
before, so this is a fix to the derivation and not a change of definition.

## A step quoting itself

Four of this step's numbers are decided by claims on *this step* — its crossing instant,
printed in two further sentences, and the two readings its 535 mV collapse is the difference
of. `claimed_accounting` cannot reach any of them, because it is positional: a claim accounts
for a number only inside the literal of its own sentence, and no claim quotes those sentences.

So `Tie::Quoted` names this same lesson, and there is no fence against it. `Tie::Elsewhere`
refuses naming its own step — a wrapper that changes *which lesson answers*, pointed at this
one, is the arm it wraps with extra words — and this is a different thing: what it names is a
**measurement**, and the claim it resolves to still answers to the engine where it lives. The
alternative was four numbers tied to nothing.

## The one edit outside step 16

Step 16's sharpest comparison is *"The twin, at that same instant, reads 3.437 V — a full volt
higher"*, and the twin's reading at 464 s could not be claimed on step 15 for the reason given
above: a claim has to be pinned to a number its own sentence spells, and step 15's list of
readings stopped at 460 s. So **step 15's prose now prints `3.437 at 464`**, one more sample
in a list that already gives 440 and 460, and the claim on it feeds both `3.437` and the
34 mV fall.

It is a prose change to a lesson this slice was not about, made to serve the ledger of the
lesson next door, and it is recorded here rather than left to be inferred. It earns its place
on step 15's own terms — it is the flattening that sentence is about — but it would not have
been written without step 16.

Step 15 also gains a claim that needed **no** prose change: `t_at_v_below:2.5 = 1060`, spelled
off the sentence it already prints (*"**2.495 V at 1060 s**, which is where it crosses the
2.50 V cut-off"*). It moves that token's accounting from "the instant we measured" to "where
the cell reaches its cut-off", which is what the sentence actually says, and it is what step
16's `596 seconds` and `factor of 2.28` are both differences and ratios of.

## Five numbers left the page, and the option that was refused

Five of step 16's figures cannot be tied to anything in this repo:

* **`140×` and nearly `500×`** — per-step cost ratios between the two models. No trajectory
  settles a cost; the same category `what-protection-costs` is unledgered for.
* **`12 seconds apart in 3484 — 0.34 %`** — the 1 C boundary. Both halves need a full
  hour-long discharge on *two* models, one of which is another lesson's pack: `run` builds one
  pack per (step, arm), and an arm must be instructed by its own step's prose, so no arm on
  step 15 can exist for a sentence step 16 writes. Claiming even the DFN half would add
  roughly 1748 steps of the most expensive cell model in the engine to the default test gate.

They are reworded out: the cheap model "costs a small fraction of what this one does per
step", and at 1 C the two models "reach their cut-offs **within a fraction of a percent of
each other**". The measured values survive in `scenarios/cc_discharge_3c_dfn.toml`'s header
and in `docs/plans/path-prose-ledger.md`, which is where a number nothing checks belongs.

Two more went the same way for related reasons: **`3.798 V`**, the equivalent circuit's
zero-length probe, is a *third* pack that no lesson in the path runs at this current — the
sentence now gives two numbers and a direction; and the **`1 C`** gloss beside `5.153198 A`,
which `Tie::Ratio` would have accounted as that field divided by itself, an arm that resolves
to 1 whatever the file says. It reads "the current that would empty this cell in an hour".
`Phase 7` is reworded to "its porous-electrode phase" on the precedent step 13 set for
`Phase 6`.

**One tempting arm was refused before it was written.** `scenarios/cc_discharge_3c_dfn.toml`
states the 1 C agreement in a `description` **field** — "at 1 C the two agree on the cut-off to
0.34 %" — and `Tie::Name` reads digit runs out of a named string field, so a rule could have
accounted `0.34` against it. That is the number written down twice and nothing measuring
either copy: no test reads that string, so the tie would resolve to whatever a human last
typed. Every existing `Tie::Name` reads an *identity* — a part number, a parameter set's year —
which is a fact a file gets to declare. A measurement is not.

**Spelling them in words was the option refused, and it is worth saying why.** The scanner
finds digits, so "two orders of magnitude" and "a third of a percent" would have cleared it —
and the step whose five most load-bearing comparative numbers are unchecked would have gone
green. The blindness this file already admits to (step 3's "about half a point") is
pre-existing prose it names as a gap; manufacturing five new instances to satisfy the check
turns the check into a fact about the author. A page that stops asserting what nothing can
verify is the honest version of the same edit.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| prose `same 100 %` → `same 99 %` | the initial-charge tie |
| scenario `initial_soc = 1.0` → `0.99` | the same tie from the file side, and this step's claims with it |
| prose `same 25 °C` → `same 26 °C` | the ambient control tie |
| prose `where the twin read 3.927` → `3.928` | `Quoted`, on step 15's probe claim |
| step 15's `v_at:0` value → `3.9` | the same quotation from the claim side, and check 7 |
| prose `against the twin's 3.918 on the first step` → `3.919` | `Quoted`, step 15's first stepped row |
| prose `where the other reads 3.471` → `3.472` | `Quoted`, step 15 at 400 s |
| prose `the minute that ends at 464 s` → `465 s` | `Quoted`, this step's own crossing claim |
| prose `it drops **535 mV**` → `536` | `Difference` over this step's own two claims |
| that rule's two operands swapped | the same tie, on the sign |
| prose `arm falls 34 mV` → `35` | `Difference` over step 15's two claims |
| step 15's new `v_at:464` value → `3.43` | the same difference from the claim side, and check 7 |
| prose `past the 2.50 V cut-off` → `2.55` | the chemistry tie |
| chemistry `v_min = 2.50` → `2.55` | the same tie from the file side |
| prose `1.99 A·h` → `2.00` | the product with `Hours` in it |
| that product's `Hours` wrapper removed | the same tie, now in ampere-seconds |
| prose `of this cell's 5.153198` → `5.153199` | the capacity tie |
| prose `reads 3.437 V` → `3.438` | `Quoted`, step 15's new claim |
| prose `has 596 seconds still to run` → `597` | the cross-step `Difference` |
| step 15's `t_at_v_below:2.5` claim deleted | that difference and the 2.28 ratio, both resolving to nothing |
| prose `factor of 2.28` → `2.29` | the `Ratio` of the two crossings |
| prose `(At 3 C the cost gap` → `(At 4 C` | the C-rate ratio |
| prose `set the current to 5.153198 A` → `5.153197` | the capacity tie again, on the other sentence |
| the new `2.808 here` claim deleted | the ledger, on a numeral nothing accounts for |
| `n_ledger_arms`'s nested walk reverted | the arm-count tally, at fifteen against sixteen |
| **`Tie::Hours` lifted out of `tie_agrees`'s rounding group** | **nothing — a registered GREEN, added after the run** |

### What the table found

**All twenty-five reddened, each on the check it was registered against**, and the two
deletions took the two derived counts with them, which is the tallies working rather than a
surprise. Three rows are worth reading twice:

* **The two file-side edits redden from the other direction.** Moving the scenario's
  `initial_soc` or step 15's claimed value fails `every_claim_matches_the_engine` as well as
  the ledger — the prose and the file are pinned to each other, not the prose to itself.
* **Swapping the 535 mV rule's two operands reddens it.** That is the whole content of "order
  is the claim" for `Tie::Difference`, and it is checked rather than asserted in a comment.
* **Removing the `Hours` wrapper reddens the arm-count tally too.** The conversion is the only
  reason that arm is in the vocabulary at all, so deleting it changes what the file says about
  itself. Registered as one redness; it came back as two, both correct.

**And one case was added after the table, because the table had a hole in exactly the place
this document lectures about.** `Tie::Hours`'s doc paragraph says its comparison is *"like a
computed tie rather than delegating to what it wraps"* — a live behavioural choice, in prose.
Lifting the variant out of `tie_agrees`'s rounding group leaves **all 28 tests green**: its one
user is a factor of a `Tie::Product`, and `tie_agrees` is asked about a rule's *outermost* tie,
so the product's own rounding decides that sentence and the new arm is never entered. A
comparison arm nothing reaches is the `CCCV_PERIOD_S` shape — pinned, and consulted by nothing
— which `Tie::Derived`'s own doc in this same file names as the thing to avoid.

The fix is a test that asks the question directly, with both sides handed in:
`an_hours_tie_rounds_the_way_a_computed_tie_does` requires the hours tie to accept
`1.9925… → "1.99"` **and** a constant tie to refuse the identical pair. The second assert is
what makes it a test rather than a restatement, and it is the same distinction that sent
`15.46` off the page. With it, the green above is red on that test alone.

## Learned while building

**A perturbation that reddens EVERY test has broken the instrument, not the code.** The first
run of the two claim-deletion cases failed all 28 tests. That reads like enormous coverage and
is the opposite: the block-cutter searched for `"""` starting *at* `note = """`, found the
opening marker's own quotes, and left the TOML unparseable, so every test died in
`parse_claims_file`. The tell is the same one the last slice named — read *which* tests failed,
not whether the run failed. A deletion perturbation should redden a handful of checks by name;
a full sweep means the file no longer parses.

**A `quoted` claim can earn its green off the sentence next door.** The new `2.808 here` claim
was written `quoted = true` and passed, because that check looks for the row string in the
step's whole **prose** and `2.808 V` is printed in the sentence above — which a different claim
owns. The flag is now off, on the same grounds step 15's `3.439 at 460` records: the
parenthetical writes the number without its unit, so it does not hand the reader a row string
at all. This is also the first live instance of the independent bite the file's own header says
`quoted` is kept for, and it arrived pointing the wrong way: at a sentence that is not the
claim's.

**A nested arm's own comparison is unreachable, and the doc paragraph describing it is the
tell.** The same nesting that hid `Tie::Hours` from the arm count hides its `tie_agrees` arm
from every rule: the outermost tie is what gets asked. What made this findable was not a test
but a *sentence* — a paragraph asserting a deliberate choice between two behaviours, in a
variant no rule reaches at that layer. **When a doc comment claims a behavioural choice, ask
which run enters that code.** If the honest answer is "none", the paragraph is the defect, and
the options are a direct test, a deletion, or a recorded green — in that order of preference.

**An arm used only inside another arm is invisible to a count that walks the outside.**
`Tie::Hours` never appears at the top of a rule — it sits inside a product — and
`n_ledger_arms` counted outermost ties only. Nothing was wrong with that walk until an arm
existed that no rule uses outermost, and then the file's own prose would have listed sixteen
arms beside a derived fifteen. Every arm that existed before is counted identically by the new
walk, which is what makes this a fix rather than a redefinition.

## What is left

* **Fourteen steps are still claims-only**, and this was the expensive one. The three cheapest
  remaining are the two the last measurement pass found need no measurement at all plus
  `the-gradient-itself`, whose numbers are almost all measurements on the pre-Run probe.
* **The twin-arm capability is now the named gap.** Three of this step's five departed numbers
  need one pack to run another lesson's scenario file — an arm that names a scenario, and a
  `Tie::Quoted` that can tell two arms of one step apart (the last slice priced the second
  half at 22 ambiguous groups). It is the only thing standing between the 1 C boundary and a
  page that states it.
* **The two cost ratios are not coming back.** No trajectory settles a per-step cost, and a
  timing-based check is not a check this file can hold. If the comparison matters on the page
  it belongs in a sentence that says "measured in the Phase 7 plan" rather than in a number
  the reader is invited to trust.
* **Step 16's own readings are now tagged (`v_at:400`), so a later step can quote it.** Nothing
  does yet.
