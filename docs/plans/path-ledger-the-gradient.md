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

PERTURBATION-TABLE-PLACEHOLDER

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
