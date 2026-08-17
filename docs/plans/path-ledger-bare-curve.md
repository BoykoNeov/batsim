# The first step, ledgered — and a note that had outlived its reason

`bare-curve` is step 1: one LFP cell, 2 A out, nothing protecting it, run past empty. It is
the **fourteenth of twenty-four** steps scanned whole, and it was picked by measurement
rather than by the recorded ranking — which, for once, agreed.

It cost six vocabulary rules, three claims, three prose edits, and one doc comment that had
been wrong for two slices without anything being able to tell.

## The ranking, re-measured — and now it budgets

`path-ledger-weaker-short.md` closed by saying the recorded table "ranks and does not
budget", because it predates four arms and two claim passes. Re-measuring it is cheap: put
all eleven unledgered steps in `[ledger].steps` at once, turn the accounting check's
first-failure panic into a print-and-continue, and read the count off one 0.4 s run. The
technique is `path-ledger-particle-step.md`'s; running it over *every* remaining step rather
than the one you are about to do is the only new part.

| step | unaccounted numerals |
| --- | --- |
| `bare-curve` | **12** |
| `protection-off` | 15 |
| `looks-fine-from-outside` | 19 |
| `one-step-that-got-through` | 21 |
| `leg-that-is-not-there` | 22 |
| `what-protection-costs` | 25 |
| `past-empty` | 27 |
| `three-times-the-current` | 31 |
| `same-discharge-other-chemistry` | 35 |
| `what-it-cost` | 35 |
| `the-gradient-itself` | 38 |

The old table's row for this step said twelve and was right. Two rows moved a long way,
though, and both in the direction that matters: `the-gradient-itself` is now the *most*
expensive step left, and `same-discharge-other-chemistry` — the step next door, which looks
like a twin of this one — is three times this step's cost. Nothing in the recorded ranking
said so.

**This table is a measurement and will age exactly as the last one did.** What keeps it
honest for one more slice is that the command producing it is written down above and takes
under a minute.

## Where the twelve went

Eight to rules, one to a rule that should not have needed inventing, three to claims.

| numeral | accounted by |
| --- | --- |
| `100 %` charge | the scenario's `pack.initial_soc` |
| `2 A`, twice | the demand box, in two sentences and so two rules |
| `0.87 C` | the box divided by the nameplate |
| `2.303451 Ah` | the chemistry's `cell.capacity_ah` |
| `0` of `I·R0` | the chemistry's provenance, where `R0` is named |
| `2.00 V` | the chemistry's `cell.v_min` |
| `step 20`, `step 21` | where `past-empty` and `what-it-cost` sit in the path |
| `20 %`, `80 %` | two new claims, one per end of the window |
| `69` minutes | a new claim, through the clock's own `69m` |

## Three prose edits, and why each was the honest one

**`2.303` became `2.303451`.** The sentence said the cell "holds 2.303 Ah", and the file
says 2.303451. A constant tie compares exactly — a rounded restatement of a constant is
neither the file's number nor a computed quantity — so the shorter form could not be tied at
all. This is the same call `path-ledger-dfn-step.md` made for `5.15` and `15.46`, and here
it costs nothing: the sentence's own point is that the figure is *not* a round number,
because it is a fitted usable window rather than a marketing capacity. Six decimals make
that argument better than three did.

**The knee now quotes the clock.** The step says "the knee, at about 69 minutes", and the
panel says `69m`. Those are not the same string, and a `displayed` claim requires the row's
own string inside the claim's literal — otherwise the chain from sentence to formatter has a
missing link. The two available moves were to drop the readable half (`at about 69m`) or to
say both. It now says both: *"the knee, at about 69 minutes — the clock reads `69m` — where
the cell empties"*. Two `69`s, one accounting; the panel's string is what both of them are.

**Nothing was reworded to make a number go away.** That was available for the `0` of `I·R0`
and was the wrong answer — see below.

## The note that had outlived its reason

`Tie::Name`'s doc comment said, of the `0` in `R0`, that it "had no field to point at and was
reworded away instead". That was true when written and stopped being true the moment the arm
below it was built, because this chemistry's provenance string says:

> `R0, RC, and the aging/safety sections remain order-of-magnitude placeholders (TODO: fit).`

So the digit has a field, and a better one than a reword: the string that ties it is the
string that *declares what `R0` is*. `RC` sits two characters later and the digit run after
it is empty, so the prefix reaches one number and not two — the same shape as `Chen2020`,
whose author-surname occurrence is dropped the same way.

The lesson is not about this arm. **A note recording why something could not be done outlives
the reason, and the ledger's own steps are where such notes get read as fact.** The doc has
been amended rather than deleted, because the historical half is still true.

## The window claims, and the thing they do not check

*"LFP's open-circuit voltage barely moves between 20 % and 80 % charge"* prints two numbers
and neither is a node of anything: the `[ocv]` table's grid has nodes at 0.25 and 0.85, not
at 0.20 and 0.80. Rewording to the nodes was considered and rejected — that would make the
reader's window an artifact of the fitting grid, which is backwards.

Instead the two ends are claimed against the run: at t = 829.0 s this discharge is at
80.0058 % and at t = 3317.0 s it is at 19.9993 %. The tolerance is **one coulomb-counting
step** — 2 A for 0.5 s out of 2.303451 A·h is 1.206e-4 of the cell, the finest a charge level
can be named on this grid — which is forty times tighter than the half-a-point the whole
percent in the prose would license.

**It is two-sided, and that was measured rather than assumed.** Moving the chemistry's
capacity by 0.1 % moves the charge at 829 s by 2.13e-4 and at 3317 s by 8.52e-4, both well
past the tolerance. What it does *not* check is the sentence's actual argument: no claim here
reads the OCV table, so a re-fit that made the plateau twice as steep would leave both claims
green. That is written into the claims' own notes rather than left to be discovered.

## Perturbations

Twelve, all red, exit codes read from the process rather than from `start` — which is
exit-code-blind and has lied about a whole harness twice in this project.

| perturbation | reddens |
| --- | --- |
| prose `100 %` → `90 %` | the scenario tie |
| prose `0.87 C` → `0.88 C` | the ratio of box over nameplate |
| prose `2.303451` → `2.303450` | the nameplate, compared exactly |
| prose `I·R0` → `I·R1` | the provenance name tie |
| prose `2.00 V` → `2.10 V` | the declared floor |
| prose demand box `2 A` → `3 A` | the second demand rule |
| prose `step 20` → `step 19` | the ordinal of `past-empty` |
| prose `step 21` → `step 22` | the ordinal of `what-it-cost` |
| prose `80 %` → `70 %` | the window claim's literal |
| prose `` `69m` `` → `` `70m` `` | the displayed claim |
| chemistry `capacity_ah` −0.1 % | the value check |
| delete the `69m` half of the knee's literal | the accounting scan |

**The engine perturbation is the one worth reading carefully, and it nearly passed for
something it is not.** Run plainly, it reddens on `0.6387 V at the mark` — a claim that has
been in this file since the day it was written — and says nothing whatever about the two
claims this slice added. Confirming those needed the same print-and-continue treatment as the
scan: with the value assert listing instead of stopping, the perturbed run names six red
claims on this step, and both `soc_at` claims are among them. **A red exit code on a
perturbation is not evidence about the check you were testing** — this project has now
mistaken that four times, and the cheap defence is to make the check enumerate.

The deletion perturbation is the last row and it is the one that tests the *claim* rather
than the sentence: shortening the knee's literal so it no longer contains `` `69m` `` leaves
the sentence intact and the number unaccounted, and the scan says so.

## What is still not checked here

* **The sentence's own argument, on both new claims.** Named above; the flatness of the
  plateau is what the reader is being told and no claim reads the OCV table.
* **`eight and a half seconds` is still invisible.** So is the `half` in it. The word-numeral
  scanner has been one future slice away since step 3, and this step needs it as much as any:
  the gap between the two flags is stated only in words, and it is the whole point of the
  paragraph.
* **`same-discharge-other-chemistry` still says `2.303`.** Step 2 refers back to "the LFP
  cell's 2.303", and this slice moved step 1's copy to `2.303451` without touching it,
  because step 2 is unledgered and its sentence is a shortened reference rather than a
  statement of the constant. Two files stating one number and only one of them checked is the
  shape that has bitten twice (`path-ledger-last-two-steps.md`,
  `path-ledger-weaker-short.md`). It is now also true *within* `web/app.js`.
* **The `69m` claim shares its quantity with the `53 seconds` claim.** Both read
  `flag_first_s:SOC_CLAMPED_LOW` at 4146.5 with the same value, which is fine today and would
  become a hazard the moment anything quotes that quantity across steps — the agreement fence
  compares claims, and two that agree pass whichever is picked.
* **Nothing checks that a ledgered step's rules stay on that step.** The vocabulary is
  global: `step {n} of this path, and step {n}` is written for step 1 and the identical
  phrase occurs in step 2, which is unledgered. When step 2 is scanned the rule will cover it
  too — correctly, as it happens, but by luck rather than by design.

## What the ledger looks like now

Fourteen of twenty-four steps scanned whole, 251 numerals, nineteen arms. Ten steps left, all
carrying claims on their claimed sentences and none scanned end to end.

**Nineteen is the same nineteen as last slice.** The three ledger slices before this one each
cost a new kind of tie — `Tie::Sum`, `Tie::Page`, `Tie::OnArm`. This step needed six new
phrases and no new arm at all, which is both why it was cheap and what a finished taxonomy
looks like from the inside.

By the table above the next one is `protection-off` at fifteen, and after this slice that
number is a budget rather than a ranking.
