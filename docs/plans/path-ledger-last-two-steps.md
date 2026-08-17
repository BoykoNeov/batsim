# The last two lead-acid steps, scanned whole

`docs/plans/path-ledger-sixth-step.md` closed with a nomination: steps 23 and 24 "share
this step's scenario file and their headline ratios are exactly the sibling-derived shape
the new arm was built for. They are the cheapest next steps to ledger *because* of this
slice." This is that slice, and the nomination was half right. The arms were reusable; the
prose was not sound.

**The finding is in step 23 and it was there before the ledger touched it.** The sentence

> `6.09 W` here against `0.07 W` at the same state of charge on the last step, 87 times as
> much from the same cell

is wrong twice over, and the two errors were pulling in opposite directions so neither was
visible from the other. Step 22's `0.07 W` is read **at its own cut-off**, where the cell is
at 3.3 % charge — not at 38.6 %, which is where step 23 stops. Measured, step 22's heat at
38.6 % charge is **0.0031 W**, so "at the same state of charge" names a number 24 times
smaller than the one quoted. And the `87` is the two *printed* figures divided
(6.09 / 0.07); the values underneath them divide to **81.6**, because `0.07 W` is a rounded
0.0746 and at two decimal places the rounding is most of the answer.

Both halves were asserted three times over — the sentence, step 23's claim note, and step
22's claim note ("Here so that step 23's 6.09 W has something to be 87 times") — and the
whole suite was green, because nothing in the file compares two steps' numbers to each
other. That is the gap this slice closes with `Tie::Quoted`.

## The two steps

`sixty-times-the-current` (step 23) prints **23 numerals**, `and-it-is-still-in-there`
(step 24) prints **26**. Ledger 6 → 8 steps of 24, 58 → 107 scanned numerals, claims
178 → 186, ledger arms 12 → 14. Both counts above were guessed wrong when this file was
written (22 and 28) and the file's own derived per-step tallies said so on the first run —
which is that machinery doing exactly what the slice before it built it for.

Both run `cc_discharge_pba.toml`, which is step 22's file — the third and fourth lessons on
one scenario, with the demand box and the demand *mode* the only differences. That is what
makes step 23's cross-step sentences ("21.6 A instead of 0.36", "reached in `12m` instead of
`19.3h`") the natural shape here and what makes them unaccountable by every arm the ledger
had: each one names a number the step **next door** decides.

## Four new arms, and why each one

### `Tie::Elsewhere { step, tie }` — the tie evaluated against another lesson

`0.36` is step 22's demand box; `19.3` is step 22's clock. Neither is a fact about step 23
at all, and both are exactly the facts `Tie::Setting` and `Tie::Clock` already read — about
the wrong lesson. So the arm is a *wrapper* rather than two more flat arms: it resolves the
named lesson, its scenario and its chemistry, and recurses.

Fenced three ways, all refusals rather than resolutions:

* **No nesting.** `Elsewhere` inside `Elsewhere` has no floor and no sentence needs one.
* **Not its own step.** A wrapper naming the step it sits in is `Tie::Setting` with extra
  words, and green for the wrong reason.
* **No inner `Derived`.** That arm reads *this* sentence's siblings, and there is no
  coherent reading of "the sibling of a token in a different lesson".

**Its blind spot is narrower than this file first claimed, and measuring it is what showed
that.** The wrapper hands its inner tie three things — the lesson block, the scenario and the
chemistry — and only the last two are blind. The first draft here said the whole resolution
was untested because both users name a step on their own scenario file; that is true of the
files and false of the *block*. Neutering the lesson argument reddens the ledger by itself,
because step 22's clock and demand box are not step 23's however much file they share.

The files really are unreachable, and the perturbation says so precisely: making the wrapper
resolve against the **calling** step's scenario and chemistry leaves the entire suite green
except for the one test written for it. So that test reads two ties, not one — a control off
the named lesson's block, and a chemistry field off the named lesson's file (LFP's
2.303451 Ah, from a wrapper invoked on a lead-acid step).

### `Tie::Quoted { step, quantity }` — the value a claim on another step pins

`0.07`, `6.9620` and (after the repair) `0.0746` are all numbers **step 22 measured**. A
constant can be tied to a file; a measurement cannot, and step 23's prose is not the place
to re-measure step 22's cell. So the tie is to the *claim*: the named step's claim on the
named quantity, whose own value is checked against the engine by check 7 where it lives.

Named by `(step, quantity)` and never by step alone. "Some claim on step 22 spells 0.07"
is the search-the-file match `Tie::Name`'s prefix and `Accounted::Setting`'s trajectory tie
both exist to refuse.

**Compared at the prose's own precision**, like every computed tie, and that is what lets
one arm carry both `0.07` (two places) and `0.0746` (four) off the same claim value. The
rule's `pow10` still applies, so a claim whose own `spells_pow10` is non-zero is refused
rather than double-scaled.

### `Tie::Derived` grows an operation

The sixth arm shipped product-only, with a stated price: "the day a ledgered step derives a
number by another operation, this grows an `op` the way check 6's `[[derived]]` carries
one." Both other operations arrive in the same slice:

* `2.5` = `6.9620 − 4.4190`, the amp-hours left in the cell — a **difference**.
* `87` = `6.09 / 0.07`, the two panel readings divided — a **quotient**.
* `82` = `6.09 / 0.0746`, the same comparison on the values — a quotient again.
* `32.2` = `1.4220 / 4.4190`, leg two against leg one — a quotient with a `pow10`.

**Operand order becomes load-bearing** where a product did not care, so a reversed-order
edit is in the perturbation table below.

### `Control::Ambient` — the fourth control

Step 23 holds everything but the demand box, and says so: "same chemistry, same 1S1P, same
25 °C, same half-second step". The `25` is the ambient slider. `Accounted::Setting` on the
claims side reads an ambient **step** (the change a reader dials in at the mark) and its
docs say a sentence printing a *level* "still fails here loudly, because nothing in the path
prints one". This is that sentence, on the other scan, and it prints a level — so the
ledger's control enum grows the fourth member its own docs reserved. The `dt` beside it is
spelled "half-second" and stays invisible, which is the digit scanner's standing limit.

## Three new quantities, and one lever they share

Step 23 and step 24 both take the `overpotential` figure apart:

* `overpotential_mv_at` — the cell's total, `CellView::overpotential_v` in millivolts.
* `rc_overpotential_mv_at` — the placeholder RC pair alone.
* `diffusion_overpotential_mv_at` — the fitted term alone, which is the difference.

The RC half has **no public accessor**: `CellView` carries the total and `EcmState::v_rc` is
reachable only through `Pack::snapshot()`, which is `Serialize`. So the row reads it out of
the serialised snapshot — no `sim-core` change, no wire-contract bump, no version bump.

**That read costs a whole chemistry per row**, which is what decided the design. The
snapshot's JSON carries the parameter set as well as the state — three kilobytes of tables —
and step 22 takes 139 241 steps, so a speculative read on every row is not affordable at any
pack size. It is therefore fenced the way `Row::rest_v` is fenced to leg boundaries, one
notch stricter: `rest_v` is taken wherever it *could* be read, and this is taken only where
it *is*. The value check gathers the instants its claims name before the run starts and
hands them to `run`; every other row carries `None`, and asking for one there panics rather
than falling back to zero — which would read as "the RC pair is spent", a real state this
quantity is used to distinguish.

The first design was a pack-size fence (1S1P only), and it was wrong for a reason worth
keeping: the cost is not per *cell*, it is per *row*, because the chemistry is serialised
whichever pack it belongs to. Measuring the JSON before writing the fence is what showed it.

Two more quantities, both about a pulse train's second leg:

* `leg_s_at_v_below:<volts>` — how far into its own leg the terminal first fell below
  `volts`. Step 24's `237.5 s` is that, and no existing frame reaches it: `since_mark` is
  zero here (the crossing *is* the mark) and `t_at_v_below` answers 737 s, because leg one
  crossed first.
* `leg_delivered_ah` — the charge delivered since that leg began. Step 24's `1.4220 A·h`.

The leg's origin is read off `Row::rest_v`, which is `Some` exactly on a pulse train's leg
boundaries and nowhere else, so neither quantity needs the program declared a second time.

## The third defect, which only building the quantity could find

The prose said leg two delivered **1.4250 A·h**, and the number was arrived at by
subtraction: the run's total amp-hours less the figure step 23 claims. That difference
carries the step ending at 737.0 s — leg **one's** last loaded step, which leg one's own
figure excludes under the marked-step bracket this file has used since the lead-acid
claims were written. Counting leg two's own steps under the same bracket gives **1.4220**.

The gap is one step's charge, 0.003 A·h, and the fraction beside it rounds to 32.2 % either
way. That is what makes it the kind of number that goes stale unnoticed: nothing else in the
sentence moves with it. It was found because `leg_delivered_ah` counts the leg rather than
subtracting two whole-run figures, so the two ways of getting the number stopped agreeing.

## The repairs

**Step 23's heat sentence.** "At the same state of charge" is deleted — it was false — and
the comparison is stated as what it is, each cell at its own cut-off. Both ratios are kept
and the gap between them is the point:

> `6.09 W` here against `0.07 W` where the last step stopped. Divide those two and you get
> 87; the honest figure is **82**, because `0.07 W` is a rounded 0.0746 and two decimal
> places is most of the difference at that size.

That is four accounted numbers where there were three, and it teaches the rounding rather
than hiding behind it. Both claim notes are corrected with it.

**Step 23's `184.29 mV showing`.** The pack grid's tile legend renders overpotential at one
decimal place (`METRICS.overpotential_v`, `dp: 1`); the two-decimal string the prose quotes
is the *pinned-cell* line, which needs a click, on a panel this step does not even list in
`watch`. "Showing" is dropped. The two claims are value-only for the same reason: `display`
names a `READOUTS` row and neither of these is one.

**Step 24's `32.2 %`.** The sentence divides leg two by leg one and prints only the
quotient, so the denominator is in step 23's prose rather than its own — out of reach of a
sibling operand, which reads one sentence. The prose prints it: "against the first leg's
4.4190". Now the quotient is two siblings, and the `4.4190` is a `Quoted` of step 23.

## Perturbations, registered before the run

| edit | must redden |
| --- | --- |
| prose `21.6 A instead of 0.36` → `0.35` | the `Elsewhere` wrapper, on step 22's demand box |
| prose `19.3h` → `19.4h` | the `Elsewhere` wrapper, on step 22's clock |
| the `Elsewhere` wrapper's step → `bare-curve` | both of the above, off an LFP lesson's files |
| step 22's `demand.value` → 0.35 | the same wrapper, from the file side |
| prose `0.0746` → `0.0747` | `Quoted`, at four places |
| prose `0.07 W` → `0.08 W` | `Quoted`, at two places off the same claim |
| step 22's heat claim `value` → 0.08 | `Quoted` from the claim side, and check 7 with it |
| prose `82` → `81` | the quotient, on values |
| prose `87` → `86` | the quotient, on printed figures |
| the `82` rule's operands reversed | the quotient's operand order |
| prose `2.5 A·h` → `2.6` | the difference |
| the `2.5` rule's operands reversed | the difference's operand order |
| prose `25 °C` → `24 °C` | `Control::Ambient` |
| prose `3 C` → `4 C` | the chemistry's `max_discharge_c` |
| prose `12m` → `13m` | the clock, on this step's own mark |
| prose `82.39 mV` → `82.40` | the RC-half claim |
| prose `184.29 mV` → `184.30` | the total-overpotential claim |
| prose `47.66 mV` → `47.67` | the diffusion-half claim |
| prose `237.5 s` → `238.0 s` | the leg-relative crossing claim |
| prose `1.4220 A·h` → `1.4260` | the leg-relative delivered claim |
| prose `2.005 V` → `2.006 V` | the one-hour rest claim |
| prose `0.000 A` → `0.001 A` | the rest-current claim |
| prose `step 23` → `step 22` in step 24 | the ordinal |
| the `32.2 %` claim's denominator sentence deleted | the quotient, on an operand nothing accounts for |
| the `Elsewhere` wrapper reads the CALLING step's scenario and chemistry | **only** its own test — the whole suite stays green |
| the own-step fence neutered | that fence's `should_panic` test |
| the nesting fence neutered | that fence's `should_panic` test |

`3 C` is tied to the chemistry's `max_discharge_c` and **not** to the ratio of the demand box
to the capacity, and the choice is not arbitrary. Both readings are true of this sentence.
The ratio's unique failure — the rating moving while the box and the capacity stay put —
would leave "the top of what this cell is rated for" false with nothing anywhere in the repo
pinning that field, where the chemistry reading's unique failure (the box moving) is already
caught by the `21.6` in the sentence before it. `Tie::Ratio` therefore stays at one user, and
that is recorded rather than fixed.

## What the perturbations said

**Thirty-one cases, thirty-one red, every one on the check named in advance.** Twenty-five
edit the prose or the claims file and need no rebuild; six edit the Rust and pay one each.
Three of them were re-run after the `Trace` refactor that clippy's argument limit forced,
because that refactor touched `drive` and `leg_start_s` — the code both new leg quantities
stand on — and a perturbation table measured against an earlier binary is a table about an
earlier binary.

Three are worth naming.

**Moving step 22's own demand box reddens step 23 too**, which is the `Elsewhere` wrapper
seen from the file side rather than the prose side — and it also reddens step 22's own value
check, because that box decides its trajectory. Two checks, one edit, and the pair is what
says the wrapper is reading a live field rather than a copy of one.

**Moving step 22's heat claim `value` reddens step 23's ledger** — the case `Tie::Quoted`
exists for. Before this slice that edit could not have reached step 23 at all; the whole
defect this file opens with survived six slices in that gap.

**One case is green everywhere but its own test, and that is the result.** Making the
wrapper resolve its inner tie against the *calling* step's scenario and chemistry leaves all
twenty-three other checks passing — which is the measurement behind the blind-spot paragraph
above, and the reason `an_elsewhere_reads_the_named_lessons_own_files` reads a chemistry field
as well as a control.

**Deleting the `4.4190` clause is confounded and is recorded as such.** It reddens the
ledger, as predicted, on `32.2`'s operand resolving to nothing — but it also reddens both
self-count checks, because deleting a number changes how many numerals step 24 prints. The
case is evidence that the operand fence is reachable, not that it is the *only* thing
reached. That is the same shape `docs/plans/path-ledger-sixth-step.md` had to build a
dedicated `#[should_panic]` test for, and the two `Elsewhere` fences here have theirs for the
same reason.

**Both new `should_panic` tests were hand-validated by neutering their asserts**, on this
repo's standing rule: a `should_panic` test that has never been seen to fail is not evidence
of anything.

## Learned while building

**A number that is right about the panel can be wrong about the cell, and the honest fix
prints both.** `6.09 / 0.07 = 87` is exactly what a reader gets by dividing the two rows in
front of them, and `81.6` is what the cell actually does. Neither number can simply replace
the other: print 82 alone and the reader's own arithmetic contradicts the page; print 87
alone and the page is wrong about the physics by 7 %. So the sentence prints both and says
which is which, and the rounding becomes something the step teaches instead of something it
hides. Two decimal places on a 0.0746 W reading is most of the answer at that size.

**Guessing a count and letting the file correct you is now cheaper than counting.** This plan
was written with 22 and 28 numerals for the two steps; the derived per-step tallies said 23
and 26 on the first run, in seconds. That machinery was built one slice ago for exactly this
and it paid for itself immediately.

**Measure the cost before designing the fence around it.** The RC-pair read was first fenced
to single-cell packs, on the reasoning that a per-step snapshot is affordable on one cell.
Printing the snapshot's JSON showed it carries the whole chemistry — tables and all — so the
cost is per *row*, not per cell, and a 1S1P step taking 139 241 rows is the worst case rather
than the safe one. The fence that survives is "only at the instants a claim named", which the
value check can supply because it knows its claims before it starts the run.

**A blind spot is a claim about the code, so it needs a perturbation like any other.** This
file asserted that `Tie::Elsewhere`'s whole resolution was untested because both its users
name a step on their own scenario file. Half of that was wrong — the lesson *block* differs
between the two steps and the ledger catches a wrapper that reads the wrong one. Only the
scenario and chemistry are genuinely unreachable, and the perturbation that proves it is the
one that leaves the entire suite green except the test written for it.

## Deferred, with a price

* **`Tie::Quoted` reads a claim's `value`, not its `spells`.** A claim whose prose rounds one
  way and whose value rounds another would be quoted at the value's rounding. Nothing in the
  path does this today and the alternative — quoting the *token* — would make the tie a
  string comparison against prose rather than against a measured number.
* **`Tie::Elsewhere` composes with three inner arms and is used with two.** `Setting` and
  `Clock` have users; `Scenario`, `Chemistry`, `Member`, `Span`, `Name` and `Ordinal` would
  all resolve and none is named by a sentence yet.
* **Sixteen of twenty-four steps are still claims-only**, and the two dense DFN/SPM steps
  (13, 16) are the next expensive ones: their numbers are nearly all measurements, so
  ledgering them is a claim-writing slice rather than an arm-building one.
