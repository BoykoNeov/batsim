# The step that is about its own step length, ledgered

`one-step-that-got-through` — step 18, the dead short the contactor catches — is the
seventeenth of the guided path's twenty-four steps to have its whole prose scanned.
Thirty-one numerals, seventeen of them inside sentences a claim already quotes and
fourteen tied by the vocabulary. It cost **three new arms**, a new control, five new
claims, one new sensor channel, **one harness defect**, and two sentences that were
rewritten rather than tied.

## The pick, and a recorded note that was wrong

The last slice's ranking nominated this step at twenty unaccounted numerals and added a
warning: *"neither of those steps has a twin that has already been scanned"*. That is
false. Step 19 (`nothing-to-clamp`) is this step's twin — same pack, same seed, same
fault at the same second, 100 milliohms instead of 30 — and it was ledgered four slices
ago. The warning was written about the *cost* of the step and it got the cost's main
driver backwards.

What the twin actually bought, measured rather than assumed: **nothing was double-covered
and nothing had drifted.** Putting this step into `[ledger].steps` and running the scan
with every failure collected instead of the first one thrown produced twenty-one gaps and
no other complaint — no rule of step 19's reached this step, and no rule of this step's
reaches step 19. The one thing the twin did cost is stated below: it reads one of this
step's claims, so re-addressing that claim reddened the neighbour.

## The harness defect, and why it had never fired

The scan's first run reported twenty-one gaps and one of them was a number a claim
already spells — *"where the cell ends 19 K hotter instead of 1"*, whose `1` is the
`spells` of a claim on that very sentence. Check 6 was green on it at the same moment.

`written_numbers` scans a digit run and then trims it: `at 5769.` is a five-byte run and
a four-character token. `Written::len` carried the **run** and `Written::token` carried the
**trim**, and `claimed_accounting` used `at + len` as the number's extent — so a number at
the very end of a claimed literal, followed in the prose by a full stop, tested as sitting
*outside* the literal that contains it.

Two directions, and only one of them is safe:

* On the number itself it fails toward **red** — the ledger demands a rule for something a
  claim already accounts for, which is loud.
* On the double-accounting panic beside it, it fails toward **green**. That check only
  fires when a rule *and* a claim both answer for one number; a claim that cannot be seen
  cannot clash with anything. Two readings of one number is the hazard the whole taxonomy
  is arranged against, and this is the one shape it could not see.

**It had a second reader, and the struct's own doc asserted the opposite.** `rule_matches`
advances past a matched number by `at + len` too, so while the field carried the run, a phrase
could never name the character that ends a sentence: `"at {n}. The"` would look for `". The"`
from a position already past the full stop. The paragraph on `Written` said the run was
*needed* for exactly this — *"what follows a number in the sentence begins after the
characters that were actually there"* — which is a true observation with the conclusion drawn
backwards. Both readers wanted the token, and neither said so.

`len` now follows the trimmed token. Measured across all seventeen ledgered steps: the fix
changes nothing anywhere else, so no existing rule was leaning on the old behaviour. **The defect was latent** — no already-scanned step prints a
claimed number at the end of a literal with a full stop behind it — and step 18 is its
first live instance. `join_thousands` was already immune, because it deliberately measured
from `prev.token.len()` after being bitten by the same field once before; that comment is
the only reason the bug was recognisable on sight. The cover is no longer incidental either —
step 18's prose is what reddens it today, and a sentence can be reworded, so the scanner's own
unit test now asserts the extent directly on five shapes.

## Three new arms

The taxonomy has been "finished" since step 22 and three slices in a row added nothing to
it. This step needs three, and each is a sentence the existing arms cannot carry.

**`Tie::Label { id, prefix }`** — a digit that is part of a **control's label**, read out of
`web/index.html`. The step says *"press **Step 1** twice"*, and that `1` is no more a
quantity than the `50` of an LG M50: it is half the name of a button. `Tie::Name`'s twin
one file over, anchored on the `id` and cut at the next tag so the search cannot wander.
Rename the button and the sentence goes red — which matters, because a label is the one
thing about a control that a reader cannot infer.

**`Tie::Count(path)`** — how many entries an array in the scenario has. The step says
**Clear queued** *"counts the short it removed as **1 fault**"*, and that number is the
file's *shape* rather than any value in it. Every other arm here reads a value; this one
reads a length, and refuses anything that is not an array — a count of a scalar would
invent the number `1`, which is the value that would most often make a wrong sentence
look right.

**`Tie::Ocv(tie)`** — the chemistry's open-circuit voltage at a charge the inner tie names,
interpolated off the `[ocv]` table the way `sim-core` reads it. The step's last aside says
*"on this plateau half a percent of charge moves the open-circuit voltage by 0.4 mV"*, and
that figure is in no file: it is four cells in series times the curve's difference between
the charge the pack started at and the charge the tooth left it with.

> **What that rule does not settle, said here rather than left to a green check to imply.**
> The sentence says "half a percent". A round 0.5 points gives 0.4 mV through this rule and
> the measured 0.557 points gives 0.44576, which also prints `0.4`. The tie cannot tell those
> two readings apart. What it catches is a re-fit of the plateau, a change of series count,
> or a different starting charge — which is what the sentence actually rests on.

Its rounding needed a test of its own, for the reason `Tie::Hours`'s did: the only user sits
two levels inside a `Tie::Product`, and `tie_agrees` is asked about a rule's outermost tie, so
lifting `Tie::Ocv` out of the rounding group leaves everything green. Measured, not assumed —
the perturbation was run, and `an_ocv_tie_rounds_the_way_a_computed_tie_does` was the only
test in the file that noticed.

A fourth arm was expected and is not here: `Control::Dt`, the `dt` box, is a new **control**
rather than a new tie. Before it the ledger could read every box on the page except the one
this step is about.

## Two readings of one temperature, and the instant tag that separates them

The step states its temperature rise twice on one trajectory: *"0.96 K, is the whole cost of
a dead short"* and *"the cell ends 19 K hotter instead of 1"*. Both are true and they are
different numbers — 0.96212 K at 60.5 s when the tooth lands, 0.92491 K at the 90 s mark
after thirty seconds of cooling. The base claim's own note has said so since it was written.

`Tie::Quoted` cannot borrow either while two claims answer to one name, and the twin next
door borrows exactly this quantity for its *"a twin whose run ends at 299.1 K"*. So the two
readings are tagged — `t_rise_k_at:90` and `t_rise_k_at:60.5` — and step 19's rule now names
the mark-side one.

The tag mechanism only covered *row* quantities before this. The two **cost** quantities —
what the run has spent by an instant, measured against the zero-length probe — take one now,
and the tag is stripped **after** the probe refusal rather than with the others. That
position is load-bearing: in front of it, a tagged cost quantity read on the probe would
resolve to zero by construction and go green about a measurement nobody made.

## A claim reads the BMS's voltage sensors, and the coincidence is stated

Two of the step's numbers are explicitly the sensor's: *"protection decides from sensors
sampled at the end of the previous step, which still read a resting 3.3142 V per group"*,
and *"the frame taken after the spike reads 1.3336 V"*. `Sensed` carried the temperature
channel alone, deliberately — the others "would be fields with no consulting code". They
have one now: `v_group_min_at`, the lowest measured group voltage, which is the channel the
hard under-voltage rung is judged on.

**Measured, and the reason it is written down rather than left as reassurance: on this pack
belief and truth are the same number to the last bit.** `sim-core` gives the current sensor
an offset and a noise term and gives the voltage sensor neither, so a claim pointed at
`v_cell_min_at` would read the same digits today. The claims read the sensor anyway, because
that is what the sentences are about, and because the day a voltage-noise term lands a
truth-side claim would go on passing about a number the sentence does not name. CLAUDE.md's
eighth principle is that the gap between truth and belief is a feature; a check that cannot
see the gap cannot see it close either.

The 1.3336 is the **lowest** group and not the mean, and that is the sentence's own claim
rather than a convenience: the four groups sag to 1.4177, 1.3846, 1.3621 and 1.3336 because
the scatter gave them different resistances, and a rule reading the average would answer
1.3745 and be wrong about which number protection saw.

## What left the page, and what was rewritten

**One numeral left**: the `0` of *"rebuilds this pack at t = 0"*. Nothing in the tree decides
the origin of a clock — it is notation, not a quantity, and the `R0` precedent covers it. The
sentence now says "rebuilds this pack from the start", which says the same thing and carries
no number. The `dt 5` arm's `instruction` quotes that sentence verbatim and moved with it.

**One sentence was rewritten for a reason the scan could not see at all.** The step's third
paragraph opened *"Eleven steps of protection have derated a demand."* That is a self-count
of the path's own contents, written in letters, and no check in this repo can read it. Two
readings of it are available and they disagree:

* *eleven steps have passed since protection was introduced* — true when the sentence was
  written (`f308f38`, when this was step 15 and protection arrived at step 3), and
  **fourteen** today, because three steps have since been inserted ahead of it;
* *through step eleven, protection has only ever derated* — still true, because
  `what-protection-costs` is still the eleventh lesson.

Neither is checkable and the sentence's truth depends on which one a reader takes.

**The first replacement was wrong in a way worth recording, because it is the trap this
whole section is about.** It read *"Every step of protection so far has derated a demand"* —
which trades an ambiguous count for a universal quantifier over seventeen other lessons that
no scan can reach, and which is *also* an incomplete picture: step 11 raises `UT` and inhibits
a charge as well as derating one, so "derated" is not everything protection has done. Insert a
step ahead of this one that inhibits rather than derates and the sentence is false with nothing
reddening.

The sentence now reads *"A derate clamps a demand; a charge inhibit stops one. This short is
neither, because there is no demand at all"*. It carries no count, claims nothing about any
other lesson, and states the mechanism the paragraph is actually about — which is the only
version of this sentence whose truth does not depend on the shape of the path around it.

**This is the second time in one slice that two readings of one number turned out to be the
defect**, and the first was in the harness itself.

## The word-numerals this step still hides, named rather than assumed

A ledgered step is where "every number here is tied to something" is said out loud, so the
exceptions belong in writing. The scan finds digits; these are invisible to it:

* *"for the first **sixty** seconds nothing whatever happens"* — the scenario's
  `faults[0].at_s = 60.0`. Hand-checked, and the run confirms the first non-zero current is
  at 60.5 s. A `Tie::Scenario` would carry it the moment the word became a digit.
* *"hold the charge fixed and that warming accounts for **nearly all** of the difference,
  while holding the temperature fixed and taking the charge away adds only about **a
  twentieth** as much"* — the step's most technical sentence is a three-way quantitative
  decomposition with **no digits in it at all**. It is also a counterfactual: neither arm of
  it is a trajectory a reader can produce, so it could not be claimed even if it were
  written in figures. It is the one statement in this step that nothing in the repo checks,
  in either scan, and it is named here because that is the only place it can be.

Four slices running have recorded that word-form self-description rots invisibly. This is
the second slice where the instance is a statement about the world rather than about the
files, and the first where a rewrite was forced by it.

## The perturbation record

Eighteen edits, each run with the panic message captured rather than the exit code alone,
because this suite has twice reddened on the wrong assertion. Every one is red and every one
names the number it was aimed at. Two things worth keeping from the run:

* **Three of the new rules sit in sentences an `[[arm]]` quotes verbatim**, so perturbing the
  prose alone reddens `every_arm_is_instructed_by_its_own_step` *as well*. Those three were
  re-run with the arm's copy of the sentence moved in the same edit, leaving the ledger as
  the only thing that could fail — and it did, naming the button label, the fault count and
  the speed slider in turn.
* **One perturbation was malformed and went green**, and the green was the harness's fault
  rather than a finding: adding an unknown key to a claim is silently ignored by the TOML
  load, so nothing moved. Re-pointed at the claim's `read_at_s` it reddens on the engine,
  which is what it was for. An edit that changes nothing is not a passing test.

## What the ledger looks like now

Seventeen of twenty-four steps scanned whole, 344 numerals, 134 vocabulary rules,
twenty-two arms. **Seven steps left.**

By the last slice's table the next step is `leg-that-is-not-there` at twenty-two, with
`what-protection-costs` and `past-empty` level behind it at twenty-seven. Those numbers come
from an out-of-tree proxy that runs optimistic; re-measure with the instrument before
budgeting. Two things about that queue that are known now rather than guessed:

* `leg-that-is-not-there` is a CC-CV step whose LFP cell never reaches the band, so it runs
  on a constant current under either windowing rule — which is why the CC-CV window fix was
  invisible to it for six slices. Its numbers are not in the part of the page that fix moved.
* `what-protection-costs` is the eleventh lesson and the last step before this one where
  protection derates a demand. The sentence rewritten above now depends on that being true,
  by inspection rather than by assertion, which is a reason to scan it sooner rather than
  later.

## The rule-reach sweep, run rather than asserted

All 134 rules were matched against all 24 lessons before this was committed, and the count of
rules reaching a step they were not written for goes from two to **three** — with the new one
in a category the other two are not.

* Step 3's two scatter rules still reach `what-protection-costs`, where they are right by a
  **coincidence of values**.
* `**Step {n}**` also matches `what-it-cost`, step 21, which tells its reader to *"pause and
  press **Step 1** if you want an exact instant"*. That is right by **identity**: it is the
  same button under the same label, so the rule answers correctly there for the reason it was
  written rather than by accident. It has no effect today — `cover_by_rule` is only called for
  steps in `[ledger].steps` and step 21 is not one — and it will do real work the day that
  step is scanned.

The distinction matters for the standing tally: a rule right by coincidence is a liability that
a re-fit could turn into a wrong green, and a rule right by identity is coverage arriving early.
Counting them together would hide that.

**An earlier draft of this document closed by saying none of the thirteen reaches a second
step.** That sentence was written from the shape of the phrases rather than from a run, and the
sweep falsified it — which is the same defect, in this file, as the two the slice found
elsewhere.

---

> **Correction, 2026-08-20 — the dependency this document recorded was already gone when it
> was written, and two later slices carried it forward as an open hole.**
>
> The paragraph above says of the rewritten sentence: *"`what-protection-costs` is the
> eleventh lesson and the last step before this one where protection derates a demand. The
> sentence rewritten above now depends on that being true, by inspection rather than by
> assertion."* `docs/plans/path-ledger-what-protection-costs.md` repeated it, and
> `docs/plans/path-ledger-leg-that-is-not-there.md` closed with *"the hole the last slice
> opened is still open"*.
>
> Read against the page, it is not a hole. Step 18's third paragraph is
>
> > A derate clamps a demand; a charge inhibit stops one. This short is neither, because
> > there is no demand at all — and that is what the contactor is for.
>
> and its `expect` names no other lesson's protection at all. The sentence *defines* both
> mechanisms and then says this fault is neither. It makes no statement about any other
> step, so no other step can falsify it — which is exactly the property this document says
> the rewrite was for: *"It carries no count, claims nothing about any other lesson."* The
> note recording a residual dependency was describing the draft that had just been deleted.
>
> The lesson is one this file has recorded before in the other direction: **a note about why
> something is still open outlives the thing that closed it.** The cost here was three
> slices carrying a to-do that nothing needed, and one slice's budget nearly spent building
> a check for a sentence that did not want one.
