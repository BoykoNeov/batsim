# The ambient slider, and an equality asserted by two pins

`docs/plans/path-prose-ledger.md` listed the controls the claim harness could not work:
"a BMS toggle, two step lengths, an **ambient change**, `Clear queued`, `Clear latched BMS
fault`, and 'press Run again'". Every one of those has since been built except the slider.
This slice builds it, and claims the three numbers it unblocks.

## The sentence

Step 11 (`what-protection-costs`) ends with two things the pack will do, and the second is
the only place in the path where a reader is told to make it cold:

> or drag the ambient to −5 °C and watch the pack cool until `UT` inhibits the charge at
> **60.8 %**, then switch the BMS off and see `PLATING_RISK` **at the same instant** with
> the charge carrying on regardless.

The step had **no claims at all** before this slice — it sits in the unledgered list with
the note "two perf ratios no trajectory can settle". It now has three.

## What the engine says

| | protection on | protection off |
| --- | --- | --- |
| the flag | `UT` at **1494.5 s** | `PLATING_RISK` at **1494.5 s** |
| charge there | **60.7529 %** | 60.7598 % |
| current after | 0 A, and 0 A to the mark | −3 A, to 95.67 % at the mark |

The prose is right on every count, including the one it would have been easiest to be loose
about: the two flags arrive on **the same step**, not merely close together. That is not a
coincidence of this pack — `nmc_18650_generic.toml` sets `t_charge_min_k` and
`t_plating_min_k` to the same 273.15 K, so the inhibit exists exactly where the damage
begins, which is the last line of the step.

**One number is 94 % of the way through its own rounding.** The engine says 60.7529 % and
the sentence says 60.8, so the stated check is spending 4.71e-4 of the 5e-4 that one decimal
licenses. Three parts in ten thousand downward and the sentence should read 60.7. Nothing is
wrong — the value simply sits just past the .75 boundary — but a sentence with that little
margin is worth knowing about rather than discovering, so the claim's note says it.

The same 60.8 % is also written into a **code comment** in `web/app.js`, where `ccCvDone`'s
doc cites it to explain why `|i| <= taper` is not on its own an answer. Nothing checks that
comment. The claim covers the sentence a reader is shown.

## "At the same instant", asserted the only way this file can

`identical_to` compares two arms' end **states**, bit for bit. Nothing compares two arms'
**events**, and no `states` variant and no field of an arm can say "the same as that other
arm". So the equality is asserted by two claims on two arms **pinning the same number**:
`flag_first_s:UT` = 1494.5 on the protected arm, `flag_first_s:PLATING_RISK` = 1494.5 on the
unprotected one. Move either arrival and exactly one claim reddens — measured, not assumed:
the perturbation that moves the plating instant to 1495.5 reddens the value check alone.

What this does **not** catch is both arrivals moving together, which would keep the sentence
true and both claims green while the number the file records went stale. That limit is named
in the module docs rather than left to be found.

## The restart-only refusal, and why it is what makes the code sound

`run` keeps **one** `Env` for a whole trajectory. That is correct only because an ambient
override implies `Start::Restart`, and a restart arm has no pre-mark segment — so there is no
stretch of the run that happened under the step's own slider. The assertion and the single
`Env` are one design, and each names the other.

This is the third such refusal in `every_arm_is_instructed_by_its_own_step` and the weakest
of them, which the message says out loud:

* **`bms`** — the page *cannot* toggle it mid-run: `$("bms").onchange` clicks Reset.
* **`dt`** — the page can, and no step asks for it.
* **`ambient`** — the page can (`oninput` calls `applyEnv` and rebuilds nothing), **and a
  step does ask for it**: step 8's "raise the ambient slider to 45 °C and press Run", at the
  mark. What blocks that arm is two things at once — the environment would have to be split
  at `until_s`, and the sentence that would pay for the split prints `20 K` and `2.7×`, both
  figures derived from their siblings, which is the accounting arm that still does not
  exist.

`start = "restart"` is also slightly wider than the button it is named after, and the arm's
note says so: what a reader does here is drag the slider and press **Run** on the pack the
step just built. That is the same trajectory the Restart button gives — a fresh pack at
t = 0 with the controls left as the reader set them — because `applyStep` has already
reloaded this scenario. The mark-side reading is not merely unbuilt but *wrong* here: at
4100 s the protected charge has already stopped at 94.9 %, so a slider dragged there could
never show a pack inhibited at 60.8 %.

## Perturbations

Thirteen cases across both halves of the slice, each asserting its own anchor matched exactly
once before running, at below-normal priority through `subprocess.run`. The ambient half:

| perturbation | result |
| --- | --- |
| drop `ambient_c` from the arm | `the run never raised UT` — at 25 °C the pack never cools into the inhibit |
| set `ambient_c` to the step's own 25 °C | the genuine-change fence, by name |
| set the arm to `start = "mark"` | the restart-only refusal (and reachability, since the arm then begins at the mark) |
| cut the temperature out of the instruction | the anchor check: `-5` is not a number in that sentence |
| move the plating arrival to 1495.5 s | the value check, on that claim alone |
| move the temperature to −10 °C (prose, instruction and field together) | `UT` at **978.5 s** instead of 1494.5 |

**That last row is the only one that proves the arm's number reaches `Env`**, and it needed
three coordinated edits — the page's sentence, the arm's copy of it, and the field — because
the anchor and genuine-change fences refuse anything less. Every other ambient case is
consistent with the override being parsed and then discarded: dropping it leaves the lesson's
value, which is what a wiring bug would also use, and setting it to 25 °C trips the
genuine-change fence in `every_arm_is_instructed_by_its_own_step` *before* any trajectory
runs. The sensor half has no equivalent hole — `no-bms-arm` panicked from inside `measure`,
which is proof the arm's `bms` reached `build_with_bms`.

The typographic-minus trap was real and was designed out rather than hit: the prose writes
`−5 °C` with U+2212, the arm's field is `-5.0`, and the instruction is normalised through the
existing `ascii_minus` before `contains_number` sees it. Without that the arm's anchor check
would have failed on the first run — and had it been "normalise, then search for `5`"
instead, `contains_number`'s flanker rule would have rejected the match for being preceded by
a minus.

## Deferred, with a price

* **The mark-side drag is still not buildable**, and step 8's second leg — the only other
  ambient instruction in the path — stays unclaimed. It needs the split environment *and*
  the derived-figure accounting arm; neither is here.
* **Both arrivals moving together is invisible**, as above.
* **The step's two performance ratios (140× and 500×) are still unclaimable** by anything
  that runs a trajectory. Unchanged, and the reason this step stays on the unledgered list.
* **The coverage counts in both headers are hand-maintained and nothing asserts them.**
  `every_lesson_is_ledgered_or_named_as_not` guards the `unledgered` *list*, not the prose
  numbers beside it — and this slice found the claims-file tallies stale by five slices
  (`same 59` against an actual 110, `spelled 53` against 102). They were re-derived by
  counting rather than by trusting, and the module docs' step counts (seven / five /
  sixteen) were too. The next slice should re-derive rather than adjust.
* **The other sentence in the same paragraph is unclaimed**: "ask for 6 A and the BMS derates
  it to exactly 4.2 A — 0.7 C, its charge rating". That is a demand arm, which has existed
  for three slices; what stops it is `0.7 C`, a figure derived from `4.2` and the pack's
  rating, so the literal would have to be cut before it. Cheap, and left for the slice that
  builds the derived arm so the whole sentence can be claimed at once.
