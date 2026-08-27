# The second step, ledgered

Step 2 — `same-discharge-other-chemistry` — is the twenty-third of twenty-four lessons the
prose ledger now scans whole. It was the last of the two the previous slice left, and the one
it left deliberately: `docs/plans/path-third-cell.md` built the arm that loads a third
chemistry from the picker, corrected four numbers on it, and then stopped, because the rules
those numbers needed could not exist until the step was ledgered.

Twenty-three of twenty-four steps are ledgered. `the-gradient-itself` is the one that is not.

## What was owed, and what it cost

Measured first with the ledger's own scan, as every slice since the particle step has: **32
unaccounted of 41**, which is exactly what the previous slice recorded. Four slices running
where the recorded sizing was right on an instrument that takes under a minute.

Closed by **five claims, sixteen rules, one arm, one new tie and the file half of an old
one**. The step now prints 43 numbers — two more than it did, both of them words that became
digits — and 29 of them answer to a rule, 14 to a claim.

| what it needed | how many | what closed it |
| --- | --- | --- |
| this step's and step 1's nameplates, boxes and floors | 10 | ordinary `Chemistry` / `Setting` rules, half of them through `Tie::Elsewhere` |
| the two rates, and step 1's | 4 | `Tie::Ratio` over a box and a nameplate |
| ordinals pointing at steps 1 and 20 | 5 | `Tie::Ordinal` |
| the third cell's file name, twice | 2 | **`Tie::Picker`**, new |
| its nameplate and the parameter set behind its curve | 2 | **the file half of `Tie::OnArm`**, new |
| the current typed on that arm, and the rate it makes | 2 | `Tie::OnArm` over a control, and a ratio of two of them |
| four subtractions the sentences state | 4 | `Tie::Derived` and `Tie::Difference` over claims |
| the window the fall is measured across, the fall, and the deficit at the mark | 4 | new claims |
| the same fall on step 1's cell | 1 | a new arm — the third `pack_from` in the file |

## The two new pieces of machinery

**`Tie::Picker`** reads the digits after a prefix inside the name of the file an arm loads
from the picker, which is what accounts for the `50` of `cc_discharge_lgm50`. It is
[`Tie::Name`]'s third sibling — `Name` reads a chemistry's name, `Label` reads a control's,
this reads a file's — and it is honest to say it guarantees nothing new: `assert_picker`'s
third fence already requires the file's stem to appear in the arm's instruction, and an
instruction is a verbatim substring of the prose. Rename the file and the arm fails there
either way. What this adds is that the *ledger* names the right decider instead of leaving
two digits unaccounted.

**`Tie::OnArm` may now read a file.** It refused one outright, on a stated argument:

> an arm overrides controls, not files: asking a scenario field under an arm's name would
> resolve to the same number and claim it came from somewhere else.

That is exactly right for every arm that changes only controls, and false for one that
carries `Arm::scenario` — the picked file really is the arm's, and step 2's third cell prints
its nameplate (`5.153198 Ah`) and the provenance of its curve (`Chen2020`), neither of which
is anywhere in `cc_discharge_nmc`. So the refusal was **narrowed, not dropped**, and it still
fires word for word on an arm that picks nothing. Three fences now, each with a `should_panic`
test because a rule is code and none of them is reachable from the claims file:

* a file tie under an arm with no `scenario` — the original refusal, kept
  (`an_on_arm_may_not_read_a_file_off_an_arm_that_picks_none`, renamed from
  `an_on_arm_may_only_wrap_a_setting`, which had stopped describing what it asserts)
* anything that is neither a control nor one of the three file ties
  (`an_on_arm_may_not_wrap_anything_else`) — an `Ordinal` or an `Elsewhere` under an arm's
  name resolves against the *step's* lesson while reading as the arm's, which is the
  misattribution the original refusal was about
* a `Picker` pointed at an arm that walks rather than picks
  (`a_picker_tie_needs_an_arm_that_picks_a_file`)

## The arm that needed a sentence written for it

`168 mV` — the LFP cell's fall across the same window — is the number the previous slice
called the expensive one, and it was right. Step 1's prose never states its own 90-to-20
fall, so there was no claim to quote; and a claim on step 2 is measured on step 2's
trajectory. The only thing that reaches it is an arm that walks next door
(`pack_from = "bare-curve"`), and an arm needs an instruction sentence in *this* step's
prose, which this step did not have.

The alternative was named in advance: delete the comparison. That was refused on its merits —
*"That ratio is the whole difference"* is the paragraph's spine, and 481 against 168 is what
it means. So the sentence was written:

> …where the LFP cell fell 168 across the same window; press Back and run the first lesson
> again to its own mark if you would rather read that off the panel than take it on trust.

`path-third-cell.md` warned that "a sentence invented to satisfy an arm is the same defect as
a number invented to fill a table", and the warning is worth answering rather than waving
past. What keeps this one honest is that it tells a reader how to see a figure the step
already gives them, and the gesture — go back one lesson and run it — is one the path asks
for elsewhere. It is not a measurement performed for the harness's benefit.

The arm itself is the plainest in the file: no overrides at all. Everything is `bare-curve`'s,
and `assert_walkable`'s fourth fence is what makes that unambiguous, since the two lessons
agree about every control neither the arm nor the sentence touches.

## Five numbers moved, and two of them were words

**`2.303` was a rounded constant.** Same token step 16 was caught by and step 1 had already
paid: a constant in prose either is the file's number or is wrong about it, so it is compared
exactly. The sentence now prints `2.303451`, which is what step 1's own prose has printed
since *it* was ledgered.

**`1.111` was printed against the wrong instant — and the first repair moved it to the wrong
subject.** The sentence read:

> Watch the new `past empty` readout come off zero at the same instant the flag appears:
> 1.111 points of charge taken out of a cell that had none.

The colon puts the number at the flag. At the flag the deficit is **0.0037 points** — three
ten-thousandths of what the sentence prints. 1.111 is the figure at the mark, 46 s later. That
is the same family as "right but unreachable", which this repo has shipped three times: a true
number attached to a moment it is not true of.

The repair said *"by the mark **it reads** 1.111 points"*, and that was the same defect one
step sideways. "It" is the `past empty` row, and this file's own header names that row as one
of two that cannot be claimed at all: it is formatted from per-cell state and sampled on a
250 ms **wall**-clock throttle, so at this step's 800× there is no such thing as what it shows
at a given simulation time. The number is the cell's and not the panel's. The sentence now
says so — *"by the mark the cell is 1.111 points past empty"* — which is what step 21 already
paid for once, where a number **left the page** for being a fact about a wall clock rather
than about the simulation.

Worth naming because nothing in the harness would have caught it: the claim behind the
sentence is `deficit_pts_at`, which is ground truth, and a claim on that quantity is forbidden
a `display` half. A sentence that describes a row while its claim reads the engine is invisible
to every check here.

**`53` could not be checked as written, and the honest figure is `53.5`.** It is step 1's
mark less step 1's empty-time, and a computed tie is compared at the prose's own precision —
so `53` fails, because 53.5 rounds to 54. Step 1's own claim note already admits its `53` is a
truncation with zero margin that "could equally say 54". The half is what a sentence quoting a
subtraction owes it.

**And two hedges were spelled in letters, where no scanner in this file can see them.**
`written_numbers` finds digits only, so *"Both empty within eight seconds of each other"* and
*"where step 1's flat cell managed eight and a half"* were outside every check the ledger runs
— true, and invisible. They are now `7.5 s` and `8.5 s`, each tied to a subtraction of two
numbers the same paragraph claims. That is the third time this file has been bitten by a word
numeral; the first was false ("seven steps, not eight"), the second was a stale self-count,
and these two were merely unwatched.

## Perturbations

Ten applied to the green tree and reverted, each reported by the **names of the tests that
reddened** rather than by an exit code — a red for the wrong reason is a defect this repo has
shipped.

| what was changed | what went red |
| --- | --- |
| the third-cell rule reads this step's chemistry, not the picked file's | the ledger scan: `3.0` against a sentence printing `5.153198` |
| the same rule asks a file field of `the first cell again`, which picks nothing | the ledger scan, on the fence — **not** a fallback to the step's own number |
| `cc_discharge_lgm50` → `lgm60` in the prose | the instruction fence **and** `Tie::Picker` |
| the 42.5 s difference takes its two flag times the other way round | the ledger scan: −42.5 |
| the twin arm walks to step 8 instead of step 1 | the instruction fence and the engine check — the 168 mV really is step 1's pack |
| the 1.111 claim is read at the flag instead of at the mark | the engine check: 0.0037 against 1.111 |
| `2.303451` rounded back to `2.303` | the ledger scan — a constant is compared exactly |
| `7.5 s` → `9 s` | the ledger scan |
| `8.5 s` → `9.5 s` | the ledger scan |
| the 481 mV claim moved one millivolt | the engine check and check 5 |

### Two of them were surprises, and both are worth keeping

**`7.5 s` → `8 s` is GREEN, and that is correct.** A computed tie is compared at the precision
the prose commits to, and `to_fixed(7.5, 0)` is `"8"`. So the sentence is free to print the
round number and still be checked; printing the half is a tighter statement the author chose.
`9` reddens, which is what makes the arm live. This is not a hole — it is the rounding rule
the whole taxonomy rests on, met at a place where a reader might mistake precision for
accuracy.

**Deleting the `53.5` clause is RED, on three independent checks.** The plan expected green,
on the standing lesson that "deleting a true sentence to clear the scan is the same defect as
inventing one". It is no longer available here, and the reasons are worth naming, because they
are the defences this file has grown since that lesson was written:

* `every_ledger_rule_is_a_phrase_and_is_used` — the rule written for that clause now matches
  nothing
* `every_count_beside_a_ledger_entry_is_derived` — the step's numeral count in `[ledger]`
  moves
* `every_count_these_files_state_about_themselves_is_derived` — and so does the file's own
  total

A narrower deletion that removes only the quantity and leaves its neighbours still reddens the
last three. The escape route is closed for any sentence a rule was written for.

## Two things found beside the work

**The arm list in the claims file was five arms short of its own derived count.** The count
(`n_ledger_arms`, twenty-four with `Tie::Picker`) has been derived and checked for two slices;
the *prose list* beside it never was, and it had been quietly missing the control label, the
array length, the open-circuit read, the magnitude wrapper and `OnArm` itself. The same list in
this test's module docs was four short. Both are now complete — but nothing keeps them so, and
that is the shape `docs/plans/path-self-counts.md` recorded as "a count of a list caught a
missing list entry". A tally that pinned the list's *length* rather than the sentence's number
would close it; it is not built here.

**Step 2 still carries two word numerals nothing reads**: *"two steps from now"* and *"in
exactly one field"*. Both are true, and both are outside every scan in this file for the same
reason the two that were fixed were. They sit on nothing a subtraction of claimed numbers could
answer, which is why they were left rather than converted — said here rather than left to a
green ledger to imply.

**And one arm of one rule is unfalsifiable at this step, by coincidence.** The 53.5 s rule
reads step 1's mark through `Tie::Elsewhere`, and both steps mark at 4200 s — so no
perturbation of that rule can tell the wrapper from a bare `Setting`. What proves the wrapper
swaps the lesson is the rule two above it, whose `Elsewhere` read of step 1's demand box
answers `2` where this step's own says `2.6`: the same line of `tie_values`, asked where the
two lessons disagree. Recorded in the rule's own comment rather than spent as a run.

## What is left

`the-gradient-itself` is the last unledgered step: four claims on the pre-Run probe, and the
rest of its numbers are measurements rather than constants. It is the whole of the remaining
gap, and `[ledger].unledgered` names it with its claim count beside it, derived and checked.

One thing outside this slice was fixed to get past the commit gate: `cargo clippy` had begun
failing on `crates/sim-core/src/dfn.rs` under the current toolchain
(`chunks_exact_to_as_chunks`, a lint on code this slice does not touch). One line, mechanical,
and the DFN goldens are unmoved.
