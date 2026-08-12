# The tolerance rule, enforced instead of written down

`web/path-claims.toml` asserts 48 numbers the guided path's prose states. Each one is
guarded by up to four checks — the sentence is still in the prose, the engine still
produces the number, a reader can still reach it, and the panel still prints what the
sentence quotes. The second of those is only as strong as `tol`, and `tol` was the one
field in the file nothing checked.

This slice makes each claim declare *which rule* its tolerance follows, and checks the
derivation. No engine behaviour changes. One tolerance in the file moves.

---

## What the gap was

The file states a rule in its header: half a unit in the last printed place of the
number the sentence spells, so the tolerance is exactly "the prose rounds to this and
nothing else". Every claim then re-derived that rule by hand and recorded the reasoning
in free text, where nothing could read it.

A rule re-derived by hand is a rule that is sometimes not derived:

* `04933c5` fixed two tolerances whose notes cited this rule and did not follow it.
* `a1b0945` fixed the same defect again — on a claim the *previous* slice had just
  added. `**383.0 s later**` was given a half-step of 0.25 with a note calling that
  tighter than the rule; the rule gives 0.05, and a half-step is five times looser.

Twice in two commits, the second on the slice fixing the first. Both were found by a
human re-reading the note, and the plan doc for each said plainly that nothing would
catch the next one.

The hazard is not only mislabelling. A tolerance nobody checks can be set wide enough
that the value check passes on anything, and the claim goes on looking like coverage.

---

## The measurement that set the design

Before writing anything, all 48 tolerances were classified against the plain rule: for
every number in each claim's `literal`, half a unit in its last printed place, at scale
1 and at scale 1/100 for prose that speaks percent or points against a fraction.

**34 of 48 followed it exactly.** The remaining 14 fell into three groups, and the
groups are what the design had to fit:

* **Grid times.** Seven claims read a flag arrival or the instant a debt clears — times
  the engine can only report on the step grid. Their tolerance is half a timestep, which
  for a grid quantity is the tightest meaningful bound: the engine hits the claimed step
  or misses by a whole one.
* **Deliberately tighter than the sentence.** A chemistry constant pinned to 1e-4 where
  the sentence prints `1.5`; an exactly-1.0 starting point pinned to 1e-12; the two
  `heat` claims whose stored `value` carries a place the sentence does not.
* **Nothing.** No claim in the file is *looser* than the rule for a legitimate reason.
  That absence is what made the design tractable — see below.

---

## Why there is no "widened" variant

The header used to say the tolerance is widened where the prose hedges, and that the
note says so. That is not what the file does. `just under 14 A` — the one claim whose
note called itself "a deliberately loose one" — holds 0.2 A against a rule of 0.5, which
is under half of what the sentence licenses.

So the design admits no way to be looser than the rule. A tolerance looser than the
sentence's own precision means the sentence is printing more places than the engine has,
and that is the sentence's defect, not the tolerance's. Removing the variant removes the
hardest question in the design — what bounds a widened tolerance — and costs nothing
today.

---

## What landed

Three fields on every claim, and one new test.

```toml
tol          = 5.0e-5
tol_from     = "spelled"     # spelled | tighter | grid
spells       = "0.6387"      # the number the sentence prints, as it prints it
spells_pow10 = 0             # 2 where the prose says % or points and value is a fraction
```

`tol_from` is required with no serde default: a default would hand a new claim a
justification nobody chose, which is the "looks like coverage" shape this file rejects
everywhere else. `every_tolerance_follows_its_declared_rule` then asserts:

| variant | count | what is asserted |
| --- | --- | --- |
| `spelled` | 35 | `tol` is exactly half a unit in `spells`'s last printed place, brought into `value`'s unit by `spells_pow10`; and `spells` is a number in the claim's own literal |
| `tighter` | 7 | the same grounding, and `tol` strictly under that rule |
| `grid` | 6 | quantity is a grid time; no `spells`; `tol` is half the step's `dt`; and no number anywhere in the literal is printed more finely than a half-step |

`tighter` is given no lower bound on purpose. A tolerance smaller than the rule can only
redden this test, never green it, so the hazard the whole check exists for does not live
on that side. What it does need is proof that the rule it beats is still computable,
which is why it still requires `spells`.

`spells` records only the printed places, so the frame need not match the claim's: the
prose may give a duration where the claim reads an absolute time (`**383.0 s later**`
against 983.0 s), or a magnitude where the value is negative (`refused 0.822 A` against
−0.82224 A). A tolerance is a precision, and a precision does not care about sign or
origin. It does care about the unit, which is what `spells_pow10` carries, and it is
always a power of ten in this file.

### The fence that makes `grid` safe

`grid` is the variant that could have re-blessed the defect it was written after, and
the obvious formulation does exactly that. "Grid-time quantities may use half a step"
accepts both the pre-fix and post-fix versions of `**383.0 s later**` — it returns a
grid time either way — so the check would enforce nothing about the thing that has now
bitten twice.

A proximity test does not close it either: the sentence spells `383.0`, a *relative*
duration, and the claim reads the absolute 983.0 s, so no number in that literal is
anywhere near its value.

What closes it is a fence about precision rather than about meaning: **`grid` is
unavailable to any claim whose literal prints a number more finely than a half-step.**
`383.0` prints a tenth, half a unit of which is 0.05, tighter than the 0.25 half-step —
so the claim is not eligible for `grid` at all and must name its number. Checked against
all seven grid candidates: six pass the fence unchanged; the seventh is the defect below.

The fence is unit-blind, and deliberately. `The cell empties at 4146.5 s at 1.9290 V`
spells a voltage as well as a time, and comparing 0.25 s against 5e-5 V is not a
comparison. It errs toward making the author name the number — which is the right error,
and on that claim the number it forces them to name is the binding one.

---

## What it found

**One live tolerance defect, and it is the same one twice in a row.**

`The cell empties at 4146.5 s at 1.9290 V` claimed the flag arrival with a half-step of
0.25. The sentence prints a tenth, so the rule gives 0.05: five times tighter. This is
`a1b0945`'s defect verbatim, sitting in the file at the moment `a1b0945` was written,
one claim away from the one it fixed. **Now 0.05**, and the test passes — the flag lands
on the grid exactly, so the tighter bound costs nothing.

Its step-1 sibling claims the same flag through a sentence that spells no time ("the
last 53 seconds"), so it is a `grid` claim and correctly stays at a half-step. The two
tolerances differ by 5× because the two *sentences* differ, which is what the rule has
said all along.

**Three notes that state a rule they do not follow.** The tolerances are all legal; the
prose beside them was not.

* Both `heat` notes claimed `value` and `tol` "follow this file's own rule — the number
  as the sentence prints it, and half a unit in its last place". The sentence prints
  `0.04` (rule 5e-3); the file holds 5e-4, which is half a unit in the last place of the
  *stored* 0.041. Ten times tighter than the rule, therefore allowed, therefore
  `tighter` — but not the rule. Third instance of the class, found by re-deriving rather
  than re-reading.
* The same note called that tolerance "a hundred times coarser than the row's own last
  digit", concluding the pair is "guarded loosely by value and exactly by display". The
  `heat` row prints two decimals, so its last digit is 0.01 W and 5e-4 is twenty times
  *finer*. The consequence is the opposite of what was written: every value inside the
  tolerance renders as `0.04 W`, so on this pair the display check cannot fail unless
  the value check fails first. Recorded in the note, on the same terms the file already
  uses for `quoted` — an implied check under a green test reads as a covering one.
* `just under 14 A` opened "A deliberately loose one" and is tighter than the rule. The
  looseness it meant is against the row, which is a different comparison — and the right
  one, since `tol` there is two hundred times the `current` row's last digit. That makes
  it the one claim in the file where the display check is demonstrably *not* implied by
  the value check, which is worth stating where the opposite claim used to be.

---

## Reddening

Nine perturbations, one per run, each launched directly with the real exit code read
(`start /wait` hides it — see [[run-tests-below-normal-priority]]) and each required to
name the tolerance test rather than some other check catching the damage first. A
control run on the unperturbed file is required to be green in the same harness.

| # | perturbation | red? |
| --- | --- | --- |
| 0 | control, unperturbed | green, as required |
| 1 | a `spelled` tolerance loosened one decade | ✅ |
| 2 | `spells` changed so it is no longer in its own literal | ✅ |
| 3 | `grid` on a continuous quantity | ✅ |
| 4 | `grid` on a literal that prints a tenth — the `383.0` fence | ✅ |
| 5 | a `grid` claim that also spells a number | ✅ |
| 6 | a `grid` tolerance that is not a half-step | ✅ |
| 7 | a `tighter` tolerance raised to its own rule | ✅ |
| 8 | `tol_from` omitted entirely | ✅ (deserialize) |
| 9 | a tolerance of zero | ✅ |

Case 4 was additionally applied by hand and its failure read directly, because a harness
that reports green while blind has happened in this repo before
([[surface-vs-bulk-slice]]). The message names the claim, both tolerances, and the
number in the literal that forced the fence.

Cases 4 and 5 are the ones that matter: they are the two ways an author could reach for
`grid` to avoid a tolerance they did not want to write. Both are red.

---

## Versions

**Nothing moves.** No engine state, no wire field, no stored layout, no schema, no
version constant. `web/pkg` needs no rebuild: the only Rust in this diff is a test, and
`path-claims.toml` is read by that test alone — the page never loads it. No lesson prose
changed, so no number a reader sees is different.

---

## Deferred, with a price

* **Nothing ties `value` to `literal`.** This is the real remaining hole and it is
  bigger than the one just closed: the literal check is a substring test against the
  prose, the value check compares the engine to `value`, and re-measuring a drifted
  `value` without re-wording the sentence leaves both green with the prose wrong.
  `tol_from` narrows it — `spells` must be a number in the claim's own literal — but
  pins only the *precision* of the prose's number, never its magnitude. Closing it needs
  a vocabulary for how a spelled number maps to a value, and the mappings here are not
  all scales: `0.53 points are gone` is the complement of `0.9947`, `refused 0.822 A` is
  the magnitude of `−0.82224`, and `it goes from 10m to 16m` is a formatter's output.
  The tolerance rule is indifferent to sign, offset, and complement, which is exactly why
  `spells` plus a power of ten is enough for *it* and not enough for this.
* **`spells` is the new unenforced choice, and on a multi-number sentence it has real
  leverage.** The check requires `spells` to be *a* number in the claim's literal, never
  that it is the number stating this claim's quantity. On a sentence with more than one
  figure an author can name the coarsest and take its tolerance. Measured rather than
  reasoned: setting the voltage claim on `The cell empties at 4146.5 s at 1.9290 V` to
  `spells = "4146.5"`, `tol = 0.05` leaves all nine tests green — a voltage pinned a
  thousand times looser than its own sentence licenses. The same leverage exists on
  `0.767 V at 200 s`, `1.731 V at 240`, `99.45 % at 250 s`, `**99.98 %** when the cell
  empties at **207.5 s and 1.9306 V**`, and `` `soh cap` leaves 100.00 % at t = 10 s ``.
  None of the 48 exploit it. The obvious guard — require the spelled number to be near
  `value` — was tried on paper and does not survive this file: `0.53 points are gone`
  states the complement of `0.9947`, `**383.0 s later**` states a duration against an
  absolute 983.0, and `**0.0 %**` states a zero. It is the same missing mapping
  vocabulary as the bullet above, reached from the other side.
* **`spells_pow10` is a power of ten, and one claim's unit is not.** `it goes from `10m`
  to `16m`` states its quantity in minutes. It is a `grid` claim and needs no scale
  today; a future claim that spells a minute figure finely would need a general scale or
  a re-worded sentence.
* **`tighter` records no reason.** It is safe by construction, so the test asks for none
  — but "tighter than the rule and nobody said why" is a smaller version of the problem
  this slice is about. Seven claims carry it and each says why in its `note`, unchecked.
* **The fence is unit-blind.** A future `grid` claim whose literal happens to print a
  fine number in an unrelated unit will be pushed onto `spelled` when a half-step was
  honest. It errs toward explicitness, which is the right direction, but it is not free.
* **Fourteen of twenty-one steps still carry no claim**, unchanged by this slice. This
  one adds no claims at all — it makes the 48 that exist mean what they say.
* **`run_for_s` is still unbounded by anything outside this file**, unchanged. `tol` is
  no longer a free number beside it — but it is not off the list either, only derived:
  the freedom moved into `spells`, one bullet up. Two author-set numbers with no
  enforcement, as before, and one of them now takes a check to reach.
