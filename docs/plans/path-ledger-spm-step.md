# The half of the pair that looks fine, ledgered — and the neighbour that made it cheap

`looks-fine-from-outside` is step 15: an LG M50 cell pulled from full at 3 C on a
single-particle model, which produces a smooth, plausible, wrong answer and raises no flag
about it. It is the **sixteenth of twenty-four** steps scanned whole.

It cost eleven vocabulary rules, **no new arm and no new claim**. The five ledger slices
before it added between one and eight claims each; this one adds none. What it cost instead
was three claims next door losing their anonymity, and two numbers on the page gaining six
digits.

## The pick, and a proxy that was close enough

The recorded ranking nominated this step at nineteen unaccounted numerals and told the next
slice to re-measure rather than trust it. Re-measured, it is still nineteen and still the
cheapest.

| step | unaccounted, this slice | recorded last slice |
| --- | --- | --- |
| `looks-fine-from-outside` | **19** | 19 |
| `one-step-that-got-through` | 20 | 21 |
| `leg-that-is-not-there` | 22 | 22 |
| `what-protection-costs` | 27 | 25 |
| `past-empty` | 27 | 27 |
| `three-times-the-current` | 31 | 31 |
| `what-it-cost` | 34 | 35 |
| `same-discharge-other-chemistry` | 35 | 35 |
| `the-gradient-itself` | 38 | 38 |

The measurement is a **proxy** and not the instrument: rather than putting every step into
`[ledger].steps` and reading a print-and-continue run, this one re-implemented the scanner
— digit runs, the thousands-space join, the claimed-literal spans — outside the tree and
counted numbers that no claim's literal contains. Two things validate it. It reproduces the
ledgered total exactly (275 numerals over the fifteen steps that were scanned), and pointed
at a *ledgered* step it finds zero unaccounted numbers, which is what the suite says.

Where the two columns differ, the proxy is the one that is wrong, and it is wrong in two
knowable ways. It does not know that a rule written for another step can reach this one —
`what-protection-costs` is 27 to the proxy and 25 to the instrument because step 3's two
scatter rules reach it. And it counts a number inside a claimed literal as covered, where
the real check asks whether a claim *accounts* for it; that is the extra one on
`one-step-that-got-through` and on `what-it-cost`. Both errors run toward optimism, and
neither changes which row is first.

## What the sweep found before a line was written: nothing

The blocking pre-check was to match all 110 existing vocabulary phrases against this step's
prose. **Not one of them reached it.** That is two answers at once: no rule was going to cut
the nineteen down, and no rule was silently accounting one of this step's numbers off the
wrong field — the `"{n} mV"` hazard `LedgerRule`'s own docs warn about. Nineteen numbers,
nineteen accountings to write.

Nine are constants and controls, three are ordinals pointing next door, three are the
sentence's own arithmetic, and four are quotations. Eleven rules carry them, split where a
percentage and a voltage sit in one sentence — `pow10` is a property of the rule, so
"3.418 V, 58.3 %" is two rules and not one.

## The two numbers that left the page in favour of their full selves

The step's arithmetic sentence read *"15.46 A for 1060 s is 4.55 A·h — 88 % of this cell's
5.15"*. The `15.46` is the demand box and the `5.15` is the nameplate, both rendered to two
decimals, and a constant tie is compared **exactly** — `tie_agrees` rounds computed ties
only, on the stated grounds that a constant in prose either is the file's number or is wrong
about it.

This is the same defect, in the same two numbers, that the twin step hit when it was
ledgered: `docs/plans/path-ledger-dfn-step.md` records `15.46 A` and `this cell's 5.15` as
the two entries in its "reachable by arms that exist" list that were not reachable, and
records the decision — **the prose prints them whole, and an arm that accepts rounded
constants does not get built**, because such an arm's failure mode is that `5` accounts for
`5.153198`. The same fix, for the same reason, one slice later and one lesson to the left.
The sentence now reads as its twin's does.

Nothing else about the prose moved. The step's own measurements are all claimed already,
which is why this slice wrote no new claim.

## The floor, and why quoting it cost the step next door its anonymity

*"it eventually pins near 0.3 V, which is the floor step 14 mentions"*. Step 14 measures
that floor and states it as `0.3095 V`; a quotation compares at the prose's own precision,
so one decimal against four is exactly what `Tie::Quoted` is for.

It could not be quoted. Step 14 carried **three** `v_at` claims on one arm — 0.612147 at
11 280 s and 0.309467 at two instants 660 s apart — and a quotation's address is
`(step, arm, quantity)`. Three readings under one name is the ambiguity the agreement fence
refuses:

> a rule quotes step `three-times-the-current`'s `v_at` on the arm `past the clamp`, and the
> claims on it answer differently: [0.612147, 0.309467, 0.309467].

So this slice tagged them — `v_at:11280`, `v_at:12600`, `v_at:13260` — which is the move the
`Tie::Quoted` docs say a step owes a sentence that wants to quote it, and which step 15 had
already paid for itself when the twin was ledgered. The tag is asserted against the claim's
own `read_at_s`, so it cannot become a second, disagreeing address.

**The perturbation for this was wrong the first time, and the way it was wrong is the
interesting part.** Untagging *one* of the three and running the suite goes red — but for
the wrong reason: it reddens because the rule names `v_at:12600` and nothing answers to that
name any more, not because the quantity is ambiguous. With one of three untagged, `v_at`
names exactly one claim and would have resolved cleanly. The case that justifies the retag
is untagging all three **and** pointing the rule at `v_at`, and only that one produces the
message above. A perturbation has to remove the thing it is arguing about.

**And the retag changed a fact, so the tree was swept for prose still asserting the old
one** — the defect this repo has recorded four times, where a note explaining why something
could not be done outlives the reason. Grepping the test, the claims file and every plan doc
for "untagged", "unquotable", "cannot be quoted" and "three readings" turns up nothing that
states step 14's voltages share a name: the claims file's one such note is about step 8 and
is correct, and the test's are about step 15 and are correct. The one near miss is
`path-ledger-particle-step.md`, which says step 15's readings "cannot be quoted until those
readings are split into per-instant quantity names" — a record of that slice's moment, which
names the payment as the next slice's and got it. A clean sweep is a result and is written
down as one.

## Perturbations

Sixteen cases, each one edit, run, revert, with the exit code read off the process rather
than off `start`. Every one is red, and every one on
`every_numeral_in_a_ledgered_step_is_accounted_for`.

| perturbation | verdict |
| --- | --- |
| demand box in the prose off by one in its last digit | red |
| the rounded demand box (`15.46`) restored | red |
| the rounded nameplate (`5.15`) restored | red |
| shell count 20 → 21 | red |
| the cut-off 2.50 → 2.55 | red |
| the amp-hours 4.55 → 4.56 | red |
| the fraction of the nameplate 88 → 89 | red |
| the mark in the closing instruction 500 → 501 | red |
| the charge next door 90 → 95 | red |
| the floor 0.3 → 0.4 | red |
| the ordinal pointed at step 12 instead of 13 | red |
| the `Elsewhere` pointed at a step with a different charge | red |
| the floor quotation pointed at the other tagged instant | red |
| the closing voltage quoting `v_at:464` instead of `v_at:500` | red |
| the mark's rule deleted outright | red |
| step 14 untagged entirely, the quotation pointed at `v_at` | red |

Five of them were re-run with the panic message captured, because an exit code does not say
*which* assertion fired — a lesson this file has already paid for. Each names the number and
the rule it was aimed at: the deletion says "prints `500` and nothing accounts for it", the
ordinal says "says `13` where the position of the lesson `circuit-repeats-itself` in the path
says [12.0]", the rounded box says "says `15.46` where the lesson's DemandValue control says
[15.459594]", the amp-hours says "says `4.56` where the lesson's DemandValue control times
… `t_at_v_below:2.5`, in hours says [4.551991566666667]", and the untagging says what is
quoted above.

## Four stale numbers on the neighbour, all about the neighbour

The twin, step 16, is described in two places and both had drifted:

* `[ledger]`'s entry says *"38: … **Fourteen** are claimed on its own packs; nine are
  readings the twin measured …; six more are worked out from this step's own claims …; and
  the last eight are …"*. Fourteen and nine and six and eight is thirty-seven, and the entry's
  own headline — the one number in it that a test derives — is thirty-eight. Measured, the
  numbers inside its claimed literals are **fifteen**, and 15 + 9 + 6 + 8 is 38. One word,
  and the arithmetic closes.
* The vocabulary table's section comment says *"40 numerals, of which 12 are claimed on its
  own pack. **Ten** of the rest are readings the step NEXT DOOR measured … **Four** more are
  readings of its OWN pack that a claim elsewhere in the step decides"*. Measured today: 38,
  15, nine, and five — with a sixth worked out from two of those five by `Tie::Derived`,
  which is the number the `[ledger]` entry's "six more" counts and the reason the two
  descriptions looked as though they disagreed. Every numeral of that step now falls in
  exactly one bucket: 15 claimed, 9 involving the twin, 5 quoting itself, 1 derived from two
  of those, 8 constants and controls — 38.

  The fourth number in that sentence was left stale in the first pass of this slice, with
  the three around it corrected. That is worse than leaving all four alone: three corrected
  numbers are a signal that the sentence was checked. It was caught in review, and the fix
  is to state the categories so both descriptions count the same way.

Both are prose about a *derived* number, sitting next to the derivation that keeps the
derived one honest — the shape three slices have now recorded as the way these files rot.
Neither is checked by anything, and this slice found them only because it re-measured the
neighbour to validate the proxy.

Five more self-counts *were* checked, and the suite named all five in turn as they went
stale: the ledgered step count in two files, the numeral total (275 → 313), the count of
unledgered steps that carry claims (nine → eight), and the module doc's "the other nine have
their claimed sentences checked". That is the difference between a tally with a derivation
behind it and a sentence beside one.

## The word-numeral blind spot, with a live instance

This step's arithmetic sentence ends *"in the eighteen minutes the clock will be showing"*.
That is a derived number written in English, inside a ledgered step, and **nothing in the
suite can see it**: the scanner finds digits. It was checked by hand — the panel's clock
renders `${(s / 60).toFixed(0)}m` below 7200 s, and 1060 s is 17.67 minutes, which prints as
`18m` — so the sentence is right, and it is right by inspection rather than by assertion.

Four slices running have now recorded that word-form self-description rots invisibly. This
is the first time the instance is a statement about the *engine's* output rather than about
the files' own contents, which is a step worse: a wrong "eighteen minutes" would be a reader
being told the wrong thing about what they are looking at. The standing options are
unchanged — give the phrases digits, or wire `WORD_NUMERALS` into the ledger scan — and the
second is now worth more than it was, because a ledgered step is where the claim "every
number here is tied to something" is made out loud.

## What the ledger looks like now

Sixteen of twenty-four steps scanned whole, 313 numerals, 121 vocabulary rules, nineteen
arms. Eight steps left.

**Three slices in a row with no new arm.** The taxonomy has been finished since step 22, and
what a step costs now is whatever instrument its sentences need: last slice it was six new
quantities on the harness, this slice it was three instant tags on a neighbour's claims.

By the table above the next step is `one-step-that-got-through` at twenty, with
`leg-that-is-not-there` two behind it — and both numbers come from the proxy, which is
optimistic by one or two for exactly the reasons stated above. Re-measure with the
instrument before budgeting, and note that neither of those steps has a twin that has
already been scanned. This slice was cheap because step 16 had done most of its work for it.

**The two vocabulary rules that reach a step they were not written for are still two.** All
121 rules were matched against all 24 lessons before this slice was committed: both of step
3's scatter rules still reach `what-protection-costs`, where they are right by a coincidence
of values, and **none of the eleven added here reaches a second step**.
