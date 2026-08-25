# The step that quotes its neighbours — and a ratio two rounded figures got wrong

`three-times-the-current` is step 14: the same single-particle cell as step 13, the same
pulse train, the current tripled to 3 C. It is the **twenty-second of twenty-four** steps
scanned whole, and it was picked by measurement — 31 unaccounted of 39, the cheapest of the
three that were left, exactly as the last slice's table predicted.

It cost eight claims, twenty vocabulary rules, one new arm, a third current box on `[[arm]]`,
and a fourth fence on `pack_from`. Three numbers in the prose moved, one of them a ratio a
previous measuring pass had recorded as correct.

## What is unusual about this step: most of its numbers belong to other steps

Every ledgered step so far has printed constants out of its own files and measurements off
its own run. This one is a *comparison*, and a comparison prints one number of its own for
every number it is comparing against:

| kind | count |
| --- | --- |
| a quotation of a claim on step 12, 13 or 15 | 6 |
| a ratio of two such quotations | 7 |
| ordinary ties (charge, shells, topology, three rates, the cut-off, three ordinals, two boxes) | 14 |
| numbers inside this step's own claimed sentences | 12 |

`Tie::Quoted` already existed — steps 22 and 23 built it — but this is the first ledgered
step where it carries most of the vocabulary, and the first where `Tie::Ratio` divides two
**measurements** rather than two constants. That combination is what makes the step's
headline checkable: *"no single resistance can be 1.87 and 6.02 at once"* is now two
quotients of four pinned claims, and moving any of the four turns the sentence red.

## The ×6.01 that is ×6.02

The prose said the particle's slow climb goes 17.3 → 103.9 mV, *"which is ×6.01"*.

103.905970 / 17.268344 = **6.0171**, which rounds to 6.02. Where 6.01 comes from is not a
mystery: 103.9 / 17.3 = 6.006. The ratio had been worked out from the two already-rounded
millivolt figures printed beside it, which is precisely the defect `pulse_train_spm.toml`'s
own header records about a different number in the same paragraph — *"that figure had been
subtracted from the three ROUNDED parts instead of measured."*

**It had been measured before, and passed.** `docs/plans/path-numbers.md` says, under
"checked and correct":

> The pulse trio's ratios are exact — ×1.872, ×6.017, ×2.483 for the particle … against
> ×1.87, ×6.01, ×2.48 … All five land.

The measurement is right and the comparison beside it truncated where it should have
rounded. That is the one way an eyeball pass over a correctly measured column can still go
wrong, and it is an argument for the ledger rather than against that pass: a rule that
divides two claims cannot truncate, because nothing in it ever reads a printed token.

The number is corrected in six places — the step's prose twice, both scenario headers,
`spm-scenario.md`'s table and its prose, and the README — and `path-numbers.md` now carries
the correction beside its own green line.

## The third current box

`[[arm]]` could type into two boxes: `demand_a` (the simple field) and `cc_cv_a` (the CC-CV
group's charge current). The page has a **third**: `applyDemandMode` hides `demand-simple`
on a `Pulse` step exactly as it does on a CC-CV one, because a pulse train needs three
numbers of its own.

So an arm typing `demand_a` here would have described a box that is not on screen — and the
failure is worse than unreachable. `arm_prog` reads the simple box as *replacing the
program*, so the arm would have run a flat 3 C discharge under a sentence whose every number
is per-tooth, and the claims would have measured that. `Arm::pulse_a` is the field, with the
same four questions asked of it that `check_cc_cv_current` asks of its neighbour, and two
`should_panic` tests for the two that no file in the tree can reach.

**One of those four fences had to choose which lesson it compares against, and this arm is
the first that could tell.** `pulse_a = 15.459` is *identical* to step 14's own demand box
and three times the box of the lesson its pack comes from. Read against the declaring step
it is "an override that changes nothing"; read against the pack's lesson it is a threefold
change. The file already said which — *"Every 'an override that changes nothing' fence below
compares against THIS lesson and not against the declaring step"* — and noted that no arm
could reach the difference. This one does.

## Which walk a walk is

`docs/plans/path-ledger-what-it-cost.md` closed with the question this slice had to answer
first:

> `pack_from` walks to another lesson; neither of those two steps' sentences is a walk to a
> lesson, it is a load from the picker. Whether that is the same thing is the next slice's
> first question.

The two readings differ in **what gets re-dialled**. Pressing **Back** lands on the named
lesson, and `applyStep` re-dials *that* lesson's controls. Loading from the scenario picker
changes the file and leaves the controls of the step you are standing on.

Here they coincide: step 12 and step 14 are both `dt = 0.5`, both 25 °C, both no BMS, both
60 s on / 600 s off. So both readings produce one trajectory and the sentence means one
thing.

**That coincidence is now a rule.** `assert_walkable` grew a fourth fence: a walk may change
the FILE and nothing else, so a `pack_from` arm whose two lessons disagree about any control
the arm does not itself type is refused. It is not a fidelity claim about `applyStep` — it
is a refusal to *pick* a reading on the author's behalf where the two would measure different
runs. `pack_from`'s own docs already admitted that its block half (timestep, ambient, BMS)
was "written the way the page behaves rather than the way a test could tell"; that half is
now checked, in the only way it can be, by refusing the case where it would matter.

Measured, not assumed: moving either lesson's `dt`, or shortening step 12's rest leg,
reddens with the new message and not with something else.

## Two prose repairs the claims could not have caught

Both are sentences that were true *at the instants their claims read* and false between
them. A claim only ever looks where it is pointed, and the ledger reads sentences rather
than trajectories, so neither check could have found these — the run had to be looked at.

* **"It pins at 0.3095 V and stays there."** Two claims pinned 0.3095 V at 12 600 s and
  13 260 s, one whole tooth apart, precisely to make "stays there" checkable. Both are ends
  of *loaded* legs. Between them the ten-minute rest carries the terminal back to about
  1.17 V, every time. What repeats exactly is the **bottom of each tooth**, which is what
  the sentence says now.
* **"Each tooth now starts at about −0.45 V."** Two things wrong. The hedge: the first frame
  of a tooth is −0.442079 V and the second is −0.450831, so "about −0.45" was true of the
  second frame and not the first, and nothing said which. And "each tooth": the circuit
  clamps in the *middle* of a tooth, so the first tooth below empty starts at 2.501 V and
  only the ones after it start on the floor. The sentence now says "from the next tooth on",
  gives −0.442 and −0.657 — the three decimals the readout actually prints — and each is
  pinned at two instants a period apart.

## Two stale headers, settled from the run

`pulse_train_spm.toml` said, of driving past the clamp, that the particle *"pins at
0.39–0.50 V … where the circuit cannot leave its own `[ocv]` and stops at 1.79 V"*, and
`spm-scenario.md` said the same. Both predate `[reversal]` (v13→14), which gave an
equivalent-circuit cell's open-circuit voltage a ramp and a floor below empty — so the
circuit does not stop at 1.79 V, it falls through zero. `reversal-ui.md` had already noted
the 1.79 was wrong and left it; this arm is the instrument that replaces it. Both paragraphs
now carry measured figures: 0.309467 V at the bottom of every loaded leg on the particle,
recovering to about 1.17 V over each rest; −0.442 V and −0.657 V on the circuit's teeth.

## Where the ledger stands

Twenty-two of twenty-four steps scanned whole, 539 numerals, 214 vocabulary rules, 249
claims and twenty-one `[[arm]]` blocks. **Two steps left.**

| step | unaccounted | of | what it needs |
|---|---|---|---|
| `same-discharge-other-chemistry` | 35 | 40 | a third scenario the reader loads from the picker, plus two demand variants |
| `the-gradient-itself` | 38 | 42 | almost all measurements: claims, not rules |

Those two counts are the previous slice's and were **not** re-measured here, on the same
terms that slice stated: this one added twenty rules, and a rule's phrase can in principle
match a sentence in a step it was not written for. The next slice should point the scan at
its own target and read the number — a print-and-continue run over one step takes under a
minute, and the recorded sizing has now been right three times running.

`same-discharge-other-chemistry` is the natural next one: it is a *load from the picker*
exactly as this step's closing instruction is, so the arm shape is built and the fence that
governs it is written down.

## Deferred, with a price

* **One figure in this step answers to nothing, and the scan cannot see that it doesn't:
  "about ten times the circuit's arithmetic per step."** It is a cost ratio the step
  deliberately refuses to make a duration, for the reason its own prose gives — the
  microseconds move by half again between sessions on one machine — and there should be no
  claim, because a benchmark in this file would be a number about a laptop rather than about
  a model. What is worth writing down is *why* it passes: the scanner reads digits, and this
  one is spelled in letters. Write the same sentence as "10 times" and the step goes red with
  nothing to fix it. That is the digit scanner's standing limit, recorded here as it was on
  step 23's "half-second step", not a hole this slice left open.
* **The new `pack_from` fence compares four things and could compare more.** It reads `dt`,
  the ambient, the BMS and the demand's shape. It does *not* read the mark or the speed,
  because neither can move a trajectory that specifies its own `to_s`. If a future walk ever
  ends on a mark rather than a `Run`, that exemption stops being free.
* **`Tie::Quoted` now carries eight placeholders on one step, and its agreement assert is
  the only thing keeping the addresses honest.** Every claim on a named `(step, arm,
  quantity)` must agree; what nothing checks is that the *quantity* named is the one the
  sentence is about. A rule pointed at `pulse_sag_mv:1` where the sentence means the jump
  would fail only if the two happened to round differently. They do here — which is luck,
  not a check.
* **The two remaining steps are the ledger's last, and one of them is a twin of this one.**
  `same-discharge-other-chemistry` sits next door to step 1 and looks like it, which the
  `bare-curve` slice already flagged: it is three times step 1's cost and nothing in the
  recorded ranking said so.
