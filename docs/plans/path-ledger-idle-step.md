# The step where nothing happens, ledgered — and a number that was true of a run nobody makes

`wearing-out-while-idle` is step 8: a pack at rest, aging on, nothing connected to it. The
measurement in `docs/plans/path-self-description-sweep.md` found it the cheapest of the
fourteen unledgered steps — eight unaccounted numerals out of sixteen — and it is now the
eleventh step scanned whole.

It cost one new tie, two new controls, one new arm, two new claims, three instant tags and one
sentence added to the page. The sentence is the interesting part.

## The 3.5× was right, unreachable, and nearly deleted

> Not the 3.5× two fresh packs would show at those temperatures

Two fresh packs is not a trajectory this step produced. Its prose instructed one control
change — drag the slider to 45 °C at the mark and press Run — and the arm behind that is a
*continuation*: the pack that produces it has already spent 200 000 s at 25 °C. Nothing in
the path built a pack that was fresh **and** hot, so the number was true of a run the reader
is never shown. That is the "right but unreachable" class this repo has shipped twice.

The measurement is worth recording because three plausible numbers sit within 0.02 of each
other and only one of them is what the sentence says:

| quantity | value |
| --- | --- |
| two fresh packs over **one 200 000 s leg** — what the page now shows | 3.7489 / 1.0579 = **3.5439** |
| the pure Arrhenius factor for +20 K on this chemistry's activation energy | 3.5535 |
| the same comparison over a **600 ks** run, which `calendar_fade_hot.toml`'s header states | 6.51 / 1.83 = 3.557 |

To one decimal that is 3.5, 3.6 and 3.6. The prose says 3.5 and the scenario file says 3.6,
and **both are right about different runs** — the difference is the warm-up. Both packs start
at 25 °C, calendar fade integrates as `√t` so the first minutes carry disproportionate weight,
and the hot pack spends its first few hundred seconds climbing to its own Arrhenius rate. Over
a longer run that dilution shrinks. A slice that "fixed" the prose to agree with the scenario
header would have broken a true number.

So the repair is an instruction rather than a deletion. The page now says:

> press **Restart** with the slider still at 45 °C and the same 200 000 s costs **3.75
> points** from new

which is one keystroke from where the reader already is, and turns the comparison into two
trajectories this suite runs.

## `Tie::OnArm`: the first tie that reads a control off an arm

Step 8 prints the ambient slider twice and the step's own value is neither: 25 °C is where the
slider starts, 45 °C is what both instructions dial it to. `Tie::Setting(Ambient)` answers 25
whatever the sentence says.

The wrong fix was available and worse than nothing: point a rule at the scenario's
`initial_temp_k` and it finds 298.15, which is not this number — and at some other lesson one
day it would be.

So the new tie is a **wrapper**, on `Tie::Elsewhere`'s terms exactly: that one changes which
*lesson* answers a question and leaves the question alone; this one changes which
*trajectory's controls* answer it. Two rules use it, one per arm, which is the distinction it
exists to make — the same control, dialled to the same 45, on a run that continues and a run
that restarts.

Two fences, neither reachable from the claims file because a rule is code, so each has a
`should_panic` test: it may only wrap a `Setting`, and the arm must actually **override** that
control. The second is the one that matters — a silent fallback to the step's own value would
have accounted the sentence's 45 against the slider's 25 and gone green.

## Two new controls, and one of them changes what another paragraph may claim

`Control::Until` is the mark (`This runs to 200 000 s of simulation`) and `Control::Speed` is
the speed slider (`twenty seconds of watching at 10 000×`). The second required `Lesson` to
scrape `speed_x` for the first time, and that has a consequence recorded in the file rather
than left to be discovered:

`Accounted::Setting` — check 6's arm for a control a reader dials in — argued at length that
the *generous* version of itself, accounting a token against any numeric field of the lesson
block, "cannot be built, or perturbed into existence, without adding the field first", because
step 18's block carries `speed_x: 10` beside a sentence whose `10` is a step length. That field
now exists, so **that paragraph became false in the commit that ledgered this step**, and it
took a second pass to notice: the new field's own doc recorded the consequence, and the
paragraph the consequence was about was left standing. Both say it now. The argument itself
holds — check 6's arm ties a token to the step length of a **trajectory** a claim is read on,
and no trajectory has a speed — but it rests on the design rather than on an absence, and a
generous version is now buildable and would be a rewrite rather than a slip.

Worth naming as its own lesson, because this is a file whose subject is stale prose: **writing
down the consequence of a change is not the same as fixing the sentence the consequence is
about.** The note on `Lesson::speed_x` and the paragraph in `Accounted::Setting` are two ends of
one fact, and only one end moved.

## A recorded gap that cannot be closed

`run` keeps two environments split at the mark, and on a *restart* arm the pre-mark one is
reached only by the zero-length probe. `docs/plans/path-derived-arm.md` recorded that branch as
unobservable "today", with the way to close it written down: "the claim that would reach it is a
`probe = true` reading on one of them."

That is false, and this slice measured it: probed at 25 °C and at 45 °C on this step's own
fresh pack, **the telemetry and the snapshot are byte-identical**. A zero-length step cannot
see an environment at all — nothing in `Env` reaches telemetry except through a `dt` that is
zero. So the branch is dead to every claim that could ever be written, not merely to the ones
written so far, and the comment now says that instead.

**A recorded plan to close a gap is a claim about the code, and it can be wrong in the
direction that looks like diligence.** The note read as a to-do; what it actually described was
impossible, and it had been sitting there since the ambient split was built.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| the arm's `ambient_c` 45 → 25 | `every_arm_is_instructed_by_its_own_step` (an override that changes nothing), before any value moves |
| the arm's `start` `restart` → `mark` | the 3.75 claim's value — a continuation from the mark is the aged pack, not a fresh one |
| the arm's `to_s` 200 000 → 100 000 | the 3.75 claim's reachability |
| the `hot from new` rule's arm → `hot` | the ledger, on the 45 in "with the slider still at 45 °C"? **No** — both arms drag to 45, so this must stay GREEN, and that is the honest limit of the two-rule split |
| `Control::Ambient` on the second rule → `Control::Until` | the new `does not override it` panic |
| prose `10 000×` → `1 000×` | the ledger, on the speed setting |
| prose `runs to 200 000 s` → `runs to 100 000 s` | the ledger, on the mark |
| prose `4S2P` → `4S3P` | the ledger, on `pack.parallel` |
| claim `soh_cap_at:200000` on the arm → `soh_cap_at` (tag dropped) | the quotation of a quantity two claims answer differently — the fence the tags exist for |
| the 3.75 claim's `value` 0.9625106 → 0.9634 | the value check, and the ratio the ledger reads through it |
| delete the `100 %` claim | the ledger, on a numeral no rule spells |

Every row came back as registered, and three came back reddening on **more** than they were
registered for, each for a reason worth stating:

* **The arm's ambient 45 → 25** fails the arm check on the assert *before* the one predicted —
  `25` is not a number in the instruction sentence — and then also fails the ledger and the
  engine. The prose, the arm and the claim are three copies of one fact, and moving the arm's
  copy alone contradicts all three.
* **`restart` → `mark`** fails reachability as well as the value, because a claim at 200 000 s
  on a continuation arm is reading the mark, which is the instant every other claim on the step
  already reads. The two failures say the same thing twice: a continuation is not a fresh pack.
* **Deleting the `100 %` claim** fails the self-count check beside the ledger, because the
  claim counts are derived now. That is the sweep of the previous commit paying for itself one
  slice later.

And the row registered as **green stayed green**: pointing the restart arm's rule at the
continuation arm changes nothing, because both arms drag the slider to the same 45. The
two-rule split is about which *sentence* is which, and it cannot tell the arms apart on this
number. Written down rather than discovered later — the same limit `Arm::pack_from` records for
two lessons that agree on their timestep.

## Three things a review caught that the suite could not

Registered and run after the slice was pushed, as its own commit.

| what | outcome |
| --- | --- |
| the stale `Accounted::Setting` paragraph, reverted | **green**, as it must be — no check reads a sentence about the code, which is why this sweep is manual and why it will be needed again |
| the second line's claim (`soh_res_at:0.5`) moved 1.0 → 2.0 | reddens the value and stated checks |
| `Tie::OnArm`'s third panic, asked directly | its own `should_panic` test — no perturbation reaches it, because renaming the arm breaks the claim that reads it first |

The second is the substantive one. "Two lines leaving 100 % together" carried **one** claim,
on the capacity trace, and the resistance trace could have started anywhere with the suite
green. Both series are drawn as percentages of new (`soh_capacity * 100` and
`soh_resistance * 100` on one axis — the page's own comment beside `plot-soh` says the same
sentence the prose does), so the second claim is what makes the sentence's *subject* checked
rather than half of it. It is also the one place the plot and the readouts part company: `soh
res` prints `1.0000 ×` where the plot prints 100 %, which is why that claim's `shows` is not a
percentage.

**A sentence about two things needs two claims even when it prints one number.** Check 6 is
satisfied by one accounting per numeral, so a plural subject can hide behind a singular figure.

## Learned while building

**A number can be unreachable and still be the only right one.** Everything about the 3.5×
looked like a defect: the scenario file's own header says 3.6 for what reads as the same
comparison, the pure Arrhenius factor rounds to 3.6, and no trajectory in the path produced
either. The temptation was to "correct" the page to 3.6 and move on, which would have replaced
a true unreachable number with a false reachable one. What separated them was measuring the
run the sentence actually describes — one leg, from new, at each temperature — and it agrees
with the prose to four digits. **Unreachability is a defect in the instructions, not evidence
about the number.**

**A recorded to-do can be a false claim about the code.** `docs/plans/path-derived-arm.md`
wrote down which claim would close the one unobserved branch in `run`. No claim can: a
zero-length probe cannot see an environment, so the branch is dead by construction. The note
read like diligence and was doing the opposite — carrying an item that would never be
actionable, and implying the gap was narrower than it is. Measuring a recorded plan is cheaper
than following it.

**The cheapest step to ledger was cheap for a reason that generalises.** Eight of step 8's
sixteen numerals were already claimed, because two earlier slices had been here for the
`[[derived]]` arm and the ambient split. The remaining eight were controls and topology — one
line of vocabulary each. **A step's ledger cost is measured in numerals nothing accounts for
yet, and the steps other slices have already visited are the cheap ones.** The four steps that
have never carried a slice of their own are the expensive tail.

**Tagging a step's readings is what makes it quotable, and it is cheap until it is needed.**
Step 8 filed three readings of one row under one name for five slices, and nothing minded until
a sentence needed to divide two of them. The tags cost three characters each; what they buy is
an address, and the fence that refuses an ambiguous one is what turned "add a tag" into a
five-minute change rather than a debugging session.
