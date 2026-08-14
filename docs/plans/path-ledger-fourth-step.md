# The fourth ledgered step, and the four arms it cost

`docs/plans/path-ledger-scenario-arm.md` built the ledger — a scan over a step's *whole*
prose, requiring every numeral in it to be tied to something — and opened it on the three
steps whose every digit is a constant some scenario file already declares. Those three were
free: they could be closed before a single number was measured. Everything after them costs
arms, and this slice is the first instalment.

**What landed: `protection-on` (step 6) is ledgered, and four new accountings exist.** The
ledger goes from three steps and fourteen numerals to **four and twenty-two**. One claim was
added. Nothing else in the path moved.

## Why this step, measured rather than assumed

The obvious next target was step 8, `wearing-out-while-idle` — the last commit of the
previous arc pointed at it by name, because it is one of only two steps whose prose contains
a spaced thousands group (`200 000`, `10 000`) and therefore the only tractable step that
exercises `join_thousands` through the ledger. It is not this step, and the reason is
written in the test suite already:

> `run` keeps ONE environment for the whole trajectory; a mark-side drag needs two, split at
> `until_s`. The sentence that would pay for the split prints `20 K` and `2.7×`, both of
> which are figures derived from their siblings, so it cannot be claimed until that
> accounting arm exists either.
>
> — `every_arm_is_instructed_by_its_own_step`, on why `ambient_c` is restart-only

So step 8 needs a harness capability *and* the one arm the design doc calls the riskiest,
before its sixteen numerals can be closed. Instead of guessing, every unledgered step's
numerals were classified against every source that could decide them (scenario file,
chemistry file, lesson controls, existing claims, ordinals), generously — "this number
appears *somewhere* in the file" — so that what the classifier could **not** place anywhere
is a lower bound on what each step still needs. Two steps came out at one unplaceable
numeral each: `circuit-repeats-itself` (a `9 mV` nobody has measured) and `protection-on`
(`4.61 Ah`). Step 6 is eight numerals against step 12's twenty-two, so it went first.

**The generous classification is a planning tool and nothing else.** It would account for a
`2` against `series = 2` by accident, which is precisely the arm the ledger refuses to
build. It was used to *rank* targets and then thrown away; every number below is tied to a
named field.

## The eight numerals, and what decides each

> Fresh pack, and now an unreasonable demand. Eight cells in **4S2P** is **4.61** Ah at pack
> level, and the chemistry is rated **3** C continuous, so the discharge limit lands just
> under **14** A. We are asking for **40**.
>
> An `OC` flag, and the current readout pinned near **14** A while the demand box still
> reads **40**.

| numeral | arm | what decides it |
| --- | --- | --- |
| `4`, `2` | scenario | `pack.series`, `pack.parallel` |
| `4.61` | product | `cell.capacity_ah` × `pack.parallel` = 4.606902 |
| `3` | chemistry | `cell.max_discharge_c` |
| `14` (first) | claimed | the existing `just under 14 A` claim, `spelled` |
| `40` (first) | setting | the lesson's own demand box |
| `14` (second) | claimed | a **new** claim on `pinned near 14 A` |
| `40` (second) | setting | the same box, said again |

## The four arms

* **`Tie::Chemistry`** — a named field of the chemistry the step's scenario names, read as
  raw TOML so a rule's path is the key an author reads in the file rather than a field name
  `sim-data` chose. A separate arm from `Tie::Scenario` rather than a second path root,
  because the two answer different questions: a scenario is what this pack *is*, a chemistry
  is what this cell *can do*. "Rated 3 C continuous" would still be 3 C if the pack were one
  cell.

* **`Tie::Setting`** — a control on the lesson's own block, named by an enum with exactly one
  variant today (`DemandValue`). Deliberately not "any numeric field of the block", which is
  the same refusal `Accounted::Setting` already makes for check 6: step 18's prose prints a
  `10` that is a step length beside a block carrying `speed_x: 10000`, and an arm that
  matched a token against whatever field happened to hold that number would be right off the
  wrong field — green, and still green the day one of the two moves.

* **`Tie::Product`** — the sentence's own arithmetic, and **the one arm that rounds.** Every
  other tie compares exactly, because a constant printed in prose either is the file's number
  or is wrong about it. A product is computed, and its exactness is spurious: no author would
  print `4.606902`. So it is compared at the precision the prose itself commits to — the
  token's own decimal places, through the page's rounding rule (`to_fixed`, ties away from
  zero, which is also the schoolbook rule a human author uses). Each factor must resolve to
  exactly one number; a `*` wildcard under a product would make "which of them" the author's
  pick, which is the hazard the strict wildcard exists to close.

  This is *not* the general `Derived` arm the design doc flags as having no precedent. It
  computes over **named fields**, never over other tokens in the sentence, and the number it
  produces is never written down. Step 8's `2.7×` (which is `2.84 / 1.06`, both printed in
  its own sentence) still has no arm.

* **`claimed_accounting`** — check 6's `accounting_for` reused *whole* rather than a second
  reading of the same claims. The ledger asks the question about a number found by scanning a
  step's prose; check 6 asks it about a number found by scanning a literal; those are the
  same question when the number lies inside the literal. Sharing the function is what stops
  the two answering differently while both stay green.

  **Positional, not step-wide, and that is the whole strength of it.** "Some claim on this
  step spells 14" would account for the `14` in *any* sentence of the step, including one no
  claim is about — which is exactly the prose the ledger exists to reach. So the number has
  to sit inside the literal of the sentence whose claims account for it. The cost is visible
  here: step 6 states its clamped current twice, and closing the second sentence meant
  claiming it rather than waiving it.

A number accounted **both** ways — by a rule and by a claim — is refused rather than
resolved, on the same terms as `cover_by_rule`'s two-rules panic and check 6's `clash`.

## What the work turned up

* **The claim this step needed was already half-written.** The existing claim quotes *"just
  under 14 A"*, an inequality about a limit the prose does not print — 13.8207 A against a
  round 14, with a `tighter` tolerance of 0.2 saying what "just under" is allowed to mean. So
  no inequality arm was needed: the sentence that looked like the hard case was the one
  already covered, and the *uncovered* one was the plain readout sentence beside it.

* **A phrase cannot put a word immediately after a number that ends a sentence.** The first
  version of the demand rule was `"We are asking for {n}."` and it matched nothing. A
  number's `len` covers the run the scanner *trimmed*: `40.` is scanned as the token `40` and
  still spans three bytes, so the phrase matcher looks for `"."` starting *after* the full
  stop. Same trimming that `join_thousands` was caught by two commits ago, seen from the
  other side. The rule drops the stop; its prefix is what makes it specific.

* **The topology phrase has to carry words, so it is per-sentence.** `every_ledger_rule_is_a_phrase_and_is_used`
  requires a rule's words to be at least four characters, which forbids `"{n}S{n}P"` — a
  phrase made of a unit and punctuation accounts for a number by its unit, which is the
  generous match the whole table exists to avoid. So the rule is `"cells in {n}S{n}P is"`,
  and step 3's `"4 in series, 2 in parallel"` keeps its own two rules. One topology, two
  vocabularies, because two sentences say it two ways.

## Reddening

Green on the first run is the failure mode this repo has shipped, so every arm was
perturbed one at a time, with the child process launched directly under
`BELOW_NORMAL_PRIORITY_CLASS` and its **real exit code** read — never through
`start /wait`, which is exit-code-blind here and has now lied twice.

**Eleven perturbations, eleven reds, each on its own assertion** (the message was read every
time, not just the exit code — a perturbation that reddens the wrong check is this file's
recorded failure mode):

| perturbation | reddens |
| --- | --- |
| prose says `4S3P` | scenario tie, against `pack.parallel` |
| prose says `rated 4 C` | chemistry tie |
| the chemistry FILE says `max_discharge_c = 4.0` | the same tie, from the file's side |
| prose says `4.62 Ah` | product, at the prose's precision |
| the cell's `capacity_ah` moves | product, from the file's side |
| prose says `asking for 41` | setting tie |
| the lesson's demand box says `41` | the same tie, from the block's side |
| **the new claim is deleted whole** | `14` unaccounted — the claimed arm is positional |
| a rule's phrase is reworded | `every_ledger_rule_is_a_phrase_and_is_used` |
| a rule reaches a number a claim accounts for | the double-cover refusal |
| a rule names a field the file has not got | the broken-rule branch |

The eighth is the one worth naming. Deleting the whole claim block — tallies set aside — is
the only honest way to ask what it catches: replacing a field instead would redden a
different check and could not be told from this one firing. It came back red on *"step
`protection-on` prints `14` and nothing accounts for it"*, which is the positional rule doing
the thing a step-wide one would not.

## Deferred, with a price

* **Step 8 is still blocked, and now on a shorter list.** It needs the environment split at
  the mark (for the ambient drag its own prose instructs) and the general `Derived` arm (for
  `20 K` = 45 − 25 and `2.7×` = 2.84 / 1.06). The arms this slice built cover its `4S2P`, its
  `95 %`, its `200 000 s` and its `10 000×` — but a ledgered step needs *all* of its numerals,
  so none of that shows until the rest lands.

* **An open question this slice found and deliberately did not answer.** Step 8's prose says
  raising the ambient by 20 K would cost **3.5×** the fade *"two fresh packs would show"*,
  and `scenarios/calendar_fade_hot.toml`'s own header says **3.6x** for what reads as the same
  counterfactual (1.83 points at 25 °C against 6.51 at 45 °C over 600 ks — a ratio of 3.557).
  Either the two are measured over different durations, in which case both are right and
  neither says so, or one is stale. It is unmeasured here because measuring it pulls in the
  mark-side ambient split, and because an accounting arm written on top of an unverified
  number is an arm that blesses a defect. **Whoever ledgers step 8 meets this first.** Note
  also that the closed-form Arrhenius ratio for +20 K on this chemistry is 3.554, which
  rounds to the scenario file's figure and not to the lesson's — which is exactly why it must
  not be reached for as the accounting: picking the formula that reproduces the digit you
  want is the declaration hazard the design refuses.

* **The remaining two arms of the taxonomy are still missing**, and every remaining step
  needs at least one of them: an ordinal naming another step ("the LFP cell of step 1"), and
  the general figure-derived-from-its-siblings. The classification pass says the cheapest
  next targets are `circuit-repeats-itself` (one unmeasured `9 mV`) and `slow-and-patient`
  (a `12 V` that is six 2 V cells, a `C/20` that is capacity over demand, and a `19.3h` the
  clock prints).

* **No step closes without at least four arms**, which is the honest sizing of what is left:
  the ledger is expensive by design, one step at a time, and the twenty steps still
  unledgered are named one line each in `[ledger].unledgered` so the gap cannot go quiet.
