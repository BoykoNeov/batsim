# The arm that walks next door, and the three numbers it brought back

`docs/plans/path-ledger-dfn-step.md` ledgered step 16 and deleted five of its figures for
want of somewhere to check them. Two of the five are per-step cost ratios and are not coming
back — no trajectory settles a cost. The other three are one sentence:

> at 1 C the two models reach their cut-offs **12 seconds apart in 3484 — 0.34 %**

and its verdict was that this "needs a full hour-long discharge on *two* models, one of which
is another lesson's pack: `run` builds one pack per (step, arm), and an arm must be
instructed by its own step's prose, so no arm on step 15 can exist for a sentence step 16
writes." That slice named the missing capability the **twin arm** and called it "the only
thing standing between the 1 C boundary and a page that states it."

This slice builds it. The sentence is back, with all three numbers, and each of them answers
to a trajectory this suite runs.

## The reading that made it cheap: it is navigation, not a control

Every override an `[[arm]]` had was a control on the page — a box, a checkbox, a slider, a
button. The twin is not one of those, and the mistake available here was to model it as a
second scenario field on the arm, or as a lesson of its own. What the prose actually asks
for is a **walk**:

> set the current to 5.153198 A … press Restart, and run this file to its cut-off; then
> press Back and do the same on the twin

Pressing Back lands on step 15, and `applyStep` reloads *that* scenario and re-dials *that*
lesson's controls. So the arm's new field names a **lesson**, and the split falls out of the
page rather than being chosen: the named lesson supplies the file, the timestep, the ambient
and the BMS, and the arm supplies only what the reader types on top. `Arm::pack_from` is
three lines in `run` and a lookup by id.

Naming a *scenario file* instead was the alternative and it is worse in the way that matters:
nothing in the guided path would let a reader produce that pack, so its numbers would be true
and unreachable — the defect this repo has shipped twice and now has a check for.

Two consequences worth stating, because neither is obvious:

* **`Start::Restart` is forced, and it is a fidelity fence rather than a scoping one.** This
  step's mark is a state of *this* scenario. A reader who walks to step 15 has left it, and
  arrives at a pack that lesson rebuilt and ran to its own mark. There is no position to
  continue from.
* **A twenty-fifth lesson would have been the wrong shape.** This trajectory is not a step in
  the path. Nobody is walked through a 1 C discharge; they are asked to produce one twice, as
  the evidence for one sentence in step 16.

## The other half: a claim's address was incomplete

`Tie::Quoted` named a claim by `(step, quantity)`, and the agreement assert underneath it
refused any quantity two claims on that step answered differently. That was sound only while
a step ran one trajectory. Step 16 now runs three, and `t_at_v_below:2.5` answers **464** on
its own run, **3484** on the 1 C rerun and **3496** on the twin's — so the three sentences
that already quote the 464 would all have gone red together, on an assert about drift, over
something that is not drift at all.

The arm is part of the address now. `None` means *the step's own run*, not "any of them": an
address that matches whatever is lying around is the search-the-file match the whole taxonomy
refuses.

**The migration found a user immediately, and it was not the new sentence.** Step 15's own
cut-off claim is read on its `carries on` arm — its crossing is 560 s past its mark — so the
two rules that quote it (`596 seconds still to run`, `a factor of 2.28`) had to say so. Under
the old "any claim on the step" reading they were resolving off an arm nobody had written
down.

## Which arm decides which number

Four numbers in one sentence, and they deliberately do not all use the same family.

| the number | tied to |
| --- | --- |
| `3484` | a claim on the `one c` arm |
| `3496` | a claim on the `the twin at one c` arm |
| `58` | `Accounted::Shown` — the `sim time` row the twin's claim asserts prints `58m` |
| `12` | `Tie::Difference` of those two claims, addressed by arm |
| `0.34` | `Tie::Derived`, the sentence's own `12` over its own `3484`, as a percentage |

`12` is tie-side rather than read off the page, and that is the choice worth defending: the
sentence *does* print both operands, which is normally `Tie::Derived`'s case. But what
separates 3496 from 3484 is **which trajectory each was read on**, and only an address can
say that. A derivation reading the two printed instants would be right about the arithmetic
and silent about the packs.

`0.34` then goes the other way, and it is admissible *because* `12` is tie-side:
`operand_value` refuses an operand whose only accounting is another derivation. Re-spell the
`12` rule as a `Tie::Derived` and this one goes red rather than quietly resting on a chain
with no floor. The two rules are coupled in the fail-toward-red direction on purpose.

The claim literals are short — `"This model crosses at **3484 s**"` and
`"the twin at **3496 s**"` — so that `12` and `0.34` fall outside them. A claimed literal
that reached either would trip the ledger's double-cover panic, which is the check that
refuses two readings of one number.

## What it costs, measured rather than estimated

Two trajectories of about 1750 steps each at `dt = 2`, one of them on the most expensive cell
model in the engine. On `every_claim_matches_the_engine`, which is the run everybody does:

* **22.64 s before, 30.82 s after.**

The whole-suite wall clock moves much less (30.6 s against 31.5 s) because the tests run in
parallel and this one was already the longest; the honest number is the one above. Each arm
runs 20 s past its claim on the precedent step 15's `carries on` set, and no further —
past the cut-off a DFN step gets dearer as the solve stops converging, which is this step's
own subject two paragraphs earlier.

## What did not come back

* **`140×` and nearly `500×`**, the per-step cost ratios. Unchanged: no trajectory settles a
  cost, and a timing-based check is not a check this file can hold.
* **`3.798 V`**, the equivalent circuit's zero-length probe at this current. It is *reachable
  in principle* now — it is a third pack, and `pack_from` is how a step reaches one — but no
  lesson in the path runs an equivalent-circuit LG M50 at 15.459594 A, so there is nowhere to
  walk to. It would also lean on the probe fix below, which has no user yet. Left as it
  stands: two numbers and a direction.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| the 1 C claim's `value` → `3486` | `every_claim_matches_the_engine` |
| prose `crosses at **3484 s**` → `3485`, leaving the literal alone | `every_claim_appears_in_its_own_step` |
| the twin claim's `value` → `3498` | `every_claim_matches_the_engine` |
| the twin arm's `pack_from` line deleted | the twin claim — the pack becomes this step's DFN and answers 3484 |
| `run`'s `pack_lesson_of` call removed | the same, from the runner's side |
| the twin arm's `pack_from` → `the-gradient-itself` | **nothing — a registered GREEN.** That lesson is on step 15's scenario file, so it is the same pack; `pack_from` names a lesson and two lessons on one file are interchangeable to it |
| prose `12 s apart` → `13` | the `Difference` rule |
| that rule's two operands swapped | the same rule, on the sign |
| that rule's twin operand `arm` → `Some("one c")` | the difference resolves to 0 |
| that rule's twin operand `arm` → `None` | it resolves to this step's *own-run* crossing, 464 |
| the `12` rule re-spelled as a `Tie::Derived` over the printed instants | the `0.34` rule, on `operand_value`'s floor |
| prose `which is 0.34 %` → `0.35` | the `Derived` ratio |
| that rule's two operands swapped | the same rule, at 290 |
| `Tie::Quoted`'s arm filter neutered | every rule quoting step 16's `t_at_v_below:2.5`, on the agreement assert |
| step 15's cut-off quotations put back to `arm: None` | those two rules, resolving to nothing |
| the twin arm's `to_s` → `3490` | `every_claim_is_reachable_in_its_own_step` |
| `assert_walkable`'s body emptied | the three `should_panic` tests |
| the probe's `probe_prog` reverted to the lesson's demand | **nothing — a registered GREEN**, which is what prices it |
| the `[ledger]` entry's numeral count off by one | `every_count_beside_a_ledger_entry_is_derived` |
| prose *and* literal `58m` → `59m`, `shows` left at `58m` | the ledger, on `Accounted::Shown` — added after the first run, with the sentence |
| the twin claim's `shows` → `59m` | the display half of `every_claim_matches_the_engine` |

### What the table found

**All nineteen came back as registered** — seventeen red on the check they were written
against, two green. Five rows are worth reading twice.

* **The two `pack_from` rows redden from opposite ends and both land on the twin claim.**
  Deleting the field from the arm and deleting the lookup from `run` are the same defect
  written in two files, and either way the pack falls back to this step's porous-electrode
  model and answers 3484 where the claim says 3496. That is the only place the new field is
  observably load-bearing, and it is worth naming that it *is* only one place: the timestep,
  the ambient and the BMS also come from the named lesson, and the two lessons agree on all
  three, so no perturbation can tell which one those were read from.

* **Pointing the twin arm at a different lesson on the same file changes nothing, as
  registered.** `the-gradient-itself` is step 17 and sits on `cc_discharge_3c_spm.toml`, the
  same file step 15 uses, so it is the same pack under a different name. `pack_from` names a
  lesson because that is what a reader walks to; what it *resolves* to is a scenario, and two
  lessons on one scenario are interchangeable to it. This is the honest reading of the
  `assert_ne!` on `scenario` in `assert_walkable`: that fence refuses naming a lesson on
  **this step's** file, not on any other shared one.

* **Re-spelling the `12 s` rule as a sentence-side derivation reddens the `0.34 %` rule, not
  itself.** Both spellings of the difference are arithmetically right, so the row that fails
  is the one downstream — `operand_value`'s refusal of an operand accounted only by another
  derivation. The coupling was built deliberately in that direction and this is the evidence
  it fires rather than an argument that it would.

* **Neutering the arm filter reddens the ledger on the agreement assert**, which is exactly
  what would have happened to the three existing sentences quoting step 16's 464 if the
  address had not grown. The failure and the reason for the slice are the same event.

* **Emptying `assert_walkable` reddens all three `should_panic` tests and nothing else**,
  which is the whole point of extracting it. Those three fences are unreachable from the
  claims file — the one twin arm satisfies all of them — so without the tests they would be
  three paragraphs asserting a behaviour no run enters, which is the shape this file has
  already been caught by twice.

**The last two rows were added after the run, with the sentence they are about, and both
redden on more than they were registered for.** Moving the prose's `58m` while leaving the
claim's `shows` at `58m` fails the ledger *and* the `quoted` check, because that flag asserts
the prose contains the row string; moving `shows` instead fails the display comparison as
well. Three checks hold one four-character string, which is more than the number is worth and
exactly what it should be.

**And the registered green on the probe is only evidence as half of a pair.** `run`'s probe
now takes the arm's typed current on a restart arm rather than the step's, which is what the
page does after Restart; reverting it leaves the whole suite green, because no claim reads a
probe on a restart arm that types a current. Written the correct way round rather than the
reachable way round, on the same terms as the ambient split two slices ago. The claim that
would reach it is a `probe = true` reading on one of the two new arms, and the `3.798 V`
sentence above is the one that would want it.

## Learned while building

**A prose perturbation on a claimed sentence does not test the claim.** The first registration
read "prose `3484` → `3485` | the 1 C claim's value check", and it is wrong: a claim's
`literal` is a substring of the prose, so editing the prose alone makes the literal *absent*
and `every_claim_appears_in_its_own_step` fires first. The value check never runs. Reaching
the value needs the edit on the **file** side — the claim's `value`, or its `spells`. Both
rows are in the table now, and they fail on different checks. Registering the wrong check is
cheap to fix before the run and invisible after it, which is the argument for writing the
table down first.

**A sentence very nearly lost a fact because a number looked unaccountable, and that is the
check shaping the prose.** The two crossings are 12 s apart and the panel renders both as
`58m` — the clock cannot tell them apart, which is worth a reader's attention in a step whose
subject is a model that gives you no sign it is wrong. The first draft had that sentence,
then cut it, on the reasoning that a `58` had no arm. It has one: `Accounted::Shown` accounts
a number sitting inside a claim's own `shows` string, and both new claims already carried
`shows = "58m"` for their own reasons. What it wanted was **positional** — the row string has
to be inside a claim's *literal* — so the fix was to reorder the sentence and hang the clause
off the twin's claim, not to delete it.

This is the inversion of the move `docs/plans/path-ledger-dfn-step.md` refused. That slice
declined to spell five measurements in words to clear the scan, because "manufacturing five
new instances to satisfy the check turns the check into a fact about the author". Deleting a
true sentence to clear the scan is the same defect wearing the opposite sign — and it is
harder to notice, because what it leaves behind is green. **When a number in a sentence you
want to write has no arm, check the arms before you check the sentence.** Step 15 already
prints its own clock rendering in prose ("in the eighteen minutes the clock will be showing"),
which is the precedent that should have made this obvious.

**An address is not complete until the thing it addresses can answer twice.** `Tie::Quoted`
was `(step, quantity)` for six slices and nothing was wrong with it, because no step ran two
trajectories that both measured the same quantity. The defect did not exist until this slice
created it — and the fix, once the field was added, immediately turned up a *pre-existing*
misuse next door: step 15's cut-off quotations were resolving off an arm the rule never
named. **A widening that exposes an older sloppiness is worth more than the widening.**

**The scenario file states one of these numbers a second time and still nothing measures it.**
`cc_discharge_3c_dfn.toml`'s `description` field says "At 1 C the two agree on the cut-off to
0.34 %". `docs/plans/path-ledger-dfn-step.md` refused a `Tie::Name` arm that would have
accounted the page's `0.34` against that string, on the grounds that a file may declare an
identity but not a measurement. That refusal stands and is now better founded: the page's copy
answers to two trajectories, and the scenario's copy answers to whoever last typed it. Two
copies of one number, one of them checked, is the right asymmetry — the wrong fix would have
been to tie them to each other.
