# The numbers in a claimed sentence that no claim was about

`web/path-claims.toml` asserts the numbers the guided path's prose states, and every
check it had was about the number a claim *names*. A sentence usually prints more than
one. This slice makes every figure inside a claimed sentence answer to something.

No engine behaviour changes and no lesson prose changes. One claim is added, and one
check.

---

## What the gap was

`docs/plans/path-prose-value-tie.md` closed with this at the top of its deferred list:

> **Numbers inside a claimed literal that no claim is about.** This slice ties the number
> a claim *spells* to the value it measures, and says nothing about the other figures in
> the same sentence. `**99.98 %** when the cell empties at **207.5 s and 1.9306 V**`
> carries three numbers and two claims [...] It could drift to 210 s and nothing would go
> red as long as the prose and the literal drifted together — which is the *old* hole,
> surviving on the one number in that sentence nobody claimed.

Worked concretely. That sentence had a claim on the `99.98` and a claim on the `1.9306`,
each pinned to the engine five ways. The `207.5` had nothing: the literal check requires
those characters to be in the prose, and the prose is where the characters came from. Move
both to `210 s` and all five checks stay green while the sentence tells a reader the cell
empties three seconds after it does.

That is the *original* hole — a number the reader is shown, checked by nothing — surviving
five slices inside the sentences those slices had already claimed.

A scan first, before any design: 42 distinct claimed sentences, 13 numbers in them that no
claim named. Three were the sign of a negative value (`**−0.0640 V**` against a `spells` of
`-0.0640`), two were the clock strings `10m`/`16m`, seven were ordinary read instants, and
one was the `207.5`. Small enough to close completely rather than partly.

---

## Derived, not declared

The three fields this file grew to close earlier holes — `tol_from`, `states`, `spells` —
are all declarations, and the reflex was to add a fourth. It would have been wrong.

Each of those exists because it encodes something a machine cannot decide: which rule an
author meant a tolerance to follow, which frame a sentence uses a number in. The
accountings here are not like that. "Is this token some claim's `spells`", "is it the
instant some claim reads at", "is it inside a string this file says the panel prints" are
exact numeric facts about the claims already sitting beside the number. A declared
`accounts = "read_at"` next to a token that is really something else would be a fresh
instance of the defect `tol_from` was introduced to kill — a claim citing a rule it does
not follow.

The decisive argument was the one the declaration could not answer: the hard case here is
`207.5`, and an author who had to declare it would write `read_at` — which is exactly the
reading that leaves the hole open. A field cannot tell those apart either, so it buys
nothing and costs a way to be wrong.

**There is no waiver variant.** Not "there is one and nothing uses it": there is none, and
the file's 42 claimed sentences need none. An escape hatch meaning "this number is not a
measurement" is precisely what re-opens the hole, so a future literal printing a chemistry
constant or a control setting fails loudly, and the answer will be an arm that checks it
against the chemistry file or the lesson block. Same shape as the refusal of a `widened`
tolerance variant one slice ago.

---

## The three accountings

55 numbers are printed across the 42 claimed sentences, and each is accounted one of
three ways.

| accounting | n | what it says |
| --- | --- | --- |
| `spelled` | 46 | a claim on this sentence names it in `spells` |
| `read at` | 7 | it is the instant a claim on this sentence is read at |
| `shown` | 2 | it is inside a string this file asserts the panel prints |

`spelled` ignores the sign on both sides — the scanner collects none, and `**−0.0640 V**`
spells `-0.0640` of a negative value.

`read at` reads the instant in the claim's own frame: absolute before the mark, since the
mark on a charge leg. `99.45 % at 250 s` prints a health and a moment, and the moment is
the claim's `read_at_s`. Fenced to one frame per claim for the reason the two duration
frames in `states` are fenced to opposite sides of the mark: two readings would let an
author try both and keep whichever matched.

`shown` covers a claim's own `shows`, and one instant more. `` it goes from `10m` to
`16m` `` quotes the clock at both ends of a leg: the second is the claim's own instant, and
the first is the mark, which is where the leg begins and the only other moment such a
sentence can mean. Only the `sim time` row qualifies — it is the one row that is a function
of time alone, and every other would need telemetry in a check that deliberately runs no
engine.

---

## What it found, and the thing it did not

The `207.5` got a claim of its own, against the flag the sentence says arrives there.
Measured before it was written rather than copied out of the prose: `SOC_CLAMPED_LOW` lands
on 207.5 s exactly, so the sentence was right and only unchecked.

Its tolerance is `spelled` at 0.05 s — half a unit in the last place of `207.5` — which is
**five times tighter than the half-step** a `grid` declaration would give. That direction
is worth noticing: the four claims this file corrected last slice were `grid` declarations
whose prose spelled a *coarser* number, and this is the same rule seen from the other side.
A grid time can only move by a whole step, so nothing is lost by pinning it under one.

**And then the check, as first written, did not force that claim.** Deleting it and running
the suite came back green. The `207.5` was accounted `read at`, because the sentence's other
two claims are read at 207.5 — the arm answered "we measured then" to a sentence that says
"the cell empties then". Those are different statements and the reader is given the second.

So `read at` is fenced against events: it is refused at any instant where the run raises a
flag it did not have a step earlier. That fence needs a trajectory, so it lives inside the
value check, which is the only place a run exists. With it, deleting the claim fails with
the flag named in the message.

This is the slice's real result. The first version would have shipped green, with a plan
doc claiming a hole closed that was closed only by the claim I happened to add by hand —
and nothing forcing the next author to do the same.

---

## Reddening

Green on the first run is the failure mode this repo has shipped, so every arm was reddened
one at a time, with the child process's real exit code read directly — never through
`start /wait`, which is exit-code-blind here and has now lied twice.

Seven perturbations, seven reds. Four are edits to the data, three cut an arm out of the
code, and the two prose cases edit the sentence **and** the literal together — editing only
one reddens the literal check and proves nothing about accounting, which is the trap the
last slice hit.

| perturbation | catches | fails |
| --- | --- | --- |
| delete the new `207.5` claim | the hole itself | the event fence |
| `250 s` → `260 s` in prose and literal, `read_at_s` left stale | the `read at` arm | check 6 |
| `` `10m` `` → `` `9m` `` in prose and literal | `shown`: the clock at the leg's origin | check 6 |
| a `42` added to a claimed sentence | that there is no waiver | check 6 |
| sign stripping cut from the code | `spelled` on a negative value | check 6 |
| the `shows` half of `shown` cut | `shown`: a claim's own row string | check 6 |
| the `spelled` arm cut | the ordinary case, 46 of the 55 numbers | check 6 + the fence |

Six of the seven fail on exactly one test. The last fails on two, and coherently: with
`spelled` gone, `` `SOC_CLAMPED_HIGH` at 5769 s `` falls through to `read at` and lands on
its own flag, which is the fence doing its job.

---

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no schema, no version
constant. `web/pkg` needs no rebuild: the only Rust here is a test, and `path-claims.toml`
is read by that test alone — the page never loads it. No lesson prose changed, so no number
a reader sees is different.

---

## Deferred, with a price

* **Sentences no claim is about — which is now the whole of the completeness gap, and the
  larger half.** This slice closed the part that lived *inside* a claimed literal. Fourteen
  of the twenty-one steps carry no claim at all, so they have no literal to scan and
  nothing here touches them. That needs a different instrument — a ledger over each step's
  whole prose rather than over the sentences already claimed — and unlike this one it
  cannot refuse a waiver taxonomy: a step's prose is full of numbers that are not
  measurements at all (currents the reader types, C-rates, chemistry constants, ordinals
  naming other steps). Costing it honestly: the check here needed no taxonomy only because
  42 hand-written sentences happened to need none.
* **The event fence can demand a claim a sentence does not make.** It fires on any flag
  arriving at a `read at` instant, and it cannot tell a sentence naming that event from one
  that reads a row at a moment where something unrelated happens to fire. No claim in the
  file trips it today. If one ever does, the escape is to claim the flag — which is true of
  the run, but says the sentence states a flag time when it may not, and that would be a
  claim asserting more than its sentence does.
* **The fence knows flags and not other events.** The debt reaching zero
  (`deficit_zero_s`) is as much an event as a flag and is not fenced against; no `read at`
  token lands on one today. The general form would be "any instant this file can measure a
  quantity *arriving* at", which is a list, not a rule.
* **A sentence is grouped by `(step, literal)`.** Two claims quoting different substrings
  of one sentence are two groups, so a number spelled only by the sibling group goes
  unaccounted. That is the fail-toward-red direction and nothing in the file is written
  that way, but the next author to split a sentence meets it; the fix is to give both
  claims the same literal.
* **`shown`'s second instant is `sim time` only.** A `displayed` claim quoting any other
  row at the mark would need telemetry, and this check runs no engine on purpose. The
  restriction is a red, not a silent pass.
* **`run_for_s` is still unbounded by anything outside this file**, unchanged by this
  slice and by the two before it.
