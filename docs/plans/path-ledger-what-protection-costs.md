# The eleventh lesson, ledgered

`what-protection-costs` is scanned whole — **eighteen of the twenty-four steps, 372
numerals, twenty-three arms**. Twenty-eight numbers, thirteen of them inside a claimed
sentence and fifteen accounted for by a vocabulary rule. It cost two new arms of the
taxonomy (`Tie::Magnitude`, and `Control::CcCvVoltage` under the existing `Setting`), two
new quantities, one new field on `[[arm]]`, eleven new claims, two new trajectories, ten new
rules, and one sentence rewritten because the number in it was not decided by the
simulation.

## Why this step and not the cheaper one

The last slice's queue put `leg-that-is-not-there` first at twenty-two and this one second
at twenty-five, both from the out-of-tree proxy. Re-measured with the instrument before any
editing — the ledger's own scan, temporarily switched from panic-on-first to
print-and-continue — this step is **25 unaccounted of 28**, which is what the proxy-vs-
instrument note in `path-ledger-spm-step.md` already recorded and which held.

The reason given for taking it out of order **was wrong, and is worth writing down as
wrong.** The previous slice rewrote a sentence in step 18 that now depends on this being
the last step before it where protection derates a demand, and the plan was that scanning
this step would convert that dependency from inspection into assertion. It does not. That
is a claim about the *set* of lessons; the numeral scan ties this step's digits to files
and says nothing whatever about whether any other step derates. **The hole is still open**,
and closing it is a check over the lesson list rather than a ledger entry — a candidate for
a later slice, named here so it does not get smuggled into one.

What the step was worth taking anyway: it is the eleventh lesson and the first place on the
page where the BMS *acts*, most of its twenty-five gaps are genuinely tie-able constants,
and its headline number is a subtraction between two trajectories that did not exist.

## The two trajectories the step never configured

Everything this step calls "what protection costs" is a difference between the step's own
run and the same pack with the checkbox cleared, and until this slice the unprotected run
was not in the tree at all. Two arms:

* **`unprotected`** — `bms = false`, `restart` (the page's BMS checkbox clicks Reset, so a
  mid-run toggle is not a thing the page can do), and **two `run` actions** rather than one.
  That is the sentence: the mark stops the page at 4100 s and the prose tells the reader in
  so many words to *"press Run again when the mark passes and let it finish"*. A single run
  to 5200 s would be a trajectory nobody is instructed to produce.
* **`one C`** — the same pack asked for 6 A instead of 3.

## `cc_cv_a`: the page has two current boxes and the harness had one

The arm schema already had `demand_a`, and it is the wrong field here. Its own doc says
typing into the simple box **replaces the program** — that is what the box does, and a
number typed there on a `Pulse` step ends the pulse train. The same is true on a CC-CV step,
except worse: the simple box is discharge-positive and the CC-CV group's charge current is
not, so `demand_a = 6.0` on this step would have run a **6 A discharge** under a sentence
about a charge. Green arithmetic on the wrong trajectory, which is the shape this whole file
exists to keep out.

`applyDemandMode`'s own comment is the argument for two fields rather than one mode-aware
one: *"The single `value` box cannot serve CC-CV: the mode needs three numbers, and one of
them is entered with the opposite sign convention to everything else on this page."* So
`cc_cv_a` is a second field, `arm_control_value` reads `cc_cv_a.or(demand_a)` because
`applyDemandMode` shows one group at a time, and `arm_prog` replaces the CC-CV current
in place instead of replacing the program.

Its three refusals — both boxes at once, a CC-CV current on a non-CC-CV step, an override
that changes nothing — are all unreachable from `path-claims.toml` the moment they work, so
the block was extracted into `check_cc_cv_current` and two of them priced with
`should_panic` tests. That is the pattern `an_on_arm_may_only_wrap_a_setting` established,
and this file has been caught twice by fences that were only paragraphs.

## One number left the page, and it was the interesting one

The sentence said:

> With the BMS off the same pack runs on to **99.5 %**, which it reaches at 4820 s, 720 s
> past the mark.

**Neither half of "which it reaches at 4820 s" was checkable, for two different reasons.**
The pack passes 99.5 % well before 4820: it reads 99.5012 % at 4800 s, already inside the
tenth the sentence prints and twenty seconds before the boundary. (How much earlier than
that is not stated here, because it was not measured — an interpolated interval quoted to
the second, in a document about numbers that have to be read off a run, is the shape this
project keeps finding.) So the instant is not an arrival. And the page's own stopping instant is not a function of the
simulation at all. `ccCvDone` is evaluated at the end of each *chopped chunk*; a chunk ends
at a decision-window boundary **or** wherever the frame's step budget ran out, and that
budget comes from `elapsed * speed / dt` — wall-clock time. The current crosses the 0.3 A
cutoff at 4817.5 s, so the reader is told somewhere in (4817.5, 4820] and only the far end
of that interval is decided by the trajectory.

`docs/plans/path-prose-ledger.md` had already reasoned about exactly this and concluded the
step *"can honestly say an unprotected charge finishes at 4820 s … the reader is told at the
next decision boundary."* That is right about the mechanism and states a **bound** as if it
were the value. The sentence now says what the mechanism supports:

> With the BMS off the same pack runs on to **99.5 %**, and the charge ends by 4820 s, 720 s
> past the mark.

and `cccv_window_close_s` is the quantity: the first decision-window boundary at or after
the crossing. `cccv_taper_s` — the quantity step 9 uses for the same event — refuses here by
design, because it asserts the crossing lands on a boundary and 4817.5 s is step 9635 at
dt = 0.5, which 20 does not divide. Step 9's 6210 s does, which is why that step can say
"stops at" and this one says "ends by".

The reading the sentence rests on is robust across the whole interval: the pack is at
99.5197 % at the crossing and 99.5223 % at the boundary, so the number a reader is shown
does not depend on the frame schedule even though the instant it is read at is the only end
of the interval that is a fact about the simulation.

## Two new quantities, and why neither is two claims on the ends

`v_cell_spread_mv_at` and `v_below_cccv_target_mv_at` are both gaps the page *shows* and
never prints, on the terms `soc_gap_pts_at` and `t_gap_k_at` were added on. The paragraph
above `v_cell_min_at` in `measure_row` argues for keeping the two ends separate because *a
claim states one number* — which is the argument **for** a spread quantity here, not
against: step 11's prose never prints either end. It prints *"spread over 11 mV"* and
*"the 130 mV gap"*, and claiming the ends would be stating two numbers the sentence does not
contain.

The second one needs the CC-CV target, which is `v_cell * series` — `ccCvNote`'s own
arithmetic and no file's field — so it needs the demand program and the topology and could
not live in `measure_row`. `Run` grew a `series` for it, read off the built pack rather than
off the scenario so a twin arm cannot be measured against the declaring step's topology.

**`25` and `130` are the same quantity a step apart**, which is why the pair is worth one
name: 24.942 mV under charge at 3985.5 s, 130.188 mV with the current at zero at 3986 s. The
alternative for the `25` was arithmetic over the sentence's own printed `16.80` and
`16.775`, which comes to exactly 25.0 and would have been green forever — it checks that the
sentence is consistent with itself and nothing else. Claimed against the engine it is pinned
at half a millivolt.

### A prose looseness the pair exposed and did not fix

*"The 130 mV gap you see after the trip is not imbalance; it is the IR drop vanishing when
the current stops."* The IR drop accounts for **105 mV of the 130**; the other 25 were
already there and are the claim three sentences earlier. The sentence is right about what a
reader watches *change* at the trip, and wrong if read as "the gap IS the IR drop". It is
left as prose and recorded in the claim's note, because the two readings differ only in
whether "the 130 mV gap" is the subject of *is* or the thing the change is measured on — and
no number moves either way. Worth knowing that a green ledger here does not settle it.

## `Tie::Magnitude`, because the scanner has never seen a minus

`written_numbers` finds digit runs and never a leading sign, which has been harmless for
seventeen steps because every file it compared against answered positive. *"drag the ambient
to −5 °C"* is the first ledgered sentence where one does not. The claim side has carried
`states = "magnitude"` since it was written, for a sentence that *"prints the magnitude and
puts the sign in the word late"*; this is its ledger twin, with the same fence — refused on a
value that is not already negative — and the same reason: on a positive value it is the tie
it wraps with extra words.

The sign has to be in the phrase (`"drag the ambient to -{n} °C"`), so a rule cannot account
five degrees above freezing against a slider dragged five below.

**It exposed a real defect in the arm counter.** `n_ledger_arms` walks each rule's ties and
its own doc says *"an arm used only inside another is still an arm"* — and `Magnitude` fell
into the walker's catch-all, so the `OnArm` and `Setting` inside it were invisible to the
count. Right today only because both are some other rule's outermost tie. Fixed, and the fix
is **unobservable**: dropping `Magnitude` back out of the walker leaves the whole suite
green. Written the correct way round rather than the reachable way round, and said so in the
code.

## `16.80` and `4.20` are the same digits off different files

One sentence prints both: the pack is *"25 mV short of the 16.80 V it is aiming for"* and its
top cell *"has already crossed 4.20"*. The first is `ccCvNote`'s target — the CC-CV box times
the series count — and the second is `cell.v_max`, the limit the over-voltage rung trips on.
They agree to every digit on this pack, which is exactly the two-readings-of-one-number
hazard the taxonomy is arranged against, so they are read off different files rather than off
whichever was reached first. Retype the page's CC-CV box to 4.15 and the first number moves
while the second stays; that is the property that makes them two rules.

`Control::CcCvVoltage` is the third field of the CC-CV group to get a variant, after the
charge current and the taper.

## The perturbation record

Thirty edits ran, each with the panic message captured rather than the exit code alone,
because this suite has reddened on the wrong assertion more than once. Twenty-nine are red
and one is green — but **twenty-seven** is the number that means anything: two of the reds
came from a different assertion than the one they were aimed at, and were superseded by
re-aimed edits which are counted among the twenty-seven. The green is the result, not a
miss. Counting 29 clean falsifications would be exactly the mistake
`path-ledger-bare-curve.md` records — a perturbation's red coming from the wrong claim. Two more were malformed and never ran — an anchor
naming a field the claim does not carry, and one that matched both cold arms at once — and
both were re-aimed. An edit that changes nothing is not a passing test, which is the same
note the last slice wrote about adding an unknown TOML key. Three things worth keeping:

* **One perturbation reddened a lesson earlier and never reached its target.** Changing
  `cell.capacity_ah` to move the rate that `3 A is 0.5 C` works out to killed a flag claim on
  a different step first (`OPERATING_POINT_OUT_OF_WINDOW` no longer fires). Re-aimed at the
  prose's own `0.5 C`, it reddens on the rate tie with the demand box untouched, which is
  what it was for.

* **The ambient rule cannot be falsified by moving the ambient, and the reason is
  structural.** An arm's control value must be spelled in its `instruction`, and the
  instruction must be a substring of the step's prose — so prose and arm cannot be made to
  disagree by any single edit, and every attempt reddens the arm check instead of the ledger.
  What *is* isolable is the substantive claim in the rule's own comment: pointing its `OnArm`
  at a name no arm carries reddens by name, which shows the rule resolves through the arm
  rather than through anything that happens to equal 5.

* **A green perturbation, and it is the result.** Removing `Tie::Magnitude` from the arm
  counter's walker changes nothing today. See above.

## The rule-reach sweep, run rather than asserted

All 145 rules matched against all 24 lessons before this was committed. The count of rules
reaching a step they were not written for stays at **three**, and none of the ten new ones is
among them — but the standing tally's *meaning* changed:

* Step 3's two scatter rules reach `what-protection-costs`, and that step is now **ledgered**,
  so for the first time they are doing real work outside the step they were written for: two
  of this step's twenty-eight numerals are accounted for by step 3's vocabulary.
* **They are right for the right reason, not by coincidence**, and the previous slice's note
  saying otherwise is corrected here. `Tie::Scenario` resolves against *the lesson being
  scanned*, so on this step the rules read `cc_cv_charge_pack.toml`'s own `capacity_sigma` and
  `r0_sigma`. What coincides is the two scenario files' values, which is why the reach went
  unnoticed; what would happen if they parted is that the rule reddens on this step's own
  prose. Perturbed and confirmed: setting this scenario's `capacity_sigma` to 0.04 reddens
  naming `what-protection-costs`.
* `**Step {n}**` still also matches `what-it-cost` (step 21, unledgered), where it is right by
  identity.

So the liability recorded last slice — *"a rule right by coincidence is a liability a re-fit
could turn into a wrong green"* — does not apply to these two. A rule whose tie is a file
path is re-resolved per step; only a rule whose tie is a *constant* could be right by
coincidence, and there are none of those.

## Where the ledger stands

Eighteen of twenty-four steps scanned whole, 372 numerals, 145 vocabulary rules,
twenty-three arms. **Six steps left**, and every one of them carries claims already, so no
step in the path is entirely unchecked.

The queue, by the proxy that runs optimistic — re-measure with the instrument before
budgeting, which is what this slice did and what confirmed the ranking:

| step | proxy |
|---|---|
| `leg-that-is-not-there` | 22 |
| `past-empty` | 27 |
| `same-discharge-other-chemistry` | ? |
| `three-times-the-current` | ? |
| `the-gradient-itself` | ? |
| `what-it-cost` | ? |

Two things known about that queue rather than guessed:

* `leg-that-is-not-there` is a CC-CV step whose LFP cell never reaches the band, so it runs
  on a constant current under either windowing rule. **It now has two quantities it did not
  have before** — `v_below_cccv_target_mv_at` is exactly the "how far from the target" figure
  a step about a missing constant-voltage leg would want, and `cccv_window_close_s` is there
  if its charge ends off a boundary.
* `what-it-cost` is step 21 and the only unledgered step a rule already reaches.
