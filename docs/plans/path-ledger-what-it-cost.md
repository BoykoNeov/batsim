# The step with a control arm, ledgered

`what-it-cost` is scanned whole: **twenty-one of the twenty-four steps, 500 numerals**.
Fifty-eight of them are this step's, and thirty-three had nothing accounting for them when
the scan was pointed here. Four became claims, twenty-nine are accounted by rule, one left
the page, and one arrived — a ratio a sentence had been leaving to the reader to work out
wrongly.

It cost **one new kind of arm**, four claims, nineteen vocabulary rules, one new word
numeral, and no new tie — the taxonomy's twenty-three arms are untouched. It also cost an
hour to a scratch harness that captured its baseline from a tree an earlier scratch harness
had left half-edited, which is the most transferable thing in this document and is written
up at the end.

## Why this step, and what was different about it

It was the step the previous slice handed off to
(`docs/plans/path-ledger-past-empty.md`), on two grounds that both held: it is step 20's
immediate neighbour and quotes step 20's closing numbers, and it was the second cheapest of
the four that were left. The recorded sizing — 33 unaccounted of 58 — reproduced exactly on
the ledger's own scan.

What made it unlike the twenty before it is one sentence:

> Measured rather than assumed: the identical run with the over-discharge coefficient set to
> zero and nothing else touched ends the same ten minutes at **99.96 %**.

Every arm in this file so far changes how a pack is **driven** — a demand box, a checkbox, a
slider, a step length, a button. This one changes how a pack is **built**. It is the control
arm of an experiment, and `docs/plans/reversal-damage-ui.md` measured it out of tree when the
step was written. Until this slice, the step's headline — *of the 4.82 points lost after the
knee, 4.80 are the reversal* — rested on two numbers nothing in the repo could check.

## `Arm::fade_per_ah`: an arm nobody can walk

The new field overwrites `[reversal] fade_per_ah` in the **parsed** chemistry between
`parse_chemistry` and `Pack::new`. The shipped TOML is untouched, so no ordering between
tests can leak it.

The design question was not whether it could be built — it is a `pub` field on a value the
harness already owns — but whether an arm is allowed to be unreachable from the page at all.
The answer taken here, and written into the field's own docs:

* **`instruction` is read as the sentence that REPORTS the change, not one that asks for it.**
  The substring check is unchanged: the sentence still has to be in this step's prose, so a
  reword still reddens. What changes is what a reader is being told — what was measured,
  rather than what to go and measure.
* **Such an arm may only assert what the prose claims about a counterfactual.** It is not a
  licence to run any trajectory under a sentence that happens to mention it.

Four fences keep it that way, all in `every_arm_is_instructed_by_its_own_step`:

| fence | what it refuses | perturbed |
|---|---|---|
| the value is spelled in the instruction | a counterfactual whose value is not in the prose — the worst case of all, because nothing a reader sees would move if it changed | yes, via the "fade = the file's own" row |
| it differs from the chemistry's own | the step's own run wearing a second name, with the subtraction quietly coming to zero | **red** |
| it implies `Start::Restart` | a pack that reached the mark under one coefficient and continued under another, which is not a trajectory | **red** |
| it may not combine with `pack_from` | two exotic overrides composed, two removes from the step the reader is on | not reachable; stated |

The first fence needed a word. `WORD_NUMERALS` had three entries and two readers — a claim's
`spells`, and a derivation's `Operand::Word`. It has four and three now: the sentence writes
its value as **"zero"**, and `contains_number` already bounds a word match the way it bounds a
digit one. `every_word_numeral_is_read_by_something` grew the third reader rather than
letting the entry sit there looking like coverage.

## What the four claims are

| claim | quantity | why it could not be a rule |
|---|---|---|
| `through zero at 287.5 s` | `t_at_v_below:0` | the terminal's own crossing, the twin of step 20's at 4226.5 s |
| `soh res` up from `1.0004` | `soh_res_at:207.5` | a reading, and unambiguous by a wide margin — 250 s already shows 1.0082 |
| the two traces leave `100` | `soh_res_at:0.5` | the plot's OTHER origin; `soh cap` leaving 100.00 % was claimed twice already and nothing said where the resistance trace starts |
| ends the same ten minutes at `99.96 %` | `soh_cap_at:600` on the control arm | the counterfactual |

The control claim carries **no `display`**, and that is not an omission: this trajectory is
not on the page, so there is no row rendering it and a `shows` would be asserting a string
nobody is shown. Every other claim on this step quotes a row because a reader is told to go
and read it; this one is quoted by the prose instead.

## The arithmetic, and the trap in it

Fifteen of the twenty-nine rule-accounted numerals are arithmetic the sentences do in front
of the reader. Three of them are worth stating, and a fourth is what no check could see.

**The `1.5` is the trap.** The sentence says the sag divides into *"3.2 of it from the instant
resistance and 1.5 from the slow one"*, in millivolts. `aging.r_growth_per_capacity_loss` is
also **1.5**. A rule pointed at that constant would have been green on a number that means
something else entirely — the same "right off the wrong field" shape
`docs/plans/path-setting-arm.md` records. What the number actually is: 2 A across the RC pair
grown by 7.26 %, which is 1.4529 mV and prints 1.5 by rounding. The coupling constant is a
*factor* of it.

**`4.6` is a product of what is printed, not a sum of what follows.** The sentence writes
`2 A × 0.032 Ω × 0.0726 = 4.6 mV` and then splits it into 3.2 and 1.5. Those halves sum to
**4.7**. A `Tie::Sum` over them would have reddened a sentence whose arithmetic is correct, so
the tie is `Tie::Derived` over the sentence's own three factors — 4.6464 mV, which prints 4.6,
as does the unrounded 4.6494.

**`0.2182 A·h` is not any endpoint product.** The obvious reading — the debt at the mark
valued at the cell's capacity, which is how step 20 ties its own `0.1410` — gives **0.2127**.
The engine bills each step's charge past empty against the capacity the cell had *on that
step*, and the cell shrank 4.8 % while it was down there, so no endpoint product recovers the
integral. What does is the damage itself: the capacity the reversal cost, divided by the cost
per amp-hour —

```
(99.956915 % − 95.156970 %) / 0.22  =  0.21818 A·h
```

— which ties the sentence to the control arm as well, and makes *"even the arithmetic is
exact"* exact about a quantity the engine never prints.

## The number the ledger was structurally blind to

The step's closing arithmetic said:

> it took **0.2128 A·h** to put back what **0.2182 A·h** took out, less in than came out,
> because the cell being refilled is **4.8 %** smaller than the one that was emptied.

Every numeral there is tied and every tie agrees. The gap between the two amp-hour figures
is **2.5 %**. The sentence offers 4.8 as its reason, and `0.2182 × 0.952` is `0.2077`, not
`0.2128`.

Both numbers are true. `4.8 %` is exactly how much smaller the cell is at the mark. What is
wrong is the "because": **the shrink accrued while the charge was still coming out**, so the
charge-out integral was billed against an average health well above the final one, and only
about half the shrink was ever in place to discount anything. Measured: the gap is 0.511 of
the shrink, and the cell at the knee was 5.06 % larger than the cell at the mark.

The ledger cannot see this and never could. It accounts each numeral against a source and
never reads the causal link *between* numerals in one sentence — the same structural
blindness `docs/plans/path-ledger-leg-that-is-not-there.md` recorded when a sentence's own
arithmetic named `R0` alone and the RC pair was a third of the drop. Two slices, two
sentences, one shape: **prose that hands a reader a multiplication which does not close, with
every number in it individually correct.**

Repaired by printing the ratio and saying what the 4.8 is:

> it took **0.2128 A·h** to put back what **0.2182 A·h** took out — **97.5 %** of it —
> because the cell being refilled is smaller than the one that was emptied. Not by the whole
> **4.8 %** it is down at the mark, though: that shrink accrued while the charge was coming
> out, so on average only about half of it was ever in place to discount anything.

`97.5` is a `Tie::Derived` ratio of the sentence's own two amp-hour figures, so the step now
prints the number a reader would otherwise have got wrong, and the ledger holds it to the two
figures beside it. It is the one numeral in this step that was *added* rather than accounted,
and it is why the step went in at 58, closed at 57, and ends at 58 again.

## `pow10` belongs to the rule, and one rule had to be two

The sag sentence was first written as one rule carrying four ties: the three factors and
their product. It failed, and the failure is the useful kind — `pow10: 3` turns volts into
millivolts and is applied to **every tie the rule carries**, so the demand box came out as
2000 A. Two rules now: the factors at `pow10: 0`, the answer at `pow10: 3`. Step 20's own
vocabulary records the same rule from the other side; this is the first time it has actually
bitten.

## One number left the page

> Measured on this page: the moment the run stopped at its mark that row read **9.438**
> points, and the next sample after the pause read 9.704 …

`9.438` is what one browser run happened to catch on a row throttled to a quarter-second of
*wall* clock. It is not the throttle bound and it is not a fact about the simulation —
`reversal-damage-ui.md` says as much in parentheses, that the driver's own gap was longer than
the throttle. Nothing in the tree decides it, and nothing could. It is the same class as the
stop instant that left `what-protection-costs`, and it is now *"that row was still behind"*.
The lesson the sentence exists for — the row lags, and catches up on a pause — is untouched,
and the `9.704` beside it is the step's own claimed reading, quoted.

## The perturbation table

Seventeen cases, each editing one thing, run against a verified-clean baseline. Every row
behaved as declared.

| perturbation | expected | got | reddened |
|---|---|---|---|
| control arm deleted | red | red | the claim's arm lookup, and two `should_panic` fences |
| control arm's `fade_per_ah` = the file's own 0.22 | red | red | `every_arm_is_instructed_by_its_own_step` + the engine |
| control arm continues from the mark | red | red | the `Start::Restart` fence |
| control's mark reading moved by 0.01 point | red | red | engine, value, **and the ledger** — `4.80` and `0.2182` move with it |
| `r_growth_per_capacity_loss` 1.5 → 1.6 | red | red | engine + ledger |
| the RC pair's `r_ohms` 0.010 → 0.011 | red | red | engine + ledger |
| `reversal.fade_per_ah` 0.22 → 0.25 | red | red | engine + ledger |
| `[r0]` at the bottom at 25 °C, 0.022 → 0.023 | red | red | engine + ledger |
| prose `3.2` → `3.3` mV | red | red | ledger |
| prose `4.80` → `4.81` points | red | red | ledger |
| prose `0.2182` → `0.2181` A·h | red | red | ledger |
| prose `5 %` → `6 %` charge | red | red | ledger |
| prose `287.5` → `288.0` s | red | red | literal + ledger |
| prose `1.0004` → `1.0005` | red | red | literal + ledger |
| prose `100` → `90` on the plot | red | red | literal + ledger |
| a NEW unaccounted number added to the prose | red | red | the ledger itself, by name — verified, not inferred from the exit code |
| step 20's demand box read as this step's | **green** | green | nothing — see below |

The last row was checked rather than trusted. `every_count_beside_a_ledger_entry_is_derived`
also reddens on the sixteenth case, and a count moving is not the ledger refusing anything —
so that run's output was read directly and the ledger's own panic confirmed: *"step
`what-it-cost` prints `1000` and nothing accounts for it"*.

### The green row, isolated

The sentence *"The last step's −0.0640 V is `2 A × 0.032 Ω`"* is about **step 20's** demand
box. Step 21's box is also 2 A, so swapping `Tie::Elsewhere` for this step's own
`Tie::Setting` changes nothing and the suite stays green. That is a blind spot of the
*whole-suite* perturbation, not of the tie, and the difference is worth more than the
registration:

| | verdict |
|---|---|
| right tie, step 20 untouched | green |
| right tie, step 20's box 2 A → 3 A | **red** — *"the lesson's DemandValue control, read on the lesson `past-empty`, says [3.0]"* |
| wrong tie, step 20's box 2 A → 3 A | **green** |
| wrong tie, step 20 untouched | green |

Run with step 21 alone in `[ledger].steps`, so no other step's red could be mistaken for this
one's. The two ties *are* distinguishable and the rule really does read the step it names.
**A confounded perturbation is not the same as an unfalsifiable claim** — the confound was
that both boxes carry the same number, and moving one of them removes it.

## The hazard that cost the most, and it was not in the repo

The isolation experiment above ran twice. The first run crashed on a `UnicodeEncodeError`
*after* it had edited `[ledger].steps` down to one entry and *before* it restored. The second
run then captured that half-edited tree as its own baseline, "restored" to it faithfully, and
left the ledger scanning one step out of twenty-one. Nothing complained, because the only test
that run invoked was the scan itself — which passes on one step as readily as on twenty-one.
It surfaced two commands later, in the full workspace run, as four unrelated-looking failures.

**A scratch harness that snapshots the working tree at import time is trusting that whatever
ran before it cleaned up.** The fix is not "restore in a `finally`" — that helps, and it is
also what the crashed run skipped. The fix is a *fingerprint*: `perturb21.py` now asserts, at
import, that the ledger's steps list has twenty-one entries and step 20's demand box is 2 A,
so a dirty baseline fails loudly instead of being measured against. The perturbation table
above was re-run from scratch under that guard, which is why it is quoted rather than the
first run's identical numbers.

The related lesson already in this project's memory is about *another session's* harness
mutating the tree. This one was mine, one command earlier, and the tree looked clean by every
test I happened to run.

## Where the ledger stands

Twenty-one of twenty-four steps scanned whole, 500 numerals, 194 vocabulary rules, and the
same twenty-three arms in the taxonomy. Twenty `[[arm]]` blocks, one of which is a
counterfactual. **Three steps left.**

| step | unaccounted | of | what it needs beyond claims |
|---|---|---|---|
| `three-times-the-current` | 31 | 36 | a **second scenario file** (`pulse_train_ecm`) and step 13's trajectory — the shape `Tie::Elsewhere` exists for |
| `same-discharge-other-chemistry` | 35 | 40 | a **third scenario** the reader loads from the picker, plus two demand variants |
| `the-gradient-itself` | 38 | 42 | almost all measurements: claims, not rules |

Those counts are the previous slice's and were **not** re-measured here. That is a deliberate
statement rather than an omission: this slice changed nothing in those three steps' prose, but
it did add nineteen rules, and a rule's phrase can in principle match a sentence in a step it
was not written for. The next slice should point the scan at its own target and read the
number, exactly as this one did — the recorded sizing was right twice running, and it has been
wrong before by 13×.

## Deferred, with a price

* **The control arm asserts one reading.** Its claim's note records that the two arms agree to
  the last decimal up to the knee — 0.99975252 on both at 207.5 s — because `[reversal]` is
  the only thing between them and nothing has gone past empty yet. That identity is a check on
  the instrument as much as on the engine, and it is **prose, not an assertion**. A second
  claim on the control arm at the knee would make it one, at the cost of a claim that says the
  same thing as its neighbour.
* **The counterfactual is one coefficient.** `Arm::fade_per_ah` names the field it overrides
  rather than taking a path-and-value pair, because a general chemistry-override mechanism
  with one user would be a mechanism designed for a case nobody has. The next control arm that
  wants a different field is the moment to generalise it, and not before.
* **Nothing checks that an unwalkable arm stays rare.** There is no fence saying "at most one
  arm may be a counterfactual", and there should not be a numeric one — but the field's docs
  are the only thing today arguing that a chemistry override is a last resort rather than a
  convenient way to reach a trajectory. That argument lives in prose and is exactly the kind
  this file usually turns into a check.
* **The three remaining steps are the ledger's last, and two of them need scenarios the
  harness has never loaded on an arm.** `pack_from` walks to another lesson; neither of those
  two steps' sentences is a walk to a lesson, it is a load from the picker. Whether that is the
  same thing is the next slice's first question.
