# The third cell, reachable

Step 2 of the guided path — `same-discharge-other-chemistry` — is one of the two lessons the
prose ledger has never scanned. Its closing paragraph sends the reader to a **third**
chemistry, `cc_discharge_lgm50.toml`, at two different currents, and everything that paragraph
says about what they will see there was tied to nothing at all: no arm could load a scenario
file that is not some lesson's own, so there was no trajectory behind the sentence and no
claim could be written on one.

This slice builds that arm and puts four of those numbers on it. It does **not** ledger the
step; the rules the rest of its prose needs cannot exist yet, for a reason recorded below.

## The measurement, and the record holding for a fourth time

The recorded queue said 35 unaccounted of 40. Measured first, as the previous slice asked —
the ledger's own accounting check switched from panic-on-first to print-and-continue and
pointed at this step alone — it is **35 of 40**, exactly. That is four slices running where
the recorded sizing was right, on an instrument that takes under a minute.

Measured again after this slice, on the rewritten prose: **32 unaccounted of 41** — the step
now prints one numeral more than it did (the file name is named twice), and four of its
numbers answer to a run. The residual, by what each will need:

| kind | how many | what closes it |
| --- | --- | --- |
| the third-cell paragraph's constants and controls | 7 | rules over the picked file — **blocked**, see below |
| this step's own files and controls | 7 | ordinary vocabulary rules |
| ordinals pointing at steps 1 and 20 | 5 | `Tie::Ordinal` |
| its own sentences' arithmetic | 3 | `Tie::Derived` |
| the window its own claims are read at | 3 | claim literals, extended |
| the neighbour's constants, and a rate over them | 3 | `Tie::Elsewhere` on `bare-curve` |
| the neighbour's **measurements** | 3 | the expensive ones — see the end |
| the `past empty` figure at this step's own mark | 1 | a claim |

## Four numbers were wrong, and one of them was wrong in the way that matters

Every number in the paragraph was driven and read before a line of it was rewritten.

**`0.868 C` was the neighbour's rate, printed on this cell.** The prose tells a reader to put
the box up to `4.47 A`, "which is the same 0.868 C". 4.47 A of a 5.153198 A·h cell is 0.8675 C,
and the same rounding the sentence uses one clause earlier — *"0.867 here against step 1's
0.868"* — makes that **0.867**. The figure it printed is the LFP cell's, one step next door,
in a sentence about this one.

**`5.15 Ah` is a rounded constant, and a rounded constant has no arm.** `tie_agrees` compares
a constant exactly, on the stated grounds that a constant in prose either is the file's number
or is wrong about it. This is the same token step 16 was caught by
(`docs/plans/path-ledger-dfn-step.md`), and it is resolved the same way: the sentence now
prints `5.153198`, which is what the chemistry file and the scenario's own header both say.

**`5700 s` is 5708.5 s.** The run reaches 20 % charge at 5708.5 on a half-second grid, and the
sentence gave a round number for a crossing that is not an approximation — the shape
`past-empty` was caught by in the other direction (`4226` against a subtraction of 4226.5).

**And the sentence told the reader they could not see the number it printed.** This is the one
that is a defect rather than a digit:

> Leave it at 2.6 A and the fall is much the same (618 mV) while the run takes 5700 s to get
> there, so this step's mark stops you at 41 %.

The mark is 4200 s. A reader who obeys the step stops 1508 s before the fall the same sentence
quotes. That is "right but unreachable", which this repo has shipped three times, and the
harness would have refused a claim on it: `every_claim_is_reachable_in_its_own_step` compares
a claim's instant against its arm's own `to_s`, and says in as many words that lengthening an
arm to cover a claim is how that check becomes a tautology. So the prose moved instead. The
mark is now named as a place the run passes, and the reader is told to keep pressing Run.

The paragraph now reads:

> Load `cc_discharge_lgm50` from the picker for a third — 620 mV, a 5.153198 Ah cell fitted
> from PyBaMM's Chen2020, but only if you also put the demand box up to 4.47 A, which is the
> same 0.867 C on the bigger cell. Leave `cc_discharge_lgm50` loaded, put the box back to
> 2.6 A, and the fall is much the same (618 mV) — but this cell takes 5708.5 s to get there,
> which is past this step's mark, where it still has 41 % left. You have to keep pressing Run
> to see it.

`620 mV`, `618 mV` and `41 %` were measured and are right: 620.0709, 618.0763, 41.137 %. So is
`168` for the LFP cell across the same window (168.15), which the plan for this slice flagged
as a plausible defect because step 1's own prose talks about 20 % to 80 %. It is the 90-to-20
window, and it is correct.

## What was built

**`Arm::scenario` — the picker.** `Arm::pack_from` models a walk to another *lesson*; this
models the `<option>` list. They cannot be one field, because a scenario the path never teaches
has no lesson to name, and `cc_discharge_lgm50.toml` is exactly that: in `web/index.html` and
in no lesson block. A picked file rebuilds the pack from t = 0 under the step's own controls,
which is what `loadScenario` does.

Five fences, in `assert_picker`, none of them reachable from the claims file — so each gets a
`should_panic` test of its own, on the terms the `pack_from` fences beside them established:

| the fence | why |
| --- | --- |
| not the step's own file | then nothing is loaded and the arm is a restart wearing a second name |
| in the picker | a file on disk is not a file a reader can reach. The check reads `web/index.html`, which is the **narrower** list — `loadScenarioList` replaces it from `GET /scenarios` at run time — so it fails toward red |
| named in the instruction | the file and the sentence that sends a reader to it are two statements of one fact |
| implies `Start::Restart` | `loadScenario` closes the backend and starts the clock at zero, whatever the reader had reached |
| not combined with `pack_from` | two navigations composed under one sentence asking for either |

**Two arms and four claims.** `the third cell` runs the picked file at 4.47 A to this step's
own mark; `the third cell at this step's own current` runs it at the step's own 2.6 A to
5708.5 s, which is the sentence's instruction to keep pressing Run. Between them they carry
the fall at each current, the time the slower one takes to get there, and the charge left at
the mark.

**Two quantities.** `v_fall_mv_soc:0.90:0.20` is the fall between two charge levels — the
first quantity for a sentence that prints a *drop* and neither of its ends, which is what both
of these sentences do, and it is deliberate: what a reader is meant to carry away is the shape
of the curve and not where this particular cell sits. `t_at_soc_below:0.20` is
`t_at_v_below`'s sibling one axis over — a voltage crossing is what the plot shows, a charge
crossing is what this sentence means.

## What was deliberately not built

`Tie::Picker` (digits inside a scenario file's own name, for the `50` of `cc_discharge_lgm50`)
and the extension of `Tie::OnArm` that would let a rule read the picked file's `capacity_ah`
and its `Chen2020` provenance. Both were written, and both were taken back out.

The reason is a rule of the harness rather than a doubt about the design:
`every_ledger_rule_is_a_phrase_and_is_used` counts a vocabulary rule's matches **only in
ledgered steps**. Step 2 is not ledgered, so a rule written for it matches nothing and reddens
— which means the ties those rules would use would land with no user at all, the
`CCCV_PERIOD_S` shape this file has been caught by once already. They belong to the slice that
ledgers the step, and `Arm::scenario`'s own doc comment now says so where the next author will
read it.

Worth writing down, because the argument it retires is a good one and is still in the tree:
`Tie::OnArm` refuses a file field on the grounds that "an arm overrides controls, not files:
asking a scenario field under an arm's name would resolve to the same number and claim it came
from somewhere else". That is exactly right for every arm that carries no `scenario`, and
false for one that does. The refusal stays; the exception is what the next slice adds.

The concrete cost of holding them back: `0.867` and `5.153198` are repairs that **nothing yet
enforces**. They are right today and no check keeps them right until those rules land. That is
this slice's known hole, named here rather than left to be found.

## Perturbations

Three, each applied to the green tree and reverted:

| what was changed | what went red |
| --- | --- |
| arm A's `scenario` pointed at `cc_discharge_lfp.toml` | the instruction fence, **and** the engine check — the 620 mV claim measured 169.3 on the LFP cell. The picker arm really does change the pack |
| arm B's `to_s` shortened to 5000 s | reachability, naming the 618 mV claim — the same refusal the old prose had earned |
| the 41 % claim detached from its arm | the engine check: 0 against 0.411369, because step 2's own cell is long past empty at its own mark |

## What is left, and what the next slice looks like

Twenty-two of twenty-four steps are ledgered. This step still is not, and its residual is
recorded in `[ledger].unledgered` as it always has been — now with the claim count beside it
(9, on 2 arms).

The cheap remainder — this step's own constants, the ordinals, the derivations, the two
`Elsewhere` reads on step 1's files — is one slice's work, now that every arm shape it needs
exists. What is **not** cheap is the neighbour's measurements: `4146.5` s, `168` mV and `53` s.

`4146.5` is quotable: step 1 claims its own knee and `Tie::Quoted` reaches a claim on another
step. The other two are not. No claim on step 1 states 168 mV or 53 s, and a claim on **this**
step is measured on this step's trajectory. Closing them needs a twin arm
(`pack_from: "bare-curve"`), and a twin arm needs an instruction sentence in *this* step's
prose — which today has none, because nothing here asks the reader to go back and run step 1
again. So the choice, when the ledgering slice reaches it, is between writing that instruction
and deleting the comparison. It should be made deliberately: a sentence invented to satisfy an
arm is the same defect as a number invented to fill a table.
