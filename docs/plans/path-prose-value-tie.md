# The sentence's number and the engine's, joined

`web/path-claims.toml` asserts 48 numbers the guided path's prose states, and it did so
in two halves that never met. `literal` is a substring test against the page. `value` is
a comparison against the engine. Nothing ran between them, so the number in the sentence
and the number the engine produces were never once compared to each other.

This slice adds that comparison. No engine behaviour changes, no lesson prose changes,
and no tolerance changes. Four claims change which rule they say they follow.

---

## What the gap was

`docs/plans/path-tolerance-rule.md` closed with this at the top of its deferred list,
and it had been at the top of the previous slice's list too:

> **Nothing ties `value` to `literal`.** This is the real remaining hole and it is bigger
> than the one just closed: the literal check is a substring test against the prose, the
> value check compares the engine to `value`, and re-measuring a drifted `value` without
> re-wording the sentence leaves both green with the prose wrong.

Worked concretely on the file's first claim. Step 1 says `0.6387 V at the mark`. Suppose
the engine moves to 0.6392 V:

* the literal check passes — the sentence still contains `0.6387 V at the mark`, because
  nobody edited it;
* the value check passes — `value` was re-measured to 0.6392, which is what the engine
  now produces;
* the **display** check passes — the `terminal` row prints to three decimals, and 0.6387
  and 0.6392 both render `0.639 V`.

Three green checks, and the sentence a reader is shown is wrong in the fourth decimal
that sentence exists to state. The display check is the one that looks like it should
have caught this, and it is exactly the wrong instrument: it compares at the *row's*
precision, and the prose here is finer than the row on purpose.

The second half of the gap was in the tolerance rule itself. `spells` — the number a
claim names to derive its tolerance from — was required to be *a* number in the claim's
literal, never the one stating that claim's quantity. On a sentence with two figures an
author could name the coarse one and take its tolerance. Measured last slice rather than
argued: pointing the voltage claim on `The cell empties at 4146.5 s at 1.9290 V` at
`4146.5` left all nine tests green with the voltage pinned a thousand times looser than
its own sentence licensed. Six claims in the file carry that leverage; none exploit it.

---

## Why not just format the value

The obvious closing move is to render `value` and require the result to appear in the
prose. This file has refused that from its first commit, and the reason is written into
its header: a formatter that has to agree with how a human wrote each sentence generates
false failures and then gets suppressed. `0.53 points are gone` of a health of 0.9947106
is not a formatting of that number in any sense a formatter could be taught.

What makes the tie possible instead is that English states a number in only a handful of
frames. Rather than deriving the frame, the claim names it, and the test checks that the
named frame really does carry the sentence's number to the engine's.

The precision to check at is the sentence's own, not the claim's. `tol` bounds the engine
against `value` and is deliberately *tighter* than the prose on eleven claims — a
chemistry constant pinned to 1e-4 where the sentence prints `1.5`, an exactly-1.0
starting point pinned to 1e-12. Asking a sentence to meet those would fail every hedge in
the file, starting with `just under 14 A` against a measured 13.8207. So the bound here
is half a unit in the last printed place of what the prose printed — the same rule
`tol_from` enforces from the other side, and the exact statement of "the engine's number
rounds to the one the reader is shown".

---

## The frames

Eight, chosen by classifying all 48 claims first rather than by inventing a vocabulary
and fitting the file to it.

| frame | n | what the sentence prints |
| --- | --- | --- |
| `same` | 38 | the quantity |
| `magnitude` | 2 | the size, with the sign in a word — `refused 0.822 A` of −0.82224 A |
| `complement` | 2 | how far below one — `0.53 points are gone` of 0.9947106 |
| `since_mark` | 2 | a duration since the step's mark — `383.0 s later` of 983.0 s |
| `until_end` | 1 | a duration remaining — `the last 53 seconds` of a flag at 4146.5 s |
| `displayed` | 1 | what the *row* prints — `it goes from 10m to 16m` of 983.0 s |
| `departure` | 1 | a value the quantity has **left** |
| `nothing` | 1 | no number about this quantity at all |

`same` is the one with no second reading available to an author, and it covers four
claims in five. Each of the others is fenced so it cannot be reached for when `same`
would have failed:

* **`magnitude` requires a negative value.** On a positive one it is a silent alias for
  `same`.
* **`since_mark` requires `after_mark`, `until_end` forbids it.** The two duration frames
  point in opposite directions from the same instant, so without this an author could try
  both and keep the one that fit. Fencing each to one side of the mark leaves at most one
  available per claim.
* **`displayed` forbids `spells`.** Two claims could be read either as stating a figure or
  as quoting a row; forbidding the field means the sentence decides rather than the
  author. What remains is the one claim whose numbers are minutes off a formatter, where
  no arithmetic turns 983.0 into `16m` except `fmtTime` itself. The tie there is the
  display check — literal ⊇ `16m` == the clock's rendering of the measured instant — so
  the frame asserts the chain is complete instead of measuring: name a row, be `quoted`,
  and carry that row's string inside your own literal.
* **`nothing` requires a literal with no digit in it.** This is the variant that would
  otherwise be the escape hatch that re-opens the whole hole, so it is checked rather
  than trusted. `An OC flag` qualifies; anything printing a figure does not.
* **`departure` cannot stand alone.** It asserts the *opposite* of `same` — the engine
  must be further from the sentence's number than the sentence's own precision — which
  any unrelated figure in the sentence satisfies. `` `soh cap` leaves 100.00 % at t = 10
  s `` would be "proved" by pointing at the `10`. So a `departure` must find an earlier claim
  on the same sentence, quantity and number stating `same`. The sentence is two
  statements — one showing the row holding the value, one showing it gone — and the
  fence is that both must be present. Requiring the sibling's own frame to be `same`, not
  `departure`, is what stops two of them vouching for each other.

The leverage `spells` had is closed as a side effect: under `same`, pointing the voltage
claim at `4146.5` compares 4146.5 against 1.9290 and fails on sight.

---

## What it found

**Four claims declared a tolerance rule their own literal contradicts.** `grid` means
"the quantity is a time the engine reports only on the step grid, *and the prose spells
no number in it*". These four spell it:

| claim | literal | was | is |
| --- | --- | --- | --- |
| step 2's knee | `It empties at 4154 s` | `grid` | `tighter`, spells `4154` |
| step 10's clamp | `` `SOC_CLAMPED_HIGH` at 5769 s `` | `grid` | `tighter`, spells `5769` |
| step 20's payoff | `exactly zero, for 254 seconds` | `grid` | `tighter`, spells `254` |
| step 1's knee | `the last 53 seconds` | `grid` | `tighter`, spells `53` |

This is the same defect class as the two that `tol_from` was built for — a claim citing a
rule it does not follow — and it was invisible until each claim had to say what its
sentence *states*, because saying "this sentence states the quantity" forces you to
notice that it does.

**No tolerance value moves.** All four hold half a step (0.25 s) and all four spell a
number with no decimal, whose rule is 0.5 s. A half-step is tighter than half a second,
so `tighter` is the accurate declaration and the numbers were right all along. That is
the whole of the change: four citations, no measurements.

The remaining two `grid` claims are exactly the two whose frame is `nothing` or
`displayed`, which is not a coincidence — a sentence that spells its own number takes
that number's rule however coarse the grid is.

**One claim sits exactly on its rounding boundary, and it is now recorded.** `the last 53
seconds` is a flag at 4146.5 s on a step ending at 4200: the duration is 53.5 s, and half
a unit in the last place of `53` is 0.5. The difference *is* the tolerance, to the last
bit. 53.5 rounds to 54 under every rule there is — the sentence truncates rather than
rounds — and the check admits it only because the window is symmetric. Left as it reads,
because 53 is the number the reader is given, but written into the claim's note: a
sentence with zero margin is worth knowing about, and an admitted-but-unremarked case is
this file's own defect class.

---

## Reddening

Green on the first run is the failure mode this repo has shipped (`surface-vs-bulk.md`:
five perturbations reported GREEN and every one was a lie), so every fence was reddened
deliberately, one at a time, with the child process's real exit code read directly —
never through `start /wait`, which hides it.

Eleven perturbations, eleven reds:

| perturbation | catches |
| --- | --- |
| `0.6387` → value 0.6392 | the hole itself: literal, value and display all stay green |
| voltage claim's `spells` repointed at `4146.5` | the leverage `spells` used to have |
| `complement` claim reframed `same` | a frame swapped for the wrong one |
| `magnitude` on a positive value | magnitude as a silent alias for `same` |
| `since_mark` with `after_mark = false` | a duration counted from a mark not passed |
| `until_end` spelling `52` instead of `53` | the countdown frame stating a wrong duration |
| `displayed` with `quoted` removed | the chain with its middle link missing |
| `nothing` on a literal printing `0.5` | the escape hatch that would re-open the hole |
| `departure` moved before its sibling | a difference claim standing on its own |
| `departure` value moved back to 100.00 | a sentence saying the row changed where it did not |
| `states` deleted from a claim | a claim with no declared frame (parse error) |

**One of the eleven lied on the first attempt and was rewritten.** Reframing the
`departure` claim's *sibling* did go red — on the reframed claim's own value assertion,
never reaching the sibling fence. A case that reddens for the wrong reason proves nothing
about the fence it was written for. Moving the `departure` claim earlier than its sibling
isolates it, and the message that comes back names the sibling rule.

---

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no schema, no
version constant. `web/pkg` needs no rebuild: the only Rust here is a test, and
`path-claims.toml` is read by that test alone — the page never loads it. No lesson prose
changed, so no number a reader sees is different.

---

## Deferred, with a price

* **Numbers inside a claimed literal that no claim is about.** This slice ties the number
  a claim *spells* to the value it measures, and says nothing about the other figures in
  the same sentence. `**99.98 %** when the cell empties at **207.5 s and 1.9306 V**`
  carries three numbers and two claims: the percentage and the voltage are each pinned
  now, and the 207.5 s is checked only as characters that must still be there. It could
  drift to 210 s and nothing would go red as long as the prose and the literal drifted
  together — which is the *old* hole, surviving on the one number in that sentence nobody
  claimed. This is the completeness direction rather than the correctness one, and it is
  the natural next slice: it is also the fourteen uncovered steps, reached from inside a
  covered one.
* **Frame-shopping is narrowed, not eliminated.** Five numeric frames exist and the
  fences make at most three reachable for any one claim (`same` always, plus one duration
  frame by side of the mark, plus `magnitude` on a negative value or `complement`
  anywhere). An author whose `same` check fails could still try `complement` and find it
  passes by coincidence. Each frame is a statement that must be *true* of the sentence, so
  this is far narrower than the free choice it replaced, but it is not nothing.
* **`complement` assumes the whole is one.** Both instances are health fractions, where
  that is the only sensible reading. A future claim stating a shortfall from anything else
  would need the base to be a field.
* **`departure`'s sibling fence is structural, not semantic.** It requires an earlier
  `same` claim on the same sentence, quantity and number. It cannot check that the two
  instants are *adjacent* — that the row changes on that step rather than some steps later
  — which is what the sentence "leaves 100.00 % at t = 10 s" actually promises. Today the
  pair reads 9.5 s and 10.0 s, one step apart on a 0.5 s grid, and nothing enforces it.
* **`run_for_s` is still unbounded by anything outside this file**, unchanged by this
  slice and unchanged by the last one.
