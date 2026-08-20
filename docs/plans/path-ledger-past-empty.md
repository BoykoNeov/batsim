# The step past empty, ledgered — and a hole three documents kept open that was never there

`past-empty` is scanned whole: **twenty of the twenty-four steps, 442 numerals**. Forty-one
numbers, twenty-three of them inside a claimed sentence and eighteen accounted for by a
vocabulary rule. It cost **eight claims, fourteen rules, two new quantities, no new arm and
no new tie** — and four numbers moved, every one of them a defect the scan found rather than
a gap in the taxonomy.

The other result is not about this step at all: the to-do three consecutive slices recorded
as the ledger's largest structural gap does not exist, and reading the sentence it is about
was enough to settle it. That is at the end.

## Picking the step, and the column that turned out to be empty

The recorded queue carried one number — `past-empty` at 27, from an out-of-tree proxy that
runs optimistic — and four question marks. Two slices have been burned by a recorded sizing
(one was wrong by 13×; one named the most expensive step as the cheapest), so the first
action was the instrument: the ledger's own scan, temporarily switched from panic-on-first
to print-and-continue, pointed at **all five** unledgered steps in one run.

| step | numerals | unaccounted | unaccounted *inside* a claimed sentence |
|---|---|---|---|
| `past-empty` | 42 | **27** | 0 |
| `three-times-the-current` | 36 | 31 | 0 |
| `what-it-cost` | 58 | 33 | 0 |
| `same-discharge-other-chemistry` | 40 | 35 | 0 |
| `the-gradient-itself` | 42 | 38 | 0 |

**The last column is structurally zero and that is worth more than the ranking.** The plan
for this measurement was to separate "unaccounted" from "unaccounted *outside* a sentence
some claim already quotes", on the evidence of `nothing-to-clamp` — the densest step left and
the cheapest to scan, because twenty-seven of its thirty-four numerals sat inside claimed
sentences. But check 6 already *requires* an accounting for every number inside a claimed
literal, so a number in that position is never unaccounted: the two columns are the same set
by construction, and claim density cannot discount the work. `nothing-to-clamp` was cheap
because its claims were dense, and the ledger scan measures what is left after that — which
is what the first column already says.

So the ranking has to come from the *kind* of number, and on that reading `past-empty` wins
by more than the count says. It is one scenario file, one chemistry, and one arm that was
already declared, where `same-discharge-other-chemistry` needs a third scenario in the picker
and two demand variants, `three-times-the-current` needs a second scenario file and step 13's
trajectory, and `the-gradient-itself` is noted in the file as almost entirely measurements —
and a measurement needs a claim where a constant needs a rule.

The proxy agreed with the instrument this time (27 against 27). It is still a proxy, and the
last two slices found it off by 5 and by 0; one agreement is not a calibration.

## The four numbers that moved

Every one was found by driving the step's two trajectories and comparing, before any rule was
written.

**The crossing was rounded down where its own sentence's subtraction was exact.** The prose
read *"through zero at about 4226 s — eighty seconds past the knee"*. The knee is claimed at
4146.5 s two sentences earlier, so "eighty seconds past the knee" is 4226.5, and the trace
crosses zero at exactly that step. The digit beside it said 4226. This is the shape
`nothing-to-clamp` had in *"73 seconds of no flags"* against a subtraction of 73.5, in the
other direction. The hedge came out with the number: a crossing on a half-second grid is not
an approximation.

**A reading was anchored to a frame nothing renders.** *"4.01 at 166 s past the knee"* is
true — 4312.5 − 4146.5 = 166 — and unaccountable, because this file reads an instant as
absolute simulation time before the mark and as a duration since the mark on a leg, and there
is no third frame. The sentence now names the clock: *"4.01 at 4312.5 s"*, which is the same
instant in the frame the panel itself is in, and the claim beside it is read there.

**A hedge stood in for a tolerance.** *"it only starts climbing at about 170 seconds"* — the
debt re-enters the collapse ramp at 4571.0 s, which is 171 s after the mark, not 170. "About"
is not a tolerance in this file; the derived rule for `170` is half a second either way, and
171 is outside it. The sentence moved to the number the run produces.

**And a sentence showed one arithmetic while printing a different number.** It read

> And 254 s at 2 A is 0.1410 A·h, which is what came out below empty.

254 × 2 / 3600 is 0.1411. The 0.1410 is the *other* quantity in that sentence — the debt at
the mark, 6.121 points of a 2.303451 A·h cell — and the two are one in the last place apart
because the clock is on a half-second grid while the debt really clears 253.79 s in. The
sentence used to hide that gap by printing the second number under the first one's
arithmetic. It now prints both and names the reason:

> And 254 s at 2 A is 0.1411 A·h, against the 0.1410 A·h that came out below empty — the same
> charge to within the half-second step the debt clears on.

This is the same family as the last slice's rewritten sentence, where the arithmetic shown to
a reader landed 12 mV from the step's own headline. **A sentence that shows its working is
worth more than one that states a result, and it is also the only kind that can be caught
being wrong.**

One number left the page: *"a real reversed cell goes on to something like −1 to −2 V while
its copper current collector dissolves"* is a fact about the world, and the only place in
this tree that decides it is the chemistry file's **provenance comment** — prose, not a
parsed value. It now reads "a volt or two negative", on `leg-that-is-not-there`'s precedent
for the voltage real LFP cells are charged to.

## What it took

**Two new quantities**, both instants the engine reports on the step grid and neither
expressible as a threshold an author picks:

* `v_floor_s` — the first instant the trace reaches the floor it never leaves. Defined off
  the run's own minimum rather than off `[reversal].floor_v`, for the reason `t_at_v_below`
  gives about taking a threshold from a file: the declared floor is 0 V of *open-circuit*
  voltage and the terminal sits at −0.064 V, so a quantity reading the field would be
  asserting the wrong crossing. Two fences: the row may not be the last one, and the voltage
  must still be within a microvolt of it at the end — which is what makes "the fall stopped"
  different from "the fall paused here".
* `deficit_falls_below_pts:{p}` — when the debt comes back down through a stated depth,
  searched **after the run's peak**. A charge leg carries every row before the mark, so the
  debt crosses every depth twice, and a first-match search would have answered with the
  outward crossing and looked right.

**Fourteen rules.** Thirteen of the eighteen rule-side numbers are constants: the demand box
in three sentences and the charge box the reader retypes in two more, the two ends of the
charge state's own interval and its low end a third time, the cell's format inside the
chemistry file's name, the two `[reversal]` fields — the first any ledgered step reads — the
width of the collapse ramp stated twice, and an ordinal pointing at step 1. The other five
are worked out from this step's own claims in sentences no claim quotes: the 83 seconds the
collapse takes, the 254 the debt takes to clear, the two amp-hour totals, and the voltage the
engine used to hold forever.

Three of those deserve naming.

* **The interval is read exactly, and the obvious tie would have been generous.** *"a charge
  state confined to `[0, 1]`"* ties to the chemistry's charge column: the low end is that
  column's first node and the high end is its *span*. `Tie::Member` — "any node of this
  table" — was the natural reach and would have accounted either token against 0.4 as
  happily as against 0 or 1. Measured: retyping the prose to `[0, 0.4]` reddens the ledger by
  name.
* **`83` is a difference of two of this step's own claims**, one of which exists only to be
  subtracted from. Nothing in any file decides how long the collapse takes, and the lesson
  block's own comment sizes the playback speed off the same 83 s — so a trajectory that moved
  would leave the comment stale beside the prose.
* **`1.93` is the engine's old behaviour, quoted from this step's knee.** Before the deficit
  existed, an empty cell held the voltage it emptied at forever; the sentence that says so
  prints the knee voltage rounded to two places, so the tie quotes the knee claim rather than
  re-measuring a run that cannot exist any more.

**No new arm, and no new tie.** The step's second trajectory — the reader retyping the demand
box to −2 — was declared as a `charge leg` arm when the step's first claims were written, and
three of this slice's numbers read a control off it through `Tie::OnArm` and `Tie::Magnitude`.
That makes this the second user of `Magnitude`, whose docs called it the only one.

## The perturbation table, enumerated rather than exit-coded

Each row is one edit, the whole `path_claims` binary run, and the test names that go red read
off the output — never the exit code, which this project has twice found reddening for the
wrong reason.

| # | the edit | red |
|---|---|---|
| P1 | the 2.01 deficit recorded as 2.03 | `every_claim_states_the_value_it_measures`, `every_claim_matches_the_engine` |
| P2 | prose instant moved off its claim: 4312.5 → 4313.0 | `every_claim_appears_in_its_own_step`, **the ledger** |
| P3 | prose restates the chemistry: `v_per_soc = 100` → `90` | **the ledger** |
| P4 | the whole `v_floor_s` claim block deleted | `every_count_these_files_state_about_themselves_is_derived`, **the ledger**, by name on `83` |
| P5 | the quoted knee voltage retyped: 1.93 → 1.94 | **the ledger** |
| P6 | the debt's amp-hours retyped: 0.1410 → 0.1411 | **the ledger** |
| P7 | the charge-state interval retyped: `[0, 1]` → `[0, 0.4]` | **the ledger** |
| P8 | the exactly-zero charge state nudged to 0.04 points | `every_claim_matches_the_engine` |
| P9 | the 171 s rebound instant recorded a step early | `every_claim_matches_the_engine` |
| P10 | the demand-box phrase shortened to the version step 21 also prints | **nothing** |

Two of these are worth more than their row.

**P4 had to be a deletion of the whole block.** The first attempt changed the claim's
`quantity` from `v_floor_s` to `v_at` and reddened three tests — but two of them were the
tolerance rule and the value check complaining about the substitution rather than about the
`83` losing its operand. A partial perturbation reddens for the wrong reason; this project
has recorded that before, and it recurred here inside one slice.

**P8 is what `tol_from = "tighter"` buys.** The sentence says the charge state reads `0.0 %`
and the next one says *"Not nearly zero — exactly zero"*. Half a unit in the last place of
`0.0` is 0.05 points, so the spelled rule would have let the engine drift by a twentieth of a
point under a green claim. At 1e-12 it does not, and nudging the recorded value to 0.0004 —
comfortably inside what the sentence's own precision would license — reddens.

**P10 is green by design and is the result.** The rule for *"put the demand box to **−2**"*
carries "which charges at the same rate" so that it cannot reach step 21, which tells a reader
to type the same −2 into the same box. Shortening it reddens nothing, because step 21 is not
ledgered and the scan never reads it. Measured rather than assumed: with the short phrase, the
rule-reach sweep resolves to `["past-empty", "what-it-cost"]`. It is written the correct way
round rather than the reachable way round, and it starts doing work the day the next step is
ledgered.

## The three fences nothing in the claims file can reach

The two new quantities carry three refusals, and not one of them is reachable by editing
`path-claims.toml`: the claim that reads `v_floor_s` sits on a run with 340 rows of flat trace
after the floor, and the claim that reads `deficit_falls_below_pts` names the arm that repays
the debt. A fence no perturbation can reach is the "pinned and consulted by nothing" shape
this file rejects everywhere else, and `docs/plans/path-ledger-sixth-step.md` already paid for
the lesson once, so each gets a `should_panic` test built from a real run rather than a
paragraph:

* `a_floor_refuses_a_run_that_ends_on_it` — the step's own run truncated at the floor row. It
  would answer 4229.5 s exactly as the real one does, with no evidence at all that the fall
  had stopped rather than paused.
* `a_floor_refuses_a_minimum_the_run_climbs_out_of` — the charge leg, unmodified. It reaches
  the same −0.064 V and then climbs to 2.07 V, so its minimum is not a floor.
* `a_repayment_refuses_a_run_that_never_repays` — the step's own run again, where the debt
  peaks at the mark and the run ends. Only the leg repays it, which is why the claim names an
  arm, and this is what stops one that forgot to.

Each `expected` fragment belongs to the fence and to nothing else in the file — the trap
`docs/plans/path-instant-tag.md` recorded is a `should_panic` satisfied by a phrase the test
itself supplies, such as a lookup's own `expect`.

## The rule-reach sweep

All rules matched against all twenty-four lessons before this was committed. The count of
rules reaching a step they were not written for stays at **three** — step 3's two scatter
rules, which reach `what-protection-costs` and are right there for the right reason, and
`**Step {n}**`, which reaches `what-it-cost` by identity. **None of the fourteen new rules
reaches a second step.**

## The hole that was not there

Three consecutive documents recorded the same open gap. `path-ledger-one-step-that-got-through.md`
opened it:

> `what-protection-costs` is the eleventh lesson and the last step before this one where
> protection derates a demand. The sentence rewritten above now depends on that being true,
> by inspection rather than by assertion.

`path-ledger-what-protection-costs.md` repeated it, and `path-ledger-leg-that-is-not-there.md`
closed with *"the hole the last slice opened is still open"* and named it as something no
numeral scan could ever reach — which is true, and was the argument for building a check over
the lesson list instead.

**Reading step 18's page settles it in the other direction.** The paragraph is

> A derate clamps a demand; a charge inhibit stops one. This short is neither, because there
> is no demand at all — and that is what the contactor is for.

and the step's `expect` mentions no other lesson's protection at all. The sentence *defines*
both mechanisms and says this fault is neither. It states nothing about any other step, so no
other step can falsify it. That is exactly the property the slice that wrote it claimed for
it, two paragraphs above the note: *"It carries no count, claims nothing about any other
lesson."* The residual dependency the note recorded was a property of the draft that had just
been deleted — *"Every step of protection so far has derated a demand"* — and it was written
down about the replacement by mistake.

The lesson is one this file has recorded in the other direction and is worth having in both:
**a note about why something is still open outlives the thing that closed it, and the ledger's
own documents are where such notes are read as fact.** `Tie::Name`'s docs carried "reworded
away instead" for four slices after the reword stopped being the answer; a doc note saying an
arm was impossible had outlived its reason; the cheapest-next-step recommendation was the most
expensive. Here the cost was three slices carrying a to-do nothing needed, and this slice's
budget nearly spent building a check for a sentence that did not want one.

A correction block is appended to the document that opened it, so the next reader of that page
gets the answer rather than the to-do.

## Where the ledger stands

Twenty of twenty-four steps scanned whole, 442 numerals, 175 vocabulary rules,
twenty-three arms. **Four steps left**, every one of them already carrying claims.

The queue, re-measured with the instrument this slice rather than with the proxy:

| step | unaccounted | of | what it needs beyond claims |
|---|---|---|---|
| `three-times-the-current` | 31 | 36 | a **second scenario file** (`pulse_train_ecm`) and step 13's trajectory — the shape `Tie::Elsewhere` exists for, unused since step 23 |
| `what-it-cost` | 33 | 58 | a charge leg and a control arm; heavy on `Tie::Derived`, since much of it is arithmetic over its own siblings |
| `same-discharge-other-chemistry` | 35 | 40 | a **third scenario** the reader loads from the picker, plus two demand variants |
| `the-gradient-itself` | 38 | 42 | almost all measurements: claims, not rules |

Two things known about that queue rather than guessed:

* `what-it-cost` is step 21, is the only unledgered step a rule already reaches, and is the
  step this one hands off to — its opening sentence is about `past-empty`'s closing one. Its
  prose quotes this step's floor twice and takes it apart, and it retypes the same `-2` into
  the same box — so the claims this slice pinned are what its quotations will address, and the
  demand-box rule above is written long enough to stay off it.
* The count beside a ledger entry is derived and checked, and it is measured **after** the
  prose edits, not before: this step went in at 42 numerals and closes at 41, because a pair
  of volts left the page and an amp-hour total arrived. The scan run that sizes a step is not
  the run that sets its entry.
