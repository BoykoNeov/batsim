# The charge with no second leg, ledgered — and the third of the drop nobody had named

`leg-that-is-not-there` is scanned whole — **nineteen of the twenty-four steps, 401
numerals, twenty-three arms**. Twenty-nine numbers, nine of them inside a claimed sentence
and twenty accounted for by a vocabulary rule. It cost **sixteen new rules, three new
claims, no new arm, no new tie, and no new quantity** — and one sentence rewritten, because
the arithmetic it showed a reader was two thirds of the drop it was explaining.

## Why this step

The last slice's queue put it first at twenty-two, from the out-of-tree proxy. Re-measured
with the instrument before any editing — the ledger's own scan, temporarily switched from
panic-on-first to print-and-continue — it was **22 unaccounted of 28**, which is the proxy
agreeing with the instrument for once, and the reason is worth naming: nearly all of this
step's numbers are constants of two files, and the proxy is optimistic exactly where a step
is full of measurements.

(The step ends at **23 of 29**, and the extra one is this slice's own doing: rewriting the
arithmetic sentence added a resistance and their sum and took away a number that was never
tied to anything. Measure again *after* a prose edit — the count beside a ledger entry is
derived and checked, and a stale self-count is the defect this project has swept most
often.)

**Every number it prints is read on its own run to the 7000 s mark** — 5000 s, 5769 s and
6000 s are all inside it — so there is no trajectory here that a reader produces by
pressing something, and this slice added no arm. That is not a first: twelve of the
nineteen ledgered steps declare no `[[arm]]` (checked, not assumed — the six on step 18 and
the four on step 11 are where the arms actually live). What is worth saying is that the
numeral count is a poor predictor of cost, and the arm count is a good one. This step has
twenty-nine numbers, more than several steps that cost four arms each.

## The sentence that was wrong, and the neighbour that rotted with it

The step's argument is one line of arithmetic:

> this file's open-circuit curve tops out at 3.60 V, its resistance is about 21 mΩ, and
> 0.5 C is 1.15 A — so 3.60 + 0.024 never reaches 3.65.

**0.024 V is the drop across R0 alone, and the cell stalls 35.7 mV above its open-circuit
ceiling, not 24.** The engine's figure is exact and this file can spell it:
3.63565 = 3.6000 + 1.15 × (0.021 + 0.010). The missing third is the RC pair, which by
5769 s has been carrying the same current for something like eleven of its own time
constants and is therefore contributing its full `I·R` — the DC resistance of this cell is
31 mΩ, not 21.

Nothing was false: 3.624 does not reach 3.65 either, so the conclusion held. What the
sentence did was hand a reader an arithmetic that lands 12 mV away from the step's **own
headline number** two paragraphs earlier, with no signpost. The prose now names both
resistances and prints `0.036`, which closes on the 3.6357 the step already claims.

**The same understatement was in the paragraph next door**, and finding one without the
other would have moved the defect rather than closed it:

> both numbers that decide it here — where the OCV table ends, how large R0 is — are
> hand-fitted values the chemistry file labels as such.

After the fix that is three numbers, and the one it omits is the RC resistance — which the
chemistry file labels a placeholder in the same breath as R0. It now reads *"the numbers
that decide it here — where the OCV table ends, and how large R0 and the RC pair are"*.
Both edits are also carried into `scenarios/cc_cv_charge_lfp.toml`'s header and
`docs/plans/cc-cv.md`, which are where the sentence came from and which said the same thing
the same way.

## One number left the page, and the argument for not deleting the sentence

> real ones are charged CC-CV to 3.65 V

**That 3.65 is a fact about real LFP cells and no file in this tree decides it.** Tying it
to `cell.v_max` would have been green today and wrong in principle: a re-fit of that field
would redden a sentence about the industry that had not become false. The tempting
alternative — delete the clause and the scan goes quiet — is the defect
`docs/plans/path-twin-arm.md` records, because the sentence is load-bearing: it is what stops a
reader concluding that LFP has no constant-voltage leg in general.

So the numeral leaves and the claim stays: *"real ones are charged CC-CV to that same
limit"*. The limit is named, with a number, two sentences earlier.

## Three readings of 3.65, off three different places

This is the hazard step 11 met with `16.80` and `4.20`, and this step has it three ways in
one lesson:

| where | what decides it |
|---|---|
| "14.3 mV short of this cell's own **3.65 V** limit" | the chemistry's `cell.v_max` |
| "3.60 + 0.036 never reaches **3.65**" | the page's CC-CV box × the series count |
| "real ones are charged CC-CV to **3.65 V**" | nothing in this tree — the number left |

The middle one is the controller's target, `ccCvNote`'s own `cfg.v_cell * series`, because
what "never reaches" is about is the **leg change**, and the leg change is the box's. The
perturbation that separates them is the cheapest one in this slice: retype the page's
`v_cell` to 3.70 and the run is bit-identical — this cell never gets near either number —
so the only thing that moves is which rule can still account for its sentence.

`3.60` is printed three times as well, all three of them `[ocv].volts.33`, the last node of
a thirty-four-entry table. Indexed rather than starred: `*` would demand every node be 3.60.

## What the harness refused, and it was right

The plan (and the advisor's) for the second `5769` — *"Watch the entry step at 5769 s,
where it reads `refused 0.822 A`"* — was to grow that claim's `literal` until the instant
sat inside it, and let check 6 account for the number as the claim's own `read_at_s`. It is
an exact substring, it keeps the quoted row, and it costs nothing.

**Check 6 refuses it, by name:**

> A sentence naming the moment something happens is claiming that moment. `read at` is the
> weaker statement that we measured then, and it would stay green if the flag moved and the
> prose and the literal moved with it.

The fence has been there since check 6 was written (`199ead6`, eight days and seventy-six
commits ago) and this is the first time this project's own planning has walked into it:
both the plan for this slice and the reviewer's version of it proposed exactly the edit it
refuses. The number is tied on the ledger side instead, by a rule
that quotes **this step's own** `flag_first_s:SOC_CLAMPED_HIGH` claim — so it answers to
the event, not to where a reading was taken. `Tie::Quoted` has allowed a step to quote
itself since step 16; this is its second user and the first outside that step.

## The step quotes itself twice

The other one is `41 mW of ohmic loss`, which is this step's own `q_gen_at` claim at 5000 s
in milliwatts. It needed the two heat claims **tagged** — `q_gen_at:5000` and
`q_gen_at:6000` — because `Tie::Quoted` refuses a quantity whose claims answer differently,
and this step files two heats under one name. The tag is asserted against each claim's own
`read_at_s`, so it can never become a second, disagreeing address.

The alternative was a derivation over the sentence's printed `0.041`, and it is worth
saying why that is weaker: it would assert only that the sentence is consistent with
itself. The quotation answers to the engine, through a claim that check 7 runs.

## The claim that is satisfied by a constant, said out loud

*"the coulomb counter hits 100 %"* is now a claim on `soc_at` with `value = 1.0`. **The
number is the clamp's own upper bound** — `soc` never leaves [0, 1], so any run that clamps
satisfies the value, and a green here is not evidence that the trajectory computed
anything. That is the shape `docs/plans/phase-3-aging-faults.md` records as a monotonicity assertion
satisfied by a constant, and it is written into the claim's note rather than left for a
reader to discover.

It is still the honest accounting: no file holds 1.0, and the sentence prints it. What the
claim asserts is the other half — that **this** run reaches it, at the instant its sibling
claim says the flag fires. Hence `read_at_s = 5769` and not 6000: by 6000 the counter has
been pinned for 231 s and the reading says nothing about when.

## The perturbation record

Thirty-two edits ran, each launched directly so the **exit code is real** and each with
the panic message captured, because this suite has reddened on the wrong assertion more
than once. **Twenty-seven reddened naming their target**: twenty over the sixteen new
rules — the three-slot resistance rule and the two two-slot rules get one edit aimed at
each slot — plus one that points the quotation at the wrong address, one per new claim,
one that deletes a claim, one that flips a `quoted` declaration, and one that moves a
row's rendering. Two are green, two were superseded, and one is the borderline probe
below.

Four are worth writing down.

* **Two reddened on the wrong assertion and were re-aimed.** Moving a claim's `value` *and*
  its `spells` together fires `every_tolerance_follows_its_declared_rule` — "it says it
  spells `0.042`, and that number is not in its own literal" — before the value check gets a
  look. The point of the edit was the value, so the re-aims move `value` alone and redden
  where they were meant to: the prose-against-value tie, and the engine. The two originals
  are superseded and are not among the twenty-seven.

* **The quotation caught something nothing else could.** Nudging the `0.041` claim by
  exactly half a milliwatt — to 0.0405, the edge of what `spells = "0.041"` licenses —
  passes the value check (5e-4 allowed, 5e-4 offered) and passes the engine check (the
  engine reads 0.040715, so the perturbed value is *closer*). What reddens is
  `Tie::Quoted`'s agreement fence: the two claims filed under `q_gen_at:5000` no longer
  answer the same number. So on this claim the quotation is strictly stronger than the value
  check inside half a milliwatt, which is not an argument anyone made for it when it was
  written.

* **And its twin at the same nudge is GREEN.** The `4.181` claim moved to 4.1805 reddens
  nothing at all: it is inside the declared tolerance on both readers and its sibling is the
  claim it agrees with. That is not a hole — it is the precision the sentence commits to,
  stated out loud rather than left to look like coverage it is not.

* **One green by design, and it is the result.** The rate rule keeps `pack.parallel` in the
  denominator, and dropping that factor leaves the whole suite green, because this pack is
  1S1P. It is kept because the arithmetic a reader is shown is the *pack's* capacity — step
  11's rule has the same shape on a pack where the factor is 2 — and it starts doing work
  the day this lesson is pointed at a pack with a parallel string. Written the correct way
  round rather than the reachable way round, and said so here.

Two more things the battery settled without being aimed at them:

* Retyping the page's `v_cell` from 3.65 to 3.70 reddens **only** the "never reaches 3.65"
  rule. The trajectory does not move a bit — this cell never approaches either voltage — and
  `cell.v_max` does not move, so the two same-digit readings are demonstrably two readings.
* Setting `quoted = true` on the 100 % claim reddens by name: the row prints `100.0 %` and
  the sentence prints `100 %`, so the sentence is *not* quoting the row. That is the
  declaration being checked rather than believed.

## The rule-reach sweep, run rather than asserted

All rules matched against all twenty-four lessons before this was committed. The count of
rules reaching a step they were not written for stays at **three** — step 3's two scatter
rules, which reach `what-protection-costs` and are right there for the right reason, and
`**Step {n}**`, which reaches `what-it-cost` by identity. **None of the sixteen new rules
reaches a second step**, which is what the phrases were made long for: this lesson prints
`3.65` three times and `3.60` three times, so every phrase here has to be unique *inside*
the step before it can be unique across the path.

## Where the ledger stands

Nineteen of twenty-four steps scanned whole, 401 numerals, 161 vocabulary rules,
twenty-three arms. **Five steps left**, every one of them already carrying claims.

The queue, by the proxy that runs optimistic — re-measure with the instrument before
budgeting:

| step | proxy |
|---|---|
| `past-empty` | 27 |
| `same-discharge-other-chemistry` | ? |
| `three-times-the-current` | ? |
| `the-gradient-itself` | ? |
| `what-it-cost` | ? |

Two things known about that queue rather than guessed:

* `what-it-cost` is step 21 and the only unledgered step a rule already reaches.
* `three-times-the-current` states its circuit figures off a **second scenario file**, which
  is the shape `Tie::Elsewhere` exists for and which no rule in this vocabulary has needed
  since step 23.

**And the hole the last slice opened is still open.** Step 18's prose depends on this being
the last step before it where protection derates a demand; that is a claim about the *set*
of lessons and no numeral scan asserts it. Closing it is a check over the lesson list, not
a ledger entry.
