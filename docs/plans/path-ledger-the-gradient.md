# The last step, ledgered — and a diffusion time nobody had divided

`the-gradient-itself` is step 17 of the guided path, the lesson that puts the surface
gap on the readout row. It was the one step of twenty-four whose prose the ledger had
never scanned: four claims on the pre-Run probe, and everything after them free. It is
scanned now, so `[ledger].unledgered` is **empty for the first time** since the ledger
was built.

Three consecutive slices named this step as what was left, and two of them said it would
be the most expensive one. It was not the most expensive — `what-it-cost` at 58 numerals
and `same-discharge-other-chemistry` at 43 both cost more work — but it is the step whose
numbers are most nearly all its own, which is a different thing and is what made it slow:
twenty-nine of its forty-four numerals sit inside a claimed sentence, and only fifteen are
anything a file could answer for.

## What had to be accounted, and how it divided

Forty-four numerals in `prose` and `expect` — the same slice of `lesson_text` the scan
takes. Measured with the ledger's own scan rather than ranked from the out-of-tree proxy;
the last recorded figure for this step was 42, taken before this slice added two instants
to the prose.

| | count |
| --- | --- |
| inside a claimed sentence | 29 |
| answered by a vocabulary rule | 15 |

The four claims that were already here account for four of the twenty-nine. The other
twenty-five are new, and they make this **the most heavily claimed step in the path** —
all of them on one trajectory. That is what a `Pulse` demand buys: `on_s = 1060`,
`off_s = 1800`, one record carrying a 3 C discharge to the cut-off and then half an hour
of rest, with the leg a pure function of `sim_time_s` so a reader arriving by **Back**
gets the same thing.

The fifteen a rule accounts for, named rather than counted: five ordinals — two pointing
into the middle of the path (steps 13 and 16, the two lessons this one is between), two
spanning the whole equivalent-circuit half (`steps 1 to 12`, the run of lessons whose gap
row says "no electrodes"), and one at the twin that shares this scenario file; the demand
box; the speed slider; the topology written `1S1P`; the rate that box works out to; the
`2020` inside `Chen2020`, which is a publication year in the chemistry's own provenance
string and no more a quantity than the `50` of an LG M50; the two diffusion times; and
two figures worked out from this step's own claims.

## The one new shape, and the one new quantity

**A ratio of two differences.** *"3.352 V is all it will ever reach, so about 96 % of the
rebound is already over"* prints one of the three voltages the fraction is built from.
The other two — where the rebound started (the cut-off, 2.495 V) and where it had got to
(3.319 V at the crossing) — are elsewhere: one is step 15's claim on its own continuation
arm, and one is this step's. So the rule is

```
Ratio([ Difference([v@1396, cutoff]), Difference([v@2860, cutoff]) ])
```

with all four leaves `Tie::Quoted`. `tie_values` already recursed, so nothing had to be
built for it; what is new is that an arithmetic tie now nests inside another one in the
vocabulary rather than only in principle. The cut-off is quoted from step 15 rather than
re-measured here for the reason that arm exists: the two lessons are the same file at the
same current, and quoting is what keeps their accounts of one trajectory from drifting
apart.

**`gap_neg_zero_s`.** The step's central sentence is *"by the time the negative gap first
reads 0.00"*, and `first` is the load-bearing word. A claim read at an instant declared in
this file would have said "we measured then", which is [`Accounted::ReadAt`]'s own stated
weakness — so the crossing is a reduction over the run, `t_at_v_below`'s shape with the
threshold taken out of the author's hands.

The threshold is **the display's**, not a number in the claims file: the first row whose
`fmt_gap_pts(neg, 2)` is `"0.00"`, which is the same mirror the display check runs on and
carries the page's negative-zero guard with it. A `gap_neg_below_pts:<x>` would have let
the sentence be true of whatever `x` made it true. The quantity also asserts that the zero
is **final** — a first match answers a flicker as readily as an arrival, and this sentence
is about a gradient that has finished draining.

Nothing else was needed. No new tie, no new arm, no new accounting arm on check 6.

## Three numbers moved, and two of them arrived

**`1040 s` is 1041.** The sentence states its own formula — *"a particle's diffusion time
is its radius squared over its diffusivity"* — so the rule is that formula against four
extracted Chen2020 keys. `5.86e-6² / 3.3e-14` is **1040.594**, and a computed tie is
compared at the precision the prose commits to, through the page's rounding rule. The
figure had been truncated where the rule rounds. The positive electrode's
`5.22e-6² / 4e-15` is 6812.100 and was right as written, which is why nothing had ever
looked at the pair: one of them agreed.

This is the narrowest kind of defect this scan finds, and it is invisible to everything
that ran before the step was ledgered. No claim quoted that sentence, so there was no
literal for check 6 to scan and no trajectory for check 7 to disagree with. The number
was simply written down once and never divided again.

**`1396 s` was added.** See above: without an instant in the prose, `first` had nothing to
answer to and the three readings that hang off that moment — the terminal at 3.319 V, the
positive gap at 6.97 points, and the 96 % that is a ratio of the first — would each have
been measured at a row this file chose.

**`518 s` replaces "about halfway through it".** The old clause read:

> **5.81 for the whole remaining twelve minutes of the discharge**, moving by three
> thousandths of a point across that entire stretch — which is why the second decimal
> ticks over from 5.80 to 5.81 about halfway through it and then never moves again.

Two defects in one sentence, and they point in opposite directions. The row prints `5.80`
until 518 s, so "5.81 for the whole remaining twelve minutes" — with `5.80 at six` in the
clause before it — is false from 360 s to 518. And `about halfway through it` has two
antecedents that disagree: halfway through **the discharge** is 530 s, which the tick is
within twelve seconds of; halfway through **the stretch the clause has just named**
(360 → 1060) is 710 s, which it is not. The sentence is true on one reading and false on
the other, which is the shape `docs/plans/path-ledger-three-times.md` records as "true at
every instant a claim read and false between them" — except that here no claim read it at
all.

It now names the instant, and the instant is pinned from both sides: `5.804999` at 516 s
and `5.805005` at 518 s, six millionths of a point apart and either side of `to_fixed`'s
half-up boundary. Those two claims are the only `tighter` tolerances in the slice, and the
reason is exactly that: under the spelled rule (5e-3) an engine drift of a thousandth
would leave both green while moving the tick by minutes, and the tick is the whole of what
the pair asserts.

## The throttle, and what it costs a display claim

`Row::surface_gap`'s doc comment said, for nine slices, that `past empty` is sampled on a
250 ms wall clock "and this row is not". It is. Both are formatted from `cells`, and
`cells` is sampled on `CELLS_PERIOD_MS`. The step's own prose has said so from the reader's
side the whole time — *"these two numbers are sampled four times a second while everything
else is redrawn every frame"* — so the page and the lesson agreed and only this file
dissented.

That comment was load-bearing: it is the paragraph that licenses a `display` on a
surface-gap claim. The correction is narrower than simply withdrawing the licence, and
worth stating exactly, because a throttled row is only behind while something is
**moving**. Paused, the next sample catches up and stays. So:

* the zero-length probe and the mark are legitimate display instants — the existing probe
  claim and the two new claims at 2860 s name what the row shows;
* every mid-run reading is value-only, and says so in its note.

Sixteen of the seventeen new surface-gap claims are mid-run. Asserting a rendered string at any of
them would have been the "true and unreachable on the panel" defect this repo has shipped
three times.

## What the perturbations said

Thirty perturbations, one at a time, each against a clean committed tree, each
followed by the whole `path_claims` suite with **which** tests reddened parsed out of the
output. Predictions were written down in advance, before the first run, because a green
where red was predicted is a finding and a green rationalised afterwards is not.

Every one of the twenty-eight exited red. **Two of them were red on the wrong check**, and
that is the most useful thing the run said.

### The eighteen prose values

Each is the smallest move the printed precision allows — one in the last digit — applied to
the lesson text and nothing else.

| moved | caught by | on |
| --- | --- | --- |
| `5.28` → `5.29` (first negative reading) | literal + scan | the claim's sentence no longer matches |
| `37.18` → `37.17` (positive at the cut-off) | literal + scan | " |
| `1041` → `1040` (**the defect this slice fixed**) | scan | radius² over diffusivity, from the chemistry file |
| `6812` → `6813` (positive diffusion time) | scan | " |
| `Chen2020` → `Chen2021` | scan | the digits after `Chen` in the chemistry's provenance |
| `96 %` → `95 %` | scan | the nested ratio of two differences |
| `33 mV` → `34 mV` | scan | difference of this step's two quoted voltages |
| `1396 s` → `1398 s` (the crossing) | literal + scan | " |
| `518 s` → `520 s` (the tick-over) | literal + scan | " |
| `3.319 V` → `3.320 V` | literal + scan | " |
| `11.7 %` → `11.8 %` | literal + scan | " |
| `200×` → `100×` (speed slider) | scan | the lesson's own Speed control |
| `1S1P` → `1S2P` | scan | the scenario's `pack.parallel` |
| `15.459594 A` → `…95 A` | scan | the lesson's DemandValue control |
| "as step 15" → "as step 14" | scan | the twin lesson's position in the array |
| "steps 1 to 12" → "1 to 11" | scan | the last equivalent-circuit lesson's position |
| `0.09` → `0.10` (positive gap at the mark) | literal + scan | " |
| `3.352` → `3.353` (both sentences) | literal + scan | " |

Nothing here needed the engine: a wrong digit in a claimed sentence stops matching its
claim, and a wrong digit in a ruled sentence stops dividing. The two chemistry-derived ones
are the interesting pair, because they are the shape of the defect that was actually here —
a number written down once and never divided again.

### The two file perturbations

| moved | caught by |
| --- | --- |
| negative diffusivity `3.3e-14` → `3.4e-14` | the diffusion rule at this step **and** the engine check at step 15 |
| negative particle radius `5.86e-06` → `5.90e-06` | the same two |

Both reddened in two places at once, which is the point of running them: the rule really
reads the chemistry file rather than a constant mirrored beside it, and the same file is
under the trajectory the claims are measured against.

### The page perturbation

Deleting the negative-zero guard from the page's `gapPts` reddens
`mirrored_constants_still_match_the_page`. That guard is the threshold `gap_neg_zero_s`
uses, so the new quantity's definition is pinned to the page's own rounding and cannot
drift away from what a reader sees.

### The prose deletion

Removing the whole tick-over clause — the escape hatch of deleting a sentence to clear the
scan — reddens three ways: the claim's literal is gone, the `[ledger]` entry's numeral
count no longer matches the prose (44 against 41), and the file's own tallies stop
deriving. That route was closed by the previous slice and is still closed.

### The six deletions, and the tripwire that masked two of them

| deleted | verdict | what actually reddened |
| --- | --- | --- |
| the tick's first pin (516 s) | RED | accounting: nothing else spells `5.80` |
| the positive gap at the mark (2860 s) | RED | accounting: nothing else spells `0.09` |
| the gap reading at the crossing (1396 s) | RED | accounting: nothing else spells `0.00` |
| the terminal at the crossing (1396 s) | RED | accounting: nothing else spells `3.319` |
| `gap_neg_zero_s` | **GREEN** | self-count only |
| `soc_at:1060` | **GREEN** | self-count only |

The last two exited 101 like the other twenty-six, and on the exit code alone this slice
would have recorded six deletions all correctly caught. They were not. Deleting any claim
changes the number of claims, and the number of claims is one of the tallies
`every_count_these_files_state_about_themselves_is_derived` re-derives — so **that check
fires on every deletion case whatever the accounting did**, and it fired first. Re-run with
that one test skipped, both cases are green with 50 of 50 passing.

Both greens were predicted, and both are properties of the design rather than holes in it:

* `gap_neg_zero_s` is not what makes `1396` accountable — the two claims read *at* 1396
  already answer for it through `Accounted::ReadAt`. The crossing claim is what makes the
  word *first* mean something, and no perturbation of a **number** can reach a word.
* `soc_at:1060` is one of a pair, and its sibling at the mark spells `11.7` in the same
  literal. Check 6 asks whether *some* claim accounts for a token, so a redundant second
  reading is not required by it — the pair exists because the sentence says the figure is
  the same at both ends, which is a statement about two instants.

The lesson is the one this repo has now written down twice, and this is the first time it
cost something concrete: **a red exit code can be the wrong check reddening.** The
enumeration of failing test names is not a nicety on top of the harness — without it, two
of six deletion cases would have been recorded backwards.

### The two that were missing: does a claim's value answer to the engine?

The first twenty-eight cases moved prose, deleted claims, and edited two files — and not
one of them moved a claim's **`value`**. So they asked of the twenty-five new claims *is
this required by the scan?* and *does the prose still match?*, and never *if this number
were wrong, would the simulation say so?* The table above admits it in a sentence:
**nothing there needed the engine.** That matters here more than it usually would, because
this slice already found one new claim that was present, spelled, and scan-green while
asserting nothing about the run — the tick pin whose tolerance was ten times too loose to
reach the boundary it existed to pin.

| moved | verdict | what reddened |
| --- | --- | --- |
| one mid-run gap claim's `value`, alone (+3× its tolerance) | RED | the engine check **and** the value-against-spelling check |
| `value`, `spells`, the shared `literal`, and the page's prose, **all together** | RED | the engine check, and nothing else |

The second is the one that answers the question. Every string in the repo agreed with
itself — the lesson said `34.17`, the claim spelled `34.17`, the sentence matched — and the
only thing left that could object was the trajectory, which did. So the spelled-tolerance
claims on this step are load-bearing: their numbers are compared against a simulation, not
merely against each other.

## Two things found beside the work

**Four doc comments called this step 18.** `fmt_gap_pts`, `render_row`, `measure_row`'s
surface-gap arm and the existing probe claim's own note all name the lesson by position,
and all four were written when it was at 18. A step was inserted ahead of it and every one
of them rotted silently. This is precisely the failure `Tie::Ordinal` exists for — an
ordinal derived from the array rather than written down — and it does not reach a doc
comment. Fixed by hand, and nothing keeps them fixed.

**Every word numeral in this step is still invisible.** The module docs list four such
blind spots across two steps; this step roughly triples that on its own. *"three
thousandths of a point"* is a measurement (0.003085). *"more than six times the
negative's"* is a ratio (6.40). *"twenty-four minutes"*, *"five and a half minutes"*,
*"half an hour"*, *"an eighteen-minute discharge"*, *"four times a second"*, *"roughly
fifty simulated seconds"*, *"six gap figures"*, *"three decimals"*, *"Four footnotes"* —
and every instant in the two gap lists is spelled in words, so what the twelve claims tie
is the twelve gap figures and what puts each at 90, 180, 360, 540, 780 and 1060 seconds is
a `read_at_s` in the claims file and nothing a reader is shown.

A green ledger on this step says its **digits** are tied to something. It does not say
the step is checked, and that distinction is now the largest thing left.

## What is left

The ledger's axis is closed: twenty-four of twenty-four steps, 626 numerals, every one of
them tied to a file, a control, a claim, or a sentence's own arithmetic. `unledgered` stays
in the file as the mechanism by which a future lesson says it is not checked.

What is open, in rough order of how much it would buy:

* **The word numerals.** Dozens of them, across the whole path, every one a quantity a
  reader leans on. A scanner that read English would need a vocabulary (`WORD_NUMERALS`
  exists but is only consulted for words a claim or rule already spells) and would then
  need to tell *"half a point"* the measurement from *"half an hour"* the setting. The
  ledger's own docs have named this since it was built; it is now the whole of the gap
  rather than half of it.
* **A tally that pins a list's length rather than a sentence's number.** Recorded by the
  previous slice and still open: the prose lists of arms in both files are checked for a
  *count* beside them and never for their own completeness, and both were found short.
* **Doc comments naming a lesson by position.** Four found here, all stale, none reachable
  by any check. A test that read `//! step N` against the array is cheap and would have
  caught all four.
* **The self-count check masks every deletion perturbation on this file.** Deleting a claim
  changes the claim count, so `every_count_these_files_state_about_themselves_is_derived`
  reddens whatever the accounting did — and it sorts first. Anyone measuring whether a claim
  earns its place must run with that one test skipped, or read the failing test names, or
  the answer is meaningless. It cost this slice two of six cases recorded backwards until
  they were re-run. Making the tally derive from the file as parsed rather than from a
  written-down number would remove the trap; nothing does that today.
