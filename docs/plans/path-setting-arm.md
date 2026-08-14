# A step length is a number too, and the numbers nobody was guarding

Two things at once, because one of them was blocking the other.

> The sentence still cannot be claimed, and the blocker has moved one check along. It
> prints `0.5`, `5` and `10` as **control settings**, and check 6 — "every number in a
> claimed sentence is tied to something" — knows only `spelled`, `read at` and `shown`. A
> step length is none of the three, so claiming the sentence would mean leaving three of
> its numbers tied to nothing (refused, on purpose) or inventing a waiver (also refused).
>
> — `docs/plans/path-arms.md`

That is the first half. The second is the standing gap both of the last two plan docs
named as the largest thing left: ten of the guided path's twenty-four steps carried no
claim at all, their numbers had been *measured* whole one slice ago, and a measurement
ages the moment it is taken. All three documented drifts in this path landed on steps with no
claim, for the reason nothing else could: there was nothing to redden.

Claims: 88 → 120. Steps with claims: 14 → 16. Arms: 10 → 12.

## The fourth accounting arm

`Accounted` had three ways to justify a number printed inside a claimed sentence — a claim
spells it, a claim is read at it, or a row a claim asserts prints it — and its own doc
comment said what to do when a fourth was needed:

> A future literal printing a chemistry constant or a control setting will fail here
> loudly, and the right answer will be to give it an arm that checks it against the
> chemistry file or the lesson block — **not a waiver**.

`Setting` is that arm, and the interesting part is the second half of the sentence, which
turned out to be *wrong in a way worth naming*. "Check it against the lesson block" is the
generous match this file refuses everywhere else. Step 18's block carries `speed_x: 10`
beside its `dt: 0.5`; an arm that accounted a token against any numeric field of the block
would tie the headline's `10` to the speed multiplier — the right answer off the wrong
field, green today and still green the day either number moves. It is exactly the defect
`ScenarioRule`'s doc refuses when it insists a rule name its field rather than search the
file for the number.

So the tie is to a **trajectory**, not to a block: the token must equal the step length of
a run that a claim *in this same sentence group* is really measured on. On step 18's
headline that resolves `0.5` to the step's own run, `5` to the `dt 5` arm and `10` to the
`dt 10` arm built here. Two consequences fall out of the shape rather than being added:

* `speed_x`, `until_s`, `ambient_c` and every other field are unreachable. Only `dt` is
  overridable by an arm, and only a trajectory some claim reads counts.
* An author cannot declare a setting without building the arm whose numbers it produces.
  The accounting and the measurement come as a pair.

Measured, not assumed: tying `Setting` to the *step's* `dt` instead of each claim's own
trajectory leaves `5` and `10` unaccounted and reddens check 6 by name.

### And a token may not have two readings

`cover_by_rule` panics when two vocabulary rules cover one number; the `ReadAt` arm is
fenced against event instants for the same reason. A fourth arm is exactly where that
hazard comes back, so a token that is both a setting and spelled, read at, or shown is
**refused** rather than resolved by trial order. With an order, an author who meant a
measurement and wrote a step length gets whichever arm happens to be tried first, and the
check becomes a fact about the function instead of about the sentence.

## What claiming the headline found

> 0.56 points at 0.5 s, **5.57 at 5 s**, 11.14 at 10 s, where the cell ends 19 K hotter
> instead of 1.

**Every number is right**, which is the result. Five of the eight are measurements and they
needed two quantities this file did not have; one needed a reading of the sentence that the
sentence does not state.

* **The five measurements are a *cost*, not a level.** 0.56 is not a reading — it is
  90.00 % minus 89.44 %. So `soc_lost_pts_at` and `t_rise_k_at` measure against the
  **zero-length probe**: what the panel showed when the reader pressed Run. That is the
  honest origin rather than a convenient one — it is literally the frame `applyStep` fills
  in after the controls are dialled in and before the run is armed. Claiming the levels
  instead (89.44 %, 316.85 K) would have claimed numbers the sentence does not print.
* **"19 K hotter" is the ten-second arm, and the sentence does not say so.** The clause
  attaches to the last number before it and could as easily have meant the five. Measured:
  the `dt 10` arm ends 18.70 K up, `dt 5` 9.30 K, the base 0.92 K. Only one of the three
  rounds to 19, so the reading is forced by the engine rather than chosen by the author.
* **The proportionality holds further than the sentence claims.** 0.55719 → 5.57188 →
  11.14376 is 10.0000× and then 2.0000×, because the spike is the same 183.84 A under a
  resistive sag that does not care how long you look at it. The heat is *not* proportional
  in the same way — 18.70 K against 0.92 K is 20.2×, not 20 — because 90 s is short
  against this pack's thermal time constant but not zero.

### The `dt 10` arm's instruction is weaker than its sibling's, and it is said so

Every arm has to cite a sentence in its own step's prose that tells the reader to make the
change. The `dt 5` arm cites one that names the value, the button and the reason. The
`dt 10` arm can only cite the headline itself, which reports what happens at 10 s without
repeating the procedure. Nothing else in the step mentions 10 s, and the procedure is the
one spelled out two sentences earlier, so a reader has exactly one way to produce the
number — but the prose never says "now do it again at 10", and a fence stricter than
`contains_number` would refuse this arm. Recorded in the arm's own note rather than left
for the next author to discover.

## The three steps that were carrying numbers nothing checked

### Step 7, the one with the most documented drift in the path

`protection-off` has been repaired twice and had nothing guarding it either time. Nine
claims now: both flags with their instants, the weakest group's 1.9731 V, the 3.7 % charge
still showing when the first one arrives, the −2.6355 V terminal at the mark, and the
−0.671 / −0.651 V spread across the groups, and both ends of the charge owed past empty.
All nine reproduce to the digit the prose prints.

Two of them needed `v_cell_min_at` / `v_cell_max_at`, which are the two ends of the
`cell v` row. Worth stating because the sentence and the field use different words: "the
weakest **group**" and "every **cell**" are the same number here, since every cell in a
parallel group sits at the group's node voltage — `Pack::step` folds both from `v_g`.

The spread of charge past empty — "the eight cells sit between 23.5 and 27.8 points" —
needed the other end of a quantity that only ever reported its maximum, because the page's
`past empty` row shows the worst cell alone and the range is visible only on the pack grid.
`Row::deficit_min` is that end.

**What is still not claimed there, and why it is not a scoping decision:** "(0,0) first at
345.0 s, (1,1) last at 356.5". The cell indices are numbers in the sentence, and no
accounting arm reaches them — they are not measurements, not instants, not settings, and
nothing prints them. Claiming that sentence means either shredding a literal into fragments
that no longer read as sentences, or a fifth arm for "an address in the pack". The crossing
times themselves are real and reproduce.

### Step 15, the first of the fourteen claimed whole

`looks-fine-from-outside` is now eleven claims across three sentences, including the three
readings the panel shows **before** the reader presses Run and the two on the continuation
the step instructs. It is the cheap continuation `docs/plans/path-arms.md` deferred, and it
cost one arm with one button in it.

It also forced a fence open, which is the one design change in this half.

### A continuation may write its instants on the clock

`Accounted::ReadAt` accounted a continuation's time tokens **only** as durations since the
mark. That was an assumption about how prose is written — step 20's `**383.0 s later**` —
rather than a fact, and this step falsifies it:

> 2.502 V at 1058 s and **2.495 V at 1060 s**

The clock is what a reader watches keep running past the mark, so the sentence writes the
absolute time and the narrow rule left both numbers tied to nothing. Both readings are now
accepted, and the fence that made that safe is arithmetic rather than judgement: the two
differ by exactly the mark, so no token can match both — asserted in `accounting_for`
rather than assumed, because the whole widening rests on it.

This is a widening of a deliberate fence, so the argument is worth stating plainly. The
fence's stated worry is "an author tries both frames and keeps whichever matched". For a
*quantity* the two frames give different numbers and the worry is real, which is why
`States::SinceMark` and `States::UntilEnd` are fenced to opposite sides of the mark. For a
read *instant* they are one moment written two ways, and accounting either is true of the
claim.

Measured: reverting to the since-mark-only rule reddens check 6 on both numbers.

### Step 19's protected half

`nothing-to-clamp` carried eleven claims and every one of them was on the arm where the
BMS is switched off. What the protection actually *does* — `OT` at 133.5 s with the pack
already at 51 %, the contactor at 156.0 s, 39.62 % left, the hottest cell at 344.52 K when
the trip fires, and the 344.5 K peak — needed no arm at all and was simply not the last
slice's subject. Seven claims now.

Two of them are worth naming:

* **"this fault costs fifty points"** is the second word numeral in the file and the second
  sentence to state a cost rather than a level: `soc_lost_pts_at` reads 50.3811 against a
  probe of 90.00 %. A word commits to no decimal place, so the rule is half a point.
* **The peak is claimed as a read instant, not as an argmax**, following the precedent
  `docs/plans/path-arms.md` set for this step's twin. What is not guarded is the peak
  moving to another step while the reading here stays put; that is in the claim's note.

## A number in the prose that does not reconstruct

> The trip is a *probe* crossing 343.15 K — the two probes sit on corner cells, and the
> cell that is genuinely hottest is at 344.52 K when it fires, so protection is late by
> **1.3 K** of somebody else's temperature.

344.52 is exact. 343.15 is the threshold from the chemistry file. Their difference is
**1.37**, which prints as 1.4 and not as 1.3 — so a reader who reconstructs the sentence's
own arithmetic from the two numbers beside it does not get the number it states.

It is not necessarily wrong. The sentence is about the *probe's* reading, which is the
value that crossed the threshold rather than the threshold itself, and a probe that lands
just over 343.15 puts the gap slightly under 1.37. **No quantity in this file reads a
sensor** — CLAUDE.md's eighth principle keeps the BMS's view behind the sensor layer, and
this harness measures ground truth — so the harness cannot settle it. Left unclaimed, and
named here and in the claim's note rather than quietly skipped: it is the one number in
these four steps that a reader could compute for themselves and get a different answer.

## What was measured

Eighteen perturbations plus the null, each launched with a real exit code from
`subprocess.run` at below-normal priority, each recording *which* test reddened. `start
/wait` is not used anywhere: it is on the record twice as exit-code-blind.

| perturbation | reddens |
| --- | --- |
| no perturbation at all — the null | nothing, exit 0 |
| `Accounted::Setting` deleted — the three-arm accounting is back | the accounting |
| `Setting` tied to the STEP's `dt` rather than to each claim's own trajectory | the accounting |
| the `dt 10` arm declares a 7 its instruction does not spell | the arm fence, the accounting, and the value |
| the 11.14 claim reads the `dt 5` arm instead of `dt 10` | the value |
| **CLASH**: a claim on the headline spells the step length `5` | **the clash refusal, by name**, plus the value, the stated check and the tolerance rule |
| the 19 K claim's `spells` mis-pointed | the stated check, the accounting, and the tolerance rule |
| **REVERT**: a continuation's instants account only since the mark | the accounting |
| the weakest-group claim reads `v_cell_max` instead of `v_cell_min` | the value |
| `deficit_pts_min_at` returns the largest deficit instead of the smallest | the value |
| the 3.7 % claim's `spells` mis-pointed | the stated check and the tolerance rule — **not the accounting** (below) |
| the −0.651 V claim's shown string loses a digit | the display half |
| the peak claim read at the contactor instead of half a step earlier | the value |
| the fifty-points claim reads the level instead of the cost | the value |
| the 3.927 V claim drops `probe` and reads the first stepped row | the value |
| the continuation's claims lose their `arm` | reachability, and the value |
| the 3.449 V claim's `spells` mis-pointed | the stated check and the tolerance rule — **not the accounting** |
| the arm runs 5 s past the mark instead of to 1080 | reachability, and the value |
| the arm stops at 1060, exactly where its furthest claim reads | **nothing — green** (below) |

Two were hand-validated against the failure text rather than trusted from an exit code. The
clash case fails with `it prints 5, which is both spelled by a claim on it and the step
length of a trajectory this sentence's claims read (5 s)` — the refusal stated in its own
words. The `Setting`-deleted case fails on the accounting with `- setting: it is not the
step length of any trajectory this sentence's claims read`, naming the arm that is missing.

Two failed to apply on the first pass and were re-run rather than reported: one named a
`Lesson` field that does not exist (a compile error, which the parser reads as "reddens
nothing") and one matched an anchor twice. Both are in the table above in their repaired
form. The first is a reminder that an exit code alone cannot tell a perturbation that
proved something from one that never ran.

### The green, and a limit it exposed

**Stopping the arm exactly where its furthest claim reads is green**, and that is expected
rather than a gap: nothing checks how far past its last claim an arm runs, which is the
standing weakness `docs/plans/path-arms.md` recorded when it extended two arms for the same
reason. The convention is a convention. What *is* checked is the other direction — an arm
that stops **short** of a claim reddens reachability, shown two rows up.

**Mis-pointing `spells` on a claim that also has a display half does not redden the
accounting**, and this is worth stating because it was expected to. Check 6 tries `spelled`
first and `shown` third, and a display claim's `shows` string contains the same number — so
`3.449` is still accounted, by the row rather than by the claim. Nothing is unguarded (the
stated check and the tolerance rule both fail, loudly, and the `shows` string is itself
asserted against the page's formatter), but the accounting's *spelled* arm is not
load-bearing for any claim that names a row. It is load-bearing for the ones that do not,
which the clash case demonstrates: that claim has no display, and check 6 fails on its
number.

## Deferred, with a price

* **Eight steps still carry no claim at all.** `pack-disagrees` and `lying-sensor` are
  ledgered rather than claimed, which is a different kind of covered — their digits are
  scenario constants and the ledger says so. The other six are unchecked.
* **Two accounting arms are still missing and both were met head-on this slice.** A
  chemistry constant (`the 2.50 V cut-off`, `5.15` A·h) and a figure derived from other
  figures in the same sentence (`96 s after the short`, `88 %`). Each one cost a literal
  that had to be cut short of the fragment naming it, and each cut is recorded in the
  claim's note. `docs/plans/path-prose-ledger.md` designs both.
* **"Not one flag is raised on any step of the run" is unclaimable**, and it is not a
  taxonomy problem. This file can say when a flag arrives; it has no quantity for "none
  did". The sentence is true of step 15's run to its mark, and the continuation arm now
  runs through t = 1062 where `OPERATING_POINT_OUT_OF_WINDOW` arrives — two seconds past
  the last number the step quotes. Nothing reads it. The step whose whole subject is "an
  answer that gives you no sign it is wrong" still has a sign just past where it stops
  looking.
* **The `1.3 K` above is unresolved, not verified.** Settling it needs the harness to read
  a BMS sensor, which is a capability this file does not have and should not grow lightly.
* **`soc_lost_pts_at` and `t_rise_k_at` measure against the probe, and the probe is a
  choice.** It is the right one — it is the panel the reader is looking at when they press
  Run — but a sentence that stated a cost against some *other* origin would be mis-served
  by them, and nothing in the file would say so.
* **An arm's `run` length is still this file's own choice**, unchanged and now over twelve
  arms rather than six. Step 15's runs 20 s past its furthest claim, following the rule
  that an arm stopping exactly where its last claim reads makes "reachable" mean only "I
  ran long enough to reach it".
